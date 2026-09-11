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
    install_single_validator_topology_for_queue_test(&mut state, 0xA7);
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
    let hash = tx.hash_as_entrypoint();
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
fn empty_replayed_journals_keep_ingress_closed_until_reconciliation_completion() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan_path = dir.path().join("empty-startup-plans.norito");
    let reservation_path = dir.path().join("empty-startup-reservations.norito");
    let mut state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    install_single_validator_topology_for_queue_test(&mut state, 0xD9);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
        lane: LaneId::SINGLE,
        dataspace: DataSpaceId::UNIVERSAL,
    });
    {
        let queue = Queue::test_with_router_for_routes(
            config_factory(),
            &time_source,
            Arc::clone(&router),
            &[],
        );
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("create empty QueuePlan journal");
        assert_eq!(
            queue
                .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
                .expect("create empty reservation journal"),
            LaneQueueReservationReplaySummary::default()
        );
        assert!(queue.lane_reservation_startup_reconciliation_pending());
    }
    let queue = Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        Arc::clone(&router),
        &[],
    );
    assert_eq!(
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("reopen empty reservation journal"),
        LaneQueueReservationReplaySummary::default()
    );
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("reopen empty QueuePlan journal"),
        0
    );
    let tx = accepted_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(&mut state, &tx);
    let hash = tx.hash_as_entrypoint();
    let plan = queue.route_plan_with_state(&tx, &state).expect("route");
    let plan_bytes = std::fs::read(&plan_path).expect("capture empty QueuePlan journal");
    let reservation_bytes =
        std::fs::read(&reservation_path).expect("capture empty reservation journal");
    let snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture empty startup ownership");
    assert!(snapshot.is_empty());
    let assert_ingress_closed = || {
        assert!(queue.lane_reservation_startup_reconciliation_pending());
        let failure = queue
            .push_with_lane_with_state_and_routing_plan_strict_durable(
                tx.clone(),
                &state,
                plan.clone(),
            )
            .expect_err("an empty replay must not admit work before exact startup completion");
        assert!(matches!(
            failure.err,
            Error::PlanJournalDurabilityRejected { ref reason }
                if reason == "queue journal startup is awaiting exact State/Kura reconciliation"
        ));
        assert_eq!(queue.active_len(), 0);
        assert!(queue.txs.is_empty());
        assert!(queue.durable_plan_claims.is_empty());
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("observe unchanged startup ownership"),
            snapshot
        );
        assert_eq!(
            std::fs::read(&plan_path).expect("read QueuePlan journal"),
            plan_bytes
        );
        assert_eq!(
            std::fs::read(&reservation_path).expect("read reservation journal"),
            reservation_bytes
        );
    };
    // HTTP availability cannot grant ingress authority, either before replay or while the
    // immutable State/Kura reconciliation receipt is retained by the startup runner.
    assert_ingress_closed();
    let replay = queue
        .replay_plan_journal(&state)
        .expect("replay empty QueuePlan journal");
    assert_eq!(replay.records, 0);
    assert_eq!(replay.replayed, 0);
    assert_ingress_closed();
    let receipt = queue
        .bind_lane_reservation_startup_reconciliation_receipt(&snapshot)
        .expect("bind exact empty startup receipt")
        .expect("no admission may invalidate the empty replay cut");
    assert_ingress_closed();
    assert!(
        queue
            .revalidate_lane_reservation_startup_reconciliation_receipt(&receipt, &snapshot)
            .expect("early rejected ingress must preserve the exact retained receipt")
    );
    queue
        .complete_lane_reservation_startup_reconciliation(receipt)
        .expect("publish exact empty startup completion");
    assert!(!queue.lane_reservation_startup_reconciliation_pending());
    queue
        .push_with_lane_with_state_and_routing_plan_strict_durable(tx.clone(), &state, plan.clone())
        .expect("the identical transaction is admitted after startup completion");
    assert_eq!(queue.active_len(), 1);
    assert!(queue.txs.contains_key(&hash));
    assert!(queue.durable_plan_claims.contains_key(&hash));
    assert_ne!(
        std::fs::read(&plan_path).expect("read admitted QueuePlan journal"),
        plan_bytes
    );
    assert_eq!(
        std::fs::read(&reservation_path).expect("read unchanged reservation journal"),
        reservation_bytes
    );
    // A second restart restores a real durable FIFO claim. Its lost-response retry must
    // not return or rebind that claim while the exact replay receipt is still retained.
    let admitted_plan_bytes =
        std::fs::read(&plan_path).expect("capture admitted QueuePlan journal");
    drop(queue);
    let restarted = Queue::test_with_router_for_routes(config_factory(), &time_source, router, &[]);
    restarted
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("reopen empty reservation journal with one durable QueuePlan claim");
    assert_eq!(
        restarted
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("reopen retained QueuePlan claim"),
        1
    );
    let replay = restarted
        .replay_plan_journal(&state)
        .expect("restore retained QueuePlan claim");
    assert_eq!(replay.records, 1);
    assert_eq!(replay.replayed, 1);
    let retained_claim = restarted
        .durable_plan_claims
        .get(&hash)
        .expect("retained claim")
        .clone();
    let retained_snapshot = restarted
        .lane_reservation_reconciliation_snapshot()
        .expect("empty restored ownership");
    assert!(retained_snapshot.is_empty());
    let receipt = restarted
        .bind_lane_reservation_startup_reconciliation_receipt(&retained_snapshot)
        .expect("bind replay receipt including retained QueuePlan claim")
        .expect("retained claim is unchanged");
    let failure = restarted
        .push_with_lane_with_state_and_routing_plan_strict_durable(tx.clone(), &state, plan.clone())
        .expect_err("retained durable-claim retry must wait for startup completion");
    assert!(
        matches!(failure.err, Error::PlanJournalDurabilityRejected { ref reason }
        if reason == "queue journal startup is awaiting exact State/Kura reconciliation")
    );
    assert_eq!(
        restarted
            .durable_plan_claims
            .get(&hash)
            .expect("unchanged retained claim")
            .journal_record_digest,
        retained_claim.journal_record_digest
    );
    assert_eq!(
        std::fs::read(&plan_path).expect("read retained QueuePlan journal"),
        admitted_plan_bytes
    );
    assert!(
        restarted
            .revalidate_lane_reservation_startup_reconciliation_receipt(
                &receipt,
                &retained_snapshot
            )
            .expect("rejected retry preserves retained startup receipt")
    );
    restarted
        .complete_lane_reservation_startup_reconciliation(receipt)
        .expect("complete retained-claim startup");
    restarted
        .push_with_lane_with_state_and_routing_plan_strict_durable(tx, &state, plan)
        .expect("retained durable claim becomes retryable after startup completion");
    assert_eq!(
        restarted
            .durable_plan_claims
            .get(&hash)
            .expect("retried retained claim")
            .journal_record_digest,
        retained_claim.journal_record_digest
    );
    assert_eq!(
        std::fs::read(&plan_path).expect("read retried QueuePlan journal"),
        admitted_plan_bytes
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
    install_single_validator_topology_for_queue_test(&mut state, 0xD7);
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
    let hash = tx.hash_as_entrypoint();
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
    install_single_validator_topology_for_queue_test(&mut state, 0xD8);
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
    let hash = tx.hash_as_entrypoint();
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
    install_single_validator_topology_for_queue_test(&mut state, 0xD9);
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
    let terminal_hash = terminal.hash_as_entrypoint();
    let live_hash = live.hash_as_entrypoint();
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
