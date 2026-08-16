#[test]
fn recovered_decision_apply_completion_drop_is_fail_stop() {
    let output_guard = ConsensusOutputGuard::isolated();
    drop(RecoveredDecisionApplyCompletionDropGuardV1::new(
        Arc::clone(&output_guard),
    ));
    assert!(output_guard.restart_required());
}
#[test]
fn settled_recovered_decision_apply_completion_disarms_drop_guard() {
    let output_guard = ConsensusOutputGuard::isolated();
    let mut guard = RecoveredDecisionApplyCompletionDropGuardV1::new(Arc::clone(&output_guard));
    guard.disarm();
    drop(guard);
    assert!(!output_guard.restart_required());
}

#[test]
fn recovered_completion_capacity_census_selects_once_and_drops_fail_stop() {
    let (mut service, keys) = fixture();
    let context = service.context.clone();
    let output_guard = Arc::clone(&service.output_guard);
    let body_root = TempDir::new().expect("mixed Completion body root");
    let body_store =
        V2BodyStore::open(body_root.path(), context.clone()).expect("open mixed body store");
    let identity = body_store.instance_identity();
    let planner = install_lifecycle_planner_io_for_test(
        &mut service,
        context.clone(),
        Arc::clone(&output_guard),
        body_store,
        identity,
        2,
    );
    let apply = RecoveredDecisionApplyDispatchKeyV1::for_height_context_test(&context, 10, 0x41);
    let sign = RecoveredLifecycleSignDispatchKeyV1::for_height_context_test(
        &context,
        11,
        0x42,
        super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignClassV1::PhaseVote,
    );
    let census = service
        .capture_recovered_completion_capacity_census(vec![
            RecoveredCompletionCapacityProbeV1::Apply {
                ordinal: 10,
                key: apply,
            },
            RecoveredCompletionCapacityProbeV1::Sign {
                ordinal: 11,
                key: sign,
            },
        ])
        .expect("freeze one mixed worker/output census");
    assert_eq!(census.capacity_for_test(10), Some((true, 0)));
    assert_eq!(census.capacity_for_test(11), Some((true, 0)));
    let reservation = match census.select_sign(11) {
        Ok(reservation) => reservation,
        Err(_) => panic!("the frozen Sign row must transfer its exact reservation"),
    };
    reservation.cancel_uncommitted();
    assert!(!output_guard.restart_required());
    assert!(planner.command_rx.queue.lock().commands.is_empty());

    let fetch_round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let fetch_subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"mixed Completion Fetch block")),
        payload_hash: Hash::new(b"mixed Completion Fetch payload"),
    };
    let (authenticated, _) = production_authenticated_serve_request(
        &context,
        &keys,
        &keys[0],
        fetch_round,
        fetch_subject,
        wire::GlobalPhase::Prepare,
        &[0, 1, 2, 3],
    );
    let fetch_key =
        RecoveredDecisionFetchDispatchKeyV1::for_height_context_test(&context, 13, 0x44);
    let fetch_owner = RecoveredDecisionFetchRequestOwnerV1::for_test(
        fetch_key,
        service.active_tag,
        context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        authenticated,
    );
    let fetch_census = service
        .capture_recovered_completion_capacity_census(vec![
            RecoveredCompletionCapacityProbeV1::Fetch {
                ordinal: 13,
                owner: fetch_owner,
                executor_available: true,
            },
        ])
        .expect("freeze one exact recovered Fetch capacity owner");
    assert_eq!(fetch_census.capacity_for_test(13), Some((true, 0)));
    let (returned_owner, output) = match fetch_census.select_fetch(13) {
        Ok(selected) => selected,
        Err(_) => panic!("the exact Fetch row must transfer request and output ownership"),
    };
    assert_eq!(returned_owner.dispatch_key(), fetch_key);
    output.abort_before_claim();
    assert!(!output_guard.restart_required());

    planner.saturate_consensus_prefix(&service);
    let saturated = service
        .capture_recovered_completion_capacity_census(vec![
            RecoveredCompletionCapacityProbeV1::Apply {
                ordinal: 10,
                key: apply,
            },
            RecoveredCompletionCapacityProbeV1::Sign {
                ordinal: 11,
                key: sign,
            },
        ])
        .expect("a saturated cut remains an authenticated census");
    assert_eq!(saturated.capacity_for_test(10), Some((false, 4)));
    assert_eq!(saturated.capacity_for_test(11), Some((false, 4)));
    saturated.complete_without_selection();
    assert!(!output_guard.restart_required());
    planner.detach(&mut service);

    let (mut dropped_service, _keys) = fixture();
    let dropped_context = dropped_service.context.clone();
    let dropped_guard = Arc::clone(&dropped_service.output_guard);
    let dropped_root = TempDir::new().expect("dropped mixed Completion body root");
    let dropped_store = V2BodyStore::open(dropped_root.path(), dropped_context.clone())
        .expect("open dropped mixed body store");
    let dropped_identity = dropped_store.instance_identity();
    let dropped_planner = install_lifecycle_planner_io_for_test(
        &mut dropped_service,
        dropped_context.clone(),
        Arc::clone(&dropped_guard),
        dropped_store,
        dropped_identity,
        1,
    );
    let dropped_key =
        RecoveredDecisionApplyDispatchKeyV1::for_height_context_test(&dropped_context, 12, 0x43);
    drop(
        dropped_service
            .capture_recovered_completion_capacity_census(vec![
                RecoveredCompletionCapacityProbeV1::Apply {
                    ordinal: 12,
                    key: dropped_key,
                },
            ])
            .expect("arm one census before abandoning it"),
    );
    assert!(dropped_guard.restart_required());
    dropped_planner.detach(&mut dropped_service);
}

