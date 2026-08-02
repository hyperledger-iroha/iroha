#[test]
fn canonical_decode_rejects_trailing_and_compressed_bytes() {
    let source = signed_source(1, 0x31, 1_800_000_000);
    let block = &source.blocks[0];
    let decoded_block: GovernanceDagBlockV1 =
        decode_canonical(&block.bytes, "governance DAG block")
            .expect("a valid signed governance block fits the bounded decoder budget");
    assert_eq!(decoded_block, block.block);
    let checkpoint = checkpoint_from_source(&source);
    let canonical = norito::to_bytes(&checkpoint).expect("encode checkpoint body");
    let decoded: CheckpointBodyV1 =
        decode_canonical(&canonical, "checkpoint").expect("canonical bytes accepted");
    assert_eq!(decoded, checkpoint);

    let mut trailing = canonical.clone();
    trailing.push(0);
    assert!(decode_canonical::<CheckpointBodyV1>(&trailing, "checkpoint").is_err());

    let compressed =
        norito::to_compressed_bytes(&checkpoint, Some(norito::CompressionConfig::default()))
            .expect("compress checkpoint body");
    assert_ne!(compressed, canonical);
    assert!(decode_canonical::<CheckpointBodyV1>(&compressed, "checkpoint").is_err());
}

#[test]
fn bounded_norito_decode_rejects_sequence_allocation_bomb() {
    let encoded = norito::to_bytes(&vec![7_u64; 64]).expect("encode bounded vector");
    let limits = DecodeLimits::new(4, encoded.len(), 8, encoded.len() * 2, 16);
    assert!(norito::decode_from_bytes_with_limits::<Vec<u64>>(&encoded, limits).is_err());
}

#[test]
fn expected_signer_rejects_wrong_key_and_peer() {
    let source = signed_source(1, 0x32, 1_800_000_000);
    let block = &source.blocks[0].block;
    let attacker = TestSigner::new(0x33);
    assert!(
        validate_expected_signer(block, &attacker.public_key, &block.publisher_peer_id,).is_err()
    );
    let mut expected_key = [0_u8; 32];
    expected_key.copy_from_slice(&block.block_signature.public_key);
    assert!(validate_expected_signer(block, &expected_key, b"wrong-peer").is_err());
}

#[test]
fn runtime_handle_uses_central_production_grammar() {
    assert_eq!(
        validate_runtime_handle(
            "kms://governance/checkpoint.primary-v1_slot-a",
            "sealed checkpoint store",
        )
        .expect("canonical production runtime handle"),
        "kms://governance/checkpoint.primary-v1_slot-a"
    );
    for handle in [
        "https://operator:secret@checkpoint",
        "https://checkpoint/path?credential=secret",
        "https://checkpoint/path#fragment",
        "kms://governance/%63heckpoint",
        "kms:\\governance\\checkpoint",
    ] {
        let error = validate_runtime_handle(handle, "sealed checkpoint store")
            .expect_err("forbidden runtime-handle character must fail closed");
        assert!(error.to_string().contains("canonical credential-free"));
    }
    let error =
        validate_runtime_handle("kms:governance/checkpoint:dummy", "sealed checkpoint store")
            .expect_err("dummy-marked provider handle must fail closed");
    assert!(error.to_string().contains("test-marked"));
}

#[test]
fn service_validates_full_history_and_canonical_checkpoint_tail() {
    let source = signed_source(
        GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 + 1,
        0x73,
        1_800_000_000,
    );
    let blocks = source
        .blocks
        .iter()
        .map(|block| block.block.clone())
        .collect::<Vec<_>>();
    let tail = &blocks[blocks.len() - GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1..];

    assert_eq!(source.head.checkpoint_cid, Some(tail[0].block_cid.clone()));
    assert_eq!(tail[0].sequence, 1);
    assert_eq!(
        tail[0].prev_block_cid,
        Some(blocks[0].block_cid.clone()),
        "the canonical tail may retain a parent outside the checkpoint window"
    );
    assert_eq!(tail[0].node.prev_cid, Some(blocks[0].node.node_cid.clone()));
    validate_source_head_chain(&source.head, &blocks)
        .expect("service accepts and validates the complete root history");
    validate_source_head_chain(&source.head, tail)
        .expect("service accepts the canonical signed checkpoint tail");

    let governed_public_key = &source.head.head_signature.public_key;
    for block in &blocks {
        assert_eq!(block.publisher_peer_id, source.head.publisher_peer_id);
        assert_eq!(block.node.publisher_peer_id, source.head.publisher_peer_id);
        assert_eq!(&block.block_signature.public_key, governed_public_key);
        assert_eq!(
            &block.node.publisher_signature.public_key,
            governed_public_key
        );
    }
}

#[test]
fn service_rejects_checkpoint_tail_signature_and_continuity_drift() {
    let source = signed_source(
        GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 + 1,
        0x74,
        1_800_000_000,
    );
    let tail_start = source.blocks.len() - GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1;
    let canonical_tail = source.blocks[tail_start..]
        .iter()
        .map(|block| block.block.clone())
        .collect::<Vec<_>>();

    let attacker = TestSigner::new(0x75);
    let mut wrong_head_identity = source.head.clone();
    wrong_head_identity.head_signature = attacker.sign(
        &wrong_head_identity
            .signature_payload_bytes()
            .expect("encode attacker head payload"),
    );
    assert!(
        validate_source_head_chain(&wrong_head_identity, &canonical_tail).is_err(),
        "a byte-valid head signature from another identity must fail closed"
    );

    let mut wrong_identity = canonical_tail.clone();
    wrong_identity[0].block_signature = attacker.sign(
        &wrong_identity[0]
            .signature_payload_bytes()
            .expect("encode attacker block payload"),
    );
    assert!(
        validate_source_head_chain(&source.head, &wrong_identity).is_err(),
        "a byte-valid block signature from another identity must fail closed"
    );

    let governed = TestSigner::new(0x74);
    let mut broken_continuity = canonical_tail;
    broken_continuity[1].prev_block_cid = Some(vec![0xA5; 32]);
    broken_continuity[1].block_cid = broken_continuity[1]
        .recompute_block_cid()
        .expect("recompute continuity-drift block CID");
    broken_continuity[1].block_signature = governed.sign(
        &broken_continuity[1]
            .signature_payload_bytes()
            .expect("encode continuity-drift block payload"),
    );
    assert!(
        validate_source_head_chain(&source.head, &broken_continuity).is_err(),
        "a re-signed internal parent discontinuity must fail closed"
    );
}

