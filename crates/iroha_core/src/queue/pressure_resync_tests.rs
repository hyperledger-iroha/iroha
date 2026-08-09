// Queue pressure, resynchronization, and requeue regression tests.

#[test]
fn lane_limits_respect_metadata_overrides() {
    let fallback = LaneSchedulingLimits::new(6_000, 120);
    let mut lane = LaneConfig {
        id: LaneId::new(3),
        alias: "custom".to_string(),
        metadata: BTreeMap::from([
            ("scheduler.teu_capacity".to_string(), "8192".to_string()),
            (
                "scheduler.starvation_bound_slots".to_string(),
                "42".to_string(),
            ),
        ]),
        ..LaneConfig::default()
    };
    let limits = QueueLimits::lane_limits_from_metadata(&lane, fallback);
    assert_eq!(limits.teu_capacity, 8_192);
    assert_eq!(limits.starvation_bound_slots, 42);

    lane.metadata.insert(
        "scheduler.teu_capacity".to_string(),
        "not-a-number".to_string(),
    );
    lane.metadata.insert(
        "scheduler.starvation_bound_slots".to_string(),
        "NaN".to_string(),
    );
    let limits = QueueLimits::lane_limits_from_metadata(&lane, fallback);
    assert_eq!(limits, fallback);
}

#[tokio::test]
async fn overweight_tx_not_starved_by_lane_teu_limit() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let time_source = TimeSource::new_system();

    let tx = accepted_tx_by_someone(&time_source);
    let teu = Queue::compute_teu_weight(&tx);
    assert!(
        teu > 1,
        "baseline transaction should carry a non-trivial TEU weight (got {teu})"
    );
    let lane_cap = teu.saturating_sub(1);
    let limits = QueueLimits {
        fallback: LaneSchedulingLimits::new(lane_cap, 0),
        per_lane: BTreeMap::new(),
    };
    let queue = Queue::test(config_factory(), &time_source);
    *queue.nexus_limits.write() = limits;
    let queue = Arc::new(queue);

    queue
        .push(tx, state.view())
        .expect("overweight tx should still be enqueued");

    let mut guards = Vec::new();
    queue.get_transactions_for_block(&state.view(), nonzero!(1_usize), &mut guards);
    assert_eq!(
        guards.len(),
        1,
        "first overweight transaction must not be indefinitely deferred"
    );
}

#[tokio::test]
async fn backpressure_state_tracks_queue_load() {
    let capacity = nonzero!(2_usize);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Arc::new(Queue::test(
        Config {
            capacity,
            ..config_factory()
        },
        &time_source,
    ));
    let mut rx = queue.backpressure_handle().subscribe();

    assert!(!rx.borrow().is_saturated());

    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("first push succeeds");
    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("second push reaches capacity");

    rx.changed().await.expect("backpressure update to saturate");
    assert!(rx.borrow().is_saturated());

    let mut expired = Vec::new();
    let guard = queue
        .pop_from_queue(&state.view(), &mut expired)
        .expect("transaction available");
    drop(guard);

    rx.changed().await.expect("backpressure update to healthy");
    assert!(!rx.borrow().is_saturated());
}

#[tokio::test]
async fn queue_pressure_snapshot_tracks_oldest_age_across_enqueue_and_dequeue() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Arc::new(Queue::test(config_factory(), &time_source));

    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("first push succeeds");
    time_handle.advance(Duration::from_millis(10));
    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("second push succeeds");

    let initial = queue.pressure_snapshot();
    assert_eq!(initial.tracked_tx_count, 2);
    assert_eq!(initial.queued_tx_count, 2);
    assert_eq!(initial.oldest_queued_tx_age_ms, 10);

    let mut expired = Vec::new();
    let guard = queue
        .pop_from_queue(&state.view(), &mut expired)
        .expect("transaction available");
    let inflight = queue.pressure_snapshot();
    assert_eq!(inflight.tracked_tx_count, 2);
    assert_eq!(inflight.queued_tx_count, 1);
    assert_eq!(inflight.oldest_queued_tx_age_ms, 0);

    drop(guard);

    let after_drop = queue.pressure_snapshot();
    assert_eq!(after_drop.tracked_tx_count, 1);
    assert_eq!(after_drop.queued_tx_count, 1);
    assert_eq!(after_drop.oldest_queued_tx_age_ms, 0);
}

