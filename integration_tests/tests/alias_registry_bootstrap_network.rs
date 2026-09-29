//! Genuine four-validator NPoS alias bootstrap and retained-Kura replay.
//!
//! This isolated network uses the native SNS policy, signed planner and paid
//! leases. Ordinary transaction fees are zero from genesis to isolate lease
//! accounting; this is not production-fee qualification. Paid alias routing
//! uses the universal registry from genesis, before any private catalog entry.
//! No unchecked blocks, fabricated certificates, injected WSV or storage reset
//! may substitute for the original persisted history and Strict daemon replay.
use iroha_model_base::domain::DomainId;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::name::Name;
use iroha_model_base::peer::PeerId;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    fs,
    mem::size_of,
    num::{NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use eyre::{Result, WrapErr as _, ensure, eyre};
use futures_util::future::try_join_all;
#[path = "kura_storage_support.rs"]
mod kura_storage_support;

use integration_tests::sandbox;
use iroha::{blocking::Client, sns::SnsNamespacePath};
use iroha_config::{
    base::WithOrigin,
    kura::FsyncMode,
    parameters::{
        actual::{Kura as KuraConfig, LaneConfig as ActualLaneConfig},
        defaults,
    },
};
use iroha_core::{
    kura::{BlockIndex, BlockStore, CertifiedLaneBlockArtifact, Kura},
    lane_consensus::{
        validate_lane_block_proposal, validate_lane_block_qc, validate_lane_block_qc_aggregate,
    },
    state::derive_committee_key_id,
};
use iroha_crypto::{Algorithm, Hash, KeyPair, PublicKey};
use iroha_data_model::{
    Level,
    alias_setup::{
        ALIAS_LEASE_YEAR_MS, AliasDataSpaceIntentV1, AliasDataspaceBootstrapGrantV1,
        AliasDomainIntentV1, AliasIntentV1, AliasLeaseAcquisitionV1, AliasPlanDispositionV1,
        AliasQuoteGuardV1, AliasSetupPlanRequestV1, ResolvedDomainV1,
    },
    asset::AssetDefinitionId,
    block::{
        SignedBlock,
        consensus::{
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK, CertPhase,
            LaneBlockDescriptorV1, LaneBlockQcV1, SumeragiLanePayloadOwnership,
        },
        consensus_v2::{ConsensusMode, finality::V2FinalityArtifact},
        decode_framed_signed_block,
    },
    isi::{
        Grant,
        alias_setup::EnsureAlias,
        consensus_keys::RegisterConsensusKey,
        staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
    },
    nexus::{
        LaneCatalog, LaneConfig as ModelLaneConfig, LaneLifecycleParameterV1, LaneLifecyclePlan,
        LaneLifecycleStatusV1, LaneVisibility, PublicLaneMonetaryPreconditionV1,
        PublicLaneMonetaryScopeV1, PublicLanePreparationOperationV1,
        PublicLanePreparationRequestV1, PublicLanePrepareRegistrationV1, PublicLanePreparedPlanV1,
    },
    parameter::{Parameters, system::SumeragiNposParameters},
    prelude::*,
    sns::{NameRecordV1, NameSelectorV1, NameStatus, SuffixPolicyV1},
    transaction::{FeePaymentIntent, SignedTransaction, TransactionEntrypoint},
};
use iroha_executor_data_model::permission::peer::CanManagePeers;
use iroha_genesis::{GenesisBlock, GenesisTopologyEntry};
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
use iroha_primitives::json::Json;
use iroha_test_network::{
    NetworkBuilder, NetworkPeer, ReleasePrebuiltBinary,
    genesis_participant_committee_key_instructions, init_instruction_registry,
    resolve_release_prebuilt_binary, unexecuted_genesis_factory_with_post_topology,
};
use iroha_test_samples::{BOB_ID, BOB_KEYPAIR};
use sha2::{Digest as _, Sha256};
use tokio::time::{Instant, sleep, timeout};
use toml::{Table, Value as TomlValue};

const BPNG_ID: u64 = 8_648_377_547_929_788_715;
// Fixture-only lane identifier. It is deliberately sparse and must never be
// cited as evidence that lane 8 is operator-allocated on public Taira.
const BPNG_FIXTURE_LANE: LaneId = LaneId::new(8);
const VALIDATOR_COUNT: usize = 4;
const BPNG_MIN_QUORUM: u32 = 3;
const FIXTURE_EPOCH_LENGTH_BLOCKS: u64 = 8;
const NETWORK_SEED: &str =
    "integration-tests-alias-registry-bootstrap-retained-bpng-lane-fixture-only";
const TAIRA_XOR_ASSET_DEFINITION_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
const READ_TIMEOUT: Duration = Duration::from_secs(180);
const SUBMISSION_TIMEOUT: Duration = Duration::from_secs(300);
const SUBMISSION_TASK_TIMEOUT: Duration = Duration::from_secs(360);
const NETWORK_TIMEOUT: Duration = Duration::from_secs(360);
const CONVERGENCE_TIMEOUT: Duration = Duration::from_secs(180);
const ADVANCE_TIMEOUT: Duration = Duration::from_secs(900);
const POLL_INTERVAL: Duration = Duration::from_millis(200);
const TRANSACTION_TTL: Duration = Duration::from_secs(600);
const MAX_RETAINED_HEIGHT: u64 = 128;
const MAX_EVIDENCE_BYTES: u64 = 128 * 1024 * 1024;
const SIDECAR_INDEX_ENTRY_BYTES: usize = 16;
const SIDECAR_INDEX_HEADER_BYTES: usize = SIDECAR_INDEX_ENTRY_BYTES * 2;
const SIDECAR_INDEX_CHECK_MASK: u64 = 0x6B75_7261_2D69_6478;
const MAX_RELEASE_IDENTITY_BYTES: u64 = 32 * 1024;
const MAX_RELEASE_TOOL_OUTPUT_BYTES: usize = 64 * 1024;

fn validator_keypair(index: usize) -> KeyPair {
    KeyPair::try_from_seed(
        format!("{NETWORK_SEED}-peer-{index}").into_bytes(),
        Algorithm::Ed25519,
    )
    .expect("derive deterministic retained-BPNG-lane validator signer")
}

fn staking_custody_account() -> AccountId {
    AccountId::new(
        KeyPair::try_from_seed(
            format!("{NETWORK_SEED}-staking-custody").into_bytes(),
            Algorithm::Ed25519,
        )
        .expect("derive fixture custody identity")
        .public_key()
        .clone(),
    )
}

fn stake_asset_definition_id() -> AssetDefinitionId {
    let definition: AssetDefinitionId = defaults::nexus::staking::stake_asset_id()
        .parse()
        .expect("canonical network XOR asset");
    assert_eq!(
        definition,
        defaults::nexus::fees::fee_asset_id()
            .parse()
            .expect("canonical Nexus fee XOR asset"),
        "genesis staking and fees must use one real XOR definition"
    );
    assert_eq!(definition.to_string(), TAIRA_XOR_ASSET_DEFINITION_ID);
    definition
}

#[test]
fn retained_bpng_staking_uses_the_real_network_xor() {
    assert_eq!(
        stake_asset_definition_id(),
        SumeragiNposParameters::default().xor_asset_definition_id
    );
}

fn custom_genesis_post_topology(
    topology: &[PeerId],
    topology_entries: &[GenesisTopologyEntry],
) -> Vec<Vec<InstructionBox>> {
    assert_eq!(
        topology.len(),
        VALIDATOR_COUNT,
        "custom genesis requires exactly four BLS validator peers"
    );
    assert_eq!(
        topology_entries.len(),
        topology.len(),
        "every BPNG peer needs a proof-of-possession entry"
    );
    let stake_asset_id = stake_asset_definition_id();
    let stake = SumeragiNposParameters::default().min_self_bond().clone();
    let two_stakes = stake
        .checked_add(&stake)
        .expect("two validator self-stakes must be representable");
    let mut bootstrap: Vec<InstructionBox> = vec![
        Register::account(Account::new(staking_custody_account())).into(),
        Register::domain(Domain::new(
            DomainId::try_new("universal", "universal").expect("XOR domain"),
        ))
        .into(),
        Register::asset_definition(AssetDefinition::new(
            stake_asset_id.clone(),
            "XOR".to_owned(),
            iroha_primitives::numeric::NumericSpec::fractional(9),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .into(),
    ];
    let mut default_lane_validators: Vec<InstructionBox> = Vec::with_capacity(VALIDATOR_COUNT * 2);
    bootstrap.extend(genesis_participant_committee_key_instructions(
        topology_entries,
        topology,
    ));
    for (index, peer_id) in topology.iter().enumerate() {
        let validator = AccountId::new(validator_keypair(index).public_key().clone());
        bootstrap.push(Register::account(Account::new(validator.clone())).into());
        bootstrap.push(
            Mint::asset_quantity(
                two_stakes.clone(),
                AssetId::new(stake_asset_id.clone(), validator.clone()),
            )
            .into(),
        );
        bootstrap.push(Grant::account_permission(CanManagePeers, validator.clone()).into());
        default_lane_validators.push(
            RegisterPublicLaneValidator::new(
                LaneId::SINGLE,
                validator.clone(),
                peer_id.clone(),
                validator.clone(),
                stake.clone(),
                Metadata::default(),
                iroha_data_model::nexus::PublicLaneMonetaryPlanV1::genesis_registration(
                    AssetId::new(stake_asset_id.clone(), validator.clone()),
                    AssetId::new(stake_asset_id.clone(), staking_custody_account()),
                    stake.clone(),
                ),
            )
            .into(),
        );
        default_lane_validators
            .push(ActivatePublicLaneValidator::new(LaneId::SINGLE, validator).into());
    }
    for instruction in &default_lane_validators {
        if let Some(registration) = instruction
            .as_any()
            .downcast_ref::<RegisterPublicLaneValidator>()
        {
            assert_eq!(
                registration.monetary_plan,
                iroha_data_model::nexus::PublicLaneMonetaryPlanV1::genesis_registration(
                    AssetId::new(
                        stake_asset_definition_id(),
                        registration.stake_account.clone()
                    ),
                    AssetId::new(stake_asset_definition_id(), staking_custody_account()),
                    registration.initial_stake.clone(),
                ),
                "fixture registration must agree with its explicitly authored staking config"
            );
        }
    }
    vec![bootstrap, default_lane_validators]
}

fn bounded_client(client: Client) -> Client {
    integration_tests::sync::rebind_blocking_client(&client, |client| {
        client.transaction_status_timeout = SUBMISSION_TIMEOUT;
        client.transaction_ttl = Some(TRANSACTION_TTL);
        client.torii_request_timeout = Duration::from_secs(20);
    })
}

async fn read<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    timeout(
        READ_TIMEOUT,
        iroha_test_network::read_on_dedicated_thread(operation),
    )
    .await
    .wrap_err("bounded fixture read timed out")?
    .wrap_err("fixture read task failed")
}

async fn height(client: &Client) -> Result<u64> {
    Ok(client.client().status().get().await?.blocks)
}


async fn common_retained_prefix(clients: &[Client]) -> Result<u64> {
    let mut common = u64::MAX;
    for client in clients {
        common = common.min(height(client).await?);
    }
    ensure!(
        (1..=MAX_RETAINED_HEIGHT).contains(&common),
        "common four-peer retained prefix is missing or exceeds the fixture bound"
    );
    Ok(common)
}


async fn submit(client: &Client, transaction: SignedTransaction) -> Result<SignedTransaction> {
    timeout(
        SUBMISSION_TASK_TIMEOUT,
        client
            .account_client()
            .submit_transaction_and_wait(&transaction),
    )
    .await
    .wrap_err("native transaction did not reach terminal status in time")?
    .wrap_err("native submission failed")?;
    Ok(transaction)
}

fn bpng_fixture_lane() -> ModelLaneConfig {
    ModelLaneConfig {
        id: BPNG_FIXTURE_LANE,
        dataspace_id: DataSpaceId::new(BPNG_ID),
        alias: "bpng-fixture-only-lane-8".to_owned(),
        description: Some(
            "integration fixture only; not a public-Taira lane allocation".to_owned(),
        ),
        visibility: LaneVisibility::Public,
        ..ModelLaneConfig::default()
    }
}

fn dataspace_only_restart_layer(grant: &AliasDataspaceBootstrapGrantV1) -> Table {
    let universal = Table::from_iter([
        (
            "alias".to_owned(),
            TomlValue::String("universal".to_owned()),
        ),
        ("id".to_owned(), TomlValue::Integer(0)),
        ("fault_tolerance".to_owned(), TomlValue::Integer(1)),
    ]);
    let bpng = Table::from_iter([
        ("alias".to_owned(), TomlValue::String("bpng".to_owned())),
        (
            "manifest_hash".to_owned(),
            TomlValue::String(hex::encode(grant.name_hash)),
        ),
        (
            "id".to_owned(),
            TomlValue::Integer(i64::try_from(BPNG_ID).expect("BPNG id fits i64")),
        ),
        ("fault_tolerance".to_owned(), TomlValue::Integer(1)),
    ]);
    let nexus = Table::from_iter([(
        "dataspace_catalog".to_owned(),
        TomlValue::Array(vec![TomlValue::Table(universal), TomlValue::Table(bpng)]),
    )]);
    debug_assert!(
        nexus.len() == 1 && nexus.contains_key("dataspace_catalog"),
        "restart layer must add only the static BPNG dataspace"
    );
    Table::from_iter([("nexus".to_owned(), TomlValue::Table(nexus))])
}


#[derive(Debug)]
struct ValidatorLifecycleSnapshot {
    total: u64,
    registered: BTreeSet<(AccountId, PeerId)>,
    pending: BTreeMap<(AccountId, PeerId), u64>,
    active: BTreeSet<(AccountId, PeerId)>,
}

fn validator_bindings(
    snapshot: &norito::json::Value,
    expected_stake: &Quantity,
) -> Result<ValidatorLifecycleSnapshot> {
    let root = snapshot
        .as_object()
        .ok_or_else(|| eyre!("lane validator response is not an object"))?;
    let total = root
        .get("total")
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| eyre!("lane validator response omitted total"))?;
    let items = root
        .get("items")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("lane validator response omitted items"))?;
    let mut registered = BTreeSet::new();
    let mut pending = BTreeMap::new();
    let mut active = BTreeSet::new();
    let expected_stake = expected_stake.to_string();
    for item in items {
        let item = item
            .as_object()
            .ok_or_else(|| eyre!("lane validator item is not an object"))?;
        ensure!(
            item.get("lane_id").and_then(norito::json::Value::as_u64)
                == Some(u64::from(BPNG_FIXTURE_LANE))
                && item
                    .get("authority_source")
                    .and_then(norito::json::Value::as_str)
                    == Some("staking")
                && item
                    .get("deactivation_height")
                    .is_some_and(norito::json::Value::is_null)
                && item
                    .get("last_reward_epoch")
                    .is_some_and(norito::json::Value::is_null),
            "BPNG validator record route, authority source or open tenure changed"
        );
        let activation_height = item
            .get("activation_height")
            .and_then(norito::json::Value::as_u64)
            .ok_or_else(|| eyre!("lane validator item omitted activation_height"))?;
        ensure!(
            activation_height > 0,
            "BPNG validator activation boundary must be non-zero"
        );
        let status = item
            .get("status")
            .and_then(norito::json::Value::as_object)
            .ok_or_else(|| eyre!("lane validator item omitted status"))?;
        let status_type = status
            .get("type")
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("lane validator item omitted status.type"))?;
        let validator = AccountId::parse_encoded(
            item.get("validator")
                .and_then(norito::json::Value::as_str)
                .ok_or_else(|| eyre!("lane validator item omitted validator"))?,
        )?;
        let peer = item
            .get("peer_id")
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("lane validator item omitted peer_id"))?
            .parse()?;
        let stake_account = AccountId::parse_encoded(
            item.get("stake_account")
                .and_then(norito::json::Value::as_str)
                .ok_or_else(|| eyre!("lane validator item omitted stake_account"))?,
        )?;
        ensure!(
            stake_account == validator
                && item
                    .get("total_stake")
                    .and_then(norito::json::Value::as_str)
                    == Some(expected_stake.as_str())
                && item.get("self_stake").and_then(norito::json::Value::as_str)
                    == Some(expected_stake.as_str()),
            "BPNG validator record does not retain its exact validator-owned minimum stake"
        );
        let binding = (validator, peer);
        ensure!(
            registered.insert(binding.clone()),
            "BPNG validator response contains a duplicate validator/peer binding"
        );
        match status_type {
            "PendingActivation" => {
                ensure!(
                    status.len() == 2
                        && status
                            .get("activates_at_height")
                            .and_then(norito::json::Value::as_u64)
                            == Some(activation_height),
                    "BPNG pending status does not bind its exact activation boundary"
                );
                pending.insert(binding, activation_height);
            }
            "Active" => {
                ensure!(
                    status.len() == 1,
                    "BPNG active status contains unexpected lifecycle fields"
                );
                active.insert(binding);
            }
            other => return Err(eyre!("unexpected BPNG validator lifecycle status {other}")),
        }
    }
    ensure!(
        usize::try_from(total)? == items.len(),
        "lane validator total does not match exact returned records"
    );
    ensure!(
        usize::try_from(total)? == registered.len()
            && pending.len().saturating_add(active.len()) == registered.len(),
        "lane validator response does not classify every exact unique binding"
    );
    Ok(ValidatorLifecycleSnapshot {
        total,
        registered,
        pending,
        active,
    })
}

