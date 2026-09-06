#[test]
fn pending_kura_mixed_decision_fetch_services_older_cold_output_before_producer_turn() {
    let pending_classifier_source = include_str!("v2_pending_kura_recovery.rs");
    let pending_lifecycle_source = include_str!("v2_lifecycle_pending_kura.rs");
    let cold_output_source = include_str!("v2_lifecycle_open_output_recovery.rs");
    let pending_runner_source = include_str!("v2_runner/lifecycle_pending_kura.rs");

    let classifier = source_region(
        pending_classifier_source,
        "pub(crate) fn authenticate_final_wal_startup_authority(",
        "#[cfg(test)]\nimpl AuthenticatedRecoveredPendingKuraAdapterStartupV1",
    );
    assert_source_tokens_in_order(
        classifier,
        &[
            "let RecoveredWalStartupAuthorityV1::DecisionFetch(fetch) = authority",
            "authority: RecoveredWalStartupAuthorityV1::None",
            "validation_authority,\n                factory_owner,\n            },",
            "replay: RecoveredPendingKuraApplyReplayV1 { expected, fetch }",
        ],
    );

    let readiness = source_region(
        pending_lifecycle_source,
        "fn locally_ready_for_finalized_rollover(&mut self) -> bool",
        "/// Settle at most one owner-held cold output in the no-clock corridor.",
    );
    assert_required_source_tokens(
        readiness,
        &[
            "self.launched.executor.ready_to_finish()",
            "!self.launched.owner.has_recovered_lifecycle_outputs()",
            "exactly_covers_finalization_work(&self.launched.owner.coordinator)",
        ],
    );

    let cold_broadcast = source_region(
        cold_output_source,
        "pub(super) fn attest_ready_recovered_lifecycle_broadcast(",
        "/// Execute and terminalize the oldest eligible authenticated cold output.",
    );
    assert_required_source_tokens(
        cold_broadcast,
        &[
            "candidate.work_class != super::LifecycleWorkClass::Broadcast",
            "!self.coordinator.ready_index.contains(&ordinal)",
            "recovered_output_matches_ready_coordinator(",
        ],
    );

    let cold_output_settlement = source_region(
        cold_output_source,
        "pub(in crate::sumeragi) fn settle_next_recovered_lifecycle_output<E>(",
        "fn recovered_output_matches_ready_coordinator(",
    );
    assert_source_tokens_in_order(
        cold_output_settlement,
        &[
            "let Some(first_ready) = coordinator.ready_index.first().copied()",
            "if coordinator.active_lease.is_some() || first_ready < ordinal",
            "execute(output.effect())",
            "LifecycleOutputServiceDispositionV1::SourceRetained",
            "RecoveredLifecycleOutputSettlementV1::SourceRetained",
            "finish_terminal(ordinal, super::TerminalOutcome::Advanced)",
            "persist_exact_staged_successor(&staged)",
            "let retired = outputs",
            ".remove(&ordinal)",
            "RecoveredLifecycleOutputSettlementV1::Completed",
        ],
    );

    let bounded_cold_output_turn = source_region(
        pending_lifecycle_source,
        "pub(in crate::sumeragi) fn settle_recovered_lifecycle_output_for_no_clock_recovery(",
        "/// Return whether the interrupted-tip executor and lifecycle owner can",
    );
    assert_source_tokens_in_order(
        bounded_cold_output_turn,
        &[
            "retry_pending_exact_output()",
            "settle_one_recovered_lifecycle_output(",
            "&mut self.launched.owner",
            "&mut self.launched.executor",
            "&mut self.launched.services",
        ],
    );
    for forbidden in [
        "drive_ingress",
        "advance_executor",
        "arm_live_clocks",
        "claim_producer_turn",
    ] {
        assert!(
            !bounded_cold_output_turn.contains(forbidden),
            "the bounded PendingKura cold-output turn cannot expose {forbidden}"
        );
    }

    let no_clock_loop = source_region(
        pending_runner_source,
        "fn run_pending_active_height(",
        "pub(super) fn run_pending_kura_lifecycle_height(",
    );
    assert_source_tokens_in_order(
        no_clock_loop,
        &[
            "settle_certified_serve_completion_for_no_clock_recovery",
            "drain_lane_relay_ingress(",
            "reconcile_pending_kura_terminal_lane_output_handoffs(",
            "if terminal_exact_output_pending",
            "wake_rx.recv_timeout(IDLE_POLL)",
            "settle_recovered_lifecycle_output_for_no_clock_recovery",
            "RecoveredLifecycleOutputSettlementV1::SourceRetained",
            "RecoveredLifecycleOutputSettlementV1::Completed",
            "RecoveredLifecycleOutputSettlementV1::Empty",
            "RecoveredLifecycleOutputSettlementV1::Deferred",
            "claim_producer_turn_for_no_clock_recovery",
            "retry_recovered_decision_fetch_if_due(",
        ],
    );
}

