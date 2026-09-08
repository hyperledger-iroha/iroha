"""Rehashed adverse controls for the reviewed runner ownership contracts."""

from __future__ import annotations

import ast
import importlib.util
from pathlib import Path
import shutil
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]
PREFIX = "crates/iroha_core/src/sumeragi/"
OWNERS = {
    "preflight_finalized_lane_rollover": "v2_runner/finalized_output_rollover.rs",
    "rollover_finalized_height_outputs": "v2_runner/finalized_output_rollover.rs",
    "drain_finalized_lane_work_output": "v2_runner/finalized_output_rollover.rs",
    "retry_exact_output_and_apply_sidecar_admissions": "v2_runner.rs",
    "dispatch_lane_work_effects_with_progress": "v2_runner.rs",
    "drain_blocked_ordinary_lane_local_ingress": "v2_runner/decided_lane_recovery.rs",
    "apply_obsolete_merge_sidecar_generation_hints": "v2_runner.rs",
    "service_historical_recovery_tick": "v2_runner/canonical_recovery_ingress.rs",
}


def load_checker():
    """Load isolated seal maps for each mutation."""
    name = "runner_ack_reconciliation_checker"
    spec = importlib.util.spec_from_file_location(
        name, ROOT / "scripts/formal/check_sumeragi_v2_proof_ledger.py",
    )
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def copy_owners(root):
    """Copy the actual physical providers, retaining the full items."""
    for relative in set(OWNERS.values()):
        target = root / PREFIX / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / PREFIX / relative, target)


def owner_item(module, root, name):
    """Require exactly one top-level function in its physical provider."""
    path = root / PREFIX / OWNERS[name]
    source = path.read_text()
    items = module.rust_function_items_from_structural(
        source, module.mask_rust_comments_and_literals(source), name,
    )
    items = [item for item in items if item.brace_context == ()]
    assert len(items) == 1, (name, items)
    return items[0]


def bindings(module, name):
    """Include every permanent runner seal for the reviewed function."""
    result = []
    if name != "drain_blocked_ordinary_lane_local_ingress":
        result.append((module._PRODUCTION_RUNNER_ACK_SEAM_ITEM_SHA256, name))
    if name in module._PRODUCTION_EXACT_OUTPUT_RUNNER_ITEM_SHA256:
        result.append((module._PRODUCTION_EXACT_OUTPUT_RUNNER_ITEM_SHA256, name))
    return result


def owner_errors(module, root):
    """Execute the actual full-checker clauses for this independent owner cohort.

    This focused source test selects the existing top-level contract calls from
    the checker AST; it does not duplicate their expected token strings. The
    complete source-fidelity gate still runs separately.
    """
    path = ROOT / "scripts/formal/check_sumeragi_v2_proof_ledger.py"
    tree = ast.parse(path.read_text())
    function = next(node for node in tree.body if isinstance(node, ast.FunctionDef)
                    and node.name == "_exact_output_production_source_fidelity_errors")
    items = {name: owner_item(module, root, name) for name in OWNERS}
    errors = []
    for name, item in items.items():
        for mapping, key in bindings(module, name):
            assert key in mapping, f"missing reviewed runner seal: {key}"
            module._require_rust_item_token_sha256(
                root / PREFIX / OWNERS[name], item, mapping[key], name, errors,
            )
    environment = dict(vars(module), runner_path=root / PREFIX / "v2_runner.rs",
                       runner_ack_items=items, runner_items=items, errors=errors)
    selected = {name: 0 for name in OWNERS}
    for node in function.body:
        if not (isinstance(node, ast.Expr) and isinstance(node.value, ast.Call)):
            continue
        call = node.value
        if not (isinstance(call.func, ast.Name) and call.func.id in {
            "_require_rust_token_sequence", "_require_exact_rust_tokens",
        } and len(call.args) >= 2):
            continue
        item = call.args[1]
        if not (isinstance(item, ast.Call) and isinstance(item.func, ast.Attribute)
                and isinstance(item.func.value, ast.Name)
                and item.func.value.id in {"runner_ack_items", "runner_items"}
                and item.func.attr == "get" and len(item.args) == 1
                and isinstance(item.args[0], ast.Constant)):
            continue
        name = item.args[0].value
        if name not in selected:
            continue
        selected[name] += 1
        exec(compile(ast.Module(body=[node], type_ignores=[]), str(path), "exec"), environment)
    assert all(selected.values()), selected
    return errors


