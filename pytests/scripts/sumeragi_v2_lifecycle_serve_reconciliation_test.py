"""Independent adverse controls for reviewed lifecycle Serve owner changes."""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import sys

import pytest


REPO = Path(__file__).resolve().parents[2]
OWNERS = {
    "turn": "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
    "height": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_height_driver.rs",
    "ordinary": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
    "pending": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
    "launch": "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
}
METHODS = {
    "height": "drain_lifecycle_v2_ingress",
    "ordinary": "run_lifecycle_active_height",
    "pending": "run_pending_active_height",
    "launch": "launch",
}


@pytest.fixture
def checker():
    """Load an isolated checker so every reseal is local to one adverse case."""
    name = "lifecycle_serve_reconciliation_checker"
    spec = importlib.util.spec_from_file_location(
        name, REPO / "scripts/formal/check_sumeragi_v2_proof_ledger.py",
    )
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


def _copy_owners(root):
    relatives = (*OWNERS.values(),
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_tests.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_recovered_fetch_source_tests.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_ready_proposal_sign_test_fixtures.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_pending_kura_source_tests.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_recovered_sign_settlement_source_tests.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_certified_response_retry_source_tests.rs",
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_recovered_fetch_settlement_source_tests.rs",
    )
    for relative in relatives:
        target = root / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(REPO / relative, target)


def _item(module, source, role, name):
    owner = {"turn": "LaunchedProductionLifecycleV1", "launch": "ProductionLifecycleOwnerV1"}.get(role)
    context = (("impl", owner),) if owner else ()
    items = [item for item in module.rust_items(source, name) if item.brace_context == context]
    assert len(items) == 1
    return items[0]


def _reseal(module, role, name, item):
    digest = module._rust_item_token_sha256(item)
    owner = {"turn": "LaunchedProductionLifecycleV1", "launch": "ProductionLifecycleOwnerV1"}.get(role)
    key = f"{role}:{owner + '::' if owner else ''}{name}"
    module._LIFECYCLE_CERTIFIED_SERVE_ITEM_SHA256[key] = digest
    if role in {"ordinary", "pending"}:
        module._PRODUCTION_LIFECYCLE_EXACT_OUTPUT_ITEM_SHA256[role + "_active"] = digest
    if role == "height":
        module._PRODUCTION_READY_PROPOSAL_SIGN_PREEMPTION_ITEM_SHA256[
            "height::drain_lifecycle_v2_ingress"
        ] = digest
    if name == "drive_completion_pre_gate_inner":
        module._PRODUCTION_READY_PROPOSAL_SIGN_PREEMPTION_ITEM_SHA256[
            "driver::LaunchedProductionLifecycleV1::drive_completion_pre_gate_inner"
        ] = digest
    return digest


def test_reviewed_lifecycle_serve_owner_deltas_are_accepted(tmp_path, checker):
    """The actual five-file production closure satisfies the semantic clauses."""
    _copy_owners(tmp_path)
    assert checker._lifecycle_certified_serve_reconciled_owner_errors(tmp_path) == []