async fn wait_for_exact_bpng_pending_registrations(
    client: &Client,
    expected: &BTreeSet<(AccountId, PeerId)>,
    expected_stake: &Quantity,
) -> Result<u64> {
    ensure!(
        expected.len() == VALIDATOR_COUNT,
        "pending BPNG qualification requires exactly four expected validator bindings"
    );
    let deadline = Instant::now() + CONVERGENCE_TIMEOUT;
    loop {
        let query_client = client.clone();
        let expected_stake = expected_stake.clone();
        let observed = read(move || {
            let snapshot = query_client
                .client()
                .get_public_lane_validators(BPNG_FIXTURE_LANE)?;
            validator_bindings(&snapshot, &expected_stake)
        })
        .await;
        if let Ok(snapshot) = &observed
            && snapshot.total == u64::try_from(expected.len())?
            && snapshot.registered == *expected
            && snapshot.pending.len() == expected.len()
            && snapshot.active.is_empty()
        {
            let boundaries = snapshot.pending.values().copied().collect::<BTreeSet<_>>();
            ensure!(
                boundaries.len() == 1,
                "four BPNG pending registrations must share one exact election boundary: {boundaries:?}"
            );
            return boundaries
                .first()
                .copied()
                .ok_or_else(|| eyre!("BPNG pending activation boundary is missing"));
        }
        ensure!(
            Instant::now() < deadline,
            "exact BPNG pending registrations did not converge: {observed:?}"
        );
        sleep(POLL_INTERVAL).await;
    }
}

