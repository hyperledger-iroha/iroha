//! Contains common types and traits to facilitate building and sending queries,
//! either from the client or from smart contracts.
//!
//! Constructed queries are ultimately converted into [`QueryBox`] trait objects
//! (`Box<dyn Query + Send + Sync>`) so that they can be executed by any component
//! expecting a type-erased query.

mod batch_downcast;
mod iter;

use std::{marker::PhantomData, vec::Vec};

use derive_where::derive_where;
pub use iter::QueryIterator;

use crate::query::{
    Query, QueryBox, QueryOutputBatchBox, QueryOutputBatchBoxTuple, QueryWithFilter,
    QueryWithParams, SingularQueryBox, SingularQueryOutputBox,
    builder::batch_downcast::HasTypedBatchIter,
    dsl::{
        BaseProjector, CompoundPredicate, HasProjection, HasPrototype, IntoSelectorTuple,
        PredicateMarker, SelectorMarker, SelectorTuple,
    },
    parameters::{FetchSize, Pagination, QueryParams, Sorting},
};

/// A trait abstracting away concrete backend for executing queries against iroha.
pub trait QueryExecutor {
    /// A type of cursor used in iterable queries.
    ///
    /// The cursor type is an opaque type that allows to continue execution of an iterable query with [`Self::continue_query`]
    type Cursor;
    /// An error that can occur during query execution.
    type Error;

    /// Executes a singular query and returns its result.
    ///
    /// # Errors
    ///
    /// Returns an error if the query execution fails.
    fn execute_singular_query(
        &self,
        query: SingularQueryBox,
    ) -> Result<SingularQueryOutputBox, Self::Error>;

    /// Starts an iterable query and returns the first batch of results, the remaining number of results and a cursor to continue the query.
    ///
    /// # Errors
    ///
    /// Returns an error if the query execution fails.
    #[expect(clippy::type_complexity)]
    fn start_query(
        &self,
        query: QueryWithParams,
    ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error>;

    /// Continues an iterable query from the given cursor and returns the next batch of results, the remaining number of results and a cursor to continue the query.
    ///
    /// # Errors
    ///
    /// Returns an error if the query execution fails.
    #[expect(clippy::type_complexity)]
    fn continue_query(
        cursor: Self::Cursor,
    ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error>;
}

/// An error that can occur when constraining the number of results of an iterable query to one.
#[derive(
    Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, displaydoc::Display, thiserror::Error,
)]
pub enum SingleQueryError<E> {
    /// An error occurred during query execution
    QueryError(E),
    /// Expected exactly one query result, got none
    ExpectedOneGotNone,
    /// Expected exactly one query result, got more than one
    ExpectedOneGotMany,
    /// Expected one or zero query results, got more than one
    ExpectedOneOrZeroGotMany,
}

impl<E> From<E> for SingleQueryError<E> {
    fn from(e: E) -> Self {
        SingleQueryError::QueryError(e)
    }
}

/// Struct that simplifies construction of an iterable query.
#[derive_where(Clone; Q, CompoundPredicate<Q::Item>, SelectorTuple<Q::Item>)]
pub struct QueryBuilder<'e, E, Q, T>
where
    Q: Query + 'static,
    T: 'static,
{
    query_executor: &'e E,
    query: Q,
    filter: CompoundPredicate<Q::Item>,
    selector: SelectorTuple<Q::Item>,
    pagination: Pagination,
    sorting: Sorting,
    fetch_size: FetchSize,
    // NOTE: T is a phantom type used to denote the selected tuple in `selector`
    phantom: PhantomData<T>,
}

impl<'a, E, Q> QueryBuilder<'a, E, Q, Q::Item>
where
    Q: Query + 'static,
{
    /// Create a new iterable query builder for a given backend and query.
    pub fn new(query_executor: &'a E, query: Q) -> Self {
        Self {
            query_executor,
            query,
            filter: CompoundPredicate::PASS,
            selector: SelectorTuple::default(),
            pagination: Pagination::default(),
            sorting: Sorting::default(),
            fetch_size: FetchSize::default(),
            phantom: PhantomData,
        }
    }

    /// Experimental: override the selector tuple directly.
    ///
    /// Note: In the lightweight DSL, selectors are not evaluated; this method
    /// simply forwards the tuple to the server. Once server-side projection is
    /// restored, the selector will take effect. The type parameter `T` remains
    /// `Q::Item` here; when projection starts returning tuples, callers should
    /// prefer `select_with` to preserve typed results.
    #[must_use]
    pub fn with_selector_tuple(self, selector: SelectorTuple<Q::Item>) -> Self {
        Self { selector, ..self }
    }
}

