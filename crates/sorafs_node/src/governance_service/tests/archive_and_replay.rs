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
    let producer_signer_handle = "provider:governance-dag:source-primary";
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
        max_future_skew_secs: 60,
        allow_head_bootstrap: true,
        expected_producer_signer_handle: producer_signer_handle.to_owned(),
        expected_producer_signer_qualification: TEST_PRODUCER_SIGNER_QUALIFICATION,
        expected_checkpoint_store_handle: "kms:governance-dag:source-producer-checkpoint"
            .to_owned(),
        expected_checkpoint_store_qualification: TEST_STORE_QUALIFICATION,
        expected_publisher_peer_id: TEST_PRODUCER_PEER_ID.as_bytes().to_vec(),
        expected_public_key: signer.public_key,
    };
    assert!(
        !config.source_dir.join("runtime-dag-index.json").exists()
            && !config.source_dir.join("runtime-dag/head.to").exists(),
        "a fresh producer must expose mutable head/index state only through the typed store"
    );
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
    let committed = load_runtime_dag_committed_snapshot_v1(&config.source_root_guard)
        .expect("load publisher typed committed state")
        .expect("publisher committed state exists");
    let original_head_bytes = committed.head_bytes().to_vec();
    let original_index_bytes = committed.index_bytes().to_vec();
    let original_index: JsonValue = json::from_slice(&original_index_bytes)
        .expect("decode publisher runtime index for strict binding tests");
    let mut strict_drift_cases = Vec::new();
    for (field, replacement) in [
        ("source", JsonValue::from("substituted")),
        ("root", JsonValue::from("runtime-dag")),
        ("generated_at", JsonValue::from(timestamp.saturating_add(1))),
        ("signer_handle", JsonValue::from("provider:attacker")),
        ("signer_revision", JsonValue::from(2_u64)),
        (
            "signer_policy_digest_hex",
            JsonValue::from(hex::encode([0xA1; 32])),
        ),
        ("publisher_peer_id", JsonValue::from("attacker-peer")),
        ("checkpoint_store_handle", JsonValue::from("kms:attacker")),
        ("checkpoint_store_revision", JsonValue::from(2_u64)),
        (
            "checkpoint_store_policy_digest_hex",
            JsonValue::from(hex::encode([0xA2; 32])),
        ),
    ] {
        let mut drifted = original_index.clone();
        drifted
            .as_object_mut()
            .expect("runtime index object")
            .insert(field.to_owned(), replacement);
        strict_drift_cases.push((field, json::to_json_pretty(&drifted).expect("encode drift")));
    }
    strict_drift_cases.push((
        "noncanonical-json",
        json::to_json(&original_index).expect("encode compact runtime index"),
    ));
    let mut unknown_top = original_index.clone();
    unknown_top
        .as_object_mut()
        .expect("runtime index object")
        .insert("unknown_top_level".into(), JsonValue::from(true));
    strict_drift_cases.push((
        "unknown-top-level",
        json::to_json_pretty(&unknown_top).expect("encode unknown top-level field"),
    ));
    let mut unknown_block = original_index.clone();
    unknown_block
        .get_mut("blocks")
        .and_then(JsonValue::as_array_mut)
        .and_then(|blocks| blocks.first_mut())
        .and_then(JsonValue::as_object_mut)
        .expect("first runtime index block")
        .insert("unknown_block_field".into(), JsonValue::from(true));
    strict_drift_cases.push((
        "unknown-block-field",
        json::to_json_pretty(&unknown_block).expect("encode unknown block field"),
    ));
    for (field, drifted) in strict_drift_cases {
        write_runtime_dag_committed_snapshot_fixture_v1(
            &config.source_dir,
            original_head_bytes.clone(),
            drifted.into_bytes(),
        )
        .expect("commit strict-boundary drift");
        let error = match load_source_snapshot(&config) {
            Ok(_) => panic!("runtime index `{field}` drift must fail closed"),
            Err(error) => error,
        };
        assert!(
            matches!(error, GovernanceDagServiceError::Source(_)),
            "unexpected `{field}` drift error: {error}"
        );
        write_runtime_dag_committed_snapshot_fixture_v1(
            &config.source_dir,
            original_head_bytes.clone(),
            original_index_bytes.clone(),
        )
        .expect("restore strict runtime index fixture");
    }
    let mut provenance_tampered_index: JsonValue = json::from_slice(&original_index_bytes)
        .expect("decode publisher runtime index for provenance tamper");
    provenance_tampered_index
        .get_mut("blocks")
        .and_then(JsonValue::as_array_mut)
        .and_then(|blocks| blocks.first_mut())
        .and_then(JsonValue::as_object_mut)
        .expect("first publisher runtime index entry")
        .insert(
            "submission_publisher_account_digest_hex".into(),
            JsonValue::from(hex::encode([0xA5; 32])),
        );
    let provenance_tampered_bytes = json::to_json_pretty(&provenance_tampered_index)
        .expect("encode provenance-tampered runtime index")
        .into_bytes();
    write_runtime_dag_committed_snapshot_fixture_v1(
        &config.source_dir,
        original_head_bytes.clone(),
        provenance_tampered_bytes,
    )
    .expect("commit provenance-tampered typed runtime state");
    let provenance_error = load_source_snapshot(&config)
        .expect_err("unsigned runtime-index provenance must not override the signed node");
    assert!(
        provenance_error
            .to_string()
            .contains("submission provenance does not match its signed governance node"),
        "unexpected provenance substitution error: {provenance_error}"
    );
    write_runtime_dag_committed_snapshot_fixture_v1(
        &config.source_dir,
        original_head_bytes.clone(),
        original_index_bytes.clone(),
    )
    .expect("restore typed runtime state");
    let original_index_value: JsonValue = json::from_slice(&original_index_bytes)
        .expect("decode runtime index for immutable-source tests");
    let first_original_entry = original_index_value
        .get("blocks")
        .and_then(JsonValue::as_array)
        .and_then(|blocks| blocks.first())
        .and_then(JsonValue::as_object)
        .expect("first immutable runtime entry");
    let json_source_path = first_original_entry
        .get("json_path")
        .and_then(JsonValue::as_str)
        .expect("JSON source path");
    let json_source_path = config.source_dir.join(json_source_path);
    let original_json_source = fs::read(&json_source_path).expect("read original JSON source");
    write_test_sidecar_file(&json_source_path, br#"{"substituted":true}"#);
    let error = load_source_snapshot(&config)
        .expect_err("JSON source substitution must violate its content-addressed pair path");
    assert!(
        error.to_string().contains("source paths do not bind"),
        "unexpected JSON source substitution error: {error}"
    );
    write_test_sidecar_file(&json_source_path, &original_json_source);
    let orphan_path = config
        .source_dir
        .join(GOVERNANCE_RUNTIME_DAG_DIR)
        .join(GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR)
        .join("orphan.to");
    write_test_sidecar_file(&orphan_path, b"unindexed-runtime-artifact");
    let error = load_source_snapshot(&config)
        .expect_err("an unindexed immutable runtime artifact must fail exact inventory");
    assert!(
        error.to_string().contains("unindexed or missing artifact"),
        "unexpected runtime inventory error: {error}"
    );
    fs::remove_file(&orphan_path).expect("remove orphan test artifact");
    fs::remove_file(digest_sidecar_path(&orphan_path)).expect("remove orphan test digest sidecar");
    let mut index: JsonValue = json::from_slice(&original_index_bytes)
        .expect("decode publisher runtime index for source substitution");
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
    write_runtime_dag_committed_snapshot_fixture_v1(
        &config.source_dir,
        original_head_bytes,
        tampered_index,
    )
    .expect("commit source-substituted typed runtime state");
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
fn source_loader_rejects_legacy_loose_authorities_without_cleanup() {
    for relative in [
        "runtime-dag-index.json",
        ".runtime-dag-index.json.tmp-42000-1",
        "runtime-dag/head.to",
        "runtime-dag/.head.to.retained-v1-0000",
    ] {
        let root = secure_temp_dir();
        let source_dir = root.path().join("source");
        let mut source = signed_source(1, 0x7a, current_unix_timestamp_seconds().saturating_sub(1));
        materialize_source_snapshot(&source_dir, &mut source);
        let legacy_path = source_dir.join(relative);
        if let Some(parent) = legacy_path.parent() {
            fs::create_dir_all(parent).expect("create legacy authority parent");
        }
        fs::write(&legacy_path, b"legacy-runtime-authority-must-remain")
            .expect("seed legacy runtime authority");
        let config = test_runtime_config(&source, root.path());
        let error = load_source_snapshot(&config)
            .expect_err("service must reject a competing legacy runtime authority");
        assert!(
            error.to_string().contains("legacy"),
            "unexpected error for `{relative}`: {error}"
        );
        assert_eq!(
            fs::read(&legacy_path).expect("read preserved legacy runtime authority"),
            b"legacy-runtime-authority-must-remain"
        );
        assert!(
            source_dir.join("governance-runtime-committed-v1").is_dir(),
            "legacy rejection must not mutate the typed committed store"
        );
    }
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
    let state_dir = root.path().join("state");
    fs::create_dir_all(&state_dir).expect("create segmented source-loader state root");
    let outgoing_config = RuntimeConfig {
        source_root_guard: GovernanceFilesystemRootGuard::capture_source(&source_dir)
            .expect("fence outgoing segmented publisher source root"),
        source_dir: source_dir.clone(),
        state_root_guard: GovernanceFilesystemRootGuard::capture_writer(&state_dir)
            .expect("fence outgoing segmented source-loader state root"),
        listen_addr: "127.0.0.1:0".parse().expect("test address"),
        poll_interval: Duration::from_millis(10),
        max_response_bytes: 1024 * 1024,
        max_request_bytes: 1024 * 1024,
        max_future_skew_secs: 60,
        allow_head_bootstrap: true,
        expected_producer_signer_handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
        expected_producer_signer_qualification: TEST_PRODUCER_SIGNER_QUALIFICATION,
        expected_checkpoint_store_handle: TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
        expected_checkpoint_store_qualification: TEST_STORE_QUALIFICATION,
        expected_publisher_peer_id: publisher_peer_id.clone(),
        expected_public_key: outgoing_public_key,
    };
    let service_store = test_checkpoint_store(Arc::clone(&checkpoint_provider));
    let outgoing_source = load_committed_source_snapshot(&outgoing_config, &service_store)
        .expect("service authenticates the outgoing source before rotation");
    let outgoing_checkpoint = checkpoint_from_source(&outgoing_source);
    let outgoing_intent = intent_from_source(&outgoing_source);
    drop(outgoing_config);
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
        max_future_skew_secs: 60,
        allow_head_bootstrap: true,
        expected_producer_signer_handle: TEST_PRODUCER_SIGNER_HANDLE.to_owned(),
        expected_producer_signer_qualification: TEST_PRODUCER_SIGNER_QUALIFICATION,
        expected_checkpoint_store_handle: TEST_CHECKPOINT_STORE_HANDLE.to_owned(),
        expected_checkpoint_store_qualification: TEST_STORE_QUALIFICATION,
        expected_publisher_peer_id: publisher_peer_id,
        expected_public_key: incoming_public_key,
    };
    let rotated_without_append = load_committed_source_snapshot(&config, &service_store)
        .expect("incoming binding authenticates the outgoing-signed retained tip");
    assert_eq!(rotated_without_append.blocks.len(), 1);
    assert_eq!(
        rotated_without_append.head_bytes,
        outgoing_source.head_bytes
    );
    assert_ne!(
        rotated_without_append.index_blake3,
        outgoing_source.index_blake3
    );
    assert_eq!(
        rotated_without_append.chain_blake3,
        outgoing_source.chain_blake3
    );
    assert_eq!(
        rotated_without_append.head.head_signature.public_key,
        outgoing_public_key.to_vec()
    );
    validate_checkpoint_against_source(Some(&outgoing_checkpoint), &rotated_without_append)
        .expect("service checkpoint continuity survives a provider-only rotation");
    validate_intent_against_source(&outgoing_intent, None, None, &rotated_without_append)
        .expect("active service intent continuity survives a provider-only rotation");
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
    validate_checkpoint_against_source(Some(&outgoing_checkpoint), &segmented)
        .expect("an authenticated checkpoint at N remains valid after source advances to N+1");
    validate_intent_against_source(&outgoing_intent, None, None, &segmented)
        .expect("a sealed target at N remains recoverable after source advances to N+1");
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
    let mut source = signed_source(2, 0x74, current_unix_timestamp_seconds().saturating_sub(10));
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
fn service_checkpoint_and_intent_bind_rotation_stable_signed_head() {
    let source = signed_source(2, 0x75, current_unix_timestamp_seconds().saturating_sub(10));
    let checkpoint = checkpoint_from_source(&source);
    let intent = intent_from_source(&source);
    let mut provider_rotated = source.clone();
    provider_rotated.index_blake3[0] ^= 0x80;
    validate_checkpoint_against_source(Some(&checkpoint), &provider_rotated)
        .expect("provider-only index drift preserves checkpoint chain continuity");
    validate_intent_against_source(&intent, None, None, &provider_rotated)
        .expect("provider-only index drift preserves intent chain continuity");
    let mut substituted_head = source.clone();
    substituted_head.blocks[0].bytes[0] ^= 0x80;
    assert!(
        validate_checkpoint_against_source(Some(&checkpoint), &substituted_head)
            .expect_err("checkpoint continuity must bind the exact signed source chain")
            .to_string()
            .contains("authenticated checkpoint")
    );
    assert!(
        validate_intent_against_source(&intent, None, None, &substituted_head)
            .expect_err("intent continuity must bind the exact signed source chain")
            .to_string()
            .contains("durable publish intent")
    );
}
#[test]
fn active_publish_intent_must_cover_the_exact_unpublished_suffix() {
    let timestamp = current_unix_timestamp_seconds().saturating_sub(10);
    let source = signed_source(3, 0x7b, timestamp);
    let mut omitted_prefix = intent_from_source(&source);
    omitted_prefix.blocks.remove(0);
    let error = validate_intent_against_source(&omitted_prefix, None, None, &source)
        .expect_err("an active intent cannot omit the first unpublished block");
    assert!(
        error
            .to_string()
            .contains("complete unpublished source suffix")
    );
    let predecessor_source = signed_source(1, 0x7b, timestamp);
    let checkpoint = checkpoint_from_source(&predecessor_source);
    let mut successor_intent = intent_from_source(&source);
    successor_intent.generation = checkpoint.generation + 1;
    successor_intent.blocks.remove(0);
    successor_intent.previous_public_head_blake3 = Some(checkpoint.head_bytes_blake3);
    validate_intent_against_source(&successor_intent, Some(&checkpoint), None, &source)
        .expect("an active intent may contain exactly the suffix after its checkpoint");
    let completed_checkpoint = checkpoint_from_source(&source);
    let completed_intent = intent_from_source(&source);
    validate_intent_against_source(
        &completed_intent,
        Some(&completed_checkpoint),
        None,
        &source,
    )
    .expect("completed same-generation recovery validates without replaying publication");
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
        handle: "provider:governance-dag:shared-store-primary".to_owned(),
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
fn mirror_retention_uses_protocol_constants_and_honours_both_caps() {
    let source = signed_source(3, 0x38, 1_800_000_000);
    let intent = intent_from_source(&source);
    let latest = source.blocks[2].bytes.len() as u64;
    let previous = source.blocks[1].bytes.len() as u64;
    let exact_two = latest + previous;
    let retained = retained_source_suffix_with_limits(&source, 2, exact_two)
        .expect("retain exact two-block suffix");
    assert_eq!(retained.len(), 2);
    assert_eq!(retained[0].block.sequence, 1);
    assert_eq!(retained[1].block.sequence, 2);
    let one = retained_source_suffix_with_limits(&source, 1, exact_two)
        .expect("entry cap retains one block");
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].block.sequence, 2);
    let byte_limited = retained_source_suffix_with_limits(&source, 3, exact_two - 1)
        .expect("byte cap retains the newest fitting suffix");
    assert_eq!(byte_limited.len(), 1);
    assert!(retained_source_suffix_with_limits(&source, 3, latest - 1).is_err());
    let protocol_retained = merge_published_blocks(None, &intent, &[], &source)
        .expect("protocol retention keeps this small complete source");
    assert_eq!(protocol_retained.len(), source.blocks.len());
    let prefix = signed_source(1, 0x38, 1_800_000_000);
    assert_eq!(prefix.blocks[0].bytes, source.blocks[0].bytes);
    let checkpoint = checkpoint_from_source(&prefix);
    let mut append_intent = intent_from_source(&source);
    append_intent.generation = checkpoint.generation + 1;
    append_intent.previous_public_head_blake3 = Some(checkpoint.head_bytes_blake3);
    append_intent.blocks.drain(..1);
    let expanded = merge_published_blocks(Some(&checkpoint), &append_intent, &[], &source)
        .expect("append from a one-block checkpoint backfills the complete retained suffix");
    assert_eq!(
        expanded
            .iter()
            .map(|block| block.sequence)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}
#[test]
fn checkpoint_requires_the_exact_protocol_retained_suffix() {
    let source = signed_source(3, 0x39, 1_800_000_000);
    let checkpoint = checkpoint_from_source(&source);
    validate_checkpoint_against_source(Some(&checkpoint), &source)
        .expect("complete protocol-retained suffix validates");
    let mut under_retained = checkpoint;
    under_retained.mirror_blocks.remove(0);
    let error = validate_checkpoint_body(&under_retained)
        .expect_err("a checkpoint cannot leave a gap between its archive and retained mirror");
    assert!(
        error.to_string().contains("one exact block prefix"),
        "unexpected exact-retention error: {error}"
    );
}
#[test]
fn checkpoint_body_rejects_inventory_above_protocol_retention_cap() {
    let source = signed_source(1, 0x3A, 1_800_000_000);
    let mut checkpoint = checkpoint_from_source(&source);
    let prototype = checkpoint.mirror_blocks[0].clone();
    let last_sequence =
        u64::try_from(GOVERNANCE_DAG_MIRROR_MAX_ENTRIES_V1).expect("V1 entry cap fits u64");
    checkpoint.mirror_blocks = (0..=last_sequence)
        .map(|sequence| {
            let mut published = prototype.clone();
            published.sequence = sequence;
            let mut block_cid = [0_u8; 32];
            block_cid[..8].copy_from_slice(&sequence.to_le_bytes());
            published.governance_block_cid = block_cid.to_vec();
            published
        })
        .collect();
    checkpoint.head_block_cid = checkpoint
        .mirror_blocks
        .last()
        .expect("over-cap inventory is nonempty")
        .governance_block_cid
        .clone();
    let error = validate_checkpoint_body(&checkpoint)
        .expect_err("one entry above the protocol retention cap must fail closed");
    assert!(
        error.to_string().contains("first-release bounds"),
        "unexpected over-cap checkpoint error: {error}"
    );
}
#[test]
fn v1_max_retention_encoding_budget_fits_durable_stores() {
    // Canonical pretty JSON uses six spaces for nested block fields, four
    // for lookup rows, and six for payload-kind positions. This per-entry
    // budget covers the eleven block fields, three 64-byte-key lookup rows,
    // and one kind position at the widest V1 numeric/string widths. The
    // fixed allowance covers root/head fields, map framing, and all kind keys.
    const MAX_PAYLOAD_KIND_BYTES: usize = 48;
    const MAX_SUBMISSION_ORIGIN_BYTES: usize = 32;
    const MAX_MIRROR_FIXED_JSON_BYTES: usize = 1024 * 1024;
    const MAX_MIRROR_STORE_WRAPPER_BYTES: usize = 4 * 1024;
    const MAX_SEALED_FIXED_BYTES: usize = 1024 * 1024;
    let quoted = |bytes: usize| bytes.saturating_add(2);
    let block_fields = [
        ("position", 5),
        ("sequence", 20),
        ("timestamp", 20),
        ("payload_kind", quoted(MAX_PAYLOAD_KIND_BYTES)),
        ("block_cid_hex", quoted(64)),
        ("node_cid_hex", quoted(64)),
        ("blake3", quoted(64)),
        (
            "encoded_len",
            GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1
                .to_string()
                .len(),
        ),
        ("ipfs_cid", quoted(59)),
        ("submission_publisher_account_digest_hex", quoted(64)),
        ("submission_origin", quoted(MAX_SUBMISSION_ORIGIN_BYTES)),
    ];
    let block_object_bytes = block_fields
        .iter()
        .map(|(key, value_bytes)| 6 + quoted(key.len()) + 2 + *value_bytes + 2)
        .sum::<usize>()
        // Opening/closing lines, conservatively retaining a trailing comma.
        .saturating_add(13);
    let lookup_row_bytes = 4 + quoted(64) + 2 + 5 + 2;
    let kind_position_bytes = 6 + 5 + 2;
    let mirror_entry_bytes = block_object_bytes
        .saturating_add(lookup_row_bytes.saturating_mul(3))
        .saturating_add(kind_position_bytes);
    let mirror_json_upper = mirror_entry_bytes
        .checked_mul(GOVERNANCE_DAG_MIRROR_MAX_ENTRIES_V1)
        .and_then(|bytes| bytes.checked_add(MAX_MIRROR_FIXED_JSON_BYTES))
        .expect("V1 mirror JSON budget arithmetic cannot overflow");
    assert!(
        mirror_json_upper <= GOVERNANCE_DAG_SERVICE_MUTABLE_STATE_MAX_BYTES_V1 as usize,
        "V1 mirror JSON upper bound {mirror_json_upper} exceeds the durable byte ceiling"
    );
    assert!(
        mirror_json_upper.saturating_add(MAX_MIRROR_STORE_WRAPPER_BYTES)
            <= MIRROR_INDEX_STORE_MAX_PAYLOAD_BYTES,
        "V1 mirror wrapper no longer fits its two-slot store"
    );
    // A standalone Norito frame includes at least as much framing as the
    // same value nested in a vector, so multiplying a widest-string sample
    // frame gives a conservative, allocation-free maximum inventory bound.
    // Submission provenance is not stored in either sealed entry type; it
    // is derived from authenticated source blocks only for the JSON bound above.
    let source = signed_source(1, 0x3B, 1_800_000_000);
    let checkpoint = checkpoint_from_source(&source);
    let mut published_sample = checkpoint.mirror_blocks[0].clone();
    published_sample.sequence = u64::MAX;
    published_sample.timestamp = u64::MAX;
    published_sample.encoded_len = u64::MAX;
    published_sample.payload_kind = "x".repeat(MAX_PAYLOAD_KIND_BYTES);
    let published_frame_bytes = norito::to_bytes(&published_sample)
        .expect("encode maximum-width published-block sizing sample")
        .len();
    let checkpoint_upper = published_frame_bytes
        .checked_mul(GOVERNANCE_DAG_MIRROR_MAX_ENTRIES_V1)
        .and_then(|bytes| bytes.checked_add(MAX_SEALED_FIXED_BYTES))
        .expect("V1 checkpoint budget arithmetic cannot overflow");
    assert!(
        checkpoint_upper <= GOVERNANCE_DAG_SERVICE_MUTABLE_STATE_MAX_BYTES_V1 as usize,
        "V1 checkpoint upper bound {checkpoint_upper} exceeds the sealed-state ceiling"
    );
    let intent = intent_from_source(&source);
    let mut intent_sample = intent.blocks[0].clone();
    intent_sample.sequence = u64::MAX;
    intent_sample.timestamp = u64::MAX;
    intent_sample.encoded_len = u64::MAX;
    intent_sample.payload_kind = "x".repeat(MAX_PAYLOAD_KIND_BYTES);
    let intent_frame_bytes = norito::to_bytes(&intent_sample)
        .expect("encode maximum-width intent-block sizing sample")
        .len();
    let intent_upper = intent_frame_bytes
        .checked_mul(SOURCE_ENTRY_HARD_CAP)
        .and_then(|bytes| bytes.checked_add(MAX_SEALED_FIXED_BYTES))
        .expect("V1 publish-intent budget arithmetic cannot overflow");
    assert!(
        intent_upper <= GOVERNANCE_DAG_SERVICE_MUTABLE_STATE_MAX_BYTES_V1 as usize,
        "V1 publish-intent upper bound {intent_upper} exceeds the sealed-state ceiling"
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
#[test]
fn signed_head_accepts_only_canonical_strong_entity_tags() {
    for valid in ["\"\"", r#""v1""#, r#""!#$%&'()*+-.^_`|~""#] {
        let value = HeaderValue::from_str(valid).expect("valid test header value");
        assert_eq!(strong_http_entity_tag(&value).as_deref(), Some(valid));
    }
    for invalid in ["v1", r#"W/"v1""#, r#""a\"b""#, r#""a b""#] {
        let value = HeaderValue::from_str(invalid).expect("representable invalid ETag");
        assert!(
            strong_http_entity_tag(&value).is_none(),
            "accepted noncanonical ETag {invalid:?}"
        );
    }
    let obs_text = HeaderValue::from_bytes(&[b'"', 0x80, b'"'])
        .expect("HTTP header values can represent obsolete text");
    assert!(strong_http_entity_tag(&obs_text).is_none());
}
#[tokio::test]
async fn routes_reject_noncanonical_identifiers_before_lookup() {
    let telemetry = ApiState(Arc::new(RwLock::new(ApiSnapshot {
        live: true,
        ready: true,
        ..ApiSnapshot::default()
    })));
    let dir = secure_temp_dir();
    let source = signed_source(1, 0x2b, 1_800_000_000);
    let config = test_runtime_config(&source, dir.path());
    let mirror_store = open_mirror_index_store(&config).expect("initialize typed mirror store");
    drop(mirror_store);
    let mirror_reader = GovernanceDagMirrorReadHandleV1::try_new(
        &config,
        test_checkpoint_store(Arc::new(TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE))),
        true,
    )
    .expect("construct bootstrap mirror reader");
    let app = service_router(ServiceApiState {
        telemetry,
        mirror_reader,
    });
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
            calls_for_resolution.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(vec![public, public])
        },
        Duration::from_secs(1),
        false,
    )
    .await
    .expect("one pinned public DNS snapshot");
    assert_eq!(resolved, vec![public]);
    assert_eq!(calls.load(AtomicOrdering::SeqCst), 1);
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
        provider.ingress_binding(),
        provider,
        GovernanceDagAuthenticationScope::Ipfs,
        "IPFS authenticator",
    )
    .expect("bind test authenticator");
    let endpoint = PinnedEndpoint {
        url: Url::parse("http://127.0.0.1:5001/").expect("test URL"),
        client: Client::builder().no_proxy().build().expect("test client"),
        authentication_scope: GovernanceDagAuthenticationScope::Ipfs,
        authenticator: authenticator.clone(),
        authenticated_wire_body_max_bytes: authenticated_ipfs_wire_body_max_bytes(
            GOVERNANCE_DAG_BLOCK_MAX_CANONICAL_BYTES_V1 as u64,
        )
        .expect("derive test authenticated wire-body bound"),
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
    let add_url = endpoint
        .ipfs_url("api/v0/add", IPFS_UNIXFS_V1_ADD_QUERY)
        .expect("construct fixed-profile IPFS add URL");
    let add_pairs = add_url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    assert_eq!(
        add_pairs,
        [
            ("chunker", "size-1048576"),
            ("cid-version", "1"),
            ("hash", "sha2-256"),
            ("max-file-links", "1024"),
            ("pin", "false"),
            ("quieter", "true"),
            ("raw-leaves", "true"),
            ("trickle", "false"),
            ("wrap-with-directory", "false"),
        ]
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
    );
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
        provider.ingress_binding(),
        provider.clone(),
        GovernanceDagAuthenticationScope::Ipfs,
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
    assert_eq!(first.public_key(), provider.ingress_binding().public_key());
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
    assert!(error.to_string().contains("ingress qualification changed"));
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
        drifting_provider.ingress_binding(),
        drifting_provider.clone(),
        GovernanceDagAuthenticationScope::Ipfs,
        "IPFS authenticator",
    )
    .expect("bind stable runtime authenticator");
    drifting_provider
        .drift_during_authentication
        .store(true, AtomicOrdering::SeqCst);
    let error = drifting_authenticator
        .authenticate(&descriptor)
        .expect_err("policy drift during authentication must discard the request");
    assert!(error.to_string().contains("ingress qualification changed"));
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
        provider.ingress_binding(),
        provider,
        GovernanceDagAuthenticationScope::Ipfs,
        "IPFS authenticator",
    )
    .expect_err("mismatched authenticator handle must fail");
    assert!(mismatch.to_string().contains("does not match"));
}
#[test]
fn outbound_nonce_sanity_window_never_exhausts_fresh_request_throughput() {
    let provider = Arc::new(TestAuthenticator::new(
        TEST_IPFS_AUTH_HANDLE,
        "bounded-sender-window",
    ));
    let authenticator = OpaqueAuthenticator::try_new(
        TEST_IPFS_AUTH_HANDLE,
        TEST_AUTH_QUALIFICATION,
        provider.ingress_binding(),
        provider,
        GovernanceDagAuthenticationScope::Ipfs,
        "IPFS authenticator",
    )
    .expect("bind request authenticator");
    let request = canonical_test_request(
        GovernanceDagAuthenticationScope::Ipfs,
        "POST",
        "https://example.invalid/api/v0/pin/add?arg=cid&recursive=true",
        &[("accept-encoding", "identity")],
        b"",
    );
    let oldest = authenticator
        .authenticate(&request)
        .expect("authenticate initial request");
    for _ in 0..GOVERNANCE_DAG_REQUEST_AUTH_REPLAY_CACHE_CAPACITY_V1 {
        authenticator
            .authenticate(&request)
            .expect("fresh outbound nonces must evict rather than exhaust the sender window");
    }
    authenticator
        .validate_envelope(&request, &oldest)
        .expect("sender eviction is not receiver replay authority");
    let now = current_unix_timestamp_seconds();
    let mut window = OutboundRequestNonceWindowV1::new();
    window
        .observe([0xA1; 32], now, now.saturating_sub(1))
        .expect("observe nonce before its expiry");
    window
        .observe([0xA1; 32], now.saturating_add(1), now)
        .expect("an envelope expiring at now is no longer live");
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
        provider.ingress_binding(),
        provider,
        GovernanceDagAuthenticationScope::Ipfs,
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
    assert!(replay.to_string().contains("reused a live outbound nonce"));
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
fn signed_block_prefix_archive_structurally_covers_pruned_prefix() {
    let source = signed_source(5, 0x74, 1_800_000_000);
    let intent = intent_from_source(&source);
    let retained_bytes = source.blocks[3..]
        .iter()
        .map(|block| u64::try_from(block.bytes.len()).expect("block length fits u64"))
        .sum();
    let by_sequence = published_blocks_by_sequence(None, &intent).expect("index published blocks");
    let retained = select_mirror_suffix(&by_sequence, &source, 2, retained_bytes)
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
        "an archive cannot substitute its qualified runtime provider"
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
        "provider:governance-dag:archive-failover-secondary",
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
    let by_sequence = published_blocks_by_sequence(None, &intent).expect("index published blocks");
    let retained = select_mirror_suffix(&by_sequence, &source, 2, retained_bytes)
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
    let by_sequence = published_blocks_by_sequence(None, &intent).expect("index published blocks");
    let retained = select_mirror_suffix(&by_sequence, &source, 2, retained_bytes)
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
fn sealed_request_auth_replay_fences_two_replica_cas_race() {
    let barrier = Arc::new(Barrier::new(2));
    let shared_store = Arc::new(
        TestSealedStore::new(TEST_CHECKPOINT_STORE_HANDLE).with_replay_load_barrier(barrier),
    );
    let first = test_checkpoint_store(shared_store.clone());
    let second = test_checkpoint_store(shared_store);
    let retry = second.clone();
    let now = current_unix_timestamp_seconds();
    let expires_at = now + 15;
    let nonce = [0xA7; 32];
    let slot = request_auth_replay_slot(GovernanceDagAuthenticationScope::Ipfs);
    let first_result = std::thread::spawn(move || {
        consume_sealed_request_auth_nonce(&first, slot, nonce, expires_at, now)
    });
    let second_result = std::thread::spawn(move || {
        consume_sealed_request_auth_nonce(&second, slot, nonce, expires_at, now)
    });
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
    let replay = consume_sealed_request_auth_nonce(&retry, slot, nonce, expires_at, now)
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
