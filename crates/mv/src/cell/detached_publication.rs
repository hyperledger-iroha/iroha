//! Caller-owned reacquisition of the exact retained EBR pair.
use super::*;
use concread::{ebrcell::EbrCellWriterAcquisition, release::DeferredReleaseBatch};

enum Role<'a, V: Value, C: Send + Sync + 'static> {
    Owned(EbrCellOwned<V, C>),
    Acquired(
        ReleaseGuard<'a, EbrCellWriterAcquisition<'a, V, C>>,
        EbrCellOwned<V, C>,
    ),
    Writer(CellWriter<'a, V, C>),
    Empty,
}
impl<'a, V: Value, C: Send + Sync + 'static> Role<'a, V, C> {
    fn acquire<E>(
        &mut self,
        target: &'a EbrCell<V, C>,
        released: &'a ReleaseNotification,
        batch: &mut DeferredReleaseBatch,
    ) -> Result<(), PublicationPreparationError<E>> {
        assert!(
            matches!(self, Self::Owned(_)),
            "original unattached generation"
        );
        let wait = released.observe();
        let Some(raw) = target.try_acquire_writer() else {
            return Err(PublicationPreparationError::after_failed_acquisition(wait));
        };
        let Self::Owned(owned) = std::mem::replace(self, Self::Empty) else {
            unreachable!()
        };
        *self = Self::Acquired(released.poisoning_guard(raw), owned);
        let Self::Acquired(raw, _) = self else {
            unreachable!()
        };
        if raw.is_poisoned() {
            return Err(PublicationPreparationError::Poisoned);
        }
        // Native attach performs poison checking and inert moves only. The raw
        // owner is already installed before the check; no clone or callback runs.
        let Self::Acquired(raw, owned) = std::mem::replace(self, Self::Empty) else {
            unreachable!()
        };
        match raw
            .try_map_preserving_release_into(
                batch,
                |raw| raw.try_write_owned(owned),
                || target.is_poisoned(),
            )
            .unwrap_or_else(|_| unreachable!("original cell acquisition release source"))
        {
            Ok(writer) => {
                *self = Self::Writer(writer);
                Ok(())
            }
            Err((raw, owned)) => {
                *self = Self::Acquired(raw, owned);
                Err(PublicationPreparationError::Poisoned)
            }
        }
    }

    fn release(&mut self, target: &EbrCell<V, C>, batch: &mut DeferredReleaseBatch) {
        let owned = match std::mem::replace(self, Self::Empty) {
            Self::Acquired(raw, owned) => {
                raw.try_release_into_observed(batch, drop, || target.is_poisoned())
                    .unwrap_or_else(|_| unreachable!("original acquisition release source"));
                owned
            }
            Self::Writer(writer) => writer
                .try_release_into_observed(batch, |writer| writer.detach(), || target.is_poisoned())
                .unwrap_or_else(|_| unreachable!("original writer release source")),
            other => {
                *self = other;
                return;
            }
        };
        *self = Self::Owned(owned);
    }
    fn into_owned(
        mut self,
        target: &EbrCell<V, C>,
        batch: &mut DeferredReleaseBatch,
    ) -> EbrCellOwned<V, C> {
        self.release(target, batch);
        let Self::Owned(owned) = self else {
            panic!("original detached generation")
        };
        owned
    }
    fn into_writer(self) -> CellWriter<'a, V, C> {
        let Self::Writer(writer) = self else {
            panic!("original acquired generation")
        };
        writer
    }
}

