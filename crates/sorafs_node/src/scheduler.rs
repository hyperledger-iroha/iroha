//! Scheduler primitives for the embedded SoraFS node.
//!
//! This module implements the pin/fetch/PoR queue coordination layer used by
//! the Torii gateway and storage backend. It applies operator supplied limits
//! and emits lightweight telemetry snapshots for the metrics pipeline.

use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex, RwLock},
    thread,
    time::{Duration, Instant},
};

use crate::config::StorageConfig;

const LOCAL_PROVIDER_LABEL: &str = "local";
const FETCH_RATE_SMOOTHING_WEIGHT: u64 = 4;

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
        let max_pins = config.max_pins().max(1);
        let fetch_parallel = config.max_parallel_fetches().max(1);
        let por_interval = config.por_sample_interval_secs().max(1);

        scheduler.pin_queue_max_inflight = max_pins;
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

/// In-memory counters emitted by the storage telemetry layer.
#[derive(Debug, Default, Clone)]
pub struct StorageTelemetrySnapshot {
    /// Current number of bytes stored on disk.
    pub bytes_used: u64,
    /// Configured on-disk capacity limit in bytes.
    pub bytes_capacity: u64,
    /// Number of active pin operations waiting on disk IO.
    pub pin_queue_depth: usize,
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
    /// Gauge: number of manifests currently queued for pinning.
    pub const STORAGE_PIN_QUEUE_DEPTH: &str = "torii_sorafs_storage_pin_queue_depth";
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

    /// Run a pin operation under the configured queue limits.
    pub fn with_pin<F, R>(&self, work: F) -> R
    where
        F: FnOnce() -> R,
    {
        let mut scope = QueueScope::new_pin(&self.inner);
        let result = work();
        scope.finish();
        result
    }

    /// Run a fetch operation under the configured concurrency and byte budgets.
    pub fn run_fetch<F, T, E>(
        &self,
        requested_bytes: u64,
        provider: Option<&str>,
        work: F,
    ) -> Result<T, E>
    where
        F: FnOnce() -> Result<T, E>,
        T: AsRef<[u8]>,
    {
        let provider_key = provider.unwrap_or(LOCAL_PROVIDER_LABEL);
        let mut scope = FetchScope::new(&self.inner, provider_key.to_string(), requested_bytes);
        let start = Instant::now();
        let result = work();
        let elapsed = start.elapsed();
        match &result {
            Ok(buffer) => scope.complete(buffer.as_ref().len() as u64, elapsed),
            Err(_) => scope.complete(0, elapsed),
        }
        scope.finish();
        result
    }

