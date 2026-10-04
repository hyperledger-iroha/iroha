//! Exact transpose, resource, failure and clearing controls for native DER batches.

use super::*;

fn challenges() -> ZkX509DerStarkChallengesV1 {
    ZkX509DerStarkChallengesV1 {
        tuple: core::array::from_fn(|lane| {
            core::array::from_fn(|column| F(u64::try_from(1_000 + lane * 100 + column).unwrap()))
        }),
        byte_lookup: [F(9_001), F(9_002), F(9_003), F(9_004)],
    }
}

fn trace() -> ZkX509DerStarkTraceV1 {
    // Actual DER parser/auxiliary builders; this small relation fixture does
    // not represent a signed credential or source/finality authority.
    let ordered_set = [0x31, 0x04, 0x05, 0x00, 0x05, 0x00];
    let base = build_zk_x509_der_stark_base_v1(&[&ordered_set]).unwrap();
    build_zk_x509_der_stark_trace_v1(base, challenges()).unwrap()
}

#[test]
fn bounded_projection_calls_each_row_once_and_preserves_every_selected_cell() {
    const ROWS: usize = 11;
    const WIDTH: usize = 19;
    for count in [1, 3, MASKED_TRACE_LDE_COLUMN_BATCH_V1] {
        for first in [0, WIDTH - count] {
            let mut columns = vec![vec![F::ONE; ROWS]; count];
            let mut targets = columns
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>();
            let mut visited = Vec::new();
            project_rows_v1(ROWS, first, &mut targets, |row| {
                visited.push(row);
                Ok(core::array::from_fn::<_, WIDTH, _>(|column| {
                    F(u64::try_from(row * WIDTH + column).unwrap())
                }))
            })
            .unwrap();
            assert_eq!(visited, (0..ROWS).collect::<Vec<_>>());
            for (offset, column) in columns.iter().enumerate() {
                for (row, value) in column.iter().enumerate() {
                    assert_eq!(
                        *value,
                        F(u64::try_from(row * WIDTH + first + offset).unwrap())
                    );
                }
            }
        }
    }
}

#[test]
fn malformed_public_extents_refuse_before_access_or_mutation() {
    for (rows, first, count, length) in [
        (0, 0, 1, 0),
        (4, 0, 0, 4),
        (4, 0, MASKED_TRACE_LDE_COLUMN_BATCH_V1 + 1, 4),
        (4, 8, 1, 4),
        (4, 7, 2, 4),
        (4, usize::MAX, 1, 4),
        (4, 0, 1, 3),
        (4, 0, 1, 5),
    ] {
        let mut columns = vec![vec![F(17); length]; count];
        let original = columns.clone();
        let mut targets = columns
            .iter_mut()
            .map(Vec::as_mut_slice)
            .collect::<Vec<_>>();
        let mut calls = 0;
        assert_eq!(
            project_rows_v1(rows, first, &mut targets, |_| {
                calls += 1;
                Ok([F::ZERO; 8])
            }),
            Err(Error::Resource)
        );
        assert_eq!(calls, 0);
        assert_eq!(columns, original);
    }
    let mut short = [F(19); 3];
    let mut full = [F(19); 4];
    assert_eq!(
        project_rows_v1::<8>(4, 0, &mut [&mut full, &mut short], |_| {
            panic!("mixed-length targets must fail before source access")
        }),
        Err(Error::Resource)
    );
    assert_eq!(full, [F(19); 4]);
    assert_eq!(short, [F(19); 3]);
}

#[test]
fn late_source_error_and_unwind_clear_the_complete_batch() {
    for panic_at_end in [false, true] {
        let mut columns = [[F(23); 7]; 3];
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut targets = columns
                .iter_mut()
                .map(|column| &mut column[..])
                .collect::<Vec<_>>();
            project_rows_v1(7, 1, &mut targets, |row| {
                if row == 6 {
                    assert!(!panic_at_end, "injected row-source unwind");
                    return Err(Error::Row);
                }
                Ok([F(u64::try_from(row + 1).unwrap()); 5])
            })
        }));
        if panic_at_end {
            assert!(outcome.is_err());
        } else {
            assert_eq!(outcome.unwrap(), Err(Error::Row));
        }
        assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
    }
}

