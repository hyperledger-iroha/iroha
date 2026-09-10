// Canonical finalized-order decoding, exact duplicate-field binding and recovery fixtures.

fn canonical_order_test_layouts() -> impl Iterator<Item = u8> {
    (0..=u8::MAX).filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
}
fn canonical_capture_page_fixture() -> ProviderIngestCompletedMusubiCaptureSourcePageV1 {
    ProviderIngestCompletedMusubiCaptureSourcePageV1 {
        network_id: test_network_id(),
        provider_id: LOCAL_PROVIDER,
        finalized_cursor: cursor(8),
        finalized_block_time_ms: 8_000,
        rows: vec![fixture_completed_musubi_capture_row(0x39, 0x86)],
        next_after_order_id: None,
    }
}
fn canonical_capture_assignment_fixture() -> ProviderIngestFinalizedAssignmentV1 {
    let factory = ProviderIngestFinalizedClaimFactoryV1::new_completed_musubi_capture(
        test_network_id(),
        LOCAL_PROVIDER,
        CompletedMusubiStoreInstanceV1::new(),
    );
    seal_completed_musubi_capture_source_page(
        canonical_capture_page_fixture(),
        &factory,
        test_network_id(),
        LOCAL_PROVIDER,
    )
    .expect("seal positive canonical fixture")
    .rows
    .remove(0)
}

#[test]
fn canonical_order_all_four_consumers_accept_every_valid_caller_layout() {
    let source = canonical_capture_page_fixture();
    let expected = decode_bound_replication_order(&source.rows[0].order).expect("canonical order");
    assert_eq!(
        expected.target_replicas,
        iroha_data_model::musubi::MUSUBI_MIN_HEALTHY_REPLICAS_V1
    );
    assert_eq!(expected.assignments.len(), 3);
    assert_eq!(source.rows[0].pin.manifest.policy.min_replicas, 3);
    assert_eq!(source.rows[0].order.status, ReplicationOrderStatus::Pending);
    assert_eq!(source.rows[0].order.provider_completions.len(), 1);
    let factory = ProviderIngestFinalizedClaimFactoryV1::new_completed_musubi_capture(
        test_network_id(),
        LOCAL_PROVIDER,
        CompletedMusubiStoreInstanceV1::new(),
    );
    let mut expected_authorization = None;
    for flags in canonical_order_test_layouts() {
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            decode_bound_replication_order(&source.rows[0].order).expect("canonical decoder"),
            expected
        );
        validate_completed_musubi_capture_source_page(
            &source,
            None,
            None,
            1,
            test_network_id(),
            LOCAL_PROVIDER,
        )
        .expect("source-page consumer accepts canonical order");
        let sealed = seal_completed_musubi_capture_source_page(
            source.clone(),
            &factory,
            test_network_id(),
            LOCAL_PROVIDER,
        )
        .expect("completed-claim consumer accepts canonical order");
        let row = &sealed.rows[0];
        let validated = validate_assignment(row, cursor(8), LOCAL_PROVIDER, runtime_policy())
            .expect("assignment consumer accepts canonical order");
        assert_eq!(
            validated.source_provider_ids,
            vec![SOURCE_PROVIDER, THIRD_PROVIDER]
        );
        if let Some(expected) = &expected_authorization {
            assert_eq!(&validated.authorization, expected);
        } else {
            expected_authorization = Some(validated.authorization.clone());
        }
        let outbox = ProviderIngestOutbox::in_memory(outbox_policy()).expect("outbox");
        outbox
            .enqueue(validated.authorization.clone())
            .expect("canonical typed authorization");
        let status = outbox
            .status(validated.authorization.job_id())
            .expect("retained status");
        assert_eq!(
            authorization_from_status_and_row(&status, row, cursor(8))
                .expect("status-recovery consumer"),
            validated.authorization
        );
    }
}

fn assert_order_consumers_reject(order: ReplicationOrderRecord) {
    assert!(matches!(
        decode_bound_replication_order(&order),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
    ));
    let mut source = canonical_capture_page_fixture();
    source.rows[0].order = order.clone();
    assert!(matches!(
        validate_completed_musubi_capture_source_page(
            &source,
            None,
            None,
            1,
            test_network_id(),
            LOCAL_PROVIDER
        ),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
    ));
    let factory = ProviderIngestFinalizedClaimFactoryV1::new_completed_musubi_capture(
        test_network_id(),
        LOCAL_PROVIDER,
        CompletedMusubiStoreInstanceV1::new(),
    );
    assert!(matches!(
        seal_completed_musubi_capture_source_page(
            source,
            &factory,
            test_network_id(),
            LOCAL_PROVIDER
        ),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedPage)
    ));
    let mut row = canonical_capture_assignment_fixture();
    let authorization = validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy())
        .expect("positive baseline")
        .authorization;
    let outbox = ProviderIngestOutbox::in_memory(outbox_policy()).expect("outbox");
    outbox
        .enqueue(authorization.clone())
        .expect("positive baseline authorization");
    let status = outbox
        .status(authorization.job_id())
        .expect("baseline status");
    row.order = order;
    assert!(matches!(
        validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy()),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
    ));
    assert!(matches!(
        authorization_from_status_and_row(&status, &row, cursor(8)),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
    ));
}

