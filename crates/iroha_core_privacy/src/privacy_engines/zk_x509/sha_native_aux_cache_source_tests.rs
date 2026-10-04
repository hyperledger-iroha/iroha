// Genuine source recurrence parity; these controls allocate one native cache at a time.

#[test]
#[ignore = "full native SHA auxiliary cache parity across four physical segments"]
fn retained_native_auxiliary_rows_match_original_recurrence_and_bound_terminals() {
    for disclosed_attributes in [2, 4] {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes,
        })
        .unwrap();
        let witnesses = witnesses(&schedule);
        let binding = credential_binding(u8::try_from(disclosed_attributes).unwrap());
        for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
            let mut base =
                ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, segment).unwrap();
            let mut source = base.bind_v1(binding).unwrap();
            assert_eq!(source.retained_aux_heap_bytes_v1(), 0);
            source.retain_native_aux_v1().unwrap();
            assert!(source.retain_native_aux_v1().is_err());
            let forecast =
                ZkX509ShaBatchSegmentAuxSourceV1::native_aux_cache_forecast_all_v1().unwrap() / 4;
            assert!(source.retained_aux_heap_bytes_v1() <= forecast);
            let cache = source.retained_aux.as_ref().unwrap();
            let mut compared = 0;
            let original = source
                .replay_aux_rows_uncached_v1(|index, row| {
                    assert_eq!(cache.rows_v1()[index], row);
                    compared += 1;
                })
                .unwrap();
            assert_eq!(compared, ZK_X509_SHA_SEGMENT_ROWS_V1);
            assert_eq!(cache.copy_terminals_v1().unwrap(), original);
            assert_eq!(
                source.rfc_union_air_terminals_v1(binding, segment).unwrap(),
                original
            );
            assert!(
                source
                    .rfc_union_air_terminals_v1(credential_binding(91), segment)
                    .is_err()
            );
            assert!(
                source
                    .rfc_union_air_terminals_v1(binding, (segment + 1) % 4)
                    .is_err()
            );
            for first in (0..78).step_by(8) {
                let count = (78 - first).min(8);
                let mut columns = PrivateTableV1::new(
                    (0..count)
                        .map(|_| native_column_v1(F(99)))
                        .collect::<Vec<_>>(),
                    zeroize_field_rows_v1,
                );
                let mut targets = columns
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>();
                assert_eq!(
                    source
                        .fill_aux_columns_with_air_terminals_v1(segment, first, &mut targets)
                        .unwrap(),
                    original
                );
                for (offset, column) in columns.iter().enumerate() {
                    for (index, value) in column.iter().enumerate() {
                        assert_eq!(*value, cache.rows_v1()[index][first + offset]);
                    }
                }
            }
            let mut untouched = native_column_v1(F(99));
            assert!(
                source
                    .fill_aux_column_v1((segment + 1) % 4, 0, &mut untouched)
                    .is_err()
            );
            assert!(untouched.iter().all(|value| *value == F(99)));
            let end = source
                .for_each_aux_row_with_air_terminals_v1(|_, _| {})
                .unwrap();
            assert_eq!(end, original);
            assert!(
                source
                    .for_each_aux_row_with_air_terminals_v1(|_, _| {})
                    .is_err()
            );
            source.zeroize_private_v1();
            assert!(source.private_is_zeroized_v1());
            assert_eq!(source.retained_aux_heap_bytes_v1(), 0);
            assert!(source.retain_native_aux_v1().is_err());
            assert!(
                source
                    .fill_aux_column_v1(segment, 0, &mut untouched)
                    .is_err()
            );
            assert!(untouched.iter().all(|value| *value == F(99)));
        }
    }
}
