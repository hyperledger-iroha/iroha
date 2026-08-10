#[test]
fn lane_block_application_receipt_persists_canonical_results_and_reloads() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let mut block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    )
    .as_ref()
    .clone();
    attach_ok_results_to_block(&mut block);
    let block_hash = block.hash();
    let block_height = block.header().height().get();
    let expected_result = block.results().next().expect("dummy block result").clone();
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(Arc::new(block))
        .expect("store block with lane artifact and results");
    kura.persist_lane_block_application_receipt(&proposal)
        .expect("persist lane application receipt");
    kura.persist_lane_block_application_receipt(&proposal)
        .expect("duplicate lane application receipt persistence is idempotent");

    let receipt = kura
        .read_lane_block_application_receipt(lane_id, lane_block_height)
        .expect("lane application receipt");
    assert_eq!(receipt.format_label(), "lane.application_receipt");
    assert_eq!(receipt.proposal, proposal);
    assert_eq!(receipt.artifact.ownership, ownership);
    assert_eq!(receipt.application_block_height, block_height);
    assert_eq!(receipt.application_block_hash, block_hash);
    assert_eq!(
        receipt.entrypoint_indices,
        proposal.descriptor.accepted_candidate_indices
    );
    assert_eq!(
        receipt.entrypoint_hashes,
        proposal.descriptor.accepted_transaction_hashes
    );
    assert_eq!(receipt.results, vec![expected_result.clone()]);
    assert_eq!(
        receipt.result_hashes,
        vec![Hash::from(expected_result.hash())]
    );
    assert!(kura.lane_block_application_receipt_available(&proposal));

    let (data_path, index_path) =
        Kura::lane_block_application_receipt_paths_for_entry(lane_entry, temp_dir.path());
    assert!(
        data_path.is_file(),
        "lane application receipt data file missing"
    );
    assert!(
        index_path.is_file(),
        "lane application receipt index file missing"
    );

    drop(kura);
    let (reloaded, _) = Kura::new(&config, &lane_config).expect("reopen kura");
    assert_eq!(
        reloaded.read_lane_block_application_receipt(lane_id, lane_block_height),
        Some(receipt)
    );
    assert!(reloaded.lane_block_application_receipt_available(&proposal));
}

#[test]
fn terminal_receipt_pair_revalidation_fails_closed_on_missing_corrupt_and_mismatched_bytes() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let mut block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    )
    .as_ref()
    .clone();
    attach_ok_results_to_block(&mut block);
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(Arc::new(block))
        .expect("store block with lane artifact and results");
    kura.persist_lane_block_application_receipt(&proposal)
        .expect("persist exact application receipt");
    let expected = kura
        .read_lane_block_application_receipt(lane_id, lane_block_height)
        .expect("read exact application receipt");
    let (data_path, index_path) =
        Kura::lane_block_application_receipt_paths_for_entry(lane_entry, temp_dir.path());
    let original_data = fs::read(&data_path).expect("read receipt data bytes");
    let original_index = fs::read(&index_path).expect("read receipt index bytes");
    assert!(original_data.len() > 1);
    assert!(original_index.len() > 1);

    let require_exact = || {
        kura.require_exact_autonomous_lifecycle_terminal_application_receipt_for_tests(
            &expected,
            &data_path,
            &index_path,
        )
    };
    require_exact().expect("an unchanged receipt pair must revalidate exactly");

    fs::write(&data_path, &original_data[..original_data.len() - 1])
        .expect("truncate receipt data");
    assert!(
        require_exact().is_err(),
        "a truncated receipt data file must fail closed",
    );
    fs::write(&data_path, &original_data).expect("restore receipt data");
    require_exact().expect("restored receipt data must revalidate");

    let mut corrupt_data = original_data.clone();
    let corrupt_data_index = corrupt_data.len() / 2;
    corrupt_data[corrupt_data_index] ^= 0x80;
    fs::write(&data_path, corrupt_data).expect("corrupt receipt data");
    assert!(
        require_exact().is_err(),
        "corrupt receipt data must fail closed",
    );
    fs::write(&data_path, &original_data).expect("restore receipt data after corruption");

    fs::write(&index_path, &original_index[..original_index.len() - 1])
        .expect("truncate receipt index");
    assert!(
        require_exact().is_err(),
        "a truncated receipt index must fail closed",
    );
    fs::write(&index_path, &original_index).expect("restore receipt index");

    let mut mismatched_index = original_index.clone();
    let mismatched_index_offset = mismatched_index.len() / 2;
    mismatched_index[mismatched_index_offset] ^= 0x40;
    fs::write(&index_path, mismatched_index).expect("mismatch receipt index");
    assert!(
        require_exact().is_err(),
        "an index which no longer binds the exact data entry must fail closed",
    );
    fs::write(&index_path, &original_index).expect("restore receipt index after mismatch");
    require_exact().expect("the fully restored receipt pair must revalidate");

    fs::remove_file(&index_path).expect("remove receipt index");
    assert!(
        require_exact().is_err(),
        "a missing receipt index must fail closed",
    );
}

#[test]
fn receipt_repair_preflight_does_not_request_an_already_present_unowned_body() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let mut block = dummy_block_with_lane_payload_ownership(lane_id, lane_entry.dataspace_id, 1)
        .as_ref()
        .clone();
    attach_ok_results_to_block(&mut block);
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let mut unowned = lane_block_proposal_from_ownership(&ownership);
    unowned.descriptor.lane_block_height = 2;
    unowned.descriptor.previous_lane_block_height = 1;
    unowned.descriptor.previous_lane_block_descriptor_hash =
        Some(Hash::new(b"unowned-receipt-predecessor"));
    unowned.descriptor.descriptor_hash = unowned.descriptor.computed_descriptor_hash();
    unowned.proposal_hash = unowned.computed_proposal_hash();

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(Arc::new(block))
        .expect("store canonical body with a different lane owner");
    assert!(
        kura.preflight_lane_block_application_receipt_repair(&unowned)
            .is_err(),
        "a present canonical body that does not own the certified proposal is a conflict, not a recoverable missing-body need",
    );
}

fn lane_block_application_receipt_strict_retry_reissues_every_barrier() {
    for (label, failure) in strict_progress_sidecar_failure_modes() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        assert_eq!(
            config.fsync_mode,
            FsyncMode::Batched,
            "fixture must prove the receipt overrides ordinary batched durability"
        );
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let lane_block_height = 1;
        let mut block = dummy_block_with_lane_payload_ownership(
            lane_id,
            lane_entry.dataspace_id,
            lane_block_height,
        )
        .as_ref()
        .clone();
        attach_ok_results_to_block(&mut block);
        let ownership = block
            .execution_context()
            .expect("execution context")
            .lane_payload_ownerships
            .first()
            .expect("lane ownership")
            .clone();
        let proposal = lane_block_proposal_from_ownership(&ownership);

        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.store_block(Arc::new(block))
            .expect("store canonical receipt evidence");
        let expected = kura
            .recover_lane_block_application_receipt_artifact(&proposal)
            .expect("recover expected receipt before fault injection");
        let (data_path, index_path) =
            Kura::lane_block_application_receipt_paths_for_entry(lane_entry, temp_dir.path());

        failure.inject();
        assert!(
            kura.persist_lane_block_application_receipt_if_ready(&proposal)
                .is_err(),
            "injected {label} barrier failure must reject receipt persistence"
        );
        let readable =
            Kura::read_indexed_sidecar_from_paths::<LaneBlockApplicationReceiptArtifact, _>(
                lane_block_height,
                &data_path,
                &index_path,
                norito::decode_from_bytes::<LaneBlockApplicationReceiptArtifact>,
                "lane block application receipt",
            )
            .expect("failed barrier leaves exact page-cache receipt bytes readable");
        assert_eq!(readable, expected);
        let first_data_len = fs::metadata(&data_path)
            .expect("receipt data metadata")
            .len();

        drop(kura);
        let (kura, _) = Kura::new(&config, &lane_config).expect("reopen Kura after fault");
        failure.inject();
        assert_eq!(
            kura.read_lane_block_application_receipt(lane_id, lane_block_height),
            None,
            "a reopened public reader must not expose a receipt while its {label} barrier fails"
        );

        failure.inject();
        assert!(
            !kura.lane_block_application_receipt_available(&proposal),
            "receipt availability must fail closed while its {label} barrier fails"
        );

        failure.inject();
        assert!(
            kura.persist_lane_block_application_receipt_if_ready(&proposal)
                .is_err(),
            "exact-existing receipt retry must reissue the {label} barrier"
        );
        assert_eq!(
            fs::metadata(&data_path)
                .expect("receipt data metadata")
                .len(),
            first_data_len,
            "failed exact receipt retry must not append duplicate bytes"
        );

        assert!(
            kura.persist_lane_block_application_receipt_if_ready(&proposal)
                .expect("receipt retry after barrier recovery"),
            "complete canonical evidence must persist a receipt"
        );
        assert_eq!(
            fs::metadata(&data_path)
                .expect("receipt data metadata")
                .len(),
            first_data_len,
            "successful exact receipt retry must not append duplicate bytes"
        );
        assert_eq!(
            kura.read_lane_block_application_receipt(lane_id, lane_block_height),
            Some(expected),
            "receipt must become observable after every strict barrier succeeds"
        );
    }
}

