// Exact global-carrier anchors for the replicated applied lane frontier.

#[derive(Clone, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::state::AppliedMergeLaneFrontierMarker")]
struct FrontierWithoutAppliedHeightForTest {
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    lane_block_descriptor_hash: Hash,
}

state_test! { sync canonical_lane_frontier_anchor_roundtrips_and_rejects_missing_or_zero_height
    let marker = AppliedMergeLaneFrontierMarker {
        version: 1,
        lane_id: LaneId::new(7),
        dataspace_id: DataSpaceId::new(9),
        lane_incarnation: Hash::new(b"anchored-frontier-incarnation"),
        lane_block_height: 41,
        lane_block_descriptor_hash: Hash::new(b"anchored-frontier-descriptor"),
        applied_global_height: 73,
    };
    let (key, payload) = State::encode_merge_lane_frontier_marker(marker).unwrap();
    assert_eq!(State::decode_exact_merge_lane_frontier_marker(&key, &payload).unwrap(), marker);
    let mut world = World::default();
    assert_eq!(State::canonical_merged_lane_frontier_with_anchor_from_world(
        &world.view(), marker.lane_id, marker.dataspace_id, marker.lane_incarnation,
    ).unwrap(), (0, None, 0));
    world.smart_contract_state.insert(key.clone(), payload);
    assert_eq!(State::canonical_merged_lane_frontier_with_anchor_from_world(
        &world.view(), marker.lane_id, marker.dataspace_id, marker.lane_incarnation,
    ).unwrap(), (41, Some(marker.lane_block_descriptor_hash), 73));
    assert_eq!(State::canonical_merged_lane_frontier_from_world(
        &world.view(), marker.lane_id, marker.dataspace_id, marker.lane_incarnation,
    ).unwrap(), (41, Some(marker.lane_block_descriptor_hash)));

    let mut zero = marker;
    zero.applied_global_height = 0;
    let (_, zero_payload) = State::encode_merge_lane_frontier_marker(zero).unwrap();
    assert!(State::decode_exact_merge_lane_frontier_marker(&key, &zero_payload).is_err());
    let omitted = FrontierWithoutAppliedHeightForTest {
        version: marker.version,
        lane_id: marker.lane_id,
        dataspace_id: marker.dataspace_id,
        lane_incarnation: marker.lane_incarnation,
        lane_block_height: marker.lane_block_height,
        lane_block_descriptor_hash: marker.lane_block_descriptor_hash,
    };
    let old_layout = norito::to_bytes(&omitted).unwrap();
    assert!(State::decode_exact_merge_lane_frontier_marker(&key, &old_layout).is_err(),
        "first release must not infer an application anchor from the retired layout");
}

state_test! { sync lane_frontier_staging_rejects_anchor_regression_and_allows_same_carrier_successors
    let state = blank_test_state();
    let lane = LaneId::SINGLE;
    let dataspace = DataSpaceId::UNIVERSAL;
    let incarnation = Hash::new(b"monotonic-anchor-incarnation");
    let (block, _, _) = lane_artifact_block_and_session_for_state_test(
        None, lane, dataspace, incarnation, 1,
    );
    let mut overlay = state.block(block.header());
    let marker = |lane_id: LaneId, lane_height: u64, applied_global_height: u64| AppliedMergeLaneFrontierMarker {
        version: 1,
        lane_id,
        dataspace_id: dataspace,
        lane_incarnation: incarnation,
        lane_block_height: lane_height,
        lane_block_descriptor_hash: Hash::new(lane_height.to_le_bytes()),
        applied_global_height,
    };
    overlay.stage_merge_lane_frontier_markers(vec![
        State::encode_merge_lane_frontier_marker(marker(lane, 1, 7)).unwrap(),
    ]).unwrap();
    let before = State::canonical_merged_lane_frontier_with_anchor_from_world(
        &overlay.world, lane, dataspace, incarnation,
    ).unwrap();
    let other_lane = LaneId::new(7);
    assert!(overlay.stage_merge_lane_frontier_markers(vec![
        State::encode_merge_lane_frontier_marker(marker(other_lane, 1, 7)).unwrap(),
        State::encode_merge_lane_frontier_marker(marker(lane, 2, 6)).unwrap(),
    ]).is_err());
    assert_eq!(State::canonical_merged_lane_frontier_with_anchor_from_world(
        &overlay.world, lane, dataspace, incarnation,
    ).unwrap(), before);
    assert_eq!(State::canonical_merged_lane_frontier_with_anchor_from_world(
        &overlay.world, other_lane, dataspace, incarnation,
    ).unwrap(), (0, None, 0), "all proposed lane updates must preflight before publication");
    overlay.stage_merge_lane_frontier_markers(vec![
        State::encode_merge_lane_frontier_marker(marker(lane, 2, 7)).unwrap(),
    ]).expect("a second contiguous lane decision can share the same global carrier");
    assert_eq!(State::canonical_merged_lane_frontier_with_anchor_from_world(
        &overlay.world, lane, dataspace, incarnation,
    ).unwrap(), (2, Some(Hash::new(2_u64.to_le_bytes())), 7));
    overlay.stage_merge_lane_frontier_markers(vec![
        State::encode_merge_lane_frontier_marker(marker(lane, 3, 8)).unwrap(),
    ]).expect("a later global carrier advances the anchor");
}

