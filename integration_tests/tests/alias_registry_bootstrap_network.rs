//! Genuine four-validator NPoS alias bootstrap and retained-Kura replay.
//!
//! This isolated network uses the native SNS policy, signed planner and paid
//! leases. Ordinary transaction fees are zero from genesis to isolate lease
//! accounting; this is not production-fee qualification. The historical alias
//! is a universal-domain control, NOT private-to-universal transition evidence.
//! No unchecked blocks, fabricated certificates, injected WSV or storage reset
//! may substitute for the original persisted history and Strict daemon replay.
use std::{
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
use integration_tests::sandbox;
use iroha::{client::Client, sns::SnsNamespacePath};
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
};
use iroha_crypto::{Algorithm, Hash, KeyPair, PublicKey};
use iroha_data_model::{
    Level,
    alias_setup::{
        ALIAS_LEASE_YEAR_MS, AliasDataSpaceIntentV1, AliasDataspaceBootstrapGrantV1,
        AliasDomainIntentV1, AliasIntentV1, AliasLeaseAcquisitionV1, AliasPlanDispositionV1,
        AliasQuoteGuardV1, AliasRegistryRoutingActivationV1, AliasSetupPlanRequestV1,
        ResolvedDomainV1,
    },
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
        staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
    },
    nexus::{
        DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneLifecycleParameterV1,
        LaneLifecyclePlan, LaneLifecycleStatusV1, LaneVisibility,
    },
    parameter::{Parameters, system::SumeragiNposParameters},
    prelude::*,
    sns::{NameRecordV1, NameSelectorV1, NameStatus, SuffixPolicyV1},
    transaction::{FeePaymentIntent, SignedTransaction, TransactionEntrypoint},
};
use iroha_executor_data_model::permission::peer::CanManagePeers;
use iroha_genesis::GenesisBlock;
use iroha_primitives::json::Json;
use iroha_test_network::{
    NetworkBuilder, NetworkPeer, ReleasePrebuiltBinary, genesis_factory_with_post_topology,
    init_instruction_registry, resolve_release_prebuilt_binary,
};
use iroha_test_samples::{BOB_ID, BOB_KEYPAIR};
use norito::codec::Encode as _;
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

fn stake_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::derive_from_components(
        DomainId::try_new("nexus", "universal").expect("nexus domain"),
        "xor".parse().expect("stake asset name"),
    )
}

fn custom_genesis_post_topology(topology: &[PeerId]) -> Vec<Vec<InstructionBox>> {
    assert_eq!(
        topology.len(),
        VALIDATOR_COUNT,
        "custom genesis requires exactly four BLS validator peers"
    );
    let stake_asset_id = stake_asset_definition_id();
    let stake = SumeragiNposParameters::default().min_self_bond().clone();
    let two_stakes = stake
        .checked_add(&stake)
        .expect("two validator self-stakes must be representable");
    let mut bootstrap = vec![
        Register::domain(Domain::new(
            DomainId::try_new("nexus", "universal").expect("nexus domain"),
        ))
        .into(),
        Register::asset_definition(AssetDefinition::numeric(
            stake_asset_id.clone(),
            "Retained BPNG lane fixture stake".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .into(),
    ];
    let mut default_lane_validators = Vec::with_capacity(VALIDATOR_COUNT * 2);
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
            )
            .into(),
        );
        default_lane_validators
            .push(ActivatePublicLaneValidator::new(LaneId::SINGLE, validator).into());
    }
    vec![bootstrap, default_lane_validators]
}

fn bounded_client(mut client: Client) -> Client {
    client.transaction_status_timeout = SUBMISSION_TIMEOUT;
    client.transaction_ttl = Some(TRANSACTION_TTL);
    client.torii_request_timeout = Duration::from_secs(20);
    client
}

async fn read<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    timeout(READ_TIMEOUT, tokio::task::spawn_blocking(operation))
        .await
        .wrap_err("bounded fixture read timed out")?
        .wrap_err("fixture read task failed")?
}

async fn height(client: &Client) -> Result<u64> {
    let client = client.clone();
    read(move || Ok(client.get_status()?.blocks)).await
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

async fn observe_catalog_expansion(
    client: &Client,
    bpng_configured: bool,
) -> Result<LaneLifecycleStatusV1> {
    let lane_client = client.clone();
    let lanes = read(move || {
        let status = lane_client.get_lane_lifecycle_status()?;
        ensure!(
            status.validate()? == LaneCatalog::default(),
            "dataspace-only expansion changed the original lane catalog"
        );
        Ok(status)
    })
    .await?;
    // This namespace parses through the daemon's actual static catalog before
    // looking in SNS state. A paid dataspace lease alone cannot satisfy it.
    // Status telemetry instead projects lane-backed dataspaces, so it cannot
    // prove this deliberately lane-free catalog addition.
    let url = client
        .torii_url
        .join("v1/sns/names/account-alias/catalog-probe@mibank.bpng")?;
    let mut response = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::none())
        .build()?
        .get(url)
        .send()
        .await?;
    let status = response.status().as_u16();
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            body.len().saturating_add(chunk.len()) <= 4096,
            "catalog probe response exceeded its fixed budget"
        );
        body.extend_from_slice(&chunk);
    }
    let (expected_status, expected_body) = if bpng_configured {
        (404, "registration `catalog-probe@mibank.bpng` not found")
    } else {
        (400, "unknown dataspace alias in account alias")
    };
    ensure!(
        status == expected_status && body == expected_body.as_bytes(),
        "live BPNG catalog probe did not match the expected configured state: configured={bpng_configured}, status={status}"
    );
    Ok(lanes)
}