#[test]
fn current_application_receipt_fails_closed_after_lane_recreation() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let mut block = dummy_block_with_lane_payload_ownership(lane_id, lane_entry.dataspace_id, 1)
        .as_ref()
        .clone();
    attach_ok_results_to_block(&mut block);
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(Arc::new(block))
        .expect("store canonical receipt anchor");
    let recovered = kura
        .recover_lane_block_payload(&proposal)
        .expect("recover globally anchored execution input");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist globally anchored execution input");
    let input = kura
        .read_lane_block_execution_input(lane_id, 1)
        .expect("read globally anchored execution input");
    let preflight_state_hash = Some(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"global marker-bound preflight state",
    )));
    let preflight_result =
        TransactionResult::new(TransactionResultInner::Ok(DataTriggerSequence::new()));
    kura.persist_lane_block_execution_preflight(
        &input,
        7,
        preflight_state_hash,
        vec![preflight_result],
    )
    .expect("persist globally anchored execution preflight");
    kura.persist_lane_block_application_receipt(&proposal)
        .expect("persist current receipt under first marker");
    assert_eq!(
        kura.read_lane_block_application_receipt(lane_id, 1)
            .expect("read first current receipt")
            .format,
        LaneBlockApplicationReceiptArtifactFormat::Current,
    );

    kura.install_lane_incarnation_marker_for_test(
        lane_entry,
        Hash::new(b"recreated-current-receipt-incarnation"),
        0,
    )
    .expect("install recreated lane marker");
    assert!(
        kura.read_lane_block_application_receipt(lane_id, 1)
            .is_none(),
        "a current receipt from the retired incarnation must not be served",
    );
    assert!(
        kura.read_lane_block_execution_input(lane_id, 1).is_none(),
        "a globally anchored input from the retired incarnation must not be served",
    );
    assert!(
        kura.read_lane_block_execution_preflight(lane_id, 1)
            .is_none(),
        "a global-input preflight from the retired incarnation must not be served",
    );
    assert!(
        kura.persist_lane_block_execution_input(&recovered).is_err(),
        "canonical global evidence must not authorize a retired-incarnation input replay",
    );
    assert!(
        kura.persist_lane_block_execution_preflight(
            &input,
            7,
            preflight_state_hash,
            vec![TransactionResult::new(TransactionResultInner::Ok(
                DataTriggerSequence::new(),
            ))],
        )
        .is_err(),
        "canonical global evidence must not authorize a retired-incarnation preflight replay",
    );
    assert!(
        kura.persist_lane_block_application_receipt(&proposal)
            .is_err(),
        "canonical global evidence must not authorize a retired-incarnation receipt replay",
    );
    drop(kura);

    let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen Kura");
    assert!(
        reopened
            .read_lane_block_application_receipt(lane_id, 1)
            .is_none(),
        "restart must preserve the recreated marker boundary for current receipts",
    );
}

#[test]
fn merge_application_receipt_is_first_release_retirement_admissible_and_fails_closed_after_lane_recreation()
 {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let lane_entry = lane_config.primary();
    let (mut kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
    let entrypoint = offline_top_up_entrypoint_for_index([0xC1; 32], [0xC2; 32]);
    let mut merge_entry = merge_entry_with_indexed_entrypoint(entrypoint);
    let execution = merge_entry
        .execution_batch
        .as_ref()
        .and_then(|batch| batch.lanes.first())
        .expect("merge execution fixture");
    let descriptor = execution.proposal.descriptor.clone();
    kura.install_lane_incarnation_marker_for_test(lane_entry, descriptor.lane_incarnation, 0)
        .expect("install merge receipt lane marker");

    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let raw_carrier = blocks.next();
    let batch = merge_entry
        .execution_batch
        .as_mut()
        .expect("merge receipt fixture has an execution batch");
    batch.application_block_header =
        crate::merge::merge_application_header_from_carrier(&raw_carrier.header());
    batch.batch_hash = crate::merge::merge_execution_batch_hash(batch);
    let mut executed_carrier = raw_carrier.as_ref().clone();
    attach_ok_results_to_block(&mut executed_carrier);
    let carrier = bind_merge_entry_to_carrier(Arc::new(executed_carrier), &mut merge_entry);
    assert!(
        carrier.has_results(),
        "a canonical merge receipt carrier must contain execution results"
    );
    assert_eq!(
        carrier.results().count(),
        carrier.external_entrypoints_cloned().count(),
        "the merge receipt carrier must contain one result per ordinary entrypoint"
    );
    assert_eq!(
        merge_entry
            .execution_batch
            .as_ref()
            .expect("merge receipt fixture has an execution batch")
            .application_block_header,
        crate::merge::merge_application_header_from_carrier(&carrier.header()),
        "the merge receipt batch must bind the canonical stripped carrier header"
    );
    let carrier_height = carrier.header().height().get();
    let carrier_hash = carrier.hash();
    kura.store_block(parent)
        .expect("store merge carrier parent");
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &merge_entry)
        .expect("store committed merge carrier");
    let _ = persist_v2_finality_chain_through(
        &kura,
        NonZeroUsize::new(usize::try_from(carrier_height).expect("carrier height fits usize"))
            .expect("carrier height is non-zero"),
    );
    kura.persist_merge_lane_block_application_receipts(&merge_entry, carrier_height, carrier_hash)
        .expect("persist marker-bound merge receipt");
    assert_eq!(
        kura.read_lane_block_application_receipt(descriptor.lane_id, descriptor.lane_block_height,)
            .expect("read merge receipt")
            .format,
        LaneBlockApplicationReceiptArtifactFormat::MergeExecution,
    );
    let frontier_path =
        Kura::lane_merge_application_frontier_path_for_entry(lane_entry, &kura.store_root());
    let frontier = kura
        .decode_lane_merge_application_frontier(lane_entry, &frontier_path)
        .expect("decode merge application frontier")
        .expect("merge receipt must publish its terminal frontier");
    assert_eq!(frontier.lane_block_height, descriptor.lane_block_height);
    assert!(
        kura.lane_merge_application_frontier_expected_receipt_under_prune_and_canonical_guards(
            &frontier,
        )
        .is_some(),
        "the compact cursor must revalidate against the exact merge entry and carrier"
    );

    let history_before_capacity_refusal = snapshot_regular_files_recursively(temp_dir.path());
    Arc::get_mut(&mut kura)
        .expect("exclusive Kura before compaction capacity refusal")
        .max_disk_usage_bytes = 1;
    let compaction_outcome = {
        let _prune_guard = kura.prune_lock.lock();
        kura.ensure_prune_recovery_not_required()
            .expect("prune recovery is complete");
        let _canonical_chain_guard = kura.canonical_chain_lock.lock();
        let pending_canonical_bytes = kura
            .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
            .expect("snapshot pending canonical capacity");
        let _geometry_guard = kura.lane_geometry_lock.lock();
        let _sidecar_guard = kura.sidecar_lock.lock();
        kura.compact_lane_histories_through_merge_frontier_locked(
            pending_canonical_bytes,
            lane_entry,
            &frontier,
        )
        .expect("capacity refusal is a bounded compaction outcome")
    };
    assert_eq!(
        compaction_outcome,
        LaneHistoryCompactionOutcome::CapacityBlocked,
    );
    assert_eq!(
        snapshot_regular_files_recursively(temp_dir.path()),
        history_before_capacity_refusal,
        "capacity-blocked compaction must not mutate durable lane history",
    );
    kura.repair_lane_merge_application_frontiers_on_startup()
        .expect("capacity-blocked startup compaction must finish one bounded pass");
    assert_eq!(
        snapshot_regular_files_recursively(temp_dir.path()),
        history_before_capacity_refusal,
        "bounded startup repair must retain uncompacted evidence without mutation",
    );
    Arc::get_mut(&mut kura)
        .expect("exclusive Kura after compaction capacity refusal")
        .max_disk_usage_bytes = 0;

    kura.first_release_lane_retirement_admissible_for_test(
        descriptor.lane_id,
        descriptor.dataspace_id,
        descriptor.lane_incarnation,
    )
    .expect("a canonical merge receipt must release its historical execution from retirement");

    kura.install_lane_incarnation_marker_for_test(
        lane_entry,
        Hash::new(b"recreated-merge-receipt-incarnation"),
        descriptor.proposal_height,
    )
    .expect("install recreated merge lane marker");
    assert!(
        kura.read_lane_block_application_receipt(descriptor.lane_id, descriptor.lane_block_height,)
            .is_none(),
        "a merge receipt from the retired incarnation must not be served",
    );
    assert!(
        kura.persist_merge_lane_block_application_receipts(
            &merge_entry,
            carrier_height,
            carrier_hash,
        )
        .is_err(),
        "committed merge evidence must not authorize a retired-incarnation receipt replay",
    );
    kura.persist_merge_lane_block_application_receipts_from_committed_log(&merge_entry)
        .expect(
            "startup repair skips historical executions instead of repopulating active storage",
        );
    assert!(
        kura.read_lane_block_application_receipt(descriptor.lane_id, descriptor.lane_block_height,)
            .is_none(),
        "historical repair must not repopulate a retired receipt into recreated storage",
    );
    drop(kura);

    assert!(
        Kura::new(&config, &lane_config).is_err(),
        "restart must fail closed when a recreated active directory still contains the old incarnation's terminal frontier",
    );
}

#[test]
fn lane_block_sidecars_remain_valid_for_hash_only_snapshot_anchor() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let mut block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    )
    .as_ref()
    .clone();
    attach_ok_results_to_block(&mut block);
    let block_height = block.header().height().get();
    let block_height_usize =
        NonZeroUsize::new(usize::try_from(block_height).expect("dummy block height fits usize"))
            .expect("dummy block height is non-zero");
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(Arc::new(block))
        .expect("store block with lane artifact and results");
    let recovered = kura
        .recover_lane_block_payload(&proposal)
        .expect("recover executable lane payload before snapshot pruning");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist lane execution input");
    kura.persist_lane_block_application_receipt(&proposal)
        .expect("persist lane application receipt");
    let input = kura
        .read_lane_block_execution_input(lane_id, lane_block_height)
        .expect("lane execution input before snapshot pruning");
    let receipt = kura
        .read_lane_block_application_receipt(lane_id, lane_block_height)
        .expect("lane application receipt before snapshot pruning");

    kura.force_hash_only_block_for_testing(block_height_usize)
        .expect("force block into hash-only snapshot form");
    assert!(kura.is_hash_only_block_height(block_height_usize));
    assert!(kura.get_block(block_height_usize).is_none());
    assert_eq!(
        kura.recover_lane_block_payload(&proposal)
            .expect_err("hash-only anchor cannot recover the canonical body"),
        LaneBlockPayloadAvailability::MissingProposalBlock
    );
    assert_eq!(
        kura.read_lane_block_execution_input(lane_id, lane_block_height),
        Some(input.clone())
    );
    assert!(kura.lane_block_execution_input_available(&proposal));
    assert_eq!(
        kura.read_lane_block_application_receipt(lane_id, lane_block_height),
        Some(receipt)
    );
    assert!(kura.lane_block_application_receipt_available(&proposal));
}

