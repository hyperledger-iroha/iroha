//! Same-State certified-chain authority for durable application archives.
//!
//! This is local runtime evidence, not an independent wire proof. It uses the current Kura
//! commit certificate and the certified-chain reader's retained-committee trust model. Archive
//! checkpoints bind the certified block id (header and execution result), never a node-local
//! quorum's choice of signatures.
//!
//! [`CertifiedArchiveView::new`] walks from signed genesis. Archive capture runs inside the
//! commit's State view, so it uses [`CertifiedArchiveView::resuming`], which continues from a
//! prefix the native verifier already finished in this process and verifies only the new
//! heights. [`ArchiveAuthentication`] also records which anchors one archive has already
//! proved, so background qualification does not repeat that work on every tick. History below
//! the retained prefix is reread by a periodic audit: one bounded slice per qualification,
//! outside the commit path, so neither captures nor readers wait on it or invalidate it.
//!
//! Retention needs no walk from signed genesis. It only prunes anchors this archive already
//! proved, and reads the block id it binds with [`CertifiedArchiveView::committed_anchor`]. A fresh
//! walk at the next open checks that binding again, and the periodic audit rereads the frames
//! meanwhile.

use std::{
    num::NonZeroUsize,
    time::{Duration, Instant},
};

use iroha_allocation::ChargedShared;
use iroha_data_model::NetworkId;

use crate::{
    execution_attempt::ExecutionAttemptError,
    kura::{ExactReplayBoundary, Kura},
    state::StateReadOnly,
    sumeragi::certified_chain::{
        ArchiveCapturePrefix, ArchivePrefixPoint, CertifiedBlock, CertifiedChain, ChainReadError,
        CommittedBlock,
    },
};

/// How long after one audit completes the next one starts.
///
/// With [`ARCHIVE_AUDIT_SLICE_HEIGHTS`] this bounds how long a frame or record file that changes
/// in place below the retained prefix can go unnoticed by archive readers.
pub(crate) const ARCHIVE_AUDIT_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Heights one audit slice verifies. At about 8 ms per height this keeps a slice well under a
/// second, and auditing 12k heights takes minutes of background ticks.
pub(crate) const ARCHIVE_AUDIT_SLICE_HEIGHTS: u64 = 32;

// At most one slice per interval, however often readers qualify.
const ARCHIVE_AUDIT_SLICE_SPACING: Duration = Duration::from_secs(1);

// Below this height there is no retainable predecessor: genesis is never a retained point.
const GENESIS_SUCCESSOR_HEIGHT: u64 = 2;

/// A failure to bind an archive operation to one exact durable, committed chain boundary.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveFinalityError {
    #[error("archive State is bound to another Kura instance")]
    ForeignKura,
    #[error("archive State or block differs from the frozen durable Kura boundary")]
    BoundaryMismatch,
    #[error("durable Kura boundary changed during archive authentication")]
    BoundaryChanged,
    #[error("cannot read durable Kura archive boundary: {0}")]
    Storage(String),
    #[error(transparent)]
    Chain(#[from] ChainReadError),
    /// Original local history acquisition has not completed.
    #[error(transparent)]
    Deferred(crate::execution_attempt::ExecutionDeferred),
}

impl From<crate::execution_attempt::ExecutionAttemptError<ChainReadError>>
    for ArchiveFinalityError
{
    fn from(error: crate::execution_attempt::ExecutionAttemptError<ChainReadError>) -> Self {
        match error {
            crate::execution_attempt::ExecutionAttemptError::Rejected(error) => Self::Chain(error),
            crate::execution_attempt::ExecutionAttemptError::Deferred(local) => {
                Self::Deferred(local)
            }
        }
    }
}

/// The exact retained content of one archive for one network.
///
/// Records only change with `generation`, and a compaction that preserves `generation`
/// replaces `base`, so equal identities retain exactly the same anchors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ArchiveContentIdentity {
    pub(crate) network_id: NetworkId,
    pub(crate) generation: u64,
    pub(crate) base: Option<[u8; 32]>,
}

#[derive(Clone, Copy, Debug)]
struct AnchorCoverage {
    identity: ArchiveContentIdentity,
    through: u64,
}

// Contiguous anchors that captures authenticated and durably published. Each insertion advances
// both the height and the generation by one, so `identity` also dates every earlier height.
#[derive(Clone, Copy, Debug)]
struct CaptureRun {
    identity: ArchiveContentIdentity,
    first: u64,
    last: u64,
}

#[derive(Debug, Default)]
struct ProvedAnchors {
    coverage: Option<AnchorCoverage>,
    run: Option<CaptureRun>,
}

#[derive(Default)]
struct AuditCursor {
    // The audit's own verified prefix, and the height it has reached in this cycle.
    point: Option<ChargedShared<ArchivePrefixPoint>>,
    verified_through: u64,
    completed_at: Option<Instant>,
    last_slice_at: Option<Instant>,
}

impl core::fmt::Debug for AuditCursor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AuditCursor")
            .field("verified_through", &self.verified_through)
            .field("completed_at", &self.completed_at)
            .finish_non_exhaustive()
    }
}

impl AuditCursor {
    fn slice_due(&self) -> bool {
        self.completed_at
            .is_some_and(|completed| completed.elapsed() >= ARCHIVE_AUDIT_INTERVAL)
            && self
                .last_slice_at
                .is_none_or(|last| last.elapsed() >= ARCHIVE_AUDIT_SLICE_SPACING)
    }

    // Returns the finished cycle's point for the caller to drop after unlocking.
    fn complete(&mut self) -> Option<ChargedShared<ArchivePrefixPoint>> {
        self.verified_through = 0;
        self.completed_at = Some(Instant::now());
        self.point.take()
    }
}

/// What one open archive has already proved against certified history in this process.
///
/// Opening an archive starts empty, so a reopened archive authenticates its history again.
/// Nothing here is persisted or received from elsewhere.
#[derive(Debug, Default)]
pub(crate) struct ArchiveAuthentication {
    prefix: ArchiveCapturePrefix,
    anchors: parking_lot::Mutex<ProvedAnchors>,
    audit: parking_lot::Mutex<AuditCursor>,
}

