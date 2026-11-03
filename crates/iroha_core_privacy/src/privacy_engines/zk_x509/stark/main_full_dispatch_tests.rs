//! Actual owner routing, full-value parity and final-column erasure controls.

use super::*;
use crate::privacy_engines::aggregate_stark::field_column_inspection;
use crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;

fn inputs(width: usize) -> Vec<Column> {
    (0..width)
        .map(|column| {
            Column::from_vec_v1(
                (0..21)
                    .map(|degree| F((column * 19 + degree * 7 + 1) as u64))
                    .collect(),
            )
        })
        .collect()
}

fn owner(device_columns: usize) -> MainTraceCosetEvaluatorV1 {
    MainTraceCosetEvaluatorV1 {
        coset_powers: (device_columns != 0).then(|| PublicCosetPowersV1::new_v1(64).unwrap()),
        selected_scratch: selected_coset::SelectedScratchV1::default(),
        evaluation_rows: 64,
        device_columns,
        receipt: MainTransformReceiptV1 {
            observe_full_cpu_routes: true,
            ..MainTransformReceiptV1::default()
        },
    }
}

#[test]
fn full_width_one_through_eight_routes_and_preserves_every_original_value() {
    for workers in [1, 8] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(workers)
            .build()
            .unwrap();
        for width in 1..=8 {
            let columns = inputs(width);
            let expected = columns
                .iter()
                .map(|column| masked_trace_coefficients_on_coset_v1(column, 4, 6).unwrap())
                .collect::<Vec<_>>();
            let mut evaluator = owner(0);
            let actual = pool
                .install(|| evaluator.evaluate_v1(&columns, 4, 6))
                .unwrap();
            assert_eq!(actual.len(), width);
            for (values, original) in actual.iter().zip(&expected) {
                assert_eq!(&**values, original);
            }
            assert_eq!(evaluator.receipt.cpu_columns, width);
            assert_eq!(evaluator.receipt.metal_columns, 0);
            assert_eq!(
                evaluator.receipt.full_prefix_cpu_columns,
                if width == 8 { 8 } else { 0 }
            );
            assert_eq!(
                evaluator.receipt.full_dense_cpu_columns,
                if width == 8 { 0 } else { width }
            );
            let ((), erased) = field_column_inspection::observe_v1(|| drop(actual));
            assert_eq!(erased.len(), width);
            assert!(erased.iter().all(|entry| entry.cells == 64
                && entry.nonzero_before > 0
                && entry.nonzero_after == 0));
        }
    }
}

#[test]
fn selected_and_device_batches_never_enter_the_full_prefix_route() {
    for width in 1..=8 {
        let columns = inputs(width);
        let expected = columns
            .iter()
            .map(|column| masked_trace_coefficients_on_coset_v1(column, 4, 6).unwrap())
            .collect::<Vec<_>>();
        for selected in [None, Some(&[0, 3, 63][..])] {
            for device_columns in [0, 1, 2, 4] {
                if selected.is_none() && device_columns == 0 {
                    continue; // Full CPU routes are exhaustively covered above.
                }
                let mut evaluator = owner(device_columns);
                let actual = evaluator
                    .evaluate_selected_with_v1(&columns, 4, 6, selected, |batch, root| {
                        assert_ne!(device_columns, 0);
                        assert!(batch.len() <= device_columns);
                        transform_goldilocks_columns_v1(
                            batch,
                            root,
                            Direction::Forward,
                            fastpq_prover::ExecutionMode::Cpu,
                        )
                        .map_err(|_| AggregateStarkErrorV1::InvalidLayout)
                    })
                    .unwrap();
                assert_eq!(actual.len(), width);
                for (values, original) in actual.iter().zip(&expected) {
                    match selected {
                        None => assert_eq!(&**values, original),
                        Some(rows) => assert_eq!(
                            &**values,
                            rows.iter().map(|&row| original[row]).collect::<Vec<_>>()
                        ),
                    }
                }
                assert_eq!(evaluator.receipt.cpu_columns, width);
                assert_eq!(evaluator.receipt.full_prefix_cpu_columns, 0);
                assert_eq!(evaluator.receipt.full_dense_cpu_columns, 0);
            }
        }
    }
}

#[test]
fn all_full_widths_refuse_geometry_and_field_errors_before_routing() {
    for width in 1..=8 {
        let mut columns = inputs(width);
        for malformed_field in [false, true] {
            let mut evaluator = owner(0);
            let receipt_before = evaluator.receipt;
            if malformed_field {
                columns[width - 1] = Column::from_vec_v1(vec![F(GOLDILOCKS_MODULUS_V1)]);
            }
            let (result, erased) = field_column_inspection::observe_v1(|| {
                evaluator.evaluate_with_v1(
                    &columns,
                    4,
                    if malformed_field { 6 } else { 7 },
                    |_, _| panic!("refused CPU batch reached device"),
                )
            });
            if malformed_field {
                assert!(matches!(
                    result,
                    Err(AggregateStarkErrorV1::NonCanonicalField)
                ));
            } else {
                assert!(matches!(result, Err(AggregateStarkErrorV1::InvalidLayout)));
            }
            assert!(erased.is_empty());
            assert_eq!(evaluator.receipt, receipt_before);
        }
    }
}