#[test]
fn lane_block_application_receipt_waits_for_committed_results() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(block)
        .expect("store block with lane artifact but no results");
    assert!(
        kura.persist_lane_block_application_receipt(&proposal)
            .is_err(),
        "receipt persistence must fail while canonical results are absent"
    );
    assert!(
        !kura
            .persist_lane_block_application_receipt_if_ready(&proposal)
            .expect("not-ready receipt recovery is non-fatal"),
        "if-ready receipt persistence should report not ready"
    );
    assert_eq!(
        kura.read_lane_block_application_receipt(lane_id, lane_block_height),
        None
    );
    assert!(!kura.lane_block_application_receipt_available(&proposal));
}

#[test]
fn lane_block_direct_application_receipt_persists_clean_preflight_results() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(block)
        .expect("store block with lane artifact but no canonical results");
    assert!(
        !kura
            .persist_lane_block_application_receipt_if_ready(&proposal)
            .expect("canonical receipt is not ready while block results are absent")
    );
    let recovered = kura
        .recover_lane_block_payload(&proposal)
        .expect("recover executable lane payload");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist lane execution input");
    let input = kura
        .read_lane_block_execution_input(lane_id, lane_block_height)
        .expect("lane execution input");
    let state_hash = Some(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"direct application base state hash",
    )));
    let result = TransactionResult::new(TransactionResultInner::Ok(DataTriggerSequence::new()));
    kura.persist_lane_block_execution_preflight(&input, 7, state_hash, vec![result.clone()])
        .expect("persist clean lane execution preflight");
    let preflight = kura
        .read_lane_block_execution_preflight(lane_id, lane_block_height)
        .expect("lane execution preflight");

    kura.persist_direct_lane_block_application_receipt(&input, &preflight)
        .expect("persist direct lane application receipt");
    kura.persist_direct_lane_block_application_receipt(&input, &preflight)
        .expect("direct lane application receipt persistence is idempotent");

    let receipt = kura
        .read_lane_block_application_receipt(lane_id, lane_block_height)
        .expect("direct lane application receipt");
    assert_eq!(
        receipt.format,
        LaneBlockApplicationReceiptArtifactFormat::DirectExecution
    );
    assert_eq!(receipt.application_block_height, 7);
    assert_eq!(
        receipt.application_block_hash,
        preflight
            .preflight_state_hash
            .expect("direct receipt state hash")
    );
    assert_eq!(receipt.results, vec![result]);
    assert!(kura.lane_block_application_receipt_available(&proposal));

    drop(kura);
    let (reloaded, _) = Kura::new(&config, &lane_config).expect("reload kura");
    assert_eq!(
        reloaded.read_lane_block_application_receipt(lane_id, lane_block_height),
        Some(receipt)
    );
    assert!(reloaded.lane_block_application_receipt_available(&proposal));
}

#[test]
fn lane_block_direct_application_receipt_rejects_rejected_preflight() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(block)
        .expect("store block with lane artifact");
    let recovered = kura
        .recover_lane_block_payload(&proposal)
        .expect("recover executable lane payload");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist lane execution input");
    let input = kura
        .read_lane_block_execution_input(lane_id, lane_block_height)
        .expect("lane execution input");
    let state_hash = Some(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"rejected direct application base state hash",
    )));
    let rejected = TransactionResult::new(TransactionResultInner::Err(
        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::NotPermitted(
                "direct receipt rejected preflight".to_owned(),
            ),
        ),
    ));
    kura.persist_lane_block_execution_preflight(&input, 7, state_hash, vec![rejected])
        .expect("persist rejected lane execution preflight");
    let preflight = kura
        .read_lane_block_execution_preflight(lane_id, lane_block_height)
        .expect("lane execution preflight");

    assert!(
        kura.persist_direct_lane_block_application_receipt(&input, &preflight)
            .is_err(),
        "direct receipts must reject failed preflight evidence"
    );
    assert_eq!(
        kura.read_lane_block_application_receipt(lane_id, lane_block_height),
        None
    );
    assert!(!kura.lane_block_application_receipt_available(&proposal));
}

#[test]
fn lane_block_application_receipt_read_rejects_tampered_sidecar() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let mut block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    )
    .as_ref()
    .clone();
    attach_ok_results_to_block(&mut block);
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(Arc::new(block))
        .expect("store block with lane artifact and results");
    kura.persist_lane_block_application_receipt(&proposal)
        .expect("persist lane application receipt");
    let mut tampered = kura
        .read_lane_block_application_receipt(lane_id, lane_block_height)
        .expect("lane application receipt");
    tampered.result_hashes[0] = Hash::new(b"tampered persisted lane application receipt");
    let payload = tampered
        .encode_framed()
        .expect("encode tampered lane application receipt");
    let (data_path, index_path) =
        Kura::lane_block_application_receipt_paths_for_entry(lane_entry, temp_dir.path());
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            lane_block_height,
            &payload,
            "lane block application receipt",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ),
        "tampered sidecar should overwrite the indexed application receipt entry"
    );

    assert_eq!(
        kura.read_lane_block_application_receipt(lane_id, lane_block_height),
        None,
        "tampered application receipt sidecars must be rejected on read"
    );
    assert!(!kura.lane_block_application_receipt_available(&proposal));
}

#[test]
fn lane_block_application_receipt_reader_rejects_legacy_omitted_merge_evidence() {
    #[derive(Encode)]
    struct LegacyLaneBlockApplicationReceiptArtifact {
        format: LaneBlockApplicationReceiptArtifactFormat,
        proposal: LaneBlockProposalV1,
        artifact: LaneBlockArtifact,
        application_block_height: u64,
        application_block_hash: HashOf<BlockHeader>,
        entrypoint_indices: Vec<u64>,
        entrypoint_hashes: Vec<Hash>,
        result_hashes: Vec<Hash>,
        results: Vec<TransactionResult>,
    }

    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let mut block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    )
    .as_ref()
    .clone();
    attach_ok_results_to_block(&mut block);
    let proposal = lane_block_proposal_from_ownership(
        block
            .execution_context()
            .expect("execution context")
            .lane_payload_ownerships
            .first()
            .expect("lane ownership"),
    );

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(Arc::new(block))
        .expect("store block with lane artifact and results");
    kura.persist_lane_block_application_receipt(&proposal)
        .expect("persist current lane application receipt");
    let current = kura
        .read_lane_block_application_receipt(lane_id, lane_block_height)
        .expect("current lane application receipt");
    let legacy = LegacyLaneBlockApplicationReceiptArtifact {
        format: current.format,
        proposal: current.proposal,
        artifact: current.artifact,
        application_block_height: current.application_block_height,
        application_block_hash: current.application_block_hash,
        entrypoint_indices: current.entrypoint_indices,
        entrypoint_hashes: current.entrypoint_hashes,
        result_hashes: current.result_hashes,
        results: current.results,
    };
    let payload = norito::to_bytes(&legacy).expect("encode legacy application receipt");
    let (data_path, index_path) =
        Kura::lane_block_application_receipt_paths_for_entry(lane_entry, temp_dir.path());
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            lane_block_height,
            &payload,
            "lane block application receipt",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ),
        "legacy application receipt should replace the indexed test entry"
    );

    assert_eq!(
        kura.read_lane_block_application_receipt_from_paths_locked(
            lane_id,
            lane_block_height,
            &data_path,
            &index_path,
            false,
        ),
        None,
        "a receipt omitting all eleven merge-evidence fields must fail closed"
    );
}

#[test]
fn autonomous_execution_input_requires_complete_exact_source_binding() {
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (chain_id_hash, epoch, payload) =
        autonomous_lane_payload_for_kura(LaneId::new(1), DataSpaceId::new(2), 1, &signer);
    let input =
        Kura::autonomous_lane_block_execution_input_candidate(&payload, chain_id_hash, epoch)
            .expect("autonomous input fixture");
    assert_eq!(
        Kura::validate_lane_block_execution_input_artifact(&input),
        Ok(()),
        "a complete reservation-bound autonomous input must be accepted"
    );

    for (label, candidate) in [
        ("chain", {
            let mut candidate = input.clone();
            candidate.autonomous_chain_id_hash = None;
            candidate
        }),
        ("epoch", {
            let mut candidate = input.clone();
            candidate.autonomous_epoch = None;
            candidate
        }),
        ("payload", {
            let mut candidate = input.clone();
            candidate.autonomous_payload_hash = None;
            candidate
        }),
    ] {
        assert_eq!(
            Kura::validate_lane_block_execution_input_artifact(&candidate),
            Err("execution input autonomous source binding is incomplete"),
            "missing {label} binding must fail closed"
        );
    }

    let mut unbound = input.clone();
    unbound.reservation_keys.clear();
    unbound.routing_plans.clear();
    unbound.native_amx_receipts.clear();
    assert_eq!(
        Kura::validate_lane_block_execution_input_artifact(&unbound),
        Err("autonomous execution input reservation and routing vectors are not aligned"),
        "the former payload-hint compatibility handoff must fail closed"
    );

    let mut unsupported_reservation_version = input;
    unsupported_reservation_version.reservation_keys[0].version =
        LaneQueueReservationKeyV2::VERSION + 1;
    assert_eq!(
        Kura::validate_lane_block_execution_input_artifact(&unsupported_reservation_version),
        Err("autonomous execution input reservation key is invalid")
    );
}