enum Phase<'a, V: Value, A, I, C: Send + Sync + 'static> {
    Original(Detached<V, A, C>),
    Acquiring {
        revert: Role<'a, Option<V>, C>,
        blocks: Role<'a, V, C>,
        metadata: DetachedMetadata<A>,
    },
    Prepared(PreparedPublication<'a, V, A, I, C>),
    Empty,
}

/// One original detached pair installed in its aggregate before any preparation.
/// Normal refusal can return the exact journal. Caught unwind and terminal
/// release permit cleanup only. This grants no State or finality authority.
#[must_use = "retain the original slot until every enclosing writer releases"]
pub struct DetachedPublicationSlot<'a, V: Value, A, I, C: Send + Sync + 'static = Untracked> {
    target: &'a Cell<V, C>,
    phase: Phase<'a, V, A, I, C>,
    attempted: bool,
    complete: bool,
    retryable: bool,
    released: bool,
    cleanup: PublicationCleanup<I>,
}
impl<'a, V: Value, A, I, C: Send + Sync + 'static> DetachedPublicationSlot<'a, V, A, I, C> {
    pub(super) fn new(original: Detached<V, A, C>, target: &'a Cell<V, C>) -> Self {
        let mut cleanup = PublicationCleanup::empty();
        cleanup.writer_batches = [
            Some(target.blocks_released.deferred_batch()),
            Some(target.revert_released.deferred_batch()),
        ];
        Self {
            target,
            phase: Phase::Original(original),
            attempted: false,
            complete: false,
            retryable: true,
            released: false,
            cleanup,
        }
    }
    /// Borrow the original journal while its caller retains all siblings.
    pub fn try_prepare<E>(
        &mut self,
        admit: impl FnOnce(&Detached<V, A, C>, &Cell<V, C>) -> Result<I, E>,
    ) -> Result<(), PublicationPreparationError<E>> {
        assert!(
            !self.attempted && !self.released,
            "original preparation is one-shot"
        );
        self.attempted = true;
        self.retryable = false;
        let result = self.prepare_inner(admit);
        // This is intentionally after the callee: unwind cannot grant retry.
        self.retryable = true;
        self.complete = result.is_ok();
        result
    }
    fn prepare_inner<E>(
        &mut self,
        admit: impl FnOnce(&Detached<V, A, C>, &Cell<V, C>) -> Result<I, E>,
    ) -> Result<(), PublicationPreparationError<E>> {
        let Phase::Original(original) = &self.phase else {
            panic!("original journal before preparation")
        };
        let (checked, probe) = original
            .metadata
            .predecessor
            .try_check_current(&self.target.publication);
        self.cleanup.identities[0] = probe;
        checked?;
        self.cleanup.installation =
            Some(admit(original, self.target).map_err(PublicationPreparationError::Admission)?);
        let Phase::Original(Detached {
            revert,
            blocks,
            metadata,
        }) = std::mem::replace(&mut self.phase, Phase::Empty)
        else {
            unreachable!()
        };
        self.phase = Phase::Acquiring {
            revert: Role::Owned(revert),
            blocks: Role::Owned(blocks),
            metadata,
        };
        let Phase::Acquiring { revert, blocks, .. } = &mut self.phase else {
            unreachable!()
        };
        revert.acquire(
            &self.target.revert,
            &self.target.revert_released,
            self.cleanup.writer_batches[1]
                .as_mut()
                .expect("original undo release"),
        )?;
        blocks.acquire(
            &self.target.blocks,
            &self.target.blocks_released,
            self.cleanup.writer_batches[0]
                .as_mut()
                .expect("original current release"),
        )?;
        let Phase::Acquiring {
            revert,
            blocks,
            metadata,
        } = std::mem::replace(&mut self.phase, Phase::Empty)
        else {
            unreachable!()
        };
        self.phase = Phase::Prepared(PreparedPublication {
            writers: PreparedCellWriters::new(
                revert.into_writer(),
                blocks.into_writer(),
                self.cleanup.identities[0].take(),
            ),
            metadata,
            installation: self
                .cleanup
                .installation
                .take()
                .expect("original installation"),
        });
        let Phase::Prepared(prepared) = &mut self.phase else {
            unreachable!()
        };
        prepared.writers.prepare(
            self.target,
            &prepared.metadata.predecessor,
            prepared.metadata.dirty,
        )
    }
    /// Unlock every original physical phase and retain payload and callbacks.
    /// This is terminal and cannot promote cleanup to journal authority.
    pub fn release_writers(&mut self) {
        if self.released {
            return;
        }
        self.released = true;
        self.retryable = false;
        self.complete = false;
        match &mut self.phase {
            Phase::Acquiring { revert, blocks, .. } => {
                blocks.release(
                    &self.target.blocks,
                    self.cleanup.writer_batches[0]
                        .as_mut()
                        .expect("original current release"),
                );
                revert.release(
                    &self.target.revert,
                    self.cleanup.writer_batches[1]
                        .as_mut()
                        .expect("original undo release"),
                );
            }
            Phase::Prepared(prepared) => prepared.writers.release(),
            _ => {}
        }
    }
    /// Return the exact original journal after a normal refusal or complete abort.
    /// Actual releases and installation remain in this caller-owned slot.
    pub fn recover_original(&mut self) -> Detached<V, A, C> {
        assert!(
            self.retryable && !self.released,
            "unwound/released preparation grants no journal"
        );
        self.released = true;
        self.complete = false;
        match std::mem::replace(&mut self.phase, Phase::Empty) {
            Phase::Original(original) => original,
            Phase::Acquiring {
                revert,
                blocks,
                metadata,
            } => Detached {
                blocks: blocks.into_owned(
                    &self.target.blocks,
                    self.cleanup.writer_batches[0]
                        .as_mut()
                        .expect("original current release"),
                ),
                revert: revert.into_owned(
                    &self.target.revert,
                    self.cleanup.writer_batches[1]
                        .as_mut()
                        .expect("original undo release"),
                ),
                metadata,
            },
            Phase::Prepared(prepared) => {
                let (original, cleanup) = prepared.abort();
                self.cleanup = cleanup; // only inactive partial-acquisition batches remain here
                original
            }
            Phase::Empty => panic!("original journal already transferred"),
        }
    }
    /// Move the same fully prepared pair into its terminal publisher.
    pub fn into_prepared(mut self) -> PreparedPublication<'a, V, A, I, C> {
        assert!(
            self.complete && !self.released,
            "complete original pair required"
        );
        self.released = true;
        let Phase::Prepared(prepared) = std::mem::replace(&mut self.phase, Phase::Empty) else {
            unreachable!()
        };
        prepared
    }
    /// Transfer retained cleanup only after normal recovery of the original journal.
    /// Terminally abandoned or unwound slots cannot use this recovery-only transfer.
    pub fn into_cleanup(mut self) -> PublicationCleanup<I> {
        assert!(
            self.retryable && self.released && matches!(self.phase, Phase::Empty),
            "original journal must first be recovered"
        );
        std::mem::replace(&mut self.cleanup, PublicationCleanup::empty())
    }
}
impl<V: Value, A, I, C: Send + Sync + 'static> Drop for DetachedPublicationSlot<'_, V, A, I, C> {
    fn drop(&mut self) {
        self.release_writers();
    }
}
