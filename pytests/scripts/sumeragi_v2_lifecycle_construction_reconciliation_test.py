"""Copied-source controls for typed lifecycle construction and finalization."""

from __future__ import annotations

import hashlib
import ast
import importlib.util
import json
from pathlib import Path
import re
import shutil
import sys

import pytest


REPO = Path(__file__).resolve().parents[2]
PREFIX = Path("crates/iroha_core/src/sumeragi/v2_runner")
SPEC = importlib.util.spec_from_file_location(
    "lifecycle_construction_reconciliation_checker",
    REPO / "scripts/formal/check_sumeragi_v2_proof_ledger.py",
)
CHECKER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = CHECKER
assert SPEC.loader is not None
SPEC.loader.exec_module(CHECKER)
OWNERS = CHECKER._LIFECYCLE_CONSTRUCTION_RECONCILED_OWNERS


@pytest.fixture(scope="module")
def production_copy(tmp_path_factory):
    root = tmp_path_factory.mktemp("lifecycle-construction-owners")
    for relative in {row[0] for row in OWNERS.values()}:
        path = root / PREFIX / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(REPO / PREFIX / relative, path)
    return root


def test_lifecycle_construction_accepts_all_four_exact_owners(production_copy):
    assert CHECKER._lifecycle_construction_reconciled_owner_errors(production_copy) == []


def test_reviewed_loop_and_duplicate_locked_body_seals_match_production():
    for key in ("ordinary_loop", "pending_loop", "ordinary_active"):
        relative, symbol, _ = OWNERS[key]
        item = CHECKER.rust_items((REPO / PREFIX / relative).read_text(), symbol)[0]
        digest = CHECKER._rust_item_token_sha256(item)
        assert CHECKER._PRODUCTION_LIFECYCLE_EXACT_OUTPUT_ITEM_SHA256[key] == digest
        if key.startswith("ordinary"):
            assert CHECKER._LOCKED_BODY_REPROPOSAL_RUST_ITEM_SHA256[symbol] == digest


