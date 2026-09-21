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
    raw_revert: Option<ReleaseGuard<'a, BptreeMapWriterAcquisition<'a, K, Option<V>>>>,
    raw_blocks: Option<ReleaseGuard<'a, BptreeMapWriterAcquisition<'a, K, V>>>,
    revert: Option<ReleaseGuard<'a, BptreeMapWriteTxn<'a, K, Option<V>>>>,
    blocks: Option<ReleaseGuard<'a, BptreeMapWriteTxn<'a, K, V>>>,
    undo_retirement: Option<BptreeMapAbandonment<K, Option<V>>>,
    current_retirement: Option<BptreeMapAbandonment<K, V>>,
    block: Option<Block<'a, K, V>>,
    started: bool,
    complete: bool,
    // Last: recorded native notifications survive payload/charge destruction.
    undo_release: DeferredReleaseBatch,
    current_release: DeferredReleaseBatch,
}

impl<'a, K: Key, V: Value> BlockAcquisitionSlot<'a, K, V> {
    pub(super) fn new(target: &'a Storage<K, V>) -> Self {
        Self {
            target,
            raw_revert: None,
            raw_blocks: None,
            revert: None,
            blocks: None,
            undo_retirement: None,
            current_retirement: None,
            block: None,
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
        self.raw_revert = Some(
            target
                .revert_released
                .poisoning_guard(target.revert.acquire_writer()),
        );
        assert!(
            !self
                .raw_revert
                .as_ref()
                .expect("original undo")
                .is_poisoned(),
            "original undo writer is poisoned"
        );
        self.raw_blocks = Some(
            target
                .blocks_released
                .poisoning_guard(target.blocks.acquire_writer()),
        );
        assert!(
            !self
                .raw_blocks
                .as_ref()
                .expect("original current")
                .is_poisoned(),
            "original storage writer is poisoned"
        );
        let undo = self.raw_revert.take().expect("original undo");
        self.revert = Some(
            match undo
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
        let current = self.raw_blocks.take().expect("original current");
        self.blocks = Some(
            match current
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
        self.block = Some(Block::new(
            StorageWriters::new(
                target,
                self.revert.take().expect("original undo writer"),
                self.blocks.take().expect("original current writer"),
            ),
            mode == BlockMode::Replace,
            predecessor,
            mode,
        ));
        let block = self.block.as_mut().expect("original storage block");
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
        if let Some(block) = &mut self.block {
            crate::BlockRetirement::release_writers(block);
        }
        let target = self.target;
        if let Some(writer) = self.blocks.take() {
            self.current_retirement = Some(
                writer
                    .try_release_into_observed(
                        &mut self.current_release,
                        |writer| writer.abort_retaining(),
                        || target.blocks.is_poisoned(),
                    )
                    .unwrap_or_else(|_| unreachable!("original current release source")),
            );
        }
        if let Some(writer) = self.revert.take() {
            self.undo_retirement = Some(
                writer
                    .try_release_into_observed(
                        &mut self.undo_release,
                        |writer| writer.abort_retaining(),
                        || target.revert.is_poisoned(),
                    )
                    .unwrap_or_else(|_| unreachable!("original undo release source")),
            );
        }
        if let Some(current) = self.raw_blocks.take() {
            current
                .try_release_into_observed(&mut self.current_release, drop, || {
                    target.blocks.is_poisoned()
                })
                .unwrap_or_else(|_| unreachable!("original current release source"));
        }
        if let Some(undo) = self.raw_revert.take() {
            undo.try_release_into_observed(&mut self.undo_release, drop, || {
                target.revert.is_poisoned()
            })
            .unwrap_or_else(|_| unreachable!("original undo release source"));
        }
    }

    fn into_block(mut self) -> Self::Block {
        assert!(
            self.complete,
            "original storage initialization did not complete"
        );
        self.block.take().expect("original completed storage block")
    }
}

impl<K: Key, V: Value> Drop for BlockAcquisitionSlot<'_, K, V> {
    fn drop(&mut self) {
        crate::BlockAcquisition::release(self);
    }
}