    /// Run a PoR sampling operation respecting the concurrency limit.
    pub fn with_por<F, R>(&self, work: F) -> R
    where
        F: FnOnce() -> R,
    {
        let mut scope = QueueScope::new_por(&self.inner);
        let result = work();
        scope.finish();
        result
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
    fn new(config: StorageSchedulerConfig) -> Self {
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
            .expect("scheduler state poisoned")
            .config
            .clone()
    }

    fn refresh_pin_metrics(&self) {
        let stats = self.pin.stats();
        let mut sched = self.schedulers.write().expect("scheduler state poisoned");
        sched.telemetry.pin_queue_depth = stats.inflight + stats.waiting;
        sched.utilisation.pin_queue_utilisation_bps =
            utilisation_ratio(stats.inflight, sched.config.pin_queue_max_inflight);
    }

    fn refresh_fetch_metrics(&self) {
        let stats = self.fetch.stats();
        let mut sched = self.schedulers.write().expect("scheduler state poisoned");
        sched.telemetry.fetch_inflight = stats.inflight;
        sched.utilisation.fetch_utilisation_bps =
            utilisation_ratio(stats.inflight, sched.config.fetch_concurrency);
    }

    fn refresh_por_metrics(&self) {
        let stats = self.por.stats();
        let mut sched = self.schedulers.write().expect("scheduler state poisoned");
        sched.telemetry.por_inflight = stats.inflight;
        sched.utilisation.por_utilisation_bps =
            utilisation_ratio(stats.inflight, sched.config.por_concurrency);
    }

    fn record_fetch_sample(&self, bytes: u64, elapsed: Duration) {
        let sample_rate = if bytes == 0 || elapsed.is_zero() {
            0
        } else {
            (bytes as f64 / elapsed.as_secs_f64()).round() as u64
        };

        let mut sched = self.schedulers.write().expect("scheduler state poisoned");
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
        let mut sched = self.schedulers.write().expect("scheduler state poisoned");
        sched.telemetry.por_samples_success =
            sched.telemetry.por_samples_success.saturating_add(success);
        sched.telemetry.por_samples_failed =
            sched.telemetry.por_samples_failed.saturating_add(failed);
    }

    fn update_storage_bytes(&self, bytes_used: u64, bytes_capacity: u64) {
        let mut sched = self.schedulers.write().expect("scheduler state poisoned");
        sched.telemetry.bytes_used = bytes_used;
        sched.telemetry.bytes_capacity = bytes_capacity;
    }

    fn telemetry_snapshot(&self) -> StorageTelemetrySnapshot {
        self.schedulers
            .read()
            .expect("scheduler state poisoned")
            .telemetry
            .clone()
    }

    fn utilisation_snapshot(&self) -> SchedulerUtilisation {
        self.schedulers
            .read()
            .expect("scheduler state poisoned")
            .utilisation
            .clone()
    }
}

#[derive(Debug)]
struct QueueLimiter {
    limit: usize,
    state: Mutex<QueueState>,
    condvar: Condvar,
}

impl QueueLimiter {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            state: Mutex::new(QueueState::default()),
            condvar: Condvar::new(),
        }
    }

    fn acquire(&self) -> QueueGuard<'_> {
        let mut guard = self.state.lock().expect("queue state poisoned");
        loop {
            if self.limit == 0 || guard.inflight < self.limit {
                guard.inflight = guard.inflight.saturating_add(1);
                return QueueGuard { limiter: self };
            }
            guard.waiting = guard.waiting.saturating_add(1);
            guard = self.condvar.wait(guard).expect("queue state poisoned");
            guard.waiting = guard.waiting.saturating_sub(1);
        }
    }

    fn stats(&self) -> QueueStats {
        let guard = self.state.lock().expect("queue state poisoned");
        QueueStats {
            inflight: guard.inflight,
            waiting: guard.waiting,
        }
    }
}

#[derive(Debug, Default)]
struct QueueState {
    inflight: usize,
    waiting: usize,
}

#[derive(Debug)]
struct QueueGuard<'a> {
    limiter: &'a QueueLimiter,
}

impl Drop for QueueGuard<'_> {
    fn drop(&mut self) {
        let mut guard = self.limiter.state.lock().expect("queue state poisoned");
        guard.inflight = guard.inflight.saturating_sub(1);
        if self.limiter.limit != 0 {
            self.limiter.condvar.notify_one();
        }
    }
}

#[derive(Debug, Default)]
struct QueueStats {
    inflight: usize,
    waiting: usize,
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
    fn new_pin(runtime: &'a RuntimeInner) -> Self {
        Self::new(runtime, QueueKind::Pin)
    }

    fn new_por(runtime: &'a RuntimeInner) -> Self {
        Self::new(runtime, QueueKind::Por)
    }

