// Lane-reservation durability, FIFO, and exact-release regression tests.
//
// Included by `queue::tests` so source-bound libtest names remain stable.

fn lane_reservation_test_state() -> Arc<State> {
    let mut state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    install_single_validator_topology_for_queue_test(&state, 0xD5);
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = false;
    state
        .set_nexus(nexus)
        .expect("use the authoritative canonical single-lane fixture");
    Arc::new(state)
}

fn lane_reservation_scope(
    state: &State,
    owner_seed: &[u8],
    proposal_seed: &[u8],
) -> LaneQueueReservationScopeV1 {
    LaneQueueReservationScopeV1 {
        lane_id: LaneId::SINGLE,
        dataspace_id: DataSpaceId::UNIVERSAL,
        lane_incarnation: state
            .lane_incarnation_at_height(LaneId::SINGLE, 1)
            .expect("default lane incarnation at first proposal height"),
        proposal_height: 1,
        lane_block_height: 1,
        lane_block_view: 0,
        reservation_owner_hash: Hash::new(owner_seed),
        proposal_identity_hash: Hash::new(proposal_seed),
    }
}

#[test]
fn lane_reservation_scope_accepts_only_canonical_single_lane_when_nexus_is_disabled() {
    let state = lane_reservation_test_state();
    let scope = lane_reservation_scope(&state, b"single-lane-owner", b"single-lane-proposal");

    assert!(Queue::validate_reservation_scope_against_view(&state.view(), scope).is_ok());

    let mut noncanonical = scope;
    noncanonical.dataspace_id = DataSpaceId::new(1);
    assert!(matches!(
        Queue::validate_reservation_scope_against_view(&state.view(), noncanonical),
        Err(LaneQueueReservationError::InactiveRoute)
    ));
}

#[test]
fn lane_reservation_key_requires_explicit_current_version() {
    #[derive(Encode)]
    struct LegacyLaneQueueReservationKeyV1 {
        signed_transaction_hash: HashOf<iroha_data_model::transaction::SignedTransaction>,
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        routing_plan_digest: Hash,
        coordinator_leg: RouteLeg,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        proposal_height: u64,
        lane_block_height: u64,
        lane_block_view: u64,
        reservation_owner_hash: Hash,
        proposal_identity_hash: Hash,
    }

    let route = RoutingDecision::new(LaneId::new(3), DataSpaceId::new(7));
    let entrypoint_hash = HashOf::from_untyped_unchecked(Hash::new(b"reservation-key-entrypoint"));
    let key = LaneQueueReservationKeyV2 {
        version: LaneQueueReservationKeyV2::VERSION,
        signed_transaction_hash: compatibility_queue_hash(entrypoint_hash.clone()),
        entrypoint_hash,
        queue_plan_admission_binding_hash: Hash::new(
            b"reservation-key-queue-plan-admission-binding",
        ),
        routing_plan_digest: Hash::new(b"reservation-key-routing-plan"),
        coordinator_leg: RouteLeg::new(route, RouteLegRole::Coordinator),
        lane_id: route.lane_id,
        dataspace_id: route.dataspace_id,
        lane_incarnation: Hash::new(b"reservation-key-incarnation"),
        proposal_height: 11,
        lane_block_height: 5,
        lane_block_view: 2,
        reservation_owner_hash: Hash::new(b"reservation-key-owner"),
        proposal_identity_hash: Hash::new(b"reservation-key-proposal"),
    };
    assert_eq!(key.validate(), Ok(()));
    let mut mismatched_queue_hash = key;
    mismatched_queue_hash.signed_transaction_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"mismatched compatibility queue hash"));
    assert_eq!(
        mismatched_queue_hash.validate(),
        Err("lane reservation compatibility transaction hash does not match its entrypoint")
    );
    let key_digest = key.digest();
    let framed = norito::encode_canonical(&key).expect("encode current reservation key");
    assert_eq!(
        norito::decode_canonical::<LaneQueueReservationKeyV2>(&framed)
            .expect("decode current reservation key"),
        key
    );
    {
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_eq!(
            key.digest(),
            key_digest,
            "reservation identity must ignore the caller's ambient Norito layout"
        );
    }

    let legacy = LegacyLaneQueueReservationKeyV1 {
        signed_transaction_hash: key.signed_transaction_hash,
        entrypoint_hash: key.entrypoint_hash,
        routing_plan_digest: key.routing_plan_digest,
        coordinator_leg: key.coordinator_leg,
        lane_id: key.lane_id,
        dataspace_id: key.dataspace_id,
        lane_incarnation: key.lane_incarnation,
        proposal_height: key.proposal_height,
        lane_block_height: key.lane_block_height,
        lane_block_view: key.lane_block_view,
        reservation_owner_hash: key.reservation_owner_hash,
        proposal_identity_hash: key.proposal_identity_hash,
    };
    let legacy_framed =
        norito::to_bytes(&legacy).expect("encode legacy versionless reservation key");
    assert!(
        norito::decode_from_bytes::<LaneQueueReservationKeyV2>(&legacy_framed).is_err(),
        "a versionless reservation key must fail closed"
    );

    for malformed_version in [0, LaneQueueReservationKeyV2::VERSION + 1] {
        let mut malformed = key;
        malformed.version = malformed_version;
        assert_eq!(
            malformed.validate(),
            Err("unsupported lane queue reservation key version")
        );
    }
}

