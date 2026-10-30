// Original full-row replay remains the independent reference for every packed cell.

#[test]
fn retained_native_base_phase_transfer_is_unique_and_clears_on_release() {
    use super::super::private_table::inspection;
    let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 2,
    })
    .unwrap();
    let witnesses = witnesses(&schedule);
    let mut base = ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, 0).unwrap();
    assert_eq!(base.retained_base_heap_bytes_v1(), 0);
    // A tiny internally constructed cache exercises the ownership transition;
    // production can only install a complete validated segment through retain.
    base.retained_base = Some(
        native_base_cache::ShaNativeBaseCacheV1::build_v1(1, |visitor| {
            visitor(0, &[F::ONE; ZK_X509_SHA_BATCH_BASE_WIDTH_V1])
        })
        .unwrap(),
    );
    let identity = base
        .retained_base
        .as_ref()
        .unwrap()
        .allocation_identity_v1();
    assert_eq!(base.retained_base_heap_bytes_v1(), 64);
    assert!(base.retain_native_base_v1().is_err());
    let mut invalid = word_challenges();
    invalid.memory.lanes[0].beta = F(u64::MAX);
    assert!(
        base.validate_bind_for_test_v1(invalid, challenges(), &rfc_challenges())
            .is_err()
    );
    assert!(!base.is_bound_for_test_v1());
    assert_eq!(
        base.retained_base
            .as_ref()
            .unwrap()
            .allocation_identity_v1(),
        identity
    );
    let mut bound = base.bind_v1(&credential_binding(0x72)).unwrap();
    assert!(base.retained_base.is_none());
    assert_eq!(
        bound
            .retained_base
            .as_ref()
            .unwrap()
            .allocation_identity_v1(),
        identity
    );
    assert_eq!(bound.retained_base_heap_bytes_v1(), 64);
    assert!(base.retain_native_base_v1().is_err());
    assert!(base.bind_v1(&credential_binding(0x72)).is_err());
    let (_, observations) = inspection::observe_v1(|| bound.zeroize_private_v1());
    assert!(bound.private_is_zeroized_v1());
    assert_eq!(bound.retained_base_heap_bytes_v1(), 0);
    assert_eq!(
        observations.iter().map(|entry| entry.cells).sum::<usize>(),
        14
    );
    assert!(observations.iter().any(|entry| entry.nonzero_before > 0));
    assert!(observations.iter().all(|entry| entry.nonzero_after == 0));
}

#[test]
#[ignore = "full original SHA base parity: all 89 columns, four segments, ordinary and maximum disclosure; run with one test thread"]
fn retained_native_base_matches_original_rows_all_columns_dummy_padding_and_bound_replay() {
    use super::super::sha256_word_air::observe_circuit_builds_v1;
    for disclosed_attributes in [2, 4] {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes,
        })
        .unwrap();
        let witnesses = witnesses(&schedule);
        let fixed = ZkX509ShaBatchFixedProviderV1::new_v1(schedule.shape()).unwrap();
        for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
            let mut base =
                ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, segment).unwrap();
            let mut original = PrivateTableV1::new(
                (0..ZK_X509_SHA_BATCH_BASE_WIDTH_V1)
                    .map(|_| native_column_v1(F::ZERO))
                    .collect::<Vec<_>>(),
                zeroize_field_rows_v1,
            );
            let mut rows = 0;
            base.for_each_base_fixed_row_v1(|index, values, _| {
                let values = zeroize::Zeroizing::new(values);
                assert_eq!(index, rows);
                for (column, value) in original.iter_mut().zip(values.iter()) {
                    column[index] = *value;
                }
                rows += 1;
            })
            .unwrap();
            assert_eq!(rows, ZK_X509_SHA_SEGMENT_ROWS_V1);
            base.retain_native_base_v1().unwrap();
            assert_eq!(base.retained_base_heap_bytes_v1(), 33_554_432);
            let cache = base.retained_base.as_ref().unwrap();
            let identity = cache.allocation_identity_v1();
            for index in 0..ZK_X509_SHA_SEGMENT_ROWS_V1 {
                let row = cache.row_v1(index);
                for (column, value) in original.iter().zip(row.iter()) {
                    assert_eq!(column[index], *value);
                }
            }
            for first in (0..ZK_X509_SHA_BATCH_BASE_WIDTH_V1).step_by(8) {
                let count = (ZK_X509_SHA_BATCH_BASE_WIDTH_V1 - first).min(8);
                let mut actual = PrivateTableV1::new(
                    (0..count)
                        .map(|_| native_column_v1(F(91)))
                        .collect::<Vec<_>>(),
                    zeroize_field_rows_v1,
                );
                let (result, builds) = observe_circuit_builds_v1(|| {
                    base.fill_base_columns_v1(
                        segment,
                        first,
                        &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>(),
                    )
                });
                result.unwrap();
                assert_eq!(builds, 0);
                for (offset, column) in actual.iter().enumerate() {
                    assert_eq!(column, &original[first + offset]);
                }
            }
            let mut bound = base.bind_v1(&credential_binding(0x73)).unwrap();
            assert_eq!(
                bound
                    .retained_base
                    .as_ref()
                    .unwrap()
                    .allocation_identity_v1(),
                identity
            );
            assert_eq!(base.retained_base_heap_bytes_v1(), 0);
            let mut untouched = native_column_v1(F(99));
            assert!(
                base.fill_base_column_v1(segment, 0, &mut untouched)
                    .is_err()
            );
            assert!(
                bound
                    .replay_base_column_v1(&fixed, (segment + 1) % 4, 0, &mut untouched)
                    .is_err()
            );
            assert!(
                bound
                    .replay_base_column_v1(&fixed, segment, 89, &mut untouched)
                    .is_err()
            );
            assert!(untouched.iter().all(|field| *field == F(99)));
            for first in (0..ZK_X509_SHA_BATCH_BASE_WIDTH_V1).step_by(8) {
                let count = (ZK_X509_SHA_BATCH_BASE_WIDTH_V1 - first).min(8);
                let mut actual = PrivateTableV1::new(
                    (0..count)
                        .map(|_| native_column_v1(F(91)))
                        .collect::<Vec<_>>(),
                    zeroize_field_rows_v1,
                );
                let (result, builds) = observe_circuit_builds_v1(|| {
                    bound.replay_base_columns_v1(
                        &fixed,
                        segment,
                        first,
                        &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>(),
                    )
                });
                result.unwrap();
                assert_eq!(builds, 0);
                for (offset, column) in actual.iter().enumerate() {
                    assert_eq!(column, &original[first + offset]);
                }
            }
            // Original auxiliary/terminal recurrence still validates and uses the
            // original witness and binding; the base cache cannot supply endpoints.
            assert!(
                bound
                    .rfc_union_air_terminals_v1(&credential_binding(0x74), segment)
                    .is_err()
            );
            bound.zeroize_private_v1();
            assert!(
                bound
                    .replay_base_column_v1(&fixed, segment, 0, &mut untouched)
                    .is_err()
            );
            assert!(untouched.iter().all(|field| *field == F(99)));
        }
    }
}