async fn wait_for_exact_bpng_validators(
    clients: &[Client],
    expected: &BTreeSet<(AccountId, PeerId)>,
    expected_stake: &Quantity,
) -> Result<()> {
    ensure!(
        clients.len() == VALIDATOR_COUNT && expected.len() == VALIDATOR_COUNT,
        "active BPNG qualification requires exactly four peers and validator bindings"
    );
    let deadline = Instant::now() + CONVERGENCE_TIMEOUT;
    loop {
        let mut matched = true;
        let mut last = Vec::new();
        for client in clients {
            let client = client.clone();
            let expected_stake = expected_stake.clone();
            match read(move || {
                let snapshot = client
                    .client()
                    .get_public_lane_validators(BPNG_FIXTURE_LANE)?;
                validator_bindings(&snapshot, &expected_stake)
            })
            .await
            {
                Ok(snapshot) => {
                    matched &= snapshot.total == u64::try_from(expected.len())?
                        && snapshot.active == *expected
                        && snapshot.registered == *expected
                        && snapshot.pending.is_empty();
                    last.push(snapshot);
                }
                Err(error) => {
                    matched = false;
                    if Instant::now() >= deadline {
                        return Err(error).wrap_err("BPNG validator status did not converge");
                    }
                }
            }
        }
        if matched {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "exact BPNG validator bindings did not converge: {last:?}"
        );
        sleep(POLL_INTERVAL).await;
    }
}