#[test]
fn source_loader_accepts_checkpointed_full_history_from_real_publisher() {
    let root = secure_temp_dir();
    let source_dir = root.path().join("source");
    let publisher_peer_id = TEST_PRODUCER_PEER_ID.as_bytes().to_vec();
    let producer_signer_handle = "pkcs11:governance-dag:source-primary";
    let signer = Arc::new(PublisherTestSigner {
        handle: producer_signer_handle.to_owned(),
        peer_id: publisher_peer_id.clone(),
        signer: TestSigner::new(0x76),
    });
    let signer = qualify_governance_dag_runtime_signer_provider(
        signer.handle().to_owned(),
        publisher_peer_id,
        signer.public_key(),
        GovernanceDagRuntimeProviderQualificationV1::new(1, [0x83; 32]),
        signer,
    )
    .expect("qualify real runtime DAG signer");
    let checkpoint_store = Arc::new(TestSealedStore::new(
        "kms:governance-dag:source-producer-checkpoint",
    ));
    let checkpoint_store = qualify_governance_dag_runtime_checkpoint_store(
        checkpoint_store.handle().to_owned(),
        GovernanceDagRuntimeProviderQualificationV1::new(1, [0x82; 32]),
        checkpoint_store,
    )
    .expect("qualify real runtime DAG producer checkpoint store");
    let publisher = FilesystemGovernancePublisher::try_new(source_dir.clone())
        .expect("create real filesystem governance publisher")
        .with_qualified_runtime_dag_providers(signer, checkpoint_store)
        .expect("configure real runtime DAG providers");
    let timestamp = current_unix_timestamp_seconds();
    for sequence in 0..=GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 as u64 {
        let settlement = settlement(sequence, timestamp);
        let encoded = norito::to_bytes(&settlement).expect("encode source settlement");
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish source settlement");
    }
    drop(publisher);

    let signer = TestSigner::new(0x76);
    let state_dir = root.path().join("state");
    fs::create_dir_all(&state_dir).expect("create source-loader state root");
    let config = RuntimeConfig {
        source_root_guard: GovernanceFilesystemRootGuard::capture_source(&source_dir)
            .expect("fence real publisher source root"),
        source_dir,
        state_root_guard: GovernanceFilesystemRootGuard::capture_writer(&state_dir)
            .expect("fence source-loader state root"),
        listen_addr: "127.0.0.1:0".parse().expect("test address"),
        poll_interval: Duration::from_millis(10),
        max_response_bytes: 1024 * 1024,
        max_request_bytes: 1024 * 1024,
        mirror_max_entries: 1024,
        mirror_max_bytes: 1024 * 1024,
        max_head_age_secs: 3600,
        max_future_skew_secs: 60,
        allow_head_bootstrap: true,
        expected_producer_signer_handle: producer_signer_handle.to_owned(),
        expected_producer_signer_qualification: TEST_PRODUCER_SIGNER_QUALIFICATION,
        expected_publisher_peer_id: TEST_PRODUCER_PEER_ID.as_bytes().to_vec(),
        expected_public_key: signer.public_key,
    };

    let loaded = load_source_snapshot(&config)
        .expect("service loads and revalidates checkpointed full source history");
    assert_eq!(
        loaded.blocks.len(),
        GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1 + 1
    );
    assert_eq!(
        loaded.head.checkpoint_cid,
        Some(
            loaded.blocks[loaded.blocks.len() - GOVERNANCE_DAG_CHECKPOINT_WINDOW_BLOCKS_V1]
                .block
                .block_cid
                .clone()
        )
    );

    let index_path = config.source_dir.join("runtime-dag-index.json");
    let mut index: JsonValue =
        json::from_slice(&fs::read(&index_path).expect("read publisher runtime index"))
            .expect("decode publisher runtime index");
    let first_entry = index
        .get_mut("blocks")
        .and_then(JsonValue::as_array_mut)
        .and_then(|blocks| blocks.first_mut())
        .and_then(JsonValue::as_object_mut)
        .expect("first publisher runtime index entry");
    let source_payload_path = first_entry
        .get("encoded_path")
        .and_then(JsonValue::as_str)
        .expect("source payload path")
        .to_owned();
    let substituted = settlement(999, timestamp);
    let substituted_bytes =
        norito::to_bytes(&substituted).expect("encode substituted source payload");
    write_test_sidecar_file(
        &config.source_dir.join(source_payload_path),
        &substituted_bytes,
    );
    first_entry.insert(
        "source_payload_len".into(),
        JsonValue::from(
            u64::try_from(substituted_bytes.len()).expect("test source payload length fits u64"),
        ),
    );
    first_entry.insert(
        "source_payload_blake3".into(),
        JsonValue::from(hex::encode(blake3_array(&substituted_bytes))),
    );
    let tampered_index = json::to_json_pretty(&index)
        .expect("encode substituted runtime index")
        .into_bytes();
    write_test_sidecar_file(&index_path, &tampered_index);
    let error = load_source_snapshot(&config)
        .expect_err("source payload substitution must not escape the signed node binding");
    assert!(
        error
            .to_string()
            .contains("source payload does not match its signed governance node"),
        "unexpected source substitution error: {error}"
    );
}

#[test]
fn committed_source_loader_authenticates_distinct_signing_key_segments() {
    let root = secure_temp_dir();
    let source_dir = root.path().join("source");
    let publisher_peer_id = TEST_PRODUCER_PEER_ID.as_bytes().to_vec();
    let checkpoint_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let producer_store = qualify_governance_dag_runtime_checkpoint_store(
        TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
        TEST_STORE_QUALIFICATION,
        checkpoint_provider.clone(),
    )
    .expect("qualify producer checkpoint store");

    let outgoing_provider = Arc::new(PublisherTestSigner {
        handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
        peer_id: publisher_peer_id.clone(),
        signer: TestSigner::new(0x76),
    });
    let outgoing_public_key = outgoing_provider.public_key();
    let outgoing_signer = qualify_governance_dag_runtime_signer_provider(
        TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
        publisher_peer_id.clone(),
        outgoing_public_key,
        TEST_PRODUCER_SIGNER_QUALIFICATION,
        outgoing_provider,
    )
    .expect("qualify outgoing producer signer");
    let mut publisher = FilesystemGovernancePublisher::try_new(source_dir.clone())
        .expect("create segmented filesystem governance publisher")
        .with_qualified_runtime_dag_providers(outgoing_signer, producer_store)
        .expect("configure outgoing producer providers");
    let timestamp = current_unix_timestamp_seconds();
    let outgoing_settlement = settlement(0, timestamp);
    let outgoing_encoded =
        norito::to_bytes(&outgoing_settlement).expect("encode outgoing settlement");
    publisher
        .publish_deal_settlement(&outgoing_settlement, &outgoing_encoded)
        .expect("publish outgoing authority block");

    let incoming_provider = Arc::new(PublisherTestSigner {
        handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
        peer_id: publisher_peer_id.clone(),
        signer: TestSigner::new(0x77),
    });
    let incoming_public_key = incoming_provider.public_key();
    assert_ne!(outgoing_public_key, incoming_public_key);
    let incoming_signer = qualify_governance_dag_runtime_signer_provider(
        TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
        publisher_peer_id.clone(),
        incoming_public_key,
        TEST_PRODUCER_SIGNER_QUALIFICATION,
        incoming_provider,
    )
    .expect("qualify incoming producer signer");
    let incoming_store = qualify_governance_dag_runtime_checkpoint_store(
        TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
        TEST_STORE_QUALIFICATION,
        checkpoint_provider.clone(),
    )
    .expect("qualify incoming producer checkpoint store");
    publisher
        .transition_qualified_runtime_dag_providers(incoming_signer, incoming_store)
        .expect("install dual-signed key transition");

    let state_dir = root.path().join("state");
    fs::create_dir_all(&state_dir).expect("create segmented source-loader state root");
    let config = RuntimeConfig {
        source_root_guard: GovernanceFilesystemRootGuard::capture_source(&source_dir)
            .expect("fence segmented publisher source root"),
        source_dir,
        state_root_guard: GovernanceFilesystemRootGuard::capture_writer(&state_dir)
            .expect("fence segmented source-loader state root"),
        listen_addr: "127.0.0.1:0".parse().expect("test address"),
        poll_interval: Duration::from_millis(10),
        max_response_bytes: 1024 * 1024,
        max_request_bytes: 1024 * 1024,
        mirror_max_entries: 1024,
        mirror_max_bytes: 1024 * 1024,
        max_head_age_secs: 3600,
        max_future_skew_secs: 60,
        allow_head_bootstrap: true,
        expected_producer_signer_handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
        expected_producer_signer_qualification: TEST_PRODUCER_SIGNER_QUALIFICATION,
        expected_publisher_peer_id: publisher_peer_id,
        expected_public_key: incoming_public_key,
    };
    let service_store = test_checkpoint_store(Arc::clone(&checkpoint_provider));
    let rotated_without_append = load_committed_source_snapshot(&config, &service_store)
        .expect("incoming binding authenticates the outgoing-signed retained tip");
    assert_eq!(rotated_without_append.blocks.len(), 1);
    assert_eq!(
        rotated_without_append.head.head_signature.public_key,
        outgoing_public_key.to_vec()
    );

    let incoming_settlement = settlement(1, timestamp.saturating_add(1));
    let incoming_encoded =
        norito::to_bytes(&incoming_settlement).expect("encode incoming settlement");
    publisher
        .publish_deal_settlement(&incoming_settlement, &incoming_encoded)
        .expect("publish incoming authority block");
    let segmented = load_committed_source_snapshot(&config, &service_store)
        .expect("service readback authenticates both signing-key segments");
    assert_eq!(segmented.blocks.len(), 2);
    assert_eq!(
        segmented.blocks[0].block.block_signature.public_key,
        outgoing_public_key.to_vec()
    );
    assert_eq!(
        segmented.blocks[1].block.block_signature.public_key,
        incoming_public_key.to_vec()
    );
    assert_eq!(
        segmented.head.head_signature.public_key,
        incoming_public_key.to_vec()
    );

    let mut sealed = checkpoint_provider
        .inner
        .lock()
        .expect("lock segmented producer checkpoint");
    let current_record = sealed
        .producer_checkpoint
        .as_ref()
        .expect("segmented producer checkpoint")
        .clone();
    let mut substituted_checkpoint: RuntimeDagProducerCheckpointV1 =
        norito::decode_from_bytes(&current_record.payload)
            .expect("decode segmented producer checkpoint");
    substituted_checkpoint.qualification_transition_digest[0] ^= 0x80;
    sealed.producer_checkpoint = Some(GovernanceDagSealedStateRecord::new(
        GovernanceDagSealedStateSlot::ProducerCheckpoint,
        current_record.generation,
        norito::to_bytes(&substituted_checkpoint)
            .expect("encode substituted segmented producer checkpoint"),
    ));
    drop(sealed);
    let error = load_committed_source_snapshot(&config, &service_store)
        .expect_err("sealed key-transition lineage substitution must fail closed");
    assert!(
        error.to_string().contains("authority lineage diverges"),
        "unexpected sealed lineage substitution error: {error}"
    );
}

