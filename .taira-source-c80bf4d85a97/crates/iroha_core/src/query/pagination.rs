//! Module with [`Paginate`] iterator adaptor which provides [`paginate`](Paginate::paginate) method.

use iroha_data_model::query::parameters::Pagination;

/// Describes a collection to which pagination can be applied.
/// Implemented for the [`Iterator`] implementors.
pub trait Paginate: Iterator + Sized {
    /// Return a paginated [`Iterator`].
    fn paginate(self, pagination: Pagination) -> Paginated<Self>;
}

impl<I: Iterator> Paginate for I {
    fn paginate(self, pagination: Pagination) -> Paginated<Self> {
        Paginated::new(pagination, self)
    }
}

/// Paginated [`Iterator`].
/// Not recommended to use directly, only use in iterator chains.
#[derive(Debug)]
pub struct Paginated<I: Iterator>(core::iter::Take<core::iter::Skip<I>>);

impl<I: Iterator> Paginated<I> {
    fn new(pagination: Pagination, iter: I) -> Self {
        // NOTE: `Pagination` stores both offset and limit as `u64`.  When
        // converting them into the `usize` values required by `skip`/`take`, we
        // must ensure that large values do not cause panics on platforms where
        // `usize` is smaller than `u64` (e.g., 32-bit targets).  In such cases we
        // saturate the values to `usize::MAX`, effectively draining the
        // iterator.

        let offset = usize::try_from(pagination.offset_value()).unwrap_or(usize::MAX);
        let limit = pagination.limit_value().map_or(usize::MAX, |limit| {
            usize::try_from(limit.get()).unwrap_or(usize::MAX)
        });

        Self(iter.skip(offset).take(limit))
    }
}

impl<I: Iterator> Iterator for Paginated<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<I: ExactSizeIterator> ExactSizeIterator for Paginated<I> {
    fn len(&self) -> usize {
        self.0.len()
    }
}

impl<I: DoubleEndedIterator + ExactSizeIterator> DoubleEndedIterator for Paginated<I> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back()
    }
}

impl<I: Iterator + core::iter::FusedIterator> core::iter::FusedIterator for Paginated<I> {}

#[cfg(test)]
mod tests {
    use iroha_data_model::query::parameters::Pagination;
    use nonzero_ext::nonzero;

    use super::*;

    #[test]
    fn empty() {
        assert_eq!(
            vec![1_i32, 2_i32, 3_i32]
                .into_iter()
                .paginate(Pagination::new(None, 0))
                .collect::<Vec<_>>(),
            vec![1_i32, 2_i32, 3_i32]
        )
    }

    #[test]
    fn start() {
        assert_eq!(
            vec![1_i32, 2_i32, 3_i32]
                .into_iter()
                .paginate(Pagination::new(None, 1))
                .collect::<Vec<_>>(),
            vec![2_i32, 3_i32]
        );
        assert_eq!(
            vec![1_i32, 2_i32, 3_i32]
                .into_iter()
                .paginate(Pagination::new(None, 3))
                .collect::<Vec<_>>(),
            Vec::<i32>::new()
        );
    }

    #[test]
    fn limit() {
        assert_eq!(
            vec![1_i32, 2_i32, 3_i32]
                .into_iter()
                .paginate(Pagination::new(Some(nonzero!(2_u64)), 0))
                .collect::<Vec<_>>(),
            vec![1_i32, 2_i32]
        );
        assert_eq!(
            vec![1_i32, 2_i32, 3_i32]
                .into_iter()
                .paginate(Pagination::new(Some(nonzero!(4_u64)), 0))
                .collect::<Vec<_>>(),
            vec![1_i32, 2_i32, 3_i32]
        );
    }

    #[test]
    fn start_and_limit() {
        assert_eq!(
            vec![1_i32, 2_i32, 3_i32]
                .into_iter()
                .paginate(Pagination::new(Some(nonzero!(1_u64)), 1))
                .collect::<Vec<_>>(),
            vec![2_i32]
        )
    }

    #[test]
    fn len_respects_pagination() {
        let paginated = vec![1_i32, 2_i32, 3_i32]
            .into_iter()
            .paginate(Pagination::new(Some(nonzero!(2_u64)), 1));
        assert_eq!(paginated.len(), 2);

        let paginated = vec![1_i32, 2_i32, 3_i32]
            .into_iter()
            .paginate(Pagination::new(Some(nonzero!(10_u64)), 1));
        assert_eq!(paginated.len(), 2);
    }

    #[test]
    fn size_hint_matches_len() {
        let paginated = vec![1_i32, 2_i32, 3_i32]
            .into_iter()
            .paginate(Pagination::new(Some(nonzero!(2_u64)), 1));
        assert_eq!(paginated.size_hint(), (2, Some(2)));
    }

    #[test]
    fn supports_double_ended_iteration() {
        let mut paginated = vec![1_i32, 2_i32, 3_i32]
            .into_iter()
            .paginate(Pagination::new(None, 0));
        assert_eq!(paginated.next_back(), Some(3));
        assert_eq!(paginated.next(), Some(1));
    }

    #[test]
    fn huge_offset_saturates_and_yields_empty() {
        assert!(
            vec![1_i32, 2_i32, 3_i32]
                .into_iter()
                .paginate(Pagination::new(None, u64::MAX))
                .next()
                .is_none()
        );
    }

    #[test]
    fn huge_limit_saturates_and_yields_all() {
        let out: Vec<_> = vec![1_i32, 2_i32, 3_i32]
            .into_iter()
            .paginate(Pagination::new(
                Some(nonzero!(18_446_744_073_709_551_615_u64)),
                0,
            ))
            .collect();
        assert_eq!(out, vec![1, 2, 3]);
    }
}
