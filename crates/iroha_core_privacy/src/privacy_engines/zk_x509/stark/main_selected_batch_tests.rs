//! Independent arithmetic, layout, erasure and timing checks for batched DIF.

use super::*;
use crate::privacy_engines::transparent_stark::{
    GOLDILOCKS_MODULUS_V1, masked_trace_coefficients_with_mask_v1,
};
use crate::privacy_engines::zk_x509::private_table::inspection;

thread_local! {
    static BATCH_ERASURES_V1: std::cell::RefCell<Option<Vec<inspection::ErasureObservationV1>>> = const { std::cell::RefCell::new(None) };
}

pub(super) fn observed_clear_count_v1() -> Option<usize> {
    BATCH_ERASURES_V1.with_borrow(|observations| observations.as_ref().map(Vec::len))
}

pub(super) fn before_clear_v1(fields: &[F]) -> Option<inspection::ErasureObservationV1> {
    BATCH_ERASURES_V1
        .with_borrow(Option::is_some)
        .then(|| inspection::ErasureObservationV1 {
            cells: fields.len(),
            nonzero_before: fields.iter().filter(|&&value| value != F::ZERO).count(),
            nonzero_after: 0,
        })
}

pub(super) fn after_clear_v1(fields: &[F], observation: Option<inspection::ErasureObservationV1>) {
    if let Some(mut observation) = observation {
        observation.nonzero_after = fields.iter().filter(|&&value| value != F::ZERO).count();
        BATCH_ERASURES_V1.with_borrow_mut(|observations| {
            if let Some(observations) = observations {
                observations.push(observation);
            }
        });
    }
}

pub(super) fn observe_batch_clear_v1<T>(
    operation: impl FnOnce() -> T,
) -> (T, Vec<inspection::ErasureObservationV1>) {
    struct Scope;
    impl Drop for Scope {
        fn drop(&mut self) {
            BATCH_ERASURES_V1.set(None);
        }
    }
    BATCH_ERASURES_V1.with_borrow_mut(|slot| {
        assert!(slot.is_none(), "nested batch erasure census");
        *slot = Some(Vec::new());
    });
    let _scope = Scope;
    let result = operation();
    (result, BATCH_ERASURES_V1.take().unwrap())
}

fn coefficients_v1(prefix: usize) -> Vec<Column> {
    (0..WIDTH_V1)
        .map(|column| {
            let length = prefix - column % prefix;
            Column::from_vec_v1(
                (0..length)
                    .map(|degree| {
                        if (degree + column) % 5 == 0 {
                            F(GOLDILOCKS_MODULUS_V1 - 1)
                        } else {
                            F((degree * 137 + column * 53 + 19) as u64)
                        }
                    })
                    .collect(),
            )
        })
        .collect()
}

fn horner_v1(coefficients: &[F], point: F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |sum, &coefficient| sum.mul(point).add(coefficient))
}

#[test]
fn batched_selected_matches_scalar_fft_and_horner_for_every_small_prefix_boundary() {
    for common in 2..=10 {
        let rows = 1usize << common;
        let root = goldilocks_primitive_root_v1(common).unwrap();
        for prefix in [1, rows / 2 - 1, rows / 2, rows / 2 + 1, rows - 1, rows] {
            let coefficients = coefficients_v1(prefix);
            for selected in [
                vec![0],
                vec![rows - 1],
                vec![0, rows / 2, rows - 1],
                (0..rows).step_by(rows.div_ceil(MAX_SELECTED_V1)).collect(),
            ] {
                let actual = evaluate_batch_v1(&coefficients, 1, common, &selected).unwrap();
                for (input, actual) in coefficients.iter().zip(&actual) {
                    let scalar = super::super::evaluate_v1(input, 1, common, &selected).unwrap();
                    assert_eq!(actual.as_ref(), scalar.as_ref());
                    for (&row, &value) in selected.iter().zip(actual.iter()) {
                        let point = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(row as u128));
                        assert_eq!(value, horner_v1(input, point));
                    }
                }
            }
        }
    }
}

