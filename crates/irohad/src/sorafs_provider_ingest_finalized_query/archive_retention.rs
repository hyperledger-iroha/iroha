//! Supervised retention of the daemon-owned finalized provider-ingest archive.
//!
//! The archive refuses new records at its capacity bounds, and a refused capture holds the
//! committed decision, so reaching a bound stops this node committing. This worker keeps the
//! archive below the bounds in small steps, off the commit path. Every checkpoint still needs the
//! deployment-owned sealed authority's approval, so a node without one stays in manual retention
//! and the worker only warns while there is time to act. It never stops on a failure: stopping a
//! supervised child would shut down the node, for a condition that only matters at the bound.

use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, eyre};
use iroha_core::{
    kura::Kura,
    query::provider_ingest_finalized::{
        ProviderIngestFinalizedArchiveCapacityV1, ProviderIngestFinalizedArchiveRetentionStepV1,
        ProviderIngestFinalizedArchiveV1,
    },
    state::State,
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};

use super::QualifiedProviderIngestRetentionAuthorityV1;
use crate::panic_recovery::{RecoverablePanic, recover_joined, spawn_blocking_recoverable};

/// How often the worker looks at the archive. An idle look is one short reader.
const RETENTION_TICK_INTERVAL: Duration = Duration::from_secs(5);
const RETENTION_SHUTDOWN_WAIT: Duration = Duration::from_secs(5);
/// How often one lasting condition may repeat its warning.
const RETENTION_WARNING_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Continuation hints expire after this interval. They never protect pending admission:
/// every in-flight page owns a separate nonexpiring exact-anchor lease through its
/// durable admission boundary, regardless of fetch, signing or submission duration.
const SCAN_PIN_LIFETIME: Duration = Duration::from_secs(2 * 60 * 60);

/// Shared by all readers and the worker for one archive tenure. Registration and release
/// serialize against the complete retention step, including sealed-outbox sampling.
#[derive(Debug, Default)]
pub(super) struct ArchiveAnchorLeaseRegistryV1 {
    active: Mutex<BTreeMap<u64, usize>>,
    // Accessed only while holding `active`. A failed step may have approved a floor
    // not yet visible locally, so no new page can trust the still-present prefix.
    admission_blocked: AtomicBool,
}

struct ArchiveAnchorLeaseV1 {
    registry: Arc<ArchiveAnchorLeaseRegistryV1>,
    height: u64,
}
impl Drop for ArchiveAnchorLeaseV1 {
    fn drop(&mut self) {
        // Poison keeps retention fail-closed; never recover an untrusted pin set.
        if let Ok(mut active) = self.registry.active.lock()
            && let Some(count) = active.get_mut(&self.height)
        {
            *count -= 1;
            if *count == 0 {
                active.remove(&self.height);
            }
        }
    }
}
impl ArchiveAnchorLeaseRegistryV1 {
    pub(super) fn retain_exact_cursor(
        self: &Arc<Self>,
        archive: &ProviderIngestFinalizedArchiveV1,
        network_id: &iroha_data_model::NetworkId,
        cursor: sorafs_node::provider_ingest_outbox::ProviderIngestFinalizedCursorV1,
    ) -> Result<
        sorafs_node::provider_ingest_runtime::ProviderIngestFinalizedAnchorLeaseV1,
        sorafs_node::provider_ingest_runtime::ProviderIngestFinalizedLedgerErrorV1,
    > {
        use sorafs_node::provider_ingest_runtime::{
            ProviderIngestFinalizedAnchorLeaseV1, ProviderIngestFinalizedLedgerErrorV1,
        };
        let mut active = self
            .active
            .lock()
            .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Unavailable)?;
        if self.admission_blocked.load(Ordering::Acquire) {
            return Err(ProviderIngestFinalizedLedgerErrorV1::Unavailable);
        }
        archive
            .resolve_exact_key(network_id, cursor.height, cursor.block_hash)
            .map_err(super::scan_read_error)?;
        let count = active.entry(cursor.height).or_default();
        *count = count
            .checked_add(1)
            .ok_or(ProviderIngestFinalizedLedgerErrorV1::Unavailable)?;
        Ok(ProviderIngestFinalizedAnchorLeaseV1::retain(
            ArchiveAnchorLeaseV1 {
                registry: Arc::clone(self),
                height: cursor.height,
            },
        ))
    }
}

