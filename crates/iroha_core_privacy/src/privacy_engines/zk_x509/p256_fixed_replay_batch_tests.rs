//! Bounded public arithmetic fixed replay with unchanged verifier schedules.
use super::*;

#[test]
fn public_fixed_matrix_constructs_each_row_once_without_reallocating_columns() {
    let rows = 8;
    let mut columns = vec![vec![F(97); rows]; P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1];
    let allocations = columns
        .iter()
        .map(|column| (column.as_ptr(), column.len(), column.capacity()))
        .collect::<Vec<_>>();
    let mut calls = 0;
    fill_public_arithmetic_fixed_matrix_with_v1(rows, &mut columns, |row| {
        assert_eq!(row, calls);
        calls += 1;
        Ok(core::array::from_fn(|column| {
            F((row * P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1 + column + 1) as u64)
        }))
    })
    .unwrap();
    assert_eq!(calls, rows);
    for (index, column) in columns.iter().enumerate() {
        assert_eq!(
            (column.as_ptr(), column.len(), column.capacity()),
            allocations[index]
        );
        for (row, value) in column.iter().enumerate() {
            assert_eq!(
                *value,
                F((row * P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1 + index + 1) as u64)
            );
        }
    }
}

#[test]
fn public_fixed_matrix_rejects_malformed_shapes_before_source_or_mutation() {
    let width = P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1;
    for (rows, count, column_rows) in [
        (0, width, 0),
        (3, width, 3),
        (8, 0, 8),
        (8, width - 1, 8),
        (8, width + 1, 8),
        (8, width, 7),
        (8, width, 9),
    ] {
        let mut columns = vec![vec![F(97); column_rows]; count];
        let expected = columns.clone();
        let mut calls = 0;
        assert_eq!(
            fill_public_arithmetic_fixed_matrix_with_v1(rows, &mut columns, |_| {
                calls += 1;
                Ok([F::ONE; P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1])
            }),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
        assert_eq!(calls, 0);
        assert_eq!(columns, expected);
    }
    let mut columns = vec![vec![F(97); 8]; width];
    columns[width / 2].pop();
    let expected = columns.clone();
    assert_eq!(
        fill_public_arithmetic_fixed_matrix_with_v1(8, &mut columns, |_| {
            panic!("malformed matrix must not consult the row source")
        }),
        Err(P256AggregateAdapterErrorV1::Topology)
    );
    assert_eq!(columns, expected);
}

#[test]
fn public_fixed_matrix_clears_every_destination_on_source_error_and_unwind() {
    let rows = 8;
    for fail_at in [0, 3, rows - 1] {
        let mut columns = vec![vec![F(97); rows]; P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1];
        let mut calls = 0;
        assert_eq!(
            fill_public_arithmetic_fixed_matrix_with_v1(rows, &mut columns, |row| {
                calls += 1;
                if row == fail_at {
                    return Err(P256AggregateAdapterErrorV1::Source);
                }
                Ok([F::ONE; P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1])
            }),
            Err(P256AggregateAdapterErrorV1::Source)
        );
        assert_eq!(calls, fail_at + 1);
        assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
    }
    let mut columns = vec![vec![F(97); rows]; P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1];
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = fill_public_arithmetic_fixed_matrix_with_v1(rows, &mut columns, |row| {
            assert_ne!(row, 3, "synthetic fixed row source unwind");
            Ok([F::ONE; P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1])
        });
    }));
    assert!(result.is_err());
    assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
}

#[test]
fn fixed_arithmetic_matrix_rejects_other_owners_and_wrong_native_shapes() {
    let fixed = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    let arithmetic = P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::Arithmetic, 0).unwrap();
    let mut invalid_signature = arithmetic;
    invalid_signature.signature = P256_X5S1_SIGNATURES_V1 as u8;
    let mut invalid_local = arithmetic;
    invalid_local.local_instance = 1;
    for registration in [
        arithmetic,
        invalid_signature,
        invalid_local,
        P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::ValueBus, 0).unwrap(),
        P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::BindingSink, 0).unwrap(),
    ] {
        let mut columns = vec![vec![F(97); 8]; P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1];
        assert_eq!(
            fixed.fill_arithmetic_fixed_matrix_v1(registration, &mut columns),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
        assert!(columns.iter().flatten().all(|value| *value == F(97)));
    }
}

