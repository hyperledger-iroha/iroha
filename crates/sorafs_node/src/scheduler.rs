//! Scheduler primitives for the embedded SoraFS node.
//!
//! This module implements the pin/fetch/PoR queue coordination layer used by
//! the Torii gateway and storage backend. It applies operator supplied limits
//! and emits lightweight telemetry snapshots for the metrics pipeline.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

use thiserror::Error;

use crate::config::StorageConfig;

const LOCAL_PROVIDER_LABEL: &str = "local";
const FETCH_RATE_SMOOTHING_WEIGHT: u64 = 4;
const MAX_PROVIDER_KEY_BYTES: usize = 128;
const MAX_TRACKED_FETCH_PROVIDERS: usize = 4_096;

/// Configuration governing the pin/fetch/PoR schedulers.
#[derive(Debug, Clone)]
pub struct StorageSchedulerConfig {
    /// Maximum number of manifests the pin queue accepts before applying back-pressure.
    pub pin_queue_max_inflight: usize,
    /// Maximum number of concurrent fetch tasks across all providers.
    pub fetch_concurrency: usize,
    /// Per-provider fetch concurrency limit (1 disables multi-source parallelism per provider).
    pub fetch_concurrency_per_provider: usize,
    /// Global bytes-per-second budget applied before provider specific budgets are considered.
    pub fetch_global_bytes_per_sec: u64,
    /// Optional bytes-per-second budget applied to each provider individually.
    pub fetch_provider_bytes_per_sec: Option<u64>,
    /// Maximum number of simultaneous PoR sampling tasks.
    pub por_concurrency: usize,
    /// Target interval for opportunistic PoR sampling when no governance request is pending.
    pub por_idle_interval: Duration,
}

impl StorageSchedulerConfig {
    /// Derive scheduler settings from the storage configuration.
    #[must_use]
    pub fn from_storage_config(config: &StorageConfig) -> Self {
        let mut scheduler = StorageSchedulerConfig::default();
        let fetch_parallel = config.max_parallel_fetches().max(1);
        let por_interval = config.por_sample_interval_secs().max(1);

        // `max_pins` bounds durable manifest cardinality, not concurrent disk
        // writers. Reuse the explicit I/O concurrency budget so a high pin
        // inventory limit cannot create thousands of simultaneous ingests.
        scheduler.pin_queue_max_inflight = fetch_parallel;
        scheduler.fetch_concurrency = fetch_parallel;
        scheduler.fetch_concurrency_per_provider = fetch_parallel;
        scheduler.por_concurrency = fetch_parallel;
        scheduler.por_idle_interval = Duration::from_secs(por_interval);
        scheduler
    }
}

impl Default for StorageSchedulerConfig {
    fn default() -> Self {
        Self {
            pin_queue_max_inflight: 64,
            fetch_concurrency: 16,
            fetch_concurrency_per_provider: 4,
            fetch_global_bytes_per_sec: 256 * 1024 * 1024,
            fetch_provider_bytes_per_sec: None,
            por_concurrency: 2,
            por_idle_interval: Duration::from_secs(60),
        }
    }
}

/// Scope of a fetch byte-budget refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchRateScope {
    /// Aggregate budget shared by every provider.
    Global,
    /// Budget assigned to one provider identity.
    Provider,
}

impl std::fmt::Display for FetchRateScope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Global => "global",
            Self::Provider => "provider",
        })
    }
}

/// Admission failures returned without parking an unbounded request thread.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerAdmissionError {
    /// Every pin worker slot is occupied.
    #[error("SoraFS pin scheduler is saturated at {limit} in-flight operations")]
    PinSaturated {
        /// Configured in-flight ceiling.
        limit: usize,
    },
    /// Every PoR worker slot is occupied.
    #[error("SoraFS PoR scheduler is saturated at {limit} in-flight operations")]
    PorSaturated {
        /// Configured in-flight ceiling.
        limit: usize,
    },
    /// The global fetch concurrency ceiling is occupied.
    #[error("SoraFS fetch scheduler is saturated at {limit} global in-flight operations")]
    FetchSaturated {
        /// Configured global in-flight ceiling.
        limit: usize,
    },
    /// One provider has exhausted its fetch concurrency budget.
    #[error(
        "SoraFS provider fetch scheduler is saturated at {limit} in-flight operations for this provider"
    )]
    ProviderFetchSaturated {
        /// Configured per-provider in-flight ceiling.
        limit: usize,
    },
    /// A provider label is empty, oversized, or contains unsafe bytes.
    #[error("SoraFS fetch provider label is not canonical")]
    InvalidProviderLabel,
    /// The bounded per-provider rate-limiter registry has no free slot.
    #[error("SoraFS provider rate-limiter registry is full at {limit} entries")]
    ProviderRegistryFull {
        /// Maximum tracked provider count.
        limit: usize,
    },
    /// A single request exceeds the configured one-second burst budget.
    #[error(
        "SoraFS {scope} fetch request of {requested_bytes} bytes exceeds the {burst_bytes}-byte burst budget"
    )]
    RequestExceedsBurst {
        /// Budget that rejected the request.
        scope: FetchRateScope,
        /// Requested byte reservation.
        requested_bytes: u64,
        /// Maximum one-request burst.
        burst_bytes: u64,
    },
    /// The current token bucket cannot admit the request immediately.
    #[error("SoraFS {scope} fetch byte budget is exhausted; retry after {retry_after:?}")]
    RateLimited {
        /// Budget that rejected the request.
        scope: FetchRateScope,
        /// Minimum delay before the same reservation can be retried.
        retry_after: Duration,
    },
    /// A scheduler lock was poisoned while processing another operation.
    #[error("SoraFS scheduler state is unavailable: {component}")]
    StateUnavailable {
        /// Scheduler component that failed closed.
        component: &'static str,
    },
}

