//! Suspend and resume one original executing map pair; frozen publication stays immutable.

use super::*;
use crate::{BlockResumption, BlockRetirement, BlockSuspension, CaptureCleanup};

struct ExecutionMetadata<Custody> {
    direct_write: bool,
    applied_transactions: u64,
    // Last: original admission scope outlives original roots and physical writers.
    custody: Custody,
}

/// Move-only original mutable-execution custody without physical writer guards.
/// Only consuming an executing Block can construct this capability. It cannot be
/// made from a frozen publication journal or from a committed view.
///
/// An original borrowed refund scope cannot be erased by suspension:
/// ```compile_fail
/// use concread::bptree::{ClonePlanning, Prepaid};
/// use iroha_allocation::AllocationBudget;
/// use mv::{BlockAcquisition, BlockMode, BlockSuspension};
/// use mv::storage::{AdmittedStoragePolicy, Storage, SuspendedBlock};
/// fn escape<'a, P>(budget: &'a AllocationBudget, target: &'a Storage<u64, u64, Prepaid<P>>)
///     -> SuspendedBlock<'a, u64, u64, Prepaid<P>>
/// where P: AdmittedStoragePolicy + ClonePlanning<u64, u64> + ClonePlanning<u64, Option<u64>> {
///     budget.with_deferred_refund_notifications(|scope| {
///         let mut acquisition = target.try_block_acquisition_admitted(scope).unwrap();
///         acquisition.try_initialize(BlockMode::Ordinary).unwrap();
///         let mut slot = acquisition.into_block().suspension_slot();
///         slot.suspend();
///         let (original, cleanup) = slot.into_suspended();
///         drop(cleanup);
///         original // Cannot escape the original borrowed scope.
///     })
/// }
/// ```
/// The same original owner may resume while its borrowed scope is still live:
/// ```
/// use concread::bptree::{ClonePlanning, Prepaid};
/// use iroha_allocation::AllocationBudget;
/// use mv::{BlockAcquisition, BlockMode, BlockResumption, BlockSuspension};
/// use mv::storage::{AdmittedStoragePolicy, Storage};
/// fn confined<P>(budget: &AllocationBudget, target: &Storage<u64, u64, Prepaid<P>>)
/// where P: AdmittedStoragePolicy + ClonePlanning<u64, u64> + ClonePlanning<u64, Option<u64>> {
///     budget.with_deferred_refund_notifications(|scope| {
///         let mut acquisition = target.try_block_acquisition_admitted(scope).unwrap();
///         acquisition.try_initialize(BlockMode::Ordinary).unwrap();
///         let mut slot = acquisition.into_block().suspension_slot();
///         slot.suspend();
///         let (original, cleanup) = slot.into_suspended();
///         drop(cleanup);
///         let mut resumed = original.resumption_slot(target);
///         resumed.try_resume().unwrap();
///         drop(resumed.take_block());
///         drop(resumed);
///     });
/// }
/// ```
/// Retiring every immutable reader permits publication preparation, never execution:
/// ```compile_fail,E0599
/// use mv::storage::{FrozenDetached, Storage};
/// fn cannot_thaw_execution(frozen: FrozenDetached<u64, u64, ()>, target: &Storage<u64, u64>) {
///     let original = frozen.try_into_detached().ok().unwrap();
///     let _mutable = original.resumption_slot(target);
/// }
/// ```
pub struct SuspendedBlock<'a, K: Key, V: Value, M: StorageMode<K, V> = Untracked> {
    original: Detached<K, V, ExecutionMetadata<M::AcquisitionCustody>, M>,
    // Borrowed prepaid acquisition must not escape its original refund-scope lifetime.
    scope_lifetime: core::marker::PhantomData<&'a ()>,
}

