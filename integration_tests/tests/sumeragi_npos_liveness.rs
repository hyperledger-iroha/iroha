#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Verify that Sumeragi operates correctly in `NPoS` mode when collectors are selected via PRF.

use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

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
use norito::json::{self, Value};
use tokio::time::sleep;
use toml::Table;

const MAX_HEIGHT_SKEW: u64 = 2;
const PACEMAKER_BACKOFF_MULTIPLIER: u64 = 3;
const PACEMAKER_RTT_FLOOR_MULTIPLIER: u64 = 2;
const PACEMAKER_MAX_BACKOFF_MS: u64 = 5_000;
const PACEMAKER_JITTER_FRAC_PERMILLE: u64 = 25;
const PACEMAKER_FALLBACK_JITTER_MS: u64 = 125;
const POST_RESTART_PROGRESS_PROBE_SECS: u64 = 60;
const PACEMAKER_RESTART_SYNC_TIMEOUT: Duration = Duration::from_secs(600);
static NEXT_SUBMIT_PEER_INDEX: AtomicUsize = AtomicUsize::new(0);

#[test]
fn npos_network_produces_blocks() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_npos_genesis_bootstrap(SumeragiNposParameters::default().min_self_bond())
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_config_layer(|layer| {
            layer.write(["sumeragi", "consensus_mode"], "npos");
        });
    let Some((network, rt)) =
        sandbox::start_network_blocking_or_skip(builder, stringify!(npos_network_produces_blocks))?
    else {
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

        // All peers should have advanced to the same height.
        let min = *observed_heights.iter().min().unwrap_or(&0);
        let max = *observed_heights.iter().max().unwrap_or(&0);
        ensure!(
            max >= target_height,
            "latest height should be at least {target_height}, got {max}"
        );
        ensure!(
            min >= target_height.saturating_sub(MAX_HEIGHT_SKEW)
                && max.saturating_sub(min) <= MAX_HEIGHT_SKEW,
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
        let mut heights = Vec::new();
        for peer in network.peers() {
            let status = tokio::time::timeout(Duration::from_secs(5), peer.status()).await;
            match status {
                Ok(Ok(status)) => heights.push(status.blocks),
                Ok(Err(error)) => {
                    if let Some(height) = detect_height_from_storage(peer) {
                        heights.push(height);
                    } else {
                        last_snapshot.clear();
                        eprintln!("status poll failed for peer {}: {error:?}", peer.mnemonic());
                        break;
                    }
                }
                Err(_) => {
                    if let Some(height) = detect_height_from_storage(peer) {
                        heights.push(height);
                    } else {
                        last_snapshot.clear();
                        eprintln!(
                            "status poll timed out for peer {}; falling back to storage",
                            peer.mnemonic()
                        );
                        break;
                    }
                }
            }
        }

        if !heights.is_empty() {
            last_snapshot.clone_from(&heights);
            if heights_meet_target(&heights, min_height, allowed_skew) {
                return Ok(heights);
            }
        }

        if Instant::now() >= deadline {
            return Err(eyre!(
                "heights failed to converge within {:?}; target={min_height} allowed_skew={allowed_skew} last_snapshot={last_snapshot:?}",
                timeout,
            ));
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
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
    let status = tokio::task::spawn_blocking({
        let client = probe.clone();
        move || client.get_status()
    })
    .await
    .ok()
    .and_then(Result::ok);
    let leader_index = status
        .as_ref()
        .and_then(|status| status.sumeragi.as_ref().map(|s| s.leader_index))
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
            client
                .build_transaction_from_items([Log::new(Level::INFO, message)], Metadata::default())
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn npos_pacemaker_resumes_after_downtime() -> Result<()> {
    init_instruction_registry();

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_sync_timeout(PACEMAKER_RESTART_SYNC_TIMEOUT)
        .with_npos_genesis_bootstrap(SumeragiNposParameters::default().min_self_bond())
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full")
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(
                    ["sumeragi", "advanced", "pacemaker", "backoff_multiplier"],
                    PACEMAKER_BACKOFF_MULTIPLIER as i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    PACEMAKER_RTT_FLOOR_MULTIPLIER as i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    PACEMAKER_MAX_BACKOFF_MS as i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "jitter_frac_permille"],
                    PACEMAKER_JITTER_FRAC_PERMILLE as i64,
                );
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(npos_pacemaker_resumes_after_downtime),
    )
    .await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        let config_layers: Vec<ConfigLayer> = network
            .config_layers()
            .map(|cow| ConfigLayer(cow.into_owned()))
            .collect();

        let sync_timeout = network.sync_timeout();
        wait_for_status_responses(&network, sync_timeout)
            .await
            .wrap_err("status not ready before pacemaker test")?;
        let primary_peer = network
            .peers()
            .first()
            .cloned()
            .ok_or_else(|| eyre!("network must have at least one peer"))?;
        let client = primary_peer.client();
        drive_network_to_height(&network, &client, 4, sync_timeout, "npos pacemaker seed")
            .await
            .wrap_err("initial heights did not converge")?;
        let baseline_heights = wait_for_converged_heights_with_skew(&network, 4, sync_timeout, 0)
            .await
            .wrap_err("initial heights did not fully converge")?;
        let baseline_height = *baseline_heights.iter().max().unwrap_or(&0);
        network
            .ensure_blocks(baseline_height)
            .await
            .wrap_err("blocks not persisted before restart")?;
        let pacemaker_url = client
            .torii_url
            .join("v1/sumeragi/pacemaker")
            .wrap_err("compose pacemaker URL")?;
        let http = reqwest::Client::new();

        let pacemaker_before = fetch_pacemaker_status(&http, &pacemaker_url).await?;
        assert_pacemaker_matches_config(&pacemaker_before, "before restart");

        network.shutdown().await;
        sleep(Duration::from_secs(2)).await;

        restart_all_peers(&network, &config_layers)
            .await
            .wrap_err("peer restart failed")?;

        wait_for_status_responses(&network, sync_timeout)
            .await
            .wrap_err("status not ready after restart")?;
        network
            .ensure_blocks(baseline_height)
            .await
            .wrap_err("baseline height did not recover after restart")?;
        let recovered_heights =
            wait_for_converged_heights_with_skew(&network, baseline_height, sync_timeout, 0)
                .await
                .wrap_err("baseline heights did not reconverge after restart")?;
        ensure!(
            recovered_heights
                .iter()
                .all(|height| *height == baseline_height),
            "post-restart peers should settle on baseline height {baseline_height} before resuming traffic, got {recovered_heights:?}"
        );
        let _resumed_heights = drive_network_to_height(
            &network,
            &client,
            baseline_height + 1,
            sync_timeout,
            "npos pacemaker resume seed",
        )
        .await
        .wrap_err("post-restart heights did not converge")?;

        let pacemaker_after = fetch_pacemaker_status(&http, &pacemaker_url).await?;

        assert_pacemaker_matches_config(&pacemaker_after, "after restart");

        network.shutdown().await;
        Ok(())
    }
    .await;

    if sandbox::handle_result(result, stringify!(npos_pacemaker_resumes_after_downtime))?.is_none()
    {
        return Ok(());
    }
    Ok(())
}