fn transaction(client: &Client, instruction: impl Into<InstructionBox>) -> SignedTransaction {
    {
        let account = client.account_client();
        account
            .prepare_transaction(iroha::client::AccountTransactionDraft::new(
                [instruction.into()],
                FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            ))
            .and_then(|payload| account.sign_transaction(payload))
    }
    .expect("build integration-test transaction")
}

#[derive(Debug, PartialEq, Eq)]
struct ReleaseSourceIdentity {
    head_commit: String,
    head_tree: String,
    source_manifest_sha256: String,
    cargo_lock_sha256: String,
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn required_exact_env(name: &str) -> Result<String> {
    let value = std::env::var(name).wrap_err_with(|| format!("missing required {name}"))?;
    ensure!(
        !value.is_empty() && value.trim() == value,
        "{name} must be one non-empty canonical value"
    );
    Ok(value)
}

fn required_canonical_path(name: &str, directory: bool) -> Result<PathBuf> {
    let path = PathBuf::from(required_exact_env(name)?);
    ensure!(path.is_absolute(), "{name} must be absolute");
    let canonical = path
        .canonicalize()
        .wrap_err_with(|| format!("{name} is unavailable: {}", path.display()))?;
    ensure!(canonical == path, "{name} must already be canonical");
    let metadata = fs::symlink_metadata(&path)?;
    ensure!(
        !metadata.file_type().is_symlink()
            && if directory {
                metadata.is_dir()
            } else {
                metadata.is_file()
            },
        "{name} must be a real {}",
        if directory {
            "directory"
        } else {
            "regular file"
        }
    );
    Ok(path)
}

fn bounded_tool_output(mut command: Command, label: &str) -> Result<Vec<u8>> {
    command.stdin(Stdio::null());
    let output = command
        .output()
        .wrap_err_with(|| format!("failed to execute {label}"))?;
    ensure!(output.status.success(), "{label} failed");
    ensure!(
        output.stdout.len() <= MAX_RELEASE_TOOL_OUTPUT_BYTES
            && output.stderr.len() <= MAX_RELEASE_TOOL_OUTPUT_BYTES,
        "{label} exceeded its fixed output budget"
    );
    Ok(output.stdout)
}

fn parse_release_source_identity(bytes: &[u8]) -> Result<ReleaseSourceIdentity> {
    ensure!(
        !bytes.is_empty() && u64::try_from(bytes.len())? <= MAX_RELEASE_IDENTITY_BYTES,
        "release source identity is empty or oversized"
    );
    let value: norito::json::Value = norito::json::from_slice(bytes)?;
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("release source identity must be a JSON object"))?;
    let expected_fields = BTreeSet::from([
        "schema_version",
        "head_commit",
        "head_tree",
        "index_tree",
        "workspace_source_manifest_sha256",
        "cargo_lock_sha256",
    ]);
    ensure!(
        object.keys().map(String::as_str).collect::<BTreeSet<_>>() == expected_fields,
        "release source identity has an open or incomplete field set"
    );
    ensure!(
        object
            .get("schema_version")
            .and_then(norito::json::Value::as_u64)
            == Some(1),
        "release source identity schema must be exactly 1"
    );
    let string = |field: &str| -> Result<String> {
        Ok(object
            .get(field)
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("release source identity omitted {field}"))?
            .to_owned())
    };
    let identity = ReleaseSourceIdentity {
        head_commit: string("head_commit")?,
        head_tree: string("head_tree")?,
        source_manifest_sha256: string("workspace_source_manifest_sha256")?,
        cargo_lock_sha256: string("cargo_lock_sha256")?,
    };
    ensure!(
        string("index_tree")? == identity.head_tree,
        "release source index tree is not exact HEAD"
    );
    for (value, label) in [
        (
            &identity.source_manifest_sha256,
            "workspace source manifest",
        ),
        (&identity.cargo_lock_sha256, "Cargo.lock"),
    ] {
        ensure!(
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "release {label} digest is not lowercase SHA-256"
        );
    }
    Ok(identity)
}

fn read_release_identity(path: &Path) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path)?;
    ensure!(
        before.is_file()
            && !before.file_type().is_symlink()
            && before.len() > 0
            && before.len() <= MAX_RELEASE_IDENTITY_BYTES,
        "sealed release identity must be one bounded regular file"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        ensure!(
            before.mode() & 0o7777 == 0o400 && before.nlink() == 1,
            "sealed release identity must have exact mode 0400 and one link"
        );
    }
    let bytes = fs::read(path)?;
    let after = fs::symlink_metadata(path)?;
    ensure!(
        before.len() == u64::try_from(bytes.len())?
            && before.modified()? == after.modified()?
            && before.len() == after.len(),
        "sealed release identity changed while it was read"
    );
    Ok(bytes)
}

