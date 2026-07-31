// Lane-reservation restart, reconciliation, and fee-capacity regression tests.
//
// Included by `queue::tests` so source-bound libtest names remain stable.

#[test]
fn install_replay_reconciliation_fault_publishes_backpressure_after_unlock() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("install-reconciliation-plans.norito");
    let reservation_path = dir.path().join("install-reconciliation-fault.norito");
    let keys = {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install reconciliation queue-plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        let transactions = (0..2)
            .map(|_| accepted_tx_by_someone(&time_source))
            .collect::<Vec<_>>();
        for transaction in &transactions {
            push_globally_bound_lane_reservation_candidate(
                &queue,
                &state,
                &dir,
                transaction.clone(),
            );
        }
        let reserved = queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(
                    &state,
                    b"install-reconciliation-owner",
                    b"install-reconciliation-proposal",
                ),
                nonzero!(2_usize),
            )
            .expect("reserve install-reconciliation batch");
        let keys = reserved.iter().map(|tx| *tx.key()).collect::<Vec<_>>();
        let barrier =
            lane_reservation_release_barrier(keys.clone(), b"install-reconciliation-retire");
        queue
            .prepare_lane_reservation_release_barrier(&barrier)
            .expect("persist prepared install-reconciliation release");
        let reservations = queue.lane_reservations.lock();
        let ordered_records = barrier
            .ordered_keys
            .iter()
            .map(|key| reservations.live_by_hash[&key.signed_transaction_hash].clone())
            .collect();
        drop(reservations);
        queue
            .lane_reservation_journal
            .lock()
            .as_mut()
            .expect("installed reservation journal")
            .complete_release(LaneQueueReservationReleaseCompletionV5 {
                version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
                barrier,
                ordered_records,
            })
            .expect("persist completed release before simulated restart");
        keys
    };

    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    queue
        .install_plan_journal(test_lane_reservation_plan_path(&dir), 1024 * 1024, true)
        .expect("reopen install-reconciliation queue-plan journal");
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("replay transaction payloads before reservation reconciliation")
            .replayed,
        2
    );
    let pressure_rx = queue.backpressure_handle().subscribe();
    assert!(
        !pressure_rx.borrow().is_saturated(),
        "pressure must be healthy before install-time reconciliation"
    );
    *queue.install_reconciliation_append_fault.lock() =
        Some((0, ReservationJournalAppendFault::SyncAfterFullWrite));

    assert!(matches!(
        queue.install_lane_reservation_journal(&reservation_path, 1024 * 1024),
        Err(LaneQueueReservationError::Journal(_))
    ));
    assert!(
        queue.lane_reservation_durability_faulted(),
        "ambiguous install-time ForgetRelease must latch the process fault"
    );
    assert!(
        pressure_rx.borrow().is_saturated(),
        "install-time reconciliation must publish saturated pressure after releasing the reservation store"
    );
    assert!(
        queue.lane_reservations.try_lock().is_some(),
        "install error must release the reservation store before returning"
    );
    assert!(
        !queue.lane_reservation_group_is_finalized_for_diagnostics(&keys),
        "install-time reconciliation ambiguity must fail diagnostics closed"
    );
}

#[test]
fn reservation_validation_failure_does_not_poison_durability() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Queue::test(config_factory(), &time_source);
    let dir = tempdir().expect("tempdir");
    install_test_reservation_journal(&queue, &dir);
    let mut malformed = lane_reservation_scope(&state, b"owner", b"proposal");
    malformed.lane_block_height = 0;
    let error = match queue.reserve_transactions_for_lane(&state, malformed, nonzero!(1_usize)) {
        Err(error) => error,
        Ok(_) => panic!("zero lane-local height must be rejected before durability changes"),
    };
    assert!(
        matches!(
            &error,
            LaneQueueReservationError::InvalidIdentity(reason)
                if reason == "lane block height must be non-zero"
        ),
        "unexpected validation error: {error:?}"
    );
    assert!(!queue.lane_reservation_durability_faulted());
}

#[test]
fn reservation_restart_restore_blocks_resync_until_kura_absence_reconciliation() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans.norito");
    let reservation_path = dir.path().join("lane-reservations.norito");
    let tx = accepted_tx_by_someone(&time_source);
    let hash = tx.hash();
    let key = {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, tx);
        queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"restart-owner", b"restart-proposal"),
                nonzero!(1_usize),
            )
            .expect("reserve before crash")[0]
            .key()
            .clone()
    };

    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let restored = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore reservations before transaction replay");
    assert_eq!(restored.restored, 1);
    assert_eq!(restored.awaiting_transaction_replay, 1);
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install plan journal after reservation restore");
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("replay pending transaction")
            .replayed,
        1
    );
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 0);

    // Force the ordinary resync corridor; the live reservation must never be reinserted.
    let mut global = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(1_usize), &mut global);
    assert!(global.is_empty());
    assert!(queue.contains_transaction_hash(hash));
    assert_eq!(
        queue
            .reconcile_orphaned_lane_reservations(core::slice::from_ref(&key), |_| true)
            .expect("retain when Kura payload exists"),
        0
    );
    assert_eq!(queue.queued_len(), 0);
    assert_eq!(
        queue
            .reconcile_orphaned_lane_reservations(core::slice::from_ref(&key), |_| false)
            .expect("release only after Kura absence proof"),
        1
    );
    queue.get_transactions_for_block_with_state(&state, nonzero!(1_usize), &mut global);
    assert_eq!(global[0].as_ref().hash(), hash);
}

#[test]
fn committed_state_with_live_reservation_retains_sole_plan_payload_source() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-committed-live-owner.norito");
    let reservation_path = dir
        .path()
        .join("lane-reservations-committed-live-owner.norito");
    let transaction = accepted_tx_by_someone(&time_source);
    let hash = transaction.hash();
    {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install committed-owner plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install committed-owner reservation journal");
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
        queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"committed-live-owner", b"committed-live-proposal"),
                nonzero!(1_usize),
            )
            .expect("reserve transaction before canonical commit");
    }

    let block_header = ValidBlock::new_dummy(&checked_random_queue_keypair().into_parts().1)
        .as_ref()
        .header();
    let mut state_block = state.block(block_header);
    state_block
        .transactions
        .insert_block(HashSet::from([hash]), nonzero!(1_usize));
    state_block.commit().expect("commit transaction identity");

    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let reservation_replay = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore live reservation before plan replay");
    assert_eq!(reservation_replay.restored, 1);
    assert_eq!(reservation_replay.awaiting_transaction_replay, 1);
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install sole payload journal"),
        1
    );

    let error = queue
        .replay_plan_journal(&state)
        .expect_err("committed state plus live reservation requires explicit reconciliation");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        error
            .to_string()
            .contains("canonically committed while queue or reservation ownership remains live"),
        "unexpected replay error: {error}"
    );
    assert!(queue.txs.is_empty());
    assert_eq!(queue.active_len(), 1);
    assert_eq!(
        queue
            .missing_reservation_payload_count
            .load(Ordering::Relaxed),
        1
    );
    assert_eq!(
        queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed sole payload journal")
            .live_record_count()
            .expect("count retained sole payload owner"),
        1,
        "terminal state must not tombstone the only payload source while reservation ownership remains live"
    );
}

