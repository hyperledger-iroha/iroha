// Canonical V1 persistence and signed identities use real viewer lifecycle records.

fn viewer_layouts() -> Vec<u8> {
    let flags = (0..=u8::MAX)
        .filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
        .collect::<Vec<_>>();
    assert_eq!(flags.len(), 10, "all supported V1 caller layouts");
    flags
}

fn independent_viewer_frame<T: norito::NoritoSerialize>(output: &mut Vec<u8>, value: &T) {
    let bytes = norito::encode_canonical(value).expect("independent canonical frame");
    output.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    output.extend_from_slice(&bytes);
}

fn independent_viewer_optional_digest(output: &mut Vec<u8>, digest: Option<[u8; 32]>) {
    output.push(u8::from(digest.is_some()));
    if let Some(digest) = digest {
        output.extend_from_slice(&digest);
    }
}

fn independent_viewer_archive_fields(
    head: &EvidenceViewerSignedCompactionArchiveHeadV1,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&head.version.to_le_bytes());
    bytes.extend_from_slice(&head.generation.to_le_bytes());
    independent_viewer_optional_digest(&mut bytes, head.predecessor_head_digest);
    independent_viewer_optional_digest(&mut bytes, head.predecessor_operation_id);
    bytes.extend_from_slice(&head.source_checkpoint_generation.to_le_bytes());
    bytes.extend_from_slice(&head.source_checkpoint_revision);
    independent_viewer_frame(&mut bytes, &head.source_checkpoint_anchor);
    bytes.extend_from_slice(&head.compacted_through_unix_ms.to_le_bytes());
    bytes.extend_from_slice(&head.maximum_records.to_le_bytes());
    bytes.extend_from_slice(&head.challenge_count.to_le_bytes());
    bytes.extend_from_slice(&head.session_count.to_le_bytes());
    bytes.extend_from_slice(&head.compacted_payload_digest);
    bytes.extend_from_slice(&(head.archive_handle.len() as u64).to_le_bytes());
    bytes.extend_from_slice(head.archive_handle.as_bytes());
    bytes.extend_from_slice(&head.archive_revision.to_le_bytes());
    bytes.extend_from_slice(&head.archive_policy_digest);
    bytes.extend_from_slice(&head.archive_id);
    bytes.extend_from_slice(&head.archive_public_key);
    bytes.extend_from_slice(&(head.signer_handle.len() as u64).to_le_bytes());
    bytes.extend_from_slice(head.signer_handle.as_bytes());
    bytes.extend_from_slice(&head.signer_public_key);
    bytes
}

fn independent_viewer_projection_digest(
    projection: &EvidenceViewerTransparencyProjectionV1,
) -> [u8; 32] {
    fn cursor(bytes: &mut Vec<u8>, cursor: Option<EvidenceViewerReceiptCursorV1>) {
        bytes.push(u8::from(cursor.is_some()));
        if let Some(cursor) = cursor {
            bytes.extend_from_slice(&cursor.sequence.to_le_bytes());
            bytes.extend_from_slice(&cursor.receipt_digest);
        }
    }
    let mut bytes = TRANSPARENCY_PROJECTION_DOMAIN_V1.to_vec();
    bytes.extend_from_slice(&projection.version.to_le_bytes());
    independent_viewer_frame(&mut bytes, &projection.checkpoint_anchor);
    bytes.push(u8::from(projection.compaction_archive_head.is_some()));
    if let Some(head) = projection.compaction_archive_head.as_ref() {
        independent_viewer_frame(&mut bytes, head);
    }
    cursor(&mut bytes, projection.predecessor);
    bytes.extend_from_slice(&projection.page_limit.to_le_bytes());
    bytes.extend_from_slice(&(projection.receipts.len() as u64).to_le_bytes());
    for receipt in &projection.receipts {
        independent_viewer_frame(&mut bytes, receipt);
    }
    cursor(&mut bytes, projection.next_cursor);
    bytes.push(u8::from(projection.has_more));
    *blake3::hash(&bytes).as_bytes()
}