#[expect(
    clippy::large_enum_variant,
    reason = "original execution roots and custody move inline without a failure-path allocation"
)]
enum SuspensionPhase<'a, K: Key, V: Value, M: StorageMode<K, V>> {
    Attached(Block<'a, K, V, M>),
    Suspended(SuspendedBlock<'a, K, V, M>),
    Empty,
}

/// Aggregate-owned suspension of the exact original current and undo writer pair.
#[must_use = "jointly suspend or abandon every original aggregate participant"]
pub struct BlockSuspensionSlot<'a, K: Key, V: Value, M: StorageMode<K, V> = Untracked> {
    phase: SuspensionPhase<'a, K, V, M>,
    started: bool,
    cleanup: CaptureCleanup,
}

impl<'a, K: Key, V: Value, M: StorageMode<K, V>> Block<'a, K, V, M> {
    /// Install this original execution owner in an inert suspension slot.
    /// No allocation, validation, callback or writer release occurs here.
    pub fn suspension_slot(self) -> BlockSuspensionSlot<'a, K, V, M> {
        BlockSuspensionSlot {
            phase: SuspensionPhase::Attached(self),
            started: false,
            cleanup: CaptureCleanup::default(),
        }
    }
}

impl<'a, K: Key, V: Value, M: StorageMode<K, V>> BlockSuspension
    for BlockSuspensionSlot<'a, K, V, M>
{
    type Block = Block<'a, K, V, M>;
    type Suspended = SuspendedBlock<'a, K, V, M>;
    fn suspend(&mut self) {
        assert!(!self.started, "original suspension is one-shot");
        self.started = true;
        let SuspensionPhase::Attached(block) = &self.phase else {
            panic!("original executing map required");
        };
        block.assert_operable();
        let SuspensionPhase::Attached(block) =
            std::mem::replace(&mut self.phase, SuspensionPhase::Empty)
        else {
            unreachable!()
        };
        let Block {
            writers,
            dirty,
            direct_write,
            applied_transactions,
            failed: _,
            predecessor,
            next,
            mode,
            _acquisition_custody,
        } = block;
        let OriginalWriters { revert, blocks } = writers.into_original();
        let (blocks, revert, cleanup) = detach_pair_retaining(blocks, revert);
        self.phase = SuspensionPhase::Suspended(SuspendedBlock {
            original: Detached {
                revert,
                blocks,
                metadata: DetachedMetadata {
                    predecessor,
                    mode,
                    dirty,
                    next: next.expect("original execution successor identity"),
                    admission: ExecutionMetadata {
                        direct_write,
                        applied_transactions,
                        custody: _acquisition_custody,
                    },
                },
            },
            scope_lifetime: core::marker::PhantomData,
        });
        self.cleanup = cleanup;
    }
    fn into_suspended(mut self) -> (Self::Suspended, CaptureCleanup) {
        match std::mem::replace(&mut self.phase, SuspensionPhase::Empty) {
            SuspensionPhase::Suspended(original) => (original, std::mem::take(&mut self.cleanup)),
            original => {
                self.phase = original;
                panic!("original suspension did not complete");
            }
        }
    }
}
impl<K: Key, V: Value, M: StorageMode<K, V>> BlockRetirement for BlockSuspensionSlot<'_, K, V, M> {
    fn release_writers(&mut self) {
        self.started = true;
        if let SuspensionPhase::Attached(block) = &mut self.phase {
            block.release_writers();
        }
    }
}
impl<K: Key, V: Value, M: StorageMode<K, V>> Drop for BlockSuspensionSlot<'_, K, V, M> {
    fn drop(&mut self) {
        self.release_writers();
    }
}