#[test]
fn expired_live_reservation_replays_payload_without_fifo_or_tombstone() {
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-expired-live-owner.norito");
    let reservation_path = dir.path().join("reservations-expired-live-owner.norito");
    let config = Config {
        transaction_time_to_live: Duration::from_millis(1),
        expired_cull_interval: Duration::from_millis(1),
        expired_cull_batch: nonzero!(16_usize),
        ..config_factory()
    };
    let transaction = accepted_tx_by_someone(&time_source);
    let hash = transaction.hash();
    let reserved_key = {
        let queue = Arc::new(Queue::test(config, &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install expired-owner plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install expired-owner reservation journal");
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
        *queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"expired-live-owner", b"expired-live-proposal"),
                nonzero!(1_usize),
            )
            .expect("reserve transaction before it expires")[0]
            .key()
    };
    time_handle.advance(Duration::from_millis(2));

    let queue = Arc::new(Queue::test(config, &time_source));
    let reservation_replay = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore expired live reservation before plan payload");
    assert_eq!(reservation_replay.restored, 1);
    assert_eq!(reservation_replay.awaiting_transaction_replay, 1);
    assert_eq!(queue.active_len(), 1);
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install expired live-owner plan journal"),
        1
    );

    let summary = queue
        .replay_plan_journal(&state)
        .expect("materialize expired payload under its durable reservation owner");
    assert_eq!(
        summary,
        QueuePlanJournalReplaySummary {
            records: 1,
            replayed: 1,
            tombstoned_committed: 0,
            tombstoned_expired: 0,
            tombstoned_conflicting_global_admission: 0,
        }
    );
    let replayed = queue
        .txs
        .get(&hash)
        .expect("expired reservation payload must be materialized");
    assert!(queue.is_expired(replayed.as_accepted()));
    drop(replayed);
    assert_eq!(
        queue
            .missing_reservation_payload_count
            .load(Ordering::Relaxed),
        0,
        "exact plan replay must clear payload-less owner accounting"
    );
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 0);
    assert!(queue.tx_hashes.is_empty());
    assert_eq!(queue.live_lane_reservations(), vec![reserved_key]);
    assert_eq!(
        queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed expired live-owner plan journal")
            .live_record_count()
            .expect("count retained expired live-owner record"),
        1,
        "expiry must not tombstone the sole payload source while reservation ownership is live"
    );
}

#[test]
fn missing_replayed_reservation_owns_capacity_until_exact_payload_replay() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-owner-capacity.norito");
    let reservation_path = dir.path().join("lane-reservations-owner-capacity.norito");
    let one_slot_config = || Config {
        capacity: nonzero!(1_usize),
        capacity_per_user: nonzero!(1_usize),
        ..config_factory()
    };
    let transaction = accepted_tx_by_someone(&time_source);
    let transaction_hash = transaction.hash();
    let retained_cost = Queue::retained_byte_cost(Queue::compute_tx_encoded_len(&transaction));
    {
        let queue = Arc::new(Queue::test(one_slot_config(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
        queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"owner-capacity-owner", b"owner-capacity-proposal"),
                nonzero!(1_usize),
            )
            .expect("reserve before restart");
    }

    let queue = Arc::new(Queue::test(one_slot_config(), &time_source));
    let replay = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore the payload-less reservation owner");
    assert_eq!(replay.restored, 1);
    assert_eq!(replay.awaiting_transaction_replay, 1);
    assert!(queue.txs.is_empty());
    assert_eq!(queue.materialized_active_len(), 0);
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.retained_bytes(), TX_RETAINED_OVERHEAD_BYTES);
    let pressure = queue.pressure_snapshot();
    assert_eq!(pressure.tracked_tx_count, 1);
    assert!(pressure.saturated_by_count);

    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install the payload journal");
    let unrelated = accepted_tx_by_someone(&time_source);
    assert_ne!(unrelated.hash(), transaction_hash);
    let failure = queue
        .push_with_lane_with_state(unrelated, &state)
        .expect_err("unrelated work must not consume the restored owner's slot");
    assert!(matches!(failure.err, Error::Full));
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.retained_bytes(), TX_RETAINED_OVERHEAD_BYTES);

    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("the exact payload must replace its reserved capacity")
            .replayed,
        1
    );
    assert_eq!(queue.materialized_active_len(), 1);
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 0);
    assert_eq!(queue.retained_bytes(), retained_cost);
    assert_eq!(
        queue
            .missing_reservation_payload_count
            .load(Ordering::Relaxed),
        0
    );
    assert!(
        queue
            .lane_reservations
            .lock()
            .missing_payload_hashes
            .is_empty()
    );
}

#[test]
fn missing_replayed_reservation_owns_retained_budget_until_exact_payload_replay() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-owner-bytes.norito");
    let reservation_path = dir.path().join("lane-reservations-owner-bytes.norito");
    let transaction = accepted_tx_by_someone(&time_source);
    let retained_cost = Queue::retained_byte_cost(Queue::compute_tx_encoded_len(&transaction));
    let bounded_config = || Config {
        capacity: nonzero!(2_usize),
        capacity_per_user: nonzero!(2_usize),
        max_retained_bytes: NonZeroU64::new(retained_cost)
            .expect("transaction retained cost is non-zero"),
        ..config_factory()
    };
    {
        let queue = Arc::new(Queue::test(bounded_config(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
        queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"owner-bytes-owner", b"owner-bytes-proposal"),
                nonzero!(1_usize),
            )
            .expect("reserve before restart");
    }

    let queue = Arc::new(Queue::test(bounded_config(), &time_source));
    queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore the payload-less reservation owner");
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.retained_bytes(), TX_RETAINED_OVERHEAD_BYTES);
    assert!(queue.pressure_snapshot().saturated_by_bytes);
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install the payload journal");

    let failure = queue
        .push_with_lane_with_state(accepted_tx_by_someone(&time_source), &state)
        .expect_err("unrelated work must not consume the restored owner's byte budget");
    assert!(matches!(failure.err, Error::Full));
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.retained_bytes(), TX_RETAINED_OVERHEAD_BYTES);

    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("the exact payload must replace its reserved byte charge")
            .replayed,
        1
    );
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.retained_bytes(), retained_cost);
    assert_eq!(
        queue
            .missing_reservation_payload_count
            .load(Ordering::Relaxed),
        0
    );
}

#[test]
fn restart_commit_barrier_tombstones_pending_plan_before_replay() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-commit-window.norito");
    let reservation_path = dir.path().join("lane-reservations-commit-window.norito");
    {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        push_globally_bound_lane_reservation_candidate(
            &queue,
            &state,
            &dir,
            accepted_tx_by_someone(&time_source),
        );
        let key = *queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"commit-window-owner", b"commit-window-proposal"),
                nonzero!(1_usize),
            )
            .expect("reserve transaction")[0]
            .key();
        // Model a crash after the reservation Commit fsync and before the independent
        // queue-plan Remove append. The public commit API closes this window with the same
        // durable barrier protocol.
        queue
            .lane_reservation_journal
            .lock()
            .as_mut()
            .expect("reservation journal")
            .commit(key)
            .expect("durable commit barrier");
    }

    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let restored = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore commit barrier before plan journal");
    assert_eq!(restored.restored, 0);
    assert_eq!(restored.commit_barriers, 1);
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("plan installation must finalize commit barrier");
    assert!(queue.lane_reservation_commit_barriers().is_empty());
    let replay = queue
        .replay_plan_journal(&state)
        .expect("replay after exact commit tombstone");
    assert_eq!(replay.replayed, 0);
    assert_eq!(replay.records, 0);
    assert_eq!(queue.active_len(), 0);
    assert_eq!(queue.queued_len(), 0);
}

