#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Localnet autoscale regression test for Nexus lane expansion/contraction.

use std::{
    fs, thread,
    time::{Duration, Instant},
};

use eyre::{Result, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    data_model::{Level, isi::Log},
};
use iroha_test_network::{NetworkBuilder, NetworkPeer};

const TOTAL_PEERS: usize = 4;
const INITIAL_PROVISIONED_LANES: usize = 1;
const SCALED_PROVISIONED_LANES: usize = 2;
const PRIMARY_LANE_ID: u32 = 0;
const ELASTIC_LANE_ID: u32 = 1;
const LOAD_TX_COUNT: usize = 48;
const LANE_POLL_INTERVAL: Duration = Duration::from_millis(250);
const EXPANSION_PROBE_INTERVAL: Duration = Duration::from_millis(1000);
const CONTRACTION_HEARTBEAT_INTERVAL: Duration = Duration::from_millis(1000);
const SCALE_OUT_WAIT_TIMEOUT: Duration = Duration::from_secs(120);
const SCALE_IN_WAIT_TIMEOUT: Duration = Duration::from_secs(180);
const TORII_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

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
                .write(["nexus", "autoscale", "scale_out_latency_ratio"], 0.8_f64)
                .write(["nexus", "autoscale", "scale_in_latency_ratio"], 0.75_f64)
                .write(
                    ["nexus", "autoscale", "scale_out_utilization_ratio"],
                    0.6_f64,
                )
                .write(
                    ["nexus", "autoscale", "scale_in_utilization_ratio"],
                    0.5_f64,
                )
                .write(["nexus", "autoscale", "scale_out_window_blocks"], 3_i64)
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
}

#[derive(Clone, Debug)]
struct PeerStatusSnapshot {
    lanes: Vec<LaneStatusSnapshot>,
}

fn status_lane_snapshot(network: &sandbox::SerializedNetwork) -> Result<Vec<PeerStatusSnapshot>> {
    network
        .peers()
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            let status = peer_client_with_timeout(peer)
                .get_status()
                .map_err(|err| eyre!("fetch peer {index} status failed: {err}"))?;
            let lanes = status
                .teu_lane_commit
                .iter()
                .map(|lane| LaneStatusSnapshot {
                    lane_id: lane.lane_id,
                    capacity: lane.capacity,
                })
                .collect::<Vec<_>>();
            Ok(PeerStatusSnapshot { lanes })
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

fn all_peers_show_contracted_profile(snapshot: &[PeerStatusSnapshot]) -> bool {
    snapshot.iter().all(|status| {
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
    })
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
        client
            .submit(Log::new(Level::INFO, format!("autoscale-load-{tx}")))
            .map_err(|err| eyre!("submit autoscale load transaction {tx} failed: {err}"))?;
    }
    Ok(())
}

fn wait_for_storage_lane_count_with_heartbeat(
    network: &sandbox::SerializedNetwork,
    heartbeat_client: &Client,
    expected_count: usize,
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

    while started.elapsed() <= timeout {
        let storage_snapshot = lane_snapshot(network)?;
        if all_peers_have_storage_lane_count(&storage_snapshot, expected_count) {
            return Ok(());
        }
        last_storage_snapshot = storage_snapshot;
        last_status_snapshot = status_lane_snapshot(network).unwrap_or_default();

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
        "{context}: timed out waiting for {expected_count} provisioned lane directories with heartbeat; last status snapshot: {last_status_snapshot:?}; last storage snapshot: {last_storage_snapshot:?}; last heartbeat error: {last_heartbeat_error:?}"
    ))
}

fn wait_for_contracted_lanes_with_heartbeat(
    network: &sandbox::SerializedNetwork,
    heartbeat_client: &Client,
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

    while started.elapsed() <= timeout {
        let status_snapshot = status_lane_snapshot(network)?;
        if all_peers_show_contracted_profile(&status_snapshot) {
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
        "{context}: timed out waiting for contracted lane profile (lane {PRIMARY_LANE_ID} active; lane {ELASTIC_LANE_ID} removed or capacity=0) with heartbeat; last status snapshot: {last_status_snapshot:?}; last storage snapshot: {last_storage_snapshot:?}; last heartbeat error: {last_heartbeat_error:?}"
    ))
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
    let load_started = Instant::now();
    submit_load_round_robin(&submitters, LOAD_TX_COUNT)?;
    eprintln!(
        "[autoscale-localnet] load submission ({} tx): {:.3}s",
        LOAD_TX_COUNT,
        load_started.elapsed().as_secs_f64()
    );

    let expansion_probe_client = peer_client_with_timeout(network.peer());
    let expansion_started = Instant::now();
    wait_for_storage_lane_count_with_heartbeat(
        &network,
        &expansion_probe_client,
        SCALED_PROVISIONED_LANES,
        SCALE_OUT_WAIT_TIMEOUT,
        "autoscale expansion",
        "autoscale-expand-probe",
        EXPANSION_PROBE_INTERVAL,
    )?;
    eprintln!(
        "[autoscale-localnet] expansion wait: {:.3}s",
        expansion_started.elapsed().as_secs_f64()
    );

    let heartbeat_client = peer_client_with_timeout(network.peer());
    let contraction_started = Instant::now();
    wait_for_contracted_lanes_with_heartbeat(
        &network,
        &heartbeat_client,
        SCALE_IN_WAIT_TIMEOUT,
        "autoscale contraction",
        "autoscale-heartbeat",
        CONTRACTION_HEARTBEAT_INTERVAL,
    )?;
    eprintln!(
        "[autoscale-localnet] contraction wait: {:.3}s",
        contraction_started.elapsed().as_secs_f64()
    );
    eprintln!(
        "[autoscale-localnet] total runtime: {:.3}s",
        test_started.elapsed().as_secs_f64()
    );

    Ok(())
}
