# Lexically loaded by sumeragi_v2_multilane_models_test.py.

def copy_queue_plan_autonomous_only_fixture(tmp_path: Path, module) -> list[dict]:
    """Copy the QueuePlan role-separation sources and positive TLA kernel."""

    models = canonical_models()
    relatives = {
        Path(relative)
        for relative, _, _, _ in module.QUEUE_PLAN_AUTONOMOUS_ONLY_BINDINGS
    }
    relatives.update(
        Path(relative)
        for relative, _, _ in module.QUEUE_PLAN_AUTONOMOUS_ONLY_TEST_BINDINGS
    )
    relatives.update(
        {
            module.FORMAL_RELATIVE / f"{module.QUEUE_PLAN_STARTUP_REPLAY_MODULE}.tla",
            module.FORMAL_RELATIVE
            / "multilane_queue_plan_admission_registry_fixed.cfg",
        }
    )
    copy_reviewed_source_fixture_with_includes(tmp_path, module, relatives)
    return models


def validate_queue_plan_autonomous_only_fixture(
    tmp_path: Path, module, models: list[dict]
) -> tuple[str, ...]:
    errors: list[str] = []
    with module._reviewed_rust_source_cache():
        module._validate_queue_plan_autonomous_only_contract(
            tmp_path,
            tmp_path / module.FORMAL_RELATIVE,
            models,
            errors,
        )
    return tuple(errors)