    fn new(runtime: &'a RuntimeInner, kind: QueueKind) -> Self {
        let limiter = match kind {
            QueueKind::Pin => &runtime.pin,
            QueueKind::Por => &runtime.por,
        };
        let guard = limiter.acquire();
        let scope = Self {
            runtime,
            kind,
            guard: Some(guard),
        };
        scope.refresh();
        scope
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
    condvar: Condvar,
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
            global_limit,
            per_provider_limit,
            state: Mutex::new(FetchState::default()),
            condvar: Condvar::new(),
            global_rate: (global_rate > 0).then(|| Arc::new(RateLimiter::new(global_rate))),
            provider_rate: provider_rate
                .filter(|limit| *limit > 0)
                .map(RateLimiterMap::new),
        }
    }

    fn acquire(&self, provider_key: &str, requested_bytes: u64) -> FetchPermit<'_> {
        let mut guard = self.state.lock().expect("fetch state poisoned");
        loop {
            let provider_inflight = guard
                .per_provider_inflight
                .get(provider_key)
                .copied()
                .unwrap_or(0);

            let global_ok = self.global_limit == 0 || guard.inflight < self.global_limit;
            let provider_ok =
                self.per_provider_limit == 0 || provider_inflight < self.per_provider_limit;

            if global_ok && provider_ok {
                guard.inflight = guard.inflight.saturating_add(1);
                *guard
                    .per_provider_inflight
                    .entry(provider_key.to_string())
                    .or_default() += 1;
                break;
            }

            guard.waiting = guard.waiting.saturating_add(1);
            guard = self.condvar.wait(guard).expect("fetch state poisoned");
            guard.waiting = guard.waiting.saturating_sub(1);
        }
        drop(guard);

        if let Some(rate) = &self.global_rate {
            rate.acquire(requested_bytes);
        }
        if let Some(map) = &self.provider_rate {
            map.acquire(provider_key, requested_bytes);
        }

        FetchPermit {
            limiter: self,
            provider_key: provider_key.to_string(),
            requested_bytes,
        }
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

        let mut guard = self.state.lock().expect("fetch state poisoned");
        guard.inflight = guard.inflight.saturating_sub(1);
        if let Some(entry) = guard.per_provider_inflight.get_mut(provider_key) {
            *entry = entry.saturating_sub(1);
            if *entry == 0 {
                guard.per_provider_inflight.remove(provider_key);
            }
        }
        if self.global_limit != 0 || self.per_provider_limit != 0 {
            self.condvar.notify_one();
        }
    }

    fn stats(&self) -> FetchStats {
        let guard = self.state.lock().expect("fetch state poisoned");
        FetchStats {
            inflight: guard.inflight,
        }
    }
}

#[derive(Debug, Default)]
struct FetchState {
    inflight: usize,
    waiting: usize,
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
    fn new(runtime: &'a RuntimeInner, provider_key: String, requested_bytes: u64) -> Self {
        let permit = runtime.fetch.acquire(&provider_key, requested_bytes);
        let scope = Self {
            runtime,
            permit: Some(permit),
            actual_bytes: 0,
            duration: Duration::ZERO,
        };
        scope.runtime.refresh_fetch_metrics();
        scope
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
                tokens: capacity_per_sec as f64,
                last_refill: Instant::now(),
            }),
        }
    }

    fn acquire(&self, amount: u64) {
        if self.capacity_per_sec == 0 || amount == 0 {
            return;
        }
        let mut remaining = amount;
        while remaining > 0 {
            let chunk = remaining.min(self.capacity_per_sec);
            self.acquire_chunk(chunk);
            remaining = remaining.saturating_sub(chunk);
        }
    }

    fn acquire_chunk(&self, amount: u64) {
        let mut state = self.state.lock().expect("rate limiter state poisoned");
        loop {
            state.refill(self.capacity_per_sec);
            if state.tokens >= amount as f64 {
                state.tokens -= amount as f64;
                return;
            }
            let deficit = amount as f64 - state.tokens;
            let wait_secs = deficit / self.capacity_per_sec as f64;
            let wait = Duration::from_secs_f64(wait_secs.max(0.001));
            drop(state);
            thread::sleep(wait);
            state = self.state.lock().expect("rate limiter state poisoned");
        }
    }

    fn refund(&self, amount: u64) {
        if self.capacity_per_sec == 0 {
            return;
        }
        let mut state = self.state.lock().expect("rate limiter state poisoned");
        state.refill(self.capacity_per_sec);
        state.tokens = (state.tokens + amount as f64).min(self.capacity_per_sec as f64);
    }
}

#[derive(Debug)]
struct RateState {
    tokens: f64,
    last_refill: Instant,
}

impl RateState {
    fn refill(&mut self, capacity_per_sec: u64) {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.last_refill);
        if elapsed.is_zero() {
            return;
        }
        let replenished = elapsed.as_secs_f64() * capacity_per_sec as f64;
        self.tokens = (self.tokens + replenished).min(capacity_per_sec as f64);
        self.last_refill = now;
    }
}

