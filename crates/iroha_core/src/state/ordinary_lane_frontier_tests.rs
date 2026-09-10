// Tests for ordinary execution sharing the canonical applied lane frontier.

fn rewrite_frontier_ownership_for_test(
    block: &SignedBlock,
    mutate: impl FnOnce(&mut iroha_data_model::block::consensus::SumeragiLanePayloadOwnership),
) -> SignedBlock {
    let mut block = block.clone();
    let mut context = block.execution_context().expect("fixture context").clone();
    let ownership = context
        .lane_payload_ownerships
        .first_mut()
        .expect("fixture ownership");
    mutate(ownership);
    let hashes = ownership
        .compute_replay_hashes()
        .expect("canonical ownership");
    ownership.subject_hash = hashes.subject_hash;
    ownership.payload_ownership_hash = hashes.payload_ownership_hash;
    ownership.rbc_instance_hash = hashes.rbc_instance_hash;
    ownership.lane_block_descriptor_hash = Some(hashes.lane_block_descriptor_hash);
    block.set_execution_context(Some(context));
    block
}

state_test! { sync ordinary_lane_frontier_publishes_once_and_rejects_invalid_successors_atomically
    let state = blank_test_state();
    let lane = LaneId::SINGLE;
    let dataspace = DataSpaceId::UNIVERSAL;
    let incarnation = Hash::new(b"ordinary-frontier-incarnation");
    let (first, _, _) = lane_artifact_block_and_session_for_state_test(
        None, lane, dataspace, incarnation, 1,
    );
    let mut overlay = state.block(first.header());
    assert!(overlay.verify_ordinary_lane_frontiers(&first).is_err(),
        "apply cannot invent an ownership frontier omitted by execution");
    overlay.stage_ordinary_lane_frontiers(&first).expect("first applied ordinary lane slot");
    overlay.verify_ordinary_lane_frontiers(&first).expect("exact executed frontier");
    let first_hash = first.execution_context().unwrap().lane_payload_ownerships[0]
        .lane_block_descriptor_hash.unwrap();
    let expected = (1, Some(first_hash));
    assert_eq!(State::canonical_merged_lane_frontier_from_world(
        &overlay.world, lane, dataspace, incarnation,
    ).unwrap(), expected);
    let (second, _, _) = lane_artifact_block_and_session_for_state_test(
        Some(&first), lane, dataspace, incarnation, 2,
    );
    let second = rewrite_frontier_ownership_for_test(&second, |ownership| {
        ownership.previous_lane_block_descriptor_hash = Some(first_hash);
    });
    let wrong_parent = rewrite_frontier_ownership_for_test(&second, |ownership| {
        ownership.previous_lane_block_descriptor_hash = Some(Hash::new(b"wrong predecessor"));
    });
    let skipped_slot = rewrite_frontier_ownership_for_test(&second, |ownership| {
        ownership.lane_block_height = 3;
        ownership.previous_lane_block_height = 2;
    });
    let mut duplicate = second.clone();
    let mut context = duplicate.execution_context().unwrap().clone();
    context.lane_payload_ownerships.push(context.lane_payload_ownerships[0].clone());
    duplicate.set_execution_context(Some(context));
    let mut wrong_round = second.clone();
    let mut context = wrong_round.execution_context().unwrap().clone();
    context.lane_payload_ownerships[0].proposal_view += 1;
    wrong_round.set_execution_context(Some(context));
    for invalid in [&first, &wrong_parent, &skipped_slot, &duplicate, &wrong_round] {
        assert!(overlay.stage_ordinary_lane_frontiers(invalid).is_err());
        assert_eq!(State::canonical_merged_lane_frontier_from_world(
            &overlay.world, lane, dataspace, incarnation,
        ).unwrap(), expected, "invalid transition must leave every frontier unchanged");
    }
    overlay.stage_ordinary_lane_frontiers(&second).expect("exact ordinary successor");
    overlay.verify_ordinary_lane_frontiers(&second).expect("successor ready for apply");
    assert!(overlay.verify_ordinary_lane_frontiers(&first).is_err(),
        "a different applied slot is not proof of this carrier's execution");
    assert_eq!(State::canonical_merged_lane_frontier_from_world(
        &overlay.world, lane, dataspace, incarnation,
    ).unwrap().0, 2);
    drop(overlay);
    assert_eq!(State::canonical_merged_lane_frontier_from_world(
        &state.view().world, lane, dataspace, incarnation,
    ).unwrap(), (0, None), "uncommitted execution must not publish a frontier");
}

state_test! { sync ordinary_lane_frontier_extends_autonomous_application_and_unblocks_next_merge
    let state = blank_test_state();
    let lane = LaneId::SINGLE;
    let dataspace = DataSpaceId::UNIVERSAL;
    let incarnation = Hash::new(b"alternating-frontier-incarnation");
    let (ordinary, _, _) = lane_artifact_block_and_session_for_state_test(
        None, lane, dataspace, incarnation, 2,
    );
    let mut overlay = state.block(ordinary.header());
    let autonomous_hash = ordinary.execution_context().unwrap().lane_payload_ownerships[0]
        .previous_lane_block_descriptor_hash.unwrap();
    overlay.stage_merge_lane_frontier_markers(vec![State::encode_merge_lane_frontier_marker(
        AppliedMergeLaneFrontierMarker {
            version: 1, lane_id: lane, dataspace_id: dataspace, lane_incarnation: incarnation,
            lane_block_height: 1, lane_block_descriptor_hash: autonomous_hash,
        },
    ).unwrap()]).expect("previous autonomous application frontier");
    overlay.stage_ordinary_lane_frontiers(&ordinary).expect("ordinary successor shares frontier");
    overlay.verify_ordinary_lane_frontiers(&ordinary).expect("ordinary application frontier");
    let ordinary_hash = ordinary.execution_context().unwrap().lane_payload_ownerships[0]
        .lane_block_descriptor_hash.unwrap();
    let (mut next, _) = sample_committed_lane_block_session_for_state_test(
        lane, dataspace, incarnation, 4, 3,
    );
    next.proposal.descriptor.previous_lane_block_descriptor_hash = Some(ordinary_hash);
    State::validate_merge_execution_predecessor_against_frontier(
        &overlay.world, &next.proposal.descriptor,
    ).expect("next autonomous merge must extend the ordinary applied slot");
    next.proposal.descriptor.previous_lane_block_descriptor_hash = Some(autonomous_hash);
    assert!(State::validate_merge_execution_predecessor_against_frontier(
        &overlay.world, &next.proposal.descriptor,
    ).is_err(), "prior autonomous slot cannot bypass intervening ordinary application");
}
