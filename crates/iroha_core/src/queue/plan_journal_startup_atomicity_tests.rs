// QueuePlan startup replay and receipt-publication atomicity regressions.
#[test]
fn queue_plan_journal_replays_matching_plan_after_restart() {
    let dir = tempfile::tempdir().expect("tempdir");
    let journal_path = dir.path().join("queue_plan_journal.norito");
    let mut state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = false;
    state
        .set_nexus(nexus)
        .expect("apply disabled Nexus state for canonical single-lane route test");
    install_single_validator_topology_for_queue_test(&state, 0xA7);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
        lane: LaneId::SINGLE,
        dataspace: DataSpaceId::UNIVERSAL,
    });
    let queue =
        Queue::test_with_router_for_routes(config_factory(), &time_source, router.clone(), &[]);
    assert_eq!(
        queue
            .install_plan_journal(&journal_path, 1024 * 1024, true)
            .expect("install journal"),
        0
    );
    let tx = accepted_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(&mut state, &tx);
    let hash = tx.hash();
    let plan = queue.route_plan_with_state(&tx, &state).expect("route");
    let payload = tx.entrypoint_bytes();
    queue
        .push_with_gossip_payload_with_state_and_routing_plan(
            tx,
            &state,
            plan.clone(),
            Some(payload.clone()),
        )
        .expect("push with plan");
    let journal_len_before_replay = std::fs::metadata(&journal_path)
        .expect("journal metadata before replay")
        .len();
    drop(queue);
    let replay_queue =
        Queue::test_with_router_for_routes(config_factory(), &time_source, router, &[]);
    assert_eq!(
        replay_queue
            .install_plan_journal(&journal_path, 1024 * 1024, true)
            .expect("install replay journal"),
        1
    );
    let summary = replay_queue
        .replay_plan_journal(&state)
        .expect("replay journal");
    assert_eq!(summary.records, 1);
    assert_eq!(summary.replayed, 1);
    let journal_len_after_replay = std::fs::metadata(&journal_path)
        .expect("journal metadata after replay")
        .len();
    assert_eq!(
        journal_len_after_replay, journal_len_before_replay,
        "journal replay must not duplicate already-durable put records"
    );
    assert!(replay_queue.txs.contains_key(&hash));
    assert_eq!(
        *replay_queue
            .routing_plans
            .get(&hash)
            .expect("replayed plan"),
        plan
    );
    let replayed_tx = replay_queue.txs.get(&hash).expect("replayed transaction");
    assert_eq!(
        replayed_tx
            .value()
            .as_accepted()
            .entrypoint_bytes()
            .as_slice(),
        payload.as_slice()
    );
}
#[test]
fn queue_plan_startup_receipt_failure_precedes_atomic_publication() {
    let dir = tempfile::tempdir().expect("tempdir");
    let journal_path = dir.path().join("queue_plan_receipt_preflight.norito");
    let mut state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = false;
    state
        .set_nexus(nexus)
        .expect("apply disabled Nexus state for canonical single-lane route test");
    install_single_validator_topology_for_queue_test(&state, 0xD7);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
        lane: LaneId::SINGLE,
        dataspace: DataSpaceId::UNIVERSAL,
    });
    let queue = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        Arc::clone(&router),
        &[],
    );
    queue
        .install_plan_journal(&journal_path, 1024 * 1024, true)
        .expect("install journal");
    let tx = accepted_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(&mut state, &tx);
    let hash = tx.hash();
    let plan = queue.route_plan_with_state(&tx, &state).expect("route");
    queue
        .push_with_lane_with_state_and_routing_plan_strict_durable(tx, &state, plan)
        .expect("persist exact QueuePlan claim");
    drop(queue);
    let replay_queue =
        Queue::test_with_router_for_routes(config_factory(), &time_source, router, &[]);
    assert_eq!(
        replay_queue
            .install_plan_journal(&journal_path, 1024 * 1024, true)
            .expect("install replay journal"),
        1
    );
    replay_queue.inject_plan_journal_fault(QueuePlanJournalTestFault::StartupReplayReceiptObserve);
    let error = replay_queue
        .replay_plan_journal(&state)
        .expect_err("receipt authentication fault must abort before Queue publication");
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
    assert_eq!(replay_queue.active_len(), 0);
    assert!(replay_queue.txs.is_empty());
    assert!(replay_queue.tx_hashes.is_empty());
    assert!(replay_queue.fifo_order_by_hash.is_empty());
    assert!(replay_queue.routing_plans.is_empty());
    assert!(replay_queue.durable_plan_claims.is_empty());
    assert!(
        replay_queue
            .plan_journal_startup_replay_receipt
            .lock()
            .is_none()
    );
    assert_eq!(
        replay_queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed journal")
            .live_record_count()
            .expect("bounded journal replay count"),
        1,
        "receipt preflight failure must retain the exact durable claim for retry"
    );
    let summary = replay_queue
        .replay_plan_journal(&state)
        .expect("one-shot receipt fault must leave a clean retry boundary");
    assert_eq!(summary.replayed, 1);
    assert!(replay_queue.txs.contains_key(&hash));
    assert!(
        replay_queue
            .plan_journal_startup_replay_receipt
            .lock()
            .is_some()
    );
}
#[test]
fn queue_plan_startup_receipt_failure_after_terminal_cleanup_retries_as_empty_stutter() {
    let dir = tempfile::tempdir().expect("tempdir");
    let journal_path = dir
        .path()
        .join("queue_plan_terminal_receipt_preflight.norito");
    let mut state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = false;
    state
        .set_nexus(nexus)
        .expect("apply disabled Nexus state for canonical single-lane route test");
    install_single_validator_topology_for_queue_test(&state, 0xD8);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
        lane: LaneId::SINGLE,
        dataspace: DataSpaceId::UNIVERSAL,
    });
    let queue = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        Arc::clone(&router),
        &[],
    );
    queue
        .install_plan_journal(&journal_path, 1024 * 1024, true)
        .expect("install journal");
    let tx = accepted_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(&mut state, &tx);
    let hash = tx.hash();
    let plan = queue.route_plan_with_state(&tx, &state).expect("route");
    queue
        .push_with_lane_with_state_and_routing_plan_strict_durable(tx, &state, plan)
        .expect("persist exact QueuePlan claim");
    drop(queue);
    {
        let mut transactions = state.transactions.block();
        transactions.insert_block_with_single_tx(hash, nonzero!(1_usize));
        transactions
            .commit()
            .expect("commit replay fixture transaction");
    }
    let replay_queue =
        Queue::test_with_router_for_routes(config_factory(), &time_source, router, &[]);
    assert_eq!(
        replay_queue
            .install_plan_journal(&journal_path, 1024 * 1024, true)
            .expect("install replay journal"),
        1
    );
    replay_queue.inject_plan_journal_fault(QueuePlanJournalTestFault::StartupReplayReceiptObserve);
    let error = replay_queue
        .replay_plan_journal(&state)
        .expect_err("receipt fault follows exact durable terminal cleanup");
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
    assert_eq!(replay_queue.active_len(), 0);
    assert!(replay_queue.txs.is_empty());
    assert!(replay_queue.tx_hashes.is_empty());
    assert!(
        replay_queue
            .plan_journal_startup_replay_receipt
            .lock()
            .is_none()
    );
    assert_eq!(
        replay_queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed journal")
            .live_record_count()
            .expect("bounded journal replay count"),
        0,
        "a canonically committed owner may be durably removed before receipt observation",
    );
    assert_eq!(
        replay_queue
            .replay_plan_journal(&state)
            .expect("retry authenticates the already-cleaned empty journal"),
        QueuePlanJournalReplaySummary::default(),
        "retry must stutter instead of resurrecting the terminal owner",
    );
    assert_eq!(replay_queue.active_len(), 0);
    assert!(!replay_queue.txs.contains_key(&hash));
    assert!(
        replay_queue
            .plan_journal_startup_replay_receipt
            .lock()
            .is_some()
    );
}
#[test]
fn queue_plan_startup_receipt_failure_after_mixed_terminal_cleanup_replays_live_suffix() {
    let dir = tempfile::tempdir().expect("tempdir");
    let journal_path = dir
        .path()
        .join("queue_plan_mixed_terminal_receipt_preflight.norito");
    let mut state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = false;
    state
        .set_nexus(nexus)
        .expect("apply disabled Nexus state for canonical single-lane route test");
    install_single_validator_topology_for_queue_test(&state, 0xD9);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
        lane: LaneId::SINGLE,
        dataspace: DataSpaceId::UNIVERSAL,
    });
    let queue = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        Arc::clone(&router),
        &[],
    );
    queue
        .install_plan_journal(&journal_path, 1024 * 1024, true)
        .expect("install journal");
    let terminal = accepted_tx_by_someone(&time_source);
    let live = accepted_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(&mut state, &terminal);
    register_accepted_tx_authority_for_queue_test(&mut state, &live);
    let terminal_hash = terminal.hash();
    let live_hash = live.hash();
    let terminal_plan = queue
        .route_plan_with_state(&terminal, &state)
        .expect("route terminal transaction");
    let live_plan = queue
        .route_plan_with_state(&live, &state)
        .expect("route live transaction");
    queue
        .push_with_lane_with_state_and_routing_plan_strict_durable(terminal, &state, terminal_plan)
        .expect("persist terminal QueuePlan claim");
    queue
        .push_with_lane_with_state_and_routing_plan_strict_durable(live, &state, live_plan)
        .expect("persist live QueuePlan claim");
    drop(queue);
    {
        let mut transactions = state.transactions.block();
        transactions.insert_block_with_single_tx(terminal_hash, nonzero!(1_usize));
        transactions
            .commit()
            .expect("commit only the terminal replay transaction");
    }
    let replay_queue =
        Queue::test_with_router_for_routes(config_factory(), &time_source, router, &[]);
    assert_eq!(
        replay_queue
            .install_plan_journal(&journal_path, 1024 * 1024, true)
            .expect("install replay journal"),
        2
    );
    replay_queue.inject_plan_journal_fault(QueuePlanJournalTestFault::StartupReplayReceiptObserve);
    let error = replay_queue
        .replay_plan_journal(&state)
        .expect_err("receipt fault follows exact cleanup of only the terminal prefix");
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
    assert_eq!(replay_queue.active_len(), 0);
    assert!(replay_queue.txs.is_empty());
    assert!(replay_queue.tx_hashes.is_empty());
    assert!(
        replay_queue
            .plan_journal_startup_replay_receipt
            .lock()
            .is_none()
    );
    assert_eq!(
        replay_queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed journal")
            .live_record_count()
            .expect("bounded journal replay count"),
        1,
        "terminal cleanup must retain the independently live durable suffix",
    );
    let summary = replay_queue
        .replay_plan_journal(&state)
        .expect("retry authenticates and publishes the retained live suffix");
    assert_eq!(summary.records, 1);
    assert_eq!(summary.replayed, 1);
    assert_eq!(replay_queue.active_len(), 1);
    assert!(!replay_queue.txs.contains_key(&terminal_hash));
    assert!(replay_queue.txs.contains_key(&live_hash));
    assert_eq!(replay_queue.fifo_snapshot_for_test(), vec![live_hash]);
    assert!(
        replay_queue
            .plan_journal_startup_replay_receipt
            .lock()
            .is_some()
    );
}
