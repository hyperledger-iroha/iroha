//! Original membership writer and caller-owned capture phases.

use super::*;

// Capture keeps an actual acquired writer or that writer's original release,
// never a synthetic notification inferred from an error.
type OriginalMembershipGuard<'storage> =
    concread::release::ReleaseGuard<'storage, MutexGuard<'storage, RawMutex, Identity>>;

enum MembershipWriterPhase<'storage> {
    Attached(OriginalMembershipGuard<'storage>),
    Released(concread::release::DeferredRelease),
}

pub(in crate::state::storage_transactions) struct MembershipWriter<'storage> {
    phase: Option<MembershipWriterPhase<'storage>>,
    pub(super) history: Option<history_slot::Slot<'storage>>,
}

impl<'storage> MembershipWriter<'storage> {
    pub(in crate::state::storage_transactions) fn new(
        guard: OriginalMembershipGuard<'storage>,
        history: Option<history_slot::Slot<'storage>>,
    ) -> Self {
        Self {
            phase: Some(MembershipWriterPhase::Attached(guard)),
            history,
        }
    }

    pub(in crate::state::storage_transactions) fn identity(&self) -> &Identity {
        match self.phase.as_ref() {
            Some(MembershipWriterPhase::Attached(guard)) => guard,
            _ => panic!("original membership writer was terminally released"),
        }
    }

    pub(super) fn identity_mut(&mut self) -> &mut Identity {
        match self.phase.as_mut() {
            Some(MembershipWriterPhase::Attached(guard)) => guard,
            _ => panic!("original membership writer was terminally released"),
        }
    }

    fn release(&mut self) {
        if let Some(history) = self.history.as_mut() {
            history.release();
        }
        match self.phase.take() {
            Some(MembershipWriterPhase::Attached(guard)) => {
                // Native unlock does not invoke the original callback.
                let ((), release) = guard.release_deferred(drop);
                self.phase = Some(MembershipWriterPhase::Released(release));
            }
            other => self.phase = other,
        }
    }

    pub(super) fn into_release(mut self) -> MembershipRelease {
        self.take_release()
    }

    fn take_release(&mut self) -> MembershipRelease {
        self.release();
        MembershipRelease {
            _history: self.history.as_mut().map(|history| history.cleanup()),
            _writer: self.take_writer_release(),
        }
    }

    pub(super) fn into_writer_release(mut self) -> concread::release::DeferredRelease {
        assert!(self.history.is_none(), "observation-only writer");
        self.take_writer_release()
    }

    fn take_writer_release(&mut self) -> concread::release::DeferredRelease {
        self.release();
        match self.phase.take() {
            Some(MembershipWriterPhase::Released(release)) => release,
            _ => unreachable!("original membership release custody"),
        }
    }
}

impl Drop for MembershipWriter<'_> {
    fn drop(&mut self) {
        self.release();
    }
}