#[test]
fn global_execution_input_rejects_unbound_autonomous_metadata() {
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (chain_id_hash, epoch, payload) =
        autonomous_lane_payload_for_kura(LaneId::new(1), DataSpaceId::new(2), 1, &signer);
    let mut input =
        Kura::autonomous_lane_block_execution_input_candidate(&payload, chain_id_hash, epoch)
            .expect("autonomous input fixture");
    input.autonomous_chain_id_hash = None;
    input.autonomous_epoch = None;
    input.autonomous_payload_hash = None;

    for (label, candidate) in [
        ("reservation", {
            let mut candidate = input.clone();
            candidate.routing_plans.clear();
            candidate.native_amx_receipts.clear();
            candidate
        }),
        ("routing", {
            let mut candidate = input.clone();
            candidate.reservation_keys.clear();
            candidate.native_amx_receipts.clear();
            candidate
        }),
        ("native-amx", {
            let mut candidate = input.clone();
            candidate.reservation_keys.clear();
            candidate.routing_plans.clear();
            candidate
        }),
    ] {
        assert_eq!(
            Kura::validate_lane_block_execution_input_artifact(&candidate),
            Err("global execution input carries autonomous reservation metadata"),
            "{label} metadata must fail closed without an autonomous source binding"
        );
    }

    input.reservation_keys.clear();
    input.routing_plans.clear();
    input.native_amx_receipts.clear();
    assert_eq!(
        Kura::validate_lane_block_execution_input_artifact(&input),
        Ok(())
    );
}

#[test]
fn lane_block_application_receipt_replaces_stale_rollback_evidence() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let mut original = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    )
    .as_ref()
    .clone();
    attach_ok_results_to_block(&mut original);
    let original_proposal = lane_block_proposal_from_ownership(
        original
            .execution_context()
            .expect("original execution context")
            .lane_payload_ownerships
            .first()
            .expect("original ownership"),
    );

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(Arc::new(original))
        .expect("store original application block");
    kura.persist_lane_block_application_receipt(&original_proposal)
        .expect("persist original receipt");
    let original_receipt = kura
        .read_lane_block_application_receipt(lane_id, lane_block_height)
        .expect("original receipt");

    kura.prune_to_height(0).expect("roll back original carrier");
    assert_eq!(
        kura.read_lane_block_application_receipt(lane_id, lane_block_height),
        None,
        "receipt must become stale when its global evidence is rolled back"
    );

    std::thread::sleep(Duration::from_millis(2));
    let mut replacement = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    )
    .as_ref()
    .clone();
    attach_ok_results_to_block(&mut replacement);
    let replacement_hash = replacement.hash();
    assert_ne!(replacement_hash, original_receipt.application_block_hash);
    let replacement_proposal = lane_block_proposal_from_ownership(
        replacement
            .execution_context()
            .expect("replacement execution context")
            .lane_payload_ownerships
            .first()
            .expect("replacement ownership"),
    );
    kura.store_block(Arc::new(replacement))
        .expect("store replacement application block");
    kura.persist_lane_block_application_receipt(&replacement_proposal)
        .expect("stale receipt must not block replacement evidence");

    let replacement_receipt = kura
        .read_lane_block_application_receipt(lane_id, lane_block_height)
        .expect("replacement receipt");
    assert_eq!(replacement_receipt.application_block_hash, replacement_hash);
    assert_ne!(replacement_receipt, original_receipt);
}

#[test]
fn lane_block_execution_input_rejects_forged_entrypoint_hashes() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(block)
        .expect("store block with lane artifact");
    let recovered = kura
        .recover_lane_block_payload(&proposal)
        .expect("recover executable lane payload");
    let mut forged = LaneBlockExecutionInputArtifact::new(recovered);
    forged.entrypoint_hashes[0] = Hash::new(b"forged lane execution input hash");

    assert!(
        kura.write_lane_block_execution_input_artifact(&forged, None, 0)
            .is_err(),
        "forged execution input hashes must not be persisted"
    );
    assert_eq!(
        kura.read_lane_block_execution_input(lane_id, lane_block_height),
        None
    );
}

#[test]
fn lane_block_execution_input_read_rejects_tampered_sidecar() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(block)
        .expect("store block with lane artifact");
    let recovered = kura
        .recover_lane_block_payload(&proposal)
        .expect("recover executable lane payload");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist lane execution input");
    let mut tampered = kura
        .read_lane_block_execution_input(lane_id, lane_block_height)
        .expect("lane execution input");
    tampered.entrypoint_hashes[0] = Hash::new(b"tampered persisted lane execution input");
    let payload = tampered
        .encode_framed()
        .expect("encode tampered lane execution input");
    let (data_path, index_path) =
        Kura::lane_block_execution_input_paths_for_entry(lane_entry, temp_dir.path());
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            lane_block_height,
            &payload,
            "lane block execution input",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ),
        "tampered sidecar should overwrite the indexed execution input entry"
    );

    assert_eq!(
        kura.read_lane_block_execution_input(lane_id, lane_block_height),
        None,
        "tampered execution input sidecars must be rejected on read"
    );
    assert!(!kura.lane_block_execution_input_available(&proposal));

    kura.persist_lane_block_execution_input(&recovered)
        .expect("canonical recovery should overwrite stale execution input");
    let healed = kura
        .read_lane_block_execution_input(lane_id, lane_block_height)
        .expect("healed lane execution input");
    assert_eq!(healed, LaneBlockExecutionInputArtifact::new(recovered));
    assert!(
        kura.lane_block_execution_input_available(&proposal),
        "healed execution input should be available to the standalone executor"
    );
}

#[test]
fn lane_block_execution_input_reader_rejects_legacy_omitted_autonomous_binding() {
    #[derive(Encode)]
    struct LegacyLaneBlockExecutionInputArtifact {
        format: LaneBlockExecutionInputArtifactFormat,
        proposal: LaneBlockProposalV1,
        artifact: LaneBlockArtifact,
        entrypoint_hashes: Vec<Hash>,
        entrypoints: Vec<TransactionEntrypoint>,
    }

    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let proposal = lane_block_proposal_from_ownership(
        block
            .execution_context()
            .expect("execution context")
            .lane_payload_ownerships
            .first()
            .expect("lane ownership"),
    );

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(block)
        .expect("store block with lane artifact");
    let recovered = kura
        .recover_lane_block_payload(&proposal)
        .expect("recover executable lane payload");
    let current = LaneBlockExecutionInputArtifact::new(recovered);
    let legacy = LegacyLaneBlockExecutionInputArtifact {
        format: current.format,
        proposal: current.proposal,
        artifact: current.artifact,
        entrypoint_hashes: current.entrypoint_hashes,
        entrypoints: current.entrypoints,
    };
    let payload = norito::to_bytes(&legacy).expect("encode legacy lane execution input");
    let (data_path, index_path) =
        Kura::lane_block_execution_input_paths_for_entry(lane_entry, temp_dir.path());
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            lane_block_height,
            &payload,
            "lane block execution input",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ),
        "legacy execution input should populate the indexed test entry"
    );

    assert_eq!(
        kura.read_lane_block_execution_input_from_paths_locked(
            lane_id,
            lane_block_height,
            &data_path,
            &index_path,
            false,
        ),
        None,
        "a pre-autonomous input omitting bindings, reservations, routing, and receipts must fail closed"
    );
}

#[test]
fn lane_block_execution_input_read_heals_stale_canonical_artifact() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(block)
        .expect("store block with lane artifact");
    let recovered = kura
        .recover_lane_block_payload(&proposal)
        .expect("recover executable lane payload");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist lane execution input");
    assert!(
        kura.lane_block_execution_input_available(&proposal),
        "fresh execution input should be available before canonical artifact drift"
    );

    let stale_artifact = LaneBlockArtifact::new(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"stale canonical lane artifact block hash",
        )),
        ownership,
    );
    let payload = stale_artifact
        .encode_framed()
        .expect("encode stale lane artifact");
    let (data_path, index_path) = Kura::lane_artifact_paths_for_entry(lane_entry, temp_dir.path());
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            lane_block_height,
            &payload,
            "lane block artifact",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ),
        "stale lane artifact should overwrite the indexed artifact entry"
    );

    assert_eq!(
        kura.read_lane_block_execution_input(lane_id, lane_block_height),
        Some(LaneBlockExecutionInputArtifact::new(recovered.clone())),
        "canonical block recovery must heal a stale lane artifact before validating input"
    );
    assert!(
        kura.lane_block_execution_input_available(&proposal),
        "a stale sidecar must not suppress input recoverable from the canonical block"
    );
    assert_eq!(
        kura.read_lane_block_artifact(lane_id, lane_block_height),
        Some(recovered.artifact),
        "healing must restore the canonical lane artifact"
    );
}

#[test]
fn lane_block_execution_preflight_persists_current_state_results_and_reloads() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(block)
        .expect("store block with lane artifact");
    let recovered = kura
        .recover_lane_block_payload(&proposal)
        .expect("recover executable lane payload");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist lane execution input");
    let input = kura
        .read_lane_block_execution_input(lane_id, lane_block_height)
        .expect("lane execution input");
    let state_hash = Some(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
        b"preflight state hash",
    )));
    let results = vec![TransactionResult::new(TransactionResultInner::Ok(
        DataTriggerSequence::new(),
    ))];

    kura.persist_lane_block_execution_preflight(&input, 7, state_hash.clone(), results.clone())
        .expect("persist lane execution preflight");
    kura.persist_lane_block_execution_preflight(&input, 7, state_hash.clone(), results.clone())
        .expect("idempotent lane execution preflight persist");
    let preflight = kura
        .read_lane_block_execution_preflight(lane_id, lane_block_height)
        .expect("lane execution preflight");
    assert_eq!(preflight.format_label(), "lane.execution_preflight");
    assert_eq!(preflight.proposal, proposal);
    assert_eq!(
        preflight.entrypoint_indices,
        proposal.descriptor.accepted_candidate_indices
    );
    assert_eq!(
        preflight.entrypoint_hashes,
        proposal.descriptor.accepted_transaction_hashes
    );
    assert_eq!(preflight.results, results);
    assert!(!preflight.has_rejections());
    assert_eq!(
        kura.lane_block_execution_preflight_has_rejections(&proposal, 7, state_hash.clone()),
        Some(false)
    );
    let ready_input = kura
        .read_preflighted_lane_block_execution_input_for_application(
            &proposal,
            7,
            state_hash.clone(),
        )
        .expect("clean current-tip preflight should expose application input");
    assert_eq!(ready_input, input);
    assert!(
        kura.read_preflighted_lane_block_execution_input_for_application(
            &proposal,
            8,
            state_hash.clone()
        )
        .is_none(),
        "stale preflight evidence must not expose application input"
    );
    assert_eq!(
        kura.lane_block_execution_preflight_has_rejections(&proposal, 8, state_hash),
        None,
        "preflight evidence must be tied to the current local state tip"
    );

    drop(kura);
    let (reloaded, _) = Kura::new(&config, &lane_config).expect("reload kura");
    assert_eq!(
        reloaded.read_lane_block_execution_preflight(lane_id, lane_block_height),
        Some(preflight)
    );
}