#[test]
fn every_full_route_erases_returned_originals_on_consumer_unwind() {
    for width in 1..=8 {
        let columns = inputs(width);
        let (result, erased) = field_column_inspection::observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let output = owner(0).evaluate_v1(&columns, 4, 6).unwrap();
                assert_eq!(output.len(), width);
                assert!(
                    output
                        .iter()
                        .all(|column| column.iter().any(|value| *value != F::ZERO))
                );
                panic!("consumer unwind with all original output columns populated");
            }))
        });
        assert!(result.is_err());
        assert_eq!(erased.len(), width);
        assert!(erased.iter().all(|entry| entry.cells == 64
            && entry.nonzero_before > 0
            && entry.nonzero_after == 0));
    }
}

#[test]
fn full_native_gate_requires_every_complete_registered_masked_length() {
    use selected_coset::measured_full_prefix_v1;
    assert_eq!(MASK_DEGREE + 1, 1816);
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let native_logs = layout
        .trace_groups
        .iter()
        .map(|group| group.native_trace_log2)
        .collect::<Vec<_>>();
    assert_eq!(native_logs, [5, 8, 15, 16, 18, 19]);
    for native in native_logs {
        let length = (1usize << native) + MASK_DEGREE + 1;
        for width in 0..=9 {
            assert_eq!(
                measured_full_prefix_v1(native, 22, std::iter::repeat_n(length, width)),
                width == 8
            );
        }
        for changed in 0..8 {
            for bad in [0, length - 1, length + 1] {
                let mut lengths = [length; 8];
                lengths[changed] = bad;
                assert!(!measured_full_prefix_v1(native, 22, lengths.into_iter()));
            }
        }
        assert!(!measured_full_prefix_v1(
            native,
            21,
            std::iter::repeat_n(length, 8)
        ));
        assert!(!measured_full_prefix_v1(
            native,
            23,
            std::iter::repeat_n(length, 8)
        ));
    }
    for native in [0, 1, 4, 6, 17, 20, 22, 255] {
        assert!(!measured_full_prefix_v1(
            native,
            22,
            std::iter::repeat_n(1816, 8)
        ));
    }
}

#[test]
fn full_native_admission_is_feature_and_architecture_bound() {
    #[cfg(target_arch = "x86_64")]
    let supported = cfg!(feature = "simd") && std::is_x86_feature_detected!("avx2");
    #[cfg(not(target_arch = "x86_64"))]
    let supported = false;
    assert_eq!(
        selected_coset::FullPrefixBackendV1::detect_v1().is_native_v1(),
        supported
    );
    assert!(!selected_coset::FullPrefixBackendV1::Scalar.is_native_v1());
}

#[cfg(not(feature = "simd"))]
#[test]
fn full_native_feature_off_measured_batch_executes_scalar_and_clears() {
    let rows = 1usize << 22;
    let native = 5;
    let length = (1usize << native) + MASK_DEGREE + 1;
    let columns = (0..8)
        .map(|column| {
            Column::from_vec_v1(
                (0..length)
                    .map(|degree| F((column * 19 + degree * 7 + 1) as u64))
                    .collect(),
            )
        })
        .collect::<Vec<_>>();
    assert!(selected_coset::measured_full_prefix_v1(
        native,
        22,
        columns.iter().map(|column| column.len())
    ));
    let layout = AggregateProofLayoutV1::for_full_profile_v1().unwrap();
    let replay = main_resources::MainProverBufferPlanV1::new_v1(&layout)
        .unwrap()
        .replay_batch;
    assert!(8 * (rows + length) * core::mem::size_of::<F>() <= replay);
    // Stream one dense original column into the oracle, then release it. The
    // complete oracle is gone before the one8N production result is allocated.
    let mut expected = blake3::Hasher::new();
    for source in &columns {
        let dense =
            Column::from_vec_v1(masked_trace_coefficients_on_coset_v1(source, native, 22).unwrap());
        for value in dense.iter() {
            expected.update(&value.0.to_le_bytes());
        }
        drop(dense);
    }
    let expected = expected.finalize();
    let mut evaluator = owner(0);
    evaluator.evaluation_rows = rows;
    let actual = evaluator.evaluate_v1(&columns, native, 22).unwrap();
    assert_eq!(evaluator.receipt.cpu_columns, 8);
    assert_eq!(evaluator.receipt.full_prefix_cpu_columns, 8);
    assert_eq!(evaluator.receipt.full_dense_cpu_columns, 0);
    assert_eq!(evaluator.receipt.full_avx2_cpu_columns, 0);
    assert_eq!(evaluator.receipt.full_avx2_vector_blocks, 0);
    assert_eq!(evaluator.receipt.full_avx2_scalar_pairs, 0);
    assert_eq!(actual.len(), 8);
    let mut digest = blake3::Hasher::new();
    for column in &actual {
        assert_eq!(column.len(), rows);
        for value in column.iter() {
            digest.update(&value.0.to_le_bytes());
        }
    }
    assert_eq!(digest.finalize(), expected);
    let ((), erased) = field_column_inspection::observe_v1(|| drop(actual));
    assert_eq!(erased.len(), 8);
    assert!(
        erased.iter().all(|entry| entry.cells == rows
            && entry.nonzero_before > 0
            && entry.nonzero_after == 0)
    );
}
