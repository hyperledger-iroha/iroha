#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Localnet simulation for a public-style Testus rollout with churn and fixed TPS load.

use std::{
    any::Any,
    fs,
    net::SocketAddr as StdSocketAddr,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::sandbox;
use iroha::{
    client::Client,
    config::{Config, LoadPath},
    crypto::{Algorithm, ExposedPrivateKey, KeyPair, bls_normal_pop_prove},
    data_model::{
        Level,
        isi::{InstructionBox, Log, Unregister, register::RegisterPeerWithPop},
        peer::PeerId,
        prelude::QueryBuilderExt,
        query::peer::prelude::FindPeers,
    },
};
use iroha_primitives::addr::SocketAddr as IrohaSocketAddr;
use iroha_test_network::{
    Program, fslock_ports::AllocatedPortBlock, init_instruction_registry, repo_root,
};
use tempfile::TempDir;
use tokio::time::sleep;
use toml::{Table, Value as TomlValue};

const TESTUS_VALIDATORS: u16 = 7;
const TESTUS_TOTAL_PORT_SLOTS: u16 = TESTUS_VALIDATORS + 1;
const READY_TIMEOUT: Duration = Duration::from_secs(180);
const STATUS_POLL: Duration = Duration::from_millis(200);
const MONITOR_PERIOD: Duration = Duration::from_secs(1);
const DEFAULT_STALL_TIMEOUT_SECS: u64 = 300;
const DEFAULT_SIM_DURATION_SECS: u64 = 3_600;
const DEFAULT_LOAD_TPS: u64 = 5;
const DEFAULT_CHURN_INTERVAL_SECS: u64 = 300;
const DEFAULT_MAX_HEIGHT_SKEW: u64 = 2;
const DEFAULT_MAX_HEIGHT_SKEW_GRACE_SECS: u64 = 30;
const INTERIM_CONVERGENCE_MAX_SKEW: u64 = 6;
const DEFAULT_MAX_VIEW_CHANGE_RATE: f64 = 0.2;
const PROCESS_DOWNTIME_SECS: u64 = 5;
const JOINER_CATCHUP_TIMEOUT_SECS: u64 = 60;
const LOCALNET_BLOCK_TIME_MS: u64 = 1_000;
const LOCALNET_COMMIT_TIME_MS: u64 = 1_000;
const MAX_TX_BURST_PER_TICK: u32 = 32;
const TORII_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const FINAL_SETTLE_WINDOW: Duration = Duration::from_secs(120);

#[derive(Clone, Copy)]
struct SimulationModes {
    process_churn: bool,
    membership_churn: bool,
}

#[derive(Clone, Copy)]
struct SimulationConfig {
    duration: Duration,
    tps: u64,
    churn_interval: Duration,
    max_height_skew: u64,
    max_height_skew_grace: Duration,
    stall_timeout: Duration,
    max_view_change_rate: f64,
    process_downtime: Duration,
}

impl SimulationConfig {
    fn from_env() -> Self {
        Self {
            duration: Duration::from_secs(env_u64(
                "IROHA_TESTUS_SIM_DURATION_SECS",
                DEFAULT_SIM_DURATION_SECS,
                30,
            )),
            tps: env_u64("IROHA_TESTUS_LOAD_TPS", DEFAULT_LOAD_TPS, 1),
            churn_interval: Duration::from_secs(env_u64(
                "IROHA_TESTUS_CHURN_INTERVAL_SECS",
                DEFAULT_CHURN_INTERVAL_SECS,
                30,
            )),
            max_height_skew: env_u64("IROHA_TESTUS_MAX_HEIGHT_SKEW", DEFAULT_MAX_HEIGHT_SKEW, 0),
            max_height_skew_grace: Duration::from_secs(env_u64(
                "IROHA_TESTUS_MAX_HEIGHT_SKEW_GRACE_SECS",
                DEFAULT_MAX_HEIGHT_SKEW_GRACE_SECS,
                0,
            )),
            stall_timeout: Duration::from_secs(env_u64(
                "IROHA_TESTUS_STALL_TIMEOUT_SECS",
                DEFAULT_STALL_TIMEOUT_SECS,
                10,
            )),
            max_view_change_rate: env_f64(
                "IROHA_TESTUS_MAX_VIEW_CHANGE_RATE",
                DEFAULT_MAX_VIEW_CHANGE_RATE,
                0.0,
            ),
            process_downtime: Duration::from_secs(PROCESS_DOWNTIME_SECS),
        }
    }

    fn quick(duration_secs: u64, churn_interval_secs: u64) -> Self {
        Self {
            duration: Duration::from_secs(duration_secs),
            tps: DEFAULT_LOAD_TPS,
            churn_interval: Duration::from_secs(churn_interval_secs),
            max_height_skew: DEFAULT_MAX_HEIGHT_SKEW,
            max_height_skew_grace: Duration::from_secs(DEFAULT_MAX_HEIGHT_SKEW_GRACE_SECS),
            stall_timeout: Duration::from_secs(DEFAULT_STALL_TIMEOUT_SECS),
            max_view_change_rate: DEFAULT_MAX_VIEW_CHANGE_RATE,
            process_downtime: Duration::from_secs(2),
        }
    }
}

#[derive(Clone, Debug)]
struct SimulationSummary {
    duration_secs: u64,
    target_tps: u64,
    tx_sent: u64,
    tx_submit_errors: u64,
    process_churn_cycles: u64,
    membership_join_cycles: u64,
    membership_leave_cycles: u64,
    max_height_skew_observed: u64,
    view_changes_start: u64,
    view_changes_end: u64,
    view_change_rate_per_sec: f64,
    submitted_tps: f64,
    saturated_samples: u64,
    total_samples: u64,
}

impl SimulationSummary {
    fn to_json_value(&self) -> norito::json::Value {
        norito::json!({
            "duration_secs": (self.duration_secs),
            "target_tps": (self.target_tps),
            "tx_sent": (self.tx_sent),
            "tx_submit_errors": (self.tx_submit_errors),
            "process_churn_cycles": (self.process_churn_cycles),
            "membership_join_cycles": (self.membership_join_cycles),
            "membership_leave_cycles": (self.membership_leave_cycles),
            "max_height_skew_observed": (self.max_height_skew_observed),
            "view_changes_start": (self.view_changes_start),
            "view_changes_end": (self.view_changes_end),
            "view_change_rate_per_sec": (self.view_change_rate_per_sec),
            "submitted_tps": (self.submitted_tps),
            "saturated_samples": (self.saturated_samples),
            "total_samples": (self.total_samples),
        })
    }
}

struct JoinerPeer {
    peer_id: PeerId,
    pop: Vec<u8>,
    config_path: PathBuf,
    client: Client,
}

struct TestusHarness {
    out_dir: PathBuf,
    localnet: ManagedLocalnet,
    primary_client: Client,
    validator_clients: Vec<Client>,
    joiner: JoinerPeer,
}

impl TestusHarness {
    fn summary_path(&self) -> PathBuf {
        self.out_dir.join("testus_simulation_summary.json")
    }
}

struct ManagedLocalnet {
    dir: PathBuf,
    irohad_bin: PathBuf,
    validator_count: u16,
    validator_children: Vec<Option<Child>>,
    joiner_child: Option<Child>,
    _port_reservations: (AllocatedPortBlock, AllocatedPortBlock),
}

impl ManagedLocalnet {
    fn start(
        out_dir: &Path,
        irohad_bin: &Path,
        validator_count: u16,
        port_reservations: (AllocatedPortBlock, AllocatedPortBlock),
    ) -> Result<Self> {
        let mut this = Self {
            dir: out_dir.to_path_buf(),
            irohad_bin: irohad_bin.to_path_buf(),
            validator_count,
            validator_children: (0..validator_count).map(|_| None).collect(),
            joiner_child: None,
            _port_reservations: port_reservations,
        };
        for idx in 0..usize::from(validator_count) {
            this.start_validator(idx)?;
        }
        Ok(this)
    }

    fn start_validator(&mut self, idx: usize) -> Result<()> {
        ensure!(
            idx < usize::from(self.validator_count),
            "validator index out of bounds: {idx}"
        );
        if self
            .validator_children
            .get_mut(idx)
            .and_then(Option::as_mut)
            .is_some_and(|child| child.try_wait().ok().flatten().is_none())
        {
            return Ok(());
        }
        let config_path = self.dir.join(format!("peer{idx}.toml"));
        let snapshot_dir = self
            .dir
            .join("storage")
            .join(format!("peer{idx}"))
            .join("snapshot");
        fs::create_dir_all(&snapshot_dir)
            .wrap_err_with(|| format!("create snapshot dir {}", snapshot_dir.display()))?;
        let child = self.spawn_with_config(
            &config_path,
            &snapshot_dir,
            &format!("peer{idx}.log"),
            &format!("peer{idx}"),
        )?;
        self.validator_children[idx] = Some(child);
        Ok(())
    }

    fn stop_validator(&mut self, idx: usize) -> Result<()> {
        ensure!(
            idx < usize::from(self.validator_count),
            "validator index out of bounds: {idx}"
        );
        if let Some(mut child) = self.validator_children[idx].take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    fn start_joiner(&mut self, config_path: &Path) -> Result<()> {
        if self
            .joiner_child
            .as_mut()
            .is_some_and(|child| child.try_wait().ok().flatten().is_none())
        {
            return Ok(());
        }
        let snapshot_dir = self.dir.join("storage").join("joiner").join("snapshot");
        fs::create_dir_all(&snapshot_dir)
            .wrap_err_with(|| format!("create joiner snapshot dir {}", snapshot_dir.display()))?;
        let child = self.spawn_with_config(config_path, &snapshot_dir, "joiner.log", "joiner")?;
        self.joiner_child = Some(child);
        Ok(())
    }

    fn stop_joiner(&mut self) -> Result<()> {
        if let Some(mut child) = self.joiner_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    fn spawn_with_config(
        &self,
        config_path: &Path,
        snapshot_dir: &Path,
        log_name: &str,
        node_label: &str,
    ) -> Result<Child> {
        let log_path = self.dir.join(log_name);
        let log_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .wrap_err_with(|| format!("open log file {}", log_path.display()))?;
        let log_file_err = log_file
            .try_clone()
            .wrap_err_with(|| format!("clone log file {}", log_path.display()))?;

        let mut cmd = Command::new(&self.irohad_bin);
        cmd.arg("--sora");
        cmd.arg("--config").arg(config_path);
        cmd.current_dir(&self.dir);
        cmd.env("SNAPSHOT_STORE_DIR", snapshot_dir);
        if std::env::var_os("RUST_LOG").is_none() {
            cmd.env("RUST_LOG", "info");
        }
        cmd.stdout(Stdio::from(log_file));
        cmd.stderr(Stdio::from(log_file_err));

        cmd.spawn()
            .wrap_err_with(|| format!("spawn irohad for {node_label}"))
    }
}

impl Drop for ManagedLocalnet {
    fn drop(&mut self) {
        for idx in 0..usize::from(self.validator_count) {
            let _ = self.stop_validator(idx);
        }
        let _ = self.stop_joiner();
        if cfg!(unix) {
            let script = self.dir.join("stop.sh");
            if script.exists() {
                let _ = Command::new("bash")
                    .arg(script)
                    .current_dir(&self.dir)
                    .output();
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn testus_localnet_bootstrap_7_validators() -> Result<()> {
    init_instruction_registry();
    let _guard = sandbox::serial_guard();

    let temp_dir = localnet_tempdir("testus-bootstrap")?;
    let out_dir = temp_dir.path().join("localnet");
    let result: Result<()> = async {
        let harness = setup_testus_harness(&out_dir, "testus-bootstrap").await?;
        wait_for_cluster_convergence(
            &harness.validator_clients,
            harness.primary_client.get_status()?.blocks,
            DEFAULT_MAX_HEIGHT_SKEW,
            READY_TIMEOUT,
        )
        .await?;

        let baseline = harness.primary_client.get_status()?.blocks_non_empty;
        harness.primary_client.submit::<InstructionBox>(
            Log::new(Level::INFO, "testus bootstrap probe".to_string()).into(),
        )?;
        wait_for_blocks_non_empty(
            &harness.primary_client,
            baseline.saturating_add(1),
            READY_TIMEOUT,
        )
        .await?;
        Ok(())
    }
    .await;

    finalize_result(temp_dir, "testus_localnet_bootstrap_7_validators", result)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires local process orchestration"]
async fn testus_localnet_joiner_register_unregister_behavior() -> Result<()> {
    init_instruction_registry();
    let _guard = sandbox::serial_guard();

    let temp_dir = localnet_tempdir("testus-membership")?;
    let out_dir = temp_dir.path().join("localnet");
    let result: Result<()> = async {
        let mut harness = setup_testus_harness(&out_dir, "testus-membership").await?;
        membership_join_cycle(&mut harness).await?;
        membership_leave_cycle(&mut harness).await?;
        Ok(())
    }
    .await;

    finalize_result(
        temp_dir,
        "testus_localnet_joiner_register_unregister_behavior",
        result,
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires local process orchestration"]
async fn testus_localnet_restart_catchup_behavior() -> Result<()> {
    init_instruction_registry();
    let _guard = sandbox::serial_guard();

    let temp_dir = localnet_tempdir("testus-restart")?;
    let out_dir = temp_dir.path().join("localnet");
    let result: Result<()> = async {
        let mut harness = setup_testus_harness(&out_dir, "testus-restart").await?;
        process_churn_cycle(&mut harness, 0, Duration::from_secs(PROCESS_DOWNTIME_SECS)).await?;
        Ok(())
    }
    .await;

    finalize_result(temp_dir, "testus_localnet_restart_catchup_behavior", result)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "long-running testus simulation"]
async fn testus_public_localnet_5tps_churn_stability() -> Result<()> {
    init_instruction_registry();
    let _guard = sandbox::serial_guard();

    let cfg = SimulationConfig::from_env();
    let temp_dir = localnet_tempdir("testus-simulation")?;
    let out_dir = temp_dir.path().join("localnet");
    let result: Result<()> = async move {
        let mut harness = setup_testus_harness(&out_dir, "testus-public-sim").await?;
        let summary = run_testus_simulation(
            &mut harness,
            cfg,
            SimulationModes {
                process_churn: true,
                membership_churn: true,
            },
        )
        .await?;
        write_summary(&harness.summary_path(), &summary)?;
        Ok(())
    }
    .await;

    finalize_result(
        temp_dir,
        "testus_public_localnet_5tps_churn_stability",
        result,
    )
}

async fn run_testus_simulation(
    harness: &mut TestusHarness,
    cfg: SimulationConfig,
    modes: SimulationModes,
) -> Result<SimulationSummary> {
    ensure!(cfg.tps > 0, "tps must be greater than zero");

    let mut tx_sent = 0_u64;
    let mut tx_submit_errors = 0_u64;
    let mut process_churn_cycles = 0_u64;
    let mut membership_join_cycles = 0_u64;
    let mut membership_leave_cycles = 0_u64;
    let mut max_height_skew_observed = 0_u64;
    let mut saturated_samples = 0_u64;
    let mut total_samples = 0_u64;
    let mut joiner_active = false;
    let mut restart_idx = first_process_churn_index(harness.validator_clients.len());
    let mut paused_for_churn = Duration::ZERO;
    let mut skew_breach_started_at = None;

    let initial_statuses = collect_statuses(&harness.validator_clients)?;
    let view_changes_start = max_view_changes(&initial_statuses);
    let mut view_changes_end = view_changes_start;
    let mut last_progress_height = max_height(&initial_statuses);
    let mut last_progress_at = Instant::now();
    let mut last_min_progress_height = min_height(&initial_statuses);
    let mut last_min_progress_at = Instant::now();

    let mut next_tx = Instant::now();
    let mut next_monitor = Instant::now();
    let mut next_process_churn = Instant::now() + cfg.churn_interval;
    let membership_offset = cfg.churn_interval / 2;
    let mut next_membership_churn = Instant::now() + membership_offset;
    let tx_period = Duration::from_secs_f64(1.0 / cfg.tps as f64);
    let start_time = Instant::now();
    let deadline = start_time + cfg.duration;

    while Instant::now() < deadline {
        let now = Instant::now();
        let allow_churn = deadline.saturating_duration_since(now) > FINAL_SETTLE_WINDOW;

        if modes.process_churn && allow_churn && now >= next_process_churn {
            let churn_start = Instant::now();
            process_churn_cycle(harness, restart_idx, cfg.process_downtime).await?;
            paused_for_churn = paused_for_churn.saturating_add(churn_start.elapsed());
            let statuses_after_churn = collect_statuses(&harness.validator_clients)?;
            let max_after_churn = max_height(&statuses_after_churn);
            if max_after_churn > last_progress_height {
                last_progress_height = max_after_churn;
            }
            last_progress_at = Instant::now();
            let min_after_churn = min_height(&statuses_after_churn);
            if min_after_churn > last_min_progress_height {
                last_min_progress_height = min_after_churn;
            }
            last_min_progress_at = Instant::now();
            restart_idx = next_process_churn_index(restart_idx, harness.validator_clients.len());
            process_churn_cycles = process_churn_cycles.saturating_add(1);
            next_process_churn = Instant::now() + cfg.churn_interval;
            continue;
        }

        if modes.membership_churn && allow_churn && now >= next_membership_churn {
            let churn_start = Instant::now();
            if joiner_active {
                membership_leave_cycle(harness).await?;
                membership_leave_cycles = membership_leave_cycles.saturating_add(1);
            } else {
                membership_join_cycle(harness).await?;
                membership_join_cycles = membership_join_cycles.saturating_add(1);
            }
            paused_for_churn = paused_for_churn.saturating_add(churn_start.elapsed());
            let statuses_after_churn = collect_statuses(&harness.validator_clients)?;
            let max_after_churn = max_height(&statuses_after_churn);
            if max_after_churn > last_progress_height {
                last_progress_height = max_after_churn;
            }
            last_progress_at = Instant::now();
            let min_after_churn = min_height(&statuses_after_churn);
            if min_after_churn > last_min_progress_height {
                last_min_progress_height = min_after_churn;
            }
            last_min_progress_at = Instant::now();
            joiner_active = !joiner_active;
            next_membership_churn = Instant::now() + cfg.churn_interval;
            continue;
        }

        if now >= next_tx {
            let mut burst_submitted = 0_u32;
            let mut catchup_now = now;
            while catchup_now >= next_tx && burst_submitted < MAX_TX_BURST_PER_TICK {
                let msg = format!("testus-load-{tx_sent}");
                if let Err(err) = harness
                    .primary_client
                    .submit::<InstructionBox>(Log::new(Level::INFO, msg).into())
                {
                    tx_submit_errors = tx_submit_errors.saturating_add(1);
                    eprintln!("testus load submit failed: {err:?}");
                    if is_http_timeout_error(&err) {
                        next_tx = Instant::now() + tx_period;
                        break;
                    }
                } else {
                    tx_sent = tx_sent.saturating_add(1);
                }
                burst_submitted = burst_submitted.saturating_add(1);
                next_tx += tx_period;
                catchup_now = Instant::now();
            }
            if catchup_now >= next_tx {
                next_tx = catchup_now + tx_period;
            }
            continue;
        }

        if now >= next_monitor {
            let statuses = collect_statuses(&harness.validator_clients)?;
            let max_height = max_height(&statuses);
            let min_height = min_height(&statuses);
            let skew = max_height.saturating_sub(min_height);
            max_height_skew_observed = max_height_skew_observed.max(skew);
            skew_breach_started_at =
                update_skew_breach_started(skew_breach_started_at, skew, cfg.max_height_skew, now);
            if let Some(breach_start) = skew_breach_started_at {
                let breach_duration = now.saturating_duration_since(breach_start);
                let min_progress_age = now.saturating_duration_since(last_min_progress_at);
                ensure!(
                    !is_skew_breach_unrecovering(
                        breach_duration,
                        min_progress_age,
                        cfg.max_height_skew_grace,
                        cfg.stall_timeout,
                    ),
                    "validator height skew exceeded threshold without lagging-peer recovery: observed={skew}, threshold={}, breach_duration={breach_duration:?}, min_progress_age={min_progress_age:?}, grace={:?}, min_progress_timeout={:?}, max_height={max_height}, min_height={min_height}",
                    cfg.max_height_skew,
                    cfg.max_height_skew_grace,
                    cfg.stall_timeout
                );
            }

            if min_height > last_min_progress_height {
                last_min_progress_height = min_height;
                last_min_progress_at = now;
            }

            if max_height > last_progress_height {
                last_progress_height = max_height;
                last_progress_at = Instant::now();
            }

            ensure!(
                Instant::now().saturating_duration_since(last_progress_at) <= cfg.stall_timeout,
                "consensus stalled: no max-height progression for {:?} (max_height={max_height}, min_height={min_height}, last_progress_height={last_progress_height})",
                cfg.stall_timeout,
            );

            view_changes_end = max_view_changes(&statuses);
            for status in &statuses {
                if let Some(sumeragi) = status.sumeragi.as_ref() {
                    total_samples = total_samples.saturating_add(1);
                    if sumeragi.tx_queue_saturated {
                        saturated_samples = saturated_samples.saturating_add(1);
                    }
                    ensure!(
                        sumeragi.commit_qc_height
                            <= status.blocks.saturating_add(cfg.max_height_skew),
                        "commit_qc height looks inconsistent with status height: commit_qc={} blocks={}",
                        sumeragi.commit_qc_height,
                        status.blocks
                    );
                }
            }

            next_monitor = Instant::now() + MONITOR_PERIOD;
            continue;
        }

        let mut wakeup = deadline;
        wakeup = wakeup.min(next_tx);
        wakeup = wakeup.min(next_monitor);
        if modes.process_churn {
            wakeup = wakeup.min(next_process_churn);
        }
        if modes.membership_churn {
            wakeup = wakeup.min(next_membership_churn);
        }
        let now = Instant::now();
        if wakeup > now {
            sleep(wakeup.saturating_duration_since(now)).await;
        }
    }

    if joiner_active {
        let churn_start = Instant::now();
        membership_leave_cycle(harness).await?;
        paused_for_churn = paused_for_churn.saturating_add(churn_start.elapsed());
        membership_leave_cycles = membership_leave_cycles.saturating_add(1);
    }

    let elapsed = Instant::now().saturating_duration_since(start_time);
    let duration_secs = elapsed.as_secs().max(1);
    let active_elapsed = elapsed.saturating_sub(paused_for_churn);
    let submitted_tps = tx_sent as f64 / active_elapsed.as_secs_f64().max(1.0);
    let view_change_rate_per_sec = (view_changes_end.saturating_sub(view_changes_start)) as f64
        / elapsed.as_secs_f64().max(1.0);

    ensure!(
        submitted_tps >= (cfg.tps as f64 * 0.8),
        "submitted tps is below threshold: submitted_tps={submitted_tps:.2}, target={}",
        cfg.tps
    );
    ensure!(
        view_change_rate_per_sec <= cfg.max_view_change_rate,
        "view-change rate exceeded threshold: observed={view_change_rate_per_sec:.4}, threshold={}",
        cfg.max_view_change_rate
    );
    ensure!(
        tx_submit_errors <= tx_sent / 20 + 1,
        "tx submission errors too high: errors={tx_submit_errors}, sent={tx_sent}"
    );

    wait_for_cluster_convergence(
        &harness.validator_clients,
        validator_max_height_with_retry(&harness.validator_clients, READY_TIMEOUT).await?,
        cfg.max_height_skew,
        READY_TIMEOUT,
    )
    .await?;

    Ok(SimulationSummary {
        duration_secs,
        target_tps: cfg.tps,
        tx_sent,
        tx_submit_errors,
        process_churn_cycles,
        membership_join_cycles,
        membership_leave_cycles,
        max_height_skew_observed,
        view_changes_start,
        view_changes_end,
        view_change_rate_per_sec,
        submitted_tps,
        saturated_samples,
        total_samples,
    })
}

async fn process_churn_cycle(
    harness: &mut TestusHarness,
    idx: usize,
    downtime: Duration,
) -> Result<()> {
    ensure!(
        idx < harness.validator_clients.len(),
        "validator index out of bounds for process churn: {idx}"
    );
    let baseline = max_height(&collect_statuses(&harness.validator_clients)?);
    harness.localnet.stop_validator(idx)?;
    sleep(downtime).await;
    harness.localnet.start_validator(idx)?;
    wait_for_status_ready(&harness.validator_clients[idx], READY_TIMEOUT).await?;
    let restart_target = validator_restart_catchup_target(baseline);
    if let Err(err) = wait_for_height_at_least(
        &harness.validator_clients[idx],
        restart_target,
        READY_TIMEOUT,
    )
    .await
    {
        eprintln!(
            "validator restart catch-up lagged: baseline={baseline}, target={restart_target}, err={err:?}"
        );
    }
    if let Err(err) = wait_for_cluster_convergence(
        &harness.validator_clients,
        baseline,
        INTERIM_CONVERGENCE_MAX_SKEW,
        READY_TIMEOUT,
    )
    .await
    {
        eprintln!("validator restart convergence lagged: {err:?}");
    }
    Ok(())
}

fn validator_restart_catchup_target(baseline: u64) -> u64 {
    baseline.saturating_sub(INTERIM_CONVERGENCE_MAX_SKEW)
}

async fn membership_join_cycle(harness: &mut TestusHarness) -> Result<()> {
    let baseline =
        validator_max_height_with_retry(&harness.validator_clients, READY_TIMEOUT).await?;
    if !is_peer_present(&harness.primary_client, &harness.joiner.peer_id)? {
        let register: InstructionBox =
            RegisterPeerWithPop::new(harness.joiner.peer_id.clone(), harness.joiner.pop.clone())
                .into();
        submit_instruction_with_retry(&harness.primary_client, &register, READY_TIMEOUT).await?;
        if let Err(err) = wait_for_peer_presence(
            &harness.primary_client,
            &harness.joiner.peer_id,
            true,
            READY_TIMEOUT,
        )
        .await
        {
            eprintln!("joiner registration propagation lagged: {err:?}");
        }
    }
    harness.localnet.start_joiner(&harness.joiner.config_path)?;
    wait_for_status_ready(&harness.joiner.client, READY_TIMEOUT).await?;
    let catchup_target = baseline.saturating_sub(INTERIM_CONVERGENCE_MAX_SKEW);
    let catchup_timeout = Duration::from_secs(JOINER_CATCHUP_TIMEOUT_SECS);
    match wait_for_height_or_progress(&harness.joiner.client, catchup_target, catchup_timeout).await
    {
        Ok((start, current)) => match assess_joiner_catchup(start, current, catchup_target) {
            JoinerCatchupAssessment::ReachedTarget => {}
            JoinerCatchupAssessment::Progressed => {
                eprintln!(
                    "joiner catch-up is still below validator target after observed progress: baseline={baseline}, target={catchup_target}, start={start}, current={current}"
                );
            }
            JoinerCatchupAssessment::Stalled => {}
        },
        Err(err) => {
            eprintln!(
                "joiner catch-up stalled after registration: baseline={baseline}, target={catchup_target}, err={err:?}"
            );
        }
    }
    let convergence_target =
        validator_max_height_with_retry(&harness.validator_clients, READY_TIMEOUT).await?;
    if let Err(err) = wait_for_cluster_convergence(
        &harness.validator_clients,
        convergence_target,
        INTERIM_CONVERGENCE_MAX_SKEW,
        READY_TIMEOUT,
    )
    .await
    {
        eprintln!("membership join convergence lagged: {err:?}");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JoinerCatchupAssessment {
    ReachedTarget,
    Progressed,
    Stalled,
}

fn assess_joiner_catchup(start: u64, current: u64, target: u64) -> JoinerCatchupAssessment {
    if current >= target {
        JoinerCatchupAssessment::ReachedTarget
    } else if current > start {
        JoinerCatchupAssessment::Progressed
    } else {
        JoinerCatchupAssessment::Stalled
    }
}

async fn wait_for_height_or_progress(
    client: &Client,
    target: u64,
    timeout: Duration,
) -> Result<(u64, u64)> {
    let deadline = Instant::now() + timeout;
    let mut start_height = None;
    loop {
        match client.get_status() {
            Ok(status) => {
                let start = *start_height.get_or_insert(status.blocks);
                let current_height = status.blocks;
                if current_height >= target || current_height > start {
                    return Ok((start, current_height));
                }
                ensure!(
                    Instant::now() < deadline,
                    "timed out waiting for joiner catch-up to reach target or show progress: target={target}, start={start}, current={current_height}"
                );
            }
            Err(err) => {
                ensure!(
                    Instant::now() < deadline,
                    "timed out waiting for joiner catch-up to reach target={target}: {err:?}"
                );
            }
        }
        sleep(STATUS_POLL).await;
    }
}

async fn membership_leave_cycle(harness: &mut TestusHarness) -> Result<()> {
    if is_peer_present(&harness.primary_client, &harness.joiner.peer_id)? {
        let unregister: InstructionBox = Unregister::peer(harness.joiner.peer_id.clone()).into();
        submit_instruction_with_retry(&harness.primary_client, &unregister, READY_TIMEOUT).await?;
        if let Err(err) = wait_for_peer_presence(
            &harness.primary_client,
            &harness.joiner.peer_id,
            false,
            READY_TIMEOUT,
        )
        .await
        {
            eprintln!("joiner unregister propagation lagged: {err:?}");
        }
    }
    harness.localnet.stop_joiner()?;
    let convergence_target =
        validator_max_height_with_retry(&harness.validator_clients, READY_TIMEOUT).await?;
    if let Err(err) = wait_for_cluster_convergence(
        &harness.validator_clients,
        convergence_target,
        INTERIM_CONVERGENCE_MAX_SKEW,
        READY_TIMEOUT,
    )
    .await
    {
        eprintln!("membership leave convergence lagged: {err:?}");
    }
    Ok(())
}

async fn setup_testus_harness(out_dir: &Path, seed: &str) -> Result<TestusHarness> {
    let api_ports = alloc_port_block(TESTUS_TOTAL_PORT_SLOTS)?;
    let p2p_ports = alloc_port_block(TESTUS_TOTAL_PORT_SLOTS)?;
    let base_api_port = api_ports.base();
    let base_p2p_port = p2p_ports.base();

    generate_localnet(
        out_dir,
        base_api_port,
        base_p2p_port,
        TESTUS_VALIDATORS,
        seed,
    )?;
    let irohad_bin = Program::Irohad
        .resolve()
        .wrap_err("resolve irohad binary")?;
    let localnet = ManagedLocalnet::start(
        out_dir,
        &irohad_bin,
        TESTUS_VALIDATORS,
        (api_ports, p2p_ports),
    )?;

    let mut primary_client = load_localnet_client(out_dir)?;
    primary_client.transaction_status_timeout = READY_TIMEOUT;
    wait_for_status_ready(&primary_client, READY_TIMEOUT).await?;
    let validator_clients =
        build_validator_clients(&primary_client, base_api_port, TESTUS_VALIDATORS)?;
    for validator_client in &validator_clients {
        wait_for_status_ready(validator_client, READY_TIMEOUT).await?;
    }
    let convergence_target =
        validator_max_height_with_retry(&validator_clients, READY_TIMEOUT).await?;
    wait_for_cluster_convergence(
        &validator_clients,
        convergence_target,
        INTERIM_CONVERGENCE_MAX_SKEW,
        READY_TIMEOUT,
    )
    .await?;

    let joiner_api_port = base_api_port + TESTUS_VALIDATORS;
    let joiner_p2p_port = base_p2p_port + TESTUS_VALIDATORS;
    let joiner = build_joiner_peer(
        out_dir,
        &primary_client,
        &out_dir.join("peer0.toml"),
        joiner_api_port,
        joiner_p2p_port,
    )?;

    Ok(TestusHarness {
        out_dir: out_dir.to_path_buf(),
        localnet,
        primary_client,
        validator_clients,
        joiner,
    })
}

fn build_joiner_peer(
    out_dir: &Path,
    template_client: &Client,
    template_peer_config: &Path,
    api_port: u16,
    p2p_port: u16,
) -> Result<JoinerPeer> {
    let template = fs::read_to_string(template_peer_config)
        .wrap_err_with(|| format!("read template config {}", template_peer_config.display()))?;
    let mut parsed: TomlValue =
        toml::from_str(&template).wrap_err("parse template peer config as TOML")?;
    let root = parsed
        .as_table_mut()
        .ok_or_else(|| eyre!("template config root must be a TOML table"))?;

    let peer_key = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
    let peer_id = PeerId::new(peer_key.public_key().clone());
    let pop = bls_normal_pop_prove(peer_key.private_key()).wrap_err("generate BLS PoP")?;
    root.insert(
        "public_key".into(),
        TomlValue::String(peer_key.public_key().to_string()),
    );
    root.insert(
        "private_key".into(),
        TomlValue::String(ExposedPrivateKey(peer_key.private_key().clone()).to_string()),
    );
    if let Some(trusted_peers_pop) = root
        .get_mut("trusted_peers_pop")
        .and_then(TomlValue::as_array_mut)
    {
        let mut joiner_pop = Table::new();
        joiner_pop.insert(
            "public_key".into(),
            TomlValue::String(peer_key.public_key().to_string()),
        );
        joiner_pop.insert("pop_hex".into(), TomlValue::String(hex::encode(&pop)));
        trusted_peers_pop.push(TomlValue::Table(joiner_pop));
    }

    let stream_key = KeyPair::random();
    let streaming = get_subtable_mut(root, "streaming")?;
    streaming.insert(
        "identity_public_key".into(),
        TomlValue::String(stream_key.public_key().to_string()),
    );
    streaming.insert(
        "identity_private_key".into(),
        TomlValue::String(ExposedPrivateKey(stream_key.private_key().clone()).to_string()),
    );

    let p2p_addr = canonical_loopback_addr(p2p_port);
    let network = get_subtable_mut(root, "network")?;
    network.insert("address".into(), TomlValue::String(p2p_addr.clone()));
    network.insert("public_address".into(), TomlValue::String(p2p_addr));

    let torii = get_subtable_mut(root, "torii")?;
    torii.insert(
        "address".into(),
        TomlValue::String(canonical_loopback_addr(api_port)),
    );

    let storage_root = out_dir.join("storage").join("joiner");
    fs::create_dir_all(&storage_root)
        .wrap_err_with(|| format!("create joiner storage root {}", storage_root.display()))?;

    let kura = get_subtable_mut(root, "kura")?;
    kura.insert(
        "store_dir".into(),
        TomlValue::String(storage_root.join("kura").to_string_lossy().into_owned()),
    );

    let tiered_state = get_subtable_mut(root, "tiered_state")?;
    tiered_state.insert(
        "cold_store_root".into(),
        TomlValue::String(storage_root.join("tiered").to_string_lossy().into_owned()),
    );
    if tiered_state.contains_key("da_store_root") {
        tiered_state.insert(
            "da_store_root".into(),
            TomlValue::String(storage_root.join("da").to_string_lossy().into_owned()),
        );
    }

    let config_path = out_dir.join("joiner.toml");
    fs::write(
        &config_path,
        toml::to_string(&parsed).expect("serialize joiner TOML"),
    )
    .wrap_err_with(|| format!("write joiner config {}", config_path.display()))?;

    let client = build_client_for_port(template_client, api_port)?;
    Ok(JoinerPeer {
        peer_id,
        pop,
        config_path,
        client,
    })
}

fn get_subtable_mut<'a>(root: &'a mut Table, key: &str) -> Result<&'a mut Table> {
    root.get_mut(key)
        .and_then(TomlValue::as_table_mut)
        .ok_or_else(|| eyre!("missing `{key}` table in peer config"))
}

fn canonical_loopback_addr(port: u16) -> String {
    IrohaSocketAddr::from(StdSocketAddr::from(([127, 0, 0, 1], port))).to_literal()
}

fn build_validator_clients(
    template: &Client,
    base_api_port: u16,
    count: u16,
) -> Result<Vec<Client>> {
    (0..count)
        .map(|idx| build_client_for_port(template, base_api_port + idx))
        .collect()
}

fn build_client_for_port(template: &Client, api_port: u16) -> Result<Client> {
    let mut client = template.clone();
    client.torii_url = format!("http://127.0.0.1:{api_port}/")
        .parse()
        .wrap_err("parse torii URL")?;
    client.torii_request_timeout = TORII_REQUEST_TIMEOUT;
    client.transaction_status_timeout = READY_TIMEOUT;
    Ok(client)
}

fn collect_statuses(clients: &[Client]) -> Result<Vec<iroha::client::Status>> {
    clients
        .iter()
        .map(Client::get_status)
        .collect::<Result<Vec<_>>>()
}

fn max_height(statuses: &[iroha::client::Status]) -> u64 {
    statuses.iter().map(|s| s.blocks).max().unwrap_or(0)
}

fn min_height(statuses: &[iroha::client::Status]) -> u64 {
    statuses.iter().map(|s| s.blocks).min().unwrap_or(0)
}

fn max_view_changes(statuses: &[iroha::client::Status]) -> u64 {
    statuses
        .iter()
        .map(|s| u64::from(s.view_changes))
        .max()
        .unwrap_or(0)
}

fn update_skew_breach_started(
    current: Option<Instant>,
    observed_skew: u64,
    max_skew: u64,
    now: Instant,
) -> Option<Instant> {
    if observed_skew > max_skew {
        Some(current.unwrap_or(now))
    } else {
        None
    }
}

fn is_skew_breach_unrecovering(
    breach_duration: Duration,
    min_progress_age: Duration,
    grace: Duration,
    min_progress_timeout: Duration,
) -> bool {
    breach_duration > grace && min_progress_age > min_progress_timeout
}

fn is_queue_timeout_error(err: &eyre::Report) -> bool {
    err.chain()
        .any(|cause| cause.to_string().contains("queued for too long"))
}

fn is_http_timeout_error(err: &eyre::Report) -> bool {
    err.chain()
        .any(|cause| cause.to_string().contains("operation timed out"))
}

#[test]
fn joiner_catchup_assessment_reached_target_when_current_at_or_above_target() {
    assert_eq!(
        assess_joiner_catchup(10, 25, 20),
        JoinerCatchupAssessment::ReachedTarget
    );
}

#[test]
fn joiner_catchup_assessment_progressed_when_current_above_start_but_below_target() {
    assert_eq!(
        assess_joiner_catchup(10, 12, 20),
        JoinerCatchupAssessment::Progressed
    );
}

#[test]
fn joiner_catchup_assessment_stalled_when_no_progress_and_below_target() {
    assert_eq!(
        assess_joiner_catchup(10, 10, 20),
        JoinerCatchupAssessment::Stalled
    );
}

#[test]
fn validator_restart_catchup_target_subtracts_interim_skew() {
    assert_eq!(validator_restart_catchup_target(31), 25);
}

#[test]
fn validator_restart_catchup_target_saturates_at_zero() {
    assert_eq!(validator_restart_catchup_target(3), 0);
}

async fn submit_instruction_with_retry(
    client: &Client,
    instruction: &InstructionBox,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match client.submit::<InstructionBox>(instruction.clone()) {
            Ok(_) => return Ok(()),
            Err(err) if is_queue_timeout_error(&err) && Instant::now() < deadline => {
                sleep(STATUS_POLL).await;
            }
            Err(err) => return Err(err).wrap_err("submit instruction"),
        }
    }
}

async fn wait_for_cluster_convergence(
    clients: &[Client],
    min_height_target: u64,
    max_skew: u64,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        let statuses = collect_statuses(clients)?;
        let max_h = max_height(&statuses);
        let min_h = min_height(&statuses);
        if max_h >= min_height_target
            && min_h >= min_height_target.saturating_sub(max_skew)
            && max_h.saturating_sub(min_h) <= max_skew
        {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "cluster failed to converge: target_height={min_height_target}, max={max_h}, min={min_h}, max_skew={max_skew}"
        );
        sleep(STATUS_POLL).await;
    }
}

async fn wait_for_status_ready(client: &Client, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if client.get_status().is_ok() {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "timed out waiting for /status readiness"
        );
        sleep(STATUS_POLL).await;
    }
}

async fn wait_for_height_at_least(client: &Client, target: u64, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match client.get_status() {
            Ok(status) => {
                if status.blocks >= target {
                    return Ok(());
                }
                ensure!(
                    Instant::now() < deadline,
                    "timed out waiting for height {target}, current={}",
                    status.blocks
                );
            }
            Err(err) => {
                ensure!(
                    Instant::now() < deadline,
                    "timed out waiting for height {target}: {err:?}"
                );
            }
        }
        sleep(STATUS_POLL).await;
    }
}

async fn wait_for_blocks_non_empty(client: &Client, target: u64, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match client.get_status() {
            Ok(status) => {
                if status.blocks_non_empty >= target {
                    return Ok(());
                }
                ensure!(
                    Instant::now() < deadline,
                    "timed out waiting for non-empty block target {target}, current={}",
                    status.blocks_non_empty
                );
            }
            Err(err) => {
                ensure!(
                    Instant::now() < deadline,
                    "timed out waiting for non-empty block target {target}: {err:?}"
                );
            }
        }
        sleep(STATUS_POLL).await;
    }
}

async fn wait_for_peer_presence(
    client: &Client,
    peer_id: &PeerId,
    present: bool,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match client.query(FindPeers::new()).execute_all() {
            Ok(peers) => {
                let found = peers.iter().any(|peer| peer == peer_id);
                if found == present {
                    return Ok(());
                }
                ensure!(
                    Instant::now() < deadline,
                    "timed out waiting for peer presence={present} ({peer_id})"
                );
            }
            Err(err) => {
                ensure!(
                    Instant::now() < deadline,
                    "timed out waiting for peer presence={present} ({peer_id}): {err:?}"
                );
            }
        }
        sleep(STATUS_POLL).await;
    }
}

fn first_process_churn_index(validator_count: usize) -> usize {
    if validator_count > 1 { 1 } else { 0 }
}

fn next_process_churn_index(current: usize, validator_count: usize) -> usize {
    if validator_count <= 1 {
        return 0;
    }
    let mut next = (current + 1) % validator_count;
    if next == 0 {
        next = 1;
    }
    next
}

async fn validator_max_height_with_retry(clients: &[Client], timeout: Duration) -> Result<u64> {
    let deadline = Instant::now() + timeout;
    loop {
        match collect_statuses(clients) {
            Ok(statuses) => return Ok(max_height(&statuses)),
            Err(err) => {
                ensure!(
                    Instant::now() < deadline,
                    "timed out collecting validator heights: {err:?}"
                );
                sleep(STATUS_POLL).await;
            }
        }
    }
}

fn is_peer_present(client: &Client, peer_id: &PeerId) -> Result<bool> {
    let peers = client.query(FindPeers::new()).execute_all()?;
    Ok(peers.iter().any(|peer| peer == peer_id))
}

fn write_summary(path: &Path, summary: &SimulationSummary) -> Result<()> {
    let mut rendered = norito::json::to_json_pretty(&summary.to_json_value())
        .wrap_err("serialize summary JSON")?;
    rendered.push('\n');
    fs::write(path, rendered).wrap_err_with(|| format!("write summary {}", path.display()))?;
    Ok(())
}

fn finalize_result(temp_dir: TempDir, context: &str, result: Result<()>) -> Result<()> {
    if let Err(err) = result {
        if let Some(reason) = sandbox::sandbox_reason(&err) {
            eprintln!("sandbox restriction detected while running {context}; skipping ({reason})");
            return Ok(());
        }
        if std::env::var_os("IROHA_TESTUS_KEEP_LOCALNET").is_some() {
            eprintln!(
                "keeping localnet artifacts at {}",
                temp_dir.path().display()
            );
            let _ = temp_dir.keep();
        }
        return Err(err);
    }
    if std::env::var_os("IROHA_TESTUS_KEEP_LOCALNET").is_some() {
        eprintln!(
            "keeping localnet artifacts at {}",
            temp_dir.path().display()
        );
        let _ = temp_dir.keep();
    }
    Ok(())
}

fn env_u64(key: &str, default: u64, min: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|value| *value >= min)
        .unwrap_or(default)
}

fn env_f64(key: &str, default: f64, min: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|raw| raw.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= min)
        .unwrap_or(default)
}

fn localnet_tempdir(label: &str) -> Result<TempDir> {
    let root = repo_root().join("target").join("testus-localnet");
    fs::create_dir_all(&root)
        .wrap_err_with(|| format!("create testus temp root {}", root.display()))?;
    tempfile::Builder::new()
        .prefix(label)
        .tempdir_in(&root)
        .wrap_err("create testus localnet temp dir")
}

fn alloc_port_block(count: u16) -> Result<AllocatedPortBlock> {
    std::panic::catch_unwind(|| AllocatedPortBlock::new(count))
        .map_err(|panic| eyre!(panic_message(&panic)))
}

fn panic_message(panic: &Box<dyn Any + Send>) -> String {
    let panic = panic.as_ref();
    panic.downcast_ref::<&str>().map_or_else(
        || {
            panic
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_else(|| "port allocation panicked".to_owned())
        },
        |message| (*message).to_owned(),
    )
}

fn generate_localnet(
    out_dir: &Path,
    base_api_port: u16,
    base_p2p_port: u16,
    peers: u16,
    seed: &str,
) -> Result<()> {
    let kagami_bin = resolve_kagami_bin()?;
    let output = Command::new(kagami_bin)
        .arg("localnet")
        .arg("--build-line")
        .arg("iroha3")
        .arg("--sora-profile")
        .arg("nexus")
        .arg("--consensus-mode")
        .arg("npos")
        .arg("--peers")
        .arg(peers.to_string())
        .arg("--seed")
        .arg(seed)
        .arg("--bind-host")
        .arg("127.0.0.1")
        .arg("--public-host")
        .arg("127.0.0.1")
        .arg("--base-api-port")
        .arg(base_api_port.to_string())
        .arg("--base-p2p-port")
        .arg(base_p2p_port.to_string())
        .arg("--block-time-ms")
        .arg(LOCALNET_BLOCK_TIME_MS.to_string())
        .arg("--commit-time-ms")
        .arg(LOCALNET_COMMIT_TIME_MS.to_string())
        .arg("--out-dir")
        .arg(out_dir.to_string_lossy().to_string())
        .output()
        .wrap_err("run kagami localnet")?;
    ensure!(
        output.status.success(),
        "kagami localnet failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn resolve_kagami_bin() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("KAGAMI_BIN") {
        return Ok(PathBuf::from(path));
    }
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_kagami") {
        return Ok(PathBuf::from(path));
    }

    let repo = repo_root();
    let target_dir = resolve_target_dir(&repo);
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_owned());
    let bin = bin_name("kagami");
    let mut candidates = vec![
        target_dir.join(format!("{profile}/{bin}")),
        target_dir.join(format!("debug/{bin}")),
        target_dir.join(format!("release/{bin}")),
        repo.join(format!("target/{profile}/{bin}")),
        repo.join(format!("target/debug/{bin}")),
        repo.join(format!("target/release/{bin}")),
    ];

    if let Some(found) = try_candidates(&candidates) {
        return Ok(found);
    }

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut command = Command::new(cargo);
    command
        .current_dir(&repo)
        .arg("build")
        .arg("-p")
        .arg("iroha_kagami")
        .arg("--bin")
        .arg("kagami");
    if profile != "debug" {
        command.arg("--profile").arg(&profile);
    }
    let output = command.output().wrap_err("build kagami binary")?;
    ensure!(
        output.status.success(),
        "failed to build kagami: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    candidates.insert(0, target_dir.join(format!("{profile}/{bin}")));
    try_candidates(&candidates).ok_or_else(|| eyre!("kagami binary not found after build"))
}

fn resolve_target_dir(repo: &Path) -> PathBuf {
    std::env::var("CARGO_TARGET_DIR").map_or_else(
        |_| repo.join("target"),
        |path| {
            let candidate = PathBuf::from(path);
            if candidate.is_absolute() {
                candidate
            } else {
                repo.join(candidate)
            }
        },
    )
}

fn bin_name(raw: &str) -> String {
    if cfg!(windows) {
        format!("{raw}.exe")
    } else {
        raw.to_owned()
    }
}

fn try_candidates(candidates: &[PathBuf]) -> Option<PathBuf> {
    for candidate in candidates {
        if let Ok(path) = candidate.canonicalize() {
            return Some(path);
        }
    }
    None
}

fn load_localnet_client(out_dir: &Path) -> Result<Client> {
    let client_path = out_dir.join("client.toml");
    let mut config = Config::load(LoadPath::Explicit(client_path.clone())).map_err(|err| {
        eyre!(
            "load localnet client config {}: {err:?}",
            client_path.display()
        )
    })?;
    config.torii_request_timeout = TORII_REQUEST_TIMEOUT;
    config.transaction_status_timeout = READY_TIMEOUT;
    Ok(Client::new(config))
}

#[test]
fn simulation_config_defaults_are_valid() {
    let cfg = SimulationConfig::quick(90, 30);
    assert!(cfg.duration >= Duration::from_secs(1));
    assert!(cfg.tps >= 1);
    assert!(cfg.churn_interval >= Duration::from_secs(1));
    assert!(cfg.max_height_skew_grace >= Duration::from_secs(1));
    assert!(cfg.stall_timeout >= Duration::from_secs(10));
}

#[test]
fn env_u64_respects_minimum() {
    assert_eq!(env_u64("IROHA_TESTUS_NO_SUCH_VAR", 10, 2), 10);
}

#[test]
fn env_f64_respects_minimum() {
    assert_eq!(env_f64("IROHA_TESTUS_NO_SUCH_VAR_FLOAT", 0.25, 0.0), 0.25);
}

#[test]
fn skew_breach_window_tracks_first_exceedance_and_recovers() {
    let base = Instant::now();
    let start = update_skew_breach_started(None, 3, 2, base).expect("breach should start");
    assert_eq!(start, base);

    let sustained = update_skew_breach_started(Some(start), 4, 2, base + Duration::from_secs(2))
        .expect("breach should stay active");
    assert_eq!(sustained, start);

    let recovered =
        update_skew_breach_started(Some(sustained), 2, 2, base + Duration::from_secs(3));
    assert!(recovered.is_none());
}

#[test]
fn skew_breach_is_not_unrecovering_when_min_height_progresses_recently() {
    assert!(!is_skew_breach_unrecovering(
        Duration::from_secs(20),
        Duration::from_secs(5),
        Duration::from_secs(15),
        Duration::from_secs(60),
    ));
}

#[test]
fn skew_breach_is_unrecovering_when_duration_and_min_age_exceed_thresholds() {
    assert!(is_skew_breach_unrecovering(
        Duration::from_secs(40),
        Duration::from_secs(61),
        Duration::from_secs(15),
        Duration::from_secs(60),
    ));
}

#[test]
fn queue_timeout_error_classifier_matches_expected_message() {
    let err = eyre!("transaction queued for too long");
    assert!(is_queue_timeout_error(&err));
}

#[test]
fn http_timeout_error_classifier_matches_expected_message() {
    let err = eyre!("operation timed out");
    assert!(is_http_timeout_error(&err));
}

#[test]
fn process_churn_index_skips_primary_when_multiple_validators() {
    assert_eq!(first_process_churn_index(7), 1);
    assert_eq!(next_process_churn_index(1, 7), 2);
    assert_eq!(next_process_churn_index(5, 7), 6);
    assert_eq!(next_process_churn_index(6, 7), 1);
}

#[test]
fn process_churn_index_handles_single_validator() {
    assert_eq!(first_process_churn_index(1), 0);
    assert_eq!(next_process_churn_index(0, 1), 0);
}
