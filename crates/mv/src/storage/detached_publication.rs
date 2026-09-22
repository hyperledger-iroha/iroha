//! Caller-owned exact retained map-pair preparation with no successor copies.
use super::*;
use concread::{bptree::BptreeMapOwnedAcquisition, release::DeferredReleaseBatch};

enum Role<'a, K: Key, V: Value, M: MapMode + NodeCloning<K, V>> {
    Owned(BptreeMapOwned<K, V, M>),
    Acquired(ReleaseGuard<'a, BptreeMapOwnedAcquisition<'a, K, V, M>>),
    Writer(ReleaseGuard<'a, BptreeMapWriteTxn<'a, K, V, M>>),
    Abandoned {
        _original: BptreeMapAbandonment<K, V, M>,
    },
    Empty,
}
fn refusal<E>(error: OwnedWriteError, wait: crate::ReleaseWait) -> PublicationPreparationError<E> {
    match error {
        OwnedWriteError::Busy => PublicationPreparationError::after_failed_acquisition(wait),
        OwnedWriteError::Poisoned => PublicationPreparationError::Poisoned,
        OwnedWriteError::Changed => PublicationPreparationError::Changed,
    }
}
impl<'a, K: Key, V: Value, M: MapMode + NodeCloning<K, V>> Role<'a, K, V, M> {
    fn acquire<E>(
        &mut self,
        target: &'a BptreeMap<K, V, M>,
        released: &'a ReleaseNotification,
        batch: &mut DeferredReleaseBatch,
    ) -> Result<(), PublicationPreparationError<E>> {
        assert!(matches!(self, Self::Owned(_)), "original unattached map");
        let wait = released.observe();
        let Self::Owned(owned) = std::mem::replace(self, Self::Empty) else {
            unreachable!()
        };
        // Root comparison and native mutex acquisition contain no user callback,
        // payload destruction or allocation. Refusal returns the exact owner.
        let acquired = match target.try_acquire_owned(owned) {
            Ok(acquired) => released.poisoning_guard(acquired),
            Err((owned, error)) => {
                *self = Self::Owned(owned);
                return Err(refusal(error, wait));
            }
        };
        *self = Self::Acquired(acquired);
        let Self::Acquired(acquired) = std::mem::replace(self, Self::Empty) else {
            unreachable!()
        };
        // Original phase is installed before native poison/base validation; the
        // same-source batch also owns any actual transition-unwind notification.
        let result = acquired
            .try_map_preserving_release_into(
                batch,
                |acquired| acquired.validate(),
                || target.is_poisoned(),
            )
            .unwrap_or_else(|_| unreachable!("original map acquisition release source"));
        match result {
            Ok(writer) => {
                *self = Self::Writer(writer);
                Ok(())
            }
            Err((acquired, error)) => {
                *self = Self::Acquired(acquired);
                Err(refusal(error, wait))
            }
        }
    }
    fn release(&mut self, target: &BptreeMap<K, V, M>, batch: &mut DeferredReleaseBatch) {
        *self = match std::mem::replace(self, Self::Empty) {
            Self::Acquired(acquired) => Self::Owned(
                acquired
                    .try_release_into_observed(batch, |a| a.abort(), || target.is_poisoned())
                    .unwrap_or_else(|_| unreachable!("original acquired map release source")),
            ),
            Self::Writer(writer) => Self::Abandoned {
                _original: writer
                    .try_release_into_observed(
                        batch,
                        |w| w.abort_retaining(),
                        || target.is_poisoned(),
                    )
                    .unwrap_or_else(|_| unreachable!("original map writer release source")),
            },
            other => other,
        };
    }
    fn into_owned(
        self,
        target: &BptreeMap<K, V, M>,
        batch: &mut DeferredReleaseBatch,
    ) -> BptreeMapOwned<K, V, M> {
        match self {
            Self::Owned(original) => original,
            Self::Acquired(acquired) => acquired
                .try_release_into_observed(batch, |a| a.abort(), || target.is_poisoned())
                .unwrap_or_else(|_| unreachable!("original acquired map release source")),
            Self::Writer(writer) => writer
                .try_release_into_observed(batch, |w| w.detach(), || target.is_poisoned())
                .unwrap_or_else(|_| unreachable!("original map writer release source")),
            _ => panic!("terminal map abandonment is not a journal"),
        }
    }
    fn into_writer(self) -> ReleaseGuard<'a, BptreeMapWriteTxn<'a, K, V, M>> {
        let Self::Writer(writer) = self else {
            panic!("original acquired map")
        };
        writer
    }
}

