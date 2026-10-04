//! Typed reads of exact frozen private maps, with no new State view or cursor.
//!
//! `Detached` already owns the original current/undo tree generations and their
//! charges. The ordinary read trait borrows them directly; it exposes neither
//! edits nor a way to reconstruct an execution block from a newer predecessor.

use super::*;

impl<K: Key, V: Value, A, M: StorageMode<K, V>> Detached<K, V, A, M> {
    /// Borrow the value before the original block's first mutation of `key`.
    ///
    /// A retained absent preimage stays absent; an untouched key comes from the
    /// original private current tree. Replacement is already reflected in those
    /// trees, so this never consults the discarded tip or a fresh State view.
    pub fn get_before_block(&self, key: &K) -> Option<&V> {
        match self.revert.get(key) {
            Some(previous) => previous.as_ref(),
            None => self.blocks.get(key),
        }
    }

    /// Observe the exact original owner, current/undo predecessor and mode.
    ///
    /// Retaining this opaque identity allocates no successor and grants no
    /// publication authority. Exact installation still rejects a foreign owner
    /// or a newer generation even if all inspected values compare equal.
    pub fn publication_identity(&self) -> crate::BlockPublicationIdentity {
        crate::BlockPublicationIdentity::capture(&self.metadata.predecessor, self.metadata.mode)
    }
}

impl<K: Key, V: Value, A, M: StorageMode<K, V>> StorageReadOnly<K, V> for Detached<K, V, A, M> {
    type Iter<'a>
        = Iter<'a, K, V, M::Charge>
    where
        Self: 'a;
    type RangeIter<'a>
        = RangeIter<'a, K, V, M::Charge>
    where
        Self: 'a;

    fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.blocks.get(key)
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.blocks.iter()
    }

    fn range<Q>(&self, bounds: impl RangeBounds<Q>) -> Self::RangeIter<'_>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.blocks.range(bounds)
    }

    fn first_key_value(&self) -> Option<(&K, &V)> {
        self.blocks.iter().next()
    }

    fn last_key_value(&self) -> Option<(&K, &V)> {
        self.blocks.iter().next_back()
    }

    fn len(&self) -> usize {
        self.blocks.len()
    }
}

#[cfg(test)]
#[path = "frozen_read_tests.rs"]
mod tests;