#[test]
fn canonical_order_rejects_real_alternate_frames_in_every_caller_layout() {
    let record = canonical_capture_page_fixture().rows.remove(0).order;
    let order = decode_bound_replication_order(&record).expect("positive order");
    let mut alternates = 0;
    for encoded_flags in canonical_order_test_layouts() {
        let frame = {
            let _encoding = norito::core::DecodeFlagsGuard::enter(encoded_flags);
            norito::to_bytes(&order).expect("well-formed advertised frame")
        };
        if frame == record.canonical_order {
            continue;
        }
        alternates += 1;
        let header = norito::core::Header::read(frame.as_slice()).expect("valid advertised header");
        {
            let _advertised = norito::core::DecodeFlagsGuard::enter(header.flags);
            assert_eq!(
                norito::decode_from_bytes_with_limits::<ReplicationOrderV1>(
                    &frame,
                    REPLICATION_ORDER_DECODE_LIMITS_V1
                )
                .expect("independent same-value alternate decode"),
                order
            );
        }
        let mut substituted = record.clone();
        substituted.canonical_order = frame;
        for flags in canonical_order_test_layouts() {
            let _caller = norito::core::DecodeFlagsGuard::enter(flags);
            assert_order_consumers_reject(substituted.clone());
        }
    }
    assert!(alternates > 0, "exercise actual alternate layouts");
}

#[test]
fn canonical_order_rejects_compression_and_oversize_before_decode_allocation() {
    let record = canonical_capture_page_fixture().rows.remove(0).order;
    let header =
        norito::core::Header::read(record.canonical_order.as_slice()).expect("canonical header");
    let mut compressed = record.canonical_order.clone();
    compressed[header.magic.len() + 2 + header.schema.len()] = norito::Compression::Zstd as u8;
    assert_eq!(
        norito::core::Header::read(compressed.as_slice())
            .expect("valid compression header")
            .compression,
        norito::Compression::Zstd
    );
    for frame in [
        compressed.clone(),
        compressed[..norito::core::Header::SIZE].to_vec(),
        vec![0; REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1 + 1],
    ] {
        let mut substituted = record.clone();
        substituted.canonical_order = frame;
        for flags in canonical_order_test_layouts() {
            let _caller = norito::core::DecodeFlagsGuard::enter(flags);
            let (result, usage) = norito::core::with_decode_limits_measured(
                REPLICATION_ORDER_DECODE_LIMITS_V1,
                || decode_bound_replication_order(&substituted),
            );
            assert!(matches!(
                result,
                Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
            ));
            assert_eq!(usage.total_allocated_bytes(), 0);
            assert_order_consumers_reject(substituted.clone());
        }
    }
}

#[test]
fn canonical_order_rebinds_body_and_record_timestamps_and_rejects_preapproval_issuance() {
    let record = canonical_capture_page_fixture().rows.remove(0).order;
    let body = decode_bound_replication_order(&record).expect("positive order");
    assert_eq!(
        (body.issued_at, body.deadline_at),
        (record.issued_epoch, record.deadline_epoch)
    );
    for issued in [true, false] {
        let mut record_substitution = record.clone();
        if issued {
            record_substitution.issued_epoch -= 1;
        } else {
            record_substitution.deadline_epoch += 1;
        }
        assert_order_consumers_reject(record_substitution);
        let mut changed_body = body.clone();
        if issued {
            changed_body.issued_at -= 1;
        } else {
            changed_body.deadline_at += 1;
        }
        changed_body
            .validate()
            .expect("semantically valid substituted payload");
        let mut body_substitution = record.clone();
        body_substitution.canonical_order =
            norito::encode_canonical(&changed_body).expect("canonical substituted body");
        assert_order_consumers_reject(body_substitution);
    }
    let mut row = canonical_capture_assignment_fixture();
    let mut changed_body = body;
    changed_body.issued_at = row.pin.manifest.approved_epoch.expect("approved pin") - 1;
    row.order.issued_epoch = changed_body.issued_at;
    row.order.canonical_order =
        norito::encode_canonical(&changed_body).expect("coherent preapproval order");
    decode_bound_replication_order(&row.order).expect("body/record agree so lifecycle must reject");
    assert!(matches!(
        validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy()),
        Err(ProviderIngestRuntimeErrorV1::InvalidFinalizedBinding)
    ));
}