impl<'a, E, Q, T> QueryBuilder<'a, E, Q, T>
where
    Q: Query + 'static,
    T: 'static,
{
    /// Only return results that match the given predicate.
    ///
    /// If multiple filters are added, they are combined with a logical AND.
    #[must_use]
    pub fn filter(self, filter: CompoundPredicate<Q::Item>) -> Self {
        Self {
            filter: self.filter.and(filter),
            ..self
        }
    }

    /// Only return results that match the predicate constructed with the given closure.
    ///
    /// If multiple filters are added, they are combined with a logical AND.
    #[must_use]
    pub fn filter_with<B>(self, predicate_builder: B) -> Self
    where
        Q::Item: HasPrototype,
        B: FnOnce(
            <Q::Item as HasPrototype>::Prototype<
                PredicateMarker,
                BaseProjector<PredicateMarker, Q::Item>,
            >,
        ) -> CompoundPredicate<Q::Item>,
        <Q::Item as HasPrototype>::Prototype<
            PredicateMarker,
            BaseProjector<PredicateMarker, Q::Item>,
        >: Default,
    {
        self.filter(predicate_builder(Default::default()))
    }

    /// Return only the fields of the results specified by the given closure.
    ///
    /// You can select multiple fields by returning a tuple from the closure.
    #[must_use]
    pub fn select_with<B, O>(self, f: B) -> QueryBuilder<'a, E, Q, O::SelectedTuple>
    where
        Q::Item: HasPrototype,
        B: FnOnce(
            <Q::Item as HasPrototype>::Prototype<
                SelectorMarker,
                BaseProjector<SelectorMarker, Q::Item>,
            >,
        ) -> O,
        <Q::Item as HasPrototype>::Prototype<
            SelectorMarker,
            BaseProjector<SelectorMarker, Q::Item>,
        >: Default,
        O: IntoSelectorTuple<SelectingType = Q::Item>,
    {
        let new_selector = f(Default::default()).into_selector_tuple();

        QueryBuilder {
            query_executor: self.query_executor,
            query: self.query,
            filter: self.filter,
            selector: new_selector,
            pagination: self.pagination,
            sorting: self.sorting,
            fetch_size: self.fetch_size,
            phantom: PhantomData,
        }
    }

    /// Sort the results according to the specified sorting.
    #[must_use]
    pub fn with_sorting(self, sorting: Sorting) -> Self {
        Self { sorting, ..self }
    }

    /// Only return part of the results specified by the pagination.
    #[must_use]
    pub fn with_pagination(self, pagination: Pagination) -> Self {
        Self { pagination, ..self }
    }

    /// Change the batch size of the iterable query.
    ///
    /// Larger batch sizes reduce the number of round-trips to iroha peer, but require more memory.
    #[must_use]
    pub fn with_fetch_size(self, fetch_size: FetchSize) -> Self {
        Self { fetch_size, ..self }
    }
}