#[test]
fn checkpoint_rejects_rollback_and_fork() {
    let original = signed_source(3, 0x34, 1_800_000_000);
    let checkpoint = checkpoint_from_source(&original);
    let rolled_back = signed_source(2, 0x34, 1_800_000_000);
    assert!(validate_checkpoint_against_source(Some(&checkpoint), &rolled_back).is_err());

    let fork = signed_source(3, 0x34, 1_800_000_100);
    assert!(validate_checkpoint_against_source(Some(&checkpoint), &fork).is_err());
}

#[test]
fn producer_commit_guard_binds_the_exact_verified_source_index() {
    let root = secure_temp_dir();
    let source_dir = root.path().join("source");
    let mut source = signed_source(2, 0x74, 1_800_000_000);
    materialize_source_snapshot(&source_dir, &mut source);
    let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    seed_producer_checkpoint(&provider, &source_dir, &source);
    let store = test_checkpoint_store(provider.clone());
    let config = test_runtime_config(&source, root.path());
    let loaded = load_committed_source_snapshot(&config, &store)
        .expect("stable sealed producer checkpoint admits the exact source snapshot");
    assert_eq!(loaded.index_blake3, source.index_blake3);

    let mut checkpoint = producer_checkpoint_from_source(&source_dir, &source);
    checkpoint.index_bytes_digest[0] ^= 0x80;
    let replacement = GovernanceDagSealedStateRecord::new(
        GovernanceDagSealedStateSlot::ProducerCheckpoint,
        checkpoint.block_count.saturating_add(1),
        norito::to_bytes(&checkpoint).expect("encode tampered producer checkpoint"),
    );
    provider
        .inner
        .lock()
        .expect("lock test producer store")
        .producer_checkpoint = Some(replacement);
    let error = load_committed_source_snapshot(&config, &store)
        .expect_err("mismatched sealed producer index digest must fail closed");
    assert!(error.to_string().contains("does not match"));
}

#[test]
fn service_checkpoint_and_intent_bind_current_source_index_digest() {
    let source = signed_source(2, 0x75, 1_800_000_000);
    let mut checkpoint = checkpoint_from_source(&source);
    checkpoint.source_index_blake3[0] ^= 0x80;
    assert!(
        validate_checkpoint_against_source(Some(&checkpoint), &source)
            .expect_err("current checkpoint must bind the exact source index")
            .to_string()
            .contains("source-index digest")
    );

    let mut intent = intent_from_source(&source);
    intent.target_source_index_blake3[0] ^= 0x80;
    let root = secure_temp_dir();
    let config = test_runtime_config(&source, root.path());
    assert!(
        validate_intent_against_source(&intent, None, None, &source, &config)
            .expect_err("current publish intent must bind the exact source index")
            .to_string()
            .contains("source-index digest")
    );
}

#[test]
fn manifest_chain_rejects_sequence_gap_and_timestamp_regression() {
    let signer = TestSigner::new(0x35);
    let source = signed_source(2, 0x35, 1_800_000_000);
    let mut sequence_blocks = source
        .blocks
        .iter()
        .map(|block| block.block.clone())
        .collect::<Vec<_>>();
    sequence_blocks[1].sequence = 7;
    sequence_blocks[1].block_cid = sequence_blocks[1]
        .recompute_block_cid()
        .expect("recompute sequence-gap CID");
    sequence_blocks[1].block_signature = signer.sign(
        &sequence_blocks[1]
            .signature_payload_bytes()
            .expect("encode sequence-gap block"),
    );
    let mut sequence_head = source.head.clone();
    sequence_head.head_block_cid = sequence_blocks[1].block_cid.clone();
    sequence_head.head_signature = signer.sign(
        &sequence_head
            .signature_payload_bytes()
            .expect("encode sequence-gap head"),
    );
    assert!(
        validate_governance_dag_head_against_chain_v1(&sequence_head, &sequence_blocks).is_err()
    );

    let mut time_blocks = source
        .blocks
        .iter()
        .map(|block| block.block.clone())
        .collect::<Vec<_>>();
    time_blocks[1].timestamp = time_blocks[0].timestamp.saturating_sub(1);
    time_blocks[1].block_cid = time_blocks[1]
        .recompute_block_cid()
        .expect("recompute regressed CID");
    time_blocks[1].block_signature = signer.sign(
        &time_blocks[1]
            .signature_payload_bytes()
            .expect("encode regressed block"),
    );
    let mut time_head = source.head.clone();
    time_head.head_block_cid = time_blocks[1].block_cid.clone();
    time_head.head_signature = signer.sign(
        &time_head
            .signature_payload_bytes()
            .expect("encode regressed head"),
    );
    assert!(validate_governance_dag_head_against_chain_v1(&time_head, &time_blocks).is_err());
}

#[test]
fn bounded_file_read_rejects_oversize() {
    let dir = secure_temp_dir();
    let path = dir.path().join("oversize.bin");
    fs::write(&path, [0_u8; 9]).expect("write oversized file");
    assert!(read_unrooted_regular_file(&path, 8, false).is_err());
}

#[test]
fn rooted_source_binding_rejects_equal_byte_substitution() {
    let dir = secure_temp_dir();
    let path = dir.path().join("source.to");
    fs::write(&path, b"same-bytes").expect("seed rooted source");
    let guard =
        GovernanceFilesystemRootGuard::capture_source(dir.path()).expect("retain source root");
    let snapshot =
        read_rooted_file(&guard, Path::new("source.to"), 32, false).expect("read rooted source");

    fs::remove_file(&path).expect("remove original source");
    fs::write(&path, b"same-bytes").expect("replace source with equal bytes");
    let error = verify_rooted_file_binding(&guard, &snapshot.binding())
        .expect_err("equal-byte identity substitution must fail closed");
    assert!(error.to_string().contains("substituted"));
}

#[cfg(unix)]
#[test]
fn rooted_source_read_rejects_descendant_symlink() {
    use std::os::unix::fs::symlink;
    let dir = secure_temp_dir();
    fs::write(dir.path().join("target.to"), b"target").expect("seed target");
    symlink(dir.path().join("target.to"), dir.path().join("linked.to"))
        .expect("create descendant symlink");
    let guard =
        GovernanceFilesystemRootGuard::capture_source(dir.path()).expect("retain source root");
    read_rooted_file(&guard, Path::new("linked.to"), 32, false)
        .expect_err("rooted source read must reject symlink");
}

#[test]
fn rooted_state_recovery_is_deterministic_after_restart() {
    let dir = secure_temp_dir();
    let guard =
        GovernanceFilesystemRootGuard::capture_writer(dir.path()).expect("retain state root");
    write_rooted_atomic_secret(&guard, Path::new(MIRROR_INDEX_FILE), b"first-generation")
        .expect("write first state generation");
    drop(guard);

    let stale = dir.path().join(format!(".{MIRROR_INDEX_FILE}.tmp-42000-9"));
    fs::write(&stale, b"crash-temporary").expect("seed restart temporary");
    let restarted = GovernanceFilesystemRootGuard::capture_writer(dir.path())
        .expect("retain restarted state root");
    write_rooted_atomic_secret(
        &restarted,
        Path::new(MIRROR_INDEX_FILE),
        b"second-generation",
    )
    .expect("recover and write second state generation");

    assert!(!stale.exists());
    assert_eq!(
        read_rooted_file(
            &restarted,
            Path::new(MIRROR_INDEX_FILE),
            MUTABLE_STATE_MAX_BYTES,
            true,
        )
        .expect("read restarted state")
        .bytes(),
        b"second-generation"
    );
}

#[cfg(unix)]
#[test]
fn bounded_file_read_rejects_symlink_hardlink_and_permissive_secret() {
    use std::os::unix::fs::symlink;
    let dir = secure_temp_dir();
    let target = dir.path().join("target.bin");
    fs::write(&target, [0x11; 32]).expect("write target");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).expect("secure target");

    let symlink_path = dir.path().join("symlink.bin");
    symlink(&target, &symlink_path).expect("create symlink");
    assert!(read_unrooted_regular_file(&symlink_path, 32, true).is_err());

    let hardlink_path = dir.path().join("hardlink.bin");
    fs::hard_link(&target, &hardlink_path).expect("create hard link");
    assert!(read_unrooted_regular_file(&target, 32, true).is_err());
    fs::remove_file(&hardlink_path).expect("remove hard link");

    fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
        .expect("make secret permissive");
    assert!(read_unrooted_regular_file(&target, 32, true).is_err());
}