#[derive(Debug)]
struct RateLimiterMap {
    limit_per_sec: u64,
    map: Mutex<HashMap<String, Arc<RateLimiter>>>,
}

impl RateLimiterMap {
    fn new(limit_per_sec: u64) -> Self {
        Self {
            limit_per_sec,
            map: Mutex::new(HashMap::new()),
        }
    }

    fn acquire(&self, key: &str, amount: u64) {
        let limiter = {
            let mut map = self.map.lock().expect("rate limiter map poisoned");
            map.entry(key.to_string())
                .or_insert_with(|| Arc::new(RateLimiter::new(self.limit_per_sec)))
                .clone()
        };
        limiter.acquire(amount);
    }

    fn refund(&self, key: &str, amount: u64) {
        let limiter = {
            let map = self.map.lock().expect("rate limiter map poisoned");
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
    use std::sync::{
        Arc, Barrier,
        atomic::{AtomicBool, Ordering},
        mpsc,
    };

    use super::*;

    #[test]
    fn pin_queue_respects_inflight_limit() {
        let config = StorageSchedulerConfig {
            pin_queue_max_inflight: 1,
            ..StorageSchedulerConfig::default()
        };
        let runtime = StorageSchedulersRuntime::new(config);

        let barrier = Arc::new(Barrier::new(2));
        let done = Arc::new(AtomicBool::new(false));

        let runtime_clone = runtime.clone();
        let barrier_clone = barrier.clone();
        let done_clone = done.clone();
        let handle = thread::spawn(move || {
            runtime_clone.with_pin(|| {
                barrier_clone.wait();
                thread::sleep(Duration::from_millis(60));
                done_clone.store(true, Ordering::SeqCst);
            });
        });

        barrier.wait();
        let start = Instant::now();
        runtime.with_pin(|| {});
        let elapsed = start.elapsed();

        assert!(done.load(Ordering::SeqCst));
        assert!(
            elapsed >= Duration::from_millis(50),
            "expected pin queue to block for at least 50ms, got {elapsed:?}"
        );
        handle.join().expect("pin worker thread panicked");
    }

    #[test]
    fn fetch_rate_budget_enforced() {
        let config = StorageSchedulerConfig {
            fetch_concurrency: 1,
            fetch_concurrency_per_provider: 1,
            fetch_global_bytes_per_sec: 1_000_000, // 1 MiB/s
            ..StorageSchedulerConfig::default()
        };
        let runtime = StorageSchedulersRuntime::new(config);

        runtime
            .run_fetch(1_000_000, None, || -> Result<Vec<u8>, ()> {
                Ok(vec![0_u8; 1_000_000])
            })
            .expect("initial fetch should succeed");

        let start = Instant::now();
        runtime
            .run_fetch(10_000, None, || -> Result<Vec<u8>, ()> {
                Ok(vec![0_u8; 10_000])
            })
            .expect("second fetch should succeed");
        let elapsed = start.elapsed();

        assert!(
            elapsed >= Duration::from_millis(8),
            "expected rate limiter to delay second fetch, got {elapsed:?}"
        );
    }

    #[test]
    fn por_concurrency_limit_is_respected() {
        let config = StorageSchedulerConfig {
            por_concurrency: 1,
            ..StorageSchedulerConfig::default()
        };
        let runtime = StorageSchedulersRuntime::new(config);

        let barrier = Arc::new(Barrier::new(2));
        let runtime_clone = runtime.clone();
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            runtime_clone.with_por(|| {
                barrier_clone.wait();
                thread::sleep(Duration::from_millis(40));
            });
        });

        barrier.wait();
        let start = Instant::now();
        runtime.with_por(|| {});
        let elapsed = start.elapsed();

        assert!(
            elapsed >= Duration::from_millis(35),
            "expected PoR limiter to block, got {elapsed:?}"
        );
        handle.join().expect("PoR worker thread panicked");
    }

    #[test]
    fn fetch_rate_budget_handles_large_request() {
        let limiter = RateLimiter::new(100);
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            limiter.acquire(150);
            let _ = tx.send(());
        });

        rx.recv_timeout(Duration::from_secs(2))
            .expect("large rate limit request should complete");
    }
}