pub(super) enum MembershipCapturePhase<'storage> {
    Empty,
    Attached(TransactionsBlock<'storage>),
    Prepared(PreparedTransactionsBlock<'storage>),
    Captured(DetachedTransactionsBlock),
    Published(PreparedTransactionsBlock<'storage>),
}

/// Caller-owned preparation and capture of the original membership writer.
///
/// The enclosing aggregate owns all sibling slots before invoking a method
/// that can refuse or unwind. Its Drop releases every slot before destroying
/// any slot. No snapshot or newly constructed release source substitutes for
/// the original guard, staged set, predecessor, or prepared action.
#[must_use = "keep the original membership capture in its enclosing aggregate"]
pub(crate) struct TransactionsCaptureSlot<'storage> {
    pub(super) phase: MembershipCapturePhase<'storage>,
    attempted: bool,
    released: bool,
    // Last: the original staged payloads precede the successful capture's
    // deferred notification. All physical siblings must already be free.
    cleanup: Option<MembershipRelease>,
}

impl<'storage> TransactionsCaptureSlot<'storage> {
    /// Validate while the original block remains in this caller-owned slot.
    /// A refusal is terminal but retains the actual physical writer in place.
    pub(crate) fn try_prepare(&mut self) -> Result<(), TransactionsBlockError> {
        assert!(
            !self.attempted && !self.released,
            "membership capture is one-shot"
        );
        self.attempted = true;
        let MembershipCapturePhase::Attached(block) = &mut self.phase else {
            panic!("original attached membership capture");
        };
        let publication = block.admit_publication()?;
        let next_identity = block
            ._guard
            .history
            .as_mut()
            .expect("original history")
            .next_identity();
        // All fallible work precedes extraction. Only original-owner moves
        // occur until the prepared owner is stored back in the caller slot.
        let MembershipCapturePhase::Attached(block) =
            std::mem::replace(&mut self.phase, MembershipCapturePhase::Empty)
        else {
            unreachable!("original checked membership block");
        };
        self.phase = MembershipCapturePhase::Prepared(PreparedTransactionsBlock::new(
            block,
            publication,
            next_identity,
        ));
        Ok(())
    }

    /// Acquire physical tree publication authority before any visible component
    /// publishes. Refusal retains the exact original work in this caller slot.
    pub(crate) fn try_prepare_physical(&mut self) -> Result<(), TransactionsBlockError> {
        assert!(!self.released, "original membership capture is live");
        let MembershipCapturePhase::Prepared(prepared) = &mut self.phase else {
            panic!("logical original membership preparation precedes physical admission");
        };
        prepared.try_prepare_physical()
    }

    /// Release the prepared original writer while retaining its exact journal
    /// and original notification in this caller-owned slot.
    pub(crate) fn try_capture(&mut self) -> Result<(), TransactionsBlockError> {
        assert!(!self.released, "membership capture was terminally released");
        if matches!(&self.phase, MembershipCapturePhase::Attached(_)) {
            self.try_prepare()?;
        }
        // This assertion also excludes an explicitly released prepared block
        // before taking it out of caller custody.
        match &self.phase {
            MembershipCapturePhase::Prepared(prepared) => {
                prepared.assert_unpublished();
                prepared.block._guard.identity();
            }
            _ => panic!("original prepared membership capture"),
        }
        let MembershipCapturePhase::Prepared(prepared) =
            std::mem::replace(&mut self.phase, MembershipCapturePhase::Empty)
        else {
            unreachable!("original checked membership preparation");
        };
        // Existing detach kernel has no callback, user Drop, semantic
        // refusal or allocation after the prepared owner is extracted.
        let (journal, release) = prepared.detach_retaining();
        self.phase = MembershipCapturePhase::Captured(journal);
        self.cleanup = Some(release);
        Ok(())
    }

    /// Publish only this slot's admitted original action. All callbacks and
    /// displaced membership payloads remain in this slot after physical release.
    pub(crate) fn publish_prepared(&mut self) {
        assert!(!self.released, "membership capture was terminally released");
        let MembershipCapturePhase::Prepared(prepared) = &mut self.phase else {
            panic!("original prepared membership publication");
        };
        prepared.publish_in_place();
        self.cleanup = Some(prepared.block._guard.take_release());
        // No fallible work occurs during this inert successful phase transfer.
        let MembershipCapturePhase::Prepared(prepared) =
            std::mem::replace(&mut self.phase, MembershipCapturePhase::Empty)
        else {
            unreachable!("original completed membership publication");
        };
        self.phase = MembershipCapturePhase::Published(prepared);
    }

    fn executing(&self) -> &TransactionsBlock<'storage> {
        assert!(
            !self.attempted && !self.released,
            "membership execution authority ended"
        );
        match &self.phase {
            MembershipCapturePhase::Attached(block) => block,
            _ => panic!("original executing membership"),
        }
    }

    fn executing_mut(&mut self) -> &mut TransactionsBlock<'storage> {
        assert!(
            !self.attempted && !self.released,
            "membership execution authority ended"
        );
        match &mut self.phase {
            MembershipCapturePhase::Attached(block) => block,
            _ => panic!("original executing membership"),
        }
    }

    #[cfg(test)]
    fn into_executing(mut self) -> TransactionsBlock<'storage> {
        self.executing();
        match std::mem::replace(&mut self.phase, MembershipCapturePhase::Empty) {
            MembershipCapturePhase::Attached(block) => block,
            _ => unreachable!("checked original executing membership"),
        }
    }

    /// Terminally unlock without running any original notification or
    /// dropping any membership payload. This never grants journal authority.
    pub(crate) fn release(&mut self) {
        self.released = true;
        let block = match &mut self.phase {
            MembershipCapturePhase::Attached(block) => Some(block),
            MembershipCapturePhase::Prepared(prepared)
            | MembershipCapturePhase::Published(prepared) => Some(&mut prepared.block),
            MembershipCapturePhase::Captured(_) | MembershipCapturePhase::Empty => None,
        };
        if let Some(block) = block {
            block.release_writers();
            if self.cleanup.is_none() {
                self.cleanup = Some(block._guard.take_release());
            }
        }
    }

    #[cfg(test)]
    pub(super) fn into_prepared(mut self) -> PreparedTransactionsBlock<'storage> {
        assert!(!self.released, "membership capture was terminally released");
        match &self.phase {
            MembershipCapturePhase::Prepared(prepared) => {
                prepared.assert_unpublished();
                prepared.block._guard.identity();
            }
            _ => panic!("original membership preparation did not complete"),
        }
        match std::mem::replace(&mut self.phase, MembershipCapturePhase::Empty) {
            MembershipCapturePhase::Prepared(prepared) => prepared,
            _ => unreachable!("original checked membership preparation"),
        }
    }

    /// Transfer only successful original capture, keeping its actual release
    /// separate until every enclosing physical writer/fence has released.
    pub(crate) fn into_detached(mut self) -> (DetachedTransactionsBlock, MembershipRelease) {
        assert!(!self.released, "membership capture was terminally released");
        match std::mem::replace(&mut self.phase, MembershipCapturePhase::Empty) {
            MembershipCapturePhase::Captured(journal) => (
                journal,
                self.cleanup
                    .take()
                    .expect("original membership capture release"),
            ),
            original => {
                self.phase = original;
                panic!("original membership capture did not complete");
            }
        }
    }
}