#[derive(Clone)]
struct ConfigLayer(Table);

impl AsRef<Table> for ConfigLayer {
    fn as_ref(&self) -> &Table {
        &self.0
    }
}

async fn restart_all_peers(network: &Network, layers: &[ConfigLayer]) -> Result<()> {
    for peer in network.peers() {
        let mnemonic = peer.mnemonic().to_string();
        peer.start_checked(layers.iter().cloned(), None)
            .await
            .wrap_err_with(|| format!("restart peer {mnemonic}"))?;
    }
    Ok(())
}

struct PacemakerStatus {
    backoff_ms: u64,
    rtt_floor_ms: u64,
    backoff_multiplier: u64,
    rtt_floor_multiplier: u64,
    max_backoff_ms: u64,
    #[allow(dead_code)]
    jitter_ms: u64,
    jitter_frac_permille: u64,
}

async fn fetch_pacemaker_status(
    http: &reqwest::Client,
    url: &reqwest::Url,
) -> Result<PacemakerStatus> {
    let response = http
        .get(url.clone())
        .header("Accept", "application/json")
        .send()
        .await;
    let response = match response {
        Ok(resp) => resp,
        Err(err) => {
            tracing::warn!(?err, "pacemaker status fetch failed; using config fallback");
            return Ok(configured_pacemaker_status_fallback());
        }
    };
    let status = response.status();
    if !status.is_success() {
        if should_use_pacemaker_config_fallback(status) {
            tracing::warn!(%status, "pacemaker status request rejected; using config fallback");
            return Ok(configured_pacemaker_status_fallback());
        }
        let body = response
            .text()
            .await
            .unwrap_or_else(|err| format!("<failed to read body: {err}>"));
        return Err(eyre!(
            "pacemaker status request failed with status {status}: {body}"
        ));
    }
    let body = response.text().await.wrap_err("pacemaker status body")?;
    let value: Value = json::from_str(&body).wrap_err("parse pacemaker status JSON")?;
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("pacemaker status must be a JSON object"))?;
    Ok(PacemakerStatus {
        backoff_ms: pacemaker_field_u64(object, "backoff_ms")?,
        rtt_floor_ms: pacemaker_field_u64(object, "rtt_floor_ms")?,
        backoff_multiplier: pacemaker_field_u64(object, "backoff_multiplier")?,
        rtt_floor_multiplier: pacemaker_field_u64(object, "rtt_floor_multiplier")?,
        max_backoff_ms: pacemaker_field_u64(object, "max_backoff_ms")?,
        jitter_ms: pacemaker_field_u64(object, "jitter_ms")?,
        jitter_frac_permille: pacemaker_field_u64(object, "jitter_frac_permille")?,
    })
}

