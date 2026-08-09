struct PreparedAutonomousCertification {
    chain_id_hash: Hash,
    epoch: u64,
    session: crate::lane_consensus::CommittedLaneBlockSession,
    signer_pops: BTreeMap<PublicKey, Vec<u8>>,
    source: DurableAutonomousLaneMergeSource,
    plan: CertifiedBundleCapacityPlan,
}

fn prepare_autonomous_certification_for_capacity(
    kura: &Kura,
    lane_config: &RuntimeLaneConfig,
    lane_id: LaneId,
) -> PreparedAutonomousCertification {
    let lane = lane_config.entry(lane_id).expect("capacity lane");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_, _, payload) = autonomous_lane_payload_for_kura(lane_id, lane.dataspace_id, 1, &signer);
    prepare_autonomous_certification_for_capacity_payload(kura, lane_config, &payload, &signer)
}

fn prepare_autonomous_certification_for_capacity_payload(
    kura: &Kura,
    lane_config: &RuntimeLaneConfig,
    payload: &LaneExecutablePayloadV1,
    signer: &KeyPair,
) -> PreparedAutonomousCertification {
    let descriptor = &payload.origin_proposal.descriptor;
    let lane_id = descriptor.lane_id;
    let lane_block_height = descriptor.lane_block_height;
    let chain_id_hash = payload.chain_id_hash;
    let epoch = payload.epoch;
    install_autonomous_lane_marker_for_kura(kura, lane_config, payload);
    kura.persist_lane_executable_payload(payload, chain_id_hash, epoch)
        .expect("persist composite-capacity payload");
    let recovered = kura
        .recover_autonomous_lane_block_payload(&payload.origin_proposal, chain_id_hash, epoch)
        .expect("recover composite-capacity input");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist composite-capacity input");
    let availability =
        durable_lane_payload_availability_for_kura(payload, &payload.origin_proposal, signer);
    kura.persist_lane_payload_availability_certificate(
        lane_id,
        lane_block_height,
        availability.clone(),
        chain_id_hash,
        epoch,
    )
    .expect("persist composite-capacity READY evidence");
    let (mut session, signer_pops) =
        committed_lane_block_session_for_kura_proposal(&payload.origin_proposal, signer);
    session.prepare_qc = availability.certificate;
    let artifact = CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
    let source = {
        let _prune_guard = kura.prune_lock.lock();
        kura.durable_autonomous_lane_merge_source_under_prune_guard(
            lane_id,
            lane_block_height,
            chain_id_hash,
            epoch,
            Some(&artifact),
            false,
        )
        .expect("construct exact pre-certificate autonomous source")
    };
    let plan = {
        let _geometry_guard = kura.lane_geometry_lock.lock();
        let entry = kura
            .lane_storage_entry(lane_id)
            .expect("capacity plan lane");
        let _sidecar_guard = kura.sidecar_lock.lock();
        kura.certified_bundle_capacity_plan(&entry, &artifact, &source)
            .expect("construct exact composite byte plan")
    };
    PreparedAutonomousCertification {
        chain_id_hash,
        epoch,
        session,
        signer_pops,
        source,
        plan,
    }
}

fn autonomous_capacity_payload_at(
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_block_height: u64,
    proposal_height: u64,
    signer: &KeyPair,
) -> LaneExecutablePayloadV1 {
    let (_, _, payload) =
        autonomous_lane_payload_for_kura(lane_id, dataspace_id, lane_block_height, signer);
    repropose_autonomous_lane_payload_for_kura(&payload, proposal_height, signer)
}

struct PreparedCertifiedBundleReset {
    prepared: PreparedAutonomousCertification,
    authority: crate::state::CertifiedLaneBlockPersistenceAuthority,
    old_slot: CertifiedLaneBlockArtifact,
    old_tip: CertifiedLaneBlockArtifact,
}