#[test]
fn stale_reservation_commit_digest_cannot_tombstone_or_forget_live_plan() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-stale-commit.norito");
    let reservation_path = dir.path().join("lane-reservations-stale-commit.norito");
    let hash;
    {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        let transaction = accepted_tx_by_someone(&time_source);
        hash = transaction.hash();
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
        let mut stale = *queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"stale-commit-owner", b"stale-commit-proposal"),
                nonzero!(1_usize),
            )
            .expect("reserve transaction")[0]
            .key();
        stale.routing_plan_digest = Hash::new(b"stale reservation commit plan digest");

        let error = queue
            .remove_plan_journal_for_reservation_commit(&stale)
            .expect_err("a stale commit digest must not append a no-op tombstone");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(queue.plan_journal_durability_faulted());
        assert!(queue.txs.contains_key(&hash));
        assert!(queue.durable_plan_claims.contains_key(&hash));
    }

    let replay_queue = Arc::new(Queue::test(config_factory(), &time_source));
    assert_eq!(
        replay_queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("reopen plan journal after rejected stale tombstone"),
        1,
        "the exact live owner must survive the rejected stale cleanup and restart"
    );
    let replay = replay_queue
        .replay_plan_journal(&state)
        .expect("replay exact plan after rejected stale tombstone");
    assert_eq!(replay.records, 1);
    assert_eq!(replay.replayed, 1);
    assert!(replay_queue.txs.contains_key(&hash));
}

#[test]
fn stale_reservation_commit_binding_cannot_tombstone_or_forget_live_plan() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-stale-commit-binding.norito");
    let reservation_path = dir
        .path()
        .join("lane-reservations-stale-commit-binding.norito");
    let hash;
    {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        let transaction = accepted_tx_by_someone(&time_source);
        hash = transaction.hash();
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
        let mut stale = *queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"stale-binding-owner", b"stale-binding-proposal"),
                nonzero!(1_usize),
            )
            .expect("reserve transaction")[0]
            .key();
        stale.queue_plan_admission_binding_hash =
            Hash::new(b"stale reservation commit admission binding");

        let error = queue
            .remove_plan_journal_for_reservation_commit(&stale)
            .expect_err("a stale binding hash must not append a plan tombstone");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(queue.plan_journal_durability_faulted());
        assert!(queue.txs.contains_key(&hash));
        assert!(queue.durable_plan_claims.contains_key(&hash));
    }

    let replay_queue = Arc::new(Queue::test(config_factory(), &time_source));
    assert_eq!(
        replay_queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("reopen plan journal after rejected stale binding"),
        1,
        "the exact live owner must survive a forged reservation binding"
    );
    let replay = replay_queue
        .replay_plan_journal(&state)
        .expect("replay exact plan after rejected stale binding");
    assert_eq!(replay.records, 1);
    assert_eq!(replay.replayed, 1);
    assert!(replay_queue.txs.contains_key(&hash));
}

#[test]
fn installed_plan_journal_reconciles_high_volume_commit_barriers_and_restarts_cleanly() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = test_lane_reservation_plan_path(&dir);
    let reservation_path = dir.path().join("installed-plan-high-volume.norito");
    let make_queue = || {
        Queue::test(
            Config {
                capacity: nonzero!(300_usize),
                capacity_per_user: nonzero!(300_usize),
                ..config_factory()
            },
            &time_source,
        )
    };
    {
        let queue = Arc::new(make_queue());
        queue
            .install_lane_reservation_journal(&reservation_path, 1)
            .expect("install reservation journal");
        for index in 0_u16..256 {
            let _ = persist_unreconciled_commit_barrier(
                &queue,
                &state,
                &dir,
                accepted_tx_by_someone(&time_source),
                &index.to_le_bytes(),
                &index.wrapping_add(1).to_le_bytes(),
            );
        }
    }

    {
        let queue = make_queue();
        let replay = queue
            .install_lane_reservation_journal(&reservation_path, 1)
            .expect("restore high-volume commit barriers before the production plan journal");
        assert_eq!(replay.restored, 0);
        assert_eq!(replay.commit_barriers, 256);
        assert_eq!(queue.lane_reservation_commit_barriers().len(), 256);
        assert_eq!(
            queue
                .install_plan_journal(&plan_path, 1024 * 1024, true)
                .expect("install exact source plan journal and finalize every commit barrier"),
            256
        );
        assert!(queue.lane_reservation_commit_barriers().is_empty());
        assert_eq!(queue.active_len(), 0);
        assert_eq!(queue.queued_len(), 0);
        assert_eq!(
            queue
                .plan_journal
                .lock()
                .as_ref()
                .expect("installed plan journal")
                .replay_scan_count(),
            2,
            "startup reconciliation must use one count scan and one batch-validation scan regardless of barrier count"
        );
        assert_eq!(
            queue
                .replay_plan_journal(&state)
                .expect("all committed plans were durably tombstoned"),
            QueuePlanJournalReplaySummary::default()
        );
        assert!(
            reservation_path.metadata().expect("journal metadata").len() < 4096,
            "atomic compaction must bound reconciled commit-barrier history"
        );
    }

    let queue = make_queue();
    let replay = queue
        .install_lane_reservation_journal(&reservation_path, 1)
        .expect("restart compacted reservation journal");
    assert_eq!(replay, LaneQueueReservationReplaySummary::default());
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("restart reconciled production plan journal"),
        0
    );
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("second restart remains empty"),
        QueuePlanJournalReplaySummary::default()
    );
}