fn verify_clean_release_source(repo_root: &Path) -> Result<ReleaseSourceIdentity> {
    ensure!(
        required_exact_env("IROHA_RELEASE_SEALED_WORKTREE")? == "1",
        "native BPNG qualification requires the sealed production release worktree"
    );
    let invocation_root = required_canonical_path("IROHA_RELEASE_INVOCATION_ROOT", true)?;
    let sealed_root = required_canonical_path("IROHA_RELEASE_SEALED_ROOT", true)?;
    ensure!(
        sealed_root == repo_root && invocation_root.join("source") == sealed_root,
        "compiled BPNG scenario escaped the exact sealed release source root"
    );
    let identity_path = required_canonical_path("IROHA_RELEASE_EXPECTED_IDENTITY_PATH", false)?;
    ensure!(
        identity_path == invocation_root.join("sealed-identity.json"),
        "release identity escaped its fixed invocation path"
    );
    let python = required_canonical_path("IROHA_RELEASE_PYTHON_BIN", false)?;
    let git = required_canonical_path("IROHA_RELEASE_GIT_BIN", false)?;
    let path = required_exact_env("PATH")?;
    let path_entries = std::env::split_paths(&path).collect::<Vec<_>>();
    ensure!(
        path_entries.len() == 1
            && path_entries[0].join("git").canonicalize()? == git
            && path_entries[0].join("python3").canonicalize()? == python,
        "release PATH must resolve only the pinned Git and Python runtime"
    );

    let mut seal = Command::new(&python);
    seal.args(["-I", "-S"])
        .arg(repo_root.join("scripts/seal_workspace_source.py"))
        .arg("--verify")
        .arg("--root")
        .arg(repo_root)
        .arg("--no-writable-paths")
        .current_dir(repo_root);
    ensure!(
        bounded_tool_output(seal, "sealed workspace verifier")?.is_empty(),
        "sealed workspace verifier emitted unexpected output"
    );

    let mut capture = Command::new(&python);
    capture
        .args(["-I", "-S"])
        .arg(repo_root.join("scripts/compute_workspace_source_manifest.py"))
        .arg("--root")
        .arg(repo_root)
        .arg("--release-identity-json")
        .current_dir(repo_root);
    let observed = parse_release_source_identity(&bounded_tool_output(
        capture,
        "clean release source identity capture",
    )?)?;
    let retained = parse_release_source_identity(&read_release_identity(&identity_path)?)?;
    ensure!(
        observed == retained,
        "sealed identity does not reproduce from the exact clean HEAD/test source tree"
    );
    ensure!(
        observed.head_commit == required_exact_env("IROHA_RELEASE_HEAD_COMMIT")?
            && observed.head_tree == required_exact_env("IROHA_RELEASE_HEAD_TREE")?
            && observed.source_manifest_sha256
                == required_exact_env("IROHA_RELEASE_SOURCE_MANIFEST_SHA256")?
            && observed.cargo_lock_sha256 == required_exact_env("IROHA_RELEASE_CARGO_LOCK_SHA256")?,
        "release identity exports disagree with clean HEAD/tree/Cargo.lock/source manifest"
    );
    Ok(observed)
}