fn prepare_certified_bundle_reset_fixture(
    kura: &Kura,
    lane_config: &RuntimeLaneConfig,
    lane_id: LaneId,
) -> PreparedCertifiedBundleReset {
    let lane = lane_config.entry(lane_id).expect("reset capacity lane");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let old_slot_payload =
        autonomous_capacity_payload_at(lane_id, lane.dataspace_id, 1, 90, &signer);
    let old_tip_payload =
        autonomous_capacity_payload_at(lane_id, lane.dataspace_id, 513, 100, &signer);
    let fresh_payload = autonomous_capacity_payload_at(lane_id, lane.dataspace_id, 1, 101, &signer);
    install_autonomous_lane_marker_for_kura(kura, lane_config, &fresh_payload);

    let (old_slot_session, old_slot_pops) =
        committed_lane_block_session_for_kura_proposal(&old_slot_payload.origin_proposal, &signer);
    let old_slot = CertifiedLaneBlockArtifact::new(old_slot_session.clone(), old_slot_pops.clone());
    kura.persist_committed_lane_block_session(&old_slot_session, &old_slot_pops)
        .expect("persist occupied pre-reset certified slot");

    let (old_tip_session, old_tip_pops) =
        committed_lane_block_session_for_kura_proposal(&old_tip_payload.origin_proposal, &signer);
    let old_tip = CertifiedLaneBlockArtifact::new(old_tip_session.clone(), old_tip_pops.clone());
    kura.persist_committed_lane_block_session(&old_tip_session, &old_tip_pops)
        .expect("persist high pre-reset certified frontier");

    let prepared = prepare_autonomous_certification_for_capacity_payload(
        kura,
        lane_config,
        &fresh_payload,
        &signer,
    );
    let descriptor = &fresh_payload.origin_proposal.descriptor;
    let authority = crate::state::CertifiedLaneBlockPersistenceAuthority::for_test(
        lane_id,
        descriptor.dataspace_id,
        descriptor.lane_incarnation,
        Some(100),
    );
    PreparedCertifiedBundleReset {
        prepared,
        authority,
        old_slot,
        old_tip,
    }
}

fn certified_bundle_reserved_for(
    plan: &CertifiedBundleCapacityPlan,
    components: impl IntoIterator<Item = CertifiedBundleCapacityComponent>,
) -> u64 {
    CertifiedBundleCapacityReservation {
        plan: plan.clone(),
        outstanding_components: components.into_iter().collect(),
    }
    .reserved_bytes()
    .expect("composite reservation byte count")
}

fn initial_certified_bundle_reserved(plan: &CertifiedBundleCapacityPlan) -> u64 {
    certified_bundle_reserved_for(plan, plan.component_bytes.keys().copied())
}

#[test]
fn certified_bundle_reservation_rejects_a_missing_outstanding_transient_entry() {
    let temp_dir = TempDir::new().expect("missing transient-entry temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let (kura, _) = Kura::new(&config, &lane_config).expect("missing transient-entry Kura");
    let mut plan = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id).plan;
    let component = plan
        .component_transient_bytes
        .keys()
        .next()
        .copied()
        .expect("capacity plan carries a transient component");
    assert!(plan.component_bytes.contains_key(&component));
    assert!(plan.component_transient_bytes.remove(&component).is_some());

    assert_eq!(
        CertifiedBundleCapacityReservation {
            plan,
            outstanding_components: BTreeSet::from([component]),
        }
        .reserved_bytes(),
        None,
    );
}

#[test]
fn certified_bundle_rejects_mismatched_authority_before_reserving_or_writing() {
    let temp_dir = TempDir::new().expect("mismatched authority temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let (kura, _) = Kura::new(&config, &lane_config).expect("mismatched authority Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
    let descriptor = &prepared.session.proposal.descriptor;
    let authority = crate::state::CertifiedLaneBlockPersistenceAuthority::for_test(
        LaneId::new(0),
        descriptor.dataspace_id,
        descriptor.lane_incarnation,
        None,
    );
    let tree_before = snapshot_regular_test_tree(temp_dir.path());

    kura.persist_committed_lane_block_session_with_authority(
        &prepared.session,
        &prepared.signer_pops,
        &authority,
    )
    .expect_err("mismatched authority must reject before composite admission");

    assert_eq!(snapshot_regular_test_tree(temp_dir.path()), tree_before);
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("mismatched authority reservation inventory"),
        0,
    );
}

#[test]
fn certified_bundle_authorized_active_slot_reset_publishes_exact_bundle() {
    let temp_dir = TempDir::new().expect("authorized reset temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let (kura, _) = Kura::new(&config, &lane_config).expect("authorized reset Kura");
    let fixture = prepare_certified_bundle_reset_fixture(&kura, &lane_config, lane_id);
    assert_eq!(
        kura.read_certified_lane_block_artifact(lane_id, 1),
        Some(fixture.old_slot.clone())
    );
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(fixture.old_tip.clone())
    );
    let expected = fixture.prepared.source.bundle.certified.clone();

    kura.persist_committed_lane_block_session_with_authority(
        &fixture.prepared.session,
        &fixture.prepared.signer_pops,
        &fixture.authority,
    )
    .expect("authorized reset replaces the active slot and high frontier");

    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("authorized reset reservation inventory"),
        0
    );
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(expected.clone())
    );
    assert_eq!(
        kura.read_certified_lane_block_artifact(lane_id, 1),
        Some(expected)
    );
    assert_eq!(
        kura.durable_autonomous_lane_merge_source(
            lane_id,
            1,
            fixture.prepared.chain_id_hash,
            fixture.prepared.epoch,
        )
        .expect("authorized reset publishes an exact durable bundle"),
        fixture.prepared.source
    );
}

