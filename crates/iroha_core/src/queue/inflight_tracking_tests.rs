#[test]
fn queued_len_excludes_inflight_transactions() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let mut cfg = config_factory();
    cfg.capacity = nonzero!(2_usize);
    cfg.capacity_per_user = nonzero!(2_usize);
    cfg.transaction_time_to_live = Duration::from_secs(100);

    let queue = Arc::new(Queue::test(cfg, &time_source));
    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("push succeeds");
    assert_eq!(queue.queued_len(), 1, "queued count before pop");
    queue.assert_pressure_counters_consistent_for_tests();

    let mut expired = Vec::new();
    let guard = queue
        .pop_from_queue(&state.view(), &mut expired)
        .expect("pop should return a transaction guard");
    assert!(expired.is_empty());
    assert_eq!(queue.queued_len(), 0, "hash queue empty after pop");
    assert_eq!(queue.active_len(), 1, "active count includes in-flight");
    assert_eq!(queue.queued_len(), 0, "queued count excludes in-flight");
    queue.assert_pressure_counters_consistent_for_tests();
    drop(guard);
    assert_eq!(
        queue.active_len(),
        0,
        "active count clears after guard drop"
    );
    queue.assert_pressure_counters_consistent_for_tests();
}

#[test]
fn inflight_cull_clears_removed_hash_marker_on_drop() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let mut cfg = config_factory();
    cfg.transaction_time_to_live = Duration::from_millis(1);
    cfg.expired_cull_interval = Duration::from_millis(1);

    let queue = Arc::new(Queue::test(cfg, &time_source));
    queue
        .push(accepted_tx_by_someone(&time_source), state.view())
        .expect("push succeeds");

    let mut expired = Vec::new();
    let guard = queue
        .pop_from_queue(&state.view(), &mut expired)
        .expect("pop should return a transaction guard");
    assert!(expired.is_empty());

    time_handle.advance(Duration::from_millis(2));
    let culled = queue.cull_expired_entries_if_due();
    assert_eq!(culled, 1, "expired in-flight tx should be culled");
    assert_eq!(
        queue.active_len(),
        0,
        "active count reflects the culled in-flight entry"
    );
    assert!(
        !queue.removed_hashes.is_empty(),
        "removed hash marker should be set after cull"
    );

    drop(guard);
    assert!(
        queue.removed_hashes.is_empty(),
        "removed hash marker should be cleared on guard drop"
    );
}

#[test]
fn remove_committed_hashes_tolerates_missing_per_user_counter() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());

    let queue = Queue::test(config_factory(), &time_source);
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash();
    let authority = tx.as_ref().authority().clone();
    queue.push(tx, state.view()).expect("push succeeds");
    assert_eq!(queue.queued_tx_count_for_user(&authority), 1);

    queue.txs_per_user.clear();
    let removed = queue.remove_committed_hashes([hash], None);

    assert_eq!(removed, 1, "committed hash should still be removed");
    assert_eq!(queue.active_len(), 0, "queue no longer tracks tx");
    assert_eq!(queue.queued_tx_count_for_user(&authority), 0);
    queue.assert_pressure_counters_consistent_for_tests();
}