#[test]
fn commit_barrier_forget_prefix_failure_evicts_all_consumed_owners_and_restarts_cleanly() {
    const BARRIER_COUNT: usize = 3;
    const JOURNAL_LIMIT: u64 = 4 * 1024 * 1024;

    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    for (successful_forgets_before_fault, label) in [(0, "first"), (1, "middle")] {
        let dir = tempdir().expect("tempdir");
        let plan_path = dir
            .path()
            .join(format!("forget-{label}-prefix-plans.norito"));
        let reservation_path = dir
            .path()
            .join(format!("forget-{label}-prefix-reservations.norito"));
        let keys = {
            let queue = Arc::new(Queue::test(config_factory(), &time_source));
            queue
                .install_plan_journal(&plan_path, JOURNAL_LIMIT, true)
                .expect("install seed plan journal");
            queue
                .install_lane_reservation_journal(&reservation_path, JOURNAL_LIMIT)
                .expect("install seed reservation journal");
            let mut keys = Vec::with_capacity(BARRIER_COUNT);
            for index in 0_u8..u8::try_from(BARRIER_COUNT).expect("barrier count fits u8") {
                admit_globally_certified_reservation_transaction_for_test(
                    &queue,
                    &state,
                    accepted_unique_entrypoint_tx_by_someone(&time_source),
                );
                keys.push(
                    *queue
                        .reserve_transactions_for_lane(
                            &state,
                            lane_reservation_scope(
                                &state,
                                &[index, successful_forgets_before_fault as u8],
                                &[index.wrapping_add(1), successful_forgets_before_fault as u8],
                            ),
                            nonzero!(1_usize),
                        )
                        .expect("reserve forget-prefix transaction")[0]
                        .key(),
                );
            }
            let mut journal_guard = queue.lane_reservation_journal.lock();
            let journal = journal_guard
                .as_mut()
                .expect("seed reservation journal remains installed");
            for key in &keys {
                journal
                    .commit(*key)
                    .expect("persist unfinished commit barrier");
            }
            keys
        };
        let hashes = keys
            .iter()
            .map(|key| key.signed_transaction_hash)
            .collect::<Vec<_>>();

        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        assert_eq!(
            queue
                .install_plan_journal(&plan_path, JOURNAL_LIMIT, true)
                .expect("restore live plans before reservation barriers"),
            BARRIER_COUNT,
        );
        assert_eq!(
            queue
                .replay_plan_journal(&state)
                .expect("materialize every plan owner before reconciliation")
                .replayed,
            BARRIER_COUNT,
        );
        assert_eq!(queue.queued_len(), BARRIER_COUNT);
        *queue.install_reconciliation_append_fault.lock() = Some((
            successful_forgets_before_fault,
            ReservationJournalAppendFault::SyncAfterFullWrite,
        ));
        let pressure_rx = queue.backpressure_handle().subscribe();

        assert!(matches!(
            queue.install_lane_reservation_journal(&reservation_path, JOURNAL_LIMIT),
            Err(LaneQueueReservationError::Journal(_))
        ));

        assert!(
            queue.lane_reservation_durability_faulted(),
            "{label} ForgetCommit failure must make selection fail closed",
        );
        assert!(pressure_rx.borrow().is_saturated());
        assert_eq!(
            queue.materialized_active_len(),
            0,
            "all transactions whose plans were batch-tombstoned must be evicted before the first ForgetCommit",
        );
        assert_eq!(queue.queued_len(), 0);
        let fifo = {
            let _queue_guard = queue.push_remove_lock.lock();
            queue.fifo_snapshot_locked()
        };
        for hash in &hashes {
            assert!(
                !fifo.contains(hash),
                "{label}: consumed hash remains in FIFO"
            );
            assert!(
                !queue.txs.contains_key(hash),
                "{label}: consumed transaction remains materialized",
            );
            assert!(
                !queue.routing_plans.contains_key(hash),
                "{label}: consumed routing plan remains materialized",
            );
            assert!(
                !queue.durable_plan_claims.contains_key(hash),
                "{label}: consumed durable plan claim remains indexed",
            );
        }
        assert!(
            queue
                .plan_journal
                .lock()
                .as_ref()
                .expect("plan journal remains installed")
                .replay()
                .expect("replay batch-tombstoned plan journal")
                .is_empty(),
            "{label}: every plan tombstone must precede any ForgetCommit append",
        );
        let mut selected = Vec::new();
        queue.get_transactions_for_block_with_state(&state, nonzero!(3_usize), &mut selected);
        assert!(
            selected.is_empty(),
            "{label}: post-batch reconciliation failure must disable selection",
        );
        drop(queue);

        let queue = Queue::test(config_factory(), &time_source);
        assert_eq!(
            queue
                .install_plan_journal(&plan_path, JOURNAL_LIMIT, true)
                .expect("restart the fully tombstoned plan journal"),
            0,
        );
        assert_eq!(
            queue
                .install_lane_reservation_journal(&reservation_path, JOURNAL_LIMIT)
                .expect("idempotently finish the remaining commit barriers after restart"),
            LaneQueueReservationReplaySummary::default(),
        );
        assert!(queue.lane_reservation_commit_barriers().is_empty());
        assert_eq!(queue.active_len(), 0);
        assert_eq!(queue.queued_len(), 0);
        assert!(
            queue
                .plan_journal
                .lock()
                .as_ref()
                .expect("restarted plan journal remains installed")
                .replay()
                .expect("replay restarted plan journal")
                .is_empty(),
        );
    }
}

#[test]
fn restart_commit_barrier_rejects_mismatched_queue_hash_without_tombstone_or_forget() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = test_lane_reservation_plan_path(&dir);
    let reservation_path = dir.path().join("restart-mismatched-queue-hash.norito");
    {
        let queue = Queue::test(config_factory(), &time_source);
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        let _ = persist_unreconciled_commit_barrier(
            &queue,
            &state,
            &dir,
            accepted_tx_by_someone(&time_source),
            b"mismatched-hash-owner",
            b"mismatched-hash-proposal",
        );
    }

    {
        let queue = Queue::test(config_factory(), &time_source);
        assert_eq!(
            queue
                .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
                .expect("restore exact commit barrier")
                .commit_barriers,
            1
        );
        queue.lane_reservations.lock().commit_barriers[0].signed_transaction_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"forged compatibility queue hash"));
        let error = queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect_err("mismatched queue hash must fail before plan cleanup");
        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        assert_eq!(queue.lane_reservation_commit_barriers().len(), 1);
        assert_eq!(
            queue
                .plan_journal
                .lock()
                .as_mut()
                .expect("installed plan journal remains inspectable")
                .replay()
                .expect("replay rejected cleanup")
                .len(),
            1
        );
    }

    let queue = Queue::test(config_factory(), &time_source);
    assert_eq!(
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("ForgetCommit was not appended")
            .commit_barriers,
        1
    );
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("the original exact barrier remains recoverable"),
        1
    );
    assert!(queue.lane_reservation_commit_barriers().is_empty());
}

#[test]
fn restart_commit_barrier_rejects_retargeted_coordinator_without_tombstone_or_forget() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = test_lane_reservation_plan_path(&dir);
    let reservation_path = dir.path().join("restart-retargeted-coordinator.norito");
    {
        let queue = Queue::test(config_factory(), &time_source);
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        let _ = persist_unreconciled_commit_barrier(
            &queue,
            &state,
            &dir,
            accepted_tx_by_someone(&time_source),
            b"retargeted-owner",
            b"retargeted-proposal",
        );
    }

    {
        let queue = Queue::test(config_factory(), &time_source);
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("restore exact commit barrier");
        let retargeted = RoutingDecision::new(LaneId::new(77), DataSpaceId::new(88));
        let mut store = queue.lane_reservations.lock();
        let key = &mut store.commit_barriers[0];
        key.coordinator_leg = RouteLeg::new(retargeted, RouteLegRole::Coordinator);
        key.lane_id = retargeted.lane_id;
        key.dataspace_id = retargeted.dataspace_id;
        key.lane_incarnation = Hash::new(b"retargeted lane incarnation");
        drop(store);
        let error = queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect_err("retargeted coordinator must fail before plan cleanup");
        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        assert_eq!(queue.lane_reservation_commit_barriers().len(), 1);
        assert_eq!(
            queue
                .plan_journal
                .lock()
                .as_mut()
                .expect("installed plan journal")
                .replay()
                .expect("replay rejected retarget")
                .len(),
            1
        );
    }

    let queue = Queue::test(config_factory(), &time_source);
    assert_eq!(
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("retarget rejection appended no ForgetCommit")
            .commit_barriers,
        1
    );
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("the durable exact coordinator still reconciles"),
        1
    );
    assert!(queue.lane_reservation_commit_barriers().is_empty());
}

#[test]
fn restart_commit_barrier_rejects_same_plan_binding_aba_without_tombstone_or_forget() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = test_lane_reservation_plan_path(&dir);
    let reservation_path = dir.path().join("restart-same-plan-binding-aba.norito");
    {
        let queue = Queue::test(config_factory(), &time_source);
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        let _ = persist_unreconciled_commit_barrier(
            &queue,
            &state,
            &dir,
            accepted_tx_by_someone(&time_source),
            b"aba-owner",
            b"aba-proposal",
        );
        let mut journal = queue.plan_journal.lock();
        let journal = journal.as_mut().expect("installed plan journal");
        let mut replacement = journal
            .replay()
            .expect("read original exact claim")
            .pop()
            .expect("one original exact claim");
        replacement.enqueue_timestamp_ms = replacement.enqueue_timestamp_ms.saturating_add(1);
        journal
            .replace_strict_durable(replacement)
            .expect("persist same-plan replacement binding");
    }

    {
        let queue = Queue::test(config_factory(), &time_source);
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("restore stale commit barrier");
        let error = queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect_err("stale binding must not delete the same-plan replacement");
        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        assert_eq!(queue.lane_reservation_commit_barriers().len(), 1);
        assert_eq!(
            queue
                .plan_journal
                .lock()
                .as_mut()
                .expect("installed plan journal")
                .replay()
                .expect("replacement survives stale cleanup")
                .len(),
            1
        );
    }

    let queue = Queue::test(config_factory(), &time_source);
    assert_eq!(
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("stale ABA cleanup appended no ForgetCommit")
            .commit_barriers,
        1
    );
    assert_eq!(
        QueuePlanJournal::open_with_limits(
            &plan_path,
            QueuePlanJournalLimits::new(
                1024 * 1024,
                queue.max_retained_bytes.get().min(u64::from(u32::MAX)),
                (1024_u64 * 1024)
                    .saturating_add(queue.max_retained_bytes.get())
                    .saturating_add(queue.max_retained_bytes.get().min(u64::from(u32::MAX)),),
                queue.capacity.get(),
            ),
            true,
        )
        .expect("open replacement plan journal")
        .replay()
        .expect("replay replacement after second restart")
        .len(),
        1
    );
}

