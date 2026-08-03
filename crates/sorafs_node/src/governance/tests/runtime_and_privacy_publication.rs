// Runtime DAG, privacy publication, and authenticated appeal-finance regressions.

#[test]
fn filesystem_publisher_recovers_checkpoint_cas_applied_response_error() {
    let temp = tempdir().expect("tempdir");
    let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
    checkpoint_store
        .fail_after_next_checkpoint_cas
        .store(true, Ordering::SeqCst);
    {
        let publisher =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        let (settlement, encoded) = sample_settlement();
        let error = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("ambiguous checkpoint CAS response must surface");
        assert!(error.to_string().contains("compare-and-swap failed"));
    }
    assert!(
        checkpoint_store
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("load retained producer intent")
            .is_some()
    );
    fs::remove_dir_all(
        temp.path()
            .join(GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGING_DIR),
    )
    .expect("simulate loss of staging bytes after committed checkpoint CAS");

    let publisher = signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
    assert!(
        checkpoint_store
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("reload producer intent")
            .is_none(),
        "restart must authenticate the committed target and delete its retained intent"
    );
    let index = runtime_index(temp.path());
    assert_eq!(
        index.get("block_count").and_then(JsonValue::as_u64),
        Some(1)
    );
    drop(publisher);
}

#[test]
fn runtime_dag_producer_bounds_accept_exact_limits_and_reject_successors() {
    let mutable_limit = GOVERNANCE_MUTABLE_INDEX_MAX_BYTES;
    let block_limit = GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES;
    validate_runtime_dag_producer_file_lengths(block_limit, mutable_limit, mutable_limit)
        .expect("exact per-file limits are accepted");
    assert!(
        validate_runtime_dag_producer_file_lengths(block_limit + 1, 1, 1).is_err(),
        "block limit + 1 must fail before sealing"
    );
    assert!(
        validate_runtime_dag_producer_file_lengths(1, mutable_limit + 1, 1).is_err(),
        "head limit + 1 must fail before sealing"
    );
    assert!(
        validate_runtime_dag_producer_file_lengths(1, 1, mutable_limit + 1).is_err(),
        "index limit + 1 must fail before sealing"
    );
    const {
        assert!(
            GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES
                > GOVERNANCE_RUNTIME_DAG_SOURCE_PAYLOAD_MAX_BYTES
        );
    }
    validate_runtime_dag_producer_entry_count(
        GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1,
        u64::try_from(GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1).expect("entry cap fits u64"),
    )
    .expect("exact entry cap is accepted");
    assert!(
        validate_runtime_dag_producer_entry_count(
            GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1 + 1,
            u64::try_from(GOVERNANCE_RUNTIME_DAG_ENTRY_HARD_CAP_V1 + 1)
                .expect("entry cap successor fits u64"),
        )
        .is_err(),
        "entry cap + 1 must fail before sealing"
    );
    let mut total = GOVERNANCE_RUNTIME_DAG_TOTAL_BYTES_HARD_CAP_V1 - 1;
    add_runtime_dag_audit_bytes(&mut total, 1).expect("exact root byte cap is accepted");
    assert!(
        add_runtime_dag_audit_bytes(&mut total, 1).is_err(),
        "root byte cap + 1 must fail before sealing"
    );
    assert_eq!(
        governance_dag_sealed_state_payload_max_bytes_v1(
            GovernanceDagSealedStateSlot::ProducerCheckpoint,
        ),
        64 * 1024
    );
    assert_eq!(
        governance_dag_sealed_state_payload_max_bytes_v1(
            GovernanceDagSealedStateSlot::ProducerPublishIntent,
        ),
        64 * 1024
    );
    assert_eq!(
        governance_dag_sealed_state_payload_max_bytes_v1(
            GovernanceDagSealedStateSlot::IpfsRequestReplay,
        ),
        256 * 1024
    );
    assert_eq!(
        governance_dag_sealed_state_payload_max_bytes_v1(
            GovernanceDagSealedStateSlot::SignedHeadRequestReplay,
        ),
        256 * 1024
    );
    assert!(
        governance_dag_sealed_state_payload_max_bytes_v1(
            GovernanceDagSealedStateSlot::ProducerPublishIntent,
        ) < governance_dag_sealed_state_payload_max_bytes_v1(
            GovernanceDagSealedStateSlot::PublishIntent,
        ),
        "the digest-only producer intent must retain its small independent ceiling"
    );
    assert!(
        governance_dag_sealed_state_payload_max_bytes_v1(
            GovernanceDagSealedStateSlot::ProducerCheckpoint,
        ) < governance_dag_sealed_state_payload_max_bytes_v1(
            GovernanceDagSealedStateSlot::Checkpoint,
        ),
        "the bounded producer checkpoint must retain its small independent ceiling"
    );
}

#[test]
fn runtime_dag_producer_intent_is_digest_only_and_stage_tamper_fails_closed() {
    let temp = tempdir().expect("tempdir");
    let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
    checkpoint_store
        .fail_after_next_intent_cas
        .store(true, Ordering::SeqCst);
    {
        let publisher =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        let (settlement, encoded) = sample_settlement();
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("retain the digest-only producer intent");
    }
    let intent_record = checkpoint_store
        .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
        .expect("load retained producer intent")
        .expect("producer intent exists");
    assert!(
        intent_record.payload.len()
            <= governance_dag_sealed_state_payload_max_bytes_v1(
                GovernanceDagSealedStateSlot::ProducerPublishIntent,
            )
    );
    let intent: RuntimeDagProducerPublishIntentV1 =
        norito::decode_from_bytes(&intent_record.payload).expect("decode digest-only intent");
    let root_guard =
        GovernanceFilesystemRootGuard::capture_writer(temp.path()).expect("retain producer root");
    let staged = load_runtime_dag_producer_staged_transaction(temp.path(), &root_guard, &intent)
        .expect("authenticate the durable staged transaction");
    assert_eq!(
        intent.index.byte_len,
        u64::try_from(staged.index_bytes.len()).expect("staged index length fits u64")
    );

    let index_path = runtime_dag_producer_staging_paths(temp.path())[2].clone();
    let mut substituted = staged.index_bytes;
    substituted[0] ^= 0x80;
    fs::write(&index_path, &substituted).expect("substitute staged index");
    write_digest_sidecar(&root_guard, &index_path, &substituted)
        .expect("refresh only the unauthenticated sidecar");

    let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("reopen publisher root")
        .with_qualified_runtime_dag_providers(
            qualified_test_runtime_dag_signer(1, 0x31),
            qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store)),
        )
        .expect_err("staged index substitution must fail closed");
    assert!(error.to_string().contains("staged index is substituted"));
    assert!(
        checkpoint_store
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("reload retained producer intent")
            .is_some(),
        "a failed staged readback must not erase the recovery intent"
    );
}

#[test]
fn runtime_dag_checkpoint_wrapper_rejects_oversized_producer_records() {
    for slot in [
        GovernanceDagSealedStateSlot::ProducerCheckpoint,
        GovernanceDagSealedStateSlot::ProducerPublishIntent,
    ] {
        let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
        let oversized_payload =
            vec![0xA5; governance_dag_sealed_state_payload_max_bytes_v1(slot) + 1];
        let record = GovernanceDagSealedStateRecord::new(slot, 1, oversized_payload);
        let mut state = checkpoint_store
            .state
            .lock()
            .expect("lock test checkpoint store");
        state.records[TestRuntimeDagCheckpointStore::slot_index(slot)] = Some(record);
        drop(state);
        let qualified = qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store));
        let error = qualified
            .load(slot)
            .expect_err("oversized producer record must fail before canonical decode");
        assert!(error.to_string().contains("oversized record"));
    }
}

#[test]
fn runtime_dag_staging_root_is_created_through_the_retained_root() {
    let temp = tempdir().expect("tempdir");
    let publisher =
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
    ensure_runtime_dag_producer_staging_root(temp.path(), publisher.root_guard())
        .expect("create and synchronize the producer staging root");
    publisher
        .root_guard()
        .rooted_directory()
        .open_directory(OsStr::new(GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGING_DIR))
        .expect("staging root remains bound below the retained producer root");
}

#[test]
fn runtime_dag_payload_preflight_counts_without_allocating_dummy_envelopes() {
    let (settlement, encoded) = sample_settlement();
    let payload = GovernanceLogPayloadV1::DealSettlement(Box::new(settlement));

    assert_eq!(
        canonical_runtime_source_payload_len(&payload).expect("count canonical source"),
        encoded.len()
    );
    preflight_runtime_signed_dag_payload(&payload, encoded.len())
        .expect("small canonical payload fits every runtime DAG envelope");
    assert!(
        preflight_runtime_signed_dag_payload(&payload, encoded.len().saturating_add(1)).is_err(),
        "source-length substitution must fail before publication"
    );
}

#[test]
fn runtime_dag_audit_rejects_substituted_generated_at_with_fresh_sidecar() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());
    let (settlement, encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("seed signed runtime DAG");
    let index_path = temp.path().join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
    let mut index = runtime_index(temp.path());
    let head_generated_at = index
        .get("head_generated_at")
        .and_then(JsonValue::as_u64)
        .expect("head timestamp");
    index.as_object_mut().expect("runtime index object").insert(
        "generated_at".to_owned(),
        JsonValue::from(head_generated_at.saturating_add(1)),
    );
    let bytes = json::to_json_pretty(&index)
        .expect("encode tampered runtime index")
        .into_bytes();
    fs::write(&index_path, &bytes).expect("replace runtime index");
    write_digest_sidecar(publisher.root_guard(), &index_path, &bytes)
        .expect("replace index sidecar");

    let error = validate_existing_runtime_dag_root(
        temp.path(),
        publisher
            .runtime_dag_signer
            .as_ref()
            .expect("signed publisher"),
        publisher
            .runtime_dag_checkpoint_store
            .as_ref()
            .expect("signed publisher store"),
    )
    .expect_err("unchecked generated_at substitution must fail");
    assert!(error.to_string().contains("index and signed head"));
}

#[cfg(windows)]
#[test]
fn atomic_temp_recovery_deletes_the_exact_opened_windows_object() {
    let temp = tempdir().expect("tempdir");
    let target = temp.path().join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
    let stale = temp_path_for_atomic(&target, 42_000, 1);
    fs::write(&stale, b"recover-exact-object").expect("seed matching crash temp");
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
        .expect("retain Windows producer root");
    remove_recoverable_atomic_temps_for_target(&root_guard, &target)
        .expect("delete exact matching crash temp");
    assert!(!stale.exists());
}

#[cfg(unix)]
#[test]
fn filesystem_publisher_temp_recovery_never_follows_substituted_parent() {
    let temp = tempdir().expect("tempdir");
    let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
    checkpoint_store
        .fail_after_next_intent_cas
        .store(true, Ordering::SeqCst);
    {
        let publisher =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        let (settlement, encoded) = sample_settlement();
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("retain sealed producer intent");
    }
    let intent_record = checkpoint_store
        .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
        .expect("load sealed producer intent")
        .expect("producer intent exists");
    let intent: RuntimeDagProducerPublishIntentV1 =
        norito::decode_from_bytes(&intent_record.payload).expect("decode producer intent");
    let block_path = runtime_dag_producer_block_path_from_intent(temp.path(), &intent)
        .expect("resolve block path");
    let outside = temp.path().join("outside-runtime");
    let outside_blocks = outside.join(GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR);
    fs::create_dir_all(&outside_blocks).expect("create outside blocks directory");
    let outside_target = outside_blocks.join(
        block_path
            .file_name()
            .expect("producer block has a file name"),
    );
    let outside_temp = temp_path_for_atomic(&outside_target, 41_000, 9);
    fs::write(&outside_temp, b"must-remain-outside").expect("seed outside temp");
    std::os::unix::fs::symlink(&outside, temp.path().join(GOVERNANCE_RUNTIME_DAG_DIR))
        .expect("substitute runtime parent");

    let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("reopen publisher root")
        .with_qualified_runtime_dag_providers(
            qualified_test_runtime_dag_signer(1, 0x31),
            qualified_test_runtime_dag_checkpoint_store(checkpoint_store),
        )
        .expect_err("substituted parent must fail closed");
    assert!(error.to_string().contains("symlink") || error.to_string().contains("real directory"));
    assert_eq!(
        fs::read(&outside_temp).expect("outside temp remains"),
        b"must-remain-outside"
    );
}