#[test]
fn batched_parallel_windows_and_subtrees_preserve_each_column_and_high_tail() {
    let common = 18;
    let rows = 1usize << common;
    let root = goldilocks_primitive_root_v1(common).unwrap();
    let coefficients = coefficients_v1(PARALLEL_PAIRS_V1 + 3);
    let selected = [0, 1, 63, 8191, rows / 2, rows - 1];
    let expected = coefficients
        .iter()
        .map(|column| {
            selected
                .iter()
                .map(|&row| {
                    horner_v1(
                        column,
                        F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(row as u128)),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for workers in [1, 4, 20] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        let output = pool
            .install(|| evaluate_batch_v1(&coefficients, 8, common, &selected))
            .unwrap();
        for (actual, expected) in output.iter().zip(&expected) {
            assert_eq!(actual.as_ref(), expected.as_slice());
        }
    }
}

#[test]
fn batched_validation_precedes_private_allocation_for_all_columns_and_coordinates() {
    for failure in 0..11 {
        let mut coefficients = coefficients_v1(17);
        let mut selected = vec![0, 7, 63];
        let mut native = 4;
        let mut common = 6;
        match failure {
            0 => {
                coefficients.pop();
            }
            1 => {
                coefficients.push(Column::from_vec_v1(vec![F::ONE]));
            }
            2 => {
                coefficients[7] = Column::from_vec_v1(vec![]);
            }
            3 => {
                coefficients[7] = Column::from_vec_v1(vec![F::ONE; 65]);
            }
            4 => {
                coefficients[7] = Column::from_vec_v1(vec![F(GOLDILOCKS_MODULUS_V1)]);
            }
            5 => {
                selected.clear();
            }
            6 => {
                selected = vec![63, 7];
            }
            7 => {
                selected = vec![7, 7];
            }
            8 => {
                selected = vec![64];
            }
            9 => {
                native = 6;
            }
            _ => {
                common = u8::MAX;
            }
        }
        let (result, erased) =
            inspection::observe_v1(|| evaluate_batch_v1(&coefficients, native, common, &selected));
        assert!(matches!(result, Err(error) if error == if failure == 4 {
            AggregateStarkErrorV1::NonCanonicalField
        } else {
            AggregateStarkErrorV1::InvalidLayout
        }));
        assert!(erased.is_empty());
    }
    let coefficients = coefficients_v1(17);
    for (common, selected) in [(10, (0..273).collect::<Vec<_>>()), (23, vec![0])] {
        let (result, erased) =
            inspection::observe_v1(|| evaluate_batch_v1(&coefficients, 4, common, &selected));
        assert!(matches!(result, Err(AggregateStarkErrorV1::InvalidLayout)));
        assert!(erased.is_empty());
    }
}

#[test]
fn batched_outputs_and_interleaved_scratch_clear_on_errors_and_unwinds() {
    let coefficients = coefficients_v1(21);
    let selected = [0, 7, 63];
    for after_prune in [false, true] {
        for unwind in [false, true] {
            let ((result, scratch_erased), erased) = inspection::observe_v1(|| {
                observe_batch_clear_v1(|| {
                    std::panic::catch_unwind(|| {
                        evaluate_batch_with_v1(
                            &coefficients,
                            4,
                            6,
                            &selected,
                            |completed, scratch| {
                                assert_eq!(scratch.len(), WIDTH_V1 * 64);
                                assert!(scratch.iter().any(|&value| value != F::ZERO));
                                if completed == after_prune {
                                    assert!(!unwind, "injected populated batched scratch unwind");
                                    return Err(AggregateStarkErrorV1::InternalInvariant);
                                }
                                Ok(())
                            },
                        )
                    })
                })
            });
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(AggregateStarkErrorV1::InternalInvariant)
                ));
            }
            assert_eq!(
                erased.iter().map(|entry| entry.cells).sum::<usize>(),
                WIDTH_V1 * selected.len()
            );
            assert_eq!(scratch_erased.len(), 1);
            assert_eq!(scratch_erased[0].cells, WIDTH_V1 * 64);
            assert!(scratch_erased[0].nonzero_before > 0);
            assert_eq!(scratch_erased[0].nonzero_after, 0);
            assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
        }
    }
    let ((output, scratch_erased), erased) = inspection::observe_v1(|| {
        observe_batch_clear_v1(|| evaluate_batch_v1(&coefficients, 4, 6, &selected).unwrap())
    });
    assert_eq!(output.len(), WIDTH_V1);
    assert!(output.iter().all(|column| column.len() == selected.len()));
    assert_eq!(erased.iter().map(|entry| entry.cells).sum::<usize>(), 0);
    assert_eq!(scratch_erased.len(), 1);
    assert_eq!(scratch_erased[0].cells, WIDTH_V1 * 64);
    assert_eq!(scratch_erased[0].nonzero_after, 0);
    assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
}