async fn submit(client: &Client, transaction: SignedTransaction) -> Result<SignedTransaction> {
    let submitter = client.clone();
    let signed = transaction.clone();
    timeout(
        SUBMISSION_TASK_TIMEOUT,
        tokio::task::spawn_blocking(move || submitter.submit_transaction_blocking(&signed)),
    )
    .await
    .wrap_err("native transaction did not reach terminal status in time")?
    .wrap_err("native submission task failed")??;
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

fn assert_bpng_lifecycle_status(
    status: &LaneLifecycleStatusV1,
    expected_original: &LaneLifecycleStatusV1,
) -> Result<Hash> {
    let original_catalog = expected_original.validate()?;
    ensure!(
        original_catalog == LaneCatalog::default(),
        "fixture must begin from the single universal lane"
    );
    let expected_catalog = original_catalog.apply_lifecycle(&LaneLifecyclePlan {
        additions: vec![bpng_fixture_lane()],
        retire: Vec::new(),
    })?;
    ensure!(
        status.validate()? == expected_catalog,
        "signed lifecycle did not add exactly fixture-only lane 8"
    );
    let original_incarnation = expected_original
        .incarnations
        .iter()
        .find(|entry| entry.lane_id == LaneId::SINGLE)
        .ok_or_else(|| eyre!("original universal incarnation missing"))?;
    let retained_original = status
        .incarnations
        .iter()
        .find(|entry| entry.lane_id == LaneId::SINGLE)
        .ok_or_else(|| eyre!("post-lifecycle universal incarnation missing"))?;
    ensure!(
        original_incarnation == retained_original,
        "adding BPNG changed the universal lane incarnation"
    );
    let bpng = status
        .incarnations
        .iter()
        .find(|entry| entry.lane_id == BPNG_FIXTURE_LANE)
        .ok_or_else(|| eyre!("fixture-only BPNG lane incarnation missing"))?;
    ensure!(
        status.incarnations.len() == 2 && bpng.incarnation.as_ref().iter().any(|byte| *byte != 0),
        "lifecycle must advertise exactly two unique non-zero lane incarnations"
    );
    Ok(bpng.incarnation)
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
        let validator = item
            .get("validator")
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("lane validator item omitted validator"))?
            .parse()?;
        let peer = item
            .get("peer_id")
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("lane validator item omitted peer_id"))?
            .parse()?;
        let stake_account: AccountId = item
            .get("stake_account")
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("lane validator item omitted stake_account"))?
            .parse()?;
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
            let snapshot = query_client.get_public_lane_validators(BPNG_FIXTURE_LANE)?;
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
                let snapshot = client.get_public_lane_validators(BPNG_FIXTURE_LANE)?;
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
    client.build_transaction(
        [instruction.into()],
        FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    )
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
        let policy = planning_client.sns().get_policy(namespace.suffix_id())?;
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
        let plan = planning_client.plan_alias_setup(&request)?;
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
        let instructions = planning_client.verify_alias_setup_plan_for_request(&request, &plan)?;
        ensure!(
            instructions.len() == 1,
            "one native EnsureAlias instruction expected"
        );
        let signed = planning_client.build_transaction(
            instructions,
            FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        );
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
    for asset in client.query(FindAssets::new()).execute_all()? {
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
    activation: AliasRegistryRoutingActivationV1,
    grant: &AliasDataspaceBootstrapGrantV1,
    transactions: &[SignedTransaction],
) -> Result<LedgerSnapshot> {
    let parameters: Parameters = client.query_single(FindParameters::new())?;
    let custom = parameters
        .custom()
        .get(&AliasRegistryRoutingActivationV1::parameter_id())
        .ok_or_else(|| eyre!("routing activation parameter missing"))?;
    ensure!(
        AliasRegistryRoutingActivationV1::from_custom_parameter(custom)? == Some(activation),
        "routing activation changed"
    );
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
            client.sns().get_policy(expected.namespace.suffix_id())? == expected.policy,
            "native suffix policy drifted"
        );
        let record = client
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
    for domain in client.query(FindDomains::new()).execute_all()? {
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
    let committed = client.query(FindTransactions::new()).execute_all()?;
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
    activation: AliasRegistryRoutingActivationV1,
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
        let observed = read(move || {
            ledger_snapshot(&client, &expectations, activation, &grant, &transactions)
        })
        .await;
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

fn assert_bpng_ownership(
    ownership: &SumeragiLanePayloadOwnership,
    incarnation: Hash,
    expected_validators: &[PeerId],
    transaction: &SignedTransaction,
) -> Result<()> {
    ownership.validate_replay_material()?;
    let transaction_hash = Hash::from(transaction.hash());
    ensure!(
        ownership.lane_id == BPNG_FIXTURE_LANE
            && ownership.dataspace_id == DataSpaceId::new(BPNG_ID)
            && ownership.lane_incarnation == incarnation,
        "BPNG payload ownership route/incarnation mismatch"
    );
    ensure!(
        ownership.accepted_transaction_hashes.as_slice() == std::slice::from_ref(&transaction_hash),
        "BPNG ownership must retain the exact ordered singleton signed-transaction hash vector"
    );
    ensure!(
        ownership.lane_block_descriptor_validator_set == expected_validators
            && ownership.lane_block_descriptor_validator_count
                == u32::try_from(expected_validators.len())?
            && ownership.lane_block_descriptor_min_quorum == BPNG_MIN_QUORUM,
        "BPNG ownership must bind the exact four-validator, three-signature committee"
    );
    Ok(())
}

async fn wait_for_bpng_frontier(
    clients: &[Client],
    incarnation: Hash,
    expected_validators: &[PeerId],
    transaction: &SignedTransaction,
) -> Result<SumeragiLanePayloadOwnership> {
    let deadline = Instant::now() + CONVERGENCE_TIMEOUT;
    let transaction_hash = Hash::from(transaction.hash());
    loop {
        let mut observations = Vec::new();
        for client in clients {
            let client = client.clone();
            let observed = read(move || {
                let diagnostics = client.get_sumeragi_diagnostics()?;
                let ownerships = diagnostics
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
                    "BPNG ownership frontier must be one exact ordered singleton"
                );
                let ownership = (*ownerships[0]).clone();
                let descriptor_hash = ownership
                    .lane_block_descriptor_hash
                    .ok_or_else(|| eyre!("BPNG ownership omitted descriptor hash"))?;
                ensure!(
                    diagnostics.committed_lane_blocks.iter().any(|block| {
                        block.lane_id == ownership.lane_id
                            && block.dataspace_id == ownership.dataspace_id
                            && block.lane_incarnation == ownership.lane_incarnation
                            && block.lane_block_height == ownership.lane_block_height
                            && block.lane_block_view == ownership.lane_block_view
                            && block.descriptor_hash == descriptor_hash
                            && block.subject_hash == ownership.subject_hash
                            && block.payload_ownership_hash == ownership.payload_ownership_hash
                            && block.rbc_instance_hash == ownership.rbc_instance_hash
                            && block.validator_count == u32::try_from(VALIDATOR_COUNT).unwrap()
                            && block.min_quorum == BPNG_MIN_QUORUM
                            && block.prepare_qc_signer_count == BPNG_MIN_QUORUM
                            && block.commit_qc_signer_count == BPNG_MIN_QUORUM
                            && block.executable_payload_available
                            && block.execution_status
                                == COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK
                    }),
                    "BPNG ownership has no matching applied certified-lane status"
                );
                Ok(ownership)
            })
            .await;
            match observed {
                Ok(ownership) => {
                    assert_bpng_ownership(
                        &ownership,
                        incarnation,
                        expected_validators,
                        transaction,
                    )?;
                    observations.push(ownership);
                }
                Err(_) => break,
            }
        }
        if observations.len() == clients.len()
            && observations.windows(2).all(|pair| pair[0] == pair[1])
        {
            return observations
                .into_iter()
                .next()
                .ok_or_else(|| eyre!("four-peer network has no BPNG ownership"));
        }
        ensure!(
            Instant::now() < deadline,
            "BPNG ownership/certified-lane frontier did not converge on all four peers"
        );
        sleep(POLL_INTERVAL).await;
    }
}