#[cfg(unix)]
#[test]
fn filesystem_publisher_root_lock_rejects_symlink() {
    let temp = tempdir().expect("tempdir");
    let target = temp.path().join("lock-target");
    fs::write(&target, b"must remain untouched").expect("write lock target");
    std::os::unix::fs::symlink(&target, temp.path().join(GOVERNANCE_PUBLISHER_LOCK_FILE))
        .expect("create publisher lock symlink");

    let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect_err("publisher lock symlink must fail closed");
    assert!(error.to_string().contains("must not be a symlink"));
    assert_eq!(
        fs::read(&target).expect("read lock target"),
        b"must remain untouched"
    );
}

#[cfg(unix)]
#[test]
fn filesystem_publisher_root_lock_rejects_hard_link() {
    let temp = tempdir().expect("tempdir");
    let target = temp.path().join("lock-target");
    fs::write(&target, b"must remain untouched").expect("write lock target");
    fs::hard_link(&target, temp.path().join(GOVERNANCE_PUBLISHER_LOCK_FILE))
        .expect("create publisher lock hard link");

    let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect_err("publisher lock hard link must fail closed");
    assert!(error.to_string().contains("exactly one hard link"));
    assert_eq!(
        fs::read(&target).expect("read lock target"),
        b"must remain untouched"
    );
}

#[cfg(unix)]
#[test]
fn governance_directory_policy_enforces_role_owner_and_sticky_ancestor_matrix() {
    let effective_uid = 42;
    let producer_uid = 77;
    let unrelated_uid = 99;

    assert!(governance_directory_policy_accepts(
        effective_uid,
        0o755,
        true,
        effective_uid,
        effective_uid,
        true,
    ));
    assert!(!governance_directory_policy_accepts(
        0,
        0o755,
        true,
        effective_uid,
        effective_uid,
        true,
    ));
    assert!(!governance_directory_policy_accepts(
        effective_uid,
        0o1777,
        true,
        effective_uid,
        effective_uid,
        true,
    ));
    assert!(governance_directory_policy_accepts(
        producer_uid,
        0o755,
        true,
        effective_uid,
        producer_uid,
        false,
    ));
    assert!(!governance_directory_policy_accepts(
        effective_uid,
        0o755,
        true,
        effective_uid,
        producer_uid,
        false,
    ));

    for owner in [0, effective_uid, producer_uid] {
        assert!(governance_directory_policy_accepts(
            owner,
            0o755,
            false,
            effective_uid,
            producer_uid,
            false,
        ));
    }
    assert!(governance_directory_policy_accepts(
        0,
        0o1777,
        false,
        effective_uid,
        producer_uid,
        false,
    ));
    assert!(!governance_directory_policy_accepts(
        unrelated_uid,
        0o1777,
        false,
        effective_uid,
        producer_uid,
        false,
    ));
    assert!(!governance_directory_policy_accepts(
        effective_uid,
        0o775,
        false,
        effective_uid,
        producer_uid,
        false,
    ));
}

#[cfg(unix)]
#[test]
fn governance_root_guard_accepts_exact_canonical_root_and_trusted_sticky_parent() {
    let temp = tempdir().expect("canonical tempdir");
    let writer_guard =
        GovernanceFilesystemRootGuard::capture_writer(temp.path()).expect("canonical writer root");
    let source_guard =
        GovernanceFilesystemRootGuard::capture_source(temp.path()).expect("canonical source root");
    assert_eq!(writer_guard.root(), temp.path());
    assert_eq!(source_guard.root(), temp.path());
    writer_guard
        .revalidate()
        .expect("writer root remains pinned");
    source_guard
        .revalidate()
        .expect("source root remains pinned");

    let sticky = temp.path().join("sticky-parent");
    fs::create_dir(&sticky).expect("create sticky parent");
    fs::set_permissions(&sticky, fs::Permissions::from_mode(0o1777))
        .expect("set sticky-parent mode");
    let child = sticky.join("writer-root");
    fs::create_dir(&child).expect("create writer root");
    fs::set_permissions(&child, fs::Permissions::from_mode(0o700)).expect("secure writer root");
    GovernanceFilesystemRootGuard::capture_writer(&child)
        .expect("trusted sticky ancestor is accepted")
        .revalidate()
        .expect("sticky-root identity remains pinned");
}

#[cfg(target_os = "macos")]
#[test]
fn governance_root_guard_rejects_descriptor_bound_acl_mutation_grant() {
    let temp = tempdir().expect("canonical tempdir");
    let status = std::process::Command::new("chmod")
        .args(["+a", "everyone allow add_file"])
        .arg(temp.path())
        .status()
        .expect("install macOS ACL mutation grant");
    assert!(status.success(), "install macOS ACL mutation grant");
    let result = GovernanceFilesystemRootGuard::capture_writer(temp.path());
    let cleanup = std::process::Command::new("chmod")
        .arg("-RN")
        .arg(temp.path())
        .status()
        .expect("remove macOS ACL mutation grant");
    assert!(cleanup.success(), "remove macOS ACL mutation grant");
    let error = result.expect_err("ACL mutation grant must fail root capture");
    assert!(
        error.to_string().contains("ACL mutation grant"),
        "unexpected ACL rejection: {error}"
    );
}

#[cfg(unix)]
#[test]
fn governance_root_guard_rejects_lexical_symlink_ancestor() {
    let temp = tempdir().expect("canonical tempdir");
    let real_parent = temp.path().join("real-parent");
    let real_root = real_parent.join("producer");
    fs::create_dir_all(&real_root).expect("create real producer root");
    let linked_parent = temp.path().join("linked-parent");
    std::os::unix::fs::symlink(&real_parent, &linked_parent)
        .expect("create lexical ancestor symlink");

    let error = GovernanceFilesystemRootGuard::capture_writer(&linked_parent.join("producer"))
        .expect_err("lexical symlink ancestor must fail closed");
    assert!(
        error.to_string().contains("canonical")
            || error.to_string().contains("symlink")
            || error.to_string().contains("real directory"),
        "unexpected lexical-symlink error: {error}"
    );
}

#[cfg(unix)]
#[test]
fn filesystem_publisher_rejects_root_mode_drift_before_publication() {
    let temp = tempdir().expect("canonical tempdir");
    let publisher =
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o777)).expect("make root unsafe");
    let (settlement, encoded) = sample_settlement();
    let error = publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect_err("unsafe root mode must fail before publication");
    assert!(
        error.to_string().contains("mode") || error.to_string().contains("group/world writable"),
        "unexpected mode-drift error: {error}"
    );
    assert!(
        !temp
            .path()
            .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
            .exists()
    );
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700))
        .expect("restore root mode for cleanup");
}

#[cfg(unix)]
#[test]
fn filesystem_publisher_rejects_root_rename_replacement_without_touching_replacement() {
    let temp = tempdir().expect("canonical tempdir");
    let root = temp.path().join("producer");
    fs::create_dir(&root).expect("create producer root");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("secure producer root");
    let publisher = FilesystemGovernancePublisher::try_new(root.clone()).expect("publisher");
    let detached = temp.path().join("producer.detached");
    fs::rename(&root, &detached).expect("detach pinned producer root");
    fs::create_dir(&root).expect("create replacement producer root");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("secure replacement root");
    let marker = root.join("must-remain");
    fs::write(&marker, b"replacement").expect("seed replacement marker");

    let (settlement, encoded) = sample_settlement();
    let error = publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect_err("root replacement must fail before publication");
    assert!(
        error.to_string().contains("identity") || error.to_string().contains("changed"),
        "unexpected root-replacement error: {error}"
    );
    assert_eq!(
        fs::read(&marker).expect("replacement marker remains"),
        b"replacement"
    );
    assert!(!root.join(GOVERNANCE_PUBLICATION_SOURCES_DIR).exists());
    assert!(!detached.join(GOVERNANCE_PUBLICATION_SOURCES_DIR).exists());
}

#[cfg(unix)]
#[test]
fn filesystem_publisher_rejects_ancestor_replacement_and_symlink_without_writing_target() {
    let temp = tempdir().expect("canonical tempdir");
    let ancestor = temp.path().join("ancestor");
    let root = ancestor.join("producer");
    fs::create_dir_all(&root).expect("create producer root");
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("secure producer root");
    let publisher = FilesystemGovernancePublisher::try_new(root.clone()).expect("publisher");
    let detached = temp.path().join("ancestor.detached");
    fs::rename(&ancestor, &detached).expect("detach pinned ancestor");
    fs::create_dir(&ancestor).expect("create replacement ancestor");
    std::os::unix::fs::symlink(detached.join("producer"), &root)
        .expect("substitute producer root symlink");
    let marker = detached.join("producer").join("must-remain");
    fs::write(&marker, b"detached").expect("seed detached marker");

    let (settlement, encoded) = sample_settlement();
    let error = publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect_err("ancestor replacement must fail before publication");
    assert!(
        error.to_string().contains("identity")
            || error.to_string().contains("changed")
            || error.to_string().contains("real directory"),
        "unexpected ancestor-replacement error: {error}"
    );
    assert_eq!(
        fs::read(&marker).expect("detached marker remains"),
        b"detached"
    );
    assert!(!detached.join("producer/settlements").exists());
}

#[test]
fn runtime_dag_signer_rejects_invalid_handle_and_oversized_identity() {
    let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
    let signer = Arc::new(TestRuntimeDagSigner::new(
        "pkcs11:governance-dag:primary",
        &peer_id,
        0x31,
    ));
    let public_key = signer.public_key();

    validate_runtime_handle(
        "pkcs11:prod/governance-dag.primary-v1_slot-a",
        "governance runtime DAG signer",
    )
    .expect("canonical production runtime handle");
    for handle in [
        "contains whitespace",
        "https://operator:secret@governance-signer",
        "https://governance-signer/path?credential=secret",
        "https://governance-signer/path#fragment",
        "pkcs11:prod/%67overnance-signer",
        "pkcs11:prod\\governance-signer",
    ] {
        let error = GovernanceRuntimeDagSigner::try_new(
            handle.to_owned(),
            peer_id.clone(),
            public_key,
            test_runtime_dag_signer_qualification(),
            signer.clone(),
        )
        .expect_err("forbidden runtime-handle character must fail closed");
        assert!(error.to_string().contains("canonical credential-free"));
    }

    let error = GovernanceRuntimeDagSigner::try_new(
        signer.handle().to_owned(),
        vec![0x41; GOVERNANCE_DAG_PUBLISHER_PEER_ID_MAX_BYTES_V1 + 1],
        public_key,
        test_runtime_dag_signer_qualification(),
        signer,
    )
    .expect_err("oversized governance publisher identity must fail closed");
    assert!(
        error
            .to_string()
            .contains("publisher peer id exceeds 128 bytes")
    );
}