/// In-memory counters emitted by the storage telemetry layer.
#[derive(Debug, Default, Clone)]
pub struct StorageTelemetrySnapshot {
    /// Current number of bytes stored on disk.
    pub bytes_used: u64,
    /// Configured on-disk capacity limit in bytes.
    pub bytes_capacity: u64,
    /// Number of finalized-ledger provider ingests holding storage-write admission.
    pub provider_ingest_inflight: usize,
    /// Number of fetch tasks currently streaming chunk data.
    pub fetch_inflight: usize,
    /// Aggregate bytes-per-second observed across fetch workers.
    pub fetch_bytes_per_sec: u64,
    /// Number of PoR sampling tasks currently in progress.
    pub por_inflight: usize,
    /// Number of PoR samples completed successfully during the current telemetry window.
    pub por_samples_success: u64,
    /// Number of PoR samples that failed during the current telemetry window.
    pub por_samples_failed: u64,
}

/// Metric label names surfaced via Prometheus.
pub mod metrics {
    /// Gauge: total bytes stored on disk by the worker.
    pub const STORAGE_BYTES_USED: &str = "torii_sorafs_storage_bytes_used";
    /// Gauge: configured storage capacity ceiling.
    pub const STORAGE_BYTES_CAPACITY: &str = "torii_sorafs_storage_bytes_capacity";
    /// Gauge: finalized-ledger provider ingests currently holding storage-write admission.
    pub const PROVIDER_INGEST_INFLIGHT: &str = "sorafs_provider_ingest_inflight";
    /// Gauge: number of active fetch workers streaming chunk data.
    pub const STORAGE_FETCH_INFLIGHT: &str = "torii_sorafs_storage_fetch_inflight";
    /// Gauge: instantaneous bytes-per-second served by fetch workers.
    pub const STORAGE_FETCH_BYTES_PER_SEC: &str = "torii_sorafs_storage_fetch_bytes_per_sec";
    /// Gauge: number of PoR sampling workers currently active.
    pub const STORAGE_POR_INFLIGHT: &str = "torii_sorafs_storage_por_inflight";
    /// Counter: PoR samples that completed successfully.
    pub const STORAGE_POR_SAMPLES_SUCCESS_TOTAL: &str =
        "torii_sorafs_storage_por_samples_success_total";
    /// Counter: PoR samples that failed.
    pub const STORAGE_POR_SAMPLES_FAILED_TOTAL: &str =
        "torii_sorafs_storage_por_samples_failed_total";
}

/// Summary of scheduler utilisation used by the shared telemetry pipeline.
#[derive(Debug, Default, Clone)]
pub struct SchedulerUtilisation {
    /// Running average of fetch worker utilisation expressed as percentage (0-10000 == basis points).
    pub fetch_utilisation_bps: u32,
    /// Running average of pin queue occupancy expressed as percentage of `pin_queue_max_inflight`.
    pub pin_queue_utilisation_bps: u32,
    /// Running average of PoR worker utilisation expressed as percentage (basis points).
    pub por_utilisation_bps: u32,
}

/// Aggregated runtime state for the pin/fetch/PoR schedulers.
#[derive(Debug)]
pub struct StorageSchedulers {
    /// Configured thresholds and limits.
    pub config: StorageSchedulerConfig,
    /// Current telemetry snapshot published periodically.
    pub telemetry: StorageTelemetrySnapshot,
    /// Rolling utilisation metrics.
    pub utilisation: SchedulerUtilisation,
}

impl StorageSchedulers {
    /// Construct schedulers with the supplied configuration.
    #[must_use]
    pub fn new(config: StorageSchedulerConfig) -> Self {
        Self {
            config,
            telemetry: StorageTelemetrySnapshot::default(),
            utilisation: SchedulerUtilisation::default(),
        }
    }
}

/// Runtime facade applying concurrency and rate limits for storage operations.
#[derive(Debug, Clone)]
pub struct StorageSchedulersRuntime {
    inner: Arc<RuntimeInner>,
}

impl StorageSchedulersRuntime {
    /// Construct the runtime from the supplied configuration.
    #[must_use]
    pub fn new(config: StorageSchedulerConfig) -> Self {
        Self {
            inner: Arc::new(RuntimeInner::new(config)),
        }
    }

    /// Returns the underlying scheduler configuration.
    #[must_use]
    pub fn config(&self) -> StorageSchedulerConfig {
        self.inner.config()
    }

    /// Attempt a pin operation without waiting for an occupied worker slot.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerAdmissionError::PinSaturated`] when the configured
    /// in-flight ceiling is already occupied, or
    /// [`SchedulerAdmissionError::StateUnavailable`] when admission state is poisoned.
    pub fn try_with_pin<F, R>(&self, work: F) -> Result<R, SchedulerAdmissionError>
    where
        F: FnOnce() -> R,
    {
        let mut scope = QueueScope::try_new_pin(&self.inner)?;
        let result = work();
        scope.finish();
        Ok(result)
    }

