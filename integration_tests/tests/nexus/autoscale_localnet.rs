#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Localnet autoscale regression test for Nexus lane expansion/contraction.

use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use eyre::{Result, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{Level, isi::Log, nexus::LaneId},
};
use iroha_core::sumeragi::network_topology::commit_quorum_from_len;
use iroha_test_network::{NetworkBuilder, NetworkPeer};

const TOTAL_PEERS: usize = 4;
const INITIAL_PROVISIONED_LANES: usize = 1;
const EXPANDED_PROVISIONED_LANES: usize = 2;
const PRIMARY_LANE_ID: u32 = 0;
const ELASTIC_LANE_ID: u32 = 1;
const LOAD_TX_COUNT: usize = 48;
const LANE_POLL_INTERVAL: Duration = Duration::from_millis(250);
const EXPANSION_PROBE_INTERVAL: Duration = Duration::from_millis(1000);
const EXPANSION_TOP_UP_EVERY_HEARTBEATS: u64 = 4;
const EXPANSION_TOP_UP_TX_COUNT: usize = 16;
const EXPANSION_REINFORCE_EVERY_HEARTBEATS: u64 = 12;
const EXPANSION_REINFORCE_TX_COUNT: usize = 32;
const EXPANSION_STATUS_SIGNAL_GRACE: Duration = Duration::from_secs(8);
const EXPANSION_POST_STORAGE_STATUS_WINDOW: Duration = Duration::from_secs(8);
const EXPANSION_POST_STORAGE_TOP_UP_TX_COUNT: usize = 16;
const CONTRACTION_HEARTBEAT_INTERVAL: Duration = Duration::from_millis(1000);
const SCALE_OUT_WAIT_TIMEOUT: Duration = Duration::from_secs(120);
const SCALE_IN_WAIT_TIMEOUT: Duration = Duration::from_secs(180);
const TORII_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const QUORUM_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(30);
const STATUS_SNAPSHOT_RETRY_LIMIT: u32 = 5;
const STATUS_SNAPSHOT_RETRY_BACKOFF: Duration = Duration::from_millis(250);
static LOAD_TX_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn autoscale_localnet_builder() -> NetworkBuilder {
    NetworkBuilder::new()
        .with_peers(TOTAL_PEERS)
        .with_pipeline_time(Duration::from_millis(300))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["nexus", "enabled"], true)
                .write(["nexus", "autoscale", "enabled"], true)
                .write(["nexus", "autoscale", "min_lanes"], 1_i64)
                .write(["nexus", "autoscale", "max_lanes"], 2_i64)
                .write(["nexus", "autoscale", "target_block_ms"], 2000_i64)
                .write(["nexus", "autoscale", "scale_out_latency_ratio"], 0.2_f64)
                .write(["nexus", "autoscale", "scale_in_latency_ratio"], 0.15_f64)
                .write(
                    ["nexus", "autoscale", "scale_out_utilization_ratio"],
                    0.6_f64,
                )
                .write(
                    ["nexus", "autoscale", "scale_in_utilization_ratio"],
                    0.5_f64,
                )
                .write(["nexus", "autoscale", "scale_out_window_blocks"], 2_i64)
                .write(["nexus", "autoscale", "scale_in_window_blocks"], 4_i64)
                .write(["nexus", "autoscale", "cooldown_blocks"], 1_i64)
                .write(["nexus", "autoscale", "per_lane_target_tps"], 1_i64);
        })
}

fn active_lane_segments(peer: &NetworkPeer) -> Result<Vec<String>> {
    let blocks_root = peer.kura_store_dir().join("blocks");
    if !blocks_root.exists() {
        return Ok(Vec::new());
    }

    let mut lanes = Vec::new();
    for entry in fs::read_dir(&blocks_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        if name.starts_with("lane_") {
            lanes.push(name);
        }
    }
    lanes.sort();
    Ok(lanes)
}

fn lane_snapshot(network: &sandbox::SerializedNetwork) -> Result<Vec<(usize, Vec<String>)>> {
    network
        .peers()
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            active_lane_segments(peer)
                .map(|lanes| (index, lanes))
                .map_err(|err| eyre!("read active lane segments on peer {index}: {err}"))
        })
        .collect()
}

fn peer_client_with_timeout(peer: &NetworkPeer) -> Client {
    let mut client = peer.client();
    client.torii_request_timeout = TORII_REQUEST_TIMEOUT;
    client
}

#[derive(Clone, Debug)]
struct LaneStatusSnapshot {
    lane_id: u32,
    capacity: u64,
    committed: u64,
}

#[derive(Clone, Debug)]
struct LaneCommitmentSnapshot {
    lane_id: u32,
    block_height: u64,
    tx_count: u64,
    teu_total: u64,
}

#[derive(Clone, Debug)]
struct LaneRelaySnapshot {
    lane_id: u32,
    block_height: u64,
}

#[derive(Clone, Debug)]
struct LaneValidatorSnapshot {
    lane_id: u32,
    total: u64,
    active: u64,
    pending_activation: u64,
    max_activation_epoch: u64,
    max_activation_height: u64,
}

#[derive(Clone, Debug)]
struct PeerStatusSnapshot {
    lanes: Vec<LaneStatusSnapshot>,
    lane_commitments: Vec<LaneCommitmentSnapshot>,
    lane_governance_ids: Vec<u32>,
    lane_relay: Vec<LaneRelaySnapshot>,
    lane_validators: Vec<LaneValidatorSnapshot>,
    commit_signatures_required: u64,
    commit_qc_validator_set_len: u64,
    txs_approved: u64,
    txs_rejected: u64,
    blocks_non_empty: u64,
}

fn decode_lane_validator_snapshot(
    payload: &norito::json::Value,
    lane_id: u32,
) -> Option<LaneValidatorSnapshot> {
    let root = payload.as_object()?;
    let items = root.get("items").and_then(norito::json::Value::as_array);
    let total = root
        .get("total")
        .and_then(norito::json::Value::as_u64)
        .or_else(|| items.and_then(|entries| u64::try_from(entries.len()).ok()))
        .unwrap_or_default();

    let mut active = 0_u64;
    let mut pending_activation = 0_u64;
    let mut max_activation_epoch = 0_u64;
    let mut max_activation_height = 0_u64;
    if let Some(entries) = items {
        for entry in entries {
            let entry_obj = entry.as_object();
            let status_type = entry
                .as_object()
                .and_then(|item| item.get("status"))
                .and_then(norito::json::Value::as_object)
                .and_then(|status| status.get("type"))
                .and_then(norito::json::Value::as_str);
            match status_type {
                Some("Active") => active = active.saturating_add(1),
                Some("PendingActivation") => {
                    pending_activation = pending_activation.saturating_add(1);
                }
                _ => {}
            }
            if let Some(epoch) = entry_obj
                .and_then(|item| item.get("activation_epoch"))
                .and_then(norito::json::Value::as_u64)
            {
                max_activation_epoch = max_activation_epoch.max(epoch);
            }
            if let Some(height) = entry_obj
                .and_then(|item| item.get("activation_height"))
                .and_then(norito::json::Value::as_u64)
            {
                max_activation_height = max_activation_height.max(height);
            }
        }
    }

    Some(LaneValidatorSnapshot {
        lane_id,
        total,
        active,
        pending_activation,
        max_activation_epoch,
        max_activation_height,
    })
}