#[test]
fn recovered_decision_apply_source_stays_outside_generic_effect_ownership() {
    let apply_source = include_str!("../v2_apply.rs");
    let task_source = apply_source
        .split_once("pub(in crate::sumeragi) struct RecoveredDecisionApplyTaskV1")
        .expect("recovered Apply task remains declared")
        .1
        .split_once("pub(in crate::sumeragi) struct RecoveredDecisionApplyCompletionV1")
        .expect("recovered Apply completion follows its task")
        .0;
    for forbidden in [
        "EffectWorkId",
        "RuntimeEffectOwnership",
        "PendingApply",
        "DurableApplyCompletion",
    ] {
        assert!(
            !task_source.contains(forbidden),
            "recovered Apply task reintroduced generic owner {forbidden}"
        );
    }
    let recovered_execute = apply_source
        .split_once("fn execute_recovered_decision_apply(")
        .expect("dedicated recovered Apply executor remains present")
        .1
        .split_once("fn execute_exact_apply(")
        .expect("dedicated recovered Apply executor precedes the shared core")
        .0;
    assert!(
        recovered_execute
            .find("matches_height_context(context)")
            .is_some_and(|oracle| {
                recovered_execute
                    .find("self.execute_exact_apply(")
                    .is_some_and(|execute| oracle < execute)
            }),
        "recovered Apply must authenticate its exact context before storage execution"
    );
    let carrier_source =
        crate::sumeragi::v2_lifecycle_coordinator::reviewed_v2_adapter_source_for_test();
    assert_eq!(
        carrier_source
            .matches("RecoveredDecisionApplyTaskV1::from_registry_projection")
            .count(),
        1,
        "only the fixed recovered carrier projection may mint the worker task"
    );
}
#[test]
fn recovered_decision_apply_worker_source_keeps_a_separate_owner_corridor() {
    let source = include_str!("../v2_worker.rs");
    let generic_enqueue = source
        .split_once("fn try_send_as(")
        .expect("generic queue enqueue remains present")
        .1
        .split_once("fn cancel(")
        .expect("generic queue enqueue precedes cancellation")
        .0;
    assert!(generic_enqueue.contains("UnreservedRecoveredDecisionApply"));
    let reservation = source
        .split_once("impl RecoveredDecisionApplyCapacityReservationV1<'_>")
        .expect("dedicated recovered Apply reservation remains present")
        .1
        .split_once("impl Drop for RecoveredDecisionApplyCapacityReservationV1")
        .expect("reservation implementation precedes its fail-stop drop")
        .0;
    assert!(reservation.contains("authenticated_predecessor_debt"));
    assert!(reservation.contains("prepared.commit_for_worker()"));
    assert!(reservation.contains("V2IoCommand::RecoveredDecisionApply(task)"));
    let completion = source
        .split_once("struct RecoveredDecisionApplyWorkAckV1")
        .expect("dedicated recovered Apply acknowledgement remains present")
        .1
        .split_once("enum V2IoCompletion")
        .expect("dedicated acknowledgement precedes the completion enum")
        .0;
    assert!(completion.contains("acknowledge_recovered_decision_apply"));
    assert!(completion.contains("fn acknowledge_retry_publication"));
    assert!(completion.contains("fn retry_deferred("));
    assert!(completion.contains("retry_recovered_decision_apply(task)"));
    assert!(completion.contains("acknowledge_recovered_decision_apply_completion(self.key)"));
    let retry = source
        .split_once("fn retry_recovered_decision_apply<")
        .expect("dedicated recovered Apply retry remains present")
        .1
        .split_once("fn acknowledge_completion(")
        .expect("retry precedes ordinary acknowledgement")
        .0;
    let pending = retry.find("V2IoWorkState::CompletionPending").unwrap();
    let barrier = retry.find("exact_target_active").unwrap();
    let reserve = retry
        .find("try_reserve(V2IoAdmissionClass::Consensus)")
        .unwrap();
    let transfer = retry
        .find("transfer_recovered_decision_apply_completion(key)")
        .unwrap();
    let queued = retry.find("V2IoWorkState::Queued").unwrap();
    let publish = retry.find("task.into_command()").unwrap();
    assert!(
        pending < barrier
            && barrier < reserve
            && reserve < transfer
            && transfer < queued
            && queued < publish,
        "deferred retry must rejoin the Serve barrier and transfer completion ownership before publication"
    );
    let generic_take = source
        .split_once("fn take_io_completion(")
        .expect("ordinary completion take remains present")
        .1
        .split_once("fn take_recovered_decision_apply_completion(")
        .expect("owner-only completion take follows ordinary take")
        .0;
    assert!(generic_take.contains("V2IoCompletion::RecoveredDecisionApply(_)"));
    assert!(generic_take.contains("IoCompletionTake::retained_runtime()"));
    let runtime_capacity = source
        .split_once("const fn requires_runtime_capacity(&self) -> bool")
        .expect("completion runtime-capacity classifier remains present")
        .1
        .split_once("fn acknowledgement(&self)")
        .expect("completion acknowledgement follows capacity classification")
        .0;
    assert!(runtime_capacity.contains("Self::RecoveredDecisionApply(_)"));
    assert!(source.contains("fn drain_recovered_decision_apply_completion("));
    assert!(source.contains("fn authorizes_sidecar_owner("));
    assert!(!source.contains("_services: &'services mut ProductionV2Services"));
    assert!(!source.contains("complete_application(*guarded"));
}
#[test]
fn recovered_decision_apply_completion_accounting_is_stable_by_exact_key() {
    let admission = V2IoAdmission::new(2, 2).expect("construct bounded I/O admission");
    let key = RecoveredDecisionApplyDispatchKeyV1::for_test(7, 1);
    let same_ordinal_foreign = RecoveredDecisionApplyDispatchKeyV1::for_test(7, 2);
    admission.retain_completion(Instant::now(), false, None, None, None, None);
    admission.retain_completion(Instant::now(), true, Some(7), Some(key), None, None);
    admission.retain_completion(Instant::now(), false, Some(8), None, None, None);
    assert!(admission.recovered_decision_apply_completion_is_exact(key));
    assert!(
        !admission.recovered_decision_apply_completion_is_exact(same_ordinal_foreign),
        "an ordinal alone cannot authorize lifecycle completion ownership"
    );
    assert!(admission.transfer_recovered_decision_apply_completion(key));
    assert!(!admission.recovered_decision_apply_completion_is_exact(key));
    assert_eq!(
        admission
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .owned
            .len(),
        2,
        "keyed transfer must preserve unrelated completion positions"
    );
}
#[test]
fn recovered_decision_apply_retry_requeues_exact_key_and_preserves_foreign_completions() {
    let admission = Arc::new(V2IoAdmission::new(2, 2).expect("bounded retry admission"));
    let (command_tx, _command_rx) =
        v2_io_command_channel(admission.capacity(), 1, 1, 1, Arc::clone(&admission));
    let key = RecoveredDecisionApplyDispatchKeyV1::for_test(7, 1);
    let same_ordinal_foreign = RecoveredDecisionApplyDispatchKeyV1::for_test(7, 2);
    admission.retain_completion(Instant::now(), false, None, None, None, None);
    admission.retain_completion(Instant::now(), true, Some(7), Some(key), None, None);
    admission.retain_completion(
        Instant::now(),
        true,
        Some(7),
        Some(same_ordinal_foreign),
        None,
        None,
    );
    command_tx.queue.lock().recovered_decision_applies.insert(
        key,
        V2IoTrackedRecoveredDecisionApplyV1 {
            state: V2IoWorkState::CompletionPending,
        },
    );
    let unrelated_before = admission
        .completion_state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .owned
        .iter()
        .filter(|owner| owner.recovered_decision_apply != Some(key))
        .map(|owner| {
            (
                owner.retained_at,
                owner.service_debt,
                owner.requires_runtime_capacity,
                owner.runtime_lifecycle_ordinal,
                owner.recovered_decision_apply,
            )
        })
        .collect::<Vec<_>>();
    assert!(
        command_tx
            .queue
            .retry_recovered_decision_apply(RecoveredDecisionApplyRetryTaskFixtureV1(key))
            .is_ok(),
        "the exact retained completion must re-enter its dedicated queue"
    );
    let state = command_tx.queue.lock();
    assert_eq!(
        state
            .recovered_decision_applies
            .get(&key)
            .map(|work| work.state),
        Some(V2IoWorkState::Queued)
    );
    assert_eq!(state.commands.len(), 1);
    assert!(matches!(
        state.commands.front(),
        Some(V2IoCommand::RecoveredDecisionApplyFixture(queued)) if *queued == key
    ));
    drop(state);
    assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 1);
    assert!(!admission.recovered_decision_apply_completion_is_exact(key));
    assert!(admission.recovered_decision_apply_completion_is_exact(same_ordinal_foreign));
    let unrelated_after = admission
        .completion_state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .owned
        .iter()
        .map(|owner| {
            (
                owner.retained_at,
                owner.service_debt,
                owner.requires_runtime_capacity,
                owner.runtime_lifecycle_ordinal,
                owner.recovered_decision_apply,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(unrelated_after, unrelated_before);
}
#[test]
fn recovered_decision_apply_retry_unavailable_preserves_pending_owner_and_barrier() {
    {
        let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded retry admission"));
        let (command_tx, _command_rx) = v2_io_command_channel(1, 1, 1, 1, Arc::clone(&admission));
        command_tx
            .try_send(V2IoCommand::Shutdown)
            .expect("fill the sole physical queue position");
        let key = RecoveredDecisionApplyDispatchKeyV1::for_test(11, 3);
        admission.retain_completion(Instant::now(), true, Some(11), Some(key), None, None);
        command_tx.queue.lock().recovered_decision_applies.insert(
            key,
            V2IoTrackedRecoveredDecisionApplyV1 {
                state: V2IoWorkState::CompletionPending,
            },
        );
        let completion_before = admission
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .owned
            .iter()
            .map(|owner| {
                (
                    owner.retained_at,
                    owner.service_debt,
                    owner.requires_runtime_capacity,
                    owner.runtime_lifecycle_ordinal,
                    owner.recovered_decision_apply,
                )
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            command_tx
                .queue
                .retry_recovered_decision_apply(RecoveredDecisionApplyRetryTaskFixtureV1(key)),
            Err(RecoveredDecisionApplyRetryQueueErrorV1::Unavailable(
                RecoveredDecisionApplyRetryTaskFixtureV1(returned)
            )) if returned == key
        ));
        let state = command_tx.queue.lock();
        assert_eq!(
            state
                .recovered_decision_applies
                .get(&key)
                .map(|work| work.state),
            Some(V2IoWorkState::CompletionPending)
        );
        assert_eq!(state.commands.len(), 1);
        assert!(matches!(
            state.commands.front(),
            Some(V2IoCommand::Shutdown)
        ));
        drop(state);
        assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 1);
        assert_eq!(
            admission
                .completion_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .owned
                .iter()
                .map(|owner| {
                    (
                        owner.retained_at,
                        owner.service_debt,
                        owner.requires_runtime_capacity,
                        owner.runtime_lifecycle_ordinal,
                        owner.recovered_decision_apply,
                    )
                })
                .collect::<Vec<_>>(),
            completion_before
        );
    }
    let (service, keys) = fixture_with_block_payload();
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let request = authenticated_serve_request(
        &service.context,
        &keys[1],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Prepare,
    );
    let requester = request.request().requester.clone();
    let (command_tx, _command_rx, admission) = test_io_command_channel(2);
    let barrier = command_tx
        .prepare_serve(CertifiedServeOwnerKey::Roster(requester), request)
        .expect("reserve an unclaimed Serve placeholder")
        .lifecycle_id;
    let key = RecoveredDecisionApplyDispatchKeyV1::for_test(13, 4);
    admission.retain_completion(Instant::now(), true, Some(13), Some(key), None, None);
    command_tx.queue.lock().recovered_decision_applies.insert(
        key,
        V2IoTrackedRecoveredDecisionApplyV1 {
            state: V2IoWorkState::CompletionPending,
        },
    );
    let completion_before = admission
        .completion_state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .owned
        .iter()
        .map(|owner| {
            (
                owner.retained_at,
                owner.service_debt,
                owner.requires_runtime_capacity,
                owner.runtime_lifecycle_ordinal,
                owner.recovered_decision_apply,
            )
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        command_tx
            .queue
            .retry_recovered_decision_apply(RecoveredDecisionApplyRetryTaskFixtureV1(key)),
        Err(RecoveredDecisionApplyRetryQueueErrorV1::Unavailable(
            RecoveredDecisionApplyRetryTaskFixtureV1(returned)
        )) if returned == key
    ));
    let state = command_tx.queue.lock();
    assert_eq!(
        state
            .recovered_decision_applies
            .get(&key)
            .map(|work| work.state),
        Some(V2IoWorkState::CompletionPending)
    );
    assert_eq!(state.serve_barrier, Some(barrier));
    assert_eq!(
        state.serves.get(&barrier).map(|serve| serve.state),
        Some(V2IoServeState::Reserved)
    );
    assert!(!state.pending_serve_requests.contains_key(&barrier));
    assert_eq!(state.commands.len(), 1);
    assert!(matches!(
        state.commands.front(),
        Some(V2IoCommand::Serve { lifecycle_id, .. }) if *lifecycle_id == barrier
    ));
    drop(state);
    assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 1);
    assert_eq!(
        admission
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .owned
            .iter()
            .map(|owner| {
                (
                    owner.retained_at,
                    owner.service_debt,
                    owner.requires_runtime_capacity,
                    owner.runtime_lifecycle_ordinal,
                    owner.recovered_decision_apply,
                )
            })
            .collect::<Vec<_>>(),
        completion_before
    );
}
/// Exact test-only worker ownership retained behind a production service.
#[must_use = "the exact test I/O fixture must remain alive with its service"]
pub(in crate::sumeragi) struct LifecyclePlannerIoFixture {
    command_rx: V2IoCommandReceiver,
    _body_store: V2BodyStore,
}
impl LifecyclePlannerIoFixture {
    /// Count exact queued certified-Fetch persistence commands.
    pub(in crate::sumeragi) fn queued_certified_fetch_count(&self) -> usize {
        self.command_rx
            .queue
            .lock()
            .commands
            .iter()
            .filter(|command| matches!(command, V2IoCommand::PersistCertifiedFetchBody(_)))
            .count()
    }
    /// Fill the exact Consensus threshold with control predecessors.
    pub(in crate::sumeragi) fn saturate_consensus_prefix(&self, services: &ProductionV2Services) {
        let io = services
            .io
            .as_ref()
            .expect("manual lifecycle planner I/O remains installed");
        assert!(
            Arc::ptr_eq(&self.command_rx.queue, &io.command_tx.queue),
            "the service must retain this fixture's exact queue"
        );
        for _ in 0..io.admission.consensus_limit {
            io.command_tx
                .try_send(V2IoCommand::Shutdown)
                .expect("control reserve admits the bounded test predecessor");
        }
    }
    /// Release one exact synthetic predecessor through the real receiver.
    pub(in crate::sumeragi) fn release_one_predecessor(&self) {
        assert!(matches!(
            self.command_rx.try_recv(),
            Ok(V2IoCommand::Shutdown)
        ));
    }
    /// Release every synthetic control predecessor queued by saturation.
    pub(in crate::sumeragi) fn release_all_predecessors(&self) {
        loop {
            match self.command_rx.try_recv() {
                Ok(V2IoCommand::Shutdown) => {}
                Err(mpsc::TryRecvError::Empty) => break,
                Ok(_) => panic!("unexpected non-control saturated predecessor"),
                Err(mpsc::TryRecvError::Disconnected) => {
                    panic!("saturated predecessor queue disconnected")
                }
            }
        }
    }
    /// Replace only the manual service's output guard for identity tests.
    pub(in crate::sumeragi) fn install_output_guard_for_test(
        &self,
        services: &mut ProductionV2Services,
        output_guard: Arc<ConsensusOutputGuard>,
    ) {
        let io = services
            .io
            .as_ref()
            .expect("manual lifecycle planner I/O remains installed");
        assert!(
            Arc::ptr_eq(&self.command_rx.queue, &io.command_tx.queue),
            "the output guard must change only on this exact test service"
        );
        services.output_guard = output_guard;
    }
    /// Retire the manual queue without invoking worker shutdown semantics.
    pub(in crate::sumeragi) fn detach(self, services: &mut ProductionV2Services) {
        services.lifecycle_body_store_identity = None;
        drop(services.io.take());
        drop(self);
    }
}
/// Install the exact body store moved out of a lifecycle owner into one
/// bounded production-service queue for the owner transaction regression.
pub(in crate::sumeragi) fn install_lifecycle_planner_io_for_test(
    services: &mut ProductionV2Services,
    context: wire::HeightContext,
    output_guard: Arc<ConsensusOutputGuard>,
    body_store: V2BodyStore,
    identity: V2BodyStoreInstanceIdentity,
    class_capacity: usize,
) -> LifecyclePlannerIoFixture {
    let active_tag = services.active_tag;
    install_lifecycle_planner_io_for_validator_for_test(
        services,
        context,
        0,
        active_tag,
        output_guard,
        body_store,
        identity,
        class_capacity,
    )
}
/// Install a moved exact store for a chosen validator and reducer incarnation.
pub(in crate::sumeragi) fn install_lifecycle_planner_io_for_validator_for_test(
    services: &mut ProductionV2Services,
    context: wire::HeightContext,
    local_validator: wire::ValidatorIndex,
    active_tag: EventTag,
    output_guard: Arc<ConsensusOutputGuard>,
    body_store: V2BodyStore,
    identity: V2BodyStoreInstanceIdentity,
    class_capacity: usize,
) -> LifecyclePlannerIoFixture {
    assert!(class_capacity > 0, "test I/O capacity must be non-zero");
    assert!(
        body_store.instance_identity().same_instance(&identity),
        "the worker identity must come from the moved exact store"
    );
    assert_eq!(
        active_tag.height(),
        context.height,
        "the test service tag must belong to its immutable height context"
    );
    let local_index = usize::try_from(local_validator).expect("test validator index fits usize");
    let local_peer = context
        .roster
        .get(local_index)
        .expect("test validator belongs to the service context")
        .validator
        .clone();
    let admission = Arc::new(
        V2IoAdmission::new(class_capacity, class_capacity)
            .expect("bounded lifecycle planner I/O admission"),
    );
    let (command_tx, command_rx) = v2_io_command_channel(
        admission.capacity(),
        context.roster.len(),
        class_capacity,
        class_capacity,
        Arc::clone(&admission),
    );
    let (_completion_tx, completion_rx) = mpsc::sync_channel(admission.capacity());
    services.context = context.clone();
    services.local_peer = local_peer;
    services.local_validator = Some(local_validator);
    services.active_tag = active_tag;
    services.output_guard = output_guard;
    services.lifecycle_body_store_identity = Some(identity);
    services.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    LifecyclePlannerIoFixture {
        command_rx,
        _body_store: body_store,
    }
}
/// Install the exact private signer matching the test service's local peer.
pub(in crate::sumeragi) fn install_local_signer_for_test(
    services: &mut ProductionV2Services,
    key_pair: &KeyPair,
) {
    assert_eq!(
        services.local_peer.public_key(),
        key_pair.public_key(),
        "test signer must match the already bound local service peer"
    );
    services.key_pair = key_pair.clone();
}
#[test]
fn lifecycle_capacity_reservation_freezes_fifo_tail_and_rolls_back_under_lock() {
    let (service, _) = fixture();
    let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded I/O admission"));
    let capacity = admission.capacity();
    let (sender, receiver) = v2_io_command_channel(capacity, 1, 1, 1, Arc::clone(&admission));
    sender
        .try_send(V2IoCommand::Shutdown)
        .expect("queue one exact predecessor");
    let queue = Arc::clone(&sender.queue);
    let output_guard = ConsensusOutputGuard::isolated();
    let operation = output_guard
        .begin_fail_stop_operation()
        .expect("open fail-stop operation");
    let target = LifecycleIngressIoTargetSeal::for_test(
        &service.context,
        LifecycleIngressIoTargetKind::CertifiedFetchBodyPersistence,
        7,
    );
    let V2IoLifecycleCapacityCapture::Reserved(reservation) = queue
        .capture_lifecycle_capacity(operation, Arc::clone(&output_guard), target)
        .expect("reserve consensus target behind one predecessor")
    else {
        panic!("consensus target must own its exact FIFO position");
    };
    assert_eq!(reservation.predecessor_debt, 1);
    assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 2);
    let (done_tx, done_rx) = std::sync::mpsc::sync_channel(1);
    let producer = std::thread::spawn(move || {
        let result = sender.try_send(V2IoCommand::Shutdown);
        done_tx
            .send(result.is_ok())
            .expect("report producer result");
    });
    assert!(matches!(
        done_rx.recv_timeout(Duration::from_millis(20)),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    ));
    reservation.cancel_before_plan_for_test();
    assert!(done_rx.recv().expect("producer resumes after rollback"));
    producer.join().expect("producer thread exits");
    assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 2);
    assert!(matches!(receiver.try_recv(), Ok(V2IoCommand::Shutdown)));
    assert!(matches!(receiver.try_recv(), Ok(V2IoCommand::Shutdown)));
    assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
}
#[test]
fn lifecycle_capacity_unfinished_reservation_closes_output_fail_stop() {
    let (service, _) = fixture();
    let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded I/O admission"));
    let (sender, _receiver) =
        v2_io_command_channel(admission.capacity(), 1, 1, 1, Arc::clone(&admission));
    let output_guard = ConsensusOutputGuard::isolated();
    let operation = output_guard
        .begin_fail_stop_operation()
        .expect("open fail-stop operation");
    let target = LifecycleIngressIoTargetSeal::for_test(
        &service.context,
        LifecycleIngressIoTargetKind::CertifiedFetchBodyPersistence,
        8,
    );
    let V2IoLifecycleCapacityCapture::Reserved(reservation) = sender
        .queue
        .capture_lifecycle_capacity(operation, Arc::clone(&output_guard), target)
        .expect("reserve one exact consensus slot")
    else {
        panic!("empty consensus suffix must reserve");
    };
    drop(reservation);
    assert!(output_guard.restart_required());
    assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
}
#[test]
fn lifecycle_capacity_uses_hierarchical_class_and_release_generation() {
    let (service, _) = fixture();
    let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded I/O admission"));
    let capacity = admission.capacity();
    let (sender, _receiver) = v2_io_command_channel(capacity, 1, 1, 1, Arc::clone(&admission));
    assert!(admission.try_reserve(V2IoAdmissionClass::Auxiliary));
    let output_guard = ConsensusOutputGuard::isolated();
    let auxiliary = LifecycleIngressIoTargetSeal::for_test(
        &service.context,
        LifecycleIngressIoTargetKind::CertifiedServe,
        11,
    );
    let operation = output_guard
        .begin_fail_stop_operation()
        .expect("open fail-stop operation");
    let V2IoLifecycleCapacityCapture::Unavailable(wait) = sender
        .queue
        .capture_lifecycle_capacity(operation, Arc::clone(&output_guard), auxiliary)
        .expect("full auxiliary prefix yields a generation wait")
    else {
        panic!("Serve must not borrow consensus-reserved capacity");
    };
    assert!(Arc::ptr_eq(&wait.queue, &sender.queue));
    assert_eq!(wait.observed_generation, 0);
    assert_eq!(
        wait.target.kind(),
        LifecycleIngressIoTargetKind::CertifiedServe
    );
    let consensus = LifecycleIngressIoTargetSeal::for_test(
        &service.context,
        LifecycleIngressIoTargetKind::CertifiedFetchBodyPersistence,
        12,
    );
    let operation = output_guard
        .begin_fail_stop_operation()
        .expect("second fail-stop operation");
    let V2IoLifecycleCapacityCapture::Reserved(reservation) = sender
        .queue
        .capture_lifecycle_capacity(operation, Arc::clone(&output_guard), consensus)
        .expect("consensus suffix remains available")
    else {
        panic!("certified-Fetch persistence owns consensus capacity");
    };
    reservation.cancel_before_plan_for_test();
    assert_eq!(admission.lifecycle_capacity_generation(), 1);
    assert!(admission.lifecycle_capacity_generation() > wait.observed_generation);
    admission.release();
}
#[test]
fn lifecycle_capacity_wait_classifies_live_release_and_terminal_service_loss() {
    let (mut service, _) = fixture();
    let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded I/O admission"));
    let (sender, _receiver) =
        v2_io_command_channel(admission.capacity(), 1, 1, 1, Arc::clone(&admission));
    let output_guard = ConsensusOutputGuard::isolated();
    let wait = LifecycleIoCapacityWait {
        queue: Arc::clone(&sender.queue),
        output_guard: Arc::clone(&output_guard),
        target: LifecycleIngressIoTargetSeal::for_test(
            &service.context,
            LifecycleIngressIoTargetKind::CertifiedFetchBodyPersistence,
            14,
        ),
        observed_generation: 0,
    };
    let (_completion_tx, completion_rx) = mpsc::sync_channel(1);
    service.output_guard = Arc::clone(&output_guard);
    service.io = Some(V2IoHandle {
        command_tx: sender,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission: Arc::clone(&admission),
    });
    assert_eq!(
        wait.status(&service),
        LifecycleIoCapacityWaitStatus::SamePending
    );
    assert!(admission.try_reserve(V2IoAdmissionClass::Consensus));
    admission.release();
    assert_eq!(
        wait.status(&service),
        LifecycleIoCapacityWaitStatus::Released
    );
    admission
        .lifecycle_capacity_generation
        .store(u64::MAX, AtomicOrdering::Release);
    admission
        .lifecycle_capacity_generation_exhausted
        .store(true, AtomicOrdering::Release);
    assert_eq!(
        wait.status(&service),
        LifecycleIoCapacityWaitStatus::GenerationExhausted
    );
    output_guard.activate_restart_required();
    assert_eq!(
        wait.status(&service),
        LifecycleIoCapacityWaitStatus::ForeignOrDisconnected
    );
    drop(service.io.take());
    assert_eq!(
        wait.status(&service),
        LifecycleIoCapacityWaitStatus::ForeignOrDisconnected
    );
}
#[test]
fn lifecycle_capacity_generation_exhaustion_never_wraps() {
    let (service, _) = fixture();
    let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded I/O admission"));
    let capacity = admission.capacity();
    let (sender, _receiver) = v2_io_command_channel(capacity, 1, 1, 1, Arc::clone(&admission));
    assert!(admission.try_reserve(V2IoAdmissionClass::Auxiliary));
    admission
        .lifecycle_capacity_generation
        .store(u64::MAX, AtomicOrdering::Release);
    admission.release();
    assert_eq!(admission.lifecycle_capacity_generation(), u64::MAX);
    assert!(admission.lifecycle_capacity_generation_exhausted());
    let output_guard = ConsensusOutputGuard::isolated();
    let operation = output_guard
        .begin_fail_stop_operation()
        .expect("open fail-stop operation");
    let target = LifecycleIngressIoTargetSeal::for_test(
        &service.context,
        LifecycleIngressIoTargetKind::CertifiedServe,
        13,
    );
    let Err((failure, restored)) =
        sender
            .queue
            .capture_lifecycle_capacity(operation, Arc::clone(&output_guard), target)
    else {
        panic!("exhausted generation must fail closed");
    };
    assert_eq!(
        failure,
        LifecycleIoCapacityCaptureFailure::GenerationExhausted
    );
    assert_eq!(
        restored.kind(),
        LifecycleIngressIoTargetKind::CertifiedServe
    );
    assert!(output_guard.restart_required());
}
#[test]
fn lifecycle_capacity_rejects_repeat_fetch_while_work_is_completion_pending() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let work_id = EffectWorkId::for_test(21);
    let durable = DurableBodyReceipt::for_test(
        service.context.id(),
        proposal.round,
        proposal.subject,
        HashOf::new(&proposal.manifest),
    );
    let (sender, receiver, admission) = test_io_command_channel(2);
    sender
        .try_send(V2IoCommand::Validate(BodyValidationTask::for_test(
            21, durable,
        )))
        .expect("install one exact in-flight work owner");
    assert!(matches!(receiver.try_recv(), Ok(V2IoCommand::Validate(_))));
    receiver.complete_work(work_id);
    assert_eq!(
        sender.queue.lock().work[&work_id].state,
        V2IoWorkState::CompletionPending
    );
    let output_guard = ConsensusOutputGuard::isolated();
    let operation = output_guard
        .begin_fail_stop_operation()
        .expect("open fail-stop operation");
    let target = LifecycleIngressIoTargetSeal::for_test(
        &service.context,
        LifecycleIngressIoTargetKind::CertifiedFetchBodyPersistence,
        work_id.get(),
    );
    let V2IoLifecycleCapacityCapture::Reserved(reservation) = sender
        .queue
        .capture_lifecycle_capacity(operation, Arc::clone(&output_guard), target)
        .expect("the service can reserve while the prior owner is active")
    else {
        panic!("repeat preflight needs one locked capacity reservation");
    };
    assert!(!reservation.preflight_selected_target_work_absent());
    reservation.cancel_before_plan_for_test();
    assert_eq!(
        sender.queue.lock().work[&work_id].state,
        V2IoWorkState::CompletionPending
    );
    sender.acknowledge_completion(work_id);
    assert!(sender.queue.lock().work.is_empty());
    assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
}
#[test]
fn unconsumed_certified_fetch_persistence_closes_output() {
    let abandoned_output = ConsensusOutputGuard::isolated();
    drop(CertifiedFetchBodyPersistenceDropGuard::new(Arc::clone(
        &abandoned_output,
    )));
    assert!(abandoned_output.restart_required());
    assert!(abandoned_output.acquire().is_none());
    let transferred_output = ConsensusOutputGuard::isolated();
    let mut transferred =
        CertifiedFetchBodyPersistenceDropGuard::new(Arc::clone(&transferred_output));
    transferred.disarm();
    drop(transferred);
    assert!(!transferred_output.restart_required());
    assert!(transferred_output.acquire().is_some());
}
