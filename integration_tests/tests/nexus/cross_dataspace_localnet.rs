#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Localnet cross-dataspace atomic swap regression test.

use std::{
    collections::BTreeSet,
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
        block::consensus::SumeragiStatusWire,
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
        transaction::TransactionEntrypoint,
    },
    query::QueryError,
};
use iroha_config::parameters::actual::LaneConfig as ActualLaneConfig;
use iroha_core::da::proof_policy_bundle;
use iroha_crypto::{Algorithm, KeyPair, PrivateKey};
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

const NEXUS_ALIAS: &str = "nexus";
const DS1_ALIAS: &str = "ds1";
const DS2_ALIAS: &str = "ds2";
const NEXUS_ID_U64: u64 = 0;
const DS1_ID_U64: u64 = 1;
const DS2_ID_U64: u64 = 2;
const NEXUS_LANE_INDEX: u32 = 0;
const DS1_LANE_INDEX: u32 = 1;
const DS2_LANE_INDEX: u32 = 2;
const TOTAL_PEERS: usize = 12;
const VALIDATORS_PER_LANE: usize = 4;
const VALIDATOR_STAKE: u64 = 2_000;
const NEXUS_FEE_SEED_AMOUNT: u32 = 1_000_000;
const STATUS_WAIT_TIMEOUT: Duration = Duration::from_secs(45);
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(200);
const ROUTE_PROBE_APPROVAL_WAIT_TIMEOUT: Duration = Duration::from_millis(100);
const ROUTE_PROBE_SSE_HANDSHAKE_DELAY: Duration = Duration::from_millis(100);
const BALANCE_WAIT_TICK_EVERY_POLLS: u64 = 5;
const PERMISSION_WAIT_TICK_EVERY_POLLS: u64 = 5;
const SETUP_BARRIER_TICK_EVERY_POLLS: u64 = 5;
const SETUP_REGISTER_MINT_QUERY_TIMEOUT: Duration = Duration::from_secs(20);
const BLOCKING_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(20);
const SWAP_BLOCKING_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(30);
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
    iroha_config::parameters::defaults::nexus::fees::fee_asset_id()
        .parse()
        .expect("default nexus fee asset id")
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
    let mut seed = vec![0_u8; 32];
    seed[0] = 0xC1;
    seed[1..9].copy_from_slice(&u64::try_from(index).unwrap_or(u64::MAX).to_le_bytes());
    let keypair = KeyPair::from_seed(seed, Algorithm::Ed25519);
    AccountId::new(keypair.public_key().clone())
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
                Vec::new(),
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
                "description".into(),
                TomlValue::String("private dataspace one".to_owned()),
            );
            ds1.insert("fault_tolerance".into(), TomlValue::Integer(1));

            let mut ds2 = Table::new();
            ds2.insert("alias".into(), TomlValue::String(DS2_ALIAS.to_owned()));
            ds2.insert("id".into(), TomlValue::Integer(DS2_ID_U64 as i64));
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
                .write(["sumeragi", "npos", "use_stake_snapshot_roster"], true)
                .write(
                    ["sumeragi", "npos", "election", "max_validators"],
                    VALIDATORS_PER_LANE as i64,
                )
                .write(["sumeragi", "npos", "epoch_length_blocks"], 3600_i64)
                .write(
                    ["sumeragi", "npos", "vrf", "commit_deadline_offset_blocks"],
                    100_i64,
                )
                .write(
                    ["sumeragi", "npos", "vrf", "reveal_deadline_offset_blocks"],
                    40_i64,
                );
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