/// Obligations cannot be discarded. Recent continuations are only advisory and cannot
/// veto recovery of an already-approved floor while all new admission is blocked.
#[derive(Default)]
pub(super) struct ProviderIngestArchivePinsSnapshotV1 {
    active_jobs: Vec<u64>,
    scan_hints: Vec<u64>,
}
impl From<Vec<u64>> for ProviderIngestArchivePinsSnapshotV1 {
    fn from(active_jobs: Vec<u64>) -> Self {
        Self {
            active_jobs,
            scan_hints: Vec::new(),
        }
    }
}
impl ProviderIngestArchivePinsSnapshotV1 {
    fn into_heights(mut self, include_scan_hints: bool) -> Vec<u64> {
        if include_scan_hints {
            self.active_jobs.append(&mut self.scan_hints);
        }
        self.active_jobs
    }
}
/// Read the sealed active-job pins and advisory scan hints without retaining their locks.
pub(super) type ProviderIngestArchivePinsV1 =
    Arc<dyn Fn() -> Result<ProviderIngestArchivePinsSnapshotV1> + Send + Sync>;

/// The anchor one reader's page scan last read, and when.
#[derive(Debug, Clone, Copy)]
pub(super) struct ArchiveScanPinV1 {
    height: u64,
    read_at: Instant,
}

/// Where one reader leaves its scan pin for the worker.
pub(super) type ArchiveScanPinSlotV1 = Arc<Mutex<Option<ArchiveScanPinV1>>>;

/// Record the anchor a reader just served a page from.
pub(super) fn note_scan_page(slot: &ArchiveScanPinSlotV1, height: u64) {
    if let Ok(mut pin) = slot.lock() {
        *pin = Some(ArchiveScanPinV1 {
            height,
            read_at: Instant::now(),
        });
    }
}

fn recent_scan_height(slot: &ArchiveScanPinSlotV1, now: Instant) -> Result<Option<u64>> {
    let pin = slot
        .lock()
        .map_err(|_| eyre!("provider-ingest archive scan pin is poisoned"))?;
    Ok(pin
        .filter(|pin| now.saturating_duration_since(pin.read_at) < SCAN_PIN_LIFETIME)
        .map(|pin| pin.height))
}

/// What this node's running components still read from the archive: each active job's immutable
/// admission anchor, and the anchor of each scan's latest page. The page pin stays after a scan's
/// last page as a continuation hint. Admission itself requires a nonexpiring lease.
pub(super) fn running_node_pins(
    active_admission_heights: impl Fn() -> Result<Vec<u64>> + Send + Sync + 'static,
    scan_pins: [ArchiveScanPinSlotV1; 3],
) -> ProviderIngestArchivePinsV1 {
    Arc::new(move || {
        let active_jobs =
            active_admission_heights().wrap_err("read provider-ingest active admission heights")?;
        let mut scan_hints = Vec::new();
        let now = Instant::now();
        for slot in &scan_pins {
            scan_hints.extend(recent_scan_height(slot, now)?);
        }
        Ok(ProviderIngestArchivePinsSnapshotV1 {
            active_jobs,
            scan_hints,
        })
    })
}

/// One look at the archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RetentionTickV1 {
    /// No authority is configured, so this node can only report how full the archive is.
    Manual(ProviderIngestFinalizedArchiveCapacityV1),
    /// One bounded step under the configured authority.
    Step(ProviderIngestFinalizedArchiveRetentionStepV1),
}

/// State and authority for one bounded retention step at a time.
pub(super) struct ProviderIngestArchiveRetentionWorkerV1 {
    archive: Arc<ProviderIngestFinalizedArchiveV1>,
    state: Arc<State>,
    kura: Arc<Kura>,
    authority: Option<QualifiedProviderIngestRetentionAuthorityV1>,
    pins: ProviderIngestArchivePinsV1,
    anchor_leases: Arc<ArchiveAnchorLeaseRegistryV1>,
}

impl ProviderIngestArchiveRetentionWorkerV1 {
    pub(super) fn new(
        archive: Arc<ProviderIngestFinalizedArchiveV1>,
        state: Arc<State>,
        kura: Arc<Kura>,
        authority: Option<QualifiedProviderIngestRetentionAuthorityV1>,
        pins: ProviderIngestArchivePinsV1,
        anchor_leases: Arc<ArchiveAnchorLeaseRegistryV1>,
    ) -> Self {
        Self {
            archive,
            state,
            kura,
            authority,
            pins,
            anchor_leases,
        }
    }

    fn tick(&self) -> Result<RetentionTickV1> {
        let Some(authority) = &self.authority else {
            return self
                .archive
                .capacity()
                .map(RetentionTickV1::Manual)
                .map_err(|error| eyre!("read archive capacity: {error}"));
        };
        // Lock BEFORE reading the sealed outbox. A page admission therefore appears as
        // either a still-live lease or a durable active job, with no snapshot/drop gap.
        // The callback may take the outbox lock; no admission drops its lease while
        // holding that lock. Keep this fence through authority CAS and physical unlink.
        let leases = self
            .anchor_leases
            .active
            .lock()
            .map_err(|_| eyre!("provider-ingest anchor lease registry is poisoned"))?;
        let recovery_blocked = self
            .anchor_leases
            .admission_blocked
            .swap(true, Ordering::AcqRel);
        let mut pinned_heights = (self.pins)()?.into_heights(!recovery_blocked);
        pinned_heights.extend(leases.keys().copied());
        let view = self.state.query_view();
        let step = self
            .archive
            .reconcile_retention_once(
                &view,
                self.kura.as_ref(),
                authority.binding(),
                authority.authority().as_ref(),
                &pinned_heights,
            )
            .map_err(|error| eyre!("retention step: {error}"))?;
        // Core recovery checks any interrupted approval before returning Idle/Pinned.
        // AwaitingProof cannot prove a previously approved floor was installed yet.
        if !matches!(
            step,
            ProviderIngestFinalizedArchiveRetentionStepV1::AwaitingProof
        ) {
            self.anchor_leases
                .admission_blocked
                .store(false, Ordering::Release);
        }
        Ok(RetentionTickV1::Step(step))
    }
}

