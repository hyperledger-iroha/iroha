//! Proposed, unexecuted full-output public-prefix parity and custody controls.

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

// Empty selected destinations mean no outputs; the same public empty slice
// under the full specialization means every output. Exercise both compile-time
// inner schedules and compare all four leaves with independent Horner values.
fn static_destination_extents_v1<const INNER_PARALLEL: bool>() {
    let original = [F(2), F(3), F(5), F(7)];
    let root = goldilocks_primitive_root_v1(2).unwrap();
    let mut no_outputs = original;
    prune_v1::<INNER_PARALLEL, false>(&mut no_outputs, 0, &[], root, 4);
    assert_eq!(no_outputs, original);
    let mut full_outputs = original;
    prune_v1::<INNER_PARALLEL, true>(&mut full_outputs, 0, &[], root, 4);
    let mut every_selected_output = original;
    prune_v1::<INNER_PARALLEL, false>(
        &mut every_selected_output,
        0,
        &[F(0), F(1), F(2), F(3)],
        root,
        4,
    );
    assert_eq!(full_outputs, every_selected_output);
    assert_ne!(full_outputs, original);
    for row in 0usize..4 {
        let destination = row.reverse_bits() >> (usize::BITS - 2);
        assert_eq!(
            full_outputs[destination],
            horner(&original, root.pow(row as u128))
        );
    }
}

#[test]
fn full_prefix_matches_dense_fft_and_horner_at_every_public_split() {
    for workers in [1, 8] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        pool.install(|| {
            static_destination_extents_v1::<false>();
            static_destination_extents_v1::<true>();
        });
        for common in 2..=9 {
            let rows = 1usize << common;
            let half = rows / 2;
            let lengths = [1, 2, half - 1, half, half + 1, rows - 1, rows]
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>();
            for length in lengths {
                let mut coefficients = (0..length)
                    .map(|index| F((index * 137 + 19) as u64))
                    .collect::<Vec<_>>();
                for index in (0..length.saturating_sub(1)).step_by(3) {
                    coefficients[index] = F::ZERO;
                }
                // A nonzero final coefficient forbids truncating the public tail.
                assert_ne!(coefficients[length - 1], F::ZERO);
                let expected = Column::from_vec_v1(
                    masked_trace_coefficients_on_coset_v1(&coefficients, 1, common).unwrap(),
                );
                let actual = pool
                    .install(|| evaluate_full_v1(&coefficients, 1, common))
                    .unwrap();
                assert_eq!(&*actual, &*expected);
                let root = goldilocks_primitive_root_v1(common).unwrap();
                for (row, &value) in actual.iter().enumerate() {
                    let point = F(GOLDILOCKS_GENERATOR_V1).mul(root.pow(row as u128));
                    assert_eq!(value, horner(&coefficients, point));
                }
                // Selected output keeps its exact original shared-kernel semantics.
                let selected = [0, half, rows - 1];
                let compact = pool
                    .install(|| evaluate_v1(&coefficients, 1, common, &selected))
                    .unwrap();
                for (&row, &value) in selected.iter().zip(compact.iter()) {
                    assert_eq!(value, actual[row]);
                }
            }
        }
    }
}

#[test]
fn full_prefix_retains_long_mask_tail_and_parallel_public_splits() {
    for (native, common, mask_length) in [(2, 7, 81), (5, 12, 1816), (5, 15, 1816)] {
        let values = vec![F(17); 1usize << native];
        let mask = (0..mask_length)
            .map(|index| F((index * 31 + 7) as u64))
            .collect::<Vec<_>>();
        let coefficients = Column::from_vec_v1(
            masked_trace_coefficients_with_mask_v1(&values, native, &mask).unwrap(),
        );
        assert_eq!(coefficients.len(), (1usize << native) + mask_length);
        let expected = Column::from_vec_v1(
            masked_trace_coefficients_on_coset_v1(&coefficients, native, common).unwrap(),
        );
        for workers in [1, 8] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            let actual = pool
                .install(|| evaluate_full_v1(&coefficients, native, common))
                .unwrap();
            assert_eq!(&*actual, &*expected);
        }
    }
}

