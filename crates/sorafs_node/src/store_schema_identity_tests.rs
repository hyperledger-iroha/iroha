// Canonical frame identities are explicit; these are local fixture checks, not runtime qualification.

#[test]
fn declared_frame_names_are_explicit_and_nominal_in_generic_parents() {
    crate::schema_identity_test_support::assert_identity::<ManifestIndex>(
        "sorafs_node::store::ManifestIndex",
    );
    crate::schema_identity_test_support::assert_identity::<StoredManifestRecord>(
        "sorafs_node::store::StoredManifestRecord",
    );
    crate::schema_identity_test_support::assert_identity::<StoredPorCommitmentV1>(
        "sorafs_node::store::StoredPorCommitmentV1",
    );
}

#[test]
fn stored_manifest_and_index_frames_advertise_explicit_schema_identity() {
    let temp = tempfile::tempdir().unwrap();
    let payload = b"persisted schema identity fixture";
    let (config, backend, manifest_id) = ingest_test_payload(&temp, payload, 0x65);
    let metadata_bytes = fs::read(
        backend
            .manifests_dir
            .join(&manifest_id)
            .join(METADATA_FILE_NAME),
    )
    .unwrap();
    let record: StoredManifestRecord = norito::decode_canonical(&metadata_bytes).unwrap();
    assert_eq!(
        crate::schema_identity_test_support::assert_canonical_frame(
            &record,
            "sorafs_node::store::StoredManifestRecord"
        ),
        metadata_bytes
    );
    let index_bytes = fs::read(&backend.index_path).unwrap();
    let index: ManifestIndex = norito::decode_canonical(&index_bytes).unwrap();
    assert_eq!(
        crate::schema_identity_test_support::assert_canonical_frame(
            &index,
            "sorafs_node::store::ManifestIndex"
        ),
        index_bytes
    );
    assert_eq!(record.content_length, payload.len() as u64);
    crate::schema_identity_test_support::assert_canonical_frame(
        &record.por_commitment,
        "sorafs_node::store::StoredPorCommitmentV1",
    );
    assert_eq!(index.entries.len(), 1);
    drop(backend);
    let reopened = StorageBackend::new(config).expect("real persisted reopen");
    assert!(reopened.manifest(&manifest_id).is_some());
}