#[test]
fn parallel_batch_eraser_clears_all_chunks_including_partial_last_chunk() {
    for workers in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.install(|| {
            let mut fields = vec![F(GOLDILOCKS_MODULUS_V1 - 1); CLEAR_CHUNK_FIELDS_V1 + 3];
            let (_, erased) = observe_batch_clear_v1(|| erase_batch_v1(&mut fields));
            assert!(fields.iter().all(|&value| value == F::ZERO));
            assert_eq!(erased.len(), 1);
            assert_eq!(erased[0].cells, fields.len());
            assert_eq!(erased[0].nonzero_before, fields.len());
            assert_eq!(erased[0].nonzero_after, 0);
        });
    }
}

#[test]
fn low_prefix_single_destination_clears_during_unwind_without_explicit_pool_setup() {
    let coefficients = coefficients_v1(3);
    let rows = 1usize << 16;
    let (result, erased) = observe_batch_clear_v1(|| {
        std::panic::catch_unwind(|| {
            // This prefix and selected path never need parallel arithmetic.
            // The evaluator must prepare its pool before owning private scratch.
            evaluate_batch_with_v1(&coefficients, 5, 16, &[0], |_, _| {
                panic!("short-prefix failure before selected traversal")
            })
        })
    });
    assert!(result.is_err());
    assert_eq!(erased.len(), 1);
    assert_eq!(erased[0].cells, WIDTH_V1 * rows);
    assert!(erased[0].nonzero_before > 0);
    assert_eq!(erased[0].nonzero_after, 0);
}

#[test]
fn parallel_batch_drop_clears_populated_scratch_during_unwind() {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap();
    let coefficients = coefficients_v1(3);
    let rows = 1usize << 16;
    for after_prune in [false, true] {
        pool.install(|| {
            let (result, erased) = observe_batch_clear_v1(|| {
                std::panic::catch_unwind(|| {
                    evaluate_batch_with_v1(&coefficients, 5, 16, &[0, rows - 1], |completed, _| {
                        assert!(
                            completed != after_prune,
                            "injected parallel batch drop unwind"
                        );
                        Ok(())
                    })
                })
            });
            assert!(result.is_err());
            assert_eq!(erased.len(), 1);
            assert_eq!(erased[0].cells, WIDTH_V1 * rows);
            assert!(erased[0].nonzero_before > 0);
            assert_eq!(erased[0].nonzero_after, 0);
        });
    }
}