#[test]
fn lane_block_execution_preflight_rejects_result_count_drift() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(block)
        .expect("store block with lane artifact");
    let recovered = kura
        .recover_lane_block_payload(&proposal)
        .expect("recover executable lane payload");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist lane execution input");
    let input = kura
        .read_lane_block_execution_input(lane_id, lane_block_height)
        .expect("lane execution input");

    let err = kura
        .persist_lane_block_execution_preflight(&input, 0, None, Vec::new())
        .expect_err("preflight result count drift must be rejected");
    match err {
        Error::IO(io, _) => {
            assert_eq!(io.kind(), ErrorKind::InvalidData);
            assert!(
                io.to_string()
                    .contains("execution preflight result count does not match entrypoints"),
                "unexpected preflight count-drift error: {io}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(
        kura.read_lane_block_execution_preflight(lane_id, lane_block_height),
        None,
        "malformed preflight sidecar must not be readable after rejected persist"
    );
}

#[test]
fn lane_block_direct_application_input_requires_predecessor_receipt() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 2;
    let block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(block)
        .expect("store block with lane artifact");
    let recovered = kura
        .recover_lane_block_payload(&proposal)
        .expect("recover executable lane payload");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist lane execution input");
    let input = kura
        .read_lane_block_execution_input(lane_id, lane_block_height)
        .expect("lane execution input");
    let result = TransactionResult::new(TransactionResultInner::Ok(DataTriggerSequence::new()));
    kura.persist_lane_block_execution_preflight(&input, 0, None, vec![result])
        .expect("persist clean lane execution preflight");

    assert!(
        !kura.lane_block_predecessor_application_receipt_available(&proposal),
        "height-two lane blocks must wait for their certified predecessor receipt"
    );
    assert!(
        kura.read_preflighted_lane_block_execution_input_for_application(&proposal, 0, None)
            .is_none(),
        "clean preflight alone must not bypass lane predecessor application"
    );
}

#[test]
fn first_lane_block_requires_the_canonical_zero_predecessor() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let block = dummy_block_with_lane_payload_ownership(lane_id, lane_entry.dataspace_id, 1);
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);

    assert!(
        kura.lane_block_predecessor_application_receipt_available(&proposal),
        "lane-local height one must use the canonical zero/None predecessor"
    );

    let mut malformed = proposal;
    malformed.descriptor.previous_lane_block_descriptor_hash =
        Some(Hash::new(b"unexpected height-one predecessor"));
    malformed.descriptor.descriptor_hash = malformed.descriptor.computed_descriptor_hash();
    malformed.proposal_hash = malformed.computed_proposal_hash();
    assert!(
        !kura.lane_block_predecessor_application_receipt_available(&malformed),
        "lane-local height one must reject any predecessor descriptor"
    );
}

#[test]
fn lane_block_predecessor_receipt_rejects_missing_non_genesis_descriptor() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 12;
    let block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let mut proposal = lane_block_proposal_from_ownership(&ownership);
    proposal.descriptor.previous_lane_block_descriptor_hash = None;
    proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
    proposal.proposal_hash = proposal.computed_proposal_hash();

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    assert!(
        !kura.lane_block_predecessor_application_receipt_available(&proposal),
        "a missing non-genesis predecessor descriptor must never bypass lane continuity"
    );
}

#[test]
fn lane_block_direct_application_input_accepts_canonical_predecessor_receipt() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let dataspace_id = lane_entry.dataspace_id;

    let mut generator = DummyBlocks::new();
    let mut predecessor_block = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane_id,
        dataspace_id,
        1,
    )
    .as_ref()
    .clone();
    attach_ok_results_to_block(&mut predecessor_block);
    let predecessor_ownership = predecessor_block
        .execution_context()
        .expect("predecessor execution context")
        .lane_payload_ownerships
        .first()
        .expect("predecessor lane ownership")
        .clone();
    let predecessor_proposal = lane_block_proposal_from_ownership(&predecessor_ownership);

    let mut successor_block = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane_id,
        dataspace_id,
        2,
    )
    .as_ref()
    .clone();
    let successor_ownership = rebind_kura_lane_payload_predecessor(
        &mut successor_block,
        predecessor_proposal.descriptor.descriptor_hash,
    );
    let successor_proposal = lane_block_proposal_from_ownership(&successor_ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(Arc::new(predecessor_block))
        .expect("store predecessor block with results");
    let predecessor_recovered = kura
        .recover_lane_block_payload(&predecessor_proposal)
        .expect("recover predecessor lane payload");
    kura.persist_lane_block_execution_input(&predecessor_recovered)
        .expect("persist predecessor lane execution input");
    let predecessor_input = kura
        .read_lane_block_execution_input(lane_id, 1)
        .expect("predecessor execution input");
    let rejected = TransactionResult::new(TransactionResultInner::Err(
        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::NotPermitted(
                "adversarial predecessor preflight mismatch".to_owned(),
            ),
        ),
    ));
    kura.persist_lane_block_execution_preflight(&predecessor_input, 0, None, vec![rejected])
        .expect("persist conflicting predecessor preflight");
    kura.persist_lane_block_application_receipt(&predecessor_proposal)
        .expect("persist readable predecessor canonical receipt");
    assert!(
        kura.read_lane_block_application_receipt(lane_id, 1)
            .is_some(),
        "conflicting predecessor receipt remains readable as forensic evidence"
    );
    assert!(
        kura.lane_block_application_receipt_available(&predecessor_proposal),
        "canonical block application must remain authoritative over stale local preflight evidence"
    );

    kura.store_block(Arc::new(successor_block))
        .expect("store successor block with predecessor descriptor");
    let successor_recovered = kura
        .recover_lane_block_payload(&successor_proposal)
        .expect("recover successor lane payload");
    kura.persist_lane_block_execution_input(&successor_recovered)
        .expect("persist successor lane execution input");
    let successor_input = kura
        .read_lane_block_execution_input(lane_id, 2)
        .expect("successor execution input");
    let ok = TransactionResult::new(TransactionResultInner::Ok(DataTriggerSequence::new()));
    kura.persist_lane_block_execution_preflight(&successor_input, 0, None, vec![ok])
        .expect("persist clean successor preflight");

    assert!(
        kura.lane_block_predecessor_application_receipt_available(&successor_proposal),
        "canonical predecessor application must unblock the successor"
    );
    assert!(
        kura.read_preflighted_lane_block_execution_input_for_application(
            &successor_proposal,
            0,
            None
        )
        .is_some(),
        "clean successor preflight should be exposed after canonical predecessor application"
    );

    let canonical_proposal_for = |mut ownership: SumeragiLanePayloadOwnership| {
        let replay_hashes = ownership
            .compute_replay_hashes()
            .expect("adversarial successor fixture remains internally canonical");
        ownership.subject_hash = replay_hashes.subject_hash;
        ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
        ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
        lane_block_proposal_from_ownership(&ownership)
    };

    let mut wrong_hash = successor_ownership.clone();
    wrong_hash.previous_lane_block_descriptor_hash =
        Some(Hash::new(b"wrong predecessor descriptor"));
    let wrong_hash_proposal = canonical_proposal_for(wrong_hash);
    assert!(
        !kura.lane_block_predecessor_application_receipt_available(&wrong_hash_proposal),
        "a different declared predecessor descriptor must not authorize a successor"
    );

    let mut non_increasing_global_height = successor_ownership.clone();
    non_increasing_global_height.proposal_height = predecessor_ownership.proposal_height;
    let non_increasing_global_height_proposal =
        canonical_proposal_for(non_increasing_global_height);
    assert!(
        !kura.lane_block_predecessor_application_receipt_available(
            &non_increasing_global_height_proposal
        ),
        "a predecessor must be anchored at an earlier global proposal height"
    );

    let mut wrong_dataspace = successor_ownership.clone();
    wrong_dataspace.dataspace_id = DataSpaceId::new(dataspace_id.as_u64().saturating_add(1));
    let wrong_dataspace_proposal = canonical_proposal_for(wrong_dataspace);
    assert!(
        !kura.lane_block_predecessor_application_receipt_available(&wrong_dataspace_proposal),
        "a predecessor receipt from another dataspace must not authorize a successor"
    );

    let mut wrong_incarnation = successor_ownership;
    wrong_incarnation.lane_incarnation = Hash::new(b"different successor lane incarnation");
    let wrong_incarnation_proposal = canonical_proposal_for(wrong_incarnation);
    assert!(
        !kura.lane_block_predecessor_application_receipt_available(&wrong_incarnation_proposal),
        "a predecessor receipt from another lane incarnation must not authorize a successor"
    );
}

fn predecessor_application_receipt_fails_closed_while_durability_barrier_fails() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let dataspace_id = lane_entry.dataspace_id;

    let mut generator = DummyBlocks::new();
    let mut predecessor_block = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane_id,
        dataspace_id,
        1,
    )
    .as_ref()
    .clone();
    attach_ok_results_to_block(&mut predecessor_block);
    let predecessor_ownership = predecessor_block
        .execution_context()
        .expect("predecessor execution context")
        .lane_payload_ownerships
        .first()
        .expect("predecessor lane ownership")
        .clone();
    let predecessor_proposal = lane_block_proposal_from_ownership(&predecessor_ownership);

    let mut successor_block = dummy_block_with_lane_payload_ownership_from_generator(
        &mut generator,
        lane_id,
        dataspace_id,
        2,
    )
    .as_ref()
    .clone();
    let successor_ownership = rebind_kura_lane_payload_predecessor(
        &mut successor_block,
        predecessor_proposal.descriptor.descriptor_hash,
    );
    let successor_proposal = lane_block_proposal_from_ownership(&successor_ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(Arc::new(predecessor_block))
        .expect("store predecessor block with canonical results");
    kura.persist_lane_block_application_receipt(&predecessor_proposal)
        .expect("persist predecessor application receipt");
    assert!(
        kura.lane_block_predecessor_application_receipt_available(&successor_proposal),
        "durable predecessor receipt must authorize its exact successor"
    );

    for (label, failure) in strict_progress_sidecar_failure_modes() {
        failure.inject();
        assert!(
            !kura.lane_block_predecessor_application_receipt_available(&successor_proposal),
            "successor progress must fail closed while the predecessor receipt's {label} barrier fails"
        );
        assert!(
            kura.lane_block_predecessor_application_receipt_available(&successor_proposal),
            "successor progress must recover after the predecessor receipt's {label} barrier succeeds"
        );
    }
}