#[tokio::test]
async fn queue_pressure_snapshot_clears_oldest_age_after_expiry() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(
        Config {
            transaction_time_to_live: Duration::from_millis(1),
            expired_cull_interval: Duration::from_millis(1),
            ..config_factory()
        },
        &time_source,
    );

    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("push succeeds");
    assert_eq!(queue.pressure_snapshot().oldest_queued_tx_age_ms, 0);

    time_handle.advance(Duration::from_millis(2));
    assert_eq!(queue.cull_expired_entries_if_due(), 1);

    let snapshot = queue.pressure_snapshot();
    assert_eq!(snapshot.tracked_tx_count, 0);
    assert_eq!(snapshot.queued_tx_count, 0);
    assert_eq!(snapshot.oldest_queued_tx_age_ms, 0);
}

#[tokio::test]
async fn backpressure_state_ignores_oldest_queue_age_without_capacity_pressure() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    queue.set_pressure_age_budget_for_tests(Duration::from_millis(5));

    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("push succeeds");
    time_handle.advance(Duration::from_millis(6));

    let snapshot = queue.pressure_snapshot();
    assert!(!snapshot.saturated_by_count);
    assert!(snapshot.saturated_by_age);
    assert!(!queue.current_backpressure().is_saturated());
}

#[tokio::test]
async fn backdate_queued_transactions_for_tests_updates_age_pressure() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(10));

    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    queue.set_pressure_age_budget_for_tests(Duration::from_millis(5));
    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("push succeeds");

    let snapshot = queue.backdate_queued_transactions_for_tests(Duration::from_millis(6));

    assert_eq!(snapshot.oldest_queued_tx_age_ms, 6);
    assert!(snapshot.saturated_by_age);
    assert!(!queue.current_backpressure().is_saturated());
}

#[tokio::test]
async fn backpressure_state_excludes_inflight_transactions_from_age_and_depth() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    queue.set_pressure_age_budget_for_tests(Duration::from_millis(5));

    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("push succeeds");

    let mut guards = Vec::new();
    queue.get_transactions_for_block(&state.view(), nonzero!(1_usize), &mut guards);
    assert_eq!(guards.len(), 1, "queue should return an in-flight guard");

    time_handle.advance(Duration::from_millis(6));
    let snapshot = queue.pressure_snapshot();
    assert_eq!(snapshot.tracked_tx_count, 1);
    assert_eq!(snapshot.queued_tx_count, 0);
    assert_eq!(snapshot.oldest_queued_tx_age_ms, 0);
    assert!(!snapshot.saturated_by_age);
    assert_eq!(queue.current_backpressure().queued(), 0);

    drop(guards);
    assert_eq!(queue.current_backpressure().queued(), 0);
}

#[tokio::test]
async fn queue_pressure_counters_track_committed_removal_before_hash_drain() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let first = accepted_tx_by_someone(&time_source);
    let first_hash = first.as_ref().hash();
    queue
        .push(first, state.view())
        .expect("first push succeeds");
    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("second push succeeds");

    assert_eq!(queue.active_len(), 2);
    assert_eq!(queue.queued_len(), 2);
    queue.assert_pressure_counters_consistent_for_tests();

    assert_eq!(queue.remove_committed_hashes([first_hash], None), 1);

    let snapshot = queue.pressure_snapshot();
    assert_eq!(snapshot.tracked_tx_count, 1);
    assert_eq!(snapshot.queued_tx_count, 1);
    assert_eq!(queue.active_len(), queue.txs.len());
    assert_eq!(queue.queued_len(), queue.queued_tx_enqueued_at_ms.len());
    queue.assert_pressure_counters_consistent_for_tests();
}