#[test]
#[ignore = "isolated production interleaved selected DIF versus scalar reference cost"]
fn selected_interleaved_batch_cost_keeps_every_mask_and_query_coordinate() {
    use std::time::Instant;

    let common = 22;
    let rows = 1usize << common;
    let cut_rows = 1 << aggregate::retained_commitment::CUT_LEVEL_V1;
    let selected = (0..QUERY_COUNT)
        .flat_map(|query| {
            let block = (query * 1729 + 17) % (rows / cut_rows);
            block * cut_rows..(block + 1) * cut_rows
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), MAX_SELECTED_V1);
    for native in [15, 16, 18, 19] {
        let coefficients = (0..WIDTH_V1)
            .map(|column| {
                let values = (0..1usize << native)
                    .map(|row| F((17 * row + 13 * column + 1) as u64))
                    .collect::<Vec<_>>();
                let mask = (0..MASK_DEGREE + 1)
                    .map(|degree| F((29 * degree + 7 * column + 3) as u64))
                    .collect::<Vec<_>>();
                Column::from_vec_v1(
                    masked_trace_coefficients_with_mask_v1(&values, native, &mask).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let expected = coefficients
            .iter()
            .map(|column| {
                let full = Column::from_vec_v1(
                    masked_trace_coefficients_on_coset_v1(column, native, common).unwrap(),
                );
                Column::from_vec_v1(selected.iter().map(|&row| full[row]).collect())
            })
            .collect::<Vec<_>>();
        for round in 0..3 {
            for batched in if round % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let started = Instant::now();
                let output = if batched {
                    evaluate_batch_v1(&coefficients, native, common, &selected).unwrap()
                } else {
                    coefficients
                        .par_iter()
                        .map(|column| super::super::evaluate_v1(column, native, common, &selected))
                        .collect::<Result<Vec<_>, _>>()
                        .unwrap()
                };
                let transform_ns = started.elapsed().as_nanos();
                for (actual, expected) in output.iter().zip(&expected) {
                    assert_eq!(actual.as_ref(), expected.as_ref());
                }
                let clear_started = Instant::now();
                drop(output);
                let clear_ns = clear_started.elapsed().as_nanos();
                println!(
                    "x509_selected_batch_cost native_log={native} common_log={common} width=8 workers={} round={round} batched={batched} selected_rows={} coefficient_count={} private_scratch_bytes={} transform_gather_scratch_clear_ns={transform_ns} output_clear_ns={clear_ns} parity=exact",
                    rayon::current_num_threads(),
                    selected.len(),
                    coefficients[0].len(),
                    rows * WIDTH_V1 * size_of::<F>(),
                );
            }
        }
    }
}

#[test]
fn residue_classes_match_dense_scalar_and_horner_at_every_prefix_and_wrap_boundary() {
    for common in 6..=10 {
        let rows = 1usize << common;
        let subtree = rows / 4;
        let native = common - 3;
        let root = goldilocks_primitive_root_v1(common).unwrap();
        for prefix in [
            1,
            subtree / 2 - 1,
            subtree / 2,
            subtree / 2 + 1,
            subtree - 1,
            subtree,
        ] {
            let coefficients = coefficients_v1(prefix);
            for selected in [
                vec![0],
                vec![rows - 1],
                vec![0, 1, 2, 3, rows / 2, rows - 1],
                (0..rows).step_by(4).take(MAX_SELECTED_V1).collect(),
                (0..rows).step_by(rows.div_ceil(MAX_SELECTED_V1)).collect(),
            ] {
                let actual = evaluate_residue_batch_with_v1(
                    &coefficients,
                    native,
                    common,
                    &selected,
                    |_, _, _| Ok(()),
                )
                .unwrap();
                let original = evaluate_batch_v1(&coefficients, native, common, &selected).unwrap();
                for ((input, actual), original) in coefficients.iter().zip(&actual).zip(&original) {
                    assert_eq!(actual.as_ref(), original.as_ref());
                    let scalar =
                        super::super::evaluate_v1(input, native, common, &selected).unwrap();
                    assert_eq!(actual.as_ref(), scalar.as_ref());
                    for (&row, &value) in selected.iter().zip(actual.iter()) {
                        let point = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(row as u128));
                        assert_eq!(value, horner_v1(input, point));
                    }
                }
            }
        }
    }
}

#[test]
fn residue_classes_preserve_parallel_windows_zero_values_and_complete_high_tails() {
    let common = 19;
    let rows = 1usize << common;
    let native = 16;
    let mut coefficients = coefficients_v1((1 << native) + MASK_DEGREE + 1);
    coefficients[7] = Column::from_vec_v1(vec![F::ZERO; coefficients[7].len()]);
    let selected = [0, 1, 2, 3, rows / 2 - 1, rows / 2, rows - 2, rows - 1];
    let expected = evaluate_batch_v1(&coefficients, native, common, &selected).unwrap();
    for workers in [1, 4, 20] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
            .install(|| {
                let actual = evaluate_residue_batch_with_v1(
                    &coefficients,
                    native,
                    common,
                    &selected,
                    |_, _, scratch| {
                        assert_eq!(scratch.len(), WIDTH_V1 * (rows / 4));
                        Ok(())
                    },
                )
                .unwrap();
                for (actual, expected) in actual.iter().zip(&expected) {
                    assert_eq!(actual.as_ref(), expected.as_ref());
                }
                assert!(actual[7].iter().all(|&value| value == F::ZERO));
            });
    }
}

