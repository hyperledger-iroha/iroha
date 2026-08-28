#[test]
fn recovered_decision_fetch_composite_dispatch_reserves_capacity_before_claim_and_commit() {
    let scheduler = include_str!("v2_lifecycle_scheduler_inputs.rs");
    let dispatch = scheduler
        .split_once("fn dispatch_completion_with_runner_debt(")
        .expect("lifecycle Completion has one composite dispatch transaction")
        .1
        .split_once(
            "/// Reserve, claim, and dispatch the sole Ready lifecycle-owned recovered Sign.",
        )
        .expect("composite dispatch stays a bounded source region")
        .0;
    let census = dispatch
        .find("capture_lifecycle_completion_capacity_census(probes)")
        .expect("the joint physical census is captured");
    let claim = dispatch
        .find("self.coordinator.plan_turn(inputs)")
        .expect("coordinator claim exists");
    let output = dispatch
        .find(".select_fetch(ordinal)")
        .expect("the selected Fetch owns exact output");
    let executor = dispatch
        .find("prepare_recovered_decision_fetch_request_registration(owner)")
        .expect("executor vacancy is reserved");
    let staged_wait = dispatch
        .find("let mut next = self.coordinator.stage_durable_transaction();")
        .expect("the exact external wait is staged before owner mutation");
    let registry = dispatch
        .find("prepare_recovered_decision_fetch_dispatch(")
        .expect("the claimed row projects its exact task");
    let commit = dispatch
        .find("registration.commit(prepared, wait_source)")
        .expect("request owner has one commit tail");
    let waiting = dispatch
        .find("self.coordinator = next;")
        .expect("the claimed Fetch is parked before external publication");
    let publication = dispatch
        .find("output.commit();")
        .expect("exact output publishes after request installation");
    assert!(
        census < claim
            && claim < output
            && output < executor
            && executor < staged_wait
            && staged_wait < registry
            && registry < commit
            && commit < waiting
            && waiting < publication
    );
}

#[test]
fn recovered_decision_fetch_queue_parks_generic_drain_and_uses_unified_completion_classifier() {
    let worker = [
        include_str!("v2_worker.rs"),
        include_str!("v2_worker_services_impl.rs"),
    ]
    .concat();
    let generic = worker
        .split_once("fn take_io_completion(")
        .expect("generic completion selector exists")
        .1
        .split_once("fn take_recovered_lifecycle_sign_completion(")
        .expect("generic selector stays bounded")
        .0;
    assert!(generic.contains("V2IoCompletion::RecoveredDecisionFetchBodyPersisted(_)"));
    assert!(generic.contains("self.held_io_completion = Some(completion);"));
    let classifier = worker
        .split_once("fn take_next_lifecycle_completion(")
        .expect("unified recovered lifecycle classifier exists")
        .1
        .split_once("pub(in crate::sumeragi) fn drain_recovered_lifecycle_sign_completion(")
        .expect("unified classifier stays bounded")
        .0;
    assert!(classifier.contains("V2IoCompletion::RecoveredDecisionFetchBodyPersisted(guarded)"));
    assert!(classifier.contains("LifecycleCompletionTakeV1::DecisionFetch("));
    assert!(worker.contains("tracked.state = V2IoWorkState::Active;"));
    assert!(worker.contains("tracked.state = V2IoWorkState::CompletionPending;"));
    assert!(!worker.contains("drain_recovered_decision_fetch_body_completion"));
}

