//! Production query scratch reuse: independent arithmetic, ownership and complete-cost checks.

use super::*;
use crate::privacy_engines::transparent_stark::{
    GOLDILOCKS_MODULUS_V1, masked_trace_coefficients_with_mask_v1,
};
use crate::privacy_engines::zk_x509::private_table::inspection;

#[test]
fn reused_selected_scratch_specialized_rows_preserve_every_public_child_choice() {
    for present in [false, true] {
        for (even_needed, odd_needed) in [(true, false), (false, true), (true, true)] {
            let mut even = (0..3 * WIDTH_V1)
                .map(|i| F((i * 17 + 11) as u64))
                .collect::<Vec<_>>();
            let mut odd = vec![F(GOLDILOCKS_MODULUS_V1 - 1); even.len()];
            let original_even = even.clone();
            let original_odd = odd.clone();
            let root = F(71);
            let first_twiddle = F(97);
            let actual_twiddle = if present {
                apply_rows_v1::<true>(
                    &mut even,
                    &mut odd,
                    root,
                    first_twiddle,
                    even_needed,
                    odd_needed,
                )
            } else {
                apply_rows_v1::<false>(
                    &mut even,
                    &mut odd,
                    root,
                    first_twiddle,
                    even_needed,
                    odd_needed,
                )
            };
            let mut twiddle = first_twiddle;
            for row in 0..3 {
                for column in 0..WIDTH_V1 {
                    let index = row * WIDTH_V1 + column;
                    let right = if present {
                        original_odd[index]
                    } else {
                        F::ZERO
                    };
                    assert_eq!(
                        even[index],
                        if even_needed {
                            original_even[index].add(right)
                        } else {
                            original_even[index]
                        }
                    );
                    assert_eq!(
                        odd[index],
                        if odd_needed {
                            original_even[index].sub(right).mul(twiddle)
                        } else {
                            original_odd[index]
                        }
                    );
                }
                if odd_needed {
                    twiddle = twiddle.mul(root);
                }
            }
            assert_eq!(actual_twiddle, twiddle);
        }
    }
}