#[tokio::test]
async fn queue_pressure_counters_restore_age_after_enqueue_compaction_retry() {
    let capacity = nonzero!(1_usize);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Arc::new(Queue::test(
        Config {
            capacity,
            ..config_factory()
        },
        &time_source,
    ));
    let first = accepted_tx_by_someone(&time_source);
    let first_hash = first.as_ref().hash();
    queue
        .push(first, state.view())
        .expect("first push succeeds");
    assert_eq!(queue.remove_committed_hashes([first_hash], None), 1);
    assert_eq!(queue.active_len(), 0);
    assert_eq!(queue.queued_len(), 0);
    queue.assert_pressure_counters_consistent_for_tests();

    let second = accepted_tx_by_someone(&time_source);
    let second_hash = second.as_ref().hash();
    queue
        .push(second, state.view())
        .expect("second push compacts stale hash and succeeds");

    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 1);
    assert!(queue.queued_tx_enqueued_at_ms.contains_key(&second_hash));
    assert_eq!(queue.pressure_snapshot().queued_tx_count, 1);
    queue.assert_pressure_counters_consistent_for_tests();
}

#[tokio::test]
async fn queue_pressure_counters_stay_consistent_under_sustained_backlog() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Arc::new(Queue::test(
        Config {
            capacity: nonzero!(64_usize),
            ..config_factory()
        },
        &time_source,
    ));
    let target_backlog = 8usize;

    for _ in 0..target_backlog {
        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("prefill push succeeds");
        time_handle.advance(Duration::from_millis(1));
    }
    queue.assert_pressure_counters_consistent_for_tests();
    assert_eq!(queue.pressure_snapshot().queued_tx_count, target_backlog);

    for _ in 0..32 {
        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("sustained push succeeds");
        queue.assert_pressure_counters_consistent_for_tests();

        let mut guards = Vec::new();
        queue.get_transactions_for_block(&state.view(), nonzero!(1_usize), &mut guards);
        assert_eq!(guards.len(), 1, "one transaction should leave the queue");

        let inflight = queue.pressure_snapshot();
        assert_eq!(inflight.tracked_tx_count, target_backlog + 1);
        assert_eq!(inflight.queued_tx_count, target_backlog);
        queue.assert_pressure_counters_consistent_for_tests();

        drop(guards);
        let after_drop = queue.pressure_snapshot();
        assert_eq!(after_drop.tracked_tx_count, target_backlog);
        assert_eq!(after_drop.queued_tx_count, target_backlog);
        queue.assert_pressure_counters_consistent_for_tests();

        time_handle.advance(Duration::from_millis(1));
    }
}

#[tokio::test]
async fn resync_rebuilds_hash_queue_when_empty() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let mut admitted_hashes = Vec::new();
    for _ in 0..3 {
        let tx = accepted_tx_by_someone(&time_source);
        admitted_hashes.push(tx.as_ref().hash());
        queue.push(tx, state.view()).expect("push succeeds");
    }

    while queue.tx_hashes.pop().is_some() {}
    assert_eq!(queue.queued_len(), 0, "hash queue should be empty");
    assert_eq!(queue.txs.len(), 3, "tx map retains queued entries");

    let mut guards = Vec::new();
    queue.get_transactions_for_block(&state.view(), nonzero!(2_usize), &mut guards);
    assert_eq!(guards.len(), 2, "resync should repopulate hashes");
    assert_eq!(
        guards
            .iter()
            .map(|guard| guard.as_ref().hash())
            .collect::<Vec<_>>(),
        admitted_hashes[..2],
        "empty-index recovery must preserve admitted FIFO order"
    );
    drop(guards);

    assert_eq!(
        queue.queued_len(),
        1,
        "remaining queue size should be tracked"
    );
    queue.assert_pressure_counters_consistent_for_tests();
}

