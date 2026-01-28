//! Orchestration layer that wires configuration, workload generation, and fault injection together.

use std::{
    borrow::Cow,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use color_eyre::{Result, eyre::eyre};
use futures::FutureExt;
use iroha_config::parameters::actual::SumeragiNposTimeouts;
use iroha_data_model::{
    parameter::SumeragiParameter, parameter::system::SumeragiNposParameters, prelude::*,
};
use iroha_genesis::GenesisBlock;
use iroha_test_network::{Network, NetworkBuilder, NetworkPeer};
use rand::{
    RngCore, SeedableRng,
    rngs::StdRng,
    seq::{IndexedRandom, SliceRandom},
};
use tokio::{
    sync::{Notify, OwnedSemaphorePermit, Semaphore},
    task::{JoinHandle, JoinSet, spawn_blocking},
    time::{self, MissedTickBehavior},
};
use toml::Table;
use tracing::{debug, info, warn};

use crate::{
    config::ChaosConfig,
    faults::{
        self, CpuStressConfig, DiskSaturationConfig, FaultConfig, NetworkLatencyConfig,
        NetworkPartitionConfig,
    },
    instructions::{self, PreparedChaos, TransactionPlan, WorkloadEngine},
};

const IZANAMI_BLOCK_PAYLOAD_QUEUE: i64 = 512;
const IZANAMI_RBC_PENDING_TTL_MS: i64 = 300_000;
const IZANAMI_RBC_SESSION_TTL_MS: i64 = 900_000;
const IZANAMI_RBC_PENDING_MAX_CHUNKS: i64 = 512;
const IZANAMI_RBC_PENDING_MAX_BYTES: i64 = 32 * 1024 * 1024;
const IZANAMI_RBC_PENDING_SESSION_LIMIT: i64 = 512;
const IZANAMI_RBC_REBROADCAST_SESSIONS_PER_TICK: i64 = 32;
const IZANAMI_RBC_PAYLOAD_CHUNKS_PER_TICK: i64 = 256;
const IZANAMI_PACEMAKER_PENDING_STALL_GRACE_MS: i64 = 1_000;
const IZANAMI_PACEMAKER_ACTIVE_PENDING_SOFT_LIMIT: i64 = 8;
const IZANAMI_PACEMAKER_RBC_BACKLOG_SESSION_SOFT_LIMIT: i64 = 8;
const IZANAMI_PACEMAKER_RBC_BACKLOG_CHUNK_SOFT_LIMIT: i64 = 128;
const IZANAMI_DA_QUORUM_TIMEOUT_MULTIPLIER: i64 = 2;
const IZANAMI_FUTURE_HEIGHT_WINDOW: i64 = 2;
const IZANAMI_FUTURE_VIEW_WINDOW: i64 = 2;
const IZANAMI_NPOS_BLOCK_TIME_MS: i64 = 1_500;
const IZANAMI_NPOS_COMMIT_TIME_MS: i64 = 2_000;
const IZANAMI_PIPELINE_DYNAMIC_PREPASS: bool = true;
const IZANAMI_PIPELINE_ACCESS_SET_CACHE_ENABLED: bool = true;
const IZANAMI_PIPELINE_PARALLEL_OVERLAY: bool = true;
const IZANAMI_PIPELINE_PARALLEL_APPLY: bool = true;
const IZANAMI_PIPELINE_WORKERS: i64 = 0;
const IZANAMI_PIPELINE_SIGNATURE_BATCH_MAX_ED25519: i64 = 128;
const IZANAMI_PIPELINE_SIGNATURE_BATCH_MAX_SECP256K1: i64 = 128;
const IZANAMI_PIPELINE_SIGNATURE_BATCH_MAX_PQC: i64 = 64;
const IZANAMI_PIPELINE_SIGNATURE_BATCH_MAX_BLS: i64 = 32;
const IZANAMI_PIPELINE_STATELESS_CACHE_CAP: i64 = 16_384;
const IZANAMI_VALIDATION_WORKER_THREADS: i64 = 0;
const IZANAMI_VALIDATION_WORK_QUEUE_CAP: i64 = 0;
const IZANAMI_VALIDATION_RESULT_QUEUE_CAP: i64 = 0;
const IZANAMI_VALIDATION_PENDING_CAP: i64 = 8_192;

#[derive(Clone, Copy, Debug)]
struct NposTiming {
    block_ms: u64,
    propose_ms: u64,
    prevote_ms: u64,
    precommit_ms: u64,
    commit_timeout_ms: u64,
    commit_time_ms: u64,
    da_ms: u64,
    aggregator_ms: u64,
}

fn clamp_nonzero_ms(value: u64) -> u64 {
    value.max(1)
}

fn duration_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis())
        .unwrap_or(u64::MAX)
        .max(1)
}

fn split_pipeline_time(duration: Duration) -> (u64, u64) {
    let total_ms_u128 = duration.as_millis();
    let total_ms = u64::try_from(total_ms_u128).expect("pipeline time fits into u64 milliseconds");
    let mut block_ms = total_ms / 3;
    if block_ms == 0 {
        block_ms = 1;
    }
    if block_ms >= total_ms {
        block_ms = total_ms.saturating_sub(1);
    }
    let mut commit_ms = total_ms.saturating_sub(block_ms);
    if commit_ms == 0 {
        commit_ms = 1;
        if block_ms > 1 {
            block_ms -= 1;
        }
    }
    (block_ms, commit_ms)
}

fn derive_npos_timing(config: &ChaosConfig) -> NposTiming {
    let (block_ms, commit_time_ms) = if let Some(duration) = config.pipeline_time {
        let (block_ms, commit_ms) = split_pipeline_time(duration);
        (block_ms, commit_ms)
    } else {
        let block_ms = u64::try_from(IZANAMI_NPOS_BLOCK_TIME_MS)
            .expect("izanami block time must be non-negative");
        let commit_ms = u64::try_from(IZANAMI_NPOS_COMMIT_TIME_MS)
            .expect("izanami commit time must be non-negative");
        (block_ms, commit_ms)
    };
    let block_ms = clamp_nonzero_ms(block_ms);
    let commit_time_ms = clamp_nonzero_ms(commit_time_ms);
    let timeouts = SumeragiNposTimeouts::from_block_time(Duration::from_millis(block_ms));
    let propose_ms = clamp_nonzero_ms(duration_ms(timeouts.propose));
    let prevote_ms = clamp_nonzero_ms(duration_ms(timeouts.prevote));
    let precommit_ms = clamp_nonzero_ms(duration_ms(timeouts.precommit));
    let commit_timeout_ms = clamp_nonzero_ms(duration_ms(timeouts.commit));
    let da_ms = clamp_nonzero_ms(duration_ms(timeouts.da));
    let aggregator_ms = clamp_nonzero_ms(duration_ms(timeouts.aggregator));
    NposTiming {
        block_ms,
        propose_ms,
        prevote_ms,
        precommit_ms,
        commit_timeout_ms,
        commit_time_ms,
        da_ms,
        aggregator_ms,
    }
}