fn is_not_found_lane_validator_error(message: &str) -> bool {
    message.contains("404") || message.contains("Not Found")
}

fn fetch_lane_validator_snapshot(client: &Client, lane_id: u32) -> Option<LaneValidatorSnapshot> {
    match client.get_public_lane_validators(LaneId::new(lane_id), None) {
        Ok(payload) => decode_lane_validator_snapshot(&payload, lane_id),
        Err(err) => {
            let message = err.to_string();
            if is_not_found_lane_validator_error(&message) {
                Some(LaneValidatorSnapshot {
                    lane_id,
                    total: 0,
                    active: 0,
                    pending_activation: 0,
                    max_activation_epoch: 0,
                    max_activation_height: 0,
                })
            } else {
                None
            }
        }
    }
}

fn status_snapshot(network: &sandbox::SerializedNetwork) -> Result<Vec<PeerStatusSnapshot>> {
    network
        .peers()
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            let client = peer_client_with_timeout(peer);
            let status = client
                .get_status()
                .map_err(|err| eyre!("fetch peer {index} status failed: {err}"))?;
            let lanes = status
                .teu_lane_commit
                .iter()
                .map(|lane| LaneStatusSnapshot {
                    lane_id: lane.lane_id,
                    capacity: lane.capacity,
                    committed: lane.committed,
                })
                .collect::<Vec<_>>();
            let (lane_commitments, lane_governance_ids, lane_relay) =
                match client.get_sumeragi_status() {
                    Ok(sumeragi_status) => (
                        sumeragi_status
                            .lane_commitments
                            .into_iter()
                            .map(|lane| LaneCommitmentSnapshot {
                                lane_id: lane.lane_id.as_u32(),
                                block_height: lane.block_height,
                                tx_count: lane.tx_count,
                                teu_total: lane.teu_total,
                            })
                            .collect::<Vec<_>>(),
                        sumeragi_status
                            .lane_governance
                            .into_iter()
                            .map(|lane| lane.lane_id.as_u32())
                            .collect::<Vec<_>>(),
                        sumeragi_status
                            .lane_relay_envelopes
                            .into_iter()
                            .map(|lane| LaneRelaySnapshot {
                                lane_id: lane.lane_id.as_u32(),
                                block_height: lane.block_height,
                            })
                            .collect::<Vec<_>>(),
                    ),
                    Err(_) => (Vec::new(), Vec::new(), Vec::new()),
                };
            let lane_validators = fetch_lane_validator_snapshot(&client, ELASTIC_LANE_ID)
                .into_iter()
                .collect::<Vec<_>>();
            let commit_signatures_required = status
                .sumeragi
                .as_ref()
                .map(|sumeragi| sumeragi.commit_signatures_required)
                .unwrap_or_default();
            let commit_qc_validator_set_len = status
                .sumeragi
                .as_ref()
                .map(|sumeragi| sumeragi.commit_qc_validator_set_len)
                .unwrap_or_default();
            Ok(PeerStatusSnapshot {
                lanes,
                lane_commitments,
                lane_governance_ids,
                lane_relay,
                lane_validators,
                commit_signatures_required,
                commit_qc_validator_set_len,
                txs_approved: status.txs_approved,
                txs_rejected: status.txs_rejected,
                blocks_non_empty: status.blocks_non_empty,
            })
        })
        .collect()
}

fn all_peers_have_storage_lane_count(
    snapshot: &[(usize, Vec<String>)],
    expected_count: usize,
) -> bool {
    snapshot
        .iter()
        .all(|(_, lanes)| lanes.len() == expected_count)
}

fn peer_has_active_lane_capacity(peer: &PeerStatusSnapshot, lane_id: u32) -> bool {
    peer.lanes
        .iter()
        .any(|lane| lane.lane_id == lane_id && (lane.capacity > 0 || lane.committed > 0))
}

fn peer_has_lane_commitment_activity(peer: &PeerStatusSnapshot, lane_id: u32) -> bool {
    peer.lane_commitments
        .iter()
        .any(|lane| lane.lane_id == lane_id && (lane.tx_count > 0 || lane.teu_total > 0))
}

fn peer_lane_status(peer: &PeerStatusSnapshot, lane_id: u32) -> Option<&LaneStatusSnapshot> {
    peer.lanes.iter().find(|lane| lane.lane_id == lane_id)
}

fn peer_lane_commitment_snapshot(
    peer: &PeerStatusSnapshot,
    lane_id: u32,
) -> Option<&LaneCommitmentSnapshot> {
    peer.lane_commitments
        .iter()
        .filter(|lane| lane.lane_id == lane_id)
        .max_by_key(|lane| lane.block_height)
}

fn peer_lane_relay_height(peer: &PeerStatusSnapshot, lane_id: u32) -> Option<u64> {
    peer.lane_relay
        .iter()
        .filter(|relay| relay.lane_id == lane_id)
        .map(|relay| relay.block_height)
        .max()
}

fn peer_lane_validator_snapshot(
    peer: &PeerStatusSnapshot,
    lane_id: u32,
) -> Option<&LaneValidatorSnapshot> {
    peer.lane_validators
        .iter()
        .filter(|lane| lane.lane_id == lane_id)
        .max_by_key(|lane| (lane.active, lane.pending_activation, lane.total))
}

fn peer_has_lane_declaration(peer: &PeerStatusSnapshot, lane_id: u32) -> bool {
    peer.lanes.iter().any(|lane| lane.lane_id == lane_id)
        || peer
            .lane_commitments
            .iter()
            .any(|lane| lane.lane_id == lane_id)
        || peer
            .lane_governance_ids
            .iter()
            .any(|declared_lane| *declared_lane == lane_id)
        || peer.lane_validators.iter().any(|lane| {
            lane.lane_id == lane_id
                && (lane.total > 0 || lane.active > 0 || lane.pending_activation > 0)
        })
}

fn peer_has_lane_declaration_transition(
    peer: &PeerStatusSnapshot,
    baseline_peer: Option<&PeerStatusSnapshot>,
    lane_id: u32,
) -> bool {
    let Some(baseline_peer) = baseline_peer else {
        return false;
    };
    !peer_has_lane_declaration(baseline_peer, lane_id) && peer_has_lane_declaration(peer, lane_id)
}

fn peer_has_lane_progress_transition(
    peer: &PeerStatusSnapshot,
    baseline_peer: Option<&PeerStatusSnapshot>,
    lane_id: u32,
) -> bool {
    let Some(baseline_peer) = baseline_peer else {
        return false;
    };

    let status_progressed = match (
        peer_lane_status(peer, lane_id),
        peer_lane_status(baseline_peer, lane_id),
    ) {
        (Some(current), Some(baseline)) => {
            current.capacity > baseline.capacity || current.committed > baseline.committed
        }
        (Some(current), None) => current.capacity > 0 || current.committed > 0,
        _ => false,
    };

    let commitment_progressed = match (
        peer_lane_commitment_snapshot(peer, lane_id),
        peer_lane_commitment_snapshot(baseline_peer, lane_id),
    ) {
        (Some(current), Some(baseline)) => {
            current.block_height > baseline.block_height
                || current.tx_count > baseline.tx_count
                || current.teu_total > baseline.teu_total
        }
        (Some(current), None) => {
            current.block_height > 0 || current.tx_count > 0 || current.teu_total > 0
        }
        _ => false,
    };

    let relay_progressed = match (
        peer_lane_relay_height(peer, lane_id),
        peer_lane_relay_height(baseline_peer, lane_id),
    ) {
        (Some(current), Some(baseline)) => current > baseline,
        (Some(current), None) => current > 0,
        _ => false,
    };

    let validator_progressed = match (
        peer_lane_validator_snapshot(peer, lane_id),
        peer_lane_validator_snapshot(baseline_peer, lane_id),
    ) {
        (Some(current), Some(baseline)) => {
            current.active > baseline.active
                || current.pending_activation > baseline.pending_activation
                || current.total > baseline.total
                || current.max_activation_epoch > baseline.max_activation_epoch
                || current.max_activation_height > baseline.max_activation_height
        }
        _ => false,
    };

    status_progressed || commitment_progressed || relay_progressed || validator_progressed
}