#[test]
fn resync_fails_closed_without_a_complete_unique_durable_fifo_index() {
    let state = Arc::new(State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    for corrupt in ["missing", "duplicate"] {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        let first = accepted_tx_by_someone(&time_source);
        let first_hash = first.as_ref().hash();
        queue.push(first, state.view()).expect("push first");
        let second = accepted_tx_by_someone(&time_source);
        let second_hash = second.as_ref().hash();
        queue.push(second, state.view()).expect("push second");

        match corrupt {
            "missing" => {
                queue.fifo_order_by_hash.remove(&second_hash);
            }
            "duplicate" => {
                let first_order = *queue
                    .fifo_order_by_hash
                    .get(&first_hash)
                    .expect("first FIFO order");
                queue.fifo_order_by_hash.insert(second_hash, first_order);
            }
            _ => unreachable!(),
        }
        while queue.tx_hashes.pop().is_some() {}

        let mut guards = Vec::new();
        queue.get_transactions_for_block(&state.view(), nonzero!(2_usize), &mut guards);

        assert!(guards.is_empty(), "{corrupt} FIFO index admitted work");
        assert_eq!(queue.active_len(), 2, "{corrupt} FIFO index lost ownership");
        assert!(queue.txs.contains_key(&first_hash));
        assert!(queue.txs.contains_key(&second_hash));
        assert!(
            queue.accepted_work_validation_faulted(),
            "{corrupt} FIFO index did not latch a fail-closed fault"
        );
        assert!(
            queue.tx_hashes.is_empty(),
            "{corrupt} FIFO index caused a partial rebuild"
        );
    }
}

#[tokio::test]
async fn get_transactions_for_block_with_state_rebuilds_hash_queue_when_empty() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    for _ in 0..3 {
        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("push succeeds");
    }

    while queue.tx_hashes.pop().is_some() {}
    assert_eq!(queue.queued_len(), 0, "hash queue should be empty");
    assert_eq!(queue.txs.len(), 3, "tx map retains queued entries");

    let mut guards = Vec::new();
    queue.get_transactions_for_block_with_state(state.as_ref(), nonzero!(2_usize), &mut guards);
    assert_eq!(guards.len(), 2, "resync should repopulate hashes");
    drop(guards);

    assert_eq!(
        queue.queued_len(),
        1,
        "remaining queue size should be tracked"
    );
    queue.assert_pressure_counters_consistent_for_tests();
}

#[tokio::test]
async fn resync_skips_when_guards_inflight() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("push succeeds");

    let mut guards = Vec::new();
    queue.get_transactions_for_block(&state.view(), nonzero!(1_usize), &mut guards);
    assert_eq!(guards.len(), 1, "expected one in-flight guard");
    assert_eq!(
        queue.queued_len(),
        0,
        "hash queue should be empty while guard is held"
    );

    let mut extra = Vec::new();
    queue.get_transactions_for_block(&state.view(), nonzero!(1_usize), &mut extra);
    assert!(
        extra.is_empty(),
        "resync should be skipped while guard is in flight"
    );

    drop(guards);
}

#[tokio::test]
async fn get_available_txs() {
    let max_txs_in_block = nonzero!(2_usize);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));

    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test(
        Config {
            transaction_time_to_live: Duration::from_secs(100),
            ..config_factory()
        },
        &time_source,
    );
    let queue = Arc::new(queue);
    for _ in 0..5 {
        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("Failed to push tx into queue");
        time_handle.advance(Duration::from_millis(10));
    }

    let available = queue.collect_transactions_for_block(&state.view(), max_txs_in_block);
    assert_eq!(available.len(), max_txs_in_block.get());
}

#[tokio::test]
async fn transaction_guard_clone_accepted_returns_owned_transaction() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Arc::new(Queue::test(config_factory(), &time_source));

    let tx = accepted_tx_by_someone(&time_source);
    let expected_hash = tx.as_ref().hash();
    queue
        .push(tx, state.view())
        .expect("Failed to push tx into queue");

    let mut expired = Vec::new();
    let guard = queue
        .pop_from_queue(&state.view(), &mut expired)
        .expect("Expected guard from queue");
    assert!(expired.is_empty());

    let guard_hash = guard.as_ref().hash();
    assert_eq!(guard_hash, expected_hash);

    let cloned = guard.clone_accepted();
    assert_eq!(cloned.as_ref().hash(), guard_hash);

    drop(guard);

    assert_eq!(queue.queued_len(), 0);
    assert_eq!(cloned.as_ref().hash(), guard_hash);
}

