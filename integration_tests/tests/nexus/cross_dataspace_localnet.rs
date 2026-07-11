#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Localnet cross-dataspace atomic swap regression test.

use super::localnet_npos::npos_override_transactions;

use std::{
    collections::{BTreeMap, BTreeSet},
    num::{NonZeroU32, NonZeroU64},
    thread,
    time::{Duration, Instant},
};

use eyre::{Result, ensure, eyre};
use futures_util::StreamExt;
use integration_tests::sandbox;
use iroha::{
    client::Client,
    crypto::HashOf,
    data_model::{
        Level, ValidationFail,
        account::{Account, AccountId},
        asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
        block::consensus::{
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION, SumeragiCommittedLaneBlock,
            SumeragiLanePayloadOwnership, SumeragiStatusWire,
        },
        da::commitment::DaProofPolicyBundle,
        domain::{Domain, DomainId},
        events::{
            EventBox,
            pipeline::{PipelineEventBox, TransactionEventFilter, TransactionStatus},
        },
        isi::{
            Grant, InstructionBox, Log, Mint, Register,
            settlement::{
                DvpIsi, SettlementAtomicity, SettlementExecutionOrder, SettlementLeg,
                SettlementPlan,
            },
            staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
        },
        metadata::Metadata,
        nexus::{DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId, LaneVisibility},
        peer::PeerId,
        permission::Permission,
        prelude::{FindAssetById, FindAssets, FindPermissionsByAccountId, Numeric},
        transaction::{SignedTransaction, TransactionEntrypoint},
    },
    query::QueryError,
};
use iroha_config::parameters::actual::LaneConfig as ActualLaneConfig;
use iroha_core::{da::proof_policy_bundle, sumeragi::network_topology::commit_quorum_from_len};
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
use iroha_test_network::{NetworkBuilder, genesis_factory_with_post_topology};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR};
use norito::json::Value as JsonValue;
use tokio::{
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
const TOTAL_PEERS: usize = 12;
const VALIDATORS_PER_LANE: usize = 4;
const VALIDATOR_STAKE: u64 = 2_000;
const NEXUS_FEE_SEED_AMOUNT: u32 = 1_000_000;
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
const SOAK_COMMITTED_OUTCOME_TIMEOUT: Duration = Duration::from_secs(6);
const SOAK_BARRIER_TICK_EVERY_POLLS: u64 = 5;
const SOAK_FALLBACK_LOG_LIMIT: usize = 3;
const SOAK_ITERATION_ATTEMPTS: usize = 3;
const SOAK_ITERATIONS: usize = 10;
const SOAK_ITERATIONS_ENV: &str = "IROHA_NEXUS_CROSS_SOAK_ITERATIONS";
// The PR soak must demonstrate repeatable progress rather than one lucky iteration. Allow one
// persistent failure and at most two retry attempts in the default ten-iteration run so shared
// host noise remains diagnosable without masking broad lane-consensus instability.
const SOAK_MIN_PASS_RATE_PERCENT: usize = 90;
const SOAK_MAX_RETRY_RATE_PERCENT: usize = 20;
const CROSS_DATASPACE_LOCALNET_STACK_BYTES: usize = 32 * 1024 * 1024;

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

fn parse_positive_usize_override(raw: Option<&str>, default: usize) -> usize {
    raw.and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn soak_iterations() -> usize {
    parse_positive_usize_override(
        std::env::var(SOAK_ITERATIONS_ENV).ok().as_deref(),
        SOAK_ITERATIONS,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SoakGateMetrics {
    iterations: usize,
    passes: usize,
    failures: usize,
    retries_used: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SoakGateFailure {
    NoIterations,
    AccountingMismatch {
        iterations: usize,
        passes: usize,
        failures: usize,
    },
    PassRateBelowMinimum {
        iterations: usize,
        passes: usize,
        minimum_passes: usize,
    },
    RetryBudgetExceeded {
        iterations: usize,
        retries_used: usize,
        maximum_retries: usize,
    },
}

impl core::fmt::Display for SoakGateFailure {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoIterations => formatter.write_str("no soak iterations were scheduled"),
            Self::AccountingMismatch {
                iterations,
                passes,
                failures,
            } => write!(
                formatter,
                "soak accounting mismatch: iterations={iterations}, passes={passes}, failures={failures}"
            ),
            Self::PassRateBelowMinimum {
                iterations,
                passes,
                minimum_passes,
            } => write!(
                formatter,
                "soak pass rate below {SOAK_MIN_PASS_RATE_PERCENT}%: passes={passes}, required={minimum_passes}, iterations={iterations}"
            ),
            Self::RetryBudgetExceeded {
                iterations,
                retries_used,
                maximum_retries,
            } => write!(
                formatter,
                "soak retry budget exceeded ({SOAK_MAX_RETRY_RATE_PERCENT}% of scheduled iterations): retries={retries_used}, maximum={maximum_retries}, iterations={iterations}"
            ),
        }
    }
}

fn soak_gate_minimum_passes(iterations: usize) -> usize {
    (iterations / 100) * SOAK_MIN_PASS_RATE_PERCENT
        + ((iterations % 100) * SOAK_MIN_PASS_RATE_PERCENT).div_ceil(100)
}

fn soak_gate_maximum_retries(iterations: usize) -> usize {
    (iterations / 100) * SOAK_MAX_RETRY_RATE_PERCENT
        + ((iterations % 100) * SOAK_MAX_RETRY_RATE_PERCENT) / 100
}

fn validate_soak_gate(metrics: SoakGateMetrics) -> core::result::Result<(), SoakGateFailure> {
    if metrics.iterations == 0 {
        return Err(SoakGateFailure::NoIterations);
    }
    if metrics.passes > metrics.iterations
        || metrics.failures != metrics.iterations.saturating_sub(metrics.passes)
    {
        return Err(SoakGateFailure::AccountingMismatch {
            iterations: metrics.iterations,
            passes: metrics.passes,
            failures: metrics.failures,
        });
    }

    let minimum_passes = soak_gate_minimum_passes(metrics.iterations);
    if metrics.passes < minimum_passes {
        return Err(SoakGateFailure::PassRateBelowMinimum {
            iterations: metrics.iterations,
            passes: metrics.passes,
            minimum_passes,
        });
    }

    let maximum_retries = soak_gate_maximum_retries(metrics.iterations);
    if metrics.retries_used > maximum_retries {
        return Err(SoakGateFailure::RetryBudgetExceeded {
            iterations: metrics.iterations,
            retries_used: metrics.retries_used,
            maximum_retries,
        });
    }

    Ok(())
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

fn localnet_builder() -> NetworkBuilder {
    let gas_account_str = cross_dataspace_gas_account_id()
        .canonical_i105()
        .expect("canonical I105 escrow account literal");
    NetworkBuilder::new()
        .with_peers(TOTAL_PEERS)
        .without_npos_genesis_bootstrap()
        .with_genesis_block(|topology, topology_entries| {
            let post_topology =
                npos_multilane_genesis_post_topology_transactions(topology.as_ref());
            let mut genesis = genesis_factory_with_post_topology(
                npos_override_transactions(VALIDATORS_PER_LANE, TOTAL_PEERS),
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
                )
                .write(["sumeragi", "npos", "use_stake_snapshot_roster"], true);
        })
}

fn multilane_da_proof_policy_bundle() -> DaProofPolicyBundle {
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
    let catalog = LaneCatalog::new(lane_count, lanes).expect("lane catalog");
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
        Mint::asset_numeric(
            100_u32,
            AssetId::new(ds1_asset_def.clone(), ALICE_ID.clone()),
        )
        .into(),
        Mint::asset_numeric(
            NEXUS_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), ALICE_ID.clone()),
        )
        .into(),
        Mint::asset_numeric(
            NEXUS_FEE_SEED_AMOUNT,
            AssetId::new(fee_asset_id.clone(), BOB_ID.clone()),
        )
        .into(),
        Mint::asset_numeric(200_u32, AssetId::new(ds2_asset_def, BOB_ID.clone())).into(),
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
                Mint::asset_numeric(
                    NEXUS_FEE_SEED_AMOUNT,
                    AssetId::new(fee_asset_id.clone(), validator_id.clone()),
                )
                .into(),
            );
        }
        bootstrap_tx.push(
            Mint::asset_numeric(
                VALIDATOR_STAKE,
                AssetId::new(stake_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        bootstrap_tx.push(
            Mint::asset_numeric(
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
                Numeric::from(VALIDATOR_STAKE),
                Metadata::default(),
            )
            .into(),
        );
        bootstrap_tx.push(ActivatePublicLaneValidator::new(lane_id, validator_id).into());
    }

    vec![bootstrap_tx]
}

fn wait_for_height(
    client: &Client,
    target_height: u64,
    context: &str,
) -> Result<SumeragiStatusWire> {
    wait_for_height_with_timeout(client, target_height, context, STATUS_WAIT_TIMEOUT)
}

fn wait_for_height_with_timeout(
    client: &Client,
    target_height: u64,
    context: &str,
    timeout_duration: Duration,
) -> Result<SumeragiStatusWire> {
    let started = Instant::now();
    let mut last_height = 0;
    let mut last_error: Option<String> = None;
    while started.elapsed() <= timeout_duration {
        match client.get_sumeragi_status_wire() {
            Ok(status) => {
                last_height = status.commit_qc.height;
                if status.commit_qc.height >= target_height {
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

fn wait_for_height_with_tick_timeout_across_clients(
    mut clients_factory: impl FnMut() -> Vec<Client>,
    target_height: u64,
    context: &str,
    timeout_duration: Duration,
    tick_every_polls: u64,
) -> Result<SumeragiStatusWire> {
    wait_for_height_with_tick_submitters_timeout_across_clients(
        &mut clients_factory,
        None,
        target_height,
        context,
        timeout_duration,
        tick_every_polls,
    )
}

fn wait_for_height_with_tick_submitters_timeout_across_clients(
    mut clients_factory: impl FnMut() -> Vec<Client>,
    tick_submitters: Option<&[Client]>,
    target_height: u64,
    context: &str,
    timeout_duration: Duration,
    tick_every_polls: u64,
) -> Result<SumeragiStatusWire> {
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
            match client.get_sumeragi_status_wire() {
                Ok(status) => {
                    last_height = last_height.max(status.commit_qc.height);
                    if best_status
                        .as_ref()
                        .is_none_or(|best: &SumeragiStatusWire| {
                            status.commit_qc.height >= best.commit_qc.height
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
            && status.commit_qc.height >= target_height
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
    expected_status: &SumeragiStatusWire,
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
    let expected_height = expected_status.commit_qc.height;
    let expected_hash = expected_status.commit_qc.block_hash;

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
            match peer.client().get_sumeragi_status_wire() {
                Ok(status) => {
                    let observed_height = status.commit_qc.height;
                    let observed_hash = status.commit_qc.block_hash;
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

fn asset_balance(client: &Client, asset_id: &AssetId) -> Result<Numeric> {
    match client.query_single(FindAssetById::new(asset_id.clone())) {
        Ok(asset) => Ok(asset.value().clone()),
        Err(QueryError::Validation(ValidationFail::QueryFailed(
            QueryExecutionFail::Find(FindError::Asset(_)) | QueryExecutionFail::NotFound,
        ))) => Ok(Numeric::zero()),
        Err(err) => Err(eyre!(err)),
    }
}

fn asset_balance_variants(client: &Client, asset_id: &AssetId) -> Result<Vec<Numeric>> {
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
    if let Err(err) = tick_client.submit(Log::new(Level::INFO, message)) {
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
    status: &SumeragiStatusWire,
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
    status: &SumeragiStatusWire,
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
    status: &SumeragiStatusWire,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
) -> Option<LaneDomainProgress> {
    latest_lane_domain_progress(status, lane_id, dataspace_id)
        .filter(lane_domain_progress_is_applied)
}

fn applied_lane_domain_progress(
    status: &SumeragiStatusWire,
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
    status: &SumeragiStatusWire,
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
            match client.get_sumeragi_status_wire() {
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
            match client.get_sumeragi_status_wire() {
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
            match client.get_sumeragi_status_wire() {
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
                    let lane_message_handling = status
                        .consensus_message_handling
                        .entries
                        .iter()
                        .filter(|entry| entry.kind.contains("lane_block"))
                        .map(|entry| {
                            format!(
                                "{}/{}/{}={}",
                                entry.kind, entry.outcome, entry.reason, entry.total
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
                        "peer#{index}: height={} committed=[{}] ownership=[{}] sessions=[{}] lane-msg=[{}]",
                        status.commit_qc.height,
                        committed.join(", "),
                        ownership.join(", "),
                        sessions.join(", "),
                        lane_message_handling.join(", ")
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
            match client.get_sumeragi_status_wire() {
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
    let transaction = submitter.build_transaction([instruction], Metadata::default());
    let hash = transaction.hash();
    let started = Instant::now();
    let submit_height = submitter
        .get_sumeragi_status_wire()
        .map_err(|err| eyre!(err))?
        .commit_qc
        .height;
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
            .get_sumeragi_status_wire()
            .map_err(|err| eyre!(err))?
            .commit_qc
            .height
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
    expected: Numeric,
}

struct BalanceExpectationAcrossClients<'a> {
    clients: &'a [Client],
    asset_id: &'a AssetId,
    expected: Numeric,
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

    if let Ok(status) = status_client.get_sumeragi_status_wire() {
        if let Ok(index) = usize::try_from(status.leader_index) {
            if index < peers.len() {
                let leader_height = peers[index]
                    .client()
                    .get_sumeragi_status_wire()
                    .map(|status| status.commit_qc.height)
                    .unwrap_or(0);
                if leader_height.saturating_add(1) >= status.commit_qc.height {
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
                .get_sumeragi_status_wire()
                .map(|status| status.commit_qc.height)
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
        .get_sumeragi_status_wire()
        .ok()
        .and_then(|status| usize::try_from(status.leader_index).ok());
    let mut ranked = (start..end)
        .map(|index| {
            let observed_height = peers[index]
                .client()
                .get_sumeragi_status_wire()
                .map(|status| status.commit_qc.height)
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

#[derive(Debug)]
struct CommittedSuccessOrHeightFallback {
    status: SumeragiStatusWire,
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
                .filter_map(|client| client.get_sumeragi_status_wire().ok())
                .max_by_key(|status| status.commit_qc.height)
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
                        .filter_map(|client| client.get_sumeragi_status_wire().ok())
                        .max_by_key(|status| status.commit_qc.height)
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
    run_cross_dataspace_localnet_test_on_large_stack(
        stringify!(cross_dataspace_atomic_swap_is_all_or_nothing),
        cross_dataspace_atomic_swap_is_all_or_nothing_impl,
    )
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

fn cross_dataspace_atomic_swap_is_all_or_nothing_impl() -> Result<()> {
    let context = stringify!(cross_dataspace_atomic_swap_is_all_or_nothing);
    let mut phase_timings = PhaseTimings::new(context);
    let (network, rt) = {
        let _phase = phase_timings.phase("start 12-peer localnet");
        let Some((network, rt)) =
            sandbox::start_network_blocking_or_skip(localnet_builder(), context)?
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
            .get_sumeragi_status_wire()
            .map_err(|err| eyre!(err))?
            .commit_qc
            .height;
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
            asset_balance(&nexus_alice_submitter, &alice_ds1_asset)? == Numeric::from(100_u32),
            "Alice ds1 balance query through Nexus ingress did not route to ds1"
        );
        ensure!(
            asset_balance(&nexus_bob_submitter, &bob_ds2_asset)? == Numeric::from(200_u32),
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
            .get_sumeragi_status_wire()
            .map_err(|err| eyre!(err))?
            .commit_qc
            .height;
        let ds1_pre_barrier_height = alice_on_ds1
            .get_sumeragi_status_wire()
            .map_err(|err| eyre!(err))?
            .commit_qc
            .height;
        let ds2_pre_barrier_height = alice_on_ds2
            .get_sumeragi_status_wire()
            .map_err(|err| eyre!(err))?
            .commit_qc
            .height;
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
                .get_sumeragi_status_wire()
                .map_err(|err| eyre!(err))?
                .commit_qc
                .height
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
                .get_sumeragi_status_wire()
                .map_err(|err| eyre!(err))?
                .commit_qc
                .height;
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
            expected: Numeric::from(100_u32),
        },
        BalanceExpectation {
            client: &bob_on_ds1,
            asset_id: &bob_ds1_asset,
            expected: Numeric::from(0_u32),
        },
        BalanceExpectation {
            client: &alice_on_ds2,
            asset_id: &alice_ds2_asset,
            expected: Numeric::from(0_u32),
        },
        BalanceExpectation {
            client: &bob_on_ds2,
            asset_id: &bob_ds2_asset,
            expected: Numeric::from(200_u32),
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

    {
        let successful_swap = DvpIsi::new(
            "ds1ds2swapok".parse().expect("settlement id"),
            SettlementLeg::new(
                ds1_asset_def.clone(),
                Numeric::from(30_u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            SettlementLeg::new(
                ds2_asset_def.clone(),
                Numeric::from(45_u32),
                BOB_ID.clone(),
                ALICE_ID.clone(),
            ),
            SettlementPlan::new(
                SettlementExecutionOrder::DeliveryThenPayment,
                SettlementAtomicity::AllOrNothing,
            ),
        );
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
                .get_sumeragi_status_wire()
                .map_err(|err| eyre!(err))?
                .commit_qc
                .height;
            let successful_swap_tx = submitter
                .build_transaction([InstructionBox::from(successful_swap)], Metadata::default());
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
        {
            let _phase = phase_timings.phase("execute successful swap: query/assert");
            let successful_swap_ds1_tick_submitters = [neutral_tick_submitter_a.clone()];
            let successful_swap_ds2_tick_submitters = [neutral_tick_submitter_b.clone()];
            let (ds1_progress, ds2_progress) =
                wait_for_independent_lane_application_progress_after(
                    &network,
                    &successful_swap_ds1_tick_submitters,
                    &successful_swap_ds2_tick_submitters,
                    &successful_swap_pre_application.0,
                    &successful_swap_pre_application.1,
                    "successful swap applied DS lane progress",
                )?;
            eprintln!(
                "[swap] applied DS lane progress ds1={}/{} status={} ds2={}/{} status={}",
                ds1_progress.lane_block_height,
                ds1_progress.lane_block_view,
                ds1_progress.execution_status,
                ds2_progress.lane_block_height,
                ds2_progress.lane_block_view,
                ds2_progress.execution_status,
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
            let successful_swap_expectations = [
                BalanceExpectationAcrossClients {
                    clients: &alice_ds1_balance_clients,
                    asset_id: &alice_ds1_asset,
                    expected: Numeric::from(70_u32),
                },
                BalanceExpectationAcrossClients {
                    clients: &bob_ds1_balance_clients,
                    asset_id: &bob_ds1_asset,
                    expected: Numeric::from(30_u32),
                },
                BalanceExpectationAcrossClients {
                    clients: &alice_ds2_balance_clients,
                    asset_id: &alice_ds2_asset,
                    expected: Numeric::from(45_u32),
                },
                BalanceExpectationAcrossClients {
                    clients: &bob_ds2_balance_clients,
                    asset_id: &bob_ds2_asset,
                    expected: Numeric::from(155_u32),
                },
            ];
            wait_for_expected_balances_across_clients_with_tick_submitters_timeout(
                &successful_swap_tick_submitters,
                &successful_swap_expectations,
                "successful swap balances after DS application",
                LANE_PROGRESS_WAIT_TIMEOUT,
            )?;
        }
    }
    let soak_iterations = soak_iterations();
    let mut soak_passes = 0usize;
    let mut soak_iteration_durations = Vec::with_capacity(soak_iterations);
    let mut soak_target_durations = Vec::with_capacity(soak_iterations);
    let mut soak_submit_durations = Vec::with_capacity(soak_iterations);
    let mut soak_barrier_durations = Vec::with_capacity(soak_iterations);
    let mut soak_query_durations = Vec::with_capacity(soak_iterations);
    let mut soak_failures = Vec::new();
    let mut soak_outcome_fallbacks = 0usize;
    let mut soak_iteration_retries_used = 0usize;
    {
        let _phase = phase_timings.phase(format!(
            "soak {soak_iterations} iterations: paired swap throughput"
        ));
        for iteration in 0..soak_iterations {
            let iteration_started = Instant::now();
            let mut run_result = Err(eyre!("iteration {} exceeded retry budget", iteration + 1));
            for attempt in 0..SOAK_ITERATION_ATTEMPTS {
                let attempt_result = (|| -> Result<(Duration, Duration, Duration, Duration)> {
                    let retarget_started = Instant::now();
                    let (soak_submitter, soak_bob_observer) = current_nexus_clients();
                    let target_elapsed = retarget_started.elapsed();
                    let forward_swap = DvpIsi::new(
                        format!("soakfwd{iteration}a{attempt}")
                            .parse()
                            .expect("settlement id"),
                        SettlementLeg::new(
                            ds1_asset_def.clone(),
                            Numeric::from(5_u32),
                            ALICE_ID.clone(),
                            BOB_ID.clone(),
                        ),
                        SettlementLeg::new(
                            ds2_asset_def.clone(),
                            Numeric::from(5_u32),
                            BOB_ID.clone(),
                            ALICE_ID.clone(),
                        ),
                        SettlementPlan::new(
                            SettlementExecutionOrder::DeliveryThenPayment,
                            SettlementAtomicity::AllOrNothing,
                        ),
                    );
                    let reverse_swap = DvpIsi::new(
                        format!("soakrev{iteration}a{attempt}")
                            .parse()
                            .expect("settlement id"),
                        SettlementLeg::new(
                            ds2_asset_def.clone(),
                            Numeric::from(5_u32),
                            ALICE_ID.clone(),
                            BOB_ID.clone(),
                        ),
                        SettlementLeg::new(
                            ds1_asset_def.clone(),
                            Numeric::from(5_u32),
                            BOB_ID.clone(),
                            ALICE_ID.clone(),
                        ),
                        SettlementPlan::new(
                            SettlementExecutionOrder::DeliveryThenPayment,
                            SettlementAtomicity::AllOrNothing,
                        ),
                    );
                    let submit_started = Instant::now();
                    let soak_swap_tx = soak_submitter.build_transaction(
                        vec![
                            InstructionBox::from(forward_swap),
                            InstructionBox::from(reverse_swap),
                        ],
                        Metadata::default(),
                    );
                    let soak_swap_entry_hash = soak_swap_tx.hash_as_entrypoint();
                    let pre_barrier_height = soak_submitter
                        .get_sumeragi_status_wire()
                        .map_err(|err| eyre!(err))?
                        .commit_qc
                        .height;
                    submit_transaction_across_clients(
                        || {
                            lane_targeted_clients_for_lane(
                                &network,
                                &alice,
                                &ALICE_ID,
                                ALICE_KEYPAIR.private_key(),
                                NEXUS_LANE_INDEX,
                            )
                        },
                        &soak_swap_tx,
                        "soak paired swaps enqueue on Nexus authoritative observers",
                        SUBMIT_ENQUEUE_REQUEST_TIMEOUT,
                    )?;
                    let submit_elapsed = submit_started.elapsed();
                    let barrier_started = Instant::now();
                    let _synced_after_paired_swaps = match wait_for_committed_success_across_clients(
                        || {
                            lane_targeted_clients_for_lane(
                                &network,
                                &alice,
                                &ALICE_ID,
                                ALICE_KEYPAIR.private_key(),
                                NEXUS_LANE_INDEX,
                            )
                        },
                        soak_swap_entry_hash.clone(),
                        "soak paired swaps confirmation on Nexus authoritative observer",
                        SOAK_COMMITTED_OUTCOME_TIMEOUT,
                    ) {
                        Ok(()) => soak_submitter
                            .get_sumeragi_status_wire()
                            .map_err(|err| eyre!(err))?,
                        Err(err) => {
                            let error_text = err.to_string();
                            if !is_inconclusive_committed_outcome_error(&error_text) {
                                return Err(err);
                            }
                            soak_outcome_fallbacks = soak_outcome_fallbacks.saturating_add(1);
                            if soak_outcome_fallbacks <= SOAK_FALLBACK_LOG_LIMIT {
                                eprintln!(
                                    "[soak] committed outcome inconclusive; falling back to height barrier"
                                );
                            }
                            match wait_for_height_with_tick_timeout_across_clients(
                                || {
                                    lane_targeted_clients_for_lane(
                                        &network,
                                        &alice,
                                        &ALICE_ID,
                                        ALICE_KEYPAIR.private_key(),
                                        NEXUS_LANE_INDEX,
                                    )
                                },
                                pre_barrier_height.saturating_add(1),
                                "soak paired swaps barrier on Nexus authoritative observer (height fallback)",
                                SOAK_PHASE_WAIT_TIMEOUT,
                                SOAK_BARRIER_TICK_EVERY_POLLS,
                            ) {
                                Ok(status) => status,
                                Err(height_err) => match wait_for_committed_success_across_clients(
                                    || {
                                        lane_targeted_clients_for_lane(
                                            &network,
                                            &alice,
                                            &ALICE_ID,
                                            ALICE_KEYPAIR.private_key(),
                                            NEXUS_LANE_INDEX,
                                        )
                                    },
                                    soak_swap_entry_hash.clone(),
                                    "soak paired swaps confirmation on Nexus authoritative observer (post-barrier-timeout)",
                                    SOAK_PHASE_WAIT_TIMEOUT,
                                ) {
                                    Ok(()) => soak_submitter
                                        .get_sumeragi_status_wire()
                                        .map_err(|err| eyre!(err))?,
                                    Err(outcome_err) => {
                                        let error_text = outcome_err.to_string();
                                        if !is_inconclusive_committed_outcome_error(&error_text) {
                                            return Err(outcome_err);
                                        }
                                        return Err(height_err);
                                    }
                                },
                            }
                        }
                    };
                    let barrier_elapsed = barrier_started.elapsed();
                    let query_started = Instant::now();
                    let soak_baseline = [
                        BalanceExpectation {
                            client: &soak_submitter,
                            asset_id: &alice_ds1_asset,
                            expected: Numeric::from(70_u32),
                        },
                        BalanceExpectation {
                            client: &soak_bob_observer,
                            asset_id: &bob_ds1_asset,
                            expected: Numeric::from(30_u32),
                        },
                        BalanceExpectation {
                            client: &soak_submitter,
                            asset_id: &alice_ds2_asset,
                            expected: Numeric::from(45_u32),
                        },
                        BalanceExpectation {
                            client: &soak_bob_observer,
                            asset_id: &bob_ds2_asset,
                            expected: Numeric::from(155_u32),
                        },
                    ];
                    wait_for_expected_balances_with_timeout(
                        &soak_baseline,
                        "soak iteration net-zero balances",
                        SOAK_PHASE_WAIT_TIMEOUT,
                    )?;
                    let query_elapsed = query_started.elapsed();
                    Ok((
                        target_elapsed,
                        submit_elapsed,
                        barrier_elapsed,
                        query_elapsed,
                    ))
                })();

                match attempt_result {
                    Ok(metrics) => {
                        run_result = Ok(metrics);
                        break;
                    }
                    Err(err) => {
                        if attempt + 1 == SOAK_ITERATION_ATTEMPTS {
                            run_result = Err(err);
                            break;
                        }
                        soak_iteration_retries_used = soak_iteration_retries_used.saturating_add(1);
                        eprintln!(
                            "[soak] iteration {} attempt {} failed; retrying: {err}",
                            iteration + 1,
                            attempt + 1
                        );
                    }
                }
            }

            match run_result {
                Ok((target_elapsed, submit_elapsed, barrier_elapsed, query_elapsed)) => {
                    soak_passes += 1;
                    soak_iteration_durations.push(iteration_started.elapsed());
                    soak_target_durations.push(target_elapsed);
                    soak_submit_durations.push(submit_elapsed);
                    soak_barrier_durations.push(barrier_elapsed);
                    soak_query_durations.push(query_elapsed);
                }
                Err(err) => {
                    soak_failures.push(format!("iteration {} failed: {err}", iteration + 1));
                }
            }
        }
    }
    if soak_outcome_fallbacks > 0 {
        eprintln!(
            "[soak] committed-outcome fallback count = {}",
            soak_outcome_fallbacks
        );
    }
    if soak_iteration_retries_used > 0 {
        eprintln!(
            "[soak] iteration retries used = {}",
            soak_iteration_retries_used
        );
    }
    if let Some((min, avg, max)) = duration_min_avg_max_secs(&soak_iteration_durations) {
        let pass_rate = (soak_passes as f64 / soak_iterations as f64) * 100.0;
        eprintln!("[soak] strict metrics (gating enabled)");
        eprintln!(
            "[soak] iterations={} pass_rate={:.1}% min={:.3}s avg={:.3}s max={:.3}s",
            soak_iterations, pass_rate, min, avg, max
        );
        if let Some((target_min, target_avg, target_max)) =
            duration_min_avg_max_secs(&soak_target_durations)
        {
            eprintln!(
                "[soak] per-iter target-refresh min/avg/max = {:.3}s/{:.3}s/{:.3}s",
                target_min, target_avg, target_max
            );
        }
        if let Some((submit_min, submit_avg, submit_max)) =
            duration_min_avg_max_secs(&soak_submit_durations)
        {
            eprintln!(
                "[soak] per-iter submit min/avg/max = {:.3}s/{:.3}s/{:.3}s",
                submit_min, submit_avg, submit_max
            );
        }
        if let Some((barrier_min, barrier_avg, barrier_max)) =
            duration_min_avg_max_secs(&soak_barrier_durations)
        {
            eprintln!(
                "[soak] per-iter barrier min/avg/max = {:.3}s/{:.3}s/{:.3}s",
                barrier_min, barrier_avg, barrier_max
            );
        }
        if let Some((query_min, query_avg, query_max)) =
            duration_min_avg_max_secs(&soak_query_durations)
        {
            eprintln!(
                "[soak] per-iter query min/avg/max = {:.3}s/{:.3}s/{:.3}s",
                query_min, query_avg, query_max
            );
        }
    }
    if !soak_failures.is_empty() {
        eprintln!("[soak] failed iterations: {}", soak_failures.len());
        for failure in soak_failures.iter().take(3) {
            eprintln!("[soak] failure detail: {failure}");
        }
    }
    let soak_gate_metrics = SoakGateMetrics {
        iterations: soak_iterations,
        passes: soak_passes,
        failures: soak_failures.len(),
        retries_used: soak_iteration_retries_used,
    };
    eprintln!(
        "[soak] gate requires at least {} passes and at most {} retries",
        soak_gate_minimum_passes(soak_iterations),
        soak_gate_maximum_retries(soak_iterations)
    );
    // Evaluate retries even when every iteration eventually passes. Otherwise the inner retry
    // loop can turn repeated transient consensus failures into a silent green PR result.
    validate_soak_gate(soak_gate_metrics)
        .map_err(|failure| eyre!("cross-dataspace soak quality gate failed: {failure}"))?;
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
                    Numeric::from(10_u32),
                    ALICE_ID.clone(),
                    BOB_ID.clone(),
                ),
                SettlementLeg::new(
                    ds2_asset_def.clone(),
                    Numeric::from(10_000_u32),
                    BOB_ID.clone(),
                    ALICE_ID.clone(),
                ),
                SettlementPlan::new(
                    SettlementExecutionOrder::DeliveryThenPayment,
                    SettlementAtomicity::AllOrNothing,
                ),
            );
            let (submitter, _) = current_nexus_clients();
            let failing_swap_tx = submitter
                .build_transaction([InstructionBox::from(failing_swap)], Metadata::default());
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
        let rollback_baseline = [
            BalanceExpectation {
                client: &rollback_alice_on_nexus,
                asset_id: &alice_ds1_asset,
                expected: Numeric::from(70_u32),
            },
            BalanceExpectation {
                client: &rollback_bob_on_nexus,
                asset_id: &bob_ds1_asset,
                expected: Numeric::from(30_u32),
            },
            BalanceExpectation {
                client: &rollback_alice_on_nexus,
                asset_id: &alice_ds2_asset,
                expected: Numeric::from(45_u32),
            },
            BalanceExpectation {
                client: &rollback_bob_on_nexus,
                asset_id: &bob_ds2_asset,
                expected: Numeric::from(155_u32),
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
        "[health] soak_passed={}/{} setup_retries={} swap_fallbacks={} swap_nonconverged_fallbacks={} soak_fallbacks={} soak_retries={}",
        soak_passes,
        soak_iterations,
        setup_register_mint_retries_used,
        swap_outcome_fallbacks,
        swap_nonconverged_fallbacks,
        soak_outcome_fallbacks,
        soak_iteration_retries_used
    );

    phase_timings.emit_summary();

    Ok(())
}

#[test]
fn cross_dataspace_localnet_genesis_preexecution_smoke() {
    // Build-only smoke test keeps genesis pre-execution coverage cheap and deterministic.
    let _guard = sandbox::serial_guard();
    let _network = localnet_builder().build();
}

#[cfg(test)]
mod tests {
    use super::{
        ALICE_ID, AccountId, Algorithm, CommittedTxOutcome, DS1_ID_U64, DS1_LANE_INDEX,
        DS1_MANIFEST_HASH, DS2_ID_U64, DS2_LANE_INDEX, DS2_MANIFEST_HASH,
        ExpectedLaneValidatorBinding, KeyPair, LaneDomainProgress, LanePayloadOwnershipProgress,
        NEXUS_ALIAS, NEXUS_ID_U64, NEXUS_LANE_INDEX, OBSERVER_QUERY_TIMEOUT_CAP, PeerId,
        RoutedJsonGetResponse, SoakGateFailure, SoakGateMetrics, TOTAL_PEERS, VALIDATORS_PER_LANE,
        applied_lane_domain_progress, bounded_observer_request_timeout,
        committed_lane_block_has_expected_quorum, committed_tx_outcome_quorum,
        cross_dataspace_gas_account_id, duration_min_avg_max_secs,
        expect_local_or_proxy_fanout_headers, expected_lane_binding_for_peer,
        is_expected_rollback_failure_text, is_inconclusive_blocking_submit_error,
        is_inconclusive_committed_outcome_error, lane_domain_progress_is_after_baseline,
        lane_validator_snapshot, latest_lane_domain_application_progress,
        latest_lane_domain_progress, latest_lane_payload_ownership_progress,
        multilane_da_proof_policy_bundle, nexus_fee_asset_definition_id,
        npos_multilane_genesis_post_topology_transactions, parse_positive_usize_override,
        peer_indices_for_committed_lane_evidence, quorum_lane_domain_progress,
        quorum_lane_payload_ownership_progress, render_error_with_debug, render_rejection_reason,
        routed_header_string, should_submit_tick, stake_asset_definition_id,
        stake_asset_id_literal, total_balance_observer_request_slots, validate_soak_gate,
        validator_authority_account_for_peer, validator_authority_seed,
    };
    use iroha::crypto::{Hash, HashOf};
    use iroha::data_model::{
        block::consensus::{
            COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK,
            COMMITTED_LANE_STATUS_STATE_APPLIED_BY_DIRECT_EXECUTION, SumeragiCommittedLaneBlock,
            SumeragiDataspaceCommitment, SumeragiLanePayloadOwnership, SumeragiStatusWire,
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
        panic,
        time::{Duration, Instant},
    };

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
    fn parse_positive_usize_override_uses_positive_input() {
        assert_eq!(parse_positive_usize_override(Some("12"), 10), 12);
        assert_eq!(parse_positive_usize_override(Some(" 7 "), 10), 7);
    }

    #[test]
    fn parse_positive_usize_override_falls_back_on_invalid_input() {
        assert_eq!(parse_positive_usize_override(None, 10), 10);
        assert_eq!(parse_positive_usize_override(Some("0"), 10), 10);
        assert_eq!(parse_positive_usize_override(Some("-1"), 10), 10);
        assert_eq!(parse_positive_usize_override(Some("1.5"), 10), 10);
        assert_eq!(parse_positive_usize_override(Some("not-a-number"), 10), 10);
        assert_eq!(parse_positive_usize_override(Some(""), 10), 10);
    }

    #[test]
    fn soak_gate_accepts_exact_pass_and_retry_boundaries() {
        assert_eq!(
            validate_soak_gate(SoakGateMetrics {
                iterations: 10,
                passes: 9,
                failures: 1,
                retries_used: 2,
            }),
            Ok(())
        );
    }

    #[test]
    fn soak_gate_rejects_pass_rate_below_boundary() {
        assert_eq!(
            validate_soak_gate(SoakGateMetrics {
                iterations: 10,
                passes: 8,
                failures: 2,
                retries_used: 0,
            }),
            Err(SoakGateFailure::PassRateBelowMinimum {
                iterations: 10,
                passes: 8,
                minimum_passes: 9,
            })
        );
    }

    #[test]
    fn soak_gate_rejects_excess_retries_even_when_every_iteration_passes() {
        assert_eq!(
            validate_soak_gate(SoakGateMetrics {
                iterations: 10,
                passes: 10,
                failures: 0,
                retries_used: 3,
            }),
            Err(SoakGateFailure::RetryBudgetExceeded {
                iterations: 10,
                retries_used: 3,
                maximum_retries: 2,
            })
        );
    }

    #[test]
    fn soak_gate_rejects_inconsistent_or_empty_metrics() {
        assert_eq!(
            validate_soak_gate(SoakGateMetrics {
                iterations: 10,
                passes: 9,
                failures: 0,
                retries_used: 0,
            }),
            Err(SoakGateFailure::AccountingMismatch {
                iterations: 10,
                passes: 9,
                failures: 0,
            })
        );
        assert_eq!(
            validate_soak_gate(SoakGateMetrics {
                iterations: 0,
                passes: 0,
                failures: 0,
                retries_used: 0,
            }),
            Err(SoakGateFailure::NoIterations)
        );
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
        let status = SumeragiStatusWire {
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
            ..Default::default()
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
        let status = SumeragiStatusWire {
            committed_lane_blocks: vec![applied_lower.clone(), unapplied_latest],
            ..Default::default()
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
        let direct_applied_status = SumeragiStatusWire {
            committed_lane_blocks: vec![applied_lower.clone(), direct_applied_latest],
            ..Default::default()
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
        let unavailable_status = SumeragiStatusWire {
            committed_lane_blocks: vec![unavailable_applied],
            ..Default::default()
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
        let status = SumeragiStatusWire {
            committed_lane_blocks: vec![applied_lower.clone(), pending_latest, applied_top.clone()],
            ..Default::default()
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
        let ambiguous_status = SumeragiStatusWire {
            committed_lane_blocks: vec![applied_lower, applied_top, conflicting_top],
            ..Default::default()
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
        let status = SumeragiStatusWire {
            committed_lane_blocks: vec![valid.clone(), exact_duplicate],
            ..Default::default()
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

        let ambiguous = SumeragiStatusWire {
            committed_lane_blocks: vec![valid, proposal_drift],
            ..Default::default()
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
        let status = SumeragiStatusWire {
            committed_lane_blocks: vec![valid, malformed_latest],
            ..Default::default()
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
        let status = SumeragiStatusWire {
            lane_payload_ownerships: vec![
                wrong_dataspace,
                under_quorum,
                empty_work,
                forged_subject,
                valid,
            ],
            ..Default::default()
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
        let status = SumeragiStatusWire {
            lane_payload_ownerships: vec![valid, malformed_latest],
            ..Default::default()
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

        let duplicate_status = SumeragiStatusWire {
            lane_payload_ownerships: vec![valid.clone(), exact_duplicate],
            ..Default::default()
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

        let ambiguous_status = SumeragiStatusWire {
            lane_payload_ownerships: vec![valid, conflicting],
            ..Default::default()
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
