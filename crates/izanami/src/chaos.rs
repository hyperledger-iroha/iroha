//! Orchestration layer that wires configuration, workload generation, and fault injection together.

use std::{
    borrow::Cow,
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
use iroha_data_model::{
    parameter::SumeragiParameter, parameter::system::SumeragiNposParameters, prelude::*,
    query::trigger::prelude::FindTriggers, trigger::action::Repeats,
};
use iroha_genesis::GenesisBlock;
use iroha_test_network::{Network, NetworkBuilder, NetworkPeer};
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
    instructions::{self, AccountRecord, PreparedChaos, TransactionPlan, WorkloadEngine},
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
const IZANAMI_PACEMAKER_ACTIVE_PENDING_SOFT_LIMIT: i64 = 16;
const IZANAMI_PACEMAKER_RBC_BACKLOG_SESSION_SOFT_LIMIT: i64 = 16;
const IZANAMI_PACEMAKER_RBC_BACKLOG_CHUNK_SOFT_LIMIT: i64 = 256;
const IZANAMI_PACING_GOVERNOR_MIN_FACTOR_BPS: i64 = 10_000;
const IZANAMI_PACING_GOVERNOR_MAX_FACTOR_BPS: i64 = 10_000;
const IZANAMI_COLLECTORS_K: u16 = 4;
const IZANAMI_REDUNDANT_SEND_R: u8 = 4;
const IZANAMI_PACING_FACTOR_BPS: u32 = 10_000;
// Shared-host soak profile: bias towards deterministic progress over peak throughput.
const IZANAMI_DA_QUORUM_TIMEOUT_MULTIPLIER: i64 = 3;
const IZANAMI_DA_AVAILABILITY_TIMEOUT_MULTIPLIER: i64 = 2;
const IZANAMI_DA_AVAILABILITY_TIMEOUT_FLOOR_MS: i64 = 1_000;
const IZANAMI_FUTURE_HEIGHT_WINDOW: i64 = 2;
const IZANAMI_FUTURE_VIEW_WINDOW: i64 = 2;
const IZANAMI_NPOS_BLOCK_TIME_MS: i64 = 150;
const IZANAMI_NPOS_COMMIT_TIME_MS: i64 = 240;
const IZANAMI_RECOVERY_HEIGHT_ATTEMPT_CAP: i64 = 24;
const IZANAMI_RECOVERY_HEIGHT_WINDOW_MS: i64 = 6_000;
const IZANAMI_RECOVERY_MISSING_QC_REACQUIRE_WINDOW_MS: i64 = 3_000;
const IZANAMI_RECOVERY_NO_ROSTER_FALLBACK_VIEWS: i64 = 2;
const IZANAMI_RECOVERY_NO_ROSTER_REFRESH_RETRY_PER_VIEW: i64 = 2;
const IZANAMI_RECOVERY_DEFERRED_QC_TTL_MS: i64 = 6_000;
const IZANAMI_RECOVERY_MISSING_BLOCK_HEIGHT_TTL_MS: i64 = IZANAMI_RECOVERY_HEIGHT_WINDOW_MS;
const IZANAMI_RECOVERY_HASH_MISS_CAP_BEFORE_RANGE_PULL: i64 = 2;
const IZANAMI_RECOVERY_MISSING_BLOCK_SIGNER_FALLBACK_ATTEMPTS: i64 = 1;
const IZANAMI_RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_MULTIPLIER: i64 = 3;
const IZANAMI_RECOVERY_MISSING_BLOCK_RETRY_BACKOFF_CAP_MS: i64 = 20_000;
const IZANAMI_RECOVERY_RANGE_PULL_ESCALATION_AFTER_HASH_MISSES: i64 = 2;
const IZANAMI_NPOS_TIMEOUT_PROPOSE_MIN_MS: u64 = 40;
const IZANAMI_NPOS_TIMEOUT_PREVOTE_MIN_MS: u64 = 60;
const IZANAMI_NPOS_TIMEOUT_PRECOMMIT_MIN_MS: u64 = 80;
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
const IZANAMI_INGRESS_MAX_ATTEMPTS: usize = 3;
const IZANAMI_INGRESS_UNHEALTHY_FAILURE_THRESHOLD: u32 = 2;
const IZANAMI_INGRESS_UNHEALTHY_COOLDOWN_MS: u64 = 5_000;
const IZANAMI_INGRESS_REPROBE_INTERVAL_MS: u64 = 1_000;
const IZANAMI_INGRESS_REQUEST_TIMEOUT_MS: u64 = 5_000;
const IZANAMI_INGRESS_STATUS_TIMEOUT_MS: u64 = 20_000;
const IZANAMI_QUEUE_TIMEOUT_RETRY_ATTEMPTS: u32 = 2;
const IZANAMI_QUEUE_TIMEOUT_RETRY_BACKOFF_MS: u64 = 250;
const IZANAMI_WORKER_SHUTDOWN_TIMEOUT_SECS: u64 = 30;
const IZANAMI_WORKER_FAILURE_SHUTDOWN_TIMEOUT_SECS: u64 = 5;
const IZANAMI_PEER_LOG_BASE_LEVEL: &str = "WARN";
const IZANAMI_STRICT_HEIGHT_DIVERGENCE_MAX_BLOCKS: u64 = 16;
const IZANAMI_STRICT_HEIGHT_DIVERGENCE_MAX_WINDOW_SECS: u64 = 60;
const IZANAMI_SHARED_HOST_RECOVERY_MIN_DURATION_SECS: u64 = 1_200;
const IZANAMI_SHARED_HOST_SOAK_MIN_DURATION_SECS: u64 = 3_600;
const IZANAMI_SHARED_HOST_SOAK_TPS_CAP: f64 = 5.0;
const IZANAMI_SHARED_HOST_SOAK_MAX_INFLIGHT_CAP: usize = 8;
const IZANAMI_SUBMISSION_BACKLOG_MULTIPLIER: usize = 8;
const IZANAMI_SHARED_HOST_SOAK_PROGRESS_TIMEOUT_FLOOR_SECS: u64 = 600;
const IZANAMI_SHARED_HOST_SOAK_PIPELINE_TIME_MS: u64 = 600;
const IZANAMI_SHARED_HOST_SOAK_DA_QUORUM_TIMEOUT_MULTIPLIER: i64 = 1;
const IZANAMI_SHARED_HOST_SOAK_DA_AVAILABILITY_TIMEOUT_MULTIPLIER: i64 = 2;
const IZANAMI_SHARED_HOST_SOAK_DA_AVAILABILITY_TIMEOUT_FLOOR_MS: i64 = 1_000;
const IZANAMI_SHARED_HOST_SOAK_RECOVERY_HEIGHT_WINDOW_MS: i64 = 4_000;
const IZANAMI_SHARED_HOST_SOAK_RECOVERY_MISSING_QC_REACQUIRE_WINDOW_MS: i64 = 2_500;
const IZANAMI_SHARED_HOST_SOAK_RECOVERY_DEFERRED_QC_TTL_MS: i64 = 4_000;
const IZANAMI_SHARED_HOST_SOAK_RECOVERY_HASH_MISS_CAP_BEFORE_RANGE_PULL: i64 = 1;
const IZANAMI_SHARED_HOST_SOAK_RECOVERY_MISSING_BLOCK_SIGNER_FALLBACK_ATTEMPTS: i64 = 0;
const IZANAMI_SHARED_HOST_SOAK_RECOVERY_RANGE_PULL_ESCALATION_AFTER_HASH_MISSES: i64 = 1;

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct EndpointHealthState {
    consecutive_failures: u32,
    unhealthy_until: Option<Instant>,
    last_probe_at: Option<Instant>,
    sticky_unhealthy_until: Option<Instant>,
}

#[derive(Clone)]
struct EndpointHealthPool {
    labels: Arc<Vec<String>>,
    state: Arc<StdMutex<Vec<EndpointHealthState>>>,
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
            cursor: Arc::new(AtomicU64::new(0)),
            config,
            ingress_stats,
        }
    }

    fn run_with_failover<T, F>(&self, op_name: &'static str, operation: F) -> Result<T>
    where
        F: FnMut(usize, &str) -> Result<T>,
    {
        self.run_with_failover_at(op_name, Instant::now(), operation)
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

    fn attempt_order_at(&self, now: Instant) -> Vec<usize> {
        let len = self.labels.len();
        if len == 0 {
            return Vec::new();
        }
        let base = self.cursor.fetch_add(1, Ordering::Relaxed);
        let base_idx_u64 = base % u64::try_from(len).unwrap_or(1);
        let base_idx = usize::try_from(base_idx_u64).unwrap_or(0);
        let mut healthy = Vec::with_capacity(len);
        let mut probes = Vec::new();
        let mut forced_probe: Option<(usize, Instant)> = None;
        let mut guard = self
            .state
            .lock()
            .expect("endpoint health state mutex should not be poisoned");
        for offset in 0..len {
            let idx = (base_idx + offset) % len;
            let state = &mut guard[idx];
            let still_unhealthy = state.unhealthy_until.is_some_and(|until| now < until);
            if !still_unhealthy {
                state.unhealthy_until = None;
                state.sticky_unhealthy_until = None;
                healthy.push(idx);
                continue;
            }
            let probe_due = state.last_probe_at.is_none_or(|last| {
                now.saturating_duration_since(last) >= self.config.reprobe_interval
            });
            if probe_due {
                state.last_probe_at = Some(now);
                probes.push(idx);
            }
            if let Some(unhealthy_until) = state.unhealthy_until {
                let should_replace = forced_probe
                    .map(|(_, current_until)| unhealthy_until < current_until)
                    .unwrap_or(true);
                if should_replace {
                    forced_probe = Some((idx, unhealthy_until));
                }
            }
        }
        if healthy.is_empty() && probes.is_empty() {
            if let Some((idx, _)) = forced_probe {
                if let Some(state) = guard.get_mut(idx) {
                    state.last_probe_at = Some(now);
                }
                probes.push(idx);
            }
        }
        healthy.extend(probes);
        healthy
    }

    fn mark_success_at(&self, endpoint_idx: usize, now: Instant) {
        if let Ok(mut guard) = self.state.lock() {
            if let Some(state) = guard.get_mut(endpoint_idx) {
                state.consecutive_failures = 0;
                if state
                    .sticky_unhealthy_until
                    .is_some_and(|until| now < until)
                {
                    return;
                }
                state.unhealthy_until = None;
                state.sticky_unhealthy_until = None;
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
        if !failure_class.is_retryable() {
            return false;
        }
        let failure_threshold = if matches!(failure_class, IngressFailureClass::QueuePressure) {
            1
        } else {
            self.config.unhealthy_failure_threshold
        };
        if state.consecutive_failures < failure_threshold {
            return false;
        }
        let cooldown = if matches!(failure_class, IngressFailureClass::QueuePressure) {
            self.config
                .unhealthy_cooldown
                .max(Duration::from_millis(IZANAMI_INGRESS_STATUS_TIMEOUT_MS))
        } else {
            self.config.unhealthy_cooldown
        };
        let was_unhealthy = state.unhealthy_until.is_some_and(|until| now < until);
        let unhealthy_until = now.checked_add(cooldown).unwrap_or(now);
        state.unhealthy_until = Some(unhealthy_until);
        state.sticky_unhealthy_until =
            matches!(failure_class, IngressFailureClass::QueuePressure).then_some(unhealthy_until);
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
        let health = EndpointHealthPool::new(labels, config, ingress_stats);
        Self {
            endpoints: Arc::new(endpoints),
            health,
        }
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

fn is_ingress_queue_pressure_message(message: &str) -> bool {
    message.contains("transaction queued for too long")
        || message.contains("status_timeout_ms")
        || message.contains("haven't got tx confirmation within")
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

fn is_ingress_failover_retryable(error: &color_eyre::Report) -> bool {
    classify_ingress_failure(error).is_retryable()
}

fn is_ingress_queue_timeout_retryable(error: &color_eyre::Report) -> bool {
    is_ingress_queue_pressure_message(&ingress_error_message(error))
}

fn run_with_queue_timeout_retry<F>(plan_label: &'static str, submit: F) -> Result<()>
where
    F: FnMut() -> Result<()>,
{
    run_with_queue_timeout_retry_with_policy(
        plan_label,
        IZANAMI_QUEUE_TIMEOUT_RETRY_ATTEMPTS,
        Duration::from_millis(IZANAMI_QUEUE_TIMEOUT_RETRY_BACKOFF_MS),
        submit,
    )
}

fn run_with_queue_timeout_retry_with_policy<F>(
    plan_label: &'static str,
    max_retry_attempts: u32,
    initial_backoff: Duration,
    mut submit: F,
) -> Result<()>
where
    F: FnMut() -> Result<()>,
{
    let mut backoff = initial_backoff;
    for attempt in 0..=max_retry_attempts {
        match submit() {
            Ok(()) => return Ok(()),
            Err(err)
                if is_ingress_queue_timeout_retryable(&err) && attempt < max_retry_attempts =>
            {
                let retry_in = backoff;
                let next_attempt = attempt.saturating_add(2);
                warn!(
                    target: "izanami::workload",
                    plan = plan_label,
                    next_attempt,
                    max_attempts = max_retry_attempts.saturating_add(1),
                    ?retry_in,
                    ?err,
                    "submission queue timeout observed; retrying plan submission"
                );
                if !retry_in.is_zero() {
                    std::thread::sleep(retry_in);
                }
                backoff = backoff.saturating_mul(2);
            }
            Err(err) => return Err(err),
        }
    }
    Err(eyre!("submission retry loop exhausted without a result"))
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

fn pending_stall_grace_ms(block_ms: u64) -> i64 {
    let scaled = block_ms
        .saturating_div(2)
        .max(IZANAMI_PACEMAKER_PENDING_STALL_FLOOR_MS);
    let capped =
        scaled.min(u64::try_from(IZANAMI_PACEMAKER_PENDING_STALL_GRACE_MS).unwrap_or(u64::MAX));
    i64::try_from(capped).unwrap_or(i64::MAX)
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
    // Derive per-phase timeouts from the scaled block time to keep soak cadence tight.
    let timeouts = SumeragiNposTimeouts::from_block_time(Duration::from_millis(timeout_block_ms));
    let propose_ms =
        clamp_nonzero_ms(duration_ms(timeouts.propose)).max(IZANAMI_NPOS_TIMEOUT_PROPOSE_MIN_MS);
    let prevote_ms =
        clamp_nonzero_ms(duration_ms(timeouts.prevote)).max(IZANAMI_NPOS_TIMEOUT_PREVOTE_MIN_MS);
    let precommit_ms = clamp_nonzero_ms(duration_ms(timeouts.precommit))
        .max(IZANAMI_NPOS_TIMEOUT_PRECOMMIT_MIN_MS);
    // Keep commit/DA windows at least as large as the target commit time for DA stability.
    let mut commit_timeout_ms = clamp_nonzero_ms(duration_ms(timeouts.commit));
    let mut da_ms = clamp_nonzero_ms(duration_ms(timeouts.da));
    if clamp_commit {
        commit_timeout_ms = commit_timeout_ms.max(commit_time_ms);
        da_ms = da_ms.max(commit_time_ms);
    }
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

fn default_nexus_pipeline_time() -> Duration {
    let block_ms =
        u64::try_from(IZANAMI_NPOS_BLOCK_TIME_MS).expect("izanami block time must be non-negative");
    let commit_ms = u64::try_from(IZANAMI_NPOS_COMMIT_TIME_MS)
        .expect("izanami commit time must be non-negative");
    Duration::from_millis(block_ms.saturating_add(commit_ms))
}

fn is_shared_host_stable_soak(config: &ChaosConfig) -> bool {
    is_shared_host_stable_recovery_run(config)
        && config.duration >= Duration::from_secs(IZANAMI_SHARED_HOST_SOAK_MIN_DURATION_SECS)
}

fn is_shared_host_stable_recovery_run(config: &ChaosConfig) -> bool {
    config.nexus.is_some()
        && matches!(config.workload_profile, WorkloadProfile::Stable)
        && config.faulty_peers == 0
        && config.peer_count >= 4
        && config.duration >= Duration::from_secs(IZANAMI_SHARED_HOST_RECOVERY_MIN_DURATION_SECS)
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

    // Keep the shared-host soak load shape fixed so pilot runs stay comparable.
    config.tps = IZANAMI_SHARED_HOST_SOAK_TPS_CAP;
    config.max_inflight = IZANAMI_SHARED_HOST_SOAK_MAX_INFLIGHT_CAP;
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

    if (config.tps - original_tps).abs() > f64::EPSILON
        || config.max_inflight != original_max_inflight
        || config.progress_timeout != original_progress_timeout
        || config.pipeline_time != original_pipeline_time
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
            "applied shared-host stable soak profile for deterministic long-run progress"
        );
    }
}

fn make_network_builder(config: &ChaosConfig, genesis: Vec<Vec<InstructionBox>>) -> NetworkBuilder {
    let mut genesis = genesis;
    let recovery_profile = recovery_profile_for(config);
    let mut builder = NetworkBuilder::new()
        .with_peers(config.peer_count)
        .with_base_seed(instructions::IZANAMI_BASE_SEED);
    let pipeline_time = config
        .pipeline_time
        .or_else(|| config.nexus.as_ref().map(|_| default_nexus_pipeline_time()));
    builder = match pipeline_time {
        Some(duration) => builder.with_pipeline_time(duration),
        None => builder.with_default_pipeline_time(),
    };
    if let Some(profile) = &config.nexus {
        builder = builder
            .with_data_availability_enabled(profile.da_enabled)
            .with_config_table(profile.config_layer.clone());
    }
    if config.nexus.is_some() {
        builder = builder.without_npos_genesis_bootstrap();
        builder = builder.with_genesis_post_topology_isi(
            instructions::npos_post_topology_instructions(config.peer_count),
        );
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
            Parameter::Sumeragi(SumeragiParameter::CollectorsK(IZANAMI_COLLECTORS_K)),
        )));
        injected.push(InstructionBox::from(SetParameter::new(
            Parameter::Sumeragi(SumeragiParameter::RedundantSendR(IZANAMI_REDUNDANT_SEND_R)),
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
                [
                    "sumeragi",
                    "advanced",
                    "pacemaker",
                    "pending_stall_grace_ms",
                ],
                pending_stall_grace_ms(npos_timing.block_ms),
            )
            .write(
                ["sumeragi", "advanced", "pacemaker", "da_fast_reschedule"],
                true,
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
                ["sumeragi", "recovery", "no_roster_fallback_views"],
                IZANAMI_RECOVERY_NO_ROSTER_FALLBACK_VIEWS,
            )
            .write(
                ["sumeragi", "recovery", "no_roster_refresh_retry_per_view"],
                IZANAMI_RECOVERY_NO_ROSTER_REFRESH_RETRY_PER_VIEW,
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

        let mut run_error = None;
        if let Err(err) = target_result {
            warn!(
                target: "izanami::progress",
                ?err,
                "target progress monitoring failed; stopping run"
            );
            run_control.stop();
            run_error = Some(err);
        }

        if self.config.target_blocks.is_some() {
            run_control.stop();
        }

        let shutdown_timeout = if run_error.is_some() {
            Duration::from_secs(IZANAMI_WORKER_FAILURE_SHUTDOWN_TIMEOUT_SECS)
        } else {
            Duration::from_secs(IZANAMI_WORKER_SHUTDOWN_TIMEOUT_SECS)
        };
        await_worker_shutdown_with_timeout(load_handles, "load", shutdown_timeout).await;
        await_worker_shutdown_with_timeout(faulty_handles, "fault", shutdown_timeout).await;
        self.network.shutdown().await;

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

fn tune_ingress_client(mut client: Client) -> Client {
    client.torii_request_timeout = Duration::from_millis(IZANAMI_INGRESS_REQUEST_TIMEOUT_MS);
    client.transaction_status_timeout = Duration::from_millis(IZANAMI_INGRESS_STATUS_TIMEOUT_MS);
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

fn sampled_peer_heights(peers: &[NetworkPeer]) -> Vec<u64> {
    peers
        .iter()
        .map(|peer| {
            peer.best_effort_block_height()
                .map(|height| height.total)
                .unwrap_or(0)
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

fn quorum_min_height_from_samples(mut heights: Vec<u64>) -> u64 {
    if heights.is_empty() {
        return 0;
    }
    heights.sort_unstable();
    let tolerated = tolerated_peer_failures(heights.len());
    let index = tolerated.min(heights.len().saturating_sub(1));
    heights[index]
}

fn strict_divergence_reference_height_from_samples(mut heights: Vec<u64>) -> u64 {
    if heights.is_empty() {
        return 0;
    }
    heights.sort_unstable();
    let tolerated = tolerated_peer_failures(heights.len());
    let index = heights
        .len()
        .saturating_sub(1 + tolerated.min(heights.len().saturating_sub(1)));
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

async fn wait_for_target_blocks(
    peers: &[NetworkPeer],
    target_blocks: u64,
    progress_interval: Duration,
    progress_timeout: Duration,
    run_control: &RunControl,
) -> Result<()> {
    let start = Instant::now();
    let mut progress = ProgressState::new(start);
    let mut divergence = HeightDivergenceState::new();
    let tolerated_failures = tolerated_peer_failures(peers.len());
    let strict_divergence_window =
        Duration::from_secs(IZANAMI_STRICT_HEIGHT_DIVERGENCE_MAX_WINDOW_SECS);
    loop {
        if run_control.should_stop() {
            return Err(eyre!("izanami run stopped before target blocks reached"));
        }
        let now = Instant::now();
        let heights = sampled_peer_heights(peers);
        let strict_min_height = heights.iter().copied().min().unwrap_or(0);
        let min_height = quorum_min_height_from_samples(heights.clone());
        let strict_reference_height =
            strict_divergence_reference_height_from_samples(heights.clone());
        let divergence_blocks = strict_reference_height.saturating_sub(strict_min_height);
        let lagging_peers = strict_divergence_lagging_peer_count(
            &heights,
            strict_reference_height,
            IZANAMI_STRICT_HEIGHT_DIVERGENCE_MAX_BLOCKS,
        );
        let strict_guard_active = lagging_peers > tolerated_failures;
        if now >= run_control.deadline() {
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
        if min_height >= target_blocks {
            info!(
                target: "izanami::progress",
                quorum_min_height = min_height,
                strict_min_height,
                tolerated_failures,
                target_blocks,
                elapsed = ?now.duration_since(start),
                "target block height reached"
            );
            return Ok(());
        }
        if progress.update(now, min_height) {
            info!(
                target: "izanami::progress",
                quorum_min_height = min_height,
                strict_min_height,
                tolerated_failures,
                target_blocks,
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

    if let Some((trigger_id, burn_amount)) = burn_target.clone() {
        let precheck = evaluate_burn_precheck(
            query_trigger_repetitions_with_failover(&ingress_pool, &signer, trigger_id.clone())
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

    if let Some((trigger_id, _mint_amount)) = mint_target.clone() {
        let precheck = evaluate_mint_precheck(
            query_trigger_repetitions_with_failover(&ingress_pool, &signer, trigger_id.clone())
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

    let ingress_pool_for_submit = Arc::clone(&ingress_pool);
    let signer_for_submit = signer.clone();
    let instructions_for_submit = instructions.clone();
    let submission_counter_for_submit = Arc::clone(&submission_counter);
    let succeeded = run_submission(
        plan_label,
        expect_success,
        Arc::clone(&metrics),
        move || {
            run_with_queue_timeout_retry(plan_label, || {
                ingress_pool_for_submit.run_with_failover(
                    "submit_all_blocking_with_metadata",
                    |peer| {
                        let client = tune_ingress_client(peer.client_for(
                            &signer_for_submit.id,
                            signer_for_submit.key_pair.private_key().clone(),
                        ));
                        let metadata = submission_metadata(submission_counter_for_submit.as_ref());
                        client
                            .submit_all_blocking_with_metadata(
                                instructions_for_submit.clone(),
                                metadata,
                            )
                            .map(|_| ())
                    },
                )
            })
        },
    )
    .await;
    drop(permit);
    if !succeeded {
        if let Some((trigger_id, _)) = mint_target {
            match query_trigger_repetitions_with_failover(
                &ingress_pool,
                &signer,
                trigger_id.clone(),
            )
            .await
            {
                Ok(Some(on_chain)) if on_chain > 0 => {
                    workload
                        .sync_trigger_repetitions(&trigger_id, Some(on_chain))
                        .await;
                }
                Ok(Some(_)) | Ok(None) => {
                    workload.sync_trigger_repetitions(&trigger_id, None).await;
                }
                _ => {}
            }
        }
    }
    workload.record_result(&plan, succeeded).await;
}

async fn query_trigger_repetitions_with_failover(
    ingress_pool: &Arc<IngressEndpointPool>,
    signer: &AccountRecord,
    trigger_id: TriggerId,
) -> Result<Option<u32>> {
    let ingress_pool = Arc::clone(ingress_pool);
    let signer = signer.clone();
    match spawn_blocking(move || {
        ingress_pool.run_with_failover("query_trigger_repetitions", |peer| {
            let client = tune_ingress_client(
                peer.client_for(&signer.id, signer.key_pair.private_key().clone()),
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

async fn run_submission<F>(
    plan_label: &'static str,
    expect_success: bool,
    metrics: Arc<Metrics>,
    blocking: F,
) -> bool
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
    if let Err(err) = result {
        warn!(
            target: "izanami::workload",
            ?err,
            plan = plan_label,
            "plan submission failed"
        );
    }
    succeeded
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
            config.max_inflight <= IZANAMI_SHARED_HOST_SOAK_MAX_INFLIGHT_CAP,
            "shared-host soak should clamp max inflight"
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
            config.tps, IZANAMI_SHARED_HOST_SOAK_TPS_CAP,
            "shared-host soak should pin canonical TPS for deterministic pilots"
        );
        assert_eq!(
            config.max_inflight, IZANAMI_SHARED_HOST_SOAK_MAX_INFLIGHT_CAP,
            "shared-host soak should pin canonical max_inflight baseline"
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
    fn ingress_failover_marks_queue_timeout_retryable() {
        let err = eyre!("transaction queued for too long");
        assert!(
            is_ingress_failover_retryable(&err),
            "queue timeout errors should trigger endpoint failover"
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
    fn endpoint_pool_queue_timeout_marks_unhealthy_on_first_failure() {
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
        let state = pool.endpoint_state(0);
        assert!(
            state.unhealthy_until.is_some(),
            "queue-timeout failure should mark endpoint unhealthy immediately"
        );
        assert_eq!(
            state.sticky_unhealthy_until, state.unhealthy_until,
            "queue-timeout unhealthy state should use sticky cooldown tracking"
        );
    }

    #[test]
    fn endpoint_pool_queue_timeout_sticky_cooldown_not_cleared_by_early_success() {
        let ingress_stats = Arc::new(IngressStats::default());
        let pool = EndpointHealthPool::new(
            vec!["http://127.0.0.1:41".to_string()],
            IngressEndpointPoolConfig {
                max_attempts: 1,
                unhealthy_failure_threshold: 3,
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
            state_after_early_success
                .unhealthy_until
                .is_some_and(|until| until == cooldown_until),
            "sticky queue-timeout cooldown should not clear before expiry"
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
    fn endpoint_pool_queue_timeout_cooldown_uses_status_timeout_floor() {
        let ingress_stats = Arc::new(IngressStats::default());
        let pool = EndpointHealthPool::new(
            vec!["http://127.0.0.1:51".to_string()],
            IngressEndpointPoolConfig {
                max_attempts: 1,
                unhealthy_failure_threshold: 3,
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
            unhealthy_until.saturating_duration_since(start)
                >= Duration::from_millis(IZANAMI_INGRESS_STATUS_TIMEOUT_MS),
            "queue-timeout cooldown should honor transaction status-timeout floor"
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
        assert!(!state.update(start, 0));
        assert!(!state.stalled(start + Duration::from_secs(2), Duration::from_secs(5)));
        assert!(state.update(start + Duration::from_secs(3), 2));
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
    fn quorum_min_height_ignores_single_straggler() {
        assert_eq!(quorum_min_height_from_samples(vec![]), 0);
        assert_eq!(
            quorum_min_height_from_samples(vec![246, 316, 316, 316]),
            316
        );
        assert_eq!(
            quorum_min_height_from_samples(vec![0, 0, 316, 316]),
            0,
            "two failed peers should not be hidden for a 4-peer run"
        );
        assert_eq!(quorum_min_height_from_samples(vec![9, 10, 11]), 9);
    }

    #[test]
    fn strict_divergence_reference_height_trims_tolerated_outliers() {
        assert_eq!(strict_divergence_reference_height_from_samples(vec![]), 0);
        assert_eq!(
            strict_divergence_reference_height_from_samples(vec![120, 149, 149, 149]),
            149,
            "single lagging outlier should not lower the strict divergence reference"
        );
        assert_eq!(
            strict_divergence_reference_height_from_samples(vec![120, 120, 120, 149]),
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
            strict_divergence_reference_height_from_samples(heights_one_outlier.clone());
        let lagging_one =
            strict_divergence_lagging_peer_count(&heights_one_outlier, strict_reference_one, 16);
        assert_eq!(lagging_one, 1);
        assert!(
            lagging_one <= tolerated_peer_failures(heights_one_outlier.len()),
            "a single outlier in a 4-peer run should stay within tolerated failures"
        );

        let heights_two_outliers = vec![120_u64, 121, 149, 149];
        let strict_reference_two =
            strict_divergence_reference_height_from_samples(heights_two_outliers.clone());
        let lagging_two =
            strict_divergence_lagging_peer_count(&heights_two_outliers, strict_reference_two, 16);
        assert_eq!(lagging_two, 2);
        assert!(
            lagging_two > tolerated_peer_failures(heights_two_outliers.len()),
            "strict divergence guard should activate once outliers exceed tolerated failures"
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
        let pending_stall_ms = pending_stall_grace_ms(npos_timing.block_ms);
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
            lookup(&[
                "sumeragi",
                "advanced",
                "pacemaker",
                "pending_stall_grace_ms"
            ]),
            Some(pending_stall_ms)
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
            lookup(&["sumeragi", "recovery", "no_roster_fallback_views"]),
            Some(IZANAMI_RECOVERY_NO_ROSTER_FALLBACK_VIEWS)
        );
        assert_eq!(
            lookup(&["sumeragi", "recovery", "no_roster_refresh_retry_per_view"]),
            Some(IZANAMI_RECOVERY_NO_ROSTER_REFRESH_RETRY_PER_VIEW)
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
