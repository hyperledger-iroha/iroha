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
    module._validate_queue_plan_pending_membership_contract(
        tmp_path, models, errors
    )
    return tuple(errors)


def test_queue_plan_pending_membership_contract_accepts_current_production(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert errors == (), errors


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
    replace_once(
        path,
        "if storage.get(&obligation_key).is_none() {",
        "if false {",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "queue_plan_pending_route_members_from_storage" in error
        and "storage.get(&obligation_key).is_none()" in error
        for error in errors
    ), errors


def test_queue_plan_pending_membership_contract_rejects_full_roster_obligation_decode(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    replace_once_after(
        path,
        "fn queue_plan_pending_route_members_from_storage_with_limit(",
        "            let obligation_key = Self::queue_plan_pending_obligation_marker_key(\n",
        "            let _ = Self::decode_exact_queue_plan_pending_obligation_marker(\n"
        "                key, payload,\n"
        "            )?;\n"
        "            let obligation_key = Self::queue_plan_pending_obligation_marker_key(\n",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "without decoding the full obligation payload" in error for error in errors
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
    "symbol",
    (
        "resolve_queue_plan_pending_obligations_for_entrypoints",
        "resolve_required_queue_plan_pending_obligations",
    ),
)
def test_queue_plan_pending_membership_contract_rejects_bulk_apply_before_list(
    tmp_path: Path, symbol: str
) -> None:
    module = load_checker()
    models = copy_queue_plan_pending_membership_fixture(tmp_path, module)
    path = tmp_path / module.QUEUE_PLAN_PENDING_MEMBERSHIP_STATE_RELATIVE
    swap_ordered_once_after(
        path,
        f"fn {symbol}(",
        "State::resolve_queue_plan_pending_obligation_in_storage(",
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
    replace_once(
        path,
        "                    QueuePlanAdmissionApplicationState::PendingStale => {\n"
        "                        PendingQueuePlanAdmissionDisposition::Stale\n"
        "                    }",
        "                    QueuePlanAdmissionApplicationState::PendingStale => {\n"
        "                        PendingQueuePlanAdmissionDisposition::Exact\n"
        "                    }",
    )
    errors = validate_queue_plan_pending_membership_fixture(
        tmp_path, module, models
    )
    assert any(
        "classify_pending_queue_plan_admission" in error
        and "PendingQueuePlanAdmissionDisposition::Stale" in error
        for error in errors
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