fn install_test_reservation_journal(queue: &Queue, dir: &tempfile::TempDir) -> PathBuf {
    let path = dir.path().join("lane-queue-reservations.norito");
    assert!(!queue.lane_reservation_journal_installed());
    assert_eq!(
        queue
            .install_lane_reservation_journal(&path, 1024 * 1024)
            .expect("install reservation journal"),
        LaneQueueReservationReplaySummary::default()
    );
    assert!(queue.lane_reservation_journal_installed());
    path
}

fn install_globally_certified_test_reservation_journals(
    queue: &Queue,
    dir: &tempfile::TempDir,
) -> PathBuf {
    queue
        .install_plan_journal(
            dir.path().join("queue-plans-for-reservations.norito"),
            1024 * 1024,
            true,
        )
        .expect("install queue-plan journal for globally certified reservation fixture");
    install_test_reservation_journal(queue, dir)
}

fn test_lane_reservation_plan_path(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("lane-queue-plans.norito")
}

fn push_globally_bound_lane_reservation_candidate(
    queue: &Queue,
    state: &State,
    dir: &tempfile::TempDir,
    transaction: AcceptedTransaction<'static>,
) -> crate::torii_proxy::QueuePlanAdmissionBindingV2 {
    if queue.plan_journal.lock().is_none() {
        queue
            .install_plan_journal(test_lane_reservation_plan_path(dir), 1024 * 1024, true)
            .expect("install reservation-candidate queue-plan journal");
    }
    admit_globally_certified_reservation_transaction_for_test(queue, state, transaction)
}

fn persist_unreconciled_commit_barrier(
    queue: &Queue,
    state: &State,
    dir: &tempfile::TempDir,
    transaction: AcceptedTransaction<'static>,
    owner_seed: &[u8],
    proposal_seed: &[u8],
) -> LaneQueueReservationKeyV2 {
    push_globally_bound_lane_reservation_candidate(queue, state, dir, transaction);
    let key = *queue
        .reserve_transactions_for_lane(
            state,
            lane_reservation_scope(state, owner_seed, proposal_seed),
            nonzero!(1_usize),
        )
        .expect("reserve startup-boundary transaction")[0]
        .key();
    queue
        .lane_reservation_journal
        .lock()
        .as_mut()
        .expect("installed reservation journal")
        .commit(key)
        .expect("persist commit before simulated startup-boundary crash");
    let mut reservations = queue.lane_reservations.lock();
    reservations
        .live_by_hash
        .remove(&key.signed_transaction_hash);
    reservations.commit_barriers.push(key);
    key
}

#[test]
fn commit_barrier_owns_hash_until_plan_reconciliation() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let queue = Queue::test(config_factory(), &time_source);
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let transaction = accepted_unique_entrypoint_tx_by_someone(&time_source);
    let hash = transaction.hash();
    let key = persist_unreconciled_commit_barrier(
        &queue,
        &state,
        &dir,
        transaction,
        b"commit-owner",
        b"commit-proposal",
    );

    let store = queue.lane_reservations.lock();
    assert_eq!(store.commit_barriers, vec![key]);
    assert!(
        store.live_hashes().contains(&hash),
        "a durable Commit barrier must exclude its hash from every FIFO selector"
    );
    drop(store);
    assert_eq!(queue.pop_queued_hash(), None);
}

