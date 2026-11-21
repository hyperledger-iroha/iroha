//! Distinct mutable-execution suspension over exact original EBR generations.

use super::*;
use crate::{BlockCapture, BlockResumption, BlockRetirement, BlockSuspension, CaptureCleanup};

/// Original execution generations without writers, constructible only by consuming a Block.
/// Ordinary frozen/Detached publication journals cannot become this capability.
///
/// Even after every immutable reader retires, the returned publication journal has
/// no API for regaining mutable execution:
/// ```compile_fail,E0599
/// use mv::cell::{Cell, FrozenDetached};
/// fn cannot_thaw_execution(frozen: FrozenDetached<u64, ()>, target: &Cell<u64>) {
///     let original = frozen.try_into_detached().ok().unwrap();
///     let _mutable = original.resumption_slot(target);
/// }
/// ```
pub struct SuspendedBlock<V: CellValue, C: Send + Sync + 'static = Untracked> {
    original: Detached<V, (), C>,
}

/// Aggregate-owned inert capture of the original mutable Cell execution owner.
#[must_use = "jointly suspend or abandon every original aggregate participant"]
pub struct BlockSuspensionSlot<'a, V: CellValue, C: Send + Sync + 'static = Untracked> {
    original: BlockCaptureSlot<'a, V, (), C>,
}
impl<'a, V: CellValue, C: Send + Sync + 'static> Block<'a, V, C> {
    /// Install exact original custody before any sibling releases a writer.
    pub fn suspension_slot(self) -> BlockSuspensionSlot<'a, V, C> {
        BlockSuspensionSlot {
            original: self.capture_slot(),
        }
    }
}
impl<'a, V: CellValue, C: Send + Sync + 'static> BlockSuspension for BlockSuspensionSlot<'a, V, C> {
    type Block = Block<'a, V, C>;
    type Suspended = SuspendedBlock<V, C>;
    fn suspend(&mut self) {
        match self
            .original
            .try_capture(|_| Ok::<_, core::convert::Infallible>(()))
        {
            Ok(()) => {}
            Err(never) => match never {},
        }
    }
    fn into_suspended(self) -> (Self::Suspended, CaptureCleanup) {
        let (original, cleanup) = self.original.into_detached();
        (SuspendedBlock { original }, cleanup)
    }
}
impl<V: CellValue, C: Send + Sync + 'static> BlockRetirement for BlockSuspensionSlot<'_, V, C> {
    fn release_writers(&mut self) {
        self.original.release();
    }
}

/// Original Cell reactivation slot retaining all acquisition/refund notices inline.
/// Keep an emptied successful slot until every resumed aggregate writer is free.
#[must_use = "retain original resumption and cleanup through aggregate release"]
pub struct BlockResumptionSlot<'a, V: CellValue, C: Send + Sync + 'static = Untracked> {
    original: DetachedPublicationSlot<'a, V, (), (), C>,
    resumed: Option<Block<'a, V, C>>,
    suspend_cleanup: CaptureCleanup,
    resume_cleanup: Option<PublicationCleanup<()>>,
    target: &'a Cell<V, C>,
    attempted: bool,
    recovery_allowed: bool,
    terminal: bool,
}
impl<V: CellValue, C: Send + Sync + 'static> SuspendedBlock<V, C> {
    /// Move original generations and their physical charges into an inert acquisition slot.
    /// Exact target and current/undo predecessor checks precede restored execution authority.
    pub fn resumption_slot<'a>(self, target: &'a Cell<V, C>) -> BlockResumptionSlot<'a, V, C> {
        BlockResumptionSlot {
            original: DetachedPublicationSlot::new(self.original, target),
            resumed: None,
            suspend_cleanup: CaptureCleanup::default(),
            resume_cleanup: None,
            target,
            attempted: false,
            recovery_allowed: true,
            terminal: false,
        }
    }
}
impl<'a, V: CellValue, C: Send + Sync + 'static> BlockResumption for BlockResumptionSlot<'a, V, C> {
    type Block = Block<'a, V, C>;
    type Suspended = SuspendedBlock<V, C>;
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
            admission: (),
        } = metadata;
        self.resumed = Some(Block {
            writers,
            dirty,
            publication: &self.target.publication,
            predecessor,
            mode,
            next: Some(next),
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
            }
        }
    }
}
impl<V: CellValue, C: Send + Sync + 'static> BlockRetirement for BlockResumptionSlot<'_, V, C> {
    fn release_writers(&mut self) {
        self.terminal = true;
        self.original.release_writers();
        if let Some(block) = &mut self.resumed {
            block.release_writers();
        }
    }
}
impl<V: CellValue, C: Send + Sync + 'static> Drop for BlockResumptionSlot<'_, V, C> {
    fn drop(&mut self) {
        self.release_writers();
    }
}

#[cfg(test)]
#[path = "suspension/tests.rs"]
mod tests;