fn make_network_builder(config: &ChaosConfig, genesis: Vec<Vec<InstructionBox>>) -> NetworkBuilder {
    let mut genesis = genesis;
    let mut builder = NetworkBuilder::new()
        .with_peers(config.peer_count)
        .with_base_seed("izanami-chaos");
    builder = match config.pipeline_time {
        Some(duration) => builder.with_pipeline_time(duration),
        None => builder.with_default_pipeline_time(),
    };
    if let Some(profile) = &config.nexus {
        builder = builder
            .with_data_availability_enabled(profile.da_enabled)
            .with_config_table(profile.config_layer.clone());
    }
    if let Ok(filter) = std::env::var("RUST_LOG") {
        let filter = filter.trim();
        if !filter.is_empty() {
            let filter = filter.to_string();
            // Forward Izanami's RUST_LOG to peer logger filters for targeted debug runs.
            builder = builder.with_config_layer(|layer| {
                layer.write(["logger", "filter"], filter);
            });
        }
    }
    // Inject Izanami timing into on-chain Sumeragi parameters.
    let npos_timing = derive_npos_timing(config);
    if config.nexus.is_some() {
        let mut injected = Vec::new();
        injected.push(InstructionBox::from(SetParameter::new(
            Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(npos_timing.commit_time_ms)),
        )));
        injected.push(InstructionBox::from(SetParameter::new(
            Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(npos_timing.block_ms)),
        )));
        injected.push(InstructionBox::from(SetParameter::new(Parameter::Custom(
            SumeragiNposParameters::default().into_custom_parameter(),
        ))));
        if !injected.is_empty() {
            if let Some(last_tx) = genesis.last_mut() {
                last_tx.extend(injected);
            } else {
                genesis.push(injected);
            }
        }
    }
    // Tune pipeline/validation throughput and raise payload/RBC budgets to keep long Izanami runs stable.
    builder = builder.with_config_layer(|layer| {
        let as_i64 = |value: u64| -> i64 {
            i64::try_from(value).expect("NPoS timing fits into i64 milliseconds")
        };
        layer
            .write(
                ["pipeline", "dynamic_prepass"],
                IZANAMI_PIPELINE_DYNAMIC_PREPASS,
            )
            .write(
                ["pipeline", "access_set_cache_enabled"],
                IZANAMI_PIPELINE_ACCESS_SET_CACHE_ENABLED,
            )
            .write(
                ["pipeline", "parallel_overlay"],
                IZANAMI_PIPELINE_PARALLEL_OVERLAY,
            )
            .write(
                ["pipeline", "parallel_apply"],
                IZANAMI_PIPELINE_PARALLEL_APPLY,
            )
            .write(["pipeline", "workers"], IZANAMI_PIPELINE_WORKERS)
            .write(
                ["pipeline", "signature_batch_max_ed25519"],
                IZANAMI_PIPELINE_SIGNATURE_BATCH_MAX_ED25519,
            )
            .write(
                ["pipeline", "signature_batch_max_secp256k1"],
                IZANAMI_PIPELINE_SIGNATURE_BATCH_MAX_SECP256K1,
            )
            .write(
                ["pipeline", "signature_batch_max_pqc"],
                IZANAMI_PIPELINE_SIGNATURE_BATCH_MAX_PQC,
            )
            .write(
                ["pipeline", "signature_batch_max_bls"],
                IZANAMI_PIPELINE_SIGNATURE_BATCH_MAX_BLS,
            )
            .write(
                ["pipeline", "stateless_cache_cap"],
                IZANAMI_PIPELINE_STATELESS_CACHE_CAP,
            )
            .write(
                ["sumeragi", "queues", "block_payload"],
                IZANAMI_BLOCK_PAYLOAD_QUEUE,
            )
            .write(
                ["sumeragi", "worker", "validation_worker_threads"],
                IZANAMI_VALIDATION_WORKER_THREADS,
            )
            .write(
                ["sumeragi", "worker", "validation_work_queue_cap"],
                IZANAMI_VALIDATION_WORK_QUEUE_CAP,
            )
            .write(
                ["sumeragi", "worker", "validation_result_queue_cap"],
                IZANAMI_VALIDATION_RESULT_QUEUE_CAP,
            )
            .write(
                ["sumeragi", "worker", "validation_pending_cap"],
                IZANAMI_VALIDATION_PENDING_CAP,
            )
            .write(
                ["sumeragi", "pacemaker", "pending_stall_grace_ms"],
                IZANAMI_PACEMAKER_PENDING_STALL_GRACE_MS,
            )
            .write(
                ["sumeragi", "pacemaker", "active_pending_soft_limit"],
                IZANAMI_PACEMAKER_ACTIVE_PENDING_SOFT_LIMIT,
            )
            .write(
                ["sumeragi", "pacemaker", "rbc_backlog_session_soft_limit"],
                IZANAMI_PACEMAKER_RBC_BACKLOG_SESSION_SOFT_LIMIT,
            )
            .write(
                ["sumeragi", "pacemaker", "rbc_backlog_chunk_soft_limit"],
                IZANAMI_PACEMAKER_RBC_BACKLOG_CHUNK_SOFT_LIMIT,
            )
            .write(
                ["sumeragi", "rbc", "pending_max_chunks"],
                IZANAMI_RBC_PENDING_MAX_CHUNKS,
            )
            .write(
                ["sumeragi", "rbc", "pending_max_bytes"],
                IZANAMI_RBC_PENDING_MAX_BYTES,
            )
            .write(
                ["sumeragi", "rbc", "pending_session_limit"],
                IZANAMI_RBC_PENDING_SESSION_LIMIT,
            )
            .write(
                ["sumeragi", "rbc", "pending_ttl_ms"],
                IZANAMI_RBC_PENDING_TTL_MS,
            )
            .write(
                ["sumeragi", "rbc", "session_ttl_ms"],
                IZANAMI_RBC_SESSION_TTL_MS,
            )
            .write(
                ["sumeragi", "rbc", "disk_store_ttl_ms"],
                IZANAMI_RBC_SESSION_TTL_MS,
            )
            .write(
                ["sumeragi", "rbc", "rebroadcast_sessions_per_tick"],
                IZANAMI_RBC_REBROADCAST_SESSIONS_PER_TICK,
            )
            .write(
                ["sumeragi", "rbc", "payload_chunks_per_tick"],
                IZANAMI_RBC_PAYLOAD_CHUNKS_PER_TICK,
            )
            .write(
                ["sumeragi", "da", "quorum_timeout_multiplier"],
                IZANAMI_DA_QUORUM_TIMEOUT_MULTIPLIER,
            )
            .write(
                ["sumeragi", "gating", "future_height_window"],
                IZANAMI_FUTURE_HEIGHT_WINDOW,
            )
            .write(
                ["sumeragi", "gating", "future_view_window"],
                IZANAMI_FUTURE_VIEW_WINDOW,
            )
            .write(
                ["sumeragi", "npos", "block_time_ms"],
                as_i64(npos_timing.block_ms),
            )
            .write(
                ["sumeragi", "advanced", "npos", "timeouts", "propose_ms"],
                as_i64(npos_timing.propose_ms),
            )
            .write(
                ["sumeragi", "advanced", "npos", "timeouts", "prevote_ms"],
                as_i64(npos_timing.prevote_ms),
            )
            .write(
                ["sumeragi", "advanced", "npos", "timeouts", "precommit_ms"],
                as_i64(npos_timing.precommit_ms),
            )
            .write(
                ["sumeragi", "advanced", "npos", "timeouts", "commit_ms"],
                as_i64(npos_timing.commit_timeout_ms),
            )
            .write(
                ["sumeragi", "advanced", "npos", "timeouts", "da_ms"],
                as_i64(npos_timing.da_ms),
            )
            .write(
                ["sumeragi", "advanced", "npos", "timeouts", "aggregator_ms"],
                as_i64(npos_timing.aggregator_ms),
            );
    });

    let genesis_len = genesis.len();
    for (idx, transaction) in genesis.into_iter().enumerate() {
        for isi in transaction {
            builder = builder.with_genesis_instruction(isi);
        }
        if idx + 1 < genesis_len {
            builder = builder.next_genesis_transaction();
        }
    }

    builder
}

/// Deterministically select which peers should receive fault injection tasks.
fn select_fault_targets(peers_len: usize, faulty_peers: usize, rng: &mut StdRng) -> Vec<usize> {
    if peers_len == 0 || faulty_peers == 0 {
        return Vec::new();
    }
    let target_count = faulty_peers.min(peers_len);
    let mut indices: Vec<_> = (0..peers_len).collect();
    indices.shuffle(rng);
    indices.into_iter().take(target_count).collect()
}