#[test]
fn ordinary_certified_body_pipeline_has_no_retained_compatibility_carrier() {
    let effects = include_str!("v2_effects.rs");
    let runtime = include_str!("v2_runtime.rs");
    let run_inner = include_str!("v2_runner/lifecycle_run_inner.rs");
    let ordinary_consumer = include_str!("v2_runner/ordinary_ingress_consumer.rs");
    let turn_driver = include_str!("v2_lifecycle_turn_driver.rs");

    for (source, forbidden) in [
        (effects, concat!("RetainedCertifiedBody", "Response")),
        (effects, concat!("retained_certified_body_", "response")),
        (
            effects,
            concat!("accept_certified_body_", "response_with_ingress_ownership"),
        ),
        (runtime, "retained_response_predecessor_target_ordinal"),
        (runtime, "retained_response_predecessor_retry_attempted"),
        (
            run_inner,
            concat!("service_retained_certified_", "response"),
        ),
        (
            run_inner,
            concat!("retry_retained_certified_body_", "response"),
        ),
    ] {
        assert!(
            !source.contains(forbidden),
            "retired ordinary response compatibility surface returned: {forbidden}",
        );
    }
    assert!(
        ordinary_consumer.contains("retired certified body response outside lifecycle selection")
    );
    assert!(
        ordinary_consumer
            .contains("a selected fetch response must instead complete through lifecycle")
    );
    assert!(!ordinary_consumer.contains(concat!("accept_certified_body_", "response(")));
    assert!(turn_driver.contains("drive_certified_fetch_ingress_selector(selector, runner)"));
    assert!(turn_driver.contains("complete_certified_fetch_body_persistence("));
}

#[test]
fn blocked_ordinary_lifecycle_owner_services_only_lane_local_fair_ingress_before_yield() {
    let run_inner = include_str!("v2_runner/lifecycle_run_inner.rs");
    let reconciled_turn = source_region(
        run_inner,
        "let directive = reconcile_executor_locked_body(executor, services)?;",
        "services\n                        .replay_buffered_chunks(executor)",
    );
    assert_source_tokens_in_order(
        reconciled_turn,
        &[
            "local_proposal\n                        .state\n                        .reconcile(LocalProposalOwner::from(directive))",
            "lane_work.retain_merge_sidecars_for_global_view(",
            "executor.acknowledge_runner_decision_cleanup(",
            "producer_claim.blocked_ordinary_lane_local_ingress_permit()",
            "drain_blocked_ordinary_lane_local_ingress(",
            "drain_lane_relay_ingress(",
            "drive_merge_sidecar_recovery(executor, services, &mut lane_work)",
            "service_historical_recovery_tick(&mut lane_work)",
            "lane_work.schedule_autonomous_new_view_timeouts(",
            "lane_work.schedule_retransmission()",
            "dispatch_lane_work_effects(",
        ],
    );
    for forbidden in [
        "drive_ingress_turn(",
        "commit_certified_serve(",
        "mark_leader_wire_volatile",
        "bind_leader_wire_runtime_ownership",
    ] {
        assert!(
            !reconciled_turn.contains(forbidden),
            "blocked ordinary lane-local turn regained global authority: {forbidden}"
        );
    }

    let runtime_barrier = source_region(
        run_inner,
        "if lane_only_completion_barrier {",
        "} else {\n            activated.with_runner_runtime(",
    );
    assert_source_tokens_in_order(
        runtime_barrier,
        &[
            "producer_claim.blocked_ordinary_lane_local_ingress_permit()",
            "drain_blocked_ordinary_lane_local_ingress(",
            "drain_lane_relay_ingress(",
            "drive_merge_sidecar_recovery(executor, services, &mut lane_work)",
            "service_historical_recovery_tick(&mut lane_work)",
            "lane_work.schedule_autonomous_new_view_timeouts(",
            "lane_work.schedule_retransmission()",
            "dispatch_lane_work_effects(",
        ],
    );

    let height_driver = include_str!("v2_runner/lifecycle_height_driver.rs");
    let permit = source_region(
        height_driver,
        ") -> Option<LifecycleBlockedOrdinaryLaneLocalIngressPermitV1> {",
        "fn observe_completion(",
    );
    assert_source_tokens_in_order(
        permit,
        &[
            "self.blocks_ingress()",
            "!self.permits_decided_lane_recovery_ingress()",
            "LifecycleBlockedOrdinaryLaneLocalIngressPermitV1",
        ],
    );

    let recovery = include_str!("v2_runner/decided_lane_recovery.rs");
    let selector = source_region(
        recovery,
        "fn select_blocked_ordinary_lane_local_ingress(",
        "fn drain_blocked_ordinary_lane_local_ingress(",
    );
    assert!(selector.contains(".try_recv_lifecycle_lane_local_checked(permit)"));
    for forbidden in [
        "CertifiedBodyRequest",
        "commit_certified_serve",
        "KuraReplicaAdvert",
        "LeaderWireRetire",
    ] {
        assert!(
            !selector.contains(forbidden),
            "lane-local selector gained a global ingress class: {forbidden}"
        );
    }

    let post_drain = source_region(
        run_inner,
        "producer_claim = drain_disposition.producer_claim();",
        "let (ready_to_finish, executor_slice)",
    );
    assert_source_tokens_in_order(
        post_drain,
        &[
            "if drain_disposition.requires_yield()",
            "wake_rx.recv_timeout(IDLE_POLL)",
            "continue;",
        ],
    );
}

