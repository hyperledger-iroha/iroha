# Executed lexically in check_sumeragi_v2_proof_ledger.py.

def _effect_capacity_apply_dispatch_barrier_source_fidelity_errors(
    effects_path: Path,
    source: str,
    generic_executor_context: tuple[tuple[str, ...], ...],
    errors: list[str],
) -> None:
    """Bind Apply dispatch to every process-local settlement owner."""

    barrier = _require_rust_item(
        effects_path,
        source,
        "decision_apply_dispatch_barrier_is_occupied",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        barrier,
        generic_executor_context,
        "closed Decision Apply process-local settlement barrier",
        errors,
    )
    _require_exact_rust_tokens(
        effects_path,
        barrier,
        """
fn decision_apply_dispatch_barrier_is_occupied(&self) -> bool {
    self.pending_runner_decision_cleanup.is_some()
        || !self.pending_durable_validate_admissions.is_empty()
        || self.pending_released_lifecycle_validate_apply.is_some()
        || self.pending_live_wal_sign_admission.is_some()
        || !self.pending_lifecycle_output_admissions.is_empty()
}
""",
        "closed Decision Apply barrier must retain runner cleanup, durable and released Validate, live-WAL Sign, and lifecycle-output owners",
        errors,
    )


def _effect_capacity_mutation_runner_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Validate the semantic inventory and outcomes pinned by the TLC runner."""

    path = repo_root / EFFECT_CAPACITY_MUTATION_RUNNER
    if not path.is_file() or path.is_symlink():
        return [
            f"{path}: effect-capacity mutation runner must be a regular file"
        ]

    source = path.read_text(encoding="utf-8")
    normalized_source = re.sub(r"[ \t]*\\\r?\n[ \t]*", " ", source)
    errors: list[str] = []
    for summary in (
        "capacity-blocked Fetch B keeps one exact task/authority/lifecycle FIFO owner without partial P/Q installation",
        "capacity release atomically installs Fetch B P/Q; new B drains T while an authority upgrade retains its exact retry barrier",
    ):
        if source.count(summary) != 1:
            errors.append(
                f"{path}: effect-capacity runner must contain exactly one "
                f"reviewed summary {summary!r}"
            )

    expected_models = {
        name
        for name in EFFECT_CAPACITY_MUTATION_FORMAL_ARTIFACTS
        if name.endswith(".tla")
    }
    expected_configs = {
        name
        for name in EFFECT_CAPACITY_MUTATION_FORMAL_ARTIFACTS
        if name.endswith(".cfg")
    }
    model_names = {
        "OWNERSHIP_MODULE": "SumeragiV2EffectCapacityOwnershipMutation.tla",
        "CERTIFIED_REQUEST_MODULE": (
            "SumeragiV2CertifiedRequestCapacityMutation.tla"
        ),
        "OUTER_TRANSPORT_MODULE": (
            "SumeragiV2EffectCapacityOuterTransportMutation.tla"
        ),
        "RETIREMENT_MODULE": "SumeragiV2EffectCapacityRetirementMutation.tla",
        "PRIORITY_MODULE": "SumeragiV2EffectPreemptionPriorityMutation.tla",
        "BATCH_MODULE": "SumeragiV2RetainedEffectBatchMutation.tla",
    }
    declared_models: dict[str, str] = {}
    for variable, expected_model in model_names.items():
        matches = re.findall(
            rf'(?m)^readonly {re.escape(variable)}="([^"]+)"$', source
        )
        if len(matches) != 1:
            errors.append(
                f"{path}: runner must declare {variable} exactly once"
            )
            continue
        declared_models[variable] = matches[0]
        if matches[0] != expected_model:
            errors.append(
                f"{path}: {variable} must name {expected_model}; "
                f"found {matches[0]}"
            )

    expected_model_by_config: dict[str, str] = {}
    config_groups = (
        (
            "SumeragiV2EffectCapacityOwnershipMutation.tla",
            {
                "effect_capacity_timeout_sign_fixed.cfg",
                "effect_capacity_timeout_sign_lost_bug.cfg",
                "effect_capacity_timeout_sign_refill_bug.cfg",
            },
        ),
        (
            "SumeragiV2CertifiedRequestCapacityMutation.tla",
            {
                "effect_capacity_certified_request_fatal_bug.cfg",
                "effect_capacity_certified_request_fixed.cfg",
                "effect_capacity_certified_request_lost_bug.cfg",
                "effect_capacity_certified_request_duplicate_bug.cfg",
                "effect_capacity_certified_request_overtake_bug.cfg",
                "effect_capacity_certified_request_partial_pq_bug.cfg",
                "effect_capacity_certified_request_substitute_bug.cfg",
                "effect_capacity_certified_request_upgrade_barrier_lost_bug.cfg",
                "effect_capacity_certified_response_blocked_bug.cfg",
                "effect_capacity_certified_response_byte_reserve_bug.cfg",
                "effect_capacity_certified_response_count_reserve_bug.cfg",
            },
        ),
        (
            "SumeragiV2EffectCapacityOuterTransportMutation.tla",
            {
                "effect_capacity_outer_transport_chunk_class_bug.cfg",
                "effect_capacity_outer_transport_class_fixed.cfg",
                "effect_capacity_outer_transport_response_class_bug.cfg",
            },
        ),
        (
            "SumeragiV2EffectCapacityRetirementMutation.tla",
            {
                "effect_capacity_decided_retirement_fixed.cfg",
                "effect_capacity_full_fetch_hol_bug.cfg",
                "effect_capacity_non_fetch_retirement_fixed.cfg",
                "effect_capacity_retirement_disabled_bug.cfg",
            },
        ),
        (
            "SumeragiV2EffectPreemptionPriorityMutation.tla",
            {
                "effect_preemption_decided_victim_bug.cfg",
                "effect_preemption_priority_fixed.cfg",
                "effect_preemption_wrong_class_bug.cfg",
                "effect_preemption_wrong_work_id_bug.cfg",
            },
        ),
        (
            "SumeragiV2RetainedEffectBatchMutation.tla",
            {
                "effect_batch_bound_fixed.cfg",
                "effect_batch_decision_filter_fixed.cfg",
                "effect_batch_decision_no_filter_bug.cfg",
                "effect_batch_oversize_accepted_bug.cfg",
                "effect_batch_partial_fifo_fixed.cfg",
                "effect_batch_partial_fifo_reverse_bug.cfg",
                "effect_batch_second_accepted_bug.cfg",
                "effect_batch_second_rejected_fixed.cfg",
            },
        ),
    )
    for model, configs in config_groups:
        for config in configs:
            expected_model_by_config[config] = model
    if set(expected_model_by_config) != expected_configs:
        errors.append(
            f"{path}: semantic config/model inventory differs from the sealed "
            "thirty-three-config corpus"
        )
    if {model for model, _ in config_groups} != expected_models:
        errors.append(
            f"{path}: semantic model inventory differs from the sealed "
            "six-model corpus"
        )

    loop_contracts = (
        "for blocking_kind in decided non-fetch; do",
        'config_kind="${blocking_kind//-/_}"',
    )
    for contract in loop_contracts:
        if source.count(contract) != 1:
            errors.append(
                f"{path}: retirement expansion must contain exactly one "
                f"{contract!r}"
            )

    cases: list[dict[str, Any]] = []
    for line in normalized_source.splitlines():
        stripped = line.strip()
        if not stripped.startswith("run_case "):
            continue
        try:
            tokens = shlex.split(stripped, comments=False, posix=True)
        except ValueError as error:
            errors.append(f"{path}: cannot parse run_case invocation: {error}")
            continue
        if len(tokens) < 6 or tokens[0] != "run_case":
            errors.append(
                f"{path}: each run_case invocation must name label, model, "
                "config, status, and outcome markers"
            )
            continue
        label, model_token, config, status_token = tokens[1:5]
        markers = tuple(tokens[5:])
        if model_token.startswith("$"):
            variable = (
                model_token.removeprefix("$")
                .removeprefix("{")
                .removesuffix("}")
            )
            model = declared_models.get(variable, model_token)
        else:
            model = model_token
        try:
            status = int(status_token)
        except ValueError:
            errors.append(
                f"{path}: {label} has non-integer TLC status {status_token!r}"
            )
            continue

        expanded = ((label, config),)
        if (
            label == "${blocking_kind}-work-fair-retirement"
            and config
            == "effect_capacity_${config_kind}_retirement_fixed.cfg"
        ):
            expanded = (
                (
                    "decided-work-fair-retirement",
                    "effect_capacity_decided_retirement_fixed.cfg",
                ),
                (
                    "non-fetch-work-fair-retirement",
                    "effect_capacity_non_fetch_retirement_fixed.cfg",
                ),
            )
        elif "${" in label or "${" in config:
            errors.append(
                f"{path}: unreviewed dynamic effect-capacity case "
                f"{label!r}/{config!r}"
            )
            continue
        for expanded_label, expanded_config in expanded:
            cases.append(
                {
                    "label": expanded_label,
                    "model": model,
                    "config": expanded_config,
                    "status": status,
                    "markers": markers,
                }
            )

    observed_configs = [case["config"] for case in cases]
    observed_config_set = set(observed_configs)
    duplicate_configs = sorted(
        config
        for config in observed_config_set
        if observed_configs.count(config) != 1
    )
    if (
        len(cases) != 33
        or observed_config_set != expected_configs
        or duplicate_configs
    ):
        errors.append(
            f"{path}: runner must execute each of the thirty-three sealed "
            "configurations exactly once; "
            f"cases={len(cases)}, missing={sorted(expected_configs - observed_config_set)}, "
            f"extra={sorted(observed_config_set - expected_configs)}, "
            f"duplicates={duplicate_configs}"
        )

    repaired_count = sum(case["status"] == 0 for case in cases)
    mutant_count = sum(case["status"] != 0 for case in cases)
    if repaired_count != 10 or mutant_count != 23:
        errors.append(
            f"{path}: runner must contain exactly 10 repaired cases and 23 "
            f"mutant cases; found repaired={repaired_count}, mutants={mutant_count}"
        )

    generated_total = 0
    distinct_total = 0
    parsed_state_cases = 0
    state_marker = re.compile(
        r"(?P<generated>\d+) states generated, "
        r"(?P<distinct>\d+) distinct states found, "
        r"\d+ states left on queue\."
    )
    for case in cases:
        matches = [
            match
            for marker in case["markers"]
            if (match := state_marker.fullmatch(marker)) is not None
        ]
        if (
            case["model"]
            == "SumeragiV2CertifiedRequestCapacityMutation.tla"
        ):
            if matches:
                errors.append(
                    f"{path}: {case['label']} must not pin unreviewed "
                    "generated/distinct-state totals for the revised "
                    "certified-request model"
                )
            continue
        if len(matches) != 1:
            errors.append(
                f"{path}: {case['label']} must pin exactly one complete "
                "generated/distinct-state marker"
            )
            continue
        parsed_state_cases += 1
        generated_total += int(matches[0].group("generated"))
        distinct_total += int(matches[0].group("distinct"))
    if (
        parsed_state_cases != 22
        or generated_total != 131
        or distinct_total != 130
    ):
        errors.append(
            f"{path}: runner must report exactly 131 generated states and "
            "130 distinct states across the 22 unchanged cases; found "
            f"generated={generated_total}, distinct={distinct_total}, "
            f"parsed_cases={parsed_state_cases}"
        )

    by_config = {
        case["config"]: case
        for case in cases
        if observed_configs.count(case["config"]) == 1
    }
    for config, expected_model in expected_model_by_config.items():
        case = by_config.get(config)
        if case is not None and case["model"] != expected_model:
            errors.append(
                f"{path}: {config} must execute with {expected_model}; "
                f"found {case['model']}"
            )
        if case is None:
            continue
        if config.endswith("_fixed.cfg") and case["status"] != 0:
            errors.append(
                f"{path}: repaired config {config} must expect TLC status 0"
            )
        if config.endswith("_bug.cfg") and case["status"] not in {12, 13}:
            errors.append(
                f"{path}: mutant config {config} must expect TLC status 12 or 13"
            )

    certified_roles = {
        "effect_capacity_certified_request_lost_bug.cfg": (
            "certified-request-retained-owner-drop",
            12,
            (
                "Invariant RetainedFetchBIsNotDropped is violated.",
                "State 2: <RetainCapacityBlockedFetchB",
                "retainedEffects = <<>>",
            ),
        ),
        "effect_capacity_certified_request_substitute_bug.cfg": (
            "certified-request-retained-owner-substitution",
            12,
            (
                "Invariant RetainedFetchBHasExactAuthorityAndTask is violated.",
                "State 2: <RetainCapacityBlockedFetchB",
            ),
        ),
        "effect_capacity_certified_request_duplicate_bug.cfg": (
            "certified-request-retained-owner-duplication",
            12,
            (
                "Invariant RetainedFetchBHasOneOwner is violated.",
                "State 2: <RetainCapacityBlockedFetchB",
            ),
        ),
        "effect_capacity_certified_request_overtake_bug.cfg": (
            "certified-request-retained-owner-overtake",
            12,
            (
                "Invariant RetainedFetchBRemainsFifoHead is violated.",
                "State 2: <RetainCapacityBlockedFetchB",
            ),
        ),
        "effect_capacity_certified_request_fatal_bug.cfg": (
            "certified-request-capacity-fatal",
            12,
            (
                "Invariant CertifiedRequestPressureIsNonfatal is violated.",
                "State 2: <RetainCapacityBlockedFetchB",
                "fatal = TRUE",
            ),
        ),
        "effect_capacity_certified_response_blocked_bug.cfg": (
            "certified-response-blocked-by-unrelated-retained-debt",
            13,
            (
                "Temporal properties were violated.",
                "State 2: <RetainCapacityBlockedFetchB",
                "unrelatedRetainedT = TRUE",
                "fatal = FALSE",
                "State 3: <AdmitOuterTransportResponseA",
                "responseAQueued = TRUE",
                "State 4: Stuttering",
            ),
        ),
        "effect_capacity_certified_response_count_reserve_bug.cfg": (
            "certified-response-count-reserve-missing",
            13,
            (
                "Temporal properties were violated.",
                "State 2: <RetainCapacityBlockedFetchB",
                "outerGenericCountOwned = TRUE",
                "responseAAdmitted = FALSE",
                "State 3: Stuttering",
            ),
        ),
        "effect_capacity_certified_response_byte_reserve_bug.cfg": (
            "certified-response-byte-reserve-missing",
            13,
            (
                "Temporal properties were violated.",
                "State 2: <RetainCapacityBlockedFetchB",
                "outerGenericBytesOwned = TRUE",
                "responseAAdmitted = FALSE",
                "State 3: Stuttering",
            ),
        ),
        "effect_capacity_certified_request_partial_pq_bug.cfg": (
            "certified-request-partial-pq-drain",
            12,
            (
                "Invariant RetainedFetchBInstallsExactPQAtomically is violated.",
                "<AdmitRetainedFetchBAtReleasedCapacity",
            ),
        ),
        "effect_capacity_certified_request_upgrade_barrier_lost_bug.cfg": (
            "certified-request-upgrade-barrier-lost",
            12,
            (
                "Invariant UpgradeFetchBKeepsExactRetryBarrier is violated.",
                "<AdmitRetainedFetchBAtReleasedCapacity",
            ),
        ),
        "effect_capacity_certified_request_fixed.cfg": (
            "certified-request-retained-owner-installs-atomically",
            0,
            (
                "Finished computing initial states: 3 distinct states generated",
                "Model checking completed. No error has been found.",
                "<RetainCapacityBlockedFetchB",
                "<AdmitOuterTransportResponseA",
                "<ConsumeTransportOnlyResponseA",
                "<ReleaseOrdinaryWorkCapacityA",
                "<AdmitRetainedFetchBAtReleasedCapacity",
            ),
        ),
    }
    for config, (expected_label, expected_status, required_markers) in (
        certified_roles.items()
    ):
        case = by_config.get(config)
        if case is None:
            continue
        missing_markers = [
            marker for marker in required_markers if marker not in case["markers"]
        ]
        if (
            case["label"] != expected_label
            or case["status"] != expected_status
            or missing_markers
        ):
            errors.append(
                f"{path}: certified-request role {config} must remain "
                f"{expected_label!r} at status {expected_status} with its "
                f"named witness markers; found label={case['label']!r}, "
                f"status={case['status']}, missing_markers={missing_markers}"
            )

    outer_transport_roles = {
        "effect_capacity_outer_transport_response_class_bug.cfg": (
            "outer-certified-response-classification-missing",
            13,
            (
                "Temporal properties were violated.",
                'completionKind = "CertifiedBodyResponse"',
                "State 2: Stuttering",
            ),
        ),
        "effect_capacity_outer_transport_chunk_class_bug.cfg": (
            "outer-payload-chunk-classification-missing",
            13,
            (
                "Temporal properties were violated.",
                'completionKind = "PayloadChunk"',
                "State 2: Stuttering",
            ),
        ),
        "effect_capacity_outer_transport_class_fixed.cfg": (
            "outer-transport-completion-shared-reserve",
            0,
            (
                "Finished computing initial states: 2 distinct states generated",
                "Model checking completed. No error has been found.",
                "<AdmitTransportCompletion",
                "<ConsumeTransportCompletion",
            ),
        ),
    }
    for config, (expected_label, expected_status, required_markers) in (
        outer_transport_roles.items()
    ):
        case = by_config.get(config)
        if case is None:
            continue
        missing_markers = [
            marker for marker in required_markers if marker not in case["markers"]
        ]
        if (
            case["label"] != expected_label
            or case["status"] != expected_status
            or missing_markers
        ):
            errors.append(
                f"{path}: outer-transport role {config} must remain "
                f"{expected_label!r} at status {expected_status} with its "
                f"named witness markers; found label={case['label']!r}, "
                f"status={case['status']}, missing_markers={missing_markers}"
            )
    return errors


_RETAINED_EFFECT_PACEMAKER_PROGRESS_REGRESSION_SHA256 = (
    "316bd1d4d3d9771f3566abaf6e77f07ad80d1d74fb1f5186875a4a3d30d1b88c"
)


def _effect_capacity_runtime_forwarding_source_fidelity_errors(
    effects_path: Path, source: str, errors: list[str]
) -> None:
    production_effect_runtime_context = (
        ("impl", "EffectRuntime", "for", "SerializedV2Runtime"),
    )
    for item_name, seal_name, delegate, description in (
        (
            "rebind_unpublished_body_available",
            "effect_runtime_rebind_unpublished_body_available",
            """