#[test]
fn original_der_row_failures_are_preserved_and_clear_prior_writes() {
    let mut trace = trace();
    let rows = trace.base.private_shape.parser_rows;
    let mut columns = [vec![F(29); rows], vec![F(29); rows]];
    zeroize_field_rows_v1(&mut trace.base.rows);
    trace.base.rows.clear();
    let mut targets = columns
        .iter_mut()
        .map(Vec::as_mut_slice)
        .collect::<Vec<_>>();
    assert_eq!(
        project_rows_v1(rows, 0, &mut targets, |index| {
            zk_x509_der_stark_aggregate_base_row_v1(&trace.base, index)
        }),
        Err(Error::Row)
    );
    assert!(columns.iter().flatten().all(|value| *value == F::ZERO));

    let mut trace = self::trace();
    let rows = trace.base.private_shape.parser_rows + 1;
    trace.base.private_shape.document_lengths.clear();
    let mut columns = [vec![F(31); rows], vec![F(31); rows]];
    let mut targets = columns
        .iter_mut()
        .map(Vec::as_mut_slice)
        .collect::<Vec<_>>();
    assert_eq!(
        project_rows_v1(rows, 0, &mut targets, |index| {
            zk_x509_der_stark_aggregate_aux_row_v1(&trace, index)
        }),
        Err(Error::Shape)
    );
    assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
}

#[test]
fn projection_keeps_canonicality_admission_at_the_existing_caller_boundary() {
    let invalid = F(GOLDILOCKS_MODULUS_V1);
    let mut target = [F::ZERO; 2];
    project_rows_v1(2, 1, &mut [&mut target], |_| Ok([F::ONE, invalid])).unwrap();
    assert_eq!(target, [invalid; 2]);
    assert!(target.iter().all(|value| F::canonical(value.0).is_none()));
}

#[test]
#[ignore = "complete original DER native-column oracle parity; run optimized"]
fn full_native_batches_match_original_columns_in_every_region_and_terminal_lane() {
    let trace = trace();
    assert!(trace.base.private_shape.parser_rows > 0);
    assert!(trace.base.private_shape.comparator_rows > 0);
    for is_aux in [false, true] {
        let width = if is_aux {
            ZK_X509_DER_STARK_AUX_WIDTH_V1
        } else {
            ZK_X509_DER_STARK_BASE_WIDTH_V1
        };
        for first in (0..width).step_by(MASKED_TRACE_LDE_COLUMN_BATCH_V1) {
            let count = (width - first).min(MASKED_TRACE_LDE_COLUMN_BATCH_V1);
            let mut columns = PrivateTableV1::new(
                vec![vec![F::ZERO; ZK_X509_DER_STARK_TRACE_SIZE_V1]; count],
                zeroize_field_rows_v1::<Vec<F>>,
            );
            let mut targets = columns
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>();
            if is_aux {
                fill_zk_x509_der_stark_native_aux_columns_v1(&trace, first, &mut targets).unwrap();
            } else {
                fill_zk_x509_der_stark_native_base_columns_v1(&trace.base, first, &mut targets)
                    .unwrap();
            }
            drop(targets);
            for (offset, column) in columns.iter().enumerate() {
                let original = PrivateTableV1::new(
                    if is_aux {
                        build_zk_x509_der_stark_native_aux_column_v1(&trace, first + offset)
                            .unwrap()
                    } else {
                        build_zk_x509_der_stark_native_base_column_v1(&trace.base, first + offset)
                            .unwrap()
                    },
                    super::super::super::private_table::zeroize_fields_v1,
                );
                assert_eq!(column.as_slice(), &original[..]);
            }
        }
    }
}
