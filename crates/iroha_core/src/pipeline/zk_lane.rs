//! ZK lane: capture IVM formal traces, verify in the background, and report.
//!
//! This module provides a lightweight, non-forking background worker that can
//! receive formal execution traces from IVM runs, verify constraints and
//! Merkle-authenticated logs, and report outcomes. It does not mutate WSV and
//! is intended purely for diagnostics/telemetry.
//!
//! Integration points:
//! - Overlay builder: after running IVM to collect queued ISIs, capture the
//!   formal trace and submit a job if ZK is enabled in config.
//! - Node startup: call `start()` once to initialize the worker.

#![deny(missing_docs)]

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    num::NonZeroU64,
    sync::mpsc as std_mpsc,
    sync::{Arc, OnceLock},
    thread,
    time::{Duration, Instant},
};

#[cfg(test)]
use iroha_crypto::HashOf;
use iroha_crypto::{Hash, streaming::TransportCapabilityResolutionSnapshot};
use ivm::zk::{Constraint, MemEvent, RegEvent, RegisterState, StepEntry};
use norito::streaming::CapabilityFlags;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

/// Task carrying a single IVM execution's formal trace and metadata.
#[derive(Clone)]
pub struct ZkTask {
    /// Transaction hash if available (zero when not applicable).
    pub tx_hash: Option<Hash>,
    /// Code hash of the executed bytecode.
    pub code_hash: [u8; 32],
    /// Full program bytes executed to produce this trace.
    pub program: Arc<Vec<u8>>,
    /// Optional block header associated with this trace (for warnings/events).
    /// If not provided, the ZK lane will emit a warning with a minimal header
    /// carrying height=1.
    pub header: Option<iroha_data_model::block::BlockHeader>,
    /// Expanded register trace.
    pub trace: Vec<RegisterState>,
    /// Logged constraints encountered during execution.
    pub constraints: Vec<Constraint>,
    /// Memory access log with Merkle proofs.
    pub mem_log: Vec<MemEvent>,
    /// Register access log with Merkle proofs.
    pub reg_log: Vec<RegEvent>,
    /// Per-step Merkle roots of registers and memory.
    pub step_log: Vec<StepEntry>,
    /// Transport capabilities negotiated for the session (if available).
    pub transport_capabilities: Option<TransportCapabilityResolutionSnapshot>,
    /// Negotiated feature flags advertised by the peer (if available).
    pub negotiated_capabilities: Option<CapabilityFlags>,
}

impl ZkTask {
    /// Compute a stable digest of the task contents for idempotence/logging.
    pub fn digest(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(self.code_hash);
        if let Some(tx) = &self.tx_hash {
            h.update(tx.as_ref());
        }
        h.update((self.program.len() as u64).to_le_bytes());
        if !self.program.is_empty() {
            let sample = if self.program.len() <= 64 {
                &self.program
            } else {
                &self.program[..64]
            };
            h.update(sample);
        }
        // Keep digest bounded and deterministic: include sizes and first/last PCs.
        let cycles = self.trace.len() as u64;
        h.update(cycles.to_le_bytes());
        let constraints = self.constraints.len() as u64;
        h.update(constraints.to_le_bytes());
        let mem = self.mem_log.len() as u64;
        h.update(mem.to_le_bytes());
        let regs = self.reg_log.len() as u64;
        h.update(regs.to_le_bytes());
        if let Some(first) = self.step_log.first() {
            h.update(first.pc.to_le_bytes());
            h.update(first.reg_root.as_ref().as_ref());
            h.update(first.mem_root.as_ref().as_ref());
        }
        if let Some(last) = self.step_log.last() {
            h.update(last.pc.to_le_bytes());
            h.update(last.reg_root.as_ref().as_ref());
            h.update(last.mem_root.as_ref().as_ref());
        }
        if let Some(caps) = &self.transport_capabilities {
            h.update(caps.hpke_suite.suite_id().to_le_bytes());
            h.update([u8::from(caps.use_datagram)]);
            h.update(caps.max_segment_datagram_size.to_le_bytes());
            h.update(caps.fec_feedback_interval_ms.to_le_bytes());
            h.update([caps.privacy_bucket_granularity as u8]);
        }
        if let Some(flags) = self.negotiated_capabilities {
            h.update(flags.bits().to_le_bytes());
        }
        h.finalize().into()
    }
}

/// Result of background verification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZkOutcome {
    /// All constraints and Merkle paths verified.
    Verified,
    /// Verification failed (constraint or path mismatch).
    Rejected,
}