fn peers_with_expanded_lane_signal(
    snapshot: &[PeerStatusSnapshot],
    baseline_snapshot: Option<&[PeerStatusSnapshot]>,
    lane_id: u32,
) -> usize {
    let mut peers_with_signal = 0_usize;
    for (index, peer) in snapshot.iter().enumerate() {
        let baseline_peer = baseline_snapshot.and_then(|baseline| baseline.get(index));
        if peer_has_active_lane_capacity(peer, lane_id)
            || peer_has_lane_commitment_activity(peer, lane_id)
            || peer_has_lane_declaration_transition(peer, baseline_peer, lane_id)
            || peer_has_lane_progress_transition(peer, baseline_peer, lane_id)
        {
            peers_with_signal += 1;
        }
    }
    peers_with_signal
}

#[derive(Clone, Copy, Debug, Default)]
struct ExpansionSignalBreakdown {
    peers_with_active_capacity: usize,
    peers_with_commitment_activity: usize,
    peers_with_lane_declaration: usize,
    peers_with_lane_declaration_transition: usize,
    peers_with_lane_progress_transition: usize,
    peers_with_lane_validator_snapshot: usize,
    peers_with_lane_validator_activity: usize,
}

fn expansion_signal_breakdown(
    snapshot: &[PeerStatusSnapshot],
    baseline_snapshot: Option<&[PeerStatusSnapshot]>,
    lane_id: u32,
) -> ExpansionSignalBreakdown {
    let mut breakdown = ExpansionSignalBreakdown::default();
    for (index, peer) in snapshot.iter().enumerate() {
        let baseline_peer = baseline_snapshot.and_then(|baseline| baseline.get(index));
        if peer_has_active_lane_capacity(peer, lane_id) {
            breakdown.peers_with_active_capacity += 1;
        }
        if peer_has_lane_commitment_activity(peer, lane_id) {
            breakdown.peers_with_commitment_activity += 1;
        }
        if peer_has_lane_declaration(peer, lane_id) {
            breakdown.peers_with_lane_declaration += 1;
        }
        if peer_has_lane_declaration_transition(peer, baseline_peer, lane_id) {
            breakdown.peers_with_lane_declaration_transition += 1;
        }
        if peer_has_lane_progress_transition(peer, baseline_peer, lane_id) {
            breakdown.peers_with_lane_progress_transition += 1;
        }
        if let Some(validator) = peer_lane_validator_snapshot(peer, lane_id) {
            breakdown.peers_with_lane_validator_snapshot += 1;
            if validator.active > 0 || validator.pending_activation > 0 {
                breakdown.peers_with_lane_validator_activity += 1;
            }
        }
    }
    breakdown
}

fn expansion_observed_on_quorum_peers(
    status_snapshot: &[PeerStatusSnapshot],
    baseline_snapshot: Option<&[PeerStatusSnapshot]>,
    quorum_required: usize,
) -> bool {
    peers_with_expanded_lane_signal(status_snapshot, baseline_snapshot, ELASTIC_LANE_ID)
        >= quorum_required
}

fn expansion_observed_on_storage(storage_snapshot: &[(usize, Vec<String>)]) -> bool {
    all_peers_have_storage_lane_count(storage_snapshot, EXPANDED_PROVISIONED_LANES)
}

fn peer_has_contracted_profile(status: &PeerStatusSnapshot) -> bool {
    let primary_lane_active = status
        .lanes
        .iter()
        .any(|lane| lane.lane_id == PRIMARY_LANE_ID && lane.capacity > 0);
    let elastic_lane_deactivated = match status
        .lanes
        .iter()
        .find(|lane| lane.lane_id == ELASTIC_LANE_ID)
    {
        Some(lane) => lane.capacity == 0,
        None => true,
    };
    primary_lane_active && elastic_lane_deactivated
}

fn peers_with_contracted_profile(snapshot: &[PeerStatusSnapshot]) -> usize {
    snapshot
        .iter()
        .filter(|status| peer_has_contracted_profile(status))
        .count()
}

fn contraction_observed_on_quorum_peers(
    status_snapshot: &[PeerStatusSnapshot],
    quorum_required: usize,
) -> bool {
    peers_with_contracted_profile(status_snapshot) >= quorum_required
}

fn max_non_empty_height(snapshot: &[PeerStatusSnapshot]) -> u64 {
    snapshot
        .iter()
        .map(|status| status.blocks_non_empty)
        .max()
        .unwrap_or_default()
}

fn max_txs_approved(snapshot: &[PeerStatusSnapshot]) -> u64 {
    snapshot
        .iter()
        .map(|status| status.txs_approved)
        .max()
        .unwrap_or_default()
}

fn max_txs_rejected(snapshot: &[PeerStatusSnapshot]) -> u64 {
    snapshot
        .iter()
        .map(|status| status.txs_rejected)
        .max()
        .unwrap_or_default()
}

fn chain_progress_advanced(
    snapshot: &[PeerStatusSnapshot],
    baseline_non_empty: u64,
    baseline_txs_approved: u64,
    baseline_txs_rejected: u64,
) -> bool {
    max_non_empty_height(snapshot) > baseline_non_empty
        || max_txs_approved(snapshot) > baseline_txs_approved
        || max_txs_rejected(snapshot) > baseline_txs_rejected
}

