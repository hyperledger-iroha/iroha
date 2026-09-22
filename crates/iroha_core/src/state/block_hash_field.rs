//! Retain the exact funded hash successor through direct State publication.

use super::*;
use concread::{
    bptree::{
        BptreeMapAbandonment, BptreeMapCommitRetirement, BptreeMapCommitSlot,
        BptreeMapOwnedAcquisition, BptreeMapPublished, OwnedWriteError,
    },
    release::{DeferredRelease, ReleaseGuard},
};
use std::ops::{Deref, DerefMut};

type Acquired<'a> =
    ReleaseGuard<'a, BptreeMapOwnedAcquisition<'a, usize, HashOf<BlockHeader>, BlockHashMode>>;
type Preparing<'a> =
    ReleaseGuard<'a, BptreeMapCommitSlot<'a, usize, HashOf<BlockHeader>, BlockHashMode>>;
type Published<'a> =
    ReleaseGuard<'a, BptreeMapPublished<'a, usize, HashOf<BlockHeader>, BlockHashMode>>;
type Abandoned = BptreeMapAbandonment<usize, HashOf<BlockHeader>, BlockHashMode>;
type Retired = BptreeMapCommitRetirement<usize, HashOf<BlockHeader>, BlockHashMode>;

enum Phase<'a> {
    Executing(BlockHashesBlock<'a>),
    Acquired(Acquired<'a>),
    Preparing(Preparing<'a>),
    Published(Published<'a>),
    Abandoned {
        _work: Abandoned,
        _reader: Option<DeferredRelease>,
        _writer: DeferredRelease,
    },
    Rejected {
        _work: BlockHashWork,
        _writer: DeferredRelease,
    },
    Retired {
        _work: Retired,
        _writer: DeferredRelease,
    },
}

/// The original hash field, retaining exact publication and retirement custody.
/// Execution access ends permanently when preparation begins.
pub struct BlockHashField<'a> {
    phase: Option<Phase<'a>>,
    target: &'a BlockHashes,
    height: usize,
    attempted: bool,
    released: bool,
}
impl<'a> BlockHashField<'a> {
    pub(crate) fn new(block: BlockHashesBlock<'a>) -> Self {
        Self {
            target: block.inner,
            phase: Some(Phase::Executing(block)),
            height: 0,
            attempted: false,
            released: false,
        }
    }
    pub(crate) fn into_executing(mut self) -> BlockHashesBlock<'a> {
        self.executing();
        match self.phase.take() {
            Some(Phase::Executing(block)) => block,
            _ => unreachable!("checked original hash execution"),
        }
    }
    fn executing(&self) -> &BlockHashesBlock<'a> {
        assert!(
            !self.attempted && !self.released,
            "hash execution authority ended"
        );
        match self.phase.as_ref() {
            Some(Phase::Executing(block)) => block,
            _ => unreachable!("original hash execution phase"),
        }
    }
    /// Reacquire only the exact funded predecessor, retaining every acquired role.
    pub(crate) fn try_prepare_publication(
        &mut self,
    ) -> Result<(), mv::PublicationPreparationError<std::convert::Infallible>> {
        self.executing();
        self.attempted = true;
        let Some(Phase::Executing(block)) = self.phase.as_ref() else {
            unreachable!()
        };
        if block.reserved_tip.is_some() {
            return Err(mv::PublicationPreparationError::Changed);
        }
        let Some(map) = self.target.map() else {
            return Err(mv::PublicationPreparationError::Changed);
        };
        // This validates cursor operability before its original owner is moved.
        // No edit occurs between this check and native acquisition.
        self.height = block.len();
        // No advisory reader acquisition: exact family/base validation happens
        // under the original writer, then reader custody stays in this slot.
        let Some(Phase::Executing(block)) = self.phase.take() else {
            unreachable!()
        };
        let BlockHashesBlock {
            inner,
            work,
            mode,
            visible_len,
            reserved_tip,
            fixture_edits,
        } = block;
        let wait = self.target.released.observe();
        let acquired = match map.try_acquire_owned(work) {
            Ok(acquired) => self.target.released.poisoning_guard(acquired),
            Err((work, error)) => {
                self.phase = Some(Phase::Executing(BlockHashesBlock {
                    inner,
                    work,
                    mode,
                    visible_len,
                    reserved_tip,
                    fixture_edits,
                }));
                return Err(Self::refusal(error, wait));
            }
        };
        // Install the actual acquired writer before inspecting predecessor state.
        self.phase = Some(Phase::Acquired(acquired));
        let Some(Phase::Acquired(acquired)) = self.phase.take() else {
            unreachable!()
        };
        // Native validation checks only poison and pointer identity, then makes
        // inert moves. It neither allocates nor invokes cursor/user code.
        let writer = match acquired.try_map_preserving_release(|a| a.validate()) {
            Ok(writer) => writer,
            Err((acquired, error)) => {
                self.phase = Some(Phase::Acquired(acquired));
                return Err(Self::refusal(error, wait));
            }
        };
        self.phase = Some(Phase::Preparing(
            writer.map_preserving_release(|w| w.commit_slot()),
        ));
        let Some(Phase::Preparing(slot)) = self.phase.as_mut() else {
            unreachable!()
        };
        let wait = map.observe_reader_release();
        // Reader acquisition and cursor validation can unwind. Their exact
        // originals stay in this field throughout that borrowed operation.
        slot.try_prepare()
            .map_err(|error| Self::refusal(error, wait))
    }
    fn refusal(
        error: OwnedWriteError,
        wait: concread::release::ReleaseWait,
    ) -> mv::PublicationPreparationError<std::convert::Infallible> {
        match error {
            OwnedWriteError::Changed => mv::PublicationPreparationError::Changed,
            OwnedWriteError::Poisoned => mv::PublicationPreparationError::Poisoned,
            OwnedWriteError::Busy => {
                mv::PublicationPreparationError::after_failed_acquisition(wait)
            }
        }
    }
    pub(crate) fn publish_prepared(&mut self) {
        assert!(!self.released, "hash field was terminally released");
        assert!(
            matches!(self.phase.as_ref(), Some(Phase::Preparing(slot)) if slot.is_prepared()),
            "original hash preparation must complete"
        );
        let Some(Phase::Preparing(slot)) = self.phase.take() else {
            unreachable!()
        };
        // Checked native publication moves the existing prepaid shell; no
        // allocation, callback, fallible work or payload destruction occurs.
        self.phase = Some(Phase::Published(
            slot.map_preserving_release(|s| s.into_prepared().publish()),
        ));
        self.target
            .committed_height
            .store(self.height, Ordering::Release);
        // Live post-publication cache work may read this map. Unlock now while
        // retaining every original retirement and notification in this field.
        mv::BlockRetirement::release_writers(self);
    }
}
impl<'a> Deref for BlockHashField<'a> {
    type Target = BlockHashesBlock<'a>;
    fn deref(&self) -> &Self::Target {
        self.executing()
    }
}
impl DerefMut for BlockHashField<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.executing();
        match self.phase.as_mut() {
            Some(Phase::Executing(block)) => block,
            _ => unreachable!(),
        }
    }
}
impl mv::BlockRetirement for BlockHashField<'_> {
    fn release_writers(&mut self) {
        self.released = true;
        self.phase = match self.phase.take() {
            Some(Phase::Acquired(acquired)) => {
                let (work, writer) = acquired.release_deferred(|a| a.abort());
                Some(Phase::Rejected {
                    _work: work,
                    _writer: writer,
                })
            }
            Some(Phase::Preparing(slot)) => {
                let ((work, reader), writer) = slot.release_deferred(|s| {
                    let (writer, reader) = s.abort_retaining();
                    (writer.abort_retaining(), reader)
                });
                Some(Phase::Abandoned {
                    _work: work,
                    _reader: reader,
                    _writer: writer,
                })
            }
            Some(Phase::Published(published)) => {
                let (work, writer) = published.release_deferred(|p| p.release());
                Some(Phase::Retired {
                    _work: work,
                    _writer: writer,
                })
            }
            other => other,
        };
    }
}
impl Drop for BlockHashField<'_> {
    fn drop(&mut self) {
        mv::BlockRetirement::release_writers(self);
    }
}

#[cfg(test)]
#[path = "block_hash_field_tests.rs"]
mod tests;