#[test]
fn restart_after_plan_tombstone_before_forget_commit_is_idempotent() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = test_lane_reservation_plan_path(&dir);
    let reservation_path = dir.path().join("restart-after-plan-tombstone.norito");
    {
        let queue = Queue::test(config_factory(), &time_source);
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        let key = persist_unreconciled_commit_barrier(
            &queue,
            &state,
            &dir,
            accepted_tx_by_someone(&time_source),
            b"tombstone-owner",
            b"tombstone-proposal",
        );
        assert_eq!(
            queue
                .plan_journal
                .lock()
                .as_mut()
                .expect("installed plan journal")
                .remove_exact_global_admission_binding_strict_durable(&key)
                .expect("persist plan tombstone before simulated crash"),
            QueuePlanJournalExactRemoveResult::Removed
        );
    }

    let queue = Queue::test(config_factory(), &time_source);
    assert_eq!(
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("restore unforgotten commit barrier")
            .commit_barriers,
        1
    );
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("an already durable plan tombstone is idempotent"),
        0
    );
    assert!(queue.lane_reservation_commit_barriers().is_empty());
    assert_eq!(queue.active_len(), 0);
    drop(queue);

    let queue = Queue::test(config_factory(), &time_source);
    assert_eq!(
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("ForgetCommit survives a second restart"),
        LaneQueueReservationReplaySummary::default()
    );
}

#[test]
fn plan_install_commit_barrier_reconciliation_refreshes_backpressure_snapshot() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = test_lane_reservation_plan_path(&dir);
    let reservation_path = dir.path().join("restart-backpressure-refresh.norito");
    let make_queue = || {
        Queue::test(
            Config {
                capacity: nonzero!(1_usize),
                capacity_per_user: nonzero!(1_usize),
                ..config_factory()
            },
            &time_source,
        )
    };
    {
        let queue = make_queue();
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        let _ = persist_unreconciled_commit_barrier(
            &queue,
            &state,
            &dir,
            accepted_tx_by_someone(&time_source),
            b"pressure-owner",
            b"pressure-proposal",
        );
    }

    let queue = make_queue();
    assert_eq!(
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("restore capacity-owning commit barrier")
            .commit_barriers,
        1
    );
    assert_eq!(
        queue.active_len(),
        1,
        "the payload-less Commit barrier must still own queue capacity"
    );
    let pressure = queue.backpressure_handle();
    assert!(
        matches!(
            pressure.snapshot(),
            BackpressureState::Saturated {
                queued: 0,
                capacity,
            } if capacity == nonzero!(1_usize)
        ),
        "the restored owner is capacity pressure, not an ordinary queued payload"
    );
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("reconcile exact commit barrier");
    assert_eq!(
        pressure.snapshot(),
        BackpressureState::Healthy {
            queued: 0,
            capacity: nonzero!(1_usize),
        },
        "successful startup cleanup must publish the terminal tracked count"
    );
}

#[test]
fn globally_bound_reservation_survives_expiry_until_canonical_commit() {
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(
        Config {
            transaction_time_to_live: Duration::from_millis(1),
            expired_cull_interval: Duration::from_millis(1),
            expired_cull_batch: nonzero!(16_usize),
            ..config_factory()
        },
        &time_source,
    ));
    let dir = tempdir().expect("tempdir");
    install_test_reservation_journal(&queue, &dir);
    let transaction = accepted_tx_by_someone(&time_source);
    let hash = transaction.hash();
    let binding = push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
    let key = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(&state, b"expiry-owner", b"expiry-proposal"),
            nonzero!(1_usize),
        )
        .expect("reserve expiring transaction")[0]
        .key()
        .clone();
    time_handle.advance(Duration::from_millis(2));
    assert_eq!(queue.cull_expired_entries(time_source.get_unix_time()), 0);
    assert_eq!(queue.active_len(), 1);

    queue
        .release_lane_reservation(&key)
        .expect("release expired reservation");
    assert_eq!(
        queue.cull_expired_entries(time_source.get_unix_time()),
        0,
        "TTL expiry cannot erase an exact globally certified owner after lane release"
    );
    assert_eq!(queue.active_len(), 1);
    assert!(
        queue
            .durable_plan_claims
            .get(&hash)
            .is_some_and(|claim| claim.journal_record_digest == binding.journal_record_digest)
    );

    assert_eq!(
        queue.remove_committed_hashes([hash], None),
        1,
        "canonical execution must terminally consume the durable owner"
    );
    assert_eq!(queue.active_len(), 0);
    assert_eq!(queue.retained_bytes(), 0);
    assert!(!queue.durable_plan_claims.contains_key(&hash));
    assert!(
        queue
            .plan_journal
            .lock()
            .as_ref()
            .expect("installed plan journal")
            .replay()
            .expect("replay post-commit journal")
            .is_empty(),
        "canonical execution must durably tombstone the exact plan claim"
    );
}

#[test]
fn concurrent_lane_reserve_attempts_cannot_duplicate_one_transaction() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_test_reservation_journal(&queue, &dir);
    push_globally_bound_lane_reservation_candidate(
        &queue,
        &state,
        &dir,
        accepted_tx_by_someone(&time_source),
    );

    let barrier = Arc::new(std::sync::Barrier::new(3));
    let mut handles = Vec::new();
    for index in 0_u8..2 {
        let queue = Arc::clone(&queue);
        let state = Arc::clone(&state);
        let barrier = Arc::clone(&barrier);
        handles.push(thread::spawn(move || {
            barrier.wait();
            queue
                .reserve_transactions_for_lane(
                    &state,
                    lane_reservation_scope(&state, &[b'o', index], &[b'p', index]),
                    nonzero!(1_usize),
                )
                .expect("concurrent reservation attempt")
                .len()
        }));
    }
    barrier.wait();
    let selected: usize = handles
        .into_iter()
        .map(|handle| handle.join().expect("join reservation attempt"))
        .sum();
    assert_eq!(selected, 1);
    assert_eq!(queue.live_lane_reservations().len(), 1);
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 0);
}

#[test]
fn stale_lane_incarnation_identity_fails_closed() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_test_reservation_journal(&queue, &dir);
    push_globally_bound_lane_reservation_candidate(
        &queue,
        &state,
        &dir,
        accepted_tx_by_someone(&time_source),
    );
    let mut stale = lane_reservation_scope(&state, b"stale-owner", b"stale-proposal");
    stale.lane_incarnation = Hash::new(b"retired-incarnation");
    assert!(matches!(
        queue.reserve_transactions_for_lane(&state, stale, nonzero!(1_usize)),
        Err(LaneQueueReservationError::StaleLaneIncarnation)
    ));
    assert_eq!(queue.queued_len(), 1);
}