@pytest.mark.parametrize(
    ("key", "required"),
    [(key, token) for key, (_, _, required) in OWNERS.items() for token in required],
    ids=[f"{key}-guard{index}" for key, (_, _, required) in OWNERS.items() for index, _ in enumerate(required)],
)
def test_each_lifecycle_construction_delta_rejects_rehashed_source(production_copy, tmp_path, key, required):
    shutil.copytree(production_copy, tmp_path, dirs_exist_ok=True)
    relative, symbol, _ = OWNERS[key]
    path = tmp_path / PREFIX / relative
    before = path.read_bytes()
    item = CHECKER.rust_items(before.decode(), symbol)[0]
    tokens = CHECKER.rust_code_tokens(item.source)
    expected = CHECKER.rust_code_tokens(required)
    positions = CHECKER._token_sequence_positions(tokens, expected)
    assert len(positions) == 1
    identifier = next(index for index, token in enumerate(expected) if re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", token))
    spans = list(CHECKER._RUST_TOKEN_RE.finditer(CHECKER.mask_rust_comments_and_literals(item.source)))
    span = spans[positions[0] + identifier]
    mutant = item.source[:span.start()] + "removed_lifecycle_construction_authority" + item.source[span.end():]
    source = before.decode()
    assert source.count(item.source) == 1
    path.write_text(source.replace(item.source, mutant))
    changed = CHECKER.rust_items(path.read_text(), symbol)[0]
    rehashed_seal = CHECKER._rust_item_token_sha256(changed)
    failures = CHECKER._lifecycle_construction_reconciled_owner_errors(tmp_path)
    (tmp_path / "rehashed-control.json").write_text(json.dumps({
        "key": key, "guard": required,
        "old_file_sha256": hashlib.sha256(before).hexdigest(),
        "new_file_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "recomputed_owner_seal": rehashed_seal,
        "errors": failures,
    }, indent=2) + "\n")
    assert failures
    assert any(f"lifecycle construction {key} lost reviewed authority" in error for error in failures)


def test_complete_checker_calls_construction_contract_with_repo_root():
    source = (REPO / "scripts/formal/check_sumeragi_v2_proof_ledger.py").read_text()
    assert "errors.extend(_lifecycle_construction_reconciled_owner_errors(repo_root))" in source
    expected = {
        "ordinary lifecycle must keep only authenticated decided-lane recovery service alive until finalized-lane durability preflight succeeds": ("ordinary_active",),
        "ordinary finalized-lane recovery service must wait without reopening ordinary ownership when no authenticated terminal ingress was consumed": ("ordinary_active",),
        "lifecycle construction must move the unique service owner into the launch corridor": ("ordinary_loop", "pending_loop"),
    }
    found = set()
    for node in ast.walk(ast.parse(source)):
        if not (isinstance(node, ast.Call) and isinstance(node.func, ast.Name)
                and node.func.id == "_require_rust_token_sequence" and len(node.args) >= 4
                and isinstance(node.args[3], ast.Constant) and node.args[3].value in expected):
            continue
        description = node.args[3].value
        found.add(description)
        required = ast.literal_eval(node.args[2])
        for key in expected[description]:
            relative, symbol, _ = OWNERS[key]
            path = REPO / PREFIX / relative
            item = CHECKER.rust_items(path.read_text(), symbol)[0]
            failures = []
            CHECKER._require_rust_token_sequence(path, item, required, description, failures)
            assert failures == []
    assert found == set(expected)
    relative, symbol, _ = OWNERS["pending_active"]
    path = REPO / PREFIX / relative
    item = CHECKER.rust_items(path.read_text(), symbol)[0]
    failures = []
    CHECKER._require_rust_token_sequence(
        path, item, CHECKER._PRODUCTION_EXACT_OUTPUT_TOKEN_SEQUENCES["pending_lifecycle_rollover_wait"],
        "pending lifecycle closed predecessor wait", failures,
    )
    assert failures == []


@pytest.mark.parametrize(
    ("key", "original", "replacement"),
    (
        (
            "ordinary_loop",
            "LaneReservationReconciliationPlanning::AlreadyCompleted(observation) => { "
            "break observe_completed_lane_reservation_reconciliation("
            "queue.as_ref(), kura.as_ref(), observation,)?; }",
            "LaneReservationReconciliationPlanning::AlreadyCompleted(_observation) => { "
            "break LaneReservationReconciliationSummary::default(); }",
        ),
        (
            "ordinary_loop",
            "observe_completed_lane_reservation_reconciliation("
            "queue.as_ref(), kura.as_ref(), observation,)?",
            "observe_completed_lane_reservation_reconciliation("
            "queue.as_ref(), kura.as_ref(), observation,).unwrap_or_default()",
        ),
        (
            "ordinary_active",
            "let pending = lane_work.has_pending_historical_recovery()?;",
            "let pending = lane_work.has_pending_historical_recovery().unwrap_or(false);",
        ),
        (
            "ordinary_active",
            "let pending = lane_work.has_pending_historical_recovery()?;",
            "let pending = lane_work.has_pending_historical_recovery().unwrap_or(true);",
        ),
    ),
)
def test_incoming_completed_observation_and_pending_errors_remain_fail_closed(
    production_copy, tmp_path, key, original, replacement,
):
    """Fresh hashes cannot authorize bypassing the observed cut or swallowing reads."""
    shutil.copytree(production_copy, tmp_path, dirs_exist_ok=True)
    relative, symbol, _ = OWNERS[key]
    path = tmp_path / PREFIX / relative
    before = path.read_bytes()
    source = before.decode()
    item = CHECKER.rust_items(source, symbol)[0]
    tokens = CHECKER.rust_code_tokens(item.source)
    required = CHECKER.rust_code_tokens(original)
    positions = CHECKER._token_sequence_positions(tokens, required)
    assert len(positions) == 1
    spans = list(CHECKER._RUST_TOKEN_RE.finditer(CHECKER.mask_rust_comments_and_literals(item.source)))
    start = spans[positions[0]].start()
    end = spans[positions[0] + len(required) - 1].end()
    changed_owner = item.source[:start] + replacement + item.source[end:]
    assert source.count(item.source) == 1
    path.write_text(source.replace(item.source, changed_owner))
    changed = CHECKER.rust_items(path.read_text(), symbol)[0]
    errors = CHECKER._lifecycle_construction_reconciled_owner_errors(tmp_path)
    (tmp_path / "rehashed-incoming-control.json").write_text(json.dumps({
        "owner": key,
        "old_source_sha256": hashlib.sha256(before).hexdigest(),
        "new_source_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "new_owner_token_sha256": CHECKER._rust_item_token_sha256(changed),
        "errors": errors,
    }, indent=2) + "\n")
    assert path.read_bytes() != before
    assert any(f"lifecycle construction {key} lost reviewed authority" in error for error in errors)