#[test]
fn runtime_dag_signer_rejects_test_marked_stale_and_drifting_provider() {
    let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
    let signer = Arc::new(TestRuntimeDagSigner::new(
        "pkcs11:governance-dag:primary",
        &peer_id,
        0x31,
    ));
    let error = GovernanceRuntimeDagSigner::try_new(
        "pkcs11:governance-dag:test".to_owned(),
        peer_id.clone(),
        signer.public_key(),
        test_runtime_dag_signer_qualification(),
        signer,
    )
    .expect_err("test-marked configured handle must fail closed");
    assert!(error.to_string().contains("test-marked"));

    let mut stale = TestRuntimeDagSigner::new("pkcs11:governance-dag:primary", &peer_id, 0x31);
    stale.qualification_error = Some("hsm_token=must-never-escape".to_owned());
    let stale = Arc::new(stale);
    let error = GovernanceRuntimeDagSigner::try_new(
        stale.handle().to_owned(),
        peer_id.clone(),
        stale.public_key(),
        test_runtime_dag_signer_qualification(),
        stale,
    )
    .expect_err("stale provider must fail startup qualification");
    assert!(error.to_string().contains("stale"));
    assert!(!error.to_string().contains("must-never-escape"));

    let invalid = Arc::new(TestRuntimeDagSigner::new(
        "pkcs11:governance-dag:primary",
        &peer_id,
        0x31,
    ));
    invalid.qualification_revision.store(0, Ordering::SeqCst);
    let error = GovernanceRuntimeDagSigner::try_new(
        invalid.handle().to_owned(),
        peer_id.clone(),
        invalid.public_key(),
        test_runtime_dag_signer_qualification(),
        invalid,
    )
    .expect_err("zero provider revision must fail startup qualification");
    assert!(error.to_string().contains("invalid policy qualification"));

    for expected_qualification in [
        GovernanceDagRuntimeProviderQualificationV1::new(2, TEST_RUNTIME_DAG_SIGNER_POLICY_DIGEST),
        GovernanceDagRuntimeProviderQualificationV1::new(1, [0x72; 32]),
    ] {
        let substituted = Arc::new(TestRuntimeDagSigner::new(
            "pkcs11:governance-dag:primary",
            &peer_id,
            0x31,
        ));
        let error = GovernanceRuntimeDagSigner::try_new(
            substituted.handle().to_owned(),
            peer_id.clone(),
            substituted.public_key(),
            expected_qualification,
            substituted,
        )
        .expect_err("substituted configured qualification must fail startup");
        assert!(
            error
                .to_string()
                .contains("does not match configured revision and digest")
        );
    }

    let drifting = Arc::new(TestRuntimeDagSigner::new(
        "pkcs11:governance-dag:primary",
        &peer_id,
        0x31,
    ));
    drifting
        .drift_on_second_qualification_read
        .store(true, Ordering::SeqCst);
    let error = GovernanceRuntimeDagSigner::try_new(
        drifting.handle().to_owned(),
        peer_id.clone(),
        drifting.public_key(),
        test_runtime_dag_signer_qualification(),
        drifting,
    )
    .expect_err("qualification drift on the second startup read must fail closed");
    assert!(
        error
            .to_string()
            .contains("policy changed during startup qualification")
    );

    let signer = Arc::new(TestRuntimeDagSigner::new(
        "pkcs11:governance-dag:primary",
        &peer_id,
        0x31,
    ));
    let wrapped = GovernanceRuntimeDagSigner::try_new(
        signer.handle().to_owned(),
        peer_id,
        signer.public_key(),
        test_runtime_dag_signer_qualification(),
        signer.clone(),
    )
    .expect("qualify stable signer");
    signer.qualification_revision.store(2, Ordering::SeqCst);
    let error = wrapped
        .sign(b"canonical governance payload")
        .expect_err("provider policy drift must fail closed");
    assert!(error.to_string().contains("policy changed"));

    let signer = Arc::new(TestRuntimeDagSigner::new(
        "pkcs11:governance-dag:primary",
        b"12D3KooWRuntimeDagPublisher",
        0x31,
    ));
    let wrapped = GovernanceRuntimeDagSigner::try_new(
        signer.handle().to_owned(),
        signer.publisher_peer_id().to_vec(),
        signer.public_key(),
        test_runtime_dag_signer_qualification(),
        signer.clone(),
    )
    .expect("qualify stable signer");
    signer.drift_during_sign.store(true, Ordering::SeqCst);
    let error = wrapped
        .sign(b"canonical governance payload")
        .expect_err("provider policy drift during signing must discard the signature");
    assert!(error.to_string().contains("policy changed"));
}