CASES = (
    ("retry_exact_output_and_apply_sidecar_admissions", "apply_obsolete_merge_sidecar_generation_hints(lane_work, services)?", "apply_obsolete_merge_sidecar_generation_hints(lane_work, services)", "runner retry must apply all retired source owners"),
    ("retry_exact_output_and_apply_sidecar_admissions", "apply_certified_merge_sidecar_chunk_admissions(lane_work, services, limit)?;", "let _ = limit;", "runner retry must observe writer flushes"),
    ("retry_exact_output_and_apply_sidecar_admissions", ".retry_pending_exact_output()", ".has_pending_exact_output()", "runner retry must observe writer flushes"),
    ("retry_exact_output_and_apply_sidecar_admissions", "apply_acknowledged_merge_sidecar_closes(lane_work, services)?", "apply_acknowledged_merge_sidecar_closes(lane_work, services)", "runner retry must apply all retired source owners"),
    ("dispatch_lane_work_effects_with_progress", "apply_obsolete_merge_sidecar_generation_hints(lane_work, services)?", "apply_retired_merge_sidecar_requests(lane_work, services)?", "runner lane dispatch must cancel retired source owners"),
    ("dispatch_lane_work_effects_with_progress", "let scan_limit = lane_work.effect_count();", "let scan_limit = limit;", "runner lane dispatch must apply receipts, preserve the bounded scan"),
    ("dispatch_lane_work_effects_with_progress", "if dispatched >= limit.max(1)", "if false", "runner lane dispatch must apply receipts, preserve the bounded scan"),
    ("drain_finalized_lane_work_output", "apply_obsolete_merge_sidecar_generation_hints(lane_work, services)?", "apply_obsolete_merge_sidecar_generation_hints(lane_work, services)", "durable finalization must cancel retired sources"),
    ("drain_finalized_lane_work_output", "if retired == 0 && dispatched == 0 && after >= before", "if retired == 0 && dispatched == 0 && after > before", "durable finalization must cancel retired sources"),
    ("drain_finalized_lane_work_output", "after == 0 && !pending", "after == 0", "durable finalization must cancel retired sources"),
    ("drain_finalized_lane_work_output", ".handoff_applied_height_output_to_durable_reconstruction(", ".discard_applied_height_output(", "durable finalization must cancel retired sources"),
    ("preflight_finalized_lane_rollover", "service_historical_recovery_tick(lane_work, services)?", "lane_work.service_next_historical_recovery()?", "finalized-lane preflight must keep the active predecessor alive"),
    ("preflight_finalized_lane_rollover", "if !executor.ready_to_finish()", "if false", "finalized-lane preflight must keep the active predecessor alive"),
    ("preflight_finalized_lane_rollover", "if lane_work.has_pending_historical_recovery()", "if false", "finalized-lane preflight must keep the active predecessor alive"),
    ("preflight_finalized_lane_rollover", "let _ = lane_work.persist_anchored_sessions()?;", "let _ = lane_work;", "finalized-lane preflight must keep the active predecessor alive"),
    ("rollover_finalized_height_outputs", "service_historical_recovery_tick(&mut lane_work, services)?", "lane_work.service_next_historical_recovery()?", "durable finalization must retry exact output"),
    ("rollover_finalized_height_outputs", "lane_work.persist_anchored_sessions()?;", "let _ = lane_work;", "finalized output rollover must durably settle every predecessor owner"),
    ("rollover_finalized_height_outputs", "if !lane_work.durable_completion_matches_finality(artifact)?", "if false", "finalized output rollover must durably settle every predecessor owner"),
    ("rollover_finalized_height_outputs", ".seal_applied_height_output_handoff(receipt, artifact, &durable_lane_authority)", ".seal_applied_height_output_handoff_unchecked()", "finalized output rollover must durably settle every predecessor owner"),
    ("drain_blocked_ordinary_lane_local_ingress", "accept_lane_message_with_ingress_ownership(inbound, active_view)?", "accept_lane_message_with_ingress_ownership(inbound, active_view)", "blocked ordinary lane-local drain must propagate admission failure"),
    ("drain_blocked_ordinary_lane_local_ingress", "Ok(true)", "Ok(false)", "blocked ordinary lane-local drain must propagate admission failure"),
    ("apply_obsolete_merge_sidecar_generation_hints", "if hints.is_empty()", "if true", "runner generation-fence cancellation must retain the entire hint batch"),
    ("apply_obsolete_merge_sidecar_generation_hints", "lane_work.requeue_obsolete_merge_sidecar_generation_hints(hints)?;", "let _ = hints;", "runner generation-fence cancellation must retain the entire hint batch"),
    ("apply_obsolete_merge_sidecar_generation_hints", "Err(V2RunnerError::Service(error))", "Ok(0)", "runner generation-fence cancellation must retain the entire hint batch"),
    ("service_historical_recovery_tick", ".has_pending_historical_recovery()", ".has_pending_committed_output_handoff()", "runner historical recovery must refresh archive targets"),
    ("service_historical_recovery_tick", "services.current_archive_targets()", "Vec::new()", "runner historical recovery must refresh archive targets"),
    ("service_historical_recovery_tick", ".map_err(V2RunnerError::from)", ".or(Ok(HistoricalRecoveryServiceOutcome::default()))", "runner historical recovery must refresh archive targets"),
)


def test_reviewed_runner_owner_contracts_accept_actual_sources(tmp_path):
    """The scoped canonical source contracts pass before adverse mutations."""
    copy_owners(tmp_path)
    assert owner_errors(load_checker(), tmp_path) == []


@pytest.mark.parametrize("name,old,new,diagnostic", CASES)
def test_runner_owner_mutations_survive_all_digest_refreshes(tmp_path, name, old, new, diagnostic):
    """Each semantic weakening fails even after every matching seal is refreshed."""
    module = load_checker()
    copy_owners(tmp_path)
    assert owner_errors(module, tmp_path) == []
    item = owner_item(module, tmp_path, name)
    assert item.source.count(old) == 1, (name, old)
    path = tmp_path / PREFIX / OWNERS[name]
    source = path.read_text()
    assert source.count(item.source) == 1
    path.write_text(source.replace(item.source, item.source.replace(old, new, 1), 1))
    changed = owner_item(module, tmp_path, name)
    for mapping, key in bindings(module, name):
        mapping[key] = module._rust_item_token_sha256(changed)
    errors = owner_errors(module, tmp_path)
    assert any(diagnostic in error for error in errors), errors
    assert not any("exact reviewed token digest" in error for error in errors), errors
