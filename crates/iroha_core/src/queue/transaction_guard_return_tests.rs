// Transaction-guard return capacity, accounting, idempotence, and expiry regressions.
//
// Included by `queue::tests` so source-bound libtest names remain stable.
#[test]
fn repeated_guard_returns_keep_one_age_entry_and_original_age() {
    let state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash_as_entrypoint();
    queue.push(tx, state.view()).expect("push transaction");
    let original_enqueued_at_ms = queue
        .tx_enqueued_at_ms
        .get(&hash)
        .map(|entry| *entry.value())
        .expect("original enqueue timestamp");
    time_handle.advance(Duration::from_millis(37));
    for _ in 0..128 {
        let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(1_usize));
        assert_eq!(guards.len(), 1);
        assert_eq!(
            queue
                .return_transaction_guards(&mut guards, &state)
                .expect("return guard")
                .returned,
            1
        );
        assert_eq!(queue.queued_len(), 1);
        assert_eq!(queue.queued_tx_enqueued_at_ms.len(), 1);
        assert_eq!(
            queue.queued_age_ring.lock().len(),
            1,
            "pop/return retries must not accumulate duplicate live age entries"
        );
    }
    assert_eq!(
        queue
            .queued_tx_enqueued_at_ms
            .get(&hash)
            .map(|entry| *entry.value()),
        Some(original_enqueued_at_ms)
    );
    assert_eq!(
        queue.oldest_queued_tx_age_ms(),
        37,
        "return retries must preserve the original queue residence age"
    );
    assert_eq!(
        queue
            .queued_age_ring
            .lock()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![(hash, original_enqueued_at_ms)]
    );
}
#[test]
fn guard_return_keeps_capacity_reserved_against_concurrent_admission() {
    let state = Arc::new(State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    ));
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut cfg = config_factory();
    cfg.capacity = nonzero!(2_usize);
    cfg.capacity_per_user = nonzero!(2_usize);
    let queue = Arc::new(Queue::test(cfg, &time_source));
    let originals = [
        accepted_tx_by_someone(&time_source),
        accepted_tx_by_someone(&time_source),
    ];
    let original_hashes = originals
        .iter()
        .map(|tx| tx.as_ref().hash_as_entrypoint())
        .collect::<BTreeSet<_>>();
    for tx in originals {
        queue.push(tx, state.view()).expect("fill queue");
    }
    let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(2_usize));
    assert_eq!(guards.len(), 2);
    assert_eq!(queue.active_len(), 2, "guards retain count reservations");
    let contender_count = 8usize;
    let barrier = Arc::new(std::sync::Barrier::new(contender_count + 1));
    let contenders = (0..contender_count)
        .map(|_| {
            let queue = Arc::clone(&queue);
            let state = Arc::clone(&state);
            let barrier = Arc::clone(&barrier);
            let tx = accepted_tx_by_someone(&time_source);
            thread::spawn(move || {
                barrier.wait();
                queue.push(tx, state.view())
            })
        })
        .collect::<Vec<_>>();
    barrier.wait();
    let report = queue
        .return_transaction_guards(&mut guards, state.as_ref())
        .expect("atomically return reserved guards");
    assert_eq!(report.returned, 2);
    for contender in contenders {
        let failure = contender
            .join()
            .expect("contender thread")
            .expect_err("in-flight reservations must prevent capacity stealing");
        assert!(matches!(failure.err, Error::Full));
    }
    assert_eq!(queue.active_len(), 2);
    assert_eq!(queue.queued_len(), 2);
    assert_eq!(
        queue
            .txs
            .iter()
            .map(|entry| *entry.key())
            .collect::<BTreeSet<_>>(),
        original_hashes
    );
    assert_eq!(queue.inflight_guards.load(Ordering::Relaxed), 0);
}
#[test]
#[allow(clippy::too_many_lines)]
fn guard_return_committed_disposition_clears_accounting_and_durable_metadata() {
    let dir = tempfile::tempdir().expect("tempdir");
    let journal_path = dir.path().join("committed_guard_return_journal.norito");
    let state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    queue
        .install_plan_journal(&journal_path, 1024 * 1024, true)
        .expect("install queue plan journal");
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash_as_entrypoint();
    let authority = tx.as_ref().authority().clone();
    queue.push(tx, state.view()).expect("push committed tx");
    let retained_bytes_before = queue.retained_bytes();
    assert!(retained_bytes_before > 0);
    assert_eq!(queue.queued_tx_count_for_user(&authority), 1);
    assert!(queue.routing_plans.contains_key(&hash));
    assert!(queue.routing_plan_hint(&hash).is_some());
    assert!(queue.expiry_ring_members.contains_key(&hash));
    assert_eq!(
        queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed journal")
            .live_record_count()
            .expect("count live journal records"),
        1
    );
    #[cfg(feature = "telemetry")]
    let terminal_teu = queue
        .tx_teu
        .get(&hash)
        .map(|entry| *entry.value())
        .expect("committed transaction TEU metadata");
    let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(1_usize));
    assert_eq!(guards.len(), 1);
    assert_eq!(queue.inflight_guards.load(Ordering::Relaxed), 1);
    assert_eq!(queue.retained_bytes(), retained_bytes_before);
    assert_eq!(queue.queued_tx_count_for_user(&authority), 1);
    assert!(queue.expiry_ring_members.contains_key(&hash));
    {
        let mut transactions = state.transactions.block();
        transactions.insert_block_with_single_tx(hash, nonzero!(1_usize));
        transactions.commit().expect("commit transaction index");
    }
    let report = queue
        .return_transaction_guards(&mut guards, &state)
        .expect("settle committed guard");
    assert_eq!(
        report,
        TransactionGuardReturnReport {
            committed: 1,
            ..TransactionGuardReturnReport::default()
        }
    );
    assert!(guards.is_empty());
    assert_eq!(queue.active_len(), 0);
    assert_eq!(queue.queued_len(), 0);
    assert_eq!(queue.inflight_guards.load(Ordering::Relaxed), 0);
    assert!(!queue.queued_tx_enqueued_at_ms.contains_key(&hash));
    assert!(
        queue
            .queued_age_ring
            .lock()
            .iter()
            .all(|(queued_hash, _)| *queued_hash != hash),
        "committed guard return must remove its lazy age entry"
    );
    assert_eq!(queue.retained_bytes(), 0);
    assert_eq!(queue.queued_tx_count_for_user(&authority), 0);
    assert!(!queue.txs.contains_key(&hash));
    assert!(!queue.routing_plans.contains_key(&hash));
    assert!(queue.routing_plan_hint(&hash).is_none());
    assert!(!queue.expiry_ring_members.contains_key(&hash));
    assert_eq!(
        queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed journal")
            .live_record_count()
            .expect("count tombstoned journal records"),
        0,
        "committed guard return must tombstone its durable routing plan"
    );
    #[cfg(feature = "telemetry")]
    {
        assert!(!queue.tx_teu.contains_key(&hash));
        assert_eq!(
            queue
                .lane_teu_pending
                .get(&terminal_teu.lane_id)
                .map(|pending| (pending.teu, pending.tx_count))
                .unwrap_or_default(),
            (0, 0)
        );
        assert_eq!(
            queue
                .dataspace_teu_pending
                .get(&(terminal_teu.lane_id, terminal_teu.dataspace_id))
                .map(|pending| (pending.teu, pending.tx_count))
                .unwrap_or_default(),
            (0, 0)
        );
    }
}
#[test]
fn guard_return_is_idempotent_across_committed_and_already_queued_races() {
    let state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let committed_tx = accepted_tx_by_someone(&time_source);
    let committed_hash = committed_tx.as_ref().hash_as_entrypoint();
    let already_queued_tx = accepted_tx_by_someone(&time_source);
    let already_queued_hash = already_queued_tx.as_ref().hash_as_entrypoint();
    queue
        .push(committed_tx, state.view())
        .expect("push committed-race tx");
    queue
        .push(already_queued_tx, state.view())
        .expect("push already-queued tx");
    let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(2_usize));
    assert_eq!(guards.len(), 2);
    {
        let mut transactions = state.transactions.block();
        transactions.insert_block_with_single_tx(committed_hash, nonzero!(1_usize));
        transactions.commit().expect("commit transaction index");
    }
    assert_eq!(
        queue.remove_committed_hashes(std::iter::once(committed_hash), None),
        1
    );
    let already_queued_at = queue
        .tx_enqueued_at_ms
        .get(&already_queued_hash)
        .map(|entry| *entry.value())
        .expect("already-queued timestamp");
    assert!(queue.push_queued_hash(already_queued_hash, already_queued_at));
    let report = queue
        .return_transaction_guards(&mut guards, &state)
        .expect("settle committed and already-queued guards");
    assert_eq!(report.committed, 1);
    assert_eq!(report.already_queued, 1);
    assert_eq!(report.returned, 0);
    assert!(!queue.txs.contains_key(&committed_hash));
    assert!(queue.txs.contains_key(&already_queued_hash));
    assert_eq!(queue.queued_len(), 1);
    assert_eq!(queue.queued_tx_enqueued_at_ms.len(), 1);
    assert_eq!(
        queue
            .queued_age_ring
            .lock()
            .iter()
            .filter(|(hash, _)| *hash == already_queued_hash)
            .count(),
        1,
        "idempotent already-queued return must canonicalize duplicate age entries"
    );
    assert_eq!(queue.inflight_guards.load(Ordering::Relaxed), 0);
    let mut expired = Vec::new();
    let only = queue
        .pop_from_queue(&state.view(), &mut expired)
        .expect("single idempotently queued guard");
    assert_eq!(only.tx.hash_as_entrypoint(), already_queued_hash);
    drop(only);
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.as_ref().hash_as_entrypoint();
    queue
        .push(tx, state.view())
        .expect("push return-before-commit tx");
    let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(1_usize));
    assert_eq!(
        queue
            .return_transaction_guards(&mut guards, &state)
            .expect("return before commit")
            .returned,
        1
    );
    {
        let mut transactions = state.transactions.block();
        transactions.insert_block_with_single_tx(hash, nonzero!(2_usize));
        transactions.commit().expect("commit returned transaction");
    }
    assert_eq!(
        queue.remove_committed_hashes(std::iter::once(hash), None),
        1
    );
    assert!(!queue.txs.contains_key(&hash));
    assert_eq!(queue.queued_len(), 0);
}
#[test]
#[allow(clippy::too_many_lines)]
fn guard_return_expires_inflight_transaction_with_explicit_event() {
    let dir = tempfile::tempdir().expect("tempdir");
    let journal_path = dir.path().join("expired_guard_return_journal.norito");
    let state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut cfg = config_factory();
    cfg.transaction_time_to_live = Duration::from_millis(10);
    let mut queue = Queue::test(cfg, &time_source);
    let (event_sender, mut event_receiver) = tokio::sync::broadcast::channel(8);
    queue.events_sender = event_sender;
    queue
        .install_plan_journal(&journal_path, 1024 * 1024, true)
        .expect("install queue plan journal");
    let queue = Arc::new(queue);
    let tx = accepted_tx_by_someone(&time_source);
    let signed_hash = tx.as_ref().hash();
    let hash = tx.as_ref().hash_as_entrypoint();
    let authority = tx.as_ref().authority().clone();
    queue.push(tx, state.view()).expect("push expiring tx");
    let retained_bytes_before = queue.retained_bytes();
    assert!(retained_bytes_before > 0);
    assert_eq!(queue.queued_tx_count_for_user(&authority), 1);
    assert!(queue.routing_plans.contains_key(&hash));
    assert!(queue.routing_plan_hint(&hash).is_some());
    assert!(queue.expiry_ring_members.contains_key(&hash));
    assert_eq!(
        queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed journal")
            .live_record_count()
            .expect("count live journal records"),
        1
    );
    #[cfg(feature = "telemetry")]
    let terminal_teu = queue
        .tx_teu
        .get(&hash)
        .map(|entry| *entry.value())
        .expect("expiring transaction TEU metadata");
    while event_receiver.try_recv().is_ok() {}
    let mut guards = queue.collect_transactions_for_block(&state.view(), nonzero!(1_usize));
    assert_eq!(queue.inflight_guards.load(Ordering::Relaxed), 1);
    assert_eq!(queue.retained_bytes(), retained_bytes_before);
    assert_eq!(queue.queued_tx_count_for_user(&authority), 1);
    assert!(queue.expiry_ring_members.contains_key(&hash));
    time_handle.advance(Duration::from_millis(11));
    let report = queue
        .return_transaction_guards(&mut guards, &state)
        .expect("settle expired guard");
    assert_eq!(report.expired, 1);
    assert_eq!(report.returned, 0);
    assert_eq!(queue.active_len(), 0);
    assert_eq!(queue.queued_len(), 0);
    assert_eq!(queue.inflight_guards.load(Ordering::Relaxed), 0);
    assert!(!queue.queued_tx_enqueued_at_ms.contains_key(&hash));
    assert!(
        queue
            .queued_age_ring
            .lock()
            .iter()
            .all(|(queued_hash, _)| *queued_hash != hash),
        "expired guard return must remove its lazy age entry"
    );
    assert_eq!(queue.retained_bytes(), 0);
    assert_eq!(queue.queued_tx_count_for_user(&authority), 0);
    assert!(!queue.txs.contains_key(&hash));
    assert!(!queue.routing_plans.contains_key(&hash));
    assert!(queue.routing_plan_hint(&hash).is_none());
    assert!(!queue.expiry_ring_members.contains_key(&hash));
    assert_eq!(
        queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed journal")
            .live_record_count()
            .expect("count tombstoned journal records"),
        0,
        "expired guard return must tombstone its durable routing plan"
    );
    #[cfg(feature = "telemetry")]
    {
        assert!(!queue.tx_teu.contains_key(&hash));
        assert_eq!(
            queue
                .lane_teu_pending
                .get(&terminal_teu.lane_id)
                .map(|pending| (pending.teu, pending.tx_count))
                .unwrap_or_default(),
            (0, 0)
        );
        assert_eq!(
            queue
                .dataspace_teu_pending
                .get(&(terminal_teu.lane_id, terminal_teu.dataspace_id))
                .map(|pending| (pending.teu, pending.tx_count))
                .unwrap_or_default(),
            (0, 0)
        );
    }
    let event = event_receiver.try_recv().expect("expired event");
    let EventBox::Pipeline(PipelineEventBox::Transaction(event)) = event else {
        panic!("expected transaction event");
    };
    assert_eq!(event.hash, signed_hash);
    assert!(matches!(event.status, TransactionStatus::Expired));
    assert!(matches!(
        event_receiver.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Empty)
    ));
}
