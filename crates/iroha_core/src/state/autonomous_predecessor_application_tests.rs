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
        !state.certified_lane_block_predecessor_is_applied_or_snapshot_anchored_cached(
            &session.proposal,
        ),
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
        !state.certified_autonomous_lane_block_predecessor_is_globally_applied_cached(
            &session.proposal,
        ),
        "an autonomous successor cannot treat missing economic evidence as application",
    );

    let (key, payload) = State::encode_merge_lane_frontier_marker(
        AppliedMergeLaneFrontierMarker {
            version: 1,
            lane_id,
            dataspace_id,
            lane_incarnation: incarnation,
            lane_block_height: 1,
            lane_block_descriptor_hash: previous_descriptor_hash,
        },
    )
    .expect("encode exact autonomous predecessor frontier");
    let mut world = World::default();
    world.smart_contract_state.insert(key, payload);
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    assert!(
        state.certified_autonomous_lane_block_predecessor_is_globally_applied_cached(
            &session.proposal,
        ),
        "the exact replicated route/incarnation/height/hash frontier authorizes its successor",
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

    let (key, payload) = State::encode_merge_lane_frontier_marker(
        AppliedMergeLaneFrontierMarker {
            version: 1,
            lane_id,
            dataspace_id,
            lane_incarnation: incarnation,
            lane_block_height: 1,
            lane_block_descriptor_hash: Hash::new(b"wrong-autonomous-predecessor"),
        },
    )
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
        !conflicting.certified_autonomous_lane_block_predecessor_is_globally_applied_cached(
            &session.proposal,
        ),
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
        !malformed.certified_autonomous_lane_block_predecessor_is_globally_applied_cached(
            &session.proposal,
        ),
        "malformed replicated evidence must fail closed",
    );
    assert_eq!(
        descriptor.previous_lane_block_height.checked_add(1),
        Some(descriptor.lane_block_height),
        "the negative controls must isolate evidence identity rather than continuity",
    );
}

#[test]
fn ready_bearing_certificate_never_enters_ordinary_receipt_repair() {
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
        execution.autonomous_chain_id_hash,
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