#[test]
fn runtime_dag_signer_rejects_handle_peer_and_public_key_mismatch() {
    let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
    let signer = Arc::new(TestRuntimeDagSigner::new(
        "pkcs11:governance-dag:primary",
        &peer_id,
        0x31,
    ));
    let public_key = signer.public_key();
    let mismatched_public_key =
        TestRuntimeDagSigner::new("pkcs11:governance-dag:primary", &peer_id, 0x32).public_key();

    for (handle, peer, key, expected) in [
        (
            "pkcs11:governance-dag:other",
            peer_id.clone(),
            public_key,
            "handle does not match",
        ),
        (
            signer.handle(),
            b"12D3KooWOtherPublisher".to_vec(),
            public_key,
            "publisher identity does not match",
        ),
        (
            signer.handle(),
            peer_id.clone(),
            mismatched_public_key,
            "public key does not match",
        ),
    ] {
        let error = GovernanceRuntimeDagSigner::try_new(
            handle.to_owned(),
            peer,
            key,
            test_runtime_dag_signer_qualification(),
            signer.clone(),
        )
        .expect_err("mismatched runtime signer must fail closed");
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn runtime_dag_signer_rejects_malformed_and_weak_ed25519_keys() {
    let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
    for (public_key, expected) in [
        ([0xFF; 32], "not canonical Ed25519"),
        (
            {
                let mut identity = [0_u8; 32];
                identity[0] = 1;
                identity
            },
            "non-canonical or weak",
        ),
    ] {
        let mut signer = TestRuntimeDagSigner::new("pkcs11:governance-dag:primary", &peer_id, 0x31);
        signer.public_key_override = Some(public_key);
        let signer = Arc::new(signer);
        let error = GovernanceRuntimeDagSigner::try_new(
            signer.handle().to_owned(),
            peer_id.clone(),
            public_key,
            test_runtime_dag_signer_qualification(),
            signer,
        )
        .expect_err("malformed or weak Ed25519 key must fail during provider binding");
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn runtime_dag_signer_redacts_provider_error_and_rejects_wrong_signature() {
    let peer_id = b"12D3KooWRuntimeDagPublisher".to_vec();
    let mut refusing = TestRuntimeDagSigner::new("pkcs11:governance-dag:primary", &peer_id, 0x31);
    refusing.refuse_with = Some("bearer=must-never-escape".to_owned());
    let refusing = Arc::new(refusing);
    let wrapped = GovernanceRuntimeDagSigner::try_new(
        refusing.handle().to_owned(),
        peer_id.clone(),
        refusing.public_key(),
        test_runtime_dag_signer_qualification(),
        refusing,
    )
    .expect("bind refusing test provider");
    let error = wrapped
        .sign(b"canonical governance payload")
        .expect_err("provider outage must fail closed");
    assert!(error.to_string().contains("refused"));
    assert!(!error.to_string().contains("must-never-escape"));

    let mut corrupt = TestRuntimeDagSigner::new("pkcs11:governance-dag:primary", &peer_id, 0x31);
    corrupt.corrupt_signature = true;
    let corrupt = Arc::new(corrupt);
    let wrapped = GovernanceRuntimeDagSigner::try_new(
        corrupt.handle().to_owned(),
        peer_id,
        corrupt.public_key(),
        test_runtime_dag_signer_qualification(),
        corrupt,
    )
    .expect("bind corrupt test provider");
    let error = wrapped
        .sign(b"canonical governance payload")
        .expect_err("wrong signature must fail closed");
    assert!(error.to_string().contains("another key or payload"));
}

#[test]
fn filesystem_publisher_serializes_concurrent_index_and_signed_head_updates() {
    const PUBLICATION_COUNT: usize = 16;

    let temp = tempdir().expect("tempdir");
    let publisher = Arc::new(signed_runtime_publisher(temp.path()));
    let (template, _) = sample_settlement();
    let threads = (0..PUBLICATION_COUNT)
        .map(|index| {
            let publisher = Arc::clone(&publisher);
            let mut settlement = template.clone();
            let marker = u8::try_from(index + 1).expect("small publication count");
            settlement.deal_id = [marker; 32];
            settlement.ledger.deal_id = settlement.deal_id;
            settlement.ledger.snapshot_id = settlement
                .ledger
                .derive_snapshot_id()
                .expect("reseal ledger snapshot");
            settlement.settlement_id = settlement
                .derive_settlement_id()
                .expect("reseal settlement");
            thread::spawn(move || {
                let encoded = norito::to_bytes(&settlement).expect("encode settlement");
                publisher
                    .publish_deal_settlement(&settlement, &encoded)
                    .expect("publish settlement concurrently");
            })
        })
        .collect::<Vec<_>>();

    for thread in threads {
        thread.join().expect("publisher thread");
    }

    let publish_index = read_publication_section_fixture(temp.path(), "publish_index");
    assert_eq!(
        publish_index.get("entry_count").and_then(JsonValue::as_u64),
        Some(PUBLICATION_COUNT as u64)
    );
    let entries = publish_index
        .get("entries")
        .and_then(JsonValue::as_array)
        .expect("publish index entries");
    assert_eq!(entries.len(), PUBLICATION_COUNT);
    for (expected_position, entry) in entries.iter().enumerate() {
        assert_eq!(
            entry.get("position").and_then(JsonValue::as_u64),
            Some(expected_position as u64)
        );
    }

    let runtime_index = runtime_index(temp.path());
    assert_eq!(
        runtime_index.get("block_count").and_then(JsonValue::as_u64),
        Some(PUBLICATION_COUNT as u64)
    );
    assert_eq!(
        runtime_index
            .get("blocks")
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(PUBLICATION_COUNT)
    );
}

#[test]
fn filesystem_publisher_poisoned_transaction_lock_fails_before_writes() {
    let temp = tempdir().expect("tempdir");
    let publisher =
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
    let poisoned = catch_unwind(AssertUnwindSafe(|| {
        let _guard = publisher
            .publication_lock
            .lock()
            .expect("publication lock starts healthy");
        panic!("poison publication transaction lock");
    }));
    assert!(poisoned.is_err());

    let (settlement, encoded) = sample_settlement();
    let error = publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect_err("poisoned publisher must fail closed");
    assert!(error.to_string().contains("transaction lock is poisoned"));
    assert!(
        !temp
            .path()
            .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
            .exists(),
        "poison detection must happen before artifact writes"
    );
    assert_empty_publication_authority(temp.path());
}

#[test]
fn filesystem_publisher_appends_signed_runtime_dag_for_supported_payloads() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());
    let (settlement, encoded) = sample_settlement();

    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("publish settlement into runtime DAG");
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("duplicate publish is idempotent");
    let index = runtime_index(temp.path());
    assert_eq!(
        index.get("block_count").and_then(JsonValue::as_u64),
        Some(1)
    );
    assert_eq!(
        index
            .get("by_payload_kind")
            .and_then(|value| value.get("deal_settlement"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );

    let (snapshot, snapshot_encoded) = sample_reputation_snapshot();
    publisher
        .publish_reputation_snapshot(&snapshot, &snapshot_encoded)
        .expect("publish reputation snapshot into runtime DAG");

    let (finance_report, finance_encoded) = sample_appeal_finance_report();
    publisher
        .publish_appeal_finance_report(&finance_report, &finance_encoded)
        .expect("publish appeal finance report into runtime DAG");

    let (finance_rollup, rollup_encoded) = sample_appeal_finance_weekly_rollup();
    publisher
        .publish_appeal_finance_weekly_rollup(&finance_rollup, &rollup_encoded)
        .expect("publish appeal finance weekly rollup into runtime DAG");

    let (finance_receipt, receipt_encoded) = sample_appeal_finance_settlement_receipt();
    publisher
        .publish_appeal_finance_settlement_receipt(&finance_receipt, &receipt_encoded)
        .expect("publish appeal finance settlement receipt into runtime DAG");

    let (transparency_publication, transparency_encoded) = sample_transparency_ledger_publication();
    publisher
        .publish_transparency_ledger_publication(
            &transparency_publication,
            &transparency_encoded,
            None,
        )
        .expect("publish transparency ledger publication into runtime DAG");

    let index = runtime_index(temp.path());
    assert_eq!(
        index.get("block_count").and_then(JsonValue::as_u64),
        Some(6)
    );
    assert_eq!(
        index
            .get("by_payload_kind")
            .and_then(|value| value.get("reputation_snapshot"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        index
            .get("by_payload_kind")
            .and_then(|value| value.get("appeal_finance_report"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        index
            .get("by_payload_kind")
            .and_then(|value| value.get("appeal_finance_weekly_rollup"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        index
            .get("by_payload_kind")
            .and_then(|value| value.get("appeal_finance_settlement_receipt"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        index
            .get("by_payload_kind")
            .and_then(|value| value.get("transparency_ledger_publication"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );

    let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
    let head: GovernanceDagHeadV1 =
        norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
    let blocks = runtime_blocks_from_index(temp.path(), &index);
    validate_governance_dag_head_against_chain_v1(&head, &blocks)
        .expect("runtime head validates against signed blocks");
    assert_eq!(blocks.len(), 6);
    assert_eq!(blocks[0].sequence, 0);
    assert_eq!(blocks[1].sequence, 1);
    assert_eq!(blocks[2].sequence, 2);
    assert_eq!(blocks[3].sequence, 3);
    assert_eq!(blocks[4].sequence, 4);
    assert_eq!(blocks[5].sequence, 5);
    assert_eq!(blocks[1].prev_block_cid, Some(blocks[0].block_cid.clone()));
    assert_eq!(blocks[2].prev_block_cid, Some(blocks[1].block_cid.clone()));
    assert_eq!(blocks[3].prev_block_cid, Some(blocks[2].block_cid.clone()));
    assert_eq!(blocks[4].prev_block_cid, Some(blocks[3].block_cid.clone()));
    assert_eq!(blocks[5].prev_block_cid, Some(blocks[4].block_cid.clone()));
    assert_eq!(
        blocks[1].node.prev_cid,
        Some(blocks[0].node.node_cid.clone())
    );
    assert_eq!(
        blocks[2].node.prev_cid,
        Some(blocks[1].node.node_cid.clone())
    );
    assert_eq!(
        blocks[3].node.prev_cid,
        Some(blocks[2].node.node_cid.clone())
    );
    assert_eq!(
        blocks[4].node.prev_cid,
        Some(blocks[3].node.node_cid.clone())
    );
    assert_eq!(
        blocks[5].node.prev_cid,
        Some(blocks[4].node.node_cid.clone())
    );
    match &blocks[0].node.payload {
        GovernanceLogPayloadV1::DealSettlement(value) => {
            assert_eq!(value.deal_id, settlement.deal_id);
        }
        other => panic!("unexpected first runtime DAG payload: {other:?}"),
    }
    match &blocks[1].node.payload {
        GovernanceLogPayloadV1::SignedReputationSnapshot(value) => {
            assert_eq!(value.snapshot.snapshot_id, snapshot.snapshot.snapshot_id);
        }
        other => panic!("unexpected second runtime DAG payload: {other:?}"),
    }
    match &blocks[2].node.payload {
        GovernanceLogPayloadV1::AppealFinanceReport(value) => {
            assert_eq!(value.report_id, finance_report.report_id);
            assert_eq!(value.case_id, finance_report.case_id);
        }
        other => panic!("unexpected third runtime DAG payload: {other:?}"),
    }
    match &blocks[3].node.payload {
        GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(value) => {
            assert_eq!(value.cycle, finance_rollup.cycle);
            assert_eq!(value.report_count, finance_rollup.report_count);
            assert_eq!(value.total_deposit_xor, finance_rollup.total_deposit_xor);
        }
        other => panic!("unexpected fourth runtime DAG payload: {other:?}"),
    }
    match &blocks[4].node.payload {
        GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(value) => {
            assert_eq!(value.receipt_id, finance_receipt.receipt_id);
            assert_eq!(value.tx_hash_hex, finance_receipt.tx_hash_hex);
            assert_eq!(
                value.reconciliation_digest_hex,
                finance_receipt.reconciliation_digest_hex
            );
        }
        other => panic!("unexpected fifth runtime DAG payload: {other:?}"),
    }
    match &blocks[5].node.payload {
        GovernanceLogPayloadV1::ExternalPayload(value) => {
            assert_eq!(value.payload_kind, "transparency_ledger_publication");
            assert_eq!(
                value.payload_version,
                MODERATION_LEDGER_PUBLICATION_VERSION_V1
            );
            assert_eq!(
                value.encoded_blake3,
                *blake3::hash(&transparency_encoded).as_bytes()
            );
            assert_eq!(value.encoded_len, transparency_encoded.len() as u64);
            assert_eq!(value.encoded_payload, transparency_encoded);
            assert_eq!(
                value
                    .metadata
                    .iter()
                    .map(|item| item.key.as_str())
                    .collect::<Vec<_>>(),
                vec![
                    "block_hash_hex",
                    "cycle_id_hex",
                    "entry_count",
                    "entry_root_hex",
                    "publication_hash_hex"
                ]
            );
        }
        other => panic!("unexpected sixth runtime DAG payload: {other:?}"),
    }
}

#[test]
fn filesystem_publisher_keeps_full_history_and_signs_checkpoint_window_with_one_identity() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());
    let (template, _) = sample_settlement();

    for marker in 1_u8..=GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 as u8 {
        let mut settlement = template.clone();
        settlement.deal_id = [marker; 32];
        settlement.ledger.deal_id = settlement.deal_id;
        settlement.ledger.snapshot_id = settlement
            .ledger
            .derive_snapshot_id()
            .expect("reseal ledger snapshot");
        settlement.settlement_id = settlement
            .derive_settlement_id()
            .expect("reseal settlement");
        let encoded = norito::to_bytes(&settlement).expect("encode settlement");
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish settlement into runtime DAG");
    }

    let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
    let head_at_window: GovernanceDagHeadV1 =
        norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
    assert_eq!(
        head_at_window.block_count,
        GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 as u64
    );
    assert_eq!(head_at_window.checkpoint_cid, None);

    let mut settlement = template;
    settlement.deal_id = [0xFF; 32];
    settlement.ledger.deal_id = settlement.deal_id;
    settlement.ledger.snapshot_id = settlement
        .ledger
        .derive_snapshot_id()
        .expect("reseal ledger snapshot");
    settlement.settlement_id = settlement
        .derive_settlement_id()
        .expect("reseal settlement");
    let encoded = norito::to_bytes(&settlement).expect("encode settlement");
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("publish first checkpointed settlement");

    let index = runtime_index(temp.path());
    let blocks = runtime_blocks_from_index(temp.path(), &index);
    assert_eq!(
        blocks.len(),
        GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 + 1,
        "checkpointing must not truncate the root history"
    );
    assert_eq!(blocks[0].sequence, 0);
    assert_eq!(blocks[0].prev_block_cid, None);
    assert_eq!(blocks[0].node.prev_cid, None);
    for (position, pair) in blocks.windows(2).enumerate() {
        assert_eq!(pair[1].sequence, (position + 1) as u64);
        assert_eq!(pair[1].prev_block_cid, Some(pair[0].block_cid.clone()));
        assert_eq!(pair[1].node.prev_cid, Some(pair[0].node.node_cid.clone()));
    }

    let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
    let head: GovernanceDagHeadV1 =
        norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
    assert_eq!(head.block_count, blocks.len() as u64);
    assert_eq!(head.checkpoint_cid, Some(blocks[1].block_cid.clone()));
    validate_governance_dag_head_against_chain_v1(&head, &blocks)
        .expect("full root chain validates against checkpointed head");
    validate_governance_dag_head_against_chain_v1(
        &head,
        &blocks[blocks.len() - GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1..],
    )
    .expect("canonical checkpoint tail validates against checkpointed head");

    let governed_public_key = &head.head_signature.public_key;
    assert_eq!(
        head.head_signature.algorithm,
        GovernanceSignatureAlgorithm::Ed25519
    );
    for block in &blocks {
        assert_eq!(block.publisher_peer_id, head.publisher_peer_id);
        assert_eq!(block.node.publisher_peer_id, head.publisher_peer_id);
        assert_eq!(
            block.block_signature.algorithm,
            GovernanceSignatureAlgorithm::Ed25519
        );
        assert_eq!(
            block.node.publisher_signature.algorithm,
            GovernanceSignatureAlgorithm::Ed25519
        );
        assert_eq!(&block.block_signature.public_key, governed_public_key);
        assert_eq!(
            &block.node.publisher_signature.public_key,
            governed_public_key
        );
    }
}

#[test]
fn filesystem_publisher_writes_moderation_ballot_event_files_and_runtime_dag() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());
    let (event, encoded) = sample_moderation_ballot_event();

    publisher
        .publish_moderation_ballot_event(&event, &encoded)
        .expect("publish moderation ballot event");

    let (encoded_path, json_path) =
        only_published_source_paths(temp.path(), "moderation_ballot_event");
    let bytes = fs::read(&encoded_path).expect("read moderation event payload");
    assert_eq!(bytes, encoded);
    let decoded: SoraFsModerationBallotGovernanceEventV1 =
        norito::decode_from_bytes(&bytes).expect("decode moderation event payload");
    assert_eq!(decoded, event);
    assert!(json_path.exists());

    let index = read_publication_section_fixture(temp.path(), "publish_index");
    assert_eq!(
        index
            .get("by_payload_kind")
            .and_then(|value| value.get("moderation_ballot_event"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );

    let runtime_index = runtime_index(temp.path());
    assert_eq!(
        runtime_index
            .get("by_payload_kind")
            .and_then(|value| value.get("moderation_ballot_event"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );
    let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
    let head: GovernanceDagHeadV1 =
        norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
    let blocks = runtime_blocks_from_index(temp.path(), &runtime_index);
    validate_governance_dag_head_against_chain_v1(&head, &blocks)
        .expect("runtime head validates against signed blocks");
    assert_eq!(blocks.len(), 1);
    let expected_provenance =
        test_submission_provenance(crate::GovernanceSubmissionOriginV1::AppealFinanceReport)
            .to_dag_provenance();
    assert_eq!(
        blocks[0].node.submission_provenance.as_ref(),
        Some(&expected_provenance)
    );
    match &blocks[0].node.payload {
        GovernanceLogPayloadV1::ModerationBallotEvent(value) => {
            assert_eq!(value.case_id, event.case_id);
            assert_eq!(value.round_id, event.round_id);
            assert_eq!(value.kind, event.kind);
        }
        other => panic!("unexpected runtime DAG payload: {other:?}"),
    }
}

#[test]
fn fused_privacy_publisher_retries_the_exact_request_idempotently() {
    let provider = Arc::new(TestFencedTransparencyPublisher::new());
    let publisher = qualified_test_fenced_publisher(Arc::clone(&provider));
    let request = sample_fenced_request(7, None);

    let first = publisher
        .compare_and_append_privacy_classified(&request)
        .expect("first fused append");
    let retried = publisher
        .compare_and_append_privacy_classified(&request)
        .expect("idempotent fused retry");

    assert_eq!(retried, first);
    assert_eq!(provider.append_count(), 1);
    assert_eq!(provider.head(), Some(first.included_head()));
}

#[test]
fn fused_privacy_target_deduplicates_same_lease_before_fencing() {
    let provider = Arc::new(TestFencedTransparencyPublisher::new());
    let publisher = qualified_test_fenced_publisher(Arc::clone(&provider));
    let first_request = sample_fenced_request(7, None);
    let first = publisher
        .compare_and_append_privacy_classified(&first_request)
        .expect("first fused append");
    let (publication, encoded) = sample_privacy_publication();
    let same_lease_authorization =
        sample_privacy_authorization(&publication, &encoded, first_request.fencing_token());
    let same_lease_request = FencedPrivacyPublicationRequestV1::try_new(
        same_lease_authorization,
        &publication,
        encoded,
        Some(first.included_head()),
        first.included_head().fencing_floor(),
    )
    .expect("same-lease lookup request remains structurally valid");

    let duplicate = publisher
        .compare_and_append_privacy_classified(&same_lease_request)
        .expect("stable scope lookup precedes stale-fence rejection");

    assert_eq!(
        duplicate.disposition(),
        FencedPrivacyPublicationDispositionV1::AlreadyIncluded
    );
    assert_eq!(duplicate.included_head(), first.included_head());
    assert_eq!(duplicate.readback_head(), first.readback_head());
    assert_eq!(provider.append_count(), 1);
}

#[test]
fn fused_privacy_target_rejects_conflicting_release_evidence_for_scope() {
    let provider = Arc::new(TestFencedTransparencyPublisher::new());
    let publisher = qualified_test_fenced_publisher(Arc::clone(&provider));
    let first_request = sample_fenced_request(7, None);
    let first = publisher
        .compare_and_append_privacy_classified(&first_request)
        .expect("first fused append");
    let conflicting_spec = SamplePrivacyReleaseSpec {
        release_record_digest: [0xB8; 32],
        ..SamplePrivacyReleaseSpec::primary()
    };
    let (publication, encoded) = sample_privacy_publication();
    let conflicting_authorization =
        sample_privacy_authorization_for(conflicting_spec, &publication, &encoded, 8, None);
    let conflicting_request = FencedPrivacyPublicationRequestV1::try_new(
        conflicting_authorization,
        &publication,
        encoded,
        Some(first.included_head()),
        first.included_head().fencing_floor(),
    )
    .expect("conflicting stable-scope request");

    let error = publisher
        .compare_and_append_privacy_classified(&conflicting_request)
        .expect_err("one release scope cannot change its release evidence");

    assert!(
        error
            .error
            .to_string()
            .contains("identity conflicts with an existing publication")
    );
    assert!(!error.may_have_appended);
    assert_eq!(provider.append_count(), 1);
    assert_eq!(provider.head(), Some(first.included_head()));
}

#[test]
fn fenced_head_reader_qualification_rejects_substitution_staleness_and_test_markers() {
    let target = Arc::new(TestFencedTransparencyPublisher::new());

    let substituted = Arc::new(TestFencedTransparencyHeadReader::with_handle(
        Arc::clone(&target),
        "https-pinned:governance:fenced-privacy-head-secondary",
    ));
    let substituted: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1> = substituted;
    let error = QualifiedFencedTransparencyHeadReaderV1::try_new(
        TEST_FENCED_HEAD_READER_HANDLE.to_owned(),
        test_fenced_head_reader_qualification(),
        substituted,
    )
    .expect_err("substituted reader identity must fail");
    assert!(error.to_string().contains("does not match configuration"));

    let stale = test_fenced_head_reader(Arc::clone(&target));
    stale.set_revision(2);
    let stale: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1> = stale;
    let error = QualifiedFencedTransparencyHeadReaderV1::try_new(
        TEST_FENCED_HEAD_READER_HANDLE.to_owned(),
        test_fenced_head_reader_qualification(),
        stale,
    )
    .expect_err("stale reader policy must fail");
    assert!(error.to_string().contains("does not match configuration"));

    let test_marked_handle = "https-pinned:governance:fenced-privacy-head-test";
    let test_marked = Arc::new(TestFencedTransparencyHeadReader::with_handle(
        target,
        test_marked_handle,
    ));
    let test_marked: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1> = test_marked;
    let error = QualifiedFencedTransparencyHeadReaderV1::try_new(
        test_marked_handle.to_owned(),
        test_fenced_head_reader_qualification(),
        test_marked,
    )
    .expect_err("test-marked reader must fail");
    assert!(error.to_string().contains("test-marked"));
}

#[test]
fn fused_writer_and_head_reader_require_one_exact_runtime_binding() {
    let target = Arc::new(TestFencedTransparencyPublisher::new());
    let writer = qualified_test_fenced_publisher(Arc::clone(&target));
    let cases = [
        (
            "hsm:governance:fenced-privacy-secondary",
            GovernanceDagRuntimeProviderQualificationV1::new(
                1,
                TEST_FENCED_PUBLISHER_POLICY_DIGEST,
            ),
        ),
        (
            TEST_FENCED_PUBLISHER_HANDLE,
            GovernanceDagRuntimeProviderQualificationV1::new(
                2,
                TEST_FENCED_PUBLISHER_POLICY_DIGEST,
            ),
        ),
        (
            TEST_FENCED_PUBLISHER_HANDLE,
            GovernanceDagRuntimeProviderQualificationV1::new(1, [0x74; 32]),
        ),
    ];

    for (handle, qualification) in cases {
        let reader = Arc::new(TestFencedTransparencyHeadReader::with_binding(
            Arc::clone(&target),
            handle,
            qualification.revision,
            qualification.policy_digest,
        ));
        let reader: Arc<dyn FencedTransparencyAuthoritativeHeadReaderV1> = reader;
        let reader = QualifiedFencedTransparencyHeadReaderV1::try_new(
            handle.to_owned(),
            qualification,
            reader,
        )
        .expect("independently qualify mismatched reader");
        let error = ensure_fenced_privacy_runtime_bindings_match(&writer, &reader)
            .expect_err("writer and reader binding mismatch must fail");
        assert!(error.to_string().contains("one exact identity"));
    }
}

#[test]
fn authenticated_head_bootstrap_rejects_read_failure_and_malformed_head_without_cache() {
    let failed_root = tempdir().expect("failed root");
    let failed_target = Arc::new(TestFencedTransparencyPublisher::new());
    let failed_reader = test_fenced_head_reader(failed_target);
    let qualified_failed_reader = qualified_test_fenced_head_reader(Arc::clone(&failed_reader));
    failed_reader.set_fail_read(true);
    let error = FilesystemGovernancePublisher::try_new(failed_root.path().to_path_buf())
        .expect("failed publisher")
        .with_qualified_fenced_privacy_head_reader(qualified_failed_reader)
        .expect_err("failed authenticated read must abort bootstrap");
    assert!(error.to_string().contains("failed authentication"));
    assert!(!fenced_privacy_head_sync_path(failed_root.path()).exists());

    let malformed_root = tempdir().expect("malformed root");
    let malformed_target = Arc::new(TestFencedTransparencyPublisher::new());
    let malformed_reader = test_fenced_head_reader(malformed_target);
    malformed_reader.override_head(Some(FencedTransparencyTargetHeadV1 {
        version: crate::FENCED_TRANSPARENCY_PUBLICATION_VERSION_V1,
        generation: 0,
        head_digest: [0xA1; 32],
        fencing_floor: 1,
    }));
    let error = FilesystemGovernancePublisher::try_new(malformed_root.path().to_path_buf())
        .expect("malformed publisher")
        .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(
            malformed_reader,
        ))
        .expect_err("malformed authoritative head must abort bootstrap");
    assert!(error.to_string().contains("failed authentication"));
    assert!(!fenced_privacy_head_sync_path(malformed_root.path()).exists());
}

#[test]
fn persisted_pending_and_head_sync_reject_qualified_target_rotation() {
    let temp = tempdir().expect("tempdir");
    let provider = Arc::new(TestFencedTransparencyPublisher::new());
    let publisher = qualified_test_fenced_publisher(Arc::clone(&provider));
    let reader = qualified_test_fenced_head_reader(test_fenced_head_reader(Arc::clone(&provider)));
    let (publication, encoded) = sample_privacy_publication();
    let request = sample_fenced_request(7, None);
    let mut pending = FencedPrivacyPendingRequestV1::from_request(&request, &publisher)
        .expect("build pending request");
    pending.target_handle = "hsm:governance:fenced-privacy-retired".to_owned();
    write_fenced_privacy_pending_request(temp.path(), &pending)
        .expect("persist old-target pending request");
    let restored = read_fenced_privacy_pending_request(temp.path())
        .expect("read pending request")
        .expect("pending request exists");

    let error = restored
        .reconstruct_request(request.authorization(), &publication, &encoded, &publisher)
        .expect_err("pending request must remain bound to its qualified target");
    assert!(
        error
            .to_string()
            .contains("belongs to a different qualified target")
    );

    let receipt = FencedPrivacyPublicationReceiptV1::from_verified_append(
        &request,
        TEST_FENCED_PUBLISHER_HANDLE,
        test_fenced_publisher_qualification(),
    )
    .expect("build verified cache receipt");
    let mut retired_cache = FencedPrivacyPublicationCacheV1::from_verified_receipt(
        &request,
        &receipt,
        Some(receipt.included_head()),
    )
    .expect("build verified publication cache");
    retired_cache.target_handle = "hsm:governance:fenced-privacy-retired".to_owned();
    write_fenced_privacy_head_cache(temp.path(), &retired_cache)
        .expect("persist retired target cache");
    let error = synchronize_fenced_privacy_authoritative_head(temp.path(), &reader, None)
        .expect_err("persisted publication cache must not rotate targets implicitly");
    assert!(
        error
            .to_string()
            .contains("publication cache belongs to a different qualified target")
    );
    fs::remove_file(fenced_privacy_head_cache_path(temp.path()))
        .expect("remove retired cache before reader-binding check");

    let retired_sync = FencedPrivacyAuthoritativeHeadSyncV1 {
        version: GOVERNANCE_FENCED_PRIVACY_HEAD_SYNC_VERSION_V1,
        reader_handle: "https-pinned:governance:fenced-privacy-retired".to_owned(),
        reader_revision: 1,
        reader_policy_digest: [0x73; 32],
        authoritative_head: None,
        ancestry_proof_digest: [0x74; 32],
    };
    write_fenced_privacy_head_sync(temp.path(), &retired_sync)
        .expect("persist retired reader binding");
    let error = synchronize_fenced_privacy_authoritative_head(temp.path(), &reader, None)
        .expect_err("persisted reader binding must not rotate implicitly");
    assert!(
        error
            .to_string()
            .contains("belongs to a different qualified reader")
    );
    assert_eq!(provider.append_count(), 0);
    assert!(provider.head().is_none());
}

#[test]
fn authenticated_head_sync_rejects_rollbacks_forks_and_stale_reader() {
    let temp = tempdir().expect("tempdir");
    let provider = Arc::new(TestFencedTransparencyPublisher::new());
    let qualified_writer = qualified_test_fenced_publisher(Arc::clone(&provider));
    let first_request = sample_fenced_request(7, None);
    let first_receipt = qualified_writer
        .compare_and_append_privacy_classified(&first_request)
        .expect("seed first authoritative head");
    let next_spec = SamplePrivacyReleaseSpec::next();
    let (next_publication, next_encoded) = sample_privacy_publication_for(next_spec);
    let next_authorization =
        sample_privacy_authorization_for(next_spec, &next_publication, &next_encoded, 8, None);
    let second_request = FencedPrivacyPublicationRequestV1::try_new(
        next_authorization,
        &next_publication,
        next_encoded,
        Some(first_receipt.included_head()),
        first_receipt.included_head().fencing_floor(),
    )
    .expect("second distinct fenced privacy request");
    let second_receipt = qualified_writer
        .compare_and_append_privacy_classified(&second_request)
        .expect("seed second authoritative head");
    let authoritative_head = second_receipt.included_head();
    let head_reader = test_fenced_head_reader(Arc::clone(&provider));
    let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("publisher")
        .with_qualified_fenced_privacy_publisher(qualified_writer)
        .expect("attach fused publisher")
        .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(Arc::clone(
            &head_reader,
        )))
        .expect("bootstrap current authoritative head");
    assert_eq!(
        read_fenced_privacy_head_sync(temp.path())
            .expect("read synchronized head")
            .and_then(|sync| sync.authoritative_head),
        Some(authoritative_head)
    );
    let (publication, encoded) = sample_privacy_publication();
    let authorization = sample_privacy_authorization(&publication, &encoded, 9);

    head_reader.override_head(Some(first_receipt.included_head()));
    let error = publisher
        .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
        .expect_err("generation rollback must fail");
    assert!(error.to_string().contains("failed authentication"));

    head_reader.override_head(None);
    let error = publisher
        .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
        .expect_err("genesis rollback must fail");
    assert!(error.to_string().contains("failed authentication"));

    head_reader.override_head(Some(
        FencedTransparencyTargetHeadV1::try_new(
            authoritative_head.generation(),
            [0xA2; 32],
            authoritative_head.fencing_floor(),
        )
        .expect("valid substituted head"),
    ));
    let error = publisher
        .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
        .expect_err("same-generation substitution must fail");
    assert!(error.to_string().contains("failed authentication"));

    head_reader.override_head(Some(
        FencedTransparencyTargetHeadV1::try_new(
            authoritative_head.generation() + 1,
            [0xA3; 32],
            authoritative_head.fencing_floor(),
        )
        .expect("structurally valid non-monotonic head"),
    ));
    let error = publisher
        .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
        .expect_err("unproven higher fork must fail");
    assert!(error.to_string().contains("failed authentication"));

    head_reader.override_head(Some(authoritative_head));
    head_reader.set_revision(2);
    let error = publisher
        .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
        .expect_err("stale reader qualification must fail");
    assert!(error.to_string().contains("changed after qualification"));

    assert_no_privacy_publication_side_effects(temp.path());
    assert!(!fenced_privacy_pending_path(temp.path()).exists());
    assert_eq!(
        read_fenced_privacy_head_sync(temp.path())
            .expect("read retained synchronized head")
            .and_then(|sync| sync.authoritative_head),
        Some(authoritative_head),
        "rejected reads must not roll back the authenticated cache"
    );
}

#[test]
fn authenticated_head_sync_rejects_publication_at_unrelated_valid_ancestor() {
    let temp = tempdir().expect("tempdir");
    let provider = Arc::new(TestFencedTransparencyPublisher::new());
    let writer = qualified_test_fenced_publisher(Arc::clone(&provider));
    let first_request = sample_fenced_request(7, None);
    let first_receipt = writer
        .compare_and_append_privacy_classified(&first_request)
        .expect("seed first release");

    let next_spec = SamplePrivacyReleaseSpec::next();
    let (next_publication, next_encoded) = sample_privacy_publication_for(next_spec);
    let next_authorization =
        sample_privacy_authorization_for(next_spec, &next_publication, &next_encoded, 8, None);
    let second_request = FencedPrivacyPublicationRequestV1::try_new(
        next_authorization,
        &next_publication,
        next_encoded,
        Some(first_receipt.included_head()),
        first_receipt.included_head().fencing_floor(),
    )
    .expect("second release request");
    let second_receipt = writer
        .compare_and_append_privacy_classified(&second_request)
        .expect("seed unrelated later release");

    let (publication, encoded) = sample_privacy_publication();
    let duplicate_authorization = sample_privacy_authorization(&publication, &encoded, 9);
    let duplicate_request = FencedPrivacyPublicationRequestV1::try_new(
        duplicate_authorization,
        &publication,
        encoded,
        Some(second_receipt.included_head()),
        second_receipt.included_head().fencing_floor(),
    )
    .expect("duplicate release lookup request");
    let forged_receipt = FencedPrivacyPublicationReceiptV1::from_verified_existing(
        &duplicate_request,
        TEST_FENCED_PUBLISHER_HANDLE,
        test_fenced_publisher_qualification(),
        second_receipt.included_head(),
        second_receipt.included_head(),
    )
    .expect("structurally valid receipt at an unrelated ancestor");
    let reader = qualified_test_fenced_head_reader(test_fenced_head_reader(Arc::clone(&provider)));

    let error =
        synchronize_fenced_privacy_authoritative_head(temp.path(), &reader, Some(&forged_receipt))
            .expect_err("ancestry alone must not prove a different publication identity");

    assert!(error.to_string().contains("failed authentication"));
    assert!(!fenced_privacy_head_sync_path(temp.path()).exists());
    assert_eq!(provider.append_count(), 2);
    assert_ne!(
        first_receipt.included_head(),
        second_receipt.included_head()
    );
}

#[test]
fn filesystem_privacy_publication_replays_cached_request_after_lease_rotation() {
    let temp = tempdir().expect("tempdir");
    let provider = Arc::new(TestFencedTransparencyPublisher::new());
    let head_reader = test_fenced_head_reader(Arc::clone(&provider));
    let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("publisher")
        .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(Arc::clone(
            &provider,
        )))
        .expect("attach fused publisher")
        .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(head_reader))
        .expect("attach authenticated head reader");
    let (publication, encoded) = sample_privacy_publication();
    let authorization = sample_privacy_authorization(&publication, &encoded, 8);
    let rotated_authorization = sample_privacy_authorization(&publication, &encoded, 9);

    publisher
        .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
        .expect("first filesystem publication");
    let first_cache = read_fenced_privacy_head_cache(temp.path())
        .expect("read first cache")
        .expect("first cache exists");
    publisher
        .publish_transparency_ledger_publication(
            &publication,
            &encoded,
            Some(&rotated_authorization),
        )
        .expect("filesystem exact retry after lease rotation");
    let retry_cache = read_fenced_privacy_head_cache(temp.path())
        .expect("read retry cache")
        .expect("retry cache exists");

    assert_eq!(retry_cache, first_cache);
    assert_eq!(retry_cache.last_fencing_token, 8);
    assert_eq!(retry_cache.authoritative_head.fencing_floor(), 8);
    assert_eq!(provider.append_count(), 1);
    assert_eq!(provider.head(), Some(retry_cache.authoritative_head));

    let conflicting_spec = SamplePrivacyReleaseSpec {
        release_record_digest: [0xB8; 32],
        ..SamplePrivacyReleaseSpec::primary()
    };
    let conflicting_authorization =
        sample_privacy_authorization_for(conflicting_spec, &publication, &encoded, 10, None);
    let error = publisher
        .publish_transparency_ledger_publication(
            &publication,
            &encoded,
            Some(&conflicting_authorization),
        )
        .expect_err("cached payload must not mask conflicting release evidence");
    assert!(
        error
            .to_string()
            .contains("identity conflicts with an existing publication")
    );
    assert_eq!(provider.append_count(), 1);
    assert!(!fenced_privacy_pending_path(temp.path()).exists());
    assert_eq!(
        read_fenced_privacy_head_cache(temp.path())
            .expect("read cache after conflict")
            .expect("cache survives conflict"),
        first_cache
    );
}

#[test]
fn filesystem_privacy_publication_without_fused_adapter_fails_before_side_effects() {
    let temp = tempdir().expect("tempdir");
    let publisher =
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
    let (publication, encoded) = sample_privacy_publication();
    let authorization = sample_privacy_authorization(&publication, &encoded, 8);

    let error = publisher
        .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
        .expect_err("privacy publication must require fused adapter");

    assert!(
        error
            .to_string()
            .contains("requires a qualified fused target publisher")
    );
    assert_no_privacy_publication_side_effects(temp.path());
    assert!(!fenced_privacy_pending_path(temp.path()).exists());
}

#[test]
fn fresh_filesystem_root_without_authenticated_head_reader_fails_closed() {
    let temp = tempdir().expect("tempdir");
    let provider = Arc::new(TestFencedTransparencyPublisher::new());
    let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("publisher")
        .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(provider))
        .expect("attach fused publisher");
    let (publication, encoded) = sample_privacy_publication();
    let authorization = sample_privacy_authorization(&publication, &encoded, 8);

    let error = publisher
        .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
        .expect_err("fresh root must not infer authoritative genesis");

    assert!(
        error
            .to_string()
            .contains("requires a qualified authenticated authoritative-head reader")
    );
    assert_no_privacy_publication_side_effects(temp.path());
    assert!(!fenced_privacy_head_sync_path(temp.path()).exists());
    assert!(!fenced_privacy_pending_path(temp.path()).exists());
}

#[test]
fn filesystem_privacy_publication_rejects_substituted_receipt_before_side_effects() {
    let temp = tempdir().expect("tempdir");
    let provider = Arc::new(TestFencedTransparencyPublisher::new());
    let head_reader = test_fenced_head_reader(Arc::clone(&provider));
    provider.set_substitute_receipt(true);
    let publisher = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("publisher")
        .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(Arc::clone(
            &provider,
        )))
        .expect("attach fused publisher")
        .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(head_reader))
        .expect("attach authenticated head reader");
    let (publication, encoded) = sample_privacy_publication();
    let authorization = sample_privacy_authorization(&publication, &encoded, 9);

    let error = publisher
        .publish_transparency_ledger_publication(&publication, &encoded, Some(&authorization))
        .expect_err("substituted receipt must fail closed");

    assert!(error.to_string().contains("publication receipt is invalid"));
    assert_eq!(provider.append_count(), 1);
    assert_no_privacy_publication_side_effects(temp.path());
    assert!(
        fenced_privacy_pending_path(temp.path()).exists(),
        "ambiguous append must retain its exact pending request"
    );

    provider.set_substitute_receipt(false);
    let rotated_authorization = sample_privacy_authorization(&publication, &encoded, 10);
    publisher
        .publish_transparency_ledger_publication(
            &publication,
            &encoded,
            Some(&rotated_authorization),
        )
        .expect("recover exact request after malformed receipt");

    assert_eq!(provider.append_count(), 1);
    assert!(!fenced_privacy_pending_path(temp.path()).exists());
    let cache = read_fenced_privacy_head_cache(temp.path())
        .expect("read recovered cache")
        .expect("recovered cache exists");
    assert_eq!(cache.last_fencing_token, 9);
    let index = read_publication_section_fixture(temp.path(), "publish_index");
    let labels = index
        .get("entries")
        .and_then(JsonValue::as_array)
        .and_then(|entries| entries.first())
        .and_then(JsonValue::as_object)
        .and_then(|entry| entry.get("labels"))
        .and_then(JsonValue::as_object)
        .expect("recovered privacy labels");
    assert_eq!(
        labels
            .get("leader_lease_fencing_token")
            .and_then(JsonValue::as_u64),
        Some(9)
    );
}

#[test]
fn fresh_roots_deduplicate_release_across_leases_and_later_heads() {
    let first_root = tempdir().expect("first tempdir");
    let same_lease_root = tempdir().expect("same-lease tempdir");
    let later_anchor_root = tempdir().expect("later-anchor tempdir");
    let provider = Arc::new(TestFencedTransparencyPublisher::new());
    let first_reader = test_fenced_head_reader(Arc::clone(&provider));
    let same_lease_reader = test_fenced_head_reader(Arc::clone(&provider));
    let first_publisher = FilesystemGovernancePublisher::try_new(first_root.path().to_path_buf())
        .expect("first publisher")
        .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(Arc::clone(
            &provider,
        )))
        .expect("attach first fused publisher")
        .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(first_reader))
        .expect("attach first authenticated head reader");
    let same_lease_publisher =
        FilesystemGovernancePublisher::try_new(same_lease_root.path().to_path_buf())
            .expect("same-lease publisher")
            .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(Arc::clone(
                &provider,
            )))
            .expect("attach same-lease fused publisher")
            .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(
                same_lease_reader,
            ))
            .expect("attach same-lease authenticated head reader");
    let (publication, encoded) = sample_privacy_publication();
    let first_authorization = sample_privacy_authorization(&publication, &encoded, 10);
    let same_lease_authorization = sample_privacy_authorization(&publication, &encoded, 10);
    first_publisher
        .publish_transparency_ledger_publication(&publication, &encoded, Some(&first_authorization))
        .expect("first root publishes from authenticated genesis");
    let first_head = provider.head().expect("first authoritative head");
    same_lease_publisher
        .publish_transparency_ledger_publication(
            &publication,
            &encoded,
            Some(&same_lease_authorization),
        )
        .expect("fresh root recognizes the same lease and stable release");

    assert_eq!(provider.append_count(), 1);
    assert_eq!(provider.head(), Some(first_head));
    assert_eq!(
        read_fenced_privacy_head_cache(first_root.path())
            .expect("first cached head")
            .map(|cache| cache.authoritative_head),
        Some(first_head)
    );
    let same_lease_cache = read_fenced_privacy_head_cache(same_lease_root.path())
        .expect("same-lease cached head")
        .expect("same-lease cache exists");
    assert_eq!(same_lease_cache.authoritative_head, first_head);
    assert_eq!(same_lease_cache.last_included_head, first_head);
    assert_eq!(
        same_lease_cache.last_disposition,
        FencedPrivacyPublicationDispositionV1::AlreadyIncluded
    );
    assert_eq!(same_lease_cache.last_fencing_token, 10);
    same_lease_publisher
        .publish_transparency_ledger_publication(
            &publication,
            &encoded,
            Some(&same_lease_authorization),
        )
        .expect("same fresh root replays its already-included cache");
    assert_eq!(provider.append_count(), 1);
    assert_eq!(
        read_fenced_privacy_head_cache(same_lease_root.path())
            .expect("same-root retry cached head")
            .expect("same-root retry cache exists")
            .last_disposition,
        FencedPrivacyPublicationDispositionV1::AlreadyIncluded
    );

    let next_spec = SamplePrivacyReleaseSpec::next();
    let (next_publication, next_encoded) = sample_privacy_publication_for(next_spec);
    let next_authorization =
        sample_privacy_authorization_for(next_spec, &next_publication, &next_encoded, 11, None);
    first_publisher
        .publish_transparency_ledger_publication(
            &next_publication,
            &next_encoded,
            Some(&next_authorization),
        )
        .expect("a genuinely distinct finalized release appends");
    let advanced_head = provider.head().expect("advanced authoritative head");
    assert_ne!(advanced_head, first_head);
    assert_eq!(provider.append_count(), 2);

    assert!(
        !first_root
            .path()
            .join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE)
            .exists()
    );
    assert!(
        !same_lease_root
            .path()
            .join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE)
            .exists()
    );

    let later_anchor_reader = test_fenced_head_reader(Arc::clone(&provider));
    let later_anchor_publisher =
        FilesystemGovernancePublisher::try_new(later_anchor_root.path().to_path_buf())
            .expect("later-anchor publisher")
            .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(Arc::clone(
                &provider,
            )))
            .expect("attach later-anchor fused publisher")
            .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(
                later_anchor_reader,
            ))
            .expect("bootstrap authoritative head");
    let advanced_block_hash = next_publication
        .block
        .block_hash()
        .expect("advanced publication block hash");
    let later_anchor_authorization = sample_privacy_authorization_for(
        SamplePrivacyReleaseSpec::primary(),
        &publication,
        &encoded,
        12,
        Some(SampleFinalizedAnchorSpec {
            sequence: next_spec.release_sequence,
            release_id: next_publication.block.cycle_id,
            record_digest: next_spec.release_record_digest,
            latest_publication_block_hash: Some(advanced_block_hash),
        }),
    );
    assert_eq!(
        first_authorization.publication_idempotency_digest(),
        later_anchor_authorization.publication_idempotency_digest(),
        "later finalized-head advancement must not change the release identity"
    );
    later_anchor_publisher
        .publish_transparency_ledger_publication(
            &publication,
            &encoded,
            Some(&later_anchor_authorization),
        )
        .expect("fresh root recognizes a release under a later finalized anchor");

    assert_eq!(provider.append_count(), 2);
    assert_eq!(provider.head(), Some(advanced_head));
    let later_cache = read_fenced_privacy_head_cache(later_anchor_root.path())
        .expect("later-anchor cached head")
        .expect("later-anchor cache exists");
    assert_eq!(later_cache.authoritative_head, advanced_head);
    assert_eq!(later_cache.last_included_head, first_head);
    assert_eq!(
        later_cache.last_disposition,
        FencedPrivacyPublicationDispositionV1::AlreadyIncluded
    );
    assert!(!fenced_privacy_pending_path(later_anchor_root.path()).exists());
}

