//! Orchestration layer that wires configuration, workload generation, and fault injection together.

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex as StdMutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use color_eyre::{
    Result,
    eyre::{WrapErr, eyre},
};
use iroha::client::Client;
use iroha_config::{kura::FsyncMode, parameters::actual::SumeragiNposTimeouts};
use iroha_crypto::{ExposedPrivateKey, KeyPair};
use iroha_data_model::{
    isi::{
        register::RegisterPeerWithPop,
        staking::{ActivatePublicLaneValidator, RegisterPublicLaneValidator},
    },
    parameter::SumeragiParameter,
    parameter::system::SumeragiNposParameters,
    prelude::*,
    query::trigger::prelude::FindTriggers,
    trigger::action::Repeats,
};
use iroha_genesis::GenesisBlock;
use iroha_test_network::{Network, NetworkBuilder, NetworkPeer, Signatory};
use rand::{RngCore, SeedableRng, rngs::StdRng, seq::SliceRandom};
use tokio::{
    sync::{Notify, Semaphore},
    task::{JoinHandle, JoinSet, spawn_blocking},
    time::{self, MissedTickBehavior},
};
use toml::Table;
use tracing::{debug, info, warn};

use crate::{
    config::{ChaosConfig, WorkloadProfile},
    faults::{
        self, CpuStressConfig, DiskSaturationConfig, FaultConfig, NetworkLatencyConfig,
        NetworkPartitionConfig,
    },
    instructions::{
        self, AccountRecord, PlanUpdate, PreparedChaos, TransactionPlan, WorkloadEngine,
    },
};

