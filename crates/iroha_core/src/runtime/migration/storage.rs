//! Pre-bound native migration storage. Only the engine owns World projections.
//!
//! A registered adapter binds one source and one successor before invoking native code.
//! The callback cannot open another table, obtain an MV writer, or recover the World. Source
//! point reads and scans are different static capabilities. Additional storage shapes need
//! another reviewed adapter, not an unrestricted projection supplied by the migration.

use core::marker::PhantomData;

use mv::{
    Key, Value,
    storage::{Block, StorageReadOnly as _, TouchedEntry, Transaction},
};

use crate::{
    runtime::{
        catalog::migrations::{DrainOutcome, StageOutcome},
        migration::{MigrationBoundaryError, MigrationBoundaryResult, MigrationResult},
    },
    state::{WorldBlock, WorldTransaction},
};

mod sealed {
    pub trait Access {}
    pub trait Lookup {}
    pub trait Scan {}
    pub trait Program {}
}

/// Fixed scratch backing funded by the exact original World execution pool.
/// This capability opens no table and grants no persistent write or extra capacity.
/// Nested payloads and codec/control graphs require their own original admission.
#[derive(Clone, Copy)]
pub struct Scratch<'a> {
    budget: &'a iroha_allocation::AllocationBudget,
}
impl Scratch<'_> {
    /// Admit one exact fixed array before asking the allocator for its backing.
    /// The returned owner retains the original charge until the backing is destroyed.
    ///
    /// # Errors
    /// Preserves the original pool or physical allocator refusal as a local deferral.
    pub fn buffer<T>(
        &self,
        capacity: usize,
    ) -> MigrationResult<iroha_allocation::ChargedBuffer<T>> {
        iroha_allocation::ChargedBuffer::new(capacity, self.budget)
            .map_err(super::scratch::buffer_error)
    }
}

/// Static source authority, committed in the new migration's native reference closure.
pub trait SourceAccess: sealed::Access + Send + Sync + 'static {
    /// Exact access contract of this adapter, including its source-read rights.
    const CONTRACT: &'static [u8];
}

/// A source capability that permits point lookup.
pub trait LookupAccess: SourceAccess + sealed::Lookup {}
/// A source capability that permits ordered scans.
pub trait ScanAccess: SourceAccess + sealed::Scan {}

/// Source lookup without traversal authority.
#[derive(Clone, Copy)]
pub struct PointOnly;
/// Source traversal without point-lookup authority.
#[derive(Clone, Copy)]
pub struct ScanOnly;
/// Explicit authority for both source lookup and traversal.
#[derive(Clone, Copy)]
pub struct PointAndScan;

macro_rules! source_access {
    ($type:ty, $contract:literal) => {
        impl sealed::Access for $type {}
        impl SourceAccess for $type {
            const CONTRACT: &'static [u8] = $contract;
        }
    };
}
source_access!(
    PointOnly,
    b"table-migration.v1/original-refusal/fixed-original-scratch/source-point"
);
source_access!(
    ScanOnly,
    b"table-migration.v1/original-refusal/fixed-original-scratch/source-scan"
);
source_access!(
    PointAndScan,
    b"table-migration.v1/original-refusal/fixed-original-scratch/source-point-and-scan"
);
impl sealed::Lookup for PointOnly {}
impl sealed::Lookup for PointAndScan {}
impl LookupAccess for PointOnly {}
impl LookupAccess for PointAndScan {}
impl sealed::Scan for ScanOnly {}
impl sealed::Scan for PointAndScan {}
impl ScanAccess for ScanOnly {}
impl ScanAccess for PointAndScan {}

/// A borrowed source table with only the adapter's declared read authority.
///
/// Point lookup does not confer scan authority:
/// ```compile_fail
/// use iroha_core::runtime::migration::storage::{PointOnly, Source};
/// fn cannot_scan(source: Source<'_, '_, PointOnly, u64, u64>) {
///     let _ = source.scan_after(None, 1);
/// }
/// ```
/// Scan authority does not confer point lookup:
/// ```compile_fail
/// use iroha_core::runtime::migration::storage::{ScanOnly, Source};
/// fn cannot_lookup(source: Source<'_, '_, ScanOnly, u64, u64>) {
///     let _ = source.get(&1);
/// }
/// ```
pub struct Source<'a, 'block, A, K: Key, V: Value> {
    storage: &'a Transaction<'block, K, V>,
    access: PhantomData<A>,
}

