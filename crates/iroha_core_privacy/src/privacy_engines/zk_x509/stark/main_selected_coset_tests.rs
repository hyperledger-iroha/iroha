//! Independent selected-row polynomial parity and clearing controls.

use super::*;
use crate::privacy_engines::transparent_stark::{
    GOLDILOCKS_MODULUS_V1, masked_trace_coefficients_with_mask_v1,
};
use crate::privacy_engines::zk_x509::private_table::inspection;

fn horner(coefficients: &[F], point: F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |sum, &value| sum.mul(point).add(value))
}

#[test]
fn every_small_coset_selection_matches_horner_full_fft_and_worker_schedules() {
    for common in 2..=10 {
        let native = common - 1;
        let rows = 1usize << common;
        let native_values = (0..1usize << native)
            .map(|i| F((i * 17 + 3) as u64))
            .collect::<Vec<_>>();
        // Tail overlaps the native-size boundary; no coefficient may be cut.
        let mask = (0..(1usize << native).min(23))
            .map(|i| F((i * 31 + 5) as u64))
            .collect::<Vec<_>>();
        let coefficients = Column::from_vec_v1(
            masked_trace_coefficients_with_mask_v1(&native_values, native, &mask).unwrap(),
        );
        let full = Column::from_vec_v1(
            masked_trace_coefficients_on_coset_v1(&coefficients, native, common).unwrap(),
        );
        let root = goldilocks_primitive_root_v1(common).unwrap();
        let cases = [
            vec![0],
            vec![rows - 1],
            vec![0, rows - 1],
            (0..rows).step_by(3).collect(),
            (0..rows).collect(),
        ];
        for workers in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            for selected in &cases {
                let actual = pool
                    .install(|| evaluate_v1(&coefficients, native, common, selected))
                    .unwrap();
                assert_eq!(actual.len(), selected.len());
                for (&row, &value) in selected.iter().zip(actual.iter()) {
                    let point = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(row as u128));
                    assert_eq!(value, full[row]);
                    assert_eq!(value, horner(&coefficients, point));
                }
            }
        }
    }
    // Long masks overlap the native coefficient range; the common15 case also
    // exercises the actual disjoint parallel-stage and recursive-join branches.
    for (native, common, mask_length) in [(2, 7, 81), (5, 12, 1816), (5, 15, 1816)] {
        let native_values = vec![F(17); 1usize << native];
        let mask = (0..mask_length)
            .map(|i| F((i * 31 + 7) as u64))
            .collect::<Vec<_>>();
        let coefficients = Column::from_vec_v1(
            masked_trace_coefficients_with_mask_v1(&native_values, native, &mask).unwrap(),
        );
        let full = Column::from_vec_v1(
            masked_trace_coefficients_on_coset_v1(&coefficients, native, common).unwrap(),
        );
        let rows = 1usize << common;
        let selected = [0, 1, 7, rows / 2, rows - 1];
        let root = goldilocks_primitive_root_v1(common).unwrap();
        for workers in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            let actual = pool
                .install(|| evaluate_v1(&coefficients, native, common, &selected))
                .unwrap();
            for (&row, &value) in selected.iter().zip(actual.iter()) {
                assert_eq!(value, full[row]);
                assert_eq!(
                    value,
                    horner(
                        &coefficients,
                        F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(row as u128))
                    )
                );
            }
        }
    }
}

#[test]
fn complete_input_and_selected_geometry_validate_before_private_scratch_writes() {
    let mut coefficients = vec![F(7); 21];
    for selected in [vec![], vec![64], vec![2, 1], vec![1, 1]] {
        assert!(evaluate_v1(&coefficients, 4, 6, &selected).is_err());
    }
    coefficients[20] = F(GOLDILOCKS_MODULUS_V1);
    let (result, erased) = inspection::observe_v1(|| evaluate_v1(&coefficients, 4, 6, &[0]));
    assert!(matches!(
        result,
        Err(AggregateStarkErrorV1::NonCanonicalField)
    ));
    assert!(
        erased.is_empty(),
        "all original coefficients are checked before scratch ownership"
    );
    assert!(evaluate_v1(&[], 4, 6, &[0]).is_err());
    assert!(evaluate_v1(&vec![F::ONE; 65], 4, 6, &[0]).is_err());
    assert!(evaluate_v1(&[F::ONE], 6, 6, &[0]).is_err());
}

