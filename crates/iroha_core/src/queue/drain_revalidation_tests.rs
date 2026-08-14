#[test]
fn state_backed_queue_rechecks_late_drain_publication_under_lifecycle_fence() {
    let close_height = 5;
    let lane_id = LaneId::new(1);
    let state = Arc::new(state_with_future_created_autoscale_lane(1, close_height));
    {
        let mut nexus = state.nexus.write();
        nexus.routing_policy.rules = vec![LaneRoutingRule {
            lane: lane_id,
            dataspace: Some(DataSpaceId::UNIVERSAL),
            matcher: LaneRoutingMatcher {
                account: None,
                instruction: Some("unregister::domain".to_owned()),
                description: Some("late drain lifecycle-fence fixture".to_owned()),
            },
        }];
    }
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Arc::new(queue_with_state_free_future_created_router(
        state.as_ref(),
        &time_source,
    ));
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.hash();
    let lifecycle_guard = state.lock_lane_lifecycle_work_admission();
    let (started_sender, started_receiver) = mpsc::sync_channel(0);
    let worker_state = Arc::clone(&state);
    let worker_queue = Arc::clone(&queue);
    let worker = thread::spawn(move || {
        started_sender
            .send(())
            .expect("announce queue admission attempt");
        worker_queue.push_with_lane_with_state(tx, worker_state.as_ref())
    });
    started_receiver
        .recv()
        .expect("queue admission worker started");
    install_autoscale_drain_close_for_queue_test(state.as_ref(), lane_id, close_height);
    drop(lifecycle_guard);
    let failure = worker
        .join()
        .expect("queue admission worker")
        .expect_err("late committed drain must win before queue ownership publication");
    assert!(matches!(failure.err, Error::UnresolvedRoute { .. }));
    assert!(!queue.txs.contains_key(&hash));
    assert!(queue.routing_plans.get(&hash).is_none());
    assert_eq!(routing_ledger::get_plan(&hash), None);
}
#[test]
fn state_backed_queue_routes_reject_inactive_catalog_lane_when_nexus_forcibly_disabled() {
    let mut state = state_with_future_created_autoscale_lane(7, 6);
    // Simulate stale or corrupted persisted state. `State::set_nexus` rejects this
    // disabled multi-lane shape, but queue admission must still fail closed if it
    // ever observes one.
    state.nexus.get_mut().enabled = false;
    let nexus = state.nexus_snapshot();
    assert!(!nexus.enabled);
    assert!(nexus.uses_multilane_catalogs());
    assert_eq!(
        crate::state::nexus_active_lane_dataspace_at_height(
            LaneId::new(1),
            &nexus,
            state_height_for_routing(&state),
        ),
        None
    );
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = queue_with_state_free_future_created_router(&state, &time_source);
    let tx = accepted_tx_by_someone(&time_source);
    let route_err = queue
        .route_plan_with_state(&tx, &state)
        .expect_err("disabled Nexus must reject inactive catalog lanes");
    assert!(matches!(
        route_err,
        RoutingResolveError::InactiveLane {
            lane_id,
            dataspace_id,
        } if lane_id == LaneId::new(1) && dataspace_id == DataSpaceId::UNIVERSAL
    ));
    let gossip_err = queue
        .route_plan_for_gossip_with_state(&tx, &state)
        .expect_err("disabled Nexus gossip routing must reject inactive catalog lanes");
    assert!(matches!(
        gossip_err,
        RoutingResolveError::InactiveLane {
            lane_id,
            dataspace_id,
        } if lane_id == LaneId::new(1) && dataspace_id == DataSpaceId::UNIVERSAL
    ));
}
