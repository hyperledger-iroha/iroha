//! Clean-client deployment uses the shipping CLI, three durable transactions and four peers.
//! Fixture-owned genesis/configuration establish trust before any proof response is read.
use super::*;
use iroha_config::base::read::ConfigReader;
use iroha_core::release_identity::BuildIdentity;
use iroha_crypto::{Hash, HashOf, PublicKey};
use iroha_data_model::{
    alias_setup::{AliasDataspaceBootstrapGrantV1, AliasPlanDispositionV1, AliasTransactionPlanV1},
    isi::{
        RegisterBox, SetParameter,
        staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
    },
    nexus::{
        LaneCatalog, LaneLifecycleParameterV1, LaneLifecycleStatusV1, NativeLaneManifestV1,
        NexusCatalogTransitionV1, RuntimeLaneManifestV1,
    },
    parameter::Parameter,
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};
use iroha_model_base::{peer::PeerId, topology::LaneId};
use iroha_version::codec::DecodeVersioned as _;
use norito::{codec::Encode as _, json::JsonSerialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

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

/// Serialize the same independently selected public authority for native operator workflows.
pub(super) fn write_fixture_trust(fixture: &PaidDeploymentFixture<'_>, path: &Path) -> Result<()> {
    let _profile = iroha_data_model::account::address::ChainDiscriminantGuard::enter(369);
    let owner = iroha::config::Config::load_file(fixture.config)
        .map_err(|error| eyre!("fresh owner configuration is invalid: {error:?}"))?;
    let (trust, _, _) = fixture_trust(fixture, &owner, fixture.build_identity)?;
    write_private(path, &json::to_vec(&trust)?)
}

/// Exact fresh fixture inputs supplied after native beacon installation.
pub(super) struct PaidDeploymentFixture<'a> {
    pub binary: &'a Path,
    /// Exact immutable harness identity admitted before the shared custody ceremony.
    pub build_identity: BuildIdentity,
    pub config: &'a Path,
    pub operator: &'a Path,
    pub root: &'a Path,
    pub genesis_wire: &'a [u8],
    pub genesis_public_key: &'a PublicKey,
    pub peer_configs: &'a [PathBuf],
    pub clients: &'a [iroha::client::Client],
}