impl ArchiveAuthentication {
    /// The height through which `identity`'s anchors stand proved.
    pub(crate) fn covered_through(&self, identity: &ArchiveContentIdentity) -> Option<u64> {
        self.anchors
            .lock()
            .coverage
            .filter(|coverage| coverage.identity == *identity)
            .map(|coverage| coverage.through)
    }

    /// Record a complete fresh authentication of `identity`'s anchors through `through`.
    ///
    /// Captures that landed while that walk ran already authenticated their own anchors, so a
    /// contiguous capture run extends the result. Under steady traffic a walk therefore still
    /// establishes coverage even though the archive advanced beneath it.
    pub(crate) fn record_audit(&self, identity: ArchiveContentIdentity, through: u64) {
        let retired = self.record_audit_retiring(identity, through);
        drop(retired);
    }

    /// As [`Self::record_audit`], for a caller holding a lock that a refund must not run under.
    /// The finished audit cycle's point refunds its pool, so the caller drops it after unlocking.
    #[must_use]
    pub(crate) fn record_audit_retiring(
        &self,
        identity: ArchiveContentIdentity,
        through: u64,
    ) -> Option<ChargedShared<ArchivePrefixPoint>> {
        {
            let mut anchors = self.anchors.lock();
            let coverage = match anchors.run {
                Some(run)
                    if run.identity.network_id == identity.network_id
                        && run.identity.base == identity.base
                        && run.first <= through.saturating_add(1)
                        && run.last > through
                        && run.identity.generation.checked_sub(run.last - through)
                            == Some(identity.generation) =>
                {
                    AnchorCoverage {
                        identity: run.identity,
                        through: run.last,
                    }
                }
                _ => AnchorCoverage { identity, through },
            };
            anchors.coverage = Some(coverage);
        }
        self.audit.lock().complete()
    }

    /// Note one capture that authenticated and durably published the anchor at `height`.
    /// A replay of an existing anchor changes nothing.
    pub(crate) fn note_capture(
        &self,
        before: &ArchiveContentIdentity,
        after: ArchiveContentIdentity,
        height: u64,
    ) {
        if before.generation.checked_add(1) != Some(after.generation) {
            return;
        }
        let mut anchors = self.anchors.lock();
        match anchors.run.as_mut() {
            Some(run) if run.identity == *before && run.last.checked_add(1) == Some(height) => {
                run.identity = after;
                run.last = height;
            }
            _ => {
                anchors.run = Some(CaptureRun {
                    identity: after,
                    first: height,
                    last: height,
                });
            }
        }
        if let Some(coverage) = anchors.coverage.as_mut()
            && coverage.identity == *before
            && coverage.through.checked_add(1) == Some(height)
        {
            coverage.identity = after;
            coverage.through = height;
        }
    }

    /// Carry proved anchors across a compaction. It only removed records at or below its new base,
    /// and that base's anchor was authenticated before the compaction was approved.
    pub(crate) fn note_compaction(
        &self,
        before: &ArchiveContentIdentity,
        after: ArchiveContentIdentity,
    ) {
        let mut anchors = self.anchors.lock();
        if let Some(coverage) = anchors.coverage.as_mut()
            && coverage.identity == *before
        {
            coverage.identity = after;
        }
        if let Some(run) = anchors.run.as_mut()
            && run.identity == *before
        {
            run.identity = after;
        }
    }

    /// Discard proved anchors so the next qualification walks from signed genesis, which
    /// fails closed for readers while the fault remains. Retained prefixes stay: what the
    /// verifier finished is still finished, so commit capture keeps its constant cost.
    pub(crate) fn forget_coverage(&self) {
        *self.anchors.lock() = ProvedAnchors::default();
        let retired = std::mem::take(&mut *self.audit.lock());
        drop(retired);
    }

    #[cfg(test)]
    pub(crate) fn expire_audit_for_testing(&self) {
        let mut audit = self.audit.lock();
        audit.completed_at = Some(
            Instant::now()
                .checked_sub(ARCHIVE_AUDIT_INTERVAL)
                .expect("test clock precedes the audit interval"),
        );
        audit.last_slice_at = None;
    }

    #[cfg(test)]
    pub(crate) fn release_audit_slice_for_testing(&self) {
        self.audit.lock().last_slice_at = None;
    }

    #[cfg(test)]
    pub(crate) fn audit_progress_for_testing(&self) -> u64 {
        self.audit.lock().verified_through
    }

    #[cfg(test)]
    pub(crate) fn audit_completed_for_testing(&self) -> bool {
        let audit = self.audit.lock();
        !audit.slice_due() && audit.point.is_none()
    }

    #[cfg(test)]
    pub(crate) fn retained_heights_for_testing(&self) -> Vec<u64> {
        self.prefix.heights()
    }

    #[cfg(test)]
    pub(crate) fn forget_retained_prefix_for_testing(&self) {
        self.prefix.forget();
    }
}

/// What a recheck of proved coverage found for one view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RetainedPrefixCheck {
    /// The retained frame and its journal still hold.
    Confirmed,
    /// This view predates the retained height. Nothing is known to be wrong; retry with a
    /// fresh view.
    Stale,
    /// No point is retained at this height, so the recheck cannot run.
    Missing,
    /// The retained frame or journal changed. The reader's fresh walk owns the diagnosis, and
    /// the next capture's own recheck retires the retained prefixes.
    Changed,
}

#[derive(Default)]
struct ResumeProgress {
    attempted: bool,
    resumed: bool,
    // A fresh walk's own target predecessor, retained after the boundary closes.
    predecessor: Option<ChargedShared<ArchivePrefixPoint>>,
}

/// One immutable State view and its exact durable Kura boundary.
pub struct CertifiedArchiveView<'v, V: StateReadOnly + ?Sized> {
    view: &'v V,
    kura: &'v Kura,
    boundary: ExactReplayBoundary,
    chain: CertifiedChain<'v, V>,
    // Absent for a complete walk from signed genesis.
    retained: Option<&'v ArchiveCapturePrefix>,
    resume: parking_lot::Mutex<ResumeProgress>,
}

