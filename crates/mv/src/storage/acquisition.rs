//! Caller-owned ordinary map-pair acquisition before any constructor work.

use super::*;
use concread::{
    bptree::{BptreeMapAbandonment, BptreeMapWriterAcquisition},
    release::DeferredReleaseBatch,
};

/// Original ordinary storage slot, owning partial construction through unwind.
/// Construct every slot of an aggregate before initializing any of them.
#[must_use = "initialize or abandon the original acquisition slot"]
pub struct BlockAcquisitionSlot<'a, K: Key, V: Value> {
    target: &'a Storage<K, V>,
    phase: AcquisitionPhase<'a, K, V>,
    started: bool,
    complete: bool,
    // Last: recorded native notifications survive payload/charge destruction.
    undo_release: DeferredReleaseBatch,
    current_release: DeferredReleaseBatch,
}

// A role owns only its current physical phase. Raw, converted and retired
// handles must not occupy simultaneous stack slots in every World field.
enum WriterPhase<'a, K: Key, V: Value> {
    Empty,
    Raw(ReleaseGuard<'a, BptreeMapWriterAcquisition<'a, K, V>>),
    Writer(ReleaseGuard<'a, BptreeMapWriteTxn<'a, K, V>>),
    Retired(BptreeMapAbandonment<K, V>),
}

impl<'a, K: Key, V: Value> WriterPhase<'a, K, V> {
    fn is_poisoned(&self) -> bool {
        match self {
            Self::Raw(raw) => raw.is_poisoned(),
            _ => panic!("original raw writer"),
        }
    }

    fn take_raw(&mut self) -> ReleaseGuard<'a, BptreeMapWriterAcquisition<'a, K, V>> {
        match std::mem::replace(self, Self::Empty) {
            Self::Raw(raw) => raw,
            other => {
                *self = other;
                panic!("original raw writer")
            }
        }
    }

    fn take_writer(&mut self) -> ReleaseGuard<'a, BptreeMapWriteTxn<'a, K, V>> {
        match std::mem::replace(self, Self::Empty) {
            Self::Writer(writer) => writer,
            other => {
                *self = other;
                panic!("original converted writer")
            }
        }
    }

    fn release(&mut self, target: &BptreeMap<K, V>, releases: &mut DeferredReleaseBatch) {
        match std::mem::replace(self, Self::Empty) {
            Self::Empty => {}
            Self::Raw(raw) => {
                raw.try_release_into_observed(releases, drop, || target.is_poisoned())
                    .unwrap_or_else(|_| unreachable!("original raw release source"));
            }
            Self::Writer(writer) => {
                let retirement = writer
                    .try_release_into_observed(
                        releases,
                        |writer| writer.abort_retaining(),
                        || target.is_poisoned(),
                    )
                    .unwrap_or_else(|_| unreachable!("original converted release source"));
                *self = Self::Retired(retirement);
            }
            Self::Retired(retirement) => *self = Self::Retired(retirement),
        }
    }
}

// Once both converted writers move into Block, the earlier phase storage is
// reused. The block is installed here before any reset/replacement payload code.
enum AcquisitionPhase<'a, K: Key, V: Value> {
    Empty,
    Pending {
        undo: WriterPhase<'a, K, Option<V>>,
        current: WriterPhase<'a, K, V>,
    },
    Block(Block<'a, K, V>),
}

impl<'a, K: Key, V: Value> BlockAcquisitionSlot<'a, K, V> {
    pub(super) fn new(target: &'a Storage<K, V>) -> Self {
        Self {
            target,
            phase: AcquisitionPhase::Pending {
                undo: WriterPhase::Empty,
                current: WriterPhase::Empty,
            },
            started: false,
            complete: false,
            undo_release: target.revert_released.deferred_batch(),
            current_release: target.blocks_released.deferred_batch(),
        }
    }
}

impl<'a, K: Key, V: Value> crate::BlockAcquisition for BlockAcquisitionSlot<'a, K, V> {
    type Block = Block<'a, K, V>;

    fn initialize(&mut self, mode: BlockMode) {
        assert!(!self.started, "original storage acquisition is one-shot");
        self.started = true;
        let target = self.target;
        let AcquisitionPhase::Pending { undo, current } = &mut self.phase else {
            panic!("original pending acquisition");
        };
        *undo = WriterPhase::Raw(
            target
                .revert_released
                .poisoning_guard(target.revert.acquire_writer()),
        );
        assert!(!undo.is_poisoned(), "original undo writer is poisoned");
        *current = WriterPhase::Raw(
            target
                .blocks_released
                .poisoning_guard(target.blocks.acquire_writer()),
        );
        assert!(
            !current.is_poisoned(),
            "original storage writer is poisoned"
        );
        let raw = undo.take_raw();
        *undo = WriterPhase::Writer(
            match raw
                .try_map_preserving_release_into(
                    &mut self.undo_release,
                    |undo| Ok::<_, (_, std::convert::Infallible)>(undo.write()),
                    || target.revert.is_poisoned(),
                )
                .unwrap_or_else(|_| unreachable!("original undo release source"))
            {
                Ok(writer) => writer,
                Err((_, never)) => match never {},
            },
        );
        let raw = current.take_raw();
        *current = WriterPhase::Writer(
            match raw
                .try_map_preserving_release_into(
                    &mut self.current_release,
                    |current| Ok::<_, (_, std::convert::Infallible)>(current.write()),
                    || target.blocks.is_poisoned(),
                )
                .unwrap_or_else(|_| unreachable!("original current release source"))
            {
                Ok(writer) => writer,
                Err((_, never)) => match never {},
            },
        );
        // The caller slot owns completed writers before identity lookup and all
        // reset/replacement operations that may invoke arbitrary payload code.
        let predecessor = target.publication.capture();
        let writers = StorageWriters::new(target, undo.take_writer(), current.take_writer());
        self.phase = AcquisitionPhase::Block(Block::new(
            writers,
            mode == BlockMode::Replace,
            predecessor,
            mode,
        ));
        let AcquisitionPhase::Block(block) = &mut self.phase else {
            unreachable!("original storage block");
        };
        block.failed = true;
        let OriginalWriters { revert, blocks } = block.writers.as_mut();
        if mode == BlockMode::Replace {
            for (key, value) in revert.iter() {
                match value {
                    None => blocks.remove(key),
                    Some(value) => blocks.insert(key.clone(), value.clone()),
                };
            }
        }
        revert.clear();
        block.failed = false;
        self.complete = true;
    }

    fn release(&mut self) {
        self.complete = false;
        self.started = true;
        match &mut self.phase {
            AcquisitionPhase::Empty => {}
            AcquisitionPhase::Block(block) => crate::BlockRetirement::release_writers(block),
            AcquisitionPhase::Pending { undo, current } => {
                current.release(&self.target.blocks, &mut self.current_release);
                undo.release(&self.target.revert, &mut self.undo_release);
            }
        }
    }

    fn into_block(mut self) -> Self::Block {
        assert!(
            self.complete,
            "original storage initialization did not complete"
        );
        match std::mem::replace(&mut self.phase, AcquisitionPhase::Empty) {
            AcquisitionPhase::Block(block) => block,
            other => {
                self.phase = other;
                panic!("original completed storage block")
            }
        }
    }
}

impl<K: Key, V: Value> Drop for BlockAcquisitionSlot<'_, K, V> {
    fn drop(&mut self) {
        crate::BlockAcquisition::release(self);
    }
}
