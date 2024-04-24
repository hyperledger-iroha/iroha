//! This module contains [`LiveQueryStore`] actor.

use std::{
    cmp::Ordering,
    num::NonZeroU64,
    time::{Duration, Instant},
};

use indexmap::IndexMap;
use iroha_config::parameters::actual::LiveQueryStore as Config;
use iroha_data_model::{
    asset::AssetValue,
    query::{
        cursor::ForwardCursor, error::QueryExecutionFail, pagination::Pagination, sorting::Sorting,
        FetchSize, QueryId, QueryOutputBox, DEFAULT_FETCH_SIZE, MAX_FETCH_SIZE,
    },
    BatchedResponse, BatchedResponseV1, HasMetadata, IdentifiableBox, ValidationFail,
};
use iroha_logger::trace;
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};

use super::{
    cursor::{Batch as _, Batched, UnknownCursor},
    pagination::Paginate as _,
};
use crate::smartcontracts::query::LazyQueryOutput;

/// Query service error.
#[derive(Debug, thiserror::Error, Copy, Clone, Serialize, Deserialize, Encode, Decode)]
pub enum Error {
    /// Unknown cursor error.
    #[error(transparent)]
    UnknownCursor(#[from] UnknownCursor),
    /// Connection with `LiveQueryStore` is closed.
    #[error("Connection with LiveQueryStore is closed")]
    ConnectionClosed,
    /// Fetch size is too big.
    #[error("Fetch size is too big")]
    FetchSizeTooBig,
}

#[allow(clippy::fallible_impl_from)]
impl From<Error> for ValidationFail {
    fn from(error: Error) -> Self {
        match error {
            Error::UnknownCursor(_) => {
                ValidationFail::QueryFailed(QueryExecutionFail::UnknownCursor)
            }
            Error::ConnectionClosed => {
                panic!("Connection to `LiveQueryStore` was unexpectedly closed, this is a bug")
            }
            Error::FetchSizeTooBig => {
                ValidationFail::QueryFailed(QueryExecutionFail::FetchSizeTooBig)
            }
        }
    }
}

/// Result type for [`LiveQueryStore`] methods.
pub type Result<T, E = Error> = std::result::Result<T, E>;

type LiveQuery = Batched<Vec<QueryOutputBox>>;

/// Service which stores queries which might be non fully consumed by a client.
///
/// Clients can handle their queries using [`LiveQueryStoreHandle`]
#[derive(Debug)]
pub struct LiveQueryStore {
    queries: IndexMap<QueryId, (LiveQuery, Instant)>,
    idle_time: Duration,
}

impl LiveQueryStore {
    /// Construct [`LiveQueryStore`] from configuration.
    pub fn from_config(cfg: Config) -> Self {
        Self {
            queries: IndexMap::new(),
            idle_time: cfg.idle_time,
        }
    }

    /// Construct [`LiveQueryStore`] for tests.
    /// Default configuration will be used.
    ///
    /// Not marked as `#[cfg(test)]` because it is used in benches as well.
    pub fn test() -> Self {
        Self::from_config(Config::default())
    }

    /// Start [`LiveQueryStore`]. Requires a [`tokio::runtime::Runtime`] being run
    /// as it will create new [`tokio::task`] and detach it.
    ///
    /// Returns a handle to interact with the service.
    pub fn start(mut self) -> LiveQueryStoreHandle {
        const ALL_HANDLERS_DROPPED: &str =
            "All handler to LiveQueryStore are dropped. Shutting down...";

        let (message_sender, mut message_receiver) = mpsc::channel(1);

        let mut idle_interval = tokio::time::interval(self.idle_time);

        tokio::task::spawn(async move {
            loop {
                tokio::select! {
                    _ = idle_interval.tick() => {
                        self.queries
                            .retain(|_, (_, last_access_time)| last_access_time.elapsed() <= self.idle_time);
                    },
                    msg = message_receiver.recv() => {
                        let Some(msg) = msg else {
                            iroha_logger::info!("{ALL_HANDLERS_DROPPED}");
                            break;
                        };

                        match msg {
                            Message::Insert(query_id, live_query) => {
                                self.insert(query_id, live_query)
                            }
                            Message::Remove(query_id, response_sender) => {
                                let live_query_opt = self.remove(&query_id);
                                let _ = response_sender.send(live_query_opt);
                            }
                        }
                    }
                    else => break,
                }
                tokio::task::yield_now().await;
            }
        });

        LiveQueryStoreHandle { message_sender }
    }