/// Handle for submitting ZK tasks to the background worker.
#[derive(Clone)]
pub struct ZkLaneHandle {
    tx: mpsc::Sender<ZkTask>,
    enqueue_wait: Duration,
    enqueue_poll: Duration,
    retry_ring: Arc<RetryRing>,
}

impl ZkLaneHandle {
    /// Submit a task; returns false only if admission fails after bounded wait/retry handling.
    pub fn submit(&self, task: ZkTask) -> bool {
        let digest = task.digest();
        if let Some(cache) = RESULT_CACHE.get() {
            match cache.admit(digest, Instant::now()) {
                CacheAdmission::Cached => return true,
                CacheAdmission::Inflight => return true,
                CacheAdmission::Accepted => {}
            }
        }
        let mut task = task;
        let started = Instant::now();
        loop {
            match self.tx.try_send(task) {
                Ok(()) => {
                    if started.elapsed() > Duration::ZERO {
                        record_enqueue_wait();
                    }
                    return true;
                }
                Err(mpsc::error::TrySendError::Full(returned)) => {
                    task = returned;
                    if started.elapsed() >= self.enqueue_wait {
                        record_enqueue_timeout();
                        if task.tx_hash.is_some() {
                            let accepted = enqueue_retry_task(
                                &self.retry_ring,
                                task,
                                "ingress_retry_ring_full",
                            );
                            if !accepted {
                                clear_inflight_digest(digest);
                            }
                            return accepted;
                        }
                        record_lane_drop("ingress_enqueue_timeout");
                        clear_inflight_digest(digest);
                        return false;
                    }
                    if self.enqueue_poll > Duration::ZERO {
                        thread::sleep(self.enqueue_poll);
                    }
                }
                Err(mpsc::error::TrySendError::Closed(_)) => {
                    record_lane_drop("ingress_channel_closed");
                    clear_inflight_digest(digest);
                    return false;
                }
            }
        }
    }
}

static GLOBAL_SENDER: OnceLock<ZkLaneHandle> = OnceLock::new();
static EVENTS: OnceLock<crate::EventsSender> = OnceLock::new();
static RESULT_CACHE: OnceLock<Arc<ZkResultCache>> = OnceLock::new();

const ZK_RESULT_CACHE_CAP: usize = 8_192;
const ZK_RESULT_CACHE_TTL_MS: u64 = 300_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CacheAdmission {
    Accepted,
    Inflight,
    Cached,
}

#[derive(Debug, Clone, Copy)]
struct ZkResultEntry {
    recorded_at: Instant,
}

#[derive(Debug)]
struct ZkResultCache {
    cap: usize,
    ttl: Duration,
    inner: std::sync::Mutex<ZkResultCacheInner>,
}

#[derive(Debug, Default)]
struct ZkResultCacheInner {
    entries: BTreeMap<[u8; 32], ZkResultEntry>,
    order: VecDeque<[u8; 32]>,
    inflight: BTreeSet<[u8; 32]>,
}

impl ZkResultCache {
    fn new(cap: usize, ttl: Duration) -> Self {
        Self {
            cap,
            ttl,
            inner: std::sync::Mutex::new(ZkResultCacheInner::default()),
        }
    }

    fn admit(&self, key: [u8; 32], now: Instant) -> CacheAdmission {
        let mut inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(err) => {
                iroha_logger::warn!(?err, "zk_lane: result cache lock poisoned during admit");
                return CacheAdmission::Accepted;
            }
        };
        inner.gc(now, self.ttl);
        if let Some(entry) = inner.entries.get(&key) {
            if self.ttl == Duration::ZERO
                || now.saturating_duration_since(entry.recorded_at) <= self.ttl
            {
                inner.touch(key);
                return CacheAdmission::Cached;
            }
            inner.remove_entry(&key);
        }
        if inner.inflight.contains(&key) {
            return CacheAdmission::Inflight;
        }
        inner.inflight.insert(key);
        CacheAdmission::Accepted
    }

    fn record(&self, key: [u8; 32], now: Instant) {
        let mut inner = match self.inner.lock() {
            Ok(guard) => guard,
            Err(err) => {
                iroha_logger::warn!(?err, "zk_lane: result cache lock poisoned during record");
                return;
            }
        };
        inner.inflight.remove(&key);
        if self.cap == 0 {
            return;
        }
        inner
            .entries
            .insert(key, ZkResultEntry { recorded_at: now });
        inner.touch(key);
        inner.gc(now, self.ttl);
        while inner.entries.len() > self.cap {
            let Some(oldest) = inner.order.pop_front() else {
                break;
            };
            inner.entries.remove(&oldest);
        }
    }

    fn clear_inflight(&self, key: [u8; 32]) {
        match self.inner.lock() {
            Ok(mut guard) => {
                guard.inflight.remove(&key);
            }
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    "zk_lane: result cache lock poisoned while clearing inflight"
                );
            }
        }
    }
}