impl<A: LookupAccess, K: Key, V: Value> Source<'_, '_, A, K, V> {
    /// Read one source row. This capability provides no mutable value or storage handle.
    pub fn get(&self, key: &K) -> Option<&V> {
        self.storage.get(key)
    }
}

impl<A: ScanAccess, K: Key, V: Value> Source<'_, '_, A, K, V> {
    /// Visit at most `limit` source rows strictly after the supplied key, in canonical order.
    pub fn scan_after(&self, after: Option<&K>, limit: usize) -> impl Iterator<Item = (&K, &V)> {
        use core::ops::Bound::{Excluded, Unbounded};
        self.storage
            .range((after.map_or(Unbounded, Excluded), Unbounded))
            .take(limit)
    }
}

/// Write authority for only the declared successor in one copying stage.
///
/// The original MV writer cannot escape the capability:
/// ```compile_fail
/// use iroha_core::runtime::migration::storage::Successor;
/// fn cannot_extract(successor: Successor<'_, '_, u64, u64>) {
///     let _ = successor.storage;
/// }
/// ```
pub struct Successor<'a, 'block, K: Key, V: Value> {
    storage: &'a mut Transaction<'block, K, V>,
}

impl<K: Key, V: Value> Successor<'_, '_, K, V> {
    /// Replace one successor row. Previous values remain private to the storage owner.
    pub fn insert(&mut self, key: K, value: V) {
        self.storage.insert(key, value);
    }
}

/// Exact source reads and successor writes for a copying stage.
pub struct StageStorage<'a, 'block, A, K: Key, V: Value, K2: Key, V2: Value> {
    /// Original-pool fixed scratch; it carries no additional storage authority.
    pub scratch: Scratch<'a>,
    /// The source, with the statically declared point and/or scan authority.
    pub source: Source<'a, 'block, A, K, V>,
    /// The successor, with no source-write or successor-read authority.
    pub successor: Successor<'a, 'block, K2, V2>,
}

/// Read authority for the original source journal at the block barrier.
pub struct SourceChanges<'a, 'world, K: Key, V: Value> {
    storage: &'a Block<'world, K, V>,
}

impl<K: Key, V: Value> SourceChanges<'_, '_, K, V> {
    /// Visit the original touched source rows, including deletions, in canonical key order.
    pub fn touched_entries(&self) -> impl Iterator<Item = TouchedEntry<'_, K, V>> {
        self.storage.touched_entries()
    }
}

/// Successor-only mutation authority for the block barrier.
pub struct BarrierSuccessor<'a, 'block, K: Key, V: Value> {
    storage: &'a mut Transaction<'block, K, V>,
}

impl<K: Key, V: Value> BarrierSuccessor<'_, '_, K, V> {
    /// Mirror one current source row into the successor.
    pub fn insert(&mut self, key: K, value: V) {
        self.storage.insert(key, value);
    }
    /// Mirror a source deletion without exposing a general storage writer.
    pub fn remove(&mut self, key: K) {
        self.storage.remove(key);
    }
}

/// Original source journal and successor writes, bound before the barrier callback starts.
pub struct BarrierStorage<'a, 'world, 'successor, K: Key, V: Value, K2: Key, V2: Value> {
    /// Original-pool fixed scratch retained by each returned buffer.
    pub scratch: Scratch<'a>,
    /// Changes in the source's original block journal, without ordinary table reads or writes.
    pub source: SourceChanges<'a, 'world, K, V>,
    /// Only the declared successor may be changed.
    pub successor: BarrierSuccessor<'a, 'successor, K2, V2>,
}

/// Source-only deletion authority after cutover.
///
/// Drain authority cannot write another old row:
/// ```compile_fail
/// use iroha_core::runtime::migration::storage::SourceDrain;
/// fn cannot_insert(mut source: SourceDrain<'_, '_, u64, u64>) {
///     source.insert(1, 2);
/// }
/// ```
pub struct SourceDrain<'a, 'block, K: Key, V: Value> {
    storage: &'a mut Transaction<'block, K, V>,
    scratch: Scratch<'a>,
}

