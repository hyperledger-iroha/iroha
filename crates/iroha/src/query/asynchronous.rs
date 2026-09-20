//! Account-bound asynchronous queries with one-shot signed requests and typed batches.

use super::*;
use crate::client::AccountClient;
use iroha_data_model::query::{
    ItemKindTag,
    builder::{HasTypedBatchIter, SingleQueryError},
    dsl::{HasProjection, PredicateMarker, SelectorMarker},
};
use std::future::Future;

async fn send_and_decode<T>(
    builder: DefaultRequestBuilder,
    decode: impl FnOnce(&http::Response<Vec<u8>>) -> QueryResult<T>,
) -> QueryResult<T> {
    // The signed nonce may have been consumed on any transport/decode error.
    // Dispatch once; never retry or restart a cursor with the same signed bytes.
    let response = builder.build()?.send().await?;
    decode(&response)
}

impl AccountClient {
    /// Execute one account-authorized singular query asynchronously.
    ///
    /// # Errors
    /// Returns compatibility, signing, transport, server or strict response-shape errors.
    pub async fn query_single<Q>(&self, query: Q) -> QueryResult<Q::Output>
    where
        Q: SingularQuery,
        SingularQueryBox: From<Q>,
        Q::Output: TryFrom<SingularQueryOutputBox>,
        <Q::Output as TryFrom<SingularQueryOutputBox>>::Error: Debug,
    {
        self.client().ensure_query_compatibility().await?;
        let query = SingularQueryBox::from(query);
        let parameters = matches!(query, SingularQueryBox::FindParameters(_));
        let head = self.client().get_query_request_head();
        let body = head.sign_and_encode(QueryRequest::Singular(query))?;
        let builder = if parameters {
            head.assemble_body_with_accept(body, "application/json")
        } else {
            head.assemble_body(body)
        };
        send_and_decode(builder, decode_singular_query_response)
            .await?
            .try_into()
            .map_err(|error| {
                QueryError::Other(eyre!("unexpected singular query output: {error:?}"))
            })
    }

    /// Build an account-authorized iterable query; import [`AsyncQueryBuilderExt`] to execute it.
    #[must_use]
    pub fn query<Q>(&self, query: Q) -> QueryBuilder<'_, Self, Q, Q::Item>
    where
        Q: Query,
    {
        QueryBuilder::new(self, query)
    }

    async fn start_query(
        &self,
        query: QueryWithParams,
    ) -> QueryResult<(QueryOutputBatchBoxTuple, Option<QueryCursor>)> {
        self.client().ensure_query_compatibility().await?;
        validate_fetch_size(
            query
                .params
                .fetch_size
                .fetch_size
                .unwrap_or(DEFAULT_FETCH_SIZE),
        )?;
        let head = self.client().get_query_request_head();
        let body = head.sign_and_encode(QueryRequest::Start(query))?;
        let response =
            send_and_decode(head.assemble_body(body), decode_iterable_query_response).await?;
        Ok(unpack_batch(head, response))
    }
}

fn unpack_batch(
    head: ClientQueryRequestHead,
    response: QueryOutput,
) -> (QueryOutputBatchBoxTuple, Option<QueryCursor>) {
    let (batch, _remaining, _has_more, cursor) = response.into_parts_with_count_mode();
    (
        batch,
        cursor.map(|cursor| QueryCursor {
            request_head: head,
            cursor,
        }),
    )
}

/// An asynchronous typed query result that owns the exact continuation authority.
///
/// A failed or cancelled continuation is terminal: its cursor is consumed before I/O.
/// Empty batches are followed iteratively without trusting advertised remote row counts.
#[derive(Debug)]
pub struct QueryStream<T: HasTypedBatchIter> {
    batch: T::TypedBatchIter,
    cursor: Option<QueryCursor>,
}

impl<T: HasTypedBatchIter> QueryStream<T> {
    fn new(batch: QueryOutputBatchBoxTuple, cursor: Option<QueryCursor>) -> QueryResult<Self> {
        Ok(Self {
            batch: T::downcast(batch)?,
            cursor,
        })
    }