SerializedV2Runtime::rebind_unpublished_body_available(
    self, previous, rebound, round, subject,
)
.map_err(|error| error.to_string())
""",
            "production EffectRuntime unpublished BodyAvailable rebind forwarding",
        ),
        (
            "retire_unpublished_body_available",
            "effect_runtime_retire_unpublished_body_available",
            """
SerializedV2Runtime::retire_unpublished_body_available(
    self, tag, round, subject
)
.map_err(|error| error.to_string())
""",
            "production EffectRuntime unpublished BodyAvailable retirement forwarding",
        ),
    ):
        matching = tuple(
            item
            for item in rust_items(source, item_name)
            if item.brace_context == production_effect_runtime_context
        )
        if len(matching) != 1:
            errors.append(
                f"{effects_path}: require exactly one production EffectRuntime "
                f"item named {item_name}; found {len(matching)}"
            )
            continue
        production_delegate = matching[0]
        _require_rust_item_context(
            effects_path,
            production_delegate,
            production_effect_runtime_context,
            description,
            errors,
        )
        _require_rust_token_sequence(
            effects_path,
            production_delegate,
            delegate,
            description,
            errors,
        )
        _require_rust_item_token_sha256(
            effects_path,
            production_delegate,
            _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[seal_name],
            description,
            errors,
        )

    for item_name, expected_source, description in (
        (
            "plan_body_pipeline_candidate_terminal",
            """