#[cfg(unix)]
#[test]
fn legacy_secret_paths_are_rejected_without_following_symlinks_or_reading_files() {
    use std::os::unix::fs::symlink;
    let dir = secure_temp_dir();
    let source_dir = dir.path().join("source");
    fs::create_dir(&source_dir).expect("create source directory");
    let target = dir.path().join("permissive-secret");
    let sentinel = b"must-never-be-read-or-overwritten";
    fs::write(&target, sentinel).expect("write legacy secret sentinel");
    fs::set_permissions(&target, fs::Permissions::from_mode(0o644))
        .expect("make legacy secret permissive");
    let link = dir.path().join("legacy-secret-link");
    symlink(&target, &link).expect("create legacy secret symlink");

    for (field, path) in [
        ("ipfs_bearer_token_path", &link),
        ("head_bearer_token_path", &target),
        ("checkpoint_key_path", &link),
    ] {
        let config_path = dir.path().join(format!("{field}.toml"));
        fs::write(
            &config_path,
            format!(
                r#"[sorafs.storage]
governance_dag_dir = "{}"

[sorafs.storage.governance_dag_service]
enabled = false
{field} = "{}"
"#,
                source_dir.display(),
                path.display(),
            ),
        )
        .expect("write legacy config");
        let error = load_service_config(&config_path).expect_err("legacy secret path must fail");
        assert!(matches!(&error, GovernanceDagServiceError::Config(_)));
        assert!(
            !error.to_string().contains(&path.display().to_string()),
            "legacy secret path leaked into the parser error: {error}"
        );
        assert_eq!(
            fs::read(&target).expect("read sentinel after config rejection"),
            sentinel
        );
    }

    let config_path = dir.path().join("governance_dag_signing_key_path.toml");
    fs::write(
        &config_path,
        format!(
            r#"[sorafs.storage]
governance_dag_dir = "{}"
governance_dag_signing_key_path = "{}"

[sorafs.storage.governance_dag_service]
enabled = false
"#,
            source_dir.display(),
            link.display(),
        ),
    )
    .expect("write legacy signer config");
    let error = load_service_config(&config_path).expect_err("legacy signing-key path must fail");
    assert!(matches!(&error, GovernanceDagServiceError::Config(_)));
    assert!(
        !error.to_string().contains(&link.display().to_string()),
        "legacy signing-key path leaked into the parser error: {error}"
    );
    assert_eq!(
        fs::read(&target).expect("read sentinel after signer config rejection"),
        sentinel
    );
}

#[test]
fn sealed_checkpoint_rejects_tamper_and_mismatched_store_handle() {
    let source = signed_source(1, 0x36, 1_800_000_000);
    let checkpoint = checkpoint_from_source(&source);
    let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let store = test_checkpoint_store(provider.clone());
    let revision = save_checkpoint(&store, None, &checkpoint).expect("save sealed checkpoint");
    assert_eq!(
        load_checkpoint(&store).expect("load sealed checkpoint"),
        (Some(checkpoint.clone()), Some(revision))
    );

    let mismatch = OpaqueCheckpointStore::try_new(
        "kms:governance/checkpoint:other",
        TEST_STORE_QUALIFICATION,
        provider.clone(),
    )
    .expect_err("mismatched checkpoint provider handle must fail");
    assert!(mismatch.to_string().contains("does not match"));

    provider
        .qualification_revision
        .store(2, AtomicOrdering::SeqCst);
    let error = load_checkpoint(&store).expect_err("checkpoint provider policy drift must fail");
    assert!(error.to_string().contains("policy changed"));
    provider
        .qualification_revision
        .store(1, AtomicOrdering::SeqCst);

    let drifting_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let drifting_store = test_checkpoint_store(drifting_provider.clone());
    drifting_provider
        .drift_during_operation
        .store(true, AtomicOrdering::SeqCst);
    let error = load_checkpoint(&drifting_store)
        .expect_err("policy drift during a sealed read must discard its result");
    assert!(error.to_string().contains("policy changed"));

    let drifting_provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let drifting_store = test_checkpoint_store(drifting_provider.clone());
    drifting_provider
        .drift_during_operation
        .store(true, AtomicOrdering::SeqCst);
    let error = save_checkpoint(&drifting_store, None, &checkpoint)
        .expect_err("policy drift during sealed CAS must fail closed");
    assert!(error.to_string().contains("policy changed"));

    let mut inner = provider.inner.lock().expect("lock test store");
    let record = inner.checkpoint.as_mut().expect("checkpoint record");
    let last = record.payload.last_mut().expect("checkpoint is non-empty");
    *last ^= 0x80;
    drop(inner);
    assert!(load_checkpoint(&store).is_err());
}

#[test]
fn sealed_intent_rejects_tamper_rollback_replay_and_store_outage() {
    let source = signed_source(1, 0x37, 1_800_000_000);
    let intent = intent_from_source(&source);
    let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let store = test_checkpoint_store(provider.clone());
    let revision = save_publish_intent(&store, None, &intent).expect("save sealed intent");
    assert_eq!(
        load_publish_intent(&store).expect("load sealed intent"),
        (Some(intent.clone()), Some(revision))
    );

    delete_publish_intent(&store, Some(revision)).expect("delete exact intent revision");
    let error = save_publish_intent(&store, None, &intent)
        .expect_err("deleted intent generation replay must fail");
    assert!(error.to_string().contains("compare-and-swap failed"));

    let mut next_intent = intent.clone();
    next_intent.generation = next_intent.generation.saturating_add(1);
    let revision =
        save_publish_intent(&store, None, &next_intent).expect("next generation may resume");
    let error = save_publish_intent(&store, Some([0xA5; 32]), &next_intent)
        .expect_err("stale CAS revision must fail");
    assert!(error.to_string().contains("compare-and-swap failed"));

    let mut inner = provider.inner.lock().expect("lock test store");
    let record = inner.publish_intent.as_mut().expect("intent record");
    record
        .payload
        .truncate(record.payload.len().saturating_sub(1));
    drop(inner);
    assert!(load_publish_intent(&store).is_err());

    provider.refuse.store(true, AtomicOrdering::SeqCst);
    let error = load_publish_intent(&store).expect_err("store outage must fail closed");
    assert!(error.to_string().contains("read failed"));
    assert!(!error.to_string().contains("must-never-escape"));
    assert_ne!(revision, [0; 32]);
}

#[test]
fn producer_and_public_service_sealed_slots_coexist_without_cross_mutation() {
    let root = secure_temp_dir();
    let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let service_checkpoint = GovernanceDagSealedStateRecord::new(
        GovernanceDagSealedStateSlot::Checkpoint,
        7,
        vec![0x71],
    );
    let service_intent = GovernanceDagSealedStateRecord::new(
        GovernanceDagSealedStateSlot::PublishIntent,
        8,
        vec![0x72],
    );
    provider
        .compare_and_swap(
            GovernanceDagSealedStateSlot::Checkpoint,
            None,
            service_checkpoint.clone(),
        )
        .expect("seed service checkpoint slot");
    provider
        .compare_and_swap(
            GovernanceDagSealedStateSlot::PublishIntent,
            None,
            service_intent.clone(),
        )
        .expect("seed service intent slot");

    let publisher_peer_id = b"12D3KooWGovernanceSharedStore".to_vec();
    let signer = Arc::new(PublisherTestSigner {
        handle: "pkcs11:governance-dag:shared-store-primary".to_owned(),
        peer_id: publisher_peer_id.clone(),
        signer: TestSigner::new(0x77),
    });
    let signer = qualify_governance_dag_runtime_signer_provider(
        signer.handle().to_owned(),
        publisher_peer_id,
        signer.public_key(),
        GovernanceDagRuntimeProviderQualificationV1::new(1, [0x83; 32]),
        signer,
    )
    .expect("qualify shared-store producer signer");
    let producer_store = qualify_governance_dag_runtime_checkpoint_store(
        TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
        TEST_STORE_QUALIFICATION,
        provider.clone(),
    )
    .expect("qualify shared sealed store for producer slots");
    let publisher = FilesystemGovernancePublisher::try_new(root.path().join("producer"))
        .expect("create shared-store publisher")
        .with_qualified_runtime_dag_providers(signer, producer_store)
        .expect("bind producer providers to shared sealed store");
    let settlement = settlement(0, current_unix_timestamp_seconds());
    let encoded = norito::to_bytes(&settlement).expect("encode shared-store settlement");
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("commit producer transaction through producer-only slots");

    assert_eq!(
        provider
            .load(GovernanceDagSealedStateSlot::Checkpoint)
            .expect("read service checkpoint"),
        Some(service_checkpoint.clone())
    );
    assert_eq!(
        provider
            .load(GovernanceDagSealedStateSlot::PublishIntent)
            .expect("read service intent"),
        Some(service_intent)
    );
    let producer_checkpoint = provider
        .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)
        .expect("read producer checkpoint")
        .expect("producer checkpoint exists");
    let producer_intent = provider
        .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
        .expect("read producer intent");
    assert!(producer_intent.is_none());

    let next_service_checkpoint = GovernanceDagSealedStateRecord::new(
        GovernanceDagSealedStateSlot::Checkpoint,
        9,
        vec![0x73],
    );
    provider
        .compare_and_swap(
            GovernanceDagSealedStateSlot::Checkpoint,
            Some(service_checkpoint.revision),
            next_service_checkpoint,
        )
        .expect("advance service-only checkpoint slot");
    assert_eq!(
        provider
            .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)
            .expect("re-read producer checkpoint"),
        Some(producer_checkpoint)
    );
    assert!(
        provider
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("re-read producer intent")
            .is_none()
    );
}

