//! Bounded-latency localnet smoke test for permissioned Sumeragi with DA enabled.

use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, ensure, eyre};
use futures_util::future::try_join_all;
use integration_tests::sandbox;
use iroha::data_model::{
    Level,
    isi::{InstructionBox, Log, SetParameter},
    parameter::{BlockParameter, Parameter},
};
use iroha_test_network::{Network, NetworkBuilder, init_instruction_registry};
use nonzero_ext::nonzero;
use tokio::{sync::Mutex, time::sleep};

static LOCALNET_SMOKE_GUARD: OnceLock<Mutex<()>> = OnceLock::new();
const SMOKE_PIPELINE_TIME: Duration = Duration::from_secs(2);
const STATUS_POLL_TIMEOUT: Duration = Duration::from_secs(5);
const STATUS_LOG_INTERVAL: Duration = Duration::from_secs(2);
const SOAK_PIPELINE_TIME: Duration = Duration::from_secs(2);
const SOAK_STATUS_POLL_TIMEOUT: Duration = Duration::from_secs(10);
const SOAK_TARGET_BLOCKS: u64 = 2_000;
const SOAK_SUBMIT_BATCH: u64 = 25;
const SOAK_QUEUE_SOFT_LIMIT: u64 = 200;
const SOAK_QUEUE_PROGRESS_TIMEOUT: Duration = Duration::from_secs(3 * 60);
const SOAK_STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);
const SOAK_PROGRESS_LOG_INTERVAL: Duration = Duration::from_secs(10);
const SOAK_STALL_THRESHOLD: Duration = Duration::from_secs(40);
const SOAK_CLIENT_TTL: Duration = Duration::from_secs(2 * 60 * 60);

#[allow(unsafe_code)]
fn set_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
    // Safety: tests serialize env mutation with LOCALNET_SMOKE_GUARD.
    unsafe {
        std::env::set_var(key, value);
    }
}

#[allow(unsafe_code)]
fn remove_env_var(key: &str) {
    // Safety: tests serialize env mutation with LOCALNET_SMOKE_GUARD.
    unsafe {
        std::env::remove_var(key);
    }
}