    /// Attempt a fetch operation without parking on concurrency or rate limits.
    ///
    /// Scheduler refusal is returned by the outer result. The inner result is
    /// the storage operation's own success or failure.
    ///
    /// # Errors
    ///
    /// Returns a [`SchedulerAdmissionError`] when concurrency, provider-map,
    /// or byte-budget admission cannot be granted immediately, including when
    /// a poisoned admission lock forces the scheduler to fail closed.
    pub fn try_run_fetch<F, T, E>(
        &self,
        requested_bytes: u64,
        provider: Option<&str>,
        work: F,
    ) -> Result<Result<T, E>, SchedulerAdmissionError>
    where
        F: FnOnce() -> Result<T, E>,
        T: AsRef<[u8]>,
    {
        self.try_run_fetch_with_failure_accounting(requested_bytes, provider, false, work)
    }

    /// Attempt a fetch and conservatively charge the requested bytes when verified work fails.
    ///
    /// Use this boundary when the inner operation can consume bytes before discovering an
    /// integrity failure but cannot safely expose a partial buffer. Charging the complete
    /// admitted request prevents repeated corrupt reads from refunding and bypassing the local
    /// byte-rate budget.
    ///
    /// # Errors
    ///
    /// Returns the same admission errors as [`Self::try_run_fetch`]. Inner work errors are
    /// preserved, while their accounting uses `requested_bytes` rather than zero.
    pub fn try_run_fetch_charging_failures<F, T, E>(
        &self,
        requested_bytes: u64,
        provider: Option<&str>,
        work: F,
    ) -> Result<Result<T, E>, SchedulerAdmissionError>
    where
        F: FnOnce() -> Result<T, E>,
        T: AsRef<[u8]>,
    {
        self.try_run_fetch_with_failure_accounting(requested_bytes, provider, true, work)
    }

    fn try_run_fetch_with_failure_accounting<F, T, E>(
        &self,
        requested_bytes: u64,
        provider: Option<&str>,
        charge_failure: bool,
        work: F,
    ) -> Result<Result<T, E>, SchedulerAdmissionError>
    where
        F: FnOnce() -> Result<T, E>,
        T: AsRef<[u8]>,
    {
        let provider_key = provider.unwrap_or(LOCAL_PROVIDER_LABEL);
        let mut scope =
            FetchScope::try_new(&self.inner, provider_key.to_string(), requested_bytes)?;
        let start = Instant::now();
        let result = work();
        let elapsed = start.elapsed();
        match &result {
            Ok(buffer) => scope.complete(buffer.as_ref().len() as u64, elapsed),
            Err(_) if charge_failure => scope.complete(requested_bytes, elapsed),
            Err(_) => scope.complete(0, elapsed),
        }
        scope.finish();
        Ok(result)
    }

    /// Attempt a PoR operation without waiting for an occupied worker slot.
    ///
    /// # Errors
    ///
    /// Returns [`SchedulerAdmissionError::PorSaturated`] when the configured
    /// in-flight ceiling is already occupied, or
    /// [`SchedulerAdmissionError::StateUnavailable`] when admission state is poisoned.
    pub fn try_with_por<F, R>(&self, work: F) -> Result<R, SchedulerAdmissionError>
    where
        F: FnOnce() -> R,
    {
        let mut scope = QueueScope::try_new_por(&self.inner)?;
        let result = work();
        scope.finish();
        Ok(result)
    }

    /// Update the storage byte usage snapshot.
    pub fn update_storage_bytes(&self, bytes_used: u64, bytes_capacity: u64) {
        self.inner.update_storage_bytes(bytes_used, bytes_capacity);
    }

    /// Record aggregated PoR sampling results.
    pub fn record_por_samples(&self, success: u64, failed: u64) {
        self.inner.record_por_samples(success, failed);
    }

    /// Retrieve the current telemetry snapshot.
    #[must_use]
    pub fn telemetry_snapshot(&self) -> StorageTelemetrySnapshot {
        self.inner.telemetry_snapshot()
    }

    /// Retrieve the current utilisation snapshot.
    #[must_use]
    pub fn utilisation_snapshot(&self) -> SchedulerUtilisation {
        self.inner.utilisation_snapshot()
    }
}

#[derive(Debug)]
struct RuntimeInner {
    schedulers: RwLock<StorageSchedulers>,
    pin: QueueLimiter,
    fetch: FetchLimiter,
    por: QueueLimiter,
}

impl RuntimeInner {
    fn new(mut config: StorageSchedulerConfig) -> Self {
        // A zero concurrency value must never turn a production limiter into
        // an unbounded one. Configuration conversion already supplies positive
        // values; this clamp keeps programmatic construction fail-safe too.
        config.pin_queue_max_inflight = config.pin_queue_max_inflight.max(1);
        config.fetch_concurrency = config.fetch_concurrency.max(1);
        config.fetch_concurrency_per_provider = config.fetch_concurrency_per_provider.max(1);
        config.por_concurrency = config.por_concurrency.max(1);
        Self {
            pin: QueueLimiter::new(config.pin_queue_max_inflight),
            fetch: FetchLimiter::new(
                config.fetch_concurrency,
                config.fetch_concurrency_per_provider,
                config.fetch_global_bytes_per_sec,
                config.fetch_provider_bytes_per_sec,
            ),
            por: QueueLimiter::new(config.por_concurrency),
            schedulers: RwLock::new(StorageSchedulers::new(config)),
        }
    }

