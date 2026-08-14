#[test]
fn merge_carrier_budget_reserves_receipt_frontier_without_double_counting_terminal_slot() {
    let temp_dir = TempDir::new().expect("create merge evidence budget temp dir");
    let cfg = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (mut kura, _) =
        Kura::new(&cfg, &RuntimeLaneConfig::default()).expect("initialize merge budget Kura");
    let entrypoint = offline_top_up_entrypoint_for_index([0x61; 32], [0x62; 32]);
    let mut entry = merge_entry_with_indexed_entrypoint(entrypoint);
    let mut blocks = DummyBlocks::new();
    let _parent = blocks.next();
    let carrier = next_merge_carrier(&mut blocks, &mut entry);
    let batch = entry
        .execution_batch
        .as_ref()
        .expect("fixture carries one execution batch");
    let execution = batch.lanes.first().expect("fixture carries one lane");
    let receipt = LaneBlockApplicationReceiptArtifact::new_merge_execution(
        &entry,
        batch,
        execution,
        Kura::merge_lane_block_artifact(execution),
        carrier.header().height().get(),
        carrier.hash(),
    );
    let receipt_len = u64::try_from(
        receipt
            .encode_framed()
            .expect("encode merge receipt fixture")
            .len(),
    )
    .expect("receipt length fits u64");
    let frontier = LaneMergeApplicationFrontierV1::from_receipt(&receipt)
        .expect("merge receipt projects a frontier");
    let frontier_len = u64::try_from(
        norito::encode_canonical(&frontier)
            .expect("encode merge frontier fixture")
            .len(),
    )
    .expect("frontier length fits u64");
    let expected_stable = receipt_len
        .checked_add(Kura::maximum_index_growth_for_unresolved_sidecar_write(
            execution.proposal.descriptor.lane_block_height,
        ))
        .and_then(|bytes| bytes.checked_add(frontier_len))
        .expect("fixture stable accounting does not overflow");
    let expected_peak = expected_stable
        .checked_add(
            u64::try_from(BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES)
                .expect("append intent bound fits u64")
                .max(frontier_len),
        )
        .expect("fixture peak accounting does not overflow");
    assert_eq!(
        kura.merge_lane_application_artifact_required_bytes_for_block(
            carrier.as_ref(),
            Some(&entry),
        )
        .expect("account post-WSV merge artifacts"),
        expected_peak,
        "carrier admission reserves receipt/frontier work while the global lifecycle slot owns terminal bytes"
    );
    let block_required = kura
        .block_required_bytes_for_budget(carrier.as_ref(), Some(&entry), u64::MAX)
        .expect("account complete merge carrier");
    let merge_commit_required = kura
        .merge_commit_required_bytes(carrier.as_ref(), &entry)
        .expect("account merge log and carrier association");
    let association_stage_required = kura
        .canonical_association_stage_additional_bytes(carrier.as_ref(), Some(&entry))
        .expect("account canonical association stage");
    let used = kura
        .refresh_disk_usage_bytes()
        .expect("measure merge Kura baseline");
    let (persisted_count, unindexed_bytes) = kura
        .persisted_count_and_unindexed_bytes()
        .expect("measure merge durable frontier");
    let pending = kura
        .pending_block_bytes(persisted_count, unindexed_bytes)
        .expect("measure merge pending canonical bytes");
    let terminal = kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("measure merge terminal reservations");
    let post_wsv = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure merge post-WSV reservations");
    let certified = kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure merge certified-bundle reservations");
    let exact_limit = used
        .checked_add(pending)
        .and_then(|bytes| bytes.checked_add(terminal))
        .and_then(|bytes| bytes.checked_add(post_wsv))
        .and_then(|bytes| bytes.checked_add(certified))
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .and_then(|bytes| bytes.checked_add(block_required))
        .and_then(|bytes| bytes.checked_add(merge_commit_required))
        .and_then(|bytes| bytes.checked_add(association_stage_required))
        .expect("exact merge evidence budget fits u64");
    Arc::get_mut(&mut kura)
        .expect("exclusive merge Kura before exact budget check")
        .max_disk_usage_bytes = exact_limit;
    kura.check_storage_budget(carrier.as_ref(), Some(&entry))
        .expect("exact post-WSV evidence reservation must admit the carrier");
    Arc::get_mut(&mut kura)
        .expect("exclusive merge Kura before negative budget check")
        .max_disk_usage_bytes = exact_limit - 1;
    let err = kura
        .check_storage_budget(carrier.as_ref(), Some(&entry))
        .expect_err("one byte below the exact post-WSV evidence reservation must reject");
    assert!(matches!(
        err,
        Error::StorageBudgetExceeded {
            limit,
            required,
            ..
        } if limit == exact_limit - 1 && required == exact_limit
    ));
}
#[test]
fn committed_merge_carrier_reconstructs_only_outstanding_post_wsv_components() {
    let temp_dir = TempDir::new().expect("create merge reservation temp dir");
    let cfg = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (mut kura, _) =
        Kura::new(&cfg, &RuntimeLaneConfig::default()).expect("initialize reservation Kura");
    let entrypoint = offline_top_up_entrypoint_for_index([0x71; 32], [0x72; 32]);
    let mut entry = merge_entry_with_indexed_entrypoint(entrypoint);
    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let carrier = next_merge_carrier(&mut blocks, &mut entry);
    let expected = kura
        .merge_lane_application_artifact_required_bytes_for_block(carrier.as_ref(), Some(&entry))
        .expect("account committed carrier envelope");
    kura.store_block(parent).expect("store reservation parent");
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
        .expect("store reservation carrier");
    assert_eq!(
        kura.post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read reservation total"),
        expected
    );
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
        .expect("exact carrier retry reconstructs the same reservation");
    assert_eq!(
        kura.post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read idempotent reservation total"),
        expected,
        "exact retry must not multiply the carrier envelope"
    );
    let wrong_hash = HashOf::from_untyped_unchecked(Hash::new(b"wrong reservation carrier"));
    kura.release_post_wsv_lane_artifact_budget_reservation(
        &entry,
        carrier.header().height().get(),
        wrong_hash,
    )
    .expect_err("another carrier must not release the reservation");
    assert_eq!(
        kura.post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read preserved reservation total"),
        expected
    );
    kura.release_post_wsv_lane_artifact_budget_reservation(
        &entry,
        carrier.header().height().get(),
        carrier.hash(),
    )
    .expect_err(
        "stable receipt/frontier components and terminal completion gate must block release",
    );
    assert_eq!(
        kura.post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read unreleased reservation total"),
        expected
    );
    kura.persist_merge_lane_block_application_receipts_from_committed_log(&entry)
        .expect_err("receipt publication must wait for exact carrier finality");
    let _ = persist_v2_finality_chain_through(
        &kura,
        NonZeroUsize::new(
            usize::try_from(carrier.header().height().get())
                .expect("reservation carrier height fits usize"),
        )
        .expect("reservation carrier height is non-zero"),
    );
    kura.persist_merge_lane_block_application_receipts_from_committed_log(&entry)
        .expect("persist exact finalized receipt and frontier components");
    let shared_transient = u64::try_from(BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES)
        .expect("append-intent transient fits u64");
    assert_eq!(
        kura.post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read consumed stable components"),
        shared_transient,
        "durability-attested receipt/frontier bytes must leave only the shared transient while terminal completion is pending"
    );
    kura.post_wsv_lane_artifact_budget_reservations
        .lock()
        .clear();
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
        .expect("restart-like exact carrier retry rebuilds outstanding state");
    assert_eq!(
        kura.post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read restart-reconstructed reservation total"),
        shared_transient,
        "restart-like reconstruction must not reserve already durable receipt/frontier components"
    );
    kura.release_post_wsv_lane_artifact_budget_reservation(
        &entry,
        carrier.header().height().get(),
        carrier.hash(),
    )
    .expect_err("incomplete terminal evidence must remain fail-closed after reconstruction");
    kura.post_wsv_lane_artifact_budget_reservations
        .lock()
        .clear();
    let stale_hash = HashOf::from_untyped_unchecked(Hash::new(b"stale lazy carrier"));
    kura.ensure_post_wsv_lane_artifact_budget_reservation(
        &entry,
        carrier.header().height().get(),
        stale_hash,
    )
    .expect_err("lazy reconstruction must authenticate the exact canonical carrier");
    assert_eq!(
        kura.post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read reservation after stale reconstruction"),
        0,
        "a stale lazy carrier must not strand a reservation",
    );
    let used = kura
        .kura_disk_usage_bytes()
        .expect("measure physical bytes before lazy reconstruction");
    let terminal = kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("measure global terminal envelope before lazy reconstruction");
    let (persisted_count, unindexed_bytes) = kura
        .persisted_count_and_unindexed_bytes()
        .expect("measure lazy reconstruction durable frontier");
    let pending = kura
        .pending_block_bytes(persisted_count, unindexed_bytes)
        .expect("measure lazy reconstruction pending canonical bytes");
    let certified = kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure lazy reconstruction certified-bundle reservations");
    let exact_required = used
        .checked_add(pending)
        .and_then(|bytes| bytes.checked_add(terminal))
        .and_then(|bytes| bytes.checked_add(shared_transient))
        .and_then(|bytes| bytes.checked_add(certified))
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .expect("lazy reconstruction capacity fits u64");
    Arc::get_mut(&mut kura)
        .expect("exclusive Kura before lazy capacity rejection")
        .max_disk_usage_bytes = exact_required - 1;
    let err = kura
        .ensure_post_wsv_lane_artifact_budget_reservation(
            &entry,
            carrier.header().height().get(),
            carrier.hash(),
        )
        .expect_err("one byte below the remaining envelope must reject lazy reconstruction");
    assert!(matches!(err, Error::StorageBudgetExceeded { .. }));
    assert_eq!(
        kura.post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read reservation after capacity rejection"),
        0,
        "capacity rejection must be atomic and leave no reservation",
    );
}
#[test]
fn direct_lane_receipt_preflights_its_exact_unreserved_append_peak() {
    let temp_dir = TempDir::new().expect("create direct receipt budget temp dir");
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
    let (mut kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.store_block(Arc::new(block))
        .expect("store direct receipt canonical evidence");
    let receipt = kura
        .recover_lane_block_application_receipt_artifact(&proposal)
        .expect("recover direct receipt fixture");
    assert_ne!(
        receipt.format,
        LaneBlockApplicationReceiptArtifactFormat::MergeExecution
    );
    let payload_len = u64::try_from(
        receipt
            .encode_framed()
            .expect("encode direct receipt fixture")
            .len(),
    )
    .expect("direct receipt length fits u64");
    let append_peak = payload_len
        .checked_add(Kura::maximum_index_growth_for_unresolved_sidecar_write(
            lane_block_height,
        ))
        .and_then(|bytes| {
            bytes.checked_add(
                u64::try_from(BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES)
                    .expect("append intent bound fits u64"),
            )
        })
        .expect("direct receipt append peak fits u64");
    let used = kura
        .kura_disk_usage_bytes()
        .expect("measure direct receipt physical baseline");
    let terminal = kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("measure direct receipt terminal envelope");
    let (persisted_count, unindexed_bytes) = kura
        .persisted_count_and_unindexed_bytes()
        .expect("measure direct receipt durable frontier");
    let pending = kura
        .pending_block_bytes(persisted_count, unindexed_bytes)
        .expect("measure direct receipt pending canonical bytes");
    let post_wsv = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure direct receipt post-WSV reservations");
    let certified = kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure direct receipt certified-bundle reservations");
    let exact_limit = used
        .checked_add(pending)
        .and_then(|bytes| bytes.checked_add(terminal))
        .and_then(|bytes| bytes.checked_add(post_wsv))
        .and_then(|bytes| bytes.checked_add(certified))
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .and_then(|bytes| bytes.checked_add(append_peak))
        .expect("direct receipt exact capacity fits u64");
    Arc::get_mut(&mut kura)
        .expect("exclusive Kura before direct receipt capacity check")
        .max_disk_usage_bytes = exact_limit - 1;
    kura.persist_lane_block_application_receipt(&proposal)
        .expect_err("one byte below the unreserved direct receipt peak must reject");
    let (data_path, index_path) =
        Kura::lane_block_application_receipt_paths_for_entry(lane_entry, temp_dir.path());
    assert!(!data_path.exists() && !index_path.exists());
    Arc::get_mut(&mut kura)
        .expect("exclusive Kura before exact direct receipt capacity check")
        .max_disk_usage_bytes = exact_limit;
    kura.persist_lane_block_application_receipt(&proposal)
        .expect("the exact unreserved direct receipt peak must admit");
}
#[test]
fn latest_execution_index_rejects_equal_height_forks_on_append_and_rebuild() {
    let first = merge_entry_with_indexed_entrypoint(offline_top_up_entrypoint_for_index(
        [0x81; 32], [0x82; 32],
    ));
    let mut fork = merge_entry_with_indexed_entrypoint(offline_top_up_entrypoint_for_index(
        [0x83; 32], [0x84; 32],
    ));
    fork.epoch_id = 2;
    let first_execution = first
        .execution_batch
        .as_ref()
        .and_then(|batch| batch.lanes.first())
        .expect("first fixture execution");
    let fork_execution = fork
        .execution_batch
        .as_ref()
        .and_then(|batch| batch.lanes.first())
        .expect("fork fixture execution");
    assert_eq!(
        (
            first_execution.proposal.descriptor.lane_id,
            first_execution.proposal.descriptor.dataspace_id,
            first_execution.proposal.descriptor.lane_incarnation,
            first_execution.proposal.descriptor.lane_block_height,
        ),
        (
            fork_execution.proposal.descriptor.lane_id,
            fork_execution.proposal.descriptor.dataspace_id,
            fork_execution.proposal.descriptor.lane_incarnation,
            fork_execution.proposal.descriptor.lane_block_height,
        ),
    );
    assert_ne!(first.canonical_hash(), fork.canonical_hash());
    let mut memory_log = MergeLedgerLog::in_memory(MERGE_LEDGER_CACHE_CAPACITY);
    assert!(memory_log.append(&first).expect("append first execution"));
    let error = memory_log
        .append(&fork)
        .expect_err("equal-height execution fork must fail before append");
    assert!(matches!(error, Error::MergeCarrierConflict(_)));
    assert_eq!(memory_log.total_entries, 1);
    assert_eq!(
        memory_log.latest_execution_entry(
            first_execution.proposal.descriptor.lane_id,
            first_execution.proposal.descriptor.dataspace_id,
            first_execution.proposal.descriptor.lane_incarnation,
        ),
        Some((
            first_execution.proposal.descriptor.lane_block_height,
            first.canonical_hash(),
        )),
    );
    let temp_dir = TempDir::new().expect("equal-height merge-log temp dir");
    let path = temp_dir.path().join("merge.log");
    let mut file = fs::File::create(&path).expect("create raw forked merge log");
    for entry in [&first, &fork] {
        let bytes = entry.encode();
        file.write_all(
            &u32::try_from(bytes.len())
                .expect("raw frame length fits u32")
                .to_le_bytes(),
        )
        .expect("write raw frame length");
        file.write_all(&bytes).expect("write raw frame payload");
    }
    file.sync_all().expect("sync raw forked merge log");
    let error = MergeLedgerLog::open_at(&path, MERGE_LEDGER_CACHE_CAPACITY)
        .expect_err("startup reconstruction must reject an equal-height execution fork");
    assert!(matches!(error, Error::MergeCarrierConflict(_)));
}
#[test]
fn bounded_forward_execution_reconstruction_keeps_an_incomplete_nonlatest_carrier() {
    let first = merge_entry_with_indexed_entrypoint(offline_top_up_entrypoint_for_index(
        [0x91; 32], [0x92; 32],
    ));
    let first_descriptor = first
        .execution_batch
        .as_ref()
        .and_then(|batch| batch.lanes.first())
        .expect("first forward-reconstruction execution")
        .proposal
        .descriptor
        .clone();
    let mut second = merge_entry_with_indexed_entrypoint(offline_top_up_entrypoint_for_index(
        [0x93; 32], [0x94; 32],
    ));
    second.epoch_id = 2;
    {
        let batch = second
            .execution_batch
            .as_mut()
            .expect("second forward-reconstruction batch");
        let execution = batch
            .lanes
            .first_mut()
            .expect("second forward-reconstruction execution");
        let descriptor = &mut execution.proposal.descriptor;
        descriptor.proposal_height = first_descriptor.proposal_height + 1;
        descriptor.previous_lane_block_height = first_descriptor.lane_block_height;
        descriptor.previous_lane_block_descriptor_hash = Some(first_descriptor.descriptor_hash);
        descriptor.lane_block_height = first_descriptor.lane_block_height + 1;
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        execution.proposal.proposal_hash = execution.proposal.computed_proposal_hash();
        execution.origin_proposal = execution.proposal.clone();
        batch.execution_root = crate::merge::merge_execution_root(&batch.lanes);
        batch.batch_hash = crate::merge::merge_execution_batch_hash(batch);
    }
    let second_descriptor = second
        .execution_batch
        .as_ref()
        .and_then(|batch| batch.lanes.first())
        .expect("second forward-reconstruction execution")
        .proposal
        .descriptor
        .clone();
    let first_identity = (
        first_descriptor.lane_id,
        first_descriptor.dataspace_id,
        first_descriptor.lane_incarnation,
        first_descriptor.lane_block_height,
        first_descriptor.proposal_height,
    );
    let second_identity = (
        second_descriptor.lane_id,
        second_descriptor.dataspace_id,
        second_descriptor.lane_incarnation,
        second_descriptor.lane_block_height,
        second_descriptor.proposal_height,
    );
    let identities = BTreeSet::from([first_identity, second_identity]);
    let temp_dir = TempDir::new().expect("forward execution reconstruction temp dir");
    let path = temp_dir.path().join("merge.log");
    {
        let mut log = MergeLedgerLog::open_at(&path, 1).expect("create bounded merge log");
        assert!(log.append(&first).expect("append incomplete height N"));
        assert!(log.append(&second).expect("append Kura-ahead height N+1"));
    }
    let mut reopened =
        MergeLedgerLog::open_at(&path, 1).expect("reconstruct latest index with only N+1 cached");
    assert_eq!(
        reopened.latest_execution_entry(
            first_descriptor.lane_id,
            first_descriptor.dataspace_id,
            first_descriptor.lane_incarnation,
        ),
        Some((second_descriptor.lane_block_height, second.canonical_hash())),
    );
    let exact = reopened
        .execution_entries_for_bounded_identities(&identities)
        .expect("forward-reconstruct both locally incomplete identities");
    assert_eq!(exact.get(&first_identity), Some(&first.canonical_hash()));
    assert_eq!(exact.get(&second_identity), Some(&second.canonical_hash()));
    assert_eq!(
        reopened.complete_execution_scans, 1,
        "all older incomplete identities share one explicit startup pass",
    );
}