#[test]
fn full_prefix_rejects_original_bad_inputs_before_private_writes() {
    for (values, native, common) in [
        (vec![], 4, 6),
        (vec![F::ONE; 65], 4, 6),
        (vec![F::ONE], 6, 6),
        (vec![F::ONE], 7, 6),
        (vec![F::ONE], 255, 6),
        (vec![F::ONE], 4, 255),
        (vec![F::ONE], 4, 33),
        (vec![F(GOLDILOCKS_MODULUS_V1)], 6, 6),
        (vec![F(GOLDILOCKS_MODULUS_V1)], 4, 6),
    ] {
        let expected = masked_trace_coefficients_on_coset_v1(&values, native, common)
            .map_err(aggregate::map_transparent_error_v1)
            .err()
            .unwrap();
        let (actual, erased) = inspection::observe_v1(|| {
            evaluate_full_with_v1(
                &values,
                native,
                common,
                |_| panic!("invalid input reached private scratch"),
                |_| panic!("invalid input reached transform"),
            )
        });
        assert_eq!(actual.err().unwrap(), expected);
        assert!(erased.is_empty());
    }
}

#[test]
fn full_prefix_returns_original_allocation_and_clears_populated_failure_paths() {
    let coefficients = vec![F(11); 21];
    let original = std::cell::Cell::new(std::ptr::null());
    let output = evaluate_full_with_v1(
        &coefficients,
        4,
        6,
        |values| {
            assert_eq!(values.len(), 64);
            original.set(values.as_ptr());
            Ok(())
        },
        |values| {
            assert_eq!(values.as_ptr(), original.get());
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(output.len(), 64);
    assert_eq!(output.as_ptr(), original.get());
    drop(output);
    for after_transform in [false, true] {
        for unwind in [false, true] {
            let reached = std::cell::Cell::new(false);
            let inject = |values: &[F]| {
                assert_eq!(values.len(), 64);
                assert!(values.iter().any(|value| *value != F::ZERO));
                reached.set(true);
                if unwind {
                    panic!("full prefix populated-owner unwind");
                }
                Err(AggregateStarkErrorV1::AllocationFailure)
            };
            let (result, erased) = inspection::observe_v1(|| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    evaluate_full_with_v1(
                        &coefficients,
                        4,
                        6,
                        |values| {
                            if after_transform {
                                Ok(())
                            } else {
                                inject(values)
                            }
                        },
                        |values| {
                            if after_transform {
                                inject(values)
                            } else {
                                Ok(())
                            }
                        },
                    )
                }))
            });
            assert!(reached.get());
            if unwind {
                assert!(result.is_err());
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(AggregateStarkErrorV1::AllocationFailure)
                ));
            }
            assert_eq!(erased.iter().map(|entry| entry.cells).sum::<usize>(), 64);
            assert!(erased.iter().any(|entry| entry.nonzero_before > 0));
            assert!(erased.iter().all(|entry| entry.nonzero_after == 0));
        }
    }
}

#[test]
fn full_prefix_cpu_batch_keeps_original_columns_and_device_boundary() {
    let columns = (0..3)
        .map(|column| Column::from_vec_v1(vec![F(11 + column); 21]))
        .collect::<Vec<_>>();
    let mut evaluator = MainTraceCosetEvaluatorV1 {
        evaluation_rows: 64,
        device_columns: 0,
        receipt: MainTransformReceiptV1::default(),
    };
    let actual = evaluator
        .evaluate_with_v1(&columns, 4, 6, |_, _| {
            panic!("CPU selected device dispatch")
        })
        .unwrap();
    assert_eq!(actual.len(), columns.len());
    for (coefficients, values) in columns.iter().zip(&actual) {
        let expected = masked_trace_coefficients_on_coset_v1(coefficients, 4, 6).unwrap();
        assert_eq!(&**values, &expected);
    }
    assert_eq!(evaluator.receipt.cpu_columns, columns.len());
    assert_eq!(evaluator.receipt.metal_columns, 0);
}

#[test]
#[ignore = "full common22 public-prefix parity at every registered native domain; unqualified draft"]
fn full_prefix_registered_domains_keep_every_output_and_mask_tail() {
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let domains = layout
        .trace_groups
        .iter()
        .map(|group| group.native_trace_log2)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(domains, [5, 8, 15, 16, 18, 19].into_iter().collect());
    for native in domains {
        let native_values = (0..1usize << native)
            .map(|index| F((index * 17 + 13) as u64))
            .collect::<Vec<_>>();
        let mask = (0..1816)
            .map(|index| F((index * 29 + 7) as u64))
            .collect::<Vec<_>>();
        let coefficients = Column::from_vec_v1(
            masked_trace_coefficients_with_mask_v1(&native_values, native, &mask).unwrap(),
        );
        let expected = Column::from_vec_v1(
            masked_trace_coefficients_on_coset_v1(&coefficients, native, 22).unwrap(),
        );
        for workers in [1, 8] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            let actual = pool
                .install(|| evaluate_full_v1(&coefficients, native, 22))
                .unwrap();
            assert_eq!(&*actual, &*expected);
        }
    }
}
