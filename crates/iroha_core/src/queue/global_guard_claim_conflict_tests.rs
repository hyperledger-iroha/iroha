#[test]
fn globally_bound_absent_registry_blocks_selection_and_preserves_exact_fifo() {
    let fixture = globally_bound_guard_fixture();
    let hash = fixture.transaction.hash();
    let follower_hash = fixture.follower_transaction.hash();
    fixture
        .queue
        .push_with_lane_with_state(fixture.follower_transaction.clone(), &fixture.state)
        .expect("enqueue FIFO follower");
    let mut expired = Vec::new();
    assert!(
        fixture
            .queue
            .pop_from_queue(&fixture.state.view(), &mut expired)
            .is_none()
            && expired.is_empty(),
        "a globally bound transaction must wait for its exact registry marker"
    );
    fixture.assert_restored_fifo_owner_with_order(&[hash, follower_hash]);
    let two = NonZeroUsize::new(2).expect("non-zero scan bound");
    let (pending, lease) = fixture
        .queue
        .bounded_pending_snapshot(&fixture.state.view(), two)
        .expect("absent marker is a healthy selection wait");
    assert!(pending.is_empty(), "the FIFO follower must not overtake");
    drop(lease);
    install_queue_plan_registry_value_for_test(&fixture.state, &fixture.binding);
    let (pending, lease) = fixture
        .queue
        .bounded_pending_snapshot(&fixture.state.view(), two)
        .expect("exact marker enables selection");
    assert_eq!(
        pending
            .iter()
            .map(AcceptedTransaction::hash)
            .collect::<Vec<_>>(),
        vec![hash, follower_hash]
    );
    drop(lease);
}

#[test]
fn globally_bound_gossip_waits_for_certificate_and_retains_it_after_exact_marker() {
    let fixture = globally_bound_guard_fixture();
    let hash = fixture.transaction.hash();
    let awaiting = fixture.queue.gossip_batch_with_state(1, &fixture.state);
    assert_eq!(awaiting.len(), 1);
    assert!(matches!(
        awaiting[0].queue_plan_admission,
        QueuePlanGossipAdmission::AwaitingCertificate
    ));

    let validator_key =
        iroha_crypto::KeyPair::from_seed(vec![0xB9; 32], iroha_crypto::Algorithm::Ed25519);
    let binding_hash = fixture.binding.canonical_hash();
    let preimage =
        crate::torii_proxy::queue_plan_admission_attestation_signing_bytes_v2(binding_hash, 0)
            .expect("build exact QueuePlan attestation preimage");
    let certificate = crate::torii_proxy::QueuePlanAdmissionCertificateV2 {
        version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V2,
        binding: fixture.binding.clone(),
        attestations: vec![crate::torii_proxy::QueuePlanAdmissionAttestationV2 {
            version: crate::torii_proxy::QUEUE_PLAN_ADMISSION_ATTESTATION_VERSION_V2,
            validator_index: 0,
            signature: iroha_crypto::Signature::try_new(validator_key.private_key(), &preimage)
                .expect("sign exact QueuePlan attestation"),
        }],
    };
    let certificate = norito::encode_canonical(&certificate).expect("encode QueuePlan certificate");
    fixture
        .state
        .kura()
        .persist_pending_queue_plan_admission_certificate(&certificate)
        .expect("persist QueuePlan gossip certificate");

    fixture.queue.requeue_gossip_hashes([hash]);
    let certified = fixture.queue.gossip_batch_with_state(1, &fixture.state);
    assert_eq!(certified.len(), 1);
    assert!(matches!(
        &certified[0].queue_plan_admission,
        QueuePlanGossipAdmission::Certified(bytes) if bytes.as_slice() == certificate
    ));

    install_queue_plan_registry_value_for_test(&fixture.state, &fixture.binding);
    fixture.queue.requeue_gossip_hashes([hash]);
    let exact_pending = fixture.queue.gossip_batch_with_state(1, &fixture.state);
    assert_eq!(exact_pending.len(), 1);
    assert!(matches!(
        &exact_pending[0].queue_plan_admission,
        QueuePlanGossipAdmission::Certified(bytes) if bytes.as_slice() == certificate
    ));

    fixture
        .time_handle
        .advance(fixture.transaction_time_to_live + Duration::from_millis(1));
    fixture.queue.requeue_gossip_hashes([hash]);
    let expired_exact_pending = fixture.queue.gossip_batch_with_state(1, &fixture.state);
    assert_eq!(expired_exact_pending.len(), 1);
    assert!(matches!(
        &expired_exact_pending[0].queue_plan_admission,
        QueuePlanGossipAdmission::Certified(bytes) if bytes.as_slice() == certificate
    ));
}

