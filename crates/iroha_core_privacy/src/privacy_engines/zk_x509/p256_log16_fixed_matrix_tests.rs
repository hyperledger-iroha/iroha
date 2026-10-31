//! Full public Log16 row emission, independent column replay and atomic clearing.

use super::*;

struct MatrixV1(Vec<Vec<F>>);
impl Drop for MatrixV1 {
    fn drop(&mut self) {
        for column in &mut self.0 {
            super::super::private_table::zeroize_fields_v1(column);
        }
    }
}

#[test]
fn log16_fixed_matrix_every_cell_matches_original_all_ten_registrations() {
    let source = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    for adapter in [
        P256MainAdapterV1::WindowBatch,
        P256MainAdapterV1::BindingSink,
    ] {
        for signature in 0..5 {
            let registration = P256MainRegistrationV1::new_v1(signature, adapter, 0).unwrap();
            let shape = registration.shape_v1().unwrap();
            let mut actual = MatrixV1(vec![vec![F(97); shape.trace_size]; shape.fixed_width]);
            let allocation = actual
                .0
                .iter()
                .map(|v| (v.as_ptr(), v.capacity()))
                .collect::<Vec<_>>();
            source
                .fill_log16_fixed_matrix_v1(registration, &mut actual.0)
                .unwrap();
            let mut expected = zeroize::Zeroizing::new(vec![F::ZERO; shape.trace_size]);
            for (column, values) in actual.0.iter().enumerate() {
                source
                    .fill_fixed_column_v1(registration, column, &mut expected)
                    .unwrap();
                assert_eq!(
                    values, &*expected,
                    "signature={signature} adapter={adapter:?} column={column}"
                );
                assert_eq!((values.as_ptr(), values.capacity()), allocation[column]);
            }
        }
    }
}

#[test]
fn log16_fixed_matrix_shape_refusals_preserve_destination() {
    let source = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    for adapter in [
        P256MainAdapterV1::WindowBatch,
        P256MainAdapterV1::BindingSink,
    ] {
        let registration = P256MainRegistrationV1::new_v1(2, adapter, 0).unwrap();
        let shape = registration.shape_v1().unwrap();
        for fault in 0..3 {
            let mut columns = match fault {
                0 => vec![vec![F(23); 8]; shape.fixed_width],
                1 => vec![vec![F(23); shape.trace_size]; shape.fixed_width - 1],
                _ => vec![vec![F(23); shape.trace_size]; shape.fixed_width],
            };
            if fault == 2 {
                columns[shape.fixed_width - 1].pop();
            }
            let before = columns.clone();
            assert_eq!(
                source.fill_log16_fixed_matrix_v1(registration, &mut columns),
                Err(P256AggregateAdapterErrorV1::Topology)
            );
            assert_eq!(columns, before);
        }
    }
    let wrong = P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::Arithmetic, 0).unwrap();
    let mut columns = vec![vec![F(31); 8]; P256_WINDOW_AGGREGATE_FIXED_WIDTH_V1];
    let before = columns.clone();
    assert_eq!(
        source.fill_log16_fixed_matrix_v1(wrong, &mut columns),
        Err(P256AggregateAdapterErrorV1::Topology)
    );
    assert_eq!(columns, before);
}

#[test]
fn log16_fixed_matrix_partial_error_and_unwind_clear_all_destinations_and_row_scratch() {
    use super::super::private_table::inspection::observe_v1;
    for outcome in 0..3 {
        let mut columns = MatrixV1(vec![vec![F(91); 8]; P256_WINDOW_AGGREGATE_FIXED_WIDTH_V1]);
        let allocations = columns.0.iter().map(Vec::as_ptr).collect::<Vec<_>>();
        let (result, erased) = observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                fill_public_fixed_matrix_with_v1(8, &mut columns.0, |row| {
                    if row == 4 {
                        if outcome == 1 {
                            return Err(P256AggregateAdapterErrorV1::Topology);
                        }
                        assert!(outcome != 2, "injected public row unwind");
                    }
                    Ok(core::array::from_fn::<
                        _,
                        P256_WINDOW_AGGREGATE_FIXED_WIDTH_V1,
                        _,
                    >(|column| {
                        F((row * 53 + column + 1) as u64)
                    }))
                })
            }))
        });
        assert_eq!(result.is_err(), outcome == 2);
        if outcome == 0 {
            assert!(result.unwrap().is_ok());
            for (column, values) in columns.0.iter().enumerate() {
                for (row, value) in values.iter().enumerate() {
                    assert_eq!(*value, F((row * 53 + column + 1) as u64));
                }
            }
        } else {
            if outcome == 1 {
                assert_eq!(result.unwrap(), Err(P256AggregateAdapterErrorV1::Topology));
            }
            assert!(columns.0.iter().flatten().all(|value| *value == F::ZERO));
            assert_eq!(
                erased.iter().filter(|event| event.cells == 8).count(),
                P256_WINDOW_AGGREGATE_FIXED_WIDTH_V1
            );
        }
        assert!(
            erased
                .iter()
                .any(|event| event.cells == P256_WINDOW_AGGREGATE_FIXED_WIDTH_V1
                    && event.nonzero_before > 0)
        );
        assert!(erased.iter().all(|event| event.nonzero_after == 0));
        assert_eq!(
            columns.0.iter().map(Vec::as_ptr).collect::<Vec<_>>(),
            allocations
        );
    }
}