fn plan_body_pipeline_candidate_terminal(
    &mut self,
    effect: &AdapterEffect,
    ownership: &RuntimeEffectOwnership,
) -> Result<Option<RuntimeEffectOwnership>, String> {
    SerializedV2Runtime::plan_body_pipeline_candidate_terminal(self, effect, ownership)
}
""",
            "production EffectRuntime body-terminal incumbent-owner plan forwarding",
        ),
        (
            "commit_body_pipeline_candidate_terminals",
            """
fn commit_body_pipeline_candidate_terminals(
    &mut self,
    terminals: &[(&AdapterEffect, &RuntimeEffectOwnership)],
) -> Result<(), String> {
    SerializedV2Runtime::commit_body_pipeline_candidate_terminals(self, terminals)
}
""",
            "production EffectRuntime atomic body-terminal authority commit forwarding",
        ),
    ):
        matching = tuple(
            item
            for item in rust_items(source, item_name)
            if item.brace_context == production_effect_runtime_context
        )
        if len(matching) != 1:
            errors.append(
                f"{effects_path}: require exactly one production EffectRuntime "
                f"item named {item_name}; found {len(matching)}"
            )
            continue
        production_delegate = matching[0]
        _require_rust_item_context(
            effects_path,
            production_delegate,
            production_effect_runtime_context,
            description,
            errors,
        )
        _require_exact_rust_tokens(
            effects_path,
            production_delegate,
            expected_source,
            description,
            errors,
        )


def _effect_capacity_terminal_retirement_source_fidelity_errors(
    effects_path: Path,
    source: str,
    errors: list[str],
    generic_executor_context: tuple,
    production_executor_context: tuple,
) -> None:
    commit_fetch_retirement = _require_rust_item(
        effects_path,
        source,
        "commit_pending_fetch_retirement",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        commit_fetch_retirement,
        generic_executor_context,
        "exact pending-Fetch terminal retirement",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        commit_fetch_retirement,
        """
