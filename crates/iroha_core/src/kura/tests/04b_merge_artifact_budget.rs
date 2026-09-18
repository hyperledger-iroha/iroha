#[test]
fn merge_carrier_budget_reserves_receipt_frontier_without_double_counting_terminal_slot() {
    let temp_dir = TempDir::new().expect("create merge evidence budget temp dir");
    let cfg = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (mut kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&cfg, &RuntimeLaneConfig::default())
            .expect("initialize merge budget Kura");
    let entrypoint = indexed_log_entrypoint([0x61; 32], [0x62; 32]);
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
        Kura::merge_lane_block_execution_source(execution),
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
fn post_wsv_successor_plan_accepts_lower_durable_frontier() {
    let entrypoint = indexed_log_entrypoint([0x69; 32], [0x6A; 32]);
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
        Kura::merge_lane_block_execution_source(execution),
        carrier.header().height().get(),
        carrier.hash(),
    );
    let predecessor = LaneMergeApplicationFrontierV1::from_receipt(&receipt)
        .expect("predecessor merge receipt projects a frontier");
    let mut successor = predecessor;
    successor.proposal_height = successor
        .proposal_height
        .checked_add(1)
        .expect("successor proposal height does not overflow");
    successor.lane_block_height = successor
        .lane_block_height
        .checked_add(1)
        .expect("successor lane height does not overflow");
    successor.lane_block_descriptor_hash = Hash::new(b"post-wsv-successor-descriptor");
    successor.proposal_hash = Hash::new(b"post-wsv-successor-proposal");
    successor.receipt_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"post-wsv-successor-receipt"));
    assert!(
        !Kura::post_wsv_frontier_conflicts_with_execution(&predecessor, &successor),
        "the durable predecessor frontier must not conflict with a contiguous successor plan"
    );

    let mut same_height_fork = predecessor;
    same_height_fork.application_block_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"post-wsv-same-height-fork"));
    assert!(
        Kura::post_wsv_frontier_conflicts_with_execution(&same_height_fork, &predecessor),
        "different carrier evidence at the same lane height must remain fail-closed"
    );
}
#[test]
fn committed_merge_carrier_reconstructs_only_outstanding_post_wsv_components() {
    let temp_dir = TempDir::new().expect("create merge reservation temp dir");
    let cfg = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (mut kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&cfg, &RuntimeLaneConfig::default())
            .expect("initialize reservation Kura");
    let entrypoint = indexed_log_entrypoint([0x71; 32], [0x72; 32]);
    let mut entry = merge_entry_with_indexed_entrypoint(entrypoint);
    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let carrier = next_merge_carrier(&mut blocks, &mut entry);
    let execution = entry
        .execution_batch
        .as_ref()
        .expect("one execution batch")
        .lanes
        .first()
        .expect("one active execution");
    let descriptor = &execution.proposal.descriptor;
    kura.bind_lane_storage_network(execution.autonomous_network_id)
        .expect("bind storage to the exact signed execution network before initial publication");
    publish_initial_configured_lane_geometry_for_test(
        &kura,
        &RuntimeLaneConfig::default(),
        &BTreeMap::from([(descriptor.lane_id, descriptor.lane_incarnation)]),
    );
    {
        let _geometry_guard = kura.lane_geometry_lock.lock();
        let active = kura
            .lane_storage_entry(descriptor.lane_id)
            .expect("the fixture published its actual execution route");
        assert_eq!(active.network_id, execution.autonomous_network_id);
        assert_eq!(active.dataspace_id, descriptor.dataspace_id);
        assert_eq!(active.incarnation, descriptor.lane_incarnation);
        assert_eq!(active.activation_height, 0);
        kura.require_active_lane_artifact(&active, descriptor)
            .expect("the exact full marker admits this execution before carrier storage");
    }
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
fn canonical_lane_receipt_preflights_its_exact_unreserved_append_peak() {
    let temp_dir = TempDir::new().expect("create canonical receipt budget temp dir");
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
    store_finalized_fixture_block(&kura, Arc::new(block));
    let receipt = kura
        .recover_lane_block_application_receipt_artifact(&proposal)
        .expect("recover canonical receipt fixture");
    assert_ne!(
        receipt.format,
        LaneBlockApplicationReceiptArtifactFormat::MergeExecution
    );
    let payload_len = u64::try_from(
        receipt
            .encode_framed()
            .expect("encode canonical receipt fixture")
            .len(),
    )
    .expect("canonical receipt length fits u64");
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
        .expect("canonical receipt append peak fits u64");
    let used = kura
        .kura_disk_usage_bytes()
        .expect("measure canonical receipt physical baseline");
    let terminal = kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("measure canonical receipt terminal envelope");
    let (persisted_count, unindexed_bytes) = kura
        .persisted_count_and_unindexed_bytes()
        .expect("measure canonical receipt durable frontier");
    let pending = kura
        .pending_block_bytes(persisted_count, unindexed_bytes)
        .expect("measure canonical receipt pending bytes");
    let post_wsv = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure canonical receipt post-WSV reservations");
    let certified = kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure canonical receipt certified-bundle reservations");
    let exact_limit = used
        .checked_add(pending)
        .and_then(|bytes| bytes.checked_add(terminal))
        .and_then(|bytes| bytes.checked_add(post_wsv))
        .and_then(|bytes| bytes.checked_add(certified))
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .and_then(|bytes| bytes.checked_add(append_peak))
        .expect("canonical receipt exact capacity fits u64");
    Arc::get_mut(&mut kura)
        .expect("exclusive Kura before canonical receipt capacity check")
        .max_disk_usage_bytes = exact_limit - 1;
    kura.persist_lane_block_application_receipt(&proposal)
        .expect_err("one byte below the unreserved canonical receipt peak must reject");
    let lane_entry = kura
        .lane_storage_entry(lane_entry.lane_id)
        .expect("capture the exact journal-published fixture identity");
    let lane_entry = &lane_entry;
    let (data_path, index_path) =
        Kura::lane_block_application_receipt_paths_for_entry(lane_entry, temp_dir.path());
    assert!(!data_path.exists() && !index_path.exists());
    Arc::get_mut(&mut kura)
        .expect("exclusive Kura before exact canonical receipt capacity check")
        .max_disk_usage_bytes = exact_limit;
    kura.persist_lane_block_application_receipt(&proposal)
        .expect("the exact unreserved canonical receipt peak must admit");
}
#[test]
fn latest_execution_index_rejects_equal_height_forks_on_append_and_rebuild() {
    let first = merge_entry_with_indexed_entrypoint(indexed_log_entrypoint([0x81; 32], [0x82; 32]));
    let mut fork =
        merge_entry_with_indexed_entrypoint(indexed_log_entrypoint([0x83; 32], [0x84; 32]));
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
    let first = merge_entry_with_indexed_entrypoint(indexed_log_entrypoint([0x91; 32], [0x92; 32]));
    let first_descriptor = first
        .execution_batch
        .as_ref()
        .and_then(|batch| batch.lanes.first())
        .expect("first forward-reconstruction execution")
        .proposal
        .descriptor
        .clone();
    let mut second =
        merge_entry_with_indexed_entrypoint(indexed_log_entrypoint([0x93; 32], [0x94; 32]));
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

/// Store an actual merge carrier before exercising its existing post-WSV owner.
fn stored_post_wsv_identity_fixture() -> (TempDir, Arc<Kura>, MergeLedgerEntry, Arc<SignedBlock>) {
    let directory = TempDir::new().expect("post-WSV target fixture");
    let config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
    let lanes = RuntimeLaneConfig::default();
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lanes)
        .expect("open post-WSV target fixture");
    let mut entry =
        merge_entry_with_indexed_entrypoint(indexed_log_entrypoint([0x73; 32], [0x74; 32]));
    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let carrier = next_merge_carrier(&mut blocks, &mut entry);
    let execution = &entry
        .execution_batch
        .as_ref()
        .expect("execution batch")
        .lanes[0];
    kura.bind_lane_storage_network(execution.autonomous_network_id)
        .expect("bind the signed execution network");
    publish_initial_configured_lane_geometry_for_test(
        &kura,
        &lanes,
        &BTreeMap::from([(
            execution.proposal.descriptor.lane_id,
            execution.proposal.descriptor.lane_incarnation,
        )]),
    );
    kura.store_block(parent).expect("store exact parent");
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
        .expect("store exact merge carrier");
    assert!(kura.post_wsv_lane_artifact_budget_reserved_bytes().unwrap() > 0);
    (directory, kura, entry, carrier)
}

/// Invoke the same authenticated reservation owner as an exact store retry,
/// before its outer committed-write fault handler poisons a failed store.
fn reconcile_stored_post_wsv_fixture(
    kura: &Kura,
    entry: &MergeLedgerEntry,
    carrier: &SignedBlock,
) -> Result<u64> {
    let _prune = kura.prune_lock.lock();
    kura.ensure_prune_recovery_not_required()?;
    let _canonical = kura.canonical_chain_lock.lock();
    kura.ensure_post_wsv_lane_artifact_budget_reservation_pre_finality_under_prune_and_canonical_guards(
        entry, carrier,
    )
}

#[test]
fn post_wsv_same_instance_corruption_cannot_discard_or_omit_reservation() {
    let (directory, kura, entry, carrier) = stored_post_wsv_identity_fixture();
    let execution = &entry.execution_batch.as_ref().unwrap().lanes[0];
    let active = kura
        .lane_storage_entry(execution.proposal.descriptor.lane_id)
        .unwrap();
    let marker = active
        .blocks_dir(kura.store_root())
        .join(".lane-incarnation.norito");
    let original_marker = fs::read(&marker).unwrap();
    let expected = kura.post_wsv_lane_artifact_budget_reserved_bytes().unwrap();
    {
        let _geometry = kura.lane_geometry_lock.lock();
        assert!(
            kura.find_existing_work_lane_storage_entry_under_geometry_guard(
                active.network_id,
                active.lane_id,
                active.dataspace_id,
                active.incarnation,
                active.activation_height,
            )
            .is_err(),
            "backdated work is invalid even through the current-identity fast path"
        );
    }
    for fault in 0..3 {
        match fault {
            0 => fs::remove_file(&marker).expect("remove same-instance marker"),
            1 => fs::write(&marker, b"corrupt same-instance marker").unwrap(),
            _ => kura
                .substitute_lane_marker_identity_for_test(
                    &active,
                    Hash::new(b"foreign post-WSV marker incarnation"),
                    active.activation_height,
                )
                .expect("replace only the original marker identity"),
        }
        let fault_tree = snapshot_regular_files_recursively(directory.path());
        // Both an existing envelope and a process-local projection rebuilt from
        // empty must refuse the same failed source authentication atomically.
        for has_reservation in [true, false] {
            if !has_reservation {
                kura.post_wsv_lane_artifact_budget_reservations
                    .lock()
                    .clear();
            }
            let before = kura
                .post_wsv_lane_artifact_budget_reservations
                .lock()
                .clone();
            reconcile_stored_post_wsv_fixture(&kura, &entry, &carrier)
                .expect_err("corrupt exact identity must not become historical absence");
            assert_eq!(
                *kura.post_wsv_lane_artifact_budget_reservations.lock(),
                before
            );
            assert_eq!(
                snapshot_regular_files_recursively(directory.path()),
                fault_tree
            );
        }
        fs::write(&marker, &original_marker).expect("restore exact original evidence");
        assert_eq!(
            reconcile_stored_post_wsv_fixture(&kura, &entry, &carrier).unwrap(),
            expected
        );
        kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
            .expect("the real exact retry retains the same restored envelope");
        assert_eq!(
            kura.post_wsv_lane_artifact_budget_reserved_bytes().unwrap(),
            expected
        );
    }
}

#[test]
fn post_wsv_retained_lookup_proves_absence_and_propagates_storage_failure() {
    let (directory, kura, entry, carrier) = stored_post_wsv_identity_fixture();
    let execution = &entry.execution_batch.as_ref().unwrap().lanes[0];
    let descriptor = &execution.proposal.descriptor;
    let original = kura.lane_storage_entry(descriptor.lane_id).unwrap();
    let expected = kura.post_wsv_lane_artifact_budget_reserved_bytes().unwrap();
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
    drop(kura);
    let config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .expect("cold restore authenticates retained post-WSV ownership");
    assert!(kura.lane_storage_entries.lock().is_empty());
    assert_eq!(
        reconcile_stored_post_wsv_fixture(&kura, &entry, &carrier).unwrap(),
        expected
    );
    let mut absent = [original.identity; 4];
    absent[0].network_id = test_network_id(b"absent post-WSV network");
    absent[1].lane_id = LaneId::new(1);
    absent[2].dataspace_id = DataSpaceId::new(1);
    absent[3].incarnation = Hash::new(b"absent post-WSV incarnation");
    {
        let _geometry = kura.lane_geometry_lock.lock();
        assert_eq!(
            kura.find_existing_work_lane_storage_entry_under_geometry_guard(
                original.network_id,
                original.lane_id,
                original.dataspace_id,
                original.incarnation,
                descriptor.proposal_height,
            )
            .unwrap(),
            Some(original.clone()),
        );
        for identity in absent {
            assert_eq!(
                kura.find_existing_work_lane_storage_entry_under_geometry_guard(
                    identity.network_id,
                    identity.lane_id,
                    identity.dataspace_id,
                    identity.incarnation,
                    descriptor.proposal_height,
                )
                .unwrap(),
                None,
                "a complete authenticated inventory proves this exact identity absent",
            );
            assert!(
                kura.existing_work_lane_storage_entry_under_geometry_guard(
                    identity.network_id,
                    identity.lane_id,
                    identity.dataspace_id,
                    identity.incarnation,
                    descriptor.proposal_height,
                )
                .is_err(),
                "mandatory-presence callers retain their strict contract"
            );
        }
        assert!(
            kura.find_existing_work_lane_storage_entry_under_geometry_guard(
                original.network_id,
                original.lane_id,
                original.dataspace_id,
                original.incarnation,
                original.activation_height,
            )
            .is_err(),
            "backdated work is invalid for a present retained identity"
        );
    }
    let merge_path = original.merge_log_path(kura.store_root());
    let held = TempDir::new().expect("hold the original retained physical object");
    let held_merge = held.path().join("original-merge.log");
    fs::rename(&merge_path, &held_merge).expect("remove an actually retained physical half");
    let fault_tree = snapshot_regular_files_recursively(directory.path());
    let before = kura
        .post_wsv_lane_artifact_budget_reservations
        .lock()
        .clone();
    reconcile_stored_post_wsv_fixture(&kura, &entry, &carrier)
        .expect_err("missing retained physical storage is not proven absence");
    {
        let _geometry = kura.lane_geometry_lock.lock();
        assert!(
            kura.find_existing_work_lane_storage_entry_under_geometry_guard(
                absent[3].network_id,
                absent[3].lane_id,
                absent[3].dataspace_id,
                absent[3].incarnation,
                descriptor.proposal_height,
            )
            .is_err(),
            "failed retained authentication cannot prove even a foreign identity absent"
        );
    }
    assert_eq!(
        *kura.post_wsv_lane_artifact_budget_reservations.lock(),
        before
    );
    assert_eq!(
        snapshot_regular_files_recursively(directory.path()),
        fault_tree
    );
    assert!(kura.lane_storage_entries.lock().is_empty());
    fs::rename(&held_merge, &merge_path).expect("restore the original retained physical object");
    assert_eq!(
        reconcile_stored_post_wsv_fixture(&kura, &entry, &carrier).unwrap(),
        expected
    );
    assert!(kura.lane_storage_entries.lock().is_empty());
}

#[test]
fn post_wsv_prepend_reserves_bounded_future_window_and_exact_retry() {
    let crash_cuts: [(bool, fn()); 2] = [
        (false, fail_next_bound_progress_append_data_sync_for_tests),
        (true, fail_next_bound_progress_append_index_sync_for_tests),
    ];
    for (index_written, crash) in crash_cuts {
        let directory = TempDir::new().unwrap();
        let config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
        let lanes = RuntimeLaneConfig::default();
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lanes).unwrap();
        let mut entry =
            merge_entry_with_indexed_entrypoint(indexed_log_entrypoint([0x75; 32], [0x76; 32]));
        let mut blocks = DummyBlocks::new();
        let parent = blocks.next();
        let carrier = next_merge_carrier(&mut blocks, &mut entry);
        let execution = &entry.execution_batch.as_ref().unwrap().lanes[0];
        let descriptor = &execution.proposal.descriptor;
        kura.bind_lane_storage_network(execution.autonomous_network_id)
            .unwrap();
        publish_initial_configured_lane_geometry_for_test(
            &kura,
            &lanes,
            &BTreeMap::from([(descriptor.lane_id, descriptor.lane_incarnation)]),
        );
        let lane = kura.lane_storage_entry(descriptor.lane_id).unwrap();
        let (data, index) =
            Kura::lane_block_application_receipt_paths_for_entry(&lane, &kura.store_root());
        fs::create_dir_all(data.parent().unwrap()).unwrap();
        // A canonical empty retained index with an advanced base requires a real
        // backward prepend. It contains no fabricated receipt or finality evidence.
        fs::write(&data, []).unwrap();
        fs::write(&index, SidecarIndexLayout::base_header(513)).unwrap();
        kura.store_block(parent).unwrap();
        let extra = {
            let _prune = kura.prune_lock.lock();
            let _canonical = kura.canonical_chain_lock.lock();
            kura.post_wsv_prepend_admission_extra_under_prune_and_canonical_guards(
                &carrier,
                Some(&entry),
            )
            .unwrap()
        };
        assert!(extra > BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES as u64);
        let ordinary = kura
            .merge_lane_application_artifact_required_bytes_for_carrier(
                &entry,
                carrier.header().height().get(),
                carrier.hash(),
            )
            .unwrap();
        kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
            .unwrap();
        assert_eq!(
            kura.post_wsv_lane_artifact_budget_reserved_bytes().unwrap(),
            ordinary + extra
        );
        let admitted = kura
            .post_wsv_lane_artifact_budget_reservations
            .lock()
            .clone();
        kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
            .unwrap();
        assert_eq!(
            *kura.post_wsv_lane_artifact_budget_reservations.lock(),
            admitted
        );
        assert!(kura.post_wsv_receipt_compaction_is_pinned_locked(&lane));
        {
            let _geometry = kura.lane_geometry_lock.lock();
            let _sidecar = kura.sidecar_lock.lock();
            let retained_layout =
                SidecarIndexLayout::based(513, INDEXED_SIDECAR_BASE_HEADER_SIZE as u64).unwrap();
            // The shared writer admission applies equally to Direct receipts. A
            // new high row may consume the last admitted slot, but cannot grow the
            // pair beyond the lower pending receipt's bounded future prepend.
            kura.validate_post_wsv_receipt_window_locked(
                &lane,
                Some(retained_layout),
                MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES as u64,
            )
            .unwrap();
            let before = snapshot_regular_test_tree(directory.path());
            let error = kura
                .validate_post_wsv_receipt_window_locked(
                    &lane,
                    Some(retained_layout),
                    MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES as u64 + 1,
                )
                .unwrap_err();
            assert!(matches!(error, Error::IO(error, _) if error.kind() == ErrorKind::WouldBlock));
            assert_eq!(snapshot_regular_test_tree(directory.path()), before);
            assert_eq!(
                *kura.post_wsv_lane_artifact_budget_reservations.lock(),
                admitted
            );
        }
        let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
        crash();
        assert!(
            kura.persist_merge_lane_block_application_receipts_from_committed_log(&entry)
                .is_err()
        );
        let interrupted = snapshot_regular_test_tree(directory.path());
        let still_reserved = kura.post_wsv_lane_artifact_budget_reserved_bytes().unwrap();
        assert_eq!(still_reserved, ordinary + extra);
        reconcile_stored_post_wsv_fixture(&kura, &entry, &carrier).unwrap();
        assert_eq!(snapshot_regular_test_tree(directory.path()), interrupted);
        let receipt_bytes = admitted
            .values()
            .flat_map(|reservation| reservation.plan.stable_components.iter())
            .filter_map(|(component, bytes)| {
                matches!(component, PostWsvLaneArtifactStableComponentId::Receipt(_))
                    .then_some(*bytes)
            })
            .sum::<u64>();
        // The index-sync cut has already exposed the exact final receipt, so
        // durable readback can consume that stable component. Its intent is
        // still physically charged and must be authenticated before cleanup.
        let expected_pending = if index_written {
            ordinary - receipt_bytes
        } else {
            still_reserved
        };
        assert_eq!(
            kura.post_wsv_lane_artifact_budget_reserved_bytes().unwrap(),
            expected_pending
        );
        let intent_path = Kura::bound_progress_append_intent_path(&index);
        let original_intent_bytes = fs::read(&intent_path).unwrap();
        let original_intent: BoundProgressAppendIntentV1 =
            norito::decode_canonical(&original_intent_bytes).unwrap();
        assert!(original_intent.is_prepend());
        assert!(
            kura.read_lane_application_receipt(descriptor.lane_id, descriptor.lane_block_height)
                .is_err()
        );
        assert_eq!(snapshot_regular_test_tree(directory.path()), interrupted);
        // A superseded build belongs to the exact intent, but must not be removed
        // until that intent has joined the authenticated receipt being retried.
        let build_path = Kura::bound_progress_append_build_path(&index);
        fs::write(&build_path, &original_intent_bytes).unwrap();
        let mut wrong_payload = original_intent.clone();
        wrong_payload.payload_hash = Hash::new(b"another canonical receipt");
        let mut wrong_height = original_intent.clone();
        wrong_height.height += 1;
        let mut wrong_namespace = original_intent.clone();
        wrong_namespace
            .namespace_components
            .push("foreign".to_owned());
        let mut damaged = original_intent_bytes.clone();
        *damaged.last_mut().unwrap() ^= 1;
        for (kind, bytes) in [
            (
                "receipt payload",
                norito::encode_canonical(&wrong_payload.seal()).unwrap(),
            ),
            (
                "receipt height",
                norito::encode_canonical(&wrong_height.seal()).unwrap(),
            ),
            (
                "receipt namespace",
                norito::encode_canonical(&wrong_namespace.seal()).unwrap(),
            ),
            ("damaged intent", damaged),
        ] {
            fs::write(&intent_path, &bytes).unwrap();
            let rejected = snapshot_regular_test_tree(directory.path());
            assert!(
                kura.persist_merge_lane_block_application_receipts_from_committed_log(&entry)
                    .is_err(),
                "{kind}"
            );
            assert_eq!(
                snapshot_regular_test_tree(directory.path()),
                rejected,
                "{kind} must not change main files or clean temporaries"
            );
            assert_eq!(
                kura.post_wsv_lane_artifact_budget_reserved_bytes().unwrap(),
                expected_pending
            );
        }
        fs::write(&intent_path, &original_intent_bytes).unwrap();
        let competing = index.with_extension("index.tmp");
        fs::write(&competing, b"unowned rewrite").unwrap();
        let rejected = snapshot_regular_test_tree(directory.path());
        assert!(
            kura.persist_merge_lane_block_application_receipts_from_committed_log(&entry)
                .is_err()
        );
        assert_eq!(snapshot_regular_test_tree(directory.path()), rejected);
        assert_eq!(
            kura.post_wsv_lane_artifact_budget_reserved_bytes().unwrap(),
            expected_pending
        );
        fs::remove_file(&competing).unwrap();
        let before_retry = snapshot_regular_test_tree(directory.path());
        reconcile_stored_post_wsv_fixture(&kura, &entry, &carrier).unwrap();
        assert_eq!(
            snapshot_regular_test_tree(directory.path()),
            before_retry,
            "planning still cannot promote the owned intent"
        );
        kura.persist_merge_lane_block_application_receipts_from_committed_log(&entry)
            .unwrap();
        assert!(!intent_path.exists());
        assert!(!build_path.exists());
        assert!(!kura.post_wsv_receipt_compaction_is_pinned_locked(&lane));
        assert_eq!(
            kura.post_wsv_lane_artifact_budget_reserved_bytes().unwrap(),
            BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES as u64
        );
        let mut file = std::fs::File::open(&index).unwrap();
        let len = file.metadata().unwrap().len();
        assert_eq!(
            SidecarIndexLayout::read_from(&mut file, len)
                .unwrap()
                .base_height,
            1
        );
        let complete = snapshot_regular_test_tree(directory.path());
        kura.persist_merge_lane_block_application_receipts_from_committed_log(&entry)
            .unwrap();
        assert_eq!(snapshot_regular_test_tree(directory.path()), complete);
        assert_eq!(
            kura.post_wsv_lane_artifact_budget_reserved_bytes().unwrap(),
            BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES as u64
        );
    }
}

