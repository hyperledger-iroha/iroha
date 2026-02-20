#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Localnet cross-dataspace atomic swap regression test.

use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroU32,
    thread,
    time::{Duration, Instant},
};

use eyre::{Result, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{
        Level, ValidationFail,
        account::{Account, AccountId},
        asset::{AssetDefinition, AssetDefinitionId, AssetId},
        block::consensus::SumeragiStatusWire,
        da::commitment::DaProofPolicyBundle,
        domain::{Domain, DomainId},
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
        prelude::{FindAssetById, Numeric},
    },
    query::QueryError,
};
use iroha_config::parameters::actual::LaneConfig as ActualLaneConfig;
use iroha_core::da::proof_policy_bundle;
use iroha_crypto::PrivateKey;
use iroha_data_model::query::error::{FindError, QueryExecutionFail};
use iroha_executor_data_model::permission::{
    asset::CanTransferAssetWithDefinition, asset_definition::CanRegisterAssetDefinition,
};
use iroha_test_network::{NetworkBuilder, genesis_factory_with_post_topology};
use iroha_test_samples::{
    ALICE_ID, ALICE_KEYPAIR, BOB_ID, BOB_KEYPAIR, SAMPLE_GENESIS_ACCOUNT_KEYPAIR,
};
use norito::json::Value as JsonValue;
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
const STAKE_ASSET_ID: &str = "xor#nexus";
const STATUS_WAIT_TIMEOUT: Duration = Duration::from_secs(45);
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(200);
const DATASPACE_COMMITMENT_TICK_EVERY_POLLS: u64 = 10;
const BALANCE_WAIT_TICK_EVERY_POLLS: u64 = 10;
const SOAK_ITERATIONS: usize = 10;
const SOAK_SUBMITTER_REFRESH_EVERY: usize = 3;

fn localnet_builder() -> NetworkBuilder {
    let gas_account_str = format!("{}@ivm", SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key());
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
                .write(["nexus", "staking", "stake_asset_id"], STAKE_ASSET_ID)
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
    let nexus_domain: DomainId = "nexus".parse().expect("nexus domain");
    let ivm_domain: DomainId = "ivm".parse().expect("ivm domain");
    let stake_asset_id: AssetDefinitionId = STAKE_ASSET_ID.parse().expect("stake asset definition");
    let gas_account_id = AccountId::new(
        ivm_domain.clone(),
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone(),
    );

    let mut bootstrap_tx = vec![
        Register::domain(Domain::new(nexus_domain.clone())).into(),
        Register::domain(Domain::new(ivm_domain)).into(),
        Register::account(Account::new(gas_account_id)).into(),
        Register::asset_definition(AssetDefinition::numeric(stake_asset_id.clone())).into(),
    ];

    let mut validator_tx = Vec::with_capacity(TOTAL_PEERS * 2);
    for (index, peer) in topology.iter().enumerate() {
        let lane_index = if index < VALIDATORS_PER_LANE {
            NEXUS_LANE_INDEX
        } else if index < VALIDATORS_PER_LANE * 2 {
            DS1_LANE_INDEX
        } else {
            DS2_LANE_INDEX
        };
        let lane_id = LaneId::new(lane_index);
        let validator_id = AccountId::new(nexus_domain.clone(), peer.public_key().clone());
        bootstrap_tx.push(Register::account(Account::new(validator_id.clone())).into());
        bootstrap_tx.push(
            Mint::asset_numeric(
                VALIDATOR_STAKE,
                AssetId::new(stake_asset_id.clone(), validator_id.clone()),
            )
            .into(),
        );
        validator_tx.push(
            RegisterPublicLaneValidator::new(
                lane_id,
                validator_id.clone(),
                validator_id.clone(),
                Numeric::from(VALIDATOR_STAKE),
                Metadata::default(),
            )
            .into(),
        );
        validator_tx.push(ActivatePublicLaneValidator::new(lane_id, validator_id).into());
    }

    vec![bootstrap_tx, validator_tx]
}

