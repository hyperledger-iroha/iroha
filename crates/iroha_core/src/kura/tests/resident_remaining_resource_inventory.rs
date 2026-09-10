// Live owner tests use existing authenticated Kura fixtures; independent recounts
// deliberately walk their test maps and are never called by production sampling.
fn prepare_remaining_resident_inventory(kura: &Kura) {
    {
        let _carrier = kura.merge_carrier_lock.lock();
        kura.ensure_merge_carrier_index_initialized_unlocked()
            .unwrap();
    }
    kura.reconcile_resident_resource_inventory().unwrap();
}
fn observed_remaining_resident(kura: &Kura, family: super::resource_inventory::Family) -> u64 {
    kura.resource_inventory
        .component_usage_for_tests(family)
        .unwrap()
        .resident_associations
}
fn recount_frontier_resident(kura: &Kura) -> u64 {
    let mut count = kura.lane_storage_entries.lock().len()
        + kura.certified_frontier_pair_durability.lock().len()
        + kura.certified_frontier_artifact_validation.lock().len();
    for reservation in kura
        .post_wsv_lane_artifact_budget_reservations
        .lock()
        .values()
    {
        count += 1
            + reservation.plan.stable_components.len()
            + reservation.plan.executions.len()
            + reservation.outstanding_components.len()
            + reservation.incomplete_terminal_outcomes.len();
    }
    for reservation in kura.certified_bundle_capacity_reservations.lock().values() {
        count += 1
            + reservation.plan.component_bytes.len()
            + reservation.plan.component_transient_bytes.len()
            + reservation.outstanding_components.len();
    }
    u64::try_from(count).unwrap()
}
fn recount_startup_inventory(inventory: &super::V2StartupFinalityVerificationInventory) -> u64 {
    let mut count = inventory.auxiliary_sidecars.len()
        + inventory.lane_auxiliary_directories.len()
        + inventory.hash_only_heights.len()
        + inventory.entries.len()
        + inventory.replay_sidecars.len()
        + usize::from(inventory.durable_tip_artifact.is_some())
        + usize::from(inventory.highest_verified_finality_artifact.is_some());
    for directory in inventory.auxiliary_sidecars.values() {
        count += directory.files.len();
    }
    for sidecars in &inventory.replay_sidecars {
        count +=
            usize::from(sidecars.checkpoint.is_some()) + usize::from(sidecars.manifest.is_some());
    }
    u64::try_from(count).unwrap()
}
// Resident reconciliation requires a complete transaction index, including
// authenticated execution results for every materialized height.
fn populate_completed_resident_store(directory: &TempDir) {
    let config = kura_config_for_dir(directory, BLOCKS_IN_MEMORY);
    let (kura, _) = test_kura_with_default_lane_markers(&config, &RuntimeLaneConfig::default());
    let mut blocks = DummyBlocks::new();
    for _ in 0..4 {
        kura.store_block(blocks.next_with_results()).unwrap();
    }
    let _ = persist_v2_finality_chain_through(&kura, nonzero!(4_usize));
}
#[test]
fn resident_verification_recounts_actual_replacement_inventory_until_last_replay_reader() {
    use super::resource_inventory::Family;
    let temp = TempDir::new().unwrap();
    populate_completed_resident_store(&temp);
    let config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    for height in 1..=4 {
        let block = kura.get_block(NonZeroUsize::new(height).unwrap()).unwrap();
        assert!(
            block.has_results(),
            "index the actual completed startup body"
        );
    }
    prepare_remaining_resident_inventory(&kura);
    let old = kura
        .v2_startup_finality_verification_inventory
        .lock()
        .as_ref()
        .unwrap()
        .clone();
    let old_count = recount_startup_inventory(&old);
    assert!(old_count > 0);
    let lru = || u64::try_from(kura.v2_finality_verification_cache.lock().len()).unwrap();
    assert_eq!(
        observed_remaining_resident(&kura, Family::ResidentVerification),
        old_count + lru()
    );
    kura.refresh_v2_startup_replay_auxiliary_binding()
        .expect_err("shared inventory cannot mutate");
    assert_eq!(
        observed_remaining_resident(&kura, Family::ResidentVerification),
        old_count + lru()
    );
    kura.refresh_v2_startup_finality_verification().unwrap();
    let current = kura
        .v2_startup_finality_verification_inventory
        .lock()
        .as_ref()
        .unwrap()
        .clone();
    let current_count = recount_startup_inventory(&current);
    assert_eq!(
        observed_remaining_resident(&kura, Family::ResidentVerification),
        old_count + current_count + lru()
    );
    kura.finish_v2_startup_finality_verification();
    assert_eq!(
        observed_remaining_resident(&kura, Family::ResidentVerification),
        old_count + current_count + lru()
    );
    drop(old);
    assert_eq!(
        observed_remaining_resident(&kura, Family::ResidentVerification),
        current_count + lru()
    );
    drop(current);
    assert_eq!(
        observed_remaining_resident(&kura, Family::ResidentVerification),
        lru()
    );
    kura.clear_v2_finality_verification_cache_for_test();
    assert_eq!(
        observed_remaining_resident(&kura, Family::ResidentVerification),
        0
    );
}
#[test]
fn resident_replica_recounts_authenticated_admission_duplicate_and_horizon_prune() {
    use super::resident_inventory::ResidentOwner;
    use super::resource_inventory::Family;
    let temp = TempDir::new().unwrap();
    populate_completed_resident_store(&temp);
    let config = kura_config_for_dir(&temp, NonZeroUsize::new(1).unwrap());
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&config, &RuntimeLaneConfig::default())
            .unwrap();
    let height = nonzero!(2_usize);
    finalize_chain_through_for_eviction(&kura, height);
    for height in 1..=4 {
        let block = kura.get_block(NonZeroUsize::new(height).unwrap()).unwrap();
        assert!(
            block.has_results(),
            "index the actual completed startup body"
        );
    }
    prepare_remaining_resident_inventory(&kura);
    let artifact = kura.v2_finality_artifact(2).unwrap().unwrap();
    let (keeper_index, keeper) = kura
        .deterministic_kura_replica_keepers(&artifact)
        .first()
        .unwrap()
        .clone();
    let keys = v2_finality_fixture_keys();
    let key = &keys[usize::try_from(keeper_index).unwrap()];
    kura.bind_local_peer_id(keeper).unwrap();
    let source = kura
        .probe_kura_replica_advert_source(2, key)
        .unwrap()
        .unwrap();
    let advert = kura
        .build_signed_kura_replica_advert_from_source(&source, key)
        .unwrap();
    kura.admit_kura_replica_advert(&advert).unwrap();
    assert_eq!(
        observed_remaining_resident(&kura, Family::ResidentReplica),
        2
    );
    kura.admit_kura_replica_advert(&advert).unwrap();
    assert_eq!(
        observed_remaining_resident(&kura, Family::ResidentReplica),
        2
    );
    {
        let owners = kura.replica_registry.lock();
        let independent = owners.len() + owners.values().map(BTreeMap::len).sum::<usize>();
        assert_eq!(
            owners.resident_associations().unwrap(),
            u64::try_from(independent).unwrap()
        );
    }
    {
        let mut owners = kura.replica_registry.lock();
        kura.prune_replica_adverts_for_horizon(&mut owners, Instant::now(), 3, 4);
    }
    assert_eq!(
        observed_remaining_resident(&kura, Family::ResidentReplica),
        0
    );
}
#[test]
fn resident_queue_counts_real_admission_cancel_drain_and_retry() {
    use super::resource_inventory::Family;
    let kura = Kura::blank_kura_for_testing();
    prepare_remaining_resident_inventory(&kura);
    kura.set_pipeline_sidecar_queue_cap_for_testing(2);
    let hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"resident queued"));
    let sidecar = |height| {
        PipelineRecoverySidecar::new(
            height,
            hash,
            PipelineDagSnapshot {
                fingerprint: [0; 32],
                key_count: 0,
            },
            Vec::new(),
        )
    };
    for height in 1..=2 {
        assert_eq!(
            kura.enqueue_pipeline_metadata(sidecar(height)),
            PipelineSidecarEnqueueResult::Enqueued {
                queue_depth: height as usize
            }
        );
    }
    assert_eq!(observed_remaining_resident(&kura, Family::ResidentQueue), 2);
    assert_eq!(
        kura.enqueue_pipeline_metadata(sidecar(3)),
        PipelineSidecarEnqueueResult::RejectedQueueFull { cap: 2 }
    );
    assert_eq!(observed_remaining_resident(&kura, Family::ResidentQueue), 2);
    let mut cancellation_checks = 0;
    assert_eq!(
        kura.enqueue_fastpq_proof_snapshot_unless(sample_fastpq_snapshot(99, hash, 8), || {
            cancellation_checks += 1;
            cancellation_checks == 2
        }),
        FastpqProofEnqueueResult::RejectedShutdown
    );
    assert_eq!(cancellation_checks, 2);
    assert_eq!(observed_remaining_resident(&kura, Family::ResidentQueue), 2);
    assert_eq!(kura.flush_pipeline_sidecars(), 2);
    assert_eq!(observed_remaining_resident(&kura, Family::ResidentQueue), 0);
    kura.fastpq_proof_sidecar_max_retries
        .store(2, Ordering::Relaxed);
    assert_eq!(
        kura.enqueue_fastpq_proof_snapshot(sample_fastpq_snapshot(99, hash, 8)),
        FastpqProofEnqueueResult::Enqueued { queue_depth: 1 }
    );
    assert_eq!(kura.flush_fastpq_proof_snapshots(), 0);
    assert_eq!(observed_remaining_resident(&kura, Family::ResidentQueue), 1);
    assert_eq!(kura.flush_fastpq_proof_snapshots(), 0);
    assert_eq!(observed_remaining_resident(&kura, Family::ResidentQueue), 0);
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut owner = kura.pipeline_sidecar_queue.lock();
        owner.push_back(sidecar(3));
        panic!("interrupt actual resident queue owner");
    }));
    assert!(unwind.is_err());
    assert_eq!(kura.pipeline_sidecar_queue.lock().len(), 1);
    assert!(kura.reconcile_resident_resource_inventory().is_err());
    assert!(
        kura.resource_inventory
            .component_usage_for_tests(Family::ResidentQueue)
            .is_err()
    );
}
#[test]
fn resident_frontier_counts_real_certified_failure_rebuild_and_terminal_consumption() {
    use super::resource_inventory::Family;
    let temp = TempDir::new().unwrap();
    let config = kura_config_for_dir(&temp, BLOCKS_IN_MEMORY);
    let lanes = two_lane_runtime_config();
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lanes).unwrap();
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lanes, LaneId::new(1));
    prepare_remaining_resident_inventory(&kura);
    assert_eq!(
        observed_remaining_resident(&kura, Family::ResidentFrontier),
        recount_frontier_resident(&kura)
    );
    fail_after_next_autonomous_certified_frontier_for_tests();
    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("actual frontier crash boundary");
    assert!(
        !kura
            .certified_bundle_capacity_reservations
            .lock()
            .is_empty()
    );
    assert_eq!(
        observed_remaining_resident(&kura, Family::ResidentFrontier),
        recount_frontier_resident(&kura)
    );
    let before = kura.certified_bundle_capacity_reservations.lock().clone();
    kura.rebuild_certified_bundle_capacity_reservations_on_startup()
        .unwrap();
    assert_eq!(*kura.certified_bundle_capacity_reservations.lock(), before);
    assert_eq!(
        observed_remaining_resident(&kura, Family::ResidentFrontier),
        recount_frontier_resident(&kura)
    );
    kura.repair_autonomous_lane_merge_bundles_on_startup()
        .unwrap();
    assert!(
        kura.certified_bundle_capacity_reservations
            .lock()
            .is_empty()
    );
    assert_eq!(
        observed_remaining_resident(&kura, Family::ResidentFrontier),
        recount_frontier_resident(&kura)
    );
    kura.rebuild_certified_bundle_capacity_reservations_on_startup()
        .unwrap();
    assert_eq!(
        observed_remaining_resident(&kura, Family::ResidentFrontier),
        recount_frontier_resident(&kura)
    );
}