#[test]
fn post_wsv_unwritten_receipt_pins_compaction_base_until_actual_publication() {
    let directory = TempDir::new().unwrap();
    let mut config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
    config.lane_history_retention = NonZeroUsize::MIN;
    let lanes = RuntimeLaneConfig::default();
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lanes).unwrap();
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let mut predecessor = None;
    let mut last = None;
    for height in 1..=2 {
        let payload = linked_compaction_payload(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            height,
            predecessor,
            &signer,
        );
        predecessor = Some(payload.origin_proposal.descriptor.descriptor_hash);
        let prepared = prepare_cold_autonomous_certification_for_capacity_payload(
            &kura, &lanes, &payload, &signer,
        );
        kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
            .unwrap();
        last = Some((payload, prepared.source));
    }
    let (payload, source) = last.unwrap();
    let execution =
        canonical_terminal_merge_execution_from_durable_source_for_test(&payload, source);
    let (parent, carrier, entry) = canonical_terminal_merge_carrier_for_test(vec![execution], 1);
    let lane = kura.lane_storage_entry(LaneId::SINGLE).unwrap();
    let (data, index) =
        Kura::lane_block_application_receipt_paths_for_entry(&lane, &kura.store_root());
    // One absent historical slot is canonical sparse index state. There is no
    // receipt at the pending height and no fake applied/terminal authority.
    fs::write(&data, []).unwrap();
    let mut empty = SidecarIndexLayout::base_header(1).to_vec();
    empty.extend_from_slice(&SidecarIndexEntry { offset: 0, len: 0 }.to_bytes());
    fs::write(&index, &empty).unwrap();
    kura.store_block(parent).unwrap();
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
        .unwrap();
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(2_usize));
    let batch = entry.execution_batch.as_ref().unwrap();
    let receipt = LaneBlockApplicationReceiptArtifact::new_merge_execution(
        &entry,
        batch,
        &batch.lanes[0],
        Kura::merge_lane_block_execution_source(&batch.lanes[0]),
        carrier.header().height().get(),
        carrier.hash(),
    );
    let frontier = LaneMergeApplicationFrontierV1::from_receipt(&receipt).unwrap();
    assert_eq!(frontier.lane_block_height, 2);
    assert!(kura.post_wsv_receipt_compaction_is_pinned_locked(&lane));
    compact_fixture_lane_histories(&kura, &lane, &frontier).unwrap();
    assert_eq!(
        fs::read(&index).unwrap(),
        empty,
        "pending ordinary receipt cannot become a prepend"
    );
    kura.persist_merge_lane_block_application_receipts_from_committed_log(&entry)
        .unwrap();
    assert!(!kura.post_wsv_receipt_compaction_is_pinned_locked(&lane));
    compact_fixture_lane_histories(&kura, &lane, &frontier).unwrap();
    let mut file = std::fs::File::open(&index).unwrap();
    let len = file.metadata().unwrap().len();
    assert_eq!(
        SidecarIndexLayout::read_from(&mut file, len)
            .unwrap()
            .base_height,
        2
    );
}