impl Drop for TransactionsCaptureSlot<'_> {
    fn drop(&mut self) {
        self.release();
    }
}

impl<'storage> TransactionsBlock<'storage> {
    /// Move the original block into an inert caller-owned capture slot.
    /// No validation, allocation or physical release occurs here.
    pub(crate) fn capture_slot(self) -> TransactionsCaptureSlot<'storage> {
        TransactionsCaptureSlot {
            phase: MembershipCapturePhase::Attached(self),
            attempted: false,
            released: false,
            cleanup: None,
        }
    }

    /// Terminally release the actual writer while retaining its staged set
    /// and original deferred notification in this block. Further membership
    /// reads, writes, preparation and publication reject this released owner.
    pub(crate) fn release_writers(&mut self) {
        self._guard.release();
    }
}

/// Original executing membership and its caller-owned publication/capture phase.
///
/// Execution borrows the original block until preparation is attempted. The
/// enclosing State owns this field before any fallible work and releases all
/// physical sibling writers before dropping any field. Publication retains its
/// original action, identities, payloads and notification in the same slot.
#[must_use = "retain original membership through enclosing physical release"]
pub struct TransactionsBlockField<'storage> {
    pub(super) slot: TransactionsCaptureSlot<'storage>,
}

impl<'storage> TransactionsBlockField<'storage> {
    pub(crate) fn new(block: TransactionsBlock<'storage>) -> Self {
        Self {
            slot: block.capture_slot(),
        }
    }

    pub(crate) fn try_prepare_publication(&mut self) -> Result<(), TransactionsBlockError> {
        self.slot.try_prepare()
    }

    /// Nonblocking physical admission after the enclosing State writer and before
    /// its visibility interval; snapshots acquire those owners in this same order.
    pub(crate) fn try_prepare_physical(&mut self) -> Result<(), TransactionsBlockError> {
        self.slot.try_prepare_physical()
    }

    /// Move the exact charged successor after a normal physical Busy refusal.
    /// The existing field retains all physical-release notices until aggregate cleanup.
    pub(crate) fn recover_preparation(&mut self) -> history::Pending {
        let MembershipCapturePhase::Prepared(prepared) = &mut self.slot.phase else {
            panic!("original logically prepared membership");
        };
        prepared.assert_unpublished();
        let mut original = prepared
            .block
            ._guard
            .history
            .as_mut()
            .expect("original history")
            .recover();
        original.next_identity = Some(std::mem::replace(
            &mut prepared.next_identity,
            original.predecessor.clone(),
        ));
        original
    }

    pub(crate) fn publish_prepared(&mut self) {
        self.slot.publish_prepared();
    }

    pub(crate) fn release_writers(&mut self) {
        self.slot.release();
    }

    #[cfg(test)]
    pub(crate) fn into_executing(self) -> TransactionsBlock<'storage> {
        self.slot.into_executing()
    }

    pub(crate) fn into_capture(self) -> TransactionsCaptureSlot<'storage> {
        self.slot.executing();
        self.slot
    }
}

// The contained slot owns terminal Drop; no wrapper-local destructor can run
// ahead of its original payload/notification custody during consuming transfer.
impl<'storage> std::ops::Deref for TransactionsBlockField<'storage> {
    type Target = TransactionsBlock<'storage>;
    fn deref(&self) -> &Self::Target {
        self.slot.executing()
    }
}

impl std::ops::DerefMut for TransactionsBlockField<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.slot.executing_mut()
    }
}

impl TransactionsReadOnly for TransactionsBlockField<'_> {
    fn get<Q>(&self, key: &Q) -> Option<Value>
    where
        Key: Borrow<Q>,
        Q: Hash + Eq + Ord + ?Sized,
    {
        self.slot.executing().get(key)
    }
}

impl JsonSerializeTrait for TransactionsBlockField<'_> {
    fn json_serialize(&self, out: &mut String) {
        self.slot.executing().json_serialize(out);
    }

    fn json_serialize_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        self.slot.executing().json_serialize_to(out)
    }
}

impl mv::BlockRetirement for TransactionsBlockField<'_> {
    fn release_writers(&mut self) {
        self.slot.release();
    }
}