#[test]
fn native_amx_participant_lane_cannot_reserve_or_execute_full_transaction() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let coordinator = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let participant = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
    let routes = [
        (coordinator.lane_id, coordinator.dataspace_id),
        (participant.lane_id, participant.dataspace_id),
    ];
    let (lane_catalog, dataspace_catalog) = Queue::test_catalogs_for_routes(&routes);
    let kura_dir = tempdir().expect("authenticated queue Kura root");
    let lane_geometry = LaneGeometry::from_catalog(&lane_catalog);
    let kura_config = KuraConfig {
        init_mode: InitMode::Strict,
        store_dir: WithOrigin::inline(kura_dir.path().join("kura")),
        max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY,
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity:
            iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: iroha_config::kura::FsyncMode::Batched,
        fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
        block_sync_roster_retention:
            iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
        roster_sidecar_retention:
            iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
        eviction_required_replicas:
            iroha_config::parameters::defaults::kura::EVICTION_REQUIRED_REPLICAS,
    };
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config, &lane_geometry, &lane_catalog)
            .expect("open authenticated two-lane reservation Kura");
    // Exercise the production startup sequence here. `State::new` is the unit-test
    // convenience constructor and eagerly installs a marker for its initial single-lane
    // catalog; that marker necessarily precedes (and therefore conflicts with) the
    // authenticated two-lane configured-primary anchor established below.
    let mut state = State::try_new(
        world_with_test_domains(),
        kura,
        LiveQueryStore::start_test(),
        #[cfg(feature = "telemetry")]
        <_>::default(),
    )
    .expect("open reservation-test State without replacing authenticated Kura markers");
    state
        .prepare_configured_primary_geometry_anchor(&lane_catalog)
        .expect("anchor authenticated reservation-test primary");
    state
        .restore_kura_lane_segments_before_startup_replay()
        .expect("restore reservation-test startup cursor");
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = true;
    nexus.fees.base_fee = Quantity::zero();
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    nexus.lane_catalog = (*lane_catalog).clone();
    nexus.configured_lane_catalog = nexus.lane_catalog.clone();
    nexus.lane_config = lane_geometry;
    nexus.dataspace_catalog = (*dataspace_catalog).clone();
    nexus.fees.base_fee = Quantity::zero();
    nexus.fees.per_byte_fee = Quantity::zero();
    nexus.fees.per_instruction_fee = Quantity::zero();
    nexus.fees.per_gas_unit_fee = Quantity::zero();
    state
        .set_nexus_from_config(nexus)
        .expect("install two-lane reservation test Nexus");
    let router: Arc<dyn LaneRouter> = Arc::new(ConfigLaneRouter::new(
        state.nexus_snapshot().routing_policy,
        (*dataspace_catalog).clone(),
        (*lane_catalog).clone(),
    ));
    let queue = Arc::new(Queue::test_with_router_for_routes(
        config_factory(),
        &time_source,
        router,
        &routes,
    ));
    install_manifest_lane_authority_for_queue_test(&mut state, queue.as_ref(), 0xC1);
    let dir = tempdir().expect("tempdir");
    install_test_reservation_journal(&queue, &dir);
    queue
        .install_plan_journal(
            dir.path().join("native-amx-queue-plans.norito"),
            1024 * 1024,
            true,
        )
        .expect("install Native AMX queue-plan journal");
    let (authority, authority_keypair) = gen_account_in("wonderland");
    let transaction = accepted_tx_with(
        authority.clone(),
        &authority_keypair,
        &time_source,
        vec![
            InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("nativeamxcoordinator", "universal")
                    .expect("coordinator domain id"),
            ))),
            InstructionBox::from(Register::domain(Domain::new(
                DomainId::try_new("nativeamxparticipant", "test-dataspace-7")
                    .expect("participant domain id"),
            ))),
        ],
        Metadata::default(),
    );
    register_test_authority(&mut state, &authority);
    let plan = queue
        .route_plan_for_gossip_with_state(&transaction, &state)
        .expect("derive exact current Native AMX reservation plan");
    let RoutingPlan::NativeAmx(native_plan) = &plan else {
        panic!("mixed-dataspace reservation transaction must use Native AMX");
    };
    assert_eq!(native_plan.coordinator.route, coordinator);
    assert!(
        native_plan
            .participants
            .iter()
            .any(|leg| leg.route == participant),
        "Native AMX reservation plan must retain the participant lane"
    );
    let admission_context = queue
        .plan_admission_context_with_state(&state, &plan)
        .expect("capture Native AMX admission context");
    let admission_binding = crate::torii_proxy::QueuePlanAdmissionBindingV2::new(
        state.chain_id_ref(),
        transaction.entrypoint(),
        &plan,
        admission_context,
        queue.queue_plan_admission_timestamp_ms(),
    )
    .expect("build Native AMX global admission binding");
    queue
        .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
            transaction,
            &state,
            plan.clone(),
            &admission_binding,
        )
        .expect("durably enqueue globally bound Native AMX transaction");

    let participant_scope = LaneQueueReservationScopeV1 {
        lane_id: participant.lane_id,
        dataspace_id: participant.dataspace_id,
        lane_incarnation: state
            .lane_incarnation_at_height(participant.lane_id, 1)
            .expect("participant lane incarnation"),
        proposal_height: 1,
        lane_block_height: 1,
        lane_block_view: 0,
        reservation_owner_hash: Hash::new(b"participant-owner"),
        proposal_identity_hash: Hash::new(b"participant-proposal"),
    };
    let coordinator_scope = LaneQueueReservationScopeV1 {
        lane_id: coordinator.lane_id,
        dataspace_id: coordinator.dataspace_id,
        lane_incarnation: state
            .lane_incarnation_at_height(coordinator.lane_id, 1)
            .expect("coordinator lane incarnation"),
        ..participant_scope
    };
    assert_eq!(
        state
            .queue_plan_admission_binding_registry_match(&admission_binding)
            .expect("read absent Native AMX admission registry"),
        QueuePlanAdmissionRegistryMatch::Absent
    );
    assert!(
        queue
            .reserve_transactions_for_lane(&state, coordinator_scope, nonzero!(1_usize))
            .expect("uncertified Native AMX selection must safely retain FIFO ownership")
            .is_empty(),
        "a durable local binding is not autonomous ownership authority before the global \
             carrier commits its exact registry marker"
    );
    assert_eq!(queue.queued_len(), 1);

    install_queue_plan_registry_value_for_test(
        &state,
        &admission_binding,
        admission_binding.canonical_hash(),
    );
    assert_eq!(
        state
            .queue_plan_admission_binding_registry_match(&admission_binding)
            .expect("read exact Native AMX admission registry"),
        QueuePlanAdmissionRegistryMatch::Exact
    );
    assert!(
        queue
            .reserve_transactions_for_lane(&state, participant_scope, nonzero!(1_usize))
            .expect("participant selection must safely return no full transaction")
            .is_empty()
    );
    assert_eq!(queue.queued_len(), 1);

    assert!(
        queue
            .reserve_transactions_for_lane_bounded(
                &state,
                coordinator_scope,
                LaneQueueReservationSelectionLimits {
                    max_transactions: nonzero!(1_usize),
                    max_scan: nonzero!(1_usize),
                    max_encoded_bytes: NonZeroU64::new(u64::MAX).expect("non-zero byte bound"),
                    max_gas: NonZeroU64::new(u64::MAX).expect("non-zero gas bound"),
                },
                &BTreeSet::new(),
                LaneQueueReservationRoutingMode::SingleRouteOnly,
            )
            .expect("single-route mode excludes Native AMX")
            .is_empty()
    );
    assert_eq!(queue.queued_len(), 1);
    let reserved = queue
        .reserve_transactions_for_lane(&state, coordinator_scope, nonzero!(1_usize))
        .expect("coordinator reserves Native AMX transaction");
    assert_eq!(reserved.len(), 1);
    assert_eq!(reserved[0].routing_plan(), &plan);
    assert_eq!(
        reserved[0].key().coordinator_leg.role,
        RouteLegRole::Coordinator
    );
    assert_eq!(reserved[0].key().lane_id, coordinator.lane_id);
}