impl ZkResultCacheInner {
    fn touch(&mut self, key: [u8; 32]) {
        self.order.retain(|entry| entry != &key);
        self.order.push_back(key);
    }

    fn remove_entry(&mut self, key: &[u8; 32]) {
        self.entries.remove(key);
        self.order.retain(|entry| entry != key);
    }

    fn gc(&mut self, now: Instant, ttl: Duration) {
        if ttl == Duration::ZERO {
            return;
        }
        let mut stale = Vec::new();
        for (key, entry) in &self.entries {
            if now.saturating_duration_since(entry.recorded_at) > ttl {
                stale.push(*key);
            }
        }
        for key in stale {
            self.remove_entry(&key);
        }
    }
}

fn clear_inflight_digest(digest: [u8; 32]) {
    if let Some(cache) = RESULT_CACHE.get() {
        cache.clear_inflight(digest);
    }
}

struct RetryTask {
    task: ZkTask,
    attempts: u32,
}

#[derive(Debug, Clone, Copy, Default)]
struct RetryDrainStats {
    replayed: u64,
    exhausted: u64,
    depth: usize,
}

struct RetryRing {
    cap: usize,
    max_attempts: u32,
    inner: std::sync::Mutex<VecDeque<RetryTask>>,
}

impl RetryRing {
    fn new(cap: usize, max_attempts: u32) -> Self {
        Self {
            cap: cap.max(1),
            max_attempts: max_attempts.max(1),
            inner: std::sync::Mutex::new(VecDeque::new()),
        }
    }

    fn enqueue(&self, task: ZkTask) -> Result<usize, ZkTask> {
        match self.inner.lock() {
            Ok(mut queue) => {
                if queue.len() >= self.cap {
                    return Err(task);
                }
                queue.push_back(RetryTask { task, attempts: 0 });
                Ok(queue.len())
            }
            Err(err) => {
                iroha_logger::warn!(?err, "zk_lane: retry ring lock poisoned during enqueue");
                Err(task)
            }
        }
    }

    fn drain_into_pending(&self, pending: &mut Vec<ZkTask>, pending_cap: usize) -> RetryDrainStats {
        let mut stats = RetryDrainStats::default();
        let mut queue = match self.inner.lock() {
            Ok(queue) => queue,
            Err(err) => {
                iroha_logger::warn!(?err, "zk_lane: retry ring lock poisoned during drain");
                return stats;
            }
        };

        if pending.len() >= pending_cap {
            for entry in queue.iter_mut() {
                entry.attempts = entry.attempts.saturating_add(1);
            }
            let before = queue.len();
            queue.retain(|entry| {
                let keep = entry.attempts < self.max_attempts;
                if !keep {
                    clear_inflight_digest(entry.task.digest());
                }
                keep
            });
            stats.exhausted = (before.saturating_sub(queue.len())) as u64;
            stats.depth = queue.len();
            return stats;
        }

        while pending.len() < pending_cap {
            let Some(entry) = queue.pop_front() else {
                break;
            };
            pending.push(entry.task);
            stats.replayed = stats.replayed.saturating_add(1);
        }
        stats.depth = queue.len();
        stats
    }

    fn clear(&self) -> usize {
        match self.inner.lock() {
            Ok(mut queue) => {
                let dropped = queue.len();
                for entry in queue.iter() {
                    clear_inflight_digest(entry.task.digest());
                }
                queue.clear();
                dropped
            }
            Err(err) => {
                iroha_logger::warn!(?err, "zk_lane: retry ring lock poisoned during clear");
                0
            }
        }
    }

    fn depth(&self) -> usize {
        match self.inner.lock() {
            Ok(queue) => queue.len(),
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    "zk_lane: retry ring lock poisoned while reading depth"
                );
                0
            }
        }
    }
}

fn with_metrics(mut apply: impl FnMut(&iroha_telemetry::metrics::Metrics)) {
    if let Some(metrics) = iroha_telemetry::metrics::global() {
        apply(metrics.as_ref());
    }
}

fn record_lane_drop(reason: &str) {
    with_metrics(|metrics| {
        metrics
            .zk_lane_drop_total
            .with_label_values(&[reason])
            .inc();
    });
}

fn record_lane_drop_by(reason: &str, count: u64) {
    if count == 0 {
        return;
    }
    with_metrics(|metrics| {
        metrics
            .zk_lane_drop_total
            .with_label_values(&[reason])
            .inc_by(count);
    });
}