#[test]
fn plan_admission_append_never_owns_queue_mutation_lock() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    queue
        .install_plan_journal(
            dir.path().join("off-lock-plan-append.norito"),
            1024 * 1024,
            true,
        )
        .expect("install plan journal");
    let transaction = accepted_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(
        Arc::get_mut(&mut state).expect("unshared lane-reservation test state"),
        &transaction,
    );
    let hash = transaction.hash();
    let reached = Arc::new(Barrier::new(2));
    let resume = Arc::new(Barrier::new(2));
    queue
        .plan_journal
        .lock()
        .as_ref()
        .expect("installed plan journal")
        .install_append_handoff(Arc::clone(&reached), Arc::clone(&resume));

    thread::scope(|scope| {
        let queue_for_admission = Arc::clone(&queue);
        let state_for_admission = Arc::clone(&state);
        let admission = scope.spawn(move || {
            queue_for_admission.push_with_lane_with_state(transaction, &state_for_admission)
        });

        reached.wait();
        assert!(queue.durability_transition_active(&hash));
        assert!(
            queue.txs.contains_key(&hash) && queue.routing_plans.contains_key(&hash),
            "the fenced admission stages its transaction and immutable plan before storage"
        );
        assert!(
            !queue.durable_plan_claims.contains_key(&hash),
            "the durable claim must not publish before the exact Put completes"
        );
        assert_eq!(
            queue.queued_len(),
            0,
            "a staged plan without its durable claim must have no selectable FIFO membership"
        );
        assert!(
            queue.push_remove_lock.try_lock().is_some(),
            "plan-journal fsync must not own the queue mutation lock"
        );
        let state_view = state.view();
        let staged = queue
            .txs
            .get(&hash)
            .map(|entry| Arc::clone(entry.value()))
            .expect("staged transaction");
        assert_eq!(
            queue.immutable_queued_routing_plan_if_available_in_view(
                hash,
                staged.as_ref(),
                &state_view,
                state_view.nexus(),
                state_view_height_for_routing(&state_view),
            ),
            Ok(None),
            "the transition fence must make the partial plan temporarily unavailable, not invalid"
        );
        let (pending, _lease) = queue
            .bounded_pending_snapshot(&state_view, nonzero!(8_usize))
            .expect("selection remains healthy during an admission append");
        assert!(
            pending.is_empty(),
            "an admission is not visible before its durable Put completes"
        );
        assert!(
            !queue.accepted_work_validation_faulted(),
            "a content-valid partial admission must not trip the sticky validation latch"
        );

        resume.wait();
        admission
            .join()
            .expect("admission thread")
            .expect("durable admission");
    });
    assert!(!queue.durability_transition_active(&hash));
    assert!(queue.contains_transaction_hash(hash));
    assert_eq!(queue.queued_len(), 1);
    let claim = queue
        .durable_plan_claims
        .get(&hash)
        .expect("durable claim must publish before FIFO visibility");
    assert_eq!(
        queue
            .routing_plans
            .get(&hash)
            .map(|plan| plan.value().clone()),
        Some(claim.routing_plan.clone())
    );
    assert!(!queue.accepted_work_validation_faulted());
}

#[test]
fn ordinary_unbound_durable_claim_waits_for_global_admission_without_fault() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let queue = Queue::test(config_factory(), &time_source);
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let transaction = accepted_unique_entrypoint_tx_by_someone(&time_source);
    let hash = transaction.hash();
    register_accepted_tx_authority_for_queue_test(
        Arc::get_mut(&mut state).expect("unshared lane-reservation test state"),
        &transaction,
    );

    queue
        .push_with_lane_with_state(transaction, &state)
        .expect("persist ordinary unbound durable claim");
    assert!(
        queue
            .durable_plan_claims
            .get(&hash)
            .is_some_and(|claim| claim.global_admission_identity.is_none())
    );

    let reserved = queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(&state, b"unbound-owner", b"unbound-proposal"),
            nonzero!(1_usize),
        )
        .expect("ordinary claim must remain a healthy pending FIFO owner");
    assert!(reserved.is_empty());
    assert_eq!(queue.active_len(), 1);
    assert_eq!(queue.queued_len(), 1);
    assert!(queue.contains_transaction_hash(hash));
    assert!(
        queue
            .durable_plan_claims
            .get(&hash)
            .is_some_and(|claim| claim.global_admission_identity.is_none())
    );
    assert!(!queue.accepted_work_validation_faulted());
    assert!(!queue.lane_reservation_durability_faulted());
}

#[test]
fn reservation_append_does_not_convoy_unrelated_queue_removal() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let selected = accepted_unique_entrypoint_tx_by_someone(&time_source);
    let selected_hash = selected.hash();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, selected);
    let unrelated = accepted_unique_entrypoint_tx_by_someone(&time_source);
    let unrelated_hash = unrelated.hash();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, unrelated);
    let reached = Arc::new(Barrier::new(2));
    let resume = Arc::new(Barrier::new(2));
    queue
        .lane_reservation_journal
        .lock()
        .as_mut()
        .expect("installed reservation journal")
        .install_append_handoff(Arc::clone(&reached), Arc::clone(&resume));

    thread::scope(|scope| {
        let queue_for_reservation = Arc::clone(&queue);
        let state_for_reservation = Arc::clone(&state);
        let reservation = scope.spawn(move || {
            queue_for_reservation.reserve_transactions_for_lane(
                &state_for_reservation,
                lane_reservation_scope(
                    &state_for_reservation,
                    b"off-lock-owner",
                    b"off-lock-proposal",
                ),
                nonzero!(1_usize),
            )
        });

        reached.wait();
        assert!(queue.durability_transition_active(&selected_hash));
        assert!(
            queue.push_remove_lock.try_lock().is_some(),
            "reservation-journal fsync must not own the queue mutation lock"
        );

        let (removed_tx, removed_rx) = std::sync::mpsc::channel();
        let queue_for_removal = Arc::clone(&queue);
        let removal = scope.spawn(move || {
            let removed = queue_for_removal.remove_committed_hashes([unrelated_hash], None);
            removed_tx.send(removed).expect("report unrelated removal");
        });
        let unrelated_result = removed_rx.recv_timeout(Duration::from_secs(5));
        assert!(
            queue.push_remove_lock.try_lock().is_some(),
            "unrelated removal must not retain the queue lock while storage is blocked"
        );
        resume.wait();

        let reserved = reservation
            .join()
            .expect("reservation thread")
            .expect("durable reservation");
        removal.join().expect("removal thread");
        assert_eq!(
            unrelated_result.expect("unrelated removal must complete during blocked fsync"),
            1
        );
        assert_eq!(reserved.len(), 1);
        assert_eq!(reserved[0].key().signed_transaction_hash, selected_hash);
    });
    assert!(!queue.contains_transaction_hash(unrelated_hash));
    assert!(!queue.durability_transition_active(&selected_hash));
}

