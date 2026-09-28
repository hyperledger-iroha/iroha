// DA admission assertions on original native proposals over genuinely committed history.
impl NativeValidationFixture {
    fn pin_intent(
        &self,
        lane_id: LaneId,
        epoch: u64,
        sequence: u64,
        storage_ticket: StorageTicketId,
        manifest_hash: ManifestDigest,
    ) -> DaPinIntent {
        crate::da::signed_test_pin_intent(
            crate::da::signed_test_ingest_authorization(
                self.chain.network_id(),
                &self.user,
                lane_id,
                epoch,
                sequence,
                1,
            ),
            &self.user,
            storage_ticket,
            manifest_hash,
            None,
        )
    }
}
#[test]
fn native_validation_rejects_unknown_da_lane() {
    let fixture = NativeValidationFixture::new();
    let record = DaCommitmentRecord::new(
        LaneId::new(7),
        1,
        1,
        BlobDigest::new([0xAA; 32]),
        ManifestDigest::new([0xBB; 32]),
        DaProofScheme::MerkleSha256,
        Hash::prehashed([0xCC; 32]),
        None,
        RetentionClass::default(),
        StorageTicketId::new([0xEE; 32]),
        checked_da_ack_signature(0x11),
    );
    let bundle = DaCommitmentBundle::new(vec![record]);
    let mut signed = fixture.proposal(vec![fixture.transaction(2_001, None)], fixture.cadence());
    signed.set_da_commitments(Some(bundle));
    let result = fixture.validate(signed).unpack(|_| {});
    let Err((_, err)) = result else {
        panic!("expected DA commitment bundle rejection");
    };
    assert!(
        matches!(
            err.as_ref(),
            BlockValidationError::DaCommitmentBundle(DaCommitmentValidationError::ProofPolicy(
                crate::da::DaProofPolicyError::UnknownLane { .. }
            ))
        ),
        "expected unknown DA lane rejection, got {err:?}"
    );

    assert_eq!(fixture.chain.state().view().height(), 2);
}