fn assert_independent_viewer_checkpoint(
    fixture: &EvidenceViewerFixture,
    service: &EvidenceViewerServiceV1,
) -> EvidenceViewerCheckpointStoreRecordV1 {
    let record = fixture
        .checkpoint_store
        .current()
        .expect("committed checkpoint");
    assert_eq!(
        fs::read(&fixture.config.checkpoint_path).expect("persisted local cache"),
        norito::encode_canonical(&record).expect("independent canonical cache")
    );
    let envelope: EvidenceViewerCheckpointEnvelopeV1 =
        norito::decode_canonical(&record.checkpoint_bytes).expect("canonical persisted envelope");
    assert_viewer_frame(
        &record,
        "sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreRecordV1",
    );
    assert_viewer_frame(
        &envelope,
        "sorafs_node::evidence_viewer::EvidenceViewerCheckpointEnvelopeV1",
    );
    assert_viewer_frame(
        &envelope.checkpoint,
        "sorafs_node::evidence_viewer::EvidenceViewerCheckpointV1",
    );
    assert_eq!(
        record.checkpoint_bytes,
        norito::encode_canonical(&envelope).unwrap()
    );
    let mut bytes = CHECKPOINT_DIGEST_DOMAIN_V1.to_vec();
    bytes.extend_from_slice(&norito::encode_canonical(&envelope.checkpoint).unwrap());
    assert_eq!(record.checkpoint_digest, *blake3::hash(&bytes).as_bytes());
    verify_checkpoint_store_record(&fixture.config, &service.checkpoint_store, &record)
        .expect("actual signature and checkpoint verification");
    assert!(
        !envelope.checkpoint.receipts.is_empty(),
        "exercise signed receipt frames"
    );
    for receipt in &envelope.checkpoint.receipts {
        assert_viewer_frame(
            receipt,
            "sorafs_node::evidence_viewer::EvidenceViewerSignedReceiptV1",
        );
        assert_viewer_frame(
            &receipt.body,
            "sorafs_node::evidence_viewer::EvidenceViewerReceiptBodyV1",
        );
        let mut bytes = RECEIPT_BODY_DOMAIN_V1.to_vec();
        bytes.extend_from_slice(&norito::encode_canonical(&receipt.body).unwrap());
        let digest = *blake3::hash(&bytes).as_bytes();
        assert_eq!(receipt.receipt_digest, digest);
        let mut message = RECEIPT_SIGNATURE_DOMAIN_V1.to_vec();
        message.extend_from_slice(&digest);
        assert_eq!(
            receipt.signature,
            fixture.signer.signing_key.sign(&message).to_bytes()
        );
    }
    let projection = service
        .transparency_projection(record.checkpoint_digest, None, 16)
        .expect("exact signed projection");
    crate::frame_test_support::assert_current_frame(
        &projection,
        "sorafs_node::evidence_viewer::EvidenceViewerTransparencyProjectionV1",
    );
    assert_viewer_frame(
        &projection.checkpoint_anchor,
        "sorafs_node::evidence_viewer::EvidenceViewerSignedCheckpointAnchorV1",
    );
    assert_eq!(
        projection.projection_digest,
        independent_viewer_projection_digest(&projection)
    );
    projection
        .verify(
            &fixture.config.receipt_signer_handle,
            fixture.config.receipt_signer_public_key,
        )
        .expect("projection and every signature verify under caller layout");
    let mut changed = projection;
    changed.receipts[0].body.issued_at_unix_ms += 1;
    assert_eq!(
        changed.verify(
            &fixture.config.receipt_signer_handle,
            fixture.config.receipt_signer_public_key
        ),
        Err(EvidenceViewerErrorV1::InvalidCheckpoint)
    );
    record
}

