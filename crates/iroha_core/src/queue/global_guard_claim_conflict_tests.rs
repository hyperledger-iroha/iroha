#[test]
fn globally_bound_guard_drop_preserves_claim_for_later_conflict_rejection() {
    let poison_expired_global_identity = |fixture: &GloballyBoundGuardFixture| {
        fixture
            .time_handle
            .advance(fixture.transaction_time_to_live + Duration::from_millis(1));
        let hash = fixture.transaction.hash();
        let mut claim = fixture
            .queue
            .durable_plan_claims
            .get_mut(&hash)
            .expect("globally bound fixture durable claim");
        claim
            .global_admission_identity
            .as_mut()
            .expect("globally bound fixture identity")
            .version = 0;
    };
    let assert_faulted_owner_retained = |fixture: &GloballyBoundGuardFixture| {
        let hash = fixture.transaction.hash();
        assert!(fixture.queue.accepted_work_validation_faulted());
        assert_eq!(fixture.queue.active_len(), 1);
        assert_eq!(fixture.queue.queued_len(), 1);
        assert!(fixture.queue.txs.contains_key(&hash));
        assert!(fixture.queue.routing_decisions.contains_key(&hash));
        assert!(fixture.queue.routing_plans.contains_key(&hash));
        assert!(
            fixture
                .queue
                .durable_plan_claims
                .get(&hash)
                .is_some_and(|claim| claim.global_admission_identity.is_some())
        );
        {
            let _queue_guard = fixture.queue.push_remove_lock.lock();
            assert_eq!(fixture.queue.fifo_snapshot_locked(), vec![hash]);
        }
        assert!(fixture.queue.global_selection_owners.lock().is_empty());
        assert_eq!(fixture.queue.inflight_guards.load(Ordering::Relaxed), 0);
        fixture.assert_live_journal_claim();
    };

    let all_transactions_fixture = globally_bound_guard_fixture();
    poison_expired_global_identity(&all_transactions_fixture);
    let state_view = all_transactions_fixture.state.view();
    assert!(
        all_transactions_fixture
            .queue
            .all_transactions(&state_view)
            .next()
            .is_none(),
        "malformed expired global ownership must fail closed without retaining a lazy lock"
    );
    drop(state_view);
    assert_faulted_owner_retained(&all_transactions_fixture);

    let bounded_snapshot_fixture = globally_bound_guard_fixture();
    poison_expired_global_identity(&bounded_snapshot_fixture);
    let state_view = bounded_snapshot_fixture.state.view();
    assert!(
        bounded_snapshot_fixture
            .queue
            .bounded_pending_snapshot(&state_view, nonzero!(1_usize))
            .is_none(),
        "malformed expired global ownership must stop bounded selection"
    );
    drop(state_view);
    assert!(
        bounded_snapshot_fixture
            .queue
            .push_remove_lock
            .try_lock()
            .is_some(),
        "bounded failure publication must release the Queue mutation lock"
    );
    assert!(
        bounded_snapshot_fixture
            .queue
            .queued_age_ring
            .try_lock()
            .is_some(),
        "bounded failure publication must release the Queue age lock"
    );
    assert!(
        bounded_snapshot_fixture
            .queue
            .global_selection_owners
            .try_lock()
            .is_some(),
        "bounded failure publication must release the selection-owner lock"
    );
    assert_faulted_owner_retained(&bounded_snapshot_fixture);

    let revalidation_fixture = globally_bound_guard_fixture();
    poison_expired_global_identity(&revalidation_fixture);
    let revalidation_hash = revalidation_fixture.transaction.hash();
    let checked_transaction = revalidation_fixture
        .queue
        .txs
        .get(&revalidation_hash)
        .map(|entry| Arc::clone(entry.value()))
        .expect("revalidation fixture checked transaction");
    let state_view = revalidation_fixture.state.view();
    let active_transitions = revalidation_fixture.queue.durability_transitions.lock();
    assert_eq!(
        revalidation_fixture
            .queue
            .pending_status_with_stable_durability_owner(
                checked_transaction.as_ref(),
                &state_view,
            ),
        Err("QueuePlan global-admission identity version is unsupported".to_owned())
    );
    assert!(
        !revalidation_fixture
            .queue
            .accepted_work_validation_faulted(),
        "pure pending validation must not publish a fault while the transition lock is held"
    );
    drop(active_transitions);
    drop(state_view);
    let router = revalidation_fixture.queue.router.read().clone();
    let lane_catalog = revalidation_fixture.queue.lane_catalog.read().clone();
    let dataspace_catalog = revalidation_fixture.queue.dataspace_catalog.read().clone();
    revalidation_fixture
        .queue
        .revalidate_pending_transactions_with_state(
            &router,
            &revalidation_fixture.state,
            &lane_catalog,
            &dataspace_catalog,
            true,
        );
    assert!(
        revalidation_fixture
            .queue
            .durability_transitions
            .try_lock()
            .is_some(),
        "revalidation fault publication must release the transition-index lock"
    );
    assert_faulted_owner_retained(&revalidation_fixture);

    let fixture = globally_bound_guard_fixture();
    let hash = fixture.transaction.hash();

    let guard = fixture.pop_guard();
    drop(guard);
    fixture.assert_restored_fifo_owner();

    let routing_plan = fixture
        .binding
        .routing_plan()
        .expect("fixture binding routing plan");
    let conflicting_binding = crate::torii_proxy::QueuePlanAdmissionBindingV2::new(
        fixture.state.network_id_ref(),
        fixture.transaction.entrypoint(),
        &routing_plan,
        fixture.binding.admission_context.clone(),
        fixture.binding.enqueue_timestamp_ms.saturating_add(1),
    )
    .expect("build coherent conflicting global guard owner");
    assert_eq!(
        conflicting_binding.entrypoint_hash,
        fixture.binding.entrypoint_hash
    );
    assert_ne!(
        conflicting_binding.canonical_hash(),
        fixture.binding.canonical_hash()
    );
    install_queue_plan_registry_value_for_test(&fixture.state, &conflicting_binding);
    assert_eq!(
        fixture
            .state
            .queue_plan_admission_binding_registry_match(&fixture.binding)
            .expect("read conflicting global guard registry"),
        QueuePlanAdmissionRegistryMatch::Conflict
    );
    fixture
        .time_handle
        .advance(fixture.transaction_time_to_live + Duration::from_millis(1));

    let mut expired = Vec::new();
    assert!(
        fixture
            .queue
            .pop_from_queue(&fixture.state.view(), &mut expired)
            .is_none(),
        "the conflicting globally admitted owner must be rejected, not selected"
    );
    assert!(expired.is_empty());
    assert!(
        fixture.queue.removed_hashes.contains_key(&hash),
        "exact conflict rejection must leave a stale-FIFO removal marker"
    );
    fixture.assert_terminally_removed();
}