#[test]
fn newer_fencing_token_wins_while_paused_predecessor_has_zero_side_effects() {
    let stale_root = tempdir().expect("stale tempdir");
    let winner_root = tempdir().expect("winner tempdir");
    let provider = Arc::new(TestFencedTransparencyPublisher::new());
    let stale_reader = test_fenced_head_reader(Arc::clone(&provider));
    let winner_reader = test_fenced_head_reader(Arc::clone(&provider));
    provider.pause_fencing_token(20);
    let stale_publisher = FilesystemGovernancePublisher::try_new(stale_root.path().to_path_buf())
        .expect("stale publisher")
        .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(Arc::clone(
            &provider,
        )))
        .expect("attach stale fused publisher")
        .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(stale_reader))
        .expect("attach stale authenticated head reader");
    let winner_publisher = FilesystemGovernancePublisher::try_new(winner_root.path().to_path_buf())
        .expect("winner publisher")
        .with_qualified_fenced_privacy_publisher(qualified_test_fenced_publisher(Arc::clone(
            &provider,
        )))
        .expect("attach winner fused publisher")
        .with_qualified_fenced_privacy_head_reader(qualified_test_fenced_head_reader(winner_reader))
        .expect("attach winner authenticated head reader");
    let (publication, encoded) = sample_privacy_publication();
    let stale_authorization = sample_privacy_authorization(&publication, &encoded, 20);
    let winner_spec = SamplePrivacyReleaseSpec::next();
    let (winner_publication, winner_encoded) = sample_privacy_publication_for(winner_spec);
    let winner_authorization = sample_privacy_authorization_for(
        winner_spec,
        &winner_publication,
        &winner_encoded,
        21,
        None,
    );
    let stale_publication = publication.clone();
    let stale_encoded = encoded.clone();
    let stale = thread::spawn(move || {
        stale_publisher.publish_transparency_ledger_publication(
            &stale_publication,
            &stale_encoded,
            Some(&stale_authorization),
        )
    });
    provider.wait_until_paused();

    let winner_result = winner_publisher.publish_transparency_ledger_publication(
        &winner_publication,
        &winner_encoded,
        Some(&winner_authorization),
    );
    provider.release_paused();
    winner_result.expect("newer fencing token wins");
    let stale_error = stale
        .join()
        .expect("stale publication thread")
        .expect_err("paused stale token must fail");

    assert!(stale_error.to_string().contains("fencing token is stale"));
    assert_eq!(provider.append_count(), 1);
    assert_no_privacy_publication_side_effects(stale_root.path());
    assert!(!fenced_privacy_pending_path(stale_root.path()).exists());
    assert_eq!(
        read_fenced_privacy_head_cache(winner_root.path())
            .expect("winner cached head")
            .map(|cache| cache.authoritative_head),
        provider.head()
    );
    assert!(
        !winner_root
            .path()
            .join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE)
            .exists()
    );
}

