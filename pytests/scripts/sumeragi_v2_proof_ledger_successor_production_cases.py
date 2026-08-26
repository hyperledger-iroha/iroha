# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

SUCCESSOR_PRODUCTION_SOURCE_FIXTURE_FILES = (
    "crates/iroha_core/src/sumeragi/v2_runner.rs",
    "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
    "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
    "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_runner_authority.rs",
    "crates/iroha_core/src/sumeragi/v2_runner/finalized_output_rollover.rs",
    "crates/iroha_core/src/sumeragi/mod.rs",
    "crates/iroha_core/src/sumeragi/status.rs",
    "crates/iroha_core/src/sumeragi/v2_first_release_recovery.rs",
    "crates/iroha_core/src/sumeragi/v2.rs",
    "crates/iroha_core/src/sumeragi/v2_runtime.rs",
    "crates/iroha_core/src/sumeragi/v2_block_sync.rs",
    "crates/iroha_core/src/sumeragi/v2_effects.rs",
    "crates/iroha_core/src/sumeragi/v2_recovery.rs",
    "crates/iroha_core/src/sumeragi/v2_context.rs",
    "crates/iroha_core/src/sumeragi/v2_apply.rs",
    "crates/iroha_core/src/sumeragi/v2_body_store.rs",
    "crates/iroha_core/src/sumeragi/safety_wal.rs",
    "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_tests.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_pending_kura.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
    "crates/iroha_core/src/sumeragi/v2_certified_serve_payload_store.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_open.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_open_output_recovery.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator_support.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_schema.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_pre_admission.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_impl.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_execution_impl.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_execution.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_sidecar.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_wal_recovery.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_ingress_position.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_body_pipeline_transition.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_concrete_admission.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_projection.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_replay_authority.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_replay_authority_live_wal.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_validate_sidecar.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_preactivation.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
    "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
    "crates/iroha_core/src/sumeragi/v2_apply_tests.rs",
    "crates/iroha_core/src/sumeragi/v2_transport.rs",
    "crates/iroha_core/src/sumeragi/v2_worker.rs",
    "crates/iroha_core/src/sumeragi/v2_worker_completion.rs",
    "crates/iroha_core/src/sumeragi/v2_worker_io_execution.rs",
    "crates/iroha_core/src/sumeragi/v2_worker_exact_output.rs",
    "crates/iroha_core/src/sumeragi/v2_worker_services.rs",
    "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
    "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
    "crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs",
    "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_height_driver.rs",
    "crates/iroha_core/src/sumeragi/v2_runner/preactivation_ingress.rs",
    "crates/iroha_core/src/sumeragi/v2_runner/decided_lane_recovery.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_recovered_wal.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_operations.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs",
    "crates/iroha_core/src/sumeragi/tests/v2_adapter_main_03.rs",
    "crates/iroha_core/src/sumeragi/tests/v2_adapter_04_wal_recovery.rs",
    "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
    "crates/iroha_core/src/sumeragi/tests/v2_effects_main_04.rs",
    "crates/iroha_core/src/sumeragi/tests/v2_runner_unsealed_00.rs",
    "crates/iroha_core/src/sumeragi/tests/v2_worker_main_01.rs",
    "crates/iroha_core/src/sumeragi/tests/v2_worker_lifecycle_capacity_cases.rs",
    "crates/iroha_core/src/sumeragi/tests/v2_worker_recovered_lifecycle_output_cases.rs",
    "crates/iroha_core/src/snapshot.rs",
    "crates/iroha_core/src/state.rs",
    "crates/iroha_core/src/kura.rs",
    "scripts/run_sumeragi_v2_release_gates.sh",
)
assert len(SUCCESSOR_PRODUCTION_SOURCE_FIXTURE_FILES) == len(
    set(SUCCESSOR_PRODUCTION_SOURCE_FIXTURE_FILES)
) == 76


LIFECYCLE_DECISION_APPLY_LINEAGE_SOURCE_FILES = (
    "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_impl.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_schema.rs",
    "crates/iroha_core/src/sumeragi/v2.rs",
    "crates/iroha_core/src/sumeragi/v2_effects.rs",
    "crates/iroha_core/src/sumeragi/v2_worker.rs",
    "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
    "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
    "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
)
assert len(LIFECYCLE_DECISION_APPLY_LINEAGE_SOURCE_FILES) == len(
    set(LIFECYCLE_DECISION_APPLY_LINEAGE_SOURCE_FILES)
) == 10


