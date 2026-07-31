# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.


def test_serve_terminal_discharge_production_contract_is_complete(
    tmp_path: Path,
) -> None:
    """The Rust restart/Decision discharge closure is source-bound."""

    module = load_checker()
    copy_serve_lifecycle_production_fixture(tmp_path, module)

    errors = module._serve_lifecycle_production_source_fidelity_errors(tmp_path)

    assert not any(
        "v2_worker" in error
        or "v2_effects.rs" in error
        or "Decision/Serve" in error
        or "terminal-discharge" in error
        for error in errors
    ), errors
    assert (
        "_serve_lifecycle_production_source_fidelity_errors"
        in module.validate_ledger.__code__.co_names
    )


@pytest.mark.parametrize(
    "filename",
    (
        "v2_worker_reply_route_cases.rs",
        "v2_worker_backpressure_cases.rs",
        "v2_worker_serve_unsealed_cases.rs",
        "v2_worker_serve_decision_restart_cases.rs",
    ),
)
def test_worker_regression_include_source_seal_rejects_drift(
    tmp_path: Path,
    filename: str,
) -> None:
    """No extracted worker regression file can drift outside the source seal."""

    module = load_checker()
    copy_serve_lifecycle_production_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/tests"
        / filename
    )
    path.write_text(
        path.read_text(encoding="utf-8") + "\n// source-seal mutation\n",
        encoding="utf-8",
    )

    errors = module._worker_test_include_source_fidelity_errors(tmp_path)

    assert any(
        str(path) in error and "must match exact reviewed SHA-256" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    "filename",
    (
        "v2_worker_reply_route_cases.rs",
        "v2_worker_backpressure_cases.rs",
        "v2_worker_serve_unsealed_cases.rs",
        "v2_worker_serve_decision_restart_cases.rs",
    ),
)
def test_worker_regression_include_invocation_cannot_move_or_disappear(
    tmp_path: Path,
    filename: str,
) -> None:
    """Every extracted source remains directly and uniquely in the tests module."""

    module = load_checker()
    copy_serve_lifecycle_production_fixture(tmp_path, module)
    worker = tmp_path / "crates/iroha_core/src/sumeragi/v2_worker.rs"
    mutate_source_once(
        worker,
        f'    include!("tests/{filename}");',
        f'    // removed include!("tests/{filename}");',
    )

    errors = module._worker_test_include_source_fidelity_errors(tmp_path)

    assert any(
        filename in error and "must occur exactly once" in error
        for error in errors
    ), errors


_SERVE_TERMINAL_WORKER_CONTEXT = (("impl", "V2IoCommandQueue"),)
_SERVE_TERMINAL_EFFECT_CONTEXT = (
    (
        "impl",
        "<",
        "R",
        ":",
        "EffectRuntime",
        ">",
        "V2EffectExecutor",
        "<",
        "R",
        ">",
    ),
)


@pytest.mark.parametrize(
    ("relative", "item_name", "context", "old", "new"),
    (
        (
            Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
            "fully_authenticate_persisted_certified_serve_request",
            (),
            "verify_quorum_certificate_with_validator_pops(",
            "verify_quorum_certificate(",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
            "validate_persisted_certified_serve_terminal_outcomes",
            (),
            "local_validator != Some(tombstone.response_responder)",
            "false",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
            "discharge_restored_certified_serve_lifecycles",
            (),
            "if outcome_count != 1 {",
            "if outcome_count == 0 {",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
            "begin_decision_serve_reconciliation",
            _SERVE_TERMINAL_WORKER_CONTEXT,
            "state.decision_reconciliation_pending = true;",
            "state.decision_reconciliation_pending = false;",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
            "finish_decision_serve_reconciliation",
            _SERVE_TERMINAL_WORKER_CONTEXT,
            "&& !carrier_owned.contains(lifecycle_id)",
            "&& carrier_owned.contains(lifecycle_id)",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
            "convert_exact_terminal_retry_after_decision",
            _SERVE_TERMINAL_WORKER_CONTEXT,
            "if request.subject == decided_subject {",
            "if request.subject != decided_subject {",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
            "serve_lifecycle_has_live_ingress_carrier",
            _SERVE_TERMINAL_WORKER_CONTEXT,
            "&& reservation.carrier_ordinal.is_some()",
            "&& reservation.carrier_ordinal.is_none()",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
            "stage_selected_serve_rejection",
            _SERVE_TERMINAL_WORKER_CONTEXT,
            "state.durable_decided_subject != Some(decided_subject)",
            "state.durable_decided_subject == Some(decided_subject)",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
            "publish_serve_ingress_physical_drain",
            _SERVE_TERMINAL_WORKER_CONTEXT,
            "CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided)",
            "CertifiedServeNegativeOutcome::InvalidCertificate",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
            "serve_completion_delivery_ownership",
            _SERVE_TERMINAL_WORKER_CONTEXT,
            "return Ok(None);",
            "return Err(\"response escaped\".to_owned());",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
            "complete_serve_response",
            _SERVE_TERMINAL_WORKER_CONTEXT,
            "return Ok(false);",
            "return Ok(true);",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
            "acknowledge_serve_completion",
            _SERVE_TERMINAL_WORKER_CONTEXT,
            "tracked.state = V2IoServeState::Terminal;",
            "tracked.state = V2IoServeState::CompletionPending;",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_effects.rs"),
            "step",
            _SERVE_TERMINAL_EFFECT_CONTEXT,
            "if let Err(error) = services.begin_decision_serve_reconciliation() {",
            "if false {",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_effects.rs"),
            "step_pending_tip_recovery",
            _SERVE_TERMINAL_EFFECT_CONTEXT,
            "if let Err(error) = services.begin_decision_serve_reconciliation() {",
            "if false {",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/v2_effects.rs"),
            "finish_decision_serve_reconciliation",
            _SERVE_TERMINAL_EFFECT_CONTEXT,
            "proposal_round != decision_round",
            "proposal_round == decision_round",
        ),
    ),
)
def test_serve_terminal_discharge_production_mutations_fail_closed(
    tmp_path: Path,
    relative: Path,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
) -> None:
    """Every authoritative restart/Decision transition is token sealed."""

    module = load_checker()
    copy_serve_lifecycle_production_fixture(tmp_path, module)
    mutate_rust_item_source_in_context(
        module,
        tmp_path / relative,
        item_name,
        context,
        old,
        new,
    )

    errors = module._serve_lifecycle_production_source_fidelity_errors(tmp_path)

    assert any(
        item_name in error and "exact reviewed token digest" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    "test_name",
    (
        "prepared_serve_carrier_is_atomically_superseded_by_decision",
        "established_serve_owner_survives_decision_retry_carrier_retirement",
        "decision_serve_fence_rejects_conflicting_durable_subject_without_ordinals",
        "decision_serve_fence_rolls_back_failed_batch_and_converts_before_ordinals",
        "active_serve_completion_after_decision_publishes_negative_without_response",
        "completion_pending_serve_is_suppressed_after_decision_before_delivery",
        "production_restart_retires_raw_terminal_replay_waiter_without_resigning",
        "production_restart_atomically_supersedes_raw_terminal_replay_waiter",
        "production_restart_rejects_negative_tombstone_with_physical_retry_waiter",
        "same_height_foreign_context_is_rejected_before_every_serve_ordinal",
    ),
)
def test_serve_terminal_discharge_regressions_cannot_be_deleted(
    tmp_path: Path,
    test_name: str,
) -> None:
    """Every repaired worker defect retains its exact regression item."""

    module = load_checker()
    copy_serve_lifecycle_production_fixture(tmp_path, module)
    relative = Path(
        "crates/iroha_core/src/sumeragi/tests/"
        "v2_worker_serve_decision_restart_cases.rs"
    )
    mutate_rust_item_source_in_context(
        module,
        tmp_path / relative,
        test_name,
        (),
        f"fn {test_name}(",
        f"fn removed_{test_name}(",
    )

    errors = module._serve_lifecycle_production_source_fidelity_errors(tmp_path)

    assert any(
        f"named {test_name}; found 0" in error for error in errors
    ), errors