#[tokio::test]
async fn push_tx_already_in_blockchain() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let tx = accepted_tx_by_someone(&time_source);
    let (_, private_key) = checked_random_queue_keypair().into_parts();
    let unverified_block: SignedBlock =
        ValidBlock::new_dummy_and_modify_header(&private_key, |header| {
            header.height = nonzero!(1_u64);
        })
        .into();
    let mut state_block = state.block(unverified_block.header());
    let block_height: NonZeroUsize = unverified_block
        .header()
        .height()
        .try_into()
        .expect("block height should fit into usize");
    state_block
        .transactions
        .insert_block_with_single_tx(tx.as_ref().hash(), block_height);
    state_block.commit().unwrap();
    let queue = Queue::test(config_factory(), &time_source);
    assert!(matches!(
        queue.push(tx, state.view()),
        Err(Failure {
            err: Error::InBlockchain,
            ..
        })
    ));
    assert_eq!(queue.txs.len(), 0);
}

#[tokio::test]
async fn push_requeued_with_routing_plan_accepts_pending_transaction() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    install_active_single_lane_nexus(&state);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash();
    let routing = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);

    queue
        .push_requeued_with_routing_plan(tx, RoutingPlan::single(routing), &state)
        .expect("requeue push succeeds");

    assert_eq!(queue.txs.len(), 1);
    assert_eq!(
        queue
            .routing_decisions
            .get(&hash)
            .map(|entry| *entry.value()),
        Some(routing)
    );
    assert!(
        queue.tx_gossip.pop().is_none(),
        "the batch owner must enqueue consensus-requeue gossip exactly once"
    );
    queue.requeue_gossip_hashes([hash]);
    assert_eq!(queue.tx_gossip.pop(), Some(hash));

    let mut expired = Vec::new();
    let guard = queue
        .pop_from_queue(&state.view(), &mut expired)
        .expect("queued tx should be available");
    assert!(expired.is_empty());
    assert_eq!(guard.as_ref().hash(), hash);
    assert_eq!(guard.routing(), routing);
    drop(guard);
}

#[test]
fn push_requeued_with_routing_plan_rejects_native_amx_participant_drift() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let fixture = native_amx_participant_drift_fixture(&time_source);
    assert_eq!(
        fixture.stale_plan.coordinator_route(),
        fixture.current_plan.coordinator_route()
    );
    assert_ne!(fixture.stale_plan.digest(), fixture.current_plan.digest());

    let queue = Queue::test(config_factory(), &time_source);
    let hash = fixture.tx.hash();
    let err = queue
        .push_requeued_with_routing_plan(fixture.tx.clone(), fixture.stale_plan, &fixture.state)
        .expect_err("requeue must reject stale Native AMX participant legs");

    assert!(
        matches!(
            &err,
            Failure {
                err: Error::UnresolvedRoute { .. },
                ..
            }
        ),
        "unexpected requeue rejection: {err:?}"
    );
    if let Error::UnresolvedRoute { reason } = &err.err {
        assert!(
            reason.contains("does not match the current Nexus routing policy")
                || reason.contains("not active"),
            "stale-plan rejection should explain current-policy or active-height mismatch: {reason}"
        );
    }
    assert_eq!(
        queue
            .route_plan_with_state(&fixture.tx, &fixture.state)
            .expect("current Native AMX plan should resolve"),
        fixture.current_plan
    );
    assert!(!queue.txs.contains_key(&hash));
    assert!(queue.routing_decisions.get(&hash).is_none());
    assert!(queue.routing_plans.get(&hash).is_none());
    assert_eq!(queue.active_len(), 0);
    assert_eq!(queue.queued_len(), 0);
    assert!(
        queue.tx_gossip.pop().is_none(),
        "rejected requeue must not publish gossip notifications"
    );
}

#[tokio::test]
async fn push_requeued_with_routing_plan_rejects_committed_transaction() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let tx = accepted_tx_by_someone(&time_source);
    let (_, private_key) = checked_random_queue_keypair().into_parts();
    let unverified_block: SignedBlock =
        ValidBlock::new_dummy_and_modify_header(&private_key, |header| {
            header.height = nonzero!(1_u64);
        })
        .into();
    let mut state_block = state.block(unverified_block.header());
    let block_height: NonZeroUsize = unverified_block
        .header()
        .height()
        .try_into()
        .expect("block height should fit into usize");
    state_block
        .transactions
        .insert_block_with_single_tx(tx.as_ref().hash(), block_height);
    state_block.commit().unwrap();
    let queue = Queue::test(config_factory(), &time_source);
    let routing = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);

    assert!(matches!(
        queue.push_requeued_with_routing_plan(tx, RoutingPlan::single(routing), &state),
        Err(Failure {
            err: Error::InBlockchain,
            ..
        })
    ));
    assert_eq!(queue.txs.len(), 0);
}

