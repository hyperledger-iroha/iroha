//! Query functionality. The common error type is also defined here,
//! alongside functions for converting them into HTTP responses.

use eyre::Result;
use iroha_config::parameters::{
    actual::{Pipeline as PipelineActual, Torii as ToriiActual},
    defaults::pipeline as pipeline_defaults,
};
use iroha_data_model::{
    offline::{
        OfflineAllowanceRecord, OfflineCounterSummary, OfflineTransferRecord,
        OfflineVerdictRevocation,
    },
    prelude::*,
    query::{
        CommittedTransaction, QueryBox, QueryOutputBatchBox, QueryRequest, QueryResponse,
        SingularQueryBox, SingularQueryOutputBox,
        dsl::{EvaluateSelector, HasProjection, SelectorMarker},
        error::QueryExecutionFail as Error,
        parameters::{DEFAULT_FETCH_SIZE, QueryParams, SortOrder},
    },
};

use crate::{
    prelude::ValidSingularQuery,
    query::{cursor::ErasedQueryIterator, pagination::Paginate as _, store::LiveQueryStoreHandle},
    smartcontracts::ValidQuery,
    state::{StateReadOnly, WorldReadOnly},
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
        dm_query::ErasedIterQuery<dm::role::Role>,
        dm_query::ErasedIterQuery<dm::role::RoleId>,
        dm_query::ErasedIterQuery<dm::peer::PeerId>,
        dm_query::ErasedIterQuery<dm::trigger::TriggerId>,
        dm_query::ErasedIterQuery<dm::trigger::Trigger>,
        dm_query::ErasedIterQuery<dm_query::CommittedTransaction>,
        dm_query::ErasedIterQuery<dm::block::SignedBlock>,
        dm_query::ErasedIterQuery<dm::block::BlockHeader>,
        dm_query::ErasedIterQuery<dm::proof::ProofRecord>,
        dm_query::ErasedIterQuery<dm::offline::OfflineAllowanceRecord>,
        dm_query::ErasedIterQuery<dm::offline::OfflineTransferRecord>,
        dm_query::ErasedIterQuery<dm::offline::OfflineCounterSummary>,
        dm_query::ErasedIterQuery<dm::offline::OfflineVerdictRevocation>,
    ]);
}

/// Allows to generalize retrieving the metadata key for all the query output types
pub trait SortableQueryOutput {
    /// Get the sorting key for the output, from metadata
    ///
    /// If the type doesn't have metadata or metadata key doesn't exist - return None
    fn get_metadata_sorting_key(&self, key: &Name) -> Option<Json>;
    /// Deterministic tie-breaker key for stable ordering across equal metadata keys.
    ///
    /// Implementations should return canonical bytes that uniquely and stably
    /// identify the item so that sorting remains stable across nodes.
    fn tiebreak_key(&self) -> Vec<u8>;
}

/// Query execution limits derived from configuration snapshots.
#[derive(Debug, Copy, Clone)]
pub struct QueryLimits {
    max_fetch_size: u64,
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
        }
    }
}

impl Default for QueryLimits {
    fn default() -> Self {
        Self::from_defaults()
    }
}

impl SortableQueryOutput for Account {
    fn get_metadata_sorting_key(&self, key: &Name) -> Option<Json> {
        self.metadata().get(key).cloned()
    }
    fn tiebreak_key(&self) -> Vec<u8> {
        use iroha_data_model::state::CanonicalStateKey;
        CanonicalStateKey::Account(self.id().clone()).to_canonical_bytes()
    }
}

impl SortableQueryOutput for Domain {
    fn get_metadata_sorting_key(&self, key: &Name) -> Option<Json> {
        self.metadata().get(key).cloned()
    }
    fn tiebreak_key(&self) -> Vec<u8> {
        use iroha_data_model::state::CanonicalStateKey;
        CanonicalStateKey::Domain(self.id().clone()).to_canonical_bytes()
    }
}

impl SortableQueryOutput for AssetDefinition {
    fn get_metadata_sorting_key(&self, key: &Name) -> Option<Json> {
        self.metadata().get(key).cloned()
    }
    fn tiebreak_key(&self) -> Vec<u8> {
        use iroha_data_model::state::CanonicalStateKey;
        CanonicalStateKey::AssetDefinition(self.id().clone()).to_canonical_bytes()
    }
}

impl SortableQueryOutput for Asset {
    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<Json> {
        None
    }
    fn tiebreak_key(&self) -> Vec<u8> {
        use iroha_data_model::state::CanonicalStateKey;
        CanonicalStateKey::Asset(self.id().clone()).to_canonical_bytes()
    }
}

impl SortableQueryOutput for Nft {
    fn get_metadata_sorting_key(&self, key: &Name) -> Option<Json> {
        self.content().get(key).cloned()
    }
    fn tiebreak_key(&self) -> Vec<u8> {
        use iroha_data_model::state::CanonicalStateKey;
        CanonicalStateKey::Nft(self.id().clone()).to_canonical_bytes()
    }
}

impl SortableQueryOutput for Role {
    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<Json> {
        None
    }
    fn tiebreak_key(&self) -> Vec<u8> {
        norito::codec::Encode::encode(self)
    }
}

impl SortableQueryOutput for RoleId {
    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<Json> {
        None
    }
    fn tiebreak_key(&self) -> Vec<u8> {
        norito::codec::Encode::encode(self)
    }
}
impl SortableQueryOutput for CommittedTransaction {
    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<Json> {
        None
    }
    fn tiebreak_key(&self) -> Vec<u8> {
        norito::codec::Encode::encode(self)
    }
}

impl SortableQueryOutput for PeerId {
    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<Json> {
        None
    }
    fn tiebreak_key(&self) -> Vec<u8> {
        norito::codec::Encode::encode(self)
    }
}

impl SortableQueryOutput for Permission {
    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<Json> {
        None
    }
    fn tiebreak_key(&self) -> Vec<u8> {
        norito::codec::Encode::encode(self)
    }
}

impl SortableQueryOutput for OfflineAllowanceRecord {
    fn get_metadata_sorting_key(&self, key: &Name) -> Option<Json> {
        self.certificate.metadata.get(key).cloned()
    }