#[test]
fn scratch_and_compact_coordinate_storage_clear_on_success_and_unwind() {
    let coefficients = vec![F(11); 21];
    let selected = [0, 7, 63];
    for unwind in [false, true] {
        let (result, erased) = inspection::observe_v1(|| {
            std::panic::catch_unwind(|| {
                evaluate_with_v1(&coefficients, 4, 6, &selected, |scratch| {
                    assert!(scratch.iter().any(|value| *value != F::ZERO));
                    if unwind {
                        panic!("selected FFT after-scaling unwind");
                    }
                })
            })
        });
        if unwind {
            assert!(result.is_err());
            assert_eq!(
                erased.iter().map(|row| row.cells).sum::<usize>(),
                64 + selected.len()
            );
        } else {
            assert_eq!(result.unwrap().unwrap().len(), selected.len());
            assert_eq!(erased.iter().map(|row| row.cells).sum::<usize>(), 64);
        }
        assert!(erased.iter().any(|row| row.nonzero_before > 0));
        assert!(erased.iter().all(|row| row.nonzero_after == 0));
    }
}

#[test]
#[ignore = "full common22 selected CPU FFT parity for every registered native domain; component evidence only"]
fn registered_native_domains_keep_high_mask_tail_and_all_selected_cut_rows() {
    let common = 22;
    let rows = 1usize << common;
    let selected = (0..136)
        .flat_map(|query| {
            let block = (query * 1_729 + 17) % (rows / 16);
            block * 16..block * 16 + 16
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    assert!(selected.len() <= 2176);
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let native_domains = layout
        .trace_groups
        .iter()
        .map(|group| group.native_trace_log2)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(native_domains, [5, 8, 15, 16, 18, 19].into_iter().collect());
    for native in native_domains {
        let values = (0..1usize << native)
            .map(|i| F((i * 17 + 13) as u64))
            .collect::<Vec<_>>();
        let mask = (0..1816)
            .map(|i| F((i * 29 + 7) as u64))
            .collect::<Vec<_>>();
        let coefficients = Column::from_vec_v1(
            masked_trace_coefficients_with_mask_v1(&values, native, &mask).unwrap(),
        );
        let expected = Column::from_vec_v1(
            masked_trace_coefficients_on_coset_v1(&coefficients, native, common).unwrap(),
        );
        let actual = evaluate_v1(&coefficients, native, common, &selected).unwrap();
        assert_eq!(actual.len(), selected.len());
        for (&row, &value) in selected.iter().zip(actual.iter()) {
            assert_eq!(value, expected[row]);
        }
    }
}

/// A test-only full transform with the same N+m clearing-owner overlap as
/// selected evaluation. All shape/canonical checks precede private writes.
fn full_compact_reference_v1(
    coefficients: &[F],
    native: u8,
    common: u8,
    selected: &[usize],
    after_transform: impl FnOnce(&[F]),
) -> Result<Column, AggregateStarkErrorV1> {
    let rows = 1usize
        .checked_shl(u32::from(common))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let native_rows = 1usize
        .checked_shl(u32::from(native))
        .ok_or(AggregateStarkErrorV1::InvalidLayout)?;
    let root = goldilocks_primitive_root_v1(common).map_err(aggregate::map_transparent_error_v1)?;
    let shift = F(GOLDILOCKS_GENERATOR_V1);
    if rows <= native_rows
        || coefficients.is_empty()
        || coefficients.len() > rows
        || selected.is_empty()
        || selected.last().is_none_or(|&row| row >= rows)
        || selected.windows(2).any(|pair| pair[0] >= pair[1])
        || shift.pow(rows as u128) == F::ONE
        || shift.pow(native_rows as u128) == F::ONE
    {
        return Err(AggregateStarkErrorV1::InvalidLayout);
    }
    if coefficients
        .iter()
        .any(|value| F::canonical(value.0).is_none())
    {
        return Err(AggregateStarkErrorV1::NonCanonicalField);
    }
    let mut scratch = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
    scratch
        .try_reserve_exact(rows)
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    let mut compact = PrivateTableV1::new(Vec::new(), zeroize_fields_v1);
    compact
        .try_reserve_exact(selected.len())
        .map_err(|_| AggregateStarkErrorV1::AllocationFailure)?;
    if scratch.capacity() != rows || compact.capacity() != selected.len() {
        return Err(AggregateStarkErrorV1::AllocationFailure);
    }
    scratch.resize(rows, F::ZERO);
    // Initialize the entire compact allocation before the injected unwind point.
    compact.resize(selected.len(), F::ZERO);
    let mut power = F::ONE;
    for (target, coefficient) in scratch.iter_mut().zip(coefficients) {
        *target = coefficient.mul(power);
        power = power.mul(shift);
    }
    crate::privacy_engines::transparent_stark::goldilocks_fft_v1(&mut scratch, root)
        .map_err(aggregate::map_transparent_error_v1)?;
    after_transform(&scratch);
    for (target, &index) in compact.iter_mut().zip(selected) {
        *target = scratch[index];
    }
    // This N-cell clear occurs before return, exactly as in evaluate_v1.
    drop(scratch);
    Ok(Column::from_vec_v1(compact.into_vec()))
}

#[test]
fn full_compact_cost_oracle_preserves_geometry_parity_and_clearing() {
    for common in 2..=9 {
        let native = common - 1;
        let rows = 1usize << common;
        let coefficients = (0..rows - 1)
            .map(|i| F((17 * i + 3) as u64))
            .collect::<Vec<_>>();
        let root = goldilocks_primitive_root_v1(common).unwrap();
        for selected in [
            vec![0],
            vec![rows - 1],
            (0..rows).step_by(3).collect(),
            (0..rows).collect(),
        ] {
            let full = full_compact_reference_v1(&coefficients, native, common, &selected, |_| {})
                .unwrap();
            let pruned = evaluate_v1(&coefficients, native, common, &selected).unwrap();
            assert_eq!(&*full, &*pruned);
            for (&row, &value) in selected.iter().zip(full.iter()) {
                assert_eq!(
                    value,
                    horner(
                        &coefficients,
                        F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(row as u128))
                    )
                );
            }
        }
    }
    let mut coefficients = vec![F(11); 21];
    for selected in [vec![], vec![64], vec![2, 1], vec![1, 1]] {
        let (result, erased) = inspection::observe_v1(|| {
            full_compact_reference_v1(&coefficients, 4, 6, &selected, |_| {
                panic!("invalid geometry must precede private writes")
            })
        });
        assert!(result.is_err());
        assert!(erased.is_empty());
    }
    coefficients[20] = F(GOLDILOCKS_MODULUS_V1);
    let (result, erased) = inspection::observe_v1(|| {
        full_compact_reference_v1(&coefficients, 4, 6, &[0], |_| {
            panic!("canonicality must precede private writes")
        })
    });
    assert!(matches!(
        result,
        Err(AggregateStarkErrorV1::NonCanonicalField)
    ));
    assert!(erased.is_empty());
    coefficients[20] = F(19);
    for unwind in [false, true] {
        let (result, erased) = inspection::observe_v1(|| {
            std::panic::catch_unwind(|| {
                full_compact_reference_v1(&coefficients, 4, 6, &[0, 7, 63], |scratch| {
                    assert!(scratch.iter().any(|value| *value != F::ZERO));
                    if unwind {
                        panic!("full compact oracle after-transform unwind");
                    }
                })
            })
        });
        if unwind {
            assert!(result.is_err());
            assert_eq!(erased.iter().map(|row| row.cells).sum::<usize>(), 64 + 3);
        } else {
            assert_eq!(result.unwrap().unwrap().len(), 3);
            assert_eq!(erased.iter().map(|row| row.cells).sum::<usize>(), 64);
        }
        assert!(erased.iter().any(|row| row.nonzero_before > 0));
        assert!(erased.iter().all(|row| row.nonzero_after == 0));
    }
}

#[test]
#[ignore = "isolated common22 CPU transform/gather/clear comparison; public fixture component evidence only"]
fn selected_cpu_pruned_and_full_compact_cost_include_original_scratch_clearing() {
    use std::time::Instant;
    let common = 22;
    let rows = 1usize << common;
    let workers = rayon::current_num_threads();
    println!(
        "x509_selected_cpu_cost_header common_log={common} workers={workers} rounds=3 production_policy_changed=false"
    );
    let cases = [
        ("singleton", vec![rows - 1]),
        (
            "spread_cut16",
            (0..136)
                .flat_map(|query| {
                    let block = (query * 1729 + 17) % (rows / 16);
                    block * 16..block * 16 + 16
                })
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect(),
        ),
        ("clustered_cut16", (rows - 136 * 16..rows).collect()),
    ];
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let domains = layout
        .trace_groups
        .iter()
        .map(|group| group.native_trace_log2)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(domains, [5, 8, 15, 16, 18, 19].into_iter().collect());
    for native in domains {
        for width in [1, aggregate::MASKED_TRACE_LDE_COLUMN_BATCH_V1] {
            let columns = (0..width)
                .map(|column| {
                    let values = (0..1usize << native)
                        .map(|row| F((17 * row + 13 * column + 1) as u64))
                        .collect::<Vec<_>>();
                    let mask = (0..1816)
                        .map(|degree| F((29 * degree + 7 * column + 3) as u64))
                        .collect::<Vec<_>>();
                    Column::from_vec_v1(
                        masked_trace_coefficients_with_mask_v1(&values, native, &mask).unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            for (name, selected) in &cases {
                assert!(!selected.is_empty() && selected.len() <= 2176);
                // Public fixture expected outputs are retained outside timed owners.
                // Their exact bm bytes are diagnostic-only; each compared path
                // independently retains at most bN scratch + bm compact output.
                let expected = columns
                    .iter()
                    .map(|column| {
                        full_compact_reference_v1(column, native, common, selected, |_| {}).unwrap()
                    })
                    .collect::<Vec<_>>();
                let path_field_bytes = width * (rows + selected.len()) * core::mem::size_of::<F>();
                let comparison_field_bytes = width * selected.len() * core::mem::size_of::<F>();
                for round in 0..3 {
                    for pruned in if round % 2 == 0 {
                        [true, false]
                    } else {
                        [false, true]
                    } {
                        let started = Instant::now();
                        let actual = columns
                            .par_iter()
                            .map(|column| {
                                if pruned {
                                    evaluate_v1(column, native, common, selected)
                                } else {
                                    full_compact_reference_v1(
                                        column,
                                        native,
                                        common,
                                        selected,
                                        |_| {},
                                    )
                                }
                            })
                            .collect::<Result<Vec<_>, _>>()
                            .unwrap();
                        let transform_gather_and_scratch_clear = started.elapsed();
                        assert_eq!(actual.len(), width);
                        for (actual, expected) in actual.iter().zip(&expected) {
                            assert_eq!(actual.len(), selected.len());
                            assert_eq!(&**actual, &**expected);
                        }
                        let clear_started = Instant::now();
                        drop(actual);
                        let compact_clear = clear_started.elapsed();
                        println!(
                            "x509_selected_cpu_cost native_log={native} common_log={common} width={width} selection={name} selected_rows={} round={round} path={} transform_gather_scratch_clear_ns={} compact_clear_ns={} path_field_bytes={path_field_bytes} comparison_field_bytes={comparison_field_bytes}",
                            selected.len(),
                            if pruned { "pruned" } else { "full_compact" },
                            transform_gather_and_scratch_clear.as_nanos(),
                            compact_clear.as_nanos()
                        );
                    }
                }
            }
        }
    }
}
