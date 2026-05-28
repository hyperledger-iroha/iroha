#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration regression tests for Sumeragi PRF-based collector selection.

use std::{collections::HashSet, time::Duration};

use eyre::{WrapErr, ensure};
use integration_tests::sandbox;
use iroha_config::parameters::actual::ConsensusMode;
use iroha_core::sumeragi::{collectors::deterministic_collectors, network_topology::Topology};
use iroha_data_model::{Level, isi::Log, peer::PeerId};
use iroha_test_network::{NetworkBuilder, init_instruction_registry};
use norito::json::{self, Value};
use tokio::time::sleep;

/// Ensure `/v1/sumeragi/collectors` aligns with deterministic PRF-based selection and
/// rotates collectors as block heights advance.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn npos_prf_collectors_track_endpoint() -> eyre::Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "da", "enabled"], true)
                .write(["sumeragi", "collectors", "k"], 2_i64)
                .write(["sumeragi", "collectors", "redundant_send_r"], 1_i64);
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_prf_collectors_track_endpoint),
    )
    .await?
    else {
        return Ok(());
    };

    // Produce a handful of blocks so VRF commit/reveal data is available.
    let client = network.client();
    drive_network_to_total_height(&network, &client, 6, "prf seed").await?;

    let topology = topology_from_peers(network.peers());
    let collectors_url = client
        .torii_url
        .join("v1/sumeragi/collectors")
        .wrap_err("compose collectors URL")?;
    let http = integration_tests::http::client();

    let snapshot_initial = fetch_collectors_snapshot(&http, &collectors_url).await?;
    ensure!(
        snapshot_initial.mode == ConsensusMode::Npos,
        "collector snapshot should report NPoS mode"
    );

    verify_snapshot(&topology, &snapshot_initial)?;

    // Wait for at least one additional block to ensure collector rotation is observable.
    let status = client.get_status()?;
    let target_height =
        next_collectors_observation_height(status.blocks, snapshot_initial.plan_height);
    drive_network_to_total_height(&network, &client, target_height, "prf rotation tick").await?;

    let snapshot_next = retry_collectors_until_height(
        &http,
        &collectors_url,
        snapshot_initial.plan_height,
        snapshot_initial.plan_view,
        Duration::from_millis(250),
        80,
    )
    .await?;
    verify_snapshot(&topology, &snapshot_next)?;

    // When the height advances the collector plan should originate from either a higher
    // height or a higher view, ensuring the hand-off is observable.
    ensure!(
        snapshot_next.plan_height > snapshot_initial.plan_height
            || snapshot_next.plan_view > snapshot_initial.plan_view,
        "collector snapshot did not advance (initial height={}, view={}; next height={}, view={})",
        snapshot_initial.plan_height,
        snapshot_initial.plan_view,
        snapshot_next.plan_height,
        snapshot_next.plan_view,
    );

    // Sequential snapshots should not report stale PRF seeds and the collectors must be
    // unique within a given response.
    ensure!(
        !snapshot_initial.collector_peer_ids.is_empty(),
        "initial snapshot should advertise at least one collector"
    );
    ensure!(
        !snapshot_next.collector_peer_ids.is_empty(),
        "next snapshot should advertise at least one collector"
    );
    let unique_initial: HashSet<&String> = snapshot_initial.collector_peer_ids.iter().collect();
    ensure!(
        unique_initial.len() == snapshot_initial.collector_peer_ids.len(),
        "collector list may not contain duplicates"
    );
    let unique_next: HashSet<&String> = snapshot_next.collector_peer_ids.iter().collect();
    ensure!(
        unique_next.len() == snapshot_next.collector_peer_ids.len(),
        "collector list may not contain duplicates"
    );

    network.shutdown().await;
    Ok(())
}

async fn drive_network_to_total_height(
    network: &sandbox::SerializedNetwork,
    client: &iroha::client::Client,
    target_height: u64,
    label: &str,
) -> eyre::Result<()> {
    let mut current_height = client.get_status()?.blocks;
    while current_height < target_height {
        let next_height = current_height.saturating_add(1);
        client.submit_all([Log::new(Level::INFO, format!("{label} {next_height}"))])?;
        network
            .ensure_blocks_with(|height| height.total >= next_height)
            .await?;
        current_height = client.get_status()?.blocks;
    }
    Ok(())
}

fn next_collectors_observation_height(current_height: u64, plan_height: u64) -> u64 {
    current_height.max(plan_height).saturating_add(1)
}

fn topology_from_peers(peers: &[iroha_test_network::NetworkPeer]) -> Topology {
    let ids: Vec<PeerId> = peers
        .iter()
        .map(iroha_test_network::NetworkPeer::id)
        .collect();
    Topology::new(ids)
}

fn verify_snapshot(topology: &Topology, snapshot: &CollectorsSnapshot) -> eyre::Result<()> {
    let expected = deterministic_collectors(
        topology,
        snapshot.mode,
        snapshot.collectors_k,
        Some(snapshot.epoch_seed),
        snapshot.plan_height,
        snapshot.plan_view,
    );
    let expected_ids: Vec<String> = expected.iter().map(peer_id_to_string).collect();
    ensure!(
        expected_ids == snapshot.collector_peer_ids,
        "collector plan mismatch: expected {:?}, got {:?}",
        expected_ids,
        snapshot.collector_peer_ids
    );
    Ok(())
}