    fn config(&self) -> StorageSchedulerConfig {
        self.schedulers
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .config
            .clone()
    }

    fn refresh_pin_metrics(&self) {
        let stats = self.pin.stats();
        let mut sched = self
            .schedulers
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sched.telemetry.provider_ingest_inflight = stats.inflight;
        sched.utilisation.pin_queue_utilisation_bps =
            utilisation_ratio(stats.inflight, sched.config.pin_queue_max_inflight);
    }

    fn refresh_fetch_metrics(&self) {
        let stats = self.fetch.stats();
        let mut sched = self
            .schedulers
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sched.telemetry.fetch_inflight = stats.inflight;
        sched.utilisation.fetch_utilisation_bps =
            utilisation_ratio(stats.inflight, sched.config.fetch_concurrency);
    }

    fn refresh_por_metrics(&self) {
        let stats = self.por.stats();
        let mut sched = self
            .schedulers
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sched.telemetry.por_inflight = stats.inflight;
        sched.utilisation.por_utilisation_bps =
            utilisation_ratio(stats.inflight, sched.config.por_concurrency);
    }

    fn record_fetch_sample(&self, bytes: u64, elapsed: Duration) {
        let sample_rate = if bytes == 0 || elapsed.is_zero() {
            0
        } else {
            let rate = u128::from(bytes)
                .saturating_mul(1_000_000_000)
                .checked_div(elapsed.as_nanos())
                .unwrap_or(0);
            u64::try_from(rate).unwrap_or(u64::MAX)
        };

        let mut sched = self
            .schedulers
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if sample_rate == 0 {
            sched.telemetry.fetch_bytes_per_sec = sched.telemetry.fetch_bytes_per_sec
                * (FETCH_RATE_SMOOTHING_WEIGHT - 1)
                / FETCH_RATE_SMOOTHING_WEIGHT;
            return;
        }

        let current = sched.telemetry.fetch_bytes_per_sec as u128;
        let smoothed = if current == 0 {
            sample_rate as u128
        } else {
            ((current * (FETCH_RATE_SMOOTHING_WEIGHT - 1) as u128) + sample_rate as u128)
                / FETCH_RATE_SMOOTHING_WEIGHT as u128
        };
        sched.telemetry.fetch_bytes_per_sec = smoothed as u64;
    }

    fn record_por_samples(&self, success: u64, failed: u64) {
        let mut sched = self
            .schedulers
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sched.telemetry.por_samples_success =
            sched.telemetry.por_samples_success.saturating_add(success);
        sched.telemetry.por_samples_failed =
            sched.telemetry.por_samples_failed.saturating_add(failed);
    }

    fn update_storage_bytes(&self, bytes_used: u64, bytes_capacity: u64) {
        let mut sched = self
            .schedulers
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sched.telemetry.bytes_used = bytes_used;
        sched.telemetry.bytes_capacity = bytes_capacity;
    }

    fn telemetry_snapshot(&self) -> StorageTelemetrySnapshot {
        self.schedulers
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .telemetry
            .clone()
    }

    fn utilisation_snapshot(&self) -> SchedulerUtilisation {
        self.schedulers
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .utilisation
            .clone()
    }
}

#[derive(Debug)]
struct QueueLimiter {
    limit: usize,
    state: Mutex<QueueState>,
}

impl QueueLimiter {
    fn new(limit: usize) -> Self {
        Self {
            limit: limit.max(1),
            state: Mutex::new(QueueState::default()),
        }
    }

    fn try_acquire(&self) -> Result<Option<QueueGuard<'_>>, SchedulerStatePoisoned> {
        let mut guard = self.state.lock().map_err(|_| SchedulerStatePoisoned)?;
        if guard.inflight >= self.limit {
            return Ok(None);
        }
        guard.inflight = guard.inflight.saturating_add(1);
        Ok(Some(QueueGuard { limiter: self }))
    }

    fn stats(&self) -> QueueStats {
        let guard = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        QueueStats {
            inflight: guard.inflight,
        }
    }
}

#[derive(Debug, Default)]
struct QueueState {
    inflight: usize,
}

#[derive(Debug)]
struct QueueGuard<'a> {
    limiter: &'a QueueLimiter,
}

impl Drop for QueueGuard<'_> {
    fn drop(&mut self) {
        let mut guard = self
            .limiter
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.inflight = guard.inflight.saturating_sub(1);
    }
}

#[derive(Debug, Clone, Copy)]
struct SchedulerStatePoisoned;

#[derive(Debug, Default)]
struct QueueStats {
    inflight: usize,
}

#[derive(Debug, Clone, Copy)]
enum QueueKind {
    Pin,
    Por,
}

#[derive(Debug)]
struct QueueScope<'a> {
    runtime: &'a RuntimeInner,
    kind: QueueKind,
    guard: Option<QueueGuard<'a>>,
}

impl<'a> QueueScope<'a> {
    fn try_new_pin(runtime: &'a RuntimeInner) -> Result<Self, SchedulerAdmissionError> {
        Self::try_new(runtime, QueueKind::Pin)
    }

    fn try_new_por(runtime: &'a RuntimeInner) -> Result<Self, SchedulerAdmissionError> {
        Self::try_new(runtime, QueueKind::Por)
    }