#[test]
fn lane_block_execution_preflight_read_rejects_tampered_sidecar() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(block)
        .expect("store block with lane artifact");
    let recovered = kura
        .recover_lane_block_payload(&proposal)
        .expect("recover executable lane payload");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist lane execution input");
    let input = kura
        .read_lane_block_execution_input(lane_id, lane_block_height)
        .expect("lane execution input");
    let result = TransactionResult::new(TransactionResultInner::Ok(DataTriggerSequence::new()));
    kura.persist_lane_block_execution_preflight(&input, 0, None, vec![result])
        .expect("persist lane execution preflight");
    let mut tampered = kura
        .read_lane_block_execution_preflight(lane_id, lane_block_height)
        .expect("lane execution preflight");
    tampered.result_hashes[0] = Hash::new(b"tampered persisted lane preflight result");
    let payload = tampered
        .encode_framed()
        .expect("encode tampered lane execution preflight");
    let (data_path, index_path) =
        Kura::lane_block_execution_preflight_paths_for_entry(lane_entry, temp_dir.path());
    assert!(
        Kura::append_indexed_sidecar(
            &data_path,
            &index_path,
            lane_block_height,
            &payload,
            "lane block execution preflight",
            FsyncMode::Batched,
            None,
            SidecarIndexOrigin::FirstWrite,
        ),
        "tampered sidecar should overwrite the indexed preflight entry"
    );

    assert_eq!(
        kura.read_lane_block_execution_preflight(lane_id, lane_block_height),
        None,
        "tampered execution preflight sidecars must be rejected on read"
    );
    assert_eq!(
        kura.lane_block_execution_preflight_has_rejections(&proposal, 0, None),
        None
    );
}

#[test]
fn canonical_lane_block_application_receipt_overrides_conflicting_preflight() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let mut block = dummy_block_with_lane_payload_ownership(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    )
    .as_ref()
    .clone();
    attach_ok_results_to_block(&mut block);
    let ownership = block
        .execution_context()
        .expect("execution context")
        .lane_payload_ownerships
        .first()
        .expect("lane ownership")
        .clone();
    let proposal = lane_block_proposal_from_ownership(&ownership);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(Arc::new(block))
        .expect("store block with lane artifact and results");
    let recovered = kura
        .recover_lane_block_payload(&proposal)
        .expect("recover executable lane payload");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist lane execution input");
    let input = kura
        .read_lane_block_execution_input(lane_id, lane_block_height)
        .expect("lane execution input");
    let rejected = TransactionResult::new(TransactionResultInner::Err(
        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::NotPermitted(
                "adversarial lane preflight mismatch".to_owned(),
            ),
        ),
    ));
    kura.persist_lane_block_execution_preflight(&input, 0, None, vec![rejected])
        .expect("persist conflicting lane execution preflight");
    assert!(
        kura.read_preflighted_lane_block_execution_input_for_application(&proposal, 0, None)
            .is_none(),
        "rejected direct preflights must not expose application input"
    );
    kura.persist_lane_block_application_receipt(&proposal)
        .expect("persist canonical lane application receipt");

    assert!(
        kura.read_lane_block_application_receipt(lane_id, lane_block_height)
            .is_some(),
        "canonical receipt should still be readable as forensic evidence"
    );
    assert!(
        kura.lane_block_application_receipt_conflicts_with_preflight(&proposal),
        "direct-execution preflight mismatch must be detected"
    );
    assert!(
        kura.lane_block_application_receipt_available(&proposal),
        "canonical block results must keep the lane block applied despite stale preflight evidence"
    );

    drop(kura);
    let (reloaded, _) = Kura::new(&config, &lane_config).expect("reload kura");
    assert!(reloaded.lane_block_application_receipt_conflicts_with_preflight(&proposal));
    assert!(reloaded.lane_block_application_receipt_available(&proposal));
}

#[test]
fn lane_block_payload_availability_rejects_entrypoint_hash_drift() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let mut block = DummyBlocks::new().next().as_ref().clone();
    let mut ownership = sample_lane_payload_ownership_for_kura(
        &block,
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    ownership.accepted_transaction_hashes = vec![Hash::new(b"forged lane payload hash")];
    let replay_hashes = ownership
        .compute_replay_hashes()
        .expect("forged lane ownership replay hashes compute");
    ownership.subject_hash = replay_hashes.subject_hash;
    ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
    ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
    ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
    let proposal = lane_block_proposal_from_ownership(&ownership);
    let execution_context = BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
        HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            ownership.accepted_transaction_hashes[0],
        ),
        lane_id,
        lane_entry.dataspace_id,
    )])
    .with_lane_payload_ownerships(vec![ownership]);
    block.set_execution_context(Some(execution_context));

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(Arc::new(block))
        .expect("store block with forged lane artifact");

    assert_eq!(
        kura.lane_block_payload_availability(&proposal),
        LaneBlockPayloadAvailability::EntrypointHashMismatch
    );
    assert_eq!(
        kura.recover_lane_block_payload(&proposal),
        Err(LaneBlockPayloadAvailability::EntrypointHashMismatch)
    );
}

#[test]
fn lane_block_payload_availability_rejects_missing_entrypoint_index() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let mut block = DummyBlocks::new().next().as_ref().clone();
    let mut ownership = sample_lane_payload_ownership_for_kura(
        &block,
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );
    ownership.accepted_candidate_indices = vec![9];
    ownership.accepted_transaction_hashes = vec![Hash::new(b"missing entrypoint hash")];
    let replay_hashes = ownership
        .compute_replay_hashes()
        .expect("missing-index lane ownership replay hashes compute");
    ownership.subject_hash = replay_hashes.subject_hash;
    ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
    ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
    ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);
    let proposal = lane_block_proposal_from_ownership(&ownership);
    let execution_context = BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
        HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            ownership.accepted_transaction_hashes[0],
        ),
        lane_id,
        lane_entry.dataspace_id,
    )])
    .with_lane_payload_ownerships(vec![ownership]);
    block.set_execution_context(Some(execution_context));

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(Arc::new(block))
        .expect("store block with missing-index lane artifact");

    assert_eq!(
        kura.recover_lane_block_payload(&proposal),
        Err(LaneBlockPayloadAvailability::MissingEntrypoint)
    );
}

#[test]
fn certified_lane_block_persists_under_lane_segment_and_reloads() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let lane_block_height = 1;
    let (session, signer_pops) = sample_committed_lane_block_session_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        lane_block_height,
    );

    let (kura, _) = Kura::new(&config, &lane_config).expect("init Kura");
    assert!(
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .is_err(),
        "a certified session must not define an uninitialized lane incarnation",
    );
    kura.install_lane_incarnation_marker_for_test(
        lane_entry,
        session.proposal.descriptor.lane_incarnation,
        session.proposal.descriptor.proposal_height,
    )
    .expect("install certified-session activation fence");
    assert!(
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .is_err(),
        "a certified session at the incarnation activation height must be rejected",
    );
    kura.install_lane_incarnation_marker_for_test(
        lane_entry,
        session.proposal.descriptor.lane_incarnation,
        0,
    )
    .expect("install explicit certified-session marker");
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified lane block");
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("duplicate certified lane block persistence is idempotent");

    let artifact = kura
        .read_certified_lane_block_artifact(lane_id, lane_block_height)
        .expect("certified lane block");
    assert_eq!(artifact.format_label(), "lane.certified_block");
    assert_eq!(artifact.proposal, session.proposal);
    assert_eq!(artifact.prepare_qc, session.prepare_qc);
    assert_eq!(artifact.commit_qc, session.commit_qc);
    assert_eq!(artifact.signer_pops, signer_pops);

    let (data_path, index_path) =
        Kura::certified_lane_block_paths_for_entry(lane_entry, temp_dir.path());
    assert!(
        data_path.is_file(),
        "certified lane block data file missing"
    );
    assert!(
        index_path.is_file(),
        "certified lane block index file missing"
    );

    drop(kura);
    let (reloaded, _) = Kura::new(&config, &lane_config).expect("reopen kura");
    assert_eq!(
        reloaded.read_certified_lane_block_artifact(lane_id, lane_block_height),
        Some(artifact)
    );
}

#[test]
fn latest_certified_frontier_reloads_and_repairs_a_missing_progress_pair() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (session, signer_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        3,
        30,
    );
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified frontier");
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected.clone())
    );

    let (data_path, index_path) =
        Kura::certified_lane_block_paths_for_entry(lane_entry, temp_dir.path());
    fs::remove_file(&data_path).expect("remove ordinary certified data");
    fs::remove_file(&index_path).expect("remove ordinary certified index");
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected.clone()),
        "the durable frontier must redo its exact ordinary pair"
    );
    assert_eq!(
        kura.read_certified_lane_block_artifact(lane_id, 3),
        Some(expected.clone())
    );

    drop(kura);
    let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen Kura");
    assert_eq!(
        reopened.latest_certified_lane_block_frontier(lane_id),
        Some(expected)
    );
}

#[test]
fn unchanged_latest_certified_frontier_does_not_repeat_pair_fsync() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (session, signer_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified frontier");
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected.clone()),
        "the first read must strictly attest the ordinary pair"
    );

    fail_next_indexed_sidecar_data_sync_for_tests();
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected),
        "an unchanged process-local attestation must avoid a repeated pair fsync"
    );
    let (data_path, _) = Kura::certified_lane_block_paths_for_entry(lane_entry, temp_dir.path());
    let data = fs::File::open(data_path).expect("open certified pair data");
    assert!(
        sync_indexed_sidecar_data(&data).is_err(),
        "the cached frontier read must leave the injected fsync fault unconsumed"
    );
}

