//! Module for cursor-based pagination functionality.
use iroha_data_model::{
    prelude::SelectorTuple,
    query::{
        QueryOutputBatchBox, QueryOutputBatchBoxTuple,
        dsl::{HasProjection, SelectorMarker},
        error::QueryExecutionFail,
    },
};
use std::{fmt::Debug, iter::Peekable, num::NonZeroU64};
trait BatchedTrait {
    fn next_batch(
        &mut self,
        cursor: u64,
    ) -> Result<(QueryOutputBatchBoxTuple, Option<NonZeroU64>), QueryExecutionFail>;
    fn remaining(&self) -> Option<u64>;
}
struct BatchedInner<I>
where
    I: ExactSizeIterator + Send + Sync,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync,
{
    iter: I,
    batch_size: NonZeroU64,
    cursor: Option<u64>,
}
impl<I> BatchedTrait for BatchedInner<I>
where
    I: ExactSizeIterator + Send + Sync,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
    QueryOutputBatchBox: From<Vec<I::Item>>,
{
    fn next_batch(
        &mut self,
        cursor: u64,
    ) -> Result<(QueryOutputBatchBoxTuple, Option<NonZeroU64>), QueryExecutionFail> {
        let Some(server_cursor) = self.cursor else {
            // the server is done with the iterator
            return Err(QueryExecutionFail::CursorDone);
        };
        if cursor != server_cursor {
            // the cursor doesn't match
            return Err(QueryExecutionFail::CursorMismatch);
        }
        let mut current_batch_size: usize = 0;
        let batch: Vec<I::Item> = self
            .iter
            .by_ref()
            .inspect(|_| current_batch_size += 1)
            .take(
                self.batch_size
                    .get()
                    .try_into()
                    .expect("`u32` should always fit into `usize`"),
            )
            .collect();
        let batch = QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::from(batch));
        // determine if there are elements left after this batch
        let remaining_after = self.iter.len();
        if remaining_after > 0 {
            // continue with the advanced cursor position
            let batch_len =
                u64::try_from(current_batch_size).expect("batch size must fit into u64");
            self.cursor = Some(cursor.strict_add(batch_len));
        } else {
            // iterator drained
            self.cursor = None;
        }
        Ok((
            batch,
            self.cursor
                .map(|cursor| NonZeroU64::new(cursor).expect("Cursor is never 0")),
        ))
    }
    fn remaining(&self) -> Option<u64> {
        Some(self.iter.len() as u64)
    }
}
struct StreamingBatchedInner<I>
where
    I: Iterator + Send + Sync,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync,
{
    iter: Peekable<I>,
    batch_size: NonZeroU64,
    cursor: Option<u64>,
}
impl<I> BatchedTrait for StreamingBatchedInner<I>
where
    I: Iterator + Send + Sync,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
    QueryOutputBatchBox: From<Vec<I::Item>>,
{
    fn next_batch(
        &mut self,
        cursor: u64,
    ) -> Result<(QueryOutputBatchBoxTuple, Option<NonZeroU64>), QueryExecutionFail> {
        let Some(server_cursor) = self.cursor else {
            return Err(QueryExecutionFail::CursorDone);
        };
        if cursor != server_cursor {
            return Err(QueryExecutionFail::CursorMismatch);
        }
        let mut current_batch_size: usize = 0;
        let batch: Vec<I::Item> = self
            .iter
            .by_ref()
            .inspect(|_| current_batch_size += 1)
            .take(
                self.batch_size
                    .get()
                    .try_into()
                    .expect("`u32` should always fit into `usize`"),
            )
            .collect();
        let batch = QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::from(batch));
        if self.iter.peek().is_some() {
            let batch_len =
                u64::try_from(current_batch_size).expect("batch size must fit into u64");
            self.cursor = Some(cursor.strict_add(batch_len));
        } else {
            self.cursor = None;
        }
        Ok((
            batch,
            self.cursor
                .map(|cursor| NonZeroU64::new(cursor).expect("Cursor is never 0")),
        ))
    }
    fn remaining(&self) -> Option<u64> {
        None
    }
}
/// A query output iterator that combines batching and type erasure.
///
/// Selectors have a single data-free layout and never project, so every batch is one column of
/// whole items.
pub struct ErasedQueryIterator {
    inner: Box<dyn BatchedTrait + Send + Sync>,
}
impl Debug for ErasedQueryIterator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryBatchedErasedIterator").finish()
    }
}
impl ErasedQueryIterator {
    /// Creates a new erased query iterator. Boxes the inner iterator to erase its type.
    ///
    /// The selector only binds the item type: it has a single data-free layout and never projects.
    pub fn new<I>(iter: I, selector: SelectorTuple<I::Item>, batch_size: NonZeroU64) -> Self
    where
        I: ExactSizeIterator + Send + Sync + 'static,
        I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
        QueryOutputBatchBox: From<Vec<I::Item>>,
    {
        Self::new_with_cursor(iter, selector, batch_size, 0)
    }
    /// Creates a new erased query iterator with a custom initial cursor value.
    pub(crate) fn new_with_cursor<I>(
        iter: I,
        _selector: SelectorTuple<I::Item>,
        batch_size: NonZeroU64,
        initial_cursor: u64,
    ) -> Self
    where
        I: ExactSizeIterator + Send + Sync + 'static,
        I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
        QueryOutputBatchBox: From<Vec<I::Item>>,
    {
        Self {
            inner: Box::new(BatchedInner {
                iter,
                batch_size,
                cursor: Some(initial_cursor),
            }),
        }
    }
    /// Creates an erased query iterator for an iterator that cannot cheaply
    /// report an exact remaining length.
    pub(crate) fn new_streaming_with_cursor<I>(
        iter: I,
        _selector: SelectorTuple<I::Item>,
        batch_size: NonZeroU64,
        initial_cursor: u64,
    ) -> Self
    where
        I: Iterator + Send + Sync + 'static,
        I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
        QueryOutputBatchBox: From<Vec<I::Item>>,
    {
        Self {
            inner: Box::new(StreamingBatchedInner {
                iter: iter.peekable(),
                batch_size,
                cursor: Some(initial_cursor),
            }),
        }
    }
    /// Gets the next batch of results.
    ///
    /// Checks if the cursor matches the server's cursor.
    ///
    /// Returns the batch and the next cursor if the query iterator is not drained.
    ///
    /// # Errors
    ///
    /// - The cursor doesn't match the server's cursor.
    /// - There aren't enough items for the cursor.
    pub fn next_batch(
        &mut self,
        cursor: u64,
    ) -> Result<(QueryOutputBatchBoxTuple, Option<NonZeroU64>), QueryExecutionFail> {
        self.inner.next_batch(cursor)
    }
    /// Returns the number of remaining elements in the iterator.
    ///
    /// You should not rely on the reported amount being correct for safety, same as [`ExactSizeIterator::len`].
    pub fn remaining(&self) -> Option<u64> {
        self.inner.remaining()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_model_base::domain::DomainId;
    use nonzero_ext::nonzero;
    #[test]
    fn empty_selector_projects_full_items() {
        let d1: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let d2: DomainId = DomainId::try_new("underland", "universal").unwrap();
        let items = vec![d1.clone(), d2.clone()];
        let mut it = ErasedQueryIterator::new(
            items.clone().into_iter(),
            SelectorTuple::<DomainId>::default(),
            nonzero!(10_u64),
        );
        let (batch_tuple, next) = it.next_batch(0).expect("batch");
        assert!(next.is_none(), "one batch only");
        assert_eq!(
            batch_tuple.column_count(),
            1,
            "single tuple element for full projection"
        );
        match batch_tuple.columns().first().expect("one projected column") {
            QueryOutputBatchBox::DomainId(v) => {
                assert_eq!(v.len(), 2);
                assert_eq!(v[0], d1);
                assert_eq!(v[1], d2);
            }
            other => panic!("unexpected batch variant: {other:?}"),
        }
    }
    #[test]
    fn cursor_mismatch_and_done_paths() {
        let d1: DomainId = DomainId::try_new("alpha", "universal").unwrap();
        let d2: DomainId = DomainId::try_new("beta", "universal").unwrap();
        let d3: DomainId = DomainId::try_new("gamma", "universal").unwrap();
        let items = vec![d1, d2, d3];
        let mut it = ErasedQueryIterator::new(
            items.into_iter(),
            SelectorTuple::<DomainId>::default(),
            nonzero!(2_u64),
        );
        assert_eq!(it.remaining(), Some(3));
        // First batch with correct cursor
        let (b1, next) = it.next_batch(0).expect("first batch");
        assert_eq!(b1.len(), 2);
        let cur = next.expect("next cursor").get();
        // Mismatch
        let err = it.next_batch(1).unwrap_err();
        assert!(matches!(err, QueryExecutionFail::CursorMismatch));
        // Second batch with correct cursor
        let (b2, next2) = it.next_batch(cur).expect("second batch");
        assert_eq!(b2.len(), 1);
        assert!(next2.is_none(), "drained");
        assert_eq!(it.remaining(), Some(0));
        // Done path
        let err = it.next_batch(cur).unwrap_err();
        assert!(matches!(err, QueryExecutionFail::CursorDone));
    }
}