#[test]
fn certified_bundle_active_slot_reset_without_authority_is_read_only() {
    let temp_dir = TempDir::new().expect("unauthorized reset temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let (kura, _) = Kura::new(&config, &lane_config).expect("unauthorized reset Kura");
    let fixture = prepare_certified_bundle_reset_fixture(&kura, &lane_config, lane_id);
    let tree_before = snapshot_regular_test_tree(temp_dir.path());
    let reservations_before = kura.certified_bundle_capacity_reservations.lock().clone();

    kura.persist_committed_lane_block_session(
        &fixture.prepared.session,
        &fixture.prepared.signer_pops,
    )
    .expect_err("same-incarnation reset without authority must fail before admission");

    assert_eq!(snapshot_regular_test_tree(temp_dir.path()), tree_before);
    assert_eq!(
        *kura.certified_bundle_capacity_reservations.lock(),
        reservations_before
    );
    assert_eq!(
        kura.read_certified_lane_block_artifact(lane_id, 1),
        Some(fixture.old_slot)
    );
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(fixture.old_tip)
    );
}

#[test]
fn certified_bundle_regressed_proposal_height_rejects_before_reserving() {
    let temp_dir = TempDir::new().expect("proposal regression temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let lane = lane_config
        .entry(lane_id)
        .expect("proposal regression lane");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let incoming_payload =
        autonomous_capacity_payload_at(lane_id, lane.dataspace_id, 2, 99, &signer);
    let (kura, _) = Kura::new(&config, &lane_config).expect("proposal regression Kura");
    let prepared = prepare_autonomous_certification_for_capacity_payload(
        &kura,
        &lane_config,
        &incoming_payload,
        &signer,
    );
    let existing_payload =
        autonomous_capacity_payload_at(lane_id, lane.dataspace_id, 1, 100, &signer);
    let (existing_session, existing_pops) =
        committed_lane_block_session_for_kura_proposal(&existing_payload.origin_proposal, &signer);
    let existing = CertifiedLaneBlockArtifact::new(existing_session.clone(), existing_pops.clone());
    kura.persist_committed_lane_block_session(&existing_session, &existing_pops)
        .expect("persist higher-proposal lower-lane-height frontier");
    let tree_before = snapshot_regular_test_tree(temp_dir.path());
    let reservations_before = kura.certified_bundle_capacity_reservations.lock().clone();
    let incoming = prepared.source.bundle.certified.clone();

    let error = {
        let _prune_guard = kura.prune_lock.lock();
        kura.ensure_certified_bundle_capacity_reservation_under_prune_guard(
            &incoming,
            &prepared.source,
            None,
        )
    };
    error.expect_err(
        "higher lane height with regressed proposal height must fail composite preflight",
    );

    assert_eq!(snapshot_regular_test_tree(temp_dir.path()), tree_before);
    assert_eq!(
        *kura.certified_bundle_capacity_reservations.lock(),
        reservations_before
    );
    assert_eq!(
        kura.latest_certified_lane_block_frontier(lane_id),
        Some(existing)
    );
}

fn exact_certified_bundle_limit(kura: &Kura, plan: &CertifiedBundleCapacityPlan) -> u64 {
    kura.refresh_disk_usage_bytes()
        .expect("refresh composite-capacity accounting");
    let used = kura
        .kura_disk_usage_bytes()
        .expect("measure composite physical bytes");
    let (persisted_count, unindexed_bytes) = kura
        .persisted_count_and_unindexed_bytes()
        .expect("measure composite pending-block cursor");
    let pending = kura
        .pending_block_bytes(persisted_count, unindexed_bytes)
        .expect("measure pending canonical blocks");
    let terminal = kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("measure terminal reservations");
    let post_wsv = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure post-WSV reservations");
    let existing = kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure existing composite reservations");
    used.checked_add(pending)
        .and_then(|bytes| bytes.checked_add(terminal))
        .and_then(|bytes| bytes.checked_add(post_wsv))
        .and_then(|bytes| bytes.checked_add(existing))
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .and_then(|bytes| bytes.checked_add(initial_certified_bundle_reserved(plan)))
        .expect("exact composite capacity fits")
}

#[test]
fn certified_bundle_composite_exact_limit_is_atomic_and_retry_leaks_nothing() {
    let temp_dir = TempDir::new().expect("composite capacity temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let (mut kura, _) = Kura::new(&config, &lane_config).expect("composite capacity Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
    let exact_limit = exact_certified_bundle_limit(&kura, &prepared.plan);
    let tree_before = snapshot_regular_test_tree(temp_dir.path());
    let accounting_before = kura.disk_usage.load(Ordering::Relaxed);
    let reservations_before = kura.certified_bundle_capacity_reservations.lock().clone();
    Arc::get_mut(&mut kura)
        .expect("exclusive composite capacity Kura")
        .max_disk_usage_bytes = exact_limit - 1;

    let error = kura
        .persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("one byte below the complete composite peak must reject");
    assert!(matches!(
        error,
        Error::StorageBudgetExceeded { limit, required, .. }
            if limit == exact_limit - 1 && required == exact_limit
    ));
    assert_eq!(snapshot_regular_test_tree(temp_dir.path()), tree_before);
    assert_eq!(kura.disk_usage.load(Ordering::Relaxed), accounting_before);
    assert_eq!(
        *kura.certified_bundle_capacity_reservations.lock(),
        reservations_before
    );

    Arc::get_mut(&mut kura)
        .expect("exclusive composite capacity Kura remains")
        .max_disk_usage_bytes = exact_limit;
    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect("the exact complete composite peak succeeds");
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("completed composite reservations"),
        0
    );
    let published = kura
        .durable_autonomous_lane_merge_source(lane_id, 1, prepared.chain_id_hash, prepared.epoch)
        .expect("exact bundle is merge eligible");
    assert_eq!(published, prepared.source);
    let completed_tree = snapshot_regular_test_tree(temp_dir.path());
    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect("exact completed retry stutters");
    assert_eq!(snapshot_regular_test_tree(temp_dir.path()), completed_tree);
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("retry composite reservations"),
        0
    );
}

