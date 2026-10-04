//! Read the frozen original cell without rebuilding or acquiring a reader.
//!
//! These observations borrow the exact private generations inside `Detached`.
//! They grant no mutation or publication authority: installation must still
//! authenticate its original owner, predecessor, mode and allocation custody.

use super::*;

impl<V: Value, A, C: Send + Sync + 'static> Detached<V, A, C> {
    /// Borrow the exact original undo image without acquiring a current reader.
    /// For optional values, absent undo and an original `None` remain distinct.
    /// This grants no execution or publication authority.
    pub fn original_undo(&self) -> &Option<V> {
        &self.revert
    }

    /// Borrow the original private successor while its physical writers are free.
    ///
    /// No current State view is opened and no payload is cloned. Even after the
    /// target publishes another generation, this remains the frozen successor;
    /// only exact-source preparation decides whether it can still be installed.
    pub fn get(&self) -> &V {
        &self.blocks
    }

    /// Borrow the value before the original block's first mutable access.
    ///
    /// An untouched block reads its retained current value. For replacement,
    /// this is the original value after undoing the discarded tip, exactly as
    /// in `Block::get_before_block`; it is not the target's current generation.
    pub fn get_before_block(&self) -> &V {
        self.revert.as_ref().unwrap_or_else(|| self.get())
    }

    /// Check the original exact Cell owner without acquiring a current generation.
    /// Equal values or matching charge types grant no publication authority.
    pub fn belongs_to(&self, cell: &Cell<V, C>) -> bool {
        self.metadata.predecessor.belongs_to(&cell.publication)
    }

    /// Observe the same opaque owner, predecessor and mode captured by the block.
    ///
    /// This only retains existing identity references. It does not acquire a
    /// writer, allocate a successor identity or turn equal values into authority.
    pub fn publication_identity(&self) -> crate::BlockPublicationIdentity {
        crate::BlockPublicationIdentity::capture(&self.metadata.predecessor, self.metadata.mode)
    }
}

#[cfg(test)]
#[path = "frozen_read_tests.rs"]
mod tests;
