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
            Path("crates/iroha_core/src/queue/reservation_journal.rs"),
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
    errors = validate_queue_plan_autonomous_only_fixture(tmp_path, module, models)
    assert errors == (), errors


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


def test_queue_plan_autonomous_only_contract_rejects_wrong_intent_provider(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_autonomous_only_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_lane_work.rs"
    replace_once_after(
        path,
        "impl CandidateWorkProvider for &mut V2LaneWorkAdapter {",
        "== iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced)",
        "== iroha_data_model::transaction::TransactionAdmissionIntent::Ordinary)",
    )
    errors = validate_queue_plan_autonomous_only_fixture(tmp_path, module, models)
    assert any(
        "&mut V2LaneWorkAdapter::prepare" in error and ("QueuePlanSynced" in error or "exclusion changed" in error)
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
        "let canonical_recovery = (|| -> crate::kura::Result<bool> {",
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
            "let (certificate_hash, durable_input) = match outcome",
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
        "PublicationMutex",
        "PublicationGuard",
        "PhysicalPublicationGuard",
        "PublicationMutex::wrap",
        "PublicationMutex::lock",
        "PublicationGuard<'_, T>::unlock_fair",
        "PhysicalPublicationGuard<'_, T>::drop",
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
        "crates/iroha_core/src/publication_lock.rs",
    )}
    errors = queue_plan_publication_source_contract_errors(module, sources)
    assert errors == (), errors


