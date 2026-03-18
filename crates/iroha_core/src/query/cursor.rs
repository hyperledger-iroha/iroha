//! Module for cursor-based pagination functionality.

use std::{fmt::Debug, num::NonZeroU64};

use iroha_data_model::{
    prelude::SelectorTuple,
    query::{
        QueryOutputBatchBox, QueryOutputBatchBoxTuple,
        dsl::{EvaluateSelector, HasProjection, SelectorMarker},
        error::QueryExecutionFail,
    },
};

fn evaluate_selector_tuple<T>(
    batch: Vec<T>,
    selector: &SelectorTuple<T>,
) -> Result<QueryOutputBatchBoxTuple, QueryExecutionFail>
where
    T: HasProjection<SelectorMarker, AtomType = ()> + 'static,
    T::Projection: EvaluateSelector<T>,
    QueryOutputBatchBox: From<Vec<T>>,
{
    let mut batch_tuple = Vec::new();

    let mut iter = selector.iter().peekable();

    // If no projection was requested, return the entire items as a single tuple element
    if iter.peek().is_none() {
        return Ok(QueryOutputBatchBoxTuple {
            tuple: vec![QueryOutputBatchBox::from(batch)],
        });
    }

    while let Some(item) = iter.next() {
        if iter.peek().is_none() {
            // do not clone the last item
            batch_tuple.push(item.project(batch.into_iter())?);
            return Ok(QueryOutputBatchBoxTuple { tuple: batch_tuple });
        }

        batch_tuple.push(item.project_clone(batch.iter())?);
    }

    // unreachable: handled empty selector above
    Ok(QueryOutputBatchBoxTuple { tuple: batch_tuple })
}

trait BatchedTrait {
    fn next_batch(
        &mut self,
        cursor: u64,
    ) -> Result<(QueryOutputBatchBoxTuple, Option<NonZeroU64>), QueryExecutionFail>;
    fn remaining(&self) -> u64;
}

struct BatchedInner<I>
where
    I: ExactSizeIterator + Send + Sync,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync,
{
    iter: I,
    selector: SelectorTuple<I::Item>,
    batch_size: NonZeroU64,
    cursor: Option<u64>,
}

impl<I> BatchedTrait for BatchedInner<I>
where
    I: ExactSizeIterator + Send + Sync,
    I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
    <I::Item as HasProjection<SelectorMarker>>::Projection: EvaluateSelector<I::Item> + Send + Sync,
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

        // evaluate the requested projections
        let batch = evaluate_selector_tuple(batch, &self.selector)?;

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

    fn remaining(&self) -> u64 {
        self.iter.len() as u64
    }
}

/// A query output iterator that combines evaluating selectors, batching and type erasure.
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
    pub fn new<I>(iter: I, selector: SelectorTuple<I::Item>, batch_size: NonZeroU64) -> Self
    where
        I: ExactSizeIterator + Send + Sync + 'static,
        I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
        <I::Item as HasProjection<SelectorMarker>>::Projection:
            EvaluateSelector<I::Item> + Send + Sync,
        QueryOutputBatchBox: From<Vec<I::Item>>,
    {
        Self::new_with_cursor(iter, selector, batch_size, 0)
    }

    /// Creates a new erased query iterator with a custom initial cursor value.
    pub(crate) fn new_with_cursor<I>(
        iter: I,
        selector: SelectorTuple<I::Item>,
        batch_size: NonZeroU64,
        initial_cursor: u64,
    ) -> Self
    where
        I: ExactSizeIterator + Send + Sync + 'static,
        I::Item: HasProjection<SelectorMarker, AtomType = ()> + Send + Sync + 'static,
        <I::Item as HasProjection<SelectorMarker>>::Projection:
            EvaluateSelector<I::Item> + Send + Sync,
        QueryOutputBatchBox: From<Vec<I::Item>>,
    {
        Self {
            inner: Box::new(BatchedInner {
                iter,
                selector,
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
    pub fn remaining(&self) -> u64 {
        self.inner.remaining()
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::prelude::*;
    use nonzero_ext::nonzero;

    use super::*;

    #[test]
    fn empty_selector_projects_full_items() {
        let d1: DomainId = "wonderland".parse().unwrap();
        let d2: DomainId = "underland".parse().unwrap();
        let items = vec![d1.clone(), d2.clone()];

        let mut it = ErasedQueryIterator::new(
            items.clone().into_iter(),
            SelectorTuple::<DomainId>::default(),
            nonzero!(10_u64),
        );

        let (batch_tuple, next) = it.next_batch(0).expect("batch");
        assert!(next.is_none(), "one batch only");
        assert_eq!(
            batch_tuple.tuple.len(),
            1,
            "single tuple element for full projection"
        );
        match &batch_tuple.tuple[0] {
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
        let d1: DomainId = "alpha".parse().unwrap();
        let d2: DomainId = "beta".parse().unwrap();
        let d3: DomainId = "gamma".parse().unwrap();
        let items = vec![d1, d2, d3];

        let mut it = ErasedQueryIterator::new(
            items.into_iter(),
            SelectorTuple::<DomainId>::default(),
            nonzero!(2_u64),
        );

        assert_eq!(it.remaining(), 3);

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
        assert_eq!(it.remaining(), 0);

        // Done path
        let err = it.next_batch(cur).unwrap_err();
        assert!(matches!(err, QueryExecutionFail::CursorDone));
    }
}
