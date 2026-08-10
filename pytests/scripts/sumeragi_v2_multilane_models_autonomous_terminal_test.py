"""Autonomous terminal negative controls for the multilane source contract."""

from __future__ import annotations

from pathlib import Path

from sumeragi_v2_multilane_models_test import (
    ROOT_DIR,
    canonical_models,
    copy_reviewed_source_fixture_with_includes,
    load_checker,
    replace_once_after,
    swap_ordered_once_after,
)


def copy_autonomous_terminal_recovery_fixture(
    tmp_path: Path, module
) -> list[dict]:
    """Copy sources consumed by the terminal join and AUT-15 contract."""

    models = canonical_models()
    relatives = {
        Path(relative)
        for relative, _, _, _ in (
            *module.AUTONOMOUS_TERMINAL_RECOVERY_BINDINGS,
            *module.AUTONOMOUS_TERMINAL_TEST_BINDINGS,
        )
    }
    relatives.update(
        Path(relative)
        for relative, _, _, _ in module.AUTONOMOUS_TERMINAL_ORDERED_SOURCE_CHECKS
    )
    relatives.update(
        Path(relative)
        for relative, _, _, _ in module.AUTONOMOUS_TERMINAL_FORBIDDEN_SOURCE_CHECKS
    )
    relatives.add(Path(module.AUTONOMOUS_TERMINAL_TLA_RELATIVE))
    relatives.update(
        Path(relative)
        for relative, _, _ in module.AUTONOMOUS_TERMINAL_RAW_TEST_CHECKS
    )
    relatives.update(
        {
            module.REVIEWED_RUST_SOURCE_HELPER_RELATIVE,
            module.REVIEWED_RUST_INCLUDE_MANIFEST_RELATIVE,
        }
    )
    copy_reviewed_source_fixture_with_includes(tmp_path, module, relatives)
    return models


def validate_autonomous_terminal_recovery_fixture(
    tmp_path: Path, module, models: list[dict]
) -> tuple[str, ...]:
    errors: list[str] = []
    module.validate_autonomous_terminal_recovery_contract(
        tmp_path, models, errors, module._rust_binding_item
    )
    return tuple(errors)


def test_autonomous_terminal_recovery_contract_accepts_current_production(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    assert validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    ) == ()