@pytest.mark.parametrize(("relative", "old", "new"), [
    ("crates/iroha_core/src/state.rs", "drop(state_view);", "// retained StateView"),
    ("crates/iroha_core/src/state.rs", "self.kura.wait_for_queue_plan_publication();", "// skip the outside-State wait"),
    ("crates/iroha_core/src/state.rs", "state_commit.unlock_fair();", "drop(state_commit);"),
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
        "crates/iroha_core/src/publication_lock.rs",
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
        "crates/iroha_core/src/publication_lock.rs",
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
    ("persist_classified_queue_plan_admission", "state_commit.unlock_fair();\n                    #[cfg(test)]", "self.kura.wait_for_queue_plan_publication();\n                    continue;"),
    ("try_queue_plan_publication_at_height", "if actual_durable_height != expected_durable_height", "Ok(Some(KuraQueuePlanPublicationGuard {"),
])
def test_queue_plan_publication_scoped_contract_rejects_reordered_authority(
    symbol: str, earlier: str, later: str,
) -> None:
    module = load_checker()
    sources = {path: (ROOT_DIR / path).read_text(encoding="utf-8") for path in (
        "crates/iroha_core/src/state.rs", "crates/iroha_core/src/kura.rs",
        "crates/iroha_core/src/publication_lock.rs",
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
        "crates/iroha_core/src/publication_lock.rs",
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


@pytest.mark.parametrize("relative,anchor,old,new", [
    ("crates/iroha_core/src/sumeragi/v2_candidate.rs", "fn snapshot_routable_candidates(",
     "&binding,\n                    state.network_id_ref(),", "&binding,\n                    &NetworkId::default(),"),
    ("crates/iroha_core/src/sumeragi/v2_lane_work.rs", "impl CandidateWorkProvider for &mut V2LaneWorkAdapter {",
     "== iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced)",
     "== iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced && false)"),
    ("crates/iroha_core/src/sumeragi/v2_lane_work.rs", "impl CandidateWorkProvider for &mut V2LaneWorkAdapter {",
     "(reserved_entrypoints.contains(&entrypoint) || route_conflict).then_some(index)",
     "reserved_entrypoints.contains(&entrypoint).then_some(index)"),
    ("crates/iroha_core/src/sumeragi/v2_lane_work.rs", "impl CandidateWorkProvider for &mut V2LaneWorkAdapter {",
     "(reserved_entrypoints.contains(&entrypoint) || route_conflict).then_some(index)",
     "route_conflict.then_some(index)"),
    ("crates/iroha_core/src/torii_proxy.rs", "pub fn validate_queue_plan_binding_for_request(",
     "if binding.network_id_digest != queue_plan_admission_network_id_digest(network_id)",
     "if false && binding.network_id_digest != queue_plan_admission_network_id_digest(network_id)"),
    ("crates/iroha_core/src/torii_proxy.rs", "pub fn validate_queue_plan_binding_for_request(",
     "if binding.request_id != queue_plan_synced_request_id(network_id, transaction.hash())",
     "if false && binding.request_id != queue_plan_synced_request_id(network_id, transaction.hash())"),
    ("crates/iroha_core/src/torii_proxy.rs", "pub fn validate_queue_plan_binding_for_transaction_and_plan(",
     "        binding.enqueue_timestamp_ms,", "        0,"),
    ("crates/iroha_core/src/torii_proxy.rs", "pub fn validate_queue_plan_binding_for_transaction_and_plan(",
     "Some(binding.global_admission_identity()),", "None,"),
    ("crates/iroha_core/src/torii_proxy.rs", "pub fn validate_queue_plan_binding_for_transaction_and_plan(",
     "if exact_digest != binding.journal_record_digest", "if false && exact_digest != binding.journal_record_digest"),
])
def test_queue_plan_autonomous_only_current_claim_and_selection_mutations(
    tmp_path: Path, relative: str, anchor: str, old: str, new: str
) -> None:
    module = load_checker()
    models = copy_queue_plan_autonomous_only_fixture(tmp_path, module)
    replace_once_after(tmp_path / relative, anchor, old, new)
    errors = validate_queue_plan_autonomous_only_fixture(tmp_path, module, models)
    assert errors, "changed QueuePlan authority or unconditional ownership must fail"


def test_queue_plan_autonomous_only_rejects_binding_in_reexport_module(tmp_path: Path) -> None:
    module = load_checker()
    models = copy_queue_plan_autonomous_only_fixture(tmp_path, module)
    model = next(row for row in models if row["module"] == module.QUEUE_PLAN_STARTUP_REPLAY_MODULE)
    binding = next(row for row in model["production_symbols"] if row["symbol"] == "QueuePlanAdmissionBindingV1")
    binding["path"] = "crates/iroha_core/src/torii_proxy.rs"
    errors = validate_queue_plan_autonomous_only_fixture(tmp_path, module, models)
    assert any("QueuePlanAdmissionBindingV1" in error and "must occur exactly once" in error for error in errors), errors


@pytest.mark.parametrize(("kind", "symbol", "old", "new"), [
    ("method", "PublicationMutex::lock", "self.wrap(self.inner.lock())", "self.inner.lock()"),
    ("method", "PublicationMutex::wrap", "self.released.guard(PhysicalPublicationGuard {", "self.released.poisoning_guard(PhysicalPublicationGuard {"),
    ("method", "PublicationMutex::wrap", "guard: Some(guard),", "guard: None,"),
    ("method", "PublicationGuard<'_, T>::unlock_fair", "self.inner.fair = true;", "self.inner.fair = false;"),
    ("method", "PhysicalPublicationGuard<'_, T>::drop", "if self.fair {", "if !self.fair {"),
    ("method", "PhysicalPublicationGuard<'_, T>::drop", "parking_lot::MutexGuard::unlock_fair(guard);", "drop(guard);"),
    ("method", "PhysicalPublicationGuard<'_, T>::drop", "self.guard.take()", "None"),
    ("struct", "PublicationGuard", "mv::ReleaseGuard<'state, PhysicalPublicationGuard<'state, T>>", "PhysicalPublicationGuard<'state, T>"),
])
def test_queue_plan_publication_scoped_contract_rejects_release_owner_drift(
    kind: str, symbol: str, old: str, new: str,
) -> None:
    module = load_checker()
    relative = "crates/iroha_core/src/publication_lock.rs"
    sources = {path: (ROOT_DIR / path).read_text(encoding="utf-8") for path in (
        "crates/iroha_core/src/state.rs", "crates/iroha_core/src/kura.rs", relative,
    )}
    owner, = module._extract_rust_binding_items(sources[relative], kind, symbol)
    assert owner.count(old) == 1
    sources[relative] = sources[relative].replace(owner, owner.replace(old, new), 1)
    errors = queue_plan_publication_source_contract_errors(module, sources)
    assert any(symbol in error for error in errors), errors


from functools import lru_cache as _retained_route_cache


@_retained_route_cache(maxsize=1)
def retained_queue_plan_route_items() -> dict:
    """Extract the real retained-custody owners once, without changing providers."""
    load_checker()
    import sumeragi_v2_multilane_queue_plan_contract as contract

    module = load_checker()
    sources = {}
    items = {}
    for relative, kind, symbol, _ in contract.QUEUE_PLAN_RETAINED_ROUTE_BINDINGS:
        source = sources.setdefault(relative, (ROOT_DIR / relative).read_text())
        found = module._extract_rust_binding_items(source, kind, symbol)
        assert len(found) == 1, (relative, symbol, len(found))
        items[(relative, kind, symbol)] = found[0]
    return items


def test_retained_queue_plan_route_authority_accepts_actual_sources() -> None:
    """A retained closed claim defers ordinary work under every original owner."""
    load_checker()
    import sumeragi_v2_multilane_queue_plan_contract as contract

    errors = []
    contract.validate_retained_queue_plan_route_authority(retained_queue_plan_route_items(), errors)
    assert errors == [], errors


@pytest.mark.parametrize("symbol,old,new", [
    ("State::queue_plan_pending_route_authority_in_view", "=> return Ok(None)",
     "=> return Ok(Some(QueuePlanPendingRouteAuthority::Draining))"),
    ("State::queue_plan_pending_route_authority_in_view", "predecessor != context.predecessor_block_hash", "false"),
    ("State::queue_plan_pending_route_authority_in_view", "record.claim != binding.registry_value()", "false"),
    ("State::queue_plan_pending_route_authority_in_view", "record.priority.carrier_height < context.proposal_height", "false"),
    ("State::queue_plan_pending_route_authority_in_view", "record.priority.carrier_height > height", "false"),
    ("State::queue_plan_pending_route_authority_in_view", "for bound in &context.route_incarnations", "for bound in context.route_incarnations.iter().take(1)"),
    ("State::queue_plan_pending_route_authority_in_view", "lane.dataspace_id == route.dataspace_id", "true"),
    ("State::queue_plan_pending_route_authority_in_view", "next_height > drain.intent.close_global_height", "false"),
    ("State::queue_plan_pending_route_authority_in_view", "validate_autoscale_lane_committee_pops(&pin).map_err(str::to_owned)?;", ""),
    ("State::queue_plan_pending_route_authority_in_view", "record.priority.carrier_height > close", "false"),
    ("State::queue_plan_pending_route_authority_in_view", "close > height", "false"),
    ("State::queue_plan_pending_route_authority_in_view", "drain.commitment.is_some()", "false"),
    ("State::queue_plan_pending_route_authority_in_view", "state.network_id(),", "&NetworkId::default(),"),
    ("State::queue_plan_pending_route_authority_in_view", "state.lane_incarnation_at_height(route.lane_id, close)", "Some(bound.lane_incarnation)"),
    ("State::queue_plan_pending_route_authority_in_view", "pin.validator_set != bound.validator_set", "false"),
    ("State::queue_plan_pending_route_authority_in_view", "state.lane_incarnation_at_height(route.lane_id, next_height)", "Some(bound.lane_incarnation)"),
    ("Queue::durable_plan_claim_route_authority_in_view", "claim.global_admission_identity.is_some()", "true"),
    ("Queue::durable_plan_claim_route_authority_in_view", "State::queue_plan_pending_route_authority_in_view(state_view, &binding)", "Ok(Some(QueuePlanPendingRouteAuthority::Draining))"),
    ("Queue::revalidated_durable_plan_claim_retry_locked", "&existing.admission_context == expected_admission_context", "true"),
    ("Queue::has_revalidatable_durable_plan_claim_with_state", "Self::durable_plan_claim_route_authority_in_view(&state_view, &claim)", "Ok(QueuePlanPendingRouteAuthority::Active)"),
    ("Queue::immutable_queued_routing_plan_in_view", "&& claim.routing_plan == plan", "&& true"),
    ("Queue::immutable_queued_routing_plan_with_view", "authority == QueuePlanPendingRouteAuthority::Active", "true"),
    ("Queue::reserve_transactions_for_lane_bounded", "Ok((_, QueuePlanPendingRouteAuthority::Draining)) => continue,", "Ok((routing_plan, QueuePlanPendingRouteAuthority::Draining)) => routing_plan,"),
    ("Queue::pop_from_queue", "self.restore_popped_hash_locked(hash)", "Ok::<(), String>(())"),
    ("Queue::bounded_pending_snapshot", "Ok(Some(QueuePlanPendingRouteAuthority::Draining)) => {\n                                    blocked_by_fifo_predecessor = true;\n                                    return None;\n                                }", "Ok(Some(QueuePlanPendingRouteAuthority::Draining)) => {},"),
    ("Queue::push_with_lane_internal_with_state_and_routing", "Ok(Some(canonical_binding)) if canonical_binding == *binding", "Ok(Some(canonical_binding))"),
    ("Queue::revalidate_pending_transactions", "Ok((plan, _)) => plan,", "Ok((plan, QueuePlanPendingRouteAuthority::Active)) => plan,"),
    ("Queue::durable_plan_admission_claim_with_state", "context: claim.admission_context,", "context: current_context,"),
], ids=[
    "terminal-is-not-pending", "predecessor", "exact-registry", "source-before-rank",
    "rank-is-committed", "all-atomic-legs", "exact-dataspace", "closed-not-active",
    "pin-pops", "rank-before-close", "close-is-committed", "drain-not-terminal",
    "exact-network", "close-incarnation", "immutable-pin", "active-incarnation",
    "global-owner-required", "canonical-custody-required", "exact-retry-context",
    "retry-retained-authority", "immutable-plan", "ordinary-pop-projection",
    "ordinary-reservation-exclusion", "pop-restores-custody", "fifo-defers-draining",
    "ingress-exact-pending-owner", "refresh-retains-draining", "lookup-keeps-original-context",
])
def test_retained_queue_plan_route_authority_rejects_semantic_mutation(
    symbol: str, old: str, new: str,
) -> None:
    """Each mutation changes executable policy while leaving other checks intact."""
    load_checker()
    import sumeragi_v2_multilane_queue_plan_contract as contract

    items = retained_queue_plan_route_items().copy()
    key, = [key for key in items if key[2] == symbol]
    assert items[key].count(old) == 1, (symbol, old)
    items[key] = items[key].replace(old, new, 1)
    errors = []
    contract.validate_retained_queue_plan_route_authority(items, errors)
    assert any(symbol in error for error in errors), errors


@pytest.mark.parametrize("mutation", ["missing", "duplicate", "weakened"])
def test_retained_queue_plan_route_authority_requires_exact_ledger(tmp_path: Path, mutation: str) -> None:
    """Canonical pending authority cannot be silently dropped from the declared owner set."""
    module = load_checker()
    models = copy_queue_plan_autonomous_only_fixture(tmp_path, module)
    model = next(row for row in models if row["module"] == module.QUEUE_PLAN_STARTUP_REPLAY_MODULE)
    rows = model["production_symbols"]
    row, = [row for row in rows if row["symbol"] == "State::queue_plan_pending_route_authority_in_view"]
    if mutation == "missing":
        rows.remove(row)
    elif mutation == "duplicate":
        rows.append(copy.deepcopy(row))
    else:
        row["required_tokens"].pop()
    errors = validate_queue_plan_autonomous_only_fixture(tmp_path, module, models)
    assert any("State::queue_plan_pending_route_authority_in_view" in error for error in errors), errors


@pytest.mark.parametrize("old,new", [
    ("height != state._curr_block.height().get()", "false"),
    (".any(|current| current == lane)", ".any(|_| true)"),
    ("state.lane_incarnations.get(&lane.id).copied() != Some(incarnation)", "false"),
    ("if height < activation", "if false"),
    ("&& height > drain.intent.close_global_height", "&& false"),
    ("drain.intent.close_global_height > committed_height", "false"),
    ("drain.commitment.is_some()", "false"),
    ("if members.is_empty()", "if false"),
    ("for (_, member) in members", "for (_, member) in members.into_iter().take(1)"),
    ("State::queue_plan_pending_route_authority_in_view(state, &obligation.binding)?", "Some(QueuePlanPendingRouteAuthority::Draining)"),
    ("!= Some(QueuePlanPendingRouteAuthority::Draining)", "!= Some(QueuePlanPendingRouteAuthority::Active)"),
    ("!= Some(QueuePlanPendingRouteAuthority::Draining)", "== Some(QueuePlanPendingRouteAuthority::Draining)"),
    ("State::queue_plan_pending_route_authority_in_view(state, &obligation.binding)?", "State::queue_plan_pending_route_authority_in_view(state, &obligation.binding).unwrap_or(Some(QueuePlanPendingRouteAuthority::Draining))"),
    ("validate_autoscale_lane_committee_pops(&pin).map_err(str::to_owned)?;", ""),
], ids=[
    "staged-header", "staged-catalog", "staged-incarnation", "activation", "closed-branch",
    "committed-close", "terminal-drain", "pending-required", "every-member", "shared-owner",
    "draining-only", "refusal-polarity", "propagate-owner-error", "immutable-pin-pops",
])
def test_native_opening_retained_route_authority_rejects_semantic_mutation(old: str, new: str) -> None:
    """Native opening cannot turn retained custody into an unchecked opening authority."""
    load_checker()
    import sumeragi_v2_multilane_queue_plan_contract as contract

    items = retained_queue_plan_route_items().copy()
    key, = [key for key in items if key[2] == "resolve_open_lane_authority"]
    assert items[key].count(old) == 1, old
    items[key] = items[key].replace(old, new, 1)
    errors = []
    contract.validate_retained_queue_plan_route_authority(items, errors)
    assert any("resolve_open_lane_authority" in error for error in errors), errors


@pytest.mark.parametrize("mutation", ["missing", "duplicate", "weakened"])
def test_native_opening_retained_route_authority_requires_exact_ledger(tmp_path: Path, mutation: str) -> None:
    """The Native consumer must be declared independently of its State/Queue producers."""
    module = load_checker()
    models = copy_queue_plan_autonomous_only_fixture(tmp_path, module)
    model = next(row for row in models if row["module"] == module.QUEUE_PLAN_STARTUP_REPLAY_MODULE)
    rows = model["production_symbols"]
    row, = [row for row in rows if row["symbol"] == "resolve_open_lane_authority"]
    if mutation == "missing":
        rows.remove(row)
    elif mutation == "duplicate":
        rows.append(copy.deepcopy(row))
    else:
        row["required_tokens"].pop()
    errors = validate_queue_plan_autonomous_only_fixture(tmp_path, module, models)
    assert any("resolve_open_lane_authority" in error for error in errors), errors


@_retained_route_cache(maxsize=1)
def canonical_queue_plan_retry_items() -> dict:
    """Read exact canonical retry owners without replacing the live providers."""
    module = load_checker()
    import sumeragi_v2_multilane_queue_plan_contract as contract

    sources, items = {}, {}
    for path, kind, symbol, _ in contract.QUEUE_PLAN_CANONICAL_RETRY_BINDINGS:
        source = sources.setdefault(path, (ROOT_DIR / path).read_text())
        owner, = module._extract_rust_binding_items(source, kind, symbol)
        items[(path, kind, symbol)] = owner
    return items


def test_canonical_queue_plan_retry_accepts_actual_sources() -> None:
    """Existing canonical inputs remain proof-backed without a new ingress promise."""
    load_checker()
    import sumeragi_v2_multilane_queue_plan_contract as contract

    errors = []
    contract.validate_canonical_queue_plan_retry(canonical_queue_plan_retry_items(), errors)
    assert errors == [], errors


@pytest.mark.parametrize("symbol,old,new", [
    ("State::canonical_queue_plan_admitted_input", "Self::queue_plan_admission_registry_value_in_view(&view, entrypoint_hash)?;", ""),
    ("State::canonical_queue_plan_admitted_input", "if application == QueuePlanAdmissionApplicationState::PendingStale", "if false"),
    ("State::canonical_queue_plan_admitted_input", "record.priority.admission_index", "0"),
    ("State::canonical_queue_plan_admitted_input", "read.finality.height_context.network_id != self.network_id", "false"),
    ("State::canonical_queue_plan_admitted_input", "read.finality.height != record.priority.carrier_height", "false"),
    ("State::canonical_queue_plan_admitted_input", "input.certificate().registry_value != record.claim", "false"),
    ("State::canonical_queue_plan_admitted_input", "input.entrypoint().hash() != entrypoint_hash", "false"),
    ("State::canonical_queue_plan_admitted_input", "context.queue_plan_admissions.get(index)", "context.queue_plan_admissions.first()"),
    ("State::canonical_queue_plan_admitted_input", "if observe()?.as_ref() != Some(&(record, carrier_hash))", "if false"),
    ("State::canonical_queue_plan_admitted_input", "norito::with_decode_limits_scope(limits, || {", "(|| {"),
    ("Kura::read_first_admission_carrier_under_prune_and_canonical_guards", "retained_block_record_at_without_live_body", "retained_block_record_at"),
    ("Kura::read_first_admission_carrier_under_prune_and_canonical_guards", "if header != record.block_header", "if false"),
    ("Kura::read_first_admission_carrier_under_prune_and_canonical_guards", "body.canonical_proposal_wire_hash()? != finality.subject.payload_hash", "false"),
    ("canonical_queue_plan_submission_response", "Ok(false) => None,", "Ok(false) => Some(transaction_submission_receipt_response(app, entrypoint_hash, None, minimal_response, format)),"),
    ("canonical_queue_plan_submission_response", "Err(error) => Some(queue_plan_admission_registry_conflict_response(", "Err(error) => Some(transaction_submission_receipt_response("),
    ("canonical_queue_plan_synced_response", "Ok(QueuePlanAdmissionRegistryMatch::Absent) => return None,", "Ok(QueuePlanAdmissionRegistryMatch::Absent) => {},"),
    ("canonical_queue_plan_synced_response", "if &input.input().certificate.binding == binding", "if true"),
    ("canonical_queue_plan_synced_response", "utils::NoritoBody(input.into_input().certificate)", "utils::NoritoBody(newly_signed_certificate)"),
    ("canonical_queue_plan_synced_response", "tokio::task::block_in_place(", "tokio::task::spawn_blocking("),
    ("canonical_queue_plan_synced_response", "runtime.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread", "true"),
    ("canonical_queue_plan_synced_response", ".unwrap_or_else(|| acquire_torii_proxy_memory(app))", ".unwrap_or_else(|| Ok(unreserved_slot))"),
    ("execute_incoming_torii_proxy_request_with_admission_inner", "if admission_binding.request_id != canonical_request_id", "if false"),
    ("execute_incoming_torii_proxy_request_with_admission_inner", "validate_queue_plan_binding_for_request(", "validate_binding_structure_only("),
    ("handler_post_transaction_entrypoint", "routing::accept_transaction_for_ingress(state, transaction, &telemetry)", "Ok(unverified_transaction)"),
    ("decode_framed_versioned_signed_block_inner", "if canonical_len != raw_for_error.len()", "if false"),
    ("decode_framed_versioned_signed_block_inner", "if canonical.as_framed() != raw_for_error", "if false"),
    ("Kura::decode_v2_finality_record_at", "canonical_len != snapshot.bytes.len() ||", "false ||"),
    ("Kura::decode_canonical_retained_block_record", "&& canonical_len == Some(bytes.len())", "&& true"),
    ("canonical_admission_read_working_set_bytes", ".try_fold(0usize, usize::checked_add)", ".fold(Some(0usize), |_, _| Some(0))"),
    ('Kura::read_first_admission_carrier', 'self.read_first_admission_carrier_under_prune_and_canonical_guards(height, expected_hash)', 'self.read_first_admission_carrier_under_prune_and_canonical_guards(height, foreign_hash)'),
    ('Kura::read_first_admission_carrier', 'let _canonical = self.canonical_chain_lock.lock();', ''),
    ('Kura::read_first_admission_carrier_under_prune_and_canonical_guards', 'self.ensure_canonical_storage_not_poisoned()?;', 'let _again = self.canonical_chain_lock.lock(); self.ensure_canonical_storage_not_poisoned()?;'),
    ('canonical_admission_read_decode_limits', '(body, 1usize)', '(body, 0usize)'),
    ('canonical_admission_read_decode_limits', '(finality, 2)', '(finality, 1)'),
    ('canonical_admission_read_decode_limits', '(retained, 2)', '(retained, 1)'),
    ('canonical_admission_read_decode_limits', '.checked_mul(2)?', '.checked_mul(1)?'),
    ('canonical_admission_read_decode_limits', '(input, 1)', '(input, 0)'),
    ('canonical_admission_read_decode_limits', 'allocated.checked_add(limits.max_total_allocated_bytes().checked_mul(count)?)?', 'allocated'),
    ('State::canonical_queue_plan_input_decode_limits', 'crate::native_amx::MAX_NATIVE_AMX_PLAN_LEGS.checked_add(2)?', '2usize'),
    ('State::canonical_queue_plan_input_decode_limits', 'MAX_QUEUE_PLAN_PENDING_OBLIGATION_BYTES.checked_mul(4)?', 'MAX_QUEUE_PLAN_PENDING_OBLIGATION_BYTES'),
    ('State::canonical_queue_plan_input_decode_limits', 'one_elements.checked_mul(2)?', 'one_elements'),
    ('State::canonical_queue_plan_input_decode_limits', 'one_allocated.checked_mul(2)?', 'one_allocated'),
    ('State::canonical_queue_plan_input_read_working_set_bytes', '?.checked_add(state_graph)', '?.checked_add(0)'),
    ('canonical_admission_read_working_set_bytes', 'let proposal_clone = norito::canonical_decode_limits(wire).max_total_allocated_bytes();', 'let proposal_clone = 0usize;'),
    ('canonical_queue_plan_synced_response', 'if tokio::time::Instant::now() >= read_deadline', 'if false'),
    ('canonical_queue_plan_synced_response', 'if tokio::time::Instant::now() >= read_deadline', 'if tokio::time::Instant::now() < read_deadline'),
    ('execute_incoming_torii_proxy_request_with_admission_inner', 'proxy_memory.as_ref(),\n                execution_deadline,', 'proxy_memory.as_ref(),\n                tokio::time::Instant::now(),'),
    ('execute_incoming_torii_proxy_request_with_admission', '.checked_sub(TORII_PROXY_RESPONSE_EGRESS_RESERVE)', '.checked_sub(Duration::ZERO)'),
    ('execute_incoming_torii_proxy_request_with_admission', '            proxy_memory,\n            deadline,', '            proxy_memory,\n            tokio::time::Instant::now(),'),
], ids=[
    "orphan-is-not-absence", "stale-pending", "exact-rank", "network", "finality-height",
    "immutable-claim", "exact-body", "exact-admission-index", "rejoin-after-io", "cumulative-budget",
    "no-live-cache-read", "retained-header", "proposal-subject", "public-needs-registry",
    "public-corruption", "peer-needs-registry", "peer-exact-binding", "original-certificate",
    "no-detached-read", "runtime-custody", "working-set-required", "request-id", "complete-binding",
    "public-signature-validation", "count-before-materialize", "exact-wire-equality",
    "finality-encoded-bound", "retained-encoded-bound", "checked-peak",
    'original-guarded-delegation',
    'canonical-fence-held',
    'guarded-no-relock',
    'body-allowance',
    'both-finality-reads',
    'both-retained-reads',
    'both-sccp-passes',
    'selected-input-allowance',
    'cumulative-allocation-sum',
    'all-state-route-members',
    'pending-existing-policy',
    'both-state-element-observations',
    'both-state-allocation-observations',
    'state-graph-charged',
    'full-proposal-clone-charged',
    'original-deadline-required',
    'deadline-direction',
    'original-instant-handoff',
    'egress-budget-retained',
    'outer-timeout-same-instant',
])
def test_canonical_queue_plan_retry_rejects_semantic_mutation(symbol: str, old: str, new: str) -> None:
    """Keep each authority and bounded-read predicate live under independent mutations."""
    load_checker()
    import sumeragi_v2_multilane_queue_plan_contract as contract

    items = canonical_queue_plan_retry_items().copy()
    key, = [key for key in items if key[2] == symbol]
    assert items[key].count(old) == 1, (symbol, old, items[key].count(old))
    items[key] = items[key].replace(old, new, 1)
    errors = []
    contract.validate_canonical_queue_plan_retry(items, errors)
    assert any(symbol in error for error in errors), errors


@pytest.mark.parametrize("symbol,first,last", [
    ("State::canonical_queue_plan_admitted_input", "let Some((record, carrier_hash)) = observe()?", "let result = (|| {"),
    ("execute_incoming_torii_proxy_request_with_admission_inner", "let accepted_tx = match routing::accept_transaction_for_ingress(", "if let Some(response) = canonical_queue_plan_synced_response("),
    ("canonical_queue_plan_synced_response", "let reservation = match proxy_memory", "let read = match tokio::runtime::Handle::try_current()"),
    ("canonical_queue_plan_synced_response", "if tokio::time::Instant::now() >= read_deadline", "let mut response = ("),
])
def test_canonical_queue_plan_retry_rejects_owner_reordering(symbol: str, first: str, last: str) -> None:
    """A copied late check cannot stand in for admission before the physical operation."""
    load_checker()
    import sumeragi_v2_multilane_queue_plan_contract as contract

    items = canonical_queue_plan_retry_items().copy()
    key, = [key for key in items if key[2] == symbol]
    source = items[key]
    assert source.count(first) == source.count(last) == 1
    begin, end = source.index(first), source.index(last)
    assert begin < end
    # Keep every token in the owner, but move the first interval after its consumer.
    items[key] = source[:begin] + source[end:] + source[begin:end]
    errors = []
    contract.validate_canonical_queue_plan_retry(items, errors)
    assert any(symbol in error and "order" in error for error in errors), errors


@pytest.mark.parametrize("mutation", ["missing", "duplicate", "weakened"])
def test_canonical_queue_plan_retry_requires_exact_ledger(tmp_path: Path, mutation: str) -> None:
    """The canonical reader cannot disappear from the authenticated owner inventory."""
    module = load_checker()
    models = copy_queue_plan_autonomous_only_fixture(tmp_path, module)
    model = next(row for row in models if row["module"] == module.QUEUE_PLAN_STARTUP_REPLAY_MODULE)
    rows = model["production_symbols"]
    row, = [row for row in rows if row["symbol"] == "State::canonical_queue_plan_admitted_input"]
    if mutation == "missing":
        rows.remove(row)
    elif mutation == "duplicate":
        rows.append(copy.deepcopy(row))
    else:
        row["required_tokens"].pop()
    errors = validate_queue_plan_autonomous_only_fixture(tmp_path, module, models)
    assert any("State::canonical_queue_plan_admitted_input" in error for error in errors), errors