#[test]
fn release_recomputes_fifo_after_unrelated_admission_during_append() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let reserved_transaction = accepted_unique_entrypoint_tx_by_someone(&time_source);
    let reserved_hash = reserved_transaction.hash();
    push_globally_bound_lane_reservation_candidate(
        &queue,
        &state,
        &dir,
        reserved_transaction,
    );
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
    let unrelated = accepted_unique_entrypoint_tx_by_someone(&time_source);
    let unrelated_hash = unrelated.hash();
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
    let reserved_transaction = accepted_unique_entrypoint_tx_by_someone(&time_source);
    let reserved_hash = reserved_transaction.hash();
    push_globally_bound_lane_reservation_candidate(
        &queue,
        &state,
        &dir,
        reserved_transaction,
    );
    let unrelated = accepted_unique_entrypoint_tx_by_someone(&time_source);
    let unrelated_hash = unrelated.hash();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, unrelated);
    let key = *queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(
                &state,
                b"release-pop-owner",
                b"release-pop-proposal",
            ),
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
        assert_eq!(held[0].as_ref().hash(), unrelated_hash);
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

    let transaction = accepted_tx_by_someone(&time_source);
    let hash = transaction.hash();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, transaction);

    let (snapshot, lease) = queue
        .bounded_pending_snapshot(&state.view(), nonzero!(1_usize))
        .expect("global selection remains healthy");
    assert_eq!(
        snapshot
            .iter()
            .map(AcceptedTransaction::hash)
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
    assert_eq!(reserved[0].key().signed_transaction_hash, hash);
}

#[test]
fn lane_reservation_group_diagnostics_follow_durable_commit_forget_boundary() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let mut state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    let transaction = accepted_tx_by_someone(&time_source);
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

    assert_eq!(
        queue
            .commit_lane_reservation(&key)
            .expect("commit diagnostic reservation"),
        LaneQueueReservationOutcome::Finalized
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
        accepted_tx_by_someone(&time_source),
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
        .commit_lane_reservation(&key)
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

fn lane_reservation_release_barrier(
    keys: Vec<LaneQueueReservationKeyV2>,
    retirement_seed: &[u8],
) -> LaneQueueReservationReleaseBarrierV3 {
    let first = keys.first().expect("release barrier needs a reservation");
    LaneQueueReservationReleaseBarrierV3 {
        version: LaneQueueReservationReleaseBarrierV3::VERSION,
        chain_id_hash: Hash::new(b"queue-release-chain"),
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
            accepted_tx_by_someone(&time_source),
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
    expected_live.sort_by_key(LaneQueueReservationKeyV2::digest);
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
fn ordered_release_restarts_from_prepared_and_completed_boundaries() {
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
                    accepted_tx_by_someone(&time_source),
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
                    .map(|key| reservations.live_by_hash[&key.signed_transaction_hash].clone())
                    .collect();
                let completion = LaneQueueReservationReleaseCompletionV5 {
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
        queue
            .finalize_plan_journal_startup_recovery()
            .expect("finish release recovery after atomic payload replay");
        if crash_after_completion {
            assert!(
                queue.lane_reservation_release_barriers().is_empty(),
                "completed release must restore FIFO and forget itself after payload replay"
            );
        } else {
            assert_eq!(queue.queued_len(), 0);
            assert_eq!(
                queue
                    .finalize_lane_reservation_release_barrier(&barrier)
                    .expect("resume prepared release after restart"),
                2
            );
        }
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
        .map(|_| accepted_tx_by_someone(&time_source))
        .collect();
    let hashes: Vec<_> = txs.iter().map(AcceptedTransaction::hash).collect();
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
            .map(|tx| tx.as_accepted().hash())
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
            .map(|tx| tx.as_ref().hash())
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
            .commit_lane_reservation(&second_key)
            .expect("commit reservation"),
        LaneQueueReservationOutcome::Finalized
    );
    assert_eq!(
        queue
            .commit_lane_reservation(&second_key)
            .expect("repeat exact commit"),
        LaneQueueReservationOutcome::AlreadyFinalized
    );
    let mut released = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(1_usize), &mut released);
    assert_eq!(released[0].as_ref().hash(), hashes[0]);
}

#[test]
fn lane_reservation_excludes_locked_global_entrypoints_and_releases_in_payload_order() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);

    let txs = (0..4)
        .map(|_| accepted_tx_by_someone(&time_source))
        .collect::<Vec<_>>();
    let hashes = txs
        .iter()
        .map(AcceptedTransaction::hash)
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
            .map(|transaction| transaction.as_accepted().hash())
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
            .map(|transaction| transaction.as_ref().hash())
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
        .map(|_| accepted_tx_by_someone(&time_source))
        .collect::<Vec<_>>();
    let hashes = txs
        .iter()
        .map(AcceptedTransaction::hash)
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
            .map(|transaction| transaction.as_ref().hash())
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
        .map(|_| accepted_tx_by_someone(&time_source))
        .collect::<Vec<_>>();
    let hashes = txs
        .iter()
        .map(AcceptedTransaction::hash)
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
            .map(|transaction| transaction.as_ref().hash())
            .collect::<Vec<_>>(),
        hashes
    );
    drop(selected);
    assert_eq!(queue.active_len(), hashes.len());
    assert_eq!(queue.queued_len(), hashes.len());
    assert_eq!(queue.durable_plan_claims.len(), hashes.len());
}

