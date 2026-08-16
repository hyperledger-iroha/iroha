#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Verify NPoS liveness with revision-4 equal-vote, full-committee consensus.
use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::client::Client;
use iroha::data_model::{
    Level,
    isi::{Log, SetParameter},
    metadata::Metadata,
    parameter::{BlockParameter, Parameter, system::SumeragiNposParameters},
};
use iroha_test_network::{Network, NetworkBuilder, NetworkPeer, init_instruction_registry};
use nonzero_ext::nonzero;
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
use tokio::time::sleep;
const MAX_HEIGHT_SKEW: u64 = 2;
const POST_RESTART_PROGRESS_PROBE_SECS: u64 = 60;
const TOLERATED_LAGGING_PEERS: usize = 1;
const NPOS_LIVENESS_SYNC_TIMEOUT: Duration = Duration::from_secs(600);
const FAIL_ON_SANDBOX_SKIP_ENV: &str = "IROHA_FAIL_ON_SANDBOX_SKIP";
static NEXT_SUBMIT_PEER_INDEX: AtomicUsize = AtomicUsize::new(0);
fn fail_on_sandbox_skip() -> bool {
    let Ok(raw) = std::env::var(FAIL_ON_SANDBOX_SKIP_ENV) else {
        return false;
    };
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}
#[test]
fn npos_network_produces_blocks() -> Result<()> {
    init_instruction_registry();
    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_npos_genesis_bootstrap(SumeragiNposParameters::default().min_self_bond().clone())
        .with_sync_timeout(NPOS_LIVENESS_SYNC_TIMEOUT)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )));
    let Some((network, rt)) =
        sandbox::start_network_blocking_or_skip(builder, stringify!(npos_network_produces_blocks))?
    else {
        ensure!(
            !fail_on_sandbox_skip(),
            "sandboxed skip surfaced and {FAIL_ON_SANDBOX_SKIP_ENV} is enabled"
        );
        return Ok(());
    };
    let sync_timeout = network.sync_timeout();
    let result: Result<()> = (|| {
        rt.block_on(async { wait_for_status_responses(&network, sync_timeout).await })
            .wrap_err("peers did not respond to status after startup")?;
        // Drive the chain forward deterministically with single-transaction blocks, but
        // choose a connected submit peer on each attempt to avoid queue-timeout flakiness
        // right after startup on slower grouped runs.
        let probe_client = network.client();
        let status_before = probe_client.get_status()?;
        let target_height = status_before.blocks + 5;
        let observed_heights = rt
            .block_on(async {
                drive_network_to_height(
                    &network,
                    &probe_client,
                    target_height,
                    sync_timeout,
                    "npos liveness seed",
                )
                .await
            })
            .wrap_err("heights did not converge")?;
        // A commit quorum should advance within the skew bound while one validator may lag.
        let max = *observed_heights.iter().max().unwrap_or(&0);
        ensure!(
            max >= target_height,
            "latest height should be at least {target_height}, got {max}"
        );
        ensure!(
            heights_meet_target_tolerating_lag(
                &observed_heights,
                target_height,
                MAX_HEIGHT_SKEW,
                TOLERATED_LAGGING_PEERS,
            ),
            "peer heights diverged during NPoS liveness check (target={target_height} allowed_skew={MAX_HEIGHT_SKEW}, got {observed_heights:?})"
        );
        rt.block_on(async {
            network.shutdown().await;
        });
        Ok(())
    })();
    if sandbox::handle_result(result, stringify!(npos_network_produces_blocks))?.is_none() {
        return Ok(());
    }
    Ok(())
}
async fn wait_for_status_responses(network: &Network, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_error = None;
    loop {
        let mut all_ok = true;
        for peer in network.peers() {
            let status = tokio::time::timeout(Duration::from_secs(5), peer.status()).await;
            match status {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => {
                    if detect_height_from_storage(peer).is_some() {
                        continue;
                    }
                    all_ok = false;
                    last_error = Some(format!("peer {} status failed: {error:?}", peer.mnemonic()));
                    break;
                }
                Err(_) => {
                    if detect_height_from_storage(peer).is_some() {
                        continue;
                    }
                    all_ok = false;
                    last_error = Some(format!(
                        "peer {} status timed out after 5s",
                        peer.mnemonic()
                    ));
                    break;
                }
            }
        }
        if all_ok {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "status responses did not converge within {:?}; last_error={:?}",
                timeout,
                last_error
            ));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
async fn wait_for_submit_connectivity(network: &Network, timeout: Duration) -> Result<Vec<u64>> {
    let deadline = Instant::now() + timeout;
    let expected = min_connected_peers_for_submit(network.peers().len());
    let mut last_snapshot = Vec::new();
    let mut last_error = None;
    loop {
        let mut peer_counts = Vec::new();
        for peer in network.peers() {
            let status = tokio::time::timeout(Duration::from_secs(5), peer.status()).await;
            match status {
                Ok(Ok(status)) => peer_counts.push(status.peers),
                Ok(Err(error)) => {
                    last_error = Some(format!("peer {} status failed: {error:?}", peer.mnemonic()));
                    peer_counts.clear();
                    break;
                }
                Err(_) => {
                    last_error = Some(format!(
                        "peer {} status timed out after 5s",
                        peer.mnemonic()
                    ));
                    peer_counts.clear();
                    break;
                }
            }
        }
        if !peer_counts.is_empty() {
            last_snapshot.clone_from(&peer_counts);
            if peer_counts.iter().all(|count| *count >= expected) {
                return Ok(peer_counts);
            }
        }
        if Instant::now() >= deadline {
            return Err(eyre!(
                "peer connectivity did not reach {expected} connected peers within {:?}; last_snapshot={last_snapshot:?} last_error={last_error:?}",
                timeout,
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}
async fn wait_for_converged_heights(
    network: &Network,
    min_height: u64,
    timeout: Duration,
) -> Result<Vec<u64>> {
    wait_for_converged_heights_with_skew(network, min_height, timeout, MAX_HEIGHT_SKEW).await
}
async fn wait_for_converged_heights_with_skew(
    network: &Network,
    min_height: u64,
    timeout: Duration,
    allowed_skew: u64,
) -> Result<Vec<u64>> {
    let deadline = Instant::now() + timeout;
    let mut last_snapshot = Vec::new();
    loop {
        let last_error = match peer_height_snapshot(network).await {
            Ok(heights) => {
                last_snapshot.clone_from(&heights);
                if heights_meet_target_tolerating_lag(
                    &heights,
                    min_height,
                    allowed_skew,
                    TOLERATED_LAGGING_PEERS,
                ) {
                    return Ok(heights);
                }
                None
            }
            Err(error) => Some(format!("{error:?}")),
        };
        if Instant::now() >= deadline {
            return Err(eyre!(
                "heights failed to converge within {:?}; target={min_height} allowed_skew={allowed_skew} last_snapshot={last_snapshot:?} last_error={last_error:?}",
                timeout,
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}
async fn peer_height_snapshot(network: &Network) -> Result<Vec<u64>> {
    let mut heights = Vec::with_capacity(network.peers().len());
    for peer in network.peers() {
        let status = tokio::time::timeout(Duration::from_secs(5), peer.status()).await;
        match status {
            Ok(Ok(status)) => heights.push(status.blocks),
            Ok(Err(error)) => {
                if let Some(height) = detect_height_from_storage(peer) {
                    heights.push(height);
                } else {
                    return Err(eyre!(
                        "status poll failed for peer {} and storage has no durable height: {error:?}",
                        peer.mnemonic()
                    ));
                }
            }
            Err(_) => {
                if let Some(height) = detect_height_from_storage(peer) {
                    heights.push(height);
                } else {
                    return Err(eyre!(
                        "status poll timed out after 5s for peer {} and storage has no durable height",
                        peer.mnemonic()
                    ));
                }
            }
        }
    }
    ensure!(
        heights.len() == network.peers().len(),
        "peer height snapshot is incomplete: expected {} peers, got {} heights",
        network.peers().len(),
        heights.len()
    );
    Ok(heights)
}
fn all_peer_heights_advanced(baseline: &[u64], current: &[u64]) -> bool {
    !baseline.is_empty()
        && baseline.len() == current.len()
        && baseline
            .iter()
            .zip(current)
            .all(|(before, after)| after > before)
}
async fn wait_for_all_peer_heights_to_advance(
    network: &Network,
    baseline: &[u64],
    timeout: Duration,
) -> Result<Vec<u64>> {
    ensure!(
        !baseline.is_empty() && baseline.len() == network.peers().len(),
        "per-peer progress baseline must cover all {} peers, got {baseline:?}",
        network.peers().len()
    );
    let deadline = Instant::now() + timeout;
    let mut last_snapshot = Vec::new();
    loop {
        let last_error = match peer_height_snapshot(network).await {
            Ok(heights) => {
                last_snapshot.clone_from(&heights);
                if all_peer_heights_advanced(baseline, &heights) {
                    return Ok(heights);
                }
                None
            }
            Err(error) => Some(format!("{error:?}")),
        };
        if Instant::now() >= deadline {
            return Err(eyre!(
                "not every peer advanced within {:?}; baseline={baseline:?} last_snapshot={last_snapshot:?} last_error={last_error:?}",
                timeout,
            ));
        }
        sleep(Duration::from_millis(250)).await;
    }
}
fn heights_meet_target(heights: &[u64], min_height: u64, allowed_skew: u64) -> bool {
    if heights.is_empty() {
        return false;
    }
    let min = *heights.iter().min().unwrap_or(&0);
    let max = *heights.iter().max().unwrap_or(&0);
    min >= min_height.saturating_sub(allowed_skew)
        && max >= min_height
        && max.saturating_sub(min) <= allowed_skew
}
fn heights_meet_target_tolerating_lag(
    heights: &[u64],
    min_height: u64,
    allowed_skew: u64,
    tolerated_lagging: usize,
) -> bool {
    if heights.is_empty() {
        return false;
    }
    if tolerated_lagging == 0 || heights.len() <= tolerated_lagging {
        return heights_meet_target(heights, min_height, allowed_skew);
    }
    let required = heights.len().saturating_sub(tolerated_lagging).max(1);
    let mut sorted = heights.to_vec();
    sorted.sort_unstable_by(|left, right| right.cmp(left));
    heights_meet_target(&sorted[..required], min_height, allowed_skew)
}
fn min_connected_peers_for_submit(peer_count: usize) -> u64 {
    match peer_count {
        0..=2 => 0,
        _ => u64::try_from(peer_count.saturating_sub(2)).expect("peer count should fit into u64"),
    }
}
fn pick_fallback_submit_peer_index(block_totals: &[u64], seed: usize) -> usize {
    if block_totals.is_empty() {
        return 0;
    }
    let best_total = block_totals.iter().copied().max().unwrap_or(0);
    let best_indices = block_totals
        .iter()
        .enumerate()
        .filter_map(|(idx, total)| (*total == best_total).then_some(idx))
        .collect::<Vec<_>>();
    if best_indices.is_empty() {
        return 0;
    }
    let offset = seed % best_indices.len();
    best_indices[offset]
}
fn pick_submit_peer_index(
    leader_index: Option<usize>,
    leader_connected: bool,
    block_totals: &[u64],
    seed: usize,
) -> usize {
    let fallback = pick_fallback_submit_peer_index(block_totals, seed);
    if leader_connected {
        leader_index.unwrap_or(fallback)
    } else {
        fallback
    }
}
fn ordered_submit_peer_indices(
    leader_index: Option<usize>,
    leader_connected: bool,
    block_totals: &[u64],
    seed: usize,
) -> Vec<usize> {
    if block_totals.is_empty() {
        return Vec::new();
    }
    let fallback = pick_fallback_submit_peer_index(block_totals, seed);
    let mut ordered = Vec::with_capacity(block_totals.len());
    if leader_connected
        && let Some(leader_index) = leader_index
        && leader_index < block_totals.len()
    {
        ordered.push(leader_index);
    }
    for offset in 0..block_totals.len() {
        let idx = (fallback + offset) % block_totals.len();
        if !ordered.contains(&idx) {
            ordered.push(idx);
        }
    }
    ordered
}
async fn submit_peer_indices_for_network(network: &Network, probe: &Client) -> Vec<usize> {
    let peer_count = network.peers().len();
    let (status, sumeragi) = tokio::task::spawn_blocking({
        let client = probe.clone();
        move || (client.get_status(), client.get_sumeragi_status())
    })
    .await
    .map(|(status, sumeragi)| (status.ok(), sumeragi.ok()))
    .unwrap_or((None, None));
    let leader_index = sumeragi
        .as_ref()
        .map(|status| status.leader)
        .and_then(|idx| usize::try_from(idx).ok())
        .filter(|&idx| idx < peer_count);
    let leader_is_connected = status
        .as_ref()
        .is_some_and(|status| status.peers >= min_connected_peers_for_submit(peer_count));
    let fallback_totals = network
        .peers()
        .iter()
        .map(|peer| {
            peer.best_effort_block_height()
                .map(|height| height.total)
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let fallback_seed = NEXT_SUBMIT_PEER_INDEX.fetch_add(1, Ordering::Relaxed);
    ordered_submit_peer_indices(
        leader_index,
        leader_is_connected,
        &fallback_totals,
        fallback_seed,
    )
}
async fn submit_seed_log(network: &Network, probe: &Client, message: String) -> Result<()> {
    let candidate_indices = submit_peer_indices_for_network(network, probe).await;
    let transaction = tokio::task::spawn_blocking({
        let client = probe.clone();
        let message = message.clone();
        move || {
            client.build_transaction_from_items(
                [Log::new(Level::INFO, message)],
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
                Metadata::default(),
            )
        }
    })
    .await
    .wrap_err("join seed transaction build task")?;
    let mut accepted = false;
    let mut errors = Vec::new();
    for idx in candidate_indices {
        let Some(peer) = network.peers().get(idx) else {
            continue;
        };
        let peer_name = peer.mnemonic().to_owned();
        let submit_client = peer.client();
        let transaction = transaction.clone();
        match tokio::task::spawn_blocking(move || submit_client.submit_transaction(&transaction))
            .await
        {
            Ok(Ok(_)) => accepted = true,
            Ok(Err(err)) => errors.push(format!("{peer_name}: {err:?}")),
            Err(err) => errors.push(format!("{peer_name}: submit join error: {err}")),
        }
    }
    ensure!(
        accepted,
        "submit seed log was not accepted by any candidate peer; errors={errors:?}"
    );
    Ok(())
}
async fn drive_network_to_height(
    network: &Network,
    probe: &Client,
    target_height: u64,
    timeout: Duration,
    label: &str,
) -> Result<Vec<u64>> {
    let deadline = Instant::now() + timeout;
    let mut attempt = 0_u64;
    let mut last_error = None;
    let mut next_height = tokio::task::spawn_blocking({
        let client = probe.clone();
        move || client.get_status()
    })
    .await
    .ok()
    .and_then(Result::ok)
    .map(|status| status.blocks.saturating_add(1))
    .unwrap_or(1)
    .min(target_height);
    let connectivity_timeout = timeout.min(Duration::from_secs(30));
    wait_for_submit_connectivity(network, connectivity_timeout)
        .await
        .wrap_err("submit connectivity not ready before seeding progress")?;
    loop {
        let now = Instant::now();
        if now >= deadline {
            return Err(eyre!(
                "heights failed to converge after {attempt} seed submissions; target={target_height} last_error={last_error:?}"
            ));
        }
        let remaining = deadline.saturating_duration_since(now);
        let probe_timeout = remaining.min(Duration::from_secs(POST_RESTART_PROGRESS_PROBE_SECS));
        if next_height > target_height {
            return wait_for_converged_heights(network, target_height, probe_timeout).await;
        }
        let message = format!("{label} {attempt}");
        submit_seed_log(network, probe, message)
            .await
            .wrap_err_with(|| format!("submit seed attempt {attempt}"))?;
        attempt = attempt.saturating_add(1);
        match wait_for_converged_heights(network, next_height, probe_timeout).await {
            Ok(heights) => {
                if next_height >= target_height {
                    return Ok(heights);
                }
                let observed_max = heights.iter().copied().max().unwrap_or(next_height);
                next_height = observed_max.saturating_add(1).min(target_height);
                last_error = None;
            }
            Err(err) => {
                last_error = Some(format!("{err:?}"));
                sleep(Duration::from_millis(500)).await;
            }
        }
    }
}
fn detect_height_from_storage(peer: &NetworkPeer) -> Option<u64> {
    let storage_dir = peer
        .latest_stdout_log_path()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .map(|dir| dir.join("storage"))?;
    let mut max_height = 0;
    let candidates = [storage_dir.join("pipeline"), storage_dir.join("blocks")];
    for candidate in candidates {
        if let Ok(entries) = fs::read_dir(&candidate) {
            for entry in entries.flatten() {
                let path = entry.path();
                let pipeline_dir = if path.is_dir() {
                    path.join("pipeline")
                } else {
                    continue;
                };
                if let Ok(pipeline_entries) = fs::read_dir(&pipeline_dir) {
                    for pe in pipeline_entries.flatten() {
                        if let Some(name) = pe.file_name().to_str()
                            && let Some(stripped) = name.strip_prefix("block_")
                            && let Some(idx_part) = stripped.strip_suffix(".norito")
                            && let Ok(height) = idx_part.parse::<u64>()
                        {
                            max_height = max_height.max(height);
                        }
                    }
                }
            }
        }
    }
    (max_height > 0).then_some(max_height)
}
#[test]
fn heights_meet_target_respects_skew_and_min_height() {
    assert!(heights_meet_target(&[4, 4, 4], 4, 0));
    assert!(!heights_meet_target(&[3, 4, 4], 4, 0));
    assert!(heights_meet_target(&[3, 4, 4], 4, 1));
    assert!(!heights_meet_target(&[3, 4, 6], 4, 1));
    assert!(!heights_meet_target(&[], 4, 1));
}
#[test]
fn heights_meet_target_tolerating_lag_accepts_quorum_progress() {
    assert!(heights_meet_target_tolerating_lag(&[5, 2, 4, 4], 5, 2, 1));
    assert!(!heights_meet_target_tolerating_lag(&[5, 2, 4, 4], 6, 2, 1));
    assert!(!heights_meet_target_tolerating_lag(&[5, 2, 2, 4], 5, 2, 1));
}
#[test]
fn all_peer_heights_advanced_requires_every_matching_peer_to_progress() {
    assert!(all_peer_heights_advanced(&[4, 4, 4, 4], &[5, 6, 5, 7]));
    assert!(!all_peer_heights_advanced(&[4, 4, 4, 4], &[5, 4, 5, 7]));
    assert!(!all_peer_heights_advanced(&[4, 4, 4, 4], &[5, 6, 5]));
    assert!(!all_peer_heights_advanced(&[], &[]));
}
#[test]
fn min_connected_peers_for_submit_keeps_quorum_margin() {
    assert_eq!(min_connected_peers_for_submit(0), 0);
    assert_eq!(min_connected_peers_for_submit(1), 0);
    assert_eq!(min_connected_peers_for_submit(2), 0);
    assert_eq!(min_connected_peers_for_submit(3), 1);
    assert_eq!(min_connected_peers_for_submit(4), 2);
}
#[test]
fn pick_fallback_submit_peer_index_prefers_best_height_round_robin() {
    let totals = [7, 11, 11, 3];
    assert_eq!(pick_fallback_submit_peer_index(&totals, 0), 1);
    assert_eq!(pick_fallback_submit_peer_index(&totals, 1), 2);
    assert_eq!(pick_fallback_submit_peer_index(&totals, 2), 1);
    assert_eq!(pick_fallback_submit_peer_index(&[], 42), 0);
}
#[test]
fn pick_submit_peer_index_prefers_connected_leader() {
    let totals = [4, 9, 9, 1];
    assert_eq!(pick_submit_peer_index(Some(3), true, &totals, 0), 3);
    assert_eq!(pick_submit_peer_index(Some(3), false, &totals, 0), 1);
    assert_eq!(pick_submit_peer_index(None, true, &totals, 1), 2);
}
#[test]
fn ordered_submit_peer_indices_prioritize_leader_then_fallback_cycle() {
    let totals = [4, 9, 9, 1];
    assert_eq!(
        ordered_submit_peer_indices(Some(3), true, &totals, 0),
        vec![3, 1, 2, 0]
    );
    assert_eq!(
        ordered_submit_peer_indices(Some(3), false, &totals, 0),
        vec![1, 2, 3, 0]
    );
    assert_eq!(
        ordered_submit_peer_indices(None, true, &totals, 1),
        vec![2, 3, 0, 1]
    );
}
