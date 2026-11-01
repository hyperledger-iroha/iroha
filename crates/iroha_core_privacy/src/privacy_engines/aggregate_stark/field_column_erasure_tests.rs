//! Observe the real field-column owner's clearing operation before deallocation.

use super::{F, ZeroizingFieldColumnV1};
use std::cell::RefCell;

/// Counts immediately before and after one actual owner erasure.
#[derive(Debug, PartialEq, Eq)]
pub struct ObservationV1 {
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
pub fn observe_v1<T>(operation: impl FnOnce() -> T) -> (T, Vec<ObservationV1>) {
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
    for words in [Vec::<u64>::new(), Vec::with_capacity(17)] {
        let capacity = words.capacity();
        let pointer = words.as_ptr().cast::<F>();
        let column = ZeroizingFieldColumnV1::from_canonical_words_v1(words).unwrap();
        assert!(column.is_empty());
        assert_eq!(column.as_ptr(), pointer);
        assert_eq!(column.0.capacity(), capacity);
    }
}

fn drop_fixture_v1(rows: usize, width: usize) -> Vec<ZeroizingFieldColumnV1> {
    (0..width)
        .map(|column| {
            let mut values = Vec::with_capacity(rows);
            assert_eq!(values.capacity(), rows);
            values.resize(rows, F(17 + column as u64));
            ZeroizingFieldColumnV1::from_vec_v1(values)
        })
        .collect()
}

/// An explicit successful-path cleanup, never a parallel implementation of Drop.
/// The pool is constructed and warmed before any private fixture is allocated.
/// Each individual owner still runs its unchanged volatile eraser and deallocator.
fn drop_batch_v1(columns: Vec<ZeroizingFieldColumnV1>, pool: &rayon::ThreadPool, parallel: bool) {
    if parallel {
        pool.install(|| super::drop_completed_field_columns_v1(columns));
    } else {
        drop(columns);
    }
}

#[test]
fn bounded_parallel_owner_drop_preserves_all_initialized_cells_and_unwind_cleanup() {
    for workers in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        for (rows, width) in [(0, 1), (17, 8), (257, 1), (1 << 16, 8)] {
            for parallel in [false, true] {
                for unwind in [false, true] {
                    // Reserve public observation slots before owner destruction;
                    // the eraser itself neither initializes a pool nor allocates.
                    pool.broadcast(|_| {
                        OBSERVATIONS.with_borrow_mut(|slot| {
                            assert!(slot.is_none());
                            *slot = Some(Vec::with_capacity(width));
                        });
                    });
                    let columns = drop_fixture_v1(rows, width);
                    assert!(
                        columns
                            .iter()
                            .all(|column| { column.len() == rows && column.0.capacity() == rows })
                    );
                    let (result, mut observed) = observe_v1(|| {
                        OBSERVATIONS.with_borrow_mut(|slot| {
                            slot.as_mut().unwrap().reserve_exact(width);
                        });
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            if unwind {
                                // Error/unwind uses ordinary owner cleanup. No
                                // new parallel work is scheduled from Drop.
                                let unwind_owners = move || {
                                    let _columns = columns;
                                    std::panic::resume_unwind(Box::new(()));
                                };
                                if parallel {
                                    pool.install(unwind_owners);
                                } else {
                                    unwind_owners();
                                }
                            } else {
                                drop_batch_v1(columns, &pool, parallel);
                            }
                        }))
                    });
                    for worker in pool.broadcast(|_| OBSERVATIONS.take().unwrap()) {
                        observed.extend(worker);
                    }
                    assert_eq!(result.is_err(), unwind);
                    assert_eq!(observed.len(), width);
                    assert!(observed.iter().all(|observation| {
                        observation.cells == rows
                            && observation.nonzero_before == rows
                            && observation.nonzero_after == 0
                    }));
                }
            }
        }
    }
}

