//! Mandatory four-validator public-transaction and snapshot-restart qualification.
//! Requires a prebuilt native daemon; sandbox denials and missing peers always fail.
use color_eyre::eyre::{self, Result, WrapErr, ensure, eyre};
use futures::future::try_join_all;
use iroha::client::{AccountTransactionDraft, FeeQuoteRequest};
use iroha_data_model::{
    Level,
    account::AccountId,
    isi::{InstructionBox, Log},
    transaction::{FeePaymentIntent, TransactionAdmissionIntent},
};
use iroha_model_base::metadata::Metadata;
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

#[cfg(unix)]
#[path = "support/dataspace_deploy_cli.rs"]
mod dataspace_deploy_cli;
#[path = "support/multiroute.rs"]
mod multiroute;
#[cfg(unix)]
#[path = "support/production_beacon_bootstrap.rs"]
mod production_beacon_bootstrap;
#[path = "support/runtime_catalog_transition.rs"]
mod runtime_catalog_transition;

// The unoptimized four-peer fixture also performs signed-snapshot recovery.
// This is a functional finality gate, not a production latency SLO; use the
// same finite budget as startup/restart while preserving exact Applied checks.
const FUNCTIONAL_FINALITY_TIMEOUT: Duration = Duration::from_secs(180);

// A classified status read has a 500ms server deadline independent of this
// fixture's absolute phase deadline. Retry only that deadline or exact State
// publication contention; invariant failures remain immediate error evidence.
async fn validator_status_until(
    client: &iroha::client::Client,
    deadline: Instant,
) -> Result<iroha_torii_shared::status::Status> {
    let mut last_retryable = None;
    timeout_at(deadline, async {
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            ensure!(
                !remaining.is_zero(),
                "validator status observation exceeded its deadline"
            );
            let mut builder = client.to_builder();
            builder.torii_request_timeout =
                iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT.min(remaining);
            match builder.build()?.status().get().await {
                Ok(status) => return Ok(status),
                Err(iroha::Error::StatusUnavailable {
                    reason: Some(reason @ (iroha::StatusFailureReason::DeadlineElapsed
                        | iroha::StatusFailureReason::StateBusy)),
                    retry_after,
                }) => {
                    if last_retryable.is_none() {
                        eprintln!(
                            "validator status read retry: reason={} remaining={:.3}s",
                            reason.code(),
                            deadline.saturating_duration_since(Instant::now()).as_secs_f64()
                        );
                    }
                    last_retryable = Some(reason);
                    sleep(
                        retry_after
                            .unwrap_or_default()
                            .max(Duration::from_millis(200)),
                    )
                    .await;
                }
                Err(error) => return Err(error.into()),
            }
        }
    })
    .await
    .wrap_err_with(|| {
        format!(
            "validator status observation exceeded its deadline; last retryable reason={}",
            last_retryable.map_or("none", iroha::StatusFailureReason::code)
        )
    })?
}

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

fn signed_snapshot_restart_layers<'a>(
    network: &'a Network,
    peer: &'a NetworkPeer,
) -> impl Iterator<Item = std::borrow::Cow<'a, toml::Table>> {
    // Preserve every shared and node-local layer; only this recovery phase enables writes.
    let snapshot = toml::Table::from_iter([(
        "mode".to_owned(),
        toml::Value::String("read_write".to_owned()),
    )]);
    let layer = toml::Table::from_iter([("snapshot".to_owned(), toml::Value::Table(snapshot))]);
    network
        .config_layers_for_peer(peer)
        .chain(std::iter::once(std::borrow::Cow::Owned(layer)))
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
        peer.start_checked(signed_snapshot_restart_layers(network, peer), Some(&genesis))
            .await?;
        ensure!(peer.latest_stdout_log_path() != previous_log, "restart did not create a new daemon run");
        loop {
            if validator_admission_ready(peer, restart_deadline).await {
                let client = peer.client().client().clone();
                let status = validator_status_until(&client, restart_deadline).await?;
                if status.blocks >= snapshot_height
                    && snapshot_log_contains_height(peer, "Successfully loaded the state from a snapshot", snapshot_height)?
                {
                    // An idle chain creates no empty blocks. Readiness permits admission at the preserved committed tip;
                    // the next exact public transaction proves renewed execution on all four peers.
                    eprintln!("Taira validator restored its signed snapshot and Torii state: snapshot_height={snapshot_height}, committed_height={}", status.blocks);
                    return Ok::<(), eyre::Report>(());
                }
            }
            sleep(Duration::from_millis(200)).await;
        }
    }).await.wrap_err("validator failed signed-snapshot restore and HTTP readiness")??;
    Ok(())
}

