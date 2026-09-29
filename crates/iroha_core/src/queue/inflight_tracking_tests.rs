#[test]
fn remove_committed_hashes_tolerates_missing_per_user_counter() {
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = Arc::new(State::new(world_with_test_domains(), kura, query_handle));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(config_factory(), &time_source);
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash_as_entrypoint();
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
