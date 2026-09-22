//! Caller-owned reacquisition of the exact admitted membership transition.

use super::*;
use concread::release::DeferredRelease;
use mv::PublicationPreparationError;

enum Phase<'storage> {
    Original(DetachedTransactionsBlock),
    Prepared(PreparedTransactionsBlock<'storage>),
}

/// Retains every original acquisition and notification before preparation runs.
///
/// An aggregate installs this inert slot before preparing any sibling. Its
/// physical-only release pass must finish before it destroys any slot.
#[must_use = "retain this slot in the owner of all enclosing publication fences"]
pub(crate) struct DetachedTransactionsPublicationSlot<'storage, Installation> {
    target: &'storage TransactionsStorage,
    phase: Option<Phase<'storage>>,
    writer: Option<MembershipWriter<'storage>>,
    attempted: bool,
    retryable: bool,
    complete: bool,
    released: bool,
    installation: Option<Installation>,
    preflight_release: Option<DeferredRelease>,
    writer_release: Option<DeferredRelease>,
}

impl DetachedTransactionsBlock {
    /// Move the original journal into an inert caller-owned preparation slot.
    pub(crate) fn publication_slot<Installation>(
        self,
        target: &TransactionsStorage,
    ) -> DetachedTransactionsPublicationSlot<'_, Installation> {
        DetachedTransactionsPublicationSlot {
            target,
            phase: Some(Phase::Original(self)),
            writer: None,
            attempted: false,
            retryable: true,
            complete: false,
            released: false,
            installation: None,
            preflight_release: None,
            writer_release: None,
        }
    }
}

impl<'storage, Installation> DetachedTransactionsPublicationSlot<'storage, Installation> {
    fn original(&self) -> &DetachedTransactionsBlock {
        match self.phase.as_ref() {
            Some(Phase::Original(original)) => original,
            _ => panic!("original detached membership phase"),
        }
    }

    fn refuse<E>(
        &mut self,
        error: PublicationPreparationError<E>,
    ) -> Result<(), PublicationPreparationError<E>> {
        self.retryable = true;
        Err(error)
    }

    /// Observe, admit and acquire once while all original owners stay here.
    pub(crate) fn try_prepare<E>(
        &mut self,
        admit: impl FnOnce(&DetachedTransactionsBlock, &TransactionsStorage) -> Result<Installation, E>,
    ) -> Result<(), PublicationPreparationError<E>> {
        assert!(
            !self.attempted && !self.released,
            "detached membership preparation is one-shot"
        );
        self.attempted = true;
        // A caught admission panic never grants retry authority.
        self.retryable = false;
        let target = self.target;
        let wait = target.released.observe();
        let Some(guard) = target.write_lock.try_lock() else {
            return self.refuse(PublicationPreparationError::after_failed_acquisition(wait));
        };
        self.writer = Some(MembershipWriter::new(target.released.guard(guard)));
        let current = Arc::ptr_eq(
            self.writer
                .as_ref()
                .expect("original observation writer")
                .identity(),
            &self.original().predecessor_identity,
        );
        // Unlock without notifying. Keep the actual event before admission can
        // run arbitrary code, refuse, or unwind through the enclosing owner.
        self.preflight_release = Some(
            self.writer
                .take()
                .expect("original observation writer")
                .into_release(),
        );
        if !current {
            return self.refuse(PublicationPreparationError::Changed);
        }
        let installation = match admit(self.original(), target) {
            Ok(installation) => installation,
            Err(error) => return self.refuse(PublicationPreparationError::Admission(error)),
        };
        self.installation = Some(installation);
        let wait = target.released.observe();
        let Some(guard) = target.write_lock.try_lock() else {
            return self.refuse(PublicationPreparationError::after_failed_acquisition(wait));
        };
        // Install the actual final acquisition before checking its predecessor.
        self.writer = Some(MembershipWriter::new(target.released.guard(guard)));
        if !Arc::ptr_eq(
            self.writer
                .as_ref()
                .expect("original publication writer")
                .identity(),
            &self.original().predecessor_identity,
        ) {
            return self.refuse(PublicationPreparationError::Changed);
        }
        // All checks precede extraction. These moves reuse the admitted action,
        // immutable payload and next identity; no new admission or allocation.
        let Some(Phase::Original(original)) = self.phase.take() else {
            unreachable!("checked original membership phase");
        };
        let DetachedTransactionsBlock {
            predecessor_identity: _,
            predecessor: _,
            current,
            revert,
            publication,
            next_identity,
        } = original;
        self.phase = Some(Phase::Prepared(PreparedTransactionsBlock::new(
            TransactionsBlock {
                latest_block_ref: &target.latest_block,
                blocks_ref: &target.blocks,
                _guard: self
                    .writer
                    .take()
                    .expect("checked original publication writer"),
                revert,
                current_block: Some(current),
            },
            publication,
            next_identity,
        )));
        self.complete = true;
        self.retryable = true;
        Ok(())
    }

    /// Recover the same journal after a normal refusal or explicit abort.
    /// Cleanup stays in this slot until the enclosing physical release finishes.
    pub(crate) fn recover_original(&mut self) -> DetachedTransactionsBlock {
        assert!(
            self.retryable && !self.released,
            "unwound or released membership is not retry authority"
        );
        if let Some(Phase::Prepared(prepared)) = self.phase.as_ref() {
            prepared.assert_unpublished();
        }
        self.released = true;
        self.complete = false;
        if let Some(writer) = self.writer.take() {
            self.writer_release = Some(writer.into_release());
        }
        match self.phase.take().expect("original membership journal") {
            Phase::Original(original) => original,
            Phase::Prepared(prepared) => {
                let (original, release) = prepared.detach_retaining();
                self.writer_release = Some(release);
                original
            }
        }
    }

    /// Terminal physical release without payload destruction or callbacks.
    pub(crate) fn release_writers(&mut self) {
        self.released = true;
        self.retryable = false;
        self.complete = false;
        if let Some(writer) = self.writer.take() {
            self.writer_release = Some(writer.into_release());
        }
        if let Some(Phase::Prepared(prepared)) = self.phase.as_mut() {
            prepared.block.release_writers();
        }
    }

    /// Transfer a complete owner and its original observation notification.
    pub(crate) fn into_prepared(
        mut self,
    ) -> PreparedDetachedTransactionsBlock<'storage, Installation> {
        assert!(
            self.complete && !self.released,
            "complete original membership preparation"
        );
        let Some(Phase::Prepared(prepared)) = self.phase.take() else {
            unreachable!("checked prepared membership phase");
        };
        self.released = true;
        PreparedDetachedTransactionsBlock {
            prepared,
            installation: self.installation.take().expect("original installation"),
            preflight_release: self.preflight_release.take(),
        }
    }

    pub(super) fn into_cleanup(mut self) -> AbortedTransactions<Installation> {
        assert!(
            self.released && self.phase.is_none(),
            "original membership was recovered"
        );
        AbortedTransactions {
            _installation: self.installation.take(),
            _preflight_release: self.preflight_release.take(),
            _release: self.writer_release.take(),
        }
    }
}

impl<Installation> Drop for DetachedTransactionsPublicationSlot<'_, Installation> {
    fn drop(&mut self) {
        self.release_writers();
    }
}
