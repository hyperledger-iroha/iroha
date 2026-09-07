#[test]
fn certified_lane_predecessor_rejects_nonzero_height_without_descriptor_hash() {
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let (mut session, _) = sample_committed_lane_block_session_for_state_test(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
        Hash::new(b"missing-predecessor-hash-incarnation"),
        2,
        2,
    );
    session
        .proposal
        .descriptor
        .previous_lane_block_descriptor_hash = None;
    session.proposal.descriptor.descriptor_hash =
        session.proposal.descriptor.computed_descriptor_hash();
    session.proposal.proposal_hash = session.proposal.computed_proposal_hash();
    assert!(
        !state
            .certified_lane_block_predecessor_is_applied_or_snapshot_anchored(&session.proposal,)
            .expect("authenticate predecessor evidence"),
        "a nonzero predecessor height without its exact descriptor hash must fail closed",
    );
}
#[test]
fn autonomous_lane_predecessor_rejects_hash_only_absence_and_accepts_exact_wsv_frontier() {
    let lane_id = LaneId::SINGLE;
    let dataspace_id = DataSpaceId::UNIVERSAL;
    let incarnation = Hash::new(b"strict-autonomous-predecessor-incarnation");
    let (session, _) = sample_committed_lane_block_session_for_state_test(
        lane_id,
        dataspace_id,
        incarnation,
        2,
        2,
    );
    let descriptor = &session.proposal.descriptor;
    let previous_descriptor_hash = descriptor
        .previous_lane_block_descriptor_hash
        .expect("height-two fixture has a predecessor descriptor hash");
    let state = State::new_for_testing(
        World::default(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    assert!(
        !state
            .certified_autonomous_lane_block_predecessor_is_globally_applied(&session.proposal,)
            .expect("authenticate predecessor evidence"),
        "an autonomous successor cannot treat missing economic evidence as application",
    );
    let (key, payload) = State::encode_merge_lane_frontier_marker(AppliedMergeLaneFrontierMarker {
        version: 1,
        lane_id,
        dataspace_id,
        lane_incarnation: incarnation,
        lane_block_height: 1,
        lane_block_descriptor_hash: previous_descriptor_hash,
    })
    .expect("encode exact autonomous predecessor frontier");
    let mut world = World::default();
    world.smart_contract_state.insert(key, payload);
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    assert!(
        state
            .certified_autonomous_lane_block_predecessor_is_globally_applied(&session.proposal,)
            .expect("authenticate predecessor evidence"),
        "the exact replicated route/incarnation/height/hash frontier authorizes its successor",
    );
}
#[test]
fn autonomous_lane_predecessor_authenticates_receipt_and_preserves_occupied_corruption() {
    let (state, kura) = blank_test_state_with_kura();
    let lane_id = LaneId::SINGLE;
    let dataspace_id = DataSpaceId::UNIVERSAL;
    let incarnation = state
        .lane_incarnation(lane_id)
        .expect("the default lane has an active incarnation");
    let (block, predecessor, signer_pops) =
        lane_artifact_block_and_session_for_state_test(None, lane_id, dataspace_id, incarnation, 1);
    let finality =
        merge_carrier_finality_artifact_with_network(&block, None, *state.network_id_ref());
    kura.store_block(Arc::new(block))
        .expect("store canonical predecessor block");
    let _ = kura.store_v2_finality_artifact(&finality)
        .expect("publish exact signed complete-wire predecessor finality");
    kura.persist_committed_lane_block_session(&predecessor, &signer_pops)
        .expect("persist canonical predecessor certificate");
    kura.persist_lane_block_application_receipt(&predecessor.proposal)
        .expect("persist canonical predecessor receipt");
    let predecessor_artifact = kura
        .read_lane_block_artifact_read_only(lane_id, 1)
        .expect("authenticate raw predecessor ownership")
        .expect("occupied raw predecessor ownership");
    assert!(
        state
            .certified_lane_block_session_is_applied_or_snapshot_anchored(&predecessor)
            .expect("authenticate applied certified session")
    );
    assert!(
        state
            .lane_block_artifact_is_applied_or_snapshot_anchored(&predecessor_artifact)
            .expect("authenticate applied raw ownership")
    );

    let mut successor = predecessor.proposal.clone();
    successor.descriptor.proposal_height = successor
        .descriptor
        .proposal_height
        .checked_add(1)
        .expect("successor proposal height");
    successor.descriptor.previous_lane_block_height = 1;
    successor.descriptor.previous_lane_block_descriptor_hash =
        Some(predecessor.proposal.descriptor.descriptor_hash);
    successor.descriptor.lane_block_height = 2;
    successor.descriptor.descriptor_hash = successor.descriptor.computed_descriptor_hash();
    successor.proposal_hash = successor.computed_proposal_hash();

    assert!(
        kura.canonical_lane_block_predecessor_receipt_revalidates_without_sidecar_repair(
            &successor,
        ),
        "the exact Current receipt must revalidate against its canonical block results",
    );
    assert!(
        state
            .certified_autonomous_lane_block_predecessor_is_globally_applied(&successor)
            .expect("authenticate predecessor evidence"),
        "ordinary canonical application must authorize the next autonomous lane height even without a merge-frontier marker",
    );

    let mut competing = successor.clone();
    competing.descriptor.previous_lane_block_descriptor_hash =
        Some(Hash::new(b"other predecessor"));
    competing.descriptor.descriptor_hash = competing.descriptor.computed_descriptor_hash();
    competing.proposal_hash = competing.computed_proposal_hash();
    assert!(
        !state
            .certified_autonomous_lane_block_predecessor_is_globally_applied(&competing)
            .expect("a healthy different predecessor is ordinary rejection")
    );
    let lane_entry = state
        .nexus_snapshot()
        .lane_config
        .entry(lane_id)
        .expect("active predecessor lane")
        .clone();
    let receipt_dir = lane_entry
        .blocks_dir(kura.store_root())
        .join("lane_artifacts");
    let receipt_data = receipt_dir.join("application_receipts.norito");
    let receipt_index = receipt_dir.join("application_receipts.index");
    let saved_data = std::fs::read(&receipt_data).expect("read authenticated receipt fixture");
    let saved_index = std::fs::read(&receipt_index).expect("read authenticated receipt index");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&receipt_data)
        .expect("open receipt fault fixture")
        .set_len(1)
        .expect("truncate occupied receipt");
    let damaged = std::fs::read(&receipt_data).expect("read exact fault bytes");
    assert!(
        state
            .certified_lane_block_session_is_applied_or_snapshot_anchored(&predecessor)
            .is_err()
    );
    assert!(
        state
            .lane_block_artifact_is_applied_or_snapshot_anchored(&predecessor_artifact)
            .is_err()
    );
    assert!(
        state
            .certified_autonomous_lane_block_predecessor_is_globally_applied(&successor)
            .is_err()
    );
    assert!(
        state
            .certified_lane_block_predecessor_is_applied_or_snapshot_anchored(&successor)
            .is_err()
    );
    assert_eq!(std::fs::read(&receipt_data).unwrap(), damaged);
    assert_eq!(std::fs::read(&receipt_index).unwrap(), saved_index);
    // Restore only this deliberately corrupted test fixture for the independent
    // malformed replicated-frontier control below.
    std::fs::write(&receipt_data, saved_data).expect("restore test fixture receipt bytes");

    let malformed_key = State::merge_lane_frontier_marker_key(lane_id, dataspace_id, incarnation)
        .expect("derive the replicated merge-frontier key");
    let mut malformed_world = World::default();
    malformed_world
        .smart_contract_state
        .insert(malformed_key, b"malformed-frontier".to_vec());
    let malformed = State::new_for_testing(
        malformed_world,
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
    );
    assert!(
        malformed
            .certified_autonomous_lane_block_predecessor_is_globally_applied(&successor)
            .is_err(),
        "malformed replicated frontier evidence must fail closed even beside an exact local Current receipt",
    );
}
#[test]
fn autonomous_lane_predecessor_rejects_conflicting_or_malformed_wsv_frontier() {
    let lane_id = LaneId::SINGLE;
    let dataspace_id = DataSpaceId::UNIVERSAL;
    let incarnation = Hash::new(b"conflicting-autonomous-predecessor-incarnation");
    let (session, _) = sample_committed_lane_block_session_for_state_test(
        lane_id,
        dataspace_id,
        incarnation,
        2,
        2,
    );
    let descriptor = &session.proposal.descriptor;
    let (key, payload) = State::encode_merge_lane_frontier_marker(AppliedMergeLaneFrontierMarker {
        version: 1,
        lane_id,
        dataspace_id,
        lane_incarnation: incarnation,
        lane_block_height: 1,
        lane_block_descriptor_hash: Hash::new(b"wrong-autonomous-predecessor"),
    })
    .expect("encode conflicting autonomous predecessor frontier");
    let mut conflicting_world = World::default();
    conflicting_world
        .smart_contract_state
        .insert(key.clone(), payload);
    let conflicting = State::new_for_testing(
        conflicting_world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    assert!(
        !conflicting
            .certified_autonomous_lane_block_predecessor_is_globally_applied(&session.proposal,)
            .expect("authenticate predecessor evidence"),
        "a same-height conflicting descriptor must fail closed",
    );
    let mut malformed_world = World::default();
    malformed_world
        .smart_contract_state
        .insert(key, b"malformed-frontier".to_vec());
    let malformed = State::new_for_testing(
        malformed_world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    assert!(
        malformed
            .certified_autonomous_lane_block_predecessor_is_globally_applied(&session.proposal,)
            .is_err(),
        "malformed replicated evidence must fail closed",
    );
    assert_eq!(
        descriptor.previous_lane_block_height.checked_add(1),
        Some(descriptor.lane_block_height),
        "the negative controls must isolate evidence identity rather than continuity",
    );
}
state_test!(consensus_stack ready_bearing_certificate_never_enters_ordinary_receipt_repair
    ready_bearing_certificate_never_enters_ordinary_receipt_repair_on_consensus_stack();
);
fn ready_bearing_certificate_never_enters_ordinary_receipt_repair_on_consensus_stack() {
    let (ordinary, ordinary_pops) = sample_committed_lane_block_session_for_state_test(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
        Hash::new(b"ordinary-receipt-repair-role"),
        1,
        1,
    );
    let ordinary = crate::kura::CertifiedLaneBlockArtifact::new(ordinary, ordinary_pops);
    assert!(State::ordinary_application_receipt_repair_session(ordinary).is_some());
    let (_, entry, _, _) = autonomous_merge_commit_authorization_fixture(false, false);
    let execution = entry
        .execution_batch
        .as_ref()
        .and_then(|batch| batch.lanes.first())
        .expect("autonomous fixture carries one merge execution");
    let autonomous = Kura::decode_autonomous_lane_merge_bundle(
        &execution.source_bundle,
        execution.autonomous_network_id,
        execution.autonomous_epoch,
    )
    .expect("decode autonomous fixture bundle")
    .certified;
    assert!(autonomous.prepare_qc.payload_availability_qc.is_some());
    assert!(
        State::ordinary_application_receipt_repair_session(autonomous).is_none(),
        "READY-bearing autonomous execution must wait for its exact merge-carrier receipt",
    );
}
