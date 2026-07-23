"""Focused tests for bounded Sumeragi v2 TLAPS proof sharding."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys


ROOT_DIR = Path(__file__).resolve().parents[2]
CHECKER_PATH = ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_proof_ledger.py"
SPEC = importlib.util.spec_from_file_location("sumeragi_v2_shard_checker", CHECKER_PATH)
assert SPEC is not None and SPEC.loader is not None
checker = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = checker
SPEC.loader.exec_module(checker)


def _sources() -> dict[str, str]:
    modules = [name for name, _ in checker.ASYNC_LIVENESS_SHARDS]
    modules.append(checker.ASYNC_LIVENESS_FACADE)
    return {
        module: (checker.FORMAL_DIR / f"{module}.tla").read_text(encoding="utf-8")
        for module in modules
    }


def test_checked_in_shards_are_bounded_ordered_and_uniquely_resolved() -> None:
    errors, providers = checker._async_liveness_shard_contract(_sources())

    assert errors == []
    assert providers["AsyncTypeInvariantObligation"] == (
        "SumeragiV2AsyncProtectedSlotProofs"
    )
    assert providers["ApplicationCompletionProgressObligation"] == (
        "SumeragiV2AsyncDecisionApplicationProofs"
    )
    assert providers["TimeoutViewProgressObligation"] == (
        checker.ASYNC_LIVENESS_DEBT_SHARD
    )


def test_shard_contract_rejects_missing_reordered_and_duplicate_sources() -> None:
    missing = _sources()
    missing.pop("SumeragiV2AsyncStage3Proofs")
    errors, _ = checker._async_liveness_shard_contract(missing)
    assert any("missing required async liveness shard" in error for error in errors)

    reordered = _sources()
    reordered["SumeragiV2AsyncStage3Proofs"] = reordered[
        "SumeragiV2AsyncStage3Proofs"
    ].replace(
        "EXTENDS SumeragiV2AsyncStage4RefinementProofs",
        "EXTENDS SumeragiV2AsyncStage2Proofs",
        1,
    )
    errors, _ = checker._async_liveness_shard_contract(reordered)
    assert any("must EXTEND exactly" in error for error in errors)

    duplicated = _sources()
    duplicated["SumeragiV2AsyncDecisionApplicationProofs"] = duplicated[
        "SumeragiV2AsyncDecisionApplicationProofs"
    ].replace(
        "=============================================================================",
        "AsyncTypeInvariantObligation == TRUE\nBY SMT\n\n"
        "=============================================================================",
        1,
    )
    errors, _ = checker._async_liveness_shard_contract(duplicated)
    assert any("declaration AsyncTypeInvariantObligation is duplicated" in error for error in errors)


def test_shard_contract_rejects_facade_declarations_caps_and_forward_references() -> None:
    facade = _sources()
    facade[checker.ASYNC_LIVENESS_FACADE] = facade[
        checker.ASYNC_LIVENESS_FACADE
    ].replace(
        "=============================================================================",
        "UnexpectedFacadeDeclaration == TRUE\n"
        "=============================================================================",
    )
    errors, _ = checker._async_liveness_shard_contract(facade)
    assert any("exact declaration-free compatibility facade" in error for error in errors)

    oversized = _sources()
    oversized["SumeragiV2AsyncDecisionApplicationProofs"] += "\\*" + (
        "x" * checker.ASYNC_LIVENESS_SHARD_MAX_BYTES
    )
    errors, _ = checker._async_liveness_shard_contract(oversized)
    assert any("exceeds 262144 bytes" in error for error in errors)

    forward = _sources()
    forward["SumeragiV2AsyncRankAndInitProofs"] = forward[
        "SumeragiV2AsyncRankAndInitProofs"
    ].replace(
        "=============================================================================",
        "ForwardReferenceProbe == OneHeightCompletionObligation\n\n"
        "=============================================================================",
    )
    errors, _ = checker._async_liveness_shard_contract(forward)
    assert any("forward async-family reference OneHeightCompletionObligation" in error for error in errors)


def test_debt_shard_allows_exactly_three_named_proofless_theorems() -> None:
    sources = _sources()
    sources[checker.ASYNC_LIVENESS_DEBT_SHARD] = sources[
        checker.ASYNC_LIVENESS_DEBT_SHARD
    ].replace(
        "=============================================================================",
        "THEOREM UnexpectedProofDebt == TRUE\n\n"
        "=============================================================================",
    )

    errors, _ = checker._async_liveness_shard_contract(sources)

    assert any("proofless theorems must equal" in error for error in errors)


def test_shard_contract_enforces_line_theorem_and_step_caps() -> None:
    line_heavy = _sources()
    line_heavy["SumeragiV2AsyncDecisionApplicationProofs"] = line_heavy[
        "SumeragiV2AsyncDecisionApplicationProofs"
    ].replace(
        "=============================================================================",
        ("\\* bounded line probe\n" * checker.ASYNC_LIVENESS_SHARD_MAX_LINES)
        + "=============================================================================",
        1,
    )
    errors, _ = checker._async_liveness_shard_contract(line_heavy)
    assert any("exceeds 5500 lines" in error for error in errors)

    theorem_heavy = _sources()
    declarations = "".join(
        f"THEOREM TheoremCapProbe{index} == TRUE\nBY SMT\n\n"
        for index in range(checker.ASYNC_LIVENESS_SHARD_MAX_THEOREMS + 1)
    )
    theorem_heavy["SumeragiV2AsyncDecisionApplicationProofs"] = theorem_heavy[
        "SumeragiV2AsyncDecisionApplicationProofs"
    ].replace(
        "=============================================================================",
        declarations + "=============================================================================",
        1,
    )
    errors, _ = checker._async_liveness_shard_contract(theorem_heavy)
    assert any("exceeds 150 top-level theorems" in error for error in errors)

    step_heavy = _sources()
    steps = "".join(
        f"  <1>{index}. TRUE\n" for index in range(1, checker.ASYNC_LIVENESS_THEOREM_MAX_STEPS + 2)
    )
    step_heavy["SumeragiV2AsyncDecisionApplicationProofs"] = step_heavy[
        "SumeragiV2AsyncDecisionApplicationProofs"
    ].replace(
        "=============================================================================",
        "THEOREM StepCapProbe == TRUE\nPROOF\n"
        + steps
        + "=============================================================================",
        1,
    )
    errors, _ = checker._async_liveness_shard_contract(step_heavy)
    assert any("exceeds 256 structured steps" in error for error in errors)


def test_shard_contract_rejects_reconstruction_drift() -> None:
    sources = _sources()
    sources["SumeragiV2AsyncDecisionApplicationProofs"] = sources[
        "SumeragiV2AsyncDecisionApplicationProofs"
    ].replace("Exact Decision-to-application", "Changed Decision-to-application", 1)

    errors, _ = checker._async_liveness_shard_contract(sources)

    assert any("not a mechanical partition" in error for error in errors)


def test_facade_evidence_maps_symbols_to_provider_logs() -> None:
    entries = checker._facade_provider_entries(checker.FORMAL_DIR, checker.ROOT_DIR)
    by_symbol = {entry["symbol"]: entry for entry in entries}

    assert by_symbol["AsyncTypeInvariantObligation"] == {
        "symbol": "AsyncTypeInvariantObligation",
        "module": "SumeragiV2AsyncProtectedSlotProofs",
        "log": (
            "target/formal/sumeragi_v2/tlaps/"
            "SumeragiV2AsyncProtectedSlotProofs.log"
        ),
    }
    assert by_symbol["TimeoutViewProgressObligation"] == {
        "symbol": "TimeoutViewProgressObligation",
        "module": checker.ASYNC_LIVENESS_DEBT_SHARD,
        "log": None,
    }