let retired_completion = self
    .runtime
    .retire_unpublished_body_available(
        plan.pending.task.tag,
        plan.pending.task.round,
        plan.pending.task.subject,
    )
    .map_err(EffectExecutorError::Runtime)?;
if !retired_completion {
    let effect = plan.pending.task.adapter_effect();
    self.runtime
        .retire_restored_body_fetch_parent(&effect, plan.pending.task.ownership())
        .map_err(EffectExecutorError::Runtime)?;
}
let work_id = plan.pending.task.id();
let removed = self.pending_fetches.remove(&work_id);
""",
        "pending Fetch retirement must release its token or restored stage-7 parent before local ownership",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        commit_fetch_retirement,
        """
if let Some(certified) = plan.certified {
    self.commit_certified_fetch_retirement(certified);
}
if retires_proposal_replay {
    self.remote_proposal_replay.remove(&key);
}
Ok(())
""",
        "pending Fetch retirement must release its certified request and exact Proposal replay only after runtime and local ownership",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        commit_fetch_retirement,
        _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
            "commit_pending_fetch_retirement"
        ],
        "exact pending-Fetch terminal retirement",
        errors,
    )

    reject_noncanonical = _require_rust_item(
        effects_path,
        source,
        "reject_noncanonical_reconstruction",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        reject_noncanonical,
        generic_executor_context,
        "noncanonical Fetch exact retry",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        reject_noncanonical,
        """