#[derive(Clone)]
struct RunControl {
    stop: Arc<AtomicBool>,
    stop_notify: Arc<Notify>,
    deadline: Instant,
}

impl RunControl {
    fn new(deadline: Instant) -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(false)),
            stop_notify: Arc::new(Notify::new()),
            deadline,
        }
    }

    fn deadline(&self) -> Instant {
        self.deadline
    }

    fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        self.stop_notify.notify_waiters();
    }

    fn should_stop(&self) -> bool {
        self.stop.load(Ordering::Relaxed) || Instant::now() >= self.deadline
    }

    fn stop_notifier(&self) -> Arc<Notify> {
        Arc::clone(&self.stop_notify)
    }
}

pub struct IzanamiRunner {
    config: ChaosConfig,
    network: Network,
    peers: Vec<NetworkPeer>,
    workload: Arc<WorkloadEngine>,
    base_domain: DomainId,
}

impl IzanamiRunner {
    pub async fn new(config: ChaosConfig) -> Result<Self> {
        if !config.allow_net {
            return Err(eyre!(
                "allow_net=false: enable networking via --allow-net or persisted configuration"
            ));
        }
        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos {
            state,
            genesis,
            recipes,
        } = instructions::prepare_state(
            account_qty,
            config.nexus.as_ref(),
            config.workload_profile,
        )?;
        let base_domain = state.base_domain().clone();

        let builder = make_network_builder(&config, genesis);

        let network = builder.start().await?;
        let peers = network.peers().clone();
        let workload = Arc::new(WorkloadEngine::new(state, recipes));

        Ok(Self {
            config,
            network,
            peers,
            workload,
            base_domain,
        })
    }

    pub async fn run(self) -> Result<()> {
        let deadline = Instant::now() + self.config.duration;
        let run_control = Arc::new(RunControl::new(deadline));
        let mut rng = self.seeded_rng();
        let config_layers = Arc::new(
            self.network
                .config_layers()
                .map(Cow::into_owned)
                .collect::<Vec<_>>(),
        );
        let genesis = Arc::new(self.network.genesis());
        let metrics = Arc::new(Metrics::default());
        let submission_counter = Arc::new(AtomicU64::new(0));

        let faulty_handles =
            self.spawn_fault_tasks(&config_layers, &genesis, &run_control, &mut rng);
        let load_handles =
            self.spawn_load_supervisors(&metrics, &run_control, &mut rng, &submission_counter);

        let target_result = if let Some(target_blocks) = self.config.target_blocks {
            wait_for_target_blocks(
                &self.peers,
                target_blocks,
                self.config.progress_interval,
                self.config.progress_timeout,
                &run_control,
            )
            .await
        } else {
            Ok(())
        };

        if let Err(err) = target_result {
            run_control.stop();
            for handle in load_handles {
                let _ = handle.await;
            }
            for handle in faulty_handles {
                let _ = handle.await;
            }
            self.network.shutdown().await;
            return Err(err);
        }

        if self.config.target_blocks.is_some() {
            run_control.stop();
        }

        for handle in load_handles {
            let _ = handle.await;
        }
        for handle in faulty_handles {
            let _ = handle.await;
        }

        self.network.shutdown().await;

        let snapshot = metrics.snapshot();
        info!(
            target: "izanami::summary",
            successes = snapshot.successes,
            failures = snapshot.failures,
            expected_failures = snapshot.expected_failures,
            unexpected_successes = snapshot.unexpected_successes,
            "izanami run complete"
        );
        Ok(())
    }

    fn seeded_rng(&self) -> StdRng {
        seeded_rng_from_seed(self.config.seed)
    }

    fn spawn_fault_tasks(
        &self,
        config_layers: &Arc<Vec<Table>>,
        genesis: &Arc<GenesisBlock>,
        run_control: &Arc<RunControl>,
        rng: &mut StdRng,
    ) -> Vec<JoinHandle<()>> {
        let targets = select_fault_targets(self.peers.len(), self.config.faulty_peers, rng);
        if targets.is_empty() {
            return Vec::new();
        }
        let deadline = run_control.deadline();
        let mut handles = Vec::new();
        let toggles = self.config.faults;
        let fault_cfg = FaultConfig {
            interval: self.config.fault_interval.clone(),
            network_latency: toggles
                .network_latency()
                .then_some(NetworkLatencyConfig::default()),
            network_partition: toggles
                .network_partition()
                .then_some(NetworkPartitionConfig::default()),
            cpu_stress: toggles.cpu_stress().then_some(CpuStressConfig::default()),
            disk_saturation: toggles
                .disk_saturation()
                .then_some(DiskSaturationConfig::default()),
        };
        for (offset, idx) in targets.into_iter().enumerate() {
            let peer = self.peers[idx].clone();
            let config_layers = Arc::clone(config_layers);
            let genesis = Arc::clone(genesis);
            let base_domain = self.base_domain.clone();
            let stop = Arc::clone(&run_control.stop);
            let stop_notify = run_control.stop_notifier();
            let cfg = fault_cfg.clone();
            let seed = rng.next_u64();
            handles.push(tokio::spawn(async move {
                faults::run_fault_loop(
                    peer,
                    cfg,
                    genesis,
                    config_layers,
                    base_domain,
                    stop,
                    stop_notify,
                    deadline,
                    seed,
                )
                .await;
            }));
            debug!(target: "izanami::faults", peer_index = idx, worker = offset, "spawned fault worker");
        }
        handles
    }

    fn spawn_load_supervisors(
        &self,
        metrics: &Arc<Metrics>,
        run_control: &Arc<RunControl>,
        rng: &mut StdRng,
        submission_counter: &Arc<AtomicU64>,
    ) -> Vec<JoinHandle<()>> {
        let peers = self.peers.clone();
        let workload = Arc::clone(&self.workload);
        let semaphore = Arc::new(Semaphore::new(self.config.max_inflight));
        let mut load_rng = rng.clone();
        let interval = Duration::from_secs_f64(1.0 / self.config.tps);
        let metrics = Arc::clone(metrics);
        let run_control = Arc::clone(run_control);
        let stop_notify = run_control.stop_notifier();
        let deadline = run_control.deadline();
        let submission_counter = Arc::clone(submission_counter);
        let handle = tokio::spawn(async move {
            let mut ticker = time::interval(interval);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            let mut submissions = JoinSet::new();
            while !run_control.should_stop() {
                tokio::select! {
                    _ = ticker.tick() => {},
                    () = stop_notify.notified() => break,
                    () = time::sleep_until(deadline.into()) => break,
                }
                drain_ready_submissions(&mut submissions);
                if run_control.should_stop() {
                    break;
                }
                let permit = match semaphore.clone().acquire_owned().await {
                    Ok(permit) => permit,
                    Err(_) => break,
                };
                if run_control.should_stop() {
                    drop(permit);
                    break;
                }
                let plan = match workload.next_plan(&mut load_rng).await {
                    Ok(plan) => plan,
                    Err(err) => {
                        warn!(target: "izanami::workload", ?err, "failed to build transaction plan");
                        drop(permit);
                        continue;
                    }
                };
                let Some(peer) = peers.choose(&mut load_rng) else {
                    drop(permit);
                    break;
                };
                let peer = peer.clone();
                let metrics = Arc::clone(&metrics);
                let submission_counter = Arc::clone(&submission_counter);
                submissions.spawn(async move {
                    submit_plan(&peer, &plan, permit, &metrics, &submission_counter).await;
                });
            }
            while let Some(result) = submissions.join_next().await {
                let _ = result;
            }
        });
        vec![handle]
    }
}