#[test]
fn exact_pending_body_handoff_preserves_historical_admission_after_ttl() {
    let dir = tempdir().expect("exact pending body-handoff directory");
    let journal_path = dir.path().join("exact_pending_body_handoff.norito");
    let mut state = State::new(
        world_with_test_domains(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    let mut nexus = state.nexus_snapshot();
    nexus.enabled = false;
    state
        .set_nexus(nexus)
        .expect("apply disabled Nexus state for exact pending handoff");
    install_single_validator_topology_for_queue_test(&state, 0xBA);
    let (time_handle, time_source) = TimeSource::new_mock(Duration::default());
    let router: Arc<dyn LaneRouter> = Arc::new(StaticRouter {
        lane: LaneId::SINGLE,
        dataspace: DataSpaceId::UNIVERSAL,
    });
    let config = config_factory();
    let transaction_time_to_live = config.transaction_time_to_live;
    let queue = Queue::test_with_router_for_routes(config, &time_source, router.clone(), &[]);
    queue
        .install_plan_journal(&journal_path, 1024 * 1024, true)
        .expect("install exact pending handoff journal");
    let transaction = accepted_queue_plan_tx_by_someone(&time_source);
    register_accepted_tx_authority_for_queue_test(&mut state, &transaction);
    let routing_plan = queue
        .route_plan_with_state(&transaction, &state)
        .expect("route exact pending handoff transaction");
    let admission_context = queue
        .plan_admission_context_with_state(&state, &routing_plan)
        .expect("capture exact pending historical context");
    let binding = crate::torii_proxy::QueuePlanAdmissionBindingV2::new(
        state.network_id_ref(),
        transaction.entrypoint(),
        &routing_plan,
        admission_context,
        queue.queue_plan_admission_timestamp_ms(),
    )
    .expect("build exact pending handoff binding");
    install_queue_plan_registry_value_for_test(&state, &binding);

    seed_committed_height_for_queue_test(&state, 1);
    time_handle.advance(transaction_time_to_live + Duration::from_millis(1));
    let authority = transaction.authority().clone();
    let mut world = state.world.block();
    world.accounts.remove(authority);
    world.commit();

    queue
        .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
            transaction.clone(),
            &state,
            routing_plan,
            &binding,
        )
        .expect("canonical pending proof must authorize historical body handoff");
    let durable = queue
        .durable_plan_admission_claim_with_state(&transaction, &state)
        .expect("read exact pending durable handoff")
        .expect("exact pending handoff must own one durable claim");
    let reconstructed =
        crate::torii_proxy::QueuePlanAdmissionBindingV2::try_from_durable_admission(&durable)
            .expect("reconstruct exact pending handoff binding");
    assert_eq!(reconstructed, binding);
    assert_eq!(queue.queued_len(), 1);

    drop(queue);
    let replay_queue =
        Queue::test_with_router_for_routes(config_factory(), &time_source, router, &[]);
    assert_eq!(
        replay_queue
            .install_plan_journal(&journal_path, 1024 * 1024, true)
            .expect("reopen exact pending handoff journal"),
        1
    );
    let replay = replay_queue
        .replay_plan_journal(&state)
        .expect("replay exact pending handoff after historical policy drift");
    assert_eq!(replay.records, 1);
    assert_eq!(replay.replayed, 1);
    let replayed_durable = replay_queue
        .durable_plan_admission_claim_with_state(&transaction, &state)
        .expect("read replayed exact pending durable handoff")
        .expect("replayed exact pending handoff must own one durable claim");
    let replayed_binding =
        crate::torii_proxy::QueuePlanAdmissionBindingV2::try_from_durable_admission(
            &replayed_durable,
        )
        .expect("reconstruct replayed exact pending handoff binding");
    assert_eq!(replayed_binding, binding);
}

#[test]
fn globally_bound_claim_validation_fails_closed_and_rejects_conflict() {
    let poison_expired_global_identity = |fixture: &GloballyBoundGuardFixture| {
        fixture
            .time_handle
            .advance(fixture.transaction_time_to_live + Duration::from_millis(1));
        let hash = fixture.transaction.hash_as_entrypoint();
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
        let hash = fixture.transaction.hash_as_entrypoint();
        assert!(fixture.queue.accepted_work_validation_faulted());
        assert_eq!(fixture.queue.active_len(), 1);
        assert_eq!(fixture.queue.queued_len(), 1);
        assert!(fixture.queue.txs.contains_key(&hash));
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
    let revalidation_hash = revalidation_fixture.transaction.hash_as_entrypoint();
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
<<<<<<< HEAD
    let hash = fixture.transaction.hash_as_entrypoint();
    let guard = fixture.pop_guard();
    drop(guard);
    fixture.assert_restored_fifo_owner();
=======
    let hash = fixture.transaction.hash();
>>>>>>> origin/optimizations
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
    let (pending, _lease) = fixture
        .queue
        .bounded_pending_snapshot(&fixture.state.view(), nonzero!(1_usize))
        .expect("conflicting marker is durably rejected without a selection fault");
    assert!(
        pending.is_empty(),
        "the conflicting globally admitted owner must be rejected, not selected"
    );
    assert!(
        fixture.queue.global_selection_owners.lock().is_empty(),
        "conflict rejection must not publish a candidate lease"
    );
    assert!(
        !fixture.queue.removed_hashes.contains_key(&hash),
        "bounded conflict rejection must synchronously remove its FIFO cell"
    );
    fixture.assert_terminally_removed();
}