#[test]
fn residue_classes_validate_before_private_writes_and_fit_the_original_scratch_budget() {
    for rows in [64usize, 1 << 16, 1 << 20, 1 << 22] {
        let cells = residue_scratch_cells_v1(rows).unwrap();
        assert_eq!(cells, rows / 4 * WIDTH_V1);
        let actual = cells * size_of::<F>() + size_of::<[F; MAX_SELECTED_V1]>();
        assert!(actual <= rows * WIDTH_V1 * size_of::<F>());
    }
    assert_eq!(residue_scratch_cells_v1(1 << 22).unwrap() * 8, 67_108_864);
    for rows in [0, 1, 3, 4, 32, usize::MAX, 1 << (usize::BITS - 1)] {
        assert!(residue_scratch_cells_v1(rows).is_err());
    }
    for failure in 0..13 {
        let mut coefficients = coefficients_v1(17);
        let mut selected = vec![0, 7, 255];
        let mut native = 4;
        let mut common = 8;
        match failure {
            0 => {
                coefficients.pop();
            }
            1 => coefficients.push(Column::from_vec_v1(vec![F::ONE])),
            2 => coefficients[7] = Column::from_vec_v1(vec![]),
            3 => coefficients[7] = Column::from_vec_v1(vec![F::ONE; 65]),
            4 => {
                coefficients[7] = Column::from_vec_v1(vec![F(GOLDILOCKS_MODULUS_V1)]);
            }
            5 => selected.clear(),
            6 => selected = vec![255, 7],
            7 => selected = vec![7, 7],
            8 => selected = vec![256],
            9 => native = 8,
            10 => common = u8::MAX,
            11 => {
                common = 10;
                selected = (0..273).collect();
            }
            _ => common = 23,
        }
        let ((result, scratch_erased), erased) = inspection::observe_v1(|| {
            observe_batch_clear_v1(|| {
                evaluate_residue_batch_with_v1(
                    &coefficients,
                    native,
                    common,
                    &selected,
                    |_, _, _| panic!("invalid geometry or field reached private scratch"),
                )
            })
        });
        assert!(matches!(result, Err(error) if error == if failure == 4 {
            AggregateStarkErrorV1::NonCanonicalField
        } else {
            AggregateStarkErrorV1::InvalidLayout
        }));
        assert!(scratch_erased.is_empty());
        assert!(erased.is_empty());
    }
}

#[test]
fn residue_classes_erase_one_reused_scratch_and_partial_outputs_on_error_and_unwind() {
    let coefficients = coefficients_v1(13);
    let selected = [0, 1, 2, 3, 31, 32, 62, 63];
    for failed_residue in 0..4 {
        for after_prune in [false, true] {
            for unwind in [false, true] {
                let ((result, scratch_erased), erased) = inspection::observe_v1(|| {
                    observe_batch_clear_v1(|| {
                        std::panic::catch_unwind(|| {
                            evaluate_residue_batch_with_v1(
                                &coefficients,
                                3,
                                6,
                                &selected,
                                |residue, completed, scratch| {
                                    assert_eq!(scratch.len(), WIDTH_V1 * 16);
                                    assert!(scratch.iter().any(|&value| value != F::ZERO));
                                    if residue == failed_residue && completed == after_prune {
                                        assert!(!unwind, "injected residue-class unwind");
                                        return Err(AggregateStarkErrorV1::InternalInvariant);
                                    }
                                    Ok(())
                                },
                            )
                        })
                    })
                });
                if unwind {
                    assert!(result.is_err());
                } else {
                    assert!(matches!(
                        result.unwrap(),
                        Err(AggregateStarkErrorV1::InternalInvariant)
                    ));
                }
                assert_eq!(scratch_erased.len(), 1);
                assert_eq!(scratch_erased[0].cells, WIDTH_V1 * 16);
                assert!(scratch_erased[0].nonzero_before > 0);
                assert_eq!(scratch_erased[0].nonzero_after, 0);
                assert_eq!(
                    erased.iter().map(|entry| entry.cells).sum::<usize>(),
                    WIDTH_V1 * selected.len()
                );
                assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
                if failed_residue != 0 {
                    assert!(erased.iter().any(|entry| entry.nonzero_before != 0));
                }
            }
        }
    }
    let (result, erased) = observe_batch_clear_v1(|| {
        evaluate_residue_batch_with_v1(&coefficients, 3, 6, &selected, |_, _, _| Ok(())).unwrap()
    });
    assert_eq!(result.len(), WIDTH_V1);
    assert_eq!(erased.len(), 1);
    assert_eq!(erased[0].cells, WIDTH_V1 * 16);
    assert_eq!(erased[0].nonzero_after, 0);
}