/// Trust comes from freshly generated native files, never HTTP discovery.
fn fixture_trust(
    fixture: &PaidDeploymentFixture<'_>,
    owner: &iroha::config::Config,
    build: BuildIdentity,
) -> Result<(Trust, LaneCatalog, BTreeMap<PeerId, AccountId>)> {
    let (_, metadata) = iroha_core::release_identity::genesis_identity(
        fixture.genesis_wire,
        fixture.genesis_public_key,
    )?;
    let genesis = iroha_genesis::decode_signed_genesis(fixture.genesis_wire)?;
    ensure!(
        owner.chain.to_string() == CHAIN
            && owner.account_chain_discriminant == 369
            && owner.network_id == iroha_data_model::NetworkId::from_genesis_hash(genesis.hash()),
        "fixture owner must bind the exact Taira signed genesis"
    );
    let genesis_peers = iroha_genesis::signed_genesis_validator_pops(&genesis)?;
    ensure!(
        genesis_peers.len() == 4 && fixture.peer_configs.len() == 4 && fixture.clients.len() == 4,
        "fixture requires exactly four BLS validators"
    );
    let mut peers = Vec::new();
    let mut selected = std::collections::BTreeSet::new();
    let mut baseline = None;
    for (config_path, client) in fixture.peer_configs.iter().zip(fixture.clients) {
        let config = ConfigReader::new()
            .without_env()
            .read_toml_with_extends(config_path)
            .map_err(|error| eyre!("read native fixture configuration: {error:?}"))?
            .read_and_complete::<iroha_config::parameters::user::Root>()
            .map_err(|error| eyre!("decode native fixture configuration: {error:?}"))?
            .parse()
            .map_err(|error| eyre!("validate native fixture configuration: {error:?}"))?;
        let catalog = config.nexus.configured_lane_catalog.clone();
        ensure!(
            catalog.lanes().iter().all(|lane| lane.id != LaneId::new(6)),
            "fixture DPN lane must be absent before deployment"
        );
        if let Some(expected) = &baseline {
            ensure!(
                &catalog == expected,
                "fixture validators disagree on the initial catalog"
            );
        } else {
            baseline = Some(catalog);
        }
        let id = config.common.peer.id.clone();
        let context = client.to_builder();
        ensure!(
            config.common.chain == owner.chain
                && context.chain == owner.chain
                && context.network_id == owner.network_id
                && context.torii_url.as_str()
                    == format!("http://{}/", config.torii.address.value())
                && genesis_peers.iter().any(|(key, _)| key == id.public_key())
                && selected.insert(id.clone()),
            "fixture must bind each distinct signed-genesis validator to its native endpoint"
        );
        let shared = config.sumeragi.v2_config(
            Duration::from_millis(metadata.block_cadence_ms.get()),
            metadata.mode.into(),
        )?;
        peers.push(TrustPeer {
            torii_origin: context.torii_url.to_string(),
            node_fingerprint: Hash::new(id.encode()),
            peer_id: id,
            build_fingerprint: build.build_fingerprint(),
            config_fingerprint: shared.fingerprint(),
        });
    }
    let authorities = genesis_validator_authorities(
        genesis
            .external_transactions()
            .filter_map(|transaction| match transaction.instructions() {
                Executable::Instructions(instructions) => Some(instructions),
                _ => None,
            })
            .flat_map(|instructions| instructions.iter()),
        &selected,
    )?;
    Ok((
        Trust {
            genesis_public_key: fixture.genesis_public_key.clone(),
            genesis_signed_wire_hex: hex(fixture.genesis_wire),
            peers,
        },
        baseline.ok_or_else(|| eyre!("fixture catalog is absent"))?,
        authorities,
    ))
}

// Native Taira genesis binds runtime authority accounts to distinct BLS peers.
// The authenticated core-lane registration, not a key-derived account guess,
// selects the authority reused by the new restricted lane.
fn genesis_validator_authorities<'a>(
    instructions: impl Iterator<Item = &'a InstructionBox>,
    peers: &BTreeSet<PeerId>,
) -> Result<BTreeMap<PeerId, AccountId>> {
    let mut registered = BTreeSet::new();
    let mut activated = BTreeSet::new();
    let mut all_bindings = BTreeMap::new();
    let mut core_bindings = BTreeMap::new();
    for instruction in instructions {
        if let Some(RegisterBox::Account(account)) =
            instruction.as_any().downcast_ref::<RegisterBox>()
        {
            registered.insert(account.object.id.clone());
        }
        if let Some(activation) = instruction
            .as_any()
            .downcast_ref::<ActivatePublicLaneValidator>()
        {
            if activation.lane_id == LaneId::SINGLE {
                ensure!(
                    activated.insert(activation.validator.clone()),
                    "duplicate signed core-lane activation"
                );
            }
        }
        if let Some(binding) = instruction
            .as_any()
            .downcast_ref::<RegisterPublicLaneValidator>()
        {
            ensure!(
                peers.contains(&binding.peer_id),
                "signed validator binding names a peer outside the trusted roster"
            );
            if let Some(previous) =
                all_bindings.insert(binding.peer_id.clone(), binding.validator.clone())
            {
                ensure!(
                    previous == binding.validator,
                    "signed cross-lane validator authority conflicts"
                );
            }
            if binding.lane_id == LaneId::SINGLE {
                ensure!(
                    core_bindings
                        .insert(binding.peer_id.clone(), binding.validator.clone())
                        .is_none(),
                    "duplicate signed core-lane validator binding"
                );
            }
        }
    }
    let accounts = core_bindings.values().cloned().collect::<BTreeSet<_>>();
    ensure!(
        peers.len() == 4
            && core_bindings.keys().cloned().collect::<BTreeSet<_>>() == *peers
            && accounts.len() == peers.len(),
        "signed core-lane authorities must bind all four peers distinctly"
    );
    ensure!(
        accounts.iter().all(|account| registered.contains(account)) && activated == accounts,
        "signed core-lane authorities must be registered and activated accounts"
    );
    Ok(core_bindings)
}

