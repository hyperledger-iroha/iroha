#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Taira-profile localnet soak with fixed load, packet impairment, and validator churn.
use eyre::{Result, WrapErr, ensure, eyre};
use integration_tests::{kagami::resolve_kagami_bin, process as test_process, sandbox};
use iroha::{
    client::Client,
    config::{Config, LoadPath},
    crypto::{
        Algorithm, ExposedPrivateKey, Hash, KeyPair, PrivateKey, PublicKey, bls_normal_pop_prove,
    },
    data_model::{
        Level,
        block::consensus_v2::{SumeragiV2LivenessBlocker, SumeragiV2Status},
        isi::{InstructionBox, Log, Unregister, register::RegisterPeerWithPop},
        peer::PeerId,
        prelude::AccountId,
        prelude::QueryBuilderExt,
        query::peer::prelude::FindPeers,
    },
};
use iroha_primitives::addr::SocketAddr as IrohaSocketAddr;
use iroha_test_network::{
    Program, fslock_ports::AllocatedPortBlock, init_instruction_registry, repo_root,
};
use std::{
    any::Any,
    cmp::Reverse,
    collections::BTreeSet,
    fs,
    net::SocketAddr as StdSocketAddr,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;
use tokio::time::sleep;
use toml::{Table, Value as TomlValue};
const TAIRA_VALIDATORS: u16 = 4;
const TAIRA_TOTAL_PORT_SLOTS: u16 = TAIRA_VALIDATORS + 1;
const READY_TIMEOUT: Duration = Duration::from_secs(300);
const STATUS_POLL: Duration = Duration::from_millis(200);
const MONITOR_PERIOD: Duration = Duration::from_secs(1);
const DEFAULT_STALL_TIMEOUT_SECS: u64 = 300;
const DEFAULT_SIM_DURATION_SECS: u64 = 24 * 60 * 60;
const DEFAULT_LOAD_TPS: u64 = 5;
const DEFAULT_CHURN_INTERVAL_SECS: u64 = 300;
const DEFAULT_PACKET_LOSS_PERCENT: u8 = 10;
const DEFAULT_MAX_HEIGHT_SKEW: u64 = 2;
const DEFAULT_MAX_HEIGHT_SKEW_GRACE_SECS: u64 = 30;
const INTERIM_CONVERGENCE_MAX_SKEW: u64 = 6;
// Rejoining validators may briefly trail >10 blocks; this is a guardrail, not a steady-state SLA.
const DEFAULT_MAX_TRANSIENT_HEIGHT_SKEW: u64 = 32;
const DEFAULT_MAX_VIEW_CHANGE_RATE: f64 = 0.2;
const DEFAULT_MAX_LAGGED_CYCLE_RATIO: f64 = 0.35;
const DEFAULT_MIN_COMMITTED_TPS_RATIO: f64 = 0.6;
const MIN_SCHEDULED_TPS_RATIO: f64 = 0.8;
const MIN_ACCEPTED_TPS_RATIO: f64 = 0.55;
const MIN_CHURN_CYCLE_NUMERATOR: u64 = 9;
const MIN_CHURN_CYCLE_DENOMINATOR: u64 = 10;
const MAX_CHURN_PAUSED_RATIO: f64 = 0.25;
const MAX_SOAK_OVERRUN_SECS: u64 = 15 * 60;
const PROCESS_DOWNTIME_SECS: u64 = 5;
const JOINER_CATCHUP_TIMEOUT_SECS: u64 = 60;
const RESTART_CATCHUP_TIMEOUT_SECS: u64 = 60;
const INTERIM_CONVERGENCE_TIMEOUT_SECS: u64 = 45;
const INTERIM_LAG_CHURN_BACKOFF_SECS: u64 = 30;
const JOINER_STALL_LOG_EVERY: u64 = 5;
const JOINER_PROGRESS_LOG_EVERY: u64 = 5;
const JOINER_STALL_WARNING_THRESHOLD: u64 = 3;
const LOCALNET_BLOCK_TIME_MS: u64 = 4_000;
const LOCALNET_COMMIT_TIME_MS: u64 = 4_000;
const LOCALNET_TRANSACTION_TTL_MS: i64 = 7_200_000;
const MAX_TX_BURST_PER_TICK: u32 = 32;
const TORII_REQUEST_TIMEOUT: Duration = Duration::from_secs(2);
const STATUS_REQUEST_RETRIES: usize = 2;
const STATUS_REQUEST_RETRY_BACKOFF: Duration = Duration::from_millis(200);
const PEER_QUERY_REQUEST_RETRIES: usize = 2;
const PEER_QUERY_REQUEST_RETRY_BACKOFF: Duration = Duration::from_millis(200);
const STATUS_QUORUM_RETRY_TIMEOUT: Duration = Duration::from_secs(15);
const FINAL_SETTLE_WINDOW: Duration = Duration::from_secs(120);
const LOAD_SUBMIT_RETRY_TIMEOUT_SECS: u64 = 3;
const LOAD_SUBMIT_RETRY_BACKOFF: Duration = Duration::from_millis(200);
const LOG_TAIL_LINES: usize = 80;
#[derive(Clone, Copy)]
struct SimulationModes {
    process_churn: bool,
    membership_churn: bool,
}
#[derive(Clone, Debug)]
struct ReleaseExecutionProfile {
    build_profile: String,
    cargo_net_offline: bool,
}
#[derive(Clone, Copy)]
struct SimulationConfig {
    duration: Duration,
    tps: u64,
    packet_loss_percent: u8,
    churn_interval: Duration,
    max_height_skew: u64,
    max_height_skew_grace: Duration,
    max_transient_height_skew: u64,
    stall_timeout: Duration,
    max_view_change_rate: f64,
    max_lagged_cycle_ratio: f64,
    min_committed_tps_ratio: f64,
    process_downtime: Duration,
}
impl SimulationConfig {
    fn from_env() -> Self {
        Self {
            duration: Duration::from_secs(env_u64(
                "IROHA_TAIRA_SIM_DURATION_SECS",
                DEFAULT_SIM_DURATION_SECS,
                30,
            )),
            tps: env_u64("IROHA_TAIRA_LOAD_TPS", DEFAULT_LOAD_TPS, 1),
            packet_loss_percent: env_u8(
                "IROHA_TAIRA_PACKET_LOSS_PERCENT",
                DEFAULT_PACKET_LOSS_PERCENT,
                0,
                100,
            ),
            churn_interval: Duration::from_secs(env_u64(
                "IROHA_TAIRA_CHURN_INTERVAL_SECS",
                DEFAULT_CHURN_INTERVAL_SECS,
                30,
            )),
            max_height_skew: env_u64("IROHA_TAIRA_MAX_HEIGHT_SKEW", DEFAULT_MAX_HEIGHT_SKEW, 0),
            max_height_skew_grace: Duration::from_secs(env_u64(
                "IROHA_TAIRA_MAX_HEIGHT_SKEW_GRACE_SECS",
                DEFAULT_MAX_HEIGHT_SKEW_GRACE_SECS,
                0,
            )),
            max_transient_height_skew: env_u64(
                "IROHA_TAIRA_MAX_TRANSIENT_HEIGHT_SKEW",
                DEFAULT_MAX_TRANSIENT_HEIGHT_SKEW,
                0,
            ),
            stall_timeout: Duration::from_secs(env_u64(
                "IROHA_TAIRA_STALL_TIMEOUT_SECS",
                DEFAULT_STALL_TIMEOUT_SECS,
                10,
            )),
            max_view_change_rate: env_f64(
                "IROHA_TAIRA_MAX_VIEW_CHANGE_RATE",
                DEFAULT_MAX_VIEW_CHANGE_RATE,
                0.0,
            ),
            max_lagged_cycle_ratio: env_f64(
                "IROHA_TAIRA_MAX_LAGGED_CYCLE_RATIO",
                DEFAULT_MAX_LAGGED_CYCLE_RATIO,
                0.0,
            ),
            min_committed_tps_ratio: env_f64(
                "IROHA_TAIRA_MIN_COMMITTED_TPS_RATIO",
                DEFAULT_MIN_COMMITTED_TPS_RATIO,
                0.0,
            ),
            process_downtime: Duration::from_secs(PROCESS_DOWNTIME_SECS),
        }
    }
    fn quick(duration_secs: u64, churn_interval_secs: u64) -> Self {
        Self {
            duration: Duration::from_secs(duration_secs),
            tps: DEFAULT_LOAD_TPS,
            packet_loss_percent: 0,
            churn_interval: Duration::from_secs(churn_interval_secs),
            max_height_skew: DEFAULT_MAX_HEIGHT_SKEW,
            max_height_skew_grace: Duration::from_secs(DEFAULT_MAX_HEIGHT_SKEW_GRACE_SECS),
            max_transient_height_skew: DEFAULT_MAX_TRANSIENT_HEIGHT_SKEW,
            stall_timeout: Duration::from_secs(DEFAULT_STALL_TIMEOUT_SECS),
            max_view_change_rate: DEFAULT_MAX_VIEW_CHANGE_RATE,
            max_lagged_cycle_ratio: DEFAULT_MAX_LAGGED_CYCLE_RATIO,
            min_committed_tps_ratio: DEFAULT_MIN_COMMITTED_TPS_RATIO,
            process_downtime: Duration::from_secs(2),
        }
    }
}
#[derive(Clone, Debug)]
struct SimulationSummary {
    git_revision: String,
    workspace_source_manifest_sha256: String,
    build_profile: String,
    cargo_net_offline: bool,
    localnet_artifact_path: String,
    daemon_binary_path: String,
    daemon_binary_blake2b_256: String,
    kagami_binary_path: String,
    kagami_binary_blake2b_256: String,
    test_binary_path: String,
    test_binary_blake2b_256: String,
    generated_config_blake2b_256: String,
    seed: String,
    duration_secs: u64,
    target_tps: u64,
    packet_loss_percent: u8,
    churn_interval_secs: u64,
    max_height_skew: u64,
    max_height_skew_grace_secs: u64,
    max_transient_height_skew: u64,
    stall_timeout_secs: u64,
    max_view_change_rate: f64,
    max_lagged_cycle_ratio: f64,
    min_committed_tps_ratio: f64,
    process_downtime_secs: u64,
    tx_attempted: u64,
    tx_sent: u64,
    tx_submit_errors: u64,
    process_churn_cycles: u64,
    expected_process_churn_cycles: u64,
    process_churn_lagged_cycles: u64,
    membership_join_cycles: u64,
    membership_leave_cycles: u64,
    expected_membership_churn_cycles: u64,
    membership_cleanup_leave: bool,
    membership_churn_lagged_cycles: u64,
    membership_churn_warning_cycles: u64,
    churn_paused_secs: f64,
    churn_paused_ratio: f64,
    soak_overrun_secs: f64,
    max_height_skew_observed: u64,
    view_changes_start: u64,
    view_changes_end: u64,
    view_change_rate_per_sec: f64,
    scheduled_tps: f64,
    submitted_tps: f64,
    committed_tps: f64,
    committed_txs_min_delta: u64,
    saturated_samples: u64,
    total_samples: u64,
    initial_status_snapshots: Vec<norito::json::Value>,
    final_status_snapshots: Vec<norito::json::Value>,
    no_progress_intervals: Vec<NoProgressInterval>,
    unclassified_no_progress_intervals: u64,
}
impl SimulationSummary {
    fn to_json_value(&self) -> norito::json::Value {
        norito::json!({
            "git_revision": (self.git_revision.clone()),
            "workspace_source_manifest_sha256": (self.workspace_source_manifest_sha256.clone()),
            "build_profile": (self.build_profile.clone()),
            "cargo_net_offline": (self.cargo_net_offline),
            "localnet_artifact_path": (self.localnet_artifact_path.clone()),
            "daemon_binary_path": (self.daemon_binary_path.clone()),
            "daemon_binary_blake2b_256": (self.daemon_binary_blake2b_256.clone()),
            "kagami_binary_path": (self.kagami_binary_path.clone()),
            "kagami_binary_blake2b_256": (self.kagami_binary_blake2b_256.clone()),
            "test_binary_path": (self.test_binary_path.clone()),
            "test_binary_blake2b_256": (self.test_binary_blake2b_256.clone()),
            "generated_config_blake2b_256": (self.generated_config_blake2b_256.clone()),
            "seed": (self.seed.clone()),
            "duration_secs": (self.duration_secs),
            "target_tps": (self.target_tps),
            "packet_loss_percent": (self.packet_loss_percent),
            "churn_interval_secs": (self.churn_interval_secs),
            "max_height_skew": (self.max_height_skew),
            "max_height_skew_grace_secs": (self.max_height_skew_grace_secs),
            "max_transient_height_skew": (self.max_transient_height_skew),
            "stall_timeout_secs": (self.stall_timeout_secs),
            "max_view_change_rate": (self.max_view_change_rate),
            "max_lagged_cycle_ratio": (self.max_lagged_cycle_ratio),
            "min_committed_tps_ratio": (self.min_committed_tps_ratio),
            "process_downtime_secs": (self.process_downtime_secs),
            "tx_attempted": (self.tx_attempted),
            "tx_sent": (self.tx_sent),
            "tx_submit_errors": (self.tx_submit_errors),
            "process_churn_cycles": (self.process_churn_cycles),
            "expected_process_churn_cycles": (self.expected_process_churn_cycles),
            "process_churn_lagged_cycles": (self.process_churn_lagged_cycles),
            "membership_join_cycles": (self.membership_join_cycles),
            "membership_leave_cycles": (self.membership_leave_cycles),
            "expected_membership_churn_cycles": (self.expected_membership_churn_cycles),
            "membership_cleanup_leave": (self.membership_cleanup_leave),
            "membership_churn_lagged_cycles": (self.membership_churn_lagged_cycles),
            "membership_churn_warning_cycles": (self.membership_churn_warning_cycles),
            "churn_paused_secs": (self.churn_paused_secs),
            "churn_paused_ratio": (self.churn_paused_ratio),
            "soak_overrun_secs": (self.soak_overrun_secs),
            "max_height_skew_observed": (self.max_height_skew_observed),
            "view_changes_start": (self.view_changes_start),
            "view_changes_end": (self.view_changes_end),
            "view_change_rate_per_sec": (self.view_change_rate_per_sec),
            "scheduled_tps": (self.scheduled_tps),
            "submitted_tps": (self.submitted_tps),
            "committed_tps": (self.committed_tps),
            "committed_txs_min_delta": (self.committed_txs_min_delta),
            "saturated_samples": (self.saturated_samples),
            "total_samples": (self.total_samples),
            "initial_status_snapshots": (self.initial_status_snapshots.clone()),
            "final_status_snapshots": (self.final_status_snapshots.clone()),
            "no_progress_intervals": (self.no_progress_intervals.iter().map(NoProgressInterval::to_json_value).collect::<Vec<_>>()),
            "unclassified_no_progress_intervals": (self.unclassified_no_progress_intervals),
        })
    }
}
#[derive(Clone, Debug)]
struct NoProgressInterval {
    start_elapsed_ms: u64,
    end_elapsed_ms: u64,
    classifications: Vec<String>,
    classified: bool,
    status_snapshots: Vec<norito::json::Value>,
}
impl NoProgressInterval {
    fn to_json_value(&self) -> norito::json::Value {
        norito::json!({
            "start_elapsed_ms": (self.start_elapsed_ms),
            "end_elapsed_ms": (self.end_elapsed_ms),
            "classifications": (self.classifications.clone()),
            "classified": (self.classified),
            "status_snapshots": (self.status_snapshots.clone()),
        })
    }
}
#[derive(Debug)]
struct ActiveNoProgressInterval {
    start_elapsed_ms: u64,
    classifications: BTreeSet<String>,
    classified: bool,
    status_snapshots: Vec<norito::json::Value>,
}
impl ActiveNoProgressInterval {
    fn finish(self, end_elapsed_ms: u64) -> NoProgressInterval {
        NoProgressInterval {
            start_elapsed_ms: self.start_elapsed_ms,
            end_elapsed_ms,
            classifications: self.classifications.into_iter().collect(),
            classified: self.classified,
            status_snapshots: self.status_snapshots,
        }
    }
}
#[derive(Debug)]
struct LivenessObservation {
    classifications: BTreeSet<String>,
    classified: bool,
    status_snapshots: Vec<norito::json::Value>,
}
fn blocker_label(blocker: SumeragiV2LivenessBlocker) -> &'static str {
    match blocker {
        SumeragiV2LivenessBlocker::MissingProposal => "missing_proposal",
        SumeragiV2LivenessBlocker::BodyUnavailable => "body_unavailable",
        SumeragiV2LivenessBlocker::PrepareQuorumMissing => "prepare_quorum_missing",
        SumeragiV2LivenessBlocker::CommitQuorumMissing => "commit_quorum_missing",
        SumeragiV2LivenessBlocker::TimeoutCertificateMissing => "timeout_certificate_missing",
        SumeragiV2LivenessBlocker::SchedulerStarvation => "scheduler_starvation",
        SumeragiV2LivenessBlocker::ApplicationPending => "application_pending",
        SumeragiV2LivenessBlocker::SuccessorActivationPending => "successor_activation_pending",
        SumeragiV2LivenessBlocker::LocalControlPending => "local_control_pending",
    }
}
fn observe_liveness(clients: &[Client], required_responsive: usize) -> LivenessObservation {
    let mut classifications = BTreeSet::new();
    let mut statuses = Vec::<(usize, SumeragiV2Status)>::new();
    let mut all_classified = true;
    for (validator_index, client) in clients.iter().enumerate() {
        let Ok(status) = client.get_sumeragi_status() else {
            continue;
        };
        if let Some(blocker) = status.liveness.blocker {
            classifications.insert(blocker_label(blocker).to_owned());
        } else {
            all_classified = false;
        }
        statuses.push((validator_index, status));
    }
    let classified = statuses.len() >= required_responsive && all_classified;
    let status_snapshots = statuses
        .iter()
        .filter_map(|(validator_index, status)| {
            norito::json::to_value(status)
                .ok()
                .map(|status| status_snapshot_value(*validator_index, status))
        })
        .collect();
    LivenessObservation {
        classifications,
        classified,
        status_snapshots,
    }
}
fn status_snapshot_value(
    validator_index: usize,
    status: norito::json::Value,
) -> norito::json::Value {
    norito::json!({
        "validator_index": (validator_index),
        "status": (status),
    })
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct MembershipCycleOutcome {
    hard_lagged: bool,
    warning_lagged: bool,
}
impl MembershipCycleOutcome {
    fn mark_hard_lag(&mut self) {
        self.hard_lagged = true;
    }
    fn mark_warning_lag(&mut self) {
        self.warning_lagged = true;
    }
}
struct JoinerPeer {
    peer_id: PeerId,
    pop: Vec<u8>,
    config_path: PathBuf,
    client: Client,
}
struct GeneratedLocalnetEvidence {
    kagami_binary_path: PathBuf,
    kagami_binary_blake2b_256: String,
}
struct TairaHarness {
    out_dir: PathBuf,
    seed: String,
    git_revision: String,
    daemon_binary_path: PathBuf,
    daemon_binary_blake2b_256: String,
    kagami_binary_path: PathBuf,
    kagami_binary_blake2b_256: String,
    test_binary_path: PathBuf,
    test_binary_blake2b_256: String,
    generated_config_blake2b_256: String,
    localnet: ManagedLocalnet,
    primary_client: Client,
    validator_clients: Vec<Client>,
    joiner: JoinerPeer,
}
impl TairaHarness {
    fn summary_path(&self) -> PathBuf {
        self.out_dir.join("taira_simulation_summary.json")
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
            .wrap_err_with(|| format!("spawn iroha3d for {node_label}"))
    }
    fn unexpected_validator_exit_report(&mut self) -> Result<Option<String>> {
        for (idx, child) in self.validator_children.iter_mut().enumerate() {
            let Some(child) = child.as_mut() else {
                continue;
            };
            let Some(status) = child
                .try_wait()
                .wrap_err_with(|| format!("poll taira validator {idx}"))?
            else {
                continue;
            };
            let log_path = self.dir.join(format!("peer{idx}.log"));
            let tail = log_tail(&log_path, LOG_TAIL_LINES);
            return Ok(Some(format!(
                "taira validator {idx} exited before readiness quorum: status={status}; log tail from {}:\n{tail}",
                log_path.display()
            )));
        }
        Ok(None)
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
                let mut command = Command::new("bash");
                command.arg(script).current_dir(&self.dir);
                let _ = test_process::output_with_timeout(
                    &mut command,
                    test_process::process_timeout(),
                );
            }
        }
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn taira_localnet_bootstrap_validators() -> Result<()> {
    init_instruction_registry();
    let _guard = sandbox::serial_guard();
    let temp_dir = localnet_tempdir("taira-bootstrap")?;
    let out_dir = temp_dir.path().join("localnet");
    let result: Result<()> = async {
        let harness = setup_taira_harness::<false>(&out_dir, "taira-bootstrap", 0).await?;
        wait_for_cluster_convergence(
            &harness.validator_clients,
            harness.primary_client.get_status()?.blocks,
            DEFAULT_MAX_HEIGHT_SKEW,
            READY_TIMEOUT,
        )
        .await?;
        let baseline = harness.primary_client.get_status()?.blocks_non_empty;
        harness.primary_client.submit::<InstructionBox>(
            Log::new(Level::INFO, "taira bootstrap probe".to_string()).into(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
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
    finalize_result(temp_dir, "taira_localnet_bootstrap_validators", result)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires local process orchestration"]
async fn taira_localnet_joiner_register_unregister_behavior() -> Result<()> {
    init_instruction_registry();
    let _guard = sandbox::serial_guard();
    let temp_dir = localnet_tempdir("taira-membership")?;
    let out_dir = temp_dir.path().join("localnet");
    let result: Result<()> = async {
        let mut harness = setup_taira_harness::<false>(&out_dir, "taira-membership", 0).await?;
        let mut joiner_warning_state = JoinerCatchupWarningState::default();
        let _ = membership_join_cycle(&mut harness, &mut joiner_warning_state).await?;
        let _ = membership_leave_cycle(&mut harness).await?;
        Ok(())
    }
    .await;
    finalize_result(
        temp_dir,
        "taira_localnet_joiner_register_unregister_behavior",
        result,
    )
}
#[path = "taira_public_localnet/strict_restart.rs"]
mod strict_restart;
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "24-hour Taira-profile soak with validator restarts and packet impairment"]
async fn taira_profile_24h_packet_impairment_and_restart_soak() -> Result<()> {
    init_instruction_registry();
    let _guard = sandbox::serial_guard();
    let cfg = SimulationConfig::from_env();
    let workspace_source_manifest_sha256 = required_release_source_manifest_sha256()?;
    let evidence_path = required_release_evidence_path()?;
    let release_execution_profile = required_release_execution_profile()?;
    let seed = std::env::var("IROHA_TAIRA_SIM_SEED")
        .ok()
        .filter(|seed| !seed.trim().is_empty())
        .unwrap_or_else(|| "taira-public-sim".to_owned());
    let temp_dir = localnet_tempdir("taira-simulation")?;
    let out_dir = temp_dir.path().join("localnet");
    let result: Result<()> = async move {
        let mut harness =
            setup_taira_harness::<false>(&out_dir, &seed, cfg.packet_loss_percent).await?;
        let summary = run_taira_simulation(
            &mut harness,
            cfg,
            SimulationModes {
                process_churn: true,
                membership_churn: true,
            },
            &workspace_source_manifest_sha256,
            &release_execution_profile,
        )
        .await?;
        write_summary(&harness.summary_path(), &evidence_path, &summary)?;
        Ok(())
    }
    .await;
    finalize_result(
        temp_dir,
        "taira_profile_24h_packet_impairment_and_restart_soak",
        result,
    )
}
async fn run_taira_simulation(
    harness: &mut TairaHarness,
    cfg: SimulationConfig,
    modes: SimulationModes,
    workspace_source_manifest_sha256: &str,
    release_execution_profile: &ReleaseExecutionProfile,
) -> Result<SimulationSummary> {
    ensure!(cfg.tps > 0, "tps must be greater than zero");
    ensure!(
        (0.0..=1.0).contains(&cfg.max_lagged_cycle_ratio),
        "max lagged cycle ratio must be in [0,1], got {}",
        cfg.max_lagged_cycle_ratio
    );
    ensure!(
        (0.0..=1.0).contains(&cfg.min_committed_tps_ratio),
        "min committed tps ratio must be in [0,1], got {}",
        cfg.min_committed_tps_ratio
    );
    ensure!(
        cfg.max_transient_height_skew >= cfg.max_height_skew,
        "max transient height skew must be >= max height skew (transient={}, steady={})",
        cfg.max_transient_height_skew,
        cfg.max_height_skew
    );
    let validator_quorum = min_presence_matches(harness.validator_clients.len());
    let mut tx_attempted = 0_u64;
    let mut tx_sent = 0_u64;
    let mut tx_submit_errors = 0_u64;
    let mut process_churn_cycles = 0_u64;
    let mut process_churn_lagged_cycles = 0_u64;
    let mut membership_join_cycles = 0_u64;
    let mut membership_leave_cycles = 0_u64;
    let mut membership_churn_lagged_cycles = 0_u64;
    let mut membership_churn_warning_cycles = 0_u64;
    let mut membership_cleanup_leave = false;
    let mut max_height_skew_observed = 0_u64;
    let mut saturated_samples = 0_u64;
    let mut total_samples = 0_u64;
    let mut joiner_active = false;
    let mut joiner_warning_state = JoinerCatchupWarningState::default();
    let mut restart_idx = first_process_churn_index(harness.validator_clients.len());
    let mut paused_for_churn = Duration::ZERO;
    let mut skew_breach_started_at = None;
    let initial_status_observations = collect_indexed_statuses_quorum_with_retry(
        &harness.validator_clients,
        validator_quorum,
        STATUS_QUORUM_RETRY_TIMEOUT,
    )
    .await?;
    let initial_statuses = top_quorum_statuses(
        &statuses_from_indexed(&initial_status_observations),
        validator_quorum,
    );
    let view_changes_start = total_indexed_view_changes(&initial_status_observations);
    let mut view_change_tracker = ViewChangeTracker::new(harness.validator_clients.len());
    view_change_tracker.establish_baseline(&initial_status_observations);
    let initial_status_snapshots =
        observe_liveness(&harness.validator_clients, validator_quorum).status_snapshots;
    ensure!(
        initial_status_snapshots.len() >= validator_quorum,
        "failed to capture an authoritative Sumeragi v2 status quorum before the soak: captured={}, required={validator_quorum}",
        initial_status_snapshots.len()
    );
    let initial_min_txs_approved = min_txs_approved(&initial_statuses);
    let mut last_progress_height = max_height(&initial_statuses);
    let mut last_progress_at = Instant::now();
    let mut last_min_progress_height = min_height(&initial_statuses);
    let mut last_min_progress_at = Instant::now();
    let mut no_progress_intervals = Vec::new();
    let mut active_no_progress_interval: Option<ActiveNoProgressInterval> = None;
    let classification_delay = cfg.stall_timeout / 2;
    let mut next_tx = Instant::now();
    let mut next_monitor = Instant::now();
    let final_settle_window = effective_final_settle_window(cfg.duration);
    let churn_window = cfg.duration.saturating_sub(final_settle_window);
    let process_initial_delay = initial_churn_delay(cfg.churn_interval, churn_window);
    let membership_offset = initial_churn_delay(cfg.churn_interval / 2, churn_window);
    let expected_process_churn_cycles =
        scheduled_churn_cycles(churn_window, process_initial_delay, cfg.churn_interval);
    let expected_membership_churn_cycles =
        scheduled_churn_cycles(churn_window, membership_offset, cfg.churn_interval);
    let mut next_process_churn = Instant::now() + process_initial_delay;
    let mut next_membership_churn = Instant::now() + membership_offset;
    let tx_period = Duration::from_secs_f64(1.0 / cfg.tps as f64);
    let start_time = Instant::now();
    let deadline = start_time + cfg.duration;
    while Instant::now() < deadline {
        let now = Instant::now();
        let allow_churn = deadline.saturating_duration_since(now) > final_settle_window;
        if modes.process_churn && allow_churn && now >= next_process_churn {
            let churn_start = Instant::now();
            ensure!(
                refresh_primary_client_from_validators(harness),
                "no validator endpoint is reachable before process churn"
            );
            let selected_restart_idx = select_process_churn_index(
                &harness.validator_clients,
                restart_idx,
                INTERIM_CONVERGENCE_MAX_SKEW,
            );
            let statuses_before_restart = collect_indexed_statuses_quorum_with_retry(
                &harness.validator_clients,
                validator_quorum,
                STATUS_QUORUM_RETRY_TIMEOUT,
            )
            .await?;
            view_change_tracker.observe(&statuses_before_restart);
            let lagged =
                process_churn_cycle(harness, selected_restart_idx, cfg.process_downtime).await?;
            paused_for_churn = paused_for_churn.saturating_add(churn_start.elapsed());
            let status_observations_after_churn = collect_indexed_statuses_quorum_with_retry(
                &harness.validator_clients,
                validator_quorum,
                STATUS_QUORUM_RETRY_TIMEOUT,
            )
            .await?;
            view_change_tracker.observe(&status_observations_after_churn);
            let statuses_after_churn = top_quorum_statuses(
                &statuses_from_indexed(&status_observations_after_churn),
                validator_quorum,
            );
            let max_after_churn = max_height(&statuses_after_churn);
            if max_after_churn > last_progress_height {
                last_progress_height = max_after_churn;
                if let Some(interval) = active_no_progress_interval.take() {
                    no_progress_intervals.push(interval.finish(
                        u64::try_from(start_time.elapsed().as_millis()).unwrap_or(u64::MAX),
                    ));
                }
                last_progress_at = Instant::now();
            }
            let min_after_churn = min_height(&statuses_after_churn);
            if min_after_churn > last_min_progress_height {
                last_min_progress_height = min_after_churn;
                last_min_progress_at = Instant::now();
            }
            restart_idx =
                next_process_churn_index(selected_restart_idx, harness.validator_clients.len());
            process_churn_cycles = process_churn_cycles.saturating_add(1);
            if lagged {
                process_churn_lagged_cycles = process_churn_lagged_cycles.saturating_add(1);
                eprintln!(
                    "process churn lagged; applying next-cycle backoff of {}s",
                    INTERIM_LAG_CHURN_BACKOFF_SECS
                );
            }
            next_process_churn =
                next_process_churn_deadline(next_process_churn, cfg.churn_interval, lagged);
            continue;
        }
        if modes.membership_churn && allow_churn && now >= next_membership_churn {
            ensure!(
                refresh_primary_client_from_validators(harness),
                "no validator endpoint is reachable before membership churn"
            );
            let churn_start = Instant::now();
            let outcome = if joiner_active {
                membership_leave_cycle(harness).await?
            } else {
                membership_join_cycle(harness, &mut joiner_warning_state).await?
            };
            if joiner_active {
                membership_leave_cycles = membership_leave_cycles.saturating_add(1);
            } else {
                membership_join_cycles = membership_join_cycles.saturating_add(1);
            }
            paused_for_churn = paused_for_churn.saturating_add(churn_start.elapsed());
            let status_observations_after_churn = collect_indexed_statuses_quorum_with_retry(
                &harness.validator_clients,
                validator_quorum,
                STATUS_QUORUM_RETRY_TIMEOUT,
            )
            .await?;
            view_change_tracker.observe(&status_observations_after_churn);
            let statuses_after_churn = top_quorum_statuses(
                &statuses_from_indexed(&status_observations_after_churn),
                validator_quorum,
            );
            let max_after_churn = max_height(&statuses_after_churn);
            if max_after_churn > last_progress_height {
                last_progress_height = max_after_churn;
                if let Some(interval) = active_no_progress_interval.take() {
                    no_progress_intervals.push(interval.finish(
                        u64::try_from(start_time.elapsed().as_millis()).unwrap_or(u64::MAX),
                    ));
                }
                last_progress_at = Instant::now();
            }
            let min_after_churn = min_height(&statuses_after_churn);
            if min_after_churn > last_min_progress_height {
                last_min_progress_height = min_after_churn;
                last_min_progress_at = Instant::now();
            }
            joiner_active = !joiner_active;
            if outcome.hard_lagged {
                membership_churn_lagged_cycles = membership_churn_lagged_cycles.saturating_add(1);
                eprintln!(
                    "membership churn lagged; applying next-cycle backoff of {}s",
                    INTERIM_LAG_CHURN_BACKOFF_SECS
                );
            }
            if outcome.warning_lagged {
                membership_churn_warning_cycles = membership_churn_warning_cycles.saturating_add(1);
            }
            next_membership_churn = next_membership_churn_deadline(
                next_membership_churn,
                cfg.churn_interval,
                membership_backoff_requires_hard_lag(outcome),
            );
            continue;
        }
        if now >= next_tx {
            let mut burst_submitted = 0_u32;
            let mut catchup_now = now;
            while catchup_now >= next_tx && burst_submitted < MAX_TX_BURST_PER_TICK {
                tx_attempted = tx_attempted.saturating_add(1);
                let msg = format!("taira-load-{tx_attempted}");
                let load_instruction: InstructionBox = Log::new(Level::INFO, msg).into();
                if let Err(err) = submit_load_instruction_with_retry(
                    harness,
                    &load_instruction,
                    Duration::from_secs(LOAD_SUBMIT_RETRY_TIMEOUT_SECS),
                )
                .await
                {
                    tx_submit_errors = tx_submit_errors.saturating_add(1);
                    eprintln!("taira load submit failed after retries: {err:?}");
                    next_tx = Instant::now() + tx_period;
                    break;
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
            let status_observations = collect_indexed_statuses_quorum_with_retry(
                &harness.validator_clients,
                validator_quorum,
                STATUS_QUORUM_RETRY_TIMEOUT,
            )
            .await?;
            view_change_tracker.observe(&status_observations);
            let statuses = top_quorum_statuses(
                &statuses_from_indexed(&status_observations),
                validator_quorum,
            );
            let max_height = max_height(&statuses);
            let min_height = min_height(&statuses);
            let skew = max_height.saturating_sub(min_height);
            max_height_skew_observed = max_height_skew_observed.max(skew);
            ensure!(
                skew <= cfg.max_transient_height_skew,
                "validator height skew exceeded absolute transient cap: observed={skew}, cap={}, max_height={max_height}, min_height={min_height}",
                cfg.max_transient_height_skew,
            );
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
                last_progress_at = now;
                if let Some(interval) = active_no_progress_interval.take() {
                    no_progress_intervals.push(interval.finish(
                        u64::try_from(start_time.elapsed().as_millis()).unwrap_or(u64::MAX),
                    ));
                }
            }
            let no_progress_age = now.saturating_duration_since(last_progress_at);
            if no_progress_age >= classification_delay {
                let observation = observe_liveness(&harness.validator_clients, validator_quorum);
                let elapsed_ms =
                    u64::try_from(start_time.elapsed().as_millis()).unwrap_or(u64::MAX);
                match &mut active_no_progress_interval {
                    Some(interval) => {
                        let changed = interval.classifications != observation.classifications;
                        interval.classifications.extend(observation.classifications);
                        interval.classified &= observation.classified;
                        if changed && interval.status_snapshots.len() < 32 {
                            interval
                                .status_snapshots
                                .extend(observation.status_snapshots);
                            interval.status_snapshots.truncate(32);
                        }
                    }
                    None => {
                        active_no_progress_interval = Some(ActiveNoProgressInterval {
                            start_elapsed_ms: elapsed_ms,
                            classifications: observation.classifications,
                            classified: observation.classified,
                            status_snapshots: observation.status_snapshots,
                        });
                    }
                }
            }
            ensure!(
                no_progress_age <= cfg.stall_timeout,
                "consensus stalled: no max-height progression for {:?} (max_height={max_height}, min_height={min_height}, last_progress_height={last_progress_height}, liveness={active_no_progress_interval:?})",
                cfg.stall_timeout,
            );
            for client in &harness.validator_clients {
                if let Ok(diagnostics) = client.get_sumeragi_diagnostics() {
                    total_samples = total_samples.saturating_add(1);
                    if diagnostics.tx_queue_saturated {
                        saturated_samples = saturated_samples.saturating_add(1);
                    }
                }
                if let Ok(sumeragi) = client.get_sumeragi_status() {
                    ensure!(
                        sumeragi.last_committed_height <= sumeragi.height,
                        "exact reducer status is internally inconsistent: last_committed_height={} height={}",
                        sumeragi.last_committed_height,
                        sumeragi.height
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
    let soak_elapsed = Instant::now().saturating_duration_since(start_time);
    if joiner_active {
        let outcome = membership_leave_cycle(harness).await?;
        membership_cleanup_leave = true;
        if outcome.hard_lagged {
            eprintln!("membership cleanup leave lagged after the scheduled soak window");
        }
        if outcome.warning_lagged {
            eprintln!("membership cleanup leave completed with a catch-up warning");
        }
    }
    let elapsed = soak_elapsed;
    let duration_secs = elapsed.as_secs().max(1);
    let elapsed_secs = elapsed.as_secs_f64().max(1.0);
    let soak_overrun_secs = elapsed.saturating_sub(cfg.duration).as_secs_f64();
    let churn_paused_secs = paused_for_churn.as_secs_f64();
    let churn_paused_ratio = churn_paused_secs / elapsed_secs;
    let scheduled_tps = tx_attempted as f64 / elapsed_secs;
    let submitted_tps = tx_sent as f64 / elapsed_secs;
    let membership_churn_cycles = membership_join_cycles.saturating_add(membership_leave_cycles);
    if modes.process_churn {
        let required_process_churn_cycles =
            minimum_required_churn_cycles(expected_process_churn_cycles);
        ensure!(
            process_churn_cycles >= required_process_churn_cycles,
            "process churn cadence fell below the scheduled release floor: observed={process_churn_cycles}, expected={expected_process_churn_cycles}, required={required_process_churn_cycles}, duration={:?}, churn_interval={:?}, final_settle_window={final_settle_window:?}",
            cfg.duration,
            cfg.churn_interval
        );
    }
    if modes.membership_churn {
        let required_membership_churn_cycles =
            minimum_required_churn_cycles(expected_membership_churn_cycles);
        ensure!(
            membership_churn_cycles >= required_membership_churn_cycles,
            "membership churn cadence fell below the scheduled release floor: observed={membership_churn_cycles}, expected={expected_membership_churn_cycles}, required={required_membership_churn_cycles}, duration={:?}, churn_interval={:?}, final_settle_window={final_settle_window:?}",
            cfg.duration,
            cfg.churn_interval
        );
        ensure!(
            membership_join_cycles > 0,
            "membership join churn did not execute (duration={:?}, churn_interval={:?}, final_settle_window={final_settle_window:?})",
            cfg.duration,
            cfg.churn_interval
        );
        ensure!(
            membership_leave_cycles > 0,
            "membership leave churn did not execute (duration={:?}, churn_interval={:?}, final_settle_window={final_settle_window:?})",
            cfg.duration,
            cfg.churn_interval
        );
    }
    ensure!(
        churn_paused_ratio <= MAX_CHURN_PAUSED_RATIO,
        "churn work consumed too much of the wall-clock soak: paused_secs={churn_paused_secs:.3}, elapsed_secs={elapsed_secs:.3}, observed_ratio={churn_paused_ratio:.4}, threshold={MAX_CHURN_PAUSED_RATIO}"
    );
    ensure!(
        soak_overrun_secs <= MAX_SOAK_OVERRUN_SECS as f64,
        "soak exceeded the configured wall-clock duration by too much: overrun_secs={soak_overrun_secs:.3}, threshold_secs={MAX_SOAK_OVERRUN_SECS}"
    );
    let process_lagged_ratio =
        lagged_cycle_ratio(process_churn_lagged_cycles, process_churn_cycles);
    ensure!(
        process_lagged_ratio <= cfg.max_lagged_cycle_ratio,
        "process churn lagged cycles exceeded threshold: lagged={process_churn_lagged_cycles}, total={process_churn_cycles}, observed_ratio={process_lagged_ratio:.4}, threshold={}",
        cfg.max_lagged_cycle_ratio
    );
    let membership_lagged_ratio =
        lagged_cycle_ratio(membership_churn_lagged_cycles, membership_churn_cycles);
    ensure!(
        membership_lagged_ratio <= cfg.max_lagged_cycle_ratio,
        "membership churn lagged cycles exceeded threshold: lagged={membership_churn_lagged_cycles}, total={membership_churn_cycles}, observed_ratio={membership_lagged_ratio:.4}, threshold={}",
        cfg.max_lagged_cycle_ratio
    );
    ensure!(
        scheduled_tps >= (cfg.tps as f64 * MIN_SCHEDULED_TPS_RATIO),
        "scheduled load tps is below threshold: scheduled_tps={scheduled_tps:.2}, target={}, min_ratio={}",
        cfg.tps,
        MIN_SCHEDULED_TPS_RATIO
    );
    ensure!(
        submitted_tps >= (cfg.tps as f64 * MIN_ACCEPTED_TPS_RATIO),
        "accepted submit tps is below threshold: submitted_tps={submitted_tps:.2}, target={}, min_ratio={}",
        cfg.tps,
        MIN_ACCEPTED_TPS_RATIO
    );
    ensure!(
        tx_attempted >= tx_sent,
        "internal load accounting is inconsistent: attempted={tx_attempted}, sent={tx_sent}"
    );
    ensure!(
        tx_submit_errors <= tx_attempted / 20 + 1,
        "tx submission errors too high: errors={tx_submit_errors}, attempted={tx_attempted}, sent={tx_sent}"
    );
    ensure!(tx_sent > 0, "no transactions were accepted during soak run");
    let final_target =
        validator_max_height_with_retry(&harness.validator_clients, READY_TIMEOUT).await?;
    wait_for_cluster_convergence_quorum(
        &harness.validator_clients,
        final_target,
        cfg.max_height_skew,
        min_presence_matches(harness.validator_clients.len()),
        READY_TIMEOUT,
    )
    .await?;
    if let Err(err) = wait_for_cluster_convergence(
        &harness.validator_clients,
        final_target,
        cfg.max_height_skew,
        READY_TIMEOUT,
    )
    .await
    {
        eprintln!("final all-validator convergence lagged; quorum convergence is healthy: {err:?}");
    }
    let final_status_observations = collect_indexed_statuses_quorum_with_retry(
        &harness.validator_clients,
        validator_quorum,
        STATUS_QUORUM_RETRY_TIMEOUT,
    )
    .await?;
    view_change_tracker.observe(&final_status_observations);
    let final_statuses = top_quorum_statuses(
        &statuses_from_indexed(&final_status_observations),
        validator_quorum,
    );
    let final_status_snapshots =
        observe_liveness(&harness.validator_clients, validator_quorum).status_snapshots;
    ensure!(
        final_status_snapshots.len() >= validator_quorum,
        "failed to capture an authoritative Sumeragi v2 status quorum after the soak: captured={}, required={validator_quorum}",
        final_status_snapshots.len()
    );
    let view_changes_end =
        view_changes_start.saturating_add(view_change_tracker.total_since_baseline());
    let view_change_rate_per_sec = view_change_rate(view_changes_start, view_changes_end, elapsed);
    ensure!(
        view_change_rate_per_sec <= cfg.max_view_change_rate,
        "view-change rate exceeded threshold: observed={view_change_rate_per_sec:.4}, threshold={}",
        cfg.max_view_change_rate
    );
    let final_min_txs_approved = min_txs_approved(&final_statuses);
    let committed_txs_min_delta = final_min_txs_approved.saturating_sub(initial_min_txs_approved);
    let committed_tps = committed_txs_min_delta as f64 / elapsed_secs;
    ensure!(
        committed_tps >= (cfg.tps as f64 * cfg.min_committed_tps_ratio),
        "committed/finalized tps is below threshold: committed_tps={committed_tps:.2}, target={}, min_ratio={}",
        cfg.tps,
        cfg.min_committed_tps_ratio
    );
    if let Some(interval) = active_no_progress_interval.take() {
        no_progress_intervals.push(
            interval.finish(u64::try_from(start_time.elapsed().as_millis()).unwrap_or(u64::MAX)),
        );
    }
    let unclassified_no_progress_intervals = u64::try_from(
        no_progress_intervals
            .iter()
            .filter(|interval| !interval.classified)
            .count(),
    )
    .unwrap_or(u64::MAX);
    ensure!(
        unclassified_no_progress_intervals == 0,
        "Taira soak observed {unclassified_no_progress_intervals} unclassified no-progress intervals: {no_progress_intervals:?}"
    );
    Ok(SimulationSummary {
        git_revision: harness.git_revision.clone(),
        workspace_source_manifest_sha256: workspace_source_manifest_sha256.to_owned(),
        build_profile: release_execution_profile.build_profile.clone(),
        cargo_net_offline: release_execution_profile.cargo_net_offline,
        localnet_artifact_path: harness.out_dir.display().to_string(),
        daemon_binary_path: harness.daemon_binary_path.display().to_string(),
        daemon_binary_blake2b_256: harness.daemon_binary_blake2b_256.clone(),
        kagami_binary_path: harness.kagami_binary_path.display().to_string(),
        kagami_binary_blake2b_256: harness.kagami_binary_blake2b_256.clone(),
        test_binary_path: harness.test_binary_path.display().to_string(),
        test_binary_blake2b_256: harness.test_binary_blake2b_256.clone(),
        generated_config_blake2b_256: harness.generated_config_blake2b_256.clone(),
        seed: harness.seed.clone(),
        duration_secs,
        target_tps: cfg.tps,
        packet_loss_percent: cfg.packet_loss_percent,
        churn_interval_secs: cfg.churn_interval.as_secs(),
        max_height_skew: cfg.max_height_skew,
        max_height_skew_grace_secs: cfg.max_height_skew_grace.as_secs(),
        max_transient_height_skew: cfg.max_transient_height_skew,
        stall_timeout_secs: cfg.stall_timeout.as_secs(),
        max_view_change_rate: cfg.max_view_change_rate,
        max_lagged_cycle_ratio: cfg.max_lagged_cycle_ratio,
        min_committed_tps_ratio: cfg.min_committed_tps_ratio,
        process_downtime_secs: cfg.process_downtime.as_secs(),
        tx_attempted,
        tx_sent,
        tx_submit_errors,
        process_churn_cycles,
        expected_process_churn_cycles,
        process_churn_lagged_cycles,
        membership_join_cycles,
        membership_leave_cycles,
        expected_membership_churn_cycles,
        membership_cleanup_leave,
        membership_churn_lagged_cycles,
        membership_churn_warning_cycles,
        churn_paused_secs,
        churn_paused_ratio,
        soak_overrun_secs,
        max_height_skew_observed,
        view_changes_start,
        view_changes_end,
        view_change_rate_per_sec,
        scheduled_tps,
        submitted_tps,
        committed_tps,
        committed_txs_min_delta,
        saturated_samples,
        total_samples,
        initial_status_snapshots,
        final_status_snapshots,
        no_progress_intervals,
        unclassified_no_progress_intervals,
    })
}
async fn process_churn_cycle(
    harness: &mut TairaHarness,
    idx: usize,
    downtime: Duration,
) -> Result<bool> {
    ensure!(
        idx < harness.validator_clients.len(),
        "validator index out of bounds for process churn: {idx}"
    );
    let mut lagged = false;
    let baseline =
        validator_max_height_with_retry(&harness.validator_clients, READY_TIMEOUT).await?;
    harness.localnet.stop_validator(idx)?;
    sleep(downtime).await;
    harness.localnet.start_validator(idx)?;
    wait_for_status_ready(&harness.validator_clients[idx], READY_TIMEOUT).await?;
    let restart_target = validator_restart_catchup_target(baseline);
    let restart_catchup_timeout = Duration::from_secs(RESTART_CATCHUP_TIMEOUT_SECS);
    if let Err(err) = wait_for_height_at_least(
        &harness.validator_clients[idx],
        restart_target,
        restart_catchup_timeout,
    )
    .await
    {
        lagged = true;
        eprintln!(
            "validator restart catch-up lagged: baseline={baseline}, target={restart_target}, timeout={restart_catchup_timeout:?}, err={err:?}"
        );
    }
    let interim_convergence_timeout = Duration::from_secs(INTERIM_CONVERGENCE_TIMEOUT_SECS);
    let validator_quorum = min_presence_matches(harness.validator_clients.len());
    if let Err(err) = wait_for_cluster_convergence_quorum(
        &harness.validator_clients,
        baseline,
        INTERIM_CONVERGENCE_MAX_SKEW,
        validator_quorum,
        interim_convergence_timeout,
    )
    .await
    {
        lagged = true;
        eprintln!(
            "validator restart quorum convergence lagged: timeout={interim_convergence_timeout:?}, err={err:?}"
        );
    } else if let Err(err) = wait_for_cluster_convergence(
        &harness.validator_clients,
        baseline,
        INTERIM_CONVERGENCE_MAX_SKEW,
        interim_convergence_timeout,
    )
    .await
    {
        lagged = true;
        eprintln!(
            "validator restart all-validator convergence lagged; quorum convergence is healthy: timeout={interim_convergence_timeout:?}, err={err:?}"
        );
    }
    Ok(lagged)
}
fn validator_restart_catchup_target(baseline: u64) -> u64 {
    baseline.saturating_sub(INTERIM_CONVERGENCE_MAX_SKEW)
}
async fn membership_join_cycle(
    harness: &mut TairaHarness,
    joiner_warning_state: &mut JoinerCatchupWarningState,
) -> Result<MembershipCycleOutcome> {
    let mut outcome = MembershipCycleOutcome::default();
    let baseline =
        validator_max_height_with_retry(&harness.validator_clients, READY_TIMEOUT).await?;
    let is_registered = match is_peer_present(&harness.primary_client, &harness.joiner.peer_id) {
        Ok(is_registered) => is_registered,
        Err(err) => {
            outcome.mark_warning_lag();
            eprintln!(
                "joiner registration pre-check lagged; proceeding with best-effort register: err={err:?}"
            );
            false
        }
    };
    if !is_registered {
        let register: InstructionBox =
            RegisterPeerWithPop::new(harness.joiner.peer_id.clone(), harness.joiner.pop.clone())
                .into();
        if let Err(err) =
            submit_instruction_with_retry(&harness.primary_client, &register, READY_TIMEOUT).await
        {
            if is_register_duplicate_error(&err) || is_submit_timeout_error(&err) {
                outcome.mark_hard_lag();
                eprintln!("joiner register submission lagged: err={err:?}");
            } else {
                return Err(err).wrap_err("submit joiner register instruction");
            }
        }
        let join_propagation_timeout = Duration::from_secs(INTERIM_CONVERGENCE_TIMEOUT_SECS);
        if let Err(err) = wait_for_peer_presence_across_clients(
            &harness.validator_clients,
            &harness.joiner.peer_id,
            true,
            join_propagation_timeout,
        )
        .await
        {
            outcome.mark_hard_lag();
            eprintln!(
                "joiner register propagation lagged: timeout={join_propagation_timeout:?}, err={err:?}"
            );
        }
    }
    harness.localnet.start_joiner(&harness.joiner.config_path)?;
    wait_for_status_ready(&harness.joiner.client, READY_TIMEOUT).await?;
    let catchup_target = baseline.saturating_sub(INTERIM_CONVERGENCE_MAX_SKEW);
    let catchup_timeout = Duration::from_secs(JOINER_CATCHUP_TIMEOUT_SECS);
    match wait_for_height_or_progress(&harness.joiner.client, catchup_target, catchup_timeout).await
    {
        Ok((start, current)) => match assess_joiner_catchup(start, current, catchup_target) {
            JoinerCatchupAssessment::ReachedTarget => {
                joiner_warning_state.on_reached_target();
            }
            JoinerCatchupAssessment::Progressed => {
                let progress_count = joiner_warning_state.on_progress_below_target();
                if should_log_on_first_and_every_nth(progress_count, JOINER_PROGRESS_LOG_EVERY) {
                    eprintln!(
                        "joiner catch-up is still below validator target after observed progress: baseline={baseline}, target={catchup_target}, start={start}, current={current}, consecutive_progress_cycles={progress_count}"
                    );
                }
            }
            JoinerCatchupAssessment::Stalled => {
                let stalled_count = joiner_warning_state.on_stalled();
                if should_log_on_first_and_every_nth(stalled_count, JOINER_STALL_LOG_EVERY) {
                    eprintln!(
                        "joiner catch-up stalled after registration: baseline={baseline}, target={catchup_target}, start={start}, current={current}, consecutive_stalled_cycles={stalled_count}"
                    );
                }
                record_joiner_stall_warning(&mut outcome, stalled_count);
            }
        },
        Err(err) => {
            let stalled_count = joiner_warning_state.on_stalled();
            if should_log_on_first_and_every_nth(stalled_count, JOINER_STALL_LOG_EVERY) {
                eprintln!(
                    "joiner catch-up stalled after registration: baseline={baseline}, target={catchup_target}, err={err:?}, consecutive_stalled_cycles={stalled_count}"
                );
            }
            record_joiner_stall_warning(&mut outcome, stalled_count);
        }
    }
    let convergence_target =
        validator_max_height_with_retry(&harness.validator_clients, READY_TIMEOUT).await?;
    let convergence_timeout = Duration::from_secs(INTERIM_CONVERGENCE_TIMEOUT_SECS);
    let validator_quorum = min_presence_matches(harness.validator_clients.len());
    if let Err(err) = wait_for_cluster_convergence_quorum(
        &harness.validator_clients,
        convergence_target,
        INTERIM_CONVERGENCE_MAX_SKEW,
        validator_quorum,
        convergence_timeout,
    )
    .await
    {
        outcome.mark_hard_lag();
        eprintln!("membership join quorum convergence lagged: {err:?}");
    } else if let Err(err) = wait_for_cluster_convergence(
        &harness.validator_clients,
        convergence_target,
        INTERIM_CONVERGENCE_MAX_SKEW,
        convergence_timeout,
    )
    .await
    {
        outcome.mark_warning_lag();
        eprintln!(
            "membership join all-validator convergence lagged; quorum convergence is healthy: {err:?}"
        );
    }
    Ok(outcome)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JoinerCatchupAssessment {
    ReachedTarget,
    Progressed,
    Stalled,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct JoinerCatchupWarningState {
    consecutive_stalled: u64,
    consecutive_progress_below_target: u64,
}
impl JoinerCatchupWarningState {
    fn on_reached_target(&mut self) {
        self.consecutive_stalled = 0;
        self.consecutive_progress_below_target = 0;
    }
    fn on_progress_below_target(&mut self) -> u64 {
        self.consecutive_progress_below_target =
            self.consecutive_progress_below_target.saturating_add(1);
        self.consecutive_stalled = 0;
        self.consecutive_progress_below_target
    }
    fn on_stalled(&mut self) -> u64 {
        self.consecutive_stalled = self.consecutive_stalled.saturating_add(1);
        self.consecutive_progress_below_target = 0;
        self.consecutive_stalled
    }
}
fn should_log_on_first_and_every_nth(count: u64, every: u64) -> bool {
    count == 1 || count % every == 0
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
fn should_count_joiner_stall_as_warning(consecutive_stalled_cycles: u64) -> bool {
    consecutive_stalled_cycles >= JOINER_STALL_WARNING_THRESHOLD
}
fn record_joiner_stall_warning(
    outcome: &mut MembershipCycleOutcome,
    consecutive_stalled_cycles: u64,
) {
    if should_count_joiner_stall_as_warning(consecutive_stalled_cycles) {
        outcome.mark_warning_lag();
    }
}
fn membership_backoff_requires_hard_lag(outcome: MembershipCycleOutcome) -> bool {
    outcome.hard_lagged
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
async fn membership_leave_cycle(harness: &mut TairaHarness) -> Result<MembershipCycleOutcome> {
    let mut outcome = MembershipCycleOutcome::default();
    let should_unregister = match is_peer_present(&harness.primary_client, &harness.joiner.peer_id)
    {
        Ok(is_registered) => is_registered,
        Err(err) => {
            outcome.mark_warning_lag();
            eprintln!(
                "joiner unregister pre-check lagged; proceeding with best-effort unregister: err={err:?}"
            );
            true
        }
    };
    // Stop the joiner process before waiting for peer removal propagation.
    // This avoids counting active-connection linger as unregister propagation lag.
    harness.localnet.stop_joiner()?;
    if should_unregister {
        let unregister: InstructionBox = Unregister::peer(harness.joiner.peer_id.clone()).into();
        if let Err(err) =
            submit_instruction_with_retry(&harness.primary_client, &unregister, READY_TIMEOUT).await
        {
            if is_unregister_missing_peer_error(&err) || is_submit_timeout_error(&err) {
                outcome.mark_hard_lag();
                eprintln!("joiner unregister submission lagged: err={err:?}");
            } else {
                return Err(err).wrap_err("submit joiner unregister instruction");
            }
        }
        let leave_propagation_timeout = Duration::from_secs(INTERIM_CONVERGENCE_TIMEOUT_SECS);
        if let Err(err) = wait_for_peer_presence_across_clients(
            &harness.validator_clients,
            &harness.joiner.peer_id,
            false,
            leave_propagation_timeout,
        )
        .await
        {
            outcome.mark_hard_lag();
            eprintln!(
                "joiner unregister propagation lagged: timeout={leave_propagation_timeout:?}, err={err:?}"
            );
        }
    }
    let convergence_target =
        validator_max_height_with_retry(&harness.validator_clients, READY_TIMEOUT).await?;
    let convergence_timeout = Duration::from_secs(INTERIM_CONVERGENCE_TIMEOUT_SECS);
    let validator_quorum = min_presence_matches(harness.validator_clients.len());
    if let Err(err) = wait_for_cluster_convergence_quorum(
        &harness.validator_clients,
        convergence_target,
        INTERIM_CONVERGENCE_MAX_SKEW,
        validator_quorum,
        convergence_timeout,
    )
    .await
    {
        outcome.mark_hard_lag();
        eprintln!("membership leave quorum convergence lagged: {err:?}");
    } else if let Err(err) = wait_for_cluster_convergence(
        &harness.validator_clients,
        convergence_target,
        INTERIM_CONVERGENCE_MAX_SKEW,
        convergence_timeout,
    )
    .await
    {
        outcome.mark_warning_lag();
        eprintln!(
            "membership leave all-validator convergence lagged; quorum convergence is healthy: {err:?}"
        );
    }
    Ok(outcome)
}
async fn setup_taira_harness<const STRICT_ALL_VALIDATORS: bool>(
    out_dir: &Path,
    seed: &str,
    packet_loss_percent: u8,
) -> Result<TairaHarness> {
    let api_ports = alloc_port_block(TAIRA_TOTAL_PORT_SLOTS)?;
    let p2p_ports = alloc_port_block(TAIRA_TOTAL_PORT_SLOTS)?;
    let base_api_port = api_ports.base();
    let base_p2p_port = p2p_ports.base();
    let generated = generate_localnet(
        out_dir,
        base_api_port,
        base_p2p_port,
        TAIRA_VALIDATORS,
        seed,
        packet_loss_percent,
    )?;
    let irohad_bin = Program::Irohad
        .resolve()
        .wrap_err("resolve iroha3d binary")?;
    let daemon_binary_blake2b_256 = file_blake2b_256(&irohad_bin)?;
    let test_binary_path = std::env::current_exe()
        .wrap_err("resolve current Taira test binary")?
        .canonicalize()
        .wrap_err("canonicalize current Taira test binary")?;
    let test_binary_blake2b_256 = file_blake2b_256(&test_binary_path)?;
    let git_revision = current_git_revision()?;
    let mut localnet = ManagedLocalnet::start(
        out_dir,
        &irohad_bin,
        TAIRA_VALIDATORS,
        (api_ports, p2p_ports),
    )?;
    let generic_client = load_localnet_client(out_dir)?;
    let mut primary_client =
        load_validator_authority_client(out_dir, &generic_client, base_api_port, 0)?;
    primary_client.transaction_status_timeout = READY_TIMEOUT;
    let validator_clients =
        build_validator_clients(&primary_client, base_api_port, TAIRA_VALIDATORS)?;
    let validator_quorum = min_presence_matches(validator_clients.len());
    wait_for_status_ready_quorum(
        &validator_clients,
        validator_quorum,
        &mut localnet,
        READY_TIMEOUT,
    )
    .await?;
    if get_status_with_retry(&primary_client).is_err() {
        if let Some(candidate) = validator_clients
            .iter()
            .find(|client| get_status_with_retry(client).is_ok())
            .cloned()
        {
            primary_client = candidate;
        }
    }
    let convergence_target =
        validator_max_height_with_retry(&validator_clients, READY_TIMEOUT).await?;
    wait_for_cluster_convergence_quorum(
        &validator_clients,
        convergence_target,
        INTERIM_CONVERGENCE_MAX_SKEW,
        validator_quorum,
        READY_TIMEOUT,
    )
    .await?;
    if let Err(err) = wait_for_cluster_convergence(
        &validator_clients,
        convergence_target,
        INTERIM_CONVERGENCE_MAX_SKEW,
        READY_TIMEOUT,
    )
    .await
    {
        if STRICT_ALL_VALIDATORS {
            return Err(err);
        }
        eprintln!("initial all-validator convergence lagged; continuing on quorum: {err:?}");
    }
    let joiner_api_port = base_api_port + TAIRA_VALIDATORS;
    let joiner_p2p_port = base_p2p_port + TAIRA_VALIDATORS;
    let joiner = build_joiner_peer(
        out_dir,
        &primary_client,
        &out_dir.join("peer0.toml"),
        joiner_api_port,
        joiner_p2p_port,
    )?;
    let generated_config_blake2b_256 = generated_config_blake2b_256(out_dir)?;
    Ok(TairaHarness {
        out_dir: out_dir.to_path_buf(),
        seed: seed.to_owned(),
        git_revision,
        daemon_binary_path: irohad_bin,
        daemon_binary_blake2b_256,
        kagami_binary_path: generated.kagami_binary_path,
        kagami_binary_blake2b_256: generated.kagami_binary_blake2b_256,
        test_binary_path,
        test_binary_blake2b_256,
        generated_config_blake2b_256,
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
    let soranet_transport_key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
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
    root.insert(
        "soranet_transport_public_key".into(),
        TomlValue::String(soranet_transport_key.public_key().to_string()),
    );
    root.insert(
        "soranet_transport_private_key".into(),
        TomlValue::String(
            ExposedPrivateKey(soranet_transport_key.private_key().clone()).to_string(),
        ),
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
    assert_ne!(
        soranet_transport_key.public_key(),
        stream_key.public_key(),
        "joiner SoraNet transport and streaming identities must be independent"
    );
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
fn apply_queue_transaction_ttl(root: &mut Table, ttl_ms: i64) -> Result<()> {
    ensure!(
        ttl_ms > 0,
        "queue transaction ttl must be positive, got {ttl_ms}"
    );
    let queue = get_subtable_mut(root, "queue")?;
    queue.insert(
        "transaction_time_to_live_ms".into(),
        TomlValue::Integer(ttl_ms),
    );
    Ok(())
}
fn apply_client_transaction_ttl(root: &mut Table, ttl_ms: i64) -> Result<()> {
    ensure!(
        ttl_ms > 0,
        "client transaction ttl must be positive, got {ttl_ms}"
    );
    let transaction = get_subtable_mut(root, "transaction")?;
    transaction.insert("time_to_live_ms".into(), TomlValue::Integer(ttl_ms));
    if let Some(status_timeout_ms) = transaction
        .get("status_timeout_ms")
        .and_then(TomlValue::as_integer)
        && status_timeout_ms > ttl_ms
    {
        transaction.insert("status_timeout_ms".into(), TomlValue::Integer(ttl_ms));
    }
    Ok(())
}
fn apply_packet_impairment(root: &mut Table, percent: u8) -> Result<()> {
    ensure!(percent <= 100, "packet loss must be at most 100 percent");
    let network = get_subtable_mut(root, "network")?;
    let percent = TomlValue::Integer(i64::from(percent));
    network.insert("debug_packet_loss_inbound_percent".into(), percent.clone());
    network.insert("debug_packet_loss_outbound_percent".into(), percent);
    Ok(())
}
fn override_localnet_transaction_ttl(out_dir: &Path, peers: u16, ttl_ms: i64) -> Result<()> {
    for idx in 0..peers {
        let config_path = out_dir.join(format!("peer{idx}.toml"));
        let config_text = fs::read_to_string(&config_path)
            .wrap_err_with(|| format!("read peer config {}", config_path.display()))?;
        let mut parsed: TomlValue = toml::from_str(&config_text)
            .wrap_err_with(|| format!("parse peer config {}", config_path.display()))?;
        let root = parsed
            .as_table_mut()
            .ok_or_else(|| eyre!("peer config root must be a TOML table"))?;
        apply_queue_transaction_ttl(root, ttl_ms)?;
        fs::write(
            &config_path,
            toml::to_string(&parsed).expect("serialize peer config TOML"),
        )
        .wrap_err_with(|| format!("write peer config {}", config_path.display()))?;
    }
    let client_path = out_dir.join("client.toml");
    let client_text = fs::read_to_string(&client_path)
        .wrap_err_with(|| format!("read client config {}", client_path.display()))?;
    let mut client_parsed: TomlValue = toml::from_str(&client_text)
        .wrap_err_with(|| format!("parse client config {}", client_path.display()))?;
    let client_root = client_parsed
        .as_table_mut()
        .ok_or_else(|| eyre!("client config root must be a TOML table"))?;
    apply_client_transaction_ttl(client_root, ttl_ms)?;
    fs::write(
        &client_path,
        toml::to_string(&client_parsed).expect("serialize client config TOML"),
    )
    .wrap_err_with(|| format!("write client config {}", client_path.display()))?;
    Ok(())
}
fn override_localnet_packet_impairment(out_dir: &Path, peers: u16, percent: u8) -> Result<()> {
    for idx in 0..peers {
        let config_path = out_dir.join(format!("peer{idx}.toml"));
        let config_text = fs::read_to_string(&config_path)
            .wrap_err_with(|| format!("read peer config {}", config_path.display()))?;
        let mut parsed: TomlValue = toml::from_str(&config_text)
            .wrap_err_with(|| format!("parse peer config {}", config_path.display()))?;
        let root = parsed
            .as_table_mut()
            .ok_or_else(|| eyre!("peer config root must be a TOML table"))?;
        apply_packet_impairment(root, percent)?;
        fs::write(
            &config_path,
            toml::to_string(&parsed).expect("serialize peer config TOML"),
        )
        .wrap_err_with(|| format!("write peer config {}", config_path.display()))?;
    }
    Ok(())
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
fn load_validator_authority_client(
    out_dir: &Path,
    template: &Client,
    api_port: u16,
    validator_index: usize,
) -> Result<Client> {
    let config_path = out_dir.join(format!("peer{validator_index}.toml"));
    let config_text = fs::read_to_string(&config_path)
        .wrap_err_with(|| format!("read validator config {}", config_path.display()))?;
    let parsed: TomlValue = toml::from_str(&config_text)
        .wrap_err_with(|| format!("parse validator config {}", config_path.display()))?;
    let root = parsed
        .as_table()
        .ok_or_else(|| eyre!("validator config root must be a TOML table"))?;
    let public_key: PublicKey = root
        .get("public_key")
        .and_then(TomlValue::as_str)
        .ok_or_else(|| eyre!("validator config missing public_key"))?
        .parse()
        .wrap_err("parse validator public_key")?;
    let private_key: PrivateKey = root
        .get("private_key")
        .and_then(TomlValue::as_str)
        .ok_or_else(|| eyre!("validator config missing private_key"))?
        .parse()
        .wrap_err("parse validator private_key")?;
    let mut client = build_client_for_port(template, api_port)?;
    client.key_pair = KeyPair::new(public_key.clone(), private_key)
        .wrap_err("construct validator authority key pair")?;
    client.account = AccountId::new(public_key);
    Ok(client)
}
fn collect_statuses(clients: &[Client]) -> Result<Vec<iroha::client::Status>> {
    clients
        .iter()
        .map(get_status_with_retry)
        .collect::<Result<Vec<_>>>()
}
type IndexedStatus = (usize, iroha::client::Status);
#[derive(Debug)]
struct ViewChangeTracker {
    last_seen: Vec<Option<u64>>,
    total_since_baseline: u64,
}
impl ViewChangeTracker {
    fn new(validator_count: usize) -> Self {
        Self {
            last_seen: vec![None; validator_count],
            total_since_baseline: 0,
        }
    }
    fn establish_baseline(&mut self, observations: &[IndexedStatus]) {
        for (index, status) in observations {
            if let Some(last_seen) = self.last_seen.get_mut(*index) {
                *last_seen = Some(u64::from(status.view_changes));
            }
        }
    }
    fn observe(&mut self, observations: &[IndexedStatus]) {
        for (index, status) in observations {
            let current = u64::from(status.view_changes);
            let Some(last_seen) = self.last_seen.get_mut(*index) else {
                continue;
            };
            let delta = if let Some(previous) = *last_seen {
                if current >= previous {
                    current - previous
                } else {
                    // A process restart resets the status counter. Count the
                    // post-restart prefix instead of discarding it.
                    current
                }
            } else {
                // If this validator was unavailable while the baseline quorum
                // was collected, count its first observable value rather than
                // silently discarding possible in-soak view changes. The
                // counter may include pre-soak changes, which is deliberately
                // conservative for a maximum-rate release gate.
                current
            };
            self.total_since_baseline = self.total_since_baseline.saturating_add(delta);
            *last_seen = Some(current);
        }
    }
    fn total_since_baseline(&self) -> u64 {
        self.total_since_baseline
    }
}
fn statuses_from_indexed(observations: &[IndexedStatus]) -> Vec<iroha::client::Status> {
    observations
        .iter()
        .map(|(_, status)| status.clone())
        .collect()
}
fn total_indexed_view_changes(observations: &[IndexedStatus]) -> u64 {
    observations.iter().fold(0_u64, |total, (_, status)| {
        total.saturating_add(u64::from(status.view_changes))
    })
}
fn top_quorum_statuses(
    statuses: &[iroha::client::Status],
    quorum_size: usize,
) -> Vec<iroha::client::Status> {
    let mut selected = statuses.to_vec();
    selected.sort_by_key(|status| Reverse(status.blocks));
    selected.truncate(quorum_size.min(selected.len()));
    selected
}
fn observed_validator_heights(clients: &[Client]) -> Vec<Option<u64>> {
    clients
        .iter()
        .map(|client| {
            get_status_with_retry(client)
                .ok()
                .map(|status| status.blocks)
        })
        .collect()
}
fn collect_statuses_quorum(
    clients: &[Client],
    min_required: usize,
) -> Result<Vec<iroha::client::Status>> {
    collect_indexed_statuses_quorum(clients, min_required)
        .map(|observations| statuses_from_indexed(&observations))
}
fn collect_indexed_statuses_quorum(
    clients: &[Client],
    min_required: usize,
) -> Result<Vec<IndexedStatus>> {
    ensure!(
        min_required > 0 && min_required <= clients.len(),
        "invalid status quorum requirement min_required={min_required} for {} clients",
        clients.len()
    );
    let mut statuses = Vec::with_capacity(clients.len());
    let mut errored_clients = Vec::new();
    for (index, client) in clients.iter().enumerate() {
        match get_status_with_retry(client) {
            Ok(status) => statuses.push((index, status)),
            Err(err) => errored_clients.push(format!("{}: {err:?}", client.torii_url)),
        }
    }
    ensure!(
        statuses.len() >= min_required,
        "failed to collect /status quorum: collected={}/{}, required={min_required}, errored_clients={errored_clients:?}",
        statuses.len(),
        clients.len()
    );
    Ok(statuses)
}
async fn collect_statuses_quorum_with_retry(
    clients: &[Client],
    min_required: usize,
    timeout: Duration,
) -> Result<Vec<iroha::client::Status>> {
    collect_indexed_statuses_quorum_with_retry(clients, min_required, timeout)
        .await
        .map(|observations| statuses_from_indexed(&observations))
}
async fn collect_indexed_statuses_quorum_with_retry(
    clients: &[Client],
    min_required: usize,
    timeout: Duration,
) -> Result<Vec<IndexedStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        match collect_indexed_statuses_quorum(clients, min_required) {
            Ok(statuses) => return Ok(statuses),
            Err(err) => {
                let err_msg = format!("{err:?}");
                ensure!(
                    Instant::now() < deadline,
                    "timed out collecting /status quorum within {timeout:?}: {err_msg}"
                );
                sleep(STATUS_POLL).await;
            }
        }
    }
}
fn refresh_primary_client_from_validators(harness: &mut TairaHarness) -> bool {
    if get_status_with_retry(&harness.primary_client).is_ok() {
        return true;
    }
    let previous_url = harness.primary_client.torii_url.to_string();
    for client in &harness.validator_clients {
        if get_status_with_retry(client).is_ok() {
            let candidate_url = client.torii_url.to_string();
            if candidate_url != previous_url {
                eprintln!(
                    "switching primary client endpoint for management operations: {previous_url} -> {candidate_url}"
                );
                harness.primary_client = client.clone();
            }
            return true;
        }
    }
    false
}
fn get_status_with_retry(client: &Client) -> Result<iroha::client::Status> {
    let mut last_error = None;
    for attempt in 0..STATUS_REQUEST_RETRIES {
        match client.get_status() {
            Ok(status) => return Ok(status),
            Err(err) => {
                let should_retry =
                    is_http_timeout_error(&err) && attempt + 1 < STATUS_REQUEST_RETRIES;
                last_error = Some(err);
                if !should_retry {
                    break;
                }
                thread::sleep(STATUS_REQUEST_RETRY_BACKOFF);
            }
        }
    }
    Err(last_error.expect("status retry loop should capture at least one error"))
        .wrap_err_with(|| format!("failed to collect /status from {}", client.torii_url))
}
fn max_height(statuses: &[iroha::client::Status]) -> u64 {
    statuses.iter().map(|s| s.blocks).max().unwrap_or(0)
}
fn min_height(statuses: &[iroha::client::Status]) -> u64 {
    statuses.iter().map(|s| s.blocks).min().unwrap_or(0)
}
fn min_txs_approved(statuses: &[iroha::client::Status]) -> u64 {
    statuses.iter().map(|s| s.txs_approved).min().unwrap_or(0)
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
    err.chain().any(|cause| {
        let message = cause.to_string();
        message.contains("operation timed out") || message.contains("timed out")
    })
}
fn is_submit_timeout_error(err: &eyre::Report) -> bool {
    is_queue_timeout_error(err) || is_http_timeout_error(err)
}
fn is_connect_error(err: &eyre::Report) -> bool {
    err.chain().any(|cause| {
        let message = cause.to_string();
        message.contains("Connection refused")
            || message.contains("client error (Connect)")
            || message.contains("tcp connect error")
    })
}
fn is_query_timeout_error(err: &iroha::client::QueryError) -> bool {
    err.to_string().contains("timed out")
}
fn is_register_duplicate_error(err: &eyre::Report) -> bool {
    err.chain().any(|cause| {
        let message = cause.to_string();
        message.contains("RepetitionError") || message.contains("Repetition of")
    })
}
fn is_unregister_missing_peer_error(err: &eyre::Report) -> bool {
    err.chain().any(|cause| {
        let message = cause.to_string();
        message.contains("FindError::Peer")
            || message.contains("Peer with id") && message.contains("not found")
    })
}
#[test]
fn http_timeout_error_detector_matches_timeout_messages() {
    let err = eyre!("operation timed out");
    assert!(is_http_timeout_error(&err));
}
#[test]
fn http_timeout_error_detector_ignores_non_timeout_messages() {
    let err = eyre!("connection refused");
    assert!(!is_http_timeout_error(&err));
}
#[test]
fn submit_timeout_error_detector_matches_queue_timeout_messages() {
    let err = eyre!("queued for too long");
    assert!(is_submit_timeout_error(&err));
}
#[test]
fn submit_timeout_error_detector_matches_http_timeout_messages() {
    let err = eyre!("operation timed out");
    assert!(is_submit_timeout_error(&err));
}
#[test]
fn connect_error_detector_matches_connection_refused_messages() {
    let err = eyre!("client error (Connect): Connection refused (os error 61)");
    assert!(is_connect_error(&err));
}
#[test]
fn connect_error_detector_ignores_non_connect_messages() {
    let err = eyre!("operation timed out");
    assert!(!is_connect_error(&err));
}
#[test]
fn register_duplicate_error_detector_matches_repetition_messages() {
    let err = eyre!("RepetitionError: Repetition of PeerId");
    assert!(is_register_duplicate_error(&err));
}
#[test]
fn unregister_missing_peer_error_detector_matches_peer_not_found_messages() {
    let err = eyre!("Peer with id `127.0.0.1:4040#abc` not found");
    assert!(is_unregister_missing_peer_error(&err));
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
#[test]
fn warning_state_logs_first_and_every_interval_for_stalls() {
    let mut warning_state = JoinerCatchupWarningState::default();
    let first = warning_state.on_stalled();
    let second = warning_state.on_stalled();
    let third = warning_state.on_stalled();
    let fourth = warning_state.on_stalled();
    let fifth = warning_state.on_stalled();
    assert!(should_log_on_first_and_every_nth(
        first,
        JOINER_STALL_LOG_EVERY
    ));
    assert!(!should_log_on_first_and_every_nth(
        second,
        JOINER_STALL_LOG_EVERY
    ));
    assert!(!should_log_on_first_and_every_nth(
        third,
        JOINER_STALL_LOG_EVERY
    ));
    assert!(!should_log_on_first_and_every_nth(
        fourth,
        JOINER_STALL_LOG_EVERY
    ));
    assert!(should_log_on_first_and_every_nth(
        fifth,
        JOINER_STALL_LOG_EVERY
    ));
}
#[test]
fn warning_state_resets_stall_counter_after_progress() {
    let mut warning_state = JoinerCatchupWarningState::default();
    warning_state.on_stalled();
    warning_state.on_stalled();
    let progress_count = warning_state.on_progress_below_target();
    assert_eq!(progress_count, 1);
    let stalled_count = warning_state.on_stalled();
    assert_eq!(stalled_count, 1);
}
#[test]
fn top_quorum_statuses_prefers_highest_block_heights() {
    let statuses = vec![
        iroha::client::Status {
            blocks: 10,
            ..Default::default()
        },
        iroha::client::Status {
            blocks: 3,
            ..Default::default()
        },
        iroha::client::Status {
            blocks: 8,
            ..Default::default()
        },
        iroha::client::Status {
            blocks: 11,
            ..Default::default()
        },
    ];
    let selected = top_quorum_statuses(&statuses, 3);
    let heights: Vec<u64> = selected.iter().map(|status| status.blocks).collect();
    assert_eq!(heights, vec![11, 10, 8]);
}
async fn submit_instruction_with_retry(
    client: &Client,
    instruction: &InstructionBox,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match client.submit::<InstructionBox>(
            instruction.clone(),
            iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        ) {
            Ok(_) => return Ok(()),
            Err(err) if is_submit_timeout_error(&err) && Instant::now() < deadline => {
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
async fn wait_for_cluster_convergence_quorum(
    clients: &[Client],
    min_height_target: u64,
    max_skew: u64,
    min_converged_peers: usize,
    timeout: Duration,
) -> Result<()> {
    ensure!(
        min_converged_peers > 0 && min_converged_peers <= clients.len(),
        "invalid min_converged_peers={min_converged_peers} for {} clients",
        clients.len()
    );
    let deadline = Instant::now() + timeout;
    loop {
        let now = Instant::now();
        let remaining = deadline.saturating_duration_since(now);
        ensure!(
            !remaining.is_zero(),
            "cluster failed to reach quorum convergence before timeout: target_height={min_height_target}, max_skew={max_skew}, required_converged_peers={min_converged_peers}"
        );
        let statuses = collect_statuses_quorum_with_retry(
            clients,
            min_converged_peers,
            remaining.min(STATUS_QUORUM_RETRY_TIMEOUT),
        )
        .await?;
        let max_h = max_height(&statuses);
        let min_h = min_height(&statuses);
        let converged_peers = statuses
            .iter()
            .filter(|status| max_h.saturating_sub(status.blocks) <= max_skew)
            .count();
        if max_h >= min_height_target && converged_peers >= min_converged_peers {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "cluster failed to reach quorum convergence: target_height={min_height_target}, max={max_h}, min={min_h}, max_skew={max_skew}, converged_peers={converged_peers}, required_converged_peers={min_converged_peers}"
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
async fn wait_for_status_ready_quorum(
    clients: &[Client],
    min_required: usize,
    localnet: &mut ManagedLocalnet,
    timeout: Duration,
) -> Result<()> {
    ensure!(
        min_required > 0 && min_required <= clients.len(),
        "invalid status readiness quorum min_required={min_required} for {} clients",
        clients.len()
    );
    let deadline = Instant::now() + timeout;
    loop {
        let ready = clients
            .iter()
            .filter(|client| client.get_status().is_ok())
            .count();
        if ready >= min_required {
            return Ok(());
        }
        if let Some(report) = localnet.unexpected_validator_exit_report()? {
            return Err(eyre!(report));
        }
        ensure!(
            Instant::now() < deadline,
            "timed out waiting for /status readiness quorum: ready={ready}/{min_required}"
        );
        sleep(STATUS_POLL).await;
    }
}
fn log_tail(path: &Path, lines: usize) -> String {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let mut tail = contents.lines().rev().take(lines).collect::<Vec<_>>();
            tail.reverse();
            tail.join("\n")
        }
        Err(err) => format!("failed to read log {}: {err}", path.display()),
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
async fn wait_for_peer_presence_across_clients(
    clients: &[Client],
    peer_id: &PeerId,
    present: bool,
    timeout: Duration,
) -> Result<()> {
    ensure!(
        !clients.is_empty(),
        "cannot wait for peer presence across an empty client set"
    );
    let min_required_matches = min_presence_matches(clients.len());
    let deadline = Instant::now() + timeout;
    loop {
        let mut matched_clients = 0_usize;
        let mut mismatched_clients = Vec::new();
        let mut errored_clients = Vec::new();
        let mut last_query_error = None;
        for client in clients {
            match query_peer_presence_with_retry(client, peer_id) {
                Ok(found) => {
                    if found == present {
                        matched_clients = matched_clients.saturating_add(1);
                    } else {
                        mismatched_clients.push(client.torii_url.to_string());
                    }
                }
                Err(err) => {
                    errored_clients.push(client.torii_url.to_string());
                    last_query_error = Some(format!("{err:?}"));
                }
            }
        }
        if matched_clients >= min_required_matches {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "timed out waiting for peer presence={present} ({peer_id}) across validators; matched={matched_clients}/{min_required_matches}; mismatched_clients={mismatched_clients:?}; errored_clients={errored_clients:?}; last_query_error={last_query_error:?}"
        );
        sleep(STATUS_POLL).await;
    }
}
fn query_peer_presence_with_retry(client: &Client, peer_id: &PeerId) -> Result<bool> {
    let mut last_error = None;
    for attempt in 0..PEER_QUERY_REQUEST_RETRIES {
        match client.query(FindPeers::new()).execute_all() {
            Ok(peers) => return Ok(peers.iter().any(|peer| peer == peer_id)),
            Err(err) => {
                let should_retry =
                    is_query_timeout_error(&err) && attempt + 1 < PEER_QUERY_REQUEST_RETRIES;
                last_error = Some(err);
                if !should_retry {
                    break;
                }
                thread::sleep(PEER_QUERY_REQUEST_RETRY_BACKOFF);
            }
        }
    }
    Err(last_error.expect("peer query retry loop should capture at least one error"))
        .wrap_err_with(|| format!("failed to query peers from {}", client.torii_url))
}
fn min_presence_matches(validator_count: usize) -> usize {
    let tolerated_faults = validator_count.saturating_sub(1) / 3;
    validator_count.saturating_sub(tolerated_faults)
}
#[test]
fn min_presence_matches_uses_bft_commit_quorum() {
    assert_eq!(min_presence_matches(1), 1);
    assert_eq!(min_presence_matches(4), 3);
    assert_eq!(min_presence_matches(7), 5);
}
fn first_process_churn_index(validator_count: usize) -> usize {
    if validator_count > 1 { 1 } else { 0 }
}
fn select_process_churn_index(
    clients: &[Client],
    fallback_idx: usize,
    lag_threshold: u64,
) -> usize {
    let observed_heights = observed_validator_heights(clients);
    let selected =
        select_process_churn_index_from_heights(&observed_heights, fallback_idx, lag_threshold);
    if selected != fallback_idx {
        eprintln!(
            "process churn selected lagging/unresponsive validator index {selected} (fallback={fallback_idx}, observed_heights={observed_heights:?})"
        );
    }
    selected
}
fn select_process_churn_index_from_heights(
    observed_heights: &[Option<u64>],
    fallback_idx: usize,
    lag_threshold: u64,
) -> usize {
    if observed_heights.is_empty() {
        return 0;
    }
    let bounded_fallback = fallback_idx.min(observed_heights.len().saturating_sub(1));
    if let Some((idx, _)) = observed_heights
        .iter()
        .enumerate()
        .find(|(_, height)| height.is_none())
    {
        return idx;
    }
    let mut max_height = 0_u64;
    let mut min_height = u64::MAX;
    let mut min_idx = bounded_fallback;
    for (idx, height) in observed_heights.iter().enumerate() {
        let height = height.expect("status heights should be present after None fast-path");
        if height > max_height {
            max_height = height;
        }
        if height < min_height {
            min_height = height;
            min_idx = idx;
        }
    }
    if max_height.saturating_sub(min_height) >= lag_threshold {
        min_idx
    } else {
        bounded_fallback
    }
}
fn next_process_churn_index(current: usize, validator_count: usize) -> usize {
    if validator_count <= 1 {
        return 0;
    }
    (current + 1) % validator_count
}
fn next_process_churn_deadline(
    scheduled_deadline: Instant,
    interval: Duration,
    lagged: bool,
) -> Instant {
    let backoff = lagged
        .then_some(Duration::from_secs(INTERIM_LAG_CHURN_BACKOFF_SECS))
        .unwrap_or(Duration::ZERO);
    scheduled_deadline + interval + backoff
}
fn next_membership_churn_deadline(
    scheduled_deadline: Instant,
    interval: Duration,
    lagged: bool,
) -> Instant {
    next_process_churn_deadline(scheduled_deadline, interval, lagged)
}
fn effective_final_settle_window(duration: Duration) -> Duration {
    let scaled = duration / 3;
    let bounded = FINAL_SETTLE_WINDOW.min(scaled);
    if duration <= Duration::from_secs(1) {
        Duration::ZERO
    } else {
        bounded.min(duration.saturating_sub(Duration::from_secs(1)))
    }
}
fn initial_churn_delay(interval: Duration, churn_window: Duration) -> Duration {
    if churn_window <= Duration::from_secs(1) {
        Duration::ZERO
    } else {
        interval.min(churn_window.saturating_sub(Duration::from_secs(1)))
    }
}
fn scheduled_churn_cycles(
    churn_window: Duration,
    first_delay: Duration,
    interval: Duration,
) -> u64 {
    if interval.is_zero() || first_delay >= churn_window {
        return 0;
    }
    let available = churn_window.saturating_sub(first_delay).as_nanos();
    let count = available.div_ceil(interval.as_nanos());
    u64::try_from(count).unwrap_or(u64::MAX)
}
fn minimum_required_churn_cycles(expected_cycles: u64) -> u64 {
    let numerator = u128::from(expected_cycles) * u128::from(MIN_CHURN_CYCLE_NUMERATOR);
    let required = numerator.div_ceil(u128::from(MIN_CHURN_CYCLE_DENOMINATOR));
    u64::try_from(required).unwrap_or(u64::MAX)
}
fn lagged_cycle_ratio(lagged_cycles: u64, total_cycles: u64) -> f64 {
    if total_cycles == 0 {
        0.0
    } else {
        lagged_cycles as f64 / total_cycles as f64
    }
}
fn view_change_rate(start: u64, end: u64, elapsed: Duration) -> f64 {
    end.saturating_sub(start) as f64 / elapsed.as_secs_f64().max(1.0)
}
async fn validator_max_height_with_retry(clients: &[Client], timeout: Duration) -> Result<u64> {
    let min_required = min_presence_matches(clients.len());
    let deadline = Instant::now() + timeout;
    loop {
        match collect_statuses_quorum(clients, min_required) {
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
async fn submit_load_instruction_with_retry(
    harness: &mut TairaHarness,
    instruction: &InstructionBox,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        match harness.primary_client.submit::<InstructionBox>(
            instruction.clone(),
            iroha::data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        ) {
            Ok(_) => return Ok(()),
            Err(err)
                if (is_submit_timeout_error(&err) || is_connect_error(&err))
                    && Instant::now() < deadline =>
            {
                let previous_url = harness.primary_client.torii_url.to_string();
                if refresh_primary_client_from_validators(harness) {
                    let next_url = harness.primary_client.torii_url.to_string();
                    if next_url != previous_url {
                        eprintln!("switching load submit endpoint: {previous_url} -> {next_url}");
                    }
                }
                sleep(LOAD_SUBMIT_RETRY_BACKOFF).await;
            }
            Err(err) => return Err(err).wrap_err("submit load instruction"),
        }
    }
}
fn is_peer_present(client: &Client, peer_id: &PeerId) -> Result<bool> {
    query_peer_presence_with_retry(client, peer_id)
}
fn file_blake2b_256(path: &Path) -> Result<String> {
    let bytes =
        fs::read(path).wrap_err_with(|| format!("read evidence file {}", path.display()))?;
    Ok(Hash::new(bytes).to_string())
}
fn generated_config_blake2b_256(root: &Path) -> Result<String> {
    fn collect(root: &Path, current: &Path, paths: &mut Vec<PathBuf>) -> Result<()> {
        let mut entries = fs::read_dir(current)
            .wrap_err_with(|| format!("read generated-config directory {}", current.display()))?
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                if path == root.join("storage") {
                    continue;
                }
                collect(root, &path, paths)?;
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let include = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    matches!(extension, "toml" | "json" | "to" | "sh" | "yaml" | "yml")
                });
            if include
                && path
                    .file_name()
                    .is_none_or(|name| name != "taira_simulation_summary.json")
            {
                paths.push(path);
            }
        }
        Ok(())
    }
    let mut paths = Vec::new();
    collect(root, root, &mut paths)?;
    paths.sort();
    let mut manifest = b"iroha-taira-generated-config-v1\0".to_vec();
    for path in paths {
        let relative = path
            .strip_prefix(root)
            .wrap_err_with(|| format!("strip generated-config root from {}", path.display()))?;
        let relative = relative.to_string_lossy();
        let bytes = fs::read(&path)
            .wrap_err_with(|| format!("read generated config {}", path.display()))?;
        manifest.extend_from_slice(&(relative.len() as u64).to_be_bytes());
        manifest.extend_from_slice(relative.as_bytes());
        manifest.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        manifest.extend_from_slice(&bytes);
    }
    Ok(Hash::new(manifest).to_string())
}
fn current_git_revision() -> Result<String> {
    let output = Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(repo_root())
        .output()
        .wrap_err("run git rev-parse HEAD")?;
    ensure!(
        output.status.success(),
        "git rev-parse HEAD failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let revision = String::from_utf8(output.stdout)
        .wrap_err("git revision is not UTF-8")?
        .trim()
        .to_owned();
    ensure!(
        matches!(revision.len(), 40 | 64) && revision.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "git rev-parse HEAD returned an invalid revision: {revision:?}"
    );
    Ok(revision)
}
fn required_release_source_manifest_sha256() -> Result<String> {
    let manifest = std::env::var("IROHA_RELEASE_SOURCE_MANIFEST_SHA256")
        .wrap_err("IROHA_RELEASE_SOURCE_MANIFEST_SHA256 must be set by the release launcher")?;
    ensure!(
        manifest.len() == 64
            && manifest.bytes().all(|byte| byte.is_ascii_hexdigit())
            && manifest == manifest.to_ascii_lowercase(),
        "IROHA_RELEASE_SOURCE_MANIFEST_SHA256 must be a lowercase SHA-256 digest"
    );
    Ok(manifest)
}
fn required_release_execution_profile() -> Result<ReleaseExecutionProfile> {
    let build_profile = std::env::var("IROHA_TEST_BUILD_PROFILE")
        .wrap_err("IROHA_TEST_BUILD_PROFILE must be set by the release launcher")?;
    let cargo_profile =
        std::env::var("PROFILE").wrap_err("PROFILE must be set by the release launcher")?;
    let cargo_net_offline = std::env::var("CARGO_NET_OFFLINE")
        .wrap_err("CARGO_NET_OFFLINE must be set by the release launcher")?;
    validate_release_execution_profile(&build_profile, &cargo_profile, &cargo_net_offline)
}
fn validate_release_execution_profile(
    build_profile: &str,
    cargo_profile: &str,
    cargo_net_offline: &str,
) -> Result<ReleaseExecutionProfile> {
    ensure!(
        build_profile == "release",
        "IROHA_TEST_BUILD_PROFILE must be exactly release"
    );
    ensure!(
        cargo_profile == build_profile,
        "PROFILE and IROHA_TEST_BUILD_PROFILE must both select release"
    );
    ensure!(
        cargo_net_offline == "true",
        "CARGO_NET_OFFLINE must be exactly true"
    );
    Ok(ReleaseExecutionProfile {
        build_profile: build_profile.to_owned(),
        cargo_net_offline: true,
    })
}
fn required_release_evidence_path() -> Result<PathBuf> {
    let raw = std::env::var("IROHA_TAIRA_EVIDENCE_PATH")
        .wrap_err("IROHA_TAIRA_EVIDENCE_PATH must be set by the release launcher")?;
    let path = PathBuf::from(raw);
    ensure!(
        path.is_absolute(),
        "IROHA_TAIRA_EVIDENCE_PATH must be absolute"
    );
    ensure!(
        path.extension()
            .is_some_and(|extension| extension == "json"),
        "IROHA_TAIRA_EVIDENCE_PATH must name a JSON file"
    );
    Ok(path)
}
fn write_summary(
    local_path: &Path,
    evidence_path: &Path,
    summary: &SimulationSummary,
) -> Result<()> {
    let mut rendered = norito::json::to_json_pretty(&summary.to_json_value())
        .wrap_err("serialize summary JSON")?;
    rendered.push('\n');
    fs::write(local_path, rendered.as_bytes())
        .wrap_err_with(|| format!("write summary {}", local_path.display()))?;
    if let Some(parent) = evidence_path.parent() {
        fs::create_dir_all(parent)
            .wrap_err_with(|| format!("create evidence directory {}", parent.display()))?;
    }
    fs::write(evidence_path, rendered.as_bytes())
        .wrap_err_with(|| format!("write durable summary {}", evidence_path.display()))?;
    Ok(())
}
fn finalize_result(temp_dir: TempDir, context: &str, result: Result<()>) -> Result<()> {
    if let Err(err) = result {
        if let Some(reason) = sandbox::sandbox_reason(&err) {
            // Developer runs may still skip a sandbox-denied localnet, but CI
            // and release corridors set `IROHA_TEST_REQUIRE_NETWORK=1`. Reuse
            // the shared parser so malformed values and required-network
            // failures both fail closed before reporting a skip.
            let _ = sandbox::enforce_network_start_requirement::<()>(None, context)?;
            eprintln!("sandbox restriction detected while running {context}; skipping ({reason})");
            return Ok(());
        }
        if std::env::var_os("IROHA_TAIRA_KEEP_LOCALNET").is_some() {
            eprintln!(
                "keeping localnet artifacts at {}",
                temp_dir.path().display()
            );
            let _ = temp_dir.keep();
        }
        return Err(err);
    }
    if std::env::var_os("IROHA_TAIRA_KEEP_LOCALNET").is_some() {
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
fn env_u8(key: &str, default: u8, min: u8, max: u8) -> u8 {
    std::env::var(key)
        .ok()
        .and_then(|raw| raw.parse::<u8>().ok())
        .filter(|value| (min..=max).contains(value))
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
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .ok_or_else(|| eyre!("CARGO_TARGET_DIR is required for Taira localnet artifacts"))?;
    ensure!(!target.is_empty(), "CARGO_TARGET_DIR must not be empty");
    let root = PathBuf::from(target).join("taira-localnet");
    fs::create_dir_all(&root).wrap_err("create Taira localnet artifact root")?;
    tempfile::Builder::new()
        .prefix(label)
        .tempdir_in(&root)
        .wrap_err("create taira localnet temp dir")
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
    packet_loss_percent: u8,
) -> Result<GeneratedLocalnetEvidence> {
    let kagami_bin = resolve_kagami_bin()?;
    let kagami_binary_blake2b_256 = file_blake2b_256(&kagami_bin)?;
    let mut command = Command::new(&kagami_bin);
    command
        .arg("localnet")
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
        .arg(out_dir.to_string_lossy().to_string());
    let output = test_process::output_with_timeout(&mut command, test_process::process_timeout())
        .wrap_err("run kagami localnet")?;
    ensure!(
        output.status.success(),
        "kagami localnet failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    override_localnet_transaction_ttl(out_dir, peers, LOCALNET_TRANSACTION_TTL_MS)?;
    override_localnet_packet_impairment(out_dir, peers, packet_loss_percent)?;
    Ok(GeneratedLocalnetEvidence {
        kagami_binary_path: kagami_bin,
        kagami_binary_blake2b_256,
    })
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
    assert!(cfg.packet_loss_percent <= 100);
    assert!(cfg.churn_interval >= Duration::from_secs(1));
    assert!(cfg.max_height_skew_grace >= Duration::from_secs(1));
    assert!(cfg.max_transient_height_skew >= cfg.max_height_skew);
    assert!(cfg.stall_timeout >= Duration::from_secs(10));
    assert!((0.0..=1.0).contains(&cfg.max_lagged_cycle_ratio));
    assert!((0.0..=1.0).contains(&cfg.min_committed_tps_ratio));
}
#[test]
fn env_u64_respects_minimum() {
    assert_eq!(env_u64("IROHA_TAIRA_NO_SUCH_VAR", 10, 2), 10);
}
#[test]
fn env_u8_respects_closed_range() {
    assert_eq!(env_u8("IROHA_TAIRA_NO_SUCH_U8_VAR", 10, 0, 100), 10);
}
#[test]
fn env_f64_respects_minimum() {
    assert_eq!(env_f64("IROHA_TAIRA_NO_SUCH_VAR_FLOAT", 0.25, 0.0), 0.25);
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
fn process_churn_index_rotates_across_all_validators() {
    assert_eq!(first_process_churn_index(7), 1);
    assert_eq!(next_process_churn_index(1, 7), 2);
    assert_eq!(next_process_churn_index(5, 7), 6);
    assert_eq!(next_process_churn_index(6, 7), 0);
    assert_eq!(next_process_churn_index(0, 7), 1);
}
#[test]
fn process_churn_index_handles_single_validator() {
    assert_eq!(first_process_churn_index(1), 0);
    assert_eq!(next_process_churn_index(0, 1), 0);
}
#[test]
fn select_process_churn_index_prioritizes_unresponsive_validator() {
    let observed = [Some(100), Some(101), None, Some(100)];
    assert_eq!(select_process_churn_index_from_heights(&observed, 0, 6), 2);
}
#[test]
fn select_process_churn_index_prioritizes_lagger_when_skew_is_large() {
    let observed = [Some(100), Some(84), Some(99), Some(100)];
    assert_eq!(select_process_churn_index_from_heights(&observed, 0, 6), 1);
}
#[test]
fn select_process_churn_index_uses_round_robin_fallback_when_balanced() {
    let observed = [Some(100), Some(98), Some(99), Some(100)];
    assert_eq!(select_process_churn_index_from_heights(&observed, 2, 6), 2);
}
#[test]
fn select_process_churn_index_clamps_out_of_bounds_fallback() {
    let observed = [Some(10), Some(10), Some(10)];
    assert_eq!(select_process_churn_index_from_heights(&observed, 7, 6), 2);
}
#[test]
fn next_process_churn_deadline_uses_interval_without_lag() {
    let now = Instant::now();
    let interval = Duration::from_secs(30);
    let deadline = next_process_churn_deadline(now, interval, false);
    assert_eq!(deadline.duration_since(now), interval);
}
#[test]
fn next_process_churn_deadline_adds_backoff_without_schedule_drift() {
    let now = Instant::now();
    let interval = Duration::from_secs(30);
    let deadline = next_process_churn_deadline(now, interval, true);
    assert_eq!(
        deadline.duration_since(now),
        interval.saturating_add(Duration::from_secs(INTERIM_LAG_CHURN_BACKOFF_SECS))
    );
}
#[test]
fn next_membership_churn_deadline_uses_interval_without_lag() {
    let now = Instant::now();
    let interval = Duration::from_secs(30);
    let deadline = next_membership_churn_deadline(now, interval, false);
    assert_eq!(deadline.duration_since(now), interval);
}
#[test]
fn next_membership_churn_deadline_adds_backoff_when_lagged() {
    let now = Instant::now();
    let interval = Duration::from_secs(30);
    let deadline = next_membership_churn_deadline(now, interval, true);
    assert_eq!(
        deadline.duration_since(now),
        interval.saturating_add(Duration::from_secs(INTERIM_LAG_CHURN_BACKOFF_SECS))
    );
}
#[test]
fn membership_backoff_triggers_only_on_hard_lag() {
    let now = Instant::now();
    let interval = Duration::from_secs(30);
    let warning_only = MembershipCycleOutcome {
        hard_lagged: false,
        warning_lagged: true,
    };
    let warning_deadline = next_membership_churn_deadline(
        now,
        interval,
        membership_backoff_requires_hard_lag(warning_only),
    );
    assert_eq!(warning_deadline.duration_since(now), interval);
    let hard_lagged = MembershipCycleOutcome {
        hard_lagged: true,
        warning_lagged: false,
    };
    let hard_deadline = next_membership_churn_deadline(
        now,
        interval,
        membership_backoff_requires_hard_lag(hard_lagged),
    );
    assert_eq!(
        hard_deadline.duration_since(now),
        interval.saturating_add(Duration::from_secs(INTERIM_LAG_CHURN_BACKOFF_SECS))
    );
}
#[test]
fn stalled_joiner_catchup_marks_warning_without_hard_lag() {
    let mut outcome = MembershipCycleOutcome::default();
    record_joiner_stall_warning(&mut outcome, JOINER_STALL_WARNING_THRESHOLD);
    assert!(outcome.warning_lagged);
    assert!(!outcome.hard_lagged);
}
#[test]
fn propagation_and_quorum_failures_mark_hard_lag() {
    let mut propagation_timeout = MembershipCycleOutcome::default();
    propagation_timeout.mark_hard_lag();
    assert!(propagation_timeout.hard_lagged);
    assert!(!propagation_timeout.warning_lagged);
    let mut quorum_timeout = MembershipCycleOutcome::default();
    quorum_timeout.mark_hard_lag();
    assert!(quorum_timeout.hard_lagged);
    assert!(!quorum_timeout.warning_lagged);
}
#[test]
fn effective_final_settle_window_scales_with_duration() {
    assert_eq!(
        effective_final_settle_window(Duration::from_secs(3_600)),
        FINAL_SETTLE_WINDOW
    );
    assert_eq!(
        effective_final_settle_window(Duration::from_secs(90)),
        Duration::from_secs(30)
    );
    assert_eq!(
        effective_final_settle_window(Duration::from_secs(30)),
        Duration::from_secs(10)
    );
}
#[test]
fn initial_churn_delay_stays_inside_churn_window() {
    assert_eq!(
        initial_churn_delay(Duration::from_secs(30), Duration::from_secs(20)),
        Duration::from_secs(19)
    );
    assert_eq!(
        initial_churn_delay(Duration::from_secs(30), Duration::from_secs(60)),
        Duration::from_secs(30)
    );
}
#[test]
fn scheduled_churn_floor_requires_sustained_cycles() {
    assert_eq!(
        scheduled_churn_cycles(
            Duration::from_secs(60),
            Duration::from_secs(30),
            Duration::from_secs(30)
        ),
        1
    );
    assert_eq!(
        scheduled_churn_cycles(
            Duration::from_secs(60),
            Duration::from_secs(15),
            Duration::from_secs(30)
        ),
        2
    );
    assert_eq!(minimum_required_churn_cycles(1), 1);
    assert_eq!(minimum_required_churn_cycles(10), 9);
    assert_eq!(minimum_required_churn_cycles(287), 259);
    let duration = Duration::from_secs(DEFAULT_SIM_DURATION_SECS);
    let churn_window = duration.saturating_sub(effective_final_settle_window(duration));
    assert_eq!(
        scheduled_churn_cycles(
            churn_window,
            initial_churn_delay(
                Duration::from_secs(DEFAULT_CHURN_INTERVAL_SECS),
                churn_window
            ),
            Duration::from_secs(DEFAULT_CHURN_INTERVAL_SECS),
        ),
        287
    );
    assert_eq!(
        scheduled_churn_cycles(
            churn_window,
            initial_churn_delay(
                Duration::from_secs(DEFAULT_CHURN_INTERVAL_SECS / 2),
                churn_window,
            ),
            Duration::from_secs(DEFAULT_CHURN_INTERVAL_SECS),
        ),
        288
    );
    assert_eq!(minimum_required_churn_cycles(288), 260);
}
#[test]
fn lagged_cycle_ratio_rejects_one_bad_cycle() {
    assert_eq!(lagged_cycle_ratio(0, 0), 0.0);
    assert_eq!(lagged_cycle_ratio(1, 1), 1.0);
    assert!((lagged_cycle_ratio(1, 3) - (1.0 / 3.0)).abs() < f64::EPSILON);
    assert!(lagged_cycle_ratio(1, 1) > DEFAULT_MAX_LAGGED_CYCLE_RATIO);
}
#[test]
fn view_change_rate_uses_final_counter_and_full_soak_time() {
    assert_eq!(view_change_rate(10, 22, Duration::from_secs(60)), 0.2);
    assert_eq!(view_change_rate(22, 10, Duration::from_secs(60)), 0.0);
    assert_eq!(view_change_rate(0, 1, Duration::ZERO), 1.0);
}
fn status_with_view_changes(view_changes: u32) -> iroha::client::Status {
    iroha::client::Status {
        view_changes,
        ..iroha::client::Status::default()
    }
}
#[test]
fn view_change_tracker_accumulates_each_validator_across_restart_resets() {
    let mut first = iroha::client::Status::default();
    first.view_changes = 10;
    let mut second = iroha::client::Status::default();
    second.view_changes = 5;
    let baseline = vec![(0, first.clone()), (1, second.clone())];
    let mut tracker = ViewChangeTracker::new(3);
    tracker.establish_baseline(&baseline);
    assert_eq!(total_indexed_view_changes(&baseline), 15);
    first.view_changes = 13;
    second.view_changes = 7;
    let mut newly_observed = iroha::client::Status::default();
    newly_observed.view_changes = 4;
    tracker.observe(&[(0, first.clone()), (1, second.clone()), (2, newly_observed)]);
    assert_eq!(tracker.total_since_baseline(), 9);
    first.view_changes = 2;
    second.view_changes = 9;
    tracker.observe(&[(0, first), (1, second)]);
    assert_eq!(tracker.total_since_baseline(), 13);
}
#[test]
fn view_change_tracker_conservatively_counts_a_late_first_observation() {
    let mut tracker = ViewChangeTracker::new(2);
    tracker.establish_baseline(&[(0, status_with_view_changes(5))]);
    tracker.observe(&[
        (0, status_with_view_changes(7)),
        (1, status_with_view_changes(3)),
    ]);
    assert_eq!(tracker.total_since_baseline(), 5);
}
#[test]
fn min_txs_approved_returns_lowest_counter() {
    let mut first = iroha::client::Status::default();
    first.txs_approved = 42;
    let mut second = iroha::client::Status::default();
    second.txs_approved = 17;
    let mut third = iroha::client::Status::default();
    third.txs_approved = 99;
    assert_eq!(min_txs_approved(&[first, second, third]), 17);
}
#[test]
fn apply_queue_transaction_ttl_updates_queue_section() {
    let mut root = Table::new();
    root.insert("queue".into(), TomlValue::Table(Table::new()));
    apply_queue_transaction_ttl(&mut root, 7_200_000).expect("queue ttl should apply");
    let applied = root
        .get("queue")
        .and_then(TomlValue::as_table)
        .and_then(|queue| {
            queue
                .get("transaction_time_to_live_ms")
                .and_then(TomlValue::as_integer)
        });
    assert_eq!(applied, Some(7_200_000));
}
#[test]
fn apply_client_transaction_ttl_caps_status_timeout() {
    let mut transaction = Table::new();
    transaction.insert("time_to_live_ms".into(), TomlValue::Integer(600_000));
    transaction.insert("status_timeout_ms".into(), TomlValue::Integer(900_000));
    let mut root = Table::new();
    root.insert("transaction".into(), TomlValue::Table(transaction));
    apply_client_transaction_ttl(&mut root, 300_000).expect("client ttl should apply");
    let tx = root
        .get("transaction")
        .and_then(TomlValue::as_table)
        .expect("transaction section should exist");
    assert_eq!(
        tx.get("time_to_live_ms").and_then(TomlValue::as_integer),
        Some(300_000)
    );
    assert_eq!(
        tx.get("status_timeout_ms").and_then(TomlValue::as_integer),
        Some(300_000)
    );
}
#[test]
fn apply_packet_impairment_sets_both_directions() {
    let mut root = Table::new();
    root.insert("network".into(), TomlValue::Table(Table::new()));
    apply_packet_impairment(&mut root, 10).expect("packet impairment should apply");
    let network = root
        .get("network")
        .and_then(TomlValue::as_table)
        .expect("network section should exist");
    assert_eq!(
        network
            .get("debug_packet_loss_inbound_percent")
            .and_then(TomlValue::as_integer),
        Some(10)
    );
    assert_eq!(
        network
            .get("debug_packet_loss_outbound_percent")
            .and_then(TomlValue::as_integer),
        Some(10)
    );
    assert!(apply_packet_impairment(&mut root, 101).is_err());
}
#[test]
fn joiner_stall_warning_threshold_matches_policy() {
    assert!(!should_count_joiner_stall_as_warning(0));
    assert!(!should_count_joiner_stall_as_warning(1));
    assert!(!should_count_joiner_stall_as_warning(2));
    assert!(should_count_joiner_stall_as_warning(3));
}
#[test]
fn release_execution_profile_accepts_only_the_exact_positive_profile() {
    let profile = validate_release_execution_profile("release", "release", "true")
        .expect("exact release/offline profile");
    assert_eq!(profile.build_profile, "release");
    assert!(profile.cargo_net_offline);
}
#[test]
fn release_execution_profile_rejects_wrong_or_blank_build_profiles() {
    for build_profile in ["", "debug", " release", "release "] {
        assert!(
            validate_release_execution_profile(build_profile, build_profile, "true").is_err(),
            "unexpectedly accepted build profile {build_profile:?}"
        );
    }
}
#[test]
fn release_execution_profile_rejects_cargo_profile_mismatch() {
    for cargo_profile in ["", "debug", "release ", "Release"] {
        assert!(
            validate_release_execution_profile("release", cargo_profile, "true").is_err(),
            "unexpectedly accepted Cargo profile {cargo_profile:?}"
        );
    }
}
#[test]
fn release_execution_profile_rejects_non_exact_offline_values() {
    for cargo_net_offline in ["", "1", "TRUE", " true", "true ", "false"] {
        assert!(
            validate_release_execution_profile("release", "release", cargo_net_offline).is_err(),
            "unexpectedly accepted CARGO_NET_OFFLINE={cargo_net_offline:?}"
        );
    }
}
fn sample_simulation_summary() -> SimulationSummary {
    SimulationSummary {
        git_revision: "1".repeat(40),
        workspace_source_manifest_sha256: "a".repeat(64),
        build_profile: "release".to_owned(),
        cargo_net_offline: true,
        localnet_artifact_path: "/tmp/taira-localnet".to_owned(),
        daemon_binary_path: "/tmp/iroha3d".to_owned(),
        daemon_binary_blake2b_256: "b".repeat(64),
        kagami_binary_path: "/tmp/kagami".to_owned(),
        kagami_binary_blake2b_256: "c".repeat(64),
        test_binary_path: "/tmp/taira-test".to_owned(),
        test_binary_blake2b_256: "d".repeat(64),
        generated_config_blake2b_256: "e".repeat(64),
        seed: "taira-public-sim".to_owned(),
        duration_secs: 60,
        target_tps: 5,
        packet_loss_percent: 10,
        churn_interval_secs: 300,
        max_height_skew: 2,
        max_height_skew_grace_secs: 30,
        max_transient_height_skew: 32,
        stall_timeout_secs: 300,
        max_view_change_rate: 0.2,
        max_lagged_cycle_ratio: 0.35,
        min_committed_tps_ratio: 0.6,
        process_downtime_secs: 5,
        tx_attempted: 300,
        tx_sent: 295,
        tx_submit_errors: 0,
        process_churn_cycles: 4,
        expected_process_churn_cycles: 4,
        process_churn_lagged_cycles: 0,
        membership_join_cycles: 3,
        membership_leave_cycles: 3,
        expected_membership_churn_cycles: 6,
        membership_cleanup_leave: false,
        membership_churn_lagged_cycles: 1,
        membership_churn_warning_cycles: 2,
        churn_paused_secs: 5.0,
        churn_paused_ratio: 1.0 / 12.0,
        soak_overrun_secs: 0.0,
        max_height_skew_observed: 1,
        view_changes_start: 0,
        view_changes_end: 0,
        view_change_rate_per_sec: 0.0,
        scheduled_tps: 5.0,
        submitted_tps: 4.9,
        committed_tps: 4.8,
        committed_txs_min_delta: 288,
        saturated_samples: 0,
        total_samples: 60,
        initial_status_snapshots: vec![norito::json!({"height": 1_u64})],
        final_status_snapshots: vec![norito::json!({"height": 61_u64})],
        no_progress_intervals: vec![NoProgressInterval {
            start_elapsed_ms: 1_000,
            end_elapsed_ms: 2_000,
            classifications: vec!["commit_quorum_missing".to_owned()],
            classified: true,
            status_snapshots: Vec::new(),
        }],
        unclassified_no_progress_intervals: 0,
    }
}
#[test]
fn simulation_summary_json_records_release_profile_and_status_evidence() {
    let summary = sample_simulation_summary();
    let value = summary.to_json_value();
    let object = value
        .as_object()
        .expect("summary must render to JSON object");
    assert_eq!(
        object.get("seed").and_then(norito::json::Value::as_str),
        Some("taira-public-sim")
    );
    assert_eq!(
        object
            .get("workspace_source_manifest_sha256")
            .and_then(norito::json::Value::as_str),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    assert_eq!(
        object
            .get("build_profile")
            .and_then(norito::json::Value::as_str),
        Some("release")
    );
    assert_eq!(
        object
            .get("cargo_net_offline")
            .and_then(norito::json::Value::as_bool),
        Some(true)
    );
    for name in [
        "daemon_binary_blake2b_256",
        "kagami_binary_blake2b_256",
        "test_binary_blake2b_256",
        "generated_config_blake2b_256",
    ] {
        assert_eq!(
            object
                .get(name)
                .and_then(norito::json::Value::as_str)
                .map(str::len),
            Some(64),
            "evidence digest {name}"
        );
    }
    assert_eq!(
        object
            .get("membership_churn_warning_cycles")
            .and_then(norito::json::Value::as_u64),
        Some(2)
    );
    for (name, expected) in [
        ("duration_secs", 60),
        ("target_tps", 5),
        ("packet_loss_percent", 10),
        ("churn_interval_secs", 300),
        ("max_height_skew", 2),
        ("max_height_skew_grace_secs", 30),
        ("max_transient_height_skew", 32),
        ("stall_timeout_secs", 300),
        ("process_downtime_secs", 5),
        ("expected_process_churn_cycles", 4),
        ("expected_membership_churn_cycles", 6),
    ] {
        assert_eq!(
            object.get(name).and_then(norito::json::Value::as_u64),
            Some(expected),
            "profile field {name}"
        );
    }
    for (name, expected) in [
        ("max_view_change_rate", 0.2),
        ("max_lagged_cycle_ratio", 0.35),
        ("min_committed_tps_ratio", 0.6),
    ] {
        assert_eq!(
            object.get(name).and_then(norito::json::Value::as_f64),
            Some(expected),
            "profile field {name}"
        );
    }
    assert_eq!(
        object
            .get("unclassified_no_progress_intervals")
            .and_then(norito::json::Value::as_u64),
        Some(0)
    );
    assert_eq!(
        object
            .get("initial_status_snapshots")
            .and_then(norito::json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        object
            .get("final_status_snapshots")
            .and_then(norito::json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        blocker_label(SumeragiV2LivenessBlocker::ApplicationPending),
        "application_pending"
    );
    assert_eq!(
        blocker_label(SumeragiV2LivenessBlocker::SuccessorActivationPending),
        "successor_activation_pending"
    );
    assert_eq!(
        blocker_label(SumeragiV2LivenessBlocker::LocalControlPending),
        "local_control_pending"
    );
}
#[test]
fn write_summary_persists_local_and_durable_evidence() {
    let temp = tempfile::tempdir().expect("temporary evidence directory");
    let local = temp.path().join("local/summary.json");
    let durable = temp.path().join("durable/taira-summary.json");
    fs::create_dir_all(local.parent().expect("local parent")).expect("create local parent");
    write_summary(&local, &durable, &sample_simulation_summary())
        .expect("write both summary copies");
    let local_bytes = fs::read(&local).expect("read local summary");
    let durable_bytes = fs::read(&durable).expect("read durable summary");
    assert_eq!(local_bytes, durable_bytes);
    assert!(
        String::from_utf8(local_bytes)
            .expect("summary UTF-8")
            .contains("workspace_source_manifest_sha256")
    );
}
include!("taira_public_localnet_config_digest_test.rs");
