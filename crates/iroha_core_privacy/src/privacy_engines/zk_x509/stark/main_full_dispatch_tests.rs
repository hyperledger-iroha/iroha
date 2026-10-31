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
