#[test]
fn state_backed_queue_rechecks_late_drain_publication_under_lifecycle_fence() {
    let close_height = 5;
    let lane_id = LaneId::new(1);
    let state = Arc::new(state_with_future_created_autoscale_lane(1, close_height));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Arc::new(queue_with_state_free_future_created_router(
        state.as_ref(),
        &time_source,
    ));
    queue.install_test_router_metadata_for_nexus(&state.nexus_snapshot());
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.hash_as_entrypoint();
    let routing_plan = queue
        .route_plan_with_state(&tx, state.as_ref())
        .expect("the original route is live before drain publication");
    assert_eq!(
        routing_plan,
        RoutingPlan::single(RoutingDecision::new(lane_id, DataSpaceId::UNIVERSAL))
    );
    {
        let control = queue_with_state_free_future_created_router(state.as_ref(), &time_source);
        control.install_test_router_metadata_for_nexus(&state.nexus_snapshot());
        assert_eq!(
            control
                .push_with_lane_with_state_and_routing_plan(
                    tx.clone(),
                    state.as_ref(),
                    routing_plan.clone(),
                )
                .expect("the exact transaction and route are admissible before drain publication"),
            routing_plan.coordinator_route(),
        );
        assert!(control.txs.contains_key(&hash));
    }
    let lifecycle_guard = state.lock_lane_lifecycle_work_admission();
    let (started_sender, started_receiver) = mpsc::sync_channel(0);
    let worker_state = Arc::clone(&state);
    let worker_queue = Arc::clone(&queue);
    let worker = thread::spawn(move || {
        started_sender
            .send(())
            .expect("announce queue admission attempt");
        worker_queue.push_with_lane_with_state_and_routing_plan(
            tx,
            worker_state.as_ref(),
            routing_plan,
        )
    });
    started_receiver
        .recv()
        .expect("queue admission worker started");
    install_autoscale_drain_close_for_queue_test(state.as_ref(), lane_id, close_height);
    // State views read the canonical runtime owner, not the diagnostic Nexus
    // projection. Publish this fixture's close while admission owns the fence.
    {
        let mut runtime = state.canonical_runtime.block();
        runtime.get_mut().lanes = state.nexus.read().lane_catalog.lanes().to_vec();
        runtime.commit();
    }
    assert!(matches!(
        resolve_routing_plan_for_queue_admission(
            RoutingPlan::single(RoutingDecision::new(lane_id, DataSpaceId::UNIVERSAL)),
            &state.nexus_snapshot(),
            close_height,
        ),
        Err(RoutingResolveError::InactiveLane { lane_id: rejected_lane, .. }) if rejected_lane == lane_id
    ));
    drop(lifecycle_guard);
    let failure = worker
        .join()
        .expect("queue admission worker")
        .expect_err("late committed drain must win before queue ownership publication");
    assert!(
        matches!(failure.err, Error::UnresolvedRoute { .. }),
        "unexpected admission error: {failure:?}"
    );
    if let Error::UnresolvedRoute { reason } = &failure.err {
        assert_eq!(
            reason,
            &RoutingResolveError::InactiveLane {
                lane_id,
                dataspace_id: DataSpaceId::UNIVERSAL,
            }
            .to_string()
        );
    }
    assert_eq!(failure.tx.hash_as_entrypoint(), hash);
    assert!(!queue.txs.contains_key(&hash));
    assert!(queue.routing_plans.get(&hash).is_none());
    assert_eq!(queue.routing_plan_hint(&hash), None);
}