#[test]
fn resident_post_wsv_recounts_real_partial_consumption_and_exact_retry() {
    let temp_dir = TempDir::new().expect("create merge reservation temp dir");
    let cfg = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let (kura, _) =
        Kura::open_test_kura_with_configured_lane_config(&cfg, &RuntimeLaneConfig::default())
            .expect("initialize reservation Kura");
    prepare_remaining_resident_inventory(&kura);
    let entrypoint = indexed_log_entrypoint([0x71; 32], [0x72; 32]);
    let mut entry = merge_entry_with_indexed_entrypoint(entrypoint);
    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let carrier = next_merge_carrier(&mut blocks, &mut entry);
    let descriptor = &entry
        .execution_batch
        .as_ref()
        .expect("one execution batch")
        .lanes
        .first()
        .expect("one active execution")
        .proposal
        .descriptor;
    publish_initial_configured_lane_geometry_for_test(
        &kura,
        &RuntimeLaneConfig::default(),
        &BTreeMap::from([(descriptor.lane_id, descriptor.lane_incarnation)]),
    );
    let expected = kura
        .merge_lane_application_artifact_required_bytes_for_block(carrier.as_ref(), Some(&entry))
        .expect("account committed carrier envelope");
    kura.store_block(parent).expect("store reservation parent");
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
        .expect("store reservation carrier");
    assert_eq!(
        observed_remaining_resident(&kura, super::resource_inventory::Family::ResidentFrontier),
        recount_frontier_resident(&kura)
    );
    assert_eq!(
        kura.post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read reservation total"),
        expected
    );
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
        .expect("exact carrier retry reconstructs the same reservation");
    assert_eq!(
        observed_remaining_resident(&kura, super::resource_inventory::Family::ResidentFrontier),
        recount_frontier_resident(&kura)
    );
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
        observed_remaining_resident(&kura, super::resource_inventory::Family::ResidentFrontier),
        recount_frontier_resident(&kura)
    );
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
        observed_remaining_resident(&kura, super::resource_inventory::Family::ResidentFrontier),
        recount_frontier_resident(&kura)
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
        observed_remaining_resident(&kura, super::resource_inventory::Family::ResidentFrontier),
        recount_frontier_resident(&kura)
    );
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
        observed_remaining_resident(&kura, super::resource_inventory::Family::ResidentFrontier),
        recount_frontier_resident(&kura)
    );
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
        observed_remaining_resident(&kura, super::resource_inventory::Family::ResidentFrontier),
        recount_frontier_resident(&kura)
    );
    assert_eq!(
        kura.post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("read reservation after stale reconstruction"),
        0,
        "a stale lazy carrier must not strand a reservation",
    );
}