#[test]
#[ignore = "isolated complete field-owner Drop and deallocation A/B"]
fn full_field_owner_drop_cost_reports_serial_and_bounded_parallel_cleanup() {
    use std::time::Instant;

    const ROUNDS: usize = 6;
    const MAIN_INITIAL_LDE_BYTES: u64 = 5_831 * (1 << 22) * 8;
    assert!(rayon::current_thread_index().is_none());
    assert!(OBSERVATIONS.with_borrow(Option::is_none));
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        // Complete worker startup before constructing any clearing owner. All
        // timing starts on the external caller, so parallel includes pool entry.
        pool.broadcast(|context| {
            assert!(OBSERVATIONS.with_borrow(Option::is_none));
            assert_eq!(rayon::current_num_threads(), workers);
            assert_eq!(rayon::current_thread_index(), Some(context.index()));
        });
        for rows in [1_usize << 19, 1 << 22] {
            for width in [1, 8] {
                let mut samples = [[0.0_f64; ROUNDS]; 2];
                for round in 0..ROUNDS {
                    for position in 0..2 {
                        let mode = (round + position) % 2;
                        let columns = drop_fixture_v1(rows, width);
                        let columns = std::hint::black_box(columns);
                        let started = Instant::now();
                        drop_batch_v1(columns, &pool, mode == 1);
                        samples[mode][round] = started.elapsed().as_secs_f64();
                    }
                }
                let means = samples.map(|values| values.into_iter().sum::<f64>() / ROUNDS as f64);
                let medians = samples.map(|mut values| {
                    values.sort_by(f64::total_cmp);
                    (values[ROUNDS / 2 - 1] + values[ROUNDS / 2]) / 2.0
                });
                let bytes = rows * width * core::mem::size_of::<F>();
                let initial_scale = MAIN_INITIAL_LDE_BYTES as f64 / bytes as f64;
                eprintln!(
                    "X509 field-owner Drop A/B: caller_context=external workers={workers} rows={rows} width={width} bytes={bytes} rounds={ROUNDS} serial_mean_seconds={:.9} parallel_mean_seconds={:.9} serial_median_seconds={:.9} parallel_median_seconds={:.9} serial_linear_main_initial_seconds={:.6} parallel_linear_main_initial_seconds={:.6}; includes original volatile erasure, deallocation and parallel pool entry; excludes fixture allocation/fill and startup; warmed-pool isolated extrapolation only, not full-proof timing",
                    means[0],
                    means[1],
                    medians[0],
                    medians[1],
                    means[0] * initial_scale,
                    means[1] * initial_scale,
                );
            }
        }
    }
}

#[test]
fn tiled_canonical_validation_checks_boundaries_and_hidden_invalid_words() {
    use super::super::transparent_stark::GOLDILOCKS_MODULUS_V1 as MODULUS;
    for workers in [1, 4, 20] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
            .install(|| {
                for rows in [0, 1, 17, (1 << 16) - 1, 1 << 16, (1 << 16) + 7, 1 << 22] {
                    let mut values = vec![F(MODULUS - 1); rows];
                    assert!(super::canonical_field_column_v1(&values));
                    for index in [0, 1, (1 << 14) - 1, 1 << 14, rows.saturating_sub(1)] {
                        if index >= rows {
                            continue;
                        }
                        for invalid in [MODULUS, u64::MAX] {
                            values[index] = F(invalid);
                            assert!(!super::canonical_field_column_v1(&values));
                            values[index] = F(MODULUS - 1);
                        }
                    }
                }
            });
    }
}

#[test]
#[ignore = "full-size canonical scan A/B with exact refusal semantics"]
fn tiled_canonical_validation_reports_full_domain_cost() {
    use super::super::transparent_stark::GOLDILOCKS_MODULUS_V1 as MODULUS;
    use std::time::Instant;
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.broadcast(|_| ());
        for rows in [1 << 19, 1 << 22] {
            let values = ZeroizingFieldColumnV1::from_vec_v1(vec![F(MODULUS - 1); rows]);
            for round in 0..6 {
                for parallel in if round % 2 == 0 {
                    [false, true]
                } else {
                    [true, false]
                } {
                    let started = Instant::now();
                    for _ in 0..16 {
                        let valid = if parallel {
                            pool.install(|| {
                                super::canonical_field_column_v1(std::hint::black_box(&values))
                            })
                        } else {
                            std::hint::black_box(&*values)
                                .iter()
                                .all(|value| F::canonical(value.0).is_some())
                        };
                        assert!(std::hint::black_box(valid));
                    }
                    eprintln!(
                        "field_canonical_scan workers={workers} rows={rows} round={round} parallel={parallel} calls=16 seconds={:.9}",
                        started.elapsed().as_secs_f64()
                    );
                }
            }
        }
    }
}