#[test]
fn native_validation_rejects_duplicate_da_manifest() {
    let fixture = NativeValidationFixture::new();
    let make_record = |sequence: u64, tag: u8| {
        DaCommitmentRecord::new(
            LaneId::new(0),
            1,
            sequence,
            BlobDigest::new([tag; 32]),
            ManifestDigest::new([0xBB; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([tag; 32]),
            None,
            RetentionClass::default(),
            StorageTicketId::new([tag; 32]),
            checked_da_ack_signature(tag),
        )
    };
    let bundle = DaCommitmentBundle::new(vec![make_record(1, 0xC1), make_record(2, 0xC2)]);
    let mut signed = fixture.proposal(vec![fixture.transaction(2_001, None)], fixture.cadence());
    signed.set_da_commitments(Some(bundle));
    let result = fixture.validate(signed).unpack(|_| {});
    let Err((_, err)) = result else {
        panic!("expected DA commitment duplicate-manifest rejection");
    };
    assert!(matches!(
        err.as_ref(),
        BlockValidationError::DaCommitmentBundle(
            DaCommitmentValidationError::DuplicateManifest { lane }
        ) if *lane == LaneId::new(0)
    ));

    assert_eq!(fixture.chain.state().view().height(), 2);
}

#[test]
fn native_validation_rejects_duplicate_da_storage_ticket() {
    let fixture = NativeValidationFixture::new();
    let make_record = |sequence: u64, tag: u8| {
        DaCommitmentRecord::new(
            LaneId::new(0),
            1,
            sequence,
            BlobDigest::new([tag; 32]),
            ManifestDigest::new([tag; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([tag; 32]),
            None,
            RetentionClass::default(),
            StorageTicketId::new([0xDD; 32]),
            checked_da_ack_signature(tag),
        )
    };
    let bundle = DaCommitmentBundle::new(vec![make_record(1, 0xC1), make_record(2, 0xC2)]);
    let mut signed = fixture.proposal(vec![fixture.transaction(2_001, None)], fixture.cadence());
    signed.set_da_commitments(Some(bundle));
    let result = fixture.validate(signed).unpack(|_| {});
    let Err((_, err)) = result else {
        panic!("expected DA commitment duplicate-storage-ticket rejection");
    };
    assert!(matches!(
        err.as_ref(),
        BlockValidationError::DaCommitmentBundle(
            DaCommitmentValidationError::DuplicateStorageTicket { lane, epoch, sequence }
        ) if *lane == LaneId::new(0) && *epoch == 1 && *sequence == 2
    ));

    assert_eq!(fixture.chain.state().view().height(), 2);
}

#[test]
fn native_validation_rejects_da_commitment_hash_mismatch() {
    let fixture = NativeValidationFixture::new();
    let record = DaCommitmentRecord::new(
        LaneId::new(0),
        1,
        1,
        BlobDigest::new([0xAA; 32]),
        ManifestDigest::new([0xBB; 32]),
        DaProofScheme::MerkleSha256,
        Hash::prehashed([0xCC; 32]),
        None,
        RetentionClass::default(),
        StorageTicketId::new([0xDD; 32]),
        checked_da_ack_signature(0xEE),
    );
    let bundle = DaCommitmentBundle::new(vec![record]);
    let expected = bundle.merkle_commitment();
    let forged = Some(HashOf::<DaCommitmentBundle>::from_untyped_unchecked(
        Hash::prehashed([0xFA; 32]),
    ));
    let mut signed = fixture.proposal(vec![fixture.transaction(2_001, None)], fixture.cadence());
    signed.set_da_commitments(Some(bundle));
    let mut header = signed.header();
    header.set_da_commitments_hash(forged);
    signed.replace_header_for_testing(header);
    assert_ne!(expected, forged, "fixture forged hash must differ");
    let result = fixture.validate(signed).unpack(|_| {});
    let Err((_, err)) = result else {
        panic!("expected DA commitment hash mismatch rejection");
    };
    assert!(matches!(
        err.as_ref(),
        BlockValidationError::DaCommitmentHashMismatch { expected: seen_expected, actual }
            if *seen_expected == expected && *actual == forged
    ));

    assert_eq!(fixture.chain.state().view().height(), 2);
}

#[test]
fn native_validation_rejects_da_pin_intent_hash_mismatch() {
    let fixture = NativeValidationFixture::new();
    let intent = fixture.pin_intent(
        LaneId::new(0),
        1,
        1,
        StorageTicketId::new([0xAA; 32]),
        ManifestDigest::new([0xBB; 32]),
    );
    let bundle = DaPinIntentBundle::new(vec![intent]);
    let expected = bundle.merkle_commitment();
    let forged = Some(HashOf::<DaPinIntentBundle>::from_untyped_unchecked(
        Hash::prehashed([0xFB; 32]),
    ));
    let mut signed = fixture.proposal(vec![fixture.transaction(2_001, None)], fixture.cadence());
    signed.set_da_pin_intents(Some(bundle));
    let mut header = signed.header();
    header.set_da_pin_intents_hash(forged);
    signed.replace_header_for_testing(header);
    assert_ne!(expected, forged, "fixture forged hash must differ");
    let result = fixture.validate(signed).unpack(|_| {});
    let Err((_, err)) = result else {
        panic!("expected DA pin-intent hash mismatch rejection");
    };
    match err.as_ref() {
        BlockValidationError::DaPinIntentHashMismatch {
            expected: seen_expected,
            actual,
        } if *seen_expected == expected && *actual == forged => {}
        other => panic!("expected DA pin-intent hash mismatch, got {other:?}"),
    }

    assert_eq!(fixture.chain.state().view().height(), 2);
}

#[test]
fn native_validation_rejects_duplicate_da_pin_intent_ticket() {
    let fixture = NativeValidationFixture::new();
    let first = fixture.pin_intent(
        LaneId::new(0),
        1,
        1,
        StorageTicketId::new([0xAA; 32]),
        ManifestDigest::new([0xB1; 32]),
    );
    let duplicate_ticket = fixture.pin_intent(
        LaneId::new(0),
        1,
        2,
        StorageTicketId::new([0xAA; 32]),
        ManifestDigest::new([0xB2; 32]),
    );
    let bundle = DaPinIntentBundle::new(vec![first, duplicate_ticket]);
    let mut signed = fixture.proposal(vec![fixture.transaction(2_001, None)], fixture.cadence());
    signed.set_da_pin_intents(Some(bundle));
    let result = fixture.validate(signed).unpack(|_| {});
    let Err((_, err)) = result else {
        panic!("expected DA pin-intent duplicate-ticket rejection");
    };
    assert!(
        matches!(
            err.as_ref(),
            BlockValidationError::DaPinIntentBundle(
                DaPinIntentValidationError::DuplicateStorageTicket {
                    lane,
                    epoch: 1,
                    sequence: 2
                }
            ) if *lane == LaneId::new(0)
        ),
        "unexpected duplicate-ticket error: {err:?}"
    );

    assert_eq!(fixture.chain.state().view().height(), 2);
}

#[test]
fn native_validation_rejects_unsupported_da_pin_intent_version() {
    let fixture = NativeValidationFixture::new();
    let intent = fixture.pin_intent(
        LaneId::new(0),
        1,
        1,
        StorageTicketId::new([0xAA; 32]),
        ManifestDigest::new([0xBB; 32]),
    );
    let mut bundle = DaPinIntentBundle::new(vec![intent]);
    bundle.version = DaPinIntentBundle::VERSION_V1 + 1;
    let mut signed = fixture.proposal(vec![fixture.transaction(2_001, None)], fixture.cadence());
    signed.set_da_pin_intents(Some(bundle));
    let result = fixture.validate(signed).unpack(|_| {});
    let Err((_, err)) = result else {
        panic!("expected DA pin-intent version rejection");
    };
    assert!(matches!(
        err.as_ref(),
        BlockValidationError::DaPinIntentBundle(
            DaPinIntentValidationError::UnsupportedVersion { version }
        ) if *version == DaPinIntentBundle::VERSION_V1 + 1
    ));

    assert_eq!(fixture.chain.state().view().height(), 2);
}