#[test]
fn mirror_retention_honours_entry_and_byte_caps() {
    let source = signed_source(3, 0x38, 1_800_000_000);
    let intent = intent_from_source(&source);
    let latest = source.blocks[2].bytes.len() as u64;
    let previous = source.blocks[1].bytes.len() as u64;
    let exact_two = latest + previous;
    let retained = merge_published_blocks(None, &intent, &source, 2, exact_two)
        .expect("retain exact two-block suffix");
    assert_eq!(retained.len(), 2);
    assert_eq!(retained[0].sequence, 1);
    assert_eq!(retained[1].sequence, 2);

    let one = merge_published_blocks(None, &intent, &source, 1, exact_two)
        .expect("entry cap retains one block");
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].sequence, 2);

    let byte_limited = merge_published_blocks(None, &intent, &source, 3, exact_two - 1)
        .expect("byte cap retains the newest fitting suffix");
    assert_eq!(byte_limited.len(), 1);
    assert!(merge_published_blocks(None, &intent, &source, 3, latest - 1).is_err());
}

#[test]
fn signed_block_prefix_archive_structurally_covers_pruned_prefix() {
    let source = signed_source(5, 0x74, 1_800_000_000);
    let intent = intent_from_source(&source);
    let retained_bytes = source.blocks[3..]
        .iter()
        .map(|block| u64::try_from(block.bytes.len()).expect("block length fits u64"))
        .sum();
    let retained = merge_published_blocks(None, &intent, &source, 2, retained_bytes)
        .expect("plan bounded mirror suffix");
    assert_eq!(
        retained
            .iter()
            .map(|block| block.sequence)
            .collect::<Vec<_>>(),
        vec![3, 4]
    );

    let endpoint = block_prefix_archive_test_endpoint();
    let (archive, bytes, archive_head) = signed_block_prefix_archive_fixture(
        &source,
        0,
        retained[0].sequence,
        BlockPrefixArchiveHeadV1::empty(),
        &endpoint,
    );
    assert_eq!(
        archive
            .blocks
            .iter()
            .map(|entry| entry.published.sequence)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert_eq!(
        decode_signed_block_prefix_archive(&bytes, 1024 * 1024)
            .expect("decode exact archived prefix"),
        archive
    );

    let mut checkpoint = checkpoint_from_source(&source);
    checkpoint.mirror_blocks = retained.clone();
    assert!(
        validate_checkpoint_body(&checkpoint).is_err(),
        "the suffix cannot commit before its exact archive head"
    );
    checkpoint.archive_head = archive_head;
    validate_checkpoint_body(&checkpoint)
        .expect("archive head and retained suffix cover one exact prefix");
}

#[test]
fn signed_block_prefix_archive_rejects_corruption_rollback_and_equivocation() {
    let source = signed_source(4, 0x75, 1_800_000_000);
    let endpoint = block_prefix_archive_test_endpoint();
    let (_first, first_bytes, first_head) = signed_block_prefix_archive_fixture(
        &source,
        0,
        2,
        BlockPrefixArchiveHeadV1::empty(),
        &endpoint,
    );
    let (second, second_bytes, second_head) =
        signed_block_prefix_archive_fixture(&source, 2, 3, first_head.clone(), &endpoint);
    verify_block_prefix_archive_publication(&second, &second_bytes, &second_head)
        .expect("successor publication extends the signed predecessor");
    assert!(
        verify_block_prefix_archive_publication(&second, &second_bytes, &first_head,).is_err(),
        "an older authenticated head must not satisfy successor readback"
    );

    let mut corrupt = first_bytes;
    let position = corrupt.len() / 2;
    corrupt[position] ^= 0x80;
    assert!(
        decode_signed_block_prefix_archive(&corrupt, 1024 * 1024).is_err(),
        "corrupt canonical archive bytes must fail closed"
    );

    let mut equivocal = second.clone();
    equivocal.target_head_block_cid[0] ^= 0x40;
    let equivocal_bytes = norito::to_bytes(&equivocal).expect("encode equivocal archive");
    assert!(
        verify_block_prefix_archive_publication(&equivocal, &equivocal_bytes, &second_head,)
            .is_err(),
        "the predecessor-bound publication signature must reject equivocation"
    );

    let mut substituted_provider = second.clone();
    substituted_provider.ipfs_authenticator_revision += 1;
    assert!(
        verify_block_prefix_archive_publication(
            &substituted_provider,
            &norito::to_bytes(&substituted_provider).expect("encode provider-substituted archive"),
            &second_head,
        )
        .is_err(),
        "an archive cannot substitute its qualified HSM provider"
    );
    let mut substituted_mapping = second.clone();
    substituted_mapping.blocks[0].published.ipfs_cid =
        canonical_raw_sha256_cid(b"substituted-block-bytes");
    assert!(
        validate_signed_block_prefix_archive(&substituted_mapping).is_err(),
        "every archived IPFS mapping is recomputed from the exact signed block bytes"
    );
    let mut test_marked_provider = second;
    test_marked_provider.ipfs_authenticator_handle = "test://governance/archive-signer".to_owned();
    assert!(
        validate_signed_block_prefix_archive(&test_marked_provider).is_err(),
        "test-marked archive providers fail canonical validation"
    );
}

#[test]
fn archive_publication_attestation_survives_endpoint_and_provider_rotation() {
    let source = signed_source(2, 0x78, 1_800_000_000);
    let primary = block_prefix_archive_test_endpoint();
    let (archive, bytes, head) = signed_block_prefix_archive_fixture(
        &source,
        0,
        1,
        BlockPrefixArchiveHeadV1::empty(),
        &primary,
    );
    let mut secondary = block_prefix_archive_test_endpoint();
    secondary.url = Url::parse("http://127.0.0.1:2/").expect("parse secondary archive endpoint");
    secondary.authenticator = test_authenticator(
        "pkcs11:governance-dag:archive-failover-hsm",
        GovernanceDagAuthenticationScope::Ipfs,
    );
    assert_ne!(
        head.publication
            .as_ref()
            .expect("archive publication exists")
            .canonical_url,
        block_prefix_archive_add_descriptor(&secondary, &archive, &bytes)
            .expect("build secondary descriptor")
            .canonical_url(),
    );
    verify_block_prefix_archive_publication(&archive, &bytes, &head)
        .expect("historical publication is self-contained after endpoint failover");
}

#[test]
fn archive_progress_is_sealed_before_suffix_checkpoint() {
    let source = signed_source(4, 0x79, 1_800_000_000);
    let endpoint = block_prefix_archive_test_endpoint();
    let (_archive, _bytes, archive_head) = signed_block_prefix_archive_fixture(
        &source,
        0,
        2,
        BlockPrefixArchiveHeadV1::empty(),
        &endpoint,
    );
    let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let store = test_checkpoint_store(provider);
    let mut intent = intent_from_source(&source);
    let initial_revision =
        save_publish_intent(&store, None, &intent).expect("seal initial publish intent");

    intent.archive_head = archive_head;
    let progress_revision = save_publish_intent(&store, Some(initial_revision), &intent)
        .expect("seal verified archive progress");
    let (loaded, loaded_revision) =
        load_publish_intent(&store).expect("reload crash-resumable archive progress");
    assert_eq!(loaded, Some(intent.clone()));
    assert_eq!(loaded_revision, Some(progress_revision));
    assert!(
        save_publish_intent(&store, Some(initial_revision), &intent).is_err(),
        "a stale replica cannot overwrite sealed archive progress"
    );
}

#[test]
fn archive_predecessor_commitment_binds_exact_checkpoint_body_and_revision() {
    let predecessor_source = signed_source(2, 0x7a, 1_800_000_000);
    let source = signed_source(4, 0x7a, 1_800_000_000);
    let checkpoint = checkpoint_from_source(&predecessor_source);
    let bytes = norito::to_bytes(&checkpoint).expect("encode predecessor checkpoint");
    let record = GovernanceDagSealedStateRecord::new(
        GovernanceDagSealedStateSlot::Checkpoint,
        checkpoint.generation,
        bytes.clone(),
    );
    let commitment = checkpoint_commitment(Some(&checkpoint), Some(record.revision))
        .expect("bind exact sealed predecessor checkpoint");
    assert_eq!(commitment.digest, blake3_array(&bytes));
    assert_eq!(commitment.block_count, checkpoint.block_count);
    assert_eq!(commitment.head_block_cid, checkpoint.head_block_cid);

    let mut substituted_revision = record.revision;
    substituted_revision[0] ^= 0x80;
    assert!(
        checkpoint_commitment(Some(&checkpoint), Some(substituted_revision)).is_err(),
        "a predecessor revision cannot be detached from its canonical checkpoint body"
    );

    let endpoint = block_prefix_archive_test_endpoint();
    let (archive, _archive_bytes, _archive_head) =
        signed_block_prefix_archive_fixture_with_checkpoint(
            &source,
            0,
            2,
            BlockPrefixArchiveHeadV1::empty(),
            commitment,
            checkpoint.generation + 1,
            &endpoint,
        );
    let mut wrong_position = archive;
    wrong_position.predecessor_block_count -= 1;
    assert!(
        validate_block_prefix_archive_against_source(&wrong_position, &source).is_err(),
        "the predecessor head must occupy its exact committed source position"
    );
}

#[test]
fn archive_readback_validation_precedes_checkpoint_construction() {
    let source = signed_source(4, 0x76, 1_800_000_000);
    let intent = intent_from_source(&source);
    let retained_bytes = source.blocks[2..]
        .iter()
        .map(|block| u64::try_from(block.bytes.len()).expect("block length fits u64"))
        .sum();
    let retained = merge_published_blocks(None, &intent, &source, 2, retained_bytes)
        .expect("plan retained suffix");
    let endpoint = block_prefix_archive_test_endpoint();
    let (archive, archive_bytes, archive_head) = signed_block_prefix_archive_fixture(
        &source,
        0,
        retained[0].sequence,
        BlockPrefixArchiveHeadV1::empty(),
        &endpoint,
    );
    let checkpoint_before_crash = checkpoint_from_source(&source);

    let truncated_readback = &archive_bytes[..archive_bytes.len() - 1];
    assert!(decode_signed_block_prefix_archive(truncated_readback, 1024 * 1024).is_err());
    assert_eq!(checkpoint_before_crash.mirror_blocks.len(), 4);
    assert_eq!(
        checkpoint_before_crash.archive_head,
        BlockPrefixArchiveHeadV1::empty(),
        "a crash before exact readback cannot expose a truncated mirror checkpoint"
    );

    assert_eq!(
        decode_signed_block_prefix_archive(&archive_bytes, 1024 * 1024)
            .expect("restart authenticates the complete staged archive"),
        archive
    );
    verify_block_prefix_archive_publication(&archive, &archive_bytes, &archive_head)
        .expect("restart verifies the exact archive publication");
    let mut committed = checkpoint_before_crash;
    committed.archive_head = archive_head;
    committed.mirror_blocks = retained;
    validate_checkpoint_body(&committed)
        .expect("only verified archive readback permits suffix checkpointing");
}

#[test]
fn signed_block_prefix_archive_checkpoint_cas_fences_replicas_and_replay() {
    let source = signed_source(4, 0x77, 1_800_000_000);
    let intent = intent_from_source(&source);
    let retained_bytes = source.blocks[2..]
        .iter()
        .map(|block| u64::try_from(block.bytes.len()).expect("block length fits u64"))
        .sum();
    let retained = merge_published_blocks(None, &intent, &source, 2, retained_bytes)
        .expect("plan retained suffix");
    let endpoint = block_prefix_archive_test_endpoint();
    let (_archive, _bytes, archive_head) = signed_block_prefix_archive_fixture(
        &source,
        0,
        retained[0].sequence,
        BlockPrefixArchiveHeadV1::empty(),
        &endpoint,
    );
    let mut checkpoint = checkpoint_from_source(&source);
    checkpoint.archive_head = archive_head;
    checkpoint.mirror_blocks = retained;
    let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    let store = OpaqueCheckpointStore::try_new(
        TEST_CHECKPOINT_STORE_HANDLE,
        TEST_STORE_QUALIFICATION,
        provider,
    )
    .expect("bind replica-fencing store");
    save_checkpoint(&store, None, &checkpoint).expect("first replica commits archive head");
    assert!(
        save_checkpoint(&store, None, &checkpoint).is_err(),
        "a concurrent replica cannot reuse the predecessor revision"
    );
    assert!(
        save_checkpoint(&store, None, &checkpoint).is_err(),
        "replaying the same archive checkpoint remains fenced"
    );
}

#[test]
fn canonical_lookup_ids_reject_uppercase_short_and_non_hex() {
    assert!(is_canonical_digest_hex(&"ab".repeat(32)));
    assert!(!is_canonical_digest_hex(&"AB".repeat(32)));
    assert!(!is_canonical_digest_hex("ab"));
    assert!(!is_canonical_digest_hex(&"gg".repeat(32)));
}

#[test]
fn json_response_etag_supports_exact_not_modified() {
    let value = JsonValue::from("stable");
    let first = json_response(StatusCode::OK, value.clone(), &HeaderMap::new());
    assert_eq!(first.status(), StatusCode::OK);
    let etag = first
        .headers()
        .get(header::ETAG)
        .expect("response has ETag")
        .clone();
    let mut request_headers = HeaderMap::new();
    request_headers.insert(header::IF_NONE_MATCH, etag.clone());
    let second = json_response(StatusCode::OK, value, &request_headers);
    assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(second.headers().get(header::ETAG), Some(&etag));
}

#[tokio::test]
async fn routes_reject_noncanonical_identifiers_before_lookup() {
    let state = ApiState(Arc::new(RwLock::new(ApiSnapshot {
        live: true,
        ready: true,
        ..ApiSnapshot::default()
    })));
    let app = service_router(state);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/sorafs/governance/dag/blocks/ABCD")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("route response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/sorafs/governance/dag/digests/gggg")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("route response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn private_ipfs_permission_does_not_authorize_private_head_endpoint() {
    let config = SorafsGovernanceDagService {
        allow_insecure_http: true,
        allow_private_ipfs_endpoint: true,
        allow_private_head_endpoint: false,
        ..SorafsGovernanceDagService::default()
    };
    let ipfs = build_pinned_endpoint(
        "http://127.0.0.1:5001",
        test_authenticator(
            TEST_IPFS_AUTH_HANDLE,
            GovernanceDagAuthenticationScope::Ipfs,
        ),
        GovernanceDagAuthenticationScope::Ipfs,
        &config,
        true,
    )
    .await;
    assert!(ipfs.is_ok());
    let head = build_pinned_endpoint(
        "http://127.0.0.1:9099/head",
        test_authenticator(
            TEST_HEAD_AUTH_HANDLE,
            GovernanceDagAuthenticationScope::SignedHead,
        ),
        GovernanceDagAuthenticationScope::SignedHead,
        &config,
        false,
    )
    .await;
    assert!(head.is_err());
}

#[tokio::test]
async fn dns_policy_rejects_mixed_mapped_overcap_and_timeout_answers() {
    let public = "8.8.8.8:443".parse().expect("public address");
    let private = "127.0.0.1:443".parse().expect("private address");
    assert!(
        resolve_endpoint_addresses(
            std::future::ready(Ok(vec![public, private])),
            Duration::from_secs(1),
            false,
        )
        .await
        .is_err()
    );

    let mapped = SocketAddr::new(
        IpAddr::V6("::ffff:127.0.0.1".parse().expect("mapped IPv6")),
        443,
    );
    assert!(
        resolve_endpoint_addresses(
            std::future::ready(Ok(vec![mapped])),
            Duration::from_secs(1),
            false,
        )
        .await
        .is_err()
    );

    let over_cap = (1..=(MAX_DNS_ADDRESSES + 1))
        .map(|last| SocketAddr::new(IpAddr::V4(Ipv4Addr::new(8, 8, 4, last as u8)), 443))
        .collect::<Vec<_>>();
    assert!(
        resolve_endpoint_addresses(
            std::future::ready(Ok(over_cap)),
            Duration::from_secs(1),
            false,
        )
        .await
        .is_err()
    );

    let delayed = async {
        time::sleep(Duration::from_millis(50)).await;
        Ok(vec![public])
    };
    assert!(
        resolve_endpoint_addresses(delayed, Duration::from_millis(1), false)
            .await
            .is_err()
    );

    let calls = Arc::new(AtomicU64::new(0));
    let calls_for_resolution = calls.clone();
    let resolved = resolve_endpoint_addresses(
        async move {
            calls_for_resolution.fetch_add(1, Ordering::SeqCst);
            Ok(vec![public, public])
        },
        Duration::from_secs(1),
        false,
    )
    .await
    .expect("one pinned public DNS snapshot");
    assert_eq!(resolved, vec![public]);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn ipfs_urls_cids_and_secret_debug_output_are_canonical() {
    let provider = Arc::new(TestAuthenticator::new(
        TEST_IPFS_AUTH_HANDLE,
        "never-log-this-token",
    ));
    let authenticator = OpaqueAuthenticator::try_new(
        TEST_IPFS_AUTH_HANDLE,
        TEST_AUTH_QUALIFICATION,
        test_request_auth_policy(provider.public_key()),
        provider,
        GovernanceDagAuthenticationScope::Ipfs,
        test_replay_store(),
        "IPFS authenticator",
    )
    .expect("bind test authenticator");
    let endpoint = PinnedEndpoint {
        url: Url::parse("http://127.0.0.1:5001/").expect("test URL"),
        client: Client::builder().no_proxy().build().expect("test client"),
        authentication_scope: GovernanceDagAuthenticationScope::Ipfs,
        authenticator: authenticator.clone(),
        max_request_bytes: GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1 as u64,
    };
    let url = endpoint
        .ipfs_url(
            "api/v0/cat",
            &[("arg", TEST_CID_PAYLOAD), ("progress", "false")],
        )
        .expect("canonical IPFS URL");
    let pairs = url.query_pairs().collect::<Vec<_>>();
    assert_eq!(pairs.len(), 2, "query fields must not be duplicated");
    assert_eq!(pairs[0], ("arg".into(), TEST_CID_PAYLOAD.into()));
    assert_eq!(pairs[1], ("progress".into(), "false".into()));

    for cid in [
        TEST_CID_PAYLOAD,
        TEST_CID_BLOCK,
        TEST_CID_HEAD,
        TEST_CID_OLD,
        TEST_CID_NEW,
        TEST_CID_ATTACKER,
    ] {
        assert!(is_canonical_cid_v1(cid), "valid CID rejected: {cid}");
        assert_eq!(
            validate_ipfs_cid(cid).expect("canonical CID must validate"),
            cid
        );
    }
    let uppercase = TEST_CID_PAYLOAD.to_ascii_uppercase();
    let padded = format!("{TEST_CID_PAYLOAD}=");
    let truncated = &TEST_CID_PAYLOAD[..TEST_CID_PAYLOAD.len() - 1];
    for cid in [
        "",
        "QmYwAPJzv5CZsnAzt8auVZRnGi2j4XQJKiTyrZq4XgNLwN",
        "bafytestcid",
        uppercase.as_str(),
        padded.as_str(),
        truncated,
    ] {
        assert!(!is_canonical_cid_v1(cid), "invalid CID accepted: {cid}");
        assert!(validate_ipfs_cid(cid).is_err());
    }

    let rendered = format!("{authenticator:?}");
    assert!(rendered.contains("[REDACTED]"));
    assert!(!rendered.contains("never-log-this-token"));
    assert!(rendered.contains(TEST_IPFS_AUTH_HANDLE));
}

#[test]
fn authenticator_rotates_per_request_and_redacts_provider_failures() {
    let provider = Arc::new(TestAuthenticator::new(
        TEST_IPFS_AUTH_HANDLE,
        "first-secret-token",
    ));
    let authenticator = OpaqueAuthenticator::try_new(
        TEST_IPFS_AUTH_HANDLE,
        TEST_AUTH_QUALIFICATION,
        test_request_auth_policy(provider.public_key()),
        provider.clone(),
        GovernanceDagAuthenticationScope::Ipfs,
        test_replay_store(),
        "IPFS authenticator",
    )
    .expect("bind runtime authenticator");
    let client = Client::builder().no_proxy().build().expect("test client");
    let url = Url::parse("https://example.invalid/").expect("test URL");
    let request = client
        .get(url)
        .header(header::ACCEPT_ENCODING, "identity")
        .build()
        .expect("build test request");
    let descriptor = canonical_outbound_request_descriptor(
        &request,
        GovernanceDagAuthenticationScope::Ipfs,
        1024,
    )
    .expect("canonical test request");

    let first = authenticator
        .authenticate(&descriptor)
        .expect("authenticate first request");
    assert_eq!(first.request_digest(), descriptor.request_digest());
    assert_eq!(first.public_key(), provider.public_key());

    provider.rotate("rotated-secret-token");
    let rotated = authenticator
        .authenticate(&descriptor)
        .expect("authenticate rotated request");
    assert_ne!(first.nonce(), rotated.nonce());

    provider
        .qualification_revision
        .store(2, AtomicOrdering::SeqCst);
    let error = authenticator
        .authenticate(&descriptor)
        .expect_err("authenticator policy drift must fail closed");
    assert!(error.to_string().contains("policy changed"));
    provider
        .qualification_revision
        .store(1, AtomicOrdering::SeqCst);

    let drifting_provider = Arc::new(TestAuthenticator::new(
        TEST_IPFS_AUTH_HANDLE,
        "must-not-be-returned",
    ));
    let drifting_authenticator = OpaqueAuthenticator::try_new(
        TEST_IPFS_AUTH_HANDLE,
        TEST_AUTH_QUALIFICATION,
        test_request_auth_policy(drifting_provider.public_key()),
        drifting_provider.clone(),
        GovernanceDagAuthenticationScope::Ipfs,
        test_replay_store(),
        "IPFS authenticator",
    )
    .expect("bind stable runtime authenticator");
    drifting_provider
        .drift_during_authentication
        .store(true, AtomicOrdering::SeqCst);
    let error = drifting_authenticator
        .authenticate(&descriptor)
        .expect_err("policy drift during authentication must discard the request");
    assert!(error.to_string().contains("policy changed"));
    assert!(!error.to_string().contains("must-not-be-returned"));

    provider.refuse.store(true, AtomicOrdering::SeqCst);
    let error = authenticator
        .authenticate(&descriptor)
        .expect_err("authenticator outage must fail closed");
    assert!(error.to_string().contains("refused"));
    assert!(!error.to_string().contains("rotated-secret-token"));

    let mismatch = OpaqueAuthenticator::try_new(
        "vault:governance/ipfs:other",
        TEST_AUTH_QUALIFICATION,
        test_request_auth_policy(provider.public_key()),
        provider,
        GovernanceDagAuthenticationScope::Ipfs,
        test_replay_store(),
        "IPFS authenticator",
    )
    .expect_err("mismatched authenticator handle must fail");
    assert!(mismatch.to_string().contains("does not match"));
}

#[test]
fn request_auth_envelope_rejects_tamper_replay_key_and_time_failures() {
    let provider = Arc::new(TestAuthenticator::new(
        TEST_IPFS_AUTH_HANDLE,
        "never-expose-hsm-diagnostic",
    ));
    let authenticator = OpaqueAuthenticator::try_new(
        TEST_IPFS_AUTH_HANDLE,
        TEST_AUTH_QUALIFICATION,
        test_request_auth_policy(provider.public_key()),
        provider,
        GovernanceDagAuthenticationScope::Ipfs,
        test_replay_store(),
        "IPFS authenticator",
    )
    .expect("bind request-auth verifier");
    let request = canonical_test_request(
        GovernanceDagAuthenticationScope::Ipfs,
        "POST",
        "https://example.invalid/api/v0/pin/add?arg=cid&recursive=true",
        &[("accept-encoding", "identity")],
        b"",
    );
    let tampered = canonical_test_request(
        GovernanceDagAuthenticationScope::Ipfs,
        "POST",
        "https://example.invalid/api/v0/pin/add?arg=other&recursive=true",
        &[("accept-encoding", "identity")],
        b"",
    );
    let now = current_unix_timestamp_seconds();
    let envelope = signed_test_request_auth_envelope(
        TEST_IPFS_AUTH_HANDLE,
        &request,
        now,
        now + 15,
        [0x11; 32],
    );
    authenticator
        .validate_envelope(&request, &envelope)
        .expect("accept first exact envelope");
    let replay = authenticator
        .validate_envelope(&request, &envelope)
        .expect_err("reject exact nonce replay");
    assert!(replay.to_string().contains("replay"));
    let tamper = authenticator
        .validate_envelope(&tampered, &envelope)
        .expect_err("reject URL/request-digest tamper");
    assert!(tamper.to_string().contains("does not match"));

    let wrong_key = signed_test_request_auth_envelope(
        TEST_HEAD_AUTH_HANDLE,
        &request,
        now,
        now + 15,
        [0x12; 32],
    );
    assert!(
        authenticator
            .validate_envelope(&request, &wrong_key)
            .expect_err("reject wrong public key")
            .to_string()
            .contains("does not match")
    );

    let invalid_signature = GovernanceDagRequestAuthenticationEnvelopeV1::try_new(
        &request,
        now,
        now + 15,
        [0x13; 32],
        test_request_auth_public_key(TEST_IPFS_AUTH_HANDLE),
        [0x55; 64],
    )
    .expect("structurally non-zero envelope");
    assert!(
        authenticator
            .validate_envelope(&request, &invalid_signature)
            .expect_err("reject invalid signature")
            .to_string()
            .contains("signature")
    );

    for (issued_at, expires_at, nonce, label) in [
        (now - 20, now - 1, [0x21; 32], "stale"),
        (now + 6, now + 16, [0x22; 32], "future"),
        (now, now + 31, [0x23; 32], "overlong"),
    ] {
        let envelope = signed_test_request_auth_envelope(
            TEST_IPFS_AUTH_HANDLE,
            &request,
            issued_at,
            expires_at,
            nonce,
        );
        let error = authenticator
            .validate_envelope(&request, &envelope)
            .unwrap_err();
        assert!(
            error.to_string().contains(label)
                || error.to_string().contains("future")
                || error.to_string().contains("overlong"),
            "{label} envelope returned unexpected error: {error}"
        );
    }
}

#[test]
fn sealed_request_auth_replay_fences_two_replica_cas_race() {
    let barrier = Arc::new(Barrier::new(2));
    let shared_store = Arc::new(
        TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE).with_replay_load_barrier(barrier),
    );
    let first = test_authenticator_with_store(
        Arc::new(TestAuthenticator::new(
            TEST_IPFS_AUTH_HANDLE,
            "first-replica-hsm",
        )),
        GovernanceDagAuthenticationScope::Ipfs,
        shared_store.clone(),
    );
    let second = test_authenticator_with_store(
        Arc::new(TestAuthenticator::new(
            TEST_IPFS_AUTH_HANDLE,
            "second-replica-hsm",
        )),
        GovernanceDagAuthenticationScope::Ipfs,
        shared_store,
    );
    let retry = second.clone();
    let request = canonical_test_request(
        GovernanceDagAuthenticationScope::Ipfs,
        "POST",
        "https://example.invalid/api/v0/pin/add?arg=shared",
        &[("accept-encoding", "identity")],
        b"",
    );
    let now = current_unix_timestamp_seconds();
    let envelope = signed_test_request_auth_envelope(
        TEST_IPFS_AUTH_HANDLE,
        &request,
        now,
        now + 15,
        [0xA7; 32],
    );
    let first_request = request.clone();
    let first_envelope = envelope.clone();
    let first_result =
        std::thread::spawn(move || first.validate_envelope(&first_request, &first_envelope));
    let second_request = request.clone();
    let second_envelope = envelope.clone();
    let second_result =
        std::thread::spawn(move || second.validate_envelope(&second_request, &second_envelope));
    let results = [
        first_result.join().expect("join first replay consumer"),
        second_result.join().expect("join second replay consumer"),
    ];
    assert_eq!(
        results.iter().filter(|result| result.is_ok()).count(),
        1,
        "linearizable CAS must accept the shared nonce exactly once"
    );
    let conflict = results
        .iter()
        .find_map(|result| result.as_ref().err())
        .expect("one replica must lose the replay-state CAS");
    assert!(conflict.to_string().contains("compare-and-swap failed"));

    let replay = retry
        .validate_envelope(&request, &envelope)
        .expect_err("a later replica must observe the committed duplicate nonce");
    assert!(replay.to_string().contains("replay was rejected"));
}

#[test]
fn sealed_request_auth_replay_prunes_expiry_but_never_evicts_live_capacity() {
    let now = 1_700_000_000_u64;
    let entries = (0..GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_CACHE_CAPACITY_V1)
        .map(|index| {
            let mut nonce = [0_u8; 32];
            nonce[0] = 1;
            nonce[24..].copy_from_slice(
                &u64::try_from(index)
                    .expect("replay cache index fits u64")
                    .to_be_bytes(),
            );
            RequestAuthReplayEntryV1 {
                nonce,
                expires_at_unix_secs: now + 10,
            }
        })
        .collect();
    let state = RequestAuthReplayStateV1 {
        version: REQUEST_AUTH_REPLAY_STATE_VERSION_V1,
        entries,
    };
    let payload = norito::to_bytes(&state).expect("encode full replay state");
    assert!(
        payload.len()
            <= governance_dag_sealed_state_payload_max_bytes_v1(
                GovernanceDagSealedStateSlot::IpfsRequestReplay,
            )
    );
    let decoded: RequestAuthReplayStateV1 = norito::decode_from_bytes_with_limits(
        &payload,
        request_auth_replay_decode_limits(governance_dag_sealed_state_payload_max_bytes_v1(
            GovernanceDagSealedStateSlot::IpfsRequestReplay,
        )),
    )
    .expect("full replay state must decode within its resource budget");
    assert_eq!(decoded, state);
    let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
    {
        let mut inner = provider.inner.lock().expect("lock test sealed state");
        inner.ipfs_request_replay = Some(GovernanceDagSealedStateRecord::new(
            GovernanceDagSealedStateSlot::IpfsRequestReplay,
            1,
            payload,
        ));
        inner.ipfs_request_replay_generation_floor = 1;
    }
    let store = test_checkpoint_store(provider.clone());
    let full = consume_sealed_request_auth_nonce(
        &store,
        GovernanceDagSealedStateSlot::IpfsRequestReplay,
        [2; 32],
        now + 20,
        now,
    )
    .expect_err("live replay entries must never be evicted at capacity");
    assert!(
        full.to_string().contains("bounded capacity"),
        "unexpected full replay-state error: {full}"
    );

    consume_sealed_request_auth_nonce(
        &store,
        GovernanceDagSealedStateSlot::IpfsRequestReplay,
        [2; 32],
        now + 20,
        now + 11,
    )
    .expect("expired replay entries should be pruned before admission");
    let observed = load_sealed_record(&store, GovernanceDagSealedStateSlot::IpfsRequestReplay)
        .expect("load pruned replay state")
        .expect("pruned replay state must exist");
    let observed_state = decode_request_auth_replay_state(
        &observed,
        GovernanceDagSealedStateSlot::IpfsRequestReplay,
        now + 11,
    )
    .expect("decode pruned replay state");
    assert_eq!(
        observed_state.entries,
        vec![RequestAuthReplayEntryV1 {
            nonce: [2; 32],
            expires_at_unix_secs: now + 20,
        }]
    );
    assert_eq!(observed.generation, 2);
}

#[test]
fn sealed_request_auth_replay_rejects_corrupted_state() {
    let unsorted = norito::to_bytes(&RequestAuthReplayStateV1 {
        version: REQUEST_AUTH_REPLAY_STATE_VERSION_V1,
        entries: vec![
            RequestAuthReplayEntryV1 {
                nonce: [0x22; 32],
                expires_at_unix_secs: 1_700_000_010,
            },
            RequestAuthReplayEntryV1 {
                nonce: [0x11; 32],
                expires_at_unix_secs: 1_700_000_010,
            },
        ],
    })
    .expect("encode structurally valid but unsorted replay state");
    for (payload, expected_error) in [
        (vec![0xFF], "not valid canonical Norito"),
        (unsorted, "contains invalid entries"),
    ] {
        let provider = Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE));
        {
            let mut inner = provider.inner.lock().expect("lock test sealed state");
            inner.ipfs_request_replay = Some(GovernanceDagSealedStateRecord::new(
                GovernanceDagSealedStateSlot::IpfsRequestReplay,
                1,
                payload,
            ));
            inner.ipfs_request_replay_generation_floor = 1;
        }
        let error = consume_sealed_request_auth_nonce(
            &test_checkpoint_store(provider),
            GovernanceDagSealedStateSlot::IpfsRequestReplay,
            [0x31; 32],
            1_700_000_010,
            1_700_000_000,
        )
        .expect_err("corrupted replay state must fail closed");
        assert!(error.to_string().contains(expected_error));
    }
}