#[test]
fn active_height_tail_bounds_executor_work_before_the_producer_point() {
    let run_inner = include_str!("v2_runner/lifecycle_run_inner.rs");
    let runner = include_str!("v2_runner.rs");
    let height_driver = include_str!("v2_runner/lifecycle_height_driver.rs");
    let post_drain_runtime = source_region(
        run_inner,
        "let (ready_to_finish, executor_slice)",
        "let apply_terminal_settled",
    );
    let executor_slice = source_region(
        post_drain_runtime,
        "let executor_slice = advance_executor(",
        ")?;",
    );

    let compact_executor_slice: String = executor_slice
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert_eq!(compact_executor_slice, "receiver,owner,executor,services,1");
    let post_slice_suffix = source_region(
        post_drain_runtime,
        "let executor_slice = advance_executor(",
        "if directive.decided_subject().is_none()",
    );
    assert_source_tokens_in_order(
        post_slice_suffix,
        &[
            "if let AdvanceExecutorSliceOutcomeV1::Yielded(_) = executor_slice",
            "return Ok::<_, V2RunnerError>((false, executor_slice));",
            "retry_exact_output_and_apply_sidecar_admissions(",
            "let directive = reconcile_executor_locked_body(executor, services)?",
        ],
    );
    assert!(
        !post_slice_suffix.contains(
            "if executor_slice == AdvanceExecutorSliceOutcomeV1::AdvancedAtSliceBoundary"
        )
    );

    let producer_suffix =
        source_region(run_inner, "match executor_slice", "let finalization_ready");
    assert_source_tokens_in_order(
        producer_suffix,
        &[
            "AdvanceExecutorSliceOutcomeV1::Idle",
            "AdvanceExecutorSliceOutcomeV1::AdvancedAtSliceBoundary => {}",
            "AdvanceExecutorSliceOutcomeV1::Yielded(reason)",
            "continue;",
            "claim_producer_turn_for_local_proposal(&mut active_runner)",
        ],
    );

    let bounded_executor = source_region(
        runner,
        "fn advance_executor(",
        "fn recovered_lifecycle_output_yield_cause(",
    );
    assert!(
        bounded_executor
            .contains("EffectExecutorStep::Idle => return Ok(AdvanceExecutorSliceOutcomeV1::Idle)")
    );
    assert!(
        bounded_executor
            .trim_end()
            .ends_with("Ok(AdvanceExecutorSliceOutcomeV1::AdvancedAtSliceBoundary)\n}")
    );

    let runtime_turn = source_region(
        height_driver,
        "LifecycleRunnerRankTarget::Runtime =>",
        "LifecycleRunnerRankTarget::Ingress =>",
    );
    assert_source_tokens_in_order(
        runtime_turn,
        &[
            "advance_executor(receiver, owner, executor, services, 1)?",
            "match executor_slice",
            "AdvanceExecutorSliceOutcomeV1::Idle",
            "AdvanceExecutorSliceOutcomeV1::AdvancedAtSliceBoundary => {}",
            "AdvanceExecutorSliceOutcomeV1::Yielded(advance_executor_yield)",
        ],
    );
}