def test_serve_terminal_discharge_effect_regression_cannot_be_deleted(
    tmp_path: Path,
) -> None:
    """The effect-side durable-Decision loss regression remains mandatory."""

    module = load_checker()
    copy_serve_lifecycle_production_fixture(tmp_path, module)
    test_name = "decision_serve_fence_rejects_durable_decision_loss_without_reopening"
    relative = Path("crates/iroha_core/src/sumeragi/v2_effects.rs")
    mutate_rust_item_source_in_context(
        module,
        tmp_path / relative,
        test_name,
        (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),),
        f"fn {test_name}(",
        f"fn removed_{test_name}(",
    )

    errors = module._serve_lifecycle_production_source_fidelity_errors(tmp_path)

    assert any(
        f"named {test_name}; found 0" in error for error in errors
    ), errors

def test_liveness_ownership_mutation_source_seal_covers_exact_corpus(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    artifacts = module.LIVENESS_OWNERSHIP_MUTATION_FORMAL_ARTIFACTS

    assert len(artifacts) == 151
    assert sum(name.endswith(".tla") for name in artifacts) == 30
    assert sum(name.endswith("_fixed.cfg") for name in artifacts) == 30
    assert sum(name.endswith("_bug.cfg") for name in artifacts) == 91
    assert len(module.LIVENESS_OWNERSHIP_MUTATION_SHA256) == 152
    assert len(module.SERVE_RESTART_TERMINAL_DISCHARGE_BITS) == 25
    assert len(module.SERVE_RESTART_TERMINAL_DISCHARGE_MUTATIONS) == 25
    assert set(module.SERVE_RESTART_TERMINAL_DISCHARGE_BITS) == {
        false_bit
        for false_bit, _invariant in (
            module.SERVE_RESTART_TERMINAL_DISCHARGE_MUTATIONS.values()
        )
    }
    assert (
        "_liveness_ownership_mutation_source_fidelity_errors"
        in module.validate_ledger.__code__.co_names
    )
    assert module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    ) == []


