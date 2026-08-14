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
        continue_cursor: Option<E::Cursor>,
    ) -> Result<Self, TypedBatchDowncastError> {
        let batch_iter = T::downcast(first_batch)?;
        Ok(Self {
            current_batch_iter: batch_iter,
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
    E::Error: From<TypedBatchDowncastError>,
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
            let (batch, _remaining_items, cursor) = match E::continue_query(cursor) {
                Ok(r) => r,
                Err(error) => return Some(Err(error)),
            };
            // Treat a malformed or schema-incompatible response as a terminal query error.
            // Continuing from a response whose columns cannot be interpreted would risk
            // silently skipping rows or combining data from incompatible schemas.
            let batch_iter = match T::downcast(batch) {
                Ok(batch_iter) => batch_iter,
                Err(error) => {
                    self.continue_cursor = None;
                    return Some(Err(error.into()));
                }
            };
            self.current_batch_iter = batch_iter;
            self.continue_cursor = cursor;
            // Loop and attempt to yield from the refreshed batch.
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let current_batch_len = self.current_batch_iter.len();
        if self.continue_cursor.is_some() {
            // Remote counts are untrusted and a continuation may fail or violate its
            // advertised count. Only already-decoded rows are guaranteed.
            return (current_batch_len, None);
        }
        (current_batch_len, Some(current_batch_len))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::QueryOutputBatchBox;
    use iroha_primitives::numeric::Numeric;
    // A dummy executor that returns a configurable number of empty batches
    // before finally returning a single-item batch and then terminating.
    struct DummyExec;
    impl QueryExecutor for DummyExec {
        type Cursor = usize; // how many empty batches left
        type Error = TypedBatchDowncastError;
        fn execute_singular_query(
            &self,
            _query: crate::query::SingularQueryBox,
        ) -> Result<crate::query::SingularQueryOutputBox, Self::Error> {
            unreachable!()
        }
        fn start_query(
            &self,
            _query: crate::query::QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, Option<u64>, Option<Self::Cursor>), Self::Error>
        {
            unreachable!()
        }
        fn continue_query(
            cursor: Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, Option<u64>, Option<Self::Cursor>), Self::Error>
        {
            if cursor > 0 {
                Ok((
                    QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Numeric(vec![])),
                    Some(1),
                    Some(cursor - 1),
                ))
            } else {
                Ok((
                    QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Numeric(vec![
                        Numeric::new(42, 0),
                    ])),
                    Some(0),
                    None,
                ))
            }
        }
    }
    #[test]
    fn iterator_handles_many_empty_batches_without_recursion() {
        // First batch is empty, but there are 64 empty batches to skip via cursor before one item appears.
        let first = QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Numeric(vec![]));
        let mut iter = QueryIterator::<DummyExec, Numeric>::new(first, Some(64))
            .expect("downcast should succeed");
        let item = iter.next().expect("some result").expect("ok result");
        assert_eq!(item, Numeric::new(42, 0));
        // Now iterator should be exhausted
        assert!(iter.next().is_none());
    }
    #[test]
    fn iterator_size_hint_has_no_upper_bound_while_cursor_exists() {
        let first = QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Numeric(vec![
            Numeric::new(1, 0),
            Numeric::new(2, 0),
        ]));
        let iter = QueryIterator::<DummyExec, Numeric>::new(first, Some(0))
            .expect("downcast should succeed");
        assert_eq!(iter.size_hint(), (2, None));
    }
    #[test]
    fn iterator_size_hint_is_exact_without_cursor() {
        let first = QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Numeric(vec![
            Numeric::new(1, 0),
            Numeric::new(2, 0),
        ]));
        let iter =
            QueryIterator::<DummyExec, Numeric>::new(first, None).expect("initial batch matches");
        assert_eq!(iter.size_hint(), (2, Some(2)));
    }
    struct HostileExec;
    impl QueryExecutor for HostileExec {
        type Cursor = ();
        type Error = TypedBatchDowncastError;
        fn execute_singular_query(
            &self,
            _query: crate::query::SingularQueryBox,
        ) -> Result<crate::query::SingularQueryOutputBox, Self::Error> {
            unreachable!()
        }
        fn start_query(
            &self,
            _query: crate::query::QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, Option<u64>, Option<Self::Cursor>), Self::Error>
        {
            unreachable!()
        }
        fn continue_query(
            (): Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, Option<u64>, Option<Self::Cursor>), Self::Error>
        {
            Ok((
                QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::String(vec![
                    "hostile".to_owned(),
                ])),
                Some(0),
                Some(()),
            ))
        }
    }
    #[test]
    fn iterator_returns_terminal_error_for_hostile_continuation_type() {
        let first = QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Numeric(vec![]));
        let mut iter = QueryIterator::<HostileExec, Numeric>::new(first, Some(()))
            .expect("initial batch matches");
        assert_eq!(
            iter.next(),
            Some(Err(TypedBatchDowncastError::WrongType { column: 0 }))
        );
        assert_eq!(iter.size_hint(), (0, Some(0)));
        assert_eq!(iter.next(), None, "hostile response must terminate cursor");
    }
    struct FailingExec;
    impl QueryExecutor for FailingExec {
        type Cursor = ();
        type Error = TypedBatchDowncastError;
        fn execute_singular_query(
            &self,
            _query: crate::query::SingularQueryBox,
        ) -> Result<crate::query::SingularQueryOutputBox, Self::Error> {
            unreachable!()
        }
        fn start_query(
            &self,
            _query: crate::query::QueryWithParams,
        ) -> Result<(QueryOutputBatchBoxTuple, Option<u64>, Option<Self::Cursor>), Self::Error>
        {
            unreachable!()
        }
        fn continue_query(
            (): Self::Cursor,
        ) -> Result<(QueryOutputBatchBoxTuple, Option<u64>, Option<Self::Cursor>), Self::Error>
        {
            Err(TypedBatchDowncastError::WrongType { column: 0 })
        }
    }
    #[test]
    fn size_hint_never_promises_remote_rows_that_can_be_replaced_by_error() {
        let first = QueryOutputBatchBoxTuple::from_batch(QueryOutputBatchBox::Numeric(vec![]));
        let mut iter = QueryIterator::<FailingExec, Numeric>::new(first, Some(()))
            .expect("initial batch matches");
        assert_eq!(iter.size_hint(), (0, None));
        assert_eq!(
            iter.next(),
            Some(Err(TypedBatchDowncastError::WrongType { column: 0 }))
        );
        assert_eq!(iter.size_hint(), (0, Some(0)));
        assert_eq!(iter.next(), None);
    }
}