fn wait_for_height(
    client: &Client,
    target_height: u64,
    context: &str,
) -> Result<SumeragiStatusWire> {
    let started = Instant::now();
    let mut last_height = 0;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let status = client
            .get_sumeragi_status_wire()
            .map_err(|err| eyre!(err))?;
        last_height = status.commit_qc.height;
        if status.commit_qc.height >= target_height {
            return Ok(status);
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "{context}: timed out waiting for block height >= {target_height}; last observed {last_height}"
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

fn has_dataspace_commitment(status: &SumeragiStatusWire, dataspace_id: u64) -> bool {
    let expected = DataSpaceId::new(dataspace_id);
    status
        .dataspace_commitments
        .iter()
        .any(|entry| entry.dataspace_id == expected && entry.tx_count > 0)
}

fn next_stall_polls(last_height: u64, observed_height: u64, stalled_polls: u64) -> u64 {
    if observed_height > last_height {
        0
    } else {
        stalled_polls.saturating_add(1)
    }
}

fn should_emit_tick(stalled_polls: u64, cadence: u64) -> bool {
    stalled_polls > 0 && cadence > 0 && stalled_polls % cadence == 0
}

#[derive(Clone, Copy, Debug)]
struct DataspaceCommitmentObservation {
    height: u64,
    elapsed: Duration,
}

fn wait_for_dataspace_commitments(
    observer: &Client,
    tick_submitters: &[&Client],
    min_height: u64,
    dataspace_ids: &[u64],
    context: &str,
) -> Result<BTreeMap<u64, DataspaceCommitmentObservation>> {
    let started = Instant::now();
    let mut last_height = 0;
    let mut last_commitments = String::new();
    let mut stalled_polls = 0_u64;
    let mut seen_heights = BTreeMap::<u64, DataspaceCommitmentObservation>::new();
    let mut tick_submitter_index = 0usize;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let status = observer
            .get_sumeragi_status_wire()
            .map_err(|err| eyre!(err))?;
        let observed_height = status.commit_qc.height;
        last_commitments = format!("{:?}", status.dataspace_commitments);
        if observed_height >= min_height {
            for dataspace_id in dataspace_ids {
                if !seen_heights.contains_key(dataspace_id)
                    && has_dataspace_commitment(&status, *dataspace_id)
                {
                    seen_heights.insert(
                        *dataspace_id,
                        DataspaceCommitmentObservation {
                            height: observed_height,
                            elapsed: started.elapsed(),
                        },
                    );
                }
            }
            if dataspace_ids
                .iter()
                .all(|dataspace_id| seen_heights.contains_key(dataspace_id))
            {
                return Ok(seen_heights);
            }
        }
        stalled_polls = next_stall_polls(last_height, observed_height, stalled_polls);
        if should_emit_tick(stalled_polls, DATASPACE_COMMITMENT_TICK_EVERY_POLLS) {
            if let Some(tick_submitter) =
                tick_submitters.get(tick_submitter_index % tick_submitters.len().max(1))
            {
                tick_submitter_index = tick_submitter_index.saturating_add(1);
                let missing = dataspace_ids
                    .iter()
                    .copied()
                    .filter(|dataspace_id| !seen_heights.contains_key(dataspace_id))
                    .collect::<Vec<_>>();
                let _ = tick_submitter.submit(Log::new(
                    Level::INFO,
                    format!(
                        "{context} stall tick {} at height {} missing {:?}",
                        stalled_polls / DATASPACE_COMMITMENT_TICK_EVERY_POLLS,
                        observed_height,
                        missing
                    ),
                ));
            }
        }
        last_height = observed_height;
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    let missing = dataspace_ids
        .iter()
        .copied()
        .filter(|dataspace_id| !seen_heights.contains_key(dataspace_id))
        .collect::<Vec<_>>();
    Err(eyre!(
        "{context}: timed out waiting for dataspace commitments {dataspace_ids:?} at or above height {min_height}; missing {missing:?}; seen {seen_heights:?}; last height {last_height}, last commitments {last_commitments}"
    ))
}

fn wait_for_expected_balances(
    client: &Client,
    expectations: &[(&AssetId, Numeric)],
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_observed = Vec::with_capacity(expectations.len());
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        last_observed.clear();
        let mut all_match = true;
        for (asset_id, expected) in expectations {
            let observed = asset_balance(client, asset_id)?;
            if observed != *expected {
                all_match = false;
            }
            last_observed.push(((*asset_id).clone(), observed));
        }
        if all_match {
            return Ok(());
        }
        thread::sleep(STATUS_POLL_INTERVAL);
    }
    Err(eyre!(
        "{context}: timed out waiting for expected balances; last observed {last_observed:?}"
    ))
}

fn wait_for_expected_balances_with_tick(
    client: &Client,
    tick_submitter: &Client,
    expectations: &[(&AssetId, Numeric)],
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_observed = Vec::with_capacity(expectations.len());
    let mut polls = 0_u64;
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        last_observed.clear();
        let mut all_match = true;
        for (asset_id, expected) in expectations {
            let observed = asset_balance(client, asset_id)?;
            if observed != *expected {
                all_match = false;
            }
            last_observed.push(((*asset_id).clone(), observed));
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
    Err(eyre!(
        "{context}: timed out waiting for expected balances with tick assist; last observed {last_observed:?}"
    ))
}

fn lane_validator_snapshot(
    snapshot: &JsonValue,
    context: &str,
) -> Result<(usize, BTreeSet<String>)> {
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
        let status_type = entry
            .get("status")
            .and_then(JsonValue::as_object)
            .and_then(|status| status.get("type"))
            .and_then(JsonValue::as_str)
            .ok_or_else(|| eyre!("{context}: validator entry missing status.type"))?;
        if status_type == "Active" {
            active.insert(validator.to_owned());
        }
    }

    Ok((usize::try_from(total).unwrap_or(usize::MAX), active))
}

fn wait_for_active_lane_validators(
    client: &Client,
    lane_id: LaneId,
    expected_active: &BTreeSet<String>,
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_total = 0usize;
    let mut last_active = BTreeSet::new();
    while started.elapsed() <= STATUS_WAIT_TIMEOUT {
        let snapshot = client
            .get_public_lane_validators(lane_id, None)
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

fn leader_targeted_client_for_account(
    network: &sandbox::SerializedNetwork,
    status_client: &Client,
    account_id: &AccountId,
    private_key: &PrivateKey,
) -> Client {
    let index = leader_or_highest_height_peer_index(network, status_client);
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
    let (network, _rt) = {
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
        let validator_domain: DomainId = "nexus".parse().expect("validator domain");
        let nexus_lane_validators: Vec<AccountId> = peers
            .iter()
            .take(VALIDATORS_PER_LANE)
            .map(|peer| AccountId::new(validator_domain.clone(), peer.id().public_key().clone()))
            .collect();
        let ds1_lane_validators: Vec<AccountId> = peers
            .iter()
            .skip(VALIDATORS_PER_LANE)
            .take(VALIDATORS_PER_LANE)
            .map(|peer| AccountId::new(validator_domain.clone(), peer.id().public_key().clone()))
            .collect();
        let ds2_lane_validators: Vec<AccountId> = peers
            .iter()
            .skip(VALIDATORS_PER_LANE * 2)
            .take(VALIDATORS_PER_LANE)
            .map(|peer| AccountId::new(validator_domain.clone(), peer.id().public_key().clone()))
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
        let expected_nexus_validators: BTreeSet<_> = nexus_lane_validators
            .iter()
            .map(ToString::to_string)
            .collect();
        let expected_ds1_validators: BTreeSet<_> = ds1_lane_validators
            .iter()
            .map(ToString::to_string)
            .collect();
        let expected_ds2_validators: BTreeSet<_> = ds2_lane_validators
            .iter()
            .map(ToString::to_string)
            .collect();
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

    let initial_height = alice
        .get_sumeragi_status_wire()
        .map_err(|err| eyre!(err))?
        .commit_qc
        .height;

    let ds1_submitter = leader_targeted_client_for_account(
        &network,
        &alice,
        &ALICE_ID,
        ALICE_KEYPAIR.private_key(),
    );
    let ds2_submitter =
        leader_targeted_client_for_account(&network, &bob, &BOB_ID, BOB_KEYPAIR.private_key());
    {
        let _phase = phase_timings.phase("route probes: tx submit enqueue");
        ds1_submitter.submit(Log::new(Level::INFO, "route probe ds1".to_string()))?;
        ds2_submitter.submit(Log::new(Level::INFO, "route probe ds2".to_string()))?;
    }
    let probe_commitments = {
        let _phase = phase_timings.phase("route probes: barrier wait");
        wait_for_dataspace_commitments(
            &alice,
            &[&ds1_submitter, &ds2_submitter],
            initial_height + 1,
            &[DS1_ID_U64, DS2_ID_U64],
            "route probes",
        )?
    };
    {
        let _phase = phase_timings.phase("route probe ds1: query/assert");
        let ds1_observation = probe_commitments
            .get(&DS1_ID_U64)
            .ok_or_else(|| eyre!("missing ds1 route commitment height"))?;
        let ds1_height = ds1_observation.height;
        eprintln!(
            "[route-probe] ds1 first_seen={}s height={}",
            ds1_observation.elapsed.as_secs_f64(),
            ds1_height
        );
        let _ds1_status_bob = wait_for_height(&bob, ds1_height, "ds1 probe on bob")?;
    }
    {
        let _phase = phase_timings.phase("route probe ds2: query/assert");
        let ds2_observation = probe_commitments
            .get(&DS2_ID_U64)
            .ok_or_else(|| eyre!("missing ds2 route commitment height"))?;
        let ds2_height = ds2_observation.height;
        eprintln!(
            "[route-probe] ds2 first_seen={}s height={}",
            ds2_observation.elapsed.as_secs_f64(),
            ds2_height
        );
        let ds1_observation = probe_commitments
            .get(&DS1_ID_U64)
            .ok_or_else(|| eyre!("missing ds1 route commitment timing"))?;
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
        let _ds2_status_bob = wait_for_height(&bob, ds2_height, "ds2 probe on bob")?;
    }

    let ds1_asset_def: AssetDefinitionId = "ds1coin#wonderland".parse().expect("asset definition");
    let ds2_asset_def: AssetDefinitionId = "ds2coin#wonderland".parse().expect("asset definition");
    let wonderland_domain: DomainId = "wonderland".parse().expect("domain id");
    let alice_ds1_asset = AssetId::new(ds1_asset_def.clone(), ALICE_ID.clone());
    let bob_ds1_asset = AssetId::new(ds1_asset_def.clone(), BOB_ID.clone());
    let alice_ds2_asset = AssetId::new(ds2_asset_def.clone(), ALICE_ID.clone());
    let bob_ds2_asset = AssetId::new(ds2_asset_def.clone(), BOB_ID.clone());

    let grants_start_height = alice
        .get_sumeragi_status_wire()
        .map_err(|err| eyre!(err))?
        .commit_qc
        .height;
    {
        let submitter = leader_targeted_client_for_account(
            &network,
            &alice,
            &ALICE_ID,
            ALICE_KEYPAIR.private_key(),
        );
        let _phase = phase_timings.phase("setup grants: tx submit enqueue");
        submitter.submit_all(vec![
            InstructionBox::from(Grant::account_permission(
                CanRegisterAssetDefinition {
                    domain: wonderland_domain.clone(),
                },
                BOB_ID.clone(),
            )),
            InstructionBox::from(Grant::account_permission(
                CanTransferAssetWithDefinition {
                    asset_definition: ds1_asset_def.clone(),
                },
                BOB_ID.clone(),
            )),
        ])?;
    }
    {
        let _phase = phase_timings.phase("setup grants: barrier wait");
        let _grants_synced_alice = wait_for_height(
            &alice,
            grants_start_height + 1,
            "grant setup confirmation on alice",
        )?;
        let _grants_synced_bob = wait_for_height(
            &bob,
            grants_start_height + 1,
            "grant setup confirmation on bob",
        )?;
    }
    {
        let _phase = phase_timings.phase("setup grants: query/assert");
        let _status = alice.get_sumeragi_status_wire().map_err(|err| eyre!(err))?;
    }

    let register_seed_start_height = alice
        .get_sumeragi_status_wire()
        .map_err(|err| eyre!(err))?
        .commit_qc
        .height;
    {
        let alice_submitter = leader_targeted_client_for_account(
            &network,
            &alice,
            &ALICE_ID,
            ALICE_KEYPAIR.private_key(),
        );
        let bob_submitter =
            leader_targeted_client_for_account(&network, &bob, &BOB_ID, BOB_KEYPAIR.private_key());
        let _phase = phase_timings.phase("setup register+mint: tx submit enqueue");
        alice_submitter.submit_all(vec![
            InstructionBox::from(Register::asset_definition(AssetDefinition::numeric(
                ds1_asset_def.clone(),
            ))),
            InstructionBox::from(Mint::asset_numeric(100_u32, alice_ds1_asset.clone())),
        ])?;
        bob_submitter.submit_all(vec![
            InstructionBox::from(Register::asset_definition(AssetDefinition::numeric(
                ds2_asset_def.clone(),
            ))),
            InstructionBox::from(Mint::asset_numeric(200_u32, bob_ds2_asset.clone())),
        ])?;
    }
    {
        let _phase = phase_timings.phase("setup register+mint: barrier wait");
        let register_seed_target_height = register_seed_start_height.saturating_add(1);
        let _setup_synced_alice = wait_for_height(
            &alice,
            register_seed_target_height,
            "register+mint setup confirmation on alice",
        )?;
        let _setup_synced_bob = wait_for_height(
            &bob,
            register_seed_target_height,
            "register+mint setup confirmation on bob",
        )?;
    }
    {
        let _phase = phase_timings.phase("setup register+mint: query/assert");
        let tick_submitter = leader_targeted_client_for_account(
            &network,
            &alice,
            &ALICE_ID,
            ALICE_KEYPAIR.private_key(),
        );
        wait_for_expected_balances_with_tick(
            &alice,
            &tick_submitter,
            &[
                (&alice_ds1_asset, Numeric::from(100_u32)),
                (&bob_ds1_asset, Numeric::from(0_u32)),
                (&alice_ds2_asset, Numeric::from(0_u32)),
                (&bob_ds2_asset, Numeric::from(200_u32)),
            ],
            "seed balances after pipelined setup",
        )?;
    }

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
        let success_start_height = alice
            .get_sumeragi_status_wire()
            .map_err(|err| eyre!(err))?
            .commit_qc
            .height;
        let submitter = leader_targeted_client_for_account(
            &network,
            &alice,
            &ALICE_ID,
            ALICE_KEYPAIR.private_key(),
        );
        {
            let _phase = phase_timings.phase("execute successful swap: tx submit enqueue");
            submitter.submit(InstructionBox::from(successful_swap))?;
        }
        let synced_after_success = {
            let _phase = phase_timings.phase("execute successful swap: barrier wait");
            wait_for_height(
                &alice,
                success_start_height.saturating_add(1),
                "successful swap confirmation on alice",
            )?
        };
        {
            let _phase = phase_timings.phase("execute successful swap: query/assert");
            let _synced_after_success_bob = wait_for_height(
                &bob,
                synced_after_success.commit_qc.height,
                "successful swap propagation to bob",
            )?;
            wait_for_expected_balances(
                &alice,
                &[
                    (&alice_ds1_asset, Numeric::from(70_u32)),
                    (&bob_ds1_asset, Numeric::from(30_u32)),
                    (&alice_ds2_asset, Numeric::from(45_u32)),
                    (&bob_ds2_asset, Numeric::from(155_u32)),
                ],
                "successful swap balances",
            )?;
        }
    }

    {
        let reverse_successful_swap = DvpIsi::new(
            "ds2ds1swapok".parse().expect("settlement id"),
            SettlementLeg::new(
                ds2_asset_def.clone(),
                Numeric::from(20_u32),
                BOB_ID.clone(),
                ALICE_ID.clone(),
            ),
            SettlementLeg::new(
                ds1_asset_def.clone(),
                Numeric::from(10_u32),
                ALICE_ID.clone(),
                BOB_ID.clone(),
            ),
            SettlementPlan::new(
                SettlementExecutionOrder::DeliveryThenPayment,
                SettlementAtomicity::AllOrNothing,
            ),
        );
        let reverse_start_height = bob
            .get_sumeragi_status_wire()
            .map_err(|err| eyre!(err))?
            .commit_qc
            .height;
        let submitter =
            leader_targeted_client_for_account(&network, &bob, &BOB_ID, BOB_KEYPAIR.private_key());
        {
            let _phase = phase_timings.phase("execute reverse swap: tx submit enqueue");
            submitter.submit(InstructionBox::from(reverse_successful_swap))?;
        }
        let synced_after_reverse = {
            let _phase = phase_timings.phase("execute reverse swap: barrier wait");
            wait_for_height(
                &bob,
                reverse_start_height.saturating_add(1),
                "reverse swap confirmation on bob",
            )?
        };
        {
            let _phase = phase_timings.phase("execute reverse swap: query/assert");
            let _synced_after_reverse_bob = wait_for_height(
                &alice,
                synced_after_reverse.commit_qc.height,
                "reverse swap propagation to alice",
            )?;
            wait_for_expected_balances(
                &alice,
                &[
                    (&alice_ds1_asset, Numeric::from(60_u32)),
                    (&bob_ds1_asset, Numeric::from(40_u32)),
                    (&alice_ds2_asset, Numeric::from(65_u32)),
                    (&bob_ds2_asset, Numeric::from(135_u32)),
                ],
                "reverse swap balances",
            )?;
        }
    }

    let soak_baseline = [
        (&alice_ds1_asset, Numeric::from(60_u32)),
        (&bob_ds1_asset, Numeric::from(40_u32)),
        (&alice_ds2_asset, Numeric::from(65_u32)),
        (&bob_ds2_asset, Numeric::from(135_u32)),
    ];
    let mut last_soak_synced_height: Option<u64> = None;
    let mut soak_passes = 0usize;
    let mut soak_iteration_durations = Vec::with_capacity(SOAK_ITERATIONS);
    let mut soak_target_durations = Vec::with_capacity(SOAK_ITERATIONS);
    let mut soak_submit_durations = Vec::with_capacity(SOAK_ITERATIONS);
    let mut soak_barrier_durations = Vec::with_capacity(SOAK_ITERATIONS);
    let mut soak_query_durations = Vec::with_capacity(SOAK_ITERATIONS);
    let mut soak_failure: Option<eyre::Report> = None;
    let mut next_soak_target_height = alice
        .get_sumeragi_status_wire()
        .map_err(|err| eyre!(err))?
        .commit_qc
        .height
        .saturating_add(1);
    {
        let _phase = phase_timings.phase("soak 10 iterations: paired swap throughput");
        let mut soak_submitter = leader_targeted_client_for_account(
            &network,
            &alice,
            &ALICE_ID,
            ALICE_KEYPAIR.private_key(),
        );
        for iteration in 0..SOAK_ITERATIONS {
            let iteration_started = Instant::now();
            let run_result = (|| -> Result<(Duration, Duration, Duration, Duration)> {
                let retarget_started = Instant::now();
                if iteration > 0 && iteration % SOAK_SUBMITTER_REFRESH_EVERY == 0 {
                    soak_submitter = leader_targeted_client_for_account(
                        &network,
                        &alice,
                        &ALICE_ID,
                        ALICE_KEYPAIR.private_key(),
                    );
                }
                let target_elapsed = retarget_started.elapsed();
                let forward_swap = DvpIsi::new(
                    format!("soakfwd{iteration}")
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
                    format!("soakrev{iteration}")
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
                soak_submitter.submit_all(vec![
                    InstructionBox::from(forward_swap),
                    InstructionBox::from(reverse_swap),
                ])?;
                let submit_elapsed = submit_started.elapsed();
                let barrier_started = Instant::now();
                let synced_after_paired_swaps = wait_for_height(
                    &alice,
                    next_soak_target_height,
                    "soak paired swaps barrier on alice",
                )?;
                let barrier_elapsed = barrier_started.elapsed();
                next_soak_target_height =
                    synced_after_paired_swaps.commit_qc.height.saturating_add(1);
                last_soak_synced_height = Some(synced_after_paired_swaps.commit_qc.height);
                let query_started = Instant::now();
                wait_for_expected_balances(
                    &alice,
                    &soak_baseline,
                    "soak iteration net-zero balances",
                )?;
                let query_elapsed = query_started.elapsed();
                Ok((
                    target_elapsed,
                    submit_elapsed,
                    barrier_elapsed,
                    query_elapsed,
                ))
            })();

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
                    soak_failure = Some(eyre!("soak iteration {} failed: {err}", iteration + 1));
                    break;
                }
            }
        }
    }
    if let Some((min, avg, max)) = duration_min_avg_max_secs(&soak_iteration_durations) {
        let pass_rate = (soak_passes as f64 / SOAK_ITERATIONS as f64) * 100.0;
        eprintln!("[soak] report-only metrics (no threshold gating)");
        eprintln!(
            "[soak] iterations={} pass_rate={:.1}% min={:.3}s avg={:.3}s max={:.3}s",
            SOAK_ITERATIONS, pass_rate, min, avg, max
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
    if let Some(err) = soak_failure {
        return Err(err);
    }
    if let Some(height) = last_soak_synced_height {
        let _phase = phase_timings.phase("soak final bob sync barrier");
        let _soak_synced_bob = wait_for_height(&bob, height, "soak final propagation to bob")?;
    }

    {
        let _phase = phase_timings.phase("execute failing swap + rollback verification");
        let failing_swap = DvpIsi::new(
            "ds1ds2swapfail".parse().expect("settlement id"),
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
        let submitter = leader_targeted_client_for_account(
            &network,
            &alice,
            &ALICE_ID,
            ALICE_KEYPAIR.private_key(),
        );
        let failure = submitter
            .submit_blocking(InstructionBox::from(failing_swap))
            .expect_err("underfunded counter-leg must reject all-or-nothing settlement");
        let failure_text = failure.to_string();
        assert!(
            failure_text.contains("settlement leg requires 10000")
                || failure_text.contains("requires 10000"),
            "unexpected failure message: {failure_text}"
        );
        wait_for_expected_balances(
            &alice,
            &soak_baseline,
            "rollback balances after failing swap",
        )?;
    }

    phase_timings.emit_summary();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{next_stall_polls, should_emit_tick};

    #[test]
    fn next_stall_polls_resets_on_height_progress() {
        assert_eq!(next_stall_polls(5, 6, 9), 0);
        assert_eq!(next_stall_polls(5, 5, 0), 1);
        assert_eq!(next_stall_polls(5, 5, 3), 4);
    }

    #[test]
    fn should_emit_tick_triggers_on_cadence_boundaries() {
        assert!(!should_emit_tick(0, 10));
        assert!(!should_emit_tick(9, 10));
        assert!(should_emit_tick(10, 10));
        assert!(should_emit_tick(20, 10));
        assert!(!should_emit_tick(10, 0));
    }
}
