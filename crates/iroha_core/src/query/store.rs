//! This module contains [`LiveQueryStore`] actor.

use std::{
    num::{NonZeroU64, NonZeroUsize},
    sync::Arc,
    time::{Duration, Instant},
};

use dashmap::{DashMap, mapref::entry::Entry};
use iroha_config::parameters::actual::LiveQueryStore as Config;
use iroha_data_model::{
    account::AccountId,
    query::{
        QueryOutput, QueryOutputBatchBoxTuple,
        error::QueryExecutionFail,
        parameters::{ForwardCursor, QueryId},
    },
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_logger::{trace, warn};
use tokio::task::JoinHandle;

use super::cursor::ErasedQueryIterator;

type LiveQuery = ErasedQueryIterator;

/// Service which stores queries which might be non fully consumed by a client.
///
/// Clients can handle their queries using [`LiveQueryStoreHandle`]
#[derive(Debug)]
pub struct LiveQueryStore {
    queries: DashMap<QueryId, QueryInfo>,
    queries_per_user: DashMap<AccountId, usize>,
    // The maximum number of queries in the store
    capacity: NonZeroUsize,
    // The maximum number of queries in the store per user
    capacity_per_user: NonZeroUsize,
    // Queries older then this time will be automatically removed from the store
    idle_time: Duration,
    shutdown_signal: ShutdownSignal,
}

#[derive(Debug)]
struct QueryInfo {
    live_query: LiveQuery,
    last_access_time: Instant,
    authority: AccountId,
}

impl LiveQueryStore {
    /// Construct [`LiveQueryStore`] from configuration.
    pub fn from_config(cfg: Config, shutdown_signal: ShutdownSignal) -> Self {
        Self {
            queries: DashMap::new(),
            queries_per_user: DashMap::new(),
            idle_time: cfg.idle_time,
            capacity: cfg.capacity,
            capacity_per_user: cfg.capacity_per_user,
            shutdown_signal,
        }
    }

    /// Construct [`LiveQueryStore`] for tests.
    /// Default configuration will be used.
    ///
    /// Not marked as `#[cfg(test)]` because it is used in benches as well.
    pub fn start_test() -> LiveQueryStoreHandle {
        // For tests, avoid spawning the pruning task to remove the dependency
        // on a running Tokio runtime. Tests typically exercise iterator paths
        // directly and do not rely on background pruning.
        let store = Arc::new(Self::from_config(Config::default(), ShutdownSignal::new()));
        LiveQueryStoreHandle::new(store)
    }

    /// Start [`LiveQueryStore`]. Requires a [`tokio::runtime::Runtime`] being run
    /// as it will create new [`tokio::task`] and detach it.
    ///
    /// Returns a handle to interact with the service.
    pub fn start(self) -> (LiveQueryStoreHandle, Child) {
        let store = Arc::new(self);
        let handle = Arc::clone(&store).spawn_pruning_task();
        (
            LiveQueryStoreHandle { store },
            Child::new(
                handle,
                // should shutdown immediately anyway
                OnShutdown::Wait(Duration::from_millis(5000)),
            ),
        )
    }

    fn spawn_pruning_task(self: Arc<Self>) -> JoinHandle<()> {
        let mut idle_interval = tokio::time::interval(self.idle_time);
        tokio::task::spawn(async move {
            loop {
                tokio::select! {
                    _ = idle_interval.tick() => {
                        self.prune_expired_queries();
                    }
                    () = self.shutdown_signal.receive() => {
                        iroha_logger::debug!("LiveQueryStore is being shut down.");
                        break;
                    }
                    else => break,
                }
            }
        })
    }

    fn prune_expired_queries(&self) -> Vec<QueryId> {
        let mut expired = Vec::new();
        self.queries.retain(|key, query| {
            if query.last_access_time.elapsed() <= self.idle_time {
                true
            } else {
                expired.push(key.clone());
                self.decrease_queries_per_user(query.authority.clone());
                false
            }
        });
        expired
    }

    fn insert(&self, query_id: QueryId, live_query: ErasedQueryIterator, authority: AccountId) {
        *self.queries_per_user.entry(authority.clone()).or_insert(0) += 1;
        let query_info = QueryInfo {
            live_query,
            last_access_time: Instant::now(),
            authority,
        };
        self.queries.insert(query_id, query_info);
    }

    fn remove(&self, query_id: &str) -> Option<QueryInfo> {
        let (_, query_info) = self.queries.remove(query_id)?;
        self.decrease_queries_per_user(query_info.authority.clone());
        Some(query_info)
    }

    fn decrease_queries_per_user(&self, authority: AccountId) {
        let Entry::Occupied(mut entry) = self.queries_per_user.entry(authority) else {
            return;
        };

        let counter = entry.get_mut();
        *counter -= 1;
        if *counter == 0 {
            entry.remove_entry();
        }
    }

    fn insert_new_query(
        &self,
        query_id: QueryId,
        live_query: ErasedQueryIterator,
        authority: AccountId,
    ) -> Result<(), QueryExecutionFail> {
        trace!(%query_id, "Inserting new query");
        self.check_capacity(&authority)?;
        self.insert(query_id, live_query, authority);
        Ok(())
    }

    // For the existing query, takes and returns the first batch.
    // If query becomes depleted, it will be removed from the store.
    fn get_query_next_batch(
        &self,
        query_id: &QueryId,
        cursor: NonZeroU64,
    ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<NonZeroU64>), QueryExecutionFail> {
        trace!(%query_id, "Advancing existing query");
        let (next_batch, remaining, next_cursor) = {
            let mut entry = self
                .queries
                .get_mut(query_id)
                .ok_or(QueryExecutionFail::Expired)?;
            let (next_batch, next_cursor) = entry.live_query.next_batch(cursor.get())?;
            let remaining = entry.live_query.remaining();
            entry.last_access_time = Instant::now();
            (next_batch, remaining, next_cursor)
        };
        if next_cursor.is_none() {
            self.remove(query_id);
        }
        Ok((next_batch, remaining, next_cursor))
    }

    fn check_capacity(&self, authority: &AccountId) -> Result<(), QueryExecutionFail> {
        if self.queries.len() >= self.capacity.get() {
            warn!(
                max_queries = self.capacity,
                "Reached maximum allowed number of queries in LiveQueryStore"
            );
            return Err(QueryExecutionFail::CapacityLimit);
        }
        if self
            .queries_per_user
            .get(authority)
            .is_some_and(|value| *value >= self.capacity_per_user.get())
        {
            warn!(
                max_queries_per_user = self.capacity_per_user,
                %authority,
                "Account reached maximum allowed number of queries in LiveQueryStore"
            );
            return Err(QueryExecutionFail::AuthorityQuotaExceeded);
        }
        Ok(())
    }
}

/// Handle to interact with [`LiveQueryStore`].
#[derive(Clone)]
pub struct LiveQueryStoreHandle {
    store: Arc<LiveQueryStore>,
}

impl LiveQueryStoreHandle {
    /// Create a new handle for the store
    pub fn new(store: Arc<LiveQueryStore>) -> Self {
        Self { store }
    }

    /// Construct a batched response from a post-processed query output.
    ///
    /// # Parameters
    /// * `gas_budget` — optional budget hint that will be echoed in the returned cursor.
    ///
    /// # Errors
    ///
    /// - Returns [`QueryExecutionFail::CapacityLimit`] if [`LiveQueryStore`] capacity is reached.
    /// - Returns [`QueryExecutionFail::AuthorityQuotaExceeded`] when the per-authority quota is reached.
    /// - Otherwise throws up query output handling errors.
    pub fn handle_iter_start(
        &self,
        mut live_query: ErasedQueryIterator,
        authority: &AccountId,
        gas_budget: Option<u64>,
    ) -> Result<QueryOutput, QueryExecutionFail> {
        let query_id = uuid::Uuid::new_v4().to_string();

        let curr_cursor = 0;
        let (batch, next_cursor) = live_query.next_batch(curr_cursor)?;

        // NOTE: we are checking remaining items _after_ the first batch is taken
        let remaining_items = live_query.remaining();

        // if the cursor is `None` - the query has ended, we can remove it from the store
        if next_cursor.is_some() {
            self.store
                .insert_new_query(query_id.clone(), live_query, authority.clone())?;
        }
        Ok(Self::construct_query_response(
            batch,
            remaining_items,
            query_id,
            next_cursor,
            gas_budget,
        ))
    }

    /// Start an iterable query without storing it in the `LiveQueryStore`.
    ///
    /// Returns the first batch and the remaining items; the cursor is always `None`.
    /// Intended for short-lived contexts (e.g., inside smart contracts) where
    /// cursors must not be reusable after the context ends.
    /// # Errors
    /// Returns an error if the first batch cannot be produced by the iterator.
    pub fn handle_iter_start_ephemeral(
        &self,
        mut live_query: ErasedQueryIterator,
    ) -> Result<QueryOutput, QueryExecutionFail> {
        let curr_cursor = 0;
        let (batch, _next_cursor) = live_query.next_batch(curr_cursor)?;
        let remaining_items = live_query.remaining();
        Ok(QueryOutput::new(batch, remaining_items, None))
    }

    /// Retrieve next batch of query output using `cursor`.
    ///
    /// # Errors
    ///
    /// - Returns an [`QueryExecutionFail`] if the query id is not found,
    ///   or if cursor position doesn't match or cannot continue.
    pub fn handle_iter_continue(
        &self,
        ForwardCursor {
            query,
            cursor,
            gas_budget,
        }: ForwardCursor,
    ) -> Result<QueryOutput, QueryExecutionFail> {
        let (batch, remaining, next_cursor) = self.store.get_query_next_batch(&query, cursor)?;

        Ok(Self::construct_query_response(
            batch,
            remaining,
            query,
            next_cursor,
            gas_budget,
        ))
    }

    /// Remove query from the storage if there is any.
    pub fn drop_query(&self, query_id: &QueryId) {
        self.store.remove(query_id);
    }

    fn construct_query_response(
        batch: QueryOutputBatchBoxTuple,
        remaining_items: u64,
        query_id: QueryId,
        cursor: Option<NonZeroU64>,
        gas_budget: Option<u64>,
    ) -> QueryOutput {
        QueryOutput::new(
            batch,
            remaining_items,
            cursor.map(|cursor| ForwardCursor {
                query: query_id,
                cursor,
                gas_budget,
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU64, time::Duration};

    use iroha_data_model::{
        permission::Permission,
        prelude::SelectorTuple,
        query::{
            error::QueryExecutionFail,
            parameters::{FetchSize, Pagination, QueryParams, Sorting},
        },
    };
    use iroha_primitives::json::Json;
    use iroha_test_samples::ALICE_ID;
    use nonzero_ext::nonzero;

    use super::*;
    use crate::smartcontracts::isi::query::QueryLimits;

    #[test]
    fn query_message_order_preserved() {
        let threaded_rt = tokio::runtime::Runtime::new().unwrap();
        let query_handle = threaded_rt.block_on(async { LiveQueryStore::start_test() });

        for i in 0..10_000 {
            let pagination = Pagination::default();
            let fetch_size = FetchSize {
                fetch_size: Some(nonzero!(1_u64)),
            };
            let sorting = Sorting::default();

            let query_params = QueryParams {
                pagination,
                sorting,
                fetch_size,
            };

            // it's not important which type we use here, just to test the flow
            let query_output =
                (0..100).map(|_| Permission::new(String::default(), Json::from(false)));
            let query_output = crate::smartcontracts::query::apply_query_postprocessing(
                query_output,
                SelectorTuple::default(),
                &query_params,
                QueryLimits::default(),
            )
            .unwrap();

            let (batch, _remaining_items, mut current_cursor) = query_handle
                .handle_iter_start(query_output, &ALICE_ID, None)
                .unwrap()
                .into_parts();

            let mut counter = 0;
            counter += batch.len();

            while let Some(cursor) = current_cursor {
                let Ok(batched) = query_handle.handle_iter_continue(cursor) else {
                    break;
                };
                let (batch, _remaining_items, cursor) = batched.into_parts();
                counter += batch.len();

                current_cursor = cursor;
            }

            assert_eq!(counter, 100, "failed on {i} iteration");
        }
    }

    #[test]
    fn cursor_ttl_eviction_returns_expired_error() {
        let config = Config {
            idle_time: Duration::from_millis(1),
            capacity: nonzero!(4_usize),
            capacity_per_user: nonzero!(4_usize),
        };
        let store = Arc::new(LiveQueryStore::from_config(config, ShutdownSignal::new()));
        let handle = LiveQueryStoreHandle::new(Arc::clone(&store));

        let query_output = (0..3).map(|i| Permission::new(format!("p{i}"), Json::from(false)));
        let query_params = QueryParams {
            fetch_size: FetchSize {
                fetch_size: Some(nonzero!(1_u64)),
            },
            ..QueryParams::default()
        };
        let query_output = crate::smartcontracts::query::apply_query_postprocessing(
            query_output,
            SelectorTuple::default(),
            &query_params,
            QueryLimits::default(),
        )
        .unwrap();

        let response = handle
            .handle_iter_start(query_output, &ALICE_ID, Some(10))
            .unwrap();
        let (_batch, _remaining, cursor) = response.into_parts();
        let mut cursor = cursor.expect("cursor stored");

        // Age the query beyond idle_time to trigger eviction.
        if let Some(mut entry) = store.queries.get_mut(cursor.query()) {
            let now = Instant::now();
            let drift = config
                .idle_time
                .checked_add(Duration::from_millis(1))
                .unwrap_or(config.idle_time);
            entry.last_access_time = now.checked_sub(drift).unwrap_or(now);
        }
        store.prune_expired_queries();

        cursor.gas_budget = Some(10);
        let err = handle.handle_iter_continue(cursor).expect_err("expired");
        assert_eq!(err, QueryExecutionFail::Expired);
    }

    #[test]
    fn per_authority_quota_is_enforced() {
        let config = Config {
            idle_time: Duration::from_secs(60),
            capacity: nonzero!(4_usize),
            capacity_per_user: nonzero!(1_usize),
        };
        let store = Arc::new(LiveQueryStore::from_config(config, ShutdownSignal::new()));
        let handle = LiveQueryStoreHandle::new(store);

        let build_iter = || {
            let query_output = (0..2).map(|_| Permission::new(String::default(), Json::from(true)));
            let query_params = QueryParams {
                fetch_size: FetchSize {
                    fetch_size: Some(nonzero!(1_u64)),
                },
                ..QueryParams::default()
            };
            crate::smartcontracts::query::apply_query_postprocessing(
                query_output,
                SelectorTuple::default(),
                &query_params,
                QueryLimits::default(),
            )
            .unwrap()
        };

        handle
            .handle_iter_start(build_iter(), &ALICE_ID, Some(5))
            .unwrap();
        let err = handle
            .handle_iter_start(build_iter(), &ALICE_ID, Some(5))
            .expect_err("quota");
        assert_eq!(err, QueryExecutionFail::AuthorityQuotaExceeded);
    }

    #[test]
    fn stored_cursor_echoes_budget_hint() {
        let handle = LiveQueryStore::start_test();
        let query_output = (0..2).map(|_| Permission::new(String::default(), Json::from(false)));
        let query_params = QueryParams {
            fetch_size: FetchSize {
                fetch_size: Some(nonzero!(1_u64)),
            },
            ..QueryParams::default()
        };
        let query_output = crate::smartcontracts::query::apply_query_postprocessing(
            query_output,
            SelectorTuple::default(),
            &query_params,
            QueryLimits::default(),
        )
        .unwrap();

        let (_batch, _remaining, cursor) = handle
            .handle_iter_start(query_output, &ALICE_ID, Some(42))
            .unwrap()
            .into_parts();
        let cursor = cursor.expect("cursor present");
        assert_eq!(cursor.gas_budget, Some(42));
    }

    #[test]
    fn cursor_mismatch_does_not_evict_query() {
        let handle = LiveQueryStore::start_test();
        let query_output = (0..2).map(|i| Permission::new(format!("p{i}"), Json::from(false)));
        let query_params = QueryParams {
            fetch_size: FetchSize {
                fetch_size: Some(nonzero!(1_u64)),
            },
            ..QueryParams::default()
        };
        let query_output = crate::smartcontracts::query::apply_query_postprocessing(
            query_output,
            SelectorTuple::default(),
            &query_params,
            QueryLimits::default(),
        )
        .unwrap();

        let (_batch, _remaining, cursor) = handle
            .handle_iter_start(query_output, &ALICE_ID, Some(7))
            .unwrap()
            .into_parts();
        let cursor = cursor.expect("cursor present");

        let mut bad_cursor = cursor.clone();
        bad_cursor.cursor =
            NonZeroU64::new(cursor.cursor.get().saturating_add(1)).expect("non-zero");
        let err = handle
            .handle_iter_continue(bad_cursor)
            .expect_err("mismatch");
        assert_eq!(err, QueryExecutionFail::CursorMismatch);

        let (batch, remaining, next) = handle
            .handle_iter_continue(cursor)
            .expect("cursor still valid")
            .into_parts();
        assert_eq!(batch.len(), 1);
        assert_eq!(remaining, 0);
        assert!(next.is_none(), "query should be drained");
    }
}
