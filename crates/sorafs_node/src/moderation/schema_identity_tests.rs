// Canonical frame identities are explicit; these are local fixture checks, not runtime qualification.

#[test]
fn declared_frame_names_are_explicit_and_nominal_in_generic_parents() {
    crate::schema_identity_test_support::assert_identity::<ModerationEvidenceViewerAuditReport>(
        "sorafs_node::moderation::ModerationEvidenceViewerAuditReport",
    );
    crate::schema_identity_test_support::assert_identity::<ModerationEvidenceViewerSnapshot>(
        "sorafs_node::moderation::ModerationEvidenceViewerSnapshot",
    );
    crate::schema_identity_test_support::assert_identity::<ModerationModelRegistrySnapshot>(
        "sorafs_node::moderation::ModerationModelRegistrySnapshot",
    );
    crate::schema_identity_test_support::assert_identity::<ModerationQuarantineAadHeaderV1>(
        "sorafs_node::moderation::ModerationQuarantineAadHeaderV1",
    );
    crate::schema_identity_test_support::assert_identity::<ModerationQuarantineChunkAadV1>(
        "sorafs_node::moderation::ModerationQuarantineChunkAadV1",
    );
    crate::schema_identity_test_support::assert_identity::<ModerationQuarantineImmutableMetadataV1>(
        "sorafs_node::moderation::ModerationQuarantineImmutableMetadataV1",
    );
    crate::schema_identity_test_support::assert_identity::<ModerationQuarantineObjectEnvelopeV1>(
        "sorafs_node::moderation::ModerationQuarantineObjectEnvelopeV1",
    );
    crate::schema_identity_test_support::assert_identity::<ModerationQuarantineObjectSnapshot>(
        "sorafs_node::moderation::ModerationQuarantineObjectSnapshot",
    );
    crate::schema_identity_test_support::assert_identity::<ModerationScreeningAuthorityBundleV1>(
        "sorafs_node::moderation::ModerationScreeningAuthorityBundleV1",
    );
    crate::schema_identity_test_support::assert_identity::<ModerationScreeningSnapshot>(
        "sorafs_node::moderation::ModerationScreeningSnapshot",
    );
}

#[test]
fn quarantine_real_aead_frames_advertise_explicit_schema_identity() {
    let wrapper = test_key_wrapper(0x75, "software://sorafs/moderation/schema-fixture");
    let binding = test_key_provider_binding();
    let payload = b"schema identity authenticated plaintext".to_vec();
    let (record, bytes) = seal_moderation_quarantine_object(
        ModerationQuarantineObjectInput {
            quarantine_id: [0x52; 16],
            payload: payload.clone(),
            captured_at_unix: 1_800_000_503,
            content_type: None,
            notes: None,
        },
        &binding,
        &wrapper,
    )
    .expect("real AEAD seal");
    let envelope = decode_moderation_quarantine_object_envelope(&bytes, 8 * 1024 * 1024).unwrap();
    assert_eq!(
        crate::schema_identity_test_support::assert_canonical_frame(
            &envelope,
            "sorafs_node::moderation::ModerationQuarantineObjectEnvelopeV1"
        ),
        bytes
    );
    let metadata = quarantine_immutable_metadata_from_envelope(&envelope).unwrap();
    crate::schema_identity_test_support::assert_canonical_frame(
        &metadata,
        "sorafs_node::moderation::ModerationQuarantineImmutableMetadataV1",
    );
    let header = quarantine_aad_header_from_envelope(&envelope).unwrap();
    crate::schema_identity_test_support::assert_canonical_frame(
        &header,
        "sorafs_node::moderation::ModerationQuarantineAadHeaderV1",
    );
    assert_eq!(
        open_moderation_quarantine_object(&envelope, &record, &binding, &wrapper)
            .unwrap()
            .as_slice(),
        payload
    );
}
