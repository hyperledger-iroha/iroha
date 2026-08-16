"""Focused structural tests for chain/epoch refinement proof sharding."""

from __future__ import annotations

import hashlib
import importlib.util
from pathlib import Path
import sys


ROOT_DIR = Path(__file__).resolve().parents[2]
CHECKER_PATH = ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_proof_ledger.py"
SPEC = importlib.util.spec_from_file_location(
    "sumeragi_v2_chain_epoch_shard_checker", CHECKER_PATH
)
assert SPEC is not None and SPEC.loader is not None
checker = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = checker
SPEC.loader.exec_module(checker)


def _sources() -> dict[str, str]:
    modules = (
        *checker.CHAIN_EPOCH_REFINEMENT_SHARDS,
        checker.CHAIN_EPOCH_REFINEMENT_FACADE,
    )
    return {
        module: (checker.FORMAL_DIR / f"{module}.tla").read_text(
            encoding="utf-8"
        )
        for module in modules
    }


def test_checked_in_chain_epoch_shards_reconstruct_exact_source() -> None:
    sources = _sources()
    errors, providers = checker._chain_epoch_refinement_shard_contract(sources)

    assert errors == []
    assert len(checker.CHAIN_EPOCH_REFINEMENT_SHARDS) == 16
    theorem_counts = tuple(
        sum(
            kind == "theorem"
            for _, kind, _, _ in checker._top_level_declarations(sources[module])
        )
        for module in checker.CHAIN_EPOCH_REFINEMENT_SHARDS
    )
    assert theorem_counts == (16,) * 15 + (5,)
    assert max(theorem_counts) == checker.CHAIN_EPOCH_REFINEMENT_SHARD_MAX_THEOREMS
    assert providers["GenesisHeightSuccessorHandoffObligation"] == (
        "SumeragiV2ChainEpochRefinementShard03"
    )
    assert providers[
        "SuccessorActivationAndExactHistoricalRecoveryCrossToolRefinement"
    ] == "SumeragiV2ChainEpochRefinementShard16"

    bodies, framing_errors = checker._chain_epoch_refinement_shard_bodies(sources)
    assert framing_errors == []
    body = "".join(bodies)
    assert hashlib.sha256(body.encode("utf-8")).hexdigest() == (
        checker.CHAIN_EPOCH_REFINEMENT_PRE_SPLIT_BODY_SHA256
    )
    virtual_source = checker._chain_epoch_refinement_source(checker.FORMAL_DIR)
    assert virtual_source == (
        "---- MODULE SumeragiV2ChainEpochRefinement ----\n"
        + body
        + "=============================================================================\n"
    )
    assert hashlib.sha256(virtual_source.encode("utf-8")).hexdigest() == (
        "37cc24f314caa35b6f29ddf5c3bd8d9e3cc58daf7bb44c0d13f8e46c18858050"
    )


def test_chain_epoch_facade_and_release_roots_are_exact() -> None:
    sources = _sources()
    facade = checker.CHAIN_EPOCH_REFINEMENT_FACADE
    final_shard = checker.CHAIN_EPOCH_REFINEMENT_SHARDS[-1]

    assert sources[facade] == (
        f"---- MODULE {facade} ----\n"
        f"EXTENDS {final_shard}\n\n"
        "=============================================================================\n"
    )
    assert checker._top_level_declarations(sources[facade]) == []
    assert facade not in checker.RELEASE_PROOF_MODULES
    assert all(
        module in checker.RELEASE_PROOF_MODULES
        for module in checker.CHAIN_EPOCH_REFINEMENT_SHARDS
    )


def test_chain_epoch_promotion_and_facade_providers_are_physical() -> None:
    contracts = {
        contract.obligation_id: contract
        for contract in checker.PROMOTION_PROOF_TARGET_CONTRACTS
    }
    assert contracts["genesis-height-successor-handoff"].ledger_module == (
        checker.CHAIN_EPOCH_REFINEMENT_FACADE
    )
    assert contracts["genesis-height-successor-handoff"].provider_module == (
        "SumeragiV2ChainEpochRefinementShard03"
    )
    exact_recovery = contracts[
        "successor-activation-exact-recovery-production-refinement"
    ]
    assert exact_recovery.ledger_module == checker.CHAIN_EPOCH_REFINEMENT_FACADE
    assert exact_recovery.provider_module == (
        "SumeragiV2ChainEpochRefinementShard16"
    )

    entries = checker._facade_provider_entries(checker.FORMAL_DIR, checker.ROOT_DIR)
    by_symbol = {entry["symbol"]: entry for entry in entries}
    assert by_symbol["GenesisHeightSuccessorHandoffObligation"] == {
        "symbol": "GenesisHeightSuccessorHandoffObligation",
        "module": "SumeragiV2ChainEpochRefinementShard03",
        "log": (
            "formal/sumeragi_v2/tlaps/"
            "SumeragiV2ChainEpochRefinementShard03.log"
        ),
    }
    assert by_symbol[
        "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation"
    ]["module"] == "SumeragiV2ChainEpochRefinementShard16"


def test_chain_epoch_shard_contract_rejects_structural_mutations() -> None:
    facade_mutation = _sources()
    facade = checker.CHAIN_EPOCH_REFINEMENT_FACADE
    facade_mutation[facade] = facade_mutation[facade].replace(
        "=============================================================================",
        "UnexpectedFacadeTheorem == TRUE\nBY SMT\n\n"
        "=============================================================================",
        1,
    )
    errors, _ = checker._chain_epoch_refinement_shard_contract(facade_mutation)
    assert any("exact theorem-free ledger-facing facade" in error for error in errors)

    digest_mutation = _sources()
    first = checker.CHAIN_EPOCH_REFINEMENT_SHARDS[0]
    digest_mutation[first] = digest_mutation[first].replace(
        "DecisionReceiptProjection == durableDecisionEvidence = decisions",
        "DecisionReceiptProjection == durableDecisionEvidence \\subseteq decisions",
        1,
    )
    errors, _ = checker._chain_epoch_refinement_shard_contract(digest_mutation)
    assert any("exact ordered reconstruction" in error for error in errors)

    cap_mutation = _sources()
    cap_mutation[first] = cap_mutation[first].replace(
        "=============================================================================",
        "THEOREM SeventeenthShardTheoremMutation == TRUE\nBY SMT\n\n"
        "=============================================================================",
        1,
    )
    errors, _ = checker._chain_epoch_refinement_shard_contract(cap_mutation)
    assert any("exceeds 16 top-level theorems: found 17" in error for error in errors)

    duplicate_mutation = _sources()
    second = checker.CHAIN_EPOCH_REFINEMENT_SHARDS[1]
    duplicate_mutation[second] = duplicate_mutation[second].replace(
        "THEOREM AsyncHistoriesArePrefixComparable ==",
        "THEOREM AsyncChainInitProjectsAsyncInit ==",
        1,
    )
    errors, _ = checker._chain_epoch_refinement_shard_contract(duplicate_mutation)
    assert any(
        "declaration AsyncChainInitProjectsAsyncInit is duplicated" in error
        for error in errors
    )