#[test]
#[ignore = "full native SHA source cost, 3 repetitions of all 89 columns and four segments; run without competing work"]
fn retained_native_base_full_batch_cost_and_identical_original_outputs() {
    let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 4,
    })
    .unwrap();
    let witnesses = witnesses(&schedule);
    let mut original_seconds = [0.0_f64; 3];
    let mut cached_seconds = [0.0_f64; 3];
    let mut build_seconds = 0.0;
    for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
        let original =
            ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, segment).unwrap();
        let mut cached =
            ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, segment).unwrap();
        let start = std::time::Instant::now();
        cached.retain_native_base_v1().unwrap();
        build_seconds += start.elapsed().as_secs_f64();
        for repetition in 0..3 {
            for first in (0..ZK_X509_SHA_BATCH_BASE_WIDTH_V1).step_by(8) {
                let count = (ZK_X509_SHA_BATCH_BASE_WIDTH_V1 - first).min(8);
                let mut expected = PrivateTableV1::new(
                    (0..count)
                        .map(|_| native_column_v1(F::ZERO))
                        .collect::<Vec<_>>(),
                    zeroize_field_rows_v1,
                );
                let mut actual = PrivateTableV1::new(
                    (0..count)
                        .map(|_| native_column_v1(F::ZERO))
                        .collect::<Vec<_>>(),
                    zeroize_field_rows_v1,
                );
                let mut run = |is_cached: bool| {
                    let (source, columns) = if is_cached {
                        (&cached, &mut actual)
                    } else {
                        (&original, &mut expected)
                    };
                    let mut targets = columns
                        .iter_mut()
                        .map(Vec::as_mut_slice)
                        .collect::<Vec<_>>();
                    let start = std::time::Instant::now();
                    source
                        .fill_base_columns_v1(segment, first, &mut targets)
                        .unwrap();
                    let elapsed = start.elapsed().as_secs_f64();
                    if is_cached {
                        cached_seconds[repetition] += elapsed;
                    } else {
                        original_seconds[repetition] += elapsed;
                    }
                };
                run(repetition % 2 == 0);
                run(repetition % 2 != 0);
                assert_eq!(&*actual, &*expected);
            }
        }
    }
    original_seconds.sort_by(f64::total_cmp);
    cached_seconds.sort_by(f64::total_cmp);
    eprintln!(
        "SHA retained base: original_median_s={:.6}, compact_median_s={:.6}, build_once_s={build_seconds:.6}, speedup={:.3}, heap_bytes=134217728, all_outputs_identical=true",
        original_seconds[1],
        cached_seconds[1],
        original_seconds[1] / cached_seconds[1]
    );
}