fn env_or_default(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn permissioned_localnet_produces_blocks_within_bound() -> Result<()> {
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_pipeline_time(SMOKE_PIPELINE_TIME)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "permissioned")
                .write(["network", "transaction_gossip_period_ms"], 200_i64)
                .write(
                    ["network", "transaction_gossip_restricted_fallback"],
                    "public_overlay",
                )
                .write(
                    ["network", "transaction_gossip_restricted_public_payload"],
                    "forward",
                )
                // Tighten local timeouts to keep proposal/view-change cadence bounded.
                .write(["sumeragi", "npos", "timeouts", "propose_ms"], 400_i64)
                .write(["sumeragi", "npos", "timeouts", "prevote_ms"], 800_i64)
                .write(["sumeragi", "npos", "timeouts", "precommit_ms"], 1_200_i64)
                .write(["sumeragi", "npos", "timeouts", "commit_ms"], 1_600_i64)
                .write(["sumeragi", "npos", "timeouts", "da_ms"], 800_i64)
                .write(["sumeragi", "pacemaker_max_backoff_ms"], 2_000_i64)
                .write(["sumeragi", "pacemaker_rtt_floor_multiplier"], 1_i64);
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(permissioned_localnet_produces_blocks_within_bound),
    )
    .await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        let baseline_statuses = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        let baseline_height = baseline_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let baseline_view_changes: Vec<u64> = baseline_statuses
            .iter()
            .map(|status| status.view_changes.into())
            .collect();
        let peer_count = network.peers().len();
        let fault_tolerance = peer_count.saturating_sub(1) / 3;
        let max_extra_view_changes = u64::try_from(fault_tolerance.saturating_add(2))
            .unwrap_or(u64::MAX);

        let submit_peer = network
            .peers()
            .first()
            .cloned()
            .ok_or_else(|| eyre!("network must have at least one peer"))?;
        let client = submit_peer.client();
        client
            .submit::<InstructionBox>(
                Log::new(Level::INFO, "localnet bounded block".to_string()).into(),
            )
            .wrap_err("failed to submit log instruction")?;

        let target_height = baseline_height.saturating_add(1);
        let start = Instant::now();
        wait_for_converged_height(&network, target_height, Duration::from_secs(15)).await?;
        let elapsed = start.elapsed();
        ensure!(
            elapsed <= Duration::from_secs(15),
            "block production exceeded bound: elapsed={:?}",
            elapsed
        );

        let after_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        ensure!(
            after_statuses
                .iter()
                .all(|status| status.blocks >= target_height),
            "not all peers reached target height {target_height}: {after_statuses:?}"
        );
        for (idx, status) in after_statuses.iter().enumerate() {
            let before = baseline_view_changes.get(idx).copied().unwrap_or_default();
            ensure!(
                u64::from(status.view_changes) <= before.saturating_add(max_extra_view_changes),
                "peer {idx} experienced repeated view changes: before={before}, after={}, max_extra={max_extra_view_changes}",
                status.view_changes,
            );
        }
        let min_view_changes = after_statuses
            .iter()
            .map(|status| u64::from(status.view_changes))
            .min()
            .unwrap_or_default();
        let max_view_changes = after_statuses
            .iter()
            .map(|status| u64::from(status.view_changes))
            .max()
            .unwrap_or_default();
        ensure!(
            max_view_changes.saturating_sub(min_view_changes) <= max_extra_view_changes,
            "view_change counters diverged across peers: {after_statuses:?}"
        );

        network.shutdown().await;
        Ok(())
    }
    .await;

    if sandbox::handle_result(
        result,
        stringify!(permissioned_localnet_produces_blocks_within_bound),
    )?
    .is_none()
    {
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[allow(clippy::too_many_lines)]
async fn permissioned_localnet_reaches_100_blocks() -> Result<()> {
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_pipeline_time(SMOKE_PIPELINE_TIME)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "permissioned")
                .write(["network", "transaction_gossip_period_ms"], 200_i64)
                .write(
                    ["network", "transaction_gossip_restricted_fallback"],
                    "public_overlay",
                )
                .write(
                    ["network", "transaction_gossip_restricted_public_payload"],
                    "forward",
                )
                // Tighten local timeouts to keep proposal/view-change cadence bounded.
                .write(["sumeragi", "npos", "timeouts", "propose_ms"], 200_i64)
                .write(["sumeragi", "npos", "timeouts", "prevote_ms"], 400_i64)
                .write(["sumeragi", "npos", "timeouts", "precommit_ms"], 600_i64)
                .write(["sumeragi", "npos", "timeouts", "commit_ms"], 800_i64)
                .write(["sumeragi", "npos", "timeouts", "da_ms"], 400_i64)
                .write(["sumeragi", "pacemaker_max_backoff_ms"], 2_000_i64)
                .write(["sumeragi", "pacemaker_rtt_floor_multiplier"], 1_i64);
        });

    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(permissioned_localnet_reaches_100_blocks),
    )
    .await?
    else {
        return Ok(());
    };

    let result: Result<()> = async {
        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        let baseline_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let baseline_height = baseline_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();

        let target_blocks = 100_u64;
        let target_height = baseline_height.saturating_add(target_blocks);
        let submit_peer = network
            .peers()
            .first()
            .cloned()
            .ok_or_else(|| eyre!("network must have at least one peer"))?;
        let client = submit_peer.client();
        for idx in 0..target_blocks {
            client
                .submit::<InstructionBox>(
                    Log::new(Level::INFO, format!("localnet block {idx}")).into(),
                )
                .wrap_err_with(|| format!("failed to submit log instruction {idx}"))?;
        }

        let timeout = scale_duration(network.pipeline_time(), target_blocks)
            .saturating_add(Duration::from_secs(30));
        let start = Instant::now();
        wait_for_converged_height(&network, target_height, timeout).await?;
        let elapsed = start.elapsed();
        ensure!(
            elapsed <= timeout,
            "block production exceeded bound: elapsed={:?}",
            elapsed
        );

        let after_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        ensure!(
            after_statuses
                .iter()
                .all(|status| status.blocks >= target_height),
            "not all peers reached target height {target_height}: {after_statuses:?}"
        );

        network.shutdown().await;
        Ok(())
    }
    .await;

    if sandbox::handle_result(result, stringify!(permissioned_localnet_reaches_100_blocks))?
        .is_none()
    {
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "long-running localnet soak (thousands of blocks/tx)"]
#[allow(clippy::too_many_lines)]
async fn permissioned_localnet_soak_thousands() -> Result<()> {
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;

    let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");
    // Extend TTL so early transactions do not expire during the soak run.
    set_env_var(
        "IROHA_TEST_CLIENT_TTL_MS",
        SOAK_CLIENT_TTL.as_millis().to_string(),
    );

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_pipeline_time(SOAK_PIPELINE_TIME)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "permissioned")
                .write(["network", "transaction_gossip_period_ms"], 200_i64)
                .write(
                    ["network", "transaction_gossip_restricted_fallback"],
                    "public_overlay",
                )
                .write(
                    ["network", "transaction_gossip_restricted_public_payload"],
                    "forward",
                )
                // Tighten local timeouts to keep proposal/view-change cadence bounded.
                .write(["sumeragi", "npos", "timeouts", "propose_ms"], 200_i64)
                .write(["sumeragi", "npos", "timeouts", "prevote_ms"], 400_i64)
                .write(["sumeragi", "npos", "timeouts", "precommit_ms"], 600_i64)
                .write(["sumeragi", "npos", "timeouts", "commit_ms"], 800_i64)
                .write(["sumeragi", "npos", "timeouts", "da_ms"], 400_i64)
                // Give DA quorum extra breathing room under sustained load.
                .write(["sumeragi", "da_quorum_timeout_multiplier"], 7_i64)
                .write(["sumeragi", "da_availability_timeout_multiplier"], 3_i64)
                .write(["sumeragi", "pacemaker_max_backoff_ms"], 10_000_i64)
                .write(["sumeragi", "pacemaker_rtt_floor_multiplier"], 2_i64);
        });

    let result: Result<()> = async {
        let Some(network) = sandbox::start_network_async_or_skip(
            builder,
            stringify!(permissioned_localnet_soak_thousands),
        )
        .await?
        else {
            return Ok(());
        };

        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        let baseline_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let baseline_non_empty = baseline_statuses
            .iter()
            .map(|status| status.blocks_non_empty)
            .min()
            .unwrap_or_default();

        // Allow shorter local runs via IROHA_SOAK_TARGET_BLOCKS while keeping the default.
        let target_blocks = env_or_default("IROHA_SOAK_TARGET_BLOCKS", SOAK_TARGET_BLOCKS);
        let submit_batch = env_or_default("IROHA_SOAK_SUBMIT_BATCH", SOAK_SUBMIT_BATCH);
        let queue_soft_limit =
            env_or_default("IROHA_SOAK_QUEUE_SOFT_LIMIT", SOAK_QUEUE_SOFT_LIMIT);
        let target_height = baseline_non_empty.saturating_add(target_blocks);
        let submit_peer = network
            .peers()
            .first()
            .cloned()
            .ok_or_else(|| eyre!("network must have at least one peer"))?;
        let client = submit_peer.client();
        for idx in 0..target_blocks {
            client
                .submit::<InstructionBox>(
                    Log::new(Level::INFO, format!("localnet soak {idx}")).into(),
                )
                .wrap_err_with(|| format!("failed to submit log instruction {idx}"))?;
            if (idx + 1) % submit_batch == 0 {
                wait_for_queue_depth(&network, queue_soft_limit, SOAK_STATUS_POLL_TIMEOUT).await?;
            }
        }

        let mut last_progress = Instant::now();
        let mut last_min_non_empty = baseline_non_empty;
        let mut last_log = Instant::now()
            .checked_sub(SOAK_PROGRESS_LOG_INTERVAL)
            .unwrap_or_else(Instant::now);
        let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();

        loop {
            if let Ok(statuses) = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await {
                let min_non_empty = statuses
                    .iter()
                    .map(|status| status.blocks_non_empty)
                    .min()
                    .unwrap_or_default();
                let max_non_empty = statuses
                    .iter()
                    .map(|status| status.blocks_non_empty)
                    .max()
                    .unwrap_or_default();
                if min_non_empty > last_min_non_empty {
                    last_min_non_empty = min_non_empty;
                    last_progress = Instant::now();
                }
                last_snapshot = statuses
                    .iter()
                    .map(StatusSnapshot::from_status)
                    .collect();
                if last_log.elapsed() >= SOAK_PROGRESS_LOG_INTERVAL {
                    eprintln!(
                        "localnet soak progress (target_non_empty={target_height}, min_non_empty={min_non_empty}, max_non_empty={max_non_empty}): {last_snapshot:?}"
                    );
                    last_log = Instant::now();
                }
                if statuses
                    .iter()
                    .all(|status| status.blocks_non_empty >= target_height)
                {
                    break;
                }
            }

            if last_progress.elapsed() >= SOAK_STALL_THRESHOLD {
                return Err(eyre!(
                    "localnet soak stalled for {:?} (min_non_empty={last_min_non_empty}, target_non_empty={target_height}): last_snapshot={last_snapshot:?}",
                    SOAK_STALL_THRESHOLD
                ));
            }

            sleep(SOAK_STATUS_POLL_INTERVAL).await;
        }

        let after_statuses = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        ensure!(
            after_statuses
                .iter()
                .all(|status| status.blocks_non_empty >= target_height),
            "not all peers reached target non-empty height {target_height}: {after_statuses:?}"
        );

        network.shutdown().await;
        Ok(())
    }
    .await;

    if let Some(previous_ttl) = previous_ttl {
        set_env_var("IROHA_TEST_CLIENT_TTL_MS", previous_ttl);
    } else {
        remove_env_var("IROHA_TEST_CLIENT_TTL_MS");
    }

    if sandbox::handle_result(result, stringify!(permissioned_localnet_soak_thousands))?.is_none() {
        return Ok(());
    }
    Ok(())
}

