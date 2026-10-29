//! Observe the real field-column owner's clearing operation before deallocation.

use super::{F, ZeroizingFieldColumnV1};
use std::cell::RefCell;

/// Counts immediately before and after one actual owner erasure.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ObservationV1 {
    /// Number of initialized field cells owned by the column.
    pub(crate) cells: usize,
    /// Number of populated cells immediately before clearing.
    pub(crate) nonzero_before: usize,
    /// Number of populated cells immediately after clearing.
    pub(crate) nonzero_after: usize,
}

thread_local! {
    static OBSERVATIONS: RefCell<Option<Vec<ObservationV1>>> = const { RefCell::new(None) };
}

pub(super) fn before_clear_v1(values: &[F]) -> Option<ObservationV1> {
    OBSERVATIONS
        .with_borrow(Option::is_some)
        .then(|| ObservationV1 {
            cells: values.len(),
            nonzero_before: values.iter().filter(|&&value| value != F::ZERO).count(),
            nonzero_after: 0,
        })
}

pub(super) fn after_clear_v1(values: &[F], observation: Option<ObservationV1>) {
    if let Some(mut observation) = observation {
        observation.nonzero_after = values.iter().filter(|&&value| value != F::ZERO).count();
        OBSERVATIONS.with_borrow_mut(|slot| {
            if let Some(observations) = slot {
                observations.push(observation);
            }
        });
    }
}

/// Collect actual field-column owner clears on this thread during an operation.
pub(crate) fn observe_v1<T>(operation: impl FnOnce() -> T) -> (T, Vec<ObservationV1>) {
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            OBSERVATIONS.set(None);
        }
    }
    OBSERVATIONS.with_borrow_mut(|slot| {
        assert!(slot.is_none(), "nested field-column erasure census");
        *slot = Some(Vec::new());
    });
    let _scope = Scope;
    let result = operation();
    (result, OBSERVATIONS.take().unwrap())
}

#[test]
fn actual_field_column_owner_clears_on_return_and_unwind() {
    for unwind in [false, true] {
        let (result, observed) = observe_v1(|| {
            std::panic::catch_unwind(|| {
                let column = ZeroizingFieldColumnV1::from_vec_v1(vec![F(17), F::ZERO, F(31)]);
                assert!(!unwind, "injected field-column owner unwind");
                drop(column);
            })
        });
        assert_eq!(result.is_err(), unwind);
        assert_eq!(
            observed,
            vec![ObservationV1 {
                cells: 3,
                nonzero_before: 2,
                nonzero_after: 0,
            }]
        );
    }
}

#[test]
fn canonical_words_transfer_the_exact_allocation_and_clear_on_return_and_unwind() {
    assert_eq!(core::mem::size_of::<F>(), core::mem::size_of::<u64>());
    assert_eq!(core::mem::align_of::<F>(), core::mem::align_of::<u64>());
    for unwind in [false, true] {
        let (result, observed) = observe_v1(|| {
            std::panic::catch_unwind(|| {
                let mut words = Vec::with_capacity(17);
                words.extend([
                    17,
                    0,
                    super::super::transparent_stark::GOLDILOCKS_MODULUS_V1 - 1,
                ]);
                let pointer = words.as_ptr().cast::<F>();
                let capacity = words.capacity();
                let column = ZeroizingFieldColumnV1::from_canonical_words_v1(words).unwrap();
                assert_eq!(column.as_ptr(), pointer);
                assert_eq!(column.0.capacity(), capacity);
                assert_eq!(
                    &*column,
                    &[
                        F(17),
                        F::ZERO,
                        F(super::super::transparent_stark::GOLDILOCKS_MODULUS_V1 - 1)
                    ]
                );
                assert!(!unwind, "injected transferred-column unwind");
                drop(column);
            })
        });
        assert_eq!(result.is_err(), unwind);
        assert_eq!(
            observed,
            vec![ObservationV1 {
                cells: 3,
                nonzero_before: 2,
                nonzero_after: 0
            }]
        );
    }
}

#[test]
fn canonical_word_transfer_rejects_every_noncanonical_position_and_accepts_empty_storage() {
    for invalid in [
        super::super::transparent_stark::GOLDILOCKS_MODULUS_V1,
        u64::MAX,
    ] {
        for position in 0..3 {
            let mut words = vec![17; 3];
            words[position] = invalid;
            let (result, observed) =
                observe_v1(|| ZeroizingFieldColumnV1::from_canonical_words_v1(words));
            assert!(matches!(
                result,
                Err(super::AggregateStarkErrorV1::NonCanonicalField)
            ));
            assert_eq!(
                observed,
                vec![ObservationV1 {
                    cells: 3,
                    nonzero_before: 3,
                    nonzero_after: 0
                }]
            );
        }
    }
    for words in [Vec::new(), Vec::with_capacity(17)] {
        let capacity = words.capacity();
        let pointer = words.as_ptr().cast::<F>();
        let column = ZeroizingFieldColumnV1::from_canonical_words_v1(words).unwrap();
        assert!(column.is_empty());
        assert_eq!(column.as_ptr(), pointer);
        assert_eq!(column.0.capacity(), capacity);
    }
}
