include!("lane_reservation_core_tests.rs"); // Preserve stable `queue::tests` libtest paths.
#[test]
fn release_recomputes_fifo_after_unrelated_admission_during_append() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let reserved_transaction = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let reserved_hash = reserved_transaction.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, reserved_transaction);
    let key = *queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(
                &state,
                b"release-admission-owner",
                b"release-admission-proposal",
            ),
            nonzero!(1_usize),
        )
        .expect("reserve release-race transaction")[0]
        .key();
    let unrelated = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let unrelated_hash = unrelated.hash_as_entrypoint();
    let reached = Arc::new(Barrier::new(2));
    let resume = Arc::new(Barrier::new(2));
    queue
        .lane_reservation_journal
        .lock()
        .as_mut()
        .expect("installed reservation journal")
        .install_append_handoff(Arc::clone(&reached), Arc::clone(&resume));
    thread::scope(|scope| {
        let queue_for_release = Arc::clone(&queue);
        let release = scope.spawn(move || queue_for_release.release_lane_reservation(&key));
        reached.wait();
        assert!(queue.durability_transition_active(&reserved_hash));
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, unrelated);
        {
            let _queue_guard = queue.push_remove_lock.lock();
            assert_eq!(queue.fifo_snapshot_locked(), vec![unrelated_hash]);
        }
        resume.wait();
        assert_eq!(
            release
                .join()
                .expect("release thread")
                .expect("release after unrelated admission"),
            LaneQueueReservationOutcome::Finalized
        );
    });
    {
        let _queue_guard = queue.push_remove_lock.lock();
        assert_eq!(
            queue.fifo_snapshot_locked(),
            vec![reserved_hash, unrelated_hash],
            "post-journal publication must merge an unrelated concurrent admission by durable ordinal"
        );
    }
    assert_eq!(queue.active_len(), 2);
    assert_eq!(queue.queued_len(), 2);
    assert!(!queue.transaction_selection_durability_faulted());
}
#[test]
fn release_recomputes_fifo_while_unrelated_pop_is_held() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let reserved_transaction = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let reserved_hash = reserved_transaction.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, reserved_transaction);
    let unrelated = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let unrelated_hash = unrelated.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, unrelated);
    let key = *queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(&state, b"release-pop-owner", b"release-pop-proposal"),
            nonzero!(1_usize),
        )
        .expect("reserve release-race transaction")[0]
        .key();
    let reached = Arc::new(Barrier::new(2));
    let resume = Arc::new(Barrier::new(2));
    queue
        .lane_reservation_journal
        .lock()
        .as_mut()
        .expect("installed reservation journal")
        .install_append_handoff(Arc::clone(&reached), Arc::clone(&resume));
    thread::scope(|scope| {
        let queue_for_release = Arc::clone(&queue);
        let release = scope.spawn(move || queue_for_release.release_lane_reservation(&key));
        reached.wait();
        let held = queue.collect_transactions_for_block(&state.view(), nonzero!(1_usize));
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].as_ref().hash_as_entrypoint(), unrelated_hash);
        assert!(queue.tx_hashes.is_empty());
        resume.wait();
        assert_eq!(
            release
                .join()
                .expect("release thread")
                .expect("release while unrelated pop is held"),
            LaneQueueReservationOutcome::Finalized
        );
        {
            let _queue_guard = queue.push_remove_lock.lock();
            assert_eq!(
                queue.fifo_snapshot_locked(),
                vec![reserved_hash],
                "post-journal publication must not resurrect an unrelated held pop"
            );
        }
        drop(held);
    });
    {
        let _queue_guard = queue.push_remove_lock.lock();
        assert_eq!(
            queue.fifo_snapshot_locked(),
            vec![reserved_hash, unrelated_hash],
            "dropping the held guard restores its own durable FIFO position"
        );
    }
    assert!(!queue.transaction_selection_durability_faulted());
}
#[test]
fn global_candidate_lease_excludes_autonomous_reservation_until_exact_drop() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let transaction = accepted_queue_plan_tx_by_someone(&time_source);
    let hash = transaction.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
    let (snapshot, lease) = queue
        .bounded_pending_snapshot(&state.view(), nonzero!(1_usize))
        .expect("global selection remains healthy");
    assert_eq!(
        snapshot
            .iter()
            .map(AcceptedTransaction::hash_as_entrypoint)
            .collect::<Vec<_>>(),
        vec![hash]
    );
    assert!(
        queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(
                    &state,
                    b"leased-autonomous-owner",
                    b"leased-autonomous-proposal",
                ),
                nonzero!(1_usize),
            )
            .expect("leased hash is skipped rather than conflicted")
            .is_empty(),
        "an autonomous slot must not reserve globally selected work"
    );
    drop(lease);
    let reserved = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(
                &state,
                b"released-autonomous-owner",
                b"released-autonomous-proposal",
            ),
            nonzero!(1_usize),
        )
        .expect("dropping the exact global lease restores autonomous eligibility");
    assert_eq!(reserved.len(), 1);
    assert_eq!(reserved[0].key().entrypoint_hash, hash);
}
#[test]
fn lane_reservation_group_diagnostics_follow_durable_commit_forget_boundary() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    let transaction = accepted_queue_plan_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(
        Arc::get_mut(&mut state).expect("unshared lane-reservation test state"),
        &transaction,
    );
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
    let scope = lane_reservation_scope(&state, b"diagnostic-owner", b"diagnostic-proposal");
    assert!(
        !queue.lane_reservation_group_is_finalized_for_diagnostics(&[]),
        "an empty identity group cannot prove queue finalization"
    );
    install_test_reservation_journal(&queue, &dir);
    let key = *queue
        .reserve_transactions_for_lane(&state, scope, nonzero!(1_usize))
        .expect("reserve diagnostic transaction")[0]
        .key();
    assert!(
        !queue.lane_reservation_group_is_finalized_for_diagnostics(&[key]),
        "live ownership must block the terminal diagnostic stage"
    );
    assert!(
        !queue.lane_reservation_group_is_finalized_for_diagnostics(&[key, key]),
        "duplicate identities cannot prove group finalization"
    );
    queue.hold_next_lane_reservation_commit_after_barrier_for_test();
    assert_eq!(
        queue
            .commit_lane_reservation_for_test(&key)
            .expect("commit diagnostic reservation"),
        LaneQueueReservationOutcome::Finalized
    );
    assert!(
        !queue.lane_reservation_group_is_finalized_for_diagnostics(&[key]),
        "a durable Commit barrier must remain visible before ForgetCommit"
    );
    assert_eq!(queue.lane_reservation_commit_barriers(), vec![key]);
    assert_eq!(
        queue
            .commit_lane_reservation_for_test(&key)
            .expect("retry the diagnostic reservation through ForgetCommit"),
        LaneQueueReservationOutcome::AlreadyFinalized
    );
    assert!(
        queue.lane_reservation_group_is_finalized_for_diagnostics(&[key]),
        "the synced queue-plan tombstone and reservation ForgetCommit prove completion"
    );
    let mut malformed = key;
    malformed.version = 0;
    assert!(
        !queue.lane_reservation_group_is_finalized_for_diagnostics(&[malformed]),
        "malformed identities must fail closed"
    );
}
#[test]
fn globally_admitted_transaction_commits_from_a_later_reservation_height() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Queue::test(config_factory(), &time_source);
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let binding = push_globally_bound_lane_reservation_candidate(
        &queue,
        &state,
        &dir,
        accepted_queue_plan_tx_by_someone(&time_source),
    );
    assert_eq!(
        binding.admission_context.proposal_height, 1,
        "the durable admission is certified for the first proposal height"
    );
    seed_committed_height_for_queue_test(&state, 4);
    let mut later_scope = lane_reservation_scope(
        &state,
        b"delayed-reservation-owner",
        b"delayed-reservation-proposal",
    );
    later_scope.proposal_height = 5;
    later_scope.lane_incarnation = state
        .lane_incarnation_at_height(LaneId::SINGLE, later_scope.proposal_height)
        .expect("the canonical lane remains active at the later proposal height");
    let reserved = queue
        .reserve_transactions_for_lane(&state, later_scope, nonzero!(1_usize))
        .expect("reserve the still-owned globally admitted transaction at a later height");
    assert_eq!(reserved.len(), 1);
    let key = *reserved[0].key();
    assert_eq!(key.proposal_height, 5);
    assert_ne!(
        key.proposal_height, binding.admission_context.proposal_height,
        "the reservation slot height and admission-certification height are distinct domains"
    );
    assert_eq!(
        queue
            .commit_lane_reservation_for_test(&key)
            .expect("the later exact reservation must consume its durable admission claim"),
        LaneQueueReservationOutcome::Finalized
    );
    assert!(!queue.transaction_selection_durability_faulted());
}
#[test]
fn lane_reservation_group_diagnostics_rechecks_fault_after_store_lock_handoff() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_test_reservation_journal(&queue, &dir);
    push_globally_bound_lane_reservation_candidate(
        &queue,
        &state,
        &dir,
        accepted_queue_plan_tx_by_someone(&time_source),
    );
    let key = *queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(
                &state,
                b"diagnostic-handoff-owner",
                b"diagnostic-handoff-proposal",
            ),
            nonzero!(1_usize),
        )
        .expect("reserve diagnostic handoff transaction")[0]
        .key();
    queue
        .commit_lane_reservation_for_test(&key)
        .expect("commit diagnostic handoff reservation");
    assert!(
        queue.lane_reservation_group_is_finalized_for_diagnostics(&[key]),
        "healthy synced plan/reservation journals must initially prove finalization"
    );
    let reached = Arc::new(Barrier::new(2));
    let resume = Arc::new(Barrier::new(2));
    *queue.durability_observer_lock_handoff.lock() = Some(QueueDurabilityObserverLockHandoff {
        reached: Arc::clone(&reached),
        resume: Arc::clone(&resume),
    });
    let reservation_guard = queue.lane_reservations.lock();
    let observer_queue = Arc::clone(&queue);
    let observer = std::thread::spawn(move || {
        observer_queue.lane_reservation_group_is_finalized_for_diagnostics(&[key])
    });
    reached.wait();
    queue
        .plan_journal_durability_fault
        .store(true, Ordering::Release);
    resume.wait();
    drop(reservation_guard);
    assert!(
        !observer.join().expect("join diagnostic handoff observer"),
        "a fault published after the optimistic precheck must fail the protected snapshot closed"
    );
}
#[test]
fn durable_reservation_diagnostics_hash_exact_fifo_group_and_reconstruct_after_restart() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let reservation_path;
    let active_route = [(
        LaneId::SINGLE,
        DataSpaceId::UNIVERSAL,
        state
            .lane_incarnation_at_height(LaneId::SINGLE, 1)
            .expect("active default-lane incarnation"),
    )];
    let expected;
    {
        let queue = Queue::test(config_factory(), &time_source);
        reservation_path = install_globally_certified_test_reservation_journals(&queue, &dir);
        for _ in 0..2 {
            push_globally_bound_lane_reservation_candidate(
                &queue,
                &state,
                &dir,
                accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source),
            );
        }
        let reserved = queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(
                    &state,
                    b"durable-diagnostic-owner",
                    b"durable-diagnostic-provisional-slot",
                ),
                nonzero!(2_usize),
            )
            .expect("reserve exact diagnostic group");
        let ordered_keys = reserved
            .iter()
            .map(|reservation| *reservation.key())
            .collect::<Vec<_>>();
        let expected_binding =
            lane_queue_reservation_group_binding_from_ordered_keys(ordered_keys.iter())
                .expect("reserved FIFO keys form one exact group");
        let no_keys = Vec::<LaneQueueReservationKeyV1>::new();
        assert_eq!(
            lane_queue_reservation_group_binding_from_ordered_keys(no_keys.iter()),
            Err("lane queue reservation diagnostics group must not be empty")
        );
        let duplicate = [ordered_keys[0], ordered_keys[0]];
        assert_eq!(
            lane_queue_reservation_group_binding_from_ordered_keys(duplicate.iter()),
            Err("lane queue reservation diagnostics group contains a duplicate key")
        );
        let mut mixed = ordered_keys.clone();
        mixed[1].proposal_identity_hash = Hash::new(b"mixed-diagnostic-proposal-identity");
        assert_eq!(
            lane_queue_reservation_group_binding_from_ordered_keys(mixed.iter()),
            Err("lane queue reservation diagnostics group mixes proposal-slot identities")
        );
        let snapshot = queue
            .lane_reservation_diagnostic_groups_bounded(&active_route, nonzero!(1_usize))
            .expect("derive bounded Queue reservation diagnostics");
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].binding, expected_binding);
        assert_eq!(snapshot[0].binding.reservation_count, 2);
        assert!(!snapshot[0].conflict);
        assert_eq!(
            snapshot[0].binding.reservation_group_hash,
            lane_queue_reservation_group_binding_from_ordered_keys(ordered_keys.iter())
                .expect("repeat exact group hash")
                .reservation_group_hash
        );
        assert_ne!(
            snapshot[0].binding.reservation_group_hash,
            lane_queue_reservation_group_binding_from_ordered_keys(ordered_keys.iter().rev())
                .expect("reverse group remains structurally valid")
                .reservation_group_hash,
            "the durable group digest must bind FIFO order"
        );
        expected = snapshot;
    }
    let restarted = Queue::test(config_factory(), &time_source);
    let replay = restarted
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("replay reservation diagnostics journal after restart");
    assert_eq!(replay.restored, 2);
    assert_eq!(replay.awaiting_transaction_replay, 2);
    assert!(
        restarted.txs.is_empty(),
        "durable reservation diagnostics must not require or materialize transaction payloads"
    );
    assert_eq!(
        restarted
            .lane_reservation_diagnostic_groups_bounded(&active_route, nonzero!(1_usize))
            .expect("reconstruct exact reservation diagnostics from replay"),
        expected
    );
}
#[test]
fn durable_reservation_group_binding_rejects_oversized_membership() {
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let incarnation = Hash::new(b"oversized-diagnostic-group-incarnation");
    let keys = (0..=crate::lane_consensus::MAX_LANE_EXECUTABLE_ENTRYPOINTS)
        .map(|index| {
            let fifo_ordinal = u64::try_from(index)
                .expect("diagnostic group index fits u64")
                .saturating_add(1);
            payload_free_diagnostic_reservation_record(
                route,
                incarnation,
                1,
                1,
                fifo_ordinal,
                b"oversized-diagnostic-group",
            )
            .key
        })
        .collect::<Vec<_>>();
    assert_eq!(
        lane_queue_reservation_group_binding_from_ordered_keys(keys.iter()),
        Err("lane queue reservation diagnostics group exceeds its protocol bound")
    );
}
#[test]
fn durable_reservation_diagnostics_are_bounded_and_report_same_slot_conflicts() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let queue = Queue::test(config_factory(), &time_source);
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let incarnation = state
        .lane_incarnation_at_height(LaneId::SINGLE, 1)
        .expect("active default-lane incarnation");
    let active_route = [(LaneId::SINGLE, DataSpaceId::UNIVERSAL, incarnation)];
    for lane_block_height in 1..=3 {
        push_globally_bound_lane_reservation_candidate(
            &queue,
            &state,
            &dir,
            accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source),
        );
        let mut scope = lane_reservation_scope(
            &state,
            format!("bounded-diagnostic-owner-{lane_block_height}").as_bytes(),
            format!("bounded-diagnostic-slot-{lane_block_height}").as_bytes(),
        );
        scope.lane_block_height = lane_block_height;
        queue
            .reserve_transactions_for_lane(&state, scope, nonzero!(1_usize))
            .expect("reserve bounded diagnostic group");
    }
    let bounded = queue
        .lane_reservation_diagnostic_groups_bounded(&active_route, nonzero!(2_usize))
        .expect("derive limit-plus-one bounded diagnostics");
    assert_eq!(bounded.len(), 2, "overflow sentinel must never escape");
    assert_eq!(
        bounded
            .iter()
            .map(|summary| summary.binding.identity.lane_block_height)
            .collect::<Vec<_>>(),
        vec![2, 3],
        "the deterministic bounded suffix must retain the newest groups"
    );
    for identity_seed in [b"same-slot-a".as_slice(), b"same-slot-b".as_slice()] {
        push_globally_bound_lane_reservation_candidate(
            &queue,
            &state,
            &dir,
            accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source),
        );
        let mut scope = lane_reservation_scope(&state, identity_seed, identity_seed);
        scope.lane_block_height = 4;
        queue
            .reserve_transactions_for_lane(&state, scope, nonzero!(1_usize))
            .expect("reserve conflicting same-slot diagnostic group");
    }
    let conflicts = queue
        .lane_reservation_diagnostic_groups_bounded(&active_route, nonzero!(8_usize))
        .expect("derive conflict-aware diagnostics");
    let same_slot = conflicts
        .iter()
        .filter(|summary| summary.binding.identity.lane_block_height == 4)
        .collect::<Vec<_>>();
    assert_eq!(same_slot.len(), 2);
    assert!(
        same_slot.iter().all(|summary| summary.conflict),
        "every durable identity claiming one lane-local slot must report conflict"
    );
}
#[test]
fn durable_reservation_diagnostics_fairly_bound_routes_after_payload_free_restart() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let dir = tempdir().expect("tempdir");
    let busy_route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let quiet_route = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
    let busy_incarnation = Hash::new(b"diagnostic-fair-busy-incarnation");
    let quiet_incarnation = Hash::new(b"diagnostic-fair-quiet-incarnation");
    let inactive_incarnation = Hash::new(b"diagnostic-fair-inactive-incarnation");
    let writer = Queue::test(config_factory(), &time_source);
    let reservation_path = install_test_reservation_journal(&writer, &dir);
    let mut records = (1..=4_u64)
        .map(|height| {
            payload_free_diagnostic_reservation_record(
                busy_route,
                busy_incarnation,
                height,
                height,
                height,
                &height.to_be_bytes(),
            )
        })
        .collect::<Vec<_>>();
    records.push(payload_free_diagnostic_reservation_record(
        quiet_route,
        quiet_incarnation,
        1,
        1,
        5,
        b"quiet-current",
    ));
    records.push(payload_free_diagnostic_reservation_record(
        quiet_route,
        inactive_incarnation,
        9,
        9,
        6,
        b"quiet-inactive",
    ));
    {
        let mut journal = writer.lane_reservation_journal.lock();
        let journal = journal.as_mut().expect("installed reservation journal");
        for record in records {
            journal
                .put_batch(vec![record])
                .expect("persist payload-free diagnostic reservation");
        }
    }
    drop(writer);
    let restarted = Queue::test(config_factory(), &time_source);
    let replay = restarted
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("replay multi-route diagnostic reservations");
    assert_eq!(replay.restored, 6);
    assert_eq!(replay.awaiting_transaction_replay, 6);
    assert!(
        restarted.txs.is_empty(),
        "the payload-free diagnostic projection must not replay transaction bodies"
    );
    let active_routes = [
        (
            busy_route.lane_id,
            busy_route.dataspace_id,
            busy_incarnation,
        ),
        (
            quiet_route.lane_id,
            quiet_route.dataspace_id,
            quiet_incarnation,
        ),
    ];
    let fair = restarted
        .lane_reservation_diagnostic_groups_bounded(&active_routes, nonzero!(2_usize))
        .expect("derive a fair two-route diagnostic bound");
    assert_eq!(fair.len(), 2);
    assert_eq!(
        fair.iter()
            .map(|summary| {
                (
                    summary.binding.identity.lane_id,
                    summary.binding.identity.lane_incarnation,
                    summary.binding.identity.lane_block_height,
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (busy_route.lane_id, busy_incarnation, 4),
            (quiet_route.lane_id, quiet_incarnation, 1),
        ],
        "one busy route must not hide the newest durable group of another active route"
    );
    assert!(
        fair.iter()
            .all(|summary| { summary.binding.identity.lane_incarnation != inactive_incarnation }),
        "an inactive incarnation must never enter the active-route projection"
    );
    let reversed = [active_routes[1], active_routes[0]];
    assert_eq!(
        restarted
            .lane_reservation_diagnostic_groups_bounded(&reversed, nonzero!(2_usize))
            .expect("route input order must not affect diagnostics"),
        fair
    );
    let over_limit_routes = [
        active_routes[0],
        active_routes[1],
        (
            LaneId::new(2),
            DataSpaceId::new(8),
            Hash::new(b"diagnostic-fair-third-incarnation"),
        ),
    ];
    assert!(matches!(
        restarted.lane_reservation_diagnostic_groups_bounded(
            &over_limit_routes,
            nonzero!(2_usize),
        ),
        Err(LaneQueueReservationError::InvalidIdentity(reason))
            if reason.contains("route input exceeds its row bound")
    ));
    restarted
        .plan_journal_durability_fault
        .store(true, Ordering::Release);
    assert!(
        matches!(
            restarted
                .lane_reservation_diagnostic_groups_bounded(&active_routes, nonzero!(2_usize),),
            Err(LaneQueueReservationError::DurabilityFault)
        ),
        "a durability fault must fail the bounded Queue projection closed"
    );
}
fn lane_reservation_release_barrier(
    keys: Vec<LaneQueueReservationKeyV1>,
    retirement_seed: &[u8],
) -> LaneQueueReservationReleaseBarrierV1 {
    let first = keys.first().expect("release barrier needs a reservation");
    LaneQueueReservationReleaseBarrierV1 {
        version: LaneQueueReservationReleaseBarrierV1::VERSION,
        network_id: super::queue_test_network_id(),
        epoch: 7,
        lane_id: first.lane_id,
        dataspace_id: first.dataspace_id,
        lane_incarnation: first.lane_incarnation,
        proposal_height: first.proposal_height,
        lane_block_height: first.lane_block_height,
        lane_block_view: first.lane_block_view,
        origin_descriptor_hash: Hash::new(b"queue-release-descriptor"),
        origin_proposal_hash: Hash::new(b"queue-release-proposal"),
        executable_payload_hash: Hash::new(b"queue-release-payload"),
        retirement_hash: Hash::new(retirement_seed),
        ordered_keys: keys,
    }
}
#[test]
fn ordered_release_barrier_is_nonselectable_idempotent_and_aba_safe() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    for _ in 0..2 {
        push_globally_bound_lane_reservation_candidate(
            &queue,
            &state,
            &dir,
            accepted_queue_plan_tx_by_someone(&time_source),
        );
    }
    let reserved = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(&state, b"release-owner", b"release-proposal"),
            nonzero!(2_usize),
        )
        .expect("reserve ordered release batch");
    let keys = reserved.iter().map(|tx| *tx.key()).collect::<Vec<_>>();
    let barrier = lane_reservation_release_barrier(keys.clone(), b"retirement-a");
    let barrier_digest = barrier.digest();
    {
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_eq!(
            barrier.digest(),
            barrier_digest,
            "release-barrier identity must ignore the caller's ambient Norito layout"
        );
    }
    assert_eq!(
        queue
            .prepare_lane_reservation_release_barrier(&barrier)
            .expect("prepare exact ordered release"),
        LaneQueueReservationOutcome::Finalized
    );
    assert_eq!(
        queue
            .prepare_lane_reservation_release_barrier(&barrier)
            .expect("retry exact ordered release"),
        LaneQueueReservationOutcome::Retained
    );
    let mut expected_live = keys.clone();
    expected_live.sort_by_key(LaneQueueReservationKeyV1::digest);
    assert_eq!(queue.live_lane_reservations(), expected_live);
    assert_eq!(queue.queued_len(), 0);
    assert!(matches!(
        queue.release_lane_reservation(&keys[0]),
        Err(LaneQueueReservationError::ReleaseConflict { .. })
    ));
    let mut global = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(2_usize), &mut global);
    assert!(
        global.is_empty(),
        "prepared release records must remain globally nonselectable"
    );
    let mut conflicting = barrier.clone();
    conflicting.retirement_hash = Hash::new(b"retirement-conflict");
    assert!(matches!(
        queue.prepare_lane_reservation_release_barrier(&conflicting),
        Err(LaneQueueReservationError::ReleaseConflict { .. })
    ));
    assert_eq!(
        queue
            .finalize_lane_reservation_release_barrier(&barrier)
            .expect("complete exact ordered release"),
        2
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert!(queue.lane_reservation_release_barriers().is_empty());
    assert_eq!(queue.queued_len(), 2);
    assert_eq!(
        queue
            .finalize_lane_reservation_release_barrier(&barrier)
            .expect("repeat completed ordered release"),
        0
    );
    let replacement = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(&state, b"replacement-owner", b"replacement-proposal"),
            nonzero!(1_usize),
        )
        .expect("reserve re-admitted FIFO owner");
    assert_eq!(replacement.len(), 1);
    assert_ne!(replacement[0].key(), &keys[0]);
    assert!(matches!(
        queue.prepare_lane_reservation_release_barrier(&barrier),
        Err(LaneQueueReservationError::Conflict { .. })
    ));
    assert_eq!(
        queue.live_lane_reservations(),
        vec![*replacement[0].key()],
        "a stale barrier must not disturb the replacement owner"
    );
}
#[test]
fn forgotten_release_requires_exact_fifo_membership_and_relative_order() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    for _ in 0..2 {
        push_globally_bound_lane_reservation_candidate(
            &queue,
            &state,
            &dir,
            accepted_queue_plan_tx_by_someone(&time_source),
        );
    }
    let reserved = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(&state, b"terminal-fifo-owner", b"terminal-fifo-proposal"),
            nonzero!(2_usize),
        )
        .expect("reserve terminal FIFO test batch");
    let keys = reserved.iter().map(|tx| *tx.key()).collect::<Vec<_>>();
    let barrier = lane_reservation_release_barrier(keys.clone(), b"terminal-fifo-retirement");
    queue
        .prepare_lane_reservation_release_barrier(&barrier)
        .expect("prepare terminal FIFO test release");
    assert_eq!(
        queue
            .finalize_lane_reservation_release_barrier(&barrier)
            .expect("complete terminal FIFO test release"),
        2
    );
    let first = keys[0].entrypoint_hash;
    let second = keys[1].entrypoint_hash;
    {
        let _queue_guard = queue.push_remove_lock.lock();
        assert_eq!(
            queue.remove_hashes_from_fifo_locked(&HashSet::from([first])),
            1
        );
    }
    for failure in [
        queue
            .prepare_lane_reservation_release_barrier(&barrier)
            .map(drop),
        queue
            .finalize_lane_reservation_release_barrier(&barrier)
            .map(drop),
    ] {
        assert!(matches!(
            failure,
            Err(LaneQueueReservationError::InvalidIdentity(ref message))
                if message.contains("lacks exact ordinary FIFO ownership")
        ));
    }
    {
        let _queue_guard = queue.push_remove_lock.lock();
        queue.replace_fifo_locked(&[second, first]);
    }
    assert!(matches!(
        queue.prepare_lane_reservation_release_barrier(&barrier),
        Err(LaneQueueReservationError::InvalidIdentity(ref message))
            if message.contains("lacks exact ordinary FIFO ownership")
    ));
    assert!(matches!(
        queue.finalize_lane_reservation_release_barrier(&barrier),
        Err(LaneQueueReservationError::InvalidIdentity(ref message))
            if message.contains("lacks exact ordinary FIFO ownership")
    ));
    {
        let _queue_guard = queue.push_remove_lock.lock();
        queue.replace_fifo_locked(&[first, second]);
    }
    assert_eq!(
        queue
            .prepare_lane_reservation_release_barrier(&barrier)
            .expect("exact terminal FIFO may remint a Queue proof"),
        LaneQueueReservationOutcome::AlreadyFinalized
    );
    assert_eq!(
        queue
            .finalize_lane_reservation_release_barrier(&barrier)
            .expect("exact terminal FIFO release retry is a stutter"),
        0
    );
    let original_plan = queue
        .routing_plans
        .get(&first)
        .expect("forgotten release retains its exact routing plan")
        .clone();
    assert!(queue.remove_routing_plan_for_test(first));
    for failure in [
        queue
            .prepare_lane_reservation_release_barrier(&barrier)
            .map(drop),
        queue
            .finalize_lane_reservation_release_barrier(&barrier)
            .map(drop),
    ] {
        assert!(matches!(
            failure,
            Err(LaneQueueReservationError::InvalidIdentity(ref message))
                if message.contains("lacks exact ordinary FIFO ownership")
        ));
    }
    let substituted_route = RoutingDecision::new(LaneId::new(99), DataSpaceId::new(99));
    queue
        .routing_plans
        .insert(first, RoutingPlan::single(substituted_route));
    assert!(matches!(
        queue.prepare_lane_reservation_release_barrier(&barrier),
        Err(LaneQueueReservationError::InvalidIdentity(ref message))
            if message.contains("lacks exact ordinary FIFO ownership")
    ));
    assert!(matches!(
        queue.finalize_lane_reservation_release_barrier(&barrier),
        Err(LaneQueueReservationError::InvalidIdentity(ref message))
            if message.contains("lacks exact ordinary FIFO ownership")
    ));
    queue.routing_plans.insert(first, original_plan);
}
#[test]
fn ordered_release_restart_retains_barrier_until_explicit_evidence_gated_finalize() {
    for crash_after_completion in [false, true] {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let state = lane_reservation_test_state();
        let dir = tempdir().expect("tempdir");
        let plan_path = dir.path().join("release-plans.norito");
        let reservation_path = dir.path().join("release-reservations.norito");
        let barrier = {
            let queue = Arc::new(Queue::test(config_factory(), &time_source));
            queue
                .install_plan_journal(&plan_path, 1024 * 1024, true)
                .expect("install plan journal");
            queue
                .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
                .expect("install reservation journal");
            for _ in 0..2 {
                push_globally_bound_lane_reservation_candidate(
                    &queue,
                    &state,
                    &dir,
                    accepted_queue_plan_tx_by_someone(&time_source),
                );
            }
            let reserved = queue
                .reserve_transactions_for_lane(
                    &state,
                    lane_reservation_scope(
                        &state,
                        b"restart-release-owner",
                        b"restart-release-proposal",
                    ),
                    nonzero!(2_usize),
                )
                .expect("reserve restart release batch");
            let barrier = lane_reservation_release_barrier(
                reserved.iter().map(|tx| *tx.key()).collect(),
                b"restart-retirement",
            );
            queue
                .prepare_lane_reservation_release_barrier(&barrier)
                .expect("persist prepared release");
            if crash_after_completion {
                let reservations = queue.lane_reservations.lock();
                let ordered_records = barrier
                    .ordered_keys
                    .iter()
                    .map(|key| reservations.live_by_entrypoint[&key.entrypoint_hash].clone())
                    .collect();
                let completion = LaneQueueReservationReleaseCompletionV1 {
                    version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
                    barrier: barrier.clone(),
                    ordered_records,
                };
                drop(reservations);
                queue
                    .lane_reservation_journal
                    .lock()
                    .as_mut()
                    .expect("reservation journal")
                    .complete_release(completion)
                    .expect("persist completion before simulated crash");
            }
            barrier
        };
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        let replay = queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("replay ordered release state");
        if crash_after_completion {
            assert_eq!(replay.restored, 0);
            assert_eq!(replay.release_barriers, 0);
            assert_eq!(replay.completed_releases, 1);
        } else {
            assert_eq!(replay.restored, 2);
            assert_eq!(replay.release_barriers, 1);
            assert_eq!(replay.completed_releases, 0);
        }
        assert_eq!(
            queue.lane_reservation_release_barriers(),
            vec![barrier.clone()]
        );
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install plan journal after release replay");
        assert_eq!(
            queue
                .replay_plan_journal(&state)
                .expect("replay exact released transactions")
                .replayed,
            2
        );
        let reconciliation_snapshot = queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture the complete replayed release ownership cut");
        let snapshot_replay_receipt = queue
            .lane_reservation_snapshot_replay_receipt()
            .expect("retain the exact checked release replay identity");
        assert!(
            snapshot_replay_receipt
                .binds_reconciliation_snapshot(&reconciliation_snapshot)
                .expect("validate the exact release reconciliation identity")
        );
        assert!(reconciliation_snapshot.commit_barriers.is_empty());
        assert_eq!(
            reconciliation_snapshot.release_barriers(),
            vec![barrier.clone()]
        );
        if crash_after_completion {
            assert!(reconciliation_snapshot.ordered_records.is_empty());
            assert!(reconciliation_snapshot.prepared_release_barriers.is_empty());
            assert_eq!(reconciliation_snapshot.completed_releases.len(), 1);
            assert_eq!(
                reconciliation_snapshot.completed_releases[0].barrier,
                barrier
            );
            assert_eq!(
                reconciliation_snapshot.completed_releases[0]
                    .ordered_records
                    .iter()
                    .map(|record| record.key)
                    .collect::<Vec<_>>(),
                barrier.ordered_keys
            );
            let mut drifted_completion = reconciliation_snapshot.clone();
            drifted_completion.completed_releases[0].ordered_records[0].enqueue_timestamp_ms ^= 1;
            assert_eq!(
                drifted_completion.durable_owner_count(),
                reconciliation_snapshot.durable_owner_count(),
                "completed-record drift must preserve owner cardinality"
            );
            assert!(
                !snapshot_replay_receipt
                    .binds_reconciliation_snapshot(&drifted_completion)
                    .expect("validate a well-formed drifted completion record"),
                "equal-count completion records cannot substitute another timestamp identity"
            );
        } else {
            assert_eq!(
                reconciliation_snapshot
                    .ordered_records
                    .iter()
                    .map(|record| record.key)
                    .collect::<Vec<_>>(),
                barrier.ordered_keys
            );
            assert_eq!(
                reconciliation_snapshot.prepared_release_barriers,
                vec![barrier.clone()]
            );
            assert!(reconciliation_snapshot.completed_releases.is_empty());
            let mut drifted_prepared = reconciliation_snapshot.clone();
            drifted_prepared.prepared_release_barriers[0].retirement_hash =
                Hash::new(b"same-count-different-prepared-release");
            assert_eq!(
                drifted_prepared.durable_owner_count(),
                reconciliation_snapshot.durable_owner_count(),
                "prepared-release drift must preserve owner cardinality"
            );
            assert!(
                !snapshot_replay_receipt
                    .binds_reconciliation_snapshot(&drifted_prepared)
                    .expect("validate a well-formed drifted prepared release"),
                "equal-count prepared owners cannot substitute another release digest"
            );
        }
        assert_eq!(
            queue.lane_reservation_release_barriers(),
            vec![barrier.clone()],
            "journal installation and QueuePlan replay must not treat payload availability as external retirement proof"
        );
        assert_eq!(
            queue.queued_len(),
            0,
            "both prepared and completed releases remain quarantined outside ordinary FIFO"
        );
        // Simulate the caller's stable Kura proof that every matching entrypoint claim is
        // durably Released. Only this explicit evidence-gated transition may restore FIFO and
        // forget the durable barrier.
        assert_eq!(
            queue
                .finalize_lane_reservation_release_barrier(&barrier)
                .expect("finalize release after simulated external retirement proof"),
            if crash_after_completion { 0 } else { 2 }
        );
        assert!(queue.lane_reservation_release_barriers().is_empty());
        assert_eq!(queue.queued_len(), 2);
        assert!(queue.live_lane_reservations().is_empty());
    }
}
#[test]
fn lane_reservation_is_durable_before_fifo_transfer_and_preserves_accounting() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let txs: Vec<_> = (0..4)
        .map(|_| accepted_queue_plan_tx_by_someone(&time_source))
        .collect();
    let hashes: Vec<_> = txs
        .iter()
        .map(AcceptedTransaction::hash_as_entrypoint)
        .collect();
    for tx in txs {
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, tx);
    }
    let retained_before = queue.retained_bytes();
    let active_before = queue.active_len();
    #[cfg(feature = "telemetry")]
    let lane_teu_before = queue
        .lane_teu_pending
        .get(&LaneId::SINGLE)
        .map(|entry| (entry.teu, entry.tx_count));
    #[cfg(feature = "telemetry")]
    let dataspace_teu_before = queue
        .dataspace_teu_pending
        .get(&(LaneId::SINGLE, DataSpaceId::UNIVERSAL))
        .map(|entry| (entry.teu, entry.tx_count));
    let reserved = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(&state, b"owner-a", b"proposal-a"),
            nonzero!(2_usize),
        )
        .expect("reserve first FIFO pair");
    assert_eq!(
        reserved
            .iter()
            .map(|tx| tx.as_accepted().hash_as_entrypoint())
            .collect::<Vec<_>>(),
        hashes[..2]
    );
    assert_eq!(queue.active_len(), active_before);
    assert_eq!(queue.retained_bytes(), retained_before);
    assert_eq!(queue.queued_len(), 2);
    assert_eq!(queue.live_lane_reservations().len(), 2);
    #[cfg(feature = "telemetry")]
    assert_eq!(
        queue
            .lane_teu_pending
            .get(&LaneId::SINGLE)
            .map(|entry| (entry.teu, entry.tx_count)),
        lane_teu_before,
        "reservation must not debit TEU before explicit commit"
    );
    #[cfg(feature = "telemetry")]
    assert_eq!(
        queue
            .dataspace_teu_pending
            .get(&(LaneId::SINGLE, DataSpaceId::UNIVERSAL))
            .map(|entry| (entry.teu, entry.tx_count)),
        dataspace_teu_before,
        "reservation must preserve dataspace TEU accounting"
    );
    let mut global = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(2_usize), &mut global);
    assert_eq!(
        global
            .iter()
            .map(|tx| tx.as_ref().hash_as_entrypoint())
            .collect::<Vec<_>>(),
        hashes[2..],
        "ordinary selection must skip durable reservations without reordering unrelated work"
    );
    let first_key = reserved[0].key().clone();
    assert_eq!(
        queue
            .retain_lane_reservation(&first_key)
            .expect("retain exact reservation"),
        LaneQueueReservationOutcome::Retained
    );
    let mut conflicting_owner = first_key.clone();
    conflicting_owner.reservation_owner_hash = Hash::new(b"other-owner");
    assert!(matches!(
        queue.retain_lane_reservation(&conflicting_owner),
        Err(LaneQueueReservationError::Conflict { .. })
    ));
    let mut participant_role = first_key.clone();
    participant_role.coordinator_leg.role = RouteLegRole::Participant;
    assert!(matches!(
        queue.retain_lane_reservation(&participant_role),
        Err(LaneQueueReservationError::InvalidIdentity(_))
    ));
    drop(global);
    assert_eq!(queue.active_len(), active_before);
    assert_eq!(
        queue.durable_plan_claims.len(),
        active_before,
        "guard return must retain every exact durable global claim"
    );
    assert_eq!(
        queue
            .release_lane_reservation(&first_key)
            .expect("release reservation"),
        LaneQueueReservationOutcome::Finalized
    );
    assert_eq!(
        queue
            .release_lane_reservation(&first_key)
            .expect("repeat exact release"),
        LaneQueueReservationOutcome::AlreadyFinalized
    );
    let second_key = reserved[1].key().clone();
    assert_eq!(
        queue
            .commit_lane_reservation_for_test(&second_key)
            .expect("commit reservation"),
        LaneQueueReservationOutcome::Finalized
    );
    assert_eq!(
        queue
            .commit_lane_reservation_for_test(&second_key)
            .expect("repeat exact commit"),
        LaneQueueReservationOutcome::AlreadyFinalized
    );
    let mut released = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(1_usize), &mut released);
    assert_eq!(released[0].as_ref().hash_as_entrypoint(), hashes[0]);
}
#[test]
fn reservation_group_commit_preflights_later_identity_before_any_prefix_mutation() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Queue::test(config_factory(), &time_source);
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    for _ in 0..2 {
        push_globally_bound_lane_reservation_candidate(
            &queue,
            &state,
            &dir,
            accepted_queue_plan_tx_by_someone(&time_source),
        );
    }
    let keys = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(
                &state,
                b"group-commit-preflight-owner",
                b"group-commit-preflight-proposal",
            ),
            nonzero!(2_usize),
        )
        .expect("reserve exact two-member group")
        .iter()
        .map(|reserved| *reserved.key())
        .collect::<Vec<_>>();
    let mut conflicting_later = keys[1];
    conflicting_later.routing_plan_digest = Hash::new(b"conflicting later routing plan");
    let error = queue
        .commit_lane_reservation_group(&[keys[0], conflicting_later])
        .expect_err("a conflicting later member must reject the complete group");
    assert!(matches!(
        error,
        LaneQueueReservationError::Conflict { hash }
            if hash == keys[1].entrypoint_hash
    ));
    assert_eq!(
        queue.live_lane_reservations().len(),
        2,
        "the valid prefix must remain live when later preflight fails"
    );
    assert!(queue.lane_reservation_commit_barriers().is_empty());
    assert_eq!(
        queue
            .commit_lane_reservation_group(&keys)
            .expect("commit exact group after failed adversarial preflight"),
        2
    );
    assert!(queue.live_lane_reservations().is_empty());
}
#[test]
fn canonical_cleanup_rejects_empty_and_oversized_group_batches_before_mutation() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Queue::test(config_factory(), &time_source);
    let carrier_limit = iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS;
    assert_eq!(
        queue
            .validate_lane_queue_carrier_cleanup_batch_bounds(&[carrier_limit, 1], 2)
            .expect("two independently bounded carriers may exceed one carrier aggregate"),
        carrier_limit + 1,
    );
    assert!(
        queue
            .validate_lane_queue_carrier_cleanup_batch_bounds(&[carrier_limit + 1], 1)
            .is_err(),
        "one carrier must remain within the merge execution bound",
    );
    assert!(
        queue
            .validate_lane_queue_carrier_cleanup_batch_bounds(&[1, 1], 1)
            .is_err(),
        "carrier batches cannot exceed their exact startup-anchor bound",
    );
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    push_globally_bound_lane_reservation_candidate(
        &queue,
        &state,
        &dir,
        accepted_queue_plan_tx_by_someone(&time_source),
    );
    let key = *queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(
                &state,
                b"bounded-cleanup-owner",
                b"bounded-cleanup-proposal",
            ),
            nonzero!(1_usize),
        )
        .expect("reserve bounded-cleanup key")[0]
        .key();
    let group = lane_queue_reservation_group_binding_from_ordered_keys([&key])
        .expect("bind bounded-cleanup key");
    let before = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture bounded-cleanup snapshot");
    let empty_error = queue
        .commit_prepared_lane_reservation_groups(Vec::new())
        .expect_err("canonical cleanup must reject an empty group batch");
    assert!(matches!(
        empty_error,
        LaneQueueReservationError::InvalidIdentity(detail)
            if detail.contains("non-empty carrier set")
    ));
    let oversized = (0..=iroha_data_model::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS)
        .map(|_| PreparedLaneQueueCarrierCleanupGroup {
            ordered_keys: vec![key],
            group_binding: group,
            cleanup_gate: LaneQueueCarrierCleanupGate::direct_test(group),
        })
        .collect::<Vec<_>>();
    let oversized_error = queue
        .commit_prepared_lane_reservation_groups(oversized)
        .expect_err("canonical cleanup must reject an oversized all-group batch");
    assert!(matches!(
        oversized_error,
        LaneQueueReservationError::InvalidIdentity(detail)
            if detail.contains("exceeds hard limit")
    ));
    assert_eq!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("recapture bounded-cleanup snapshot"),
        before,
        "whole-call bounds must fail before any Queue owner changes",
    );
}
#[test]
fn canonical_cleanup_replays_two_finalized_carriers_beyond_live_queue_capacity() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let mut config = config_factory();
    config.capacity = nonzero!(2_usize);
    config.capacity_per_user = nonzero!(2_usize);
    let queue = Queue::test(config, &time_source);
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let mut finalized_groups = Vec::new();
    for carrier_index in 0_u64..2 {
        for _ in 0..2 {
            push_globally_bound_lane_reservation_candidate(
                &queue,
                &state,
                &dir,
                accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source),
            );
        }
        let mut scope = lane_reservation_scope(
            &state,
            format!("finalized-carrier-owner-{carrier_index}").as_bytes(),
            format!("finalized-carrier-proposal-{carrier_index}").as_bytes(),
        );
        scope.lane_block_height = carrier_index.saturating_add(1);
        let keys = queue
            .reserve_transactions_for_lane(&state, scope, nonzero!(2_usize))
            .expect("reserve one full-capacity canonical carrier")
            .iter()
            .map(|reserved| *reserved.key())
            .collect::<Vec<_>>();
        assert_eq!(keys.len(), 2);
        assert_eq!(
            queue
                .commit_lane_reservation_group(&keys)
                .expect("finalize canonical carrier before aggregate replay"),
            2,
        );
        finalized_groups.push(keys);
    }
    assert_eq!(
        finalized_groups.iter().map(Vec::len).sum::<usize>(),
        4,
        "the finalized sibling set must truly exceed Queue's live capacity",
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture terminal Queue snapshot")
            .is_empty(),
    );
    let carriers = finalized_groups
        .into_iter()
        .map(|keys| vec![prepared_canonical_cleanup_group(keys)])
        .collect::<Vec<_>>();
    let result = queue
        .commit_prepared_lane_reservation_carriers(carriers, 2)
        .expect("one journal snapshot authenticates all finalized carrier siblings");
    let (finalized, terminal_evidence) = result.into_parts();
    assert_eq!(finalized, 0, "all four reservations were already terminal");
    assert_eq!(
        terminal_evidence.len(),
        2,
        "each exact carrier group still mints terminal Queue evidence",
    );
    assert!(queue.live_lane_reservations().is_empty());
}
#[test]
fn canonical_cleanup_rejects_cross_carrier_aliases_before_mutation() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Queue::test(config_factory(), &time_source);
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let [first_key, second_key] =
        reserve_two_canonical_cleanup_carrier_groups(&queue, &state, &dir, &time_source);
    let first_binding = lane_queue_reservation_group_binding_from_ordered_keys([&first_key])
        .expect("bind first cross-carrier alias group");
    let second_binding = lane_queue_reservation_group_binding_from_ordered_keys([&second_key])
        .expect("bind second cross-carrier alias group");
    let before = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture cross-carrier alias snapshot");
    for alias_case in 0_u8..4 {
        let mut aliased_key = second_key;
        let mut aliased_binding = second_binding;
        match alias_case {
            0 => {
                aliased_key.lane_block_height = first_key.lane_block_height;
                aliased_binding =
                    lane_queue_reservation_group_binding_from_ordered_keys([&aliased_key])
                        .expect("bind same-slot carrier alias");
            }
            1 => {
                aliased_binding.reservation_group_hash = first_binding.reservation_group_hash;
            }
            2 => {
                aliased_key.entrypoint_hash = first_key.entrypoint_hash;
                aliased_binding.reservation_group_hash =
                    Hash::new(b"cross-carrier duplicate transaction group");
            }
            3 => {
                aliased_key.entrypoint_hash = first_key.entrypoint_hash;
                aliased_binding.reservation_group_hash =
                    Hash::new(b"cross-carrier duplicate entrypoint group");
            }
            _ => unreachable!(),
        }
        let aliased = PreparedLaneQueueCarrierCleanupGroup {
            ordered_keys: vec![aliased_key],
            group_binding: aliased_binding,
            cleanup_gate: LaneQueueCarrierCleanupGate::direct_test(aliased_binding),
        };
        let error = queue
            .commit_prepared_lane_reservation_carriers(
                vec![
                    vec![prepared_canonical_cleanup_group(vec![first_key])],
                    vec![aliased],
                ],
                2,
            )
            .expect_err("a cross-carrier identity alias must reject the whole batch");
        let expected_detail = if alias_case < 2 {
            "reservation group hash, attempt identity, or lane slot"
        } else {
            "transaction or entrypoint owner"
        };
        assert!(matches!(
            error,
            LaneQueueReservationError::InvalidIdentity(detail)
                if detail.contains(expected_detail)
        ));
        assert_eq!(
            queue
                .lane_reservation_reconciliation_snapshot()
                .expect("recapture cross-carrier alias snapshot"),
            before,
            "cross-carrier alias case {alias_case} must fail before any Queue mutation",
        );
        assert!(queue.lane_reservation_commit_barriers().is_empty());
    }
}
#[test]
fn canonical_cleanup_later_carrier_conflict_preserves_every_earlier_live_owner() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Queue::test(config_factory(), &time_source);
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let [first_key, second_key] =
        reserve_two_canonical_cleanup_carrier_groups(&queue, &state, &dir, &time_source);
    let before = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture both live carrier owners");
    let mut conflicting_later = second_key;
    conflicting_later.routing_plan_digest =
        Hash::new(b"later carrier changed its authenticated routing plan");
    let error = queue
        .commit_prepared_lane_reservation_carriers(
            vec![
                vec![prepared_canonical_cleanup_group(vec![first_key])],
                vec![prepared_canonical_cleanup_group(vec![conflicting_later])],
            ],
            2,
        )
        .expect_err("a later carrier conflict must reject the complete cleanup set");
    assert!(matches!(
        error,
        LaneQueueReservationError::Conflict { hash }
            if hash == second_key.entrypoint_hash
    ));
    assert_eq!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("recapture both live carrier owners after rejection"),
        before,
        "the valid earlier carrier must remain live when a later carrier conflicts",
    );
    assert_eq!(queue.live_lane_reservations().len(), 2);
    assert!(queue.lane_reservation_commit_barriers().is_empty());
}
#[test]
fn canonical_cleanup_later_replica_conflict_preserves_every_ordinary_owner() {
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let producer = Queue::test(config_factory(), &time_source);
    let replica = Queue::test(config_factory(), &time_source);
    let producer_dir = tempdir().expect("producer tempdir");
    let replica_dir = tempdir().expect("replica tempdir");
    install_globally_certified_test_reservation_journals(&producer, &producer_dir);
    install_globally_certified_test_reservation_journals(&replica, &replica_dir);

    let first = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let first_hash = first.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(&producer, &state, &producer_dir, first.clone());
    push_globally_bound_lane_reservation_candidate(&replica, &state, &replica_dir, first);
    time_handle.advance(Duration::from_millis(1));
    let second = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let second_hash = second.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(
        &producer,
        &state,
        &producer_dir,
        second.clone(),
    );
    push_globally_bound_lane_reservation_candidate(&replica, &state, &replica_dir, second);

    let first_key = *producer
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(
                &state,
                b"replica-preflight-owner-a",
                b"replica-preflight-proposal-a",
            ),
            nonzero!(1_usize),
        )
        .expect("reserve first producer carrier group")[0]
        .key();
    let mut second_scope = lane_reservation_scope(
        &state,
        b"replica-preflight-owner-b",
        b"replica-preflight-proposal-b",
    );
    second_scope.lane_block_height = 2;
    let second_key = *producer
        .reserve_transactions_for_lane(&state, second_scope, nonzero!(1_usize))
        .expect("reserve second producer carrier group")[0]
        .key();
    assert_eq!(first_key.entrypoint_hash, first_hash);
    assert_eq!(second_key.entrypoint_hash, second_hash);

    let hashes = [first_hash, second_hash];
    let fifo_before = replica.fifo_snapshot_for_test();
    let active_before = replica.active_len();
    let queued_before = replica.queued_len();
    let replica_plan_path = replica_dir
        .path()
        .join("queue-plans-for-reservations.norito");
    let journal_before = std::fs::read(&replica_plan_path).expect("read replica plan journal");
    #[cfg(feature = "telemetry")]
    let teu_before = hashes
        .iter()
        .map(|hash| {
            let info = *replica
                .tx_teu
                .get(hash)
                .expect("replica transaction must have TEU identity")
                .value();
            (info.lane_id, info.dataspace_id, info.teu)
        })
        .collect::<Vec<_>>();
    #[cfg(feature = "telemetry")]
    let teu_aggregate_before = (
        replica
            .lane_teu_pending
            .iter()
            .map(|entry| entry.value().teu)
            .sum::<u64>(),
        replica
            .lane_teu_pending
            .iter()
            .map(|entry| entry.value().tx_count)
            .sum::<usize>(),
        replica
            .dataspace_teu_pending
            .iter()
            .map(|entry| entry.value().teu)
            .sum::<u64>(),
        replica
            .dataspace_teu_pending
            .iter()
            .map(|entry| entry.value().tx_count)
            .sum::<usize>(),
    );

    let mut conflicting_later = second_key;
    conflicting_later.routing_plan_digest =
        Hash::new(b"later replica changed its authenticated routing plan");
    let error = replica
        .commit_prepared_lane_reservation_carriers(
            vec![
                vec![prepared_canonical_cleanup_group(vec![first_key])],
                vec![prepared_canonical_cleanup_group(vec![conflicting_later])],
            ],
            2,
        )
        .expect_err("a later replica conflict must reject the complete cleanup set");
    assert!(matches!(
        error,
        LaneQueueReservationError::ReconciliationDurableClaimMismatch { hash, .. }
            if hash == second_hash
    ));
    assert_eq!(
        std::fs::read(&replica_plan_path).expect("reread replica plan journal"),
        journal_before,
        "a later replica conflict must precede every durable QueuePlan tombstone",
    );
    assert_eq!(replica.fifo_snapshot_for_test(), fifo_before);
    assert_eq!(replica.active_len(), active_before);
    assert_eq!(replica.queued_len(), queued_before);
    for hash in hashes {
        assert!(replica.txs.contains_key(&hash));
        assert!(replica.routing_plans.contains_key(&hash));
        assert!(replica.durable_plan_claims.contains_key(&hash));
        assert!(replica.fifo_order_by_hash.contains_key(&hash));
        assert!(replica.tx_encoded_len.contains_key(&hash));
        assert!(replica.tx_gas_cost.contains_key(&hash));
        assert!(replica.tx_enqueued_at_ms.contains_key(&hash));
        assert!(replica.queued_tx_enqueued_at_ms.contains_key(&hash));
        assert!(replica.expiry_ring_members.contains_key(&hash));
        assert!(!replica.removed_hashes.contains_key(&hash));
    }
    #[cfg(feature = "telemetry")]
    {
        let teu_after = hashes
            .iter()
            .map(|hash| {
                let info = *replica
                    .tx_teu
                    .get(hash)
                    .expect("rejected cleanup must retain replica TEU identity")
                    .value();
                (info.lane_id, info.dataspace_id, info.teu)
            })
            .collect::<Vec<_>>();
        let teu_aggregate_after = (
            replica
                .lane_teu_pending
                .iter()
                .map(|entry| entry.value().teu)
                .sum::<u64>(),
            replica
                .lane_teu_pending
                .iter()
                .map(|entry| entry.value().tx_count)
                .sum::<usize>(),
            replica
                .dataspace_teu_pending
                .iter()
                .map(|entry| entry.value().teu)
                .sum::<u64>(),
            replica
                .dataspace_teu_pending
                .iter()
                .map(|entry| entry.value().tx_count)
                .sum::<usize>(),
        );
        assert_eq!(teu_after, teu_before);
        assert_eq!(teu_aggregate_after, teu_aggregate_before);
    }
}
#[test]
fn canonical_cleanup_atomically_consumes_committed_revalidated_ordinary_replica_group() {
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let producer = Queue::test(config_factory(), &time_source);
    let replica = Arc::new(Queue::test(config_factory(), &time_source));
    let producer_dir = tempdir().expect("producer tempdir");
    let replica_dir = tempdir().expect("replica tempdir");
    install_globally_certified_test_reservation_journals(&producer, &producer_dir);
    install_globally_certified_test_reservation_journals(&replica, &replica_dir);

    let first = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let first_hash = first.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(&producer, &state, &producer_dir, first.clone());
    push_globally_bound_lane_reservation_candidate(&replica, &state, &replica_dir, first);
    time_handle.advance(Duration::from_millis(1));
    let second = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let second_hash = second.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(
        &producer,
        &state,
        &producer_dir,
        second.clone(),
    );
    push_globally_bound_lane_reservation_candidate(&replica, &state, &replica_dir, second);

    let keys = producer
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(&state, b"atomic-replica-owner", b"atomic-replica-proposal"),
            nonzero!(2_usize),
        )
        .expect("reserve exact producer group")
        .iter()
        .map(|reserved| *reserved.key())
        .collect::<Vec<_>>();
    assert_eq!(
        keys.iter()
            .map(|key| key.entrypoint_hash)
            .collect::<Vec<_>>(),
        vec![first_hash, second_hash],
    );

    let (selected, global_selection) = replica
        .bounded_pending_snapshot(&state.view(), nonzero!(2_usize))
        .expect("acquire the replica's process-local global selection fence");
    assert_eq!(
        selected
            .iter()
            .map(|transaction| transaction.hash_as_entrypoint())
            .collect::<Vec<_>>(),
        vec![first_hash, second_hash],
    );

    {
        let mut transactions = state.transactions.block();
        transactions.insert_block(
            [first_hash, second_hash].into_iter().collect(),
            nonzero!(1_usize),
        );
        transactions
            .commit()
            .expect("publish the committed carrier transaction identities");
    }
    replica.reconfigure_nexus_with_state(&state.nexus_snapshot(), &state, None);
    #[cfg(feature = "telemetry")]
    for hash in [first_hash, second_hash] {
        assert!(
            replica.tx_teu.contains_key(&hash),
            "committed replica TEU must remain until exact canonical Queue cleanup",
        );
    }

    let conflict = replica
        .commit_prepared_lane_reservation_groups(vec![prepared_canonical_cleanup_group(
            keys.clone(),
        )])
        .expect_err("a live global selection lease must fence canonical replica cleanup");
    assert!(matches!(
        conflict,
        LaneQueueReservationError::Conflict { hash } if hash == first_hash
    ));
    drop(global_selection);

    let result = replica
        .commit_prepared_lane_reservation_groups(vec![prepared_canonical_cleanup_group(
            keys.clone(),
        )])
        .expect("consume the exact ordinary replica group");
    let (finalized, terminal_evidence) = result.into_parts();
    assert_eq!(
        finalized, 0,
        "replica cleanup must not claim a producer reservation finalization"
    );
    assert_eq!(terminal_evidence.len(), 1);
    for hash in [first_hash, second_hash] {
        assert!(!replica.txs.contains_key(&hash));
        assert!(!replica.routing_plans.contains_key(&hash));
        assert!(!replica.durable_plan_claims.contains_key(&hash));
        assert!(!replica.fifo_order_by_hash.contains_key(&hash));
        assert!(!replica.tx_encoded_len.contains_key(&hash));
        assert!(!replica.tx_gas_cost.contains_key(&hash));
        assert!(!replica.tx_enqueued_at_ms.contains_key(&hash));
        assert!(!replica.queued_tx_enqueued_at_ms.contains_key(&hash));
        assert!(!replica.expiry_ring_members.contains_key(&hash));
        assert!(!replica.removed_hashes.contains_key(&hash));
        #[cfg(feature = "telemetry")]
        assert!(!replica.tx_teu.contains_key(&hash));
    }
    assert!(replica.fifo_snapshot_for_test().is_empty());
    assert_eq!(replica.active_len(), 0);
    assert_eq!(replica.queued_len(), 0);

    let retry = replica
        .commit_prepared_lane_reservation_groups(vec![prepared_canonical_cleanup_group(keys)])
        .expect("retry exact already-terminal replica cleanup");
    let (retry_finalized, retry_evidence) = retry.into_parts();
    assert_eq!(retry_finalized, 0);
    assert_eq!(retry_evidence.len(), 1);
}
#[test]
fn finalized_canonical_group_rejects_dangling_queue_owners_and_metadata() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Queue::test(config_factory(), &time_source);
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    push_globally_bound_lane_reservation_candidate(
        &queue,
        &state,
        &dir,
        accepted_queue_plan_tx_by_someone(&time_source),
    );
    let key = *queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(
                &state,
                b"terminal-metadata-owner",
                b"terminal-metadata-proposal",
            ),
            nonzero!(1_usize),
        )
        .expect("reserve canonical terminal metadata fixture")[0]
        .key();
    assert_eq!(
        queue
            .commit_lane_reservation_group(&[key])
            .expect("complete canonical Queue cleanup"),
        1
    );
    let hash = key.entrypoint_hash;
    let assert_rejected = || {
        assert!(matches!(
            queue.commit_lane_reservation_group(&[key]),
            Err(LaneQueueReservationError::Conflict { hash: conflict }) if conflict == hash
        ));
    };
    queue.global_selection_owners.lock().insert(hash, 1);
    assert_rejected();
    queue.global_selection_owners.lock().remove(&hash);
    queue.removed_hashes.insert(hash, ());
    assert_rejected();
    queue.removed_hashes.remove(&hash);
    queue.tx_encoded_len.insert(hash, 1);
    assert_rejected();
    queue.tx_encoded_len.remove(&hash);
    queue.tx_gas_cost.insert(hash, 1);
    assert_rejected();
    queue.tx_gas_cost.remove(&hash);
    queue.tx_enqueued_at_ms.insert(hash, 1);
    assert_rejected();
    queue.tx_enqueued_at_ms.remove(&hash);
    queue.queued_tx_enqueued_at_ms.insert(hash, 1);
    assert_rejected();
    queue.queued_tx_enqueued_at_ms.remove(&hash);
    assert_eq!(
        queue
            .commit_lane_reservation_group(&[key])
            .expect("clean already-empty canonical retry"),
        0
    );
}
#[test]
fn reservation_group_commit_stages_complete_commit_prefix_before_tombstones() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Queue::test(config_factory(), &time_source);
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    for _ in 0..3 {
        push_globally_bound_lane_reservation_candidate(
            &queue,
            &state,
            &dir,
            accepted_queue_plan_tx_by_someone(&time_source),
        );
    }
    let keys = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(
                &state,
                b"group-commit-three-phase-owner",
                b"group-commit-three-phase-proposal",
            ),
            nonzero!(3_usize),
        )
        .expect("reserve exact three-member group")
        .iter()
        .map(|reserved| *reserved.key())
        .collect::<Vec<_>>();
    queue.hold_next_lane_reservation_commit_after_barrier_for_test();
    assert_eq!(
        queue
            .commit_lane_reservation_group(&keys)
            .expect("stop after the first QueuePlan tombstone"),
        3,
        "all reservation Commit frames precede the first QueuePlan tombstone"
    );
    assert!(queue.live_lane_reservations().is_empty());
    let mut expected_barriers = keys.clone();
    expected_barriers.sort_by_key(LaneQueueReservationKeyV1::digest);
    assert_eq!(
        queue.lane_reservation_commit_barriers(),
        expected_barriers,
        "the crash cut must retain the complete committed group"
    );
    let reconciliation_snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture the complete committed crash cut atomically");
    assert!(reconciliation_snapshot.ordered_records.is_empty());
    assert_eq!(reconciliation_snapshot.commit_barriers, expected_barriers);
    assert!(reconciliation_snapshot.release_barriers().is_empty());
    assert!(!queue.txs.contains_key(&keys[0].entrypoint_hash));
    assert!(queue.txs.contains_key(&keys[1].entrypoint_hash));
    assert!(queue.txs.contains_key(&keys[2].entrypoint_hash));
    assert_eq!(
        queue
            .commit_lane_reservation_group(&keys)
            .expect("resume the exact group through tombstone and ForgetCommit"),
        0,
        "retry resumes barriers without claiming a second reservation commit"
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert!(queue.lane_reservation_commit_barriers().is_empty());
    assert!(
        keys.iter()
            .all(|key| !queue.txs.contains_key(&key.entrypoint_hash))
    );
}
#[test]
fn reservation_group_forget_prefix_replays_and_resumes_exactly_once() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("queue-plans-for-reservations.norito");
    let reservation_path;
    let keys;
    {
        let queue = Queue::test(config_factory(), &time_source);
        reservation_path = install_globally_certified_test_reservation_journals(&queue, &dir);
        for _ in 0..3 {
            push_globally_bound_lane_reservation_candidate(
                &queue,
                &state,
                &dir,
                accepted_queue_plan_tx_by_someone(&time_source),
            );
        }
        keys = queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(
                    &state,
                    b"group-forget-prefix-owner",
                    b"group-forget-prefix-proposal",
                ),
                nonzero!(3_usize),
            )
            .expect("reserve exact three-member group")
            .iter()
            .map(|reserved| *reserved.key())
            .collect::<Vec<_>>();
        queue
            .lane_reservation_journal
            .lock()
            .as_mut()
            .expect("installed reservation journal")
            .inject_append_fault_after(
                4,
                ReservationJournalAppendFault::AfterSyncBeforeReplayPublication,
            );
        let error = queue
            .commit_lane_reservation_group(&keys)
            .expect_err("the second ForgetCommit publication must fail closed");
        assert!(matches!(error, LaneQueueReservationError::Journal(_)));
        assert!(queue.lane_reservation_durability_faulted());
        assert!(queue.live_lane_reservations().is_empty());
        let mut expected_memory_barriers = vec![keys[1], keys[2]];
        expected_memory_barriers.sort_by_key(LaneQueueReservationKeyV1::digest);
        assert_eq!(
            queue.lane_reservation_commit_barriers(),
            expected_memory_barriers,
            "the synchronized second ForgetCommit must remain unpublished in memory"
        );
        assert!(
            keys.iter()
                .all(|key| !queue.txs.contains_key(&key.entrypoint_hash)),
            "the complete QueuePlan tombstone prefix must precede every ForgetCommit"
        );
    }
    let restarted = Queue::test(config_factory(), &time_source);
    let replay = restarted
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restart must replay the synchronized ForgetCommit prefix");
    assert_eq!(replay.restored, 0);
    assert_eq!(replay.commit_barriers, 1);
    assert_eq!(
        restarted.lane_reservation_commit_barriers(),
        vec![keys[2]],
        "durable replay must recover the exact two-forgotten/one-barrier prefix"
    );
    restarted
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install the already-tombstoned exact QueuePlan journal");
    let reconciliation_snapshot = restarted
        .lane_reservation_reconciliation_snapshot()
        .expect("capture the replayed Commit barrier in the startup ownership cut");
    assert!(reconciliation_snapshot.ordered_records.is_empty());
    assert_eq!(reconciliation_snapshot.commit_barriers, vec![keys[2]]);
    assert!(reconciliation_snapshot.prepared_release_barriers.is_empty());
    assert!(reconciliation_snapshot.completed_releases.is_empty());
    assert_eq!(
        restarted
            .commit_lane_reservation_group(&keys)
            .expect("resume the exact group from its durable ForgetCommit prefix"),
        0,
        "restart must not claim a second reservation commit"
    );
    assert!(restarted.live_lane_reservations().is_empty());
    assert!(restarted.lane_reservation_commit_barriers().is_empty());
    assert_eq!(
        restarted
            .commit_lane_reservation_group(&keys)
            .expect("repeat the finalized exact group"),
        0,
        "a finalized group retry must remain exactly-once"
    );
}
#[test]
fn restart_reconciliation_snapshot_is_fifo_group_complete_and_read_only() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    let reservation_path = install_globally_certified_test_reservation_journals(&queue, &dir);
    let first_bindings = (0..2)
        .map(|_| {
            push_globally_bound_lane_reservation_candidate(
                &queue,
                &state,
                &dir,
                accepted_queue_plan_tx_by_someone(&time_source),
            )
        })
        .collect::<Vec<_>>();
    let key_for_scope = |binding: &crate::torii_proxy::QueuePlanAdmissionBindingV1,
                         scope: LaneQueueReservationScopeV1| {
        let routing_plan = binding
            .routing_plan()
            .expect("rebuild reservation fixture routing plan");
        LaneQueueReservationKeyV1 {
            version: LaneQueueReservationKeyV1::VERSION,
            entrypoint_hash: binding.entrypoint_hash,
            queue_plan_admission_binding_hash: binding.canonical_hash(),
            routing_plan_digest: routing_plan.digest(),
            coordinator_leg: routing_plan.coordinator_leg(),
            lane_id: scope.lane_id,
            dataspace_id: scope.dataspace_id,
            lane_incarnation: scope.lane_incarnation,
            proposal_height: scope.proposal_height,
            lane_block_height: scope.lane_block_height,
            lane_block_view: scope.lane_block_view,
            reservation_owner_hash: scope.reservation_owner_hash,
            proposal_identity_hash: scope.proposal_identity_hash,
        }
    };
    let (first_scope, expected_first) = (0_u32..4096)
        .find_map(|salt| {
            let salt = salt.to_be_bytes();
            let mut scope = lane_reservation_scope(
                &state,
                b"reconciliation-snapshot-first-owner",
                b"reconciliation-snapshot-first-proposal",
            );
            scope.reservation_owner_hash =
                Hash::new_from_chunks(&[b"reconciliation-snapshot-first-owner\0", &salt]);
            scope.proposal_identity_hash =
                Hash::new_from_chunks(&[b"reconciliation-snapshot-first-proposal\0", &salt]);
            let expected = key_for_scope(&first_bindings[0], scope);
            (expected.digest().as_ref()[0] >= 0x80).then_some((scope, expected))
        })
        .expect("find a deterministic first-group digest in the upper half of hash space");
    let first_group = queue
        .reserve_transactions_for_lane(&state, first_scope, nonzero!(2_usize))
        .expect("reserve first reconciliation group");
    let first_keys = first_group
        .iter()
        .map(|transaction| *transaction.key())
        .collect::<Vec<_>>();
    assert_eq!(first_keys[0], expected_first);
    let second_bindings = (0..2)
        .map(|_| {
            push_globally_bound_lane_reservation_candidate(
                &queue,
                &state,
                &dir,
                accepted_queue_plan_tx_by_someone(&time_source),
            )
        })
        .collect::<Vec<_>>();
    let (second_scope, expected_second_first) = (0_u32..4096)
        .find_map(|salt| {
            let salt = salt.to_be_bytes();
            let mut scope = lane_reservation_scope(
                &state,
                b"reconciliation-snapshot-second-owner",
                b"reconciliation-snapshot-second-proposal",
            );
            scope.lane_block_height = 2;
            scope.reservation_owner_hash =
                Hash::new_from_chunks(&[b"reconciliation-snapshot-second-owner\0", &salt]);
            scope.proposal_identity_hash =
                Hash::new_from_chunks(&[b"reconciliation-snapshot-second-proposal\0", &salt]);
            let expected = key_for_scope(&second_bindings[0], scope);
            (expected.digest() < first_keys[0].digest()).then_some((scope, expected))
        })
        .expect("find a deterministic second-group identity below the first FIFO key digest");
    let second_group = queue
        .reserve_transactions_for_lane(&state, second_scope, nonzero!(2_usize))
        .expect("reserve second reconciliation group");
    let second_keys = second_group
        .iter()
        .map(|transaction| *transaction.key())
        .collect::<Vec<_>>();
    assert_eq!(second_keys[0], expected_second_first);
    let fifo_keys = first_keys
        .iter()
        .chain(&second_keys)
        .copied()
        .collect::<Vec<_>>();
    let mut digest_ordered_keys = fifo_keys.clone();
    digest_ordered_keys.sort_by_key(LaneQueueReservationKeyV1::digest);
    assert_ne!(
        digest_ordered_keys, fifo_keys,
        "fixture must distinguish digest order from original durable FIFO order"
    );
    let capture_store = || {
        let store = queue.lane_reservations.lock();
        let mut live = store
            .live_by_entrypoint
            .values()
            .cloned()
            .collect::<Vec<_>>();
        live.sort_by_key(|record| (record.fifo_order.ordinal, record.key.entrypoint_hash));
        (
            live,
            store.commit_barriers.clone(),
            store.release_barriers.clone(),
            store.completed_releases.clone(),
            store.missing_payload_hashes.clone(),
        )
    };
    let capture_fifo_index = || {
        let mut fifo = queue
            .fifo_order_by_hash
            .iter()
            .map(|entry| (*entry.key(), *entry.value()))
            .collect::<Vec<_>>();
        fifo.sort_by_key(|(hash, _)| *hash);
        fifo
    };
    let capture_claims = || {
        let mut claims = queue
            .durable_plan_claims
            .iter()
            .map(|entry| (*entry.key(), entry.value().durable_admission()))
            .collect::<Vec<_>>();
        claims.sort_by_key(|(hash, _)| *hash);
        claims
    };
    let store_before = capture_store();
    let fifo_index_before = capture_fifo_index();
    let claims_before = capture_claims();
    let live_before = queue.live_lane_reservations();
    let journal_before = fs::read(&reservation_path).expect("read reservation journal before");
    let active_before = queue.active_len();
    let queued_before = queue.queued_len();
    let snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture strict reconciliation snapshot");
    assert_eq!(
        snapshot
            .ordered_records
            .iter()
            .map(|record| record.key)
            .collect::<Vec<_>>(),
        fifo_keys,
        "snapshot records must follow durable FIFO rather than reservation digest order"
    );
    assert!(
        snapshot
            .ordered_records
            .windows(2)
            .all(|records| records[0].fifo_ordinal < records[1].fifo_ordinal)
    );
    assert_eq!(
        snapshot.ordered_groups,
        vec![
            LaneQueueReservationReconciliationGroupV1 {
                identity: LaneQueueReservationGroupIdentityV1::from_key(&first_keys[0]),
                ordered_keys: first_keys,
            },
            LaneQueueReservationReconciliationGroupV1 {
                identity: LaneQueueReservationGroupIdentityV1::from_key(&second_keys[0]),
                ordered_keys: second_keys,
            },
        ],
        "every exact proposal group must retain complete FIFO-ordered membership"
    );
    assert!(snapshot.commit_barriers.is_empty());
    assert!(snapshot.prepared_release_barriers.is_empty());
    assert!(snapshot.completed_releases.is_empty());
    assert!(snapshot.release_barriers().is_empty());
    for record in &snapshot.ordered_records {
        let expected_claim = claims_before
            .iter()
            .find_map(|(hash, claim)| (*hash == record.key.entrypoint_hash).then_some(claim))
            .expect("snapshot record has an indexed durable claim");
        assert_eq!(&record.durable_admission, expected_claim);
        assert_eq!(
            record.group,
            LaneQueueReservationGroupIdentityV1::from_key(&record.key)
        );
    }
    assert_eq!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("repeat read-only reconciliation snapshot"),
        snapshot
    );
    assert_eq!(capture_store(), store_before);
    assert_eq!(capture_fifo_index(), fifo_index_before);
    assert_eq!(capture_claims(), claims_before);
    assert_eq!(queue.live_lane_reservations(), live_before);
    assert_eq!(queue.active_len(), active_before);
    assert_eq!(queue.queued_len(), queued_before);
    assert_eq!(
        fs::read(&reservation_path).expect("read reservation journal after"),
        journal_before,
        "snapshot observer must not append, compact, or rewrite durable ownership"
    );
    let claim_hash = fifo_keys[0].entrypoint_hash;
    let original_claim = queue
        .durable_plan_claims
        .remove(&claim_hash)
        .expect("remove exact fixture claim")
        .1;
    assert!(matches!(
        queue.lane_reservation_reconciliation_snapshot(),
        Err(LaneQueueReservationError::ReconciliationMissingDurableClaim { hash })
            if hash == claim_hash
    ));
    queue
        .durable_plan_claims
        .insert(claim_hash, original_claim.clone());
    let mut mismatched_claim = original_claim.clone();
    mismatched_claim.enqueue_timestamp_ms = mismatched_claim.enqueue_timestamp_ms.saturating_add(1);
    queue
        .durable_plan_claims
        .insert(claim_hash, mismatched_claim);
    assert!(matches!(
        queue.lane_reservation_reconciliation_snapshot(),
        Err(LaneQueueReservationError::ReconciliationDurableClaimMismatch { hash, .. })
            if hash == claim_hash
    ));
    queue.durable_plan_claims.insert(claim_hash, original_claim);
    let duplicate_hash = fifo_keys[1].entrypoint_hash;
    let (original_duplicate_record, original_duplicate_fifo, first_fifo) = {
        let mut store = queue.lane_reservations.lock();
        let first_fifo = store.live_by_entrypoint[&claim_hash].fifo_order;
        let duplicate_record = store.live_by_entrypoint[&duplicate_hash].clone();
        store
            .live_by_entrypoint
            .get_mut(&duplicate_hash)
            .expect("mutate duplicate-ordinal fixture")
            .fifo_order = first_fifo;
        let duplicate_fifo = queue
            .fifo_order_by_hash
            .insert(duplicate_hash, first_fifo)
            .expect("duplicate fixture has an indexed FIFO identity");
        (duplicate_record, duplicate_fifo, first_fifo)
    };
    match queue.lane_reservation_reconciliation_snapshot() {
        Err(LaneQueueReservationError::ReconciliationDuplicateFifoOrdinal {
            ordinal,
            first_hash,
            second_hash,
        }) => {
            assert_eq!(ordinal, first_fifo.ordinal);
            assert_eq!(
                BTreeSet::from([first_hash, second_hash]),
                BTreeSet::from([claim_hash, duplicate_hash])
            );
        }
        other => panic!("duplicate durable FIFO ordinal must fail typed: {other:?}"),
    }
    queue
        .lane_reservations
        .lock()
        .live_by_entrypoint
        .insert(duplicate_hash, original_duplicate_record);
    queue
        .fifo_order_by_hash
        .insert(duplicate_hash, original_duplicate_fifo);
    assert_eq!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("restored strict reconciliation snapshot"),
        snapshot
    );
}
#[test]
fn lane_reservation_excludes_locked_global_entrypoints_and_releases_in_payload_order() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let txs = (0..4)
        .map(|_| accepted_queue_plan_tx_by_someone(&time_source))
        .collect::<Vec<_>>();
    let hashes = txs
        .iter()
        .map(AcceptedTransaction::hash_as_entrypoint)
        .collect::<Vec<_>>();
    let excluded = BTreeSet::from([txs[0].hash_as_entrypoint(), txs[2].hash_as_entrypoint()]);
    for tx in txs {
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, tx);
    }
    let reserved = queue
        .reserve_transactions_for_lane_excluding(
            &state,
            lane_reservation_scope(
                &state,
                b"excluded-global-owner",
                b"excluded-global-proposal",
            ),
            nonzero!(2_usize),
            &excluded,
        )
        .expect("reserve only entrypoints not owned by the locked global body");
    assert_eq!(
        reserved
            .iter()
            .map(|transaction| transaction.as_accepted().hash_as_entrypoint())
            .collect::<Vec<_>>(),
        vec![hashes[1], hashes[3]],
    );
    assert_eq!(queue.queued_len(), 2);
    let ordered_keys = reserved
        .iter()
        .map(|transaction| *transaction.key())
        .collect::<Vec<_>>();
    assert!(matches!(
        queue.release_lane_reservations_in_order(&[ordered_keys[0], ordered_keys[0],]),
        Err(LaneQueueReservationError::InvalidIdentity(_))
    ));
    assert_eq!(queue.live_lane_reservations().len(), 2);
    assert_eq!(
        queue
            .release_lane_reservation(&ordered_keys[0])
            .expect("simulate a crash after the first ordered release"),
        LaneQueueReservationOutcome::Finalized
    );
    assert_eq!(
        queue
            .release_lane_reservations_in_order(&ordered_keys)
            .expect("prefix retry resumes at the first still-live reservation"),
        1
    );
    assert_eq!(
        queue
            .release_lane_reservations_in_order(&ordered_keys)
            .expect("ordered release retry is idempotent"),
        0
    );
    let mut selected = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(4_usize), &mut selected);
    assert_eq!(
        selected
            .iter()
            .map(|transaction| transaction.as_ref().hash_as_entrypoint())
            .collect::<Vec<_>>(),
        hashes,
        "interleaved lane reservations must regain their original global FIFO positions",
    );
    drop(selected);
    assert_eq!(queue.active_len(), hashes.len());
    assert_eq!(queue.queued_len(), hashes.len());
    assert_eq!(queue.durable_plan_claims.len(), hashes.len());
}
#[test]
fn released_prefix_precedes_work_enqueued_after_reservation() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let txs = (0..5)
        .map(|_| accepted_queue_plan_tx_by_someone(&time_source))
        .collect::<Vec<_>>();
    let hashes = txs
        .iter()
        .map(AcceptedTransaction::hash_as_entrypoint)
        .collect::<Vec<_>>();
    for tx in txs[..3].iter().cloned() {
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, tx);
    }
    let keys = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(&state, b"fifo-owner", b"fifo-proposal"),
            nonzero!(2_usize),
        )
        .expect("reserve A/B")
        .iter()
        .map(|transaction| *transaction.key())
        .collect::<Vec<_>>();
    for tx in txs[3..].iter().cloned() {
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, tx);
    }
    assert_eq!(
        queue
            .release_lane_reservations_in_order(&keys)
            .expect("restore A/B at their original ordinals"),
        2
    );
    assert_eq!(
        queue
            .release_lane_reservations_in_order(&keys)
            .expect("exact release retry"),
        0
    );
    let mut selected = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(5_usize), &mut selected);
    assert_eq!(
        selected
            .iter()
            .map(|transaction| transaction.as_ref().hash_as_entrypoint())
            .collect::<Vec<_>>(),
        hashes
    );
    drop(selected);
    assert_eq!(queue.active_len(), hashes.len());
    assert_eq!(queue.queued_len(), hashes.len());
    assert_eq!(queue.durable_plan_claims.len(), hashes.len());
}
#[test]
fn interleaved_reservation_batches_restore_one_global_fifo() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let txs = (0..4)
        .map(|_| accepted_queue_plan_tx_by_someone(&time_source))
        .collect::<Vec<_>>();
    let hashes = txs
        .iter()
        .map(AcceptedTransaction::hash_as_entrypoint)
        .collect::<Vec<_>>();
    for tx in &txs {
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, tx.clone());
    }
    let excluded = BTreeSet::from([txs[0].hash_as_entrypoint(), txs[2].hash_as_entrypoint()]);
    let second_and_fourth = queue
        .reserve_transactions_for_lane_excluding(
            &state,
            lane_reservation_scope(
                &state,
                b"interleaved-owner-even",
                b"interleaved-proposal-even",
            ),
            nonzero!(2_usize),
            &excluded,
        )
        .expect("reserve second/fourth FIFO entries")
        .iter()
        .map(|transaction| *transaction.key())
        .collect::<Vec<_>>();
    let first_and_third = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(
                &state,
                b"interleaved-owner-odd",
                b"interleaved-proposal-odd",
            ),
            nonzero!(2_usize),
        )
        .expect("reserve remaining first/third FIFO entries")
        .iter()
        .map(|transaction| *transaction.key())
        .collect::<Vec<_>>();
    assert_eq!(queue.queued_len(), 0);
    queue
        .release_lane_reservations_in_order(&second_and_fourth)
        .expect("release one interleaved ownership batch");
    queue
        .release_lane_reservations_in_order(&first_and_third)
        .expect("release the other interleaved ownership batch");
    let mut selected = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(4_usize), &mut selected);
    assert_eq!(
        selected
            .iter()
            .map(|transaction| transaction.as_ref().hash_as_entrypoint())
            .collect::<Vec<_>>(),
        hashes
    );
    drop(selected);
    assert_eq!(queue.active_len(), hashes.len());
    assert_eq!(queue.queued_len(), hashes.len());
    assert_eq!(queue.durable_plan_claims.len(), hashes.len());
}
#[test]
fn empty_startup_reconciliation_receipt_publishes_with_gate_already_open() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(config_factory(), &time_source);
    let dir = tempdir().expect("empty startup-reconciliation journal directory");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let receipt = checked_startup_reconciliation_receipt(&queue);
    assert!(receipt.initial_snapshot.is_empty());
    assert!(
        !queue.lane_reservation_startup_reconciliation_pending(),
        "an initially empty durable replay has no quarantined owner"
    );
    queue
        .complete_lane_reservation_startup_reconciliation(receipt)
        .expect("an exact empty replay receipt may idempotently publish an open gate");
    assert!(!queue.lane_reservation_startup_reconciliation_pending());
}
#[test]
fn reservation_restart_release_restores_exact_global_fifo() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("fifo-plans.norito");
    let reservation_path = dir.path().join("fifo-reservations.norito");
    let txs = (0..5)
        .map(|_| accepted_queue_plan_tx_by_someone(&time_source))
        .collect::<Vec<_>>();
    let hashes = txs
        .iter()
        .map(AcceptedTransaction::hash_as_entrypoint)
        .collect::<Vec<_>>();
    let keys = {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install reservation journal");
        for tx in txs[..3].iter().cloned() {
            push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, tx);
        }
        let keys = queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"restart-fifo-owner", b"restart-fifo-proposal"),
                nonzero!(2_usize),
            )
            .expect("reserve restart A/B")
            .iter()
            .map(|transaction| *transaction.key())
            .collect::<Vec<_>>();
        for tx in txs[3..].iter().cloned() {
            push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, tx);
        }
        keys
    };
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore durable FIFO ordinals before payload replay");
    queue
        .install_plan_journal(&plan_path, 1024 * 1024, true)
        .expect("install plan journal");
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("replay A/B/C/D/E")
            .replayed,
        5
    );
    let replay_receipt = queue
        .lane_reservation_snapshot_replay_receipt()
        .expect("retain exact live-owner replay identity");
    let reconciliation_snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture exact live-owner reconciliation snapshot");
    assert_eq!(reconciliation_snapshot.ordered_records.len(), 2);
    assert_eq!(reconciliation_snapshot.ordered_groups.len(), 1);
    assert_eq!(
        reconciliation_snapshot.ordered_groups[0].ordered_keys.len(),
        2
    );
    assert!(
        replay_receipt
            .binds_reconciliation_snapshot(&reconciliation_snapshot)
            .expect("validate exact live-owner reconciliation identity")
    );
    let mut drifted_timestamp = reconciliation_snapshot.clone();
    drifted_timestamp.ordered_records[0].enqueue_timestamp_ms ^= 1;
    assert_eq!(
        drifted_timestamp.durable_owner_count(),
        reconciliation_snapshot.durable_owner_count()
    );
    assert!(
        !replay_receipt
            .binds_reconciliation_snapshot(&drifted_timestamp)
            .expect("validate drifted live-owner timestamp"),
        "equal-count live owners cannot substitute another enqueue timestamp"
    );
    let mut drifted_fifo = reconciliation_snapshot.clone();
    drifted_fifo.ordered_records[0].fifo_ordinal = drifted_fifo.ordered_records[0]
        .fifo_ordinal
        .checked_add(10)
        .expect("small FIFO fixture increment");
    assert_eq!(
        drifted_fifo.durable_owner_count(),
        reconciliation_snapshot.durable_owner_count()
    );
    assert!(
        !replay_receipt
            .binds_reconciliation_snapshot(&drifted_fifo)
            .expect("validate drifted live-owner FIFO ordinal"),
        "equal-count live owners cannot substitute another FIFO identity"
    );
    let mut reordered_group = reconciliation_snapshot.clone();
    reordered_group.ordered_groups[0].ordered_keys.swap(0, 1);
    assert_eq!(
        reordered_group.durable_owner_count(),
        reconciliation_snapshot.durable_owner_count()
    );
    assert!(
        replay_receipt
            .binds_reconciliation_snapshot(&reordered_group)
            .is_err(),
        "equal-count proposal groups cannot reorder their exact FIFO membership"
    );
    let mut changed_group_membership = reconciliation_snapshot.clone();
    changed_group_membership.ordered_groups[0].ordered_keys[0] =
        changed_group_membership.ordered_groups[0].ordered_keys[1];
    assert_eq!(
        changed_group_membership.durable_owner_count(),
        reconciliation_snapshot.durable_owner_count()
    );
    assert!(
        replay_receipt
            .binds_reconciliation_snapshot(&changed_group_membership)
            .is_err(),
        "equal-count proposal groups cannot replace one exact member"
    );
    let reconciliation_receipt = checked_startup_reconciliation_receipt(&queue);
    assert_eq!(
        queue
            .release_lane_reservations_in_order(&keys)
            .expect("release replayed A/B"),
        2
    );
    assert!(
        queue.lane_reservation_startup_reconciliation_pending(),
        "restart release must remain quarantined until the State/Kura publication gate"
    );
    assert!(
        queue
            .lane_reservation_reconciliation_snapshot()
            .expect("capture empty post-release reconciliation snapshot")
            .is_empty(),
        "consuming the last owner leaves an empty snapshot while its original gate stays closed"
    );
    queue
        .complete_lane_reservation_startup_reconciliation(reconciliation_receipt)
        .expect("publish completed FIFO restart reconciliation");
    assert!(!queue.lane_reservation_startup_reconciliation_pending());
    let mut selected = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(5_usize), &mut selected);
    assert_eq!(
        selected
            .iter()
            .map(|transaction| transaction.as_ref().hash_as_entrypoint())
            .collect::<Vec<_>>(),
        hashes
    );
    drop(selected);
    assert_eq!(queue.active_len(), hashes.len());
    assert_eq!(queue.queued_len(), hashes.len());
    assert_eq!(queue.durable_plan_claims.len(), hashes.len());
}
#[test]
fn reservation_restart_fits_ordinary_fifo_around_middle_anchor() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("middle-fifo-plans.norito");
    let reservation_path = dir.path().join("middle-fifo-reservations.norito");
    let txs = (0..3)
        .map(|_| accepted_queue_plan_tx_by_someone(&time_source))
        .collect::<Vec<_>>();
    let hashes = txs
        .iter()
        .map(AcceptedTransaction::hash_as_entrypoint)
        .collect::<Vec<_>>();
    let reserved_key = {
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install middle-anchor plan journal");
        queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("install middle-anchor reservation journal");
        for tx in txs.iter().cloned() {
            push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, tx);
        }
        let excluded = BTreeSet::from([txs[0].hash_as_entrypoint()]);
        let reserved = queue
            .reserve_transactions_for_lane_excluding(
                &state,
                lane_reservation_scope(&state, b"middle-fifo-owner", b"middle-fifo-proposal"),
                nonzero!(1_usize),
                &excluded,
            )
            .expect("reserve only middle transaction B");
        assert_eq!(reserved.len(), 1);
        assert_eq!(reserved[0].as_accepted().hash_as_entrypoint(), hashes[1]);
        *reserved[0].key()
    };
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let reservation_replay = queue
        .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
        .expect("restore middle durable FIFO anchor before payload replay");
    assert_eq!(reservation_replay.restored, 1);
    assert_eq!(reservation_replay.awaiting_transaction_replay, 1);
    assert_eq!(
        queue
            .install_plan_journal(&plan_path, 1024 * 1024, true)
            .expect("install middle-anchor plan journal"),
        3
    );
    assert_eq!(
        queue
            .replay_plan_journal(&state)
            .expect("fit A and C around durable B")
            .replayed,
        3
    );
    let reconciliation_receipt = checked_startup_reconciliation_receipt(&queue);
    let stale_reconciliation_receipt = checked_startup_reconciliation_receipt(&queue);
    queue
        .complete_lane_reservation_startup_reconciliation(reconciliation_receipt)
        .expect("publish completed middle-anchor startup reconciliation");
    assert!(
        !queue.lane_reservation_startup_reconciliation_pending(),
        "the first exact non-empty receipt opens the startup gate"
    );
    assert!(matches!(
        queue.complete_lane_reservation_startup_reconciliation(stale_reconciliation_receipt),
        Err(LaneQueueReservationError::InvalidIdentity(ref reason))
            if reason.contains("stale at the final publication gate")
    ));
    for (index, hash) in hashes.iter().copied().enumerate() {
        assert_eq!(
            queue
                .fifo_order_by_hash
                .get(&hash)
                .map(|entry| entry.value().ordinal),
            Some(u64::try_from(index).expect("small fixture index") + 1),
        );
    }
    assert_eq!(queue.queued_len(), 2);
    assert_eq!(
        queue
            .release_lane_reservations_in_order(&[reserved_key])
            .expect("release middle FIFO anchor"),
        1
    );
    let mut selected = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(3_usize), &mut selected);
    assert_eq!(
        selected
            .iter()
            .map(|transaction| transaction.as_ref().hash_as_entrypoint())
            .collect::<Vec<_>>(),
        hashes,
        "restart replay must preserve A/B/C when only B retained a durable FIFO ordinal",
    );
    drop(selected);
    assert_eq!(queue.active_len(), hashes.len());
    assert_eq!(queue.queued_len(), hashes.len());
    assert_eq!(queue.durable_plan_claims.len(), hashes.len());
}
#[test]
fn bounded_lane_reservation_charges_scans_bytes_and_gas() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let txs = (0..3)
        .map(|_| accepted_queue_plan_tx_by_someone(&time_source))
        .collect::<Vec<_>>();
    let hashes = txs
        .iter()
        .map(AcceptedTransaction::hash_as_entrypoint)
        .collect::<Vec<_>>();
    for tx in &txs {
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, tx.clone());
    }
    let excluded = txs
        .iter()
        .take(2)
        .map(AcceptedTransaction::hash_as_entrypoint)
        .collect::<BTreeSet<_>>();
    let limits = |max_scan, max_encoded_bytes, max_gas| LaneQueueReservationSelectionLimits {
        max_transactions: nonzero!(3_usize),
        max_scan: NonZeroUsize::new(max_scan).expect("positive scan bound"),
        max_encoded_bytes: NonZeroU64::new(max_encoded_bytes).expect("positive byte bound"),
        max_gas: NonZeroU64::new(max_gas).expect("positive gas bound"),
    };
    assert!(
        queue
            .reserve_transactions_for_lane_bounded(
                &state,
                AutonomousLaneReservationSelectionAuthorization::single_validator_for_test(
                    lane_reservation_scope(&state, b"scan-owner", b"scan-proposal"),
                ),
                limits(2, u64::MAX, u64::MAX),
                &excluded,
                LaneQueueReservationRoutingMode::AnyCoordinatorPlan,
            )
            .expect("bounded scan")
            .is_empty(),
        "both excluded FIFO slots consume the complete scan budget"
    );
    queue.tx_encoded_len.insert(hashes[0], 10);
    queue.tx_encoded_len.insert(hashes[1], 11);
    let byte_reserved = queue
        .reserve_transactions_for_lane_bounded(
            &state,
            AutonomousLaneReservationSelectionAuthorization::single_validator_for_test(
                lane_reservation_scope(&state, b"byte-owner", b"byte-proposal"),
            ),
            limits(3, 10, u64::MAX),
            &BTreeSet::new(),
            LaneQueueReservationRoutingMode::AnyCoordinatorPlan,
        )
        .expect("bounded byte reservation");
    assert_eq!(byte_reserved.len(), 1);
    assert_eq!(
        byte_reserved[0].as_accepted().hash_as_entrypoint(),
        hashes[0]
    );
    queue
        .release_lane_reservation(byte_reserved[0].key())
        .expect("release byte-bounded owner");
    queue.tx_gas_cost.insert(hashes[0], 5);
    queue.tx_gas_cost.insert(hashes[1], 6);
    let gas_reserved = queue
        .reserve_transactions_for_lane_bounded(
            &state,
            AutonomousLaneReservationSelectionAuthorization::single_validator_for_test(
                lane_reservation_scope(&state, b"gas-owner", b"gas-proposal"),
            ),
            limits(3, u64::MAX, 5),
            &BTreeSet::new(),
            LaneQueueReservationRoutingMode::AnyCoordinatorPlan,
        )
        .expect("bounded gas reservation");
    assert_eq!(gas_reserved.len(), 1);
    assert_eq!(
        gas_reserved[0].as_accepted().hash_as_entrypoint(),
        hashes[0]
    );
}
#[test]
fn bounded_lane_reservation_rejects_4097_before_fifo_or_journal_mutation() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    let reservation_path = install_globally_certified_test_reservation_journals(&queue, &dir);
    let transaction = accepted_queue_plan_tx_by_someone(&time_source);
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction.clone());
    let journal_len_before = std::fs::metadata(&reservation_path)
        .expect("stat empty reservation journal")
        .len();
    let queued_before = queue.queued_len();
    let error = match queue.reserve_transactions_for_lane_bounded(
        &state,
        AutonomousLaneReservationSelectionAuthorization::single_validator_for_test(
            lane_reservation_scope(&state, b"oversize-owner", b"oversize-proposal"),
        ),
        LaneQueueReservationSelectionLimits {
            max_transactions: NonZeroUsize::new(4_097).expect("non-zero oversize bound"),
            max_scan: nonzero!(1_usize),
            max_encoded_bytes: NonZeroU64::new(u64::MAX).expect("maximum byte bound is non-zero"),
            max_gas: NonZeroU64::new(u64::MAX).expect("maximum gas bound is non-zero"),
        },
        &BTreeSet::new(),
        LaneQueueReservationRoutingMode::AnyCoordinatorPlan,
    ) {
        Ok(_) => panic!("4,097-entry autonomous selection must fail closed"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        LaneQueueReservationError::InvalidIdentity(reason)
            if reason.contains("exceeds the first-release maximum 4096")
    ));
    assert_eq!(queue.queued_len(), queued_before);
    assert!(queue.live_lane_reservations().is_empty());
    assert_eq!(
        std::fs::metadata(&reservation_path)
            .expect("restat reservation journal")
            .len(),
        journal_len_before,
        "oversize rejection must precede the reservation fsync"
    );
}
#[test]
fn committing_reservation_owned_transaction_does_not_create_fifo_tombstone() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let transaction = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let hash = transaction.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
    let reserved = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(&state, b"tombstone-owner", b"tombstone-proposal"),
            nonzero!(1_usize),
        )
        .expect("reserve transaction");
    assert_eq!(reserved.len(), 1);
    assert!(
        !queue.queued_tx_enqueued_at_ms.contains_key(&hash),
        "lane reservation removes ordinary FIFO ownership"
    );
    assert_eq!(queue.remove_committed_hashes([hash], None), 1);
    assert!(
        queue.removed_hashes.is_empty(),
        "a reservation-owned hash has no stale FIFO cell and must not leave a tombstone"
    );
}