#[test]
fn apply_barriers_reconcile_current_serve_and_unadmitted_fetch_capacity_before_direct_recovery() {
    let run_inner = include_str!("v2_runner/lifecycle_run_inner.rs");
    let barrier = source_region(
        run_inner,
        "let lane_only_completion_barrier = producer_claim.blocks_runtime();",
        "let discovery_was_outstanding = if lane_only_completion_barrier",
    );
    assert_source_tokens_in_order(
        barrier,
        &[
            "producer_claim.decided_lane_recovery_permit()",
            ".reconcile_decided_lane_certified_serve(&mut active_runner, permit)",
            "activated.with_runner_runtime(",
            "producer_claim.permits_decided_lane_recovery_ingress()",
            "settle_apply_barrier_runner_decision_handoff(",
            "reconcile_terminal_lane_output_handoffs(",
            "drain_decided_lane_recovery_ingress(",
        ],
    );
    assert!(
        !barrier.contains("drive_ingress_turn("),
        "an Apply barrier may inherit a capacity wait but cannot admit fresh ordinary ingress"
    );

    let handoff = source_region(
        run_inner,
        "pub(in crate::sumeragi) fn settle_apply_barrier_runner_decision_handoff(",
        "#[allow(clippy::too_many_arguments, clippy::too_many_lines)]",
    );
    assert_source_tokens_in_order(
        handoff,
        &[
            "executor.reconcile_pending_runner_decision_cleanup(services)?",
            "let directive = executor.local_proposal_directive()?",
            "local_proposal\n        .state\n        .reconcile(LocalProposalOwner::from(directive))",
            "lane_work.retain_merge_sidecars_for_global_view(",
            "executor.acknowledge_runner_decision_cleanup(",
        ],
    );

    let turn_driver = include_str!("v2_lifecycle_turn_driver.rs");
    let reconcile = source_region(
        turn_driver,
        "fn reconcile_decided_lane_certified_serve(",
        "/// Claim the oldest lifecycle-owned ProducerTurn",
    );
    assert_source_tokens_in_order(
        reconcile,
        &[
            "drain_lifecycle_certified_serve_completion()",
            ".settle_deliver_and_acknowledge(",
            "self.launched.pending_ingress_capacity.take()",
            "PendingIngressCapacityV1::CertifiedServe(wait)",
            "PendingIngressCapacityV1::CertifiedFetch(wait)",
            "pending @ PendingIngressCapacityV1::RecoveredDecisionFetch(_)",
        ],
    );
    let certified_serve = source_region(
        reconcile,
        "PendingIngressCapacityV1::CertifiedServe(wait)",
        "PendingIngressCapacityV1::CertifiedFetch(wait)",
    );
    assert_source_tokens_in_order(
        certified_serve,
        &[
            "LifecycleIoCapacityWaitStatus::SamePending",
            "LifecycleIoCapacityWaitStatus::Released",
            "drop(wait)",
            "LifecycleIoCapacityWaitStatus::GenerationExhausted",
            "close_admission_for_restart()",
        ],
    );
    let certified_fetch = source_region(
        reconcile,
        "PendingIngressCapacityV1::CertifiedFetch(wait)",
        "pending @ PendingIngressCapacityV1::RecoveredDecisionFetch(_)",
    );
    assert_source_tokens_in_order(
        certified_fetch,
        &[
            "wait.capacity_status(&self.launched.services)",
            "ProductionIngressCapacityStatus::Pending",
            "ProductionIngressCapacityStatus::Released",
            "This attempt captured capacity before admitting a new",
            "drop(wait)",
            "ProductionIngressCapacityStatus::GenerationExhausted",
            "close_admission_for_restart()",
        ],
    );
    let (_, recovered_fetch) = reconcile
        .split_once("pending @ PendingIngressCapacityV1::RecoveredDecisionFetch(_)")
        .expect("recovered Decision-Fetch capacity branch");
    assert_source_tokens_in_order(
        recovered_fetch,
        &[
            "self.launched.pending_ingress_capacity = Some(pending)",
            "close_admission_for_restart()",
            "terminal barrier retained a recovered Decision-Fetch ingress-capacity owner",
        ],
    );
    assert!(!reconcile.contains("drive_completion_turn("));
    assert!(!reconcile.contains("drive_ingress_turn("));
    assert!(!reconcile.contains("drain_completions_with_lifecycle("));
}

