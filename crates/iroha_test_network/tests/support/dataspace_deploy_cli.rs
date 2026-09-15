//! Clean-client deployment uses the shipping CLI, three durable transactions and four peers.
//! Fixture-owned genesis/configuration establish trust before any proof response is read.
use super::*;
use iroha_config::base::read::ConfigReader;
use iroha_core::release_identity::BuildIdentity;
use iroha_crypto::{ExposedPrivateKey, Hash, KeyPair, PublicKey};
use iroha_data_model::{
    alias_setup::{AliasDataspaceBootstrapGrantV1, AliasPlanDispositionV1, AliasTransactionPlanV1},
    isi::SetParameter,
    nexus::{LaneLifecycleStatusV1, NexusCatalogTransitionV1},
    parameter::Parameter,
    transaction::{Executable, SignedTransaction},
};
use iroha_model_base::{peer::PeerId, topology::LaneId};
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use iroha_version::codec::DecodeVersioned as _;
use norito::{codec::Encode as _, json::JsonSerialize};
use std::{collections::BTreeMap, path::PathBuf};

const CHAIN: &str = "fc56984b-2be7-431d-840e-21514d1883f0";
const OPERATION: &str = "clean-client-dpn";
const CADENCE: Duration = Duration::from_secs(2);
const PHASES: [&str; 3] = ["catalog", "bootstrap", "aliases"];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(value: &str) -> Result<Vec<u8>> {
    ensure!(
        value.len() % 2 == 0 && value.is_ascii(),
        "invalid wire hexadecimal"
    );
    let bytes = (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ensure!(hex(&bytes) == value, "noncanonical wire hexadecimal");
    Ok(bytes)
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a Value> {
    value
        .get(name)
        .ok_or_else(|| eyre!("native output omitted {name}"))
}

fn text_field<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    field(value, name)?
        .as_str()
        .ok_or_else(|| eyre!("{name} is not text"))
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::{io::Write as _, os::unix::fs::OpenOptionsExt as _};
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

#[derive(JsonSerialize)]
struct TrustPeer {
    torii_origin: String,
    peer_id: PeerId,
    node_fingerprint: Hash,
    build_fingerprint: Hash,
    config_fingerprint: Hash,
}

#[derive(JsonSerialize)]
struct Trust {
    genesis_public_key: PublicKey,
    genesis_signed_wire_hex: String,
    peers: Vec<TrustPeer>,
}

/// Parse only the freshly created fixture's native configuration, never a live deployment file.
fn fixture_trust(network: &Network, genesis_key: &KeyPair, build: BuildIdentity) -> Result<Trust> {
    let wire = network.genesis().0.encode_wire()?;
    let (genesis_hash, metadata) =
        iroha_core::release_identity::genesis_identity(&wire, genesis_key.public_key())?;
    ensure!(
        genesis_hash == Hash::from(network.genesis().0.hash()),
        "fixture genesis drift"
    );
    let genesis_peers = iroha_genesis::signed_genesis_validator_pops(&network.genesis().0)?;
    ensure!(
        genesis_peers.len() == 4,
        "genesis must contain four BLS validators"
    );
    let mut peers = Vec::new();
    for peer in network.peers() {
        ensure!(
            genesis_peers
                .iter()
                .any(|(key, pop)| Some(key) == peer.bls_public_key()
                    && Some(pop.as_slice()) == peer.bls_pop()),
            "signed genesis must bind each independently generated peer and PoP"
        );
        // The first run's config and extends files were just authored by NetworkPeer.
        let log = peer
            .latest_stdout_log_path()
            .ok_or_else(|| eyre!("missing fixture run log"))?;
        let config_path = log
            .parent()
            .ok_or_else(|| eyre!("fixture log has no parent"))?
            .join("run-1-config.toml");
        let config = ConfigReader::new()
            .without_env()
            .read_toml_with_extends(config_path)
            .map_err(|error| eyre!("read native fixture configuration: {error:?}"))?
            .read_and_complete::<iroha_config::parameters::user::Root>()
            .map_err(|error| eyre!("decode native fixture configuration: {error:?}"))?
            .parse()
            .map_err(|error| eyre!("validate native fixture configuration: {error:?}"))?;
        ensure!(
            config.common.chain.to_string() == CHAIN,
            "fixture chain differs"
        );
        let id = peer.network_peer_id();
        ensure!(config.common.peer.id == id, "fixture config peer differs");
        let shared = config.sumeragi.v2_config(
            Duration::from_millis(metadata.block_cadence_ms.get()),
            metadata.mode.into(),
        )?;
        peers.push(TrustPeer {
            torii_origin: format!("{}/", peer.torii_url()),
            node_fingerprint: Hash::new(id.encode()),
            peer_id: id,
            build_fingerprint: build.build_fingerprint(),
            config_fingerprint: shared.fingerprint(),
        });
    }
    Ok(Trust {
        genesis_public_key: genesis_key.public_key().clone(),
        genesis_signed_wire_hex: hex(&wire),
        peers,
    })
}

struct Cli {
    binary: PathBuf,
    config: PathBuf,
    operator: PathBuf,
}

impl Cli {
    async fn run(&self, arguments: &[&str], deadline: Instant) -> Result<Value> {
        let output = timeout_at(
            deadline,
            tokio::process::Command::new(&self.binary)
                .env_clear()
                .args(["--machine", "--config"])
                .arg(&self.config)
                .arg("--operator-private-key-file")
                .arg(&self.operator)
                .args(["taira", "dataspace-deploy"])
                .args(arguments)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .wrap_err("native dataspace deployment CLI exceeded its fixed deadline")??;
        ensure!(
            output.status.success(),
            "native CLI {:?} failed: stdout={} stderr={}",
            arguments.first(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        json::from_slice(&output.stdout).wrap_err("native CLI did not return its typed JSON report")
    }
}

async fn drained(network: &Network, deadline: Instant) -> Result<Vec<(u64, u64, u64)>> {
    timeout_at(deadline, async {
        loop {
            let statuses = try_join_all(network.peers().iter().map(|peer| async move {
                validator_status_until(peer.client().client(), deadline).await
            }))
            .await?;
            if statuses.iter().all(|status| status.queue_size == 0)
                && statuses
                    .iter()
                    .all(|status| status.blocks == statuses[0].blocks)
            {
                return Ok(statuses
                    .iter()
                    .map(|status| (status.blocks, status.txs_approved, status.txs_rejected))
                    .collect());
            }
            sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .wrap_err("four peers did not converge with empty queues")?
}

fn retained_transactions(
    operation: &Path,
    network: &Network,
    plan: &Value,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let transition: NexusCatalogTransitionV1 =
        json::from_value(field(plan, "catalog_transition")?.clone())?;
    let grant: AliasDataspaceBootstrapGrantV1 =
        json::from_value(field(plan, "bootstrap_grant")?.clone())?;
    let mut retained = BTreeMap::new();
    for phase in PHASES {
        let name = format!("{phase}.prepared.json");
        let bytes = fs::read(operation.join(&name))?;
        let prepared: Value = json::from_slice(&bytes)?;
        ensure!(
            text_field(&prepared, "phase")? == phase
                && text_field(&prepared, "operation_id")? == OPERATION,
            "retained phase identity differs"
        );
        let wire = unhex(text_field(&prepared, "signed_transaction_wire_hex")?)?;
        let tx = SignedTransaction::decode_all_versioned(&wire)?;
        tx.verify_signature()?;
        ensure!(
            tx.encode_wire_v1()? == wire
                && tx.authority() == &*ALICE_ID
                && tx.network_id() == Some(&network.network_id())
                && tx.admission_intent() == TransactionAdmissionIntent::QueuePlanSynced
                && hex(tx.hash().as_ref()) == text_field(&prepared, "transaction_hash")?,
            "retained transaction differs from the exact configured owner/network/wire"
        );
        let expected: Vec<InstructionBox> = match phase {
            "catalog" => vec![
                SetParameter::new(Parameter::Custom(
                    transition.clone().into_custom_parameter()?,
                ))
                .into(),
            ],
            "bootstrap" => vec![
                SetParameter::new(Parameter::Custom(grant.clone().into_custom_parameter()?)).into(),
            ],
            "aliases" => {
                let aliases: AliasTransactionPlanV1 =
                    json::from_value(field(&prepared, "alias_plan")?.clone())?;
                ensure!(
                    aliases.verify_hash()
                        && aliases.body.resources.len() == 2
                        && aliases.body.instructions.len() == 2
                        && aliases.body.blockers.is_empty(),
                    "aliases must retain the exact two-resource native paid plan"
                );
                let price = "0.5".parse::<iroha_primitives::numeric::Quantity>()?;
                ensure!(
                    aliases
                        .body
                        .resources
                        .iter()
                        .all(
                            |resource| resource.disposition == AliasPlanDispositionV1::Create
                                && resource
                                    .quote
                                    .as_ref()
                                    .is_some_and(|quote| quote.exact_amount == price)
                        ),
                    "both aliases must be paid creates at the selected native price"
                );
                aliases
                    .body
                    .instructions
                    .iter()
                    .map(|frame| {
                        iroha_data_model::isi::decode_instruction_from_pair(
                            &frame.wire_id,
                            &frame.framed_payload,
                        )
                        .map_err(Into::into)
                    })
                    .collect::<Result<Vec<_>>>()?
            }
            _ => unreachable!("closed three-phase set"),
        };
        ensure!(
            tx.instructions() == &Executable::from(expected),
            "unexpected phase instructions"
        );
        let prepared_digest = hex(&iroha_crypto::sha256(&bytes));
        retained.insert(name, bytes);
        for suffix in ["submitted.json", "submission-result.json"] {
            let name = format!("{phase}.{suffix}");
            let bytes = fs::read(operation.join(&name))?;
            let value: Value = json::from_slice(&bytes)?;
            if suffix == "submitted.json" {
                let claim = value
                    .as_str()
                    .ok_or_else(|| eyre!("dispatch claim is not text"))?;
                ensure!(
                    claim == prepared_digest,
                    "dispatch claim must bind the exact native canonical prepared JSON"
                );
            } else {
                ensure!(
                    text_field(&value, "transaction_hash")? == hex(tx.hash().as_ref())
                        && field(&value, "accepted")?.as_bool() == Some(true)
                        && field(&value, "error")?.is_null(),
                    "phase {phase} submission was not accepted exactly: {value:?}"
                );
            }
            retained.insert(name, bytes);
        }
    }
    let names = fs::read_dir(operation)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<std::io::Result<Vec<_>>>()?;
    for suffix in [
        ".prepared.json",
        ".submitted.json",
        ".submission-result.json",
    ] {
        ensure!(
            names
                .iter()
                .filter(|name| name.to_string_lossy().ends_with(suffix))
                .count()
                == 3,
            "each of the three phases must have exactly one prepared/claimed/submission record"
        );
    }
    retained.insert("plan.json".into(), fs::read(operation.join("plan.json"))?);
    Ok(retained)
}

fn assert_completed(report: &Value, operation: &Path, trust: &Trust) -> Result<()> {
    ensure!(
        text_field(report, "state")? == "completed"
            && field(report, "deployment_complete")?.as_bool() == Some(true)
            && field(report, "verification_error")?.is_null(),
        "native deployment did not complete: {report:?}"
    );
    let receipt = text_field(report, "completion_receipt")?;
    ensure!(
        !receipt.contains('/') && receipt.starts_with("completion-") && receipt.ends_with(".json"),
        "invalid completion receipt path"
    );
    let completion: Value = json::from_slice(&fs::read(operation.join(receipt))?)?;
    let peers = field(&completion, "peers")?
        .as_array()
        .ok_or_else(|| eyre!("completion peers missing"))?;
    ensure!(
        peers.len() == 4,
        "native completion must verify all four validators"
    );
    for (observed, expected) in peers.iter().zip(&trust.peers) {
        let id: PeerId = json::from_value(field(observed, "peer_id")?.clone())?;
        ensure!(
            id == expected.peer_id,
            "completion changed ordered peer identity"
        );
        let transactions = field(observed, "transactions")?
            .as_array()
            .ok_or_else(|| eyre!("completion transactions missing"))?;
        ensure!(
            transactions.len() == 3,
            "every peer must authenticate all three phases"
        );
        for (transaction, phase) in transactions.iter().zip(PHASES) {
            ensure!(
                text_field(transaction, "phase")? == phase
                    && text_field(transaction, "state")? == "applied_verification_pending",
                "one peer lacks an exact applied phase"
            );
        }
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn clean_client_deploys_paid_dataspace_once_with_four_peer_finality() -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    init_instruction_registry();
    let cli_binary = std::env::var_os("TEST_NETWORK_BIN_IROHA")
        .map(PathBuf::from)
        .ok_or_else(|| eyre!("prebuilt native CLI is required"))?;
    let daemon = std::env::var_os("TEST_NETWORK_BIN_IROHAD")
        .map(PathBuf::from)
        .ok_or_else(|| eyre!("prebuilt native daemon is required"))?;
    ensure!(
        cli_binary.is_file() && daemon.is_file(),
        "exact native executables are absent"
    );
    // The maintained native graph pins this same source/version into daemon and test executable.
    let build = iroha_core::compiled_build_identity!()?;
    let genesis_key = KeyPair::try_random()?;
    let operator_key = KeyPair::try_random()?;
    let genesis_for_builder = genesis_key.clone();
    let operator_public = operator_key.public_key().to_string();
    let startup = Instant::now() + Duration::from_secs(180);
    let network = timeout_at(
        startup,
        tokio::task::spawn_blocking(move || {
            let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
            NetworkBuilder::new()
                .with_peers(4)
                .with_auto_populated_trusted_peers()
                .with_npos_consensus()
                .with_genesis_keypair(genesis_for_builder)
                .with_block_cadence(CADENCE)
                .with_config_layer(move |layer| {
                    layer
                        .write("chain", CHAIN)
                        .write("chain_discriminant", 369_i64)
                        .write(["torii", "operator_signatures", "enabled"], true)
                        .write(
                            ["torii", "operator_signatures", "allowed_public_keys"],
                            toml::Value::Array(vec![toml::Value::String(operator_public.clone())]),
                        )
                        .write(["snapshot", "mode"], "disabled")
                        .write(["logger", "format"], "json")
                        .write(["logger", "level"], "INFO");
                })
                .build()
        }),
    )
    .await
    .wrap_err("clean-client genesis preparation timed out")??;
    let result = async {
        timeout_at(startup, async {
            network.start_all().await?;
            network.ensure_blocks(1).await?;
            for peer in network.peers() {
                while !validator_admission_ready(peer, startup).await {
                    sleep(Duration::from_millis(200)).await;
                }
            }
            Ok::<(), eyre::Report>(())
        }).await.wrap_err("clean-client four-peer startup timed out")??;
        ensure!(network.chain_id().to_string() == CHAIN && network.peers().len() == 4,
            "canonical Taira four-peer fixture required");
        let trust = fixture_trust(&network, &genesis_key, build)?;
        let root = fs::canonicalize(network.env_dir())?.join("clean-client");
        fs::create_dir(&root)?; fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        let journal = root.join("journal");
        fs::create_dir(&journal)?; fs::set_permissions(&journal, fs::Permissions::from_mode(0o700))?;
        let operator = root.join("operator.key");
        write_private(&operator, format!("{}\n", ExposedPrivateKey(operator_key.private_key().clone())).as_bytes())?;
        let account_key = root.join("owner.key");
        write_private(&account_key, format!("{}\n", ExposedPrivateKey(ALICE_KEYPAIR.private_key().clone())).as_bytes())?;
        let mut client = toml::Table::new();
        iroha_config::base::toml::Writer::new(&mut client)
            .write("chain", CHAIN).write("network_id", network.network_id().to_string())
            .write("torii_url", format!("{}/", network.peers()[0].torii_url()))
            .write(["account", "domain"], "universal").write(["account", "profile"], "taira")
            .write(["account", "public_key"], ALICE_KEYPAIR.public_key().to_string())
            .write(["account", "private_key_file"], account_key.to_str().unwrap())
            .write(["transaction", "time_to_live_ms"], 600_000_i64)
            .write(["transaction", "status_timeout_ms"], 30_000_i64)
            .write("torii_request_timeout_ms", 30_000_i64);
        let config = root.join("client.toml");
        write_private(&config, toml::to_string(&client)?.as_bytes())?;
        let cli = Cli { binary: cli_binary, config, operator };
        let trust_path = root.join("trust.json");
        write_private(&trust_path, &json::to_vec(&trust)?)?;
        let mut validators = Vec::new();
        for peer in network.peers() {
            let validator = peer.account_id().to_i105_for_discriminant(369)?;
            let peer_id = peer.network_peer_id().to_string();
            validators.push(norito::json!({"validator": validator, "peer_id": peer_id}));
        }
        let manifest = norito::json!({"lane": "dpn", "governance": "parliament", "version": 1,
            "validators": validators, "quorum": 3});
        let manifest_path = root.join("lane.json");
        write_private(&manifest_path, &json::to_vec(&manifest)?)?;
        let bundle = root.join("intent");
        let payment = iroha_config::parameters::defaults::nexus::fees::fee_asset_id();
        let idle_before = drained(&network, startup).await?;
        let deadline = Instant::now() + Duration::from_secs(180);
        cli.run(&["init", "--dataspace", "dpn", "--lane-id", "6", "--lane-profile", "restricted-full-replica",
            "--account-alias", "admin", "--lane-manifest", manifest_path.to_str().unwrap(),
            "--trust", trust_path.to_str().unwrap(), "--payment-asset", &payment,
            "--alias-create-maximum", "0.5", "--transaction-fee-maximum", "100",
            "--lease-years", "1", "--quote-lifetime-secs", "3600", "--operation-id", OPERATION,
            "--output-dir", bundle.to_str().unwrap()], deadline).await?;
        let deployment = bundle.join("deployment.json");
        let plan = cli.run(&["plan", "--manifest", deployment.to_str().unwrap(),
            "--journal-dir", journal.to_str().unwrap()], deadline).await?;
        let baseline: LaneLifecycleStatusV1 = json::from_value(field(&plan, "baseline")?.clone())?;
        ensure!(baseline.lanes.iter().map(|lane| lane.id).collect::<Vec<_>>() == [LaneId::new(0)],
            "fixture should start from the native single-lane baseline");
        ensure!(drained(&network, deadline).await? == idle_before, "init/plan submitted a transaction");
        let operation = journal.join(OPERATION);
        let mut completed = false;
        let mut last_report = None;
        for _ in 0..120 {
            let report = cli.run(&["apply", "--journal-dir", journal.to_str().unwrap(),
                "--operation-id", OPERATION], deadline).await
                .wrap_err_with(|| format!("bounded apply failed; last native report: {last_report:?}"))?;
            if field(&report, "deployment_complete")?.as_bool() == Some(true) {
                assert_completed(&report, &operation, &trust)?; completed = true; break;
            }
            ensure!(text_field(&report, "state")? != "failed", "deployment transaction rejected: {report:?}");
            last_report = Some(report);
            sleep(Duration::from_millis(500)).await;
        }
        ensure!(completed, "bounded native apply sequence did not complete; last native report: {last_report:?}");
        let retained = retained_transactions(&operation, &network, &plan)?;
        let idle_after = drained(&network, deadline).await?;
        for command in ["apply", "status", "apply", "status"] {
            let report = cli.run(&[command, "--journal-dir", journal.to_str().unwrap(),
                "--operation-id", OPERATION], deadline).await?;
            assert_completed(&report, &operation, &trust)?;
            ensure!(retained_transactions(&operation, &network, &plan)? == retained,
                "repetition replaced or added a retained transaction/dispatch claim");
            ensure!(drained(&network, deadline).await? == idle_after,
                "repetition changed committed or rejected transaction counters");
        }
        sleep(CADENCE * 3).await;
        ensure!(drained(&network, deadline).await? == idle_after,
            "drained idle chain produced work without a transaction");
        eprintln!("clean client completed three paid deployment phases on all four peers; repeats and idle remained unchanged");
        Ok(())
    }.await;
    network.shutdown().await;
    result
}
