//! Exact original map capture retained through every enclosing field.

use super::*;
use crate::{BlockCapture, BlockRetirement, CaptureCleanup};

/// Caller-owned capture of the exact current/undo maps and publication metadata.
/// Its attached and detached phases share stack storage; capture makes no copies.
#[must_use = "capture or abandon the original block through its aggregate"]
pub struct BlockCaptureSlot<'a, K: Key, V: Value, Admission, M: StorageMode<K, V> = Untracked> {
    phase: CapturePhase<'a, K, V, Admission, M>,
    started: bool,
    cleanup: CaptureCleanup,
}

enum CapturePhase<'a, K: Key, V: Value, Admission, M: StorageMode<K, V>> {
    Empty,
    Attached {
        block: Block<'a, K, V, M>,
        admission: Option<Admission>,
    },
    Captured(Detached<K, V, Admission, M>),
}

impl<'a, K: Key, V: Value, M: StorageMode<K, V>> Block<'a, K, V, M> {
    /// Move this exact block into a caller-owned capture slot without checks.
    /// The caller installs every sibling slot before starting capture.
    pub fn capture_slot<Admission>(self) -> BlockCaptureSlot<'a, K, V, Admission, M> {
        BlockCaptureSlot {
            phase: CapturePhase::Attached {
                block: self,
                admission: None,
            },
            started: false,
            cleanup: CaptureCleanup::default(),
        }
    }
}

impl<'a, K: Key, V: Value, Admission, M: StorageMode<K, V>>
    BlockCaptureSlot<'a, K, V, Admission, M>
{
    // The prepaid HRTB executor already owns its returned admission. Store it
    // before checking cursor operability, so panic cannot refund it under locks.
    pub(super) fn capture_admitted(&mut self, admission: Admission) {
        assert!(!self.started, "original map capture is one-shot");
        self.started = true;
        let CapturePhase::Attached {
            block,
            admission: retained,
        } = &mut self.phase
        else {
            panic!("original attached map capture");
        };
        *retained = Some(admission);
        block.assert_operable();
        self.finish_capture();
    }

    fn finish_capture(&mut self) {
        let CapturePhase::Attached {
            block,
            admission: Some(admission),
        } = std::mem::replace(&mut self.phase, CapturePhase::Empty)
        else {
            unreachable!("original checked map block");
        };
        let Block {
            writers,
            dirty,
            failed: _,
            predecessor,
            next,
            mode,
        } = block;
        let OriginalWriters { revert, blocks } = writers.into_original();
        let (blocks, revert, cleanup) = detach_pair_retaining(blocks, revert);
        self.phase = CapturePhase::Captured(Detached {
            revert,
            blocks,
            metadata: DetachedMetadata {
                predecessor,
                mode,
                dirty,
                next,
                admission,
            },
        });
        self.cleanup = cleanup;
    }
}

impl<'a, K: Key, V: Value, Admission, M: StorageMode<K, V>> BlockCapture<Admission>
    for BlockCaptureSlot<'a, K, V, Admission, M>
{
    type Block = Block<'a, K, V, M>;
    type Detached = Detached<K, V, Admission, M>;

    fn try_capture<E>(
        &mut self,
        admit: impl FnOnce(&Self::Block) -> Result<Admission, E>,
    ) -> Result<(), E> {
        assert!(!self.started, "original map capture is one-shot");
        self.started = true;
        let CapturePhase::Attached {
            block,
            admission: retained,
        } = &mut self.phase
        else {
            panic!("original attached map capture");
        };
        // Both logical cursor verdicts run before taking the original Block.
        // A caught panic leaves failed private roots owned here for abandonment.
        block.assert_operable();
        *retained = Some(admit(block)?);
        self.finish_capture();
        Ok(())
    }

    fn release(&mut self) {
        self.started = true;
        if let CapturePhase::Attached { block, .. } = &mut self.phase {
            block.release_writers();
        }
    }

    fn into_detached(mut self) -> (Self::Detached, CaptureCleanup) {
        match std::mem::replace(&mut self.phase, CapturePhase::Empty) {
            CapturePhase::Captured(journal) => (journal, std::mem::take(&mut self.cleanup)),
            original => {
                self.phase = original;
                panic!("original map capture did not complete");
            }
        }
    }
}

impl<K: Key, V: Value, Admission, M: StorageMode<K, V>> Drop
    for BlockCaptureSlot<'_, K, V, Admission, M>
{
    fn drop(&mut self) {
        self.release();
    }
}
