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
use iroha_data_model::prelude::*;
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

fn make_network_builder(config: &ChaosConfig, genesis: Vec<Vec<InstructionBox>>) -> NetworkBuilder {
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
    use std::io;

    use color_eyre::eyre::{WrapErr, eyre};
    use iroha_data_model::{isi::SetParameter, parameter::SumeragiParameter};
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