@pytest.mark.parametrize("role,name,old,new,diagnostic", [
    ("turn", "drive_completion_pre_gate", "lane_work, None)", "lane_work, Some(permit))", "ordinary pre-gate cannot mint preemption"),
    ("turn", "drive_completion_pre_gate_inner", "proposal_sign_preemption.is_none()", "false", "physical ordinary head requires sealed preemption"),
    ("turn", "drive_completion_pre_gate_inner", "ready_proposal_sign_preempts_bounded_producer_point(fence)", "ready_proposal_sign_preempts_bounded_producer_point_unchecked(fence)", "physical ordinary head requires sealed preemption"),
    ("turn", "drive_completion_pre_gate_inner", "settle_deliver_and_acknowledge(&mut self.owner, &self.services)", "settle_without_acknowledgement(&mut self.owner, &self.services)", "Certified-Serve completion transport and fail-stop publication must retain ordered marker"),
    ("turn", "drive_completion_pre_gate_inner", "ProductionLifecycleCompletionSelectionV1::CertifiedServeReplayCompleted", "ProductionLifecycleCompletionSelectionV1::CertifiedServeClaimedCompleted", "Certified-Serve completion transport and fail-stop publication must retain ordered marker"),
    ("turn", "drive_completion_pre_gate_inner", "self.close_output_for_restart();", "let _ = self;", "physical ordinary head requires sealed preemption"),
    ("turn", "drive_ready_completion_turn_with_required_ordinal", "ordinal,\n                            None,", "ordinal,\n                            Some(completion),", "normal Ready dispatch cannot fabricate physical Validate custody"),
    ("height", "drain_lifecycle_v2_ingress", "producer_claim.required_ready_ordinal()", "None", "Runtime retains exact live Apply ordinal"),
    ("ordinary", "run_lifecycle_active_height", "producer_claim.required_ready_ordinal()", "None", "Runtime retains exact live Apply ordinal"),
    ("ordinary", "run_lifecycle_active_height", "settle_historical_body_serve_completion(", "ignore_historical_body_serve_completion(", "historical completion settles before active recovery"),
    ("ordinary", "run_lifecycle_active_height", "!block_sync_server.has_pending_historical_body_serve()", "true", "active rollover retains historical output owner"),
    ("ordinary", "run_lifecycle_active_height", "prepare_validate_sidecar_pacemaker_ingress_turn(permit)", "prepare_validate_sidecar_pacemaker_ingress_turn_unchecked(permit)", "sidecar ingress needs typed permit and prepared owner"),
    ("ordinary", "run_lifecycle_active_height", "set_ingress_physical_cut(receiver.next_physical_admission_ordinal())", "set_ingress_physical_cut(0)", "sidecar Runtime preserves physical cut and typed escape"),
    ("ordinary", "run_lifecycle_active_height", "step_pacemaker_after_completion_runtime_cut(", "step_after_completion_runtime_cut(", "sidecar Runtime preserves physical cut and typed escape"),
    ("ordinary", "run_lifecycle_active_height", "V2CompletionRuntimeCutDecisionV1::RetryCompletion => {}", "V2CompletionRuntimeCutDecisionV1::RetryCompletion => { break; }", "sidecar Runtime preserves physical cut and typed escape"),
    ("ordinary", "run_lifecycle_active_height", "step_completion_capacity_relief_after_cut(", "step_after_completion_runtime_cut(", "sidecar Runtime preserves physical cut and typed escape"),
    ("pending", "run_pending_active_height", "settle_historical_body_serve_completion(", "ignore_historical_body_serve_completion(", "pending completion settles before no-clock Serve"),
    ("pending", "run_pending_active_height", "!block_sync_server.has_pending_historical_body_serve()", "true", "pending rollover retains historical output owner"),
    ("pending", "run_pending_active_height", "let drained = drain_decided_lane_recovery_ingress(", "let drained = ignore_decided_lane_recovery_ingress(", "closed-prefix drain settles historical completion first"),
    ("pending", "run_pending_active_height", "if block_sync_server.has_pending_historical_body_serve()", "if false", "closed-prefix rollover cannot drop pending historical output"),
    ("ordinary", "run_lifecycle_active_height", "authenticate_terminal_complete_tip(", "skip_terminal_complete_tip_authentication(", "terminal height authenticates exact durable evidence"),
    ("pending", "run_pending_active_height", "proofs_of_possession,\n                        artifact,", "&[],\n                        artifact,", "terminal height authenticates exact durable evidence"),
    ("ordinary", "run_lifecycle_active_height", "return Ok(HeightRunOutcome::Terminal);", "return Ok(HeightRunOutcome::Shutdown);", "terminal shutdown follows evidence validation"),
    ("pending", "run_pending_active_height", "return Ok(HeightRunOutcome::Terminal);", "return Ok(HeightRunOutcome::Shutdown);", "terminal shutdown follows evidence validation"),
    ("launch", "launch", "&mut self.registry,", "&self.registry,", "launch restores registered sidecar custody"),
    ("launch", "launch", "inputs.kagemusha_mint_finality_authority,", "None,", "launch threads owned body store and typed service authority"),
    ("launch", "launch", "body_store,\n            payload_store_identity.clone(),", "foreign_body_store,\n            payload_store_identity.clone(),", "launch threads owned body store and typed service authority"),
    ("launch", "launch", "!services.matches_lifecycle_body_store(&body_store_identity)", "false", "launch checks output and storage identities before publication"),
])
def test_lifecycle_serve_owner_mutation_fails_after_reseal(
    tmp_path, checker, role, name, old, new, diagnostic,
):
    """A refreshed source digest cannot authorize each independent owner regression."""
    _copy_owners(tmp_path)
    path = tmp_path / OWNERS[role]
    source = path.read_text()
    original = _item(checker, source, role, name)
    assert old in original.source
    mutated_source = original.source.replace(old, new)
    assert mutated_source != original.source
    source = source.replace(original.source, mutated_source, 1)
    path.write_text(source)
    mutated = _item(checker, source, role, name)
    digest = _reseal(checker, role, name, mutated)
    seal_errors = []
    checker._require_rust_item_token_sha256(path, mutated, digest, name, seal_errors)
    assert seal_errors == []
    errors = checker._lifecycle_certified_serve_reconciled_owner_errors(tmp_path)
    assert any(diagnostic in error for error in errors), errors
    (tmp_path / "resealed-control.json").write_text(json.dumps({
        "role": role, "item": name, "mutant_file_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "refreshed_item_sha256": digest, "seal_errors": seal_errors,
        "expected_diagnostic": diagnostic, "semantic_errors": errors,
    }, indent=2) + "\n")