#[test]
fn post_wsv_absent_receipts_keep_ordinary_budgets_when_higher_writes_first() {
    let directory = TempDir::new().unwrap();
    let config = kura_config_for_dir(&directory, BLOCKS_IN_MEMORY);
    let lanes = RuntimeLaneConfig::default();
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lanes).unwrap();
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let mut predecessor = None;
    let mut executions = Vec::new();
    for height in 1..=2 {
        let payload = linked_compaction_payload(
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            height,
            predecessor,
            &signer,
        );
        predecessor = Some(payload.origin_proposal.descriptor.descriptor_hash);
        let prepared = prepare_cold_autonomous_certification_for_capacity_payload(
            &kura, &lanes, &payload, &signer,
        );
        kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
            .unwrap();
        executions.push(
            canonical_terminal_merge_execution_from_durable_source_for_test(
                &payload,
                prepared.source,
            ),
        );
    }
    let higher = executions.pop().unwrap();
    let lower = executions.pop().unwrap();
    let (parent, low_carrier, low_entry) =
        canonical_terminal_merge_carrier_for_test(vec![lower], 1);
    let (_, _, mut high_entry) = canonical_terminal_merge_carrier_for_test(vec![higher], 2);
    let mut generator = DummyBlocks {
        blocks: vec![Arc::clone(&parent), Arc::clone(&low_carrier)],
    };
    let header = crate::merge::merge_application_header_from_carrier(&generator.next().header());
    let raw = Arc::new(
        iroha_data_model::block::builder::BlockBuilder::new(header)
            .build_with_signature(0, SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key()),
    );
    let batch = high_entry.execution_batch.as_mut().unwrap();
    batch.application_block_header =
        crate::merge::merge_application_header_from_carrier(&raw.header());
    batch.batch_hash = crate::merge::merge_execution_batch_hash(batch);
    let high_carrier = bind_merge_entry_to_carrier(raw, &mut high_entry);
    let mut high_carrier = high_carrier.as_ref().clone();
    attach_ok_results_to_block(&mut high_carrier);
    crate::sumeragi::exec::NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(
        &high_carrier, Some(&high_entry)).unwrap();
    let high_carrier = Arc::new(high_carrier);
    let lane = kura.lane_storage_entry(LaneId::SINGLE).unwrap();
    let (data, index) =
        Kura::lane_block_application_receipt_paths_for_entry(&lane, &kura.store_root());
    assert!(!data.exists() && !index.exists());
    kura.store_block(parent).unwrap();
    kura.store_block_with_merge_entry(Arc::clone(&low_carrier), &low_entry)
        .unwrap();
    let low_budget = kura.post_wsv_lane_artifact_budget_reserved_bytes().unwrap();
    {
        let _geometry = kura.lane_geometry_lock.lock();
        let _sidecar = kura.sidecar_lock.lock();
        assert!(
            kura.validate_post_wsv_receipt_window_locked(
                &lane,
                None,
                MAX_INDEXED_SIDECAR_GAP_ENTRIES + 2
            )
            .is_err()
        );
        assert_eq!(kura.pending_post_wsv_receipt_range_locked(&lane, 2), (1, 2));
    }
    assert!(!data.exists() && !index.exists());
    assert_eq!(
        kura.post_wsv_lane_artifact_budget_reserved_bytes().unwrap(),
        low_budget
    );
    kura.store_block_with_merge_entry(Arc::clone(&high_carrier), &high_entry)
        .unwrap();
    assert!(
        kura.post_wsv_lane_artifact_budget_reservations
            .lock()
            .values()
            .all(|reservation| reservation.prepend_transient_bytes == 0)
    );
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(3_usize));
    kura.persist_merge_lane_block_application_receipts_from_committed_log(&high_entry)
        .unwrap();
    let mut file = std::fs::File::open(&index).unwrap();
    let len = file.metadata().unwrap().len();
    let layout = SidecarIndexLayout::read_from(&mut file, len).unwrap();
    assert_eq!(
        layout.base_height, 1,
        "higher first writer retains the admitted lower base"
    );
    assert_eq!(layout.entry_count, 2);
    assert!(
        kura.read_lane_block_application_receipt(LaneId::SINGLE, 1)
            .is_none()
    );
    assert!(
        kura.read_lane_block_application_receipt(LaneId::SINGLE, 2)
            .is_some()
    );
    kura.persist_merge_lane_block_application_receipts_from_committed_log(&low_entry)
        .unwrap();
    assert!(
        kura.read_lane_block_application_receipt(LaneId::SINGLE, 1)
            .is_some()
    );
    assert!(
        kura.read_lane_block_application_receipt(LaneId::SINGLE, 2)
            .is_some()
    );
    let low_frontier_bytes = {
        let plan = kura
            .post_wsv_lane_artifact_budget_plan(
                &low_entry,
                low_carrier.header().height().get(),
                low_carrier.hash(),
            )
            .unwrap()
            .unwrap();
        norito::encode_canonical(&plan.executions.values().next().unwrap().frontier)
            .unwrap()
            .len() as u64
    };
    assert_eq!(
        kura.post_wsv_lane_artifact_budget_reserved_bytes().unwrap(),
        2 * BOUND_PROGRESS_APPEND_INTENT_MAX_BYTES as u64 + low_frontier_bytes,
        "the lower frontier was superseded, so its component remains charged until exact terminal completion"
    );
    assert!(
        kura.post_wsv_lane_artifact_budget_reservations
            .lock()
            .values()
            .all(|reservation| reservation.outstanding_components.iter().all(
                |component| !matches!(component, PostWsvLaneArtifactStableComponentId::Receipt(_))
            ))
    );
    assert!(
        kura.post_wsv_lane_artifact_budget_reservations
            .lock()
            .values()
            .all(|reservation| reservation.prepend_transient_bytes == 0)
    );
}