#[test]
fn canonical_viewer_persistence_archive_and_signed_identities_ignore_caller_layout() {
    for flags in viewer_layouts() {
        let fixture = EvidenceViewerFixture::new();
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        let service = fixture.open();
        let challenge = fixture.issue_challenge(
            &service,
            JUROR_ACCOUNT,
            EvidenceViewerRoleV1::Juror,
            [0xE8; 32],
            BASE_UNIX_MS,
        );
        fixture
            .create_session(
                &service,
                challenge.challenge.expose(),
                b"valid-webauthn-assertion-canonical-archive",
                [0xE9; 32],
                BASE_UNIX_MS + 1,
            )
            .expect("commit signed receipt and session under caller layout");
        let before = assert_independent_viewer_checkpoint(&fixture, &service);
        let head = service
            .compact_expired_with_archive(EvidenceViewerCompactionArchiveRequestV1 {
                expected_checkpoint_anchor: service.audit_status().unwrap().checkpoint_anchor,
                expected_archive_head_digest: None,
                compacted_through_unix_ms: BASE_UNIX_MS + 1 + EVIDENCE_VIEWER_MAX_SESSION_TTL_MS_V1,
                maximum_records: 2,
            })
            .expect("persist and verify canonical archive before pruning");
        assert_eq!((head.challenge_count, head.session_count), (1, 1));
        assert_eq!(head.source_checkpoint_revision, before.revision);
        let artifact_bytes = fixture.compaction_archive.artifact(head.operation_id);
        let artifact = verify_compaction_archive_artifact(&fixture.config, &artifact_bytes)
            .expect("actual canonical signed archive");
        assert_eq!(artifact_bytes, norito::encode_canonical(&artifact).unwrap());
        assert_viewer_frame(
            &artifact,
            "sorafs_node::evidence_viewer::EvidenceViewerCompactionArchiveArtifactV1",
        );
        assert_viewer_frame(
            &artifact.payload,
            "sorafs_node::evidence_viewer::EvidenceViewerCompactionArchivePayloadV1",
        );
        assert_viewer_frame(
            &head,
            "sorafs_node::evidence_viewer::EvidenceViewerSignedCompactionArchiveHeadV1",
        );
        let mut payload = COMPACTION_ARCHIVE_PAYLOAD_DOMAIN_V1.to_vec();
        independent_viewer_frame(&mut payload, &artifact.payload);
        assert_eq!(
            head.compacted_payload_digest,
            *blake3::hash(&payload).as_bytes()
        );
        let fields = independent_viewer_archive_fields(&head);
        let operation = [COMPACTION_ARCHIVE_OPERATION_DOMAIN_V1, fields.as_slice()].concat();
        assert_eq!(head.operation_id, *blake3::hash(&operation).as_bytes());
        let message = [
            COMPACTION_ARCHIVE_SIGNATURE_DOMAIN_V1,
            fields.as_slice(),
            &head.operation_id,
        ]
        .concat();
        assert_eq!(
            head.signature,
            fixture
                .signer
                .signing_key
                .sign(blake3::hash(&message).as_bytes())
                .to_bytes()
        );
        let identity = [
            COMPACTION_ARCHIVE_HEAD_DOMAIN_V1,
            fields.as_slice(),
            &head.operation_id,
            &head.signature,
        ]
        .concat();
        assert_eq!(head.head_digest, *blake3::hash(&identity).as_bytes());
        let after = assert_independent_viewer_checkpoint(&fixture, &service);
        assert_eq!(after.predecessor_revision, Some(before.revision));
        drop(service);
        let recovered = fixture.open();
        assert_eq!(
            assert_independent_viewer_checkpoint(&fixture, &recovered),
            after
        );
        assert_eq!(current_compaction_archive_head(&recovered), Some(head));
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
}

#[test]
fn canonical_viewer_recovery_rejects_equivalent_alternate_frames_and_forgery() {
    let (fixture, head) = fixture_with_first_archive_generation();
    let service = fixture.open();
    let record = fixture.checkpoint_store.current().unwrap();
    let canonical_record = norito::encode_canonical(&record).unwrap();
    let envelope: EvidenceViewerCheckpointEnvelopeV1 =
        norito::decode_canonical(&record.checkpoint_bytes).unwrap();
    let canonical_artifact = fixture.compaction_archive.artifact(head.operation_id);
    let artifact =
        verify_compaction_archive_artifact(&fixture.config, &canonical_artifact).unwrap();
    let mut rejected = [0; 3];
    for flags in viewer_layouts() {
        let _caller = norito::core::DecodeFlagsGuard::enter(flags);
        write_local_checkpoint_store_record(&fixture.config, &record).unwrap();
        assert_eq!(
            fs::read(&fixture.config.checkpoint_path).unwrap(),
            canonical_record
        );
        assert_eq!(
            read_local_checkpoint_store_record(&fixture.config, &service.checkpoint_store).unwrap(),
            Some(record.clone())
        );
        assert_eq!(
            verify_compaction_archive_artifact(&fixture.config, &canonical_artifact).unwrap(),
            artifact
        );
        let alternate_record = norito::to_bytes(&record).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<EvidenceViewerCheckpointStoreRecordV1>(&alternate_record)
                .unwrap(),
            record
        );
        if alternate_record != canonical_record {
            rejected[0] += 1;
            fs::write(&fixture.config.checkpoint_path, &alternate_record).unwrap();
            assert_eq!(
                read_local_checkpoint_store_record(&fixture.config, &service.checkpoint_store),
                Err(EvidenceViewerErrorV1::InvalidCheckpoint)
            );
            write_local_checkpoint_store_record(&fixture.config, &record).unwrap();
        }
        let alternate_envelope = norito::to_bytes(&envelope).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<EvidenceViewerCheckpointEnvelopeV1>(&alternate_envelope)
                .unwrap(),
            envelope
        );
        if alternate_envelope != record.checkpoint_bytes {
            rejected[1] += 1;
            let mut changed = record.clone();
            changed.checkpoint_bytes = alternate_envelope;
            // Authenticate the exact changed outer bytes so only inner canonical admission fails.
            changed.signature = fixture
                .signer
                .signing_key
                .sign(&checkpoint_store_record_signature_message(&changed))
                .to_bytes();
            changed.revision = checkpoint_store_record_revision(&changed);
            assert_eq!(
                verify_checkpoint_store_record(
                    &fixture.config,
                    &service.checkpoint_store,
                    &changed
                ),
                Err(EvidenceViewerErrorV1::InvalidCheckpoint)
            );
        }
        let alternate_artifact = norito::to_bytes(&artifact).unwrap();
        assert_eq!(
            norito::decode_from_bytes::<EvidenceViewerCompactionArchiveArtifactV1>(
                &alternate_artifact
            )
            .unwrap(),
            artifact
        );
        if alternate_artifact != canonical_artifact {
            rejected[2] += 1;
            assert_eq!(
                verify_compaction_archive_artifact(&fixture.config, &alternate_artifact),
                Err(EvidenceViewerErrorV1::InvalidCheckpoint)
            );
        }
        let mut forged = artifact.clone();
        forged.head.signature[0] ^= 1;
        assert_eq!(
            verify_compaction_archive_artifact(
                &fixture.config,
                &norito::encode_canonical(&forged).unwrap()
            ),
            Err(EvidenceViewerErrorV1::InvalidCheckpoint)
        );
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert!(
        rejected.into_iter().all(|count| count > 0),
        "exercise each equivalent alternate frame"
    );
    assert_eq!(fixture.checkpoint_store.current(), Some(record));
    assert_eq!(
        fs::read(&fixture.config.checkpoint_path).unwrap(),
        canonical_record
    );
}