#[test]
fn unchanged_latest_certified_frontier_does_not_repeat_bls_validation() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (session, signer_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified frontier");
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected.clone()),
        "first read must perform full artifact validation"
    );

    fail_next_certified_lane_block_artifact_validation_for_tests();
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected.clone()),
        "exact stable frontier identity must reuse its bounded BLS attestation"
    );
    assert_eq!(
        Kura::validate_certified_lane_block_artifact(&expected),
        Err("injected certified lane block artifact validation failure"),
        "the unchanged cached read must leave the injected validation fault unconsumed"
    );
}

#[test]
fn latest_certified_matching_reuses_attested_frontier_before_history_scan() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (session, signer_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified frontier");
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected.clone()),
        "prime the exact frontier validation attestation"
    );

    fail_next_certified_lane_block_artifact_validation_for_tests();
    assert_eq!(
        kura.latest_certified_lane_block_artifact_matching(lane_id, |_| {
            let geometry_guard = kura
                .lane_geometry_lock
                .try_lock()
                .expect("frontier predicate must run without lane_geometry_lock");
            let sidecar_guard = kura
                .sidecar_lock
                .try_lock()
                .expect("frontier predicate must run without sidecar_lock");
            drop(sidecar_guard);
            drop(geometry_guard);
            true
        }),
        Some(expected.clone()),
        "matching must return the attested frontier without validating historical sidecars"
    );
    assert_eq!(
        Kura::validate_certified_lane_block_artifact(&expected),
        Err("injected certified lane block artifact validation failure"),
        "the frontier short-circuit must leave historical validation untouched"
    );
}

#[test]
fn latest_certified_frontier_validation_attestation_is_exact_artifact_bound() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (session, signer_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certified frontier");
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected),
        "first read must validate and attest the exact artifact"
    );

    let (frontier_path, _) =
        Kura::latest_certified_lane_block_frontier_paths_for_entry(lane_entry, temp_dir.path());
    let stored = fs::read(&frontier_path).expect("read attested frontier");
    let mut frontier = norito::decode_from_bytes::<LatestCertifiedLaneBlockFrontierV1>(&stored)
        .expect("decode attested frontier");
    *frontier
        .artifact
        .commit_qc
        .bls_aggregate_signature
        .first_mut()
        .expect("valid commit aggregate signature is nonempty") ^= 1;
    let invalid = LatestCertifiedLaneBlockFrontierV1::new(frontier.artifact)
        .expect("seal structurally canonical invalid-proof frontier");
    fs::write(
        &frontier_path,
        norito::to_bytes(&invalid).expect("encode invalid-proof frontier"),
    )
    .expect("replace frontier with an invalid proof");

    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        None,
        "a different artifact hash must never reuse the prior BLS validation attestation"
    );
}

#[test]
fn latest_certified_frontier_rejects_equal_height_conflict_before_publication() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (first, first_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        1,
        10,
    );
    let (conflict, conflict_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        1,
        11,
    );
    let (older_conflict, older_conflict_pops) =
        sample_committed_lane_block_session_at_proposal_height_for_kura(
            lane_id,
            lane_entry.dataspace_id,
            1,
            9,
        );
    let expected = CertifiedLaneBlockArtifact::new(first.clone(), first_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&first, &first_pops)
        .expect("persist first certificate");
    assert!(
        kura.persist_committed_lane_block_session(&conflict, &conflict_pops)
            .is_err(),
        "a distinct proposal at an occupied lane height must fail before frontier publication"
    );
    assert!(
        kura.persist_committed_lane_block_session(&older_conflict, &older_conflict_pops,)
            .is_err(),
        "equal lane height must conflict even when the distinct proposal has a lower global height"
    );
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected.clone())
    );
    let conflicting_artifact = CertifiedLaneBlockArtifact::new(conflict, conflict_pops);
    let conflicting_payload = conflicting_artifact
        .encode_framed()
        .expect("encode conflicting certificate");
    let (data_path, index_path) =
        Kura::certified_lane_block_paths_for_entry(lane_entry, temp_dir.path());
    assert!(Kura::append_indexed_sidecar(
        &data_path,
        &index_path,
        1,
        &conflicting_payload,
        "certified lane block conflict fixture",
        FsyncMode::Always,
        None,
        SidecarIndexOrigin::FirstWrite,
    ));
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        None,
        "a conflicting active ordinary slot must not be silently repaired without reset authority"
    );
    assert_ne!(
        kura.read_certified_lane_block_artifact(lane_id, 1),
        Some(expected)
    );
}

#[test]
fn latest_certified_frontier_reset_authority_crosses_height_and_repairs_crash() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (old_slot, old_slot_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        1,
        90,
    );
    let (old_tip, old_tip_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        513,
        100,
    );
    let (fresh, fresh_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        1,
        101,
    );
    let authority = crate::state::CertifiedLaneBlockPersistenceAuthority::for_test(
        lane_id,
        lane_entry.dataspace_id,
        fresh.proposal.descriptor.lane_incarnation,
        Some(100),
    );
    let expected = CertifiedLaneBlockArtifact::new(fresh.clone(), fresh_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&old_slot, &old_slot_pops)
        .expect("persist pre-reset occupied slot");
    kura.persist_committed_lane_block_session(&old_tip, &old_tip_pops)
        .expect("persist high pre-reset tip");

    fail_next_bound_progress_append_data_sync_for_tests();
    assert!(
        kura.persist_committed_lane_block_session_with_authority(&fresh, &fresh_pops, &authority,)
            .is_err(),
        "fault must interrupt after the lower post-reset frontier wins but before pair replacement"
    );
    drop(kura);

    let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen after frontier crash");
    assert_eq!(
        reopened.latest_certified_lane_block_frontier_with_authority(lane_id, &authority,),
        Some(expected.clone()),
        "State-authenticated reset authority must repair the reused lower slot after restart"
    );
    assert_eq!(
        reopened.read_certified_lane_block_artifact(lane_id, 1),
        Some(expected)
    );
}

#[test]
fn read_only_certified_frontier_preflight_plans_reused_slot_without_mutation() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (old_slot, old_slot_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        1,
        90,
    );
    let (old_tip, old_tip_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        513,
        100,
    );
    let (fresh, fresh_pops) = sample_committed_lane_block_session_at_proposal_height_for_kura(
        lane_id,
        lane_entry.dataspace_id,
        1,
        101,
    );
    let authority = crate::state::CertifiedLaneBlockPersistenceAuthority::for_test(
        lane_id,
        lane_entry.dataspace_id,
        fresh.proposal.descriptor.lane_incarnation,
        Some(100),
    );
    let expected = CertifiedLaneBlockArtifact::new(fresh.clone(), fresh_pops.clone());
    let old_artifact = CertifiedLaneBlockArtifact::new(old_slot.clone(), old_slot_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&old_slot, &old_slot_pops)
        .expect("persist pre-reset occupied slot");
    kura.persist_committed_lane_block_session(&old_tip, &old_tip_pops)
        .expect("persist high pre-reset tip");

    fail_next_bound_progress_append_data_sync_for_tests();
    assert!(
        kura.persist_committed_lane_block_session_with_authority(&fresh, &fresh_pops, &authority,)
            .is_err(),
        "fixture must leave the fresh frontier over the stale ordinary slot"
    );
    drop(kura);

    let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen after frontier crash");
    let (data_path, index_path) =
        Kura::certified_lane_block_paths_for_entry(lane_entry, temp_dir.path());
    let (frontier_path, build_path) =
        Kura::latest_certified_lane_block_frontier_paths_for_entry(lane_entry, temp_dir.path());
    let before = [
        fs::read(&data_path).expect("read ordinary data before preflight"),
        fs::read(&index_path).expect("read ordinary index before preflight"),
        fs::read(&frontier_path).expect("read frontier before preflight"),
    ];
    assert!(!build_path.exists());
    let revision = reopened.committed_lane_status_revision();

    let planned = reopened
        .preflight_latest_certified_lane_block_frontier_with_authority(lane_id, &authority)
        .expect("read-only frontier preflight")
        .expect("fresh frontier");
    assert_eq!(planned, (expected, true));
    assert_eq!(
        reopened
            .read_certified_lane_block_artifact_read_only(lane_id, 1)
            .expect("read stale ordinary slot without recovery"),
        Some(old_artifact),
    );
    assert_eq!(
        before,
        [
            fs::read(&data_path).expect("read ordinary data after preflight"),
            fs::read(&index_path).expect("read ordinary index after preflight"),
            fs::read(&frontier_path).expect("read frontier after preflight"),
        ],
        "read-only planning must not repair or rewrite Kura bytes",
    );
    assert_eq!(
        reopened.committed_lane_status_revision(),
        revision,
        "read-only planning must not publish a status generation",
    );
    assert!(!build_path.exists());
}

#[test]
fn latest_certified_frontier_absence_never_bootstraps_from_ordinary_history() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (session, signer_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
    let expected = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&session, &signer_pops)
        .expect("persist certificate");
    let (frontier_path, _) =
        Kura::latest_certified_lane_block_frontier_paths_for_entry(lane_entry, temp_dir.path());
    fs::remove_file(&frontier_path).expect("remove mandatory frontier");

    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        None,
        "frontier reads must not fall back to reverse ordinary history"
    );
    assert_eq!(
        kura.read_certified_lane_block_artifact(lane_id, 1),
        Some(expected),
        "fixture must retain valid ordinary history"
    );
    assert!(
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .is_err(),
        "a nonempty ordinary pair without its frontier is unsupported, not a migration source"
    );
    assert!(!frontier_path.exists());
}

