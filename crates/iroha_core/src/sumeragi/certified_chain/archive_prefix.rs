//! Process-local resume points for durable archive authentication.
//!
//! Archive capture runs inside the commit's State view, so its authentication cannot
//! grow with chain height. A point retains what the sole native verifier finished in
//! this process: the schedule and once-constructed authority after one certified frame.
//!
//! The trust model is the one `prefix_checkpoints` documents for finality proofs. Every
//! resume rechecks the signed genesis identity, chain instance and root scope, the exact
//! durable hash journal through the retained height, and the retained frame's header hash
//! and canonical wire identity, which includes its local certificate. Every height above
//! the point still passes the native verifier. An invalid, missing or forked successor
//! therefore rejects without moving a point, and a failed recheck retires the points so
//! the reader starts again at signed genesis.
//!
//! A frame below the retained height that changes in place leaves the journal unchanged,
//! and a resume does not read it. Fresh walks own that detection: each process start and the
//! archive's periodic audit. Chain authority is unaffected: a
//! prefix the verifier completed stays completed when a local copy later rots, and any
//! peer or restart reading that copy still rejects it.
//!
//! Points are never serialized, accepted from a peer, or consulted by consensus execution.
//! Their retained graphs hold State pool credits. Refused retention only forgoes the
//! optimization and never changes an archive verdict. An enclosing decode-limit scope uses
//! no point, so a caller's budget still sees its complete original physical charges.

use iroha_allocation::{AllocationBudget, AllocationReservation, ChargedShared};

use super::*;

/// One original completed native prefix with independent persistent custody.
pub(crate) struct ArchivePrefixPoint {
    height: u64,
    block_hash: HashOf<IrohaHeader>,
    wire: (u64, Hash),
    journal: Hash,
    genesis: HashOf<IrohaHeader>,
    instance: Hash32,
    root_scope: SumeragiRootScope,
    schedule: schedule::RetainedConsensusSchedule,
    authority: PrefixAuthority,
    // Credits for the shared authority graph, released with this point's last reference.
    authority_credits: AllocationReservation,
}
impl ArchivePrefixPoint {
    fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.schedule.belongs_to(budget) && self.authority_credits.belongs_to(budget)
    }
}

/// The newest completed prefix and its predecessor.
///
/// The predecessor lets a same-height retry produce an actual verification receipt
/// instead of a verdict inferred from a hash.
#[derive(Default)]
pub(crate) struct ArchiveCapturePrefix {
    // Ordered [older, newest]. Replaced owners always drop after this lock releases,
    // so a refund notification never runs under it.
    points: parking_lot::Mutex<[Option<ChargedShared<ArchivePrefixPoint>>; 2]>,
}
impl core::fmt::Debug for ArchiveCapturePrefix {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let points = self.points.lock();
        f.debug_struct("ArchiveCapturePrefix")
            .field("older", &points[0].as_ref().map(|point| point.height))
            .field("newest", &points[1].as_ref().map(|point| point.height))
            .finish()
    }
}
impl ArchiveCapturePrefix {
    // The highest point below `target`; only a lower point can verify `target` again.
    fn nearest_below(&self, target: u64) -> Option<ChargedShared<ArchivePrefixPoint>> {
        let points = self.points.lock();
        points
            .iter()
            .rev()
            .flatten()
            .find(|point| point.height < target)
            .cloned()
    }

    /// The retained point at exactly `height`.
    pub(crate) fn exact(&self, height: u64) -> Option<ChargedShared<ArchivePrefixPoint>> {
        let points = self.points.lock();
        points
            .iter()
            .flatten()
            .find(|point| point.height == height)
            .cloned()
    }

    /// Whether a newer prefix than `height` is retained.
    pub(crate) fn has_point_above(&self, height: u64) -> bool {
        self.points
            .lock()
            .iter()
            .flatten()
            .any(|point| point.height > height)
    }

    /// Retire every retained point. The next authentication starts at signed genesis.
    pub(crate) fn forget(&self) {
        let retired = std::mem::take(&mut *self.points.lock());
        drop(retired);
    }

