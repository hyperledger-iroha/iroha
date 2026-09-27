// Concurrent index ownership and degraded payload recovery regressions.

#[test]
fn reads_cannot_roll_back_other_manifest_admission_or_retirement() {
    for retire_other in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let (config, backend, first_id) = ingest_test_payload(&temp, b"read owner", 0x51);
        let backend = Arc::new(backend);
        let other_payload = b"independent commit";
        let other_plan = single_file_plan(other_payload).unwrap();
        let other_manifest = test_manifest(other_payload, &other_plan, 0x52);
        let other_id = hex::encode(other_manifest.digest().unwrap().as_bytes());
        if retire_other {
            backend
                .ingest_manifest(&other_manifest, &other_plan, &mut other_payload.as_slice())
                .unwrap();
        }
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let reader_backend = Arc::clone(&backend);
        let reader = thread::spawn(move || {
            reader_backend
                .with_manifest_for_access(&first_id, |_| {
                    // A payload callback retains only its own lifecycle lease, not global state.
                    assert!(reader_backend.state.try_write().is_ok());
                    entered_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(())
                })
                .unwrap();
        });
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        if retire_other {
            backend.evict_manifest(&other_id).unwrap();
        } else {
            backend
                .ingest_manifest(&other_manifest, &other_plan, &mut other_payload.as_slice())
                .unwrap();
        }
        let committed = fs::read(&backend.index_path).unwrap();
        release_tx.send(()).unwrap();
        reader.join().unwrap();
        assert_eq!(fs::read(&backend.index_path).unwrap(), committed);
        drop(backend);
        let reopened = StorageBackend::new(config).unwrap();
        assert_eq!(reopened.manifest(&other_id).is_some(), !retire_other);
        if !retire_other {
            assert_eq!(
                reopened
                    .read_payload_range(&other_id, 0, other_payload.len())
                    .unwrap(),
                other_payload
            );
        }
    }
}

#[test]
fn missing_payload_retains_metadata_and_repair_rebuilds_verified_proofs() {
    let temp = tempfile::tempdir().unwrap();
    let payload = b"repairable missing payload";
    let (config, backend, damaged_id) = ingest_test_payload(&temp, payload, 0x53);
    let healthy_payload = b"healthy independent manifest";
    let healthy_plan = single_file_plan(healthy_payload).unwrap();
    let healthy_manifest = test_manifest(healthy_payload, &healthy_plan, 0x54);
    let healthy_id = backend
        .ingest_manifest(
            &healthy_manifest,
            &healthy_plan,
            &mut healthy_payload.as_slice(),
        )
        .unwrap();
    let chunk = backend
        .manifest(&damaged_id)
        .unwrap()
        .chunk(0)
        .unwrap()
        .clone();
    drop(backend);
    fs::remove_file(&chunk.path).unwrap();
    let backend = StorageBackend::new(config.clone()).unwrap();
    let damaged = backend.manifest(&damaged_id).unwrap();
    assert!(!damaged.payload_available());
    assert!(damaged.load_manifest().is_ok());
    assert_eq!(
        backend
            .read_payload_range(&healthy_id, 0, healthy_payload.len())
            .unwrap(),
        healthy_payload
    );
    assert!(matches!(
        backend.sample_por(&damaged_id, 1, 0),
        Err(StorageError::PayloadUnavailable { .. })
    ));
    assert!(matches!(
        backend.prove_pdp_samples(&damaged_id, &[]),
        Err(StorageError::PayloadUnavailable { .. })
    ));
    backend
        .with_manifest_io(&damaged_id, |manifest| {
            assert!(backend.publish_verified_repair(manifest, &|| Ok(())).is_err());
            backend
                .replace_chunk_for_repair(manifest, &chunk, payload)
                .unwrap();
            assert!(backend.publish_verified_repair(manifest, &|| Err(crate::native_repair_worker::NativeRepairExecutionErrorV1::LeaseInvalid)).is_err());
            assert!(!backend.manifest(&damaged_id).unwrap().payload_available(), "verified bytes cannot clear quarantine after authority loss");
            backend.publish_verified_repair(manifest, &|| Ok(())).unwrap();
        })
        .unwrap();
    // Previously handed-out unavailable snapshots cannot produce proofs after publication.
    assert!(damaged.por_tree_ref().is_err());
    let repaired = backend.manifest(&damaged_id).unwrap();
    assert!(repaired.payload_available());
    assert_eq!(
        backend.read_chunk(&damaged_id, &chunk.digest).unwrap(),
        payload
    );
    let proofs = backend.sample_por(&damaged_id, 1, 0).unwrap();
    assert!(proofs[0].1.verify(repaired.por_tree_ref().unwrap().root()));
    drop(backend);
    let reopened = StorageBackend::new(config).unwrap();
    assert!(reopened.manifest(&damaged_id).unwrap().payload_available());
    assert_eq!(
        reopened.read_chunk(&damaged_id, &chunk.digest).unwrap(),
        payload
    );
}

#[test]
fn access_sequence_overflow_fails_without_touching_index() {
    let temp = tempfile::tempdir().unwrap();
    let (_config, backend, id) = ingest_test_payload(&temp, b"sequence exhaustion", 0x55);
    let before = fs::read(&backend.index_path).unwrap();
    backend.access_counter.store(u64::MAX, Ordering::Relaxed);
    assert!(matches!(
        backend.read_payload_range(&id, 0, 1),
        Err(StorageError::CorruptStorageState { .. })
    ));
    assert_eq!(fs::read(&backend.index_path).unwrap(), before);
}