impl<E, Q, T> QueryBuilder<'_, E, Q, T>
where
    Q: Query
        + HasProjection<PredicateMarker>
        + HasProjection<SelectorMarker, AtomType = ()>
        + 'static,
    E: QueryExecutor,
    Q::Item: Send + Sync,
    T: HasTypedBatchIter + HasProjection<PredicateMarker> + 'static,
    QueryBox<QueryOutputBatchBox>: From<QueryWithFilter<Q::Item>>,
{
    /// Execute the query, returning an iterator over its results.
    ///
    /// # Errors
    ///
    /// Returns an error if the query execution fails.
    #[allow(clippy::too_many_lines, clippy::option_if_let_else)]
    pub fn execute(self) -> Result<QueryIterator<E, T>, E::Error>
    where
        T: crate::query::ItemKindTag,
    {
        #[cfg(not(feature = "fast_dsl"))]
        let boxed: QueryBox<QueryOutputBatchBox> = {
            use crate::query::ErasedIterQuery;
            // Preserve the concrete query bytes so the server can reconstruct
            // variant-specific parameters (e.g., FindAccountsWithAsset).
            // Use a conservative downcast to known query types to avoid adding
            // additional trait bounds to `Q`.
            let any: &dyn core::any::Any = &self.query;
            let payload: Vec<u8> = if let Some(q) =
                any.downcast_ref::<crate::query::account::prelude::FindAccountsWithAsset>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::account::prelude::FindAccounts>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<crate::query::domain::prelude::FindDomains>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<crate::query::asset::prelude::FindAssets>() {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::asset::prelude::FindAssetsDefinitions>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::repo::prelude::FindRepoAgreements>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<crate::query::nft::prelude::FindNfts>() {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<crate::query::rwa::prelude::FindRwas>() {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<crate::query::peer::prelude::FindPeers>() {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::trigger::prelude::FindActiveTriggerIds>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::trigger::prelude::FindTriggers>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::role::prelude::FindRolesByAccountId>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::permission::prelude::FindPermissionsByAccountId>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::transaction::prelude::FindTransactions>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<crate::query::block::prelude::FindBlocks>() {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::block::prelude::FindBlockHeaders>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::offline::prelude::FindOfflineAllowances>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any
                .downcast_ref::<crate::query::offline::prelude::FindOfflineAllowanceByCertificateId>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::offline::prelude::FindOfflineCounterSummaries>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::offline::prelude::FindOfflineToOnlineTransfers>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<
                crate::query::offline::prelude::FindOfflineToOnlineTransfersByPolicy,
            >() {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any
                .downcast_ref::<crate::query::offline::prelude::FindOfflineToOnlineTransferById>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any
                .downcast_ref::<crate::query::offline::prelude::FindOfflineVerdictRevocations>()
            {
                norito::codec::Encode::encode(q)
            } else {
                Vec::new()
            };
            let erased = ErasedIterQuery::new(self.filter, self.selector, payload);
            Box::new(erased)
        };
        #[cfg(feature = "fast_dsl")]
        let item_kind = <T as crate::query::ItemKindTag>::kind();

        // Capture encoded payload of the concrete query variant when possible, so that
        // fast_dsl builds can reconstruct parameterized queries on the server side.
        #[allow(unused_mut)]
        #[cfg(feature = "fast_dsl")]
        let mut query_payload: Vec<u8> = {
            let any: &dyn core::any::Any = &self.query;
            if let Some(q) =
                any.downcast_ref::<crate::query::account::prelude::FindAccountsWithAsset>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::account::prelude::FindAccounts>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<crate::query::domain::prelude::FindDomains>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<crate::query::asset::prelude::FindAssets>() {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::asset::prelude::FindAssetsDefinitions>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::repo::prelude::FindRepoAgreements>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<crate::query::nft::prelude::FindNfts>() {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<crate::query::rwa::prelude::FindRwas>() {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<crate::query::peer::prelude::FindPeers>() {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::trigger::prelude::FindActiveTriggerIds>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::trigger::prelude::FindTriggers>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::transaction::prelude::FindTransactions>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<crate::query::block::prelude::FindBlocks>() {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::block::prelude::FindBlockHeaders>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::offline::prelude::FindOfflineAllowances>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any
                .downcast_ref::<crate::query::offline::prelude::FindOfflineAllowanceByCertificateId>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::offline::prelude::FindOfflineCounterSummaries>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::offline::prelude::FindOfflineToOnlineTransfers>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<
                crate::query::offline::prelude::FindOfflineToOnlineTransfersByPolicy,
            >() {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any
                .downcast_ref::<crate::query::offline::prelude::FindOfflineToOnlineTransferById>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any
                .downcast_ref::<crate::query::offline::prelude::FindOfflineVerdictRevocations>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::role::prelude::FindRolesByAccountId>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) =
                any.downcast_ref::<crate::query::permission::prelude::FindPermissionsByAccountId>()
            {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<crate::query::role::prelude::FindRoles>() {
                norito::codec::Encode::encode(q)
            } else if let Some(q) = any.downcast_ref::<crate::query::role::prelude::FindRoleIds>() {
                norito::codec::Encode::encode(q)
            } else {
                Vec::new()
            }
        };

        let query = QueryWithParams {
            #[cfg(not(feature = "fast_dsl"))]
            query: boxed,
            #[cfg(feature = "fast_dsl")]
            query: (),
            #[cfg(feature = "fast_dsl")]
            query_payload,
            #[cfg(feature = "fast_dsl")]
            item: item_kind,
            #[cfg(feature = "fast_dsl")]
            predicate_bytes: norito::codec::Encode::encode(&self.filter),
            #[cfg(feature = "fast_dsl")]
            selector_bytes: norito::codec::Encode::encode(&self.selector),
            params: QueryParams {
                pagination: self.pagination,
                sorting: self.sorting,
                fetch_size: self.fetch_size,
            },
        };

        let (first_batch, remaining_items, continue_cursor) =
            self.query_executor.start_query(query)?;

        let iterator = QueryIterator::<E, T>::new(first_batch, remaining_items, continue_cursor)
            .expect(
                "INTERNAL BUG: iroha returned unexpected type in iterable query. Is there a schema mismatch?",
            );

        Ok(iterator)
    }
}

/// An extension trait for query builders that provides convenience methods to execute queries.
pub trait QueryBuilderExt<E, Q, T>
where
    E: QueryExecutor,
    Q: Query
        + HasProjection<PredicateMarker>
        + HasProjection<SelectorMarker, AtomType = ()>
        + 'static,
    T: HasTypedBatchIter + HasProjection<PredicateMarker> + 'static,
    QueryBox<QueryOutputBatchBox>: From<QueryWithFilter<Q>>,
{
    /// Execute the query, returning all the results collected into a vector.
    ///
    /// # Errors
    ///
    /// Returns an error if the query execution fails.
    fn execute_all(self) -> Result<Vec<T>, E::Error>;

    /// Execute the query, constraining the number of results to zero or one.
    ///
    /// # Errors
    ///
    /// Returns an error if the query execution fails or if more than one result is returned.
    fn execute_single_opt(self) -> Result<Option<T>, SingleQueryError<E::Error>>;

    /// Execute the query, constraining the number of results to exactly one.
    ///
    /// # Errors
    ///
    /// Returns an error if the query execution fails or if zero or more than one result is returned.
    fn execute_single(self) -> Result<T, SingleQueryError<E::Error>>;
}

/// Blanket implementation of [`QueryBuilderExt`] for [`QueryBuilder`].
impl<E, Q, T> QueryBuilderExt<E, Q, T> for QueryBuilder<'_, E, Q, T>
where
    E: QueryExecutor,
    Q: Query
        + HasProjection<PredicateMarker>
        + HasProjection<SelectorMarker, AtomType = ()>
        + 'static,
    Q::Item: Send + Sync,
    T: HasTypedBatchIter + HasProjection<PredicateMarker> + 'static + crate::query::ItemKindTag,
    QueryBox<QueryOutputBatchBox>: From<QueryWithFilter<Q::Item>>,
{
    fn execute_all(self) -> Result<Vec<T>, E::Error> {
        self.execute()?.collect::<Result<Vec<_>, _>>()
    }

    fn execute_single_opt(self) -> Result<Option<T>, SingleQueryError<E::Error>> {
        let mut iter = self.execute()?;
        let first = iter.next().transpose()?;
        let second = iter.next().transpose()?;

        match (first, second) {
            (None, None) => Ok(None),
            (Some(result), None) => Ok(Some(result)),
            (Some(_), Some(_)) => Err(SingleQueryError::ExpectedOneOrZeroGotMany),
            (None, Some(_)) => {
                unreachable!()
            }
        }
    }

    fn execute_single(self) -> Result<T, SingleQueryError<E::Error>> {
        let mut iter = self.execute()?;
        let first = iter.next().transpose()?;
        let second = iter.next().transpose()?;

        match (first, second) {
            (None, None) => Err(SingleQueryError::ExpectedOneGotNone),
            (Some(result), None) => Ok(result),
            (Some(_), Some(_)) => Err(SingleQueryError::ExpectedOneGotMany),
            (None, Some(_)) => {
                unreachable!()
            }
        }
    }
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::QueryBuilderExt;
}