    /// Keep a completed prefix. Older heights never displace newer points.
    pub(crate) fn retain(&self, point: ChargedShared<ArchivePrefixPoint>) {
        let retired = {
            let mut points = self.points.lock();
            let [older, newest] = &mut *points;
            match (
                older.as_ref().map(|original| original.height),
                newest.as_ref().map(|original| original.height),
            ) {
                (_, None) => newest.replace(point),
                (_, Some(height)) if point.height > height => {
                    let retired = older.take();
                    *older = newest.take();
                    *newest = Some(point);
                    retired
                }
                (_, Some(height)) if point.height == height => newest.replace(point),
                (Some(height), _) if point.height < height => Some(point),
                _ => older.replace(point),
            }
        };
        drop(retired);
    }

    #[cfg(test)]
    pub(crate) fn heights(&self) -> Vec<u64> {
        self.points
            .lock()
            .iter()
            .flatten()
            .map(|point| point.height)
            .collect()
    }
}

// Exact identity of the durable journal through one retained height. It detects a
// changed entry that a retained frame's own hash cannot, and it is never a wire field.
fn journal_digest(hashes: &[HashOf<IrohaHeader>]) -> Option<Hash> {
    Hash::new_from_writer(|writer| {
        for hash in hashes {
            let bytes: &[u8; Hash::LENGTH] = hash.as_ref();
            writer.write_all(bytes)?;
        }
        Ok(())
    })
    .ok()
}

