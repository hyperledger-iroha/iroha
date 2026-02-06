#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Bounded-latency localnet smoke and throughput tests for permissioned and `NPoS` Sumeragi.

use std::{
    cmp::Ordering,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    sync::OnceLock,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use blake3::Hasher as Blake3Hasher;
use eyre::{Result, WrapErr, bail, ensure, eyre};
use futures_util::{StreamExt, TryStreamExt, future::try_join_all, stream};
use integration_tests::sandbox;
use iroha::data_model::{
    Level,
    block::consensus::SumeragiStatusWire,
    isi::{InstructionBox, Log, SetParameter},
    metadata::Metadata,
    name::Name,
    parameter::{BlockParameter, Parameter, SumeragiParameter, system::SumeragiNposParameters},
};
use iroha_test_network::{Network, NetworkBuilder, init_instruction_registry};
use nonzero_ext::nonzero;
use norito::json::{Map, Value};
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use reqwest::Client as HttpClient;
use tempfile::tempdir;
use tokio::{sync::Mutex, task, time::sleep};
use toml::Value as TomlValue;

static LOCALNET_SMOKE_GUARD: OnceLock<Mutex<()>> = OnceLock::new();
const SMOKE_PIPELINE_TIME: Duration = Duration::from_secs(2);
const SMOKE_BLOCK_TIME_MS: u64 = 1_000;
const SMOKE_COMMIT_TIME_MS: u64 = 1_000;
const STATUS_POLL_TIMEOUT: Duration = Duration::from_secs(15);
const STATUS_LOG_INTERVAL: Duration = Duration::from_secs(2);
const SOAK_PIPELINE_TIME: Duration = Duration::from_secs(2);
const SOAK_BLOCK_TIME_MS: u64 = 1_000;
const SOAK_COMMIT_TIME_MS: u64 = 1_000;
const SOAK_STATUS_POLL_TIMEOUT: Duration = Duration::from_secs(20);
const SOAK_TARGET_BLOCKS: u64 = 2_000;
const SOAK_SUBMIT_BATCH: u64 = 25;
const SOAK_QUEUE_SOFT_LIMIT: u64 = 200;
const SOAK_QUEUE_PROGRESS_TIMEOUT: Duration = Duration::from_secs(3 * 60);
const SOAK_STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);
const SOAK_PROGRESS_LOG_INTERVAL: Duration = Duration::from_secs(10);
const SOAK_STALL_THRESHOLD: Duration = Duration::from_secs(40);
const SOAK_CLIENT_TTL: Duration = Duration::from_secs(2 * 60 * 60);
const THROUGHPUT_PIPELINE_TIME: Duration = Duration::from_secs(2);
const THROUGHPUT_BLOCK_TIME_MS: u64 = 1_000;
const THROUGHPUT_COMMIT_TIME_MS: u64 = 1_000;
const THROUGHPUT_BLOCK_MAX_TXS: u64 = 10_000;
const THROUGHPUT_SUBMIT_BATCH: u64 = 512;
const THROUGHPUT_SUBMIT_PARALLELISM: u64 = 128;
const THROUGHPUT_QUEUE_SOFT_LIMIT: u64 = 20_000;
const THROUGHPUT_STALL_THRESHOLD: Duration = Duration::from_secs(60);
const THROUGHPUT_COMMIT_TIME_MAX_MULTIPLIER: u64 = 2;
const THROUGHPUT_CLIENT_TTL: Duration = Duration::from_secs(2 * 60 * 60);
const THROUGHPUT_WARMUP_BLOCKS: u64 = 10;
const THROUGHPUT_STEADY_BLOCKS: u64 = 30;
const THROUGHPUT_SAMPLE_INTERVAL: Duration = Duration::from_secs(2);
const THROUGHPUT_PROGRESS_LOG_INTERVAL: Duration = Duration::from_secs(10);
const THROUGHPUT_METRICS_TIMEOUT: Duration = Duration::from_secs(5);
const THROUGHPUT_PAYLOAD_BYTES: usize = 512;
const THROUGHPUT_RNG_SEED: u64 = 0x0049_524f_4841;
const THROUGHPUT_SLO_P95_MS: u64 = 1_500;
const THROUGHPUT_SLO_P99_MS: u64 = 2_000;
const THROUGHPUT_SLO_VIEW_CHANGE_RATE_MAX: f64 = 0.1;
const THROUGHPUT_SLO_BACKPRESSURE_RATE_MAX: f64 = 2.0;
const THROUGHPUT_SLO_QUEUE_SAT_FRAC_MAX: f64 = 0.2;
const THROUGHPUT_NPOS_SLO_P95_MS: u64 = 2_000;
const THROUGHPUT_NPOS_SLO_P99_MS: u64 = 3_000;
const THROUGHPUT_NPOS_SLO_VIEW_CHANGE_RATE_MAX: f64 = 0.2;
const THROUGHPUT_NPOS_SLO_BACKPRESSURE_RATE_MAX: f64 = 3.0;
const THROUGHPUT_NPOS_SLO_QUEUE_SAT_FRAC_MAX: f64 = 0.3;
const THROUGHPUT_QUEUE_PROGRESS_TIMEOUT_ENV: &str = "IROHA_THROUGHPUT_QUEUE_PROGRESS_TIMEOUT_SECS";

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

fn env_or_default_usize(key: &str, default: usize) -> usize {
    let default_u64 = u64::try_from(default).unwrap_or(u64::MAX);
    let value = env_or_default(key, default_u64);
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn env_or_default_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0)
        .unwrap_or(default)
}

fn queue_progress_timeout() -> Duration {
    let default_secs = SOAK_QUEUE_PROGRESS_TIMEOUT.as_secs();
    let secs = env_or_default(THROUGHPUT_QUEUE_PROGRESS_TIMEOUT_ENV, default_secs);
    Duration::from_secs(secs)
}

