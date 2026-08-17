#[test]
fn gossip_batch_fails_closed_on_corrupt_native_amx_plan_index() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut fixture = native_amx_participant_drift_fixture(&time_source);
    {
        let nexus = fixture.state.nexus.get_mut();
        nexus.fees.base_fee = Quantity::zero();
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
    }
    assert_eq!(
        fixture.stale_plan.coordinator_route(),
        fixture.current_plan.coordinator_route()
    );
    assert_ne!(fixture.stale_plan, fixture.current_plan);
    let queue = Queue::test(config_factory(), &time_source);
    let hash = fixture.tx.hash_as_entrypoint();
    queue
        .push_with_gossip_payload_with_state_and_routing_plan(
            fixture.tx.clone(),
            &fixture.state,
            fixture.current_plan.clone(),
            None,
        )
        .expect("current Native AMX plan should enqueue");
    queue.routing_plans.insert(hash, fixture.stale_plan.clone());
    let batch = queue.gossip_batch_with_state(1, &fixture.state);
    assert!(batch.is_empty());
    assert!(queue.accepted_work_validation_faulted());
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 1);
    assert_eq!(
        queue.routing_plans.get(&hash).map(|entry| entry.clone()),
        Some(fixture.stale_plan.clone())
    );
    assert_eq!(queue.routing_plan_hint(&hash), Some(fixture.stale_plan));
}
#[test]
fn route_for_gossip_with_state_uses_router_decision() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let expected_lane = LaneId::SINGLE;
    let expected_dataspace = DataSpaceId::UNIVERSAL;
    let queue = Queue::test_with_router(
        config_factory(),
        &time_source,
        Arc::new(StaticRouter {
            lane: expected_lane,
            dataspace: expected_dataspace,
        }),
    );
    let tx = accepted_tx_by_someone(&time_source);
    let routing = queue
        .route_plan_for_gossip_with_state(&tx, state.as_ref())
        .map(|plan| plan.coordinator_route())
        .expect("route should resolve with configured catalogs");
    assert_eq!(routing.lane_id, expected_lane);
    assert_eq!(routing.dataspace_id, expected_dataspace);
}
#[test]
fn route_for_gossip_with_state_prefers_no_state_router_path() {
    struct PanicOnViewRouter {
        lane: LaneId,
        dataspace: DataSpaceId,
    }
    impl LaneRouter for PanicOnViewRouter {
        fn try_route(
            &self,
            _tx: &dyn TransactionRoutingView,
        ) -> Result<RoutingDecision, RoutingResolveError> {
            Ok(RoutingDecision::new(self.lane, self.dataspace))
        }
        fn try_route_without_state(
            &self,
            _tx: &dyn TransactionRoutingView,
        ) -> Result<Option<RoutingDecision>, RoutingResolveError> {
            Ok(Some(RoutingDecision::new(self.lane, self.dataspace)))
        }
    }
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let expected_lane = LaneId::SINGLE;
    let expected_dataspace = DataSpaceId::UNIVERSAL;
    let queue = Queue::test_with_router(
        config_factory(),
        &time_source,
        Arc::new(PanicOnViewRouter {
            lane: expected_lane,
            dataspace: expected_dataspace,
        }),
    );
    let tx = accepted_tx_by_someone(&time_source);
    let routing = queue
        .route_plan_for_gossip_with_state(&tx, state.as_ref())
        .map(|plan| plan.coordinator_route())
        .expect("route should resolve with configured catalogs");
    assert_eq!(routing.lane_id, expected_lane);
    assert_eq!(routing.dataspace_id, expected_dataspace);
}
#[test]
fn state_backed_queue_routes_reject_state_free_future_created_autoscale_hint() {
    let state = state_with_future_created_autoscale_lane(7, 6);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = queue_with_state_free_future_created_router(&state, &time_source);
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.hash_as_entrypoint();
    let route_err = queue
        .route_plan_with_state(&tx, &state)
        .expect_err("state-backed route must reject future-created state-free hint");
    assert_eq!(route_err.as_label(), "inactive_lane");
    assert!(matches!(
        route_err,
        RoutingResolveError::InactiveLane {
            lane_id,
            dataspace_id,
        } if lane_id == LaneId::new(1) && dataspace_id == DataSpaceId::UNIVERSAL
    ));
    let gossip_err = queue
        .route_plan_for_gossip_with_state(&tx, &state)
        .expect_err("state-backed gossip route must reject future-created state-free hint");
    assert_eq!(gossip_err.as_label(), "inactive_lane");
    assert!(matches!(
        gossip_err,
        RoutingResolveError::InactiveLane {
            lane_id,
            dataspace_id,
        } if lane_id == LaneId::new(1) && dataspace_id == DataSpaceId::UNIVERSAL
    ));
    let push_err = queue
        .push_with_gossip_payload_with_state(tx, &state, None)
        .expect_err("admission must reject future-created state-free hint");
    assert!(
        matches!(
            &push_err,
            Failure {
                err: Error::UnresolvedRoute { .. },
                ..
            }
        ),
        "unexpected admission error: {push_err:?}"
    );
    if let Error::UnresolvedRoute { reason } = &push_err.err {
        assert!(
            reason.contains("not active"),
            "inactive-lane admission rejection should explain the active-height boundary: {reason}"
        );
    }
    assert!(!queue.txs.contains_key(&hash));
    assert!(queue.routing_plans.get(&hash).is_none());
    assert_eq!(
        queue.routing_plan_hint(&hash),
        None,
        "rejected inactive state-free route must not enter the queue plan index"
    );
}
#[test]
fn state_backed_queue_rejects_new_ownership_at_committed_drain_close() {
    let close_height = 5;
    let lane_id = LaneId::new(1);
    let state = state_with_future_created_autoscale_lane(1, close_height);
    install_autoscale_drain_close_for_queue_test(&state, lane_id, close_height);
    let nexus = state.nexus_snapshot();
    let plan = RoutingPlan::single(RoutingDecision::new(lane_id, DataSpaceId::UNIVERSAL));
    assert_eq!(
        resolve_routing_plan_against_nexus_at_height(plan.clone(), &nexus, close_height)
            .expect("the closing lane remains valid for its exact close-height proposal"),
        plan
    );
    assert!(matches!(
        resolve_routing_plan_for_queue_admission(plan, &nexus, close_height),
        Err(RoutingResolveError::InactiveLane {
            lane_id: rejected_lane,
            dataspace_id: DataSpaceId::UNIVERSAL,
        }) if rejected_lane == lane_id
    ));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = queue_with_state_free_future_created_router(&state, &time_source);
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.hash_as_entrypoint();
    let failure = queue
        .push_with_lane_with_state(tx, &state)
        .expect_err("post-close ingress must not acquire ordinary queue ownership");
    assert!(matches!(failure.err, Error::UnresolvedRoute { .. }));
    assert!(!queue.txs.contains_key(&hash));
    assert!(queue.routing_plans.get(&hash).is_none());
    assert_eq!(queue.routing_plan_hint(&hash), None);
}
#[test]
fn state_backed_queue_routes_reject_unknown_dataspace_when_nexus_is_disabled() {
    let mut state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    state.nexus.get_mut().enabled = false;
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let dynamic_dataspace = DataSpaceId::new(4_242);
    let queue = Queue::test_with_router(
        config_factory(),
        &time_source,
        Arc::new(StaticRouter {
            lane: LaneId::SINGLE,
            dataspace: dynamic_dataspace,
        }),
    );
    queue.install_test_router_metadata_for_nexus(&state.nexus_snapshot());
    let tx = accepted_tx_by_someone(&time_source);
    assert_eq!(
        queue.route_plan_with_state(&tx, &state),
        Err(RoutingResolveError::UnknownDataspace {
            dataspace_id: dynamic_dataspace,
        })
    );
    assert_eq!(
        queue.route_plan_for_gossip_with_state(&tx, &state),
        Err(RoutingResolveError::UnknownDataspace {
            dataspace_id: dynamic_dataspace,
        })
    );
}