impl<'v, V: StateReadOnly + ?Sized> CertifiedArchiveView<'v, V> {
    /// Authenticate the State/Kura association and freeze the durable hash journal.
    pub(crate) fn new(view: &'v V, kura: &'v Kura) -> Result<Self, ArchiveFinalityError> {
        Self::open(view, kura, None)
    }

    /// As [`Self::new`], continuing from a prefix this archive already authenticated.
    ///
    /// An enclosing decode-limit scope keeps the complete fresh walk and its original charges.
    pub(crate) fn resuming(
        view: &'v V,
        kura: &'v Kura,
        authentication: &'v ArchiveAuthentication,
    ) -> Result<Self, ArchiveFinalityError> {
        let retained = (!norito::core::decode_limits_active()).then_some(&authentication.prefix);
        Self::open(view, kura, retained)
    }

    fn open(
        view: &'v V,
        kura: &'v Kura,
        retained: Option<&'v ArchiveCapturePrefix>,
    ) -> Result<Self, ArchiveFinalityError> {
        if !std::ptr::eq(view.kura(), kura) {
            return Err(ArchiveFinalityError::ForeignKura);
        }
        let boundary = kura
            .exact_replay_boundary()
            .map_err(|error| ArchiveFinalityError::Storage(error.to_string()))?;
        if u64::try_from(view.height())
            .ok()
            .is_none_or(|height| height > boundary.count)
        {
            return Err(ArchiveFinalityError::BoundaryMismatch);
        }
        let chain = CertifiedChain::new(view)?;
        Ok(Self {
            view,
            kura,
            boundary,
            chain,
            retained,
            resume: parking_lot::Mutex::new(ResumeProgress::default()),
        })
    }

    /// The exact durable height against which lag is measured.
    pub(crate) fn tip_height(&self) -> u64 {
        self.boundary.count
    }

    /// Read one certified block belonging to both this State and the frozen durable boundary.
    pub(crate) fn block(&self, height: u64) -> Result<CertifiedBlock, ArchiveFinalityError> {
        let block = self.certified(height)?;
        self.require_journal_block(height, &block)?;
        Ok(block)
    }

    fn require_journal_block(
        &self,
        height: u64,
        block: &CertifiedBlock,
    ) -> Result<(), ArchiveFinalityError> {
        let index = usize::try_from(height)
            .ok()
            .and_then(|value| value.checked_sub(1));
        if index
            .and_then(|index| self.boundary.hashes.get(index))
            .copied()
            != Some(block.block_hash())
        {
            return Err(ArchiveFinalityError::BoundaryMismatch);
        }
        Ok(())
    }

    fn certified(&self, height: u64) -> Result<CertifiedBlock, ArchiveFinalityError> {
        let Some(retained) = self.retained else {
            return Ok(self.chain.certified(height)?);
        };
        if !self.prepare_resume(retained, height)? {
            return Ok(self.chain.certified(height)?);
        }
        match self.chain.certified(height) {
            // Kura could not supply the frame, which may be transient, or this view does not
            // reach it. A fresh walk would need the same frame, so keep the points for a retry.
            Err(ExecutionAttemptError::Rejected(
                error @ (ChainReadError::NotInView { .. } | ChainReadError::NotCommitted { .. }),
            )) => Err(error.into()),
            Err(ExecutionAttemptError::Rejected(_)) => {
                // A retained prefix never owns a rejection's diagnosis. The original fresh
                // walk decides, and no retained point survives to speed up a later attempt.
                iroha_logger::warn!(
                    height,
                    "archive authentication rejected a successor of its retained prefix and is walking from signed genesis"
                );
                retained.forget();
                self.resume.lock().resumed = false;
                self.chain.reset_archive_prefix();
                Ok(self.chain.certified(height)?)
            }
            other => Ok(other?),
        }
    }

    // Runs once per view, at its first read. Returns whether the cursor sits on a point.
    fn prepare_resume(
        &self,
        retained: &ArchiveCapturePrefix,
        height: u64,
    ) -> Result<bool, ArchiveFinalityError> {
        let mut progress = self.resume.lock();
        if progress.attempted {
            return Ok(progress.resumed);
        }
        progress.attempted = true;
        if !self.state_matches_journal() {
            retained.forget();
        } else {
            progress.resumed =
                self.chain
                    .resume_archive_prefix(retained, height, &self.boundary.hashes)?;
        }
        if !progress.resumed && height > GENESIS_SUCCESSOR_HEIGHT {
            // Expected once per process start. Anywhere else this repeats the walk under the
            // caller's State view, which long-held-view warnings alone do not attribute.
            iroha_logger::warn!(
                height,
                "archive authentication is walking the certified chain from signed genesis"
            );
            // Stop one height short once. Retaining this predecessor lets a retry of
            // `height` verify it again rather than restart at signed genesis.
            self.chain.certified(height - 1)?;
            progress.predecessor = self.chain.archive_prefix_point(&self.boundary.hashes);
        }
        Ok(progress.resumed)
    }

    /// Require every later read through this view to walk from signed genesis.
    ///
    /// The view's cursor can still be retained afterward. This is how qualification's
    /// periodic audit reads the history a resume does not.
    pub(crate) fn audit_from_genesis(&self) {
        self.resume.lock().attempted = true;
    }

    // A fresh walk compares each frame's hash against both sources. A resume reads one frame,
    // so it compares the two complete hash sequences directly.
    fn state_matches_journal(&self) -> bool {
        let hashes = self.view.block_hashes();
        hashes.len() <= self.boundary.hashes.len()
            && hashes
                .iter()
                .zip(&self.boundary.hashes)
                .all(|(state, journal)| state == journal)
    }