async fn collect_statuses(
    network: &Network,
    status_timeout: Duration,
) -> Result<Vec<iroha::client::Status>> {
    try_join_all(network.peers().iter().map(|peer| async move {
        tokio::time::timeout(status_timeout, peer.status())
            .await
            .map_or_else(
                |_| {
                    eprintln!(
                        "status request timed out for peer {} after {:?} (best_effort={:?}, last_known_peers={:?}, stdout={:?})",
                        peer.mnemonic(),
                        status_timeout,
                        peer.best_effort_block_height(),
                        peer.last_known_peers(),
                        peer.latest_stdout_log_path()
                    );
                    Err(eyre!(
                        "status request timed out after {:?} for peer {}",
                        status_timeout,
                        peer.mnemonic()
                    ))
                },
                |result| {
                    result
                        .map_err(|err| {
                            eprintln!(
                                "status request failed for peer {}: {err:?} (best_effort={:?}, last_known_peers={:?}, stdout={:?})",
                                peer.mnemonic(),
                                peer.best_effort_block_height(),
                                peer.last_known_peers(),
                                peer.latest_stdout_log_path()
                            );
                            err
                        })
                        .wrap_err_with(|| format!("status request failed for peer {}", peer.mnemonic()))
                },
            )
    }))
    .await
}

