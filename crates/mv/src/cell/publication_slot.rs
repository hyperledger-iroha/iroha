//! Original Cell publication authority retained through aggregate preparation.

use super::*;

/// Caller-owned original Cell and its mutually exclusive publication phases.
/// Only an owned Block creates this slot; borrowed execution callbacks cannot
/// publish their caller's Block. Creation performs no check or allocation.
#[must_use = "prepare and publish, or jointly release the original slot"]
pub struct BlockPublicationSlot<'a, V: Value, C: Send + Sync + 'static = Untracked> {
    block: Block<'a, V, C>,
}

impl<'a, V: Value, C: Send + Sync + 'static> Block<'a, V, C> {
    /// Move the original Block into its caller's aggregate publication owner.
    /// Install every sibling slot before invoking preparation on any slot.
    pub fn publication_slot(self) -> BlockPublicationSlot<'a, V, C> {
        BlockPublicationSlot { block: self }
    }
}

impl<V: Value, C: Send + Sync + 'static> crate::BlockPublication
    for BlockPublicationSlot<'_, V, C>
{
    fn prepare_publication(&mut self) {
        self.block.prepare_attached_publication();
    }

    fn publish_prepared(&mut self) {
        self.block.publish_attached_prepared();
    }
}

impl<V: Value, C: Send + Sync + 'static> crate::BlockRetirement for BlockPublicationSlot<'_, V, C> {
    fn release_writers(&mut self) {
        crate::BlockRetirement::release_writers(&mut self.block);
    }
}