#[tokio::test]
async fn push_expired_tx_already_in_blockchain() {
    let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

    let (alice_id, alice_keypair) = gen_account_in("wonderland");

    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (max_clock_drift, tx_limits) = {
        let state_view = state.world.view();
        let params = state_view.parameters();
        (params.sumeragi().max_clock_drift(), params.transaction())
    };
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let ok_instruction = Log::new(iroha_logger::Level::INFO, "pass".into());
    let mut tx = TransactionBuilder::new_with_time_source(
        chain_id.clone(),
        alice_id,
        &time_source,
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([ok_instruction]);
    tx.set_ttl(Duration::from_millis(100));
    let tx = tx.sign(alice_keypair.private_key());
    let tx = {
        let crypto_cfg = state.crypto();
        AcceptedTransaction::accept_with_time_source(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            &crypto_cfg,
            &time_source,
        )
        .expect("Failed to accept Transaction.")
    };

    let (_, private_key) = checked_random_queue_keypair().into_parts();
    let unverified_block: SignedBlock =
        ValidBlock::new_dummy_and_modify_header(&private_key, |header| {
            header.height = nonzero!(1_u64);
        })
        .into();
    let mut state_block = state.block(unverified_block.header());
    let block_height: NonZeroUsize = unverified_block
        .header()
        .height()
        .try_into()
        .expect("block height should fit into usize");
    state_block
        .transactions
        .insert_block_with_single_tx(tx.as_ref().hash(), block_height);
    state_block.commit().unwrap();
    let queue = Queue::test(config_factory(), &time_source);
    time_handle.advance(Duration::from_secs(100));
    assert!(matches!(
        queue.push(tx, state.view()),
        Err(Failure {
            err: Error::InBlockchain,
            ..
        })
    ));
    assert_eq!(queue.txs.len(), 0);
}

#[tokio::test]
async fn get_tx_drop_if_in_blockchain() {
    let max_txs_in_block = nonzero!(2_usize);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new(world_with_test_domains(), kura, query_handle);
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let tx = accepted_tx_by_someone(&time_source);
    let tx_hash = tx.as_ref().hash();
    let queue = Queue::test(config_factory(), &time_source);
    let queue = Arc::new(queue);
    queue.push(tx, state.view()).unwrap();
    let (_, private_key) = checked_random_queue_keypair().into_parts();
    let unverified_block: SignedBlock =
        ValidBlock::new_dummy_and_modify_header(&private_key, |header| {
            header.height = nonzero!(1_u64);
        })
        .into();
    let mut state_block = state.block(unverified_block.header());
    let block_height: NonZeroUsize = unverified_block
        .header()
        .height()
        .try_into()
        .expect("block height should fit into usize");
    state_block
        .transactions
        .insert_block_with_single_tx(tx_hash, block_height);
    state_block.commit().unwrap();
    assert_eq!(
        queue
            .collect_transactions_for_block(&state.view(), max_txs_in_block)
            .len(),
        0
    );
    assert_eq!(queue.txs.len(), 0);
}

#[tokio::test]
async fn get_available_txs_with_timeout() {
    let max_txs_in_block = nonzero!(6_usize);
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));

    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test(
        Config {
            transaction_time_to_live: Duration::from_millis(200),
            ..config_factory()
        },
        &time_source,
    );
    let queue = Arc::new(queue);
    for _ in 0..(max_txs_in_block.get() - 1) {
        queue
            .push(accepted_tx_by_someone(&time_source), state.view())
            .expect("Failed to push tx into queue");
        time_handle.advance(Duration::from_millis(100));
    }

    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("Failed to push tx into queue");
    time_handle.advance(Duration::from_millis(101));
    assert_eq!(
        queue
            .collect_transactions_for_block(&state.view(), max_txs_in_block)
            .len(),
        1
    );

    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("Failed to push tx into queue");
    time_handle.advance(Duration::from_millis(210));
    assert_eq!(
        queue
            .collect_transactions_for_block(&state.view(), max_txs_in_block)
            .len(),
        0
    );
}