#[test]
fn latest_certified_frontier_corruption_and_post_validation_substitution_fail_closed() {
    let make_kura = || {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry").clone();
        let (session, signer_pops) =
            sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .expect("persist certificate");
        (temp_dir, lane_config, lane_entry, lane_id, kura)
    };

    let (corrupt_dir, _corrupt_config, corrupt_entry, corrupt_lane, corrupt_kura) = make_kura();
    let (corrupt_path, _) = Kura::latest_certified_lane_block_frontier_paths_for_entry(
        &corrupt_entry,
        corrupt_dir.path(),
    );
    let mut noncanonical = fs::read(&corrupt_path).expect("read frontier");
    noncanonical.push(0);
    fs::write(&corrupt_path, noncanonical).expect("write noncanonical frontier");
    assert_eq!(
        corrupt_kura.latest_certified_lane_block_frontier(corrupt_lane),
        None
    );

    let (substitute_dir, _substitute_config, substitute_entry, substitute_lane, substitute_kura) =
        make_kura();
    let (substitute_path, _) = Kura::latest_certified_lane_block_frontier_paths_for_entry(
        &substitute_entry,
        substitute_dir.path(),
    );
    let hook_path = substitute_path.clone();
    set_latest_certified_frontier_post_validation_hook_for_tests(move || {
        let mut bytes = fs::read(&hook_path).expect("read authenticated frontier");
        let last = bytes.last_mut().expect("frontier is nonempty");
        *last ^= 1;
        fs::write(&hook_path, bytes).expect("substitute frontier after validation");
    });
    assert_eq!(
        substitute_kura.latest_certified_lane_block_frontier(substitute_lane),
        None,
        "exact post-BLS reread must reject in-place substitution"
    );
    assert!(
        substitute_kura
            .latest_certified_frontier_storage_unknown
            .load(Ordering::Acquire),
        "post-authentication ambiguity must fail-stop the live frontier"
    );
}

#[cfg(unix)]
#[test]
fn latest_certified_frontier_rejects_hardlink_and_symlink_paths() {
    use std::os::unix::fs::symlink;

    for hardlink in [true, false] {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let (session, signer_pops) =
            sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
        let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .expect("persist certificate");
        let (frontier_path, _) =
            Kura::latest_certified_lane_block_frontier_paths_for_entry(lane_entry, temp_dir.path());
        let attacker_path = frontier_path.with_extension("attacker");
        if hardlink {
            fs::hard_link(&frontier_path, &attacker_path).expect("add a second hard link");
        } else {
            fs::rename(&frontier_path, &attacker_path).expect("move frontier to attacker path");
            symlink(&attacker_path, &frontier_path).expect("substitute frontier symlink");
        }
        assert_eq!(
            kura.latest_certified_lane_block_frontier(lane_id),
            None,
            "frontier must reject non-single-link or symlink storage"
        );
    }
}

#[test]
fn certified_lane_block_encoding_enforces_source_envelope() {
    let lane_id = LaneId::from(1);
    let dataspace_id = DataSpaceId::new(7);
    let (session, signer_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, dataspace_id, 1);
    let mut artifact = CertifiedLaneBlockArtifact::new(session, signer_pops);

    assert!(
        artifact.encode_framed().is_ok(),
        "a normal certified lane source must fit its reserved envelope"
    );
    artifact.commit_qc.bls_aggregate_signature =
        vec![0xA5; MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES];

    assert!(
        artifact.encode_framed().is_err(),
        "an oversized certified source must fail before persistence or recovery fanout"
    );
    assert_eq!(
        Kura::validate_certified_lane_block_artifact(&artifact),
        Err("certified lane block exceeds the merge source envelope byte limit")
    );
}

fn certified_lane_block_strict_retry_reissues_every_barrier() {
    for (label, failure) in strict_progress_sidecar_failure_modes() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
        assert_eq!(
            config.fsync_mode,
            FsyncMode::Batched,
            "fixture must prove the certificate overrides ordinary batched durability"
        );
        let lane_config = two_lane_runtime_config();
        let lane_id = LaneId::from(1);
        let lane_entry = lane_config.entry(lane_id).expect("lane entry");
        let lane_block_height = 1;
        let (session, signer_pops) = sample_committed_lane_block_session_for_kura(
            lane_id,
            lane_entry.dataspace_id,
            lane_block_height,
        );
        let expected = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());

        let (kura, _) = Kura::new(&config, &lane_config).expect("init Kura");
        kura.install_lane_incarnation_marker_for_test(
            lane_entry,
            session.proposal.descriptor.lane_incarnation,
            0,
        )
        .expect("install explicit certified-session marker");
        let (data_path, index_path) =
            Kura::certified_lane_block_paths_for_entry(lane_entry, temp_dir.path());

        failure.inject();
        assert!(
            kura.persist_committed_lane_block_session(&session, &signer_pops)
                .is_err(),
            "injected {label} barrier failure must reject certificate persistence"
        );
        let readable = Kura::read_indexed_sidecar_from_paths::<CertifiedLaneBlockArtifact, _>(
            lane_block_height,
            &data_path,
            &index_path,
            norito::decode_from_bytes::<CertifiedLaneBlockArtifact>,
            "certified lane block",
        )
        .expect("failed barrier leaves exact page-cache certificate bytes readable");
        assert_eq!(readable, expected);
        let first_data_len = fs::metadata(&data_path)
            .expect("certified lane data metadata")
            .len();

        drop(kura);
        let (kura, _) = Kura::new(&config, &lane_config).expect("reopen Kura after fault");
        failure.inject();
        assert_eq!(
            kura.read_certified_lane_block_artifact(lane_id, lane_block_height),
            None,
            "a reopened public reader must not expose a certificate while its {label} barrier fails"
        );

        failure.inject();
        assert!(
            kura.persist_committed_lane_block_session(&session, &signer_pops)
                .is_err(),
            "exact-existing certificate retry must reissue the {label} barrier"
        );
        assert_eq!(
            fs::metadata(&data_path)
                .expect("certified lane data metadata")
                .len(),
            first_data_len,
            "failed exact certificate retry must not append duplicate bytes"
        );

        kura.persist_committed_lane_block_session(&session, &signer_pops)
            .expect("certificate retry after barrier recovery");
        assert_eq!(
            fs::metadata(&data_path)
                .expect("certified lane data metadata")
                .len(),
            first_data_len,
            "successful exact certificate retry must not append duplicate bytes"
        );
        assert_eq!(
            kura.read_certified_lane_block_artifact(lane_id, lane_block_height),
            Some(expected),
            "certificate must become observable after every strict barrier succeeds"
        );
    }
}

#[test]
fn certified_lane_block_rejects_foreign_active_dataspace() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (active, active_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 2);
    let (foreign, foreign_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, DataSpaceId::new(77), 3);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&active, &active_pops)
        .expect("persist active certified lane block");
    assert!(
        kura.persist_committed_lane_block_session(&foreign, &foreign_pops)
            .is_err(),
        "a certified session must not define the dataspace of active lane storage"
    );

    let latest = kura
        .latest_certified_lane_block_artifact_for_dataspace(lane_id, lane_entry.dataspace_id)
        .expect("latest certified active lane block");
    assert_eq!(latest.proposal, active.proposal);
    assert_eq!(latest.proposal.descriptor.lane_block_height, 2);
}

#[test]
fn certified_lane_block_artifacts_for_dataspace_replays_ordered_active_backlog() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::from(1);
    let lane_entry = lane_config.entry(lane_id).expect("lane entry");
    let (first, first_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 1);
    let (second, second_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane_entry.dataspace_id, 2);
    let (foreign, foreign_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, DataSpaceId::new(77), 3);

    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&first, &first_pops)
        .expect("persist first active certified lane block");
    kura.persist_committed_lane_block_session(&second, &second_pops)
        .expect("persist second active certified lane block");
    assert!(
        kura.persist_committed_lane_block_session(&foreign, &foreign_pops)
            .is_err(),
        "foreign-dataspace history must be rejected before entering the active segment"
    );

    let active =
        kura.certified_lane_block_artifacts_for_dataspace(lane_id, lane_entry.dataspace_id);
    assert_eq!(
        active
            .iter()
            .map(|artifact| artifact.proposal.descriptor.lane_block_height)
            .collect::<Vec<_>>(),
        vec![1, 2],
        "all active certified lane blocks should replay in lane-local height order"
    );
    assert_eq!(active[0].proposal, first.proposal);
    assert_eq!(active[1].proposal, second.proposal);

    let latest = kura
        .latest_certified_lane_block_artifact_for_dataspace(lane_id, lane_entry.dataspace_id)
        .expect("latest certified active lane block");
    assert_eq!(latest.proposal, second.proposal);

    let first_from_two = kura
        .first_certified_lane_block_artifact_matching_from(lane_id, 2, |artifact| {
            artifact.proposal.descriptor.dataspace_id == lane_entry.dataspace_id
        })
        .expect("first active certified block from lower bound");
    assert_eq!(first_from_two.proposal, second.proposal);
    assert!(
        kura.first_certified_lane_block_artifact_matching_from(lane_id, 3, |artifact| artifact
            .proposal
            .descriptor
            .dataspace_id
            == lane_entry.dataspace_id,)
            .is_none(),
        "a rejected foreign height must not appear in the active backlog"
    );

    let lifecycle_filtered = kura.certified_lane_block_artifacts_matching(lane_id, |artifact| {
        artifact.proposal.descriptor.dataspace_id == lane_entry.dataspace_id
            && artifact.proposal.descriptor.lane_block_height == 2
    });
    assert_eq!(lifecycle_filtered.len(), 1);
    assert_eq!(lifecycle_filtered[0].proposal, second.proposal);

    let reverse_filtered = kura
        .latest_certified_lane_block_artifact_matching(lane_id, |artifact| {
            artifact.proposal.descriptor.dataspace_id == lane_entry.dataspace_id
                && artifact.proposal.descriptor.lane_block_height < 2
        })
        .expect("reverse scan should continue past rejected newer sidecars");
    assert_eq!(reverse_filtered.proposal, first.proposal);

    let bounded_latest =
        kura.latest_certified_lane_block_artifacts_matching(lane_id, 1, |artifact| {
            artifact.proposal.descriptor.dataspace_id == lane_entry.dataspace_id
        });
    assert_eq!(bounded_latest.len(), 1);
    assert_eq!(bounded_latest[0].proposal, second.proposal);
    assert!(
        kura.latest_certified_lane_block_artifacts_matching(lane_id, 0, |_| true)
            .is_empty(),
        "a zero recovery budget must not scan certified history"
    );
}