#[test]
fn recovered_decision_fetch_phase_a_is_reachable_only_after_runner_validation() {
    let driver = include_str!("v2_lifecycle_turn_driver.rs");
    let scheduler = include_str!("v2_lifecycle_scheduler_inputs.rs");
    let ingress_turn = driver
        .split_once("pub(in crate::sumeragi) fn drive_ingress_turn")
        .expect("unified ingress driver exists")
        .1
        .split_once("fn drive_recovered_ingress_selector")
        .expect("runner validation precedes the recovered Phase-A helper")
        .0;
    let cursor = ingress_turn
        .find("if !self.runner_turn_matches(")
        .expect("the driver validates the borrow-bound runner");
    let handoff = ingress_turn
        .find("self.drive_recovered_ingress_selector(selector, runner)")
        .expect("the validated runner enters recovered Phase A");
    assert!(cursor < handoff);
    assert!(driver.contains("persist_recovered_decision_fetch_response_after_runner("));
    assert!(!scheduler.contains("fn persist_recovered_decision_fetch_response("));
}

#[test]
fn authenticated_current_serve_context_drift_fails_closed_instead_of_retrying() {
    let driver = include_str!("v2_lifecycle_turn_driver.rs");
    let narrowing = source_region(
        driver,
        "let expected_context = lifecycle_context_for_ingress(executor.context());",
        "capture_fenced_certified_serve_ingress_selector(lifecycle_cut)",
    );
    assert_source_tokens_in_order(
        narrowing,
        &[
            "Ok(FairIngressTurnContextCut::Ordinary(cut))",
            "authenticated current Certified-Serve lost its active lifecycle context",
            "close_admission_for_restart()",
            "drop(cut);",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::RestartRequired",
        ],
    );
    assert!(!driver.contains("OrdinaryRetained"));
}

