// Included inside the sidecar owner's tests; native custody, not protocol qualification.

#[test]
fn sidecar_custody_uses_the_original_native_exclusive_writer_lock() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("native-writer");
    let store = PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap();
    let original = FileIdentity::of(&store.writer_lock).unwrap();
    validate_sidecar_writer_v1(&store.staging, &store.writer_lock).unwrap();
    // Bypass the process-local lease to exercise the actual kernel ownership boundary.
    let contender = PrivateDirectory::open(&root)
        .unwrap()
        .open_child(SIDECAR_STAGING_DIRECTORY_V1)
        .unwrap();
    assert_eq!(
        acquire_sidecar_store_writer_lock_v1(&contender).unwrap_err(),
        PrivateSettlementSidecarStoreErrorV1::StoreAlreadyOpen
    );
    drop(contender);
    drop(store);
    let reopened = PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap();
    assert_eq!(FileIdentity::of(&reopened.writer_lock).unwrap(), original);
}

#[test]
fn sidecar_custody_rejects_unknown_names_before_discarding_a_staged_file() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("staging");
    drop(PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap());
    let owner = PrivateDirectory::open(&root)
        .unwrap()
        .open_child(SIDECAR_STAGING_DIRECTORY_V1)
        .unwrap();
    owner
        .write_atomic(".iroha-fs-1-0.tmp", b"partial", PublishMode::CreateNew)
        .unwrap();
    owner
        .write_atomic(".iroha-fs-01-0.tmp", b"unknown", PublishMode::CreateNew)
        .unwrap();
    let before = owner.entries(3).unwrap();
    assert_eq!(
        PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap_err(),
        PrivateSettlementSidecarStoreErrorV1::Corrupt
    );
    assert_eq!(owner.entries(3).unwrap(), before);
    assert_eq!(
        owner.read(".iroha-fs-1-0.tmp", 7).unwrap().as_slice(),
        b"partial"
    );
    owner.remove_private(".iroha-fs-01-0.tmp").unwrap();
    drop(owner);
    let recovered = PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap();
    assert_eq!(
        recovered.staging.entries(1).unwrap(),
        [SIDECAR_WRITER_LOCK_FILE_V1]
    );
}

#[test]
fn sidecar_custody_refuses_record_hardlinks_and_missing_replace_targets() {
    let fixture = sidecar_fixture();
    let digest = fixture.sidecar.payload_digest();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("records");
    let store = PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap();
    store.store(fixture.sidecar).unwrap();
    let name = sidecar_file_name_v1(digest);
    let before = store
        .root
        .read(
            &name,
            PRIVATE_SETTLEMENT_SIDECAR_MAX_RECORD_BYTES_V1 as usize,
        )
        .unwrap();
    assert!(
        store
            .persist_named_record_v1(&name, b"substitution", false)
            .is_err()
    );
    assert_eq!(store.root.read(&name, before.len()).unwrap(), before);
    let absent = sidecar_file_name_v1(Hash::prehashed([0x91; 32]));
    assert!(
        store
            .persist_named_record_v1(&absent, b"new", true)
            .is_err()
    );
    assert!(store.root.read_optional(&absent, 3).unwrap().is_none());
    fs::hard_link(root.join(&name), root.join(&absent)).unwrap();
    assert!(matches!(
        store.read_record_v1(digest),
        Err(PrivateSettlementSidecarStoreErrorV1::Corrupt)
    ));
    drop(store);
    assert_eq!(
        PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap_err(),
        PrivateSettlementSidecarStoreErrorV1::Corrupt
    );
}

