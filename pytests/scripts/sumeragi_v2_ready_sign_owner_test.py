"""Current Ready Proposal Sign contracts reject lost authority even after resealing."""

from __future__ import annotations

import ast
import importlib.util
from pathlib import Path
import sys

import pytest


ROOT = Path(__file__).resolve().parents[2]
SOURCE_FILES = {
    "scheduler": "v2_lifecycle_scheduler_inputs.rs",
    "height_driver": "v2_runner/lifecycle_height_driver.rs",
    "driver": "v2_lifecycle_turn_driver.rs",
    "worker": "v2_worker.rs",
    "launch_tests": "v2_lifecycle_launch_tests.rs",
    "dispatch_test": "tests/v2_lifecycle_work_registry_validate_dispatch_execution_cases.rs",
    "wal_test": "tests/v2_adapter_04_wal_recovery.rs",
}


@pytest.fixture(scope="module")
def checker():
    path = ROOT / "scripts/formal/check_sumeragi_v2_proof_ledger.py"
    spec = importlib.util.spec_from_file_location("ready_sign_owner_checker", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def sources(checker):
    paths, sources, errors = {}, {}, []
    for role, name in SOURCE_FILES.items():
        paths[role], sources[role] = checker._read_reviewed_rust_source(
            ROOT, "crates/iroha_core/src/sumeragi/" + name, errors,
            "Ready Sign defining owner",
        )
    assert errors == []
    return paths, sources


@pytest.fixture(scope="module")
def probe(checker):
    """Invoke the real consumer call with its unchanged item and order predicates."""
    path = ROOT / "scripts/formal/sumeragi_v2_proof_ledger_successor_recovery_tail_contracts.py"
    owner, = [node for node in ast.parse(path.read_text()).body
              if isinstance(node, ast.FunctionDef)
              and node.name == "_lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors"]
    helpers = [node for node in owner.body if isinstance(node, ast.FunctionDef)
               and node.name in {"item", "require_order"}]
    assert len(helpers) == 2
    mint, = [node for node in owner.body if isinstance(node, ast.Assign)
             and any(isinstance(target, ast.Name)
                     and target.id == "blocked_lane_local_permit_mint"
                     for target in node.targets)]
    call, = [node for node in owner.body if isinstance(node, ast.Expr)
             and isinstance(node.value, ast.Call) and isinstance(node.value.func, ast.Name)
             and node.value.func.id == "_successor_recovery_ready_proposal_sign_source_fidelity_errors"]
    function = ast.parse(
        "def ready_probe(paths, sources):\n    errors = []\n    return errors\n"
    ).body[0]
    function.body[-1:-1] = helpers + [mint, call]
    module = ast.fix_missing_locations(ast.Module(body=[function], type_ignores=[]))
    namespace = dict(checker.__dict__)
    exec(compile(module, str(path), "exec"), namespace)
    return namespace["ready_probe"]


def test_current_ready_sign_owners(probe, sources):
    assert probe(*sources) == []


def test_ready_sign_review_ledger_matches_literal_mirror(checker):
    path = ROOT / "pytests/scripts/sumeragi_v2_proof_ledger_release_inventory_cases.py"
    mirror, = [node.test.comparators[0] for node in ast.walk(ast.parse(path.read_text()))
               if isinstance(node, ast.Assert) and isinstance(node.test, ast.Compare)
               and isinstance(node.test.left, ast.Attribute)
               and node.test.left.attr == "_PRODUCTION_READY_PROPOSAL_SIGN_PREEMPTION_ITEM_SHA256"]
    assert checker._PRODUCTION_READY_PROPOSAL_SIGN_PREEMPTION_ITEM_SHA256 == ast.literal_eval(mirror)


@pytest.mark.parametrize(("role", "key", "owner", "name", "old", "new"), (
    ("height_driver", "height::drain_lifecycle_v2_ingress", None,
     "drain_lifecycle_v2_ingress", "producer_claim.ready_proposal_sign_preemption_permit()",
     "producer_claim.unchecked_permit()"),
    ("height_driver", "height_test::only_an_eligible_claim_can_preempt_an_ordinary_head_for_ready_proposal_sign", None,
     "only_an_eligible_claim_can_preempt_an_ordinary_head_for_ready_proposal_sign",
     "LifecycleProducerClaimDispositionV1::AwaitingNativeSource", "LifecycleProducerClaimDispositionV1::Eligible"),
    ("driver", "driver::LaunchedProductionLifecycleV1::drive_completion_pre_gate_with_ready_proposal_sign_preemption",
     "LaunchedProductionLifecycleV1", "drive_completion_pre_gate_with_ready_proposal_sign_preemption",
     "self.drive_completion_pre_gate_inner(runner, Some(permit))",
     "self.drive_completion_pre_gate_inner(runner, None)"),
    ("driver", "driver::LaunchedProductionLifecycleV1::drive_completion_pre_gate_inner",
     "LaunchedProductionLifecycleV1", "drive_completion_pre_gate_inner",
     "proposal_sign_preemption.is_none()", "false"),
    ("driver", "driver::ActivatedProductionLifecycleV1::drive_completion_pre_gate_with_ready_proposal_sign_preemption",
     "ActivatedProductionLifecycleV1", "drive_completion_pre_gate_with_ready_proposal_sign_preemption",
     "(runner, permit)", "(runner, foreign_permit)"),
    ("dispatch_test", "dispatch_test::local_proposal_intent_live_wal_sign_fixture", None,
     "local_proposal_intent_live_wal_sign_fixture",
     ".drain_ordinary_completion_head_for_ready_sign_test()", ".discard_ordinary_completion()"),
))
def test_ready_sign_owner_loss_rejected_after_reseal(
    checker, sources, probe, monkeypatch, role, key, owner, name, old, new,
):
    paths, original = sources
    source = original[role]
    items = checker.rust_items(source, name)
    if owner is not None:
        items = [item for item in items if item.brace_context == (("impl", owner),)]
    item, = items
    assert item.source.count(old) == 1
    changed_item = item.source.replace(old, new, 1)
    assert source.count(item.source) == 1
    changed = {**original, role: source.replace(item.source, changed_item, 1)}
    changed_items = checker.rust_items(changed[role], name)
    if owner is not None:
        changed_items = [item for item in changed_items if item.brace_context == (("impl", owner),)]
    changed_item, = changed_items
    monkeypatch.setitem(checker._PRODUCTION_READY_PROPOSAL_SIGN_PREEMPTION_ITEM_SHA256,
                        key, checker._rust_item_token_sha256(changed_item))
    errors = probe(paths, changed)
    assert errors and any("preserve exact order" in error for error in errors), errors