#[test]
fn resident_failed_authenticated_rebuild_never_requalifies_partial_frontier() {
    let temp_dir = TempDir::new().expect("frontier build conflict temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let lane = lane_config
        .entry(lane_id)
        .expect("frontier build conflict lane");
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("frontier build conflict Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
    prepare_remaining_resident_inventory(&kura);
    fail_after_next_certified_frontier_build_for_tests();
    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("leave authenticated frontier build");
    let (frontier_path, build_path) =
        Kura::latest_certified_lane_block_frontier_paths_for_entry(lane, temp_dir.path());
    let mut conflict_artifact = prepared.source.bundle.certified.clone();
    conflict_artifact.signer_pops.clear();
    let conflict = LatestCertifiedLaneBlockFrontierV1::new(conflict_artifact)
        .expect("seal conflicting canonical frontier");
    fs::write(
        &frontier_path,
        norito::encode_canonical(&conflict).expect("encode conflicting frontier"),
    )
    .expect("stage conflicting durable frontier");
    let reservations_before = kura.certified_bundle_capacity_reservations.lock().clone();
    let tree_before = snapshot_regular_test_tree(temp_dir.path());
    kura.rebuild_certified_bundle_capacity_reservations_on_startup()
        .expect_err("conflicting frontier and build must fail closed");
    assert_eq!(snapshot_regular_test_tree(temp_dir.path()), tree_before);
    assert!(frontier_path.exists());
    assert!(build_path.exists());
    assert_eq!(
        *kura.certified_bundle_capacity_reservations.lock(),
        reservations_before
    );
    assert!(
        !kura
            .certified_resident_recovery_complete
            .load(Ordering::Acquire)
    );
    assert!(
        kura.resource_inventory
            .component_usage_for_tests(super::resource_inventory::Family::ResidentFrontier)
            .is_err()
    );
    assert!(kura.reconcile_resident_resource_inventory().is_err());
    assert!(kura.resource_inventory.try_snapshot().is_err());
}