#[test]
fn fixed_arithmetic_batch_rejects_bad_ranges_and_nonarithmetic_owners() {
    let fixed = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    let arithmetic = P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::Arithmetic, 0).unwrap();
    for (count, first, rows) in [
        (0, 0, 8),
        (9, 0, 8),
        (1, usize::MAX, 8),
        (1, 134, 8),
        (1, 0, 8),
    ] {
        let mut columns = vec![vec![F(97); rows]; count];
        let mut targets = columns
            .iter_mut()
            .map(Vec::as_mut_slice)
            .collect::<Vec<_>>();
        assert_eq!(
            fixed.fill_arithmetic_fixed_columns_v1(arithmetic, first, &mut targets),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
        assert!(columns.iter().flatten().all(|value| *value == F(97)));
    }
    for adapter in [
        P256MainAdapterV1::ValueBus,
        P256MainAdapterV1::BindingSink,
        P256MainAdapterV1::WindowBatch,
    ] {
        let registration = P256MainRegistrationV1::new_v1(0, adapter, 0).unwrap();
        let mut output = [F(97); 8];
        assert_eq!(
            fixed.fill_arithmetic_fixed_columns_v1(registration, 0, &mut [&mut output]),
            Err(P256AggregateAdapterErrorV1::Topology)
        );
        assert_eq!(output, [F(97); 8]);
    }
}

#[test]
#[ignore = "every fixed field at every native row for both verifier-owned role schedules"]
fn fixed_arithmetic_batches_match_full_rows_scalar_columns_and_parallel_coefficients() {
    let fixed = P256MainVerifierFixedSourceV1::new_v1().unwrap();
    let root = crate::privacy_engines::transparent_stark::goldilocks_primitive_root_v1(19).unwrap();
    for index in [0, P256_X5S1_SIGNATURES_V1 - 1] {
        let registration =
            P256MainRegistrationV1::new_v1(index, P256MainAdapterV1::Arithmetic, 0).unwrap();
        let shape = registration.shape_v1().unwrap();
        let mut matrix = vec![vec![F::ZERO; shape.trace_size]; shape.fixed_width];
        let matrix_started = std::time::Instant::now();
        fixed
            .fill_arithmetic_fixed_matrix_v1(registration, &mut matrix)
            .unwrap();
        let matrix_elapsed = matrix_started.elapsed();
        let mut batch_elapsed = std::time::Duration::ZERO;
        for first in (0..shape.fixed_width).step_by(8) {
            let count = 8.min(shape.fixed_width - first);
            let mut columns = vec![vec![F::ZERO; shape.trace_size]; count];
            let mut targets = columns
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>();
            let batch_started = std::time::Instant::now();
            fixed
                .fill_arithmetic_fixed_columns_v1(registration, first, &mut targets)
                .unwrap();
            batch_elapsed += batch_started.elapsed();
            drop(targets);
            for (offset, column) in columns.iter().enumerate() {
                assert_eq!(column, &matrix[first + offset]);
            }
            for row in 0..shape.trace_size {
                let expected = fixed
                    .arithmetic_v1(registration.role_v1())
                    .row_v1(row)
                    .unwrap();
                for offset in 0..count {
                    assert_eq!(columns[offset][row], expected[first + offset]);
                }
            }
            for selected in [0, shape.fixed_width / 2, shape.fixed_width - 1] {
                if (first..first + count).contains(&selected) {
                    let mut scalar = vec![F::ZERO; shape.trace_size];
                    fixed
                        .fill_fixed_column_v1(registration, selected, &mut scalar)
                        .unwrap();
                    assert_eq!(columns[selected - first], scalar);
                    crate::privacy_engines::transparent_stark::goldilocks_ifft_v1(
                        &mut scalar,
                        root,
                    )
                    .unwrap();
                    crate::privacy_engines::transparent_stark::goldilocks_ifft_v1(
                        &mut columns[selected - first],
                        root,
                    )
                    .unwrap();
                    assert_eq!(columns[selected - first], scalar);
                }
            }
        }
        // Fixed public geometry only. Both durations exclude allocation,
        // equality checks and inverse transforms; no timing gate is asserted.
        eprintln!(
            "public_fixed_matrix_cost signature={index} rows={} columns={} matrix_row_passes=1 batch_row_passes={} matrix_fill_seconds={:.6} batch_fill_seconds={:.6}",
            shape.trace_size,
            shape.fixed_width,
            shape.fixed_width.div_ceil(8),
            matrix_elapsed.as_secs_f64(),
            batch_elapsed.as_secs_f64(),
        );
    }
    assert_eq!(5 * 134 * (1 << 19), 351_272_960);
    assert_eq!(5 * 134_usize.div_ceil(8) * (1 << 19), 44_564_480);
    assert_eq!(5 * (1 << 19), 2_621_440);
}