let _retirement = self
    .plan_pending_fetch_retirement(&pending)
    .map_err(|error| self.fail_closed_transport(error, services))?;
""",
        "noncanonical reconstruction must preflight every retained exact Fetch index",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        reject_noncanonical,
        """
if let Err(error) = services.complete_body_reconstruction_fetch(&pending.task) {
    return Err(self.fail_closed_transport(error, services));
}
if let Err(error) = services.enqueue_body_fetch(pending.task) {
    return Err(self.fail_closed_transport(error, services));
}
""",
        "noncanonical reconstruction must reset service work while retaining exact executor ownership",
        errors,
    )

    install_view = _require_rust_item(
        effects_path,
        source,
        "install_view",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        install_view,
        generic_executor_context,
        "certified-view protected Fetch transfer",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        install_view,
        """
self.authenticated_genesis_replay.retain(|key, stage| {
    (Some(*key) == protected_body || Some(*key) == highest_prepare_body)
        && matches!(
            stage,
            AuthenticatedGenesisReplayStageV1::Store { .. }
                | AuthenticatedGenesisReplayStageV1::Stored { .. }
        )
});
""",
        "certified-view installation must retain only Store-or-Stored authenticated-genesis owners selected by the protected or cleanup-only highest-Prepare body",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        install_view,
        _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256["install_view"],
        "certified-view protected Fetch transfer",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        install_view,
        """
protected.validate(&self.context).map_err(|error| {
    EffectExecutorError::Contract(format!(
        "EnterView protected lock is invalid: {error}"
    ))
})?;
if protected.phase != wire::GlobalPhase::Prepare
    || protected.proposal_round.context_id != self.context.id()
    || protected.proposal_round.height != self.context.height
    || protected.proposal_round.view >= tag.view()
{
""",
        "protected Fetch transfer must validate the full PrepareQC lock before use",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        install_view,
        """
if protected.round.view < highest.round.view
    || (protected.round.view == highest.round.view
        && (protected.round != highest.round
            || protected.proposal_round != highest.proposal_round
            || protected.phase != highest.phase
            || protected.subject != highest.subject
            || protected.execution_commitment != highest.execution_commitment))
{
""",
        "protected Fetch transfer must retain the exact highest PrepareQC identity",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        install_view,
        """
self.runtime
    .rebind_unpublished_body_available(
        pending.task.tag,
        tag,
        pending.task.round,
        pending.task.subject,
    )
    .map_err(EffectExecutorError::Runtime)?;
let work_id = pending.task.id();
""",
        "protected Fetch transfer must retag its unpublished completion before local mutation",
        errors,
    )
    if install_view is not None:
        install_tokens = rust_code_tokens(install_view.source)
        ordered_fragments = tuple(
            rust_code_tokens(fragment)
            for fragment in (
                "services.rebind_body_fetch(&pending.task, rebound.clone())",
                "self.runtime.rebind_unpublished_body_available(",
                "current.task = rebound",
            )
        )
        ordered_positions: list[int] = []
        order_is_exact = True
        for fragment in ordered_fragments:
            positions = [
                index
                for index in range(len(install_tokens) - len(fragment) + 1)
                if install_tokens[index : index + len(fragment)] == fragment
            ]
            if len(positions) != 1:
                order_is_exact = False
            else:
                ordered_positions.append(positions[0])
        if not (
            order_is_exact
            and ordered_positions == sorted(ordered_positions)
        ):
            errors.append(
                f"{effects_path}:{install_view.line}: EnterView must transfer "
                "the external Fetch service, then its unpublished runtime "
                "token, then the local pending-Fetch consumer"
            )

    commit_fetch_completion = _require_rust_item(
        effects_path,
        source,
        "commit_fetch_completion",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        commit_fetch_completion,
        generic_executor_context,
        "fallible exact Fetch completion publication",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        commit_fetch_completion,
        """
self.runtime
    .commit_body_available(plan.runtime_reservation)?;
self.commit_body_pipeline_owner(plan.owner);
match plan.ready {
""",
        "runtime BodyAvailable commit must precede every local Fetch-owner mutation",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        commit_fetch_completion,
        """
self.pending_fetches.remove(&plan.work_id);
if let Some(retirement) = plan.certified_retirement {
    self.commit_certified_fetch_retirement(retirement);
}
if advances_proposal_replay {
    let Some(RemoteProposalReplayStageV1::Fetch { replay, .. }) =
        self.remote_proposal_replay.remove(&key)
    else {
        unreachable!("preflighted Proposal Fetch replay remains installed")
    };
    let previous = self
        .remote_proposal_replay
        .insert(key, RemoteProposalReplayStageV1::BodyAvailable(replay));
    debug_assert!(previous.is_none());
}
Ok(())
""",
        "local Fetch and certified-request retirement and Proposal replay advancement must follow runtime publication",
        errors,
    )
    if commit_fetch_completion is not None:
        _require_rust_item_token_sha256(
            effects_path,
            commit_fetch_completion,
            _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
                "commit_fetch_completion"
            ],
            "fallible exact Fetch completion publication",
            errors,
        )
        completion_tokens = rust_code_tokens(commit_fetch_completion.source)
        completion_fragments = tuple(
            rust_code_tokens(fragment)
            for fragment in (
                "self.runtime.commit_body_available(plan.runtime_reservation)?",
                "self.commit_body_pipeline_owner(plan.owner)",
                "match plan.ready",
                "self.pending_fetches.remove(&plan.work_id)",
                "self.commit_certified_fetch_retirement(retirement)",
            )
        )
        completion_positions: list[int] = []
        completion_cardinality_ok = True
        for fragment in completion_fragments:
            positions = [
                index
                for index in range(
                    len(completion_tokens) - len(fragment) + 1
                )
                if completion_tokens[index : index + len(fragment)] == fragment
            ]
            if len(positions) != 1:
                completion_cardinality_ok = False
            else:
                completion_positions.append(positions[0])
        if not (
            completion_cardinality_ok
            and completion_positions == sorted(completion_positions)
        ):
            errors.append(
                f"{effects_path}:{commit_fetch_completion.line}: exact Fetch "
                "completion must publish BodyAvailable before local owner, "
                "ready-body, Fetch, and certified-request retirement"
            )

    bound_leader_wire_ingress = _require_rust_item(
        effects_path,
        source,
        "bound_leader_wire_ingress_ownership",
        errors,
    )
    _require_exact_rust_tokens(
        effects_path,
        bound_leader_wire_ingress,
        """
fn bound_leader_wire_ingress_ownership(
    ingress: &crate::sumeragi::FairV2Ingress,
    message: wire::ConsensusMessageV2,
    sender: PeerId,
) -> FairV2IngressOwnershipEvidence {
    let expected = BlockMessage::V2(message.clone());
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            expected.clone(),
            sender,
        )),
        Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
    ));
    let mut delivered = ingress
        .try_recv()
        .expect("bound leader-wire ingress returns its admitted owner");
    assert_eq!(delivered.message().encode(), expected.encode());
    let ownership = delivered
        .take_ingress_ownership()
        .expect("bound leader-wire ingress attaches exact ownership");
    assert!(
        ownership.leader_wire_token().is_some(),
        "productive wire must carry its full-roster lifecycle token"
    );
    assert!(
        ownership.leader_wire_runtime_receipt().is_some(),
        "checked dequeue must durably transfer the leader-wire token to runtime"
    );
    ownership
}
""",
        "saturation ingress helper must authenticate, dequeue, and retain the exact leader-wire token and runtime receipt",
        errors,
    )

    saturation_regression = _require_rust_item(
        effects_path,
        source,
        "production_capacity_saturation_admits_response_and_reconstructible_fetch",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        saturation_regression,
        _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
            "production_capacity_saturation_admits_response_and_reconstructible_fetch"
        ],
        "production capacity saturation with exact retransmit lifecycle ownership",
        errors,
    )
    for required, description in (
        (
            """