fn drain_ready_submissions(submissions: &mut JoinSet<()>) {
    while let Some(joined) = submissions.join_next().now_or_never() {
        if let Some(result) = joined {
            let _ = result;
        } else {
            break;
        }
    }
}

fn seeded_rng_from_seed(seed: Option<u64>) -> StdRng {
    seed.map_or_else(
        || {
            let mut thread_rng = rand::rng();
            StdRng::from_rng(&mut thread_rng)
        },
        StdRng::seed_from_u64,
    )
}

fn min_peer_height(peers: &[NetworkPeer]) -> u64 {
    peers
        .iter()
        .map(|peer| {
            peer.best_effort_block_height()
                .map(|height| height.total)
                .unwrap_or(0)
        })
        .min()
        .unwrap_or(0)
}

struct ProgressState {
    last_height: u64,
    last_progress_at: Instant,
}

impl ProgressState {
    fn new(now: Instant) -> Self {
        Self {
            last_height: 0,
            last_progress_at: now,
        }
    }

    fn update(&mut self, now: Instant, height: u64) -> bool {
        if height > self.last_height {
            self.last_height = height;
            self.last_progress_at = now;
            true
        } else {
            false
        }
    }

    fn stalled(&self, now: Instant, timeout: Duration) -> bool {
        now.duration_since(self.last_progress_at) >= timeout
    }
}

async fn wait_for_target_blocks(
    peers: &[NetworkPeer],
    target_blocks: u64,
    progress_interval: Duration,
    progress_timeout: Duration,
    run_control: &RunControl,
) -> Result<()> {
    let start = Instant::now();
    let mut progress = ProgressState::new(start);
    loop {
        if run_control.should_stop() {
            return Err(eyre!("izanami run stopped before target blocks reached"));
        }
        let now = Instant::now();
        if now >= run_control.deadline() {
            return Err(eyre!(
                "timed out before reaching target blocks (min height {}, target {})",
                progress.last_height,
                target_blocks
            ));
        }
        let min_height = min_peer_height(peers);
        if min_height >= target_blocks {
            info!(
                target: "izanami::progress",
                min_height,
                target_blocks,
                elapsed = ?now.duration_since(start),
                "target block height reached"
            );
            return Ok(());
        }
        if progress.update(now, min_height) {
            info!(
                target: "izanami::progress",
                min_height,
                target_blocks,
                "block height advanced"
            );
        } else if progress.stalled(now, progress_timeout) {
            return Err(eyre!(
                "no block height progress for {:?} (min height {}, target {})",
                progress_timeout,
                min_height,
                target_blocks
            ));
        }
        let remaining = run_control
            .deadline()
            .checked_duration_since(Instant::now())
            .unwrap_or_default();
        if remaining.is_zero() {
            return Err(eyre!(
                "timed out before reaching target blocks (min height {}, target {})",
                min_height,
                target_blocks
            ));
        }
        time::sleep(progress_interval.min(remaining)).await;
    }
}

static SUBMISSION_METADATA_KEY: OnceLock<Name> = OnceLock::new();

fn submission_metadata(counter: &AtomicU64) -> Metadata {
    let key = SUBMISSION_METADATA_KEY
        .get_or_init(|| "izanami_submission_id".parse().expect("valid metadata key"));
    let mut metadata = Metadata::default();
    metadata.insert(key.clone(), counter.fetch_add(1, Ordering::Relaxed));
    metadata
}

async fn submit_plan(
    peer: &NetworkPeer,
    plan: &TransactionPlan,
    _permit: OwnedSemaphorePermit,
    metrics: &Arc<Metrics>,
    submission_counter: &Arc<AtomicU64>,
) {
    let peer = peer.clone();
    let signer = plan.signer.clone();
    let instructions = plan.instructions.clone();
    let plan_label = plan.label;
    let expect_success = plan.expect_success;
    let metrics = Arc::clone(metrics);
    let submission_counter = Arc::clone(submission_counter);

    run_submission(plan_label, expect_success, metrics, move || {
        let client = peer.client_for(&signer.id, signer.key_pair.private_key().clone());
        let metadata = submission_metadata(&submission_counter);
        client
            .submit_all_blocking_with_metadata(instructions, metadata)
            .map(|_| ())
    })
    .await;
}

async fn run_submission<F>(
    plan_label: &'static str,
    expect_success: bool,
    metrics: Arc<Metrics>,
    blocking: F,
) where
    F: FnOnce() -> Result<()> + Send + 'static,
{
    let result = match spawn_blocking(blocking).await {
        Ok(result) => result,
        Err(err) => Err(err.into()),
    };
    let succeeded = result.is_ok();
    debug!(
        target: "izanami::workload",
        plan = plan_label,
        expect_success,
        succeeded,
        "submitted chaos transaction plan"
    );
    match (&result, expect_success) {
        (Ok(()), true) => metrics.record_success(),
        (Ok(()), false) => metrics.record_unexpected_success(),
        (Err(_), true) => metrics.record_failure(),
        (Err(_), false) => metrics.record_expected_failure(),
    }
    if let Err(err) = result {
        warn!(
            target: "izanami::workload",
            ?err,
            plan = plan_label,
            "plan submission failed"
        );
    }
}

#[derive(Default)]
struct Metrics {
    successes: AtomicU64,
    failures: AtomicU64,
    expected_failures: AtomicU64,
    unexpected_successes: AtomicU64,
}

impl Metrics {
    fn record_success(&self) {
        self.successes.fetch_add(1, Ordering::Relaxed);
    }

    fn record_failure(&self) {
        self.failures.fetch_add(1, Ordering::Relaxed);
    }

    fn record_expected_failure(&self) {
        self.expected_failures.fetch_add(1, Ordering::Relaxed);
    }

    fn record_unexpected_success(&self) {
        self.unexpected_successes.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            successes: self.successes.load(Ordering::Relaxed),
            failures: self.failures.load(Ordering::Relaxed),
            expected_failures: self.expected_failures.load(Ordering::Relaxed),
            unexpected_successes: self.unexpected_successes.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Copy)]
struct MetricsSnapshot {
    successes: u64,
    failures: u64,
    expected_failures: u64,
    unexpected_successes: u64,
}

#[cfg(test)]
mod tests {
    use std::{env, io};

    use color_eyre::eyre::{WrapErr, eyre};
    use iroha_data_model::{
        isi::SetParameter,
        parameter::{Parameter, SumeragiParameter},
    };
    use iroha_test_network::init_instruction_registry;
    use tokio::time::timeout;

    use super::*;
    use crate::config::{
        DEFAULT_PROGRESS_INTERVAL, DEFAULT_PROGRESS_TIMEOUT, FaultToggles, NexusProfile,
        WorkloadProfile,
    };

    fn allow_net_for_tests() -> bool {
        std::env::var("IZANAMI_ALLOW_NET")
            .or_else(|_| std::env::var("IROHA_ALLOW_NET"))
            .ok()
            .map(|val| {
                matches!(
                    val.to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on" | "y"
                )
            })
            .unwrap_or(false)
    }

    struct EnvGuard {
        key: &'static str,
        original: Option<String>,
    }

    impl EnvGuard {
        #[allow(unsafe_code)]
        fn set(key: &'static str, value: &str) -> Self {
            let original = env::var(key).ok();
            // Safety: test-only environment changes are scoped to the guard.
            unsafe {
                env::set_var(key, value);
            }
            Self { key, original }
        }
    }

