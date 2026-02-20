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
const INITIAL_ACTIVE_LANES: usize = 1;
const SCALED_ACTIVE_LANES: usize = 2;
const LOAD_TX_COUNT: usize = 64;
const LANE_POLL_INTERVAL: Duration = Duration::from_millis(250);
const EXPANSION_PROBE_INTERVAL: Duration = Duration::from_millis(1000);
const CONTRACTION_HEARTBEAT_INTERVAL: Duration = Duration::from_millis(1000);
const SCALE_OUT_WAIT_TIMEOUT: Duration = Duration::from_secs(120);
const SCALE_IN_WAIT_TIMEOUT: Duration = Duration::from_secs(180);

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

fn status_lane_snapshot(
    network: &sandbox::SerializedNetwork,
) -> Result<Vec<(usize, u64, u64, u64, Vec<u32>)>> {
    network
        .peers()
        .iter()
        .enumerate()
        .map(|(index, peer)| {
            let status = peer
                .client()
                .get_status()
                .map_err(|err| eyre!("fetch peer {index} status failed: {err}"))?;
            let lane_ids = status
                .teu_lane_commit
                .iter()
                .map(|lane| lane.lane_id)
                .collect::<Vec<_>>();
            Ok((
                index,
                status.blocks,
                status.commit_time_ms,
                status.queue_size,
                lane_ids,
            ))
        })
        .collect()
}

fn all_peers_have_lane_count(snapshot: &[(usize, Vec<String>)], expected: usize) -> bool {
    snapshot.iter().all(|(_, lanes)| lanes.len() == expected)
}

fn wait_for_lane_count(
    network: &sandbox::SerializedNetwork,
    expected: usize,
    timeout: Duration,
    context: &str,
) -> Result<()> {
    let started = Instant::now();
    let mut last_status_snapshot = Vec::new();
    let mut last_snapshot = Vec::new();
    while started.elapsed() <= timeout {
        let snapshot = lane_snapshot(network)?;
        if all_peers_have_lane_count(&snapshot, expected) {
            return Ok(());
        }
        last_snapshot = snapshot;
        last_status_snapshot = status_lane_snapshot(network).unwrap_or_default();
        thread::sleep(LANE_POLL_INTERVAL);
    }

    Err(eyre!(
        "{context}: timed out waiting for {expected} active lanes on all peers; last status snapshot: {last_status_snapshot:?}; last storage snapshot: {last_snapshot:?}"
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

fn wait_for_lane_count_with_heartbeat(
    network: &sandbox::SerializedNetwork,
    heartbeat_client: &Client,
    expected: usize,
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
        if all_peers_have_lane_count(&storage_snapshot, expected) {
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
        "{context}: timed out waiting for {expected} active lanes with heartbeat; last status snapshot: {last_status_snapshot:?}; last storage snapshot: {last_storage_snapshot:?}; last heartbeat error: {last_heartbeat_error:?}"
    ))
}

#[test]
fn nexus_autoscale_expands_and_contracts_lanes_in_localnet() -> Result<()> {
    let context = stringify!(nexus_autoscale_expands_and_contracts_lanes_in_localnet);
    let Some((network, _rt)) =
        sandbox::start_network_blocking_or_skip(autoscale_localnet_builder(), context)?
    else {
        return Ok(());
    };

    ensure!(
        network.peers().len() == TOTAL_PEERS,
        "expected {TOTAL_PEERS} peers, got {}",
        network.peers().len()
    );

    wait_for_lane_count(
        &network,
        INITIAL_ACTIVE_LANES,
        SCALE_OUT_WAIT_TIMEOUT,
        "baseline lane count",
    )?;

    let submitters: Vec<Client> = network.peers().iter().map(NetworkPeer::client).collect();
    submit_load_round_robin(&submitters, LOAD_TX_COUNT)?;

    let expansion_probe_client = network.peer().client();
    wait_for_lane_count_with_heartbeat(
        &network,
        &expansion_probe_client,
        SCALED_ACTIVE_LANES,
        SCALE_OUT_WAIT_TIMEOUT,
        "autoscale expansion",
        "autoscale-expand-probe",
        EXPANSION_PROBE_INTERVAL,
    )?;

    let heartbeat_client = network.peer().client();
    wait_for_lane_count_with_heartbeat(
        &network,
        &heartbeat_client,
        INITIAL_ACTIVE_LANES,
        SCALE_IN_WAIT_TIMEOUT,
        "autoscale contraction",
        "autoscale-heartbeat",
        CONTRACTION_HEARTBEAT_INTERVAL,
    )?;

    Ok(())
}