    fn insert(&mut self, query_id: QueryId, live_query: LiveQuery) {
        self.queries.insert(query_id, (live_query, Instant::now()));
    }

    fn remove(&mut self, query_id: &str) -> Option<LiveQuery> {
        self.queries.remove(query_id).map(|(output, _)| output)
    }
}

enum Message {
    Insert(QueryId, Batched<Vec<QueryOutputBox>>),
    Remove(
        QueryId,
        oneshot::Sender<Option<Batched<Vec<QueryOutputBox>>>>,
    ),
}

/// Handle to interact with [`LiveQueryStore`].
#[derive(Clone)]
pub struct LiveQueryStoreHandle {
    message_sender: mpsc::Sender<Message>,
}

impl LiveQueryStoreHandle {
    /// Apply sorting and pagination to the query output.
    ///
    /// # Errors
    ///
    /// - Returns [`Error::ConnectionClosed`] if [`LiveQueryStore`] is dropped,
    /// - Otherwise throws up query output handling errors.
    pub fn handle_query_output(
        &self,
        query_output: LazyQueryOutput<'_>,
        sorting: &Sorting,
        pagination: Pagination,
        fetch_size: FetchSize,
    ) -> Result<BatchedResponse<QueryOutputBox>> {
        match query_output {
            LazyQueryOutput::QueryOutput(batch) => {
                let cursor = ForwardCursor::default();
                let result = BatchedResponseV1 { batch, cursor };
                Ok(result.into())
            }
            LazyQueryOutput::Iter(iter) => {
                let fetch_size = fetch_size.fetch_size.unwrap_or(DEFAULT_FETCH_SIZE);
                if fetch_size > MAX_FETCH_SIZE {
                    return Err(Error::FetchSizeTooBig);
                }

                let live_query = Self::apply_sorting_and_pagination(iter, sorting, pagination);
                let query_id = uuid::Uuid::new_v4().to_string();

                let curr_cursor = Some(0);
                let live_query = live_query.batched(fetch_size);
                self.construct_query_response(query_id, curr_cursor, live_query)
            }
        }
    }

    /// Retrieve next batch of query output using `cursor`.
    ///
    /// # Errors
    ///
    /// - Returns [`Error::ConnectionClosed`] if [`LiveQueryStore`] is dropped,
    /// - Otherwise throws up query output handling errors.
    pub fn handle_query_cursor(
        &self,
        cursor: ForwardCursor,
    ) -> Result<BatchedResponse<QueryOutputBox>> {
        let query_id = cursor.query_id.ok_or(UnknownCursor)?;
        let live_query = self.remove(query_id.clone())?.ok_or(UnknownCursor)?;

        self.construct_query_response(query_id, cursor.cursor.map(NonZeroU64::get), live_query)
    }

    /// Remove query from the storage if there is any.
    ///
    /// Returns `true` if query was removed, `false` otherwise.
    ///
    /// # Errors
    ///
    /// - Returns [`Error::ConnectionClosed`] if [`QueryService`] is dropped,
    /// - Otherwise throws up query output handling errors.
    pub fn drop_query(&self, query_id: QueryId) -> Result<bool> {
        self.remove(query_id).map(|query_opt| query_opt.is_some())
    }

    fn insert(&self, query_id: QueryId, live_query: LiveQuery) -> Result<()> {
        trace!(%query_id, "Inserting");
        self.message_sender
            .blocking_send(Message::Insert(query_id, live_query))
            .map_err(|_| Error::ConnectionClosed)
    }