/// Reports what the worker found, repeating a lasting warning only occasionally.
#[derive(Default)]
struct RetentionReporterV1 {
    last_warning: Option<Instant>,
}

impl RetentionReporterV1 {
    fn warning_due(&mut self, now: Instant) -> bool {
        let due = self
            .last_warning
            .is_none_or(|last| now.duration_since(last) >= RETENTION_WARNING_INTERVAL);
        if due {
            self.last_warning = Some(now);
        }
        due
    }

    fn report(
        &mut self,
        result: std::result::Result<Result<RetentionTickV1>, RecoverablePanic>,
        now: Instant,
    ) {
        match result {
            Ok(Ok(RetentionTickV1::Manual(capacity))) => {
                if capacity.at_high_watermark() && self.warning_due(now) {
                    iroha_logger::warn!(
                        retained_entries = capacity.retained_entries(),
                        maximum_entries = capacity.maximum_entries(),
                        total_bytes = capacity.total_bytes(),
                        maximum_total_bytes = capacity.maximum_total_bytes(),
                        "finalized provider-ingest archive is nearly full and no retention authority is configured; at its bound this node stops committing"
                    );
                }
            }
            Ok(Ok(RetentionTickV1::Step(step))) => self.report_step(step, now),
            Ok(Err(error)) => {
                if self.warning_due(now) {
                    iroha_logger::warn!(
                        error = %format!("{error:#}"),
                        "finalized provider-ingest archive retention step failed; retrying"
                    );
                }
            }
            Err(RecoverablePanic) => {
                if self.warning_due(now) {
                    iroha_logger::error!(
                        "finalized provider-ingest archive retention step panicked; retrying"
                    );
                }
            }
        }
    }

    fn report_step(&mut self, step: ProviderIngestFinalizedArchiveRetentionStepV1, now: Instant) {
        match step {
            ProviderIngestFinalizedArchiveRetentionStepV1::Idle => {}
            ProviderIngestFinalizedArchiveRetentionStepV1::AwaitingProof => {
                iroha_logger::debug!(
                    "finalized provider-ingest archive retention is waiting for qualification to prove its anchors"
                );
            }
            ProviderIngestFinalizedArchiveRetentionStepV1::Pinned {
                pinned_height,
                retained_entries,
            } => {
                if self.warning_due(now) {
                    iroha_logger::warn!(
                        pinned_height,
                        retained_entries,
                        "finalized provider-ingest archive retention is due but history at this height is still in use; compaction resumes when that job or scan finishes"
                    );
                }
            }
            ProviderIngestFinalizedArchiveRetentionStepV1::Compacted(outcome) => {
                iroha_logger::info!(
                    retention_floor_height = outcome.retention_floor().height,
                    pruned_entries = outcome.pruned_entries(),
                    pruned_bytes = outcome.pruned_bytes(),
                    "finalized provider-ingest archive compacted a retained prefix"
                );
            }
        }
    }
}

/// Run the worker under the supervisor until shutdown.
pub(super) fn start(
    worker: ProviderIngestArchiveRetentionWorkerV1,
    shutdown_signal: ShutdownSignal,
) -> Child {
    let task = tokio::spawn(run(Arc::new(worker), shutdown_signal));
    Child::new(task, OnShutdown::Wait(RETENTION_SHUTDOWN_WAIT))
}

async fn run(worker: Arc<ProviderIngestArchiveRetentionWorkerV1>, shutdown_signal: ShutdownSignal) {
    let mut interval = tokio::time::interval(RETENTION_TICK_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut reporter = RetentionReporterV1::default();
    loop {
        tokio::select! {
            _ = interval.tick() => {
                let worker = Arc::clone(&worker);
                let mut tick = spawn_blocking_recoverable(move || worker.tick());
                tokio::select! {
                    joined = &mut tick => reporter.report(recover_joined(joined), Instant::now()),
                    () = shutdown_signal.receive() => {
                        tick.abort();
                        break;
                    }
                }
            }
            () = shutdown_signal.receive() => break,
        }
    }
}

#[cfg(test)]
mod tests;