    impl Drop for EnvGuard {
        #[allow(unsafe_code)]
        fn drop(&mut self) {
            if let Some(value) = &self.original {
                // Safety: test-only environment changes are scoped to the guard.
                unsafe {
                    env::set_var(self.key, value);
                }
            } else {
                // Safety: test-only environment changes are scoped to the guard.
                unsafe {
                    env::remove_var(self.key);
                }
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn npos_network_progresses() -> Result<()> {
        if !allow_net_for_tests() {
            // Restricted sandboxes may forbid binding loopback ports; treat this as a skipped check.
            return Ok(());
        }
        crate::config::init_tracing_with_filter("warn");
        init_instruction_registry();

        let config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(2),
            pipeline_time: None,
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            seed: Some(42),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            fault_interval: Duration::from_secs(5)..=Duration::from_secs(5),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: None,
        };

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos {
            state: _,
            genesis,
            recipes: _,
        } = instructions::prepare_state(account_qty, None, config.workload_profile)?;
        let mut builder = make_network_builder(&config, genesis);
        builder = builder.with_config_layer(|layer| {
            layer.write(["sumeragi", "consensus_mode"], "npos");
        });

        let network = match builder.start().await {
            Ok(network) => network,
            Err(err) => {
                let looks_like_permission_denied = err
                    .downcast_ref::<io::Error>()
                    .is_some_and(|io_err| io_err.kind() == io::ErrorKind::PermissionDenied)
                    || err.to_string().contains("Operation not permitted");
                if looks_like_permission_denied {
                    // CI sandboxes (or restricted environments) may block binding loopback ports.
                    // Treat this as a skipped test rather than a hard failure so other coverage runs.
                    return Ok(());
                }
                return Err(err);
            }
        };
        network
            .ensure_blocks_with(|height| height.total >= 4)
            .await
            .wrap_err("NPoS network failed to reach expected height")?;
        network.shutdown().await;

        Ok(())
    }

    #[test]
    fn npos_genesis_sets_sumeragi_timing() -> Result<()> {
        init_instruction_registry();
        let profile = NexusProfile::sora_defaults()?;
        let config = ChaosConfig {
            allow_net: false,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(1),
            pipeline_time: None,
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            seed: Some(7),
            tps: 1.0,
            max_inflight: 1,
            workload_profile: WorkloadProfile::Stable,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: Some(profile),
        };

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos {
            state: _,
            genesis,
            recipes: _,
        } = instructions::prepare_state(
            account_qty,
            config.nexus.as_ref(),
            config.workload_profile,
        )?;

        let network = make_network_builder(&config, genesis).build();
        let timing = derive_npos_timing(&config);
        let mut block_time = None;
        let mut commit_time = None;
        for isi in network.genesis_isi().iter().flatten() {
            if let Some(set_param) = isi.as_any().downcast_ref::<SetParameter>() {
                match set_param.inner() {
                    Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(ms)) => {
                        block_time = Some(*ms);
                    }
                    Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(ms)) => {
                        commit_time = Some(*ms);
                    }
                    _ => {}
                }
            }
        }

        assert_eq!(
            block_time,
            Some(timing.block_ms),
            "genesis should set sumeragi block_time_ms for NPoS"
        );
        assert_eq!(
            commit_time,
            Some(timing.commit_time_ms),
            "genesis should set sumeragi commit_time_ms for NPoS"
        );
        Ok(())
    }