fn record_enqueue_wait() {
    with_metrics(|metrics| {
        metrics.zk_lane_enqueue_wait_total.inc();
    });
}

fn record_enqueue_timeout() {
    with_metrics(|metrics| {
        metrics.zk_lane_enqueue_timeout_total.inc();
    });
}

fn set_pending_depth(depth: usize) {
    with_metrics(|metrics| {
        metrics
            .zk_lane_pending_depth
            .set(u64::try_from(depth).unwrap_or(u64::MAX));
    });
}

fn set_retry_ring_depth(depth: usize) {
    with_metrics(|metrics| {
        metrics
            .zk_lane_retry_ring_depth
            .set(u64::try_from(depth).unwrap_or(u64::MAX));
    });
}

fn enqueue_retry_task(retry_ring: &RetryRing, task: ZkTask, full_reason: &str) -> bool {
    match retry_ring.enqueue(task) {
        Ok(depth) => {
            with_metrics(|metrics| {
                metrics.zk_lane_retry_enqueued_total.inc();
            });
            set_retry_ring_depth(depth);
            true
        }
        Err(_) => {
            record_lane_drop(full_reason);
            false
        }
    }
}

struct WorkerPool {
    work_txs: Vec<std_mpsc::SyncSender<ZkTask>>,
    join_handles: Vec<thread::JoinHandle<()>>,
    next_worker: usize,
}

impl WorkerPool {
    fn spawn(worker_threads: usize, work_queue_cap: usize) -> Self {
        let mut work_txs = Vec::with_capacity(worker_threads);
        let mut join_handles = Vec::with_capacity(worker_threads);
        for idx in 0..worker_threads {
            let (work_tx, work_rx) = std_mpsc::sync_channel::<ZkTask>(work_queue_cap);
            work_txs.push(work_tx);
            let name = format!("zk-lane-verify-{idx}");
            let join_handle = thread::Builder::new()
                .name(name)
                .spawn(move || {
                    while let Ok(job) = work_rx.recv() {
                        process_job(job);
                    }
                })
                .expect("failed to spawn zk lane verifier thread");
            join_handles.push(join_handle);
        }
        Self {
            work_txs,
            join_handles,
            next_worker: 0,
        }
    }

    fn dispatch(&mut self, mut job: ZkTask) -> Result<(), ZkTask> {
        if self.work_txs.is_empty() {
            return Err(job);
        }
        let total = self.work_txs.len();
        let mut disconnected = Vec::new();
        for _ in 0..total {
            let idx = self.next_worker % total;
            self.next_worker = self.next_worker.saturating_add(1);
            let work_tx = &self.work_txs[idx];
            match work_tx.try_send(job) {
                Ok(()) => {
                    if !disconnected.is_empty() {
                        self.prune_disconnected(disconnected);
                    }
                    return Ok(());
                }
                Err(std_mpsc::TrySendError::Full(returned)) => {
                    job = returned;
                }
                Err(std_mpsc::TrySendError::Disconnected(returned)) => {
                    job = returned;
                    disconnected.push(idx);
                }
            }
        }
        if !disconnected.is_empty() {
            self.prune_disconnected(disconnected);
        }
        Err(job)
    }

    fn prune_disconnected(&mut self, mut disconnected: Vec<usize>) {
        disconnected.sort_unstable();
        disconnected.dedup();
        for idx in disconnected.into_iter().rev() {
            if idx < self.work_txs.len() {
                self.work_txs.swap_remove(idx);
            }
        }
        if self.next_worker >= self.work_txs.len() {
            self.next_worker = 0;
        }
    }
}

fn resolve_worker_threads(configured: usize) -> usize {
    if configured == 0 {
        std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1)
            .max(1)
    } else {
        configured.max(1)
    }
}

fn resolve_queue_cap(configured: usize, workers: usize) -> usize {
    if configured == 0 {
        workers.saturating_mul(4).max(128)
    } else {
        configured.max(1)
    }
}

fn resolve_enqueue_wait_ms(configured: u64) -> Duration {
    Duration::from_millis(configured)
}

fn resolve_retry_tick_ms(configured: u64) -> Duration {
    Duration::from_millis(configured.max(1))
}

fn resolve_enqueue_poll(wait: Duration) -> Duration {
    if wait > Duration::ZERO {
        Duration::from_millis(1)
    } else {
        Duration::ZERO
    }
}