#[allow(clippy::too_many_arguments)]
async fn submit_logs(
    start_idx: u64,
    tx_count: u64,
    network: &Network,
    submit_clients: &[iroha::client::Client],
    submit_batch: u64,
    submit_parallelism: usize,
    queue_soft_limit: u64,
    payload_bytes: usize,
    rng_seed: u64,
) -> Result<Duration> {
    let submit_clients = std::sync::Arc::new(submit_clients.to_vec());
    ensure!(
        !submit_clients.is_empty(),
        "submit_logs requires at least one client"
    );
    let client_count = u64::try_from(submit_clients.len()).unwrap_or(1);
    let submit_start = Instant::now();
    let mut submitted = 0_u64;
    while submitted < tx_count {
        let remaining = tx_count.saturating_sub(submitted);
        let batch_count = remaining.min(submit_batch);
        let batch_start = start_idx.saturating_add(submitted);
        stream::iter(batch_start..batch_start.saturating_add(batch_count))
            .map(|idx| {
                let submit_clients = std::sync::Arc::clone(&submit_clients);
                async move {
                    if let Ok(delay) = std::env::var("IROHA_THROUGHPUT_DELAY_MS") {
                        if let Ok(ms) = delay.parse::<u64>() {
                            tokio::time::sleep(Duration::from_millis(ms)).await;
                        }
                    }
                    let payload = throughput_payload(idx, payload_bytes, rng_seed);
                    let client_idx = usize::try_from(idx % client_count).unwrap_or_default();
                    let client = submit_clients[client_idx].clone();
                    let handle = task::spawn_blocking(move || {
                        client
                            .submit::<InstructionBox>(Log::new(Level::INFO, payload).into())
                            .wrap_err_with(|| format!("failed to submit log instruction {idx}"))
                    });
                    handle.await.wrap_err("submit task join failed")?
                }
            })
            .buffer_unordered(submit_parallelism)
            .try_for_each(|_| async { Ok(()) })
            .await?;
        submitted = submitted.saturating_add(batch_count);
        wait_for_queue_depth(network, queue_soft_limit, SOAK_STATUS_POLL_TIMEOUT).await?;
    }
    Ok(submit_start.elapsed())
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
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(SMOKE_BLOCK_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(SMOKE_COMMIT_TIME_MS),
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
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "propose_ms"],
                    400_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "prevote_ms"],
                    800_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "precommit_ms"],
                    1_200_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "commit_ms"],
                    1_600_i64,
                )
                .write(
                    ["sumeragi", "advanced", "npos", "timeouts", "da_ms"],
                    800_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    2_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    1_i64,
                );
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
        let warmup_height = baseline_height.saturating_add(1);
        for peer in network.peers() {
            let message = format!("localnet warmup block {}", peer.mnemonic());
            peer.client()
                .submit::<InstructionBox>(Log::new(Level::INFO, message).into())
                .wrap_err_with(|| {
                    format!("failed to submit warmup log instruction to {}", peer.mnemonic())
                })?;
        }
        wait_for_converged_height(&network, warmup_height, Duration::from_secs(45)).await?;
        let warmup_statuses = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        let baseline_height = warmup_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let baseline_view_changes: Vec<u64> = warmup_statuses
            .iter()
            .map(|status| status.view_changes.into())
            .collect();
        let peer_count = network.peers().len();
        let fault_tolerance = peer_count.saturating_sub(1) / 3;
        let max_extra_view_changes = u64::try_from(fault_tolerance.saturating_add(2))
            .unwrap_or(u64::MAX);

        ensure!(!network.peers().is_empty(), "network must have at least one peer");
        for peer in network.peers() {
            let message = format!("localnet bounded block {}", peer.mnemonic());
            peer.client()
                .submit::<InstructionBox>(Log::new(Level::INFO, message).into())
                .wrap_err_with(|| {
                    format!("failed to submit log instruction to {}", peer.mnemonic())
                })?;
        }

        let target_height = baseline_height.saturating_add(1);
        let start = Instant::now();
        wait_for_converged_height(&network, target_height, Duration::from_secs(45)).await?;
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

    let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");
    set_env_var(
        "IROHA_TEST_CLIENT_TTL_MS",
        SOAK_CLIENT_TTL.as_millis().to_string(),
    );

    let builder = NetworkBuilder::new()
        .with_peers(4)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_pipeline_time(SMOKE_PIPELINE_TIME)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(1_u64)),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(SMOKE_BLOCK_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(SMOKE_COMMIT_TIME_MS),
        )))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "permissioned")
                .write(["network", "transaction_gossip_period_ms"], 200_i64)
                .write(["network", "transaction_gossip_public_target_cap"], 3_i64)
                .write(
                    ["network", "transaction_gossip_restricted_target_cap"],
                    3_i64,
                )
                .write(
                    ["network", "transaction_gossip_restricted_fallback"],
                    "public_overlay",
                )
                .write(
                    ["network", "transaction_gossip_restricted_public_payload"],
                    "forward",
                )
                .write(["network", "p2p_post_queue_cap"], 8192_i64)
                .write(["network", "p2p_queue_cap_high"], 16384_i64)
                .write(["network", "p2p_queue_cap_low"], 65536_i64)
                .write(["network", "disconnect_on_post_overflow"], false)
                // Tighten local timeouts to keep proposal/view-change cadence bounded.
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
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    2_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    1_i64,
                );
        });

    let result: Result<()> = async {
        let Some(network) = sandbox::start_network_async_or_skip(
            builder,
            stringify!(permissioned_localnet_reaches_100_blocks),
        )
        .await?
        else {
            return Ok(());
        };

        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        let baseline_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let baseline_height = baseline_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();
        let warmup_height = baseline_height.saturating_add(1);
        for peer in network.peers() {
            let message = format!("localnet warmup block {}", peer.mnemonic());
            peer.client()
                .submit::<InstructionBox>(Log::new(Level::INFO, message).into())
                .wrap_err_with(|| {
                    format!("failed to submit warmup log instruction to {}", peer.mnemonic())
                })?;
        }
        wait_for_converged_height(&network, warmup_height, Duration::from_secs(45)).await?;
        let warmup_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let baseline_height = warmup_statuses
            .iter()
            .map(|status| status.blocks)
            .min()
            .unwrap_or_default();

        let target_blocks = if cfg!(debug_assertions) { 30_u64 } else { 100_u64 };
        let target_height = baseline_height.saturating_add(target_blocks);
        let peers = network.peers();
        ensure!(!peers.is_empty(), "network must have at least one peer");
        let peer_count = peers.len();
        let sequence_key = Name::from_str("tx_sequence").expect("tx_sequence metadata key");
        let debug_multiplier = if cfg!(debug_assertions) { 3 } else { 1 };
        let timeout = scale_duration(
            scale_duration(network.pipeline_time(), target_blocks.saturating_mul(3))
                .saturating_add(Duration::from_secs(30)),
            debug_multiplier,
        );
        let per_block_timeout = scale_duration(
            scale_duration(network.pipeline_time(), 8).saturating_add(Duration::from_secs(4)),
            debug_multiplier,
        );
        let start = Instant::now();
        let mut next_height = baseline_height;
        for idx in 0..target_blocks {
            let peer = &peers[usize::try_from(idx).unwrap_or(0) % peer_count];
            let message = format!("localnet block {idx} via {}", peer.mnemonic());
            let mut metadata = Metadata::default();
            metadata.insert(sequence_key.clone(), idx.saturating_add(1));
            peer.client()
                .submit_with_metadata::<InstructionBox>(
                    Log::new(Level::INFO, message).into(),
                    metadata,
                )
                .wrap_err_with(|| {
                    format!(
                        "failed to submit log instruction {idx} to {}",
                        peer.mnemonic()
                    )
                })?;
            next_height = next_height.saturating_add(1);
            let remaining = timeout.saturating_sub(start.elapsed());
            let block_timeout = if remaining < per_block_timeout {
                remaining
            } else {
                per_block_timeout
            };
            let deadline = Instant::now() + block_timeout;
            let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();
            let mut last_log = Instant::now()
                .checked_sub(STATUS_LOG_INTERVAL)
                .unwrap_or_else(Instant::now);
            loop {
                match collect_statuses(&network, STATUS_POLL_TIMEOUT).await {
                    Ok(statuses) => {
                        let snapshot: Vec<StatusSnapshot> =
                            statuses.iter().map(StatusSnapshot::from_status).collect();
                        if snapshot != last_snapshot || last_log.elapsed() >= STATUS_LOG_INTERVAL {
                            eprintln!(
                                "localnet status snapshot (target_height={next_height}): {snapshot:?}"
                            );
                            last_log = Instant::now();
                        }
                        last_snapshot = snapshot;
                        let max_height = statuses
                            .iter()
                            .map(|status| status.blocks)
                            .max()
                            .unwrap_or_default();
                        if max_height >= next_height {
                            break;
                        }
                        if Instant::now() >= deadline {
                            return Err(eyre!(
                                "height failed to reach {next_height} within {:?}: last_snapshot={last_snapshot:?}",
                                block_timeout
                            ));
                        }
                    }
                    Err(err) => {
                        if Instant::now() >= deadline {
                            return Err(eyre!(
                                "height failed to reach {next_height} within {:?}: last_snapshot={last_snapshot:?}, last_error={err:?}",
                                block_timeout
                            ));
                        }
                    }
                }
                sleep(Duration::from_millis(200)).await;
            }
        }

        let remaining = timeout.saturating_sub(start.elapsed());
        let catch_up_timeout = per_block_timeout.min(remaining);
        if catch_up_timeout > Duration::ZERO {
            let deadline = Instant::now() + catch_up_timeout;
            let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();
            let mut last_log = Instant::now()
                .checked_sub(STATUS_LOG_INTERVAL)
                .unwrap_or_else(Instant::now);
            loop {
                match collect_statuses(&network, STATUS_POLL_TIMEOUT).await {
                    Ok(statuses) => {
                        let snapshot: Vec<StatusSnapshot> =
                            statuses.iter().map(StatusSnapshot::from_status).collect();
                        if snapshot != last_snapshot || last_log.elapsed() >= STATUS_LOG_INTERVAL {
                            eprintln!(
                                "localnet catch-up snapshot (target_height={target_height}): {snapshot:?}"
                            );
                            last_log = Instant::now();
                        }
                        last_snapshot = snapshot;
                        if statuses
                            .iter()
                            .all(|status| status.blocks >= target_height)
                        {
                            break;
                        }
                        if Instant::now() >= deadline {
                            return Err(eyre!(
                                "peers failed to catch up to {target_height} within {:?}: last_snapshot={last_snapshot:?}",
                                catch_up_timeout
                            ));
                        }
                    }
                    Err(err) => {
                        if Instant::now() >= deadline {
                            return Err(eyre!(
                                "peers failed to catch up to {target_height} within {:?}: last_snapshot={last_snapshot:?}, last_error={err:?}",
                                catch_up_timeout
                            ));
                        }
                    }
                }
                sleep(Duration::from_millis(200)).await;
            }
        }

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

    if let Some(previous_ttl) = previous_ttl {
        set_env_var("IROHA_TEST_CLIENT_TTL_MS", previous_ttl);
    } else {
        remove_env_var("IROHA_TEST_CLIENT_TTL_MS");
    }

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
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(SOAK_BLOCK_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(SOAK_COMMIT_TIME_MS),
        )))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "permissioned")
                .write(["network", "transaction_gossip_period_ms"], 200_i64)
                .write(["network", "transaction_gossip_public_target_cap"], 3_i64)
                .write(
                    ["network", "transaction_gossip_restricted_target_cap"],
                    3_i64,
                )
                .write(
                    ["network", "transaction_gossip_restricted_fallback"],
                    "public_overlay",
                )
                .write(
                    ["network", "transaction_gossip_restricted_public_payload"],
                    "forward",
                )
                .write(["network", "p2p_post_queue_cap"], 8192_i64)
                .write(["network", "p2p_queue_cap_high"], 16384_i64)
                .write(["network", "p2p_queue_cap_low"], 65536_i64)
                .write(["network", "disconnect_on_post_overflow"], false)
                // Tighten local timeouts to keep proposal/view-change cadence bounded.
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
                // Give DA quorum extra breathing room under sustained load.
                .write(
                    ["sumeragi", "advanced", "da", "quorum_timeout_multiplier"],
                    7_i64,
                )
                .write(
                    [
                        "sumeragi",
                        "advanced",
                        "da",
                        "availability_timeout_multiplier",
                    ],
                    3_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    10_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    2_i64,
                );
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "long-running 7-peer localnet throughput regression (10k tps target)"]
#[allow(clippy::too_many_lines)]
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
async fn permissioned_localnet_throughput_10k_tps() -> Result<()> {
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;

    let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");
    set_env_var(
        "IROHA_TEST_CLIENT_TTL_MS",
        THROUGHPUT_CLIENT_TTL.as_millis().to_string(),
    );

    let builder = NetworkBuilder::new()
        .with_peers(7)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_pipeline_time(THROUGHPUT_PIPELINE_TIME)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(THROUGHPUT_BLOCK_MAX_TXS)),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(THROUGHPUT_BLOCK_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(THROUGHPUT_COMMIT_TIME_MS),
        )))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "permissioned")
                .write(["sumeragi", "collectors", "k"], 3_i64)
                .write(["sumeragi", "collectors", "redundant_send_r"], 2_i64)
                .write(["network", "transaction_gossip_period_ms"], 200_i64)
                .write(["network", "transaction_gossip_public_target_cap"], 3_i64)
                .write(
                    ["network", "transaction_gossip_restricted_target_cap"],
                    3_i64,
                )
                .write(
                    ["network", "transaction_gossip_restricted_fallback"],
                    "public_overlay",
                )
                .write(
                    ["network", "transaction_gossip_restricted_public_payload"],
                    "forward",
                )
                .write(["network", "p2p_post_queue_cap"], 8192_i64)
                .write(["network", "p2p_queue_cap_high"], 16384_i64)
                .write(["network", "p2p_queue_cap_low"], 65536_i64)
                .write(["network", "disconnect_on_post_overflow"], false)
                // Tighten local timeouts to keep proposal/view-change cadence bounded.
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
                // Give DA quorum extra breathing room under sustained load.
                .write(
                    ["sumeragi", "advanced", "da", "quorum_timeout_multiplier"],
                    7_i64,
                )
                .write(
                    [
                        "sumeragi",
                        "advanced",
                        "da",
                        "availability_timeout_multiplier",
                    ],
                    3_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    5_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    1_i64,
                )
                // Lift Torii limits for sustained local throughput runs.
                .write(
                    ["torii", "preauth_allow_cidrs"],
                    TomlValue::Array(vec![
                        TomlValue::String("127.0.0.0/8".into()),
                        TomlValue::String("::1/128".into()),
                    ]),
                )
                .write(
                    ["torii", "api_allow_cidrs"],
                    TomlValue::Array(vec![
                        TomlValue::String("127.0.0.0/8".into()),
                        TomlValue::String("::1/128".into()),
                    ]),
                )
                .write(["torii", "preauth_rate_per_ip_per_sec"], 1_000_000_i64)
                .write(["torii", "preauth_burst_per_ip"], 2_000_000_i64)
                .write(["torii", "query_rate_per_authority_per_sec"], 0_i64)
                .write(["torii", "query_burst_per_authority"], 0_i64)
                .write(["torii", "tx_rate_per_authority_per_sec"], 0_i64)
                .write(["torii", "tx_burst_per_authority"], 0_i64)
                .write(["torii", "api_high_load_tx_threshold"], 262_144_i64);
        });

    let result: Result<()> = async {
        let Some(network) = sandbox::start_network_async_or_skip(
            builder,
            stringify!(permissioned_localnet_throughput_10k_tps),
        )
        .await?
        else {
            return Ok(());
        };

        let network_dir = network.env_dir().to_path_buf();
        let http = HttpClient::new();
        let mut artifacts = ThroughputArtifacts::default();

        let run_result: Result<()> = async {
        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        let baseline_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let baseline_non_empty = baseline_statuses
            .iter()
            .map(|status| status.blocks_non_empty)
            .min()
            .unwrap_or_default();
        let baseline_approved = baseline_statuses
            .iter()
            .map(|status| status.txs_approved)
            .min()
            .unwrap_or_default();

        let total_blocks_default = THROUGHPUT_WARMUP_BLOCKS.saturating_add(THROUGHPUT_STEADY_BLOCKS);
        let total_blocks =
            env_or_default("IROHA_THROUGHPUT_TARGET_BLOCKS", total_blocks_default).max(1);
        let warmup_blocks = env_or_default(
            "IROHA_THROUGHPUT_WARMUP_BLOCKS",
            THROUGHPUT_WARMUP_BLOCKS,
        )
        .min(total_blocks.saturating_sub(1).max(1));
        let steady_blocks_default = total_blocks.saturating_sub(warmup_blocks).max(1);
        let steady_blocks = env_or_default(
            "IROHA_THROUGHPUT_STEADY_BLOCKS",
            steady_blocks_default,
        )
        .max(1);
        let total_blocks = warmup_blocks.saturating_add(steady_blocks);
        let submit_batch =
            env_or_default("IROHA_THROUGHPUT_SUBMIT_BATCH", THROUGHPUT_SUBMIT_BATCH).max(1);
        let submit_parallelism =
            env_or_default("IROHA_THROUGHPUT_PARALLELISM", THROUGHPUT_SUBMIT_PARALLELISM)
                .max(1)
                .min(submit_batch);
        let submit_parallelism = usize::try_from(submit_parallelism)
            .wrap_err("submit parallelism exceeds host limits")?;
        let queue_soft_limit = env_or_default(
            "IROHA_THROUGHPUT_QUEUE_SOFT_LIMIT",
            THROUGHPUT_QUEUE_SOFT_LIMIT,
        );
        let payload_bytes = env_or_default_usize(
            "IROHA_THROUGHPUT_PAYLOAD_BYTES",
            THROUGHPUT_PAYLOAD_BYTES,
        )
        .max(32);
        let rng_seed = env_or_default("IROHA_THROUGHPUT_RNG_SEED", THROUGHPUT_RNG_SEED);
        let warmup_target_height = baseline_non_empty.saturating_add(warmup_blocks);
        let steady_target_height = warmup_target_height.saturating_add(steady_blocks);
        let warmup_txs = warmup_blocks.saturating_mul(THROUGHPUT_BLOCK_MAX_TXS);
        let steady_txs = steady_blocks.saturating_mul(THROUGHPUT_BLOCK_MAX_TXS);
        let total_txs = warmup_txs.saturating_add(steady_txs);

        artifacts.recipe = Some(ThroughputArtifactRecipe {
            peers: network.peers().len() as u64,
            block_time_ms: THROUGHPUT_BLOCK_TIME_MS,
            commit_time_ms: THROUGHPUT_COMMIT_TIME_MS,
            block_max_txs: THROUGHPUT_BLOCK_MAX_TXS,
            warmup_blocks,
            steady_blocks,
            total_blocks,
            warmup_txs,
            steady_txs,
            total_txs,
            submit_batch,
            submit_parallelism: submit_parallelism as u64,
            queue_soft_limit,
            payload_bytes: payload_bytes as u64,
            rng_seed,
        });

        let slo_p95_ms = env_or_default("IROHA_THROUGHPUT_SLO_P95_MS", THROUGHPUT_SLO_P95_MS);
        let slo_p99_ms = env_or_default("IROHA_THROUGHPUT_SLO_P99_MS", THROUGHPUT_SLO_P99_MS);
        let slo_view_change_rate = env_or_default_f64(
            "IROHA_THROUGHPUT_SLO_VIEW_CHANGE_RATE",
            THROUGHPUT_SLO_VIEW_CHANGE_RATE_MAX,
        );
        let slo_backpressure_rate = env_or_default_f64(
            "IROHA_THROUGHPUT_SLO_BACKPRESSURE_RATE",
            THROUGHPUT_SLO_BACKPRESSURE_RATE_MAX,
        );
        let slo_queue_saturation = env_or_default_f64(
            "IROHA_THROUGHPUT_SLO_QUEUE_SAT_FRAC",
            THROUGHPUT_SLO_QUEUE_SAT_FRAC_MAX,
        );
        artifacts.slo = Some(ThroughputArtifactSlo {
            commit_p95_ms: slo_p95_ms,
            commit_p99_ms: slo_p99_ms,
            view_change_rate_max: slo_view_change_rate,
            backpressure_rate_max: slo_backpressure_rate,
            queue_saturation_max: slo_queue_saturation,
        });

        let submit_clients: Vec<_> =
            network.peers().iter().map(|peer| peer.client()).collect();
        ensure!(
            !submit_clients.is_empty(),
            "network must have at least one peer"
        );
        eprintln!(
            "localnet throughput recipe: peers={}, block_time_ms={}, commit_time_ms={}, block_max_txs={}, warmup_blocks={}, steady_blocks={}, total_blocks={}, payload_bytes={}, submit_batch={}, submit_parallelism={}, queue_soft_limit={}, rng_seed={}, baseline_non_empty={}, baseline_approved={}",
            network.peers().len(),
            THROUGHPUT_BLOCK_TIME_MS,
            THROUGHPUT_COMMIT_TIME_MS,
            THROUGHPUT_BLOCK_MAX_TXS,
            warmup_blocks,
            steady_blocks,
            total_blocks,
            payload_bytes,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            rng_seed,
            baseline_non_empty,
            baseline_approved,
        );

        let warmup_submit_elapsed = submit_logs(
            0,
            warmup_txs,
            &network,
            &submit_clients,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            payload_bytes,
            rng_seed,
        )
        .await?;

        let mut last_progress = Instant::now();
        let mut last_min_non_empty = baseline_non_empty;
        let mut last_log = Instant::now()
            .checked_sub(THROUGHPUT_PROGRESS_LOG_INTERVAL)
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
                if last_log.elapsed() >= THROUGHPUT_PROGRESS_LOG_INTERVAL {
                    eprintln!(
                        "localnet throughput warmup progress (target_non_empty={warmup_target_height}, min_non_empty={min_non_empty}, max_non_empty={max_non_empty}): {last_snapshot:?}"
                    );
                    last_log = Instant::now();
                }
                if statuses
                    .iter()
                    .all(|status| status.blocks_non_empty >= warmup_target_height)
                {
                    break;
                }
            }

            if last_progress.elapsed() >= THROUGHPUT_STALL_THRESHOLD {
                return Err(eyre!(
                    "localnet throughput warmup stalled for {:?} (min_non_empty={last_min_non_empty}, target_non_empty={warmup_target_height}): last_snapshot={last_snapshot:?}",
                    THROUGHPUT_STALL_THRESHOLD
                ));
            }

            sleep(SOAK_STATUS_POLL_INTERVAL).await;
        }

        let warmup_metrics =
            collect_metrics_snapshots(&network, &http, THROUGHPUT_METRICS_TIMEOUT).await?;

        let steady_start_statuses = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        let steady_start_sumeragi = collect_sumeragi_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        let steady_start_approved = steady_start_statuses
            .iter()
            .map(|status| status.txs_approved)
            .min()
            .unwrap_or(baseline_approved);
        let steady_start = Instant::now();

        let steady_submit_elapsed = submit_logs(
            warmup_txs,
            steady_txs,
            &network,
            &submit_clients,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            payload_bytes,
            rng_seed,
        )
        .await?;

        let mut samples: Vec<ThroughputSample> = Vec::new();
        let mut last_progress = Instant::now();
        let mut last_min_non_empty = warmup_target_height;
        let mut last_log = Instant::now()
            .checked_sub(THROUGHPUT_PROGRESS_LOG_INTERVAL)
            .unwrap_or_else(Instant::now);
        let mut last_snapshot: Vec<StatusSnapshot> = Vec::new();

        loop {
            if let Ok(statuses) = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await {
                let sumeragi_statuses =
                    collect_sumeragi_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
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
                let status_snapshots: Vec<StatusSnapshot> = statuses
                    .iter()
                    .map(StatusSnapshot::from_status)
                    .collect();
                let sumeragi_snapshots: Vec<SumeragiStatusSnapshot> = sumeragi_statuses
                    .iter()
                    .map(SumeragiStatusSnapshot::from_status)
                    .collect();
                let timestamp_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                samples.push(ThroughputSample {
                    timestamp_ms: u64::try_from(timestamp_ms).unwrap_or(u64::MAX),
                    statuses: status_snapshots.clone(),
                    sumeragi: sumeragi_snapshots,
                });
                last_snapshot = status_snapshots;
                if last_log.elapsed() >= THROUGHPUT_PROGRESS_LOG_INTERVAL {
                    eprintln!(
                        "localnet throughput steady progress (target_non_empty={steady_target_height}, min_non_empty={min_non_empty}, max_non_empty={max_non_empty}): {last_snapshot:?}"
                    );
                    last_log = Instant::now();
                }
                if statuses
                    .iter()
                    .all(|status| status.blocks_non_empty >= steady_target_height)
                {
                    break;
                }
            }

            if last_progress.elapsed() >= THROUGHPUT_STALL_THRESHOLD {
                return Err(eyre!(
                    "localnet throughput stalled for {:?} (min_non_empty={last_min_non_empty}, target_non_empty={steady_target_height}): last_snapshot={last_snapshot:?}",
                    THROUGHPUT_STALL_THRESHOLD
                ));
            }

            sleep(THROUGHPUT_SAMPLE_INTERVAL).await;
        }

        let steady_elapsed = steady_start.elapsed();

        let after_statuses = collect_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        let after_sumeragi = collect_sumeragi_statuses(&network, SOAK_STATUS_POLL_TIMEOUT).await?;
        let after_metrics =
            collect_metrics_snapshots(&network, &http, THROUGHPUT_METRICS_TIMEOUT).await?;
        ensure!(
            after_statuses
                .iter()
                .all(|status| status.blocks_non_empty >= steady_target_height),
            "not all peers reached target non-empty height {steady_target_height}: {after_statuses:?}"
        );
        let max_commit_time_ms = after_statuses
            .iter()
            .map(|status| status.commit_time_ms)
            .max()
            .unwrap_or_default();
        let max_commit_time_allowed = THROUGHPUT_COMMIT_TIME_MS
            .saturating_mul(THROUGHPUT_COMMIT_TIME_MAX_MULTIPLIER);
        let mut min_commit_time_ms = u64::MAX;
        let mut sum_commit_time_ms = 0_u128;
        for status in &after_statuses {
            let value = status.commit_time_ms;
            min_commit_time_ms = min_commit_time_ms.min(value);
            sum_commit_time_ms = sum_commit_time_ms.saturating_add(u128::from(value));
        }
        let avg_commit_time_ms = if after_statuses.is_empty() {
            0_u64
        } else {
            (sum_commit_time_ms / u128::from(after_statuses.len() as u64)) as u64
        };
        let min_commit_time_ms = if min_commit_time_ms == u64::MAX {
            0_u64
        } else {
            min_commit_time_ms
        };

        let (commit_p95_ms, commit_p99_ms, commit_hist_count) =
            commit_time_quantiles(&warmup_metrics, &after_metrics);
        let commit_p95_ms = commit_p95_ms.unwrap_or_default();
        let commit_p99_ms = commit_p99_ms.unwrap_or_default();

        let committed_approved = after_statuses
            .iter()
            .map(|status| status.txs_approved)
            .min()
            .unwrap_or(steady_start_approved)
            .saturating_sub(steady_start_approved);
        let committed_tps = if steady_elapsed.as_secs_f64() > 0.0 {
            committed_approved as f64 / steady_elapsed.as_secs_f64()
        } else {
            0.0
        };
        let submitted_tps = if steady_submit_elapsed.as_secs_f64() > 0.0 {
            steady_txs as f64 / steady_submit_elapsed.as_secs_f64()
        } else {
            0.0
        };

        let (view_change_avg, view_change_max) = rate_summary(
            steady_start_sumeragi
                .iter()
                .map(|status| status.view_change_install_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            after_sumeragi
                .iter()
                .map(|status| status.view_change_install_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            steady_elapsed,
        );
        let (backpressure_avg, backpressure_max) = rate_summary(
            steady_start_sumeragi
                .iter()
                .map(|status| status.pacemaker_backpressure_deferrals_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            after_sumeragi
                .iter()
                .map(|status| status.pacemaker_backpressure_deferrals_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            steady_elapsed,
        );

        let mut saturated_samples = 0_u64;
        let mut total_samples = 0_u64;
        let mut max_queue_depth = 0_u64;
        for sample in &samples {
            for status in &sample.sumeragi {
                total_samples = total_samples.saturating_add(1);
                if status.tx_queue_saturated {
                    saturated_samples = saturated_samples.saturating_add(1);
                }
                max_queue_depth = max_queue_depth.max(status.tx_queue_depth);
            }
        }
        let queue_saturated_frac = if total_samples > 0 {
            saturated_samples as f64 / total_samples as f64
        } else {
            0.0
        };

        let metrics = ThroughputArtifactMetrics {
            submitted_tps,
            committed_tps,
            commit_p95_ms,
            commit_p99_ms,
            commit_hist_count,
            commit_time_ms_min: min_commit_time_ms,
            commit_time_ms_avg: avg_commit_time_ms,
            commit_time_ms_max: max_commit_time_ms,
            view_change_rate_avg: view_change_avg,
            view_change_rate_max: view_change_max,
            backpressure_rate_avg: backpressure_avg,
            backpressure_rate_max: backpressure_max,
            queue_saturated_frac,
            max_queue_depth,
            steady_elapsed_ms: steady_elapsed.as_millis() as u64,
            warmup_submit_elapsed_ms: warmup_submit_elapsed.as_millis() as u64,
            steady_submit_elapsed_ms: steady_submit_elapsed.as_millis() as u64,
        };
        artifacts.metrics = Some(metrics);
        artifacts.samples = samples;
        artifacts.warmup_metrics = warmup_metrics;
        artifacts.after_metrics = after_metrics;

        eprintln!(
            "localnet throughput metrics: peers={}, warmup_blocks={}, steady_blocks={}, warmup_txs={}, steady_txs={}, submit_batch={}, submit_parallelism={}, queue_soft_limit={}, payload_bytes={}, warmup_submit_elapsed={:?}, steady_submit_elapsed={:?}, steady_elapsed={:?}, submitted_tps={:.2}, committed_tps={:.2}, commit_hist_count={}, commit_time_ms(min/avg/max/p95/p99)={}/{}/{}/{}/{}, view_change_rate(avg/max)={:.4}/{:.4}, backpressure_rate(avg/max)={:.4}/{:.4}, queue_saturated_frac={:.2}, max_queue_depth={}",
            network.peers().len(),
            warmup_blocks,
            steady_blocks,
            warmup_txs,
            steady_txs,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            payload_bytes,
            warmup_submit_elapsed,
            steady_submit_elapsed,
            steady_elapsed,
            submitted_tps,
            committed_tps,
            commit_hist_count,
            min_commit_time_ms,
            avg_commit_time_ms,
            max_commit_time_ms,
            commit_p95_ms,
            commit_p99_ms,
            view_change_avg,
            view_change_max,
            backpressure_avg,
            backpressure_max,
            queue_saturated_frac,
            max_queue_depth,
        );
        ensure!(
            max_commit_time_ms <= max_commit_time_allowed,
            "commit time exceeded target: max_commit_time_ms={max_commit_time_ms}, allowed={max_commit_time_allowed}",
        );

        if commit_hist_count > 0 {
            ensure!(
                commit_p95_ms <= slo_p95_ms,
                "p95 commit time exceeded SLO: p95_ms={commit_p95_ms}, slo_p95_ms={slo_p95_ms}",
            );
            ensure!(
                commit_p99_ms <= slo_p99_ms,
                "p99 commit time exceeded SLO: p99_ms={commit_p99_ms}, slo_p99_ms={slo_p99_ms}",
            );
        }
        if slo_view_change_rate > 0.0 {
            ensure!(
                view_change_max <= slo_view_change_rate,
                "view change rate exceeded SLO: max_rate={view_change_max:.4}, slo_rate={slo_view_change_rate:.4}",
            );
        }
        if slo_backpressure_rate > 0.0 {
            ensure!(
                backpressure_max <= slo_backpressure_rate,
                "backpressure deferral rate exceeded SLO: max_rate={backpressure_max:.4}, slo_rate={slo_backpressure_rate:.4}",
            );
        }
        if slo_queue_saturation > 0.0 {
            ensure!(
                queue_saturated_frac <= slo_queue_saturation,
                "queue saturation exceeded SLO: fraction={queue_saturated_frac:.2}, slo={slo_queue_saturation:.2}",
            );
        }

        Ok(())
    }
    .await;

        if let Err(err) = &run_result {
            artifacts.error = Some(err.to_string());
        }
        if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {
            let root = PathBuf::from(artifact_root);
            let peer_logs: Vec<PeerLogInfo> = network
                .peers()
                .iter()
                .enumerate()
                .map(|(index, peer)| PeerLogInfo {
                    index: index as u64,
                    mnemonic: peer.mnemonic().to_string(),
                    stdout_log: peer
                        .latest_stdout_log_path()
                        .map(|path| path.to_string_lossy().to_string()),
                    stderr_log: peer
                        .latest_stderr_log_path()
                        .map(|path| path.to_string_lossy().to_string()),
                })
                .collect();
            if let Err(err) =
                write_throughput_artifacts(&root, &network_dir, &peer_logs, &artifacts)
            {
                eprintln!("throughput artifact write failed: {err:?}");
            }
        }

        network.shutdown().await;
        run_result
    }
    .await;

    if let Some(previous_ttl) = previous_ttl {
        set_env_var("IROHA_TEST_CLIENT_TTL_MS", previous_ttl);
    } else {
        remove_env_var("IROHA_TEST_CLIENT_TTL_MS");
    }

    if sandbox::handle_result(result, stringify!(permissioned_localnet_throughput_10k_tps))?
        .is_none()
    {
        return Ok(());
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "long-running 7-peer localnet throughput regression (10k tps target, NPoS)"]
#[allow(clippy::too_many_lines)]
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
async fn npos_localnet_throughput_10k_tps() -> Result<()> {
    init_instruction_registry();
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;

    let previous_ttl = std::env::var_os("IROHA_TEST_CLIENT_TTL_MS");
    set_env_var(
        "IROHA_TEST_CLIENT_TTL_MS",
        THROUGHPUT_CLIENT_TTL.as_millis().to_string(),
    );

    let npos_params = SumeragiNposParameters {
        k_aggregators: 3,
        redundant_send_r: 2,
        ..SumeragiNposParameters::default()
    };

    let builder = NetworkBuilder::new()
        .with_peers(7)
        .with_auto_populated_trusted_peers()
        .with_real_genesis_keypair()
        .with_pipeline_time(THROUGHPUT_PIPELINE_TIME)
        .with_genesis_instruction(SetParameter::new(Parameter::Block(
            BlockParameter::MaxTransactions(nonzero!(THROUGHPUT_BLOCK_MAX_TXS)),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::BlockTimeMs(THROUGHPUT_BLOCK_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::CommitTimeMs(THROUGHPUT_COMMIT_TIME_MS),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Sumeragi(
            SumeragiParameter::DaEnabled(true),
        )))
        .with_genesis_instruction(SetParameter::new(Parameter::Custom(
            npos_params.into_custom_parameter(),
        )))
        .with_config_layer(|layer| {
            layer
                .write(["sumeragi", "consensus_mode"], "npos")
                .write(["sumeragi", "collectors", "k"], 3_i64)
                .write(["sumeragi", "collectors", "redundant_send_r"], 2_i64)
                .write(["network", "transaction_gossip_period_ms"], 200_i64)
                .write(["network", "transaction_gossip_public_target_cap"], 3_i64)
                .write(
                    ["network", "transaction_gossip_restricted_target_cap"],
                    3_i64,
                )
                .write(
                    ["network", "transaction_gossip_restricted_fallback"],
                    "public_overlay",
                )
                .write(
                    ["network", "transaction_gossip_restricted_public_payload"],
                    "forward",
                )
                .write(["network", "p2p_post_queue_cap"], 8192_i64)
                .write(["network", "p2p_queue_cap_high"], 16384_i64)
                .write(["network", "p2p_queue_cap_low"], 65536_i64)
                .write(["network", "disconnect_on_post_overflow"], false)
                // Give DA quorum extra breathing room under sustained load.
                .write(
                    ["sumeragi", "advanced", "da", "quorum_timeout_multiplier"],
                    7_i64,
                )
                .write(
                    [
                        "sumeragi",
                        "advanced",
                        "da",
                        "availability_timeout_multiplier",
                    ],
                    3_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "max_backoff_ms"],
                    5_000_i64,
                )
                .write(
                    ["sumeragi", "advanced", "pacemaker", "rtt_floor_multiplier"],
                    1_i64,
                )
                // Lift Torii limits for sustained local throughput runs.
                .write(
                    ["torii", "preauth_allow_cidrs"],
                    TomlValue::Array(vec![
                        TomlValue::String("127.0.0.0/8".into()),
                        TomlValue::String("::1/128".into()),
                    ]),
                )
                .write(
                    ["torii", "api_allow_cidrs"],
                    TomlValue::Array(vec![
                        TomlValue::String("127.0.0.0/8".into()),
                        TomlValue::String("::1/128".into()),
                    ]),
                )
                .write(["torii", "preauth_rate_per_ip_per_sec"], 1_000_000_i64)
                .write(["torii", "preauth_burst_per_ip"], 2_000_000_i64)
                .write(["torii", "query_rate_per_authority_per_sec"], 0_i64)
                .write(["torii", "query_burst_per_authority"], 0_i64)
                .write(["torii", "tx_rate_per_authority_per_sec"], 0_i64)
                .write(["torii", "tx_burst_per_authority"], 0_i64)
                .write(["torii", "api_high_load_tx_threshold"], 262_144_i64);
        });

    let result: Result<()> = async {
        let Some(network) = sandbox::start_network_async_or_skip(
            builder,
            stringify!(npos_localnet_throughput_10k_tps),
        )
        .await?
        else {
            return Ok(());
        };

        let network_dir = network.env_dir().to_path_buf();
        let http = HttpClient::new();
        let mut artifacts = ThroughputArtifacts::default();

        let run_result: Result<()> = async {
        wait_for_status_responses(&network, Duration::from_secs(30)).await?;
        let baseline_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let baseline_non_empty = baseline_statuses
            .iter()
            .map(|status| status.blocks_non_empty)
            .min()
            .unwrap_or_default();
        let baseline_approved = baseline_statuses
            .iter()
            .map(|status| status.txs_approved)
            .min()
            .unwrap_or_default();

        let total_blocks_default =
            THROUGHPUT_WARMUP_BLOCKS.saturating_add(THROUGHPUT_STEADY_BLOCKS);
        let total_blocks =
            env_or_default("IROHA_THROUGHPUT_TARGET_BLOCKS", total_blocks_default).max(1);
        let warmup_blocks = env_or_default(
            "IROHA_THROUGHPUT_WARMUP_BLOCKS",
            THROUGHPUT_WARMUP_BLOCKS,
        )
        .min(total_blocks.saturating_sub(1).max(1));
        let steady_blocks_default = total_blocks.saturating_sub(warmup_blocks).max(1);
        let steady_blocks = env_or_default("IROHA_THROUGHPUT_STEADY_BLOCKS", steady_blocks_default)
            .max(1);
        let total_blocks = warmup_blocks.saturating_add(steady_blocks);
        let submit_batch =
            env_or_default("IROHA_THROUGHPUT_SUBMIT_BATCH", THROUGHPUT_SUBMIT_BATCH).max(1);
        let submit_parallelism =
            env_or_default("IROHA_THROUGHPUT_PARALLELISM", THROUGHPUT_SUBMIT_PARALLELISM)
                .max(1)
                .min(submit_batch);
        let submit_parallelism = usize::try_from(submit_parallelism)
            .wrap_err("submit parallelism exceeds host limits")?;
        let queue_soft_limit = env_or_default(
            "IROHA_THROUGHPUT_QUEUE_SOFT_LIMIT",
            THROUGHPUT_QUEUE_SOFT_LIMIT,
        );
        let payload_bytes = env_or_default_usize(
            "IROHA_THROUGHPUT_PAYLOAD_BYTES",
            THROUGHPUT_PAYLOAD_BYTES,
        )
        .max(32);
        let rng_seed = env_or_default("IROHA_THROUGHPUT_RNG_SEED", THROUGHPUT_RNG_SEED);
        let warmup_target_height = baseline_non_empty.saturating_add(warmup_blocks);
        let steady_target_height = warmup_target_height.saturating_add(steady_blocks);
        let warmup_txs = warmup_blocks.saturating_mul(THROUGHPUT_BLOCK_MAX_TXS);
        let steady_txs = steady_blocks.saturating_mul(THROUGHPUT_BLOCK_MAX_TXS);
        let total_txs = warmup_txs.saturating_add(steady_txs);
        artifacts.recipe = Some(ThroughputArtifactRecipe {
            peers: network.peers().len() as u64,
            block_time_ms: THROUGHPUT_BLOCK_TIME_MS,
            commit_time_ms: THROUGHPUT_COMMIT_TIME_MS,
            block_max_txs: THROUGHPUT_BLOCK_MAX_TXS,
            warmup_blocks,
            steady_blocks,
            total_blocks,
            warmup_txs,
            steady_txs,
            total_txs,
            submit_batch,
            submit_parallelism: submit_parallelism as u64,
            queue_soft_limit,
            payload_bytes: payload_bytes as u64,
            rng_seed,
        });

        let slo_p95_ms =
            env_or_default("IROHA_THROUGHPUT_SLO_P95_MS", THROUGHPUT_NPOS_SLO_P95_MS);
        let slo_p99_ms =
            env_or_default("IROHA_THROUGHPUT_SLO_P99_MS", THROUGHPUT_NPOS_SLO_P99_MS);
        let slo_view_change_rate = env_or_default_f64(
            "IROHA_THROUGHPUT_SLO_VIEW_CHANGE_RATE",
            THROUGHPUT_NPOS_SLO_VIEW_CHANGE_RATE_MAX,
        );
        let slo_backpressure_rate = env_or_default_f64(
            "IROHA_THROUGHPUT_SLO_BACKPRESSURE_RATE",
            THROUGHPUT_NPOS_SLO_BACKPRESSURE_RATE_MAX,
        );
        let slo_queue_saturation = env_or_default_f64(
            "IROHA_THROUGHPUT_SLO_QUEUE_SAT_FRAC",
            THROUGHPUT_NPOS_SLO_QUEUE_SAT_FRAC_MAX,
        );
        artifacts.slo = Some(ThroughputArtifactSlo {
            commit_p95_ms: slo_p95_ms,
            commit_p99_ms: slo_p99_ms,
            view_change_rate_max: slo_view_change_rate,
            backpressure_rate_max: slo_backpressure_rate,
            queue_saturation_max: slo_queue_saturation,
        });

        let submit_clients: Vec<_> =
            network.peers().iter().map(|peer| peer.client()).collect();
        ensure!(
            !submit_clients.is_empty(),
            "network must have at least one peer"
        );
        eprintln!(
            "localnet throughput recipe: peers={}, block_time_ms={}, commit_time_ms={}, block_max_txs={}, warmup_blocks={}, steady_blocks={}, total_blocks={}, payload_bytes={}, submit_batch={}, submit_parallelism={}, queue_soft_limit={}, rng_seed={}, baseline_non_empty={}, baseline_approved={}",
            network.peers().len(),
            THROUGHPUT_BLOCK_TIME_MS,
            THROUGHPUT_COMMIT_TIME_MS,
            THROUGHPUT_BLOCK_MAX_TXS,
            warmup_blocks,
            steady_blocks,
            total_blocks,
            payload_bytes,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            rng_seed,
            baseline_non_empty,
            baseline_approved,
        );

        let warmup_submit_elapsed = submit_logs(
            0,
            warmup_txs,
            &network,
            &submit_clients,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            payload_bytes,
            rng_seed,
        )
        .await?;

        let mut last_progress = Instant::now();
        let mut last_min_non_empty = baseline_non_empty;
        let mut last_log = Instant::now()
            .checked_sub(THROUGHPUT_PROGRESS_LOG_INTERVAL)
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
                last_snapshot = statuses
                    .iter()
                    .map(StatusSnapshot::from_status)
                    .collect();
                if min_non_empty >= warmup_target_height {
                    break;
                }
                if min_non_empty > last_min_non_empty {
                    last_min_non_empty = min_non_empty;
                    last_progress = Instant::now();
                }
                if last_log.elapsed() >= THROUGHPUT_PROGRESS_LOG_INTERVAL {
                    last_log = Instant::now();
                    eprintln!(
                        "localnet throughput warmup progress (target_non_empty={warmup_target_height}, min_non_empty={min_non_empty}, max_non_empty={max_non_empty}): {last_snapshot:?}"
                    );
                }
            }
            if last_progress.elapsed() >= THROUGHPUT_STALL_THRESHOLD {
                bail!(
                    "localnet throughput warmup stalled for {:?} (min_non_empty={last_min_non_empty}, target_non_empty={warmup_target_height}): last_snapshot={last_snapshot:?}",
                    THROUGHPUT_STALL_THRESHOLD
                );
            }
            sleep(THROUGHPUT_SAMPLE_INTERVAL).await;
        }

        let warmup_metrics =
            collect_metrics_snapshots(&network, &http, THROUGHPUT_METRICS_TIMEOUT).await?;
        let steady_start_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let steady_start_sumeragi =
            collect_sumeragi_statuses(&network, STATUS_POLL_TIMEOUT).await?;

        let steady_submit_elapsed = submit_logs(
            warmup_txs,
            steady_txs,
            &network,
            &submit_clients,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            payload_bytes,
            rng_seed,
        )
        .await?;

        let steady_start = Instant::now();
        let mut samples: Vec<ThroughputSample> = Vec::new();
        let mut last_progress = Instant::now();
        let mut last_min_non_empty = warmup_target_height;
        let mut last_log = Instant::now()
            .checked_sub(THROUGHPUT_PROGRESS_LOG_INTERVAL)
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
                last_snapshot = statuses
                    .iter()
                    .map(StatusSnapshot::from_status)
                    .collect();
                if min_non_empty >= steady_target_height {
                    break;
                }
                if min_non_empty > last_min_non_empty {
                    last_min_non_empty = min_non_empty;
                    last_progress = Instant::now();
                }
                if last_log.elapsed() >= THROUGHPUT_PROGRESS_LOG_INTERVAL {
                    last_log = Instant::now();
                    eprintln!(
                        "localnet throughput steady progress (target_non_empty={steady_target_height}, min_non_empty={min_non_empty}, max_non_empty={max_non_empty}): {last_snapshot:?}"
                    );
                }
            }
            if last_progress.elapsed() >= THROUGHPUT_STALL_THRESHOLD {
                bail!(
                    "localnet throughput stalled for {:?} (min_non_empty={last_min_non_empty}, target_non_empty={steady_target_height}): last_snapshot={last_snapshot:?}",
                    THROUGHPUT_STALL_THRESHOLD
                );
            }
            let statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
            let sumeragi = collect_sumeragi_statuses(&network, STATUS_POLL_TIMEOUT).await?;
            let status_snapshots: Vec<StatusSnapshot> =
                statuses.iter().map(StatusSnapshot::from_status).collect();
            let sumeragi_snapshots: Vec<SumeragiStatusSnapshot> = sumeragi
                .iter()
                .map(SumeragiStatusSnapshot::from_status)
                .collect();
            let timestamp_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            samples.push(ThroughputSample {
                timestamp_ms: u64::try_from(timestamp_ms).unwrap_or(u64::MAX),
                statuses: status_snapshots,
                sumeragi: sumeragi_snapshots,
            });
            sleep(THROUGHPUT_SAMPLE_INTERVAL).await;
        }

        let steady_elapsed = steady_start.elapsed();
        let after_statuses = collect_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let after_sumeragi = collect_sumeragi_statuses(&network, STATUS_POLL_TIMEOUT).await?;
        let after_metrics =
            collect_metrics_snapshots(&network, &http, THROUGHPUT_METRICS_TIMEOUT).await?;

        let max_commit_time_ms = after_statuses
            .iter()
            .map(|status| status.commit_time_ms)
            .max()
            .unwrap_or_default();
        let max_commit_time_allowed = THROUGHPUT_COMMIT_TIME_MS
            .saturating_mul(THROUGHPUT_COMMIT_TIME_MAX_MULTIPLIER);
        let mut min_commit_time_ms = u64::MAX;
        let mut sum_commit_time_ms = 0_u128;
        for status in &after_statuses {
            let value = status.commit_time_ms;
            min_commit_time_ms = min_commit_time_ms.min(value);
            sum_commit_time_ms = sum_commit_time_ms.saturating_add(u128::from(value));
        }
        let avg_commit_time_ms = if after_statuses.is_empty() {
            0_u64
        } else {
            (sum_commit_time_ms / u128::from(after_statuses.len() as u64)) as u64
        };
        let min_commit_time_ms = if min_commit_time_ms == u64::MAX {
            0_u64
        } else {
            min_commit_time_ms
        };

        let (commit_p95_ms, commit_p99_ms, commit_hist_count) =
            commit_time_quantiles(&warmup_metrics, &after_metrics);
        let commit_p95_ms = commit_p95_ms.unwrap_or_default();
        let commit_p99_ms = commit_p99_ms.unwrap_or_default();

        let steady_approved = after_statuses
            .iter()
            .map(|status| status.txs_approved)
            .min()
            .unwrap_or_default()
            .saturating_sub(
                steady_start_statuses
                    .iter()
                    .map(|status| status.txs_approved)
                    .min()
                    .unwrap_or_default(),
            );
        let committed_approved = steady_approved.saturating_sub(baseline_approved);
        let committed_tps = if steady_elapsed.as_secs_f64() > 0.0 {
            committed_approved as f64 / steady_elapsed.as_secs_f64()
        } else {
            0.0
        };
        let submitted_tps = if steady_submit_elapsed.as_secs_f64() > 0.0 {
            steady_txs as f64 / steady_submit_elapsed.as_secs_f64()
        } else {
            0.0
        };

        let (view_change_avg, view_change_max) = rate_summary(
            steady_start_sumeragi
                .iter()
                .map(|status| status.view_change_install_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            after_sumeragi
                .iter()
                .map(|status| status.view_change_install_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            steady_elapsed,
        );
        let (backpressure_avg, backpressure_max) = rate_summary(
            steady_start_sumeragi
                .iter()
                .map(|status| status.pacemaker_backpressure_deferrals_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            after_sumeragi
                .iter()
                .map(|status| status.pacemaker_backpressure_deferrals_total)
                .collect::<Vec<u64>>()
                .as_slice(),
            steady_elapsed,
        );

        let mut saturated_samples = 0_u64;
        let mut total_samples = 0_u64;
        let mut max_queue_depth = 0_u64;
        for sample in &samples {
            for status in &sample.sumeragi {
                total_samples = total_samples.saturating_add(1);
                if status.tx_queue_saturated {
                    saturated_samples = saturated_samples.saturating_add(1);
                }
                max_queue_depth = max_queue_depth.max(status.tx_queue_depth);
            }
        }
        let queue_saturated_frac = if total_samples > 0 {
            saturated_samples as f64 / total_samples as f64
        } else {
            0.0
        };

        let metrics = ThroughputArtifactMetrics {
            submitted_tps,
            committed_tps,
            commit_p95_ms,
            commit_p99_ms,
            commit_hist_count,
            commit_time_ms_min: min_commit_time_ms,
            commit_time_ms_avg: avg_commit_time_ms,
            commit_time_ms_max: max_commit_time_ms,
            view_change_rate_avg: view_change_avg,
            view_change_rate_max: view_change_max,
            backpressure_rate_avg: backpressure_avg,
            backpressure_rate_max: backpressure_max,
            queue_saturated_frac,
            max_queue_depth,
            steady_elapsed_ms: steady_elapsed.as_millis() as u64,
            warmup_submit_elapsed_ms: warmup_submit_elapsed.as_millis() as u64,
            steady_submit_elapsed_ms: steady_submit_elapsed.as_millis() as u64,
        };
        artifacts.metrics = Some(metrics);
        artifacts.samples = samples;
        artifacts.warmup_metrics = warmup_metrics;
        artifacts.after_metrics = after_metrics;

        eprintln!(
            "localnet throughput metrics: peers={}, warmup_blocks={}, steady_blocks={}, warmup_txs={}, steady_txs={}, submit_batch={}, submit_parallelism={}, queue_soft_limit={}, payload_bytes={}, warmup_submit_elapsed={:?}, steady_submit_elapsed={:?}, steady_elapsed={:?}, submitted_tps={:.2}, committed_tps={:.2}, commit_hist_count={}, commit_time_ms(min/avg/max/p95/p99)={}/{}/{}/{}/{}, view_change_rate(avg/max)={:.4}/{:.4}, backpressure_rate(avg/max)={:.4}/{:.4}, queue_saturated_frac={:.2}, max_queue_depth={}",
            network.peers().len(),
            warmup_blocks,
            steady_blocks,
            warmup_txs,
            steady_txs,
            submit_batch,
            submit_parallelism,
            queue_soft_limit,
            payload_bytes,
            warmup_submit_elapsed,
            steady_submit_elapsed,
            steady_elapsed,
            submitted_tps,
            committed_tps,
            commit_hist_count,
            min_commit_time_ms,
            avg_commit_time_ms,
            max_commit_time_ms,
            commit_p95_ms,
            commit_p99_ms,
            view_change_avg,
            view_change_max,
            backpressure_avg,
            backpressure_max,
            queue_saturated_frac,
            max_queue_depth,
        );
        ensure!(
            max_commit_time_ms <= max_commit_time_allowed,
            "commit time exceeded target: max_commit_time_ms={max_commit_time_ms}, allowed={max_commit_time_allowed}",
        );

        if commit_hist_count > 0 {
            ensure!(
                commit_p95_ms <= slo_p95_ms,
                "p95 commit time exceeded SLO: p95_ms={commit_p95_ms}, slo_p95_ms={slo_p95_ms}",
            );
            ensure!(
                commit_p99_ms <= slo_p99_ms,
                "p99 commit time exceeded SLO: p99_ms={commit_p99_ms}, slo_p99_ms={slo_p99_ms}",
            );
        }
        if slo_view_change_rate > 0.0 {
            ensure!(
                view_change_max <= slo_view_change_rate,
                "view change rate exceeded SLO: max_rate={view_change_max:.4}, slo_rate={slo_view_change_rate:.4}",
            );
        }
        if slo_backpressure_rate > 0.0 {
            ensure!(
                backpressure_max <= slo_backpressure_rate,
                "backpressure deferral rate exceeded SLO: max_rate={backpressure_max:.4}, slo_rate={slo_backpressure_rate:.4}",
            );
        }
        if slo_queue_saturation > 0.0 {
            ensure!(
                queue_saturated_frac <= slo_queue_saturation,
                "queue saturation exceeded SLO: fraction={queue_saturated_frac:.2}, slo={slo_queue_saturation:.2}",
            );
        }

        Ok(())
    }
    .await;

        if let Err(err) = &run_result {
            artifacts.error = Some(err.to_string());
        }
        if let Some(artifact_root) = std::env::var_os("IROHA_THROUGHPUT_ARTIFACT_DIR") {
            let root = PathBuf::from(artifact_root);
            let peer_logs: Vec<PeerLogInfo> = network
                .peers()
                .iter()
                .enumerate()
                .map(|(index, peer)| PeerLogInfo {
                    index: index as u64,
                    mnemonic: peer.mnemonic().to_string(),
                    stdout_log: peer
                        .latest_stdout_log_path()
                        .map(|path| path.to_string_lossy().to_string()),
                    stderr_log: peer
                        .latest_stderr_log_path()
                        .map(|path| path.to_string_lossy().to_string()),
                })
                .collect();
            if let Err(err) =
                write_throughput_artifacts(&root, &network_dir, &peer_logs, &artifacts)
            {
                eprintln!("throughput artifact write failed: {err:?}");
            }
        }

        network.shutdown().await;
        run_result
    }
    .await;

    if let Some(previous_ttl) = previous_ttl {
        set_env_var("IROHA_TEST_CLIENT_TTL_MS", previous_ttl);
    } else {
        remove_env_var("IROHA_TEST_CLIENT_TTL_MS");
    }

    if sandbox::handle_result(result, stringify!(npos_localnet_throughput_10k_tps))?.is_none() {
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

async fn collect_sumeragi_statuses(
    network: &Network,
    status_timeout: Duration,
) -> Result<Vec<SumeragiStatusWire>> {
    try_join_all(network.peers().iter().map(|peer| async move {
        let client = peer.client();
        let handle = task::spawn_blocking(move || client.get_sumeragi_status_wire());
        if let Ok(joined) = tokio::time::timeout(status_timeout, handle).await {
            joined
                .map_err(|err| {
                    eyre!(
                        "sumeragi status join failed for peer {}: {err:?}",
                        peer.mnemonic()
                    )
                })?
                .map_err(|err| {
                    eprintln!(
                        "sumeragi status request failed for peer {}: {err:?} (best_effort={:?}, stdout={:?})",
                        peer.mnemonic(),
                        peer.best_effort_block_height(),
                        peer.latest_stdout_log_path()
                    );
                    err
                })
                .wrap_err_with(|| format!("sumeragi status request failed for peer {}", peer.mnemonic()))
        } else {
            eprintln!(
                "sumeragi status request timed out for peer {} after {:?} (best_effort={:?}, stdout={:?})",
                peer.mnemonic(),
                status_timeout,
                peer.best_effort_block_height(),
                peer.latest_stdout_log_path()
            );
            Err(eyre!(
                "sumeragi status request timed out after {:?} for peer {}",
                status_timeout,
                peer.mnemonic()
            ))
        }
    }))
    .await
}

async fn collect_metrics_snapshots(
    network: &Network,
    http: &HttpClient,
    timeout: Duration,
) -> Result<Vec<PeerMetricsSnapshot>> {
    try_join_all(network.peers().iter().map(|peer| async move {
        let url = metrics_url(&peer.torii_url());
        let response = http
            .get(url.clone())
            .timeout(timeout)
            .send()
            .await
            .wrap_err_with(|| format!("metrics request failed for peer {}", peer.mnemonic()))?;
        let response = response.error_for_status().wrap_err_with(|| {
            format!(
                "metrics request returned error for peer {}",
                peer.mnemonic()
            )
        })?;
        let payload = response
            .text()
            .await
            .wrap_err_with(|| format!("metrics body decode failed for peer {}", peer.mnemonic()))?;
        let commit_time_hist = parse_prom_histogram(&payload, "commit_time_ms");
        Ok(PeerMetricsSnapshot {
            peer: peer.mnemonic().to_string(),
            payload,
            commit_time_hist,
        })
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
    let progress_timeout = queue_progress_timeout();
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
                    .iter()
                    .map(|status| status.queue_size)
                    .max()
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

        if last_progress.elapsed() >= progress_timeout {
            return Err(eyre!(
                "submit queue did not drain below {max_queue} within {:?}",
                progress_timeout
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

#[tokio::test]
async fn env_or_default_usize_reads_positive_values() {
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    let key = "IROHA_ENV_OR_DEFAULT_USIZE_TEST";
    set_env_var(key, "64");
    assert_eq!(env_or_default_usize(key, 8), 64);
    remove_env_var(key);
}

#[tokio::test]
async fn env_or_default_f64_reads_values() {
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    let key = "IROHA_ENV_OR_DEFAULT_F64_TEST";
    set_env_var(key, "1.25");
    assert!((env_or_default_f64(key, 0.5) - 1.25).abs() < f64::EPSILON);
    set_env_var(key, "-1.0");
    assert!((env_or_default_f64(key, 0.5) - 0.5).abs() < f64::EPSILON);
    remove_env_var(key);
}

#[tokio::test]
async fn queue_progress_timeout_reads_override() {
    let _guard = LOCALNET_SMOKE_GUARD
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    set_env_var(THROUGHPUT_QUEUE_PROGRESS_TIMEOUT_ENV, "12");
    assert_eq!(queue_progress_timeout(), Duration::from_secs(12));
    set_env_var(THROUGHPUT_QUEUE_PROGRESS_TIMEOUT_ENV, "0");
    assert_eq!(queue_progress_timeout(), SOAK_QUEUE_PROGRESS_TIMEOUT);
    remove_env_var(THROUGHPUT_QUEUE_PROGRESS_TIMEOUT_ENV);
}

#[test]
fn write_throughput_artifacts_writes_error_summary() {
    let dir = tempdir().expect("tempdir");
    let artifact_root = dir.path().join("artifacts");
    let network_dir = dir.path().join("network");
    let artifacts = ThroughputArtifacts {
        error: Some("boom".to_string()),
        ..ThroughputArtifacts::default()
    };
    let peer_logs = vec![PeerLogInfo {
        index: 0,
        mnemonic: "peer0".to_string(),
        stdout_log: None,
        stderr_log: None,
    }];

    let run_dir = write_throughput_artifacts(&artifact_root, &network_dir, &peer_logs, &artifacts)
        .expect("write artifacts");
    let summary_path = run_dir.join("summary.json");
    let summary_json = fs::read_to_string(&summary_path).expect("read summary");
    let Value::Object(map) =
        norito::json::from_json::<Value>(&summary_json).expect("parse summary")
    else {
        panic!("expected summary object");
    };
    assert_eq!(map.get("error"), Some(&Value::String("boom".to_string())));
    assert!(run_dir.join("status_samples.json").exists());
}

#[test]
fn throughput_payload_is_deterministic() {
    let payload = throughput_payload(7, 64, 123);
    let payload_repeat = throughput_payload(7, 64, 123);
    assert_eq!(payload, payload_repeat);
    assert_eq!(payload.len(), 64);
    assert!(payload.contains("localnet throughput 7"));
    let different = throughput_payload(8, 64, 123);
    assert_ne!(payload, different);
}

#[test]
fn metrics_url_handles_variants() {
    assert_eq!(
        metrics_url("http://127.0.0.1:8080"),
        "http://127.0.0.1:8080/metrics"
    );
    assert_eq!(
        metrics_url("http://127.0.0.1:8080/"),
        "http://127.0.0.1:8080/metrics"
    );
    assert_eq!(
        metrics_url("http://127.0.0.1:8080/metrics"),
        "http://127.0.0.1:8080/metrics"
    );
}

#[test]
fn parse_prom_histogram_extracts_quantiles() {
    let payload = r#"
# HELP commit_time_ms Average block commit time on this peer
# TYPE commit_time_ms histogram
commit_time_ms_bucket{le="5"} 1
commit_time_ms_bucket{le="10"} 3
commit_time_ms_bucket{le="+Inf"} 4
commit_time_ms_sum 27
commit_time_ms_count 4
"#;
    let hist = parse_prom_histogram(payload, "commit_time_ms");
    assert_eq!(hist.count, 4);
    assert_eq!(hist.buckets.len(), 3);
    let p50 = hist.quantile(0.5).expect("p50");
    assert!((p50 - 7.5).abs() < 0.25);
}

#[test]
fn aggregate_histograms_sums_counts() {
    let h1 = HistogramSnapshot {
        buckets: vec![(1.0, 2), (2.0, 3)],
        sum: 5.0,
        count: 3,
    };
    let h2 = HistogramSnapshot {
        buckets: vec![(1.0, 1), (2.0, 2)],
        sum: 4.0,
        count: 2,
    };
    let merged = aggregate_histograms([&h1, &h2]);
    assert_eq!(merged.count, 5);
    assert!((merged.sum - 9.0).abs() < f64::EPSILON);
    assert_eq!(merged.buckets.len(), 2);
}

#[test]
fn commit_time_quantiles_use_delta_histogram() {
    let warmup = PeerMetricsSnapshot {
        peer: "peer0".to_string(),
        payload: String::new(),
        commit_time_hist: HistogramSnapshot {
            buckets: vec![(5.0, 1), (10.0, 2), (f64::INFINITY, 2)],
            sum: 15.0,
            count: 2,
        },
    };
    let steady = PeerMetricsSnapshot {
        peer: "peer0".to_string(),
        payload: String::new(),
        commit_time_hist: HistogramSnapshot {
            buckets: vec![(5.0, 1), (10.0, 4), (f64::INFINITY, 4)],
            sum: 35.0,
            count: 4,
        },
    };
    let (p95, p99, count) = commit_time_quantiles(&[warmup], &[steady]);
    assert_eq!(count, 2);
    assert_eq!(p95, Some(10));
    assert_eq!(p99, Some(10));
}

#[test]
fn rate_summary_reports_avg_and_max() {
    let (avg, max) = rate_summary(&[0, 10], &[10, 40], Duration::from_secs(10));
    assert!((avg - 2.0).abs() < f64::EPSILON);
    assert!((max - 3.0).abs() < f64::EPSILON);
}

#[test]
fn config_fingerprint_changes_on_update() {
    let dir = tempdir().expect("tempdir");
    let config_path = dir.path().join("config.base.toml");
    fs::write(&config_path, "a = 1").expect("write config");
    let first = config_fingerprint(dir.path())
        .expect("fingerprint")
        .expect("fingerprint value");
    fs::write(&config_path, "a = 2").expect("write config");
    let second = config_fingerprint(dir.path())
        .expect("fingerprint")
        .expect("fingerprint value");
    assert_ne!(first, second);
}

#[test]
fn status_snapshot_value_handles_options() {
    let snapshot = StatusSnapshot {
        blocks: 1,
        queue_size: 2,
        txs_approved: 3,
        txs_rejected: 4,
        view_changes: 5,
        leader_index: None,
        highest_qc_height: Some(9),
        locked_qc_height: None,
        tx_queue_depth: Some(11),
        tx_queue_saturated: Some(true),
        block_created_dropped_by_lock_total: None,
        block_created_hint_mismatch_total: Some(13),
        block_created_proposal_mismatch_total: None,
        commit_signatures_present: Some(15),
        commit_signatures_required: None,
    };
    let value = status_snapshot_value(&snapshot);
    let Value::Object(map) = value else {
        panic!("expected object");
    };
    assert_eq!(map.get("blocks"), Some(&Value::from(1)));
    assert_eq!(map.get("leader_index"), Some(&Value::Null));
    assert_eq!(map.get("highest_qc_height"), Some(&Value::from(9)));
    assert_eq!(map.get("tx_queue_saturated"), Some(&Value::from(true)));
}

#[test]
fn sumeragi_snapshot_value_maps_fields() {
    let snapshot = SumeragiStatusSnapshot {
        view_change_install_total: 1,
        pacemaker_backpressure_deferrals_total: 2,
        tx_queue_depth: 3,
        tx_queue_capacity: 4,
        tx_queue_saturated: true,
        commit_qc_height: 5,
    };
    let value = sumeragi_snapshot_value(&snapshot);
    let Value::Object(map) = value else {
        panic!("expected object");
    };
    assert_eq!(map.get("commit_qc_height"), Some(&Value::from(5)));
    assert_eq!(map.get("tx_queue_saturated"), Some(&Value::from(true)));
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

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Debug)]
struct ThroughputSample {
    timestamp_ms: u64,
    statuses: Vec<StatusSnapshot>,
    sumeragi: Vec<SumeragiStatusSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SumeragiStatusSnapshot {
    view_change_install_total: u64,
    pacemaker_backpressure_deferrals_total: u64,
    tx_queue_depth: u64,
    tx_queue_capacity: u64,
    tx_queue_saturated: bool,
    commit_qc_height: u64,
}

impl SumeragiStatusSnapshot {
    fn from_status(status: &SumeragiStatusWire) -> Self {
        Self {
            view_change_install_total: status.view_change_install_total,
            pacemaker_backpressure_deferrals_total: status.pacemaker_backpressure_deferrals_total,
            tx_queue_depth: status.tx_queue_depth,
            tx_queue_capacity: status.tx_queue_capacity,
            tx_queue_saturated: status.tx_queue_saturated,
            commit_qc_height: status.commit_qc.height,
        }
    }
}

fn status_snapshot_value(snapshot: &StatusSnapshot) -> Value {
    let mut map = Map::new();
    let opt_u64 = |value: Option<u64>| value.map_or(Value::Null, Value::from);
    let opt_bool = |value: Option<bool>| value.map_or(Value::Null, Value::from);

    map.insert("blocks".to_string(), Value::from(snapshot.blocks));
    map.insert("queue_size".to_string(), Value::from(snapshot.queue_size));
    map.insert(
        "txs_approved".to_string(),
        Value::from(snapshot.txs_approved),
    );
    map.insert(
        "txs_rejected".to_string(),
        Value::from(snapshot.txs_rejected),
    );
    map.insert(
        "view_changes".to_string(),
        Value::from(u64::from(snapshot.view_changes)),
    );
    map.insert("leader_index".to_string(), opt_u64(snapshot.leader_index));
    map.insert(
        "highest_qc_height".to_string(),
        opt_u64(snapshot.highest_qc_height),
    );
    map.insert(
        "locked_qc_height".to_string(),
        opt_u64(snapshot.locked_qc_height),
    );
    map.insert(
        "tx_queue_depth".to_string(),
        opt_u64(snapshot.tx_queue_depth),
    );
    map.insert(
        "tx_queue_saturated".to_string(),
        opt_bool(snapshot.tx_queue_saturated),
    );
    map.insert(
        "block_created_dropped_by_lock_total".to_string(),
        opt_u64(snapshot.block_created_dropped_by_lock_total),
    );
    map.insert(
        "block_created_hint_mismatch_total".to_string(),
        opt_u64(snapshot.block_created_hint_mismatch_total),
    );
    map.insert(
        "block_created_proposal_mismatch_total".to_string(),
        opt_u64(snapshot.block_created_proposal_mismatch_total),
    );
    map.insert(
        "commit_signatures_present".to_string(),
        opt_u64(snapshot.commit_signatures_present),
    );
    map.insert(
        "commit_signatures_required".to_string(),
        opt_u64(snapshot.commit_signatures_required),
    );
    Value::Object(map)
}

fn sumeragi_snapshot_value(snapshot: &SumeragiStatusSnapshot) -> Value {
    let mut map = Map::new();
    map.insert(
        "view_change_install_total".to_string(),
        Value::from(snapshot.view_change_install_total),
    );
    map.insert(
        "pacemaker_backpressure_deferrals_total".to_string(),
        Value::from(snapshot.pacemaker_backpressure_deferrals_total),
    );
    map.insert(
        "tx_queue_depth".to_string(),
        Value::from(snapshot.tx_queue_depth),
    );
    map.insert(
        "tx_queue_capacity".to_string(),
        Value::from(snapshot.tx_queue_capacity),
    );
    map.insert(
        "tx_queue_saturated".to_string(),
        Value::from(snapshot.tx_queue_saturated),
    );
    map.insert(
        "commit_qc_height".to_string(),
        Value::from(snapshot.commit_qc_height),
    );
    Value::Object(map)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PeerLogInfo {
    index: u64,
    mnemonic: String,
    stdout_log: Option<String>,
    stderr_log: Option<String>,
}

#[derive(Clone, Debug)]
struct ThroughputArtifactRecipe {
    peers: u64,
    block_time_ms: u64,
    commit_time_ms: u64,
    block_max_txs: u64,
    warmup_blocks: u64,
    steady_blocks: u64,
    total_blocks: u64,
    warmup_txs: u64,
    steady_txs: u64,
    total_txs: u64,
    submit_batch: u64,
    submit_parallelism: u64,
    queue_soft_limit: u64,
    payload_bytes: u64,
    rng_seed: u64,
}

#[derive(Clone, Debug)]
struct ThroughputArtifactSlo {
    commit_p95_ms: u64,
    commit_p99_ms: u64,
    view_change_rate_max: f64,
    backpressure_rate_max: f64,
    queue_saturation_max: f64,
}

#[derive(Clone, Debug)]
struct ThroughputArtifactMetrics {
    submitted_tps: f64,
    committed_tps: f64,
    commit_p95_ms: u64,
    commit_p99_ms: u64,
    commit_hist_count: u64,
    commit_time_ms_min: u64,
    commit_time_ms_avg: u64,
    commit_time_ms_max: u64,
    view_change_rate_avg: f64,
    view_change_rate_max: f64,
    backpressure_rate_avg: f64,
    backpressure_rate_max: f64,
    queue_saturated_frac: f64,
    max_queue_depth: u64,
    steady_elapsed_ms: u64,
    warmup_submit_elapsed_ms: u64,
    steady_submit_elapsed_ms: u64,
}

#[derive(Debug, Default)]
struct ThroughputArtifacts {
    recipe: Option<ThroughputArtifactRecipe>,
    slo: Option<ThroughputArtifactSlo>,
    metrics: Option<ThroughputArtifactMetrics>,
    warmup_metrics: Vec<PeerMetricsSnapshot>,
    after_metrics: Vec<PeerMetricsSnapshot>,
    samples: Vec<ThroughputSample>,
    error: Option<String>,
}

fn write_throughput_artifacts(
    artifact_root: &Path,
    network_dir: &Path,
    peer_logs: &[PeerLogInfo],
    artifacts: &ThroughputArtifacts,
) -> Result<PathBuf> {
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let run_dir = artifact_root.join(format!(
        "throughput-{}",
        u64::try_from(timestamp_ms).unwrap_or(u64::MAX)
    ));
    fs::create_dir_all(&run_dir).wrap_err("create throughput artifact dir")?;

    let metrics_dir = run_dir.join("metrics");
    fs::create_dir_all(&metrics_dir).wrap_err("create metrics dir")?;

    for snapshot in &artifacts.warmup_metrics {
        let path = metrics_dir.join(format!("{}-warmup.prom", snapshot.peer));
        fs::write(&path, &snapshot.payload)
            .wrap_err_with(|| format!("write warmup metrics {}", path.display()))?;
    }
    for snapshot in &artifacts.after_metrics {
        let path = metrics_dir.join(format!("{}-steady.prom", snapshot.peer));
        fs::write(&path, &snapshot.payload)
            .wrap_err_with(|| format!("write steady metrics {}", path.display()))?;
    }

    let status_samples_value = Value::Array(
        artifacts
            .samples
            .iter()
            .map(|sample| {
                let mut map = Map::new();
                map.insert("timestamp_ms".to_string(), Value::from(sample.timestamp_ms));
                map.insert(
                    "status".to_string(),
                    Value::Array(sample.statuses.iter().map(status_snapshot_value).collect()),
                );
                map.insert(
                    "sumeragi".to_string(),
                    Value::Array(
                        sample
                            .sumeragi
                            .iter()
                            .map(sumeragi_snapshot_value)
                            .collect(),
                    ),
                );
                Value::Object(map)
            })
            .collect(),
    );

    let status_path = run_dir.join("status_samples.json");
    let status_json = norito::json::to_json_pretty(&status_samples_value)
        .map_err(|err| eyre!(err.to_string()))?;
    fs::write(&status_path, status_json)
        .wrap_err_with(|| format!("write {}", status_path.display()))?;

    let config_fingerprint = config_fingerprint(network_dir)?;

    let mut summary = Map::new();
    summary.insert(
        "run_id".to_string(),
        Value::String(
            run_dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        ),
    );
    summary.insert(
        "timestamp_ms".to_string(),
        Value::from(u64::try_from(timestamp_ms).unwrap_or(u64::MAX)),
    );
    summary.insert(
        "network_dir".to_string(),
        Value::String(network_dir.to_string_lossy().to_string()),
    );
    summary.insert(
        "config_fingerprint".to_string(),
        config_fingerprint.map_or(Value::Null, Value::String),
    );
    if let Some(error) = artifacts.error.as_ref() {
        summary.insert("error".to_string(), Value::String(error.clone()));
    }

    if let Some(recipe) = artifacts.recipe.as_ref() {
        let mut recipe_map = Map::new();
        recipe_map.insert("peers".to_string(), Value::from(recipe.peers));
        recipe_map.insert(
            "block_time_ms".to_string(),
            Value::from(recipe.block_time_ms),
        );
        recipe_map.insert(
            "commit_time_ms".to_string(),
            Value::from(recipe.commit_time_ms),
        );
        recipe_map.insert(
            "block_max_txs".to_string(),
            Value::from(recipe.block_max_txs),
        );
        recipe_map.insert(
            "warmup_blocks".to_string(),
            Value::from(recipe.warmup_blocks),
        );
        recipe_map.insert(
            "steady_blocks".to_string(),
            Value::from(recipe.steady_blocks),
        );
        recipe_map.insert("total_blocks".to_string(), Value::from(recipe.total_blocks));
        recipe_map.insert("warmup_txs".to_string(), Value::from(recipe.warmup_txs));
        recipe_map.insert("steady_txs".to_string(), Value::from(recipe.steady_txs));
        recipe_map.insert("total_txs".to_string(), Value::from(recipe.total_txs));
        recipe_map.insert("submit_batch".to_string(), Value::from(recipe.submit_batch));
        recipe_map.insert(
            "submit_parallelism".to_string(),
            Value::from(recipe.submit_parallelism),
        );
        recipe_map.insert(
            "queue_soft_limit".to_string(),
            Value::from(recipe.queue_soft_limit),
        );
        recipe_map.insert(
            "payload_bytes".to_string(),
            Value::from(recipe.payload_bytes),
        );
        recipe_map.insert("rng_seed".to_string(), Value::from(recipe.rng_seed));
        summary.insert("recipe".to_string(), Value::Object(recipe_map));
    }

    if let Some(slo) = artifacts.slo.as_ref() {
        let mut slo_map = Map::new();
        slo_map.insert("commit_p95_ms".to_string(), Value::from(slo.commit_p95_ms));
        slo_map.insert("commit_p99_ms".to_string(), Value::from(slo.commit_p99_ms));
        slo_map.insert(
            "view_change_rate_max".to_string(),
            Value::from(slo.view_change_rate_max),
        );
        slo_map.insert(
            "backpressure_rate_max".to_string(),
            Value::from(slo.backpressure_rate_max),
        );
        slo_map.insert(
            "queue_saturation_max".to_string(),
            Value::from(slo.queue_saturation_max),
        );
        summary.insert("slo".to_string(), Value::Object(slo_map));
    }

    if let Some(metrics) = artifacts.metrics.as_ref() {
        let mut metrics_map = Map::new();
        metrics_map.insert(
            "submitted_tps".to_string(),
            Value::from(metrics.submitted_tps),
        );
        metrics_map.insert(
            "committed_tps".to_string(),
            Value::from(metrics.committed_tps),
        );
        metrics_map.insert(
            "commit_p95_ms".to_string(),
            Value::from(metrics.commit_p95_ms),
        );
        metrics_map.insert(
            "commit_p99_ms".to_string(),
            Value::from(metrics.commit_p99_ms),
        );
        metrics_map.insert(
            "commit_hist_count".to_string(),
            Value::from(metrics.commit_hist_count),
        );
        metrics_map.insert(
            "commit_time_ms_min".to_string(),
            Value::from(metrics.commit_time_ms_min),
        );
        metrics_map.insert(
            "commit_time_ms_avg".to_string(),
            Value::from(metrics.commit_time_ms_avg),
        );
        metrics_map.insert(
            "commit_time_ms_max".to_string(),
            Value::from(metrics.commit_time_ms_max),
        );
        metrics_map.insert(
            "view_change_rate_avg".to_string(),
            Value::from(metrics.view_change_rate_avg),
        );
        metrics_map.insert(
            "view_change_rate_max".to_string(),
            Value::from(metrics.view_change_rate_max),
        );
        metrics_map.insert(
            "backpressure_rate_avg".to_string(),
            Value::from(metrics.backpressure_rate_avg),
        );
        metrics_map.insert(
            "backpressure_rate_max".to_string(),
            Value::from(metrics.backpressure_rate_max),
        );
        metrics_map.insert(
            "queue_saturated_frac".to_string(),
            Value::from(metrics.queue_saturated_frac),
        );
        metrics_map.insert(
            "max_queue_depth".to_string(),
            Value::from(metrics.max_queue_depth),
        );
        metrics_map.insert(
            "steady_elapsed_ms".to_string(),
            Value::from(metrics.steady_elapsed_ms),
        );
        metrics_map.insert(
            "warmup_submit_elapsed_ms".to_string(),
            Value::from(metrics.warmup_submit_elapsed_ms),
        );
        metrics_map.insert(
            "steady_submit_elapsed_ms".to_string(),
            Value::from(metrics.steady_submit_elapsed_ms),
        );
        summary.insert("metrics".to_string(), Value::Object(metrics_map));
    }

    let peer_logs_value: Vec<Value> = peer_logs
        .iter()
        .map(|peer| {
            let mut map = Map::new();
            map.insert("index".to_string(), Value::from(peer.index));
            map.insert("mnemonic".to_string(), Value::String(peer.mnemonic.clone()));
            let stdout = peer
                .stdout_log
                .as_ref()
                .map_or(Value::Null, |path| Value::String(path.clone()));
            let stderr = peer
                .stderr_log
                .as_ref()
                .map_or(Value::Null, |path| Value::String(path.clone()));
            map.insert("stdout_log".to_string(), stdout);
            map.insert("stderr_log".to_string(), stderr);
            Value::Object(map)
        })
        .collect();
    summary.insert("peer_logs".to_string(), Value::Array(peer_logs_value));
    summary.insert(
        "status_samples_path".to_string(),
        Value::String(status_path.to_string_lossy().to_string()),
    );
    summary.insert(
        "metrics_dir".to_string(),
        Value::String(metrics_dir.to_string_lossy().to_string()),
    );

    let summary_value = Value::Object(summary);
    let summary_path = run_dir.join("summary.json");
    let summary_json =
        norito::json::to_json_pretty(&summary_value).map_err(|err| eyre!(err.to_string()))?;
    fs::write(&summary_path, summary_json)
        .wrap_err_with(|| format!("write {}", summary_path.display()))?;

    Ok(run_dir)
}

#[derive(Clone, Debug, Default)]
struct HistogramSnapshot {
    buckets: Vec<(f64, u64)>,
    sum: f64,
    count: u64,
}

impl HistogramSnapshot {
    #[allow(clippy::float_cmp)]
    fn saturating_sub(&self, baseline: &Self) -> Self {
        let buckets = self
            .buckets
            .iter()
            .map(|(le, count)| {
                let base = baseline
                    .buckets
                    .iter()
                    .find_map(|(base_le, base_count)| (*base_le == *le).then_some(*base_count))
                    .unwrap_or(0);
                (*le, count.saturating_sub(base))
            })
            .collect();
        let sum = (self.sum - baseline.sum).max(0.0);
        let count = self.count.saturating_sub(baseline.count);
        Self {
            buckets,
            sum,
            count,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn quantile(&self, quantile: f64) -> Option<f64> {
        if !(0.0..=1.0).contains(&quantile) || self.count == 0 {
            return None;
        }
        let mut buckets = self.buckets.clone();
        buckets.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
        let target = (quantile * self.count as f64).ceil();
        let mut prev_count = 0_u64;
        let mut prev_le = 0.0;
        for (le, count) in buckets {
            if (count as f64) >= target {
                if le.is_infinite() {
                    return Some(prev_le);
                }
                let bucket_count = count.saturating_sub(prev_count);
                if bucket_count == 0 {
                    return Some(le);
                }
                let ratio = (target - prev_count as f64) / bucket_count as f64;
                return Some((le - prev_le).mul_add(ratio, prev_le));
            }
            prev_count = count;
            prev_le = le;
        }
        Some(prev_le)
    }
}

#[derive(Clone, Debug)]
struct PeerMetricsSnapshot {
    peer: String,
    payload: String,
    commit_time_hist: HistogramSnapshot,
}

#[allow(clippy::float_cmp, single_use_lifetimes)]
fn aggregate_histograms<'a>(
    histograms: impl IntoIterator<Item = &'a HistogramSnapshot>,
) -> HistogramSnapshot {
    let mut buckets: Vec<(f64, u64)> = Vec::new();
    let mut sum = 0.0;
    let mut count = 0_u64;
    for hist in histograms {
        sum += hist.sum;
        count = count.saturating_add(hist.count);
        for (le, bucket_count) in &hist.buckets {
            if let Some(entry) = buckets.iter_mut().find(|(bound, _)| *bound == *le) {
                entry.1 = entry.1.saturating_add(*bucket_count);
            } else {
                buckets.push((*le, *bucket_count));
            }
        }
    }
    buckets.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
    HistogramSnapshot {
        buckets,
        sum,
        count,
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::float_cmp
)]
fn parse_prom_histogram(payload: &str, metric: &str) -> HistogramSnapshot {
    let mut buckets: Vec<(f64, u64)> = Vec::new();
    let mut sum = 0.0;
    let mut count = 0_u64;
    let bucket_prefix = format!("{metric}_bucket");
    let sum_prefix = format!("{metric}_sum");
    let count_prefix = format!("{metric}_count");

    for line in payload.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let name = match parts.next() {
            Some(name) => name,
            None => continue,
        };
        let value_raw = match parts.next() {
            Some(value) => value,
            None => continue,
        };
        if name.starts_with(&bucket_prefix) {
            let le = name.find("le=\"").and_then(|pos| {
                let rest = &name[pos + 4..];
                rest.find('"').and_then(|end| {
                    let value = &rest[..end];
                    if value == "+Inf" {
                        Some(f64::INFINITY)
                    } else {
                        value.parse::<f64>().ok()
                    }
                })
            });
            let le = match le {
                Some(value) => value,
                None => continue,
            };
            let value = value_raw
                .parse::<u64>()
                .ok()
                .or_else(|| value_raw.parse::<f64>().ok().map(|v| v.round() as u64));
            if let Some(value) = value {
                if let Some(entry) = buckets.iter_mut().find(|(bound, _)| *bound == le) {
                    entry.1 = value;
                } else {
                    buckets.push((le, value));
                }
            }
        } else if name.starts_with(&sum_prefix) {
            if let Ok(value) = value_raw.parse::<f64>() {
                sum = value;
            }
        } else if name.starts_with(&count_prefix) {
            if let Ok(value) = value_raw.parse::<f64>() {
                count = value.round() as u64;
            } else if let Ok(value) = value_raw.parse::<u64>() {
                count = value;
            }
        }
    }

    buckets.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
    HistogramSnapshot {
        buckets,
        sum,
        count,
    }
}

fn metrics_url(torii_url: &str) -> String {
    if torii_url.ends_with("/metrics") {
        torii_url.to_string()
    } else if torii_url.ends_with('/') {
        format!("{torii_url}metrics")
    } else {
        format!("{torii_url}/metrics")
    }
}

fn throughput_payload(index: u64, payload_bytes: usize, seed: u64) -> String {
    let prefix = format!("localnet throughput {index} ");
    let mut payload = String::with_capacity(payload_bytes.max(prefix.len()));
    payload.push_str(&prefix);
    if payload.len() < payload_bytes {
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ index);
        let alphabet = b"abcdefghijklmnopqrstuvwxyz0123456789";
        let remaining = payload_bytes - payload.len();
        for _ in 0..remaining {
            let idx = (rng.next_u32() as usize) % alphabet.len();
            payload.push(alphabet[idx] as char);
        }
    }
    if payload.len() > payload_bytes {
        payload.truncate(payload_bytes);
    }
    payload
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn commit_time_quantiles(
    warmup: &[PeerMetricsSnapshot],
    steady: &[PeerMetricsSnapshot],
) -> (Option<u64>, Option<u64>, u64) {
    if warmup.is_empty() && steady.is_empty() {
        return (None, None, 0);
    }
    let warmup_hist = aggregate_histograms(warmup.iter().map(|s| &s.commit_time_hist));
    let steady_hist = aggregate_histograms(steady.iter().map(|s| &s.commit_time_hist));
    let delta = steady_hist.saturating_sub(&warmup_hist);
    let p95 = delta.quantile(0.95).map(|v| v.round() as u64);
    let p99 = delta.quantile(0.99).map(|v| v.round() as u64);
    (p95, p99, delta.count)
}

#[allow(clippy::cast_precision_loss)]
fn rate_summary(start: &[u64], end: &[u64], elapsed: Duration) -> (f64, f64) {
    let count = start.len().min(end.len());
    let secs = elapsed.as_secs_f64();
    if count == 0 || secs <= 0.0 {
        return (0.0, 0.0);
    }
    let mut sum = 0.0;
    let mut max_rate = 0.0;
    for (start, end) in start.iter().zip(end.iter()).take(count) {
        let delta = end.saturating_sub(*start) as f64 / secs;
        sum += delta;
        if delta > max_rate {
            max_rate = delta;
        }
    }
    (sum / count as f64, max_rate)
}

fn config_fingerprint(root: &Path) -> Result<Option<String>> {
    if !root.exists() {
        return Ok(None);
    }
    let mut paths = Vec::new();
    collect_config_paths(root, &mut paths);
    if paths.is_empty() {
        return Ok(None);
    }
    paths.sort();
    let mut hasher = Blake3Hasher::new();
    for path in paths {
        hasher.update(path.to_string_lossy().as_bytes());
        let contents = fs::read(&path).wrap_err_with(|| format!("read {}", path.display()))?;
        hasher.update(&contents);
    }
    Ok(Some(hasher.finalize().to_hex().to_string()))
}

fn collect_config_paths(root: &Path, output: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_config_paths(&path, output);
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if name.contains("config")
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
        {
            output.push(path);
        }
    }
}