// Independently compare native CLI output with the authenticated fixture inputs;
// this oracle never supplies a manifest to the deployment command.
fn assert_generated_lane_manifest(
    intent: &Value,
    trust: &Trust,
    authorities: &BTreeMap<PeerId, AccountId>,
) -> Result<RuntimeLaneManifestV1> {
    let runtime: RuntimeLaneManifestV1 = json::from_value(field(intent, "lane_manifest")?.clone())?;
    runtime.validate_structure()?;
    let descriptor: NativeLaneManifestV1 = json::from_str(runtime.manifest.get())?;
    ensure!(
        runtime.lane_id == LaneId::new(6)
            && descriptor.lane.as_deref() == Some("dpn")
            && descriptor.version == Some(NativeLaneManifestV1::VERSION)
            && descriptor.quorum == Some(3)
            && descriptor.governance.is_none()
            && descriptor.protected_namespaces.is_none()
            && descriptor.hooks.is_none()
            && descriptor.privacy_commitments.is_none(),
        "native init changed the requested DPN lane manifest policy"
    );
    let bindings = descriptor
        .validators
        .ok_or_else(|| eyre!("native init omitted validator bindings"))?;
    let expected = trust
        .peers
        .iter()
        .map(|peer| (&peer.peer_id, &peer.torii_origin))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        trust.peers.len() == 4
            && expected.len() == 4
            && authorities.len() == 4
            && bindings.len() == 4,
        "native init must retain all four independently selected validator bindings"
    );
    let mut observed = BTreeSet::new();
    for binding in bindings {
        let literal = binding
            .peer_id
            .as_deref()
            .ok_or_else(|| eyre!("native validator binding omitted its peer identity"))?;
        let peer: PeerId = literal.parse()?;
        let authority = authorities
            .get(&peer)
            .ok_or_else(|| eyre!("native init substituted a peer outside signed genesis"))?;
        let endpoint = expected
            .get(&peer)
            .ok_or_else(|| eyre!("native init selected a peer outside its trust profile"))?;
        ensure!(
            peer.to_string() == literal
                && observed.insert(peer)
                && binding.validator.as_deref()
                    == Some(authority.to_i105_for_discriminant(369)?.as_str())
                && binding.torii_url.as_deref() == Some(endpoint.as_str()),
            "native init changed or duplicated the signed authority/peer/endpoint binding"
        );
    }
    Ok(runtime)
}

