// Runtime DAG provider, staging, filesystem, and signer regressions.
#[test]
fn filesystem_publisher_recovers_checkpoint_cas_applied_response_error() {
    let temp = tempdir().expect("tempdir");
    let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
    {
        let publisher =
            signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
        checkpoint_store
            .fail_after_next_checkpoint_cas
            .store(true, Ordering::SeqCst);
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
    let staging_store = open_runtime_dag_staging_store_v1(temp.path(), publisher.root_guard())
        .expect("open typed staging store after recovery");
    assert!(
        load_runtime_dag_staging_state_v1(&staging_store)
            .expect("load typed staging state")
            .0
            .staged
            .is_none(),
        "committed recovery must clear the exact staged envelope"
    );
    drop(publisher);
}
#[test]
fn runtime_dag_producer_bounds_accept_exact_limits_and_reject_successors() {
    let index_limit = GOVERNANCE_MUTABLE_INDEX_MAX_BYTES;
    let head_limit = GOVERNANCE_RUNTIME_DAG_HEAD_MAX_BYTES_V1;
    let block_limit = GOVERNANCE_RUNTIME_DAG_BLOCK_MAX_BYTES;
    validate_runtime_dag_producer_file_lengths(block_limit, head_limit, index_limit)
        .expect("exact per-file limits are accepted");
    assert!(
        validate_runtime_dag_producer_file_lengths(block_limit + 1, 1, 1).is_err(),
        "block limit + 1 must fail before sealing"
    );
    assert!(
        validate_runtime_dag_producer_file_lengths(1, head_limit + 1, 1).is_err(),
        "head limit + 1 must fail before sealing"
    );
    assert!(
        validate_runtime_dag_producer_file_lengths(1, 1, index_limit + 1).is_err(),
        "index limit + 1 must fail before sealing"
    );
    let staging_upper_bound = block_limit
        .checked_add(head_limit)
        .and_then(|bytes| bytes.checked_add(index_limit))
        .and_then(|bytes| {
            bytes.checked_add(GOVERNANCE_RUNTIME_DAG_PRODUCER_INTENT_SEALED_MAX_BYTES_V1)
        })
        .expect("staging bound arithmetic");
    assert!(
        staging_upper_bound < governance_rooted_fs::TWO_SLOT_MAX_PAYLOAD_BYTES_V1,
        "the fixed store ceiling must admit one maximum valid staging transaction plus its intent"
    );
    assert!(
        head_limit + index_limit + 64 * 1024 < GOVERNANCE_RUNTIME_DAG_COMMITTED_STATE_MAX_BYTES_V1,
        "the committed mirror must admit maximum head/index bytes plus codec overhead"
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
    let staging_store = open_runtime_dag_staging_store_v1(temp.path(), &root_guard)
        .expect("open typed staging store");
    let (mut state, snapshot) =
        load_runtime_dag_staging_state_v1(&staging_store).expect("load typed staging state");
    state
        .staged
        .as_mut()
        .expect("staged envelope")
        .transaction
        .index_bytes[0] ^= 0x80;
    let substituted = encode_governance_two_slot_value_v1(
        &state,
        "substituted governance runtime DAG staging state",
    )
    .expect("encode substituted staging state");
    compare_and_swap_governance_two_slot_store_v1(
        &staging_store,
        &snapshot,
        &substituted,
        "substituted governance runtime DAG staging state",
    )
    .expect("install authenticated-store substitution for recovery test");
    let error = FilesystemGovernancePublisher::try_new(temp.path().to_path_buf())
        .expect("reopen publisher root")
        .with_qualified_runtime_dag_providers(
            qualified_test_runtime_dag_signer(1, 0x31),
            qualified_test_runtime_dag_checkpoint_store(Arc::clone(&checkpoint_store)),
        )
        .expect_err("staged index substitution must fail closed");
    assert!(
        error.to_string().contains("bytes or path are substituted")
            || error.to_string().contains("staged index")
    );
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
fn runtime_dag_staging_store_is_created_through_the_retained_root() {
    let temp = tempdir().expect("tempdir");
    let publisher =
        FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
    let store = open_runtime_dag_staging_store_v1(temp.path(), publisher.root_guard())
        .expect("create typed producer staging store");
    assert!(
        load_runtime_dag_staging_state_v1(&store)
            .expect("load initial typed staging state")
            .0
            .staged
            .is_none()
    );
    assert!(
        !temp
            .path()
            .join(GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGING_DIR)
            .exists(),
        "the retired mutable staging directory must not be recreated"
    );
}
#[test]
fn authenticated_runtime_dag_reader_is_read_only_for_genesis_and_committed_state() {
    let genesis = tempdir().expect("genesis tempdir");
    let genesis_publisher = signed_runtime_publisher(genesis.path());
    let genesis_signer = genesis_publisher
        .runtime_dag_signer
        .as_ref()
        .expect("genesis signer")
        .clone();
    let genesis_store = genesis_publisher
        .runtime_dag_checkpoint_store
        .as_ref()
        .expect("genesis checkpoint store")
        .clone();
    drop(genesis_publisher);
    let genesis_reader =
        GovernanceFilesystemRootGuard::capture_source(genesis.path()).expect("retain genesis root");
    let genesis_inventory = filesystem_inventory_fixture(genesis.path());
    assert!(
        load_authenticated_runtime_dag_snapshot_v1(
            &genesis_reader,
            &genesis_signer,
            &genesis_store,
        )
        .expect("authenticate genesis")
        .is_none()
    );
    assert_eq!(
        filesystem_inventory_fixture(genesis.path()),
        genesis_inventory,
        "authenticated genesis read must not create or remove filesystem entries"
    );
    let committed = tempdir().expect("committed tempdir");
    let committed_publisher = signed_runtime_publisher(committed.path());
    let (settlement, encoded) = sample_settlement();
    committed_publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("publish committed runtime DAG");
    let signer = committed_publisher
        .runtime_dag_signer
        .as_ref()
        .expect("committed signer")
        .clone();
    let store = committed_publisher
        .runtime_dag_checkpoint_store
        .as_ref()
        .expect("committed checkpoint store")
        .clone();
    drop(committed_publisher);
    let reader = GovernanceFilesystemRootGuard::capture_source(committed.path())
        .expect("retain committed root");
    let before = filesystem_inventory_fixture(committed.path());
    let snapshot = load_authenticated_runtime_dag_snapshot_v1(&reader, &signer, &store)
        .expect("authenticate committed runtime DAG")
        .expect("non-genesis snapshot");
    assert_eq!(snapshot.head_bytes(), runtime_head_bytes(committed.path()));
    assert_eq!(
        snapshot.index_bytes(),
        json::to_json_pretty(&runtime_index(committed.path()))
            .expect("encode runtime index")
            .as_bytes()
    );
    assert_ne!(snapshot.store_identity().1, [0; 32]);
    assert_ne!(snapshot.checkpoint_identity().1, [0; 32]);
    assert_eq!(
        filesystem_inventory_fixture(committed.path()),
        before,
        "authenticated committed read must not create or remove filesystem entries"
    );
}
#[test]
fn authenticated_runtime_dag_reader_rejects_active_intent_and_substitutions() {
    let temp = tempdir().expect("tempdir");
    let (publisher, signer_provider, checkpoint_provider) =
        signed_runtime_publisher_with_observable_providers(temp.path());
    let (settlement, encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("publish committed runtime DAG");
    let signer = publisher
        .runtime_dag_signer
        .as_ref()
        .expect("runtime signer")
        .clone();
    let store = publisher
        .runtime_dag_checkpoint_store
        .as_ref()
        .expect("runtime checkpoint store")
        .clone();
    drop(publisher);
    let reader =
        GovernanceFilesystemRootGuard::capture_source(temp.path()).expect("retain committed root");
    load_authenticated_runtime_dag_snapshot_v1(&reader, &signer, &store)
        .expect("baseline authenticated read")
        .expect("baseline snapshot");
    let stable_checkpoint_record = checkpoint_provider
        .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)
        .expect("load stable producer checkpoint")
        .expect("stable producer checkpoint exists");
    let mut moved_checkpoint =
        decode_runtime_dag_unqualified_checkpoint_record(&stable_checkpoint_record, temp.path())
            .expect("decode stable producer checkpoint");
    moved_checkpoint.index_bytes_digest[0] ^= 0x40;
    checkpoint_provider.return_producer_checkpoint_on_second_load(
        runtime_dag_producer_checkpoint_record(&moved_checkpoint)
            .expect("encode moved producer checkpoint"),
    );
    let error = load_authenticated_runtime_dag_snapshot_v1(&reader, &signer, &store)
        .expect_err("checkpoint A/B movement must fail closed");
    assert!(error.to_string().contains("changed during read"));
    let active_intent = GovernanceDagSealedStateRecord::new(
        GovernanceDagSealedStateSlot::ProducerPublishIntent,
        u64::MAX,
        b"active test producer intent".to_vec(),
    );
    {
        let mut state = checkpoint_provider
            .state
            .lock()
            .expect("lock checkpoint fixture");
        state.records[TestRuntimeDagCheckpointStore::slot_index(
            GovernanceDagSealedStateSlot::ProducerPublishIntent,
        )] = Some(active_intent);
    }
    let error = load_authenticated_runtime_dag_snapshot_v1(&reader, &signer, &store)
        .expect_err("active producer intent must block reads");
    assert!(error.to_string().contains("transaction is active"));
    checkpoint_provider
        .state
        .lock()
        .expect("lock checkpoint fixture")
        .records[TestRuntimeDagCheckpointStore::slot_index(
        GovernanceDagSealedStateSlot::ProducerPublishIntent,
    )] = None;
    let checkpoint_index =
        TestRuntimeDagCheckpointStore::slot_index(GovernanceDagSealedStateSlot::ProducerCheckpoint);
    let original_checkpoint_record = checkpoint_provider
        .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)
        .expect("load producer checkpoint")
        .expect("producer checkpoint exists");
    let mut substituted_checkpoint =
        decode_runtime_dag_unqualified_checkpoint_record(&original_checkpoint_record, temp.path())
            .expect("decode producer checkpoint");
    substituted_checkpoint.index_bytes_digest[0] ^= 0x80;
    checkpoint_provider
        .state
        .lock()
        .expect("lock checkpoint fixture")
        .records[checkpoint_index] = Some(
        runtime_dag_producer_checkpoint_record(&substituted_checkpoint)
            .expect("encode substituted checkpoint"),
    );
    let error = load_authenticated_runtime_dag_snapshot_v1(&reader, &signer, &store)
        .expect_err("checkpoint substitution must fail closed");
    assert!(
        error
            .to_string()
            .contains("does not match its sealed producer checkpoint")
    );
    checkpoint_provider
        .state
        .lock()
        .expect("lock checkpoint fixture")
        .records[checkpoint_index] = Some(original_checkpoint_record);
    let writer_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
        .expect("retain writer root for typed substitution fixture");
    let committed_store = open_runtime_dag_committed_store_v1(temp.path(), &writer_guard)
        .expect("open committed runtime DAG store");
    let (original_committed, committed_snapshot) =
        load_runtime_dag_committed_state_v1(&committed_store)
            .expect("load committed runtime DAG fixture");
    let mut malformed_committed = original_committed.clone();
    malformed_committed.index_bytes = Some(vec![b'{'; 1024]);
    let malformed_bytes = encode_governance_two_slot_value_v1(
        &malformed_committed,
        "malformed authenticated runtime DAG fixture",
    )
    .expect("encode malformed authenticated runtime DAG fixture");
    compare_and_swap_governance_two_slot_store_v1(
        &committed_store,
        &committed_snapshot,
        &malformed_bytes,
        "malformed authenticated runtime DAG fixture",
    )
    .expect("commit malformed authenticated runtime DAG fixture");
    let error = load_authenticated_runtime_dag_snapshot_v1(&reader, &signer, &store)
        .expect_err("typed index mutation must fail before JSON parsing");
    assert!(error.to_string().contains("byte generation"));
    let (_, malformed_snapshot) = load_runtime_dag_committed_state_v1(&committed_store)
        .expect("load malformed committed runtime DAG fixture");
    let original_committed_bytes = encode_governance_two_slot_value_v1(
        &original_committed,
        "restored authenticated runtime DAG fixture",
    )
    .expect("encode restored authenticated runtime DAG fixture");
    compare_and_swap_governance_two_slot_store_v1(
        &committed_store,
        &malformed_snapshot,
        &original_committed_bytes,
        "restored authenticated runtime DAG fixture",
    )
    .expect("restore authenticated runtime DAG fixture");
    drop(committed_store);
    drop(writer_guard);
    let writer_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
        .expect("retain writer root for semantic substitution fixture");
    let committed_store = open_runtime_dag_committed_store_v1(temp.path(), &writer_guard)
        .expect("open committed runtime DAG store");
    let (original_committed, committed_snapshot) =
        load_runtime_dag_committed_state_v1(&committed_store)
            .expect("load semantic runtime DAG fixture");
    let mut inconsistent_index: JsonValue = json::from_slice(
        original_committed
            .index_bytes
            .as_deref()
            .expect("committed index bytes"),
    )
    .expect("decode semantic runtime DAG fixture");
    inconsistent_index
        .as_object_mut()
        .expect("runtime index object")
        .insert("by_payload_kind".into(), JsonValue::Object(JsonMap::new()));
    let inconsistent_index_bytes = json::to_json_pretty(&inconsistent_index)
        .expect("encode inconsistent reverse map")
        .into_bytes();
    let mut inconsistent_committed = original_committed.clone();
    inconsistent_committed.index_bytes = Some(inconsistent_index_bytes.clone());
    let inconsistent_committed_bytes = encode_governance_two_slot_value_v1(
        &inconsistent_committed,
        "inconsistent authenticated runtime DAG fixture",
    )
    .expect("encode inconsistent runtime DAG fixture");
    compare_and_swap_governance_two_slot_store_v1(
        &committed_store,
        &committed_snapshot,
        &inconsistent_committed_bytes,
        "inconsistent authenticated runtime DAG fixture",
    )
    .expect("commit inconsistent runtime DAG fixture");
    let original_checkpoint_record = checkpoint_provider
        .load(GovernanceDagSealedStateSlot::ProducerCheckpoint)
        .expect("load original producer checkpoint")
        .expect("original producer checkpoint exists");
    let mut inconsistent_checkpoint =
        decode_runtime_dag_unqualified_checkpoint_record(&original_checkpoint_record, temp.path())
            .expect("decode original producer checkpoint");
    inconsistent_checkpoint.index_bytes_digest =
        *blake3::hash(&inconsistent_index_bytes).as_bytes();
    checkpoint_provider
        .state
        .lock()
        .expect("lock checkpoint fixture")
        .records[checkpoint_index] = Some(
        runtime_dag_producer_checkpoint_record(&inconsistent_checkpoint)
            .expect("encode inconsistent producer checkpoint"),
    );
    let error = load_authenticated_runtime_dag_snapshot_v1(&reader, &signer, &store)
        .expect_err("checkpoint-authenticated reverse-map drift must fail closed");
    assert!(error.to_string().contains("reverse map `by_payload_kind`"));
    checkpoint_provider
        .state
        .lock()
        .expect("lock checkpoint fixture")
        .records[checkpoint_index] = Some(original_checkpoint_record);
    let (_, inconsistent_snapshot) = load_runtime_dag_committed_state_v1(&committed_store)
        .expect("load inconsistent committed runtime DAG fixture");
    let original_committed_bytes = encode_governance_two_slot_value_v1(
        &original_committed,
        "restored semantic runtime DAG fixture",
    )
    .expect("encode restored semantic runtime DAG fixture");
    compare_and_swap_governance_two_slot_store_v1(
        &committed_store,
        &inconsistent_snapshot,
        &original_committed_bytes,
        "restored semantic runtime DAG fixture",
    )
    .expect("restore semantic runtime DAG fixture");
    drop(committed_store);
    drop(writer_guard);
    let index = runtime_index(temp.path());
    let block_path = index
        .get("blocks")
        .and_then(JsonValue::as_array)
        .and_then(|blocks| blocks.first())
        .and_then(|block| block.get("block_path"))
        .and_then(JsonValue::as_str)
        .and_then(|path| resolve_index_path(temp.path(), path).ok())
        .expect("first runtime block path");
    let json_path = index
        .get("blocks")
        .and_then(JsonValue::as_array)
        .and_then(|blocks| blocks.first())
        .and_then(|block| block.get("json_path"))
        .and_then(JsonValue::as_str)
        .and_then(|path| resolve_index_path(temp.path(), path).ok())
        .expect("first runtime JSON source path");
    let original_json = fs::read(&json_path).expect("read original runtime JSON source");
    let mut substituted_json = original_json.clone();
    substituted_json.push(b' ');
    fs::write(&json_path, &substituted_json).expect("substitute runtime JSON source fixture");
    fs::write(
        digest_sidecar_path_for(&json_path),
        format!("{}\n", blake3::hash(&substituted_json).to_hex()),
    )
    .expect("bind substituted runtime JSON sidecar");
    let error = load_authenticated_runtime_dag_snapshot_v1(&reader, &signer, &store)
        .expect_err("source-pair identity drift must fail closed");
    assert!(error.to_string().contains("source paths do not bind"));
    fs::write(&json_path, &original_json).expect("restore runtime JSON source");
    fs::write(
        digest_sidecar_path_for(&json_path),
        format!("{}\n", blake3::hash(&original_json).to_hex()),
    )
    .expect("restore runtime JSON sidecar");
    let original_block = fs::read(&block_path).expect("read original runtime block");
    fs::write(&block_path, b"substituted runtime block")
        .expect("substitute immutable runtime block fixture");
    let error = load_authenticated_runtime_dag_snapshot_v1(&reader, &signer, &store)
        .expect_err("block substitution must fail closed");
    assert!(
        error.to_string().contains("digest sidecar")
            || error.to_string().contains("block length or digest")
    );
    fs::write(&block_path, original_block).expect("restore runtime block fixture");
    signer_provider
        .qualification_revision
        .store(2, Ordering::SeqCst);
    let error = load_authenticated_runtime_dag_snapshot_v1(&reader, &signer, &store)
        .expect_err("provider qualification drift must fail closed");
    assert!(
        error
            .to_string()
            .contains("signer identity or policy changed after injection")
    );
}
#[test]
fn authenticated_runtime_dag_genesis_rejects_orphan_immutable_inventory() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());
    let signer = publisher
        .runtime_dag_signer
        .as_ref()
        .expect("genesis signer")
        .clone();
    let store = publisher
        .runtime_dag_checkpoint_store
        .as_ref()
        .expect("genesis checkpoint store")
        .clone();
    drop(publisher);
    fs::create_dir(temp.path().join(GOVERNANCE_RUNTIME_DAG_DIR))
        .expect("seed orphan runtime directory");
    let reader =
        GovernanceFilesystemRootGuard::capture_source(temp.path()).expect("retain genesis root");
    let error = load_authenticated_runtime_dag_snapshot_v1(&reader, &signer, &store)
        .expect_err("orphan immutable inventory must fail closed");
    assert!(error.to_string().contains("unindexed immutable artifacts"));
}
#[test]
fn authenticated_runtime_dag_genesis_authenticates_pre_block_provider_rotation() {
    let temp = tempdir().expect("tempdir");
    let checkpoint_provider = Arc::new(TestRuntimeDagCheckpointStore::default());
    let mut publisher =
        signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_provider));
    let next_store = publisher
        .runtime_dag_checkpoint_store
        .as_ref()
        .expect("checkpoint store")
        .clone();
    publisher
        .transition_qualified_runtime_dag_providers(
            qualified_test_runtime_dag_signer(2, 0x32),
            next_store,
        )
        .expect("rotate qualified provider before the first block");
    let signer = publisher
        .runtime_dag_signer
        .as_ref()
        .expect("rotated signer")
        .clone();
    let store = publisher
        .runtime_dag_checkpoint_store
        .as_ref()
        .expect("rotated checkpoint store")
        .clone();
    drop(publisher);
    let reader = GovernanceFilesystemRootGuard::capture_source(temp.path())
        .expect("retain rotated genesis root");
    assert!(
        load_authenticated_runtime_dag_snapshot_v1(&reader, &signer, &store)
            .expect("authenticate rotated genesis")
            .is_none()
    );
    let archives = temp
        .path()
        .join(GOVERNANCE_RUNTIME_DAG_QUALIFICATION_ARCHIVES_DIR);
    fs::create_dir(&archives).expect("create unindexed archive inventory");
    fs::write(
        archives.join("unindexed.to"),
        b"unindexed qualification fork",
    )
    .expect("seed unindexed archive");
    let error = load_authenticated_runtime_dag_snapshot_v1(&reader, &signer, &store)
        .expect_err("unindexed qualification inventory must fail closed");
    assert!(error.to_string().contains("unindexed"));
    fs::remove_dir_all(&archives).expect("remove unindexed archive fixture");
    fs::remove_dir_all(
        temp.path()
            .join(GOVERNANCE_RUNTIME_DAG_QUALIFICATION_STORE_DIR_V1),
    )
    .expect("remove typed qualification history fixture");
    let error = load_authenticated_runtime_dag_snapshot_v1(&reader, &signer, &store)
        .expect_err("missing rotated qualification history must fail closed");
    assert!(error.to_string().contains("authority lineage diverges"));
}
#[test]
fn runtime_dag_staging_transaction_survives_ambiguous_cycle_and_clears_on_restart() {
    let temp = tempdir().expect("tempdir");
    let checkpoint_store = Arc::new(TestRuntimeDagCheckpointStore::default());
    let publisher = signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
    let (first, first_encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&first, &first_encoded)
        .expect("publish first staging cycle");
    let mut successor = first;
    successor.deal_id = [0xA6; 32];
    successor.ledger.deal_id = successor.deal_id;
    successor.ledger.snapshot_id = successor
        .ledger
        .derive_snapshot_id()
        .expect("reseal successor ledger snapshot");
    successor.settlement_id = successor
        .derive_settlement_id()
        .expect("reseal successor settlement");
    let successor_encoded = norito::to_bytes(&successor).expect("encode successor settlement");
    checkpoint_store
        .fail_after_next_intent_cas
        .store(true, Ordering::SeqCst);
    publisher
        .publish_deal_settlement(&successor, &successor_encoded)
        .expect_err("retain the second sealed staging cycle");
    let intent_record = checkpoint_store
        .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
        .expect("load second-cycle intent")
        .expect("second-cycle intent exists");
    let intent: RuntimeDagProducerPublishIntentV1 =
        norito::decode_from_bytes(&intent_record.payload).expect("decode second-cycle intent");
    load_runtime_dag_producer_staged_transaction(temp.path(), publisher.root_guard(), &intent)
        .expect("typed staging state retains the exact sealed transaction");
    let staging_store = open_runtime_dag_staging_store_v1(temp.path(), publisher.root_guard())
        .expect("open typed staging state");
    assert!(
        load_runtime_dag_staging_state_v1(&staging_store)
            .expect("load typed staging state")
            .0
            .staged
            .is_some()
    );
    drop(staging_store);
    drop(publisher);
    let restarted = signed_runtime_publisher_with_store(temp.path(), Arc::clone(&checkpoint_store));
    let staging_store = open_runtime_dag_staging_store_v1(temp.path(), restarted.root_guard())
        .expect("reopen typed staging state after intent recovery");
    assert!(
        load_runtime_dag_staging_state_v1(&staging_store)
            .expect("load recovered typed staging state")
            .0
            .staged
            .is_none(),
        "the staged transaction is cleared only after the sealed intent completes"
    );
    assert!(
        !temp
            .path()
            .join(GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGING_DIR)
            .exists(),
        "the retired mutable staging directory must not be recreated"
    );
    assert_eq!(
        runtime_index(temp.path())
            .get("block_count")
            .and_then(JsonValue::as_u64),
        Some(2)
    );
    assert!(
        checkpoint_store
            .load(GovernanceDagSealedStateSlot::ProducerPublishIntent)
            .expect("reload second-cycle intent")
            .is_none()
    );
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
fn runtime_dag_audit_rejects_substituted_generated_at_in_committed_state() {
    let temp = tempdir().expect("tempdir");
    let publisher = signed_runtime_publisher(temp.path());
    let (settlement, encoded) = sample_settlement();
    publisher
        .publish_deal_settlement(&settlement, &encoded)
        .expect("seed signed runtime DAG");
    let store = open_runtime_dag_committed_store_v1(temp.path(), publisher.root_guard())
        .expect("open committed runtime DAG state");
    let (mut committed, snapshot) =
        load_runtime_dag_committed_state_v1(&store).expect("load committed runtime DAG state");
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
    committed.index_bytes = Some(bytes);
    let encoded =
        encode_governance_two_slot_value_v1(&committed, "tampered committed runtime DAG state")
            .expect("encode tampered committed state");
    compare_and_swap_governance_two_slot_store_v1(
        &store,
        &snapshot,
        &encoded,
        "tampered committed runtime DAG state",
    )
    .expect("commit internally coherent but semantically substituted state");
    drop(store);
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
#[test]
fn runtime_dag_store_rejects_legacy_atomic_temp_without_online_cleanup() {
    for legacy_name in [
        format!(".{GOVERNANCE_RUNTIME_DAG_INDEX_FILE}.tmp-42000-1"),
        format!(".{GOVERNANCE_RUNTIME_DAG_INDEX_FILE}.tmp-bad"),
        format!(".{GOVERNANCE_RUNTIME_DAG_INDEX_FILE}.retained-v1-bad"),
    ] {
        let temp = tempdir().expect("tempdir");
        let stale = temp.path().join(&legacy_name);
        fs::write(&stale, b"legacy-mutable-state").expect("seed legacy crash temp");
        let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
            .expect("retain producer root");
        let error = open_runtime_dag_committed_store_v1(temp.path(), &root_guard)
            .expect_err("legacy crash temp must fail closed");
        assert!(error.to_string().contains("legacy"));
        assert!(
            stale.exists(),
            "online startup must not silently delete legacy authority"
        );
        assert!(
            !temp
                .path()
                .join(GOVERNANCE_RUNTIME_DAG_COMMITTED_STORE_DIR_V1)
                .exists(),
            "legacy rejection must precede typed-store creation for `{legacy_name}`"
        );
    }
}
#[test]
fn runtime_dag_store_rejects_legacy_staging_directory_without_online_cleanup() {
    let temp = tempdir().expect("tempdir");
    let legacy = temp
        .path()
        .join(GOVERNANCE_RUNTIME_DAG_PRODUCER_STAGING_DIR);
    fs::create_dir(&legacy).expect("seed legacy staging directory");
    let root_guard =
        GovernanceFilesystemRootGuard::capture_writer(temp.path()).expect("retain producer root");
    let error = open_runtime_dag_staging_store_v1(temp.path(), &root_guard)
        .expect_err("legacy staging directory must fail closed");
    assert!(error.to_string().contains("legacy mutable"));
    assert!(
        legacy.is_dir(),
        "online startup must not delete the retired staging authority"
    );
}
#[test]
fn runtime_dag_store_rejects_legacy_nested_head_generation_without_online_cleanup() {
    let temp = tempdir().expect("tempdir");
    let runtime = temp.path().join(GOVERNANCE_RUNTIME_DAG_DIR);
    fs::create_dir(&runtime).expect("seed legacy runtime directory");
    let legacy = runtime.join(format!(
        ".{GOVERNANCE_RUNTIME_DAG_HEAD_FILE}.retained-v1-0000"
    ));
    fs::write(&legacy, b"legacy-head-generation").expect("seed retained legacy head");
    let root_guard =
        GovernanceFilesystemRootGuard::capture_writer(temp.path()).expect("retain producer root");
    let error = open_runtime_dag_committed_store_v1(temp.path(), &root_guard)
        .expect_err("legacy retained head must fail closed");
    assert!(error.to_string().contains("legacy"));
    assert!(
        legacy.is_file(),
        "online startup must not delete a retired head generation"
    );
}
#[test]
fn qualification_store_rejects_legacy_history_without_online_cleanup() {
    let temp = tempdir().expect("tempdir");
    let legacy = runtime_dag_qualification_history_path(temp.path());
    fs::write(&legacy, b"legacy-qualification-history").expect("seed legacy qualification history");
    let root_guard = GovernanceFilesystemRootGuard::capture_writer(temp.path())
        .expect("retain qualification root");
    let error = open_runtime_dag_qualification_store_v1(temp.path(), &root_guard)
        .expect_err("legacy qualification history must fail closed");
    assert!(error.to_string().contains("legacy mutable"));
    assert!(
        legacy.is_file(),
        "online startup must not delete retired qualification history"
    );
}
#[test]
fn qualification_archive_adjacency_rejects_u64_exhaustion() {
    assert!(runtime_dag_generation_immediately_precedes(41, 42));
    assert!(
        !runtime_dag_generation_immediately_precedes(u64::MAX, 0),
        "an exhausted archived generation must not wrap into a valid successor"
    );
}
#[test]
fn fenced_privacy_store_rejects_legacy_pending_state_without_online_cleanup() {
    let temp = tempdir().expect("tempdir");
    let legacy = fenced_privacy_pending_path(temp.path());
    fs::write(&legacy, b"legacy-pending-request").expect("seed legacy pending state");
    let error = open_fenced_privacy_store_v1(temp.path())
        .expect_err("legacy pending state must fail closed");
    assert!(error.to_string().contains("legacy mutable"));
    assert!(
        legacy.is_file(),
        "online startup must not delete retired privacy authority"
    );
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
    let message = error.to_string();
    assert!(
        message.contains("symlink")
            || message.contains("symbolic link")
            || message.contains("legacy mutable")
            || message.contains("real directory"),
        "unexpected substituted-parent diagnostic: {error}"
    );
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
        .sign(
            GovernanceDagSigningPurposeV1::LogNode,
            b"canonical governance payload",
        )
        .expect_err("provider policy drift must fail closed");
    assert!(error.to_string().contains("policy changed"));
    assert_eq!(signer.observed_purpose(), None);
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
        .sign(
            GovernanceDagSigningPurposeV1::LogNode,
            b"canonical governance payload",
        )
        .expect_err("provider policy drift during signing must discard the signature");
    assert!(error.to_string().contains("policy changed"));
    assert_eq!(
        signer.observed_purpose(),
        Some(GovernanceDagSigningPurposeV1::LogNode)
    );
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
        refusing.clone(),
    )
    .expect("bind refusing test provider");
    let error = wrapped
        .sign(
            GovernanceDagSigningPurposeV1::LogNode,
            b"canonical governance payload",
        )
        .expect_err("provider outage must fail closed");
    assert!(error.to_string().contains("refused"));
    assert!(!error.to_string().contains("must-never-escape"));
    assert_eq!(
        refusing.observed_purpose(),
        Some(GovernanceDagSigningPurposeV1::LogNode)
    );
    let mut corrupt = TestRuntimeDagSigner::new("pkcs11:governance-dag:primary", &peer_id, 0x31);
    corrupt.corrupt_signature = true;
    let corrupt = Arc::new(corrupt);
    let wrapped = GovernanceRuntimeDagSigner::try_new(
        corrupt.handle().to_owned(),
        peer_id,
        corrupt.public_key(),
        test_runtime_dag_signer_qualification(),
        corrupt.clone(),
    )
    .expect("bind corrupt test provider");
    let error = wrapped
        .sign(
            GovernanceDagSigningPurposeV1::LogNode,
            b"canonical governance payload",
        )
        .expect_err("wrong signature must fail closed");
    assert!(error.to_string().contains("another key or payload"));
    assert_eq!(
        corrupt.observed_purpose(),
        Some(GovernanceDagSigningPurposeV1::LogNode)
    );
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