fn dispatch_pending(pool: &mut WorkerPool, pending: &mut Vec<ZkTask>) {
    if pending.is_empty() {
        set_pending_depth(0);
        return;
    }
    let mut retained = Vec::new();
    for job in pending.drain(..) {
        if let Err(job) = pool.dispatch(job) {
            retained.push(job);
        }
    }
    *pending = retained;
    set_pending_depth(pending.len());
}

fn process_job(job: ZkTask) {
    let dig = job.digest();
    let outcome = ivm::zk::verify_trace(&job.trace, &job.constraints, &job.mem_log, &job.reg_log)
        .map(|()| ZkOutcome::Verified)
        .unwrap_or(ZkOutcome::Rejected);
    if let Some(cache) = RESULT_CACHE.get() {
        cache.record(dig, Instant::now());
    }
    emit_outcome(job, dig, outcome);
}

fn emit_outcome(job: ZkTask, dig: [u8; 32], outcome: ZkOutcome) {
    #[cfg(feature = "zk-preverify")]
    if matches!(outcome, ZkOutcome::Verified) {
        if let Some(header) = &job.header {
            let artifact =
                crate::zk::make_trace_digest_artifact(job.code_hash, job.tx_hash.as_ref(), dig);
            crate::zk::queue_trace_proof(header.height().get(), artifact);
            crate::zk::queue_trace_for_proving(
                header.height().get(),
                crate::zk::TraceForProving::from_task(&job, dig),
            );
        }
    }
    let transport_desc = job.transport_capabilities.as_ref().map(|caps| {
        format!(
            "suite={}, datagram={}, max_dgram={}, feedback_ms={}, privacy={:?}",
            caps.hpke_suite.suite_id(),
            caps.use_datagram,
            caps.max_segment_datagram_size,
            caps.fec_feedback_interval_ms,
            caps.privacy_bucket_granularity
        )
    });
    let negotiated_desc = job
        .negotiated_capabilities
        .map(|flags| format!("0x{:x}", flags.bits()));
    iroha_logger::info!(
        code_hash = %hex::encode(job.code_hash),
        tx_hash = job.tx_hash.as_ref().map(|hash| hex::encode(hash.as_ref())),
        transport = transport_desc.as_deref(),
        negotiated_capabilities = negotiated_desc.as_deref(),
        digest = %hex::encode(dig),
        outcome = ?outcome,
        cycles = job.trace.len(),
        constraints = job.constraints.len(),
        mem_events = job.mem_log.len(),
        reg_events = job.reg_log.len(),
        "zk_lane: verified formal trace"
    );
    if let Some(es) = EVENTS.get() {
        let header = job.header.unwrap_or_else(|| {
            iroha_data_model::block::BlockHeader::new(
                NonZeroU64::new(1).expect("non-zero constant"),
                None,
                None,
                None,
                0,
                0,
            )
        });
        let (kind, details) = match outcome {
            ZkOutcome::Verified => (
                "zk_trace_verified",
                format!(
                    "trace verified: code_hash={}, digest={}, transport={:?}, negotiated={:?}",
                    hex::encode(job.code_hash),
                    hex::encode(dig),
                    transport_desc,
                    negotiated_desc
                ),
            ),
            ZkOutcome::Rejected => (
                "zk_trace_rejected",
                format!(
                    "trace rejected: code_hash={}, digest={}, transport={:?}, negotiated={:?}",
                    hex::encode(job.code_hash),
                    hex::encode(dig),
                    transport_desc,
                    negotiated_desc
                ),
            ),
        };
        let warn = iroha_data_model::events::pipeline::PipelineWarning {
            header,
            kind: kind.to_string(),
            details,
        };
        let _ = es.send(iroha_data_model::events::EventBox::from(
            iroha_data_model::events::pipeline::PipelineEventBox::Warning(warn),
        ));
    }
}

#[cfg(test)]
#[allow(dead_code)]
fn process_batch(batch: &mut Vec<ZkTask>) {
    for job in batch.drain(..) {
        process_job(job);
    }
}

/// Try to submit a task through a globally registered lane, if present.
pub fn try_submit(task: ZkTask) -> bool {
    GLOBAL_SENDER
        .get()
        .is_some_and(|handle| handle.submit(task))
}

/// Register a global events sender to receive ZK verification warnings.
/// No-op if already registered.
pub fn register_events_sender(sender: crate::EventsSender) {
    let _ = EVENTS.set(sender);
}

