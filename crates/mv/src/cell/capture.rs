//! Retain an original cell block through capture checks and admission.

use super::*;
use crate::{BlockCapture, BlockRetirement, CaptureCleanup};

/// Caller-owned cell capture; phases reuse the same stack storage.
#[must_use = "capture or abandon the original block through its aggregate"]
pub struct BlockCaptureSlot<'a, V: Value, Admission, C: Send + Sync + 'static = Untracked> {
    phase: CapturePhase<'a, V, Admission, C>,
    started: bool,
    // Last: captured payloads and admission outlive all physical writers and
    // are destroyed before these original notifications on abandonment.
    cleanup: CaptureCleanup,
}

enum CapturePhase<'a, V: Value, Admission, C: Send + Sync + 'static> {
    Empty,
    Attached {
        block: Block<'a, V, C>,
        admission: Option<Admission>,
    },
    Captured(Detached<V, Admission, C>),
}

impl<'a, V: Value, C: Send + Sync + 'static> Block<'a, V, C> {
    /// Put the exact attached block in its caller's capture aggregate.
    /// No check, admission, allocation or physical release happens here.
    pub fn capture_slot<Admission>(self) -> BlockCaptureSlot<'a, V, Admission, C> {
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

impl<'a, V: Value, Admission, C: Send + Sync + 'static> BlockCapture<Admission>
    for BlockCaptureSlot<'a, V, Admission, C>
{
    type Block = Block<'a, V, C>;
    type Detached = Detached<V, Admission, C>;

    fn try_capture<E>(
        &mut self,
        admit: impl FnOnce(&Self::Block) -> Result<Admission, E>,
    ) -> Result<(), E> {
        assert!(!self.started, "original cell capture is one-shot");
        self.started = true;
        let CapturePhase::Attached {
            block,
            admission: retained,
        } = &mut self.phase
        else {
            panic!("original attached cell capture");
        };
        // Released cleanup-only blocks must never regain journal authority.
        block.writers.as_ref();
        *retained = Some(admit(block)?);
        let next = NextPublication::new();
        // From here only infallible original-owner moves/native unlocks occur.
        // Any future fallible/user operation belongs above this extraction.
        let CapturePhase::Attached {
            block,
            admission: Some(admission),
        } = std::mem::replace(&mut self.phase, CapturePhase::Empty)
        else {
            unreachable!("original checked cell block");
        };
        let Block {
            writers,
            dirty,
            predecessor,
            mode,
            publication: _,
        } = block;
        let (revert, blocks, cleanup) = writers.detach_retaining();
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
                panic!("original cell capture did not complete");
            }
        }
    }
}

impl<V: Value, Admission, C: Send + Sync + 'static> Drop for BlockCaptureSlot<'_, V, Admission, C> {
    fn drop(&mut self) {
        self.release();
    }
}