    fn try_new(
        runtime: &'a RuntimeInner,
        kind: QueueKind,
    ) -> Result<Self, SchedulerAdmissionError> {
        let limiter = match kind {
            QueueKind::Pin => &runtime.pin,
            QueueKind::Por => &runtime.por,
        };
        let guard = limiter
            .try_acquire()
            .map_err(|_| SchedulerAdmissionError::StateUnavailable {
                component: match kind {
                    QueueKind::Pin => "pin queue",
                    QueueKind::Por => "PoR queue",
                },
            })?
            .ok_or(match kind {
                QueueKind::Pin => SchedulerAdmissionError::PinSaturated {
                    limit: limiter.limit,
                },
                QueueKind::Por => SchedulerAdmissionError::PorSaturated {
                    limit: limiter.limit,
                },
            })?;
        let scope = Self {
            runtime,
            kind,
            guard: Some(guard),
        };
        scope.refresh();
        Ok(scope)
    }

    fn finish(&mut self) {
        if let Some(guard) = self.guard.take() {
            drop(guard);
            self.refresh();
        }
    }

    fn refresh(&self) {
        match self.kind {
            QueueKind::Pin => self.runtime.refresh_pin_metrics(),
            QueueKind::Por => self.runtime.refresh_por_metrics(),
        }
    }
}

impl Drop for QueueScope<'_> {
    fn drop(&mut self) {
        self.finish();
    }
}

#[derive(Debug)]
struct FetchLimiter {
    global_limit: usize,
    per_provider_limit: usize,
    state: Mutex<FetchState>,
    global_rate: Option<Arc<RateLimiter>>,
    provider_rate: Option<RateLimiterMap>,
}

impl FetchLimiter {
    fn new(
        global_limit: usize,
        per_provider_limit: usize,
        global_rate: u64,
        provider_rate: Option<u64>,
    ) -> Self {
        Self {
            global_limit: global_limit.max(1),
            per_provider_limit: per_provider_limit.max(1),
            state: Mutex::new(FetchState::default()),
            global_rate: (global_rate > 0).then(|| Arc::new(RateLimiter::new(global_rate))),
            provider_rate: provider_rate
                .filter(|limit| *limit > 0)
                .map(RateLimiterMap::new),
        }
    }

    fn try_acquire(
        &self,
        provider_key: &str,
        requested_bytes: u64,
    ) -> Result<FetchPermit<'_>, SchedulerAdmissionError> {
        if !canonical_provider_label(provider_key) {
            return Err(SchedulerAdmissionError::InvalidProviderLabel);
        }

        {
            let mut guard =
                self.state
                    .lock()
                    .map_err(|_| SchedulerAdmissionError::StateUnavailable {
                        component: "fetch concurrency",
                    })?;
            if guard.inflight >= self.global_limit {
                return Err(SchedulerAdmissionError::FetchSaturated {
                    limit: self.global_limit,
                });
            }
            let provider_inflight = guard
                .per_provider_inflight
                .get(provider_key)
                .copied()
                .unwrap_or(0);
            if provider_inflight >= self.per_provider_limit {
                return Err(SchedulerAdmissionError::ProviderFetchSaturated {
                    limit: self.per_provider_limit,
                });
            }
            guard.inflight = guard.inflight.saturating_add(1);
            *guard
                .per_provider_inflight
                .entry(provider_key.to_owned())
                .or_default() += 1;
        }

        if let Some(rate) = &self.global_rate
            && let Err(err) = rate.try_acquire(requested_bytes, FetchRateScope::Global)
        {
            self.release_concurrency(provider_key);
            return Err(err);
        }
        if let Some(map) = &self.provider_rate
            && let Err(err) = map.try_acquire(provider_key, requested_bytes)
        {
            if let Some(rate) = &self.global_rate {
                rate.refund(requested_bytes);
            }
            self.release_concurrency(provider_key);
            return Err(err);
        }

        Ok(FetchPermit {
            limiter: self,
            provider_key: provider_key.to_owned(),
            requested_bytes,
        })
    }

    fn release(&self, provider_key: &str, requested_bytes: u64, actual_bytes: u64) {
        if requested_bytes > actual_bytes {
            let refund = requested_bytes - actual_bytes;
            if let Some(rate) = &self.global_rate {
                rate.refund(refund);
            }
            if let Some(map) = &self.provider_rate {
                map.refund(provider_key, refund);
            }
        }

        self.release_concurrency(provider_key);
    }

    fn release_concurrency(&self, provider_key: &str) {
        let mut guard = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.inflight = guard.inflight.saturating_sub(1);
        if let Some(entry) = guard.per_provider_inflight.get_mut(provider_key) {
            *entry = entry.saturating_sub(1);
            if *entry == 0 {
                guard.per_provider_inflight.remove(provider_key);
            }
        }
    }

    fn stats(&self) -> FetchStats {
        let guard = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        FetchStats {
            inflight: guard.inflight,
        }
    }
}

fn canonical_provider_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PROVIDER_KEY_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

#[derive(Debug, Default)]
struct FetchState {
    inflight: usize,
    per_provider_inflight: HashMap<String, usize>,
}

#[derive(Debug)]
struct FetchPermit<'a> {
    limiter: &'a FetchLimiter,
    provider_key: String,
    requested_bytes: u64,
}

#[derive(Debug, Default)]
struct FetchStats {
    inflight: usize,
}

#[derive(Debug)]
struct FetchScope<'a> {
    runtime: &'a RuntimeInner,
    permit: Option<FetchPermit<'a>>,
    actual_bytes: u64,
    duration: Duration,
}