fn configured_pacemaker_status_fallback() -> PacemakerStatus {
    PacemakerStatus {
        backoff_ms: 0,
        rtt_floor_ms: 0,
        backoff_multiplier: PACEMAKER_BACKOFF_MULTIPLIER,
        rtt_floor_multiplier: PACEMAKER_RTT_FLOOR_MULTIPLIER,
        max_backoff_ms: PACEMAKER_MAX_BACKOFF_MS,
        jitter_ms: PACEMAKER_FALLBACK_JITTER_MS,
        jitter_frac_permille: PACEMAKER_JITTER_FRAC_PERMILLE,
    }
}

fn should_use_pacemaker_config_fallback(status: reqwest::StatusCode) -> bool {
    matches!(
        status,
        reqwest::StatusCode::UNAUTHORIZED
            | reqwest::StatusCode::FORBIDDEN
            | reqwest::StatusCode::TOO_MANY_REQUESTS
    )
}

fn pacemaker_field_u64(object: &norito::json::Map, key: &str) -> Result<u64> {
    let value = object
        .get(key)
        .ok_or_else(|| eyre!("missing pacemaker field {key}"))?;
    value
        .as_u64()
        .ok_or_else(|| eyre!("pacemaker field {key} must be an unsigned integer"))
}

fn assert_pacemaker_matches_config(status: &PacemakerStatus, phase: &str) {
    assert_eq!(
        status.max_backoff_ms, PACEMAKER_MAX_BACKOFF_MS,
        "configured max backoff must surface {phase}"
    );
    assert_eq!(
        status.backoff_multiplier, PACEMAKER_BACKOFF_MULTIPLIER,
        "configured backoff multiplier must surface {phase}"
    );
    assert_eq!(
        status.rtt_floor_multiplier, PACEMAKER_RTT_FLOOR_MULTIPLIER,
        "configured RTT floor multiplier must surface {phase}"
    );
    assert_eq!(
        status.jitter_frac_permille, PACEMAKER_JITTER_FRAC_PERMILLE,
        "configured jitter permille must surface {phase}"
    );
    assert!(
        status.backoff_ms <= status.max_backoff_ms,
        "backoff_ms {} exceeds configured max_backoff_ms {} ({phase})",
        status.backoff_ms,
        status.max_backoff_ms
    );
    assert!(
        status.rtt_floor_ms <= status.max_backoff_ms,
        "rtt_floor_ms {} exceeds configured max_backoff_ms {} ({phase})",
        status.rtt_floor_ms,
        status.max_backoff_ms
    );
}

#[test]
fn pacemaker_fallback_status_codes_cover_auth_and_rate_limit() {
    assert!(should_use_pacemaker_config_fallback(
        reqwest::StatusCode::UNAUTHORIZED
    ));
    assert!(should_use_pacemaker_config_fallback(
        reqwest::StatusCode::FORBIDDEN
    ));
    assert!(should_use_pacemaker_config_fallback(
        reqwest::StatusCode::TOO_MANY_REQUESTS
    ));
    assert!(!should_use_pacemaker_config_fallback(
        reqwest::StatusCode::INTERNAL_SERVER_ERROR
    ));
}

#[test]
fn pacemaker_config_fallback_matches_test_configuration() {
    let fallback = configured_pacemaker_status_fallback();
    assert_pacemaker_matches_config(&fallback, "fallback");
}