#[test]
fn opposite_global_and_lane_call_orders_never_select_the_same_hash() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let first = accepted_tx_by_someone(&time_source);
    let second = accepted_tx_by_someone(&time_source);
    let all_hashes = BTreeSet::from([first.hash(), second.hash()]);

    let run = |lane_first: bool, suffix: &str| {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join(format!("reservations-{suffix}.norito"));
        queue
            .install_plan_journal(
                dir.path().join(format!("queue-plans-{suffix}.norito")),
                1024 * 1024,
                true,
            )
            .expect("install globally certified reservation plan journal");
        queue
            .install_lane_reservation_journal(path, 1024 * 1024)
            .expect("install reservation journal");
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, first.clone());
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, second.clone());
        let mut global = Vec::new();
        let reserved = if lane_first {
            let reserved = queue
                .reserve_transactions_for_lane(
                    &state,
                    lane_reservation_scope(&state, b"order-owner", suffix.as_bytes()),
                    nonzero!(1_usize),
                )
                .expect("lane-first reservation");
            queue.get_transactions_for_block_with_state(&state, nonzero!(1_usize), &mut global);
            reserved
        } else {
            queue.get_transactions_for_block_with_state(&state, nonzero!(1_usize), &mut global);
            queue
                .reserve_transactions_for_lane(
                    &state,
                    lane_reservation_scope(&state, b"order-owner", suffix.as_bytes()),
                    nonzero!(1_usize),
                )
                .expect("global-first reservation")
        };
        let global_hash = global[0].as_ref().hash();
        let reserved_hash = reserved[0].as_accepted().hash();
        assert_ne!(global_hash, reserved_hash);
        assert_eq!(BTreeSet::from([global_hash, reserved_hash]), all_hashes);
    };

    run(true, "lane-first");
    run(false, "global-first");
}

#[test]
fn fee_capacity_reservations_prevent_queue_oversubscription() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_hash = accepted_tx_by_someone(&time_source).hash();
    let second_hash = accepted_tx_by_someone(&time_source).hash();
    assert_ne!(
        first_hash, second_hash,
        "fixtures must identify two transactions"
    );

    let (sponsor, _) = gen_account_in("sponsor");
    let (beneficiary, _) = gen_account_in("beneficiary");
    let program_id = FeeSponsorProgramId::new(
        sponsor,
        "queue_capacity".parse().expect("valid program name"),
    );
    let asset_definition_id = AssetDefinitionId::new(
        DomainId::try_new("fees", "universal").expect("valid fee domain"),
        "xor".parse().expect("valid asset name"),
    );
    let amount = Quantity::from(6_u32);
    let remaining = Quantity::from(10_u32);

    let reservation = || {
        let block_key = FeeSponsorBudgetCounterKey {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
            window: FeeSponsorBudgetWindow::Block(FeeSponsorBlockBudgetWindow { height: 7 }),
        };
        let program_epoch_key = FeeSponsorBudgetCounterKey {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
            window: FeeSponsorBudgetWindow::ProgramEpoch(FeeSponsorProgramEpochBudgetWindow {
                epoch: 1,
            }),
        };
        let beneficiary_epoch_key = FeeSponsorBudgetCounterKey {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
            window: FeeSponsorBudgetWindow::BeneficiaryEpoch(
                FeeSponsorBeneficiaryEpochBudgetWindow {
                    epoch: 1,
                    beneficiary: beneficiary.clone(),
                },
            ),
        };
        let source = FeeReservationAssetSource::SponsorProgram {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
        };
        FeeAdmissionReservation {
            program_revision: Some(1),
            beneficiary: beneficiary.clone(),
            asset_charges: BTreeMap::from([(source.clone(), amount.clone())]),
            window_charges: BTreeMap::from([
                (block_key.clone(), amount.clone()),
                (program_epoch_key.clone(), amount.clone()),
                (beneficiary_epoch_key.clone(), amount.clone()),
            ]),
            relay_lease_charges: BTreeMap::new(),
            asset_remaining: BTreeMap::from([(source, Quantity::from(100_u32))]),
            window_remaining: BTreeMap::from([
                (block_key, remaining.clone()),
                (program_epoch_key, remaining.clone()),
                (beneficiary_epoch_key, remaining.clone()),
            ]),
            relay_lease_remaining: BTreeMap::new(),
        }
    };

    let mut store = FeeAdmissionReservationStore::default();
    store
        .reserve(first_hash, reservation())
        .expect("first transaction reserves the shared capacity");
    let err = store
        .reserve(second_hash, reservation())
        .expect_err("second transaction must not overbook the same snapshot");
    assert!(matches!(
        err,
        Error::NexusFeeAdmissionRejected {
            code: FeeRejectionCode::ProgramBlockBudgetExhausted,
            ..
        }
    ));

    store.release(&first_hash);
    store
        .reserve(second_hash, reservation())
        .expect("released capacity is immediately reusable");
}

#[test]
fn fee_reservation_refresh_moves_carried_transaction_to_current_block_window() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_hash = accepted_tx_by_someone(&time_source).hash();
    let second_hash = accepted_tx_by_someone(&time_source).hash();
    let (sponsor, _) = gen_account_in("refresh_fee_reservation");
    let (beneficiary, _) = gen_account_in("refresh_fee_beneficiary");
    let program_id =
        FeeSponsorProgramId::new(sponsor, "rollover".parse().expect("valid program name"));
    let asset_definition_id = AssetDefinitionId::new(
        DomainId::try_new("fees", "universal").expect("valid fee domain"),
        "xor".parse().expect("valid asset name"),
    );
    let reservation_at = |height| {
        let key = FeeSponsorBudgetCounterKey {
            program_id: program_id.clone(),
            asset_definition_id: asset_definition_id.clone(),
            window: FeeSponsorBudgetWindow::Block(FeeSponsorBlockBudgetWindow { height }),
        };
        FeeAdmissionReservation {
            program_revision: Some(1),
            beneficiary: beneficiary.clone(),
            asset_charges: BTreeMap::new(),
            window_charges: BTreeMap::from([(key.clone(), Quantity::from(6_u32))]),
            relay_lease_charges: BTreeMap::new(),
            asset_remaining: BTreeMap::new(),
            window_remaining: BTreeMap::from([(key, Quantity::from(10_u32))]),
            relay_lease_remaining: BTreeMap::new(),
        }
    };

    let mut store = FeeAdmissionReservationStore::default();
    store
        .reserve(first_hash, reservation_at(7))
        .expect("transaction reserves its enqueue-height window");
    store
        .refresh(first_hash, Some(reservation_at(8)))
        .expect("pop-time recheck moves the hold to the execution-height window");

    let err = store
        .reserve(second_hash, reservation_at(8))
        .expect_err("a competing transaction must see the refreshed current-height hold");
    assert!(matches!(
        err,
        Error::NexusFeeAdmissionRejected {
            code: FeeRejectionCode::ProgramBlockBudgetExhausted,
            ..
        }
    ));

    store
        .refresh(first_hash, None)
        .expect("disabling fee charging releases the stale hold");
    store
        .reserve(second_hash, reservation_at(8))
        .expect("released current-height capacity is reusable");
}