def test_autonomous_terminal_recovery_rejects_prime_unchanged_vacuity(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = tmp_path / module.AUTONOMOUS_TERMINAL_TLA_RELATIVE
    replace_once_after(
        path,
        "PublishCanonicalQueueTerminalEvidence ==",
        "/\\ UNCHANGED canonicalTerminalBatchVars",
        "/\\ UNCHANGED canonicalTerminalBatchVars\n"
        "  /\\ UNCHANGED startupTerminalUnitVars",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "PublishCanonicalQueueTerminalEvidence" in error
        and "both assigns and leaves UNCHANGED" in error
        and "canonicalGroupATerminalPublished" in error
        for error in errors
    ), errors

def test_autonomous_terminal_recovery_rejects_per_group_canonical_cleanup(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_apply/reconciliation_authority.rs"
    )
    replace_once_after(
        path,
        "fn recover_pending_canonical_terminal_outcome(",
        ".authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcomes(",
        ".authenticate_autonomous_lifecycle_pending_canonical_queue_terminal_outcome(",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "recover_pending_canonical_terminal_outcome" in error
        and ("missing source-bound token" in error or "forbidden per-group" in error)
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_cleanup_before_all_group_preflight(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/queue/canonical_terminal_cleanup.rs"
    )
    swap_ordered_once_after(
        path,
        "fn commit_prepared_lane_reservation_carriers(",
        "self.preflight_lane_reservation_group_locked(",
        "for group in carriers.into_iter().flatten()",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "commit_prepared_lane_reservation_carriers" in error
        and "missing or reorders token" in error
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_first_carrier_only_mutation(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/queue/canonical_terminal_cleanup.rs"
    )
    replace_once_after(
        path,
        "fn commit_prepared_lane_reservation_carriers(",
        "for group in carriers.into_iter().flatten()",
        "for group in carriers.into_iter().next().into_iter().flatten()",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "commit_prepared_lane_reservation_carriers" in error
        and "carriers.into_iter().flatten()" in error
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_non_snapshot_carrier_bound(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_apply/committed_carrier_cleanup.rs"
    )
    replace_once_after(
        path,
        "fn finalize_startup_committed_canonical_carriers(",
        "let anchored_carrier_bound = authorized_commit_groups.len();",
        "let anchored_carrier_bound = carrier_publications.len();",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "finalize_startup_committed_canonical_carriers" in error
        and "authorized_commit_groups.len()" in error
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_unsorted_committed_carriers(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_apply/committed_carrier_cleanup.rs"
    )
    replace_once_after(
        path,
        "fn finalize_startup_committed_canonical_carriers(",
        "source_authorized_carriers.sort_by_key("
        "|(height, entry_hash, _, _, _)| (*height, *entry_hash));",
        "source_authorized_carriers.reverse();",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "finalize_startup_committed_canonical_carriers" in error
        and "sort_by_key" in error
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_budget_release_before_complete_loop(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_apply/reconciliation_authority.rs"
    )
    complete_loop = (
        "for evidence in terminal_evidence {\n"
        "        kura.complete_autonomous_lifecycle_canonical_terminal_outcome(evidence)?;\n"
        "    }"
    )
    exact_release = (
        "kura.release_post_wsv_lane_artifact_budget_reservation(\n"
        "        &entry,\n"
        "        carrier_block_height,\n"
        "        carrier_block_hash,\n"
        "    )?;"
    )
    swap_ordered_once_after(
        path,
        "fn recover_pending_canonical_terminal_outcome(",
        complete_loop,
        exact_release,
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "recover_pending_canonical_terminal_outcome" in error
        and (
            "only after the full Kura Complete publication loop" in error
            or "missing or reorders token" in error
        )
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_duplicate_budget_release(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_apply/reconciliation_authority.rs"
    )
    exact_release = (
        "kura.release_post_wsv_lane_artifact_budget_reservation(\n"
        "        &entry,\n"
        "        carrier_block_height,\n"
        "        carrier_block_hash,\n"
        "    )?;"
    )
    replace_once_after(
        path,
        "fn recover_pending_canonical_terminal_outcome(",
        exact_release,
        exact_release + "\n    " + exact_release,
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "recover_pending_canonical_terminal_outcome" in error
        and "must release exactly one exact post-WSV carrier reservation" in error
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_aggregate_finalized_journal_cap(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/queue/journal_reservation_commit_preflight.rs"
    )
    replace_once_after(
        path,
        "fn observe_startup_replay_receipt_with_finalized_absence(",
        "if phases.len() > self.limits.max_live_records",
        "if phases.len() + finalized_keys.len() > self.limits.max_live_records",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "absent carrier siblings must not share" in error
        or (
            "observe_startup_replay_receipt_with_finalized_absence" in error
            and "phases.len()" in error
        )
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_second_queue_plan_replay(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/queue/journal_reservation_commit_preflight.rs"
    )
    replay = (
        "let mut replay = "
        "self.prepare_replay_with_removed_entrypoints(Some(&entrypoints))?;"
    )
    replace_once_after(
        path,
        "fn observe_startup_replay_receipt_with_finalized_absence(",
        replay,
        replay + "\n    " + replay,
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "exactly one immutable QueuePlan replay snapshot" in error
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_unchecked_later_carrier_group(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/sumeragi/v2_apply/reconciliation_authority.rs"
    )
    replace_once_after(
        path,
        "fn recover_pending_canonical_terminal_outcome(",
        "if !authenticated_groups.is_empty()",
        "if false",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "recover_pending_canonical_terminal_outcome" in error
        and "authenticated_groups.is_empty()" in error
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_queue_planning_before_terminal_partition(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_runner.rs"
    swap_ordered_once_after(
        path,
        "fn run_inner(",
        "reconcile_lifecycle_terminal_outcomes_before_queue_planning(",
        "let planning = plan_lane_reservation_ownership(",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "run_inner" in error and "missing or reorders token" in error
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_all_owned_deferral_predicate(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs"
    replace_once_after(
        path,
        "fn reconcile_pending_autonomous_lifecycle_terminal_outcomes(",
        "let deferred = !owned_group_hashes.is_empty();",
        "let deferred = owned_group_hashes.len() == pending_groups.len();",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "reconcile_pending_autonomous_lifecycle_terminal_outcomes" in error
        and (
            "!owned_group_hashes.is_empty()" in error
            or "forbidden per-group" in error
        )
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_producer_without_queue_owner(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs"
    replace_once_after(
        path,
        "fn require_local_producer_queue_owner(",
        "local_actor != binding.producer_actor_projection()",
        "false",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "require_local_producer_queue_owner" in error
        and "producer_actor_projection" in error
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_unordered_producer_queue_owner(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs"
    replace_once_after(
        path,
        "fn exact_current_queue_group_matches(",
        "current_keys.as_slice() == ordered_keys",
        "ordered_keys.iter().all(|expected_key| current_keys.contains(expected_key))",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "exact_current_queue_group_matches" in error
        and "current_keys.as_slice() == ordered_keys" in error
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_unbound_live_queue_group(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs"
    replace_once_after(
        path,
        "fn exact_current_queue_group_matches(",
        "lane_queue_reservation_group_binding_from_ordered_keys(current_keys.iter()).ok()",
        "lane_queue_reservation_group_binding_from_ordered_keys(ordered_keys.iter()).ok()",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "exact_current_queue_group_matches" in error
        and "lane_queue_reservation_group_binding_from_ordered_keys(current_keys.iter()).ok()"
        in error
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_deferred_sibling_mutation(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_lifecycle_recovery.rs"
    replace_once_after(
        path,
        "// Consume the checked action-25 stutters",
        "seen_pending_identities.contains(&identity)",
        "false",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "reconcile_autonomous_lifecycle_startup" in error
        and "seen_pending_identities.contains(&identity)" in error
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_unvalidated_pending_padding(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/kura/pipeline_and_lane_artifacts.rs"
    )
    replace_once_after(
        path,
        "fn validate_body(",
        "!reserved_terminal.is_terminal_outcome_pending_reservation()",
        "false",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "AutonomousLifecycleTerminalOutcomeV1::validate_body" in error
        and "is_terminal_outcome_pending_reservation" in error
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_completion_length_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs"
    )
    replace_once_after(
        path,
        "fn complete_autonomous_lifecycle_terminal_outcome(",
        "if next_bytes.len() != current_bytes.len()",
        "if false",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "complete_autonomous_lifecycle_terminal_outcome" in error
        and "next_bytes.len() != current_bytes.len()" in error
        for error in errors
    ), errors


def test_autonomous_terminal_recovery_rejects_complete_without_queue_evidence(
    tmp_path: Path,
) -> None:
    module = load_checker()
    models = copy_autonomous_terminal_recovery_fixture(tmp_path, module)
    path = (
        tmp_path
        / "crates/iroha_core/src/kura/autonomous_lifecycle_terminal_outcomes.rs"
    )
    replace_once_after(
        path,
        "fn complete_autonomous_lifecycle_canonical_terminal_outcome(",
        "evidence.consume_for_kura()",
        "evidence.consume_for_kura_unchecked()",
    )
    errors = validate_autonomous_terminal_recovery_fixture(
        tmp_path, module, models
    )
    assert any(
        "complete_autonomous_lifecycle_canonical_terminal_outcome" in error
        and "evidence.consume_for_kura()" in error
        for error in errors
    ), errors