fn required_prebuilt_binaries() -> Result<()> {
    ensure!(
        std::env::var("IROHA_TEST_SKIP_BUILD").as_deref() == Ok("1"),
        "native BPNG qualification is lookup-only and must not launch child Cargo builds"
    );
    ensure!(
        required_exact_env("IROHA_TEST_BUILD_PROFILE")? == "release"
            && required_exact_env("PROFILE")? == "release",
        "native BPNG qualification requires the exact release build profile"
    );
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()?;
    let source_identity = verify_clean_release_source(&repo_root)?;
    let manifest_source = required_exact_env("IROHA_RELEASE_SOURCE_MANIFEST_SHA256")?;
    ensure!(
        source_identity.source_manifest_sha256 == manifest_source,
        "prebuilt bundle source anchor differs from the clean qualification source"
    );
    for (name, kind) in [
        ("TEST_NETWORK_BIN_IROHAD", ReleasePrebuiltBinary::Irohad),
        (
            "TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL",
            ReleasePrebuiltBinary::IrohadMessageControl,
        ),
        ("TEST_NETWORK_BIN_IROHA", ReleasePrebuiltBinary::Iroha),
        ("KAGAMI_BIN", ReleasePrebuiltBinary::Kagami),
    ] {
        let path = PathBuf::from(required_exact_env(name)?);
        ensure!(
            path.is_absolute(),
            "{name} must be an absolute manifest-bound path"
        );
        let resolved = resolve_release_prebuilt_binary(kind)?
            .ok_or_else(|| eyre!("source-bound release prebuilt contract is not active"))?;
        ensure!(
            path == resolved,
            "{name} is not the exact canonical SHA-256/size/mode/profile/target/toolchain-bound release binary"
        );
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LeaseExpectation {
    namespace: SnsNamespacePath,
    literal: String,
    policy: SuffixPolicyV1,
    amount: Quantity,
}

async fn acquire(
    payer: &Client,
    intent: AliasIntentV1,
    namespace: SnsNamespacePath,
    literal: &str,
) -> Result<(SignedTransaction, LeaseExpectation)> {
    let planning_client = payer.clone();
    let literal = literal.to_owned();
    let (signed, expectation) = read(move || {
        let policy = planning_client
            .client()
            .sns()
            .get_policy(namespace.suffix_id())?;
        ensure!(
            policy.fund_splitter_account != *BOB_ID,
            "fixture payer must differ from native lease collector"
        );
        let payment_asset = AssetDefinitionId::parse_address_literal(&policy.payment_asset_id)?;
        let valid_until_ms =
            u64::try_from(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())?
                .checked_add(u64::try_from(TRANSACTION_TTL.as_millis())?)
                .ok_or_else(|| eyre!("lease deadline overflow"))?;
        let request = AliasSetupPlanRequestV1::new(vec![EnsureAlias::new(
            intent,
            AliasLeaseAcquisitionV1::new(1, None),
            AliasQuoteGuardV1 {
                expected_policy_version: policy.policy_version,
                expected_payment_asset: payment_asset,
                max_amount: Quantity::from(100_u32),
                valid_until_ms,
            },
        )]);
        let plan = planning_client.client().plan_alias_setup(&request)?;
        ensure!(
            plan.body.blockers.is_empty() && plan.body.resources.len() == 1,
            "native planner did not produce one executable resource"
        );
        let resource = &plan.body.resources[0];
        ensure!(
            resource.disposition == AliasPlanDispositionV1::Create,
            "lease must be a first native acquisition, not repair/no-op"
        );
        let quote = resource
            .quote
            .as_ref()
            .ok_or_else(|| eyre!("native create plan omitted its paid quote"))?;
        ensure!(
            !quote.exact_amount.is_zero(),
            "SNS acquisition must charge a real lease payment"
        );
        let instructions = planning_client
            .client()
            .verify_alias_setup_plan_for_request(&request, &plan)?;
        ensure!(
            instructions.len() == 1,
            "one native EnsureAlias instruction expected"
        );
        let signed = {
            let account = planning_client.account_client();
            account
                .prepare_transaction(iroha::client::AccountTransactionDraft::new(
                    instructions,
                    FeePaymentIntent::authority(Vec::new(), None),
                    Metadata::default(),
                ))
                .and_then(|payload| account.sign_transaction(payload))
        }
        .expect("build integration-test transaction");
        Ok((
            signed,
            LeaseExpectation {
                namespace,
                literal,
                policy,
                amount: quote.exact_amount.clone(),
            },
        ))
    })
    .await?;
    Ok((submit(payer, signed).await?, expectation))
}

fn balances(client: &Client, leases: &[LeaseExpectation]) -> Result<BTreeMap<AssetId, Quantity>> {
    let mut selected = BTreeMap::new();
    for lease in leases {
        let definition = AssetDefinitionId::parse_address_literal(&lease.policy.payment_asset_id)?;
        for owner in [BOB_ID.clone(), lease.policy.fund_splitter_account.clone()] {
            selected.insert(AssetId::of(definition.clone(), owner), Quantity::zero());
        }
    }
    for asset in client.client().query(FindAssets::new()).execute_all()? {
        if let Some(amount) = selected.get_mut(asset.id()) {
            *amount = asset.value().clone();
        }
    }
    Ok(selected)
}

fn assert_paid_once(
    before: &BTreeMap<AssetId, Quantity>,
    after: &BTreeMap<AssetId, Quantity>,
    leases: &[LeaseExpectation],
) -> Result<()> {
    let mut expected = before.clone();
    for lease in leases {
        let asset = AssetDefinitionId::parse_address_literal(&lease.policy.payment_asset_id)?;
        let payer = expected
            .get_mut(&AssetId::of(asset.clone(), BOB_ID.clone()))
            .ok_or_else(|| eyre!("missing baseline payer balance"))?;
        *payer = payer.checked_sub(&lease.amount)?;
        let collector = expected
            .get_mut(&AssetId::of(
                asset,
                lease.policy.fund_splitter_account.clone(),
            ))
            .ok_or_else(|| eyre!("missing baseline collector balance"))?;
        *collector = collector.checked_add(&lease.amount)?;
    }
    ensure!(
        &expected == after,
        "native lease payer/collector balances differ from exact once-only quoted charges"
    );
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LedgerSnapshot {
    parameters: Vec<u8>,
    leases: Vec<NameRecordV1>,
    domains: BTreeMap<DomainId, Vec<u8>>,
    balances: BTreeMap<AssetId, Quantity>,
}

fn ledger_snapshot(
    client: &Client,
    expectations: &[LeaseExpectation],
    grant: &AliasDataspaceBootstrapGrantV1,
    transactions: &[SignedTransaction],
) -> Result<LedgerSnapshot> {
    let parameters: Parameters = client.client().query_single(FindParameters::new())?;
    let custom = parameters
        .custom()
        .get(&grant.parameter_id()?)
        .ok_or_else(|| eyre!("owner bootstrap grant missing"))?;
    ensure!(
        AliasDataspaceBootstrapGrantV1::from_custom_parameter(custom)?.as_ref() == Some(grant),
        "owner bootstrap grant changed"
    );
    let mut leases = Vec::new();
    for expected in expectations {
        ensure!(
            client
                .client()
                .sns()
                .get_policy(expected.namespace.suffix_id())?
                == expected.policy,
            "native suffix policy drifted"
        );
        let record = client
            .client()
            .sns()
            .get_name(expected.namespace, &expected.literal)?;
        let selector = NameSelectorV1::new(expected.namespace.suffix_id(), &expected.literal)?;
        ensure!(
            record.selector == selector && record.name_hash == selector.name_hash(),
            "native lease selector/hash mismatch"
        );
        ensure!(
            record.owner == *BOB_ID
                && record.ownership_generation == 1
                && record.status == NameStatus::Active,
            "lease owner/generation/status mismatch"
        );
        ensure!(
            record.expires_at_ms.checked_sub(record.registered_at_ms) == Some(ALIAS_LEASE_YEAR_MS),
            "lease term is not exactly one year"
        );
        ensure!(
            record.grace_expires_at_ms.checked_sub(record.expires_at_ms)
                == Some(u64::from(expected.policy.grace_period_days) * 86_400_000),
            "lease grace term mismatch"
        );
        ensure!(
            record
                .redemption_expires_at_ms
                .checked_sub(record.grace_expires_at_ms)
                == Some(u64::from(expected.policy.redemption_period_days) * 86_400_000),
            "lease redemption term mismatch"
        );
        leases.push(record);
    }
    let mut domains = BTreeMap::new();
    for domain in client.client().query(FindDomains::new()).execute_all()? {
        if [
            DomainId::try_new("history", "universal")?,
            DomainId::try_new("mibank", "bpng")?,
        ]
        .contains(domain.id())
        {
            ensure!(
                domain.owned_by() == &*BOB_ID,
                "native domain owner mismatch"
            );
            domains.insert(domain.id().clone(), domain.encode());
        }
    }
    ensure!(domains.len() == 2, "both native domains must be present");
    let committed = client
        .client()
        .query(FindTransactions::new())
        .execute_all()?;
    for transaction in transactions {
        let matching = committed
            .iter()
            .filter(|record| record.entrypoint_hash() == &transaction.hash_as_entrypoint())
            .collect::<Vec<_>>();
        ensure!(
            matching.len() == 1,
            "exact signed transaction must appear once in committed query history"
        );
        ensure!(
            matching[0].entrypoint() == &TransactionEntrypoint::External(transaction.clone())
                && matching[0].result().0.is_ok(),
            "committed transaction bytes/result mismatch"
        );
    }
    Ok(LedgerSnapshot {
        parameters: parameters.encode(),
        leases,
        domains,
        balances: balances(client, expectations)?,
    })
}

async fn wait_for_snapshot(
    client: &Client,
    expectations: &[LeaseExpectation],
    grant: &AliasDataspaceBootstrapGrantV1,
    transactions: &[SignedTransaction],
    expected: Option<&LedgerSnapshot>,
) -> Result<LedgerSnapshot> {
    let deadline = Instant::now() + CONVERGENCE_TIMEOUT;
    loop {
        let client = client.clone();
        let expectations = expectations.to_vec();
        let grant = grant.clone();
        let transactions = transactions.to_vec();
        let observed =
            read(move || ledger_snapshot(&client, &expectations, &grant, &transactions)).await;
        match observed {
            Ok(snapshot) if expected.is_none_or(|expected| expected == &snapshot) => {
                return Ok(snapshot);
            }
            outcome => {
                ensure!(
                    Instant::now() < deadline,
                    "exact ledger state failed to converge: {outcome:?}"
                );
                sleep(POLL_INTERVAL).await;
            }
        }
    }
}



async fn assert_bpng_metadata(
    client: &Client,
    predecessor_key: &Name,
    predecessor_value: &Json,
    successor: Option<(&Name, &Json)>,
) -> Result<()> {
    let client = client.clone();
    let domain = read(move || {
        Ok(client
            .client()
            .query_single(FindDomainById::new(DomainId::try_new("mibank", "bpng")?))?)
    })
    .await?;
    ensure!(
        domain.metadata().get(predecessor_key) == Some(predecessor_value),
        "pre-restart BPNG state is absent"
    );
    if let Some((key, value)) = successor {
        ensure!(
            domain.metadata().get(key) == Some(value),
            "post-restart BPNG successor state is absent"
        );
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct RetainedHistory {
    blocks: Vec<Vec<u8>>,
    sidecars: BTreeMap<PathBuf, Vec<u8>>,
}

#[derive(Debug, PartialEq, Eq)]
struct StoppedEvidence {
    retained: RetainedHistory,
    certified_bpng_lane: CertifiedBpngLaneEvidence,
}

#[derive(Debug, PartialEq, Eq)]
struct CertifiedBpngLaneEvidence {
    artifacts: Vec<Vec<u8>>,
    data: Vec<u8>,
    data_sha256: String,
    index: Vec<u8>,
    index_sha256: String,
}

impl CertifiedBpngLaneEvidence {
    fn absent() -> Self {
        Self {
            artifacts: Vec::new(),
            data: Vec::new(),
            data_sha256: sha256_hex(&[]),
            index: Vec::new(),
            index_sha256: sha256_hex(&[]),
        }
    }

    fn new(artifacts: Vec<Vec<u8>>, data: Vec<u8>, index: Vec<u8>) -> Self {
        Self {
            artifacts,
            data_sha256: sha256_hex(&data),
            data,
            index_sha256: sha256_hex(&index),
            index,
        }
    }

    fn assert_exact_prefix_of(&self, successor: &Self) -> Result<()> {
        ensure!(
            sha256_hex(&self.data) == self.data_sha256
                && sha256_hex(&self.index) == self.index_sha256
                && sha256_hex(&successor.data) == successor.data_sha256
                && sha256_hex(&successor.index) == successor.index_sha256,
            "retained BPNG sidecar evidence digest does not match its exact bytes"
        );
        ensure!(
            successor.artifacts.starts_with(&self.artifacts)
                && successor.data.starts_with(&self.data)
                && successor.index.starts_with(&self.index)
                && sha256_hex(&successor.data[..self.data.len()]) == self.data_sha256
                && sha256_hex(&successor.index[..self.index.len()]) == self.index_sha256,
            "strict replay changed the raw certified BPNG data/index byte prefix or its SHA-256"
        );
        Ok(())
    }
}


fn bitmap_signer_count(bitmap: &[u8]) -> u32 {
    bitmap.iter().map(|byte| byte.count_ones()).sum()
}

fn bpng_certified_sidecar_paths(
    store_root: &Path,
    network_id: NetworkId,
    expected_incarnation: Option<Hash>,
) -> Result<Option<(PathBuf, PathBuf)>> {
    let Some(blocks) = kura_storage_support::lane_instance_blocks_dir(
        store_root,
        network_id,
        BPNG_FIXTURE_LANE,
        DataSpaceId::new(BPNG_ID),
        expected_incarnation,
        None,
    )?
    else {
        return Ok(None);
    };
    let directory = blocks.join("lane_artifacts");
    Ok(Some((
        directory.join("certified_blocks.norito"),
        directory.join("certified_blocks.index"),
    )))
}

fn read_optional_evidence_file(path: &Path) -> Result<Option<Vec<u8>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        metadata.is_file() && metadata.len() <= MAX_EVIDENCE_BYTES,
        "retained BPNG evidence must be a bounded regular file: {}",
        path.display()
    );
    let bytes = fs::read(path)?;
    ensure!(
        u64::try_from(bytes.len())? == metadata.len(),
        "retained BPNG evidence changed while being read: {}",
        path.display()
    );
    Ok(Some(bytes))
}

fn sidecar_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    let end = offset
        .checked_add(size_of::<u64>())
        .ok_or_else(|| eyre!("sidecar index offset overflow"))?;
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or_else(|| eyre!("truncated retained BPNG sidecar index"))?
            .try_into()?,
    ))
}

