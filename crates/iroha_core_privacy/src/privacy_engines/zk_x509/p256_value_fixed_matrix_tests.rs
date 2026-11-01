//! Complete ValueBus fixed matrices preserve every scalar replay cell and custody.

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
fn value_fixed_matrix_all_ten_registrations_match_every_original_column_cell() {
    let source = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    let mut checked = 0;
    for signature in 0..5 {
        for endpoint in 0..2 {
            let registration =
                P256MainRegistrationV1::new_v1(signature, P256MainAdapterV1::ValueBus, endpoint)
                    .unwrap();
            let shape = registration.shape_v1().unwrap();
            assert_eq!(shape.trace_size, 1 << 19);
            assert_eq!(shape.fixed_width, if endpoint == 0 { 46 } else { 22 });
            let mut actual = MatrixV1(vec![vec![F(97); shape.trace_size]; shape.fixed_width]);
            let allocations = actual
                .0
                .iter()
                .map(|column| (column.as_ptr(), column.capacity()))
                .collect::<Vec<_>>();
            source
                .fill_value_fixed_matrix_v1(registration, &mut actual.0)
                .unwrap();
            let mut expected = zeroize::Zeroizing::new(vec![F::ZERO; shape.trace_size]);
            for (column, values) in actual.0.iter().enumerate() {
                source
                    .fill_fixed_column_v1(registration, column, &mut expected)
                    .unwrap();
                assert_eq!(
                    values, &*expected,
                    "signature={signature} endpoint={endpoint} column={column}"
                );
                assert_eq!((values.as_ptr(), values.capacity()), allocations[column]);
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 10);
}

#[test]
fn value_fixed_matrix_geometry_precedes_cleared_phase_without_changing_destination() {
    let source = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    let cleared = P256MainBoundSourceV1 {
        signatures: None,
        fixed: None,
        post_base: None,
        terminal_claims: None,
    };
    for endpoint in 0..2 {
        let registration =
            P256MainRegistrationV1::new_v1(4, P256MainAdapterV1::ValueBus, endpoint).unwrap();
        let shape = registration.shape_v1().unwrap();
        let mut columns = MatrixV1(vec![vec![F(31); shape.trace_size]; shape.fixed_width]);
        assert_eq!(
            cleared.fill_value_fixed_matrix_v1(registration, &mut columns.0),
            Err(P256AggregateAdapterErrorV1::Phase)
        );
        assert!(columns.0.iter().flatten().all(|value| *value == F(31)));
        columns.0[shape.fixed_width - 1].pop();
        for bound in [false, true] {
            let result = if bound {
                cleared.fill_value_fixed_matrix_v1(registration, &mut columns.0)
            } else {
                source.fill_value_fixed_matrix_v1(registration, &mut columns.0)
            };
            assert_eq!(result, Err(P256AggregateAdapterErrorV1::Topology));
            assert!(columns.0.iter().flatten().all(|value| *value == F(31)));
        }
        columns.0.pop();
        assert_eq!(
            source.fill_value_fixed_matrix_v1(registration, &mut columns.0),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
    }
    let wrong = P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::WindowBatch, 0).unwrap();
    let mut columns = MatrixV1(vec![vec![F(31); 8]; 46]);
    assert_eq!(
        source.fill_value_fixed_matrix_v1(wrong, &mut columns.0),
        Err(P256AggregateAdapterErrorV1::Topology)
    );
    assert_eq!(
        cleared.fill_value_fixed_matrix_v1(wrong, &mut columns.0),
        Err(P256AggregateAdapterErrorV1::Topology)
    );
    assert!(columns.0.iter().flatten().all(|value| *value == F(31)));
}

fn guarded_row_failures_v1<const WIDTH: usize>() {
    use super::super::private_table::inspection::observe_v1;
    for unwind in [false, true] {
        let mut matrix = MatrixV1(vec![vec![F(71); 8]; WIDTH]);
        let pointers = matrix.0.iter().map(Vec::as_ptr).collect::<Vec<_>>();
        let (result, erased) = observe_v1(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                fill_public_fixed_matrix_with_v1(8, &mut matrix.0, |row| {
                    if row == 4 {
                        assert!(!unwind, "injected ValueBus fixed-row unwind");
                        return Err(P256AggregateAdapterErrorV1::Source);
                    }
                    Ok(core::array::from_fn::<_, WIDTH, _>(|column| {
                        F((row * 53 + column + 1) as u64)
                    }))
                })
            }))
        });
        assert_eq!(result.is_err(), unwind);
        if !unwind {
            assert_eq!(result.unwrap(), Err(P256AggregateAdapterErrorV1::Source));
        }
        assert!(matrix.0.iter().flatten().all(|value| *value == F::ZERO));
        assert_eq!(
            matrix.0.iter().map(Vec::as_ptr).collect::<Vec<_>>(),
            pointers
        );
        assert_eq!(
            erased.iter().filter(|event| event.cells == 8).count(),
            WIDTH
        );
        assert_eq!(
            erased.iter().filter(|event| event.cells == WIDTH).count(),
            4
        );
        assert!(erased.iter().all(|event| event.nonzero_after == 0));
    }
}

#[test]
fn value_fixed_matrix_partial_errors_and_unwind_clear_rows_and_all_final_columns() {
    guarded_row_failures_v1::<P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1>();
    guarded_row_failures_v1::<P256_VALUE_BUS_STARK_FIXED_WIDTH_V1>();
}
