// Real autonomous producer packing and fail-closed malformed-policy ingress.

fn set_autonomous_source_capacity_for_packing_test(state: &State, count: u32) {
    use iroha_data_model::parameter::{BlockParameter, FastpqSourcePolicyV1, Parameter};
    let mut world = state.world.block();
    let parameters = world.parameters.get_mut();
    let block = parameters.block();
    let policy = block.fastpq_source();
    let bounded = FastpqSourcePolicyV1::from_sizing(
        block.execution_output(),
        policy.intrinsic,
        policy.mandatory,
        count,
    )
    .unwrap();
    parameters.set_parameter(Parameter::Block(BlockParameter::FastpqSource(bounded)));
    world.commit();
}

#[test]
fn autonomous_producer_signs_only_a_source_that_fits_the_agreed_capacity() {
    let (mut adapter, keys) = autonomous_test_fixture(wire::ConsensusMode::Permissioned, true);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut adapter, &keys, lane_id, dataspace_id);
    set_autonomous_source_capacity_for_packing_test(adapter.state.as_ref(), 2);
    adapter
        .retain_merge_sidecars_for_global_view(0, None, None)
        .expect("install exact unlocked candidate view");
    assert_autonomous_test_role(&adapter, &keys, lane_id, dataspace_id, true);
    let journal_dir = tempfile::tempdir().unwrap();
    let journal_path = journal_dir.path().join("source-packing.norito");
    let queue = install_autonomous_test_queue(&mut adapter, lane_id, dataspace_id, &journal_path);
    let inputs = enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 4);
    adapter
        .schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(6, 6))
        .expect("cap before Queue reservation and source signing");
    let payload = adapter
        .pending_autonomous_anchor_payloads
        .values()
        .find(|payload| payload.origin_proposal.descriptor.lane_id == lane_id)
        .expect("real signed autonomous source")
        .clone();
    payload
        .validate(adapter.native_network_id(), adapter.context.epoch)
        .unwrap();
    assert_eq!(payload.entrypoints, inputs[..2]);
    assert_eq!(payload.reservation_keys.len(), 2);
    assert_eq!(queue.live_lane_reservations().len(), 2);
    assert_eq!(queue.queued_len(), 2);

    // Supported parameter execution forbids this post-genesis change. Inject it
    // directly only to check imported/corrupted policy mismatch fails closed and
    // does not rewrite an already authenticated whole source or its Queue owner.
    set_autonomous_source_capacity_for_packing_test(adapter.state.as_ref(), 1);
    assert_eq!(
        adapter.insert_autonomous_lane_payload(payload.clone(), Some(&payload.producer), 0),
        V2LaneIngressOutcome::Rejected
    );
    assert!(matches!(
        adapter.persist_and_authorize_autonomous_payload(&payload, &payload.origin_proposal),
        Err(AutonomousPayloadDurabilityError::Fatal(reason))
            if reason == "autonomous payload exceeds the agreed FASTPQ source capacity"
    ));
    assert!(
        adapter
            .pending_autonomous_anchor_payloads
            .values()
            .any(|actual| actual == &payload)
    );
    assert_eq!(queue.live_lane_reservations().len(), 2);
    assert_eq!(queue.queued_len(), 2);
}