const IZANAMI_BLOCK_PAYLOAD_QUEUE: i64 = 512;
const IZANAMI_RBC_PENDING_TTL_MS: i64 = 300_000;
const IZANAMI_RBC_SESSION_TTL_MS: i64 = 900_000;
const IZANAMI_RBC_PENDING_MAX_CHUNKS: i64 = 512;
const IZANAMI_RBC_PENDING_MAX_BYTES: i64 = 32 * 1024 * 1024;
const IZANAMI_RBC_PENDING_SESSION_LIMIT: i64 = 512;
const IZANAMI_RBC_REBROADCAST_SESSIONS_PER_TICK: i64 = 12;
const IZANAMI_RBC_PAYLOAD_CHUNKS_PER_TICK: i64 = 96;
const IZANAMI_PACEMAKER_PENDING_STALL_GRACE_MS: i64 = 1_000;
const IZANAMI_PACEMAKER_PENDING_STALL_FLOOR_MS: u64 = 100;
const IZANAMI_SHARED_HOST_SOAK_PENDING_STALL_GRACE_MS: i64 = 300;
const IZANAMI_PACEMAKER_ACTIVE_PENDING_SOFT_LIMIT: i64 = 16;
const IZANAMI_PACEMAKER_RBC_BACKLOG_SESSION_SOFT_LIMIT: i64 = 16;
const IZANAMI_PACEMAKER_RBC_BACKLOG_CHUNK_SOFT_LIMIT: i64 = 256;
const IZANAMI_PACING_GOVERNOR_MIN_FACTOR_BPS: i64 = 10_000;
const IZANAMI_PACING_GOVERNOR_MAX_FACTOR_BPS: i64 = 10_000;
const IZANAMI_COLLECTORS_K: u16 = 4;
const IZANAMI_REDUNDANT_SEND_R: u8 = 4;
const IZANAMI_SHARED_HOST_SOAK_COLLECTORS_K_4_PEERS: u16 = 3;
const IZANAMI_SHARED_HOST_SOAK_REDUNDANT_SEND_R_4_PEERS: u8 = 3;
const IZANAMI_PACING_FACTOR_BPS: u32 = 10_000;
// Shared-host soak profile: bias towards deterministic progress over peak throughput.
const IZANAMI_DA_QUORUM_TIMEOUT_MULTIPLIER: i64 = 1;
const IZANAMI_DA_AVAILABILITY_TIMEOUT_MULTIPLIER: i64 = 1;
const IZANAMI_DA_AVAILABILITY_TIMEOUT_FLOOR_MS: i64 = 750;
const IZANAMI_FUTURE_HEIGHT_WINDOW: i64 = 2;
const IZANAMI_FUTURE_VIEW_WINDOW: i64 = 2;
const IZANAMI_NPOS_BLOCK_TIME_MS: i64 = 120;
const IZANAMI_NPOS_COMMIT_TIME_MS: i64 = 180;
const IZANAMI_RECOVERY_HEIGHT_ATTEMPT_CAP: i64 = 24;
const IZANAMI_RECOVERY_HEIGHT_WINDOW_MS: i64 = 3_000;
const IZANAMI_RECOVERY_MISSING_QC_REACQUIRE_WINDOW_MS: i64 = 1_500;
const IZANAMI_RECOVERY_DEFERRED_QC_TTL_MS: i64 = 3_000;
const IZANAMI_RECOVERY_MISSING_BLOCK_HEIGHT_TTL_MS: i64 = IZANAMI_RECOVERY_HEIGHT_WINDOW_MS;
const IZANAMI_RECOVERY_HASH_MISS_CAP_BEFORE_RANGE_PULL: i64 = 2;
const IZANAMI_RECOVERY_MISSING_BLOCK_SIGNER_FALLBACK_ATTEMPTS: i64 = 1;
const IZANAMI_RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_MULTIPLIER: i64 = 3;
const IZANAMI_RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_CAP_MS: i64 = 8_000;
const IZANAMI_RECOVERY_RANGE_PULL_ESCALATION_AFTER_HASH_MISSES: i64 = 2;
const IZANAMI_NPOS_TIMEOUT_PROPOSE_MIN_MS: u64 = 40;
const IZANAMI_NPOS_TIMEOUT_PREVOTE_MIN_MS: u64 = 60;
const IZANAMI_NPOS_TIMEOUT_PRECOMMIT_MIN_MS: u64 = 80;
const IZANAMI_NPOS_TIMEOUT_COMMIT_MIN_MS: u64 = 1;
const IZANAMI_NPOS_TIMEOUT_DA_MIN_MS: u64 = 1;
const IZANAMI_NPOS_TIMEOUT_AGGREGATOR_MIN_MS: u64 = 1;
const IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_PROPOSE_MIN_MS: u64 = 50;
const IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_PREVOTE_MIN_MS: u64 = 70;
const IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_PRECOMMIT_MIN_MS: u64 = 90;
const IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_COMMIT_MIN_MS: u64 = 220;
const IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_DA_MIN_MS: u64 = 220;
const IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_AGGREGATOR_MIN_MS: u64 = 10;
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
const IZANAMI_KURA_FSYNC_MODE: FsyncMode = FsyncMode::Off;
const IZANAMI_VALIDATION_WORKER_THREADS: i64 = 0;
const IZANAMI_VALIDATION_WORK_QUEUE_CAP: i64 = 0;
const IZANAMI_VALIDATION_RESULT_QUEUE_CAP: i64 = 0;
const IZANAMI_VALIDATION_PENDING_CAP: i64 = 8_192;
const IZANAMI_WORKER_ITERATION_BUDGET_CAP_MS: i64 = 250;
const IZANAMI_WORKER_ITERATION_DRAIN_BUDGET_CAP_MS: i64 = 250;
const IZANAMI_INGRESS_MAX_ATTEMPTS: usize = 3;
const IZANAMI_INGRESS_UNHEALTHY_FAILURE_THRESHOLD: u32 = 2;
const IZANAMI_INGRESS_UNHEALTHY_COOLDOWN_MS: u64 = 5_000;
const IZANAMI_INGRESS_REPROBE_INTERVAL_MS: u64 = 1_000;
const IZANAMI_INGRESS_REQUEST_TIMEOUT_MS: u64 = 5_000;
const IZANAMI_INGRESS_STATUS_TIMEOUT_MS: u64 = 20_000;
const IZANAMI_QUEUE_TIMEOUT_RETRY_ATTEMPTS: u32 = 2;
const IZANAMI_QUEUE_TIMEOUT_RETRY_BACKOFF_MS: u64 = 250;
const IZANAMI_QUEUE_TIMEOUT_ENDPOINT_BACKPRESSURE_RETRY_MULTIPLIER: u32 = 2;
const IZANAMI_WORKER_SHUTDOWN_TIMEOUT_SECS: u64 = 30;
const IZANAMI_WORKER_FAILURE_SHUTDOWN_TIMEOUT_SECS: u64 = 2;
const IZANAMI_PEER_LOG_BASE_LEVEL: &str = "WARN";
const IZANAMI_TELEMETRY_PROFILE: &str = "developer";
const IZANAMI_STRICT_HEIGHT_DIVERGENCE_MAX_BLOCKS: u64 = 16;
const IZANAMI_STRICT_HEIGHT_DIVERGENCE_MAX_WINDOW_SECS: u64 = 60;
const IZANAMI_SHARED_HOST_RECOVERY_MIN_DURATION_SECS: u64 = 1_200;
const IZANAMI_SHARED_HOST_SOAK_MIN_DURATION_SECS: u64 = 3_600;
const IZANAMI_SHARED_HOST_SOAK_TPS_FLOOR: f64 = 5.0;
const IZANAMI_SHARED_HOST_SOAK_MAX_INFLIGHT_FLOOR: usize = 8;
const IZANAMI_SUBMISSION_BACKLOG_MULTIPLIER: usize = 4;
const IZANAMI_SHARED_HOST_SOAK_PROGRESS_TIMEOUT_FLOOR_SECS: u64 = 600;
const IZANAMI_SHARED_HOST_SOAK_PIPELINE_TIME_MS: u64 = 150;
const IZANAMI_SHARED_HOST_SOAK_DA_QUORUM_TIMEOUT_MULTIPLIER: i64 = 1;
const IZANAMI_SHARED_HOST_SOAK_DA_AVAILABILITY_TIMEOUT_MULTIPLIER: i64 = 1;
const IZANAMI_SHARED_HOST_SOAK_DA_AVAILABILITY_TIMEOUT_FLOOR_MS: i64 = 300;
const IZANAMI_SHARED_HOST_SOAK_RECOVERY_HEIGHT_WINDOW_MS: i64 = 2_000;
const IZANAMI_SHARED_HOST_SOAK_RECOVERY_MISSING_QC_REACQUIRE_WINDOW_MS: i64 = 800;
const IZANAMI_SHARED_HOST_SOAK_RECOVERY_DEFERRED_QC_TTL_MS: i64 = 2_000;
const IZANAMI_SHARED_HOST_SOAK_RECOVERY_HASH_MISS_CAP_BEFORE_RANGE_PULL: i64 = 1;
const IZANAMI_SHARED_HOST_SOAK_RECOVERY_MISSING_BLOCK_SIGNER_FALLBACK_ATTEMPTS: i64 = 1;
const IZANAMI_SHARED_HOST_SOAK_RECOVERY_RANGE_PULL_ESCALATION_AFTER_HASH_MISSES: i64 = 1;
const IZANAMI_SHARED_HOST_SOAK_LATENCY_P95_THRESHOLD_SECS: u64 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubmissionConfirmationMode {
    BlockingApplied,
    AcceptedByIngress,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RecoveryProfile {
    da_quorum_timeout_multiplier: i64,
    da_availability_timeout_multiplier: i64,
    da_availability_timeout_floor_ms: i64,
    height_window_ms: i64,
    missing_qc_reacquire_window_ms: i64,
    deferred_qc_ttl_ms: i64,
    missing_block_height_ttl_ms: i64,
    hash_miss_cap_before_range_pull: i64,
    missing_block_signer_fallback_attempts: i64,
    range_pull_escalation_after_hash_misses: i64,
}

fn baseline_recovery_profile() -> RecoveryProfile {
    RecoveryProfile {
        da_quorum_timeout_multiplier: IZANAMI_DA_QUORUM_TIMEOUT_MULTIPLIER,
        da_availability_timeout_multiplier: IZANAMI_DA_AVAILABILITY_TIMEOUT_MULTIPLIER,
        da_availability_timeout_floor_ms: IZANAMI_DA_AVAILABILITY_TIMEOUT_FLOOR_MS,
        height_window_ms: IZANAMI_RECOVERY_HEIGHT_WINDOW_MS,
        missing_qc_reacquire_window_ms: IZANAMI_RECOVERY_MISSING_QC_REACQUIRE_WINDOW_MS,
        deferred_qc_ttl_ms: IZANAMI_RECOVERY_DEFERRED_QC_TTL_MS,
        missing_block_height_ttl_ms: IZANAMI_RECOVERY_MISSING_BLOCK_HEIGHT_TTL_MS,
        hash_miss_cap_before_range_pull: IZANAMI_RECOVERY_HASH_MISS_CAP_BEFORE_RANGE_PULL,
        missing_block_signer_fallback_attempts:
            IZANAMI_RECOVERY_MISSING_BLOCK_SIGNER_FALLBACK_ATTEMPTS,
        range_pull_escalation_after_hash_misses:
            IZANAMI_RECOVERY_RANGE_PULL_ESCALATION_AFTER_HASH_MISSES,
    }
}

fn shared_host_recovery_profile() -> RecoveryProfile {
    RecoveryProfile {
        da_quorum_timeout_multiplier: IZANAMI_SHARED_HOST_SOAK_DA_QUORUM_TIMEOUT_MULTIPLIER,
        da_availability_timeout_multiplier:
            IZANAMI_SHARED_HOST_SOAK_DA_AVAILABILITY_TIMEOUT_MULTIPLIER,
        da_availability_timeout_floor_ms: IZANAMI_SHARED_HOST_SOAK_DA_AVAILABILITY_TIMEOUT_FLOOR_MS,
        height_window_ms: IZANAMI_SHARED_HOST_SOAK_RECOVERY_HEIGHT_WINDOW_MS,
        missing_qc_reacquire_window_ms:
            IZANAMI_SHARED_HOST_SOAK_RECOVERY_MISSING_QC_REACQUIRE_WINDOW_MS,
        deferred_qc_ttl_ms: IZANAMI_SHARED_HOST_SOAK_RECOVERY_DEFERRED_QC_TTL_MS,
        missing_block_height_ttl_ms: IZANAMI_SHARED_HOST_SOAK_RECOVERY_HEIGHT_WINDOW_MS,
        hash_miss_cap_before_range_pull:
            IZANAMI_SHARED_HOST_SOAK_RECOVERY_HASH_MISS_CAP_BEFORE_RANGE_PULL,
        missing_block_signer_fallback_attempts:
            IZANAMI_SHARED_HOST_SOAK_RECOVERY_MISSING_BLOCK_SIGNER_FALLBACK_ATTEMPTS,
        range_pull_escalation_after_hash_misses:
            IZANAMI_SHARED_HOST_SOAK_RECOVERY_RANGE_PULL_ESCALATION_AFTER_HASH_MISSES,
    }
}

#[derive(Clone, Copy, Debug)]
struct IngressEndpointPoolConfig {
    max_attempts: usize,
    unhealthy_failure_threshold: u32,
    unhealthy_cooldown: Duration,
    reprobe_interval: Duration,
}

impl Default for IngressEndpointPoolConfig {
    fn default() -> Self {
        Self {
            max_attempts: IZANAMI_INGRESS_MAX_ATTEMPTS,
            unhealthy_failure_threshold: IZANAMI_INGRESS_UNHEALTHY_FAILURE_THRESHOLD,
            unhealthy_cooldown: Duration::from_millis(IZANAMI_INGRESS_UNHEALTHY_COOLDOWN_MS),
            reprobe_interval: Duration::from_millis(IZANAMI_INGRESS_REPROBE_INTERVAL_MS),
        }
    }
}

#[derive(Default)]
struct IngressStats {
    failover_total: AtomicU64,
    endpoint_unhealthy_total: AtomicU64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct IngressStatsSnapshot {
    failover_total: u64,
    endpoint_unhealthy_total: u64,
}

impl IngressStats {
    fn record_failover(&self) {
        self.failover_total.fetch_add(1, Ordering::Relaxed);
    }

    fn record_endpoint_unhealthy(&self) {
        self.endpoint_unhealthy_total
            .fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> IngressStatsSnapshot {
        IngressStatsSnapshot {
            failover_total: self.failover_total.load(Ordering::Relaxed),
            endpoint_unhealthy_total: self.endpoint_unhealthy_total.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IngressEndpointState {
    Healthy,
    LaggingExcluded,
    QueuePressureCooldown {
        streak: u32,
        next_probe_window: Instant,
    },
    UnhealthyRetryable {
        next_probe_window: Instant,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EndpointHealthState {
    consecutive_failures: u32,
    consecutive_queue_pressure_failures: u32,
    unhealthy_until: Option<Instant>,
    last_probe_at: Option<Instant>,
    sticky_unhealthy_until: Option<Instant>,
    endpoint_state: IngressEndpointState,
}

impl Default for EndpointHealthState {
    fn default() -> Self {
        Self {
            consecutive_failures: 0,
            consecutive_queue_pressure_failures: 0,
            unhealthy_until: None,
            last_probe_at: None,
            sticky_unhealthy_until: None,
            endpoint_state: IngressEndpointState::Healthy,
        }
    }
}

#[derive(Clone, Debug)]
struct IngressLagSnapshot {
    quorum_min_height: u64,
    peer_heights: Vec<Option<u64>>,
    observed_at: Instant,
}

#[derive(Clone)]
struct EndpointHealthPool {
    labels: Arc<Vec<String>>,
    state: Arc<StdMutex<Vec<EndpointHealthState>>>,
    all_unhealthy_probe_at: Arc<StdMutex<Option<Instant>>>,
    lag_snapshot: Arc<StdMutex<Option<IngressLagSnapshot>>>,
    cursor: Arc<AtomicU64>,
    config: IngressEndpointPoolConfig,
    ingress_stats: Arc<IngressStats>,
}

impl EndpointHealthPool {
    fn new(
        labels: Vec<String>,
        config: IngressEndpointPoolConfig,
        ingress_stats: Arc<IngressStats>,
    ) -> Self {
        let len = labels.len();
        Self {
            labels: Arc::new(labels),
            state: Arc::new(StdMutex::new(vec![EndpointHealthState::default(); len])),
            all_unhealthy_probe_at: Arc::new(StdMutex::new(None)),
            lag_snapshot: Arc::new(StdMutex::new(None)),
            cursor: Arc::new(AtomicU64::new(0)),
            config,
            ingress_stats,
        }
    }

    fn update_lag_snapshot(&self, snapshot: IngressLagSnapshot) {
        if let Ok(mut guard) = self.lag_snapshot.lock() {
            *guard = Some(snapshot);
        }
    }

    fn run_with_failover<T, F>(&self, op_name: &'static str, operation: F) -> Result<T>
    where
        F: FnMut(usize, &str) -> Result<T>,
    {
        self.run_with_failover_at(op_name, Instant::now(), operation)
    }

    fn select_endpoint(&self, op_name: &'static str) -> Result<usize> {
        self.select_endpoint_at(op_name, Instant::now())
    }

    fn select_endpoint_at(&self, op_name: &'static str, now: Instant) -> Result<usize> {
        self.attempt_order_at(now)
            .into_iter()
            .next()
            .ok_or_else(|| eyre!("no ingress endpoints available for operation `{op_name}`"))
    }

    fn run_on_endpoint<T, F>(
        &self,
        op_name: &'static str,
        endpoint_idx: usize,
        operation: F,
    ) -> Result<T>
    where
        F: FnOnce(usize, &str) -> Result<T>,
    {
        self.run_on_endpoint_at(op_name, endpoint_idx, Instant::now(), operation)
    }

    fn run_on_endpoint_at<T, F>(
        &self,
        op_name: &'static str,
        endpoint_idx: usize,
        now: Instant,
        operation: F,
    ) -> Result<T>
    where
        F: FnOnce(usize, &str) -> Result<T>,
    {
        let label = self
            .labels
            .get(endpoint_idx)
            .map(String::as_str)
            .ok_or_else(|| eyre!("endpoint index {endpoint_idx} out of range"))?;
        match operation(endpoint_idx, label) {
            Ok(value) => {
                self.mark_success_at(endpoint_idx, now);
                Ok(value)
            }
            Err(err) => {
                let failure_class = classify_ingress_failure(&err);
                let retryable = failure_class.is_retryable();
                let transitioned_unhealthy = self.mark_failure_at(endpoint_idx, now, failure_class);
                if transitioned_unhealthy {
                    self.ingress_stats.record_endpoint_unhealthy();
                    warn!(
                        target: "izanami::ingress",
                        operation = op_name,
                        endpoint = label,
                        failure_class = failure_class.as_str(),
                        "marking pinned ingress endpoint unhealthy"
                    );
                }
                warn!(
                    target: "izanami::ingress",
                    ?err,
                    operation = op_name,
                    endpoint = label,
                    failure_class = failure_class.as_str(),
                    retryable,
                    "pinned ingress endpoint request failed"
                );
                Err(err)
            }
        }
    }

    fn run_with_failover_at<T, F>(
        &self,
        op_name: &'static str,
        now: Instant,
        mut operation: F,
    ) -> Result<T>
    where
        F: FnMut(usize, &str) -> Result<T>,
    {
        let attempt_order = self.attempt_order_at(now);
        if attempt_order.is_empty() {
            return Err(eyre!(
                "no ingress endpoints available for operation `{op_name}`"
            ));
        }
        let max_attempts = self.config.max_attempts.max(1).min(attempt_order.len());
        let mut last_error = None;
        let mut attempted = 0usize;
        for (attempt_idx, endpoint_idx) in attempt_order.into_iter().take(max_attempts).enumerate()
        {
            if attempt_idx > 0 {
                self.ingress_stats.record_failover();
            }
            let label = self
                .labels
                .get(endpoint_idx)
                .map(String::as_str)
                .unwrap_or("<unknown>");
            attempted = attempted.saturating_add(1);
            match operation(endpoint_idx, label) {
                Ok(value) => {
                    self.mark_success_at(endpoint_idx, now);
                    return Ok(value);
                }
                Err(err) => {
                    let failure_class = classify_ingress_failure(&err);
                    let retryable = failure_class.is_retryable();
                    let transitioned_unhealthy =
                        self.mark_failure_at(endpoint_idx, now, failure_class);
                    if transitioned_unhealthy {
                        self.ingress_stats.record_endpoint_unhealthy();
                        warn!(
                            target: "izanami::ingress",
                            operation = op_name,
                            endpoint = label,
                            attempt = attempt_idx + 1,
                            failure_class = failure_class.as_str(),
                            "marking ingress endpoint unhealthy"
                        );
                    }
                    warn!(
                        target: "izanami::ingress",
                        ?err,
                        operation = op_name,
                        endpoint = label,
                        attempt = attempt_idx + 1,
                        failure_class = failure_class.as_str(),
                        retryable,
                        "ingress endpoint request failed"
                    );
                    last_error = Some(err);
                    if !retryable {
                        break;
                    }
                }
            }
        }
        match last_error {
            Some(err) => Err(err).wrap_err_with(|| {
                format!("ingress operation `{op_name}` failed after {attempted} attempt(s)")
            }),
            None => Err(eyre!(
                "ingress operation `{op_name}` failed without making an endpoint attempt"
            )),
        }
    }

    fn probe_due_at(&self, state: &EndpointHealthState, now: Instant) -> bool {
        state
            .last_probe_at
            .is_none_or(|last| now.saturating_duration_since(last) >= self.config.reprobe_interval)
    }

    fn all_unhealthy_probe_slot_available(&self, now: Instant, reserve: bool) -> bool {
        let Ok(mut guard) = self.all_unhealthy_probe_at.lock() else {
            return true;
        };
        let due = guard
            .is_none_or(|last| now.saturating_duration_since(last) >= self.config.reprobe_interval);
        if due && reserve {
            *guard = Some(now);
        }
        due
    }

    fn lagging_flags_at(&self, now: Instant, len: usize) -> Vec<bool> {
        let lag_snapshot = self
            .lag_snapshot
            .lock()
            .ok()
            .and_then(|guard| (*guard).clone())
            .filter(|snapshot| snapshot.peer_heights.len() == len);
        let _lag_snapshot_age = lag_snapshot
            .as_ref()
            .map(|snapshot| now.saturating_duration_since(snapshot.observed_at));
        lag_snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .peer_heights
                    .iter()
                    .map(|height| {
                        height.is_some_and(|height| {
                            snapshot.quorum_min_height.saturating_sub(height)
                                > IZANAMI_STRICT_HEIGHT_DIVERGENCE_MAX_BLOCKS
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec![false; len])
    }

    fn attempt_order_with_state(
        &self,
        state: &mut [EndpointHealthState],
        lagging_flags: &[bool],
        now: Instant,
        base_idx: usize,
        mark_probe_timestamps: bool,
    ) -> Vec<usize> {
        let len = state.len();
        let has_non_lagging_endpoint = lagging_flags.iter().any(|flag| !flag);
        let mut healthy = Vec::with_capacity(len);
        let mut probes = Vec::new();
        let mut sticky_probe_candidates = Vec::new();
        let mut lagging_fallback_healthy = Vec::new();
        let mut lagging_fallback_unhealthy = Vec::new();
        let mut forced_probe: Option<(usize, Instant)> = None;
        for offset in 0..len {
            let idx = (base_idx + offset) % len;
            let excluded_by_lag = has_non_lagging_endpoint && lagging_flags[idx];
            let endpoint_state = &mut state[idx];
            let still_unhealthy = endpoint_state
                .unhealthy_until
                .is_some_and(|until| now < until);
            if excluded_by_lag {
                if !still_unhealthy {
                    endpoint_state.endpoint_state = IngressEndpointState::LaggingExcluded;
                }
                if still_unhealthy {
                    lagging_fallback_unhealthy.push(idx);
                } else {
                    lagging_fallback_healthy.push(idx);
                }
                continue;
            }
            if !still_unhealthy {
                endpoint_state.unhealthy_until = None;
                endpoint_state.sticky_unhealthy_until = None;
                endpoint_state.endpoint_state = IngressEndpointState::Healthy;
                healthy.push(idx);
                continue;
            }
            if self.probe_due_at(endpoint_state, now) {
                let sticky_active = endpoint_state
                    .sticky_unhealthy_until
                    .is_some_and(|until| now < until);
                if sticky_active {
                    if let Some(next_probe_window) = endpoint_state.sticky_unhealthy_until {
                        endpoint_state.endpoint_state =
                            IngressEndpointState::QueuePressureCooldown {
                                streak: endpoint_state.consecutive_queue_pressure_failures.max(1),
                                next_probe_window,
                            };
                    }
                    sticky_probe_candidates.push(idx);
                } else {
                    if let Some(next_probe_window) = endpoint_state.unhealthy_until {
                        endpoint_state.endpoint_state =
                            IngressEndpointState::UnhealthyRetryable { next_probe_window };
                    }
                    probes.push(idx);
                }
            }
            if let Some(unhealthy_until) = endpoint_state.unhealthy_until {
                let should_replace = forced_probe
                    .map(|(_, current_until)| unhealthy_until < current_until)
                    .unwrap_or(true);
                if should_replace {
                    forced_probe = Some((idx, unhealthy_until));
                }
            }
        }
        if healthy.is_empty() {
            if !probes.is_empty() {
                // Keep exactly one unhealthy probe candidate per ordering pass.
                probes.truncate(1);
            } else if let Some(idx) = sticky_probe_candidates.first().copied() {
                probes.push(idx);
            }
        }
        if healthy.is_empty() && probes.is_empty() {
            if let Some(idx) = lagging_fallback_healthy.first().copied() {
                probes.push(idx);
            } else if let Some(idx) = lagging_fallback_unhealthy.first().copied() {
                let probe_due = state
                    .get(idx)
                    .is_some_and(|endpoint_state| self.probe_due_at(endpoint_state, now));
                if probe_due {
                    probes.push(idx);
                }
            } else if let Some((idx, _)) = forced_probe {
                let probe_due = state
                    .get(idx)
                    .is_some_and(|endpoint_state| self.probe_due_at(endpoint_state, now));
                if probe_due {
                    probes.push(idx);
                }
            }
        }
        if healthy.is_empty()
            && !probes.is_empty()
            && !self.all_unhealthy_probe_slot_available(now, mark_probe_timestamps)
        {
            probes.clear();
        }
        if mark_probe_timestamps {
            for idx in &probes {
                if let Some(endpoint_state) = state.get_mut(*idx) {
                    endpoint_state.last_probe_at = Some(now);
                }
            }
        }
        healthy.extend(probes);
        healthy
    }

    fn attempt_order_preview_at(&self, now: Instant) -> Vec<usize> {
        let len = self.labels.len();
        if len == 0 {
            return Vec::new();
        }
        let lagging_flags = self.lagging_flags_at(now, len);
        let base = self.cursor.load(Ordering::Relaxed);
        let base_idx_u64 = base % u64::try_from(len).unwrap_or(1);
        let base_idx = usize::try_from(base_idx_u64).unwrap_or(0);
        let state = self
            .state
            .lock()
            .expect("endpoint health state mutex should not be poisoned")
            .clone();
        let mut preview_state = state;
        self.attempt_order_with_state(
            preview_state.as_mut_slice(),
            &lagging_flags,
            now,
            base_idx,
            false,
        )
    }

    fn submission_backpressure_delay_at(&self, now: Instant) -> Option<Duration> {
        if !self.attempt_order_preview_at(now).is_empty() {
            return None;
        }
        let guard = self
            .state
            .lock()
            .expect("endpoint health state mutex should not be poisoned");
        let mut next_delay: Option<Duration> = None;
        for endpoint_state in guard.iter() {
            if let Some(last_probe_at) = endpoint_state.last_probe_at {
                let elapsed = now.saturating_duration_since(last_probe_at);
                if elapsed < self.config.reprobe_interval {
                    let remaining = self.config.reprobe_interval.saturating_sub(elapsed);
                    next_delay =
                        Some(next_delay.map_or(remaining, |current| current.min(remaining)));
                }
            }
            if let Some(unhealthy_until) = endpoint_state.unhealthy_until
                && now < unhealthy_until
            {
                let remaining = unhealthy_until.saturating_duration_since(now);
                next_delay = Some(next_delay.map_or(remaining, |current| current.min(remaining)));
            }
        }
        Some(
            next_delay
                .unwrap_or_else(|| Duration::from_millis(IZANAMI_QUEUE_TIMEOUT_RETRY_BACKOFF_MS))
                .max(Duration::from_millis(1)),
        )
    }

    fn attempt_order_at(&self, now: Instant) -> Vec<usize> {
        let len = self.labels.len();
        if len == 0 {
            return Vec::new();
        }
        let lagging_flags = self.lagging_flags_at(now, len);
        let base = self.cursor.fetch_add(1, Ordering::Relaxed);
        let base_idx_u64 = base % u64::try_from(len).unwrap_or(1);
        let base_idx = usize::try_from(base_idx_u64).unwrap_or(0);
        let mut guard = self
            .state
            .lock()
            .expect("endpoint health state mutex should not be poisoned");
        self.attempt_order_with_state(guard.as_mut_slice(), &lagging_flags, now, base_idx, true)
    }

    fn mark_success_at(&self, endpoint_idx: usize, now: Instant) {
        if let Ok(mut guard) = self.state.lock() {
            let all_endpoints_unhealthy = guard
                .iter()
                .all(|state| state.unhealthy_until.is_some_and(|until| now < until));
            if let Some(state) = guard.get_mut(endpoint_idx) {
                if state
                    .sticky_unhealthy_until
                    .is_some_and(|until| now < until)
                {
                    if all_endpoints_unhealthy {
                        state.consecutive_failures = 0;
                        state.consecutive_queue_pressure_failures = 0;
                        state.unhealthy_until = None;
                        state.sticky_unhealthy_until = None;
                        state.endpoint_state = IngressEndpointState::Healthy;
                        return;
                    }
                    if let Some(next_probe_window) = state.sticky_unhealthy_until {
                        state.endpoint_state = IngressEndpointState::QueuePressureCooldown {
                            streak: state.consecutive_queue_pressure_failures.max(1),
                            next_probe_window,
                        };
                    }
                    return;
                }
                state.consecutive_failures = 0;
                state.consecutive_queue_pressure_failures = 0;
                state.unhealthy_until = None;
                state.sticky_unhealthy_until = None;
                state.endpoint_state = IngressEndpointState::Healthy;
            }
        }
    }

    fn mark_failure_at(
        &self,
        endpoint_idx: usize,
        now: Instant,
        failure_class: IngressFailureClass,
    ) -> bool {
        let Ok(mut guard) = self.state.lock() else {
            return false;
        };
        let Some(state) = guard.get_mut(endpoint_idx) else {
            return false;
        };
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        if matches!(failure_class, IngressFailureClass::QueuePressure) {
            state.consecutive_queue_pressure_failures =
                state.consecutive_queue_pressure_failures.saturating_add(1);
        } else {
            state.consecutive_queue_pressure_failures = 0;
        }
        if !failure_class.is_retryable() {
            return false;
        }
        let failure_threshold = self.config.unhealthy_failure_threshold;
        if state.consecutive_failures < failure_threshold {
            return false;
        }
        let cooldown = self.config.unhealthy_cooldown;
        let was_unhealthy = state.unhealthy_until.is_some_and(|until| now < until);
        let unhealthy_until = now.checked_add(cooldown).unwrap_or(now);
        state.unhealthy_until = Some(unhealthy_until);
        if matches!(failure_class, IngressFailureClass::QueuePressure) {
            state.sticky_unhealthy_until = Some(unhealthy_until);
            state.endpoint_state = IngressEndpointState::QueuePressureCooldown {
                streak: state.consecutive_queue_pressure_failures.max(1),
                next_probe_window: unhealthy_until,
            };
        } else {
            state.sticky_unhealthy_until = None;
            state.endpoint_state = IngressEndpointState::UnhealthyRetryable {
                next_probe_window: unhealthy_until,
            };
        }
        !was_unhealthy
    }

    #[cfg(test)]
    fn endpoint_state(&self, endpoint_idx: usize) -> EndpointHealthState {
        self.state
            .lock()
            .expect("endpoint health state mutex should not be poisoned")
            .get(endpoint_idx)
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Clone)]
struct IngressEndpoint {
    peer: NetworkPeer,
    label: String,
}

#[derive(Clone)]
struct IngressEndpointPool {
    endpoints: Arc<Vec<IngressEndpoint>>,
    endpoint_index_by_peer: Arc<BTreeMap<PeerId, usize>>,
    health: EndpointHealthPool,
}

impl IngressEndpointPool {
    fn from_peers(
        peers: &[NetworkPeer],
        config: IngressEndpointPoolConfig,
        ingress_stats: Arc<IngressStats>,
    ) -> Self {
        let mut endpoints: Vec<_> = peers
            .iter()
            .cloned()
            .map(|peer| IngressEndpoint {
                label: peer.torii_url(),
                peer,
            })
            .collect();
        endpoints.sort_by(|lhs, rhs| {
            lhs.peer
                .id()
                .cmp(&rhs.peer.id())
                .then_with(|| lhs.label.cmp(&rhs.label))
        });
        let labels = endpoints
            .iter()
            .map(|endpoint| endpoint.label.clone())
            .collect();
        let endpoint_index_by_peer = endpoints
            .iter()
            .enumerate()
            .map(|(idx, endpoint)| (endpoint.peer.id().clone(), idx))
            .collect();
        let health = EndpointHealthPool::new(labels, config, ingress_stats);
        Self {
            endpoints: Arc::new(endpoints),
            endpoint_index_by_peer: Arc::new(endpoint_index_by_peer),
            health,
        }
    }

    fn update_lag_snapshot(
        &self,
        quorum_min_height: u64,
        sampled_heights: &[(PeerId, u64)],
        sampled_at: Instant,
    ) {
        let mut peer_heights = vec![None; self.endpoints.len()];
        for (peer_id, height) in sampled_heights {
            if let Some(endpoint_idx) = self.endpoint_index_by_peer.get(peer_id)
                && let Some(slot) = peer_heights.get_mut(*endpoint_idx)
            {
                *slot = Some(*height);
            }
        }
        self.health.update_lag_snapshot(IngressLagSnapshot {
            quorum_min_height,
            peer_heights,
            observed_at: sampled_at,
        });
    }

    fn run_with_failover<T, F>(&self, op_name: &'static str, mut operation: F) -> Result<T>
    where
        F: FnMut(&NetworkPeer) -> Result<T>,
    {
        let endpoints = Arc::clone(&self.endpoints);
        self.health
            .run_with_failover(op_name, move |endpoint_idx, _label| {
                let endpoint = endpoints
                    .get(endpoint_idx)
                    .ok_or_else(|| eyre!("endpoint index {endpoint_idx} out of range"))?;
                operation(&endpoint.peer)
            })
    }

    fn select_endpoint(&self, op_name: &'static str) -> Result<usize> {
        self.health.select_endpoint(op_name)
    }

    fn run_on_endpoint<T, F>(
        &self,
        op_name: &'static str,
        endpoint_idx: usize,
        operation: F,
    ) -> Result<T>
    where
        F: FnOnce(&NetworkPeer) -> Result<T>,
    {
        let endpoints = Arc::clone(&self.endpoints);
        self.health
            .run_on_endpoint(op_name, endpoint_idx, move |idx, _label| {
                let endpoint = endpoints
                    .get(idx)
                    .ok_or_else(|| eyre!("endpoint index {idx} out of range"))?;
                operation(&endpoint.peer)
            })
    }

    fn submission_backpressure_delay(&self, now: Instant) -> Option<Duration> {
        self.health.submission_backpressure_delay_at(now)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IngressFailureClass {
    NonRetryable,
    Retryable,
    QueuePressure,
}

impl IngressFailureClass {
    const fn is_retryable(self) -> bool {
        !matches!(self, Self::NonRetryable)
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::NonRetryable => "non_retryable",
            Self::Retryable => "retryable",
            Self::QueuePressure => "queue_pressure",
        }
    }
}

fn ingress_error_message(error: &color_eyre::Report) -> String {
    format!("{error:#}").to_ascii_lowercase()
}

fn is_idempotent_duplicate_submission_message(message: &str) -> bool {
    message.contains("repeated instruction")
        || message.contains("repetition of")
        || message.contains("already exists")
}

fn is_idempotent_duplicate_submission(error: &color_eyre::Report) -> bool {
    let message = ingress_error_message(error);
    is_idempotent_duplicate_submission_message(&message)
}

fn is_ingress_queue_pressure_message(message: &str) -> bool {
    message.contains("transaction queued for too long")
        || message.contains("status_timeout_ms")
        || message.contains("haven't got tx confirmation within")
        || contains_http_429_status(message)
}

fn is_ingress_endpoint_backpressure_message(message: &str) -> bool {
    message.contains("no ingress endpoints available for operation")
}

fn classify_ingress_failure(error: &color_eyre::Report) -> IngressFailureClass {
    let message = ingress_error_message(error);
    if is_ingress_queue_pressure_message(&message) {
        return IngressFailureClass::QueuePressure;
    }
    if message.contains("timed out")
        || message.contains("timeout")
        || message.contains("connection refused")
        || message.contains("connection reset")
        || message.contains("broken pipe")
        || contains_http_5xx_status(&message)
    {
        IngressFailureClass::Retryable
    } else {
        IngressFailureClass::NonRetryable
    }
}

#[cfg(test)]
fn is_ingress_failover_retryable(error: &color_eyre::Report) -> bool {
    classify_ingress_failure(error).is_retryable()
}

fn is_ingress_queue_timeout_retryable(error: &color_eyre::Report) -> bool {
    let message = ingress_error_message(error);
    is_ingress_queue_pressure_message(&message)
        || is_ingress_endpoint_backpressure_message(&message)
}

fn queue_timeout_retry_delay(
    backoff: Duration,
    endpoint_backpressure: bool,
    dynamic_backpressure_delay: Option<Duration>,
) -> Duration {
    if endpoint_backpressure {
        backoff.max(
            dynamic_backpressure_delay
                .unwrap_or_else(|| Duration::from_millis(IZANAMI_INGRESS_REPROBE_INTERVAL_MS)),
        )
    } else {
        backoff
    }
}

#[cfg(test)]
fn run_with_queue_timeout_retry_with_policy<F>(
    plan_label: &'static str,
    max_retry_attempts: u32,
    initial_backoff: Duration,
    submit: F,
) -> Result<()>
where
    F: FnMut() -> Result<()>,
{
    run_with_queue_timeout_retry_with_policy_and_delay(
        plan_label,
        max_retry_attempts,
        initial_backoff,
        || None,
        submit,
    )
}

fn run_with_queue_timeout_retry_with_policy_and_delay<F, G>(
    plan_label: &'static str,
    max_retry_attempts: u32,
    initial_backoff: Duration,
    mut no_endpoint_backpressure_delay: G,
    mut submit: F,
) -> Result<()>
where
    F: FnMut() -> Result<()>,
    G: FnMut() -> Option<Duration>,
{
    let mut backoff = initial_backoff;
    let mut retryable_attempts = 0_u32;
    let mut endpoint_backpressure_retries = 0_u32;
    let max_endpoint_backpressure_retries = max_retry_attempts
        .saturating_mul(IZANAMI_QUEUE_TIMEOUT_ENDPOINT_BACKPRESSURE_RETRY_MULTIPLIER)
        .max(1);
    loop {
        match submit() {
            Ok(()) => return Ok(()),
            Err(err) if is_idempotent_duplicate_submission(&err) => {
                warn!(
                    target: "izanami::workload",
                    plan = plan_label,
                    ?err,
                    "treating duplicate submission rejection as idempotent success"
                );
                return Ok(());
            }
            Err(err) if is_ingress_queue_timeout_retryable(&err) => {
                let error_message = ingress_error_message(&err);
                let endpoint_backpressure =
                    is_ingress_endpoint_backpressure_message(&error_message);
                let retry_budget_available = if endpoint_backpressure {
                    endpoint_backpressure_retries < max_endpoint_backpressure_retries
                } else {
                    retryable_attempts < max_retry_attempts
                };
                if !retry_budget_available {
                    return Err(err);
                }
                let dynamic_backpressure_delay = if endpoint_backpressure {
                    no_endpoint_backpressure_delay()
                } else {
                    None
                };
                if endpoint_backpressure {
                    endpoint_backpressure_retries = endpoint_backpressure_retries.saturating_add(1);
                } else {
                    retryable_attempts = retryable_attempts.saturating_add(1);
                }
                let retry_in = queue_timeout_retry_delay(
                    backoff,
                    endpoint_backpressure,
                    dynamic_backpressure_delay,
                );
                let next_attempt = retryable_attempts
                    .saturating_add(endpoint_backpressure_retries)
                    .saturating_add(1);
                warn!(
                    target: "izanami::workload",
                    plan = plan_label,
                    next_attempt,
                    max_attempts = max_retry_attempts
                        .saturating_add(max_endpoint_backpressure_retries)
                        .saturating_add(1),
                    endpoint_backpressure,
                    ?dynamic_backpressure_delay,
                    ?retry_in,
                    ?err,
                    "submission ingress backpressure observed; retrying plan submission"
                );
                if !retry_in.is_zero() {
                    std::thread::sleep(retry_in);
                }
                backoff = backoff.saturating_mul(2);
            }
            Err(err) => return Err(err),
        }
    }
}

fn contains_http_5xx_status(message: &str) -> bool {
    (500..=599).any(|status| {
        let code = status.to_string();
        message.contains(&format!("status code: {code}"))
            || message.contains(&format!("status: {code}"))
            || message.contains(&format!("http {code}"))
            || message.contains(&format!(" {code} "))
    })
}

fn contains_http_429_status(message: &str) -> bool {
    message.contains("429 too many requests")
        || message.contains("status code: 429")
        || message.contains("status: 429")
        || message.contains("http 429")
}

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

#[derive(Clone, Debug, PartialEq, Eq)]
struct NposGenesisPreflightSummary {
    peer_with_pop_count: usize,
    register_validator_count: usize,
    activate_validator_count: usize,
    min_self_bond: u64,
    stake_distribution: Vec<(u64, usize)>,
}

#[derive(Clone, Copy, Debug)]
struct NposTimeoutFloors {
    propose_ms: u64,
    prevote_ms: u64,
    precommit_ms: u64,
    commit_ms: u64,
    da_ms: u64,
    aggregator_ms: u64,
}

fn clamp_nonzero_ms(value: u64) -> u64 {
    value.max(1)
}

fn pending_stall_grace_ms(block_ms: u64) -> i64 {
    let scaled = block_ms
        .saturating_div(2)
        .max(IZANAMI_PACEMAKER_PENDING_STALL_FLOOR_MS);
    let capped =
        scaled.min(u64::try_from(IZANAMI_PACEMAKER_PENDING_STALL_GRACE_MS).unwrap_or(u64::MAX));
    i64::try_from(capped).unwrap_or(i64::MAX)
}

fn is_shared_host_balanced_latency_profile(config: &ChaosConfig) -> bool {
    is_shared_host_stable_recovery_run(config)
}

fn npos_timeout_floors(config: &ChaosConfig) -> NposTimeoutFloors {
    if is_shared_host_balanced_latency_profile(config) {
        NposTimeoutFloors {
            propose_ms: IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_PROPOSE_MIN_MS,
            prevote_ms: IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_PREVOTE_MIN_MS,
            precommit_ms: IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_PRECOMMIT_MIN_MS,
            commit_ms: IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_COMMIT_MIN_MS,
            da_ms: IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_DA_MIN_MS,
            aggregator_ms: IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_AGGREGATOR_MIN_MS,
        }
    } else {
        NposTimeoutFloors {
            propose_ms: IZANAMI_NPOS_TIMEOUT_PROPOSE_MIN_MS,
            prevote_ms: IZANAMI_NPOS_TIMEOUT_PREVOTE_MIN_MS,
            precommit_ms: IZANAMI_NPOS_TIMEOUT_PRECOMMIT_MIN_MS,
            commit_ms: IZANAMI_NPOS_TIMEOUT_COMMIT_MIN_MS,
            da_ms: IZANAMI_NPOS_TIMEOUT_DA_MIN_MS,
            aggregator_ms: IZANAMI_NPOS_TIMEOUT_AGGREGATOR_MIN_MS,
        }
    }
}

fn npos_pending_stall_grace_ms(config: &ChaosConfig, block_ms: u64) -> i64 {
    if is_shared_host_balanced_latency_profile(config) {
        IZANAMI_SHARED_HOST_SOAK_PENDING_STALL_GRACE_MS
    } else {
        pending_stall_grace_ms(block_ms)
    }
}

fn npos_collectors_and_redundancy(config: &ChaosConfig) -> (u16, u8) {
    if is_shared_host_balanced_latency_profile(config) && config.peer_count == 4 {
        (
            IZANAMI_SHARED_HOST_SOAK_COLLECTORS_K_4_PEERS,
            IZANAMI_SHARED_HOST_SOAK_REDUNDANT_SEND_R_4_PEERS,
        )
    } else {
        (IZANAMI_COLLECTORS_K, IZANAMI_REDUNDANT_SEND_R)
    }
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
    let (block_ms, commit_time_ms, timeout_block_ms, clamp_commit) =
        if let Some(duration) = config.pipeline_time {
            let (block_ms, _commit_ms) = split_pipeline_time(duration);
            // Favor block cadence for soak tests; commit time must be >= block time.
            // Use the block cadence for timeout derivation to avoid over-eager reschedules.
            (block_ms, block_ms, block_ms, true)
        } else {
            let block_ms = u64::try_from(IZANAMI_NPOS_BLOCK_TIME_MS)
                .expect("izanami block time must be non-negative");
            let commit_ms = u64::try_from(IZANAMI_NPOS_COMMIT_TIME_MS)
                .expect("izanami commit time must be non-negative");
            (block_ms, commit_ms, block_ms, true)
        };
    let block_ms = clamp_nonzero_ms(block_ms);
    let commit_time_ms = clamp_nonzero_ms(commit_time_ms);
    let timeout_floors = npos_timeout_floors(config);
    // Derive per-phase timeouts from the scaled block time to keep soak cadence tight.
    let timeouts = SumeragiNposTimeouts::from_block_time(Duration::from_millis(timeout_block_ms));
    let propose_ms = clamp_nonzero_ms(duration_ms(timeouts.propose)).max(timeout_floors.propose_ms);
    let prevote_ms = clamp_nonzero_ms(duration_ms(timeouts.prevote)).max(timeout_floors.prevote_ms);
    let precommit_ms =
        clamp_nonzero_ms(duration_ms(timeouts.precommit)).max(timeout_floors.precommit_ms);
    // Keep commit/DA windows at least as large as the target commit time for DA stability.
    let mut commit_timeout_ms = clamp_nonzero_ms(duration_ms(timeouts.commit));
    let mut da_ms = clamp_nonzero_ms(duration_ms(timeouts.da));
    if clamp_commit {
        commit_timeout_ms = commit_timeout_ms.max(commit_time_ms);
        da_ms = da_ms.max(commit_time_ms);
    }
    commit_timeout_ms = commit_timeout_ms.max(timeout_floors.commit_ms);
    da_ms = da_ms.max(timeout_floors.da_ms);
    let aggregator_ms =
        clamp_nonzero_ms(duration_ms(timeouts.aggregator)).max(timeout_floors.aggregator_ms);
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

fn default_nexus_pipeline_time() -> Duration {
    let block_ms =
        u64::try_from(IZANAMI_NPOS_BLOCK_TIME_MS).expect("izanami block time must be non-negative");
    let commit_ms = u64::try_from(IZANAMI_NPOS_COMMIT_TIME_MS)
        .expect("izanami commit time must be non-negative");
    Duration::from_millis(block_ms.saturating_add(commit_ms))
}

fn default_izanami_pipeline_time() -> Duration {
    default_nexus_pipeline_time()
}

fn sumeragi_phase_operator_keypair() -> KeyPair {
    Signatory::Peer.key_pair().clone()
}

fn izanami_npos_parameters(peer_count: usize) -> SumeragiNposParameters {
    let mut params = SumeragiNposParameters::default();
    params.max_validators = u32::try_from(peer_count.max(1)).unwrap_or(u32::MAX);
    params
}

fn npos_min_self_bond_from_genesis(genesis: &GenesisBlock) -> u64 {
    let mut params = Parameters::default();
    for tx in genesis.0.transactions_vec() {
        let Executable::Instructions(instructions) = tx.instructions() else {
            continue;
        };
        for instruction in instructions {
            let Some(set_param) = instruction.as_any().downcast_ref::<SetParameter>() else {
                continue;
            };
            params.set_parameter(set_param.inner().clone());
        }
    }
    params
        .custom()
        .get(&SumeragiNposParameters::parameter_id())
        .and_then(SumeragiNposParameters::from_custom_parameter)
        .unwrap_or_default()
        .min_self_bond()
}

#[allow(single_use_lifetimes)]
fn audit_npos_preflight_instructions<'a>(
    instructions: impl IntoIterator<Item = &'a InstructionBox>,
    peer_count: usize,
    min_self_bond: u64,
) -> Result<NposGenesisPreflightSummary> {
    let expected_peers = peer_count.max(1);
    let mut peer_with_pop_count = 0usize;
    let mut register_validator_count = 0usize;
    let mut activate_validator_count = 0usize;
    let mut validator_stakes = BTreeMap::<AccountId, u64>::new();
    let mut activated_validators = BTreeSet::<AccountId>::new();

    for instruction in instructions {
        if instruction
            .as_any()
            .downcast_ref::<RegisterPeerWithPop>()
            .is_some()
        {
            peer_with_pop_count = peer_with_pop_count.saturating_add(1);
        }
        if let Some(register) = instruction
            .as_any()
            .downcast_ref::<RegisterPublicLaneValidator>()
        {
            register_validator_count = register_validator_count.saturating_add(1);
            let stake = u64::try_from(register.initial_stake.clone()).map_err(|_| {
                eyre!(
                    "Izanami NPoS preflight failed: validator {} has non-integer initial_stake {}",
                    register.validator,
                    register.initial_stake
                )
            })?;
            if stake < min_self_bond {
                return Err(eyre!(
                    "Izanami NPoS preflight failed: validator {} initial_stake={} below min_self_bond={}",
                    register.validator,
                    stake,
                    min_self_bond
                ));
            }
            validator_stakes.insert(register.validator.clone(), stake);
        }
        if let Some(activate) = instruction
            .as_any()
            .downcast_ref::<ActivatePublicLaneValidator>()
        {
            activate_validator_count = activate_validator_count.saturating_add(1);
            activated_validators.insert(activate.validator.clone());
        }
    }

    if peer_with_pop_count != expected_peers {
        return Err(eyre!(
            "Izanami NPoS preflight failed: RegisterPeerWithPop count={} expected={}",
            peer_with_pop_count,
            expected_peers
        ));
    }
    if register_validator_count != expected_peers {
        return Err(eyre!(
            "Izanami NPoS preflight failed: RegisterPublicLaneValidator count={} expected={}",
            register_validator_count,
            expected_peers
        ));
    }
    if activate_validator_count != expected_peers {
        return Err(eyre!(
            "Izanami NPoS preflight failed: ActivatePublicLaneValidator count={} expected={}",
            activate_validator_count,
            expected_peers
        ));
    }

    let registered_validators: BTreeSet<_> = validator_stakes.keys().cloned().collect();
    let missing_activation: Vec<_> = registered_validators
        .difference(&activated_validators)
        .cloned()
        .collect();
    if !missing_activation.is_empty() {
        return Err(eyre!(
            "Izanami NPoS preflight failed: validator activation missing for {} account(s)",
            missing_activation.len()
        ));
    }
    let unexpected_activation: Vec<_> = activated_validators
        .difference(&registered_validators)
        .cloned()
        .collect();
    if !unexpected_activation.is_empty() {
        return Err(eyre!(
            "Izanami NPoS preflight failed: activation references {} unregistered validator account(s)",
            unexpected_activation.len()
        ));
    }

    let mut stake_distribution = BTreeMap::<u64, usize>::new();
    for stake in validator_stakes.values().copied() {
        *stake_distribution.entry(stake).or_insert(0) += 1;
    }
    if stake_distribution.len() != 1 {
        return Err(eyre!(
            "Izanami NPoS preflight failed: validator initial_stake distribution is non-uniform: {:?}",
            stake_distribution
        ));
    }

    Ok(NposGenesisPreflightSummary {
        peer_with_pop_count,
        register_validator_count,
        activate_validator_count,
        min_self_bond,
        stake_distribution: stake_distribution.into_iter().collect(),
    })
}

fn audit_npos_genesis_preflight(
    genesis: &GenesisBlock,
    peer_count: usize,
) -> Result<NposGenesisPreflightSummary> {
    let min_self_bond = npos_min_self_bond_from_genesis(genesis);
    let mut instructions = Vec::<InstructionBox>::new();
    for tx in genesis.0.transactions_vec() {
        let Executable::Instructions(tx_instructions) = tx.instructions() else {
            continue;
        };
        instructions.extend(tx_instructions.iter().cloned());
    }
    audit_npos_preflight_instructions(instructions.iter(), peer_count, min_self_bond)
}

fn is_shared_host_stable_soak(config: &ChaosConfig) -> bool {
    matches!(config.workload_profile, WorkloadProfile::Stable)
        && config.faulty_peers <= 1
        && config.peer_count >= 4
        && config.duration >= Duration::from_secs(IZANAMI_SHARED_HOST_SOAK_MIN_DURATION_SECS)
}

fn is_shared_host_stable_recovery_run(config: &ChaosConfig) -> bool {
    matches!(config.workload_profile, WorkloadProfile::Stable)
        && config.faulty_peers <= 1
        && config.peer_count >= 4
        && config.duration >= Duration::from_secs(IZANAMI_SHARED_HOST_RECOVERY_MIN_DURATION_SECS)
}

fn submission_confirmation_mode(config: &ChaosConfig) -> SubmissionConfirmationMode {
    if matches!(config.workload_profile, WorkloadProfile::Stable) && config.faulty_peers == 0 {
        SubmissionConfirmationMode::AcceptedByIngress
    } else {
        SubmissionConfirmationMode::BlockingApplied
    }
}

fn recovery_profile_for(config: &ChaosConfig) -> RecoveryProfile {
    if is_shared_host_stable_recovery_run(config) {
        shared_host_recovery_profile()
    } else {
        baseline_recovery_profile()
    }
}

fn apply_shared_host_stable_soak_profile(config: &mut ChaosConfig) {
    if !is_shared_host_stable_soak(config) {
        return;
    }

    let original_tps = config.tps;
    let original_max_inflight = config.max_inflight;
    let original_progress_timeout = config.progress_timeout;
    let original_pipeline_time = config.pipeline_time;
    let original_latency_p95_threshold = config.latency_p95_threshold;

    // Preserve operator-selected stress settings while enforcing a minimum load floor
    // for shared-host soaks so runs remain comparable at low load.
    config.tps = config.tps.max(IZANAMI_SHARED_HOST_SOAK_TPS_FLOOR);
    // Keep enough in-flight room to sustain the configured TPS without permit starvation.
    let inflight_floor_from_tps = usize::try_from((config.tps * 2.0).ceil() as u64).unwrap_or(1);
    config.max_inflight = config
        .max_inflight
        .max(IZANAMI_SHARED_HOST_SOAK_MAX_INFLIGHT_FLOOR)
        .max(inflight_floor_from_tps.max(1));
    config.progress_timeout = config.progress_timeout.max(Duration::from_secs(
        IZANAMI_SHARED_HOST_SOAK_PROGRESS_TIMEOUT_FLOOR_SECS,
    ));
    let pipeline_time_floor = Duration::from_millis(IZANAMI_SHARED_HOST_SOAK_PIPELINE_TIME_MS);
    config.pipeline_time = Some(
        config
            .pipeline_time
            .map_or(pipeline_time_floor, |existing| {
                existing.max(pipeline_time_floor)
            }),
    );
    if config.latency_p95_threshold.is_none() {
        config.latency_p95_threshold = Some(Duration::from_secs(
            IZANAMI_SHARED_HOST_SOAK_LATENCY_P95_THRESHOLD_SECS,
        ));
    }

    if (config.tps - original_tps).abs() > f64::EPSILON
        || config.max_inflight != original_max_inflight
        || config.progress_timeout != original_progress_timeout
        || config.pipeline_time != original_pipeline_time
        || config.latency_p95_threshold != original_latency_p95_threshold
    {
        info!(
            target: "izanami::profile",
            peers = config.peer_count,
            duration_secs = config.duration.as_secs(),
            target_blocks = config.target_blocks.unwrap_or_default(),
            original_tps,
            tuned_tps = config.tps,
            original_max_inflight,
            tuned_max_inflight = config.max_inflight,
            original_progress_timeout_secs = original_progress_timeout.as_secs(),
            tuned_progress_timeout_secs = config.progress_timeout.as_secs(),
            original_pipeline_time_ms = original_pipeline_time
                .map(|duration| duration.as_millis())
                .unwrap_or_default(),
            tuned_pipeline_time_ms = config
                .pipeline_time
                .map(|duration| duration.as_millis())
                .unwrap_or_default(),
            original_latency_p95_threshold_ms = original_latency_p95_threshold
                .map(|duration| duration.as_millis())
                .unwrap_or_default(),
            tuned_latency_p95_threshold_ms = config
                .latency_p95_threshold
                .map(|duration| duration.as_millis())
                .unwrap_or_default(),
            "applied shared-host stable soak profile for deterministic long-run progress"
        );
    }
}

fn consensus_mode_label(config: &ChaosConfig) -> &'static str {
    if config.nexus.is_some() {
        "npos"
    } else {
        "permissioned"
    }
}

fn log_effective_consensus_soak_overrides(config: &ChaosConfig) {
    let recovery_profile = recovery_profile_for(config);
    let npos_timing = derive_npos_timing(config);
    let balanced_latency_profile = is_shared_host_balanced_latency_profile(config);
    let pending_stall_grace_ms = npos_pending_stall_grace_ms(config, npos_timing.block_ms);
    let (collectors_k, redundant_send_r) = npos_collectors_and_redundancy(config);
    let latency_profile = if balanced_latency_profile {
        "shared_host_balanced_sub_1s"
    } else {
        "default"
    };
    let latency_p95_gate_ms = config
        .latency_p95_threshold
        .map(|threshold| u64::try_from(threshold.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default();
    info!(
        target: "izanami::profile",
        consensus_mode = consensus_mode_label(config),
        shared_host_consensus_profile = is_shared_host_stable_recovery_run(config),
        shared_host_stable_soak = is_shared_host_stable_soak(config),
        latency_profile,
        latency_p95_gate_configured = config.latency_p95_threshold.is_some(),
        latency_p95_gate_ms,
        pending_stall_grace_ms,
        da_fast_reschedule = balanced_latency_profile,
        collectors_k,
        redundant_send_r,
        recovery_height_window_ms = recovery_profile.height_window_ms,
        recovery_missing_qc_reacquire_window_ms = recovery_profile.missing_qc_reacquire_window_ms,
        recovery_deferred_qc_ttl_ms = recovery_profile.deferred_qc_ttl_ms,
        recovery_missing_block_height_ttl_ms = recovery_profile.missing_block_height_ttl_ms,
        recovery_hash_miss_cap_before_range_pull = recovery_profile.hash_miss_cap_before_range_pull,
        recovery_missing_block_signer_fallback_attempts = recovery_profile
            .missing_block_signer_fallback_attempts,
        recovery_range_pull_escalation_after_hash_misses = recovery_profile
            .range_pull_escalation_after_hash_misses,
        "effective consensus soak overrides"
    );
}

fn make_network_builder(config: &ChaosConfig, genesis: Vec<Vec<InstructionBox>>) -> NetworkBuilder {
    let mut genesis = genesis;
    let recovery_profile = recovery_profile_for(config);
    let phase_operator_keypair = sumeragi_phase_operator_keypair();
    let torii_receipt_public_key = phase_operator_keypair.public_key().to_string();
    let torii_receipt_private_key =
        ExposedPrivateKey(phase_operator_keypair.private_key().clone()).to_string();
    let mut builder = NetworkBuilder::new()
        .with_peers(config.peer_count)
        .with_base_seed(instructions::IZANAMI_BASE_SEED);
    let npos_params = izanami_npos_parameters(config.peer_count);
    let pipeline_time = config
        .pipeline_time
        .unwrap_or_else(default_izanami_pipeline_time);
    let (pipeline_block_ms, pipeline_commit_ms) = split_pipeline_time(pipeline_time);
    let min_finality_floor = iroha_data_model::parameter::SumeragiParameters::default()
        .min_finality_ms()
        .max(1);
    let needs_manual_pipeline_genesis = pipeline_block_ms < min_finality_floor;
    if needs_manual_pipeline_genesis {
        builder = builder.with_default_pipeline_time();
        let timing_prefix = vec![
            InstructionBox::from(SetParameter::new(Parameter::Sumeragi(
                SumeragiParameter::CommitTimeMs(pipeline_commit_ms),
            ))),
            InstructionBox::from(SetParameter::new(Parameter::Sumeragi(
                SumeragiParameter::MinFinalityMs(pipeline_block_ms),
            ))),
            InstructionBox::from(SetParameter::new(Parameter::Sumeragi(
                SumeragiParameter::BlockTimeMs(pipeline_block_ms),
            ))),
        ];
        if let Some(first_tx) = genesis.first_mut() {
            first_tx.splice(0..0, timing_prefix);
        } else {
            genesis.push(timing_prefix);
        }
    } else {
        builder = builder.with_pipeline_time(pipeline_time);
    }
    if let Some(profile) = &config.nexus {
        builder = builder
            .with_data_availability_enabled(profile.da_enabled)
            .with_config_table(profile.config_layer.clone());
        let gas_account_id = instructions::nexus_gas_account_id().to_string();
        builder = builder.with_config_layer(move |layer| {
            layer
                .write(
                    ["pipeline", "gas", "tech_account_id"],
                    gas_account_id.clone(),
                )
                .write(
                    ["nexus", "fees", "fee_sink_account_id"],
                    gas_account_id.clone(),
                )
                .write(
                    ["nexus", "staking", "stake_escrow_account_id"],
                    gas_account_id.clone(),
                )
                .write(
                    ["nexus", "staking", "slash_sink_account_id"],
                    gas_account_id.clone(),
                );
        });
    }
    if config.nexus.is_some() {
        builder = builder.without_npos_genesis_bootstrap();
        builder =
            builder.with_genesis_post_topology_isi(instructions::npos_post_topology_instructions(
                config.peer_count,
                npos_params.min_self_bond(),
            ));
    }
    if let Ok(filter) = std::env::var("RUST_LOG") {
        let filter = filter.trim();
        if !filter.is_empty() {
            let filter = filter.to_string();
            // Keep peer logs sparse by default, while still allowing targeted directives via RUST_LOG.
            builder = builder.with_config_layer(|layer| {
                layer
                    .write(["logger", "level"], IZANAMI_PEER_LOG_BASE_LEVEL)
                    .write(["logger", "filter"], filter);
            });
        }
    }
    // Inject Izanami timing into on-chain Sumeragi parameters.
    let npos_timing = derive_npos_timing(config);
    let (collectors_k, redundant_send_r) = npos_collectors_and_redundancy(config);
    if config.nexus.is_some() {
        let mut injected = Vec::new();
        injected.push(InstructionBox::from(SetParameter::new(
            Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(npos_timing.commit_time_ms)),
        )));
        injected.push(InstructionBox::from(SetParameter::new(
            Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(npos_timing.block_ms)),
        )));
        injected.push(InstructionBox::from(SetParameter::new(
            Parameter::Sumeragi(SumeragiParameter::PacingFactorBps(
                IZANAMI_PACING_FACTOR_BPS,
            )),
        )));
        injected.push(InstructionBox::from(SetParameter::new(
            Parameter::Sumeragi(SumeragiParameter::CollectorsK(collectors_k)),
        )));
        injected.push(InstructionBox::from(SetParameter::new(
            Parameter::Sumeragi(SumeragiParameter::RedundantSendR(redundant_send_r)),
        )));
        injected.push(InstructionBox::from(SetParameter::new(Parameter::Custom(
            npos_params.into_custom_parameter(),
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
    builder = builder.with_config_layer(move |layer| {
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
                ["torii", "receipt_public_key"],
                torii_receipt_public_key.clone(),
            )
            .write(
                ["torii", "receipt_private_key"],
                torii_receipt_private_key.clone(),
            )
            .write(["telemetry_profile"], IZANAMI_TELEMETRY_PROFILE)
            .write(["kura", "fsync_mode"], IZANAMI_KURA_FSYNC_MODE.to_string())
            .write(
                ["sumeragi", "advanced", "queues", "block_payload"],
                IZANAMI_BLOCK_PAYLOAD_QUEUE,
            )
            .write(
                [
                    "sumeragi",
                    "advanced",
                    "worker",
                    "validation_worker_threads",
                ],
                IZANAMI_VALIDATION_WORKER_THREADS,
            )
            .write(
                [
                    "sumeragi",
                    "advanced",
                    "worker",
                    "validation_work_queue_cap",
                ],
                IZANAMI_VALIDATION_WORK_QUEUE_CAP,
            )
            .write(
                [
                    "sumeragi",
                    "advanced",
                    "worker",
                    "validation_result_queue_cap",
                ],
                IZANAMI_VALIDATION_RESULT_QUEUE_CAP,
            )
            .write(
                ["sumeragi", "advanced", "worker", "validation_pending_cap"],
                IZANAMI_VALIDATION_PENDING_CAP,
            )
            .write(
                ["sumeragi", "advanced", "worker", "iteration_budget_cap_ms"],
                IZANAMI_WORKER_ITERATION_BUDGET_CAP_MS,
            )
            .write(
                [
                    "sumeragi",
                    "advanced",
                    "worker",
                    "iteration_drain_budget_cap_ms",
                ],
                IZANAMI_WORKER_ITERATION_DRAIN_BUDGET_CAP_MS,
            )
            .write(
                [
                    "sumeragi",
                    "advanced",
                    "pacemaker",
                    "pending_stall_grace_ms",
                ],
                npos_pending_stall_grace_ms(config, npos_timing.block_ms),
            )
            .write(
                ["sumeragi", "advanced", "pacemaker", "da_fast_reschedule"],
                is_shared_host_balanced_latency_profile(config),
            )
            .write(
                [
                    "sumeragi",
                    "advanced",
                    "pacemaker",
                    "active_pending_soft_limit",
                ],
                IZANAMI_PACEMAKER_ACTIVE_PENDING_SOFT_LIMIT,
            )
            .write(
                [
                    "sumeragi",
                    "advanced",
                    "pacemaker",
                    "rbc_backlog_session_soft_limit",
                ],
                IZANAMI_PACEMAKER_RBC_BACKLOG_SESSION_SOFT_LIMIT,
            )
            .write(
                [
                    "sumeragi",
                    "advanced",
                    "pacemaker",
                    "rbc_backlog_chunk_soft_limit",
                ],
                IZANAMI_PACEMAKER_RBC_BACKLOG_CHUNK_SOFT_LIMIT,
            )
            .write(
                ["sumeragi", "advanced", "pacing_governor", "min_factor_bps"],
                IZANAMI_PACING_GOVERNOR_MIN_FACTOR_BPS,
            )
            .write(
                ["sumeragi", "advanced", "pacing_governor", "max_factor_bps"],
                IZANAMI_PACING_GOVERNOR_MAX_FACTOR_BPS,
            )
            .write(
                ["sumeragi", "advanced", "rbc", "pending_max_chunks"],
                IZANAMI_RBC_PENDING_MAX_CHUNKS,
            )
            .write(
                ["sumeragi", "advanced", "rbc", "pending_max_bytes"],
                IZANAMI_RBC_PENDING_MAX_BYTES,
            )
            .write(
                ["sumeragi", "advanced", "rbc", "pending_session_limit"],
                IZANAMI_RBC_PENDING_SESSION_LIMIT,
            )
            .write(
                ["sumeragi", "advanced", "rbc", "pending_ttl_ms"],
                IZANAMI_RBC_PENDING_TTL_MS,
            )
            .write(
                ["sumeragi", "advanced", "rbc", "session_ttl_ms"],
                IZANAMI_RBC_SESSION_TTL_MS,
            )
            .write(
                ["sumeragi", "advanced", "rbc", "disk_store_ttl_ms"],
                IZANAMI_RBC_SESSION_TTL_MS,
            )
            .write(
                [
                    "sumeragi",
                    "advanced",
                    "rbc",
                    "rebroadcast_sessions_per_tick",
                ],
                IZANAMI_RBC_REBROADCAST_SESSIONS_PER_TICK,
            )
            .write(
                ["sumeragi", "advanced", "rbc", "payload_chunks_per_tick"],
                IZANAMI_RBC_PAYLOAD_CHUNKS_PER_TICK,
            )
            .write(
                ["sumeragi", "recovery", "height_attempt_cap"],
                IZANAMI_RECOVERY_HEIGHT_ATTEMPT_CAP,
            )
            .write(
                ["sumeragi", "recovery", "height_window_ms"],
                recovery_profile.height_window_ms,
            )
            .write(
                ["sumeragi", "recovery", "missing_qc_reacquire_window_ms"],
                recovery_profile.missing_qc_reacquire_window_ms,
            )
            .write(
                ["sumeragi", "recovery", "deferred_qc_ttl_ms"],
                recovery_profile.deferred_qc_ttl_ms,
            )
            .write(
                ["sumeragi", "recovery", "missing_block_height_ttl_ms"],
                recovery_profile.missing_block_height_ttl_ms,
            )
            .write(
                ["sumeragi", "recovery", "hash_miss_cap_before_range_pull"],
                recovery_profile.hash_miss_cap_before_range_pull,
            )
            .write(
                [
                    "sumeragi",
                    "recovery",
                    "missing_block_signer_fallback_attempts",
                ],
                recovery_profile.missing_block_signer_fallback_attempts,
            )
            .write(
                [
                    "sumeragi",
                    "recovery",
                    "missing_block_retry_backoff_multiplier",
                ],
                IZANAMI_RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_MULTIPLIER,
            )
            .write(
                ["sumeragi", "recovery", "missing_block_retry_backoff_cap_ms"],
                IZANAMI_RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_CAP_MS,
            )
            .write(
                [
                    "sumeragi",
                    "recovery",
                    "range_pull_escalation_after_hash_misses",
                ],
                recovery_profile.range_pull_escalation_after_hash_misses,
            )
            .write(
                ["sumeragi", "advanced", "da", "quorum_timeout_multiplier"],
                recovery_profile.da_quorum_timeout_multiplier,
            )
            .write(
                [
                    "sumeragi",
                    "advanced",
                    "da",
                    "availability_timeout_multiplier",
                ],
                recovery_profile.da_availability_timeout_multiplier,
            )
            .write(
                [
                    "sumeragi",
                    "advanced",
                    "da",
                    "availability_timeout_floor_ms",
                ],
                recovery_profile.da_availability_timeout_floor_ms,
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

    fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
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
        let mut config = config;
        apply_shared_host_stable_soak_profile(&mut config);
        log_effective_consensus_soak_overrides(&config);

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
            Some(config.peer_count),
            config.nexus.as_ref(),
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
        )?;
        let base_domain = state.base_domain().clone();

        let builder = make_network_builder(&config, genesis);

        let network = builder.start().await?;
        if config.nexus.is_some() {
            let genesis = network.genesis();
            let preflight = audit_npos_genesis_preflight(&genesis, config.peer_count)?;
            info!(
                target: "izanami::preflight",
                peer_with_pop_count = preflight.peer_with_pop_count,
                register_validator_count = preflight.register_validator_count,
                activate_validator_count = preflight.activate_validator_count,
                min_self_bond = preflight.min_self_bond,
                validator_stake_distribution = ?preflight.stake_distribution,
                validator_stake_distribution_entries = preflight.stake_distribution.len(),
                "validated Izanami NPoS genesis preflight"
            );
        }
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
        let ingress_stats = Arc::new(IngressStats::default());
        let ingress_pool = Arc::new(IngressEndpointPool::from_peers(
            &self.peers,
            IngressEndpointPoolConfig::default(),
            Arc::clone(&ingress_stats),
        ));
        let submission_counter = Arc::new(AtomicU64::new(0));

        let faulty_handles =
            self.spawn_fault_tasks(&config_layers, &genesis, &run_control, &mut rng);
        let load_handles = self.spawn_load_supervisors(
            &metrics,
            &ingress_pool,
            &run_control,
            &mut rng,
            &submission_counter,
        );

        let soft_target_kpi =
            self.config.target_blocks.is_some() && is_shared_host_stable_soak(&self.config);
        let target_result = if let Some(target_blocks) = self.config.target_blocks {
            wait_for_target_blocks(
                &self.peers,
                target_blocks,
                self.config.faulty_peers,
                self.config.progress_interval,
                self.config.progress_timeout,
                self.config.latency_p95_threshold,
                &run_control,
                Some(ingress_pool.as_ref()),
                soft_target_kpi,
            )
            .await
        } else {
            Ok(TargetProgressResult::default())
        };

        let mut run_error = None;
        match target_result {
            Ok(target_progress) => {
                if soft_target_kpi && let Some(target_blocks) = self.config.target_blocks {
                    if target_progress.target_reached {
                        info!(
                            target: "izanami::progress",
                            target_blocks,
                            quorum_min_height = target_progress.quorum_min_height,
                            strict_min_height = target_progress.strict_min_height,
                            "stable soak duration completed and target_blocks KPI was reached"
                        );
                    } else {
                        warn!(
                            target: "izanami::progress",
                            target_blocks,
                            quorum_min_height = target_progress.quorum_min_height,
                            strict_min_height = target_progress.strict_min_height,
                            "stable soak duration completed without target_blocks KPI; reporting as warning (non-fatal)"
                        );
                    }
                }
            }
            Err(err) => {
                warn!(
                    target: "izanami::progress",
                    ?err,
                    "target progress monitoring failed; stopping run"
                );
                run_control.stop();
                run_error = Some(err);
            }
        }

        if self.config.target_blocks.is_some() {
            run_control.stop();
        }

        if run_error.is_some() {
            // Cut off peer services first on fatal progress failure so blocking submitters
            // terminate quickly instead of draining through long status timeouts.
            self.network.shutdown().await;
        }

        let shutdown_timeout = if run_error.is_some() {
            Duration::from_secs(IZANAMI_WORKER_FAILURE_SHUTDOWN_TIMEOUT_SECS)
        } else {
            Duration::from_secs(IZANAMI_WORKER_SHUTDOWN_TIMEOUT_SECS)
        };
        await_worker_shutdown_with_timeout(load_handles, "load", shutdown_timeout).await;
        await_worker_shutdown_with_timeout(faulty_handles, "fault", shutdown_timeout).await;
        if run_error.is_none() {
            self.network.shutdown().await;
        }

        let snapshot = metrics.snapshot();
        let ingress_snapshot = ingress_stats.snapshot();
        if let Some(err) = run_error {
            warn!(
                target: "izanami::summary",
                successes = snapshot.successes,
                failures = snapshot.failures,
                expected_failures = snapshot.expected_failures,
                unexpected_successes = snapshot.unexpected_successes,
                izanami_ingress_failover_total = ingress_snapshot.failover_total,
                izanami_ingress_endpoint_unhealthy_total = ingress_snapshot.endpoint_unhealthy_total,
                ?err,
                "izanami run finished with errors"
            );
            Err(err)
        } else {
            info!(
                target: "izanami::summary",
                successes = snapshot.successes,
                failures = snapshot.failures,
                expected_failures = snapshot.expected_failures,
                unexpected_successes = snapshot.unexpected_successes,
                izanami_ingress_failover_total = ingress_snapshot.failover_total,
                izanami_ingress_endpoint_unhealthy_total = ingress_snapshot.endpoint_unhealthy_total,
                "izanami run complete"
            );
            Ok(())
        }
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
        ingress_pool: &Arc<IngressEndpointPool>,
        run_control: &Arc<RunControl>,
        rng: &mut StdRng,
        submission_counter: &Arc<AtomicU64>,
    ) -> Vec<JoinHandle<()>> {
        let submission_confirmation = submission_confirmation_mode(&self.config);
        let workload = Arc::clone(&self.workload);
        let semaphore = Arc::new(Semaphore::new(self.config.max_inflight));
        let mut load_rng = rng.clone();
        let interval = Duration::from_secs_f64(1.0 / self.config.tps);
        let metrics = Arc::clone(metrics);
        let ingress_pool = Arc::clone(ingress_pool);
        let run_control = Arc::clone(run_control);
        let stop_notify = run_control.stop_notifier();
        let deadline = run_control.deadline();
        let submission_counter = Arc::clone(submission_counter);
        let backlog_limit = submission_backlog_limit(self.config.max_inflight);
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
                if !wait_for_submission_capacity(
                    &mut submissions,
                    backlog_limit,
                    stop_notify.as_ref(),
                    deadline,
                )
                .await
                {
                    break;
                }
                if run_control.should_stop() {
                    break;
                }
                if let Some(retry_after) =
                    ingress_pool.submission_backpressure_delay(Instant::now())
                {
                    debug!(
                        target: "izanami::ingress",
                        ?retry_after,
                        "deferring workload submit while ingress endpoints remain in cooldown"
                    );
                    tokio::select! {
                        () = time::sleep(retry_after) => {},
                        () = stop_notify.notified() => break,
                        () = time::sleep_until(deadline.into()) => break,
                    }
                    continue;
                }
                let plan = match workload.next_plan(&mut load_rng).await {
                    Ok(plan) => plan,
                    Err(err) => {
                        warn!(target: "izanami::workload", ?err, "failed to build transaction plan");
                        continue;
                    }
                };
                let metrics = Arc::clone(&metrics);
                let ingress_pool = Arc::clone(&ingress_pool);
                let submission_counter = Arc::clone(&submission_counter);
                let workload = Arc::clone(&workload);
                let semaphore = Arc::clone(&semaphore);
                submissions.spawn(async move {
                    submit_plan(
                        &ingress_pool,
                        plan,
                        submission_confirmation,
                        semaphore,
                        &metrics,
                        &submission_counter,
                        &workload,
                    )
                    .await;
                });
            }
            submissions.abort_all();
            while let Some(result) = submissions.join_next().await {
                let _ = result;
            }
        });
        vec![handle]
    }
}

fn drain_ready_submissions(submissions: &mut JoinSet<()>) {
    while let Some(result) = submissions.try_join_next() {
        let _ = result;
    }
}

fn submission_backlog_limit(max_inflight: usize) -> usize {
    max_inflight
        .max(1)
        .saturating_mul(IZANAMI_SUBMISSION_BACKLOG_MULTIPLIER)
}

const fn should_run_trigger_precheck(submission_confirmation: SubmissionConfirmationMode) -> bool {
    matches!(
        submission_confirmation,
        SubmissionConfirmationMode::BlockingApplied
    )
}

fn state_updates_require_applied_confirmation(state_updates: &[PlanUpdate]) -> bool {
    state_updates.iter().any(|update| {
        matches!(
            update,
            PlanUpdate::TrackAccount(_)
                | PlanUpdate::TrackAssetInstance(_)
                | PlanUpdate::RegisterTrigger(_)
                | PlanUpdate::RegisterCallTrigger(_)
                | PlanUpdate::TrackRepeatableTrigger(_)
                | PlanUpdate::MintTriggerRepetitions { .. }
                | PlanUpdate::BurnTriggerRepetitions { .. }
                | PlanUpdate::ReleaseTriggerRepetitionsReservation { .. }
                | PlanUpdate::SetTriggerMetadata { .. }
                | PlanUpdate::ClearTriggerMetadata(_)
        )
    })
}

fn effective_submission_confirmation(
    submission_confirmation: SubmissionConfirmationMode,
    state_updates: &[PlanUpdate],
) -> SubmissionConfirmationMode {
    if matches!(
        submission_confirmation,
        SubmissionConfirmationMode::AcceptedByIngress
    ) && state_updates_require_applied_confirmation(state_updates)
    {
        SubmissionConfirmationMode::BlockingApplied
    } else {
        submission_confirmation
    }
}

fn tracked_repeatable_trigger(state_updates: &[PlanUpdate]) -> Option<TriggerId> {
    state_updates.iter().find_map(|update| match update {
        PlanUpdate::TrackRepeatableTrigger(trigger_id) => Some(trigger_id.clone()),
        _ => None,
    })
}

fn is_trigger_not_found_error(error: &color_eyre::Report) -> bool {
    let message = ingress_error_message(error);
    message.contains("trigger with id") && message.contains("not found")
}

async fn wait_for_submission_capacity(
    submissions: &mut JoinSet<()>,
    backlog_limit: usize,
    stop_notify: &Notify,
    deadline: Instant,
) -> bool {
    while submissions.len() >= backlog_limit {
        let joined = tokio::select! {
            result = submissions.join_next() => result,
            () = stop_notify.notified() => return false,
            () = time::sleep_until(deadline.into()) => return false,
        };
        match joined {
            Some(result) => {
                let _ = result;
                drain_ready_submissions(submissions);
            }
            None => return false,
        }
    }
    true
}

fn tune_ingress_client(mut client: Client, mode: SubmissionConfirmationMode) -> Client {
    client.torii_request_timeout = Duration::from_millis(IZANAMI_INGRESS_REQUEST_TIMEOUT_MS);
    if matches!(mode, SubmissionConfirmationMode::BlockingApplied) {
        client.transaction_status_timeout =
            Duration::from_millis(IZANAMI_INGRESS_STATUS_TIMEOUT_MS);
    }
    client
}

async fn await_worker_shutdown_with_timeout(
    handles: Vec<JoinHandle<()>>,
    worker_kind: &'static str,
    timeout: Duration,
) {
    for mut handle in handles {
        match time::timeout(timeout, &mut handle).await {
            Ok(result) => {
                let _ = result;
            }
            Err(_) => {
                warn!(
                    target: "izanami::run",
                    worker_kind,
                    ?timeout,
                    "worker shutdown timed out; aborting task"
                );
                handle.abort();
                let _ = handle.await;
            }
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

fn sampled_peer_heights_with_ids(peers: &[NetworkPeer]) -> Vec<(PeerId, u64)> {
    peers
        .iter()
        .map(|peer| {
            (
                peer.id().clone(),
                peer.best_effort_block_height()
                    .map(|height| height.total)
                    .unwrap_or(0),
            )
        })
        .collect()
}

fn tolerated_peer_failures(peer_count: usize) -> usize {
    if peer_count < 4 {
        0
    } else {
        peer_count.saturating_sub(1) / 3
    }
}

fn effective_tolerated_peer_failures(peer_count: usize, configured_faulty_peers: usize) -> usize {
    tolerated_peer_failures(peer_count).min(configured_faulty_peers)
}

fn quorum_min_height_from_samples(mut heights: Vec<u64>, tolerated_failures: usize) -> u64 {
    if heights.is_empty() {
        return 0;
    }
    heights.sort_unstable();
    let index = tolerated_failures.min(heights.len().saturating_sub(1));
    heights[index]
}

fn strict_divergence_reference_height_from_samples(
    mut heights: Vec<u64>,
    tolerated_failures: usize,
) -> u64 {
    if heights.is_empty() {
        return 0;
    }
    heights.sort_unstable();
    let index = heights
        .len()
        .saturating_sub(tolerated_failures.saturating_add(1));
    heights[index]
}

fn strict_divergence_lagging_peer_count(
    heights: &[u64],
    reference_height: u64,
    max_allowed_divergence: u64,
) -> usize {
    heights
        .iter()
        .filter(|height| reference_height.saturating_sub(**height) > max_allowed_divergence)
        .count()
}

fn should_enforce_strict_progress_timeout(lagging_peers: usize, tolerated_failures: usize) -> bool {
    lagging_peers == 0 || lagging_peers > tolerated_failures
}

fn strict_progress_stall_scope_message(
    lagging_peers: usize,
    tolerated_failures: usize,
) -> &'static str {
    if lagging_peers == 0 {
        "strict block height is stalled with no lagging peers; if this persists until the strict timeout the run will fail"
    } else if lagging_peers > tolerated_failures {
        "strict block height is stalled with broad peer lag beyond tolerated failures; if this persists until the strict timeout the run will fail"
    } else {
        "strict block height is stalled under tolerated outlier lag; continuing with quorum progress"
    }
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

    fn update(&mut self, now: Instant, height: u64) -> Option<(u64, Duration)> {
        if height > self.last_height {
            let blocks_advanced = height.saturating_sub(self.last_height);
            let elapsed = now.duration_since(self.last_progress_at);
            self.last_height = height;
            self.last_progress_at = now;
            Some((blocks_advanced, elapsed))
        } else {
            None
        }
    }

    fn stalled(&self, now: Instant, timeout: Duration) -> bool {
        now.duration_since(self.last_progress_at) >= timeout
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WeightedIntervalSample {
    interval_ms: u64,
    weight: u64,
}

#[derive(Default)]
struct BlockIntervalTracker {
    samples: Vec<WeightedIntervalSample>,
    total_weight: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BlockIntervalSummary {
    p50_ms: u64,
    p95_ms: u64,
    samples: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SumeragiPhaseSnapshot {
    propose_ms: u64,
    collect_da_ms: u64,
    collect_prevote_ms: u64,
    collect_precommit_ms: u64,
    collect_aggregator_ms: u64,
    commit_ms: u64,
    pipeline_total_ms: u64,
    pipeline_total_ema_ms: u64,
}

fn parse_sumeragi_phase_snapshot(value: norito::json::Value) -> Option<SumeragiPhaseSnapshot> {
    let norito::json::Value::Object(root) = value else {
        return None;
    };
    let ema = match root.get("ema_ms") {
        Some(norito::json::Value::Object(ema)) => ema,
        _ => return None,
    };
    Some(SumeragiPhaseSnapshot {
        propose_ms: root.get("propose_ms")?.as_u64()?,
        collect_da_ms: root.get("collect_da_ms")?.as_u64()?,
        collect_prevote_ms: root.get("collect_prevote_ms")?.as_u64()?,
        collect_precommit_ms: root.get("collect_precommit_ms")?.as_u64()?,
        collect_aggregator_ms: root.get("collect_aggregator_ms")?.as_u64()?,
        commit_ms: root.get("commit_ms")?.as_u64()?,
        pipeline_total_ms: root.get("pipeline_total_ms")?.as_u64()?,
        pipeline_total_ema_ms: ema.get("pipeline_total_ms")?.as_u64()?,
    })
}

async fn sample_sumeragi_phases(peers: &[NetworkPeer]) -> Result<SumeragiPhaseSnapshot, String> {
    let peer = peers
        .first()
        .cloned()
        .ok_or_else(|| "no peers available for phase sampling".to_owned())?;
    spawn_blocking(move || {
        let mut client = peer.client();
        client.set_operator_key_pair(sumeragi_phase_operator_keypair());
        let phases = client
            .get_sumeragi_phases_json()
            .map_err(|err| format!("failed to fetch sumeragi phases snapshot: {err}"))?;
        parse_sumeragi_phase_snapshot(phases)
            .ok_or_else(|| "phase payload missing expected fields".to_owned())
    })
    .await
    .map_err(|err| format!("phase sampling task failed: {err}"))?
}

impl BlockIntervalTracker {
    fn record(&mut self, blocks_advanced: u64, elapsed: Duration) -> Option<u64> {
        if blocks_advanced == 0 {
            return None;
        }
        let elapsed_ms = u64::try_from(elapsed.as_millis())
            .unwrap_or(u64::MAX)
            .max(1);
        let interval_ms = elapsed_ms.div_ceil(blocks_advanced);
        self.samples.push(WeightedIntervalSample {
            interval_ms,
            weight: blocks_advanced,
        });
        self.total_weight = self.total_weight.saturating_add(blocks_advanced);
        Some(interval_ms)
    }

    fn summary(&self) -> Option<BlockIntervalSummary> {
        if self.total_weight == 0 {
            return None;
        }
        Some(BlockIntervalSummary {
            p50_ms: self.quantile_ms(0.50),
            p95_ms: self.quantile_ms(0.95),
            samples: self.total_weight,
        })
    }

    fn quantile_ms(&self, quantile: f64) -> u64 {
        debug_assert!((0.0..=1.0).contains(&quantile));
        if self.total_weight == 0 {
            return 0;
        }
        let mut sorted = self.samples.clone();
        sorted.sort_unstable_by_key(|sample| sample.interval_ms);
        let rank = ((self.total_weight as f64) * quantile)
            .ceil()
            .clamp(1.0, self.total_weight as f64) as u64;
        let mut cumulative = 0u64;
        for sample in sorted {
            cumulative = cumulative.saturating_add(sample.weight);
            if cumulative >= rank {
                return sample.interval_ms;
            }
        }
        0
    }
}

struct HeightDivergenceState {
    first_seen_above_threshold: Option<Instant>,
}

impl HeightDivergenceState {
    fn new() -> Self {
        Self {
            first_seen_above_threshold: None,
        }
    }

    fn observe(
        &mut self,
        now: Instant,
        divergence_blocks: u64,
        max_allowed_divergence: u64,
    ) -> bool {
        if divergence_blocks > max_allowed_divergence {
            if self.first_seen_above_threshold.is_none() {
                self.first_seen_above_threshold = Some(now);
                return true;
            }
        } else {
            self.first_seen_above_threshold = None;
        }
        false
    }

    fn violated(&self, now: Instant, max_window: Duration) -> bool {
        self.first_seen_above_threshold
            .is_some_and(|started| now.saturating_duration_since(started) >= max_window)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct TargetProgressResult {
    target_reached: bool,
    quorum_min_height: u64,
    strict_min_height: u64,
}

fn enforce_latency_p95_gate(
    block_intervals: &BlockIntervalTracker,
    strict_block_intervals: &BlockIntervalTracker,
    latency_p95_threshold: Option<Duration>,
    target_blocks: u64,
    elapsed: Duration,
    checkpoint: &'static str,
) -> Result<()> {
    let Some(threshold) = latency_p95_threshold else {
        return Ok(());
    };
    let threshold_ms = u64::try_from(threshold.as_millis()).unwrap_or(u64::MAX);
    let Some(summary) = block_intervals.summary() else {
        warn!(
            target: "izanami::progress",
            target_blocks,
            threshold_ms,
            checkpoint,
            elapsed = ?elapsed,
            "latency p95 gate is configured but quorum interval samples are unavailable; skipping gate evaluation"
        );
        return Ok(());
    };
    if summary.p95_ms > threshold_ms {
        return Err(eyre!(
            "quorum p95 block interval {}ms exceeded threshold {}ms (samples {}, target {}, elapsed {:?}, checkpoint {})",
            summary.p95_ms,
            threshold_ms,
            summary.samples,
            target_blocks,
            elapsed,
            checkpoint
        ));
    }
    if let Some(strict_summary) = strict_block_intervals.summary()
        && strict_summary.p95_ms > threshold_ms
    {
        return Err(eyre!(
            "strict p95 block interval {}ms exceeded threshold {}ms (samples {}, target {}, elapsed {:?}, checkpoint {})",
            strict_summary.p95_ms,
            threshold_ms,
            strict_summary.samples,
            target_blocks,
            elapsed,
            checkpoint
        ));
    }
    Ok(())
}

async fn wait_for_target_blocks(
    peers: &[NetworkPeer],
    target_blocks: u64,
    configured_faulty_peers: usize,
    progress_interval: Duration,
    progress_timeout: Duration,
    latency_p95_threshold: Option<Duration>,
    run_control: &RunControl,
    ingress_pool: Option<&IngressEndpointPool>,
    target_blocks_soft_kpi: bool,
) -> Result<TargetProgressResult> {
    let start = Instant::now();
    let mut progress = ProgressState::new(start);
    let mut strict_progress = ProgressState::new(start);
    let mut strict_tolerated_stall_logged_at: Option<Instant> = None;
    let mut divergence = HeightDivergenceState::new();
    let mut block_intervals = BlockIntervalTracker::default();
    let mut strict_block_intervals = BlockIntervalTracker::default();
    let mut target_reached = false;
    let tolerated_failures =
        effective_tolerated_peer_failures(peers.len(), configured_faulty_peers);
    let strict_divergence_window =
        Duration::from_secs(IZANAMI_STRICT_HEIGHT_DIVERGENCE_MAX_WINDOW_SECS);
    loop {
        if run_control.stop_requested() {
            return Err(eyre!("izanami run stopped before target blocks reached"));
        }
        let now = Instant::now();
        let sampled_heights = sampled_peer_heights_with_ids(peers);
        let heights: Vec<_> = sampled_heights.iter().map(|(_, height)| *height).collect();
        let strict_min_height = heights.iter().copied().min().unwrap_or(0);
        let min_height = quorum_min_height_from_samples(heights.clone(), tolerated_failures);
        if let Some(ingress_pool) = ingress_pool {
            ingress_pool.update_lag_snapshot(min_height, sampled_heights.as_slice(), now);
        }
        let strict_reference_height =
            strict_divergence_reference_height_from_samples(heights.clone(), tolerated_failures);
        let divergence_blocks = strict_reference_height.saturating_sub(strict_min_height);
        let lagging_peers = strict_divergence_lagging_peer_count(
            &heights,
            strict_reference_height,
            IZANAMI_STRICT_HEIGHT_DIVERGENCE_MAX_BLOCKS,
        );
        let strict_guard_active =
            should_enforce_strict_progress_timeout(lagging_peers, tolerated_failures);
        if now >= run_control.deadline() {
            if target_blocks_soft_kpi {
                enforce_latency_p95_gate(
                    &block_intervals,
                    &strict_block_intervals,
                    latency_p95_threshold,
                    target_blocks,
                    now.duration_since(start),
                    "duration_deadline",
                )?;
                return Ok(TargetProgressResult {
                    target_reached,
                    quorum_min_height: min_height,
                    strict_min_height,
                });
            }
            return Err(eyre!(
                "timed out before reaching target blocks (quorum min height {}, strict min {}, target {}, tolerated_failures {})",
                progress.last_height,
                strict_min_height,
                target_blocks,
                tolerated_failures
            ));
        }
        let divergence_started = if strict_guard_active {
            divergence.observe(
                now,
                divergence_blocks,
                IZANAMI_STRICT_HEIGHT_DIVERGENCE_MAX_BLOCKS,
            )
        } else {
            // A single tolerated outlier should not start the strict divergence timer.
            let _ = divergence.observe(now, 0, IZANAMI_STRICT_HEIGHT_DIVERGENCE_MAX_BLOCKS);
            false
        };
        if divergence_started {
            warn!(
                target: "izanami::progress",
                quorum_min_height = min_height,
                strict_reference_height,
                strict_min_height,
                divergence_blocks,
                lagging_peers,
                tolerated_failures,
                max_allowed_divergence = IZANAMI_STRICT_HEIGHT_DIVERGENCE_MAX_BLOCKS,
                max_window = ?strict_divergence_window,
                "detected quorum/strict height divergence above safety threshold"
            );
        }
        if strict_guard_active && divergence.violated(now, strict_divergence_window) {
            return Err(eyre!(
                "height divergence exceeded safety window (divergence {}, threshold {}, window {:?}, quorum min {}, strict reference {}, strict min {}, lagging peers {}, target {}, tolerated_failures {})",
                divergence_blocks,
                IZANAMI_STRICT_HEIGHT_DIVERGENCE_MAX_BLOCKS,
                strict_divergence_window,
                min_height,
                strict_reference_height,
                strict_min_height,
                lagging_peers,
                target_blocks,
                tolerated_failures
            ));
        }
        if min_height >= target_blocks && !target_reached {
            target_reached = true;
            let strict_summary = strict_block_intervals.summary();
            if let Some(summary) = block_intervals.summary() {
                info!(
                    target: "izanami::progress",
                    quorum_min_height = min_height,
                    strict_min_height,
                    tolerated_failures,
                    target_blocks,
                    interval_p50_ms = summary.p50_ms,
                    interval_p95_ms = summary.p95_ms,
                    interval_samples = summary.samples,
                    strict_interval_p50_ms = strict_summary.map(|item| item.p50_ms),
                    strict_interval_p95_ms = strict_summary.map(|item| item.p95_ms),
                    strict_interval_samples = strict_summary.map(|item| item.samples),
                    elapsed = ?now.duration_since(start),
                    "target block height reached"
                );
                match sample_sumeragi_phases(peers).await {
                    Ok(phases) => {
                        info!(
                            target: "izanami::progress",
                            phase_propose_ms = phases.propose_ms,
                            phase_collect_da_ms = phases.collect_da_ms,
                            phase_collect_prevote_ms = phases.collect_prevote_ms,
                            phase_collect_precommit_ms = phases.collect_precommit_ms,
                            phase_collect_aggregator_ms = phases.collect_aggregator_ms,
                            phase_commit_ms = phases.commit_ms,
                            phase_pipeline_total_ms = phases.pipeline_total_ms,
                            phase_pipeline_total_ema_ms = phases.pipeline_total_ema_ms,
                            "sumeragi phase timing snapshot at target height"
                        );
                    }
                    Err(err) => {
                        warn!(
                            target: "izanami::progress",
                            error = %err,
                            "sumeragi phase timing snapshot unavailable at target height"
                        );
                    }
                }
                enforce_latency_p95_gate(
                    &block_intervals,
                    &strict_block_intervals,
                    latency_p95_threshold,
                    target_blocks,
                    now.duration_since(start),
                    "target_reached",
                )?;
            } else {
                info!(
                    target: "izanami::progress",
                    quorum_min_height = min_height,
                    strict_min_height,
                    tolerated_failures,
                    target_blocks,
                    elapsed = ?now.duration_since(start),
                    "target block height reached"
                );
                match sample_sumeragi_phases(peers).await {
                    Ok(phases) => {
                        info!(
                            target: "izanami::progress",
                            phase_propose_ms = phases.propose_ms,
                            phase_collect_da_ms = phases.collect_da_ms,
                            phase_collect_prevote_ms = phases.collect_prevote_ms,
                            phase_collect_precommit_ms = phases.collect_precommit_ms,
                            phase_collect_aggregator_ms = phases.collect_aggregator_ms,
                            phase_commit_ms = phases.commit_ms,
                            phase_pipeline_total_ms = phases.pipeline_total_ms,
                            phase_pipeline_total_ema_ms = phases.pipeline_total_ema_ms,
                            "sumeragi phase timing snapshot at target height"
                        );
                    }
                    Err(err) => {
                        warn!(
                            target: "izanami::progress",
                            error = %err,
                            "sumeragi phase timing snapshot unavailable at target height"
                        );
                    }
                }
            }
            if !target_blocks_soft_kpi {
                return Ok(TargetProgressResult {
                    target_reached: true,
                    quorum_min_height: min_height,
                    strict_min_height,
                });
            }
            info!(
                target: "izanami::progress",
                target_blocks,
                quorum_min_height = min_height,
                strict_min_height,
                tolerated_failures,
                "target_blocks KPI reached; continuing until soak deadline"
            );
        }

        if let Some((blocks_advanced, elapsed)) = strict_progress.update(now, strict_min_height) {
            let interval_ms = strict_block_intervals
                .record(blocks_advanced, elapsed)
                .unwrap_or_default();
            strict_tolerated_stall_logged_at = None;
            info!(
                target: "izanami::progress",
                strict_min_height,
                target_blocks,
                blocks_advanced,
                interval_ms,
                "strict block height advanced"
            );
        } else if strict_progress.stalled(now, progress_timeout) {
            if strict_guard_active {
                return Err(eyre!(
                    "no strict block height progress for {:?} (strict min height {}, quorum min height {}, target {}, tolerated_failures {})",
                    progress_timeout,
                    strict_min_height,
                    min_height,
                    target_blocks,
                    tolerated_failures
                ));
            }
            let should_log = strict_tolerated_stall_logged_at.map_or(true, |last| {
                now.saturating_duration_since(last) >= progress_interval
            });
            if should_log {
                let lagging = sampled_heights
                    .iter()
                    .filter_map(|(peer_id, height)| {
                        (strict_reference_height.saturating_sub(*height)
                            > IZANAMI_STRICT_HEIGHT_DIVERGENCE_MAX_BLOCKS)
                            .then_some((peer_id.clone(), *height))
                    })
                    .collect::<Vec<_>>();
                warn!(
                    target: "izanami::progress",
                    strict_min_height,
                    quorum_min_height = min_height,
                    strict_reference_height,
                    lagging_peers,
                    tolerated_failures,
                    strict_timeout = ?progress_timeout,
                    ?lagging,
                    "strict block height is stalled past strict timeout under tolerated outlier lag; continuing with quorum progress"
                );
                strict_tolerated_stall_logged_at = Some(now);
            }
        } else {
            let should_log = strict_tolerated_stall_logged_at.map_or(true, |last| {
                now.saturating_duration_since(last) >= progress_interval
            });
            if should_log && strict_progress.stalled(now, progress_interval) {
                let lagging = sampled_heights
                    .iter()
                    .filter_map(|(peer_id, height)| {
                        (strict_reference_height.saturating_sub(*height)
                            > IZANAMI_STRICT_HEIGHT_DIVERGENCE_MAX_BLOCKS)
                            .then_some((peer_id.clone(), *height))
                    })
                    .collect::<Vec<_>>();
                warn!(
                    target: "izanami::progress",
                    strict_min_height,
                    quorum_min_height = min_height,
                    strict_reference_height,
                    lagging_peers,
                    tolerated_failures,
                    strict_timeout = ?progress_timeout,
                    ?lagging,
                    "{}",
                    strict_progress_stall_scope_message(lagging_peers, tolerated_failures)
                );
                strict_tolerated_stall_logged_at = Some(now);
            }
        }

        if let Some((blocks_advanced, elapsed)) = progress.update(now, min_height) {
            let interval_ms = block_intervals
                .record(blocks_advanced, elapsed)
                .unwrap_or_default();
            info!(
                target: "izanami::progress",
                quorum_min_height = min_height,
                strict_min_height,
                tolerated_failures,
                target_blocks,
                blocks_advanced,
                interval_ms,
                "block height advanced"
            );
        } else if progress.stalled(now, progress_timeout) {
            return Err(eyre!(
                "no block height progress for {:?} (quorum min height {}, strict min {}, target {}, tolerated_failures {})",
                progress_timeout,
                min_height,
                strict_min_height,
                target_blocks,
                tolerated_failures
            ));
        }
        let remaining = run_control
            .deadline()
            .checked_duration_since(Instant::now())
            .unwrap_or_default();
        if remaining.is_zero() {
            if target_blocks_soft_kpi {
                enforce_latency_p95_gate(
                    &block_intervals,
                    &strict_block_intervals,
                    latency_p95_threshold,
                    target_blocks,
                    now.duration_since(start),
                    "duration_deadline",
                )?;
                return Ok(TargetProgressResult {
                    target_reached,
                    quorum_min_height: min_height,
                    strict_min_height,
                });
            }
            return Err(eyre!(
                "timed out before reaching target blocks (quorum min height {}, strict min {}, target {}, tolerated_failures {})",
                min_height,
                strict_min_height,
                target_blocks,
                tolerated_failures
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

fn repetitions_from_repeats(repeats: Repeats) -> Option<u32> {
    match repeats {
        Repeats::Exactly(count) => Some(count),
        Repeats::Indefinitely => None,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum MintPrecheck {
    Proceed { on_chain: u32 },
    SkipMissing,
    SkipQueryFailed,
}

fn evaluate_mint_precheck<E>(result: Result<Option<u32>, E>) -> MintPrecheck {
    match result {
        Ok(Some(0)) | Ok(None) => MintPrecheck::SkipMissing,
        Ok(Some(on_chain)) => MintPrecheck::Proceed { on_chain },
        Err(_) => MintPrecheck::SkipQueryFailed,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum BurnPrecheck {
    Proceed { on_chain: u32 },
    SkipMissing,
    SkipInsufficient { on_chain: u32 },
    SkipQueryFailed,
}

fn evaluate_burn_precheck<E>(result: Result<Option<u32>, E>, burn_amount: u32) -> BurnPrecheck {
    match result {
        Ok(None) => BurnPrecheck::SkipMissing,
        Ok(Some(on_chain)) if on_chain <= burn_amount => {
            BurnPrecheck::SkipInsufficient { on_chain }
        }
        Ok(Some(on_chain)) => BurnPrecheck::Proceed { on_chain },
        Err(_) => BurnPrecheck::SkipQueryFailed,
    }
}

fn query_trigger_repetitions(client: &Client, trigger_id: &TriggerId) -> Result<Option<u32>> {
    let iter = client.query(FindTriggers::new()).execute()?;
    for trigger in iter {
        let trigger = trigger?;
        if trigger.id() == trigger_id {
            return Ok(repetitions_from_repeats(trigger.action().repeats()));
        }
    }
    Ok(None)
}

fn record_plan_skip(
    metrics: &Metrics,
    plan_label: &'static str,
    expect_success: bool,
    reason: &'static str,
) {
    debug!(
        target: "izanami::workload",
        plan = plan_label,
        reason,
        "skipping plan submission"
    );
    if expect_success {
        metrics.record_failure();
    } else {
        metrics.record_expected_failure();
    }
}

async fn submit_plan(
    ingress_pool: &Arc<IngressEndpointPool>,
    plan: TransactionPlan,
    submission_confirmation: SubmissionConfirmationMode,
    semaphore: Arc<Semaphore>,
    metrics: &Arc<Metrics>,
    submission_counter: &Arc<AtomicU64>,
    workload: &Arc<WorkloadEngine>,
) {
    let ingress_pool = Arc::clone(ingress_pool);
    let signer = plan.signer.clone();
    let instructions = plan.instructions.clone();
    let plan_label = plan.label;
    let expect_success = plan.expect_success;
    let metrics = Arc::clone(metrics);
    let submission_counter = Arc::clone(submission_counter);
    let workload = Arc::clone(workload);
    let burn_target = plan.burn_trigger_repetitions();
    let mint_target = plan.mint_trigger_repetitions();
    let register_target = tracked_repeatable_trigger(&plan.state_updates);
    let repeatable_trigger_target = burn_target
        .as_ref()
        .map(|(trigger_id, _)| trigger_id.clone())
        .or_else(|| {
            mint_target
                .as_ref()
                .map(|(trigger_id, _)| trigger_id.clone())
        })
        .or(register_target);
    let effective_submission_confirmation =
        effective_submission_confirmation(submission_confirmation, &plan.state_updates);
    let run_trigger_precheck = should_run_trigger_precheck(effective_submission_confirmation);
    let pinned_trigger_endpoint = if repeatable_trigger_target.is_some() {
        match ingress_pool.select_endpoint("submit_repeatable_trigger_plan") {
            Ok(endpoint_idx) => Some(endpoint_idx),
            Err(err) => {
                warn!(
                    target: "izanami::workload",
                    ?err,
                    plan = plan_label,
                    "failed to select pinned ingress endpoint for repeatable trigger plan"
                );
                if expect_success {
                    metrics.record_failure();
                } else {
                    metrics.record_expected_failure();
                }
                if let Some(trigger_id) = repeatable_trigger_target {
                    workload.mark_trigger_unknown(&trigger_id).await;
                }
                workload.record_result(&plan, false).await;
                return;
            }
        }
    } else {
        None
    };

    if run_trigger_precheck && let Some((trigger_id, burn_amount)) = burn_target.clone() {
        let endpoint_idx =
            pinned_trigger_endpoint.expect("repeatable trigger plan should pin ingress");
        let precheck = evaluate_burn_precheck(
            query_trigger_repetitions_on_endpoint(
                &ingress_pool,
                endpoint_idx,
                &signer,
                trigger_id.clone(),
            )
            .await,
            burn_amount,
        );
        match precheck {
            BurnPrecheck::Proceed { on_chain } => {
                workload
                    .sync_trigger_repetitions(&trigger_id, Some(on_chain))
                    .await;
            }
            BurnPrecheck::SkipMissing => {
                workload.sync_trigger_repetitions(&trigger_id, None).await;
                record_plan_skip(
                    &metrics,
                    plan_label,
                    expect_success,
                    "on-chain trigger repetition drift",
                );
                workload.record_result(&plan, false).await;
                return;
            }
            BurnPrecheck::SkipInsufficient { on_chain } => {
                workload
                    .sync_trigger_repetitions(&trigger_id, Some(on_chain))
                    .await;
                record_plan_skip(
                    &metrics,
                    plan_label,
                    expect_success,
                    "on-chain trigger repetition drift",
                );
                workload.record_result(&plan, false).await;
                return;
            }
            BurnPrecheck::SkipQueryFailed => {
                reconcile_repeatable_trigger_with_endpoint(
                    &workload,
                    &ingress_pool,
                    endpoint_idx,
                    &signer,
                    &trigger_id,
                )
                .await;
                record_plan_skip(
                    &metrics,
                    plan_label,
                    expect_success,
                    "trigger repetition query failed",
                );
                workload.record_result(&plan, false).await;
                return;
            }
        }
    }

    if run_trigger_precheck && let Some((trigger_id, _mint_amount)) = mint_target.clone() {
        let endpoint_idx =
            pinned_trigger_endpoint.expect("repeatable trigger plan should pin ingress");
        let precheck = evaluate_mint_precheck(
            query_trigger_repetitions_on_endpoint(
                &ingress_pool,
                endpoint_idx,
                &signer,
                trigger_id.clone(),
            )
            .await,
        );
        match precheck {
            MintPrecheck::Proceed { on_chain } => {
                workload
                    .sync_trigger_repetitions(&trigger_id, Some(on_chain))
                    .await;
            }
            MintPrecheck::SkipMissing => {
                workload.sync_trigger_repetitions(&trigger_id, None).await;
                record_plan_skip(
                    &metrics,
                    plan_label,
                    expect_success,
                    "on-chain trigger repetition drift",
                );
                workload.record_result(&plan, false).await;
                return;
            }
            MintPrecheck::SkipQueryFailed => {
                reconcile_repeatable_trigger_with_endpoint(
                    &workload,
                    &ingress_pool,
                    endpoint_idx,
                    &signer,
                    &trigger_id,
                )
                .await;
                record_plan_skip(
                    &metrics,
                    plan_label,
                    expect_success,
                    "trigger repetition query failed",
                );
                workload.record_result(&plan, false).await;
                return;
            }
        }
    }

    let permit = match semaphore.acquire_owned().await {
        Ok(permit) => permit,
        Err(_) => {
            warn!(
                target: "izanami::workload",
                plan = plan_label,
                "submission permit channel closed before submit"
            );
            if expect_success {
                metrics.record_failure();
            } else {
                metrics.record_expected_failure();
            }
            workload.record_result(&plan, false).await;
            return;
        }
    };

    let submission_result = if let Some(endpoint_idx) = pinned_trigger_endpoint {
        let ingress_pool_for_submit = Arc::clone(&ingress_pool);
        let signer_for_submit = signer.clone();
        let instructions_for_submit = instructions.clone();
        let submission_counter_for_submit = Arc::clone(&submission_counter);
        run_submission_result(
            plan_label,
            expect_success,
            Arc::clone(&metrics),
            move || {
                ingress_pool_for_submit.run_on_endpoint(
                    "submit_repeatable_trigger_plan",
                    endpoint_idx,
                    |peer| {
                        let client = tune_ingress_client(
                            peer.client_for(
                                &signer_for_submit.id,
                                signer_for_submit.key_pair.private_key().clone(),
                            ),
                            effective_submission_confirmation,
                        );
                        let metadata = submission_metadata(submission_counter_for_submit.as_ref());
                        match effective_submission_confirmation {
                            SubmissionConfirmationMode::BlockingApplied => client
                                .submit_all_blocking_with_metadata(
                                    instructions_for_submit.clone(),
                                    metadata,
                                )
                                .map(|_| ()),
                            SubmissionConfirmationMode::AcceptedByIngress => client
                                .submit_all_with_metadata(instructions_for_submit.clone(), metadata)
                                .map(|_| ()),
                        }
                    },
                )
            },
        )
        .await
    } else {
        let ingress_pool_for_submit = Arc::clone(&ingress_pool);
        let signer_for_submit = signer.clone();
        let instructions_for_submit = instructions.clone();
        let submission_counter_for_submit = Arc::clone(&submission_counter);
        run_submission_result(
            plan_label,
            expect_success,
            Arc::clone(&metrics),
            move || {
                let ingress_pool_for_retry_delay = Arc::clone(&ingress_pool_for_submit);
                run_with_queue_timeout_retry_with_policy_and_delay(
                    plan_label,
                    IZANAMI_QUEUE_TIMEOUT_RETRY_ATTEMPTS,
                    Duration::from_millis(IZANAMI_QUEUE_TIMEOUT_RETRY_BACKOFF_MS),
                    move || {
                        ingress_pool_for_retry_delay.submission_backpressure_delay(Instant::now())
                    },
                    || {
                        ingress_pool_for_submit.run_with_failover(
                            "submit_transaction_plan",
                            |peer| {
                                let client = tune_ingress_client(
                                    peer.client_for(
                                        &signer_for_submit.id,
                                        signer_for_submit.key_pair.private_key().clone(),
                                    ),
                                    effective_submission_confirmation,
                                );
                                let metadata =
                                    submission_metadata(submission_counter_for_submit.as_ref());
                                match effective_submission_confirmation {
                                    SubmissionConfirmationMode::BlockingApplied => client
                                        .submit_all_blocking_with_metadata(
                                            instructions_for_submit.clone(),
                                            metadata,
                                        )
                                        .map(|_| ()),
                                    SubmissionConfirmationMode::AcceptedByIngress => client
                                        .submit_all_with_metadata(
                                            instructions_for_submit.clone(),
                                            metadata,
                                        )
                                        .map(|_| ()),
                                }
                            },
                        )
                    },
                )
            },
        )
        .await
    };
    drop(permit);

    match submission_result {
        Ok(()) => {
            workload.record_result(&plan, true).await;
            if let (Some(endpoint_idx), Some(trigger_id)) =
                (pinned_trigger_endpoint, repeatable_trigger_target.as_ref())
            {
                reconcile_repeatable_trigger_with_endpoint(
                    &workload,
                    &ingress_pool,
                    endpoint_idx,
                    &signer,
                    trigger_id,
                )
                .await;
            }
        }
        Err(err) => {
            workload.record_result(&plan, false).await;
            if let Some(trigger_id) = repeatable_trigger_target.as_ref() {
                if is_trigger_not_found_error(&err) {
                    workload.sync_trigger_repetitions(trigger_id, None).await;
                } else if let Some(endpoint_idx) = pinned_trigger_endpoint {
                    reconcile_repeatable_trigger_with_endpoint(
                        &workload,
                        &ingress_pool,
                        endpoint_idx,
                        &signer,
                        trigger_id,
                    )
                    .await;
                } else {
                    workload.mark_trigger_unknown(trigger_id).await;
                }
            }
        }
    }
}

async fn query_trigger_repetitions_on_endpoint(
    ingress_pool: &Arc<IngressEndpointPool>,
    endpoint_idx: usize,
    signer: &AccountRecord,
    trigger_id: TriggerId,
) -> Result<Option<u32>> {
    let ingress_pool = Arc::clone(ingress_pool);
    let signer = signer.clone();
    match spawn_blocking(move || {
        ingress_pool.run_on_endpoint("query_trigger_repetitions", endpoint_idx, |peer| {
            let client = tune_ingress_client(
                peer.client_for(&signer.id, signer.key_pair.private_key().clone()),
                SubmissionConfirmationMode::AcceptedByIngress,
            );
            query_trigger_repetitions(&client, &trigger_id)
        })
    })
    .await
    {
        Ok(result) => result,
        Err(err) => Err(err.into()),
    }
}

async fn reconcile_repeatable_trigger_with_endpoint(
    workload: &Arc<WorkloadEngine>,
    ingress_pool: &Arc<IngressEndpointPool>,
    endpoint_idx: usize,
    signer: &AccountRecord,
    trigger_id: &TriggerId,
) {
    match query_trigger_repetitions_on_endpoint(
        ingress_pool,
        endpoint_idx,
        signer,
        trigger_id.clone(),
    )
    .await
    {
        Ok(Some(on_chain)) if on_chain > 0 => {
            workload
                .sync_trigger_repetitions(trigger_id, Some(on_chain))
                .await;
        }
        Ok(Some(_)) | Ok(None) => {
            workload.sync_trigger_repetitions(trigger_id, None).await;
        }
        Err(err) => {
            warn!(
                target: "izanami::workload",
                ?err,
                trigger = %trigger_id,
                endpoint_idx,
                "failed to reconcile repeatable trigger state from pinned ingress endpoint"
            );
            workload.mark_trigger_unknown(trigger_id).await;
        }
    }
}

async fn run_submission_result<F>(
    plan_label: &'static str,
    expect_success: bool,
    metrics: Arc<Metrics>,
    blocking: F,
) -> Result<()>
where
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
    if let Err(err) = &result {
        warn!(
            target: "izanami::workload",
            ?err,
            plan = plan_label,
            "plan submission failed"
        );
    }
    result
}

#[allow(dead_code)] // Retained for workload helpers that need success-only submission semantics.
async fn run_submission<F>(
    plan_label: &'static str,
    expect_success: bool,
    metrics: Arc<Metrics>,
    blocking: F,
) -> bool
where
    F: FnOnce() -> Result<()> + Send + 'static,
{
    run_submission_result(plan_label, expect_success, metrics, blocking)
        .await
        .is_ok()
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

    fn synthetic_npos_preflight_instructions(
        peer_count: usize,
        include_pop: bool,
        include_activation: bool,
        stake_values: &[u64],
    ) -> Vec<InstructionBox> {
        let mut instructions = Vec::new();
        let fallback_stake = SumeragiNposParameters::default().min_self_bond();
        for idx in 0..peer_count {
            let key_pair = KeyPair::random();
            let validator = AccountId::new(key_pair.public_key().clone());
            let stake = stake_values.get(idx).copied().unwrap_or(fallback_stake);
            if include_pop {
                instructions.push(
                    <RegisterPeerWithPop as iroha_data_model::isi::Instruction>::into_instruction_box(
                        Box::new(RegisterPeerWithPop::new(
                            PeerId::new(key_pair.public_key().clone()),
                            vec![u8::try_from(idx).unwrap_or(u8::MAX)],
                        )),
                    ),
                );
            }
            instructions.push(
                <RegisterPublicLaneValidator as iroha_data_model::isi::Instruction>::into_instruction_box(
                    Box::new(RegisterPublicLaneValidator {
                        lane_id: LaneId::SINGLE,
                        validator: validator.clone(),
                        stake_account: validator.clone(),
                        initial_stake: Numeric::from(stake),
                        metadata: Metadata::default(),
                    }),
                ),
            );
            if include_activation {
                instructions.push(
                    <ActivatePublicLaneValidator as iroha_data_model::isi::Instruction>::into_instruction_box(
                        Box::new(ActivatePublicLaneValidator {
                            lane_id: LaneId::SINGLE,
                            validator,
                        }),
                    ),
                );
            }
        }
        instructions
    }

    #[test]
    fn repetitions_from_repeats_exactly_returns_value() {
        assert_eq!(repetitions_from_repeats(Repeats::Exactly(7)), Some(7));
    }

    #[test]
    fn repetitions_from_repeats_indefinitely_returns_none() {
        assert_eq!(repetitions_from_repeats(Repeats::Indefinitely), None);
    }

    #[test]
    fn evaluate_mint_precheck_handles_trigger_states() {
        assert_eq!(
            evaluate_mint_precheck::<color_eyre::eyre::Report>(Ok(Some(3))),
            MintPrecheck::Proceed { on_chain: 3 }
        );
        assert_eq!(
            evaluate_mint_precheck::<color_eyre::eyre::Report>(Ok(Some(0))),
            MintPrecheck::SkipMissing
        );
        assert_eq!(
            evaluate_mint_precheck::<color_eyre::eyre::Report>(Ok(None)),
            MintPrecheck::SkipMissing
        );
        assert_eq!(
            evaluate_mint_precheck::<&'static str>(Err("boom")),
            MintPrecheck::SkipQueryFailed
        );
    }

    #[test]
    fn evaluate_burn_precheck_handles_trigger_states() {
        assert_eq!(
            evaluate_burn_precheck::<color_eyre::eyre::Report>(Ok(Some(5)), 3),
            BurnPrecheck::Proceed { on_chain: 5 }
        );
        assert_eq!(
            evaluate_burn_precheck::<color_eyre::eyre::Report>(Ok(Some(3)), 3),
            BurnPrecheck::SkipInsufficient { on_chain: 3 }
        );
        assert_eq!(
            evaluate_burn_precheck::<color_eyre::eyre::Report>(Ok(Some(1)), 2),
            BurnPrecheck::SkipInsufficient { on_chain: 1 }
        );
        assert_eq!(
            evaluate_burn_precheck::<color_eyre::eyre::Report>(Ok(None), 1),
            BurnPrecheck::SkipMissing
        );
        assert_eq!(
            evaluate_burn_precheck::<&'static str>(Err("boom"), 1),
            BurnPrecheck::SkipQueryFailed
        );
    }

    #[test]
    fn progress_state_update_reports_advanced_blocks_and_elapsed() {
        let start = Instant::now();
        let mut progress = ProgressState::new(start);
        assert_eq!(progress.update(start, 0), None);
        let advanced = progress
            .update(start + Duration::from_millis(120), 3)
            .expect("height increase should report progress");
        assert_eq!(advanced.0, 3);
        assert!(advanced.1 >= Duration::from_millis(120));
    }

    #[test]
    fn block_interval_tracker_reports_weighted_quantiles() {
        let mut tracker = BlockIntervalTracker::default();
        assert_eq!(tracker.summary(), None);
        assert_eq!(tracker.record(1, Duration::from_millis(100)), Some(100));
        assert_eq!(tracker.record(4, Duration::from_millis(2_000)), Some(500));
        let summary = tracker.summary().expect("tracker should have samples");
        assert_eq!(
            summary,
            BlockIntervalSummary {
                p50_ms: 500,
                p95_ms: 500,
                samples: 5,
            }
        );
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
            latency_p95_threshold: None,
            seed: Some(42),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
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
        } = instructions::prepare_state(
            account_qty,
            Some(config.peer_count),
            None,
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
        )?;
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
            latency_p95_threshold: None,
            seed: Some(7),
            tps: 1.0,
            max_inflight: 1,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
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
            Some(config.peer_count),
            config.nexus.as_ref(),
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
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
            latency_p95_threshold: None,
            seed: Some(7),
            tps: 1.0,
            max_inflight: 1,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
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
            Some(config.peer_count),
            config.nexus.as_ref(),
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
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
    fn derive_npos_timing_scales_timeouts_for_pipeline_time() {
        let config = ChaosConfig {
            allow_net: false,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(1),
            pipeline_time: Some(Duration::from_millis(3_000)),
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            latency_p95_threshold: None,
            seed: Some(7),
            tps: 1.0,
            max_inflight: 1,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: None,
        };

        let timing = derive_npos_timing(&config);
        let expected =
            SumeragiNposTimeouts::from_block_time(Duration::from_millis(timing.block_ms));
        let expected_commit = duration_ms(expected.commit).max(timing.commit_time_ms);
        let expected_da = duration_ms(expected.da).max(timing.commit_time_ms);
        assert_eq!(timing.commit_timeout_ms, expected_commit);
        assert_eq!(timing.da_ms, expected_da);
    }

    #[test]
    fn derive_npos_timing_clamps_commit_timeout_without_pipeline_time() {
        let config = ChaosConfig {
            allow_net: false,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(1),
            pipeline_time: None,
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            latency_p95_threshold: None,
            seed: Some(7),
            tps: 1.0,
            max_inflight: 1,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: None,
        };

        let timing = derive_npos_timing(&config);
        let expected =
            SumeragiNposTimeouts::from_block_time(Duration::from_millis(timing.block_ms));
        let expected_commit = duration_ms(expected.commit).max(timing.commit_time_ms);
        let expected_da = duration_ms(expected.da).max(timing.commit_time_ms);
        assert_eq!(timing.commit_timeout_ms, expected_commit);
        assert_eq!(timing.da_ms, expected_da);
        assert!(
            timing.propose_ms >= IZANAMI_NPOS_TIMEOUT_PROPOSE_MIN_MS,
            "propose timeout must respect minimum floor"
        );
        assert!(
            timing.prevote_ms >= IZANAMI_NPOS_TIMEOUT_PREVOTE_MIN_MS,
            "prevote timeout must respect minimum floor"
        );
        assert!(
            timing.precommit_ms >= IZANAMI_NPOS_TIMEOUT_PRECOMMIT_MIN_MS,
            "precommit timeout must respect minimum floor"
        );
    }

    #[test]
    fn derive_npos_timing_uses_conservative_floors_for_shared_host_npos_soak() -> Result<()> {
        let profile = crate::config::NexusProfile::sora_defaults()?;
        let config = ChaosConfig {
            allow_net: false,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(IZANAMI_SHARED_HOST_SOAK_MIN_DURATION_SECS),
            pipeline_time: None,
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            latency_p95_threshold: None,
            seed: Some(7),
            tps: 5.0,
            max_inflight: 8,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: Some(profile),
        };

        let timing = derive_npos_timing(&config);
        assert!(
            timing.propose_ms >= IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_PROPOSE_MIN_MS
                && timing.prevote_ms >= IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_PREVOTE_MIN_MS
                && timing.precommit_ms >= IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_PRECOMMIT_MIN_MS
                && timing.commit_timeout_ms >= IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_COMMIT_MIN_MS
                && timing.da_ms >= IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_DA_MIN_MS
                && timing.aggregator_ms >= IZANAMI_SHARED_HOST_SOAK_NPOS_TIMEOUT_AGGREGATOR_MIN_MS,
            "shared-host NPoS soak should enforce balanced sub-1s timeout floors"
        );
        assert_eq!(
            npos_pending_stall_grace_ms(&config, timing.block_ms),
            IZANAMI_SHARED_HOST_SOAK_PENDING_STALL_GRACE_MS
        );
        assert_eq!(
            npos_collectors_and_redundancy(&config),
            (
                IZANAMI_SHARED_HOST_SOAK_COLLECTORS_K_4_PEERS,
                IZANAMI_SHARED_HOST_SOAK_REDUNDANT_SEND_R_4_PEERS
            )
        );
        Ok(())
    }

    #[test]
    fn npos_preflight_audit_passes_for_generated_genesis() -> Result<()> {
        init_instruction_registry();
        let profile = crate::config::NexusProfile::sora_defaults()?;
        let config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(1),
            pipeline_time: None,
            target_blocks: None,
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: DEFAULT_PROGRESS_TIMEOUT,
            latency_p95_threshold: None,
            seed: Some(23),
            tps: 2.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([false, false, false, false]),
            nexus: Some(profile),
        };

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos { genesis, .. } = instructions::prepare_state(
            account_qty,
            Some(config.peer_count),
            config.nexus.as_ref(),
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
        )?;
        let network = make_network_builder(&config, genesis).build();
        let summary = audit_npos_genesis_preflight(&network.genesis(), config.peer_count)?;
        assert_eq!(summary.peer_with_pop_count, config.peer_count);
        assert_eq!(summary.register_validator_count, config.peer_count);
        assert_eq!(summary.activate_validator_count, config.peer_count);
        assert_eq!(
            summary.stake_distribution.len(),
            1,
            "generated NPoS genesis should have a uniform validator self-bond distribution"
        );
        Ok(())
    }

    #[test]
    fn npos_preflight_audit_fails_on_missing_activation() {
        init_instruction_registry();
        let min_self_bond = SumeragiNposParameters::default().min_self_bond();
        let instructions =
            synthetic_npos_preflight_instructions(4, true, false, &[min_self_bond; 4]);
        let err = audit_npos_preflight_instructions(instructions.iter(), 4, min_self_bond)
            .expect_err("missing activation should fail preflight");
        let message = err.to_string();
        assert!(
            message.contains("ActivatePublicLaneValidator count="),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn npos_preflight_audit_fails_on_missing_pop() {
        init_instruction_registry();
        let min_self_bond = SumeragiNposParameters::default().min_self_bond();
        let instructions =
            synthetic_npos_preflight_instructions(4, false, true, &[min_self_bond; 4]);
        let err = audit_npos_preflight_instructions(instructions.iter(), 4, min_self_bond)
            .expect_err("missing pop should fail preflight");
        let message = err.to_string();
        assert!(
            message.contains("RegisterPeerWithPop count="),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn npos_preflight_audit_fails_on_unequal_initial_stake() {
        init_instruction_registry();
        let min_self_bond = SumeragiNposParameters::default().min_self_bond();
        let instructions = synthetic_npos_preflight_instructions(
            4,
            true,
            true,
            &[
                min_self_bond,
                min_self_bond.saturating_add(1),
                min_self_bond,
                min_self_bond,
            ],
        );
        let err = audit_npos_preflight_instructions(instructions.iter(), 4, min_self_bond)
            .expect_err("unequal initial stake should fail preflight");
        let message = err.to_string();
        assert!(
            message.contains("non-uniform"),
            "unexpected error message: {message}"
        );
    }

    #[test]
    fn shared_host_stable_soak_profile_caps_load_and_timeout() {
        let mut config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(3_600),
            pipeline_time: None,
            target_blocks: Some(3_600),
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: Duration::from_secs(300),
            latency_p95_threshold: None,
            seed: Some(7),
            tps: 5.0,
            max_inflight: 8,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: Some(NexusProfile::sora_defaults().expect("nexus profile")),
        };

        assert!(is_shared_host_stable_soak(&config));
        apply_shared_host_stable_soak_profile(&mut config);

        assert_eq!(
            config.tps, 5.0,
            "shared-host soak should preserve canonical 5 TPS pacing"
        );
        assert!(
            config.max_inflight >= IZANAMI_SHARED_HOST_SOAK_MAX_INFLIGHT_FLOOR,
            "shared-host soak should enforce max-inflight floor"
        );
        assert!(
            config.progress_timeout
                >= Duration::from_secs(IZANAMI_SHARED_HOST_SOAK_PROGRESS_TIMEOUT_FLOOR_SECS),
            "shared-host soak should raise progress-timeout floor"
        );
        assert_eq!(
            config.pipeline_time,
            Some(Duration::from_millis(
                IZANAMI_SHARED_HOST_SOAK_PIPELINE_TIME_MS
            )),
            "shared-host soak should enforce a conservative pipeline-time floor"
        );
        assert_eq!(
            config.latency_p95_threshold,
            Some(Duration::from_secs(
                IZANAMI_SHARED_HOST_SOAK_LATENCY_P95_THRESHOLD_SECS
            )),
            "shared-host soak should default the quorum latency gate to sub-1s when unset"
        );
        let recovery = recovery_profile_for(&config);
        assert_eq!(
            recovery.height_window_ms,
            IZANAMI_SHARED_HOST_SOAK_RECOVERY_HEIGHT_WINDOW_MS
        );
        assert_eq!(
            recovery.missing_qc_reacquire_window_ms,
            IZANAMI_SHARED_HOST_SOAK_RECOVERY_MISSING_QC_REACQUIRE_WINDOW_MS
        );
        assert_eq!(
            recovery.deferred_qc_ttl_ms,
            IZANAMI_SHARED_HOST_SOAK_RECOVERY_DEFERRED_QC_TTL_MS
        );
        assert_eq!(
            recovery.hash_miss_cap_before_range_pull,
            IZANAMI_SHARED_HOST_SOAK_RECOVERY_HASH_MISS_CAP_BEFORE_RANGE_PULL
        );
        assert_eq!(
            recovery.missing_block_signer_fallback_attempts,
            IZANAMI_SHARED_HOST_SOAK_RECOVERY_MISSING_BLOCK_SIGNER_FALLBACK_ATTEMPTS
        );
        assert_eq!(
            recovery.range_pull_escalation_after_hash_misses,
            IZANAMI_SHARED_HOST_SOAK_RECOVERY_RANGE_PULL_ESCALATION_AFTER_HASH_MISSES
        );
        assert_eq!(
            recovery.da_quorum_timeout_multiplier,
            IZANAMI_SHARED_HOST_SOAK_DA_QUORUM_TIMEOUT_MULTIPLIER
        );
        assert_eq!(
            recovery.da_availability_timeout_multiplier,
            IZANAMI_SHARED_HOST_SOAK_DA_AVAILABILITY_TIMEOUT_MULTIPLIER
        );
        assert_eq!(
            recovery.da_availability_timeout_floor_ms,
            IZANAMI_SHARED_HOST_SOAK_DA_AVAILABILITY_TIMEOUT_FLOOR_MS
        );
    }

    #[test]
    fn shared_host_stable_soak_profile_applies_to_permissioned_long_run() {
        let mut config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(3_600),
            pipeline_time: None,
            target_blocks: Some(2_000),
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: Duration::from_secs(300),
            latency_p95_threshold: None,
            seed: Some(21),
            tps: 7.0,
            max_inflight: 13,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: None,
        };

        assert!(is_shared_host_stable_soak(&config));
        apply_shared_host_stable_soak_profile(&mut config);
        assert_eq!(
            config.pipeline_time,
            Some(Duration::from_millis(
                IZANAMI_SHARED_HOST_SOAK_PIPELINE_TIME_MS
            )),
            "permissioned long-run soak should use the same conservative pipeline floor"
        );
        assert_eq!(
            config.latency_p95_threshold,
            Some(Duration::from_secs(
                IZANAMI_SHARED_HOST_SOAK_LATENCY_P95_THRESHOLD_SECS
            )),
            "permissioned shared-host soak should use the same sub-1s latency gate default"
        );
        assert_eq!(config.tps, 7.0);
        assert_eq!(config.max_inflight, 14);
        assert!(
            config.progress_timeout
                >= Duration::from_secs(IZANAMI_SHARED_HOST_SOAK_PROGRESS_TIMEOUT_FLOOR_SECS)
        );
    }

    #[test]
    fn shared_host_stable_soak_consensus_overrides_match_between_permissioned_and_npos() {
        let mut permissioned = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(3_600),
            pipeline_time: None,
            target_blocks: Some(2_000),
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: Duration::from_secs(300),
            latency_p95_threshold: None,
            seed: Some(22),
            tps: 7.0,
            max_inflight: 13,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: None,
        };
        let mut npos = ChaosConfig {
            nexus: Some(NexusProfile::sora_defaults().expect("nexus profile")),
            ..permissioned.clone()
        };

        apply_shared_host_stable_soak_profile(&mut permissioned);
        apply_shared_host_stable_soak_profile(&mut npos);

        let permissioned_recovery = recovery_profile_for(&permissioned);
        let npos_recovery = recovery_profile_for(&npos);
        assert_eq!(
            permissioned_recovery, npos_recovery,
            "stable shared-host consensus recovery tuning should be mode-parity"
        );

        let permissioned_timing = derive_npos_timing(&permissioned);
        let npos_timing = derive_npos_timing(&npos);
        assert_eq!(
            npos_pending_stall_grace_ms(&permissioned, permissioned_timing.block_ms),
            npos_pending_stall_grace_ms(&npos, npos_timing.block_ms),
            "stable shared-host pending stall grace should be mode-parity"
        );
        assert_eq!(
            npos_collectors_and_redundancy(&permissioned),
            npos_collectors_and_redundancy(&npos),
            "stable shared-host collector/redundancy tuning should be mode-parity"
        );
        assert_eq!(
            is_shared_host_balanced_latency_profile(&permissioned),
            is_shared_host_balanced_latency_profile(&npos),
            "stable shared-host DA fast-reschedule policy should be mode-parity"
        );
    }

    #[test]
    fn shared_host_stable_soak_profile_pins_canonical_load_shape() {
        let mut config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(3_600),
            pipeline_time: None,
            target_blocks: Some(3_600),
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: Duration::from_secs(300),
            latency_p95_threshold: None,
            seed: Some(17),
            tps: 3.0,
            max_inflight: 6,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: Some(NexusProfile::sora_defaults().expect("nexus profile")),
        };

        assert!(is_shared_host_stable_soak(&config));
        apply_shared_host_stable_soak_profile(&mut config);

        assert_eq!(
            config.tps, IZANAMI_SHARED_HOST_SOAK_TPS_FLOOR,
            "shared-host soak should enforce canonical minimum TPS for deterministic pilots"
        );
        assert_eq!(
            config.max_inflight, 10,
            "shared-host soak should enforce max_inflight floor derived from canonical TPS floor"
        );
    }

    #[test]
    fn shared_host_stable_soak_profile_does_not_touch_non_soak_runs() {
        let mut config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(600),
            pipeline_time: None,
            target_blocks: Some(600),
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: Duration::from_secs(300),
            latency_p95_threshold: None,
            seed: Some(9),
            tps: 5.0,
            max_inflight: 8,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: Some(NexusProfile::sora_defaults().expect("nexus profile")),
        };

        assert!(!is_shared_host_stable_soak(&config));
        apply_shared_host_stable_soak_profile(&mut config);

        assert!((config.tps - 5.0).abs() <= f64::EPSILON);
        assert_eq!(config.max_inflight, 8);
        assert_eq!(config.progress_timeout, Duration::from_secs(300));
        assert_eq!(config.pipeline_time, None);
        assert_eq!(recovery_profile_for(&config), baseline_recovery_profile());
    }

    #[test]
    fn shared_host_stable_soak_profile_applies_with_single_frozen_peer() {
        let mut config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 1,
            duration: Duration::from_secs(3_600),
            pipeline_time: None,
            target_blocks: Some(2_000),
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: Duration::from_secs(300),
            latency_p95_threshold: None,
            seed: Some(29),
            tps: 4.0,
            max_inflight: 6,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: Some(NexusProfile::sora_defaults().expect("nexus profile")),
        };

        assert!(
            is_shared_host_stable_soak(&config),
            "single frozen peer should still use shared-host stable soak policy"
        );
        assert!(
            is_shared_host_stable_recovery_run(&config),
            "single frozen peer should still use shared-host recovery policy"
        );

        apply_shared_host_stable_soak_profile(&mut config);

        assert_eq!(config.tps, IZANAMI_SHARED_HOST_SOAK_TPS_FLOOR);
        assert_eq!(
            config.pipeline_time,
            Some(Duration::from_millis(
                IZANAMI_SHARED_HOST_SOAK_PIPELINE_TIME_MS
            ))
        );
        assert_eq!(
            config.latency_p95_threshold,
            Some(Duration::from_secs(
                IZANAMI_SHARED_HOST_SOAK_LATENCY_P95_THRESHOLD_SECS
            ))
        );
        assert_eq!(
            recovery_profile_for(&config),
            shared_host_recovery_profile()
        );
    }

    #[test]
    fn submission_confirmation_mode_uses_ingress_acceptance_for_stable_no_faults() {
        let config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(600),
            pipeline_time: None,
            target_blocks: Some(200),
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: Duration::from_secs(300),
            latency_p95_threshold: None,
            seed: Some(5),
            tps: 5.0,
            max_inflight: 8,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: None,
        };

        assert_eq!(
            submission_confirmation_mode(&config),
            SubmissionConfirmationMode::AcceptedByIngress
        );
    }

    #[test]
    fn submission_confirmation_mode_keeps_blocking_confirmation_for_faulty_or_chaos_runs() {
        let mut config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 1,
            duration: Duration::from_secs(600),
            pipeline_time: None,
            target_blocks: Some(200),
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: Duration::from_secs(300),
            latency_p95_threshold: None,
            seed: Some(5),
            tps: 5.0,
            max_inflight: 8,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: None,
        };

        assert_eq!(
            submission_confirmation_mode(&config),
            SubmissionConfirmationMode::BlockingApplied
        );
        config.faulty_peers = 0;
        config.workload_profile = WorkloadProfile::Chaos;
        assert_eq!(
            submission_confirmation_mode(&config),
            SubmissionConfirmationMode::BlockingApplied
        );
    }

    #[test]
    fn trigger_precheck_runs_only_for_blocking_confirmation() {
        assert!(should_run_trigger_precheck(
            SubmissionConfirmationMode::BlockingApplied
        ));
        assert!(!should_run_trigger_precheck(
            SubmissionConfirmationMode::AcceptedByIngress
        ));
    }

    #[test]
    fn trigger_state_updates_force_blocking_confirmation() {
        let updates = vec![PlanUpdate::TrackRepeatableTrigger(
            "repeat_trigger_test".parse().expect("valid trigger id"),
        )];
        assert_eq!(
            effective_submission_confirmation(
                SubmissionConfirmationMode::AcceptedByIngress,
                &updates,
            ),
            SubmissionConfirmationMode::BlockingApplied
        );
    }

    #[test]
    fn asset_and_account_state_updates_force_blocking_confirmation() {
        let key_pair = KeyPair::random();
        let account = AccountRecord {
            id: AccountId::new(key_pair.public_key().clone()),
            key_pair,
            uaid: None,
        };
        let asset = AssetId::new(
            AssetDefinitionId::new(
                "chaosnet".parse().expect("domain id"),
                "chaos_coin".parse().expect("asset name"),
            ),
            account.id.clone(),
        );
        for updates in [
            vec![PlanUpdate::TrackAccount(account)],
            vec![PlanUpdate::TrackAssetInstance(asset)],
        ] {
            assert_eq!(
                effective_submission_confirmation(
                    SubmissionConfirmationMode::AcceptedByIngress,
                    &updates,
                ),
                SubmissionConfirmationMode::BlockingApplied
            );
        }
    }

    #[test]
    fn non_trigger_state_updates_keep_ingress_confirmation() {
        let updates = Vec::<PlanUpdate>::new();
        assert_eq!(
            effective_submission_confirmation(
                SubmissionConfirmationMode::AcceptedByIngress,
                &updates,
            ),
            SubmissionConfirmationMode::AcceptedByIngress
        );
    }

    #[test]
    fn tracked_repeatable_trigger_extracts_registration_target() {
        let trigger_id: TriggerId = "repeat_trigger_test".parse().expect("valid trigger id");
        let updates = vec![PlanUpdate::TrackRepeatableTrigger(trigger_id.clone())];
        assert_eq!(tracked_repeatable_trigger(&updates), Some(trigger_id));
    }

    #[test]
    fn trigger_not_found_error_is_detected() {
        let err = eyre!("Trigger with id repeat_trigger_7 not found");
        assert!(
            is_trigger_not_found_error(&err),
            "repeatable trigger drift should classify missing-trigger errors"
        );
    }

    #[test]
    fn endpoint_pool_can_pin_and_reuse_same_endpoint() {
        let ingress_stats = Arc::new(IngressStats::default());
        let pool = EndpointHealthPool::new(
            vec![
                "http://127.0.0.1:101".to_string(),
                "http://127.0.0.1:102".to_string(),
            ],
            IngressEndpointPoolConfig {
                max_attempts: 2,
                unhealthy_failure_threshold: 1,
                unhealthy_cooldown: Duration::from_secs(5),
                reprobe_interval: Duration::from_millis(500),
            },
            ingress_stats,
        );
        let now = Instant::now();
        let endpoint_idx = pool
            .select_endpoint_at("repeatable_trigger_plan", now)
            .expect("an endpoint should be selectable");
        let mut attempts = Vec::new();
        let precheck: Result<()> = pool.run_on_endpoint_at(
            "repeatable_trigger_precheck",
            endpoint_idx,
            now,
            |idx, _| {
                attempts.push(idx);
                Ok(())
            },
        );
        assert!(precheck.is_ok(), "pinned precheck should succeed");
        let submit: Result<()> =
            pool.run_on_endpoint_at("repeatable_trigger_submit", endpoint_idx, now, |idx, _| {
                attempts.push(idx);
                Ok(())
            });
        assert!(
            submit.is_ok(),
            "pinned submit should reuse the same endpoint"
        );
        assert_eq!(
            attempts,
            vec![endpoint_idx, endpoint_idx],
            "repeatable trigger precheck and submit must stay on the pinned endpoint"
        );
    }

    #[test]
    fn parse_sumeragi_phase_snapshot_extracts_expected_fields() {
        let json = norito::json::from_str::<norito::json::Value>(
            r#"{
                "propose_ms": 11,
                "collect_da_ms": 22,
                "collect_prevote_ms": 33,
                "collect_precommit_ms": 44,
                "collect_aggregator_ms": 55,
                "commit_ms": 66,
                "pipeline_total_ms": 176,
                "ema_ms": {
                    "pipeline_total_ms": 123
                }
            }"#,
        )
        .expect("valid phase JSON");
        let snapshot = parse_sumeragi_phase_snapshot(json).expect("phase snapshot should parse");
        assert_eq!(
            snapshot,
            SumeragiPhaseSnapshot {
                propose_ms: 11,
                collect_da_ms: 22,
                collect_prevote_ms: 33,
                collect_precommit_ms: 44,
                collect_aggregator_ms: 55,
                commit_ms: 66,
                pipeline_total_ms: 176,
                pipeline_total_ema_ms: 123,
            }
        );
    }

    #[test]
    fn parse_sumeragi_phase_snapshot_rejects_missing_ema() {
        let json = norito::json::from_str::<norito::json::Value>(
            r#"{
                "propose_ms": 11,
                "collect_da_ms": 22,
                "collect_prevote_ms": 33,
                "collect_precommit_ms": 44,
                "collect_aggregator_ms": 55,
                "commit_ms": 66,
                "pipeline_total_ms": 176
            }"#,
        )
        .expect("valid phase JSON");
        assert!(
            parse_sumeragi_phase_snapshot(json).is_none(),
            "phase snapshot parser should reject incomplete payloads"
        );
    }

    #[test]
    fn shared_host_recovery_profile_applies_to_stable_pilot_runs() {
        let config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(1_200),
            pipeline_time: None,
            target_blocks: Some(1_200),
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: Duration::from_secs(300),
            latency_p95_threshold: None,
            seed: Some(12),
            tps: 5.0,
            max_inflight: 8,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: Some(NexusProfile::sora_defaults().expect("nexus profile")),
        };

        assert!(!is_shared_host_stable_soak(&config));
        assert!(is_shared_host_stable_recovery_run(&config));
        assert_eq!(
            recovery_profile_for(&config),
            shared_host_recovery_profile()
        );
    }

    #[test]
    fn shared_host_recovery_profile_applies_to_permissioned_stable_pilot_runs() {
        let config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(1_200),
            pipeline_time: None,
            target_blocks: Some(1_200),
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: Duration::from_secs(300),
            latency_p95_threshold: None,
            seed: Some(13),
            tps: 5.0,
            max_inflight: 8,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: None,
        };

        assert!(!is_shared_host_stable_soak(&config));
        assert!(is_shared_host_stable_recovery_run(&config));
        assert_eq!(
            recovery_profile_for(&config),
            shared_host_recovery_profile()
        );
    }

    #[test]
    fn shared_host_recovery_profile_does_not_apply_below_pilot_duration() {
        let config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(600),
            pipeline_time: None,
            target_blocks: Some(600),
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: Duration::from_secs(300),
            latency_p95_threshold: None,
            seed: Some(15),
            tps: 5.0,
            max_inflight: 8,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: Some(NexusProfile::sora_defaults().expect("nexus profile")),
        };

        assert!(!is_shared_host_stable_recovery_run(&config));
        assert_eq!(recovery_profile_for(&config), baseline_recovery_profile());
    }

    #[test]
    fn shared_host_recovery_profile_config_layer_writes_tuned_da_timeouts() -> Result<()> {
        init_instruction_registry();
        let config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(1_200),
            pipeline_time: None,
            target_blocks: Some(1_200),
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: Duration::from_secs(300),
            latency_p95_threshold: None,
            seed: Some(27),
            tps: 5.0,
            max_inflight: 8,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: Some(NexusProfile::sora_defaults().expect("nexus profile")),
        };

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos { genesis, .. } = instructions::prepare_state(
            account_qty,
            Some(config.peer_count),
            config.nexus.as_ref(),
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
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

        let layers: Vec<Table> = network.config_layers().map(Cow::into_owned).collect();
        let lookup = |path: &[&str]| {
            layers.iter().rev().find_map(|layer| {
                let mut current = layer;
                for (idx, key) in path.iter().enumerate() {
                    let value = current.get(*key)?;
                    if idx + 1 == path.len() {
                        return value.as_integer();
                    }
                    current = value.as_table()?;
                }
                None
            })
        };

        assert_eq!(
            lookup(&["sumeragi", "advanced", "da", "quorum_timeout_multiplier"]),
            Some(IZANAMI_SHARED_HOST_SOAK_DA_QUORUM_TIMEOUT_MULTIPLIER)
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "advanced",
                "da",
                "availability_timeout_multiplier"
            ]),
            Some(IZANAMI_SHARED_HOST_SOAK_DA_AVAILABILITY_TIMEOUT_MULTIPLIER)
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "advanced",
                "da",
                "availability_timeout_floor_ms"
            ]),
            Some(IZANAMI_SHARED_HOST_SOAK_DA_AVAILABILITY_TIMEOUT_FLOOR_MS)
        );

        Ok(())
    }

    #[test]
    fn shared_host_stable_soak_profile_keeps_higher_pipeline_time() {
        let mut config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(3_600),
            pipeline_time: Some(Duration::from_millis(1_200)),
            target_blocks: Some(3_600),
            progress_interval: DEFAULT_PROGRESS_INTERVAL,
            progress_timeout: Duration::from_secs(300),
            latency_p95_threshold: Some(Duration::from_secs(2)),
            seed: Some(11),
            tps: 5.0,
            max_inflight: 8,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: Some(NexusProfile::sora_defaults().expect("nexus profile")),
        };

        assert!(is_shared_host_stable_soak(&config));
        apply_shared_host_stable_soak_profile(&mut config);
        assert_eq!(config.pipeline_time, Some(Duration::from_millis(1_200)));
        assert_eq!(config.latency_p95_threshold, Some(Duration::from_secs(2)));
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
            latency_p95_threshold: None,
            seed: Some(7),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
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
            Some(config.peer_count),
            config.nexus.as_ref(),
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
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
        let succeeded = run_submission("success", true, Arc::clone(&metrics), || Ok(())).await;
        assert!(succeeded);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.successes, 1);
        assert_eq!(snapshot.failures, 0);
        assert_eq!(snapshot.expected_failures, 0);
        assert_eq!(snapshot.unexpected_successes, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_submission_records_failure() {
        let metrics = Arc::new(Metrics::default());
        let succeeded = run_submission("failure", true, Arc::clone(&metrics), || {
            Err(eyre!("submission failed"))
        })
        .await;
        assert!(!succeeded);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.successes, 0);
        assert_eq!(snapshot.failures, 1);
        assert_eq!(snapshot.expected_failures, 0);
        assert_eq!(snapshot.unexpected_successes, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_submission_records_expected_failure() {
        let metrics = Arc::new(Metrics::default());
        let succeeded = run_submission("expected_failure", false, Arc::clone(&metrics), || {
            Err(eyre!("submission failed"))
        })
        .await;
        assert!(!succeeded);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.successes, 0);
        assert_eq!(snapshot.failures, 0);
        assert_eq!(snapshot.expected_failures, 1);
        assert_eq!(snapshot.unexpected_successes, 0);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn run_submission_records_unexpected_success() {
        let metrics = Arc::new(Metrics::default());
        let succeeded =
            run_submission("unexpected_success", false, Arc::clone(&metrics), || Ok(())).await;
        assert!(succeeded);

        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.successes, 0);
        assert_eq!(snapshot.failures, 0);
        assert_eq!(snapshot.expected_failures, 0);
        assert_eq!(snapshot.unexpected_successes, 1);
    }

    #[test]
    fn run_with_queue_timeout_retry_retries_and_succeeds() {
        let attempts = AtomicU64::new(0);
        let result =
            run_with_queue_timeout_retry_with_policy("retry_success", 2, Duration::ZERO, || {
                let attempt = attempts.fetch_add(1, Ordering::Relaxed);
                if attempt == 0 {
                    Err(eyre!("transaction queued for too long"))
                } else {
                    Ok(())
                }
            });
        assert!(result.is_ok(), "queue-timeout retries should recover");
        assert_eq!(
            attempts.load(Ordering::Relaxed),
            2,
            "helper should perform one retry before succeeding"
        );
    }

    #[test]
    fn run_with_queue_timeout_retry_retries_on_no_endpoint_backpressure() {
        let attempts = AtomicU64::new(0);
        let result = run_with_queue_timeout_retry_with_policy(
            "retry_no_endpoint_backpressure",
            2,
            Duration::ZERO,
            || {
                let attempt = attempts.fetch_add(1, Ordering::Relaxed);
                if attempt == 0 {
                    Err(eyre!(
                        "no ingress endpoints available for operation `submit_all_blocking_with_metadata`"
                    ))
                } else {
                    Ok(())
                }
            },
        );
        assert!(
            result.is_ok(),
            "temporary no-endpoint backpressure should be retried"
        );
        assert_eq!(
            attempts.load(Ordering::Relaxed),
            2,
            "helper should perform one retry before succeeding after no-endpoint backpressure"
        );
    }

    #[test]
    fn queue_timeout_retry_delay_uses_dynamic_floor_for_no_endpoint_backpressure() {
        let backoff = Duration::from_millis(250);
        let dynamic = Duration::from_secs(3);
        assert_eq!(
            queue_timeout_retry_delay(backoff, true, Some(dynamic)),
            dynamic,
            "no-endpoint backpressure should honor the pool-derived cooldown floor"
        );
        assert_eq!(
            queue_timeout_retry_delay(backoff, true, None),
            Duration::from_millis(IZANAMI_INGRESS_REPROBE_INTERVAL_MS),
            "no-endpoint backpressure without a pool delay should fall back to reprobe floor"
        );
        assert_eq!(
            queue_timeout_retry_delay(backoff, false, Some(dynamic)),
            backoff,
            "non-endpoint retryable failures should keep exponential backoff pacing"
        );
    }

    #[test]
    fn run_with_queue_timeout_retry_stops_on_non_retryable_error() {
        let attempts = AtomicU64::new(0);
        let result = run_with_queue_timeout_retry_with_policy(
            "retry_non_retryable",
            2,
            Duration::ZERO,
            || {
                attempts.fetch_add(1, Ordering::Relaxed);
                Err(eyre!("permission denied"))
            },
        );
        assert!(result.is_err(), "non-retryable errors should bubble up");
        assert_eq!(
            attempts.load(Ordering::Relaxed),
            1,
            "non-retryable failures must not be retried"
        );
    }

    #[test]
    fn run_with_queue_timeout_retry_treats_duplicate_rejection_as_success() {
        let attempts = AtomicU64::new(0);
        let result = run_with_queue_timeout_retry_with_policy(
            "retry_duplicate_idempotent",
            2,
            Duration::ZERO,
            || {
                attempts.fetch_add(1, Ordering::Relaxed);
                Err(eyre!(
                    "Transaction rejected: Repetition of `Register` for id `chaos_nft_4$chaosnet`"
                ))
            },
        );
        assert!(
            result.is_ok(),
            "duplicate register rejections should be treated as idempotent success"
        );
        assert_eq!(
            attempts.load(Ordering::Relaxed),
            1,
            "idempotent duplicate should not trigger extra retries"
        );
    }

    #[test]
    fn ingress_failover_marks_queue_timeout_retryable() {
        let err = eyre!("transaction queued for too long");
        assert!(
            is_ingress_failover_retryable(&err),
            "queue timeout errors should trigger endpoint failover"
        );
    }

    #[test]
    fn ingress_failover_marks_http_429_retryable() {
        let err = eyre!("Failed to get pipeline transaction status: 429 Too Many Requests");
        assert!(
            is_ingress_failover_retryable(&err),
            "HTTP 429 should be treated as queue-pressure backpressure"
        );
    }

    #[test]
    fn ingress_queue_timeout_retryable_for_no_endpoint_backpressure() {
        let err = eyre!(
            "no ingress endpoints available for operation `submit_all_blocking_with_metadata`"
        );
        assert!(
            is_ingress_queue_timeout_retryable(&err),
            "no-endpoint ingress backpressure should be retryable for submit helper"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn await_worker_shutdown_returns_for_completed_tasks() {
        let handle = tokio::spawn(async {});
        await_worker_shutdown_with_timeout(vec![handle], "test", Duration::from_millis(100)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn await_worker_shutdown_aborts_hung_tasks() {
        let handle = tokio::spawn(async {
            loop {
                time::sleep(Duration::from_secs(60)).await;
            }
        });

        timeout(
            Duration::from_secs(1),
            await_worker_shutdown_with_timeout(vec![handle], "test", Duration::from_millis(10)),
        )
        .await
        .expect("worker shutdown should return after aborting hung tasks");
    }

    #[test]
    fn endpoint_pool_rotates_on_retryable_failure() {
        let ingress_stats = Arc::new(IngressStats::default());
        let pool = EndpointHealthPool::new(
            vec![
                "http://127.0.0.1:1".to_string(),
                "http://127.0.0.1:2".to_string(),
            ],
            IngressEndpointPoolConfig {
                max_attempts: 3,
                unhealthy_failure_threshold: 1,
                unhealthy_cooldown: Duration::from_secs(5),
                reprobe_interval: Duration::from_millis(500),
            },
            Arc::clone(&ingress_stats),
        );
        let now = Instant::now();
        let mut attempts = Vec::new();
        let result: Result<&'static str> = pool.run_with_failover_at("submit", now, |idx, _| {
            attempts.push(idx);
            if idx == 0 {
                Err(eyre!("connection refused"))
            } else {
                Ok("ok")
            }
        });
        assert_eq!(result.expect("alternate endpoint should succeed"), "ok");
        assert_eq!(attempts, vec![0, 1]);
        assert_eq!(
            ingress_stats.snapshot(),
            IngressStatsSnapshot {
                failover_total: 1,
                endpoint_unhealthy_total: 1
            }
        );
    }

    #[test]
    fn endpoint_pool_query_confirmation_fails_over() {
        let ingress_stats = Arc::new(IngressStats::default());
        let pool = EndpointHealthPool::new(
            vec![
                "http://127.0.0.1:11".to_string(),
                "http://127.0.0.1:12".to_string(),
            ],
            IngressEndpointPoolConfig {
                max_attempts: 3,
                unhealthy_failure_threshold: 1,
                unhealthy_cooldown: Duration::from_secs(5),
                reprobe_interval: Duration::from_millis(500),
            },
            ingress_stats,
        );
        let now = Instant::now();
        let mut attempts = Vec::new();
        let result: Result<Option<u32>> =
            pool.run_with_failover_at("query_confirmation", now, |idx, _| {
                attempts.push(idx);
                if idx == 0 {
                    Err(eyre!("request timed out"))
                } else {
                    Ok(Some(9))
                }
            });
        assert_eq!(
            result.expect("query should fail over to alternate endpoint"),
            Some(9)
        );
        assert_eq!(attempts, vec![0, 1]);
    }

    #[test]
    fn endpoint_pool_respects_cooldown_then_reprobes() {
        let ingress_stats = Arc::new(IngressStats::default());
        let pool = EndpointHealthPool::new(
            vec![
                "http://127.0.0.1:21".to_string(),
                "http://127.0.0.1:22".to_string(),
            ],
            IngressEndpointPoolConfig {
                max_attempts: 3,
                unhealthy_failure_threshold: 1,
                unhealthy_cooldown: Duration::from_secs(10),
                reprobe_interval: Duration::from_secs(2),
            },
            ingress_stats,
        );
        let start = Instant::now();
        let first: Result<&'static str> = pool.run_with_failover_at("submit", start, |idx, _| {
            if idx == 0 {
                Err(eyre!("connection refused"))
            } else {
                Ok("ok")
            }
        });
        assert_eq!(first.expect("second endpoint should succeed"), "ok");
        assert!(
            pool.endpoint_state(0).unhealthy_until.is_some(),
            "first endpoint should enter cooldown after retryable failure"
        );

        let mut attempts_before_reprobe = Vec::new();
        let second: Result<&'static str> =
            pool.run_with_failover_at("submit", start + Duration::from_secs(1), |idx, _| {
                attempts_before_reprobe.push(idx);
                Ok("ok")
            });
        assert_eq!(
            second.expect("healthy endpoint should continue serving"),
            "ok"
        );
        assert_eq!(
            attempts_before_reprobe,
            vec![1],
            "cooldown should suppress early reprobes"
        );

        let mut attempts_after_reprobe = Vec::new();
        let third: Result<&'static str> =
            pool.run_with_failover_at("submit", start + Duration::from_secs(3), |idx, _| {
                attempts_after_reprobe.push(idx);
                if idx == 1 {
                    Err(eyre!("timed out"))
                } else {
                    Ok("recovered")
                }
            });
        assert_eq!(
            third.expect("pool should reprobe cooled endpoint and recover"),
            "recovered"
        );
        assert_eq!(
            attempts_after_reprobe,
            vec![1, 0],
            "reprobe should include the unhealthy endpoint after the interval"
        );
    }

    #[test]
    fn ingress_pool_excludes_lagging_endpoint_when_healthy_alternatives_exist() {
        let ingress_stats = Arc::new(IngressStats::default());
        let pool = EndpointHealthPool::new(
            vec![
                "http://127.0.0.1:61".to_string(),
                "http://127.0.0.1:62".to_string(),
                "http://127.0.0.1:63".to_string(),
            ],
            IngressEndpointPoolConfig {
                max_attempts: 3,
                unhealthy_failure_threshold: 1,
                unhealthy_cooldown: Duration::from_secs(10),
                reprobe_interval: Duration::from_secs(5),
            },
            ingress_stats,
        );
        let now = Instant::now();
        pool.update_lag_snapshot(IngressLagSnapshot {
            quorum_min_height: 220,
            peer_heights: vec![Some(180), Some(220), Some(221)],
            observed_at: now,
        });

        let mut attempts = Vec::new();
        let result: Result<&'static str> = pool.run_with_failover_at("submit", now, |idx, _| {
            attempts.push(idx);
            Ok("ok")
        });
        assert_eq!(
            result.expect("healthy non-lagging endpoint should succeed"),
            "ok"
        );
        assert_eq!(
            attempts,
            vec![1],
            "lagging endpoint should be excluded while healthy non-lagging alternatives exist"
        );
    }

    #[test]
    fn ingress_pool_forced_probe_when_all_endpoints_excluded_or_unhealthy() {
        let ingress_stats = Arc::new(IngressStats::default());
        let pool = EndpointHealthPool::new(
            vec![
                "http://127.0.0.1:71".to_string(),
                "http://127.0.0.1:72".to_string(),
                "http://127.0.0.1:73".to_string(),
            ],
            IngressEndpointPoolConfig {
                max_attempts: 3,
                unhealthy_failure_threshold: 1,
                unhealthy_cooldown: Duration::from_secs(30),
                reprobe_interval: Duration::from_secs(30),
            },
            ingress_stats,
        );
        let start = Instant::now();
        assert!(pool.mark_failure_at(1, start, IngressFailureClass::Retryable));
        assert!(pool.mark_failure_at(2, start, IngressFailureClass::Retryable));
        {
            let mut guard = pool
                .state
                .lock()
                .expect("endpoint health state mutex should not be poisoned");
            if let Some(state) = guard.get_mut(1) {
                state.last_probe_at = Some(start);
            }
            if let Some(state) = guard.get_mut(2) {
                state.last_probe_at = Some(start);
            }
        }
        pool.update_lag_snapshot(IngressLagSnapshot {
            quorum_min_height: 300,
            peer_heights: vec![Some(250), Some(300), Some(301)],
            observed_at: start,
        });

        let mut attempts = Vec::new();
        let result: Result<&'static str> =
            pool.run_with_failover_at("submit", start + Duration::from_secs(1), |idx, _| {
                attempts.push(idx);
                Ok("forced")
            });
        assert_eq!(
            result.expect("forced probe should avoid a dead-end"),
            "forced"
        );
        assert_eq!(
            attempts,
            vec![0],
            "when all non-lagging endpoints are unavailable, forced probe should use the excluded lagging endpoint"
        );
    }

    #[test]
    fn queue_pressure_sticky_cooldown_blocks_early_reprobe_after_threshold() {
        let ingress_stats = Arc::new(IngressStats::default());
        let pool = EndpointHealthPool::new(
            vec![
                "http://127.0.0.1:81".to_string(),
                "http://127.0.0.1:82".to_string(),
            ],
            IngressEndpointPoolConfig {
                max_attempts: 2,
                unhealthy_failure_threshold: 3,
                unhealthy_cooldown: Duration::from_secs(5),
                reprobe_interval: Duration::from_millis(500),
            },
            ingress_stats,
        );
        let start = Instant::now();
        let first: Result<&'static str> = pool.run_with_failover_at("submit", start, |idx, _| {
            if idx == 0 {
                Err(eyre!("transaction queued for too long"))
            } else {
                Ok("ok")
            }
        });
        assert_eq!(
            first.expect("healthy alternate endpoint should succeed"),
            "ok"
        );
        assert!(
            pool.endpoint_state(0).unhealthy_until.is_none(),
            "first queue-pressure failure should not quarantine endpoint before threshold"
        );
        assert!(
            !pool.mark_failure_at(
                0,
                start + Duration::from_millis(10),
                IngressFailureClass::QueuePressure
            ),
            "second queue-pressure failure should still remain below threshold"
        );
        assert!(
            pool.mark_failure_at(
                0,
                start + Duration::from_millis(20),
                IngressFailureClass::QueuePressure
            ),
            "queue-pressure endpoint should become unhealthy once threshold is reached"
        );

        let mut attempts = Vec::new();
        let second: Result<&'static str> =
            pool.run_with_failover_at("submit", start + Duration::from_secs(1), |idx, _| {
                attempts.push(idx);
                Ok("ok")
            });
        assert_eq!(second.expect("submission should still succeed"), "ok");
        assert_eq!(
            attempts,
            vec![1],
            "sticky queue-pressure endpoint should not be reprobed early while healthy alternatives exist"
        );
    }

    #[test]
    fn ingress_fsm_excludes_lagging_when_healthy_alternative_exists() {
        ingress_pool_excludes_lagging_endpoint_when_healthy_alternatives_exist();
    }

    #[test]
    fn ingress_fsm_forced_probe_when_all_excluded_or_unhealthy() {
        ingress_pool_forced_probe_when_all_endpoints_excluded_or_unhealthy();
    }

    #[test]
    fn ingress_queue_pressure_cooldown_blocks_early_reprobe_once_threshold_reached() {
        queue_pressure_sticky_cooldown_blocks_early_reprobe_after_threshold();
    }

    #[test]
    fn ingress_submission_backpressure_defers_when_all_endpoints_recently_probed_and_unhealthy() {
        let ingress_stats = Arc::new(IngressStats::default());
        let pool = EndpointHealthPool::new(
            vec![
                "http://127.0.0.1:91".to_string(),
                "http://127.0.0.1:92".to_string(),
            ],
            IngressEndpointPoolConfig {
                max_attempts: 2,
                unhealthy_failure_threshold: 1,
                unhealthy_cooldown: Duration::from_secs(10),
                reprobe_interval: Duration::from_secs(2),
            },
            ingress_stats,
        );
        let start = Instant::now();
        assert!(pool.mark_failure_at(0, start, IngressFailureClass::QueuePressure));
        assert!(pool.mark_failure_at(1, start, IngressFailureClass::Retryable));
        {
            let mut guard = pool
                .state
                .lock()
                .expect("endpoint health state mutex should not be poisoned");
            for endpoint_state in guard.iter_mut() {
                endpoint_state.last_probe_at = Some(start);
            }
        }

        let now = start + Duration::from_millis(100);
        let attempt_order = pool.attempt_order_at(now);
        assert!(
            attempt_order.is_empty(),
            "no endpoint should be reprobed before reprobe interval elapses"
        );
        let backpressure = pool
            .submission_backpressure_delay_at(now)
            .expect("backpressure should be active while all endpoints are cooling down");
        assert!(
            backpressure >= Duration::from_millis(1),
            "backpressure delay should be bounded and positive"
        );
        assert!(
            backpressure <= Duration::from_secs(2),
            "backpressure should be bounded by reprobe interval under cooldown saturation"
        );
    }

    #[test]
    fn all_unhealthy_pool_promotes_successful_probe_to_healthy_endpoint() {
        let ingress_stats = Arc::new(IngressStats::default());
        let pool = EndpointHealthPool::new(
            vec![
                "http://127.0.0.1:111".to_string(),
                "http://127.0.0.1:112".to_string(),
                "http://127.0.0.1:113".to_string(),
            ],
            IngressEndpointPoolConfig {
                max_attempts: 3,
                unhealthy_failure_threshold: 1,
                unhealthy_cooldown: Duration::from_secs(30),
                reprobe_interval: Duration::from_secs(2),
            },
            ingress_stats,
        );
        let start = Instant::now();
        assert!(pool.mark_failure_at(0, start, IngressFailureClass::QueuePressure));
        assert!(pool.mark_failure_at(1, start, IngressFailureClass::QueuePressure));
        assert!(pool.mark_failure_at(2, start, IngressFailureClass::QueuePressure));

        let mut first_attempts = Vec::new();
        let first: Result<&'static str> =
            pool.run_with_failover_at("submit", start + Duration::from_secs(1), |idx, _| {
                first_attempts.push(idx);
                Ok("probe")
            });
        assert_eq!(first.expect("first unhealthy probe should run"), "probe");
        assert_eq!(
            first_attempts.len(),
            1,
            "all-unhealthy path should probe only one endpoint per interval"
        );

        let mut second_attempts = Vec::new();
        let second: Result<&'static str> =
            pool.run_with_failover_at("submit", start + Duration::from_secs(1), |idx, _| {
                second_attempts.push(idx);
                Ok("recovered")
            });
        assert_eq!(
            second.expect("successful probe should recover one endpoint immediately"),
            "recovered"
        );
        assert_eq!(
            second_attempts.len(),
            1,
            "after successful probe promotion, a single healthy endpoint should serve follow-up requests"
        );

        let mut third_attempts = Vec::new();
        let third: Result<&'static str> =
            pool.run_with_failover_at("submit", start + Duration::from_secs(3), |idx, _| {
                third_attempts.push(idx);
                Ok("probe")
            });
        assert_eq!(
            third.expect("probe should reopen after reprobe interval"),
            "probe"
        );
        assert_eq!(
            third_attempts.len(),
            1,
            "reopened all-unhealthy interval should still probe exactly one endpoint"
        );
    }

    #[test]
    fn ingress_submission_backpressure_is_disabled_when_endpoint_is_ready() {
        let ingress_stats = Arc::new(IngressStats::default());
        let pool = EndpointHealthPool::new(
            vec![
                "http://127.0.0.1:93".to_string(),
                "http://127.0.0.1:94".to_string(),
            ],
            IngressEndpointPoolConfig {
                max_attempts: 2,
                unhealthy_failure_threshold: 1,
                unhealthy_cooldown: Duration::from_secs(10),
                reprobe_interval: Duration::from_secs(2),
            },
            ingress_stats,
        );
        let now = Instant::now();
        assert!(pool.mark_failure_at(0, now, IngressFailureClass::QueuePressure));
        assert!(
            pool.submission_backpressure_delay_at(now + Duration::from_millis(100))
                .is_none(),
            "healthy endpoint availability should disable submit backpressure"
        );
    }

    #[test]
    fn endpoint_pool_queue_timeout_respects_unhealthy_failure_threshold() {
        let ingress_stats = Arc::new(IngressStats::default());
        let pool = EndpointHealthPool::new(
            vec!["http://127.0.0.1:31".to_string()],
            IngressEndpointPoolConfig {
                max_attempts: 1,
                unhealthy_failure_threshold: 3,
                unhealthy_cooldown: Duration::from_secs(5),
                reprobe_interval: Duration::from_millis(500),
            },
            ingress_stats,
        );
        let now = Instant::now();
        let result: Result<()> = pool.run_with_failover_at("submit", now, |_idx, _| {
            Err(eyre!("transaction queued for too long"))
        });
        assert!(result.is_err(), "queue-timeout failure should bubble up");
        let state_after_first_failure = pool.endpoint_state(0);
        assert!(
            state_after_first_failure.unhealthy_until.is_none(),
            "first queue-timeout failure should stay below unhealthy threshold"
        );

        let second_result: Result<()> =
            pool.run_with_failover_at("submit", now + Duration::from_millis(10), |_idx, _| {
                Err(eyre!("transaction queued for too long"))
            });
        assert!(
            second_result.is_err(),
            "second queue-timeout failure should still bubble up"
        );
        let state_after_second_failure = pool.endpoint_state(0);
        assert!(
            state_after_second_failure.unhealthy_until.is_none(),
            "second queue-timeout failure should stay below unhealthy threshold"
        );

        let third_result: Result<()> =
            pool.run_with_failover_at("submit", now + Duration::from_millis(20), |_idx, _| {
                Err(eyre!("transaction queued for too long"))
            });
        assert!(
            third_result.is_err(),
            "third queue-timeout failure should bubble up"
        );
        let state = pool.endpoint_state(0);
        assert!(
            state.unhealthy_until.is_some(),
            "queue-timeout endpoint should become unhealthy after threshold failures"
        );
        assert_eq!(
            state.sticky_unhealthy_until, state.unhealthy_until,
            "queue-timeout unhealthy state should use sticky cooldown tracking"
        );
    }

    #[test]
    fn endpoint_pool_queue_timeout_sticky_cooldown_clears_on_success_when_all_endpoints_unhealthy()
    {
        let ingress_stats = Arc::new(IngressStats::default());
        let pool = EndpointHealthPool::new(
            vec!["http://127.0.0.1:41".to_string()],
            IngressEndpointPoolConfig {
                max_attempts: 1,
                unhealthy_failure_threshold: 1,
                unhealthy_cooldown: Duration::from_secs(5),
                reprobe_interval: Duration::from_millis(500),
            },
            ingress_stats,
        );
        let start = Instant::now();
        let first: Result<()> = pool.run_with_failover_at("submit", start, |_idx, _| {
            Err(eyre!("transaction queued for too long"))
        });
        assert!(first.is_err(), "first queue-timeout should fail");
        let state_after_failure = pool.endpoint_state(0);
        let cooldown_until = state_after_failure
            .unhealthy_until
            .expect("queue-timeout should set cooldown");
        assert!(
            cooldown_until > start,
            "cooldown should extend beyond initial failure timestamp"
        );

        let early = start + Duration::from_secs(1);
        let second: Result<&'static str> =
            pool.run_with_failover_at("submit", early, |_idx, _| Ok("ok"));
        assert_eq!(
            second.expect("early probe success should still complete request"),
            "ok"
        );
        let state_after_early_success = pool.endpoint_state(0);
        assert!(
            state_after_early_success.unhealthy_until.is_none(),
            "successful probe should immediately clear sticky cooldown when all endpoints were unhealthy"
        );

        let after_expiry = cooldown_until + Duration::from_millis(1);
        let third: Result<&'static str> =
            pool.run_with_failover_at("submit", after_expiry, |_idx, _| Ok("ok"));
        assert_eq!(
            third.expect("endpoint should recover after sticky cooldown expiry"),
            "ok"
        );
        assert!(
            pool.endpoint_state(0).unhealthy_until.is_none(),
            "sticky queue-timeout cooldown should clear after expiry"
        );
    }

    #[test]
    fn endpoint_pool_queue_timeout_cooldown_uses_configured_unhealthy_cooldown() {
        let ingress_stats = Arc::new(IngressStats::default());
        let pool = EndpointHealthPool::new(
            vec!["http://127.0.0.1:51".to_string()],
            IngressEndpointPoolConfig {
                max_attempts: 1,
                unhealthy_failure_threshold: 1,
                unhealthy_cooldown: Duration::from_secs(1),
                reprobe_interval: Duration::from_millis(500),
            },
            ingress_stats,
        );
        let start = Instant::now();
        let result: Result<()> = pool.run_with_failover_at("submit", start, |_idx, _| {
            Err(eyre!("transaction queued for too long"))
        });
        assert!(result.is_err(), "queue-timeout failure should bubble up");
        let state = pool.endpoint_state(0);
        let unhealthy_until = state
            .unhealthy_until
            .expect("queue-timeout failure should set unhealthy cooldown");
        assert!(
            unhealthy_until.saturating_duration_since(start) >= Duration::from_secs(1),
            "queue-timeout cooldown should respect configured unhealthy cooldown"
        );
        assert!(
            unhealthy_until.saturating_duration_since(start) < Duration::from_secs(2),
            "queue-timeout cooldown should not be stretched to status-timeout floor"
        );
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
    fn pending_stall_grace_scales_with_block_time() {
        assert_eq!(pending_stall_grace_ms(100), 100);
        assert_eq!(pending_stall_grace_ms(200), 100);
        assert_eq!(pending_stall_grace_ms(300), 150);
        assert_eq!(pending_stall_grace_ms(4_000), 1_000);
    }

    #[test]
    fn progress_state_tracks_stalls() {
        let start = Instant::now();
        let mut state = ProgressState::new(start);
        assert!(!state.stalled(start, Duration::from_secs(5)));
        assert!(state.update(start, 0).is_none());
        assert!(!state.stalled(start + Duration::from_secs(2), Duration::from_secs(5)));
        assert!(state.update(start + Duration::from_secs(3), 2).is_some());
        assert!(!state.stalled(start + Duration::from_secs(6), Duration::from_secs(5)));
        assert!(state.stalled(start + Duration::from_secs(9), Duration::from_secs(5)));
    }

    #[test]
    fn divergence_state_trips_after_sustained_window() {
        let start = Instant::now();
        let mut state = HeightDivergenceState::new();
        let threshold = 2;
        let window = Duration::from_secs(30);

        assert!(state.observe(start, 3, threshold));
        assert!(!state.violated(start + Duration::from_secs(29), window));
        assert!(state.violated(start + Duration::from_secs(30), window));
    }

    #[test]
    fn divergence_state_resets_when_converged() {
        let start = Instant::now();
        let mut state = HeightDivergenceState::new();
        let threshold = 2;
        let window = Duration::from_secs(30);

        assert!(state.observe(start, 4, threshold));
        assert!(!state.observe(start + Duration::from_secs(10), 1, threshold));
        assert!(!state.violated(start + Duration::from_secs(40), window));
        assert!(
            state.observe(start + Duration::from_secs(41), 5, threshold),
            "fresh divergence window should start after convergence reset"
        );
        assert!(!state.violated(start + Duration::from_secs(60), window));
        assert!(state.violated(start + Duration::from_secs(71), window));
    }

    #[test]
    fn tolerated_peer_failures_matches_bft_window() {
        assert_eq!(tolerated_peer_failures(0), 0);
        assert_eq!(tolerated_peer_failures(1), 0);
        assert_eq!(tolerated_peer_failures(3), 0);
        assert_eq!(tolerated_peer_failures(4), 1);
        assert_eq!(tolerated_peer_failures(7), 2);
    }

    #[test]
    fn effective_tolerated_peer_failures_honors_configured_fault_budget() {
        assert_eq!(effective_tolerated_peer_failures(4, 0), 0);
        assert_eq!(effective_tolerated_peer_failures(4, 1), 1);
        assert_eq!(
            effective_tolerated_peer_failures(7, 1),
            1,
            "strict guard should only tolerate the configured injected-fault budget"
        );
        assert_eq!(
            effective_tolerated_peer_failures(7, 5),
            2,
            "configured budget must still remain bounded by protocol tolerance"
        );
    }

    #[test]
    fn quorum_min_height_respects_effective_tolerance() {
        assert_eq!(quorum_min_height_from_samples(vec![], 0), 0);
        assert_eq!(
            quorum_min_height_from_samples(vec![246, 316, 316, 316], 0),
            246,
            "healthy runs should fail-open on no unexpected stragglers"
        );
        assert_eq!(
            quorum_min_height_from_samples(vec![246, 316, 316, 316], 1),
            316
        );
        assert_eq!(
            quorum_min_height_from_samples(vec![0, 0, 316, 316], 1),
            0,
            "two failed peers should not be hidden for a 4-peer run"
        );
        assert_eq!(quorum_min_height_from_samples(vec![9, 10, 11], 0), 9);
    }

    #[test]
    fn strict_divergence_reference_height_trims_only_effectively_tolerated_outliers() {
        assert_eq!(
            strict_divergence_reference_height_from_samples(vec![], 0),
            0
        );
        assert_eq!(
            strict_divergence_reference_height_from_samples(vec![120, 149, 149, 149], 1),
            149,
            "single lagging outlier should not lower the strict divergence reference"
        );
        assert_eq!(
            strict_divergence_reference_height_from_samples(vec![120, 149, 149, 149], 0),
            149,
            "zero tolerated failures should anchor the strict reference on the highest healthy sample"
        );
        assert_eq!(
            strict_divergence_reference_height_from_samples(vec![120, 121, 149, 149], 1),
            149,
            "two lagging peers should keep the strict reference on the healthy quorum side"
        );
        assert_eq!(
            strict_divergence_reference_height_from_samples(vec![120, 120, 120, 149], 1),
            120,
            "single leading outlier should not raise the strict divergence reference"
        );
    }

    #[test]
    fn strict_divergence_lagging_peer_count_respects_threshold() {
        let strict_reference = 149_u64;
        assert_eq!(
            strict_divergence_lagging_peer_count(&[141, 149, 149, 149], strict_reference, 16),
            0,
            "lag below threshold should not count as strict divergence"
        );
        assert_eq!(
            strict_divergence_lagging_peer_count(&[141, 149, 149, 149], strict_reference, 2),
            1,
            "lag above threshold should be counted"
        );
    }

    #[test]
    fn strict_divergence_guard_requires_more_than_tolerated_outliers() {
        let heights_one_outlier = vec![120_u64, 149, 149, 149];
        let strict_reference_one =
            strict_divergence_reference_height_from_samples(heights_one_outlier.clone(), 1);
        let lagging_one =
            strict_divergence_lagging_peer_count(&heights_one_outlier, strict_reference_one, 16);
        assert_eq!(lagging_one, 1);
        assert!(
            lagging_one <= tolerated_peer_failures(heights_one_outlier.len()),
            "a single outlier in a 4-peer run should stay within tolerated failures"
        );

        let heights_two_outliers = vec![120_u64, 121, 149, 149];
        let strict_reference_two =
            strict_divergence_reference_height_from_samples(heights_two_outliers.clone(), 1);
        assert_eq!(
            strict_reference_two, 149,
            "strict reference should stay on the healthy quorum side for two lagging peers"
        );
        let lagging_two =
            strict_divergence_lagging_peer_count(&heights_two_outliers, strict_reference_two, 16);
        assert_eq!(lagging_two, 2);
        assert!(
            lagging_two > tolerated_peer_failures(heights_two_outliers.len()),
            "strict divergence guard should activate once outliers exceed tolerated failures"
        );
    }

    #[test]
    fn strict_progress_timeout_enforcement_respects_bft_tolerance() {
        assert!(
            should_enforce_strict_progress_timeout(0, 1),
            "global stalls with no lagging peers must enforce strict-timeout failure"
        );
        assert!(
            !should_enforce_strict_progress_timeout(1, 1),
            "a tolerated single outlier should not force strict-timeout failure"
        );
        assert!(
            should_enforce_strict_progress_timeout(2, 1),
            "strict-timeout should be enforced once lagging peers exceed BFT tolerance"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn drain_ready_submissions_clears_completed_tasks() {
        let mut set = JoinSet::new();
        set.spawn(async {});
        set.spawn(async {});

        let deadline = Instant::now() + Duration::from_secs(1);
        while set.len() > 0 && Instant::now() < deadline {
            tokio::task::yield_now().await;
            drain_ready_submissions(&mut set);
        }
        assert_eq!(
            set.len(),
            0,
            "drain should clear completed submissions within timeout"
        );
    }

    #[test]
    fn submission_backlog_limit_scales_from_max_inflight() {
        assert_eq!(
            submission_backlog_limit(8),
            8 * IZANAMI_SUBMISSION_BACKLOG_MULTIPLIER
        );
        assert_eq!(
            submission_backlog_limit(0),
            IZANAMI_SUBMISSION_BACKLOG_MULTIPLIER
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn submission_permit_scope_does_not_block_precheck_phase() {
        let semaphore = Arc::new(Semaphore::new(1));
        let saturated = semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("must acquire permit");
        let precheck_started = Arc::new(Notify::new());
        let submit_started = Arc::new(Notify::new());

        let semaphore_for_task = Arc::clone(&semaphore);
        let precheck_for_task = Arc::clone(&precheck_started);
        let submit_for_task = Arc::clone(&submit_started);
        let task = tokio::spawn(async move {
            precheck_for_task.notify_one();
            let permit = semaphore_for_task
                .acquire_owned()
                .await
                .expect("submit stage should acquire permit once available");
            submit_for_task.notify_one();
            drop(permit);
        });

        timeout(Duration::from_millis(200), precheck_started.notified())
            .await
            .expect("precheck phase should proceed even when submit permits are saturated");
        assert!(
            timeout(Duration::from_millis(100), submit_started.notified())
                .await
                .is_err(),
            "submit stage should remain blocked while permits are saturated"
        );

        drop(saturated);
        timeout(Duration::from_secs(1), submit_started.notified())
            .await
            .expect("submit stage should proceed after permit release");
        task.await.expect("permit-scope task should finish");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_for_submission_capacity_enforces_backlog_bound() {
        let backlog_limit = submission_backlog_limit(1);
        let gate = Arc::new(Notify::new());
        let mut submissions = JoinSet::new();
        for _ in 0..backlog_limit {
            let gate = Arc::clone(&gate);
            submissions.spawn(async move {
                gate.notified().await;
            });
        }
        assert_eq!(submissions.len(), backlog_limit);

        let stop_notify = Notify::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        {
            let wait_future = wait_for_submission_capacity(
                &mut submissions,
                backlog_limit,
                &stop_notify,
                deadline,
            );
            tokio::pin!(wait_future);

            assert!(
                timeout(Duration::from_millis(100), &mut wait_future)
                    .await
                    .is_err(),
                "capacity wait should block while backlog remains saturated"
            );

            gate.notify_one();

            assert!(
                timeout(Duration::from_secs(1), &mut wait_future)
                    .await
                    .expect("capacity wait should complete after one task finishes"),
                "capacity wait should continue when stop/deadline are not reached"
            );
        }
        assert!(
            submissions.len() < backlog_limit,
            "draining one completed submission should reduce backlog below the cap"
        );

        let gate_for_new_submission = Arc::clone(&gate);
        submissions.spawn(async move {
            gate_for_new_submission.notified().await;
        });
        assert!(
            submissions.len() <= backlog_limit,
            "queued submissions should stay bounded by max_inflight * backlog multiplier"
        );

        submissions.abort_all();
        while let Some(result) = submissions.join_next().await {
            let _ = result;
        }
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
            latency_p95_threshold: None,
            seed: Some(9),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
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
        } = instructions::prepare_state(
            account_qty,
            Some(config.peer_count),
            None,
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
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

        let run_control = RunControl::new(Instant::now() + Duration::from_secs(20));
        let progress = wait_for_target_blocks(
            network.peers(),
            2,
            0,
            Duration::from_millis(200),
            Duration::from_secs(5),
            None,
            &run_control,
            None,
            false,
        )
        .await?;
        assert!(progress.target_reached);
        network.shutdown().await;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_for_target_blocks_soft_kpi_allows_duration_completion_without_target()
    -> Result<()> {
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
            latency_p95_threshold: None,
            seed: Some(10),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
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
        } = instructions::prepare_state(
            account_qty,
            Some(config.peer_count),
            None,
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
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

        let run_control = RunControl::new(Instant::now() + Duration::from_secs(3));
        let target_blocks = 10_000;
        let progress = wait_for_target_blocks(
            network.peers(),
            target_blocks,
            0,
            Duration::from_millis(200),
            Duration::from_secs(30),
            None,
            &run_control,
            None,
            true,
        )
        .await?;
        assert!(
            !progress.target_reached,
            "soft-KPI monitoring should complete duration without forcing a target hit"
        );
        assert!(
            progress.quorum_min_height < target_blocks,
            "expected unmet target for soft-KPI duration test"
        );
        network.shutdown().await;

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_for_target_blocks_soft_kpi_enforces_latency_gate_at_duration_end() -> Result<()> {
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
            latency_p95_threshold: None,
            seed: Some(11),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
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
        } = instructions::prepare_state(
            account_qty,
            Some(config.peer_count),
            None,
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
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

        let run_control = RunControl::new(Instant::now() + Duration::from_secs(3));
        let target_blocks = 10_000;
        let result = wait_for_target_blocks(
            network.peers(),
            target_blocks,
            0,
            Duration::from_millis(200),
            Duration::from_secs(30),
            Some(Duration::from_millis(1)),
            &run_control,
            None,
            true,
        )
        .await;
        network.shutdown().await;

        let err = result.expect_err(
            "soft-KPI runs should fail when the latency p95 gate is exceeded at duration end",
        );
        assert!(
            err.to_string().contains("p95 block interval")
                && err.to_string().contains("checkpoint duration_deadline"),
            "expected duration-end latency gate error, got: {err}"
        );

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn wait_for_target_blocks_explicit_stop_is_error_even_in_soft_kpi_mode() {
        let run_control = RunControl::new(Instant::now() + Duration::from_secs(30));
        run_control.stop();
        let err = wait_for_target_blocks(
            &[],
            100,
            0,
            Duration::from_millis(5),
            Duration::from_secs(1),
            None,
            &run_control,
            None,
            true,
        )
        .await
        .expect_err("explicit stop must terminate target monitoring");
        assert!(
            err.to_string()
                .contains("izanami run stopped before target blocks reached"),
            "unexpected stop error: {err}"
        );
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
            latency_p95_threshold: None,
            seed: Some(5),
            tps: 0.1,
            max_inflight: 1,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
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
            latency_p95_threshold: None,
            seed: Some(17),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([false, false, false, false]),
            nexus: None,
        };

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos { genesis, .. } = instructions::prepare_state(
            account_qty,
            Some(config.peer_count),
            None,
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
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
        let read_string = |layer: &Table, path: &[&str]| -> Option<String> {
            let mut current = layer;
            for (idx, key) in path.iter().enumerate() {
                let value = current.get(*key)?;
                if idx + 1 == path.len() {
                    return value.as_str().map(str::to_string);
                }
                current = value.as_table()?;
            }
            None
        };
        let lookup = |path| layers.iter().rev().find_map(|layer| read_i64(layer, path));
        let lookup_bool = |path| layers.iter().rev().find_map(|layer| read_bool(layer, path));
        let lookup_string = |path| {
            layers
                .iter()
                .rev()
                .find_map(|layer| read_string(layer, path))
        };
        let npos_timing = derive_npos_timing(&config);
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
        let pending_stall_ms = npos_pending_stall_grace_ms(&config, npos_timing.block_ms);
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
            lookup_string(&["kura", "fsync_mode"]),
            Some(IZANAMI_KURA_FSYNC_MODE.to_string())
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "queues", "block_payload"]),
            Some(IZANAMI_BLOCK_PAYLOAD_QUEUE)
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "advanced",
                "worker",
                "validation_worker_threads"
            ]),
            Some(IZANAMI_VALIDATION_WORKER_THREADS)
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "advanced",
                "worker",
                "validation_work_queue_cap"
            ]),
            Some(IZANAMI_VALIDATION_WORK_QUEUE_CAP)
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "advanced",
                "worker",
                "validation_result_queue_cap"
            ]),
            Some(IZANAMI_VALIDATION_RESULT_QUEUE_CAP)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "worker", "validation_pending_cap"]),
            Some(IZANAMI_VALIDATION_PENDING_CAP)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "worker", "iteration_budget_cap_ms"]),
            Some(IZANAMI_WORKER_ITERATION_BUDGET_CAP_MS)
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "advanced",
                "worker",
                "iteration_drain_budget_cap_ms"
            ]),
            Some(IZANAMI_WORKER_ITERATION_DRAIN_BUDGET_CAP_MS)
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "advanced",
                "pacemaker",
                "pending_stall_grace_ms"
            ]),
            Some(pending_stall_ms)
        );
        assert_eq!(
            lookup_bool(&["sumeragi", "advanced", "pacemaker", "da_fast_reschedule"]),
            Some(is_shared_host_balanced_latency_profile(&config))
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "advanced",
                "pacemaker",
                "active_pending_soft_limit"
            ]),
            Some(IZANAMI_PACEMAKER_ACTIVE_PENDING_SOFT_LIMIT)
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "advanced",
                "pacemaker",
                "rbc_backlog_session_soft_limit",
            ]),
            Some(IZANAMI_PACEMAKER_RBC_BACKLOG_SESSION_SOFT_LIMIT)
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "advanced",
                "pacemaker",
                "rbc_backlog_chunk_soft_limit",
            ]),
            Some(IZANAMI_PACEMAKER_RBC_BACKLOG_CHUNK_SOFT_LIMIT)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "rbc", "pending_max_chunks"]),
            Some(IZANAMI_RBC_PENDING_MAX_CHUNKS)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "rbc", "pending_max_bytes"]),
            Some(IZANAMI_RBC_PENDING_MAX_BYTES)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "rbc", "pending_session_limit"]),
            Some(IZANAMI_RBC_PENDING_SESSION_LIMIT)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "rbc", "pending_ttl_ms"]),
            Some(IZANAMI_RBC_PENDING_TTL_MS)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "rbc", "session_ttl_ms"]),
            Some(IZANAMI_RBC_SESSION_TTL_MS)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "rbc", "disk_store_ttl_ms"]),
            Some(IZANAMI_RBC_SESSION_TTL_MS)
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "advanced",
                "rbc",
                "rebroadcast_sessions_per_tick"
            ]),
            Some(IZANAMI_RBC_REBROADCAST_SESSIONS_PER_TICK)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "rbc", "payload_chunks_per_tick"]),
            Some(IZANAMI_RBC_PAYLOAD_CHUNKS_PER_TICK)
        );
        assert_eq!(
            lookup(&["sumeragi", "recovery", "height_attempt_cap"]),
            Some(IZANAMI_RECOVERY_HEIGHT_ATTEMPT_CAP)
        );
        assert_eq!(
            lookup(&["sumeragi", "recovery", "height_window_ms"]),
            Some(IZANAMI_RECOVERY_HEIGHT_WINDOW_MS)
        );
        assert_eq!(
            lookup(&["sumeragi", "recovery", "missing_qc_reacquire_window_ms"]),
            Some(IZANAMI_RECOVERY_MISSING_QC_REACQUIRE_WINDOW_MS)
        );
        assert_eq!(
            lookup(&["sumeragi", "recovery", "deferred_qc_ttl_ms"]),
            Some(IZANAMI_RECOVERY_DEFERRED_QC_TTL_MS)
        );
        assert_eq!(
            lookup(&["sumeragi", "recovery", "missing_block_height_ttl_ms"]),
            Some(IZANAMI_RECOVERY_MISSING_BLOCK_HEIGHT_TTL_MS)
        );
        assert_eq!(
            lookup(&["sumeragi", "recovery", "hash_miss_cap_before_range_pull"]),
            Some(IZANAMI_RECOVERY_HASH_MISS_CAP_BEFORE_RANGE_PULL)
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "recovery",
                "missing_block_signer_fallback_attempts",
            ]),
            Some(IZANAMI_RECOVERY_MISSING_BLOCK_SIGNER_FALLBACK_ATTEMPTS)
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "recovery",
                "missing_block_retry_backoff_multiplier",
            ]),
            Some(IZANAMI_RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_MULTIPLIER)
        );
        assert_eq!(
            lookup(&["sumeragi", "recovery", "missing_block_retry_backoff_cap_ms",]),
            Some(IZANAMI_RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_CAP_MS)
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "recovery",
                "range_pull_escalation_after_hash_misses",
            ]),
            Some(IZANAMI_RECOVERY_RANGE_PULL_ESCALATION_AFTER_HASH_MISSES)
        );
        assert_eq!(
            lookup(&["sumeragi", "advanced", "da", "quorum_timeout_multiplier"]),
            Some(IZANAMI_DA_QUORUM_TIMEOUT_MULTIPLIER)
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "advanced",
                "da",
                "availability_timeout_multiplier"
            ]),
            Some(IZANAMI_DA_AVAILABILITY_TIMEOUT_MULTIPLIER)
        );
        assert_eq!(
            lookup(&[
                "sumeragi",
                "advanced",
                "da",
                "availability_timeout_floor_ms"
            ]),
            Some(IZANAMI_DA_AVAILABILITY_TIMEOUT_FLOOR_MS)
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
    fn make_network_builder_forwards_rust_log_and_sets_peer_base_level() -> Result<()> {
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
            latency_p95_threshold: None,
            seed: Some(19),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([false, false, false, false]),
            nexus: None,
        };

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos { genesis, .. } = instructions::prepare_state(
            account_qty,
            Some(config.peer_count),
            None,
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
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
        let level = layers
            .iter()
            .rev()
            .find_map(|layer| read_str(layer, &["logger", "level"]));
        assert_eq!(level.as_deref(), Some(IZANAMI_PEER_LOG_BASE_LEVEL));

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
            latency_p95_threshold: None,
            seed: Some(19),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([false, false, false, false]),
            nexus: Some(profile),
        };

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos { genesis, .. } = instructions::prepare_state(
            account_qty,
            Some(config.peer_count),
            config.nexus.as_ref(),
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
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
        assert_eq!(
            network.pipeline_time(),
            default_nexus_pipeline_time(),
            "nexus runs without explicit pipeline_time should use Izanami fast pipeline defaults"
        );
        assert_eq!(params.sumeragi().block_time_ms(), expected.block_ms);
        assert_eq!(params.sumeragi().commit_time_ms(), expected.commit_time_ms);
        let injected_npos_params = params
            .custom()
            .get(&SumeragiNposParameters::parameter_id())
            .and_then(SumeragiNposParameters::from_custom_parameter)
            .expect("nexus runs should inject sumeragi_npos custom parameter");
        assert_eq!(
            injected_npos_params.max_validators(),
            u32::try_from(config.peer_count).unwrap_or(u32::MAX),
            "izanami should cap NPoS election set to active peer count in soak runs"
        );
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
        let layers: Vec<Table> = network.config_layers().map(Cow::into_owned).collect();
        let expected_gas_account = instructions::nexus_gas_account_id().to_string();
        for path in [
            &["pipeline", "gas", "tech_account_id"][..],
            &["nexus", "fees", "fee_sink_account_id"][..],
            &["nexus", "staking", "stake_escrow_account_id"][..],
            &["nexus", "staking", "slash_sink_account_id"][..],
        ] {
            let actual = layers.iter().rev().find_map(|layer| read_str(layer, path));
            assert_eq!(
                actual.as_deref(),
                Some(expected_gas_account.as_str()),
                "config override for {:?} should use deterministic Izanami gas account",
                path
            );
        }
        Ok(())
    }

    #[test]
    fn make_network_builder_uses_fast_pipeline_default_without_nexus() -> Result<()> {
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
            latency_p95_threshold: None,
            seed: Some(71),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([false, false, false, false]),
            nexus: None,
        };
        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos { genesis, .. } = instructions::prepare_state(
            account_qty,
            Some(config.peer_count),
            config.nexus.as_ref(),
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
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
        assert_eq!(network.pipeline_time(), default_izanami_pipeline_time());
        Ok(())
    }

    #[test]
    fn shared_host_stable_soak_pipeline_timing_keeps_block_at_or_above_min_finality() -> Result<()>
    {
        init_instruction_registry();
        let mut config = ChaosConfig {
            allow_net: true,
            peer_count: 4,
            faulty_peers: 0,
            duration: Duration::from_secs(3_600),
            pipeline_time: None,
            target_blocks: Some(2_000),
            progress_interval: Duration::from_secs(10),
            progress_timeout: Duration::from_secs(600),
            latency_p95_threshold: None,
            seed: Some(47),
            tps: 5.0,
            max_inflight: 8,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(5)..=Duration::from_secs(20),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([true, true, true, true]),
            nexus: None,
        };
        apply_shared_host_stable_soak_profile(&mut config);

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos { genesis, .. } = instructions::prepare_state(
            account_qty,
            Some(config.peer_count),
            config.nexus.as_ref(),
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
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

        let pipeline_time = config
            .pipeline_time
            .expect("shared-host soak profile should materialize pipeline time");
        let (pipeline_block_ms, pipeline_commit_ms) = split_pipeline_time(pipeline_time);
        assert_eq!(params.sumeragi().block_time_ms(), pipeline_block_ms);
        assert_eq!(params.sumeragi().commit_time_ms(), pipeline_commit_ms);
        assert_eq!(params.sumeragi().min_finality_ms(), pipeline_block_ms);
        assert!(
            params.sumeragi().block_time_ms() >= params.sumeragi().min_finality_ms(),
            "shared-host soak block_time must satisfy min_finality invariant"
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
            latency_p95_threshold: None,
            seed: Some(13),
            tps: 0.5,
            max_inflight: 2,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
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
            latency_p95_threshold: None,
            seed: Some(23),
            tps: 1.0,
            max_inflight: 4,
            workload_profile: WorkloadProfile::Stable,
            allow_contract_deploy_in_stable: false,
            fault_interval: Duration::from_secs(1)..=Duration::from_secs(1),
            log_filter: "warn".to_string(),
            faults: FaultToggles::from_array([false, false, false, false]),
            nexus: Some(nexus.clone()),
        };

        let account_qty = config.peer_count.saturating_mul(3).max(6);
        let PreparedChaos { genesis, .. } = instructions::prepare_state(
            account_qty,
            Some(config.peer_count),
            config.nexus.as_ref(),
            config.workload_profile,
            config.allow_contract_deploy_in_stable,
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