#[cfg(test)]
mod status_observation_tests {
    use super::*;
    use iroha::http::{HttpTransport, Response, TransportFuture, TransportRequest};
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    #[derive(Debug)]
    struct StatusTransport {
        responses: Mutex<VecDeque<(u16, Vec<u8>, Option<&'static str>, Option<&'static str>)>>,
        request_budgets: Mutex<Vec<Duration>>,
    }

    impl HttpTransport for StatusTransport {
        fn send_blocking(&self, _: TransportRequest) -> Result<Response<Vec<u8>>> {
            panic!("status observation must use asynchronous reads")
        }

        fn send(&self, request: TransportRequest) -> TransportFuture<'_> {
            Box::pin(async move {
                assert_eq!(request.method, iroha::http::Method::GET);
                assert_eq!(request.url.path(), "/status");
                assert!(
                    request.body.is_empty(),
                    "the observation must not submit work"
                );
                self.request_budgets
                    .lock()
                    .unwrap()
                    .push(request.timeout.unwrap());
                let (status, body, retry_after, reason) = self
                    .responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("unexpected status retry");
                let mut response = Response::builder()
                    .status(status)
                    .header("content-type", "application/json");
                if let Some(retry_after) = retry_after {
                    response = response.header("retry-after", retry_after);
                }
                if let Some(reason) = reason {
                    response = response.header("x-iroha-reject-code", reason);
                }
                Ok(response.body(body)?)
            })
        }
    }

    fn client(transport: Arc<StatusTransport>) -> iroha::client::Client {
        use iroha_crypto::{Hash, HashOf};
        let config = iroha::config::Config {
            chain: "status-observation-test".into(),
            network_id: iroha_data_model::NetworkId::from_genesis_hash(
                HashOf::from_untyped_unchecked(Hash::prehashed([0xA5; Hash::LENGTH])),
            ),
            key_pair: iroha_test_samples::ALICE_KEYPAIR.clone(),
            account: iroha_test_samples::ALICE_ID.clone(),
            account_chain_discriminant: iroha_torii_shared::MINAMOTO_CHAIN_DISCRIMINANT,
            torii_api_url: "http://status-observation.invalid/".parse().unwrap(),
            torii_request_timeout: iroha::config::DEFAULT_TORII_REQUEST_TIMEOUT,
            basic_auth: None,
            transaction_add_nonce: false,
            transaction_ttl: Duration::from_secs(5),
            transaction_status_timeout: Duration::from_secs(10),
            sorafs_alias_cache: iroha::config::AliasCache::default().into_policy(),
            sorafs_anonymity_policy: Default::default(),
            sorafs_rollout_phase: Default::default(),
        };
        iroha::client::Client::builder(config)
            .http_transport(transport)
            .build()
            .unwrap()
    }

    fn transport(
        responses: impl IntoIterator<Item = (u16, Vec<u8>, Option<&'static str>, Option<&'static str>)>,
    ) -> Arc<StatusTransport> {
        Arc::new(StatusTransport {
            responses: Mutex::new(responses.into_iter().collect()),
            request_budgets: Mutex::new(Vec::new()),
        })
    }

    #[tokio::test]
    async fn status_observation_retries_typed_busy_json_and_norito_with_remaining_budget() {
        for reason in [
            iroha::StatusFailureReason::DeadlineElapsed,
            iroha::StatusFailureReason::StateBusy,
        ] {
            let envelope = iroha_torii_shared::ErrorEnvelope::new(reason.code(), "busy");
            let status = iroha_torii_shared::status::Status {
                blocks: 4,
                ..Default::default()
            };
            let transport = transport([
                (
                    503,
                    json::to_vec(&envelope).unwrap(),
                    None,
                    Some(reason.code()),
                ),
                (
                    503,
                    norito::to_bytes(&envelope).unwrap(),
                    None,
                    Some(reason.code()),
                ),
                (200, json::to_vec(&status).unwrap(), None, None),
            ]);
            let client = client(transport.clone());
            let budget = Duration::from_secs(5);
            let observed = validator_status_until(&client, Instant::now() + budget)
                .await
                .unwrap();
            assert_eq!(observed.blocks, 4);
            let budgets = transport.request_budgets.lock().unwrap();
            assert_eq!(budgets.len(), 3);
            assert!(budgets[0] <= budget);
            assert!(
                budgets.windows(2).all(|pair| pair[1] < pair[0]),
                "retries must not renew the caller's deadline"
            );
        }
    }

    #[tokio::test]
    async fn status_observation_stops_at_original_deadline_during_retry_after() {
        for reason in [
            iroha::StatusFailureReason::DeadlineElapsed,
            iroha::StatusFailureReason::StateBusy,
        ] {
            let envelope = iroha_torii_shared::ErrorEnvelope::new(reason.code(), "busy");
            let transport = transport([(
                503,
                json::to_vec(&envelope).unwrap(),
                Some("60"),
                Some(reason.code()),
            )]);
            let client = client(transport.clone());
            let result = tokio::time::timeout(
                Duration::from_secs(2),
                validator_status_until(&client, Instant::now() + Duration::from_millis(80)),
            )
            .await
            .expect("Retry-After must remain bounded by the existing deadline");
            let error = result.unwrap_err().to_string();
            assert!(error.contains("exceeded its deadline"));
            assert!(error.contains(&format!("last retryable reason={}", reason.code())));
            assert_eq!(transport.request_budgets.lock().unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn status_observation_propagates_auth_other_service_and_decode_failures() {
        use iroha::StatusFailureReason;
        for reason in [
            StatusFailureReason::Disabled,
            StatusFailureReason::MailboxUnavailable,
            StatusFailureReason::ActorClosed,
            StatusFailureReason::StateUnavailable,
            StatusFailureReason::CheckpointChanged,
            StatusFailureReason::MissingBlock,
            StatusFailureReason::JournalMismatch,
            StatusFailureReason::CounterOverflow,
            StatusFailureReason::CounterMismatch,
            StatusFailureReason::MetricsStale,
            StatusFailureReason::ProfileRestricted,
        ] {
            let transport = transport([(503, Vec::new(), None, Some(reason.code()))]);
            let client = client(transport.clone());
            let error = validator_status_until(&client, Instant::now() + Duration::from_secs(5))
                .await
                .unwrap_err();
            assert!(matches!(
                error.downcast_ref::<iroha::Error>(),
                Some(iroha::Error::StatusUnavailable { reason: Some(actual), .. }) if *actual == reason
            ));
            assert_eq!(transport.request_budgets.lock().unwrap().len(), 1);
        }
        for (status, body, reason) in [
            (401, b"unauthorized".to_vec(), None),
            // A recognized code at another HTTP status is not retry authority.
            (429, Vec::new(), Some("status_deadline_elapsed")),
            (429, Vec::new(), Some("status_state_busy")),
            (503, Vec::new(), Some("another_service_unavailable")),
            (503, Vec::new(), Some("status_metrics_unavailable")),
            // Only the SDK's typed header classification is authoritative.
            (
                503,
                br#"{"code":"status_deadline_elapsed","message":"busy"}"#.to_vec(),
                None,
            ),
            (
                503,
                br#"{"code":"status_state_busy","message":"busy"}"#.to_vec(),
                None,
            ),
            (503, b"malformed service error".to_vec(), None),
            (200, b"malformed status".to_vec(), None),
        ] {
            let transport = transport([(status, body, None, reason)]);
            let client = client(transport.clone());
            let error = validator_status_until(&client, Instant::now() + Duration::from_secs(5))
                .await
                .unwrap_err();
            match status {
                200 => assert!(matches!(
                    error.downcast_ref::<iroha::Error>(),
                    Some(iroha::Error::Decode {
                        operation: "diagnostic.status",
                        ..
                    })
                )),
                503 => assert!(matches!(
                    error.downcast_ref::<iroha::Error>(),
                    Some(iroha::Error::StatusUnavailable { reason: None, .. })
                )),
                _ => assert!(matches!(
                    error.downcast_ref::<iroha::Error>(),
                    Some(iroha::Error::Http { operation: "diagnostic.status", status: actual, .. }) if *actual == status
                )),
            }
            assert_eq!(transport.request_budgets.lock().unwrap().len(), 1);
        }
    }
}