async fn wait_for_status_responses(network: &Network, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_log = Instant::now()
        .checked_sub(STATUS_LOG_INTERVAL)
        .unwrap_or_else(Instant::now);
    loop {
        match collect_statuses(network, STATUS_POLL_TIMEOUT).await {
            Ok(_) => return Ok(()),
            Err(err) => {
                if last_log.elapsed() >= STATUS_LOG_INTERVAL {
                    eprintln!(
                        "waiting for status responses (timeout={timeout:?}): last_error={err:?}"
                    );
                    last_log = Instant::now();
                }
                if Instant::now() >= deadline {
                    return Err(eyre!(
                        "status responses did not converge within {:?}; last_error={err:?}",
                        timeout,
                    ));
                }
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
}

async fn wait_for_queue_depth(
    network: &Network,
    max_queue: u64,
    status_timeout: Duration,
) -> Result<()> {
    let mut last_progress = Instant::now();
    let mut last_queue: Option<u64> = None;
    let mut last_blocks_non_empty: Option<u64> = None;
    let mut last_log = Instant::now()
        .checked_sub(STATUS_LOG_INTERVAL)
        .unwrap_or_else(Instant::now);
    loop {
        match collect_statuses(network, status_timeout).await {
            Ok(statuses) => {
                let submitter_queue = statuses
                    .first()
                    .map(|status| status.queue_size)
                    .unwrap_or_default();
                let min_non_empty = statuses
                    .iter()
                    .map(|status| status.blocks_non_empty)
                    .min()
                    .unwrap_or_default();
                if submitter_queue <= max_queue {
                    return Ok(());
                }
                let progressed = last_queue.is_some_and(|prev| submitter_queue < prev)
                    || last_blocks_non_empty.is_some_and(|prev| min_non_empty > prev);
                if progressed {
                    last_progress = Instant::now();
                }
                last_queue = Some(submitter_queue);
                last_blocks_non_empty = Some(min_non_empty);
                if last_log.elapsed() >= STATUS_LOG_INTERVAL {
                    eprintln!(
                        "waiting for submit queue to drain (queue_size={submitter_queue}, limit={max_queue}, min_non_empty={min_non_empty})"
                    );
                    last_log = Instant::now();
                }
            }
            Err(err) => {
                if last_log.elapsed() >= STATUS_LOG_INTERVAL {
                    eprintln!("waiting for submit queue to drain: status poll failed: {err:?}");
                    last_log = Instant::now();
                }
            }
        }

        if last_progress.elapsed() >= SOAK_QUEUE_PROGRESS_TIMEOUT {
            return Err(eyre!(
                "submit queue did not drain below {max_queue} within {:?}",
                SOAK_QUEUE_PROGRESS_TIMEOUT
            ));
        }

        sleep(SOAK_STATUS_POLL_INTERVAL).await;
    }
}

#[tokio::test]
async fn env_or_default_reads_positive_values() {
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    let key = "IROHA_ENV_OR_DEFAULT_TEST";
    set_env_var(key, "42");
    assert_eq!(env_or_default(key, 7), 42);
    remove_env_var(key);
}

#[tokio::test]
async fn env_or_default_ignores_invalid_or_zero() {
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    let key = "IROHA_ENV_OR_DEFAULT_TEST";
    set_env_var(key, "0");
    assert_eq!(env_or_default(key, 7), 7);
    set_env_var(key, "nope");
    assert_eq!(env_or_default(key, 7), 7);
    remove_env_var(key);
}

async fn wait_for_converged_height(
    network: &Network,
    target_height: u64,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();
    let mut last_log = Instant::now()
        .checked_sub(STATUS_LOG_INTERVAL)
        .unwrap_or_else(Instant::now);
    loop {
        match collect_statuses(network, STATUS_POLL_TIMEOUT).await {
            Ok(statuses) => {
                let snapshot: Vec<StatusSnapshot> =
                    statuses.iter().map(StatusSnapshot::from_status).collect();
                if snapshot != last_snapshot || last_log.elapsed() >= STATUS_LOG_INTERVAL {
                    eprintln!(
                        "localnet status snapshot (target_height={target_height}): {snapshot:?}"
                    );
                    last_log = Instant::now();
                }
                last_snapshot = snapshot;
                if statuses.iter().all(|status| status.blocks >= target_height) {
                    let first_height = statuses.first().map(|s| s.blocks);
                    if statuses
                        .iter()
                        .all(|status| Some(status.blocks) == first_height)
                    {
                        return Ok(());
                    }
                }
                if Instant::now() >= deadline {
                    return Err(eyre!(
                        "heights failed to converge to {target_height} within {:?}: last_snapshot={last_snapshot:?}",
                        timeout
                    ));
                }
            }
            Err(err) => {
                if Instant::now() >= deadline {
                    return Err(eyre!(
                        "heights failed to converge to {target_height} within {:?}: last_snapshot={last_snapshot:?}, last_error={err:?}",
                        timeout
                    ));
                }
            }
        }
        sleep(Duration::from_millis(200)).await;
    }
}

fn scale_duration(duration: Duration, factor: u64) -> Duration {
    let total_ms = duration.as_millis().saturating_mul(u128::from(factor));
    Duration::from_millis(u64::try_from(total_ms).unwrap_or(u64::MAX))
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
struct StatusSnapshot {
    blocks: u64,
    queue_size: u64,
    txs_approved: u64,
    txs_rejected: u64,
    view_changes: u32,
    leader_index: Option<u64>,
    highest_qc_height: Option<u64>,
    locked_qc_height: Option<u64>,
    tx_queue_depth: Option<u64>,
    tx_queue_saturated: Option<bool>,
    block_created_dropped_by_lock_total: Option<u64>,
    block_created_hint_mismatch_total: Option<u64>,
    block_created_proposal_mismatch_total: Option<u64>,
    commit_signatures_present: Option<u64>,
    commit_signatures_required: Option<u64>,
}

impl StatusSnapshot {
    fn from_status(status: &iroha::client::Status) -> Self {
        let sumeragi = status.sumeragi.as_ref();
        Self {
            blocks: status.blocks,
            queue_size: status.queue_size,
            txs_approved: status.txs_approved,
            txs_rejected: status.txs_rejected,
            view_changes: status.view_changes,
            leader_index: sumeragi.map(|s| s.leader_index),
            highest_qc_height: sumeragi.map(|s| s.highest_qc_height),
            locked_qc_height: sumeragi.map(|s| s.locked_qc_height),
            tx_queue_depth: sumeragi.map(|s| s.tx_queue_depth),
            tx_queue_saturated: sumeragi.map(|s| s.tx_queue_saturated),
            block_created_dropped_by_lock_total: sumeragi
                .map(|s| s.block_created_dropped_by_lock_total),
            block_created_hint_mismatch_total: sumeragi
                .map(|s| s.block_created_hint_mismatch_total),
            block_created_proposal_mismatch_total: sumeragi
                .map(|s| s.block_created_proposal_mismatch_total),
            commit_signatures_present: sumeragi.map(|s| s.commit_signatures_present),
            commit_signatures_required: sumeragi.map(|s| s.commit_signatures_required),
        }
    }
}