#[test]
fn certified_bundle_frontier_crash_rebuilds_exact_remaining_obligation() {
    let temp_dir = TempDir::new().expect("frontier crash temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let (kura, _) = Kura::new(&config, &lane_config).expect("frontier crash Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
    fail_after_next_autonomous_certified_frontier_for_tests();
    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("inject crash after strict frontier publication");
    let expected = certified_bundle_reserved_for(
        &prepared.plan,
        [
            CertifiedBundleCapacityComponent::CertifiedPair,
            CertifiedBundleCapacityComponent::AutonomousBundlePair,
        ],
    );
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("frontier-crash reservation"),
        expected
    );
    assert!(
        kura.read_certified_lane_block_artifact(lane_id, 1)
            .is_none()
    );
    let reservation_before = kura.certified_bundle_capacity_reservations.lock().clone();
    kura.rebuild_certified_bundle_capacity_reservations_on_startup()
        .expect("rebuild frontier-crash obligation");
    assert_eq!(
        *kura.certified_bundle_capacity_reservations.lock(),
        reservation_before
    );
    kura.repair_autonomous_lane_merge_bundles_on_startup()
        .expect("repair certificate and bundle from frontier");
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("frontier repair reservations"),
        0
    );
}

#[test]
fn certified_frontier_build_only_restart_promotes_then_rebuilds_remaining_obligation() {
    let temp_dir = TempDir::new().expect("frontier build-only temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let lane = lane_config
        .entry(lane_id)
        .expect("frontier build-only lane");
    let (kura, _) = Kura::new(&config, &lane_config).expect("frontier build-only Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
    fail_after_next_certified_frontier_build_for_tests();
    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("inject crash after frontier build fsync");
    let (frontier_path, build_path) =
        Kura::latest_certified_lane_block_frontier_paths_for_entry(lane, temp_dir.path());
    assert!(!frontier_path.exists());
    assert!(build_path.exists());

    kura.rebuild_certified_bundle_capacity_reservations_on_startup()
        .expect("authenticate and promote exact frontier build");
    assert!(frontier_path.exists());
    assert!(!build_path.exists());
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("promoted-build reservation"),
        certified_bundle_reserved_for(
            &prepared.plan,
            [
                CertifiedBundleCapacityComponent::CertifiedPair,
                CertifiedBundleCapacityComponent::AutonomousBundlePair,
            ],
        )
    );
    kura.repair_autonomous_lane_merge_bundles_on_startup()
        .expect("finish promoted frontier publication");
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("promoted-build repair reservations"),
        0
    );
}