#[cfg(feature = "telemetry")]
#[test]
fn nexus_revalidation_preserves_committed_live_reservation_teu_until_terminal_cleanup() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);

    let transaction = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let hash = transaction.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
    let key = *queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(
                &state,
                b"committed-revalidation-owner",
                b"committed-revalidation-proposal",
            ),
            nonzero!(1_usize),
        )
        .expect("reserve transaction before committed Nexus revalidation")[0]
        .key();
    assert!(queue.tx_teu.contains_key(&hash));

    {
        let mut transactions = state.transactions.block();
        transactions.insert_block_with_single_tx(hash, nonzero!(1_usize));
        transactions
            .commit()
            .expect("publish committed transaction identity");
    }
    queue.reconfigure_nexus_with_state(&state.nexus_snapshot(), &state, None);
    assert_eq!(
        queue.live_lane_reservations(),
        vec![key],
        "post-WSV Nexus revalidation must preserve the exact producer reservation owner",
    );
    assert!(
        !queue.global_selection_owners.lock().contains_key(&hash),
        "the committed producer candidate must not regain an ordinary selection owner",
    );
    assert!(
        queue.fifo_order_by_hash.contains_key(&hash),
        "the lane-owned producer candidate must retain its durable FIFO identity",
    );
    assert!(
        queue.fifo_snapshot_for_test().is_empty(),
        "the committed producer candidate must remain absent from the physical FIFO",
    );
    assert!(
        !queue.removed_hashes.contains_key(&hash),
        "the reservation-owned producer candidate must not gain a lazy FIFO tombstone",
    );
    assert!(
        !queue.queued_tx_enqueued_at_ms.contains_key(&hash),
        "the reservation-owned producer candidate must not regain FIFO telemetry ownership",
    );
    assert!(
        queue.tx_teu.contains_key(&hash),
        "post-WSV Nexus revalidation must retain telemetry ownership until canonical Queue cleanup",
    );
    let teu = *queue
        .tx_teu
        .get(&hash)
        .expect("revalidated reservation TEU identity")
        .value();
    assert_eq!(
        queue
            .lane_teu_pending
            .get(&teu.lane_id)
            .map(|pending| (pending.teu, pending.tx_count)),
        Some((teu.teu, 1)),
        "TEU revalidation must not double-count the retained reservation owner",
    );

    assert_eq!(
        queue
            .commit_lane_reservation_group(&[key])
            .expect("complete canonical Queue cleanup after Nexus revalidation"),
        1,
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert!(!queue.txs.contains_key(&hash));
    assert!(!queue.routing_plans.contains_key(&hash));
    assert!(!queue.durable_plan_claims.contains_key(&hash));
    assert!(!queue.fifo_order_by_hash.contains_key(&hash));
    assert!(!queue.removed_hashes.contains_key(&hash));
    assert!(!queue.tx_encoded_len.contains_key(&hash));
    assert!(!queue.tx_gas_cost.contains_key(&hash));
    assert!(!queue.tx_enqueued_at_ms.contains_key(&hash));
    assert!(!queue.queued_tx_enqueued_at_ms.contains_key(&hash));
    assert!(!queue.expiry_ring_members.contains_key(&hash));
    assert!(!queue.tx_teu.contains_key(&hash));
    assert!(queue.fifo_snapshot_for_test().is_empty());
}
#[cfg(feature = "telemetry")]
#[test]
fn nexus_revalidation_preserves_commit_barrier_teu_until_resumed_cleanup() {
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);

    let first = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let first_hash = first.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, first);
    time_handle.advance(Duration::from_millis(1));
    let second = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let second_hash = second.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, second);
    let keys = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(
                &state,
                b"commit-barrier-revalidation-owner",
                b"commit-barrier-revalidation-proposal",
            ),
            nonzero!(2_usize),
        )
        .expect("reserve two-member revalidation group")
        .iter()
        .map(|reserved| *reserved.key())
        .collect::<Vec<_>>();
    assert_eq!(
        queue
            .commit_lane_reservation_group_prefix_for_test(&keys, 1)
            .expect("persist one reservation Commit prefix"),
        1,
    );
    assert!(queue.lane_reservation_commit_barriers().contains(&keys[0]));
    assert_eq!(queue.live_lane_reservations(), vec![keys[1]]);

    {
        let mut transactions = state.transactions.block();
        transactions.insert_block(
            [first_hash, second_hash].into_iter().collect(),
            nonzero!(1_usize),
        );
        transactions
            .commit()
            .expect("publish committed group transaction identities");
    }
    queue.reconfigure_nexus_with_state(&state.nexus_snapshot(), &state, None);
    for hash in [first_hash, second_hash] {
        assert!(
            queue.tx_teu.contains_key(&hash),
            "Nexus revalidation must retain TEU for both Commit-barrier and live reservation owners",
        );
    }
    assert!(queue.lane_reservation_commit_barriers().contains(&keys[0]));
    assert_eq!(queue.live_lane_reservations(), vec![keys[1]]);

    assert_eq!(
        queue
            .commit_lane_reservation_group(&keys)
            .expect("resume exact canonical cleanup from Commit prefix"),
        1,
        "only the still-live suffix is newly committed",
    );
    assert!(queue.lane_reservation_commit_barriers().is_empty());
    assert!(queue.live_lane_reservations().is_empty());
    for hash in [first_hash, second_hash] {
        assert!(!queue.tx_teu.contains_key(&hash));
        assert!(!queue.txs.contains_key(&hash));
    }
}
#[cfg(feature = "telemetry")]
#[test]
fn nexus_revalidation_preserves_teu_during_reservation_fsync_window() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);

    let transaction = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let hash = transaction.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
    let reached = Arc::new(Barrier::new(2));
    let resume = Arc::new(Barrier::new(2));
    queue
        .lane_reservation_journal
        .lock()
        .as_mut()
        .expect("installed reservation journal")
        .install_append_handoff(Arc::clone(&reached), Arc::clone(&resume));

    thread::scope(|scope| {
        let reservation_queue = Arc::clone(&queue);
        let reservation_state = &state;
        let reservation = scope.spawn(move || {
            reservation_queue.reserve_transactions_for_lane(
                reservation_state,
                lane_reservation_scope(
                    reservation_state,
                    b"revalidation-fsync-owner",
                    b"revalidation-fsync-proposal",
                ),
                nonzero!(1_usize),
            )
        });
        reached.wait();
        assert!(queue.durability_transition_active(&hash));
        assert!(queue.live_lane_reservations().is_empty());

        let router = queue.router.read().clone();
        let lane_catalog = queue.lane_catalog.read().clone();
        let dataspace_catalog = queue.dataspace_catalog.read().clone();
        queue.revalidate_pending_transactions_with_state(
            &router,
            &state,
            &lane_catalog,
            &dataspace_catalog,
            true,
        );
        assert!(
            queue.tx_teu.contains_key(&hash),
            "Nexus revalidation must not clear TEU while durable reservation publication is in flight",
        );

        resume.wait();
        let reserved = reservation
            .join()
            .expect("join reservation thread")
            .expect("publish reservation after revalidation");
        assert_eq!(reserved.len(), 1);
        assert_eq!(reserved[0].as_accepted().hash_as_entrypoint(), hash);
        assert_eq!(queue.live_lane_reservations(), vec![*reserved[0].key()]);
        assert!(queue.tx_teu.contains_key(&hash));
    });
}
#[cfg(feature = "telemetry")]
#[test]
fn nexus_revalidation_observes_reservation_published_after_teu_snapshot() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);

    let transaction = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let hash = transaction.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
    let reached = Arc::new(Barrier::new(2));
    let resume = Arc::new(Barrier::new(2));
    *queue.nexus_revalidation_snapshot_handoff.lock() = Some(QueueDurabilityObserverLockHandoff {
        reached: Arc::clone(&reached),
        resume: Arc::clone(&resume),
    });

    thread::scope(|scope| {
        let observer_queue = Arc::clone(&queue);
        let observer_state = &state;
        let observer = scope.spawn(move || {
            let router = observer_queue.router.read().clone();
            let lane_catalog = observer_queue.lane_catalog.read().clone();
            let dataspace_catalog = observer_queue.dataspace_catalog.read().clone();
            observer_queue.revalidate_pending_transactions_with_state(
                &router,
                observer_state,
                &lane_catalog,
                &dataspace_catalog,
                true,
            );
        });
        reached.wait();

        let reserved = queue
            .reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(&state, b"post-snapshot-owner", b"post-snapshot-proposal"),
                nonzero!(1_usize),
            )
            .expect("publish reservation after the revalidation snapshot");
        let key = *reserved[0].key();
        {
            let mut transactions = state.transactions.block();
            transactions.insert_block_with_single_tx(hash, nonzero!(1_usize));
            transactions
                .commit()
                .expect("publish committed transaction identity");
        }

        resume.wait();
        observer.join().expect("join Nexus revalidation observer");
        assert_eq!(queue.live_lane_reservations(), vec![key]);
        assert!(
            queue.tx_teu.contains_key(&hash),
            "the current live owner must supersede the stale pre-reservation snapshot",
        );
        assert_eq!(
            queue
                .commit_lane_reservation_group(&[key])
                .expect("terminal cleanup after post-snapshot reservation"),
            1,
        );
        assert!(!queue.tx_teu.contains_key(&hash));
    });
}
#[test]
fn lane_reservation_drains_committed_physical_fifo_tombstone() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let committed = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let committed_hash = committed.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, committed);
    let candidate = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let candidate_hash = candidate.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, candidate);
    assert_eq!(queue.remove_committed_hashes([committed_hash], None), 1);
    assert!(queue.removed_hashes.contains_key(&committed_hash));
    assert!(!queue.txs.contains_key(&committed_hash));
    assert!(!queue.fifo_order_by_hash.contains_key(&committed_hash));
    let unrelated_non_fifo_fence =
        accepted_unique_entrypoint_tx_by_someone(&time_source).hash_as_entrypoint();
    queue.removed_hashes.insert(unrelated_non_fifo_fence, ());
    {
        let _queue_guard = queue.push_remove_lock.lock();
        assert_eq!(
            queue.fifo_snapshot_locked(),
            vec![committed_hash, candidate_hash],
            "committed removal intentionally leaves its physical FIFO cell for the next consumer"
        );
    }
    let reserved = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(
                &state,
                b"committed-tombstone-owner",
                b"committed-tombstone-proposal",
            ),
            nonzero!(1_usize),
        )
        .expect("terminal committed tombstone must not block autonomous reservation");
    assert_eq!(reserved.len(), 1);
    assert_eq!(
        reserved[0].as_accepted().hash_as_entrypoint(),
        candidate_hash
    );
    assert!(
        !queue.removed_hashes.contains_key(&committed_hash),
        "the exact physical tombstone was atomically drained"
    );
    assert!(
        queue.removed_hashes.contains_key(&unrelated_non_fifo_fence),
        "FIFO reconstruction must preserve unrelated non-FIFO removal fences"
    );
    assert!(queue.tx_hashes.is_empty());
    assert!(!queue.accepted_work_validation_faulted());
    assert!(!queue.lane_reservation_durability_faulted());
}
#[test]
fn lane_reservation_rejects_tracked_fifo_hash_without_order_identity() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let transaction = accepted_queue_plan_unique_entrypoint_tx_by_someone(&time_source);
    let hash = transaction.hash_as_entrypoint();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);
    assert!(queue.fifo_order_by_hash.remove(&hash).is_some());
    assert!(!queue.removed_hashes.contains_key(&hash));
    assert!(queue.txs.contains_key(&hash));
    let error = match queue.reserve_transactions_for_lane(
        &state,
        lane_reservation_scope(
            &state,
            b"missing-fifo-order-owner",
            b"missing-fifo-order-proposal",
        ),
        nonzero!(1_usize),
    ) {
        Err(error) => error,
        Ok(_) => panic!("a tracked physical hash without FIFO identity must fail closed"),
    };
    assert!(matches!(
        error,
        LaneQueueReservationError::InvalidIdentity(reason)
            if reason.contains("inconsistent FIFO ownership")
                && reason.contains("removed=false")
                && reason.contains("tracked=true")
                && reason.contains("fifo_order=false")
    ));
    {
        let _queue_guard = queue.push_remove_lock.lock();
        assert_eq!(queue.fifo_snapshot_locked(), vec![hash]);
    }
    assert!(queue.txs.contains_key(&hash));
    assert!(!queue.removed_hashes.contains_key(&hash));
}
#[test]
fn lane_pending_work_snapshot_separates_ordinary_and_exact_reservation_ownership() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_test_reservation_journal(&queue, &dir);
    push_globally_bound_lane_reservation_candidate(
        &queue,
        &state,
        &dir,
        accepted_queue_plan_tx_by_someone(&time_source),
    );
    let scope = lane_reservation_scope(&state, b"pending-work-owner", b"pending-work-proposal");
    let other_incarnation = Hash::new(b"pending-work-recreated-incarnation");
    assert!(
        queue.lane_has_pending_work(scope.lane_id, scope.dataspace_id, scope.lane_incarnation,)
    );
    assert!(
        queue.lane_has_pending_work(scope.lane_id, scope.dataspace_id, other_incarnation,),
        "ordinary route work blocks the route before an incarnation owns it",
    );
    assert!(!queue.lane_has_pending_work(
        LaneId::new(77),
        scope.dataspace_id,
        scope.lane_incarnation,
    ));
    assert!(!queue.lane_has_pending_work(
        scope.lane_id,
        DataSpaceId::new(77),
        scope.lane_incarnation,
    ));
    let key = *queue
        .reserve_transactions_for_lane(&state, scope, nonzero!(1_usize))
        .expect("reserve ordinary work")[0]
        .key();
    assert!(
        queue.lane_has_pending_work(scope.lane_id, scope.dataspace_id, scope.lane_incarnation,)
    );
    assert!(
        !queue.lane_has_pending_work(scope.lane_id, scope.dataspace_id, other_incarnation,),
        "a live reservation blocks only its exact incarnation",
    );
    {
        queue
            .lane_reservation_journal
            .lock()
            .as_mut()
            .expect("reservation journal")
            .commit(key)
            .expect("persist simulated commit barrier");
        let mut reservations = queue.lane_reservations.lock();
        reservations.live_by_entrypoint.remove(&key.entrypoint_hash);
        reservations.commit_barriers.push(key);
    }
    assert!(
        queue.lane_has_pending_work(scope.lane_id, scope.dataspace_id, scope.lane_incarnation,)
    );
    assert!(
        !queue.lane_has_pending_work(scope.lane_id, scope.dataspace_id, other_incarnation,),
        "a commit barrier blocks only its exact incarnation",
    );
    queue
        .lane_reservation_durability_fault
        .store(true, Ordering::Release);
    assert!(
        queue.lane_has_pending_work(LaneId::new(88), DataSpaceId::new(88), other_incarnation,),
        "ambiguous reservation durability must fail closed for every drain query",
    );
}
#[test]
fn lane_pending_work_rechecks_durability_fault_after_queue_lock_handoff() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let lane_id = LaneId::new(91);
    let dataspace_id = DataSpaceId::new(91);
    let lane_incarnation = Hash::new(b"pending-work-lock-handoff-incarnation");
    assert!(
        !queue.lane_has_pending_work(lane_id, dataspace_id, lane_incarnation),
        "an unrelated healthy empty route must initially be drainable"
    );
    let reached = Arc::new(Barrier::new(2));
    let resume = Arc::new(Barrier::new(2));
    *queue.durability_observer_lock_handoff.lock() = Some(QueueDurabilityObserverLockHandoff {
        reached: Arc::clone(&reached),
        resume: Arc::clone(&resume),
    });
    let queue_guard = queue.push_remove_lock.lock();
    let observer_queue = Arc::clone(&queue);
    let observer = std::thread::spawn(move || {
        observer_queue.lane_has_pending_work(lane_id, dataspace_id, lane_incarnation)
    });
    reached.wait();
    queue
        .plan_journal_durability_fault
        .store(true, Ordering::Release);
    resume.wait();
    drop(queue_guard);
    assert!(
        observer.join().expect("join pending-work handoff observer"),
        "a fault published after the optimistic precheck must block drain under the queue lock"
    );
}
#[test]
fn lane_retirement_observer_holds_transition_before_lifecycle_fence() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Queue::test(config_factory(), &time_source);
    let lane_id = LaneId::new(92);
    let dataspace_id = DataSpaceId::new(92);
    let lane_incarnation = Hash::new(b"retirement-observer-lock-order-incarnation");
    let observer = queue.lock_lane_retirement_observer();
    assert!(
        queue.lane_reservation_transition_lock.try_lock().is_none(),
        "the retirement observer must retain the reservation-transition fence"
    );
    let lifecycle_guard = state.lock_lane_lifecycle_work_admission();
    assert!(
        !observer.lane_has_pending_work(lane_id, dataspace_id, lane_incarnation),
        "the guarded predicate must not reacquire the reservation-transition lock"
    );
    drop(lifecycle_guard);
    drop(observer);
    assert!(
        queue.lane_reservation_transition_lock.try_lock().is_some(),
        "dropping the observer must release the reservation-transition fence"
    );
}
#[test]
fn reservation_journal_install_rejects_selection_publication_window() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(config_factory(), &time_source);
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("selection-window.norito");
    let attempt = queue.begin_selection_attempt();
    assert!(matches!(
        queue.install_lane_reservation_journal(&path, 1024 * 1024),
        Err(LaneQueueReservationError::InflightSelection)
    ));
    assert!(
        !path.exists(),
        "a rejected startup installation must not create or repair its journal path"
    );
    drop(attempt);
    queue
        .install_lane_reservation_journal(&path, 1024 * 1024)
        .expect("install after selection window closes");
    assert!(queue.lane_reservation_journal_installed());
    assert_eq!(
        queue
            .lane_reservation_snapshot_replay_receipt()
            .expect("installed journal retains its replay receipt")
            .owner_transition_count(),
        0,
        "an empty journal must not manufacture a primitive owner transition"
    );
}
#[test]
fn second_reservation_journal_installer_cannot_touch_its_losing_path() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Queue::test(config_factory(), &time_source);
    let dir = tempdir().expect("tempdir");
    let installed_path = dir.path().join("installed.norito");
    let losing_path = dir.path().join("losing.norito");
    queue
        .install_lane_reservation_journal(&installed_path, 1024 * 1024)
        .expect("install the one active reservation journal");
    assert!(matches!(
        queue.install_lane_reservation_journal(&losing_path, 1024 * 1024),
        Err(LaneQueueReservationError::JournalAlreadyInstalled)
    ));
    assert!(
        !losing_path.exists(),
        "the losing installer must fail before opening, creating, repairing, or truncating storage"
    );
}
#[test]
fn concurrent_reservation_journal_installers_publish_one_untouched_winner() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    let paths = [
        dir.path().join("candidate-a.norito"),
        dir.path().join("candidate-b.norito"),
    ];
    let start = Arc::new(Barrier::new(3));
    let attempts = paths
        .iter()
        .cloned()
        .map(|path| {
            let queue = Arc::clone(&queue);
            let start = Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                let result = queue.install_lane_reservation_journal(&path, 1024 * 1024);
                (path, result)
            })
        })
        .collect::<Vec<_>>();
    start.wait();
    let results = attempts
        .into_iter()
        .map(|attempt| attempt.join().expect("join concurrent installer"))
        .collect::<Vec<_>>();
    assert_eq!(
        results.iter().filter(|(_, result)| result.is_ok()).count(),
        1,
        "exactly one concurrent installer must publish"
    );
    for (path, result) in results {
        match result {
            Ok(_) => assert!(path.exists(), "the winning journal must be durable"),
            Err(LaneQueueReservationError::JournalAlreadyInstalled) => assert!(
                !path.exists(),
                "the losing installer must not touch its candidate path"
            ),
            Err(error) => panic!("unexpected concurrent installation error: {error}"),
        }
    }
}
#[test]
fn ambiguous_reservation_put_disables_global_and_lane_selection_until_restart_recovery() {
    for (fault, expected_restored) in [
        (ReservationJournalAppendFault::PartialWrite, 0),
        (ReservationJournalAppendFault::SyncAfterFullWrite, 1),
    ] {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let state = lane_reservation_test_state();
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join(format!("ambiguous-{fault:?}.norito"));
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        let pressure_rx = queue.backpressure_handle().subscribe();
        queue
            .install_plan_journal(
                dir.path().join(format!("ambiguous-plans-{fault:?}.norito")),
                1024 * 1024,
                true,
            )
            .expect("install globally certified reservation plan journal");
        queue
            .install_lane_reservation_journal(&path, 1024 * 1024)
            .expect("install reservation journal");
        let tx = accepted_queue_plan_tx_by_someone(&time_source);
        let hash = tx.hash_as_entrypoint();
        push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, tx);
        queue
            .lane_reservation_journal
            .lock()
            .as_mut()
            .expect("installed journal")
            .inject_next_append_fault(fault);
        let result = queue.reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(&state, b"ambiguous-owner", b"ambiguous-proposal"),
            nonzero!(1_usize),
        );
        assert!(matches!(result, Err(LaneQueueReservationError::Journal(_))));
        assert!(queue.lane_reservation_durability_faulted());
        assert!(
            pressure_rx.borrow().is_saturated(),
            "ambiguous append must publish saturated pressure to existing observers"
        );
        assert_eq!(queue.active_len(), 1);
        assert_eq!(queue.queued_len(), 1);
        assert!(queue.live_lane_reservations().is_empty());
        assert!(
            !queue.durability_transition_active(&hash),
            "failed reservation append must release its exact transition fence"
        );
        let mut global = Vec::new();
        queue.get_transactions_for_block_with_state(&state, nonzero!(1_usize), &mut global);
        assert!(
            global.is_empty(),
            "ordinary selection must stop while durable ownership is ambiguous"
        );
        assert_eq!(queue.queued_len(), 1);
        assert!(queue.txs.contains_key(&hash));
        assert!(matches!(
            queue.reserve_transactions_for_lane(
                &state,
                lane_reservation_scope(
                    &state,
                    b"ambiguous-owner-retry",
                    b"ambiguous-proposal-retry"
                ),
                nonzero!(1_usize),
            ),
            Err(LaneQueueReservationError::DurabilityFault)
        ));
        // Force the empty-index recovery corridor. The sticky fault must keep the ambiguous
        // transaction out of ordinary FIFO selection instead of silently making it eligible.
        while queue.tx_hashes.pop().is_some() {}
        let mut after_forced_empty = Vec::new();
        queue.get_transactions_for_block_with_state(
            &state,
            nonzero!(1_usize),
            &mut after_forced_empty,
        );
        assert!(after_forced_empty.is_empty());
        assert!(queue.tx_hashes.is_empty());
        assert!(queue.txs.contains_key(&hash));
        drop(queue);
        let restarted = Queue::test(config_factory(), &time_source);
        let replay = restarted
            .install_lane_reservation_journal(&path, 1024 * 1024)
            .expect("restart must repair/replay the ambiguous append");
        assert!(!restarted.lane_reservation_durability_faulted());
        assert_eq!(replay.restored, expected_restored);
        assert_eq!(
            replay.awaiting_transaction_replay, expected_restored,
            "a fully framed append owns the payload; a torn prefix is truncated"
        );
    }
}
