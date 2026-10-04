//! Concrete original current/undo sources for one shared bounded relation.
//!
//! This sealed borrow surface forwards only native iterators. It never acquires a
//! reader, clones a row, compares a key, merges images or grants publication authority.
//! The retained committed view still owns its final identity fence; a frozen borrow
//! still belongs to its original immutable publication journal and acquisition mode.

use mv::{
    Key, Value,
    storage::{CommittedStorageView, FrozenStorageImages, StorageMode},
};

mod sealed {
    pub trait Sealed {}
    impl<K: super::Key, V: super::Value, M: super::StorageMode<K, V>> Sealed
        for super::CommittedStorageView<'_, K, V, M>
    {
    }
    impl<K: super::Key, V: super::Value, M: super::StorageMode<K, V>> Sealed
        for super::FrozenStorageImages<'_, K, V, M>
    {
    }
}

/// The two concrete original map pairs accepted by bounded State source checks.
///
/// Implementations are closed. Callers must prepay physical rows and comparisons,
/// including masked rows and absent preimages, before inspecting or filtering them.
/// Callers cannot supply an ad hoc row iterator through this interface. Owning a
/// native map still does not establish State provenance: each consumer must retain
/// and authenticate its actual source separately, including after snapshot decode.
pub(in crate::state) trait RawStorageImages<K: Key, V: Value>:
    sealed::Sealed
{
    /// Borrow every original current row without a lookup or fresh read.
    fn current_entries(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> + ExactSizeIterator;
    /// Borrow every physical undo row, including explicit absent/no-op preimages.
    fn undo_entries(&self)
    -> impl DoubleEndedIterator<Item = (&K, &Option<V>)> + ExactSizeIterator;
}

impl<K: Key, V: Value, M: StorageMode<K, V>> RawStorageImages<K, V>
    for CommittedStorageView<'_, K, V, M>
{
    fn current_entries(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> + ExactSizeIterator {
        self.current().iter()
    }
    fn undo_entries(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&K, &Option<V>)> + ExactSizeIterator {
        self.undo().iter()
    }
}

impl<K: Key, V: Value, M: StorageMode<K, V>> RawStorageImages<K, V>
    for FrozenStorageImages<'_, K, V, M>
{
    fn current_entries(&self) -> impl DoubleEndedIterator<Item = (&K, &V)> + ExactSizeIterator {
        FrozenStorageImages::current_entries(self)
    }
    fn undo_entries(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&K, &Option<V>)> + ExactSizeIterator {
        FrozenStorageImages::undo_entries(self)
    }
}