#[test]
fn certified_frontier_build_conflict_fails_before_rebuild_map_publication() {
    let temp_dir = TempDir::new().expect("frontier build conflict temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let lane = lane_config
        .entry(lane_id)
        .expect("frontier build conflict lane");
    let (kura, _) = Kura::new(&config, &lane_config).expect("frontier build conflict Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
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
}

#[test]
fn certified_pair_crash_rebuilds_only_bundle_obligation() {
    let temp_dir = TempDir::new().expect("bundle crash temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let (kura, _) = Kura::new(&config, &lane_config).expect("bundle crash Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
    fail_next_autonomous_merge_bundle_persistence_for_tests();
    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("inject crash after strict certified pair");
    let expected = certified_bundle_reserved_for(
        &prepared.plan,
        [CertifiedBundleCapacityComponent::AutonomousBundlePair],
    );
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("bundle-crash reservation"),
        expected
    );
    let reservation_before = kura.certified_bundle_capacity_reservations.lock().clone();
    kura.rebuild_certified_bundle_capacity_reservations_on_startup()
        .expect("rebuild bundle-only obligation");
    assert_eq!(
        *kura.certified_bundle_capacity_reservations.lock(),
        reservation_before
    );
    kura.repair_autonomous_lane_merge_bundles_on_startup()
        .expect("repair missing autonomous bundle");
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("bundle repair reservations"),
        0
    );
}

#[test]
fn durable_bundle_pair_crash_rebuild_consumes_obligation_from_exact_readback() {
    let temp_dir = TempDir::new().expect("durable bundle-pair crash temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let (kura, _) = Kura::new(&config, &lane_config).expect("durable bundle-pair Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
    fail_after_next_autonomous_merge_bundle_pair_for_tests();
    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("inject crash after durable bundle pair and before reservation consumption");
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("pre-restart durable bundle reservation"),
        certified_bundle_reserved_for(
            &prepared.plan,
            [CertifiedBundleCapacityComponent::AutonomousBundlePair],
        )
    );

    kura.rebuild_certified_bundle_capacity_reservations_on_startup()
        .expect("strict bundle readback consumes rebuilt obligation");
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("rebuilt durable bundle reservation"),
        0
    );
    assert_eq!(
        kura.durable_autonomous_lane_merge_source(
            lane_id,
            1,
            prepared.chain_id_hash,
            prepared.epoch,
        )
        .expect("durable bundle remains exact after rebuild"),
        prepared.source
    );
}

#[test]
fn bundle_pair_append_intent_rebuilds_then_repairs_exact_obligation() {
    let temp_dir = TempDir::new().expect("bundle append-intent temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let lane = lane_config
        .entry(lane_id)
        .expect("bundle append-intent lane");
    let (mut kura, _) = Kura::new(&config, &lane_config).expect("bundle append-intent Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
    let exact_limit = exact_certified_bundle_limit(&kura, &prepared.plan);
    Arc::get_mut(&mut kura)
        .expect("exclusive bundle append-intent Kura")
        .max_disk_usage_bytes = exact_limit;
    fail_next_autonomous_merge_bundle_append_data_sync_for_tests();
    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("inject bundle-pair append-intent crash");
    let (_, bundle_index_path) =
        Kura::autonomous_lane_merge_bundle_paths_for_entry(lane, temp_dir.path());
    let intent_path = Kura::bound_progress_append_intent_path(&bundle_index_path);
    assert!(intent_path.exists());
    let expected = certified_bundle_reserved_for(
        &prepared.plan,
        [CertifiedBundleCapacityComponent::AutonomousBundlePair],
    );
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("bundle append-intent reservation"),
        expected
    );
    kura.rebuild_certified_bundle_capacity_reservations_on_startup()
        .expect("rebuild exact bundle append-intent obligation");
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("rebuilt bundle append-intent reservation"),
        expected
    );
    assert!(
        intent_path.exists(),
        "rebuild is read-only for pair intents"
    );
    kura.repair_autonomous_lane_merge_bundles_on_startup()
        .expect("recover exact bundle append intent");
    assert!(!intent_path.exists());
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("repaired bundle append-intent reservation"),
        0
    );
    assert_eq!(
        kura.durable_autonomous_lane_merge_source(
            lane_id,
            1,
            prepared.chain_id_hash,
            prepared.epoch,
        )
        .expect("repaired bundle source is exact"),
        prepared.source
    );
}

