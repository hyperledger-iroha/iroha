//! Mandatory four-validator public-transaction and snapshot-restart qualification.
//! Requires a prebuilt native daemon; sandbox denials and missing peers always fail.
use color_eyre::eyre::{self, Result, WrapErr, ensure, eyre};
use futures::future::try_join_all;
use iroha::client::{AccountTransactionDraft, FeeQuoteRequest};
use iroha_data_model::{
    Level,
    account::AccountId,
    isi::{InstructionBox, Log},
    metadata::Metadata,
    transaction::{FeePaymentIntent, TransactionAdmissionIntent},
};
use iroha_test_network::{
    Network, NetworkPeer, init_instruction_registry, read_on_dedicated_thread,
};
use norito::json::{self, Value};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
    time::Duration,
};
use tokio::time::{Instant, sleep, timeout_at};

#[path = "support/multiroute.rs"]
mod multiroute;

// This fixture uses a fixed loopback HTTP listener, so a bounded status-line
// probe needs no additional HTTP client dependency or runtime signing context.
async fn validator_admission_ready(peer: &NetworkPeer, deadline: Instant) -> bool {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    let probe_deadline = (Instant::now() + Duration::from_secs(2)).min(deadline);
    let result = timeout_at(probe_deadline, async {
        let address = peer.api_address().to_string();
        let mut stream = tokio::net::TcpStream::connect(&address).await?;
        stream
            .write_all(
                format!("GET /readyz HTTP/1.1\r\nHost: {address}\r\nAccept: text/plain, application/json\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await?;
        let mut line = String::new();
        BufReader::new(stream.take(256))
            .read_line(&mut line)
            .await?;
        Ok::<bool, std::io::Error>(
            line.ends_with("\r\n")
                && (line.starts_with("HTTP/1.1 200 ") || line.starts_with("HTTP/1.0 200 ")),
        )
    })
    .await;
    matches!(result, Ok(Ok(true)))
}

async fn verify_basic_public_doctor(peer: &NetworkPeer) -> Result<()> {
    let binary = std::env::var_os("TEST_NETWORK_BIN_IROHA")
        .ok_or_else(|| eyre!("TEST_NETWORK_BIN_IROHA must name the prebuilt native CLI"))?;
    let root = format!("http://{}", peer.api_address());
    let deadline = Instant::now() + Duration::from_secs(60);
    timeout_at(deadline, async {
        while !validator_admission_ready(peer, deadline).await {
            sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .wrap_err("validator admission did not become ready for the basic doctor")?;
    // Exercise the real CLI consumer against the real daemon catalogue before
    // release compilation. Small mock tool lists cannot qualify this boundary.
    let output = timeout_at(
        deadline,
        tokio::process::Command::new(binary)
            .env_clear()
            .args([
                "--machine",
                "taira",
                "doctor",
                "--scope",
                "basic",
                "--public-root",
                &root,
                "--json",
            ])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .wrap_err("basic public doctor exceeded its fixture deadline")??;
    ensure!(
        output.status.success(),
        "basic public doctor rejected the actual daemon: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let report: Value =
        json::from_slice(&output.stdout).wrap_err("basic public doctor returned invalid JSON")?;
    ensure!(
        report.get("status").and_then(Value::as_str) == Some("ok")
            && report.get("scope").and_then(Value::as_str) == Some("basic"),
        "basic public doctor omitted its successful scope"
    );
    eprintln!("Taira basic public doctor passed against the actual native daemon");
    Ok(())
}

fn snapshot_log_contains_height(peer: &NetworkPeer, message: &str, height: u64) -> Result<bool> {
    for path in [peer.latest_stdout_log_path(), peer.latest_stderr_log_path()]
        .into_iter()
        .flatten()
    {
        for line in BufReader::new(fs::File::open(path)?).lines() {
            let line = line?;
            if !line.contains(message) {
                continue;
            }
            let Ok(record) = json::from_str::<Value>(&line) else {
                continue;
            };
            if record.get("fields").is_some_and(|fields| {
                fields.get("message").and_then(Value::as_str) == Some(message)
                    && fields.get("at_height").and_then(Value::as_u64) == Some(height)
            }) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn published_snapshot_height(peer: &NetworkPeer) -> Result<Option<u64>> {
    let root = peer.kura_store_dir().join("snapshot");
    let pointer = match fs::read_to_string(root.join("current")) {
        Ok(pointer) => pointer,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let digest = pointer.trim();
    ensure!(
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "snapshot generation pointer is malformed"
    );
    let generation = root.join("generations").join(digest);
    for artifact in [
        "snapshot.data",
        "snapshot.sha256",
        "snapshot.sig",
        "snapshot.fast.norito",
        "snapshot.merkle.json",
    ] {
        let metadata = fs::metadata(generation.join(artifact))?;
        ensure!(
            metadata.is_file() && metadata.len() > 0,
            "published snapshot is missing its complete signed artifact set"
        );
    }
    let snapshot: Value = json::from_slice(&fs::read(generation.join("snapshot.data"))?)?;
    let hashes = snapshot
        .get("block_hashes")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre!("published snapshot has no committed block-hash vector"))?;
    Ok(Some(u64::try_from(hashes.len())?))
}

async fn restart_validator_from_applied_snapshot(
    network: &Network,
    applied_height: u64,
) -> Result<()> {
    let peer = &network.peers()[0];
    // Wait for an actual completed generation before asking the harness to stop the process.
    // Its bounded shutdown may force-kill a slow daemon; this must still exercise cold restore.
    let snapshot_deadline = Instant::now() + Duration::from_secs(60);
    timeout_at(snapshot_deadline, async {
        loop {
            if let Some(height) = published_snapshot_height(peer)?
                && height >= applied_height
                && snapshot_log_contains_height(
                    peer,
                    "Successfully created a snapshot of state",
                    height,
                )?
            {
                return Ok::<(), eyre::Report>(());
            }
            sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .wrap_err(
        "validator did not publish a signed snapshot after the exact Applied transaction",
    )??;
    let previous_log = peer.latest_stdout_log_path();
    peer.shutdown().await;
    // Shutdown may publish a newer complete generation; qualify the one startup will read.
    let snapshot_height = published_snapshot_height(peer)?
        .ok_or_else(|| eyre!("validator lost its published snapshot during shutdown"))?;
    ensure!(
        snapshot_height >= applied_height,
        "shutdown snapshot regressed behind the Applied transaction"
    );
    let genesis = network.genesis();
    let restart_deadline = Instant::now() + Duration::from_secs(180);
    timeout_at(restart_deadline, async {
        peer.start_checked(network.config_layers_for_peer(peer), Some(&genesis)).await?;
        ensure!(peer.latest_stdout_log_path() != previous_log, "restart did not create a new daemon run");
        loop {
            let remaining = restart_deadline.saturating_duration_since(Instant::now());
            ensure!(!remaining.is_zero(), "validator snapshot restart exceeded its deadline");
            let mut builder = peer.client().client().to_builder();
            builder.torii_request_timeout = iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT.min(remaining);
            let client = builder.build()?;
            if validator_admission_ready(peer, restart_deadline).await
                && let Ok(status) = client.status().get().await
                && status.blocks >= snapshot_height
                && snapshot_log_contains_height(peer, "Successfully loaded the state from a snapshot", snapshot_height)?
            {
                // An idle chain creates no empty blocks. Readiness permits admission at the preserved committed tip;
                // the next exact public transaction proves renewed execution on all four peers.
                eprintln!("Taira validator restored its signed snapshot and Torii state: snapshot_height={snapshot_height}, committed_height={}", status.blocks);
                return Ok::<(), eyre::Report>(());
            }
            sleep(Duration::from_millis(200)).await;
        }
    }).await.wrap_err("validator failed signed-snapshot restore and HTTP readiness")??;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn four_peer_multiroute_public_transaction_sequence_reaches_applied() -> Result<()> {
    public_transaction_sequence_reaches_applied(false).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn four_peer_universal_public_transaction_sequence_reaches_applied() -> Result<()> {
    public_transaction_sequence_reaches_applied(true).await
}

async fn public_transaction_sequence_reaches_applied(universal_route: bool) -> Result<()> {
    init_instruction_registry();
    for variable in ["TEST_NETWORK_BIN_IROHAD", "TEST_NETWORK_BIN_IROHA"] {
        let binary = std::env::var_os(variable)
            .ok_or_else(|| eyre!("{variable} must name the prebuilt native executable"))?;
        ensure!(
            Path::new(&binary).is_file(),
            "{variable} must name an existing executable file"
        );
    }
    let startup_deadline = Instant::now() + Duration::from_secs(180);
    let network = timeout_at(
        startup_deadline,
        tokio::task::spawn_blocking(move || {
            multiroute::network_builder()
                .with_config_layer(|layer| {
                    layer
                        .write(["torii", "mcp", "enabled"], true)
                        .write(["torii", "mcp", "profile"], "writer")
                        .write(
                            ["torii", "mcp", "allow_tool_prefixes"],
                            toml::Value::Array(vec![toml::Value::String("iroha.".to_owned())]),
                        )
                        .write(["snapshot", "mode"], "read_write")
                        .write(["snapshot", "store_dir"], "./storage/snapshot")
                        .write(["snapshot", "create_every_ms"], 1_000_i64)
                        .write(["logger", "format"], "json")
                        .write(["logger", "level"], "INFO");
                })
                .with_base_seed_if_unset(if universal_route {
                    "four_peer_universal_public_transaction_sequence_reaches_applied"
                } else {
                    "four_peer_multiroute_public_transaction_sequence_reaches_applied"
                })
                .build()
        }),
    )
    .await
    .wrap_err("four-peer genesis preparation exceeded its deadline")?
    .wrap_err("four-peer genesis preparation failed")?;
    let result = async {
        timeout_at(startup_deadline, async {
            network.start_all().await?;
            network.ensure_blocks(1).await?;
            Ok::<(), eyre::Report>(())
        })
        .await
        .wrap_err("four-peer startup exceeded its deadline")??;
        ensure!(network.peers().len() == 4, "the fixture must start all four validators");
        let initial = timeout_at(startup_deadline, try_join_all(network.peers().iter().map(|peer| async move {
            let remaining = startup_deadline.saturating_duration_since(Instant::now());
            ensure!(!remaining.is_zero(), "four-peer startup observation exceeded its deadline");
            let mut builder = peer.client().client().to_builder();
            builder.torii_request_timeout = iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT.min(remaining);
            let client = builder.build()?;
            client.status().get().await.map_err(eyre::Report::from)
        }))).await.wrap_err("four-peer startup observation exceeded its deadline")??;
        ensure!(initial.iter().all(|status| status.blocks >= 1), "all peers must apply genesis");
        verify_basic_public_doctor(&network.peers()[0]).await?;
        // Both scopes retain the same four-validator, three-dataspace topology.
        // Basic BPNG traffic uses the funded universal default-route account;
        // the full scope also exercises ALICE's explicit lane-1/dataspace-1 route.
        let fixture_client = if universal_route {
            let key_pair = multiroute::universal_route_key_pair();
            let account_id = AccountId::new(key_pair.public_key().clone());
            ensure!(account_id != *iroha_test_samples::ALICE_ID
                && account_id != *iroha_test_samples::BOB_ID,
                "universal fixture account must not match an explicit account route");
            network.peers()[0].client_for(&account_id, key_pair.private_key().clone())
        } else {
            network.client()
        };
        let mut builder = fixture_client.client().to_builder();
        builder.transaction_status_timeout = Duration::from_secs(75);
        // Public QueuePlan certification uses the SDK's routed request budget.
        builder.torii_request_timeout = iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT;
        let client = builder.build()?;
        let account = client.account_client()?;
        let mut preceding_applied_height = 1;
        // Exercise admission, autonomous execution and height rollover repeatedly.
        // A successful first transaction alone does not qualify continued progress.
        for sequence in 1..=3 {
        // Public Torii ingress requires signature-bound QueuePlanSynced admission.
        // Internal Ordinary work has separate Core candidate-provider regressions.
        let mut payload = account.prepare_transaction(
            AccountTransactionDraft::new(
                vec![InstructionBox::from(Log::new(Level::INFO, format!("strict Taira public transaction {sequence}")))],
                FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            ).with_admission_intent(TransactionAdmissionIntent::QueuePlanSynced),
        )?;
        let quote = account.quote_fees(FeeQuoteRequest::AccountSignature { payload: &payload }).await?;
        ensure!(payload.fee_payment.has_same_payer_and_gas_bound(&quote.intent), "fee quote changed the selected payer or gas bound");
        payload.fee_payment = quote.intent;
        let transaction = account.sign_transaction(payload)?;
        let expected_hash = transaction.hash();
        let submitted_hash = account.submit_transaction_and_wait(&transaction).await
            .wrap_err("the exact public transaction did not reach state-resolved Applied")?;
        ensure!(submitted_hash == expected_hash, "submission returned a different signed transaction hash");
        let expected_hex = expected_hash
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        // The all-peer observation deadline starts after SDK reconciliation;
        // it must not cancel a potentially durable public admission.
        let observation_deadline = Instant::now() + Duration::from_secs(90);
        let result = timeout_at(observation_deadline, async {
        loop {
            let observations = try_join_all(network.peers().iter().map(|peer| async move {
                let observation_client = peer.client().client().clone();
                // Global status can use Torii's routed/fanout budget. Bound each
                // read by the same observation deadline, never an arbitrary
                // shorter timeout or a new deadline for each peer/request.
                let bounded_client = move || -> Result<iroha::client::Client> {
                    let remaining = observation_deadline.saturating_duration_since(Instant::now());
                    ensure!(!remaining.is_zero(), "four-peer public transaction observation exceeded its fixed 90-second deadline");
                    let mut builder = observation_client.to_builder();
                    builder.torii_request_timeout = iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT.min(remaining);
                    Ok(builder.build()?)
                };
                let global = bounded_client()?.fetch_transaction_status_response_global(expected_hash).await?;
                // Global lookups may fan out to another validator. Prove this
                // peer's own committed state before counting it as applied.
                let status = bounded_client()?.status().get().await?;
                let local = read_on_dedicated_thread(move || {
                    // Recompute after thread scheduling, immediately before
                    // the blocking request takes its remaining I/O budget.
                    bounded_client()?.get_transaction_status_response_local(expected_hash)
                }).await?;
                Ok::<_, eyre::Report>((status.blocks, global, local))
            })).await?;
            let all_applied = observations.iter().all(|(height, global, local)| {
                [("global", global), ("local", local)].iter().all(|(scope, response)| response.as_ref().is_some_and(|response| {
                    response.hash == expected_hex
                        && response.scope == *scope
                        && response.resolved_from == "state"
                        && response.status.kind == "Applied"
                        && response.status.block_height.is_some_and(|applied| applied > 1 && *height >= applied)
                }))
            });
            if all_applied {
                let applied_height = observations[0].1.as_ref().unwrap().status.block_height;
                ensure!(observations.iter().all(|(_, global, local)| global.as_ref().unwrap().status.block_height == applied_height && local.as_ref().unwrap().status.block_height == applied_height), "peers disagree on the exact transaction's applied height");
                eprintln!("Taira four-peer public transaction Applied in local and global state: hash={expected_hex}, height={applied_height:?}, peer_heights={:?}", observations.iter().map(|(height, _, _)| *height).collect::<Vec<_>>());
                return Ok(applied_height.expect("all observations have an Applied height"));
            }
            eprintln!("waiting for all four peers to apply exact public transaction {expected_hex}: {observations:?}");
            sleep(Duration::from_millis(200)).await;
        }
    }).await;
        let applied_height = result.map_err(|_| {
            eyre!("four-peer public transaction observation exceeded its fixed 90-second deadline")
        })??;
        ensure!(applied_height > preceding_applied_height, "each sequential transaction must reach a later committed height");
        preceding_applied_height = applied_height;
        if sequence == 2 {
            restart_validator_from_applied_snapshot(&network, applied_height).await?;
        }
        }
        Ok(())
    }
    .await;
    network.shutdown().await;
    result
}