impl<V: StateReadOnly + ?Sized> CertifiedChain<'_, V> {
    fn archive_budget(&self) -> Option<AllocationBudget> {
        match &self.source {
            ChainSource::State(view) => Some(view.execution_budget()),
            ChainSource::Pinned { .. } | ChainSource::Frames { .. } => None,
        }
    }

    // Read the retained frame only after every constant-size binding holds. `None` means the
    // bindings or the frame's identity no longer hold. A failed source read is returned as it
    // is: it can be transient, and it proves nothing about the frame's content.
    fn retained_archive_frame(
        &self,
        point: &ChargedShared<ArchivePrefixPoint>,
        journal: &[HashOf<IrohaHeader>],
    ) -> Result<
        Option<iroha_data_model::block::SharedSignedBlock>,
        ExecutionAttemptError<ChainReadError>,
    > {
        let Some(budget) = self.archive_budget() else {
            return Ok(None);
        };
        let count = usize::try_from(point.height).unwrap_or(usize::MAX);
        if !point.belongs_to(&budget)
            || !ArchivePrefixPoint::belongs_to(point, &budget)
            || point.instance != self.instance
            || point.root_scope != self.root_scope
            || point.genesis != self.genesis.hash()
            || count > journal.len()
            || journal.get(count.wrapping_sub(1)) != Some(&point.block_hash)
            || journal_digest(&journal[..count]) != Some(point.journal)
        {
            return Ok(None);
        }
        let block = self.source.block(point.height)?;
        #[cfg(not(all(test, sumeragi_core_mutation = "HC984")))]
        if block.hash() != point.block_hash
            || block.canonical_wire_identity().ok() != Some(point.wire)
        {
            return Ok(None);
        }
        #[cfg(all(test, sumeragi_core_mutation = "HC984"))]
        let _ = point.wire;
        Ok(Some(block))
    }

    /// Continue from the nearest retained prefix below `target`.
    ///
    /// Returns whether the cursor now sits on that point. On `false` the cursor is untouched,
    /// so the ordinary walk starts at signed genesis. A failed recheck also retires every
    /// retained point; simply having no point below `target` does not.
    ///
    /// # Errors
    /// The retained frame could not be read, or its acquisition deferred locally. Either
    /// leaves the points in place for a retry. A frame that reads back different refuses
    /// the resume instead, and the fresh walk owns its diagnosis.
    pub(crate) fn resume_archive_prefix(
        &self,
        retained: &ArchiveCapturePrefix,
        target: u64,
        journal: &[HashOf<IrohaHeader>],
    ) -> Result<bool, ExecutionAttemptError<ChainReadError>> {
        let Some(point) = retained.nearest_below(target) else {
            return Ok(false);
        };
        let resumed = self.install_archive_point(&point, journal)?;
        if !resumed {
            retained.forget();
        }
        Ok(resumed)
    }

    /// Put the cursor on one point after rechecking it. An existing cursor is left alone.
    ///
    /// # Errors
    /// The retained frame could not be read, or its acquisition deferred locally.
    pub(crate) fn install_archive_point(
        &self,
        point: &ChargedShared<ArchivePrefixPoint>,
        journal: &[HashOf<IrohaHeader>],
    ) -> Result<bool, ExecutionAttemptError<ChainReadError>> {
        let mut cursor = self.prefix.lock();
        if cursor.is_some() {
            return Ok(false);
        }
        let Some(block) = self.retained_archive_frame(point, journal)? else {
            return Ok(false);
        };
        let mut validation = EpochValidationScope::new();
        let tip =
            match read_frame_with_validation(block, point.height, self.root_scope, &mut validation)
            {
                Ok(tip) => tip,
                Err(ExecutionAttemptError::Rejected(_)) => return Ok(false),
                Err(deferred) => return Err(deferred),
            };
        *cursor = Some(VerifiedPrefix {
            tip,
            schedule: point.schedule.canonical().clone(),
            authority: point.authority.clone(),
            validation,
            proof_source: None,
        });
        Ok(true)
    }

    /// Recheck a retained point without moving the cursor.
    ///
    /// # Errors
    /// The retained frame could not be read, or its acquisition deferred locally.
    pub(crate) fn check_archive_point(
        &self,
        point: &ChargedShared<ArchivePrefixPoint>,
        journal: &[HashOf<IrohaHeader>],
    ) -> Result<bool, ExecutionAttemptError<ChainReadError>> {
        Ok(self.retained_archive_frame(point, journal)?.is_some())
    }

    /// Whether the cursor has verified anything beyond signed genesis.
    pub(crate) fn archive_cursor_past_genesis(&self) -> bool {
        self.prefix
            .lock()
            .as_ref()
            .is_some_and(|prefix| prefix.tip.height > GENESIS_HEIGHT)
    }

    /// Export the cursor's completed prefix, or `None` when retention is refused.
    ///
    /// The cursor only ever sits on signed genesis or a frame the native verifier finished.
    /// Genesis stays out because a fresh start reaches it at constant cost.
    pub(crate) fn archive_prefix_point(
        &self,
        journal: &[HashOf<IrohaHeader>],
    ) -> Option<ChargedShared<ArchivePrefixPoint>> {
        let budget = self.archive_budget()?;
        let cursor = self.prefix.lock();
        let prefix = cursor.as_ref()?;
        let height = prefix.tip.height;
        if height <= GENESIS_HEIGHT {
            return None;
        }
        let count = usize::try_from(height).ok()?;
        let block = prefix.tip.block();
        let block_hash = block.hash();
        if journal.get(count.checked_sub(1)?) != Some(&block_hash) {
            return None;
        }
        let wire = block.canonical_wire_identity().ok()?;
        let journal = journal_digest(&journal[..count])?;
        let authority_bytes =
            query_reader::authority_retention_bytes(prefix.authority.material()).ok()?;
        let authority_credits = budget.try_reserve_bytes(authority_bytes).ok()?;
        let schedule =
            schedule::RetainedConsensusSchedule::admit(&prefix.schedule, &budget).ok()?;
        let mut shell = budget
            .try_reserve(ChargedShared::<ArchivePrefixPoint>::allocation_layout())
            .ok()?;
        let point = ArchivePrefixPoint {
            height,
            block_hash,
            wire,
            journal,
            genesis: self.genesis.hash(),
            instance: self.instance,
            root_scope: self.root_scope,
            schedule,
            authority: prefix.authority.clone(),
            authority_credits,
        };
        ChargedShared::from_reservation(point, &mut shell).ok()
    }

    /// Discard the cursor so the next read starts at signed genesis.
    pub(crate) fn reset_archive_prefix(&self) {
        let retired = self.prefix.lock().take();
        drop(retired);
    }
}