fn wait_for_commit_quorum_required(
    network: &sandbox::SerializedNetwork,
    timeout: Duration,
    context: &str,
) -> Result<usize> {
    let started = Instant::now();
    let mut last_error = None::<String>;
    let mut last_observed_required = Vec::<u64>::new();
    let mut last_observed_validator_set_len = Vec::<u64>::new();
    let mut consecutive_failures = 0_u32;

    while started.elapsed() <= timeout {
        match status_snapshot(network) {
            Ok(snapshot) => {
                consecutive_failures = 0;
                let observed_required = snapshot
                    .iter()
                    .map(|status| status.commit_signatures_required)
                    .filter(|value| *value > 0)
                    .collect::<Vec<_>>();
                last_observed_required = observed_required.clone();

                if !observed_required.is_empty() {
                    let min_required = *observed_required.iter().min().unwrap_or(&0);
                    let max_required = *observed_required.iter().max().unwrap_or(&0);
                    if min_required == max_required {
                        let quorum_required = usize::try_from(max_required).map_err(|_| {
                            eyre!("{context}: quorum value does not fit usize: {max_required}")
                        })?;
                        ensure!(
                            (1..=network.peers().len()).contains(&quorum_required),
                            "{context}: invalid commit quorum value {quorum_required} for peer count {}; observed values: {observed_required:?}",
                            network.peers().len()
                        );
                        return Ok(quorum_required);
                    }
                }

                let observed_validator_set_len = snapshot
                    .iter()
                    .map(|status| status.commit_qc_validator_set_len)
                    .filter(|value| *value > 0)
                    .collect::<Vec<_>>();
                last_observed_validator_set_len = observed_validator_set_len.clone();
                if !observed_validator_set_len.is_empty() {
                    let min_len = *observed_validator_set_len.iter().min().unwrap_or(&0);
                    let max_len = *observed_validator_set_len.iter().max().unwrap_or(&0);
                    if min_len == max_len {
                        let validator_set_len = usize::try_from(max_len).map_err(|_| {
                            eyre!("{context}: validator set length does not fit usize: {max_len}")
                        })?;
                        let quorum_required = commit_quorum_from_len(validator_set_len);
                        ensure!(
                            (1..=network.peers().len()).contains(&quorum_required),
                            "{context}: invalid derived quorum {quorum_required} from validator set len {validator_set_len} for peer count {}",
                            network.peers().len()
                        );
                        return Ok(quorum_required);
                    }
                }

                thread::sleep(STATUS_SNAPSHOT_RETRY_BACKOFF);
            }
            Err(err) => {
                last_error = Some(err.to_string());
                consecutive_failures = consecutive_failures.saturating_add(1);
                if consecutive_failures >= STATUS_SNAPSHOT_RETRY_LIMIT {
                    return Err(eyre!(
                        "{context}: status snapshot failed {} consecutive times; last error: {last_error:?}",
                        STATUS_SNAPSHOT_RETRY_LIMIT
                    ));
                }
                thread::sleep(STATUS_SNAPSHOT_RETRY_BACKOFF);
            }
        }
    }

    let fallback_quorum = commit_quorum_from_len(network.peers().len());
    eprintln!(
        "[autoscale-localnet] commit quorum fallback from peer count: {} (context: {context}; last required={last_observed_required:?}; last validator_set_len={last_observed_validator_set_len:?}; last error={last_error:?})",
        fallback_quorum
    );
    Ok(fallback_quorum)
}

fn wait_for_storage_lane_count(
    network: &sandbox::SerializedNetwork,
    expected_count: usize,
    timeout: Duration,
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_storage_snapshot = Vec::new();
    while started.elapsed() <= timeout {
        let storage_snapshot = lane_snapshot(network)?;
        if all_peers_have_storage_lane_count(&storage_snapshot, expected_count) {
            return Ok(());
        }
        last_storage_snapshot = storage_snapshot;
        thread::sleep(LANE_POLL_INTERVAL);
    }

    Err(eyre!(
        "{context}: timed out waiting for {expected_count} provisioned lane directories on all peers; last storage snapshot: {last_storage_snapshot:?}"
    ))
}

fn submit_load_round_robin(clients: &[Client], tx_count: usize) -> Result<()> {
    ensure!(
        !clients.is_empty(),
        "load submission requires at least one client"
    );

    for tx in 0..tx_count {
        let client = &clients[tx % clients.len()];
        let load_sequence = LOAD_TX_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        client
            .submit(Log::new(
                Level::INFO,
                format!("autoscale-load-{load_sequence}"),
            ))
            .map_err(|err| eyre!("submit autoscale load transaction {tx} failed: {err}"))?;
    }
    Ok(())
}

fn expansion_top_up_tx_count(heartbeat_seq: u64) -> usize {
    if heartbeat_seq == 0 {
        return 0;
    }
    if heartbeat_seq % EXPANSION_REINFORCE_EVERY_HEARTBEATS == 0 {
        return EXPANSION_REINFORCE_TX_COUNT;
    }
    if heartbeat_seq % EXPANSION_TOP_UP_EVERY_HEARTBEATS == 0 {
        return EXPANSION_TOP_UP_TX_COUNT;
    }
    0
}

fn expansion_probe_top_up_tx_count(
    heartbeat_seq: u64,
    storage_expanded: bool,
    elapsed: Duration,
) -> usize {
    if storage_expanded && elapsed >= EXPANSION_STATUS_SIGNAL_GRACE {
        return EXPANSION_POST_STORAGE_TOP_UP_TX_COUNT;
    }
    expansion_top_up_tx_count(heartbeat_seq)
}