fn assert_bpng_metadata(
    client: &Client,
    predecessor_key: &Name,
    predecessor_value: &Json,
    successor: Option<(&Name, &Json)>,
) -> Result<()> {
    let domain = client.query_single(FindDomainById::new(DomainId::try_new("mibank", "bpng")?))?;
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

fn ownership_matches_descriptor(
    ownership: &SumeragiLanePayloadOwnership,
    descriptor: &LaneBlockDescriptorV1,
) -> bool {
    ownership.lane_id == descriptor.lane_id
        && ownership.dataspace_id == descriptor.dataspace_id
        && ownership.lane_incarnation == descriptor.lane_incarnation
        && ownership.proposal_height == descriptor.proposal_height
        && ownership.lane_block_height == descriptor.lane_block_height
        && ownership.lane_block_view == descriptor.lane_block_view
        && ownership.subject_hash == descriptor.subject_hash
        && ownership.accepted_candidate_indices == descriptor.accepted_candidate_indices
        && ownership.accepted_transaction_hashes == descriptor.accepted_transaction_hashes
        && ownership.previous_lane_block_height == descriptor.previous_lane_block_height
        && ownership.previous_lane_block_descriptor_hash
            == descriptor.previous_lane_block_descriptor_hash
        && ownership.lane_block_descriptor_hash == Some(descriptor.descriptor_hash)
        && ownership.lane_block_descriptor_validator_set == descriptor.validator_set
        && ownership.lane_block_descriptor_validator_count == descriptor.validator_count
        && ownership.lane_block_descriptor_min_quorum == descriptor.min_quorum
        && ownership.payload_ownership_hash == descriptor.payload_ownership_hash
        && ownership.rbc_instance_hash == descriptor.rbc_instance_hash
        && ownership.qc_mode_tag == descriptor.qc_mode_tag
}

fn bitmap_signer_count(bitmap: &[u8]) -> u32 {
    bitmap.iter().map(|byte| byte.count_ones()).sum()
}

fn bpng_certified_sidecar_paths(store_root: &Path) -> Result<(PathBuf, PathBuf)> {
    let catalog = LaneCatalog::default().apply_lifecycle(&LaneLifecyclePlan {
        additions: vec![bpng_fixture_lane()],
        retire: Vec::new(),
    })?;
    let lanes = ActualLaneConfig::from_catalog(&catalog);
    let entry = lanes
        .entry(BPNG_FIXTURE_LANE)
        .ok_or_else(|| eyre!("fixture-only BPNG lane has no derived Kura segment"))?;
    let directory = entry.blocks_dir(store_root).join("lane_artifacts");
    Ok((
        directory.join("certified_blocks.norito"),
        directory.join("certified_blocks.index"),
    ))
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

// Emergency-Fast Kura deliberately withholds dynamic-lane readers. Retain the
// stopped raw indexed sidecar pair and its SHA-256s without mutating it, then
// run the same public proposal/QC/aggregate validators used by the shipping
// Kura reader.
fn inspect_certified_bpng_lane_evidence(
    store_root: &Path,
    retained: &RetainedHistory,
    expected_incarnation: Option<Hash>,
    expected_validators: &[PeerId],
    original_voters: &BTreeMap<PeerId, Vec<u8>>,
) -> Result<CertifiedBpngLaneEvidence> {
    let (data_path, index_path) = bpng_certified_sidecar_paths(store_root)?;
    let before = (
        read_optional_evidence_file(&data_path)?,
        read_optional_evidence_file(&index_path)?,
    );
    let (Some(data), Some(index)) = (&before.0, &before.1) else {
        ensure!(
            before.0.is_none() && before.1.is_none() && expected_incarnation.is_none(),
            "retained BPNG certified sidecar pair is incomplete or unexpectedly absent"
        );
        return Ok(CertifiedBpngLaneEvidence::absent());
    };
    let incarnation = expected_incarnation
        .ok_or_else(|| eyre!("BPNG certified lane blocks exist before the signed lifecycle"))?;
    ensure!(
        !data.is_empty()
            && index.len() >= SIDECAR_INDEX_HEADER_BYTES
            && (index.len() - SIDECAR_INDEX_HEADER_BYTES) % SIDECAR_INDEX_ENTRY_BYTES == 0,
        "retained BPNG certified sidecar pair is empty, truncated or misaligned"
    );
    ensure!(
        sidecar_u64(index, 0)? == u64::MAX && sidecar_u64(index, size_of::<u64>())? == u64::MAX,
        "retained BPNG certified index omitted its V1 marker"
    );
    let base_height = sidecar_u64(index, SIDECAR_INDEX_ENTRY_BYTES)?;
    ensure!(
        base_height == 1
            && sidecar_u64(index, SIDECAR_INDEX_ENTRY_BYTES + size_of::<u64>())?
                == base_height ^ SIDECAR_INDEX_CHECK_MASK,
        "retained BPNG certified index must begin at exact lane height one"
    );
    let entry_count = (index.len() - SIDECAR_INDEX_HEADER_BYTES) / SIDECAR_INDEX_ENTRY_BYTES;
    ensure!(
        entry_count > 0 && u64::try_from(entry_count)? <= MAX_RETAINED_HEIGHT,
        "retained BPNG certified index exceeds the fixture bound"
    );
    let mut previous_height = 0_u64;
    let mut previous_descriptor = None;
    let mut encoded = Vec::with_capacity(entry_count);
    let mut indexed_end = 0_u64;
    for slot in 0..entry_count {
        let position = SIDECAR_INDEX_HEADER_BYTES
            .checked_add(
                slot.checked_mul(SIDECAR_INDEX_ENTRY_BYTES)
                    .ok_or_else(|| eyre!("retained BPNG sidecar slot overflow"))?,
            )
            .ok_or_else(|| eyre!("retained BPNG sidecar position overflow"))?;
        let offset = sidecar_u64(index, position)?;
        let len = sidecar_u64(index, position + size_of::<u64>())?;
        ensure!(
            len > 0 && offset == indexed_end,
            "retained BPNG sidecar index must cover non-empty data contiguously from offset zero"
        );
        let end = offset
            .checked_add(len)
            .ok_or_else(|| eyre!("retained BPNG sidecar range overflow"))?;
        ensure!(
            len <= MAX_EVIDENCE_BYTES && end <= u64::try_from(data.len())?,
            "retained BPNG sidecar slot points outside its bounded data file"
        );
        let start = usize::try_from(offset)?;
        let end_usize = usize::try_from(end)?;
        let bytes = data
            .get(start..end_usize)
            .ok_or_else(|| eyre!("retained BPNG sidecar payload range is invalid"))?
            .to_vec();
        let artifact = norito::decode_canonical::<CertifiedLaneBlockArtifact>(&bytes)?;
        validate_certified_bpng_artifact(&artifact)?;
        ensure!(
            artifact.encode_framed()? == bytes,
            "retained BPNG certified artifact is not canonical"
        );
        let descriptor = &artifact.proposal.descriptor;
        let indexed_height = base_height
            .checked_add(u64::try_from(slot)?)
            .ok_or_else(|| eyre!("retained BPNG certified height overflow"))?;
        ensure!(
            descriptor.lane_id == BPNG_FIXTURE_LANE
                && descriptor.dataspace_id == DataSpaceId::new(BPNG_ID)
                && descriptor.lane_incarnation == incarnation
                && descriptor.lane_block_height == indexed_height,
            "certified BPNG lane block route/incarnation mismatch"
        );
        ensure!(
            descriptor.lane_block_height == previous_height.saturating_add(1)
                && descriptor.previous_lane_block_height == previous_height
                && descriptor.previous_lane_block_descriptor_hash == previous_descriptor,
            "certified BPNG lane ownership chain is not contiguous"
        );
        ensure!(
            descriptor.validator_set == expected_validators
                && descriptor.validator_count == u32::try_from(expected_validators.len())?
                && descriptor.min_quorum == BPNG_MIN_QUORUM,
            "certified BPNG descriptor must bind exactly four validators and quorum three"
        );
        ensure!(
            artifact.prepare_qc.validator_set == expected_validators
                && artifact.commit_qc.validator_set == expected_validators
                && artifact.prepare_qc.body.validator_count
                    == u32::try_from(expected_validators.len())?
                && artifact.commit_qc.body.validator_count
                    == u32::try_from(expected_validators.len())?
                && artifact.prepare_qc.body.min_quorum == BPNG_MIN_QUORUM
                && artifact.commit_qc.body.min_quorum == BPNG_MIN_QUORUM
                && bitmap_signer_count(&artifact.prepare_qc.signers_bitmap) == BPNG_MIN_QUORUM
                && bitmap_signer_count(&artifact.commit_qc.signers_bitmap) == BPNG_MIN_QUORUM,
            "retained BPNG lane QCs are not exact three-of-four certificates"
        );
        ensure!(
            artifact.prepare_qc.payload_availability_qc.is_none()
                && artifact.commit_qc.payload_availability_qc.is_none(),
            "globally carried BPNG lane block unexpectedly uses autonomous availability proof"
        );
        for (public_key, pop) in &artifact.signer_pops {
            ensure!(
                original_voters.get(&PeerId::new(public_key.clone())) == Some(pop),
                "retained BPNG lane QC PoP was not signed into original genesis"
            );
        }
        let hint = artifact
            .proposal
            .payload_block_hint
            .as_ref()
            .ok_or_else(|| eyre!("certified BPNG lane block omitted global carrier hint"))?;
        ensure!(
            hint.proposal_height == descriptor.proposal_height,
            "BPNG certified artifact carrier height mismatch"
        );
        let carrier_index = usize::try_from(hint.proposal_height.saturating_sub(1))?;
        let carrier_wire = retained
            .blocks
            .get(carrier_index)
            .ok_or_else(|| eyre!("BPNG certified artifact points outside retained Kura"))?;
        let carrier = decode_framed_signed_block(carrier_wire)?;
        ensure!(
            carrier.hash() == hint.proposal_block_hash,
            "BPNG certified artifact points to a different retained carrier"
        );
        let ownership = carrier
            .execution_context()
            .and_then(|context| {
                context
                    .lane_payload_ownerships
                    .iter()
                    .find(|ownership| ownership_matches_descriptor(ownership, descriptor))
            })
            .ok_or_else(|| eyre!("retained Kura carrier omitted certified BPNG ownership"))?;
        ownership.validate_replay_material()?;
        previous_height = descriptor.lane_block_height;
        previous_descriptor = Some(descriptor.descriptor_hash);
        indexed_end = end;
        encoded.push(bytes);
    }
    ensure!(
        encoded.len() == entry_count
            && indexed_end == u64::try_from(data.len())?
            && !data.is_empty(),
        "retained BPNG certified data lacks exact contiguous indexed coverage"
    );
    ensure!(
        (
            read_optional_evidence_file(&data_path)?,
            read_optional_evidence_file(&index_path)?,
        ) == before,
        "retained BPNG certified sidecar pair changed during read-only inspection"
    );
    Ok(CertifiedBpngLaneEvidence::new(
        encoded,
        data.clone(),
        index.clone(),
    ))
}

fn inspection_fingerprint(
    blocks_dir: &Path,
    indexed_count: u64,
) -> Result<BTreeMap<PathBuf, (u64, Hash)>> {
    let mut paths = ["blocks.data", "blocks.index", "blocks.hashes"]
        .map(PathBuf::from)
        .to_vec();
    for height in 1..=indexed_count {
        for directory in [
            "wsv_checkpoints",
            "commit_manifests",
            "v2_finality",
            "retained_blocks",
        ] {
            paths.push(PathBuf::from(directory).join(format!("{height:020}.norito")));
        }
    }
    let mut fingerprint = BTreeMap::new();
    let mut total = 0_u64;
    for relative in paths {
        let path = blocks_dir.join(&relative);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && relative.components().count() == 2 =>
            {
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        ensure!(
            metadata.is_file(),
            "inspection evidence must be a regular file"
        );
        total = total
            .checked_add(metadata.len())
            .ok_or_else(|| eyre!("inspection size overflow"))?;
        ensure!(
            total <= MAX_EVIDENCE_BYTES,
            "inspection evidence exceeds fixture budget"
        );
        fingerprint.insert(relative, (metadata.len(), Hash::new(fs::read(path)?)));
    }
    Ok(fingerprint)
}

fn read_retained_prefix(blocks_dir: &Path, prefix: u64) -> Result<RetainedHistory> {
    ensure!(
        (1..=MAX_RETAINED_HEIGHT).contains(&prefix),
        "retained prefix exceeds fixture bound"
    );
    let mut store = BlockStore::open_read_only(blocks_dir)?;
    let mut blocks = Vec::new();
    let mut sidecars = BTreeMap::new();
    let mut total = 0_u64;
    for height in 1..=prefix {
        let mut index = [BlockIndex::default()];
        store.read_block_indices(height - 1, &mut index)?;
        ensure!(
            index[0].length > 0 && index[0].length <= MAX_EVIDENCE_BYTES,
            "missing/oversized retained block body at {height}"
        );
        total = total
            .checked_add(index[0].length)
            .ok_or_else(|| eyre!("evidence size overflow"))?;
        ensure!(
            total <= MAX_EVIDENCE_BYTES,
            "retained evidence exceeds fixture budget"
        );
        let wire = store.block_bytes(index[0].start, index[0].length)?.to_vec();
        ensure!(
            decode_framed_signed_block(&wire)?.header().height().get() == height,
            "stored block height mismatch"
        );
        blocks.push(wire);
        for directory in [
            "wsv_checkpoints",
            "commit_manifests",
            "v2_finality",
            "retained_blocks",
        ] {
            let relative = PathBuf::from(directory).join(format!("{height:020}.norito"));
            let path = blocks_dir.join(&relative);
            let metadata = fs::symlink_metadata(&path)
                .wrap_err_with(|| format!("missing required retained evidence: {relative:?}"))?;
            ensure!(
                metadata.is_file() && metadata.len() > 0,
                "retained sidecar must be a nonempty regular file"
            );
            total = total
                .checked_add(metadata.len())
                .ok_or_else(|| eyre!("evidence size overflow"))?;
            ensure!(
                total <= MAX_EVIDENCE_BYTES,
                "retained evidence exceeds fixture budget"
            );
            sidecars.insert(relative, fs::read(path)?);
        }
    }
    Ok(RetainedHistory { blocks, sidecars })
}

fn assert_finality(
    artifact: &V2FinalityArtifact,
    block: &SignedBlock,
    wire: &[u8],
    network_id: NetworkId,
    original_voters: &BTreeMap<PeerId, Vec<u8>>,
) -> Result<()> {
    artifact.validate_for_header(&block.header())?;
    let context = &artifact.height_context;
    ensure!(
        context.network_id == network_id && context.mode == ConsensusMode::Npos,
        "finality network/mode changed"
    );
    ensure!(
        context.roster.len() == 4 && artifact.validator_set_pops.len() == 4,
        "finality must retain four original voters"
    );
    ensure!(
        context.quorum.total_power == 4
            && context.quorum.min_signers == 3
            && artifact.commit_qc.signers.len() == 3,
        "expected genuine three-of-four CommitQC"
    );
    for (voter, pop) in context.roster.iter().zip(&artifact.validator_set_pops) {
        ensure!(
            voter.power == 1 && original_voters.get(&voter.validator) == Some(pop),
            "finality voter/PoP differs from signed genesis"
        );
    }
    ensure!(
        artifact.subject.payload_hash == block.canonical_proposal_wire_hash()?,
        "certified proposal wire mismatch"
    );
    let execution = &artifact.commit_qc.execution_commitment;
    ensure!(
        execution.executed_block_wire_len == u64::try_from(wire.len())?
            && execution.executed_block_wire_hash == Hash::new(wire),
        "CommitQC does not authenticate exact executed block bytes"
    );
    // The public Kura getter has already cryptographically verified the BLS
    // aggregate and PoPs. Never replace it with a self-asserted synthetic QC.
    Ok(())
}

fn inspect_stopped_peer(
    peer: &NetworkPeer,
    prefix: Option<u64>,
    genesis: &GenesisBlock,
    original_voters: &BTreeMap<PeerId, Vec<u8>>,
    expected_bpng_incarnation: Option<Hash>,
    expected_bpng_validators: &[PeerId],
) -> Result<StoppedEvidence> {
    let catalog = LaneCatalog::default();
    let lanes = ActualLaneConfig::from_catalog(&catalog);
    let blocks_dir = lanes.primary().blocks_dir(peer.kura_store_dir());
    let indexed_count = BlockStore::open_read_only(&blocks_dir)?.read_index_count()?;
    ensure!(
        (1..=MAX_RETAINED_HEIGHT).contains(&indexed_count),
        "missing/oversized stopped Kura history"
    );
    // Physical journal length may contain an unpublished suffix. Preserve it
    // without treating it as committed or demanding absent suffix sidecars.
    let fingerprint = inspection_fingerprint(&blocks_dir, indexed_count)?;
    // Inspection ONLY. Fast opens existing journals without repair or writes;
    // its existing store-root lock is O_RDWR/create(false), never initialized.
    // The actual daemon always replays in Strict with snapshots disabled.
    let config = KuraConfig {
        init_mode: iroha_config::kura::InitMode::Fast,
        store_dir: WithOrigin::inline(peer.kura_store_dir()),
        max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: NonZeroUsize::new(2).expect("nonzero"),
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity: defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: FsyncMode::Batched,
        fsync_interval: defaults::kura::FSYNC_INTERVAL,
        lane_history_retention: defaults::kura::LANE_HISTORY_RETENTION,
        replica_advert: defaults::kura::REPLICA_ADVERT_POLICY,
    };
    let (kura, _) = Kura::new_with_configured_lane_catalog(&config, &lanes, &catalog)?;
    let durable_count = u64::try_from(kura.exact_durable_blocks_count()?)?;
    ensure!(
        durable_count > 0 && durable_count <= indexed_count,
        "stopped store has no authenticated durable prefix"
    );
    let prefix = prefix.unwrap_or(durable_count);
    ensure!(
        prefix <= durable_count,
        "restart lost retained carrier heights"
    );
    let retained = read_retained_prefix(&blocks_dir, prefix)?;
    for (index, wire) in retained.blocks.iter().enumerate() {
        let height = u64::try_from(index)? + 1;
        let block = decode_framed_signed_block(wire)?;
        let artifact = kura
            .v2_finality_artifact(height)?
            .ok_or_else(|| eyre!("missing genuine CommitQC at height {height}"))?;
        assert_finality(
            &artifact,
            &block,
            wire,
            NetworkId::from_genesis_hash(genesis.0.hash()),
            original_voters,
        )?;
        if height == 1 {
            ensure!(
                block.hash() == genesis.0.hash() && block.execution_context().is_none(),
                "original genesis identity/context changed"
            );
            ensure!(
                block.canonical_resultless_proposal().encode_wire()?
                    == genesis.0.canonical_resultless_proposal().encode_wire()?,
                "canonical signed genesis proposal changed"
            );
            iroha_core::sumeragi::validate_signed_genesis_v2_authority(
                genesis,
                &artifact.height_context,
                &artifact.validator_set_pops,
            )?;
        }
    }
    let certified_bpng_lane = inspect_certified_bpng_lane_evidence(
        &peer.kura_store_dir(),
        &retained,
        expected_bpng_incarnation,
        expected_bpng_validators,
        original_voters,
    )?;
    drop(kura); // Release every inspector handle before Strict daemon startup.
    ensure!(
        inspection_fingerprint(&blocks_dir, indexed_count)? == fingerprint
            && read_retained_prefix(&blocks_dir, prefix)? == retained,
        "inspection changed retained or unpublished journal/sidecar bytes"
    );
    Ok(StoppedEvidence {
        retained,
        certified_bpng_lane,
    })
}

fn execution_height(history: &RetainedHistory, transaction: &SignedTransaction) -> Result<u64> {
    let expected = TransactionEntrypoint::External(transaction.clone());
    let mut found = None;
    for wire in &history.blocks {
        let block = decode_framed_signed_block(wire)?;
        for (_, entrypoint, result) in block.entrypoint_results() {
            if entrypoint == expected {
                ensure!(
                    found.is_none() && result.0.is_ok(),
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
            .entrypoint_results()
            .filter(|(_, entrypoint, _)| entrypoint == &expected_entrypoint)
            .collect::<Vec<_>>();
        if matches.is_empty() {
            continue;
        }
        ensure!(
            matches.len() == 1 && matches[0].2.0.is_ok() && found.is_none(),
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn bpng_native_bootstrap_survives_four_peer_retained_kura_catalog_expansion() -> Result<()> {
    required_prebuilt_binaries()?;
    init_instruction_registry();
    let mut npos = SumeragiNposParameters::default();
    npos.max_validators = u32::try_from(VALIDATOR_COUNT)?;
    // Keep the genuine next-election boundary inside this bounded integration
    // history; validator promotion still waits for the signed epoch boundary.
    // These are fixture horizons, not public-Taira operating recommendations.
    npos.epoch_length_blocks =
        NonZeroU64::new(FIXTURE_EPOCH_LENGTH_BLOCKS).expect("non-zero fixture epoch");
    npos.finality_margin_blocks = 2;
    npos.evidence_horizon_blocks = 16;
    npos.slashing_delay_blocks = 8;
    npos.validate()?;
    let builder = NetworkBuilder::new()
        .with_peers(VALIDATOR_COUNT)
        .with_base_seed(NETWORK_SEED)
        .with_auto_populated_trusted_peers()
        .with_npos_consensus()
        .without_npos_genesis_bootstrap()
        .with_genesis_block(|topology, topology_entries| {
            genesis_factory_with_post_topology(
                Vec::new(),
                custom_genesis_post_topology(topology.as_ref()),
                topology,
                topology_entries,
            )
        })
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos.into_custom_parameter(),
        )))
        .with_block_cadence(Duration::from_secs(1))
        .with_config_layer(|layer| {
            layer
                .write(["snapshot", "mode"], "disabled")
                .write(["kura", "init_mode"], "strict")
                .write(
                    ["nexus", "storage", "local_budget_bytes"],
                    1_073_741_824_i64,
                );
            // Fixed in the ORIGINAL network, never changed during restart.
            for field in [
                "base_fee",
                "per_byte_fee",
                "per_instruction_fee",
                "per_gas_unit_fee",
            ] {
                layer.write(["nexus", "fees", field], "0");
            }
        });
    let network = timeout(
        NETWORK_TIMEOUT,
        sandbox::start_network_async_or_skip(
            builder,
            stringify!(
                bpng_native_bootstrap_survives_four_peer_retained_kura_catalog_expansion
            ),
        ),
    )
    .await??
    .ok_or_else(|| {
        eyre!(
            "this retained-Kura qualification requires a real four-peer network; skipping is forbidden"
        )
    })?;
    ensure!(
        network.peers().len() == VALIDATOR_COUNT,
        "exactly four original validators required"
    );
    let genesis = network.genesis();
    let original_voters = network
        .peers()
        .iter()
        .map(|peer| {
            let key = peer
                .bls_public_key()
                .ok_or_else(|| eyre!("missing original BLS voter key"))?;
            ensure!(
                key.algorithm() == Algorithm::BlsNormal
                    && PeerId::new(key.clone()) == peer.network_peer_id(),
                "original BLS voter identity mismatch"
            );
            Ok((
                peer.network_peer_id(),
                peer.bls_pop()
                    .ok_or_else(|| eyre!("missing original PoP"))?
                    .to_vec(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    ensure!(
        original_voters.len() == VALIDATOR_COUNT
            && iroha_core::sumeragi::signed_genesis_validator_pops(&genesis)? == original_voters,
        "signed genesis must contain exactly the original four voters"
    );
    let expected_validator_peers = original_voters.keys().cloned().collect::<Vec<_>>();
    let validator_keypairs = (0..VALIDATOR_COUNT)
        .map(validator_keypair)
        .collect::<Vec<_>>();
    let expected_validator_bindings = network
        .peers()
        .iter()
        .zip(&validator_keypairs)
        .map(|(peer, keypair)| {
            ensure!(
                peer.streaming_public_key() == keypair.public_key(),
                "deterministic validator signer differs from NetworkBuilder identity"
            );
            Ok((
                AccountId::new(keypair.public_key().clone()),
                peer.network_peer_id(),
            ))
        })
        .collect::<Result<BTreeSet<_>>>()?;
    ensure!(
        expected_validator_bindings.len() == VALIDATOR_COUNT,
        "validator signer/peer bindings must be unique"
    );
    let clients = network
        .peers()
        .iter()
        .map(|peer| bounded_client(peer.client()))
        .collect::<Vec<_>>();
    let validator_clients = network
        .peers()
        .iter()
        .zip(&validator_keypairs)
        .map(|(peer, keypair)| {
            bounded_client(peer.client_for(
                &AccountId::new(keypair.public_key().clone()),
                keypair.private_key().clone(),
            ))
        })
        .collect::<Vec<_>>();
    let authority = &clients[0];
    let payer =
        bounded_client(network.peers()[0].client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone()));
    let grant = AliasDataspaceBootstrapGrantV1::try_new("bpng", BOB_ID.clone())?;
    ensure!(
        grant.dataspace.dataspace_id.as_u64() == BPNG_ID,
        "canonical BPNG identity must never be DPN 10"
    );
    let read_client = authority.clone();
    let baseline_leases = read(move || {
        let params: Parameters = read_client.query_single(FindParameters::new())?;
        ensure!(
            !params
                .custom()
                .contains_key(&AliasRegistryRoutingActivationV1::parameter_id()),
            "history must begin before routing activation is installed"
        );
        [SnsNamespacePath::Domain, SnsNamespacePath::Dataspace]
            .into_iter()
            .map(|namespace| {
                Ok(LeaseExpectation {
                    namespace,
                    literal: String::new(),
                    policy: read_client.sns().get_policy(namespace.suffix_id())?,
                    amount: Quantity::zero(),
                })
            })
            .collect::<Result<Vec<_>>>()
    })
    .await?;
    let read_client = authority.clone();
    let baseline_terms = baseline_leases.clone();
    let baseline_balances = read(move || balances(&read_client, &baseline_terms)).await?;
    let domain_intent = |domain: &str, dataspace: &str, id| -> Result<_> {
        Ok(AliasIntentV1::Domain(AliasDomainIntentV1 {
            domain: ResolvedDomainV1::new(DomainId::try_new(domain, dataspace)?, id),
            owner: BOB_ID.clone(),
        }))
    };
    let (historical, historical_lease) = acquire(
        &payer,
        domain_intent("history", "universal", DataSpaceId::UNIVERSAL)?,
        SnsNamespacePath::Domain,
        "history.universal",
    )
    .await?;
    let activation = AliasRegistryRoutingActivationV1::new(
        height(authority)
            .await?
            .checked_add(24)
            .ok_or_else(|| eyre!("activation height overflow"))?,
    );
    let installed = submit(
        authority,
        transaction(
            authority,
            SetParameter::new(Parameter::Custom(activation.into_custom_parameter())),
        ),
    )
    .await?;
    let granted = submit(
        authority,
        transaction(
            authority,
            SetParameter::new(Parameter::Custom(grant.clone().into_custom_parameter()?)),
        ),
    )
    .await?;
    let deadline = Instant::now() + ADVANCE_TIMEOUT;
    for index in 0..32 {
        if height(authority).await? >= activation.activation_height {
            break;
        }
        ensure!(
            Instant::now() < deadline && index < 31,
            "future activation did not become effective within bounded real consensus progress"
        );
        // Real signed transactions, never empty height carriers. QueuePlan can
        // consume multiple carriers; do not assume one transaction = one block.
        submit(
            authority,
            transaction(
                authority,
                Log::new(Level::INFO, format!("alias-bootstrap-activation-{index}")),
            ),
        )
        .await?;
    }
    ensure!(
        height(authority).await? >= activation.activation_height,
        "routing activation not reached"
    );
    let (dataspace, dataspace_lease) = acquire(
        &payer,
        AliasIntentV1::Dataspace(AliasDataSpaceIntentV1 {
            dataspace: grant.dataspace.clone(),
            owner: BOB_ID.clone(),
        }),
        SnsNamespacePath::Dataspace,
        "bpng",
    )
    .await?;
    let (domain, domain_lease) = acquire(
        &payer,
        domain_intent("mibank", "bpng", grant.dataspace.dataspace_id)?,
        SnsNamespacePath::Domain,
        "mibank.bpng",
    )
    .await?;
    let leases = vec![historical_lease, dataspace_lease, domain_lease];
    let mut transactions = vec![historical, installed, granted, dataspace, domain];
    let expected =
        wait_for_snapshot(authority, &leases, activation, &grant, &transactions, None).await?;
    assert_paid_once(&baseline_balances, &expected.balances, &leases)?;
    let mut original_lanes = Vec::new();
    for client in &clients {
        wait_for_snapshot(
            client,
            &leases,
            activation,
            &grant,
            &transactions,
            Some(&expected),
        )
        .await?;
        original_lanes.push(observe_catalog_expansion(client, false).await?);
    }
    let initial_prefix = common_retained_prefix(&clients).await?;
    stop_all(network.peers()).await?;
    let initial_evidence = network
        .peers()
        .iter()
        .map(|peer| {
            inspect_stopped_peer(
                peer,
                Some(initial_prefix),
                &genesis,
                &original_voters,
                None,
                &expected_validator_peers,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        initial_evidence.windows(2).all(|pair| pair[0] == pair[1]),
        "four validators did not retain the same original canonical prefix"
    );
    for evidence in &initial_evidence {
        let history = &evidence.retained;
        ensure!(
            execution_height(history, &transactions[0])?
                < execution_height(history, &transactions[1])?,
            "historical alias did not precede activation installation"
        );
        ensure!(
            execution_height(history, &transactions[1])? < activation.activation_height,
            "activation was not installed at a genuinely future height"
        );
        ensure!(
            execution_height(history, &transactions[2])?
                < execution_height(history, &transactions[3])?,
            "owner grant must precede first paid dataspace lease"
        );
        ensure!(
            execution_height(history, &transactions[3])? >= activation.activation_height
                && execution_height(history, &transactions[4])? >= activation.activation_height,
            "BPNG aliases did not execute under active registry routing"
        );
        ensure!(
            evidence.certified_bpng_lane == CertifiedBpngLaneEvidence::absent(),
            "fixture-only BPNG lane must not exist before its signed lifecycle"
        );
    }
    // Preserve every original layer, including fees, lane authority and signed
    // genesis. The one new layer adds only the canonical BPNG catalog identity.
    let mut layers = network
        .config_layers()
        .map(|layer| layer.into_owned())
        .collect::<Vec<_>>();
    layers.push(dataspace_only_restart_layer(&grant));
    try_join_all(network.peers().iter().map(|peer| async {
        timeout(NETWORK_TIMEOUT, peer.start_checked(layers.iter(), None)).await??;
        Ok::<_, eyre::Report>(())
    }))
    .await?;
    for ((client, retained), original_lanes) in
        clients.iter().zip(&initial_evidence).zip(&original_lanes)
    {
        wait_for_snapshot(
            client,
            &leases,
            activation,
            &grant,
            &transactions,
            Some(&expected),
        )
        .await?;
        ensure!(
            &observe_catalog_expansion(client, true).await? == original_lanes,
            "dataspace-only restart changed a lane or its incarnation commitment"
        );
        ensure!(
            height(client).await? >= u64::try_from(retained.retained.blocks.len())?,
            "restart did not recover original retained tip"
        );
    }

    let original_lifecycle = authority.get_lane_lifecycle_status()?;
    ensure!(
        original_lifecycle.validate()? == LaneCatalog::default(),
        "dataspace-only restart must not create a lane"
    );
    let lifecycle_plan = LaneLifecyclePlan {
        additions: vec![bpng_fixture_lane()],
        retire: Vec::new(),
    };
    let lifecycle_parameter = LaneLifecycleParameterV1::new(
        &original_lifecycle.validate()?,
        &original_lifecycle.incarnations,
        lifecycle_plan,
    )?;
    let lifecycle = submit(
        authority,
        transaction(
            authority,
            SetParameter::new(Parameter::Custom(
                lifecycle_parameter.into_custom_parameter(),
            )),
        ),
    )
    .await?;
    transactions.push(lifecycle);
    let lifecycle_deadline = Instant::now() + CONVERGENCE_TIMEOUT;
    let lifecycle_status = loop {
        let status = authority.get_lane_lifecycle_status()?;
        if let Ok(incarnation) = assert_bpng_lifecycle_status(&status, &original_lifecycle) {
            break (status, incarnation);
        }
        ensure!(
            Instant::now() < lifecycle_deadline,
            "signed fixture-only BPNG lifecycle did not converge"
        );
        sleep(POLL_INTERVAL).await;
    };
    let bpng_incarnation = lifecycle_status.1;
    for client in &clients {
        let status = client.get_lane_lifecycle_status()?;
        ensure!(
            status == lifecycle_status.0
                && assert_bpng_lifecycle_status(&status, &original_lifecycle)? == bpng_incarnation,
            "signed fixture-only BPNG lifecycle did not converge exactly"
        );
    }

    let stake = SumeragiNposParameters::default().min_self_bond().clone();
    let registration_alignment_deadline = Instant::now() + ADVANCE_TIMEOUT;
    let mut registration_alignment_tick = 0_u32;
    while height(authority).await? % FIXTURE_EPOCH_LENGTH_BLOCKS != 0 {
        ensure!(
            Instant::now() < registration_alignment_deadline && registration_alignment_tick < 16,
            "could not align BPNG registrations to one exact election epoch"
        );
        submit(
            authority,
            transaction(
                authority,
                Log::new(
                    Level::INFO,
                    format!("bpng-fixture-registration-alignment-{registration_alignment_tick}"),
                ),
            ),
        )
        .await?;
        registration_alignment_tick = registration_alignment_tick.saturating_add(1);
    }
    let mut self_registrations = Vec::with_capacity(VALIDATOR_COUNT);
    for ((validator_client, peer), keypair) in validator_clients
        .iter()
        .zip(network.peers())
        .zip(&validator_keypairs)
    {
        let validator = AccountId::new(keypair.public_key().clone());
        let signed = transaction(
            validator_client,
            RegisterPublicLaneValidator::new(
                BPNG_FIXTURE_LANE,
                validator.clone(),
                peer.network_peer_id(),
                validator.clone(),
                stake.clone(),
                Metadata::default(),
            ),
        );
        ensure!(
            signed.authority() == &validator && signed.verify_signature().is_ok(),
            "BPNG validator registration must be signed by its exact validator account"
        );
        self_registrations.push(submit(validator_client, signed).await?);
    }
    transactions.extend(self_registrations);
    let activation_boundary =
        wait_for_exact_bpng_pending_registrations(authority, &expected_validator_bindings, &stake)
            .await?;
    let deadline = Instant::now() + ADVANCE_TIMEOUT;
    let mut activation_tick = 0_u32;
    while height(authority).await? < activation_boundary {
        ensure!(
            Instant::now() < deadline && activation_tick < 64,
            "BPNG validator activation boundary did not arrive"
        );
        submit(
            authority,
            transaction(
                authority,
                Log::new(
                    Level::INFO,
                    format!("bpng-fixture-validator-activation-{activation_tick}"),
                ),
            ),
        )
        .await?;
        activation_tick = activation_tick.saturating_add(1);
    }
    ensure!(
        wait_for_exact_bpng_pending_registrations(authority, &expected_validator_bindings, &stake,)
            .await?
            == activation_boundary,
        "BPNG pending set or its exact boundary changed before the governed sweep"
    );
    let sweep_validator = AccountId::new(validator_keypairs[0].public_key().clone());
    let sweep_permissions = validator_clients[0]
        .query(FindPermissionsByAccountId::new(sweep_validator.clone()))
        .execute_all()?;
    ensure!(
        sweep_permissions
            .iter()
            .any(|permission| permission == &Permission::from(CanManagePeers)),
        "BPNG activation sweep authority lacks its signed-genesis CanManagePeers grant"
    );
    // ActivatePublicLaneValidator begins with the canonical lifecycle finalizer.
    // One authorised transaction at the shared boundary therefore sweeps all
    // four eligible PendingActivation records before idempotently observing
    // its named target as Active. This is governed batch promotion evidence,
    // not evidence for four independent per-validator promotions.
    let activation_sweep = transaction(
        &validator_clients[0],
        ActivatePublicLaneValidator::new(BPNG_FIXTURE_LANE, sweep_validator.clone()),
    );
    ensure!(
        activation_sweep.authority() == &sweep_validator
            && activation_sweep.verify_signature().is_ok(),
        "BPNG activation sweep must be signed by the exact authorised manager"
    );
    transactions.push(submit(&validator_clients[0], activation_sweep).await?);
    wait_for_exact_bpng_validators(&clients, &expected_validator_bindings, &stake).await?;

    let predecessor_key: Name = "retained_bpng_predecessor".parse()?;
    let predecessor_value = Json::new("committed-before-strict-restart");
    let predecessor = submit(
        &payer,
        transaction(
            &payer,
            SetKeyValue::domain(
                DomainId::try_new("mibank", "bpng")?,
                predecessor_key.clone(),
                predecessor_value.clone(),
            ),
        ),
    )
    .await?;
    let predecessor_frontier = wait_for_bpng_frontier(
        &clients,
        bpng_incarnation,
        &expected_validator_peers,
        &predecessor,
    )
    .await?;
    transactions.push(predecessor.clone());
    for client in &clients {
        assert_bpng_metadata(client, &predecessor_key, &predecessor_value, None)?;
    }
    let before_second_restart =
        wait_for_snapshot(authority, &leases, activation, &grant, &transactions, None).await?;
    assert_paid_once(&baseline_balances, &before_second_restart.balances, &leases)?;
    let pre_restart_prefix = common_retained_prefix(&clients).await?;
    ensure!(
        pre_restart_prefix > initial_prefix,
        "BPNG predecessor did not extend the original retained prefix"
    );
    stop_all(network.peers()).await?;

    let pre_restart_evidence = network
        .peers()
        .iter()
        .map(|peer| {
            inspect_stopped_peer(
                peer,
                Some(pre_restart_prefix),
                &genesis,
                &original_voters,
                Some(bpng_incarnation),
                &expected_validator_peers,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    for ((evidence, initial), peer_index) in
        pre_restart_evidence.iter().zip(&initial_evidence).zip(0..)
    {
        ensure!(
            evidence
                .retained
                .blocks
                .starts_with(&initial.retained.blocks)
                && initial
                    .retained
                    .sidecars
                    .iter()
                    .all(|(path, bytes)| { evidence.retained.sidecars.get(path) == Some(bytes) }),
            "first strict replay changed an original carrier or sidecar on peer {peer_index}"
        );
        let (_, ownership) = bpng_transaction_ownership(
            &evidence.retained,
            &predecessor,
            bpng_incarnation,
            &expected_validator_peers,
        )?;
        ensure!(
            ownership == predecessor_frontier,
            "live and stopped-Kura BPNG predecessor ownership differ"
        );
    }
    ensure!(
        pre_restart_evidence
            .windows(2)
            .all(|pair| pair[0] == pair[1]),
        "four validators did not retain identical BPNG carriers and QCs"
    );

    // Restart two deliberately reuses the same dataspace-only operator layer.
    // Lane 8 must come exclusively from the signed lifecycle replay.
    try_join_all(network.peers().iter().map(|peer| async {
        timeout(NETWORK_TIMEOUT, peer.start_checked(layers.iter(), None)).await??;
        Ok::<_, eyre::Report>(())
    }))
    .await?;
    for (client, retained) in clients.iter().zip(&pre_restart_evidence) {
        wait_for_snapshot(
            client,
            &leases,
            activation,
            &grant,
            &transactions,
            Some(&before_second_restart),
        )
        .await?;
        let status = client.get_lane_lifecycle_status()?;
        ensure!(
            status == lifecycle_status.0
                && assert_bpng_lifecycle_status(&status, &original_lifecycle)? == bpng_incarnation,
            "strict restart did not replay exact signed BPNG lifecycle/incarnation"
        );
        ensure!(
            height(client).await? >= u64::try_from(retained.retained.blocks.len())?,
            "strict restart did not recover the BPNG predecessor carrier"
        );
        assert_bpng_metadata(client, &predecessor_key, &predecessor_value, None)?;
    }
    wait_for_exact_bpng_validators(&clients, &expected_validator_bindings, &stake).await?;
    ensure!(
        wait_for_bpng_frontier(
            &clients,
            bpng_incarnation,
            &expected_validator_peers,
            &predecessor,
        )
        .await?
            == predecessor_frontier,
        "strict restart did not recover exact BPNG predecessor ownership"
    );

    let successor_key: Name = "retained_bpng_successor".parse()?;
    let successor_value = Json::new("committed-after-strict-restart");
    let successor = submit(
        &payer,
        transaction(
            &payer,
            SetKeyValue::domain(
                DomainId::try_new("mibank", "bpng")?,
                successor_key.clone(),
                successor_value.clone(),
            ),
        ),
    )
    .await?;
    let successor_frontier = wait_for_bpng_frontier(
        &clients,
        bpng_incarnation,
        &expected_validator_peers,
        &successor,
    )
    .await?;
    ensure!(
        successor_frontier.lane_incarnation == predecessor_frontier.lane_incarnation
            && successor_frontier.lane_block_height
                == predecessor_frontier.lane_block_height.saturating_add(1)
            && successor_frontier.previous_lane_block_height
                == predecessor_frontier.lane_block_height
            && successor_frontier.previous_lane_block_descriptor_hash
                == predecessor_frontier.lane_block_descriptor_hash,
        "post-restart BPNG successor did not extend the retained predecessor/incarnation"
    );
    transactions.push(successor.clone());
    for client in &clients {
        assert_bpng_metadata(
            client,
            &predecessor_key,
            &predecessor_value,
            Some((&successor_key, &successor_value)),
        )?;
    }
    let after_successor =
        wait_for_snapshot(authority, &leases, activation, &grant, &transactions, None).await?;
    assert_paid_once(&baseline_balances, &after_successor.balances, &leases)?;
    let after_restart_prefix = common_retained_prefix(&clients).await?;
    ensure!(
        after_restart_prefix > pre_restart_prefix,
        "BPNG successor did not extend the pre-restart retained prefix"
    );
    stop_all(network.peers()).await?;

    let after_restart_evidence = network
        .peers()
        .iter()
        .map(|peer| {
            inspect_stopped_peer(
                peer,
                Some(after_restart_prefix),
                &genesis,
                &original_voters,
                Some(bpng_incarnation),
                &expected_validator_peers,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    for ((after, before), peer_index) in after_restart_evidence
        .iter()
        .zip(&pre_restart_evidence)
        .zip(0..)
    {
        ensure!(
            after.retained.blocks.starts_with(&before.retained.blocks)
                && before
                    .retained
                    .sidecars
                    .iter()
                    .all(|(path, bytes)| { after.retained.sidecars.get(path) == Some(bytes) }),
            "second strict replay changed retained carriers, sidecars or BPNG QCs on peer {peer_index}"
        );
        before
            .certified_bpng_lane
            .assert_exact_prefix_of(&after.certified_bpng_lane)?;
        ensure!(
            after.certified_bpng_lane.artifacts.len()
                == before.certified_bpng_lane.artifacts.len().saturating_add(1),
            "post-restart successor must append exactly one certified BPNG lane block"
        );
        let (predecessor_height, retained_predecessor) = bpng_transaction_ownership(
            &after.retained,
            &predecessor,
            bpng_incarnation,
            &expected_validator_peers,
        )?;
        let (successor_height, retained_successor) = bpng_transaction_ownership(
            &after.retained,
            &successor,
            bpng_incarnation,
            &expected_validator_peers,
        )?;
        ensure!(
            retained_predecessor == predecessor_frontier
                && retained_successor == successor_frontier
                && successor_height > predecessor_height,
            "stopped Kura does not retain the exact BPNG predecessor/successor chain"
        );
    }
    ensure!(
        after_restart_evidence
            .windows(2)
            .all(|pair| pair[0] == pair[1]),
        "post-restart BPNG carriers/QCs differ across validators"
    );
    Ok(())
}