state_test! { sync native_participant_frontier_retains_the_actual_application_carrier_height
    let block = crate::sumeragi::exec::result_bearing_native_manifest_block_for_tests();
    let expected = State::native_amx_participant_frontier_markers(&block).unwrap();
    assert_eq!(expected.len(), 2, "exercise both separate participant routes");
    let state = blank_test_state();
    let mut overlay = state.block(block.header());
    overlay.stage_native_amx_participant_frontiers(&block).unwrap();
    for marker in expected {
        assert_eq!(State::canonical_merged_lane_frontier_with_anchor_from_world(
            &overlay.world, marker.lane_id, marker.dataspace_id, marker.lane_incarnation,
        ).unwrap(), (
            marker.lane_block_height,
            Some(marker.lane_block_descriptor_hash),
            block.header().height().get(),
        ));
    }
}

state_test!(consensus_stack native_decision_frontier_retains_the_actual_application_carrier_height
    native_decision_frontier_retains_the_actual_application_carrier_height_on_consensus_stack();
);
fn native_decision_frontier_retains_the_actual_application_carrier_height_on_consensus_stack() {
    let (fixture, carrier, context) =
        native_publication_fixture_for_test(&[NativeEconomicCase::Transfer(25)]);
    let state = &fixture.native.state;
    let batch = carrier
        .execution_context()
        .unwrap()
        .native_lane_decisions
        .as_deref()
        .unwrap();
    assert!(!batch.groups.is_empty());
    let (overlay, committed) = prepared_native_publication_for_test(state, &carrier, context);
    overlay
        .commit()
        .expect("apply the actual Decision-authenticated native carrier");
    promote_native_execution_finality_for_test(state, &committed);
    let world = state.world.view();
    for group in &batch.groups {
        let descriptor = &group.payload.descriptor;
        for slot in &descriptor.slots {
            assert_eq!(
                State::canonical_merged_lane_frontier_with_anchor_from_world(
                    &world,
                    slot.route.lane_id,
                    slot.route.dataspace_id,
                    slot.lane_incarnation,
                )
                .unwrap(),
                (
                    slot.lane_height,
                    Some(descriptor.canonical_hash().unwrap()),
                    carrier.header().height().get(),
                )
            );
        }
    }
}

state_test! { sync snapshot_settlement_frontier_retains_the_actual_application_carrier_height
    let (state, _, _, commit_keypairs) = setup_nexus_fee_merge_state(
        Quantity::from(10_u32), Quantity::from(3_u32), [0x46; 32],
    );
    let candidate = state.merge_entry_candidates_from_lane_relays().into_iter().next().unwrap();
    let qc = merge_qc_for_candidate(&state, &candidate, &commit_keypairs, &[0]);
    let entry = merge_entry_from_candidate(candidate, qc);
    assert!(entry.execution_batch.is_none());
    assert!(!entry.lane_snapshots.is_empty());
    let carrier = store_merge_carrier_without_state_publication_for_test(&state, &entry);
    let reference = iroha_data_model::block::CertifiedMergeLedgerReference::new(&entry);
    let mut overlay = state.block_with_certified_merge_reference(
        carrier.as_ref().header().clone(), &reference, ConsensusMode::Permissioned,
    ).expect("stage the exact globally certified snapshot settlement");
    let _ = overlay.apply_without_execution(&carrier, Vec::new());
    overlay.commit().expect("commit the exact snapshot-settlement carrier");
    let world = state.world.view();
    for snapshot in &entry.lane_snapshots {
        let descriptor_hash = snapshot.relay_envelope.as_ref().unwrap()
            .lane_block_descriptor_hash.unwrap();
        assert_eq!(State::canonical_merged_lane_frontier_with_anchor_from_world(
            &world, snapshot.lane_id, snapshot.dataspace_id, snapshot.lane_incarnation,
        ).unwrap(), (
            snapshot.lane_block_height, Some(descriptor_hash), carrier.as_ref().header().height().get(),
        ));
    }
}

state_test! { sync ordinary_lane_frontier_apply_rejects_same_descriptor_with_wrong_global_height
    let state = blank_test_state();
    let lane = LaneId::SINGLE;
    let dataspace = DataSpaceId::UNIVERSAL;
    let incarnation = Hash::new(b"ordinary-exact-anchor-incarnation");
    let (block, _, _) = lane_artifact_block_and_session_for_state_test(
        None, lane, dataspace, incarnation, 1,
    );
    let mut overlay = state.block(block.header());
    overlay.stage_ordinary_lane_frontiers(&block).unwrap();
    overlay.verify_ordinary_lane_frontiers(&block).unwrap();
    let key = State::merge_lane_frontier_marker_key(lane, dataspace, incarnation).unwrap();
    let payload = overlay.world.smart_contract_state.get(&key).unwrap();
    let mut marker = State::decode_exact_merge_lane_frontier_marker(&key, payload).unwrap();
    assert_eq!(marker.applied_global_height, block.header().height().get());
    marker.applied_global_height += 1;
    let (_, wrong_height) = State::encode_merge_lane_frontier_marker(marker).unwrap();
    overlay.world.smart_contract_state.insert(key.clone(), wrong_height.clone());
    assert!(overlay.verify_ordinary_lane_frontiers(&block).is_err(),
        "same lane descriptor does not authenticate another carrier's execution");
    assert_eq!(overlay.world.smart_contract_state.get(&key), Some(&wrong_height),
        "verification must not repair or publish an invented carrier anchor");
}