fn wait_for_expanded_lanes_with_heartbeat(
    network: &sandbox::SerializedNetwork,
    heartbeat_client: &Client,
    top_up_clients: &[Client],
    baseline_status_snapshot: &[PeerStatusSnapshot],
    quorum_required: usize,
    timeout: Duration,
    context: &str,
    heartbeat_prefix: &str,
    heartbeat_interval: Duration,
) -> Result<()> {
    let started = Instant::now();
    let mut heartbeat_seq = 0_u64;
    let mut last_storage_snapshot = Vec::new();
    let mut last_status_snapshot = Vec::new();
    let mut last_heartbeat_error = None::<String>;
    let mut last_top_up_error = None::<String>;
    let mut last_status_error = None::<String>;
    let mut consecutive_status_failures = 0_u32;
    let mut post_grace_wait_logged = false;

    while started.elapsed() <= timeout {
        let storage_snapshot = lane_snapshot(network)?;
        let storage_expanded = expansion_observed_on_storage(&storage_snapshot);
        let elapsed = started.elapsed();
        let fallback_ready_at =
            EXPANSION_STATUS_SIGNAL_GRACE + EXPANSION_POST_STORAGE_STATUS_WINDOW;
        let status_snapshot = match status_snapshot(network) {
            Ok(snapshot) => {
                consecutive_status_failures = 0;
                snapshot
            }
            Err(err) => {
                last_status_error = Some(err.to_string());
                consecutive_status_failures = consecutive_status_failures.saturating_add(1);
                if consecutive_status_failures >= STATUS_SNAPSHOT_RETRY_LIMIT {
                    return Err(eyre!(
                        "{context}: status snapshot failed {} consecutive times while waiting for expansion; last status error: {last_status_error:?}; last storage snapshot: {last_storage_snapshot:?}; last heartbeat error: {last_heartbeat_error:?}; last top-up error: {last_top_up_error:?}",
                        STATUS_SNAPSHOT_RETRY_LIMIT
                    ));
                }
                if storage_expanded && elapsed >= fallback_ready_at {
                    eprintln!(
                        "[autoscale-localnet] {context}: expansion observed via storage lane provisioning fallback after status errors (grace {:?} + post-storage status window {:?})",
                        EXPANSION_STATUS_SIGNAL_GRACE, EXPANSION_POST_STORAGE_STATUS_WINDOW
                    );
                    return Ok(());
                }
                if storage_expanded
                    && elapsed >= EXPANSION_STATUS_SIGNAL_GRACE
                    && !post_grace_wait_logged
                {
                    eprintln!(
                        "[autoscale-localnet] {context}: storage expansion reached grace window; continuing status probe for {:?} before fallback",
                        EXPANSION_POST_STORAGE_STATUS_WINDOW
                    );
                    post_grace_wait_logged = true;
                }
                last_storage_snapshot = storage_snapshot;
                if let Err(heartbeat_err) = heartbeat_client.submit(Log::new(
                    Level::INFO,
                    format!("{heartbeat_prefix}-{heartbeat_seq}"),
                )) {
                    last_heartbeat_error = Some(heartbeat_err.to_string());
                }
                heartbeat_seq = heartbeat_seq.saturating_add(1);
                let top_up_tx_count =
                    expansion_probe_top_up_tx_count(heartbeat_seq, storage_expanded, elapsed);
                if top_up_tx_count > 0 {
                    if let Err(top_up_err) =
                        submit_load_round_robin(top_up_clients, top_up_tx_count)
                    {
                        last_top_up_error = Some(top_up_err.to_string());
                    }
                }
                thread::sleep(heartbeat_interval);
                continue;
            }
        };

        if expansion_observed_on_quorum_peers(
            &status_snapshot,
            Some(baseline_status_snapshot),
            quorum_required,
        ) {
            eprintln!(
                "[autoscale-localnet] {context}: expansion observed via status lane activity/lifecycle transitions"
            );
            return Ok(());
        }
        if storage_expanded && elapsed >= fallback_ready_at {
            let peers_with_status_signal = peers_with_expanded_lane_signal(
                &status_snapshot,
                Some(baseline_status_snapshot),
                ELASTIC_LANE_ID,
            );
            let signal_breakdown = expansion_signal_breakdown(
                &status_snapshot,
                Some(baseline_status_snapshot),
                ELASTIC_LANE_ID,
            );
            eprintln!(
                "[autoscale-localnet] {context}: expansion observed via storage lane provisioning fallback after {:.3}s (status signal {peers_with_status_signal}/{quorum_required}, grace {:?} + post-storage status window {:?}); signal breakdown: {signal_breakdown:?}",
                elapsed.as_secs_f64(),
                EXPANSION_STATUS_SIGNAL_GRACE,
                EXPANSION_POST_STORAGE_STATUS_WINDOW
            );
            return Ok(());
        }
        if storage_expanded && elapsed >= EXPANSION_STATUS_SIGNAL_GRACE && !post_grace_wait_logged {
            eprintln!(
                "[autoscale-localnet] {context}: storage expansion reached grace window; continuing status probe for {:?} before fallback",
                EXPANSION_POST_STORAGE_STATUS_WINDOW
            );
            post_grace_wait_logged = true;
        }
        last_storage_snapshot = storage_snapshot;
        last_status_snapshot = status_snapshot;

        if let Err(err) = heartbeat_client.submit(Log::new(
            Level::INFO,
            format!("{heartbeat_prefix}-{heartbeat_seq}"),
        )) {
            last_heartbeat_error = Some(err.to_string());
        }
        heartbeat_seq = heartbeat_seq.saturating_add(1);

        let top_up_tx_count =
            expansion_probe_top_up_tx_count(heartbeat_seq, storage_expanded, elapsed);
        if top_up_tx_count > 0 {
            if let Err(err) = submit_load_round_robin(top_up_clients, top_up_tx_count) {
                last_top_up_error = Some(err.to_string());
            }
        }
        thread::sleep(heartbeat_interval);
    }

    Err(eyre!(
        "{context}: timed out waiting for expanded lane profile (lane {ELASTIC_LANE_ID} active via status `capacity>0 || committed>0`, sumeragi lane commitment `tx_count>0 || teu_total>0`, public-lane validator lifecycle activity (`active || pending_activation`), or baseline transition via lane declaration/progress on >= {quorum_required}/{TOTAL_PEERS} peers; storage lane count={EXPANDED_PROVISIONED_LANES} accepted only as fallback after grace {:?} + post-storage status window {:?}); last status snapshot: {last_status_snapshot:?}; last storage snapshot: {last_storage_snapshot:?}; last status error: {last_status_error:?}; last heartbeat error: {last_heartbeat_error:?}; last top-up error: {last_top_up_error:?}",
        EXPANSION_STATUS_SIGNAL_GRACE,
        EXPANSION_POST_STORAGE_STATUS_WINDOW
    ))
}

fn wait_for_chain_progress_with_heartbeat(
    network: &sandbox::SerializedNetwork,
    heartbeat_client: &Client,
    baseline_non_empty: u64,
    baseline_txs_approved: u64,
    baseline_txs_rejected: u64,
    timeout: Duration,
    context: &str,
    heartbeat_prefix: &str,
    heartbeat_interval: Duration,
) -> Result<()> {
    let started = Instant::now();
    let mut heartbeat_seq = 0_u64;
    let mut last_status_snapshot = Vec::new();
    let mut last_status_error = None::<String>;
    let mut last_heartbeat_error = None::<String>;
    let mut consecutive_status_failures = 0_u32;

    while started.elapsed() <= timeout {
        match status_snapshot(network) {
            Ok(snapshot) => {
                consecutive_status_failures = 0;
                if chain_progress_advanced(
                    &snapshot,
                    baseline_non_empty,
                    baseline_txs_approved,
                    baseline_txs_rejected,
                ) {
                    return Ok(());
                }
                last_status_snapshot = snapshot;
            }
            Err(err) => {
                last_status_error = Some(err.to_string());
                consecutive_status_failures = consecutive_status_failures.saturating_add(1);
                if consecutive_status_failures >= STATUS_SNAPSHOT_RETRY_LIMIT {
                    return Err(eyre!(
                        "{context}: status snapshot failed {} consecutive times while waiting for chain progress; last status error: {last_status_error:?}; last status snapshot: {last_status_snapshot:?}; last heartbeat error: {last_heartbeat_error:?}",
                        STATUS_SNAPSHOT_RETRY_LIMIT
                    ));
                }
            }
        }

        if let Err(err) = heartbeat_client.submit(Log::new(
            Level::INFO,
            format!("{heartbeat_prefix}-{heartbeat_seq}"),
        )) {
            last_heartbeat_error = Some(err.to_string());
        }
        heartbeat_seq = heartbeat_seq.saturating_add(1);
        thread::sleep(heartbeat_interval);
    }

    Err(eyre!(
        "{context}: timed out waiting for chain progress (baseline blocks_non_empty={baseline_non_empty}, txs_approved={baseline_txs_approved}, txs_rejected={baseline_txs_rejected}); last status snapshot: {last_status_snapshot:?}; last status error: {last_status_error:?}; last heartbeat error: {last_heartbeat_error:?}"
    ))
}