// Only public status fields may enter failure diagnostics. Never include phase
// instructions, prepared wire, aliases, or authenticated committed payloads.
fn public_phase_summary(report: Option<&Value>) -> Vec<Value> {
    report
        .and_then(|report| report.get("verification"))
        .and_then(|verification| verification.get("transactions"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|phase| {
            let status = |name| {
                let observation = phase.get(name);
                let details = observation.and_then(|value| value.get("status"));
                let kind = details
                    .and_then(|value| value.get("kind"))
                    .and_then(Value::as_str);
                let block_height = details
                    .and_then(|value| value.get("block_height"))
                    .and_then(Value::as_u64);
                let scope = observation
                    .and_then(|value| value.get("scope"))
                    .and_then(Value::as_str);
                let resolved_from = observation
                    .and_then(|value| value.get("resolved_from"))
                    .and_then(Value::as_str);
                norito::json!({
                    "kind": kind,
                    "block_height": block_height,
                    "scope": scope,
                    "resolved_from": resolved_from
                })
            };
            let phase_name = phase.get("phase").and_then(Value::as_str);
            let state = phase.get("state").and_then(Value::as_str);
            let transaction_hash = phase.get("transaction_hash").and_then(Value::as_str);
            let global_status = status("global_status");
            let peer_status = status("peer_status");
            norito::json!({
                "phase": phase_name,
                "state": state,
                "transaction_hash": transaction_hash,
                "global_status": global_status,
                "peer_status": peer_status
            })
        })
        .collect()
}

struct Cli {
    binary: PathBuf,
    config: PathBuf,
    operator: PathBuf,
}

impl Cli {
    async fn run(&self, arguments: &[&str], deadline: Instant) -> Result<Value> {
        let started = Instant::now();
        let operation = arguments.first().copied().unwrap_or("unknown");
        eprintln!(
            "clean-client CLI {operation}: start, remaining {}ms",
            deadline.saturating_duration_since(started).as_millis()
        );
        let mut command = tokio::process::Command::new(&self.binary);
        command
            .env_clear()
            .args(["--machine", "--config"])
            .arg(&self.config)
            .arg("--operator-private-key-file")
            .arg(&self.operator)
            .args(["taira", "dataspace-deploy"])
            .args(arguments)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .kill_on_drop(true);
        if matches!(arguments.first(), Some(&"apply" | &"status")) {
            let remaining_ms = remaining_cli_budget_ms(deadline, Instant::now())?;
            command.args(["--timeout-ms", &remaining_ms.to_string()]);
        }
        let child = command
            .spawn()
            .wrap_err("failed to start native deployment CLI")?;
        let output = timeout_at(deadline, child.wait_with_output())
            .await
            .wrap_err("native dataspace deployment CLI exceeded its fixed deadline")??;
        eprintln!(
            "clean-client CLI {operation}: exited {} after {}ms, remaining {}ms",
            output.status,
            started.elapsed().as_millis(),
            deadline
                .saturating_duration_since(Instant::now())
                .as_millis()
        );
        let report = json::from_slice::<Value>(&output.stdout)
            .wrap_err("native CLI did not return its typed JSON report");
        ensure!(
            output.status.success(),
            "native CLI {operation} failed: state={:?}, verification_error={:?}, phases={:?} (stderr is retained in the fixture log)",
            report.as_ref().ok().and_then(|value| value.get("state")),
            report
                .as_ref()
                .ok()
                .and_then(|value| value.get("verification_error")),
            public_phase_summary(report.as_ref().ok())
        );
        report
    }
}

fn remaining_cli_budget_ms(deadline: Instant, now: Instant) -> Result<u64> {
    let remaining_ms = u64::try_from(deadline.saturating_duration_since(now).as_millis())?;
    ensure!(
        remaining_ms > 0,
        "native deployment deadline exhausted before child dispatch"
    );
    Ok(remaining_ms)
}

#[test]
fn remaining_cli_budget_keeps_original_deadline_and_never_rounds_up() {
    let now = Instant::now();
    let deadline = now + Duration::from_millis(180_000);
    assert_eq!(remaining_cli_budget_ms(deadline, now).unwrap(), 180_000);
    assert_eq!(
        remaining_cli_budget_ms(deadline, now + Duration::from_micros(999)).unwrap(),
        179_999
    );
    assert!(remaining_cli_budget_ms(deadline, deadline - Duration::from_micros(999)).is_err());
    assert!(remaining_cli_budget_ms(deadline, deadline).is_err());
    assert!(remaining_cli_budget_ms(deadline, deadline + Duration::from_secs(1)).is_err());
}

async fn drained(
    clients: &[iroha::client::Client],
    deadline: Instant,
) -> Result<Vec<(u64, u64, u64)>> {
    timeout_at(deadline, async {
        loop {
            let statuses = try_join_all(
                clients
                    .iter()
                    .map(|client| async move { validator_status_until(client, deadline).await }),
            )
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
    owner: &iroha::config::Config,
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
                && tx.authority() == &owner.account
                && tx.network_id() == Some(&owner.network_id)
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
        "native deployment did not complete: state={:?}, deployment_complete={:?}, verification_error={:?}, phases={:?}",
        report.get("state"),
        report.get("deployment_complete"),
        report.get("verification_error"),
        public_phase_summary(Some(report))
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

pub(super) async fn run_paid_deployment(
    fixture: PaidDeploymentFixture<'_>,
) -> Result<HashOf<TransactionEntrypoint>> {
    use std::os::unix::fs::PermissionsExt as _;
    init_instruction_registry();
    ensure!(
        fixture.binary.is_file(),
        "exact prebuilt native CLI is absent"
    );
    let owner = iroha::config::Config::load_file(fixture.config)
        .map_err(|error| eyre!("fresh owner configuration is invalid: {error:?}"))?;
    let (trust, expected_catalog, authorities) =
        fixture_trust(&fixture, &owner, fixture.build_identity)?;
    let root = fixture.root;
    fs::create_dir(root)?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    let journal = root.join("journal");
    fs::create_dir(&journal)?;
    fs::set_permissions(&journal, fs::Permissions::from_mode(0o700))?;
    let cli = Cli {
        binary: fixture.binary.to_owned(),
        config: fixture.config.to_owned(),
        operator: fixture.operator.to_owned(),
    };
    let startup = Instant::now() + Duration::from_secs(180);
    let trust_path = root.join("trust.json");
    write_private(&trust_path, &json::to_vec(&trust)?)?;
    let bundle = root.join("intent");
    let payment = iroha_config::parameters::defaults::nexus::fees::fee_asset_id();
    let idle_before = drained(fixture.clients, startup).await?;
    let deadline = Instant::now() + Duration::from_secs(180);
    let intent = cli
        .run(
            &[
                "init",
                "--dataspace",
                "dpn",
                "--lane-id",
                "6",
                "--lane-profile",
                "restricted-full-replica",
                "--account-alias",
                "admin",
                "--trust",
                trust_path.to_str().unwrap(),
                "--payment-asset",
                &payment,
                "--alias-create-maximum",
                "0.5",
                "--transaction-fee-maximum",
                "100",
                "--lease-years",
                "1",
                "--quote-lifetime-secs",
                "3600",
                "--operation-id",
                OPERATION,
                "--output-dir",
                bundle.to_str().unwrap(),
            ],
            deadline,
        )
        .await?;
    let generated_manifest = assert_generated_lane_manifest(&intent, &trust, &authorities)?;
    let deployment = bundle.join("deployment.json");
    ensure!(
        json::from_slice::<Value>(&fs::read(&deployment)?)? == intent,
        "native init retained a different deployment intent from its reported output"
    );
    let plan = cli
        .run(
            &[
                "plan",
                "--manifest",
                deployment.to_str().unwrap(),
                "--journal-dir",
                journal.to_str().unwrap(),
            ],
            deadline,
        )
        .await?;
    let baseline: LaneLifecycleStatusV1 = json::from_value(field(&plan, "baseline")?.clone())?;
    ensure!(
        baseline.validate()? == expected_catalog
            && baseline.catalog_hash == LaneLifecycleParameterV1::catalog_hash(&expected_catalog),
        "native plan baseline differs from the independently generated four-peer catalog"
    );
    let transition: NexusCatalogTransitionV1 =
        json::from_value(field(&plan, "catalog_transition")?.clone())?;
    ensure!(
        transition.manifest_additions.as_slice() == std::slice::from_ref(&generated_manifest),
        "native plan changed the independently checked generated lane manifest"
    );
    ensure!(
        drained(fixture.clients, deadline).await? == idle_before,
        "init/plan submitted a transaction"
    );
    let operation = journal.join(OPERATION);
    // Native apply owns phase observation and typed finality-progress waits.
    // A failed proof or other verification error must not trigger a new child.
    let report = cli
        .run(
            &[
                "apply",
                "--journal-dir",
                journal.to_str().unwrap(),
                "--operation-id",
                OPERATION,
            ],
            deadline,
        )
        .await
        .wrap_err("single bounded native apply failed")?;
    assert_completed(&report, &operation, &trust)?;
    let retained = retained_transactions(&operation, &owner, &plan)?;
    let idle_after = drained(fixture.clients, deadline).await?;
    for command in ["apply", "status", "apply", "status"] {
        let report = cli
            .run(
                &[
                    command,
                    "--journal-dir",
                    journal.to_str().unwrap(),
                    "--operation-id",
                    OPERATION,
                ],
                deadline,
            )
            .await?;
        assert_completed(&report, &operation, &trust)?;
        ensure!(
            retained_transactions(&operation, &owner, &plan)? == retained,
            "repetition replaced or added a retained transaction/dispatch claim"
        );
        ensure!(
            drained(fixture.clients, deadline).await? == idle_after,
            "repetition changed committed or rejected transaction counters"
        );
    }
    sleep(CADENCE * 3).await;
    ensure!(
        drained(fixture.clients, deadline).await? == idle_after,
        "drained idle chain produced work without a transaction"
    );
    eprintln!(
        "clean client completed three paid deployment phases on all four peers; repeats and idle remained unchanged"
    );
    // Return the exact catalog identity already checked against owner, network,
    // instructions, dispatch claim and all-four authenticated completion above.
    let catalog: Value = json::from_slice(&retained["catalog.prepared.json"])?;
    let wire = unhex(text_field(&catalog, "signed_transaction_wire_hex")?)?;
    Ok(SignedTransaction::decode_all_versioned(&wire)?.hash_as_entrypoint())
}

#[test]
fn signed_genesis_validator_mapping_preserves_runtime_accounts() {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::Account,
        asset::{AssetDefinitionId, AssetId},
        isi::Register,
    };
    let stake_asset_id = AssetDefinitionId::derive_from_components(
        iroha_model_base::domain::DomainId::try_new("nexus", "universal").unwrap(),
        "xor".parse().unwrap(),
    );
    let escrow_account_id = AccountId::new(
        KeyPair::from_seed(vec![25; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
    );
    let mut instructions = Vec::<InstructionBox>::new();
    let mut peers = BTreeSet::new();
    let mut expected = BTreeMap::new();
    for marker in 1_u8..=4 {
        let peer = PeerId::new(
            KeyPair::from_seed(vec![marker; 32], Algorithm::BlsNormal)
                .public_key()
                .clone(),
        );
        let account = AccountId::new(
            KeyPair::from_seed(vec![marker + 10; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        assert_ne!(account, AccountId::new(peer.public_key().clone()));
        instructions.push(Register::account(Account::new(account.clone())).into());
        for lane in [LaneId::SINGLE, LaneId::new(3)] {
            instructions.push(
                RegisterPublicLaneValidator::new(
                    lane,
                    account.clone(),
                    peer.clone(),
                    account.clone(),
                    100_u32.into(),
                    Metadata::default(),
                    iroha_data_model::nexus::PublicLaneMonetaryPlanV1::genesis_registration(
                        AssetId::new(stake_asset_id.clone(), account.clone()),
                        AssetId::new(stake_asset_id.clone(), escrow_account_id.clone()),
                        100_u32.into(),
                    ),
                )
                .into(),
            );
        }
        instructions.push(
            ActivatePublicLaneValidator {
                lane_id: LaneId::SINGLE,
                validator: account.clone(),
            }
            .into(),
        );
        peers.insert(peer.clone());
        expected.insert(peer, account);
    }
    assert_eq!(
        genesis_validator_authorities(instructions.iter(), &peers).unwrap(),
        expected
    );
    assert_generated_binding_controls(&expected);
    // Consistent bindings on another public lane are valid; every failure below
    // changes one prerequisite while keeping the remaining native bindings.
    for omitted in [0, 1, 3] {
        let mut missing = instructions.clone();
        missing.remove(omitted);
        assert!(genesis_validator_authorities(missing.iter(), &peers).is_err());
    }
    let mut duplicate = instructions.clone();
    duplicate.push(instructions[1].clone());
    assert!(genesis_validator_authorities(duplicate.iter(), &peers).is_err());
    let mut conflict = instructions.clone();
    let binding = instructions[2]
        .as_any()
        .downcast_ref::<RegisterPublicLaneValidator>()
        .unwrap();
    let mut changed = binding.clone();
    changed.validator = AccountId::new(changed.peer_id.public_key().clone());
    conflict[2] = changed.into();
    assert!(genesis_validator_authorities(conflict.iter(), &peers).is_err());
}

fn assert_generated_binding_controls(authorities: &BTreeMap<PeerId, AccountId>) {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::nexus::NativeLaneValidatorBindingV1;
    let trust = Trust {
        genesis_public_key: KeyPair::from_seed(vec![50; 32], Algorithm::Ed25519)
            .public_key()
            .clone(),
        genesis_signed_wire_hex: String::new(),
        peers: authorities
            .keys()
            .enumerate()
            .map(|(index, peer)| TrustPeer {
                torii_origin: format!("http://127.0.0.1:{}/", 8080 + index),
                peer_id: peer.clone(),
                node_fingerprint: Hash::new(peer.encode()),
                build_fingerprint: Hash::new(b"fixture build"),
                config_fingerprint: Hash::new(b"fixture config"),
            })
            .collect(),
    };
    let descriptor = NativeLaneManifestV1 {
        lane: Some("dpn".into()),
        version: Some(NativeLaneManifestV1::VERSION),
        quorum: Some(3),
        validators: Some(
            trust
                .peers
                .iter()
                .map(|peer| NativeLaneValidatorBindingV1 {
                    validator: Some(
                        authorities[&peer.peer_id]
                            .to_i105_for_discriminant(369)
                            .unwrap(),
                    ),
                    peer_id: Some(peer.peer_id.to_string()),
                    torii_url: Some(peer.torii_origin.clone()),
                })
                .collect(),
        ),
        ..NativeLaneManifestV1::default()
    };
    let intent = |descriptor: &NativeLaneManifestV1| {
        let manifest = RuntimeLaneManifestV1 {
            lane_id: LaneId::new(6),
            manifest: iroha_primitives::json::Json::try_new(descriptor).unwrap(),
        };
        let value = json::to_value(&manifest).unwrap();
        norito::json!({"lane_manifest": value})
    };
    assert_generated_lane_manifest(&intent(&descriptor), &trust, authorities).unwrap();
    for mutation in 0..5 {
        let mut changed = descriptor.clone();
        let bindings = changed.validators.as_mut().unwrap();
        match mutation {
            0 => bindings[0].validator = bindings[1].validator.clone(),
            1 => bindings[0].peer_id = bindings[1].peer_id.clone(),
            2 => bindings[0].torii_url = bindings[1].torii_url.clone(),
            3 => {
                bindings.pop();
            }
            4 => changed.quorum = Some(2),
            _ => unreachable!(),
        }
        assert!(assert_generated_lane_manifest(&intent(&changed), &trust, authorities).is_err());
    }
}

#[test]
fn phase_failure_summary_excludes_signed_payloads() {
    let report = norito::json!({"verification": {"transactions": [{
        "phase": "catalog", "state": "failed", "transaction_hash": "public-hash",
        "instructions": ["do-not-log-instructions"], "signed_transaction_wire_hex": "do-not-log-wire",
        "committed": {"transaction": "do-not-log-committed"},
        "alias_plan": "do-not-log-aliases",
        "global_status": {"scope": "global", "resolved_from": "state", "status": {"kind": "Rejected", "block_height": 10, "extra": "do-not-log-extra"}},
        "peer_status": {"scope": "local", "resolved_from": "state", "status": {"kind": "Rejected", "block_height": 10}}
    }]}});
    let summary = public_phase_summary(Some(&report));
    assert_eq!(summary.len(), 1);
    assert_eq!(
        summary[0].get("phase").and_then(Value::as_str),
        Some("catalog")
    );
    assert_eq!(
        summary[0]
            .get("global_status")
            .and_then(|v| v.get("kind"))
            .and_then(Value::as_str),
        Some("Rejected")
    );
    assert_eq!(
        summary[0]
            .get("peer_status")
            .and_then(|v| v.get("block_height"))
            .and_then(Value::as_u64),
        Some(10)
    );
    assert!(
        !String::from_utf8(json::to_vec(&summary).unwrap())
            .unwrap()
            .contains("do-not-log")
    );
    assert!(public_phase_summary(None).is_empty());
}