    fn tiebreak_key(&self) -> Vec<u8> {
        self.certificate_id().as_ref().to_vec()
    }
}

impl SortableQueryOutput for OfflineTransferRecord {
    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<Json> {
        None
    }

    fn tiebreak_key(&self) -> Vec<u8> {
        self.transfer.bundle_id.as_ref().to_vec()
    }
}

impl SortableQueryOutput for OfflineCounterSummary {
    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<Json> {
        None
    }

    fn tiebreak_key(&self) -> Vec<u8> {
        norito::codec::Encode::encode(self)
    }
}

impl SortableQueryOutput for OfflineVerdictRevocation {
    fn get_metadata_sorting_key(&self, key: &Name) -> Option<Json> {
        self.metadata.get(key).cloned()
    }

    fn tiebreak_key(&self) -> Vec<u8> {
        norito::codec::Encode::encode(self)
    }
}

impl SortableQueryOutput for Trigger {
    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<Json> {
        None
    }
    fn tiebreak_key(&self) -> Vec<u8> {
        norito::codec::Encode::encode(self)
    }
}

impl SortableQueryOutput for TriggerId {
    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<Json> {
        None
    }
    fn tiebreak_key(&self) -> Vec<u8> {
        norito::codec::Encode::encode(self)
    }
}

impl SortableQueryOutput for RepoAgreement {
    fn get_metadata_sorting_key(&self, key: &Name) -> Option<Json> {
        self.collateral_leg().metadata().get(key).cloned()
    }

    fn tiebreak_key(&self) -> Vec<u8> {
        norito::codec::Encode::encode(self.id())
    }
}

