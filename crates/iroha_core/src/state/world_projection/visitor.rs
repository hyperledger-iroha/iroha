//! One borrowed World visitor with owner-specific typed error custody.

use super::*;

/// Exhaustive semantic World visitor shared by delta owners and the World state accumulator.
/// Each owner supplies the hash of its actual borrowed value, excluding caches.
pub(crate) trait WorldProjection {
    /// Owner-specific errors retain local resource custody through this visitor.
    type Error: From<String>;
    fn append_storage_with<K: Key + Encode, V: Value, M: mv::storage::StorageMode<K, V>>(
        &mut self,
        name: &'static str,
        storage: &StorageBlock<'_, K, V, M>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), Self::Error>;

    fn append_cell_with<V: Value, C: Send + Sync + 'static>(
        &mut self,
        name: &'static str,
        cell: &CellBlock<'_, V, C>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), Self::Error>;

    /// Whether the trigger owner re-checks its contract rows against its action stores (a
    /// scan of every trigger) before this pass. A change-set pass over a block whose execution
    /// seal already ran the same check skips it, keeping its cost proportional to the changes.
    fn validates_trigger_contract_rows(&self) -> bool {
        true
    }

    /// The complete World state accumulator uses the semantic anchor; the physical
    /// publication journal retains the full original row through this default.
    fn append_musubi_archive_availability(
        &mut self,
        storage: &StorageBlock<'_, ArchiveId, MusubiArchiveAvailabilityV1>,
    ) -> Result<(), Self::Error> {
        self.append_storage_with("musubi_archive_availability", storage, hash_value)
    }

    /// The accumulator retains each independently assigned resolver revision.
    /// The physical publication journal keeps complete rows through this default.
    fn append_musubi_resolver_index(
        &mut self,
        storage: &StorageBlock<'_, MusubiReleaseIdV1, MusubiResolverReleaseRowV1>,
    ) -> Result<(), Self::Error> {
        self.append_storage_with("musubi_resolver_index", storage, hash_value)
    }

    /// The accumulator retains each independently assigned directory revision.
    /// The physical publication journal keeps complete rows through this default.
    fn append_musubi_public_directory(
        &mut self,
        storage: &StorageBlock<'_, MusubiPackageSelectorV1, MusubiOrderedPackageEntryV1>,
    ) -> Result<(), Self::Error> {
        self.append_storage_with("musubi_public_directory", storage, hash_value)
    }
}

impl<T: WorldProjection> WorldProjection for &mut T {
    type Error = T::Error;
    fn validates_trigger_contract_rows(&self) -> bool {
        (**self).validates_trigger_contract_rows()
    }
    fn append_storage_with<K: Key + Encode, V: Value, M: mv::storage::StorageMode<K, V>>(
        &mut self,
        name: &'static str,
        storage: &StorageBlock<'_, K, V, M>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), Self::Error> {
        (**self).append_storage_with(name, storage, encode)
    }
    fn append_cell_with<V: Value, C: Send + Sync + 'static>(
        &mut self,
        name: &'static str,
        cell: &CellBlock<'_, V, C>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), Self::Error> {
        (**self).append_cell_with(name, cell, encode)
    }

    fn append_musubi_archive_availability(
        &mut self,
        storage: &StorageBlock<'_, ArchiveId, MusubiArchiveAvailabilityV1>,
    ) -> Result<(), Self::Error> {
        (**self).append_musubi_archive_availability(storage)
    }

    fn append_musubi_resolver_index(
        &mut self,
        storage: &StorageBlock<'_, MusubiReleaseIdV1, MusubiResolverReleaseRowV1>,
    ) -> Result<(), Self::Error> {
        (**self).append_musubi_resolver_index(storage)
    }

    fn append_musubi_public_directory(
        &mut self,
        storage: &StorageBlock<'_, MusubiPackageSelectorV1, MusubiOrderedPackageEntryV1>,
    ) -> Result<(), Self::Error> {
        (**self).append_musubi_public_directory(storage)
    }
}

impl WorldProjection for WorldDeltaBuilder {
    type Error = String;
    fn append_storage_with<K: Key + Encode, V: Value, M: mv::storage::StorageMode<K, V>>(
        &mut self,
        name: &'static str,
        storage: &StorageBlock<'_, K, V, M>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), Self::Error> {
        Self::append_storage_with(self, name, storage, encode)
    }

    fn append_cell_with<V: Value, C: Send + Sync + 'static>(
        &mut self,
        name: &'static str,
        cell: &CellBlock<'_, V, C>,
        encode: impl Fn(&V) -> Result<Hash, String>,
    ) -> Result<(), Self::Error> {
        Self::append_cell_with(self, name, cell, encode)
    }
}

pub(super) trait AppendWorldField {
    fn append_world_field<P: WorldProjection>(
        &self,
        name: &'static str,
        builder: &mut P,
    ) -> Result<(), P::Error>;
}

impl<K: Key + Encode, V: Value + Encode, M: mv::storage::StorageMode<K, V>> AppendWorldField
    for StorageBlock<'_, K, V, M>
{
    fn append_world_field<P: WorldProjection>(
        &self,
        name: &'static str,
        builder: &mut P,
    ) -> Result<(), P::Error> {
        builder.append_storage_with(name, self, hash_value)
    }
}

impl<V: Value + Encode, C: Send + Sync + 'static> AppendWorldField for CellBlock<'_, V, C> {
    fn append_world_field<P: WorldProjection>(
        &self,
        name: &'static str,
        builder: &mut P,
    ) -> Result<(), P::Error> {
        builder.append_cell_with(name, self, hash_value)
    }
}
