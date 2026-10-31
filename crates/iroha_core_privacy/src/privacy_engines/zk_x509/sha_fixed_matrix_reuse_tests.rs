// Public fixed matrix construction preserves all columns and physical padding.

#[test]
fn public_fixed_matrix_matches_column_replay_and_builds_each_row_once() {
    const ROWS: usize = 32;
    use super::super::p256_aggregate_adapter::P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1;
    assert!(
        2 * core::mem::size_of::<[F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1]>()
            + core::mem::size_of::<ShaPublicFixedMatrixDestinationV1<'_>>()
            + 8 * core::mem::size_of::<usize>()
            <= 2 * ZK_X509_SHA_BATCH_FIXED_WIDTH_V1.max(P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1)
                * core::mem::size_of::<F>()
                + core::mem::size_of::<[usize; 16]>()
    );
    let mut reference_calls = 0;
    let mut expected = vec![vec![F::ZERO; ROWS]; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1];
    let make_row = |row: usize| {
        core::array::from_fn::<F, ZK_X509_SHA_BATCH_FIXED_WIDTH_V1, _>(|column| {
            F((row * 173 + column * 31 + 1) as u64)
        })
    };
    // Faithful old transposition: reconstruct the entire row for each column.
    for (column, output) in expected.iter_mut().enumerate() {
        for (row, value) in output.iter_mut().enumerate() {
            reference_calls += 1;
            *value = make_row(row)[column];
        }
    }
    let mut actual = vec![vec![F(91); ROWS]; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1];
    let capacities = actual.iter().map(Vec::capacity).collect::<Vec<_>>();
    let pointers = actual.iter().map(Vec::as_ptr).collect::<Vec<_>>();
    let mut actual_calls = 0;
    fill_sha_public_fixed_matrix_with_v1(ROWS, &mut actual, |row| {
        assert_eq!(row, actual_calls);
        actual_calls += 1;
        Ok(make_row(row))
    })
    .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual_calls, ROWS);
    assert_eq!(reference_calls, ROWS * ZK_X509_SHA_BATCH_FIXED_WIDTH_V1);
    assert_eq!(
        actual.iter().map(Vec::capacity).collect::<Vec<_>>(),
        capacities
    );
    assert_eq!(actual.iter().map(Vec::as_ptr).collect::<Vec<_>>(), pointers);
}

#[test]
fn public_fixed_matrix_refuses_geometry_before_writing_and_clears_late_failures() {
    const ROWS: usize = 8;
    for (rows, width, length) in [
        (0, ZK_X509_SHA_BATCH_FIXED_WIDTH_V1, 0),
        (3, ZK_X509_SHA_BATCH_FIXED_WIDTH_V1, 3),
        (ZK_X509_SHA_SEGMENT_ROWS_V1 + 1, 0, 0),
        (ROWS, ZK_X509_SHA_BATCH_FIXED_WIDTH_V1 - 1, ROWS),
        (ROWS, ZK_X509_SHA_BATCH_FIXED_WIDTH_V1, ROWS - 1),
    ] {
        let mut columns = vec![vec![F(73); length]; width];
        let before = columns.clone();
        assert!(
            fill_sha_public_fixed_matrix_with_v1(rows, &mut columns, |_| {
                panic!("invalid output shape reached public row reconstruction")
            })
            .is_err()
        );
        assert_eq!(columns, before);
    }
    let provider = ZkX509ShaBatchFixedProviderV1::new_v1(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 1,
    })
    .unwrap();
    let mut short = vec![vec![F(73); ROWS]; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1];
    let before = short.clone();
    assert!(provider.fill_fixed_matrix_v1(0, &mut short).is_err());
    assert!(
        provider
            .fill_fixed_matrix_v1(ZK_X509_SHA_SEGMENT_COUNT_V1, &mut short)
            .is_err()
    );
    assert_eq!(short, before);
    for unwind in [false, true] {
        let mut columns = vec![vec![F(73); ROWS]; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            fill_sha_public_fixed_matrix_with_v1(ROWS, &mut columns, |row| {
                if row == ROWS - 1 {
                    if unwind {
                        panic!("synthetic late fixed-row interruption");
                    }
                    return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
                }
                Ok([F((row + 1) as u64); ZK_X509_SHA_BATCH_FIXED_WIDTH_V1])
            })
        }));
        assert!(if unwind {
            result.is_err()
        } else {
            result.unwrap().is_err()
        });
        assert!(columns.iter().flatten().all(|value| *value == F::ZERO));
    }
}

#[test]
#[ignore = "all 137 fixed columns/all 524288 rows of all 4 segments and all 5 disclosure shapes; about 548 MiB public matrix; run explicitly"]
fn public_fixed_matrix_preserves_every_canonical_cell_for_all_supported_shapes() {
    assert_eq!(ZK_X509_SHA_BATCH_FIXED_WIDTH_V1, 137);
    for disclosed_attributes in 0..=4 {
        let provider = ZkX509ShaBatchFixedProviderV1::new_v1(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes,
        })
        .unwrap();
        let payload = provider.allocated_heap_bytes_v1();
        for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
            let mut columns = PrivateTableV1::new(
                (0..ZK_X509_SHA_BATCH_FIXED_WIDTH_V1)
                    .map(|_| vec![F::ZERO; ZK_X509_SHA_SEGMENT_ROWS_V1])
                    .collect::<Vec<_>>(),
                zeroize_field_rows_v1,
            );
            let capacities = columns.iter().map(Vec::capacity).collect::<Vec<_>>();
            provider
                .fill_fixed_matrix_v1(segment, &mut columns)
                .unwrap();
            // Original canonical row function is independent of the new transpose;
            // compare every field, active boundary, inactive call and padding row.
            for row in 0..ZK_X509_SHA_SEGMENT_ROWS_V1 {
                let expected = provider.fixed_row_v1(segment, row).unwrap();
                for (column, value) in expected.iter().enumerate() {
                    assert_eq!(
                        columns[column][row], *value,
                        "shape={disclosed_attributes} segment={segment} row={row} column={column}"
                    );
                }
            }
            assert_eq!(
                columns.iter().map(Vec::capacity).collect::<Vec<_>>(),
                capacities
            );
            assert_eq!(provider.allocated_heap_bytes_v1(), payload);
        }
    }
}