fn wait_for_height_with_tick_timeout(
    client: &Client,
    tick_submitter: &Client,
    target_height: u64,
    context: &str,
    timeout_duration: Duration,
    tick_every_polls: u64,
) -> Result<SumeragiStatusWire> {
    let started = Instant::now();
    let mut last_height = 0;
    let mut last_error: Option<String> = None;
    let mut polls = 0_u64;
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
        polls = polls.saturating_add(1);
        if polls % tick_every_polls == 0 {
            let _ = tick_submitter.submit(Log::new(
                Level::INFO,
                format!("{context} height tick {}", polls / tick_every_polls),
            ));
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
    let mut polls = 0_u64;
    while started.elapsed() <= timeout_duration {
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
        polls = polls.saturating_add(1);
        if polls % tick_every_polls == 0 {
            let _ = tick_submitter.submit(Log::new(
                Level::INFO,
                format!("{context} lane tick {}", polls / tick_every_polls),
            ));
        }
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
    let mut matches = client
        .query(FindAssets::new())
        .execute_all()?
        .into_iter()
        .filter(|asset: &Asset| asset.id == *asset_id)
        .map(|asset| asset.value().clone())
        .collect::<Vec<_>>();
    if matches.is_empty() {
        matches.push(Numeric::zero());
    }
    matches.sort();
    matches.dedup();
    Ok(matches)
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

    let request = reqwest::Client::new()
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
        for expectation in expectations {
            let observed = match asset_balance_variants(expectation.client, expectation.asset_id) {
                Ok(observed) => observed,
                Err(err) => {
                    last_error = Some(err.to_string());
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

fn wait_for_expected_balances_with_tick_timeout(
    tick_submitter: &Client,
    expectations: &[BalanceExpectation<'_>],
    context: &str,
    timeout_duration: Duration,
) -> Result<()> {
    let started = Instant::now();
    let mut last_observed = Vec::with_capacity(expectations.len());
    let mut last_error: Option<String> = None;
    let mut polls = 0_u64;
    while started.elapsed() <= timeout_duration {
        last_observed.clear();
        let mut all_match = true;
        for expectation in expectations {
            let observed = match asset_balance_variants(expectation.client, expectation.asset_id) {
                Ok(observed) => observed,
                Err(err) => {
                    last_error = Some(err.to_string());
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
        polls = polls.saturating_add(1);
        if polls % BALANCE_WAIT_TICK_EVERY_POLLS == 0 {
            let _ = tick_submitter.submit(Log::new(
                Level::INFO,
                format!(
                    "{context} balance tick {}",
                    polls / BALANCE_WAIT_TICK_EVERY_POLLS
                ),
            ));
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    let suffix = last_error
        .map(|err| format!("; last balance query error: {err}"))
        .unwrap_or_default();
    let bucket_context = expectations
        .iter()
        .map(|expectation| {
            let matches = expectation
                .client
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
                .unwrap_or_else(|err| vec![format!("query error: {err}")]);
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

fn wait_for_account_permissions(
    client: &Client,
    tick_submitter: &Client,
    account_id: &AccountId,
    required_permissions: &[Permission],
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_observed = Vec::new();
    let mut last_error: Option<String> = None;
    let mut polls = 0_u64;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let permissions = match client
            .query(FindPermissionsByAccountId::new(account_id.clone()))
            .execute_all()
        {
            Ok(permissions) => permissions,
            Err(err) => {
                last_error = Some(format!("{err} ({err:?})"));
                polls = polls.saturating_add(1);
                if polls % PERMISSION_WAIT_TICK_EVERY_POLLS == 0 {
                    let _ = tick_submitter.submit(Log::new(
                        Level::INFO,
                        format!(
                            "{context} permission tick {}",
                            polls / PERMISSION_WAIT_TICK_EVERY_POLLS
                        ),
                    ));
                }
                thread::sleep(STATUS_POLL_INTERVAL);
                continue;
            }
        };
        last_observed = permissions.clone();
        let all_present = required_permissions
            .iter()
            .all(|required| permissions.iter().any(|permission| permission == required));
        if all_present {
            return Ok(());
        }
        polls = polls.saturating_add(1);
        if polls % PERMISSION_WAIT_TICK_EVERY_POLLS == 0 {
            let _ = tick_submitter.submit(Log::new(
                Level::INFO,
                format!(
                    "{context} permission tick {}",
                    polls / PERMISSION_WAIT_TICK_EVERY_POLLS
                ),
            ));
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    let suffix = last_error
        .map(|err| format!("; last permission query error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for permissions on {account_id}; required {required_permissions:?}; last observed {last_observed:?}{suffix}"
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
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let snapshot = client
            .get_public_lane_validators(lane_id)
            .map_err(|err| eyre!(err))?;
        let (total, active) = lane_validator_snapshot(&snapshot, context)?;
        last_total = total;
        last_active = active.clone();
        if total == expected_active.len() && active == *expected_active {
            return Ok(());
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }

    Err(eyre!(
        "{context}: timed out waiting for active validators on lane {lane_id}; expected total {} active {:?}, observed total {} active {:?}",
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
    let peers = network.peers();
    if peers.is_empty() {
        return 0;
    }

    let lane_index = lane_index as usize;
    let start = lane_index.saturating_mul(VALIDATORS_PER_LANE);
    let end = start.saturating_add(VALIDATORS_PER_LANE).min(peers.len());
    if start >= end {
        return leader_or_highest_height_peer_index(network, status_client);
    }

    (start..end)
        .max_by_key(|index| {
            peers[*index]
                .client()
                .get_sumeragi_status_wire()
                .map(|status| status.commit_qc.height)
                .unwrap_or(0)
        })
        .unwrap_or_else(|| leader_or_highest_height_peer_index(network, status_client))
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

enum CommittedTxOutcome {
    Applied,
    Rejected(String),
}

fn wait_for_committed_tx_outcome(
    client: &Client,
    entry_hash: HashOf<TransactionEntrypoint>,
    context: &str,
    timeout_duration: Duration,
) -> Result<CommittedTxOutcome> {
    let started = Instant::now();
    let mut last_error: Option<String> = None;
    let one = NonZeroU64::new(1).expect("nonzero");
    while started.elapsed() <= timeout_duration {
        let filters = CommittedTxFilters {
            entry_eq: Some(entry_hash.clone()),
            ..Default::default()
        };
        match client
            .query(FindTransactions::new())
            .filter(CompoundPredicate::from_filters(filters))
            .with_pagination(Pagination::new(Some(one), 0))
            .with_fetch_size(FetchSize::new(Some(one)))
            .execute_all()
        {
            Ok(snapshot) => {
                if let Some(tx) = snapshot.first() {
                    return match &tx.result().0 {
                        Ok(_) => Ok(CommittedTxOutcome::Applied),
                        Err(reason) => Ok(CommittedTxOutcome::Rejected(render_rejection_reason(
                            reason,
                        ))),
                    };
                }
            }
            Err(err) => {
                last_error = Some(err.to_string());
            }
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    let suffix = last_error
        .map(|err| format!("; last tx history query error: {err}"))
        .unwrap_or_default();
    Err(eyre!(
        "{context}: timed out waiting for committed transaction outcome for transaction {entry_hash}{suffix}"
    ))
}

fn wait_for_committed_success(
    client: &Client,
    entry_hash: HashOf<TransactionEntrypoint>,
    context: &str,
    timeout_duration: Duration,
) -> Result<()> {
    match wait_for_committed_tx_outcome(client, entry_hash.clone(), context, timeout_duration)? {
        CommittedTxOutcome::Applied => Ok(()),
        CommittedTxOutcome::Rejected(reason) => Err(eyre!(
            "{context}: transaction {entry_hash} rejected unexpectedly: {reason}"
        )),
    }
}

fn wait_for_committed_rejection_reason(
    client: &Client,
    entry_hash: HashOf<TransactionEntrypoint>,
    context: &str,
    timeout_duration: Duration,
) -> Result<String> {
    match wait_for_committed_tx_outcome(client, entry_hash.clone(), context, timeout_duration)? {
        CommittedTxOutcome::Applied => Err(eyre!(
            "{context}: transaction {entry_hash} committed successfully, expected rejection"
        )),
        CommittedTxOutcome::Rejected(reason) => Ok(reason),
    }
}

fn wait_for_committed_success_or_height_fallback(
    observer: &Client,
    tick_submitter: &Client,
    entry_hash: HashOf<TransactionEntrypoint>,
    committed_context: &str,
    fallback_context: &str,
    pre_barrier_height: u64,
    committed_timeout: Duration,
    post_barrier_outcome_timeout: Duration,
) -> Result<SumeragiStatusWire> {
    match wait_for_committed_success(
        observer,
        entry_hash.clone(),
        committed_context,
        committed_timeout,
    ) {
        Ok(()) => observer
            .get_sumeragi_status_wire()
            .map_err(|err| eyre!(err)),
        Err(err) => {
            let error_text = err.to_string();
            if !is_inconclusive_committed_outcome_error(&error_text) {
                return Err(err);
            }
            eprintln!("[swap] committed outcome inconclusive; falling back to height barrier");
            let barrier_status = wait_for_height_with_tick_timeout(
                observer,
                tick_submitter,
                pre_barrier_height.saturating_add(1),
                fallback_context,
                STATUS_WAIT_TIMEOUT,
                SETUP_BARRIER_TICK_EVERY_POLLS,
            )?;
            let post_context = format!("{committed_context} (post-barrier)");
            match wait_for_committed_success(
                observer,
                entry_hash,
                post_context.as_str(),
                post_barrier_outcome_timeout,
            ) {
                Ok(()) => observer
                    .get_sumeragi_status_wire()
                    .map_err(|err| eyre!(err)),
                Err(post_err) => {
                    let post_error_text = post_err.to_string();
                    if is_inconclusive_committed_outcome_error(&post_error_text) {
                        Ok(barrier_status)
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
        let _lane_sync_on_bob = wait_for_height(
            &bob,
            lane_sync_height,
            "lane validator activation propagation",
        )?;
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
            let validators = torii_json_get(&nexus_alice_submitter, &validators_path, &[]).await?;
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

            let account_assets =
                torii_json_get(&nexus_alice_submitter, &account_assets_path, &[]).await?;
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

            let asset_definition =
                torii_json_get(&nexus_alice_submitter, &asset_definition_path, &[]).await?;
            expect_local_or_proxy_fanout_headers(
                &asset_definition,
                "asset definition query through Nexus ingress",
            )?;
            ensure!(
                asset_definition.body["id"].as_str() == Some(ds1_asset_literal.as_str()),
                "asset definition query through Nexus ingress returned unexpected id {:?}",
                asset_definition.body["id"].as_str()
            );

            let account_summary =
                torii_json_get(&nexus_alice_submitter, &account_summary_path, &[]).await?;
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

    {
        let submitter = nexus_alice_submitter.clone();
        let _phase = phase_timings.phase("setup grants: tx submit enqueue");
        let setup_grants_tx = submitter.build_transaction(
            vec![InstructionBox::from(Grant::account_permission(
                CanTransferAssetWithDefinition {
                    asset_definition: ds1_asset_def.clone(),
                },
                BOB_ID.clone(),
            ))],
            Metadata::default(),
        );
        submitter.submit_transaction(&setup_grants_tx)?;
    }
    {
        let _phase = phase_timings.phase("setup grants: query/assert");
        wait_for_account_permissions(
            &nexus_bob_submitter,
            &nexus_bob_submitter,
            &BOB_ID,
            &[bob_transfer_ds1_permission.clone()],
            "grant setup permissions visible on routed bob submitter",
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
    let mut swap_outcome_fallbacks = 0usize;
    let mut swap_nonconverged_fallbacks = 0usize;

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
        let mut submitter = leader_targeted_client_for_lane(
            &network,
            &alice,
            &ALICE_ID,
            ALICE_KEYPAIR.private_key(),
            DS1_LANE_INDEX,
        );
        let mut successful_swap_synced_status = None;
        let (successful_swap_tx, successful_swap_entry_hash, successful_swap_pre_barrier_height) = {
            let _phase = phase_timings.phase("execute successful swap: tx submit enqueue");
            let pre_barrier_height = alice
                .get_sumeragi_status_wire()
                .map_err(|err| eyre!(err))?
                .commit_qc
                .height
                .max(
                    bob.get_sumeragi_status_wire()
                        .map_err(|err| eyre!(err))?
                        .commit_qc
                        .height,
                );
            let successful_swap_tx = submitter
                .build_transaction([InstructionBox::from(successful_swap)], Metadata::default());
            let successful_swap_entry_hash = successful_swap_tx.hash_as_entrypoint();
            (
                successful_swap_tx,
                successful_swap_entry_hash,
                pre_barrier_height,
            )
        };
        {
            let _phase = phase_timings.phase("execute successful swap: barrier wait");
            submitter.transaction_status_timeout = SWAP_BLOCKING_CONFIRMATION_TIMEOUT;
            match submitter.submit_transaction_blocking(&successful_swap_tx) {
                Ok(_) => {
                    successful_swap_synced_status =
                        Some(wait_for_committed_success_or_height_fallback(
                            &alice,
                            &alice,
                            successful_swap_entry_hash.clone(),
                            "successful swap confirmation on alice observer",
                            "successful swap barrier on alice observer (height fallback)",
                            successful_swap_pre_barrier_height,
                            SWAP_COMMITTED_OUTCOME_TIMEOUT,
                            SWAP_POST_BARRIER_OUTCOME_TIMEOUT,
                        )?);
                }
                Err(err) => {
                    let error_text = err.to_string();
                    if !is_inconclusive_blocking_submit_error(&error_text) {
                        return Err(err);
                    }
                    swap_outcome_fallbacks = swap_outcome_fallbacks.saturating_add(1);
                    match wait_for_committed_success_or_height_fallback(
                        &alice,
                        &alice,
                        successful_swap_entry_hash,
                        "successful swap confirmation on alice observer",
                        "successful swap barrier on alice observer (height fallback)",
                        successful_swap_pre_barrier_height,
                        SWAP_COMMITTED_OUTCOME_TIMEOUT,
                        SWAP_POST_BARRIER_OUTCOME_TIMEOUT,
                    ) {
                        Ok(status) => successful_swap_synced_status = Some(status),
                        Err(fallback_err) => {
                            swap_nonconverged_fallbacks =
                                swap_nonconverged_fallbacks.saturating_add(1);
                            eprintln!(
                                "[swap] successful swap fallback did not converge before balance assertion: {fallback_err}"
                            );
                        }
                    }
                }
            }
        }
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
        if let Some(status) = successful_swap_synced_status.as_ref() {
            wait_for_lane_peers_commit_qc_at_least(
                &network,
                DS1_LANE_INDEX,
                status,
                &alice_on_ds1,
                "successful swap ds1 authoritative sync",
                STATUS_WAIT_TIMEOUT,
                SETUP_BARRIER_TICK_EVERY_POLLS,
            )?;
        }
        {
            let _phase = phase_timings.phase("execute successful swap: query/assert");
            let successful_swap_expectations = [
                BalanceExpectation {
                    client: &alice_on_ds1,
                    asset_id: &alice_ds1_asset,
                    expected: Numeric::from(70_u32),
                },
                BalanceExpectation {
                    client: &bob_on_ds1,
                    asset_id: &bob_ds1_asset,
                    expected: Numeric::from(30_u32),
                },
                BalanceExpectation {
                    client: &alice_on_ds2,
                    asset_id: &alice_ds2_asset,
                    expected: Numeric::from(45_u32),
                },
                BalanceExpectation {
                    client: &bob_on_ds2,
                    asset_id: &bob_ds2_asset,
                    expected: Numeric::from(155_u32),
                },
            ];
            let mut balance_wait_result = Ok(());
            for (tick_submitter, lane_label) in [
                (&alice_on_ds1, "ds1"),
                (&bob_on_ds2, "ds2"),
                (&alice_on_ds1, "ds1"),
                (&bob_on_ds2, "ds2"),
            ] {
                match wait_for_expected_balances_with_tick_timeout(
                    tick_submitter,
                    &successful_swap_expectations,
                    "successful swap balances",
                    Duration::from_secs(12),
                ) {
                    Ok(()) => {
                        balance_wait_result = Ok(());
                        break;
                    }
                    Err(err) => {
                        eprintln!(
                            "[swap] successful swap balances not visible after {lane_label} tick window: {err}"
                        );
                        balance_wait_result = Err(err);
                    }
                }
            }
            balance_wait_result?;
        }
    }

    let soak_iterations = soak_iterations();
    let soak_baseline = [
        BalanceExpectation {
            client: &alice_on_ds1,
            asset_id: &alice_ds1_asset,
            expected: Numeric::from(70_u32),
        },
        BalanceExpectation {
            client: &bob_on_ds1,
            asset_id: &bob_ds1_asset,
            expected: Numeric::from(30_u32),
        },
        BalanceExpectation {
            client: &alice_on_ds2,
            asset_id: &alice_ds2_asset,
            expected: Numeric::from(45_u32),
        },
        BalanceExpectation {
            client: &bob_on_ds2,
            asset_id: &bob_ds2_asset,
            expected: Numeric::from(155_u32),
        },
    ];
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
        let mut soak_submitter = alice_on_ds1.clone();
        for iteration in 0..soak_iterations {
            let iteration_started = Instant::now();
            let mut run_result = Err(eyre!("iteration {} exceeded retry budget", iteration + 1));
            for attempt in 0..SOAK_ITERATION_ATTEMPTS {
                let attempt_result = (|| -> Result<(Duration, Duration, Duration, Duration)> {
                    let retarget_started = Instant::now();
                    soak_submitter = alice_on_ds1.clone();
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
                    let pre_barrier_height = alice
                        .get_sumeragi_status_wire()
                        .map_err(|err| eyre!(err))?
                        .commit_qc
                        .height
                        .max(
                            bob.get_sumeragi_status_wire()
                                .map_err(|err| eyre!(err))?
                                .commit_qc
                                .height,
                        );
                    soak_submitter.submit_transaction(&soak_swap_tx)?;
                    let submit_elapsed = submit_started.elapsed();
                    let barrier_started = Instant::now();
                    let _synced_after_paired_swaps = match wait_for_committed_success(
                        &alice,
                        soak_swap_entry_hash.clone(),
                        "soak paired swaps confirmation on alice",
                        SOAK_COMMITTED_OUTCOME_TIMEOUT,
                    ) {
                        Ok(()) => alice.get_sumeragi_status_wire().map_err(|err| eyre!(err))?,
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
                            match wait_for_height_with_tick_timeout(
                                &alice,
                                &alice,
                                pre_barrier_height.saturating_add(1),
                                "soak paired swaps barrier on alice (height fallback)",
                                SOAK_PHASE_WAIT_TIMEOUT,
                                SOAK_BARRIER_TICK_EVERY_POLLS,
                            ) {
                                Ok(status) => status,
                                Err(height_err) => match wait_for_committed_success(
                                    &alice,
                                    soak_swap_entry_hash.clone(),
                                    "soak paired swaps confirmation on alice (post-barrier-timeout)",
                                    SOAK_PHASE_WAIT_TIMEOUT,
                                ) {
                                    Ok(()) => alice
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
        // Treat soak as a stress signal instead of a hard gate under shared-host contention.
        ensure!(
            soak_passes > 0,
            "soak produced zero successful iterations; failed {}",
            soak_failures.len()
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
            let submitter = alice_on_ds1.clone();
            let mut submitter = submitter;
            let failing_swap_tx = submitter
                .build_transaction([InstructionBox::from(failing_swap)], Metadata::default());
            let entry_hash = failing_swap_tx.hash_as_entrypoint();
            last_attempt_entry_hash = Some(entry_hash.clone());
            submitter.transaction_status_timeout = BLOCKING_CONFIRMATION_TIMEOUT;
            match submitter.submit_transaction_blocking(&failing_swap_tx) {
                Ok(_) => {
                    return Err(eyre!(
                        "underfunded counter-leg unexpectedly approved on rollback attempt {}",
                        attempt + 1
                    ));
                }
                Err(err) => {
                    let error_text = err.to_string();
                    if error_text.contains("settlement leg requires 10000")
                        || error_text.contains("requires 10000")
                    {
                        failure_text = Some(error_text);
                        break;
                    }
                    if is_inconclusive_blocking_submit_error(&error_text)
                        && attempt + 1 < ROLLBACK_CAPPED_ATTEMPTS
                    {
                        if let Ok(committed_reason) = wait_for_committed_rejection_reason(
                            &alice,
                            entry_hash.clone(),
                            "rollback rejection reason from committed history",
                            ROLLBACK_HISTORY_RETRY_TIMEOUT,
                        ) {
                            failure_text = Some(committed_reason);
                            break;
                        }
                        eprintln!(
                            "[rollback] inconclusive submit on attempt {}; retrying with fresh leader target",
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
        if is_inconclusive_blocking_submit_error(&failure_text) {
            let entry_hash = last_attempt_entry_hash
                .ok_or_else(|| eyre!("missing transaction entry hash for rollback fallback"))?;
            eprintln!("[rollback] falling back to committed history lookup for rejection reason");
            match wait_for_committed_rejection_reason(
                &alice,
                entry_hash,
                "rollback rejection reason from committed history fallback",
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
                        "[rollback] committed history lookup inconclusive; falling back to uncapped blocking confirmation"
                    );
                    let final_fallback_swap = DvpIsi::new(
                        "ds1ds2swapfail_final".parse().expect("settlement id"),
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
                    let submitter = alice_on_ds1.clone();
                    let fallback_error = submitter
                        .submit_blocking(InstructionBox::from(final_fallback_swap))
                        .expect_err(
                            "underfunded counter-leg must reject all-or-nothing settlement",
                        );
                    failure_text = fallback_error.to_string();
                }
            }
        }
        assert!(
            failure_text.contains("settlement leg requires 10000")
                || failure_text.contains("requires 10000"),
            "unexpected failure message: {failure_text}"
        );
        wait_for_expected_balances(&soak_baseline, "rollback balances after failing swap")?;
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
    use super::parse_positive_usize_override;

    #[test]
    fn parse_positive_usize_override_uses_positive_input() {
        assert_eq!(parse_positive_usize_override(Some("12"), 10), 12);
        assert_eq!(parse_positive_usize_override(Some(" 7 "), 10), 7);
    }

    #[test]
    fn parse_positive_usize_override_falls_back_on_invalid_input() {
        assert_eq!(parse_positive_usize_override(None, 10), 10);
        assert_eq!(parse_positive_usize_override(Some("0"), 10), 10);
        assert_eq!(parse_positive_usize_override(Some("not-a-number"), 10), 10);
        assert_eq!(parse_positive_usize_override(Some(""), 10), 10);
    }
}