    /// Recheck the retained frame behind proved coverage through `height`.
    ///
    /// This only informs the reader. It retires nothing, so background reads cannot push the
    /// next commit back to a walk: that commit's own resume repeats this recheck on any frame
    /// it depends on, and retires the points itself if the frame really changed.
    ///
    /// # Errors
    /// The retained frame could not be read, or its acquisition deferred locally.
    pub(crate) fn confirm_retained(
        &self,
        height: u64,
    ) -> Result<RetainedPrefixCheck, ArchiveFinalityError> {
        let Some(retained) = self.retained else {
            return Ok(RetainedPrefixCheck::Missing);
        };
        let reached = u64::try_from(self.view.height())
            .is_ok_and(|view_height| view_height >= height)
            && self.boundary.count >= height;
        if !reached {
            return Ok(RetainedPrefixCheck::Stale);
        }
        let Some(point) = retained.exact(height) else {
            return Ok(if retained.has_point_above(height) {
                RetainedPrefixCheck::Stale
            } else {
                RetainedPrefixCheck::Missing
            });
        };
        if self.state_matches_journal()
            && self
                .chain
                .check_archive_point(&point, &self.boundary.hashes)?
        {
            Ok(RetainedPrefixCheck::Confirmed)
        } else {
            Ok(RetainedPrefixCheck::Changed)
        }
    }

    /// Verify the audit's next bounded slice of history from signed genesis.
    ///
    /// Returns the heights `(after, through]` verified by this call, or `None` when no slice
    /// is due or another reader is running one. Only this view's own cursor moves, and later
    /// captures do not disturb the audit's retained prefix.
    ///
    /// # Errors
    /// A frame in the slice failed native verification or left the frozen journal, or its
    /// acquisition deferred locally. The cycle's progress stands until the caller decides.
    pub(crate) fn audit_next_slice(
        &self,
        authentication: &ArchiveAuthentication,
        through: u64,
    ) -> Result<Option<(u64, u64)>, ArchiveFinalityError> {
        let Some(mut audit) = authentication.audit.try_lock() else {
            return Ok(None);
        };
        if !audit.slice_due() {
            return Ok(None);
        }
        audit.last_slice_at = Some(Instant::now());
        self.chain.reset_archive_prefix();
        let resumed = match audit.point.as_ref() {
            Some(point) => self
                .chain
                .install_archive_point(point, &self.boundary.hashes)?,
            None => false,
        };
        let after = if resumed { audit.verified_through } else { 0 };
        if through <= after {
            // Another reader's slice already passed the tip this one sampled.
            let retired = audit.complete();
            drop(audit);
            drop(retired);
            return Ok(None);
        }
        let end = through.min(after.saturating_add(ARCHIVE_AUDIT_SLICE_HEIGHTS));
        for height in after.saturating_add(1)..=end {
            let block = self.chain.certified(height)?;
            self.require_journal_block(height, &block)?;
        }
        let retired = if end >= through {
            audit.complete()
        } else if let Some(point) = self.chain.archive_prefix_point(&self.boundary.hashes) {
            audit.verified_through = end;
            audit.point.replace(point)
        } else {
            // A refused retention costs only this slice: the next one starts from genesis.
            iroha_logger::warn!(height = end, "archive audit could not retain its progress");
            audit.verified_through = 0;
            audit.point.take()
        };
        // A retired point refunds its pool, so it drops after this lock like every other owner.
        drop(audit);
        drop(retired);
        Ok(Some((after, end)))
    }

    /// Read an already proved anchor's committed receipt, from the nearest verified height above.
    ///
    /// This follows parent links down from an identity this node already authenticated and
    /// checks no commit certificates, so its cost is the distance to that identity. The chain
    /// leaves sparse checkpoints as it advances, and the walk leaves more behind it, so only the
    /// first read in a stretch the cache no longer reaches pays for many frames. Use it only for a
    /// height this archive already proved against certified history: the block id it yields binds
    /// that same header and result.
    ///
    /// # Errors
    /// The height is not committed in this view, a frame on the way is unreadable or does not
    /// link to its child, or the frozen journal names another block there.
    pub(crate) fn committed_anchor(
        &self,
        height: u64,
    ) -> Result<CommittedBlock, ArchiveFinalityError> {
        let index = usize::try_from(height)
            .ok()
            .and_then(NonZeroUsize::new)
            .filter(|index| index.get() <= self.view.block_hashes().len())
            .ok_or(ChainReadError::NotCommitted { height })?;
        let block = self
            .view
            .canonical_history()
            .executed_receipt_from_checkpoints(index)
            .map_err(|error| {
                error.map_rejection(|error| ChainReadError::Malformed {
                    height,
                    reason: error.to_string(),
                })
            })?;
        self.require_journal_anchor(height, block.block_hash().as_ref())?;
        Ok(block)
    }

    /// Refuse only if the durable journal changed at or below `height`.
    ///
    /// A retention decision reads a finalized prefix far below the tip, so a commit landing
    /// during it changes nothing the decision read. Requiring an unmoved tip instead would let
    /// steady traffic starve retention until the bound stops that traffic.
    pub(crate) fn verify_unchanged_through(&self, height: u64) -> Result<(), ArchiveFinalityError> {
        let current = self
            .kura
            .exact_replay_boundary()
            .map_err(|error| ArchiveFinalityError::Storage(error.to_string()))?;
        let count = usize::try_from(height).map_err(|_| ArchiveFinalityError::BoundaryChanged)?;
        match (
            current.hashes.get(..count),
            self.boundary.hashes.get(..count),
        ) {
            (Some(current), Some(frozen)) if current == frozen => Ok(()),
            _ => Err(ArchiveFinalityError::BoundaryChanged),
        }
    }

    /// Require an already proved anchor to still name the frozen durable journal's hash.
    pub(crate) fn require_journal_anchor(
        &self,
        height: u64,
        block_hash: &[u8; 32],
    ) -> Result<(), ArchiveFinalityError> {
        let journal = usize::try_from(height)
            .ok()
            .and_then(|value| value.checked_sub(1))
            .and_then(|index| self.boundary.hashes.get(index));
        match journal {
            Some(hash) if <_ as AsRef<[u8; 32]>>::as_ref(hash) == block_hash => Ok(()),
            _ => Err(ArchiveFinalityError::BoundaryMismatch),
        }
    }