fn coefficients_v1(prefix: usize) -> Vec<Column> {
    (0..WIDTH_V1)
        .map(|column| {
            Column::from_vec_v1(
                (0..prefix - column % prefix)
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

fn assert_outputs_v1(actual: &[Column], expected: &[Column]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.as_ref(), expected.as_ref());
    }
}

#[test]
fn reused_selected_scratch_preserves_mixed_prefixes_destinations_and_absent_suffixes() {
    let mut scratch = ScratchV1::default();
    for common in 2..=10 {
        let rows = 1usize << common;
        let root = goldilocks_primitive_root_v1(common).unwrap();
        let mut allocation = None;
        for prefix in [rows, 1, rows / 2 + 1, rows / 2 - 1, rows - 1, rows / 2] {
            let coefficients = coefficients_v1(prefix);
            for selected in [
                vec![0],
                vec![rows - 1],
                vec![0, rows / 2, rows - 1],
                (0..rows).step_by(rows.div_ceil(MAX_SELECTED_V1)).collect(),
            ] {
                // Poison even the mathematically absent suffix. Correct output
                // cannot depend on stale cells from an earlier selected path.
                if let Some(fields) = &mut scratch.fields {
                    fields.fill(F(GOLDILOCKS_MODULUS_V1 - 1));
                }
                let expected = evaluate_batch_v1(&coefficients, 1, common, &selected).unwrap();
                let actual = scratch
                    .evaluate_with_v1(&coefficients, 1, common, &selected, |_, _| Ok(()))
                    .unwrap();
                assert_outputs_v1(&actual, &expected);
                for (input, actual) in coefficients.iter().zip(&actual) {
                    let scalar = super::super::evaluate_v1(input, 1, common, &selected).unwrap();
                    assert_eq!(actual.as_ref(), scalar.as_ref());
                    for (&row, &value) in selected.iter().zip(actual.iter()) {
                        let point = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(row as u128));
                        let expected = input
                            .iter()
                            .rev()
                            .fold(F::ZERO, |sum, &coefficient| sum.mul(point).add(coefficient));
                        assert_eq!(value, expected);
                    }
                }
                let fields = scratch.fields.as_ref().unwrap();
                assert_eq!(fields.len(), rows * WIDTH_V1);
                assert_eq!(fields.capacity(), fields.len());
                let identity = fields.as_ptr() as usize;
                assert_eq!(*allocation.get_or_insert(identity), identity);
            }
        }
    }
}

#[test]
fn reused_selected_scratch_preserves_parallel_mask_tails_and_clears_complete_owner_once() {
    let common = 18;
    let rows = 1usize << common;
    let selected = [0, 1, 63, 8191, rows / 2, rows - 1];
    for workers in [1, 4, 20] {
        rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap()
            .install(|| {
                let (_, erased) = super::tests::observe_batch_clear_v1(|| {
                    let mut scratch = ScratchV1::default();
                    for prefix in [PARALLEL_PAIRS_V1 + MASK_DEGREE + 1, 3, rows - 1] {
                        let coefficients = coefficients_v1(prefix);
                        let expected =
                            evaluate_batch_v1(&coefficients, 8, common, &selected).unwrap();
                        let actual = scratch
                            .evaluate_with_v1(&coefficients, 8, common, &selected, |_, _| Ok(()))
                            .unwrap();
                        assert_outputs_v1(&actual, &expected);
                    }
                    // Include dormant cells in the terminal wipe obligation.
                    scratch.fields.as_mut().unwrap().fill(F(71));
                });
                // Three independent oracle owners plus one reused owner.
                assert_eq!(erased.len(), 4);
                assert!(erased.iter().all(|entry| entry.cells == WIDTH_V1 * rows));
                assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
                assert_eq!(erased.last().unwrap().nonzero_before, WIDTH_V1 * rows);
            });
    }
}

#[test]
fn reused_selected_scratch_preserves_refusal_order_and_erases_prior_batch_on_refusal() {
    for failure in 0..13 {
        let mut coefficients = coefficients_v1(17);
        let mut selected = vec![0, 7, 63];
        let mut native = 4;
        let mut common = 6;
        match failure {
            0 => {
                coefficients.pop();
            }
            1 => coefficients.push(Column::from_vec_v1(vec![F::ONE])),
            2 => coefficients[7] = Column::from_vec_v1(vec![]),
            3 => coefficients[7] = Column::from_vec_v1(vec![F::ONE; 65]),
            4 => coefficients[7] = Column::from_vec_v1(vec![F(GOLDILOCKS_MODULUS_V1)]),
            5 => selected.clear(),
            6 => selected = vec![63, 7],
            7 => selected = vec![7, 7],
            8 => selected = vec![64],
            9 => native = 6,
            10 => common = u8::MAX,
            11 => {
                common = 10;
                selected = (0..273).collect();
            }
            _ => common = 23,
        }
        let expected = evaluate_batch_v1(&coefficients, native, common, &selected)
            .err()
            .unwrap();
        for populated in [false, true] {
            let mut scratch = ScratchV1::default();
            if populated {
                scratch
                    .evaluate_with_v1(&coefficients_v1(17), 4, 6, &[0, 63], |_, _| Ok(()))
                    .unwrap();
                scratch.fields.as_mut().unwrap().fill(F(91));
            }
            let ((actual, erased), output_erased) = inspection::observe_v1(|| {
                super::tests::observe_batch_clear_v1(|| {
                    scratch.evaluate_with_v1(&coefficients, native, common, &selected, |_, _| {
                        panic!("invalid inputs reached private writes")
                    })
                })
            });
            assert_eq!(actual.err().unwrap(), expected);
            assert!(scratch.fields.is_none());
            assert!(output_erased.is_empty());
            assert_eq!(erased.len(), usize::from(populated));
            if populated {
                assert_eq!(erased[0].cells, WIDTH_V1 * 64);
                assert_eq!(erased[0].nonzero_before, WIDTH_V1 * 64);
                assert_eq!(erased[0].nonzero_after, 0);
            }
        }
    }
}

#[test]
fn reused_selected_scratch_erases_partial_outputs_and_all_dormant_cells_on_fault_or_unwind() {
    for after_prune in [false, true] {
        for unwind in [false, true] {
            let coefficients = coefficients_v1(13);
            let selected = [0, 7, 63];
            let mut scratch = ScratchV1::default();
            scratch
                .evaluate_with_v1(&coefficients_v1(64), 4, 6, &[0, 63], |_, _| Ok(()))
                .unwrap();
            scratch.fields.as_mut().unwrap().fill(F(97));
            let ((result, erased), outputs_erased) = inspection::observe_v1(|| {
                super::tests::observe_batch_clear_v1(|| {
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        scratch.evaluate_with_v1(&coefficients, 4, 6, &selected, |completed, _| {
                            if completed == after_prune {
                                assert!(!unwind, "injected reused scratch unwind");
                                return Err(AggregateStarkErrorV1::InternalInvariant);
                            }
                            Ok(())
                        })
                    }))
                })
            });
            if unwind {
                assert!(result.is_err());
            } else {
                assert_eq!(
                    result.unwrap().err().unwrap(),
                    AggregateStarkErrorV1::InternalInvariant
                );
            }
            assert!(scratch.fields.is_none());
            assert_eq!(erased.len(), 1);
            assert_eq!(erased[0].cells, WIDTH_V1 * 64);
            assert!(erased[0].nonzero_before > 0);
            assert_eq!(erased[0].nonzero_after, 0);
            assert_eq!(
                outputs_erased
                    .iter()
                    .map(|entry| entry.cells)
                    .sum::<usize>(),
                WIDTH_V1 * selected.len()
            );
            assert!(outputs_erased.iter().all(|entry| entry.nonzero_after == 0));
            // The same public owner may start a new complete transaction after failure.
            let actual = scratch
                .evaluate_with_v1(&coefficients, 4, 6, &selected, |_, _| Ok(()))
                .unwrap();
            assert_outputs_v1(
                &actual,
                &evaluate_batch_v1(&coefficients, 4, 6, &selected).unwrap(),
            );
        }
    }
}