#[test]
fn sidecar_custody_recovers_a_real_promotion_prefix_at_the_configured_count_limit() {
    let fixture = sidecar_fixture();
    let digest = fixture.sidecar.payload_digest();
    let material = provisional_material_fixture(&fixture);
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("promotion");
    let config = PrivateSettlementSidecarStoreConfigV1::new(
        1,
        PRIVATE_SETTLEMENT_SIDECAR_MAX_RECORD_BYTES_V1,
    )
    .unwrap();
    let store = PrivateSettlementFileSidecarStoreV1::open(&root, config).unwrap();
    store
        .store_provisional(material, fixture.sidecar.stored_at_height)
        .unwrap();
    // Exact crash point: final bytes have committed, provisional deletion has not.
    let final_record = DurablePrivateSettlementSidecarV1::new(fixture.sidecar);
    final_record.validate().unwrap();
    assert!(final_matches_provisional_v1(
        &final_record.sidecar,
        &store.read_provisional_record_v1(digest).unwrap()
    ));
    let encoded = norito::encode_canonical(&final_record).unwrap();
    store.persist_record_v1(digest, &encoded, false).unwrap();
    assert_eq!(store.root.entries(3).unwrap().len(), 3);
    drop(store);
    let recovered = PrivateSettlementFileSidecarStoreV1::open(&root, config).unwrap();
    assert!(recovered.read_record_v1(digest).unwrap() == final_record);
    assert!(
        recovered
            .root
            .read_optional(
                provisional_file_name_v1(digest),
                PRIVATE_SETTLEMENT_SIDECAR_MAX_RECORD_BYTES_V1 as usize
            )
            .unwrap()
            .is_none()
    );
    let state = recovered.state.lock().unwrap();
    assert_eq!(state.index.len(), 1);
    assert!(state.provisional_index.is_empty());
    assert_eq!(state.canonical_bytes, encoded.len() as u64);
    assert!(!state.poisoned);
}

#[cfg(unix)]
#[test]
fn sidecar_custody_rejects_replaced_lock_and_ancestor_without_publication() {
    use std::os::unix::fs::PermissionsExt as _;
    for substitute in ["lock", "ancestor"] {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path().join("original");
        let store = PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap();
        match substitute {
            "lock" => {
                fs::rename(
                    store.staging.path().join(SIDECAR_WRITER_LOCK_FILE_V1),
                    store.staging.path().join("displaced-lock"),
                )
                .unwrap();
                let replacement = store
                    .staging
                    .create_lock(SIDECAR_WRITER_LOCK_FILE_V1)
                    .unwrap();
                assert_ne!(
                    FileIdentity::of(&replacement).unwrap(),
                    FileIdentity::of(&store.writer_lock).unwrap()
                );
            }
            "ancestor" => {
                fs::rename(&root, temporary.path().join("displaced-root")).unwrap();
                fs::create_dir(&root).unwrap();
                fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            }
            _ => unreachable!(),
        }
        assert_eq!(
            validate_sidecar_writer_v1(&store.staging, &store.writer_lock),
            Err(PrivateSettlementSidecarStoreErrorV1::Corrupt)
        );
        let target = sidecar_file_name_v1(Hash::prehashed([0x73; 32]));
        let failure = store
            .persist_named_record_v1(&target, b"must not publish", false)
            .unwrap_err();
        assert!(!failure.may_have_committed);
        assert!(!root.join(target).exists());
    }
}

#[cfg(windows)]
#[test]
fn sidecar_custody_windows_retains_no_delete_sharing_for_root_and_lock() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("original");
    let store = PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap();
    assert!(fs::rename(&root, temporary.path().join("substitution")).is_err());
    assert!(
        fs::rename(
            store.staging.path().join(SIDECAR_WRITER_LOCK_FILE_V1),
            root.join("substitution-lock")
        )
        .is_err()
    );
    validate_sidecar_writer_v1(&store.staging, &store.writer_lock).unwrap();
}

#[test]
fn sidecar_custody_rejects_retired_flat_names_and_missing_original_staging() {
    let temporary = tempfile::tempdir().unwrap();
    for flat_name in [SIDECAR_WRITER_LOCK_FILE_V1, ".iroha-fs-1-0.tmp", ".tmp"] {
        let root = temporary
            .path()
            .join(format!("retired-{}", flat_name.len()));
        let owner = PrivateDirectory::open_or_create(&root).unwrap();
        owner
            .write_atomic(flat_name, b"retired", PublishMode::CreateNew)
            .unwrap();
        let before = owner.entries(1).unwrap();
        assert_eq!(
            PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap_err(),
            PrivateSettlementSidecarStoreErrorV1::Corrupt
        );
        assert_eq!(owner.entries(1).unwrap(), before);
        assert_eq!(owner.read(flat_name, 7).unwrap().as_slice(), b"retired");
    }
    let root = temporary.path().join("missing-staging");
    let fixture = sidecar_fixture();
    let digest = fixture.sidecar.payload_digest();
    let store = PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap();
    store.store(fixture.sidecar).unwrap();
    let original = store.read_record_v1(digest).unwrap();
    drop(store);
    fs::rename(
        root.join(SIDECAR_STAGING_DIRECTORY_V1),
        temporary.path().join("original-staging"),
    )
    .unwrap();
    assert_eq!(
        PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap_err(),
        PrivateSettlementSidecarStoreErrorV1::Corrupt
    );
    assert!(!root.join(SIDECAR_STAGING_DIRECTORY_V1).exists());
    let owner = PrivateDirectory::open(&root).unwrap();
    let bytes = owner
        .read(
            sidecar_file_name_v1(digest),
            PRIVATE_SETTLEMENT_SIDECAR_MAX_RECORD_BYTES_V1 as usize,
        )
        .unwrap();
    assert!(
        norito::decode_canonical::<DurablePrivateSettlementSidecarV1>(&bytes).unwrap() == original
    );
}