fn lane_qc_signer_keys(qc: &LaneBlockQcV1) -> Result<BTreeSet<PublicKey>> {
    validate_lane_block_qc(qc)?;
    let mut signers = BTreeSet::new();
    for (byte_index, byte) in qc.signers_bitmap.iter().copied().enumerate() {
        for bit in 0..8 {
            if byte & (1_u8 << bit) == 0 {
                continue;
            }
            let signer_index = byte_index
                .checked_mul(8)
                .and_then(|base| base.checked_add(bit))
                .ok_or_else(|| eyre!("lane QC signer index overflow"))?;
            signers.insert(
                qc.validator_set
                    .get(signer_index)
                    .ok_or_else(|| eyre!("lane QC signer bitmap exceeds validator set"))?
                    .public_key()
                    .clone(),
            );
        }
    }
    Ok(signers)
}

fn validate_certified_bpng_artifact(artifact: &CertifiedLaneBlockArtifact) -> Result<()> {
    artifact.encode_framed()?;
    validate_lane_block_proposal(&artifact.proposal)?;
    validate_lane_block_qc(&artifact.prepare_qc)?;
    validate_lane_block_qc(&artifact.commit_qc)?;
    let descriptor = &artifact.proposal.descriptor;
    ensure!(
        artifact.prepare_qc.body == artifact.proposal.vote_body(CertPhase::Prepare)
            && artifact.commit_qc.body == artifact.proposal.vote_body(CertPhase::Commit),
        "retained BPNG QCs do not certify their exact proposal"
    );
    for qc in [&artifact.prepare_qc, &artifact.commit_qc] {
        ensure!(
            qc.validator_set_hash_version == descriptor.validator_set_hash_version
                && qc.validator_set_hash == descriptor.validator_set_hash
                && qc.validator_set == descriptor.validator_set,
            "retained BPNG QC committee does not match its descriptor"
        );
    }
    let mut expected_pops = lane_qc_signer_keys(&artifact.prepare_qc)?;
    expected_pops.extend(lane_qc_signer_keys(&artifact.commit_qc)?);
    ensure!(
        artifact
            .signer_pops
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>()
            == expected_pops,
        "retained BPNG artifact PoPs do not exactly cover its QC signers"
    );
    validate_lane_block_qc_aggregate(&artifact.prepare_qc, &artifact.signer_pops)?;
    validate_lane_block_qc_aggregate(&artifact.commit_qc, &artifact.signer_pops)?;
    Ok(())
}






