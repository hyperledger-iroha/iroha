#[test]
fn gossip_batch_returns_routing_metadata() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(config_factory(), &time_source);
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash_as_entrypoint();
    queue
        .push(tx, state.view())
        .expect("enqueue accepted transaction");
    let batch = queue.gossip_batch(1, &state.view());
    assert_eq!(batch.len(), 1);
    let entry = &batch[0];
    assert_eq!(entry.tx.as_ref().hash_as_entrypoint(), hash);
    let expected_payload =
        ncore::to_bytes(entry.tx.entrypoint()).expect("encode transaction entrypoint");
    assert_eq!(entry.payload.as_slice(), expected_payload.as_slice());
    assert_eq!(entry.routing.lane_id, LaneId::SINGLE);
    assert_eq!(entry.routing.dataspace_id, DataSpaceId::UNIVERSAL);
}
#[test]
fn gossip_batch_preserves_admitted_routing_across_policy_change() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let refreshed = RoutingDecision::new(LaneId::new(3), DataSpaceId::UNIVERSAL);
    let (fresh_lanes, fresh_dataspaces) = Queue::test_catalogs_for_routes(&[
        (LaneId::SINGLE, DataSpaceId::UNIVERSAL),
        (refreshed.lane_id, refreshed.dataspace_id),
    ]);
    let queue = Queue::test(config_factory(), &time_source);
    let (account_id, key_pair) = gen_account_in("wonderland");
    let tx = accepted_tx_with(
        account_id,
        &key_pair,
        &time_source,
        vec![InstructionBox::from(Log::new(
            Level::INFO,
            "fresh gossip route".into(),
        ))],
        Metadata::default(),
    );
    let hash = tx.as_ref().hash_as_entrypoint();
    queue.push(tx.clone(), state.view()).expect("push tx");
    assert_eq!(
        queue
            .routing_plans
            .get(&hash)
            .map(|entry| entry.value().coordinator_route()),
        Some(RoutingDecision::default())
    );
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = true;
    nexus.lane_catalog = (*fresh_lanes).clone();
    nexus.dataspace_catalog = (*fresh_dataspaces).clone();
    nexus.fees.base_fee = Quantity::zero();
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    nexus.routing_policy.default_lane = refreshed.lane_id;
    nexus.routing_policy.default_dataspace = refreshed.dataspace_id;
    state.set_nexus(nexus).expect("apply fresh Nexus state");
    let immutable_route = queue
        .route_plan_with_state(&tx, &state)
        .map(|plan| plan.coordinator_route())
        .expect("admitted route should remain active");
    assert_eq!(immutable_route, RoutingDecision::default());
    let batch = queue.gossip_batch_with_state(1, &state);
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0].routing, RoutingDecision::default());
    assert_eq!(
        queue
            .routing_plans
            .get(&hash)
            .map(|entry| entry.value().coordinator_route()),
        Some(RoutingDecision::default())
    );
    assert_eq!(
        queue
            .routing_plan_hint(&hash)
            .map(|plan| plan.coordinator_route()),
        Some(RoutingDecision::default())
    );
    assert!(!queue.accepted_work_validation_faulted());
}