#[test]
fn certified_pair_append_intent_rebuilds_and_repairs_at_original_exact_limit() {
    let temp_dir = TempDir::new().expect("certified append-intent temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let lane = lane_config
        .entry(lane_id)
        .expect("certified append-intent lane");
    let (mut kura, _) = Kura::new(&config, &lane_config).expect("certified append-intent Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
    let exact_limit = exact_certified_bundle_limit(&kura, &prepared.plan);
    Arc::get_mut(&mut kura)
        .expect("exclusive certified append-intent Kura")
        .max_disk_usage_bytes = exact_limit;
    fail_next_bound_progress_append_data_sync_for_tests();
    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("inject certified-pair append-intent crash");
    let (_, certified_index_path) =
        Kura::certified_lane_block_paths_for_entry(lane, temp_dir.path());
    let intent_path = Kura::bound_progress_append_intent_path(&certified_index_path);
    assert!(intent_path.exists());

    kura.rebuild_certified_bundle_capacity_reservations_on_startup()
        .expect("original exact limit admits certified append recovery");
    assert!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("certified append recovery reservation")
            > 0
    );
    kura.repair_autonomous_lane_merge_bundles_on_startup()
        .expect("repair certified append and downstream bundle at exact limit");
    assert!(!intent_path.exists());
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("certified append repair reservation"),
        0
    );
    assert_eq!(
        kura.durable_autonomous_lane_merge_source(
            lane_id,
            1,
            prepared.chain_id_hash,
            prepared.epoch,
        )
        .expect("certified append repair source is exact"),
        prepared.source
    );
}

fn assert_authenticated_append_build_restart_at_exact_limit(
    calls_before_failure: usize,
    bundle_role: bool,
) {
    let temp_dir = TempDir::new().expect("authenticated append-build temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let lane = lane_config
        .entry(lane_id)
        .expect("authenticated build lane");
    let (mut kura, _) = Kura::new(&config, &lane_config).expect("authenticated build Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
    let exact_limit = exact_certified_bundle_limit(&kura, &prepared.plan);
    Arc::get_mut(&mut kura)
        .expect("exclusive authenticated build Kura")
        .max_disk_usage_bytes = exact_limit;
    fail_after_bound_progress_append_build_for_tests(calls_before_failure);
    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("inject crash after authenticated append-build fsync");

    let (_, index_path) = if bundle_role {
        Kura::autonomous_lane_merge_bundle_paths_for_entry(lane, temp_dir.path())
    } else {
        Kura::certified_lane_block_paths_for_entry(lane, temp_dir.path())
    };
    let build_path = Kura::bound_progress_append_build_path(&index_path);
    let intent_path = Kura::bound_progress_append_intent_path(&index_path);
    assert!(build_path.exists());
    assert!(!intent_path.exists());
    kura.refresh_disk_usage_bytes()
        .expect("refresh authenticated build physical usage");

    kura.rebuild_certified_bundle_capacity_reservations_on_startup()
        .expect("authenticated append build is admitted at its original exact limit");
    assert!(
        build_path.exists(),
        "capacity rebuild must remain read-only"
    );
    let expected = if bundle_role {
        certified_bundle_reserved_for(
            &prepared.plan,
            [CertifiedBundleCapacityComponent::AutonomousBundlePair],
        )
    } else {
        certified_bundle_reserved_for(
            &prepared.plan,
            [
                CertifiedBundleCapacityComponent::CertifiedPair,
                CertifiedBundleCapacityComponent::AutonomousBundlePair,
            ],
        )
    };
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("authenticated build restart reservation"),
        expected
    );
    kura.repair_autonomous_lane_merge_bundles_on_startup()
        .expect("discard authenticated build and retry exact publication");
    assert!(!build_path.exists());
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("authenticated build repair reservation"),
        0
    );
    assert_eq!(
        kura.durable_autonomous_lane_merge_source(
            lane_id,
            1,
            prepared.chain_id_hash,
            prepared.epoch,
        )
        .expect("authenticated build repair source is exact"),
        prepared.source
    );
}

#[test]
fn certified_and_bundle_authenticated_append_builds_restart_at_original_exact_limit() {
    assert_authenticated_append_build_restart_at_exact_limit(0, false);
    assert_authenticated_append_build_restart_at_exact_limit(1, true);
}