#[test]
fn filesystem_publisher_writes_transparency_ledger_publication_files_and_car_queue() {
    let temp = tempdir().expect("tempdir");
    let publisher =
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
    let (publication, encoded) = sample_transparency_ledger_publication();

    publisher
        .publish_transparency_ledger_publication(&publication, &encoded, None)
        .expect("publish transparency ledger publication");

    let (encoded_path, json_path) =
        only_published_source_paths(temp.path(), "transparency_ledger_publication");
    let bytes = fs::read(&encoded_path).expect("read transparency ledger payload");
    assert_eq!(bytes, encoded);
    let decoded: ModerationLedgerCyclePublicationV1 =
        norito::decode_from_bytes(&bytes).expect("decode transparency ledger publication");
    assert_eq!(decoded, publication);
    assert!(json_path.exists());

    let index = read_publication_section_fixture(temp.path(), "publish_index");
    assert_eq!(
        index
            .get("by_payload_kind")
            .and_then(|value| value.get("transparency_ledger_publication"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );
    let entry = index
        .get("entries")
        .and_then(JsonValue::as_array)
        .and_then(|entries| entries.first())
        .and_then(JsonValue::as_object)
        .expect("publish index entry");
    let labels = entry
        .get("labels")
        .and_then(JsonValue::as_object)
        .expect("publish labels");
    let expected_cycle_id = hex::encode(publication.block.cycle_id);
    assert_eq!(
        labels.get("cycle_id_hex").and_then(JsonValue::as_str),
        Some(expected_cycle_id.as_str())
    );
    assert_eq!(
        labels.get("entry_count").and_then(JsonValue::as_u64),
        Some(u64::from(publication.block.entry_count))
    );

    let queue = read_publication_section_fixture(temp.path(), "car_queue");
    assert_eq!(
        queue
            .get("by_payload_kind")
            .and_then(|value| value.get("transparency_ledger_publication"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        queue.get("assembled_count").and_then(JsonValue::as_u64),
        Some(1)
    );
}

#[test]
fn filesystem_publisher_writes_proof_token_issuance_files_and_car_queue() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());
    let (issuance, encoded) = sample_proof_token_issuance();

    publisher
        .publish_proof_token_issuance(&issuance, &encoded)
        .expect("publish proof-token issuance");

    let token_id_hex = hex::encode(issuance.token_id);
    let (encoded_path, json_path) =
        only_published_source_paths(temp.path(), "proof_token_issuance");
    let bytes = fs::read(&encoded_path).expect("read proof-token issuance payload");
    assert_eq!(bytes, encoded);
    let decoded: ProofTokenIssuanceV1 =
        norito::decode_from_bytes(&bytes).expect("decode proof-token issuance");
    assert_eq!(decoded, issuance);

    assert!(json_path.exists());
    let json_body = fs::read(&json_path).expect("read proof-token issuance json");
    let json_value: JsonValue = json::from_slice(&json_body).expect("issuance json");
    assert_eq!(
        json_value
            .get("metadata")
            .and_then(|value| value.get("token_id_hex"))
            .and_then(JsonValue::as_str),
        Some(token_id_hex.as_str())
    );

    let index = read_publication_section_fixture(temp.path(), "publish_index");
    assert_eq!(
        index
            .get("by_payload_kind")
            .and_then(|value| value.get("proof_token_issuance"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );
    let entry = index
        .get("entries")
        .and_then(JsonValue::as_array)
        .and_then(|entries| entries.first())
        .and_then(JsonValue::as_object)
        .expect("publish index entry");
    let labels = entry
        .get("labels")
        .and_then(JsonValue::as_object)
        .expect("publish labels");
    assert_eq!(
        labels.get("token_id_hex").and_then(JsonValue::as_str),
        Some(token_id_hex.as_str())
    );
    assert_eq!(
        labels.get("entry_count").and_then(JsonValue::as_u64),
        Some(2)
    );
    assert_single_runtime_external(temp.path(), "proof_token_issuance", &encoded);

    let queue = read_publication_section_fixture(temp.path(), "car_queue");
    assert_eq!(
        queue
            .get("by_payload_kind")
            .and_then(|value| value.get("proof_token_issuance"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        queue.get("assembled_count").and_then(JsonValue::as_u64),
        Some(1)
    );
}

#[test]
fn filesystem_publisher_writes_appeal_finance_report_files_and_runtime_dag() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());
    let (report, encoded) = sample_appeal_finance_report();

    publisher
        .publish_appeal_finance_report(&report, &encoded)
        .expect("publish appeal finance report");

    let (encoded_path, json_path) =
        only_published_source_paths(temp.path(), "appeal_finance_report");
    let bytes = fs::read(&encoded_path).expect("read appeal finance report payload");
    assert_eq!(bytes, encoded);
    let decoded: SoraFsAppealFinanceReportV1 =
        norito::decode_from_bytes(&bytes).expect("decode appeal finance report");
    assert_eq!(decoded, report);
    assert!(json_path.exists());

    let index = read_publication_section_fixture(temp.path(), "publish_index");
    assert_eq!(
        index
            .get("by_payload_kind")
            .and_then(|value| value.get("appeal_finance_report"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );

    let runtime_index = runtime_index(temp.path());
    assert_eq!(
        runtime_index
            .get("by_payload_kind")
            .and_then(|value| value.get("appeal_finance_report"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );
    let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
    let head: GovernanceDagHeadV1 =
        norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
    let blocks = runtime_blocks_from_index(temp.path(), &runtime_index);
    validate_governance_dag_head_against_chain_v1(&head, &blocks)
        .expect("runtime head validates against signed blocks");
    assert_eq!(blocks.len(), 1);
    match &blocks[0].node.payload {
        GovernanceLogPayloadV1::AppealFinanceReport(value) => {
            assert_eq!(value.report_id, report.report_id);
            assert_eq!(value.case_id, report.case_id);
            assert_eq!(value.outcome, report.outcome);
        }
        other => panic!("unexpected runtime DAG payload: {other:?}"),
    }
}

#[test]
fn signed_runtime_dag_rejects_missing_authenticated_submission_provenance_before_writes() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());
    let (report, encoded) = sample_appeal_finance_report();
    let payload = GovernanceLogPayloadV1::AppealFinanceReport(report);

    let error = publisher
        .preflight_runtime_signed_payload_with_provenance(&payload, encoded.len(), None)
        .expect_err("signed caller-supplied payload must retain authenticated provenance");
    assert!(
        error
            .to_string()
            .contains("requires authenticated submission provenance")
    );
    assert!(
        !temp
            .path()
            .join(GOVERNANCE_PUBLICATION_SOURCES_DIR)
            .exists()
    );
    assert_empty_publication_authority(temp.path());
    assert!(!temp.path().join(GOVERNANCE_RUNTIME_DAG_DIR).exists());
}

#[test]
fn authenticated_submission_identity_participates_in_publication_idempotency() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());
    let (report, encoded) = sample_appeal_finance_report();
    let first =
        test_submission_provenance(crate::GovernanceSubmissionOriginV1::AppealFinanceReport);
    let other_key = PublicKey::from_bytes(Algorithm::Ed25519, &[0xA6; 32])
        .expect("fixed second publisher key must be valid");
    let second = GovernanceSubmissionProvenanceV1::new(
        AccountId::new(other_key),
        crate::GovernanceSubmissionOriginV1::AppealFinanceReport,
    );

    for provenance in [&first, &second] {
        <FilesystemGovernancePublisher as GovernancePublisher>::publish_appeal_finance_report(
            &publisher, &report, &encoded, provenance,
        )
        .expect("distinct authenticated publisher is a distinct attestation");
    }

    let publish_index = read_publication_section_fixture(temp.path(), "publish_index");
    assert_eq!(
        publish_index
            .get("entries")
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(2)
    );

    let runtime_index = runtime_index(temp.path());
    let blocks = runtime_blocks_from_index(temp.path(), &runtime_index);
    assert_eq!(blocks.len(), 2);
    assert_ne!(
        blocks[0].node.submission_provenance,
        blocks[1].node.submission_provenance
    );
    assert_ne!(blocks[0].node.node_cid, blocks[1].node.node_cid);
}

#[test]
fn filesystem_publisher_writes_appeal_finance_weekly_rollup_files_and_runtime_dag() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());
    let (rollup, encoded) = sample_appeal_finance_weekly_rollup();

    publisher
        .publish_appeal_finance_weekly_rollup(&rollup, &encoded)
        .expect("publish appeal finance weekly rollup");

    let (encoded_path, json_path) =
        only_published_source_paths(temp.path(), "appeal_finance_weekly_rollup");
    let bytes = fs::read(&encoded_path).expect("read appeal finance weekly rollup payload");
    assert_eq!(bytes, encoded);
    let decoded: SoraFsAppealFinanceWeeklyRollupV1 =
        norito::decode_from_bytes(&bytes).expect("decode appeal finance weekly rollup");
    assert_eq!(decoded, rollup);
    assert!(json_path.exists());
    let json_body = fs::read(&json_path).expect("read appeal finance weekly rollup json");
    let json_value: JsonValue = json::from_slice(&json_body).expect("weekly rollup json");
    assert_eq!(
        json_value
            .get("metadata")
            .and_then(|value| value.get("cycle"))
            .and_then(JsonValue::as_str),
        Some("2026-W26")
    );

    let index = read_publication_section_fixture(temp.path(), "publish_index");
    assert_eq!(
        index
            .get("by_payload_kind")
            .and_then(|value| value.get("appeal_finance_weekly_rollup"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );

    let runtime_index = runtime_index(temp.path());
    assert_eq!(
        runtime_index
            .get("by_payload_kind")
            .and_then(|value| value.get("appeal_finance_weekly_rollup"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );
    let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
    let head: GovernanceDagHeadV1 =
        norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
    let blocks = runtime_blocks_from_index(temp.path(), &runtime_index);
    validate_governance_dag_head_against_chain_v1(&head, &blocks)
        .expect("runtime head validates against signed blocks");
    assert_eq!(blocks.len(), 1);
    match &blocks[0].node.payload {
        GovernanceLogPayloadV1::AppealFinanceWeeklyRollup(value) => {
            assert_eq!(value.cycle, rollup.cycle);
            assert_eq!(value.report_count, rollup.report_count);
            assert_eq!(value.total_deposit_xor, rollup.total_deposit_xor);
        }
        other => panic!("unexpected runtime DAG payload: {other:?}"),
    }
}

#[test]
fn appeal_finance_settlement_receipt_source_identity_binds_finalized_cursor() {
    let (receipt, encoded) = sample_appeal_finance_settlement_receipt();
    let source_identity = |receipt: &SoraFsAppealFinanceSettlementReceiptV1, encoded: &[u8]| {
        let encoded_blake3 = blake3::hash(encoded).to_hex().to_string();
        let json = appeal_finance_settlement_receipt_json(receipt, encoded, &encoded_blake3)
            .expect("encode receipt JSON");
        governance_source_pair_relative_paths(
            "appeal_finance_settlement_receipt",
            u64::try_from(encoded.len()).expect("encoded length"),
            &encoded_blake3,
            u64::try_from(json.len()).expect("JSON length"),
            &blake3::hash(json.as_bytes()).to_hex().to_string(),
        )
        .expect("derive composite source identity")
    };
    let path = source_identity(&receipt, &encoded);

    let mut changed_height = receipt.clone();
    changed_height.finalized_block_height += 1;
    let changed_height_encoded =
        norito::to_bytes(&changed_height).expect("encode changed-height receipt");
    let changed_height_path = source_identity(&changed_height, &changed_height_encoded);
    assert_ne!(changed_height_path, path);

    let mut changed_hash = receipt;
    changed_hash.finalized_block_hash[0] ^= 0x01;
    let changed_hash_encoded =
        norito::to_bytes(&changed_hash).expect("encode changed-hash receipt");
    let changed_hash_path = source_identity(&changed_hash, &changed_hash_encoded);
    assert_ne!(changed_hash_path, path);
}

#[test]
fn filesystem_publisher_writes_appeal_finance_settlement_receipt_files_and_runtime_dag() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());
    let (receipt, encoded) = sample_appeal_finance_settlement_receipt();

    publisher
        .publish_appeal_finance_settlement_receipt(&receipt, &encoded)
        .expect("publish appeal finance settlement receipt");

    let (encoded_path, json_path) =
        only_published_source_paths(temp.path(), "appeal_finance_settlement_receipt");
    let bytes = fs::read(&encoded_path).expect("read settlement receipt payload");
    assert_eq!(bytes, encoded);
    let decoded: SoraFsAppealFinanceSettlementReceiptV1 =
        norito::decode_from_bytes(&bytes).expect("decode settlement receipt");
    assert_eq!(decoded, receipt);
    assert!(json_path.exists());
    let json_body = fs::read(&json_path).expect("read settlement receipt json");
    let json_value: JsonValue = json::from_slice(&json_body).expect("receipt json");
    let expected_policy_digest_hex = hex::encode(receipt.appeal_finance_policy_digest);
    let expected_finalized_block_hash_hex = hex::encode(receipt.finalized_block_hash);
    assert_eq!(
        json_value
            .get("metadata")
            .and_then(|value| value.get("tx_hash_hex"))
            .and_then(JsonValue::as_str),
        Some(receipt.tx_hash_hex.as_str())
    );
    assert_eq!(
        json_value
            .get("metadata")
            .and_then(|value| value.get("appeal_finance_policy_digest_hex"))
            .and_then(JsonValue::as_str),
        Some(expected_policy_digest_hex.as_str())
    );
    assert_eq!(
        json_value
            .get("metadata")
            .and_then(|value| value.get("finalized_block_height"))
            .and_then(JsonValue::as_u64),
        Some(receipt.finalized_block_height)
    );
    assert_eq!(
        json_value
            .get("metadata")
            .and_then(|value| value.get("finalized_block_hash_hex"))
            .and_then(JsonValue::as_str),
        Some(expected_finalized_block_hash_hex.as_str())
    );

    let index = read_publication_section_fixture(temp.path(), "publish_index");
    assert_eq!(
        index
            .get("by_payload_kind")
            .and_then(|value| value.get("appeal_finance_settlement_receipt"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        index
            .get("entries")
            .and_then(JsonValue::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("labels"))
            .and_then(|labels| labels.get("appeal_finance_policy_digest_hex"))
            .and_then(JsonValue::as_str),
        Some(expected_policy_digest_hex.as_str())
    );
    assert_eq!(
        index
            .get("entries")
            .and_then(JsonValue::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("labels"))
            .and_then(|labels| labels.get("finalized_block_height"))
            .and_then(JsonValue::as_u64),
        Some(receipt.finalized_block_height)
    );
    assert_eq!(
        index
            .get("entries")
            .and_then(JsonValue::as_array)
            .and_then(|entries| entries.first())
            .and_then(|entry| entry.get("labels"))
            .and_then(|labels| labels.get("finalized_block_hash_hex"))
            .and_then(JsonValue::as_str),
        Some(expected_finalized_block_hash_hex.as_str())
    );

    let runtime_index = runtime_index(temp.path());
    assert_eq!(
        runtime_index
            .get("by_payload_kind")
            .and_then(|value| value.get("appeal_finance_settlement_receipt"))
            .and_then(JsonValue::as_array)
            .map(Vec::len),
        Some(1)
    );
    let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
    let head: GovernanceDagHeadV1 =
        norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
    let blocks = runtime_blocks_from_index(temp.path(), &runtime_index);
    validate_governance_dag_head_against_chain_v1(&head, &blocks)
        .expect("runtime head validates against signed blocks");
    assert_eq!(blocks.len(), 1);
    match &blocks[0].node.payload {
        GovernanceLogPayloadV1::AppealFinanceSettlementReceipt(value) => {
            assert_eq!(value.receipt_id, receipt.receipt_id);
            assert_eq!(value.case_id, receipt.case_id);
            assert_eq!(value.submitted_step, receipt.submitted_step);
            assert_eq!(value.finalized_block_height, receipt.finalized_block_height);
            assert_eq!(value.finalized_block_hash, receipt.finalized_block_hash);
        }
        other => panic!("unexpected runtime DAG payload: {other:?}"),
    }
}

// Textual inclusion preserves the original governance test-module paths.