impl<'a> FetchScope<'a> {
    fn try_new(
        runtime: &'a RuntimeInner,
        provider_key: String,
        requested_bytes: u64,
    ) -> Result<Self, SchedulerAdmissionError> {
        let permit = runtime.fetch.try_acquire(&provider_key, requested_bytes)?;
        let scope = Self {
            runtime,
            permit: Some(permit),
            actual_bytes: 0,
            duration: Duration::ZERO,
        };
        scope.runtime.refresh_fetch_metrics();
        Ok(scope)
    }

    fn complete(&mut self, actual_bytes: u64, duration: Duration) {
        self.actual_bytes = actual_bytes;
        self.duration = duration;
    }

    fn finish(&mut self) {
        if let Some(permit) = self.permit.take() {
            permit.limiter.release(
                &permit.provider_key,
                permit.requested_bytes,
                self.actual_bytes,
            );
            self.runtime.refresh_fetch_metrics();
            self.runtime
                .record_fetch_sample(self.actual_bytes, self.duration);
        }
    }
}

impl Drop for FetchScope<'_> {
    fn drop(&mut self) {
        self.finish();
    }
}

#[derive(Debug)]
struct RateLimiter {
    capacity_per_sec: u64,
    state: Mutex<RateState>,
}

impl RateLimiter {
    fn new(capacity_per_sec: u64) -> Self {
        Self {
            capacity_per_sec,
            state: Mutex::new(RateState {
                tokens: capacity_per_sec,
                last_refill: Instant::now(),
                fractional_token_nanos: 0,
            }),
        }
    }

    fn try_acquire(
        &self,
        amount: u64,
        scope: FetchRateScope,
    ) -> Result<(), SchedulerAdmissionError> {
        if self.capacity_per_sec == 0 || amount == 0 {
            return Ok(());
        }
        if amount > self.capacity_per_sec {
            return Err(SchedulerAdmissionError::RequestExceedsBurst {
                scope,
                requested_bytes: amount,
                burst_bytes: self.capacity_per_sec,
            });
        }
        let mut state =
            self.state
                .lock()
                .map_err(|_| SchedulerAdmissionError::StateUnavailable {
                    component: match scope {
                        FetchRateScope::Global => "global fetch rate limiter",
                        FetchRateScope::Provider => "provider fetch rate limiter",
                    },
                })?;
        state.refill(self.capacity_per_sec);
        if state.tokens < amount {
            return Err(SchedulerAdmissionError::RateLimited {
                scope,
                retry_after: retry_after_for_deficit(
                    amount.saturating_sub(state.tokens),
                    self.capacity_per_sec,
                ),
            });
        }
        state.tokens -= amount;
        Ok(())
    }

    fn refund(&self, amount: u64) {
        if self.capacity_per_sec == 0 {
            return;
        }
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.refill(self.capacity_per_sec);
        state.tokens = state
            .tokens
            .saturating_add(amount)
            .min(self.capacity_per_sec);
    }
}

#[derive(Debug)]
struct RateState {
    tokens: u64,
    last_refill: Instant,
    fractional_token_nanos: u128,
}

impl RateState {
    fn refill(&mut self, capacity_per_sec: u64) {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.last_refill);
        if elapsed.is_zero() {
            return;
        }
        let scaled = elapsed
            .as_nanos()
            .saturating_mul(u128::from(capacity_per_sec))
            .saturating_add(self.fractional_token_nanos);
        let replenished = scaled / 1_000_000_000;
        self.fractional_token_nanos = scaled % 1_000_000_000;
        self.tokens = self
            .tokens
            .saturating_add(u64::try_from(replenished).unwrap_or(u64::MAX))
            .min(capacity_per_sec);
        if self.tokens == capacity_per_sec {
            self.fractional_token_nanos = 0;
        }
        self.last_refill = now;
    }
}

fn retry_after_for_deficit(deficit: u64, capacity_per_sec: u64) -> Duration {
    if deficit == 0 || capacity_per_sec == 0 {
        return Duration::ZERO;
    }
    let numerator = u128::from(deficit).saturating_mul(1_000_000_000);
    let denominator = u128::from(capacity_per_sec);
    let nanos = numerator
        .saturating_add(denominator.saturating_sub(1))
        .checked_div(denominator)
        .unwrap_or(u128::MAX)
        .max(1);
    Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX))
}

#[derive(Debug)]
struct RateLimiterMap {
    limit_per_sec: u64,
    max_entries: usize,
    map: Mutex<HashMap<String, Arc<RateLimiter>>>,
}

impl RateLimiterMap {
    fn new(limit_per_sec: u64) -> Self {
        Self {
            limit_per_sec,
            max_entries: MAX_TRACKED_FETCH_PROVIDERS,
            map: Mutex::new(HashMap::new()),
        }
    }

    #[cfg(test)]
    fn with_max_entries(limit_per_sec: u64, max_entries: usize) -> Self {
        Self {
            limit_per_sec,
            max_entries,
            map: Mutex::new(HashMap::new()),
        }
    }