#[test]
fn pending_kura_actor_backpressure_gates_rollover_through_closed_prefix() {
    let pending_runner_source = include_str!("v2_runner/lifecycle_pending_kura.rs");
    let worker_source = include_str!("v2_worker_services_impl.rs");

    let exact_output_drive = source_region(
        worker_source,
        "fn drive_pending_exact_output(&self, pending: &mut PendingExactOutput)",
        "fn enqueue_exact_fanout_while_guarded(",
    );
    assert_source_tokens_in_order(
        exact_output_drive,
        &[
            "ExactOutputDriveOutcome::Backpressured",
            "Ok(pending.is_pending())",
        ],
    );

    let runtime_readiness = source_region(
        pending_runner_source,
        "let (ready_to_finish, terminal_exact_output_pending) =",
        "let finalization_ready = activated.ready_for_finalized_rollover",
    );
    assert_source_tokens_in_order(
        runtime_readiness,
        &[
            "dispatch_lane_work_effects(lane_work, services, control_queue_capacity)",
            "let terminal_exact_output_pending =\n                    retry_exact_output_and_apply_sidecar_admissions(",
            "Ok((executor.ready_to_finish(), terminal_exact_output_pending))",
            "let ready = ready_to_finish && !terminal_exact_output_pending",
            "if !ready",
            "wake_rx.recv_timeout(IDLE_POLL)",
        ],
    );

    let finalized_preflight = source_region(
        pending_runner_source,
        "let finalization_ready = activated.ready_for_finalized_rollover",
        "activated.close_runner_ingress_for_finalized_drain",
    );
    assert_source_tokens_in_order(
        finalized_preflight,
        &[
            "preflight_finalized_lane_rollover(",
            "reconcile_pending_kura_terminal_lane_output_handoffs(",
            "if !rollover_ready",
            "wake_rx.recv_timeout(IDLE_POLL)",
        ],
    );
    assert!(
        !finalized_preflight.contains("if terminal_exact_output_pending"),
        "successful preflight closes shared admission before exact-output waiting"
    );

    let closed_prefix = source_region(
        pending_runner_source,
        "activated.close_runner_ingress_for_finalized_drain",
        "let (finalized, lane_work) = activated.into_finalized_rollover",
    );
    assert_source_tokens_in_order(
        closed_prefix,
        &[
            "loop {",
            "DecidedLaneRecoveryIngressDrainMode::FinalizedClosedPrefix",
            "drain_finalized_lane_relay_prefix(",
            "dispatch_lane_work_effects(lane_work, services, control_queue_capacity)",
            "reconcile_pending_kura_terminal_lane_output_handoffs(",
            "if terminal_exact_output_pending",
            "wake_rx.recv_timeout(IDLE_POLL)",
            "if !drained_terminal_ingress",
            "!drained_terminal_relay",
            "break;",
            "ensure_closed_drained_cut()",
        ],
    );
}

#[test]
fn pending_kura_terminal_height_authenticates_after_closed_drain_without_a_successor() {
    let source = include_str!("v2_runner/lifecycle_pending_kura.rs");
    let terminal = source_region(
        source,
        "activated.close_runner_ingress_for_finalized_drain",
        "let (finalized, lane_work) = activated.into_finalized_rollover",
    );
    assert_source_tokens_in_order(
        terminal,
        &[
            "DecidedLaneRecoveryIngressDrainMode::FinalizedClosedPrefix",
            "drain_finalized_lane_relay_prefix(",
            "ensure_closed_drained_cut()",
            "if context.height == u64::MAX",
            "executor.durable_finality()",
            "authenticate_terminal_complete_tip(",
            "activated.into_clean_shutdown(&mut active_runner)?",
            "return Ok(HeightRunOutcome::Terminal);",
        ],
    );
    assert_forbidden_source_tokens(
        terminal,
        &["build_verified_successor(", "into_finalized_rollover("],
    );
}

#[test]
fn apply_terminal_settlement_fails_closed_if_runtime_reopens() {
    let source = include_str!("v2_runner/lifecycle_run_inner.rs");
    let terminal_tail = source_region(
        source,
        "let terminal_planning_fenced =",
        "if !terminal_planning_fenced\n            && pending_queue_plan_admission_dirty.swap",
    );

    assert_source_tokens_in_order(
        terminal_tail,
        &[
            "terminal_finalization_fenced || producer_claim.apply_terminal_settled()",
            "if terminal_planning_fenced && !ready_to_finish",
            "executor.ready_to_finish_blockers()",
            "terminal-finalization Completion reopened reducer/runtime ownership",
            "output_guard.close_admission_for_restart();",
            "return Err(V2RunnerError::RestartRequired);",
        ],
    );
    assert_forbidden_source_tokens(
        terminal_tail,
        &[
            "advance_executor_slice(",
            "schedule_local_proposal(",
            "wake_rx.recv_timeout(IDLE_POLL)",
            "continue;",
        ],
    );
}
