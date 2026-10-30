// Exact compact base projection, public-shape refusal and partial-output erasure.

#[test]
fn compact_sha_base_projection_matches_full_rows_across_word_and_length_boundary() {
    let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 4,
    })
    .unwrap();
    let witnesses = witnesses(&schedule);
    for segment in [0, 3] {
        let source =
            ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, segment).unwrap();
        let first = SHA_WORD_CAPACITY_BASE_WIDTH_V1 - 3;
        let mut expected = vec![native_column_v1(F::ZERO); 8];
        source
            .for_each_base_fixed_row_v1(|row, base, _| {
                for (offset, column) in expected.iter_mut().enumerate() {
                    column[row] = base[first + offset];
                }
            })
            .unwrap();
        let mut actual = vec![native_column_v1(F(91)); 8];
        source
            .fill_base_columns_v1(
                segment,
                first,
                &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>(),
            )
            .unwrap();
        assert_eq!(actual, expected);
        for column in &actual {
            assert!(
                column[source.replay.active_rows()..]
                    .iter()
                    .all(|value| *value == F::ZERO)
            );
        }
    }
}

#[test]
fn compact_sha_base_projection_checks_length_and_clears_failed_or_unwound_batches() {
    let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 4,
    })
    .unwrap();
    let witnesses = witnesses(&schedule);
    let source = ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, 0).unwrap();
    let consumer = sha_rfc_consumer_channels_v1(
        schedule.call(0).unwrap().call,
        schedule.call(0).unwrap().role,
        4,
    )
    .unwrap()
    .unwrap();
    for length in [0, consumer.message_capacity_bytes] {
        assert_eq!(
            sha_batch_raw_length_v1(consumer.message_prefix_bytes + length, Some(consumer)),
            Ok(Some(length))
        );
    }
    assert!(
        sha_batch_raw_length_v1(
            consumer.message_prefix_bytes + consumer.message_capacity_bytes + 1,
            Some(consumer)
        )
        .is_err()
    );
    let mut prefixed = consumer;
    prefixed.message_prefix_bytes = 1;
    assert!(sha_batch_raw_length_v1(0, Some(prefixed)).is_err());
    assert_eq!(sha_batch_raw_length_v1(usize::MAX, None), Ok(None));
    for failure in 0..3 {
        let mut actual = vec![native_column_v1(F(91)); 8];
        let mut calls = 0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            source.fill_base_columns_using_v1(
                0,
                0,
                &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>(),
                |manifest, witness| {
                    calls += 1;
                    let mut call =
                        build_zk_x509_sha_batch_call_base_source_v1(manifest, witness, 4)?;
                    if calls == 2 {
                        match failure {
                            0 => call.manifest.first_logical_row += 1,
                            1 => {
                                let mut invalid = consumer;
                                invalid.message_prefix_bytes = usize::MAX;
                                call.rfc_consumer = Some(invalid);
                            }
                            _ => panic!("injected construction unwind"),
                        }
                    }
                    Ok(call)
                },
            )
        }));
        assert_eq!(calls, 2);
        if failure == 2 {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert!(actual.iter().flatten().all(|value| *value == F::ZERO));
    }
}

#[test]
#[ignore = "release-only comparison of original full-row replay and compact base projection"]
fn compact_sha_base_projection_cost_diagnostic() {
    assert!(!cfg!(debug_assertions), "use an optimized build");
    let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 4,
    })
    .unwrap();
    let witnesses = witnesses(&schedule);
    let fixed = ZkX509ShaBatchFixedProviderV1::new_v1(schedule.shape()).unwrap();
    for segment in [0, 3] {
        let source =
            ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, segment).unwrap();
        for first in [
            0,
            SHA_WORD_CAPACITY_BASE_WIDTH_V1 - 3,
            ZK_X509_SHA_BATCH_BASE_WIDTH_V1 - 8,
        ] {
            let mut columns = [
                vec![native_column_v1(F::ZERO); 8],
                vec![native_column_v1(F::ZERO); 8],
            ];
            let mut elapsed = [std::time::Duration::ZERO; 2];
            for round in 0..2 {
                for mode in [round, 1 - round] {
                    let start = std::time::Instant::now();
                    if mode == 0 {
                        let mut targets = columns[mode]
                            .iter_mut()
                            .map(Vec::as_mut_slice)
                            .collect::<Vec<_>>();
                        let mut fills = sha_column_fill_batch_v1(&mut targets).unwrap();
                        source
                            .for_each_base_fixed_row_using_v1(
                                |row, base, _| {
                                    for (offset, fill) in fills.iter_mut().enumerate() {
                                        fill.write_v1(row, base[first + offset]);
                                    }
                                },
                                |manifest, witness| fixed.call_base_source_v1(manifest, witness),
                            )
                            .unwrap();
                        finish_sha_column_fill_batch_v1(fills).unwrap();
                    } else {
                        source
                            .fill_base_columns_using_v1(
                                segment,
                                first,
                                &mut columns[mode]
                                    .iter_mut()
                                    .map(Vec::as_mut_slice)
                                    .collect::<Vec<_>>(),
                                |manifest, witness| fixed.call_base_source_v1(manifest, witness),
                            )
                            .unwrap();
                    }
                    elapsed[mode] += start.elapsed();
                }
                assert_eq!(columns[0], columns[1]);
            }
            println!(
                "X509_SHA_BASE_PROJECTION segment={segment} first={first} original_s={:.6} compact_s={:.6} speedup={:.3}",
                elapsed[0].as_secs_f64(),
                elapsed[1].as_secs_f64(),
                elapsed[0].as_secs_f64() / elapsed[1].as_secs_f64()
            );
        }
    }
}