/// Exact original map reacquisition, with all partial phases installed before work.
/// Even after `take_block`, retain this slot until every resumed writer is free;
/// the slot owns original validation reader/identity release notifications.
#[must_use = "retain original resumption and cleanup through aggregate release"]
pub struct BlockResumptionSlot<'a, K: Key, V: Value, M: StorageMode<K, V> = Untracked> {
    original:
        DetachedPublicationSlotInner<'a, K, V, ExecutionMetadata<M::AcquisitionCustody>, (), M>,
    resumed: Option<Block<'a, K, V, M>>,
    // Re-suspending a fully resumed-but-untransferred pair has its own exact notices.
    suspend_cleanup: CaptureCleanup,
    resume_cleanup: Option<PublicationCleanup<()>>,
    attempted: bool,
    recovery_allowed: bool,
    terminal: bool,
}
impl<'scope, K: Key, V: Value, M: StorageMode<K, V>> SuspendedBlock<'scope, K, V, M> {
    /// Install original custody before any physical acquisition or predecessor check.
    /// The original admission scope moves with it; the target cannot replace that scope.
    pub fn resumption_slot<'a>(
        self,
        target: &'a Storage<K, V, M>,
    ) -> BlockResumptionSlot<'a, K, V, M>
    where
        'scope: 'a,
    {
        BlockResumptionSlot {
            original: DetachedPublicationSlotInner::new(self.original, target),
            resumed: None,
            suspend_cleanup: CaptureCleanup::default(),
            resume_cleanup: None,
            attempted: false,
            recovery_allowed: true,
            terminal: false,
        }
    }
}
impl<'a, K: Key, V: Value, M: StorageMode<K, V>> BlockResumption
    for BlockResumptionSlot<'a, K, V, M>
{
    type Block = Block<'a, K, V, M>;
    type Suspended = SuspendedBlock<'a, K, V, M>;
    fn try_resume(&mut self) -> Result<(), PublicationPreparationError<core::convert::Infallible>> {
        assert!(
            !self.terminal && !self.attempted,
            "original resumption is one-shot"
        );
        self.attempted = true;
        self.recovery_allowed = false;
        if let Err(error) = self.original.try_prepare(|_, _| Ok(())) {
            self.recovery_allowed = true;
            return Err(error);
        }
        let PreparedPublication {
            writers,
            metadata,
            installation: (),
        } = self.original.take_prepared();
        let (writers, cleanup) = writers.resume_execution();
        let DetachedMetadata {
            predecessor,
            mode,
            dirty,
            next,
            admission,
        } = metadata;
        self.resumed = Some(Block {
            writers,
            dirty,
            direct_write: admission.direct_write,
            applied_transactions: admission.applied_transactions,
            failed: false,
            predecessor,
            next: Some(next),
            mode,
            _acquisition_custody: admission.custody,
        });
        self.resume_cleanup = Some(cleanup);
        self.recovery_allowed = true;
        Ok(())
    }
    fn is_resumed(&self) -> bool {
        !self.terminal && self.recovery_allowed && self.resumed.is_some()
    }
    fn take_block(&mut self) -> Self::Block {
        assert!(
            self.is_resumed(),
            "complete original execution pair required"
        );
        self.terminal = true;
        self.resumed
            .take()
            .expect("checked original execution pair")
    }
    fn recover_suspended(&mut self) -> Self::Suspended {
        assert!(
            !self.terminal && self.recovery_allowed,
            "unwound/released/transferred owner cannot resume"
        );
        self.terminal = true;
        if let Some(block) = self.resumed.take() {
            let mut slot = block.suspension_slot();
            slot.suspend();
            let (original, cleanup) = slot.into_suspended();
            self.suspend_cleanup = cleanup;
            original
        } else {
            SuspendedBlock {
                original: self.original.recover_original(),
                scope_lifetime: core::marker::PhantomData,
            }
        }
    }
}
impl<K: Key, V: Value, M: StorageMode<K, V>> BlockRetirement for BlockResumptionSlot<'_, K, V, M> {
    fn release_writers(&mut self) {
        self.terminal = true;
        self.original.release_writers();
        if let Some(block) = &mut self.resumed {
            block.release_writers();
        }
    }
}
impl<K: Key, V: Value, M: StorageMode<K, V>> Drop for BlockResumptionSlot<'_, K, V, M> {
    fn drop(&mut self) {
        self.release_writers();
    }
}

#[cfg(test)]
#[path = "suspension/tests.rs"]
mod tests;
