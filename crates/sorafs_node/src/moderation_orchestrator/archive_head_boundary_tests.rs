// Signed broker head admission preserves schema bounds and independent trust checks.

#[test]
fn archive_head_broker_accepts_real_signatures_and_rejects_alternate_caller_frames() {
    let fixture =
        moderation_panel_notification_archive_broker_fixture_v1().expect("signed fixture");
    let expected = fixture.expectation();
    let canonical = &fixture.canonical_signed_head;
    let (head, validation) =
        validate_moderation_panel_notification_archive_head_for_broker_v1(canonical, &expected)
            .expect("valid native producer head must fit the schema field budget");
    assert_eq!(validation, fixture.validation);
    assert_eq!(head.archive_signature, fixture.archive_signature);
    assert_eq!(norito::encode_canonical(&head).unwrap(), *canonical);
    assert!(
        matches!(
            norito::decode_canonical_with_limits::<ModerationPanelNotificationArchiveHeadV1>(
                canonical,
                DecodeLimits::new(canonical.len(), 16, usize::MAX, usize::MAX, 32),
            ),
            Err(norito::Error::FieldLengthExceeded { limit: 16, .. })
        ),
        "a valid fixture actually exercises fields longer than sixteen bytes"
    );
    let mut alternate_count = 0;
    for flags in 0..=u8::MAX {
        if norito::core::validate_header_flags(flags).is_err() {
            continue;
        }
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            validate_moderation_panel_notification_archive_head_for_broker_v1(canonical, &expected)
                .unwrap(),
            (head.clone(), validation)
        );
        let alternate = norito::to_bytes(&head).expect("advertised alternate head");
        if alternate != *canonical {
            alternate_count += 1;
            assert_eq!(
                norito::decode_from_bytes::<ModerationPanelNotificationArchiveHeadV1>(&alternate)
                    .unwrap(),
                head
            );
            assert_eq!(
                validate_moderation_panel_notification_archive_head_for_broker_v1(
                    &alternate, &expected
                ),
                Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
            );
        }
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert!(alternate_count > 0);
    for source_signature in [false, true] {
        let mut tampered = head.clone();
        if source_signature {
            tampered.source_attestation_signature[0] ^= 1;
        } else {
            tampered.archive_signature[0] ^= 1;
        }
        let bytes = norito::encode_canonical(&tampered).unwrap();
        assert_eq!(
            validate_moderation_panel_notification_archive_head_for_broker_v1(&bytes, &expected),
            Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
        );
    }
    let other_network = iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"other broker fixture network")),
    );
    for substituted in [
        ModerationPanelNotificationArchiveBrokerExpectationV1 {
            network_id: &other_network,
            ..expected.clone()
        },
        ModerationPanelNotificationArchiveBrokerExpectationV1 {
            archive_id: [0x19; 32],
            ..expected.clone()
        },
        ModerationPanelNotificationArchiveBrokerExpectationV1 {
            checkpoint_handle: "sealed-cas.different-checkpoint",
            ..expected.clone()
        },
        ModerationPanelNotificationArchiveBrokerExpectationV1 {
            checkpoint_attestation_public_key: [0x23; 32],
            ..expected.clone()
        },
    ] {
        assert_eq!(
            validate_moderation_panel_notification_archive_head_for_broker_v1(
                canonical,
                &substituted
            ),
            Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
        );
    }
}

#[test]
fn archive_head_broker_rejects_compression_before_allocation_and_preserves_limits() {
    let fixture =
        moderation_panel_notification_archive_broker_fixture_v1().expect("signed fixture");
    let expected = fixture.expectation();
    let canonical = &fixture.canonical_signed_head;
    let limits = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 128);
    let (positive, usage) = norito::core::with_decode_limits_measured(limits, || {
        validate_moderation_panel_notification_archive_head_for_broker_v1(canonical, &expected)
    });
    assert_eq!(positive.unwrap().1, fixture.validation);
    assert!(
        usage.total_allocated_bytes() > 0,
        "positive control exercises measured decode allocation"
    );
    let header = norito::core::Header::read(canonical.as_slice()).unwrap();
    let compression_offset = header.magic.len() + 2 + header.schema.len();
    let mut tagged = canonical.clone();
    tagged[compression_offset] = norito::Compression::Zstd as u8;
    let mut oversized_header = tagged[..norito::core::Header::SIZE].to_vec();
    oversized_header[compression_offset + 1..compression_offset + 9]
        .copy_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        norito::core::Header::read(oversized_header.as_slice())
            .unwrap()
            .length,
        u64::MAX
    );
    for bytes in [
        &tagged[..],
        &tagged[..norito::core::Header::SIZE],
        &oversized_header[..],
    ] {
        let (result, usage) = norito::core::with_decode_limits_measured(limits, || {
            validate_moderation_panel_notification_archive_head_for_broker_v1(bytes, &expected)
        });
        assert_eq!(
            result,
            Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
        );
        assert_eq!(
            usage.total_allocated_bytes(),
            0,
            "forbidden compression is rejected at the header"
        );
    }
    let no_allocation = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 128);
    assert_eq!(
        norito::with_decode_limits_scope(no_allocation, || {
            validate_moderation_panel_notification_archive_head_for_broker_v1(canonical, &expected)
        }),
        Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
    );
    let smaller = ModerationPanelNotificationArchiveBrokerExpectationV1 {
        archive_max_bytes: u64::try_from(canonical.len() - 1).unwrap(),
        ..expected.clone()
    };
    let (too_large, usage) = norito::core::with_decode_limits_measured(limits, || {
        validate_moderation_panel_notification_archive_head_for_broker_v1(canonical, &smaller)
    });
    assert_eq!(
        too_large,
        Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
    );
    assert_eq!(usage.total_allocated_bytes(), 0);
    assert_eq!(
        validate_moderation_panel_notification_archive_head_for_broker_v1(&[], &expected),
        Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
    );
    let mut trailing = canonical.clone();
    trailing.push(0);
    assert_eq!(
        validate_moderation_panel_notification_archive_head_for_broker_v1(&trailing, &expected),
        Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid)
    );
}
