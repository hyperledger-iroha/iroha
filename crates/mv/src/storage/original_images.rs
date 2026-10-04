//! Borrow raw original frozen maps before any caller-defined predecessor traversal.
//!
//! These iterators perform no key comparison or current-value lookup. A bounded
//! caller can charge each physical row and comparison before inspecting it, including
//! masked rows and absent undo entries. This borrow neither validates a derived
//! relation nor creates a state commitment or publication authority.

use super::*;

/// Immutable original current and undo maps from one retained frozen journal.
///
/// The private constructor borrows the existing owners: it allocates no reader,
/// clones no key/value, and cannot outlive the original journal. An undo `None`
/// is an explicit absent preimage, including a redundant absent-to-absent touch.
/// The predecessor image is the current map overlaid by this undo map; callers
/// must include untouched rows and must not treat the touched subset as a root.
pub struct FrozenStorageImages<'a, K: Key, V: Value, M: StorageMode<K, V> = Untracked> {
    current: &'a BptreeMapOwned<K, V, M>,
    undo: &'a BptreeMapOwned<K, Option<V>, M>,
    predecessor: &'a CapturedPublication,
    mode: BlockMode,
}

impl<K: Key, V: Value, A, M: StorageMode<K, V>> Detached<K, V, A, M> {
    /// Borrow both original frozen maps without acquiring a target view or writer.
    ///
    /// Replacement semantics are already reflected in these original maps. No
    /// newly published target value can change this borrow or its predecessor.
    pub fn original_images(&self) -> FrozenStorageImages<'_, K, V, M> {
        FrozenStorageImages {
            current: &self.blocks,
            undo: &self.revert,
            predecessor: &self.metadata.predecessor,
            mode: self.metadata.mode,
        }
    }
}

impl<K: Key, V: Value, M: StorageMode<K, V>> FrozenStorageImages<'_, K, V, M> {
    /// Direct original current rows in native key order, without key comparisons.
    pub fn current_entries(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> + ExactSizeIterator {
        self.current.iter()
    }

    /// Direct physical undo rows, including absent and equal-value preimages.
    ///
    /// Unlike touched-entry traversal, this never looks up a successor value.
    /// It does not merge, compare or filter keys on the caller's behalf.
    pub fn undo_entries(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&K, &Option<V>)> + ExactSizeIterator {
        self.undo.iter()
    }

    /// Original acquisition mode; replacement never refreshes the discarded tip.
    pub fn mode(&self) -> BlockMode {
        self.mode
    }

    /// Retain the unchanged original owner/predecessor/mode identity without allocation.
    /// Equality is local observation only and cannot authorize publication.
    pub fn publication_identity(&self) -> crate::BlockPublicationIdentity {
        crate::BlockPublicationIdentity::capture(self.predecessor, self.mode)
    }

    /// Compare only the original owner, without locking or refreshing its generation.
    /// A true result does not establish currentness; original publication must recheck it.
    pub fn belongs_to(&self, target: &Storage<K, V, M>) -> bool {
        self.predecessor.belongs_to(&target.publication)
    }
}

#[cfg(test)]
#[path = "original_images/tests.rs"]
mod tests;
