#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Localnet cross-dataspace atomic swap regression test.

use super::localnet_npos::npos_override_transactions;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Seek, SeekFrom},
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, ensure, eyre};
use futures_util::{StreamExt, future::try_join_all};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    crypto::HashOf,
    data_model::{
        ChainId, Level, ValidationFail,
        account::{Account, AccountId},
        asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
        block::consensus::{
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION, SumeragiCommittedLaneBlock,
            SumeragiDiagnosticsStatus, SumeragiLanePayloadOwnership,
            SumeragiNativeAmxParticipantApplication, SumeragiNativeAmxParticipantApplicationState,
            committed_lane_block_status_counts_as_progress,
        },
        block::consensus_v2::{HeightContext, SumeragiV2Status},
        bridge::{BridgeFinalityProof, verify_bridge_finality_proof},
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        da::commitment::DaProofPolicyBundle,
        domain::{Domain, DomainId},
        events::{
            EventBox,
            pipeline::{PipelineEventBox, TransactionEventFilter, TransactionStatus},
        },
        isi::{
            Grant, InstructionBox, Log, Mint, Register, Transfer,
            settlement::{
                DvpIsi, SettlementAtomicity, SettlementExecutionOrder, SettlementLeg,
                SettlementPlan,
            },
            staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
        },
        merge::{LaneDrainCertificateV1, MAX_MERGE_LEDGER_ENTRY_BYTES, MergeLedgerEntry},
        metadata::Metadata,
        nexus::{DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneVisibility},
        peer::PeerId,
        permission::Permission,
        prelude::{FindAssetById, FindAssets, FindPermissionsByAccountId, Quantity},
        query::block::prelude::FindBlocks,
        transaction::{SignedTransaction, TransactionEntrypoint},
    },
    query::QueryError,
};
use iroha_config::{
    kura::{FsyncMode, InitMode},
    parameters::{
        actual::{Kura as KuraConfig, LaneConfig as ActualLaneConfig},
        defaults,
    },
};
use iroha_config_base::WithOrigin;
use iroha_core::{
    da::proof_policy_bundle,
    kura::Kura,
    merge::{MergeLedgerCandidate, merge_chain_id_digest, merge_qc_message_digest},
    sumeragi::network_topology::commit_quorum_from_len,
};
use iroha_crypto::{Algorithm, Hash, KeyPair, PrivateKey};
use iroha_data_model::{
    prelude::QueryBuilderExt,
    query::{
        CommittedTxFilters,
        dsl::CompoundPredicate,
        error::{FindError, QueryExecutionFail},
        parameters::{FetchSize, Pagination},
        transaction::prelude::FindTransactions,
    },
};
use iroha_executor_data_model::permission::asset::CanTransferAssetWithDefinition;
use iroha_executor_data_model::permission::settlement::CanExecuteSettlement;
use iroha_test_network::{NetworkBuilder, NetworkPeer, genesis_factory_with_post_topology};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};
use norito::codec::{DecodeAll, Encode};
use norito::json::Value as JsonValue;
use tokio::{
    runtime::Runtime,
    task::spawn_blocking,
    time::{sleep, timeout},
};
use toml::{Table, Value as TomlValue};

const NEXUS_ALIAS: &str = "universal";
const DS1_ALIAS: &str = "ds1";
const DS2_ALIAS: &str = "ds2";
const NEXUS_ID_U64: u64 = 0;
const DS1_ID_U64: u64 = 1;
const DS2_ID_U64: u64 = 2;
const DS1_MANIFEST_HASH: &str = "0100000000000000000000000000000000000000000000000000000000000000";
const DS2_MANIFEST_HASH: &str = "0200000000000000000000000000000000000000000000000000000000000000";
const NEXUS_LANE_INDEX: u32 = 0;
const DS1_LANE_INDEX: u32 = 1;
const DS2_LANE_INDEX: u32 = 2;
const AUTOSCALE_LANE_INDEX: u32 = 3;
const TOTAL_PEERS: usize = 12;
const VALIDATORS_PER_LANE: usize = 4;
const VALIDATOR_STAKE: u64 = 2_000;
const NEXUS_FEE_SEED_AMOUNT: u32 = 1_000_000;
const DS1_WORKLOAD_SEED_AMOUNT: u64 = 5_000;
const DS2_WORKLOAD_SEED_AMOUNT: u64 = 5_000;
const STATUS_WAIT_TIMEOUT: Duration = Duration::from_secs(45);
const LANE_PROGRESS_WAIT_TIMEOUT: Duration = Duration::from_secs(180);
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(200);
const ROUTE_PROBE_APPROVAL_WAIT_TIMEOUT: Duration = Duration::from_millis(100);
const ROUTE_PROBE_SSE_HANDSHAKE_DELAY: Duration = Duration::from_millis(100);
const SETUP_BARRIER_TICK_EVERY_POLLS: u64 = 5;
const LANE_PROGRESS_RECOVERY_TICK_EVERY_POLLS: u64 = 25;
const OBSERVER_QUERY_TIMEOUT_CAP: Duration = Duration::from_secs(12);
const OBSERVER_QUERY_TIMEOUT_FLOOR: Duration = Duration::from_secs(2);
const PERMISSION_VISIBILITY_WAIT_TIMEOUT: Duration = Duration::from_secs(90);
const SETUP_REGISTER_MINT_QUERY_TIMEOUT: Duration = Duration::from_secs(20);
const SUBMIT_ENQUEUE_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const BLOCKING_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(20);
const ROLLBACK_CAPPED_ATTEMPTS: usize = 2;
const ROLLBACK_HISTORY_RETRY_TIMEOUT: Duration = Duration::from_secs(4);
const ROLLBACK_HISTORY_FALLBACK_TIMEOUT: Duration = Duration::from_secs(25);
const SWAP_COMMITTED_OUTCOME_TIMEOUT: Duration = Duration::from_secs(8);
const SWAP_POST_BARRIER_OUTCOME_TIMEOUT: Duration = Duration::from_secs(6);
const SWAP_NONCONVERGED_FALLBACK_MAX: usize = 2;
const SOAK_PHASE_WAIT_TIMEOUT: Duration = Duration::from_secs(32);
const NETWORK_BASE_SEED_ENV: &str = "IROHA_TEST_NETWORK_BASE_SEED";
const REQUIRE_CORRIDOR_SEED_ENV: &str = "IROHA_NEXUS_CROSS_REQUIRE_SEED";
const CORRIDOR_SEED_PREFIX: &str = "nexus-cross-dataspace-v1-seed-";
const DEFAULT_CORRIDOR_SEED: &str = "nexus-cross-dataspace-v1-seed-00";
const CORRIDOR_SEED_COUNT: usize = 10;
const FAULT_SOAK_DURATION_ENV: &str = "IROHA_NEXUS_CROSS_FAULT_SOAK_DURATION_SECS";
const FAULT_SOAK_DURATION_SECS: u64 = 2 * 60 * 60;
const CROSS_DATASPACE_LOCALNET_STACK_BYTES: usize = 32 * 1024 * 1024;
const AUTOSCALE_BASE_LANE_COUNT: usize = 3;
const AUTOSCALE_EXPANDED_LANE_COUNT: usize = 4;
// Default-dataspace sharding candidates are lane 0 and managed elastic lane 3;
// restricted lanes 1 and 2 are not members of this routing set.
const AUTOSCALE_DEFAULT_ROUTE_SHARD_COUNT: u64 = 2;
const AUTOSCALE_DEFAULT_ROUTE_TARGET_SHARD: u64 = 1;
const AUTOSCALE_LOAD_TX_COUNT: usize = 256;
const AUTOSCALE_SCALE_OUT_WAIT_TIMEOUT: Duration = Duration::from_secs(180);
const AUTOSCALE_SCALE_IN_WAIT_TIMEOUT: Duration = Duration::from_secs(240);
const AUTOSCALE_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(1);
const AUTOSCALE_LOG_TAIL_MAX_BYTES: u64 = 4 * 1024 * 1024;
const AUTOSCALE_COPY_MAX_ENTRIES: usize = 65_536;
const AUTOSCALE_COPY_MAX_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const AUTOSCALE_SCALE_OUT_LOG_MARKER: &str =
    "applied deterministic lane autoscale scale-out transition";
const AUTOSCALE_SCALE_IN_LOG_MARKER: &str =
    "applied deterministic lane autoscale scale-in transition";
const AUTOSCALE_DRAIN_INTENT_LOG_MARKER: &str =
    "committed deterministic lane autoscale drain intent";
const AUTOSCALE_DRAIN_COMMITMENT_LOG_MARKER: &str =
    "committed globally certified lane autoscale drain frontier";
const LANE_INCARNATION_MARKER_VERSION: u8 = 3;
const LANE_INCARNATION_MARKER_FILE: &str = ".lane-incarnation.norito";

#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
#[norito(deny_unknown_fields)]
struct LaneIncarnationMarkerV3 {
    version: u8,
    lane_id: LaneId,
    incarnation: Hash,
    activation_height: u64,
    move_target_blocks: Option<String>,
    move_target_merge: Option<String>,
    block_store_digest: Hash,
    merge_log_digest: Hash,
}

#[derive(Clone)]
struct ConfigLayer(Table);

impl AsRef<Table> for ConfigLayer {
    fn as_ref(&self) -> &Table {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CorridorSeed {
    value: String,
    ordinal: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CorridorRunMode {
    SeedCase,
    FaultSoak { duration: Duration },
}

fn stake_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("nexus", "universal").expect("nexus domain"),
        "xor".parse().expect("stake asset name"),
    )
}

fn stake_asset_id_literal() -> String {
    stake_asset_definition_id().to_string()
}

fn nexus_fee_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::new(
        DomainId::try_new("universal", "universal").expect("fee asset domain"),
        "xor".parse().expect("fee asset name"),
    )
}

fn parse_corridor_seed(raw: &str) -> Result<CorridorSeed> {
    ensure!(
        raw.len() == CORRIDOR_SEED_PREFIX.len() + 2 && raw.starts_with(CORRIDOR_SEED_PREFIX),
        "{NETWORK_BASE_SEED_ENV} must match {CORRIDOR_SEED_PREFIX}NN"
    );
    let suffix = &raw[CORRIDOR_SEED_PREFIX.len()..];
    ensure!(
        suffix.as_bytes().iter().all(|byte| byte.is_ascii_digit()),
        "{NETWORK_BASE_SEED_ENV} seed ordinal must contain exactly two ASCII digits"
    );
    let ordinal = suffix
        .parse::<usize>()
        .map_err(|err| eyre!("{NETWORK_BASE_SEED_ENV} has invalid seed ordinal: {err}"))?;
    ensure!(
        ordinal < CORRIDOR_SEED_COUNT,
        "{NETWORK_BASE_SEED_ENV} seed ordinal must be in 00..09, got {suffix}"
    );
    Ok(CorridorSeed {
        value: raw.to_owned(),
        ordinal,
    })
}

fn parse_required_seed_flag(raw: Option<&str>) -> Result<bool> {
    match raw {
        None | Some("0") => Ok(false),
        Some("1") => Ok(true),
        Some(value) => Err(eyre!(
            "{REQUIRE_CORRIDOR_SEED_ENV} must be exactly 0 or 1, got {value:?}"
        )),
    }
}

fn corridor_seed_from_env() -> Result<CorridorSeed> {
    let require_seed = match std::env::var(REQUIRE_CORRIDOR_SEED_ENV) {
        Ok(raw) => parse_required_seed_flag(Some(&raw))?,
        Err(std::env::VarError::NotPresent) => parse_required_seed_flag(None)?,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(eyre!(
                "{REQUIRE_CORRIDOR_SEED_ENV} must contain valid Unicode"
            ));
        }
    };
    match std::env::var(NETWORK_BASE_SEED_ENV) {
        Ok(raw) => parse_corridor_seed(&raw),
        Err(std::env::VarError::NotPresent) if !require_seed => {
            parse_corridor_seed(DEFAULT_CORRIDOR_SEED)
        }
        Err(std::env::VarError::NotPresent) => Err(eyre!(
            "{NETWORK_BASE_SEED_ENV} is required when {REQUIRE_CORRIDOR_SEED_ENV}=1"
        )),
        Err(std::env::VarError::NotUnicode(_)) => Err(eyre!(
            "{NETWORK_BASE_SEED_ENV} must contain valid Unicode matching the corridor seed format"
        )),
    }
}

fn parse_fault_soak_duration(raw: Option<&str>) -> Result<Duration> {
    let seconds = match raw {
        None => FAULT_SOAK_DURATION_SECS,
        Some(raw) => raw.parse::<u64>().map_err(|err| {
            eyre!("{FAULT_SOAK_DURATION_ENV} must be the integer {FAULT_SOAK_DURATION_SECS}: {err}")
        })?,
    };
    ensure!(
        seconds == FAULT_SOAK_DURATION_SECS,
        "{FAULT_SOAK_DURATION_ENV} must be exactly {FAULT_SOAK_DURATION_SECS} seconds, got {seconds}"
    );
    Ok(Duration::from_secs(seconds))
}

fn fault_soak_duration_from_env() -> Result<Duration> {
    match std::env::var(FAULT_SOAK_DURATION_ENV) {
        Ok(raw) => parse_fault_soak_duration(Some(&raw)),
        Err(std::env::VarError::NotPresent) => parse_fault_soak_duration(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(eyre!(
            "{FAULT_SOAK_DURATION_ENV} must contain valid Unicode"
        )),
    }
}

fn cross_dataspace_gas_account_id() -> AccountId {
    // Use an existing single-domain subject to keep staking literals unambiguous.
    ALICE_ID.clone()
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ExpectedLaneValidatorBinding {
    validator: String,
    peer_id: String,
}

fn validator_authority_account_for_peer(index: usize) -> AccountId {
    let keypair = validator_authority_keypair(index);
    AccountId::new(keypair.public_key().clone())
}

fn validator_authority_keypair(index: usize) -> KeyPair {
    KeyPair::try_from_seed(validator_authority_seed(index), Algorithm::Ed25519)
        .expect("fixture cross-dataspace validator authority key")
}

fn validator_authority_seed(index: usize) -> Vec<u8> {
    let mut seed = vec![0_u8; 32];
    seed[0] = 0xC1;
    seed[1..9].copy_from_slice(&u64::try_from(index).unwrap_or(u64::MAX).to_le_bytes());
    seed
}

fn expected_lane_binding_for_peer(index: usize, peer_id: &PeerId) -> ExpectedLaneValidatorBinding {
    ExpectedLaneValidatorBinding {
        validator: validator_authority_account_for_peer(index).to_string(),
        peer_id: peer_id.to_string(),
    }
}

fn localnet_builder(seed: &str) -> NetworkBuilder {
    let gas_account_str = cross_dataspace_gas_account_id()
        .canonical_i105()
        .expect("canonical I105 escrow account literal");
    NetworkBuilder::new()
        .with_base_seed(seed)
        .with_peers(TOTAL_PEERS)
        .without_npos_genesis_bootstrap()
        .with_genesis_block(|topology, topology_entries| {
            let post_topology =
                npos_multilane_genesis_post_topology_transactions(topology.as_ref());
            let mut genesis = genesis_factory_with_post_topology(
                npos_override_transactions(VALIDATORS_PER_LANE),
                post_topology,
                topology,
                topology_entries,
            );
            genesis
                .0
                .set_da_proof_policies(Some(multilane_da_proof_policy_bundle()));
            genesis
        })
        .with_config_layer(move |layer| {
            let mut lane_nexus = Table::new();
            lane_nexus.insert("index".into(), TomlValue::Integer(0));
            lane_nexus.insert("alias".into(), TomlValue::String("lane-nexus".to_owned()));
            lane_nexus.insert(
                "dataspace".into(),
                TomlValue::String(NEXUS_ALIAS.to_owned()),
            );
            lane_nexus.insert("visibility".into(), TomlValue::String("public".to_owned()));
            lane_nexus.insert("metadata".into(), TomlValue::Table(Table::new()));

            let mut lane_ds1 = Table::new();
            lane_ds1.insert("index".into(), TomlValue::Integer(1));
            lane_ds1.insert("alias".into(), TomlValue::String("lane-ds1".to_owned()));
            lane_ds1.insert("dataspace".into(), TomlValue::String(DS1_ALIAS.to_owned()));
            lane_ds1.insert(
                "visibility".into(),
                TomlValue::String("restricted".to_owned()),
            );
            lane_ds1.insert("metadata".into(), TomlValue::Table(Table::new()));

            let mut lane_ds2 = Table::new();
            lane_ds2.insert("index".into(), TomlValue::Integer(2));
            lane_ds2.insert("alias".into(), TomlValue::String("lane-ds2".to_owned()));
            lane_ds2.insert("dataspace".into(), TomlValue::String(DS2_ALIAS.to_owned()));
            lane_ds2.insert(
                "visibility".into(),
                TomlValue::String("restricted".to_owned()),
            );
            lane_ds2.insert("metadata".into(), TomlValue::Table(Table::new()));

            let mut ds_nexus = Table::new();
            ds_nexus.insert("alias".into(), TomlValue::String(NEXUS_ALIAS.to_owned()));
            ds_nexus.insert("id".into(), TomlValue::Integer(NEXUS_ID_U64 as i64));
            ds_nexus.insert(
                "description".into(),
                TomlValue::String("main nexus dataspace".to_owned()),
            );
            ds_nexus.insert("fault_tolerance".into(), TomlValue::Integer(1));

            let mut ds1 = Table::new();
            ds1.insert("alias".into(), TomlValue::String(DS1_ALIAS.to_owned()));
            ds1.insert("id".into(), TomlValue::Integer(DS1_ID_U64 as i64));
            ds1.insert(
                "manifest_hash".into(),
                TomlValue::String(DS1_MANIFEST_HASH.to_owned()),
            );
            ds1.insert(
                "description".into(),
                TomlValue::String("private dataspace one".to_owned()),
            );
            ds1.insert("fault_tolerance".into(), TomlValue::Integer(1));

            let mut ds2 = Table::new();
            ds2.insert("alias".into(), TomlValue::String(DS2_ALIAS.to_owned()));
            ds2.insert("id".into(), TomlValue::Integer(DS2_ID_U64 as i64));
            ds2.insert(
                "manifest_hash".into(),
                TomlValue::String(DS2_MANIFEST_HASH.to_owned()),
            );
            ds2.insert(
                "description".into(),
                TomlValue::String("private dataspace two".to_owned()),
            );
            ds2.insert("fault_tolerance".into(), TomlValue::Integer(1));

            let mut matcher_alice = Table::new();
            matcher_alice.insert("account".into(), TomlValue::String(ALICE_ID.to_string()));
            let mut rule_alice = Table::new();
            rule_alice.insert("lane".into(), TomlValue::Integer(1));
            rule_alice.insert("dataspace".into(), TomlValue::String(DS1_ALIAS.to_owned()));
            rule_alice.insert("matcher".into(), TomlValue::Table(matcher_alice));

            let mut matcher_bob = Table::new();
            matcher_bob.insert("account".into(), TomlValue::String(BOB_ID.to_string()));
            let mut rule_bob = Table::new();
            rule_bob.insert("lane".into(), TomlValue::Integer(2));
            rule_bob.insert("dataspace".into(), TomlValue::String(DS2_ALIAS.to_owned()));
            rule_bob.insert("matcher".into(), TomlValue::Table(matcher_bob));

            let mut policy = Table::new();
            policy.insert("default_lane".into(), TomlValue::Integer(0));
            policy.insert(
                "default_dataspace".into(),
                TomlValue::String(NEXUS_ALIAS.to_owned()),
            );
            policy.insert(
                "rules".into(),
                TomlValue::Array(vec![
                    TomlValue::Table(rule_alice),
                    TomlValue::Table(rule_bob),
                ]),
            );

            layer
                .write(["nexus", "enabled"], true)
                .write(["nexus", "lane_count"], 3_i64)
                .write(["nexus", "autoscale", "enabled"], true)
                .write(
                    ["nexus", "autoscale", "min_lanes"],
                    i64::from(AUTOSCALE_LANE_INDEX),
                )
                .write(
                    ["nexus", "autoscale", "max_lanes"],
                    i64::from(AUTOSCALE_LANE_INDEX + 1),
                )
                .write(["nexus", "autoscale", "target_block_ms"], 120_000_i64)
                .write(["nexus", "autoscale", "scale_out_latency_ratio"], 1.20_f64)
                .write(["nexus", "autoscale", "scale_in_latency_ratio"], 0.80_f64)
                .write(
                    ["nexus", "autoscale", "scale_out_utilization_ratio"],
                    0.25_f64,
                )
                .write(
                    ["nexus", "autoscale", "scale_in_utilization_ratio"],
                    0.05_f64,
                )
                .write(["nexus", "autoscale", "scale_out_window_blocks"], 2_i64)
                .write(["nexus", "autoscale", "scale_in_window_blocks"], 4_i64)
                .write(["nexus", "autoscale", "cooldown_blocks"], 1_i64)
                .write(["nexus", "autoscale", "per_lane_target_tps"], 100_i64)
                .write(["norito", "allow_gpu_compression"], false)
                .write(
                    ["nexus", "lane_catalog"],
                    TomlValue::Array(vec![
                        TomlValue::Table(lane_nexus),
                        TomlValue::Table(lane_ds1),
                        TomlValue::Table(lane_ds2),
                    ]),
                )
                .write(
                    ["nexus", "dataspace_catalog"],
                    TomlValue::Array(vec![
                        TomlValue::Table(ds_nexus),
                        TomlValue::Table(ds1),
                        TomlValue::Table(ds2),
                    ]),
                )
                .write(["nexus", "routing_policy"], TomlValue::Table(policy))
                .write(
                    ["nexus", "staking", "restricted_validator_mode"],
                    "stake_elected",
                )
                .write(
                    ["nexus", "staking", "public_validator_mode"],
                    "stake_elected",
                )
                .write(
                    ["nexus", "staking", "stake_asset_id"],
                    stake_asset_id_literal(),
                )
                .write(
                    ["nexus", "staking", "stake_escrow_account_id"],
                    gas_account_str.clone(),
                )
                .write(
                    ["nexus", "staking", "slash_sink_account_id"],
                    gas_account_str.clone(),
                )
                .write(
                    ["nexus", "staking", "max_validators"],
                    VALIDATORS_PER_LANE as i64,
                );
        })
}

fn multilane_lane_catalog() -> LaneCatalog {
    let lane_count = NonZeroU32::new(3).expect("lane count");
    let lanes = vec![
        ModelLaneConfig {
            id: LaneId::new(NEXUS_LANE_INDEX),
            dataspace_id: DataSpaceId::new(NEXUS_ID_U64),
            alias: "lane-nexus".to_owned(),
            visibility: LaneVisibility::Public,
            ..ModelLaneConfig::default()
        },
        ModelLaneConfig {
            id: LaneId::new(DS1_LANE_INDEX),
            dataspace_id: DataSpaceId::new(DS1_ID_U64),
            alias: "lane-ds1".to_owned(),
            visibility: LaneVisibility::Restricted,
            ..ModelLaneConfig::default()
        },
        ModelLaneConfig {
            id: LaneId::new(DS2_LANE_INDEX),
            dataspace_id: DataSpaceId::new(DS2_ID_U64),
            alias: "lane-ds2".to_owned(),
            visibility: LaneVisibility::Restricted,
            ..ModelLaneConfig::default()
        },
    ];
    LaneCatalog::new(lane_count, lanes).expect("lane catalog")
}

fn multilane_da_proof_policy_bundle() -> DaProofPolicyBundle {
    let catalog = multilane_lane_catalog();
    let lane_config = ActualLaneConfig::from_catalog(&catalog);
    proof_policy_bundle(&lane_config)
}

fn npos_multilane_genesis_post_topology_transactions(
    topology: &[PeerId],
) -> Vec<Vec<InstructionBox>> {
    assert_eq!(
        topology.len(),
        TOTAL_PEERS,
        "expected {TOTAL_PEERS} peers in genesis topology, got {}",
        topology.len()
    );
    let nexus_domain: DomainId = DomainId::try_new("nexus", "universal").expect("nexus domain");
    let universal_domain: DomainId =
        DomainId::try_new("universal", "universal").expect("universal domain");
    let ds1_domain: DomainId = DomainId::try_new("ds1", "universal").expect("ds1 domain");
    let ds2_domain: DomainId = DomainId::try_new("ds2", "universal").expect("ds2 domain");
    let stake_asset_id = stake_asset_definition_id();
    let fee_asset_id = nexus_fee_asset_definition_id();
    let ds1_asset_def: AssetDefinitionId = AssetDefinitionId::new(
        DomainId::try_new("nexus", "universal").expect("asset definition domain"),
        "ds1coin".parse().expect("asset definition name"),
    );
    let ds2_asset_def: AssetDefinitionId = AssetDefinitionId::new(
        DomainId::try_new("nexus", "universal").expect("asset definition domain"),
        "ds2coin".parse().expect("asset definition name"),
    );
    let mut bootstrap_tx = vec![
        Register::domain(Domain::new(nexus_domain.clone())).into(),
        Register::domain(Domain::new(universal_domain)).into(),
        Register::domain(Domain::new(ds1_domain)).into(),
        Register::domain(Domain::new(ds2_domain)).into(),
        Register::asset_definition({
            let __asset_definition_id = stake_asset_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .into(),
        Register::asset_definition({
            let __asset_definition_id = fee_asset_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .into(),
        Register::asset_definition({
            let __asset_definition_id = ds1_asset_def.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .into(),
        Register::asset_definition({
            let __asset_definition_id = ds2_asset_def.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        })
        .into(),
        Mint::asset_quantity(
            DS1_WORKLOAD_SEED_AMOUNT,
            AssetId::new(ds1_asset_def.clone(), ALICE_ID.clone()),
        )
        .into(),
        Mint::asset_quantity(
            NEXUS_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), ALICE_ID.clone()),
        )
        .into(),
        Mint::asset_quantity(
            NEXUS_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), BOB_ID.clone()),
        )
        .into(),
        Mint::asset_quantity(
            DS2_WORKLOAD_SEED_AMOUNT,
            AssetId::new(ds2_asset_def, BOB_ID.clone()),
        )
        .into(),
    ];

    for (index, peer) in topology.iter().enumerate() {
        let lane_index = if index < VALIDATORS_PER_LANE {
            NEXUS_LANE_INDEX
        } else if index < VALIDATORS_PER_LANE * 2 {
            DS1_LANE_INDEX
        } else {
            DS2_LANE_INDEX
        };
        let lane_id = LaneId::new(lane_index);
        let validator_id = validator_authority_account_for_peer(index);
        bootstrap_tx.push(Register::account(Account::new(validator_id.clone())).into());
        if lane_index == NEXUS_LANE_INDEX {
            bootstrap_tx.push(
                Mint::asset_quantity(
                    NEXUS_FEE_SEED_AMOUNT,
                    AssetId::new(fee_asset_id.clone(), validator_id.clone()),
                )
                .into(),
            );
        }
        bootstrap_tx.push(
            Mint::asset_quantity(
                VALIDATOR_STAKE,
                AssetId::new(stake_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        bootstrap_tx.push(
            Mint::asset_quantity(
                NEXUS_FEE_SEED_AMOUNT,
                AssetId::new(fee_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        bootstrap_tx.push(
            RegisterPublicLaneValidator::new(
                lane_id,
                validator_id.clone(),
                peer.clone(),
                validator_id.clone(),
                Quantity::from(VALIDATOR_STAKE),
                Metadata::default(),
            )
            .into(),
        );
        bootstrap_tx.push(ActivatePublicLaneValidator::new(lane_id, validator_id).into());
    }

    vec![bootstrap_tx]
}

#[derive(Clone, Debug)]
struct SumeragiObservation {
    canonical: SumeragiV2Status,
    diagnostics: SumeragiDiagnosticsStatus,
}

impl std::ops::Deref for SumeragiObservation {
    type Target = SumeragiDiagnosticsStatus;

    fn deref(&self) -> &Self::Target {
        &self.diagnostics
    }
}

fn sumeragi_observation(client: &Client) -> Result<SumeragiObservation> {
    Ok(SumeragiObservation {
        canonical: client.get_sumeragi_status()?,
        diagnostics: client.get_sumeragi_diagnostics()?,
    })
}

fn wait_for_height(
    client: &Client,
    target_height: u64,
    context: &str,
) -> Result<SumeragiObservation> {
    wait_for_height_with_timeout(client, target_height, context, STATUS_WAIT_TIMEOUT)
}

fn wait_for_height_with_timeout(
    client: &Client,
    target_height: u64,
    context: &str,
    timeout_duration: Duration,
) -> Result<SumeragiObservation> {
    let started = Instant::now();
    let mut last_height = 0;
    let mut last_error: Option<String> = None;
    while started.elapsed() <= timeout_duration {
        match sumeragi_observation(client) {
            Ok(status) => {
                last_height = status.canonical.last_committed_height;
                if status.canonical.last_committed_height >= target_height {
                    return Ok(status);
                }
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    let suffix = last_error
        .map(|err| format!("; last status query error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for block height >= {target_height}; last observed {last_height}{suffix}"
    ))
}

fn wait_for_height_with_tick_submitters_timeout_across_clients(
    mut clients_factory: impl FnMut() -> Vec<Client>,
    tick_submitters: Option<&[Client]>,
    target_height: u64,
    context: &str,
    timeout_duration: Duration,
    tick_every_polls: u64,
) -> Result<SumeragiObservation> {
    let started = Instant::now();
    let mut last_height = 0;
    let mut last_error: Option<String> = None;
    let mut poll_count = 0;
    while started.elapsed() <= timeout_duration {
        let mut best_status = None;
        let clients = clients_factory();
        if should_submit_tick(poll_count, tick_every_polls) {
            let tick_clients = tick_submitters.unwrap_or(clients.as_slice());
            if !tick_clients.is_empty() {
                submit_wait_ticks(
                    tick_clients,
                    context,
                    poll_count,
                    started,
                    timeout_duration,
                    &mut last_error,
                );
            }
        }
        for client in clients {
            match sumeragi_observation(&client) {
                Ok(status) => {
                    last_height = last_height.max(status.canonical.last_committed_height);
                    if best_status
                        .as_ref()
                        .is_none_or(|best: &SumeragiObservation| {
                            status.canonical.last_committed_height
                                >= best.canonical.last_committed_height
                        })
                    {
                        best_status = Some(status);
                    }
                }
                Err(err) => {
                    last_error = Some(err.to_string());
                }
            }
        }
        if let Some(status) = best_status
            && status.canonical.last_committed_height >= target_height
        {
            return Ok(status);
        }
        poll_count = poll_count.saturating_add(1);
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    let suffix = last_error
        .map(|err| format!("; last status query error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for block height >= {target_height}; last observed {last_height}{suffix}"
    ))
}

fn wait_for_lane_peers_commit_qc_at_least(
    network: &sandbox::SerializedNetwork,
    lane_index: u32,
    expected_status: &SumeragiObservation,
    tick_submitter: &Client,
    context: &str,
    timeout_duration: Duration,
    tick_every_polls: u64,
) -> Result<()> {
    let start = lane_index as usize * VALIDATORS_PER_LANE;
    let end = start
        .saturating_add(VALIDATORS_PER_LANE)
        .min(network.peers().len());
    let peers = &network.peers()[start..end];
    let expected_height = expected_status.canonical.last_committed_height;
    let expected_hash = expected_status
        .canonical
        .last_committed_subject
        .map(|subject| subject.block_hash);

    let started = Instant::now();
    let mut last_observed = Vec::with_capacity(peers.len());
    let mut last_error: Option<String> = None;
    let mut poll_count = 0;
    while started.elapsed() <= timeout_duration {
        if should_submit_tick(poll_count, tick_every_polls) {
            submit_wait_tick(
                tick_submitter,
                context,
                poll_count,
                started,
                timeout_duration,
                &mut last_error,
            );
        }
        last_observed.clear();
        let mut all_match = true;
        for peer in peers {
            match sumeragi_observation(&peer.client()) {
                Ok(status) => {
                    let observed_height = status.canonical.last_committed_height;
                    let observed_hash = status
                        .canonical
                        .last_committed_subject
                        .map(|subject| subject.block_hash);
                    let converged = observed_height > expected_height
                        || (observed_height == expected_height && observed_hash == expected_hash);
                    if !converged {
                        all_match = false;
                    }
                    last_observed.push(format!(
                        "{}@h{}:{:?}",
                        peer.id(),
                        observed_height,
                        observed_hash
                    ));
                }
                Err(err) => {
                    last_error = Some(err.to_string());
                    all_match = false;
                    break;
                }
            }
        }
        if all_match {
            return Ok(());
        }
        poll_count = poll_count.saturating_add(1);
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    let suffix = last_error
        .map(|err| format!("; last status query error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for lane {lane_index} peers to converge on commit QC at least h{expected_height}:{expected_hash:?}; last observed {last_observed:?}{suffix}"
    ))
}

fn asset_balance(client: &Client, asset_id: &AssetId) -> Result<Quantity> {
    match client.query_single(FindAssetById::new(asset_id.clone())) {
        Ok(asset) => Ok(asset.value().clone()),
        Err(QueryError::Validation(ValidationFail::QueryFailed(
            QueryExecutionFail::Find(FindError::Asset(_)) | QueryExecutionFail::NotFound,
        ))) => Ok(Quantity::zero()),
        Err(err) => Err(eyre!(err)),
    }
}

fn asset_balance_variants(client: &Client, asset_id: &AssetId) -> Result<Vec<Quantity>> {
    Ok(vec![asset_balance(client, asset_id)?])
}

fn render_error_with_debug(err: &(impl core::fmt::Display + core::fmt::Debug)) -> String {
    format!("{err} ({err:?})")
}

fn bounded_observer_request_timeout(
    started: Instant,
    overall_timeout: Duration,
    remaining_clients: usize,
) -> Duration {
    let remaining = overall_timeout.saturating_sub(started.elapsed());
    if remaining.is_zero() {
        return Duration::from_millis(1);
    }
    let divisor = u32::try_from(remaining_clients.max(1)).unwrap_or(u32::MAX);
    let slice = remaining.checked_div(divisor).unwrap_or(remaining);
    slice
        .min(OBSERVER_QUERY_TIMEOUT_CAP)
        .max(OBSERVER_QUERY_TIMEOUT_FLOOR)
        .min(remaining)
}

fn should_submit_tick(poll_count: u64, tick_every_polls: u64) -> bool {
    tick_every_polls > 0 && poll_count % tick_every_polls == 0
}

fn peer_indices_for_committed_lane_evidence(peer_count: usize) -> Vec<usize> {
    // Committed-lane rows are self-certified by the embedded lane QC. Do not
    // require a specific telemetry fanout peer to observe the same safety proof.
    (0..peer_count).collect()
}

fn submit_wait_tick(
    tick_submitter: &Client,
    context: &str,
    poll_count: u64,
    started: Instant,
    timeout_duration: Duration,
    last_error: &mut Option<String>,
) {
    let mut tick_client = tick_submitter.clone();
    tick_client.torii_request_timeout =
        tick_client
            .torii_request_timeout
            .min(bounded_observer_request_timeout(
                started,
                timeout_duration,
                1,
            ));
    let message = format!("{context} tick {poll_count}");
    if let Err(err) = tick_client.submit(
        Log::new(Level::INFO, message),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    ) {
        *last_error = Some(format!(
            "tick submit error: {}",
            render_error_with_debug(&err)
        ));
    }
}

fn submit_wait_ticks(
    tick_submitters: &[Client],
    context: &str,
    poll_count: u64,
    started: Instant,
    timeout_duration: Duration,
    last_error: &mut Option<String>,
) {
    if tick_submitters.is_empty() {
        return;
    }
    for tick_submitter in tick_submitters {
        submit_wait_tick(
            tick_submitter,
            context,
            poll_count,
            started,
            timeout_duration,
            last_error,
        );
    }
}

#[test]
fn bounded_observer_request_timeout_slices_remaining_budget() {
    let timeout = bounded_observer_request_timeout(Instant::now(), STATUS_WAIT_TIMEOUT, 4);
    assert!(
        timeout <= OBSERVER_QUERY_TIMEOUT_CAP,
        "observer timeout should respect the per-peer cap"
    );
    assert!(
        timeout >= OBSERVER_QUERY_TIMEOUT_FLOOR,
        "observer timeout should keep a minimum per-peer slice"
    );
    assert!(
        timeout <= STATUS_WAIT_TIMEOUT,
        "observer timeout should not exceed the remaining outer budget"
    );
}

#[derive(Debug)]
struct RoutedJsonGetResponse {
    body: JsonValue,
    routed_by: Option<String>,
    route_lane_id: Option<String>,
    route_dataspace_id: Option<String>,
}

fn routed_header_string(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn add_client_headers(
    client: &Client,
    mut request: reqwest::RequestBuilder,
) -> reqwest::RequestBuilder {
    for (name, value) in &client.headers {
        request = request.header(name, value);
    }
    request
}

async fn torii_json_get(
    client: &Client,
    path_segments: &[String],
    query_pairs: &[(String, String)],
) -> Result<RoutedJsonGetResponse> {
    let mut url = client.torii_url.clone();
    let torii_url_literal = url.to_string();
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| eyre!("torii URL `{torii_url_literal}` cannot accept path segments"))?;
        segments.pop_if_empty();
        for segment in path_segments {
            segments.push(segment);
        }
    }
    if !query_pairs.is_empty() {
        let mut query = url.query_pairs_mut();
        for (key, value) in query_pairs {
            query.append_pair(key, value);
        }
    }

    let request = integration_tests::http::client()
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json");
    let response = add_client_headers(client, request).send().await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await?;
    if status != reqwest::StatusCode::OK {
        return Err(eyre!(
            "Torii GET failed with status {}: {}",
            status,
            String::from_utf8_lossy(&body)
        ));
    }

    Ok(RoutedJsonGetResponse {
        body: norito::json::from_slice(&body)?,
        routed_by: routed_header_string(&headers, "x-iroha-routed-by"),
        route_lane_id: routed_header_string(&headers, "x-iroha-route-lane-id"),
        route_dataspace_id: routed_header_string(&headers, "x-iroha-route-dataspace-id"),
    })
}

async fn torii_json_get_with_retry(
    client: &Client,
    path_segments: &[String],
    query_pairs: &[(String, String)],
    context: &str,
) -> Result<RoutedJsonGetResponse> {
    let started = Instant::now();
    let mut last_error: Option<String> = None;
    let mut attempts = 0_u64;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        attempts = attempts.saturating_add(1);
        match timeout(
            OBSERVER_QUERY_TIMEOUT_CAP,
            torii_json_get(client, path_segments, query_pairs),
        )
        .await
        {
            Ok(Ok(response)) => return Ok(response),
            Ok(Err(err)) => {
                last_error = Some(render_error_with_debug(&err));
            }
            Err(_) => {
                last_error = Some(format!(
                    "request attempt timed out after {}s",
                    OBSERVER_QUERY_TIMEOUT_CAP.as_secs()
                ));
            }
        }
        sleep(STATUS_POLL_INTERVAL).await;
    }
    let suffix = last_error
        .map(|err| format!("; last error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out after {attempts} attempts waiting for routed Torii GET{suffix}"
    ))
}

fn expect_local_or_proxy_fanout_headers(
    response: &RoutedJsonGetResponse,
    context: &str,
) -> Result<()> {
    ensure!(
        matches!(response.routed_by.as_deref(), Some("local" | "proxy")),
        "{context}: expected local or proxy fanout read, observed {:?}",
        response.routed_by
    );
    ensure!(
        response.route_lane_id.is_none(),
        "{context}: fanout response should not expose a singular route lane {:?}",
        response.route_lane_id
    );
    ensure!(
        response.route_dataspace_id.is_none(),
        "{context}: fanout response should not expose a singular route dataspace {:?}",
        response.route_dataspace_id
    );
    Ok(())
}

#[derive(Clone, Copy, Debug)]
struct DataspaceCommitmentObservation {
    height: u64,
    elapsed: Duration,
    approval_observed: bool,
}

#[derive(Clone, Copy, Debug)]
struct RoutedTransactionObservation {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    approved_height: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LaneDomainProgress {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    lane_block_view: u64,
    descriptor_hash: Hash,
    proposal_hash: Hash,
    subject_hash: Hash,
    payload_ownership_hash: Hash,
    rbc_instance_hash: Hash,
    qc_mode_tag: String,
    dataspace_commitment_height: Option<u64>,
    prepare_qc_signer_count: u32,
    commit_qc_signer_count: u32,
    execution_status: String,
    executable_payload_available: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LanePayloadOwnershipProgress {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    lane_block_view: u64,
    proposal_height: u64,
    proposal_view: u64,
    subject_hash: Hash,
    lane_block_descriptor_hash: Hash,
    payload_ownership_hash: Hash,
    rbc_instance_hash: Hash,
    accepted_transaction_count: usize,
    validator_count: u32,
    min_quorum: u32,
}

fn committed_lane_block_has_expected_quorum(
    block: &SumeragiCommittedLaneBlock,
    expected_validator_count: usize,
) -> bool {
    let Ok(validator_count) = usize::try_from(block.validator_count) else {
        return false;
    };
    let Ok(min_quorum) = usize::try_from(block.min_quorum) else {
        return false;
    };
    let Ok(prepare_qc_signer_count) = usize::try_from(block.prepare_qc_signer_count) else {
        return false;
    };
    let Ok(commit_qc_signer_count) = usize::try_from(block.commit_qc_signer_count) else {
        return false;
    };
    let expected_quorum = commit_quorum_from_len(expected_validator_count).max(1);
    validator_count == expected_validator_count
        && min_quorum == expected_quorum
        && prepare_qc_signer_count >= expected_quorum
        && prepare_qc_signer_count <= validator_count
        && commit_qc_signer_count >= expected_quorum
        && commit_qc_signer_count <= validator_count
}

fn lane_payload_ownership_has_expected_quorum(
    ownership: &SumeragiLanePayloadOwnership,
    expected_validator_count: usize,
) -> bool {
    let Ok(validator_count) = usize::try_from(ownership.lane_block_descriptor_validator_count)
    else {
        return false;
    };
    let Ok(min_quorum) = usize::try_from(ownership.lane_block_descriptor_min_quorum) else {
        return false;
    };
    let expected_quorum = commit_quorum_from_len(expected_validator_count).max(1);
    validator_count == expected_validator_count
        && ownership.lane_block_descriptor_validator_set.len() == expected_validator_count
        && min_quorum == expected_quorum
        && ownership.validate_replay_material().is_ok()
}

fn dataspace_commitment_height_for(
    status: &SumeragiDiagnosticsStatus,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
) -> Option<u64> {
    status
        .dataspace_commitments
        .iter()
        .filter(|commitment| {
            commitment.lane_id == lane_id
                && commitment.dataspace_id == dataspace_id
                && (commitment.tx_count > 0 || commitment.teu_total > 0)
        })
        .map(|commitment| commitment.block_height)
        .max()
}

fn lane_domain_progress_from_block(
    block: &SumeragiCommittedLaneBlock,
    dataspace_commitment_height: Option<u64>,
) -> LaneDomainProgress {
    LaneDomainProgress {
        lane_id: block.lane_id,
        dataspace_id: block.dataspace_id,
        lane_incarnation: block.lane_incarnation,
        lane_block_height: block.lane_block_height,
        lane_block_view: block.lane_block_view,
        descriptor_hash: block.descriptor_hash,
        proposal_hash: block.proposal_hash,
        subject_hash: block.subject_hash,
        payload_ownership_hash: block.payload_ownership_hash,
        rbc_instance_hash: block.rbc_instance_hash,
        qc_mode_tag: block.qc_mode_tag.clone(),
        dataspace_commitment_height,
        prepare_qc_signer_count: block.prepare_qc_signer_count,
        commit_qc_signer_count: block.commit_qc_signer_count,
        execution_status: block.execution_status.clone(),
        executable_payload_available: block.executable_payload_available,
    }
}

fn latest_lane_domain_progress(
    status: &SumeragiDiagnosticsStatus,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
) -> Option<LaneDomainProgress> {
    let dataspace_commitment_height =
        dataspace_commitment_height_for(status, lane_id, dataspace_id);
    let mut latest = None::<&SumeragiCommittedLaneBlock>;
    let mut latest_is_ambiguous = false;
    for block in status
        .committed_lane_blocks
        .iter()
        .filter(|block| block.lane_id == lane_id && block.dataspace_id == dataspace_id)
    {
        let block_key = (block.lane_block_height, block.lane_block_view);
        let Some(current) = latest else {
            latest = Some(block);
            latest_is_ambiguous = false;
            continue;
        };
        let current_key = (current.lane_block_height, current.lane_block_view);
        if block_key > current_key {
            latest = Some(block);
            latest_is_ambiguous = false;
        } else if block_key == current_key && block != current {
            latest_is_ambiguous = true;
        }
    }
    if latest_is_ambiguous {
        return None;
    }
    latest
        .filter(|block| {
            block.lane_block_height > 0
                && committed_lane_block_has_expected_quorum(block, VALIDATORS_PER_LANE)
        })
        .map(|block| lane_domain_progress_from_block(block, dataspace_commitment_height))
}

fn lane_domain_progress_is_applied(progress: &LaneDomainProgress) -> bool {
    progress.executable_payload_available
        && matches!(
            progress.execution_status.as_str(),
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK
                | COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION
        )
}

fn latest_lane_domain_application_progress(
    status: &SumeragiDiagnosticsStatus,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
) -> Option<LaneDomainProgress> {
    latest_lane_domain_progress(status, lane_id, dataspace_id)
        .filter(lane_domain_progress_is_applied)
}

fn applied_lane_domain_progress(
    status: &SumeragiDiagnosticsStatus,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
) -> Option<LaneDomainProgress> {
    let dataspace_commitment_height =
        dataspace_commitment_height_for(status, lane_id, dataspace_id);
    let mut latest_applied = None::<&SumeragiCommittedLaneBlock>;
    let mut latest_applied_is_ambiguous = false;
    for block in status
        .committed_lane_blocks
        .iter()
        .filter(|block| block.lane_id == lane_id && block.dataspace_id == dataspace_id)
        .filter(|block| {
            block.lane_block_height > 0
                && committed_lane_block_has_expected_quorum(block, VALIDATORS_PER_LANE)
                && block.executable_payload_available
                && matches!(
                    block.execution_status.as_str(),
                    COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK
                        | COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION
                )
        })
    {
        let block_key = (block.lane_block_height, block.lane_block_view);
        let Some(current) = latest_applied else {
            latest_applied = Some(block);
            latest_applied_is_ambiguous = false;
            continue;
        };
        let current_key = (current.lane_block_height, current.lane_block_view);
        if block_key > current_key {
            latest_applied = Some(block);
            latest_applied_is_ambiguous = false;
        } else if block_key == current_key && block != current {
            latest_applied_is_ambiguous = true;
        }
    }
    if latest_applied_is_ambiguous {
        return None;
    }
    latest_applied.map(|block| lane_domain_progress_from_block(block, dataspace_commitment_height))
}

fn latest_lane_payload_ownership_progress(
    status: &SumeragiDiagnosticsStatus,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
) -> Option<LanePayloadOwnershipProgress> {
    let mut latest = None::<&SumeragiLanePayloadOwnership>;
    let mut latest_is_ambiguous = false;
    for ownership in status
        .lane_payload_ownerships
        .iter()
        .filter(|ownership| ownership.lane_id == lane_id && ownership.dataspace_id == dataspace_id)
    {
        let ownership_key = (
            ownership.lane_block_height,
            ownership.lane_block_view,
            ownership.proposal_height,
            ownership.proposal_view,
        );
        let Some(current) = latest else {
            latest = Some(ownership);
            latest_is_ambiguous = false;
            continue;
        };
        let current_key = (
            current.lane_block_height,
            current.lane_block_view,
            current.proposal_height,
            current.proposal_view,
        );
        if ownership_key > current_key {
            latest = Some(ownership);
            latest_is_ambiguous = false;
        } else if ownership_key == current_key && ownership != current {
            latest_is_ambiguous = true;
        }
    }
    if latest_is_ambiguous {
        return None;
    }
    latest
        .filter(|ownership| {
            lane_payload_ownership_has_expected_quorum(ownership, VALIDATORS_PER_LANE)
        })
        .map(|ownership| LanePayloadOwnershipProgress {
            lane_id,
            dataspace_id,
            lane_incarnation: ownership.lane_incarnation,
            lane_block_height: ownership.lane_block_height,
            lane_block_view: ownership.lane_block_view,
            proposal_height: ownership.proposal_height,
            proposal_view: ownership.proposal_view,
            subject_hash: ownership.subject_hash,
            lane_block_descriptor_hash: ownership
                .lane_block_descriptor_hash
                .expect("validated ownership has descriptor hash"),
            payload_ownership_hash: ownership.payload_ownership_hash,
            rbc_instance_hash: ownership.rbc_instance_hash,
            accepted_transaction_count: ownership.accepted_transaction_hashes.len(),
            validator_count: ownership.lane_block_descriptor_validator_count,
            min_quorum: ownership.lane_block_descriptor_min_quorum,
        })
}

fn lane_domain_progress_matches_candidate(
    observed: &LaneDomainProgress,
    candidate: &LaneDomainProgress,
) -> bool {
    observed.lane_id == candidate.lane_id
        && observed.dataspace_id == candidate.dataspace_id
        && observed.lane_incarnation == candidate.lane_incarnation
        && (observed.lane_block_height > candidate.lane_block_height
            || (observed.lane_block_height == candidate.lane_block_height
                && (observed.lane_block_view > candidate.lane_block_view
                    || (observed.lane_block_view == candidate.lane_block_view
                        && observed.descriptor_hash == candidate.descriptor_hash
                        && observed.proposal_hash == candidate.proposal_hash
                        && observed.subject_hash == candidate.subject_hash
                        && observed.payload_ownership_hash == candidate.payload_ownership_hash
                        && observed.rbc_instance_hash == candidate.rbc_instance_hash
                        && observed.qc_mode_tag == candidate.qc_mode_tag))))
}

fn lane_domain_progress_same_tip_identity(
    left: &LaneDomainProgress,
    right: &LaneDomainProgress,
) -> bool {
    left.lane_id == right.lane_id
        && left.dataspace_id == right.dataspace_id
        && left.lane_incarnation == right.lane_incarnation
        && left.lane_block_height == right.lane_block_height
        && left.lane_block_view == right.lane_block_view
        && left.descriptor_hash == right.descriptor_hash
        && left.proposal_hash == right.proposal_hash
        && left.subject_hash == right.subject_hash
        && left.payload_ownership_hash == right.payload_ownership_hash
        && left.rbc_instance_hash == right.rbc_instance_hash
        && left.qc_mode_tag == right.qc_mode_tag
}

fn lane_domain_progress_is_after_baseline(
    progress: &LaneDomainProgress,
    baseline: &LaneDomainProgress,
) -> bool {
    progress.lane_id == baseline.lane_id
        && progress.dataspace_id == baseline.dataspace_id
        && progress.lane_incarnation == baseline.lane_incarnation
        && (progress.lane_block_height, progress.lane_block_view)
            > (baseline.lane_block_height, baseline.lane_block_view)
}

fn lane_payload_ownership_progress_matches_candidate(
    observed: &LanePayloadOwnershipProgress,
    candidate: &LanePayloadOwnershipProgress,
) -> bool {
    observed.lane_id == candidate.lane_id
        && observed.dataspace_id == candidate.dataspace_id
        && observed.lane_incarnation == candidate.lane_incarnation
        && (observed.lane_block_height > candidate.lane_block_height
            || (observed.lane_block_height == candidate.lane_block_height
                && (observed.lane_block_view > candidate.lane_block_view
                    || (observed.lane_block_view == candidate.lane_block_view
                        && observed.proposal_height == candidate.proposal_height
                        && observed.proposal_view == candidate.proposal_view
                        && observed.subject_hash == candidate.subject_hash
                        && observed.lane_block_descriptor_hash
                            == candidate.lane_block_descriptor_hash
                        && observed.payload_ownership_hash == candidate.payload_ownership_hash
                        && observed.rbc_instance_hash == candidate.rbc_instance_hash))))
}

fn lane_payload_ownership_progress_same_tip_identity(
    left: &LanePayloadOwnershipProgress,
    right: &LanePayloadOwnershipProgress,
) -> bool {
    left.lane_id == right.lane_id
        && left.dataspace_id == right.dataspace_id
        && left.lane_incarnation == right.lane_incarnation
        && left.lane_block_height == right.lane_block_height
        && left.lane_block_view == right.lane_block_view
        && left.proposal_height == right.proposal_height
        && left.proposal_view == right.proposal_view
        && left.subject_hash == right.subject_hash
        && left.lane_block_descriptor_hash == right.lane_block_descriptor_hash
        && left.payload_ownership_hash == right.payload_ownership_hash
        && left.rbc_instance_hash == right.rbc_instance_hash
}

fn quorum_lane_domain_progress(
    observations: &[LaneDomainProgress],
    quorum_required: usize,
) -> Option<LaneDomainProgress> {
    let mut selected = None::<LaneDomainProgress>;
    for candidate in observations.iter().cloned().filter(|candidate| {
        observations
            .iter()
            .filter(|observed| lane_domain_progress_matches_candidate(observed, candidate))
            .count()
            >= quorum_required
    }) {
        let Some(current) = selected.as_ref() else {
            selected = Some(candidate);
            continue;
        };
        let candidate_key = (candidate.lane_block_height, candidate.lane_block_view);
        let current_key = (current.lane_block_height, current.lane_block_view);
        match candidate_key.cmp(&current_key) {
            std::cmp::Ordering::Greater => selected = Some(candidate),
            std::cmp::Ordering::Equal => {
                if !lane_domain_progress_same_tip_identity(current, &candidate) {
                    return None;
                }
            }
            std::cmp::Ordering::Less => {}
        }
    }
    selected
}

fn quorum_lane_payload_ownership_progress(
    observations: &[LanePayloadOwnershipProgress],
    quorum_required: usize,
) -> Option<LanePayloadOwnershipProgress> {
    let mut selected = None::<LanePayloadOwnershipProgress>;
    for candidate in observations.iter().cloned().filter(|candidate| {
        observations
            .iter()
            .filter(|observed| {
                lane_payload_ownership_progress_matches_candidate(observed, candidate)
            })
            .count()
            >= quorum_required
    }) {
        let Some(current) = selected.as_ref() else {
            selected = Some(candidate);
            continue;
        };
        let candidate_key = (candidate.lane_block_height, candidate.lane_block_view);
        let current_key = (current.lane_block_height, current.lane_block_view);
        match candidate_key.cmp(&current_key) {
            std::cmp::Ordering::Greater => selected = Some(candidate),
            std::cmp::Ordering::Equal => {
                if !lane_payload_ownership_progress_same_tip_identity(current, &candidate) {
                    return None;
                }
            }
            std::cmp::Ordering::Less => {}
        }
    }
    selected
}

fn lane_domain_progress_observations(
    network: &sandbox::SerializedNetwork,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    last_error: &mut Option<String>,
) -> Vec<LaneDomainProgress> {
    let started = Instant::now();
    let indices = peer_indices_for_committed_lane_evidence(network.peers().len());
    let request_count = indices.len();
    indices
        .into_iter()
        .enumerate()
        .filter_map(|(position, index)| {
            let mut client = network.peers()[index].client();
            client.torii_request_timeout =
                client
                    .torii_request_timeout
                    .min(bounded_observer_request_timeout(
                        started,
                        OBSERVER_QUERY_TIMEOUT_CAP,
                        request_count.saturating_sub(position),
                    ));
            match sumeragi_observation(&client) {
                Ok(status) => latest_lane_domain_progress(&status, lane_id, dataspace_id),
                Err(err) => {
                    *last_error = Some(err.to_string());
                    None
                }
            }
        })
        .collect()
}

fn lane_domain_application_observations(
    network: &sandbox::SerializedNetwork,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    last_error: &mut Option<String>,
) -> Vec<LaneDomainProgress> {
    let started = Instant::now();
    let indices = peer_indices_for_committed_lane_evidence(network.peers().len());
    let request_count = indices.len();
    indices
        .into_iter()
        .enumerate()
        .filter_map(|(position, index)| {
            let mut client = network.peers()[index].client();
            client.torii_request_timeout =
                client
                    .torii_request_timeout
                    .min(bounded_observer_request_timeout(
                        started,
                        OBSERVER_QUERY_TIMEOUT_CAP,
                        request_count.saturating_sub(position),
                    ));
            match sumeragi_observation(&client) {
                Ok(status) => applied_lane_domain_progress(&status, lane_id, dataspace_id),
                Err(err) => {
                    *last_error = Some(err.to_string());
                    None
                }
            }
        })
        .collect()
}

fn raw_lane_domain_observation_summaries(
    network: &sandbox::SerializedNetwork,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
) -> Vec<String> {
    let started = Instant::now();
    let indices = peer_indices_for_committed_lane_evidence(network.peers().len());
    let request_count = indices.len();
    indices
        .into_iter()
        .enumerate()
        .map(|(position, index)| {
            let mut client = network.peers()[index].client();
            client.torii_request_timeout = client.torii_request_timeout.min(
                bounded_observer_request_timeout(
                    started,
                    OBSERVER_QUERY_TIMEOUT_CAP,
                    request_count.saturating_sub(position),
                ),
            );
            match sumeragi_observation(&client) {
                Ok(status) => {
                    let committed = status
                        .committed_lane_blocks
                        .iter()
                        .filter(|block| {
                            block.lane_id == lane_id && block.dataspace_id == dataspace_id
                        })
                        .map(|block| {
                            format!(
                                "{}/{} status={} exec={} quorum={}/{} prepare={} commit={}",
                                block.lane_block_height,
                                block.lane_block_view,
                                block.execution_status,
                                block.executable_payload_available,
                                block.min_quorum,
                                block.validator_count,
                                block.prepare_qc_signer_count,
                                block.commit_qc_signer_count
                            )
                        })
                        .collect::<Vec<_>>();
                    let ownership = status
                        .lane_payload_ownerships
                        .iter()
                        .filter(|ownership| {
                            ownership.lane_id == lane_id && ownership.dataspace_id == dataspace_id
                        })
                        .map(|ownership| {
                            format!(
                                "{}/{} accepted={} quorum={}/{}",
                                ownership.lane_block_height,
                                ownership.lane_block_view,
                                ownership.accepted_transaction_hashes.len(),
                                ownership.lane_block_descriptor_min_quorum,
                                ownership.lane_block_descriptor_validator_count
                            )
                        })
                        .collect::<Vec<_>>();
                    let sessions = status
                        .lane_block_sessions
                        .iter()
                        .filter(|session| {
                            session.lane_id == lane_id && session.dataspace_id == dataspace_id
                        })
                        .map(|session| {
                            format!(
                                "{}/{} prop={} pv={} cv={} pqc={} cqc={} drain={} drained={} quorum={}/{} hash={}",
                                session.lane_block_height,
                                session.lane_block_view,
                                session.has_proposal,
                                session.prepare_vote_count,
                                session.commit_vote_count,
                                session.has_prepare_qc,
                                session.has_commit_qc,
                                session.pending_committed_session_drain,
                                session.committed_session_drained,
                                session.min_quorum,
                                session.validator_count,
                                session.proposal_hash,
                            )
                        })
                        .collect::<Vec<_>>();
                    format!(
                        "peer#{index}: height={} committed=[{}] ownership=[{}] sessions=[{}]",
                        status.canonical.last_committed_height,
                        committed.join(", "),
                        ownership.join(", "),
                        sessions.join(", ")
                    )
                }
                Err(err) => format!("peer#{index}: status error={}", err),
            }
        })
        .collect()
}

fn lane_payload_ownership_progress_observations(
    network: &sandbox::SerializedNetwork,
    status_client: &Client,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    last_error: &mut Option<String>,
) -> Vec<LanePayloadOwnershipProgress> {
    let started = Instant::now();
    let indices = lane_bounded_peer_indices(network, status_client, lane_id.as_u32());
    let request_count = indices.len();
    indices
        .into_iter()
        .enumerate()
        .filter_map(|(position, index)| {
            let mut client = network.peers()[index].client();
            client.torii_request_timeout =
                client
                    .torii_request_timeout
                    .min(bounded_observer_request_timeout(
                        started,
                        OBSERVER_QUERY_TIMEOUT_CAP,
                        request_count.saturating_sub(position),
                    ));
            match client.get_sumeragi_diagnostics() {
                Ok(status) => {
                    latest_lane_payload_ownership_progress(&status, lane_id, dataspace_id)
                }
                Err(err) => {
                    *last_error = Some(err.to_string());
                    None
                }
            }
        })
        .collect()
}

fn wait_for_independent_lane_domain_progress(
    network: &sandbox::SerializedNetwork,
    leading_tick_submitters: &[Client],
    trailing_tick_submitters: &[Client],
    leading_lane: (LaneId, DataSpaceId),
    trailing_lane: (LaneId, DataSpaceId),
    context: &str,
) -> Result<(LaneDomainProgress, LaneDomainProgress)> {
    // The committed status row embeds prepare/commit QC signer counts, so one
    // observed row with lane-quorum signer counts is sufficient to prove the
    // lane committee certified that standalone lane block. Status-row fanout to
    // every lane validator is best-effort operator telemetry and is not itself
    // the safety proof.
    let quorum_required = 1;
    let started = Instant::now();
    let mut last_leading = Vec::new();
    let mut last_trailing = Vec::new();
    let mut last_error: Option<String> = None;
    let mut poll_count = 0;
    while started.elapsed() <= LANE_PROGRESS_WAIT_TIMEOUT {
        last_leading = lane_domain_progress_observations(
            network,
            leading_lane.0,
            leading_lane.1,
            &mut last_error,
        );
        last_trailing = lane_domain_progress_observations(
            network,
            trailing_lane.0,
            trailing_lane.1,
            &mut last_error,
        );
        let leading_progress = quorum_lane_domain_progress(&last_leading, quorum_required);
        let trailing_progress = quorum_lane_domain_progress(&last_trailing, quorum_required);
        if let (Some(leading), Some(trailing)) =
            (leading_progress.clone(), trailing_progress.clone())
        {
            return Ok((leading, trailing));
        }
        if should_submit_tick(poll_count, LANE_PROGRESS_RECOVERY_TICK_EVERY_POLLS) {
            if leading_progress.is_none() {
                submit_wait_ticks(
                    leading_tick_submitters,
                    context,
                    poll_count,
                    started,
                    LANE_PROGRESS_WAIT_TIMEOUT,
                    &mut last_error,
                );
            }
            if trailing_progress.is_none() {
                submit_wait_ticks(
                    trailing_tick_submitters,
                    context,
                    poll_count,
                    started,
                    LANE_PROGRESS_WAIT_TIMEOUT,
                    &mut last_error,
                );
            }
        }
        poll_count = poll_count.saturating_add(1);
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    let suffix = last_error
        .map(|err| format!("; last status query/tick error: {err}"))
        .unwrap_or_default();
    let raw_leading =
        raw_lane_domain_observation_summaries(network, leading_lane.0, leading_lane.1);
    let raw_trailing =
        raw_lane_domain_observation_summaries(network, trailing_lane.0, trailing_lane.1);
    Err(eyre!(
        "{context}: timed out waiting for observable committed lane-block QCs; expected lane {} dataspace {} and lane {} dataspace {} to publish embedded lane quorum; last leading observations {last_leading:?}; last trailing observations {last_trailing:?}; raw leading {raw_leading:?}; raw trailing {raw_trailing:?}{suffix}",
        leading_lane.0.as_u32(),
        leading_lane.1.as_u64(),
        trailing_lane.0.as_u32(),
        trailing_lane.1.as_u64(),
    ))
}

fn wait_for_independent_lane_application_progress(
    network: &sandbox::SerializedNetwork,
    leading_tick_submitters: &[Client],
    trailing_tick_submitters: &[Client],
    leading_lane: (LaneId, DataSpaceId),
    trailing_lane: (LaneId, DataSpaceId),
    context: &str,
) -> Result<(LaneDomainProgress, LaneDomainProgress)> {
    wait_for_independent_lane_application_progress_with_baseline(
        network,
        leading_tick_submitters,
        trailing_tick_submitters,
        leading_lane,
        trailing_lane,
        None,
        None,
        context,
    )
}

fn wait_for_independent_lane_application_progress_after(
    network: &sandbox::SerializedNetwork,
    leading_tick_submitters: &[Client],
    trailing_tick_submitters: &[Client],
    leading_baseline: &LaneDomainProgress,
    trailing_baseline: &LaneDomainProgress,
    context: &str,
) -> Result<(LaneDomainProgress, LaneDomainProgress)> {
    wait_for_independent_lane_application_progress_with_baseline(
        network,
        leading_tick_submitters,
        trailing_tick_submitters,
        (leading_baseline.lane_id, leading_baseline.dataspace_id),
        (trailing_baseline.lane_id, trailing_baseline.dataspace_id),
        Some(leading_baseline),
        Some(trailing_baseline),
        context,
    )
}

fn current_lane_application_progress(
    network: &sandbox::SerializedNetwork,
    lane: (LaneId, DataSpaceId),
) -> Option<LaneDomainProgress> {
    let mut last_error = None;
    let observations =
        lane_domain_application_observations(network, lane.0, lane.1, &mut last_error);
    quorum_lane_domain_progress(&observations, 1)
}

fn wait_for_lane_application_progress_after(
    network: &sandbox::SerializedNetwork,
    tick_submitters: &[Client],
    lane: (LaneId, DataSpaceId),
    baseline: Option<&LaneDomainProgress>,
    context: &str,
) -> Result<LaneDomainProgress> {
    let started = Instant::now();
    let mut last_observations = Vec::new();
    let mut last_error: Option<String> = None;
    let mut poll_count = 0;
    while started.elapsed() <= LANE_PROGRESS_WAIT_TIMEOUT {
        last_observations =
            lane_domain_application_observations(network, lane.0, lane.1, &mut last_error)
                .into_iter()
                .filter(|progress| {
                    baseline
                        .map(|baseline| lane_domain_progress_is_after_baseline(progress, baseline))
                        .unwrap_or(true)
                })
                .collect();
        if let Some(progress) = quorum_lane_domain_progress(&last_observations, 1) {
            return Ok(progress);
        }
        if should_submit_tick(poll_count, LANE_PROGRESS_RECOVERY_TICK_EVERY_POLLS) {
            submit_wait_ticks(
                tick_submitters,
                context,
                poll_count,
                started,
                LANE_PROGRESS_WAIT_TIMEOUT,
                &mut last_error,
            );
        }
        poll_count = poll_count.saturating_add(1);
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    let suffix = last_error
        .map(|err| format!("; last status query/tick error: {err}"))
        .unwrap_or_default();
    let raw = raw_lane_domain_observation_summaries(network, lane.0, lane.1);
    let baseline_context = baseline
        .map(|baseline| {
            format!(
                "; baseline {}/{}",
                baseline.lane_block_height, baseline.lane_block_view
            )
        })
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for observable applied committed lane-block evidence; expected lane {} dataspace {} to publish applied lane status{baseline_context}; last observations {last_observations:?}; raw {raw:?}{suffix}",
        lane.0.as_u32(),
        lane.1.as_u64(),
    ))
}

fn wait_for_independent_lane_application_progress_with_baseline(
    network: &sandbox::SerializedNetwork,
    leading_tick_submitters: &[Client],
    trailing_tick_submitters: &[Client],
    leading_lane: (LaneId, DataSpaceId),
    trailing_lane: (LaneId, DataSpaceId),
    leading_baseline: Option<&LaneDomainProgress>,
    trailing_baseline: Option<&LaneDomainProgress>,
    context: &str,
) -> Result<(LaneDomainProgress, LaneDomainProgress)> {
    // Application status is still tied to a committed lane-block row carrying
    // prepare/commit QC signer counts; one observed row with lane-quorum signer
    // counts and applied execution status proves that lane's executor boundary
    // reached a receipt-backed state.
    let quorum_required = 1;
    let started = Instant::now();
    let mut last_leading = Vec::new();
    let mut last_trailing = Vec::new();
    let mut last_error: Option<String> = None;
    let mut poll_count = 0;
    while started.elapsed() <= LANE_PROGRESS_WAIT_TIMEOUT {
        last_leading = lane_domain_application_observations(
            network,
            leading_lane.0,
            leading_lane.1,
            &mut last_error,
        )
        .into_iter()
        .filter(|progress| {
            leading_baseline
                .map(|baseline| lane_domain_progress_is_after_baseline(progress, baseline))
                .unwrap_or(true)
        })
        .collect();
        last_trailing = lane_domain_application_observations(
            network,
            trailing_lane.0,
            trailing_lane.1,
            &mut last_error,
        )
        .into_iter()
        .filter(|progress| {
            trailing_baseline
                .map(|baseline| lane_domain_progress_is_after_baseline(progress, baseline))
                .unwrap_or(true)
        })
        .collect();
        let leading_progress = quorum_lane_domain_progress(&last_leading, quorum_required);
        let trailing_progress = quorum_lane_domain_progress(&last_trailing, quorum_required);
        if let (Some(leading), Some(trailing)) =
            (leading_progress.clone(), trailing_progress.clone())
        {
            return Ok((leading, trailing));
        }
        if should_submit_tick(poll_count, LANE_PROGRESS_RECOVERY_TICK_EVERY_POLLS) {
            if leading_progress.is_none() {
                submit_wait_ticks(
                    leading_tick_submitters,
                    context,
                    poll_count,
                    started,
                    LANE_PROGRESS_WAIT_TIMEOUT,
                    &mut last_error,
                );
            }
            if trailing_progress.is_none() {
                submit_wait_ticks(
                    trailing_tick_submitters,
                    context,
                    poll_count,
                    started,
                    LANE_PROGRESS_WAIT_TIMEOUT,
                    &mut last_error,
                );
            }
        }
        poll_count = poll_count.saturating_add(1);
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    let suffix = last_error
        .map(|err| format!("; last status query/tick error: {err}"))
        .unwrap_or_default();
    let raw_leading =
        raw_lane_domain_observation_summaries(network, leading_lane.0, leading_lane.1);
    let raw_trailing =
        raw_lane_domain_observation_summaries(network, trailing_lane.0, trailing_lane.1);
    let baseline_context = match (leading_baseline, trailing_baseline) {
        (Some(leading), Some(trailing)) => format!(
            "; baselines leading {}/{} trailing {}/{}",
            leading.lane_block_height,
            leading.lane_block_view,
            trailing.lane_block_height,
            trailing.lane_block_view
        ),
        _ => String::new(),
    };
    Err(eyre!(
        "{context}: timed out waiting for observable applied committed lane-block evidence; expected lane {} dataspace {} and lane {} dataspace {} to publish applied lane status{baseline_context}; last leading observations {last_leading:?}; last trailing observations {last_trailing:?}; raw leading {raw_leading:?}; raw trailing {raw_trailing:?}{suffix}",
        leading_lane.0.as_u32(),
        leading_lane.1.as_u64(),
        trailing_lane.0.as_u32(),
        trailing_lane.1.as_u64(),
    ))
}

fn wait_for_independent_lane_payload_ownership_progress(
    network: &sandbox::SerializedNetwork,
    status_client: &Client,
    leading_tick_submitters: &[Client],
    trailing_tick_submitters: &[Client],
    leading_lane: (LaneId, DataSpaceId),
    trailing_lane: (LaneId, DataSpaceId),
    context: &str,
) -> Result<(LanePayloadOwnershipProgress, LanePayloadOwnershipProgress)> {
    // `lane_payload_ownerships` is a proposer-local operator diagnostic emitted
    // while assembling a block, not a replicated lane-validator status slot.
    // Require observability here and leave quorum enforcement to the committed
    // lane-block QC gate below.
    let quorum_required = 1;
    let started = Instant::now();
    let mut last_leading = Vec::new();
    let mut last_trailing = Vec::new();
    let mut last_error: Option<String> = None;
    let mut poll_count = 0;
    while started.elapsed() <= LANE_PROGRESS_WAIT_TIMEOUT {
        last_leading = lane_payload_ownership_progress_observations(
            network,
            status_client,
            leading_lane.0,
            leading_lane.1,
            &mut last_error,
        );
        last_trailing = lane_payload_ownership_progress_observations(
            network,
            status_client,
            trailing_lane.0,
            trailing_lane.1,
            &mut last_error,
        );
        let leading_progress =
            quorum_lane_payload_ownership_progress(&last_leading, quorum_required);
        let trailing_progress =
            quorum_lane_payload_ownership_progress(&last_trailing, quorum_required);
        if let (Some(leading), Some(trailing)) = (leading_progress, trailing_progress)
            && leading.accepted_transaction_count > 0
            && trailing.accepted_transaction_count > 0
        {
            return Ok((leading, trailing));
        }
        if should_submit_tick(poll_count, SETUP_BARRIER_TICK_EVERY_POLLS) {
            submit_wait_ticks(
                leading_tick_submitters,
                context,
                poll_count,
                started,
                LANE_PROGRESS_WAIT_TIMEOUT,
                &mut last_error,
            );
            submit_wait_ticks(
                trailing_tick_submitters,
                context,
                poll_count,
                started,
                LANE_PROGRESS_WAIT_TIMEOUT,
                &mut last_error,
            );
        }
        poll_count = poll_count.saturating_add(1);
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    let suffix = last_error
        .map(|err| format!("; last status query/tick error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for observable independent lane-payload ownership progress; expected lane {} dataspace {} and lane {} dataspace {} to publish non-empty lane-local ownership; last leading observations {last_leading:?}; last trailing observations {last_trailing:?}{suffix}",
        leading_lane.0.as_u32(),
        leading_lane.1.as_u64(),
        trailing_lane.0.as_u32(),
        trailing_lane.1.as_u64(),
    ))
}

async fn wait_for_route_probe_approval(
    submitter: &Client,
    instruction: InstructionBox,
    expected_lane_id: LaneId,
    expected_dataspace_id: DataSpaceId,
    context: &str,
) -> Result<DataspaceCommitmentObservation> {
    let transaction = submitter.build_transaction(
        [instruction],
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    );
    let hash = transaction.hash();
    let started = Instant::now();
    let submit_height = submitter
        .get_sumeragi_status()
        .map_err(|err| eyre!(err))?
        .last_committed_height;
    let mut events = timeout(
        STATUS_WAIT_TIMEOUT,
        submitter.listen_for_events_async([TransactionEventFilter::default().for_hash(hash)]),
    )
    .await
    .map_err(|_| eyre!("{context}: timed out opening transaction event stream"))??;

    // Give the SSE subscription handshake a brief head start so we can
    // opportunistically observe the queued routing decision event.
    sleep(ROUTE_PROBE_SSE_HANDSHAKE_DELAY).await;

    let submitter_for_submit = submitter.clone();
    spawn_blocking(move || submitter_for_submit.submit_transaction(&transaction))
        .await
        .map_err(|err| eyre!("{context}: route probe submit task join error: {err}"))?
        .map_err(|err| eyre!("{context}: failed to submit route probe transaction: {err}"))?;

    let mut first_seen_elapsed: Option<Duration> = None;
    let mut approved_height = None;
    let event_poll_deadline = Instant::now() + ROUTE_PROBE_APPROVAL_WAIT_TIMEOUT;
    while Instant::now() <= event_poll_deadline {
        let Some(next) = timeout(STATUS_POLL_INTERVAL, events.next())
            .await
            .ok()
            .flatten()
        else {
            continue;
        };
        let EventBox::Pipeline(PipelineEventBox::Transaction(event)) = next? else {
            continue;
        };
        match event.status() {
            TransactionStatus::Queued => {
                ensure!(
                    event.lane_id() == expected_lane_id,
                    "{context}: expected queued lane {}, observed {}",
                    expected_lane_id.as_u32(),
                    event.lane_id().as_u32()
                );
                ensure!(
                    event.dataspace_id() == expected_dataspace_id,
                    "{context}: expected queued dataspace {}, observed {}",
                    expected_dataspace_id.as_u64(),
                    event.dataspace_id().as_u64()
                );
                first_seen_elapsed.get_or_insert_with(|| started.elapsed());
            }
            TransactionStatus::Approved => {
                first_seen_elapsed.get_or_insert_with(|| started.elapsed());
                approved_height =
                    Some(event.block_height().map(NonZeroU64::get).ok_or_else(|| {
                        eyre!("{context}: approved transaction event missing block height")
                    })?);
                break;
            }
            TransactionStatus::Rejected(reason) => {
                return Err(eyre!(
                    "{context}: route probe transaction rejected: {reason}"
                ));
            }
            TransactionStatus::Expired => {
                return Err(eyre!("{context}: route probe transaction expired"));
            }
        }
    }

    events.close().await;
    let (height, approval_observed) = if let Some(height) = approved_height {
        (height, true)
    } else {
        let fallback_height = submitter
            .get_sumeragi_status()
            .map_err(|err| eyre!(err))?
            .last_committed_height
            .max(submit_height)
            .saturating_add(1);
        (fallback_height, false)
    };
    Ok(DataspaceCommitmentObservation {
        height,
        elapsed: first_seen_elapsed.unwrap_or_else(|| started.elapsed()),
        approval_observed,
    })
}

async fn submit_transaction_with_route_observation(
    submitter: &Client,
    transaction: &SignedTransaction,
    context: &str,
) -> Result<RoutedTransactionObservation> {
    let hash = transaction.hash();
    let mut events = timeout(
        STATUS_WAIT_TIMEOUT,
        submitter.listen_for_events_async([TransactionEventFilter::default().for_hash(hash)]),
    )
    .await
    .map_err(|_| eyre!("{context}: timed out opening transaction event stream"))??;

    sleep(ROUTE_PROBE_SSE_HANDSHAKE_DELAY).await;

    let submitter_for_submit = submitter.clone();
    let transaction_for_submit = transaction.clone();
    spawn_blocking(move || submitter_for_submit.submit_transaction(&transaction_for_submit))
        .await
        .map_err(|err| eyre!("{context}: route-observed submit task join error: {err}"))?
        .map_err(|err| eyre!("{context}: failed to submit route-observed transaction: {err}"))?;

    let mut observed = None;
    let event_poll_deadline = Instant::now() + BLOCKING_CONFIRMATION_TIMEOUT;
    while Instant::now() <= event_poll_deadline {
        let Some(next) = timeout(STATUS_POLL_INTERVAL, events.next())
            .await
            .ok()
            .flatten()
        else {
            continue;
        };
        let EventBox::Pipeline(PipelineEventBox::Transaction(event)) = next? else {
            continue;
        };
        match event.status() {
            TransactionStatus::Queued => {
                observed.get_or_insert(RoutedTransactionObservation {
                    lane_id: event.lane_id(),
                    dataspace_id: event.dataspace_id(),
                    approved_height: None,
                });
            }
            TransactionStatus::Approved => {
                let approved_height = event.block_height().map(NonZeroU64::get);
                observed = Some(RoutedTransactionObservation {
                    lane_id: event.lane_id(),
                    dataspace_id: event.dataspace_id(),
                    approved_height,
                });
                break;
            }
            TransactionStatus::Rejected(reason) => {
                events.close().await;
                return Err(eyre!(
                    "{context}: route-observed transaction rejected: {reason}"
                ));
            }
            TransactionStatus::Expired => {
                events.close().await;
                return Err(eyre!("{context}: route-observed transaction expired"));
            }
        }
    }

    events.close().await;
    observed.ok_or_else(|| eyre!("{context}: timed out observing queued transaction route"))
}

fn wait_for_expected_balances(
    expectations: &[BalanceExpectation<'_>],
    context: &str,
) -> Result<()> {
    wait_for_expected_balances_with_timeout(expectations, context, STATUS_WAIT_TIMEOUT)
}

struct BalanceExpectation<'a> {
    client: &'a Client,
    asset_id: &'a AssetId,
    expected: Quantity,
}

struct BalanceExpectationAcrossClients<'a> {
    clients: &'a [Client],
    asset_id: &'a AssetId,
    expected: Quantity,
}

fn total_balance_observer_request_slots(client_counts: impl IntoIterator<Item = usize>) -> usize {
    client_counts
        .into_iter()
        .map(|client_count| client_count.max(1))
        .sum()
}

fn wait_for_expected_balances_with_timeout(
    expectations: &[BalanceExpectation<'_>],
    context: &str,
    timeout_duration: Duration,
) -> Result<()> {
    let started = Instant::now();
    let mut last_observed = Vec::with_capacity(expectations.len());
    let mut last_error: Option<String> = None;
    while started.elapsed() <= timeout_duration {
        last_observed.clear();
        let mut all_match = true;
        let expectation_count = expectations.len();
        for (index, expectation) in expectations.iter().enumerate() {
            let mut client = expectation.client.clone();
            client.torii_request_timeout =
                client
                    .torii_request_timeout
                    .min(bounded_observer_request_timeout(
                        started,
                        timeout_duration,
                        expectation_count.saturating_sub(index),
                    ));
            let observed = match asset_balance_variants(&client, expectation.asset_id) {
                Ok(observed) => observed,
                Err(err) => {
                    last_error = Some(render_error_with_debug(&err));
                    all_match = false;
                    break;
                }
            };
            if !observed.iter().any(|value| value == &expectation.expected) {
                all_match = false;
            }
            last_observed.push((expectation.asset_id.clone(), observed));
        }
        if all_match {
            return Ok(());
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    let suffix = last_error
        .map(|err| format!("; last balance query error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for expected balances; last observed {last_observed:?}{suffix}"
    ))
}

fn wait_for_expected_balances_across_clients_with_tick_submitters_timeout(
    tick_submitters: &[Client],
    expectations: &[BalanceExpectationAcrossClients<'_>],
    context: &str,
    timeout_duration: Duration,
) -> Result<()> {
    let started = Instant::now();
    let mut last_observed = Vec::with_capacity(expectations.len());
    let mut last_error: Option<String> = None;
    let mut poll_count = 0;
    while started.elapsed() <= timeout_duration {
        if should_submit_tick(poll_count, SETUP_BARRIER_TICK_EVERY_POLLS) {
            submit_wait_ticks(
                tick_submitters,
                context,
                poll_count,
                started,
                timeout_duration,
                &mut last_error,
            );
        }
        last_observed.clear();
        let mut all_match = true;
        let total_request_slots = total_balance_observer_request_slots(
            expectations
                .iter()
                .map(|expectation| expectation.clients.len()),
        );
        let mut remaining_request_slots = total_request_slots;
        for expectation in expectations {
            if expectation.clients.is_empty() {
                last_error = Some(format!(
                    "no balance observer clients for {}",
                    expectation.asset_id.canonical_literal()
                ));
                all_match = false;
                last_observed.push((expectation.asset_id.clone(), vec!["no clients".to_owned()]));
                continue;
            }

            let mut expectation_matches = false;
            let mut observed_for_expectation = Vec::with_capacity(expectation.clients.len());
            for (client_index, client) in expectation.clients.iter().enumerate() {
                let mut client = client.clone();
                client.torii_request_timeout =
                    client
                        .torii_request_timeout
                        .min(bounded_observer_request_timeout(
                            started,
                            timeout_duration,
                            remaining_request_slots,
                        ));
                remaining_request_slots = remaining_request_slots.saturating_sub(1);
                match asset_balance_variants(&client, expectation.asset_id) {
                    Ok(observed) => {
                        let observed_matches =
                            observed.iter().any(|value| value == &expectation.expected);
                        observed_for_expectation
                            .push(format!("client#{client_index}:{observed:?}"));
                        if observed_matches {
                            expectation_matches = true;
                            break;
                        }
                    }
                    Err(err) => {
                        let rendered_error = render_error_with_debug(&err);
                        last_error = Some(rendered_error.clone());
                        observed_for_expectation
                            .push(format!("client#{client_index}:error:{rendered_error}"));
                    }
                }
            }
            if !expectation_matches {
                all_match = false;
            }
            last_observed.push((expectation.asset_id.clone(), observed_for_expectation));
        }
        if all_match {
            return Ok(());
        }
        poll_count = poll_count.saturating_add(1);
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    let suffix = last_error
        .map(|err| format!("; last balance query error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for expected balances across clients with tick assist; last observed {last_observed:?}{suffix}"
    ))
}

fn wait_for_expected_balances_with_tick_timeout(
    tick_submitter: &Client,
    expectations: &[BalanceExpectation<'_>],
    context: &str,
    timeout_duration: Duration,
) -> Result<()> {
    wait_for_expected_balances_with_tick_submitters_timeout(
        std::slice::from_ref(tick_submitter),
        expectations,
        context,
        timeout_duration,
    )
}

fn wait_for_expected_balances_with_tick_submitters_timeout(
    tick_submitters: &[Client],
    expectations: &[BalanceExpectation<'_>],
    context: &str,
    timeout_duration: Duration,
) -> Result<()> {
    let started = Instant::now();
    let mut last_observed = Vec::with_capacity(expectations.len());
    let mut last_error: Option<String> = None;
    let mut poll_count = 0;
    while started.elapsed() <= timeout_duration {
        if should_submit_tick(poll_count, SETUP_BARRIER_TICK_EVERY_POLLS) {
            submit_wait_ticks(
                tick_submitters,
                context,
                poll_count,
                started,
                timeout_duration,
                &mut last_error,
            );
        }
        last_observed.clear();
        let mut all_match = true;
        let expectation_count = expectations.len();
        for (index, expectation) in expectations.iter().enumerate() {
            let mut client = expectation.client.clone();
            client.torii_request_timeout =
                client
                    .torii_request_timeout
                    .min(bounded_observer_request_timeout(
                        started,
                        timeout_duration,
                        expectation_count.saturating_sub(index),
                    ));
            let observed = match asset_balance_variants(&client, expectation.asset_id) {
                Ok(observed) => observed,
                Err(err) => {
                    last_error = Some(render_error_with_debug(&err));
                    all_match = false;
                    break;
                }
            };
            if !observed.iter().any(|value| value == &expectation.expected) {
                all_match = false;
            }
            last_observed.push((expectation.asset_id.clone(), observed));
        }
        if all_match {
            return Ok(());
        }
        poll_count = poll_count.saturating_add(1);
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    let suffix = last_error
        .map(|err| format!("; last balance query error: {err}"))
        .unwrap_or_default();
    let bucket_context = expectations
        .iter()
        .enumerate()
        .map(|(index, expectation)| {
            let mut client = expectation.client.clone();
            client.torii_request_timeout =
                client
                    .torii_request_timeout
                    .min(bounded_observer_request_timeout(
                        started,
                        timeout_duration,
                        expectations.len().saturating_sub(index),
                    ));
            let matches = client
                .query(FindAssets::new())
                .execute_all()
                .map(|assets: Vec<Asset>| {
                    assets
                        .into_iter()
                        .filter(|asset| {
                            asset.id.definition() == expectation.asset_id.definition()
                                && asset.id.account() == expectation.asset_id.account()
                        })
                        .map(|asset| format!("{}={}", asset.id.canonical_literal(), asset.value()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|err| {
                    vec![format!("query error: {}", render_error_with_debug(&err))]
                });
            format!(
                "{} => [{}]",
                expectation.asset_id.canonical_literal(),
                matches.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    Err(eyre!(
        "{context}: timed out waiting for expected balances with tick assist; last observed {last_observed:?}; buckets {bucket_context}{suffix}"
    ))
}

fn wait_for_account_permissions_across_clients(
    tick_submitters: &[Client],
    mut clients_factory: impl FnMut() -> Vec<Client>,
    account_id: &AccountId,
    required_permissions: &[Permission],
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_observed = Vec::new();
    let mut last_error: Option<String> = None;
    let mut poll_count = 0;
    while started.elapsed() <= PERMISSION_VISIBILITY_WAIT_TIMEOUT {
        if should_submit_tick(poll_count, SETUP_BARRIER_TICK_EVERY_POLLS) {
            submit_wait_ticks(
                tick_submitters,
                context,
                poll_count,
                started,
                PERMISSION_VISIBILITY_WAIT_TIMEOUT,
                &mut last_error,
            );
        }
        let mut observed = Vec::new();
        let mut saw_success = false;
        let clients = clients_factory();
        let client_count = clients.len();
        for (index, mut client) in clients.into_iter().enumerate() {
            client.torii_request_timeout =
                client
                    .torii_request_timeout
                    .min(bounded_observer_request_timeout(
                        started,
                        PERMISSION_VISIBILITY_WAIT_TIMEOUT,
                        client_count.saturating_sub(index),
                    ));
            match client
                .query(FindPermissionsByAccountId::new(account_id.clone()))
                .execute_all()
            {
                Ok(permissions) => {
                    saw_success = true;
                    for permission in permissions {
                        if !observed.iter().any(|existing| existing == &permission) {
                            observed.push(permission);
                        }
                    }
                }
                Err(err) => {
                    last_error = Some(render_error_with_debug(&err));
                }
            }
        }
        if saw_success {
            last_observed = observed.clone();
            let all_present = required_permissions
                .iter()
                .all(|required| observed.iter().any(|permission| permission == required));
            if all_present {
                return Ok(());
            }
        }
        poll_count = poll_count.saturating_add(1);
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    let suffix = last_error
        .map(|err| format!("; last permission query error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out after {:?} waiting for permissions on {account_id}; required {required_permissions:?}; last observed {last_observed:?}{suffix}",
        PERMISSION_VISIBILITY_WAIT_TIMEOUT
    ))
}

fn lane_validator_snapshot(
    snapshot: &JsonValue,
    context: &str,
) -> Result<(usize, BTreeSet<ExpectedLaneValidatorBinding>)> {
    let root = snapshot
        .as_object()
        .ok_or_else(|| eyre!("{context}: lane validator response is not an object"))?;
    let total = root
        .get("total")
        .and_then(JsonValue::as_u64)
        .ok_or_else(|| eyre!("{context}: lane validator response is missing total"))?;
    let items = root
        .get("items")
        .and_then(JsonValue::as_array)
        .ok_or_else(|| eyre!("{context}: lane validator response is missing items"))?;

    let mut active = BTreeSet::new();
    for item in items {
        let entry = item
            .as_object()
            .ok_or_else(|| eyre!("{context}: validator entry is not an object"))?;
        let validator = entry
            .get("validator")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| eyre!("{context}: validator entry missing validator literal"))?;
        let peer_id = entry
            .get("peer_id")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| eyre!("{context}: validator entry missing peer_id literal"))?;
        let status_type = entry
            .get("status")
            .and_then(JsonValue::as_object)
            .and_then(|status| status.get("type"))
            .and_then(JsonValue::as_str)
            .ok_or_else(|| eyre!("{context}: validator entry missing status.type"))?;
        if status_type == "Active" {
            active.insert(ExpectedLaneValidatorBinding {
                validator: validator.to_owned(),
                peer_id: peer_id.to_owned(),
            });
        }
    }

    Ok((usize::try_from(total).unwrap_or(usize::MAX), active))
}

fn wait_for_active_lane_validators(
    client: &Client,
    lane_id: LaneId,
    expected_active: &BTreeSet<ExpectedLaneValidatorBinding>,
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_total = 0usize;
    let mut last_active = BTreeSet::new();
    let mut last_error = None::<String>;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        match client.get_public_lane_validators(lane_id) {
            Ok(snapshot) => {
                let (total, active) = lane_validator_snapshot(&snapshot, context)?;
                last_total = total;
                last_active = active.clone();
                if total == expected_active.len() && active == *expected_active {
                    return Ok(());
                }
            }
            Err(err) => {
                last_error = Some(render_error_with_debug(&err));
            }
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    let suffix = last_error
        .map(|err| format!("; last query error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for active validators on lane {lane_id}; expected total {} active {:?}, observed total {} active {:?}{suffix}",
        expected_active.len(),
        expected_active,
        last_total,
        last_active
    ))
}

fn leader_or_highest_height_peer_index(
    network: &sandbox::SerializedNetwork,
    status_client: &Client,
) -> usize {
    let peers = network.peers();
    if peers.is_empty() {
        return 0;
    }

    if let Ok(status) = status_client.get_sumeragi_status() {
        if let Ok(index) = usize::try_from(status.leader) {
            if index < peers.len() {
                let leader_height = peers[index]
                    .client()
                    .get_sumeragi_status()
                    .map(|status| status.last_committed_height)
                    .unwrap_or(0);
                if leader_height.saturating_add(1) >= status.last_committed_height {
                    return index;
                }
            }
        }
    }

    peers
        .iter()
        .enumerate()
        .fold((0usize, 0u64), |best, (index, peer)| {
            let observed_height = peer
                .client()
                .get_sumeragi_status()
                .map(|status| status.last_committed_height)
                .unwrap_or(0);
            if observed_height >= best.1 {
                (index, observed_height)
            } else {
                best
            }
        })
        .0
}

fn lane_bounded_peer_index(
    network: &sandbox::SerializedNetwork,
    status_client: &Client,
    lane_index: u32,
) -> usize {
    lane_bounded_peer_indices(network, status_client, lane_index)
        .into_iter()
        .next()
        .unwrap_or_else(|| leader_or_highest_height_peer_index(network, status_client))
}

fn lane_bounded_peer_indices(
    network: &sandbox::SerializedNetwork,
    status_client: &Client,
    lane_index: u32,
) -> Vec<usize> {
    let peers = network.peers();
    if peers.is_empty() {
        return Vec::new();
    }

    let lane_index = lane_index as usize;
    let start = lane_index.saturating_mul(VALIDATORS_PER_LANE);
    let end = start.saturating_add(VALIDATORS_PER_LANE).min(peers.len());
    if start >= end {
        return vec![leader_or_highest_height_peer_index(network, status_client)];
    }

    let leader_index = status_client
        .get_sumeragi_status()
        .ok()
        .and_then(|status| usize::try_from(status.leader).ok());
    let mut ranked = (start..end)
        .map(|index| {
            let observed_height = peers[index]
                .client()
                .get_sumeragi_status()
                .map(|status| status.last_committed_height)
                .unwrap_or(0);
            (
                index,
                observed_height,
                leader_index.is_some_and(|leader| leader == index),
            )
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .2
            .cmp(&left.2)
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| left.0.cmp(&right.0))
    });
    ranked.into_iter().map(|(index, _, _)| index).collect()
}

fn leader_targeted_client_for_lane(
    network: &sandbox::SerializedNetwork,
    status_client: &Client,
    account_id: &AccountId,
    private_key: &PrivateKey,
    lane_index: u32,
) -> Client {
    let index = lane_bounded_peer_index(network, status_client, lane_index);
    network.peers()[index].client_for(account_id, private_key.clone())
}

fn lane_targeted_clients_for_lane(
    network: &sandbox::SerializedNetwork,
    status_client: &Client,
    account_id: &AccountId,
    private_key: &PrivateKey,
    lane_index: u32,
) -> Vec<Client> {
    lane_bounded_peer_indices(network, status_client, lane_index)
        .into_iter()
        .map(|index| network.peers()[index].client_for(account_id, private_key.clone()))
        .collect()
}

fn duration_min_avg_max_secs(samples: &[Duration]) -> Option<(f64, f64, f64)> {
    let mut iter = samples.iter();
    let first = iter.next()?;
    let mut min = first.as_secs_f64();
    let mut max = min;
    let mut total = min;
    let mut count = 1usize;
    for sample in iter {
        let secs = sample.as_secs_f64();
        min = min.min(secs);
        max = max.max(secs);
        total += secs;
        count += 1;
    }
    Some((min, total / count as f64, max))
}

fn is_inconclusive_blocking_submit_error(error_text: &str) -> bool {
    error_text.contains("transaction.status_timeout_ms")
        || error_text.contains("haven't got tx confirmation within")
        || error_text.contains("transaction queued for too long")
        || error_text.contains("Transaction submitter thread exited with error")
        || error_text.contains("Failed to send http POST request")
}

fn is_inconclusive_committed_outcome_error(error_text: &str) -> bool {
    error_text.contains("timed out waiting for committed transaction outcome")
}

fn is_expected_rollback_failure_text(error_text: &str) -> bool {
    error_text.contains("settlement leg requires 10000")
        || error_text.contains("requires 10000")
        || is_inconclusive_blocking_submit_error(error_text)
        || is_inconclusive_committed_outcome_error(error_text)
}

fn render_rejection_reason(
    reason: &iroha::data_model::transaction::error::TransactionRejectionReason,
) -> String {
    let display = reason.to_string();
    let debug = format!("{reason:?}");
    if display == debug {
        display
    } else {
        format!("{display}; details: {debug}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CommittedTxOutcome {
    Applied,
    Rejected(String),
}

fn committed_tx_outcome_quorum(
    outcomes: &[CommittedTxOutcome],
    quorum_required: usize,
) -> Option<CommittedTxOutcome> {
    let quorum_required = quorum_required.max(1);
    let applied_count = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, CommittedTxOutcome::Applied))
        .count();
    if applied_count >= quorum_required {
        return Some(CommittedTxOutcome::Applied);
    }

    let mut rejected_counts = BTreeMap::<&str, usize>::new();
    for outcome in outcomes {
        let CommittedTxOutcome::Rejected(reason) = outcome else {
            continue;
        };
        let count = rejected_counts.entry(reason.as_str()).or_default();
        *count = (*count).saturating_add(1);
        if *count >= quorum_required {
            return Some(CommittedTxOutcome::Rejected(reason.clone()));
        }
    }

    None
}

fn query_committed_tx_outcome(
    client: &Client,
    entry_hash: &HashOf<TransactionEntrypoint>,
) -> core::result::Result<Option<CommittedTxOutcome>, QueryError> {
    let one = NonZeroU64::new(1).expect("nonzero");
    let filters = CommittedTxFilters {
        entry_eq: Some(entry_hash.clone()),
        ..Default::default()
    };
    client
        .query(FindTransactions::new())
        .filter(CompoundPredicate::from_filters(filters))
        .with_pagination(Pagination::new(Some(one), 0))
        .with_fetch_size(FetchSize::new(Some(one)))
        .execute_all()
        .map(|snapshot| {
            snapshot.first().map(|tx| match &tx.result().0 {
                Ok(_) => CommittedTxOutcome::Applied,
                Err(reason) => CommittedTxOutcome::Rejected(render_rejection_reason(reason)),
            })
        })
}

fn wait_for_committed_tx_outcome_across_clients(
    mut clients_factory: impl FnMut() -> Vec<Client>,
    entry_hash: HashOf<TransactionEntrypoint>,
    context: &str,
    timeout_duration: Duration,
) -> Result<CommittedTxOutcome> {
    let started = Instant::now();
    let mut last_error: Option<String> = None;
    let mut last_observed = Vec::new();
    while started.elapsed() <= timeout_duration {
        let clients = clients_factory();
        let client_count = clients.len();
        let quorum_required = commit_quorum_from_len(client_count).max(1);
        let mut observed_outcomes = Vec::new();
        last_observed.clear();
        for (index, mut client) in clients.into_iter().enumerate() {
            client.torii_request_timeout =
                client
                    .torii_request_timeout
                    .min(bounded_observer_request_timeout(
                        started,
                        timeout_duration,
                        client_count.saturating_sub(index),
                    ));
            match query_committed_tx_outcome(&client, &entry_hash) {
                Ok(Some(outcome)) => {
                    last_observed.push(format!("{outcome:?}"));
                    observed_outcomes.push(outcome);
                    if let Some(quorum_outcome) =
                        committed_tx_outcome_quorum(&observed_outcomes, quorum_required)
                    {
                        return Ok(quorum_outcome);
                    }
                }
                Ok(None) => {
                    last_observed.push("none".to_owned());
                }
                Err(err) => {
                    last_error = Some(err.to_string());
                    last_observed.push(format!("error:{err}"));
                }
            }
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    let suffix = last_error
        .map(|err| format!("; last tx history query error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for committed transaction outcome quorum for transaction {entry_hash}; last observed {last_observed:?}{suffix}"
    ))
}

fn wait_for_committed_success_across_clients(
    clients_factory: impl FnMut() -> Vec<Client>,
    entry_hash: HashOf<TransactionEntrypoint>,
    context: &str,
    timeout_duration: Duration,
) -> Result<()> {
    match wait_for_committed_tx_outcome_across_clients(
        clients_factory,
        entry_hash.clone(),
        context,
        timeout_duration,
    )? {
        CommittedTxOutcome::Applied => Ok(()),
        CommittedTxOutcome::Rejected(reason) => Err(eyre!(
            "{context}: transaction {entry_hash} rejected unexpectedly: {reason}"
        )),
    }
}

fn wait_for_committed_rejection_reason_across_clients(
    clients_factory: impl FnMut() -> Vec<Client>,
    entry_hash: HashOf<TransactionEntrypoint>,
    context: &str,
    timeout_duration: Duration,
) -> Result<String> {
    match wait_for_committed_tx_outcome_across_clients(
        clients_factory,
        entry_hash.clone(),
        context,
        timeout_duration,
    )? {
        CommittedTxOutcome::Applied => Err(eyre!(
            "{context}: transaction {entry_hash} committed successfully, expected rejection"
        )),
        CommittedTxOutcome::Rejected(reason) => Ok(reason),
    }
}

fn submit_transaction_across_clients(
    mut clients_factory: impl FnMut() -> Vec<Client>,
    transaction: &SignedTransaction,
    context: &str,
    request_timeout: Duration,
) -> Result<HashOf<SignedTransaction>> {
    let mut last_error: Option<String> = None;
    let clients = clients_factory();
    ensure!(
        !clients.is_empty(),
        "{context}: no clients available for transaction submission"
    );
    for mut client in clients {
        client.torii_request_timeout = client.torii_request_timeout.min(request_timeout);
        match client.submit_transaction(transaction) {
            Ok(hash) => return Ok(hash),
            Err(err) => {
                last_error = Some(render_error_with_debug(&err));
            }
        }
    }
    let suffix = last_error
        .map(|err| format!("; last submit error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: failed to submit transaction through any routed observer{suffix}"
    ))
}

fn grant_exact_dvp_consents_across_clients(
    mut clients_factory: impl FnMut() -> Vec<Client>,
    settlements: &[(&DvpIsi, AssetId)],
    grantee: &AccountId,
    context: &str,
) -> Result<()> {
    ensure!(
        !settlements.is_empty(),
        "{context}: at least one exact settlement consent is required"
    );
    let builder = clients_factory()
        .into_iter()
        .next()
        .ok_or_else(|| eyre!("{context}: no counterparty client available"))?;
    let grants = settlements
        .iter()
        .map(|(settlement, debited_asset)| {
            InstructionBox::from(Grant::account_permission(
                CanExecuteSettlement {
                    debited_asset: debited_asset.clone(),
                    settlement_id: settlement.settlement_id().clone(),
                    intent_hash: settlement.intent_hash(),
                },
                grantee.clone(),
            ))
        })
        .collect::<Vec<_>>();
    let transaction = builder.build_transaction(
        grants,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        Metadata::default(),
    );
    let entrypoint_hash = transaction.hash_as_entrypoint();
    submit_transaction_across_clients(
        || clients_factory(),
        &transaction,
        &format!("{context}: submit exact consent"),
        SUBMIT_ENQUEUE_REQUEST_TIMEOUT,
    )?;
    wait_for_committed_success_across_clients(
        || clients_factory(),
        entrypoint_hash,
        &format!("{context}: wait for exact consent"),
        BLOCKING_CONFIRMATION_TIMEOUT,
    )
}

fn entrypoint_occurrences(
    client: &Client,
    entrypoint_hashes: &[HashOf<TransactionEntrypoint>],
    context: &str,
) -> Result<Vec<usize>> {
    ensure!(
        !entrypoint_hashes.is_empty(),
        "{context}: exact-once check requires at least one transaction"
    );
    let mut occurrences = vec![0usize; entrypoint_hashes.len()];
    for block in client
        .query(FindBlocks)
        .execute_all()
        .wrap_err_with(|| format!("{context}: query canonical blocks"))?
    {
        for observed in block.entrypoint_hashes() {
            for (index, expected) in entrypoint_hashes.iter().enumerate() {
                if observed == *expected {
                    occurrences[index] = occurrences[index].saturating_add(1);
                }
            }
        }
    }
    Ok(occurrences)
}

fn ensure_entrypoints_committed_once(
    client: &Client,
    entrypoint_hashes: &[HashOf<TransactionEntrypoint>],
    context: &str,
) -> Result<()> {
    let occurrences = entrypoint_occurrences(client, entrypoint_hashes, context)?;
    for (entrypoint_hash, count) in entrypoint_hashes.iter().zip(occurrences) {
        ensure!(
            count == 1,
            "{context}: expected one canonical application for {entrypoint_hash}, observed {count}"
        );
    }
    Ok(())
}

fn wait_for_entrypoints_committed_once_on_all_peers(
    network: &sandbox::SerializedNetwork,
    entrypoint_hashes: &[HashOf<TransactionEntrypoint>],
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_observed = Vec::new();
    while started.elapsed() <= LANE_PROGRESS_WAIT_TIMEOUT {
        last_observed.clear();
        let mut converged_peers = 0usize;
        for (peer_index, peer) in network.peers().iter().enumerate() {
            let mut client = peer.client();
            client.torii_request_timeout =
                client
                    .torii_request_timeout
                    .min(bounded_observer_request_timeout(
                        started,
                        LANE_PROGRESS_WAIT_TIMEOUT,
                        network.peers().len().saturating_sub(peer_index),
                    ));
            match entrypoint_occurrences(
                &client,
                entrypoint_hashes,
                &format!("{context}: peer {peer_index}"),
            ) {
                Ok(occurrences) => {
                    ensure!(
                        occurrences.iter().all(|count| *count <= 1),
                        "{context}: peer {peer_index} observed duplicate canonical applications {occurrences:?}"
                    );
                    if occurrences.iter().all(|count| *count == 1) {
                        converged_peers = converged_peers.saturating_add(1);
                    }
                    last_observed.push(format!("peer#{peer_index}:{occurrences:?}"));
                }
                Err(err) => {
                    last_observed.push(format!("peer#{peer_index}:error={err}"));
                }
            }
        }
        if converged_peers == network.peers().len() {
            for (peer_index, peer) in network.peers().iter().enumerate() {
                ensure_entrypoints_committed_once(
                    &peer.client(),
                    entrypoint_hashes,
                    &format!("{context}: peer {peer_index} final exact-once check"),
                )?;
            }
            return Ok(());
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "{context}: timed out waiting for exact-once canonical history on all {} peers; last observed {last_observed:?}",
        network.peers().len()
    ))
}

fn durable_native_participant_row(
    diagnostics: &SumeragiDiagnosticsStatus,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    context: &str,
) -> Result<Option<SumeragiNativeAmxParticipantApplication>> {
    ensure!(
        !diagnostics
            .native_amx_participant_applications
            .iter()
            .any(|row| row.state == SumeragiNativeAmxParticipantApplicationState::Conflict),
        "{context}: Native AMX diagnostics reported conflicting participant evidence"
    );
    let matching = diagnostics
        .native_amx_participant_applications
        .iter()
        .filter(|row| row.lane_id == lane_id && row.dataspace_id == dataspace_id)
        .copied()
        .collect::<Vec<_>>();
    ensure!(
        matching.len() <= 1,
        "{context}: expected at most one active Native AMX participant row for lane {} dataspace {}, got {}",
        lane_id.as_u32(),
        dataspace_id.as_u64(),
        matching.len()
    );
    let Some(row) = matching.first().copied() else {
        return Ok(None);
    };
    if row.state != SumeragiNativeAmxParticipantApplicationState::DurablyApplied {
        return Ok(None);
    }
    row.validate()
        .map_err(|err| eyre!("{context}: invalid durable Native AMX participant row: {err}"))?;
    ensure!(
        row.application_block_height.is_some() && row.application_block_hash.is_some(),
        "{context}: durable Native AMX participant row omitted canonical carrier identity"
    );
    Ok(Some(row))
}

fn durable_native_participant_evidence_is_after_baseline(
    row: &SumeragiNativeAmxParticipantApplication,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    expected_incarnation: Hash,
    baseline: Option<&SumeragiNativeAmxParticipantApplication>,
    minimum_application_block_height_exclusive: Option<u64>,
) -> Result<bool> {
    ensure!(
        row.lane_id == lane_id && row.dataspace_id == dataspace_id,
        "Native AMX participant evidence belongs to another route"
    );
    ensure!(
        row.lane_incarnation == expected_incarnation,
        "Native AMX participant evidence belongs to a stale lane incarnation"
    );
    ensure!(
        row.state == SumeragiNativeAmxParticipantApplicationState::DurablyApplied,
        "Native AMX participant evidence is not durably applied"
    );
    row.validate()
        .map_err(|err| eyre!("invalid durable Native AMX participant evidence: {err}"))?;
    let application_block_height = row
        .application_block_height
        .ok_or_else(|| eyre!("durable Native AMX participant evidence has no carrier height"))?;
    if minimum_application_block_height_exclusive
        .is_some_and(|minimum| application_block_height <= minimum)
    {
        return Ok(false);
    }

    let Some(baseline) = baseline else {
        return Ok(true);
    };
    ensure!(
        baseline.lane_id == lane_id
            && baseline.dataspace_id == dataspace_id
            && baseline.lane_incarnation == expected_incarnation,
        "Native AMX participant baseline belongs to another route or incarnation"
    );
    ensure!(
        baseline.state == SumeragiNativeAmxParticipantApplicationState::DurablyApplied,
        "Native AMX participant baseline is not durably applied"
    );
    baseline
        .validate()
        .map_err(|err| eyre!("invalid durable Native AMX participant baseline: {err}"))?;
    match row.participant_height.cmp(&baseline.participant_height) {
        std::cmp::Ordering::Less => Ok(false),
        std::cmp::Ordering::Equal => {
            ensure!(
                row == baseline,
                "Native AMX participant evidence conflicts with the baseline at the same height"
            );
            Ok(false)
        }
        std::cmp::Ordering::Greater => Ok(true),
    }
}

fn wait_for_durable_native_participant_evidence_after(
    network: &sandbox::SerializedNetwork,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    expected_incarnation: Hash,
    baseline: Option<&SumeragiNativeAmxParticipantApplication>,
    minimum_application_block_height_exclusive: Option<u64>,
    context: &str,
) -> Result<SumeragiNativeAmxParticipantApplication> {
    let started = Instant::now();
    let mut last_observed = Vec::new();
    while started.elapsed() <= LANE_PROGRESS_WAIT_TIMEOUT {
        let mut rows = Vec::with_capacity(network.peers().len());
        last_observed.clear();
        for (peer_index, peer) in network.peers().iter().enumerate() {
            let mut client = peer.client();
            client.torii_request_timeout =
                client
                    .torii_request_timeout
                    .min(bounded_observer_request_timeout(
                        started,
                        LANE_PROGRESS_WAIT_TIMEOUT,
                        network.peers().len().saturating_sub(peer_index),
                    ));
            match client.get_sumeragi_diagnostics() {
                Ok(diagnostics) => match durable_native_participant_row(
                    &diagnostics,
                    lane_id,
                    dataspace_id,
                    context,
                ) {
                    Ok(Some(row)) => match durable_native_participant_evidence_is_after_baseline(
                        &row,
                        lane_id,
                        dataspace_id,
                        expected_incarnation,
                        baseline,
                        minimum_application_block_height_exclusive,
                    ) {
                        Ok(true) => {
                            last_observed.push(format!(
                                "peer#{peer_index}: durable height={} view={} source_count={} block={:?}",
                                row.participant_height,
                                row.participant_view,
                                row.source_count,
                                row.application_block_height
                            ));
                            rows.push(row);
                        }
                        Ok(false) => {
                            last_observed.push(format!(
                                "peer#{peer_index}: durable evidence has not advanced (height={} block={:?})",
                                row.participant_height, row.application_block_height
                            ));
                        }
                        Err(err) => {
                            return Err(err).wrap_err_with(|| {
                                format!("{context}: peer {peer_index} Native participant evidence")
                            });
                        }
                    },
                    Ok(None) => {
                        last_observed.push(format!("peer#{peer_index}: not durable"));
                    }
                    Err(err) => return Err(err),
                },
                Err(err) => {
                    last_observed.push(format!("peer#{peer_index}: diagnostics error={err}"));
                }
            }
        }
        if rows.len() == network.peers().len() {
            let expected = rows[0];
            ensure!(
                rows.iter().all(|row| *row == expected),
                "{context}: peers exposed different durable Native AMX participant identities: {rows:?}"
            );
            return Ok(expected);
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "{context}: timed out waiting for strictly newer durable Native AMX participant evidence on all {} peers; last observed {last_observed:?}",
        network.peers().len()
    ))
}

fn rotating_validator_indices(seed_ordinal: usize, iteration: usize) -> [usize; 3] {
    let lane_offset = seed_ordinal
        .saturating_add(iteration)
        .wrapping_rem(VALIDATORS_PER_LANE);
    [
        lane_offset,
        VALIDATORS_PER_LANE + lane_offset,
        (VALIDATORS_PER_LANE * 2) + lane_offset,
    ]
}

fn shutdown_rotated_validators(
    runtime: &Runtime,
    peers: &[NetworkPeer],
    context: &str,
) -> Result<()> {
    runtime.block_on(async {
        for peer in peers {
            ensure!(
                peer.shutdown_if_started().await,
                "{context}: selected validator was not running before outage"
            );
        }
        Ok(())
    })
}

fn restart_rotated_validators(
    runtime: &Runtime,
    peers: &[NetworkPeer],
    config_layers: &[ConfigLayer],
    context: &str,
) -> Result<()> {
    runtime.block_on(async {
        let mut failures = Vec::new();
        for peer in peers {
            if let Err(err) = peer
                .start_checked(config_layers.iter().cloned(), None)
                .await
                .wrap_err_with(|| format!("{context}: restart validator {}", peer.id()))
            {
                failures.push(err.to_string());
            }
        }
        ensure!(
            failures.is_empty(),
            "{context}: one or more validators failed to restart: {failures:?}"
        );
        Ok(())
    })
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AutoscaleDrainIntentLog {
    height: u64,
    close_global_height: u64,
    initial_merged_lane_height: u64,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct AutoscaleDrainCommitmentLog {
    height: u64,
    carrier_height: u64,
    final_lane_block_height: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct AutoscaleLifecycleLog {
    scale_out_heights: BTreeSet<u64>,
    drain_intents: BTreeSet<AutoscaleDrainIntentLog>,
    drain_commitments: BTreeSet<AutoscaleDrainCommitmentLog>,
    scale_in_heights: BTreeSet<u64>,
}

#[derive(Clone, Debug)]
struct AutoscaleAutonomousEvidence {
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    merge_entry: MergeLedgerEntry,
    lane_block_height: u64,
    descriptor_hash: Hash,
}

fn autoscale_storage_segment() -> String {
    format!("lane_{AUTOSCALE_LANE_INDEX:03}_elastic_lane_{AUTOSCALE_LANE_INDEX}")
}

fn active_lane_storage_ids(peer: &NetworkPeer) -> Result<BTreeSet<u32>> {
    let root = peer.kura_store_dir().join("blocks");
    if !root.exists() {
        return Ok(BTreeSet::new());
    }
    let mut ids = BTreeSet::new();
    let mut lane_directory_count = 0_usize;
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        let Some(rest) = name.strip_prefix("lane_") else {
            continue;
        };
        let Some(digits) = rest.get(..3) else {
            continue;
        };
        if !digits.bytes().all(|byte| byte.is_ascii_digit())
            || rest.get(3..).is_none_or(|suffix| !suffix.starts_with('_'))
        {
            continue;
        }
        lane_directory_count = lane_directory_count.saturating_add(1);
        ids.insert(
            digits
                .parse()
                .map_err(|err| eyre!("parse lane storage directory {name}: {err}"))?,
        );
    }
    ensure!(
        ids.len() == lane_directory_count,
        "peer Kura contains duplicate lane storage identifiers"
    );
    Ok(ids)
}

fn expected_active_lane_storage_ids(expanded: bool) -> BTreeSet<u32> {
    let mut expected = [NEXUS_LANE_INDEX, DS1_LANE_INDEX, DS2_LANE_INDEX]
        .into_iter()
        .collect::<BTreeSet<_>>();
    if expanded {
        expected.insert(AUTOSCALE_LANE_INDEX);
    }
    expected
}

fn all_peers_have_lane_storage_profile(
    network: &sandbox::SerializedNetwork,
    expanded: bool,
) -> Result<bool> {
    let expected = expected_active_lane_storage_ids(expanded);
    Ok(network
        .peers()
        .iter()
        .map(active_lane_storage_ids)
        .collect::<Result<Vec<_>>>()?
        .iter()
        .all(|ids| ids == &expected))
}

fn wait_for_autoscale_baseline(network: &sandbox::SerializedNetwork, context: &str) -> Result<()> {
    let started = Instant::now();
    let mut last_storage = Vec::new();
    let mut last_diagnostics = Vec::new();
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        last_storage = network
            .peers()
            .iter()
            .map(active_lane_storage_ids)
            .collect::<Result<Vec<_>>>()?;
        last_diagnostics.clear();
        let mut endpoints_ready = true;
        for (index, peer) in network.peers().iter().enumerate() {
            let client = peer.client();
            match (
                client.get_sumeragi_status(),
                client.get_sumeragi_diagnostics(),
            ) {
                (Ok(status), Ok(diagnostics)) => {
                    if let Err(err) = status.validate() {
                        endpoints_ready = false;
                        last_diagnostics.push(format!("peer#{index}:invalid-status={err:?}"));
                        continue;
                    }
                    let elastic_governance = diagnostics
                        .lane_governance
                        .iter()
                        .filter(|row| row.lane_id == LaneId::new(AUTOSCALE_LANE_INDEX))
                        .count();
                    let elastic_blocks = diagnostics
                        .committed_lane_blocks
                        .iter()
                        .filter(|row| row.lane_id == LaneId::new(AUTOSCALE_LANE_INDEX))
                        .count();
                    if status.restart_required || elastic_governance != 0 || elastic_blocks != 0 {
                        endpoints_ready = false;
                    }
                    last_diagnostics.push(format!(
                        "peer#{index}:height={} restart={} elastic_governance={} elastic_blocks={}",
                        status.last_committed_height,
                        status.restart_required,
                        elastic_governance,
                        elastic_blocks
                    ));
                }
                (status, diagnostics) => {
                    endpoints_ready = false;
                    last_diagnostics.push(format!(
                        "peer#{index}:status={:?} diagnostics={:?}",
                        status.as_ref().err().map(ToString::to_string),
                        diagnostics.as_ref().err().map(ToString::to_string)
                    ));
                }
            }
        }
        if endpoints_ready
            && last_storage
                .iter()
                .all(|ids| ids == &expected_active_lane_storage_ids(false))
        {
            return Ok(());
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "{context}: baseline did not converge to exactly lanes 0,1,2 with no active lane-3 diagnostics; storage={last_storage:?}; endpoints={last_diagnostics:?}"
    ))
}

fn peer_latest_stdout_log(peer: &NetworkPeer) -> Result<Option<PathBuf>> {
    let kura_store_dir = peer.kura_store_dir();
    let root = kura_store_dir
        .parent()
        .ok_or_else(|| eyre!("derive peer root from Kura store"))?;
    let mut latest = None::<(u64, PathBuf)>;
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        let Some(run_id) = name
            .strip_prefix("run-")
            .and_then(|value| value.strip_suffix("-stdout.log"))
            .and_then(|value| value.parse::<u64>().ok())
        else {
            continue;
        };
        if latest
            .as_ref()
            .is_none_or(|(latest_id, _)| run_id > *latest_id)
        {
            latest = Some((run_id, entry.path()));
        }
    }
    Ok(latest.map(|(_, path)| path))
}

fn read_bounded_log_tail(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(AUTOSCALE_LOG_TAIL_MAX_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::with_capacity(usize::try_from(len.saturating_sub(start))?);
    file.take(AUTOSCALE_LOG_TAIL_MAX_BYTES)
        .read_to_end(&mut bytes)?;
    if start > 0
        && let Some(first_newline) = bytes.iter().position(|byte| *byte == b'\n')
    {
        bytes.drain(..=first_newline);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn log_unsigned_field(line: &str, field: &str) -> Option<u64> {
    let prefixes = [
        format!("{field}="),
        format!("{field}:"),
        format!("\"{field}\":"),
    ];
    let mut observed = Vec::new();
    for prefix in prefixes {
        for (offset, _) in line.match_indices(prefix.as_str()) {
            if offset > 0
                && line[..offset]
                    .chars()
                    .next_back()
                    .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            {
                continue;
            }
            let raw = &line[offset + prefix.len()..];
            let raw = raw.trim_start_matches(|ch: char| ch.is_ascii_whitespace());
            let digit_count = raw.bytes().take_while(u8::is_ascii_digit).count();
            if digit_count == 0 {
                observed.push(None);
                continue;
            }
            if raw[digit_count..].chars().next().is_some_and(|ch| {
                !ch.is_ascii_whitespace() && !matches!(ch, ',' | ';' | '}' | ']' | ')')
            }) {
                observed.push(None);
                continue;
            }
            observed.push(raw[..digit_count].parse::<u64>().ok());
        }
    }
    match observed.as_slice() {
        [Some(value)] => Some(*value),
        _ => None,
    }
}

fn parse_autoscale_lifecycle_log(contents: &str) -> AutoscaleLifecycleLog {
    let mut evidence = AutoscaleLifecycleLog::default();
    for line in contents.lines() {
        if log_unsigned_field(line, "lane") != Some(u64::from(AUTOSCALE_LANE_INDEX)) {
            continue;
        }
        if line.contains(AUTOSCALE_SCALE_OUT_LOG_MARKER) {
            if let Some(height) = log_unsigned_field(line, "height") {
                evidence.scale_out_heights.insert(height);
            }
        } else if line.contains(AUTOSCALE_DRAIN_INTENT_LOG_MARKER) {
            if let (Some(height), Some(close_global_height), Some(initial_merged_lane_height)) = (
                log_unsigned_field(line, "height"),
                log_unsigned_field(line, "close_global_height"),
                log_unsigned_field(line, "initial_merged_lane_height"),
            ) && height == close_global_height
            {
                evidence.drain_intents.insert(AutoscaleDrainIntentLog {
                    height,
                    close_global_height,
                    initial_merged_lane_height,
                });
            }
        } else if line.contains(AUTOSCALE_DRAIN_COMMITMENT_LOG_MARKER) {
            if let (Some(height), Some(carrier_height), Some(final_lane_block_height)) = (
                log_unsigned_field(line, "height"),
                log_unsigned_field(line, "carrier_height"),
                log_unsigned_field(line, "final_lane_block_height"),
            ) && height == carrier_height
            {
                evidence
                    .drain_commitments
                    .insert(AutoscaleDrainCommitmentLog {
                        height,
                        carrier_height,
                        final_lane_block_height,
                    });
            }
        } else if line.contains(AUTOSCALE_SCALE_IN_LOG_MARKER)
            && let Some(height) = log_unsigned_field(line, "height")
        {
            evidence.scale_in_heights.insert(height);
        }
    }
    evidence
}

fn peer_autoscale_lifecycle_log(peer: &NetworkPeer) -> Result<AutoscaleLifecycleLog> {
    let Some(path) = peer_latest_stdout_log(peer)? else {
        return Ok(AutoscaleLifecycleLog::default());
    };
    match read_bounded_log_tail(&path) {
        Ok(contents) => Ok(parse_autoscale_lifecycle_log(&contents)),
        Err(err) if matches!(err.downcast_ref::<std::io::Error>(), Some(io) if io.kind() == std::io::ErrorKind::NotFound) => {
            Ok(AutoscaleLifecycleLog::default())
        }
        Err(err) => {
            Err(err).wrap_err_with(|| format!("read bounded peer log tail {}", path.display()))
        }
    }
}

fn read_lane_incarnation_marker(path: &Path) -> Result<LaneIncarnationMarkerV3> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("read lane incarnation marker {}", path.display()))?;
    let mut cursor = bytes.as_slice();
    let marker = LaneIncarnationMarkerV3::decode_all(&mut cursor)
        .map_err(|err| eyre!("decode lane incarnation marker {}: {err}", path.display()))?;
    ensure!(
        marker.encode() == bytes,
        "{} is not a canonical lane incarnation marker",
        path.display()
    );
    ensure!(
        marker.version == LANE_INCARNATION_MARKER_VERSION,
        "{} uses marker version {}, expected {}",
        path.display(),
        marker.version,
        LANE_INCARNATION_MARKER_VERSION
    );
    ensure!(
        marker.lane_id == LaneId::new(AUTOSCALE_LANE_INDEX)
            && marker.incarnation.as_ref().iter().any(|byte| *byte != 0),
        "{} does not bind the live non-zero lane-3 incarnation",
        path.display()
    );
    Ok(marker)
}

fn active_autoscale_marker_path(peer: &NetworkPeer) -> PathBuf {
    peer.kura_store_dir()
        .join("blocks")
        .join(autoscale_storage_segment())
        .join(LANE_INCARNATION_MARKER_FILE)
}

fn active_autoscale_marker(peer: &NetworkPeer) -> Result<LaneIncarnationMarkerV3> {
    read_lane_incarnation_marker(&active_autoscale_marker_path(peer))
}

fn converged_active_autoscale_marker(
    network: &sandbox::SerializedNetwork,
) -> Result<LaneIncarnationMarkerV3> {
    let markers = network
        .peers()
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            active_autoscale_marker(peer)
                .wrap_err_with(|| format!("read active lane-3 marker on peer {index}"))
        })
        .collect::<Result<Vec<_>>>()?;
    let expected = markers
        .first()
        .ok_or_else(|| eyre!("lane-3 marker convergence has no peers"))?;
    ensure!(
        markers.iter().all(|marker| marker == expected),
        "active lane-3 incarnation markers diverged: {markers:?}"
    );
    Ok(expected.clone())
}

fn archived_autoscale_markers(
    peer: &NetworkPeer,
) -> Result<Vec<(PathBuf, LaneIncarnationMarkerV3)>> {
    let root = peer.kura_store_dir().join("retired").join("lane_geometry");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut transitions = fs::read_dir(&root)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    transitions.sort();
    ensure!(
        transitions.len() <= 1_024,
        "retired lane geometry exceeds the bounded lifecycle inspection limit"
    );
    let mut markers = Vec::new();
    for transition in transitions {
        let transition_metadata = fs::symlink_metadata(&transition)?;
        ensure!(
            transition_metadata.is_dir() && !transition_metadata.file_type().is_symlink(),
            "retired geometry transition is not a regular directory: {}",
            transition.display()
        );
        let marker_path = transition
            .join(format!("lane_{AUTOSCALE_LANE_INDEX:010}"))
            .join("previous_blocks")
            .join(LANE_INCARNATION_MARKER_FILE);
        if !marker_path.exists() {
            continue;
        }
        let metadata = fs::symlink_metadata(&marker_path)?;
        ensure!(
            metadata.is_file() && !metadata.file_type().is_symlink(),
            "retired lane marker is not a regular file: {}",
            marker_path.display()
        );
        markers.push((
            marker_path.clone(),
            read_lane_incarnation_marker(&marker_path)?,
        ));
    }
    markers.sort_by_key(|(_, marker)| (marker.activation_height, marker.incarnation));
    ensure!(
        markers
            .windows(2)
            .all(|pair| pair[0].1.incarnation != pair[1].1.incarnation),
        "retired geometry duplicated one lane-3 incarnation"
    );
    Ok(markers)
}

fn archived_autoscale_marker_for_incarnation(
    peer: &NetworkPeer,
    incarnation: Hash,
) -> Result<Option<(PathBuf, LaneIncarnationMarkerV3)>> {
    let matches = archived_autoscale_markers(peer)?
        .into_iter()
        .filter(|(_, marker)| marker.incarnation == incarnation)
        .collect::<Vec<_>>();
    ensure!(
        matches.len() <= 1,
        "peer retained multiple archives for lane-3 incarnation {incarnation}"
    );
    Ok(matches.into_iter().next())
}

fn read_peer_merge_ledger_entries(peer: &NetworkPeer) -> Result<Vec<MergeLedgerEntry>> {
    let root = peer.kura_store_dir().join("merge_ledger");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(&root)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.path().extension().and_then(|value| value.to_str()) == Some("log")
        {
            paths.push(entry.path());
        }
    }
    paths.sort();
    let mut by_epoch = BTreeMap::<u64, MergeLedgerEntry>::new();
    for path in paths {
        let bytes = fs::read(&path)?;
        let mut cursor = 0_usize;
        while bytes.len().saturating_sub(cursor) >= core::mem::size_of::<u32>() {
            let mut length = [0_u8; core::mem::size_of::<u32>()];
            length.copy_from_slice(&bytes[cursor..cursor + core::mem::size_of::<u32>()]);
            let payload_len =
                usize::try_from(u32::from_le_bytes(length)).expect("u32 frame length fits usize");
            ensure!(
                (1..=MAX_MERGE_LEDGER_ENTRY_BYTES).contains(&payload_len),
                "{} contains invalid merge frame length {payload_len}",
                path.display()
            );
            let payload_start = cursor + core::mem::size_of::<u32>();
            let Some(payload_end) = payload_start.checked_add(payload_len) else {
                return Err(eyre!("merge frame offset overflow in {}", path.display()));
            };
            if payload_end > bytes.len() {
                break;
            }
            let payload = &bytes[payload_start..payload_end];
            let entry: MergeLedgerEntry = norito::decode_from_bytes(payload).map_err(|err| {
                eyre!(
                    "decode merge frame at {} offset {cursor}: {err}",
                    path.display()
                )
            })?;
            ensure!(
                entry.canonical_bytes() == payload,
                "{} contains a non-canonical merge frame at offset {cursor}",
                path.display()
            );
            match by_epoch.entry(entry.epoch_id) {
                std::collections::btree_map::Entry::Vacant(slot) => {
                    slot.insert(entry);
                }
                std::collections::btree_map::Entry::Occupied(slot) => ensure!(
                    slot.get() == &entry,
                    "merge epoch {} has conflicting durable bytes",
                    entry.epoch_id
                ),
            }
            cursor = payload_end;
        }
    }
    Ok(by_epoch.into_values().collect())
}

fn autonomous_merge_entry(
    peer: &NetworkPeer,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
) -> Result<Option<MergeLedgerEntry>> {
    let matches = read_peer_merge_ledger_entries(peer)?
        .into_iter()
        .filter(|entry| {
            entry.execution_batch.as_ref().is_some_and(|batch| {
                batch.lanes.iter().any(|execution| {
                    execution
                        .entrypoints
                        .iter()
                        .any(|entrypoint| entrypoint.hash() == entrypoint_hash)
                })
            })
        })
        .collect::<Vec<_>>();
    ensure!(
        matches.len() <= 1,
        "entrypoint {entrypoint_hash} occurs in multiple durable merge entries"
    );
    Ok(matches.into_iter().next())
}

fn drain_merge_entry(peer: &NetworkPeer, incarnation: Hash) -> Result<Option<MergeLedgerEntry>> {
    let target_lane = LaneId::new(AUTOSCALE_LANE_INDEX);
    let matches = read_peer_merge_ledger_entries(peer)?
        .into_iter()
        .filter(|entry| {
            entry
                .lane_drain_certificates
                .first()
                .is_some_and(|certificate| {
                    certificate.body.intent.lane_id == target_lane
                        && certificate.body.intent.lane_incarnation == incarnation
                })
        })
        .collect::<Vec<_>>();
    ensure!(
        matches.len() <= 1,
        "lane-3 incarnation {incarnation} has conflicting drain merge entries"
    );
    Ok(matches.into_iter().next())
}

fn submit_autoscale_load(clients: &[Client], cycle: usize, tx_count: usize) -> Result<()> {
    ensure!(!clients.is_empty(), "autoscale load has no submitters");
    let mut accepted = 0_usize;
    let mut first_error = None;
    for index in 0..tx_count {
        let client = &clients[index % clients.len()];
        match client.submit(
            Log::new(
                Level::INFO,
                format!("g12p-autoscale-cycle-{cycle}-load-{index}"),
            ),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        ) {
            Ok(_) => accepted = accepted.saturating_add(1),
            Err(err) if first_error.is_none() => first_error = Some(err.to_string()),
            Err(_) => {}
        }
    }
    ensure!(
        accepted > 0,
        "autoscale cycle {cycle} rejected all {tx_count} load transactions: {first_error:?}"
    );
    eprintln!(
        "[g12p-autoscale] cycle={cycle} load accepted={accepted}/{tx_count} first_error={first_error:?}"
    );
    Ok(())
}

fn wait_for_autoscale_expansion(
    network: &sandbox::SerializedNetwork,
    load_clients: &[Client],
    cycle: usize,
) -> Result<LaneIncarnationMarkerV3> {
    submit_autoscale_load(load_clients, cycle, AUTOSCALE_LOAD_TX_COUNT)?;
    let started = Instant::now();
    let mut last_storage = Vec::new();
    let mut last_endpoints = Vec::new();
    let mut last_logs = Vec::new();
    let mut last_error = None;
    let mut next_top_up = Duration::from_secs(15);
    while started.elapsed() <= AUTOSCALE_SCALE_OUT_WAIT_TIMEOUT {
        last_storage = network
            .peers()
            .iter()
            .map(active_lane_storage_ids)
            .collect::<Result<Vec<_>>>()?;
        let storage_ready = last_storage
            .iter()
            .all(|ids| ids == &expected_active_lane_storage_ids(true));
        let marker = storage_ready
            .then(|| converged_active_autoscale_marker(network))
            .transpose();
        last_endpoints.clear();
        let mut endpoints_ready = true;
        for (index, peer) in network.peers().iter().enumerate() {
            let client = peer.client();
            match (
                client.get_sumeragi_status(),
                client.get_sumeragi_diagnostics(),
            ) {
                (Ok(status), Ok(diagnostics)) => {
                    let governance_rows = diagnostics
                        .lane_governance
                        .iter()
                        .filter(|row| row.lane_id == LaneId::new(AUTOSCALE_LANE_INDEX))
                        .collect::<Vec<_>>();
                    if status.validate().is_err()
                        || status.restart_required
                        || governance_rows.len() != 1
                        || governance_rows[0].alias
                            != format!("elastic-lane-{AUTOSCALE_LANE_INDEX}")
                    {
                        endpoints_ready = false;
                    }
                    last_endpoints.push(format!(
                        "peer#{index}:height={} governance={:?}",
                        status.last_committed_height,
                        governance_rows
                            .iter()
                            .map(|row| (&row.alias, row.manifest_required, row.manifest_ready))
                            .collect::<Vec<_>>()
                    ));
                }
                (status, diagnostics) => {
                    endpoints_ready = false;
                    last_endpoints.push(format!(
                        "peer#{index}:status={:?} diagnostics={:?}",
                        status.as_ref().err().map(ToString::to_string),
                        diagnostics.as_ref().err().map(ToString::to_string)
                    ));
                }
            }
        }
        last_logs = network
            .peers()
            .iter()
            .map(peer_autoscale_lifecycle_log)
            .collect::<Result<Vec<_>>>()?;
        match marker {
            Ok(Some(marker)) => {
                let transition_ready = last_logs
                    .iter()
                    .all(|log| log.scale_out_heights.contains(&marker.activation_height));
                if endpoints_ready && transition_ready {
                    eprintln!(
                        "[g12p-autoscale] cycle={cycle} expanded lane={} incarnation={} activation_height={}",
                        AUTOSCALE_LANE_INDEX, marker.incarnation, marker.activation_height
                    );
                    return Ok(marker);
                }
            }
            Ok(None) => {}
            Err(err) => last_error = Some(err.to_string()),
        }
        if started.elapsed() >= next_top_up {
            if let Err(err) = submit_autoscale_load(load_clients, cycle, 128) {
                last_error = Some(err.to_string());
            }
            next_top_up += Duration::from_secs(15);
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "autoscale cycle {cycle}: timed out waiting for automatic 3->4 expansion on all {TOTAL_PEERS} peers through status, diagnostics, bounded logs, and Kura markers; storage={last_storage:?}; endpoints={last_endpoints:?}; logs={last_logs:?}; last_error={last_error:?}"
    ))
}

fn recreated_autoscale_lane_diagnostics_ready(
    diagnostics: &SumeragiDiagnosticsStatus,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    expected_incarnation: Hash,
) -> bool {
    let committed = diagnostics
        .committed_lane_blocks
        .iter()
        .filter(|row| row.lane_id == lane_id)
        .collect::<Vec<_>>();
    if committed.is_empty()
        || committed.iter().any(|row| {
            row.lane_incarnation != expected_incarnation
                || !committed_lane_block_has_expected_quorum(row, VALIDATORS_PER_LANE)
        })
    {
        return false;
    }

    latest_lane_domain_progress(diagnostics, lane_id, dataspace_id).is_some_and(|latest| {
        latest.lane_incarnation == expected_incarnation
            && latest.executable_payload_available
            && committed_lane_block_status_counts_as_progress(
                &latest.execution_status,
                latest.executable_payload_available,
            )
    })
}

fn wait_for_recreated_autoscale_lane_ready(
    network: &sandbox::SerializedNetwork,
    load_clients: &[Client],
    expected_marker: &LaneIncarnationMarkerV3,
    context: &str,
) -> Result<()> {
    ensure!(
        !load_clients.is_empty(),
        "{context}: recreated-lane readiness has no load clients"
    );
    let started = Instant::now();
    let mut last_storage = Vec::new();
    let mut last_marker = None;
    let mut last_endpoints = Vec::new();
    let mut last_load_error = None;
    let mut next_top_up = Duration::ZERO;
    while started.elapsed() <= AUTOSCALE_SCALE_OUT_WAIT_TIMEOUT {
        if started.elapsed() >= next_top_up {
            if let Err(err) = submit_autoscale_load(load_clients, 2, 32) {
                last_load_error = Some(err.to_string());
            }
            next_top_up += Duration::from_secs(2);
        }
        last_storage = network
            .peers()
            .iter()
            .map(active_lane_storage_ids)
            .collect::<Result<Vec<_>>>()?;
        let storage_ready = last_storage
            .iter()
            .all(|ids| ids == &expected_active_lane_storage_ids(true));
        let marker_ready = if storage_ready {
            match converged_active_autoscale_marker(network) {
                Ok(marker) => {
                    let matches = marker == *expected_marker;
                    last_marker = Some(format!("{marker:?}"));
                    matches
                }
                Err(err) => {
                    last_marker = Some(format!("error={err}"));
                    false
                }
            }
        } else {
            false
        };
        last_endpoints.clear();
        let mut endpoints_ready = true;
        for (index, peer) in network.peers().iter().enumerate() {
            match (
                peer.client().get_sumeragi_status(),
                peer.client().get_sumeragi_diagnostics(),
            ) {
                (Ok(status), Ok(diagnostics)) => {
                    let governance = diagnostics
                        .lane_governance
                        .iter()
                        .filter(|row| row.lane_id == expected_marker.lane_id)
                        .collect::<Vec<_>>();
                    let committed = diagnostics
                        .committed_lane_blocks
                        .iter()
                        .filter(|row| row.lane_id == expected_marker.lane_id)
                        .collect::<Vec<_>>();
                    let peer_ready = status.validate().is_ok()
                        && !status.restart_required
                        && status.last_committed_height >= expected_marker.activation_height
                        && governance.len() == 1
                        && governance[0].alias == format!("elastic-lane-{AUTOSCALE_LANE_INDEX}")
                        && recreated_autoscale_lane_diagnostics_ready(
                            &diagnostics,
                            expected_marker.lane_id,
                            DataSpaceId::new(NEXUS_ID_U64),
                            expected_marker.incarnation,
                        );
                    endpoints_ready &= peer_ready;
                    last_endpoints.push(format!(
                        "peer#{index}:height={} restart={} governance={} committed={} ready={peer_ready}",
                        status.last_committed_height,
                        status.restart_required,
                        governance.len(),
                        committed.len(),
                    ));
                }
                (status, diagnostics) => {
                    endpoints_ready = false;
                    last_endpoints.push(format!(
                        "peer#{index}:status={:?} diagnostics={:?}",
                        status.as_ref().err().map(ToString::to_string),
                        diagnostics.as_ref().err().map(ToString::to_string),
                    ));
                }
            }
        }
        if storage_ready && marker_ready && endpoints_ready {
            return Ok(());
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "{context}: recreated lane-3 incarnation did not become ready on every peer after restart; expected={expected_marker:?}; storage={last_storage:?}; marker={last_marker:?}; endpoints={last_endpoints:?}; last_load_error={last_load_error:?}"
    ))
}

fn build_default_route_transaction_for_autoscale_lane(
    client: &Client,
    cycle: usize,
) -> Result<SignedTransaction> {
    (0_u64..4_096)
        .find_map(|nonce| {
            let transaction = client.build_transaction(
                [Log::new(
                    Level::INFO,
                    format!("g12p-autoscale-autonomous-{cycle}-{nonce}"),
                )],
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            );
            let hash = HashOf::new(transaction.payload());
            let mut shard = [0_u8; core::mem::size_of::<u64>()];
            shard.copy_from_slice(&hash.as_ref()[..core::mem::size_of::<u64>()]);
            (u64::from_le_bytes(shard) % AUTOSCALE_DEFAULT_ROUTE_SHARD_COUNT
                == AUTOSCALE_DEFAULT_ROUTE_TARGET_SHARD)
                .then_some(transaction)
        })
        .ok_or_else(|| eyre!("failed to build a default-route transaction for elastic lane 3"))
}

fn wait_for_autoscale_autonomous_merge(
    network: &sandbox::SerializedNetwork,
    heartbeat_clients: &[Client],
    marker: &LaneIncarnationMarkerV3,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    cycle: usize,
) -> Result<AutoscaleAutonomousEvidence> {
    let started = Instant::now();
    let mut sequence = 0_u64;
    let mut last_entries = Vec::new();
    let mut last_error = None;
    while started.elapsed() <= LANE_PROGRESS_WAIT_TIMEOUT {
        match network
            .peers()
            .iter()
            .map(|peer| autonomous_merge_entry(peer, entrypoint_hash.clone()))
            .collect::<Result<Vec<_>>>()
        {
            Ok(entries) => {
                if let Some(expected) = entries.first().and_then(Option::as_ref)
                    && entries.iter().all(|entry| entry.as_ref() == Some(expected))
                {
                    let batch = expected
                        .execution_batch
                        .as_ref()
                        .ok_or_else(|| eyre!("autonomous merge entry omitted execution batch"))?;
                    let matching = batch
                        .lanes
                        .iter()
                        .filter(|execution| {
                            execution
                                .entrypoints
                                .iter()
                                .any(|entrypoint| entrypoint.hash() == entrypoint_hash)
                        })
                        .collect::<Vec<_>>();
                    ensure!(
                        matching.len() == 1,
                        "autonomous entrypoint occurs in {} lane executions",
                        matching.len()
                    );
                    let execution = matching[0];
                    let descriptor = &execution.proposal.descriptor;
                    ensure!(
                        descriptor.lane_id == LaneId::new(AUTOSCALE_LANE_INDEX)
                            && descriptor.dataspace_id == DataSpaceId::new(NEXUS_ID_U64)
                            && descriptor.lane_incarnation == marker.incarnation,
                        "autonomous transaction was carried by another route or incarnation"
                    );
                    ensure!(
                        execution.origin_proposal.descriptor.lane_id == descriptor.lane_id
                            && execution.origin_proposal.descriptor.dataspace_id
                                == descriptor.dataspace_id
                            && execution.origin_proposal.descriptor.lane_incarnation
                                == descriptor.lane_incarnation
                            && execution
                                .entrypoints
                                .iter()
                                .filter(|entrypoint| entrypoint.hash() == entrypoint_hash)
                                .count()
                                == 1,
                        "autonomous origin/current identity or exact-once merge membership diverged"
                    );
                    return Ok(AutoscaleAutonomousEvidence {
                        entrypoint_hash,
                        merge_entry: expected.clone(),
                        lane_block_height: descriptor.lane_block_height,
                        descriptor_hash: descriptor.descriptor_hash,
                    });
                }
                last_entries = entries;
                last_error = None;
            }
            Err(err) => last_error = Some(err.to_string()),
        }
        if sequence % 5 == 0 {
            let client = &heartbeat_clients
                [usize::try_from(sequence).unwrap_or(0) % heartbeat_clients.len()];
            if let Err(err) = client.submit(
                Log::new(
                    Level::INFO,
                    format!("g12p-autoscale-autonomous-{cycle}-heartbeat-{sequence}"),
                ),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            ) {
                last_error = Some(err.to_string());
            }
        }
        sequence = sequence.saturating_add(1);
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "autoscale cycle {cycle}: timed out waiting for identical durable autonomous merge entry on all peers; entries={last_entries:?}; last_error={last_error:?}"
    ))
}

fn wait_for_active_autoscale_diagnostics_convergence(
    network: &sandbox::SerializedNetwork,
    marker: &LaneIncarnationMarkerV3,
    expected: &AutoscaleAutonomousEvidence,
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_rows = Vec::new();
    while started.elapsed() <= LANE_PROGRESS_WAIT_TIMEOUT {
        last_rows.clear();
        let mut identities = BTreeSet::new();
        let mut ready = true;
        for (index, peer) in network.peers().iter().enumerate() {
            match peer.client().get_sumeragi_diagnostics() {
                Ok(diagnostics) => {
                    let rows = diagnostics
                        .committed_lane_blocks
                        .iter()
                        .filter(|row| row.lane_id == LaneId::new(AUTOSCALE_LANE_INDEX))
                        .collect::<Vec<_>>();
                    if rows
                        .iter()
                        .any(|row| row.lane_incarnation != marker.incarnation)
                    {
                        return Err(eyre!(
                            "{context}: peer {index} published stale lane-3 diagnostics beside incarnation {}",
                            marker.incarnation
                        ));
                    }
                    let matching = rows
                        .iter()
                        .filter(|row| {
                            row.lane_incarnation == marker.incarnation
                                && row.lane_block_height == expected.lane_block_height
                                && row.descriptor_hash == expected.descriptor_hash
                                && row.executable_payload_available
                                && row.validator_count
                                    == u32::try_from(VALIDATORS_PER_LANE).unwrap_or(u32::MAX)
                                && usize::try_from(row.min_quorum).ok()
                                    == Some(commit_quorum_from_len(VALIDATORS_PER_LANE))
                                && row.prepare_qc_signer_count >= row.min_quorum
                                && row.commit_qc_signer_count >= row.min_quorum
                        })
                        .collect::<Vec<_>>();
                    if matching.len() != 1 {
                        ready = false;
                    } else {
                        identities.insert((
                            matching[0].lane_block_height,
                            matching[0].lane_block_view,
                            matching[0].descriptor_hash,
                            matching[0].proposal_hash,
                            matching[0].subject_hash,
                        ));
                    }
                    last_rows.push(format!(
                        "peer#{index}:rows={} exact={}",
                        rows.len(),
                        matching.len()
                    ));
                }
                Err(err) => {
                    ready = false;
                    last_rows.push(format!("peer#{index}:error={err}"));
                }
            }
        }
        if ready && identities.len() == 1 {
            return Ok(());
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "{context}: active lane-3 diagnostics did not converge on the exact autonomous descriptor and incarnation; rows={last_rows:?}"
    ))
}

fn execute_autoscale_autonomous_work(
    network: &sandbox::SerializedNetwork,
    clients: &[Client],
    marker: &LaneIncarnationMarkerV3,
    cycle: usize,
) -> Result<AutoscaleAutonomousEvidence> {
    let submitter = clients
        .first()
        .ok_or_else(|| eyre!("autoscale autonomous work has no submitter"))?;
    let transaction = build_default_route_transaction_for_autoscale_lane(submitter, cycle)?;
    let transaction_hash = transaction.hash();
    let entrypoint_hash = transaction.hash_as_entrypoint();
    ensure!(
        submitter.submit_transaction(&transaction)? == transaction_hash,
        "Torii returned another hash for autoscale autonomous work"
    );
    wait_for_committed_success_across_clients(
        || clients.to_vec(),
        entrypoint_hash.clone(),
        &format!("autoscale cycle {cycle}: autonomous transaction committed"),
        LANE_PROGRESS_WAIT_TIMEOUT,
    )?;
    wait_for_entrypoints_committed_once_on_all_peers(
        network,
        std::slice::from_ref(&entrypoint_hash),
        &format!("autoscale cycle {cycle}: autonomous exact-once canonical history"),
    )?;
    let evidence =
        wait_for_autoscale_autonomous_merge(network, clients, marker, entrypoint_hash, cycle)?;
    wait_for_active_autoscale_diagnostics_convergence(
        network,
        marker,
        &evidence,
        &format!("autoscale cycle {cycle}: active diagnostics convergence"),
    )?;
    Ok(evidence)
}

fn validate_autoscale_drain_certificate(
    chain_id: &ChainId,
    certificate: &LaneDrainCertificateV1,
) -> Result<()> {
    let body = &certificate.body;
    let intent = &body.intent;
    let initial_frontier = &intent.initial_frontier;
    let final_frontier = &body.final_frontier;
    ensure!(
        body.version == 1
            && intent.version == 1
            && initial_frontier.version == 1
            && final_frontier.version == 1,
        "lane-3 drain certificate contains an unsupported layout version"
    );
    ensure!(
        intent.chain_id_digest == merge_chain_id_digest(chain_id),
        "lane-3 drain intent is bound to another chain"
    );
    ensure!(
        intent.validator_set_hash_version == VALIDATOR_SET_HASH_VERSION_V1,
        "lane-3 drain intent uses unsupported validator-set hashing"
    );
    ensure!(
        !intent.validator_set.is_empty()
            && intent.validator_set.len() <= 128
            && intent
                .validator_set
                .windows(2)
                .all(|pair| pair[0] < pair[1]),
        "lane-3 drain committee is empty, oversized, or non-canonical"
    );
    ensure!(
        usize::try_from(intent.validator_count).ok() == Some(intent.validator_set.len())
            && intent.validator_set_hash == HashOf::new(&intent.validator_set)
            && usize::try_from(intent.min_quorum).ok()
                == Some(commit_quorum_from_len(intent.validator_set.len()))
            && certificate.validator_set == intent.validator_set,
        "lane-3 drain certificate substituted or mis-described its committee"
    );
    ensure!(
        initial_frontier.matches_route(
            intent.lane_id,
            intent.dataspace_id,
            intent.lane_incarnation,
        ) && final_frontier.matches_route(
            intent.lane_id,
            intent.dataspace_id,
            intent.lane_incarnation,
        ) && (initial_frontier.lane_block_height == 0)
            == initial_frontier.lane_block_descriptor_hash.is_none()
            && (final_frontier.lane_block_height == 0)
                == final_frontier.lane_block_descriptor_hash.is_none()
            && final_frontier.lane_block_height >= initial_frontier.lane_block_height,
        "lane-3 drain certificate carries an invalid or regressing frontier"
    );
    let empty_unresolved_root =
        iroha::data_model::merge::lane_drain_empty_unresolved_evidence_root();
    ensure!(
        intent.close_global_height > 0
            && intent
                .lane_incarnation
                .as_ref()
                .iter()
                .any(|byte| *byte != 0)
            && initial_frontier.native_application.is_none()
            && final_frontier.native_application.is_none()
            && initial_frontier.unresolved_evidence_root == empty_unresolved_root
            && final_frontier.unresolved_evidence_root == empty_unresolved_root
            && (final_frontier.lane_block_height != initial_frontier.lane_block_height
                || final_frontier == initial_frontier),
        "lane-3 drain certificate carries non-empty, Native, or conflicting frontier evidence"
    );

    let expected_bitmap_len = certificate.validator_set.len().div_ceil(8);
    ensure!(
        certificate.signers_bitmap.len() == expected_bitmap_len,
        "lane-3 drain signer bitmap length does not match its committee"
    );
    if certificate.validator_set.len() % 8 != 0 {
        let used_bits = certificate.validator_set.len() % 8;
        let padding_mask = !((1_u8 << used_bits) - 1);
        ensure!(
            certificate.signers_bitmap[expected_bitmap_len - 1] & padding_mask == 0,
            "lane-3 drain signer bitmap has non-zero padding"
        );
    }
    let mut signer_indices = Vec::new();
    for (byte_index, byte) in certificate.signers_bitmap.iter().copied().enumerate() {
        for bit in 0_u8..8 {
            if byte & (1_u8 << bit) == 0 {
                continue;
            }
            let signer_index = byte_index * 8 + usize::from(bit);
            ensure!(
                signer_index < certificate.validator_set.len(),
                "lane-3 drain signer bitmap selects an out-of-range validator"
            );
            signer_indices.push(signer_index);
        }
    }
    ensure!(
        signer_indices.len() >= commit_quorum_from_len(certificate.validator_set.len())
            && certificate.signer_proofs.len() == signer_indices.len(),
        "lane-3 drain certificate is below quorum or has unaligned signer proofs"
    );
    let mut public_keys = Vec::with_capacity(signer_indices.len());
    let mut proof_refs = Vec::with_capacity(signer_indices.len());
    for (signer_index, proof) in signer_indices.iter().zip(&certificate.signer_proofs) {
        ensure!(
            proof.signer == u32::try_from(*signer_index)?,
            "lane-3 drain signer proof names another committee index"
        );
        let public_key = certificate.validator_set[*signer_index].public_key();
        iroha_crypto::bls_normal_pop_verify(public_key, &proof.proof_of_possession).map_err(
            |err| {
                eyre!("lane-3 drain signer {signer_index} has invalid proof-of-possession: {err:?}")
            },
        )?;
        public_keys.push(public_key);
        proof_refs.push(proof.proof_of_possession.as_slice());
    }
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &body.signature_preimage(),
        &certificate.aggregate_signature,
        &public_keys,
        &proof_refs,
    )
    .map_err(|err| eyre!("lane-3 drain aggregate signature is invalid: {err:?}"))?;
    Ok(())
}

async fn fetch_autoscale_bridge_finality_proof(
    peer: &NetworkPeer,
    height: u64,
) -> Result<BridgeFinalityProof> {
    let client = peer.client();
    let url = client
        .torii_url
        .join(&format!("v1/bridge/finality/{height}"))
        .wrap_err("construct autoscale carrier-finality URL")?;
    let request = reqwest::Client::builder()
        .timeout(client.torii_request_timeout)
        .build()
        .wrap_err("build autoscale carrier-finality HTTP client")?
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json");
    let response = add_client_headers(&client, request)
        .send()
        .await
        .wrap_err_with(|| {
            format!(
                "fetch height-{height} autoscale carrier finality from {}",
                peer.mnemonic()
            )
        })?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .wrap_err_with(|| format!("read carrier finality from {}", peer.mnemonic()))?;
    ensure!(
        status.is_success(),
        "{} returned HTTP {status} for height-{height} carrier finality: {}",
        peer.mnemonic(),
        String::from_utf8_lossy(&bytes),
    );
    norito::json::from_slice::<BridgeFinalityProof>(&bytes).wrap_err_with(|| {
        format!(
            "{} returned malformed height-{height} carrier-finality JSON",
            peer.mnemonic()
        )
    })
}

fn exact_autoscale_carrier_height_context(
    runtime: &Runtime,
    network: &sandbox::SerializedNetwork,
    carrier_height: u64,
) -> Result<HeightContext> {
    let proofs = runtime.block_on(async {
        try_join_all(
            network
                .peers()
                .iter()
                .map(|peer| fetch_autoscale_bridge_finality_proof(peer, carrier_height)),
        )
        .await
    })?;
    let first = proofs
        .first()
        .ok_or_else(|| eyre!("autoscale carrier-height proof set is empty"))?;
    verify_bridge_finality_proof(first, &network.chain_id())
        .wrap_err("first autoscale carrier finality proof is invalid")?;
    ensure!(
        first.finality_artifact.height == carrier_height,
        "first autoscale finality proof names height {}, expected carrier height {carrier_height}",
        first.finality_artifact.height,
    );
    let expected_context = &first.finality_artifact.height_context;
    let expected_context_id = expected_context.id();
    let expected_block_hash = first.block_header.hash();
    for (peer_index, proof) in proofs.iter().enumerate().skip(1) {
        verify_bridge_finality_proof(proof, &network.chain_id()).wrap_err_with(|| {
            format!("peer {peer_index} autoscale carrier finality proof is invalid")
        })?;
        let context = &proof.finality_artifact.height_context;
        ensure!(
            proof.finality_artifact.height == carrier_height
                && proof.block_header.hash() == expected_block_hash
                && context.id() == expected_context_id
                && context.chain_id == expected_context.chain_id
                && context.height == expected_context.height
                && context.epoch == expected_context.epoch
                && context.roster == expected_context.roster
                && context.quorum == expected_context.quorum,
            "peer {peer_index} disagrees on the exact canonical carrier block or frozen powered height context"
        );
    }
    Ok(expected_context.clone())
}

fn validate_autoscale_merge_qc_height_context_binding(
    chain_id: &ChainId,
    context: &HeightContext,
    carrier_height: u64,
    epoch_id: u64,
    validator_set: &[PeerId],
    signer_indices: &[usize],
) -> Result<()> {
    ensure!(
        context.chain_id == *chain_id
            && context.height == carrier_height
            && context.epoch == epoch_id,
        "merge QC carrier height/epoch/chain differs from its historical Sumeragi v2 context"
    );
    ensure!(
        validator_set.len() == context.roster.len()
            && validator_set
                .iter()
                .zip(&context.roster)
                .all(|(actual, frozen)| actual == &frozen.validator),
        "merge QC validator set differs from the exact frozen carrier-height roster"
    );
    let weighted_signers = signer_indices
        .iter()
        .copied()
        .map(u32::try_from)
        .collect::<std::result::Result<Vec<_>, _>>()
        .wrap_err("merge QC signer index exceeds the historical context range")?;
    context
        .validate_signers(&weighted_signers)
        .map_err(|err| eyre!("merge QC fails the historical count-and-power quorum: {err}"))
}

fn validate_autoscale_merge_qc(
    chain_id: &ChainId,
    context: &HeightContext,
    entry: &MergeLedgerEntry,
) -> Result<()> {
    let qc = &entry.merge_qc;
    ensure!(qc.epoch_id == entry.epoch_id, "merge QC epoch mismatch");
    ensure!(
        qc.chain_id_digest == merge_chain_id_digest(chain_id),
        "merge QC is bound to another chain"
    );
    ensure!(
        qc.validator_set_hash_version == VALIDATOR_SET_HASH_VERSION_V1
            && qc.validator_set_hash == HashOf::new(&qc.validator_set)
            && !qc.validator_set.is_empty()
            && qc.validator_set.iter().collect::<BTreeSet<_>>().len() == qc.validator_set.len(),
        "merge QC has an invalid validator-set commitment"
    );
    let candidate = MergeLedgerCandidate::from(entry);
    ensure!(
        qc.message_digest
            == merge_qc_message_digest(
                chain_id,
                &candidate,
                qc.validator_set_hash_version,
                qc.validator_set_hash,
            ),
        "merge QC message digest does not bind the exact drain carrier"
    );
    let expected_bitmap_len = qc.validator_set.len().div_ceil(8);
    ensure!(
        qc.signers_bitmap.len() == expected_bitmap_len,
        "merge QC signer bitmap length does not match its roster"
    );
    if qc.validator_set.len() % 8 != 0 {
        let used_bits = qc.validator_set.len() % 8;
        let padding_mask = !((1_u8 << used_bits) - 1);
        ensure!(
            qc.signers_bitmap[expected_bitmap_len - 1] & padding_mask == 0,
            "merge QC signer bitmap has non-zero padding"
        );
    }
    let mut signer_indices = Vec::new();
    for (byte_index, byte) in qc.signers_bitmap.iter().copied().enumerate() {
        for bit in 0_u8..8 {
            if byte & (1_u8 << bit) == 0 {
                continue;
            }
            let signer_index = byte_index * 8 + usize::from(bit);
            ensure!(
                signer_index < qc.validator_set.len(),
                "merge QC signer bitmap selects an out-of-range validator"
            );
            signer_indices.push(signer_index);
        }
    }
    ensure!(
        signer_indices.len() >= commit_quorum_from_len(qc.validator_set.len())
            && qc.signer_proofs.len() == signer_indices.len(),
        "merge QC is below quorum or has unaligned signer proofs"
    );
    validate_autoscale_merge_qc_height_context_binding(
        chain_id,
        context,
        qc.carrier_height,
        qc.epoch_id,
        &qc.validator_set,
        &signer_indices,
    )?;
    let mut public_keys = Vec::with_capacity(signer_indices.len());
    let mut proof_refs = Vec::with_capacity(signer_indices.len());
    for (signer_index, proof) in signer_indices.iter().zip(&qc.signer_proofs) {
        ensure!(
            proof.signer == u32::try_from(*signer_index)?,
            "merge QC signer proof names another roster index"
        );
        let public_key = qc.validator_set[*signer_index].public_key();
        iroha_crypto::bls_normal_pop_verify(public_key, &proof.proof_of_possession).map_err(
            |err| eyre!("merge QC signer {signer_index} has invalid proof-of-possession: {err:?}"),
        )?;
        public_keys.push(public_key);
        proof_refs.push(proof.proof_of_possession.as_slice());
    }
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        qc.message_digest.as_ref(),
        &qc.aggregate_signature,
        &public_keys,
        &proof_refs,
    )
    .map_err(|err| eyre!("merge QC aggregate signature is invalid: {err:?}"))?;
    Ok(())
}

fn validate_autoscale_retirement_evidence(
    runtime: &Runtime,
    network: &sandbox::SerializedNetwork,
    marker: &LaneIncarnationMarkerV3,
    autonomous: &AutoscaleAutonomousEvidence,
    entry: &MergeLedgerEntry,
    logs: &[AutoscaleLifecycleLog],
) -> Result<u64> {
    let [certificate] = entry.lane_drain_certificates.as_slice() else {
        return Err(eyre!(
            "lane-3 retirement merge entry must carry exactly one drain certificate"
        ));
    };
    let intent = &certificate.body.intent;
    let final_frontier = &certificate.body.final_frontier;
    validate_autoscale_drain_certificate(&network.chain_id(), certificate)?;
    let carrier_context =
        exact_autoscale_carrier_height_context(runtime, network, entry.merge_qc.carrier_height)?;
    validate_autoscale_merge_qc(&network.chain_id(), &carrier_context, entry)?;
    ensure!(
        entry.execution_batch.is_none() && entry.lane_snapshots.is_empty(),
        "drain carrier mixed the certificate with autonomous execution or lane snapshots"
    );
    ensure!(
        intent.lane_id == LaneId::new(AUTOSCALE_LANE_INDEX)
            && intent.dataspace_id == DataSpaceId::new(NEXUS_ID_U64)
            && intent.lane_incarnation == marker.incarnation
            && intent.initial_frontier.matches_route(
                intent.lane_id,
                intent.dataspace_id,
                intent.lane_incarnation,
            ),
        "drain intent did not bind the exact retired lane-3 incarnation"
    );
    ensure!(
        final_frontier.matches_route(intent.lane_id, intent.dataspace_id, intent.lane_incarnation,)
            && final_frontier.lane_block_height >= autonomous.lane_block_height
            && final_frontier.unresolved_evidence_root
                == iroha::data_model::merge::lane_drain_empty_unresolved_evidence_root(),
        "drain certificate did not close an evidence-clean frontier containing useful autonomous work"
    );
    ensure!(
        final_frontier.lane_block_height > 0
            && final_frontier.lane_block_descriptor_hash.is_some()
            && intent.validator_set == certificate.validator_set
            && certificate.validator_set.len() == VALIDATORS_PER_LANE
            && usize::try_from(intent.validator_count).ok() == Some(VALIDATORS_PER_LANE)
            && usize::try_from(intent.min_quorum).ok()
                == Some(commit_quorum_from_len(VALIDATORS_PER_LANE)),
        "drain certificate committee/frontier shape is not the exact 4-validator lane proof"
    );
    let signer_count = certificate
        .signers_bitmap
        .iter()
        .map(|byte| byte.count_ones() as usize)
        .sum::<usize>();
    ensure!(
        signer_count >= commit_quorum_from_len(VALIDATORS_PER_LANE)
            && certificate.signer_proofs.len() == signer_count,
        "drain certificate does not carry an aligned lane quorum"
    );
    ensure!(
        entry.merge_qc.carrier_height > intent.close_global_height,
        "drain certificate carrier was not strictly later than its close boundary"
    );
    let expected_intent = AutoscaleDrainIntentLog {
        height: intent.close_global_height,
        close_global_height: intent.close_global_height,
        initial_merged_lane_height: intent.initial_frontier.lane_block_height,
    };
    let expected_commitment = AutoscaleDrainCommitmentLog {
        height: entry.merge_qc.carrier_height,
        carrier_height: entry.merge_qc.carrier_height,
        final_lane_block_height: final_frontier.lane_block_height,
    };
    ensure!(
        logs.len() == network.peers().len()
            && logs.iter().all(|log| {
                log.drain_intents.contains(&expected_intent)
                    && log.drain_commitments.contains(&expected_commitment)
            }),
        "bounded peer logs do not all bind the exact drain intent and carried frontier"
    );
    let retirement_heights = logs
        .iter()
        .map(|log| {
            log.scale_in_heights
                .iter()
                .copied()
                .find(|height| *height > entry.merge_qc.carrier_height)
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| eyre!("one or more peers omitted the post-carrier scale-in transition"))?;
    ensure!(
        retirement_heights.windows(2).all(|pair| pair[0] == pair[1]),
        "peers disagree on lane-3 retirement height: {retirement_heights:?}"
    );
    let retirement_height = retirement_heights[0];
    ensure!(
        intent.close_global_height < entry.merge_qc.carrier_height
            && entry.merge_qc.carrier_height < retirement_height,
        "lane-3 lifecycle order is not close < carrier < removal"
    );
    Ok(retirement_height)
}

fn wait_for_autoscale_retirement(
    runtime: &Runtime,
    network: &sandbox::SerializedNetwork,
    heartbeat_clients: &[Client],
    marker: &LaneIncarnationMarkerV3,
    autonomous: &AutoscaleAutonomousEvidence,
    cycle: usize,
) -> Result<(MergeLedgerEntry, Vec<PathBuf>, u64)> {
    let started = Instant::now();
    let mut heartbeat_sequence = 0_u64;
    let mut next_heartbeat = Instant::now();
    let mut last_entries = Vec::new();
    let mut last_archives = Vec::new();
    let mut last_logs = Vec::new();
    let mut last_status = Vec::new();
    let mut last_error = None;
    while started.elapsed() <= AUTOSCALE_SCALE_IN_WAIT_TIMEOUT {
        let storage_retired = all_peers_have_lane_storage_profile(network, false)?;
        let entries = network
            .peers()
            .iter()
            .map(|peer| drain_merge_entry(peer, marker.incarnation))
            .collect::<Result<Vec<_>>>();
        let archives = network
            .peers()
            .iter()
            .map(|peer| archived_autoscale_marker_for_incarnation(peer, marker.incarnation))
            .collect::<Result<Vec<_>>>();
        let logs = network
            .peers()
            .iter()
            .map(peer_autoscale_lifecycle_log)
            .collect::<Result<Vec<_>>>();
        last_status.clear();
        let mut diagnostics_retired = true;
        for (index, peer) in network.peers().iter().enumerate() {
            match (
                peer.client().get_sumeragi_status(),
                peer.client().get_sumeragi_diagnostics(),
            ) {
                (Ok(status), Ok(diagnostics)) => {
                    let lane_blocks = diagnostics
                        .committed_lane_blocks
                        .iter()
                        .filter(|row| row.lane_id == LaneId::new(AUTOSCALE_LANE_INDEX))
                        .count();
                    let lane_governance = diagnostics
                        .lane_governance
                        .iter()
                        .filter(|row| row.lane_id == LaneId::new(AUTOSCALE_LANE_INDEX))
                        .count();
                    if status.validate().is_err()
                        || status.restart_required
                        || lane_blocks != 0
                        || lane_governance != 0
                    {
                        diagnostics_retired = false;
                    }
                    last_status.push(format!(
                        "peer#{index}:height={} blocks={lane_blocks} governance={lane_governance}",
                        status.last_committed_height
                    ));
                }
                (status, diagnostics) => {
                    diagnostics_retired = false;
                    last_status.push(format!(
                        "peer#{index}:status={:?} diagnostics={:?}",
                        status.as_ref().err().map(ToString::to_string),
                        diagnostics.as_ref().err().map(ToString::to_string)
                    ));
                }
            }
        }
        match (entries, archives, logs) {
            (Ok(entries), Ok(archives), Ok(logs)) => {
                if storage_retired
                    && diagnostics_retired
                    && entries
                        .first()
                        .and_then(Option::as_ref)
                        .is_some_and(|expected| {
                            entries.iter().all(|entry| entry.as_ref() == Some(expected))
                        })
                    && archives.iter().all(Option::is_some)
                {
                    let expected = entries[0].as_ref().expect("checked Some");
                    let retirement_height = validate_autoscale_retirement_evidence(
                        runtime, network, marker, autonomous, expected, &logs,
                    )?;
                    let archive_paths = archives
                        .into_iter()
                        .map(|archive| {
                            let (path, archived_marker) = archive.expect("checked Some");
                            ensure!(
                                archived_marker.incarnation == marker.incarnation
                                    && archived_marker.activation_height
                                        == marker.activation_height,
                                "retirement archive changed lane-3 incarnation identity"
                            );
                            Ok(path)
                        })
                        .collect::<Result<Vec<_>>>()?;
                    eprintln!(
                        "[g12p-autoscale] cycle={cycle} retired incarnation={} close={} carrier={} removal={retirement_height}",
                        marker.incarnation,
                        expected.lane_drain_certificates[0]
                            .body
                            .intent
                            .close_global_height,
                        expected.merge_qc.carrier_height,
                    );
                    return Ok((expected.clone(), archive_paths, retirement_height));
                }
                last_entries = entries;
                last_archives = archives;
                last_logs = logs;
                last_error = None;
            }
            (entries, archives, logs) => {
                if let Err(err) = entries {
                    last_error = Some(err.to_string());
                } else if let Err(err) = archives {
                    last_error = Some(err.to_string());
                } else if let Err(err) = logs {
                    last_error = Some(err.to_string());
                }
            }
        }
        let now = Instant::now();
        if now >= next_heartbeat {
            let client = &heartbeat_clients
                [usize::try_from(heartbeat_sequence).unwrap_or(0) % heartbeat_clients.len()];
            if let Err(err) = client.submit(
                Log::new(
                    Level::INFO,
                    format!("g12p-autoscale-cycle-{cycle}-drain-{heartbeat_sequence}"),
                ),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            ) {
                last_error = Some(err.to_string());
            }
            heartbeat_sequence = heartbeat_sequence.saturating_add(1);
            next_heartbeat = now + AUTOSCALE_HEARTBEAT_INTERVAL;
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "autoscale cycle {cycle}: timed out waiting for evidence-aware drain, carried certificate, archive, and removal on all peers; entries={last_entries:?}; archives={last_archives:?}; logs={last_logs:?}; status={last_status:?}; last_error={last_error:?}"
    ))
}

#[derive(Debug)]
struct KuraCopyPlan {
    directories: Vec<PathBuf>,
    files: Vec<KuraCopyFile>,
}

#[derive(Debug)]
struct KuraCopyFile {
    source: PathBuf,
    relative: PathBuf,
    len: u64,
}

fn plan_kura_tree_copy(source: &Path, max_entries: usize, max_bytes: u64) -> Result<KuraCopyPlan> {
    let source_metadata = fs::symlink_metadata(source)?;
    ensure!(
        source_metadata.is_dir() && !source_metadata.file_type().is_symlink(),
        "bounded Kura copy source is not a regular directory: {}",
        source.display()
    );
    let mut pending = vec![source.to_path_buf()];
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let mut entries_seen = 0_usize;
    let mut bytes_planned = 0_u64;
    while let Some(source_dir) = pending.pop() {
        let mut entries = fs::read_dir(&source_dir)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort();
        for source_path in entries {
            if source_path == source.join(".kura.lock") {
                continue;
            }
            entries_seen = entries_seen
                .checked_add(1)
                .ok_or_else(|| eyre!("bounded Kura copy entry count overflowed"))?;
            ensure!(
                entries_seen <= max_entries,
                "bounded Kura copy exceeded {max_entries} entries"
            );
            let metadata = fs::symlink_metadata(&source_path)?;
            ensure!(
                !metadata.file_type().is_symlink(),
                "bounded Kura copy encountered symlink {}",
                source_path.display()
            );
            let relative = source_path
                .strip_prefix(source)
                .wrap_err("Kura copy entry escaped its source root")?
                .to_path_buf();
            if metadata.is_dir() {
                directories.push(relative);
                pending.push(source_path);
            } else {
                ensure!(
                    metadata.is_file(),
                    "bounded Kura copy encountered non-regular entry {}",
                    source_path.display()
                );
                bytes_planned = bytes_planned
                    .checked_add(metadata.len())
                    .ok_or_else(|| eyre!("bounded Kura copy byte count overflowed"))?;
                ensure!(
                    bytes_planned <= max_bytes,
                    "bounded Kura copy exceeded {max_bytes} bytes"
                );
                files.push(KuraCopyFile {
                    source: source_path,
                    relative,
                    len: metadata.len(),
                });
            }
        }
    }
    directories.sort_by(|left, right| {
        left.components()
            .count()
            .cmp(&right.components().count())
            .then_with(|| left.cmp(right))
    });
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(KuraCopyPlan { directories, files })
}

fn copy_kura_tree_with_limits(
    source: &Path,
    destination: &Path,
    max_entries: usize,
    max_bytes: u64,
) -> Result<()> {
    let plan = plan_kura_tree_copy(source, max_entries, max_bytes)?;
    match fs::symlink_metadata(destination) {
        Ok(_) => {
            return Err(eyre!(
                "bounded Kura destination already exists: {}",
                destination.display()
            ));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err.into()),
    }
    fs::create_dir(destination)?;
    for relative in &plan.directories {
        fs::create_dir(destination.join(relative))?;
    }
    for planned in &plan.files {
        let source_metadata = fs::symlink_metadata(&planned.source)?;
        ensure!(
            source_metadata.is_file()
                && !source_metadata.file_type().is_symlink()
                && source_metadata.len() == planned.len,
            "bounded Kura source changed after preflight: {}",
            planned.source.display()
        );
        let destination_path = destination.join(&planned.relative);
        let copied = fs::copy(&planned.source, &destination_path)?;
        ensure!(
            copied == planned.len,
            "bounded Kura copy wrote {copied} bytes for {}, expected {}",
            planned.source.display(),
            planned.len,
        );
        let destination_metadata = fs::symlink_metadata(&destination_path)?;
        ensure!(
            destination_metadata.is_file()
                && !destination_metadata.file_type().is_symlink()
                && destination_metadata.len() == planned.len,
            "bounded Kura copy produced an invalid destination file: {}",
            destination_path.display()
        );
    }
    Ok(())
}

fn copy_kura_tree_bounded(source: &Path, destination: &Path) -> Result<()> {
    copy_kura_tree_with_limits(
        source,
        destination,
        AUTOSCALE_COPY_MAX_ENTRIES,
        AUTOSCALE_COPY_MAX_BYTES,
    )
}

fn offline_kura_config(store_dir: PathBuf) -> KuraConfig {
    KuraConfig {
        init_mode: InitMode::Strict,
        store_dir: WithOrigin::inline(store_dir),
        max_disk_usage_bytes: defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: NonZeroUsize::new(2).expect("two is non-zero"),
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity: defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: FsyncMode::Batched,
        fsync_interval: defaults::kura::FSYNC_INTERVAL,
        block_sync_roster_retention: defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
        roster_sidecar_retention: defaults::kura::ROSTER_SIDECAR_RETENTION,
        eviction_required_replicas: defaults::kura::EVICTION_REQUIRED_REPLICAS,
    }
}

fn assert_stale_archived_marker_rejected(
    peer: &NetworkPeer,
    stale_marker_path: &Path,
    expected_active: &LaneIncarnationMarkerV3,
) -> Result<()> {
    let temp = tempfile::tempdir()?;
    let cloned_store = temp.path().join("g12p-stale-incarnation");
    let live_marker_path = active_autoscale_marker_path(peer);
    let live_marker_bytes = fs::read(&live_marker_path)?;
    copy_kura_tree_bounded(&peer.kura_store_dir(), &cloned_store)?;
    let cloned_active_marker = cloned_store
        .join("blocks")
        .join(autoscale_storage_segment())
        .join(LANE_INCARNATION_MARKER_FILE);
    ensure!(
        read_lane_incarnation_marker(&cloned_active_marker)? == *expected_active,
        "cloned Kura did not retain the recreated active incarnation"
    );
    // Production startup authenticates the original static three-lane catalog;
    // the elastic lane is reconstructed from the durable geometry journal.
    let catalog = multilane_lane_catalog();
    let lane_config = ActualLaneConfig::from_catalog(&catalog);
    let config = offline_kura_config(cloned_store);
    let (control, _) = Kura::new_with_configured_lane_catalog(&config, &lane_config, &catalog)
        .map_err(|err| {
            eyre!("control cloned Kura rejected the unmodified recreated store: {err}")
        })?;
    drop(control);
    fs::copy(stale_marker_path, &cloned_active_marker)?;
    let stale = read_lane_incarnation_marker(&cloned_active_marker)?;
    ensure!(
        stale.incarnation != expected_active.incarnation,
        "stale marker fixture accidentally names the recreated incarnation"
    );
    ensure!(
        Kura::new_with_configured_lane_catalog(&config, &lane_config, &catalog).is_err(),
        "Kura admitted archived incarnation A as recreated lane B's active marker"
    );
    ensure!(
        fs::read(&live_marker_path)? == live_marker_bytes,
        "stale-incarnation admission fixture modified the live peer store"
    );
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn prove_g12p_autoscale_lifecycle(
    network: &sandbox::SerializedNetwork,
    runtime: &Runtime,
    load_clients: &[Client],
    restart_layers: &[ConfigLayer],
    seed: &CorridorSeed,
) -> Result<()> {
    ensure!(
        network.peers().len() == TOTAL_PEERS,
        "G-12P lifecycle requires all {TOTAL_PEERS} peers"
    );
    ensure!(
        !load_clients.is_empty()
            && expected_active_lane_storage_ids(false).len() == AUTOSCALE_BASE_LANE_COUNT
            && expected_active_lane_storage_ids(true).len() == AUTOSCALE_EXPANDED_LANE_COUNT,
        "G-12P lifecycle requires non-empty load clients and an exact 3->4 lane profile"
    );
    wait_for_autoscale_baseline(network, "G-12P autoscale baseline")?;

    let marker_a = wait_for_autoscale_expansion(network, load_clients, 1)?;
    let autonomous_a = execute_autoscale_autonomous_work(network, load_clients, &marker_a, 1)?;
    let (drain_a, archive_a_paths, retirement_a_height) =
        wait_for_autoscale_retirement(runtime, network, load_clients, &marker_a, &autonomous_a, 1)?;

    let marker_b = wait_for_autoscale_expansion(network, load_clients, 2)?;
    ensure!(
        marker_b.lane_id == marker_a.lane_id
            && marker_b.incarnation != marker_a.incarnation
            && marker_b.activation_height > marker_a.activation_height,
        "same lane ID was not recreated with a later fresh incarnation: A={marker_a:?}, B={marker_b:?}"
    );
    for (index, peer) in network.peers().iter().enumerate() {
        ensure!(
            drain_merge_entry(peer, marker_a.incarnation)?.as_ref() == Some(&drain_a),
            "peer {index} lost or changed incarnation A's carried drain certificate during recreation"
        );
    }
    let autonomous_b = execute_autoscale_autonomous_work(network, load_clients, &marker_b, 2)?;
    ensure!(
        autonomous_b.entrypoint_hash != autonomous_a.entrypoint_hash
            && autonomous_b.merge_entry != autonomous_a.merge_entry,
        "recreated lane reused incarnation A's autonomous source identity"
    );
    submit_autoscale_load(load_clients, 2, 64)?;

    let stale_probe_peer_index = seed.ordinal % TOTAL_PEERS;
    let stale_probe_peer = network.peers()[stale_probe_peer_index].clone();
    let stale_context =
        format!("G-12P stale-incarnation admission probe on peer {stale_probe_peer_index}");
    shutdown_rotated_validators(
        runtime,
        std::slice::from_ref(&stale_probe_peer),
        &stale_context,
    )?;
    let stale_probe_result = assert_stale_archived_marker_rejected(
        &stale_probe_peer,
        &archive_a_paths[stale_probe_peer_index],
        &marker_b,
    );
    let restart_result = restart_rotated_validators(
        runtime,
        std::slice::from_ref(&stale_probe_peer),
        restart_layers,
        &stale_context,
    );
    match (stale_probe_result, restart_result) {
        (Ok(()), Ok(())) => {}
        (Err(probe_err), Ok(())) => return Err(probe_err),
        (Ok(()), Err(restart_err)) => return Err(restart_err),
        (Err(probe_err), Err(restart_err)) => {
            return Err(eyre!(
                "{stale_context}: stale-artifact probe failed: {probe_err}; restart also failed: {restart_err}"
            ));
        }
    }

    wait_for_recreated_autoscale_lane_ready(
        network,
        load_clients,
        &marker_b,
        "G-12P recreated lane readiness after stale-artifact rejection",
    )?;
    let (drain_b, _, retirement_b_height) =
        wait_for_autoscale_retirement(runtime, network, load_clients, &marker_b, &autonomous_b, 2)?;
    ensure!(
        drain_b != drain_a
            && drain_b.lane_drain_certificates[0]
                .body
                .intent
                .lane_incarnation
                == marker_b.incarnation
            && drain_b.lane_drain_certificates[0]
                .body
                .intent
                .lane_incarnation
                != marker_a.incarnation,
        "recreated lane drain accepted incarnation A evidence"
    );
    ensure!(
        retirement_a_height < marker_b.activation_height
            && marker_b.activation_height < retirement_b_height,
        "A/B lifecycle heights are not strictly ordered"
    );
    for (index, peer) in network.peers().iter().enumerate() {
        let archived = archived_autoscale_markers(peer)?
            .into_iter()
            .map(|(_, marker)| marker.incarnation)
            .collect::<BTreeSet<_>>();
        ensure!(
            archived.contains(&marker_a.incarnation) && archived.contains(&marker_b.incarnation),
            "peer {index} did not retain both same-ID incarnation archives: {archived:?}"
        );
        ensure!(
            autonomous_merge_entry(peer, autonomous_a.entrypoint_hash.clone())?.as_ref()
                == Some(&autonomous_a.merge_entry)
                && autonomous_merge_entry(peer, autonomous_b.entrypoint_hash.clone())?.as_ref()
                    == Some(&autonomous_b.merge_entry),
            "peer {index} lost or changed autonomous A/B merge evidence"
        );
    }
    wait_for_entrypoints_committed_once_on_all_peers(
        network,
        &[autonomous_a.entrypoint_hash, autonomous_b.entrypoint_hash],
        "G-12P autoscale A/B final exact-once convergence",
    )?;
    wait_for_autoscale_baseline(network, "G-12P autoscale final 3-lane convergence")?;
    eprintln!(
        "[g12p-autoscale] seed={} baseline=3 expansion=A work=A drain=A archive=A recreation=B stale_A=rejected work=B drain=B final=3 passed",
        seed.value
    );
    Ok(())
}

fn expected_post_swap_balances(completed_work_units: usize) -> Result<[Quantity; 4]> {
    let completed = u64::try_from(completed_work_units)
        .map_err(|_| eyre!("autonomous work-unit count does not fit u64"))?;
    ensure!(
        completed <= DS1_WORKLOAD_SEED_AMOUNT.saturating_sub(30)
            && completed <= DS2_WORKLOAD_SEED_AMOUNT.saturating_sub(45),
        "autonomous fault soak exhausted seeded workload balances after {completed} work units"
    );
    Ok([
        Quantity::from(
            DS1_WORKLOAD_SEED_AMOUNT
                .saturating_sub(30)
                .saturating_sub(completed),
        ),
        Quantity::from(30_u64.saturating_add(completed)),
        Quantity::from(45_u64.saturating_add(completed)),
        Quantity::from(
            DS2_WORKLOAD_SEED_AMOUNT
                .saturating_sub(45)
                .saturating_sub(completed),
        ),
    ])
}

#[derive(Debug)]
struct CommittedSuccessOrHeightFallback {
    status: SumeragiObservation,
    committed_outcome_confirmed: bool,
}

fn wait_for_committed_success_or_height_fallback_across_clients(
    mut observer_factory: impl FnMut() -> Vec<Client>,
    tick_submitters: &[Client],
    entry_hash: HashOf<TransactionEntrypoint>,
    committed_context: &str,
    fallback_context: &str,
    pre_barrier_height: u64,
    committed_timeout: Duration,
    post_barrier_outcome_timeout: Duration,
) -> Result<CommittedSuccessOrHeightFallback> {
    match wait_for_committed_success_across_clients(
        &mut observer_factory,
        entry_hash.clone(),
        committed_context,
        committed_timeout,
    ) {
        Ok(()) => {
            let status = observer_factory()
                .into_iter()
                .filter_map(|client| sumeragi_observation(&client).ok())
                .max_by_key(|status| status.canonical.last_committed_height)
                .ok_or_else(|| {
                    eyre!("{committed_context}: no observer returned a status snapshot")
                })?;
            Ok(CommittedSuccessOrHeightFallback {
                status,
                committed_outcome_confirmed: true,
            })
        }
        Err(err) => {
            let error_text = err.to_string();
            if !is_inconclusive_committed_outcome_error(&error_text) {
                return Err(err);
            }
            eprintln!("[swap] committed outcome inconclusive; falling back to height barrier");
            let barrier_status = wait_for_height_with_tick_submitters_timeout_across_clients(
                &mut observer_factory,
                Some(tick_submitters),
                pre_barrier_height.saturating_add(1),
                fallback_context,
                STATUS_WAIT_TIMEOUT,
                SETUP_BARRIER_TICK_EVERY_POLLS,
            )?;
            let post_context = format!("{committed_context} (post-barrier)");
            match wait_for_committed_success_across_clients(
                &mut observer_factory,
                entry_hash,
                post_context.as_str(),
                post_barrier_outcome_timeout,
            ) {
                Ok(()) => {
                    let status = observer_factory()
                        .into_iter()
                        .filter_map(|client| sumeragi_observation(&client).ok())
                        .max_by_key(|status| status.canonical.last_committed_height)
                        .ok_or_else(|| {
                            eyre!("{post_context}: no observer returned a status snapshot")
                        })?;
                    Ok(CommittedSuccessOrHeightFallback {
                        status,
                        committed_outcome_confirmed: true,
                    })
                }
                Err(post_err) => {
                    let post_error_text = post_err.to_string();
                    if is_inconclusive_committed_outcome_error(&post_error_text) {
                        Ok(CommittedSuccessOrHeightFallback {
                            status: barrier_status,
                            committed_outcome_confirmed: false,
                        })
                    } else {
                        Err(post_err)
                    }
                }
            }
        }
    }
}

struct PhaseTimings {
    test_name: &'static str,
    started: Instant,
    phases: Vec<(String, Duration)>,
    summary_emitted: bool,
}

impl PhaseTimings {
    fn new(test_name: &'static str) -> Self {
        Self {
            test_name,
            started: Instant::now(),
            phases: Vec::new(),
            summary_emitted: false,
        }
    }

    fn phase<'a>(&'a mut self, label: impl Into<String>) -> PhaseGuard<'a> {
        PhaseGuard {
            timings: self,
            label: label.into(),
            started: Instant::now(),
        }
    }

    fn emit_summary(&mut self) {
        if self.summary_emitted || self.phases.is_empty() {
            return;
        }
        self.summary_emitted = true;
        let total = self.started.elapsed();
        let total_secs = total.as_secs_f64();
        eprintln!("[phase-timer] summary for {}:", self.test_name);
        for (index, (phase, duration)) in self.phases.iter().enumerate() {
            let secs = duration.as_secs_f64();
            let pct = if total_secs > 0.0 {
                100.0 * secs / total_secs
            } else {
                0.0
            };
            eprintln!(
                "[phase-timer] {:02}. {:<50} {:>8.3}s ({:>5.1}%)",
                index + 1,
                phase,
                secs,
                pct
            );
        }
        eprintln!("[phase-timer] total{:>57.3}s", total.as_secs_f64());
    }
}

impl Drop for PhaseTimings {
    fn drop(&mut self) {
        self.emit_summary();
    }
}

struct PhaseGuard<'a> {
    timings: &'a mut PhaseTimings,
    label: String,
    started: Instant,
}

impl Drop for PhaseGuard<'_> {
    fn drop(&mut self) {
        let elapsed = self.started.elapsed();
        eprintln!(
            "[phase-timer] {}: {:.3}s",
            self.label,
            elapsed.as_secs_f64()
        );
        self.timings.phases.push((self.label.clone(), elapsed));
    }
}

#[test]
fn cross_dataspace_atomic_swap_is_all_or_nothing() -> Result<()> {
    let seed = corridor_seed_from_env()?;
    let context = stringify!(cross_dataspace_atomic_swap_is_all_or_nothing);
    run_cross_dataspace_localnet_test_on_large_stack(context, move || {
        cross_dataspace_atomic_swap_is_all_or_nothing_impl(context, seed, CorridorRunMode::SeedCase)
    })
}

#[test]
#[ignore = "two-hour rotating-validator 12-peer fault soak"]
fn cross_dataspace_two_hour_fault_soak_preserves_multilane_application() -> Result<()> {
    let seed = corridor_seed_from_env()?;
    let duration = fault_soak_duration_from_env()?;
    let context = stringify!(cross_dataspace_two_hour_fault_soak_preserves_multilane_application);
    run_cross_dataspace_localnet_test_on_large_stack(context, move || {
        cross_dataspace_atomic_swap_is_all_or_nothing_impl(
            context,
            seed,
            CorridorRunMode::FaultSoak { duration },
        )
    })
}

fn run_cross_dataspace_localnet_test_on_large_stack<F>(name: &'static str, test: F) -> Result<()>
where
    F: FnOnce() -> Result<()> + Send + 'static,
{
    // The 12-peer localnet startup exceeds default libtest stack budgets on some hosts.
    let handle = thread::Builder::new()
        .name(name.to_owned())
        .stack_size(CROSS_DATASPACE_LOCALNET_STACK_BYTES)
        .spawn(test)
        .expect("spawn cross-dataspace localnet test thread");
    match handle.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

fn cross_dataspace_atomic_swap_is_all_or_nothing_impl(
    context: &'static str,
    seed: CorridorSeed,
    run_mode: CorridorRunMode,
) -> Result<()> {
    let mut phase_timings = PhaseTimings::new(context);
    eprintln!(
        "[corridor] seed={} ordinal={} mode={run_mode:?}",
        seed.value, seed.ordinal
    );
    let (network, rt) = {
        let _phase = phase_timings.phase("start 12-peer localnet");
        let started =
            sandbox::start_network_blocking_or_skip(localnet_builder(&seed.value), context)?;
        let Some((network, rt)) = sandbox::enforce_network_start_requirement(started, context)?
        else {
            return Ok(());
        };
        (network, rt)
    };

    let alice = network.client();
    let bob = network
        .peer()
        .client_for(&BOB_ID, BOB_KEYPAIR.private_key().clone());

    let (expected_nexus_validators, expected_ds1_validators, expected_ds2_validators) = {
        let _phase = phase_timings.phase("derive 4+4+4 lane validator sets");
        let peers = network.peers();
        ensure!(
            peers.len() == TOTAL_PEERS,
            "expected {TOTAL_PEERS} peers for cross-dataspace topology, got {}",
            peers.len()
        );
        let nexus_lane_validators: Vec<ExpectedLaneValidatorBinding> = peers
            .iter()
            .enumerate()
            .take(VALIDATORS_PER_LANE)
            .map(|(index, peer)| expected_lane_binding_for_peer(index, &peer.id()))
            .collect();
        let ds1_lane_validators: Vec<ExpectedLaneValidatorBinding> = peers
            .iter()
            .enumerate()
            .skip(VALIDATORS_PER_LANE)
            .take(VALIDATORS_PER_LANE)
            .map(|(index, peer)| expected_lane_binding_for_peer(index, &peer.id()))
            .collect();
        let ds2_lane_validators: Vec<ExpectedLaneValidatorBinding> = peers
            .iter()
            .enumerate()
            .skip(VALIDATORS_PER_LANE * 2)
            .take(VALIDATORS_PER_LANE)
            .map(|(index, peer)| expected_lane_binding_for_peer(index, &peer.id()))
            .collect();
        let mut all_validators = Vec::with_capacity(TOTAL_PEERS);
        all_validators.extend(nexus_lane_validators.iter().cloned());
        all_validators.extend(ds1_lane_validators.iter().cloned());
        all_validators.extend(ds2_lane_validators.iter().cloned());
        let unique_validators: BTreeSet<_> = all_validators.into_iter().collect();
        ensure!(
            unique_validators.len() == TOTAL_PEERS,
            "validator groups must be disjoint and total {}",
            TOTAL_PEERS
        );
        let expected_nexus_validators: BTreeSet<_> =
            nexus_lane_validators.iter().cloned().collect();
        let expected_ds1_validators: BTreeSet<_> = ds1_lane_validators.iter().cloned().collect();
        let expected_ds2_validators: BTreeSet<_> = ds2_lane_validators.iter().cloned().collect();
        (
            expected_nexus_validators,
            expected_ds1_validators,
            expected_ds2_validators,
        )
    };

    {
        let _phase = phase_timings.phase("wait for lane validators + cross-peer sync");
        wait_for_active_lane_validators(
            &alice,
            LaneId::new(NEXUS_LANE_INDEX),
            &expected_nexus_validators,
            "nexus lane validator activation",
        )?;
        wait_for_active_lane_validators(
            &alice,
            LaneId::new(DS1_LANE_INDEX),
            &expected_ds1_validators,
            "ds1 lane validator activation",
        )?;
        wait_for_active_lane_validators(
            &alice,
            LaneId::new(DS2_LANE_INDEX),
            &expected_ds2_validators,
            "ds2 lane validator activation",
        )?;
        let lane_sync_height = alice
            .get_sumeragi_status()
            .map_err(|err| eyre!(err))?
            .last_committed_height;
        let lane_sync_status = wait_for_height(
            &alice,
            lane_sync_height,
            "lane validator activation local height",
        )?;
        let _lane_sync_on_bob = wait_for_height(
            &bob,
            lane_sync_height,
            "lane validator activation propagation",
        )?;
        for (lane_index, context) in [
            (NEXUS_LANE_INDEX, "nexus lane validator commit QC sync"),
            (DS1_LANE_INDEX, "ds1 lane validator commit QC sync"),
            (DS2_LANE_INDEX, "ds2 lane validator commit QC sync"),
        ] {
            wait_for_lane_peers_commit_qc_at_least(
                &network,
                lane_index,
                &lane_sync_status,
                &alice,
                context,
                STATUS_WAIT_TIMEOUT,
                LANE_PROGRESS_RECOVERY_TICK_EVERY_POLLS,
            )?;
        }
    }

    let nexus_alice_submitter = leader_targeted_client_for_lane(
        &network,
        &alice,
        &ALICE_ID,
        ALICE_KEYPAIR.private_key(),
        NEXUS_LANE_INDEX,
    );
    let nexus_bob_submitter = leader_targeted_client_for_lane(
        &network,
        &bob,
        &BOB_ID,
        BOB_KEYPAIR.private_key(),
        NEXUS_LANE_INDEX,
    );
    let ds1_tick_submitters = vec![leader_targeted_client_for_lane(
        &network,
        &alice,
        &ALICE_ID,
        ALICE_KEYPAIR.private_key(),
        DS1_LANE_INDEX,
    )];
    let ds2_tick_submitters = vec![leader_targeted_client_for_lane(
        &network,
        &bob,
        &BOB_ID,
        BOB_KEYPAIR.private_key(),
        DS2_LANE_INDEX,
    )];
    let neutral_tick_submitter_a_keypair = validator_authority_keypair(0);
    let neutral_tick_submitter_a_account = validator_authority_account_for_peer(0);
    let neutral_tick_submitter_a = leader_targeted_client_for_lane(
        &network,
        &alice,
        &neutral_tick_submitter_a_account,
        neutral_tick_submitter_a_keypair.private_key(),
        NEXUS_LANE_INDEX,
    );
    let neutral_tick_submitter_b_keypair = validator_authority_keypair(1);
    let neutral_tick_submitter_b_account = validator_authority_account_for_peer(1);
    let neutral_tick_submitter_b = leader_targeted_client_for_lane(
        &network,
        &alice,
        &neutral_tick_submitter_b_account,
        neutral_tick_submitter_b_keypair.private_key(),
        NEXUS_LANE_INDEX,
    );
    let config_layers = network
        .config_layers()
        .map(|layer| ConfigLayer(layer.into_owned()))
        .collect::<Vec<_>>();
    {
        let _phase =
            phase_timings.phase("G-12P autoscale lifecycle: A retire, B recreate, stale-A reject");
        prove_g12p_autoscale_lifecycle(
            &network,
            &rt,
            &[
                neutral_tick_submitter_a.clone(),
                neutral_tick_submitter_b.clone(),
            ],
            &config_layers,
            &seed,
        )?;
    }
    let (ds1_observation, ds2_observation) = {
        let _phase = phase_timings.phase("route probes ds1+ds2: tx submit + route wait");
        rt.block_on(async {
            tokio::try_join!(
                wait_for_route_probe_approval(
                    &nexus_alice_submitter,
                    InstructionBox::from(Log::new(Level::INFO, "route probe ds1".to_string())),
                    LaneId::new(DS1_LANE_INDEX),
                    DataSpaceId::new(DS1_ID_U64),
                    "route probe ds1",
                ),
                wait_for_route_probe_approval(
                    &nexus_bob_submitter,
                    InstructionBox::from(Log::new(Level::INFO, "route probe ds2".to_string())),
                    LaneId::new(DS2_LANE_INDEX),
                    DataSpaceId::new(DS2_ID_U64),
                    "route probe ds2",
                )
            )
        })?
    };
    let ds1_height = ds1_observation.height;
    let ds2_height = ds2_observation.height;
    let ds1_followup_observation = {
        let _phase = phase_timings.phase("route probe ds1 follow-up: tx submit + route wait");
        rt.block_on(wait_for_route_probe_approval(
            &nexus_alice_submitter,
            InstructionBox::from(Log::new(
                Level::INFO,
                "route probe ds1 follow-up".to_string(),
            )),
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            "route probe ds1 follow-up",
        ))?
    };
    {
        let _phase = phase_timings.phase("route probe ds1: query/assert");
        let ds1_source = if ds1_observation.approval_observed {
            "approved"
        } else {
            "fallback+1"
        };
        eprintln!(
            "[route-probe] ds1 first_seen={}s height={} source={}",
            ds1_observation.elapsed.as_secs_f64(),
            ds1_height,
            ds1_source
        );
    }
    {
        let _phase = phase_timings.phase("route probe ds2: query/assert");
        let ds2_source = if ds2_observation.approval_observed {
            "approved"
        } else {
            "fallback+1"
        };
        eprintln!(
            "[route-probe] ds2 first_seen={}s height={} source={}",
            ds2_observation.elapsed.as_secs_f64(),
            ds2_height,
            ds2_source
        );
        if ds1_observation.elapsed >= ds2_observation.elapsed {
            eprintln!(
                "[route-probe] ds1 lag vs ds2 = {:.3}s",
                ds1_observation
                    .elapsed
                    .saturating_sub(ds2_observation.elapsed)
                    .as_secs_f64()
            );
        } else {
            eprintln!(
                "[route-probe] ds2 lag vs ds1 = {:.3}s",
                ds2_observation
                    .elapsed
                    .saturating_sub(ds1_observation.elapsed)
                    .as_secs_f64()
            );
        }
    }
    {
        let _phase = phase_timings.phase("route probes: independent lane-payload ownership");
        let (ds1_progress, ds2_progress) = wait_for_independent_lane_payload_ownership_progress(
            &network,
            &alice,
            &ds1_tick_submitters,
            &ds2_tick_submitters,
            (LaneId::new(DS1_LANE_INDEX), DataSpaceId::new(DS1_ID_U64)),
            (LaneId::new(DS2_LANE_INDEX), DataSpaceId::new(DS2_ID_U64)),
            "route probes independent lane-payload ownership progress",
        )?;
        eprintln!(
            "[route-probe] ds1 follow-up height={} approval_observed={} lane_block={}/{} ds2_lane_block={}/{} ds1_committee={}/{} ds2_committee={}/{} accepted={}/{}",
            ds1_followup_observation.height,
            ds1_followup_observation.approval_observed,
            ds1_progress.lane_block_height,
            ds1_progress.lane_block_view,
            ds2_progress.lane_block_height,
            ds2_progress.lane_block_view,
            ds1_progress.min_quorum,
            ds1_progress.validator_count,
            ds2_progress.min_quorum,
            ds2_progress.validator_count,
            ds1_progress.accepted_transaction_count,
            ds2_progress.accepted_transaction_count,
        );
    }
    {
        let _phase = phase_timings.phase("route probes: committed lane-block QCs");
        let (ds1_progress, ds2_progress) = wait_for_independent_lane_domain_progress(
            &network,
            &ds1_tick_submitters,
            &ds2_tick_submitters,
            (LaneId::new(DS1_LANE_INDEX), DataSpaceId::new(DS1_ID_U64)),
            (LaneId::new(DS2_LANE_INDEX), DataSpaceId::new(DS2_ID_U64)),
            "route probes committed lane-block progress",
        )?;
        eprintln!(
            "[route-probe] committed lane-block ds1={}/{} ds2={}/{} ds1_qc={}/{} ds2_qc={}/{} ds1_commitment={:?} ds2_commitment={:?}",
            ds1_progress.lane_block_height,
            ds1_progress.lane_block_view,
            ds2_progress.lane_block_height,
            ds2_progress.lane_block_view,
            ds1_progress.prepare_qc_signer_count,
            ds1_progress.commit_qc_signer_count,
            ds2_progress.prepare_qc_signer_count,
            ds2_progress.commit_qc_signer_count,
            ds1_progress.dataspace_commitment_height,
            ds2_progress.dataspace_commitment_height,
        );
    }
    {
        let _phase = phase_timings.phase("route probes: applied committed lane-block evidence");
        let (ds1_progress, ds2_progress) = wait_for_independent_lane_application_progress(
            &network,
            std::slice::from_ref(&neutral_tick_submitter_a),
            std::slice::from_ref(&neutral_tick_submitter_b),
            (LaneId::new(DS1_LANE_INDEX), DataSpaceId::new(DS1_ID_U64)),
            (LaneId::new(DS2_LANE_INDEX), DataSpaceId::new(DS2_ID_U64)),
            "route probes applied committed lane-block progress",
        )?;
        eprintln!(
            "[route-probe] applied committed lane-block ds1={}/{} status={} ds2={}/{} status={} ds1_qc={}/{} ds2_qc={}/{}",
            ds1_progress.lane_block_height,
            ds1_progress.lane_block_view,
            ds1_progress.execution_status,
            ds2_progress.lane_block_height,
            ds2_progress.lane_block_view,
            ds2_progress.execution_status,
            ds1_progress.prepare_qc_signer_count,
            ds1_progress.commit_qc_signer_count,
            ds2_progress.prepare_qc_signer_count,
            ds2_progress.commit_qc_signer_count,
        );
    }
    let ds1_asset_def: AssetDefinitionId = AssetDefinitionId::new(
        DomainId::try_new("nexus", "universal").expect("asset definition"),
        "ds1coin".parse().expect("asset definition"),
    );
    let ds2_asset_def: AssetDefinitionId = AssetDefinitionId::new(
        DomainId::try_new("nexus", "universal").expect("asset definition"),
        "ds2coin".parse().expect("asset definition"),
    );
    let bob_transfer_ds1_permission: Permission = CanTransferAssetWithDefinition {
        asset_definition: ds1_asset_def.clone(),
    }
    .into();
    let alice_ds1_asset = AssetId::new(ds1_asset_def.clone(), ALICE_ID.clone());
    let bob_ds1_asset = AssetId::new(ds1_asset_def.clone(), BOB_ID.clone());
    let alice_ds2_asset = AssetId::new(ds2_asset_def.clone(), ALICE_ID.clone());
    let bob_ds2_asset = AssetId::new(ds2_asset_def.clone(), BOB_ID.clone());
    let alice_on_ds1 = leader_targeted_client_for_lane(
        &network,
        &alice,
        &ALICE_ID,
        ALICE_KEYPAIR.private_key(),
        DS1_LANE_INDEX,
    );
    let bob_on_ds1 = leader_targeted_client_for_lane(
        &network,
        &alice,
        &BOB_ID,
        BOB_KEYPAIR.private_key(),
        DS1_LANE_INDEX,
    );
    let alice_on_ds2 = leader_targeted_client_for_lane(
        &network,
        &alice,
        &ALICE_ID,
        ALICE_KEYPAIR.private_key(),
        DS2_LANE_INDEX,
    );
    let bob_on_ds2 = leader_targeted_client_for_lane(
        &network,
        &alice,
        &BOB_ID,
        BOB_KEYPAIR.private_key(),
        DS2_LANE_INDEX,
    );

    {
        let _phase = phase_timings.phase("route probes: wrong-dataspace query/assert");
        ensure!(
            asset_balance(&nexus_alice_submitter, &alice_ds1_asset)?
                == Quantity::from(DS1_WORKLOAD_SEED_AMOUNT),
            "Alice ds1 balance query through Nexus ingress did not route to ds1"
        );
        ensure!(
            asset_balance(&nexus_bob_submitter, &bob_ds2_asset)?
                == Quantity::from(DS2_WORKLOAD_SEED_AMOUNT),
            "Bob ds2 balance query through Nexus ingress did not route to ds2"
        );
    }
    {
        let _phase = phase_timings.phase("route probes: wrong-dataspace app-api query/assert");
        let ds1_asset_literal = ds1_asset_def.to_string();
        let alice_account_literal = ALICE_ID.to_string();
        let expected_ds1_lane_literal = DS1_LANE_INDEX.to_string();
        let expected_ds1_dataspace_literal = DS1_ID_U64.to_string();
        let validators_path = vec![
            "v1".to_owned(),
            "nexus".to_owned(),
            "public_lanes".to_owned(),
            DS1_LANE_INDEX.to_string(),
            "validators".to_owned(),
        ];
        let account_assets_path = vec![
            "v1".to_owned(),
            "accounts".to_owned(),
            alice_account_literal.clone(),
            "assets".to_owned(),
        ];
        let asset_definition_path = vec![
            "v1".to_owned(),
            "assets".to_owned(),
            "definitions".to_owned(),
            ds1_asset_literal.clone(),
        ];
        let account_summary_path = vec![
            "v1".to_owned(),
            "nexus".to_owned(),
            "dataspaces".to_owned(),
            "accounts".to_owned(),
            alice_account_literal.clone(),
            "summary".to_owned(),
        ];
        rt.block_on(async {
            let validators = torii_json_get_with_retry(
                &nexus_alice_submitter,
                &validators_path,
                &[],
                "DS1 validator query through Nexus ingress",
            )
            .await?;
            ensure!(
                validators.routed_by.as_deref() == Some("proxy"),
                "DS1 validator query through Nexus ingress should be proxied, observed {:?}",
                validators.routed_by
            );
            ensure!(
                validators.route_lane_id.as_deref() == Some(expected_ds1_lane_literal.as_str()),
                "DS1 validator query should advertise lane {DS1_LANE_INDEX}, observed {:?}",
                validators.route_lane_id
            );
            ensure!(
                validators.route_dataspace_id.as_deref()
                    == Some(expected_ds1_dataspace_literal.as_str()),
                "DS1 validator query should advertise dataspace {DS1_ID_U64}, observed {:?}",
                validators.route_dataspace_id
            );
            let (total, active) =
                lane_validator_snapshot(&validators.body, "nexus-routed ds1 validator query")?;
            ensure!(
                total == expected_ds1_validators.len() && active == expected_ds1_validators,
                "DS1 validator query through Nexus ingress returned total {total} active {:?}, expected total {} active {:?}",
                active,
                expected_ds1_validators.len(),
                expected_ds1_validators
            );

            let account_assets = torii_json_get_with_retry(
                &nexus_alice_submitter,
                &account_assets_path,
                &[],
                "account assets query through Nexus ingress",
            )
            .await?;
            expect_local_or_proxy_fanout_headers(
                &account_assets,
                "account assets query through Nexus ingress",
            )?;
            let account_asset_items = account_assets
                .body
                .get("items")
                .and_then(JsonValue::as_array)
                .ok_or_else(|| eyre!("account assets response missing items array"))?;
            ensure!(
                account_asset_items.iter().any(|item| {
                    item.get("asset").and_then(JsonValue::as_str) == Some(ds1_asset_literal.as_str())
                }),
                "account assets query through Nexus ingress did not include routed asset definition {} in {:?}",
                ds1_asset_literal,
                account_asset_items
            );

            let asset_definition = torii_json_get_with_retry(
                &nexus_alice_submitter,
                &asset_definition_path,
                &[],
                "asset definition query through Nexus ingress",
            )
            .await?;
            expect_local_or_proxy_fanout_headers(
                &asset_definition,
                "asset definition query through Nexus ingress",
            )?;
            ensure!(
                asset_definition.body["id"].as_str() == Some(ds1_asset_literal.as_str()),
                "asset definition query through Nexus ingress returned unexpected id {:?}",
                asset_definition.body["id"].as_str()
            );

            let account_summary = torii_json_get_with_retry(
                &nexus_alice_submitter,
                &account_summary_path,
                &[],
                "dataspace summary query through Nexus ingress",
            )
            .await?;
            expect_local_or_proxy_fanout_headers(
                &account_summary,
                "dataspace summary query through Nexus ingress",
            )?;
            ensure!(
                account_summary.body["account_id"].as_str() == Some(alice_account_literal.as_str()),
                "dataspace summary query through Nexus ingress returned unexpected account_id {:?}",
                account_summary.body["account_id"].as_str()
            );
            let summary_rows = account_summary
                .body
                .get("dataspaces")
                .and_then(JsonValue::as_array)
                .ok_or_else(|| eyre!("dataspace summary response missing dataspaces array"))?;
            let summary_totals = account_summary
                .body
                .get("totals")
                .and_then(JsonValue::as_object)
                .ok_or_else(|| eyre!("dataspace summary response missing totals object"))?;
            ensure!(
                summary_totals.get("dataspaces").and_then(JsonValue::as_u64)
                    == Some(summary_rows.len() as u64),
                "dataspace summary query through Nexus ingress returned inconsistent totals {:?} vs rows {:?}",
                summary_totals,
                summary_rows
            );
            Ok::<(), eyre::Report>(())
        })?;
    }

    let setup_grants_lane = (
        LaneId::new(NEXUS_LANE_INDEX),
        DataSpaceId::new(NEXUS_ID_U64),
    );
    let build_setup_grants_tx = |client: &Client| {
        client.build_transaction(
            vec![InstructionBox::from(Grant::account_permission(
                CanTransferAssetWithDefinition {
                    asset_definition: ds1_asset_def.clone(),
                },
                BOB_ID.clone(),
            ))],
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            Metadata::default(),
        )
    };
    let (
        setup_grants_tx,
        setup_grants_authoritative_lane_index,
        setup_grants_pre_barrier_height,
        setup_grants_pre_application,
    ) = {
        let submitter = alice_on_ds1.clone();
        let _phase = phase_timings.phase("setup grants: tx submit enqueue");
        let setup_grants_tx = build_setup_grants_tx(&submitter);
        let nexus_pre_barrier_height = nexus_alice_submitter
            .get_sumeragi_status()
            .map_err(|err| eyre!(err))?
            .last_committed_height;
        let ds1_pre_barrier_height = alice_on_ds1
            .get_sumeragi_status()
            .map_err(|err| eyre!(err))?
            .last_committed_height;
        let ds2_pre_barrier_height = alice_on_ds2
            .get_sumeragi_status()
            .map_err(|err| eyre!(err))?
            .last_committed_height;
        let pre_application = current_lane_application_progress(&network, setup_grants_lane);
        let route_observation = match rt.block_on(submit_transaction_with_route_observation(
            &submitter,
            &setup_grants_tx,
            "setup grants route observation",
        )) {
            Ok(observation) => Some(observation),
            Err(err)
                if err
                    .to_string()
                    .contains("timed out observing queued transaction route") =>
            {
                eprintln!(
                    "[setup-grants] route observation timed out after submit; continuing with authoritative lane fallback"
                );
                None
            }
            Err(err) => return Err(err),
        };
        // A queued event from the ingress listener can still reflect the submitter-side lane even
        // after the authoritative router forwards the transaction. This grant targets a universal
        // asset-definition permission, so the authoritative route is the Nexus lane regardless of
        // the route event lane observed through the submitter.
        let authoritative_lane_index = NEXUS_LANE_INDEX;
        let pre_barrier_height = match authoritative_lane_index {
            NEXUS_LANE_INDEX => nexus_pre_barrier_height,
            DS1_LANE_INDEX => ds1_pre_barrier_height,
            DS2_LANE_INDEX => ds2_pre_barrier_height,
            _ => {
                leader_targeted_client_for_lane(
                    &network,
                    &alice,
                    &ALICE_ID,
                    ALICE_KEYPAIR.private_key(),
                    authoritative_lane_index,
                )
                .get_sumeragi_status()
                .map_err(|err| eyre!(err))?
                .last_committed_height
            }
        };
        if let Some(route_observation) = route_observation {
            eprintln!(
                "[setup-grants] observed lane={} dataspace={} approved_height={:?} authoritative_lane={}",
                route_observation.lane_id.as_u32(),
                route_observation.dataspace_id.as_u64(),
                route_observation.approved_height,
                authoritative_lane_index,
            );
        } else {
            eprintln!(
                "[setup-grants] route observation unavailable authoritative_lane={}",
                authoritative_lane_index,
            );
        }
        (
            setup_grants_tx,
            authoritative_lane_index,
            pre_barrier_height,
            pre_application,
        )
    };
    let mut setup_grants_application_baseline = setup_grants_pre_application;
    {
        let _phase = phase_timings.phase("setup grants: tx committed outcome");
        let setup_grants_entry_hash = setup_grants_tx.hash_as_entrypoint();
        let mut committed_outcome_confirmed = false;
        match wait_for_committed_success_or_height_fallback_across_clients(
            || {
                lane_targeted_clients_for_lane(
                    &network,
                    &alice,
                    &ALICE_ID,
                    ALICE_KEYPAIR.private_key(),
                    setup_grants_authoritative_lane_index,
                )
            },
            &[],
            setup_grants_entry_hash,
            "setup grants committed outcome on authoritative observer",
            "setup grants commit barrier on authoritative observer",
            setup_grants_pre_barrier_height,
            BLOCKING_CONFIRMATION_TIMEOUT,
            BLOCKING_CONFIRMATION_TIMEOUT,
        ) {
            Ok(observation) => {
                committed_outcome_confirmed = observation.committed_outcome_confirmed;
            }
            Err(setup_err) => {
                eprintln!(
                    "[setup-grants] committed outcome did not converge before permission assertion: {setup_err}"
                );
            }
        }
        if !committed_outcome_confirmed {
            eprintln!(
                "[setup-grants] committed outcome remained inconclusive after height barrier; resubmitting fresh grant transaction"
            );
            let authoritative_submitter = leader_targeted_client_for_lane(
                &network,
                &alice,
                &ALICE_ID,
                ALICE_KEYPAIR.private_key(),
                setup_grants_authoritative_lane_index,
            );
            let retry_pre_barrier_height = authoritative_submitter
                .get_sumeragi_status()
                .map_err(|err| eyre!(err))?
                .last_committed_height;
            let retry_pre_application =
                current_lane_application_progress(&network, setup_grants_lane);
            let setup_grants_retry_tx = build_setup_grants_tx(&authoritative_submitter);
            match submit_transaction_across_clients(
                || {
                    lane_targeted_clients_for_lane(
                        &network,
                        &alice,
                        &ALICE_ID,
                        ALICE_KEYPAIR.private_key(),
                        setup_grants_authoritative_lane_index,
                    )
                },
                &setup_grants_retry_tx,
                "setup grants authoritative resubmit",
                SUBMIT_ENQUEUE_REQUEST_TIMEOUT,
            ) {
                Ok(_) => {
                    setup_grants_application_baseline = retry_pre_application;
                    let setup_grants_retry_entry_hash = setup_grants_retry_tx.hash_as_entrypoint();
                    match wait_for_committed_success_or_height_fallback_across_clients(
                        || {
                            lane_targeted_clients_for_lane(
                                &network,
                                &alice,
                                &ALICE_ID,
                                ALICE_KEYPAIR.private_key(),
                                setup_grants_authoritative_lane_index,
                            )
                        },
                        &[],
                        setup_grants_retry_entry_hash,
                        "setup grants retry committed outcome on authoritative observer",
                        "setup grants retry commit barrier on authoritative observer",
                        retry_pre_barrier_height,
                        BLOCKING_CONFIRMATION_TIMEOUT,
                        BLOCKING_CONFIRMATION_TIMEOUT,
                    ) {
                        Ok(observation) => {
                            committed_outcome_confirmed = observation.committed_outcome_confirmed;
                            if !committed_outcome_confirmed {
                                eprintln!(
                                    "[setup-grants] retry committed outcome remained inconclusive after height barrier"
                                );
                            }
                        }
                        Err(retry_err) => {
                            eprintln!(
                                "[setup-grants] retry committed outcome did not converge before permission assertion: {retry_err}"
                            );
                        }
                    }
                }
                Err(resubmit_err) => {
                    eprintln!(
                        "[setup-grants] authoritative resubmit did not enqueue before permission assertion: {resubmit_err}"
                    );
                }
            }
        }
    };
    {
        let _phase = phase_timings.phase("setup grants: Nexus lane application");
        match wait_for_lane_application_progress_after(
            &network,
            &[],
            setup_grants_lane,
            setup_grants_application_baseline.as_ref(),
            "setup grants Nexus lane application",
        ) {
            Ok(setup_progress) => {
                eprintln!(
                    "[setup-grants] applied Nexus lane block={}/{} status={}",
                    setup_progress.lane_block_height,
                    setup_progress.lane_block_view,
                    setup_progress.execution_status,
                );
            }
            Err(err) => {
                eprintln!(
                    "[setup-grants] Nexus lane application did not converge before permission assertion: {err}"
                );
            }
        }
    }
    {
        let _phase = phase_timings.phase("setup grants: query/assert");
        // Confirm Bob-facing routed views before proceeding to the swap path that depends on them.
        wait_for_account_permissions_across_clients(
            &[],
            || {
                let mut clients = Vec::new();
                for lane_index in [NEXUS_LANE_INDEX, DS1_LANE_INDEX, DS2_LANE_INDEX] {
                    clients.extend(lane_targeted_clients_for_lane(
                        &network,
                        &bob,
                        &BOB_ID,
                        BOB_KEYPAIR.private_key(),
                        lane_index,
                    ));
                }
                clients
            },
            &BOB_ID,
            &[bob_transfer_ds1_permission.clone()],
            "grant setup permissions visible on routed bob observers",
        )?;
    }

    let seeded_balances = [
        BalanceExpectation {
            client: &alice_on_ds1,
            asset_id: &alice_ds1_asset,
            expected: Quantity::from(DS1_WORKLOAD_SEED_AMOUNT),
        },
        BalanceExpectation {
            client: &bob_on_ds1,
            asset_id: &bob_ds1_asset,
            expected: Quantity::from(0_u32),
        },
        BalanceExpectation {
            client: &alice_on_ds2,
            asset_id: &alice_ds2_asset,
            expected: Quantity::from(0_u32),
        },
        BalanceExpectation {
            client: &bob_on_ds2,
            asset_id: &bob_ds2_asset,
            expected: Quantity::from(DS2_WORKLOAD_SEED_AMOUNT),
        },
    ];
    let setup_register_mint_retries_used = 0usize;
    {
        let _phase = phase_timings.phase("setup register+mint: query/assert");
        wait_for_expected_balances_with_tick_timeout(
            &alice,
            &seeded_balances,
            "seed balances from genesis setup",
            SETUP_REGISTER_MINT_QUERY_TIMEOUT,
        )?;
    }
    let swap_outcome_fallbacks = 0usize;
    let mut swap_nonconverged_fallbacks = 0usize;
    let current_nexus_clients = || {
        (
            leader_targeted_client_for_lane(
                &network,
                &alice,
                &ALICE_ID,
                ALICE_KEYPAIR.private_key(),
                NEXUS_LANE_INDEX,
            ),
            leader_targeted_client_for_lane(
                &network,
                &alice,
                &BOB_ID,
                BOB_KEYPAIR.private_key(),
                NEXUS_LANE_INDEX,
            ),
        )
    };

    let mut durable_native_evidence = {
        let successful_swap = DvpIsi::new(
            "ds1ds2swapok".parse().expect("settlement id"),
            SettlementLeg::new(
                ds1_asset_def.clone(),
                Quantity::from(30_u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            SettlementLeg::new(
                ds2_asset_def.clone(),
                Quantity::from(45_u32),
                BOB_ID.clone(),
                ALICE_ID.clone(),
            ),
            SettlementPlan::new(
                SettlementExecutionOrder::DeliveryThenPayment,
                SettlementAtomicity::AllOrNothing,
            ),
        );
        grant_exact_dvp_consents_across_clients(
            || {
                let (_, bob_submitter) = current_nexus_clients();
                vec![bob_submitter]
            },
            &[(&successful_swap, bob_ds2_asset.clone())],
            &ALICE_ID,
            "successful swap counterparty consent",
        )?;
        let (submitter, _) = current_nexus_clients();
        let mut successful_swap_synced_status = None;
        let successful_swap_pre_application = wait_for_independent_lane_application_progress(
            &network,
            std::slice::from_ref(&neutral_tick_submitter_a),
            std::slice::from_ref(&neutral_tick_submitter_b),
            (LaneId::new(DS1_LANE_INDEX), DataSpaceId::new(DS1_ID_U64)),
            (LaneId::new(DS2_LANE_INDEX), DataSpaceId::new(DS2_ID_U64)),
            "successful swap pre-application baseline",
        )?;
        let (successful_swap_entry_hash, successful_swap_pre_barrier_height) = {
            let _phase = phase_timings.phase("execute successful swap: tx submit enqueue");
            let pre_barrier_height = submitter
                .get_sumeragi_status()
                .map_err(|err| eyre!(err))?
                .last_committed_height;
            let successful_swap_tx = submitter.build_transaction(
                [InstructionBox::from(successful_swap)],
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            );
            let successful_swap_entry_hash = successful_swap_tx.hash_as_entrypoint();
            let route_observation = rt.block_on(submit_transaction_with_route_observation(
                &submitter,
                &successful_swap_tx,
                "successful swap route observation",
            ))?;
            eprintln!(
                "[swap] successful swap observed lane={} dataspace={} approved_height={:?}",
                route_observation.lane_id.as_u32(),
                route_observation.dataspace_id.as_u64(),
                route_observation.approved_height,
            );
            (successful_swap_entry_hash, pre_barrier_height)
        };
        {
            let _phase = phase_timings.phase("execute successful swap: barrier wait");
            match wait_for_committed_success_or_height_fallback_across_clients(
                || {
                    lane_targeted_clients_for_lane(
                        &network,
                        &alice,
                        &ALICE_ID,
                        ALICE_KEYPAIR.private_key(),
                        NEXUS_LANE_INDEX,
                    )
                },
                &[],
                successful_swap_entry_hash,
                "successful swap confirmation on Nexus authoritative observer",
                "successful swap barrier on Nexus authoritative observer (height fallback)",
                successful_swap_pre_barrier_height,
                SWAP_COMMITTED_OUTCOME_TIMEOUT,
                SWAP_POST_BARRIER_OUTCOME_TIMEOUT,
            ) {
                Ok(observation) => successful_swap_synced_status = Some(observation.status),
                Err(fallback_err) => {
                    swap_nonconverged_fallbacks = swap_nonconverged_fallbacks.saturating_add(1);
                    eprintln!(
                        "[swap] successful swap fallback did not converge before balance assertion: {fallback_err}"
                    );
                }
            }
        }
        if let Some(status) = successful_swap_synced_status.as_ref() {
            if let Err(sync_err) = wait_for_lane_peers_commit_qc_at_least(
                &network,
                NEXUS_LANE_INDEX,
                status,
                &neutral_tick_submitter_a,
                "successful swap Nexus authoritative sync",
                STATUS_WAIT_TIMEOUT,
                SETUP_BARRIER_TICK_EVERY_POLLS,
            ) {
                swap_nonconverged_fallbacks = swap_nonconverged_fallbacks.saturating_add(1);
                eprintln!(
                    "[swap] successful swap Nexus authoritative sync did not converge before balance assertion: {sync_err}"
                );
            }
        }
        let durable_native_evidence = {
            let _phase = phase_timings.phase("execute successful swap: query/assert");
            let successful_swap_ds1_tick_submitters = [neutral_tick_submitter_a.clone()];
            let ds1_progress = wait_for_lane_application_progress_after(
                &network,
                &successful_swap_ds1_tick_submitters,
                (
                    successful_swap_pre_application.0.lane_id,
                    successful_swap_pre_application.0.dataspace_id,
                ),
                Some(&successful_swap_pre_application.0),
                "successful swap applied DS1 coordinator-lane progress",
            )?;
            let ds2_progress = wait_for_durable_native_participant_evidence_after(
                &network,
                successful_swap_pre_application.1.lane_id,
                successful_swap_pre_application.1.dataspace_id,
                successful_swap_pre_application.1.lane_incarnation,
                None,
                Some(successful_swap_pre_barrier_height),
                "successful swap durable DS2 participant progress",
            )?;
            ensure!(
                ds2_progress.participant_height
                    > successful_swap_pre_application.1.lane_block_height,
                "successful swap DS2 participant height did not advance beyond its applied lane baseline (before {}, after {})",
                successful_swap_pre_application.1.lane_block_height,
                ds2_progress.participant_height
            );
            eprintln!(
                "[swap] applied DS1 coordinator progress={}/{} status={} durable DS2 participant progress={}/{} carrier={:?}",
                ds1_progress.lane_block_height,
                ds1_progress.lane_block_view,
                ds1_progress.execution_status,
                ds2_progress.participant_height,
                ds2_progress.participant_view,
                ds2_progress.application_block_height,
            );
            let successful_swap_tick_submitters = [
                neutral_tick_submitter_a.clone(),
                neutral_tick_submitter_b.clone(),
            ];
            let alice_ds1_balance_clients = lane_targeted_clients_for_lane(
                &network,
                &alice,
                &ALICE_ID,
                ALICE_KEYPAIR.private_key(),
                DS1_LANE_INDEX,
            );
            let bob_ds1_balance_clients = lane_targeted_clients_for_lane(
                &network,
                &alice,
                &BOB_ID,
                BOB_KEYPAIR.private_key(),
                DS1_LANE_INDEX,
            );
            let alice_ds2_balance_clients = lane_targeted_clients_for_lane(
                &network,
                &alice,
                &ALICE_ID,
                ALICE_KEYPAIR.private_key(),
                DS2_LANE_INDEX,
            );
            let bob_ds2_balance_clients = lane_targeted_clients_for_lane(
                &network,
                &alice,
                &BOB_ID,
                BOB_KEYPAIR.private_key(),
                DS2_LANE_INDEX,
            );
            let successful_balances = expected_post_swap_balances(0)?;
            let successful_swap_expectations = [
                BalanceExpectationAcrossClients {
                    clients: &alice_ds1_balance_clients,
                    asset_id: &alice_ds1_asset,
                    expected: successful_balances[0].clone(),
                },
                BalanceExpectationAcrossClients {
                    clients: &bob_ds1_balance_clients,
                    asset_id: &bob_ds1_asset,
                    expected: successful_balances[1].clone(),
                },
                BalanceExpectationAcrossClients {
                    clients: &alice_ds2_balance_clients,
                    asset_id: &alice_ds2_asset,
                    expected: successful_balances[2].clone(),
                },
                BalanceExpectationAcrossClients {
                    clients: &bob_ds2_balance_clients,
                    asset_id: &bob_ds2_asset,
                    expected: successful_balances[3].clone(),
                },
            ];
            wait_for_expected_balances_across_clients_with_tick_submitters_timeout(
                &successful_swap_tick_submitters,
                &successful_swap_expectations,
                "successful swap balances after DS application",
                LANE_PROGRESS_WAIT_TIMEOUT,
            )?;
            ds2_progress
        };
        durable_native_evidence
    };
    let fault_phase_started = Instant::now();
    let mut completed_work_units = 0usize;
    let mut work_unit_durations = Vec::new();
    let mut observed_work_entrypoints = BTreeSet::new();
    {
        let phase_label = match run_mode {
            CorridorRunMode::SeedCase => {
                "strict rotating-outage Native + autonomous work unit".to_owned()
            }
            CorridorRunMode::FaultSoak { duration } => format!(
                "strict rotating-outage Native + autonomous fault soak for {} seconds",
                duration.as_secs()
            ),
        };
        let _phase = phase_timings.phase(phase_label);
        loop {
            if completed_work_units > 0 {
                match run_mode {
                    CorridorRunMode::SeedCase => break,
                    CorridorRunMode::FaultSoak { duration }
                        if fault_phase_started.elapsed() >= duration =>
                    {
                        break;
                    }
                    CorridorRunMode::FaultSoak { .. } => {}
                }
            }

            let iteration = completed_work_units;
            let iteration_started = Instant::now();
            let autonomous_pre_application = wait_for_independent_lane_application_progress(
                &network,
                std::slice::from_ref(&neutral_tick_submitter_a),
                std::slice::from_ref(&neutral_tick_submitter_b),
                (LaneId::new(DS1_LANE_INDEX), DataSpaceId::new(DS1_ID_U64)),
                (LaneId::new(DS2_LANE_INDEX), DataSpaceId::new(DS2_ID_U64)),
                &format!("work unit {iteration}: autonomous pre-application baseline"),
            )?;
            let offline_indices = rotating_validator_indices(seed.ordinal, iteration);
            let outage_status_index =
                (offline_indices[0].saturating_add(1)).wrapping_rem(VALIDATORS_PER_LANE);
            let outage_status_client = network.peers()[outage_status_index].client();
            let offline_peers = offline_indices
                .iter()
                .map(|index| network.peers()[*index].clone())
                .collect::<Vec<_>>();
            let outage_context = format!(
                "work unit {iteration}: validators {:?} offline",
                offline_indices
            );
            shutdown_rotated_validators(&rt, &offline_peers, &outage_context)?;

            let outage_result = (|| -> Result<[HashOf<TransactionEntrypoint>; 3]> {
                let soak_submitter = leader_targeted_client_for_lane(
                    &network,
                    &outage_status_client,
                    &ALICE_ID,
                    ALICE_KEYPAIR.private_key(),
                    NEXUS_LANE_INDEX,
                );
                let soak_bob_observer = leader_targeted_client_for_lane(
                    &network,
                    &outage_status_client,
                    &BOB_ID,
                    BOB_KEYPAIR.private_key(),
                    NEXUS_LANE_INDEX,
                );
                let forward_swap = DvpIsi::new(
                    format!("corridorf{:02}{iteration}", seed.ordinal)
                        .parse()
                        .expect("settlement id"),
                    SettlementLeg::new(
                        ds1_asset_def.clone(),
                        Quantity::from(5_u32),
                        ALICE_ID.clone(),
                        BOB_ID.clone(),
                    ),
                    SettlementLeg::new(
                        ds2_asset_def.clone(),
                        Quantity::from(5_u32),
                        BOB_ID.clone(),
                        ALICE_ID.clone(),
                    ),
                    SettlementPlan::new(
                        SettlementExecutionOrder::DeliveryThenPayment,
                        SettlementAtomicity::AllOrNothing,
                    ),
                );
                let reverse_swap = DvpIsi::new(
                    format!("corridorr{:02}{iteration}", seed.ordinal)
                        .parse()
                        .expect("settlement id"),
                    SettlementLeg::new(
                        ds2_asset_def.clone(),
                        Quantity::from(5_u32),
                        ALICE_ID.clone(),
                        BOB_ID.clone(),
                    ),
                    SettlementLeg::new(
                        ds1_asset_def.clone(),
                        Quantity::from(5_u32),
                        BOB_ID.clone(),
                        ALICE_ID.clone(),
                    ),
                    SettlementPlan::new(
                        SettlementExecutionOrder::DeliveryThenPayment,
                        SettlementAtomicity::AllOrNothing,
                    ),
                );
                grant_exact_dvp_consents_across_clients(
                    || vec![soak_bob_observer.clone()],
                    &[
                        (&forward_swap, bob_ds2_asset.clone()),
                        (&reverse_swap, bob_ds1_asset.clone()),
                    ],
                    &ALICE_ID,
                    &format!("work unit {iteration}: bilateral settlement consent"),
                )?;
                let paired_swap_tx = soak_submitter.build_transaction(
                    vec![
                        InstructionBox::from(forward_swap),
                        InstructionBox::from(reverse_swap),
                    ],
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                    Metadata::default(),
                );
                let paired_swap_entry_hash = paired_swap_tx.hash_as_entrypoint();
                submit_transaction_across_clients(
                    || {
                        lane_targeted_clients_for_lane(
                            &network,
                            &outage_status_client,
                            &ALICE_ID,
                            ALICE_KEYPAIR.private_key(),
                            NEXUS_LANE_INDEX,
                        )
                    },
                    &paired_swap_tx,
                    &format!("work unit {iteration}: paired Native swaps enqueue"),
                    SUBMIT_ENQUEUE_REQUEST_TIMEOUT,
                )?;
                wait_for_committed_success_across_clients(
                    || {
                        lane_targeted_clients_for_lane(
                            &network,
                            &outage_status_client,
                            &ALICE_ID,
                            ALICE_KEYPAIR.private_key(),
                            NEXUS_LANE_INDEX,
                        )
                    },
                    paired_swap_entry_hash.clone(),
                    &format!("work unit {iteration}: paired Native swaps committed"),
                    SOAK_PHASE_WAIT_TIMEOUT,
                )?;
                let pre_autonomous_balances = expected_post_swap_balances(completed_work_units)?;
                let native_net_zero_expectations = [
                    BalanceExpectation {
                        client: &soak_submitter,
                        asset_id: &alice_ds1_asset,
                        expected: pre_autonomous_balances[0].clone(),
                    },
                    BalanceExpectation {
                        client: &soak_bob_observer,
                        asset_id: &bob_ds1_asset,
                        expected: pre_autonomous_balances[1].clone(),
                    },
                    BalanceExpectation {
                        client: &soak_submitter,
                        asset_id: &alice_ds2_asset,
                        expected: pre_autonomous_balances[2].clone(),
                    },
                    BalanceExpectation {
                        client: &soak_bob_observer,
                        asset_id: &bob_ds2_asset,
                        expected: pre_autonomous_balances[3].clone(),
                    },
                ];
                wait_for_expected_balances_with_timeout(
                    &native_net_zero_expectations,
                    &format!("work unit {iteration}: paired Native swaps remain net zero"),
                    SOAK_PHASE_WAIT_TIMEOUT,
                )?;

                let ds1_submitter = leader_targeted_client_for_lane(
                    &network,
                    &outage_status_client,
                    &ALICE_ID,
                    ALICE_KEYPAIR.private_key(),
                    DS1_LANE_INDEX,
                );
                let ds2_submitter = leader_targeted_client_for_lane(
                    &network,
                    &outage_status_client,
                    &BOB_ID,
                    BOB_KEYPAIR.private_key(),
                    DS2_LANE_INDEX,
                );
                let ds1_autonomous_tx = ds1_submitter.build_transaction(
                    [Transfer::asset_quantity(
                        alice_ds1_asset.clone(),
                        1_u32,
                        BOB_ID.clone(),
                    )],
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                    Metadata::default(),
                );
                let ds2_autonomous_tx = ds2_submitter.build_transaction(
                    [Transfer::asset_quantity(
                        bob_ds2_asset.clone(),
                        1_u32,
                        ALICE_ID.clone(),
                    )],
                    iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                    Metadata::default(),
                );
                let ds1_entry_hash = ds1_autonomous_tx.hash_as_entrypoint();
                let ds2_entry_hash = ds2_autonomous_tx.hash_as_entrypoint();
                let ds1_clients = lane_targeted_clients_for_lane(
                    &network,
                    &outage_status_client,
                    &ALICE_ID,
                    ALICE_KEYPAIR.private_key(),
                    DS1_LANE_INDEX,
                );
                let ds2_clients = lane_targeted_clients_for_lane(
                    &network,
                    &outage_status_client,
                    &BOB_ID,
                    BOB_KEYPAIR.private_key(),
                    DS2_LANE_INDEX,
                );
                let ds1_submit_context =
                    format!("work unit {iteration}: DS1 autonomous transfer enqueue");
                let ds2_submit_context =
                    format!("work unit {iteration}: DS2 autonomous transfer enqueue");
                let (ds1_submit, ds2_submit) = rt.block_on(async move {
                    let ds1_task = spawn_blocking(move || {
                        submit_transaction_across_clients(
                            || ds1_clients.clone(),
                            &ds1_autonomous_tx,
                            &ds1_submit_context,
                            SUBMIT_ENQUEUE_REQUEST_TIMEOUT,
                        )
                    });
                    let ds2_task = spawn_blocking(move || {
                        submit_transaction_across_clients(
                            || ds2_clients.clone(),
                            &ds2_autonomous_tx,
                            &ds2_submit_context,
                            SUBMIT_ENQUEUE_REQUEST_TIMEOUT,
                        )
                    });
                    tokio::join!(ds1_task, ds2_task)
                });
                ds1_submit.map_err(|err| {
                    eyre!("work unit {iteration}: DS1 submit task failed: {err}")
                })??;
                ds2_submit.map_err(|err| {
                    eyre!("work unit {iteration}: DS2 submit task failed: {err}")
                })??;
                wait_for_committed_success_across_clients(
                    || {
                        lane_targeted_clients_for_lane(
                            &network,
                            &outage_status_client,
                            &ALICE_ID,
                            ALICE_KEYPAIR.private_key(),
                            DS1_LANE_INDEX,
                        )
                    },
                    ds1_entry_hash.clone(),
                    &format!("work unit {iteration}: DS1 autonomous transfer committed"),
                    SOAK_PHASE_WAIT_TIMEOUT,
                )?;
                wait_for_committed_success_across_clients(
                    || {
                        lane_targeted_clients_for_lane(
                            &network,
                            &outage_status_client,
                            &BOB_ID,
                            BOB_KEYPAIR.private_key(),
                            DS2_LANE_INDEX,
                        )
                    },
                    ds2_entry_hash.clone(),
                    &format!("work unit {iteration}: DS2 autonomous transfer committed"),
                    SOAK_PHASE_WAIT_TIMEOUT,
                )?;
                let ds1_tick_submitters = [ds1_submitter];
                let ds2_tick_submitters = [ds2_submitter];
                let _autonomous_progress = wait_for_independent_lane_application_progress_after(
                    &network,
                    &ds1_tick_submitters,
                    &ds2_tick_submitters,
                    &autonomous_pre_application.0,
                    &autonomous_pre_application.1,
                    &format!("work unit {iteration}: independent autonomous lane application"),
                )?;
                Ok([paired_swap_entry_hash, ds1_entry_hash, ds2_entry_hash])
            })();

            let restart_result =
                restart_rotated_validators(&rt, &offline_peers, &config_layers, &outage_context);
            let entrypoint_hashes = match (outage_result, restart_result) {
                (Ok(entrypoint_hashes), Ok(())) => entrypoint_hashes,
                (Err(work_err), Ok(())) => return Err(work_err),
                (Ok(_), Err(restart_err)) => return Err(restart_err),
                (Err(work_err), Err(restart_err)) => {
                    return Err(eyre!(
                        "{outage_context}: workload failed: {work_err}; restart also failed: {restart_err}"
                    ));
                }
            };
            for entrypoint_hash in &entrypoint_hashes {
                ensure!(
                    observed_work_entrypoints.insert(entrypoint_hash.clone()),
                    "work unit {iteration}: transaction entrypoint identity was reused: {entrypoint_hash}"
                );
            }

            let next_durable_native_evidence = wait_for_durable_native_participant_evidence_after(
                &network,
                LaneId::new(DS2_LANE_INDEX),
                DataSpaceId::new(DS2_ID_U64),
                durable_native_evidence.lane_incarnation,
                Some(&durable_native_evidence),
                None,
                &format!("work unit {iteration}: post-restart durable DS2 participant evidence"),
            )?;
            durable_native_evidence = next_durable_native_evidence;
            wait_for_entrypoints_committed_once_on_all_peers(
                &network,
                &entrypoint_hashes,
                &format!("work unit {iteration}: exact-once canonical history"),
            )?;

            completed_work_units = completed_work_units.saturating_add(1);
            let expected_balances = expected_post_swap_balances(completed_work_units)?;
            let alice_ds1_balance_clients = lane_targeted_clients_for_lane(
                &network,
                &alice,
                &ALICE_ID,
                ALICE_KEYPAIR.private_key(),
                DS1_LANE_INDEX,
            );
            let bob_ds1_balance_clients = lane_targeted_clients_for_lane(
                &network,
                &alice,
                &BOB_ID,
                BOB_KEYPAIR.private_key(),
                DS1_LANE_INDEX,
            );
            let alice_ds2_balance_clients = lane_targeted_clients_for_lane(
                &network,
                &alice,
                &ALICE_ID,
                ALICE_KEYPAIR.private_key(),
                DS2_LANE_INDEX,
            );
            let bob_ds2_balance_clients = lane_targeted_clients_for_lane(
                &network,
                &alice,
                &BOB_ID,
                BOB_KEYPAIR.private_key(),
                DS2_LANE_INDEX,
            );
            let post_restart_expectations = [
                BalanceExpectationAcrossClients {
                    clients: &alice_ds1_balance_clients,
                    asset_id: &alice_ds1_asset,
                    expected: expected_balances[0].clone(),
                },
                BalanceExpectationAcrossClients {
                    clients: &bob_ds1_balance_clients,
                    asset_id: &bob_ds1_asset,
                    expected: expected_balances[1].clone(),
                },
                BalanceExpectationAcrossClients {
                    clients: &alice_ds2_balance_clients,
                    asset_id: &alice_ds2_asset,
                    expected: expected_balances[2].clone(),
                },
                BalanceExpectationAcrossClients {
                    clients: &bob_ds2_balance_clients,
                    asset_id: &bob_ds2_asset,
                    expected: expected_balances[3].clone(),
                },
            ];
            wait_for_expected_balances_across_clients_with_tick_submitters_timeout(
                &[
                    neutral_tick_submitter_a.clone(),
                    neutral_tick_submitter_b.clone(),
                ],
                &post_restart_expectations,
                &format!("work unit {iteration}: post-restart autonomous balances"),
                LANE_PROGRESS_WAIT_TIMEOUT,
            )?;
            work_unit_durations.push(iteration_started.elapsed());
            eprintln!(
                "[fault-work] seed={} iteration={} offline={offline_indices:?} passed elapsed={:.3}s",
                seed.value,
                completed_work_units,
                iteration_started.elapsed().as_secs_f64()
            );
        }
    }
    ensure!(
        completed_work_units > 0,
        "corridor must complete at least one strict fault-work unit"
    );
    ensure!(
        observed_work_entrypoints.len() == completed_work_units.saturating_mul(3),
        "strict fault-work entrypoint accounting mismatch: work_units={}, unique_entrypoints={}",
        completed_work_units,
        observed_work_entrypoints.len()
    );
    if let CorridorRunMode::FaultSoak { duration } = run_mode {
        ensure!(
            fault_phase_started.elapsed() >= duration,
            "fault soak ended before its configured duration: elapsed {:?}, required {duration:?}",
            fault_phase_started.elapsed()
        );
    }
    if let Some((min, avg, max)) = duration_min_avg_max_secs(&work_unit_durations) {
        eprintln!(
            "[fault-work] exact accounting: scheduled={} passed={} failed=0 retries=0 min={min:.3}s avg={avg:.3}s max={max:.3}s",
            completed_work_units, completed_work_units
        );
    }
    {
        let _phase = phase_timings.phase("execute failing swap + rollback verification");
        let mut failure_text = None;
        let mut last_attempt_entry_hash: Option<HashOf<TransactionEntrypoint>> = None;
        for attempt in 0..ROLLBACK_CAPPED_ATTEMPTS {
            let settlement_id = if attempt == 0 {
                "ds1ds2swapfail".to_owned()
            } else {
                format!("ds1ds2swapfail_retry{attempt}")
            };
            let failing_swap = DvpIsi::new(
                settlement_id.parse().expect("settlement id"),
                SettlementLeg::new(
                    ds1_asset_def.clone(),
                    Quantity::from(10_u32),
                    ALICE_ID.clone(),
                    BOB_ID.clone(),
                ),
                SettlementLeg::new(
                    ds2_asset_def.clone(),
                    Quantity::from(10_000_u32),
                    BOB_ID.clone(),
                    ALICE_ID.clone(),
                ),
                SettlementPlan::new(
                    SettlementExecutionOrder::DeliveryThenPayment,
                    SettlementAtomicity::AllOrNothing,
                ),
            );
            grant_exact_dvp_consents_across_clients(
                || {
                    let (_, bob_submitter) = current_nexus_clients();
                    vec![bob_submitter]
                },
                &[(&failing_swap, bob_ds2_asset.clone())],
                &ALICE_ID,
                &format!("rollback attempt {attempt}: counterparty consent"),
            )?;
            let (submitter, _) = current_nexus_clients();
            let failing_swap_tx = submitter.build_transaction(
                [InstructionBox::from(failing_swap)],
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            );
            let entry_hash = failing_swap_tx.hash_as_entrypoint();
            last_attempt_entry_hash = Some(entry_hash.clone());
            if let Err(err) = submit_transaction_across_clients(
                || {
                    lane_targeted_clients_for_lane(
                        &network,
                        &alice,
                        &ALICE_ID,
                        ALICE_KEYPAIR.private_key(),
                        NEXUS_LANE_INDEX,
                    )
                },
                &failing_swap_tx,
                "rollback failing swap enqueue on Nexus authoritative observers",
                SUBMIT_ENQUEUE_REQUEST_TIMEOUT,
            ) {
                let error_text = err.to_string();
                if is_inconclusive_blocking_submit_error(&error_text)
                    && attempt + 1 < ROLLBACK_CAPPED_ATTEMPTS
                {
                    eprintln!(
                        "[rollback] inconclusive enqueue on attempt {}; retrying with fresh leader target",
                        attempt + 1
                    );
                    continue;
                }
                failure_text = Some(error_text);
                break;
            }

            match wait_for_committed_rejection_reason_across_clients(
                || {
                    lane_targeted_clients_for_lane(
                        &network,
                        &alice,
                        &ALICE_ID,
                        ALICE_KEYPAIR.private_key(),
                        NEXUS_LANE_INDEX,
                    )
                },
                entry_hash.clone(),
                "rollback rejection reason from Nexus authoritative history",
                ROLLBACK_HISTORY_RETRY_TIMEOUT,
            ) {
                Ok(committed_reason) => {
                    failure_text = Some(committed_reason);
                    break;
                }
                Err(err) => {
                    let error_text = err.to_string();
                    if error_text.contains("settlement leg requires 10000")
                        || error_text.contains("requires 10000")
                    {
                        failure_text = Some(error_text);
                        break;
                    }
                    if is_inconclusive_committed_outcome_error(&error_text)
                        && attempt + 1 < ROLLBACK_CAPPED_ATTEMPTS
                    {
                        eprintln!(
                            "[rollback] inconclusive rejection lookup on attempt {}; retrying with fresh leader target",
                            attempt + 1
                        );
                        continue;
                    }
                    failure_text = Some(error_text);
                    break;
                }
            }
        }
        let mut failure_text = failure_text
            .ok_or_else(|| eyre!("rollback rejection attempt did not produce an error"))?;
        if is_inconclusive_blocking_submit_error(&failure_text)
            || is_inconclusive_committed_outcome_error(&failure_text)
        {
            let entry_hash = last_attempt_entry_hash
                .ok_or_else(|| eyre!("missing transaction entry hash for rollback fallback"))?;
            eprintln!("[rollback] falling back to committed history lookup for rejection reason");
            match wait_for_committed_rejection_reason_across_clients(
                || {
                    lane_targeted_clients_for_lane(
                        &network,
                        &alice,
                        &ALICE_ID,
                        ALICE_KEYPAIR.private_key(),
                        NEXUS_LANE_INDEX,
                    )
                },
                entry_hash,
                "rollback rejection reason from Nexus authoritative history fallback",
                ROLLBACK_HISTORY_FALLBACK_TIMEOUT,
            ) {
                Ok(reason) => {
                    failure_text = reason;
                }
                Err(err) => {
                    let error_text = err.to_string();
                    if !is_inconclusive_committed_outcome_error(&error_text) {
                        return Err(err);
                    }
                    eprintln!(
                        "[rollback] committed history lookup inconclusive; balance rollback check remains the safety gate"
                    );
                    failure_text = error_text;
                }
            }
        }
        assert!(
            is_expected_rollback_failure_text(&failure_text),
            "unexpected failure message: {failure_text}"
        );
        if !failure_text.contains("settlement leg requires 10000")
            && !failure_text.contains("requires 10000")
        {
            eprintln!(
                "[rollback] rejection reason remained inconclusive; balance rollback check remains the safety gate"
            );
        }
        let (rollback_alice_on_nexus, rollback_bob_on_nexus) = current_nexus_clients();
        let rollback_expected_balances = expected_post_swap_balances(completed_work_units)?;
        let rollback_baseline = [
            BalanceExpectation {
                client: &rollback_alice_on_nexus,
                asset_id: &alice_ds1_asset,
                expected: rollback_expected_balances[0].clone(),
            },
            BalanceExpectation {
                client: &rollback_bob_on_nexus,
                asset_id: &bob_ds1_asset,
                expected: rollback_expected_balances[1].clone(),
            },
            BalanceExpectation {
                client: &rollback_alice_on_nexus,
                asset_id: &alice_ds2_asset,
                expected: rollback_expected_balances[2].clone(),
            },
            BalanceExpectation {
                client: &rollback_bob_on_nexus,
                asset_id: &bob_ds2_asset,
                expected: rollback_expected_balances[3].clone(),
            },
        ];
        wait_for_expected_balances(&rollback_baseline, "rollback balances after failing swap")?;
    }

    ensure!(
        swap_nonconverged_fallbacks <= SWAP_NONCONVERGED_FALLBACK_MAX,
        "swap fallback non-convergence exceeded threshold: observed {}, max {}",
        swap_nonconverged_fallbacks,
        SWAP_NONCONVERGED_FALLBACK_MAX
    );

    eprintln!(
        "[health] strict_work_passed={} strict_work_failed=0 strict_work_retries=0 setup_retries={} swap_fallbacks={} swap_nonconverged_fallbacks={}",
        completed_work_units,
        setup_register_mint_retries_used,
        swap_outcome_fallbacks,
        swap_nonconverged_fallbacks
    );

    phase_timings.emit_summary();

    Ok(())
}

#[test]
fn cross_dataspace_localnet_genesis_preexecution_smoke() {
    // Build-only smoke test keeps genesis pre-execution coverage cheap and deterministic.
    let _guard = sandbox::serial_guard();
    let _network = localnet_builder(DEFAULT_CORRIDOR_SEED).build();
}

#[cfg(test)]
mod tests {
    use super::{
        ALICE_ID, AUTOSCALE_DRAIN_COMMITMENT_LOG_MARKER, AUTOSCALE_DRAIN_INTENT_LOG_MARKER,
        AUTOSCALE_LANE_INDEX, AUTOSCALE_SCALE_IN_LOG_MARKER, AUTOSCALE_SCALE_OUT_LOG_MARKER,
        AccountId, Algorithm, AutoscaleDrainCommitmentLog, AutoscaleDrainIntentLog,
        CORRIDOR_SEED_COUNT, CommittedTxOutcome, DS1_ID_U64, DS1_LANE_INDEX, DS1_MANIFEST_HASH,
        DS2_ID_U64, DS2_LANE_INDEX, DS2_MANIFEST_HASH, ExpectedLaneValidatorBinding,
        FAULT_SOAK_DURATION_SECS, KeyPair, LaneDomainProgress, LanePayloadOwnershipProgress,
        NEXUS_ALIAS, NEXUS_ID_U64, NEXUS_LANE_INDEX, OBSERVER_QUERY_TIMEOUT_CAP, PeerId,
        RoutedJsonGetResponse, TOTAL_PEERS, VALIDATORS_PER_LANE, applied_lane_domain_progress,
        bounded_observer_request_timeout, committed_lane_block_has_expected_quorum,
        committed_tx_outcome_quorum, copy_kura_tree_with_limits, cross_dataspace_gas_account_id,
        durable_native_participant_evidence_is_after_baseline, durable_native_participant_row,
        duration_min_avg_max_secs, expect_local_or_proxy_fanout_headers,
        expected_lane_binding_for_peer, expected_post_swap_balances,
        is_expected_rollback_failure_text, is_inconclusive_blocking_submit_error,
        is_inconclusive_committed_outcome_error, lane_domain_progress_is_after_baseline,
        lane_validator_snapshot, latest_lane_domain_application_progress,
        latest_lane_domain_progress, latest_lane_payload_ownership_progress,
        multilane_da_proof_policy_bundle, nexus_fee_asset_definition_id,
        npos_multilane_genesis_post_topology_transactions, parse_autoscale_lifecycle_log,
        parse_corridor_seed, parse_fault_soak_duration, parse_required_seed_flag,
        peer_indices_for_committed_lane_evidence, quorum_lane_domain_progress,
        quorum_lane_payload_ownership_progress, recreated_autoscale_lane_diagnostics_ready,
        render_error_with_debug, render_rejection_reason, rotating_validator_indices,
        routed_header_string, should_submit_tick, stake_asset_definition_id,
        stake_asset_id_literal, total_balance_observer_request_slots,
        validate_autoscale_merge_qc_height_context_binding, validator_authority_account_for_peer,
        validator_authority_seed,
    };
    use iroha::crypto::{Hash, HashOf};
    use iroha::data_model::{
        ChainId,
        block::consensus::{
            COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD,
            COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION, SumeragiCommittedLaneBlock,
            SumeragiDataspaceCommitment, SumeragiDiagnosticsStatus, SumeragiLanePayloadOwnership,
            SumeragiNativeAmxParticipantApplication, SumeragiNativeAmxParticipantApplicationState,
        },
        block::consensus_v2::{
            ConsensusMode, DataAvailabilityLayout, DualQuorum, HeightContext, PROTOCOL_VERSION,
            PayloadEncoding, ValidatorPower,
        },
        da::commitment::{DaProofPolicyBundle, DaProofScheme},
        nexus::{DataSpaceId, LaneId},
        transaction::error::{TransactionLimitError, TransactionRejectionReason},
    };
    use iroha_core::sumeragi::network_topology::commit_quorum_from_len;
    use norito::json::Value as JsonValue;
    use reqwest::header::{HeaderMap, HeaderValue};
    use std::{
        fmt::{Debug, Display, Formatter, Result as FmtResult},
        fs, panic,
        time::{Duration, Instant},
    };

    #[test]
    fn g12p_autoscale_log_parser_requires_exact_producer_fields() {
        let log = format!(
            "INFO height=10 lane={lane} {scale_out}\n\
             INFO height=20 lane={lane} close_global_height=20 initial_merged_lane_height=7 {intent}\n\
             INFO height=21 lane={lane} carrier_height=21 final_lane_block_height=9 {commitment}\n\
             INFO height=22 lane={lane} {scale_in}\n\
             INFO close_global_height=99 lane={lane} initial_merged_lane_height=7 {intent}\n\
             INFO height=30 height=31 lane={lane} carrier_height=30 final_lane_block_height=9 {commitment}\n\
             INFO height=40 lane=2 {scale_out}",
            lane = AUTOSCALE_LANE_INDEX,
            scale_out = AUTOSCALE_SCALE_OUT_LOG_MARKER,
            intent = AUTOSCALE_DRAIN_INTENT_LOG_MARKER,
            commitment = AUTOSCALE_DRAIN_COMMITMENT_LOG_MARKER,
            scale_in = AUTOSCALE_SCALE_IN_LOG_MARKER,
        );

        let evidence = parse_autoscale_lifecycle_log(&log);
        assert_eq!(
            evidence.scale_out_heights,
            std::collections::BTreeSet::from([10])
        );
        assert_eq!(
            evidence.drain_intents,
            std::collections::BTreeSet::from([AutoscaleDrainIntentLog {
                height: 20,
                close_global_height: 20,
                initial_merged_lane_height: 7,
            }])
        );
        assert_eq!(
            evidence.drain_commitments,
            std::collections::BTreeSet::from([AutoscaleDrainCommitmentLog {
                height: 21,
                carrier_height: 21,
                final_lane_block_height: 9,
            }])
        );
        assert_eq!(
            evidence.scale_in_heights,
            std::collections::BTreeSet::from([22])
        );
    }

    fn empty_sumeragi_diagnostics() -> SumeragiDiagnosticsStatus {
        SumeragiDiagnosticsStatus {
            pipeline_execution: Default::default(),
            tx_queue_depth: 0,
            tx_queue_capacity: 1,
            tx_queue_retained_bytes: 0,
            tx_queue_max_retained_bytes: 1,
            tx_queue_saturated: false,
            tx_queue_saturated_by_count: false,
            tx_queue_saturated_by_bytes: false,
            tx_queue_saturated_by_age: false,
            tx_queue_oldest_queued_age_ms: 0,
            npos: None,
            lane_commitments: Vec::new(),
            dataspace_commitments: Vec::new(),
            lane_settlement_commitments: Vec::new(),
            lane_relay_envelopes: Vec::new(),
            lane_payload_ownerships: Vec::new(),
            committed_lane_blocks: Vec::new(),
            lane_block_sessions: Vec::new(),
            lane_governance_sealed_total: 0,
            lane_governance_sealed_aliases: Vec::new(),
            lane_governance: Vec::new(),
            native_amx_participant_applications: Vec::new(),
            autonomous_lane_executions: Vec::new(),
        }
    }

    fn deterministic_topology(peer_count: usize) -> Vec<PeerId> {
        (0..peer_count)
            .map(|index| {
                let mut seed = vec![0_u8; 32];
                seed[0] = 0xD1;
                seed[1..9].copy_from_slice(&u64::try_from(index).unwrap_or(u64::MAX).to_le_bytes());
                let key_pair = KeyPair::try_from_seed(seed, Algorithm::Ed25519)
                    .expect("fixture cross-dataspace topology peer key");
                PeerId::new(key_pair.public_key().clone())
            })
            .collect()
    }

    fn weighted_height_context(powers: &[u64]) -> HeightContext {
        let mut validators = deterministic_topology(powers.len());
        validators.sort();
        let roster = validators
            .into_iter()
            .zip(powers.iter().copied())
            .map(|(validator, power)| ValidatorPower { validator, power })
            .collect::<Vec<_>>();
        HeightContext {
            chain_id: ChainId::from("g12p-historical-roster-test"),
            protocol_version: PROTOCOL_VERSION,
            height: 1,
            epoch: 7,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Npos,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("valid weighted fixture roster"),
            roster,
            nexus_amx_context_hash: Hash::new(b"g12p historical roster"),
            execution_policy_hash: Hash::new(b"g12p historical roster execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::Plain,
                chunk_size_bytes: 4,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1024,
                max_chunk_count: 256,
            },
            leader_seed: [0xA5; 32],
        }
    }

    fn decode_manifest_hash_fixture(raw: &str) -> [u8; 32] {
        assert_eq!(raw.len(), 64);
        let mut hash = [0_u8; 32];
        for (idx, chunk) in raw.as_bytes().chunks_exact(2).enumerate() {
            let pair = std::str::from_utf8(chunk).expect("hex pair");
            hash[idx] = u8::from_str_radix(pair, 16).expect("manifest hash hex");
        }
        hash
    }

    #[test]
    fn dataspace_fixture_manifest_hashes_derive_config_ids() {
        let ds1_hash = decode_manifest_hash_fixture(DS1_MANIFEST_HASH);
        let ds2_hash = decode_manifest_hash_fixture(DS2_MANIFEST_HASH);

        assert_eq!(NEXUS_ALIAS, "universal");
        assert_eq!(NEXUS_ID_U64, DataSpaceId::UNIVERSAL.as_u64());
        assert_eq!(DataSpaceId::from_hash(&ds1_hash).as_u64(), DS1_ID_U64);
        assert_eq!(DataSpaceId::from_hash(&ds2_hash).as_u64(), DS2_ID_U64);
        assert_ne!(DataSpaceId::from_hash(&ds1_hash), DataSpaceId::UNIVERSAL);
        assert_ne!(DataSpaceId::from_hash(&ds2_hash), DataSpaceId::UNIVERSAL);
    }

    #[test]
    fn corridor_seed_parser_accepts_exact_ten_seed_domain() {
        for ordinal in 0..CORRIDOR_SEED_COUNT {
            let raw = format!("nexus-cross-dataspace-v1-seed-{ordinal:02}");
            let parsed = parse_corridor_seed(&raw).expect("valid corridor seed");
            assert_eq!(parsed.value, raw);
            assert_eq!(parsed.ordinal, ordinal);
        }
    }

    #[test]
    fn corridor_seed_parser_rejects_noncanonical_or_out_of_matrix_values() {
        for raw in [
            "",
            "seed-00",
            "nexus-cross-dataspace-v1-seed-0",
            "nexus-cross-dataspace-v1-seed-000",
            "nexus-cross-dataspace-v1-seed-aa",
            "nexus-cross-dataspace-v1-seed-10",
        ] {
            assert!(parse_corridor_seed(raw).is_err(), "accepted {raw:?}");
        }
    }

    #[test]
    fn required_seed_flag_is_fail_closed() {
        assert!(!parse_required_seed_flag(None).unwrap());
        assert!(!parse_required_seed_flag(Some("0")).unwrap());
        assert!(parse_required_seed_flag(Some("1")).unwrap());
        for raw in ["", "true", "01", " 1 "] {
            assert!(parse_required_seed_flag(Some(raw)).is_err());
        }
    }

    #[test]
    fn fault_soak_duration_accepts_exactly_two_hours() {
        assert_eq!(
            parse_fault_soak_duration(None).unwrap().as_secs(),
            FAULT_SOAK_DURATION_SECS
        );
        assert_eq!(
            parse_fault_soak_duration(Some("7200")).unwrap().as_secs(),
            FAULT_SOAK_DURATION_SECS
        );
        for raw in ["", "0", "7199", "7201", "two-hours"] {
            assert!(parse_fault_soak_duration(Some(raw)).is_err());
        }
    }

    #[test]
    fn outage_rotation_selects_one_validator_per_dataspace() {
        assert_eq!(rotating_validator_indices(0, 0), [0, 4, 8]);
        assert_eq!(rotating_validator_indices(1, 2), [3, 7, 11]);
        assert_eq!(rotating_validator_indices(1, 3), [0, 4, 8]);
    }

    #[test]
    fn autonomous_balance_model_is_bounded_and_monotonic() {
        let initial = expected_post_swap_balances(0).unwrap();
        let after_one = expected_post_swap_balances(1).unwrap();
        assert_ne!(initial, after_one);
        assert!(expected_post_swap_balances(1_000_001).is_err());
    }

    #[test]
    fn asset_definition_helpers_keep_stake_and_fee_domains_distinct() {
        let stake_definition_id = stake_asset_definition_id();
        let fee_definition_id = nexus_fee_asset_definition_id();

        assert_eq!(stake_asset_id_literal(), stake_definition_id.to_string());
        assert_ne!(
            stake_definition_id.to_string(),
            fee_definition_id.to_string(),
            "stake and fee helpers should not collapse cross-dataspace asset domains"
        );
    }

    #[test]
    fn committed_lane_evidence_peer_scan_is_not_lane_bound() {
        assert_eq!(
            peer_indices_for_committed_lane_evidence(VALIDATORS_PER_LANE),
            vec![0, 1, 2, 3],
            "self-certified committed lane evidence can be observed from any peer"
        );
        assert_eq!(
            peer_indices_for_committed_lane_evidence(0),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn multilane_da_policy_bundle_preserves_lane_dataspace_order() {
        let bundle = multilane_da_proof_policy_bundle();
        let expected_hash = DaProofPolicyBundle::new(bundle.policies.clone()).policy_hash;

        assert_eq!(bundle.version, DaProofPolicyBundle::VERSION_V1);
        assert_eq!(bundle.policy_hash, expected_hash);
        assert_eq!(bundle.policies.len(), 3);
        assert_eq!(bundle.policies[0].lane_id.as_u32(), NEXUS_LANE_INDEX);
        assert_eq!(bundle.policies[0].dataspace_id.as_u64(), NEXUS_ID_U64);
        assert_eq!(bundle.policies[0].alias, "lane-nexus");
        assert_eq!(bundle.policies[0].proof_scheme, DaProofScheme::MerkleSha256);
        assert_eq!(bundle.policies[1].lane_id.as_u32(), DS1_LANE_INDEX);
        assert_eq!(bundle.policies[1].dataspace_id.as_u64(), DS1_ID_U64);
        assert_eq!(bundle.policies[1].alias, "lane-ds1");
        assert_eq!(bundle.policies[2].lane_id.as_u32(), DS2_LANE_INDEX);
        assert_eq!(bundle.policies[2].dataspace_id.as_u64(), DS2_ID_U64);
        assert_eq!(bundle.policies[2].alias, "lane-ds2");
    }

    #[test]
    fn multilane_da_policy_bundle_hash_is_deterministic_and_order_sensitive() {
        let bundle = multilane_da_proof_policy_bundle();
        let repeated = multilane_da_proof_policy_bundle();
        let mut reversed_policies = bundle.policies.clone();
        reversed_policies.reverse();
        let reversed_hash = DaProofPolicyBundle::new(reversed_policies).policy_hash;

        assert_eq!(bundle, repeated);
        assert_ne!(bundle.policy_hash, reversed_hash);
    }

    #[test]
    fn genesis_post_topology_builder_covers_all_lane_buckets() {
        let topology = deterministic_topology(TOTAL_PEERS);
        let transactions = npos_multilane_genesis_post_topology_transactions(&topology);

        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].len(), 12 + TOTAL_PEERS * 5);
        assert_eq!(
            expected_lane_binding_for_peer(0, &topology[0]).peer_id,
            topology[0].to_string()
        );
        assert_eq!(
            expected_lane_binding_for_peer(VALIDATORS_PER_LANE, &topology[VALIDATORS_PER_LANE])
                .peer_id,
            topology[VALIDATORS_PER_LANE].to_string()
        );
        assert_eq!(
            expected_lane_binding_for_peer(
                VALIDATORS_PER_LANE * 2,
                &topology[VALIDATORS_PER_LANE * 2],
            )
            .peer_id,
            topology[VALIDATORS_PER_LANE * 2].to_string()
        );
    }

    #[test]
    fn genesis_post_topology_builder_is_deterministic_for_same_roster() {
        let topology = deterministic_topology(TOTAL_PEERS);
        let first = npos_multilane_genesis_post_topology_transactions(&topology);
        let second = npos_multilane_genesis_post_topology_transactions(&topology);

        assert_eq!(format!("{first:?}"), format!("{second:?}"));
    }

    #[test]
    fn genesis_post_topology_builder_rejects_wrong_peer_count() {
        let topology = deterministic_topology(TOTAL_PEERS - 1);

        let result =
            panic::catch_unwind(|| npos_multilane_genesis_post_topology_transactions(&topology));

        assert!(result.is_err());
    }

    #[test]
    fn cross_dataspace_gas_account_uses_alice_canonical_subject() {
        let gas_account = cross_dataspace_gas_account_id();

        assert_eq!(gas_account, ALICE_ID.clone());
        assert_eq!(
            gas_account.canonical_i105().expect("gas account i105"),
            ALICE_ID.canonical_i105().expect("alice i105")
        );
    }

    #[test]
    fn validator_authority_account_for_peer_is_deterministic_and_indexed() {
        let first = validator_authority_account_for_peer(2);
        let repeated = validator_authority_account_for_peer(2);
        let next = validator_authority_account_for_peer(3);
        let expected = KeyPair::try_from_seed(validator_authority_seed(2), Algorithm::Ed25519)
            .expect("fixture cross-dataspace validator authority key");

        assert_eq!(first, repeated);
        assert_ne!(first, next);
        assert_eq!(first, AccountId::new(expected.public_key().clone()));
        assert!(
            !first
                .canonical_i105()
                .expect("validator account")
                .is_empty()
        );
    }

    #[test]
    fn expected_lane_binding_for_peer_pairs_validator_and_peer_id() {
        let mut seed = vec![0_u8; 32];
        seed[0] = 0xA5;
        let peer_key_pair = KeyPair::try_from_seed(seed, Algorithm::Ed25519)
            .expect("fixture cross-dataspace binding peer key");
        let peer_id = PeerId::new(peer_key_pair.public_key().clone());

        let binding = expected_lane_binding_for_peer(4, &peer_id);

        assert_eq!(binding.peer_id, peer_id.to_string());
        assert_eq!(
            binding.validator,
            validator_authority_account_for_peer(4).to_string()
        );
    }

    #[test]
    fn lane_validator_snapshot_filters_active_bindings_and_preserves_total() {
        let body = norito::json!({
            "total": 3,
            "items": [
                {
                    "validator": "validator-a",
                    "peer_id": "peer-a",
                    "status": { "type": "Active" },
                },
                {
                    "validator": "validator-b",
                    "peer_id": "peer-b",
                    "status": { "type": "Pending" },
                },
                {
                    "validator": "validator-c",
                    "peer_id": "peer-c",
                    "status": { "type": "Active" },
                },
            ],
        });

        let (total, active) =
            lane_validator_snapshot(&body, "lane validators").expect("lane snapshot should parse");

        assert_eq!(total, 3);
        assert_eq!(active.len(), 2);
        assert!(active.contains(&ExpectedLaneValidatorBinding {
            validator: "validator-a".to_owned(),
            peer_id: "peer-a".to_owned(),
        }));
        assert!(active.contains(&ExpectedLaneValidatorBinding {
            validator: "validator-c".to_owned(),
            peer_id: "peer-c".to_owned(),
        }));
    }

    #[test]
    fn lane_validator_snapshot_rejects_malformed_payloads() {
        for (body, expected) in [
            (JsonValue::Null, "lane validator response is not an object"),
            (
                norito::json!({ "items": [] }),
                "lane validator response is missing total",
            ),
            (
                norito::json!({ "total": 1 }),
                "lane validator response is missing items",
            ),
            (
                norito::json!({
                    "total": 1,
                    "items": [{ "status": { "type": "Active" }, "peer_id": "peer-a" }],
                }),
                "validator entry missing validator literal",
            ),
            (
                norito::json!({
                    "total": 1,
                    "items": [{ "validator": "validator-a", "status": { "type": "Active" } }],
                }),
                "validator entry missing peer_id literal",
            ),
            (
                norito::json!({
                    "total": 1,
                    "items": [{ "validator": "validator-a", "peer_id": "peer-a" }],
                }),
                "validator entry missing status.type",
            ),
            (
                norito::json!({
                    "total": 1,
                    "items": ["not-an-entry"],
                }),
                "validator entry is not an object",
            ),
        ] {
            let err = lane_validator_snapshot(&body, "lane validators")
                .expect_err("malformed lane snapshot should fail");

            assert!(
                err.to_string().contains(expected),
                "expected `{expected}` in `{err}`"
            );
        }
    }

    fn test_hash(tag: u8) -> Hash {
        Hash::new([tag; 4])
    }

    fn test_durable_native_participant_application(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        participant_height: u64,
        application_block_height: u64,
        identity_tag: u8,
    ) -> SumeragiNativeAmxParticipantApplication {
        let predecessor_height = participant_height.saturating_sub(1);
        SumeragiNativeAmxParticipantApplication {
            lane_id,
            dataspace_id,
            lane_incarnation,
            participant_height,
            participant_view: 0,
            predecessor_height,
            predecessor_descriptor_hash: (predecessor_height != 0)
                .then(|| test_hash(identity_tag.wrapping_add(1))),
            descriptor_hash: test_hash(identity_tag.wrapping_add(2)),
            proposal_hash: test_hash(identity_tag.wrapping_add(3)),
            settlement_hash: HashOf::from_untyped_unchecked(test_hash(
                identity_tag.wrapping_add(4),
            )),
            source_count: 1,
            application_block_height: Some(application_block_height),
            application_block_hash: Some(HashOf::from_untyped_unchecked(test_hash(
                identity_tag.wrapping_add(5),
            ))),
            state: SumeragiNativeAmxParticipantApplicationState::DurablyApplied,
        }
    }

    #[test]
    fn native_participant_progress_accepts_no_prior_row_after_carrier_floor() {
        let lane_id = LaneId::new(DS2_LANE_INDEX);
        let dataspace_id = DataSpaceId::new(DS2_ID_U64);
        let incarnation = test_hash(0x31);
        let row = test_durable_native_participant_application(
            lane_id,
            dataspace_id,
            incarnation,
            1,
            11,
            0x40,
        );

        assert!(
            durable_native_participant_evidence_is_after_baseline(
                &row,
                lane_id,
                dataspace_id,
                incarnation,
                None,
                Some(10),
            )
            .expect("first durable row after the pre-submit carrier floor")
        );
        assert!(
            !durable_native_participant_evidence_is_after_baseline(
                &row,
                lane_id,
                dataspace_id,
                incarnation,
                None,
                Some(11),
            )
            .expect("a pre-existing carrier at the floor is not new evidence")
        );
    }

    #[test]
    fn native_participant_progress_requires_strict_same_incarnation_advance() {
        let lane_id = LaneId::new(DS2_LANE_INDEX);
        let dataspace_id = DataSpaceId::new(DS2_ID_U64);
        let incarnation = test_hash(0x32);
        let baseline = test_durable_native_participant_application(
            lane_id,
            dataspace_id,
            incarnation,
            4,
            20,
            0x50,
        );
        let advanced = test_durable_native_participant_application(
            lane_id,
            dataspace_id,
            incarnation,
            5,
            21,
            0x60,
        );

        assert!(
            durable_native_participant_evidence_is_after_baseline(
                &advanced,
                lane_id,
                dataspace_id,
                incarnation,
                Some(&baseline),
                None,
            )
            .expect("strict same-incarnation participant advance")
        );
        assert!(
            !durable_native_participant_evidence_is_after_baseline(
                &baseline,
                lane_id,
                dataspace_id,
                incarnation,
                Some(&baseline),
                None,
            )
            .expect("an identical replay is not progress")
        );
    }

    #[test]
    fn native_participant_progress_rejects_stale_incarnation() {
        let lane_id = LaneId::new(DS2_LANE_INDEX);
        let dataspace_id = DataSpaceId::new(DS2_ID_U64);
        let active_incarnation = test_hash(0x33);
        let stale = test_durable_native_participant_application(
            lane_id,
            dataspace_id,
            test_hash(0x34),
            5,
            21,
            0x70,
        );

        let error = durable_native_participant_evidence_is_after_baseline(
            &stale,
            lane_id,
            dataspace_id,
            active_incarnation,
            None,
            None,
        )
        .expect_err("a stale incarnation must fail closed");
        assert!(error.to_string().contains("stale lane incarnation"));
    }

    #[test]
    fn native_participant_progress_rejects_same_height_conflict() {
        let lane_id = LaneId::new(DS2_LANE_INDEX);
        let dataspace_id = DataSpaceId::new(DS2_ID_U64);
        let incarnation = test_hash(0x35);
        let baseline = test_durable_native_participant_application(
            lane_id,
            dataspace_id,
            incarnation,
            5,
            21,
            0x80,
        );
        let conflicting = test_durable_native_participant_application(
            lane_id,
            dataspace_id,
            incarnation,
            5,
            22,
            0x90,
        );

        let error = durable_native_participant_evidence_is_after_baseline(
            &conflicting,
            lane_id,
            dataspace_id,
            incarnation,
            Some(&baseline),
            None,
        )
        .expect_err("same-height identity drift must fail closed");
        assert!(
            error
                .to_string()
                .contains("conflicts with the baseline at the same height")
        );

        let mut diagnostics = empty_sumeragi_diagnostics();
        diagnostics.native_amx_participant_applications =
            vec![SumeragiNativeAmxParticipantApplication {
                state: SumeragiNativeAmxParticipantApplicationState::Conflict,
                ..conflicting
            }];
        assert!(
            durable_native_participant_row(&diagnostics, lane_id, dataspace_id, "conflict fixture")
                .expect_err("public conflict state must fail the corridor gate")
                .to_string()
                .contains("conflicting participant evidence")
        );
    }

    fn test_lane_domain_progress(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_block_height: u64,
        lane_block_view: u64,
    ) -> LaneDomainProgress {
        LaneDomainProgress {
            lane_id,
            dataspace_id,
            lane_incarnation: test_hash(0x0E),
            lane_block_height,
            lane_block_view,
            descriptor_hash: test_hash(0xA0),
            proposal_hash: test_hash(0xA1),
            subject_hash: test_hash(0xA2),
            payload_ownership_hash: test_hash(0xA3),
            rbc_instance_hash: test_hash(0xA4),
            qc_mode_tag: "test-bls".to_owned(),
            dataspace_commitment_height: Some(6 + lane_block_height),
            prepare_qc_signer_count: 3,
            commit_qc_signer_count: 3,
            execution_status: COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK.to_owned(),
            executable_payload_available: true,
        }
    }

    fn test_lane_payload_ownership_progress(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_block_height: u64,
        lane_block_view: u64,
        proposal_height: u64,
    ) -> LanePayloadOwnershipProgress {
        LanePayloadOwnershipProgress {
            lane_id,
            dataspace_id,
            lane_incarnation: test_hash(0x0F),
            lane_block_height,
            lane_block_view,
            proposal_height,
            proposal_view: 0,
            subject_hash: test_hash(0xB0),
            lane_block_descriptor_hash: test_hash(0xB1),
            payload_ownership_hash: test_hash(0xB2),
            rbc_instance_hash: test_hash(0xB3),
            accepted_transaction_count: 1,
            validator_count: VALIDATORS_PER_LANE as u32,
            min_quorum: 3,
        }
    }

    fn sample_committed_lane_block(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_block_height: u64,
        min_quorum: u32,
        prepare_qc_signer_count: u32,
        commit_qc_signer_count: u32,
        execution_status: &str,
    ) -> SumeragiCommittedLaneBlock {
        SumeragiCommittedLaneBlock {
            lane_id,
            dataspace_id,
            lane_incarnation: test_hash(0x0E),
            lane_block_height,
            lane_block_view: 0,
            descriptor_hash: test_hash(0x01),
            proposal_hash: test_hash(0x02),
            execution_status: execution_status.to_owned(),
            executable_payload_available: true,
            subject_hash: test_hash(0x03),
            payload_ownership_hash: test_hash(0x04),
            rbc_instance_hash: test_hash(0x05),
            qc_mode_tag: "test-bls".to_owned(),
            validator_count: VALIDATORS_PER_LANE as u32,
            min_quorum,
            prepare_qc_signer_count,
            commit_qc_signer_count,
        }
    }

    #[test]
    fn recreated_lane_readiness_tolerates_pruned_history_but_fails_closed_on_latest() {
        let lane_id = LaneId::new(AUTOSCALE_LANE_INDEX);
        let dataspace_id = DataSpaceId::new(NEXUS_ID_U64);
        let incarnation = test_hash(0xA8);
        let quorum = u32::try_from(commit_quorum_from_len(VALIDATORS_PER_LANE))
            .expect("fixture quorum fits u32");
        let mut pruned_older = sample_committed_lane_block(
            lane_id,
            dataspace_id,
            1,
            quorum,
            quorum,
            quorum,
            COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD,
        );
        pruned_older.lane_incarnation = incarnation;
        pruned_older.executable_payload_available = false;
        let mut ready_latest = sample_committed_lane_block(
            lane_id,
            dataspace_id,
            2,
            quorum,
            quorum,
            quorum,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
        );
        ready_latest.lane_incarnation = incarnation;
        ready_latest.proposal_hash = test_hash(0xA9);
        let ready = SumeragiDiagnosticsStatus {
            committed_lane_blocks: vec![pruned_older.clone(), ready_latest.clone()],
            ..empty_sumeragi_diagnostics()
        };
        assert!(
            recreated_autoscale_lane_diagnostics_ready(&ready, lane_id, dataspace_id, incarnation,),
            "legitimately pruned older certified payloads must not block recreated-lane readiness"
        );

        let mut unavailable_latest = ready_latest.clone();
        unavailable_latest.execution_status =
            COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD.to_owned();
        unavailable_latest.executable_payload_available = false;
        assert!(
            !recreated_autoscale_lane_diagnostics_ready(
                &SumeragiDiagnosticsStatus {
                    committed_lane_blocks: vec![pruned_older.clone(), unavailable_latest],
                    ..empty_sumeragi_diagnostics()
                },
                lane_id,
                dataspace_id,
                incarnation,
            ),
            "the latest certified row must remain executable"
        );

        let mut rejected_latest = ready_latest.clone();
        rejected_latest.execution_status =
            COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION.to_owned();
        assert!(
            !recreated_autoscale_lane_diagnostics_ready(
                &SumeragiDiagnosticsStatus {
                    committed_lane_blocks: vec![pruned_older.clone(), rejected_latest],
                    ..empty_sumeragi_diagnostics()
                },
                lane_id,
                dataspace_id,
                incarnation,
            ),
            "an executable but progress-blocking latest state must fail readiness"
        );

        let mut stale_older = pruned_older.clone();
        stale_older.lane_incarnation = test_hash(0xAA);
        assert!(
            !recreated_autoscale_lane_diagnostics_ready(
                &SumeragiDiagnosticsStatus {
                    committed_lane_blocks: vec![stale_older, ready_latest.clone()],
                    ..empty_sumeragi_diagnostics()
                },
                lane_id,
                dataspace_id,
                incarnation,
            ),
            "retained rows from a stale incarnation must fail readiness"
        );

        let mut malformed_older = pruned_older.clone();
        malformed_older.commit_qc_signer_count = quorum - 1;
        assert!(
            !recreated_autoscale_lane_diagnostics_ready(
                &SumeragiDiagnosticsStatus {
                    committed_lane_blocks: vec![malformed_older, ready_latest.clone()],
                    ..empty_sumeragi_diagnostics()
                },
                lane_id,
                dataspace_id,
                incarnation,
            ),
            "malformed retained QC rows must fail readiness"
        );

        let mut conflicting_latest = ready_latest.clone();
        conflicting_latest.proposal_hash = test_hash(0xAB);
        assert!(
            !recreated_autoscale_lane_diagnostics_ready(
                &SumeragiDiagnosticsStatus {
                    committed_lane_blocks: vec![pruned_older, ready_latest, conflicting_latest,],
                    ..empty_sumeragi_diagnostics()
                },
                lane_id,
                dataspace_id,
                incarnation,
            ),
            "conflicting identities at the latest certified slot must fail readiness"
        );
    }

    #[test]
    fn merge_qc_binding_uses_exact_historical_weighted_height_context() {
        let historical = weighted_height_context(&[8, 1, 1, 1]);
        historical.validate().expect("valid historical context");
        let historical_validators = historical
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        validate_autoscale_merge_qc_height_context_binding(
            &historical.chain_id,
            &historical,
            historical.height,
            historical.epoch,
            &historical_validators,
            &[0, 1, 2],
        )
        .expect("exact historical powered quorum should pass");

        let count_only_error = validate_autoscale_merge_qc_height_context_binding(
            &historical.chain_id,
            &historical,
            historical.height,
            historical.epoch,
            &historical_validators,
            &[1, 2, 3],
        )
        .expect_err("three low-power signers must not satisfy weighted quorum");
        assert!(
            count_only_error.to_string().contains("count-and-power"),
            "unexpected weighted-quorum rejection: {count_only_error}"
        );

        let mut current_rotated = weighted_height_context(&[8, 1, 1, 1]);
        current_rotated.height = 2;
        current_rotated.epoch = historical.epoch + 1;
        let replacement = KeyPair::try_from_seed(vec![0xF4; 32], Algorithm::Ed25519)
            .expect("derive rotated validator");
        current_rotated.roster[0].validator = PeerId::new(replacement.public_key().clone());
        current_rotated.roster.sort();
        current_rotated.quorum =
            DualQuorum::from_roster(&current_rotated.roster).expect("rotated quorum");
        assert!(
            validate_autoscale_merge_qc_height_context_binding(
                &historical.chain_id,
                &current_rotated,
                historical.height,
                historical.epoch,
                &historical_validators,
                &[0, 1, 2],
            )
            .is_err(),
            "a current rotated roster must not stand in for the historical carrier context"
        );

        let mut reordered = historical_validators.clone();
        reordered.swap(0, 1);
        assert!(
            validate_autoscale_merge_qc_height_context_binding(
                &historical.chain_id,
                &historical,
                historical.height,
                historical.epoch,
                &reordered,
                &[0, 1, 2],
            )
            .is_err(),
            "merge-QC validator order must match the frozen carrier roster exactly"
        );
    }

    #[test]
    fn bounded_kura_copy_preflights_limits_before_creating_destination() {
        let temp = tempfile::tempdir().expect("temporary Kura copy root");
        let source = temp.path().join("source");
        let destination = temp.path().join("destination");
        fs::create_dir(&source).expect("create source");
        let oversized = fs::File::create(source.join("oversized")).expect("create sparse file");
        oversized.set_len(9).expect("size sparse file");

        let error = copy_kura_tree_with_limits(&source, &destination, 1, 8)
            .expect_err("byte cap must reject the preflighted tree");
        assert!(
            error.to_string().contains("exceeded 8 bytes"),
            "unexpected byte-cap rejection: {error}"
        );
        assert!(
            fs::symlink_metadata(&destination)
                .is_err_and(|err| err.kind() == std::io::ErrorKind::NotFound),
            "an oversized tree must be rejected before materializing a partial destination"
        );
    }

    #[test]
    fn bounded_kura_copy_enforces_aggregate_caps_and_exact_copy() {
        let temp = tempfile::tempdir().expect("temporary Kura copy root");
        let source = temp.path().join("source");
        let destination = temp.path().join("destination");
        let capped_destination = temp.path().join("capped-destination");
        fs::create_dir(&source).expect("create source");
        fs::create_dir(source.join("nested")).expect("create nested source");
        fs::write(source.join("root"), b"ab").expect("write root fixture");
        fs::write(source.join("nested").join("leaf"), b"cd").expect("write leaf fixture");
        let lock = fs::File::create(source.join(".kura.lock")).expect("create source lock");
        lock.set_len(1_024).expect("size skipped source lock");

        copy_kura_tree_with_limits(&source, &destination, 3, 4)
            .expect("exact entry and byte bounds should copy");
        assert_eq!(
            fs::read(destination.join("root")).expect("read copied root"),
            b"ab"
        );
        assert_eq!(
            fs::read(destination.join("nested").join("leaf")).expect("read copied leaf"),
            b"cd"
        );
        assert!(
            !destination.join(".kura.lock").exists(),
            "the live Kura lock must not be cloned"
        );

        let error = copy_kura_tree_with_limits(&source, &capped_destination, 2, 4)
            .expect_err("aggregate entry cap must reject the tree");
        assert!(
            error.to_string().contains("exceeded 2 entries"),
            "unexpected entry-cap rejection: {error}"
        );
        assert!(
            !capped_destination.exists(),
            "entry-cap rejection must occur before destination creation"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bounded_kura_copy_rejects_source_and_destination_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temporary Kura copy root");
        let source = temp.path().join("source");
        let destination = temp.path().join("destination");
        let victim = temp.path().join("victim");
        fs::create_dir(&source).expect("create source");
        fs::create_dir(&victim).expect("create victim");
        symlink(&victim, source.join("linked-victim")).expect("create source symlink");

        let error = copy_kura_tree_with_limits(&source, &destination, 4, 64)
            .expect_err("source symlink must fail preflight");
        assert!(
            error.to_string().contains("encountered symlink"),
            "unexpected source-symlink rejection: {error}"
        );
        assert!(
            !destination.exists(),
            "source-symlink rejection must not create a destination"
        );

        fs::remove_file(source.join("linked-victim")).expect("remove source symlink");
        fs::write(source.join("regular"), b"ok").expect("write regular source");
        symlink(temp.path().join("missing"), &destination).expect("create broken destination link");
        let error = copy_kura_tree_with_limits(&source, &destination, 4, 64)
            .expect_err("existing destination symlink must be rejected");
        assert!(
            error.to_string().contains("destination already exists"),
            "unexpected destination-symlink rejection: {error}"
        );
        assert!(
            fs::symlink_metadata(&destination)
                .expect("destination symlink preserved")
                .file_type()
                .is_symlink(),
            "destination symlink must not be replaced"
        );
    }

    fn sample_lane_payload_ownership(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_block_height: u64,
        min_quorum: u32,
        accepted_count: usize,
    ) -> SumeragiLanePayloadOwnership {
        let mut validator_set = deterministic_topology(VALIDATORS_PER_LANE);
        validator_set.sort();
        let accepted_candidate_indices = (0..accepted_count)
            .map(|index| u64::try_from(index).expect("fixture candidate index fits u64"))
            .collect::<Vec<_>>();
        let accepted_transaction_hashes = (0..accepted_count)
            .map(|index| test_hash(u8::try_from(0x40 + index).expect("fixture hash tag")))
            .collect::<Vec<_>>();
        let mut ownership = SumeragiLanePayloadOwnership {
            proposal_height: lane_block_height.saturating_add(10),
            proposal_view: 0,
            lane_id,
            dataspace_id,
            lane_incarnation: test_hash(0x0F),
            lane_block_height,
            lane_block_view: 0,
            subject_hash: test_hash(0x10),
            qc_mode_tag: "test-bls".to_owned(),
            accepted_candidate_indices,
            accepted_transaction_hashes,
            previous_lane_block_height: lane_block_height.saturating_sub(1),
            previous_lane_block_descriptor_hash: None,
            lane_block_descriptor_hash: Some(test_hash(0x11)),
            lane_block_descriptor_validator_set: validator_set,
            lane_block_descriptor_validator_count: VALIDATORS_PER_LANE as u32,
            lane_block_descriptor_min_quorum: min_quorum,
            payload_ownership_hash: test_hash(0x12),
            rbc_instance_hash: test_hash(0x13),
        };
        if accepted_count > 0 && min_quorum > 0 && min_quorum <= VALIDATORS_PER_LANE as u32 {
            let replay_hashes = ownership
                .compute_replay_hashes()
                .expect("fixture lane payload ownership replay hashes");
            ownership.subject_hash = replay_hashes.subject_hash;
            ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
            ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
            ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
        }
        ownership
    }

    #[test]
    fn committed_lane_block_quorum_requires_exact_lane_committee() {
        let quorum = u32::try_from(commit_quorum_from_len(VALIDATORS_PER_LANE))
            .expect("fixture quorum fits u32");
        let valid = sample_committed_lane_block(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            1,
            quorum,
            quorum,
            quorum,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
        );
        assert!(committed_lane_block_has_expected_quorum(
            &valid,
            VALIDATORS_PER_LANE
        ));

        let mut under_quorum = valid.clone();
        under_quorum.commit_qc_signer_count = quorum - 1;
        assert!(!committed_lane_block_has_expected_quorum(
            &under_quorum,
            VALIDATORS_PER_LANE
        ));

        let mut wrong_committee = valid;
        wrong_committee.validator_count = (VALIDATORS_PER_LANE + 1) as u32;
        assert!(!committed_lane_block_has_expected_quorum(
            &wrong_committee,
            VALIDATORS_PER_LANE
        ));
    }

    #[test]
    fn latest_lane_domain_progress_requires_exact_dataspace_and_qc_quorum() {
        let quorum = u32::try_from(commit_quorum_from_len(VALIDATORS_PER_LANE))
            .expect("fixture quorum fits u32");
        let mut under_quorum = sample_committed_lane_block(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            4,
            quorum,
            quorum,
            quorum - 1,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
        );
        under_quorum.proposal_hash = test_hash(0x20);
        let rejected = sample_committed_lane_block(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            5,
            quorum,
            quorum,
            quorum,
            COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION,
        );
        let wrong_dataspace = sample_committed_lane_block(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS2_ID_U64),
            99,
            quorum,
            quorum,
            quorum,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
        );
        let valid = sample_committed_lane_block(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            3,
            quorum,
            quorum,
            quorum,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
        );
        let status = SumeragiDiagnosticsStatus {
            dataspace_commitments: vec![SumeragiDataspaceCommitment {
                block_height: 11,
                lane_id: LaneId::new(DS1_LANE_INDEX),
                dataspace_id: DataSpaceId::new(DS1_ID_U64),
                tx_count: 1,
                total_chunks: 1,
                rbc_bytes_total: 32,
                teu_total: 1,
                block_hash: HashOf::from_untyped_unchecked(test_hash(0x30)),
            }],
            committed_lane_blocks: vec![wrong_dataspace, under_quorum, rejected, valid],
            ..empty_sumeragi_diagnostics()
        };

        let progress = latest_lane_domain_progress(
            &status,
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
        )
        .expect("valid exact-dataspace lane-domain QC progress");

        assert_eq!(
            progress.lane_block_height, 5,
            "committed QC evidence counts even when execution is not yet progress-ready"
        );
        assert_eq!(progress.dataspace_commitment_height, Some(11));
        assert_eq!(progress.prepare_qc_signer_count, quorum);
        assert_eq!(progress.commit_qc_signer_count, quorum);
        assert!(
            latest_lane_domain_progress(
                &status,
                LaneId::new(DS2_LANE_INDEX),
                DataSpaceId::new(DS1_ID_U64),
            )
            .is_none(),
            "wrong lane must not satisfy the progress probe"
        );
    }

    #[test]
    fn latest_lane_domain_application_progress_requires_applied_latest_row() {
        let quorum = u32::try_from(commit_quorum_from_len(VALIDATORS_PER_LANE))
            .expect("fixture quorum fits u32");
        let applied_lower = sample_committed_lane_block(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            3,
            quorum,
            quorum,
            quorum,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
        );
        let unapplied_latest = sample_committed_lane_block(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            4,
            quorum,
            quorum,
            quorum,
            COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION,
        );
        let status = SumeragiDiagnosticsStatus {
            committed_lane_blocks: vec![applied_lower.clone(), unapplied_latest],
            ..empty_sumeragi_diagnostics()
        };

        assert_eq!(
            latest_lane_domain_progress(
                &status,
                LaneId::new(DS1_LANE_INDEX),
                DataSpaceId::new(DS1_ID_U64),
            )
            .map(|progress| progress.lane_block_height),
            Some(4),
            "the committed-QC gate should still expose the latest certified lane block"
        );
        assert!(
            latest_lane_domain_application_progress(
                &status,
                LaneId::new(DS1_LANE_INDEX),
                DataSpaceId::new(DS1_ID_U64),
            )
            .is_none(),
            "an unapplied latest row must block receipt-backed application progress"
        );

        let direct_applied_latest = sample_committed_lane_block(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            4,
            quorum,
            quorum,
            quorum,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
        );
        let direct_applied_status = SumeragiDiagnosticsStatus {
            committed_lane_blocks: vec![applied_lower.clone(), direct_applied_latest],
            ..empty_sumeragi_diagnostics()
        };
        assert_eq!(
            latest_lane_domain_application_progress(
                &direct_applied_status,
                LaneId::new(DS1_LANE_INDEX),
                DataSpaceId::new(DS1_ID_U64),
            )
            .map(|progress| progress.lane_block_height),
            Some(4),
            "direct execution receipt status should satisfy applied progress"
        );

        let mut unavailable_applied = applied_lower;
        unavailable_applied.executable_payload_available = false;
        let unavailable_status = SumeragiDiagnosticsStatus {
            committed_lane_blocks: vec![unavailable_applied],
            ..empty_sumeragi_diagnostics()
        };
        assert!(
            latest_lane_domain_application_progress(
                &unavailable_status,
                LaneId::new(DS1_LANE_INDEX),
                DataSpaceId::new(DS1_ID_U64),
            )
            .is_none(),
            "applied status without executable payload material must not count"
        );
    }

    #[test]
    fn applied_lane_domain_progress_accepts_lower_applied_certified_row() {
        let quorum = u32::try_from(commit_quorum_from_len(VALIDATORS_PER_LANE))
            .expect("fixture quorum fits u32");
        let applied_lower = sample_committed_lane_block(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            3,
            quorum,
            quorum,
            quorum,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
        );
        let pending_latest = sample_committed_lane_block(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            4,
            quorum,
            quorum,
            quorum,
            COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION,
        );
        let mut applied_top = sample_committed_lane_block(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            5,
            quorum,
            quorum,
            quorum,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION,
        );
        applied_top.proposal_hash = test_hash(0x70);
        let status = SumeragiDiagnosticsStatus {
            committed_lane_blocks: vec![applied_lower.clone(), pending_latest, applied_top.clone()],
            ..empty_sumeragi_diagnostics()
        };

        let progress = applied_lane_domain_progress(
            &status,
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
        )
        .expect("highest applied certified row should count");
        assert_eq!(progress.lane_block_height, 5);
        assert_eq!(progress.proposal_hash, applied_top.proposal_hash);

        let mut conflicting_top = applied_top.clone();
        conflicting_top.proposal_hash = test_hash(0x71);
        let ambiguous_status = SumeragiDiagnosticsStatus {
            committed_lane_blocks: vec![applied_lower, applied_top, conflicting_top],
            ..empty_sumeragi_diagnostics()
        };
        assert!(
            applied_lane_domain_progress(
                &ambiguous_status,
                LaneId::new(DS1_LANE_INDEX),
                DataSpaceId::new(DS1_ID_U64),
            )
            .is_none(),
            "conflicting applied identities at the same top height must fail closed"
        );
    }

    #[test]
    fn quorum_lane_domain_progress_requires_quorum_at_candidate_height() {
        let lane_id = LaneId::new(DS1_LANE_INDEX);
        let dataspace_id = DataSpaceId::new(DS1_ID_U64);
        let observations = [
            test_lane_domain_progress(lane_id, dataspace_id, 1, 0),
            test_lane_domain_progress(lane_id, dataspace_id, 2, 0),
            test_lane_domain_progress(lane_id, dataspace_id, 2, 1),
            test_lane_domain_progress(lane_id, dataspace_id, 3, 0),
        ];

        let progress =
            quorum_lane_domain_progress(&observations, 3).expect("height 2 has quorum support");

        assert_eq!(progress.lane_block_height, 2);
        assert_eq!(progress.lane_block_view, 0);
        assert!(
            quorum_lane_domain_progress(&observations, 5).is_none(),
            "larger than peer-count quorum must not be satisfied"
        );

        let mut conflicting_same_height = test_lane_domain_progress(lane_id, dataspace_id, 2, 0);
        conflicting_same_height.proposal_hash = test_hash(0xD1);
        let conflicting_observations = [
            test_lane_domain_progress(lane_id, dataspace_id, 2, 0),
            conflicting_same_height,
        ];
        assert!(
            quorum_lane_domain_progress(&conflicting_observations, 2).is_none(),
            "conflicting same-height committed-lane identities must not combine into quorum progress"
        );

        let incarnation_a = test_lane_domain_progress(lane_id, dataspace_id, 2, 0);
        let mut incarnation_b = incarnation_a.clone();
        incarnation_b.lane_incarnation = test_hash(0xE1);
        assert!(
            quorum_lane_domain_progress(&[incarnation_a, incarnation_b], 2).is_none(),
            "different lane incarnations must not combine into quorum progress"
        );

        let quorum_identity_a = test_lane_domain_progress(lane_id, dataspace_id, 4, 0);
        let mut quorum_identity_b = quorum_identity_a.clone();
        quorum_identity_b.proposal_hash = test_hash(0xD2);
        let split_top_quorum = vec![
            quorum_identity_a.clone(),
            quorum_identity_a.clone(),
            quorum_identity_a,
            quorum_identity_b.clone(),
            quorum_identity_b.clone(),
            quorum_identity_b,
        ];
        assert!(
            quorum_lane_domain_progress(&split_top_quorum, 3).is_none(),
            "conflicting top committed-lane identities with independent quorum must fail closed"
        );
    }

    #[test]
    fn lane_domain_progress_after_baseline_requires_strict_same_lane_tip_advance() {
        let lane_id = LaneId::new(DS1_LANE_INDEX);
        let dataspace_id = DataSpaceId::new(DS1_ID_U64);
        let baseline = test_lane_domain_progress(lane_id, dataspace_id, 4, 1);
        let same_tip = test_lane_domain_progress(lane_id, dataspace_id, 4, 1);
        let higher_view = test_lane_domain_progress(lane_id, dataspace_id, 4, 2);
        let higher_height = test_lane_domain_progress(lane_id, dataspace_id, 5, 0);
        let wrong_lane = test_lane_domain_progress(LaneId::new(DS2_LANE_INDEX), dataspace_id, 5, 0);
        let mut wrong_incarnation = higher_height.clone();
        wrong_incarnation.lane_incarnation = test_hash(0xEE);

        assert!(!lane_domain_progress_is_after_baseline(
            &same_tip, &baseline
        ));
        assert!(lane_domain_progress_is_after_baseline(
            &higher_view,
            &baseline
        ));
        assert!(lane_domain_progress_is_after_baseline(
            &higher_height,
            &baseline
        ));
        assert!(!lane_domain_progress_is_after_baseline(
            &wrong_lane,
            &baseline
        ));
        assert!(!lane_domain_progress_is_after_baseline(
            &wrong_incarnation,
            &baseline
        ));
    }

    #[test]
    fn latest_lane_domain_progress_rejects_ambiguous_latest_committed_rows() {
        let quorum = u32::try_from(commit_quorum_from_len(VALIDATORS_PER_LANE))
            .expect("fixture quorum fits u32");
        let valid = sample_committed_lane_block(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            3,
            quorum,
            quorum,
            quorum,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
        );
        let mut proposal_drift = valid.clone();
        proposal_drift.proposal_hash = test_hash(0x31);
        let exact_duplicate = valid.clone();
        let status = SumeragiDiagnosticsStatus {
            committed_lane_blocks: vec![valid.clone(), exact_duplicate],
            ..empty_sumeragi_diagnostics()
        };
        assert!(
            latest_lane_domain_progress(
                &status,
                LaneId::new(DS1_LANE_INDEX),
                DataSpaceId::new(DS1_ID_U64),
            )
            .is_some(),
            "exact duplicate committed-lane rows should remain idempotent"
        );

        let ambiguous = SumeragiDiagnosticsStatus {
            committed_lane_blocks: vec![valid, proposal_drift],
            ..empty_sumeragi_diagnostics()
        };
        assert!(
            latest_lane_domain_progress(
                &ambiguous,
                LaneId::new(DS1_LANE_INDEX),
                DataSpaceId::new(DS1_ID_U64),
            )
            .is_none(),
            "same-slot committed-lane proposal drift must not publish cross-dataspace QC progress"
        );
    }

    #[test]
    fn latest_lane_domain_progress_rejects_malformed_latest_committed_row() {
        let quorum = u32::try_from(commit_quorum_from_len(VALIDATORS_PER_LANE))
            .expect("fixture quorum fits u32");
        let valid = sample_committed_lane_block(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            3,
            quorum,
            quorum,
            quorum,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
        );
        let malformed_latest = sample_committed_lane_block(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            4,
            quorum,
            quorum,
            quorum - 1,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
        );
        let status = SumeragiDiagnosticsStatus {
            committed_lane_blocks: vec![valid, malformed_latest],
            ..empty_sumeragi_diagnostics()
        };
        assert!(
            latest_lane_domain_progress(
                &status,
                LaneId::new(DS1_LANE_INDEX),
                DataSpaceId::new(DS1_ID_U64),
            )
            .is_none(),
            "malformed latest committed-lane rows must block cross-dataspace QC progress"
        );
    }

    #[test]
    fn latest_lane_payload_ownership_progress_requires_exact_replayable_quorum() {
        let quorum = u32::try_from(commit_quorum_from_len(VALIDATORS_PER_LANE))
            .expect("fixture quorum fits u32");
        let under_quorum = sample_lane_payload_ownership(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            2,
            quorum - 1,
            1,
        );
        let empty_work = sample_lane_payload_ownership(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            1,
            quorum,
            0,
        );
        let wrong_dataspace = sample_lane_payload_ownership(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS2_ID_U64),
            99,
            quorum,
            1,
        );
        let mut forged_subject = sample_lane_payload_ownership(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            2,
            quorum,
            1,
        );
        forged_subject.subject_hash = test_hash(0x52);
        let valid = sample_lane_payload_ownership(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            3,
            quorum,
            2,
        );
        let status = SumeragiDiagnosticsStatus {
            lane_payload_ownerships: vec![
                wrong_dataspace,
                under_quorum,
                empty_work,
                forged_subject,
                valid,
            ],
            ..empty_sumeragi_diagnostics()
        };

        let progress = latest_lane_payload_ownership_progress(
            &status,
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
        )
        .expect("valid exact-dataspace lane payload ownership progress");

        assert_eq!(progress.lane_block_height, 3);
        assert_eq!(progress.accepted_transaction_count, 2);
        assert_eq!(progress.min_quorum, quorum);
        assert_eq!(progress.validator_count, VALIDATORS_PER_LANE as u32);
        assert!(
            latest_lane_payload_ownership_progress(
                &status,
                LaneId::new(DS2_LANE_INDEX),
                DataSpaceId::new(DS1_ID_U64),
            )
            .is_none(),
            "wrong lane must not satisfy the ownership progress probe"
        );
    }

    #[test]
    fn latest_lane_payload_ownership_progress_rejects_malformed_latest_replay_row() {
        let quorum = u32::try_from(commit_quorum_from_len(VALIDATORS_PER_LANE))
            .expect("fixture quorum fits u32");
        let valid = sample_lane_payload_ownership(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            3,
            quorum,
            2,
        );
        let mut malformed_latest = sample_lane_payload_ownership(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            4,
            quorum,
            1,
        );
        malformed_latest.subject_hash = test_hash(0x53);
        let status = SumeragiDiagnosticsStatus {
            lane_payload_ownerships: vec![valid, malformed_latest],
            ..empty_sumeragi_diagnostics()
        };
        assert!(
            latest_lane_payload_ownership_progress(
                &status,
                LaneId::new(DS1_LANE_INDEX),
                DataSpaceId::new(DS1_ID_U64),
            )
            .is_none(),
            "malformed latest ownership replay rows must block lane-payload progress"
        );
    }

    #[test]
    fn latest_lane_payload_ownership_progress_rejects_ambiguous_latest_replay_identity() {
        let quorum = u32::try_from(commit_quorum_from_len(VALIDATORS_PER_LANE))
            .expect("fixture quorum fits u32");
        let valid = sample_lane_payload_ownership(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            4,
            quorum,
            1,
        );
        let exact_duplicate = valid.clone();
        let mut conflicting = sample_lane_payload_ownership(
            LaneId::new(DS1_LANE_INDEX),
            DataSpaceId::new(DS1_ID_U64),
            4,
            quorum,
            2,
        );
        conflicting.proposal_height = valid.proposal_height;
        conflicting.proposal_view = valid.proposal_view;
        let replay_hashes = conflicting
            .compute_replay_hashes()
            .expect("conflicting fixture replay hashes");
        conflicting.subject_hash = replay_hashes.subject_hash;
        conflicting.payload_ownership_hash = replay_hashes.payload_ownership_hash;
        conflicting.rbc_instance_hash = replay_hashes.rbc_instance_hash;
        conflicting.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);

        let duplicate_status = SumeragiDiagnosticsStatus {
            lane_payload_ownerships: vec![valid.clone(), exact_duplicate],
            ..empty_sumeragi_diagnostics()
        };
        assert!(
            latest_lane_payload_ownership_progress(
                &duplicate_status,
                LaneId::new(DS1_LANE_INDEX),
                DataSpaceId::new(DS1_ID_U64),
            )
            .is_some(),
            "exact duplicate ownership rows should remain idempotent"
        );

        let ambiguous_status = SumeragiDiagnosticsStatus {
            lane_payload_ownerships: vec![valid, conflicting],
            ..empty_sumeragi_diagnostics()
        };
        assert!(
            latest_lane_payload_ownership_progress(
                &ambiguous_status,
                LaneId::new(DS1_LANE_INDEX),
                DataSpaceId::new(DS1_ID_U64),
            )
            .is_none(),
            "same-slot ownership replay identity drift must not publish lane-payload progress"
        );
    }

    #[test]
    fn quorum_lane_payload_ownership_progress_requires_quorum_at_candidate_height() {
        let lane_id = LaneId::new(DS1_LANE_INDEX);
        let dataspace_id = DataSpaceId::new(DS1_ID_U64);
        let observations = [
            test_lane_payload_ownership_progress(lane_id, dataspace_id, 1, 0, 10),
            test_lane_payload_ownership_progress(lane_id, dataspace_id, 2, 0, 11),
            test_lane_payload_ownership_progress(lane_id, dataspace_id, 2, 1, 12),
            test_lane_payload_ownership_progress(lane_id, dataspace_id, 3, 0, 13),
        ];

        let progress = quorum_lane_payload_ownership_progress(&observations, 3)
            .expect("height 2 ownership has quorum support");

        assert_eq!(progress.lane_block_height, 2);
        assert_eq!(progress.lane_block_view, 0);
        assert!(
            quorum_lane_payload_ownership_progress(&observations, 5).is_none(),
            "larger than peer-count quorum must not be satisfied"
        );

        let mut conflicting_same_height =
            test_lane_payload_ownership_progress(lane_id, dataspace_id, 2, 0, 11);
        conflicting_same_height.payload_ownership_hash = test_hash(0xC1);
        let conflicting_observations = [
            test_lane_payload_ownership_progress(lane_id, dataspace_id, 2, 0, 11),
            conflicting_same_height,
        ];
        assert!(
            quorum_lane_payload_ownership_progress(&conflicting_observations, 2).is_none(),
            "conflicting same-height ownership identities must not combine into quorum progress"
        );

        let incarnation_a = test_lane_payload_ownership_progress(lane_id, dataspace_id, 2, 0, 11);
        let mut incarnation_b = incarnation_a.clone();
        incarnation_b.lane_incarnation = test_hash(0xE2);
        assert!(
            quorum_lane_payload_ownership_progress(&[incarnation_a, incarnation_b], 2).is_none(),
            "different lane incarnations must not combine into ownership quorum progress"
        );

        let quorum_identity_a =
            test_lane_payload_ownership_progress(lane_id, dataspace_id, 4, 0, 20);
        let mut quorum_identity_b = quorum_identity_a.clone();
        quorum_identity_b.payload_ownership_hash = test_hash(0xC2);
        let split_top_quorum = vec![
            quorum_identity_a.clone(),
            quorum_identity_a.clone(),
            quorum_identity_a,
            quorum_identity_b.clone(),
            quorum_identity_b.clone(),
            quorum_identity_b,
        ];
        assert!(
            quorum_lane_payload_ownership_progress(&split_top_quorum, 3).is_none(),
            "conflicting top ownership identities with independent quorum must fail closed"
        );
    }

    #[test]
    fn duration_min_avg_max_secs_reports_expected_values() {
        assert!(duration_min_avg_max_secs(&[]).is_none());

        let (min, avg, max) = duration_min_avg_max_secs(&[
            Duration::from_millis(500),
            Duration::from_millis(1500),
            Duration::from_secs(1),
        ])
        .expect("duration summary");

        assert_eq!(min, 0.5);
        assert_eq!(avg, 1.0);
        assert_eq!(max, 1.5);
    }

    #[test]
    fn duration_min_avg_max_secs_handles_single_sample() {
        let (min, avg, max) =
            duration_min_avg_max_secs(&[Duration::from_millis(250)]).expect("duration summary");

        assert_eq!(min, 0.25);
        assert_eq!(avg, 0.25);
        assert_eq!(max, 0.25);
    }

    #[test]
    fn committed_tx_outcome_quorum_requires_matching_outcome_quorum() {
        assert_eq!(
            committed_tx_outcome_quorum(
                &[CommittedTxOutcome::Applied, CommittedTxOutcome::Applied],
                3
            ),
            None,
            "minority applied observations must not satisfy committed outcome"
        );

        assert_eq!(
            committed_tx_outcome_quorum(
                &[
                    CommittedTxOutcome::Applied,
                    CommittedTxOutcome::Rejected("insufficient funds".to_owned()),
                    CommittedTxOutcome::Applied,
                ],
                2,
            ),
            Some(CommittedTxOutcome::Applied)
        );

        assert_eq!(
            committed_tx_outcome_quorum(
                &[
                    CommittedTxOutcome::Rejected("insufficient funds".to_owned()),
                    CommittedTxOutcome::Rejected("other rejection".to_owned()),
                    CommittedTxOutcome::Rejected("insufficient funds".to_owned()),
                ],
                2,
            ),
            Some(CommittedTxOutcome::Rejected(
                "insufficient funds".to_owned()
            ))
        );

        assert_eq!(
            committed_tx_outcome_quorum(
                &[
                    CommittedTxOutcome::Rejected("left".to_owned()),
                    CommittedTxOutcome::Rejected("right".to_owned()),
                ],
                2,
            ),
            None,
            "split rejection reasons must fail closed"
        );
    }

    #[test]
    fn bounded_observer_request_timeout_handles_exhausted_and_large_budgets() {
        let exhausted = bounded_observer_request_timeout(
            Instant::now() - Duration::from_secs(10),
            Duration::from_secs(1),
            4,
        );
        assert_eq!(exhausted, Duration::from_millis(1));

        let large_single_client =
            bounded_observer_request_timeout(Instant::now(), Duration::from_secs(90), 1);
        assert!(
            large_single_client <= OBSERVER_QUERY_TIMEOUT_CAP,
            "large per-client slice should be capped"
        );

        let short_budget =
            bounded_observer_request_timeout(Instant::now(), Duration::from_millis(500), 100);
        assert!(
            short_budget <= Duration::from_millis(500),
            "short remaining budgets should not be inflated by the floor"
        );
    }

    #[test]
    fn bounded_observer_request_timeout_treats_zero_remaining_clients_as_one() {
        let timeout =
            bounded_observer_request_timeout(Instant::now(), OBSERVER_QUERY_TIMEOUT_CAP * 3, 0);

        assert!(
            timeout <= OBSERVER_QUERY_TIMEOUT_CAP,
            "zero remaining clients should still apply the per-peer cap"
        );
    }

    #[test]
    fn total_balance_observer_request_slots_counts_empty_sets_as_one_slot() {
        let slots = total_balance_observer_request_slots([3, 0, 2, 1]);

        assert_eq!(
            slots, 7,
            "empty balance observer sets should still consume one diagnostic slot"
        );
    }

    #[test]
    fn bounded_observer_request_timeout_applies_floor_when_slice_is_small() {
        let timeout = bounded_observer_request_timeout(Instant::now(), Duration::from_secs(9), 100);

        assert_eq!(
            timeout,
            Duration::from_secs(2),
            "small per-client slices should use the observer floor when the remaining budget allows it"
        );
    }

    #[test]
    fn bounded_observer_request_timeout_uses_divided_slice_between_floor_and_cap() {
        let timeout = bounded_observer_request_timeout(Instant::now(), Duration::from_secs(9), 3);

        assert!(
            timeout > Duration::from_secs(2),
            "divided slice should stay above the observer floor"
        );
        assert!(
            timeout <= Duration::from_secs(3),
            "divided slice should be close to the remaining budget divided by clients"
        );
    }

    #[test]
    fn should_submit_tick_fires_on_initial_and_interval_polls() {
        assert!(should_submit_tick(0, 5));
        assert!(!should_submit_tick(4, 5));
        assert!(should_submit_tick(5, 5));
        assert!(!should_submit_tick(0, 0));
    }

    #[test]
    fn tx_fallback_error_classifiers_match_cross_dataspace_outcomes() {
        for error_text in [
            "transaction.status_timeout_ms elapsed",
            "haven't got tx confirmation within 20s",
            "transaction queued for too long",
            "Transaction submitter thread exited with error: closed",
            "Failed to send http POST request: connection reset",
        ] {
            assert!(
                is_inconclusive_blocking_submit_error(error_text),
                "{error_text} should be treated as inconclusive"
            );
        }
        assert!(!is_inconclusive_blocking_submit_error(
            "settlement leg requires 10000"
        ));

        assert!(is_inconclusive_committed_outcome_error(
            "timed out waiting for committed transaction outcome"
        ));
        assert!(!is_inconclusive_committed_outcome_error(
            "transaction rejected by validation"
        ));
    }

    #[test]
    fn tx_fallback_error_classifiers_ignore_unrelated_phrases() {
        for error_text in [
            "queued transaction was rejected by validation",
            "failed to send http GET request",
            "submitter thread completed normally",
            "transaction status timeout was not configured",
            "Failed to send http PATCH request",
        ] {
            assert!(
                !is_inconclusive_blocking_submit_error(error_text),
                "{error_text} should stay conclusive"
            );
        }

        assert!(!is_inconclusive_committed_outcome_error(
            "waiting for committed transaction outcome succeeded"
        ));
    }

    #[test]
    fn rollback_failure_classifier_accepts_rejection_or_inconclusive_confirmation() {
        assert!(is_expected_rollback_failure_text(
            "settlement leg requires 10000 units"
        ));
        assert!(is_expected_rollback_failure_text(
            "haven't got tx confirmation within 600s (configured with `transaction.status_timeout_ms`)"
        ));
        assert!(is_expected_rollback_failure_text(
            "timed out waiting for committed transaction outcome"
        ));
        assert!(!is_expected_rollback_failure_text(
            "transaction applied successfully"
        ));
    }

    #[test]
    fn render_rejection_reason_includes_debug_details_when_display_is_generic() {
        let reason = TransactionRejectionReason::LimitCheck(TransactionLimitError {
            reason: "cross-dataspace route limit exceeded".to_owned(),
        });

        let rendered = render_rejection_reason(&reason);

        assert!(rendered.contains("Failed to validate transaction limits"));
        assert!(rendered.contains("details:"));
        assert!(rendered.contains("cross-dataspace route limit exceeded"));
    }

    #[test]
    fn fanout_header_helper_accepts_local_and_proxy_without_singular_route() {
        for routed_by in ["local", "proxy"] {
            let response = RoutedJsonGetResponse {
                body: JsonValue::Null,
                routed_by: Some(routed_by.to_owned()),
                route_lane_id: None,
                route_dataspace_id: None,
            };

            expect_local_or_proxy_fanout_headers(&response, "fanout")
                .expect("local/proxy fanout response should pass");
        }

        let missing_route_source = RoutedJsonGetResponse {
            body: JsonValue::Null,
            routed_by: None,
            route_lane_id: None,
            route_dataspace_id: None,
        };
        let err = expect_local_or_proxy_fanout_headers(&missing_route_source, "fanout")
            .expect_err("missing route source should fail");
        assert!(err.to_string().contains("expected local or proxy fanout"));

        let unknown_route_source = RoutedJsonGetResponse {
            body: JsonValue::Null,
            routed_by: Some("remote".to_owned()),
            route_lane_id: None,
            route_dataspace_id: None,
        };
        let err = expect_local_or_proxy_fanout_headers(&unknown_route_source, "fanout")
            .expect_err("unknown route source should fail");
        assert!(err.to_string().contains("expected local or proxy fanout"));

        let singular_route = RoutedJsonGetResponse {
            body: JsonValue::Null,
            routed_by: Some("proxy".to_owned()),
            route_lane_id: Some("1".to_owned()),
            route_dataspace_id: None,
        };
        let err = expect_local_or_proxy_fanout_headers(&singular_route, "fanout")
            .expect_err("singular fanout route should fail");
        assert!(
            err.to_string()
                .contains("fanout response should not expose a singular route lane")
        );

        let singular_dataspace = RoutedJsonGetResponse {
            body: JsonValue::Null,
            routed_by: Some("proxy".to_owned()),
            route_lane_id: None,
            route_dataspace_id: Some("2".to_owned()),
        };
        let err = expect_local_or_proxy_fanout_headers(&singular_dataspace, "fanout")
            .expect_err("singular fanout dataspace should fail");
        assert!(
            err.to_string()
                .contains("fanout response should not expose a singular route dataspace")
        );
    }

    #[test]
    fn routed_header_string_reads_present_headers_and_ignores_absent_ones() {
        let mut headers = HeaderMap::new();
        headers.insert("x-iroha-routed-by", HeaderValue::from_static("proxy"));
        headers.insert(
            "x-iroha-invalid",
            HeaderValue::from_bytes(&[0xFF]).expect("binary header value"),
        );

        assert_eq!(
            routed_header_string(&headers, "x-iroha-routed-by"),
            Some("proxy".to_owned())
        );
        assert_eq!(
            routed_header_string(&headers, "x-iroha-route-lane-id"),
            None
        );
        assert_eq!(routed_header_string(&headers, "x-iroha-invalid"), None);
    }

    #[derive(Debug)]
    struct DisplayOnlyTxError;

    impl Display for DisplayOnlyTxError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
            formatter.write_str("route probe failed")
        }
    }

    #[test]
    fn render_error_with_debug_keeps_display_and_debug_context() {
        assert_eq!(
            render_error_with_debug(&DisplayOnlyTxError),
            "route probe failed (DisplayOnlyTxError)"
        );
    }
}
