//! Original committed map-pair custody before an overlay clears previous undo.
//!
//! Both map reads are bracketed by equal opaque publication observations. Every
//! MV publication retains the same identity mutex through physical current/undo
//! installation and identity rotation. Retaining the first identity prevents
//! equal-value or allocator ABA from making a later generation compare equal.
//! No publication mutex is held while creating or dropping native map readers.

use super::*;
use std::convert::Infallible;

/// Immutable original current and undo maps, before execution overlay reset.
///
/// This owner keeps real Concread readers and the original MV publication identity.
/// It creates no successor, clones no rows and grants no publication permission.
/// Native reader retention/control storage needs separate resource admission.
pub struct CommittedStorageView<'storage, K: Key, V: Value, M: StorageMode<K, V> = Untracked> {
    current: BptreeMapReadTxn<'storage, K, V, M>,
    undo: BptreeMapReadTxn<'storage, K, Option<V>, M>,
    original: CapturedPublication,
}

impl<'storage, K: Key, V: Value, M: StorageMode<K, V>> CommittedStorageView<'storage, K, V, M> {
    /// Borrow the exact original published current map without cloning any row.
    pub fn current(&self) -> &BptreeMapReadTxn<'storage, K, V, M> {
        &self.current
    }

    /// Borrow every published undo entry, preserving absent versus untouched keys.
    /// A stored `None` is an absent preimage; a missing key has no undo entry.
    pub fn undo(&self) -> &BptreeMapReadTxn<'storage, K, Option<V>, M> {
        &self.undo
    }

    /// Join the actual writer predecessor, not its reset undo or staged values.
    /// Ordinary and replacement modes name the same original committed source;
    /// the caller must separately preserve and interpret acquisition mode.
    pub fn matches_block_source(&self, block: &Block<'_, K, V, M>) -> bool {
        self.original.same_as(&block.predecessor)
    }

    /// Compare this retained original against its actual current owner without another read.
    ///
    /// # Errors
    /// Preserves contention or poisoning of the original publication identity lock.
    pub fn try_matches_current(
        &self,
        storage: &Storage<K, V, M>,
    ) -> Result<bool, PublicationPreparationError<Infallible>> {
        Ok(self
            .original
            .same_as(&storage.publication.try_capture_reads(|| true)?))
    }

    /// Compare original owner and published pair without a data read or lock.
    pub fn same_publication(&self, other: &Self) -> bool {
        self.original.same_as(&other.original)
    }
}

// Preserve the retained current generation through the native MV read-only API.
// Undo and original publication remain owned by this same view; no new read,
// lookup allocation, adapter heap, or row clone is introduced by forwarding.
impl<K: Key, V: Value, M: StorageMode<K, V>> StorageReadOnly<K, V>
    for CommittedStorageView<'_, K, V, M>
{
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
        K: Ord + Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.current.get(key)
    }
    fn iter(&self) -> Self::Iter<'_> {
        self.current.iter()
    }
    fn range<Q>(&self, bounds: impl RangeBounds<Q>) -> Self::RangeIter<'_>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.current.range(bounds)
    }
    fn first_key_value(&self) -> Option<(&K, &V)> {
        self.current.first_key_value()
    }
    fn last_key_value(&self) -> Option<(&K, &V)> {
        self.current.last_key_value()
    }
    fn len(&self) -> usize {
        self.current.len()
    }
}

impl<K: Key, V: Value, M: StorageMode<K, V>> Storage<K, V, M> {
    /// Retain complete committed current/undo maps before block initialization.
    ///
    /// The first original identity is retained before either reader is created.
    /// The second observation must name that identical owner and generation.
    /// A publication overlapping either read therefore refuses the whole pair,
    /// even when all values compare equal. Native reads and their cleanup occur
    /// outside the identity mutex; this function never acquires physical writers.
    ///
    /// # Errors
    /// Returns the original publication's release source on contention, a poison
    /// condition requiring owner reconstruction, or `Changed` after publication.
    pub fn try_committed_view(
        &self,
    ) -> Result<CommittedStorageView<'_, K, V, M>, PublicationPreparationError<Infallible>> {
        let original = self.publication.try_capture_reads(|| true)?;
        let current = self.blocks.read();
        let undo = self.revert.read();
        self.finish_committed_reads(original, current, undo)
    }

    /// Retain the exact published current/undo pair without waiting or allocating.
    ///
    /// Original identities bracket both native reads. Busy errors name the exact
    /// reader mutex release rather than an unrelated physical writer. No
    /// collector pin or copied row is needed by the shared B+tree generations.
    ///
    /// # Errors
    /// Preserves Busy, Poisoned and Changed from original native owners.
    pub fn try_committed_view_nonblocking(
        &self,
    ) -> Result<CommittedStorageView<'_, K, V, M>, PublicationPreparationError<Infallible>> {
        let original = self.publication.try_capture_reads(|| true)?;
        let wait = self.blocks.observe_reader_release();
        let current = self.blocks.try_read().map_err(|e| match e {
            OwnedWriteError::Busy => PublicationPreparationError::Busy(wait),
            OwnedWriteError::Poisoned => PublicationPreparationError::Poisoned,
            OwnedWriteError::Changed => PublicationPreparationError::Changed,
        })?;
        let wait = self.revert.observe_reader_release();
        let undo = self.revert.try_read().map_err(|e| match e {
            OwnedWriteError::Busy => PublicationPreparationError::Busy(wait),
            OwnedWriteError::Poisoned => PublicationPreparationError::Poisoned,
            OwnedWriteError::Changed => PublicationPreparationError::Changed,
        })?;
        self.finish_committed_reads(original, current, undo)
    }

    fn finish_committed_reads<'storage>(
        &'storage self,
        original: CapturedPublication,
        current: BptreeMapReadTxn<'storage, K, V, M>,
        undo: BptreeMapReadTxn<'storage, K, Option<V>, M>,
    ) -> Result<CommittedStorageView<'storage, K, V, M>, PublicationPreparationError<Infallible>>
    {
        let after = self.publication.try_capture_reads(|| true)?;
        if !original.same_as(&after) {
            return Err(PublicationPreparationError::Changed);
        }
        Ok(CommittedStorageView {
            current,
            undo,
            original,
        })
    }
}

#[cfg(test)]
#[path = "original_read_tests.rs"]
mod tests;
