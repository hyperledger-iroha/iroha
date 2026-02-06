#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Regression tests ensuring Sumeragi keeps `locked_qc` in sync during view changes and restarts.

use std::{
    convert::TryFrom,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::{Client, Status},
    data_model::{Level, isi::Log},
};
use iroha_test_network::{NetworkBuilder, NetworkPeer, init_instruction_registry};
use norito::json::Value;
use tokio::{task, time::sleep};
use toml::Table;

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_view_change_lock_convergence() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(
                    ["sumeragi", "advanced", "pacemaker", "backoff_multiplier"],
                    2_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    1_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    2_000_i64,
                );
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(sumeragi_view_change_lock_convergence),
    )
    .await?
    else {
        return Ok(());
    };

    let client = network.client();
    let status = client.get_status()?;
    for idx in status.blocks..3 {
        client.submit_blocking(Log::new(
            Level::INFO,
            format!("lock convergence seed {idx}"),
        ))?;
    }
    network
        .ensure_blocks_with(|height| height.total >= 3)
        .await?;

    let mut baseline_view_changes = Vec::new();
    let mut baseline_blocks = Vec::new();
    for peer in network.peers() {
        let status = peer.status().await?;
        baseline_view_changes.push(u64::from(status.view_changes));
        baseline_blocks.push(status.blocks);
    }

    let primary_peer = network
        .peers()
        .iter()
        .find(|peer| peer.is_running())
        .cloned()
        .ok_or_else(|| eyre!("expected at least one running peer"))?;
    let leader_index = fetch_leader_index(&primary_peer.client()).await?;
    ensure!(
        leader_index < network.peers().len(),
        "leader_index {leader_index} out of bounds for {} peers",
        network.peers().len()
    );
    let leader_peer = network.peers()[leader_index].clone();
    leader_peer.shutdown().await;
    sleep(Duration::from_secs(1)).await;

    let running: Vec<NetworkPeer> = network
        .peers()
        .iter()
        .filter(|peer| peer.is_running())
        .cloned()
        .collect();
    ensure!(
        running.len() >= 3,
        "expected at least 3 running peers after leader shutdown, got {}",
        running.len()
    );

    let baseline_height = baseline_blocks.into_iter().max().unwrap_or_default();
    let target_height = baseline_height + 1;
    let wait_client = running
        .first()
        .ok_or_else(|| eyre!("no running peers available"))?
        .client();
    wait_client.submit_blocking(Log::new(
        Level::INFO,
        "lock convergence view-change tick".to_string(),
    ))?;
    let _ = wait_for_height(&wait_client, target_height, Duration::from_secs(60)).await?;

    let view_change_deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let mut all_advanced = true;
        for (idx, peer) in network.peers().iter().enumerate() {
            if !peer.is_running() {
                continue;
            }
            let status = peer.status().await?;
            let baseline = baseline_view_changes[idx];
            if u64::from(status.view_changes) < baseline.saturating_add(1) {
                all_advanced = false;
                break;
            }
        }
        if all_advanced {
            break;
        }
        if Instant::now() >= view_change_deadline {
            let mut snapshots = Vec::new();
            for (idx, peer) in network.peers().iter().enumerate() {
                if !peer.is_running() {
                    continue;
                }
                let status = peer.status().await?;
                snapshots.push((idx, status.view_changes));
            }
            return Err(eyre!(
                "timed out waiting for view change counters to advance; view_changes={snapshots:?}"
            ));
        }
        sleep(Duration::from_millis(200)).await;
    }

    let mut locked_entries = Vec::new();
    for peer in network.peers() {
        if !peer.is_running() {
            continue;
        }
        let snapshot = fetch_qc_snapshot(&peer.client()).await?;
        locked_entries.push(snapshot.locked);
    }

    assert_qc_entries_match(&locked_entries, "locked QC divergence after view change")?;

    network.shutdown().await;
    Ok(())
}

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_restart_retains_lock_convergence() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full");
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(sumeragi_restart_retains_lock_convergence),
    )
    .await?
    else {
        return Ok(());
    };

    let client = network.client();
    let status = client.get_status()?;
    for idx in status.blocks..3 {
        client.submit_blocking(Log::new(
            Level::INFO,
            format!("lock convergence restart seed {idx}"),
        ))?;
    }
    network
        .ensure_blocks_with(|height| height.total >= 3)
        .await?;

    let mut baseline_snapshots = Vec::new();
    for peer in network.peers() {
        baseline_snapshots.push(fetch_qc_snapshot(&peer.client()).await?);
    }
    let baseline_locked: Vec<QcEntry> = baseline_snapshots
        .iter()
        .map(|snap| snap.locked.clone())
        .collect();
    assert_qc_entries_match(&baseline_locked, "baseline locked QC mismatch")?;
    let baseline_highest: Vec<QcEntry> = baseline_snapshots
        .iter()
        .map(|snap| snap.highest.clone())
        .collect();
    assert_qc_entries_match(&baseline_highest, "baseline highest QC mismatch")?;
    let baseline_locked_height = baseline_locked
        .first()
        .map(|entry| entry.height)
        .unwrap_or_default();
    ensure!(
        baseline_locked_height > 0,
        "expected baseline locked QC height to be greater than zero"
    );

    let config_layers: Vec<ConfigLayer> = network
        .config_layers()
        .map(|cow| ConfigLayer(cow.into_owned()))
        .collect();

    network.shutdown().await;
    sleep(Duration::from_secs(2)).await;

    for peer in network.peers() {
        let mnemonic = peer.mnemonic().to_string();
        peer.start_checked(config_layers.iter().cloned(), None)
            .await
            .wrap_err_with(|| format!("restart peer {mnemonic}"))?;
    }

    let running: Vec<NetworkPeer> = network
        .peers()
        .iter()
        .filter(|peer| peer.is_running())
        .cloned()
        .collect();
    ensure!(
        running.len() == network.peers().len(),
        "expected all peers to run after restart (running={}, total={})",
        running.len(),
        network.peers().len()
    );

    let post_restart_deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let mut post_restart_snapshots = Vec::new();
        for peer in &running {
            post_restart_snapshots.push(fetch_qc_snapshot(&peer.client()).await?);
        }
        let post_locked: Vec<QcEntry> = post_restart_snapshots
            .iter()
            .map(|snap| snap.locked.clone())
            .collect();
        let post_highest: Vec<QcEntry> = post_restart_snapshots
            .iter()
            .map(|snap| snap.highest.clone())
            .collect();
        let locked_converged =
            assert_qc_entries_match(&post_locked, "post-restart locked QC mismatch").is_ok();
        let highest_converged =
            assert_qc_entries_match(&post_highest, "post-restart highest QC mismatch").is_ok();
        let locked_height_ok = post_locked
            .first()
            .is_some_and(|entry| entry.height >= baseline_locked_height);
        if locked_converged && highest_converged && locked_height_ok {
            break;
        }
        if Instant::now() >= post_restart_deadline {
            return Err(eyre!(
                "timed out waiting for post-restart QC convergence; locked={post_locked:?}, highest={post_highest:?}"
            ));
        }
        sleep(Duration::from_millis(200)).await;
    }

    let target_height = baseline_locked_height + 1;
    let wait_client = running
        .first()
        .ok_or_else(|| eyre!("no running peers after restart"))?
        .client();
    wait_client.submit_blocking(Log::new(
        Level::INFO,
        "lock convergence restart tick".to_string(),
    ))?;
    let _ = wait_for_height(&wait_client, target_height, Duration::from_secs(60)).await?;
    let mut final_locked = Vec::new();
    for peer in &running {
        final_locked.push(fetch_qc_snapshot(&peer.client()).await?.locked);
    }
    assert_qc_entries_match(
        &final_locked,
        "locked QC divergence after post-restart progress",
    )?;

    network.shutdown().await;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct QcEntry {
    height: u64,
    view: u64,
    subject: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct QcSnapshot {
    highest: QcEntry,
    locked: QcEntry,
}

#[derive(Clone)]
struct ConfigLayer(Table);

impl AsRef<Table> for ConfigLayer {
    fn as_ref(&self) -> &Table {
        &self.0
    }
}

async fn fetch_leader_index(client: &Client) -> Result<usize> {
    let client = client.clone();
    let payload = task::spawn_blocking(move || client.get_sumeragi_status_json())
        .await
        .wrap_err("fetch sumeragi status payload")??;
    parse_leader_index(&payload)
}

async fn fetch_qc_snapshot(client: &Client) -> Result<QcSnapshot> {
    let client = client.clone();
    let payload = task::spawn_blocking(move || client.get_sumeragi_qc_json())
        .await
        .wrap_err("fetch sumeragi QC snapshot")??;
    parse_qc_snapshot(&payload)
}

async fn wait_for_height(client: &Client, target_height: u64, timeout: Duration) -> Result<Status> {
    let client = client.clone();
    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() > deadline {
            return Err(eyre!("timed out waiting for block height {target_height}"));
        }
        let status = fetch_status(&client).await?;
        if status.blocks >= target_height {
            return Ok(status);
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn fetch_status(client: &Client) -> Result<Status> {
    let client = client.clone();
    task::spawn_blocking(move || client.get_status())
        .await
        .wrap_err("join status fetch task")?
        .wrap_err("fetch status")
}

fn parse_leader_index(value: &Value) -> Result<usize> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("status payload must be a JSON object"))?;
    let leader = object
        .get("leader_index")
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("status payload missing leader_index"))?;
    usize::try_from(leader).wrap_err("leader_index does not fit into usize")
}

