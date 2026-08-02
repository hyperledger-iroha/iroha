// Lane-reservation durability, FIFO, and exact-release regression tests.
//
// Included by `queue::tests` so source-bound libtest names remain stable.

fn checked_startup_reconciliation_receipt(
    queue: &Queue,
) -> LaneReservationStartupReconciliationReceipt {
    let snapshot = queue
        .lane_reservation_reconciliation_snapshot()
        .expect("capture exact startup reconciliation snapshot");
    queue
        .bind_lane_reservation_startup_reconciliation_receipt(&snapshot)
        .expect("bind checked snapshot replay receipt")
        .expect("queue snapshot remains unchanged while binding receipt")
}

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

fn payload_free_diagnostic_reservation_record(
    route: RoutingDecision,
    lane_incarnation: Hash,
    proposal_height: u64,
    lane_block_height: u64,
    fifo_ordinal: u64,
    group_seed: &[u8],
) -> LaneQueueReservationRecordV5 {
    let ordinal = fifo_ordinal.to_be_bytes();
    let entrypoint_hash = HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
        b"payload-free-diagnostic-entrypoint",
        &ordinal,
    ]));
    let routing_plan = RoutingPlan::single(route);
    LaneQueueReservationRecordV5 {
        version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION,
        key: LaneQueueReservationKeyV2 {
            version: LaneQueueReservationKeyV2::VERSION,
            signed_transaction_hash: HashOf::from_untyped_unchecked(Hash::from(entrypoint_hash)),
            entrypoint_hash,
            queue_plan_admission_binding_hash: Hash::new_from_chunks(&[
                b"payload-free-diagnostic-admission",
                &ordinal,
            ]),
            routing_plan_digest: routing_plan.digest(),
            coordinator_leg: routing_plan.coordinator_leg(),
            lane_id: route.lane_id,
            dataspace_id: route.dataspace_id,
            lane_incarnation,
            proposal_height,
            lane_block_height,
            lane_block_view: 0,
            reservation_owner_hash: Hash::new_from_chunks(&[
                b"payload-free-diagnostic-owner",
                group_seed,
            ]),
            proposal_identity_hash: Hash::new_from_chunks(&[
                b"payload-free-diagnostic-proposal",
                group_seed,
            ]),
        },
        enqueue_timestamp_ms: fifo_ordinal,
        fifo_order: LaneQueueFifoOrderV5::new(fifo_ordinal)
            .expect("diagnostic FIFO ordinal is positive"),
    }
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