def test_queue_plan_autonomous_only_contract_accepts_current_production(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_autonomous_only_fixture(tmp_path, module)
    assert validate_queue_plan_autonomous_only_fixture(tmp_path, module, models) == ()


def test_queue_plan_autonomous_only_contract_rejects_candidate_fifo_bypass(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_autonomous_only_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_candidate.rs"
    replace_once_after(
        path,
        "fn snapshot_routable_candidates(",
        "            if queue_plan_synced {\n",
        "            if false {\n",
    )
    errors = validate_queue_plan_autonomous_only_fixture(tmp_path, module, models)
    assert any(
        "V2CandidateAssembler::snapshot_routable_candidates" in error
        and "if queue_plan_synced" in error
        for error in errors
    ), errors


def test_queue_plan_autonomous_only_contract_rejects_virtual_fifo_cut_bypass(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_autonomous_only_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/queue.rs"
    replace_once_after(
        path,
        "pub(crate) fn bounded_pending_snapshot(",
        "if live_reservation_fifo_cut.is_some_and(|cut| fifo_order.value().ordinal >= cut) {",
        "if live_reservation_fifo_cut.is_none() {",
    )
    errors = validate_queue_plan_autonomous_only_fixture(tmp_path, module, models)
    assert any(
        "Queue::bounded_pending_snapshot" in error
        and "live_reservation_fifo_cut.is_some_and" in error
        for error in errors
    ), errors


def test_queue_plan_autonomous_only_contract_rejects_ordinary_lane_payload_intent(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_autonomous_only_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/lane_consensus.rs"
    replace_once_after(
        path,
        "fn validate_lane_executable_payload_body(",
        "entrypoint.admission_intent() != TransactionAdmissionIntent::QueuePlanSynced",
        "entrypoint.admission_intent() == TransactionAdmissionIntent::QueuePlanSynced",
    )
    errors = validate_queue_plan_autonomous_only_fixture(tmp_path, module, models)
    assert any(
        "validate_lane_executable_payload_body" in error
        and "TransactionAdmissionIntent::QueuePlanSynced" in error
        for error in errors
    ), errors


def test_queue_plan_autonomous_only_contract_rejects_route_gated_provider(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_autonomous_only_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_lane_work.rs"
    replace_once_after(
        path,
        "impl CandidateWorkProvider for &mut V2LaneWorkAdapter {",
        "(is_queue_plan_synced\n",
        "(false\n",
    )
    errors = validate_queue_plan_autonomous_only_fixture(tmp_path, module, models)
    assert any(
        "&mut V2LaneWorkAdapter::prepare" in error and "(is_queue_plan_synced" in error
        for error in errors
    ), errors


def test_queue_plan_autonomous_only_contract_rejects_late_locked_body_guard(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_autonomous_only_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_lane_work.rs"
    swap_ordered_once_after(
        path,
        "fn bind_locked_global_body_from_origin(",
        "if crate::block::external_queue_plan_synced_entrypoint_index(block).is_some()",
        "let canonical_recovery = canonical_v2_lane_payload_matches_kura(",
    )
    errors = validate_queue_plan_autonomous_only_fixture(tmp_path, module, models)
    assert any(
        "V2LaneWorkAdapter::bind_locked_global_body_from_origin" in error
        and "missing or reorders token" in error
        for error in errors
    ), errors


def test_queue_plan_autonomous_only_contract_rejects_disabled_common_guard(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_autonomous_only_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/block.rs"
    replace_once_after(
        path,
        "fn validate_staged_execution_controls(",
        "if let Some(index) = external_queue_plan_synced_entrypoint_index(block)",
        "if false && let Some(index) = external_queue_plan_synced_entrypoint_index(block)",
    )
    errors = validate_queue_plan_autonomous_only_fixture(tmp_path, module, models)
    assert any(
        "validate_staged_execution_controls" in error and "if let Some(index)" in error
        for error in errors
    ), errors


def test_queue_plan_autonomous_only_contract_rejects_tla_ordinary_execution(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_autonomous_only_fixture(tmp_path, module)
    path = (
        tmp_path
        / module.FORMAL_RELATIVE
        / f"{module.QUEUE_PLAN_STARTUP_REPLAY_MODULE}.tla"
    )
    replace_once(path, '       ELSE "Autonomous"\n', '       ELSE "Ordinary"\n')
    errors = validate_queue_plan_autonomous_only_fixture(tmp_path, module, models)
    assert any(
        "QueuePlan autonomous-only TLA token" in error and 'ELSE "Autonomous"' in error
        for error in errors
    ), errors


def copy_queue_plan_pending_membership_fixture(
    tmp_path: Path, module
) -> list[dict]:
    """Copy sources consumed by the exact QueuePlan route-member contract."""

    models = canonical_models()
    relatives = {
        Path(relative)
        for relative, _, _, _ in module.QUEUE_PLAN_PENDING_MEMBERSHIP_BINDINGS
    }
    relatives.update(
        Path(relative)
        for relative, _, _ in module.QUEUE_PLAN_PENDING_MEMBERSHIP_TEST_BINDINGS
    )
    relatives.update(
        Path(row[0])
        for row in module.QUEUE_PLAN_PENDING_MEMBERSHIP_ORDERED_SOURCE_CHECKS
    )
    copy_reviewed_source_fixture_with_includes(tmp_path, module, relatives)
    return models


def validate_queue_plan_pending_membership_fixture(
    tmp_path: Path, module, models: list[dict]
) -> tuple[str, ...]:
    errors: list[str] = []
    with module._reviewed_rust_source_cache():
        module._validate_queue_plan_pending_membership_contract(
            tmp_path, models, errors
        )
    return tuple(errors)


def test_queue_plan_pending_membership_ledger_accepts_all_current_owners() -> None:
    module = load_checker()
    errors: list[str] = []
    assert module._validate_queue_plan_pending_membership_model_bindings(
        canonical_models(), errors
    )
    assert errors == [], errors


@pytest.mark.parametrize("mutation", (None, "network-owner", "removed-finalizer"))
def test_queue_plan_startup_model_requires_current_source_declarations(
    tmp_path: Path, mutation: str | None
) -> None:
    """Startup ledger rows resolve directly without token translation or deletion."""
    module = load_checker()
    model = next(
        model for model in canonical_models()
        if model["module"] == module.QUEUE_PLAN_STARTUP_REPLAY_MODULE
    )
    model["production_symbols"] = [
        binding for binding in model["production_symbols"]
        if binding["symbol"] == "Iroha::start_with_runtime_deps"
    ]
    assert len(model["production_symbols"]) == 1
    binding = model["production_symbols"][0]
    copy_reviewed_rust_source_fixture(tmp_path, module, binding["path"])
    if mutation == "network-owner":
        current = "IrohaNetwork::start_with_crypto_and_initial_authorities("
        obsolete = "IrohaNetwork::start_with_crypto_and_initial_trusted_sources("
        binding["required_tokens"][binding["required_tokens"].index(current)] = obsolete
    elif mutation == "removed-finalizer":
        obsolete = "finalize_plan_journal_startup_recovery()"
        binding["required_tokens"].append(obsolete)
    errors: list[str] = []
    module._validate_model(tmp_path, ROOT_DIR / "formal/sumeragi_v2", model, errors)
    if mutation is None:
        assert errors == [], errors
    else:
        assert len(errors) == 1, errors
        assert obsolete in errors[0] and "missing source-binding token" in errors[0]


@pytest.mark.parametrize("mutation", ("missing", "duplicate", "weakened"))
@pytest.mark.parametrize(
    "symbol",
    (
        'QueuePlanPendingSignedAliasMemberV1',
        'QueuePlanSignedAliasTerminalV1',
        'decode_exact_queue_plan_pending_signed_alias_member_marker',
        'decode_exact_queue_plan_signed_alias_terminal_marker',
        'prevalidate_queue_plan_pending_route_rosters',
        'queue_plan_admission_registry_value_in_view',
        'queue_plan_binding_application_evidence_in_view',
        'queue_plan_binding_application_state',
        'queue_plan_binding_application_state_in_storage',
        'queue_plan_pending_exact_route_member_state_after_roster_prevalidation',
        'queue_plan_pending_exact_route_member_state_in_storage',
        'queue_plan_pending_signed_alias_member_from_obligation',
        'queue_plan_pending_signed_alias_member_marker_key',
        'queue_plan_pending_signed_alias_member_marker_payload',
        'queue_plan_pending_signed_alias_member_marker_prefix',
        'queue_plan_pending_signed_alias_members_from_storage',
        'queue_plan_registry_owner_application_state_in_view',
        'queue_plan_signed_alias_terminal_marker_key',
        'queue_plan_signed_alias_terminal_marker_key_from_claim',
        'queue_plan_signed_alias_terminal_marker_payload',
        'queue_plan_terminal_signed_alias_member_from_obligation',
        'require_queue_plan_pending_signed_alias_member_marker',
        'resolve_queue_plan_pending_obligation_after_roster_prevalidation',
        'resolve_queue_plan_pending_obligation_by_signed_alias_in_storage',
        'resolve_queue_plan_pending_obligation_in_storage',
        'resolve_queue_plan_pending_obligations_from_block',
        'resolve_required_queue_plan_pending_obligations',
        'stage_queue_plan_pending_obligation_marker_in_storage',
    ),
)
def test_queue_plan_pending_membership_ledger_rejects_replay_owner_drift(
    symbol: str, mutation: str
) -> None:
    """Every replay-terminal owner needs one exact, non-weakened ledger row."""
    module = load_checker()
    models = canonical_models()
    model = next(
        model for model in models
        if model["module"] == module.QUEUE_PLAN_PENDING_MEMBERSHIP_MODULE
    )
    rows = model["production_symbols"]
    matches = [row for row in rows if row["symbol"] == symbol]
    assert len(matches) == 1
    row = matches[0]
    if mutation == "missing":
        rows.remove(row)
        expected_error = "exactly once, found 0"
    elif mutation == "duplicate":
        rows.append(copy.deepcopy(row))
        expected_error = "exactly once, found 2"
    else:
        row["required_tokens"].pop()
        expected_error = "route-membership tokens changed"
    errors: list[str] = []
    assert module._validate_queue_plan_pending_membership_model_bindings(models, errors)
    assert len(errors) == 1, errors
    assert symbol in errors[0] and expected_error in errors[0], errors


def test_queue_plan_pending_membership_contract_accepts_current_production(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert errors == (), errors


def test_queue_plan_exact_membership_contract_rejects_whole_roster_scan(
    tmp_path: Path,
) -> None:
    """A valid exact lookup must stay independent of unrelated route siblings."""
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once_after(
        path,
        "fn queue_plan_pending_exact_route_member_state_in_storage(",
        "        let mut present = 0usize;",
        "        Self::prevalidate_queue_plan_pending_route_rosters(\n"
        "            storage, obligation.routes.iter().copied(),\n"
        "        )?;\n"
        "        let mut present = 0usize;",
    )
    errors = validate_queue_plan_pending_membership_fixture(tmp_path, module, models)
    assert any("without scanning route rosters" in error for error in errors), errors


@pytest.mark.parametrize(
    "condition",
    (
        "if storage.get(&terminal_key).is_some()",
        "if outer_committed && storage.get(&terminal_key).is_some()",
    ),
)
def test_queue_plan_exact_binding_contract_rejects_terminal_marker_bypass(
    tmp_path: Path, condition: str,
) -> None:
    """Pending and directly applied exact owners reject stray terminal aliases."""
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once_after(
        path, "fn queue_plan_binding_application_state_in_storage(",
        condition, "if false",
    )
    errors = validate_queue_plan_pending_membership_fixture(tmp_path, module, models)
    assert any(
        "queue_plan_binding_application_state_in_storage" in error
        and condition in error for error in errors
    ), errors


def test_queue_plan_idempotent_staging_contract_requires_roster_prevalidation(
    tmp_path: Path,
) -> None:
    """Read-side bounded lookup never substitutes for a mutation preflight."""
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once_after(
        path, "fn stage_queue_plan_pending_obligation_marker_in_storage(",
        "Self::prevalidate_queue_plan_pending_route_rosters(",
        "Self::unchecked_route_rosters(",
    )
    errors = validate_queue_plan_pending_membership_fixture(tmp_path, module, models)
    assert any(
        "stage_queue_plan_pending_obligation_marker_in_storage" in error
        and "prevalidate_queue_plan_pending_route_rosters" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_bound_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "const MAX_QUEUE_PLAN_COMPACT_MARKER_BYTES: usize = 1024;",
        "const MAX_QUEUE_PLAN_COMPACT_MARKER_BYTES: usize = 2048;",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any("one exact reviewed 1024-byte declaration" in error for error in errors), errors


def test_queue_plan_pending_membership_contract_rejects_roster_bound_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "const MAX_QUEUE_PLAN_PENDING_ROUTE_MEMBERS: usize = "
        "MAX_QUEUE_PLAN_ADMISSIONS_PER_BLOCK;",
        "const MAX_QUEUE_PLAN_PENDING_ROUTE_MEMBERS: usize = usize::MAX;",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "exact block/proposal admission consensus bound" in error for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_signed_alias_roster_bound_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "const MAX_QUEUE_PLAN_PENDING_SIGNED_ALIAS_MEMBERS: usize = "
        "MAX_QUEUE_PLAN_ADMISSIONS_PER_BLOCK;",
        "const MAX_QUEUE_PLAN_PENDING_SIGNED_ALIAS_MEMBERS: usize = usize::MAX;",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "signed-alias reverse roster" in error
        and "exact block/proposal admission consensus bound" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_unbounded_roster_scan(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "            route,\n"
        "            MAX_QUEUE_PLAN_PENDING_ROUTE_MEMBERS,\n",
        "            route,\n"
        "            usize::MAX,\n",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_pending_route_members_from_storage" in error
        and "MAX_QUEUE_PLAN_PENDING_ROUTE_MEMBERS" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_phantom_member(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once_after(
        path,
        "fn queue_plan_pending_route_members_from_storage_with_limit(",
        "            let obligation_payload = storage.get(&obligation_key).ok_or_else(|| {\n"
        "                MergeLedgerCommitError::ExecutionMarkerConflict(format!(\n"
        "                    \"QueuePlan pending-route member marker `{key}` has no exact obligation `{obligation_key}`\"\n"
        "                ))\n"
        "            })?;\n",
        "            let obligation_payload = storage.get(&obligation_key).unwrap_or(payload);\n",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_pending_route_members_from_storage" in error
        and "storage.get(&obligation_key).ok_or_else" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_inexact_roster_obligation_projection(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once_after(
        path,
        "fn queue_plan_pending_route_members_from_storage_with_limit(",
        "if marker != expected {",
        "if false {",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "decode the exact bounded obligation" in error
        and "complete canonical projection" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_untyped_member_claim(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once_after(
        path,
        "fn queue_plan_pending_route_member_identity_from_claim(",
        "        entrypoint_hash: HashOf<TransactionEntrypoint>,\n",
        "        entrypoint_hash: Hash,\n",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_pending_route_member_identity_from_claim" in error
        and "HashOf<TransactionEntrypoint>" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_visible_native_prefix(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_HOST_RELATIVE
    replace_once(
        path,
        '    "queue_plan_pending_obligation_v1_",\n',
        "",
    )
    replace_once(
        path,
        '    "queue_plan_pending_route_member_v1_",\n',
        "",
    )
    replace_once(
        path,
        '    "queue_plan_pending_signed_alias_member_v1_",\n',
        "",
    )
    replace_once(
        path,
        '    "queue_plan_signed_alias_terminal_v1_",\n',
        "",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_pending_obligation_v1_" in error
        and "opaque system contract-state namespace" in error
        for error in errors
    ), errors
    assert any(
        "queue_plan_pending_route_member_v1_" in error
        and "opaque system contract-state namespace" in error
        for error in errors
    ), errors
    assert any(
        "queue_plan_pending_signed_alias_member_v1_" in error
        and "opaque system contract-state namespace" in error
        for error in errors
    ), errors
    assert any(
        "queue_plan_signed_alias_terminal_v1_" in error
        and "opaque system contract-state namespace" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_state_prefix_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    for prefix in module.QUEUE_PLAN_PENDING_OPAQUE_PREFIXES:
        replace_once(path, f'"{prefix}"', f'"drifted_{prefix}"')
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    for prefix in module.QUEUE_PLAN_PENDING_OPAQUE_PREFIXES:
        assert any(
            prefix in error and "one exact canonical declaration" in error
            for error in errors
        ), errors


def test_queue_plan_pending_membership_contract_rejects_inexact_signed_alias_reverse_owner(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once_after(
        path,
        "fn queue_plan_pending_signed_alias_members_from_storage(",
        "if registry.binding_hash != marker.binding_hash {",
        "if false {",
    )
    replace_once_after(
        path,
        "fn queue_plan_pending_signed_alias_members_from_storage(",
        "if Self::queue_plan_pending_signed_alias_member_from_obligation(&obligation).as_ref()\n"
        "                != Some(&marker)",
        "if false",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_pending_signed_alias_members_from_storage" in error
        and "registry.binding_hash != marker.binding_hash" in error
        for error in errors
    ), errors
    assert any(
        "queue_plan_pending_signed_alias_members_from_storage" in error
        and "queue_plan_pending_signed_alias_member_from_obligation" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_direct_alias_evidence_conflation(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once_after(
        path,
        "fn queue_plan_binding_application_evidence_in_view(",
        "QueuePlanBindingApplicationEvidence::AppliedDirect",
        "QueuePlanBindingApplicationEvidence::Pending",
    )
    replace_once_after(
        path,
        "fn queue_plan_binding_application_evidence_in_view(",
        "QueuePlanBindingApplicationEvidence::AppliedViaSignedAlias",
        "QueuePlanBindingApplicationEvidence::PendingStale",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    for evidence in (
        "QueuePlanBindingApplicationEvidence::AppliedDirect",
        "QueuePlanBindingApplicationEvidence::AppliedViaSignedAlias",
    ):
        assert any(
            "queue_plan_binding_application_evidence_in_view" in error
            and evidence in error
            for error in errors
        ), errors


def test_queue_plan_pending_membership_contract_rejects_alias_terminal_prefix_write(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once_after(
        path,
        "fn resolve_queue_plan_pending_obligation_by_signed_alias_in_storage(",
        "        let alias_key = Self::queue_plan_pending_signed_alias_member_marker_key(member)?;\n",
        "        let alias_key = Self::queue_plan_pending_signed_alias_member_marker_key(member)?;\n"
        "        storage.insert_queue_plan_marker(alias_key.clone(), Vec::new());\n",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "resolve_queue_plan_pending_obligation_by_signed_alias_in_storage mutates WSV "
        "before completing all-route preflight" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_alias_decode_before_bound(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    for symbol, decode in (
        (
            "decode_exact_queue_plan_pending_signed_alias_member_marker",
            "norito::decode_from_bytes::<QueuePlanPendingSignedAliasMemberV1>(payload)",
        ),
        (
            "decode_exact_queue_plan_signed_alias_terminal_marker",
            "norito::decode_from_bytes::<QueuePlanSignedAliasTerminalV1>(payload)",
        ),
    ):
        swap_ordered_once_after(
            path,
            f"fn {symbol}(",
            "payload.is_empty() || payload.len() > MAX_QUEUE_PLAN_COMPACT_MARKER_BYTES",
            decode,
        )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    for symbol in (
        "decode_exact_queue_plan_pending_signed_alias_member_marker",
        "decode_exact_queue_plan_signed_alias_terminal_marker",
    ):
        assert any(
            f"ordered QueuePlan pending route-membership item {symbol}" in error
            for error in errors
        ), errors


def assert_inflight_order_drift_rejected(
    tmp_path: Path, earlier: str, later: str,
    rejected_token: str, required_scope: str,
) -> None:
    module = load_checker()
    contract = canonical_contract()
    copy_layout_fixture(tmp_path, module, contract)
    path = tmp_path / "crates/iroha_core/src/queue/reservation_journal.rs"
    swap_ordered_once(path, earlier, later)
    errors = validate_fixture(tmp_path, module, contract)
    assert any(
        required_scope in error
        and f"missing or reorders token {rejected_token!r}" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "token"),
    (
        (
            "assert_queue_plan_native_batch_rollback_is_atomic",
            "failed whole-list staging must restore the exact prior overlay",
        ),
        (
            "queue_plan_pending_resolution_corrupt_route_counts_fail_without_partial_mutation",
            "failed whole-list resolution must restore the exact prior overlay",
        ),
        (
            "queue_plan_signed_alias_terminal_evidence_is_exact_and_fail_closed",
            "terminalization must remove the signed-first pending reverse index",
        ),
    ),
)
def test_queue_plan_pending_membership_contract_rejects_atomic_test_weakening(
    tmp_path: Path, symbol: str, token: str
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/state/autonomous_merge_and_queue_plan_tests.rs"
    )
    replace_once(path, token, "weakened atomic rollback assertion")
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(symbol in error and token in error for error in errors), errors


def test_queue_plan_pending_membership_contract_rejects_inner_stage_prefix_write(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "        let obligation_payload = Self::queue_plan_pending_obligation_marker_payload(&obligation)?;\n",
        "        storage.insert_queue_plan_marker(obligation_key.clone(), Vec::new());\n"
        "        let obligation_payload = Self::queue_plan_pending_obligation_marker_payload(&obligation)?;\n",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "stage_queue_plan_pending_obligation_marker_in_storage mutates WSV "
        "before completing all-route preflight" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_stage_apply_before_list(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    swap_ordered_once_after(
        path,
        "fn stage_queue_plan_admissions(",
        "State::stage_queue_plan_pending_obligation_in_storage(&mut markers, &admission)?;",
        "markers.apply();",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "ordered QueuePlan pending route-membership item "
        "stage_queue_plan_admissions" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "resolution_token"),
    (
        (
            "resolve_queue_plan_pending_obligations_for_entrypoints",
            "State::resolve_queue_plan_pending_obligation_in_storage(",
        ),
        (
            "resolve_required_queue_plan_pending_obligations",
            "State::resolve_queue_plan_pending_obligation_by_signed_alias_in_storage(",
        ),
    ),
)
def test_queue_plan_pending_membership_contract_rejects_bulk_apply_before_list(
    tmp_path: Path, symbol: str, resolution_token: str
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    swap_ordered_once_after(
        path,
        f"fn {symbol}(",
        resolution_token,
        "markers.apply();",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        f"ordered QueuePlan pending route-membership item {symbol}" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_decode_before_bound(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    swap_ordered_once_after(
        path,
        "fn decode_exact_queue_plan_pending_route_member_marker(",
        "payload.is_empty() || payload.len() > MAX_QUEUE_PLAN_COMPACT_MARKER_BYTES",
        "norito::decode_from_bytes::<QueuePlanPendingRouteMemberV1>(payload)",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "ordered QueuePlan pending route-membership item "
        "decode_exact_queue_plan_pending_route_member_marker" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_lifecycle_height_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "state.lane_incarnation_at_height(route.lane_id, proposal_height)",
        "state.lane_incarnation_at_height("
        "route.lane_id, proposal_height.saturating_add(1))",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_pending_obligation_matches_active_lifecycle" in error
        and "lane_incarnation_at_height" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_stale_queue_ownership(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once(
        path,
        "application_state != Some(QueuePlanAdmissionApplicationState::Pending)",
        "application_state != Some(QueuePlanAdmissionApplicationState::PendingStale)",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_admission_registry_match" in error
        and "QueuePlanAdmissionApplicationState::Pending" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_stale_cleanup_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once_after(
        path,
        "fn classify_pending_queue_plan_admission_in_view(",
        "PendingQueuePlanAdmissionDisposition::Stale",
        "PendingQueuePlanAdmissionDisposition::ExactPending",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "classify_pending_queue_plan_admission" in error
        and "PendingQueuePlanAdmissionDisposition::Stale" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "symbol", "old", "new"),
    [
        (
            "crates/iroha_core/src/state.rs",
            "persist_classified_queue_plan_admission",
            "self.queue_plan_admission_persistence_lock.lock()",
            "()",
        ),
        (
            "crates/iroha_core/src/state.rs",
            "persist_classified_queue_plan_admission",
            "existing.certificate.binding == incoming.certificate.binding",
            "true",
        ),
        (
            "crates/iroha_core/src/state.rs",
            "persist_classified_queue_plan_admission",
            "one_ahead != Some(actual_durable_height)",
            "false",
        ),
        (
            "crates/iroha_core/src/state.rs",
            "persist_classified_queue_plan_admission",
            "Instant::now() >= *deadline",
            "false",
        ),
        (
            "crates/iroha_core/src/state.rs",
            "classify_pending_queue_plan_admission_in_view",
            "PendingQueuePlanAdmissionDisposition::Applied",
            "PendingQueuePlanAdmissionDisposition::ExactPending",
        ),
    ],
    ids=("serialize-writers", "exact-logical-binding", "one-ahead-only",
         "bounded-reconciliation", "applied-is-terminal"),
)
def test_queue_plan_pending_membership_contract_rejects_persistence_guard_drift(
    tmp_path: Path, relative: str, symbol: str, old: str, new: str
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    replace_once_after(tmp_path / relative, f"fn {symbol}(", old, new)
    errors = validate_queue_plan_pending_membership_fixture(tmp_path, module, models)
    assert any(symbol in error and old in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "symbol", "earlier", "later"),
    [
        (
            "crates/iroha_core/src/state.rs",
            "persist_classified_queue_plan_admission",
            "let state_commit = self.state_commit_lock.lock();",
            "let disposition = Self::classify_pending_queue_plan_admission_in_view(",
        ),
        (
            "crates/iroha_core/src/kura.rs",
            "try_queue_plan_publication_at_height",
            "if actual_durable_height != expected_durable_height",
            "Ok(Some(KuraQueuePlanPublicationGuard {",
        ),
        (
            "crates/iroha_torii/src/lib.rs",
            "persist_queue_plan_admission_certificate",
            "snapshot.body = durable_certificate;",
            "disseminate_queue_plan_admission_publication(",
        ),
    ],
    ids=("state-fence-before-classification", "height-check-before-guard",
         "durable-body-before-publication"),
)
def test_queue_plan_pending_membership_contract_rejects_persistence_order_drift(
    tmp_path: Path, relative: str, symbol: str, earlier: str, later: str
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    swap_ordered_once_after(
        tmp_path / relative, f"fn {symbol}(", earlier, later
    )
    errors = validate_queue_plan_pending_membership_fixture(tmp_path, module, models)
    assert any(
        "ordered QueuePlan" in error and symbol in error for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_preserves_historical_applied(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once_after(
        path,
        "fn queue_plan_binding_application_evidence_in_view(",
        "QueuePlanBindingApplicationEvidence::AppliedDirect",
        "QueuePlanBindingApplicationEvidence::Absent",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_binding_application_evidence_in_view" in error
        and "QueuePlanBindingApplicationEvidence::AppliedDirect" in error
        for error in errors
    ), errors


def queue_plan_publication_source_contract_errors(module, sources: dict[str, str]) -> tuple[str, ...]:
    """Evaluate only the changed publication owners using the real checker contracts/parser."""
    symbols = {
        "persist_classified_queue_plan_admission",
        "try_queue_plan_publication_at_height",
        "wait_for_queue_plan_publication",
        "KuraQueuePlanPublicationGuard",
        "KuraQueuePlanPublicationGuard<'_>::retire",
        "KuraQueuePlanPublicationGuard<'_>::persist",
    }
    errors: list[str] = []
    items = {}
    for relative, kind, symbol, tokens in module.QUEUE_PLAN_PENDING_MEMBERSHIP_BINDINGS:
        if symbol not in symbols:
            continue
        found = module._extract_rust_binding_items(sources[relative], kind, symbol)
        if len(found) != 1:
            errors.append(f"{symbol}: expected one owner, found {len(found)}")
            continue
        items[(relative, kind, symbol)] = found[0]
        for token in tokens:
            if token not in found[0]:
                errors.append(f"{symbol}: missing {token!r}")
    for relative, kind, symbol, tokens in module.QUEUE_PLAN_PENDING_MEMBERSHIP_ORDERED_SOURCE_CHECKS:
        if symbol not in symbols:
            continue
        item = items.get((relative, kind, symbol))
        if item is None:
            continue
        cursor = -1
        for token in tokens:
            at = item.find(token, cursor + 1)
            if item.count(token) != 1 or at < 0:
                errors.append(f"{symbol}: reordered or duplicated {token!r}")
                break
            cursor = at
    module._validate_queue_plan_publication_lock_items(items, errors)
    return tuple(errors)


def test_queue_plan_publication_scoped_contract_accepts_current_owners() -> None:
    module = load_checker()
    sources = {relative: (ROOT_DIR / relative).read_text(encoding="utf-8") for relative in (
        "crates/iroha_core/src/state.rs", "crates/iroha_core/src/kura.rs",
    )}
    errors = queue_plan_publication_source_contract_errors(module, sources)
    assert errors == (), errors


@pytest.mark.parametrize(("relative", "old", "new"), [
    ("crates/iroha_core/src/state.rs", "drop(state_view);", "// retained StateView"),
    ("crates/iroha_core/src/state.rs", "self.kura.wait_for_queue_plan_publication();", "// skip the outside-State wait"),
    ("crates/iroha_core/src/state.rs", "parking_lot::MutexGuard::unlock_fair(state_commit);", "drop(state_commit);"),
    ("crates/iroha_core/src/state.rs", "self.authenticate_pending_queue_plan_admission(bytes)?", "self.authenticate_pending_queue_plan_admission(bytes).unwrap()"),
    ("crates/iroha_core/src/kura.rs", "self.canonical_chain_lock.try_lock()", "Some(self.canonical_chain_lock.lock())"),
    ("crates/iroha_core/src/kura.rs", "actual_durable_height != expected_durable_height", "actual_durable_height < expected_durable_height"),
    ("crates/iroha_core/src/kura.rs", "_guard: canonical_guard,", "_guard: self.canonical_chain_lock.lock(),"),
])
def test_queue_plan_publication_scoped_contract_rejects_lock_and_height_drift(
    relative: str, old: str, new: str,
) -> None:
    module = load_checker()
    sources = {path: (ROOT_DIR / path).read_text(encoding="utf-8") for path in (
        "crates/iroha_core/src/state.rs", "crates/iroha_core/src/kura.rs",
    )}
    symbol = "persist_classified_queue_plan_admission" if relative.endswith("state.rs") else "try_queue_plan_publication_at_height"
    owner = module._extract_rust_binding_items(sources[relative], "fn", symbol)[0]
    assert old in owner
    sources[relative] = sources[relative].replace(owner, owner.replace(old, new, 1), 1)
    assert queue_plan_publication_source_contract_errors(module, sources)


@pytest.mark.parametrize("symbol", [
    "KuraQueuePlanPublicationGuard<'_>::retire", "KuraQueuePlanPublicationGuard<'_>::persist",
])
def test_queue_plan_publication_scoped_contract_rejects_recursive_canonical_lock(symbol: str) -> None:
    module = load_checker()
    sources = {path: (ROOT_DIR / path).read_text(encoding="utf-8") for path in (
        "crates/iroha_core/src/state.rs", "crates/iroha_core/src/kura.rs",
    )}
    relative = "crates/iroha_core/src/kura.rs"
    owner = module._extract_rust_binding_items(sources[relative], "method", symbol)[0]
    changed = owner.replace("{", "{ let _recursive = self.kura.canonical_chain_lock.lock();", 1)
    sources[relative] = sources[relative].replace(owner, changed, 1)
    errors = queue_plan_publication_source_contract_errors(module, sources)
    assert any(symbol in error and "recursive" in error for error in errors), errors


@pytest.mark.parametrize(("symbol", "earlier", "later"), [
    ("persist_classified_queue_plan_admission", "drop(state_view);", ".try_queue_plan_publication_at_height(committed_height)"),
    ("persist_classified_queue_plan_admission", ".try_queue_plan_publication_at_height(committed_height)", "publication.retire(hash)?;"),
    ("persist_classified_queue_plan_admission", "parking_lot::MutexGuard::unlock_fair(state_commit);\n                    #[cfg(test)]", "self.kura.wait_for_queue_plan_publication();\n                    continue;"),
    ("try_queue_plan_publication_at_height", "if actual_durable_height != expected_durable_height", "Ok(Some(KuraQueuePlanPublicationGuard {"),
])
def test_queue_plan_publication_scoped_contract_rejects_reordered_authority(
    symbol: str, earlier: str, later: str,
) -> None:
    module = load_checker()
    sources = {path: (ROOT_DIR / path).read_text(encoding="utf-8") for path in (
        "crates/iroha_core/src/state.rs", "crates/iroha_core/src/kura.rs",
    )}
    relative = "crates/iroha_core/src/" + (
        "state.rs" if symbol == "persist_classified_queue_plan_admission" else "kura.rs"
    )
    owner = module._extract_rust_binding_items(sources[relative], "fn", symbol)[0]
    assert owner.count(earlier) == owner.count(later) == 1
    swapped = owner.replace(earlier, "__HELD_ORDER_SWAP__").replace(later, earlier).replace("__HELD_ORDER_SWAP__", later)
    sources[relative] = sources[relative].replace(owner, swapped, 1)
    assert queue_plan_publication_source_contract_errors(module, sources)


def test_queue_plan_publication_scoped_contract_rejects_wait_returning_authority() -> None:
    module = load_checker()
    sources = {path: (ROOT_DIR / path).read_text(encoding="utf-8") for path in (
        "crates/iroha_core/src/state.rs", "crates/iroha_core/src/kura.rs",
    )}
    relative = "crates/iroha_core/src/kura.rs"
    symbol = "wait_for_queue_plan_publication"
    owner = module._extract_rust_binding_items(sources[relative], "fn", symbol)[0]
    changed = owner.replace("(&self) {", "(&self) -> KuraQueuePlanPublicationGuard<'_> {", 1)
    assert changed != owner
    sources[relative] = sources[relative].replace(owner, changed, 1)
    assert any("wait must carry no" in error for error in queue_plan_publication_source_contract_errors(module, sources))


def test_queue_plan_publication_scoped_contract_keeps_native_control_anchors() -> None:
    module = load_checker()
    symbols = {
        "pending_queue_plan_publication_guard_checks_height_before_exposing_mutations",
        "pending_queue_plan_busy_kura_releases_state_and_reclassifies_after_publication",
        "pending_queue_plan_frontier_mismatch_preserves_all_retirement_candidates",
        "pending_queue_plan_height_mismatch_preserves_stale_conflicting_binding",
    }
    checked = set()
    for relative, symbol, tokens in module.QUEUE_PLAN_PENDING_MEMBERSHIP_TEST_BINDINGS:
        if symbol not in symbols:
            continue
        source = (ROOT_DIR / relative).read_text(encoding="utf-8")
        owners = module._extract_rust_binding_items(source, "fn", symbol)
        assert len(owners) == 1, symbol
        assert all(token in owners[0] for token in tokens), (symbol, [token for token in tokens if token not in owners[0]])
        checked.add(symbol)
    assert checked == symbols


@pytest.mark.parametrize(
    ("relative", "symbol", "old", "new"),
    [
        (
            "crates/iroha_core/src/state.rs",
            "authenticate_pending_queue_plan_admission",
            "&self.network_id,",
            "&NetworkId::from(\"wrong-network\"),",
        ),
        (
            "crates/iroha_core/src/state.rs",
            "validate_authenticated_queue_plan_admission_for_carrier_in_view",
            "exact_predecessor != context.predecessor_block_hash",
            "false",
        ),
        (
            "crates/iroha_torii/src/queue_plan_publication_wait.rs",
            "persist",
            "QueuePlanAdmissionPersistenceScope::Admission,",
            "QueuePlanAdmissionPersistenceScope::Carrier { height: 1 },",
        ),
        (
            "crates/iroha_torii/src/queue_plan_publication_wait.rs",
            "publication_overlap_height",
            "expected_durable_height.checked_add(1) == Some(*actual_durable_height)",
            "expected_durable_height < actual_durable_height",
        ),
    ],
    ids=("authenticated-network", "exact-history", "admission-scope", "one-ahead-wait"),
)
def test_queue_plan_pending_membership_contract_rejects_current_owner_drift(
    tmp_path: Path, relative: str, symbol: str, old: str, new: str,
) -> None:
    """Authentication, historical authority, and deadline retries retain their owners."""
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    replace_once_after(tmp_path / relative, f"fn {symbol}(", old, new)
    errors = validate_queue_plan_pending_membership_fixture(tmp_path, module, models)
    assert any(symbol in error and old in error for error in errors), errors


def test_queue_plan_pending_membership_contract_rejects_historical_authority_order_drift(
    tmp_path: Path,
) -> None:
    """The authenticated helper keeps exact history ahead of live authority lookup."""
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    symbol = "validate_authenticated_queue_plan_admission_for_carrier_in_view"
    swap_ordered_once_after(
        path,
        f"fn {symbol}(",
        "state_view.block_hashes().get(index).copied()",
        "queue_plan_authoritative_peers_in_view_at_height(",
    )
    errors = validate_queue_plan_pending_membership_fixture(tmp_path, module, models)
    assert any(
        "ordered QueuePlan" in error and symbol in error for error in errors
    ), errors