impl<K: Key, V: Value> SourceDrain<'_, '_, K, V> {
    /// Borrow only fixed scratch admission from this original source owner.
    pub fn scratch(&self) -> Scratch<'_> {
        self.scratch
    }
    /// Visit at most `limit` original source keys in canonical order.
    pub fn keys(&self, limit: usize) -> impl Iterator<Item = &K> {
        self.storage.iter().take(limit).map(|(key, _)| key)
    }
    /// Remove an old row. Insertion and mutable-value access are unavailable after cutover.
    pub fn remove(&mut self, key: K) {
        self.storage.remove(key);
    }
    /// Whether every old row has been drained.
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }
}

/// A native copying stage in its own original World child. Rejection and local refusal
/// drop that child before status recording or caller propagation.
pub type TableStage<A, K, V, K2, V2> =
    fn(StageStorage<'_, '_, A, K, V, K2, V2>, &[u8], u32) -> MigrationResult<StageOutcome>;
/// Mirror all accepted source rows, or refuse the complete unpublished block.
/// An unfinished codec/allocation operation is never a successful no-op.
pub type TableBarrier<K, V, K2, V2> =
    fn(BarrierStorage<'_, '_, '_, K, V, K2, V2>, &[u8]) -> MigrationResult<()>;
/// A bounded source drain after the governed cutover.
pub type TableDrain<K, V> = fn(SourceDrain<'_, '_, K, V>, u32) -> MigrationResult<DrainOutcome>;
/// Whether the original source journal contains a post-cutover write.
pub type TableStale<K, V> = fn(SourceChanges<'_, '_, K, V>) -> bool;

type TransactionPair<K, V, K2, V2> = for<'a, 'block> fn(
    &'a mut WorldTransaction<'block, '_>,
) -> (
    &'a Transaction<'block, K, V>,
    &'a mut Transaction<'block, K2, V2>,
);
type BlockPair<K, V, K2, V2> =
    for<'a, 'world> fn(
        &'a mut WorldBlock<'world>,
    ) -> (&'a Block<'world, K, V>, &'a mut Block<'world, K2, V2>);

/// One engine-owned pair of native fields. Migrations cannot construct or change projections.
pub struct TablePair<K: Key, V: Value, K2: Key, V2: Value> {
    source: &'static str,
    successor: &'static str,
    transaction: TransactionPair<K, V, K2, V2>,
    block: BlockPair<K, V, K2, V2>,
    drain: for<'a, 'block> fn(
        &'a mut WorldTransaction<'block, '_>,
    ) -> &'a mut Transaction<'block, K, V>,
    changes: for<'a, 'world> fn(&'a WorldBlock<'world>) -> &'a Block<'world, K, V>,
}

impl<K: Key, V: Value, K2: Key, V2: Value> TablePair<K, V, K2, V2> {
    /// Attach native callbacks to this already registered field pair and explicit source rights.
    pub const fn migration<A: SourceAccess>(
        &'static self,
        stage: TableStage<A, K, V, K2, V2>,
        barrier: TableBarrier<K, V, K2, V2>,
        drain: TableDrain<K, V>,
        stale: TableStale<K, V>,
    ) -> TableMigration<A, K, V, K2, V2> {
        TableMigration {
            pair: self,
            stage,
            barrier,
            drain,
            stale,
        }
    }
}

/// Typed implementation of the first supported native migration storage contract.
pub struct TableMigration<A, K: Key, V: Value, K2: Key, V2: Value> {
    pair: &'static TablePair<K, V, K2, V2>,
    stage: TableStage<A, K, V, K2, V2>,
    barrier: TableBarrier<K, V, K2, V2>,
    drain: TableDrain<K, V>,
    stale: TableStale<K, V>,
}

/// Sealed dispatch used only by the engine; native callbacks receive the typed capabilities.
pub(crate) trait MigrationProgram: sealed::Program + Sync {
    /// Refuse mismatched or unsupported declarations before publishing any migration definition.
    fn matches_storage(&self, from: &[&str], to: &[&str]) -> bool;
    /// Access contract included in this new native migration's immutable reference closure.
    fn access_contract(&self) -> &'static [u8];
    /// Bind the original source and successor before invoking a copying stage.
    fn stage(
        &self,
        world: &mut WorldTransaction<'_, '_>,
        cursor: &[u8],
        limit: u32,
    ) -> MigrationResult<StageOutcome>;
    /// Bind the original source journal and successor before invoking the total barrier.
    fn barrier(&self, world: &mut WorldBlock<'_>, cursor: &[u8]) -> MigrationBoundaryResult<()>;
    /// Give the drain callback only source deletion authority.
    fn drain(
        &self,
        world: &mut WorldTransaction<'_, '_>,
        limit: u32,
    ) -> MigrationResult<DrainOutcome>;
    /// Give the stale-write callback only original source-journal read authority.
    fn stale(&self, world: &WorldBlock<'_>) -> bool;
}

impl core::fmt::Debug for dyn MigrationProgram {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MigrationProgram")
            .field("access_contract", &self.access_contract())
            .finish_non_exhaustive()
    }
}

impl<A: SourceAccess, K: Key, V: Value, K2: Key, V2: Value> sealed::Program
    for TableMigration<A, K, V, K2, V2>
{
}

impl<A: SourceAccess, K: Key, V: Value, K2: Key, V2: Value> MigrationProgram
    for TableMigration<A, K, V, K2, V2>
{
    fn matches_storage(&self, from: &[&str], to: &[&str]) -> bool {
        from == [self.pair.source] && to == [self.pair.successor]
    }

    fn access_contract(&self) -> &'static [u8] {
        A::CONTRACT
    }

    fn stage(
        &self,
        world: &mut WorldTransaction<'_, '_>,
        cursor: &[u8],
        limit: u32,
    ) -> MigrationResult<StageOutcome> {
        let budget = world.sccp_committee_generations.allocation_budget().clone();
        let (source, successor) = (self.pair.transaction)(world);
        (self.stage)(
            StageStorage {
                scratch: Scratch { budget: &budget },
                source: Source {
                    storage: source,
                    access: PhantomData,
                },
                successor: Successor { storage: successor },
            },
            cursor,
            limit,
        )
    }

    fn barrier(&self, world: &mut WorldBlock<'_>, cursor: &[u8]) -> MigrationBoundaryResult<()> {
        let budget = world.sccp_committee_generations.allocation_budget().clone();
        let (source, successor) = (self.pair.block)(world);
        let mut child = successor.try_transaction().map_err(|original| {
            MigrationBoundaryError::Storage(crate::state::StateStorageAdmissionError::World(
                mv::storage::AdmittedStorageError::Planning(original),
            ))
        })?;
        (self.barrier)(
            BarrierStorage {
                scratch: Scratch { budget: &budget },
                source: SourceChanges { storage: source },
                successor: BarrierSuccessor {
                    storage: &mut child,
                },
            },
            cursor,
        )?;
        child.apply();
        Ok(())
    }

    fn drain(
        &self,
        world: &mut WorldTransaction<'_, '_>,
        limit: u32,
    ) -> MigrationResult<DrainOutcome> {
        let budget = world.sccp_committee_generations.allocation_budget().clone();
        (self.drain)(
            SourceDrain {
                storage: (self.pair.drain)(world),
                scratch: Scratch { budget: &budget },
            },
            limit,
        )
    }

    fn stale(&self, world: &WorldBlock<'_>) -> bool {
        (self.stale)(SourceChanges {
            storage: (self.pair.changes)(world),
        })
    }
}

/// The fixture's native adapter is registered by the engine, not by its migration callbacks.
/// TODO(N14): register each real migration's exact field pair here when it is implemented.
#[cfg(test)]
pub(crate) static SEQUENCE_TABLES: TablePair<
    iroha_data_model::account::AccountId,
    u64,
    iroha_crypto::Hash,
    bool,
> = TablePair {
    source: "world.tx_sequences",
    successor: "world.viral_bonus_paid",
    transaction: |world| (&world.tx_sequences, &mut world.viral_bonus_paid),
    block: |world| {
        let fields = &mut **world;
        (&fields.tx_sequences, &mut fields.viral_bonus_paid)
    },
    drain: |world| &mut world.tx_sequences,
    changes: |world| &world.tx_sequences,
};

#[cfg(test)]
mod tests;
