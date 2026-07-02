#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration regression tests for Sumeragi PRF-based collector selection.

use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use eyre::{WrapErr, ensure};
use integration_tests::sandbox;
use iroha_config::parameters::actual::ConsensusMode;
use iroha_core::sumeragi::{
    collectors::deterministic_collectors,
    network_topology::{Topology, commit_quorum_from_len},
};
use iroha_data_model::{Level, isi::Log, peer::PeerId};
use iroha_test_network::{NetworkBuilder, NetworkPeer, init_instruction_registry};
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
        .with_sync_timeout(Duration::from_secs(420))
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
    let clients = network
        .peers()
        .iter()
        .map(NetworkPeer::client)
        .collect::<Vec<_>>();
    let client = clients
        .first()
        .cloned()
        .ok_or_else(|| eyre::eyre!("test network must expose at least one client"))?;
    drive_network_to_total_height(&network, &client, 6, "prf seed").await?;

    let topology = topology_from_peers(network.peers());
    let collectors_urls = clients
        .iter()
        .map(|client| {
            client
                .torii_url
                .join("v1/sumeragi/collectors")
                .wrap_err("compose collectors URL")
        })
        .collect::<eyre::Result<Vec<_>>>()?;
    let http = integration_tests::http::client();

    let snapshot_initial = fetch_collectors_snapshot(&http, &collectors_urls[0]).await?;
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
        &collectors_urls,
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
    let quorum = commit_quorum_from_len(network.peers().len()).max(1);
    while current_height < target_height {
        let next_height = current_height.saturating_add(1);
        client.submit_all([Log::new(Level::INFO, format!("{label} {next_height}"))])?;
        let heights =
            wait_for_total_height_quorum(network, next_height, quorum, network.sync_timeout())
                .await?;
        current_height = heights.iter().copied().max().unwrap_or(next_height);
    }
    Ok(())
}

async fn wait_for_total_height_quorum(
    network: &sandbox::SerializedNetwork,
    target_height: u64,
    quorum: usize,
    timeout: Duration,
) -> eyre::Result<Vec<u64>> {
    let deadline = Instant::now() + timeout;
    let mut last_observed = Vec::new();
    loop {
        last_observed.clear();
        let mut heights = Vec::new();
        for peer in network.peers() {
            match peer.status().await {
                Ok(status) => {
                    heights.push(status.blocks);
                    last_observed.push(format!("ok:{}", status.blocks));
                }
                Err(err) => last_observed.push(format!("err:{err}")),
            }
        }
        if count_heights_at_or_above(&heights, target_height) >= quorum {
            return Ok(heights);
        }
        if Instant::now() >= deadline {
            eyre::bail!(
                "total height {target_height} did not reach quorum {quorum} within {timeout:?}; last observed {last_observed:?}"
            );
        }
        sleep(Duration::from_millis(250)).await;
    }
}

fn count_heights_at_or_above(heights: &[u64], target_height: u64) -> usize {
    heights
        .iter()
        .filter(|height| **height >= target_height)
        .count()
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
    urls: &[reqwest::Url],
    min_height: u64,
    min_view: u64,
    interval: Duration,
    attempts: usize,
) -> eyre::Result<CollectorsSnapshot> {
    ensure!(
        !urls.is_empty(),
        "collector retry requires at least one URL"
    );
    let mut last_snapshots = Vec::new();
    for attempt in 0..=attempts {
        last_snapshots.clear();
        for url in urls {
            last_snapshots.push(fetch_collectors_snapshot(http, url).await?);
        }
        if let Some(snapshot) =
            first_advanced_collectors_snapshot(&last_snapshots, min_height, min_view)
        {
            return Ok(snapshot);
        }
        if attempt == attempts {
            break;
        }
        sleep(interval).await;
    }
    let last_observed = last_snapshots
        .iter()
        .map(|snapshot| (snapshot.plan_height, snapshot.plan_view))
        .collect::<Vec<_>>();
    eyre::bail!(
        "collector snapshot did not advance beyond height {min_height} / view {min_view} after {attempts} attempts; last observed {last_observed:?}"
    )
}

fn first_advanced_collectors_snapshot(
    snapshots: &[CollectorsSnapshot],
    min_height: u64,
    min_view: u64,
) -> Option<CollectorsSnapshot> {
    snapshots
        .iter()
        .find(|snapshot| collectors_snapshot_advanced(snapshot, min_height, min_view))
        .cloned()
}

fn collectors_snapshot_advanced(
    snapshot: &CollectorsSnapshot,
    min_height: u64,
    min_view: u64,
) -> bool {
    snapshot.plan_height > min_height
        || (snapshot.plan_height == min_height && snapshot.plan_view > min_view)
}

#[derive(Clone, Debug, PartialEq)]
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
fn first_advanced_collectors_snapshot_scans_all_peers() {
    let stale = CollectorsSnapshot {
        mode: ConsensusMode::Npos,
        plan_height: 6,
        plan_view: 0,
        epoch_seed: [0; 32],
        collectors_k: 1,
        collector_peer_ids: vec!["peer-a".to_string()],
    };
    let advanced = CollectorsSnapshot {
        plan_height: 7,
        collector_peer_ids: vec!["peer-b".to_string()],
        ..stale.clone()
    };

    assert_eq!(
        first_advanced_collectors_snapshot(&[stale, advanced.clone()], 6, 0)
            .expect("advanced snapshot"),
        advanced
    );
}

#[test]
fn next_collectors_observation_height_forces_one_more_block_when_chain_is_ahead() {
    assert_eq!(next_collectors_observation_height(6, 5), 7);
    assert_eq!(next_collectors_observation_height(4, 5), 6);
}

#[test]
fn count_heights_at_or_above_counts_quorum_candidates() {
    assert_eq!(count_heights_at_or_above(&[5, 4, 5, 3], 5), 2);
    assert_eq!(count_heights_at_or_above(&[6, 6, 5, 1], 5), 3);
    assert_eq!(count_heights_at_or_above(&[], 1), 0);
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