let mut fixture = ProductionTransportFixture::new_with_runtime_queue_config(
    RuntimeQueueConfig::new(12, 4, 4),
);
""",
            "saturation regression must exercise all authenticated normal and progress owners within the explicit runtime queue geometry",
        ),
        (
            """
let sender = fixture.context.roster[ordinal].validator.clone();
let ingress_ownership =
    bound_leader_wire_ingress_ownership(&saturation_ingress, message.clone(), sender);
assert!(
    fixture
        .executor
        .can_admit_network_message_with_ingress_ownership(&message, &ingress_ownership)
);
fixture
    .executor
    .enqueue_network_with_ingress_ownership(message, ingress_ownership)
    .expect("admit production Normal ingress");
""",
            "normal-lane saturation must retain authenticated leader-wire ownership through executor admission",
        ),
        (
            """
let signer = wire::ValidatorIndex::try_from(offset)
    .expect("four-validator Progress saturation signer");
let message = fixture.signed_timeout_vote_from(view, signer);
let ingress_ownership = bound_leader_wire_ingress_ownership(
    &saturation_ingress,
    message.clone(),
    fixture.context.roster[offset].validator.clone(),
);
assert!(
    fixture
        .executor
        .can_admit_network_message_with_ingress_ownership(&message, &ingress_ownership)
);
fixture
    .executor
    .enqueue_network_with_ingress_ownership(message, ingress_ownership)
    .expect("admit production Progress ingress");
""",
            "progress-lane saturation must retain the exact authenticated signer and leader-wire ownership",
        ),
        (
            """
fixture
    .executor
    .runtime
    .retain_retransmit_effect_ownership_for_test(&effects)
    .expect("bind production retransmit lifecycle ownership");
assert_eq!(
    fixture
        .executor
        .consume_effects(effects, &mut services)
        .expect("fill production certified-request ownership"),
    1
);
""",
            "every synthetic saturated Fetch must carry production retransmit ownership",
        ),
        (
            """
fixture
    .executor
    .runtime
    .retain_retransmit_effect_ownership_for_test(&fetch_b_effects)
    .expect("bind deferred production retransmit lifecycle ownership");
assert_eq!(
    fixture
        .executor
        .consume_effects(fetch_b_effects, &mut services)
        .expect("defer Fetch B at production certified-request capacity"),
    0,
);
""",
            "the deferred saturated Fetch must carry the same production ownership",
        ),
    ):
        _require_rust_token_sequence(
            effects_path,
            saturation_regression,
            required,
            description,
            errors,
        )
    _require_rust_token_sequence(
        effects_path,
        saturation_regression,
        """
owner
    .complete_certified_fetch_for_test(
        &mut fixture.executor,
        &mut production_services,
        &ingress,
        completion,
    )
    .unwrap_or_else(|error| match error {
        crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::Retry(
            error,
        ) => panic!(
            "A must publish Ready and retire its physical response: {}: {}",
            error.reason(),
            error.detail(),
        ),
        crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredBeforeLedger(
            error,
        ) => panic!(
            "A lost its productive ingress before persistence: {}: {}",
            error.reason(),
            error.detail(),
        ),
        crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequired(
            error,
        ) => panic!(
            "A reached a restart-only persistence failure: {}: {}",
            error.reason(),
            error.detail(),
        ),
        crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterDequeue(
            error,
        ) => panic!("A lost its exact Runtime handoff after dequeue: {error}"),
        crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(
            error,
        ) => panic!("A failed after the persistence commit: {error}"),
    });