#[test]
fn reservation_restart_release_restores_exact_global_fifo() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let plan_path = dir.path().join("fifo-plans.norito");
    let reservation_path = dir.path().join("fifo-reservations.norito");
    let txs = (0..5)
        .map(|_| accepted_tx_by_someone(&time_source))
        .collect::<Vec<_>>();
    let hashes = txs
        .iter()
        .map(AcceptedTransaction::hash)
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
    assert_eq!(
        queue
            .release_lane_reservations_in_order(&keys)
            .expect("release replayed A/B"),
        2
    );
    let mut selected = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(5_usize), &mut selected);
    assert_eq!(
        selected
            .iter()
            .map(|transaction| transaction.as_ref().hash())
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
        .map(|_| accepted_tx_by_someone(&time_source))
        .collect::<Vec<_>>();
    let hashes = txs
        .iter()
        .map(AcceptedTransaction::hash)
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
        assert_eq!(reserved[0].as_accepted().hash(), hashes[1]);
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
            .map(|transaction| transaction.as_ref().hash())
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
        .map(|_| accepted_tx_by_someone(&time_source))
        .collect::<Vec<_>>();
    let hashes = txs
        .iter()
        .map(AcceptedTransaction::hash)
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
                lane_reservation_scope(&state, b"scan-owner", b"scan-proposal"),
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
            lane_reservation_scope(&state, b"byte-owner", b"byte-proposal"),
            limits(3, 10, u64::MAX),
            &BTreeSet::new(),
            LaneQueueReservationRoutingMode::AnyCoordinatorPlan,
        )
        .expect("bounded byte reservation");
    assert_eq!(byte_reserved.len(), 1);
    assert_eq!(byte_reserved[0].as_accepted().hash(), hashes[0]);
    queue
        .release_lane_reservation(byte_reserved[0].key())
        .expect("release byte-bounded owner");

    queue.tx_gas_cost.insert(hashes[0], 5);
    queue.tx_gas_cost.insert(hashes[1], 6);
    let gas_reserved = queue
        .reserve_transactions_for_lane_bounded(
            &state,
            lane_reservation_scope(&state, b"gas-owner", b"gas-proposal"),
            limits(3, u64::MAX, 5),
            &BTreeSet::new(),
            LaneQueueReservationRoutingMode::AnyCoordinatorPlan,
        )
        .expect("bounded gas reservation");
    assert_eq!(gas_reserved.len(), 1);
    assert_eq!(gas_reserved[0].as_accepted().hash(), hashes[0]);
}

#[test]
fn committing_reservation_owned_transaction_does_not_create_fifo_tombstone() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let transaction = accepted_unique_entrypoint_tx_by_someone(&time_source);
    let hash = transaction.hash();
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

#[test]
fn lane_reservation_drains_committed_physical_fifo_tombstone() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    let dir = tempdir().expect("tempdir");
    install_globally_certified_test_reservation_journals(&queue, &dir);
    let committed = accepted_unique_entrypoint_tx_by_someone(&time_source);
    let committed_hash = committed.hash();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, committed);
    let candidate = accepted_unique_entrypoint_tx_by_someone(&time_source);
    let candidate_hash = candidate.hash();
    push_globally_bound_lane_reservation_candidate(&queue, &state, &dir, candidate);

    assert_eq!(queue.remove_committed_hashes([committed_hash], None), 1);
    assert!(queue.removed_hashes.contains_key(&committed_hash));
    assert!(!queue.txs.contains_key(&committed_hash));
    assert!(!queue.fifo_order_by_hash.contains_key(&committed_hash));
    let unrelated_non_fifo_fence = accepted_unique_entrypoint_tx_by_someone(&time_source).hash();
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
    assert_eq!(reserved[0].as_accepted().hash(), candidate_hash);
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
    let transaction = accepted_unique_entrypoint_tx_by_someone(&time_source);
    let hash = transaction.hash();
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
        accepted_tx_by_someone(&time_source),
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
        reservations
            .live_by_hash
            .remove(&key.signed_transaction_hash);
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
        let tx = accepted_tx_by_someone(&time_source);
        let hash = tx.hash();
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

