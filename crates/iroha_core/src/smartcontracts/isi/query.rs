//! Query functionality. The common error type is also defined here,
//! alongside functions for converting them into HTTP responses.

use std::{
    cell::Cell,
    collections::BinaryHeap,
    mem,
    num::NonZeroU64,
    ops::ControlFlow,
    sync::{Arc, Mutex, Weak},
};

use eyre::Result;
use iroha_config::parameters::{
    actual::{Pipeline as PipelineActual, Torii as ToriiActual},
    defaults::pipeline as pipeline_defaults,
};
use iroha_data_model::{
    escrow::{AnonymousAssetEscrowRecord, AssetEscrowRecord},
    prelude::*,
    query::{
        CommittedTransaction, QueryBox, QueryOutput, QueryOutputBatchBox, QueryOutputBatchBoxTuple,
        QueryRequest, QueryResponse, SingularQueryBox, SingularQueryOutputBox,
        dsl::{CompoundPredicate, EvaluateSelector, HasProjection, SelectorMarker},
        error::QueryExecutionFail as Error,
        parameters::{DEFAULT_FETCH_SIZE, QueryParams, SortOrder},
    },
};
use mv::storage::StorageReadOnly as _;
use norito::core::{Archived, Header, NoritoSerialize};

use crate::{
    prelude::ValidSingularQuery,
    query::{
        cursor::ErasedQueryIterator,
        pagination::Paginate as _,
        store::{
            DeferredQueryContinuation, LiveQueryStoreHandle, PagedQueryContinuation,
            PreparedPagedQueryStart, PreparedQueryStart,
        },
    },
    smartcontracts::ValidQuery,
    smartcontracts::isi::tx::{
        TransactionHistoryAnchor, TransactionHistoryCursor, visit_committed_transactions,
    },
    state::{State, StateReadOnly, WorldReadOnly},
};

#[inline]
fn ensure_query_registry_initialized() {
    // Initialize the global query registry once. Safe to call multiple times:
    // iroha_data_model uses `OnceLock` and ignores subsequent sets.
    use iroha_data_model as dm;
    use iroha_data_model::query as dm_query;
    dm_query::set_query_registry(dm::query_registry![
        dm_query::ErasedIterQuery<dm::domain::Domain>,
        dm_query::ErasedIterQuery<dm::account::Account>,
        dm_query::ErasedIterQuery<dm::asset::value::Asset>,
        dm_query::ErasedIterQuery<dm::asset::definition::AssetDefinition>,
        dm_query::ErasedIterQuery<dm::repo::RepoAgreement>,
        dm_query::ErasedIterQuery<dm::nft::Nft>,
        dm_query::ErasedIterQuery<dm::rwa::Rwa>,
        dm_query::ErasedIterQuery<dm::role::Role>,
        dm_query::ErasedIterQuery<dm::role::RoleId>,
        dm_query::ErasedIterQuery<dm::peer::PeerId>,
        dm_query::ErasedIterQuery<dm::trigger::TriggerId>,
        dm_query::ErasedIterQuery<dm::trigger::Trigger>,
        dm_query::ErasedIterQuery<dm_query::CommittedTransaction>,
        dm_query::ErasedIterQuery<dm::block::SignedBlock>,
        dm_query::ErasedIterQuery<dm::block::BlockHeader>,
        dm_query::ErasedIterQuery<dm::proof::ProofRecord>,
        dm_query::ErasedIterQuery<dm::oracle::FeedConfig>,
        dm_query::ErasedIterQuery<dm::events::data::oracle::FeedEventRecord>,
        dm_query::ErasedIterQuery<dm::oracle::OracleProviderStatsRecord>,
        dm_query::ErasedIterQuery<dm::oracle::OracleDispute>,
        dm_query::ErasedIterQuery<dm::oracle::OracleChangeProposal>,
        dm_query::ErasedIterQuery<dm::oracle::TwitterBindingRecord>,
        dm_query::ErasedIterQuery<dm::oracle::DefiOracleAttestation>,
        dm_query::ErasedIterQuery<dm::escrow::AssetEscrowRecord>,
        dm_query::ErasedIterQuery<dm::escrow::AnonymousAssetEscrowRecord>,
        dm_query::ErasedIterQuery<dm::nexus::FeeSponsorPolicy>,
        dm_query::ErasedIterQuery<dm::nexus::FeeSponsorPolicyId>,
    ]);
}

/// Allows to generalize retrieving the metadata key for all the query output types
pub trait SortableQueryOutput {
    /// Type used for deterministic tie-breaking when metadata sort keys are equal.
    type TiebreakKey: Ord + NoritoSerialize + Send + Sync;

    /// Get the sorting key for the output, from metadata
    ///
    /// If the type doesn't have metadata or metadata key doesn't exist - return None
    fn get_metadata_sorting_key(&self, key: &Name) -> Option<&Json>;
    /// Deterministic tie-breaker key for stable ordering across equal metadata keys.
    ///
    /// Implementations should return a deterministic key that uniquely and
    /// stably identifies the item so that sorting remains stable across nodes.
    fn tiebreak_key(&self) -> Self::TiebreakKey;

    /// Measure the encoded tiebreak key without materializing it.
    ///
    /// Metered sorting calls this before [`Self::tiebreak_key`], so an
    /// attacker-sized key is rejected before any key allocation or clone.
    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error>;

    /// Compare two items by their deterministic tie-break order.
    fn tiebreak_cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.tiebreak_key().cmp(&other.tiebreak_key())
    }
}

/// Query execution limits derived from configuration snapshots.
#[derive(Debug, Copy, Clone)]
pub struct QueryLimits {
    max_fetch_size: u64,
    count_mode: QueryCountMode,
}

/// Whether query pagination should compute exact counts or only bounded continuation metadata.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum QueryCountMode {
    /// Compute exact totals and remaining item counts.
    Exact,
    /// Stop once enough items are available to answer the requested page and `has_more`.
    Bounded,
}

/// Deterministic work budget for an ephemeral query.
///
/// The weighted limit prevents callers from independently exhausting the item
/// and byte ceilings from the same pool of execution units. Bytes are measured
/// without allocating an encoded buffer and include every value traversed by
/// sorting or pagination, plus the final framed response.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct QueryExecutionBudget {
    max_items: u64,
    max_bytes: u64,
    max_units: u64,
    units_per_item: u64,
    units_per_byte: u64,
}

impl QueryExecutionBudget {
    /// Build a budget from one shared weighted work limit.
    #[must_use]
    pub const fn from_weighted_limit(
        max_units: u64,
        units_per_item: u64,
        units_per_byte: u64,
    ) -> Self {
        let max_items = if units_per_item == 0 {
            u64::MAX
        } else {
            max_units / units_per_item
        };
        let max_bytes = if units_per_byte == 0 {
            u64::MAX
        } else {
            max_units / units_per_byte
        };
        Self {
            max_items,
            max_bytes,
            max_units,
            units_per_item,
            units_per_byte,
        }
    }

    /// Maximum number of charged items when no bytes are consumed.
    #[must_use]
    pub const fn max_items(self) -> u64 {
        self.max_items
    }

    /// Maximum number of charged bytes when no items are consumed.
    #[must_use]
    pub const fn max_bytes(self) -> u64 {
        self.max_bytes
    }

    fn ensure(self, items: u64, bytes: u64) -> Result<(), Error> {
        let weighted = self
            .units_per_item
            .saturating_mul(items)
            .saturating_add(self.units_per_byte.saturating_mul(bytes));
        if items > self.max_items || bytes > self.max_bytes || weighted > self.max_units {
            return Err(Error::GasBudgetExceeded);
        }
        Ok(())
    }

    fn remaining_bytes(self, items: u64, bytes: u64) -> Result<u64, Error> {
        self.ensure(items, bytes)?;
        let cap_remaining = self.max_bytes.saturating_sub(bytes);
        if self.units_per_byte == 0 {
            return Ok(cap_remaining);
        }
        let item_units = self.units_per_item.saturating_mul(items);
        let byte_units = self.units_per_byte.saturating_mul(bytes);
        let units_remaining = self
            .max_units
            .saturating_sub(item_units.saturating_add(byte_units));
        Ok(cap_remaining.min(units_remaining / self.units_per_byte))
    }
}

/// Work observed while executing an ephemeral query.
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub struct QueryExecutionStats {
    processed_items: u64,
    processed_bytes: u64,
}

impl QueryExecutionStats {
    /// Number of items charged by query execution.
    #[must_use]
    pub const fn processed_items(self) -> u64 {
        self.processed_items
    }

    /// Number of encoded bytes charged by query execution.
    ///
    /// This intentionally includes both source values traversed by the query
    /// and the final framed response: scanning/sorting and response encoding
    /// are separate pieces of deterministic work.
    #[must_use]
    pub const fn processed_bytes(self) -> u64 {
        self.processed_bytes
    }

    fn record_item<T: NoritoSerialize>(
        &mut self,
        value: &T,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<(), Error> {
        self.processed_items = self.processed_items.saturating_add(1);
        self.record_value_bytes(value, budget)
    }

    fn record_skipped_value<T: NoritoSerialize>(
        &mut self,
        value: &T,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<(), Error> {
        self.record_value_bytes(value, budget)
    }

    fn record_value_bytes<T: NoritoSerialize>(
        &mut self,
        value: &T,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<(), Error> {
        let Some(budget) = budget else {
            return Ok(());
        };
        let remaining = budget.remaining_bytes(self.processed_items, self.processed_bytes)?;
        let encoded = bounded_bare_encoded_len(value, remaining)?;
        self.processed_bytes = self.processed_bytes.saturating_add(encoded);
        budget.ensure(self.processed_items, self.processed_bytes)
    }

    fn record_response<T: NoritoSerialize>(
        &mut self,
        value: &T,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<(), Error> {
        let Some(budget) = budget else {
            return Ok(());
        };
        let remaining = budget.remaining_bytes(self.processed_items, self.processed_bytes)?;
        let encoded = bounded_framed_encoded_len(value, remaining)?;
        self.processed_bytes = self.processed_bytes.saturating_add(encoded);
        budget.ensure(self.processed_items, self.processed_bytes)
    }

    fn record_precomputed_bytes(
        &mut self,
        encoded: u64,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<(), Error> {
        let Some(budget) = budget else {
            return Ok(());
        };
        let remaining = budget.remaining_bytes(self.processed_items, self.processed_bytes)?;
        if encoded > remaining {
            return Err(Error::GasBudgetExceeded);
        }
        self.processed_bytes = self.processed_bytes.saturating_add(encoded);
        budget.ensure(self.processed_items, self.processed_bytes)
    }

    fn record_preflighted_item(
        &mut self,
        encoded: u64,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<(), Error> {
        self.processed_items = self.processed_items.saturating_add(1);
        self.record_precomputed_bytes(encoded, budget)
    }
}

struct BoundedLengthWriter {
    bytes: u64,
    limit: u64,
    exceeded: bool,
}

impl BoundedLengthWriter {
    const fn new(limit: u64) -> Self {
        Self {
            bytes: 0,
            limit,
            exceeded: false,
        }
    }
}

impl std::io::Write for BoundedLengthWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let len = u64::try_from(buf.len()).unwrap_or(u64::MAX);
        let Some(next) = self.bytes.checked_add(len) else {
            self.exceeded = true;
            return Err(std::io::Error::other("query byte budget exceeded"));
        };
        if next > self.limit {
            self.exceeded = true;
            return Err(std::io::Error::other("query byte budget exceeded"));
        }
        self.bytes = next;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn bounded_bare_encoded_len<T: NoritoSerialize>(value: &T, limit: u64) -> Result<u64, Error> {
    if let Some(exact) = value.encoded_len_exact() {
        let exact = u64::try_from(exact).unwrap_or(u64::MAX);
        return (exact <= limit)
            .then_some(exact)
            .ok_or(Error::GasBudgetExceeded);
    }

    let mut writer = BoundedLengthWriter::new(limit);
    if value.serialize(&mut writer).is_err() {
        return if writer.exceeded {
            Err(Error::GasBudgetExceeded)
        } else {
            Err(Error::Conversion(
                "failed to measure query result encoding".to_owned(),
            ))
        };
    }
    Ok(writer.bytes)
}

fn bounded_framed_encoded_len<T: NoritoSerialize>(value: &T, limit: u64) -> Result<u64, Error> {
    let header = u64::try_from(Header::SIZE).unwrap_or(u64::MAX);
    let align = mem::align_of::<Archived<T>>();
    let padding = if align <= 1 {
        0
    } else {
        let remainder = Header::SIZE % align;
        if remainder == 0 { 0 } else { align - remainder }
    };
    let overhead = header.saturating_add(u64::try_from(padding).unwrap_or(u64::MAX));
    if overhead > limit {
        return Err(Error::GasBudgetExceeded);
    }
    bounded_bare_encoded_len(value, limit - overhead)
        .map(|payload| overhead.saturating_add(payload))
}

fn bounded_encoded_vec_tiebreak_len<T: NoritoSerialize>(
    value: &T,
    limit: u64,
) -> Result<u64, Error> {
    // `Encode::encode(value)` is the bare archived payload. The resulting
    // `Vec<u8>` tiebreak key adds Norito's fixed u64 sequence length prefix.
    const VEC_LENGTH_PREFIX: u64 = 8;
    if limit < VEC_LENGTH_PREFIX {
        return Err(Error::GasBudgetExceeded);
    }
    let payload = bounded_bare_encoded_len(value, limit - VEC_LENGTH_PREFIX)?;
    Ok(VEC_LENGTH_PREFIX.saturating_add(payload))
}

fn materialize_admitted_tiebreak_key<T: SortableQueryOutput>(
    value: &T,
    stats: &mut QueryExecutionStats,
    budget: Option<QueryExecutionBudget>,
) -> Result<T::TiebreakKey, Error> {
    if let Some(budget) = budget {
        let remaining = budget.remaining_bytes(stats.processed_items, stats.processed_bytes)?;
        let encoded = value.bounded_tiebreak_key_len(remaining)?;
        stats.record_precomputed_bytes(encoded, Some(budget))?;
    }
    Ok(value.tiebreak_key())
}

impl QueryLimits {
    /// Construct limits from a Torii configuration snapshot.
    #[must_use]
    pub fn from_torii(cfg: &ToriiActual) -> Self {
        Self::new(u64::from(cfg.app_api.max_fetch_size.get()))
    }

    /// Construct limits from a Pipeline configuration snapshot.
    #[must_use]
    pub fn from_pipeline(cfg: &PipelineActual) -> Self {
        Self::new(cfg.query_max_fetch_size)
    }

    /// Construct limits from pipeline defaults (used outside Torii contexts).
    #[must_use]
    pub fn from_defaults() -> Self {
        Self::new(pipeline_defaults::QUERY_MAX_FETCH_SIZE)
    }

    /// Construct limits from a maximum fetch size value.
    #[must_use]
    pub fn new(max_fetch_size: u64) -> Self {
        Self {
            max_fetch_size: max_fetch_size.max(1),
            count_mode: QueryCountMode::Exact,
        }
    }

    /// Return limits with a different count mode.
    #[must_use]
    pub fn with_count_mode(mut self, count_mode: QueryCountMode) -> Self {
        self.count_mode = count_mode;
        self
    }
}

impl Default for QueryLimits {
    fn default() -> Self {
        Self::from_defaults()
    }
}

impl SortableQueryOutput for Account {
    type TiebreakKey = AccountId;

    fn get_metadata_sorting_key(&self, key: &Name) -> Option<&Json> {
        self.metadata().get(key)
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.id().clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(self.id(), limit)
    }

    fn tiebreak_cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id().cmp(other.id())
    }
}

impl SortableQueryOutput for AccountId {
    type TiebreakKey = Self;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(self, limit)
    }

    fn tiebreak_cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.cmp(other)
    }
}

impl SortableQueryOutput for Domain {
    type TiebreakKey = DomainId;

    fn get_metadata_sorting_key(&self, key: &Name) -> Option<&Json> {
        self.metadata().get(key)
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.id().clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(self.id(), limit)
    }

    fn tiebreak_cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id().cmp(other.id())
    }
}

impl SortableQueryOutput for AssetDefinition {
    type TiebreakKey = AssetDefinitionId;

    fn get_metadata_sorting_key(&self, key: &Name) -> Option<&Json> {
        self.metadata().get(key)
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.id().clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(self.id(), limit)
    }

    fn tiebreak_cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id().cmp(other.id())
    }
}

impl SortableQueryOutput for Asset {
    type TiebreakKey = AssetId;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.id().clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(self.id(), limit)
    }

    fn tiebreak_cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id().cmp(other.id())
    }
}

impl SortableQueryOutput for Nft {
    type TiebreakKey = iroha_data_model::nft::NftId;

    fn get_metadata_sorting_key(&self, key: &Name) -> Option<&Json> {
        self.content().get(key)
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.id().clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(self.id(), limit)
    }

    fn tiebreak_cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id().cmp(other.id())
    }
}

impl SortableQueryOutput for Rwa {
    type TiebreakKey = iroha_data_model::rwa::RwaId;

    fn get_metadata_sorting_key(&self, key: &Name) -> Option<&Json> {
        self.metadata().get(key)
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.id().clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(self.id(), limit)
    }

    fn tiebreak_cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id().cmp(other.id())
    }
}

impl SortableQueryOutput for Role {
    type TiebreakKey = iroha_data_model::role::RoleId;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.id.clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(&self.id, limit)
    }

    fn tiebreak_cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl SortableQueryOutput for RoleId {
    type TiebreakKey = Self;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(self, limit)
    }

    fn tiebreak_cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.cmp(other)
    }
}
impl SortableQueryOutput for CommittedTransaction {
    type TiebreakKey = Vec<u8>;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        norito::codec::Encode::encode(self)
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_encoded_vec_tiebreak_len(self, limit)
    }
}

impl SortableQueryOutput for PeerId {
    type TiebreakKey = Vec<u8>;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        norito::codec::Encode::encode(self)
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_encoded_vec_tiebreak_len(self, limit)
    }
}

impl SortableQueryOutput for iroha_data_model::nexus::FeeSponsorPolicy {
    type TiebreakKey = iroha_data_model::nexus::FeeSponsorPolicyId;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.id.clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(&self.id, limit)
    }
}

impl SortableQueryOutput for iroha_data_model::nexus::FeeSponsorPolicyId {
    type TiebreakKey = Self;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(self, limit)
    }

    fn tiebreak_cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.cmp(other)
    }
}

impl SortableQueryOutput for Permission {
    type TiebreakKey = Vec<u8>;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        norito::codec::Encode::encode(self)
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_encoded_vec_tiebreak_len(self, limit)
    }
}

impl SortableQueryOutput for Trigger {
    type TiebreakKey = TriggerId;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.id().clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(self.id(), limit)
    }

    fn tiebreak_cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id().cmp(other.id())
    }
}

impl SortableQueryOutput for TriggerId {
    type TiebreakKey = Self;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(self, limit)
    }

    fn tiebreak_cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.cmp(other)
    }
}

impl SortableQueryOutput for RepoAgreement {
    type TiebreakKey = iroha_data_model::repo::RepoAgreementId;

    fn get_metadata_sorting_key(&self, key: &Name) -> Option<&Json> {
        self.collateral_leg().metadata().get(key)
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.id().clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(self.id(), limit)
    }

    fn tiebreak_cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id().cmp(other.id())
    }
}

impl SortableQueryOutput for AssetEscrowRecord {
    type TiebreakKey = iroha_data_model::escrow::EscrowId;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.id
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(&self.id, limit)
    }
}

impl SortableQueryOutput for AnonymousAssetEscrowRecord {
    type TiebreakKey = iroha_data_model::escrow::EscrowId;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.id
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(&self.id, limit)
    }
}

trait ExecuteSingularQuery {
    fn execute(self, state: &impl StateReadOnly) -> Result<SingularQueryOutputBox, Error>;
}

fn preflight_singular_source_materialization(
    query: &SingularQueryBox,
    state: &impl StateReadOnly,
    budget: Option<QueryExecutionBudget>,
) -> Result<u64, Error> {
    let Some(budget) = budget else {
        return Ok(0);
    };
    let limit = budget.remaining_bytes(1, 0)?;
    let world = state.world();

    fn charge<T: NoritoSerialize>(value: &T, remaining: &mut u64) -> Result<(), Error> {
        let bytes = bounded_bare_encoded_len(value, *remaining)?;
        *remaining = (*remaining).saturating_sub(bytes);
        Ok(())
    }

    fn reject_unbounded(name: &str) -> Error {
        Error::Conversion(format!(
            "IVM singular query `{name}` has no bounded materialization adapter"
        ))
    }

    let mut remaining = limit;

    // These entity queries otherwise clone arbitrarily large metadata/content
    // before the generic output enum can be measured. Measure the borrowed
    // state value first, so the only owned materialization is already bounded.
    // The match is deliberately exhaustive: a new singular variant cannot enter
    // metered IVM execution until it supplies either a borrowed preflight or an
    // explicitly fixed-size result. Synthesized/decode-heavy legacy singulars
    // fail closed instead of allocating first and checking later.
    match query {
        SingularQueryBox::FindExecutorDataModel(_) => {
            let model = world.executor_data_model();
            if model.permissions().is_empty() {
                return Err(reject_unbounded("FindExecutorDataModel fallback"));
            }
            charge(model, &mut remaining)?;
        }
        SingularQueryBox::FindParameters(_) => charge(world.parameters(), &mut remaining)?,
        SingularQueryBox::FindAccountById(query) => {
            if let Ok(account) = world.account(query.account_id()) {
                charge(account.value().as_ref(), &mut remaining)?;
            }
        }
        SingularQueryBox::FindAccountByAlias(query) => {
            let account_id = world
                .account_rekey_records()
                .get(query.alias())
                .map(|record| &record.active_account_id)
                .or_else(|| world.account_aliases().get(query.alias()))
                .ok_or_else(|| reject_unbounded("FindAccountByAlias without indexed binding"))?;
            if let Ok(account) = world.account(account_id) {
                charge(account.value().as_ref(), &mut remaining)?;
            }
        }
        SingularQueryBox::FindAliasesByAccountId(_) => {
            return Err(reject_unbounded("FindAliasesByAccountId"));
        }
        SingularQueryBox::FindAccountRecoveryPolicyByAlias(query) => {
            if let Some(policy) = world.account_recovery_policies().get(query.alias()) {
                charge(policy, &mut remaining)?;
            }
        }
        SingularQueryBox::FindAccountRecoveryRequestByAlias(query) => {
            if let Some(request) = world.account_recovery_requests().get(query.alias()) {
                charge(request, &mut remaining)?;
            }
        }
        SingularQueryBox::FindProofRecordById(query) => {
            if let Some(record) = world.proofs().get(&query.id) {
                charge(record, &mut remaining)?;
            }
        }
        SingularQueryBox::FindContractManifestByCodeHash(query) => {
            if let Some(manifest) = world.contract_manifests().get(&query.code_hash) {
                charge(manifest, &mut remaining)?;
            }
        }
        SingularQueryBox::FindAbiVersion(_) => {}
        SingularQueryBox::FindAssetById(query) => {
            if let Ok(asset) = world.asset(query.asset_id()) {
                charge(asset.value().as_ref(), &mut remaining)?;
            }
        }
        SingularQueryBox::FindDomainById(query) => {
            if let Ok(domain) = world.domain(query.domain_id()) {
                charge(domain, &mut remaining)?;
            }
        }
        SingularQueryBox::FindAssetDefinitionById(query) => {
            if let Some(definition) = world.asset_definitions().get(query.asset_definition_id()) {
                charge(definition, &mut remaining)?;
            }
        }
        SingularQueryBox::FindAssetEscrowById(query) => {
            if let Some(record) = world.asset_escrows().get(&query.escrow_id) {
                charge(record, &mut remaining)?;
            }
        }
        SingularQueryBox::FindAnonymousAssetEscrowById(query) => {
            if let Some(record) = world.anonymous_asset_escrows().get(&query.escrow_id) {
                charge(record, &mut remaining)?;
            }
        }
        SingularQueryBox::FindTriggerById(_) => {
            return Err(reject_unbounded("FindTriggerById"));
        }
        SingularQueryBox::FindTwitterBindingByHash(query) => {
            if let Some(record) = world.twitter_bindings().get(&query.binding_hash.digest) {
                charge(record, &mut remaining)?;
            }
        }
        SingularQueryBox::FindOracleFeedById(query) => {
            if let Some(record) = world.oracle_feeds().get(&query.feed_id) {
                charge(record, &mut remaining)?;
            }
        }
        SingularQueryBox::FindOracleDisputeById(query) => {
            if let Some(record) = world.oracle_disputes().get(&query.dispute_id) {
                charge(record, &mut remaining)?;
            }
        }
        SingularQueryBox::FindOracleChangeById(query) => {
            if let Some(record) = world.oracle_changes().get(&query.change_id) {
                charge(record, &mut remaining)?;
            }
        }
        SingularQueryBox::FindOracleProviderStatsByKey(_) => {}
        SingularQueryBox::FindLatestDefiOracleAttestation(query) => {
            if let Some(record) = world
                .defi_oracle_attestations()
                .get(&query.key)
                .and_then(|records| records.last())
            {
                charge(record, &mut remaining)?;
            }
        }
        SingularQueryBox::FindDomainEndorsements(query) => {
            if let Some(hashes) = world.domain_endorsements_by_domain().get(&query.domain_id) {
                charge(hashes, &mut remaining)?;
                for hash in hashes {
                    if let Some(record) = world.domain_endorsements().get(hash) {
                        charge(record, &mut remaining)?;
                    }
                }
            }
        }
        SingularQueryBox::FindDomainEndorsementPolicy(query) => {
            if let Some(policy) = world.domain_endorsement_policies().get(&query.domain_id) {
                charge(policy, &mut remaining)?;
            }
        }
        SingularQueryBox::FindDomainCommittee(query) => {
            if let Some(committee) = world.domain_committees().get(&query.committee_id) {
                charge(committee, &mut remaining)?;
            }
        }
        SingularQueryBox::FindDaPinIntentByTicket(query) => {
            if let Some(intent) = world.da_pin_intents_by_ticket().get(&query.storage_ticket) {
                charge(intent, &mut remaining)?;
            }
        }
        SingularQueryBox::FindDaPinIntentByManifest(query) => {
            if let Some(ticket) = world.da_pin_intents_by_manifest().get(&query.manifest_hash)
                && let Some(intent) = world.da_pin_intents_by_ticket().get(ticket)
            {
                charge(intent, &mut remaining)?;
            }
        }
        SingularQueryBox::FindDaPinIntentByAlias(query) => {
            if let Some(ticket) = world.da_pin_intents_by_alias().get(&query.alias)
                && let Some(intent) = world.da_pin_intents_by_ticket().get(ticket)
            {
                charge(intent, &mut remaining)?;
            }
        }
        SingularQueryBox::FindDaPinIntentByLaneEpochSequence(query) => {
            if let Some(ticket) = world.da_pin_intents_by_lane_epoch().get(&(
                query.lane_id,
                query.epoch,
                query.sequence,
            )) && let Some(intent) = world.da_pin_intents_by_ticket().get(ticket)
            {
                charge(intent, &mut remaining)?;
            }
        }
        SingularQueryBox::FindLaneRelayEnvelopeByRef(_) => {
            return Err(reject_unbounded("FindLaneRelayEnvelopeByRef"));
        }
        SingularQueryBox::FindFeeSponsorPolicyById(query) => {
            if let Some(policy) = world.fee_sponsor_policies().get(&query.id) {
                charge(policy, &mut remaining)?;
            }
        }
        SingularQueryBox::FindFxCorridorPolicyRegistry(_) => {
            return Err(reject_unbounded("FindFxCorridorPolicyRegistry"));
        }
        SingularQueryBox::FindFxCorridorPolicyById(_) => {
            return Err(reject_unbounded("FindFxCorridorPolicyById"));
        }
        SingularQueryBox::FindSorafsProviderOwner(query) => {
            if let Some(owner) = world.provider_owners().get(&query.provider_id) {
                charge(owner, &mut remaining)?;
            }
        }
        SingularQueryBox::FindSorafsOrderbookPolicy(_)
        | SingularQueryBox::FindSorafsOrderbookOrderById(_)
        | SingularQueryBox::FindSorafsOrderbookCancellationByOrderId(_)
        | SingularQueryBox::FindSorafsOrderbookReceiptById(_)
        | SingularQueryBox::FindSorafsOrderbookStatus(_)
        | SingularQueryBox::FindSorafsOrderbookOrders(_)
        | SingularQueryBox::FindSorafsOrderbookReceipts(_) => {
            return Err(reject_unbounded("SoraFS orderbook query"));
        }
        SingularQueryBox::FindSorafsPopIssuerPolicy(_)
        | SingularQueryBox::FindSorafsPopCredentialCommitmentByDigest(_)
        | SingularQueryBox::FindSorafsPopCommitmentRootByVersion(_)
        | SingularQueryBox::FindSorafsPopRevocationPublicationByVersion(_)
        | SingularQueryBox::FindSorafsPopRevocationByNonceCommitment(_)
        | SingularQueryBox::FindSorafsPopAuditDigestBySequence(_)
        | SingularQueryBox::FindSorafsPopRegistryStatus(_) => {
            return Err(reject_unbounded("SoraFS PoP registry query"));
        }
        SingularQueryBox::FindSorafsModerationPolicy(_)
        | SingularQueryBox::FindSorafsModerationAppeal(_)
        | SingularQueryBox::FindSorafsModerationJurorEligibility(_)
        | SingularQueryBox::FindSorafsModerationCase(_)
        | SingularQueryBox::FindSorafsModerationCommit(_)
        | SingularQueryBox::FindSorafsModerationReveal(_)
        | SingularQueryBox::FindSorafsModerationChallenge(_)
        | SingularQueryBox::FindSorafsModerationOutcome(_)
        | SingularQueryBox::FindSorafsModerationNoShow(_)
        | SingularQueryBox::FindSorafsModerationStatus(_) => {
            return Err(reject_unbounded("SoraFS moderation query"));
        }
        SingularQueryBox::FindDataspaceNameOwnerById(_) => {
            return Err(reject_unbounded("FindDataspaceNameOwnerById"));
        }
        SingularQueryBox::FindMusubiReleaseByRef(_) => {
            return Err(reject_unbounded("FindMusubiReleaseByRef"));
        }
        SingularQueryBox::FindMusubiPackageVersions(_) => {
            return Err(reject_unbounded("FindMusubiPackageVersions"));
        }
        SingularQueryBox::FindMusubiPackageReleases(_) => {
            return Err(reject_unbounded("FindMusubiPackageReleases"));
        }
        SingularQueryBox::SearchMusubiPackages(_) => {
            return Err(reject_unbounded("SearchMusubiPackages"));
        }
        SingularQueryBox::FindMusubiShortAliasByName(_) => {
            return Err(reject_unbounded("FindMusubiShortAliasByName"));
        }
        SingularQueryBox::FindNftById(query) => {
            if let Ok(nft) = world.nft(query.nft_id()) {
                charge(nft.value().as_ref(), &mut remaining)?;
            }
        }
    }
    Ok(limit.saturating_sub(remaining))
}

impl ExecuteSingularQuery for SingularQueryBox {
    fn execute(self, state: &impl StateReadOnly) -> Result<SingularQueryOutputBox, Error> {
        match self {
            SingularQueryBox::FindExecutorDataModel(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindParameters(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindAccountById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindAccountByAlias(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindAliasesByAccountId(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindAccountRecoveryPolicyByAlias(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindAccountRecoveryRequestByAlias(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindProofRecordById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindContractManifestByCodeHash(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindAbiVersion(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindAssetById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindAssetDefinitionById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindAssetEscrowById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindAnonymousAssetEscrowById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindTriggerById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindTwitterBindingByHash(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindOracleFeedById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindOracleDisputeById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindOracleChangeById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindOracleProviderStatsByKey(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindLatestDefiOracleAttestation(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindDomainEndorsements(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindDomainEndorsementPolicy(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindDomainCommittee(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindDaPinIntentByTicket(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindDaPinIntentByManifest(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindDaPinIntentByAlias(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindDaPinIntentByLaneEpochSequence(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindLaneRelayEnvelopeByRef(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindFeeSponsorPolicyById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindFxCorridorPolicyRegistry(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindFxCorridorPolicyById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsProviderOwner(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsOrderbookPolicy(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsOrderbookOrderById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsOrderbookCancellationByOrderId(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsOrderbookReceiptById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsOrderbookStatus(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsOrderbookOrders(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsOrderbookReceipts(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsPopIssuerPolicy(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsPopCredentialCommitmentByDigest(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsPopCommitmentRootByVersion(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsPopRevocationPublicationByVersion(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsPopRevocationByNonceCommitment(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsPopAuditDigestBySequence(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsPopRegistryStatus(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsModerationPolicy(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsModerationAppeal(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsModerationJurorEligibility(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsModerationCase(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsModerationCommit(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsModerationReveal(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsModerationChallenge(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsModerationOutcome(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsModerationNoShow(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindSorafsModerationStatus(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindDataspaceNameOwnerById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindMusubiReleaseByRef(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindMusubiPackageVersions(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindMusubiPackageReleases(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::SearchMusubiPackages(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindMusubiShortAliasByName(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindDomainById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindNftById(q) => Ok(SingularQueryOutputBox::from(q.execute(state)?)),
        }
    }
}

#[allow(dead_code)]
trait ExecuteQueryBox {
    fn execute(
        self,
        state: &impl StateReadOnly,
        params: &QueryParams,
    ) -> Result<QueryOutputBatchBox, Error>;
}

// NOTE: This trait is currently unused. Iterable query execution of erased
// `QueryBox<QueryOutputBatchBox>` is performed in `ValidQueryRequest::execute`
// via registry-based dispatch (`iter_query_inner::<T>`), followed by
// post-processing and registration in the live-query store. If a direct
// `QueryBox::execute` path becomes necessary, this impl should be updated to
// mirror that behavior instead of returning an error.
impl ExecuteQueryBox for QueryBox<QueryOutputBatchBox> {
    fn execute(
        self,
        state: &impl StateReadOnly,
        params: &QueryParams,
    ) -> Result<QueryOutputBatchBox, Error> {
        use iroha_data_model as dm;
        fn decode_query<Q: norito::codec::Decode>(payload: &[u8]) -> Result<Q, Error> {
            let mut cursor = std::io::Cursor::new(payload);
            let query = Q::decode(&mut cursor).map_err(|_| {
                Error::Conversion("failed to decode iterable query payload".to_string())
            })?;
            if usize::try_from(cursor.position()).unwrap_or(usize::MAX) != payload.len() {
                return Err(Error::Conversion(
                    "iterable query payload had trailing bytes".to_string(),
                ));
            }
            Ok(query)
        }

        fn run_dispatch<T, Q>(
            qbox: &QueryBox<QueryOutputBatchBox>,
            state: &impl StateReadOnly,
            params: &QueryParams,
            limits: QueryLimits,
        ) -> Option<Result<QueryOutputBatchBox, Error>>
        where
            T: HasProjection<SelectorMarker, AtomType = ()>
                + HasProjection<PredicateMarker>
                + SortableQueryOutput
                + Send
                + Sync
                + 'static,
            <T as HasProjection<SelectorMarker>>::Projection: EvaluateSelector<T> + Send + Sync,
            Q: super::super::ValidQuery<Item = T> + norito::codec::Decode,
            QueryOutputBatchBox: From<Vec<T>>,
        {
            let erased = dm::query::iter_query_inner::<T>(qbox)?;
            let concrete = match decode_query::<Q>(erased.payload()) {
                Ok(q) => q,
                Err(_) => return None,
            };
            let iter =
                match super::super::ValidQuery::execute(concrete, erased.predicate_cloned(), state)
                {
                    Ok(iter) => iter,
                    Err(err) => return Some(Err(err)),
                };
            let mut batched =
                match apply_query_postprocessing(iter, erased.selector_cloned(), params, limits) {
                    Ok(b) => b,
                    Err(err) => return Some(Err(err)),
                };
            let (tuple, _next) = match batched.next_batch(0) {
                Ok(batch) => batch,
                Err(err) => return Some(Err(err)),
            };
            let batch = tuple
                .into_iter()
                .next()
                .unwrap_or_else(|| QueryOutputBatchBox::from(Vec::<T>::new()));
            Some(Ok(batch))
        }

        let limits = QueryLimits::from_defaults();
        macro_rules! dispatch {
            ($($item:ty => $query:ty),+ $(,)?) => {{
                $(if let Some(out) = run_dispatch::<$item, $query>(&self, state, params, limits) {
                    return out;
                })+
            }};
        }

        dispatch! {
            dm::domain::Domain => dm::query::domain::prelude::FindDomains,
            dm::account::Account => dm::query::account::prelude::FindAccounts,
            dm::asset::value::Asset => dm::query::asset::prelude::FindAssets,
            dm::asset::definition::AssetDefinition =>
                dm::query::asset::prelude::FindAssetsDefinitions,
            dm::repo::RepoAgreement => dm::query::repo::prelude::FindRepoAgreements,
            dm::nft::Nft => dm::query::nft::prelude::FindNfts,
            dm::rwa::Rwa => dm::query::rwa::prelude::FindRwas,
            dm::role::Role => dm::query::role::prelude::FindRoles,
            dm::role::RoleId => dm::query::role::prelude::FindRoleIds,
            dm::peer::PeerId => dm::query::peer::prelude::FindPeers,
            dm::trigger::Trigger => dm::query::trigger::prelude::FindTriggers,
            dm::trigger::TriggerId => dm::query::trigger::prelude::FindActiveTriggerIds,
            dm::block::SignedBlock => dm::query::block::prelude::FindBlocks,
            dm::block::BlockHeader => dm::query::block::prelude::FindBlockHeaders,
            dm::proof::ProofRecord => dm::query::proof::prelude::FindProofRecordsByBackend,
            dm::proof::ProofRecord => dm::query::proof::prelude::FindProofRecordsByStatus,
            dm::proof::ProofRecord => dm::query::proof::prelude::FindProofRecords,
            dm::oracle::FeedConfig => dm::query::oracle::prelude::FindOracleFeeds,
            dm::events::data::oracle::FeedEventRecord =>
                dm::query::oracle::prelude::FindOracleHistoryByFeedId,
            dm::oracle::OracleProviderStatsRecord =>
                dm::query::oracle::prelude::FindOracleProviderStatsByFeedId,
            dm::oracle::OracleDispute => dm::query::oracle::prelude::FindOracleDisputes,
            dm::oracle::OracleChangeProposal => dm::query::oracle::prelude::FindOracleChanges,
            dm::oracle::TwitterBindingRecord =>
                dm::query::oracle::prelude::FindTwitterBindingsByUaid,
            dm::oracle::DefiOracleAttestation =>
                dm::query::oracle::prelude::FindDefiOracleAttestationsByKey,
            dm::query::CommittedTransaction => dm::query::transaction::prelude::FindTransactions,
            dm::escrow::AssetEscrowRecord => dm::query::escrow::prelude::FindAssetEscrows,
            dm::escrow::AnonymousAssetEscrowRecord =>
                dm::query::escrow::prelude::FindAnonymousAssetEscrows,
            dm::nexus::FeeSponsorPolicy =>
                dm::query::nexus::prelude::FindFeeSponsorPoliciesBySponsor,
            dm::nexus::FeeSponsorPolicy => dm::query::nexus::prelude::FindFeeSponsorPolicies,
            dm::nexus::FeeSponsorPolicyId => dm::query::nexus::prelude::FindFeeSponsorPolicyIds,
        }

        Err(Error::Conversion(
            "dynamic QueryBox execution type not supported".to_string(),
        ))
    }
}

impl SortableQueryOutput for iroha_data_model::block::SignedBlock {
    type TiebreakKey = Vec<u8>;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        norito::codec::Encode::encode(self)
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_encoded_vec_tiebreak_len(self, limit)
    }
}

impl SortableQueryOutput for iroha_data_model::block::BlockHeader {
    type TiebreakKey = Vec<u8>;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        norito::codec::Encode::encode(self)
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_encoded_vec_tiebreak_len(self, limit)
    }
}

impl SortableQueryOutput for iroha_data_model::proof::ProofRecord {
    type TiebreakKey = iroha_data_model::proof::ProofId;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.id.clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(&self.id, limit)
    }

    fn tiebreak_cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl SortableQueryOutput for iroha_data_model::oracle::FeedConfig {
    type TiebreakKey = iroha_data_model::oracle::FeedId;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.feed_id.clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(&self.feed_id, limit)
    }
}

impl SortableQueryOutput for iroha_data_model::events::data::oracle::FeedEventRecord {
    type TiebreakKey = Vec<u8>;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        norito::codec::Encode::encode(&self.event)
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_encoded_vec_tiebreak_len(&self.event, limit)
    }
}

impl SortableQueryOutput for iroha_data_model::oracle::OracleProviderStatsRecord {
    type TiebreakKey = iroha_data_model::oracle::OracleProviderKey;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.key.clone()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(&self.key, limit)
    }
}

impl SortableQueryOutput for iroha_data_model::oracle::OracleDispute {
    type TiebreakKey = iroha_data_model::oracle::OracleDisputeId;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.id
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(&self.id, limit)
    }
}

impl SortableQueryOutput for iroha_data_model::oracle::OracleChangeProposal {
    type TiebreakKey = iroha_data_model::oracle::OracleChangeId;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.id
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(&self.id, limit)
    }
}

impl SortableQueryOutput for iroha_data_model::oracle::TwitterBindingRecord {
    type TiebreakKey = iroha_crypto::Hash;

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        self.binding_digest()
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(&self.binding_digest(), limit)
    }
}

impl SortableQueryOutput for iroha_data_model::oracle::DefiOracleAttestation {
    type TiebreakKey = (iroha_data_model::oracle::DefiOracleAttestationKey, u64);

    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
        None
    }

    fn tiebreak_key(&self) -> Self::TiebreakKey {
        (self.key, self.oracle_slot)
    }

    fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
        bounded_bare_encoded_len(&(self.key, self.oracle_slot), limit)
    }
}

/// Applies sorting and pagination to the query output and wraps it into a type-erasing batching iterator.
///
/// # Errors
///
/// Returns an error if the fetch size exceeds the configured limits.
pub fn apply_query_postprocessing<I>(
    iter: I,
    selector: SelectorTuple<I::Item>,
    params: &QueryParams,
    limits: QueryLimits,
) -> Result<ErasedQueryIterator, Error>
where
    I: Iterator<Item: SortableQueryOutput>,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
    <I::Item as HasProjection<SelectorMarker>>::Projection: EvaluateSelector<I::Item> + Send + Sync,
    QueryOutputBatchBox: From<Vec<I::Item>>,
{
    let (output, _processed_items) =
        apply_query_postprocessing_with_budget(iter, selector, params, limits, None)?;
    Ok(output)
}

fn compare_sorted_query_keys<K: Ord>(
    left_key: Option<&Json>,
    left_tiebreak: &K,
    right_key: Option<&Json>,
    right_tiebreak: &K,
    order: SortOrder,
) -> core::cmp::Ordering {
    use core::cmp::Ordering::*;

    match (left_key, right_key) {
        (None, None) => left_tiebreak.cmp(right_tiebreak),
        (None, Some(_)) => Greater,
        (Some(_), None) => Less,
        (Some(left_key), Some(right_key)) => {
            let primary = match order {
                SortOrder::Asc => left_key.cmp(right_key),
                SortOrder::Desc => right_key.cmp(left_key),
            };
            if primary == Equal {
                left_tiebreak.cmp(right_tiebreak)
            } else {
                primary
            }
        }
    }
}

fn compare_sorted_query_indices<T: SortableQueryOutput>(
    left_index: usize,
    right_index: usize,
    sort_keys: &[Option<Json>],
    tiebreak_keys: &[T::TiebreakKey],
    order: SortOrder,
) -> core::cmp::Ordering {
    compare_sorted_query_keys(
        sort_keys[left_index].as_ref(),
        &tiebreak_keys[left_index],
        sort_keys[right_index].as_ref(),
        &tiebreak_keys[right_index],
        order,
    )
}

struct EphemeralSortedEntry<T: SortableQueryOutput> {
    value: T,
    sort_key: Option<Json>,
    tiebreak_key: T::TiebreakKey,
    order: SortOrder,
}

impl<T: SortableQueryOutput> PartialEq for EphemeralSortedEntry<T> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == core::cmp::Ordering::Equal
    }
}

impl<T: SortableQueryOutput> Eq for EphemeralSortedEntry<T> {}

impl<T: SortableQueryOutput> PartialOrd for EphemeralSortedEntry<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: SortableQueryOutput> Ord for EphemeralSortedEntry<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        compare_sorted_query_keys(
            self.sort_key.as_ref(),
            &self.tiebreak_key,
            other.sort_key.as_ref(),
            &other.tiebreak_key,
            self.order,
        )
    }
}

const STREAMING_SORTED_PREFIX_LIMIT: usize = 4096;

struct ScannedTransactionPage {
    values: Vec<CommittedTransaction>,
    remaining_items: Option<u64>,
    has_more: bool,
    stats: QueryExecutionStats,
    history_cursor: Option<TransactionHistoryCursor>,
}

struct TransactionReplayCheckpoint {
    history_cursor: TransactionHistoryCursor,
    remaining_items: Option<u64>,
}

fn transaction_fetch_size(params: &QueryParams, limits: QueryLimits) -> Result<NonZeroU64, Error> {
    let fetch_size = params.fetch_size.fetch_size.unwrap_or(DEFAULT_FETCH_SIZE);
    if fetch_size.get() > limits.max_fetch_size {
        return Err(Error::FetchSizeTooBig);
    }
    Ok(fetch_size)
}

fn scan_unsorted_transaction_page(
    state: &impl StateReadOnly,
    filter: &CompoundPredicate<CommittedTransaction>,
    params: &QueryParams,
    limits: QueryLimits,
    returned_offset: u64,
    anchor: TransactionHistoryAnchor,
    history_cursor: Option<TransactionHistoryCursor>,
    known_remaining_items: Option<u64>,
    budget_items: Option<u64>,
    execution_budget: Option<QueryExecutionBudget>,
) -> Result<ScannedTransactionPage, Error> {
    let fetch_size = transaction_fetch_size(params, limits)?;
    let remaining_limit = params
        .pagination
        .limit_value()
        .map(|limit| limit.get().saturating_sub(returned_offset));
    if remaining_limit == Some(0) {
        return Err(Error::CursorDone);
    }

    let exact = limits.count_mode == QueryCountMode::Exact;
    if known_remaining_items.is_some() && !exact {
        return Err(Error::Conversion(
            "bounded transaction scan received an exact remaining count".to_owned(),
        ));
    }
    let fetch_size = usize::try_from(fetch_size.get()).unwrap_or(usize::MAX);
    let page_capacity = remaining_limit.map_or(fetch_size, |limit| {
        usize::try_from(limit).unwrap_or(usize::MAX).min(fetch_size)
    });
    let page_capacity = known_remaining_items.map_or(page_capacity, |remaining| {
        usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(page_capacity)
    });
    let source_offset = history_cursor
        .is_none()
        .then(|| params.pagination.offset_value())
        .unwrap_or(0);
    let scan_limit = remaining_limit;
    let limit_allows_probe = remaining_limit
        .is_none_or(|remaining| remaining > u64::try_from(page_capacity).unwrap_or(u64::MAX));
    let exact_full_scan = exact && known_remaining_items.is_none();

    let mut matched = 0_u64;
    let processed_items = Cell::new(0_u64);
    let processed_bytes = Cell::new(0_u64);
    let mut values = Vec::with_capacity(page_capacity.min(1024));
    let mut has_more = false;
    let mut next_history_cursor = None;
    visit_committed_transactions(
        state,
        filter,
        anchor,
        history_cursor,
        |projection_work| {
            let next_items = processed_items.get().saturating_add(projection_work);
            processed_items.set(next_items);
            if budget_items.is_some_and(|budget| next_items > budget) {
                return Err(Error::GasBudgetExceeded);
            }
            if let Some(budget) = execution_budget {
                budget.ensure(next_items, processed_bytes.get())?;
            }
            Ok(())
        },
        |transaction, matches, cursor_after| {
            let mut current_stats = QueryExecutionStats {
                processed_items: processed_items.get(),
                processed_bytes: processed_bytes.get(),
            };
            current_stats.record_skipped_value(&transaction, execution_budget)?;
            processed_bytes.set(current_stats.processed_bytes);
            if !matches {
                return Ok(ControlFlow::Continue(()));
            }

            let position = matched;
            matched = matched.saturating_add(1);

            let Some(after_offset) = position.checked_sub(source_offset) else {
                return Ok(ControlFlow::Continue(()));
            };
            if scan_limit.is_some_and(|limit| after_offset >= limit) {
                // Exact mode continues to validate every selected carrier even after
                // the requested pagination window is fully counted.
                return Ok(if exact_full_scan {
                    ControlFlow::Continue(())
                } else {
                    ControlFlow::Break(())
                });
            }

            if values.len() < page_capacity {
                values.push(transaction);
                if values.len() == page_capacity {
                    next_history_cursor = Some(cursor_after);
                }
                if values.len() == page_capacity
                    && (known_remaining_items.is_some()
                        || (!exact_full_scan && !limit_allows_probe))
                {
                    return Ok(ControlFlow::Break(()));
                }
                return Ok(ControlFlow::Continue(()));
            }

            if !exact_full_scan {
                has_more = true;
                return Ok(ControlFlow::Break(()));
            }
            Ok(ControlFlow::Continue(()))
        },
    )?;
    let stats = QueryExecutionStats {
        processed_items: processed_items.get(),
        processed_bytes: processed_bytes.get(),
    };

    if let Some(known_remaining_items) = known_remaining_items {
        let returned = u64::try_from(values.len()).unwrap_or(u64::MAX);
        let expected = known_remaining_items
            .min(scan_limit.unwrap_or(u64::MAX))
            .min(u64::try_from(fetch_size).unwrap_or(u64::MAX));
        if returned != expected {
            return Err(Error::Expired);
        }
        let remaining_items = known_remaining_items.saturating_sub(returned);
        has_more = remaining_items > 0;
        Ok(ScannedTransactionPage {
            values,
            remaining_items: Some(remaining_items),
            has_more,
            stats,
            history_cursor: next_history_cursor,
        })
    } else if exact {
        let total_after_pagination = matched
            .saturating_sub(source_offset)
            .min(scan_limit.unwrap_or(u64::MAX));
        let returned = u64::try_from(values.len()).unwrap_or(u64::MAX);
        let remaining_items = total_after_pagination.saturating_sub(returned);
        has_more = remaining_items > 0;
        Ok(ScannedTransactionPage {
            values,
            remaining_items: Some(remaining_items),
            has_more,
            stats,
            history_cursor: next_history_cursor,
        })
    } else {
        Ok(ScannedTransactionPage {
            values,
            remaining_items: None,
            has_more,
            stats,
            history_cursor: next_history_cursor,
        })
    }
}

fn transaction_sorted_prefix_keep(
    params: &QueryParams,
    limits: QueryLimits,
    returned_offset: u64,
) -> Result<usize, Error> {
    let fetch_size = transaction_fetch_size(params, limits)?;
    let remaining_limit = params
        .pagination
        .limit_value()
        .map(|limit| limit.get().saturating_sub(returned_offset));
    if remaining_limit == Some(0) {
        return Err(Error::CursorDone);
    }
    let take = remaining_limit.map_or(fetch_size.get(), |limit| limit.min(fetch_size.get()));
    let keep = params
        .pagination
        .offset_value()
        .saturating_add(returned_offset)
        .saturating_add(take);
    Ok(usize::try_from(keep).unwrap_or(usize::MAX))
}

#[allow(clippy::too_many_arguments)]
fn collect_sorted_transaction_prefix(
    state: &impl StateReadOnly,
    filter: &CompoundPredicate<CommittedTransaction>,
    key: &Name,
    order: SortOrder,
    keep: usize,
    anchor: TransactionHistoryAnchor,
    budget_items: Option<u64>,
    execution_budget: Option<QueryExecutionBudget>,
) -> Result<(Vec<CommittedTransaction>, u64, QueryExecutionStats), Error> {
    if keep > STREAMING_SORTED_PREFIX_LIMIT {
        return Err(Error::GasBudgetExceeded);
    }
    let mut heap = BinaryHeap::with_capacity(keep);
    let mut matched = 0_u64;
    let processed_items = Cell::new(0_u64);
    let processed_bytes = Cell::new(0_u64);

    visit_committed_transactions(
        state,
        filter,
        anchor,
        None,
        |projection_work| {
            let next_items = processed_items.get().saturating_add(projection_work);
            processed_items.set(next_items);
            if budget_items.is_some_and(|budget| next_items > budget) {
                return Err(Error::GasBudgetExceeded);
            }
            if let Some(budget) = execution_budget {
                budget.ensure(next_items, processed_bytes.get())?;
            }
            Ok(())
        },
        |value, matches, _| {
            let mut current_stats = QueryExecutionStats {
                processed_items: processed_items.get(),
                processed_bytes: processed_bytes.get(),
            };
            current_stats.record_skipped_value(&value, execution_budget)?;
            if !matches {
                processed_bytes.set(current_stats.processed_bytes);
                return Ok(ControlFlow::Continue(()));
            }
            matched = matched.saturating_add(1);
            if keep == 0 {
                processed_bytes.set(current_stats.processed_bytes);
                return Ok(ControlFlow::Continue(()));
            }
            let sort_key = value.get_metadata_sorting_key(key);
            if let Some(sort_key) = sort_key {
                current_stats.record_skipped_value(sort_key, execution_budget)?;
            }
            let tiebreak_key =
                materialize_admitted_tiebreak_key(&value, &mut current_stats, execution_budget)?;
            processed_bytes.set(current_stats.processed_bytes);
            let entry = EphemeralSortedEntry {
                sort_key: sort_key.cloned(),
                tiebreak_key,
                value,
                order,
            };
            if heap.len() < keep {
                heap.push(entry);
            } else if heap
                .peek()
                .is_some_and(|worst| entry.cmp(worst) == core::cmp::Ordering::Less)
            {
                let _ = heap.pop();
                heap.push(entry);
            }
            Ok(ControlFlow::Continue(()))
        },
    )?;
    let stats = QueryExecutionStats {
        processed_items: processed_items.get(),
        processed_bytes: processed_bytes.get(),
    };

    let mut entries = heap.into_vec();
    entries.sort_unstable();
    Ok((
        entries.into_iter().map(|entry| entry.value).collect(),
        matched,
        stats,
    ))
}

fn scan_sorted_transaction_page(
    state: &impl StateReadOnly,
    filter: &CompoundPredicate<CommittedTransaction>,
    params: &QueryParams,
    limits: QueryLimits,
    returned_offset: u64,
    anchor: TransactionHistoryAnchor,
    budget_items: Option<u64>,
    execution_budget: Option<QueryExecutionBudget>,
) -> Result<ScannedTransactionPage, Error> {
    let keep = transaction_sorted_prefix_keep(params, limits, returned_offset)?;
    let key = params
        .sorting
        .sort_by_metadata_key
        .as_ref()
        .ok_or_else(|| Error::Conversion("sorted transaction scan has no metadata key".into()))?;
    let order = params.sorting.order.unwrap_or(SortOrder::Asc);
    let (entries, matched, stats) = collect_sorted_transaction_prefix(
        state,
        filter,
        key,
        order,
        keep,
        anchor,
        budget_items,
        execution_budget,
    )?;

    let offset = params
        .pagination
        .offset_value()
        .saturating_add(returned_offset);
    let offset = usize::try_from(offset).unwrap_or(usize::MAX);
    let values = entries.into_iter().skip(offset).collect::<Vec<_>>();
    let total_after_pagination = matched
        .saturating_sub(params.pagination.offset_value())
        .min(
            params
                .pagination
                .limit_value()
                .map_or(u64::MAX, NonZeroU64::get),
        );
    let returned = returned_offset.saturating_add(u64::try_from(values.len()).unwrap_or(u64::MAX));
    let remaining_items = total_after_pagination.saturating_sub(returned);

    let exact = limits.count_mode == QueryCountMode::Exact;
    Ok(ScannedTransactionPage {
        values,
        remaining_items: exact.then_some(remaining_items),
        has_more: remaining_items > 0,
        stats,
        history_cursor: None,
    })
}

fn materialize_sorted_transaction_window(
    state: &impl StateReadOnly,
    filter: &CompoundPredicate<CommittedTransaction>,
    params: &QueryParams,
    anchor: TransactionHistoryAnchor,
    budget_items: Option<u64>,
) -> Result<Vec<CommittedTransaction>, Error> {
    let limit = params
        .pagination
        .limit_value()
        .ok_or(Error::GasBudgetExceeded)?;
    let offset = params.pagination.offset_value();
    let keep = offset.saturating_add(limit.get());
    let keep = usize::try_from(keep).map_err(|_| Error::GasBudgetExceeded)?;
    let key = params
        .sorting
        .sort_by_metadata_key
        .as_ref()
        .ok_or_else(|| Error::Conversion("sorted transaction scan has no metadata key".into()))?;
    let order = params.sorting.order.unwrap_or(SortOrder::Asc);
    let (prefix, _, _) = collect_sorted_transaction_prefix(
        state,
        filter,
        key,
        order,
        keep,
        anchor,
        budget_items,
        None,
    )?;
    let offset = usize::try_from(offset).unwrap_or(usize::MAX);
    let limit = usize::try_from(limit.get()).unwrap_or(usize::MAX);
    Ok(prefix.into_iter().skip(offset).take(limit).collect())
}

fn project_transaction_page(
    values: Vec<CommittedTransaction>,
    selector: SelectorTuple<CommittedTransaction>,
    fetch_size: NonZeroU64,
) -> Result<QueryOutputBatchBoxTuple, Error> {
    let mut iter = ErasedQueryIterator::new(values.into_iter(), selector, fetch_size);
    iter.next_batch(0).map(|(batch, _)| batch)
}

fn prepare_materialized_transaction_start(
    values: Vec<CommittedTransaction>,
    selector: SelectorTuple<CommittedTransaction>,
    fetch_size: NonZeroU64,
    count_mode: QueryCountMode,
) -> Result<PreparedQueryStart, Error> {
    let fetch_size_usize = usize::try_from(fetch_size.get()).unwrap_or(usize::MAX);
    let mut values = values.into_iter();
    let first_values = values.by_ref().take(fetch_size_usize).collect::<Vec<_>>();
    let first_len = first_values.len();
    let deferred_values = values.collect::<Vec<_>>();
    let remaining = u64::try_from(deferred_values.len()).unwrap_or(u64::MAX);
    let first_batch = project_transaction_page(first_values, selector.clone(), fetch_size)?;
    let reported_remaining = (count_mode == QueryCountMode::Exact).then_some(remaining);
    if deferred_values.is_empty() {
        return Ok(PreparedQueryStart {
            first_batch,
            remaining_items: reported_remaining,
            deferred_continuation: None,
        });
    }

    let first_cursor =
        NonZeroU64::new(u64::try_from(first_len).unwrap_or(u64::MAX)).ok_or_else(|| {
            Error::Conversion("materialized transaction window has an empty first page".to_owned())
        })?;
    let deferred_continuation = DeferredQueryContinuation::new(
        first_cursor,
        reported_remaining,
        move || match count_mode {
            QueryCountMode::Exact => ErasedQueryIterator::new_with_cursor(
                deferred_values.into_iter(),
                selector,
                fetch_size,
                first_cursor.get(),
            ),
            QueryCountMode::Bounded => ErasedQueryIterator::new_streaming_with_cursor(
                deferred_values.into_iter(),
                selector,
                fetch_size,
                first_cursor.get(),
            ),
        },
    );
    Ok(PreparedQueryStart {
        first_batch,
        remaining_items: reported_remaining,
        deferred_continuation: Some(deferred_continuation),
    })
}

fn validate_transaction_sorted_materialization_budget(
    params: &QueryParams,
    limits: QueryLimits,
) -> Result<(), Error> {
    let _ = transaction_fetch_size(params, limits)?;
    if params.sorting.sort_by_metadata_key.is_none() {
        return Ok(());
    }

    // Sorting requires a global scan. Require a finite, bounded prefix so a
    // client cannot turn the transaction query into history-sized retained
    // materialization. `GasBudgetExceeded` is also the public materialization-
    // budget error.
    let Some(limit) = params.pagination.limit_value() else {
        return Err(Error::GasBudgetExceeded);
    };
    let keep = params.pagination.offset_value().saturating_add(limit.get());
    if !usize::try_from(keep).is_ok_and(|keep| keep <= STREAMING_SORTED_PREFIX_LIMIT) {
        return Err(Error::GasBudgetExceeded);
    }
    Ok(())
}

fn scan_transaction_page(
    state: &impl StateReadOnly,
    filter: &CompoundPredicate<CommittedTransaction>,
    params: &QueryParams,
    limits: QueryLimits,
    returned_offset: u64,
    anchor: TransactionHistoryAnchor,
    history_cursor: Option<TransactionHistoryCursor>,
    known_remaining_items: Option<u64>,
    budget_items: Option<u64>,
    execution_budget: Option<QueryExecutionBudget>,
) -> Result<ScannedTransactionPage, Error> {
    if params.sorting.sort_by_metadata_key.is_some() {
        if history_cursor.is_some() || known_remaining_items.is_some() {
            return Err(Error::Conversion(
                "sorted transaction scans do not accept history cursors".to_owned(),
            ));
        }
        scan_sorted_transaction_page(
            state,
            filter,
            params,
            limits,
            returned_offset,
            anchor,
            budget_items,
            execution_budget,
        )
    } else {
        scan_unsorted_transaction_page(
            state,
            filter,
            params,
            limits,
            returned_offset,
            anchor,
            history_cursor,
            known_remaining_items,
            budget_items,
            execution_budget,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn try_handle_find_transactions_stored(
    state: &impl StateReadOnly,
    filter: CompoundPredicate<CommittedTransaction>,
    selector: SelectorTuple<CommittedTransaction>,
    params: &QueryParams,
    limits: QueryLimits,
    live_query_store: &LiveQueryStoreHandle,
    authority: &AccountId,
    gas_budget: Option<u64>,
    replay_state: Option<Weak<State>>,
) -> Result<QueryOutput, Error> {
    validate_transaction_sorted_materialization_budget(params, limits)?;

    let fixed_anchor = TransactionHistoryAnchor::capture(state);
    let fetch_size = transaction_fetch_size(params, limits)?;

    if params.sorting.sort_by_metadata_key.is_some() {
        let values = materialize_sorted_transaction_window(
            state,
            &filter,
            params,
            fixed_anchor,
            gas_budget,
        )?;
        let continuation_required =
            values.len() > usize::try_from(fetch_size.get()).unwrap_or(usize::MAX);
        if continuation_required && replay_state.is_none() {
            return Err(Error::Conversion(
                "FindTransactions continuation requires replay-capable stored query state"
                    .to_owned(),
            ));
        }
        let prepared = prepare_materialized_transaction_start(
            values,
            selector,
            fetch_size,
            limits.count_mode,
        )?;
        return live_query_store.handle_iter_start_prepared(prepared, authority, gas_budget);
    }

    let page = scan_transaction_page(
        state,
        &filter,
        params,
        limits,
        0,
        fixed_anchor,
        None,
        None,
        gas_budget,
        None,
    )?;
    let page_len = page.values.len();
    let first_batch = project_transaction_page(page.values, selector.clone(), fetch_size)?;
    let continuation_required = page.has_more;

    if !continuation_required {
        return live_query_store.handle_iter_start_prepared(
            PreparedQueryStart {
                first_batch,
                remaining_items: page.remaining_items,
                deferred_continuation: None,
            },
            authority,
            gas_budget,
        );
    }

    let Some(replay_state) = replay_state else {
        return Err(Error::Conversion(
            "FindTransactions continuation requires replay-capable stored query state".to_owned(),
        ));
    };
    let history_cursor = page.history_cursor.ok_or_else(|| {
        Error::Conversion("transaction continuation omitted its history checkpoint".to_owned())
    })?;

    let first_cursor =
        NonZeroU64::new(u64::try_from(page_len).unwrap_or(u64::MAX)).ok_or_else(|| {
            Error::Conversion("transaction continuation has an empty first page".to_owned())
        })?;
    let params_for_replay = params.clone();
    let filter_for_replay = filter.clone();
    let selector_for_replay = selector;
    let checkpoint = Arc::new(Mutex::new(TransactionReplayCheckpoint {
        history_cursor,
        remaining_items: page.remaining_items,
    }));
    let make_next_page = Arc::new(move |cursor: u64, gas_budget: Option<u64>| {
        let state = replay_state.upgrade().ok_or(Error::Expired)?;
        let view = state.query_view();
        let mut checkpoint = checkpoint.lock().map_err(|_| {
            Error::Conversion("transaction replay checkpoint lock is poisoned".to_owned())
        })?;
        let page = scan_transaction_page(
            &view,
            &filter_for_replay,
            &params_for_replay,
            limits,
            cursor,
            fixed_anchor,
            Some(checkpoint.history_cursor),
            checkpoint.remaining_items,
            gas_budget,
            None,
        )?;
        let page_len = u64::try_from(page.values.len()).unwrap_or(u64::MAX);
        let batch = project_transaction_page(page.values, selector_for_replay.clone(), fetch_size)?;
        let next_cursor = if page.has_more {
            if page_len == 0 {
                return Err(Error::Conversion(
                    "transaction continuation made no progress".to_owned(),
                ));
            }
            Some(
                cursor
                    .checked_add(page_len)
                    .and_then(NonZeroU64::new)
                    .ok_or_else(|| {
                        Error::Conversion("transaction continuation cursor overflowed".to_owned())
                    })?,
            )
        } else {
            None
        };
        if page.has_more {
            checkpoint.history_cursor = page.history_cursor.ok_or_else(|| {
                Error::Conversion(
                    "transaction continuation omitted its next history checkpoint".to_owned(),
                )
            })?;
        }
        checkpoint.remaining_items = page.remaining_items;
        Ok((batch, page.remaining_items, next_cursor))
    });

    let paged_continuation = if let Some(remaining_items) = page.remaining_items {
        let make_next_page = Arc::clone(&make_next_page);
        PagedQueryContinuation::new_counted_budgeted(
            first_cursor,
            remaining_items,
            move |cursor, gas_budget| {
                let (batch, remaining_items, next_cursor) = make_next_page(cursor, gas_budget)?;
                let remaining_items = remaining_items.ok_or_else(|| {
                    Error::Conversion(
                        "exact transaction page omitted its remaining count".to_owned(),
                    )
                })?;
                Ok((batch, remaining_items, next_cursor))
            },
        )
    } else {
        let make_next_page = Arc::clone(&make_next_page);
        PagedQueryContinuation::new_budgeted(first_cursor, move |cursor, gas_budget| {
            let (batch, _, next_cursor) = make_next_page(cursor, gas_budget)?;
            Ok((batch, next_cursor))
        })
    };

    live_query_store.handle_iter_start_paged_prepared(
        PreparedPagedQueryStart {
            first_batch,
            paged_continuation: Some(paged_continuation),
        },
        authority,
        gas_budget,
    )
}

fn try_handle_find_transactions_ephemeral(
    state: &impl StateReadOnly,
    filter: &CompoundPredicate<CommittedTransaction>,
    selector: SelectorTuple<CommittedTransaction>,
    params: &QueryParams,
    limits: QueryLimits,
    budget: Option<QueryExecutionBudget>,
) -> Result<(QueryOutput, QueryExecutionStats), Error> {
    validate_transaction_sorted_materialization_budget(params, limits)?;
    let page = scan_transaction_page(
        state,
        filter,
        params,
        limits,
        0,
        TransactionHistoryAnchor::capture(state),
        None,
        None,
        budget.map(QueryExecutionBudget::max_items),
        budget,
    )?;
    let batch = project_transaction_page(
        page.values,
        selector,
        transaction_fetch_size(params, limits)?,
    )?;
    let output = match page.remaining_items {
        Some(remaining_items) => QueryOutput::new(batch, remaining_items, None),
        None => QueryOutput::new_bounded(batch, page.has_more, None),
    };
    Ok((output, page.stats))
}

#[allow(clippy::too_many_arguments)]
fn handle_find_transactions_stored(
    state: &impl StateReadOnly,
    filter: CompoundPredicate<CommittedTransaction>,
    selector: SelectorTuple<CommittedTransaction>,
    params: &QueryParams,
    limits: QueryLimits,
    live_query_store: &LiveQueryStoreHandle,
    authority: &AccountId,
    gas_budget: Option<u64>,
    replay_state: Option<Weak<State>>,
) -> Result<QueryOutput, Error> {
    try_handle_find_transactions_stored(
        state,
        filter,
        selector,
        params,
        limits,
        live_query_store,
        authority,
        gas_budget,
        replay_state,
    )
}

fn handle_find_transactions_ephemeral(
    state: &impl StateReadOnly,
    filter: CompoundPredicate<CommittedTransaction>,
    selector: SelectorTuple<CommittedTransaction>,
    params: &QueryParams,
    limits: QueryLimits,
    budget: Option<QueryExecutionBudget>,
) -> Result<(QueryOutput, QueryExecutionStats), Error> {
    try_handle_find_transactions_ephemeral(state, &filter, selector, params, limits, budget)
}

fn collect_ephemeral_sorted_prefix<I>(
    iter: I,
    key: &Name,
    order: SortOrder,
    keep: usize,
    budget: Option<QueryExecutionBudget>,
    stats: &mut QueryExecutionStats,
) -> Result<(Vec<I::Item>, u64), Error>
where
    I: Iterator,
    I::Item: SortableQueryOutput + NoritoSerialize,
{
    let mut count = 0_u64;
    if keep == 0 {
        for value in iter {
            count = count.saturating_add(1);
            stats.record_item(&value, budget)?;
        }
        return Ok((Vec::new(), count));
    }

    let mut heap = BinaryHeap::with_capacity(keep);
    for value in iter {
        count = count.saturating_add(1);
        stats.record_item(&value, budget)?;
        let sort_key = value.get_metadata_sorting_key(key);
        if let Some(sort_key) = sort_key {
            stats.record_skipped_value(sort_key, budget)?;
        }
        let sort_key = sort_key.cloned();
        let tiebreak_key = materialize_admitted_tiebreak_key(&value, stats, budget)?;

        let entry = EphemeralSortedEntry {
            sort_key,
            tiebreak_key,
            value,
            order,
        };

        if heap.len() < keep {
            heap.push(entry);
            continue;
        }

        let should_replace = heap
            .peek()
            .is_some_and(|worst| entry.cmp(worst) == core::cmp::Ordering::Less);
        if should_replace {
            let _ = heap.pop();
            heap.push(entry);
        }
    }

    let mut entries = heap.into_vec();
    entries.sort_unstable();
    Ok((
        entries.into_iter().map(|entry| entry.value).collect(),
        count,
    ))
}

#[derive(Debug, Clone)]
struct StoredSortedFastStartParams {
    key: Name,
    order: SortOrder,
    fetch_size: NonZeroU64,
    offset: usize,
    offset_u64: u64,
    limit: usize,
    keep: usize,
}

fn stored_sorted_fast_start_params(
    params: &QueryParams,
    limits: QueryLimits,
) -> Result<Option<StoredSortedFastStartParams>, Error> {
    let fetch_size = params
        .fetch_size
        .fetch_size
        .unwrap_or(iroha_data_model::query::parameters::DEFAULT_FETCH_SIZE);
    if fetch_size.get() > limits.max_fetch_size {
        return Err(Error::FetchSizeTooBig);
    }

    let Some(key) = params.sorting.sort_by_metadata_key.clone() else {
        return Ok(None);
    };
    let offset_u64 = params.pagination.offset_value();
    let offset = usize::try_from(offset_u64).unwrap_or(usize::MAX);
    let limit = params.pagination.limit_value().map_or(usize::MAX, |limit| {
        usize::try_from(limit.get()).unwrap_or(usize::MAX)
    });
    let fetch_size_usize = usize::try_from(fetch_size.get()).unwrap_or(usize::MAX);
    let keep = offset.saturating_add(limit.min(fetch_size_usize));
    if keep > STREAMING_SORTED_PREFIX_LIMIT {
        return Ok(None);
    }

    Ok(Some(StoredSortedFastStartParams {
        key,
        order: params.sorting.order.unwrap_or(SortOrder::Asc),
        fetch_size,
        offset,
        offset_u64,
        limit,
        keep,
    }))
}

fn prepare_stored_sorted_start<I>(
    iter: I,
    selector: SelectorTuple<I::Item>,
    fast: StoredSortedFastStartParams,
    budget_items: Option<u64>,
) -> Result<PreparedQueryStart, Error>
where
    I: Iterator<Item: SortableQueryOutput>,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
    <I::Item as HasProjection<SelectorMarker>>::Projection: EvaluateSelector<I::Item> + Send + Sync,
    QueryOutputBatchBox: From<Vec<I::Item>>,
{
    let StoredSortedFastStartParams {
        key,
        order,
        fetch_size,
        offset,
        offset_u64,
        limit,
        keep,
    } = fast;

    let mut count = 0_u64;
    let mut overflow_values = Vec::new();
    let mut heap = BinaryHeap::with_capacity(keep);

    for value in iter {
        count = count.saturating_add(1);
        if budget_items.is_some_and(|limit| count > limit) {
            return Err(Error::GasBudgetExceeded);
        }

        if keep == 0 {
            overflow_values.push(value);
            continue;
        }

        let entry = EphemeralSortedEntry {
            sort_key: value.get_metadata_sorting_key(&key).cloned(),
            tiebreak_key: value.tiebreak_key(),
            value,
            order,
        };

        if heap.len() < keep {
            heap.push(entry);
            continue;
        }

        let should_replace = heap
            .peek()
            .is_some_and(|worst| entry.cmp(worst) == core::cmp::Ordering::Less);
        if should_replace {
            let dropped = heap
                .pop()
                .expect("heap contains an item when replacement is requested");
            overflow_values.push(dropped.value);
            heap.push(entry);
        } else {
            overflow_values.push(entry.value);
        }
    }

    let total_after_pagination = usize::try_from(count)
        .unwrap_or(usize::MAX)
        .saturating_sub(offset)
        .min(limit);
    let batch_len =
        total_after_pagination.min(usize::try_from(fetch_size.get()).unwrap_or(usize::MAX));

    let mut prefix_entries = heap.into_vec();
    prefix_entries.sort_unstable();

    let mut first_batch_values = Vec::with_capacity(batch_len);
    let mut deferred_raw_values = Vec::with_capacity(overflow_values.len().saturating_add(offset));
    for (index, entry) in prefix_entries.into_iter().enumerate() {
        if index < offset {
            deferred_raw_values.push(entry.value);
        } else if first_batch_values.len() < batch_len {
            first_batch_values.push(entry.value);
        } else {
            deferred_raw_values.push(entry.value);
        }
    }
    deferred_raw_values.append(&mut overflow_values);

    let selector_for_deferred = selector.clone();
    let mut batch_iter =
        ErasedQueryIterator::new(first_batch_values.into_iter(), selector, fetch_size);
    let (first_batch, _next) = batch_iter.next_batch(0)?;

    let remaining_items =
        u64::try_from(total_after_pagination.saturating_sub(batch_len)).unwrap_or(u64::MAX);
    if remaining_items == 0 {
        return Ok(PreparedQueryStart {
            first_batch,
            remaining_items: Some(remaining_items),
            deferred_continuation: None,
        });
    }

    let first_cursor = NonZeroU64::new(u64::try_from(batch_len).unwrap_or(u64::MAX))
        .expect("non-empty first batch is required for continuation");
    let continuation_limit =
        NonZeroU64::new(remaining_items).expect("continuation limit must be non-zero");
    let deferred_continuation =
        DeferredQueryContinuation::new(first_cursor, Some(remaining_items), move || {
            let mut values = Vec::with_capacity(deferred_raw_values.len());
            let mut sort_keys = Vec::with_capacity(deferred_raw_values.len());
            let mut tiebreak_keys = Vec::with_capacity(deferred_raw_values.len());
            for value in deferred_raw_values {
                sort_keys.push(value.get_metadata_sorting_key(&key).cloned());
                tiebreak_keys.push(value.tiebreak_key());
                values.push(Some(value));
            }

            let pagination = iroha_data_model::query::parameters::Pagination {
                offset: offset_u64,
                limit: Some(continuation_limit),
            };
            ErasedQueryIterator::new_with_cursor(
                IncrementalSortedValues::new(
                    values,
                    sort_keys,
                    tiebreak_keys,
                    pagination,
                    fetch_size,
                    order,
                ),
                selector_for_deferred,
                fetch_size,
                first_cursor.get(),
            )
        });

    Ok(PreparedQueryStart {
        first_batch,
        remaining_items: Some(remaining_items),
        deferred_continuation: Some(deferred_continuation),
    })
}

fn prepare_stored_unsorted_bounded_start<I>(
    iter: I,
    selector: SelectorTuple<I::Item>,
    params: &QueryParams,
    limits: QueryLimits,
) -> Result<PreparedQueryStart, Error>
where
    I: Iterator<Item: SortableQueryOutput>,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
    <I::Item as HasProjection<SelectorMarker>>::Projection: EvaluateSelector<I::Item> + Send + Sync,
    QueryOutputBatchBox: From<Vec<I::Item>>,
{
    let fetch_size = params
        .fetch_size
        .fetch_size
        .unwrap_or(iroha_data_model::query::parameters::DEFAULT_FETCH_SIZE);
    if fetch_size.get() > limits.max_fetch_size {
        return Err(Error::FetchSizeTooBig);
    }

    let offset = usize::try_from(params.pagination.offset_value()).unwrap_or(usize::MAX);
    let limit = params
        .pagination
        .limit_value()
        .map(|limit| usize::try_from(limit.get()).unwrap_or(usize::MAX));
    let fetch_size_usize = usize::try_from(fetch_size.get()).unwrap_or(usize::MAX);
    let first_take = limit.map_or(fetch_size_usize, |limit| limit.min(fetch_size_usize));
    let mut iter = iter.skip(offset).peekable();
    let first_batch_values: Vec<_> = iter.by_ref().take(first_take).collect();
    let batch_len = first_batch_values.len();
    let selector_for_deferred = selector.clone();
    let mut batch_iter =
        ErasedQueryIterator::new(first_batch_values.into_iter(), selector, fetch_size);
    let (first_batch, _next) = batch_iter.next_batch(0)?;

    let remaining_limit = limit.map(|limit| limit.saturating_sub(batch_len));
    let has_more = remaining_limit != Some(0) && iter.peek().is_some();
    if !has_more {
        return Ok(PreparedQueryStart {
            first_batch,
            remaining_items: None,
            deferred_continuation: None,
        });
    }

    let first_cursor = NonZeroU64::new(u64::try_from(batch_len).unwrap_or(u64::MAX))
        .expect("stored bounded continuation requires a non-empty first batch");
    let deferred_values: Vec<_> = match remaining_limit {
        Some(remaining_limit) => iter.take(remaining_limit).collect(),
        None => iter.collect(),
    };
    let deferred_continuation = DeferredQueryContinuation::new(first_cursor, None, move || {
        ErasedQueryIterator::new_streaming_with_cursor(
            deferred_values.into_iter(),
            selector_for_deferred,
            fetch_size,
            first_cursor.get(),
        )
    });

    Ok(PreparedQueryStart {
        first_batch,
        remaining_items: None,
        deferred_continuation: Some(deferred_continuation),
    })
}

fn collect_unsorted_bounded_page<I>(
    iter: I,
    selector: SelectorTuple<I::Item>,
    params: &QueryParams,
    limits: QueryLimits,
    returned_offset: u64,
) -> Result<(QueryOutputBatchBoxTuple, usize, bool), Error>
where
    I: Iterator<Item: SortableQueryOutput>,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
    <I::Item as HasProjection<SelectorMarker>>::Projection: EvaluateSelector<I::Item> + Send + Sync,
    QueryOutputBatchBox: From<Vec<I::Item>>,
{
    let fetch_size = params
        .fetch_size
        .fetch_size
        .unwrap_or(iroha_data_model::query::parameters::DEFAULT_FETCH_SIZE);
    if fetch_size.get() > limits.max_fetch_size {
        return Err(Error::FetchSizeTooBig);
    }

    let remaining_limit = params
        .pagination
        .limit_value()
        .map(|limit| limit.get().saturating_sub(returned_offset));
    if remaining_limit == Some(0) {
        return Err(Error::CursorDone);
    }

    let source_offset = params
        .pagination
        .offset_value()
        .saturating_add(returned_offset);
    let source_offset = usize::try_from(source_offset).unwrap_or(usize::MAX);
    let fetch_size_usize = usize::try_from(fetch_size.get()).unwrap_or(usize::MAX);
    let first_take = remaining_limit.map_or(fetch_size_usize, |limit| {
        usize::try_from(limit)
            .unwrap_or(usize::MAX)
            .min(fetch_size_usize)
    });

    let mut iter = iter.skip(source_offset).peekable();
    let first_batch_values: Vec<_> = iter.by_ref().take(first_take).collect();
    let batch_len = first_batch_values.len();
    let mut batch_iter =
        ErasedQueryIterator::new(first_batch_values.into_iter(), selector, fetch_size);
    let (first_batch, _next) = batch_iter.next_batch(0)?;

    let batch_len_u64 = u64::try_from(batch_len).unwrap_or(u64::MAX);
    let limit_allows_more = remaining_limit != Some(batch_len_u64);
    let has_more = batch_len > 0 && limit_allows_more && iter.peek().is_some();

    Ok((first_batch, batch_len, has_more))
}

fn prepare_stored_unsorted_bounded_replay_start<I, Q>(
    iter: I,
    query: Q,
    predicate: CompoundPredicate<I::Item>,
    selector: SelectorTuple<I::Item>,
    params: &QueryParams,
    limits: QueryLimits,
    replay_state: Weak<State>,
) -> Result<PreparedPagedQueryStart, Error>
where
    I: Iterator<Item: SortableQueryOutput>,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
    <I::Item as HasProjection<SelectorMarker>>::Projection: EvaluateSelector<I::Item> + Send + Sync,
    QueryOutputBatchBox: From<Vec<I::Item>>,
    Q: ValidQuery<Item = I::Item> + Clone + Send + Sync + 'static,
{
    let (first_batch, batch_len, has_more) =
        collect_unsorted_bounded_page(iter, selector.clone(), params, limits, 0)?;
    if !has_more {
        return Ok(PreparedPagedQueryStart {
            first_batch,
            paged_continuation: None,
        });
    }

    let first_cursor = NonZeroU64::new(u64::try_from(batch_len).unwrap_or(u64::MAX))
        .expect("stored bounded continuation requires a non-empty first batch");
    let params_for_replay = params.clone();
    let continuation = PagedQueryContinuation::new(first_cursor, move |cursor| {
        let state = replay_state.upgrade().ok_or(Error::Expired)?;
        let view = state.query_view();
        let iter = ValidQuery::execute(query.clone(), predicate.clone(), &view)?;
        let (batch, batch_len, has_more) = collect_unsorted_bounded_page(
            iter,
            selector.clone(),
            &params_for_replay,
            limits,
            cursor,
        )?;
        let next_cursor = has_more.then(|| {
            NonZeroU64::new(cursor.saturating_add(u64::try_from(batch_len).unwrap_or(u64::MAX)))
                .expect("cursor remains non-zero after a non-empty page")
        });
        Ok((batch, next_cursor))
    });

    Ok(PreparedPagedQueryStart {
        first_batch,
        paged_continuation: Some(continuation),
    })
}

fn handle_iter_start_stored<I>(
    iter: I,
    selector: SelectorTuple<I::Item>,
    params: &QueryParams,
    limits: QueryLimits,
    live_query_store: &LiveQueryStoreHandle,
    authority: &AccountId,
    gas_budget: Option<u64>,
) -> Result<QueryOutput, Error>
where
    I: Iterator<Item: SortableQueryOutput>,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
    <I::Item as HasProjection<SelectorMarker>>::Projection: EvaluateSelector<I::Item> + Send + Sync,
    QueryOutputBatchBox: From<Vec<I::Item>>,
{
    if params.sorting.sort_by_metadata_key.is_some() {
        if let Some(fast) = stored_sorted_fast_start_params(params, limits)? {
            let prepared = prepare_stored_sorted_start(iter, selector, fast, None)?;
            return live_query_store.handle_iter_start_prepared(prepared, authority, gas_budget);
        }
    } else if limits.count_mode == QueryCountMode::Bounded {
        let prepared = prepare_stored_unsorted_bounded_start(iter, selector, params, limits)?;
        return live_query_store.handle_iter_start_prepared(prepared, authority, gas_budget);
    }

    let batched = apply_query_postprocessing(iter, selector, params, limits)?;
    live_query_store.handle_iter_start(batched, authority, gas_budget)
}

#[allow(clippy::too_many_arguments)]
fn handle_iter_start_stored_replayable<I, Q>(
    iter: I,
    query: Q,
    predicate: CompoundPredicate<I::Item>,
    selector: SelectorTuple<I::Item>,
    params: &QueryParams,
    limits: QueryLimits,
    live_query_store: &LiveQueryStoreHandle,
    authority: &AccountId,
    gas_budget: Option<u64>,
    replay_state: Option<Weak<State>>,
) -> Result<QueryOutput, Error>
where
    I: Iterator<Item: SortableQueryOutput>,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
    <I::Item as HasProjection<SelectorMarker>>::Projection: EvaluateSelector<I::Item> + Send + Sync,
    QueryOutputBatchBox: From<Vec<I::Item>>,
    Q: ValidQuery<Item = I::Item> + Clone + Send + Sync + 'static,
{
    if params.sorting.sort_by_metadata_key.is_none()
        && limits.count_mode == QueryCountMode::Bounded
        && let Some(replay_state) = replay_state
    {
        let prepared = prepare_stored_unsorted_bounded_replay_start(
            iter,
            query,
            predicate,
            selector,
            params,
            limits,
            replay_state,
        )?;
        return live_query_store.handle_iter_start_paged_prepared(prepared, authority, gas_budget);
    }

    handle_iter_start_stored(
        iter,
        selector,
        params,
        limits,
        live_query_store,
        authority,
        gas_budget,
    )
}

struct IncrementalSortedValues<T: SortableQueryOutput> {
    values: Vec<Option<T>>,
    sort_keys: Vec<Option<Json>>,
    tiebreak_keys: Vec<T::TiebreakKey>,
    order_indices: Vec<usize>,
    prepared: usize,
    next: usize,
    end: usize,
    chunk_size: usize,
    order: SortOrder,
}

impl<T: SortableQueryOutput> IncrementalSortedValues<T> {
    fn new(
        values: Vec<Option<T>>,
        sort_keys: Vec<Option<Json>>,
        tiebreak_keys: Vec<T::TiebreakKey>,
        pagination: iroha_data_model::query::parameters::Pagination,
        chunk_size: NonZeroU64,
        order: SortOrder,
    ) -> Self {
        let order_indices: Vec<_> = (0..values.len()).collect();
        let next = usize::try_from(pagination.offset_value())
            .unwrap_or(usize::MAX)
            .min(order_indices.len());
        let len = pagination
            .limit_value()
            .map_or(order_indices.len() - next, |limit| {
                usize::try_from(limit.get())
                    .unwrap_or(usize::MAX)
                    .min(order_indices.len() - next)
            });
        let end = next.saturating_add(len);

        Self {
            values,
            sort_keys,
            tiebreak_keys,
            order_indices,
            prepared: 0,
            next,
            end,
            chunk_size: usize::try_from(chunk_size.get())
                .unwrap_or(usize::MAX)
                .max(1),
            order,
        }
    }

    fn ensure_prepared(&mut self, required: usize) {
        let required = required.min(self.end).min(self.order_indices.len());
        if required <= self.prepared {
            return;
        }

        let sort_keys = &self.sort_keys;
        let tiebreak_keys = &self.tiebreak_keys;
        let order = self.order;
        let additional = required - self.prepared;
        let tail = &mut self.order_indices[self.prepared..];
        if additional < tail.len() {
            tail.select_nth_unstable_by(additional - 1, |left, right| {
                compare_sorted_query_indices::<T>(*left, *right, sort_keys, tiebreak_keys, order)
            });
        }
        tail[..additional].sort_by(|left, right| {
            compare_sorted_query_indices::<T>(*left, *right, sort_keys, tiebreak_keys, order)
        });
        self.prepared = required;
    }
}

impl<T: SortableQueryOutput> Iterator for IncrementalSortedValues<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.end {
            return None;
        }

        if self.next >= self.prepared {
            self.ensure_prepared(self.next.saturating_add(self.chunk_size));
        }

        let value_index = self.order_indices[self.next];
        self.next += 1;
        Some(
            self.values[value_index]
                .take()
                .expect("sorted query item should be present"),
        )
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end.saturating_sub(self.next);
        (remaining, Some(remaining))
    }
}

impl<T: SortableQueryOutput> ExactSizeIterator for IncrementalSortedValues<T> {
    fn len(&self) -> usize {
        self.end.saturating_sub(self.next)
    }
}

fn apply_query_postprocessing_ephemeral_with_budget<I>(
    iter: I,
    selector: SelectorTuple<I::Item>,
    params: &QueryParams,
    limits: QueryLimits,
    budget: Option<QueryExecutionBudget>,
) -> Result<(QueryOutput, QueryExecutionStats), Error>
where
    I: Iterator<Item: SortableQueryOutput>,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + NoritoSerialize + Send + Sync + 'static,
    <I::Item as HasProjection<SelectorMarker>>::Projection: EvaluateSelector<I::Item> + Send + Sync,
    QueryOutputBatchBox: From<Vec<I::Item>>,
{
    let batch_size = params
        .fetch_size
        .fetch_size
        .unwrap_or(iroha_data_model::query::parameters::DEFAULT_FETCH_SIZE);
    let max_fetch = limits.max_fetch_size;
    if batch_size.get() > max_fetch {
        return Err(Error::FetchSizeTooBig);
    }
    let mut stats = QueryExecutionStats::default();

    if limits.count_mode == QueryCountMode::Bounded && params.sorting.sort_by_metadata_key.is_none()
    {
        let fetch_size = usize::try_from(batch_size.get()).unwrap_or(usize::MAX);
        let offset = params.pagination.offset_value();
        let limit = params.pagination.limit_value().map(|limit| limit.get());
        let probe = limit.map_or_else(
            || fetch_size.saturating_add(1),
            |limit| {
                usize::try_from(limit)
                    .unwrap_or(usize::MAX)
                    .min(fetch_size.saturating_add(1))
            },
        );
        let mut processed = 0_u64;
        let mut skipped = 0_u64;
        let mut has_more = false;
        let mut first_batch_values = Vec::with_capacity(fetch_size.min(1024));
        let mut iter = iter;
        while skipped < offset {
            let Some(value) = iter.next() else {
                break;
            };
            stats.record_skipped_value(&value, budget)?;
            skipped = skipped.saturating_add(1);
        }
        while usize::try_from(processed).unwrap_or(usize::MAX) < probe {
            let Some(value) = iter.next() else {
                break;
            };
            processed = processed.saturating_add(1);
            stats.record_item(&value, budget)?;
            if first_batch_values.len() < fetch_size {
                first_batch_values.push(value);
            } else {
                has_more = true;
                break;
            }
        }

        let mut batch_iter =
            ErasedQueryIterator::new(first_batch_values.into_iter(), selector, batch_size);
        let (batch, _next) = batch_iter.next_batch(0)?;
        debug_assert_eq!(stats.processed_items(), processed);
        return Ok((QueryOutput::new_bounded(batch, has_more, None), stats));
    }

    if let Some(key) = params.sorting.sort_by_metadata_key.as_ref() {
        let offset = usize::try_from(params.pagination.offset_value()).unwrap_or(usize::MAX);
        let limit = params.pagination.limit_value().map_or(usize::MAX, |limit| {
            usize::try_from(limit.get()).unwrap_or(usize::MAX)
        });
        let fetch_size = usize::try_from(batch_size.get()).unwrap_or(usize::MAX);
        let order = params.sorting.order.unwrap_or(SortOrder::Asc);
        let keep = offset.saturating_add(limit.min(fetch_size));

        if keep <= STREAMING_SORTED_PREFIX_LIMIT {
            let (values, count) =
                collect_ephemeral_sorted_prefix(iter, key, order, keep, budget, &mut stats)?;
            let total_after_pagination = usize::try_from(count)
                .unwrap_or(usize::MAX)
                .saturating_sub(offset)
                .min(limit);
            let batch_len = total_after_pagination.min(fetch_size);
            let batch_values: Vec<_> = values.into_iter().skip(offset).take(batch_len).collect();
            let mut batch_iter =
                ErasedQueryIterator::new(batch_values.into_iter(), selector, batch_size);
            let (batch, _next) = batch_iter.next_batch(0)?;
            let remaining_items =
                u64::try_from(total_after_pagination.saturating_sub(batch_len)).unwrap_or(u64::MAX);
            debug_assert_eq!(stats.processed_items(), count);
            return Ok((QueryOutput::new(batch, remaining_items, None), stats));
        }

        let mut count = 0_u64;
        let mut values = Vec::new();
        let mut sort_keys = Vec::new();
        let mut tiebreak_keys = Vec::new();
        for value in iter {
            count = count.saturating_add(1);
            stats.record_item(&value, budget)?;
            let sort_key = value.get_metadata_sorting_key(key);
            if let Some(sort_key) = sort_key {
                stats.record_skipped_value(sort_key, budget)?;
            }
            sort_keys.push(sort_key.cloned());
            let tiebreak_key = materialize_admitted_tiebreak_key(&value, &mut stats, budget)?;
            tiebreak_keys.push(tiebreak_key);
            values.push(Some(value));
        }

        let total_after_pagination = values.len().saturating_sub(offset).min(limit);
        let batch_len = total_after_pagination.min(fetch_size);
        let mut order_indices: Vec<_> = (0..values.len()).collect();

        if batch_len > 0 {
            let keep = offset.saturating_add(batch_len);
            if keep < order_indices.len() {
                order_indices.select_nth_unstable_by(keep - 1, |left, right| {
                    compare_sorted_query_indices::<I::Item>(
                        *left,
                        *right,
                        &sort_keys,
                        &tiebreak_keys,
                        order,
                    )
                });
                order_indices.truncate(keep);
            }
            order_indices.sort_by(|left, right| {
                compare_sorted_query_indices::<I::Item>(
                    *left,
                    *right,
                    &sort_keys,
                    &tiebreak_keys,
                    order,
                )
            });
        }

        let batch_values: Vec<_> = order_indices
            .into_iter()
            .skip(offset)
            .take(batch_len)
            .map(|index| {
                values[index]
                    .take()
                    .expect("sorted query item should be present")
            })
            .collect();
        let mut batch_iter =
            ErasedQueryIterator::new(batch_values.into_iter(), selector, batch_size);
        let (batch, _next) = batch_iter.next_batch(0)?;
        let remaining_items =
            u64::try_from(total_after_pagination.saturating_sub(batch_len)).unwrap_or(u64::MAX);
        debug_assert_eq!(stats.processed_items(), count);
        return Ok((QueryOutput::new(batch, remaining_items, None), stats));
    }

    let fetch_size = usize::try_from(batch_size.get()).unwrap_or(usize::MAX);
    let offset = params.pagination.offset_value();
    let limit = params.pagination.limit_value().map(|limit| limit.get());
    let mut skipped = 0_u64;
    let mut count = 0_u64;
    let mut first_batch_values = Vec::with_capacity(fetch_size.min(1024));
    let mut iter = iter;
    while skipped < offset {
        let Some(value) = iter.next() else {
            break;
        };
        stats.record_skipped_value(&value, budget)?;
        skipped = skipped.saturating_add(1);
    }
    while !limit.is_some_and(|limit| count >= limit) {
        let Some(value) = iter.next() else {
            break;
        };
        count = count.saturating_add(1);
        stats.record_item(&value, budget)?;
        if first_batch_values.len() < fetch_size {
            first_batch_values.push(value);
        }
    }

    let batch_len = first_batch_values.len();
    let mut batch_iter =
        ErasedQueryIterator::new(first_batch_values.into_iter(), selector, batch_size);
    let (batch, _next) = batch_iter.next_batch(0)?;
    let remaining_items = count.saturating_sub(u64::try_from(batch_len).unwrap_or(u64::MAX));
    debug_assert_eq!(stats.processed_items(), count);
    Ok((QueryOutput::new(batch, remaining_items, None), stats))
}

fn apply_query_postprocessing_with_budget<I>(
    iter: I,
    selector: SelectorTuple<I::Item>,
    params: &QueryParams,
    limits: QueryLimits,
    budget_items: Option<u64>,
) -> Result<(ErasedQueryIterator, u64), Error>
where
    I: Iterator<Item: SortableQueryOutput>,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
    <I::Item as HasProjection<SelectorMarker>>::Projection: EvaluateSelector<I::Item> + Send + Sync,
    QueryOutputBatchBox: From<Vec<I::Item>>,
{
    // Validate and pick the fetch (aka batch) size from params
    let fetch_size = params
        .fetch_size
        .fetch_size
        .unwrap_or(iroha_data_model::query::parameters::DEFAULT_FETCH_SIZE);
    let max_fetch = limits.max_fetch_size;
    if fetch_size.get() > max_fetch {
        return Err(Error::FetchSizeTooBig);
    }

    // sort & paginate, erase the iterator with QueryBatchedErasedIterator
    let (output, processed_items) = if let Some(key) = params.sorting.sort_by_metadata_key.as_ref()
    {
        // if sorting was requested, we need to retrieve all the results first
        let mut count = 0_u64;
        let mut values = Vec::new();
        let mut sort_keys = Vec::new();
        let mut tiebreak_keys = Vec::new();
        for value in iter {
            count = count.saturating_add(1);
            if budget_items.is_some_and(|limit| count > limit) {
                return Err(Error::GasBudgetExceeded);
            }
            sort_keys.push(value.get_metadata_sorting_key(key).cloned());
            tiebreak_keys.push(value.tiebreak_key());
            values.push(Some(value));
        }
        let order = params.sorting.order.unwrap_or(SortOrder::Asc);

        (
            ErasedQueryIterator::new(
                IncrementalSortedValues::new(
                    values,
                    sort_keys,
                    tiebreak_keys,
                    params.pagination,
                    fetch_size,
                    order,
                ),
                selector,
                fetch_size,
            ),
            count,
        )
    } else {
        // FP: this collect is very deliberate
        #[allow(clippy::needless_collect)]
        let mut count = 0_u64;
        let output = {
            let mut output = Vec::new();
            for value in iter.paginate(params.pagination) {
                count = count.saturating_add(1);
                if budget_items.is_some_and(|limit| count > limit) {
                    return Err(Error::GasBudgetExceeded);
                }
                output.push(value);
            }
            output
        };

        (
            ErasedQueryIterator::new(output.into_iter(), selector, fetch_size),
            count,
        )
    };

    Ok((output, processed_items))
}

fn validate_query_request_limits(
    request: &QueryRequest,
    limits: QueryLimits,
) -> Result<(), ValidationFail> {
    let max_fetch = limits.max_fetch_size;
    if let QueryRequest::Start(start) = request {
        let fetch_size = start
            .params
            .fetch_size
            .fetch_size
            .unwrap_or(DEFAULT_FETCH_SIZE);
        if fetch_size.get() > max_fetch {
            return Err(ValidationFail::QueryFailed(Error::FetchSizeTooBig));
        }
    }
    Ok(())
}

#[cfg(test)]
mod fetch_size_limit_tests {
    use std::io::Write;

    use iroha_config::parameters::{actual::Root as ConfigRoot, defaults::torii as torii_defaults};
    use iroha_data_model::{
        permission::Permission,
        prelude::SelectorTuple,
        query::{
            QueryWithParams,
            parameters::{FetchSize, Pagination, QueryParams, Sorting},
        },
    };
    use iroha_primitives::json::Json;
    use nonzero_ext::nonzero;
    use tempfile::NamedTempFile;

    use super::*;

    fn request_with_fetch_size(fetch_size: u64) -> QueryRequest {
        let fetch_size = std::num::NonZeroU64::new(fetch_size).expect("nonzero fetch size");
        QueryRequest::Start(QueryWithParams {
            #[cfg(not(feature = "fast_dsl"))]
            query: QueryBox::from(iroha_data_model::query::account::prelude::FindAccounts),
            #[cfg(feature = "fast_dsl")]
            query: (),
            #[cfg(feature = "fast_dsl")]
            query_payload: Vec::new(),
            #[cfg(feature = "fast_dsl")]
            item: iroha_data_model::query::QueryItemKind::Account,
            #[cfg(feature = "fast_dsl")]
            predicate_bytes: Vec::new(),
            #[cfg(feature = "fast_dsl")]
            selector_bytes: Vec::new(),
            params: QueryParams {
                fetch_size: FetchSize::new(Some(fetch_size)),
                ..QueryParams::default()
            },
        })
    }

    fn minimal_root_with_max_fetch(max_fetch_size: u32) -> ConfigRoot {
        let config = format!(
            r#"
chain = "00000000-0000-0000-0000-000000000000"
public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
private_key = "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"
trusted_peers_pop = [
  {{ public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2", pop_hex = "8515da750f81182aaba5c22fc9f03a01e81ed85e4495a2ca6b29a71c0c8549537e31e79cddf6ff285b9e22d0d9dc17ce0f46e7d0cf78b2ef9feab50c849a1ea8e1e4f07e966f6113faa8a999317545d9f111b8e08a7273913710b43a20b19c08" }},
]

[network]
address = "addr:127.0.0.1:1337#8F78"
public_address = "addr:127.0.0.1:1337#8F78"

[torii]
address = "addr:127.0.0.1:8080#8942"
app_api_max_fetch_size = {max_fetch_size}

[genesis]
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"

[streaming]
identity_public_key = "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB"
identity_private_key = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"
"#
        );
        let mut file = NamedTempFile::new().expect("temp config file");
        file.write_all(config.as_bytes()).expect("write config");
        let source =
            iroha_config::base::toml::TomlSource::from_file(file.path()).expect("read config");
        ConfigRoot::from_toml_source(source).expect("load minimal config")
    }

    fn minimal_root_with_pipeline_max_fetch(max_fetch_size: u64) -> ConfigRoot {
        let config = format!(
            r#"
chain = "00000000-0000-0000-0000-000000000000"
public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
private_key = "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"
trusted_peers_pop = [
  {{ public_key = "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2", pop_hex = "8515da750f81182aaba5c22fc9f03a01e81ed85e4495a2ca6b29a71c0c8549537e31e79cddf6ff285b9e22d0d9dc17ce0f46e7d0cf78b2ef9feab50c849a1ea8e1e4f07e966f6113faa8a999317545d9f111b8e08a7273913710b43a20b19c08" }},
]

[network]
address = "addr:127.0.0.1:1337#8F78"
public_address = "addr:127.0.0.1:1337#8F78"

[torii]
address = "addr:127.0.0.1:8080#8942"

[pipeline]
query_max_fetch_size = {max_fetch_size}

[genesis]
public_key = "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"

[streaming]
identity_public_key = "ed01208BA62848CF767D72E7F7F4B9D2D7BA07FEE33760F79ABE5597A51520E292A0CB"
identity_private_key = "8026208F4C15E5D664DA3F13778801D23D4E89B76E94C1B94B389544168B6CB894F84F"
"#
        );
        let mut file = NamedTempFile::new().expect("temp config file");
        file.write_all(config.as_bytes()).expect("write config");
        let source =
            iroha_config::base::toml::TomlSource::from_file(file.path()).expect("read config");
        ConfigRoot::from_toml_source(source).expect("load minimal config")
    }

    #[test]
    fn reject_fetch_size_above_max() {
        let over = u64::from(torii_defaults::APP_API_MAX_FETCH_SIZE)
            .checked_add(1)
            .expect("nonzero add");
        let request = QueryRequest::Start(QueryWithParams {
            #[cfg(not(feature = "fast_dsl"))]
            query: QueryBox::from(iroha_data_model::query::account::prelude::FindAccounts),
            #[cfg(feature = "fast_dsl")]
            query: (),
            #[cfg(feature = "fast_dsl")]
            query_payload: Vec::new(),
            #[cfg(feature = "fast_dsl")]
            item: iroha_data_model::query::QueryItemKind::Account,
            #[cfg(feature = "fast_dsl")]
            predicate_bytes: Vec::new(),
            #[cfg(feature = "fast_dsl")]
            selector_bytes: Vec::new(),
            params: QueryParams {
                fetch_size: FetchSize::new(Some(
                    std::num::NonZeroU64::new(over).expect("nonzero fetch size"),
                )),
                ..QueryParams::default()
            },
        });

        let err = validate_query_request_limits(&request, QueryLimits::from_defaults())
            .expect_err("must reject oversized fetch");
        assert!(matches!(
            err,
            ValidationFail::QueryFailed(Error::FetchSizeTooBig)
        ));
    }

    #[test]
    fn postprocessing_rejects_fetch_size_above_limits() {
        let params = QueryParams {
            fetch_size: FetchSize::new(Some(nonzero!(2_u64))),
            ..QueryParams::default()
        };
        let iter = std::iter::once(Permission::new("p".to_owned(), Json::from(false)));
        let err = apply_query_postprocessing(
            iter,
            SelectorTuple::default(),
            &params,
            QueryLimits::new(1),
        )
        .expect_err("fetch size should be rejected");
        assert!(matches!(err, Error::FetchSizeTooBig));
    }

    #[test]
    fn postprocessing_reports_processed_items_for_sorted_queries() {
        let key: iroha_data_model::name::Name = "rank".parse().expect("name");
        let params = QueryParams {
            pagination: Pagination::new(Some(nonzero!(1_u64)), 0),
            sorting: Sorting::by_metadata_key(key),
            fetch_size: FetchSize::new(Some(nonzero!(1_u64))),
        };
        let items = vec![
            Permission::new("p1".to_owned(), Json::from(false)),
            Permission::new("p2".to_owned(), Json::from(false)),
            Permission::new("p3".to_owned(), Json::from(false)),
        ];

        let (iter, processed_items) = apply_query_postprocessing_with_budget(
            items.into_iter(),
            SelectorTuple::default(),
            &params,
            QueryLimits::new(10),
            None,
        )
        .expect("postprocess sorted query");

        assert_eq!(processed_items, 3);
        assert_eq!(iter.remaining(), Some(1));
    }

    #[test]
    fn query_limits_new_clamps_to_one() {
        let request_ok = request_with_fetch_size(1);
        validate_query_request_limits(&request_ok, QueryLimits::new(0))
            .expect("clamped fetch size should be accepted");

        let request_over = request_with_fetch_size(2);
        let err = validate_query_request_limits(&request_over, QueryLimits::new(0))
            .expect_err("clamped limit should reject larger fetch sizes");
        assert!(matches!(
            err,
            ValidationFail::QueryFailed(Error::FetchSizeTooBig)
        ));
    }

    #[test]
    fn query_limits_from_torii_uses_configured_max_fetch() {
        let root = minimal_root_with_max_fetch(3);
        let limits = QueryLimits::from_torii(&root.torii);

        let request = request_with_fetch_size(4);
        let err = validate_query_request_limits(&request, limits)
            .expect_err("configured max fetch should be enforced");
        assert!(matches!(
            err,
            ValidationFail::QueryFailed(Error::FetchSizeTooBig)
        ));
    }

    #[test]
    fn query_limits_from_pipeline_uses_configured_max_fetch() {
        let root = minimal_root_with_pipeline_max_fetch(3);
        let limits = QueryLimits::from_pipeline(&root.pipeline);

        let request = request_with_fetch_size(4);
        let err = validate_query_request_limits(&request, limits)
            .expect_err("configured pipeline max fetch should be enforced");
        assert!(matches!(
            err,
            ValidationFail::QueryFailed(Error::FetchSizeTooBig)
        ));
    }
}

/// Query Request statefully validated on the Iroha node side.
pub struct ValidQueryRequest {
    request: QueryRequest,
    limits: QueryLimits,
}

/// Lightweight trait abstraction for IVM-side query validation to decouple from `ivm::state`.
pub trait IvmQueryValidator {
    /// Account on whose behalf the query will run.
    fn authority(&self) -> &AccountId;
    /// Validate a query in the executor context.
    ///
    /// # Errors
    /// Returns [`ValidationFail`] if the query is not permitted.
    fn validate_query(
        &mut self,
        authority: &AccountId,
        query: &QueryRequest,
    ) -> Result<(), ValidationFail>;
}

impl ValidQueryRequest {
    /// Validate a query for an API client by calling the executor.
    ///
    /// # Errors
    ///
    /// Returns an error if the query validation fails or request limits are exceeded.
    pub fn validate_for_client_parts(
        request: QueryRequest,
        authority: &AccountId,
        state_ro: &impl StateReadOnly,
        limits: QueryLimits,
    ) -> Result<Self, ValidationFail> {
        let latest_block = state_ro.latest_block().map(|block| block.header());
        Self::validate_for_client_world_parts(
            request,
            authority,
            state_ro.world(),
            latest_block,
            limits,
        )
    }

    /// Validate a query for an API client using world-state and latest committed block header.
    ///
    /// # Errors
    ///
    /// Returns an error if the query validation fails or request limits are exceeded.
    pub fn validate_for_client_world_parts(
        request: QueryRequest,
        authority: &AccountId,
        world_ro: &impl WorldReadOnly,
        latest_block: Option<BlockHeader>,
        limits: QueryLimits,
    ) -> Result<Self, ValidationFail> {
        ensure_query_registry_initialized();
        validate_query_request_limits(&request, limits)?;
        world_ro.executor().validate_query_with_world_parts(
            world_ro,
            latest_block,
            authority,
            &request,
        )?;
        Ok(Self { request, limits })
    }

    /// Validate a query for an API client using the provided Torii configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the query validation fails.
    pub fn validate_for_client_parts_with_config(
        request: QueryRequest,
        authority: &AccountId,
        state_ro: &impl StateReadOnly,
        torii_cfg: &ToriiActual,
    ) -> Result<Self, ValidationFail> {
        let limits = QueryLimits::from_torii(torii_cfg);
        Self::validate_for_client_parts(request, authority, state_ro, limits)
    }

    /// Validate a query for an IVM program.
    ///
    /// NOTE: The previous API used `ivm::state` types directly which are no longer exposed.
    /// This shim keeps the public surface while decoupling from IVM internals.
    /// Provide a state object that can validate a query via this trait.
    ///
    /// # Errors
    /// Returns a validation error if the request is rejected by the IVM validator.
    pub fn validate_for_ivm(
        query: QueryRequest,
        state: &mut impl IvmQueryValidator,
        limits: QueryLimits,
    ) -> Result<Self, ValidationFail> {
        ensure_query_registry_initialized();
        if matches!(&query, QueryRequest::Continue(_)) {
            return Err(ValidationFail::NotPermitted(
                "QueryRequest::Continue is not supported in IVM".to_string(),
            ));
        }
        validate_query_request_limits(&query, limits)?;
        let authority = state.authority().clone();
        state.validate_query(&authority, &query)?;
        Ok(Self {
            request: query,
            limits,
        })
    }

    /// Execute a validated query request.
    ///
    /// # Errors
    ///
    /// Returns an error if the query execution fails.
    pub fn execute(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
    ) -> Result<QueryResponse, Error> {
        self.execute_stored_and_bind_revalidation(live_query_store, state, authority, None, None)
    }

    /// Execute a validated query request with an optional state handle for
    /// bounded stored cursors that can replay one continuation page at a time.
    ///
    /// # Errors
    ///
    /// Returns an error if the query execution fails.
    pub fn execute_with_replay_state(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
        replay_state: Weak<State>,
    ) -> Result<QueryResponse, Error> {
        self.execute_stored_and_bind_revalidation(
            live_query_store,
            state,
            authority,
            Some(replay_state),
            None,
        )
    }

    /// Execute a validated stored query with an owning replay state and the
    /// client-provided budget for the initial `Start` request.
    ///
    /// # Errors
    ///
    /// Returns an error if query execution or budgeted projection fails.
    pub fn execute_with_replay_state_and_start_budget(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
        replay_state: Weak<State>,
        stored_start_budget: Option<u64>,
    ) -> Result<QueryResponse, Error> {
        self.execute_stored_and_bind_revalidation(
            live_query_store,
            state,
            authority,
            Some(replay_state),
            stored_start_budget,
        )
    }

    fn execute_stored_and_bind_revalidation(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
        replay_state: Option<Weak<State>>,
        stored_start_budget: Option<u64>,
    ) -> Result<QueryResponse, Error> {
        let revalidation_archive = matches!(&self.request, QueryRequest::Start(_))
            .then(|| {
                norito::to_bytes(&self.request).map_err(|error| {
                    Error::Conversion(format!(
                        "failed to encode stored-query authorization request: {error}"
                    ))
                })
            })
            .transpose()?;
        let response = self.execute_stored_inner(
            live_query_store,
            state,
            authority,
            replay_state,
            stored_start_budget,
        )?;

        if let (
            Some(archive),
            QueryResponse::Iterable(QueryOutput {
                continue_cursor: Some(cursor),
                ..
            }),
        ) = (revalidation_archive, &response)
            && let Err(error) =
                live_query_store.bind_revalidation_request(cursor, authority, archive)
        {
            live_query_store.drop_query(&cursor.query);
            return Err(error);
        }

        Ok(response)
    }

    #[allow(clippy::too_many_lines)] // not much we can do, we _need_ to list all the box types here
    fn execute_stored_inner(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
        replay_state: Option<Weak<State>>,
        stored_start_budget: Option<u64>,
    ) -> Result<QueryResponse, Error> {
        let Self { request, limits } = self;
        match request {
            QueryRequest::Singular(singular_query) => {
                let output = singular_query.execute(state)?;
                Ok(QueryResponse::Singular(output))
            }
            QueryRequest::Start(iter_query) => {
                use iroha_data_model::query;

                fn try_decode_query<Q>(
                    erased: &query::ErasedIterQuery<
                        impl HasProjection<PredicateMarker>
                        + HasProjection<SelectorMarker, AtomType = ()>
                        + Send
                        + Sync,
                    >,
                ) -> Option<Q>
                where
                    Q: norito::codec::Decode,
                {
                    let bytes = erased.payload();
                    let mut cur = bytes;
                    let query = Q::decode(&mut cur).ok()?;
                    cur.is_empty().then_some(query)
                }

                #[allow(clippy::too_many_arguments)]
                fn run_dispatch<T, Q, F>(
                    qbox: &query::QueryBox<query::QueryOutputBatchBox>,
                    params: &query::parameters::QueryParams,
                    limits: QueryLimits,
                    state: &impl StateReadOnly,
                    live_query_store: &LiveQueryStoreHandle,
                    authority: &AccountId,
                    gas_budget: Option<u64>,
                    replay_state: Option<Weak<State>>,
                    decode: F,
                ) -> Result<Option<QueryResponse>, Error>
                where
                    T: Send + Sync + 'static,
                    Q: super::super::ValidQuery<Item = T> + Clone + Send + Sync + 'static,
                    T: HasProjection<SelectorMarker, AtomType = ()>
                        + HasProjection<PredicateMarker>
                        + crate::smartcontracts::isi::query::SortableQueryOutput
                        + Send
                        + Sync
                        + 'static,
                    <T as HasProjection<SelectorMarker>>::Projection:
                        EvaluateSelector<T> + Send + Sync,
                    query::QueryOutputBatchBox: From<Vec<T>>,
                    F: Fn(&query::ErasedIterQuery<T>) -> Option<Q>,
                {
                    if let Some(erased) = query::iter_query_inner::<T>(qbox) {
                        // Decode the concrete query variant from the payload
                        let Some(concrete) = decode(erased) else {
                            return Ok(None);
                        };
                        // Execute the concrete ValidQuery with provided predicate
                        let predicate = erased.predicate_cloned();
                        let iter = ValidQuery::execute(concrete.clone(), predicate.clone(), state)?;

                        // Postprocess and register a live iterator (or prepared fast-start).
                        let output = handle_iter_start_stored_replayable(
                            iter,
                            concrete,
                            predicate,
                            erased.selector_cloned(),
                            params,
                            limits,
                            live_query_store,
                            authority,
                            gas_budget,
                            replay_state,
                        )?;
                        return Ok(Some(QueryResponse::Iterable(output)));
                    }
                    Ok(None)
                }

                let params = &iter_query.params;
                #[cfg_attr(not(feature = "fast_dsl"), allow(unused_variables))]
                let stored_cursor_budget = {
                    let min = state.pipeline().query_stored_min_gas_units;
                    stored_start_budget.or_else(|| (min > 0).then_some(min))
                };
                // Fast-DSL path: when the boxed query payload is not present, reconstruct
                // from item kind and encoded predicate/selector.
                if iter_query.query_box().is_none() {
                    {
                        use iroha_data_model::query::QueryItemKind;
                        // Helpers to decode bytes into concrete predicate/selector
                        fn dec<T: norito::codec::Decode>(bytes: &[u8]) -> Result<T, Error> {
                            let mut cursor = std::io::Cursor::new(bytes);
                            norito::codec::Decode::decode(&mut cursor).map_err(|_| {
                                Error::Conversion(
                                    "failed to decode query predicate/selector".into(),
                                )
                            })
                        }
                        // Helper to run a unit iterable query ("find all ...") using the encoded predicate/selector.
                        macro_rules! run_payload_or_default {
                            // For unit queries: ignore payload and run the default constructor (FindX::new())
                            ($itemty:ty, $find:ty) => {{
                                let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                    dec(&iter_query.predicate_bytes)?;
                                let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                    dec(&iter_query.selector_bytes)?;
                                let concrete = <$find>::new();
                                let iter =
                                    ValidQuery::execute(concrete.clone(), pred.clone(), state)?;
                                let output = handle_iter_start_stored_replayable(
                                    iter,
                                    concrete,
                                    pred,
                                    sel,
                                    params,
                                    limits,
                                    live_query_store,
                                    authority,
                                    stored_cursor_budget,
                                    replay_state.clone(),
                                )?;
                                return Ok(QueryResponse::Iterable(output));
                            }};
                            // For parameterized queries that require payload: fail if missing
                            (require_payload $itemty:ty, $find:ty) => {{
                                let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                    dec(&iter_query.predicate_bytes)?;
                                let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                    dec(&iter_query.selector_bytes)?;
                                if iter_query.query_payload.is_empty() {
                                    return Err(Error::Conversion(
                                        "missing query payload for parameterized iterable query"
                                            .into(),
                                    ));
                                }
                                let mut cursor = std::io::Cursor::new(&iter_query.query_payload);
                                let concrete: $find = norito::codec::Decode::decode(&mut cursor)
                                    .map_err(|_| {
                                        Error::Conversion("failed to decode query payload".into())
                                    })?;
                                let iter =
                                    ValidQuery::execute(concrete.clone(), pred.clone(), state)?;
                                let output = handle_iter_start_stored_replayable(
                                    iter,
                                    concrete,
                                    pred,
                                    sel,
                                    params,
                                    limits,
                                    live_query_store,
                                    authority,
                                    stored_cursor_budget,
                                    replay_state.clone(),
                                )?;
                                return Ok(QueryResponse::Iterable(output));
                            }};
                        }
                        macro_rules! run_fast {
                            ($itemty:ty, $find:ty) => {{
                                let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                    dec(&iter_query.predicate_bytes)?;
                                let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                    dec(&iter_query.selector_bytes)?;
                                let concrete = <$find>::new();
                                let iter =
                                    ValidQuery::execute(concrete.clone(), pred.clone(), state)?;
                                let output = handle_iter_start_stored_replayable(
                                    iter,
                                    concrete,
                                    pred,
                                    sel,
                                    params,
                                    limits,
                                    live_query_store,
                                    authority,
                                    stored_cursor_budget,
                                    replay_state.clone(),
                                )?;
                                return Ok(QueryResponse::Iterable(output));
                            }};
                        }
                        match iter_query.item {
                            QueryItemKind::Domain => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::domain::Domain,
                                        iroha_data_model::query::domain::prelude::FindDomainsByAccountId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::domain::Domain,
                                    iroha_data_model::query::domain::prelude::FindDomains
                                )
                            }
                            QueryItemKind::Account => {
                                // Prefer parameterized query when payload is present; otherwise default.
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(require_payload iroha_data_model::account::Account, iroha_data_model::query::account::prelude::FindAccountsWithAsset)
                                }
                                run_fast!(
                                    iroha_data_model::account::Account,
                                    iroha_data_model::query::account::prelude::FindAccounts
                                )
                            }
                            QueryItemKind::AccountId => run_payload_or_default!(
                                iroha_data_model::account::AccountId,
                                iroha_data_model::query::account::prelude::FindAccountIds
                            ),
                            QueryItemKind::Asset => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::asset::value::Asset,
                                        iroha_data_model::query::asset::prelude::FindAssetsByAccountId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::asset::value::Asset,
                                    iroha_data_model::query::asset::prelude::FindAssets
                                )
                            }
                            QueryItemKind::AssetDefinition => run_payload_or_default!(
                                iroha_data_model::asset::definition::AssetDefinition,
                                iroha_data_model::query::asset::prelude::FindAssetsDefinitions
                            ),
                            QueryItemKind::RepoAgreement => run_payload_or_default!(
                                iroha_data_model::repo::RepoAgreement,
                                iroha_data_model::query::repo::prelude::FindRepoAgreements
                            ),
                            QueryItemKind::Nft => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::nft::Nft,
                                        iroha_data_model::query::nft::prelude::FindNftsByAccountId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::nft::Nft,
                                    iroha_data_model::query::nft::prelude::FindNfts
                                )
                            }
                            QueryItemKind::Rwa => run_payload_or_default!(
                                iroha_data_model::rwa::Rwa,
                                iroha_data_model::query::rwa::prelude::FindRwas
                            ),
                            QueryItemKind::Role => run_payload_or_default!(
                                iroha_data_model::role::Role,
                                iroha_data_model::query::role::prelude::FindRoles
                            ),
                            QueryItemKind::RoleId => {
                                // If payload present, it's a parameterized FindRolesByAccountId; otherwise use FindRoleIds.
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(require_payload iroha_data_model::role::RoleId, iroha_data_model::query::role::prelude::FindRolesByAccountId)
                                }
                                run_fast!(
                                    iroha_data_model::role::RoleId,
                                    iroha_data_model::query::role::prelude::FindRoleIds
                                )
                            }
                            QueryItemKind::PeerId => run_payload_or_default!(
                                iroha_data_model::peer::PeerId,
                                iroha_data_model::query::peer::prelude::FindPeers
                            ),
                            QueryItemKind::TriggerId => run_payload_or_default!(
                                iroha_data_model::trigger::TriggerId,
                                iroha_data_model::query::trigger::prelude::FindActiveTriggerIds
                            ),
                            QueryItemKind::Trigger => run_payload_or_default!(
                                iroha_data_model::trigger::Trigger,
                                iroha_data_model::query::trigger::prelude::FindTriggers
                            ),
                            QueryItemKind::CommittedTransaction => {
                                let pred = dec::<CompoundPredicate<CommittedTransaction>>(
                                    &iter_query.predicate_bytes,
                                )?;
                                let sel = dec::<SelectorTuple<CommittedTransaction>>(
                                    &iter_query.selector_bytes,
                                )?;
                                let output = handle_find_transactions_stored(
                                    state,
                                    pred,
                                    sel,
                                    params,
                                    limits,
                                    live_query_store,
                                    authority,
                                    stored_cursor_budget,
                                    replay_state.clone(),
                                )?;
                                return Ok(QueryResponse::Iterable(output));
                            }
                            QueryItemKind::SignedBlock => run_payload_or_default!(
                                iroha_data_model::block::SignedBlock,
                                iroha_data_model::query::block::prelude::FindBlocks
                            ),
                            QueryItemKind::BlockHeader => run_payload_or_default!(
                                iroha_data_model::block::BlockHeader,
                                iroha_data_model::query::block::prelude::FindBlockHeaders
                            ),
                            QueryItemKind::ProofRecord => {
                                let pred = dec::<
                                    iroha_data_model::query::dsl::CompoundPredicate<
                                        iroha_data_model::proof::ProofRecord,
                                    >,
                                >(
                                    &iter_query.predicate_bytes
                                )?;
                                let sel = dec::<
                                    iroha_data_model::query::dsl::SelectorTuple<
                                        iroha_data_model::proof::ProofRecord,
                                    >,
                                >(
                                    &iter_query.selector_bytes
                                )?;
                                macro_rules! try_proof_query {
                                    ($find:ty) => {{
                                        let mut cursor =
                                            std::io::Cursor::new(&iter_query.query_payload);
                                        if let Ok(concrete) =
                                            <$find as norito::codec::Decode>::decode(&mut cursor)
                                            && usize::try_from(cursor.position())
                                                .unwrap_or(usize::MAX)
                                                == iter_query.query_payload.len()
                                        {
                                            let iter = ValidQuery::execute(
                                                concrete.clone(),
                                                pred.clone(),
                                                state,
                                            )?;
                                            let output = handle_iter_start_stored_replayable(
                                                iter,
                                                concrete,
                                                pred,
                                                sel,
                                                params,
                                                limits,
                                                live_query_store,
                                                authority,
                                                stored_cursor_budget,
                                                replay_state.clone(),
                                            )?;
                                            return Ok(QueryResponse::Iterable(output));
                                        }
                                    }};
                                }
                                if !iter_query.query_payload.is_empty() {
                                    try_proof_query!(
                                        iroha_data_model::query::proof::prelude::FindProofRecordsByBackend
                                    );
                                    try_proof_query!(
                                        iroha_data_model::query::proof::prelude::FindProofRecordsByStatus
                                    );
                                    return Err(Error::Conversion(
                                        "failed to decode proof query payload".into(),
                                    ));
                                }
                                let concrete =
                                    iroha_data_model::query::proof::prelude::FindProofRecords;
                                let iter =
                                    ValidQuery::execute(concrete.clone(), pred.clone(), state)?;
                                let output = handle_iter_start_stored_replayable(
                                    iter,
                                    concrete,
                                    pred,
                                    sel,
                                    params,
                                    limits,
                                    live_query_store,
                                    authority,
                                    stored_cursor_budget,
                                    replay_state.clone(),
                                )?;
                                return Ok(QueryResponse::Iterable(output));
                            }
                            QueryItemKind::AssetEscrowRecord => run_payload_or_default!(
                                iroha_data_model::escrow::AssetEscrowRecord,
                                iroha_data_model::query::escrow::prelude::FindAssetEscrows
                            ),
                            QueryItemKind::AnonymousAssetEscrowRecord => {
                                run_payload_or_default!(
                                    iroha_data_model::escrow::AnonymousAssetEscrowRecord,
                                    iroha_data_model::query::escrow::prelude::FindAnonymousAssetEscrows
                                )
                            }
                            QueryItemKind::OracleFeedConfig => run_payload_or_default!(
                                iroha_data_model::oracle::FeedConfig,
                                iroha_data_model::query::oracle::prelude::FindOracleFeeds
                            ),
                            QueryItemKind::OracleFeedEventRecord => {
                                run_payload_or_default!(require_payload iroha_data_model::events::data::oracle::FeedEventRecord, iroha_data_model::query::oracle::prelude::FindOracleHistoryByFeedId)
                            }
                            QueryItemKind::OracleProviderStatsRecord => {
                                run_payload_or_default!(require_payload iroha_data_model::oracle::OracleProviderStatsRecord, iroha_data_model::query::oracle::prelude::FindOracleProviderStatsByFeedId)
                            }
                            QueryItemKind::OracleDispute => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::oracle::OracleDispute,
                                        iroha_data_model::query::oracle::prelude::FindOracleDisputesByFeedId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::oracle::OracleDispute,
                                    iroha_data_model::query::oracle::prelude::FindOracleDisputes
                                )
                            }
                            QueryItemKind::OracleChangeProposal => run_payload_or_default!(
                                iroha_data_model::oracle::OracleChangeProposal,
                                iroha_data_model::query::oracle::prelude::FindOracleChanges
                            ),
                            QueryItemKind::TwitterBindingRecord => {
                                run_payload_or_default!(require_payload iroha_data_model::oracle::TwitterBindingRecord, iroha_data_model::query::oracle::prelude::FindTwitterBindingsByUaid)
                            }
                            QueryItemKind::DefiOracleAttestation => {
                                run_payload_or_default!(require_payload iroha_data_model::oracle::DefiOracleAttestation, iroha_data_model::query::oracle::prelude::FindDefiOracleAttestationsByKey)
                            }
                            QueryItemKind::Permission => {
                                run_payload_or_default!(require_payload iroha_data_model::permission::Permission, iroha_data_model::query::permission::prelude::FindPermissionsByAccountId)
                            }
                            QueryItemKind::FeeSponsorPolicy => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(require_payload iroha_data_model::nexus::FeeSponsorPolicy, iroha_data_model::query::nexus::prelude::FindFeeSponsorPoliciesBySponsor)
                                }
                                run_payload_or_default!(
                                    iroha_data_model::nexus::FeeSponsorPolicy,
                                    iroha_data_model::query::nexus::prelude::FindFeeSponsorPolicies
                                )
                            }
                            QueryItemKind::FeeSponsorPolicyId => run_payload_or_default!(
                                iroha_data_model::nexus::FeeSponsorPolicyId,
                                iroha_data_model::query::nexus::prelude::FindFeeSponsorPolicyIds
                            ),
                        }
                    }
                    #[cfg(any())]
                    {
                        // unreachable: iroha_core is built with std; fast_dsl iterable path requires std in data_model.
                        return Err(Error::Conversion(
                            "fast_dsl iterable path requires std".into(),
                        ));
                    }
                }
                // Fallback for fast_dsl-enabled callers: if the boxed query is absent,
                // reconstruct a default iterable query from the item kind.
                if iter_query.query_box().is_none() {
                    use iroha_data_model::query::QueryItemKind;
                    fn dec<T: norito::codec::Decode>(bytes: &[u8]) -> Result<T, Error> {
                        let mut cursor = std::io::Cursor::new(bytes);
                        norito::codec::Decode::decode(&mut cursor).map_err(|_| {
                            Error::Conversion("failed to decode query predicate/selector".into())
                        })
                    }
                    macro_rules! run_unit {
                        ($itemty:ty, $find:ty) => {{
                            let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                dec(&iter_query.predicate_bytes)?;
                            let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                dec(&iter_query.selector_bytes)?;
                            let concrete = <$find>::new();
                            let iter = ValidQuery::execute(concrete.clone(), pred.clone(), state)?;
                            let output = handle_iter_start_stored_replayable(
                                iter,
                                concrete,
                                pred,
                                sel,
                                params,
                                limits,
                                live_query_store,
                                authority,
                                stored_cursor_budget,
                                replay_state.clone(),
                            )?;
                            return Ok(QueryResponse::Iterable(output));
                        }};
                    }
                    match iter_query.item {
                        QueryItemKind::Domain => run_unit!(
                            iroha_data_model::domain::Domain,
                            iroha_data_model::query::domain::prelude::FindDomains
                        ),
                        QueryItemKind::Account => run_unit!(
                            iroha_data_model::account::Account,
                            iroha_data_model::query::account::prelude::FindAccounts
                        ),
                        QueryItemKind::AccountId => run_unit!(
                            iroha_data_model::account::AccountId,
                            iroha_data_model::query::account::prelude::FindAccountIds
                        ),
                        QueryItemKind::Asset => run_unit!(
                            iroha_data_model::asset::value::Asset,
                            iroha_data_model::query::asset::prelude::FindAssets
                        ),
                        QueryItemKind::AssetDefinition => run_unit!(
                            iroha_data_model::asset::definition::AssetDefinition,
                            iroha_data_model::query::asset::prelude::FindAssetsDefinitions
                        ),
                        QueryItemKind::RepoAgreement => run_unit!(
                            iroha_data_model::repo::RepoAgreement,
                            iroha_data_model::query::repo::prelude::FindRepoAgreements
                        ),
                        QueryItemKind::Nft => run_unit!(
                            iroha_data_model::nft::Nft,
                            iroha_data_model::query::nft::prelude::FindNfts
                        ),
                        QueryItemKind::Rwa => run_unit!(
                            iroha_data_model::rwa::Rwa,
                            iroha_data_model::query::rwa::prelude::FindRwas
                        ),
                        QueryItemKind::Role => run_unit!(
                            iroha_data_model::role::Role,
                            iroha_data_model::query::role::prelude::FindRoles
                        ),
                        QueryItemKind::RoleId => run_unit!(
                            iroha_data_model::role::RoleId,
                            iroha_data_model::query::role::prelude::FindRoleIds
                        ),
                        QueryItemKind::PeerId => run_unit!(
                            iroha_data_model::peer::PeerId,
                            iroha_data_model::query::peer::prelude::FindPeers
                        ),
                        QueryItemKind::TriggerId => run_unit!(
                            iroha_data_model::trigger::TriggerId,
                            iroha_data_model::query::trigger::prelude::FindActiveTriggerIds
                        ),
                        QueryItemKind::Trigger => run_unit!(
                            iroha_data_model::trigger::Trigger,
                            iroha_data_model::query::trigger::prelude::FindTriggers
                        ),
                        QueryItemKind::CommittedTransaction => run_unit!(
                            iroha_data_model::query::CommittedTransaction,
                            iroha_data_model::query::transaction::prelude::FindTransactions
                        ),
                        QueryItemKind::SignedBlock => run_unit!(
                            iroha_data_model::block::SignedBlock,
                            iroha_data_model::query::block::prelude::FindBlocks
                        ),
                        QueryItemKind::BlockHeader => run_unit!(
                            iroha_data_model::block::BlockHeader,
                            iroha_data_model::query::block::prelude::FindBlockHeaders
                        ),
                        QueryItemKind::ProofRecord => run_unit!(
                            iroha_data_model::proof::ProofRecord,
                            iroha_data_model::query::proof::prelude::FindProofRecords
                        ),
                        QueryItemKind::AssetEscrowRecord => run_unit!(
                            iroha_data_model::escrow::AssetEscrowRecord,
                            iroha_data_model::query::escrow::prelude::FindAssetEscrows
                        ),
                        QueryItemKind::AnonymousAssetEscrowRecord => run_unit!(
                            iroha_data_model::escrow::AnonymousAssetEscrowRecord,
                            iroha_data_model::query::escrow::prelude::FindAnonymousAssetEscrows
                        ),
                        QueryItemKind::OracleFeedConfig => run_unit!(
                            iroha_data_model::oracle::FeedConfig,
                            iroha_data_model::query::oracle::prelude::FindOracleFeeds
                        ),
                        QueryItemKind::OracleFeedEventRecord
                        | QueryItemKind::OracleProviderStatsRecord
                        | QueryItemKind::TwitterBindingRecord
                        | QueryItemKind::DefiOracleAttestation => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                        QueryItemKind::OracleDispute => run_unit!(
                            iroha_data_model::oracle::OracleDispute,
                            iroha_data_model::query::oracle::prelude::FindOracleDisputes
                        ),
                        QueryItemKind::OracleChangeProposal => run_unit!(
                            iroha_data_model::oracle::OracleChangeProposal,
                            iroha_data_model::query::oracle::prelude::FindOracleChanges
                        ),
                        QueryItemKind::Permission => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                        QueryItemKind::FeeSponsorPolicy => run_unit!(
                            iroha_data_model::nexus::FeeSponsorPolicy,
                            iroha_data_model::query::nexus::prelude::FindFeeSponsorPolicies
                        ),
                        QueryItemKind::FeeSponsorPolicyId => run_unit!(
                            iroha_data_model::nexus::FeeSponsorPolicyId,
                            iroha_data_model::query::nexus::prelude::FindFeeSponsorPolicyIds
                        ),
                    }
                }
                if iter_query.query_box().is_none() {
                    use iroha_data_model::query::QueryItemKind;
                    fn dec<T: norito::codec::Decode>(bytes: &[u8]) -> Result<T, Error> {
                        let mut cursor = std::io::Cursor::new(bytes);
                        norito::codec::Decode::decode(&mut cursor).map_err(|_| {
                            Error::Conversion("failed to decode query predicate/selector".into())
                        })
                    }
                    macro_rules! run_unit {
                        ($itemty:ty, $find:ty) => {{
                            let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                dec(&iter_query.predicate_bytes)?;
                            let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                dec(&iter_query.selector_bytes)?;
                            let iter = ValidQuery::execute(<$find>::new(), pred, state)?;
                            let (output, _processed_items) =
                                apply_query_postprocessing_ephemeral_with_budget(
                                    iter, sel, params, limits, None,
                                )?;
                            return Ok(QueryResponse::Iterable(output));
                        }};
                    }
                    match iter_query.item {
                        QueryItemKind::Domain => run_unit!(
                            iroha_data_model::domain::Domain,
                            iroha_data_model::query::domain::prelude::FindDomains
                        ),
                        QueryItemKind::Account => run_unit!(
                            iroha_data_model::account::Account,
                            iroha_data_model::query::account::prelude::FindAccounts
                        ),
                        QueryItemKind::AccountId => run_unit!(
                            iroha_data_model::account::AccountId,
                            iroha_data_model::query::account::prelude::FindAccountIds
                        ),
                        QueryItemKind::Asset => run_unit!(
                            iroha_data_model::asset::value::Asset,
                            iroha_data_model::query::asset::prelude::FindAssets
                        ),
                        QueryItemKind::AssetDefinition => run_unit!(
                            iroha_data_model::asset::definition::AssetDefinition,
                            iroha_data_model::query::asset::prelude::FindAssetsDefinitions
                        ),
                        QueryItemKind::RepoAgreement => run_unit!(
                            iroha_data_model::repo::RepoAgreement,
                            iroha_data_model::query::repo::prelude::FindRepoAgreements
                        ),
                        QueryItemKind::Nft => run_unit!(
                            iroha_data_model::nft::Nft,
                            iroha_data_model::query::nft::prelude::FindNfts
                        ),
                        QueryItemKind::Rwa => run_unit!(
                            iroha_data_model::rwa::Rwa,
                            iroha_data_model::query::rwa::prelude::FindRwas
                        ),
                        QueryItemKind::Role => run_unit!(
                            iroha_data_model::role::Role,
                            iroha_data_model::query::role::prelude::FindRoles
                        ),
                        QueryItemKind::RoleId => run_unit!(
                            iroha_data_model::role::RoleId,
                            iroha_data_model::query::role::prelude::FindRoleIds
                        ),
                        QueryItemKind::PeerId => run_unit!(
                            iroha_data_model::peer::PeerId,
                            iroha_data_model::query::peer::prelude::FindPeers
                        ),
                        QueryItemKind::TriggerId => run_unit!(
                            iroha_data_model::trigger::TriggerId,
                            iroha_data_model::query::trigger::prelude::FindActiveTriggerIds
                        ),
                        QueryItemKind::Trigger => run_unit!(
                            iroha_data_model::trigger::Trigger,
                            iroha_data_model::query::trigger::prelude::FindTriggers
                        ),
                        QueryItemKind::CommittedTransaction => run_unit!(
                            iroha_data_model::query::CommittedTransaction,
                            iroha_data_model::query::transaction::prelude::FindTransactions
                        ),
                        QueryItemKind::SignedBlock => run_unit!(
                            iroha_data_model::block::SignedBlock,
                            iroha_data_model::query::block::prelude::FindBlocks
                        ),
                        QueryItemKind::BlockHeader => run_unit!(
                            iroha_data_model::block::BlockHeader,
                            iroha_data_model::query::block::prelude::FindBlockHeaders
                        ),
                        QueryItemKind::ProofRecord => run_unit!(
                            iroha_data_model::proof::ProofRecord,
                            iroha_data_model::query::proof::prelude::FindProofRecords
                        ),
                        QueryItemKind::AssetEscrowRecord => run_unit!(
                            iroha_data_model::escrow::AssetEscrowRecord,
                            iroha_data_model::query::escrow::prelude::FindAssetEscrows
                        ),
                        QueryItemKind::AnonymousAssetEscrowRecord => run_unit!(
                            iroha_data_model::escrow::AnonymousAssetEscrowRecord,
                            iroha_data_model::query::escrow::prelude::FindAnonymousAssetEscrows
                        ),
                        QueryItemKind::OracleFeedConfig => run_unit!(
                            iroha_data_model::oracle::FeedConfig,
                            iroha_data_model::query::oracle::prelude::FindOracleFeeds
                        ),
                        QueryItemKind::OracleFeedEventRecord
                        | QueryItemKind::OracleProviderStatsRecord
                        | QueryItemKind::TwitterBindingRecord
                        | QueryItemKind::DefiOracleAttestation => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                        QueryItemKind::OracleDispute => run_unit!(
                            iroha_data_model::oracle::OracleDispute,
                            iroha_data_model::query::oracle::prelude::FindOracleDisputes
                        ),
                        QueryItemKind::OracleChangeProposal => run_unit!(
                            iroha_data_model::oracle::OracleChangeProposal,
                            iroha_data_model::query::oracle::prelude::FindOracleChanges
                        ),
                        QueryItemKind::Permission => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                        QueryItemKind::FeeSponsorPolicy => run_unit!(
                            iroha_data_model::nexus::FeeSponsorPolicy,
                            iroha_data_model::query::nexus::prelude::FindFeeSponsorPolicies
                        ),
                        QueryItemKind::FeeSponsorPolicyId => run_unit!(
                            iroha_data_model::nexus::FeeSponsorPolicyId,
                            iroha_data_model::query::nexus::prelude::FindFeeSponsorPolicyIds
                        ),
                    }
                }
                let Some(qbox) = iter_query.query_box() else {
                    // Final fallback: default unit iterable by item kind
                    use iroha_data_model::query::QueryItemKind;
                    fn dec<T: norito::codec::Decode>(bytes: &[u8]) -> Result<T, Error> {
                        let mut cursor = std::io::Cursor::new(bytes);
                        norito::codec::Decode::decode(&mut cursor).map_err(|_| {
                            Error::Conversion("failed to decode query predicate/selector".into())
                        })
                    }
                    macro_rules! run_unit {
                        ($itemty:ty, $find:ty) => {{
                            let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                dec(&iter_query.predicate_bytes)?;
                            let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                dec(&iter_query.selector_bytes)?;
                            let concrete = <$find>::new();
                            let iter = ValidQuery::execute(concrete.clone(), pred.clone(), state)?;
                            let output = handle_iter_start_stored_replayable(
                                iter,
                                concrete,
                                pred,
                                sel,
                                params,
                                limits,
                                live_query_store,
                                authority,
                                stored_cursor_budget,
                                replay_state.clone(),
                            )?;
                            return Ok(QueryResponse::Iterable(output));
                        }};
                    }
                    match iter_query.item {
                        QueryItemKind::Domain => run_unit!(
                            iroha_data_model::domain::Domain,
                            iroha_data_model::query::domain::prelude::FindDomains
                        ),
                        QueryItemKind::Account => run_unit!(
                            iroha_data_model::account::Account,
                            iroha_data_model::query::account::prelude::FindAccounts
                        ),
                        QueryItemKind::AccountId => run_unit!(
                            iroha_data_model::account::AccountId,
                            iroha_data_model::query::account::prelude::FindAccountIds
                        ),
                        QueryItemKind::Asset => run_unit!(
                            iroha_data_model::asset::value::Asset,
                            iroha_data_model::query::asset::prelude::FindAssets
                        ),
                        QueryItemKind::AssetDefinition => run_unit!(
                            iroha_data_model::asset::definition::AssetDefinition,
                            iroha_data_model::query::asset::prelude::FindAssetsDefinitions
                        ),
                        QueryItemKind::RepoAgreement => run_unit!(
                            iroha_data_model::repo::RepoAgreement,
                            iroha_data_model::query::repo::prelude::FindRepoAgreements
                        ),
                        QueryItemKind::Nft => run_unit!(
                            iroha_data_model::nft::Nft,
                            iroha_data_model::query::nft::prelude::FindNfts
                        ),
                        QueryItemKind::Rwa => run_unit!(
                            iroha_data_model::rwa::Rwa,
                            iroha_data_model::query::rwa::prelude::FindRwas
                        ),
                        QueryItemKind::Role => run_unit!(
                            iroha_data_model::role::Role,
                            iroha_data_model::query::role::prelude::FindRoles
                        ),
                        QueryItemKind::RoleId => run_unit!(
                            iroha_data_model::role::RoleId,
                            iroha_data_model::query::role::prelude::FindRoleIds
                        ),
                        QueryItemKind::PeerId => run_unit!(
                            iroha_data_model::peer::PeerId,
                            iroha_data_model::query::peer::prelude::FindPeers
                        ),
                        QueryItemKind::TriggerId => run_unit!(
                            iroha_data_model::trigger::TriggerId,
                            iroha_data_model::query::trigger::prelude::FindActiveTriggerIds
                        ),
                        QueryItemKind::Trigger => run_unit!(
                            iroha_data_model::trigger::Trigger,
                            iroha_data_model::query::trigger::prelude::FindTriggers
                        ),
                        QueryItemKind::CommittedTransaction => run_unit!(
                            iroha_data_model::query::CommittedTransaction,
                            iroha_data_model::query::transaction::prelude::FindTransactions
                        ),
                        QueryItemKind::SignedBlock => run_unit!(
                            iroha_data_model::block::SignedBlock,
                            iroha_data_model::query::block::prelude::FindBlocks
                        ),
                        QueryItemKind::BlockHeader => run_unit!(
                            iroha_data_model::block::BlockHeader,
                            iroha_data_model::query::block::prelude::FindBlockHeaders
                        ),
                        QueryItemKind::ProofRecord => run_unit!(
                            iroha_data_model::proof::ProofRecord,
                            iroha_data_model::query::proof::prelude::FindProofRecords
                        ),
                        QueryItemKind::AssetEscrowRecord => run_unit!(
                            iroha_data_model::escrow::AssetEscrowRecord,
                            iroha_data_model::query::escrow::prelude::FindAssetEscrows
                        ),
                        QueryItemKind::AnonymousAssetEscrowRecord => run_unit!(
                            iroha_data_model::escrow::AnonymousAssetEscrowRecord,
                            iroha_data_model::query::escrow::prelude::FindAnonymousAssetEscrows
                        ),
                        QueryItemKind::OracleFeedConfig => run_unit!(
                            iroha_data_model::oracle::FeedConfig,
                            iroha_data_model::query::oracle::prelude::FindOracleFeeds
                        ),
                        QueryItemKind::OracleFeedEventRecord
                        | QueryItemKind::OracleProviderStatsRecord
                        | QueryItemKind::TwitterBindingRecord
                        | QueryItemKind::DefiOracleAttestation => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                        QueryItemKind::OracleDispute => run_unit!(
                            iroha_data_model::oracle::OracleDispute,
                            iroha_data_model::query::oracle::prelude::FindOracleDisputes
                        ),
                        QueryItemKind::OracleChangeProposal => run_unit!(
                            iroha_data_model::oracle::OracleChangeProposal,
                            iroha_data_model::query::oracle::prelude::FindOracleChanges
                        ),
                        QueryItemKind::Permission => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                        QueryItemKind::FeeSponsorPolicy => run_unit!(
                            iroha_data_model::nexus::FeeSponsorPolicy,
                            iroha_data_model::query::nexus::prelude::FindFeeSponsorPolicies
                        ),
                        QueryItemKind::FeeSponsorPolicyId => run_unit!(
                            iroha_data_model::nexus::FeeSponsorPolicyId,
                            iroha_data_model::query::nexus::prelude::FindFeeSponsorPolicyIds
                        ),
                    }
                };

                // Try dispatch for all supported iterable queries, keyed by their item type.
                // For item types that have multiple concrete query variants (e.g., Account),
                // attempt decodes in priority order.
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::domain::Domain,
                    iroha_data_model::query::domain::prelude::FindDomainsByAccountId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::domain::prelude::FindDomainsByAccountId,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::domain::Domain,
                    iroha_data_model::query::domain::prelude::FindDomains,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::domain::prelude::FindDomains>(e)
                            .or(Some(iroha_data_model::query::domain::prelude::FindDomains))
                    },
                )? {
                    return Ok(resp);
                }
                // Accounts: support both `FindAccounts` and `FindAccountsWithAsset`
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::account::Account,
                    iroha_data_model::query::account::prelude::FindAccounts,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::account::prelude::FindAccounts>(
                            e,
                        )
                        .or(Some(
                            iroha_data_model::query::account::prelude::FindAccounts,
                        ))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::account::Account,
                    iroha_data_model::query::account::prelude::FindAccountsWithAsset,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::account::prelude::FindAccountsWithAsset,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::asset::value::Asset,
                    iroha_data_model::query::asset::prelude::FindAssetsByAccountId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::asset::prelude::FindAssetsByAccountId,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::asset::value::Asset,
                    iroha_data_model::query::asset::prelude::FindAssets,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::asset::prelude::FindAssets>(e)
                            .or(Some(iroha_data_model::query::asset::prelude::FindAssets))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::asset::definition::AssetDefinition,
                    iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
                        ))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::repo::RepoAgreement,
                    iroha_data_model::query::repo::prelude::FindRepoAgreements,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::repo::prelude::FindRepoAgreements,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::repo::prelude::FindRepoAgreements,
                        ))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::nft::Nft,
                    iroha_data_model::query::nft::prelude::FindNftsByAccountId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::nft::prelude::FindNftsByAccountId>(
                            e,
                        )
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::nft::Nft,
                    iroha_data_model::query::nft::prelude::FindNfts,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::nft::prelude::FindNfts>(e)
                            .or(Some(iroha_data_model::query::nft::prelude::FindNfts))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::role::Role,
                    iroha_data_model::query::role::prelude::FindRoles,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::role::prelude::FindRoles>(e)
                            .or(Some(iroha_data_model::query::role::prelude::FindRoles))
                    },
                )? {
                    return Ok(resp);
                }
                // RoleId: support both `FindRoleIds` and `FindRolesByAccountId`.
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::role::RoleId,
                    iroha_data_model::query::role::prelude::FindRoleIds,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::role::prelude::FindRoleIds>(e)
                            .or(Some(iroha_data_model::query::role::prelude::FindRoleIds))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::role::RoleId,
                    iroha_data_model::query::role::prelude::FindRolesByAccountId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::role::prelude::FindRolesByAccountId,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::proof::ProofRecord,
                    iroha_data_model::query::proof::prelude::FindProofRecordsByBackend,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::proof::prelude::FindProofRecordsByBackend,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::proof::ProofRecord,
                    iroha_data_model::query::proof::prelude::FindProofRecordsByStatus,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::proof::prelude::FindProofRecordsByStatus,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::proof::ProofRecord,
                    iroha_data_model::query::proof::prelude::FindProofRecords,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::proof::prelude::FindProofRecords>(
                            e,
                        )
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::peer::PeerId,
                    iroha_data_model::query::peer::prelude::FindPeers,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::peer::prelude::FindPeers>(e)
                            .or(Some(iroha_data_model::query::peer::prelude::FindPeers))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::trigger::TriggerId,
                    iroha_data_model::query::trigger::prelude::FindActiveTriggerIds,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::trigger::prelude::FindActiveTriggerIds,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::trigger::prelude::FindActiveTriggerIds,
                        ))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::trigger::Trigger,
                    iroha_data_model::query::trigger::prelude::FindTriggers,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::trigger::prelude::FindTriggers>(
                            e,
                        )
                        .or(Some(
                            iroha_data_model::query::trigger::prelude::FindTriggers,
                        ))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(erased) = query::iter_query_inner::<CommittedTransaction>(qbox) {
                    let output = handle_find_transactions_stored(
                        state,
                        erased.predicate_cloned(),
                        erased.selector_cloned(),
                        params,
                        limits,
                        live_query_store,
                        authority,
                        stored_cursor_budget,
                        replay_state.clone(),
                    )?;
                    return Ok(QueryResponse::Iterable(output));
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::block::SignedBlock,
                    iroha_data_model::query::block::prelude::FindBlocks,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::block::prelude::FindBlocks>(e)
                            .or(Some(iroha_data_model::query::block::prelude::FindBlocks))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::block::BlockHeader,
                    iroha_data_model::query::block::prelude::FindBlockHeaders,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<iroha_data_model::query::block::prelude::FindBlockHeaders>(
                            e,
                        )
                        .or(Some(
                            iroha_data_model::query::block::prelude::FindBlockHeaders,
                        ))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::oracle::FeedConfig,
                    iroha_data_model::query::oracle::prelude::FindOracleFeeds,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleFeeds,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::oracle::prelude::FindOracleFeeds,
                        ))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::events::data::oracle::FeedEventRecord,
                    iroha_data_model::query::oracle::prelude::FindOracleHistoryByFeedId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleHistoryByFeedId,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::oracle::OracleProviderStatsRecord,
                    iroha_data_model::query::oracle::prelude::FindOracleProviderStatsByFeedId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleProviderStatsByFeedId,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::oracle::OracleDispute,
                    iroha_data_model::query::oracle::prelude::FindOracleDisputesByFeedId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleDisputesByFeedId,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::oracle::OracleDispute,
                    iroha_data_model::query::oracle::prelude::FindOracleDisputes,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleDisputes,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::oracle::prelude::FindOracleDisputes,
                        ))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::oracle::OracleChangeProposal,
                    iroha_data_model::query::oracle::prelude::FindOracleChanges,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleChanges,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::oracle::prelude::FindOracleChanges,
                        ))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::oracle::TwitterBindingRecord,
                    iroha_data_model::query::oracle::prelude::FindTwitterBindingsByUaid,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindTwitterBindingsByUaid,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::oracle::DefiOracleAttestation,
                    iroha_data_model::query::oracle::prelude::FindDefiOracleAttestationsByKey,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    replay_state.clone(),
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindDefiOracleAttestationsByKey,
                        >(e)
                    },
                )? {
                    return Ok(resp);
                }

                Err(Error::Conversion(
                    "unsupported iterable query type".to_string(),
                ))
            }
            QueryRequest::Continue(cursor) => Ok(QueryResponse::Iterable(
                live_query_store.handle_iter_continue(cursor, authority)?,
            )),
        }
    }

    /// Execute a validated query request using an ephemeral iterator for iterable queries.
    ///
    /// Iterable queries return only the first batch and do not allocate a
    /// reusable cursor in the [`LiveQueryStore`]. Suitable for snapshot-bound
    /// contexts where queries must not outlive the captured view.
    ///
    /// # Errors
    /// Returns an error if the query execution fails.
    pub fn execute_ephemeral(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
    ) -> Result<QueryResponse, Error> {
        self.execute_ephemeral_with_stats(live_query_store, state, authority, None)
            .map(|(response, _)| response)
    }

    pub(crate) fn execute_ephemeral_with_stats(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<(QueryResponse, QueryExecutionStats), Error> {
        let (response, mut stats) =
            self.execute_ephemeral_inner_with_stats(live_query_store, state, authority, budget)?;
        stats.record_response(&response, budget)?;
        Ok((response, stats))
    }

    #[allow(clippy::too_many_lines)]
    fn execute_ephemeral_inner_with_stats(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
        budget: Option<QueryExecutionBudget>,
    ) -> Result<(QueryResponse, QueryExecutionStats), Error> {
        let Self { request, limits } = self;
        let budget_items = budget;
        match request {
            QueryRequest::Singular(singular_query) => {
                let source_bytes =
                    preflight_singular_source_materialization(&singular_query, state, budget)?;
                let mut stats = QueryExecutionStats::default();
                // The borrowed preflight is real deterministic work and must
                // be admitted together with the one result item before the
                // query clones or synthesizes its owned output.
                stats.record_preflighted_item(source_bytes, budget)?;
                let output = singular_query.execute(state)?;
                // Materializing the generic output and framing the response
                // are separate serialization passes, charged below and by
                // `execute_ephemeral_with_stats`, respectively.
                stats.record_value_bytes(&output, budget)?;
                Ok((QueryResponse::Singular(output), stats))
            }
            QueryRequest::Start(iter_query) => {
                use iroha_data_model::query;

                fn try_decode_query<Q>(
                    erased: &query::ErasedIterQuery<
                        impl HasProjection<PredicateMarker>
                        + HasProjection<SelectorMarker, AtomType = ()>
                        + Send
                        + Sync,
                    >,
                ) -> Option<Q>
                where
                    Q: norito::codec::Decode,
                {
                    let bytes = erased.payload();
                    let mut cur = bytes;
                    let query = Q::decode(&mut cur).ok()?;
                    cur.is_empty().then_some(query)
                }

                #[allow(clippy::too_many_arguments)]
                fn run_dispatch<T, Q, F>(
                    qbox: &query::QueryBox<query::QueryOutputBatchBox>,
                    params: &query::parameters::QueryParams,
                    limits: QueryLimits,
                    budget: Option<QueryExecutionBudget>,
                    state: &impl StateReadOnly,
                    _live_query_store: &LiveQueryStoreHandle,
                    _authority: &AccountId,
                    __stored_cursor_budget: Option<u64>,
                    decode: F,
                ) -> Result<Option<(QueryResponse, QueryExecutionStats)>, Error>
                where
                    T: Send + Sync + 'static,
                    Q: super::super::ValidQuery<Item = T>,
                    T: HasProjection<SelectorMarker, AtomType = ()>
                        + HasProjection<PredicateMarker>
                        + crate::smartcontracts::isi::query::SortableQueryOutput
                        + NoritoSerialize
                        + Send
                        + Sync
                        + 'static,
                    <T as HasProjection<SelectorMarker>>::Projection:
                        EvaluateSelector<T> + Send + Sync,
                    query::QueryOutputBatchBox: From<Vec<T>>,
                    F: Fn(&query::ErasedIterQuery<T>) -> Option<Q>,
                {
                    if let Some(erased) = query::iter_query_inner::<T>(qbox) {
                        // Decode the concrete query variant from the payload
                        let Some(concrete) = decode(erased) else {
                            return Ok(None);
                        };
                        // Execute the concrete ValidQuery with provided predicate
                        let iter = ValidQuery::execute(concrete, erased.predicate_cloned(), state)?;

                        // Postprocess: sort/paginate/project and return only the first batch (no cursor)
                        let (output, stats) = apply_query_postprocessing_ephemeral_with_budget(
                            iter,
                            erased.selector_cloned(),
                            params,
                            limits,
                            budget,
                        )?;
                        return Ok(Some((QueryResponse::Iterable(output), stats)));
                    }
                    Ok(None)
                }

                let params = &iter_query.params;
                // Fast-DSL path: when the boxed query payload is not present, reconstruct
                // from item kind and encoded predicate/selector.
                if iter_query.query_box().is_none() {
                    #[cfg(feature = "fast_dsl")]
                    {
                        use iroha_data_model::query::QueryItemKind;
                        // Helpers to decode bytes into concrete predicate/selector
                        fn dec<T: norito::codec::Decode>(bytes: &[u8]) -> Result<T, Error> {
                            let mut cursor = std::io::Cursor::new(bytes);
                            norito::codec::Decode::decode(&mut cursor).map_err(|_| {
                                Error::Conversion(
                                    "failed to decode query predicate/selector".into(),
                                )
                            })
                        }
                        // Helper to run a unit iterable query ("find all ...") using the encoded predicate/selector.
                        macro_rules! run_payload_or_default {
                            // For unit queries: ignore payload and run the default constructor (FindX::new())
                            ($itemty:ty, $find:ty) => {{
                                let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                    dec(&iter_query.predicate_bytes)?;
                                let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                    dec(&iter_query.selector_bytes)?;
                                let iter = ValidQuery::execute(<$find>::new(), pred, state)?;
                                let (output, processed_items) =
                                    apply_query_postprocessing_ephemeral_with_budget(
                                        iter,
                                        sel,
                                        params,
                                        limits,
                                        budget_items,
                                    )?;
                                return Ok((QueryResponse::Iterable(output), processed_items));
                            }};
                            // For queries that always require a payload (e.g., FindPermissionsByAccountId)
                            (require_payload $itemty:ty, $find:ty) => {{
                                let pred: iroha_data_model::query::dsl::CompoundPredicate<$itemty> =
                                    dec(&iter_query.predicate_bytes)?;
                                let sel: iroha_data_model::query::dsl::SelectorTuple<$itemty> =
                                    dec(&iter_query.selector_bytes)?;
                                let mut cursor = std::io::Cursor::new(&iter_query.query_payload);
                                let concrete = <$find as norito::codec::Decode>::decode(
                                    &mut cursor,
                                )
                                .map_err(|_| {
                                    Error::Conversion("missing or malformed query payload".into())
                                })?;
                                let iter = ValidQuery::execute(concrete, pred, state)?;
                                let (output, processed_items) =
                                    apply_query_postprocessing_ephemeral_with_budget(
                                        iter,
                                        sel,
                                        params,
                                        limits,
                                        budget_items,
                                    )?;
                                return Ok((QueryResponse::Iterable(output), processed_items));
                            }};
                        }
                        match iter_query.item {
                            QueryItemKind::Domain => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::domain::Domain,
                                        iroha_data_model::query::domain::prelude::FindDomainsByAccountId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::domain::Domain,
                                    iroha_data_model::query::domain::prelude::FindDomains
                                )
                            }
                            QueryItemKind::Account => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(require_payload iroha_data_model::account::Account, iroha_data_model::query::account::prelude::FindAccountsWithAsset)
                                }
                                run_payload_or_default!(
                                    iroha_data_model::account::Account,
                                    iroha_data_model::query::account::prelude::FindAccounts
                                )
                            }
                            QueryItemKind::AccountId => run_payload_or_default!(
                                iroha_data_model::account::AccountId,
                                iroha_data_model::query::account::prelude::FindAccountIds
                            ),
                            QueryItemKind::Asset => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::asset::value::Asset,
                                        iroha_data_model::query::asset::prelude::FindAssetsByAccountId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::asset::value::Asset,
                                    iroha_data_model::query::asset::prelude::FindAssets
                                )
                            }
                            QueryItemKind::AssetDefinition => run_payload_or_default!(
                                iroha_data_model::asset::definition::AssetDefinition,
                                iroha_data_model::query::asset::prelude::FindAssetsDefinitions
                            ),
                            QueryItemKind::RepoAgreement => run_payload_or_default!(
                                iroha_data_model::repo::RepoAgreement,
                                iroha_data_model::query::repo::prelude::FindRepoAgreements
                            ),
                            QueryItemKind::Nft => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::nft::Nft,
                                        iroha_data_model::query::nft::prelude::FindNftsByAccountId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::nft::Nft,
                                    iroha_data_model::query::nft::prelude::FindNfts
                                )
                            }
                            QueryItemKind::Rwa => run_payload_or_default!(
                                iroha_data_model::rwa::Rwa,
                                iroha_data_model::query::rwa::prelude::FindRwas
                            ),
                            QueryItemKind::Role => run_payload_or_default!(
                                iroha_data_model::role::Role,
                                iroha_data_model::query::role::prelude::FindRoles
                            ),
                            QueryItemKind::RoleId => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(require_payload iroha_data_model::role::RoleId, iroha_data_model::query::role::prelude::FindRolesByAccountId)
                                }
                                run_payload_or_default!(
                                    iroha_data_model::role::RoleId,
                                    iroha_data_model::query::role::prelude::FindRoleIds
                                )
                            }
                            QueryItemKind::PeerId => run_payload_or_default!(
                                iroha_data_model::peer::PeerId,
                                iroha_data_model::query::peer::prelude::FindPeers
                            ),
                            QueryItemKind::TriggerId => run_payload_or_default!(
                                iroha_data_model::trigger::TriggerId,
                                iroha_data_model::query::trigger::prelude::FindActiveTriggerIds
                            ),
                            QueryItemKind::Trigger => run_payload_or_default!(
                                iroha_data_model::trigger::Trigger,
                                iroha_data_model::query::trigger::prelude::FindTriggers
                            ),
                            QueryItemKind::CommittedTransaction => {
                                let pred = dec::<CompoundPredicate<CommittedTransaction>>(
                                    &iter_query.predicate_bytes,
                                )?;
                                let sel = dec::<SelectorTuple<CommittedTransaction>>(
                                    &iter_query.selector_bytes,
                                )?;
                                let (output, processed_items) = handle_find_transactions_ephemeral(
                                    state,
                                    pred,
                                    sel,
                                    params,
                                    limits,
                                    budget_items,
                                )?;
                                return Ok((QueryResponse::Iterable(output), processed_items));
                            }
                            QueryItemKind::SignedBlock => run_payload_or_default!(
                                iroha_data_model::block::SignedBlock,
                                iroha_data_model::query::block::prelude::FindBlocks
                            ),
                            QueryItemKind::BlockHeader => run_payload_or_default!(
                                iroha_data_model::block::BlockHeader,
                                iroha_data_model::query::block::prelude::FindBlockHeaders
                            ),
                            QueryItemKind::ProofRecord => {
                                let pred = dec::<
                                    iroha_data_model::query::dsl::CompoundPredicate<
                                        iroha_data_model::proof::ProofRecord,
                                    >,
                                >(
                                    &iter_query.predicate_bytes
                                )?;
                                let sel = dec::<
                                    iroha_data_model::query::dsl::SelectorTuple<
                                        iroha_data_model::proof::ProofRecord,
                                    >,
                                >(
                                    &iter_query.selector_bytes
                                )?;
                                macro_rules! try_proof_query {
                                    ($find:ty) => {{
                                        let mut cursor =
                                            std::io::Cursor::new(&iter_query.query_payload);
                                        if let Ok(concrete) =
                                            <$find as norito::codec::Decode>::decode(&mut cursor)
                                            && usize::try_from(cursor.position())
                                                .unwrap_or(usize::MAX)
                                                == iter_query.query_payload.len()
                                        {
                                            let iter = ValidQuery::execute(concrete, pred, state)?;
                                            let (output, processed_items) =
                                                apply_query_postprocessing_ephemeral_with_budget(
                                                    iter,
                                                    sel,
                                                    params,
                                                    limits,
                                                    budget_items,
                                                )?;
                                            return Ok((
                                                QueryResponse::Iterable(output),
                                                processed_items,
                                            ));
                                        }
                                    }};
                                }
                                if !iter_query.query_payload.is_empty() {
                                    try_proof_query!(
                                        iroha_data_model::query::proof::prelude::FindProofRecordsByBackend
                                    );
                                    try_proof_query!(
                                        iroha_data_model::query::proof::prelude::FindProofRecordsByStatus
                                    );
                                    return Err(Error::Conversion(
                                        "failed to decode proof query payload".into(),
                                    ));
                                }
                                let iter = ValidQuery::execute(
                                    iroha_data_model::query::proof::prelude::FindProofRecords,
                                    pred,
                                    state,
                                )?;
                                let (output, processed_items) =
                                    apply_query_postprocessing_ephemeral_with_budget(
                                        iter,
                                        sel,
                                        params,
                                        limits,
                                        budget_items,
                                    )?;
                                return Ok((QueryResponse::Iterable(output), processed_items));
                            }
                            QueryItemKind::AssetEscrowRecord => run_payload_or_default!(
                                iroha_data_model::escrow::AssetEscrowRecord,
                                iroha_data_model::query::escrow::prelude::FindAssetEscrows
                            ),
                            QueryItemKind::AnonymousAssetEscrowRecord => {
                                run_payload_or_default!(
                                    iroha_data_model::escrow::AnonymousAssetEscrowRecord,
                                    iroha_data_model::query::escrow::prelude::FindAnonymousAssetEscrows
                                )
                            }
                            QueryItemKind::OracleFeedConfig => run_payload_or_default!(
                                iroha_data_model::oracle::FeedConfig,
                                iroha_data_model::query::oracle::prelude::FindOracleFeeds
                            ),
                            QueryItemKind::OracleFeedEventRecord => {
                                run_payload_or_default!(require_payload iroha_data_model::events::data::oracle::FeedEventRecord, iroha_data_model::query::oracle::prelude::FindOracleHistoryByFeedId)
                            }
                            QueryItemKind::OracleProviderStatsRecord => {
                                run_payload_or_default!(require_payload iroha_data_model::oracle::OracleProviderStatsRecord, iroha_data_model::query::oracle::prelude::FindOracleProviderStatsByFeedId)
                            }
                            QueryItemKind::OracleDispute => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::oracle::OracleDispute,
                                        iroha_data_model::query::oracle::prelude::FindOracleDisputesByFeedId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::oracle::OracleDispute,
                                    iroha_data_model::query::oracle::prelude::FindOracleDisputes
                                )
                            }
                            QueryItemKind::OracleChangeProposal => run_payload_or_default!(
                                iroha_data_model::oracle::OracleChangeProposal,
                                iroha_data_model::query::oracle::prelude::FindOracleChanges
                            ),
                            QueryItemKind::TwitterBindingRecord => {
                                run_payload_or_default!(require_payload iroha_data_model::oracle::TwitterBindingRecord, iroha_data_model::query::oracle::prelude::FindTwitterBindingsByUaid)
                            }
                            QueryItemKind::DefiOracleAttestation => {
                                run_payload_or_default!(require_payload iroha_data_model::oracle::DefiOracleAttestation, iroha_data_model::query::oracle::prelude::FindDefiOracleAttestationsByKey)
                            }
                            QueryItemKind::Permission => {
                                run_payload_or_default!(require_payload iroha_data_model::permission::Permission, iroha_data_model::query::permission::prelude::FindPermissionsByAccountId)
                            }
                            QueryItemKind::FeeSponsorPolicy => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(require_payload iroha_data_model::nexus::FeeSponsorPolicy, iroha_data_model::query::nexus::prelude::FindFeeSponsorPoliciesBySponsor)
                                }
                                run_payload_or_default!(
                                    iroha_data_model::nexus::FeeSponsorPolicy,
                                    iroha_data_model::query::nexus::prelude::FindFeeSponsorPolicies
                                )
                            }
                            QueryItemKind::FeeSponsorPolicyId => run_payload_or_default!(
                                iroha_data_model::nexus::FeeSponsorPolicyId,
                                iroha_data_model::query::nexus::prelude::FindFeeSponsorPolicyIds
                            ),
                        }
                    }
                    #[cfg(not(feature = "fast_dsl"))]
                    {
                        return Err(Error::Conversion("missing iterator payload".into()));
                    }
                }
                let Some(qbox) = iter_query.query_box() else {
                    return Err(Error::Conversion("missing iterator payload".into()));
                };

                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::domain::Domain,
                    iroha_data_model::query::domain::prelude::FindDomainsByAccountId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::domain::prelude::FindDomainsByAccountId,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::domain::Domain,
                    iroha_data_model::query::domain::prelude::FindDomains,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::domain::prelude::FindDomains>(e)
                            .or(Some(iroha_data_model::query::domain::prelude::FindDomains))
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::account::Account,
                    iroha_data_model::query::account::prelude::FindAccounts,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::account::prelude::FindAccounts>(
                            e,
                        )
                        .or(Some(
                            iroha_data_model::query::account::prelude::FindAccounts,
                        ))
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::account::Account,
                    iroha_data_model::query::account::prelude::FindAccountsWithAsset,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::account::prelude::FindAccountsWithAsset,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::asset::value::Asset,
                    iroha_data_model::query::asset::prelude::FindAssetsByAccountId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::asset::prelude::FindAssetsByAccountId,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::asset::value::Asset,
                    iroha_data_model::query::asset::prelude::FindAssets,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::asset::prelude::FindAssets>(e)
                            .or(Some(iroha_data_model::query::asset::prelude::FindAssets))
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::asset::definition::AssetDefinition,
                    iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
                        ))
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::nft::Nft,
                    iroha_data_model::query::nft::prelude::FindNftsByAccountId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::nft::prelude::FindNftsByAccountId>(
                            e,
                        )
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::nft::Nft,
                    iroha_data_model::query::nft::prelude::FindNfts,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::nft::prelude::FindNfts>(e)
                            .or(Some(iroha_data_model::query::nft::prelude::FindNfts))
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::role::Role,
                    iroha_data_model::query::role::prelude::FindRoles,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::role::prelude::FindRoles>(e)
                            .or(Some(iroha_data_model::query::role::prelude::FindRoles))
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::role::RoleId,
                    iroha_data_model::query::role::prelude::FindRoleIds,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::role::prelude::FindRoleIds>(e)
                            .or(Some(iroha_data_model::query::role::prelude::FindRoleIds))
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::peer::PeerId,
                    iroha_data_model::query::peer::prelude::FindPeers,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::peer::prelude::FindPeers>(e)
                            .or(Some(iroha_data_model::query::peer::prelude::FindPeers))
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::trigger::TriggerId,
                    iroha_data_model::query::trigger::prelude::FindActiveTriggerIds,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::trigger::prelude::FindActiveTriggerIds,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::trigger::prelude::FindActiveTriggerIds,
                        ))
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::trigger::Trigger,
                    iroha_data_model::query::trigger::prelude::FindTriggers,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::trigger::prelude::FindTriggers>(
                            e,
                        )
                        .or(Some(
                            iroha_data_model::query::trigger::prelude::FindTriggers,
                        ))
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some(erased) = query::iter_query_inner::<CommittedTransaction>(qbox) {
                    let (output, processed_items) = handle_find_transactions_ephemeral(
                        state,
                        erased.predicate_cloned(),
                        erased.selector_cloned(),
                        params,
                        limits,
                        budget_items,
                    )?;
                    return Ok((QueryResponse::Iterable(output), processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::block::SignedBlock,
                    iroha_data_model::query::block::prelude::FindBlocks,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::block::prelude::FindBlocks>(e)
                            .or(Some(iroha_data_model::query::block::prelude::FindBlocks))
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::block::BlockHeader,
                    iroha_data_model::query::block::prelude::FindBlockHeaders,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                        iroha_data_model::query::block::prelude::FindBlockHeaders,
                    >(e)
                    .or(Some(
                        iroha_data_model::query::block::prelude::FindBlockHeaders,
                    ))
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::proof::ProofRecord,
                    iroha_data_model::query::proof::prelude::FindProofRecordsByBackend,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::proof::prelude::FindProofRecordsByBackend,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::proof::ProofRecord,
                    iroha_data_model::query::proof::prelude::FindProofRecordsByStatus,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::proof::prelude::FindProofRecordsByStatus,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::proof::ProofRecord,
                    iroha_data_model::query::proof::prelude::FindProofRecords,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<iroha_data_model::query::proof::prelude::FindProofRecords>(
                            e,
                        )
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::oracle::FeedConfig,
                    iroha_data_model::query::oracle::prelude::FindOracleFeeds,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleFeeds,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::oracle::prelude::FindOracleFeeds,
                        ))
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::events::data::oracle::FeedEventRecord,
                    iroha_data_model::query::oracle::prelude::FindOracleHistoryByFeedId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleHistoryByFeedId,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::oracle::OracleProviderStatsRecord,
                    iroha_data_model::query::oracle::prelude::FindOracleProviderStatsByFeedId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleProviderStatsByFeedId,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::oracle::OracleDispute,
                    iroha_data_model::query::oracle::prelude::FindOracleDisputesByFeedId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleDisputesByFeedId,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::oracle::OracleDispute,
                    iroha_data_model::query::oracle::prelude::FindOracleDisputes,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleDisputes,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::oracle::prelude::FindOracleDisputes,
                        ))
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::oracle::OracleChangeProposal,
                    iroha_data_model::query::oracle::prelude::FindOracleChanges,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindOracleChanges,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::oracle::prelude::FindOracleChanges,
                        ))
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::oracle::TwitterBindingRecord,
                    iroha_data_model::query::oracle::prelude::FindTwitterBindingsByUaid,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindTwitterBindingsByUaid,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::oracle::DefiOracleAttestation,
                    iroha_data_model::query::oracle::prelude::FindDefiOracleAttestationsByKey,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    budget_items,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::oracle::prelude::FindDefiOracleAttestationsByKey,
                        >(e)
                    },
                )? {
                    return Ok((resp, processed_items));
                }

                Err(Error::Conversion(
                    "unsupported iterable query in ephemeral execution".into(),
                ))
            }
            QueryRequest::Continue(_cursor) => Err(Error::Conversion(
                "ephemeral execution does not support continuation".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::many_single_char_names)]
    use core::time::Duration;
    use std::{borrow::Cow, num::NonZeroUsize, sync::Arc};

    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        AccountId, ChainId, DomainId, Level,
        isi::Log,
        query::{QueryRequest, SingularQueryBox, dsl::CompoundPredicate, prelude::FindParameters},
        transaction::TransactionBuilder,
    };
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, gen_account_in};
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        block::*,
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::{Execute, ValidQuery},
        state::{State, StateReadOnly, World, WorldReadOnly},
        sumeragi::network_topology::Topology,
        tx::AcceptedTransaction,
    };

    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("query fixture key generation should succeed")
    }

    fn checked_keypair_with_algorithm(algorithm: Algorithm) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm)
            .expect("query algorithm-specific fixture key generation should succeed")
    }

    fn find_transactions_request_with_filter(
        params: QueryParams,
        filter: CompoundPredicate<CommittedTransaction>,
    ) -> QueryRequest {
        let payload = norito::codec::Encode::encode(
            &iroha_data_model::query::transaction::prelude::FindTransactions,
        );
        let qbox: QueryBox<_> = Box::new(iroha_data_model::query::ErasedIterQuery::<
            CommittedTransaction,
        >::new(filter, SelectorTuple::default(), payload));
        #[cfg(feature = "fast_dsl")]
        let query = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        #[cfg(not(feature = "fast_dsl"))]
        let query = iroha_data_model::query::QueryWithParams::new(qbox, params);
        QueryRequest::Start(query)
    }

    fn find_transactions_request(params: QueryParams) -> QueryRequest {
        find_transactions_request_with_filter(params, CompoundPredicate::PASS)
    }

    fn transactions_from_batch(batch: QueryOutputBatchBoxTuple) -> Vec<CommittedTransaction> {
        match batch.into_iter().next().expect("transaction batch") {
            QueryOutputBatchBox::CommittedTransaction(transactions) => transactions,
            other => panic!("unexpected transaction batch: {other:?}"),
        }
    }

    #[test]
    fn checked_keypair_helpers_preserve_requested_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
        assert_eq!(
            checked_keypair_with_algorithm(Algorithm::Ed25519).algorithm(),
            Algorithm::Ed25519
        );
        #[cfg(feature = "bls")]
        assert_eq!(
            checked_keypair_with_algorithm(Algorithm::BlsNormal).algorithm(),
            Algorithm::BlsNormal
        );
    }

    fn dummy_accepted_transaction() -> AcceptedTransaction<'static> {
        let chain_id: ChainId = "00000000-0000-0000-0000-000000000000"
            .parse()
            .expect("valid chain id");
        let keypair = checked_keypair_with_algorithm(Algorithm::Ed25519);
        let authority = AccountId::new(keypair.public_key().clone());
        let mut builder = TransactionBuilder::new(chain_id, authority);
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions([Log::new(Level::INFO, "dummy".to_owned())])
            .sign(keypair.private_key());
        AcceptedTransaction::new_unchecked(Cow::Owned(tx))
    }

    #[tokio::test]
    async fn validate_for_client_world_parts_matches_state_view_path() {
        let state = State::new_for_testing(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let limits = QueryLimits::default();

        ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Singular(SingularQueryBox::FindParameters(FindParameters)),
            &ALICE_ID,
            &state.view(),
            limits,
        )
        .expect("state-view validation should pass");

        let world = state.world_view();
        let latest_block = state.latest_block_header_fast();
        ValidQueryRequest::validate_for_client_world_parts(
            QueryRequest::Singular(SingularQueryBox::FindParameters(FindParameters)),
            &ALICE_ID,
            &world,
            latest_block,
            limits,
        )
        .expect("world validation should pass");
    }

    #[tokio::test]
    async fn sorting_by_metadata_key_and_fetch_size() {
        use iroha_data_model::{
            domain::Domain,
            query::parameters::{FetchSize, Pagination, QueryParams, Sorting},
        };
        use nonzero_ext::nonzero;

        // Build sample domains with a sortable metadata key "rank"
        let mut d1 = Domain::new(DomainId::try_new("d1", "universal").unwrap()).build(&ALICE_ID);
        let mut d2 = Domain::new(DomainId::try_new("d2", "universal").unwrap()).build(&ALICE_ID);
        let d3 = Domain::new(DomainId::try_new("d3", "universal").unwrap()).build(&ALICE_ID); // no rank
        d1.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
        d2.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));

        let iter = vec![d1.clone(), d2.clone(), d3.clone()].into_iter();

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(iroha_data_model::query::parameters::SortOrder::Asc),
            },
            fetch_size: FetchSize {
                fetch_size: Some(nonzero!(2_u64)),
            },
        };

        let selector = SelectorTuple::default();
        let mut erased =
            apply_query_postprocessing(iter, selector, &params, QueryLimits::default()).unwrap();

        // First batch should be [d2(rank=1), d1(rank=2)]
        let (batch, next) = erased.next_batch(0).expect("first batch");
        let mut tuple_iter = batch.into_iter();
        let v = match tuple_iter.next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].id, d2.id);
        assert_eq!(v[1].id, d1.id);
        assert!(next.is_some());

        // Second batch should be [d3] (no rank -> sorted last)
        let (batch2, next2) = erased
            .next_batch(next.unwrap().get())
            .expect("second batch");
        let mut tuple_iter2 = batch2.into_iter();
        let v2 = match tuple_iter2.next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v2.len(), 1);
        assert_eq!(v2[0].id, d3.id);
        assert!(next2.is_none());
    }

    #[tokio::test]
    async fn sorting_descending_and_fetch_size() {
        use iroha_data_model::{
            domain::Domain,
            query::parameters::{FetchSize, Pagination, QueryParams, SortOrder, Sorting},
        };
        use nonzero_ext::nonzero;

        // Domains with rank metadata
        let mut d1 = Domain::new(DomainId::try_new("d1", "universal").unwrap()).build(&ALICE_ID);
        let mut d2 = Domain::new(DomainId::try_new("d2", "universal").unwrap()).build(&ALICE_ID);
        let d3 = Domain::new(DomainId::try_new("d3", "universal").unwrap()).build(&ALICE_ID); // no rank
        d1.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
        d2.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));

        let iter = vec![d1.clone(), d2.clone(), d3.clone()].into_iter();

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(SortOrder::Desc),
            },
            fetch_size: FetchSize {
                fetch_size: Some(nonzero!(2_u64)),
            },
        };

        let selector = SelectorTuple::default();
        let mut erased =
            apply_query_postprocessing(iter, selector, &params, QueryLimits::default()).unwrap();

        // First batch should be [d1(rank=2), d2(rank=1)] for descending
        let (batch, next) = erased.next_batch(0).expect("first batch");
        let mut tuple_iter = batch.into_iter();
        let v = match tuple_iter.next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].id, d1.id);
        assert_eq!(v[1].id, d2.id);
        assert!(next.is_some());

        // Second batch should be [d3]
        let (batch2, next2) = erased
            .next_batch(next.unwrap().get())
            .expect("second batch");
        let mut tuple_iter2 = batch2.into_iter();
        let v2 = match tuple_iter2.next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v2.len(), 1);
        assert_eq!(v2[0].id, d3.id);
        assert!(next2.is_none());
    }

    #[tokio::test]
    async fn ephemeral_sorted_query_respects_offset_and_limit() {
        use iroha_data_model::{
            domain::Domain,
            query::parameters::{FetchSize, Pagination, QueryParams, Sorting},
        };
        use nonzero_ext::nonzero;

        let mut d1 = Domain::new(DomainId::try_new("d1", "universal").unwrap()).build(&ALICE_ID);
        let mut d2 = Domain::new(DomainId::try_new("d2", "universal").unwrap()).build(&ALICE_ID);
        let mut d3 = Domain::new(DomainId::try_new("d3", "universal").unwrap()).build(&ALICE_ID);
        let d4 = Domain::new(DomainId::try_new("d4", "universal").unwrap()).build(&ALICE_ID);
        d1.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
        d2.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
        d3.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(3)));

        let params = QueryParams {
            pagination: Pagination {
                offset: 1,
                limit: Some(nonzero!(2_u64)),
            },
            sorting: Sorting::by_metadata_key("rank".parse().unwrap()),
            fetch_size: FetchSize {
                fetch_size: Some(nonzero!(2_u64)),
            },
        };

        let selector = SelectorTuple::<Domain>::default();
        let (output, _processed_items) = apply_query_postprocessing_ephemeral_with_budget(
            vec![d4, d3.clone(), d1, d2.clone()].into_iter(),
            selector,
            &params,
            QueryLimits::default(),
            None,
        )
        .expect("postprocess");

        let (batch, remaining, cursor) = output.into_parts();
        assert!(cursor.is_none());
        assert_eq!(remaining, 0);
        let mut tuple_iter = batch.into_iter();
        let v = match tuple_iter.next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].id, d2.id);
        assert_eq!(v[1].id, d3.id);
    }

    fn domain_with_query_payload(name: &str, payload_bytes: usize, rank: u64) -> Domain {
        let mut domain =
            Domain::new(DomainId::try_new(name, "universal").expect("domain id")).build(&ALICE_ID);
        domain.metadata_mut().insert(
            "payload".parse().expect("metadata key"),
            Json::new("x".repeat(payload_bytes)),
        );
        domain
            .metadata_mut()
            .insert("rank".parse().expect("metadata key"), Json::new(rank));
        domain
    }

    #[test]
    fn query_budget_enforces_the_shared_item_and_byte_limit() {
        let value = Permission::new("query_budget".to_owned(), Json::new("small"));
        let bytes = bounded_bare_encoded_len(&value, u64::MAX).expect("measure permission");
        let budget = QueryExecutionBudget::from_weighted_limit(bytes, bytes, 1);
        assert_eq!(budget.max_items(), 1);
        assert_eq!(budget.max_bytes(), bytes);

        let error = QueryExecutionStats::default()
            .record_item(&value, Some(budget))
            .expect_err("item and byte work must share one budget");
        assert!(matches!(error, Error::GasBudgetExceeded));
    }

    fn root_account_alias(label: &str) -> iroha_data_model::account::AccountAlias {
        iroha_data_model::account::AccountAlias::domainless(
            label.parse().expect("alias label"),
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        )
    }

    #[test]
    fn singular_alias_preflight_uses_the_index_and_rejects_large_account_before_clone() {
        let alias = root_account_alias("budgeted-alias");
        let account_id = ALICE_ID.clone();
        let mut metadata = iroha_data_model::metadata::Metadata::default();
        metadata.insert(
            "oversized".parse().expect("metadata key"),
            Json::new("x".repeat(128 * 1024)),
        );
        let account = Account::new(account_id.clone())
            .with_label(Some(alias.clone()))
            .with_metadata(metadata)
            .build(&account_id);
        let mut world = World::with([], [account], []);
        world
            .account_aliases
            .insert(alias.clone(), account_id.clone());
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let query = SingularQueryBox::FindAccountByAlias(
            iroha_data_model::query::account::prelude::FindAccountByAlias { alias },
        );
        let budget = QueryExecutionBudget::from_weighted_limit(512, 0, 1);

        let error = preflight_singular_source_materialization(&query, &state.view(), Some(budget))
            .expect_err("large indexed account must fail before materialization");
        assert!(matches!(error, Error::GasBudgetExceeded));
    }

    #[test]
    fn singular_alias_preflight_never_falls_back_to_a_world_scan() {
        let alias = root_account_alias("unindexed-alias");
        let account_id = ALICE_ID.clone();
        let account = Account::new(account_id.clone())
            .with_label(Some(alias.clone()))
            .build(&account_id);
        let state = State::new_for_testing(
            World::with([], [account], []),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let query = SingularQueryBox::FindAccountByAlias(
            iroha_data_model::query::account::prelude::FindAccountByAlias { alias },
        );

        let error = preflight_singular_source_materialization(
            &query,
            &state.view(),
            Some(QueryExecutionBudget::from_weighted_limit(1_000_000, 1, 1)),
        )
        .expect_err("unindexed aliases must fail closed without scanning accounts");
        assert!(matches!(error, Error::Conversion(_)));
    }

    #[test]
    fn singular_manifest_preflight_rejects_large_record_before_clone() {
        let code_hash = Hash::new(b"large-manifest");
        let manifest = iroha_data_model::smart_contract::manifest::ContractManifest {
            seiyaku_name: Some("x".repeat(128 * 1024)),
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            error_codes: None,
            kotoba: None,
            provenance: None,
        };
        let mut world = World::default();
        world.contract_manifests.insert(code_hash, manifest);
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let query = SingularQueryBox::FindContractManifestByCodeHash(
            iroha_data_model::query::smart_contract::prelude::FindContractManifestByCodeHash {
                code_hash,
            },
        );

        let error = preflight_singular_source_materialization(
            &query,
            &state.view(),
            Some(QueryExecutionBudget::from_weighted_limit(512, 0, 1)),
        )
        .expect_err("large manifest must fail before materialization");
        assert!(matches!(error, Error::GasBudgetExceeded));
    }

    #[test]
    fn singular_preflight_work_is_charged_and_exact_budget_passes() {
        let code_hash = Hash::new(b"metered-manifest");
        let manifest = iroha_data_model::smart_contract::manifest::ContractManifest {
            seiyaku_name: Some("metered".repeat(32)),
            code_hash: Some(code_hash),
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            error_codes: None,
            kotoba: None,
            provenance: None,
        };
        let mut world = World::default();
        world.contract_manifests.insert(code_hash, manifest.clone());
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let query = SingularQueryBox::FindContractManifestByCodeHash(
            iroha_data_model::query::smart_contract::prelude::FindContractManifestByCodeHash {
                code_hash,
            },
        );
        let output = SingularQueryOutputBox::ContractManifest(manifest.clone());
        let response = QueryResponse::Singular(output.clone());
        let source_bytes =
            bounded_bare_encoded_len(&manifest, u64::MAX).expect("measure borrowed manifest");
        let output_bytes =
            bounded_bare_encoded_len(&output, u64::MAX).expect("measure generic output");
        let response_bytes =
            bounded_framed_encoded_len(&response, u64::MAX).expect("measure framed response");
        let exact_units = 1_u64
            .saturating_add(source_bytes)
            .saturating_add(output_bytes)
            .saturating_add(response_bytes);

        let tight_budget = QueryExecutionBudget::from_weighted_limit(
            exact_units.saturating_sub(source_bytes).saturating_sub(1),
            1,
            1,
        );
        let measured =
            preflight_singular_source_materialization(&query, &state.view(), Some(tight_budget))
                .expect("borrowed source fits the initial preflight window");
        assert_eq!(measured, source_bytes);
        let mut tight_stats = QueryExecutionStats::default();
        tight_stats
            .record_preflighted_item(measured, Some(tight_budget))
            .expect("item and preflight source fit before owned materialization");
        tight_stats
            .record_value_bytes(&output, Some(tight_budget))
            .expect("generic output still fits");
        assert!(matches!(
            tight_stats.record_response(&response, Some(tight_budget)),
            Err(Error::GasBudgetExceeded)
        ));

        let exact_budget = QueryExecutionBudget::from_weighted_limit(exact_units, 1, 1);
        let measured =
            preflight_singular_source_materialization(&query, &state.view(), Some(exact_budget))
                .expect("exact budget admits borrowed source");
        let mut exact_stats = QueryExecutionStats::default();
        exact_stats
            .record_preflighted_item(measured, Some(exact_budget))
            .expect("charge borrowed source");
        exact_stats
            .record_value_bytes(&output, Some(exact_budget))
            .expect("charge generic output");
        exact_stats
            .record_response(&response, Some(exact_budget))
            .expect("charge framed response");
        assert_eq!(exact_stats.processed_items(), 1);
        assert_eq!(exact_stats.processed_bytes(), exact_units - 1);
    }

    #[test]
    fn ephemeral_offset_charges_bytes_for_skipped_values() {
        let oversized = domain_with_query_payload("oversized", 128 * 1024, 0);
        let retained = domain_with_query_payload("retained", 8, 1);
        let oversized_bytes =
            bounded_bare_encoded_len(&oversized, u64::MAX).expect("measure oversized domain");
        let params = QueryParams {
            pagination: Pagination::new(None, 1),
            fetch_size: FetchSize::new(Some(nonzero!(1_u64))),
            ..QueryParams::default()
        };
        let budget =
            QueryExecutionBudget::from_weighted_limit(oversized_bytes.saturating_sub(1), 0, 1);

        let error = apply_query_postprocessing_ephemeral_with_budget(
            vec![oversized, retained].into_iter(),
            SelectorTuple::default(),
            &params,
            QueryLimits::new(2),
            Some(budget),
        )
        .expect_err("an oversized skipped value must exhaust the byte budget");
        assert!(matches!(error, Error::GasBudgetExceeded));
    }

    #[test]
    fn ephemeral_sort_rejects_oversized_values_before_retaining_them() {
        let oversized = domain_with_query_payload("oversized-sort", 128 * 1024, 2);
        let small = domain_with_query_payload("small-sort", 8, 1);
        let oversized_bytes =
            bounded_bare_encoded_len(&oversized, u64::MAX).expect("measure oversized domain");
        let params = QueryParams {
            sorting: Sorting::by_metadata_key("rank".parse().expect("sort key")),
            fetch_size: FetchSize::new(Some(nonzero!(1_u64))),
            ..QueryParams::default()
        };
        let budget = QueryExecutionBudget::from_weighted_limit(oversized_bytes, 1, 1);

        let error = apply_query_postprocessing_ephemeral_with_budget(
            vec![oversized, small].into_iter(),
            SelectorTuple::default(),
            &params,
            QueryLimits::new(2),
            Some(budget),
        )
        .expect_err("sorting must meter a value before inserting it into the heap");
        assert!(matches!(error, Error::GasBudgetExceeded));
    }

    #[derive(norito::derive::NoritoSerialize)]
    struct CountingTiebreakValue {
        id: u8,
    }

    static TIEBREAK_DERIVATIONS: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);

    impl SortableQueryOutput for CountingTiebreakValue {
        type TiebreakKey = Vec<u8>;

        fn get_metadata_sorting_key(&self, _key: &Name) -> Option<&Json> {
            None
        }

        fn tiebreak_key(&self) -> Self::TiebreakKey {
            TIEBREAK_DERIVATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            vec![self.id; 16 * 1024]
        }

        fn bounded_tiebreak_key_len(&self, limit: u64) -> Result<u64, Error> {
            let encoded = 8_u64.saturating_add(16 * 1024);
            (encoded <= limit)
                .then_some(encoded)
                .ok_or(Error::GasBudgetExceeded)
        }
    }

    #[test]
    fn ephemeral_sort_derives_and_charges_each_large_tiebreak_key_once() {
        TIEBREAK_DERIVATIONS.store(0, std::sync::atomic::Ordering::Relaxed);
        let mut stats = QueryExecutionStats::default();
        let values = (0_u8..32).map(|id| CountingTiebreakValue { id });
        let budget = QueryExecutionBudget::from_weighted_limit(2_000_000, 1, 1);

        let (sorted, count) = collect_ephemeral_sorted_prefix(
            values,
            &"unused".parse().expect("sort key"),
            SortOrder::Asc,
            16,
            Some(budget),
            &mut stats,
        )
        .expect("sort values with precomputed keys");

        assert_eq!(count, 32);
        assert_eq!(sorted.len(), 16);
        assert_eq!(
            TIEBREAK_DERIVATIONS.load(std::sync::atomic::Ordering::Relaxed),
            32,
            "sorting comparisons must reuse the one admitted key per item",
        );
        assert!(stats.processed_bytes() >= 32 * 16 * 1024);
    }

    #[test]
    fn ephemeral_sort_rejects_large_tiebreak_key_before_materialization() {
        TIEBREAK_DERIVATIONS.store(0, std::sync::atomic::Ordering::Relaxed);
        let value = CountingTiebreakValue { id: 7 };
        let item_bytes = bounded_bare_encoded_len(&value, u64::MAX).expect("measure item");
        let key_bytes = value
            .bounded_tiebreak_key_len(u64::MAX)
            .expect("measure tiebreak key");
        let budget = QueryExecutionBudget::from_weighted_limit(
            item_bytes.saturating_add(key_bytes).saturating_sub(1),
            0,
            1,
        );
        let mut stats = QueryExecutionStats::default();
        stats
            .record_item(&value, Some(budget))
            .expect("item fits before its large key");

        let error = materialize_admitted_tiebreak_key(&value, &mut stats, Some(budget))
            .expect_err("oversized key must fail its allocation-free preflight");
        assert!(matches!(error, Error::GasBudgetExceeded));
        assert_eq!(
            TIEBREAK_DERIVATIONS.load(std::sync::atomic::Ordering::Relaxed),
            0,
            "rejected keys must never be constructed",
        );
    }

    #[test]
    fn singular_response_is_measured_before_host_serialization() {
        let domain = domain_with_query_payload("singular-budget", 64 * 1024, 0);
        let output = SingularQueryOutputBox::Domain(domain);
        let response = QueryResponse::Singular(output.clone());
        let item_bytes =
            bounded_bare_encoded_len(&output, u64::MAX).expect("measure singular output");
        let response_bytes =
            bounded_framed_encoded_len(&response, u64::MAX).expect("measure query response");
        assert_eq!(
            response_bytes,
            u64::try_from(
                norito::to_bytes(&response)
                    .expect("encode query response")
                    .len()
            )
            .expect("response length fits u64"),
            "manual Norito framing measurement must match the canonical codec",
        );
        let budget = QueryExecutionBudget::from_weighted_limit(
            item_bytes.saturating_add(response_bytes).saturating_sub(1),
            0,
            1,
        );
        let mut stats = QueryExecutionStats::default();
        stats
            .record_item(&output, Some(budget))
            .expect("singular item alone fits");

        let error = stats
            .record_response(&response, Some(budget))
            .expect_err("framed response must be admitted before serialization");
        assert!(matches!(error, Error::GasBudgetExceeded));
    }

    #[test]
    fn query_budget_allows_a_legitimate_offset_page() {
        let skipped = domain_with_query_payload("skip-ok", 16, 0);
        let first = domain_with_query_payload("first-ok", 16, 1);
        let second = domain_with_query_payload("second-ok", 16, 2);
        let byte_budget = [&skipped, &first, &second]
            .into_iter()
            .map(|value| bounded_bare_encoded_len(value, u64::MAX).expect("measure domain"))
            .fold(0_u64, u64::saturating_add);
        let params = QueryParams {
            pagination: Pagination::new(Some(nonzero!(2_u64)), 1),
            fetch_size: FetchSize::new(Some(nonzero!(2_u64))),
            ..QueryParams::default()
        };
        let budget = QueryExecutionBudget::from_weighted_limit(byte_budget.saturating_add(2), 1, 1);

        let (output, stats) = apply_query_postprocessing_ephemeral_with_budget(
            vec![skipped, first.clone(), second.clone()].into_iter(),
            SelectorTuple::default(),
            &params,
            QueryLimits::new(3),
            Some(budget),
        )
        .expect("legitimate offset page should fit its exact budget");
        let iterable_response = QueryResponse::Iterable(output.clone());
        assert_eq!(
            bounded_framed_encoded_len(&iterable_response, u64::MAX)
                .expect("measure iterable response"),
            u64::try_from(
                norito::to_bytes(&iterable_response)
                    .expect("encode iterable response")
                    .len(),
            )
            .expect("response length fits u64"),
            "iterable framing measurement must match the canonical codec",
        );
        assert_eq!(stats.processed_items(), 2);
        assert_eq!(stats.processed_bytes(), byte_budget);
        assert_eq!(
            domain_ids_from_batch(output.batch),
            vec![first.id, second.id]
        );
    }

    #[tokio::test]
    async fn ephemeral_unsorted_query_returns_first_batch_and_remaining_without_cursor() {
        use iroha_data_model::{
            domain::Domain,
            query::parameters::{FetchSize, Pagination, QueryParams, Sorting},
        };
        use nonzero_ext::nonzero;

        let d1 = Domain::new(DomainId::try_new("d1", "universal").unwrap()).build(&ALICE_ID);
        let d2 = Domain::new(DomainId::try_new("d2", "universal").unwrap()).build(&ALICE_ID);
        let d3 = Domain::new(DomainId::try_new("d3", "universal").unwrap()).build(&ALICE_ID);

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize {
                fetch_size: Some(nonzero!(2_u64)),
            },
        };
        let selector = SelectorTuple::<Domain>::default();

        let (output, stats) = apply_query_postprocessing_ephemeral_with_budget(
            vec![d1.clone(), d2.clone(), d3].into_iter(),
            selector,
            &params,
            QueryLimits::default(),
            None,
        )
        .expect("postprocess");

        let (batch, remaining, cursor) = output.into_parts();
        assert!(cursor.is_none());
        assert_eq!(remaining, 1);
        assert_eq!(stats.processed_items(), 3);
        assert_eq!(domain_ids_from_batch(batch), vec![d1.id, d2.id]);
    }

    #[tokio::test]
    async fn ephemeral_unsorted_bounded_count_stops_after_probe_item() {
        use iroha_data_model::{
            domain::Domain,
            query::parameters::{FetchSize, Pagination, QueryParams, Sorting},
        };
        use nonzero_ext::nonzero;

        let d1 = Domain::new(DomainId::try_new("d1", "universal").unwrap()).build(&ALICE_ID);
        let d2 = Domain::new(DomainId::try_new("d2", "universal").unwrap()).build(&ALICE_ID);
        let d3 = Domain::new(DomainId::try_new("d3", "universal").unwrap()).build(&ALICE_ID);
        let d4 = Domain::new(DomainId::try_new("d4", "universal").unwrap()).build(&ALICE_ID);

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize {
                fetch_size: Some(nonzero!(2_u64)),
            },
        };
        let selector = SelectorTuple::<Domain>::default();

        let (output, stats) = apply_query_postprocessing_ephemeral_with_budget(
            vec![d1.clone(), d2.clone(), d3, d4].into_iter(),
            selector,
            &params,
            QueryLimits::default().with_count_mode(QueryCountMode::Bounded),
            None,
        )
        .expect("postprocess");

        assert_eq!(output.remaining_items, None);
        assert!(output.has_more);
        let (batch, remaining_hint, cursor) = output.into_parts();
        assert!(cursor.is_none());
        assert_eq!(remaining_hint, 0);
        assert_eq!(stats.processed_items(), 3);
        assert_eq!(domain_ids_from_batch(batch), vec![d1.id, d2.id]);
    }

    #[tokio::test]
    async fn stored_unsorted_bounded_cursor_materializes_owned_tail_without_exact_count() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        use iroha_data_model::{
            domain::Domain,
            query::parameters::{FetchSize, Pagination, QueryParams, Sorting},
        };
        use nonzero_ext::nonzero;

        let domains = ["d1", "d2", "d3", "d4", "d5"]
            .into_iter()
            .map(|name| Domain::new(DomainId::try_new(name, "universal").unwrap()).build(&ALICE_ID))
            .collect::<Vec<_>>();
        let expected_ids = domains
            .iter()
            .map(|domain| domain.id.clone())
            .collect::<Vec<_>>();
        let visited = Arc::new(AtomicUsize::new(0));
        let visited_for_iter = Arc::clone(&visited);
        let iter = domains.into_iter().inspect(move |_| {
            visited_for_iter.fetch_add(1, Ordering::SeqCst);
        });

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(nonzero!(2_u64))),
        };
        let prepared = prepare_stored_unsorted_bounded_start(
            iter,
            SelectorTuple::<Domain>::default(),
            &params,
            QueryLimits::default().with_count_mode(QueryCountMode::Bounded),
        )
        .expect("bounded prepared start");

        assert_eq!(
            visited.load(Ordering::SeqCst),
            expected_ids.len(),
            "stored bounded start must own the continuation tail before inserting it into the shared live query store"
        );
        assert_eq!(prepared.remaining_items, None);
        assert!(prepared.deferred_continuation.is_some());

        let handle = LiveQueryStore::start_test();
        let first = handle
            .handle_iter_start_prepared(prepared, &ALICE_ID, None)
            .expect("store prepared");
        assert_eq!(first.remaining_items, None);
        assert!(first.has_more);
        let (first_batch, first_remaining_hint, cursor) = first.into_parts();
        assert_eq!(first_remaining_hint, 0);
        assert_eq!(
            domain_ids_from_batch(first_batch),
            expected_ids[0..2].to_vec()
        );

        let second = handle
            .handle_iter_continue(cursor.expect("first cursor"), &ALICE_ID)
            .expect("second page");
        assert_eq!(second.remaining_items, None);
        assert!(second.has_more);
        let (second_batch, _, cursor) = second.into_parts();
        assert_eq!(
            domain_ids_from_batch(second_batch),
            expected_ids[2..4].to_vec()
        );

        let third = handle
            .handle_iter_continue(cursor.expect("second cursor"), &ALICE_ID)
            .expect("third page");
        assert_eq!(third.remaining_items, None);
        assert!(!third.has_more);
        let (third_batch, _, cursor) = third.into_parts();
        assert!(cursor.is_none());
        assert_eq!(
            domain_ids_from_batch(third_batch),
            expected_ids[4..5].to_vec()
        );
        assert_eq!(
            visited.load(Ordering::SeqCst),
            expected_ids.len(),
            "continuations should not revisit the source iterator"
        );
    }

    #[tokio::test]
    async fn stored_unsorted_bounded_replay_cursor_does_not_materialize_tail_on_start() {
        use std::sync::{
            Arc, Weak,
            atomic::{AtomicUsize, Ordering},
        };

        use iroha_data_model::{
            domain::Domain,
            query::{
                domain::prelude::FindDomains,
                dsl::CompoundPredicate,
                parameters::{FetchSize, Pagination, QueryParams, Sorting},
            },
        };
        use nonzero_ext::nonzero;

        let domains = ["d1", "d2", "d3", "d4", "d5"]
            .into_iter()
            .map(|name| Domain::new(DomainId::try_new(name, "universal").unwrap()).build(&ALICE_ID))
            .collect::<Vec<_>>();
        let expected_ids = domains
            .iter()
            .map(|domain| domain.id.clone())
            .collect::<Vec<_>>();
        let visited = Arc::new(AtomicUsize::new(0));
        let visited_for_iter = Arc::clone(&visited);
        let iter = domains.into_iter().inspect(move |_| {
            visited_for_iter.fetch_add(1, Ordering::SeqCst);
        });

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(nonzero!(2_u64))),
        };
        let prepared = prepare_stored_unsorted_bounded_replay_start(
            iter,
            FindDomains,
            CompoundPredicate::PASS,
            SelectorTuple::<Domain>::default(),
            &params,
            QueryLimits::default().with_count_mode(QueryCountMode::Bounded),
            Weak::<State>::new(),
        )
        .expect("bounded replay prepared start");

        assert_eq!(
            visited.load(Ordering::SeqCst),
            3,
            "replay-backed bounded start should consume only the first batch plus a probe"
        );

        let handle = LiveQueryStore::start_test();
        let first = handle
            .handle_iter_start_paged_prepared(prepared, &ALICE_ID, None)
            .expect("store prepared");
        assert_eq!(first.remaining_items, None);
        assert!(first.has_more);
        let (first_batch, first_remaining_hint, cursor) = first.into_parts();
        assert_eq!(first_remaining_hint, 0);
        assert_eq!(
            domain_ids_from_batch(first_batch),
            expected_ids[0..2].to_vec()
        );
        assert_eq!(
            visited.load(Ordering::SeqCst),
            3,
            "storing the cursor must not force tail materialization"
        );

        let err = handle
            .handle_iter_continue(cursor.expect("first cursor"), &ALICE_ID)
            .expect_err("missing replay state expires the cursor");
        assert!(matches!(err, Error::Expired));
    }

    #[tokio::test]
    async fn stored_unsorted_bounded_replay_limit_boundary_does_not_probe_or_store_cursor() {
        use std::sync::{
            Arc, Weak,
            atomic::{AtomicUsize, Ordering},
        };

        use iroha_data_model::{
            domain::Domain,
            query::{
                domain::prelude::FindDomains,
                dsl::CompoundPredicate,
                parameters::{FetchSize, Pagination, QueryParams, Sorting},
            },
        };
        use nonzero_ext::nonzero;

        let domains = ["d1", "d2", "d3", "d4", "d5"]
            .into_iter()
            .map(|name| Domain::new(DomainId::try_new(name, "universal").unwrap()).build(&ALICE_ID))
            .collect::<Vec<_>>();
        let expected_ids = domains
            .iter()
            .map(|domain| domain.id.clone())
            .collect::<Vec<_>>();
        let visited = Arc::new(AtomicUsize::new(0));
        let visited_for_iter = Arc::clone(&visited);
        let iter = domains.into_iter().inspect(move |_| {
            visited_for_iter.fetch_add(1, Ordering::SeqCst);
        });

        let params = QueryParams {
            pagination: Pagination {
                limit: Some(nonzero!(2_u64)),
                offset: 1,
            },
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(nonzero!(5_u64))),
        };
        let prepared = prepare_stored_unsorted_bounded_replay_start(
            iter,
            FindDomains,
            CompoundPredicate::PASS,
            SelectorTuple::<Domain>::default(),
            &params,
            QueryLimits::default().with_count_mode(QueryCountMode::Bounded),
            Weak::<State>::new(),
        )
        .expect("bounded replay prepared start");

        assert_eq!(
            visited.load(Ordering::SeqCst),
            3,
            "explicit limit should consume offset plus the requested rows and skip the probe"
        );
        assert!(prepared.paged_continuation.is_none());

        let handle = LiveQueryStore::start_test();
        let first = handle
            .handle_iter_start_paged_prepared(prepared, &ALICE_ID, None)
            .expect("store prepared");
        assert_eq!(first.remaining_items, None);
        assert!(!first.has_more);
        let (first_batch, first_remaining_hint, cursor) = first.into_parts();
        assert_eq!(first_remaining_hint, 0);
        assert!(cursor.is_none());
        assert_eq!(
            domain_ids_from_batch(first_batch),
            expected_ids[1..3].to_vec()
        );
    }

    #[tokio::test]
    async fn collect_unsorted_bounded_page_rejects_returned_offset_at_limit_without_reading() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        use iroha_data_model::{
            domain::Domain,
            query::parameters::{FetchSize, Pagination, QueryParams, Sorting},
        };
        use nonzero_ext::nonzero;

        let visited = Arc::new(AtomicUsize::new(0));
        let visited_for_iter = Arc::clone(&visited);
        let iter = ["d1", "d2", "d3"].into_iter().map(move |name| {
            visited_for_iter.fetch_add(1, Ordering::SeqCst);
            Domain::new(DomainId::try_new(name, "universal").unwrap()).build(&ALICE_ID)
        });
        let params = QueryParams {
            pagination: Pagination {
                limit: Some(nonzero!(2_u64)),
                offset: 0,
            },
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(nonzero!(2_u64))),
        };

        let err = collect_unsorted_bounded_page(
            iter,
            SelectorTuple::<Domain>::default(),
            &params,
            QueryLimits::default().with_count_mode(QueryCountMode::Bounded),
            2,
        )
        .expect_err("cursor at the limit is done");

        assert!(matches!(err, Error::CursorDone));
        assert_eq!(
            visited.load(Ordering::SeqCst),
            0,
            "limit-exhausted continuations must fail before reading source rows"
        );
    }

    #[tokio::test]
    async fn collect_unsorted_bounded_page_rejects_oversized_fetch_without_reading() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        use iroha_data_model::{
            domain::Domain,
            query::parameters::{FetchSize, Pagination, QueryParams, Sorting},
        };
        use nonzero_ext::nonzero;

        let visited = Arc::new(AtomicUsize::new(0));
        let visited_for_iter = Arc::clone(&visited);
        let iter = ["d1", "d2", "d3"].into_iter().map(move |name| {
            visited_for_iter.fetch_add(1, Ordering::SeqCst);
            Domain::new(DomainId::try_new(name, "universal").unwrap()).build(&ALICE_ID)
        });
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(nonzero!(2_u64))),
        };

        let err = collect_unsorted_bounded_page(
            iter,
            SelectorTuple::<Domain>::default(),
            &params,
            QueryLimits::new(1).with_count_mode(QueryCountMode::Bounded),
            0,
        )
        .expect_err("oversized fetch should fail");

        assert!(matches!(err, Error::FetchSizeTooBig));
        assert_eq!(
            visited.load(Ordering::SeqCst),
            0,
            "fetch-size abuse must fail before reading source rows"
        );
    }

    fn domain_ids_from_batch(
        batch: iroha_data_model::query::QueryOutputBatchBoxTuple,
    ) -> Vec<DomainId> {
        let mut tuple_iter = batch.into_iter();
        match tuple_iter.next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Domain(v) => {
                v.into_iter().map(|domain| domain.id).collect()
            }
            other => panic!("unexpected batch variant: {other:?}"),
        }
    }

    fn sample_sorted_domains() -> Vec<Domain> {
        let mut d1 = Domain::new(DomainId::try_new("d1", "universal").unwrap()).build(&ALICE_ID);
        let mut d2 = Domain::new(DomainId::try_new("d2", "universal").unwrap()).build(&ALICE_ID);
        let mut d3 = Domain::new(DomainId::try_new("d3", "universal").unwrap()).build(&ALICE_ID);
        let mut d4 = Domain::new(DomainId::try_new("d4", "universal").unwrap()).build(&ALICE_ID);
        let mut d5 = Domain::new(DomainId::try_new("d5", "universal").unwrap()).build(&ALICE_ID);
        let d6 = Domain::new(DomainId::try_new("d6", "universal").unwrap()).build(&ALICE_ID);

        d1.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
        d2.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
        d3.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
        d4.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(3)));
        d5.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));

        vec![d6, d5, d4, d3, d2, d1]
    }

    #[tokio::test]
    async fn stored_sorted_fast_start_matches_legacy_first_batch_variants() {
        use iroha_data_model::query::parameters::{FetchSize, Pagination, QueryParams, Sorting};
        use std::num::NonZeroU64;

        let cases = [
            (0_u64, None, 1_u64),
            (0_u64, None, 2_u64),
            (1_u64, None, 2_u64),
            (1_u64, Some(3_u64), 2_u64),
            (2_u64, Some(3_u64), 1_u64),
            (3_u64, Some(2_u64), 2_u64),
        ];

        for (offset, limit, fetch_size) in cases {
            let params = QueryParams {
                pagination: Pagination {
                    offset,
                    limit: limit.and_then(NonZeroU64::new),
                },
                sorting: Sorting::by_metadata_key("rank".parse().unwrap()),
                fetch_size: FetchSize::new(Some(
                    NonZeroU64::new(fetch_size).expect("non-zero fetch size"),
                )),
            };
            let selector = SelectorTuple::<Domain>::default();

            let mut historical = apply_query_postprocessing(
                sample_sorted_domains().into_iter(),
                selector.clone(),
                &params,
                QueryLimits::default(),
            )
            .expect("historical path");
            let (legacy_first_batch, legacy_next) =
                historical.next_batch(0).expect("historical first");
            let legacy_remaining = historical.remaining();

            let fast = stored_sorted_fast_start_params(&params, QueryLimits::default())
                .expect("fast path selection")
                .expect("fast path should be available for this test");
            let prepared = prepare_stored_sorted_start(
                sample_sorted_domains().into_iter(),
                selector,
                fast,
                None,
            )
            .expect("prepared start");

            assert_eq!(
                domain_ids_from_batch(prepared.first_batch),
                domain_ids_from_batch(legacy_first_batch),
                "offset={offset} limit={limit:?} fetch={fetch_size}"
            );
            assert_eq!(
                prepared.remaining_items, legacy_remaining,
                "offset={offset} limit={limit:?} fetch={fetch_size}"
            );
            assert_eq!(
                prepared.deferred_continuation.is_some(),
                legacy_next.is_some(),
                "offset={offset} limit={limit:?} fetch={fetch_size}"
            );
        }
    }

    #[tokio::test]
    async fn deferred_stored_start_first_continue_preserves_global_order() {
        use iroha_data_model::query::parameters::{FetchSize, Pagination, QueryParams, Sorting};
        use nonzero_ext::nonzero;

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::by_metadata_key("rank".parse().unwrap()),
            fetch_size: FetchSize::new(Some(nonzero!(2_u64))),
        };
        let selector = SelectorTuple::<Domain>::default();

        let mut historical = apply_query_postprocessing(
            sample_sorted_domains().into_iter(),
            selector.clone(),
            &params,
            QueryLimits::default(),
        )
        .expect("historical path");
        let (_legacy_first, legacy_cursor) = historical.next_batch(0).expect("historical first");
        let expected_cursor = legacy_cursor.expect("historical continuation");
        let (legacy_second, _legacy_next) = historical
            .next_batch(expected_cursor.get())
            .expect("historical second");
        let expected_second_ids = domain_ids_from_batch(legacy_second);

        let fast = stored_sorted_fast_start_params(&params, QueryLimits::default())
            .expect("fast path selection")
            .expect("fast path should be available");
        let prepared =
            prepare_stored_sorted_start(sample_sorted_domains().into_iter(), selector, fast, None)
                .expect("prepared start");

        let handle = LiveQueryStore::start_test();
        let (_batch, _remaining, cursor) = handle
            .handle_iter_start_prepared(prepared, &ALICE_ID, None)
            .expect("store prepared")
            .into_parts();
        let cursor = cursor.expect("cursor");
        let next = handle
            .handle_iter_continue(cursor, &ALICE_ID)
            .expect("first continuation")
            .into_parts();

        assert_eq!(domain_ids_from_batch(next.0), expected_second_ids);
    }

    #[tokio::test]
    async fn validate_for_ivm_uses_validator() -> Result<()> {
        struct DummyValidator {
            authority: AccountId,
            validated: bool,
        }

        impl IvmQueryValidator for DummyValidator {
            fn authority(&self) -> &AccountId {
                &self.authority
            }

            fn validate_query(
                &mut self,
                authority: &AccountId,
                _query: &QueryRequest,
            ) -> Result<(), ValidationFail> {
                assert_eq!(authority, &self.authority);
                self.validated = true;
                Ok(())
            }
        }

        let mut validator = DummyValidator {
            authority: ALICE_ID.clone(),
            validated: false,
        };
        let query = QueryRequest::Singular(FindParameters.into());

        ValidQueryRequest::validate_for_ivm(query, &mut validator, QueryLimits::default())?;

        assert!(validator.validated);

        Ok(())
    }

    #[tokio::test]
    async fn validate_for_ivm_rejects_continue() {
        use iroha_data_model::query::parameters::ForwardCursor;

        struct DummyValidator {
            authority: AccountId,
        }

        impl IvmQueryValidator for DummyValidator {
            fn authority(&self) -> &AccountId {
                &self.authority
            }

            fn validate_query(
                &mut self,
                _authority: &AccountId,
                _query: &QueryRequest,
            ) -> Result<(), ValidationFail> {
                Ok(())
            }
        }

        let mut validator = DummyValidator {
            authority: ALICE_ID.clone(),
        };
        let cursor = ForwardCursor {
            query: "ivm-cursor".to_string(),
            cursor: nonzero!(1_u64),
            gas_budget: None,
        };
        let request = QueryRequest::Continue(cursor);

        let err = match ValidQueryRequest::validate_for_ivm(
            request,
            &mut validator,
            QueryLimits::default(),
        ) {
            Ok(_) => panic!("IVM must reject query continuations"),
            Err(err) => err,
        };
        assert!(matches!(err, ValidationFail::NotPermitted(msg) if msg.contains("Continue")));
    }

    fn world_with_test_domains() -> World {
        let domain_id = DomainId::try_new("wonderland", "universal").expect("Valid");
        let domain = Domain::new(domain_id).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let asset_definition_id = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_definition = AssetDefinition::numeric(asset_definition_id).build(&ALICE_ID);
        World::with([domain], [account], [asset_definition])
    }

    #[cfg(feature = "bls")]
    fn bls_test_keypair() -> KeyPair {
        checked_keypair_with_algorithm(Algorithm::BlsNormal)
    }

    #[cfg(not(feature = "bls"))]
    fn bls_test_keypair() -> KeyPair {
        checked_keypair()
    }

    fn state_with_test_blocks_and_transactions(
        blocks: u64,
        valid_tx_per_block: usize,
        invalid_tx_per_block: usize,
    ) -> Result<State> {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world_with_test_domains(), kura.clone(), query_handle);
        {
            let (max_clock_drift, tx_limits) = {
                let state_view = state.world.view();
                let params = state_view.parameters();
                (params.sumeragi().max_clock_drift(), params.transaction())
            };
            let crypto_cfg = state.crypto();

            let valid_tx = {
                let ok_instruction = Log::new(iroha_logger::Level::INFO, "pass".into());
                let tx = TransactionBuilder::new(chain_id.clone(), ALICE_ID.clone())
                    .with_instructions([ok_instruction])
                    .sign(ALICE_KEYPAIR.private_key());
                AcceptedTransaction::accept(
                    tx,
                    &chain_id,
                    max_clock_drift,
                    tx_limits,
                    crypto_cfg.as_ref(),
                )?
            };
            let invalid_tx = {
                let fail_isi = Unregister::domain(DomainId::try_new("dummy", "universal").unwrap());
                let tx = TransactionBuilder::new(chain_id.clone(), ALICE_ID.clone())
                    .with_instructions([fail_isi.clone(), fail_isi])
                    .sign(ALICE_KEYPAIR.private_key());
                AcceptedTransaction::accept(
                    tx,
                    &chain_id,
                    max_clock_drift,
                    tx_limits,
                    crypto_cfg.as_ref(),
                )?
            };

            let mut transactions = vec![valid_tx; valid_tx_per_block];
            transactions.append(&mut vec![invalid_tx; invalid_tx_per_block]);

            let (peer_public_key, peer_private_key) = bls_test_keypair().into_parts();
            let peer_id = PeerId::new(peer_public_key);
            let topology = Topology::new(vec![peer_id]);
            let unverified_first_block = BlockBuilder::new(transactions.clone())
                .chain(0, state.view().latest_block().as_deref())
                .sign(&peer_private_key)
                .unpack(|_| {});
            let mut state_block = state.block(unverified_first_block.header());
            let first_block = unverified_first_block
                .validate_and_record_transactions(&mut state_block)
                .unpack(|_| {})
                .commit(&topology)
                .unpack(|_| {})
                .unwrap();

            let _events = state_block.apply(&first_block, topology.as_ref().to_owned());
            kura.store_block(first_block).expect("store first block");
            state_block.commit().unwrap();

            for _ in 1u64..blocks {
                let unverified_block = BlockBuilder::new(transactions.clone())
                    .chain(0, state.view().latest_block().as_deref())
                    .sign(&peer_private_key)
                    .unpack(|_| {});
                let mut state_block = state.block(unverified_block.header());

                let block = unverified_block
                    .validate_and_record_transactions(&mut state_block)
                    .unpack(|_| {})
                    .commit(&topology)
                    .unpack(|_| {})
                    .expect("Block is valid");

                let _events = state_block.apply(&block, topology.as_ref().to_owned());
                kura.store_block(block).expect("store block");
                state_block.commit().unwrap();
            }
        }

        Ok(state)
    }

    #[tokio::test]
    async fn iter_dispatch_sorts_and_paginates_end_to_end() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{FetchSize, Pagination, QueryParams, Sorting};
        use iroha_futures::supervisor::ShutdownSignal;
        use iroha_primitives::json::Json;

        // Build world with three domains and ALICE account
        let d1_id: DomainId = DomainId::try_new("d1", "universal").unwrap();
        let d2_id: DomainId = DomainId::try_new("d2", "universal").unwrap();
        let d3_id: DomainId = DomainId::try_new("d3", "universal").unwrap();
        let mut d1 = Domain::new(d1_id.clone()).build(&ALICE_ID);
        let mut d2 = Domain::new(d2_id.clone()).build(&ALICE_ID);
        let d3 = Domain::new(d3_id.clone()).build(&ALICE_ID);
        d1.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
        d2.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));

        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([d1.clone(), d2.clone(), d3.clone()], [account], []);

        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new_with_chain(world, kura, handle.clone(), ChainId::from("chain"));
        let state_view = state.view();

        // Build params: sort by metadata key asc, fetch_size=2
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::by_metadata_key("rank".parse().unwrap()),
            fetch_size: FetchSize::new(nonzero_ext::nonzero!(2_u64).into()),
        };

        // Build an erased iterable query for Domains and wrap with params
        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::domain::prelude::FindDomains);
        let qbox: iroha_data_model::query::QueryBox<_> =
            Box::new(iroha_data_model::query::ErasedIterQuery::<Domain>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<Domain>::PASS,
                SelectorTuple::<Domain>::default(),
                payload,
            ));
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, _remaining, cursor) = first.into_parts();
        let mut tuple_iter = batch.into_iter();
        let v = match tuple_iter.next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].id, d2_id);
        assert_eq!(v[1].id, d1_id);

        // Continue for the remaining item
        let cursor = cursor.expect("should continue");
        let next = handle.handle_iter_continue(cursor, &ALICE_ID).unwrap();
        let (batch2, _rem2, cur2) = next.into_parts();
        let mut tuple_iter2 = batch2.into_iter();
        let v2 = match tuple_iter2.next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v2.len(), 1);
        assert_eq!(v2[0].id, d3_id);
        assert!(cur2.is_none());
    }

    #[tokio::test]
    async fn iter_dispatch_erased_and_fastdsl_parity_for_domains() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::{
            QueryBox, QueryOutputBatchBox, QueryRequest, QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
            parameters::{QueryParams, SortOrder, Sorting},
        };
        use iroha_futures::supervisor::ShutdownSignal;

        fn make_world() -> World {
            let d1_id: DomainId = DomainId::try_new("d1", "universal").unwrap();
            let d2_id: DomainId = DomainId::try_new("d2", "universal").unwrap();
            let d3_id: DomainId = DomainId::try_new("d3", "universal").unwrap();

            let mut d1 = Domain::new(d1_id).build(&ALICE_ID);
            let mut d2 = Domain::new(d2_id).build(&ALICE_ID);
            let d3 = Domain::new(d3_id).build(&ALICE_ID);
            d1.metadata_mut()
                .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
            d2.metadata_mut()
                .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));

            let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
            World::with([d1, d2, d3], [account], [])
        }

        fn build_state(world: World) -> (State, crate::query::store::LiveQueryStoreHandle) {
            let kura = Kura::blank_kura_for_testing();
            let store = std::sync::Arc::new(LiveQueryStore::from_config(
                StoreCfg::default(),
                ShutdownSignal::new(),
            ));
            let handle = crate::query::store::LiveQueryStoreHandle::new(store);
            let state = State::new(world, kura, handle.clone());
            (state, handle)
        }

        let params = QueryParams {
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(SortOrder::Asc),
            },
            ..Default::default()
        };

        // Erased query path using a boxed iterator payload.
        let (state_boxed, handle_boxed) = build_state(make_world());
        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::domain::prelude::FindDomains);
        let qbox: QueryBox<_> = Box::new(iroha_data_model::query::ErasedIterQuery::<Domain>::new(
            CompoundPredicate::PASS,
            SelectorTuple::default(),
            payload,
        ));
        let boxed_qwp = QueryWithParams::new(&qbox, params.clone());
        let boxed_req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(boxed_qwp),
            &ALICE_ID,
            &state_boxed.view(),
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(boxed_output) = boxed_req
            .execute(&handle_boxed, &state_boxed.view(), &ALICE_ID)
            .unwrap()
        else {
            panic!("expected iterable");
        };
        let (boxed_batch, _boxed_remaining, _boxed_cursor) = boxed_output.into_parts();
        let mut boxed_iter = boxed_batch.into_iter();
        let boxed_domains = match boxed_iter.next().expect("slice") {
            QueryOutputBatchBox::Domain(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };

        // fast_dsl-style path with encoded predicate/selector and no boxed payload.
        let (state_fast, handle_fast) = build_state(make_world());
        let fast_qwp = QueryWithParams {
            query: (),
            query_payload: Vec::new(),
            item: iroha_data_model::query::QueryItemKind::Domain,
            predicate_bytes: norito::codec::Encode::encode(&CompoundPredicate::<Domain>::PASS),
            selector_bytes: norito::codec::Encode::encode(&SelectorTuple::<Domain>::default()),
            params,
        };
        let fast_req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(fast_qwp),
            &ALICE_ID,
            &state_fast.view(),
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(fast_output) = fast_req
            .execute(&handle_fast, &state_fast.view(), &ALICE_ID)
            .unwrap()
        else {
            panic!("expected iterable");
        };
        let (fast_batch, _fast_remaining, _fast_cursor) = fast_output.into_parts();
        let mut fast_iter = fast_batch.into_iter();
        let fast_domains = match fast_iter.next().expect("slice") {
            QueryOutputBatchBox::Domain(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };

        let boxed_ids: Vec<_> = boxed_domains.into_iter().map(|d| d.id).collect();
        let fast_ids: Vec<_> = fast_domains.into_iter().map(|d| d.id).collect();
        assert_eq!(boxed_ids, fast_ids);
    }

    #[tokio::test]
    async fn iter_dispatch_erased_and_fastdsl_parity_for_assets() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::{
            QueryBox, QueryOutputBatchBox, QueryRequest, QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
            parameters::QueryParams,
        };
        use iroha_futures::supervisor::ShutdownSignal;

        fn make_world() -> (World, AssetDefinitionId, AssetId) {
            let domain =
                iroha_data_model::domain::Domain::new(DomainId::try_new("w", "universal").unwrap())
                    .build(&ALICE_ID);
            let account =
                iroha_data_model::account::Account::new(ALICE_ID.clone()).build(&ALICE_ID);
            let ad_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("w", "universal").unwrap(),
                "rose".parse().unwrap(),
            );
            let ad = iroha_data_model::asset::definition::AssetDefinition::numeric(ad_id.clone())
                .build(&ALICE_ID);
            let asset_id = AssetId::new(ad_id.clone(), ALICE_ID.clone());
            let asset = iroha_data_model::asset::value::Asset::new(asset_id.clone(), 10_u32);

            let world = World::with_assets([domain], [account], [ad.clone()], [asset], []);

            (world, ad_id, asset_id)
        }

        fn build_state(world: World) -> (State, crate::query::store::LiveQueryStoreHandle) {
            let kura = Kura::blank_kura_for_testing();
            let store = std::sync::Arc::new(LiveQueryStore::from_config(
                StoreCfg::default(),
                ShutdownSignal::new(),
            ));
            let handle = crate::query::store::LiveQueryStoreHandle::new(store);
            let state = State::new(world, kura, handle.clone());
            (state, handle)
        }

        let params = QueryParams::default();

        // Erased query path
        let (world_boxed, ad_id, asset_id) = make_world();
        let (state_boxed, handle_boxed) = build_state(world_boxed);
        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::asset::prelude::FindAssets);
        let qbox: QueryBox<_> = Box::new(iroha_data_model::query::ErasedIterQuery::<
            iroha_data_model::asset::value::Asset,
        >::new(
            CompoundPredicate::PASS,
            SelectorTuple::<iroha_data_model::asset::value::Asset>::default(),
            payload,
        ));
        let boxed_qwp = QueryWithParams::new(&qbox, params.clone());
        let boxed_req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(boxed_qwp),
            &ALICE_ID,
            &state_boxed.view(),
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(boxed_output) = boxed_req
            .execute(&handle_boxed, &state_boxed.view(), &ALICE_ID)
            .unwrap()
        else {
            panic!("expected iterable");
        };
        let (boxed_batch, _boxed_remaining, _boxed_cursor) = boxed_output.into_parts();
        let mut boxed_iter = boxed_batch.into_iter();
        let boxed_assets = match boxed_iter.next().expect("slice") {
            QueryOutputBatchBox::Asset(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };

        // fast_dsl path
        let (world_fast, _ad_fast, _asset_fast) = make_world();
        let (state_fast, handle_fast) = build_state(world_fast);
        let predicate = CompoundPredicate::<iroha_data_model::asset::value::Asset>::PASS;
        let fast_qwp = QueryWithParams {
            query: (),
            query_payload: Vec::new(),
            item: iroha_data_model::query::QueryItemKind::Asset,
            predicate_bytes: norito::codec::Encode::encode(&predicate),
            selector_bytes: norito::codec::Encode::encode(&SelectorTuple::<
                iroha_data_model::asset::value::Asset,
            >::default()),
            params,
        };
        let fast_req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(fast_qwp),
            &ALICE_ID,
            &state_fast.view(),
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(fast_output) = fast_req
            .execute(&handle_fast, &state_fast.view(), &ALICE_ID)
            .unwrap()
        else {
            panic!("expected iterable");
        };
        let (fast_batch, _fast_remaining, _fast_cursor) = fast_output.into_parts();
        let mut fast_iter = fast_batch.into_iter();
        let fast_assets = match fast_iter.next().expect("slice") {
            QueryOutputBatchBox::Asset(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };

        let boxed_ids: Vec<_> = boxed_assets.into_iter().map(|a| a.id().clone()).collect();
        let fast_ids: Vec<_> = fast_assets.into_iter().map(|a| a.id().clone()).collect();
        assert_eq!(boxed_ids, fast_ids);
        assert_eq!(boxed_ids, vec![asset_id]);
        assert_eq!(ad_id, boxed_ids[0].definition().clone());
    }

    #[tokio::test]
    async fn iter_dispatch_erased_and_fastdsl_parity_for_nfts() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::{
            QueryBox, QueryOutputBatchBox, QueryRequest, QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
            parameters::QueryParams,
        };
        use iroha_futures::supervisor::ShutdownSignal;

        fn make_world() -> World {
            let domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
            let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
            let n1 =
                Nft::new("n1$w.universal".parse().unwrap(), Metadata::default()).build(&ALICE_ID);
            let n2 =
                Nft::new("n2$w.universal".parse().unwrap(), Metadata::default()).build(&ALICE_ID);
            World::with_assets([domain], [account], [], [], [n1, n2])
        }

        fn build_state(world: World) -> (State, crate::query::store::LiveQueryStoreHandle) {
            let kura = Kura::blank_kura_for_testing();
            let store = std::sync::Arc::new(LiveQueryStore::from_config(
                StoreCfg::default(),
                ShutdownSignal::new(),
            ));
            let handle = crate::query::store::LiveQueryStoreHandle::new(store);
            let state = State::new(world, kura, handle.clone());
            (state, handle)
        }

        let params = QueryParams::default();

        // Erased query path
        let (state_boxed, handle_boxed) = build_state(make_world());
        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::nft::prelude::FindNfts);
        let qbox: QueryBox<_> = Box::new(iroha_data_model::query::ErasedIterQuery::<
            iroha_data_model::nft::Nft,
        >::new(
            CompoundPredicate::PASS,
            SelectorTuple::<iroha_data_model::nft::Nft>::default(),
            payload,
        ));
        let boxed_qwp = QueryWithParams::new(&qbox, params.clone());
        let boxed_req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(boxed_qwp),
            &ALICE_ID,
            &state_boxed.view(),
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(boxed_output) = boxed_req
            .execute(&handle_boxed, &state_boxed.view(), &ALICE_ID)
            .unwrap()
        else {
            panic!("expected iterable");
        };
        let (boxed_batch, _boxed_remaining, _boxed_cursor) = boxed_output.into_parts();
        let mut boxed_iter = boxed_batch.into_iter();
        let boxed_nfts = match boxed_iter.next().expect("slice") {
            QueryOutputBatchBox::Nft(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };

        // fast_dsl path
        let (state_fast, handle_fast) = build_state(make_world());
        let fast_qwp = QueryWithParams {
            query: (),
            query_payload: Vec::new(),
            item: iroha_data_model::query::QueryItemKind::Nft,
            predicate_bytes: norito::codec::Encode::encode(
                &CompoundPredicate::<iroha_data_model::nft::Nft>::PASS,
            ),
            selector_bytes: norito::codec::Encode::encode(&SelectorTuple::<
                iroha_data_model::nft::Nft,
            >::default()),
            params,
        };
        let fast_req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(fast_qwp),
            &ALICE_ID,
            &state_fast.view(),
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(fast_output) = fast_req
            .execute(&handle_fast, &state_fast.view(), &ALICE_ID)
            .unwrap()
        else {
            panic!("expected iterable");
        };
        let (fast_batch, _fast_remaining, _fast_cursor) = fast_output.into_parts();
        let mut fast_iter = fast_batch.into_iter();
        let fast_nfts = match fast_iter.next().expect("slice") {
            QueryOutputBatchBox::Nft(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };

        let mut boxed_ids: Vec<_> = boxed_nfts.into_iter().map(|n| n.id().clone()).collect();
        let mut fast_ids: Vec<_> = fast_nfts.into_iter().map(|n| n.id().clone()).collect();
        boxed_ids.sort();
        fast_ids.sort();
        assert_eq!(boxed_ids, fast_ids);
    }

    #[tokio::test]
    async fn iter_dispatch_erased_and_fastdsl_parity_for_accounts() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::{
            QueryBox, QueryOutputBatchBox, QueryRequest, QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
            parameters::QueryParams,
        };
        use iroha_futures::supervisor::ShutdownSignal;

        fn make_world() -> World {
            let domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
            let alice = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
            let bob = Account::new(BOB_ID.clone()).build(&ALICE_ID);
            World::with([domain], [alice, bob], [])
        }

        fn build_state(world: World) -> (State, crate::query::store::LiveQueryStoreHandle) {
            let kura = Kura::blank_kura_for_testing();
            let store = std::sync::Arc::new(LiveQueryStore::from_config(
                StoreCfg::default(),
                ShutdownSignal::new(),
            ));
            let handle = crate::query::store::LiveQueryStoreHandle::new(store);
            let state = State::new(world, kura, handle.clone());
            (state, handle)
        }

        let params = QueryParams::default();

        // Erased query path
        let (state_boxed, handle_boxed) = build_state(make_world());
        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::account::prelude::FindAccounts);
        let qbox: QueryBox<_> = Box::new(iroha_data_model::query::ErasedIterQuery::<
            iroha_data_model::account::Account,
        >::new(
            CompoundPredicate::PASS,
            SelectorTuple::<iroha_data_model::account::Account>::default(),
            payload,
        ));
        let boxed_qwp = QueryWithParams::new(&qbox, params.clone());
        let boxed_req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(boxed_qwp),
            &ALICE_ID,
            &state_boxed.view(),
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(boxed_output) = boxed_req
            .execute(&handle_boxed, &state_boxed.view(), &ALICE_ID)
            .unwrap()
        else {
            panic!("expected iterable");
        };
        let (boxed_batch, _boxed_remaining, _boxed_cursor) = boxed_output.into_parts();
        let mut boxed_iter = boxed_batch.into_iter();
        let boxed_accounts = match boxed_iter.next().expect("slice") {
            QueryOutputBatchBox::Account(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };

        // fast_dsl path
        let (state_fast, handle_fast) = build_state(make_world());
        let fast_qwp = QueryWithParams {
            query: (),
            query_payload: Vec::new(),
            item: iroha_data_model::query::QueryItemKind::Account,
            predicate_bytes: norito::codec::Encode::encode(
                &CompoundPredicate::<iroha_data_model::account::Account>::PASS,
            ),
            selector_bytes: norito::codec::Encode::encode(&SelectorTuple::<
                iroha_data_model::account::Account,
            >::default()),
            params,
        };
        let fast_req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(fast_qwp),
            &ALICE_ID,
            &state_fast.view(),
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(fast_output) = fast_req
            .execute(&handle_fast, &state_fast.view(), &ALICE_ID)
            .unwrap()
        else {
            panic!("expected iterable");
        };
        let (fast_batch, _fast_remaining, _fast_cursor) = fast_output.into_parts();
        let mut fast_iter = fast_batch.into_iter();
        let fast_accounts = match fast_iter.next().expect("slice") {
            QueryOutputBatchBox::Account(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };

        let mut boxed_ids: Vec<_> = boxed_accounts.into_iter().map(|a| a.id().clone()).collect();
        let mut fast_ids: Vec<_> = fast_accounts.into_iter().map(|a| a.id().clone()).collect();
        boxed_ids.sort();
        fast_ids.sort();
        assert_eq!(boxed_ids, fast_ids);
        assert!(boxed_ids.contains(&ALICE_ID));
        assert!(boxed_ids.contains(&BOB_ID));
    }

    #[tokio::test]
    async fn iter_dispatch_erased_and_fastdsl_parity_for_block_headers() -> Result<()> {
        use iroha_data_model::query::{
            QueryBox, QueryOutputBatchBox, QueryRequest, QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
            parameters::QueryParams,
        };

        // Build a small chain with a few blocks.
        let state = state_with_test_blocks_and_transactions(3, 1, 0)?;
        let handle = LiveQueryStore::start_test();
        let state_view = state.view();

        let params = QueryParams::default();

        // Erased query path
        let payload = norito::codec::Encode::encode(
            &iroha_data_model::query::block::prelude::FindBlockHeaders,
        );
        let qbox: QueryBox<_> = Box::new(iroha_data_model::query::ErasedIterQuery::<
            iroha_data_model::block::BlockHeader,
        >::new(
            CompoundPredicate::PASS,
            SelectorTuple::<iroha_data_model::block::BlockHeader>::default(),
            payload,
        ));
        let boxed_qwp = QueryWithParams::new(&qbox, params.clone());
        let boxed_req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(boxed_qwp),
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )?;
        let QueryResponse::Iterable(boxed_output) =
            boxed_req.execute(&handle, &state_view, &ALICE_ID)?
        else {
            panic!("expected iterable");
        };
        let (boxed_batch, _boxed_remaining, _boxed_cursor) = boxed_output.into_parts();
        let mut boxed_iter = boxed_batch.into_iter();
        let boxed_headers = match boxed_iter.next().expect("slice") {
            QueryOutputBatchBox::BlockHeader(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };

        // fast_dsl path
        let fast_qwp = QueryWithParams {
            query: (),
            query_payload: Vec::new(),
            item: iroha_data_model::query::QueryItemKind::BlockHeader,
            predicate_bytes: norito::codec::Encode::encode(
                &CompoundPredicate::<iroha_data_model::block::BlockHeader>::PASS,
            ),
            selector_bytes: norito::codec::Encode::encode(&SelectorTuple::<
                iroha_data_model::block::BlockHeader,
            >::default()),
            params,
        };
        let fast_req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(fast_qwp),
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )?;
        let QueryResponse::Iterable(fast_output) =
            fast_req.execute(&handle, &state_view, &ALICE_ID)?
        else {
            panic!("expected iterable");
        };
        let (fast_batch, _fast_remaining, _fast_cursor) = fast_output.into_parts();
        let mut fast_iter = fast_batch.into_iter();
        let fast_headers = match fast_iter.next().expect("slice") {
            QueryOutputBatchBox::BlockHeader(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };

        let boxed_hashes: Vec<_> = boxed_headers
            .iter()
            .map(iroha_data_model::block::Header::hash)
            .collect();
        let fast_hashes: Vec<_> = fast_headers
            .iter()
            .map(iroha_data_model::block::Header::hash)
            .collect();
        assert_eq!(boxed_hashes, fast_hashes);
        Ok(())
    }

    #[tokio::test]
    async fn iter_dispatch_sorts_desc_end_to_end() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{
            FetchSize, Pagination, QueryParams, SortOrder, Sorting,
        };
        use iroha_futures::supervisor::ShutdownSignal;
        use iroha_primitives::json::Json;

        // Build world with three domains and ALICE account
        let d1_id: DomainId = DomainId::try_new("d1", "universal").unwrap();
        let d2_id: DomainId = DomainId::try_new("d2", "universal").unwrap();
        let d3_id: DomainId = DomainId::try_new("d3", "universal").unwrap();
        let mut d1 = Domain::new(d1_id.clone()).build(&ALICE_ID);
        let mut d2 = Domain::new(d2_id.clone()).build(&ALICE_ID);
        let d3 = Domain::new(d3_id.clone()).build(&ALICE_ID);
        d1.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
        d2.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));

        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([d1.clone(), d2.clone(), d3.clone()], [account], []);

        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new_with_chain(world, kura, handle.clone(), ChainId::from("chain"));
        let state_view = state.view();

        // Desc sort by rank; fetch_size 2
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(SortOrder::Desc),
            },
            fetch_size: FetchSize::new(nonzero_ext::nonzero!(2_u64).into()),
        };

        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::domain::prelude::FindDomains);
        let qbox: iroha_data_model::query::QueryBox<_> =
            Box::new(iroha_data_model::query::ErasedIterQuery::<Domain>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<Domain>::PASS,
                SelectorTuple::<Domain>::default(),
                payload,
            ));
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, _remaining, cursor) = first.into_parts();
        let mut tuple_iter = batch.into_iter();
        let v = match tuple_iter.next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].id, d1_id);
        assert_eq!(v[1].id, d2_id);

        // Continue for the last (no-rank) domain
        let next = handle
            .handle_iter_continue(cursor.expect("should continue"), &ALICE_ID)
            .unwrap();
        let (batch2, _rem2, cur2) = next.into_parts();
        let mut tuple_iter2 = batch2.into_iter();
        let v2 = match tuple_iter2.next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v2.len(), 1);
        assert_eq!(v2[0].id, d3_id);
        assert!(cur2.is_none());
    }

    #[tokio::test]
    async fn iter_dispatch_nfts() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{FetchSize, Pagination, QueryParams, Sorting};
        use iroha_futures::supervisor::ShutdownSignal;

        let domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let n1 = Nft::new("n1$w.universal".parse().unwrap(), Metadata::default()).build(&ALICE_ID);
        let n2 = Nft::new("n2$w.universal".parse().unwrap(), Metadata::default()).build(&ALICE_ID);
        let world = World::with_assets([domain], [account], [], [], [n1.clone(), n2.clone()]);

        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new_with_chain(world, kura, handle.clone(), ChainId::from("chain"));
        let state_view = state.view();

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::default(),
        };
        // Build an erased iterable query over NFTs with a pass predicate and empty selector
        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::nft::prelude::FindNfts);
        let erased = iroha_data_model::query::ErasedIterQuery::<iroha_data_model::nft::Nft>::new(
            iroha_data_model::query::dsl::CompoundPredicate::PASS,
            SelectorTuple::default(),
            payload,
        );
        let qbox: iroha_data_model::query::QueryBox<iroha_data_model::query::QueryOutputBatchBox> =
            Box::new(erased);
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, _rem, _cur) = first.into_parts();
        let v = match batch.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Nft(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 2);
        assert!(v.iter().any(|x| x.id() == n1.id()));
        assert!(v.iter().any(|x| x.id() == n2.id()));
    }

    #[tokio::test]
    async fn iter_dispatch_triggers_basic() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::{
            events::time::{ExecutionTime, TimeEventFilter},
            prelude::*,
            query::parameters::{FetchSize, Pagination, QueryParams, Sorting},
        };
        use iroha_futures::supervisor::ShutdownSignal;

        // Minimal world with ALICE
        let domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([domain], [account], []);

        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());

        // Add two simple time triggers
        {
            let mut block = state.world.triggers.block();
            let mut tx = block.transaction();
            let exec = [Log::new(iroha_logger::Level::INFO, "x".into())];
            let filter = TimeEventFilter::new(ExecutionTime::PreCommit);
            let action = Action::new(exec, Repeats::Indefinitely, ALICE_ID.clone(), filter);
            let t1 = Trigger::new("t1".parse().unwrap(), action.clone())
                .try_into()
                .unwrap();
            let t2 = Trigger::new("t2".parse().unwrap(), action)
                .try_into()
                .unwrap();
            tx.add_time_trigger(t1).unwrap();
            tx.add_time_trigger(t2).unwrap();
            tx.apply();
            block.commit();
        }

        let state_view = state.view();

        // Query active trigger ids
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::default(),
        };
        // Build erased iterable FindActiveTriggerIds query
        let payload = norito::codec::Encode::encode(
            &iroha_data_model::query::trigger::prelude::FindActiveTriggerIds,
        );
        let erased =
            iroha_data_model::query::ErasedIterQuery::<iroha_data_model::trigger::TriggerId>::new(
                iroha_data_model::query::dsl::CompoundPredicate::PASS,
                SelectorTuple::default(),
                payload,
            );
        let qbox: iroha_data_model::query::QueryBox<iroha_data_model::query::QueryOutputBatchBox> =
            Box::new(erased);
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, _rem, _cur) = first.into_parts();
        let v = match batch.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::TriggerId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 2);
        let ids: std::collections::BTreeSet<_> = v.into_iter().collect();
        assert!(ids.contains(&"t1".parse().unwrap()));
        assert!(ids.contains(&"t2".parse().unwrap()));
    }

    #[tokio::test]
    async fn iter_dispatch_pagination_offset_limit() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{FetchSize, Pagination, QueryParams, Sorting};
        use iroha_futures::supervisor::ShutdownSignal;

        // World with ordered domains a,b,c
        let a: Domain = Domain::new(DomainId::try_new("a", "universal").unwrap()).build(&ALICE_ID);
        let b: Domain = Domain::new(DomainId::try_new("b", "universal").unwrap()).build(&ALICE_ID);
        let c: Domain = Domain::new(DomainId::try_new("c", "universal").unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([a.clone(), b.clone(), c.clone()], [account], []);

        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());
        let state_view = state.view();

        // Pagination: offset 1, limit 1, no sorting
        let params = QueryParams {
            pagination: Pagination::new(Some(nonzero_ext::nonzero!(1_u64)), 1),
            sorting: Sorting::default(),
            fetch_size: FetchSize::default(),
        };

        // Build erased iterable FindDomains query
        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::domain::prelude::FindDomains);
        let erased =
            iroha_data_model::query::ErasedIterQuery::<iroha_data_model::domain::Domain>::new(
                iroha_data_model::query::dsl::CompoundPredicate::PASS,
                SelectorTuple::default(),
                payload,
            );
        let qbox: iroha_data_model::query::QueryBox<iroha_data_model::query::QueryOutputBatchBox> =
            Box::new(erased);
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, remaining, cursor) = first.into_parts();
        let mut tuple_iter = batch.into_iter();
        let v = match tuple_iter.next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, b.id);
        assert_eq!(remaining, 0);
        assert!(cursor.is_none());
    }

    #[tokio::test]
    async fn iter_dispatch_offset_and_fetch_size_interplay() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{FetchSize, Pagination, QueryParams, Sorting};
        use iroha_futures::supervisor::ShutdownSignal;

        // World with ordered domains a,b,c,d
        let a: Domain = Domain::new(DomainId::try_new("a", "universal").unwrap()).build(&ALICE_ID);
        let b: Domain = Domain::new(DomainId::try_new("b", "universal").unwrap()).build(&ALICE_ID);
        let c: Domain = Domain::new(DomainId::try_new("c", "universal").unwrap()).build(&ALICE_ID);
        let d: Domain = Domain::new(DomainId::try_new("d", "universal").unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([a.clone(), b.clone(), c.clone(), d.clone()], [account], []);

        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());
        let state_view = state.view();

        // Pagination: offset 1, limit 3; fetch_size 2
        let params = QueryParams {
            pagination: Pagination::new(Some(nonzero_ext::nonzero!(3_u64)), 1),
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(nonzero_ext::nonzero!(2_u64))),
        };

        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::domain::prelude::FindDomains);
        let qbox: iroha_data_model::query::QueryBox<_> =
            Box::new(iroha_data_model::query::ErasedIterQuery::<Domain>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<Domain>::PASS,
                SelectorTuple::<Domain>::default(),
                payload,
            ));
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, remaining, cursor) = first.into_parts();
        let mut tuple_iter = batch.into_iter();
        let v = match tuple_iter.next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 2); // fetch_size=2
        assert_eq!(v[0].id, b.id); // offset skips 'a'
        assert_eq!(v[1].id, c.id);
        assert_eq!(remaining, 1); // one more item within limit
        assert!(cursor.is_some());

        // Next batch should contain the last within limit: 'd'
        let next = handle
            .handle_iter_continue(cursor.unwrap(), &ALICE_ID)
            .unwrap();
        let (batch2, remaining2, cursor2) = next.into_parts();
        let mut tuple_iter2 = batch2.into_iter();
        let v2 = match tuple_iter2.next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Domain(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v2.len(), 1);
        assert_eq!(v2[0].id, d.id);
        assert_eq!(remaining2, 0);
        assert!(cursor2.is_none());
    }

    #[tokio::test]
    async fn iter_dispatch_accounts_and_asset_definitions() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::{
            account::Account,
            asset::definition::AssetDefinition,
            query::parameters::{FetchSize, Pagination, QueryParams, Sorting},
        };
        use iroha_futures::supervisor::ShutdownSignal;

        // Build world with two accounts and two asset definitions
        let domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let (acc1_id, _) = iroha_test_samples::gen_account_in("w");
        let (acc2_id, _) = iroha_test_samples::gen_account_in("w");
        let acc1 = Account::new(acc1_id.clone()).build(&ALICE_ID);
        let acc2 = Account::new(acc2_id.clone()).build(&ALICE_ID);
        let ad1 = AssetDefinition::new(
            iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("w", "universal").unwrap(),
                "rose".parse().unwrap(),
            ),
            NumericSpec::default(),
        )
        .build(&ALICE_ID);
        let ad2 = AssetDefinition::new(
            iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("w", "universal").unwrap(),
                "tulip".parse().unwrap(),
            ),
            NumericSpec::default(),
        )
        .build(&ALICE_ID);
        let world = World::with(
            [domain],
            [acc1.clone(), acc2.clone()],
            [ad1.clone(), ad2.clone()],
        );

        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());
        let state_view = state.view();

        // Accounts: default params
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::default(),
        };
        let payload_acc =
            norito::codec::Encode::encode(&iroha_data_model::query::account::prelude::FindAccounts);
        let qbox_acc: iroha_data_model::query::QueryBox<_> =
            Box::new(iroha_data_model::query::ErasedIterQuery::<Account>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<Account>::PASS,
                SelectorTuple::<Account>::default(),
                payload_acc,
            ));
        let qwp_acc = iroha_data_model::query::QueryWithParams::new(&qbox_acc, params.clone());
        let request_acc = QueryRequest::Start(qwp_acc);
        let validated_acc = ValidQueryRequest::validate_for_client_parts(
            request_acc,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first_acc) = validated_acc
            .execute(&handle, &state_view, &ALICE_ID)
            .unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch_acc, _rem_acc, _cur_acc) = first_acc.into_parts();
        let v_acc = match batch_acc.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Account(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v_acc.len(), 2);

        // AssetDefinitions: default params
        let payload_ad = norito::codec::Encode::encode(
            &iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
        );
        let qbox_ad: iroha_data_model::query::QueryBox<_> = Box::new(
            iroha_data_model::query::ErasedIterQuery::<AssetDefinition>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<AssetDefinition>::PASS,
                SelectorTuple::<AssetDefinition>::default(),
                payload_ad,
            ),
        );
        let qwp_ad = iroha_data_model::query::QueryWithParams::new(&qbox_ad, params);
        let request_ad = QueryRequest::Start(qwp_ad);
        let validated_ad = ValidQueryRequest::validate_for_client_parts(
            request_ad,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first_ad) = validated_ad
            .execute(&handle, &state_view, &ALICE_ID)
            .unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch_ad, _rem_ad, _cur_ad) = first_ad.into_parts();
        let v_ad = match batch_ad.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::AssetDefinition(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v_ad.len(), 2);
    }

    #[tokio::test]
    async fn iter_dispatch_accounts_sort_desc_end_to_end() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{
            FetchSize, Pagination, QueryParams, SortOrder, Sorting,
        };
        use iroha_futures::supervisor::ShutdownSignal;
        use iroha_primitives::json::Json;

        // Create a domain and three accounts in it with ranked metadata
        let w: Domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let (a_id, _) = iroha_test_samples::gen_account_in("w");
        let (b_id, _) = iroha_test_samples::gen_account_in("w");
        let (c_id, _) = iroha_test_samples::gen_account_in("w");

        let a = Account::new(a_id.clone())
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
                m
            })
            .build(&a_id);
        let b = Account::new(b_id.clone())
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
                m
            })
            .build(&b_id);
        let c = Account::new(c_id.clone()).build(&c_id);

        let world = World::with([w], [a.clone(), b.clone(), c.clone()], []);
        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());
        let state_view = state.view();

        // Desc by rank
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(SortOrder::Desc),
            },
            fetch_size: FetchSize::default(),
        };

        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::account::prelude::FindAccounts);
        let qbox: iroha_data_model::query::QueryBox<_> =
            Box::new(iroha_data_model::query::ErasedIterQuery::<Account>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<Account>::PASS,
                SelectorTuple::<Account>::default(),
                payload,
            ));
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, _rem, _cur) = first.into_parts();
        let v = match batch.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Account(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 3);
        // Desc → a(rank=2), b(rank=1), c(no-rank)
        assert_eq!(v[0].id(), &a_id);
        assert_eq!(v[1].id(), &b_id);
        assert_eq!(v[2].id(), &c_id);
    }

    #[tokio::test]
    async fn iter_dispatch_accounts_sort_ties_stable_by_id() {
        use iroha_data_model::query::parameters::{
            FetchSize, Pagination, QueryParams, SortOrder, Sorting,
        };
        use iroha_primitives::json::Json;

        // Prepare three accounts with identical sortable metadata key "rank"
        let (a_id, _) = iroha_test_samples::gen_account_in("w");
        let (b_id, _) = iroha_test_samples::gen_account_in("w");
        let (c_id, _) = iroha_test_samples::gen_account_in("w");

        let make = |id: &AccountId| {
            Account::new(id.clone())
                .with_metadata({
                    let mut m = Metadata::default();
                    m.insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
                    m
                })
                .build(id)
        };
        let a = make(&a_id);
        let b = make(&b_id);
        let c = make(&c_id);

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(SortOrder::Asc),
            },
            fetch_size: FetchSize::default(),
        };

        let selector = SelectorTuple::<Account>::default();
        // Run postprocessing on a local iterator; fetch_size=nonzero!(10)
        let mut it = apply_query_postprocessing(
            vec![a, b, c].into_iter(),
            selector,
            &params,
            QueryLimits::default(),
        )
        .expect("postprocess");
        let (batch, next) = it.next_batch(0).expect("first batch");
        assert!(next.is_none());
        let v = match batch.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Account(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 3);
        let mut ids = [a_id.clone(), b_id.clone(), c_id.clone()];
        ids.sort();
        assert_eq!(v[0].id(), &ids[0]);
        assert_eq!(v[1].id(), &ids[1]);
        assert_eq!(v[2].id(), &ids[2]);
    }

    #[tokio::test]
    async fn iter_dispatch_asset_definitions_sort_ties_stable_by_id() {
        use iroha_data_model::query::parameters::{
            FetchSize, Pagination, QueryParams, SortOrder, Sorting,
        };
        use iroha_primitives::json::Json;

        let make = |name: &str| {
            AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("w", "universal").unwrap(),
                name.parse().unwrap(),
            ))
            .with_metadata({
                let mut metadata = Metadata::default();
                metadata.insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
                metadata
            })
            .build(&ALICE_ID)
        };

        let ad_a = make("rose");
        let ad_b = make("tulip");
        let ad_c = make("peony");

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(SortOrder::Asc),
            },
            fetch_size: FetchSize::default(),
        };

        let selector = SelectorTuple::<AssetDefinition>::default();
        let mut it = apply_query_postprocessing(
            vec![ad_a.clone(), ad_b.clone(), ad_c.clone()].into_iter(),
            selector,
            &params,
            QueryLimits::default(),
        )
        .expect("postprocess");
        let (batch, next) = it.next_batch(0).expect("first batch");
        assert!(next.is_none());
        let v = match batch.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::AssetDefinition(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 3);
        let mut ids = [ad_a.id().clone(), ad_b.id().clone(), ad_c.id().clone()];
        ids.sort();
        assert_eq!(v[0].id(), &ids[0]);
        assert_eq!(v[1].id(), &ids[1]);
        assert_eq!(v[2].id(), &ids[2]);
    }

    #[tokio::test]
    async fn iter_dispatch_asset_definitions_sort_desc() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{
            FetchSize, Pagination, QueryParams, SortOrder, Sorting,
        };
        use iroha_futures::supervisor::ShutdownSignal;

        let domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let mut ad1 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "rose".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let mut ad2 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "tulip".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let ad3 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "peony".parse().unwrap(),
        ))
        .build(&ALICE_ID); // no rank
        ad1.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
        ad2.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
        let world = World::with([domain], [account], [ad1.clone(), ad2.clone(), ad3.clone()]);

        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());
        let state_view = state.view();

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(SortOrder::Desc),
            },
            fetch_size: FetchSize::default(),
        };

        let payload = norito::codec::Encode::encode(
            &iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
        );
        let qbox: iroha_data_model::query::QueryBox<_> = Box::new(
            iroha_data_model::query::ErasedIterQuery::<AssetDefinition>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<AssetDefinition>::PASS,
                SelectorTuple::<AssetDefinition>::default(),
                payload,
            ),
        );
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, _rem, _cur) = first.into_parts();
        let v = match batch.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::AssetDefinition(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 3);
        // Desc → ad2(rank=2), ad1(rank=1), ad3(no-rank)
        assert_eq!(v[0].id(), ad2.id());
        assert_eq!(v[1].id(), ad1.id());
        assert_eq!(v[2].id(), ad3.id());
    }

    #[tokio::test]
    async fn iter_dispatch_find_triggers_full() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::{
            events::time::{ExecutionTime, TimeEventFilter},
            prelude::*,
            query::parameters::{FetchSize, Pagination, QueryParams, Sorting},
        };
        use iroha_futures::supervisor::ShutdownSignal;

        let domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([domain], [account], []);

        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());

        // Insert two time triggers
        {
            let mut block = state.world.triggers.block();
            let mut tx = block.transaction();
            let exec = [Log::new(iroha_logger::Level::INFO, "x".into())];
            let filter = TimeEventFilter::new(ExecutionTime::PreCommit);
            let action = Action::new(exec, Repeats::Indefinitely, ALICE_ID.clone(), filter);
            let t1 = Trigger::new("t1".parse().unwrap(), action.clone())
                .try_into()
                .unwrap();
            let t2 = Trigger::new(
                "t2".parse().unwrap(),
                action.with_metadata(Metadata::default()),
            )
            .try_into()
            .unwrap();
            tx.add_time_trigger(t1).unwrap();
            tx.add_time_trigger(t2).unwrap();
            tx.apply();
            block.commit();
        }

        let state_view = state.view();
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::default(),
        };
        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::trigger::prelude::FindTriggers);
        let qbox: iroha_data_model::query::QueryBox<_> =
            Box::new(iroha_data_model::query::ErasedIterQuery::<Trigger>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<Trigger>::PASS,
                SelectorTuple::<Trigger>::default(),
                payload,
            ));
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, _rem, _cur) = first.into_parts();
        let v = match batch.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Trigger(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 2);
        let ids: std::collections::BTreeSet<_> = v.iter().map(|t| t.id().clone()).collect();
        assert!(ids.contains(&"t1".parse().unwrap()));
        assert!(ids.contains(&"t2".parse().unwrap()));
        // Basic field assertions on the fetched triggers
        for tr in v {
            match tr.action().filter() {
                iroha_data_model::events::EventFilterBox::Time(_) => {}
                other => panic!("unexpected filter: {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn find_all_blocks() -> Result<()> {
        let num_blocks = 100;

        let state = state_with_test_blocks_and_transactions(num_blocks, 1, 1)?;
        let blocks = ValidQuery::execute(FindBlocks, CompoundPredicate::PASS, &state.view())?
            .collect::<Vec<_>>();

        assert_eq!(blocks.len() as u64, num_blocks);
        assert!(
            blocks
                .windows(2)
                .all(|wnd| wnd[0].header() >= wnd[1].header())
        );

        Ok(())
    }

    #[tokio::test]
    async fn find_all_block_headers() -> Result<()> {
        let num_blocks = 100;

        let state = state_with_test_blocks_and_transactions(num_blocks, 1, 1)?;
        let block_headers =
            ValidQuery::execute(FindBlockHeaders, CompoundPredicate::PASS, &state.view())?
                .collect::<Vec<_>>();

        assert_eq!(block_headers.len() as u64, num_blocks);
        assert!(block_headers.windows(2).all(|wnd| wnd[0] >= wnd[1]));

        Ok(())
    }

    #[tokio::test]
    async fn find_blocks_and_headers_by_height() -> Result<()> {
        let state = state_with_test_blocks_and_transactions(10, 1, 1)?;
        let state_view = state.view();

        let blocks = ValidQuery::execute(
            FindBlocks,
            CompoundPredicate::<iroha_data_model::block::SignedBlock>::build(|p| {
                p.equals("height", 4_u64)
            }),
            &state_view,
        )?
        .collect::<Vec<_>>();

        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].header().height().get(), 4);
        let target_hash = blocks[0].hash();

        let header_by_hash = ValidQuery::execute(
            FindBlockHeaders,
            CompoundPredicate::<iroha_data_model::block::BlockHeader>::build(|p| {
                p.equals("hash", target_hash)
            }),
            &state_view,
        )?
        .collect::<Vec<_>>();

        assert_eq!(header_by_hash.len(), 1);
        assert_eq!(header_by_hash[0].height().get(), 4);

        let headers = ValidQuery::execute(
            FindBlockHeaders,
            CompoundPredicate::<iroha_data_model::block::BlockHeader>::build(|p| {
                p.in_values("height", [2_u64, 7_u64, 3_u64])
            }),
            &state_view,
        )?
        .map(|header| header.height().get())
        .collect::<Vec<_>>();

        assert_eq!(headers, vec![7, 3, 2]);

        let missing = ValidQuery::execute(
            FindBlockHeaders,
            CompoundPredicate::<iroha_data_model::block::BlockHeader>::build(|p| {
                p.equals("height", 42_u64)
            }),
            &state_view,
        )?
        .collect::<Vec<_>>();

        assert!(missing.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn find_block_header_by_hash() -> Result<()> {
        let state = state_with_test_blocks_and_transactions(1, 1, 1)?;
        let state_view = state.view();
        let block = state_view
            .all_blocks(nonzero!(1_usize))
            .last()
            .expect("state is empty");

        let mut headers = FindBlockHeaders::new()
            .execute(CompoundPredicate::PASS, &state_view)
            .expect("Query execution should not fail");
        let found = headers.any(|header| header.hash() == block.hash());
        assert!(found, "Query should return the block header");

        let unexpected_hash = HashOf::from_untyped_unchecked(Hash::new([42]));
        let missing = FindBlockHeaders::new()
            .execute(CompoundPredicate::PASS, &state_view)
            .expect("Query execution should not fail")
            .any(|header| header.hash() == unexpected_hash);
        assert!(!missing, "Block header should not be found");

        Ok(())
    }

    #[tokio::test]
    async fn start_iterable_query_for_domains() -> Result<()> {
        use iroha_data_model::query::{
            ErasedIterQuery, QueryBox, QueryOutputBatchBox, QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
            parameters::QueryParams,
        };
        // Build a small state with a domain and account
        let state = state_with_test_blocks_and_transactions(1, 1, 0)?;
        let state_view = state.view();
        let query_handle = LiveQueryStore::start_test();

        // Build an erased iterable query over domains with a pass predicate and empty selector
        // Build an erased query with preserved payload for dispatch
        let payload = norito::codec::Encode::encode(&FindDomains);
        let erased: ErasedIterQuery<iroha_data_model::domain::Domain> =
            ErasedIterQuery::new(CompoundPredicate::PASS, SelectorTuple::default(), payload);
        let boxed: QueryBox<QueryOutputBatchBox> = Box::new(erased);
        let iter_query = QueryWithParams::new(&boxed, QueryParams::default());

        let request = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(iter_query),
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )?;

        let response = request.execute(&query_handle, &state_view, &ALICE_ID)?;
        match response {
            QueryResponse::Iterable(output) => {
                // Should produce a batch and optionally a cursor
                let (_batch, _rem, _cursor) = output.into_parts();
            }
            _ => panic!("expected iterable response"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn iterable_sorting_by_metadata_desc() -> Result<()> {
        use iroha_data_model::query::{
            QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
            parameters::{QueryParams, Sorting},
        };

        // Build a state and add two domains with comparable metadata
        let kura = Kura::blank_kura_for_testing();
        let state = State::new(
            world_with_test_domains(),
            kura.clone(),
            LiveQueryStore::start_test(),
        );
        let parent_block = state.view().latest_block();
        let block_header = ValidBlock::new_dummy(&bls_test_keypair().into_parts().1)
            .as_ref()
            .header();
        let mut state_block = state.block(block_header);
        let mut state_tx = state_block.transaction();

        // Register a second domain
        let alpha_id = DomainId::try_new("alpha", "universal").expect("valid");
        Register::domain(Domain::new(alpha_id.clone())).execute(&ALICE_ID, &mut state_tx)?;

        // Set metadata key "rank" on both domains: wonderland=1, alpha=2
        let key = "rank".parse::<Name>().expect("valid");
        SetKeyValue::domain(
            DomainId::try_new("wonderland", "universal").unwrap(),
            key.clone(),
            Json::new(1_u32),
        )
        .execute(&ALICE_ID, &mut state_tx)?;
        SetKeyValue::domain(alpha_id.clone(), key.clone(), Json::new(2_u32))
            .execute(&ALICE_ID, &mut state_tx)?;

        // Apply world changes and commit a minimal block to satisfy transaction storage invariants
        state_tx.apply();

        let (peer_pk, _) = bls_test_keypair().into_parts();
        let peer_id = PeerId::new(peer_pk);
        let topology = Topology::new(vec![peer_id]);
        let unverified_block = BlockBuilder::new(vec![dummy_accepted_transaction()])
            .chain(0, parent_block.as_deref())
            .sign(ALICE_KEYPAIR.private_key())
            .unpack(|_| {});
        let vcb = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {})
            .commit(&topology)
            .unpack(|_| {})
            .unwrap();

        let _events = state_block.apply(&vcb, topology.as_ref().to_owned());
        kura.store_block(vcb).expect("store block");
        state_block.commit().unwrap();

        // Build an erased iterable query over domains with sorting by metadata desc (fast_dsl bundle)
        let params = QueryParams {
            sorting: Sorting {
                sort_by_metadata_key: Some(key.clone()),
                order: Some(iroha_data_model::query::parameters::SortOrder::Desc),
            },
            ..Default::default()
        };
        let iter_query = QueryWithParams {
            query: (),
            query_payload: Vec::new(),
            item: iroha_data_model::query::QueryItemKind::Domain,
            predicate_bytes: norito::codec::Encode::encode(&CompoundPredicate::<Domain>::PASS),
            selector_bytes: norito::codec::Encode::encode(&SelectorTuple::<Domain>::default()),
            params,
        };

        let req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(iter_query),
            &ALICE_ID,
            &state.view(),
            QueryLimits::default(),
        )?;
        let resp = req.execute(&LiveQueryStore::start_test(), &state.view(), &ALICE_ID)?;
        let QueryResponse::Iterable(output) = resp else {
            panic!("expected iterable response")
        };
        let (_batch, _rem, _cursor) = output.into_parts();

        Ok(())
    }

    #[tokio::test]
    async fn iter_dispatch_assets_non_empty_and_contains_minted() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{FetchSize, Pagination, QueryParams, Sorting};
        use iroha_futures::supervisor::ShutdownSignal;

        // World with a domain, ALICE account, one asset definition, and a minted asset
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let ad_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_id = AssetId::new(ad_id.clone(), ALICE_ID.clone());

        let world = World::default();
        // Build state and register domain/account/asset def; then mint
        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());
        let header = ValidBlock::new_dummy(ALICE_KEYPAIR.private_key())
            .as_ref()
            .header();
        let mut sblock = state.block(header);
        let mut stx = sblock.transaction();
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .expect("register domain");
        Register::account(Account::new(ALICE_ID.clone()))
            .execute(&ALICE_ID, &mut stx)
            .expect("register account");
        Register::asset_definition(
            AssetDefinition::numeric(ad_id.clone()).with_name(ad_id.name().to_string()),
        )
        .execute(&ALICE_ID, &mut stx)
        .expect("register asset definition");
        Mint::asset_quantity(13_u32, asset_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("mint asset");
        stx.apply();
        let _ = sblock.commit();

        let state_view = state.view();

        // Default params
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::default(),
        };

        // Build erased iterable FindAssets query
        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::asset::prelude::FindAssets);
        let qbox: iroha_data_model::query::QueryBox<_> =
            Box::new(iroha_data_model::query::ErasedIterQuery::<Asset>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<Asset>::PASS,
                SelectorTuple::<Asset>::default(),
                payload,
            ));
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .expect("validate");
        let QueryResponse::Iterable(first) = validated
            .execute(&handle, &state_view, &ALICE_ID)
            .expect("execute")
        else {
            panic!("expected iterable")
        };
        let (batch, _rem, _cur) = first.into_parts();
        let v = match batch.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Asset(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert!(!v.is_empty(), "expected at least one asset");
        let rose = v
            .iter()
            .find(|a| a.id() == &asset_id)
            .expect("minted asset not found");
        assert_eq!(rose.value().as_numeric(), &numeric!(13));
    }

    #[tokio::test]
    async fn fast_dsl_iter_accounts_with_asset_uses_payload() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::{
            QueryItemKind, QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
            parameters::{FetchSize, Pagination, QueryParams, Sorting},
        };
        use iroha_futures::supervisor::ShutdownSignal;

        // World with a domain, two accounts, one asset definition, and a minted asset to one account
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let (acc1_id, _) = iroha_test_samples::gen_account_in("wonderland");
        let (acc2_id, _) = iroha_test_samples::gen_account_in("wonderland");
        let ad_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_id = AssetId::new(ad_id.clone(), acc1_id.clone());

        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(World::default(), kura, handle.clone());
        let header = ValidBlock::new_dummy(ALICE_KEYPAIR.private_key())
            .as_ref()
            .header();
        let mut sblock = state.block(header);
        let mut stx = sblock.transaction();
        Register::domain(Domain::new(domain_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .expect("register domain");
        Register::account(Account::new(acc1_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .expect("register account1");
        Register::account(Account::new(acc2_id.clone()))
            .execute(&ALICE_ID, &mut stx)
            .expect("register account2");
        Register::asset_definition(
            AssetDefinition::numeric(ad_id.clone()).with_name(ad_id.name().to_string()),
        )
        .execute(&ALICE_ID, &mut stx)
        .expect("register asset definition");
        Mint::asset_quantity(1_u32, asset_id.clone())
            .execute(&ALICE_ID, &mut stx)
            .expect("mint asset");
        stx.apply();
        let _ = sblock.commit();

        let state_view = state.view();

        // fast_dsl-style iterable query bundle: Accounts + payload FindAccountsWithAsset
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::default(),
        };
        let query_payload = norito::codec::Encode::encode(
            &iroha_data_model::query::account::prelude::FindAccountsWithAsset::new(ad_id.clone()),
        );
        let iter_query = QueryWithParams {
            query: (),
            query_payload,
            item: QueryItemKind::Account,
            predicate_bytes: norito::codec::Encode::encode(&CompoundPredicate::<Account>::PASS),
            selector_bytes: norito::codec::Encode::encode(&SelectorTuple::<Account>::default()),
            params,
        };

        let validated = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(iter_query),
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .expect("validate");
        let QueryResponse::Iterable(first) = validated
            .execute(&handle, &state_view, &ALICE_ID)
            .expect("execute")
        else {
            panic!("expected iterable")
        };
        let (batch, _rem, _cur) = first.into_parts();
        let v = match batch.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Account(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        // Should only include account that holds the specified asset
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id(), &acc1_id);
    }

    #[tokio::test]
    #[allow(clippy::too_many_lines)]
    async fn iter_dispatch_accounts_with_asset_parity_and_continue() {
        use std::collections::BTreeSet;

        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::{
            QueryBox, QueryItemKind, QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryRequest,
            QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
            parameters::{FetchSize, QueryParams, Sorting},
        };
        use iroha_futures::supervisor::ShutdownSignal;

        fn build_state_with_holdings() -> (
            State,
            crate::query::store::LiveQueryStoreHandle,
            AssetDefinitionId,
        ) {
            let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
            let ad_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            );

            let kura = Kura::blank_kura_for_testing();
            let store = std::sync::Arc::new(LiveQueryStore::from_config(
                StoreCfg::default(),
                ShutdownSignal::new(),
            ));
            let handle = crate::query::store::LiveQueryStoreHandle::new(store);
            let state = State::new(World::default(), kura, handle.clone());

            let header = ValidBlock::new_dummy(ALICE_KEYPAIR.private_key())
                .as_ref()
                .header();
            let mut sblock = state.block(header);
            let mut stx = sblock.transaction();

            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register domain");
            Register::account(Account::new(ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register ALICE");
            Register::account(Account::new(BOB_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("register BOB");
            Register::asset_definition(
                AssetDefinition::numeric(ad_id.clone()).with_name(ad_id.name().to_string()),
            )
            .execute(&ALICE_ID, &mut stx)
            .expect("register asset definition");
            Mint::asset_quantity(5_u32, AssetId::new(ad_id.clone(), ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("mint asset for ALICE");
            Mint::asset_quantity(7_u32, AssetId::new(ad_id.clone(), BOB_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("mint asset for BOB");
            stx.apply();
            let _ = sblock.commit();

            (state, handle, ad_id)
        }

        fn drain_accounts(
            first_batch: iroha_data_model::query::QueryOutput,
            handle: &crate::query::store::LiveQueryStoreHandle,
        ) -> Vec<AccountId> {
            fn to_ids(batch: QueryOutputBatchBoxTuple) -> Vec<AccountId> {
                let mut iter = batch.into_iter();
                let accounts = match iter.next().expect("single tuple element") {
                    QueryOutputBatchBox::Account(v) => v,
                    other => panic!("unexpected batch variant: {other:?}"),
                };
                accounts.into_iter().map(|acc| acc.id().clone()).collect()
            }

            let (batch, _remaining, mut cursor) = first_batch.into_parts();
            let mut ids = to_ids(batch);

            while let Some(c) = cursor {
                let next = handle
                    .handle_iter_continue(c, &ALICE_ID)
                    .expect("continue cursor");
                let (next_batch, _next_remaining, next_cursor) = next.into_parts();
                ids.extend(to_ids(next_batch));
                cursor = next_cursor;
            }

            ids
        }

        let params = QueryParams {
            sorting: Sorting::default(),
            pagination: Pagination::default(),
            fetch_size: FetchSize {
                fetch_size: Some(nonzero!(1_u64)),
            },
        };

        // Erased QueryBox path with encoded FindAccountsWithAsset payload.
        let (state_boxed, handle_boxed, ad_id) = build_state_with_holdings();
        let payload = norito::codec::Encode::encode(
            &iroha_data_model::query::account::prelude::FindAccountsWithAsset::new(ad_id.clone()),
        );
        let qbox: QueryBox<_> = Box::new(iroha_data_model::query::ErasedIterQuery::<Account>::new(
            CompoundPredicate::PASS,
            SelectorTuple::default(),
            payload,
        ));
        let boxed_qwp = QueryWithParams::new(&qbox, params.clone());
        let boxed_req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(boxed_qwp),
            &ALICE_ID,
            &state_boxed.view(),
            QueryLimits::default(),
        )
        .expect("validate boxed");
        let QueryResponse::Iterable(first_boxed) = boxed_req
            .execute(&handle_boxed, &state_boxed.view(), &ALICE_ID)
            .expect("execute boxed")
        else {
            panic!("expected iterable response");
        };
        let boxed_accounts = drain_accounts(first_boxed, &handle_boxed);

        // fast_dsl-style bundle using predicate/selector bytes and payload.
        let (state_fast, handle_fast, ad_id_fast) = build_state_with_holdings();
        let iter_query = QueryWithParams {
            query: (),
            query_payload: norito::codec::Encode::encode(
                &iroha_data_model::query::account::prelude::FindAccountsWithAsset::new(
                    ad_id_fast.clone(),
                ),
            ),
            item: QueryItemKind::Account,
            predicate_bytes: norito::codec::Encode::encode(&CompoundPredicate::<Account>::PASS),
            selector_bytes: norito::codec::Encode::encode(&SelectorTuple::<Account>::default()),
            params,
        };
        let fast_req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(iter_query),
            &ALICE_ID,
            &state_fast.view(),
            QueryLimits::default(),
        )
        .expect("validate fast");
        let QueryResponse::Iterable(first_fast) = fast_req
            .execute(&handle_fast, &state_fast.view(), &ALICE_ID)
            .expect("execute fast")
        else {
            panic!("expected iterable response");
        };
        let fast_accounts = drain_accounts(first_fast, &handle_fast);

        let expected: BTreeSet<_> = [ALICE_ID.clone(), BOB_ID.clone()].into_iter().collect();
        let boxed_set: BTreeSet<_> = boxed_accounts.into_iter().collect();
        let fast_set: BTreeSet<_> = fast_accounts.into_iter().collect();
        assert_eq!(boxed_set, expected);
        assert_eq!(fast_set, expected);
        assert_eq!(boxed_set, fast_set);
    }

    #[tokio::test]
    async fn find_all_transactions() -> Result<()> {
        let num_blocks = 100;

        let state = state_with_test_blocks_and_transactions(num_blocks, 1, 1)?;
        let txs = ValidQuery::execute(FindTransactions, CompoundPredicate::PASS, &state.view())?
            .collect::<Vec<_>>();

        assert_eq!(txs.len() as u64, num_blocks * 2);
        assert_eq!(
            txs.iter().filter(|txn| txn.result().is_err()).count() as u64,
            num_blocks
        );
        assert_eq!(
            txs.iter().filter(|txn| txn.result().is_err()).count() as u64,
            num_blocks
        );

        Ok(())
    }

    #[test]
    fn find_transactions_bounded_ephemeral_scans_only_the_page_carriers() {
        use iroha_data_model::query::parameters::{FetchSize, Pagination, Sorting};

        let fixture = crate::smartcontracts::isi::tx::tests::merge_query_fixture();
        let state_view = fixture.sandbox.state.view();
        let query_handle = state_view.query_handle().clone();
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(nonzero!(3_u64))),
        };
        let limits = QueryLimits::default().with_count_mode(QueryCountMode::Bounded);
        let validated = ValidQueryRequest::validate_for_client_parts(
            find_transactions_request(params),
            &ALICE_ID,
            &state_view,
            limits,
        )
        .expect("validate bounded transaction query");
        state_view.kura().reset_merge_query_read_counters_for_test();

        let QueryResponse::Iterable(output) = validated
            .execute_ephemeral(&query_handle, &state_view, &ALICE_ID)
            .expect("execute bounded transaction query")
        else {
            panic!("expected iterable transaction output");
        };
        let (batch, remaining_items, has_more, cursor) = output.into_parts_with_count_mode();
        assert_eq!(transactions_from_batch(batch).len(), 3);
        assert_eq!(remaining_items, None);
        assert!(has_more);
        assert!(cursor.is_none());
        assert_eq!(
            state_view.kura().merge_query_read_counters_for_test(),
            (0, 0, 2),
            "three transactions plus the bounded probe touch two two-entry carriers"
        );
    }

    #[test]
    fn find_transactions_bounded_replay_ignores_blocks_appended_after_start() {
        use iroha_data_model::query::parameters::{FetchSize, Pagination, Sorting};

        let fixture = crate::smartcontracts::isi::tx::tests::merge_query_fixture();
        let state = Arc::new(fixture.sandbox.state);
        let state_view = state.view();
        let query_handle = state_view.query_handle().clone();
        let expected = crate::smartcontracts::isi::tx::committed_transactions_snapshot(&state_view)
            .expect("bounded replay baseline");
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(nonzero!(3_u64))),
        };
        let validated = ValidQueryRequest::validate_for_client_parts(
            find_transactions_request(params),
            &ALICE_ID,
            &state_view,
            QueryLimits::default().with_count_mode(QueryCountMode::Bounded),
        )
        .expect("validate bounded replay query");
        let QueryResponse::Iterable(first) = validated
            .execute_with_replay_state(
                &query_handle,
                &state_view,
                &ALICE_ID,
                Arc::downgrade(&state),
            )
            .expect("start bounded replay query")
        else {
            panic!("expected iterable transaction output");
        };
        let mut collected = transactions_from_batch(first.batch);
        let mut cursor = first.continue_cursor;

        let latest_height = NonZeroUsize::new(state_view.height()).expect("seeded history");
        let latest = state_view
            .kura()
            .get_block(latest_height)
            .expect("latest seeded carrier");
        let (appended, entry) =
            crate::smartcontracts::isi::tx::tests::certified_query_carrier(&latest, 17, true);
        state_view
            .kura()
            .store_block_with_merge_entry(appended, &entry)
            .expect("append carrier after query start");

        while let Some(current) = cursor {
            let next = query_handle
                .handle_iter_continue(current, &ALICE_ID)
                .expect("continue bounded replay query");
            collected.extend(transactions_from_batch(next.batch));
            cursor = next.continue_cursor;
        }
        assert_eq!(collected, expected);
    }

    #[test]
    fn find_transactions_exact_ephemeral_counts_without_complete_carrier_snapshot() {
        use iroha_data_model::query::parameters::{FetchSize, Pagination, Sorting};

        let fixture = crate::smartcontracts::isi::tx::tests::merge_query_fixture();
        let state_view = fixture.sandbox.state.view();
        let query_handle = state_view.query_handle().clone();
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(nonzero!(3_u64))),
        };
        let validated = ValidQueryRequest::validate_for_client_parts(
            find_transactions_request(params),
            &ALICE_ID,
            &state_view,
            QueryLimits::default().with_count_mode(QueryCountMode::Exact),
        )
        .expect("validate exact transaction query");
        state_view.kura().reset_merge_query_read_counters_for_test();

        let item_budget = QueryExecutionBudget::from_weighted_limit(64, 1, 0);
        let (QueryResponse::Iterable(output), stats) = validated
            .execute_ephemeral_with_stats(&query_handle, &state_view, &ALICE_ID, Some(item_budget))
            .expect("execute exact transaction query")
        else {
            panic!("expected iterable transaction output");
        };
        let (batch, remaining_items, has_more, cursor) = output.into_parts_with_count_mode();
        assert_eq!(transactions_from_batch(batch).len(), 3);
        assert_eq!(remaining_items, Some(29));
        assert!(has_more);
        assert!(cursor.is_none());
        assert_eq!(stats.processed_items(), 32);
        assert_eq!(
            state_view.kura().merge_query_read_counters_for_test(),
            (0, 0, 16),
            "exact query point-resolves every carrier without materializing a carrier snapshot"
        );
    }

    #[test]
    fn find_transactions_exact_budget_charges_matches_outside_pagination_window() {
        use iroha_data_model::query::parameters::{FetchSize, Pagination, Sorting};

        let fixture = crate::smartcontracts::isi::tx::tests::merge_query_fixture();
        let state_view = fixture.sandbox.state.view();
        let query_handle = state_view.query_handle().clone();
        let limits = QueryLimits::default().with_count_mode(QueryCountMode::Exact);

        for offset in [0, 31] {
            let params = QueryParams {
                pagination: Pagination::new(Some(nonzero!(1_u64)), offset),
                sorting: Sorting::default(),
                fetch_size: FetchSize::new(Some(nonzero!(1_u64))),
            };
            let validated = ValidQueryRequest::validate_for_client_parts(
                find_transactions_request(params),
                &ALICE_ID,
                &state_view,
                limits,
            )
            .expect("validate budgeted exact transaction query");
            let item_budget = QueryExecutionBudget::from_weighted_limit(1, 1, 0);
            let err = validated
                .execute_ephemeral_with_stats(
                    &query_handle,
                    &state_view,
                    &ALICE_ID,
                    Some(item_budget),
                )
                .expect_err("exact full-history scan must exceed a one-item budget");
            assert_eq!(err, Error::GasBudgetExceeded);
        }
    }

    #[test]
    fn find_transactions_false_predicate_cannot_force_uncharged_projection() {
        use iroha_data_model::query::parameters::{FetchSize, Pagination, Sorting};

        let fixture = crate::smartcontracts::isi::tx::tests::merge_query_fixture();
        let state_view = fixture.sandbox.state.view();
        let query_handle = state_view.query_handle().clone();
        let false_filter = CompoundPredicate::<CommittedTransaction>::build(|prototype| {
            prototype.equals("field_that_does_not_exist", true)
        });
        for (count_mode, sorting) in [
            (QueryCountMode::Bounded, Sorting::default()),
            (QueryCountMode::Exact, Sorting::default()),
            (
                QueryCountMode::Bounded,
                Sorting {
                    sort_by_metadata_key: Some("rank".parse().expect("metadata key")),
                    order: Some(SortOrder::Asc),
                },
            ),
            (
                QueryCountMode::Exact,
                Sorting {
                    sort_by_metadata_key: Some("rank".parse().expect("metadata key")),
                    order: Some(SortOrder::Desc),
                },
            ),
        ] {
            let params = QueryParams {
                pagination: Pagination::new(Some(nonzero!(1_u64)), 0),
                sorting,
                fetch_size: FetchSize::new(Some(nonzero!(1_u64))),
            };
            let validated = ValidQueryRequest::validate_for_client_parts(
                find_transactions_request_with_filter(params, false_filter.clone()),
                &ALICE_ID,
                &state_view,
                QueryLimits::default().with_count_mode(count_mode),
            )
            .expect("validate false-predicate transaction query");
            crate::smartcontracts::isi::tx::reset_certified_merge_projection_calls_for_test();

            let item_budget = QueryExecutionBudget::from_weighted_limit(1, 1, 0);
            let err = validated
                .execute_ephemeral_with_stats(
                    &query_handle,
                    &state_view,
                    &ALICE_ID,
                    Some(item_budget),
                )
                .expect_err("eager carrier projection must be precharged before proof work");
            assert_eq!(err, Error::GasBudgetExceeded);
            assert_eq!(
                crate::smartcontracts::isi::tx::certified_merge_projection_calls_for_test(),
                0,
                "insufficient gas must reject before merge Merkle proof reconstruction"
            );
        }
    }

    #[test]
    fn find_transactions_stored_start_precharges_before_false_predicate_or_sorted_projection() {
        use iroha_data_model::query::parameters::{FetchSize, Pagination, Sorting};

        let mut fixture = crate::smartcontracts::isi::tx::tests::merge_query_fixture();
        fixture.sandbox.state.pipeline.query_stored_min_gas_units = 1;
        let state = Arc::new(fixture.sandbox.state);
        let state_view = state.view();
        let query_handle = state_view.query_handle().clone();
        let false_filter = CompoundPredicate::<CommittedTransaction>::build(|prototype| {
            prototype.equals("field_that_does_not_exist", true)
        });

        for (count_mode, sorting) in [
            (QueryCountMode::Bounded, Sorting::default()),
            (
                QueryCountMode::Exact,
                Sorting {
                    sort_by_metadata_key: Some("rank".parse().expect("metadata key")),
                    order: Some(SortOrder::Desc),
                },
            ),
        ] {
            let params = QueryParams {
                pagination: Pagination::new(Some(nonzero!(1_u64)), 0),
                sorting,
                fetch_size: FetchSize::new(Some(nonzero!(1_u64))),
            };
            let validated = ValidQueryRequest::validate_for_client_parts(
                find_transactions_request_with_filter(params, false_filter.clone()),
                &ALICE_ID,
                &state_view,
                QueryLimits::default().with_count_mode(count_mode),
            )
            .expect("validate stored transaction query");
            state_view.kura().reset_merge_query_read_counters_for_test();
            crate::smartcontracts::isi::tx::reset_certified_merge_projection_calls_for_test();

            let err = validated
                .execute_with_replay_state_and_start_budget(
                    &query_handle,
                    &state_view,
                    &ALICE_ID,
                    Arc::downgrade(&state),
                    Some(1),
                )
                .expect_err("stored start must enforce its projection budget");
            assert_eq!(err, Error::GasBudgetExceeded);
            assert_eq!(
                crate::smartcontracts::isi::tx::certified_merge_projection_calls_for_test(),
                0,
                "stored start must reject before merge Merkle proof reconstruction"
            );
            assert_eq!(
                state_view.kura().merge_query_read_counters_for_test(),
                (0, 0, 0),
                "stored start must reject before merge sidecar resolution or decode"
            );
        }

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(nonzero!(1_u64))),
        };
        let validated = ValidQueryRequest::validate_for_client_parts(
            find_transactions_request(params),
            &ALICE_ID,
            &state_view,
            QueryLimits::default().with_count_mode(QueryCountMode::Bounded),
        )
        .expect("validate sufficiently budgeted stored transaction query");
        let QueryResponse::Iterable(output) = validated
            .execute_with_replay_state_and_start_budget(
                &query_handle,
                &state_view,
                &ALICE_ID,
                Arc::downgrade(&state),
                Some(2),
            )
            .expect("client budget above the server minimum covers one carrier")
        else {
            panic!("expected iterable transaction output");
        };
        assert_eq!(
            output
                .continue_cursor
                .expect("stored continuation")
                .gas_budget,
            Some(2),
            "the actual validated Start budget must be carried into the cursor"
        );
    }

    #[test]
    fn find_transactions_stored_continue_precharges_and_underfunded_retry_does_not_advance() {
        use iroha_data_model::query::parameters::{FetchSize, Pagination, Sorting};

        let mut fixture = crate::smartcontracts::isi::tx::tests::merge_query_fixture();
        fixture.sandbox.state.pipeline.query_stored_min_gas_units = 2;
        let state = Arc::new(fixture.sandbox.state);
        let state_view = state.view();
        let query_handle = state_view.query_handle().clone();
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(nonzero!(1_u64))),
        };
        let validated = ValidQueryRequest::validate_for_client_parts(
            find_transactions_request(params),
            &ALICE_ID,
            &state_view,
            QueryLimits::default().with_count_mode(QueryCountMode::Bounded),
        )
        .expect("validate stored transaction query");
        let QueryResponse::Iterable(first) = validated
            .execute_with_replay_state(
                &query_handle,
                &state_view,
                &ALICE_ID,
                Arc::downgrade(&state),
            )
            .expect("start budgeted stored transaction query")
        else {
            panic!("expected iterable transaction output");
        };
        let original_cursor = first.continue_cursor.expect("stored continuation");
        assert_eq!(original_cursor.gas_budget, Some(2));

        let mut underfunded = original_cursor.clone();
        underfunded.gas_budget = Some(1);
        state_view.kura().reset_merge_query_read_counters_for_test();
        crate::smartcontracts::isi::tx::reset_certified_merge_projection_calls_for_test();
        let err = query_handle
            .handle_iter_continue(underfunded, &ALICE_ID)
            .expect_err("continuation must enforce its current request budget");
        assert_eq!(err, Error::GasBudgetExceeded);
        assert_eq!(
            crate::smartcontracts::isi::tx::certified_merge_projection_calls_for_test(),
            0,
            "underfunded continuation must reject before merge Merkle proof reconstruction"
        );
        assert_eq!(
            state_view.kura().merge_query_read_counters_for_test(),
            (0, 0, 0),
            "underfunded continuation must reject before merge sidecar resolution or decode"
        );

        let next = query_handle
            .handle_iter_continue(original_cursor, &ALICE_ID)
            .expect("the same cursor remains retryable with sufficient budget");
        assert_eq!(transactions_from_batch(next.batch).len(), 1);
        assert!(
            crate::smartcontracts::isi::tx::certified_merge_projection_calls_for_test() > 0,
            "successful retry should reconstruct the selected carrier proof"
        );
    }

    #[test]
    fn find_transactions_exact_stored_replay_preserves_count_cursor_and_order() {
        use iroha_data_model::query::parameters::{FetchSize, Pagination, Sorting};

        let fixture = crate::smartcontracts::isi::tx::tests::merge_query_fixture();
        let state = Arc::new(fixture.sandbox.state);
        let state_view = state.view();
        let query_handle = state_view.query_handle().clone();
        let expected = crate::smartcontracts::isi::tx::committed_transactions_snapshot(&state_view)
            .expect("exact transaction baseline");
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(nonzero!(5_u64))),
        };
        let limits = QueryLimits::default().with_count_mode(QueryCountMode::Exact);
        let validated = ValidQueryRequest::validate_for_client_parts(
            find_transactions_request(params),
            &ALICE_ID,
            &state_view,
            limits,
        )
        .expect("validate exact stored transaction query");
        state_view.kura().reset_merge_query_read_counters_for_test();

        let QueryResponse::Iterable(first) = validated
            .execute_with_replay_state(
                &query_handle,
                &state_view,
                &ALICE_ID,
                Arc::downgrade(&state),
            )
            .expect("start exact stored transaction query")
        else {
            panic!("expected iterable transaction output");
        };
        assert_eq!(first.remaining_items, Some(27));
        let mut collected = transactions_from_batch(first.batch);
        let mut cursor = first.continue_cursor;
        let mut expected_remaining = 27_u64;
        while let Some(current) = cursor {
            let next = query_handle
                .handle_iter_continue(current, &ALICE_ID)
                .expect("continue exact transaction query");
            let page = transactions_from_batch(next.batch);
            expected_remaining = expected_remaining
                .saturating_sub(u64::try_from(page.len()).expect("page length fits u64"));
            assert_eq!(next.remaining_items, Some(expected_remaining));
            collected.extend(page);
            cursor = next.continue_cursor;
        }

        assert_eq!(collected, expected);
        assert_eq!(expected_remaining, 0);
        let (_, complete_scans, indexed_lookups) =
            state_view.kura().merge_query_read_counters_for_test();
        assert_eq!(complete_scans, 0);
        assert_eq!(
            indexed_lookups,
            16 * 2,
            "exact replay validates every carrier once, then resumes through each carrier once"
        );
    }

    #[test]
    fn find_transactions_sorted_prefix_matches_deterministic_full_order_across_pages() {
        use iroha_data_model::query::parameters::{FetchSize, Pagination, Sorting};

        let fixture = crate::smartcontracts::isi::tx::tests::merge_query_fixture();
        let state = Arc::new(fixture.sandbox.state);
        let state_view = state.view();
        let query_handle = state_view.query_handle().clone();
        let mut expected =
            crate::smartcontracts::isi::tx::committed_transactions_snapshot(&state_view)
                .expect("sorted transaction baseline");
        expected.sort_unstable_by(|left, right| left.tiebreak_cmp(right));
        let expected = expected.into_iter().skip(2).take(7).collect::<Vec<_>>();
        let params = QueryParams {
            pagination: Pagination::new(Some(nonzero!(7_u64)), 2),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().expect("metadata key")),
                order: Some(SortOrder::Asc),
            },
            fetch_size: FetchSize::new(Some(nonzero!(3_u64))),
        };
        let limits = QueryLimits::default().with_count_mode(QueryCountMode::Exact);

        let ephemeral = ValidQueryRequest::validate_for_client_parts(
            find_transactions_request(params.clone()),
            &ALICE_ID,
            &state_view,
            limits,
        )
        .expect("validate sorted ephemeral transaction query");
        let QueryResponse::Iterable(first_ephemeral) = ephemeral
            .execute_ephemeral(&query_handle, &state_view, &ALICE_ID)
            .expect("execute sorted ephemeral transaction query")
        else {
            panic!("expected sorted iterable transaction output");
        };
        assert_eq!(first_ephemeral.remaining_items, Some(4));
        assert_eq!(
            transactions_from_batch(first_ephemeral.batch),
            expected[..3]
        );

        let stored = ValidQueryRequest::validate_for_client_parts(
            find_transactions_request(params),
            &ALICE_ID,
            &state_view,
            limits,
        )
        .expect("validate sorted stored transaction query");
        let QueryResponse::Iterable(first) = stored
            .execute_with_replay_state(
                &query_handle,
                &state_view,
                &ALICE_ID,
                Arc::downgrade(&state),
            )
            .expect("start sorted stored transaction query")
        else {
            panic!("expected sorted iterable transaction output");
        };
        let mut collected = transactions_from_batch(first.batch);
        let mut cursor = first.continue_cursor;
        while let Some(current) = cursor {
            let next = query_handle
                .handle_iter_continue(current, &ALICE_ID)
                .expect("continue sorted transaction query");
            collected.extend(transactions_from_batch(next.batch));
            cursor = next.continue_cursor;
        }
        assert_eq!(collected, expected);
    }

    #[test]
    fn find_transactions_exact_replay_fails_closed_on_sidecar_corruption() {
        use iroha_data_model::query::parameters::{FetchSize, Pagination, Sorting};

        let fixture = crate::smartcontracts::isi::tx::tests::merge_query_fixture();
        let target_entry_hash = fixture.target_entry_hash;
        let state = Arc::new(fixture.sandbox.state);
        let state_view = state.view();
        let query_handle = state_view.query_handle().clone();
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(nonzero!(2_u64))),
        };
        let validated = ValidQueryRequest::validate_for_client_parts(
            find_transactions_request(params),
            &ALICE_ID,
            &state_view,
            QueryLimits::default().with_count_mode(QueryCountMode::Exact),
        )
        .expect("validate exact replay corruption query");
        let QueryResponse::Iterable(first) = validated
            .execute_with_replay_state(
                &query_handle,
                &state_view,
                &ALICE_ID,
                Arc::downgrade(&state),
            )
            .expect("start exact replay corruption query")
        else {
            panic!("expected iterable transaction output");
        };
        let mut cursor = first.continue_cursor.expect("exact query continuation");

        state
            .kura()
            .remove_merge_entry_payload_for_test(target_entry_hash);
        loop {
            match query_handle.handle_iter_continue(cursor, &ALICE_ID) {
                Ok(next) => {
                    cursor = next.continue_cursor.expect(
                        "replay must encounter selected corruption before exhausting the cursor",
                    );
                }
                Err(err) => {
                    assert!(matches!(err, Error::Conversion(_)));
                    break;
                }
            }
        }
    }

    #[test]
    fn find_transactions_stored_without_replay_rejects_required_continuation() {
        use iroha_data_model::query::parameters::{FetchSize, Pagination, Sorting};

        let fixture = crate::smartcontracts::isi::tx::tests::merge_query_fixture();
        let state_view = fixture.sandbox.state.view();
        let query_handle = state_view.query_handle().clone();
        let limits = QueryLimits::default().with_count_mode(QueryCountMode::Bounded);
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(nonzero!(2_u64))),
        };
        let validated = ValidQueryRequest::validate_for_client_parts(
            find_transactions_request(params.clone()),
            &ALICE_ID,
            &state_view,
            limits,
        )
        .expect("validate stored transaction query");
        let err = validated
            .execute(&query_handle, &state_view, &ALICE_ID)
            .expect_err("borrowed stored execution cannot safely retain a transaction tail");
        assert!(matches!(err, Error::Conversion(message) if message.contains("replay-capable")));

        let terminal_params = QueryParams {
            pagination: Pagination::new(Some(nonzero!(2_u64)), 0),
            ..params
        };
        let terminal = ValidQueryRequest::validate_for_client_parts(
            find_transactions_request(terminal_params),
            &ALICE_ID,
            &state_view,
            limits,
        )
        .expect("validate terminal stored transaction query");
        let QueryResponse::Iterable(output) = terminal
            .execute(&query_handle, &state_view, &ALICE_ID)
            .expect("one-page bounded query does not need replay state")
        else {
            panic!("expected iterable transaction output");
        };
        assert_eq!(transactions_from_batch(output.batch).len(), 2);
        assert!(!output.has_more);
        assert!(output.continue_cursor.is_none());
    }

    #[test]
    fn find_transactions_bounded_defers_old_corruption_but_exact_fails() {
        use iroha_data_model::query::parameters::{FetchSize, Pagination, Sorting};

        let fixture = crate::smartcontracts::isi::tx::tests::merge_query_fixture();
        fixture
            .sandbox
            .state
            .kura()
            .remove_merge_entry_payload_for_test(fixture.unrelated_entry_hash);
        let state_view = fixture.sandbox.state.view();
        let query_handle = state_view.query_handle().clone();
        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::new(Some(nonzero!(2_u64))),
        };

        let bounded = ValidQueryRequest::validate_for_client_parts(
            find_transactions_request(params.clone()),
            &ALICE_ID,
            &state_view,
            QueryLimits::default().with_count_mode(QueryCountMode::Bounded),
        )
        .expect("validate bounded query");
        let QueryResponse::Iterable(output) = bounded
            .execute_ephemeral(&query_handle, &state_view, &ALICE_ID)
            .expect("bounded first page avoids corrupt oldest carrier")
        else {
            panic!("expected bounded transaction output");
        };
        assert_eq!(transactions_from_batch(output.batch).len(), 2);

        let exact = ValidQueryRequest::validate_for_client_parts(
            find_transactions_request(params),
            &ALICE_ID,
            &state_view,
            QueryLimits::default().with_count_mode(QueryCountMode::Exact),
        )
        .expect("validate exact query");
        let err = exact
            .execute_ephemeral(&query_handle, &state_view, &ALICE_ID)
            .expect_err("exact query must validate corrupt selected history");
        assert!(matches!(err, Error::Conversion(_)));
    }

    #[test]
    fn find_transactions_rejects_unbounded_or_oversized_sorted_prefix() {
        use iroha_data_model::query::parameters::{FetchSize, Pagination, Sorting};

        let fixture = crate::smartcontracts::isi::tx::tests::merge_query_fixture();
        let state_view = fixture.sandbox.state.view();
        let query_handle = state_view.query_handle().clone();
        let sorted = Sorting {
            sort_by_metadata_key: Some("rank".parse().expect("metadata key")),
            order: Some(SortOrder::Asc),
        };
        let limits = QueryLimits::default().with_count_mode(QueryCountMode::Bounded);

        for pagination in [
            Pagination::default(),
            Pagination::new(Some(nonzero!(4_097_u64)), 0),
            Pagination::new(Some(nonzero!(1_u64)), 4_096),
        ] {
            let params = QueryParams {
                pagination,
                sorting: sorted.clone(),
                fetch_size: FetchSize::new(Some(nonzero!(1_u64))),
            };
            let validated = ValidQueryRequest::validate_for_client_parts(
                find_transactions_request(params),
                &ALICE_ID,
                &state_view,
                limits,
            )
            .expect("materialization budget is enforced during execution");
            let err = validated
                .execute_ephemeral(&query_handle, &state_view, &ALICE_ID)
                .expect_err("unsafe sorted prefix must be rejected");
            assert_eq!(err, Error::GasBudgetExceeded);
        }
    }

    #[tokio::test]
    async fn find_transactions_by_block_hash_uses_block_index() -> Result<()> {
        let state = state_with_test_blocks_and_transactions(8, 1, 1)?;
        let state_view = state.view();
        let block = state_view
            .kura()
            .get_block(nonzero!(4_usize))
            .expect("block available");
        let block_hash = block.hash();

        let txs = ValidQuery::execute(
            FindTransactions,
            CompoundPredicate::<iroha_data_model::query::CommittedTransaction>::build(|p| {
                p.equals("block_hash", block_hash)
            }),
            &state_view,
        )?
        .collect::<Vec<_>>();

        assert_eq!(txs.len(), 2);
        assert!(txs.iter().all(|tx| tx.block_hash == block_hash));
        assert_eq!(
            txs.iter().map(|tx| tx.entrypoint_hash).collect::<Vec<_>>(),
            block.entrypoint_hashes().rev().collect::<Vec<_>>()
        );

        let unknown_hash =
            iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                Hash::new("missing block"),
            );
        let missing = ValidQuery::execute(
            FindTransactions,
            CompoundPredicate::<iroha_data_model::query::CommittedTransaction>::build(|p| {
                p.equals("block_hash", unknown_hash)
            }),
            &state_view,
        )?
        .collect::<Vec<_>>();

        assert!(missing.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn find_transactions_by_entrypoint_hash_uses_kura_index() -> Result<()> {
        let num_blocks = 8;
        let state = state_with_test_blocks_and_transactions(num_blocks, 1, 1)?;
        let state_view = state.view();
        let block = state_view
            .kura()
            .get_block(nonzero!(4_usize))
            .expect("block available");
        let entrypoint_hash = block
            .entrypoint_hashes()
            .next()
            .expect("test block has transactions");
        let indexed_heights = state_view
            .kura()
            .get_block_heights_by_entrypoint_hash(entrypoint_hash)
            .expect("test Kura transaction index is complete");

        assert_eq!(
            indexed_heights,
            (1..=num_blocks)
                .filter_map(|height| std::num::NonZeroUsize::new(height as usize))
                .collect()
        );

        let txs = ValidQuery::execute(
            FindTransactions,
            CompoundPredicate::<iroha_data_model::query::CommittedTransaction>::build(|p| {
                p.equals("entrypoint_hash", entrypoint_hash)
            }),
            &state_view,
        )?
        .collect::<Vec<_>>();

        assert_eq!(txs.len() as u64, num_blocks);
        assert!(txs.iter().all(|tx| tx.entrypoint_hash == entrypoint_hash));
        assert_eq!(
            txs.iter().map(|tx| tx.block_hash).collect::<Vec<_>>(),
            (1..=num_blocks)
                .rev()
                .map(|height| {
                    state_view
                        .kura()
                        .get_block(std::num::NonZeroUsize::new(height as usize).unwrap())
                        .expect("block available")
                        .hash()
                })
                .collect::<Vec<_>>()
        );

        let unknown_hash = iroha_crypto::HashOf::<
            iroha_data_model::transaction::signed::TransactionEntrypoint,
        >::from_untyped_unchecked(Hash::new(
            "missing transaction entrypoint",
        ));
        let missing = ValidQuery::execute(
            FindTransactions,
            CompoundPredicate::<iroha_data_model::query::CommittedTransaction>::build(|p| {
                p.equals("entrypoint_hash", unknown_hash)
            }),
            &state_view,
        )?
        .collect::<Vec<_>>();

        assert!(missing.is_empty());

        state_view
            .kura()
            .prune_to_height(4)
            .expect("prune test blocks");
        assert_eq!(
            state_view
                .kura()
                .get_block_heights_by_entrypoint_hash(entrypoint_hash)
                .expect("test Kura transaction index is complete"),
            (1..=4)
                .filter_map(|height| std::num::NonZeroUsize::new(height as usize))
                .collect()
        );

        Ok(())
    }

    #[tokio::test]
    async fn find_transactions_by_authority_timestamp_and_result_use_kura_indexes() -> Result<()> {
        let num_blocks = 8;
        let state = state_with_test_blocks_and_transactions(num_blocks, 1, 1)?;
        let state_view = state.view();
        let all_txs = ValidQuery::execute(FindTransactions, CompoundPredicate::PASS, &state_view)?
            .collect::<Vec<_>>();
        let first_tx = all_txs.first().expect("test state has transactions");
        let authority = first_tx
            .entrypoint
            .authority_opt()
            .expect("test transaction has authority")
            .clone();
        let timestamp_ms = first_tx
            .entrypoint
            .creation_time_ms()
            .expect("test transaction has timestamp");

        assert_eq!(
            state_view
                .kura()
                .get_block_heights_by_transaction_authority(&authority)
                .expect("test Kura transaction index is complete")
                .len() as u64,
            num_blocks
        );
        assert_eq!(
            state_view
                .kura()
                .get_block_heights_by_transaction_timestamp_ms(timestamp_ms)
                .expect("test Kura transaction index is complete")
                .len() as u64,
            num_blocks
        );
        assert_eq!(
            state_view
                .kura()
                .get_block_heights_by_transaction_result_status(false)
                .expect("test Kura transaction index is complete")
                .len() as u64,
            num_blocks
        );

        let by_authority = ValidQuery::execute(
            FindTransactions,
            CompoundPredicate::<iroha_data_model::query::CommittedTransaction>::build(|p| {
                p.equals("authority", authority.to_string())
            }),
            &state_view,
        )?
        .collect::<Vec<_>>();
        let expected_by_authority = all_txs
            .iter()
            .filter(|tx| tx.entrypoint.authority_opt() == Some(&authority))
            .count();
        assert_eq!(by_authority.len(), expected_by_authority);
        assert!(
            by_authority
                .iter()
                .all(|tx| tx.entrypoint.authority_opt() == Some(&authority))
        );

        let by_timestamp = ValidQuery::execute(
            FindTransactions,
            CompoundPredicate::<iroha_data_model::query::CommittedTransaction>::build(|p| {
                p.equals("timestamp_ms", timestamp_ms)
            }),
            &state_view,
        )?
        .collect::<Vec<_>>();
        let expected_by_timestamp = all_txs
            .iter()
            .filter(|tx| tx.entrypoint.creation_time_ms() == Some(timestamp_ms))
            .count();
        assert_eq!(by_timestamp.len(), expected_by_timestamp);
        assert!(
            by_timestamp
                .iter()
                .all(|tx| tx.entrypoint.creation_time_ms() == Some(timestamp_ms))
        );

        let failed = ValidQuery::execute(
            FindTransactions,
            CompoundPredicate::<iroha_data_model::query::CommittedTransaction>::build(|p| {
                p.equals("result_ok", false)
            }),
            &state_view,
        )?
        .collect::<Vec<_>>();
        assert_eq!(failed.len() as u64, num_blocks);
        assert!(failed.iter().all(|tx| tx.result.as_ref().is_err()));

        let missing_authority = ValidQuery::execute(
            FindTransactions,
            CompoundPredicate::<iroha_data_model::query::CommittedTransaction>::build(|p| {
                p.equals("authority", BOB_ID.to_string())
            }),
            &state_view,
        )?
        .collect::<Vec<_>>();
        assert!(missing_authority.is_empty());

        let contradictory_authority = ValidQuery::execute(
            FindTransactions,
            CompoundPredicate::<iroha_data_model::query::CommittedTransaction>::build(|p| {
                p.equals("authority", authority.to_string())
                    .equals("authority", BOB_ID.to_string())
            }),
            &state_view,
        )?
        .collect::<Vec<_>>();
        assert!(contradictory_authority.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn find_transactions_by_filter_timestamp_range_uses_kura_index() -> Result<()> {
        let num_blocks = 8;
        let state = state_with_test_blocks_and_transactions(num_blocks, 1, 1)?;
        let state_view = state.view();
        let all_txs = ValidQuery::execute(FindTransactions, CompoundPredicate::PASS, &state_view)?
            .collect::<Vec<_>>();
        let first_tx = all_txs.first().expect("test state has transactions");
        let timestamp_ms = first_tx
            .entrypoint
            .creation_time_ms()
            .expect("test transaction has timestamp");
        let result_ok = first_tx.result.as_ref().is_ok();

        let expected_heights = all_txs
            .iter()
            .filter(|tx| tx.entrypoint.creation_time_ms() == Some(timestamp_ms))
            .filter_map(|tx| state_view.kura().get_block_height_by_hash(tx.block_hash))
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            state_view
                .kura()
                .get_block_heights_by_transaction_timestamp_range(
                    Some(timestamp_ms),
                    Some(timestamp_ms)
                )
                .expect("test Kura transaction index is complete"),
            expected_heights
        );

        let by_timestamp_range = ValidQuery::execute(
            FindTransactions,
            CompoundPredicate::<iroha_data_model::query::CommittedTransaction>::from_filters(
                iroha_data_model::query::CommittedTxFilters {
                    ts_ge: Some(timestamp_ms),
                    ts_le: Some(timestamp_ms),
                    ..Default::default()
                },
            ),
            &state_view,
        )?
        .collect::<Vec<_>>();
        let expected_by_range = all_txs
            .iter()
            .filter(|tx| tx.entrypoint.creation_time_ms() == Some(timestamp_ms))
            .count();
        assert_eq!(by_timestamp_range.len(), expected_by_range);
        assert!(
            by_timestamp_range
                .iter()
                .all(|tx| tx.entrypoint.creation_time_ms() == Some(timestamp_ms))
        );

        let by_timestamp_and_result = ValidQuery::execute(
            FindTransactions,
            CompoundPredicate::<iroha_data_model::query::CommittedTransaction>::from_filters(
                iroha_data_model::query::CommittedTxFilters {
                    ts_ge: Some(timestamp_ms),
                    ts_le: Some(timestamp_ms),
                    result_ok: Some(result_ok),
                    ..Default::default()
                },
            ),
            &state_view,
        )?
        .collect::<Vec<_>>();
        let expected_by_timestamp_and_result = all_txs
            .iter()
            .filter(|tx| {
                tx.entrypoint.creation_time_ms() == Some(timestamp_ms)
                    && tx.result.as_ref().is_ok() == result_ok
            })
            .count();
        assert_eq!(
            by_timestamp_and_result.len(),
            expected_by_timestamp_and_result
        );
        assert!(by_timestamp_and_result.iter().all(|tx| {
            tx.entrypoint.creation_time_ms() == Some(timestamp_ms)
                && tx.result.as_ref().is_ok() == result_ok
        }));

        let impossible_lower_bound = timestamp_ms + 1;
        assert!(
            state_view
                .kura()
                .get_block_heights_by_transaction_timestamp_range(
                    Some(impossible_lower_bound),
                    Some(timestamp_ms)
                )
                .expect("test Kura transaction index is complete")
                .is_empty()
        );
        let impossible_range = ValidQuery::execute(
            FindTransactions,
            CompoundPredicate::<iroha_data_model::query::CommittedTransaction>::from_filters(
                iroha_data_model::query::CommittedTxFilters {
                    ts_ge: Some(impossible_lower_bound),
                    ts_le: Some(timestamp_ms),
                    ..Default::default()
                },
            ),
            &state_view,
        )?
        .collect::<Vec<_>>();
        assert!(impossible_range.is_empty());

        assert_eq!(expected_heights.len() as u64, num_blocks);

        Ok(())
    }

    #[tokio::test]
    async fn find_proof_records_intersects_backend_and_status_indexes() -> Result<()> {
        fn proof_record(
            backend: &str,
            proof_byte: u8,
            status: iroha_data_model::proof::ProofStatus,
        ) -> iroha_data_model::proof::ProofRecord {
            iroha_data_model::proof::ProofRecord {
                id: iroha_data_model::proof::ProofId {
                    backend: backend.into(),
                    proof_hash: [proof_byte; 32],
                },
                vk_ref: None,
                vk_commitment: None,
                status,
                verified_at_height: Some(1),
                bridge: None,
            }
        }

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let mut state = State::new_for_testing(World::default(), kura, query_handle);
        let target = proof_record(
            "halo2/test",
            1,
            iroha_data_model::proof::ProofStatus::Verified,
        );
        let same_backend_wrong_status = proof_record(
            "halo2/test",
            2,
            iroha_data_model::proof::ProofStatus::Rejected,
        );
        let wrong_backend_same_status = proof_record(
            "stark/test",
            3,
            iroha_data_model::proof::ProofStatus::Verified,
        );
        let mut proof_status_index = std::collections::BTreeMap::<
            iroha_data_model::proof::ProofStatus,
            std::collections::BTreeSet<iroha_data_model::proof::ProofId>,
        >::new();
        for record in [
            target.clone(),
            same_backend_wrong_status.clone(),
            wrong_backend_same_status.clone(),
        ] {
            state.world.proofs.insert(record.id.clone(), record.clone());
            proof_status_index
                .entry(record.status)
                .or_default()
                .insert(record.id);
        }
        for (status, proof_ids) in proof_status_index {
            state.world.proofs_by_status.insert(status, proof_ids);
        }

        let state_view = state.view();
        assert_eq!(
            state_view
                .world()
                .proofs_by_backend_iter("halo2/test")
                .count(),
            2,
            "fixture should populate the backend range used by the query planner",
        );

        let matching = ValidQuery::execute(
            iroha_data_model::query::proof::prelude::FindProofRecords,
            CompoundPredicate::<iroha_data_model::proof::ProofRecord>::build(|p| {
                p.equals("backend", "halo2/test")
                    .equals("status", iroha_data_model::proof::ProofStatus::Verified)
            }),
            &state_view,
        )?
        .collect::<Vec<_>>();
        assert_eq!(matching, vec![target.clone()]);

        let backend_only = ValidQuery::execute(
            iroha_data_model::query::proof::prelude::FindProofRecordsByBackend {
                backend: "halo2/test".into(),
            },
            CompoundPredicate::<iroha_data_model::proof::ProofRecord>::PASS,
            &state_view,
        )?
        .collect::<Vec<_>>();
        assert_eq!(
            backend_only,
            vec![target.clone(), same_backend_wrong_status.clone()],
            "backend-specific proof query must not leak records from another backend",
        );

        let missing_backend = ValidQuery::execute(
            iroha_data_model::query::proof::prelude::FindProofRecordsByBackend {
                backend: "missing/backend".into(),
            },
            CompoundPredicate::<iroha_data_model::proof::ProofRecord>::PASS,
            &state_view,
        )?
        .collect::<Vec<_>>();
        assert!(missing_backend.is_empty());

        let contradictory_backend = ValidQuery::execute(
            iroha_data_model::query::proof::prelude::FindProofRecords,
            CompoundPredicate::<iroha_data_model::proof::ProofRecord>::build(|p| {
                p.equals("backend", "halo2/test")
                    .equals("backend", "stark/test")
            }),
            &state_view,
        )?
        .collect::<Vec<_>>();
        assert!(contradictory_backend.is_empty());

        Ok(())
    }

    #[cfg(feature = "ids_projection")]
    #[tokio::test]
    async fn iter_dispatch_domains_ids_only_projection() {
        use iroha_data_model::query::{
            self, QueryItemKind, QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
        };

        // Build world with two domains and ALICE account
        let d1: Domain =
            Domain::new(DomainId::try_new("w1", "universal").unwrap()).build(&ALICE_ID);
        let d2: Domain =
            Domain::new(DomainId::try_new("w2", "universal").unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let world = World::with([d1.clone(), d2.clone()], [account], []);

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle.clone());
        let state_view = state.view();

        let qwp = QueryWithParams {
            query: (),
            query_payload: Vec::new(),
            item: QueryItemKind::Domain,
            predicate_bytes: norito::codec::Encode::encode(&CompoundPredicate::<Domain>::PASS),
            selector_bytes: norito::codec::Encode::encode(&SelectorTuple::<Domain>::ids_only()),
            params: query::parameters::QueryParams::default(),
        };
        let req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(qwp),
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            req.execute(&query_handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, _rem, _cur) = first.into_parts();
        let ids = match batch.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::DomainId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().any(|id| id == d1.id()));
        assert!(ids.iter().any(|id| id == d2.id()));
    }

    #[cfg(feature = "ids_projection")]
    #[tokio::test]
    async fn iter_dispatch_accounts_ids_only_projection() {
        use iroha_data_model::query::{
            self, QueryItemKind, QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
        };

        let w: Domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let (a_id, _) = iroha_test_samples::gen_account_in("w");
        let (b_id, _) = iroha_test_samples::gen_account_in("w");
        let a = Account::new(a_id.clone()).build(&a_id);
        let b = Account::new(b_id.clone()).build(&b_id);
        let world = World::with([w], [a.clone(), b.clone()], []);

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle.clone());
        let state_view = state.view();

        let qwp = QueryWithParams {
            query: (),
            item: QueryItemKind::Account,
            predicate_bytes: norito::codec::Encode::encode(&CompoundPredicate::<Account>::PASS),
            selector_bytes: norito::codec::Encode::encode(&SelectorTuple::<Account>::ids_only()),
            params: query::parameters::QueryParams::default(),
        };
        let req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(qwp),
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            req.execute(&query_handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, _rem, _cur) = first.into_parts();
        let ids = match batch.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::AccountId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().any(|id| id == &a_id));
        assert!(ids.iter().any(|id| id == &b_id));
    }

    #[cfg(feature = "ids_projection")]
    #[tokio::test]
    async fn iter_dispatch_asset_definitions_ids_only_projection() {
        use iroha_data_model::query::{
            self, QueryItemKind, QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
        };

        let domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let ad1 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "rose".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let ad2 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "tulip".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let world = World::with([domain], [account], [ad1.clone(), ad2.clone()]);

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle.clone());
        let state_view = state.view();

        let qwp = QueryWithParams {
            query: (),
            item: QueryItemKind::AssetDefinition,
            predicate_bytes: norito::codec::Encode::encode(
                &CompoundPredicate::<AssetDefinition>::PASS,
            ),
            selector_bytes: norito::codec::Encode::encode(
                &SelectorTuple::<AssetDefinition>::ids_only(),
            ),
            params: query::parameters::QueryParams::default(),
        };
        let req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(qwp),
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            req.execute(&query_handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, _rem, _cur) = first.into_parts();
        let ids = match batch.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::AssetDefinitionId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().any(|id| id == ad1.id()));
        assert!(ids.iter().any(|id| id == ad2.id()));
    }

    #[cfg(feature = "ids_projection")]
    #[tokio::test]
    async fn iter_dispatch_nfts_ids_only_projection() {
        use iroha_data_model::query::{
            self, QueryBox, QueryWithFilter, QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
        };

        let domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let nft1 =
            Nft::new("n1$w.universal".parse().unwrap(), Metadata::default()).build(&ALICE_ID);
        let nft2 =
            Nft::new("n2$w.universal".parse().unwrap(), Metadata::default()).build(&ALICE_ID);
        let world = World::with_assets([domain], [account], [], [], [nft1.clone(), nft2.clone()]);

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle.clone());
        let state_view = state.view();

        let qwf: QueryWithFilter<_> = QueryWithFilter::new(
            {
                #[cfg(not(feature = "fast_dsl"))]
                {
                    Box::new(query::nft::prelude::FindNfts)
                }
                #[cfg(feature = "fast_dsl")]
                {
                    ()
                }
            },
            CompoundPredicate::PASS,
            SelectorTuple::<Nft>::ids_only(),
        );
        let qbox: QueryBox<query::QueryOutputBatchBox> = qwf.into();
        let qwp = QueryWithParams::new(qbox, query::parameters::QueryParams::default());
        let req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(qwp),
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            req.execute(&query_handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, _rem, _cur) = first.into_parts();
        let ids = match batch.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::NftId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().any(|id| id == nft1.id()));
        assert!(ids.iter().any(|id| id == nft2.id()));
    }

    #[cfg(feature = "ids_projection")]
    #[tokio::test]
    async fn iter_dispatch_roles_ids_only_projection() {
        use iroha_data_model::query::{
            self, QueryBox, QueryWithFilter, QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
        };

        // Create a role and store it in world
        let domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let role1 = Role::new("r1".parse().unwrap(), ALICE_ID.clone()).build(&ALICE_ID);
        let role2 = Role::new("r2".parse().unwrap(), ALICE_ID.clone()).build(&ALICE_ID);
        let world = {
            let mut w = World::with(
                [domain],
                [Account::new(ALICE_ID.clone()).build(&ALICE_ID)],
                [],
            );
            let mut block = w.block();
            // Insert roles via the world roles map (simulate registration)
            block.roles.insert(role1.id().clone(), role1.clone());
            block.roles.insert(role2.id().clone(), role2.clone());
            block.commit();
            w
        };

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle.clone());
        let state_view = state.view();

        let qwf: QueryWithFilter<_> = QueryWithFilter::new(
            {
                #[cfg(not(feature = "fast_dsl"))]
                {
                    Box::new(query::role::prelude::FindRoles)
                }
                #[cfg(feature = "fast_dsl")]
                {
                    ()
                }
            },
            CompoundPredicate::PASS,
            SelectorTuple::<Role>::ids_only(),
        );
        let qbox: QueryBox<query::QueryOutputBatchBox> = qwf.into();
        let qwp = QueryWithParams::new(qbox, query::parameters::QueryParams::default());
        let req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(qwp),
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            req.execute(&query_handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, _rem, _cur) = first.into_parts();
        let ids = match batch.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::RoleId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().any(|id| id == role1.id()));
        assert!(ids.iter().any(|id| id == role2.id()));
    }

    #[cfg(feature = "ids_projection")]
    #[tokio::test]
    async fn iter_dispatch_triggers_ids_only_projection() {
        use iroha_data_model::{
            events::time::{ExecutionTime, TimeEventFilter},
            query::{
                self, QueryBox, QueryWithFilter, QueryWithParams,
                dsl::{CompoundPredicate, SelectorTuple},
            },
        };

        let domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let mut world = World::with([domain], [account], []);
        // Add 2 time triggers
        {
            let mut block = world.triggers.block();
            let mut tx = block.transaction();
            let action = Action::new(
                [Log::new(iroha_logger::Level::INFO, "x".into())],
                Repeats::Indefinitely,
                ALICE_ID.clone(),
                TimeEventFilter::new(ExecutionTime::PreCommit),
            );
            let t1 = Trigger::new("t1".parse().unwrap(), action.clone())
                .try_into()
                .unwrap();
            let t2 = Trigger::new("t2".parse().unwrap(), action)
                .try_into()
                .unwrap();
            tx.add_time_trigger(t1).unwrap();
            tx.add_time_trigger(t2).unwrap();
            tx.apply();
            block.commit();
        }

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle.clone());
        let state_view = state.view();

        let qwf: QueryWithFilter<_> = QueryWithFilter::new(
            {
                #[cfg(not(feature = "fast_dsl"))]
                {
                    Box::new(query::trigger::prelude::FindTriggers)
                }
                #[cfg(feature = "fast_dsl")]
                {
                    ()
                }
            },
            CompoundPredicate::PASS,
            SelectorTuple::<Trigger>::ids_only(),
        );
        let qbox: QueryBox<query::QueryOutputBatchBox> = qwf.into();
        let qwp = QueryWithParams::new(qbox, query::parameters::QueryParams::default());
        let req = ValidQueryRequest::validate_for_client_parts(
            QueryRequest::Start(qwp),
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            req.execute(&query_handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, _rem, _cur) = first.into_parts();
        let ids = match batch.into_iter().next().expect("slice") {
            query::QueryOutputBatchBox::TriggerId(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().any(|id| id == &"t1".parse().unwrap()));
        assert!(ids.iter().any(|id| id == &"t2".parse().unwrap()));
    }

    #[tokio::test]
    async fn iter_dispatch_asset_definitions_sort_asc() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{
            FetchSize, Pagination, QueryParams, SortOrder, Sorting,
        };
        use iroha_futures::supervisor::ShutdownSignal;

        let domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let mut ad1 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "rose".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let mut ad2 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "tulip".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let ad3 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "peony".parse().unwrap(),
        ))
        .build(&ALICE_ID); // no rank
        ad1.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
        ad2.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
        let world = World::with([domain], [account], [ad1.clone(), ad2.clone(), ad3.clone()]);

        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());
        let state_view = state.view();

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(SortOrder::Asc),
            },
            fetch_size: FetchSize::default(),
        };

        let payload = norito::codec::Encode::encode(
            &iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
        );
        let qbox: iroha_data_model::query::QueryBox<_> = Box::new(
            iroha_data_model::query::ErasedIterQuery::<AssetDefinition>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<AssetDefinition>::PASS,
                SelectorTuple::<AssetDefinition>::default(),
                payload,
            ),
        );
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, _rem, _cur) = first.into_parts();
        let v = match batch.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::AssetDefinition(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].id(), ad1.id());
        assert_eq!(v[1].id(), ad2.id());
        assert_eq!(v[2].id(), ad3.id());
    }

    #[tokio::test]
    async fn iter_dispatch_accounts_sort_asc_end_to_end() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{
            FetchSize, Pagination, QueryParams, SortOrder, Sorting,
        };
        use iroha_futures::supervisor::ShutdownSignal;
        use iroha_primitives::json::Json;

        let w: Domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let (a_id, _) = iroha_test_samples::gen_account_in("w");
        let (b_id, _) = iroha_test_samples::gen_account_in("w");
        let (c_id, _) = iroha_test_samples::gen_account_in("w");

        let a = Account::new(a_id.clone())
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
                m
            })
            .build(&a_id);
        let b = Account::new(b_id.clone())
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
                m
            })
            .build(&b_id);
        let c = Account::new(c_id.clone()).build(&c_id);

        let world = World::with([w], [a.clone(), b.clone(), c.clone()], []);
        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());
        let state_view = state.view();

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(SortOrder::Asc),
            },
            fetch_size: FetchSize::default(),
        };

        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::account::prelude::FindAccounts);
        let qbox: iroha_data_model::query::QueryBox<_> =
            Box::new(iroha_data_model::query::ErasedIterQuery::<Account>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<Account>::PASS,
                SelectorTuple::<Account>::default(),
                payload,
            ));
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch, _rem, _cur) = first.into_parts();
        let v = match batch.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Account(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].id(), &b_id);
        assert_eq!(v[1].id(), &a_id);
        assert_eq!(v[2].id(), &c_id);
    }

    #[tokio::test]
    async fn iter_dispatch_accounts_sort_desc_batched() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{
            FetchSize, Pagination, QueryParams, SortOrder, Sorting,
        };
        use iroha_futures::supervisor::ShutdownSignal;
        use iroha_primitives::json::Json;
        use nonzero_ext::nonzero;

        let w: Domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let (a_id, _) = iroha_test_samples::gen_account_in("w");
        let (b_id, _) = iroha_test_samples::gen_account_in("w");
        let (c_id, _) = iroha_test_samples::gen_account_in("w");

        let a = Account::new(a_id.clone())
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
                m
            })
            .build(&a_id);
        let b = Account::new(b_id.clone())
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
                m
            })
            .build(&b_id);
        let c = Account::new(c_id.clone()).build(&c_id);

        let world = World::with([w], [a.clone(), b.clone(), c.clone()], []);
        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());
        let state_view = state.view();

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(SortOrder::Desc),
            },
            fetch_size: FetchSize::new(Some(nonzero!(2_u64))),
        };

        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::account::prelude::FindAccounts);
        let qbox: iroha_data_model::query::QueryBox<_> =
            Box::new(iroha_data_model::query::ErasedIterQuery::<Account>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<Account>::PASS,
                SelectorTuple::<Account>::default(),
                payload,
            ));
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch1, remaining, cursor) = first.into_parts();
        let v1 = match batch1.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Account(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v1.len(), 2);
        assert_eq!(v1[0].id(), &a_id);
        assert_eq!(v1[1].id(), &b_id);
        assert_eq!(remaining, 1);
        let cursor = cursor.expect("should continue");

        let next = handle.handle_iter_continue(cursor, &ALICE_ID).unwrap();
        let (batch2, remaining2, cursor2) = next.into_parts();
        let v2 = match batch2.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Account(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v2.len(), 1);
        assert_eq!(v2[0].id(), &c_id);
        assert_eq!(remaining2, 0);
        assert!(cursor2.is_none());
    }

    #[tokio::test]
    async fn iter_dispatch_asset_definitions_sort_desc_batched() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{
            FetchSize, Pagination, QueryParams, SortOrder, Sorting,
        };
        use iroha_futures::supervisor::ShutdownSignal;
        use nonzero_ext::nonzero;

        let domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let mut ad1 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "rose".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let mut ad2 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "tulip".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let ad3 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "peony".parse().unwrap(),
        ))
        .build(&ALICE_ID); // no rank
        ad1.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
        ad2.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
        let world = World::with([domain], [account], [ad1.clone(), ad2.clone(), ad3.clone()]);

        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());
        let state_view = state.view();

        let params = QueryParams {
            pagination: Pagination::default(),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(SortOrder::Desc),
            },
            fetch_size: FetchSize::new(Some(nonzero!(2_u64))),
        };

        let payload = norito::codec::Encode::encode(
            &iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
        );
        let qbox: iroha_data_model::query::QueryBox<_> = Box::new(
            iroha_data_model::query::ErasedIterQuery::<AssetDefinition>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<AssetDefinition>::PASS,
                SelectorTuple::<AssetDefinition>::default(),
                payload,
            ),
        );
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch1, remaining, cursor) = first.into_parts();
        let v1 = match batch1.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::AssetDefinition(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v1.len(), 2);
        assert_eq!(v1[0].id(), ad2.id());
        assert_eq!(v1[1].id(), ad1.id());
        assert_eq!(remaining, 1);
        let cursor = cursor.expect("should continue");

        let next = handle.handle_iter_continue(cursor, &ALICE_ID).unwrap();
        let (batch2, remaining2, cursor2) = next.into_parts();
        let v2 = match batch2.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::AssetDefinition(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v2.len(), 1);
        assert_eq!(v2[0].id(), ad3.id());
        assert_eq!(remaining2, 0);
        assert!(cursor2.is_none());
    }

    #[tokio::test]
    async fn iter_dispatch_asset_definitions_offset_and_fetch_size_interplay_asc() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{
            FetchSize, Pagination, QueryParams, SortOrder, Sorting,
        };
        use iroha_futures::supervisor::ShutdownSignal;
        use iroha_primitives::json::Json;
        use nonzero_ext::nonzero;

        // Build three asset definitions with rank metadata: 0,1,2
        let domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let mut ad0 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "a0".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let mut ad1 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "a1".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let mut ad2 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "a2".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        ad0.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(0)));
        ad1.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
        ad2.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
        let world = World::with([domain], [account], [ad0.clone(), ad1.clone(), ad2.clone()]);

        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());
        let state_view = state.view();

        // Asc by rank, offset=1, limit=2, fetch_size=1 => expect a1 then a2
        let params = QueryParams {
            pagination: Pagination::new(Some(nonzero!(2_u64)), 1),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(SortOrder::Asc),
            },
            fetch_size: FetchSize::new(Some(nonzero!(1_u64))),
        };

        let payload = norito::codec::Encode::encode(
            &iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
        );
        let qbox: iroha_data_model::query::QueryBox<_> = Box::new(
            iroha_data_model::query::ErasedIterQuery::<AssetDefinition>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<AssetDefinition>::PASS,
                SelectorTuple::<AssetDefinition>::default(),
                payload,
            ),
        );
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch1, remaining, cursor) = first.into_parts();
        let v1 = match batch1.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::AssetDefinition(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v1.len(), 1);
        assert_eq!(v1[0].id(), ad1.id());
        assert_eq!(remaining, 1);

        let cursor = cursor.expect("should continue");
        let next = handle.handle_iter_continue(cursor, &ALICE_ID).unwrap();
        let (batch2, remaining2, cursor2) = next.into_parts();
        let v2 = match batch2.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::AssetDefinition(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v2.len(), 1);
        assert_eq!(v2[0].id(), ad2.id());
        assert_eq!(remaining2, 0);
        assert!(cursor2.is_none());
    }

    #[tokio::test]
    async fn iter_dispatch_asset_definitions_offset_and_fetch_size_interplay_desc() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{
            FetchSize, Pagination, QueryParams, SortOrder, Sorting,
        };
        use iroha_futures::supervisor::ShutdownSignal;
        use iroha_primitives::json::Json;
        use nonzero_ext::nonzero;

        // Build three asset definitions with rank metadata: 0,1,2
        let domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone()).build(&ALICE_ID);
        let mut ad0 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "a0".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let mut ad1 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "a1".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let mut ad2 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("w", "universal").unwrap(),
            "a2".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        ad0.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(0)));
        ad1.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
        ad2.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
        let world = World::with([domain], [account], [ad0.clone(), ad1.clone(), ad2.clone()]);

        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());
        let state_view = state.view();

        // Desc by rank: list is [2,1,0]; offset=1 -> start from rank=1; limit=2 -> ranks [1,0]; fetch_size=1 -> first rank=1, then rank=0
        let params = QueryParams {
            pagination: Pagination::new(Some(nonzero!(2_u64)), 1),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(SortOrder::Desc),
            },
            fetch_size: FetchSize::new(Some(nonzero!(1_u64))),
        };

        let payload = norito::codec::Encode::encode(
            &iroha_data_model::query::asset::prelude::FindAssetsDefinitions,
        );
        let qbox: iroha_data_model::query::QueryBox<_> = Box::new(
            iroha_data_model::query::ErasedIterQuery::<AssetDefinition>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<AssetDefinition>::PASS,
                SelectorTuple::<AssetDefinition>::default(),
                payload,
            ),
        );
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch1, remaining, cursor) = first.into_parts();
        let v1 = match batch1.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::AssetDefinition(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v1.len(), 1);
        assert_eq!(v1[0].id(), ad1.id());
        assert_eq!(remaining, 1);

        let cursor = cursor.expect("should continue");
        let next = handle.handle_iter_continue(cursor, &ALICE_ID).unwrap();
        let (batch2, remaining2, cursor2) = next.into_parts();
        let v2 = match batch2.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::AssetDefinition(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v2.len(), 1);
        assert_eq!(v2[0].id(), ad0.id());
        assert_eq!(remaining2, 0);
        assert!(cursor2.is_none());
    }

    #[tokio::test]
    async fn iter_dispatch_accounts_offset_and_fetch_size_interplay() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{
            FetchSize, Pagination, QueryParams, SortOrder, Sorting,
        };
        use iroha_futures::supervisor::ShutdownSignal;
        use iroha_primitives::json::Json;
        use nonzero_ext::nonzero;

        // Build three accounts with explicit rank metadata: a(0), b(1), c(2)
        let w: Domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let (a_id, _) = iroha_test_samples::gen_account_in("w");
        let (b_id, _) = iroha_test_samples::gen_account_in("w");
        let (c_id, _) = iroha_test_samples::gen_account_in("w");

        let a = Account::new(a_id.clone())
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(0)));
                m
            })
            .build(&a_id);
        let b = Account::new(b_id.clone())
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
                m
            })
            .build(&b_id);
        let c = Account::new(c_id.clone())
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
                m
            })
            .build(&c_id);

        let world = World::with([w], [a.clone(), b.clone(), c.clone()], []);
        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());
        let state_view = state.view();

        // Asc sort by rank, offset=1, limit=2, fetch_size=1
        let params = QueryParams {
            pagination: Pagination::new(Some(nonzero!(2_u64)), 1),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(SortOrder::Asc),
            },
            fetch_size: FetchSize::new(Some(nonzero!(1_u64))),
        };

        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::account::prelude::FindAccounts);
        let qbox: iroha_data_model::query::QueryBox<_> =
            Box::new(iroha_data_model::query::ErasedIterQuery::<Account>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<Account>::PASS,
                SelectorTuple::<Account>::default(),
                payload,
            ));
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        // First batch: should contain rank=1 (b)
        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch1, remaining, cursor) = first.into_parts();
        let v1 = match batch1.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Account(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v1.len(), 1);
        assert_eq!(v1[0].id(), &b_id);
        assert_eq!(remaining, 1);

        // Second batch: should contain rank=2 (c)
        let cursor = cursor.expect("should continue");
        let next = handle.handle_iter_continue(cursor, &ALICE_ID).unwrap();
        let (batch2, remaining2, cursor2) = next.into_parts();
        let v2 = match batch2.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Account(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v2.len(), 1);
        assert_eq!(v2[0].id(), &c_id);
        assert_eq!(remaining2, 0);
        assert!(cursor2.is_none());
    }

    #[tokio::test]
    async fn iter_dispatch_accounts_offset_and_fetch_size_interplay_desc() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{
            FetchSize, Pagination, QueryParams, SortOrder, Sorting,
        };
        use iroha_futures::supervisor::ShutdownSignal;
        use iroha_primitives::json::Json;
        use nonzero_ext::nonzero;

        // Build three accounts with rank metadata: 0,1,2
        let w: Domain = Domain::new(DomainId::try_new("w", "universal").unwrap()).build(&ALICE_ID);
        let (a0_id, _) = iroha_test_samples::gen_account_in("w");
        let (a1_id, _) = iroha_test_samples::gen_account_in("w");
        let (a2_id, _) = iroha_test_samples::gen_account_in("w");

        let a0 = Account::new(a0_id.clone())
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(0)));
                m
            })
            .build(&a0_id);
        let a1 = Account::new(a1_id.clone())
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
                m
            })
            .build(&a1_id);
        let a2 = Account::new(a2_id.clone())
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
                m
            })
            .build(&a2_id);

        let world = World::with([w], [a0.clone(), a1.clone(), a2.clone()], []);
        let kura = Kura::blank_kura_for_testing();
        let store = std::sync::Arc::new(LiveQueryStore::from_config(
            StoreCfg::default(),
            ShutdownSignal::new(),
        ));
        let handle = crate::query::store::LiveQueryStoreHandle::new(store);
        let state = State::new(world, kura, handle.clone());
        let state_view = state.view();

        // Desc order gives [2,1,0]; offset=1 -> start at rank=1; limit=2 -> [1,0]; fetch_size=1 splits into two batches
        let params = QueryParams {
            pagination: Pagination::new(Some(nonzero!(2_u64)), 1),
            sorting: Sorting {
                sort_by_metadata_key: Some("rank".parse().unwrap()),
                order: Some(SortOrder::Desc),
            },
            fetch_size: FetchSize::new(Some(nonzero!(1_u64))),
        };

        let payload =
            norito::codec::Encode::encode(&iroha_data_model::query::account::prelude::FindAccounts);
        let qbox: iroha_data_model::query::QueryBox<_> =
            Box::new(iroha_data_model::query::ErasedIterQuery::<Account>::new(
                iroha_data_model::query::dsl::CompoundPredicate::<Account>::PASS,
                SelectorTuple::<Account>::default(),
                payload,
            ));
        let qwp = iroha_data_model::query::QueryWithParams::new(&qbox, params);
        let request = QueryRequest::Start(qwp);

        let validated = ValidQueryRequest::validate_for_client_parts(
            request,
            &ALICE_ID,
            &state_view,
            QueryLimits::default(),
        )
        .unwrap();
        let QueryResponse::Iterable(first) =
            validated.execute(&handle, &state_view, &ALICE_ID).unwrap()
        else {
            panic!("expected iterable")
        };
        let (batch1, remaining, cursor) = first.into_parts();
        let v1 = match batch1.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Account(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v1.len(), 1);
        assert_eq!(v1[0].id(), &a1_id);
        assert_eq!(remaining, 1);

        let cursor = cursor.expect("should continue");
        let next = handle.handle_iter_continue(cursor, &ALICE_ID).unwrap();
        let (batch2, remaining2, cursor2) = next.into_parts();
        let v2 = match batch2.into_iter().next().expect("slice") {
            iroha_data_model::query::QueryOutputBatchBox::Account(v) => v,
            other => panic!("unexpected batch variant: {other:?}"),
        };
        assert_eq!(v2.len(), 1);
        assert_eq!(v2[0].id(), &a0_id);
        assert_eq!(remaining2, 0);
        assert!(cursor2.is_none());
    }

    #[tokio::test]
    async fn find_transaction() -> Result<()> {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world_with_test_domains(), kura.clone(), query_handle);
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };

        let crypto_cfg = state.crypto();

        let ok_instruction = Log::new(iroha_logger::Level::INFO, "pass".into());
        let tx = TransactionBuilder::new(chain_id.clone(), ALICE_ID.clone())
            .with_instructions([ok_instruction])
            .sign(ALICE_KEYPAIR.private_key());

        let va_tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
        )?;

        let (peer_public_key, _) = bls_test_keypair().into_parts();
        let peer_id = PeerId::new(peer_public_key);
        let topology = Topology::new(vec![peer_id]);
        let unverified_block = BlockBuilder::new(vec![va_tx.clone()])
            .chain(0, state.view().latest_block().as_deref())
            .sign(ALICE_KEYPAIR.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header());
        let vcb = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {})
            .commit(&topology)
            .unpack(|_| {})
            .unwrap();

        let _events = state_block.apply(&vcb, topology.as_ref().to_owned());
        kura.store_block(vcb).expect("store block");
        state_block.commit().unwrap();

        let state_view = state.view();

        let unapplied_tx = TransactionBuilder::new(chain_id, ALICE_ID.clone())
            .with_instructions([Unregister::account(gen_account_in("domain").0)])
            .sign(ALICE_KEYPAIR.private_key());
        let wrong_hash = TransactionEntrypoint::from(unapplied_tx).hash();

        let not_found = FindTransactions::new()
            .execute(CompoundPredicate::PASS, &state_view)
            .expect("Query execution should not fail")
            .find(|tx| *tx.entrypoint_hash() == wrong_hash);
        assert_eq!(not_found, None, "Transaction should not be found");

        let found_accepted = FindTransactions::new()
            .execute(CompoundPredicate::PASS, &state_view)
            .expect("Query execution should not fail")
            .find(|tx| *tx.entrypoint_hash() == va_tx.as_ref().hash_as_entrypoint())
            .expect("Query should return a transaction");

        if found_accepted.result().is_err() {
            assert_eq!(
                va_tx.as_ref().hash_as_entrypoint(),
                found_accepted.entrypoint().hash(),
            )
        }
        Ok(())
    }
}