#[test]
#[ignore = "isolated four-coset query A/B; complete CPU oracle, allocation and erasure timing"]
fn selected_residue_class_cost_keeps_full_native_masks_and_every_query_coordinate() {
    use std::time::{Duration, Instant};

    assert!(!cfg!(debug_assertions), "use an optimized build");
    let common = 22;
    let rows = 1usize << common;
    let cut_rows = 1 << aggregate::retained_commitment::CUT_LEVEL_V1;
    let selected = (0..QUERY_COUNT)
        .flat_map(|query| {
            let block = match query {
                0 => 0,
                1 => rows / cut_rows - 1,
                _ => ((query - 2) * 1729 + 17) % (rows / cut_rows),
            };
            block * cut_rows..(block + 1) * cut_rows
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), MAX_SELECTED_V1);
    let selections = [
        selected,
        vec![0, 1, 2, 3, rows / 2 - 1, rows / 2, rows - 2, rows - 1],
    ];
    for native in [15, 16, 18, 19] {
        let coefficients = (0..WIDTH_V1)
            .map(|column| {
                let values = Column::from_vec_v1(
                    (0..1usize << native)
                        .map(|row| F((17 * row + 13 * column + 1) as u64))
                        .collect(),
                );
                let mask = Column::from_vec_v1(
                    (0..MASK_DEGREE + 1)
                        .map(|degree| F((29 * degree + 7 * column + 3) as u64))
                        .collect(),
                );
                Column::from_vec_v1(
                    masked_trace_coefficients_with_mask_v1(&values, native, &mask).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let mut expected: [Vec<Column>; 2] = core::array::from_fn(|_| Vec::new());
        for column in &coefficients {
            // One full independent CPU FFT at a time, outside all timings.
            let full = Column::from_vec_v1(
                masked_trace_coefficients_on_coset_v1(column, native, common).unwrap(),
            );
            for (output, selected) in expected.iter_mut().zip(&selections) {
                output.push(Column::from_vec_v1(
                    selected.iter().map(|&row| full[row]).collect(),
                ));
            }
        }
        for (selected, expected) in selections.iter().zip(&expected) {
            const ROUNDS: usize = 5;
            let mut elapsed = [[Duration::ZERO; ROUNDS]; 2];
            for round in 0..ROUNDS {
                for residue_classes in if round % 2 == 0 {
                    [false, true]
                } else {
                    [true, false]
                } {
                    let started = Instant::now();
                    let output = if residue_classes {
                        evaluate_residue_batch_with_v1(
                            &coefficients,
                            native,
                            common,
                            selected,
                            |_, _, _| Ok(()),
                        )
                    } else {
                        evaluate_batch_v1(&coefficients, native, common, selected)
                    }
                    .unwrap();
                    let transform = started.elapsed();
                    for (actual, expected) in output.iter().zip(expected) {
                        assert_eq!(actual.as_ref(), expected.as_ref());
                    }
                    let clear_started = Instant::now();
                    drop(output);
                    let clear = clear_started.elapsed();
                    elapsed[usize::from(residue_classes)][round] = transform + clear;
                    println!(
                        "x509_selected_residue_class_cost native_log={native} common_log={common} width=8 workers={} round={round} residue_classes={residue_classes} selected_rows={} coefficient_count={} private_scratch_bytes={} public_index_bytes={} transform_allocation_gather_scratch_clear_ns={} output_clear_ns={} parity=all_fields_exact",
                        rayon::current_num_threads(),
                        selected.len(),
                        coefficients[0].len(),
                        (if residue_classes { rows / 4 } else { rows }) * WIDTH_V1 * size_of::<F>(),
                        if residue_classes {
                            size_of::<[F; MAX_SELECTED_V1]>()
                        } else {
                            0
                        },
                        transform.as_nanos(),
                        clear.as_nanos(),
                    );
                }
            }
            for values in &mut elapsed {
                values.sort_unstable();
            }
            println!(
                "x509_selected_residue_class_median native_log={native} common_log={common} width=8 workers={} selected_rows={} original_ns={} four_class_ns={} speedup={:.6} parity=all_fields_exact",
                rayon::current_num_threads(),
                selected.len(),
                elapsed[0][ROUNDS / 2].as_nanos(),
                elapsed[1][ROUNDS / 2].as_nanos(),
                elapsed[0][ROUNDS / 2].as_secs_f64() / elapsed[1][ROUNDS / 2].as_secs_f64(),
            );
        }
    }
}
