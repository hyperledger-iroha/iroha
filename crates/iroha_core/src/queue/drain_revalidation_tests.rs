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

/// Retain an actual durable claim across a committed close in a component fixture.
fn retained_queue_plan_drain_fixture(ranked: bool) -> GloballyBoundGuardFixture {
    let close = 5;
    let mut state = state_with_future_created_autoscale_lane(1, close - 1);
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Arc::new(queue_with_state_free_future_created_router(
        &state,
        &time_source,
    ));
    queue.install_test_router_metadata_for_nexus(&state.nexus_snapshot());
    let dir = tempdir().expect("retained drain journal directory");
    queue
        .install_plan_journal(&dir.path().join("plan.norito"), 1024 * 1024, true)
        .expect("install durable admission journal");
    let transaction = accepted_queue_plan_tx_by_someone(&time_source);
    let follower_transaction = accepted_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(&mut state, &transaction);
    let plan = queue
        .route_plan_with_state(&transaction, &state)
        .expect("open route");
    let context = queue
        .plan_admission_context_with_state(&state, &plan)
        .expect("pinned authority");
    let binding = crate::torii_proxy::new_queue_plan_admission_binding(
        state.network_id_ref(),
        transaction.entrypoint(),
        &plan,
        context,
        queue.queue_plan_admission_timestamp_ms(),
    )
    .expect("original exact binding");
    queue
        .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
            transaction.clone(),
            &state,
            plan,
            &binding,
        )
        .expect("original durable admission");
    if ranked {
        install_queue_plan_registry_value_for_test(&state, &binding);
    }
    seed_committed_height_for_queue_test(&state, close);
    install_autoscale_drain_close_for_queue_test(&state, LaneId::new(1), close);
    let mut runtime = state.canonical_runtime.block();
    runtime.get_mut().lanes = state.nexus.read().lane_catalog.lanes().to_vec();
    runtime.commit();
    GloballyBoundGuardFixture {
        state,
        queue,
        time_handle,
        transaction,
        follower_transaction,
        binding,
        transaction_time_to_live: config_factory().transaction_time_to_live,
        _dir: dir,
    }
}

#[test]
fn retained_queue_plan_drain_preserves_lookup_retry_and_defers_ordinary_selection() {
    let fixture = retained_queue_plan_drain_fixture(true);
    let plan = fixture.binding.routing_plan().unwrap();
    assert_eq!(
        State::queue_plan_pending_route_authority_in_view(&fixture.state.view(), &fixture.binding)
            .unwrap(),
        Some(QueuePlanPendingRouteAuthority::Draining)
    );
    assert_eq!(
        fixture
            .queue
            .route_plan_with_state(&fixture.transaction, &fixture.state)
            .unwrap(),
        plan
    );
    let durable = fixture
        .queue
        .durable_plan_admission_claim_with_state(&fixture.transaction, &fixture.state)
        .unwrap()
        .expect("original durable claim");
    assert_eq!(
        crate::torii_proxy::queue_plan_binding_from_durable_admission(&durable).unwrap(),
        fixture.binding
    );
    assert!(
        fixture
            .queue
            .has_revalidatable_durable_plan_claim_with_state(
                &fixture.transaction,
                &fixture.state,
                &plan,
                &fixture.binding.admission_context,
            )
    );
    let mut rebound = fixture.binding.admission_context.clone();
    rebound.authority_height += 1;
    rebound.proposal_height += 1;
    assert!(
        !fixture
            .queue
            .has_revalidatable_durable_plan_claim_with_state(
                &fixture.transaction,
                &fixture.state,
                &plan,
                &rebound,
            )
    );
    fixture
        .queue
        .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
            fixture.transaction.clone(),
            &fixture.state,
            plan,
            &fixture.binding,
        )
        .expect("lost response retries the exact closed owner");
    let (pending, lease) = fixture
        .queue
        .bounded_pending_snapshot(&fixture.state.view(), nonzero!(1_usize))
        .expect("draining work is a healthy selection deferral");
    assert!(pending.is_empty());
    drop(lease);
    let mut expired = Vec::new();
    assert!(
        fixture
            .queue
            .pop_from_queue(&fixture.state.view(), &mut expired)
            .is_none()
    );
    assert!(expired.is_empty());
    assert!(
        fixture
            .queue
            .gossip_batch_with_state(1, &fixture.state)
            .is_empty()
    );
    fixture.assert_restored_fifo_owner();
}

#[test]
fn retained_queue_plan_drain_replays_original_journal_after_close() {
    let fixture = retained_queue_plan_drain_fixture(true);
    let time_source = fixture.queue.time_source.clone();
    let GloballyBoundGuardFixture {
        queue,
        state,
        transaction,
        binding,
        _dir: dir,
        ..
    } = fixture;
    drop(queue);
    let reopened = queue_with_state_free_future_created_router(&state, &time_source);
    reopened.install_test_router_metadata_for_nexus(&state.nexus_snapshot());
    assert_eq!(
        reopened
            .install_plan_journal(&dir.path().join("plan.norito"), 1024 * 1024, true)
            .unwrap(),
        1
    );
    let replay = reopened
        .replay_plan_journal(&state)
        .expect("recover exact canonical pending work after close");
    assert_eq!(replay.records, 1);
    assert_eq!(replay.replayed, 1);
    let durable = reopened
        .durable_plan_admission_claim_with_state(&transaction, &state)
        .unwrap()
        .unwrap();
    assert_eq!(
        crate::torii_proxy::queue_plan_binding_from_durable_admission(&durable).unwrap(),
        binding
    );
    assert_eq!(
        reopened.fifo_snapshot_for_test(),
        vec![transaction.hash_as_entrypoint()]
    );
    assert!(!reopened.accepted_work_validation_faulted());
}

#[test]
fn retained_queue_plan_drain_handoff_requires_canonical_rank_and_terminalizes_unranked_claim() {
    let fixture = retained_queue_plan_drain_fixture(true);
    let handoff =
        queue_with_state_free_future_created_router(&fixture.state, &fixture.queue.time_source);
    handoff.install_test_router_metadata_for_nexus(&fixture.state.nexus_snapshot());
    handoff
        .install_plan_journal(
            &fixture._dir.path().join("handoff.norito"),
            1024 * 1024,
            true,
        )
        .unwrap();
    handoff
        .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
            fixture.transaction.clone(),
            &fixture.state,
            fixture.binding.routing_plan().unwrap(),
            &fixture.binding,
        )
        .expect("a distinct local Queue can recover exact canonical pre-close bytes");
    assert!(
        handoff
            .push_with_lane_with_state(fixture.follower_transaction.clone(), &fixture.state)
            .is_err()
    );
    assert_eq!(handoff.queued_len(), 1);
    assert!(!handoff.accepted_work_validation_faulted());

    let unranked = retained_queue_plan_drain_fixture(false);
    assert_eq!(
        State::queue_plan_pending_route_authority_in_view(
            &unranked.state.view(),
            &unranked.binding
        )
        .unwrap(),
        None
    );
    assert!(
        unranked
            .queue
            .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
                unranked.transaction.clone(),
                &unranked.state,
                unranked.binding.routing_plan().unwrap(),
                &unranked.binding,
            )
            .is_err(),
        "an off-chain receipt cannot acquire canonical drain authority"
    );
    unranked.assert_terminally_removed();
    assert!(
        !unranked.queue.transaction_selection_durability_faulted(),
        "a committed close must not turn unranked local custody into a queue-wide fault"
    );
}
