#[test]
fn manifest_payload_rejects_unconsumed_trailing_bytes() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let committed_prefix = b"manifest payload prefix";
    let payload = [committed_prefix.as_slice(), b" attacker trailer"].concat();
    let (_config, backend, manifest_id) = ingest_test_payload(&temp_dir, &payload, 0xA2);
    let stored = backend.manifest(&manifest_id).expect("stored manifest");
    let mut source = ManifestPayload::new(&stored);
    let prefix_plan = single_file_plan(committed_prefix).expect("prefix plan");
    let mut rebuilt = ChunkStore::new();

    let error = rebuilt
        .ingest_plan_source(&prefix_plan, &mut source)
        .expect_err("uncommitted trailer must fail closed");
    assert!(matches!(
        error,
        ChunkStoreError::LengthMismatch { expected, actual }
            if expected == prefix_plan.content_length && actual == stored.content_length
    ));
}

#[test]
fn staged_car_reconstruction_rejects_short_trailing_and_corrupt_chunks() {
    let payload = b"bounded staged CAR reconstruction";
    let plan = single_file_plan(payload).expect("plan");
    assert_eq!(plan.chunks.len(), 1, "fixture must use one chunk");
    let manifest = test_manifest(payload, &plan, 0x93);
    let planned = &plan.chunks[0];
    let records = vec![StoredChunkRecord {
        file_name: "chunk_00000.bin".to_owned(),
        offset: planned.offset,
        length: planned.length,
        digest: planned.digest,
        role: None,
    }];

    for (label, staged_bytes) in [
        ("short", payload[..payload.len() - 1].to_vec()),
        ("trailing", [payload.as_slice(), &[0xA5]].concat()),
        ("corrupt", {
            let mut corrupt = payload.to_vec();
            corrupt[0] ^= 0x80;
            corrupt
        }),
    ] {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let chunks_dir = temp_dir.path().join("chunks");
        fs::create_dir(&chunks_dir).expect("create staged chunk directory");
        fs::write(chunks_dir.join("chunk_00000.bin"), staged_bytes).expect("write staged chunk");

        let error = verify_staged_manifest_car_archive(&manifest, &plan, &records, &chunks_dir)
            .expect_err("invalid staged chunk must fail closed");

        assert!(
            matches!(&error, StorageError::CarArchiveReconstruction { .. }),
            "{label} staged chunk produced unexpected error: {error}"
        );
    }
}