#[test]
fn compaction_archive_decode_rejects_a_maximal_nested_length_prefix() {
    let mut fixture = EvidenceViewerFixture::new();
    fixture.config.compaction_max_records = 7;
    let sequence_limit = compaction_archive_sequence_limit(&fixture.config);
    assert_eq!(sequence_limit, 7);
    // Challenge records are nested archive fields, so enforce their payload boundary directly.
    let _flags = norito::core::DecodeFlagsGuard::enter(0);
    let maximum_bytes =
        usize::try_from(compaction_archive_max_bytes(&fixture.config)).expect("byte limit");
    let limits = norito::DecodeLimits::new(
        sequence_limit,
        maximum_bytes,
        sequence_limit.saturating_mul(2),
        maximum_bytes.saturating_mul(4),
        64,
    );
    let (empty, used) = norito::with_decode_limits(limits, || {
        norito::core::decode_field_canonical::<Vec<ChallengeRecordV1>>(&0_u64.to_le_bytes())
    })
    .expect("empty nested collection control");
    assert!(empty.is_empty());
    assert_eq!(used, 8);
    let error = norito::with_decode_limits(limits, || {
        norito::core::decode_field_canonical::<Vec<ChallengeRecordV1>>(&u64::MAX.to_le_bytes())
    })
    .expect_err("maximal declared record count must fail before allocation");
    assert!(matches!(
        error,
        norito::core::Error::SequenceLengthExceeded {
            length: u64::MAX,
            limit: 7
        }
    ));
}

fn assert_viewer_frame<T>(value: &T, name: &str)
where
    T: norito::NoritoSerialize
        + for<'de> norito::NoritoDeserialize<'de>
        + PartialEq
        + std::fmt::Debug,
{
    assert_eq!(T::nominal_name(), name);
    assert_eq!(T::frame_name(), name);
    let bytes = norito::encode_canonical(value).expect("canonical viewer frame");
    assert_eq!(bytes[6..22], norito::schema::identity::frame_hash::<T>());
    let decoded: T = norito::decode_canonical(&bytes).expect("exact viewer frame roundtrip");
    assert_eq!(&decoded, value);
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), bytes);
    let mut wrong_owner = bytes.clone();
    wrong_owner[6] ^= 1;
    assert!(matches!(
        norito::decode_canonical::<T>(&wrong_owner),
        Err(norito::Error::SchemaMismatch)
    ));
    assert!(norito::decode_canonical::<T>(&bytes[..bytes.len() - 1]).is_err());
    let mut trailing = bytes;
    trailing.push(0);
    assert!(norito::decode_canonical::<T>(&trailing).is_err());
}
