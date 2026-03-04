use crate::query::{
    QueryOutputBatchBoxTuple,
    builder::{
        QueryExecutor,
        batch_downcast::{HasTypedBatchIter, TypedBatchDowncastError},
    },
};

/// An iterator over results of an iterable query.
#[derive(Debug)]
pub struct QueryIterator<E: QueryExecutor, T: HasTypedBatchIter> {
    current_batch_iter: T::TypedBatchIter,
    remaining_items: u64,
    continue_cursor: Option<E::Cursor>,
}

impl<E, T> QueryIterator<E, T>
where
    E: QueryExecutor,
    T: HasTypedBatchIter,
{
    /// Create a new iterator over iterable query results.
    ///
    /// # Errors
    ///
    /// Returns an error if the type of the batch does not match the expected type `T`.
    pub fn new(
        first_batch: QueryOutputBatchBoxTuple,
        remaining_items: u64,
        continue_cursor: Option<E::Cursor>,
    ) -> Result<Self, TypedBatchDowncastError> {
        let batch_iter = T::downcast(first_batch)?;

        Ok(Self {
            current_batch_iter: batch_iter,
            remaining_items,
            continue_cursor,
        })
    }

    /// Returns the cursor for the next batch, if available.
    pub fn continue_cursor(&self) -> Option<&E::Cursor> {
        self.continue_cursor.as_ref()
    }
}

impl<E, T> Iterator for QueryIterator<E, T>
where
    E: QueryExecutor,
    T: HasTypedBatchIter,
{
    type Item = Result<T, E::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        // Keep fetching next batches until we either return an item,
        // encounter an error, or reach the end (no cursor).
        loop {
            // If we haven't exhausted the current batch yet - return its next item.
            if let Some(item) = self.current_batch_iter.next() {
                return Some(Ok(item));
            }

            // No cursor means the query result is exhausted or an error occurred on a previous iteration.
            let cursor = self.continue_cursor.take()?;

            // Get the next batch from the executor.
            let (batch, remaining_items, cursor) = match E::continue_query(cursor) {
                Ok(r) => r,
                Err(e) => return Some(Err(e)),
            };
            self.continue_cursor = cursor;

            // Downcast the batch to the expected type.
            // We've already downcast the first batch to the expected type, so if the executor
            // returns a different type here, it surely is a bug.
            let batch_iter =
                T::downcast(batch).expect("BUG: iroha returned unexpected type in iterable query");

            self.current_batch_iter = batch_iter;
            self.remaining_items = remaining_items;
            // Loop and attempt to yield from the refreshed batch.
        }
    }
}

impl<E, T> ExactSizeIterator for QueryIterator<E, T>
where
    E: QueryExecutor,
    T: HasTypedBatchIter,
{
    fn len(&self) -> usize {
        self.remaining_items
            .try_into()
            .ok()
            .and_then(|r: usize| r.checked_add(self.current_batch_iter.len()))
            .expect("should be within the range of usize")
    }
}

#[cfg(test)]
mod tests {
    use iroha_primitives::numeric::Numeric;

    use super::*;
    use crate::query::QueryOutputBatchBox;

    // A dummy executor that returns a configurable number of empty batches
    // before finally returning a single-item batch and then terminating.
    struct DummyExec;

    impl QueryExecutor for DummyExec {
        type Cursor = usize; // how many empty batches left
        type Error = ();

        fn execute_singular_query(
            &self,
            _query: crate::query::SingularQueryBox,
        ) -> Result<crate::query::SingularQueryOutputBox, Self::Error> {
            unreachable!()
        }

        fn start_query(
            &self,
            _query: crate::query::QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            unreachable!()
        }

        fn continue_query(
            cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, u64, Option<Self::Cursor>), Self::Error> {
            if cursor > 0 {
                Ok((
                    QueryOutputBatchBoxTuple {
                        tuple: vec![QueryOutputBatchBox::Numeric(vec![])],
                    },
                    1,
                    Some(cursor - 1),
                ))
            } else {
                Ok((
                    QueryOutputBatchBoxTuple {
                        tuple: vec![QueryOutputBatchBox::Numeric(vec![Numeric::new(42, 0)])],
                    },
                    0,
                    None,
                ))
            }
        }
    }

    #[test]
    fn iterator_handles_many_empty_batches_without_recursion() {
        // First batch is empty, but there are 64 empty batches to skip via cursor before one item appears.
        let first = QueryOutputBatchBoxTuple {
            tuple: vec![QueryOutputBatchBox::Numeric(vec![])],
        };
        let mut iter = QueryIterator::<DummyExec, Numeric>::new(first, 1, Some(64))
            .expect("downcast should succeed");

        let item = iter.next().expect("some result").expect("ok result");
        assert_eq!(item, Numeric::new(42, 0));
        // Now iterator should be exhausted
        assert!(iter.next().is_none());
    }
}