#[test]
fn ambiguous_terminal_reservation_appends_fail_closed_for_diagnostics_and_drain() {
    #[derive(Clone, Copy, Debug)]
    enum TerminalAppend {
        Release,
        OrderedRelease,
        PrepareRelease,
        CompleteRelease,
        Commit,
        Prune,
    }

    for terminal in [
        TerminalAppend::Release,
        TerminalAppend::OrderedRelease,
        TerminalAppend::PrepareRelease,
        TerminalAppend::CompleteRelease,
        TerminalAppend::Commit,
        TerminalAppend::Prune,
    ] {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let state = lane_reservation_test_state();
        let dir = tempdir().expect("tempdir");
        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        install_test_reservation_journal(&queue, &dir);
        for _ in 0..2 {
            push_globally_bound_lane_reservation_candidate(
                &queue,
                &state,
                &dir,
                accepted_tx_by_someone(&time_source),
            );
        }
        let scope = lane_reservation_scope(
            &state,
            format!("terminal-fault-owner-{terminal:?}").as_bytes(),
            format!("terminal-fault-proposal-{terminal:?}").as_bytes(),
        );
        let reserved = queue
            .reserve_transactions_for_lane(&state, scope, nonzero!(2_usize))
            .expect("reserve terminal-fault batch");
        let keys = reserved.iter().map(|tx| *tx.key()).collect::<Vec<_>>();
        let barrier = lane_reservation_release_barrier(
            keys.clone(),
            format!("terminal-fault-retirement-{terminal:?}").as_bytes(),
        );
        let unrelated_lane = LaneId::new(93);
        let unrelated_dataspace = DataSpaceId::new(93);
        let unrelated_incarnation =
            Hash::new(format!("terminal-fault-unrelated-{terminal:?}").as_bytes());
        assert!(
            !queue.lane_has_pending_work(
                unrelated_lane,
                unrelated_dataspace,
                unrelated_incarnation,
            ),
            "{terminal:?}: healthy unrelated route must initially be drainable"
        );
        let pressure_rx = queue.backpressure_handle().subscribe();
        assert!(
            !pressure_rx.borrow().is_saturated(),
            "{terminal:?}: pressure must be healthy before fault injection"
        );

        let inject_ambiguous_append = || {
            queue
                .lane_reservation_journal
                .lock()
                .as_mut()
                .expect("installed reservation journal")
                .inject_next_append_fault(ReservationJournalAppendFault::SyncAfterFullWrite);
        };
        let result = match terminal {
            TerminalAppend::Release => {
                inject_ambiguous_append();
                queue.release_lane_reservation(&keys[0]).map(|_| ())
            }
            TerminalAppend::OrderedRelease => {
                inject_ambiguous_append();
                queue.release_lane_reservations_in_order(&keys).map(|_| ())
            }
            TerminalAppend::PrepareRelease => {
                inject_ambiguous_append();
                queue
                    .prepare_lane_reservation_release_barrier(&barrier)
                    .map(|_| ())
            }
            TerminalAppend::CompleteRelease => {
                queue
                    .prepare_lane_reservation_release_barrier(&barrier)
                    .expect("prepare completion-fault release");
                inject_ambiguous_append();
                queue
                    .finalize_lane_reservation_release_barrier(&barrier)
                    .map(|_| ())
            }
            TerminalAppend::Commit => {
                inject_ambiguous_append();
                queue.commit_lane_reservation(&keys[0]).map(|_| ())
            }
            TerminalAppend::Prune => {
                inject_ambiguous_append();
                queue
                    .prune_lane_reservations(scope.lane_id, scope.lane_incarnation)
                    .map(|_| ())
            }
        };

        assert!(
            matches!(result, Err(LaneQueueReservationError::Journal(_))),
            "{terminal:?}: ambiguous terminal append must report a journal error"
        );
        assert!(
            queue.lane_reservation_durability_faulted(),
            "{terminal:?}: terminal ambiguity must latch the process fault"
        );
        assert!(
            pressure_rx.borrow().is_saturated(),
            "{terminal:?}: terminal ambiguity must publish saturated pressure to existing observers"
        );
        assert!(
            !queue.lane_reservation_group_is_finalized_for_diagnostics(&keys),
            "{terminal:?}: diagnostics must fail closed after terminal ambiguity"
        );
        assert!(
            queue
                .lane_has_pending_work(unrelated_lane, unrelated_dataspace, unrelated_incarnation,),
            "{terminal:?}: terminal ambiguity must block even unrelated lane drain"
        );
    }

    #[derive(Clone, Copy, Debug)]
    enum StartupTerminalAppend {
        ForgetCommitDuringStartup,
        ForgetReleaseDuringStartup,
    }

    for terminal in [
        StartupTerminalAppend::ForgetCommitDuringStartup,
        StartupTerminalAppend::ForgetReleaseDuringStartup,
    ] {
        let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
        let state = lane_reservation_test_state();
        let dir = tempdir().expect("tempdir");
        let reservation_path = dir
            .path()
            .join(format!("startup-terminal-fault-{terminal:?}.norito"));
        let plan_path = test_lane_reservation_plan_path(&dir);
        let (keys, barrier) = {
            let queue = Arc::new(Queue::test(config_factory(), &time_source));
            queue
                .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
                .expect("install startup-terminal reservation journal");
            let transaction_count = match terminal {
                StartupTerminalAppend::ForgetCommitDuringStartup => 1,
                StartupTerminalAppend::ForgetReleaseDuringStartup => 2,
            };
            for _ in 0..transaction_count {
                push_globally_bound_lane_reservation_candidate(
                    &queue,
                    &state,
                    &dir,
                    accepted_tx_by_someone(&time_source),
                );
            }
            let scope = lane_reservation_scope(
                &state,
                format!("startup-terminal-owner-{terminal:?}").as_bytes(),
                format!("startup-terminal-proposal-{terminal:?}").as_bytes(),
            );
            let reserved = queue
                .reserve_transactions_for_lane(
                    &state,
                    scope,
                    NonZeroUsize::new(transaction_count)
                        .expect("startup terminal fixture is nonempty"),
                )
                .expect("reserve startup-terminal batch");
            let keys = reserved.iter().map(|tx| *tx.key()).collect::<Vec<_>>();
            let barrier = lane_reservation_release_barrier(
                keys.clone(),
                format!("startup-terminal-retirement-{terminal:?}").as_bytes(),
            );
            match terminal {
                StartupTerminalAppend::ForgetCommitDuringStartup => {
                    queue
                        .lane_reservation_journal
                        .lock()
                        .as_mut()
                        .expect("installed reservation journal")
                        .commit(keys[0])
                        .expect("persist commit before startup-boundary crash");
                }
                StartupTerminalAppend::ForgetReleaseDuringStartup => {
                    queue
                        .prepare_lane_reservation_release_barrier(&barrier)
                        .expect("persist prepared startup release");
                    let store = queue.lane_reservations.lock();
                    let ordered_records = barrier
                        .ordered_keys
                        .iter()
                        .map(|key| store.live_by_hash[&key.signed_transaction_hash].clone())
                        .collect();
                    drop(store);
                    queue
                        .lane_reservation_journal
                        .lock()
                        .as_mut()
                        .expect("installed reservation journal")
                        .complete_release(LaneQueueReservationReleaseCompletionV5 {
                            version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
                            barrier: barrier.clone(),
                            ordered_records,
                        })
                        .expect("persist release completion before startup-boundary crash");
                }
            }
            (keys, barrier)
        };

        let queue = Arc::new(Queue::test(config_factory(), &time_source));
        let replay = queue
            .install_lane_reservation_journal(&reservation_path, 1024 * 1024)
            .expect("restore startup-terminal boundary before its plan journal");
        match terminal {
            StartupTerminalAppend::ForgetCommitDuringStartup => {
                assert_eq!(keys.len(), 1);
                assert_eq!(replay.restored, 0);
                assert_eq!(replay.commit_barriers, 1);
                assert_eq!(replay.completed_releases, 0);
                assert!(queue.live_lane_reservations().is_empty());
                assert_eq!(
                    queue.lane_reservation_commit_barriers(),
                    vec![keys[0]],
                    "restart must reconstruct the exact commit identity that still needs a plan tombstone"
                );
            }
            StartupTerminalAppend::ForgetReleaseDuringStartup => {
                assert_eq!(keys.len(), 2);
                assert_eq!(replay.restored, 0);
                assert_eq!(replay.commit_barriers, 0);
                assert_eq!(replay.completed_releases, 1);
                assert!(queue.live_lane_reservations().is_empty());
                assert_eq!(
                    queue.lane_reservation_release_barriers(),
                    vec![barrier.clone()],
                    "restart must reconstruct the exact completed release barrier"
                );
                let store = queue.lane_reservations.lock();
                assert_eq!(store.completed_releases.len(), 1);
                assert_eq!(store.completed_releases[0].barrier, barrier);
                assert_eq!(
                    store.completed_releases[0]
                        .ordered_records
                        .iter()
                        .map(|record| record.key)
                        .collect::<Vec<_>>(),
                    keys,
                    "completed release replay must retain the exact ordered reservation records"
                );
            }
        }
        let unrelated_lane = LaneId::new(95);
        let unrelated_dataspace = DataSpaceId::new(95);
        let unrelated_incarnation =
            Hash::new(format!("startup-terminal-unrelated-{terminal:?}").as_bytes());
        assert!(!queue.lane_has_pending_work(
            unrelated_lane,
            unrelated_dataspace,
            unrelated_incarnation,
        ));
        let pressure_rx = queue.backpressure_handle().subscribe();
        queue
            .lane_reservation_journal
            .lock()
            .as_mut()
            .expect("restored reservation journal")
            .inject_next_append_fault(ReservationJournalAppendFault::SyncAfterFullWrite);

        let error = match terminal {
            StartupTerminalAppend::ForgetCommitDuringStartup => queue
                .install_plan_journal(&plan_path, 1024 * 1024, true)
                .expect_err("ambiguous startup ForgetCommit must fail plan-journal install"),
            StartupTerminalAppend::ForgetReleaseDuringStartup => {
                queue
                    .install_plan_journal(&plan_path, 1024 * 1024, true)
                    .expect("release recovery installs the exact production plan journal");
                queue
                    .replay_plan_journal(&state)
                    .expect("payload replay commits before release reconciliation");
                queue.finalize_plan_journal_startup_recovery().expect_err(
                    "ambiguous startup ForgetRelease must fail separate startup recovery",
                )
            }
        };
        assert!(
            error.to_string().contains("lane queue reservation journal"),
            "{terminal:?}: startup error must identify the ambiguous reservation boundary: {error}"
        );
        assert!(
            queue.lane_reservation_durability_faulted(),
            "{terminal:?}: ambiguous startup Forget must latch the process fault"
        );
        assert!(
            pressure_rx.borrow().is_saturated(),
            "{terminal:?}: ambiguous startup Forget must publish saturated pressure"
        );
        assert!(
            !queue.lane_reservation_group_is_finalized_for_diagnostics(&keys),
            "{terminal:?}: diagnostics must fail closed after startup Forget ambiguity"
        );
        assert!(
            queue
                .lane_has_pending_work(unrelated_lane, unrelated_dataspace, unrelated_incarnation,),
            "{terminal:?}: startup Forget ambiguity must block unrelated lane drain"
        );
        match terminal {
            StartupTerminalAppend::ForgetCommitDuringStartup => assert_eq!(
                queue.lane_reservation_commit_barriers(),
                vec![keys[0]],
                "ambiguous ForgetCommit must retain the exact affected identity"
            ),
            StartupTerminalAppend::ForgetReleaseDuringStartup => {
                let store = queue.lane_reservations.lock();
                assert_eq!(store.completed_releases.len(), 1);
                assert_eq!(store.completed_releases[0].barrier, barrier);
                assert_eq!(
                    store.completed_releases[0]
                        .ordered_records
                        .iter()
                        .map(|record| record.key)
                        .collect::<Vec<_>>(),
                    keys,
                    "ambiguous ForgetRelease must retain the exact affected ordered group"
                );
            }
        }
    }
}