/// Start the background ZK lane if Halo2 verification is enabled.
///
/// Returns an optional handle and a `tokio` task `JoinHandle` wrapped for
/// supervisor registration. If the lane is already running, returns the
/// existing handle and no new task.
pub fn start(
    cfg: &iroha_config::parameters::actual::Halo2,
) -> Option<(ZkLaneHandle, tokio::task::JoinHandle<()>)> {
    if !cfg.enabled {
        return None;
    }
    if let Some(existing) = GLOBAL_SENDER.get() {
        // Already started in this process.
        return Some((existing.clone(), tokio::spawn(async {})));
    }
    let _ = RESULT_CACHE.set(Arc::new(ZkResultCache::new(
        ZK_RESULT_CACHE_CAP,
        Duration::from_millis(ZK_RESULT_CACHE_TTL_MS),
    )));
    let workers = resolve_worker_threads(cfg.verifier_worker_threads);
    let queue_cap = resolve_queue_cap(cfg.verifier_queue_cap, workers);
    let enqueue_wait = resolve_enqueue_wait_ms(cfg.verifier_enqueue_wait_ms);
    let enqueue_poll = resolve_enqueue_poll(enqueue_wait);
    let retry_tick = resolve_retry_tick_ms(cfg.verifier_retry_tick_ms);
    let retry_ring = Arc::new(RetryRing::new(
        cfg.verifier_retry_ring_cap,
        cfg.verifier_retry_max_attempts,
    ));
    let worker_queue_cap = queue_cap.saturating_div(workers).max(1);
    let (tx, mut rx) = mpsc::channel::<ZkTask>(queue_cap);
    let handle = ZkLaneHandle {
        tx: tx.clone(),
        enqueue_wait,
        enqueue_poll,
        retry_ring: Arc::clone(&retry_ring),
    };
    let _ = GLOBAL_SENDER.set(handle.clone());
    with_metrics(|metrics| {
        metrics
            .zk_halo2_verifier_worker_threads
            .set(u64::try_from(workers).unwrap_or(u64::MAX));
        metrics
            .zk_halo2_verifier_queue_cap
            .set(u64::try_from(queue_cap).unwrap_or(u64::MAX));
    });
    set_pending_depth(0);
    set_retry_ring_depth(0);

    let max_batch = cfg.verifier_max_batch.max(1) as usize;
    let task = tokio::spawn(async move {
        let mut worker_pool = WorkerPool::spawn(workers, worker_queue_cap);
        let mut pending: Vec<ZkTask> = Vec::with_capacity(max_batch);
        loop {
            tokio::select! {
                biased;
                maybe = rx.recv() => {
                    if let Some(job) = maybe {
                        let job = job;
                        let started = Instant::now();
                        loop {
                            if pending.len() < queue_cap {
                                pending.push(job);
                                set_pending_depth(pending.len());
                                if started.elapsed() > Duration::ZERO {
                                    record_enqueue_wait();
                                }
                                break;
                            }
                            dispatch_pending(&mut worker_pool, &mut pending);
                            if started.elapsed() >= enqueue_wait {
                                record_enqueue_timeout();
                                if job.tx_hash.is_some() {
                                    let accepted = enqueue_retry_task(
                                        retry_ring.as_ref(),
                                        job,
                                        "dispatch_retry_ring_full",
                                    );
                                    if !accepted {
                                        iroha_logger::warn!(
                                            queue_cap,
                                            pending = pending.len(),
                                            retry_depth = retry_ring.depth(),
                                            workers,
                                            "zk_lane: dropped important task after dispatch saturation"
                                        );
                                    }
                                } else {
                                    record_lane_drop("dispatch_enqueue_timeout");
                                    iroha_logger::warn!(
                                        queue_cap,
                                        pending = pending.len(),
                                        workers,
                                        "zk_lane: dropping trace verification task due to saturated dispatch backlog"
                                    );
                                }
                                break;
                            }
                            if enqueue_poll > Duration::ZERO {
                                tokio::time::sleep(enqueue_poll).await;
                            } else {
                                tokio::task::yield_now().await;
                            }
                        }
                        if pending.len() >= max_batch {
                            dispatch_pending(&mut worker_pool, &mut pending);
                        }
                    } else {
                        dispatch_pending(&mut worker_pool, &mut pending);
                        break;
                    }
                }
                // Periodically drain small batches to avoid unbounded latency
                () = tokio::time::sleep(retry_tick) => {
                    dispatch_pending(&mut worker_pool, &mut pending);
                }
            }

            let retry_stats = retry_ring.drain_into_pending(&mut pending, queue_cap);
            if retry_stats.replayed > 0 {
                with_metrics(|metrics| {
                    metrics
                        .zk_lane_retry_replayed_total
                        .inc_by(retry_stats.replayed);
                });
                set_pending_depth(pending.len());
            }
            if retry_stats.exhausted > 0 {
                with_metrics(|metrics| {
                    metrics
                        .zk_lane_retry_exhausted_total
                        .inc_by(retry_stats.exhausted);
                });
                record_lane_drop_by("retry_exhausted", retry_stats.exhausted);
                iroha_logger::warn!(
                    exhausted = retry_stats.exhausted,
                    "zk_lane: dropping retry-ring tasks after exhausting retry rounds"
                );
            }
            set_retry_ring_depth(retry_stats.depth);
            if pending.len() >= max_batch {
                dispatch_pending(&mut worker_pool, &mut pending);
            }
        }
        if !pending.is_empty() {
            for task in &pending {
                clear_inflight_digest(task.digest());
            }
            record_lane_drop_by("shutdown_flush_drop", pending.len() as u64);
            iroha_logger::warn!(
                dropped = pending.len(),
                "zk_lane: dropping pending verification tasks during shutdown"
            );
        }
        let retry_dropped = retry_ring.clear();
        if retry_dropped > 0 {
            record_lane_drop_by("shutdown_flush_drop", retry_dropped as u64);
            iroha_logger::warn!(
                dropped = retry_dropped,
                "zk_lane: dropping retry-ring tasks during shutdown"
            );
            set_retry_ring_depth(0);
        }
        let WorkerPool {
            work_txs,
            join_handles: joins,
            ..
        } = worker_pool;
        drop(work_txs);
        let _ = tokio::task::spawn_blocking(move || {
            for join in joins {
                if let Err(err) = join.join() {
                    iroha_logger::warn!(?err, "zk lane verifier thread exited with panic");
                }
            }
        })
        .await;
    });
    Some((handle, task))
}

