#[test]
fn manifest_references_resolve_without_copying_stored_metadata() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let (_config, backend, manifest_id) =
        ingest_test_payload(&temp_dir, b"manifest reference lookup payload", 0xE1);
    let stored = backend.manifest(&manifest_id).expect("stored manifest");
    // Identifier references resolve in any letter case to the canonical lowercase identifier.
    assert_eq!(
        backend.resolve_manifest_reference(&manifest_id).as_deref(),
        Some(manifest_id.as_str())
    );
    assert_eq!(
        backend
            .resolve_manifest_reference(&manifest_id.to_ascii_uppercase())
            .as_deref(),
        Some(manifest_id.as_str())
    );
    // Hex CID references resolve to the identifier carrying that CID.
    let cid_hex = hex::encode(stored.manifest_cid());
    assert_ne!(cid_hex, manifest_id);
    assert_eq!(
        backend.resolve_manifest_reference(&cid_hex).as_deref(),
        Some(manifest_id.as_str())
    );
    assert_eq!(
        backend
            .resolve_manifest_reference(&cid_hex.to_ascii_uppercase())
            .as_deref(),
        Some(manifest_id.as_str())
    );
    // Unknown identifiers, unknown CIDs and non-hex references do not resolve.
    assert_eq!(backend.resolve_manifest_reference(&"0".repeat(64)), None);
    assert_eq!(backend.resolve_manifest_reference("00ff"), None);
    assert_eq!(backend.resolve_manifest_reference("not-a-reference"), None);
    // Every accessor shares the stored descriptor rather than copying its chunk records.
    let listed = backend.manifests();
    assert_eq!(listed.len(), 1);
    assert!(Arc::ptr_eq(&listed[0], &stored));
    let by_digest = backend
        .manifest_by_digest(stored.manifest_digest())
        .expect("manifest by digest");
    assert!(Arc::ptr_eq(&by_digest, &stored));
    let by_cid = backend
        .manifest_by_cid_prefer_site(stored.manifest_cid())
        .expect("manifest by CID");
    assert!(Arc::ptr_eq(&by_cid, &stored));
}