#[test]
fn ambiguous_reservation_compaction_fails_closed_after_terminal_application() {
    let (_time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let state = lane_reservation_test_state();
    let dir = tempdir().expect("tempdir");
    let path = dir.path().join("ambiguous-compaction.norito");
    let queue = Arc::new(Queue::test(config_factory(), &time_source));
    queue
        .install_plan_journal(
            dir.path().join("ambiguous-compaction-plans.norito"),
            1024 * 1024,
            true,
        )
        .expect("install globally certified reservation plan journal");
    queue
        .install_lane_reservation_journal(&path, 1)
        .expect("install aggressively compacting reservation journal");
    push_globally_bound_lane_reservation_candidate(
        &queue,
        &state,
        &dir,
        accepted_tx_by_someone(&time_source),
    );
    let key = *queue
        .reserve_transactions_for_lane(
            &state,
            lane_reservation_scope(
                &state,
                b"compaction-fault-owner",
                b"compaction-fault-proposal",
            ),
            nonzero!(1_usize),
        )
        .expect("reserve compaction-fault transaction")[0]
        .key();
    let unrelated_lane = LaneId::new(94);
    let unrelated_dataspace = DataSpaceId::new(94);
    let unrelated_incarnation = Hash::new(b"compaction-fault-unrelated-incarnation");
    assert!(!queue.lane_has_pending_work(
        unrelated_lane,
        unrelated_dataspace,
        unrelated_incarnation,
    ));
    let pressure_rx = queue.backpressure_handle().subscribe();
    assert!(
        !pressure_rx.borrow().is_saturated(),
        "pressure must be healthy before compaction fault injection"
    );
    queue
        .lane_reservation_journal
        .lock()
        .as_mut()
        .expect("installed reservation journal")
        .inject_next_compaction_fault(
            ReservationJournalCompactionFault::AfterRenameBeforeParentSync,
        );

    assert_eq!(
        queue
            .release_lane_reservation(&key)
            .expect("terminal release remains durably applied before compaction"),
        LaneQueueReservationOutcome::Finalized
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert!(queue.lane_reservation_commit_barriers().is_empty());
    assert!(queue.lane_reservation_release_barriers().is_empty());
    assert!(
        queue.lane_reservation_durability_faulted(),
        "rename-before-parent-sync ambiguity must latch the process fault"
    );
    assert!(
        pressure_rx.borrow().is_saturated(),
        "compaction ambiguity must publish saturated pressure to existing observers"
    );
    assert!(
        !queue.lane_reservation_group_is_finalized_for_diagnostics(&[key]),
        "a fully applied terminal transition must still fail diagnostics closed while its compacted replacement is ambiguous"
    );
    assert!(
        queue.lane_has_pending_work(unrelated_lane, unrelated_dataspace, unrelated_incarnation,),
        "compaction ambiguity must block even unrelated lane drain"
    );
    let mut selected = Vec::new();
    queue.get_transactions_for_block_with_state(&state, nonzero!(1_usize), &mut selected);
    assert!(
        selected.is_empty(),
        "the FIFO-restored transaction must remain nonselectable until restart recovery"
    );
}
