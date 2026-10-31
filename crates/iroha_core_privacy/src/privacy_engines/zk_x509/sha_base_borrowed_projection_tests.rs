// Actual borrowed-row admission, public selection and transactional clearing controls.

fn borrowed_projection_fixture_v1() -> ZkX509ShaBatchCallBaseSourceV1 {
    let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 4,
    })
    .unwrap();
    let manifest = schedule.calls[CRL_ISSUER_SPKI_CALL_V1];
    build_zk_x509_sha_batch_call_base_source_v1(manifest, &witness_for(manifest), 4).unwrap()
}

#[test]
fn sha_borrowed_projection_matches_full_row_for_every_public_range() {
    for disclosed in 0..=4 {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: disclosed,
        })
        .unwrap();
        for call_index in ZK_X509_SHA_PHYSICAL_CALL_ORDER_V1 {
            let manifest = schedule.call(usize::from(call_index)).unwrap();
            let call = build_zk_x509_sha_batch_call_base_source_v1(
                manifest,
                &witness_for(manifest),
                disclosed,
            )
            .unwrap();
            for row in [0, call.logical_rows() / 2, call.logical_rows() - 1] {
                let mut expected = PrivateTableV1::new(
                    vec![[F::ZERO; ZK_X509_SHA_BATCH_BASE_WIDTH_V1]],
                    zeroize_field_rows_v1,
                );
                expected[0] = call.base_row(row).unwrap();
                let mut selected = PrivateTableV1::new(vec![[F::ZERO; 8]], zeroize_field_rows_v1);
                for first in 0..ZK_X509_SHA_BATCH_BASE_WIDTH_V1 {
                    for width in 1..=8.min(ZK_X509_SHA_BATCH_BASE_WIDTH_V1 - first) {
                        selected[0].fill(F::ZERO);
                        let mut emitted = 0;
                        call.emit_base_columns_v1(row, first, width, |offset, value| {
                            assert_eq!(offset, emitted);
                            selected[0][offset] = value;
                            emitted += 1;
                        })
                        .unwrap();
                        assert_eq!(emitted, width);
                        assert_eq!(
                            &selected[0][..width],
                            &expected[0][first..first + width],
                            "disclosed={disclosed} call={call_index} row={row} first={first} width={width}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn sha_borrowed_projection_length_admission_precedes_original_row_bounds() {
    let mut call = borrowed_projection_fixture_v1();
    let message_len = call.word.message_len();
    assert!(message_len > 0);
    let original = call.rfc_consumer.unwrap();
    for (prefix, capacity) in [(message_len + 1, 0), (0, message_len - 1)] {
        let mut consumer = original;
        consumer.message_prefix_bytes = prefix;
        consumer.message_capacity_bytes = capacity;
        call.rfc_consumer = Some(consumer);
        let admitted_row = (0..call.logical_rows())
            .find(|&row| call.validate_fixed_row_shape_v1(row).is_ok())
            .expect("small invalid RFC length still has a valid fixed row");
        assert!(call.fixed_row(admitted_row).is_ok());
        for first in [0, ZK_X509_SHA_LENGTH_BITS_START_V1] {
            let mut emitted = false;
            assert_eq!(
                call.emit_base_columns_v1(admitted_row, first, 8, |_, _| emitted = true),
                Err(ZkX509ShaCallBusStarkErrorV1::LengthOrPadding)
            );
            assert!(
                !emitted,
                "RFC length admission must precede every selected write"
            );
        }
        for row in [0, call.logical_rows(), usize::MAX] {
            assert!(matches!(
                call.borrowed_base_row_v1(row),
                Err(ZkX509ShaCallBusStarkErrorV1::LengthOrPadding)
            ));
            assert_eq!(
                call.base_row(row).map(|_| ()),
                Err(ZkX509ShaCallBusStarkErrorV1::LengthOrPadding)
            );
        }
    }
    for (prefix, capacity, expected_length) in [(0, message_len, message_len), (message_len, 0, 0)]
    {
        let mut consumer = original;
        consumer.message_prefix_bytes = prefix;
        consumer.message_capacity_bytes = capacity;
        call.rfc_consumer = Some(consumer);
        let (base, mut length) = call.borrowed_base_row_v1(0).unwrap();
        assert_eq!(*length, Some(expected_length));
        assert!(core::ptr::eq(base, call.word.base_row(0).unwrap()));
        // Exercise the actual scalar owner on its live storage; no dead-stack
        // inspection or assertion about compiler-register erasure is made.
        zeroize::Zeroize::zeroize(&mut *length);
        assert!(length.is_none());
        assert_eq!(base, call.word.base_row(0).unwrap());
        assert!(matches!(
            call.borrowed_base_row_v1(call.logical_rows()),
            Err(ZkX509ShaCallBusStarkErrorV1::Resource)
        ));
    }
    call.rfc_consumer = None;
    let (base, length) = call.borrowed_base_row_v1(0).unwrap();
    assert!(length.is_none());
    assert!(core::ptr::eq(base, call.word.base_row(0).unwrap()));
    assert!(matches!(
        call.borrowed_base_row_v1(call.logical_rows()),
        Err(ZkX509ShaCallBusStarkErrorV1::Resource)
    ));
}

#[test]
fn sha_borrowed_projection_fixed_admission_precedes_unselected_length_failure() {
    let mut call = borrowed_projection_fixture_v1();
    let input_row = (0..call.logical_rows())
        .find(|&row| call.word.fixed_row(row).unwrap()[SHA_WORD_CAPACITY_INPUT_WORD_V1] == F::ONE)
        .unwrap();
    let mut consumer = call.rfc_consumer.unwrap();
    consumer.message_prefix_bytes = usize::MAX;
    consumer.message_capacity_bytes = 1;
    call.rfc_consumer = Some(consumer);
    assert!(matches!(
        call.borrowed_base_row_v1(input_row),
        Err(ZkX509ShaCallBusStarkErrorV1::LengthOrPadding)
    ));
    assert_eq!(
        call.fixed_row(input_row).map(|_| ()),
        Err(ZkX509ShaCallBusStarkErrorV1::Resource)
    );
    for first in [0, ZK_X509_SHA_LENGTH_BITS_START_V1] {
        let mut emitted = false;
        assert_eq!(
            call.emit_base_columns_v1(input_row, first, 8, |_, _| emitted = true),
            Err(ZkX509ShaCallBusStarkErrorV1::Resource)
        );
        assert!(!emitted);
    }
}

#[test]
fn sha_borrowed_projection_real_guards_clear_every_column_after_late_failure() {
    for fail_canonicality in [false, true] {
        let mut call = borrowed_projection_fixture_v1();
        let input_row = (0..call.logical_rows())
            .find(|&row| {
                call.word.fixed_row(row).unwrap()[SHA_WORD_CAPACITY_INPUT_WORD_V1] == F::ONE
            })
            .unwrap();
        let mut destinations = vec![vec![F(71); 2]; 8];
        let result = (|| {
            let mut targets = destinations
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>();
            let mut fills = sha_column_fill_batch_v1(&mut targets)?;
            call.emit_base_columns_v1(input_row, 0, fills.len(), |offset, value| {
                fills[offset].write_v1(0, value);
            })?;
            assert!(fills.iter().all(|fill| fill.written == 1));
            if fail_canonicality {
                // Deliberately hand the final REAL guard an invalid field after
                // the other columns complete. This tests atomic group commit,
                // not a claim that the canonical SHA constructor emits bad F.
                call.emit_base_columns_v1(input_row, 0, fills.len(), |offset, value| {
                    let value = if offset == 7 { F(u64::MAX) } else { value };
                    fills[offset].write_v1(1, value);
                })?;
                finish_sha_column_fill_batch_v1(fills)
            } else {
                let mut consumer = call.rfc_consumer.unwrap();
                consumer.message_prefix_bytes = usize::MAX;
                consumer.message_capacity_bytes = 1;
                call.rfc_consumer = Some(consumer);
                call.emit_base_columns_v1(input_row, 0, fills.len(), |offset, value| {
                    fills[offset].write_v1(1, value);
                })?;
                panic!("late fixed admission must refuse")
            }
        })();
        assert_eq!(
            result,
            Err(if fail_canonicality {
                ZkX509ShaCallBusStarkErrorV1::Topology
            } else {
                ZkX509ShaCallBusStarkErrorV1::Resource
            })
        );
        assert!(destinations.iter().flatten().all(|value| *value == F::ZERO));
        call.zeroize_private_v1();
        assert!(call.private_is_zeroized_v1());
    }
}

#[test]
fn sha_borrowed_projection_callback_unwind_clears_real_output_owners() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let mut call = borrowed_projection_fixture_v1();
    let mut destinations = vec![vec![F(79); 1]; 8];
    let mut reached = false;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut targets = destinations
            .iter_mut()
            .map(Vec::as_mut_slice)
            .collect::<Vec<_>>();
        let mut fills = sha_column_fill_batch_v1(&mut targets).unwrap();
        call.emit_base_columns_v1(0, ZK_X509_SHA_LENGTH_BITS_START_V1, 8, |offset, value| {
            fills[offset].write_v1(0, value);
            if offset == 3 {
                reached = true;
                panic!("deliberate borrowed SHA row callback unwind");
            }
        })
        .unwrap();
    }));
    assert!(reached && result.is_err());
    assert!(destinations.iter().flatten().all(|value| *value == F::ZERO));
    // The borrowed callback neither consumes nor clears its original call.
    // Its real original owner still admits replay, then explicitly clears.
    assert!(call.borrowed_base_row_v1(0).is_ok());
    call.zeroize_private_v1();
    assert!(call.private_is_zeroized_v1());
}