fn peer_id_to_string(peer: &PeerId) -> String {
    peer.to_string()
}

async fn retry_collectors_until_height(
    http: &reqwest::Client,
    url: &reqwest::Url,
    min_height: u64,
    min_view: u64,
    interval: Duration,
    attempts: usize,
) -> eyre::Result<CollectorsSnapshot> {
    let mut last_snapshot = fetch_collectors_snapshot(http, url).await?;
    if collectors_snapshot_advanced(&last_snapshot, min_height, min_view) {
        return Ok(last_snapshot);
    }
    for _ in 0..attempts {
        sleep(interval).await;
        last_snapshot = fetch_collectors_snapshot(http, url).await?;
        if collectors_snapshot_advanced(&last_snapshot, min_height, min_view) {
            return Ok(last_snapshot);
        }
    }
    eyre::bail!(
        "collector snapshot did not advance beyond height {min_height} / view {min_view} after {attempts} attempts"
    )
}

fn collectors_snapshot_advanced(
    snapshot: &CollectorsSnapshot,
    min_height: u64,
    min_view: u64,
) -> bool {
    snapshot.plan_height > min_height
        || (snapshot.plan_height == min_height && snapshot.plan_view > min_view)
}

#[derive(Clone, Debug)]
struct CollectorsSnapshot {
    mode: ConsensusMode,
    plan_height: u64,
    plan_view: u64,
    epoch_seed: [u8; 32],
    collectors_k: usize,
    collector_peer_ids: Vec<String>,
}

async fn fetch_collectors_snapshot(
    http: &reqwest::Client,
    url: &reqwest::Url,
) -> eyre::Result<CollectorsSnapshot> {
    let response = http
        .get(url.clone())
        .header("accept", "application/json")
        .send()
        .await
        .wrap_err("fetch collectors snapshot")?;
    ensure!(
        response.status().is_success(),
        "collectors endpoint returned status {}",
        response.status()
    );
    let body = response.text().await.wrap_err("collectors body")?;
    let value: Value = json::from_str(&body).wrap_err("parse collectors JSON")?;
    let root = value
        .as_object()
        .ok_or_else(|| eyre::eyre!("collectors payload must be a JSON object"))?;

    let mode = match root
        .get("consensus_mode")
        .and_then(Value::as_str)
        .ok_or_else(|| eyre::eyre!("collectors payload missing consensus_mode"))?
    {
        "Permissioned" => ConsensusMode::Permissioned,
        "Npos" => ConsensusMode::Npos,
        other => eyre::bail!("unexpected consensus mode reported: {other}"),
    };

    let k = root
        .get("collectors_k")
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre::eyre!("collectors payload missing collectors_k"))?;
    let collectors_k = usize::try_from(k)
        .map_err(|_| eyre::eyre!("collectors_k value {k} does not fit into usize"))?;

    let prf = root
        .get("prf")
        .and_then(Value::as_object)
        .ok_or_else(|| eyre::eyre!("collectors payload missing prf context"))?;
    let plan_height = prf
        .get("height")
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre::eyre!("collectors payload missing prf.height"))?;
    let plan_view = prf
        .get("view")
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre::eyre!("collectors payload missing prf.view"))?;
    let seed_hex = prf
        .get("epoch_seed")
        .and_then(Value::as_str)
        .ok_or_else(|| eyre::eyre!("collectors payload missing prf.epoch_seed"))?;
    let epoch_seed = parse_seed(seed_hex)?;

    let collectors = root
        .get("collectors")
        .and_then(Value::as_array)
        .ok_or_else(|| eyre::eyre!("collectors payload missing collectors list"))?;
    let collector_peer_ids = collectors
        .iter()
        .map(|value| {
            value
                .as_object()
                .and_then(|entry| entry.get("peer_id"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| eyre::eyre!("collector entry missing peer_id"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CollectorsSnapshot {
        mode,
        plan_height,
        plan_view,
        epoch_seed,
        collectors_k,
        collector_peer_ids,
    })
}

#[test]
fn collectors_snapshot_advanced_accepts_same_height_with_higher_view() {
    let snapshot = CollectorsSnapshot {
        mode: ConsensusMode::Npos,
        plan_height: 5,
        plan_view: 2,
        epoch_seed: [0; 32],
        collectors_k: 2,
        collector_peer_ids: Vec::new(),
    };

    assert!(collectors_snapshot_advanced(&snapshot, 5, 1));
    assert!(collectors_snapshot_advanced(&snapshot, 4, 99));
    assert!(!collectors_snapshot_advanced(&snapshot, 5, 2));
}

#[test]
fn next_collectors_observation_height_forces_one_more_block_when_chain_is_ahead() {
    assert_eq!(next_collectors_observation_height(6, 5), 7);
    assert_eq!(next_collectors_observation_height(4, 5), 6);
}

fn parse_seed(hex_str: &str) -> eyre::Result<[u8; 32]> {
    let bytes = hex::decode(hex_str).wrap_err("decode epoch seed hex")?;
    ensure!(
        bytes.len() == 32,
        "epoch seed must be 32 bytes, got {}",
        bytes.len()
    );
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&bytes);
    Ok(seed)
}