    fn remove(&self, query_id: QueryId) -> Result<Option<LiveQuery>> {
        trace!(%query_id, "Removing");
        let (sender, receiver) = oneshot::channel();

        self.message_sender
            .blocking_send(Message::Remove(query_id, sender))
            .or(Err(Error::ConnectionClosed))?;

        receiver.blocking_recv().or(Err(Error::ConnectionClosed))
    }

    fn construct_query_response(
        &self,
        query_id: QueryId,
        curr_cursor: Option<u64>,
        mut live_query: Batched<Vec<QueryOutputBox>>,
    ) -> Result<BatchedResponse<QueryOutputBox>> {
        let (batch, next_cursor) = live_query.next_batch(curr_cursor)?;

        if !live_query.is_depleted() {
            self.insert(query_id.clone(), live_query)?
        }

        let query_response = BatchedResponseV1 {
            batch: QueryOutputBox::Vec(batch),
            cursor: ForwardCursor {
                query_id: Some(query_id),
                cursor: next_cursor,
            },
        };

        Ok(query_response.into())
    }

    fn apply_sorting_and_pagination(
        iter: impl Iterator<Item = QueryOutputBox>,
        sorting: &Sorting,
        pagination: Pagination,
    ) -> Vec<QueryOutputBox> {
        if let Some(key) = &sorting.sort_by_metadata_key {
            let mut pairs: Vec<(Option<QueryOutputBox>, QueryOutputBox)> = iter
                .map(|value| {
                    let key = match &value {
                        QueryOutputBox::Identifiable(IdentifiableBox::Asset(asset)) => {
                            match asset.value() {
                                AssetValue::Store(store) => store.get(key).cloned().map(Into::into),
                                _ => None,
                            }
                        }
                        QueryOutputBox::Identifiable(v) => TryInto::<&dyn HasMetadata>::try_into(v)
                            .ok()
                            .and_then(|has_metadata| has_metadata.metadata().get(key))
                            .cloned()
                            .map(Into::into),
                        _ => None,
                    };
                    (key, value)
                })
                .collect();
            pairs.sort_by(
                |(left_key, _), (right_key, _)| match (left_key, right_key) {
                    (Some(l), Some(r)) => l.cmp(r),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                },
            );
            pairs
                .into_iter()
                .map(|(_, val)| val)
                .paginate(pagination)
                .collect()
        } else {
            iter.paginate(pagination).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::metadata::MetadataValueBox;
    use nonzero_ext::nonzero;

    use super::*;

    #[test]
    fn query_message_order_preserved() {
        let query_store = LiveQueryStore::test();
        let threaded_rt = tokio::runtime::Runtime::new().unwrap();
        let query_store_handle = threaded_rt.block_on(async { query_store.start() });

        for i in 0..10_000 {
            let pagination = Pagination::default();
            let fetch_size = FetchSize {
                fetch_size: Some(nonzero!(1_u32)),
            };
            let sorting = Sorting::default();

            let query_output = LazyQueryOutput::Iter(Box::new(
                (0..100).map(|_| MetadataValueBox::from(false).into()),
            ));

            let mut counter = 0;

            let (batch, mut cursor) = query_store_handle
                .handle_query_output(query_output, &sorting, pagination, fetch_size)
                .unwrap()
                .into();
            let QueryOutputBox::Vec(v) = batch else {
                panic!("not expected result")
            };
            counter += v.len();

            while cursor.cursor.is_some() {
                let Ok(batched) = query_store_handle.handle_query_cursor(cursor) else {
                    break;
                };
                let (batch, new_cursor) = batched.into();
                let QueryOutputBox::Vec(v) = batch else {
                    panic!("not expected result")
                };
                counter += v.len();

                cursor = new_cursor;
            }

            assert_eq!(counter, 100, "failed on {i} iteration");
        }
    }
}