#[test]
fn sha_borrowed_projection_bits_only_preserves_late_witness_digest_refusal() {
    let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 4,
    })
    .unwrap();
    let mut witnesses = witnesses(&schedule);
    let late_call = usize::from(ZK_X509_SHA_PHYSICAL_CALL_ORDER_V1[1]);
    witnesses[late_call].digest[0] ^= 1;
    // Match the existing genuine late-witness negative: bypass only the outer
    // fixture constructor so per-call revalidation fails after earlier writes.
    let source = ZkX509ShaBatchSegmentBaseSourceV1 {
        schedule: &schedule,
        witnesses: &witnesses,
        replay: ZkX509ShaSegmentReplayV1::new(0).unwrap(),
        bound: false,
    };
    let mut destinations = vec![native_column_v1(F(71)); 8];
    assert_eq!(
        source.fill_base_columns_v1(
            0,
            ZK_X509_SHA_LENGTH_BITS_START_V1,
            &mut destinations
                .iter_mut()
                .map(Vec::as_mut_slice)
                .collect::<Vec<_>>(),
        ),
        Err(ZkX509ShaCallBusStarkErrorV1::Digest)
    );
    assert!(destinations.iter().flatten().all(|value| *value == F::ZERO));
}

#[test]
fn sha_borrowed_projection_optional_empty_keeps_canonical_sha_rows() {
    let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: 0,
    })
    .unwrap();
    let manifest = *schedule
        .calls
        .iter()
        .find(|manifest| manifest.activation == ZkX509ShaCallActivationV1::OptionalPrivate)
        .unwrap();
    let witness = ZkX509ShaCallWitnessV1 {
        role: manifest.role,
        message: Vec::new(),
        digest: canonical_dummy_digest_v1(),
    };
    let call = build_zk_x509_sha_batch_call_base_source_v1(manifest, &witness, 0).unwrap();
    assert_eq!(call.word.message_len(), 0);
    for row in [0, call.logical_rows() - 1] {
        let mut expected = PrivateTableV1::new(
            vec![[F::ZERO; ZK_X509_SHA_BATCH_BASE_WIDTH_V1]],
            zeroize_field_rows_v1,
        );
        expected[0] = call.base_row(row).unwrap();
        for (first, width) in [(0, 8), (75, 8), (84, 5)] {
            let mut emitted = 0;
            call.emit_base_columns_v1(row, first, width, |offset, value| {
                assert_eq!(offset, emitted);
                assert_eq!(value, expected[0][first + offset]);
                emitted += 1;
            })
            .unwrap();
            assert_eq!(emitted, width);
        }
        assert!(
            expected[0][ZK_X509_SHA_LENGTH_BITS_START_V1..]
                .iter()
                .all(|value| *value == F::ZERO)
        );
    }
}
