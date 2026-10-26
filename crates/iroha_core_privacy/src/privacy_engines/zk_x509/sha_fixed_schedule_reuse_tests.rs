// Bound replay borrows the existing public provider; every private row stays in
// its original clearing owner and every original constraint remains unchanged.

#[test]
#[ignore = "all 89 base columns and all physical rows for ordinary/maximum disclosure shapes; run explicitly with one test thread"]
fn borrowed_fixed_bound_replay_matches_all_columns_and_counts_actual_construction() {
    use super::super::sha256_word_air::observe_circuit_builds_v1;
    for disclosed_attributes in [1, 4] {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes,
        })
        .unwrap();
        let witnesses = witnesses(&schedule);
        let fixed = ZkX509ShaBatchFixedProviderV1::new_v1(schedule.shape()).unwrap();
        let payload = fixed.allocated_heap_bytes_v1();
        for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
            let mut source =
                ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, segment).unwrap();
            let mut expected = PrivateTableV1::new(
                (0..ZK_X509_SHA_BATCH_BASE_WIDTH_V1)
                    .map(|_| native_column_v1(F::ZERO))
                    .collect::<Vec<_>>(),
                zeroize_field_rows_v1,
            );
            let mut rows = 0;
            let (result, fresh_builds) = observe_circuit_builds_v1(|| {
                source.for_each_base_fixed_row_v1(|index, base, _| {
                    assert_eq!(index, rows);
                    for (column, value) in expected.iter_mut().zip(base) {
                        column[index] = value;
                    }
                    rows += 1;
                })
            });
            result.unwrap();
            assert_eq!(rows, ZK_X509_SHA_SEGMENT_ROWS_V1);
            assert_eq!(
                fresh_builds,
                4 * ZK_X509_SHA_PHYSICAL_CALL_COUNTS_V1[segment]
            );
            let bound = source.bind_v1(&credential_binding(0x63)).unwrap();
            for first in (0..ZK_X509_SHA_BATCH_BASE_WIDTH_V1).step_by(8) {
                let count = (ZK_X509_SHA_BATCH_BASE_WIDTH_V1 - first).min(8);
                let mut actual = PrivateTableV1::new(
                    (0..count)
                        .map(|_| native_column_v1(F(91)))
                        .collect::<Vec<_>>(),
                    zeroize_field_rows_v1,
                );
                let (result, borrowed_builds) = observe_circuit_builds_v1(|| {
                    bound.replay_base_columns_v1(
                        &fixed,
                        segment,
                        first,
                        &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>(),
                    )
                });
                result.unwrap();
                assert_eq!(
                    borrowed_builds,
                    2 * ZK_X509_SHA_PHYSICAL_CALL_COUNTS_V1[segment]
                );
                for (offset, column) in actual.iter().enumerate() {
                    assert!(
                        column == &expected[first + offset],
                        "every row of every original column"
                    );
                }
            }
            assert_eq!(fixed.allocated_heap_bytes_v1(), payload);
        }
    }
}

#[test]
fn borrowed_fixed_provider_rejects_foreign_shape_manifest_and_invalid_witness() {
    use super::super::sha256_word_air::observe_circuit_builds_v1;
    let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 1,
    })
    .unwrap();
    let witnesses = witnesses(&schedule);
    let mut fixed = ZkX509ShaBatchFixedProviderV1::new_v1(schedule.shape()).unwrap();
    let mut source = ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, 0).unwrap();
    let mut bound = source.bind_v1(&credential_binding(0x64)).unwrap();
    let mut output = native_column_v1(F(91));
    fixed.schedule.shape.disclosed_attributes = 4;
    let (result, builds) =
        observe_circuit_builds_v1(|| bound.replay_base_column_v1(&fixed, 0, 0, &mut output));
    assert!(matches!(
        result,
        Err(ZkX509ShaCallBusStarkErrorV1::Topology)
    ));
    assert_eq!(builds, 0);
    assert!(output.iter().all(|value| *value == F(91)));
    fixed.schedule.shape.disclosed_attributes = 1;
    let manifest = schedule.call(ZK_X509_SHA_CA_LEAF_CALL_V1).unwrap();
    let mut changed = manifest;
    changed.first_logical_row += 1;
    let (result, builds) = observe_circuit_builds_v1(|| {
        fixed.call_base_source_v1(changed, &witnesses[usize::from(manifest.call)])
    });
    assert!(result.is_err());
    assert_eq!(builds, 0);
    let mut changed_witness = witnesses[usize::from(manifest.call)].clone();
    changed_witness.digest[0] ^= 1;
    let (result, builds) =
        observe_circuit_builds_v1(|| fixed.call_base_source_v1(manifest, &changed_witness));
    assert!(result.is_err());
    assert_eq!(builds, 0);
    bound.zeroize_private_v1();
    assert!(matches!(
        bound.replay_base_column_v1(&fixed, 0, 0, &mut output),
        Err(ZkX509ShaCallBusStarkErrorV1::Phase)
    ));
    assert!(output.iter().all(|value| *value == F(91)));
}

#[test]
fn borrowed_fixed_bound_replay_clears_partial_destinations_on_error_and_unwind() {
    let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 1,
    })
    .unwrap();
    let witnesses = witnesses(&schedule);
    let fixed = ZkX509ShaBatchFixedProviderV1::new_v1(schedule.shape()).unwrap();
    let mut changed = witnesses.clone();
    let second_call = usize::from(ZK_X509_SHA_PHYSICAL_CALL_ORDER_V1[1]);
    changed[second_call].digest[0] ^= 1;
    let mut source = ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, 0).unwrap();
    let mut bound = source.bind_v1(&credential_binding(0x65)).unwrap();
    bound.witnesses = &changed;
    let mut output = native_column_v1(F(91));
    assert!(
        bound
            .replay_base_column_v1(&fixed, 0, 0, &mut output)
            .is_err()
    );
    assert!(output.iter().all(|value| *value == F::ZERO));
    let source = ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, 0).unwrap();
    output.fill(F(91));
    let mut calls = 0;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        source.fill_base_columns_using_v1(0, 0, &mut [&mut output], |manifest, witness| {
            calls += 1;
            assert!(calls != 2, "injected unwind after the first complete call");
            fixed.call_base_source_v1(manifest, witness)
        })
    }));
    assert!(result.is_err());
    assert_eq!(calls, 2);
    assert!(output.iter().all(|value| *value == F::ZERO));
}