fn wait_for_contracted_lanes_with_heartbeat(
    network: &sandbox::SerializedNetwork,
    heartbeat_client: &Client,
    quorum_required: usize,
    timeout: Duration,
    context: &str,
    heartbeat_prefix: &str,
    heartbeat_interval: Duration,
) -> Result<()> {
    let started = Instant::now();
    let mut heartbeat_seq = 0_u64;
    let mut last_storage_snapshot = Vec::new();
    let mut last_status_snapshot = Vec::new();
    let mut last_heartbeat_error = None::<String>;
    let mut last_status_error = None::<String>;
    let mut consecutive_status_failures = 0_u32;

    while started.elapsed() <= timeout {
        let status_snapshot = match status_snapshot(network) {
            Ok(snapshot) => {
                consecutive_status_failures = 0;
                snapshot
            }
            Err(err) => {
                last_status_error = Some(err.to_string());
                consecutive_status_failures = consecutive_status_failures.saturating_add(1);
                if consecutive_status_failures >= STATUS_SNAPSHOT_RETRY_LIMIT {
                    return Err(eyre!(
                        "{context}: status snapshot failed {} consecutive times while waiting for contraction; last status error: {last_status_error:?}; last status snapshot: {last_status_snapshot:?}; last storage snapshot: {last_storage_snapshot:?}; last heartbeat error: {last_heartbeat_error:?}",
                        STATUS_SNAPSHOT_RETRY_LIMIT
                    ));
                }
                if let Err(heartbeat_err) = heartbeat_client.submit(Log::new(
                    Level::INFO,
                    format!("{heartbeat_prefix}-{heartbeat_seq}"),
                )) {
                    last_heartbeat_error = Some(heartbeat_err.to_string());
                }
                heartbeat_seq = heartbeat_seq.saturating_add(1);
                thread::sleep(heartbeat_interval);
                continue;
            }
        };
        if contraction_observed_on_quorum_peers(&status_snapshot, quorum_required) {
            return Ok(());
        }
        last_status_snapshot = status_snapshot;
        last_storage_snapshot = lane_snapshot(network).unwrap_or_default();

        if let Err(err) = heartbeat_client.submit(Log::new(
            Level::INFO,
            format!("{heartbeat_prefix}-{heartbeat_seq}"),
        )) {
            last_heartbeat_error = Some(err.to_string());
        }
        heartbeat_seq = heartbeat_seq.saturating_add(1);
        thread::sleep(heartbeat_interval);
    }

    Err(eyre!(
        "{context}: timed out waiting for contracted lane profile (lane {PRIMARY_LANE_ID} active; lane {ELASTIC_LANE_ID} removed or capacity=0 on >= {quorum_required}/{TOTAL_PEERS} peers) with heartbeat; last status snapshot: {last_status_snapshot:?}; last storage snapshot: {last_storage_snapshot:?}; last status error: {last_status_error:?}; last heartbeat error: {last_heartbeat_error:?}"
    ))
}

fn run_expand_contract_cycle(
    network: &sandbox::SerializedNetwork,
    submitters: &[Client],
    quorum_required: usize,
    cycle_index: usize,
) -> Result<()> {
    let pre_cycle_heartbeat_client = peer_client_with_timeout(network.peer());
    let pre_contraction_context = format!("autoscale contraction pre-check cycle {cycle_index}");
    let pre_contraction_prefix = format!("autoscale-precheck-heartbeat-cycle-{cycle_index}");
    wait_for_contracted_lanes_with_heartbeat(
        network,
        &pre_cycle_heartbeat_client,
        quorum_required,
        SCALE_IN_WAIT_TIMEOUT,
        &pre_contraction_context,
        &pre_contraction_prefix,
        CONTRACTION_HEARTBEAT_INTERVAL,
    )?;

    let pre_cycle_status = status_snapshot(network)?;
    let pre_cycle_max_non_empty_height = max_non_empty_height(&pre_cycle_status);
    let pre_cycle_max_txs_approved = max_txs_approved(&pre_cycle_status);
    let pre_cycle_max_txs_rejected = max_txs_rejected(&pre_cycle_status);

    let load_started = Instant::now();
    submit_load_round_robin(submitters, LOAD_TX_COUNT)?;
    eprintln!(
        "[autoscale-localnet][cycle {cycle_index}] load submission ({} tx): {:.3}s",
        LOAD_TX_COUNT,
        load_started.elapsed().as_secs_f64()
    );
    let activity_probe_client = peer_client_with_timeout(network.peer());
    let activity_context = format!("autoscale activity cycle {cycle_index}");
    let activity_prefix = format!("autoscale-activity-heartbeat-cycle-{cycle_index}");
    wait_for_chain_progress_with_heartbeat(
        network,
        &activity_probe_client,
        pre_cycle_max_non_empty_height,
        pre_cycle_max_txs_approved,
        pre_cycle_max_txs_rejected,
        SCALE_OUT_WAIT_TIMEOUT,
        &activity_context,
        &activity_prefix,
        EXPANSION_PROBE_INTERVAL,
    )?;

    let expansion_probe_client = peer_client_with_timeout(network.peer());
    let expansion_started = Instant::now();
    let expansion_context = format!("autoscale expansion cycle {cycle_index}");
    let expansion_prefix = format!("autoscale-expand-probe-cycle-{cycle_index}");
    wait_for_expanded_lanes_with_heartbeat(
        network,
        &expansion_probe_client,
        submitters,
        &pre_cycle_status,
        quorum_required,
        SCALE_OUT_WAIT_TIMEOUT,
        &expansion_context,
        &expansion_prefix,
        EXPANSION_PROBE_INTERVAL,
    )?;
    eprintln!(
        "[autoscale-localnet][cycle {cycle_index}] expansion wait: {:.3}s",
        expansion_started.elapsed().as_secs_f64()
    );

    let heartbeat_client = peer_client_with_timeout(network.peer());
    let contraction_started = Instant::now();
    let contraction_context = format!("autoscale contraction cycle {cycle_index}");
    let contraction_prefix = format!("autoscale-heartbeat-cycle-{cycle_index}");
    wait_for_contracted_lanes_with_heartbeat(
        network,
        &heartbeat_client,
        quorum_required,
        SCALE_IN_WAIT_TIMEOUT,
        &contraction_context,
        &contraction_prefix,
        CONTRACTION_HEARTBEAT_INTERVAL,
    )?;
    eprintln!(
        "[autoscale-localnet][cycle {cycle_index}] contraction wait: {:.3}s",
        contraction_started.elapsed().as_secs_f64()
    );
    let post_cycle_status = status_snapshot(network)?;
    let post_cycle_max_non_empty_height = max_non_empty_height(&post_cycle_status);
    let post_cycle_max_txs_approved = max_txs_approved(&post_cycle_status);
    let post_cycle_max_txs_rejected = max_txs_rejected(&post_cycle_status);
    ensure!(
        chain_progress_advanced(
            &post_cycle_status,
            pre_cycle_max_non_empty_height,
            pre_cycle_max_txs_approved,
            pre_cycle_max_txs_rejected
        ),
        "autoscale cycle {cycle_index}: chain activity did not advance (blocks_non_empty: {pre_cycle_max_non_empty_height}->{post_cycle_max_non_empty_height}, txs_approved: {pre_cycle_max_txs_approved}->{post_cycle_max_txs_approved}, txs_rejected: {pre_cycle_max_txs_rejected}->{post_cycle_max_txs_rejected})"
    );

    Ok(())
}