enum Phase<'a, K: Key, V: Value, A, I, M: StorageMode<K, V>> {
    Original(Detached<K, V, A, M>),
    Acquiring {
        revert: Role<'a, K, Option<V>, M>,
        blocks: Role<'a, K, V, M>,
        metadata: DetachedMetadata<A>,
    },
    Prepared(PreparedPublication<'a, K, V, A, I, M>),
    Empty,
}

// Generic custody stays private: public Prepaid entry must retain its original
// AllocationScope instead of exposing an unscoped generic publication owner.
pub(super) struct DetachedPublicationSlotInner<'a, K: Key, V: Value, A, I, M: StorageMode<K, V>> {
    target: &'a Storage<K, V, M>,
    phase: Phase<'a, K, V, A, I, M>,
    attempted: bool,
    complete: bool,
    retryable: bool,
    released: bool,
    cleanup: PublicationCleanup<I>,
}
impl<'a, K: Key, V: Value, A, I, M: StorageMode<K, V>>
    DetachedPublicationSlotInner<'a, K, V, A, I, M>
{
    pub(super) fn new(original: Detached<K, V, A, M>, target: &'a Storage<K, V, M>) -> Self {
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
    pub(super) fn try_prepare<E>(
        &mut self,
        admit: impl FnOnce(&Detached<K, V, A, M>, &Storage<K, V, M>) -> Result<I, E>,
    ) -> Result<(), PublicationPreparationError<E>> {
        assert!(
            !self.attempted && !self.released,
            "original preparation is one-shot"
        );
        self.attempted = true;
        self.retryable = false;
        let result = self.prepare_inner(admit);
        self.retryable = true; // a caught callee panic never reaches this assignment
        self.complete = result.is_ok();
        result
    }
    fn prepare_inner<E>(
        &mut self,
        admit: impl FnOnce(&Detached<K, V, A, M>, &Storage<K, V, M>) -> Result<I, E>,
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
            writers: PreparedStorageWriters::new(
                StorageWriters::new(self.target, revert.into_writer(), blocks.into_writer()),
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
        prepared
            .writers
            .prepare(&prepared.metadata.predecessor, prepared.metadata.dirty)
    }
    pub(super) fn release_writers(&mut self) {
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
    pub(super) fn recover_original(&mut self) -> Detached<K, V, A, M> {
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
                self.cleanup = cleanup;
                original
            }
            Phase::Empty => panic!("original journal already transferred"),
        }
    }
    pub(super) fn into_prepared(mut self) -> PreparedPublication<'a, K, V, A, I, M> {
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
    pub(super) fn into_cleanup(mut self) -> PublicationCleanup<I> {
        assert!(
            self.retryable && self.released && matches!(self.phase, Phase::Empty),
            "original journal must first be recovered"
        );
        std::mem::replace(&mut self.cleanup, PublicationCleanup::empty())
    }
}
impl<K: Key, V: Value, A, I, M: StorageMode<K, V>> Drop
    for DetachedPublicationSlotInner<'_, K, V, A, I, M>
{
    fn drop(&mut self) {
        self.release_writers();
    }
}

/// Caller-owned original Untracked journal, before any physical reacquisition.
/// Installation admission is separate from execution funding. Prepaid storage
/// has a distinct scope-borrowing entry and cannot use this constructor.
#[must_use = "retain the original slot through every enclosing physical owner"]
pub struct DetachedPublicationSlot<'a, K: Key, V: Value, A, I> {
    pub(super) inner: DetachedPublicationSlotInner<'a, K, V, A, I, Untracked>,
}
impl<'a, K: Key, V: Value, A, I> DetachedPublicationSlot<'a, K, V, A, I> {
    /// Prepare by borrowing the original journal in this caller-owned slot.
    pub fn try_prepare<E>(
        &mut self,
        admit: impl FnOnce(&Detached<K, V, A>, &Storage<K, V>) -> Result<I, E>,
    ) -> Result<(), PublicationPreparationError<E>> {
        self.inner.try_prepare(admit)
    }
    /// Terminally unlock without destroying payloads or invoking callbacks.
    pub fn release_writers(&mut self) {
        self.inner.release_writers();
    }
    /// Recover exact original journals after normal refusal, retaining cleanup.
    pub fn recover_original(&mut self) -> Detached<K, V, A> {
        self.inner.recover_original()
    }
    /// Transfer retained cleanup only after normal recovery of the original journal.
    /// Terminally abandoned or unwound slots cannot use this recovery-only transfer.
    pub fn into_cleanup(self) -> PublicationCleanup<I> {
        self.inner.into_cleanup()
    }
    /// Inertly transfer the complete original pair to its terminal publisher.
    pub fn into_prepared(self) -> PreparedPublication<'a, K, V, A, I> {
        self.inner.into_prepared()
    }
}