""",
        "the saturated response must complete through the current lifecycle Phase-B persistence adapter",
        errors,
    )

    tc_token_regression = _require_rust_item(
        effects_path,
        source,
        "tc_retires_unprotected_retryable_body_token_before_the_next_fetch",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        tc_token_regression,
        _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
            "tc_retires_unprotected_retryable_body_token_before_the_next_fetch"
        ],
        "unprotected TC unpublished BodyAvailable retirement regression",
        errors,
    )
    for required, description in (
        (
            "assert!(executor.pending_fetches.is_empty());",
            "unprotected TC regression must retire the stale Fetch",
        ),
        (
            """
executor
    .body_ownership_projection()
    .runtime_body_reservation
    .is_none()
""",
            "unprotected TC regression must release the unpublished Completion token",
        ),
        (
            """
[RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
    if *completion_tag == tag(1) && manifest == &manifest_b
""",
            "unprotected TC regression must publish one distinct successor completion",
        ),
        (
            """
executor.probe_certified_response_priority(&response_a, &responder),
Ok(CertifiedResponsePriorityProbe::PreflightRequired(_))
""",
            "unprotected TC regression must first authenticate the exact live response through the read-only priority probe",
        ),
        (
            """
executor.probe_certified_response_priority(&response_a, &responder),
Ok(CertifiedResponsePriorityProbe::DefinitelyNonPriority(
    CertifiedResponsePriorityNonPriority::Unsolicited { request_hash }
)) if request_hash == request_hash_a
""",
            "unprotected TC regression must classify the retired response carrier as unsolicited instead of resurrecting its token",
        ),
        (
            "assert!(!executor.status().fail_closed);",
            "unprotected TC regression must preserve an open runtime after exact retirement and successor publication",
        ),
    ):
        _require_rust_token_sequence(
            effects_path,
            tc_token_regression,
            required,
            description,
            errors,
        )

    sign_token_regression = _require_rust_item(
        effects_path,
        source,
        "durable_sign_preemption_retires_a_retryable_body_token",
        errors,
    )
    _require_rust_item_token_sha256(
        effects_path,
        sign_token_regression,
        _EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
            "durable_sign_preemption_retires_a_retryable_body_token"
        ],
        "durable Sign unpublished BodyAvailable retirement regression",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        sign_token_regression,
        """
executor
    .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
    .expect("durable signing preempts the retryable fetch and its token");
""",
        "durable-Sign regression must exercise pending-Fetch preemption",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        sign_token_regression,
        """
assert!(executor.pending_fetches.is_empty());
assert!(executor.certified_work.is_empty());
assert!(executor.outstanding_requests.is_empty());
assert!(executor.runtime.reserved_body_available.is_none());
assert!(executor.runtime.completions.is_empty());
assert_eq!(executor.pending_signatures.len(), 1);
assert!(!executor.output_guard.restart_required());
assert!(!executor.status().fail_closed);
""",
        "durable-Sign regression must release the complete stale Fetch/token/request owner set and admit one signature without fail-close",
        errors,
    )

    classification = _require_rust_item(
        effects_path,
        source,
        "network_ingress_requires_reducer_order",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        classification,
        generic_executor_context,
        "exhaustive retained-ingress reducer-order classifier",
        errors,
    )
    _require_exact_rust_tokens(
        effects_path,
        classification,
        """
fn network_ingress_requires_reducer_order(
    payload: &wire::ConsensusMessageV2Payload
) -> bool {
    match payload {
        wire::ConsensusMessageV2Payload::Proposal(_)
        | wire::ConsensusMessageV2Payload::Vote(_)
        | wire::ConsensusMessageV2Payload::QuorumCertificate(_)
        | wire::ConsensusMessageV2Payload::TimeoutVote(_)
        | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_) => true,
        wire::ConsensusMessageV2Payload::PayloadManifest(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => false,
    }
}
""",
        "exhaustive transport/reducer ingress classification",
        errors,
    )

    certified_escape = _require_rust_item(
        effects_path,
        source,
        "network_ingress_is_certified_fence_escape",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        certified_escape,
        (),
        "closed authenticated fence-escape classifier",
        errors,
    )
    _require_exact_rust_tokens(
        effects_path,
        certified_escape,
        """
pub(crate) const fn network_ingress_is_certified_fence_escape(
    payload: &wire::ConsensusMessageV2Payload,
) -> bool {
    match payload {
        wire::ConsensusMessageV2Payload::TimeoutCertificate(_) => true,
        wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => {
            matches!(certificate.phase, wire::GlobalPhase::Commit)
        }
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) => {
            matches!(response.certificate.phase, wire::GlobalPhase::Commit)
        }
        wire::ConsensusMessageV2Payload::Proposal(_)
        | wire::ConsensusMessageV2Payload::Vote(_)
        | wire::ConsensusMessageV2Payload::TimeoutVote(_)
        | wire::ConsensusMessageV2Payload::PayloadManifest(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(_) => false,
    }
}
""",
        "only TC, direct CommitQC, and discovery CommitQC may escape a hung signer",
        errors,
    )

    retained_dispatch = _require_rust_item(
        effects_path,
        source,
        "retained_dispatch_allows_network_ingress",
        errors,
    )
    _require_rust_item_context(
        effects_path,
        retained_dispatch,
        generic_executor_context,
        "retained dispatch transport-completion bypass",
        errors,
    )
    _require_exact_rust_tokens(
        effects_path,
        retained_dispatch,
        """
fn retained_dispatch_allows_network_ingress(
    &self,
    payload: &wire::ConsensusMessageV2Payload,
) -> bool {
    network_ingress_is_certified_fence_escape(payload)
        || self
            .runtime
            .wire_ingress_may_use_pacemaker_progress(payload)
        || (self.retained_effect_batch.is_none() && self.parked_effect_batch.is_none()
            || !Self::network_ingress_requires_reducer_order(payload))
}
""",
        "retained dispatch transport completion, certified fence escape, and typed pacemaker-progress policy",
        errors,
    )

    retained_debt_regression_name = (
        "retained_effect_debt_admits_only_pacemaker_progress_leader_wires"
    )
    retained_debt_regression = _require_rust_item(
        effects_path,
        source,
        retained_debt_regression_name,
        errors,
    )
    _require_rust_item_context(
        effects_path,
        retained_debt_regression,
        (
            (
                "#",
                "[",
                "cfg",
                "(",
                "test",
                ")",
                "]",
                "mod",
                "tests",
            ),
        ),
        "retained-effect pacemaker Progress cross-layer regression",
        errors,
        expected_attributes=("#[test]",),
    )
    _require_rust_item_token_sha256(
        effects_path,
        retained_debt_regression,
        _RETAINED_EFFECT_PACEMAKER_PROGRESS_REGRESSION_SHA256,
        "retained-effect pacemaker Progress cross-layer regression",
        errors,
    )
    for required, description in (
        (
            """
assert!(executor.retained_effect_batch.is_some());
executor.runtime.protected_commit = Some((
    fixture.manifest.round,
    fixture.manifest.subject,
    fixture_execution_commitment(),
));
executor.runtime.protected_prepare = Some((
    fixture.manifest.round,
    fixture.manifest.subject,
    fixture_execution_commitment(),
));
""",
            "retained debt must install the exact protected Commit and locked-reproposal Prepare authorities",
        ),
        (
            """
let inbound = crate::sumeragi::fair_v2_ingress_admit_for_test(
    InboundBlockMessage::from_authenticated_peer(
        BlockMessage::V2(message.clone()),
        sender.clone(),
    ),
);
v2_ingress_head_can_drain(&inbound, executor, None)
""",
            "the effects regression must cross fair ingress and the executor-aware drain gate",
        ),
        (
            """
let mut prepare_vote = vote(&fixture);
prepare_vote.signature = vec![0x73];
let prepare_vote =
    wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(prepare_vote));
