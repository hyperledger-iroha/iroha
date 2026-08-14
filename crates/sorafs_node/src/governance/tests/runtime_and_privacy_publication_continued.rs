// Signed runtime DAG, privacy publication, and appeal-finance regressions.
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
    let head_bytes = runtime_head_bytes(temp.path());
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
    let head_bytes = runtime_head_bytes(temp.path());
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
    let head_bytes = runtime_head_bytes(temp.path());
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
    let head_bytes = runtime_head_bytes(temp.path());
    let head: GovernanceDagHeadV1 =
        norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
    let blocks = runtime_blocks_from_index(temp.path(), &runtime_index);
    validate_governance_dag_head_against_chain_v1(&head, &blocks)
        .expect("runtime head validates against signed blocks");
    assert_eq!(blocks.len(), 1);
    assert!(
        blocks[0].node.submission_provenance.is_none(),
        "moderation ballot events have no authenticated-submission provenance input"
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
    assert_eq!(
        read_fenced_privacy_head_sync(failed_root.path()).expect("read failed bootstrap state"),
        None
    );
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
    assert_eq!(
        read_fenced_privacy_head_sync(malformed_root.path())
            .expect("read malformed bootstrap state"),
        None
    );
}
#[test]
fn fenced_privacy_pending_clear_is_typed_and_idempotent() {
    let temp = tempdir().expect("tempdir");
    let provider = Arc::new(TestFencedTransparencyPublisher::new());
    let publisher = qualified_test_fenced_publisher(provider);
    let request = sample_fenced_request(7, None);
    let pending = FencedPrivacyPendingRequestV1::from_request(&request, &publisher)
        .expect("build pending request");
    write_fenced_privacy_pending_request(temp.path(), &pending).expect("persist pending");
    assert_eq!(
        read_fenced_privacy_pending_request(temp.path()).expect("read pending"),
        Some(pending)
    );
    remove_fenced_privacy_pending_request(temp.path()).expect("clear pending request");
    assert_fenced_privacy_pending_logically_cleared(temp.path());
    remove_fenced_privacy_pending_request(temp.path()).expect("repeat pending clear");
    assert_fenced_privacy_pending_logically_cleared(temp.path());
    assert!(!fenced_privacy_pending_path(temp.path()).exists());
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
    update_fenced_privacy_state_v1(
        temp.path(),
        "clear retired writer-bound fenced privacy records",
        |state| {
            state.pending = None;
            state.publication_cache = None;
        },
    )
    .expect("clear retired writer-bound records before reader-binding check");
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
    assert_eq!(
        read_fenced_privacy_pending_request(temp.path()).expect("read pending request"),
        None
    );
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
    assert_eq!(
        read_fenced_privacy_head_sync(temp.path()).expect("read rejected head sync state"),
        None
    );
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
    assert_fenced_privacy_pending_logically_cleared(temp.path());
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
    assert_eq!(
        read_fenced_privacy_pending_request(temp.path()).expect("read pending request"),
        None
    );
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
    assert_eq!(
        read_fenced_privacy_head_sync(temp.path()).expect("read head sync state"),
        None
    );
    assert_eq!(
        read_fenced_privacy_pending_request(temp.path()).expect("read pending request"),
        None
    );
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
        read_fenced_privacy_pending_request(temp.path())
            .expect("read ambiguous pending request")
            .is_some(),
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
    assert_fenced_privacy_pending_logically_cleared(temp.path());
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
    assert_fenced_privacy_pending_logically_cleared(later_anchor_root.path());
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
    assert_fenced_privacy_pending_logically_cleared(stale_root.path());
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
    let head_bytes = runtime_head_bytes(temp.path());
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
    let other_key = PublicKey::from_bytes(Algorithm::Ed25519, &[0x97; 32])
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
    let head_bytes = runtime_head_bytes(temp.path());
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
    let head_bytes = runtime_head_bytes(temp.path());
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