#[cfg(test)]
mod tests {
    //! Minimal unit tests for `ZkLane` helpers.
    use super::*;

    #[test]
    fn digest_changes_with_roots_and_sizes() {
        fn mk(n: usize) -> ZkTask {
            let mut task = ZkTask {
                tx_hash: None,
                code_hash: [0xAB; 32],
                program: Arc::new(vec![0, 1, 2, 3]),
                header: None,
                trace: Vec::new(),
                constraints: Vec::new(),
                mem_log: Vec::new(),
                reg_log: Vec::new(),
                step_log: Vec::new(),
                transport_capabilities: None,
                negotiated_capabilities: None,
            };
            for i in 0..n {
                task.trace.push(RegisterState {
                    pc: i as u64,
                    gpr: [0; 256],
                    tags: [false; 256],
                });
            }
            task.step_log.push(StepEntry {
                pc: 0,
                reg_root: HashOf::from_untyped_unchecked(Hash::prehashed([1; 32])),
                mem_root: HashOf::from_untyped_unchecked(Hash::prehashed([2; 32])),
            });
            task.step_log.push(StepEntry {
                pc: n as u64,
                reg_root: HashOf::from_untyped_unchecked(Hash::prehashed([3; 32])),
                mem_root: HashOf::from_untyped_unchecked(Hash::prehashed([4; 32])),
            });
            task
        }
        let a = mk(3).digest();
        let b = mk(4).digest();
        assert_ne!(a, b);
    }

    #[test]
    fn queue_cap_defaults_scale_with_workers() {
        assert_eq!(resolve_queue_cap(0, 1), 128);
        assert_eq!(resolve_queue_cap(0, 32), 128);
        assert_eq!(resolve_queue_cap(0, 64), 256);
        assert_eq!(resolve_queue_cap(17, 4), 17);
    }