    #[test]
    fn npos_genesis_sets_commit_time_before_block_time() -> Result<()> {
        init_instruction_registry();
        let profile = NexusProfile::sora_defaults()?;
        let config = ChaosConfig {
            allow_net: false,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(1),
            pipeline_time: None,
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            seed: Some(7),
            tps: 1.0,
            max_inflight: 1,
            workload_profile: WorkloadProfile::Stable,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: Some(profile),
        };

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos {
            state: _,
            genesis,
            recipes: _,
        } = instructions::prepare_state(
            account_qty,
            config.nexus.as_ref(),
            config.workload_profile,
        )?;

        let network = make_network_builder(&config, genesis).build();
        let mut commit_pos = None;
        let mut block_pos = None;
        let mut idx = 0usize;
        for isi in network.genesis_isi().iter().flatten() {
            if let Some(set_param) = isi.as_any().downcast_ref::<SetParameter>() {
                match set_param.inner() {
                    Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(_)) => {
                        if commit_pos.is_none() {
                            commit_pos = Some(idx);
                        }
                    }
                    Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(_)) => {
                        if block_pos.is_none() {
                            block_pos = Some(idx);
                        }
                    }
                    _ => {}
                }
            }
            idx = idx.saturating_add(1);
        }

        let commit_pos = commit_pos.expect("commit_time_ms should be injected");
        let block_pos = block_pos.expect("block_time_ms should be injected");
        assert!(
            commit_pos < block_pos,
            "commit_time_ms must be set before block_time_ms to satisfy validation"
        );
        Ok(())
    }

    #[test]
    fn derive_npos_timing_uses_block_time_for_timeouts() {
        let config = ChaosConfig {
            allow_net: false,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(1),
            pipeline_time: Some(Duration::from_millis(3_000)),
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            seed: Some(7),
            tps: 1.0,
            max_inflight: 1,
            workload_profile: WorkloadProfile::Stable,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: None,
        };

        let timing = derive_npos_timing(&config);
        let expected =
            SumeragiNposTimeouts::from_block_time(Duration::from_millis(timing.block_ms));
        assert_eq!(timing.commit_timeout_ms, duration_ms(expected.commit));
        assert_ne!(timing.commit_timeout_ms, timing.commit_time_ms);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nexus_status_reports_teu_metrics() -> Result<()> {
        if !allow_net_for_tests() {
            return Ok(());
        }
        crate::config::init_tracing_with_filter("warn");
        init_instruction_registry();

        let nexus = NexusProfile::sora_defaults().expect("nexus profile");
        let config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(2),
            pipeline_time: None,
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            seed: Some(7),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            fault_interval: Duration::from_secs(5)..=Duration::from_secs(5),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: Some(nexus.clone()),
        };

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos {
            state: _, genesis, ..
        } = instructions::prepare_state(
            account_qty,
            config.nexus.as_ref(),
            config.workload_profile,
        )?;
        let builder = make_network_builder(&config, genesis);

        let network = match builder.start().await {
            Ok(network) => network,
            Err(err) => {
                let looks_like_permission_denied = err
                    .downcast_ref::<io::Error>()
                    .is_some_and(|io_err| io_err.kind() == io::ErrorKind::PermissionDenied)
                    || err.to_string().contains("Operation not permitted");
                if looks_like_permission_denied {
                    return Ok(());
                }
                return Err(err);
            }
        };

        let status = match network.peer().status().await {
            Ok(status) => status,
            Err(err) => {
                let looks_like_permission_denied = err
                    .downcast_ref::<io::Error>()
                    .is_some_and(|io_err| io_err.kind() == io::ErrorKind::PermissionDenied)
                    || err.to_string().contains("Operation not permitted");
                if looks_like_permission_denied {
                    return Ok(());
                }
                return Err(err);
            }
        };

        let expected_lanes = nexus.lane_catalog.lane_count().get() as usize;
        assert!(
            status.teu_lane_commit.len() >= expected_lanes,
            "expected at least {expected_lanes} lane TEU snapshots"
        );
        assert!(
            status.teu_dataspace_backlog.len() >= nexus.dataspace_catalog.entries().len(),
            "dataspace backlog telemetry should be populated"
        );

        network.shutdown().await;
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_submission_records_success() {
        let metrics = Arc::new(Metrics::default());
        run_submission("success", true, Arc::clone(&metrics), || Ok(())).await;

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.successes, 1);
        assert_eq!(snapshot.failures, 0);
        assert_eq!(snapshot.expected_failures, 0);
        assert_eq!(snapshot.unexpected_successes, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_submission_records_failure() {
        let metrics = Arc::new(Metrics::default());
        run_submission("failure", true, Arc::clone(&metrics), || {
            Err(eyre!("submission failed"))
        })
        .await;

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.successes, 0);
        assert_eq!(snapshot.failures, 1);
        assert_eq!(snapshot.expected_failures, 0);
        assert_eq!(snapshot.unexpected_successes, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_submission_records_expected_failure() {
        let metrics = Arc::new(Metrics::default());
        run_submission("expected_failure", false, Arc::clone(&metrics), || {
            Err(eyre!("submission failed"))
        })
        .await;

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.successes, 0);
        assert_eq!(snapshot.failures, 0);
        assert_eq!(snapshot.expected_failures, 1);
        assert_eq!(snapshot.unexpected_successes, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_submission_records_unexpected_success() {
        let metrics = Arc::new(Metrics::default());
        run_submission("unexpected_success", false, Arc::clone(&metrics), || Ok(())).await;

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.successes, 0);
        assert_eq!(snapshot.failures, 0);
        assert_eq!(snapshot.expected_failures, 0);
        assert_eq!(snapshot.unexpected_successes, 1);
    }

    #[test]
    fn seeded_rng_is_deterministic_for_same_seed() {
        let mut rng_a = seeded_rng_from_seed(Some(777));
        let mut rng_b = seeded_rng_from_seed(Some(777));

        let sample_a: [u64; 3] = [rng_a.next_u64(), rng_a.next_u64(), rng_a.next_u64()];
        let sample_b: [u64; 3] = [rng_b.next_u64(), rng_b.next_u64(), rng_b.next_u64()];

        assert_eq!(
            sample_a, sample_b,
            "identical seeds must yield same sequence"
        );
    }

    #[test]
    fn seeded_rng_diverges_for_different_seeds() {
        let mut rng_a = seeded_rng_from_seed(Some(1));
        let mut rng_b = seeded_rng_from_seed(Some(2));

        let sample_a: [u64; 3] = [rng_a.next_u64(), rng_a.next_u64(), rng_a.next_u64()];
        let sample_b: [u64; 3] = [rng_b.next_u64(), rng_b.next_u64(), rng_b.next_u64()];

        assert_ne!(
            sample_a, sample_b,
            "different seeds should produce different sequences"
        );
    }

    #[test]
    fn progress_state_tracks_stalls() {
        let start = Instant::now();
        let mut state = ProgressState::new(start);
        assert!(!state.stalled(start, Duration::from_secs(5)));
        assert!(!state.update(start, 0));
        assert!(!state.stalled(start + Duration::from_secs(2), Duration::from_secs(5)));
        assert!(state.update(start + Duration::from_secs(3), 2));
        assert!(!state.stalled(start + Duration::from_secs(6), Duration::from_secs(5)));
        assert!(state.stalled(start + Duration::from_secs(9), Duration::from_secs(5)));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn drain_ready_submissions_clears_completed_tasks() {
        let mut set = JoinSet::new();
        set.spawn(async {});
        set.spawn(async {});

        tokio::task::yield_now().await;
        assert_eq!(set.len(), 2);

        drain_ready_submissions(&mut set);
        assert_eq!(set.len(), 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_for_target_blocks_reaches_target() -> Result<()> {
        if !allow_net_for_tests() {
            return Ok(());
        }
        crate::config::init_tracing_with_filter("warn");
        init_instruction_registry();

        let config = ChaosConfig {
            allow_net: true,
            peer_count: 2,
            faulty_peers: 0,
            duration: Duration::from_secs(4),
            pipeline_time: Some(Duration::from_millis(250)),
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            seed: Some(9),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            fault_interval: Duration::from_secs(5)..=Duration::from_secs(5),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([false, false, false, false]),
            nexus: None,
        };

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos {
            state: _,
            genesis,
            recipes: _,
        } = instructions::prepare_state(account_qty, None, config.workload_profile)?;
        let builder = make_network_builder(&config, genesis);

        let network = match builder.start().await {
            Ok(network) => network,
            Err(err) => {
                let looks_like_permission_denied = err
                    .downcast_ref::<io::Error>()
                    .is_some_and(|io_err| io_err.kind() == io::ErrorKind::PermissionDenied)
                    || err.to_string().contains("Operation not permitted");
                if looks_like_permission_denied {
                    return Ok(());
                }
                return Err(err);
            }
        };

        let run_control = RunControl::new(Instant::now() + Duration::from_secs(20));
        wait_for_target_blocks(
            network.peers(),
            2,
            Duration::from_millis(200),
            Duration::from_secs(5),
            &run_control,
        )
        .await?;
        network.shutdown().await;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn allow_net_false_rejects_runner() {
        let config = ChaosConfig {
            allow_net: false,
            peer_count: 1,
            faulty_peers: 0,
            duration: Duration::from_secs(1),
            pipeline_time: None,
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            seed: Some(5),
            tps: 0.1,
            max_inflight: 1,
            workload_profile: WorkloadProfile::Stable,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([false, false, false, false]),
            nexus: None,
        };

        let err = IzanamiRunner::new(config)
            .await
            .err()
            .expect("runner must reject allow_net=false");
        assert!(
            err.to_string().contains("allow_net=false"),
            "error should mention allow_net guard: {err:?}"
        );
    }

    #[test]
    fn fault_target_selection_is_deterministic() {
        let mut rng_a = StdRng::seed_from_u64(5);
        let mut rng_b = StdRng::seed_from_u64(5);

        let first = select_fault_targets(6, 2, &mut rng_a);
        let second = select_fault_targets(6, 2, &mut rng_b);

        assert_eq!(first, second, "same seed must yield same targets");
        assert_eq!(first.len(), 2);
    }

    #[test]
    fn fault_target_selection_diverges_with_different_seeds() {
        let mut rng_a = StdRng::seed_from_u64(11);
        let mut rng_b = StdRng::seed_from_u64(19);

        let first = select_fault_targets(8, 3, &mut rng_a);
        let second = select_fault_targets(8, 3, &mut rng_b);

        assert_eq!(first.len(), 3);
        assert_eq!(second.len(), 3);
        assert_ne!(
            first, second,
            "different seeds should produce different fault targets"
        );
    }

    #[test]
    fn metrics_snapshot_accumulates_counts() {
        let metrics = Metrics::default();
        metrics.record_success();
        metrics.record_success();
        metrics.record_failure();
        metrics.record_expected_failure();
        metrics.record_unexpected_success();

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.successes, 2);
        assert_eq!(snapshot.failures, 1);
        assert_eq!(snapshot.expected_failures, 1);
        assert_eq!(snapshot.unexpected_successes, 1);
    }

    #[test]
    fn submission_metadata_increments_counter() {
        let counter = AtomicU64::new(0);
        let meta_a = submission_metadata(&counter);
        let meta_b = submission_metadata(&counter);
        assert_eq!(counter.load(Ordering::Relaxed), 2);
        let key = SUBMISSION_METADATA_KEY
            .get_or_init(|| "izanami_submission_id".parse().expect("valid metadata key"));
        let value_a = meta_a
            .get(key)
            .and_then(|value| value.try_into_any::<u64>().ok())
            .expect("first metadata entry should decode");
        let value_b = meta_b
            .get(key)
            .and_then(|value| value.try_into_any::<u64>().ok())
            .expect("second metadata entry should decode");
        assert_ne!(value_a, value_b, "each submission should be unique");
    }

    #[test]
    fn make_network_builder_applies_pipeline_time() -> Result<()> {
        init_instruction_registry();
        let pipeline_time = Duration::from_millis(300);
        let config = ChaosConfig {
            allow_net: true,
            peer_count: 2,
            faulty_peers: 0,
            duration: Duration::from_secs(1),
            pipeline_time: Some(pipeline_time),
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            seed: Some(17),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([false, false, false, false]),
            nexus: None,
        };

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos { genesis, .. } =
            instructions::prepare_state(account_qty, None, config.workload_profile)?;
        let network = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            make_network_builder(&config, genesis).build()
        })) {
            Ok(network) => network,
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(ToString::to_string))
                    .unwrap_or_default();
                if msg.contains("Operation not permitted") || msg.contains("permission denied") {
                    return Ok(());
                }
                std::panic::resume_unwind(payload);
            }
        };

        assert_eq!(network.pipeline_time(), pipeline_time);
        let layers: Vec<Table> = network.config_layers().map(Cow::into_owned).collect();
        let read_i64 = |layer: &Table, path: &[&str]| -> Option<i64> {
            let mut current = layer;
            for (idx, key) in path.iter().enumerate() {
                let value = current.get(*key)?;
                if idx + 1 == path.len() {
                    return value.as_integer();
                }
                current = value.as_table()?;
            }
            None
        };
        let read_bool = |layer: &Table, path: &[&str]| -> Option<bool> {
            let mut current = layer;
            for (idx, key) in path.iter().enumerate() {
                let value = current.get(*key)?;
                if idx + 1 == path.len() {
                    return value.as_bool();
                }
                current = value.as_table()?;
            }
            None
        };
        let lookup = |path| layers.iter().rev().find_map(|layer| read_i64(layer, path));
        let lookup_bool = |path| layers.iter().rev().find_map(|layer| read_bool(layer, path));
        let npos_timing = derive_npos_timing(&config);
        let npos_block_ms =
            i64::try_from(npos_timing.block_ms).expect("npos block time fits into i64");
        let npos_propose_ms =
            i64::try_from(npos_timing.propose_ms).expect("npos propose timeout fits into i64");
        let npos_prevote_ms =
            i64::try_from(npos_timing.prevote_ms).expect("npos prevote timeout fits into i64");
        let npos_precommit_ms =
            i64::try_from(npos_timing.precommit_ms).expect("npos precommit timeout fits into i64");
        let npos_commit_ms = i64::try_from(npos_timing.commit_timeout_ms)
            .expect("npos commit timeout fits into i64");
        let npos_da_ms = i64::try_from(npos_timing.da_ms).expect("npos DA timeout fits into i64");
        let npos_aggregator_ms = i64::try_from(npos_timing.aggregator_ms)
            .expect("npos aggregator timeout fits into i64");
        assert_eq!(
            lookup_bool(&["pipeline", "dynamic_prepass"]),
            Some(IZANAMI_PIPELINE_DYNAMIC_PREPASS)
        );
        assert_eq!(
            lookup_bool(&["pipeline", "access_set_cache_enabled"]),
            Some(IZANAMI_PIPELINE_ACCESS_SET_CACHE_ENABLED)
        );
        assert_eq!(
            lookup_bool(&["pipeline", "parallel_overlay"]),
            Some(IZANAMI_PIPELINE_PARALLEL_OVERLAY)
        );
        assert_eq!(
            lookup_bool(&["pipeline", "parallel_apply"]),
            Some(IZANAMI_PIPELINE_PARALLEL_APPLY)
        );
        assert_eq!(
            lookup(&["pipeline", "workers"]),
            Some(IZANAMI_PIPELINE_WORKERS)
        );
        assert_eq!(
            lookup(&["pipeline", "signature_batch_max_ed25519"]),
            Some(IZANAMI_PIPELINE_SIGNATURE_BATCH_MAX_ED25519)
        );
        assert_eq!(
            lookup(&["pipeline", "signature_batch_max_secp256k1"]),
            Some(IZANAMI_PIPELINE_SIGNATURE_BATCH_MAX_SECP256K1)
        );
        assert_eq!(
            lookup(&["pipeline", "signature_batch_max_pqc"]),
            Some(IZANAMI_PIPELINE_SIGNATURE_BATCH_MAX_PQC)
        );
        assert_eq!(
            lookup(&["pipeline", "signature_batch_max_bls"]),
            Some(IZANAMI_PIPELINE_SIGNATURE_BATCH_MAX_BLS)
        );
        assert_eq!(
            lookup(&["pipeline", "stateless_cache_cap"]),
            Some(IZANAMI_PIPELINE_STATELESS_CACHE_CAP)
        );
        assert_eq!(
            lookup(&["sumeragi", "queues", "block_payload"]),
            Some(IZANAMI_BLOCK_PAYLOAD_QUEUE)
        );
        assert_eq!(
            lookup(&["sumeragi", "worker", "validation_worker_threads"]),
            Some(IZANAMI_VALIDATION_WORKER_THREADS)
        );
        assert_eq!(
            lookup(&["sumeragi", "worker", "validation_work_queue_cap"]),
            Some(IZANAMI_VALIDATION_WORK_QUEUE_CAP)
        );
        assert_eq!(
            lookup(&["sumeragi", "worker", "validation_result_queue_cap"]),
            Some(IZANAMI_VALIDATION_RESULT_QUEUE_CAP)
        );
        assert_eq!(
            lookup(&["sumeragi", "worker", "validation_pending_cap"]),
            Some(IZANAMI_VALIDATION_PENDING_CAP)
        );
        assert_eq!(
            lookup(&["sumeragi", "pacemaker", "pending_stall_grace_ms"]),
            Some(IZANAMI_PACEMAKER_PENDING_STALL_GRACE_MS)
        );
        assert_eq!(
            lookup(&["sumeragi", "pacemaker", "active_pending_soft_limit"]),
            Some(IZANAMI_PACEMAKER_ACTIVE_PENDING_SOFT_LIMIT)
        );
        assert_eq!(
            lookup(&["sumeragi", "pacemaker", "rbc_backlog_session_soft_limit"]),
            Some(IZANAMI_PACEMAKER_RBC_BACKLOG_SESSION_SOFT_LIMIT)
        );
        assert_eq!(
            lookup(&["sumeragi", "pacemaker", "rbc_backlog_chunk_soft_limit"]),
            Some(IZANAMI_PACEMAKER_RBC_BACKLOG_CHUNK_SOFT_LIMIT)
        );
        assert_eq!(
            lookup(&["sumeragi", "rbc", "pending_max_chunks"]),
            Some(IZANAMI_RBC_PENDING_MAX_CHUNKS)
        );
        assert_eq!(
            lookup(&["sumeragi", "rbc", "pending_max_bytes"]),
            Some(IZANAMI_RBC_PENDING_MAX_BYTES)
        );
        assert_eq!(
            lookup(&["sumeragi", "rbc", "pending_session_limit"]),
            Some(IZANAMI_RBC_PENDING_SESSION_LIMIT)
        );
        assert_eq!(
            lookup(&["sumeragi", "rbc", "pending_ttl_ms"]),
            Some(IZANAMI_RBC_PENDING_TTL_MS)
        );
        assert_eq!(
            lookup(&["sumeragi", "rbc", "session_ttl_ms"]),
            Some(IZANAMI_RBC_SESSION_TTL_MS)
        );
        assert_eq!(
            lookup(&["sumeragi", "rbc", "disk_store_ttl_ms"]),
            Some(IZANAMI_RBC_SESSION_TTL_MS)
        );
        assert_eq!(
            lookup(&["sumeragi", "rbc", "rebroadcast_sessions_per_tick"]),
            Some(IZANAMI_RBC_REBROADCAST_SESSIONS_PER_TICK)
        );
        assert_eq!(
            lookup(&["sumeragi", "rbc", "payload_chunks_per_tick"]),
            Some(IZANAMI_RBC_PAYLOAD_CHUNKS_PER_TICK)
        );
        assert_eq!(
            lookup(&["sumeragi", "da", "quorum_timeout_multiplier"]),
            Some(IZANAMI_DA_QUORUM_TIMEOUT_MULTIPLIER)
        );
        assert_eq!(
            lookup(&["sumeragi", "gating", "future_height_window"]),
            Some(IZANAMI_FUTURE_HEIGHT_WINDOW)
        );
        assert_eq!(
            lookup(&["sumeragi", "gating", "future_view_window"]),
            Some(IZANAMI_FUTURE_VIEW_WINDOW)
        );
        assert_eq!(
            lookup(&["sumeragi", "npos", "block_time_ms"]),
            Some(npos_block_ms)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "npos", "timeouts", "propose_ms"]),
            Some(npos_propose_ms)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "npos", "timeouts", "prevote_ms"]),
            Some(npos_prevote_ms)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "npos", "timeouts", "precommit_ms"]),
            Some(npos_precommit_ms)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "npos", "timeouts", "commit_ms"]),
            Some(npos_commit_ms)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "npos", "timeouts", "da_ms"]),
            Some(npos_da_ms)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "npos", "timeouts", "aggregator_ms"]),
            Some(npos_aggregator_ms)
        );
        Ok(())
    }

    #[test]
    fn make_network_builder_forwards_rust_log() -> Result<()> {
        init_instruction_registry();
        let _env_guard = EnvGuard::set("RUST_LOG", "iroha_p2p=debug,iroha_core=debug");
        let config = ChaosConfig {
            allow_net: true,
            peer_count: 2,
            faulty_peers: 0,
            duration: Duration::from_secs(1),
            pipeline_time: None,
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            seed: Some(19),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([false, false, false, false]),
            nexus: None,
        };

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos { genesis, .. } =
            instructions::prepare_state(account_qty, None, config.workload_profile)?;
        let network = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            make_network_builder(&config, genesis).build()
        })) {
            Ok(network) => network,
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(ToString::to_string))
                    .unwrap_or_default();
                if msg.contains("Operation not permitted") || msg.contains("permission denied") {
                    return Ok(());
                }
                std::panic::resume_unwind(payload);
            }
        };

        let layers: Vec<Table> = network.config_layers().map(Cow::into_owned).collect();
        let read_str = |layer: &Table, path: &[&str]| -> Option<String> {
            let mut current = layer;
            for (idx, key) in path.iter().enumerate() {
                let value = current.get(*key)?;
                if idx + 1 == path.len() {
                    return value.as_str().map(ToString::to_string);
                }
                current = value.as_table()?;
            }
            None
        };
        let filter = layers
            .iter()
            .rev()
            .find_map(|layer| read_str(layer, &["logger", "filter"]));
        assert_eq!(filter.as_deref(), Some("iroha_p2p=debug,iroha_core=debug"));

        Ok(())
    }

    #[test]
    fn make_network_builder_injects_npos_parameters() -> Result<()> {
        init_instruction_registry();
        let profile = crate::config::NexusProfile::sora_defaults()?;
        let config = ChaosConfig {
            allow_net: true,
            peer_count: 2,
            faulty_peers: 0,
            duration: Duration::from_secs(1),
            pipeline_time: None,
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            seed: Some(19),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([false, false, false, false]),
            nexus: Some(profile),
        };

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos { genesis, .. } = instructions::prepare_state(
            account_qty,
            config.nexus.as_ref(),
            config.workload_profile,
        )?;
        let network = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            make_network_builder(&config, genesis).build()
        })) {
            Ok(network) => network,
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(ToString::to_string))
                    .unwrap_or_default();
                if msg.contains("Operation not permitted") || msg.contains("permission denied") {
                    return Ok(());
                }
                std::panic::resume_unwind(payload);
            }
        };

        let mut params = Parameters::default();
        for tx in network.genesis_isi() {
            for isi in tx {
                let Some(set_param) = isi.as_any().downcast_ref::<SetParameter>() else {
                    continue;
                };
                params.set_parameter(set_param.inner().clone());
            }
        }

        let expected = derive_npos_timing(&config);
        assert_eq!(params.sumeragi().block_time_ms(), expected.block_ms);
        assert_eq!(params.sumeragi().commit_time_ms(), expected.commit_time_ms);
        assert!(
            params
                .custom()
                .get(&SumeragiNposParameters::parameter_id())
                .and_then(SumeragiNposParameters::from_custom_parameter)
                .is_some()
        );
        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn runner_respects_deadline_and_shuts_down() -> Result<()> {
        if !allow_net_for_tests() {
            return Ok(());
        }
        crate::config::init_tracing_with_filter("warn");
        init_instruction_registry();

        let config = ChaosConfig {
            allow_net: true,
            peer_count: 2,
            faulty_peers: 0,
            duration: Duration::from_secs(1),
            pipeline_time: None,
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            seed: Some(13),
            tps: 0.5,
            max_inflight: 2,
            workload_profile: WorkloadProfile::Stable,
            fault_interval: Duration::from_secs(5)..=Duration::from_secs(5),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([false, false, false, false]),
            nexus: None,
        };

        let runner = match IzanamiRunner::new(config).await {
            Ok(runner) => runner,
            Err(err) => {
                let looks_like_permission_denied = err
                    .downcast_ref::<io::Error>()
                    .is_some_and(|io_err| io_err.kind() == io::ErrorKind::PermissionDenied)
                    || err.to_string().contains("Operation not permitted");
                if looks_like_permission_denied {
                    return Ok(());
                }
                return Err(err);
            }
        };

        timeout(Duration::from_secs(20), runner.run())
            .await
            .map_err(|_| eyre!("runner timed out before deadline"))??;
        Ok(())
    }

    #[test]
    fn nexus_profile_wires_rbc_da_and_config_layer() -> Result<()> {
        init_instruction_registry();
        let nexus = NexusProfile::sora_defaults()?;
        let config = ChaosConfig {
            allow_net: true,
            peer_count: 3,
            faulty_peers: 0,
            duration: Duration::from_secs(1),
            pipeline_time: None,
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            seed: Some(23),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([false, false, false, false]),
            nexus: Some(nexus.clone()),
        };

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos { genesis, .. } = instructions::prepare_state(
            account_qty,
            config.nexus.as_ref(),
            config.workload_profile,
        )?;
        let network = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            make_network_builder(&config, genesis).build()
        })) {
            Ok(network) => network,
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(ToString::to_string))
                    .unwrap_or_default();
                if msg.contains("Operation not permitted") || msg.contains("permission denied") {
                    return Ok(());
                }
                std::panic::resume_unwind(payload);
            }
        };

        let mut saw_da_enabled = false;
        for tx in network.genesis_isi() {
            for isi in tx {
                if let Some(set_param) = isi.as_any().downcast_ref::<SetParameter>()
                    && let Parameter::Sumeragi(SumeragiParameter::DaEnabled(value)) =
                        set_param.inner()
                {
                    saw_da_enabled = saw_da_enabled || *value == nexus.da_enabled;
                }
            }
        }
        assert!(
            saw_da_enabled,
            "DA parameter should be threaded from nexus profile"
        );

        let layers: Vec<_> = network.config_layers().collect();
        assert!(
            layers.len() >= 2,
            "expected base layer plus nexus config layer"
        );
        let has_nexus_layer = layers.iter().any(|layer| {
            layer
                .as_ref()
                .get("nexus")
                .and_then(toml::Value::as_table)
                .and_then(|table| table.get("enabled"))
                .and_then(toml::Value::as_bool)
                .unwrap_or(false)
        });
        assert!(has_nexus_layer, "nexus config layer must be attached");
        let lane_catalog = layers.iter().find_map(|layer| {
            layer
                .as_ref()
                .get("nexus")
                .and_then(toml::Value::as_table)
                .and_then(|table| table.get("lane_catalog"))
                .and_then(toml::Value::as_array)
        });
        let Some(lane_catalog) = lane_catalog else {
            return Err(eyre!("expected nexus lane_catalog in config layer"));
        };
        let missing_metadata = lane_catalog.iter().any(|entry| {
            entry
                .as_table()
                .and_then(|table| table.get("metadata"))
                .and_then(toml::Value::as_table)
                .is_none()
        });
        assert!(
            !missing_metadata,
            "nexus lane_catalog entries must include metadata"
        );

        Ok(())
    }
}