#[test]
fn sidecar_custody_preserves_staging_on_unknown_or_unsafe_committed_entry() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("preserve-staging");
    let fixture = sidecar_fixture();
    let digest = fixture.sidecar.payload_digest();
    let store = PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap();
    store.store(fixture.sidecar).unwrap();
    store
        .staging
        .write_atomic(".iroha-fs-1-0.tmp", b"partial", PublishMode::CreateNew)
        .unwrap();
    drop(store);
    let owner = PrivateDirectory::open(&root).unwrap();
    owner
        .write_atomic("unknown", b"unknown", PublishMode::CreateNew)
        .unwrap();
    assert!(PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).is_err());
    let staging = owner.open_child(SIDECAR_STAGING_DIRECTORY_V1).unwrap();
    assert_eq!(
        staging.read(".iroha-fs-1-0.tmp", 7).unwrap().as_slice(),
        b"partial"
    );
    owner.remove_private("unknown").unwrap();
    let linked = sidecar_file_name_v1(Hash::prehashed([0x98; 32]));
    fs::hard_link(root.join(sidecar_file_name_v1(digest)), root.join(&linked)).unwrap();
    assert_eq!(
        PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap_err(),
        PrivateSettlementSidecarStoreErrorV1::Corrupt
    );
    assert_eq!(
        staging.read(".iroha-fs-1-0.tmp", 7).unwrap().as_slice(),
        b"partial"
    );
    assert_eq!(owner.entries(3).unwrap().len(), 3);
}

#[cfg(unix)]
#[test]
fn sidecar_custody_restart_preserves_authenticated_records_under_64_descriptor_limit() {
    const CHILD_MARKER: &str = "IROHA_SIDECAR_STAGING_LIMIT_TEST";
    if std::env::var_os(CHILD_MARKER).is_none() {
        let result = std::process::Command::new("/bin/sh")
            .args(["-c", "limit=$(ulimit -n); if [ \"$limit\" = unlimited ] || [ \"$limit\" -gt 64 ]; then ulimit -n 64 || exit; fi; exec \"$1\" --exact private_settlement::sidecar_store::tests::sidecar_custody_restart_preserves_authenticated_records_under_64_descriptor_limit --nocapture", "iroha-sidecar-staging-test"])
            .arg(std::env::current_exe().unwrap()).env(CHILD_MARKER, "1").output().unwrap();
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(
            result.status.success()
                && stdout.contains("running 1 test")
                && stdout.contains("test result: ok. 1 passed; 0 failed"),
            "{}{}",
            stdout,
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("restart");
    let store = PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap();
    let mut originals = Vec::new();
    for ordinal in [0, 1] {
        let fixture = sidecar_fixture_with_threshold_and_ordinal(1, ordinal);
        let digest = fixture.sidecar.payload_digest();
        store.store(fixture.sidecar).unwrap();
        originals.push((digest, store.read_record_v1(digest).unwrap()));
    }
    let before = store.staged_lock_evidence_v1().unwrap();
    store
        .staging
        .write_atomic(".iroha-fs-1-0.tmp", b"interrupted", PublishMode::CreateNew)
        .unwrap();
    drop(store);
    let reopened = PrivateSettlementFileSidecarStoreV1::open(&root, Default::default()).unwrap();
    for (digest, original) in originals {
        assert!(reopened.read_record_v1(digest).unwrap() == original);
    }
    assert_eq!(reopened.staged_lock_evidence_v1().unwrap(), before);
    assert_eq!(
        reopened.staging.entries(1).unwrap(),
        [SIDECAR_WRITER_LOCK_FILE_V1]
    );
    assert_eq!(reopened.root.entries(3).unwrap().len(), 3);
    assert_eq!(
        acquire_sidecar_store_writer_lock_v1(&reopened.staging).unwrap_err(),
        PrivateSettlementSidecarStoreErrorV1::StoreAlreadyOpen
    );
}