    /// Read the next row, continuing asynchronously when the current batch is exhausted.
    ///
    /// An error ends this result stream; subsequent calls return `None`.
    pub async fn next(&mut self) -> Option<QueryResult<T>> {
        loop {
            if let Some(item) = self.batch.next() {
                return Some(Ok(item));
            }
            let QueryCursor {
                request_head,
                cursor,
            } = self.cursor.take()?;
            let next = async {
                let body = request_head.sign_and_encode(QueryRequest::Continue(cursor))?;
                let response = send_and_decode(
                    request_head.assemble_body(body),
                    decode_iterable_query_response,
                )
                .await?;
                let (batch, cursor) = unpack_batch(request_head, response);
                Ok::<_, QueryError>((T::downcast(batch)?, cursor))
            }
            .await;
            match next {
                Ok((batch, cursor)) => {
                    self.batch = batch;
                    self.cursor = cursor;
                }
                Err(error) => return Some(Err(error)),
            }
        }
    }
}

/// Asynchronous execution for iterable queries built by an immutable account context.
pub trait AsyncQueryBuilderExt<T: HasTypedBatchIter> {
    /// Start the query and validate its first typed batch.
    fn execute(self) -> impl Future<Output = QueryResult<QueryStream<T>>> + Send;
    /// Collect all rows, validating every continuation and stopping on any error.
    fn execute_all(self) -> impl Future<Output = QueryResult<Vec<T>>> + Send;
    /// Require zero or one result across every batch visited.
    fn execute_single_opt(
        self,
    ) -> impl Future<Output = Result<Option<T>, SingleQueryError<QueryError>>> + Send;
    /// Require exactly one result across every batch visited.
    fn execute_single(self)
    -> impl Future<Output = Result<T, SingleQueryError<QueryError>>> + Send;
}

impl<Q, T> AsyncQueryBuilderExt<T> for QueryBuilder<'_, AccountClient, Q, T>
where
    Q: Query
        + HasProjection<PredicateMarker>
        + HasProjection<SelectorMarker, AtomType = ()>
        + norito::codec::Encode
        + 'static,
    Q::Item: Send + Sync + ItemKindTag,
    T: HasTypedBatchIter + HasProjection<PredicateMarker> + Send + 'static,
    T::TypedBatchIter: Send,
{
    fn execute(self) -> impl Future<Output = QueryResult<QueryStream<T>>> + Send {
        // Consume the generic builder before creating the future. Only the
        // concrete request and account reference cross the transport await.
        let (account, request) = self.into_request();
        async move {
            let (batch, cursor) = account.start_query(request).await?;
            QueryStream::new(batch, cursor)
        }
    }

    fn execute_all(self) -> impl Future<Output = QueryResult<Vec<T>>> + Send {
        let query = AsyncQueryBuilderExt::execute(self);
        async move {
            let mut stream = query.await?;
            let mut rows = Vec::new();
            while let Some(row) = stream.next().await {
                rows.push(row?);
            }
            Ok(rows)
        }
    }

    fn execute_single_opt(
        self,
    ) -> impl Future<Output = Result<Option<T>, SingleQueryError<QueryError>>> + Send {
        let query = AsyncQueryBuilderExt::execute(self);
        async move {
            let mut stream = query.await?;
            let first = stream.next().await.transpose()?;
            if stream.next().await.transpose()?.is_some() {
                return Err(SingleQueryError::ExpectedOneOrZeroGotMany);
            }
            Ok(first)
        }
    }

    fn execute_single(
        self,
    ) -> impl Future<Output = Result<T, SingleQueryError<QueryError>>> + Send {
        let query = AsyncQueryBuilderExt::execute(self);
        async move {
            let mut stream = query.await?;
            let first = stream.next().await.transpose()?;
            if stream.next().await.transpose()?.is_some() {
                return Err(SingleQueryError::ExpectedOneGotMany);
            }
            first.ok_or(SingleQueryError::ExpectedOneGotNone)
        }
    }
}

#[cfg(test)]
mod tests;