#[test]
fn canonical_capture_reconciliation_fixture_is_native_lifecycle_coherent() {
    let fixture = verified_attestation_bundle_fixture(0xEC);
    let manifest = completed_attestation_manifest(&fixture);
    let source_row = completed_attestation_capture_source_row(&fixture, &manifest);
    let source = ProviderIngestCompletedMusubiCaptureSourcePageV1 {
        network_id: test_network_id(),
        provider_id: LOCAL_PROVIDER,
        finalized_cursor: cursor(8),
        finalized_block_time_ms: 8_000,
        rows: vec![source_row],
        next_after_order_id: None,
    };
    validate_completed_musubi_capture_source_page(
        &source,
        None,
        None,
        1,
        test_network_id(),
        LOCAL_PROVIDER,
    )
    .expect("source fixture shape");
    let factory = ProviderIngestFinalizedClaimFactoryV1::new_completed_musubi_capture(
        test_network_id(),
        LOCAL_PROVIDER,
        CompletedMusubiStoreInstanceV1::new(),
    );
    let sealed = seal_completed_musubi_capture_source_page(
        source,
        &factory,
        test_network_id(),
        LOCAL_PROVIDER,
    )
    .expect("seal native-shaped completed-local row");
    let row = &sealed.rows[0];
    let body = decode_bound_replication_order(&row.order).expect("canonical body");
    assert_eq!((body.issued_at, row.order.issued_epoch), (8, 8));
    assert_eq!((body.deadline_at, row.order.deadline_epoch), (20, 20));
    assert_eq!(row.pin.manifest.approved_epoch, Some(8));
    assert_eq!(
        (
            body.target_replicas,
            row.pin.manifest.policy.min_replicas,
            manifest.pin_policy.min_replicas
        ),
        (3, 3, 3)
    );
    assert_eq!(body.assignments.len(), 3);
    assert_eq!(row.order.provider_completions.len(), 1);
    assert_eq!(
        row.order.provider_completions[0].provider_id,
        ProviderId::new(LOCAL_PROVIDER)
    );
    assert_eq!(row.order.status, ReplicationOrderStatus::Pending);
    validate_assignment(row, cursor(8), LOCAL_PROVIDER, runtime_policy())
        .expect("reconciliation must reach storage and inventory assertions");
}

#[test]
fn canonical_stored_receipt_bound_measures_the_actual_frame_in_every_layout() {
    let row = fixture_musubi_row(0x38, 0x85);
    let authorization = validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy())
        .expect("canonical Musubi assignment")
        .authorization;
    let receipt =
        test_verified_musubi_receipt(row.musubi_archive.as_ref().unwrap(), &authorization);
    let expected =
        norito::encode_canonical(&receipt.to_stored()).expect("actual canonical receipt frame");
    assert!(expected.len() <= PROVIDER_INGEST_VERIFIED_MUSUBI_RECEIPT_MAX_CANONICAL_BYTES_V1);
    for flags in canonical_order_test_layouts() {
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(
            receipt
                .canonical_stored_len()
                .expect("canonical receipt frame measurement"),
            expected.len()
        );
        assert!(receipt.validate_stored(&authorization));
        let mut substituted = receipt.clone();
        substituted.network_id = foreign_test_network_id();
        assert!(!substituted.validate_stored(&authorization));
        let mut substituted = receipt.clone();
        substituted.manifest_digest = [0xEF; 32];
        assert!(!substituted.validate_stored(&authorization));
        let mut substituted = receipt.clone();
        substituted.semantic_release_manifest_digest = MusubiSemanticReleaseDigestV1::new([0; 32]);
        assert!(!substituted.validate_stored(&authorization));
    }
}

#[test]
fn verified_receipt_frame_replays_owned_fields_and_rejects_a_vector_root() {
    let row = fixture_musubi_row(0x38, 0x85);
    let authorization = validate_assignment(&row, cursor(8), LOCAL_PROVIDER, runtime_policy())
        .unwrap()
        .authorization;
    let receipt =
        test_verified_musubi_receipt(row.musubi_archive.as_ref().unwrap(), &authorization);
    let stored = receipt.to_stored();
    let bytes = crate::schema_identity_test_support::assert_canonical_frame(
        &stored,
        "sorafs_node::provider_ingest_runtime::StoredProviderIngestVerifiedMusubiBundleReceiptV1",
    );
    assert_eq!(bytes.len(), receipt.canonical_stored_len().unwrap());
    assert_eq!(
        norito::decode_canonical::<StoredProviderIngestVerifiedMusubiBundleReceiptV1>(&bytes)
            .unwrap(),
        stored
    );
    let foreign = norito::encode_canonical(&vec![stored]).unwrap();
    assert_eq!(
        norito::decode_canonical::<Vec<StoredProviderIngestVerifiedMusubiBundleReceiptV1>>(
            &foreign
        )
        .unwrap()
        .len(),
        1
    );
    assert!(matches!(
        norito::decode_canonical::<StoredProviderIngestVerifiedMusubiBundleReceiptV1>(&foreign),
        Err(norito::Error::SchemaMismatch)
    ));
    crate::schema_identity_test_support::assert_identity::<
        ProviderIngestMusubiCompletionClaimDigestPreimageV1,
    >(
        "sorafs_node::provider_ingest_runtime::ProviderIngestMusubiCompletionClaimDigestPreimageV1"
    );
}
