#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Regression tests ensuring Sumeragi keeps `locked_qc` in sync during view changes and restarts.

use std::time::{Duration, Instant};

use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::{Client, Status},
    data_model::{
        Level,
        isi::{Log, SetParameter},
        parameter::{Parameter, SumeragiParameter},
    },
};
use iroha_core::sumeragi::network_topology::Topology;
use iroha_test_network::{NetworkBuilder, NetworkPeer, init_instruction_registry};
use norito::json::Value;
use tokio::{task, time::sleep};
use toml::Table;

const VIEW_CHANGE_RECOVERY_TIMEOUT: Duration = Duration::from_secs(300);

#[allow(clippy::too_many_lines)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sumeragi_view_change_lock_convergence() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(500),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(1_000),
        )))
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "consensus_mode"], "permissioned")
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "propose_ms"],
                    200_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "prevote_ms"],
                    400_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "precommit_ms"],
                    600_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "commit_ms"],
                    800_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "da_ms"],
                    400_i64,
                )
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

    let baseline_height = baseline_blocks.into_iter().max().unwrap_or_default();
    let target_height = baseline_height + 1;
    let prf_seed = chain_epoch_seed(&network.chain_id());
    let leader_peer =
        resolve_permissioned_leader_peer(network.peers(), target_height, 0, Some(prf_seed))
            .wrap_err_with(|| {
                format!("resolve permissioned leader for height {target_height} view 0")
            })?;
    ensure!(
        leader_peer.is_running(),
        "resolved leader peer for height {target_height} view 0 is not running"
    );
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

    let wait_client = running
        .first()
        .ok_or_else(|| eyre!("no running peers available"))?
        .client();
    let mut submitted = false;
    let mut submit_errors = Vec::new();
    for (idx, peer) in running.iter().enumerate() {
        let submit_client = peer.client();
        match task::spawn_blocking(move || {
            submit_client.submit(Log::new(
                Level::INFO,
                format!("lock convergence view-change tick {idx}"),
            ))
        })
        .await
        {
            Ok(Ok(_)) => {
                submitted = true;
            }
            Ok(Err(err)) => submit_errors.push(err),
            Err(err) => submit_errors.push(eyre!("submit join error: {err}")),
        }
    }
    ensure!(
        submitted,
        "failed to submit log instruction to any running peer: {submit_errors:?}"
    );
    let _ = wait_for_height(&wait_client, target_height, VIEW_CHANGE_RECOVERY_TIMEOUT).await?;

    let view_change_deadline = Instant::now() + VIEW_CHANGE_RECOVERY_TIMEOUT;
    let mut observed_view_change_advance = false;
    while Instant::now() < view_change_deadline {
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
            observed_view_change_advance = true;
            break;
        }
        sleep(Duration::from_millis(200)).await;
    }
    if !observed_view_change_advance {
        let mut snapshots = Vec::new();
        for (idx, peer) in network.peers().iter().enumerate() {
            if !peer.is_running() {
                continue;
            }
            let status = peer.status().await?;
            snapshots.push((idx, status.view_changes));
        }
        eprintln!(
            "view-change counters did not advance before timeout; continuing with locked QC convergence check: {snapshots:?}"
        );
    }

    let locked_convergence_deadline = Instant::now() + VIEW_CHANGE_RECOVERY_TIMEOUT;
    loop {
        let mut locked_entries = Vec::new();
        for peer in network.peers() {
            if !peer.is_running() {
                continue;
            }
            let snapshot = fetch_qc_snapshot(&peer.client()).await?;
            locked_entries.push(snapshot.locked);
        }

        let locked_converged =
            assert_qc_entries_match(&locked_entries, "locked QC divergence after view change")
                .is_ok();
        let locked_height_ok = locked_entries
            .first()
            .is_some_and(|entry| entry.height >= target_height);
        if locked_converged && locked_height_ok {
            break;
        }
        if Instant::now() >= locked_convergence_deadline {
            return Err(eyre!(
                "timed out waiting for locked QC convergence after view change; locked={locked_entries:?}, target_height={target_height}"
            ));
        }
        sleep(Duration::from_millis(200)).await;
    }

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
    wait_client
        .submit_all([Log::new(
            Level::INFO,
            "lock convergence restart tick".to_string(),
        )])
        .wrap_err("submit lock convergence restart tick")?;
    let _ = wait_for_height(&wait_client, target_height, Duration::from_secs(60)).await?;
    let final_deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let mut final_locked = Vec::new();
        for peer in &running {
            final_locked.push(fetch_qc_snapshot(&peer.client()).await?.locked);
        }
        if assert_qc_entries_match(
            &final_locked,
            "locked QC divergence after post-restart progress",
        )
        .is_ok()
        {
            break;
        }
        if Instant::now() >= final_deadline {
            return Err(eyre!(
                "timed out waiting for locked QC convergence after post-restart progress; locked={final_locked:?}"
            ));
        }
        sleep(Duration::from_millis(200)).await;
    }

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

fn resolve_permissioned_leader_peer(
    peers: &[NetworkPeer],
    height: u64,
    view: u64,
    prf_seed: Option<[u8; 32]>,
) -> Result<NetworkPeer> {
    let mut roster: Vec<_> = peers.iter().map(NetworkPeer::id).collect();
    roster.sort();
    roster.dedup();
    let mut topology = Topology::new(roster);
    if let Some(seed) = prf_seed {
        topology.shuffle_prf(seed, height);
    }
    topology.nth_rotation(view);
    let leader_peer_id = topology
        .as_ref()
        .first()
        .ok_or_else(|| eyre!("empty topology when resolving leader"))?;
    peers
        .iter()
        .find(|peer| peer.id() == *leader_peer_id)
        .cloned()
        .ok_or_else(|| eyre!("leader peer id not found in network peers"))
}

fn chain_epoch_seed(chain_id: &iroha::data_model::ChainId) -> [u8; 32] {
    let chain = chain_id.clone().into_inner();
    let hash = iroha_crypto::Hash::new(chain.as_bytes());
    <[u8; 32]>::from(hash)
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