fn assert_append_recovery_restart_rejects_one_under(build_only: bool) {
    let temp_dir = TempDir::new().expect("one-under append recovery temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let lane = lane_config.entry(lane_id).expect("one-under recovery lane");
    let (mut kura, _) = Kura::new(&config, &lane_config).expect("one-under recovery Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
    let exact_limit = exact_certified_bundle_limit(&kura, &prepared.plan);
    Arc::get_mut(&mut kura)
        .expect("exclusive one-under recovery Kura")
        .max_disk_usage_bytes = exact_limit;
    if build_only {
        fail_after_bound_progress_append_build_for_tests(1);
    } else {
        fail_next_bound_progress_append_data_sync_for_tests();
    }
    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("inject authenticated append recovery crash");

    let (_, index_path) = if build_only {
        Kura::autonomous_lane_merge_bundle_paths_for_entry(lane, temp_dir.path())
    } else {
        Kura::certified_lane_block_paths_for_entry(lane, temp_dir.path())
    };
    let recovery_path = if build_only {
        Kura::bound_progress_append_build_path(&index_path)
    } else {
        Kura::bound_progress_append_intent_path(&index_path)
    };
    assert!(recovery_path.exists());
    kura.refresh_disk_usage_bytes()
        .expect("refresh one-under recovery physical usage");
    Arc::get_mut(&mut kura)
        .expect("exclusive one-under recovery Kura remains")
        .max_disk_usage_bytes = exact_limit - 1;
    let tree_before = snapshot_regular_test_tree(temp_dir.path());
    let reservations_before = kura.certified_bundle_capacity_reservations.lock().clone();

    let error = kura
        .rebuild_certified_bundle_capacity_reservations_on_startup()
        .expect_err("one byte below the authenticated recovery envelope must reject");
    assert!(matches!(
        error,
        Error::StorageBudgetExceeded { limit, required, .. }
            if limit == exact_limit - 1 && required == exact_limit
    ));
    assert_eq!(snapshot_regular_test_tree(temp_dir.path()), tree_before);
    assert_eq!(
        *kura.certified_bundle_capacity_reservations.lock(),
        reservations_before
    );
}

#[test]
fn append_intent_and_build_restart_preflight_reject_one_under_without_mutation() {
    assert_append_recovery_restart_rejects_one_under(false);
    assert_append_recovery_restart_rejects_one_under(true);
}

#[test]
fn lane_retirement_is_blocked_by_outstanding_certified_bundle_reservation() {
    let temp_dir = TempDir::new().expect("retirement blocker temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let lane = lane_config.entry(lane_id).expect("retirement blocker lane");
    let (kura, _) = Kura::new(&config, &lane_config).expect("retirement blocker Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
    fail_after_next_autonomous_certified_frontier_for_tests();
    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("leave an outstanding composite reservation");
    assert!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("retirement blocker reservation")
            > 0
    );
    kura.preflight_retire_lane_storage(lane)
        .expect_err("lane retirement must not outrun composite publication capacity");
}

#[test]
fn certified_bundle_preflight_rejects_lone_append_build_without_mutation() {
    let temp_dir = TempDir::new().expect("lone append-build temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let (kura, _) = Kura::new(&config, &lane_config).expect("lone append-build Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
    let lane = lane_config.entry(lane_id).expect("lone-build lane");
    let (_, index_path) = Kura::certified_lane_block_paths_for_entry(lane, temp_dir.path());
    let build_path = Kura::bound_progress_append_build_path(&index_path);
    fs::write(&build_path, b"forged lone append build").expect("stage lone append build");
    let tree_before = snapshot_regular_test_tree(temp_dir.path());
    let reservations_before = kura.certified_bundle_capacity_reservations.lock().clone();

    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("a lone unauthenticated append build must fail closed");
    assert_eq!(snapshot_regular_test_tree(temp_dir.path()), tree_before);
    assert_eq!(
        *kura.certified_bundle_capacity_reservations.lock(),
        reservations_before
    );
}

#[test]
fn certified_bundle_preflight_rejects_authenticated_mismatched_append_build() {
    let temp_dir = TempDir::new().expect("mismatched append-build temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let lane = lane_config
        .entry(lane_id)
        .expect("mismatched append-build lane");
    let (kura, _) = Kura::new(&config, &lane_config).expect("mismatched append-build Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
    fail_after_bound_progress_append_build_for_tests(0);
    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("leave exact certified append build");
    let (_, index_path) = Kura::certified_lane_block_paths_for_entry(lane, temp_dir.path());
    let build_path = Kura::bound_progress_append_build_path(&index_path);
    let mut intent = norito::decode_canonical::<BoundProgressAppendIntentV1>(
        &fs::read(&build_path).expect("read exact append build"),
    )
    .expect("decode exact append build");
    intent.payload_hash = Hash::new(b"another certified payload");
    fs::write(
        &build_path,
        norito::encode_canonical(&intent.seal()).expect("encode mismatched sealed append build"),
    )
    .expect("replace build with mismatched sealed intent");
    let tree_before = snapshot_regular_test_tree(temp_dir.path());
    let reservations_before = kura.certified_bundle_capacity_reservations.lock().clone();

    kura.rebuild_certified_bundle_capacity_reservations_on_startup()
        .expect_err("sealed append build for another payload must fail closed");
    assert_eq!(snapshot_regular_test_tree(temp_dir.path()), tree_before);
    assert_eq!(
        *kura.certified_bundle_capacity_reservations.lock(),
        reservations_before
    );
}

#[test]
fn certified_bundle_preflight_checks_bad_older_history_beneath_exact_append_intent() {
    let temp_dir = TempDir::new().expect("append preimage conflict temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let lane = lane_config.entry(lane_id).expect("append preimage lane");
    let (first, first_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane.dataspace_id, 1);
    let (second, second_pops) =
        sample_committed_lane_block_session_for_kura(lane_id, lane.dataspace_id, 2);
    let (kura, _) = test_kura_with_default_lane_markers(&config, &lane_config);
    kura.persist_committed_lane_block_session(&first, &first_pops)
        .expect("persist stable older certified history");
    fail_next_bound_progress_append_data_sync_for_tests();
    kura.persist_committed_lane_block_session(&second, &second_pops)
        .expect_err("leave exact current certified append intent");
    let (data_path, index_path) = Kura::certified_lane_block_paths_for_entry(lane, temp_dir.path());
    let intent_path = Kura::bound_progress_append_intent_path(&index_path);
    assert!(intent_path.exists());
    let mut data = fs::read(&data_path).expect("read certified pair with append suffix");
    data[0] ^= 1;
    fs::write(&data_path, data).expect("corrupt only older stable certified payload");
    let tree_before = snapshot_regular_test_tree(temp_dir.path());
    let reservations_before = kura.certified_bundle_capacity_reservations.lock().clone();

    kura.rebuild_certified_bundle_capacity_reservations_on_startup()
        .expect_err("bad older history beneath an exact intent must fail before repair");
    assert_eq!(snapshot_regular_test_tree(temp_dir.path()), tree_before);
    assert!(intent_path.exists());
    assert_eq!(
        *kura.certified_bundle_capacity_reservations.lock(),
        reservations_before
    );
}

#[test]
fn certified_bundle_stale_incarnation_reservation_blocks_aba_without_mutation() {
    let temp_dir = TempDir::new().expect("ABA reservation temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_id = LaneId::new(1);
    let (kura, _) = Kura::new(&config, &lane_config).expect("ABA reservation Kura");
    let prepared = prepare_autonomous_certification_for_capacity(&kura, &lane_config, lane_id);
    let mut stale_plan = prepared.plan.clone();
    stale_plan.identity.lane_incarnation = Hash::new(b"retired composite incarnation");
    let stale_identity = stale_plan.identity;
    kura.certified_bundle_capacity_reservations.lock().insert(
        stale_identity,
        CertifiedBundleCapacityReservation {
            outstanding_components: stale_plan.component_bytes.keys().copied().collect(),
            plan: stale_plan,
        },
    );
    let reservations_before = kura.certified_bundle_capacity_reservations.lock().clone();
    let tree_before = snapshot_regular_test_tree(temp_dir.path());

    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("a stale same-route reservation must block lane-ID ABA");
    assert_eq!(snapshot_regular_test_tree(temp_dir.path()), tree_before);
    assert_eq!(
        *kura.certified_bundle_capacity_reservations.lock(),
        reservations_before
    );
}

#[test]
fn certified_bundle_startup_rebuild_publishes_nothing_on_late_route_error() {
    let temp_dir = TempDir::new().expect("late-route rebuild temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane0 = ModelLaneConfig::default();
    let lane1 = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "beta".to_owned(),
        ..ModelLaneConfig::default()
    };
    let lane2 = ModelLaneConfig {
        id: LaneId::new(2),
        alias: "gamma".to_owned(),
        ..ModelLaneConfig::default()
    };
    let catalog =
        LaneCatalog::new(nonzero!(3_u32), vec![lane0, lane1, lane2]).expect("three-lane catalog");
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    let (kura, _) = Kura::new(&config, &lane_config).expect("late-route rebuild Kura");
    let prepared =
        prepare_autonomous_certification_for_capacity(&kura, &lane_config, LaneId::new(1));
    fail_after_next_autonomous_certified_frontier_for_tests();
    kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
        .expect_err("leave a valid first-route frontier obligation");
    let reservations_before = kura.certified_bundle_capacity_reservations.lock().clone();
    assert_eq!(reservations_before.len(), 1);
    let lane2 = lane_config
        .entry(LaneId::new(2))
        .expect("late failing lane");
    let (late_frontier_path, _) =
        Kura::latest_certified_lane_block_frontier_paths_for_entry(lane2, temp_dir.path());
    fs::write(&late_frontier_path, b"malformed late-route frontier")
        .expect("stage malformed late-route frontier");

    kura.rebuild_certified_bundle_capacity_reservations_on_startup()
        .expect_err("late-route conflict must fail the whole rebuild");
    assert_eq!(
        *kura.certified_bundle_capacity_reservations.lock(),
        reservations_before,
        "a late rebuild error must not publish a partial replacement map"
    );
}