#[test]
fn authenticated_current_serve_queue_refresh_retries_without_closing_output() {
    let driver = include_str!("v2_lifecycle_turn_driver.rs");
    let fence = source_region(
        driver,
        "let cut = match cut.fence_producer_publication_retaining()",
        "let classified = prepare_current_certified_serve_pre_admission(",
    );
    assert_source_tokens_in_order(
        fence,
        &[
            "Err((FairIngressQueueCutError::QueueCutChanged, retained))",
            "drop(retained);",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedServeRetry",
            "Err((error, retained))",
            "close_admission_for_restart()",
            "ProductionLifecycleIngressSelectionV1::RestartRequired",
        ],
    );
    let narrowing = source_region(
        driver,
        "let expected_context = lifecycle_context_for_ingress(executor.context());",
        "capture_fenced_certified_serve_ingress_selector(lifecycle_cut)",
    );
    let retry = source_region(
        narrowing,
        "Err((FairIngressQueueCutError::QueueCutChanged, retained))",
        "Err((error, retained))",
    );
    assert_source_tokens_in_order(
        retry,
        &[
            "drop(retained);",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedServeRetry",
        ],
    );
    assert!(!retry.contains("close_admission_for_restart()"));

    let structural_failure = source_region(
        driver,
        "Err((error, retained))",
        "capture_fenced_certified_serve_ingress_selector(lifecycle_cut)",
    );
    assert_source_tokens_in_order(
        structural_failure,
        &[
            "close_admission_for_restart()",
            "drop(retained);",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::RestartRequired",
        ],
    );

    let selector_capture = source_region(
        driver,
        "capture_fenced_certified_serve_ingress_selector(lifecycle_cut)",
        "let (dequeue, target)",
    );
    let selector_retry = source_region(
        selector_capture,
        "Err(LifecycleIngressSelectorError::QueueCutChanged)",
        "Err(error)",
    );
    assert_source_tokens_in_order(
        selector_retry,
        &[
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedServeRetry",
        ],
    );
    assert!(!selector_retry.contains("close_admission_for_restart()"));

    let selector_structural_failure = source_region(
        driver,
        "authenticated current Certified-Serve selector capture failed closed",
        "let (dequeue, target)",
    );
    assert_source_tokens_in_order(
        selector_structural_failure,
        &[
            "close_admission_for_restart()",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::RestartRequired",
        ],
    );

    let exact_dequeue = source_region(driver, "let (dequeue, target)", "let ready_ledger =");
    let exact_dequeue_retry = source_region(
        exact_dequeue,
        "Err(CertifiedServeExactDequeueErrorV1::Queue(",
        "Err(error)",
    );
    assert_source_tokens_in_order(
        exact_dequeue_retry,
        &[
            "FairIngressQueueCutError::QueueCutChanged",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedServeRetry",
        ],
    );
    assert!(!exact_dequeue_retry.contains("close_admission_for_restart()"));

    let exact_dequeue_structural_failure = source_region(
        driver,
        "Certified-Serve exact dequeue failed closed",
        "let ready_ledger =",
    );
    assert_source_tokens_in_order(
        exact_dequeue_structural_failure,
        &[
            "close_admission_for_restart()",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::RestartRequired",
        ],
    );
}

#[test]
fn certified_response_queue_refresh_retries_without_closing_output() {
    let driver = include_str!("v2_lifecycle_turn_driver.rs");
    let response_path = source_region(
        driver,
        "if !selected_ingress_is_certified_body_response",
        "fn drive_recovered_ingress_selector",
    );

    let narrowing = source_region(
        response_path,
        "let expected_context = lifecycle_context_for_ingress(self.executor.context());",
        "match contextual",
    );
    let narrowing_retry = source_region(
        narrowing,
        "Err((FairIngressQueueCutError::QueueCutChanged, retained))",
        "Err((error, retained))",
    );
    assert_source_tokens_in_order(
        narrowing_retry,
        &[
            "drop(retained);",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry",
        ],
    );
    assert!(!narrowing_retry.contains("close_output_for_restart()"));

    let priority = source_region(
        response_path,
        "let selected_priority = match self",
        "match selected_priority",
    );
    let ordinary = source_region(
        response_path,
        "SelectedCertifiedResponsePriorityV1::OrdinaryClaimed",
        "SelectedCertifiedResponsePriorityV1::RecoveredClaimed",
    );
    let recovered = source_region(
        response_path,
        "SelectedCertifiedResponsePriorityV1::RecoveredClaimed",
        "self.drive_recovered_ingress_selector(selector, runner)",
    );

    let priority_retry = source_region(
        priority,
        "Err(LifecycleIngressSelectorError::QueueCutChanged)",
        "Err(error)",
    );
    assert_source_tokens_in_order(
        priority_retry,
        &[
            "drop(cut);",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry",
        ],
    );
    assert!(!priority_retry.contains("close_output_for_restart()"));

    for retry in [
        source_region(
            ordinary,
            "Err(LifecycleIngressSelectorError::QueueCutChanged)",
            "Err(error)",
        ),
        source_region(
            recovered,
            "Err(LifecycleIngressSelectorError::QueueCutChanged)",
            "Err(error)",
        ),
    ] {
        assert_source_tokens_in_order(
            retry,
            &[
                "drop(runner);",
                "ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry",
            ],
        );
        assert!(!retry.contains("close_output_for_restart()"));
    }

    for structural_failure in [
        source_region(narrowing, "Err((error, retained))", "};"),
        source_region(priority, "Err(error)", "};"),
        source_region(ordinary, "Err(error)", "};"),
        source_region(recovered, "Err(error)", "};"),
    ] {
        assert_source_tokens_in_order(
            structural_failure,
            &[
                "close_output_for_restart();",
                "drop(runner);",
                "ProductionLifecycleIngressSelectionV1::RestartRequired",
            ],
        );
    }
}