fn selected_v1(rows: usize, variant: usize) -> Vec<usize> {
    let cut_rows = 1 << aggregate::retained_commitment::CUT_LEVEL_V1;
    let selected = (0..QUERY_COUNT)
        .flat_map(|query| {
            let block = match query {
                0 => 0,
                1 => rows / cut_rows - 1,
                _ => ((query - 2) * 1729 + 17 + variant * 31) % (rows / cut_rows),
            };
            block * cut_rows..(block + 1) * cut_rows
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), MAX_SELECTED_V1);
    selected
}

#[test]
#[ignore = "balanced whole-query-batch scratch reuse cost, including allocation and terminal erasure"]
fn reused_selected_scratch_cost_preserves_full_masks_and_all_query_coordinates() {
    selected_scratch_cost_with_workers_v1(&[1, 4, 20]);
}

#[test]
#[ignore = "isolated16-worker complete-batch comparison for hardware selection"]
fn reused_selected_scratch_sixteen_worker_cost_preserves_full_masks_and_all_query_coordinates() {
    selected_scratch_cost_with_workers_v1(&[16]);
}

fn selected_scratch_cost_with_workers_v1(worker_counts: &[usize]) {
    use std::time::{Duration, Instant};

    assert!(!cfg!(debug_assertions), "use an optimized build");
    const COMMON: u8 = 22;
    const BATCHES: usize = 8;
    const ROUNDS: usize = 4;
    let rows = 1usize << COMMON;
    let selections = [selected_v1(rows, 0), selected_v1(rows, 1)];
    let native_logs = [15, 16, 18, 19];
    let coefficients = native_logs.map(|native| {
        (0..WIDTH_V1)
            .map(|column| {
                let native_values = Column::from_vec_v1(
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
                    masked_trace_coefficients_with_mask_v1(&native_values, native, &mask).unwrap(),
                )
            })
            .collect::<Vec<_>>()
    });
    // The full CPU transform independently checks every returned field. It runs
    // once per fixture column outside all candidate timing and owner lifetimes.
    let expected: [[Vec<Column>; 2]; 4] = core::array::from_fn(|index| {
        let mut output: [Vec<Column>; 2] = core::array::from_fn(|_| Vec::new());
        for column in &coefficients[index] {
            let full = Column::from_vec_v1(
                masked_trace_coefficients_on_coset_v1(column, native_logs[index], COMMON).unwrap(),
            );
            for (output, selected) in output.iter_mut().zip(&selections) {
                output.push(Column::from_vec_v1(
                    selected.iter().map(|&row| full[row]).collect(),
                ));
            }
        }
        output
    });
    for &workers in worker_counts {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.install(|| {
            // The fifth case mixes all native logs in one reused allocation.
            for geometry in 0..=native_logs.len() {
                let mut elapsed = [[Duration::ZERO; ROUNDS]; 2];
                for round in 0..ROUNDS {
                    for reused in if round % 2 == 0 { [false, true] } else { [true, false] } {
                        let started = Instant::now();
                        let mut scratch = ScratchV1::default();
                        let mut complete = started.elapsed();
                        for batch in 0..BATCHES {
                            let index = if geometry == native_logs.len() { batch % native_logs.len() } else { geometry };
                            let selection = batch % selections.len();
                            let started = Instant::now();
                            let output = if reused {
                                scratch.evaluate_with_v1(&coefficients[index], native_logs[index], COMMON, &selections[selection], |_, _| Ok(()))
                            } else {
                                evaluate_batch_v1(&coefficients[index], native_logs[index], COMMON, &selections[selection])
                            }.unwrap();
                            complete += started.elapsed();
                            assert_outputs_v1(&output, &expected[index][selection]);
                            let started = Instant::now();
                            drop(output);
                            complete += started.elapsed();
                        }
                        let started = Instant::now();
                        drop(scratch);
                        complete += started.elapsed();
                        elapsed[usize::from(reused)][round] = complete;
                        println!("x509_query_reuse_cost workers={workers} round={round} reused={reused} native_log={} batches={BATCHES} selected_rows={} scratch_bytes={} complete_seconds={:.9} per_batch_seconds={:.9} parity=all_fields_exact; includes allocation, initialized storage, validation, scale, prune, gather, compact-output drop and complete final scratch erasure; excludes fixture construction and comparison",
                            if geometry == native_logs.len() { 0 } else { native_logs[geometry] },
                            MAX_SELECTED_V1, rows * WIDTH_V1 * size_of::<F>(), complete.as_secs_f64(), complete.as_secs_f64() / BATCHES as f64);
                    }
                }
                for values in &mut elapsed { values.sort_unstable(); }
                let medians = elapsed.map(|values| {
                    (values[ROUNDS / 2 - 1].as_secs_f64() + values[ROUNDS / 2].as_secs_f64()) / 2.0
                });
                println!("x509_query_reuse_median workers={workers} native_log={} batches={BATCHES} original_seconds={:.9} reused_seconds={:.9} speedup={:.6} isolated_726_batch_saving_seconds={:.6}; mixed native logs use0; isolated extrapolation is not a full-proof claim",
                    if geometry == native_logs.len() { 0 } else { native_logs[geometry] },
                    medians[0], medians[1],
                    medians[0] / medians[1],
                    (medians[0] - medians[1]) * 726.0 / BATCHES as f64);
            }
        });
    }
}