@pytest.mark.parametrize(
    "artifact_name",
    (
        "SumeragiV2ExactResponseClaimLifecycleMutation.tla",
        "SumeragiV2ControlLivePredecessorMutation.tla",
        "control_live_predecessor_bug.cfg",
        "SumeragiV2ImportedCertificateTailMutation.tla",
        "imported_tc_tail_bug.cfg",
        "SumeragiV2TimeoutLifecycleStageClassifierMutation.tla",
        "timeout_lifecycle_stage_classifier_bug.cfg",
        "SumeragiV2PersistInstallTimeoutRootRetirementMutation.tla",
        "persist_install_timeout_root_retirement_bug.cfg",
        "exact_serve_frozen_predecessor_churn_bug.cfg",
        "adequate_leader_candidate_tombstone_fixed.cfg",
        "adequate_leader_candidate_terminal_discard_resurrection_bug.cfg",
        "adequate_leader_candidate_retired_chunk_decision_bug.cfg",
        "fixed_corridor_action_credit_per_child_recharge_bug.cfg",
        "SumeragiV2AdequateLeaderDeadlineAuthorityMutation.tla",
        "adequate_leader_deadline_authority_fixed.cfg",
        "adequate_leader_deadline_authority_omitted_roster_bound_bug.cfg",
        "SumeragiV2AdequateLeaderSelectedLifecycleEpisodeMutation.tla",
        "adequate_leader_selected_lifecycle_episode_fixed.cfg",
        (
            "adequate_leader_selected_lifecycle_episode_"
            "semantic_shortcut_bug.cfg"
        ),
        "SumeragiV2FixedCorridorReceiptAcquisitionMutation.tla",
        "fixed_corridor_receipt_acquisition_global_retire_bug.cfg",
        "SumeragiV2ProducerReplayCapacityMutation.tla",
        "producer_replay_capacity_replenishment_lasso_bug.cfg",
        "SumeragiV2OrdinaryIngressCarrierRebaseMutation.tla",
        "ordinary_ingress_carrier_rebase_minimum_bug.cfg",
        "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
        "serve_restart_terminal_discharge_fixed.cfg",
        "serve_restart_terminal_discharge_negative_terminal_sign_bug.cfg",
        "serve_restart_terminal_discharge_prepared_decision_drain_bug.cfg",
        "serve_restart_terminal_discharge_prefence_decision_rewrite_bug.cfg",
        "serve_restart_terminal_discharge_signer_authority_bug.cfg",
        "serve_restart_terminal_discharge_terminal_replay_resign_bug.cfg",
        "scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh",
    ),
)
def test_liveness_ownership_mutation_source_seal_rejects_stale_artifact(
    tmp_path: Path,
    artifact_name: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    path = (
        repo_root / artifact_name
        if "/" in artifact_name
        else formal_dir / artifact_name
    )
    path.write_text(
        path.read_text(encoding="utf-8") + "\n\\* stale mutation\n",
        encoding="utf-8",
    )

    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(
        str(path) in error and "must match exact reviewed SHA-256" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("artifact_name", "old", "new", "expected_error"),
    (
        (
            "SumeragiV2FixedCorridorPhysicalBudgetMutation.tla",
            "  2 * DeferredNormalCapacity\n",
            "  DeferredNormalCapacity\n",
            "DeferredDrainBudget must equal only",
        ),
        (
            "SumeragiV2FixedCorridorPhysicalBudgetMutation.tla",
            "    + 4 * DeferredDrainBudget\n",
            "    + DeferredDrainBudget\n",
            "CompletePhysicalWindowBudget must equal only",
        ),
        (
            "fixed_corridor_physical_budget_omitted_lane_cursor_bug.cfg",
            "INVARIANT PhysicalWindowBudgetCoversIndependentLanesAndCursorResets\n",
            "INVARIANT TypeInvariant\n",
            "physical-budget mutation config must equal only",
        ),
        (
            "SumeragiV2FixedCorridorActionCreditMutation.tla",
            "BeginTimeoutParentDebt == 69\n",
            "BeginTimeoutParentDebt == 68\n",
            "BeginTimeoutParentDebt must equal only",
        ),
        (
            "SumeragiV2FixedCorridorActionCreditMutation.tla",
            "       THEN ValidateBodyExactChildBatchDebt\n",
            "       THEN ValidateBodyParentDebt\n",
            "DispatchValidateBody must equal only",
        ),
        (
            "fixed_corridor_action_credit_per_child_recharge_bug.cfg",
            "INVARIANT ExactSuccessorHandoffStrictlyConsumesCumulativeActionDebt\n",
            "INVARIANT TypeInvariant\n",
            "action-credit mutation config must equal only",
        ),
        (
            "SumeragiV2ProposalPipelineBudgetMutation.tla",
            "  4 * ValidatorCount * SlotCapacity * PerSlotEpisode\n",
            "  4 * ValidatorCount * "
            "(PhysicalEpisodeBudget + SlotCapacity)\n",
            "ExactPipelineBudget must equal only",
        ),
        (
            "SumeragiV2FixedCorridorReceiptAcquisitionMutation.tla",
            "  /\\ leaderArmable'\n",
            "  /\\ ~leaderArmable'\n",
            "InstallSynchronizedLeaderView must equal only",
        ),
        (
            "SumeragiV2FixedCorridorReceiptAcquisitionMutation.tla",
            "  OtherReceipt(otherView, 0) \\in receipts\n",
            "  LeaderReceipt(otherView, 0) \\in receipts\n",
            "UnchangedLeaderKeyRetainsReceipt must equal only",
        ),
        (
            "fixed_corridor_receipt_acquisition_prestate_only_bug.cfg",
            "INVARIANT ReceiptAcquisitionAndRetention\n",
            "INVARIANT TypeInvariant\n",
            "receipt-acquisition mutation config must equal only",
        ),
        (
            "SumeragiV2AdequateLeaderDeadlineAuthorityMutation.tla",
            "  THEN deadline <= RosterDeadline\n",
            "  THEN deadline <= FabricatedReceiptDeadline\n",
            "deadline-authority mutation operator "
            "ReceiptOwnsFrozenRosterWindow must equal only",
        ),
        (
            "adequate_leader_deadline_authority_fixed.cfg",
            "CONSTANT EnforceRosterDeadlineAuthority = TRUE\n",
            "CONSTANT EnforceRosterDeadlineAuthority = FALSE\n",
            "deadline-authority mutation config must equal only",
        ),
        (
            "adequate_leader_deadline_authority_omitted_roster_bound_bug.cfg",
            "CONSTANT EnforceRosterDeadlineAuthority = FALSE\n",
            "CONSTANT EnforceRosterDeadlineAuthority = TRUE\n",
            "deadline-authority mutation config must equal only",
        ),
        (
            "SumeragiV2AdequateLeaderSelectedLifecycleEpisodeMutation.tla",
            "     ELSE ~LowerOccurrenceCoexists\n",
            "     ELSE TRUE\n",
            "selected-lifecycle mutation operator "
            "SelectedLifecycleEpisodeActive must equal only",
        ),
        (
            "adequate_leader_selected_lifecycle_episode_fixed.cfg",
            "CONSTANT PreserveSelectedLifecycleEpisode = TRUE\n",
            "CONSTANT PreserveSelectedLifecycleEpisode = FALSE\n",
            "selected-lifecycle mutation config must equal only",
        ),
        (
            "adequate_leader_selected_lifecycle_episode_semantic_shortcut_bug.cfg",
            "CONSTANT PreserveSelectedLifecycleEpisode = FALSE\n",
            "CONSTANT PreserveSelectedLifecycleEpisode = TRUE\n",
            "selected-lifecycle mutation config must equal only",
        ),
        (
            "SumeragiV2ProducerReplayCapacityMutation.tla",
            "  /\\ queueDepth + 1 + TargetReservationCharge <= Capacity\n",
            "  /\\ queueDepth < Capacity\n",
            "OrdinaryEnqueuePreservesReplayReservation must equal only",
        ),
        (
            "SumeragiV2ProducerReplayCapacityMutation.tla",
            '  /\\ targetStatus\' = "Queued"\n',
            '  /\\ targetStatus\' = "Dormant"\n',
            "ExactReplayAtomicallyConsumesReservation must equal only",
        ),
        (
            "producer_replay_capacity_replenishment_lasso_bug.cfg",
            "PROPERTY EventuallyExactTargetIsTombstoned\n",
            "INVARIANT TypeInvariant\n",
            "replay-capacity mutation config must equal only",
        ),
        (
            "SumeragiV2OrdinaryIngressCarrierRebaseMutation.tla",
            "       THEN Owner((CarrierNamed(\"Older\")).ordinal, \"StableOwner\")\n",
            "       THEN owner\n",
            "ordinary ingress carrier-rebase operator "
            "DrainOlderCompatible must equal only",
        ),
        (
            "SumeragiV2OrdinaryIngressCarrierRebaseMutation.tla",
            "  \\/ DrainOlderWithIdentityMutation\n",
            "",
            "ordinary ingress carrier-rebase operator Next must equal only",
        ),
        (
            "SumeragiV2OrdinaryIngressCarrierRebaseMutation.tla",
            "Spec == Init /\\ [][Next]_vars\n",
            "Spec == Init /\\ [][DrainNewer]_vars\n",
            "ordinary ingress carrier-rebase operator Spec must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "  ELSE IF ~admission.localSigner\n",
            "  ELSE IF FALSE /\\ ~admission.localSigner\n",
            "terminal-discharge operator ReconstructedOutcome must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "  ELSE IF SignOnlyResponseStartupTerminals THEN 0 ELSE 1\n",
            "  ELSE IF SignOnlyResponseStartupTerminals THEN 0 ELSE 0\n",
            "terminal-discharge operator "
            "StartupTerminalSignatureCost must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "  IF useCanonical \\/ Cardinality(records) = 1\n",
            "  IF useCanonical\n",
            "terminal-discharge operator SelectedAdmission must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            '   "InterruptedCrashed", "ResumedAfterSecond",\n',
            '   "InterruptedCrashed", "ResumedAfterSecondSkipped",\n',
            "terminal-discharge operator Phases must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            '           ELSE "ResumedAfterSecond"\n',
            '           ELSE "Complete"\n',
            "terminal-discharge operator "
            "ResumeSecondInterruptedEntry must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "  /\\ Cardinality(waiters) = 1\n",
            "  /\\ Cardinality(waiters) >= 0\n",
            "terminal-discharge operator TerminalWaiterAccepted must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            '          /\\ scenario \\in {"TerminalReplay", "TerminalDecision"}\n',
            '          /\\ scenario = "TerminalReplay"\n',
            "terminal-discharge operator HandleTerminalWaiter must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "  \\/ ResumeSecondInterruptedEntry\n",
            "",
            "terminal-discharge operator Next must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "Outcome(kind, decidedSubject) ==\n",
            "Outcome(decidedSubject, kind) ==\n",
            "terminal-discharge operator signature Outcome must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "Admission(\n"
            "    identity, family, view, lifecycleOrdinal, schedulerOrdinal,\n"
            "    certificateValid, localSigner, bodyState) ==\n",
            "Admission(\n"
            "    family, identity, view, lifecycleOrdinal, schedulerOrdinal,\n"
            "    certificateValid, localSigner, bodyState) ==\n",
            "terminal-discharge operator signature Admission must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "BeginStateAtCuts(\n"
            "    nextScenario, nextAdmissions, nextWaiters, nextTombstones,\n"
            "    nextResponseSource, nextSchedulerCut, nextPhysicalCut) ==\n",
            "BeginStateAtCuts(\n"
            "    nextResponseSource, nextAdmissions, nextWaiters, "
            "nextTombstones,\n"
            "    nextScenario, nextSchedulerCut, nextPhysicalCut) ==\n",
            "terminal-discharge operator signature "
            "BeginStateAtCuts must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "DischargeSelectedStartupOwner(selected, nextPhase, dropSuffix) "
            "==\n",
            "DischargeSelectedStartupOwner(selected, dropSuffix, nextPhase) "
            "==\n",
            "terminal-discharge operator signature "
            "DischargeSelectedStartupOwner must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "  [kind |-> kind, decidedSubject |-> decidedSubject]\n",
            '  [kind |-> "Response", decidedSubject |-> decidedSubject]\n',
            "terminal-discharge operator Outcome must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            '  Outcome("SupersededByDurableDecision", subject)\n',
            '  Outcome("Response", subject)\n',
            "terminal-discharge operator DecisionOutcome must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "   lifecycleOrdinal |-> lifecycleOrdinal,\n"
            "   schedulerOrdinal |-> schedulerOrdinal,\n"
            "   certificateValid |-> certificateValid,\n",
            "   lifecycleOrdinal |-> lifecycleOrdinal,\n"
            "   schedulerOrdinal |-> 1,\n"
            "   certificateValid |-> certificateValid,\n",
            "terminal-discharge operator Admission must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "   ownerIdentity |-> ownerIdentity,\n",
            "   ownerIdentity |-> identity,\n",
            "terminal-discharge operator OwnedWaiter must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "    identity, identity, lifecycleOrdinal, schedulerOrdinal,\n",
            "    identity, \"A\", lifecycleOrdinal, schedulerOrdinal,\n",
            "terminal-discharge operator Waiter must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "   outcome |-> outcome,\n",
            "   outcome |-> ResponseOutcome,\n",
            "terminal-discharge operator Tombstone must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            '    "R", "family-R", 4, 4, DecisionOutcome("Decision-B"), {})\n',
            '    "R", "family-R", 4, 4, ResponseOutcome, {})\n',
            "terminal-discharge operator DecisionTombstoneR must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            'LiveDecisionRetryWaiter == Waiter("R", 4, 10, 10)\n',
            'LiveDecisionRetryWaiter == Waiter("R", 4, 11, 10)\n',
            "terminal-discharge operator "
            "LiveDecisionRetryWaiter must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            '   "LiveDecisionRetry", "LiveDecisionPreFenceCarrier",\n'
            '   "LiveDecisionPreFencePreparedCarrier",\n'
            '   "TerminalMismatchCorrupt", "TerminalOrphanCorrupt",\n',
            '   "LiveDecisionRetry", "LiveDecisionPreFenceCarrier",\n'
            '   "LiveDecisionPreFencePreparedCarrierRelabeled",\n'
            '   "TerminalMismatchCorrupt", "TerminalOrphanCorrupt",\n',
            "terminal-discharge operator Scenarios must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "vars ==\n"
            "  <<phase, scenario, admissions, waiters, tombstones, "
            "dischargeOrder,\n"
            "    producerRuns, nextLifecycleOrdinal, nextSchedulerOrdinal,\n"
            "    nextPhysicalOrdinal, signatureCount, emittedOutputs, fanout,\n"
            "    responseSource, responseOutcome, transportPassed, "
            "lifecycleAdmitted>>\n",
            "vars ==\n"
            "  <<phase, scenario, admissions, waiters, tombstones, "
            "dischargeOrder,\n"
            "    producerRuns, nextLifecycleOrdinal, nextSchedulerOrdinal,\n"
            "    nextPhysicalOrdinal, emittedOutputs, fanout,\n"
            "    responseSource, responseOutcome, transportPassed, "
            "lifecycleAdmitted>>\n",
            "terminal-discharge operator vars must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            '  /\\ scenario = "None"\n',
            '  /\\ scenario = "Union"\n',
            "terminal-discharge operator Init must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "  /\\ phase = \"ChooseScenario\"\n"
            "  /\\ phase' = \"Pending\"\n",
            "  /\\ FALSE\n"
            "  /\\ phase' = \"Pending\"\n",
            "terminal-discharge operator BeginStateAtCuts must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "    {PreFencePreparedAdmissionR},\n",
            "    {},\n",
            "terminal-discharge operator "
            "BeginLiveDecisionPreFencePreparedCarrier must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "  /\\ IF CompletePreparedCarrierDecisionDrain\n",
            "  /\\ IF FALSE\n",
            "terminal-discharge operator "
            "DrainPreparedCarrierAfterDecision must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            '    => /\\ phase # "PolicyRejected"\n',
            "    => /\\ TRUE\n",
            "terminal-discharge operator "
            "PreparedCarrierDecisionDrainIsAtomicAndOrdinalStable "
            "must equal only",
        ),
        (
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
            "  \\/ DrainPreparedCarrierAfterDecision\n",
            "",
            "terminal-discharge operator Next must equal only",
        ),
        (
            "ordinary_ingress_carrier_rebase_minimum_bug.cfg",
            "  RebaseToMinimum = FALSE\n",
            "  RebaseToMinimum = TRUE\n",
            "ordinary ingress carrier-rebase mutation config must equal only",
        ),
        (
            "serve_restart_terminal_discharge_fixed.cfg",
            "  ConvertLiveResponseBeforeOrdinal = TRUE\n",
            "  ConvertLiveResponseBeforeOrdinal = FALSE\n",
            "terminal-discharge mutation config must equal only",
        ),
        (
            "serve_restart_terminal_discharge_prefence_decision_rewrite_bug.cfg",
            "  PreservePreFenceResponseUntilCheckedDrain = FALSE\n",
            "  PreservePreFenceResponseUntilCheckedDrain = TRUE\n",
            "terminal-discharge mutation config must equal only",
        ),
        (
            "serve_restart_terminal_discharge_terminal_replay_resign_bug.cfg",
            "  AvoidTerminalReplayResigning = FALSE\n",
            "  AvoidTerminalReplayResigning = TRUE\n",
            "terminal-discharge mutation config must equal only",
        ),
        (
            "serve_restart_terminal_discharge_terminal_replay_resign_bug.cfg",
            "INVARIANT TypeInvariant\n",
            "",
            "terminal-discharge mutation config must equal only",
        ),
        (
            "serve_restart_terminal_discharge_negative_terminal_sign_bug.cfg",
            "  SignOnlyResponseStartupTerminals = FALSE\n",
            "  SignOnlyResponseStartupTerminals = TRUE\n",
            "terminal-discharge mutation config must equal only",
        ),
        (
            "serve_restart_terminal_discharge_prepared_decision_drain_bug.cfg",
            "  CompletePreparedCarrierDecisionDrain = FALSE\n",
            "  CompletePreparedCarrierDecisionDrain = TRUE\n",
            "terminal-discharge mutation config must equal only",
        ),
        (
            "serve_restart_terminal_discharge_prepared_decision_drain_bug.cfg",
            "INVARIANT TypeInvariant\n",
            "",
            "terminal-discharge mutation config must equal only",
        ),
        (
            "serve_restart_terminal_discharge_signer_authority_bug.cfg",
            "INVARIANT OnlyFrozenQcSignersCanRespond\n",
            "INVARIANT TypeInvariant\n",
            "terminal-discharge mutation config must equal only",
        ),
        (
            "serve_restart_terminal_discharge_body_fail_open_bug.cfg",
            "CHECK_DEADLOCK FALSE\n",
            "CHECK_DEADLOCK TRUE\n",
            "terminal-discharge mutation config must equal only",
        ),
    ),
)
def test_liveness_ownership_exact_mutations_fail_after_reseal(
    tmp_path: Path,
    artifact_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path,
        module,
    )
    path = formal_dir / artifact_name
    source = path.read_text(encoding="utf-8")
    assert source.count(old) == 1
    path.write_text(source.replace(old, new, 1), encoding="utf-8")
    module.LIVENESS_OWNERSHIP_MUTATION_SHA256[artifact_name] = hashlib.sha256(
        path.read_bytes()
    ).hexdigest()

    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir,
        repo_root,
    )

    assert any(expected_error in error for error in errors), errors


def test_liveness_ownership_mutation_source_seal_rejects_inventory_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    missing = (
        formal_dir
        / "serve_restart_terminal_discharge_raw_context_gate_bug.cfg"
    )
    missing.unlink()
    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )
    assert any(
        str(missing) in error
        and "missing liveness-ownership mutation artifact" in error
        for error in errors
    ), errors

    shutil.copy2(module.FORMAL_DIR / missing.name, missing)
    extra = formal_dir / "adequate_leader_candidate_unreviewed_bug.cfg"
    extra.write_text("SPECIFICATION Spec\n", encoding="utf-8")
    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )
    assert any(
        str(extra) in error
        and "extra liveness-ownership mutation artifact" in error
        for error in errors
    ), errors

    extra.unlink()
    symlink = formal_dir / "exact_ingress_ticket_priority_fixed.cfg"
    target = formal_dir / "exact_ingress_ticket_priority_fixed.target"
    symlink.rename(target)
    symlink.symlink_to(target.name)
    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )
    assert any(
        str(symlink) in error and "artifact must be a regular file" in error
        for error in errors
    ), errors