trait ExecuteSingularQuery {
    fn execute(self, state: &impl StateReadOnly) -> Result<SingularQueryOutputBox, Error>;
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
            SingularQueryBox::FindDomainsByAccountId(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindAccountIdsByDomainId(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindProofRecordById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindContractManifestByCodeHash(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindActiveAbiVersions(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindAssetById(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
            SingularQueryBox::FindTwitterBindingByHash(q) => {
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
            SingularQueryBox::FindSorafsProviderOwner(q) => {
                Ok(SingularQueryOutputBox::from(q.execute(state)?))
            }
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
            Q::decode(&mut cursor).map_err(|_| {
                Error::Conversion("failed to decode iterable query payload".to_string())
            })
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
                Err(err) => return Some(Err(err)),
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
                .tuple
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
            dm::role::Role => dm::query::role::prelude::FindRoles,
            dm::role::RoleId => dm::query::role::prelude::FindRoleIds,
            dm::peer::PeerId => dm::query::peer::prelude::FindPeers,
            dm::trigger::Trigger => dm::query::trigger::prelude::FindTriggers,
            dm::trigger::TriggerId => dm::query::trigger::prelude::FindActiveTriggerIds,
            dm::block::SignedBlock => dm::query::block::prelude::FindBlocks,
            dm::block::BlockHeader => dm::query::block::prelude::FindBlockHeaders,
            dm::proof::ProofRecord => dm::query::proof::prelude::FindProofRecords,
            dm::query::CommittedTransaction => dm::query::transaction::prelude::FindTransactions,
            dm::offline::OfflineAllowanceRecord =>
                dm::query::offline::prelude::FindOfflineAllowances,
            dm::offline::OfflineCounterSummary =>
                dm::query::offline::prelude::FindOfflineCounterSummaries,
            dm::offline::OfflineTransferRecord =>
                dm::query::offline::prelude::FindOfflineToOnlineTransfers,
            dm::offline::OfflineVerdictRevocation =>
                dm::query::offline::prelude::FindOfflineVerdictRevocations,
        }

        Err(Error::Conversion(
            "dynamic QueryBox execution type not supported".to_string(),
        ))
    }
}

impl SortableQueryOutput for iroha_data_model::block::SignedBlock {
    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<Json> {
        None
    }
    fn tiebreak_key(&self) -> Vec<u8> {
        norito::codec::Encode::encode(self)
    }
}

impl SortableQueryOutput for iroha_data_model::block::BlockHeader {
    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<Json> {
        None
    }
    fn tiebreak_key(&self) -> Vec<u8> {
        norito::codec::Encode::encode(self)
    }
}

impl SortableQueryOutput for iroha_data_model::proof::ProofRecord {
    fn get_metadata_sorting_key(&self, _key: &Name) -> Option<Json> {
        None
    }
    fn tiebreak_key(&self) -> Vec<u8> {
        norito::codec::Encode::encode(self)
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
    I: Iterator<Item: SortableQueryOutput + Send + Sync + 'static>,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
    <I::Item as HasProjection<SelectorMarker>>::Projection: EvaluateSelector<I::Item> + Send + Sync,
    QueryOutputBatchBox: From<Vec<I::Item>>,
{
    let (output, _processed_items) =
        apply_query_postprocessing_with_budget(iter, selector, params, limits, None)?;
    Ok(output)
}

fn apply_query_postprocessing_with_budget<I>(
    iter: I,
    selector: SelectorTuple<I::Item>,
    params: &QueryParams,
    limits: QueryLimits,
    budget_items: Option<u64>,
) -> Result<(ErasedQueryIterator, u64), Error>
where
    I: Iterator<Item: SortableQueryOutput + Send + Sync + 'static>,
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
        let mut pairs: Vec<(Option<Json>, Vec<u8>, I::Item)> = Vec::new();
        for value in iter {
            count = count.saturating_add(1);
            if budget_items.is_some_and(|limit| count > limit) {
                return Err(Error::GasBudgetExceeded);
            }
            let key = value.get_metadata_sorting_key(key);
            let tb = value.tiebreak_key();
            pairs.push((key, tb, value));
        }
        // Stable sort by metadata value; missing keys always sort last.
        let order = params.sorting.order.unwrap_or(SortOrder::Asc);
        pairs.sort_by(|(a_key, a_tb, _), (b_key, b_tb, _)| {
            use core::cmp::Ordering::*;
            match (a_key.as_ref(), b_key.as_ref()) {
                (None, None) => a_tb.cmp(b_tb),
                (None, Some(_)) => Greater, // `a` missing -> after `b`
                (Some(_), None) => Less,    // `b` missing -> `a` first
                (Some(a), Some(b)) => {
                    let primary = match order {
                        SortOrder::Asc => a.cmp(b),
                        SortOrder::Desc => b.cmp(a),
                    };
                    if primary == Equal {
                        // Ties: resolve by canonical tie-break key ascending
                        a_tb.cmp(b_tb)
                    } else {
                        primary
                    }
                }
            }
        });

        let output: Vec<_> = pairs
            .into_iter()
            .map(|(_, _, val)| val)
            .paginate(params.pagination)
            .collect();

        (
            ErasedQueryIterator::new(output.into_iter(), selector, fetch_size),
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
        assert_eq!(iter.remaining(), 1);
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

    /// Execute a validated query request
    ///
    /// # Errors
    ///
    /// Returns an error if the query execution fails.
    #[allow(clippy::too_many_lines)] // not much we can do, we _need_ to list all the box types here
    pub fn execute(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
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
                    Q::decode(&mut cur).ok()
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
                    decode: F,
                ) -> Result<Option<QueryResponse>, Error>
                where
                    T: Send + Sync + 'static,
                    Q: super::super::ValidQuery<Item = T>,
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
                        let iter = ValidQuery::execute(concrete, erased.predicate_cloned(), state)?;

                        // Postprocess: sort/paginate/project and register a live iterator
                        let batched = apply_query_postprocessing(
                            iter,
                            erased.selector_cloned(),
                            params,
                            limits,
                        )?;
                        let output =
                            live_query_store.handle_iter_start(batched, authority, gas_budget)?;
                        return Ok(Some(QueryResponse::Iterable(output)));
                    }
                    Ok(None)
                }

                let params = &iter_query.params;
                #[cfg_attr(not(feature = "fast_dsl"), allow(unused_variables))]
                let stored_cursor_budget = {
                    let min = state.pipeline().query_stored_min_gas_units;
                    (min > 0).then_some(min)
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
                                let iter = ValidQuery::execute(<$find>::new(), pred, state)?;
                                let batched =
                                    apply_query_postprocessing(iter, sel, params, limits)?;
                                let output = live_query_store.handle_iter_start(
                                    batched,
                                    authority,
                                    stored_cursor_budget,
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
                                let iter = ValidQuery::execute(concrete, pred, state)?;
                                let batched =
                                    apply_query_postprocessing(iter, sel, params, limits)?;
                                let output = live_query_store.handle_iter_start(
                                    batched,
                                    authority,
                                    stored_cursor_budget,
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
                                let iter = ValidQuery::execute(<$find>::new(), pred, state)?;
                                let batched =
                                    apply_query_postprocessing(iter, sel, params, limits)?;
                                let output = live_query_store.handle_iter_start(
                                    batched,
                                    authority,
                                    stored_cursor_budget,
                                )?;
                                return Ok(QueryResponse::Iterable(output));
                            }};
                        }
                        match iter_query.item {
                            QueryItemKind::Domain => run_payload_or_default!(
                                iroha_data_model::domain::Domain,
                                iroha_data_model::query::domain::prelude::FindDomains
                            ),
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
                            QueryItemKind::Asset => run_payload_or_default!(
                                iroha_data_model::asset::value::Asset,
                                iroha_data_model::query::asset::prelude::FindAssets
                            ),
                            QueryItemKind::AssetDefinition => run_payload_or_default!(
                                iroha_data_model::asset::definition::AssetDefinition,
                                iroha_data_model::query::asset::prelude::FindAssetsDefinitions
                            ),
                            QueryItemKind::RepoAgreement => run_payload_or_default!(
                                iroha_data_model::repo::RepoAgreement,
                                iroha_data_model::query::repo::prelude::FindRepoAgreements
                            ),
                            QueryItemKind::Nft => run_payload_or_default!(
                                iroha_data_model::nft::Nft,
                                iroha_data_model::query::nft::prelude::FindNfts
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
                            QueryItemKind::CommittedTransaction => run_payload_or_default!(
                                iroha_data_model::query::CommittedTransaction,
                                iroha_data_model::query::transaction::prelude::FindTransactions
                            ),
                            QueryItemKind::SignedBlock => run_payload_or_default!(
                                iroha_data_model::block::SignedBlock,
                                iroha_data_model::query::block::prelude::FindBlocks
                            ),
                            QueryItemKind::BlockHeader => run_payload_or_default!(
                                iroha_data_model::block::BlockHeader,
                                iroha_data_model::query::block::prelude::FindBlockHeaders
                            ),
                            QueryItemKind::ProofRecord => run_payload_or_default!(
                                iroha_data_model::proof::ProofRecord,
                                iroha_data_model::query::proof::prelude::FindProofRecords
                            ),
                            QueryItemKind::OfflineAllowanceRecord => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::offline::OfflineAllowanceRecord,
                                        iroha_data_model::query::offline::prelude::FindOfflineAllowanceByCertificateId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::offline::OfflineAllowanceRecord,
                                    iroha_data_model::query::offline::prelude::FindOfflineAllowances
                                )
                            }
                            QueryItemKind::OfflineToOnlineTransfer => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::offline::OfflineTransferRecord,
                                        iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransferById
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::offline::OfflineTransferRecord,
                                    iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfers
                                )
                            }
                            QueryItemKind::OfflineCounterSummary => run_payload_or_default!(
                                iroha_data_model::offline::OfflineCounterSummary,
                                iroha_data_model::query::offline::prelude::FindOfflineCounterSummaries
                            ),
                            QueryItemKind::OfflineVerdictRevocation => run_payload_or_default!(
                                iroha_data_model::offline::OfflineVerdictRevocation,
                                iroha_data_model::query::offline::prelude::FindOfflineVerdictRevocations
                            ),
                            QueryItemKind::Permission => {
                                run_payload_or_default!(require_payload iroha_data_model::permission::Permission, iroha_data_model::query::permission::prelude::FindPermissionsByAccountId)
                            }
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
                            let iter = ValidQuery::execute(<$find>::new(), pred, state)?;
                            let batched = apply_query_postprocessing(iter, sel, params, limits)?;
                            let output = live_query_store.handle_iter_start(
                                batched,
                                authority,
                                stored_cursor_budget,
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
                        QueryItemKind::OfflineAllowanceRecord => run_unit!(
                            iroha_data_model::offline::OfflineAllowanceRecord,
                            iroha_data_model::query::offline::prelude::FindOfflineAllowances
                        ),
                        QueryItemKind::OfflineToOnlineTransfer => run_unit!(
                            iroha_data_model::offline::OfflineTransferRecord,
                            iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfers
                        ),
                        QueryItemKind::OfflineCounterSummary => run_unit!(
                            iroha_data_model::offline::OfflineCounterSummary,
                            iroha_data_model::query::offline::prelude::FindOfflineCounterSummaries
                        ),
                        QueryItemKind::OfflineVerdictRevocation => run_unit!(
                            iroha_data_model::offline::OfflineVerdictRevocation,
                            iroha_data_model::query::offline::prelude::FindOfflineVerdictRevocations
                        ),
                        QueryItemKind::Permission => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
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
                            let batched = apply_query_postprocessing(iter, sel, params, limits)?;
                            let output = live_query_store.handle_iter_start_ephemeral(batched)?;
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
                        QueryItemKind::OfflineAllowanceRecord => run_unit!(
                            iroha_data_model::offline::OfflineAllowanceRecord,
                            iroha_data_model::query::offline::prelude::FindOfflineAllowances
                        ),
                        QueryItemKind::OfflineToOnlineTransfer => run_unit!(
                            iroha_data_model::offline::OfflineTransferRecord,
                            iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfers
                        ),
                        QueryItemKind::OfflineCounterSummary => run_unit!(
                            iroha_data_model::offline::OfflineCounterSummary,
                            iroha_data_model::query::offline::prelude::FindOfflineCounterSummaries
                        ),
                        QueryItemKind::OfflineVerdictRevocation => run_unit!(
                            iroha_data_model::offline::OfflineVerdictRevocation,
                            iroha_data_model::query::offline::prelude::FindOfflineVerdictRevocations
                        ),
                        QueryItemKind::Permission => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
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
                            let iter = ValidQuery::execute(<$find>::new(), pred, state)?;
                            let batched = apply_query_postprocessing(iter, sel, params, limits)?;
                            let output = live_query_store.handle_iter_start(
                                batched,
                                authority,
                                stored_cursor_budget,
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
                        QueryItemKind::OfflineAllowanceRecord => run_unit!(
                            iroha_data_model::offline::OfflineAllowanceRecord,
                            iroha_data_model::query::offline::prelude::FindOfflineAllowances
                        ),
                        QueryItemKind::OfflineToOnlineTransfer => run_unit!(
                            iroha_data_model::offline::OfflineTransferRecord,
                            iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfers
                        ),
                        QueryItemKind::OfflineCounterSummary => run_unit!(
                            iroha_data_model::offline::OfflineCounterSummary,
                            iroha_data_model::query::offline::prelude::FindOfflineCounterSummaries
                        ),
                        QueryItemKind::OfflineVerdictRevocation => run_unit!(
                            iroha_data_model::offline::OfflineVerdictRevocation,
                            iroha_data_model::query::offline::prelude::FindOfflineVerdictRevocations
                        ),
                        QueryItemKind::Permission => {
                            return Err(Error::Conversion(
                                "missing or malformed query payload".into(),
                            ));
                        }
                    }
                };

                // Try dispatch for all supported iterable queries, keyed by their item type.
                // For item types that have multiple concrete query variants (e.g., Account),
                // attempt decodes in priority order.
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
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::proof::prelude::FindProofRecords,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::proof::prelude::FindProofRecords,
                        ))
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
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::proof::prelude::FindProofRecordsByStatus,
                        >(e)
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
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::query::CommittedTransaction,
                    iroha_data_model::query::transaction::prelude::FindTransactions,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::transaction::prelude::FindTransactions,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::transaction::prelude::FindTransactions,
                        ))
                    },
                )? {
                    return Ok(resp);
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
                    iroha_data_model::offline::OfflineAllowanceRecord,
                    iroha_data_model::query::offline::prelude::FindOfflineAllowanceByCertificateId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    try_decode_query::<
                        iroha_data_model::query::offline::prelude::FindOfflineAllowanceByCertificateId,
                    >,
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::offline::OfflineAllowanceRecord,
                    iroha_data_model::query::offline::prelude::FindOfflineAllowances,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::offline::prelude::FindOfflineAllowances,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::offline::prelude::FindOfflineAllowances,
                        ))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::offline::OfflineTransferRecord,
                    iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransferById,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    |e| {
                        try_decode_query::<
                        iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransferById,
                    >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::offline::OfflineTransferRecord,
                    iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfers,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfers,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfers,
                        ))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::offline::OfflineTransferRecord,
                    iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfersByController,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    try_decode_query::<
                        iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfersByController,
                    >,
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::offline::OfflineTransferRecord,
                    iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfersByReceiver,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    try_decode_query::<
                        iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfersByReceiver,
                    >,
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::offline::OfflineTransferRecord,
                    iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfersByStatus,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    stored_cursor_budget,
                    try_decode_query::<
                        iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfersByStatus,
                    >,
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::offline::OfflineTransferRecord,
                    iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfersByPolicy,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    None,
                    try_decode_query::<
                        iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfersByPolicy,
                    >,
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::offline::OfflineAllowanceRecord,
                    iroha_data_model::query::offline::prelude::FindOfflineAllowanceByCertificateId,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                        iroha_data_model::query::offline::prelude::FindOfflineAllowanceByCertificateId,
                    >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::offline::OfflineAllowanceRecord,
                    iroha_data_model::query::offline::prelude::FindOfflineAllowances,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::offline::prelude::FindOfflineAllowances,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::offline::prelude::FindOfflineAllowances,
                        ))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::offline::OfflineTransferRecord,
                    iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransferById,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                        iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransferById,
                    >(e)
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::offline::OfflineTransferRecord,
                    iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfers,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                            iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfers,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfers,
                        ))
                    },
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::offline::OfflineTransferRecord,
                    iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfersByController,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    None,
                    try_decode_query::<
                        iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfersByController,
                    >,
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::offline::OfflineTransferRecord,
                    iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfersByReceiver,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    None,
                    try_decode_query::<
                        iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfersByReceiver,
                    >,
                )? {
                    return Ok(resp);
                }
                if let Some(resp) = run_dispatch::<
                    iroha_data_model::offline::OfflineTransferRecord,
                    iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfersByStatus,
                    _,
                >(
                    qbox,
                    params,
                    limits,
                    state,
                    live_query_store,
                    authority,
                    None,
                    |e| {
                        try_decode_query::<
                        iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfersByStatus,
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
                live_query_store.handle_iter_continue(cursor)?,
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
        budget_items: Option<u64>,
    ) -> Result<(QueryResponse, u64), Error> {
        self.execute_ephemeral_inner_with_stats(live_query_store, state, authority, budget_items)
    }

    #[allow(clippy::too_many_lines)]
    fn execute_ephemeral_inner_with_stats(
        self,
        live_query_store: &LiveQueryStoreHandle,
        state: &impl StateReadOnly,
        authority: &AccountId,
        budget_items: Option<u64>,
    ) -> Result<(QueryResponse, u64), Error> {
        let Self { request, limits } = self;
        match request {
            QueryRequest::Singular(singular_query) => {
                let output = singular_query.execute(state)?;
                Ok((QueryResponse::Singular(output), 1))
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
                    Q::decode(&mut cur).ok()
                }

                #[allow(clippy::too_many_arguments)]
                fn run_dispatch<T, Q, F>(
                    qbox: &query::QueryBox<query::QueryOutputBatchBox>,
                    params: &query::parameters::QueryParams,
                    limits: QueryLimits,
                    budget_items: Option<u64>,
                    state: &impl StateReadOnly,
                    live_query_store: &LiveQueryStoreHandle,
                    _authority: &AccountId,
                    __stored_cursor_budget: Option<u64>,
                    decode: F,
                ) -> Result<Option<(QueryResponse, u64)>, Error>
                where
                    T: Send + Sync + 'static,
                    Q: super::super::ValidQuery<Item = T>,
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
                        let iter = ValidQuery::execute(concrete, erased.predicate_cloned(), state)?;

                        // Postprocess: sort/paginate/project and return only the first batch (no cursor)
                        let (batched, processed_items) = apply_query_postprocessing_with_budget(
                            iter,
                            erased.selector_cloned(),
                            params,
                            limits,
                            budget_items,
                        )?;
                        let output = live_query_store.handle_iter_start_ephemeral(batched)?;
                        return Ok(Some((QueryResponse::Iterable(output), processed_items)));
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
                                let (batched, processed_items) =
                                    apply_query_postprocessing_with_budget(
                                        iter,
                                        sel,
                                        params,
                                        limits,
                                        budget_items,
                                    )?;
                                let output =
                                    live_query_store.handle_iter_start_ephemeral(batched)?;
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
                                let (batched, processed_items) =
                                    apply_query_postprocessing_with_budget(
                                        iter,
                                        sel,
                                        params,
                                        limits,
                                        budget_items,
                                    )?;
                                let output =
                                    live_query_store.handle_iter_start_ephemeral(batched)?;
                                return Ok((QueryResponse::Iterable(output), processed_items));
                            }};
                        }
                        match iter_query.item {
                            QueryItemKind::Domain => run_payload_or_default!(
                                iroha_data_model::domain::Domain,
                                iroha_data_model::query::domain::prelude::FindDomains
                            ),
                            QueryItemKind::Account => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(require_payload iroha_data_model::account::Account, iroha_data_model::query::account::prelude::FindAccountsWithAsset)
                                }
                                run_payload_or_default!(
                                    iroha_data_model::account::Account,
                                    iroha_data_model::query::account::prelude::FindAccounts
                                )
                            }
                            QueryItemKind::Asset => run_payload_or_default!(
                                iroha_data_model::asset::value::Asset,
                                iroha_data_model::query::asset::prelude::FindAssets
                            ),
                            QueryItemKind::AssetDefinition => run_payload_or_default!(
                                iroha_data_model::asset::definition::AssetDefinition,
                                iroha_data_model::query::asset::prelude::FindAssetsDefinitions
                            ),
                            QueryItemKind::RepoAgreement => run_payload_or_default!(
                                iroha_data_model::repo::RepoAgreement,
                                iroha_data_model::query::repo::prelude::FindRepoAgreements
                            ),
                            QueryItemKind::Nft => run_payload_or_default!(
                                iroha_data_model::nft::Nft,
                                iroha_data_model::query::nft::prelude::FindNfts
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
                            QueryItemKind::CommittedTransaction => run_payload_or_default!(
                                iroha_data_model::query::CommittedTransaction,
                                iroha_data_model::query::transaction::prelude::FindTransactions
                            ),
                            QueryItemKind::SignedBlock => run_payload_or_default!(
                                iroha_data_model::block::SignedBlock,
                                iroha_data_model::query::block::prelude::FindBlocks
                            ),
                            QueryItemKind::BlockHeader => run_payload_or_default!(
                                iroha_data_model::block::BlockHeader,
                                iroha_data_model::query::block::prelude::FindBlockHeaders
                            ),
                            QueryItemKind::ProofRecord => run_payload_or_default!(
                                iroha_data_model::proof::ProofRecord,
                                iroha_data_model::query::proof::prelude::FindProofRecords
                            ),
                            QueryItemKind::OfflineAllowanceRecord => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::offline::OfflineAllowanceRecord,
                                        iroha_data_model::query::offline::prelude::FindOfflineAllowanceByCertificateId
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::offline::OfflineAllowanceRecord,
                                    iroha_data_model::query::offline::prelude::FindOfflineAllowances
                                )
                            }
                            QueryItemKind::OfflineToOnlineTransfer => {
                                if !iter_query.query_payload.is_empty() {
                                    run_payload_or_default!(
                                        require_payload iroha_data_model::offline::OfflineTransferRecord,
                                        iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransferById
                                    )
                                }
                                run_payload_or_default!(
                                    iroha_data_model::offline::OfflineTransferRecord,
                                    iroha_data_model::query::offline::prelude::FindOfflineToOnlineTransfers
                                )
                            }
                            QueryItemKind::OfflineCounterSummary => run_payload_or_default!(
                                iroha_data_model::offline::OfflineCounterSummary,
                                iroha_data_model::query::offline::prelude::FindOfflineCounterSummaries
                            ),
                            QueryItemKind::OfflineVerdictRevocation => run_payload_or_default!(
                                iroha_data_model::offline::OfflineVerdictRevocation,
                                iroha_data_model::query::offline::prelude::FindOfflineVerdictRevocations
                            ),
                            QueryItemKind::Permission => {
                                run_payload_or_default!(require_payload iroha_data_model::permission::Permission, iroha_data_model::query::permission::prelude::FindPermissionsByAccountId)
                            }
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
                if let Some((resp, processed_items)) = run_dispatch::<
                    iroha_data_model::query::CommittedTransaction,
                    iroha_data_model::query::transaction::prelude::FindTransactions,
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
                            iroha_data_model::query::transaction::prelude::FindTransactions,
                        >(e)
                        .or(Some(
                            iroha_data_model::query::transaction::prelude::FindTransactions,
                        ))
                    },
                )? {
                    return Ok((resp, processed_items));
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
                        try_decode_query::<
                                iroha_data_model::query::proof::prelude::FindProofRecords,
                            >(e)
                            .or(Some(
                                iroha_data_model::query::proof::prelude::FindProofRecords,
                            ))
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
    use std::borrow::Cow;

    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{
        AccountId, ChainId, DomainId, Level,
        isi::Log,
        query::{QueryRequest, SingularQueryBox, dsl::CompoundPredicate, prelude::FindParameters},
        transaction::TransactionBuilder,
    };
    use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR, BOB_ID, gen_account_in};
    use nonzero_ext::nonzero;
    use tokio::test;

    use super::*;
    use crate::{
        block::*,
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::{Execute, ValidQuery},
        state::{State, World},
        sumeragi::network_topology::Topology,
        tx::AcceptedTransaction,
    };

    fn dummy_accepted_transaction() -> AcceptedTransaction<'static> {
        let chain_id: ChainId = "00000000-0000-0000-0000-000000000000"
            .parse()
            .expect("valid chain id");
        let keypair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
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
        let mut d1 = Domain::new("d1".parse().unwrap()).build(&ALICE_ID);
        let mut d2 = Domain::new("d2".parse().unwrap()).build(&ALICE_ID);
        let d3 = Domain::new("d3".parse().unwrap()).build(&ALICE_ID); // no rank
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
        let mut d1 = Domain::new("d1".parse().unwrap()).build(&ALICE_ID);
        let mut d2 = Domain::new("d2".parse().unwrap()).build(&ALICE_ID);
        let d3 = Domain::new("d3".parse().unwrap()).build(&ALICE_ID); // no rank
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

    #[test]
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
        let domain_id = "wonderland".parse().expect("Valid");
        let domain = Domain::new(domain_id).build(&ALICE_ID);
        let account = Account::new(
            ALICE_ID
                .clone()
                .to_account_id("wonderland".parse().unwrap()),
        )
        .build(&ALICE_ID);
        let asset_definition_id = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
            "rose".parse().unwrap(),
        );
        let asset_definition = AssetDefinition::numeric(asset_definition_id).build(&ALICE_ID);
        World::with([domain], [account], [asset_definition])
    }

    #[cfg(feature = "bls")]
    fn bls_test_keypair() -> KeyPair {
        KeyPair::random_with_algorithm(Algorithm::BlsNormal)
    }

    #[cfg(not(feature = "bls"))]
    fn bls_test_keypair() -> KeyPair {
        KeyPair::random()
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
                let fail_isi = Unregister::domain("dummy".parse().unwrap());
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
        let d1_id: DomainId = "d1".parse().unwrap();
        let d2_id: DomainId = "d2".parse().unwrap();
        let d3_id: DomainId = "d3".parse().unwrap();
        let mut d1 = Domain::new(d1_id.clone()).build(&ALICE_ID);
        let mut d2 = Domain::new(d2_id.clone()).build(&ALICE_ID);
        let d3 = Domain::new(d3_id.clone()).build(&ALICE_ID);
        d1.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
        d2.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));

        let account = Account::new(ALICE_ID.clone().to_account_id(d1_id.clone())).build(&ALICE_ID);
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
        let next = handle.handle_iter_continue(cursor).unwrap();
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
            let d1_id: DomainId = "d1".parse().unwrap();
            let d2_id: DomainId = "d2".parse().unwrap();
            let d3_id: DomainId = "d3".parse().unwrap();

            let mut d1 = Domain::new(d1_id).build(&ALICE_ID);
            let mut d2 = Domain::new(d2_id).build(&ALICE_ID);
            let d3 = Domain::new(d3_id).build(&ALICE_ID);
            d1.metadata_mut()
                .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
            d2.metadata_mut()
                .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));

            let account =
                Account::new(ALICE_ID.clone().to_account_id(d1.id.clone())).build(&ALICE_ID);
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
                iroha_data_model::domain::Domain::new("w".parse().unwrap()).build(&ALICE_ID);
            let account = iroha_data_model::account::Account::new(
                ALICE_ID.clone().to_account_id("w".parse().unwrap()),
            )
            .build(&ALICE_ID);
            let ad_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "w".parse().unwrap(),
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
            let domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
            let account =
                Account::new(ALICE_ID.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
            let n1 = Nft::new("n1$w".parse().unwrap(), Metadata::default()).build(&ALICE_ID);
            let n2 = Nft::new("n2$w".parse().unwrap(), Metadata::default()).build(&ALICE_ID);
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
            let domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
            let alice =
                Account::new(ALICE_ID.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
            let bob =
                Account::new(BOB_ID.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
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
        let d1_id: DomainId = "d1".parse().unwrap();
        let d2_id: DomainId = "d2".parse().unwrap();
        let d3_id: DomainId = "d3".parse().unwrap();
        let mut d1 = Domain::new(d1_id.clone()).build(&ALICE_ID);
        let mut d2 = Domain::new(d2_id.clone()).build(&ALICE_ID);
        let d3 = Domain::new(d3_id.clone()).build(&ALICE_ID);
        d1.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
        d2.metadata_mut()
            .insert("rank".parse().unwrap(), Json::from(norito::json!(1)));

        let account = Account::new(ALICE_ID.clone().to_account_id(d1_id.clone())).build(&ALICE_ID);
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
            .handle_iter_continue(cursor.expect("should continue"))
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

        let domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let account =
            Account::new(ALICE_ID.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
        let n1 = Nft::new("n1$w".parse().unwrap(), Metadata::default()).build(&ALICE_ID);
        let n2 = Nft::new("n2$w".parse().unwrap(), Metadata::default()).build(&ALICE_ID);
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
        let domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let account =
            Account::new(ALICE_ID.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
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
        let a: Domain = Domain::new("a".parse().unwrap()).build(&ALICE_ID);
        let b: Domain = Domain::new("b".parse().unwrap()).build(&ALICE_ID);
        let c: Domain = Domain::new("c".parse().unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone().to_account_id(a.id.clone())).build(&ALICE_ID);
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
        let a: Domain = Domain::new("a".parse().unwrap()).build(&ALICE_ID);
        let b: Domain = Domain::new("b".parse().unwrap()).build(&ALICE_ID);
        let c: Domain = Domain::new("c".parse().unwrap()).build(&ALICE_ID);
        let d: Domain = Domain::new("d".parse().unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone().to_account_id(a.id.clone())).build(&ALICE_ID);
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
        let next = handle.handle_iter_continue(cursor.unwrap()).unwrap();
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
        let domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let (acc1_id, _) = iroha_test_samples::gen_account_in("w");
        let (acc2_id, _) = iroha_test_samples::gen_account_in("w");
        let acc1 =
            Account::new(acc1_id.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
        let acc2 =
            Account::new(acc2_id.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
        let ad1 = AssetDefinition::new(
            iroha_data_model::asset::AssetDefinitionId::new(
                "w".parse().unwrap(),
                "rose".parse().unwrap(),
            ),
            NumericSpec::default(),
        )
        .build(&ALICE_ID);
        let ad2 = AssetDefinition::new(
            iroha_data_model::asset::AssetDefinitionId::new(
                "w".parse().unwrap(),
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
        let w: Domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let (a_id, _) = iroha_test_samples::gen_account_in("w");
        let (b_id, _) = iroha_test_samples::gen_account_in("w");
        let (c_id, _) = iroha_test_samples::gen_account_in("w");

        let a = Account::new(a_id.clone().to_account_id("w".parse().unwrap()))
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
                m
            })
            .build(&a_id);
        let b = Account::new(b_id.clone().to_account_id("w".parse().unwrap()))
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
                m
            })
            .build(&b_id);
        let c = Account::new(c_id.clone().to_account_id("w".parse().unwrap())).build(&c_id);

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
            Account::new(id.clone().to_account_id("w".parse().unwrap()))
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
    async fn iter_dispatch_asset_definitions_sort_desc() {
        use iroha_config::parameters::actual::LiveQueryStore as StoreCfg;
        use iroha_data_model::query::parameters::{
            FetchSize, Pagination, QueryParams, SortOrder, Sorting,
        };
        use iroha_futures::supervisor::ShutdownSignal;

        let domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let account =
            Account::new(ALICE_ID.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
        let mut ad1 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
            "rose".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let mut ad2 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
            "tulip".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let ad3 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
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

        let domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let account =
            Account::new(ALICE_ID.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
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

    #[test]
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

    #[test]
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

    #[test]
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

    #[test]
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

    #[test]
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
        let alpha_id = "alpha".parse::<DomainId>().expect("valid");
        Register::domain(Domain::new(alpha_id.clone())).execute(&ALICE_ID, &mut state_tx)?;

        // Set metadata key "rank" on both domains: wonderland=1, alpha=2
        let key = "rank".parse::<Name>().expect("valid");
        SetKeyValue::domain("wonderland".parse().unwrap(), key.clone(), Json::new(1_u32))
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
        let domain_id: DomainId = "wonderland".parse().unwrap();
        let ad_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
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
        Register::account(Account::new(
            ALICE_ID.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .expect("register account");
        Register::asset_definition(
            AssetDefinition::numeric(ad_id.clone()).with_name(ad_id.name().to_string()),
        )
        .execute(&ALICE_ID, &mut stx)
        .expect("register asset definition");
        Mint::asset_numeric(13_u32, asset_id.clone())
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
        assert_eq!(*rose.value(), numeric!(13));
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
        let domain_id: DomainId = "wonderland".parse().unwrap();
        let (acc1_id, _) = iroha_test_samples::gen_account_in("wonderland");
        let (acc2_id, _) = iroha_test_samples::gen_account_in("wonderland");
        let ad_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonderland".parse().unwrap(),
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
        Register::account(Account::new(
            acc1_id.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .expect("register account1");
        Register::account(Account::new(
            acc2_id.clone().to_account_id(domain_id.clone()),
        ))
        .execute(&ALICE_ID, &mut stx)
        .expect("register account2");
        Register::asset_definition(
            AssetDefinition::numeric(ad_id.clone()).with_name(ad_id.name().to_string()),
        )
        .execute(&ALICE_ID, &mut stx)
        .expect("register asset definition");
        Mint::asset_numeric(1_u32, asset_id.clone())
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
            let domain_id: DomainId = "wonderland".parse().unwrap();
            let ad_id: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
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
            Register::account(Account::new(
                ALICE_ID.clone().to_account_id(domain_id.clone()),
            ))
            .execute(&ALICE_ID, &mut stx)
            .expect("register ALICE");
            Register::account(Account::new(
                BOB_ID.clone().to_account_id(domain_id.clone()),
            ))
            .execute(&ALICE_ID, &mut stx)
            .expect("register BOB");
            Register::asset_definition(
                AssetDefinition::numeric(ad_id.clone()).with_name(ad_id.name().to_string()),
            )
            .execute(&ALICE_ID, &mut stx)
            .expect("register asset definition");
            Mint::asset_numeric(5_u32, AssetId::new(ad_id.clone(), ALICE_ID.clone()))
                .execute(&ALICE_ID, &mut stx)
                .expect("mint asset for ALICE");
            Mint::asset_numeric(7_u32, AssetId::new(ad_id.clone(), BOB_ID.clone()))
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
                let next = handle.handle_iter_continue(c).expect("continue cursor");
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

    #[test]
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

    #[cfg(feature = "ids_projection")]
    #[tokio::test]
    async fn iter_dispatch_domains_ids_only_projection() {
        use iroha_data_model::query::{
            self, QueryItemKind, QueryWithParams,
            dsl::{CompoundPredicate, SelectorTuple},
        };

        // Build world with two domains and ALICE account
        let d1: Domain = Domain::new("w1".parse().unwrap()).build(&ALICE_ID);
        let d2: Domain = Domain::new("w2".parse().unwrap()).build(&ALICE_ID);
        let account = Account::new(ALICE_ID.clone().to_account_id(d1.id.clone())).build(&ALICE_ID);
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

        let w: Domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let (a_id, _) = iroha_test_samples::gen_account_in("w");
        let (b_id, _) = iroha_test_samples::gen_account_in("w");
        let a = Account::new(a_id.clone().to_account_id("w".parse().unwrap())).build(&a_id);
        let b = Account::new(b_id.clone().to_account_id("w".parse().unwrap())).build(&b_id);
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

        let domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let account =
            Account::new(ALICE_ID.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
        let ad1 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
            "rose".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let ad2 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
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

        let domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let account =
            Account::new(ALICE_ID.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
        let nft1 = Nft::new("n1$w".parse().unwrap(), Metadata::default()).build(&ALICE_ID);
        let nft2 = Nft::new("n2$w".parse().unwrap(), Metadata::default()).build(&ALICE_ID);
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
        let domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let role1 = Role::new("r1".parse().unwrap(), ALICE_ID.clone()).build(&ALICE_ID);
        let role2 = Role::new("r2".parse().unwrap(), ALICE_ID.clone()).build(&ALICE_ID);
        let world = {
            let mut w = World::with(
                [domain],
                [
                    Account::new(ALICE_ID.clone().to_account_id("w".parse().unwrap()))
                        .build(&ALICE_ID),
                ],
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

        let domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let account =
            Account::new(ALICE_ID.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
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

        let domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let account =
            Account::new(ALICE_ID.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
        let mut ad1 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
            "rose".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let mut ad2 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
            "tulip".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let ad3 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
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

        let w: Domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let (a_id, _) = iroha_test_samples::gen_account_in("w");
        let (b_id, _) = iroha_test_samples::gen_account_in("w");
        let (c_id, _) = iroha_test_samples::gen_account_in("w");

        let a = Account::new(a_id.clone().to_account_id("w".parse().unwrap()))
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
                m
            })
            .build(&a_id);
        let b = Account::new(b_id.clone().to_account_id("w".parse().unwrap()))
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
                m
            })
            .build(&b_id);
        let c = Account::new(c_id.clone().to_account_id("w".parse().unwrap())).build(&c_id);

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

        let w: Domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let (a_id, _) = iroha_test_samples::gen_account_in("w");
        let (b_id, _) = iroha_test_samples::gen_account_in("w");
        let (c_id, _) = iroha_test_samples::gen_account_in("w");

        let a = Account::new(a_id.clone().to_account_id("w".parse().unwrap()))
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(2)));
                m
            })
            .build(&a_id);
        let b = Account::new(b_id.clone().to_account_id("w".parse().unwrap()))
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
                m
            })
            .build(&b_id);
        let c = Account::new(c_id.clone().to_account_id("w".parse().unwrap())).build(&c_id);

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

        let next = handle.handle_iter_continue(cursor).unwrap();
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

        let domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let account =
            Account::new(ALICE_ID.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
        let mut ad1 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
            "rose".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let mut ad2 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
            "tulip".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let ad3 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
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

        let next = handle.handle_iter_continue(cursor).unwrap();
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
        let domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let account =
            Account::new(ALICE_ID.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
        let mut ad0 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
            "a0".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let mut ad1 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
            "a1".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let mut ad2 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
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
        let next = handle.handle_iter_continue(cursor).unwrap();
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
        let domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let account =
            Account::new(ALICE_ID.clone().to_account_id("w".parse().unwrap())).build(&ALICE_ID);
        let mut ad0 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
            "a0".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let mut ad1 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
            "a1".parse().unwrap(),
        ))
        .build(&ALICE_ID);
        let mut ad2 = AssetDefinition::numeric(iroha_data_model::asset::AssetDefinitionId::new(
            "w".parse().unwrap(),
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
        let next = handle.handle_iter_continue(cursor).unwrap();
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
        let w: Domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let (a_id, _) = iroha_test_samples::gen_account_in("w");
        let (b_id, _) = iroha_test_samples::gen_account_in("w");
        let (c_id, _) = iroha_test_samples::gen_account_in("w");

        let a = Account::new(a_id.clone().to_account_id("w".parse().unwrap()))
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(0)));
                m
            })
            .build(&a_id);
        let b = Account::new(b_id.clone().to_account_id("w".parse().unwrap()))
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
                m
            })
            .build(&b_id);
        let c = Account::new(c_id.clone().to_account_id("w".parse().unwrap()))
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
        let next = handle.handle_iter_continue(cursor).unwrap();
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
        let w: Domain = Domain::new("w".parse().unwrap()).build(&ALICE_ID);
        let (a0_id, _) = iroha_test_samples::gen_account_in("w");
        let (a1_id, _) = iroha_test_samples::gen_account_in("w");
        let (a2_id, _) = iroha_test_samples::gen_account_in("w");

        let a0 = Account::new(a0_id.clone().to_account_id("w".parse().unwrap()))
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(0)));
                m
            })
            .build(&a0_id);
        let a1 = Account::new(a1_id.clone().to_account_id("w".parse().unwrap()))
            .with_metadata({
                let mut m = Metadata::default();
                m.insert("rank".parse().unwrap(), Json::from(norito::json!(1)));
                m
            })
            .build(&a1_id);
        let a2 = Account::new(a2_id.clone().to_account_id("w".parse().unwrap()))
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
        let next = handle.handle_iter_continue(cursor).unwrap();
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

    #[test]
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