let mut mismatched_prepare_vote = vote(&fixture);
mismatched_prepare_vote.subject.block_hash =
    HashOf::from_untyped_unchecked(Hash::new(b"effects retained-debt mismatched Prepare"));
mismatched_prepare_vote.signature = vec![0x76];
let mismatched_prepare_vote = wire::ConsensusMessageV2::new(
    wire::ConsensusMessageV2Payload::Vote(mismatched_prepare_vote),
);
""",
            "the negative Prepare witness must differ from the protected locked-reproposal subject",
        ),
        (
            """
assert!(
    can_drain(executor, &prepare_vote),
    "the exact current locked-reproposal PrepareVote reaches Progress across {debt} debt"
);
""",
            "the protected locked-reproposal Prepare must be admitted under effect debt",
        ),
        (
            """
assert!(
    !can_drain(executor, &mismatched_prepare_vote),
    "an unrelated PrepareVote remains ordinary ingress across {debt} debt"
);
""",
            "a mismatched Prepare must remain behind effect debt",
        ),
        (
            """
assert_matrix(&executor, "retained");
assert!(executor.retained_effect_batch.is_some());
executor
    .park_retained_effect_batch()
    .expect("park the exact ordinary suffix behind protected Progress");
assert!(executor.retained_effect_batch.is_none());
assert!(executor.parked_effect_batch.is_some());
assert_matrix(&executor, "parked");
""",
            "the exact admission matrix must hold for retained and parked effect debt",
        ),
    ):
        _require_rust_token_sequence(
            effects_path,
            retained_debt_regression,
            required,
            description,
            errors,
        )

    for item_name, required, description in (
        (
            "enqueue_network_with_ingress_ownership",
            """
if self.fatal_reason.is_some() || self.output_guard.restart_required() {
    return Err(NetworkIngressError::FailClosed);
}
let result = self
    .runtime
    .enqueue_network_with_ingress_ownership(message, ingress_ownership);
if matches!(&result, Err(NetworkIngressError::FailClosed)) {
    self.output_guard.activate_restart_required();
    self.fatal_reason.get_or_insert_with(|| {
        "Sumeragi v2 runtime rejected authenticated ingress ownership".to_owned()
    });
}
result
""",
            "public ownership-aware network admission and fail-stop latch",
        ),
        (
            "can_admit_network_message_with_ingress_ownership",
            """
if self.fatal_reason.is_some() || self.output_guard.restart_required() {
    return false;
}
let retained_dispatch_allows =
    self.retained_dispatch_allows_network_ingress(&message.payload);
let timeout_vote_recovery_episode = !retained_dispatch_allows
    && self
        .runtime
        .can_admit_timeout_vote_recovery_episode(message, ingress_ownership);
(retained_dispatch_allows || timeout_vote_recovery_episode)
    && self
        .runtime
        .can_admit_network_message_with_ingress_ownership(message, ingress_ownership)
""",
            "public ownership-aware retained-debt capacity preflight",
        ),
    ):
        executor_context = (
            production_executor_context
            if item_name == "enqueue_network_with_ingress_ownership"
            else generic_executor_context
        )
        matches = tuple(
            candidate
            for candidate in rust_items(source, item_name)
            if candidate.brace_context == executor_context
        )
        item = matches[0] if len(matches) == 1 else None
        if len(matches) != 1:
            errors.append(
                f"{effects_path}: require exactly one production executor item "
                f"{item_name}; found {len(matches)}"
            )
        _require_rust_item_context(
            effects_path,
            item,
            executor_context,
            description,
            errors,
        )
        _require_rust_item_token_sha256(
            effects_path,
            item,
            _EFFECT_CAPACITY_PRODUCTION_RUST_ITEM_SHA256[item_name],
            description,
            errors,
        )
        _require_rust_token_sequence(
            effects_path,
            item,
            required,
            description,
            errors,
        )