    /// Retain this view's verified cursor for the next authentication.
    ///
    /// Call after [`Self::verify_unchanged`]. A point only records completed chain
    /// verification, so an archive's later publication outcome cannot invalidate it, and a
    /// retry after index contention does not repeat the walk.
    pub(crate) fn retain_verified_prefix(&self) {
        let Some(retained) = self.retained else {
            return;
        };
        if let Some(predecessor) = self.resume.lock().predecessor.take() {
            retained.retain(predecessor);
        }
        if let Some(point) = self.chain.archive_prefix_point(&self.boundary.hashes) {
            retained.retain(point);
        } else if self.chain.archive_cursor_past_genesis() {
            iroha_logger::warn!(
                "archive prefix retention was refused; a later capture may walk from signed genesis"
            );
        }
    }

    /// Refuse publication when Kura moved while the archive operation was being prepared.
    pub(crate) fn verify_unchanged(&self) -> Result<(), ArchiveFinalityError> {
        let current = self
            .kura
            .exact_replay_boundary()
            .map_err(|error| ArchiveFinalityError::Storage(error.to_string()))?;
        if current != self.boundary {
            return Err(ArchiveFinalityError::BoundaryChanged);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kura::history_checkpoints::{HISTORY_CHECKPOINT_INTERVAL, HistoryCheckpoint},
        query::store::LiveQueryStore,
        state::{State, World},
        sumeragi::{
            certified_chain::relation_counts,
            test_chain::{CertifiedTestChain, Signers, TestChainConfig},
        },
    };
    use std::sync::Arc;

    fn chain() -> CertifiedTestChain {
        CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000))
            .expect("signed genesis")
    }

    /// Nothing near the target has been verified, so the first read pays for the walk down from
    /// the tip. It leaves those heights behind, so later reads start right at their target.
    #[test]
    fn committed_anchor_starts_from_the_nearest_verified_height() {
        let mut chain = chain();
        for _ in 0..10 {
            chain.commit(Vec::new());
        }
        let view = chain.state().query_view();
        let certified = CertifiedArchiveView::new(&view, chain.kura()).unwrap();
        let tip = u64::try_from(view.height()).unwrap();
        chain.kura().history_checkpoints().clear();
        let (first, counts) = relation_counts::measure(|| certified.committed_anchor(3));
        let first = first.unwrap();
        assert_eq!(counts.frames, (3..=tip).rev().collect::<Vec<_>>());
        assert!(counts.qcs.is_empty(), "no commit certificate is checked");
        let (later, counts) = relation_counts::measure(|| certified.committed_anchor(5));
        let later = later.unwrap();
        assert_eq!(counts.frames, [5], "however far below the tip it is");
        assert_eq!(first.id(), chain.committed(3).id());
        assert_eq!(later.id(), chain.committed(5).id());
    }

    /// The chain leaves a sparse checkpoint as its tip advances, so even a first read stops well
    /// short of the tip. Committing here also leaves nearer entries behind, which only shortens
    /// the read, so the cache is reduced to the one checkpoint the tip's advance records.
    #[test]
    fn committed_anchor_reads_down_from_the_sparse_checkpoint_above_it() {
        let mut chain = chain();
        for _ in 0..HISTORY_CHECKPOINT_INTERVAL + 4 {
            chain.commit(Vec::new());
        }
        let view = chain.state().query_view();
        let certified = CertifiedArchiveView::new(&view, chain.kura()).unwrap();
        let target = HISTORY_CHECKPOINT_INTERVAL - 2;
        let (read, counts) = relation_counts::measure(|| certified.committed_anchor(target));
        assert_eq!(read.unwrap().id(), chain.committed(target).id());
        assert!(
            counts.frames.first() <= Some(&HISTORY_CHECKPOINT_INTERVAL)
                && counts.frames.last() == Some(&target),
            "{:?}",
            counts.frames
        );
        let sparse = chain.committed(HISTORY_CHECKPOINT_INTERVAL);
        let checkpoints = chain.kura().history_checkpoints();
        checkpoints.clear();
        checkpoints.record_sparse(
            HISTORY_CHECKPOINT_INTERVAL,
            HistoryCheckpoint {
                iroha_hash: sparse.block_hash(),
                core_hash: sparse.core_hash(),
                result: sparse.result(),
            },
        );
        let (read, counts) = relation_counts::measure(|| certified.committed_anchor(target));
        assert_eq!(read.unwrap().id(), chain.committed(target).id());
        assert_eq!(
            counts.frames,
            (target..=HISTORY_CHECKPOINT_INTERVAL)
                .rev()
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn committed_anchor_needs_no_commit_certificate_below_it() {
        let mut chain = chain();
        for _ in 0..3 {
            chain.commit(Vec::new());
        }
        // A walk from signed genesis cannot pass this frame, and a committed read never needs to.
        chain
            .kura()
            .corrupt_commit_certificate_for_testing(std::num::NonZeroUsize::new(2).unwrap(), None)
            .unwrap();
        let view = chain.state().query_view();
        let walking = CertifiedArchiveView::new(&view, chain.kura()).unwrap();
        assert!(walking.block(4).is_err());
        let reading = CertifiedArchiveView::new(&view, chain.kura()).unwrap();
        assert_eq!(
            reading.committed_anchor(4).unwrap().id(),
            chain.committed(4).id(),
            "both name the same header and result"
        );
        assert!(reading.committed_anchor(5).is_err(), "not committed");
        assert!(reading.committed_anchor(0).is_err());
    }

    #[test]
    fn a_moved_tip_refuses_the_exact_check_but_not_a_prefix_below_it() {
        let mut chain = chain();
        chain.commit(Vec::new());
        let state = Arc::clone(chain.state());
        let view = state.query_view();
        let certified = CertifiedArchiveView::new(&view, state.kura()).unwrap();
        certified.verify_unchanged().unwrap();
        certified.verify_unchanged_through(2).unwrap();
        chain.commit(Vec::new());
        assert!(matches!(
            certified.verify_unchanged(),
            Err(ArchiveFinalityError::BoundaryChanged)
        ));
        certified.verify_unchanged_through(2).unwrap();
        assert!(
            matches!(
                certified.verify_unchanged_through(3),
                Err(ArchiveFinalityError::BoundaryChanged)
            ),
            "the frozen boundary never covered this height"
        );
    }

    #[test]
    fn certified_archive_authenticates_genesis_and_current_commit_certificates() {
        let mut chain = chain();
        chain.commit_at(2_000, Vec::new());
        let view = chain.state().view();
        let archive = CertifiedArchiveView::new(&view, chain.kura()).unwrap();
        assert_eq!(archive.tip_height(), 2);
        for height in 1..=2 {
            assert_eq!(
                archive.block(height).unwrap().id(),
                chain.committed(height).id()
            );
        }
        assert!(archive.block(0).is_err());
        assert!(archive.block(3).is_err());
        archive.verify_unchanged().unwrap();
    }

    #[test]
    fn certified_archive_rejects_foreign_kura_and_hash_cache_only_state() {
        let chain = chain();
        let kura = Kura::blank_kura_for_testing();
        assert!(matches!(
            CertifiedArchiveView::new(&chain.state().view(), &kura),
            Err(ArchiveFinalityError::ForeignKura)
        ));
        let mut state = State::new_with_chain_and_network_id_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
            "sumeragi-certified-test-chain".parse().unwrap(),
            chain.network_id(),
        );
        state.push_block_hash_for_testing(chain.genesis().hash());
        assert!(matches!(
            CertifiedArchiveView::new(&state.view(), &kura),
            Err(ArchiveFinalityError::BoundaryMismatch)
        ));
    }

    #[test]
    fn certified_archive_refuses_changed_durable_boundary_and_uncommitted_successor() {
        let mut chain = chain();
        let state = Arc::clone(chain.state());
        let kura = Arc::clone(chain.kura());
        let view = state.view();
        let archive = CertifiedArchiveView::new(&view, &kura).unwrap();
        chain.commit_at(2_000, Vec::new());
        assert!(matches!(
            archive.verify_unchanged(),
            Err(ArchiveFinalityError::BoundaryChanged)
        ));
        assert!(
            archive.block(2).is_err(),
            "a durable successor is absent from the frozen State"
        );
    }

    #[test]
    fn certified_archive_does_not_accept_a_below_quorum_certificate() {
        let mut chain = chain();
        chain.commit_at(2_000, Vec::new());
        chain.corrupt_local_quorum_for_test(2, Signers::BelowQuorum);
        let view = chain.state().view();
        let archive = CertifiedArchiveView::new(&view, chain.kura()).unwrap();
        assert!(archive.block(2).is_err());
    }

    #[test]
    fn certified_archive_rejects_identical_durable_frames_for_a_foreign_state_network() {
        use iroha_crypto::{Hash, HashOf};
        use iroha_data_model::NetworkId;
        let chain = chain();
        let foreign_kura = Kura::blank_kura_for_testing();
        let mut foreign = State::new_with_chain_and_network_id_for_testing(
            World::new(),
            Arc::clone(&foreign_kura),
            LiveQueryStore::start_test(),
            "sumeragi-certified-test-chain".parse().unwrap(),
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
                b"another network",
            ))),
        );
        let executed_genesis = chain
            .kura()
            .get_block(
                std::num::NonZeroUsize::MIN,
                &chain.state().ivm_execution_budget(),
            )
            .expect("original genesis history read completes")
            .expect("retained executed genesis");
        assert!(executed_genesis.commit_certificate().is_some());
        let genesis_hash = executed_genesis.hash();
        foreign_kura.store_block(executed_genesis).unwrap();
        foreign.push_block_hash_for_testing(genesis_hash);
        assert_eq!(
            foreign_kura
                .canonical_block_wire_bytes_for_testing(std::num::NonZeroUsize::MIN)
                .unwrap(),
            chain
                .kura()
                .canonical_block_wire_bytes_for_testing(std::num::NonZeroUsize::MIN)
                .unwrap(),
            "foreign State sees byte-identical durable genesis"
        );
        assert!(matches!(
            CertifiedArchiveView::new(&foreign.view(), &foreign_kura),
            Err(ArchiveFinalityError::Chain(ChainReadError::ForeignGenesis))
        ));
    }

    // Authenticate the view's tip the way archive capture does.
    fn authenticate_tip(
        chain: &CertifiedTestChain,
        authentication: &ArchiveAuthentication,
    ) -> Result<u64, ArchiveFinalityError> {
        let view = chain.state().view();
        let height = u64::try_from(view.height()).unwrap();
        let certified = CertifiedArchiveView::resuming(&view, chain.kura(), authentication)?;
        certified.block(height)?;
        certified.verify_unchanged()?;
        certified.retain_verified_prefix();
        Ok(height)
    }

    #[test]
    fn archive_prefix_verifies_new_heights_without_replaying_history() {
        let mut chain = chain();
        for time in [2_000, 3_000, 4_000] {
            chain.commit_at(time, Vec::new());
        }
        let authentication = ArchiveAuthentication::default();
        assert_eq!(authenticate_tip(&chain, &authentication).unwrap(), 4);
        assert_eq!(authentication.retained_heights_for_testing(), [3, 4]);

        // This frame is outside every retained point, and a fresh walk must read it. Success
        // below therefore shows that resuming never walks back through it.
        chain.corrupt_local_quorum_for_test(2, Signers::BelowQuorum);
        assert_eq!(authenticate_tip(&chain, &authentication).unwrap(), 4);
        chain.commit_at(5_000, Vec::new());
        assert_eq!(authenticate_tip(&chain, &authentication).unwrap(), 5);
        assert_eq!(authentication.retained_heights_for_testing(), [4, 5]);

        let view = chain.state().view();
        let fresh = CertifiedArchiveView::new(&view, chain.kura()).unwrap();
        assert!(
            fresh.block(5).is_err(),
            "the complete walk still rejects the changed history frame"
        );
        let reopened = ArchiveAuthentication::default();
        let restarted = CertifiedArchiveView::resuming(&view, chain.kura(), &reopened).unwrap();
        assert!(
            restarted.block(5).is_err(),
            "a new process has no retained prefix and reauthenticates history"
        );
    }

    #[test]
    fn archive_prefix_repeats_a_same_height_read_with_an_actual_verification() {
        let mut chain = chain();
        chain.commit_at(2_000, Vec::new());
        chain.commit_at(3_000, Vec::new());
        let authentication = ArchiveAuthentication::default();
        authenticate_tip(&chain, &authentication).unwrap();
        assert_eq!(authentication.retained_heights_for_testing(), [2, 3]);
        authenticate_tip(&chain, &authentication).unwrap();
        assert_eq!(authentication.retained_heights_for_testing(), [2, 3]);
        // The tip's own certificate is verified on every read, so changing it rejects even
        // though its header hash and the journal are unchanged.
        chain.corrupt_local_quorum_for_test(3, Signers::BelowQuorum);
        assert!(authenticate_tip(&chain, &authentication).is_err());
        assert!(authentication.retained_heights_for_testing().is_empty());
    }

    #[test]
    fn archive_prefix_never_advances_over_a_rejected_successor() {
        let mut chain = chain();
        chain.commit_at(2_000, Vec::new());
        chain.commit_at(3_000, Vec::new());
        let authentication = ArchiveAuthentication::default();
        authenticate_tip(&chain, &authentication).unwrap();
        chain.commit_at(4_000, Vec::new());
        chain.corrupt_local_quorum_for_test(4, Signers::BelowQuorum);
        for _ in 0..2 {
            assert!(authenticate_tip(&chain, &authentication).is_err());
            assert!(
                authentication
                    .retained_heights_for_testing()
                    .iter()
                    .all(|height| *height < 4),
                "a rejected successor is never retained"
            );
        }
    }

    #[test]
    fn archive_prefix_refuses_a_changed_retained_frame_before_trusting_it() {
        let mut chain = chain();
        chain.commit_at(2_000, Vec::new());
        chain.commit_at(3_000, Vec::new());
        let authentication = ArchiveAuthentication::default();
        authenticate_tip(&chain, &authentication).unwrap();
        // Height 3 is the newest point. Its header hash and the journal stay unchanged, so
        // only the recheck of its certificate-inclusive wire identity can notice.
        chain.commit_at(4_000, Vec::new());
        chain.corrupt_local_quorum_for_test(3, Signers::BelowQuorum);
        assert!(authenticate_tip(&chain, &authentication).is_err());
        assert!(authentication.retained_heights_for_testing().is_empty());
    }

    #[test]
    fn archive_prefix_is_bound_to_its_own_chain() {
        let mut first = chain();
        first.commit_at(2_000, Vec::new());
        first.commit_at(3_000, Vec::new());
        let authentication = ArchiveAuthentication::default();
        authenticate_tip(&first, &authentication).unwrap();
        // Another chain at another block time forks the journal below every retained height.
        let mut second = chain();
        second.commit_at(2_500, Vec::new());
        second.commit_at(3_500, Vec::new());
        second.commit_at(4_500, Vec::new());
        assert_eq!(authenticate_tip(&second, &authentication).unwrap(), 4);
        assert_eq!(authentication.retained_heights_for_testing(), [3, 4]);
    }

    #[test]
    fn archive_prefix_is_bypassed_by_an_enclosing_decode_limit_scope() {
        let mut chain = chain();
        chain.commit_at(2_000, Vec::new());
        let authentication = ArchiveAuthentication::default();
        let wide =
            norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 128);
        norito::core::with_decode_limits_scope(wide, || authenticate_tip(&chain, &authentication))
            .unwrap();
        assert!(authentication.retained_heights_for_testing().is_empty());
    }

    fn identity(network_id: NetworkId, generation: u64) -> ArchiveContentIdentity {
        ArchiveContentIdentity {
            network_id,
            generation,
            base: None,
        }
    }

    #[test]
    fn archive_coverage_extends_only_over_the_exact_next_capture() {
        let network_id = chain().network_id();
        let identity = |generation| identity(network_id, generation);
        let authentication = ArchiveAuthentication::default();
        assert_eq!(authentication.covered_through(&identity(1)), None);
        // Nothing to extend until a complete authentication established coverage.
        authentication.note_capture(&identity(1), identity(2), 8);
        assert_eq!(authentication.covered_through(&identity(2)), None);

        authentication.record_audit(identity(2), 8);
        assert_eq!(authentication.covered_through(&identity(2)), Some(8));
        assert_eq!(authentication.covered_through(&identity(3)), None);
        authentication.note_capture(&identity(2), identity(3), 10);
        assert_eq!(
            authentication.covered_through(&identity(2)),
            Some(8),
            "a gap never extends coverage"
        );
        authentication.note_capture(&identity(2), identity(3), 9);
        assert_eq!(authentication.covered_through(&identity(3)), Some(9));
        assert_eq!(authentication.covered_through(&identity(2)), None);
        authentication.note_capture(&identity(3), identity(3), 9);
        assert_eq!(
            authentication.covered_through(&identity(3)),
            Some(9),
            "an exact replay changes nothing"
        );
        authentication.forget_coverage();
        assert_eq!(authentication.covered_through(&identity(3)), None);
    }

    #[test]
    fn a_walk_that_captures_overtook_still_establishes_coverage() {
        let network_id = chain().network_id();
        let identity = |generation| identity(network_id, generation);
        let authentication = ArchiveAuthentication::default();
        // Three captures land while one walk authenticates the anchors at generation 5.
        for (generation, height) in [(5, 10), (6, 11), (7, 12)] {
            authentication.note_capture(&identity(generation), identity(generation + 1), height);
        }
        authentication.record_audit(identity(6), 10);
        assert_eq!(authentication.covered_through(&identity(8)), Some(12));

        // A capture run that does not reach back to the walk's height cannot fill the gap.
        let gapped = ArchiveAuthentication::default();
        gapped.note_capture(&identity(7), identity(8), 12);
        gapped.record_audit(identity(6), 10);
        assert_eq!(gapped.covered_through(&identity(8)), None);
        assert_eq!(gapped.covered_through(&identity(6)), Some(10));

        // Another generation at the same heights is another history.
        let foreign = ArchiveAuthentication::default();
        foreign.note_capture(&identity(5), identity(6), 11);
        foreign.record_audit(identity(9), 10);
        assert_eq!(foreign.covered_through(&identity(6)), None);
        assert_eq!(foreign.covered_through(&identity(9)), Some(10));
    }

    #[test]
    fn archive_coverage_follows_a_compaction_of_the_same_history() {
        let network_id = chain().network_id();
        let before = identity(network_id, 4);
        let after = ArchiveContentIdentity {
            base: Some([7; 32]),
            ..before
        };
        let authentication = ArchiveAuthentication::default();
        authentication.record_audit(before, 9);
        authentication.note_compaction(&before, after);
        assert_eq!(authentication.covered_through(&before), None);
        assert_eq!(authentication.covered_through(&after), Some(9));
    }

    #[test]
    fn retained_check_separates_a_stale_view_from_a_changed_frame() {
        let mut chain = chain();
        chain.commit_at(2_000, Vec::new());
        chain.commit_at(3_000, Vec::new());
        let authentication = ArchiveAuthentication::default();
        authenticate_tip(&chain, &authentication).unwrap();
        assert_eq!(authentication.retained_heights_for_testing(), [2, 3]);
        let state = Arc::clone(chain.state());
        let kura = Arc::clone(chain.kura());
        let old_view = state.view();
        let old = CertifiedArchiveView::resuming(&old_view, &kura, &authentication).unwrap();
        assert_eq!(
            old.confirm_retained(3).unwrap(),
            RetainedPrefixCheck::Confirmed
        );

        chain.commit_at(4_000, Vec::new());
        authenticate_tip(&chain, &authentication).unwrap();
        assert_eq!(
            old.confirm_retained(4).unwrap(),
            RetainedPrefixCheck::Stale,
            "a view from before this capture proves nothing and harms nothing"
        );
        assert_eq!(
            old.confirm_retained(2).unwrap(),
            RetainedPrefixCheck::Stale,
            "newer points mean the coverage this reader sampled has moved on"
        );
        assert_eq!(authentication.retained_heights_for_testing(), [3, 4]);

        chain.commit_at(5_000, Vec::new());
        let view = state.view();
        let current = CertifiedArchiveView::resuming(&view, &kura, &authentication).unwrap();
        assert_eq!(
            current.confirm_retained(5).unwrap(),
            RetainedPrefixCheck::Missing,
            "this height was never retained, and nothing newer is"
        );
        assert_eq!(authentication.retained_heights_for_testing(), [3, 4]);
        drop(current);
        drop(view);
        drop(old);
        drop(old_view);

        chain.corrupt_local_quorum_for_test(4, Signers::BelowQuorum);
        let view = state.view();
        let current = CertifiedArchiveView::resuming(&view, &kura, &authentication).unwrap();
        assert_eq!(
            current.confirm_retained(4).unwrap(),
            RetainedPrefixCheck::Changed
        );
        assert_eq!(
            authentication.retained_heights_for_testing(),
            [3, 4],
            "a reader's recheck retires nothing"
        );
        drop(current);
        drop(view);
        assert!(authenticate_tip(&chain, &authentication).is_err());
        assert!(
            authentication.retained_heights_for_testing().is_empty(),
            "the commit path's own recheck retires them"
        );
    }

    #[test]
    fn a_failed_frame_read_keeps_the_points_for_a_retry() {
        let mut chain = chain();
        chain.commit_at(2_000, Vec::new());
        chain.commit_at(3_000, Vec::new());
        let authentication = ArchiveAuthentication::default();
        authenticate_tip(&chain, &authentication).unwrap();
        assert_eq!(authentication.retained_heights_for_testing(), [2, 3]);
        // The view names a successor Kura does not hold, as a failed read of the target does.
        let view = chain.state().view();
        let certified =
            CertifiedArchiveView::resuming(&view, chain.kura(), &authentication).unwrap();
        assert!(certified.block(9).is_err());
        assert_eq!(authentication.retained_heights_for_testing(), [2, 3]);
    }

    #[test]
    fn audit_advances_in_bounded_slices_across_captures_and_views() {
        let mut chain = chain();
        for height in 2..=(ARCHIVE_AUDIT_SLICE_HEIGHTS + 8) {
            chain.commit_at(height * 1_000, Vec::new());
        }
        let authentication = ArchiveAuthentication::default();
        let slice = |chain: &CertifiedTestChain, through| {
            let view = chain.state().view();
            let certified =
                CertifiedArchiveView::resuming(&view, chain.kura(), &authentication).unwrap();
            authentication.release_audit_slice_for_testing();
            certified.audit_next_slice(&authentication, through)
        };
        let tip = ARCHIVE_AUDIT_SLICE_HEIGHTS + 8;
        assert_eq!(
            slice(&chain, tip).unwrap(),
            None,
            "no audit has been due yet"
        );

        authentication.record_audit(identity(chain.network_id(), 1), tip);
        assert_eq!(slice(&chain, tip).unwrap(), None, "an audit just completed");

        authentication.expire_audit_for_testing();
        assert_eq!(
            slice(&chain, tip).unwrap(),
            Some((0, ARCHIVE_AUDIT_SLICE_HEIGHTS))
        );
        assert_eq!(
            authentication.audit_progress_for_testing(),
            ARCHIVE_AUDIT_SLICE_HEIGHTS
        );
        // A height landing beneath the audit does not restart it.
        chain.commit_at((tip + 1) * 1_000, Vec::new());
        assert_eq!(
            slice(&chain, tip + 1).unwrap(),
            Some((ARCHIVE_AUDIT_SLICE_HEIGHTS, tip + 1))
        );
        assert!(authentication.audit_completed_for_testing());
        assert_eq!(slice(&chain, tip + 1).unwrap(), None);
    }

    #[test]
    fn audit_rejects_a_changed_history_frame_that_a_resume_never_reads() {
        let mut chain = chain();
        for time in [2_000, 3_000, 4_000] {
            chain.commit_at(time, Vec::new());
        }
        let authentication = ArchiveAuthentication::default();
        authenticate_tip(&chain, &authentication).unwrap();
        chain.corrupt_local_quorum_for_test(2, Signers::BelowQuorum);
        authenticate_tip(&chain, &authentication).unwrap();
        authentication.record_audit(identity(chain.network_id(), 1), 4);
        authentication.expire_audit_for_testing();
        let view = chain.state().view();
        let certified =
            CertifiedArchiveView::resuming(&view, chain.kura(), &authentication).unwrap();
        assert!(certified.audit_next_slice(&authentication, 4).is_err());
    }
}