def test_liveness_ownership_terminal_discharge_artifact_rename_fails_closed(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    original = (
        formal_dir
        / "serve_restart_terminal_discharge_receiver_close_bug.cfg"
    )
    renamed = (
        formal_dir
        / "serve_restart_terminal_discharge_receiver_closed_bug.cfg"
    )
    original.rename(renamed)

    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(
        str(original) in error
        and "missing liveness-ownership mutation artifact" in error
        for error in errors
    ), errors
    assert any(
        str(renamed) in error
        and "extra liveness-ownership mutation artifact" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new", "expected_error"),
    (
        (
            "AsyncControlServiceAdmissionStartsOrReplaces",
            "        /\\ AsyncControlServiceRecordForItem(item).consumed\n",
            "",
            "strict-newer replacement requires consumed predecessor",
        ),
        (
            "CanAdmitIngressItem",
            "  /\\ ~AsyncControlServiceAdmissionBlockedByLivePredecessor(item)\n",
            "",
            "full live-predecessor and ordinary-carrier ingress admission gate",
        ),
    ),
)
def test_liveness_ownership_source_seal_rejects_live_predecessor_gate_weakening(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new", "expected_error"),
    (
        (
            "AsyncTimeoutLifecycleCandidateRetiredBy",
            "  /\\ candidate.causalOrigin.height = height\n",
            "",
            "exact same-node/context/height timeout-root retirement boundary",
        ),
        (
            "CandidateAdmissionCoalesced",
            "    \\/ AsyncTimeoutLifecycleCandidateRetiredThroughInstall(candidate)\n",
            "",
            "installed timeout-root retransmission coalescing",
        ),
        (
            "RemoveNextNodeCommandAfterDispatch",
            '        IF command.kind = "PersistInstallTC"\n',
            "        IF FALSE\n",
            "Runtime FIFO timeout-root retirement",
        ),
        (
            "AppendCausalSuccessors",
            '        IF command.kind = "PersistInstallTC"\n',
            "        IF FALSE\n",
            "causal timeout-root retirement before install successors",
        ),
        (
            "AsyncIoTimeoutLifecycleRetirementTransition",
            "                  ![node] = AsyncTimeoutLifecycleSetAfterInstall(\n",
            "                  ![node] = @\n",
            "atomic install retirement and PersistDecision Serve conversion",
        ),
        (
            "SerializedRuntimeStep",
            "  /\\ AsyncIoTimeoutLifecycleRetirementTransition(node)\n",
            "  /\\ UNCHANGED AsyncIoVars\n",
            "ordinary Runtime path applies timeout-root I/O retirement",
        ),
        (
            "SerializedRuntimePrecedesServeIngressStep",
            "  /\\ AsyncIoTimeoutLifecycleRetirementTransition(node)\n",
            "  /\\ UNCHANGED AsyncIoVars\n",
            "older Runtime interleave applies timeout-root I/O retirement",
        ),
        (
            "AsyncControlServiceStateAfterTimeoutRetirement",
            "               THEN command.view\n",
            "               THEN @\n",
            "durable monotone per-height timeout retirement high-watermark",
        ),
    ),
)
def test_liveness_ownership_source_seal_rejects_timeout_root_retirement_weakening(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new", "expected_error"),
    (
        (
            "AsyncPersistDecisionCommandThisStep",
            "       /\\ PersistDecision(request)\n",
            "",
            "PersistDecision exact current-step classifier",
        ),
        (
            "AsyncPersistDecisionCommandThisStep",
            '  /\\ command.kind = "PersistDecision"\n',
            '  /\\ command.kind = "PersistInstallTC"\n',
            "PersistDecision exact current-step classifier",
        ),
        (
            "AsyncPersistDecisionCommandsThisStep",
            "  {command \\in AsyncCandidateSet:\n",
            "  {command \\in QueuedCandidates:\n",
            "PersistDecision current-step command set",
        ),
        (
            "AsyncPersistDecisionCommandsForNodeThisStep",
            "     command.node = node}\n",
            "     TRUE}\n",
            "PersistDecision node-local current-step command set",
        ),
        (
            "AsyncServePersistedDecisionHasCheckedDrainOwner",
            "  AsyncServeIngressAdmissionOwned(\n"
            "    tombstone.node, tombstone.identity)\n",
            "  FALSE\n",
            "PersistDecision checked-drain owner classifier",
        ),
        (
            "AsyncServeTombstoneAfterPersistedDecision",
            "        /\\ ~AsyncServePersistedDecisionHasCheckedDrainOwner(tombstone)\n",
            "",
            "PersistDecision exact Response conversion gate",
        ),
        (
            "AsyncServeTombstoneAfterPersistedDecision",
            '           IN /\\ request.kind = "CertifiedRequest"\n',
            '           IN /\\ request.kind = "BlockCreated"\n',
            "PersistDecision exact Response conversion gate",
        ),
        (
            "AsyncServeTombstonesAfterPersistedDecision",
            "     tombstone \\in asyncServeTombstones}\n",
            "     tombstone \\in {}}\n",
            "PersistDecision atomic Serve tombstone projection",
        ),
        (
            "AsyncIoTimeoutLifecycleRetirementTransition",
            "  ELSE IF AsyncPersistDecisionCommandsForNodeThisStep(node) # {}\n",
            "  ELSE IF FALSE\n",
            "atomic install retirement and PersistDecision Serve conversion",
        ),
    ),
)
def test_liveness_ownership_rejects_persist_decision_operator_weakening(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("theorem", "expected_error"),
    (
        (
            "PersistDecisionConvertsIncompatibleResponseBeforeRetryOrdinal",
            "PersistDecision pre-retry conversion theorem",
        ),
        (
            "PersistDecisionPreservesPreFenceResponseUntilCheckedDrain",
            "PersistDecision pre-fence checked-drain preservation theorem",
        ),
    ),
)
def test_liveness_ownership_requires_persist_decision_theorems(
    tmp_path: Path,
    theorem: str,
    expected_error: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    old = f"THEOREM {theorem} =="
    new = f"THEOREM Weakened{theorem} =="
    assert source.count(old) == 1
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("theorem", "old", "new", "expected_error"),
    (
        (
            "PersistDecisionConvertsIncompatibleResponseBeforeRetryOrdinal",
            "       /\\ ~AsyncServePersistedDecisionHasCheckedDrainOwner(tombstone)\n",
            "",
            "PersistDecision pre-retry conversion theorem",
        ),
        (
            "PersistDecisionPreservesPreFenceResponseUntilCheckedDrain",
            "       /\\ AsyncServeIngressAdmissionOwned(\n"
            "            tombstone.node, tombstone.identity)\n",
            "       /\\ TRUE\n",
            "PersistDecision pre-fence checked-drain preservation theorem",
        ),
    ),
)
def test_liveness_ownership_rejects_persist_decision_theorem_weakening(
    tmp_path: Path,
    theorem: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(source, theorem, old, new),
        encoding="utf-8",
    )

    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(expected_error in error for error in errors), errors


def test_liveness_ownership_source_seal_requires_blocked_packet_action_theorem(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    old = "THEOREM AsyncControlServiceBlockedNewerPacketCannotPassIngress =="
    new = "THEOREM WeakenedBlockedNewerPacketCanPassIngress =="
    assert source.count(old) == 1
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any("blocked newer packet action theorem" in error for error in errors), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new", "expected_error"),
    (
        (
            "CandidateConsumerCurrent",
            "     THEN candidate.height = height\n",
            "     THEN /\\ candidate.consumerView = nodeView[candidate.node]\n"
            "          /\\ candidate.consumerGeneration = generation[candidate.node]\n",
            "height/context-stable imported certificate consumer",
        ),
        (
            "ImportedCommitDecisionTail",
            '       \\in {"CommitQC", "CommitCertificateResponse"}\n',
            '       = "CommitQC"\n',
            "authenticated CommitQC import-tail scope",
        ),
        (
            "ImportedTimeoutCertificateTail",
            '  /\\ candidate.evidence.kind = "TimeoutCertificate"\n',
            "",
            "authenticated timeout-certificate import-tail scope",
        ),
    ),
)
def test_liveness_ownership_source_seal_rejects_imported_tail_weakening(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("theorem", "expected_error"),
    (
        (
            "AsyncNextNodeCommandOwnsOldestLifecycleOrdinal",
            "runtime FIFO selector theorem for the oldest lifecycle",
        ),
        (
            "AsyncNextDeferredCommandOwnsOldestLifecycleWithoutHandoff",
            "Busy-deferred selector theorem for the oldest lifecycle",
        ),
        (
            "AsyncDeferredHandoffRetainsExactSelectedLifecycle",
            "Busy handoff theorem retaining the exact predecessor",
        ),
    ),
)
def test_liveness_ownership_source_seal_requires_runtime_predecessor_theorems(
    tmp_path: Path,
    theorem: str,
    expected_error: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    old = f"THEOREM {theorem} =="
    new = f"THEOREM Weakened{theorem} =="
    assert source.count(old) == 1
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "           /\\ AsyncPersistDecisionCommandsForNodeThisStep(node) = {}\n",
            "",
            "paired PersistInstall/PersistDecision stutter premise",
        ),
        (
            "             AsyncPersistDecisionCommandsForNodeThisStep,\n",
            "",
            "paired PersistInstall/PersistDecision DEF dependency",
        ),
        (
            "THEOREM ExactRuntimeContinuationReplayPreservesProgressOwnership ==",
            "THEOREM WeakenedRuntimeContinuationReplayProgressOwnership ==",
            "exact Runtime continuation replay progress-ownership theorem",
        ),
        (
            "THEOREM ExactRuntimeContinuationReplayPreservesProgressOwnership ==",
            "(* THEOREM "
            "ExactRuntimeContinuationReplayPreservesProgressOwnership == *)\n"
            "THEOREM WeakenedRuntimeContinuationReplayProgressOwnership ==",
            "exact Runtime continuation replay progress-ownership theorem",
        ),
    ),
)
def test_liveness_ownership_rejects_progress_dependency_weakening(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    path = formal_dir / "SumeragiV2AsyncProgressOwnershipProofs.tla"
    source = path.read_text(encoding="utf-8")
    assert source.count(old) == 1
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(expected_error in error for error in errors), errors


def test_liveness_ownership_runner_rejects_mutation_helper_deletion(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    runner = repo_root / module.LIVENESS_OWNERSHIP_MUTATION_RUNNER
    source = runner.read_text(encoding="utf-8")
    helper_start = source.index("assert_mutation_failure_contract() {")
    helper_end = source.index("\n\nrun_case() {", helper_start)
    runner.write_text(
        source[:helper_start] + source[helper_end + 2 :],
        encoding="utf-8",
    )
    module.LIVENESS_OWNERSHIP_MUTATION_SHA256[
        module.LIVENESS_OWNERSHIP_MUTATION_RUNNER
    ] = hashlib.sha256(runner.read_bytes()).hexdigest()

    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(
        "must retain one exact mutation-failure helper before run_case"
        in error
        for error in errors
    ), errors




@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            'source "$TLC_RESULT_CONTRACT"\n',
            "",
            "shared TLC result contract import",
        ),
        (
            '[[ "$expected_primary" =~ ^Error:\\ Invariant\\ '
            '.+\\ is\\ violated\\.$ ]]',
            '[[ "$expected_primary" =~ ^Error: ]]',
            "status-12 named-invariant diagnostic classifier",
        ),
        (
            '    sumeragi_v2_tlc_assert_nonzero_state_space "$label" "$log"\n',
            "",
            "shared nonzero state-space assertion",
        ),
        (
            '    sumeragi_v2_tlc_assert_terminal "$label" "$log"\n',
            "",
            "shared terminal-marker assertion",
        ),
        (
            'run_case "$label" "$model" "$config" 12 \\\n',
            'run_case "$label" "$model" "$config" 0 \\\n',
            "TLC invariant-violation status check",
        ),
        (
            'run_case "$label" "$model" "$config" 13 \\\n',
            'run_case "$label" "$model" "$config" 0 \\\n',
            "TLC temporal-violation status check",
        ),
        (
            'for case_spec in "${mutation_cases[@]}"; do\n',
            'for case_spec in "${mutation_cases[@]:0:0}"; do\n',
            "exact invariant-mutation execution loop",
        ),
        (
            '  "serve-certified-request-raw-context-gate|'
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla|"
            "serve_restart_terminal_discharge_raw_context_gate_bug.cfg|"
            'RawContextGateSeparatesLifecycleAuthority"\n',
            "",
            "runner census must equal exactly 30 repaired / 88 "
            "invariant-mutation / 3 temporal-mutation cases",
        ),
        (
            '  "serve-restart-terminal-replay-resign|'
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla|"
            "serve_restart_terminal_discharge_terminal_replay_resign_bug.cfg|"
            'TerminalReplayAndDecisionConversionDoNotResignOrMintOrdinal"\n',
            "",
            "runner census must equal exactly 30 repaired / 88 "
            "invariant-mutation / 3 temporal-mutation cases",
        ),
        (
            '  "serve-live-prefence-prepared-decision-drain|'
            "SumeragiV2ServeRestartTerminalDischargeMutation.tla|"
            "serve_restart_terminal_discharge_prepared_decision_drain_bug.cfg|"
            'PreparedCarrierDecisionDrainIsAtomicAndOrdinalStable"\n',
            "",
            "runner census must equal exactly 30 repaired / 88 "
            "invariant-mutation / 3 temporal-mutation cases",
        ),
        (
            '  "ordinary-ingress-carrier-rebase|'
            "SumeragiV2OrdinaryIngressCarrierRebaseMutation.tla|"
            'ordinary_ingress_carrier_rebase_fixed.cfg"\n',
            '  "ordinary-ingress-carrier-rebase|'
            "SumeragiV2OrdinaryIngressCarrierRebaseMutation.tla|"
            'ordinary_ingress_carrier_rebase_fixed.cfg"\n'
            '  "ordinary-ingress-carrier-rebase|'
            "SumeragiV2OrdinaryIngressCarrierRebaseMutation.tla|"
            'ordinary_ingress_carrier_rebase_fixed.cfg"\n',
            "runner census must equal exactly 30 repaired / 88 "
            "invariant-mutation / 3 temporal-mutation cases",
        ),
        (
            "|exact_response_claim_duplicate_bug.cfg|"
            "OneLogicalChargePerWaiterFamily",
            "|exact_response_claim_duplicate_bug.cfg|TypeInvariant",
            "failing case matrix must equal",
        ),
        (
            "|adequate_leader_candidate_terminal_discard_resurrection_bug.cfg|"
            "TerminalDiscardCannotBeReadmitted",
            "|adequate_leader_candidate_terminal_discard_resurrection_bug.cfg|"
            "TypeInvariant",
            "failing case matrix must equal",
        ),
        (
            "|exact_installed_tc_view_only_bug.cfg|ExactInstalledTcAuthority",
            "|exact_installed_tc_view_only_bug.cfg|TypeInvariant",
            "failing case matrix must equal",
        ),
        (
            "|adequate_leader_candidate_retired_chunk_view_bug.cfg|"
            "RetiredChunkStageCannotReadmitAfterViewAdvance",
            "|adequate_leader_candidate_retired_chunk_view_bug.cfg|"
            "TypeInvariant",
            "failing case matrix must equal",
        ),
        (
            "|fixed_corridor_receipt_acquisition_prestate_only_bug.cfg|"
            "ReceiptAcquisitionAndRetention",
            "|fixed_corridor_receipt_acquisition_prestate_only_bug.cfg|"
            "TypeInvariant",
            "failing case matrix must equal",
        ),
        (
            "|serve_restart_terminal_discharge_signer_authority_bug.cfg|"
            "OnlyFrozenQcSignersCanRespond",
            "|serve_restart_terminal_discharge_signer_authority_bug.cfg|"
            "TypeInvariant",
            "failing case matrix must equal",
        ),
        (
            "|serve_restart_terminal_discharge_negative_terminal_sign_bug.cfg|"
            "UnsealedRestartResponsesSignExactlyOnce",
            "|serve_restart_terminal_discharge_negative_terminal_sign_bug.cfg|"
            "TypeInvariant",
            "failing case matrix must equal",
        ),
        (
            "|serve_restart_terminal_discharge_prepared_decision_drain_bug.cfg|"
            "PreparedCarrierDecisionDrainIsAtomicAndOrdinalStable",
            "|serve_restart_terminal_discharge_prepared_decision_drain_bug.cfg|"
            "TypeInvariant",
            "failing case matrix must equal",
        ),
        (
            "|SumeragiV2AdequateLeaderDeadlineAuthorityMutation.tla|"
            "adequate_leader_deadline_authority_fixed.cfg",
            "|SumeragiV2AdequateLeaderDeadlineAuthorityMutation.tla|"
            "adequate_leader_deadline_authority_omitted_roster_bound_bug.cfg",
            "repaired case matrix must equal",
        ),
        (
            "|adequate_leader_deadline_authority_omitted_roster_bound_bug.cfg|"
            "NoPrematureExit",
            "|adequate_leader_deadline_authority_omitted_roster_bound_bug.cfg|"
            "TypeInvariant",
            "failing case matrix must equal",
        ),
        (
            "|SumeragiV2AdequateLeaderSelectedLifecycleEpisodeMutation.tla|"
            "adequate_leader_selected_lifecycle_episode_fixed.cfg",
            "|SumeragiV2AdequateLeaderSelectedLifecycleEpisodeMutation.tla|"
            "adequate_leader_selected_lifecycle_episode_"
            "semantic_shortcut_bug.cfg",
            "repaired case matrix must equal",
        ),
        (
            "|adequate_leader_selected_lifecycle_episode_"
            "semantic_shortcut_bug.cfg|"
            "SelectedLifecycleEpisodeOrPhysicalDescent",
            "|adequate_leader_selected_lifecycle_episode_"
            "semantic_shortcut_bug.cfg|TypeInvariant",
            "failing case matrix must equal",
        ),
        (
            "producer-replay-capacity-replenishment-lasso|"
            "SumeragiV2ProducerReplayCapacityMutation.tla|"
            "producer_replay_capacity_replenishment_lasso_bug.cfg",
            "producer-replay-capacity-replenishment-lasso|"
            "SumeragiV2ProducerReplayCapacityMutation.tla|"
            "producer_replay_capacity_fixed.cfg",
            "temporal liveness mutation matrix must equal",
        ),
        (
            'echo "[tlc] all ${#mutation_cases[@]} invariant and '
            "${#temporal_mutation_cases[@]} temporal liveness-ownership "
            "mutations produced their exact named counterexamples; all "
            '${#fixed_cases[@]} repaired models passed"',
            "all liveness-ownership cases passed",
            "exact mutation completion marker",
        ),
    ),
)
def test_liveness_ownership_runner_rejects_status_property_and_marker_weakening(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    runner = repo_root / module.LIVENESS_OWNERSHIP_MUTATION_RUNNER
    source = runner.read_text(encoding="utf-8")
    assert source.count(old) == 1
    runner.write_text(source.replace(old, new, 1), encoding="utf-8")
    module.LIVENESS_OWNERSHIP_MUTATION_SHA256[
        module.LIVENESS_OWNERSHIP_MUTATION_RUNNER
    ] = hashlib.sha256(runner.read_bytes()).hexdigest()

    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(expected_error in error for error in errors), errors


def test_liveness_ownership_source_seal_rejects_skipped_ci_invocation(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_liveness_ownership_mutation_fixture(
        tmp_path, module
    )
    ci_gate = repo_root / "ci" / "check_sumeragi_formal.sh"
    invocation = (
        "bash scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh\n"
    )
    source = ci_gate.read_text(encoding="utf-8")
    assert source.count(invocation) == 1
    ci_gate.write_text(source.replace(invocation, "", 1), encoding="utf-8")

    errors = module._liveness_ownership_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(
        "must invoke the sealed liveness-ownership mutation runner exactly once"
        in error
        for error in errors
    ), errors