#[test]
fn unsigned_payload_routing_matches_signed_queue_admission_routing() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let tx = accepted_tx_by_someone(&time_source);
    let payload = tx
        .external()
        .expect("external transaction fixture")
        .payload()
        .clone();
    let state = State::new_for_testing(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let queue = Queue::test(config_factory(), &time_source);

    let signed = queue
        .route_plan_with_state(&tx, &state)
        .expect("signed route");
    let unsigned = queue
        .route_payload_plan_with_state(&payload, &state)
        .expect("unsigned route");
    assert_eq!(unsigned, signed);
}

#[test]
fn receipt_settled_queue_admission_rejects_authority_payer() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let (authority, keypair) = gen_account_in("receipt_fee_admission");
    let domain_id =
        DomainId::try_new("receipt_fee_admission", "universal").expect("receipt fee domain");
    let fee_asset = AssetDefinitionId::new(
        domain_id.clone(),
        "xor".parse().expect("receipt fee asset name"),
    );
    let world = World::with(
        [Domain::new(domain_id).build(&authority)],
        [Account::new(authority.clone()).build(&authority)],
        [AssetDefinition::numeric(fee_asset.clone())
            .with_name("receipt fee XOR".to_owned())
            .build(&authority)],
    );
    let state = State::new_for_testing(
        world,
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    {
        let mut nexus = state.nexus.write();
        nexus.enabled = true;
        nexus.fees.settlement_mode =
            iroha_config::parameters::actual::NexusFeeSettlementMode::LaneRelayBurn;
        nexus.fees.fee_asset_id = fee_asset.canonical_address();
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
    }
    let queue = Queue::test(config_factory(), &time_source);
    let transaction = accepted_tx_by(authority, &keypair, &time_source);

    let error = queue
        .push(transaction, state.view())
        .expect_err("receipt-settled queue admission must require a sponsor");

    assert!(matches!(
        error.err,
        Error::NexusFeeAdmissionRejected {
            code: FeeRejectionCode::RelayCapacityUnavailable,
            ref reason,
        } if reason.contains("active fee sponsor program")
            && reason.contains("exact active revision")
    ));
    assert_eq!(queue.active_len(), 0);
}

#[test]
fn authority_fee_reservations_prevent_overbooking_and_release_capacity() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_hash = accepted_tx_by_someone(&time_source).hash();
    let second_hash = accepted_tx_by_someone(&time_source).hash();
    let (authority, _) = gen_account_in("authority_fee_reservation");
    let asset_definition_id = AssetDefinitionId::new(
        DomainId::try_new("fees", "universal").expect("valid fee domain"),
        "xor".parse().expect("valid asset name"),
    );
    let asset_id = AssetId::new(asset_definition_id, authority.clone());
    let source = FeeReservationAssetSource::Authority(asset_id);
    let reservation = || FeeAdmissionReservation {
        program_revision: None,
        beneficiary: authority.clone(),
        asset_charges: BTreeMap::from([(source.clone(), Quantity::from(6_u32))]),
        window_charges: BTreeMap::new(),
        relay_lease_charges: BTreeMap::new(),
        asset_remaining: BTreeMap::from([(source.clone(), Quantity::from(10_u32))]),
        window_remaining: BTreeMap::new(),
        relay_lease_remaining: BTreeMap::new(),
    };

    let mut store = FeeAdmissionReservationStore::default();
    store
        .reserve(first_hash, reservation())
        .expect("first authority transaction reserves its balance");
    let err = store
        .reserve(second_hash, reservation())
        .expect_err("second authority transaction must not overbook the balance");
    assert!(matches!(
        err,
        Error::NexusFeeAdmissionRejected {
            code: FeeRejectionCode::AuthorityPayerInsufficient,
            ..
        }
    ));

    store.release(&first_hash);
    store
        .reserve(second_hash, reservation())
        .expect("released authority capacity is immediately reusable");
}

#[test]
fn relay_spend_lease_reservations_prevent_overbooking_and_release_capacity() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let first_hash = accepted_tx_by_someone(&time_source).hash();
    let second_hash = accepted_tx_by_someone(&time_source).hash();
    let (beneficiary, _) = gen_account_in("relay_lease_reservation");
    let lease_id = Hash::new(b"exact verified sponsor spend lease");
    let reservation = || FeeAdmissionReservation {
        program_revision: Some(3),
        beneficiary: beneficiary.clone(),
        asset_charges: BTreeMap::new(),
        window_charges: BTreeMap::new(),
        relay_lease_charges: BTreeMap::from([(lease_id, Quantity::from(6_u32))]),
        asset_remaining: BTreeMap::new(),
        window_remaining: BTreeMap::new(),
        relay_lease_remaining: BTreeMap::from([(lease_id, Quantity::from(10_u32))]),
    };

    let mut store = FeeAdmissionReservationStore::default();
    store
        .reserve(first_hash, reservation())
        .expect("first transaction reserves exact lease capacity");
    let err = store
        .reserve(second_hash, reservation())
        .expect_err("second transaction must not overbook the exact lease");
    assert!(matches!(
        err,
        Error::NexusFeeAdmissionRejected {
            code: FeeRejectionCode::RelayCapacityUnavailable,
            ..
        }
    ));

    store.release(&first_hash);
    store
        .reserve(second_hash, reservation())
        .expect("released lease capacity is immediately reusable");
}

#[test]
fn relay_spend_lease_reservation_maps_use_aggregate_per_asset_charges() {
    let (sponsor, _) = gen_account_in("relay_lease_map_sponsor");
    let program_id =
        FeeSponsorProgramId::new(sponsor, "relay_maps".parse().expect("valid program name"));
    let domain_id = DomainId::try_new("relay_lease_maps", "universal").expect("valid fee domain");
    let shared_asset = AssetDefinitionId::new(
        domain_id.clone(),
        "shared".parse().expect("valid shared asset name"),
    );
    let distinct_asset = AssetDefinitionId::new(
        domain_id,
        "distinct".parse().expect("valid distinct asset name"),
    );
    let shared_lease = Hash::new(b"queue-shared-asset-spend-lease");
    let distinct_lease = Hash::new(b"queue-distinct-asset-spend-lease");
    let component_charges = [
        FeeChargeBound {
            kind: iroha_data_model::transaction::FeeChargeKind::Nexus,
            asset_definition_id: shared_asset.clone(),
            max_bound: Quantity::from(4_u32),
        },
        FeeChargeBound {
            kind: iroha_data_model::transaction::FeeChargeKind::PipelineGas,
            asset_definition_id: shared_asset.clone(),
            max_bound: Quantity::from(6_u32),
        },
    ];
    let sponsor_charges = sponsored_charge_totals(&component_charges)
        .expect("Nexus and PipelineGas charges aggregate by fee asset");
    assert_eq!(sponsor_charges[&shared_asset], Quantity::from(10_u32));
    let sponsor_charges = BTreeMap::from([
        (shared_asset.clone(), sponsor_charges[&shared_asset].clone()),
        (distinct_asset.clone(), Quantity::from(7_u32)),
    ]);
    let selections = BTreeMap::from([
        (
            shared_asset,
            FeeSponsorRelayLeaseCapacity {
                lease_id: shared_lease,
                remaining: Quantity::from(15_u32),
            },
        ),
        (
            distinct_asset,
            FeeSponsorRelayLeaseCapacity {
                lease_id: distinct_lease,
                remaining: Quantity::from(8_u32),
            },
        ),
    ]);

    let (charges, remaining) =
        relay_lease_reservation_maps(&program_id, &sponsor_charges, selections)
            .expect("each charged asset maps to its exact selected lease");

    assert_eq!(charges[&shared_lease], Quantity::from(10_u32));
    assert_eq!(charges[&distinct_lease], Quantity::from(7_u32));
    assert_eq!(remaining[&shared_lease], Quantity::from(15_u32));
    assert_eq!(remaining[&distinct_lease], Quantity::from(8_u32));
}