#[test]
fn nexus_autoscale_expands_and_contracts_lanes_in_localnet() -> Result<()> {
    let context = stringify!(nexus_autoscale_expands_and_contracts_lanes_in_localnet);
    let test_started = Instant::now();
    let startup_started = Instant::now();
    let Some((network, _rt)) =
        sandbox::start_network_blocking_or_skip(autoscale_localnet_builder(), context)?
    else {
        return Ok(());
    };
    eprintln!(
        "[autoscale-localnet] network startup: {:.3}s",
        startup_started.elapsed().as_secs_f64()
    );

    ensure!(
        network.peers().len() == TOTAL_PEERS,
        "expected {TOTAL_PEERS} peers, got {}",
        network.peers().len()
    );

    let baseline_started = Instant::now();
    wait_for_storage_lane_count(
        &network,
        INITIAL_PROVISIONED_LANES,
        SCALE_OUT_WAIT_TIMEOUT,
        "baseline lane count",
    )?;
    eprintln!(
        "[autoscale-localnet] baseline lane count wait: {:.3}s",
        baseline_started.elapsed().as_secs_f64()
    );

    let submitters: Vec<Client> = network
        .peers()
        .iter()
        .map(peer_client_with_timeout)
        .collect();
    let quorum_required = wait_for_commit_quorum_required(
        &network,
        QUORUM_DISCOVERY_TIMEOUT,
        "discover autoscale commit quorum",
    )?;
    eprintln!("[autoscale-localnet] dynamic commit quorum (2f+1): {quorum_required}");

    run_expand_contract_cycle(&network, &submitters, quorum_required, 1)?;
    eprintln!(
        "[autoscale-localnet] total runtime: {:.3}s",
        test_started.elapsed().as_secs_f64()
    );

    Ok(())
}