LIFECYCLE_DECISION_APPLY_LINEAGE_MUTATIONS = (
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "pub(in crate::sumeragi) enum LifecycleDecisionApplyLineageV1 {",
        "    Live,\n    /// Apply reconstructed",
        "    Recovered,\n    /// Apply reconstructed",
        "closed live/recovered lineage",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_impl.rs",
        "pub(super) fn attest_ready_lifecycle_decision_apply(",
        "LifecycleDecisionApplyLineageV1::Live,",
        "LifecycleDecisionApplyLineageV1::Recovered,",
        "classifier must distinguish both exact undispatched carriers",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_impl.rs",
        "pub(super) fn prepare_ready_live_decision_apply_reconciliation(",
        "dispatch_key.lineage() == LifecycleDecisionApplyLineageV1::Recovered",
        "dispatch_key.lineage() == LifecycleDecisionApplyLineageV1::Live",
        "reject recovered substitution",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_impl.rs",
        "pub(super) fn prepare_lifecycle_decision_apply_dispatch(",
        "LifecycleDecisionApplyLineageV1::Recovered,",
        "LifecycleDecisionApplyLineageV1::Live,",
        "lineage-aware Apply dispatch",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_tail_impl.rs",
        "pub(super) fn prepare_lifecycle_decision_apply_terminal_transition(",
        "LifecycleDecisionApplyLineageV1::Live,",
        "LifecycleDecisionApplyLineageV1::Recovered,",
        "exact live carrier and lineage",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "impl LifecycleDecisionApplyDispatchKeyV1 {",
        "self.context == context.id()",
        "true",
        "every isolated carrier-coordinate substitution",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "impl LifecycleDecisionApplyDispatchKeyV1 {",
        "&& self.height == context.height()",
        "&& true",
        "every isolated carrier-coordinate substitution",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "pub(in crate::sumeragi) fn project_live_decision_apply_completion(",
        "LifecycleDecisionApplyLineageV1::Live,",
        "LifecycleDecisionApplyLineageV1::Recovered,",
        "shared worker corridor with live lineage",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn project_lifecycle_decision_apply_completion(",
        "key.matches_carrier(context, address, installed_digest, lineage)",
        "key.matches_height_context(&artifact.height_context)",
        "exact lineage-tagged carrier",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "let lineage_owner_is_exact = match authority.lineage() {",
        "LifecycleDecisionApplyLineageV1::Live => self",
        "LifecycleDecisionApplyLineageV1::Recovered => self",
        "distinguish exact live ownership from recovered non-substitution",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "let lineage_owner_is_exact = match authority.lineage() {",
        "LifecycleDecisionApplyLineageV1::Recovered => {",
        "LifecycleDecisionApplyLineageV1::Live => {",
        "distinguish exact live ownership from recovered non-substitution",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "pub(in crate::sumeragi) fn prepare_lifecycle_decision_apply_completion(",
        "|| !lineage_owner_is_exact",
        "|| false",
        "reject an authority-only lineage substitution",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn dispatch_completion_with_runner_debt_and_required_ordinal(",
        "executor.exactly_owns_live_lifecycle_decision_apply(&authority)",
        "false",
        "live reconciliation, complete Apply census, and neutral worker publication",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn dispatch_completion_with_runner_debt_and_required_ordinal(",
        ".map_err(ProductionCompletionDispatchErrorV1::LiveApplyReconciliation)?;",
        ".map_err(ProductionCompletionDispatchErrorV1::Service)?;",
        "neutral Apply reservation must join executor evidence before one-shot queue publication",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "pub(crate) enum ProductionSchedulerInputsError {",
        "InvalidLifecycleDecisionApplyCarrier",
        "InvalidRecoveredDecisionApplyCarrier",
        "scheduler Apply carrier failure must use the lifecycle-neutral class",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_schema.rs",
        "pub(super) fn from_authenticated(",
        "lifecycle_decision_apply_attestation",
        "recovered_apply_attestation",
        "scheduler schema Apply corridor",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker.rs",
        "pub(in crate::sumeragi) fn select_apply(",
        "LifecycleCompletionPreparedCapacityV1::Apply {",
        "LifecycleCompletionPreparedCapacityV1::Sign {",
        "consume only the frozen exact row",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn settle_lifecycle_decision_apply_completion_owner(",
        "persist_exact_staged_successor(&staged)",
        "persist_inexact_staged_successor(&staged)",
        "neutral lifecycle Apply durable terminal settlement",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn settle_lifecycle_decision_apply_completion_owner(",
        "settle_applied_lifecycle_decision_apply_completion(owner, executor, completion)",
        "settle_pending_kura_applied_decision_apply_completion(owner, executor, completion)",
        "terminal Apply corridor retains retired recovered-only aliases",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn drive_lifecycle_decision_apply_deferred(",
        "dispatch_next_lifecycle_decision_apply_sidecar_request",
        "dispatch_next_recovered_apply_sidecar_request",
        "deferred lifecycle Apply must use the lifecycle-neutral sidecar dispatcher",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "fn new_with_output_guard_and_transport_inner(",
        "lifecycle_decision_apply_sidecar_waits",
        "recovered_apply_sidecar_waits",
        "distinct neutral lifecycle Apply wait and rejection owners",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "pub(crate) struct V2LaneWorkAdapter {",
        "rejected_lifecycle_decision_apply_sidecars",
        "rejected_recovered_apply_sidecars",
        "lifecycle-neutral Apply sidecar rejection owner",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "pub(in crate::sumeragi) fn dispatch_next_lifecycle_decision_apply_sidecar_request(",
        "dispatch_next_lifecycle_decision_apply_sidecar_request",
        "dispatch_next_recovered_apply_sidecar_request",
        "lifecycle-neutral Apply sidecar dispatcher",
    ),
)
assert len(LIFECYCLE_DECISION_APPLY_LINEAGE_MUTATIONS) == len(
    set(LIFECYCLE_DECISION_APPLY_LINEAGE_MUTATIONS)
) == 23


@pytest.mark.parametrize(
    ("relative_path", "region_marker", "old", "new", "error_fragment"),
    LIFECYCLE_DECISION_APPLY_LINEAGE_MUTATIONS,
)
def test_lifecycle_decision_apply_lineage_mutations_fail_closed(
    tmp_path: Path,
    relative_path: str,
    region_marker: str,
    old: str,
    new: str,
    error_fragment: str,
) -> None:
    module = load_checker()
    for source_name in LIFECYCLE_DECISION_APPLY_LINEAGE_SOURCE_FILES:
        destination = tmp_path / source_name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / source_name, destination)
    copy_reviewed_rust_include_components(tmp_path)

    baseline_errors = module._lifecycle_decision_apply_lineage_source_fidelity_errors(
        tmp_path
    )
    assert baseline_errors == [], baseline_errors

    path = tmp_path / relative_path
    source = path.read_text(encoding="utf-8")
    region_start = source.find(region_marker)
    assert region_start >= 0
    mutation = source.find(old, region_start)
    assert mutation >= 0
    path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )
    errors = module._lifecycle_decision_apply_lineage_source_fidelity_errors(tmp_path)
    assert any(error_fragment in error for error in errors), errors


COLD_READY_VALIDATE_RETRY_MUTATIONS = (
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "impl RecoveredDurableValidateRetryOwnerV1",
        "fn bind_validated_marker(",
        "fn inspect_validated_marker(",
        "move-only cold Ready Validate retry owner",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "impl RecoveredDurableValidateRetryCensusV1",
        "self.owners.get_mut(&key)",
        "self.owners.get(&key)",
        "opaque complete cold Ready Validate retry census",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "impl RecoveredDurableValidateRetryCensusV1",
        "for owner in self.owners.into_values()",
        "for owner in self.owners.values()",
        "opaque complete cold Ready Validate retry census",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runtime_durable_recovery_pending.rs",
        "fn bind_validated_marker_commitment(",
        "self.authority_ceiling_commitment = Some(commitment)",
        "self.authority_ceiling_commitment = None",
        "closed cold Ready Validate retry binding and frontier",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runtime_durable_recovery_pending.rs",
        "fn project_retry(",
        "incoming_tag != frontier_tag",
        "incoming_tag != recovered_tag",
        "exact cold Ready Validate retry binding",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runtime_durable_recovery_pending.rs",
        "fn project_retry(",
        ".zip(incoming_commitment)",
        ".zip(None)",
        "exact cold Ready Validate retry binding",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runtime_durable_recovery_pending.rs",
        "fn project_retry(",
        ".or(incoming_commitment)",
        ".or(None)",
        "exact cold Ready Validate retry binding",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runtime_durable_recovery_pending.rs",
        "fn project_retry(",
        "let recovered_statement = self.pending.candidate_statement.ok_or_else(|| {",
        "let _ = incoming.exact_pending_adapter_effect_binding(effect);\n"
        "        let recovered_statement = self.pending.candidate_statement.ok_or_else(|| {",
        "origin-neutral cold Ready Validate retry binding",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runtime_durable_recovery_pending.rs",
        "fn project_retry(",
        "effect: effect.clone()",
        "effect: recovered_effect.clone()",
        "exact cold Ready Validate retry binding",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects_lifecycle_admission_settlement.rs",
        "fn project_retry(",
        "owner: Arc::clone(owner)",
        "owner: Arc::new((**owner).clone())",
        "non-substitutable live and recovered Validate retry projection",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects_lifecycle_admission_settlement.rs",
        "pub(in crate::sumeragi) fn absorb(",
        "frontier: owner.initial_retry_frontier()",
        "frontier: None",
        "atomic cold Ready Validate retry installation",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "pub(in crate::sumeragi) fn open_with_body_store(",
        "mut recovered_validate_retry_census: RecoveredDurableValidateRetryCensusV1",
        "recovered_validate_retry_census: RecoveredDurableValidateRetryCensusV1",
        "owner-exact cold Ready Validate marker deferral",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "pub(in crate::sumeragi) fn open_with_body_store(",
        ".classify_and_bind_validated_marker(*key, validated_receipt)",
        ".exactly_defers_validated_marker(*key, validated_receipt)",
        "owner-exact cold Ready Validate marker deferral",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "fn retain_effect_batch_at_frontier(",
        "retained_validate_retry_seals.insert((*round, *subject), projected.seal)",
        "retained_validate_retry_seals.remove(&(*round, *subject))",
        "exact cold Ready Validate retry stutter",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn launch(",
        "recovered_validate_retry_census,",
        "&recovered_validate_retry_census,",
        "cold Ready Validate census launch installation",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runtime_durable_recovery_pending.rs",
        "fn project_commitment_ceiling(",
        "expected != commitment",
        "expected == commitment",
        "pure recovered Validate durable commitment projection",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects_lifecycle_admission_settlement.rs",
        "fn project_recovered_commitment_ceiling(",
        "Self::Live { .. } => Ok(None)",
        "Self::Live { .. } => unreachable!()",
        "lineage-preserving recovered Validate durable commitment join",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "fn record_lifecycle_validated_body(",
        "seal.project_recovered_commitment_ceiling(validated.execution_commitment())",
        "Ok(None)",
        "pre-mutation recovered Validate marker commitment join",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "fn reconcile_decision_work<S: V2EffectServices>(",
        "seal.project_recovered_commitment_ceiling(decision_commitment)",
        "Ok(None)",
        "Decision-scoped cold Validate retry cleanup",
    ),
)
assert len(COLD_READY_VALIDATE_RETRY_MUTATIONS) == len(
    set(COLD_READY_VALIDATE_RETRY_MUTATIONS)
) == 19


@pytest.mark.parametrize(
    ("relative_path", "region_marker", "old", "new", "error_fragment"),
    COLD_READY_VALIDATE_RETRY_MUTATIONS,
)
def test_cold_ready_validate_retry_mutations_fail_closed(
    tmp_path: Path,
    relative_path: str,
    region_marker: str,
    old: str,
    new: str,
    error_fragment: str,
) -> None:
    module = load_checker()
    for source_name in SUCCESSOR_PRODUCTION_SOURCE_FIXTURE_FILES:
        destination = tmp_path / source_name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / source_name, destination)
    copy_reviewed_rust_include_components(tmp_path)

    baseline_errors = module._successor_recovery_source_fidelity_errors(tmp_path)
    assert baseline_errors == [], baseline_errors

    path = tmp_path / relative_path
    source = path.read_text(encoding="utf-8")
    region_start = source.find(region_marker)
    assert region_start >= 0
    mutation = source.find(old, region_start)
    assert mutation >= 0
    path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )
    errors = module._successor_recovery_source_fidelity_errors(tmp_path)
    assert any(error_fragment in error for error in errors), errors


def test_successor_run_inner_parser_rejects_neighbor_lookalike(
    tmp_path: Path,
) -> None:
    """Successor checks may consume only the parsed `run_inner` item."""

    module = load_checker()
    for relative in SUCCESSOR_PRODUCTION_SOURCE_FIXTURE_FILES:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    copy_reviewed_rust_include_components(tmp_path)

    runner = tmp_path / "crates/iroha_core/src/sumeragi/v2_runner.rs"
    source = runner.read_text(encoding="utf-8")
    run_inner_items = module.rust_items(source, "run_inner")
    assert len(run_inner_items) == 1
    run_inner = run_inner_items[0]
    owner_binding = (
        "let pending_successor_activation = recovered_successor_activation\n"
        "            .map(|authority| {\n"
        "                PendingSuccessorActivation::recovered(authority, &common_config.key_pair)\n"
        "            })\n"
        "            .transpose()?;"
    )
    assert run_inner.source.count(owner_binding) == 1
    weakened = run_inner.source.replace(
        owner_binding,
        "let pending_successor_activation = None;",
        1,
    )
    neighboring_lookalike = (
        "\n\nfn parser_only_run_inner_lookalike() {\n"
        f"    {owner_binding}\n"
        "    let _ = &mut pending_successor_activation;\n"
        "}\n"
    )
    assert source.count(run_inner.source) == 1
    runner.write_text(
        source.replace(
            run_inner.source,
            weakened + neighboring_lookalike,
            1,
        ),
        encoding="utf-8",
    )

    errors = module._successor_production_source_fidelity_errors(tmp_path)
    assert any(
        "run_inner recovery ownership omits production refinement tokens"
        in error
        for error in errors
    ), errors


SUCCESSOR_PRODUCTION_SOURCE_MAPPING_MUTATIONS = (
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn project_proposal_exact_output_authority(",
        "!matches!(",
        "matches!(",
        "affine recovered Proposal exact-output projection must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "fn prepare_recovered_lifecycle_sign_completion_with_body(",
        ".prepare_proposal_prepare_wal_body_lookup(",
        ".project_broadcast_and_sign_body_lookup(",
        "single-preview recovered next-Vote body executor join must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn prepare_proposal_prepare_wal_body_lookup(",
        "next_reducer.step(persisted_event.clone())",
        "self.next_reducer.step(persisted_event.clone())",
        "pre-WAL initial Proposal continuation must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn append_recovered_lifecycle_proposal_prepare_wal(",
        "self.adapter.wal.append(&encoded_wal_payload)",
        "self.adapter.wal.recovered_records().last()",
        "fail-stop initial Proposal WAL append must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn append_recovered_lifecycle_proposal_prepare_wal(",
        "permit.authorizes(",
        "permit.authorizes_for_test(",
        "fail-stop initial Proposal WAL append must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn append_recovered_lifecycle_proposal_prepare_wal(",
        "permit.cross_wal_attempt_boundary()",
        "drop(permit)",
        "fail-stop initial Proposal WAL append must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services.rs",
        "fn prepare_wal_append_permit(",
        "&& !self.wal_append.attempted",
        "|| !self.wal_append.attempted",
        "armed Proposal reservation lends WAL authority without parts must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services.rs",
        "fn abort_before_publication(",
        "assert!(\n            !self.wal_append.attempted",
        "debug_assert!(\n            !self.wal_append.attempted",
        "retry-safe recovered Proposal reservation abort must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn settle_recovered_lifecycle_proposal_prepare_wal(",
        "output.prepare_wal_append_permit()",
        "None::<super::v2_worker::RecoveredLifecycleProposalPrepareWalAppendPermitV1<'_>>",
        "restart-closed initial Proposal PrepareIntent settlement must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn settle_recovered_lifecycle_proposal_prepare_wal(",
        "preview\n            .append_recovered_lifecycle_proposal_prepare_wal(wal_permit)",
        "drop(preview)",
        "restart-closed initial Proposal PrepareIntent settlement must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn prepare_next_recovered_decision_fetch_ingress_selector(",
        "PreparedLifecycleIngressIoTarget::RecoveredDecisionFetchBodyPersistence",
        "PreparedLifecycleIngressIoTarget::CertifiedFetchBodyPersistence",
        "queue-owned recovered Decision Fetch selector must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn prepare_next_recovered_decision_fetch_ingress_selector(",
        "v2_ingress_head_can_drain(occurrence.inbound(), self, terminal_subject)",
        "true",
        "queue-owned recovered Decision Fetch selector must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/mod.rs",
        "fn select_fair_v2_ingress_candidate<T>(",
        "for dependency_pass in [false, true]",
        "for dependency_pass in [true, false]",
        "shared strict-then-dependency fair selection must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "fn v2_ingress_head_can_drain<R: EffectRuntime>(",
        "fn v2_ingress_head_can_drain<R: EffectRuntime>(",
        "fn v2_ingress_head_can_drain<R>(",
        "shared pure ingress drain predicate omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_effects_main_04.rs",
        "fn recovered_decision_fetch_fences_later_ordinary_body_coordinates()",
        "a later recovered response cannot leapfrog the ordinary fair winner",
        "a later recovered response may leapfrog the ordinary fair winner",
        "queue-owned recovered Decision Fetch selector behavior must retain each executable literal exactly once",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn activate_with(",
        "self.executor\n            .arm_live_clocks(clock_activation, now)",
        "let _ = now",
        "one-shot lifecycle activation transaction must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn activate_with(",
        "activation.complete();",
        "drop(activation);",
        "one-shot lifecycle activation transaction must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_runner_authority.rs",
        "struct ProductionLifecycleRunnerActivationV1",
        "Arc::ptr_eq(&self.block_ingress, launched_ingress)",
        "true",
        "runner-owned lifecycle activation authority must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_runner_authority.rs",
        "struct ProductionLifecycleRunnerActivationV1",
        "fn current_height(",
        "pub(in crate::sumeragi) fn current_height(",
        "runner-owned lifecycle activation status classes must use the opaque checked-transition gate",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_runner_authority.rs",
        "struct ProductionLifecycleCompleteTipRunnerActivationV1",
        "retirement.authorizes_successor_status(&successor)",
        "true",
        "runner-owned CompleteTip lifecycle activation authority must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_runner_authority.rs",
        "struct ProductionLifecycleCompleteTipRunnerActivationV1",
        "fn mint_for_recovered_runner(",
        "pub(in crate::sumeragi) fn mint_for_recovered_runner(",
        "runner-owned CompleteTip lifecycle activation seal must use the opaque checked-transition gate",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn with_runner_runtime<R>(",
        "_runner: &mut super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1",
        "_runner: &mut ()",
        "borrow-bound activated lifecycle owner omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "struct ActivatedProductionLifecycleV1",
        "launched: LaunchedProductionLifecycleV1",
        "pub(in crate::sumeragi) launched: LaunchedProductionLifecycleV1",
        "opaque activated lifecycle owner must use the opaque checked-transition gate",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "struct ProductionLifecycleActivatedRunnerAuthorityV1",
        "self.ingress_ready.store(false, Ordering::Release);",
        "let _ = &self.ingress_ready;",
        "activated runner readiness retirement must contain",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_certified_serve_payload_store.rs",
        "fn authenticate_current_for_lifecycle_retirement(",
        "self.validate_authenticated_cut(&authenticated)?;",
        "let _ = &authenticated;",
        "live Serve retirement directory authentication must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn refresh_live_serve_retirement_cut(",
        "exactly_covers_finalization_work(&self.coordinator)",
        "exactly_covers_recovered_ready_work(&self.coordinator)",
        "launched live Serve retirement refresh must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_open.rs",
        "fn authenticate_live_finalization_serve_census(",
        "receipt.exactly_matches_pending(payload.request())",
        "true",
        "live finalization Serve ledger/admission-wait join must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_wal_recovery.rs",
        "fn matches_current_finalization_record(",
        "WaitSource::Recovery(digest)",
        "WaitSource::External(digest)",
        "volatile refanned Broadcast finalization state omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn recovered_broadcast_refanout_ranks_exact_pair_before_unrelated_ready_sign()",
        "finalization accepts the exact volatile refanout wait after its next Sign retires",
        "finalization accepts an inexact volatile refanout wait after its next Sign retires",
        "volatile refanned Broadcast finalization behavior must retain each executable literal exactly once",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_impl.rs",
        "fn exact_optional_recovered_wal_authority(",
        "!matches!(record.state, super::LifecycleState::Waiting(_))",
        "matches!(record.state, super::LifecycleState::Waiting(_))",
        "finalization recovered Broadcast pair link omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "fn paired_next_sign_matches_terminal_record(",
        "installed_digest == next_digest",
        "installed_digest != next_digest",
        "retained recovered Broadcast terminal next-Sign link omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn run_lifecycle_active_height(",
        "if !apply_terminal_settled && (!ready_to_finish || producer_turn.is_some()) {",
        "if !ready_to_finish || producer_turn.is_some() {",
        "runner lifecycle finalization preflight must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn run_lifecycle_active_height(",
        "let finalization_ready =\n            ready_to_finish && activated.ready_for_finalized_rollover(&mut active_runner);",
        "let finalization_ready = ready_to_finish;",
        "runner lifecycle finalization preflight must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_impl.rs",
        "fn exact_optional_recovered_wal_authority(",
        "carrier.pairs_exact_next_sign(next_sign, next_sign_digest)",
        "true",
        "finalization recovered Broadcast pair link omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn into_finalized_rollover(",
        "launched\n            .leader_wire_ingress_binding\n            .retire()",
        "Ok(())",
        "activated lifecycle finalization must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "impl FinalizedProductionLifecycleRolloverV1",
        "refresh_live_serve_retirement_cut(&services, &retired_ingress)",
        "refresh_live_serve_retirement_cut_for_test()",
        "typed lifecycle finalized-output rollover must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "impl ProductionLifecyclePostOutputHandoffV1",
        "publication.consume_owners(registry)",
        "drop(publication)",
        "post-output lifecycle-store retirement must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs",
        "fn persist_exact_finalization_successor(",
        "store.load()? != retired",
        "false",
        "opaque all-row finalization publication must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs",
        "fn persist_exact_finalization_successor(",
        "coordinator: self",
        "coordinator: LifecycleCoordinator::from_recovery",
        "opaque all-row finalization publication must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "impl ProductionLifecycleCleanupReadyV1",
        "self.services.allow_clean_shutdown()",
        "let _ = &self.services",
        "cleanup-ready lifecycle service teardown must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
        "TransactionBuilder::new_genesis(",
        "TransactionBuilder::new(",
        "production lifecycle finalization behavior must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
        "Algorithm::Ed25519",
        "Algorithm::BlsNormal",
        "production lifecycle finalization behavior must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
        "lifecycle_run_inner::finalize_lifecycle_height(",
        "lifecycle_run_inner::removed_finalize_lifecycle_height(",
        "production lifecycle finalization behavior must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
        "services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));",
        "let _ = services;",
        "production lifecycle finalization behavior must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
        "super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedServeQueued,",
        "super::super::v2_lifecycle_coordinator::ProductionLifecycleIngressSelectionV1::CertifiedServeCapacityPending,",
        "production lifecycle finalization behavior must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
        ") => {}\n                    ProductionLifecycleIngressTurnV1::PassThrough(runner)",
        ") => { consume_prepared_ordinary_ingress_turn(); }\n                    ProductionLifecycleIngressTurnV1::PassThrough(runner)",
        "direct lifecycle Certified-Serve dispatch must use the opaque checked-transition gate",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
        "!selected.restart_required()",
        "false",
        "production lifecycle finalization behavior must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
        ".claim_producer_turn_for_local_proposal(&mut serve_runner)",
        ".claim_unreviewed_producer_turn_for_local_proposal(&mut serve_runner)",
        "production lifecycle finalization behavior must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
        ".settle_producer_turn_after_local_proposal(&mut serve_runner, attempted_producer)",
        ".settle_unreviewed_producer_turn_after_local_proposal(&mut serve_runner, attempted_producer)",
        "production lifecycle finalization behavior must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
        ".retain_merge_sidecars_for_global_view(",
        ".retain_merge_sidecars_for_global_view_removed(",
        "production lifecycle finalization behavior must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "pub(in crate::sumeragi) fn project_proposal_exact_output_authority(",
        "RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign\n                    | RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal",
        "RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign",
        "affine recovered Proposal exact-output projection must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_impl.rs",
        "fn install_recovered_wal_decision_store<'registry>(",
        "fn install_recovered_wal_decision_store<'registry>(",
        "fn install_recovered_wal_decision_store(",
        "dedicated recovered Decision Store registry install omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "pub(in crate::sumeragi) struct ProductionLifecyclePreActivationRunnerBorrowV1",
        "_seal: ProductionLifecyclePreActivationRunnerBorrowSealV1,",
        "pub(in crate::sumeragi) _seal: ProductionLifecyclePreActivationRunnerBorrowSealV1,",
        "sealed lifecycle preactivation runner borrow exposes forbidden surface",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "impl ProductionLifecyclePreActivationRunnerBorrowV1 {",
        "fn mint_for_recovered_runner() -> Self {",
        "pub(in crate::sumeragi) fn mint_for_recovered_runner() -> Self {",
        "sealed lifecycle preactivation runner borrow exposes forbidden surface",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "fn bind_recovered_local_proposal(",
        "if !local_proposal.state.is_pristine() {",
        "if false {",
        "sealed lifecycle preactivation runner borrow omits",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "fn bind_recovered_local_proposal(",
        "LocalProposalState::from_recovered_lifecycle_attempt(true, directive)",
        "LocalProposalState::from_recovered_lifecycle_attempt(false, directive)",
        "sealed lifecycle preactivation runner borrow omits",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runtime.rs",
        "fn lifecycle_live_clocks_are_armed(&self) -> bool {",
        "self.clocks_armed",
        "false",
        "preactivation live-clock state oracle",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "fn lifecycle_live_clocks_are_unarmed(&self) -> bool {",
        "!self.runtime.lifecycle_live_clocks_are_armed()",
        "true",
        "preactivation executor live-clock state oracle",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_preactivation.rs",
        "fn with_runner_setup_transaction<R, E>(",
        "drop(initial_admission);",
        "let _ = &initial_admission;",
        "fail-stop closed-ingress lifecycle preactivation setup",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_preactivation.rs",
        "impl Drop for ProductionLifecyclePreActivationFailStopScopeV1 {",
        "self.output_guard.close_admission_for_restart();",
        "let _ = &self.output_guard;",
        "lifecycle preactivation non-permit fail-stop scope omits",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_preactivation.rs",
        "fn with_runner_setup_transaction<R, E>(",
        "operation(&mut self.executor, &mut self.services)?",
        "operation(&mut self.executor, &mut self.services).expect(\"setup\")",
        "fail-stop closed-ingress lifecycle preactivation setup",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_preactivation.rs",
        "fn with_runner_setup_transaction<R, E>(",
        "setup.complete();",
        "drop(setup);",
        "fail-stop closed-ingress lifecycle preactivation setup",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_preactivation.rs",
        "fn with_runner_setup_transaction<R, E>(",
        "let final_admission = output_guard",
        "let unchecked_final_admission = output_guard",
        "fail-stop closed-ingress lifecycle preactivation setup",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
        ".with_runner_setup(&mut setup_runner, |executor, services| {",
        ".without_runner_setup(&mut setup_runner, |executor, services| {",
        "production-shaped closed-ingress preactivation setup behavior",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_tests.rs",
        "fn preactivation_fail_stop_scope_closes_on_drop_and_disarms_on_complete()",
        "assert!(dropped_guard.restart_required());",
        "assert!(!dropped_guard.restart_required());",
        "preactivation non-permit fail-stop behavior",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn composite_recovered_completion_dispatches_one_ranked_sign_and_preserves_the_other()",
        "ProductionCompletionDispatchV1::SignQueued { ordinal: paired }",
        "ProductionCompletionDispatchV1::CapacityUnavailable",
        "composite recovered Completion Sign selection behavior",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn composite_recovered_completion_dispatches_one_ranked_sign_and_preserves_the_other()",
        "assert_eq!(state.records[&unrelated].state, LifecycleState::Ready);",
        "assert_eq!(state.records[&unrelated].state, LifecycleState::Claimed(lease));",
        "composite recovered Completion Sign selection behavior",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn composite_recovered_completion_capacity_unavailable_claims_no_ready_sign()",
        "planner_io.saturate_consensus_prefix(&services);",
        "planner_io.release_all_predecessors();",
        "composite recovered Completion capacity-unavailable behavior",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_worker_lifecycle_capacity_cases.rs",
        "fn lifecycle_completion_capacity_census_selects_once_and_drops_fail_stop()",
        "output.abort_before_claim();",
        "drop(output);",
        "lifecycle Completion worker Fetch ownership behavior",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
        "fn capture_lifecycle_completion_capacity_census(",
        "let fanout = self.recovered_decision_fetch_fanout(&owner)?;",
        "let fanout = None;",
        "joint recovered Completion physical-corridor census",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
        "fn capture_lifecycle_completion_capacity_census(",
        "let pending = self.lock_pending_exact_output()?;",
        "let pending = self.lock_pending_exact_output_removed()?;",
        "joint recovered Completion physical-corridor census",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn dispatch_completion_with_runner_debt_and_required_ordinal(",
        "for ordinal in &exact_ready {",
        "for ordinal in exact_ready.iter().take(1) {",
        "all-row recovered Completion authentication and selection",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn dispatch_completion_with_runner_debt_and_required_ordinal(",
        "capture_lifecycle_completion_capacity_census(probes)",
        "capture_lifecycle_completion_capacity_census_removed(probes)",
        "all-row recovered Completion authentication and selection",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn dispatch_completion_with_runner_debt_and_required_ordinal(",
        "authenticated_ready_row_with_physical_capacity(",
        "authenticated_ready_row(",
        "all-row recovered Completion authentication and selection",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn dispatch_completion_with_runner_debt_and_required_ordinal(",
        ".select_fetch(ordinal)",
        ".select_sign(ordinal)",
        "all-row recovered Completion authentication and selection",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "fn drive_ready_completion_turn_with_required_ordinal<'cursor>(",
        "owner.dispatch_completion_with_runner_debt(",
        "owner.dispatch_lifecycle_decision_apply_with_runner_debt(",
        "fresh lifecycle Completion Ready-work dispatch",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn composite_recovered_completion_capacity_unavailable_claims_no_ready_sign()",
        "fn composite_recovered_completion_capacity_unavailable_claims_no_ready_sign()",
        "fn composite_recovered_completion_capacity_unavailable_claims_ready_sign()",
        "require exactly one real Rust/Verus function item",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
        "current certified Serve rejection must own ingress",
        "current certified Serve rejection bypassed ingress",
        "real-cursor ordinary ingress regression omits",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04_wal_recovery.rs",
        "fn bls_decision_fetch_repairs_and_coalesces_without_rewrite()",
        "mixed_sign_ordinal > first_summary.0",
        "mixed_sign_ordinal < first_summary.0",
        "genuine recovered Fetch composite-dispatch behavior",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04_wal_recovery.rs",
        "fn bls_decision_fetch_repairs_and_coalesces_without_rewrite()",
        "ProductionCompletionDispatchV1::SignQueued",
        "ProductionCompletionDispatchV1::FetchDispatched",
        "genuine recovered Fetch composite-dispatch behavior",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04_wal_recovery.rs",
        "fn bls_decision_fetch_repairs_and_coalesces_without_rewrite()",
        "output_guard.close_admission_for_restart();",
        "drop(output_guard);",
        "genuine recovered Fetch composite-dispatch behavior",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04_wal_recovery.rs",
        "fn bls_decision_fetch_repairs_and_coalesces_without_rewrite()",
        "dispatch_completion_for_test(",
        "dispatch_recovered_decision_fetch(",
        "genuine recovered Fetch composite-dispatch behavior",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04_wal_recovery.rs",
        "fn bls_decision_fetch_repairs_and_coalesces_without_rewrite()",
        "ProductionCompletionDispatchV1::FetchDispatched",
        "ProductionCompletionDispatchV1::CapacityUnavailable",
        "genuine recovered Fetch composite-dispatch behavior",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
        "backpressured certified Serve remains lifecycle-owned",
        "backpressured certified Serve lost lifecycle ownership",
        "real-cursor ordinary ingress regression omits",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
        "ProductionPreparedCertifiedServeTestSettlementV1::Rejected(reason)",
        "ProductionPreparedCertifiedServeTestSettlementV1::Rejected(String::new())",
        "real-cursor ordinary ingress regression omits",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "pub(in crate::sumeragi) enum ProductionLifecycleCompletionTurnV1<'cursor> {",
        "PassThrough(LifecycleCurrentRunnerTurn<'cursor>)",
        "PassThrough(LifecycleRunnerRankSnapshot)",
        "borrow-bound lifecycle turn outcomes",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "fn settle_parked_recovered_sign_completion(",
        "self.settle_recovered_lifecycle_vote_broadcast_and_sign()",
        "self.settle_recovered_lifecycle_proposal_broadcast_and_sign()",
        "unified recovered Sign settlement routing",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::Service {",
        "ProductionLifecycleIngressSelectionV1::RestartRequired",
        "ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchPreparationRetry",
        "recovered Fetch Phase-A service failure",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "pub(in crate::sumeragi) fn drive_ingress_turn<'cursor>(",
        "crate::sumeragi::v2_effects::v2_ingress_head_can_drain(",
        "true",
        "ordinary/recovered ingress owner order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ingress_position.rs",
        "pub(super) fn capture_next_ingress_turn_cut(",
        "let service_guard = self.service_lock.lock();",
        "let service_guard = self.state.lock();",
        "queue-owned fair winner capture",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ingress_position.rs",
        "pub(super) fn capture_next_ingress_turn_cut(",
        "select_fair_v2_ingress_candidate(",
        "select_next_admissible_ordinal(",
        "queue-owned fair winner capture",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ingress_position.rs",
        "pub(super) fn dequeue_exact_retaining(",
        "self.selected_physical_ordinal,",
        "1,",
        "exact queue-owned physical dequeue",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs",
        "fn prepare_current_certified_serve_pre_admission(",
        "CurrentCertifiedServePreAdmissionV1::AuthenticatedNegative {",
        "CurrentCertifiedServePreAdmissionV1::Negative {",
        "shared current Serve transport/authentication classifier",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/decided_lane_recovery.rs",
        "fn prepare_decided_lane_recovery_ingress(",
        "if request.round.height == active_height {\n"
        "        return DecidedLaneRecoveryIngressPreparation::CurrentServe;\n"
        "    }",
        "if request.round.height == active_height {\n"
        "        return DecidedLaneRecoveryIngressPreparation::LeaderWireRetire;\n"
        "    }",
        "terminal recovery classifies exact current Serve for guarded service",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/decided_lane_recovery.rs",
        "fn authorize_decided_lane_recovery_drain(",
        "DecidedLaneRecoveryIngressPreparation::CurrentServe => {\n"
        "            DecidedLaneRecoveryDrainAuthorization::CurrentServe\n"
        "        }",
        "DecidedLaneRecoveryIngressPreparation::CurrentServe => {\n"
        "            DecidedLaneRecoveryDrainAuthorization::LeaderWireRetire\n"
        "        }",
        "terminal recovery authorizes exact current-Serve service",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/decided_lane_recovery.rs",
        "fn drain_decided_lane_recovery_ingress(",
        "commit_decided_lane_recovery_drain(authorization, &mut committer)?;",
        "commit_decided_lane_recovery_drain(\n"
        "        DecidedLaneRecoveryDrainAuthorization::LeaderWireRetire,\n"
        "        &mut committer,\n"
        "    )?;",
        "live terminal drain directly serves authorized current recovery",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/decided_lane_recovery.rs",
        "fn authorize_decided_lane_recovery_drain(",
        ") -> DecidedLaneRecoveryDrainAuthorization {\n    match preparation {",
        ") -> DecidedLaneRecoveryDrainAuthorization {\n"
        "    let _legacy = CertifiedServeAdmission;\n"
        "    match preparation {",
        "terminal recovery cannot mint coordinator-owned Serve authority",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "pub(in crate::sumeragi) fn drive_ingress_turn<'cursor>(",
        "self.pending_ingress_capacity.take()",
        "self.pending_ingress_capacity.as_ref()",
        "ordinary/recovered ingress owner order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "impl Drop for ProductionPreparedOrdinaryIngressTurnV1 {",
        "handoff.close_output_for_restart();",
        "let _ = handoff;",
        "opaque ordinary token omits",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(super) fn classify_selected_certified_response_priority(",
        "if response.request_hash != selected_request_hash {\n                continue;\n            }",
        "if false {\n                continue;\n            }",
        "closed selected certified-response priority census",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(super) fn classify_selected_certified_response_priority(",
        "SelectedCertifiedResponsePriorityV1::OrdinaryClaimed",
        "SelectedCertifiedResponsePriorityV1::RecoveredClaimed",
        "closed selected certified-response priority census",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(super) fn classify_selected_certified_response_priority(",
        "SelectedCertifiedResponsePriorityV1::RecoveredClaimed",
        "SelectedCertifiedResponsePriorityV1::OrdinaryClaimed",
        "closed selected certified-response priority census",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(super) fn classify_selected_certified_response_priority(",
        "if !cut.pre_cut_is_intact() {",
        "if false {",
        "closed selected certified-response priority census",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "pub(in crate::sumeragi) fn drive_ingress_turn<'cursor>(",
        "cut.into_ordinary_turn_cut()",
        "cut",
        "ordinary/recovered ingress owner order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "pub(in crate::sumeragi) fn drive_ingress_turn<'cursor>(",
        "capture_lifecycle_ingress_selector(cut)",
        "prepare_recovered_decision_fetch_from_selected_cut(cut)",
        "ordinary/recovered ingress owner order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "pub(in crate::sumeragi) fn drive_ingress_turn<'cursor>(",
        "prepare_recovered_decision_fetch_from_selected_cut(cut)",
        "capture_lifecycle_ingress_selector(cut)",
        "ordinary/recovered ingress owner order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(super) fn prepare_recovered_decision_fetch_from_selected_cut(",
        "Some(selected_request_hash)",
        "None",
        "selected-family Phase-A preparation",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "pub(in crate::sumeragi) fn drive_ingress_turn<'cursor>(",
        "if !selected_ingress_is_certified_body_response(",
        "if false && !selected_ingress_is_certified_body_response(",
        "selected non-response winner bypasses response census",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "let (ordinary_turn, after_ordinary_ingress) =",
        "an ordinary head cannot be poisoned by a later response family",
        "ordinary head unexpectedly selected lifecycle work",
        "real-cursor ordinary ingress regression omits",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "fn armed_token_closes_output_before_releasing_dequeued_carrier_and_serve_result()",
        "fn armed_token_closes_output_before_releasing_dequeued_carrier_and_serve_result()",
        "fn armed_token_drops_without_closing_output()",
        "require exactly one real Rust/Verus function item",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "pub(in crate::sumeragi) struct ProductionPreparedOrdinaryIngressTurnV1 {",
        "handoff: Option<PreparedDequeuedV2IngressV1>,",
        "pub(in crate::sumeragi) handoff: Option<PreparedDequeuedV2IngressV1>,",
        "opaque ordinary token exposes forbidden surface",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "pub(crate) struct PreparedProductionIngressCapacityWait {",
        "pub(crate) struct PreparedProductionIngressCapacityWait {",
        "#[derive(Clone)]\npub(crate) struct PreparedProductionIngressCapacityWait {",
        "retained ingress capacity wait must remain sealed",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "pub(crate) struct PreparedProductionIngressCapacityWait {",
        "selector: PreparedLifecycleIngressSelector",
        "pub(crate) selector: PreparedLifecycleIngressSelector",
        "retained ingress capacity wait must remain sealed",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "impl PreparedProductionIngressCapacityWait {",
        "/// Classify the exact retained service generation without exposing it.",
        "pub(crate) fn selector(&self) -> &PreparedLifecycleIngressSelector {\n        &self.selector\n    }\n\n    /// Classify the exact retained service generation without exposing it.",
        "retained ingress capacity wait must remain sealed",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "impl PreparedProductionIngressCapacityWait {",
        "/// Classify the exact retained service generation without exposing it.",
        "pub(crate) fn into_parts(self) {\n        drop(self);\n    }\n\n    /// Classify the exact retained service generation without exposing it.",
        "retained ingress capacity wait must remain sealed",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_operations.rs",
        "fn authenticate_recovered_phase_signed_broadcast_and_sign(",
        "combined.broadcast_exactly_matches(&broadcast)",
        "true",
        "cold recovered phase Broadcast-and-Sign ledger join omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_recovered_wal_persisted_ledger_impl.rs",
        "fn prepare_cold_signed_broadcast_and_next_vote_branch(",
        "authenticate_recovered_lifecycle_next_vote_body(&mut preview)",
        "authenticate_recovered_lifecycle_next_vote_body_unchecked(&mut preview)",
        "cold recovered phase Broadcast-and-Sign registry join omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_recovered_wal_persisted_ledger_impl.rs",
        "fn prepare_cold_adapter_startup(",
        "Self::prepare_cold_signed_broadcast_branch(",
        "Self::prepare_cold_signed_broadcast_branch_unchecked(",
        "cold recovered phase Broadcast-and-Sign registry join omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_recovered_wal_persisted_ledger_impl.rs",
        "fn prepare_cold_signed_broadcast_branch(",
        "drop(matching);",
        "let _ = matching;",
        "cold recovered phase Broadcast-and-Sign registry join omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_recovered_wal_persisted_ledger_impl.rs",
        "fn prepare_cold_signed_broadcast_branch(",
        "Self::prepare_cold_single_signed_broadcast_branch(",
        "Self::prepare_cold_single_signed_broadcast_branch_unchecked(",
        "cold recovered phase Broadcast-and-Sign registry join omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_recovered_wal_persisted_ledger_impl.rs",
        "fn install_recovered_wal_sign(",
        "Self::install_recovered_broadcast_and_next_vote_branch(",
        "Self::install_recovered_broadcast_and_next_vote_branch_unchecked(",
        "cold recovered phase Broadcast-and-Sign registry join omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
        "fn open_recovered_decision_apply_startup(",
        ".stage_authenticated_wal_decision_fetch(projection.fetch())",
        ".stage_recovered_decision_apply(projection.as_ref())",
        "cold recovered Decision Apply startup lineage",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
        "fn open_recovered_decision_apply_startup(",
        "ledger_store,\n            predecessor,\n            successor.clone(),",
        "ledger_store,\n            staged_predecessor,\n            successor.clone(),",
        "cold recovered Decision Apply startup lineage",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_recovered_wal.rs",
        "fn install_recovered_broadcast_and_next_vote(",
        "paired_next_sign: Some((next_sign_address, next_sign_digest))",
        "paired_next_sign: None",
        "cold recovered phase Broadcast-and-Sign registry join omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_main_03.rs",
        "fn production_recovered_proposal_sign_joins_exact_next_vote_body_store()",
        "fn production_recovered_proposal_sign_joins_exact_next_vote_body_store()",
        "fn production_recovered_proposal_sign_skips_next_vote_body_store()",
        "recovered Sign adapter preview behavior regression omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_worker_main_01.rs",
        "fn recovered_proposal_exact_output_is_atomic_retryable_and_store_bound()",
        "fn recovered_proposal_exact_output_is_atomic_retryable_and_store_bound()",
        "fn recovered_proposal_exact_output_allows_partial_control()",
        "atomic Proposal output behavior regression omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_worker_recovered_lifecycle_output_cases.rs",
        "fn atomic_fanout_batch_preflights_aggregate_capacity_and_rebases_only_on_commit()",
        "fn atomic_fanout_batch_preflights_aggregate_capacity_and_rebases_only_on_commit()",
        "fn atomic_fanout_batch_allows_one_child_prefix()",
        "atomic Proposal aggregate-capacity regression omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_operations.rs",
        "fn project_validated_recovered_lifecycle_signed_broadcast_and_sign_at(",
        "let next_sign_ordinal = broadcast_ordinal.checked_add(1)?",
        "let next_sign_ordinal = broadcast_ordinal.checked_add(2)?",
        "frame-bound recovered Broadcast-and-next-Sign ledger classifier omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_operations.rs",
        "fn recovered_lifecycle_signed_broadcast_and_sign_pairs(",
        "&index,",
        "&RecoveredLifecycleSignedBroadcastAndSignLedgerIndexV1::new(&self.records),",
        "combined Broadcast-and-next-Sign enumeration must reuse one bounded frame index",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_operations.rs",
        "fn project_validated_recovered_lifecycle_signed_broadcast_and_sign_at(",
        "index.owner_record_count(next_sign_owner) != 1",
        "index.owner_record_count(next_sign_owner) != 0",
        "frame-bound recovered Broadcast-and-next-Sign ledger classifier omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_recovered_wal.rs",
        "fn prepare_recovered_lifecycle_sign_broadcast_and_sign_successor<",
        "adapter.project_broadcast_and_sign_authority(body)",
        "adapter.project_broadcast_and_sign_without_body()",
        "opaque recovered Broadcast-and-next-Sign registry preparation must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn commit_after_durable_broadcast_and_sign(self)",
        "adapter.pending_persistence_id = None",
        "let _ = adapter.pending_persistence_id.take();",
        "durable recovered Proposal adapter two-child commit must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn persist_recovered_decision_fetch_response_after_runner(",
        "executor\n            .prepare_recovered_decision_fetch_response_claim(&task)",
        "executor\n            .prepare_unowned_decision_fetch_response_claim(&task)",
        "recovered Decision Fetch response persistence Phase A must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "pub(in crate::sumeragi) fn recovered_decision_fetch_registration_available(",
        "self.validated_certified_request_presence().is_err()",
        "false",
        "dedicated recovered Decision Fetch request owner census omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) struct LaunchedProductionLifecycleV1 {",
        "pending_lifecycle_completion: Option<PendingLifecycleCompletionV1>,",
        "pending_lifecycle_completion: (),",
        "launched unified lifecycle completion/capacity Drop order must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_impl.rs",
        "pub(super) fn install_recovered_wal_decision_store<'registry>(",
        "pub(super) fn install_recovered_wal_decision_store<'registry>(",
        "pub(super) fn install_unchecked_recovered_wal_decision_store<'registry>(",
        "dedicated recovered Decision Store registry install omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_runner_authority.rs",
        "impl RecoveredLifecycleOwnerFactoryDependencyPermitV1 {",
        "pub(super) fn mint_for_recovered_runner(\n        local_signer: KeyPair,",
        "pub(in crate::sumeragi) fn mint_for_recovered_runner(\n        local_signer: KeyPair,",
        "runner-sealed recovered lifecycle factory dependencies must use the opaque checked-transition gate",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "pub(in crate::sumeragi) fn bind_production_lifecycle_owner_factory_inputs_v1(",
        "let (local_signer, block_cadence) = permit.into_factory_dependencies();",
        "let (local_signer, block_cadence) = permit.into_factory_dependencies();\n        let _placeholder_cadence = state.sumeragi_block_cadence();",
        "authenticated lifecycle factory cadence must use the opaque checked-transition gate",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn bind(",
        "ingress.bind_leader_wire_lifecycle_gate(",
        "ingress.bind_unreviewed_leader_wire_lifecycle_gate(",
        "sealed leader-wire launch binding omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn retire(&mut self) -> Result<(), String>",
        "self.ingress.retire_leader_wire_lifecycle_gate(&gate)",
        "self.ingress.retire_unreviewed_leader_wire_lifecycle_gate(&gate)",
        "sealed leader-wire launch binding omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs",
        "fn consume_prepared_dequeued_v2_ingress(",
        ".serve_historical_body(kura, request, &sender, local_key)",
        ".serve_historical_body(kura, context_store, request, &sender, local_key)",
        "historical ingress routing omits production refinement tokens",
    ),
    (
        "scripts/run_sumeragi_v2_release_gates.sh",
        "required_production_liveness_tests=(",
        "sumeragi::v2::tests::production_recovered_proposal_sign_joins_exact_next_vote_body_store",
        "sumeragi::v2::tests::production_recovered_proposal_sign_is_not_release_bound",
        "production refinement test must be pinned exactly once",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout()",
        ".activate(Instant::now(), activation, local_proposal_state)",
        ".activate(Instant::now(), activation_forbidden, local_proposal_state)",
        "production lifecycle activation behavior must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
        "fn launch(",
        "launched\n            .reauthenticate_recovered_complete_tip_successor(&mut retirement)\n            .map_err(|_| super::launch::ProductionLifecycleLaunchErrorV1::InvalidOwner)?;",
        "let _ = &mut retirement;",
        "CompleteTip sealed H+1 owner launch must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
        "impl LaunchedRecoveredCompleteTipSuccessorLifecycleV1",
        "launched.activate_recovered_complete_tip(now, runner, retirement, local_proposal)",
        "launched.activate(now, runner, local_proposal)",
        "CompleteTip exact H+1 owner bind must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "struct ActivatedProductionLifecycleV1",
        "local_proposal: ProductionLifecyclePreparedLocalProposalStateV1,\n    launched: LaunchedProductionLifecycleV1,",
        "launched: LaunchedProductionLifecycleV1,\n    local_proposal: ProductionLifecyclePreparedLocalProposalStateV1,",
        "opaque activated lifecycle owner drop order must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_recovery.rs",
        "fn into_parts_with_lifecycle_storage_authority(",
        "if !kura_identity.matches(kura) {",
        "if false {",
        "verified successor lifecycle storage authority projection must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_recovery.rs",
        "fn into_parts_with_lifecycle_storage_authority(",
        "let signature_policy = BlockSignaturePolicy::RotatingLeader;",
        "let signature_policy = caller_signature_policy;",
        "verified successor lifecycle storage authority projection must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_recovery.rs",
        "fn build_verified_successor(",
        "kura_identity: state.kura().instance_identity(),",
        "kura_identity: foreign_kura.instance_identity(),",
        "verified successor exact Kura retention must contain",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_recovery.rs",
        "fn verified_successor_projects_only_its_exact_kura_lifecycle_storage()",
        "Err(V2RecoveryError::SuccessorLifecycleStorageKuraMismatch { height: 2 })",
        "Err(V2RecoveryError::SuccessorLifecycleStorageKuraMismatch { height: 3 })",
        "verified successor lifecycle storage projection behavior must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs",
        "pub(in crate::sumeragi) struct PreparedDequeuedV2IngressV1 {",
        "inbound: Option<InboundBlockMessage>,",
        "pub(in crate::sumeragi) inbound: Option<InboundBlockMessage>,",
        "opaque already-dequeued ordinary owner exposes forbidden surface",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs",
        "fn consume_prepared_dequeued_v2_ingress(",
        "if !prepared.matches_output_guard(&services_output_guard) {",
        "if false {",
        "single exact ordinary post-dequeue runner tail",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/decided_lane_recovery.rs",
        "fn drain_decided_lane_recovery_ingress(",
        "commit_decided_lane_recovery_drain(authorization, &mut committer)",
        "commit_unchecked_decided_lane_recovery_drain(authorization, &mut committer)",
        "live terminal drain retains current Serve before checked dequeue",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "fn consume_prepared_ordinary_ingress_turn(",
        "consume_prepared_dequeued_v2_ingress(",
        "consume_unsealed_v2_ingress(",
        "activated lifecycle ordinary ingress shares the runner tail",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "pub(in crate::sumeragi) struct PreparedRecoveredPendingKuraApplyReplayV1 {",
        "effect: AdapterEffect,",
        "pub(in crate::sumeragi) effect: AdapterEffect,",
        "opaque pending-Kura replay types expose forbidden surface",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_recovered_wal.rs",
        "impl<'registry, 'adapter> PreparedRecoveredLifecycleSignBroadcastSuccessor<'registry, 'adapter>",
        "exact_staged_recovered_lifecycle_broadcast_address(",
        "inexact_staged_recovered_lifecycle_broadcast_address(",
        "staged recovered Broadcast registry binding must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_recovered_wal.rs",
        "pub(super) fn exact_staged_recovered_lifecycle_broadcast_address(",
        "broadcast.validates_at(verified, broadcast_address, child_digest)",
        "broadcast.validates_at_for_mutation(verified, broadcast_address, child_digest)",
        "staged recovered Broadcast address authentication must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_recovered_wal.rs",
        "impl<'registry, 'adapter> PreparedRecoveredDecisionFetchStoreSuccessor<'registry, 'adapter>",
        "exact_staged_recovered_decision_store_address(",
        "inexact_staged_recovered_decision_store_address(",
        "staged recovered Store registry binding must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_recovered_wal.rs",
        "pub(super) fn exact_staged_recovered_decision_store_address(",
        "store.matches_current_ready_record(",
        "store.matches_current_ready_record_for_mutation(",
        "staged recovered Store address authentication must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "fn exactly_matches_fresh_staged_append(",
        "serve <= current.high_water",
        "serve > current.high_water",
        "gap-aware fresh Serve staged append",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "fn exactly_matches_fresh_staged_append(",
        "serve.checked_add(1) != Some(producer)",
        "serve.checked_add(1) == Some(producer)",
        "gap-aware fresh Serve staged append",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "fn exactly_matches_fresh_staged_append(",
        "current.records.len().checked_add(2) != Some(staged.records.len())",
        "current.records.len().checked_add(1) != Some(staged.records.len())",
        "gap-aware fresh Serve staged append",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn authenticate_final_wal_startup_authority(",
        "let RecoveredWalStartupAuthorityV1::DecisionFetch(fetch) = authority else {",
        "let RecoveredWalStartupAuthorityV1::None = authority else {",
        "pending-Kura Decision-Fetch ownership transfer into storage-only startup",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn authenticate_final_wal_startup_authority(",
        "replay: RecoveredPendingKuraApplyReplayV1 { expected, fetch },",
        "replay: RecoveredPendingKuraApplyReplayV1 { expected, fetch: recovered_fetch },",
        "pending-Kura Decision-Fetch ownership transfer into storage-only startup",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn into_serialized_runtime(",
        "let RecoveredWalDecisionFetch {",
        "let RecoveredWalDecisionFetchRemoved {",
        "pending-Kura exact Fetch ownership roundtrip through runtime startup",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "pub(in crate::sumeragi) struct PreparedPendingKuraValidatedApplyV1<'a> {",
        "child_ownership: crate::sumeragi::v2_runtime::RuntimeEffectOwnership,",
        "pub(in crate::sumeragi) child_ownership: crate::sumeragi::v2_runtime::RuntimeEffectOwnership,",
        "move-only pending-Kura marker/child types expose forbidden surface",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "pub(in crate::sumeragi) fn classify_and_defer_validated_marker(",
        "if self.deferred_validated_marker.is_some() {",
        "if false {",
        "pending-Kura validated-marker deferral",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "pub(in crate::sumeragi) fn exactly_matches_recovery(",
        "self.tag == replay_tag",
        "true",
        "pending-Kura exact deferred marker",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "pub(in crate::sumeragi) fn prepare_apply<'a>(",
        "validate_pending.project_validate_apply_successor(predecessor, &apply_effect)",
        "validate_pending.project_validate_apply_successor_for_mutation(predecessor, &apply_effect)",
        "pending-Kura marker-owned direct Validate-to-Apply preview",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn install(",
        "let genesis = executor.verify_pending_kura_apply_replay(",
        "let genesis = executor.verify_pending_kura_apply_replay_without_marker(",
        "pending-Kura marker-verified direct pipeline install",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn classify_and_defer_validated_marker(",
        "self.deferred_validated_marker = Some(DeferredPendingKuraValidatedMarkerV1 {",
        "let _deferred_validated_marker = DeferredPendingKuraValidatedMarkerV1 {",
        "move-only pending-Kura marker deferral",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn exactly_matches_recovery(",
        "self.certificate == *certificate",
        "true",
        "exact pending-Kura marker authority",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn prepare_apply<'a>(",
        "DirectValidationSucceededPreparation::Apply(prepared)",
        "DirectValidationSucceededPreparation::Sign(prepared)",
        "sealed pending-Kura ValidationCompleted Apply preview",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn prepare_apply<'a>(",
        "project_validate_apply_successor(predecessor, &apply_effect)",
        "project_store_validate_successor(predecessor, &apply_effect)",
        "sealed pending-Kura ValidationCompleted Apply preview",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runtime.rs",
        "pub(in crate::sumeragi) fn prepare_pending_kura_validated_apply(",
        "|| self.clocks_armed",
        "|| false",
        "no-clock pending-Kura validation runtime seam",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects_lifecycle_admission_settlement.rs",
        "fn validate_body<S: V2EffectServices>(",
        "take_deferred_validated_marker()?",
        "clone_deferred_validated_marker()?",
        "direct pending-Kura Validate-to-Apply child",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "fn consume_one<S: V2EffectServices>(",
        "if let Some(stage) = recovery_transition {",
        "if false {",
        "pending-Kura outer-stage-before-direct-child dispatch",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "fn step_pending_tip_recovery<S: V2EffectServices>(",
        "self.consume_pending_tip_recovery_effects(effects, services)?",
        "self.consume_effects(effects, services)?",
        "stage-complete direct-marker pending-tip recovery step",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "fn step_pending_tip_recovery<S: V2EffectServices>(",
        "RuntimeStep::Advanced(effects) => {",
        "RuntimeStep::Advanced(effects) => {\n                let stage = PendingKuraApplyRecoveryStage::Apply;\n                if stage != PendingKuraApplyRecoveryStage::Apply { unreachable!(); }",
        "stage-complete direct-marker pending-tip recovery step",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn consume_for_executor(",
        "_permit: crate::sumeragi::v2_effects::PendingKuraApplySuccessorExecutorPermitV1",
        "_permit: ()",
        "executor-permit pending-Kura Apply child release",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn bind_pending_kura_apply(",
        "expected.height() != self.adapter.wire_context.height",
        "false",
        "pending-Kura startup context binding",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn authenticate_final_wal_startup_authority(",
        "subject.block_hash == expected.block_hash()",
        "true",
        "pending-Kura Decision-Fetch ownership transfer into storage-only startup",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn authenticate_final_wal_startup_authority(",
        "authority: RecoveredWalStartupAuthorityV1::None,",
        "authority: RecoveredWalStartupAuthorityV1::DecisionFetch(fetch),",
        "pending-Kura Decision-Fetch ownership transfer into storage-only startup",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn into_serialized_runtime(",
        "vec![effect],",
        "Vec::new(),",
        "pending-Kura exact Fetch ownership roundtrip through runtime startup",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn install(",
        "executor.consume_pending_tip_recovery_effects(effects, services)?;",
        "executor.consume_effects(effects, services)?;",
        "pending-Kura marker-verified direct pipeline install",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn install(",
        "executor.verify_pending_kura_apply_replay(",
        "executor.verify_pending_kura_apply_replay_unchecked(",
        "pending-Kura marker-aware verification-before-dispatch install",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_preactivation.rs",
        "fn missing_pending_kura_replay(",
        "output_guard.close_admission_for_restart();",
        "let _ = output_guard;",
        "missing pending-Kura replay fail-stop",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_pending_kura.rs",
        "fn install_pending_kura_apply(",
        "super::preactivation::missing_pending_kura_replay(",
        "ProductionPendingKuraApplyInstallErrorV1::MissingReplay",
        "fail-stop pending-Kura preactivation install",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) enum CertifiedFetchBodyPersistenceCompletionError {",
        "RestartRequiredBeforeLedger(CertifiedFetchBodyPersistencePreLedgerRestartError)",
        "RetryBeforeLedger(CertifiedFetchBodyPersistencePreLedgerRestartError)",
        "certified Fetch Phase-B result split",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) enum CertifiedFetchBodyPersistenceCompletionError {",
        "RestartRequiredAfterDequeue(String)",
        "RestartRequiredAfterCommit(String)",
        "certified Fetch Phase-B result split",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn complete_certified_fetch_body_persistence(",
        "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredBeforeLedger(",
        "CertifiedFetchBodyPersistenceCompletionError::Retry(",
        "complete_certified_fetch_body_persistence must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn complete_certified_fetch_body_persistence(",
        "durable_registry.durable_body_receipt(),",
        "durable_registry.unchecked_body_receipt(),",
        "complete_certified_fetch_body_persistence must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn certified_fetch_preledger_productive_ingress_token(",
        "inbound\n        .ingress_ownership()",
        "None",
        "certified Fetch pre-Ledger productive-ingress validation",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "fn certified_fetch_ingress_ownership_is_exact(",
        "ownership.validate_exact()",
        "true",
        "certified Fetch exact ingress ownership predicate",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn complete_certified_fetch_body_persistence(",
        "exact_dequeue.commit(ingress)",
        "exact_dequeue.commit_without_runtime_receipt(ingress)",
        "complete_certified_fetch_body_persistence must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn certified_fetch_preledger_productive_ingress_token(",
        "if !certified_fetch_ingress_ownership_is_exact(inbound, ownership) {",
        "if false {",
        "certified Fetch pre-Ledger productive-ingress validation",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn certified_fetch_preledger_productive_ingress_token(",
        ".leader_wire_token()",
        ".leader_wire_runtime_receipt()",
        "certified Fetch pre-Ledger productive-ingress validation",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn certified_fetch_preledger_productive_ingress_token(",
        "ownership.leader_wire_runtime_receipt().is_some()",
        "ownership.leader_wire_runtime_receipt().is_none()",
        "certified Fetch pre-Ledger productive-ingress validation",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn certified_fetch_postdequeue_runtime_receipt(",
        "inbound\n        .ingress_ownership()",
        "None",
        "certified Fetch post-dequeue Runtime-receipt validation",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn certified_fetch_postdequeue_runtime_receipt(",
        "if !certified_fetch_ingress_ownership_is_exact(inbound, ownership) {",
        "if false {",
        "certified Fetch post-dequeue Runtime-receipt validation",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn certified_fetch_postdequeue_runtime_receipt(",
        ".leader_wire_runtime_receipt()",
        ".leader_wire_token()",
        "certified Fetch post-dequeue Runtime-receipt validation",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn certified_fetch_postdequeue_runtime_receipt(",
        "receipt.token() != expected_token",
        "receipt.token() == expected_token",
        "certified Fetch post-dequeue Runtime-receipt validation",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn certified_fetch_postdequeue_runtime_receipt(",
        "receipt.owner().causal_lifecycle_key() != expected_token.identity_hash()",
        "receipt.owner().causal_lifecycle_key() == expected_token.identity_hash()",
        "certified Fetch post-dequeue Runtime-receipt validation",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn certified_fetch_postdequeue_runtime_receipt(",
        "receipt.owner().admission_ordinal() != expected_token.scheduler_ordinal()",
        "receipt.owner().admission_ordinal() == expected_token.scheduler_ordinal()",
        "certified Fetch post-dequeue Runtime-receipt validation",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) struct CertifiedFetchBodyPersistencePreLedgerRestartError {",
        "failure: CertifiedFetchPreLedgerProductiveIngressErrorV1,",
        "failure: (),",
        "certified Fetch pre-Ledger restart owner",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn complete_certified_fetch_body_persistence(",
        "output_guard.close_admission_for_restart();",
        "let _ = &output_guard;",
        "complete_certified_fetch_body_persistence must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn complete_certified_fetch_body_persistence(",
        "restart_invalid_leader_wire!(error, receipt);",
        "retry!(CertifiedFetchBodyPersistenceRetryFailure::CompletionIdentity, receipt);",
        "certified Fetch pre-dequeue invalid-owner fail-stop",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn complete_certified_fetch_body_persistence(",
        "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterDequeue(",
        "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(",
        "certified Fetch post-dequeue restart boundary",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) fn complete_certified_fetch_body_persistence(",
        "durable_registry.commit_after_exact_dequeue(dequeued);",
        "drop(dequeued);",
        "complete_certified_fetch_body_persistence must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/mod.rs",
        "fn dequeue_selected_locked(",
        "Self::bind_leader_wire_runtime_ownership_locked(state, &mut staged_ownership)?;",
        "let _ = (&state, &mut staged_ownership);",
        "sole exact-dequeue leader-wire Runtime receipt mint",
    ),
    (
        "crates/iroha_core/src/sumeragi/mod.rs",
        "fn bind_leader_wire_runtime_ownership_locked(",
        "ownership.install_leader_wire_runtime_receipt(receipt)",
        "ownership.leader_wire_runtime_receipt().is_some()",
        "leader-wire Runtime receipt mint",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "fn settle_parked_certified_fetch_body_persistence(",
        "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredBeforeLedger(",
        "CertifiedFetchBodyPersistenceCompletionError::Retry(",
        "certified Fetch Phase-B turn result split",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "fn settle_parked_certified_fetch_body_persistence(",
        "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterDequeue(",
        "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(",
        "certified Fetch Phase-B turn result split",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn activate_with(",
        "self.recovered_local_proposal_attempt.is_some()",
        "false",
        "ordinary activation rejects incomplete recovered local-Proposal setup",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn lifecycle_activation_recovery_blocker(",
        "pending_kura_replay || pending_kura_evidence",
        "false",
        "ordinary activation recovery preflight",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_preactivation.rs",
        "fn initialize_recovered_local_proposal(",
        "recovered.exactly_matches_directive(directive)",
        "true",
        "closed-ingress recovered local-Proposal initialization",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_preactivation.rs",
        "fn initialize_recovered_local_proposal(",
        "if !runner.bind_recovered_local_proposal(directive) {",
        "if false {",
        "closed-ingress recovered local-Proposal initialization",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "struct RecoveredLifecycleLocalProposalAttemptV1 {",
        "subject: wire::BlockSubject,",
        "pub(in crate::sumeragi) subject: wire::BlockSubject,",
        "opaque recovered local-Proposal owner exposes forbidden surface",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        "fn production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout()",
        "assert!(local_proposal_state.already_attempted(directive));",
        "assert!(!local_proposal_state.already_attempted(directive));",
        "production-shaped recovered local-Proposal initialization behavior",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn activate_with(",
        "local_proposal,\n            launched: self,",
        "launched: self,",
        "ordinary activation retains prepared local-Proposal state",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn activate_with(",
        "if !local_proposal.exactly_matches(self.executor.context().id(), current_directive) {",
        "if false {",
        "ordinary activation rejects incomplete recovered local-Proposal setup",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "impl ProductionLifecyclePreparedLocalProposalStateV1",
        "self.context_id == context_id",
        "true",
        "affine prepared local-Proposal state omits",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "impl ProductionLifecyclePreparedLocalProposalStateV1",
        "self.directive == directive",
        "true",
        "affine prepared local-Proposal state omits",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "impl ProductionLifecyclePreparedLocalProposalStateV1",
        ".prepared_local_proposal_exactly_matches(directive)",
        ".local_proposal_state_is_pristine()",
        "affine prepared local-Proposal state omits",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_tests.rs",
        "fn prepared_local_proposal_state_is_affine_and_context_directive_bound()",
        "assert!(!prepared.exactly_matches(foreign_context, directive));",
        "assert!(prepared.exactly_matches(foreign_context, directive));",
        "affine prepared local-Proposal state behavior",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
        "impl LaunchedRecoveredCompleteTipSuccessorLifecycleV1",
        "self.launched.initialize_recovered_local_proposal(runner)",
        "unreachable!()",
        "CompleteTip recovered local-Proposal initialization delegation",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn into_finalized_rollover(",
        "mut launched,\n            local_proposal,\n            runner_activation,",
        "runner_activation,\n            local_proposal,\n            mut launched,",
        "activated lifecycle finalization must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn retire_lifecycle_stores_for_test(",
        "mut launched,\n            local_proposal,\n            runner_activation,",
        "runner_activation,\n            local_proposal,\n            mut launched,",
        "consuming activated Serve retirement fixture must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/tests/v2_adapter_04_wal_recovery_decision_classifier_cases.rs",
        "fn recovered_decision_fetch_classifier_authenticates_exact_absent_manifest_and_sources()",
        "decision.subject.block_hash,",
        "HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b\"wrong pending block\")),",
        "pending-Kura bridge behavior",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_preactivation.rs",
        "fn missing_pending_kura_replay_closes_canonical_output()",
        "assert!(output_guard.restart_required());",
        "assert!(!output_guard.restart_required());",
        "missing pending-Kura replay behavior",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs",
        "impl Drop for PreparedDequeuedV2IngressFailStopScopeV1 {",
        "self.output_guard.close_admission_for_restart();",
        "let _ = &self.output_guard;",
        "ordinary runner-tail non-permit fail-stop scope omits",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn with_pending_kura_apply_replay(",
        "effects.is_empty() && pending_kura_apply.is_none()",
        "true && pending_kura_apply.is_none()",
        "pending-Kura pristine storage-only startup attachment",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "#[path = \"v2_pending_kura_recovery.rs\"]",
        "#[path = \"v2_pending_kura_recovery.rs\"]",
        "#[path = \"v2_pending_kura_recovery_removed.rs\"]",
        "sealed lifecycle child module wiring",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "#[path = \"v2_lifecycle_preactivation.rs\"]",
        "#[path = \"v2_lifecycle_preactivation.rs\"]",
        "#[path = \"v2_lifecycle_preactivation_removed.rs\"]",
        "sealed lifecycle child module wiring",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "#[path = \"v2_runner/ordinary_ingress_consumer.rs\"]",
        "#[path = \"v2_runner/ordinary_ingress_consumer.rs\"]",
        "#[path = \"v2_runner/ordinary_ingress_consumer_removed.rs\"]",
        "sealed lifecycle child module wiring",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs",
        "#[path = \"v2_lifecycle_coordinator_support.rs\"]",
        "#[path = \"v2_lifecycle_coordinator_support.rs\"]",
        "#[path = \"v2_lifecycle_coordinator_support_removed.rs\"]",
        "sealed lifecycle child module wiring",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn dispatch_recovered_lifecycle_sign_with_runner_debt(",
        "services.matches_lifecycle_body_store(body_store_identity)",
        "true",
        "lifecycle-owned recovered Sign dispatch must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn dispatch_recovered_lifecycle_sign_with_runner_debt(",
        "reservation.class() == CapacityClass::Consensus",
        "true",
        "lifecycle-owned recovered Sign dispatch must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_completion.rs",
        "impl PreparedRecoveredLifecycleSignCompletionV1",
        "result.is_exact()",
        "true",
        "adapter-private recovered Sign completion projection omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion(",
        "verify_individual_signature(",
        "trust_individual_signature(",
        "drop-inert recovered Sign adapter preview must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion(",
        "vote.phase == wire::GlobalPhase::Prepare",
        "true",
        "closed recovered Sign adapter successor shapes omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn settle_recovered_lifecycle_sign_broadcast(",
        "output_guard.begin_fail_stop_operation()",
        "output_guard.is_open()",
        "restart-closed recovered Sign-to-Broadcast settlement must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn settle_recovered_lifecycle_sign_broadcast(",
        "transition.persist_exact_successor().is_err()",
        "transition.skip_durable_publication().is_err()",
        "restart-closed recovered Sign-to-Broadcast settlement must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
        "services.matches_lifecycle_body_store(body_store_identity)",
        "true",
        "restart-safe recovered signed-Broadcast refanout must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
        "settle_turn(lease, super::TurnOutcome::Blocked(wait))",
        "settle_turn(lease, super::TurnOutcome::Terminal(TerminalOutcome::Completed(None)))",
        "restart-safe recovered signed-Broadcast refanout must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
        "recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal",
        "recovered_lifecycle_signed_broadcast_unchecked_adjacent_ordinal",
        "restart-safe recovered signed-Broadcast refanout must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_impl.rs",
        "fn recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal(",
        "next.ordinal == broadcast_ordinal.checked_add(1)?",
        "next.ordinal > broadcast_ordinal",
        "retained recovered Broadcast-and-next-Vote pair seal omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
        "for ready_ordinal in &exact_ready",
        "for ready_ordinal in core::iter::once(&ordinal)",
        "restart-safe recovered signed-Broadcast refanout must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
        "fn capture_recovered_lifecycle_signed_broadcast_refanout(",
        "authority.consume_for_service(RecoveredLifecycleSignBroadcastOutputPermitV1::new())",
        "authority.into_parts()",
        "durable recovered signed-Broadcast service capture omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
        "fn capture_recovered_lifecycle_cold_proposal_message(",
        "pending.prepare_atomic_fanout_batch(fanouts)",
        "Ok(None)",
        "durable recovered signed-Broadcast service capture omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_wal_recovery.rs",
        "fn recover_durable_signed_broadcast(",
        "verified.verify_consensus_message(message)",
        "Ok(())",
        "cold recovered signed-Broadcast WAL and roster join omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn advance_recovered_lifecycle_signed_broadcast(",
        "let [reducer::Effect::Broadcast(message)] = core_effects.as_slice()",
        "let [message, ..] = core_effects.as_slice()",
        "cold recovered signed-Broadcast reducer fast-forward omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_open.rs",
        "fn assemble_storage_only_with_recovered_phase_broadcast_and_body_pipeline_startup(",
        "RecoveredWalStartupProjectionV1::PhaseBroadcast(projection, broadcast)",
        "RecoveredWalStartupProjectionV1::PhaseVote(projection)",
        "cold recovered phase-Broadcast storage assembly omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_open.rs",
        "fn assemble_storage_only_with_recovered_phase_broadcast_and_next_sign_and_body_pipeline_startup(",
        "RecoveredWalStartupProjectionV1::PhaseBroadcastAndNextSign(",
        "RecoveredWalStartupProjectionV1::PhaseBroadcast(",
        "cold recovered signed-Broadcast storage census omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "fn prepare_recovered_phase_vote_cold_adapter_stage<'registry>(",
        "prepare_cold_adapter_startup(&verified, adapter_startup, body_store)",
        "prepare_cold_adapter_startup_unchecked(&verified, adapter_startup, body_store)",
        "cold recovered phase owner handoff omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "fn open_recovered_control_authority_branch(",
        "Self::ensure_recovered_body_store_context(&body_store, &verified)?;",
        "Self::ensure_recovered_body_store_context_unchecked(&body_store, &verified)?;",
        "recovered local-Proposal owner factory handoff",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "fn open_recovered_control_projection_branch(",
        "ProductionLifecycleAdapterStartupV1::recovered_with_local_proposal_attempt(",
        "ProductionLifecycleAdapterStartupV1::recovered(",
        "recovered local-Proposal owner factory handoff",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "fn open_recovered_phase_vote_branch(",
        "Self::ensure_recovered_body_store_context(&body_store, &verified)?;",
        "Self::ensure_recovered_body_store_context_unchecked(&body_store, &verified)?;",
        "cold recovered phase owner handoff",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "fn open_recovered_phase_vote_branch(",
        "Self::persist_recovered_phase_vote_stage(authenticated)",
        "Self::persist_recovered_phase_vote_stage_unchecked(authenticated)",
        "cold recovered phase owner handoff",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "fn open_recovered_phase_vote_branch(",
        "Self::prepare_recovered_phase_vote_cold_adapter_stage(",
        "Self::prepare_recovered_phase_vote_cold_adapter_stage_unchecked(",
        "cold recovered phase owner handoff",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "fn open_recovered_phase_vote_branch(",
        "Self::install_recovered_phase_vote_sign_stage(prepared)",
        "Self::install_recovered_phase_vote_sign_stage_unchecked(prepared)",
        "cold recovered phase owner handoff",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "fn open_recovered_phase_vote_branch(",
        "Self::open_recovered_phase_vote_seals_stage(",
        "Self::open_recovered_phase_vote_seals_stage_unchecked(",
        "cold recovered phase owner handoff",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "fn open_recovered_phase_vote_branch(",
        "Self::finish_recovered_phase_vote_owner_stage(",
        "Self::finish_recovered_phase_vote_owner_stage_unchecked(",
        "cold recovered phase owner handoff",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "fn authenticate_recovered_phase_vote_stage<'registry>(",
        "Ok(Box::new(authenticated))",
        "Ok(authenticated)",
        "cold recovered phase owner handoff",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
        "fn prepare_recovered_lifecycle_sign_completion_with_body<'executor>(",
        ".prepare_recovered_lifecycle_sign_completion_with_body(permit, completion)",
        ".prepare_recovered_lifecycle_sign_completion(completion)",
        "single-preview recovered next-Vote body service join must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
        "fn capture_recovered_lifecycle_proposal_exact_output(",
        "if self.proposal_work_retired",
        "if false",
        "recovered Proposal output must remain terminal after Decision",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
        "fn capture_recovered_lifecycle_proposal_exact_output(",
        "identity.same_instance(&body_store_identity)",
        "true",
        "recovered Proposal exact-output capture must retain its body-store owner",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
        "fn capture_recovered_lifecycle_proposal_exact_output(",
        "Arc::ptr_eq(&self.output_guard, &authority_output_guard)",
        "true",
        "recovered Proposal exact-output capture must retain its output guard",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
        "fn capture_recovered_lifecycle_proposal_exact_output(",
        "RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(\n                retry_authority,\n            )",
        "RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(unreachable!())",
        "recovered Proposal capacity retry must remain source-token guarded",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_exact_output.rs",
        "fn prepare_atomic_fanout_batch(",
        "if !self.ownership_capacity_available(&additions)?",
        "if false",
        "atomic Proposal fanout preflight must preserve aggregate capacity",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_exact_output.rs",
        "fn prepare_atomic_fanout_batch(",
        "aggregate.checked_add(count)",
        "aggregate.saturating_add(count)",
        "atomic Proposal fanout preflight must preserve aggregate capacity",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
        "fn capture_recovered_lifecycle_proposal_exact_output(",
        "proposal\n            .validate(&self.context)",
        "Ok::<(), String>(())\n            .map_err(|error| error.to_string())",
        "retry-safe recovered Proposal exact-output capture omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker/effect_services_impl.rs",
        "fn broadcast_consensus(",
        "self.enqueue_atomic_fanout_batch_while_guarded(",
        "self.enqueue_exact_fanout_while_guarded(",
        "live Proposal output must not split control from chunk ownership",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "fn authenticate_recovered_lifecycle_next_vote_body_catalogs(",
        "durable_bodies.get(&key) != Some(durable)",
        "false",
        "exact recovered next-Vote body catalog join omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn consume_for_adapter(",
        "body_store_identity.same_instance(expected_body_store_identity)",
        "true",
        "opaque recovered next-Vote body authority omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn project_broadcast_and_sign_authority(",
        "self.adapter.authenticate_recovered_lifecycle_next_vote(",
        "self.adapter.trust_recovered_lifecycle_next_vote(",
        "affine recovered Broadcast-and-next-Sign adapter projection must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_replay_authority.rs",
        "fn into_candidate_projection(",
        "self.wal_identity.is_exact()",
        "true",
        "full executable recovered next-WAL-Vote candidate must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_wal_recovery.rs",
        "fn project_cold_adapter_replay_authority(",
        "self.cold_adapter_authority_minted = true",
        "self.cold_adapter_authority_minted = false",
        "affine recovered Broadcast-and-next-Sign cold adapter projection must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_wal_recovery.rs",
        "fn owns_spliced_candidates(",
        "candidates.get(&self.broadcast.candidate.key) == Some(&self.broadcast.candidate)",
        "true",
        "combined cold census must retain the exact Broadcast without claiming unrelated carriers",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_replay_authority.rs",
        "fn project_cold_adapter_next_sign(",
        "self.is_exact(verified)",
        "true",
        "sealed recovered next-WAL-Vote cold adapter projection must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn advance_recovered_lifecycle_signed_broadcast_and_sign(",
        "verified.verify_consensus_message(message)",
        "Ok::<(), AdapterError>(())",
        "recovered Broadcast-and-next-Sign cold adapter replay must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "impl RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1",
        "wire::GlobalPhase::Commit => tag.view() >= next_vote.round.view",
        "wire::GlobalPhase::Commit => tag.view() == next_vote.round.view",
        "opaque recovered Broadcast-and-next-Sign cold adapter authority omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn advance_recovered_lifecycle_signed_broadcast_and_sign(",
        "replayed_next_sign != next_sign",
        "false",
        "recovered Broadcast-and-next-Sign cold adapter replay must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
        "fn exactly_matches_ledger(&self, ledger: &LifecycleLedgerV1) -> bool {",
        "project_recovered_lifecycle_signed_broadcast_and_sign_at(self.broadcast_ordinal)",
        "project_recovered_lifecycle_signed_broadcast_and_sign_at(0)",
        "combined Broadcast-and-next-Sign reauthentication must retain the exact ordinal",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_body_pipeline_transition.rs",
        "fn stage_recovered_lifecycle_sign_broadcast_and_sign_transition(",
        ".checked_add(1)",
        ".checked_add(0)",
        "inert recovered Broadcast-and-next-Sign coordinator staging must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_body_pipeline_transition.rs",
        "impl PreparedRecoveredLifecycleSignBroadcastAndSignTransition<'_, '_, '_> {",
        "ready_index.remove(&broadcast_ordinal)",
        "ready_index.remove(&next_sign_ordinal)",
        "durable recovered Proposal Broadcast-and-next-Sign publication must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_body_pipeline_transition.rs",
        "impl PreparedRecoveredLifecycleSignBroadcastAndSignTransition<'_, '_, '_> {",
        "adapter.commit_after_durable_broadcast_and_sign()",
        "drop(adapter)",
        "durable recovered Proposal Broadcast-and-next-Sign publication must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_body_pipeline_transition.rs",
        "impl PreparedRecoveredLifecycleSignBroadcastAndSignTransition<'_, '_, '_> {",
        "adapter.commit_after_durable_vote_broadcast_and_sign()",
        "drop(adapter)",
        "durable recovered Broadcast-and-next-Sign publication must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn commit_after_durable_broadcast_and_sign(self)",
        "proposal_output_authority_minted: true",
        "proposal_output_authority_minted: _",
        "durable recovered Proposal adapter two-child commit must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn settle_recovered_lifecycle_proposal_broadcast_and_sign(",
        "transition.persist_exact_successor().is_err()",
        "false",
        "restart-closed recovered Proposal Broadcast-and-next-Sign settlement must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn settle_recovered_lifecycle_proposal_broadcast_and_sign(",
        "output.abort_before_publication()",
        "drop(output)",
        "typed recovered Proposal pre-fsync output release must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn settle_recovered_lifecycle_vote_broadcast_and_sign(",
        "preview.is_vote_broadcast_and_sign_shape()",
        "preview.is_vote_broadcast_and_sign()",
        "restart-closed recovered Vote Broadcast-and-next-Sign settlement must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn settle_recovered_lifecycle_vote_broadcast_and_sign(",
        "transition.persist_exact_successor().is_err()",
        "false",
        "restart-closed recovered Vote Broadcast-and-next-Sign settlement must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "fn dispatch_completion_with_runner_debt_and_required_ordinal(",
        "registration.commit(prepared, wait_source)",
        "registration.abort(prepared)",
        "lifecycle-owned recovered Decision Fetch dispatch must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "fn begin_fetch<S: V2EffectServices>(",
        "owner.matches_body_coordinates(round, subject)",
        "false",
        "ordinary and recovered Decision Fetch coordinate fence omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_body_persistence(",
        "self.revalidate_recovered_decision_fetch_response_candidate(",
        "self.trust_recovered_decision_fetch_response_candidate(",
        "typed recovered Decision Fetch selector consumption must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "pub(in crate::sumeragi) fn commit_with_queue(",
        "owner.commit_exact_response_claim(response_hash)",
        "true",
        "recovered Decision Fetch response claim publication must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
        "fn take_io_completion(&mut self, runtime_capacity_available: bool)",
        "owned.recovered_decision_fetch.is_some()",
        "false",
        "recovered Decision Fetch mixed completion head fence must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn settle_recovered_decision_fetch_store(",
        "transition.persist_exact_successor().is_err()",
        "false",
        "restart-closed recovered Decision Fetch-to-Store settlement must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn settle_recovered_decision_fetch_store(",
        "locked_dequeue.commit()",
        "drop(locked_dequeue)",
        "restart-closed recovered Decision Fetch-to-Store settlement must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
        "fn open_recovered_decision_store_startup(",
        ".authenticate_recovered_decision_fetch_store(&projection, &store_projection)",
        ".trust_recovered_decision_fetch_store(&projection, &store_projection)",
        "recovered Decision Store cold restart and marker-prefix closure omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "pub(in crate::sumeragi) fn advance_recovered_decision_fetch_store(",
        ".project_store_adapter_authority(body)",
        ".trust_store_adapter_authority(body)",
        "recovered Decision Store cold adapter reconstruction omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_transport.rs",
        "pub(in crate::sumeragi) fn authenticate_response(",
        "authenticate_certified_body_response_for_request(",
        "authenticate_certified_body_response_without_request(",
        "request-scoped certified response authentication omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
        "fn matches_successor_owner_ledger(",
        ".validate_authenticated_cut(&owner.serve_payloads)",
        ".validate_authenticated_cut_for_mutation(&owner.serve_payloads)",
        "CompleteTip canonical predecessor store join omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_open.rs",
        "pub(super) fn into_serve_payloads(self)",
        "pub(super) fn into_serve_payloads(self)",
        "pub(super) fn into_unsealed_payloads(self)",
        "CompleteTip bodyless completion promotion guard omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_certified_serve_payload_store.rs",
        "pub(super) fn validate_authenticated_cut(",
        "let observed = self.reload_payload_census_strict()?;",
        "let observed = BTreeMap::new();",
        "CompleteTip body-independent Completed metadata authority omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_certified_serve_payload_store.rs",
        "fn reload_payload_census_strict(",
        "fs::read_dir(&self.directory)",
        "fs::read_dir(temporary_path_for_mutation)",
        "CompleteTip Serve payload directory census must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_certified_serve_payload_store.rs",
        "fn reload_payload_census_strict(",
        "fs::symlink_metadata(&self.directory)",
        "fs::metadata(&self.directory)",
        "CompleteTip Serve payload directory census must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_certified_serve_payload_store.rs",
        "fn reload_payload_census_strict(",
        "self.load_path(&path, metadata.len())?",
        "return Ok(payloads);",
        "CompleteTip Serve payload directory census must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs",
        "pub(crate) struct ProductionLifecycleOwnerV1",
        "serve_payloads: crate::sumeragi::v2_certified_serve_payload_store::AuthenticatedCertifiedServePayloadRecoveryCut,",
        "serve_payloads: (),",
        "production lifecycle owner retained Serve census omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs",
        "fn run_complete_tip_retirement_release_regressions()",
        "ledger::tests::durable_ready_fetch_recovery::complete_tip_retirement_binds_only_the_exact_unlaunched_successor_owner();",
        "let _ = ();",
        "production lifecycle owner retained Serve census omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_first_release_recovery.rs",
        "pub(crate) use super::v2_lifecycle_coordinator::{",
        "run_complete_tip_retirement_release_regressions",
        "run_unchecked_complete_tip_retirement_release_regressions",
        "CompleteTip first-release recovery seam omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
        "fn authorizes_successor_status(",
        "self.complete_tip.successor_context_id() == successor.height_context_id",
        "true",
        "CompleteTip restart publication authority must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
        "pub(in crate::sumeragi) struct BoundRecoveredCompleteTipSuccessorOwnerV1 {",
        "#[cfg(test)]\nimpl BoundRecoveredCompleteTipSuccessorOwnerV1 {",
        "impl BoundRecoveredCompleteTipSuccessorOwnerV1 {\n"
        "    pub(in crate::sumeragi) fn into_owner(self) -> ProductionLifecycleOwnerV1 { self.owner }\n"
        "}\n\n"
        "#[cfg(test)]\nimpl BoundRecoveredCompleteTipSuccessorOwnerV1 {",
        "CompleteTip exact H+1 owner bind must use the opaque checked-transition gate",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_recovery.rs",
        "pub(in crate::sumeragi) fn authorizes(\n        self,",
        "self.kura_identity.matches(kura)",
        "true",
        "recovered lifecycle storage authority handoff omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "pub(in crate::sumeragi) fn mint_from_recovered_height(",
        "assert!(permit.authorizes(kura, verified, signature_policy, genesis_account));",
        "assert!(true);",
        "recovery-minted lifecycle storage authority omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) struct ProductionLifecycleLaunchInputsV1 {",
        "authenticated_genesis: Option<AuthenticatedGenesisBodyV1>,",
        "authenticated_genesis: Option<AuthenticatedGenesisBodyV1>,\n    genesis_account: AccountId,",
        "move-only authenticated genesis launch input must use the opaque checked-transition gate",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn launch(\n        mut self,",
        "binding.matches_launch_identity(inputs.kura.as_ref(), &inputs.key_pair)",
        "true",
        "Kura-bound production lifecycle launch must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn launch(\n        mut self,",
        "super::authority::lifecycle_ordinal_authorities_after_high_watermark",
        "RuntimeLifecycleOrdinalSource::after_high_watermark",
        "Kura-bound production lifecycle launch must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn launch(\n        mut self,",
        "RuntimeLifecycleOrdinalSource::from_authority(runtime_ordinal_authority)",
        "RuntimeLifecycleOrdinalSource::after_high_watermark(0)",
        "Kura-bound production lifecycle launch must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn launch(\n        mut self,",
        ".bind_live_lifecycle_ordinal_authority(coordinator_ordinal_authority)",
        ".discard_live_lifecycle_ordinal_authority(coordinator_ordinal_authority)",
        "Kura-bound production lifecycle launch must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn launch_local_identity_matches(",
        "local_peer.public_key() != key_pair.public_key()",
        "false",
        "local launch identity preflight omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn launch_local_identity_matches(",
        "local_validator.is_none_or(|observed| roster_position == Some(observed))",
        "local_validator.is_none_or(|_| true)",
        "local launch identity preflight omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(",
        "self.adapter.wal.matches_path(&storage.wal_path)",
        "true",
        "canonical Kura-bound lifecycle-owner factory must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(",
        "Arc::ptr_eq(&adapter_owner, &self.factory_owner)",
        "true",
        "canonical Kura-bound lifecycle-owner factory must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(",
        "body_store: super::v2_body_store::QuarantinedV2BodyStore",
        "body_store: super::v2_body_store::V2BodyStore",
        "canonical Kura-bound lifecycle-owner factory must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(",
        ".into_revalidated_lifecycle_startup(",
        ".into_revalidated_startup(",
        "canonical Kura-bound lifecycle-owner factory must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_body_store.rs",
        "pub(in crate::sumeragi) fn into_quarantined_recovered_startup(",
        "!self.validated.is_empty()",
        "false",
        "fresh quarantined recovered body-store cut omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_body_store.rs",
        "pub(in crate::sumeragi) fn into_revalidated_lifecycle_startup(",
        "apply_service.recovered_finality_subject(context)",
        "None::<VerifiedRecoveredFinalitySubject>.ok_or(())?",
        "fixed quarantined recovered marker replay must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_body_store.rs",
        "pub(in crate::sumeragi) fn into_revalidated_lifecycle_startup(",
        ".retain_recovered_markers_for_authority(validation_authority)",
        ".retain_recovered_markers_for_mutation(validation_authority)",
        "fixed quarantined recovered marker replay must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_body_store.rs",
        "pub(in crate::sumeragi) fn into_revalidated_lifecycle_startup(",
        ".revalidate_recovered_markers(|body|",
        ".retain_recovered_markers_for_mutation(|body|",
        "fixed quarantined recovered marker replay must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs",
        "pub(in crate::sumeragi) fn bind_production_lifecycle_owner_factory_inputs_v1(",
        "state.matches_kura_instance(&kura)",
        "true",
        "recovery-minted lifecycle storage authority omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn launch(\n        mut self,",
        "ProductionV2Services::start_with_apply_service(",
        "ProductionV2Services::start(",
        "Kura-bound production lifecycle launch must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn launch(\n        mut self,",
        "ProductionLifecycleApplyServiceLaunchPermitV1 {",
        "ForgedProductionLifecycleApplyServiceLaunchPermitV1 {",
        "sealed replay-service permit mint must contain",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs",
        "pub(in crate::sumeragi) fn with_recovered_kura_binding_and_apply_service(",
        "self.apply_service = Some(apply_service);",
        "drop(apply_service);",
        "production lifecycle owner Kura seal omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
        "pub(in crate::sumeragi) fn start_with_apply_service(",
        "apply_service.matches_lifecycle_launch(&state, &kura, &context, &validator_set_pops)",
        "true",
        "sealed replay-service worker transfer omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn launch(\n        mut self,",
        "let payload_store_identity = self.payload_store.instance_identity();",
        "let payload_store_identity = self.body_store_identity.clone().unwrap();",
        "Kura-bound production lifecycle launch must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn launch(\n        mut self,",
        "payload_store_identity.clone(),",
        "body_store_identity.clone(),",
        "Kura-bound production lifecycle launch must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn launch(\n        mut self,",
        "|| !services.matches_lifecycle_payload_store(&payload_store_identity)",
        "|| false",
        "Kura-bound production lifecycle launch must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
        "pub(in crate::sumeragi) fn start_with_apply_service(",
        "            Some(payload_store_identity),",
        "            None,",
        "recovered startup must transfer the exact Certified-Serve payload-store identity",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
        "pub(in crate::sumeragi) fn matches_lifecycle_payload_store(",
        "service_identity.same_instance(owner_identity)",
        "true",
        "the live service must require its retained exact Certified-Serve payload-store instance",
    ),
    (
        "crates/iroha_core/src/state.rs",
        "pub(crate) fn matches_kura_instance(",
        "Arc::ptr_eq(&self.kura, kura)",
        "true",
        "fixed State/Kura identity oracle omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "pub(in crate::sumeragi) fn matches_lifecycle_launch(",
        "Arc::ptr_eq(&self.state, state)",
        "true",
        "fixed recovered Apply-service identity oracle omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "pub(in crate::sumeragi) fn prepare_leader_wire_launch(",
        "adapter.wal.matches_path(expected_wal_path)",
        "true",
        "sealed adapter leader-wire launch projection omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/safety_wal.rs",
        "fn publish_atomic(&self, frame: &[u8], maximum: u64, label: &str)",
        "let durable = rustix::fs::statat(",
        "let durable = promoted;",
        "opened safety-WAL directory authority omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs",
        "pub(crate) fn open_with_safety_wal_authority(\n"
        "        storage: SafetyWalServicedCandidateStoreAuthority,",
        "storage: SafetyWalServicedCandidateStoreAuthority",
        "storage: SafetyWalLeaderWireStoreAuthority",
        "typed WAL-adjacent production stores omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "pub(in crate::sumeragi) fn prepare_leader_wire_launch(",
        "*leader_wire_launch_prepared = true;",
        "let _ = leader_wire_launch_prepared;",
        "sealed adapter leader-wire launch projection omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "pub(in crate::sumeragi) fn open_gate(",
        "body_store\n            .recovery_catalog()",
        "BTreeMap::new()",
        "sealed adapter leader-wire launch projection omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn launch(\n        mut self,",
        "leader_wire_launch.restored_producer_ordinal_high_watermark()",
        "None",
        "Kura-bound production lifecycle launch must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn launch(\n        mut self,",
        "leader_wire_restore.scheduler_ordinal_high_watermark()",
        "0",
        "Kura-bound production lifecycle launch must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn retire(&mut self) -> Result<(), String>",
        "self.ingress.retire_leader_wire_lifecycle_gate(&gate)?",
        "self.gate = None;",
        "sealed leader-wire launch binding omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_context.rs",
        "pub fn freeze_staged_genesis_v2(",
        "let authenticated_genesis = AuthenticatedGenesisBodyV1::authenticate(genesis)?;",
        "let authenticated_genesis = forged_authenticated_genesis;",
        "signed genesis bootstrap seal mint omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_context.rs",
        "pub struct GenesisV2Bootstrap {",
        "pub struct GenesisV2Bootstrap {",
        "#[derive(Debug, Clone)]\npub struct GenesisV2Bootstrap {",
        "move-only authenticated genesis bootstrap must use the opaque checked-transition gate",
    ),
    (
        "crates/iroha_core/src/sumeragi/mod.rs",
        "pub struct GenesisWithPubKey {",
        "pub struct GenesisWithPubKey {",
        "#[derive(Debug, Clone)]\npub struct GenesisWithPubKey {",
        "move-only genesis runner bundle must use the opaque checked-transition gate",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_recovery.rs",
        "pub(crate) fn recover_active_height_with_plan(",
        "if !authenticated_genesis.authorizes(&genesis_public_key) {",
        "if false {",
        "recovery-sealed fresh genesis handoff omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_recovery.rs",
        "pub(crate) fn recover_active_height_with_plan(",
        "authenticated_genesis: Some(authenticated_genesis),",
        "authenticated_genesis: None,",
        "recovery-sealed fresh genesis handoff omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "pub(in crate::sumeragi) fn launch(\n        mut self,",
        ".install_authenticated_genesis_body(authenticated_genesis)",
        ".install_authenticated_genesis_body(forged_genesis_body_for_mutation)",
        "move-only authenticated genesis launch input omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
        "fn authorizes_retained_successor(",
        "self.successor_store.load().ok().as_ref() == Some(&self.successor_ledger)",
        "true",
        "CompleteTip restart publication authority must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "fn open_ingress_for_active_height(",
        "output_guard.begin_fail_stop_operation()",
        "output_guard.acquire()",
        "open_ingress_for_active_height must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn run_non_pending_lifecycle_loop(",
        "SumeragiV2Adapter::open_recovered_startup_with_capacity_geometry(",
        "SumeragiV2Adapter::open_recovered_startup(",
        "non-pending lifecycle live successor startup must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn run_non_pending_lifecycle_loop(",
        "open_production_lifecycle_owner_v1(",
        "open_unchecked_production_lifecycle_owner_v1(",
        "non-pending lifecycle live successor startup must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "fn initial_block_sync_deadline(",
        "if eager_recovery {\n        height_started_at\n    } else {",
        "if eager_recovery {\n"
        "        deadline_after(height_started_at, round_timeout)\n"
        "    } else {",
        "recovery-scoped eager block-sync initial_block_sync_deadline "
        "declaration and complete control flow",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn run_lifecycle_active_height(",
        "admitted_discovered_commit_qc = true;",
        "admitted_discovered_commit_qc = false;",
        "only authenticated discovered CommitQC admission/coalescing may retain eager block-sync",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn run_lifecycle_active_height(",
        "retain_eager_block_sync(false, admitted_discovered_commit_qc)",
        "retain_eager_block_sync(true, admitted_discovered_commit_qc)",
        "ordinary lifecycle successor handoff must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "const fn retain_eager_block_sync(",
        "recovering_interrupted_tip || admitted_discovered_commit_qc",
        "{ let _ = admitted_discovered_commit_qc; recovering_interrupted_tip }",
        "recovery-scoped eager block-sync retain_eager_block_sync "
        "declaration and complete control flow",
    ),
    (
        "crates/iroha_core/src/sumeragi/status.rs",
        "fn publish_recovered_v2_successor_height_at(",
        "set_v2_status_at(successor, now);",
        "update_v2_successor_work_stage_at(finalized_height, SumeragiV2LocalWorkStage::Running, SumeragiV2LocalWorkStage::Complete, now)?; set_v2_status_at(successor, now);",
        "may not fabricate physical predecessor completion",
    ),
    (
        "crates/iroha_core/src/sumeragi/status.rs",
        "fn activate_v2_successor_height_at(",
        "validate_v2_predecessor_status(\n"
        "        &predecessor_status,\n"
        "        finalized_height,\n"
        "        SumeragiV2LocalWorkStage::Running,\n"
        "    )?;",
        "let _ = &predecessor_status;",
        "activate_v2_successor_height_at omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/status.rs",
        "fn activate_v2_successor_height_at(",
        "predecessor_status_height: predecessor_status.height,",
        "predecessor_status_height: finalized_height,",
        "activate_v2_successor_height_at omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/status.rs",
        "fn activate_v2_successor_height_at(",
        "update_v2_successor_work_stage_at(\n"
        "        finalized_height,\n"
        "        SumeragiV2LocalWorkStage::Running,\n"
        "        SumeragiV2LocalWorkStage::Complete,\n"
        "        now,\n"
        "    )?;",
        "update_v2_successor_work_stage_at(finalized_height, SumeragiV2LocalWorkStage::Running, SumeragiV2LocalWorkStage::Running, now)?;",
        "activate_v2_successor_height_at omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/status.rs",
        "fn activate_v2_successor_height_at(",
        "let _authorized_trace = checked_trace.into_projection();",
        "drop(checked_trace);",
        "activate_v2_successor_height_at omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/status.rs",
        "pub(crate) fn begin_v2_successor_activation(",
        "let _authorized_lifecycle = checked_lifecycle.into_projection();\n"
        "    update_v2_successor_work_stage_at(\n"
        "        height,\n"
        "        SumeragiV2LocalWorkStage::Queued,\n"
        "        SumeragiV2LocalWorkStage::Running,\n"
        "        Instant::now(),\n"
        "    )",
        "let mutation_result = update_v2_successor_work_stage_at(\n"
        "        height,\n"
        "        SumeragiV2LocalWorkStage::Queued,\n"
        "        SumeragiV2LocalWorkStage::Running,\n"
        "        Instant::now(),\n"
        "    );\n"
        "    let _authorized_lifecycle = checked_lifecycle.into_projection();\n"
        "    mutation_result",
        "begin_v2_successor_activation must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/status.rs",
        "fn publish_recovered_v2_successor_height_at(",
        "published_status_height_before: published.as_ref().map_or(0, |status| status.height),",
        "published_status_height_before: 0,",
        "publish_recovered_v2_successor_height_at omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/status.rs",
        "fn publish_recovered_v2_successor_height_at(",
        "if let Some(published) = published {\n"
        "        return Err(V2SuccessorActivationError::RecoveredStatusAlreadyPublished(\n"
        "            published.height,\n"
        "        ));\n"
        "    }\n"
        "    set_v2_status_at(successor, now);",
        "set_v2_status_at(successor, now);",
        "publish_recovered_v2_successor_height_at must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/status.rs",
        "pub(crate) fn begin_v2_successor_activation(",
        "stage_before: successor_stage_projection(status.liveness.work.successor_height),",
        "stage_before: SUCCESSOR_STAGE_QUEUED,",
        "begin_v2_successor_activation omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/status.rs",
        "pub(crate) fn mark_v2_restart_required()",
        '"Sumeragi v2 Running successor failure projection was rejected; preserving the unchecked status"\n'
        "                );\n"
        "                return;",
        '"Sumeragi v2 Running successor failure projection was rejected; preserving the unchecked status"\n'
        "                );",
        "mark_v2_restart_required must contain 'return;' exactly 2 time(s)",
    ),
    (
        "crates/iroha_core/src/sumeragi/status.rs",
        "pub(crate) fn mark_v2_restart_required()",
        "check_production_successor_startup_lifecycle_transition(lifecycle)",
        "Some(lifecycle)",
        "mark_v2_restart_required omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn recovered(\n        authority: RecoveredSuccessorActivationAuthority,",
        "let published_height = super::super::status::v2_status().map_or(0, |status| status.height);",
        "let published_height = 0;",
        "PendingSuccessorActivation omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "fn recovered(\n        authority: RecoveredSuccessorActivationAuthority,",
        "let Some(checked_lifecycle) =\n"
        "            check_production_successor_startup_lifecycle_transition(lifecycle)\n"
        "        else {\n"
        "            return Err(V2RunnerError::SuccessorRefinementRejected);\n"
        "        };\n"
        "        let _authorized_lifecycle = checked_lifecycle.into_projection();",
        "if !production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(\n"
        "            lifecycle,\n"
        "        ) {\n"
        "            return Err(V2RunnerError::SuccessorRefinementRejected);\n"
        "        }",
        "must use the opaque checked-transition gate; found obsolete direct-kernel forms",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "fn bind(\n        self,",
        "authority_predecessor: authority.predecessor().refinement_projection(),",
        "authority_predecessor: self.predecessor.refinement_projection(),",
        "PendingSuccessorConstruction omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_recovery.rs",
        "pub(crate) fn authenticate(\n        artifact: &wire::finality::V2FinalityArtifact,",
        "|| receipt.certificate() != artifact.commit_qc.as_ref()",
        "|| false",
        "DurableV2PredecessorIdentity::authenticate omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_recovery.rs",
        "pub(crate) fn authenticate(\n        artifact: &wire::finality::V2FinalityArtifact,",
        "if !production_durable_predecessor_identity_kernel(identity.refinement_projection()) {",
        "if false {",
        "DurableV2PredecessorIdentity::authenticate omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_recovery.rs",
        "fn new(record: &wire::SnapshotV2BootstrapRecord) -> Self",
        "record_hash: HashOf::new(record),",
        "record_hash: HashOf::new(&wire::SnapshotV2BootstrapRecord::default()),",
        "SnapshotSuccessorActivationAuthority::new omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_recovery.rs",
        "pub(crate) fn recover_active_height_with_plan(",
        "if record.context() != &bootstrap.context\n"
        "            || record.proofs_of_possession() != bootstrap.validator_set_pops",
        "if false",
        "recover_active_height_with_plan snapshot authority omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_recovery.rs",
        "pub(crate) fn recover_active_height_with_plan(",
        "v2_finality_artifact_with_receipt(durable_height)",
        "v2_finality_artifact(durable_height)",
        "recover_active_height_with_plan complete-tip authority omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "    fn register_parent_qc(",
        "if !reference.same_commit_decision(frozen) {",
        "if false {",
        "WireRegistry::register_parent_qc omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "    fn justification_to_core(",
        ".map(|certificate| self.register_parent_qc(certificate))",
        ".map(|certificate| self.qc_reference_to_core(&certificate.as_ref()))",
        "WireRegistry::justification_to_core omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn verify_proposal_justification_authority(",
        "(Some(certificate), Some(parent_verification)) => verify_quorum_certificate(",
        "(Some(_), Some(_)) => Ok(",
        "verify_proposal_justification_authority omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_block_sync.rs",
        "fn build_historical_body_response(",
        ".position(|entry| entry.validator == responder_peer)",
        ".any(|entry| entry.validator == responder_peer)",
        "build_historical_body_response must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "pub(crate) struct SumeragiV2Adapter {",
        "status_publication_enabled: bool,",
        "status_publication_enabled_removed: bool,",
        "SumeragiV2Adapter status publication latch omits production refinement tokens",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn open_with_aggregator_and_publication_with_capacity(",
        "status_publication_enabled: publish_initial_status,",
        "status_publication_enabled: true,",
        "deferred status publication latch must initialize from publish_initial_status",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "impl PreparedReadyDurableValidatePersistedSign<'_> {",
        "if self.adapter.status_publication_enabled {",
        "if true {",
        "Ready-Validate direct status publication must remain latch-dominated",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "fn publish_status(&mut self)",
        "if self.status_publication_enabled {",
        "if true {",
        "adapter status publication must compute before its latch-dominated global setter",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "pub(crate) fn successor_activation_status(",
        "let status = self.status()?;\n        self.status_publication_enabled = true;",
        "self.status_publication_enabled = true;\n        let status = self.status()?;",
        "successor activation may enable status publication only after a successful snapshot",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "pub(in crate::sumeragi) fn pending_kura_activation_status(",
        "let status = self.status()?;\n        self.status_publication_enabled = true;",
        "self.status_publication_enabled = true;\n        let status = self.status()?;",
        "PendingKura activation may enable status publication only after a successful snapshot",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runtime.rs",
        "pub(crate) fn pending_kura_activation_status_snapshot(",
        "self.driver.pending_kura_activation_status()",
        "self.driver.status()",
        "pending_kura_activation_status_snapshot must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_pending_kura.rs",
        "pub(in crate::sumeragi) fn activate_no_clock(",
        ".open_and_publish_recovered_height(",
        ".open_recovered_height(",
        "PendingKura activation status-before-ingress boundary must preserve exact production order",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "fn settle_lifecycle_decision_apply_completion_owner(",
        "super::super::status::set_v2_status(status);",
        "if false { super::super::status::set_v2_status(status); }",
        "lifecycle Decision Apply settlement must preserve its intentional unguarded final publication",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(super) fn classify_selected_certified_response_priority(",
        "occurrence.queue_gate() == FairV2IngressQueueGateVerdict::Blocked",
        "occurrence.queue_gate() != FairV2IngressQueueGateVerdict::Blocked",
        "closed selected certified-response priority census",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(super) fn classify_selected_certified_response_priority(",
        "let mut selected_priority = SelectedCertifiedResponsePriorityV1::DefinitelyNonPriority;",
        "let mut selected_priority = SelectedCertifiedResponsePriorityV1::OrdinaryClaimed;",
        "closed selected certified-response priority census",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(super) fn classify_selected_certified_response_priority(",
        "Err(error) if response_error_is_remote_nonpriority(&error) => continue,",
        "Err(_) => continue,",
        "closed selected certified-response priority census",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(super) fn classify_selected_certified_response_priority(",
        ".insert(occurrence.physical_admission_ordinal(), candidate)\n                .is_some()",
        ".insert(occurrence.physical_admission_ordinal(), candidate)\n                .is_none()",
        "closed selected certified-response priority census",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(super) fn classify_selected_certified_response_priority(",
        "if !exact {",
        "if false {",
        "closed selected certified-response priority census",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ingress_position.rs",
        "pub(super) fn into_ordinary_turn_cut(",
        "bound_context: Some(bound_context),",
        "bound_context: None,",
        "exact current-context cut widening",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ingress_position.rs",
        "pub(super) fn into_ordinary_turn_cut(",
        ".position(|source| source == selected_source)",
        ".rposition(|source| source == selected_source)",
        "exact current-context cut widening",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "fn exactly_matches_fresh_staged_append(",
        "producer != staged.high_water",
        "producer == staged.high_water",
        "gap-aware fresh Serve staged append",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "fn exactly_matches_fresh_staged_append(",
        "current.admission_waits != staged.admission_waits",
        "current.admission_waits == staged.admission_waits",
        "gap-aware fresh Serve staged append",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "fn exactly_matches_fresh_staged_append(",
        "carrier.matches_record(record, metadata, work.digest)",
        "true",
        "gap-aware fresh Serve staged append",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "fn exactly_matches_fresh_staged_append(",
        "serve_used.checked_add(1)",
        "serve_used.checked_add(2)",
        "gap-aware fresh Serve staged append",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "pub(in crate::sumeragi) fn commit(self) -> PendingKuraValidatedApplySuccessorV1 {",
        "adapter.reducer = next_reducer;",
        "drop(next_reducer);",
        "pending-Kura deferred validation commit",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "pub(in crate::sumeragi) fn consume_for_executor(",
        "_permit: crate::sumeragi::v2_effects::PendingKuraApplySuccessorExecutorPermitV1,",
        "_permit: (),",
        "pending-Kura executor-only Apply child release",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "pub(crate) fn verify_pending_kura_apply_replay(",
        "verify_pending_kura_apply_parts_with_marker(",
        "verify_pending_kura_apply_parts(",
        "pending-Kura replay verification with deferred marker",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_runtime.rs",
        "pub(in crate::sumeragi) fn prepare_pending_kura_validated_apply(",
        "|| self.clocks_armed",
        "|| false",
        "pending-Kura no-clock marker preparation",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "pub(crate) trait EffectRuntime {",
        '"runtime cannot commit a deferred pending-Kura validation marker"',
        '"runtime accepted a fabricated pending-Kura validation marker"',
        "generic runtime pending-Kura marker fail-closed default",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "impl EffectRuntime for SerializedV2Runtime {",
        "Ok(prepared) => Ok(prepared.commit()),",
        "Ok(prepared) => Err((prepared.into_marker(), String::new())),",
        "serialized pending-Kura marker commit",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects_lifecycle_admission_settlement.rs",
        "fn validate_body<S: V2EffectServices>(",
        ".take_deferred_validated_marker()?;",
        ".take_deferred_validated_marker_for_mutation()?;",
        "pending-Kura Validate exact Apply child",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "fn consume_one<S: V2EffectServices>(",
        "result?;",
        "result_for_mutation?;",
        "pending-Kura stage-before-child dispatch",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "pub(crate) fn step_pending_tip_recovery<S: V2EffectServices>(",
        "self.consume_pending_tip_recovery_effects(effects, services)?;",
        "self.consume_effects(effects, services)?;",
        "pending-Kura exact local stage consumer",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_pending_kura.rs",
        "pub(in crate::sumeragi) fn drive_apply_recovery_turn(",
        "executor.step_pending_tip_recovery(Instant::now(), services)?",
        "executor.step_effects(Instant::now(), services)?",
        "bounded closed-ingress pending-Kura direct-pipeline turn",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_pending_kura.rs",
        "fn drive_apply_recovery_turn(",
        "if stage == Stage::ApplicationDispatched =>",
        "if true =>",
        "one-item closed-ingress pending-Kura lifecycle Apply completion turn",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "fn prepare_lifecycle_decision_apply_executor_dispatch",
        "prepared.exactly_matches_pending_kura_recovery(",
        "prepared.matches_pending_kura_recovery(",
        "exact pending-Kura lifecycle Apply executor dispatch preflight",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "fn commit_after_worker_dispatch(self)",
        "pending.evidence.stage = PendingKuraApplyRecoveryStage::ApplicationDispatched;",
        "pending.evidence.stage = PendingKuraApplyRecoveryStage::Completed;",
        "physical lifecycle Apply publication advances pending stage once",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn classify_and_defer_validated_marker(",
        "self.expected.height() != context.height",
        "self.expected.height() == context.height",
        "move-only pending-Kura marker deferral",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        "fn classify_and_defer_validated_marker(",
        "certificate.proposal_round != *round",
        "certificate.proposal_round == *round",
        "move-only pending-Kura marker deferral",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects_lifecycle_admission_settlement.rs",
        "fn validate_body<S: V2EffectServices>(",
        "self.ensure_pending_slot()?;",
        "let _ = self.remaining_capacity();",
        "direct pending-Kura Validate-to-Apply child",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects_lifecycle_admission_settlement.rs",
        "fn validate_body<S: V2EffectServices>(",
        ".restore_deferred_validated_marker(marker);",
        ".discard_deferred_validated_marker(marker);",
        "direct pending-Kura Validate-to-Apply child",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "pub(crate) trait EffectRuntime {",
        "runtime cannot commit a deferred pending-Kura validation marker",
        "runtime accepted a deferred pending-Kura validation marker",
        "fail-closed generic pending-Kura validation hook",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "fn consume_one<S: V2EffectServices>(",
        "result?;",
        "let _ = result;",
        "pending-Kura outer-stage-before-direct-child dispatch",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        "pub(crate) struct CertifiedFetchBodyPersistencePreLedgerRestartError {",
        "self.completion.work_id()",
        "EffectWorkId::new(0)",
        "certified Fetch pre-Ledger restart owner",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "fn settle_parked_certified_fetch_body_persistence(",
        "work_id = error.work_id().get(),",
        "work_id = 0,",
        "certified Fetch Phase-B RestartRequiredBeforeLedger branch",
    ),
    (
        "scripts/run_sumeragi_v2_release_gates.sh",
        "required_production_liveness_tests=(",
        "sumeragi::v2_block_sync::tests::catch_up_is_strictly_sequential_across_contexts",
        "sumeragi::v2_block_sync::tests::catch_up_is_not_release_bound",
        "production refinement test must be pinned exactly once",
    ),
)


assert len(SUCCESSOR_PRODUCTION_SOURCE_MAPPING_MUTATIONS) == len(
    set(SUCCESSOR_PRODUCTION_SOURCE_MAPPING_MUTATIONS)
) == 430


@pytest.mark.parametrize(
    ("relative_path", "region_marker", "old", "new", "error_fragment"),
    SUCCESSOR_PRODUCTION_SOURCE_MAPPING_MUTATIONS,
)
def test_successor_production_source_mapping_mutations_fail_closed(
    tmp_path: Path,
    relative_path: str,
    region_marker: str,
    old: str,
    new: str,
    error_fragment: str,
) -> None:
    module = load_checker()
    for source_name in SUCCESSOR_PRODUCTION_SOURCE_FIXTURE_FILES:
        destination = tmp_path / source_name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / source_name, destination)
    copy_reviewed_rust_include_components(tmp_path)

    baseline_errors = module._successor_production_source_fidelity_errors(tmp_path)
    assert baseline_errors == [], baseline_errors

    path = tmp_path / relative_path
    source = path.read_text(encoding="utf-8")
    region_start = source.find(region_marker)
    assert region_start >= 0
    mutation = source.find(old, region_start)
    assert mutation >= 0
    function_name = re.search(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)", region_marker)
    if function_name is not None:
        owning_items = []
        for item in module.rust_items(source, function_name.group(1)):
            item_start = source.find(item.source)
            if item_start <= region_start < item_start + len(item.source):
                owning_items.append((item_start, item))
        assert len(owning_items) == 1, (
            "region marker did not select exactly one production Rust item",
            relative_path,
            region_marker,
        )
        item_start, item = owning_items[0]
        assert item_start <= mutation < item_start + len(item.source), (
            "mutation escaped the production Rust item selected by its region marker",
            relative_path,
            region_marker,
            old,
        )
    path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )

    errors = module._successor_production_source_fidelity_errors(tmp_path)
    assert any(error_fragment in error for error in errors), errors
    if (
        relative_path == "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs"
        and region_marker == "fn authorizes_retained_successor("
    ):
        successor_reload = (
            "self.successor_store.load().ok().as_ref() "
            "== Some(&self.successor_ledger)"
        )
        assert source.count(successor_reload) == 1
        path.write_text(source.replace(successor_reload, "true", 1), encoding="utf-8")
        errors = module._successor_production_source_fidelity_errors(tmp_path)
        assert any(error_fragment in error for error in errors), errors


def test_retired_generic_runtime_recovery_symbol_fails_closed(
    tmp_path: Path,
) -> None:
    """PendingKura recovery cannot regain a generic Runtime scheduler owner."""

    module = load_checker()
    for source_name in SUCCESSOR_PRODUCTION_SOURCE_FIXTURE_FILES:
        destination = tmp_path / source_name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / source_name, destination)
    copy_reviewed_rust_include_components(tmp_path)

    runtime_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    source = runtime_path.read_text(encoding="utf-8")
    old = "    /// No live owner was ready.\n    Idle,\n}"
    new = "    /// No live owner was ready.\n    Idle,\n    RecoveryIdle,\n}"
    assert source.count(old) == 1
    baseline_errors = (
        module._lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors(
            tmp_path
        )
    )
    diagnostic = "retired generic Runtime recovery symbol RecoveryIdle"
    assert not any(diagnostic in error for error in baseline_errors)

    runtime_path.write_text(source.replace(old, new, 1), encoding="utf-8")
    errors = (
        module._lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors(
            tmp_path
        )
    )

    assert any(diagnostic in error for error in errors), errors


def test_replayed_proposal_owner_semantics_survive_digest_refresh(
    tmp_path: Path,
) -> None:
    """The externalized replay-owner regression remains a semantic seal."""

    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2Core.tla",
        "SumeragiV2InductiveProofs.tla",
    )
    for relative in (
        Path("crates/iroha_core/src/sumeragi/v2_core/wal.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_candidate.rs"),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    replay_path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/tests/v2_runner_unsealed_02.rs"
    )
    test_name = "recovered_lifecycle_proposal_attempt_suppresses_same_view_after_lock_upgrade"
    mutate_rust_item_source(
        module,
        replay_path,
        test_name,
        "recovered.exactly_matches_directive(upgraded_lock),",
        "!recovered.exactly_matches_directive(upgraded_lock),",
    )
    item = module.rust_items(
        replay_path.read_text(encoding="utf-8"), test_name
    )[0]
    module._LOCKED_BODY_REPROPOSAL_RUST_ITEM_SHA256[test_name] = (
        module._rust_item_token_sha256(item)
    )

    errors = module._locked_body_reproposal_source_fidelity_errors(
        formal_dir, tmp_path
    )

    assert any(
        "the recovered-attempt regression must prove affine same-view suppression across a lock upgrade while rejecting foreign rounds and decisions"
        in error
        for error in errors
    ), errors


def test_borrow_bound_outer_ingress_cursor_contract_is_current(tmp_path: Path) -> None:
    """The live cursor advances only when its exact current-turn borrow drops."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    runner_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_runner.rs"
    runner_source = runner_path.read_text(encoding="utf-8")
    item_errors: list[str] = []
    outer_turns = module._require_rust_item(
        runner_path, runner_source, "outer_ingress_turns", item_errors
    )

    assert item_errors == []
    assert module._outer_ingress_cursor_source_fidelity_errors(
        runner_path, runner_source, outer_turns
    ) == []


@pytest.mark.parametrize(
    ("item_name", "context", "old", "new", "expected_error"),
    (
        (
            "next_current",
            (("impl", "OuterIngressTurns"),),
            "fn next_current",
            "fn removed_next_current",
            "borrow-bound outer-ingress current-turn mint; found 0",
        ),
        (
            "next_current",
            (("impl", "OuterIngressTurns"),),
            """Some(LifecycleCurrentRunnerTurn {
            turn: self.next_turn,
            cursor: self,
        })""",
            """Some(LifecycleCurrentRunnerTurn {
            cursor: self,
            turn: self.next_turn,
        })""",
            "current-turn mint must freeze the exact next turn",
        ),
        (
            "advance_current",
            (("impl", "OuterIngressTurns"),),
            """OuterIngressTurn::Completion => OuterIngressTurn::Runtime,
            OuterIngressTurn::Runtime => OuterIngressTurn::Ingress,""",
            """OuterIngressTurn::Completion => OuterIngressTurn::Ingress,
            OuterIngressTurn::Runtime => OuterIngressTurn::Runtime,""",
            "cursor advance must preserve Completion/Runtime/Ingress",
        ),
        (
            "drop",
            (("impl", "Drop", "for", "LifecycleCurrentRunnerTurn", "<", "'", "_", ">"),),
            "fn drop",
            "fn removed_drop",
            "borrow-bound outer-ingress current-turn Drop; found 0",
        ),
    ),
)
def test_borrow_bound_outer_ingress_cursor_mutations_fail_closed(
    tmp_path: Path,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Removal or reordering cannot bypass the borrow-bound cursor contract."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    runner_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_runner.rs"
    mutate_rust_item_source_in_context(
        module, runner_path, item_name, context, old, new
    )
    runner_source = runner_path.read_text(encoding="utf-8")
    outer_turns = module._require_rust_item(
        runner_path, runner_source, "outer_ingress_turns", []
    )
    errors = module._outer_ingress_cursor_source_fidelity_errors(
        runner_path, runner_source, outer_turns
    )

    assert any(expected_error in error for error in errors), errors


def test_borrow_bound_outer_ingress_reordering_fails_closed(tmp_path: Path) -> None:
    """Ingress cannot replace the serialized Runtime lifecycle target."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    drain_path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_height_driver.rs"
    )
    source = drain_path.read_text(encoding="utf-8")
    (drain,) = module.rust_items(source, "drain_lifecycle_v2_ingress")
    assert module._borrow_bound_outer_ingress_order_errors(drain_path, drain) == []
    mutated = drain.source.replace(
        "LifecycleRunnerRankTarget::Runtime =>",
        "LifecycleRunnerRankTarget::Ingress =>",
        1,
    )
    drain_path.write_text(
        source.replace(drain.source, mutated, 1), encoding="utf-8"
    )
    (mutated_drain,) = module.rust_items(
        drain_path.read_text(encoding="utf-8"),
        "drain_lifecycle_v2_ingress",
    )

    errors = module._borrow_bound_outer_ingress_order_errors(
        drain_path, mutated_drain
    )
    assert any(
        "serialized advance_executor turn before the single ingress owner" in error
        for error in errors
    ), errors