fn parse_qc_snapshot(value: &Value) -> Result<QcSnapshot> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("QC snapshot must be a JSON object"))?;
    let highest = object
        .get("highest_qc")
        .ok_or_else(|| eyre!("QC snapshot missing highest_qc field"))?;
    let locked = object
        .get("locked_qc")
        .ok_or_else(|| eyre!("QC snapshot missing locked_qc field"))?;
    Ok(QcSnapshot {
        highest: parse_qc_entry(highest)?,
        locked: parse_qc_entry(locked)?,
    })
}

fn parse_qc_entry(value: &Value) -> Result<QcEntry> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("QC entry must be a JSON object"))?;
    let height = object
        .get("height")
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("QC entry missing height"))?;
    let view = object
        .get("view")
        .and_then(Value::as_u64)
        .ok_or_else(|| eyre!("QC entry missing view"))?;
    let subject = object
        .get("subject_block_hash")
        .map(|raw| {
            if raw.is_null() {
                Ok(None)
            } else {
                raw.as_str()
                    .map(|s| Some(s.to_owned()))
                    .ok_or_else(|| eyre!("subject_block_hash must be string or null"))
            }
        })
        .transpose()?
        .flatten();
    Ok(QcEntry {
        height,
        view,
        subject,
    })
}

fn assert_qc_entries_match(entries: &[QcEntry], context: &str) -> Result<()> {
    ensure!(
        !entries.is_empty(),
        "{context}: no entries available for comparison"
    );
    let reference = &entries[0];
    ensure!(
        entries.iter().all(|entry| entry == reference),
        "{context}: expected all entries to equal {:?}, got {:?}",
        reference,
        entries
    );
    Ok(())
}