#[test]
fn recovered_decision_fetch_phase_a_wakes_waiting_owner_before_queue_publication() {
    let scheduler = include_str!("v2_lifecycle_scheduler_inputs.rs");
    let registry = include_str!("v2_lifecycle_work_registry_validate_recovery_registry_impl.rs");
    let phase_a = source_region(
        scheduler,
        "pub(super) fn persist_recovered_decision_fetch_response_after_runner(",
        "/// Plan, submit, and reblock one exact selected certified-Fetch response.",
    );
    assert_source_tokens_in_order(
        phase_a,
        &[
            "self.coordinator.active_lease.is_some()",
            "attest_scheduler_recovered_fetch_carrier(",
            "capture_lifecycle_capacity_rank(selector)",
            "authenticated_waiting_fetch_ready_row(",
            "prepare_recovered_decision_fetch_response_claim(&task)",
            "let mut next = self.coordinator.stage_durable_transaction();",
            "let lease = match next.plan_turn(inputs)",
            "matches_claimed_dispatched_recovered_decision_fetch(",
            "self.coordinator = next;",
            "claim.commit_with_queue(reservation, task);",
        ],
    );
    let swap = source_token_position(phase_a, "self.coordinator = next;");
    let tail = &phase_a[swap..];
    assert!(!tail.contains("return Err"));
    assert!(!tail.contains("settle_turn("));
    assert!(tail.contains("assert_eq!(self.coordinator.active_lease.as_ref(), Some(&lease))"));
    let waiting_carrier = source_region(
        registry,
        "pub(super) fn matches_waiting_dispatched_recovered_decision_fetch(",
        "/// Join one exact claimed recovered Decision Fetch back to its closed carrier.",
    );
    assert_required_source_tokens(
        waiting_carrier,
        &[
            "coordinator.records.iter().any",
            "*candidate != ordinal",
            "wait.source() == wait_source",
        ],
    );
}

#[test]
fn recovered_decision_fetch_response_claim_precedes_assertion_only_queue_publication() {
    let effects = reviewed_v2_effects_source_for_test();
    let commit = effects
        .split_once("pub(in crate::sumeragi) fn commit_with_queue(")
        .expect("recovered response has one composite commit")
        .1
        .split_once("impl RecoveredDecisionFetchResponseCandidateV1")
        .expect("composite commit stays bounded")
        .0;
    let claim = commit
        .find("owner.commit_exact_response_claim(response_hash)")
        .expect("exact response claim is installed");
    let queue = commit
        .find("queue.commit_recovered_decision_fetch_body_persistence(task)")
        .expect("dedicated persistence is published");
    assert!(claim < queue);
    assert!(commit.contains("assert!(owner.matches_response_claim_preflight"));
    let worker = include_str!("v2_worker.rs");
    let queue_commit = worker
        .split_once("fn commit_recovered_decision_fetch_body_persistence(")
        .expect("dedicated queue commit exists")
        .1
        .split_once("#[cfg(test)]")
        .expect("queue commit stays bounded")
        .0;
    assert!(queue_commit.contains("assert!("));
    assert!(!queue_commit.contains("return Err"));
}