    fn try_acquire(&self, key: &str, amount: u64) -> Result<(), SchedulerAdmissionError> {
        if !canonical_provider_label(key) {
            return Err(SchedulerAdmissionError::InvalidProviderLabel);
        }
        let limiter = {
            let mut map =
                self.map
                    .lock()
                    .map_err(|_| SchedulerAdmissionError::StateUnavailable {
                        component: "provider rate registry",
                    })?;
            if let Some(limiter) = map.get(key) {
                Arc::clone(limiter)
            } else {
                if map.len() >= self.max_entries {
                    return Err(SchedulerAdmissionError::ProviderRegistryFull {
                        limit: self.max_entries,
                    });
                }
                let limiter = Arc::new(RateLimiter::new(self.limit_per_sec));
                map.insert(key.to_owned(), Arc::clone(&limiter));
                limiter
            }
        };
        limiter.try_acquire(amount, FetchRateScope::Provider)
    }

    fn refund(&self, key: &str, amount: u64) {
        let limiter = {
            let map = self
                .map
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            map.get(key).cloned()
        };
        if let Some(limiter) = limiter {
            limiter.refund(amount);
        }
    }
}

fn utilisation_ratio(inflight: usize, limit: usize) -> u32 {
    if limit == 0 {
        return 0;
    }
    let capped = inflight.min(limit);
    ((capped * 10_000) / limit) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fail_fast_pin_and_por_refuse_without_waiting() {
        let runtime = StorageSchedulersRuntime::new(StorageSchedulerConfig {
            pin_queue_max_inflight: 1,
            por_concurrency: 1,
            ..StorageSchedulerConfig::default()
        });

        let pin_guard = runtime.inner.pin.try_acquire().expect("acquire pin slot");
        let start = Instant::now();
        assert_eq!(
            runtime.try_with_pin(|| ()),
            Err(SchedulerAdmissionError::PinSaturated { limit: 1 })
        );
        assert!(start.elapsed() < Duration::from_millis(50));
        drop(pin_guard);

        let por_guard = runtime.inner.por.try_acquire().expect("acquire PoR slot");
        let start = Instant::now();
        assert_eq!(
            runtime.try_with_por(|| ()),
            Err(SchedulerAdmissionError::PorSaturated { limit: 1 })
        );
        assert!(start.elapsed() < Duration::from_millis(50));
        drop(por_guard);
    }

    #[test]
    fn fail_fast_fetch_refuses_global_and_provider_saturation() {
        let runtime = StorageSchedulersRuntime::new(StorageSchedulerConfig {
            fetch_concurrency: 1,
            fetch_concurrency_per_provider: 1,
            fetch_global_bytes_per_sec: 0,
            ..StorageSchedulerConfig::default()
        });
        let scope = FetchScope::try_new(&runtime.inner, "provider-a".to_owned(), 0)
            .expect("acquire global fetch slot");

        let error = runtime
            .try_run_fetch(0, Some("provider-b"), || -> Result<Vec<u8>, ()> {
                Ok(Vec::new())
            })
            .expect_err("occupied global slot must refuse immediately");
        assert_eq!(error, SchedulerAdmissionError::FetchSaturated { limit: 1 });
        drop(scope);

        let runtime = StorageSchedulersRuntime::new(StorageSchedulerConfig {
            fetch_concurrency: 2,
            fetch_concurrency_per_provider: 1,
            fetch_global_bytes_per_sec: 0,
            ..StorageSchedulerConfig::default()
        });
        let scope = FetchScope::try_new(&runtime.inner, "provider-a".to_owned(), 0)
            .expect("acquire provider fetch slot");
        let error = runtime
            .try_run_fetch(0, Some("provider-a"), || -> Result<Vec<u8>, ()> {
                Ok(Vec::new())
            })
            .expect_err("occupied provider slot must refuse immediately");
        assert_eq!(
            error,
            SchedulerAdmissionError::ProviderFetchSaturated { limit: 1 }
        );
        drop(scope);
    }

    #[test]
    fn fail_fast_rate_limiter_is_integer_and_burst_bounded() {
        let limiter = RateLimiter::new(100);
        limiter
            .try_acquire(100, FetchRateScope::Global)
            .expect("initial burst must be available");
        let error = limiter
            .try_acquire(1, FetchRateScope::Global)
            .expect_err("depleted bucket must refuse");
        assert!(matches!(
            error,
            SchedulerAdmissionError::RateLimited {
                scope: FetchRateScope::Global,
                retry_after,
            } if retry_after > Duration::ZERO
        ));
        assert_eq!(
            limiter.try_acquire(101, FetchRateScope::Global),
            Err(SchedulerAdmissionError::RequestExceedsBurst {
                scope: FetchRateScope::Global,
                requested_bytes: 101,
                burst_bytes: 100,
            })
        );
    }

    #[test]
    fn provider_rate_registry_and_labels_are_bounded() {
        let map = RateLimiterMap::with_max_entries(100, 1);
        map.try_acquire("provider-a", 0)
            .expect("first provider fits registry");
        assert_eq!(
            map.try_acquire("provider-b", 0),
            Err(SchedulerAdmissionError::ProviderRegistryFull { limit: 1 })
        );
        assert_eq!(
            map.try_acquire(" provider-a", 0),
            Err(SchedulerAdmissionError::InvalidProviderLabel)
        );
        assert_eq!(
            map.try_acquire(&"a".repeat(MAX_PROVIDER_KEY_BYTES + 1), 0),
            Err(SchedulerAdmissionError::InvalidProviderLabel)
        );
    }

    #[test]
    fn failed_fetch_work_releases_fail_fast_permit() {
        let runtime = StorageSchedulersRuntime::new(StorageSchedulerConfig {
            fetch_concurrency: 1,
            fetch_concurrency_per_provider: 1,
            fetch_global_bytes_per_sec: 0,
            ..StorageSchedulerConfig::default()
        });
        let work_error = runtime
            .try_run_fetch(
                0,
                Some("provider-a"),
                || -> Result<Vec<u8>, &'static str> { Err("injected") },
            )
            .expect("scheduler must admit first request")
            .expect_err("work failure must be preserved");
        assert_eq!(work_error, "injected");

        runtime
            .try_run_fetch(0, Some("provider-a"), || -> Result<Vec<u8>, ()> {
                Ok(Vec::new())
            })
            .expect("permit must be released after work failure")
            .expect("second work succeeds");
    }

    #[test]
    fn verified_fetch_failure_charges_the_admitted_byte_budget() {
        let runtime = StorageSchedulersRuntime::new(StorageSchedulerConfig {
            fetch_global_bytes_per_sec: 32,
            ..StorageSchedulerConfig::default()
        });
        let work_error = runtime
            .try_run_fetch_charging_failures(
                32,
                Some("provider-a"),
                || -> Result<Vec<u8>, &'static str> { Err("integrity failure") },
            )
            .expect("scheduler admits verified read")
            .expect_err("verified read failure is preserved");
        assert_eq!(work_error, "integrity failure");

        let global_rate = runtime
            .inner
            .fetch
            .global_rate
            .as_ref()
            .expect("configured global byte budget");
        assert_eq!(
            global_rate
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .tokens,
            0,
            "failed verified work must not refund the admitted bytes"
        );
    }

    #[test]
    fn zero_concurrency_configuration_clamps_to_one() {
        let runtime = StorageSchedulersRuntime::new(StorageSchedulerConfig {
            pin_queue_max_inflight: 0,
            fetch_concurrency: 0,
            fetch_concurrency_per_provider: 0,
            por_concurrency: 0,
            ..StorageSchedulerConfig::default()
        });
        let config = runtime.config();
        assert_eq!(config.pin_queue_max_inflight, 1);
        assert_eq!(config.fetch_concurrency, 1);
        assert_eq!(config.fetch_concurrency_per_provider, 1);
        assert_eq!(config.por_concurrency, 1);
    }

    #[test]
    fn storage_pin_inventory_does_not_expand_ingest_concurrency() {
        let storage = StorageConfig::builder()
            .max_pins(100_000)
            .max_parallel_fetches(3)
            .build();
        let config = StorageSchedulerConfig::from_storage_config(&storage);
        assert_eq!(config.pin_queue_max_inflight, 3);
        assert_eq!(config.fetch_concurrency, 3);
        assert_eq!(config.por_concurrency, 3);
    }

    #[test]
    fn poisoned_admission_state_fails_closed_without_repanicking() {
        let runtime = StorageSchedulersRuntime::new(StorageSchedulerConfig {
            fetch_global_bytes_per_sec: 0,
            ..StorageSchedulerConfig::default()
        });
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = runtime.inner.pin.state.lock().expect("lock pin state");
            panic!("poison pin state");
        }));
        assert!(poisoned.is_err());
        assert_eq!(
            runtime.try_with_pin(|| ()),
            Err(SchedulerAdmissionError::StateUnavailable {
                component: "pin queue"
            })
        );

        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = runtime.inner.fetch.state.lock().expect("lock fetch state");
            panic!("poison fetch state");
        }));
        assert!(poisoned.is_err());
        assert_eq!(
            runtime
                .try_run_fetch(0, Some("provider-a"), || -> Result<Vec<u8>, ()> {
                    Ok(Vec::new())
                })
                .expect_err("poisoned fetch admission must fail"),
            SchedulerAdmissionError::StateUnavailable {
                component: "fetch concurrency"
            }
        );
    }

    #[test]
    fn poisoned_rate_limit_state_fails_closed_and_refunds_do_not_panic() {
        let limiter = RateLimiter::new(10);
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = limiter.state.lock().expect("lock rate state");
            panic!("poison rate state");
        }));
        assert!(poisoned.is_err());
        assert_eq!(
            limiter.try_acquire(1, FetchRateScope::Global),
            Err(SchedulerAdmissionError::StateUnavailable {
                component: "global fetch rate limiter"
            })
        );
        limiter.refund(1);

        let map = RateLimiterMap::with_max_entries(10, 1);
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = map.map.lock().expect("lock provider map");
            panic!("poison provider map");
        }));
        assert!(poisoned.is_err());
        assert_eq!(
            map.try_acquire("provider-a", 1),
            Err(SchedulerAdmissionError::StateUnavailable {
                component: "provider rate registry"
            })
        );
        map.refund("provider-a", 1);
    }

    #[test]
    fn poisoned_telemetry_state_recovers_without_process_panic() {
        let runtime = StorageSchedulersRuntime::new(StorageSchedulerConfig::default());
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = runtime
                .inner
                .schedulers
                .write()
                .expect("lock scheduler telemetry");
            panic!("poison scheduler telemetry");
        }));
        assert!(poisoned.is_err());

        runtime.update_storage_bytes(7, 11);
        let snapshot = runtime.telemetry_snapshot();
        assert_eq!(snapshot.bytes_used, 7);
        assert_eq!(snapshot.bytes_capacity, 11);
        assert!(runtime.config().fetch_concurrency > 0);
    }
}
