// Canonical frame identities are explicit; these are local fixture checks, not runtime qualification.

#[test]
fn declared_frame_names_are_explicit_and_nominal_in_generic_parents() {
    crate::schema_identity_test_support::assert_identity::<EvidenceViewerTransparencyProjectionV1>(
        "sorafs_node::evidence_viewer::EvidenceViewerTransparencyProjectionV1",
    );
    crate::schema_identity_test_support::assert_identity::<EvidenceViewerGrantClaimsV1>(
        "sorafs_node::evidence_viewer::EvidenceViewerGrantClaimsV1",
    );
    crate::schema_identity_test_support::assert_identity::<EvidenceViewerCheckpointEnvelopeV1>(
        "sorafs_node::evidence_viewer::EvidenceViewerCheckpointEnvelopeV1",
    );
    crate::schema_identity_test_support::assert_identity::<EvidenceViewerCheckpointStoreRecordV1>(
        "sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreRecordV1",
    );
    crate::schema_identity_test_support::assert_identity::<EvidenceViewerCheckpointV1>(
        "sorafs_node::evidence_viewer::EvidenceViewerCheckpointV1",
    );
    crate::schema_identity_test_support::assert_identity::<EvidenceViewerCompactionArchiveArtifactV1>(
        "sorafs_node::evidence_viewer::EvidenceViewerCompactionArchiveArtifactV1",
    );
    crate::schema_identity_test_support::assert_identity::<EvidenceViewerCompactionArchivePayloadV1>(
        "sorafs_node::evidence_viewer::EvidenceViewerCompactionArchivePayloadV1",
    );
    crate::schema_identity_test_support::assert_identity::<EvidenceViewerReceiptBodyV1>(
        "sorafs_node::evidence_viewer::EvidenceViewerReceiptBodyV1",
    );
    crate::schema_identity_test_support::assert_identity::<EvidenceViewerSignedCheckpointAnchorV1>(
        "sorafs_node::evidence_viewer::EvidenceViewerSignedCheckpointAnchorV1",
    );
    crate::schema_identity_test_support::assert_identity::<
        EvidenceViewerSignedCompactionArchiveHeadV1,
    >("sorafs_node::evidence_viewer::EvidenceViewerSignedCompactionArchiveHeadV1");
    crate::schema_identity_test_support::assert_identity::<EvidenceViewerSignedReceiptV1>(
        "sorafs_node::evidence_viewer::EvidenceViewerSignedReceiptV1",
    );
}

#[test]
fn viewer_real_checkpoint_frames_advertise_explicit_schema_identity() {
    let fixture = EvidenceViewerFixture::new();
    let service = fixture.open();
    let challenge = fixture.issue_challenge(
        &service,
        JUROR_ACCOUNT,
        EvidenceViewerRoleV1::Juror,
        [0xD4; 32],
        BASE_UNIX_MS,
    );
    fixture
        .create_session(
            &service,
            challenge.challenge.expose(),
            b"valid-webauthn-assertion-schema-identity",
            [0xD5; 32],
            BASE_UNIX_MS + 1,
        )
        .expect("commit a real session receipt before inspecting checkpoint frames");
    let record = assert_independent_viewer_checkpoint(&fixture, &service);
    let bytes = crate::schema_identity_test_support::assert_canonical_frame(
        &record,
        "sorafs_node::evidence_viewer::EvidenceViewerCheckpointStoreRecordV1",
    );
    assert_eq!(bytes, fs::read(&fixture.config.checkpoint_path).unwrap());
    let envelope: EvidenceViewerCheckpointEnvelopeV1 =
        norito::decode_canonical(&record.checkpoint_bytes).unwrap();
    assert_eq!(
        crate::schema_identity_test_support::assert_canonical_frame(
            &envelope,
            "sorafs_node::evidence_viewer::EvidenceViewerCheckpointEnvelopeV1"
        ),
        record.checkpoint_bytes
    );
    crate::schema_identity_test_support::assert_canonical_frame(
        &envelope.checkpoint,
        "sorafs_node::evidence_viewer::EvidenceViewerCheckpointV1",
    );
    assert!(!envelope.checkpoint.receipts.is_empty());
    for receipt in &envelope.checkpoint.receipts {
        crate::schema_identity_test_support::assert_canonical_frame(
            receipt,
            "sorafs_node::evidence_viewer::EvidenceViewerSignedReceiptV1",
        );
        crate::schema_identity_test_support::assert_canonical_frame(
            &receipt.body,
            "sorafs_node::evidence_viewer::EvidenceViewerReceiptBodyV1",
        );
    }
}