fn execution_height(history: &RetainedHistory, transaction: &SignedTransaction) -> Result<u64> {
    let expected = TransactionEntrypoint::External(transaction.clone());
    let mut found = None;
    for wire in &history.blocks {
        let block = decode_framed_signed_block(wire)?;
        for (input_index, entrypoint) in block.network_entrypoints().enumerate() {
            if entrypoint == &expected {
                let (_, output) = block
                    .network_output_at(u32::try_from(input_index)?)
                    .ok_or_else(|| eyre!("retained transaction omitted its Network output"))?;
                ensure!(
                    found.is_none() && output.result.0.is_ok(),
                    "signed transaction must have one successful retained execution"
                );
                let context = block
                    .execution_context()
                    .ok_or_else(|| eyre!("committed execution plan missing"))?;
                let routes = context
                    .external
                    .iter()
                    .filter(|route| route.entrypoint_hash == transaction.hash_as_entrypoint())
                    .collect::<Vec<_>>();
                ensure!(
                    routes.len() == 1,
                    "exact entrypoint execution route missing/duplicated"
                );
                let route = routes[0];
                ensure!(
                    route.lane_id == LaneId::SINGLE && route.dataspace_id == DataSpaceId::UNIVERSAL,
                    "alias/control escaped original universal lane"
                );
                ensure!(
                    route.routing_plan_legs.len() == 1
                        && route.routing_plan_legs[0].lane_id == LaneId::SINGLE
                        && route.routing_plan_legs[0].dataspace_id == DataSpaceId::UNIVERSAL,
                    "full retained routing plan changed scope"
                );
                found = Some(block.header().height().get());
            }
        }
    }
    found.ok_or_else(|| eyre!("signed native transaction missing from persisted execution history"))
}

fn bpng_transaction_ownership(
    history: &RetainedHistory,
    transaction: &SignedTransaction,
    incarnation: Hash,
    expected_validators: &[PeerId],
) -> Result<(u64, SumeragiLanePayloadOwnership)> {
    let expected_entrypoint = TransactionEntrypoint::External(transaction.clone());
    let transaction_hash = Hash::from(transaction.hash());
    let mut found = None;
    for wire in &history.blocks {
        let block = decode_framed_signed_block(wire)?;
        let matches = block
            .network_entrypoints()
            .enumerate()
            .filter(|(_, entrypoint)| *entrypoint == &expected_entrypoint)
            .collect::<Vec<_>>();
        if matches.is_empty() {
            continue;
        }
        ensure!(
            matches.len() == 1
                && block
                    .network_output_at(u32::try_from(matches[0].0)?)
                    .is_some_and(|(_, output)| output.result.0.is_ok())
                && found.is_none(),
            "BPNG transaction must have one successful retained execution"
        );
        let context = block
            .execution_context()
            .ok_or_else(|| eyre!("BPNG carrier omitted execution context"))?;
        let routes = context
            .external
            .iter()
            .filter(|route| route.entrypoint_hash == transaction.hash_as_entrypoint())
            .collect::<Vec<_>>();
        ensure!(
            routes.len() == 1
                && routes[0].lane_id == BPNG_FIXTURE_LANE
                && routes[0].dataspace_id == DataSpaceId::new(BPNG_ID)
                && routes[0].routing_plan_legs.len() == 1
                && routes[0].routing_plan_legs[0].lane_id == BPNG_FIXTURE_LANE
                && routes[0].routing_plan_legs[0].dataspace_id == DataSpaceId::new(BPNG_ID),
            "BPNG transaction did not retain its exact single-lane route"
        );
        let ownerships = context
            .lane_payload_ownerships
            .iter()
            .filter(|ownership| {
                ownership.lane_id == BPNG_FIXTURE_LANE
                    && ownership.dataspace_id == DataSpaceId::new(BPNG_ID)
            })
            .collect::<Vec<_>>();
        ensure!(
            ownerships.len() == 1
                && ownerships[0].accepted_transaction_hashes.as_slice()
                    == std::slice::from_ref(&transaction_hash),
            "BPNG carrier omitted, duplicated or widened exact singleton transaction ownership"
        );
        assert_bpng_ownership(ownerships[0], incarnation, expected_validators, transaction)?;
        ensure!(
            ownerships[0].proposal_height == block.header().height().get(),
            "BPNG ownership points to a different global carrier height"
        );
        found = Some((block.header().height().get(), ownerships[0].clone()));
    }
    found.ok_or_else(|| eyre!("BPNG signed transaction missing from retained Kura ownership"))
}

async fn stop_all(peers: &[NetworkPeer]) -> Result<()> {
    try_join_all(peers.iter().map(|peer| async move {
        ensure!(
            timeout(NETWORK_TIMEOUT, peer.shutdown_if_started()).await?,
            "expected running original peer"
        );
        Ok::<_, eyre::Report>(())
    }))
    .await?;
    Ok(())
}


#[test]
fn genesis_staking_plans_bind_funded_validators_to_configured_custody() {
    iroha_test_network::init_instruction_registry();
    let entries = (0..4)
        .map(|index| {
            let key = iroha_crypto::KeyPair::try_from_seed(
                vec![index + 1; 32],
                iroha_crypto::Algorithm::BlsNormal,
            )
            .expect("deterministic genesis staking validator");
            GenesisTopologyEntry::new(
                PeerId::new(key.public_key().clone()),
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("deterministic genesis staking validator PoP"),
            )
        })
        .collect::<Vec<_>>();
    let topology = entries
        .iter()
        .map(|entry| entry.peer.clone())
        .collect::<Vec<_>>();
    let transactions = custom_genesis_post_topology(&topology, &entries);
    let committee_registrations = transactions
        .iter()
        .flatten()
        .filter_map(|instruction| instruction.as_any().downcast_ref::<RegisterConsensusKey>())
        .collect::<Vec<_>>();
    assert_eq!(committee_registrations.len(), VALIDATOR_COUNT);
    for (registration, peer) in committee_registrations.iter().zip(&topology) {
        assert_eq!(registration.id, derive_committee_key_id(peer.public_key()));
        assert_eq!(registration.record.public_key, peer.public_key().clone());
    }
    let registrations = transactions
        .iter()
        .flatten()
        .filter_map(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<RegisterPublicLaneValidator>()
        })
        .collect::<Vec<_>>();
    assert_eq!(registrations.len(), 4);
    for registration in registrations {
        assert_eq!(
            registration.monetary_plan.network_scope,
            iroha_data_model::nexus::PublicLaneMonetaryScopeV1::Genesis
        );
        assert_eq!(registration.monetary_plan.valid_until_height, 1);
        assert_eq!(
            registration.monetary_plan.source_asset,
            iroha_data_model::asset::AssetId::new(
                stake_asset_definition_id(),
                registration.validator.clone()
            )
        );
        assert_eq!(
            registration.monetary_plan.destination_asset,
            iroha_data_model::asset::AssetId::new(
                stake_asset_definition_id(),
                staking_custody_account()
            )
        );
        assert_eq!(
            registration.monetary_plan.amount,
            registration.initial_stake
        );
        assert_eq!(
            registration.monetary_plan.precondition,
            iroha_data_model::nexus::PublicLaneMonetaryPreconditionV1::Registration(
                iroha_data_model::nexus::PublicLaneMonetaryRegistrationV1 {
                    activation_height: 1
                }
            )
        );
    }
}