    #[test]
    fn retry_ring_replays_when_pending_has_capacity() {
        let ring = RetryRing::new(8, 3);
        let task = ZkTask {
            tx_hash: Some(Hash::prehashed([0x11; 32])),
            code_hash: [0x22; 32],
            program: Arc::new(vec![0x01, 0x02]),
            header: None,
            trace: Vec::new(),
            constraints: Vec::new(),
            mem_log: Vec::new(),
            reg_log: Vec::new(),
            step_log: Vec::new(),
            transport_capabilities: None,
            negotiated_capabilities: None,
        };
        assert!(ring.enqueue(task).is_ok(), "ring accepts first task");

        let mut pending = Vec::new();
        let stats = ring.drain_into_pending(&mut pending, 1);
        assert_eq!(stats.replayed, 1);
        assert_eq!(stats.exhausted, 0);
        assert_eq!(stats.depth, 0);
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn retry_ring_exhausts_when_pending_stays_full() {
        let ring = RetryRing::new(4, 2);
        for idx in 0..2 {
            let task = ZkTask {
                tx_hash: Some(Hash::prehashed([idx as u8; 32])),
                code_hash: [0xAA; 32],
                program: Arc::new(vec![idx as u8]),
                header: None,
                trace: Vec::new(),
                constraints: Vec::new(),
                mem_log: Vec::new(),
                reg_log: Vec::new(),
                step_log: Vec::new(),
                transport_capabilities: None,
                negotiated_capabilities: None,
            };
            assert!(ring.enqueue(task).is_ok(), "ring enqueue should succeed");
        }

        let mut pending = vec![ZkTask {
            tx_hash: None,
            code_hash: [0xFF; 32],
            program: Arc::new(vec![0xFF]),
            header: None,
            trace: Vec::new(),
            constraints: Vec::new(),
            mem_log: Vec::new(),
            reg_log: Vec::new(),
            step_log: Vec::new(),
            transport_capabilities: None,
            negotiated_capabilities: None,
        }];
        let first = ring.drain_into_pending(&mut pending, 1);
        assert_eq!(first.exhausted, 0);
        assert_eq!(first.depth, 2);

        let second = ring.drain_into_pending(&mut pending, 1);
        assert_eq!(second.exhausted, 2);
        assert_eq!(second.depth, 0);
    }

    #[test]
    fn result_cache_dedupes_inflight_and_reuses_recent_result() {
        let cache = ZkResultCache::new(8, Duration::from_millis(100));
        let now = Instant::now();
        let key = [0x5A; 32];

        assert_eq!(cache.admit(key, now), CacheAdmission::Accepted);
        assert_eq!(
            cache.admit(key, now + Duration::from_millis(10)),
            CacheAdmission::Inflight
        );

        cache.record(key, now + Duration::from_millis(15));
        assert_eq!(
            cache.admit(key, now + Duration::from_millis(20)),
            CacheAdmission::Cached
        );

        assert_eq!(
            cache.admit(key, now + Duration::from_millis(130)),
            CacheAdmission::Accepted
        );
    }

    #[test]
    fn result_cache_evicts_oldest_entries_when_capacity_reached() {
        let cache = ZkResultCache::new(2, Duration::from_secs(1));
        let now = Instant::now();
        let a = [0x01; 32];
        let b = [0x02; 32];
        let c = [0x03; 32];

        assert_eq!(cache.admit(a, now), CacheAdmission::Accepted);
        cache.record(a, now);
        assert_eq!(cache.admit(b, now), CacheAdmission::Accepted);
        cache.record(b, now + Duration::from_millis(1));
        assert_eq!(cache.admit(c, now), CacheAdmission::Accepted);
        cache.record(c, now + Duration::from_millis(2));

        assert_eq!(
            cache.admit(a, now + Duration::from_millis(3)),
            CacheAdmission::Accepted
        );
        assert_eq!(
            cache.admit(c, now + Duration::from_millis(3)),
            CacheAdmission::Cached
        );
    }

    #[cfg(feature = "zk-preverify")]
    #[test]
    fn process_batch_enqueues_digest_and_trace_jobs() {
        use std::num::NonZeroU64;

        use iroha_data_model::block::BlockHeader;

        crate::zk::reset_trace_proof_state_for_tests();
        crate::zk::reset_trace_proving_state_for_tests();

        let header = BlockHeader::new(
            NonZeroU64::new(42).expect("non-zero"),
            None,
            None,
            None,
            0,
            0,
        );
        let program = Arc::new(vec![0x13, 0x37, 0xC0, 0xDE]);
        let trace = vec![
            RegisterState {
                pc: 0,
                gpr: [0u64; 256],
                tags: [false; 256],
            },
            RegisterState {
                pc: 4,
                gpr: [0u64; 256],
                tags: [false; 256],
            },
        ];
        let task = ZkTask {
            tx_hash: None,
            code_hash: [0xDA; 32],
            program: Arc::clone(&program),
            header: Some(header.clone()),
            trace,
            constraints: Vec::new(),
            mem_log: Vec::new(),
            reg_log: Vec::new(),
            step_log: Vec::new(),
            transport_capabilities: None,
            negotiated_capabilities: None,
        };
        let expected_digest = task.digest();

        let mut batch = vec![task];
        process_batch(&mut batch);
        assert!(batch.is_empty(), "processing drains the batch");

        let proving_jobs = crate::zk::collect_traces_for_proving(header.height().get());
        assert_eq!(proving_jobs.len(), 1, "trace job enqueued for proving");

        let proofs = crate::zk::collect_trace_proofs_for_height(header.height().get());
        assert_eq!(proofs.len(), 1, "digest artifact recorded");
        let artifact = &proofs[0];
        assert_eq!(artifact.backend, "zk-trace/digest");
        assert_eq!(artifact.proof, expected_digest.to_vec());
    }
}
