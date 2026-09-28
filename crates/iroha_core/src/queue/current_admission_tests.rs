// Current executable admission must precede any queue, fee, or journal custody.
fn current_admission_queue_fixture() -> (
    State,
    Queue,
    TimeSource,
    tempfile::TempDir,
    std::path::PathBuf,
) {
    let mut state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    install_single_validator_topology_for_queue_test(&mut state, 0xB7);
    let (_clock, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        Arc::new(StaticRouter {
            lane: LaneId::SINGLE,
            dataspace: DataSpaceId::UNIVERSAL,
        }),
        &[],
    );
    let directory = tempfile::tempdir().expect("current admission journal directory");
    let path = directory.path().join("current-admission.norito");
    queue
        .install_plan_journal(&path, 1024 * 1024, true)
        .expect("install journal");
    (state, queue, time_source, directory, path)
}

#[test]
fn current_admission_rejects_unsupported_intent_across_direct_queue_boundaries() {
    let (mut state, queue, time_source, _directory, path) = current_admission_queue_fixture();
    let unsupported = accepted_queue_plan_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(&mut state, &unsupported);
    let plan = queue
        .route_plan_with_state(&unsupported, &state)
        .expect("single route");
    let before = std::fs::read(&path).expect("journal before rejected attempts");
    for boundary in 0..5 {
        let result = match boundary {
            0 => queue.push(unsupported.clone(), state.view()).map(|_| ()),
            1 => queue
                .push_with_lane_with_state(unsupported.clone(), &state)
                .map(|_| ()),
            2 => queue
                .push_with_gossip_payload_with_state_and_routing_plan(
                    unsupported.clone(),
                    &state,
                    plan.clone(),
                    None,
                )
                .map(|_| ()),
            3 => queue
                .push_with_lane_with_state_and_routing_plan_strict_durable(
                    unsupported.clone(),
                    &state,
                    plan.clone(),
                )
                .map(|_| ()),
            _ => queue
                .push_batch_with_lane_with_state_and_routing_plans(
                    vec![(unsupported.clone(), plan.clone())],
                    &state,
                )
                .map(|_| ()),
        };
        let failure = result.expect_err("unsupported signed intent must be rejected");
        assert!(matches!(
            failure.err,
            Error::UnsupportedTransactionAdmission { .. }
        ));
        assert_eq!(failure.tx.entrypoint(), unsupported.entrypoint());
        assert_eq!(queue.active_len(), 0);
        assert!(queue.txs.is_empty());
        assert!(queue.durable_plan_claims.is_empty());
        assert!(
            queue
                .fee_admission_reservations
                .lock()
                .live_by_entrypoint
                .is_empty()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    let ordinary = accepted_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(&mut state, &ordinary);
    let ordinary_plan = queue.route_plan_with_state(&ordinary, &state).unwrap();
    queue
        .push_with_lane_with_state_and_routing_plan_strict_durable(ordinary, &state, ordinary_plan)
        .expect("supported Ordinary work retains real durable admission");
    assert_eq!(queue.active_len(), 1);
    assert_ne!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn current_admission_rejects_actual_multiroute_before_queue_custody() {
    let (_clock, time_source) = TimeSource::new_mock(Duration::default());
    let fixture = native_amx_participant_drift_fixture(&time_source);
    let queue = Queue::test(config_factory(), &time_source);
    let actual = queue
        .route_plan_with_state(&fixture.tx, &fixture.state)
        .expect("real multi-route resolves");
    assert_eq!(actual, fixture.current_plan);
    assert!(!matches!(actual, RoutingPlan::Single(_)));
    assert_eq!(
        fixture.tx.entrypoint().admission_intent(),
        TransactionAdmissionIntent::Ordinary
    );
    for boundary in 0..3 {
        let failure = match boundary {
            0 => queue
                .push(fixture.tx.clone(), fixture.state.view())
                .map(|_| ()),
            1 => queue
                .push_with_lane_with_state_and_routing_plan(
                    fixture.tx.clone(),
                    &fixture.state,
                    actual.clone(),
                )
                .map(|_| ()),
            _ => queue
                .push_batch_with_lane_with_state_and_routing_plans(
                    vec![(fixture.tx.clone(), actual.clone())],
                    &fixture.state,
                )
                .map(|_| ()),
        }
        .expect_err("resolved multi-route work has no current execution owner");
        assert!(matches!(
            failure.err,
            Error::UnsupportedTransactionAdmission { .. }
        ));
        assert_eq!(queue.active_len(), 0);
        assert!(queue.durable_plan_claims.is_empty());
        assert!(
            queue
                .fee_admission_reservations
                .lock()
                .live_by_entrypoint
                .is_empty()
        );
    }
}

#[test]
fn current_admission_replay_rejects_unsupported_record_without_publishing_or_rewriting() {
    let (mut state, queue, time_source, _directory, path) = current_admission_queue_fixture();
    let unsupported = accepted_queue_plan_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(&mut state, &unsupported);
    let plan = queue.route_plan_with_state(&unsupported, &state).unwrap();
    let context = queue
        .plan_admission_context_with_state(&state, &plan)
        .unwrap();
    // Seed an on-disk incompatible record directly; the live API must never create it.
    queue
        .record_plan_journal_put_durable(
            &unsupported,
            &plan,
            &context,
            queue.queue_plan_admission_timestamp_ms(),
            None,
            None,
            true,
        )
        .expect("write incompatible disk fixture");
    let before = std::fs::read(&path).unwrap();
    drop(queue);
    let replay = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        Arc::new(StaticRouter {
            lane: LaneId::SINGLE,
            dataspace: DataSpaceId::UNIVERSAL,
        }),
        &[],
    );
    assert_eq!(
        replay
            .install_plan_journal(&path, 1024 * 1024, true)
            .unwrap(),
        1
    );
    let error = replay
        .replay_plan_journal(&state)
        .expect_err("unsupported custody must fail startup explicitly");
    assert!(
        error
            .to_string()
            .contains("unsupported_transaction_admission")
    );
    assert_eq!(replay.active_len(), 0);
    assert!(replay.txs.is_empty());
    assert!(replay.durable_plan_claims.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
