//! Original Storage publication authority retained through aggregate preparation.

use super::*;

/// Caller-owned original map Block and its exclusive publication phases.
/// Consuming construction prevents a borrowed prepaid operation callback from
/// publishing before its enclosing HRTB operation accepts the complete result.
#[must_use = "prepare and publish, or jointly release the original slot"]
pub struct BlockPublicationSlot<'a, K: Key, V: Value, M: StorageMode<K, V> = Untracked> {
    block: Block<'a, K, V, M>,
}

impl<'a, K: Key, V: Value, M: StorageMode<K, V>> Block<'a, K, V, M> {
    /// Move the exact original Block into an inert caller-owned publication slot.
    /// No writer is released, cloned or reacquired during this transition.
    pub fn publication_slot(self) -> BlockPublicationSlot<'a, K, V, M> {
        BlockPublicationSlot { block: self }
    }
}

impl<K: Key, V: Value, M: StorageMode<K, V>> crate::BlockPublication
    for BlockPublicationSlot<'_, K, V, M>
{
    fn prepare_publication(&mut self) {
        self.block.prepare_attached_publication();
    }

    fn publish_prepared(&mut self) {
        self.block.publish_attached_prepared();
    }
}

impl<K: Key, V: Value, M: StorageMode<K, V>> crate::BlockRetirement
    for BlockPublicationSlot<'_, K, V, M>
{
    fn release_writers(&mut self) {
        crate::BlockRetirement::release_writers(&mut self.block);
    }
}