#[test]
fn nexus_autoscale_repeats_expand_contract_cycles_in_localnet() -> Result<()> {
    let context = stringify!(nexus_autoscale_repeats_expand_contract_cycles_in_localnet);
    let test_started = Instant::now();
    let startup_started = Instant::now();
    let Some((network, _rt)) =
        sandbox::start_network_blocking_or_skip(autoscale_localnet_builder(), context)?
    else {
        return Ok(());
    };
    eprintln!(
        "[autoscale-localnet][multi-cycle] network startup: {:.3}s",
        startup_started.elapsed().as_secs_f64()
    );

    ensure!(
        network.peers().len() == TOTAL_PEERS,
        "expected {TOTAL_PEERS} peers, got {}",
        network.peers().len()
    );

    let baseline_started = Instant::now();
    wait_for_storage_lane_count(
        &network,
        INITIAL_PROVISIONED_LANES,
        SCALE_OUT_WAIT_TIMEOUT,
        "baseline lane count for repeated cycles",
    )?;
    eprintln!(
        "[autoscale-localnet][multi-cycle] baseline lane count wait: {:.3}s",
        baseline_started.elapsed().as_secs_f64()
    );

    let submitters: Vec<Client> = network
        .peers()
        .iter()
        .map(peer_client_with_timeout)
        .collect();
    let quorum_required = wait_for_commit_quorum_required(
        &network,
        QUORUM_DISCOVERY_TIMEOUT,
        "discover autoscale commit quorum for repeated cycles",
    )?;
    eprintln!("[autoscale-localnet][multi-cycle] dynamic commit quorum (2f+1): {quorum_required}");

    run_expand_contract_cycle(&network, &submitters, quorum_required, 1)?;
    run_expand_contract_cycle(&network, &submitters, quorum_required, 2)?;

    eprintln!(
        "[autoscale-localnet][multi-cycle] total runtime: {:.3}s",
        test_started.elapsed().as_secs_f64()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        LaneCommitmentSnapshot, LaneStatusSnapshot, LaneValidatorSnapshot, PeerStatusSnapshot,
        contraction_observed_on_quorum_peers, expansion_observed_on_quorum_peers,
        expansion_observed_on_storage, expansion_probe_top_up_tx_count, expansion_top_up_tx_count,
    };

    #[test]
    fn expansion_top_up_profile_is_deterministic() {
        assert_eq!(expansion_top_up_tx_count(0), 0);
        assert_eq!(expansion_top_up_tx_count(1), 0);
        assert_eq!(expansion_top_up_tx_count(3), 0);
        assert_eq!(expansion_top_up_tx_count(4), 16);
        assert_eq!(expansion_top_up_tx_count(8), 16);
        assert_eq!(expansion_top_up_tx_count(11), 0);
        assert_eq!(expansion_top_up_tx_count(12), 32);
        assert_eq!(expansion_top_up_tx_count(24), 32);
    }

    #[test]
    fn expansion_probe_top_up_intensifies_after_storage_grace() {
        assert_eq!(
            expansion_probe_top_up_tx_count(4, false, Duration::from_secs(8)),
            expansion_top_up_tx_count(4)
        );
        assert_eq!(
            expansion_probe_top_up_tx_count(4, true, Duration::from_secs(7)),
            expansion_top_up_tx_count(4)
        );
        assert_eq!(
            expansion_probe_top_up_tx_count(1, true, Duration::from_secs(8)),
            16
        );
        assert_eq!(
            expansion_probe_top_up_tx_count(7, true, Duration::from_secs(15)),
            16
        );
    }

    #[test]
    fn expansion_requires_active_lane_signal_on_quorum_peers() {
        let status_snapshot = vec![
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 5000,
                        committed: 4,
                    },
                ],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 4000,
                        committed: 3,
                    },
                ],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 3000,
                        committed: 2,
                    },
                ],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
        ];

        assert!(expansion_observed_on_quorum_peers(
            &status_snapshot,
            None,
            3
        ));
        assert!(!expansion_observed_on_quorum_peers(
            &status_snapshot,
            None,
            4
        ));

        let zero_capacity_snapshot = status_snapshot
            .iter()
            .cloned()
            .map(|mut peer| {
                for lane in &mut peer.lanes {
                    if lane.lane_id == 1 {
                        lane.capacity = 0;
                        lane.committed = 0;
                    }
                }
                peer
            })
            .collect::<Vec<_>>();
        assert!(!expansion_observed_on_quorum_peers(
            &zero_capacity_snapshot,
            None,
            3
        ));

        let committed_only_snapshot = status_snapshot
            .iter()
            .cloned()
            .map(|mut peer| {
                for lane in &mut peer.lanes {
                    if lane.lane_id == 1 {
                        lane.capacity = 0;
                        lane.committed = 1;
                    }
                }
                peer
            })
            .collect::<Vec<_>>();
        assert!(expansion_observed_on_quorum_peers(
            &committed_only_snapshot,
            None,
            3
        ));
    }

    #[test]
    fn expansion_accepts_sumeragi_lane_commitment_activity_on_quorum_peers() {
        let commitment_only_snapshot = vec![
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 0,
                        committed: 0,
                    },
                ],
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 10,
                    tx_count: 4,
                    teu_total: 128,
                }],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 0,
                        committed: 0,
                    },
                ],
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 10,
                    tx_count: 2,
                    teu_total: 64,
                }],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 0,
                        committed: 0,
                    },
                ],
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 10,
                    tx_count: 1,
                    teu_total: 32,
                }],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
        ];

        assert!(expansion_observed_on_quorum_peers(
            &commitment_only_snapshot,
            None,
            3
        ));
        assert!(!expansion_observed_on_quorum_peers(
            &commitment_only_snapshot,
            None,
            4
        ));

        let zero_commitment_activity = commitment_only_snapshot
            .iter()
            .cloned()
            .map(|mut peer| {
                for commitment in &mut peer.lane_commitments {
                    commitment.tx_count = 0;
                    commitment.teu_total = 0;
                }
                peer
            })
            .collect::<Vec<_>>();
        assert!(!expansion_observed_on_quorum_peers(
            &zero_commitment_activity,
            None,
            3
        ));
    }

    #[test]
    fn expansion_accepts_lane_declaration_transition_on_quorum_peers() {
        let baseline_snapshot = vec![
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
        ];

        let declaration_transition_snapshot = vec![
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
        ];

        assert!(expansion_observed_on_quorum_peers(
            &declaration_transition_snapshot,
            Some(&baseline_snapshot),
            3
        ));
        assert!(!expansion_observed_on_quorum_peers(
            &declaration_transition_snapshot,
            None,
            3
        ));
    }

    #[test]
    fn expansion_accepts_lane_progress_transition_on_quorum_peers() {
        let baseline_snapshot = vec![
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 0,
                        committed: 0,
                    },
                ],
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 10,
                    tx_count: 0,
                    teu_total: 0,
                }],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 0,
                        committed: 0,
                    },
                ],
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 10,
                    tx_count: 0,
                    teu_total: 0,
                }],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 0,
                        committed: 0,
                    },
                ],
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 10,
                    tx_count: 0,
                    teu_total: 0,
                }],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 6000,
                        committed: 12,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 0,
                        committed: 0,
                    },
                ],
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 10,
                    tx_count: 0,
                    teu_total: 0,
                }],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
        ];

        let progress_transition_snapshot = vec![
            PeerStatusSnapshot {
                lanes: baseline_snapshot[0].lanes.clone(),
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 11,
                    tx_count: 0,
                    teu_total: 0,
                }],
                lane_governance_ids: baseline_snapshot[0].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: baseline_snapshot[1].lanes.clone(),
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 11,
                    tx_count: 0,
                    teu_total: 0,
                }],
                lane_governance_ids: baseline_snapshot[1].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: baseline_snapshot[2].lanes.clone(),
                lane_commitments: vec![LaneCommitmentSnapshot {
                    lane_id: 1,
                    block_height: 11,
                    tx_count: 0,
                    teu_total: 0,
                }],
                lane_governance_ids: baseline_snapshot[2].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: baseline_snapshot[3].lanes.clone(),
                lane_commitments: baseline_snapshot[3].lane_commitments.clone(),
                lane_governance_ids: baseline_snapshot[3].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
        ];

        assert!(expansion_observed_on_quorum_peers(
            &progress_transition_snapshot,
            Some(&baseline_snapshot),
            3
        ));
        assert!(!expansion_observed_on_quorum_peers(
            &progress_transition_snapshot,
            None,
            3
        ));
    }

    #[test]
    fn expansion_accepts_lane_validator_transition_on_quorum_peers() {
        let baseline_snapshot = vec![
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![LaneValidatorSnapshot {
                    lane_id: 1,
                    total: 4,
                    active: 0,
                    pending_activation: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 100,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![LaneValidatorSnapshot {
                    lane_id: 1,
                    total: 4,
                    active: 0,
                    pending_activation: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 100,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![LaneValidatorSnapshot {
                    lane_id: 1,
                    total: 4,
                    active: 0,
                    pending_activation: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 100,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 6000,
                    committed: 12,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![0, 1],
                lane_relay: vec![],
                lane_validators: vec![LaneValidatorSnapshot {
                    lane_id: 1,
                    total: 4,
                    active: 0,
                    pending_activation: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 100,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
        ];

        let validator_transition_snapshot = vec![
            PeerStatusSnapshot {
                lanes: baseline_snapshot[0].lanes.clone(),
                lane_commitments: baseline_snapshot[0].lane_commitments.clone(),
                lane_governance_ids: baseline_snapshot[0].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: vec![LaneValidatorSnapshot {
                    lane_id: 1,
                    total: 4,
                    active: 3,
                    pending_activation: 0,
                    max_activation_epoch: 1,
                    max_activation_height: 100,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: baseline_snapshot[1].lanes.clone(),
                lane_commitments: baseline_snapshot[1].lane_commitments.clone(),
                lane_governance_ids: baseline_snapshot[1].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: vec![LaneValidatorSnapshot {
                    lane_id: 1,
                    total: 4,
                    active: 2,
                    pending_activation: 1,
                    max_activation_epoch: 1,
                    max_activation_height: 101,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: baseline_snapshot[2].lanes.clone(),
                lane_commitments: baseline_snapshot[2].lane_commitments.clone(),
                lane_governance_ids: baseline_snapshot[2].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: vec![LaneValidatorSnapshot {
                    lane_id: 1,
                    total: 4,
                    active: 1,
                    pending_activation: 0,
                    max_activation_epoch: 2,
                    max_activation_height: 102,
                }],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
            PeerStatusSnapshot {
                lanes: baseline_snapshot[3].lanes.clone(),
                lane_commitments: baseline_snapshot[3].lane_commitments.clone(),
                lane_governance_ids: baseline_snapshot[3].lane_governance_ids.clone(),
                lane_relay: vec![],
                lane_validators: baseline_snapshot[3].lane_validators.clone(),
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 11,
                txs_rejected: 0,
                blocks_non_empty: 11,
            },
        ];

        assert!(expansion_observed_on_quorum_peers(
            &validator_transition_snapshot,
            Some(&baseline_snapshot),
            3
        ));
        assert!(!expansion_observed_on_quorum_peers(
            &validator_transition_snapshot,
            None,
            3
        ));
    }

    #[test]
    fn expansion_storage_requires_two_lanes_on_all_peers() {
        let expanded_storage = vec![
            (
                0,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
            (
                1,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
            (
                2,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
            (
                3,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
        ];
        assert!(expansion_observed_on_storage(&expanded_storage));

        let partial_storage = vec![
            (
                0,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
            (
                1,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
            (2, vec!["lane_000_default".to_owned()]),
            (
                3,
                vec![
                    "lane_000_default".to_owned(),
                    "lane_001_elastic_lane_1".to_owned(),
                ],
            ),
        ];
        assert!(!expansion_observed_on_storage(&partial_storage));
    }

    #[test]
    fn contraction_profile_uses_quorum_threshold() {
        let absent_elastic = vec![
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 9000,
                    committed: 10,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![
                    LaneStatusSnapshot {
                        lane_id: 0,
                        capacity: 8000,
                        committed: 10,
                    },
                    LaneStatusSnapshot {
                        lane_id: 1,
                        capacity: 0,
                        committed: 0,
                    },
                ],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
            PeerStatusSnapshot {
                lanes: vec![LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 7000,
                    committed: 9,
                }],
                lane_commitments: vec![],
                lane_governance_ids: vec![],
                lane_relay: vec![],
                lane_validators: vec![],
                commit_signatures_required: 3,
                commit_qc_validator_set_len: 4,
                txs_approved: 10,
                txs_rejected: 0,
                blocks_non_empty: 10,
            },
        ];
        assert!(contraction_observed_on_quorum_peers(&absent_elastic, 3));
        assert!(!contraction_observed_on_quorum_peers(&absent_elastic, 4));

        let elastic_still_active = vec![PeerStatusSnapshot {
            lanes: vec![
                LaneStatusSnapshot {
                    lane_id: 0,
                    capacity: 9000,
                    committed: 10,
                },
                LaneStatusSnapshot {
                    lane_id: 1,
                    capacity: 1,
                    committed: 1,
                },
            ],
            lane_commitments: vec![],
            lane_governance_ids: vec![],
            lane_relay: vec![],
            lane_validators: vec![],
            commit_signatures_required: 3,
            commit_qc_validator_set_len: 4,
            txs_approved: 10,
            txs_rejected: 0,
            blocks_non_empty: 10,
        }];
        assert!(!contraction_observed_on_quorum_peers(
            &elastic_still_active,
            1
        ));
    }
}
