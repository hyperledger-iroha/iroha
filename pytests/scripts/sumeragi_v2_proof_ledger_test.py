"""Tests for the fail-closed Sumeragi v2 formal proof ledger gate."""

from __future__ import annotations

import copy
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import pytest


ROOT_DIR = Path(__file__).resolve().parents[2]
SCRIPT = ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_proof_ledger.py"


def load_checker():
    spec = importlib.util.spec_from_file_location("sumeragi_v2_proof_ledger", SCRIPT)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def copy_async_source_fidelity_fixture(
    tmp_path: Path, module, *formal_names: str
) -> Path:
    """Copy the async formal inputs and their production-source bindings."""

    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for relative in (
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_effects.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
        Path("crates/iroha_sumeragi_core/src/verus_proofs.rs"),
        Path("crates/iroha_sumeragi_core/VERIFICATION.md"),
        Path("scripts/verify_sumeragi_v2.sh"),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)
    for name in formal_names:
        shutil.copyfile(module.FORMAL_DIR / name, formal_dir / name)
    return formal_dir


def mutate_tla_operator(
    source: str,
    symbol: str,
    old: str,
    new: str,
) -> str:
    """Replace one exact fragment inside a named top-level TLA+ operator."""

    declaration = re.search(
        rf"(?m)^{re.escape(symbol)}\s*(?:\([^)=]*\))?\s*==", source
    )
    assert declaration is not None, symbol
    next_declaration = re.search(
        r"(?m)^(?:[A-Za-z_][A-Za-z0-9_]*\s*"
        r"(?:\([^)=]*\))?\s*==|={4,}\s*$)",
        source[declaration.end() :],
    )
    operator_end = (
        len(source)
        if next_declaration is None
        else declaration.end() + next_declaration.start()
    )
    position = source.find(old, declaration.end(), operator_end)
    assert position >= 0, (symbol, old)
    return source[:position] + new + source[position + len(old) :]


def mutate_tla_theorem(
    source: str,
    symbol: str,
    old: str,
    new: str,
) -> str:
    """Replace one exact fragment inside a named top-level TLA+ theorem."""

    declaration = re.search(
        rf"(?m)^THEOREM\s+{re.escape(symbol)}\s*(?:\([^)=]*\))?\s*==",
        source,
    )
    assert declaration is not None, symbol
    next_declaration = re.search(
        r"(?m)^(?:(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)\s+"
        r"[A-Za-z_][A-Za-z0-9_]*\s*(?:\([^)=]*\))?\s*==|"
        r"[A-Za-z_][A-Za-z0-9_]*\s*(?:\([^)=]*\))?\s*==|={4,}\s*$)",
        source[declaration.end() :],
    )
    theorem_end = (
        len(source)
        if next_declaration is None
        else declaration.end() + next_declaration.start()
    )
    position = source.find(old, declaration.end(), theorem_end)
    assert position >= 0, (symbol, old)
    return source[:position] + new + source[position + len(old) :]


def delete_tla_theorem_token(source: str, symbol: str, token: str) -> str:
    """Delete every occurrence of a token inside one top-level theorem."""

    declaration = re.search(
        rf"(?m)^THEOREM\s+{re.escape(symbol)}\s*(?:\([^)=]*\))?\s*==",
        source,
    )
    assert declaration is not None, symbol
    next_declaration = re.search(
        r"(?m)^(?:(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)\s+"
        r"[A-Za-z_][A-Za-z0-9_]*\s*(?:\([^)=]*\))?\s*==|"
        r"[A-Za-z_][A-Za-z0-9_]*\s*(?:\([^)=]*\))?\s*==|={4,}\s*$)",
        source[declaration.end() :],
    )
    theorem_end = (
        len(source)
        if next_declaration is None
        else declaration.end() + next_declaration.start()
    )
    theorem = source[declaration.end() : theorem_end]
    assert token in theorem, (symbol, token)
    return (
        source[: declaration.end()]
        + theorem.replace(token, "")
        + source[theorem_end:]
    )


def test_tla_comment_stripping_reuses_bounded_content_cache() -> None:
    module = load_checker()
    source = 'Value == "kept" (* stripped *)\n'
    module.strip_tla_comments.cache_clear()

    assert module.strip_tla_comments(source) == 'Value == "    "               \n'
    first = module.strip_tla_comments.cache_info()
    assert (first.hits, first.misses, first.maxsize) == (0, 1, 64)

    assert module.strip_tla_comments(source) == 'Value == "    "               \n'
    second = module.strip_tla_comments.cache_info()
    assert (second.hits, second.misses, second.maxsize) == (1, 1, 64)

    assert module.strip_tla_comments(
        source, preserve_string_contents=True
    ) == 'Value == "kept"               \n'
    third = module.strip_tla_comments.cache_info()
    assert (third.hits, third.misses, third.maxsize) == (1, 2, 64)


def test_repository_ledger_has_only_explicit_unproved_debt() -> None:
    module = load_checker()
    ledger = module.load_ledger()
    result = module.validate_ledger(ledger)

    height = next(
        obligation
        for obligation in ledger["obligations"]
        if obligation["id"] == "height-liveness"
    )
    assert result.errors == ()
    assert height == {
        "id": "height-liveness",
        "requirement": (
            "Every joined post-GST context eventually applies at the terminal "
            "MaxHeight, or advances every responsive validator into a successor "
            "context when nonterminal"
        ),
        "module": "SumeragiV2ChainEpochRefinement",
        "symbol": "HeightLivenessObligation",
        "status": "specified_unproved",
    }
    assert result.machine_checked_completion is ledger["machine_checked_completion"]


def test_reviewed_obligation_inventory_rejects_deleted_obligation() -> None:
    module = load_checker()
    ledger = copy.deepcopy(module.load_ledger())
    ledger["obligations"] = [
        obligation
        for obligation in ledger["obligations"]
        if obligation["id"] != "dual-quorum-definition"
    ]

    errors = module.validate_ledger(ledger).errors

    assert (
        "proof ledger is missing reviewed obligation dual-quorum-definition" in errors
    )
    assert any("must follow the reviewed canonical order" in error for error in errors)


@pytest.mark.parametrize(
    ("field", "replacement", "expected_error"),
    (
        (
            "module",
            "SumeragiV2VocabularyProofs",
            "proof obligation dual-quorum-definition must use reviewed module "
            "SumeragiV2QuorumProofs",
        ),
        (
            "symbol",
            "PrepareSignerAvailabilityIncludesDurability",
            "proof obligation dual-quorum-definition must use reviewed symbol "
            "DualQuorumCarriesBothThresholds",
        ),
    ),
)
def test_reviewed_obligation_inventory_rejects_retargeting(
    field: str, replacement: str, expected_error: str
) -> None:
    module = load_checker()
    ledger = copy.deepcopy(module.load_ledger())
    ledger["obligations"][0][field] = replacement

    errors = module.validate_ledger(ledger).errors

    assert expected_error in errors


def test_repository_ledger_pins_exact_current_proof_debt_and_dependencies() -> None:
    module = load_checker()
    ledger = module.load_ledger()

    application = next(
        obligation
        for obligation in ledger["obligations"]
        if obligation["id"] == "application-liveness"
    )
    assert application["module"] == "SumeragiV2AsyncLivenessProofs"
    assert application["symbol"] == "ApplicationCompletionProgressObligation"
    assert application["status"] == "specified_unproved"
    assert ledger["machine_checked_completion"] is False

    assert tuple(
        obligation["id"]
        for obligation in ledger["obligations"]
        if obligation["status"] == "specified_unproved"
    ) == (
        "effective-lock-body-acquisition-model",
        "effective-lock-body-acquisition-production-refinement",
        "async-runner-scheduler-preservation",
        "async-type-invariant",
        "post-decision-timeout-exclusion",
        "decision-recovery-across-restart",
        "progress-witness-production-refinement",
        "progress-witness-preservation",
        "post-gst-deadlock-freedom",
        "protected-service-rank",
        "post-gst-starvation-freedom",
        "timeout-view-liveness",
        "rotating-leader-liveness",
        "application-liveness",
        "successor-activation-starvation-freedom",
        "successor-activation-exact-recovery-production-refinement",
        "genesis-height-successor-handoff",
        "height-liveness",
    )
    assert module.PROOF_STATUS_DEPENDENCIES == {
        "timeout-protection": ("historical-tc-lock-commit",),
        "async-type-invariant": ("async-runner-scheduler-preservation",),
        "post-decision-timeout-exclusion": ("async-type-invariant",),
        "decision-recovery-across-restart": ("async-type-invariant",),
        "progress-witness-preservation": (
            "async-type-invariant",
            "generation-scoped-vote-delivery",
            "post-decision-timeout-exclusion",
            "decision-recovery-across-restart",
            "progress-witness-production-refinement",
        ),
        "post-gst-deadlock-freedom": (
            "async-type-invariant",
            "async-fair-action-refinement",
        ),
        "protected-service-rank": (
            "async-type-invariant",
            "async-fair-action-refinement",
        ),
        "post-gst-starvation-freedom": (
            "async-type-invariant",
            "async-fair-action-refinement",
            "protected-service-rank",
        ),
        "timeout-view-liveness": (
            "async-fair-action-refinement",
            "progress-witness-preservation",
            "post-gst-starvation-freedom",
        ),
        "rotating-leader-liveness": (
            "effective-lock-body-acquisition-model",
            "effective-lock-body-acquisition-production-refinement",
            "async-fair-action-refinement",
            "progress-witness-preservation",
            "post-gst-starvation-freedom",
            "timeout-view-liveness",
        ),
        "application-liveness": (
            "async-fair-action-refinement",
            "progress-witness-preservation",
            "post-gst-starvation-freedom",
        ),
        "genesis-height-successor-handoff": (
            "rotating-leader-liveness",
            "application-liveness",
            "successor-activation-starvation-freedom",
            "successor-activation-exact-recovery-production-refinement",
        ),
        "height-liveness": (
            "rotating-leader-liveness",
            "application-liveness",
            "successor-activation-starvation-freedom",
            "successor-activation-exact-recovery-production-refinement",
        ),
    }
    assert module._proof_status_dependency_errors(ledger["obligations"]) == []


def test_every_declared_proof_dependency_fails_closed_on_early_promotion() -> None:
    module = load_checker()

    for dependent_id, prerequisite_ids in module.PROOF_STATUS_DEPENDENCIES.items():
        for prerequisite_id in prerequisite_ids:
            obligations = copy.deepcopy(module.load_ledger()["obligations"])
            by_id = {item["id"]: item for item in obligations}
            by_id[dependent_id]["status"] = "tlaps_proved"
            by_id[prerequisite_id]["status"] = "specified_unproved"

            errors = module._proof_status_dependency_errors(obligations)
            assert (
                f"proof obligation {dependent_id} cannot be tlaps_proved before "
                f"prerequisite {prerequisite_id} is tlaps_proved"
            ) in errors


@pytest.mark.parametrize(
    "dependent_id",
    (
        "post-gst-deadlock-freedom",
        "protected-service-rank",
        "post-gst-starvation-freedom",
        "timeout-view-liveness",
        "rotating-leader-liveness",
        "application-liveness",
    ),
)
def test_fairness_consuming_proof_cannot_precede_action_refinement(
    dependent_id: str,
) -> None:
    module = load_checker()
    obligations = copy.deepcopy(module.load_ledger()["obligations"])
    by_id = {item["id"]: item for item in obligations}
    by_id[dependent_id]["status"] = "tlaps_proved"
    by_id["async-fair-action-refinement"]["status"] = "specified_unproved"

    errors = module._proof_status_dependency_errors(obligations)

    assert (
        f"proof obligation {dependent_id} cannot be tlaps_proved before "
        "prerequisite async-fair-action-refinement is tlaps_proved"
    ) in errors


def test_historical_tc_lock_commit_exception_is_exact_and_wal_backed() -> None:
    module = load_checker()
    core_source = (module.FORMAL_DIR / "SumeragiV2Core.tla").read_text()
    async_source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text()

    historical = module._top_level_operator_body(
        core_source, "HistoricalTcLockedPrepareForCommit"
    )
    assert historical is not None
    historical_body = " ".join(historical[0].split())
    for required in (
        "qc \\in prepareQCs",
        "qc.view < nodeView[node]",
        "qc.view = lockRank[node]",
        "qc.subject = lockSubject[node]",
        "InstalledTcSelectsPrepareFor(node, qc)",
        "NoHigherConflictingPrepareKnown(node, qc)",
    ):
        assert required in historical_body

    conflict_fence = module._top_level_operator_body(
        core_source, "NoHigherConflictingPrepareKnown"
    )
    assert conflict_fence is not None
    conflict_body = " ".join(conflict_fence[0].split())
    assert "vote \\in prepareIntents" in conflict_body
    assert "vote.view > qc.view" in conflict_body
    assert "vote.subject # qc.subject" in conflict_body
    assert "highestRank[node] > qc.view" in conflict_body
    assert "highestSubject[node] # qc.subject" in conflict_body

    begin = module._top_level_operator_body(core_source, "BeginLockCommit")
    persist = module._top_level_operator_body(core_source, "PersistLockCommit")
    assert begin is not None
    assert persist is not None
    begin_body = " ".join(begin[0].split())
    persist_body = " ".join(persist[0].split())
    assert "CurrentOpenPrepareForCommit(node, qc)" in begin_body
    assert "HistoricalTcLockedPrepareForCommit(node, qc)" in begin_body
    assert "pendingLockCommit' = pendingLockCommit \\cup {request}" in begin_body
    assert "commitIntents' = commitIntents \\cup {request.vote}" in persist_body
    assert "signVotes' = signVotes \\cup {signRequest}" in persist_body

    successors = module._top_level_operator_body(async_source, "CommandSuccessors")
    assert successors is not None
    assert re.search(
        r'command\.kind = "ValidateBody"\s*->\s*'
        r'<<[^>]*CausalCandidate\("Progress", "BeginLockCommit", command\)',
        async_source,
        re.DOTALL,
    )

    by_id = {
        obligation["id"]: obligation
        for obligation in module.load_ledger()["obligations"]
    }
    assert by_id["historical-tc-lock-commit"]["status"] == "tlaps_proved"
    assert by_id["timeout-protection"]["status"] == "tlaps_proved"


def test_historical_timeout_authorization_is_derived_not_duplicated(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    invariant_path = formal_dir / "SumeragiV2Inductive.tla"
    proof_path = formal_dir / "SumeragiV2InductiveProofs.tla"
    invariant = r"""---- MODULE SumeragiV2Inductive ----
ReducerProvenanceInvariant ==
  /\ PendingVoteWritesAuthorized
  /\ DurableTimeoutsProtectCommits
ReducerProvenanceWithoutVoteTransport ==
  /\ PendingVoteWritesAuthorized
  /\ DurableTimeoutsProtectCommits
ReducerProvenanceWithoutTimeoutTransport ==
  /\ PendingVoteWritesAuthorized
  /\ DurableTimeoutsProtectCommits
=============================================================================
"""
    proof = r"""---- MODULE SumeragiV2InductiveProofs ----
THEOREM ReducerProvenanceImpliesHistoricalTcLockedCommitAuthorization ==
  ReducerProvenanceInvariant
    => HistoricalTcLockedCommitAuthorizationInvariant
BY PendingLowerLockCommitRequiresHistoricalTcAuthorization,
   DurableTimeoutProtectionSuppliesInstalledTcAuthorization
=============================================================================
"""
    invariant_path.write_text(invariant, encoding="utf-8")
    proof_path.write_text(proof, encoding="utf-8")

    assert module._historical_timeout_derivation_errors(formal_dir) == []

    invariant_path.write_text(
        invariant.replace(
            "/\\ DurableTimeoutsProtectCommits",
            "/\\ DurableTimeoutsProtectCommits\n"
            "  /\\ HistoricalTcLockedCommitAuthorizationInvariant",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._historical_timeout_derivation_errors(formal_dir)
    assert any("may not duplicate the derived" in error for error in errors)

    invariant_path.write_text(invariant, encoding="utf-8")
    proof_path.write_text(
        proof.replace(
            "ReducerProvenanceInvariant\n"
            "    => HistoricalTcLockedCommitAuthorizationInvariant",
            "TRUE",
        ),
        encoding="utf-8",
    )
    errors = module._historical_timeout_derivation_errors(formal_dir)
    assert any("must state only" in error for error in errors)


def test_completion_claim_rejects_unproved_debt_without_release_mode() -> None:
    module = load_checker()
    ledger = copy.deepcopy(module.load_ledger())
    ledger["machine_checked_completion"] = True

    errors = module.validate_ledger(ledger).errors

    assert any(
        error.startswith(
            "machine_checked_completion=true rejects specified_unproved obligations:"
        )
        for error in errors
    )


def test_temporal_proof_promotions_require_prerequisites_and_ledger_order() -> None:
    module = load_checker()
    obligations = copy.deepcopy(module.load_ledger()["obligations"])
    by_id = {obligation["id"]: obligation for obligation in obligations}

    for dependent_id, prerequisite_id in (
        ("async-type-invariant", "async-runner-scheduler-preservation"),
        ("post-gst-deadlock-freedom", "async-type-invariant"),
    ):
        by_id[dependent_id]["status"] = "tlaps_proved"
        errors = module._proof_status_dependency_errors(obligations)
        assert (
            f"proof obligation {dependent_id} cannot be tlaps_proved before "
            f"prerequisite {prerequisite_id} is tlaps_proved"
        ) in errors
        by_id[dependent_id]["status"] = "specified_unproved"

    by_id["post-gst-starvation-freedom"]["status"] = "tlaps_proved"
    errors = module._proof_status_dependency_errors(obligations)
    assert (
        "proof obligation post-gst-starvation-freedom cannot be tlaps_proved "
        "before prerequisite protected-service-rank is tlaps_proved"
    ) in errors

    by_id["post-gst-starvation-freedom"]["status"] = "specified_unproved"
    for dependent_id in (
        "genesis-height-successor-handoff",
        "height-liveness",
    ):
        by_id[dependent_id]["status"] = "tlaps_proved"
        errors = module._proof_status_dependency_errors(obligations)
        for prerequisite_id in (
            "rotating-leader-liveness",
            "application-liveness",
            "successor-activation-starvation-freedom",
            "successor-activation-exact-recovery-production-refinement",
        ):
            assert (
                f"proof obligation {dependent_id} cannot be tlaps_proved before "
                f"prerequisite {prerequisite_id} is tlaps_proved"
            ) in errors
        by_id[dependent_id]["status"] = "specified_unproved"

    rank_index = next(
        index
        for index, obligation in enumerate(obligations)
        if obligation["id"] == "protected-service-rank"
    )
    starvation_index = next(
        index
        for index, obligation in enumerate(obligations)
        if obligation["id"] == "post-gst-starvation-freedom"
    )
    obligations[rank_index], obligations[starvation_index] = (
        obligations[starvation_index],
        obligations[rank_index],
    )
    errors = module._proof_status_dependency_errors(obligations)
    assert (
        "proof obligation post-gst-starvation-freedom must appear after "
        "prerequisite protected-service-rank"
    ) in errors

    obligations = copy.deepcopy(module.load_ledger()["obligations"])
    fair = next(
        obligation
        for obligation in obligations
        if obligation["id"] == "async-fair-action-refinement"
    )
    obligations.remove(fair)
    protected_rank_index = next(
        index
        for index, obligation in enumerate(obligations)
        if obligation["id"] == "protected-service-rank"
    )
    obligations.insert(protected_rank_index + 1, fair)
    errors = module._proof_status_dependency_errors(obligations)
    assert (
        "proof obligation protected-service-rank must appear after "
        "prerequisite async-fair-action-refinement"
    ) in errors

    for dependent_id, prerequisite_id in (
        ("async-type-invariant", "async-runner-scheduler-preservation"),
        ("post-gst-deadlock-freedom", "async-type-invariant"),
    ):
        obligations = copy.deepcopy(module.load_ledger()["obligations"])
        dependent = next(
            obligation
            for obligation in obligations
            if obligation["id"] == dependent_id
        )
        obligations.remove(dependent)
        prerequisite_index = next(
            index
            for index, obligation in enumerate(obligations)
            if obligation["id"] == prerequisite_id
        )
        obligations.insert(prerequisite_index, dependent)
        errors = module._proof_status_dependency_errors(obligations)
        assert (
            f"proof obligation {dependent_id} must appear after prerequisite "
            f"{prerequisite_id}"
        ) in errors

    for dependent_id in (
        "genesis-height-successor-handoff",
        "height-liveness",
    ):
        obligations = copy.deepcopy(module.load_ledger()["obligations"])
        dependent = next(
            obligation
            for obligation in obligations
            if obligation["id"] == dependent_id
        )
        obligations.remove(dependent)
        rotating_index = next(
            index
            for index, obligation in enumerate(obligations)
            if obligation["id"] == "rotating-leader-liveness"
        )
        obligations.insert(rotating_index, dependent)
        errors = module._proof_status_dependency_errors(obligations)
        assert (
            f"proof obligation {dependent_id} must appear after prerequisite "
            "rotating-leader-liveness"
        ) in errors
        assert (
            f"proof obligation {dependent_id} must appear after prerequisite "
            "application-liveness"
        ) in errors
        assert (
            f"proof obligation {dependent_id} must appear after prerequisite "
            "successor-activation-starvation-freedom"
        ) in errors
        assert (
            f"proof obligation {dependent_id} must appear after prerequisite "
            "successor-activation-exact-recovery-production-refinement"
        ) in errors


def test_retired_v1_corridor_is_absent() -> None:
    module = load_checker()

    assert all(not module._retired_path_present(path) for path in module.RETIRED_PATHS)


def test_release_gate_fails_closed_while_completion_is_false() -> None:
    module = load_checker()
    result = module.validate_ledger(module.load_ledger(), release=True)

    assert "release gate requires machine_checked_completion=true" in result.errors
    assert any("release gate rejects unproved obligation" in error for error in result.errors)
    assert "release gate requires fresh TLAPS proof evidence" in result.errors


def complete_ledger(module):
    ledger = copy.deepcopy(module.load_ledger())
    ledger["machine_checked_completion"] = True
    for obligation in ledger["obligations"]:
        if obligation["status"] == "specified_unproved":
            obligation["status"] = "tlaps_proved"
    return ledger


def build_test_evidence(module, tmp_path: Path):
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    log_dir = tmp_path / "target" / "formal" / "sumeragi_v2" / "tlaps"
    log_dir.mkdir(parents=True)
    source_manifest_sha256 = module._formal_source_manifest(
        formal_dir, tmp_path
    )["sha256"]
    for name in module.RELEASE_PROOF_MODULES:
        (log_dir / f"{name}.log").write_text(
            "[INFO]: All 1 obligation proved.\n"
            f"{module._tlapm_runner_marker(name, source_manifest_sha256)}\n",
            encoding="utf-8",
        )
    evidence = module.build_release_evidence(
        tlapm_version=module.TLAPM_COMMIT[:7],
        log_dir=log_dir,
        formal_dir=formal_dir,
        root_dir=tmp_path,
    )
    return formal_dir, log_dir, evidence


def test_release_gate_requires_every_deductive_module_and_positive_counts(
    tmp_path: Path,
) -> None:
    module = load_checker()
    ledger = complete_ledger(module)
    formal_dir, _, evidence = build_test_evidence(module, tmp_path)

    assert module._release_evidence_errors(
        ledger, evidence, formal_dir=formal_dir, root_dir=tmp_path
    ) == []

    evidence["modules"].pop()
    errors = module._release_evidence_errors(
        ledger, evidence, formal_dir=formal_dir, root_dir=tmp_path
    )
    assert any("canonical order" in error for error in errors)

    _, _, evidence = build_test_evidence(module, tmp_path / "reordered")
    evidence["modules"][0], evidence["modules"][1] = (
        evidence["modules"][1],
        evidence["modules"][0],
    )
    errors = module._release_evidence_errors(
        ledger,
        evidence,
        formal_dir=tmp_path / "reordered" / "docs" / "formal" / "sumeragi_v2",
        root_dir=tmp_path / "reordered",
    )
    assert any("canonical order" in error for error in errors)


def test_release_evidence_is_bound_to_current_sources_and_logs(tmp_path: Path) -> None:
    module = load_checker()
    ledger = complete_ledger(module)
    formal_dir, log_dir, evidence = build_test_evidence(module, tmp_path)

    first_source = next(formal_dir.glob("*.tla"))
    first_source.write_text(first_source.read_text() + "\n\\* drift\n", encoding="utf-8")
    errors = module._release_evidence_errors(
        ledger, evidence, formal_dir=formal_dir, root_dir=tmp_path
    )
    assert "proof evidence source manifest does not match current TLA+ sources" in errors

    formal_dir, log_dir, evidence = build_test_evidence(module, tmp_path / "fresh")
    first_log = log_dir / f"{module.RELEASE_PROOF_MODULES[0]}.log"
    first_log.write_text(first_log.read_text() + "drift\n", encoding="utf-8")
    errors = module._release_evidence_errors(
        ledger, evidence, formal_dir=formal_dir, root_dir=tmp_path / "fresh"
    )
    assert any("log digest mismatch" in error for error in errors)


def test_release_evidence_rejects_stale_or_marker_stuffed_logs(tmp_path: Path) -> None:
    module = load_checker()
    ledger = complete_ledger(module)
    formal_dir, log_dir, evidence = build_test_evidence(module, tmp_path)
    first_module = module.RELEASE_PROOF_MODULES[0]
    first_log = log_dir / f"{first_module}.log"

    stale_manifest = "0" * 64
    first_log.write_text(
        "[INFO]: All 1 obligation proved.\n"
        f"{module._tlapm_runner_marker(first_module, stale_manifest)}\n",
        encoding="utf-8",
    )
    evidence["modules"][0]["log_sha256"] = module._sha256_file(first_log)
    errors = module._release_evidence_errors(
        ledger, evidence, formal_dir=formal_dir, root_dir=tmp_path
    )
    assert any("manifest-bound successful suffix" in error for error in errors)

    _, log_dir, evidence = build_test_evidence(module, tmp_path / "stuffed")
    first_log = log_dir / f"{first_module}.log"
    first_log.write_text(
        "[INFO]: All 999 obligations proved.\n" + first_log.read_text(encoding="utf-8"),
        encoding="utf-8",
    )
    evidence["modules"][0]["log_sha256"] = module._sha256_file(first_log)
    errors = module._release_evidence_errors(
        ledger,
        evidence,
        formal_dir=tmp_path / "stuffed" / "docs" / "formal" / "sumeragi_v2",
        root_dir=tmp_path / "stuffed",
    )
    assert any("manifest-bound successful suffix" in error for error in errors)


def test_release_evidence_requires_exact_pinned_tool_identity(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    log_dir = tmp_path / "target" / "formal" / "sumeragi_v2" / "tlaps"
    log_dir.mkdir(parents=True)

    with pytest.raises(ValueError, match="must equal pinned identity"):
        module.build_release_evidence(
            tlapm_version=f"forged-{module.TLAPM_COMMIT[:7]}",
            log_dir=log_dir,
            formal_dir=formal_dir,
            root_dir=tmp_path,
        )


def test_tlaps_proved_symbol_must_be_a_theorem_declaration() -> None:
    module = load_checker()
    source = """---- MODULE Example ----
OperatorOnly == TRUE
THEOREM Proved == TRUE
=============================================================================
"""

    assert module._symbol_exists(source, "OperatorOnly")
    assert not module._symbol_exists(source, "OperatorOnly", theorem_only=True)
    assert module._symbol_exists(source, "Proved", theorem_only=True)
    assert module._symbol_exists(
        "---- MODULE Example ----\n  THEOREM LocalOnly == TRUE\n====\n",
        "LocalOnly",
        theorem_only=True,
    )
    assert not module._symbol_exists(
        "---- MODULE Example ----\nMalformed(arg) = Wrapper(value)\n====\n",
        "Malformed",
    )
    assert not module._symbol_exists(
        "---- MODULE Example ----\nTHEOREM Malformed(arg) = Wrapper(value)\n====\n",
        "Malformed",
        theorem_only=True,
    )


def test_tlaps_proved_obligation_must_use_a_release_proof_module() -> None:
    module = load_checker()
    ledger = copy.deepcopy(module.load_ledger())
    obligation = ledger["obligations"][0]
    obligation["status"] = "tlaps_proved"
    obligation["module"] = "SumeragiV2Core"
    obligation["symbol"] = "Init"

    errors = module.validate_ledger(ledger).errors
    assert any("claims TLAPS proof in non-release module" in error for error in errors)


def test_release_module_list_covers_every_present_module_with_theorems() -> None:
    module = load_checker()
    theorem_modules = {
        path.stem
        for path in module.FORMAL_DIR.glob("*.tla")
        if re.search(r"(?m)^THEOREM\b", module.strip_tla_comments(path.read_text()))
    }
    present_release_modules = {
        name
        for name in module.RELEASE_PROOF_MODULES
        if (module.FORMAL_DIR / f"{name}.tla").is_file()
    }
    source_fidelity_theorem_modules: set[str] = set()
    async_source = module.strip_tla_comments(
        (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text()
    )
    async_theorems = tuple(
        re.findall(
            r"(?m)^[ \t]*(?:LOCAL[ \t]+)?"
            r"(?:THEOREM|LEMMA|COROLLARY|PROPOSITION)[ \t]+"
            r"([A-Za-z_][A-Za-z0-9_]*)\b",
            async_source,
        )
    )

    assert theorem_modules == (
        present_release_modules | source_fidelity_theorem_modules
    )
    assert async_theorems == ()


def test_async_production_model_and_proofs_are_ci_gated() -> None:
    module = load_checker()

    assert "SumeragiV2AsyncNetwork" in module.REQUIRED_MODEL_MODULES
    assert "SumeragiV2TemporalLemmas" in module.REQUIRED_MODEL_MODULES
    assert "SumeragiV2ServiceRankLemmas" in module.REQUIRED_MODEL_MODULES
    assert "SumeragiV2AsyncLivenessProofs" in module.REQUIRED_MODEL_MODULES
    assert "SumeragiTimeoutIngressGuardTest" in module.REQUIRED_MODEL_MODULES
    assert "SumeragiV2TimeoutDurability" in module.REQUIRED_MODEL_MODULES
    assert "SumeragiV2TimeoutSigningInvariant" in module.REQUIRED_MODEL_MODULES
    assert "SumeragiV2TimeoutViewInvariant" in module.REQUIRED_MODEL_MODULES
    assert "SumeragiV2TimeoutWireAuthorization" in module.REQUIRED_MODEL_MODULES
    assert "SumeragiV2TemporalLemmas" in module.RELEASE_PROOF_MODULES
    assert "SumeragiV2ServiceRankLemmas" in module.RELEASE_PROOF_MODULES
    assert "SumeragiV2EffectiveLockAcquisition" in module.REQUIRED_MODEL_MODULES
    assert (
        "SumeragiV2EffectiveLockAcquisitionProofs"
        in module.RELEASE_PROOF_MODULES
    )
    assert (
        "SumeragiV2AsyncFairnessRefinementProofs"
        in module.RELEASE_PROOF_MODULES
    )
    assert "SumeragiV2AsyncLivenessProofs" in module.RELEASE_PROOF_MODULES
    assert "SumeragiTimeoutIngressGuardTest" in module.RELEASE_PROOF_MODULES
    assert "SumeragiV2TimeoutDurability" in module.RELEASE_PROOF_MODULES
    assert "SumeragiV2TimeoutSigningInvariant" in module.RELEASE_PROOF_MODULES
    assert "SumeragiV2TimeoutViewInvariant" in module.RELEASE_PROOF_MODULES
    assert "SumeragiV2TimeoutWireAuthorization" in module.RELEASE_PROOF_MODULES
    assert "SumeragiV2AsyncNetwork" not in module.RELEASE_PROOF_MODULES


def test_first_release_type_and_height_debt_targets_are_pinned() -> None:
    module = load_checker()
    ledger = module.load_ledger()
    by_id = {obligation["id"]: obligation for obligation in ledger["obligations"]}

    expected_status = {
        "timeout-wire-authorization": "tlaps_proved",
        "historical-tc-lock-commit": "tlaps_proved",
        "effective-lock-body-acquisition-model": "specified_unproved",
        "effective-lock-body-acquisition-production-refinement": (
            "specified_unproved"
        ),
        "async-type-invariant": "specified_unproved",
        "successor-activation-starvation-freedom": "specified_unproved",
        "successor-activation-exact-recovery-production-refinement": (
            "specified_unproved"
        ),
        "genesis-height-successor-handoff": "specified_unproved",
        "height-liveness": "specified_unproved",
    }
    for obligation_id, target in module.FIXED_PROOF_OBLIGATION_TARGETS.items():
        target_module, symbol = target
        obligation = by_id[obligation_id]
        assert obligation["module"] == target_module
        assert obligation["symbol"] == symbol
        assert obligation["status"] == expected_status[obligation_id]

    drifted = copy.deepcopy(ledger["obligations"])
    height = next(item for item in drifted if item["id"] == "height-liveness")
    height["module"] = "SumeragiV2Proofs"
    errors = module._proof_obligation_architecture_errors(drifted, {})
    assert any(
        "proof obligation height-liveness must use SumeragiV2ChainEpochRefinement"
        in error
        for error in errors
    )


def test_effective_lock_body_model_and_production_refinement_remain_separate_debt() -> None:
    module = load_checker()
    ledger = module.load_ledger()
    by_id = {obligation["id"]: obligation for obligation in ledger["obligations"]}

    model = by_id["effective-lock-body-acquisition-model"]
    assert model == {
        "id": "effective-lock-body-acquisition-model",
        "requirement": model["requirement"],
        "module": "SumeragiV2EffectiveLockAcquisitionProofs",
        "symbol": "EffectiveLockAcquisitionModelObligation",
        "status": "specified_unproved",
    }
    model_source = (
        module.FORMAL_DIR / "SumeragiV2EffectiveLockAcquisitionProofs.tla"
    ).read_text()
    model_theorem = module._top_level_theorem_body(
        model_source, "EffectiveLockAcquisitionModelObligation"
    )
    assert model_theorem is not None
    assert "AcquisitionSpec" in model_theorem[0]
    assert "EffectiveLockAcquisitionProgress" in model_theorem[0]
    assert "StableEffectiveLockDelivery" in model_theorem[0]

    refinement = by_id[
        "effective-lock-body-acquisition-production-refinement"
    ]
    assert refinement == {
        "id": "effective-lock-body-acquisition-production-refinement",
        "requirement": refinement["requirement"],
        "module": "SumeragiV2AsyncLivenessProofs",
        "symbol": "EffectiveLockBodyAcquisitionProductionRefinementObligation",
        "status": "specified_unproved",
    }
    source = (module.FORMAL_DIR / "SumeragiV2AsyncLivenessProofs.tla").read_text()
    theorem = module._top_level_theorem_body(
        source, "EffectiveLockBodyAcquisitionProductionRefinementObligation"
    )
    assert theorem is not None
    statement = theorem[0].strip()
    assert statement == "ProductionEffectiveLockBodyAcquisitionRefinement"
    for proposition in (
        "ProductionEnterViewUsesPostInstallEffectiveLock",
        "ProductionBodyOwnershipPreservesEffectiveLock",
        "ProductionBodyCapacityRetirementPreservesEffectiveLock",
        "ProductionBodyServiceRefinesAsyncFairness",
    ):
        assert f"{proposition} = TRUE" in source


def copy_effective_lock_acquisition_fixture(tmp_path: Path, module) -> Path:
    """Copy the executable owner, proof boundary, and adversarial TLC configs."""

    formal_dir = tmp_path / "sumeragi_v2"
    formal_dir.mkdir()
    for name in (
        "SumeragiV2EffectiveLockAcquisition.tla",
        "SumeragiV2EffectiveLockAcquisitionProofs.tla",
        "SumeragiV2EffectiveLockAcquisitionMutation.tla",
        "effective_lock_acquisition.cfg",
        "effective_lock_rebind_fixed.cfg",
        "effective_lock_rebind_bug.cfg",
        "effective_lock_no_retry_bug.cfg",
        "effective_lock_future_completion_bug.cfg",
    ):
        shutil.copyfile(module.FORMAL_DIR / name, formal_dir / name)
    return formal_dir


def test_effective_lock_acquisition_source_fidelity_is_green(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = copy_effective_lock_acquisition_fixture(tmp_path, module)

    assert module._effective_lock_acquisition_source_fidelity_errors(formal_dir) == []


@pytest.mark.parametrize(
    ("name", "symbol", "old", "new", "expected_error"),
    (
        (
            "SumeragiV2EffectiveLockAcquisition.tla",
            "PhysicalCompletionDisposition",
            "completionId > physicalId",
            "completionId < physicalId",
            "PhysicalCompletionDisposition must match",
        ),
        (
            "SumeragiV2EffectiveLockAcquisition.tla",
            "RebindSameLock",
            "consumerGeneration' = nextGeneration",
            "consumerGeneration' = consumerGeneration",
            "RebindSameLock must match",
        ),
        (
            "SumeragiV2EffectiveLockAcquisition.tla",
            "InstallHigherLock",
            'acquisitionPhase = "Loading"',
            'acquisitionPhase = "Waiting"',
            "InstallHigherLock must match",
        ),
        (
            "SumeragiV2EffectiveLockAcquisition.tla",
            "AcquisitionSpec",
            "/\\ WF_acquisitionVars(RetryRecoveredBody)",
            "",
            "AcquisitionSpec must match",
        ),
        (
            "SumeragiV2EffectiveLockAcquisition.tla",
            "StableEffectiveLockDelivery",
            "/\\ CurrentConsumerDelivered",
            "/\\ TRUE",
            "StableEffectiveLockDelivery must match",
        ),
        (
            "SumeragiV2EffectiveLockAcquisitionMutation.tla",
            "NoRetryNext",
            "CompleteOwnedLoad",
            "RetryRecoveredBody",
            "NoRetryNext must retain adversarial clause",
        ),
    ),
)
def test_effective_lock_acquisition_operator_mutations_fail_closed(
    tmp_path: Path,
    name: str,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_effective_lock_acquisition_fixture(tmp_path, module)
    path = formal_dir / name
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new), encoding="utf-8"
    )

    errors = module._effective_lock_acquisition_source_fidelity_errors(formal_dir)
    assert any(expected_error in error for error in errors), errors


def test_effective_lock_acquisition_theorem_weakening_fails_closed(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_effective_lock_acquisition_fixture(tmp_path, module)
    path = formal_dir / "SumeragiV2EffectiveLockAcquisitionProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            "EffectiveLockAcquisitionModelObligation",
            "/\\ StableEffectiveLockDelivery",
            "/\\ TRUE",
        ),
        encoding="utf-8",
    )

    errors = module._effective_lock_acquisition_source_fidelity_errors(formal_dir)
    assert any(
        "must state type closure plus both temporal properties" in error
        for error in errors
    )


@pytest.mark.parametrize(
    ("name", "clause", "expected_error"),
    (
        (
            "effective_lock_acquisition.cfg",
            "PROPERTY StableEffectiveLockDelivery\n",
            "executable acquisition search must contain exactly one",
        ),
        (
            "effective_lock_rebind_bug.cfg",
            "INVARIANT ViewRebindKeepsOnePhysicalLoad\n",
            "mutation config must contain exactly one",
        ),
        (
            "effective_lock_no_retry_bug.cfg",
            "PROPERTY EffectiveLockAcquisitionProgress\n",
            "mutation config must contain exactly one",
        ),
        (
            "effective_lock_future_completion_bug.cfg",
            "INVARIANT BuggyFutureCompletionFailsClosed\n",
            "mutation config must contain exactly one",
        ),
    ),
)
def test_effective_lock_acquisition_config_weakening_fails_closed(
    tmp_path: Path,
    name: str,
    clause: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_effective_lock_acquisition_fixture(tmp_path, module)
    path = formal_dir / name
    source = path.read_text(encoding="utf-8")
    assert source.count(clause) == 1
    path.write_text(source.replace(clause, "", 1), encoding="utf-8")

    errors = module._effective_lock_acquisition_source_fidelity_errors(formal_dir)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "successor_stale_token_bug.cfg 12",
            "successor_stale_token_bug.cfg 0",
            "successor-stale-token-bug exactly once with status 12",
        ),
        (
            '"Invariant SuccessorActivationProtocolInvariantProjection is '
            'violated." \\\n'
            '  "2 states generated, 2 distinct states found, 0 states left '
            'on queue."',
            '"Invariant SuccessorActivationProtocolInvariantProjection is '
            'violated." \\\n'
            '  "3 states generated, 3 distinct states found, 0 states left '
            'on queue."',
            "successor-stale-token-bug must require exact markers",
        ),
        (
            "effective_lock_rebind_bug.cfg 12",
            "effective_lock_rebind_bug.cfg 0",
            "effective-lock-rebind-bug exactly once with status 12",
        ),
        (
            '"5 distinct states" "State 4: Stuttering"',
            '"5 distinct states"',
            "effective-lock-no-retry-bug must require exact markers",
        ),
        (
            '"Invariant BuggyFutureCompletionFailsClosed is violated by the '
            'initial state"',
            '"Invariant BuggyFutureCompletionFailsClosed is violated."',
            "effective-lock-future-completion-bug must require exact markers",
        ),
    ),
)
def test_effective_lock_mutation_runner_weakening_fails_closed(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    repo_root = tmp_path / "repo"
    runner = repo_root / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh"
    runner.parent.mkdir(parents=True)
    source = (
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh"
    ).read_text(encoding="utf-8")
    assert source.count(old) == 1
    runner.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._effective_lock_acquisition_mutation_runner_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("label", "block"),
    (
        (
            "successor-stale-token-bug",
            "run_case successor-stale-token-bug \\\n"
            "  SumeragiV2SuccessorStaleTokenMutation.tla \\\n"
            "  successor_stale_token_bug.cfg 12 \\\n"
            '  "Invariant SuccessorActivationProtocolInvariantProjection '
            'is violated." \\\n'
            '  "2 states generated, 2 distinct states found, 0 states left '
            'on queue." \\\n'
            '  "BuggyBeginSuccessorActivation"\n',
        ),
        (
            "successor-stale-token-fixed",
            "run_case successor-stale-token-fixed \\\n"
            "  SumeragiV2SuccessorStaleTokenMutation.tla \\\n"
            "  successor_stale_token_fixed.cfg 0 \\\n"
            '  "Model checking completed. No error has been found." \\\n'
            '  "2 states generated, 2 distinct states found, 0 states left '
            'on queue." \\\n'
            '  "depth of the complete state graph search is 2"\n',
        ),
    ),
)
def test_successor_stale_token_runner_requires_both_tlc_cases(
    tmp_path: Path,
    label: str,
    block: str,
) -> None:
    module = load_checker()
    repo_root = tmp_path / "repo"
    runner = repo_root / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh"
    runner.parent.mkdir(parents=True)
    source = (
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh"
    ).read_text(encoding="utf-8")
    assert source.count(block) == 1
    runner.write_text(source.replace(block, "", 1), encoding="utf-8")

    errors = module._effective_lock_acquisition_mutation_runner_errors(repo_root)

    assert any(
        f"must invoke {label} exactly once" in error for error in errors
    ), errors


def test_successor_stale_token_runner_rejects_swapped_red_green_order(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root = tmp_path / "repo"
    runner = repo_root / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh"
    runner.parent.mkdir(parents=True)
    source = (
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh"
    ).read_text(encoding="utf-8")
    bug_start = source.index("run_case successor-stale-token-bug")
    fixed_start = source.index("run_case successor-stale-token-fixed")
    next_start = source.index("run_case effective-lock-rebind-fixed")
    bug_block = source[bug_start:fixed_start]
    fixed_block = source[fixed_start:next_start]
    runner.write_text(
        source[:bug_start] + fixed_block + bug_block + source[next_start:],
        encoding="utf-8",
    )

    errors = module._effective_lock_acquisition_mutation_runner_errors(repo_root)

    assert any(
        "successor stale-token cases must keep bug-before-fixed order" in error
        for error in errors
    ), errors


def test_causal_fifo_rank_mutation_source_fidelity_fails_closed(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for name in (
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2CausalFifoRankMutation.tla",
        "causal_fifo_rank_multiplier_one_bug.cfg",
        "causal_fifo_rank_doubled.cfg",
    ):
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)

    assert module._causal_fifo_rank_mutation_source_fidelity_errors(formal_dir) == []

    mutations = (
        (
            "SumeragiV2LivenessProofs.tla",
            "  2 * CandidateSequenceIndex(\n",
            "  1 * CandidateSequenceIndex(\n",
            "CausalCandidatePosition must match the reviewed doubled-FIFO",
        ),
        (
            "SumeragiV2CausalFifoRankMutation.tla",
            '  /\\ preferredLocalSource\' = "Producer"\n',
            '  /\\ preferredLocalSource\' = "Causal"\n',
            "RemoveEarlierHead must match the reviewed causal FIFO/cursor mutation",
        ),
        (
            "SumeragiV2CausalFifoRankMutation.tla",
            "  earlierHeadRemoved => TargetRank < InitialTargetRank\n",
            "  earlierHeadRemoved => TargetRank <= InitialTargetRank\n",
            "EarlierHeadRemovalStrictlyDropsTargetRank must match",
        ),
        (
            "causal_fifo_rank_doubled.cfg",
            "CONSTANT RankMultiplier = 2\n",
            "CONSTANT RankMultiplier = 1\n",
            "must equal the exact reviewed TLC contract",
        ),
    )
    for name, old, new, expected_error in mutations:
        path = formal_dir / name
        canonical = (module.FORMAL_DIR / name).read_text(encoding="utf-8")
        assert canonical.count(old) == 1, (name, old)
        path.write_text(canonical.replace(old, new, 1), encoding="utf-8")
        errors = module._causal_fifo_rank_mutation_source_fidelity_errors(
            formal_dir
        )
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        path.write_text(canonical, encoding="utf-8")

    obligation = next(
        obligation
        for obligation in module.load_ledger()["obligations"]
        if obligation["id"] == "protected-service-rank"
    )
    assert obligation["status"] == "specified_unproved"
    assert "doubled causal FIFO position" in obligation["requirement"]


def test_causal_fifo_rank_mutation_runner_fails_closed(tmp_path: Path) -> None:
    module = load_checker()
    repo_root = tmp_path / "repo"
    runner = repo_root / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh"
    runner.parent.mkdir(parents=True)
    source = (
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh"
    ).read_text(encoding="utf-8")
    runner.write_text(source, encoding="utf-8")
    assert module._causal_fifo_rank_mutation_runner_errors(repo_root) == []

    mutations = (
        (
            "  causal_fifo_rank_multiplier_one_bug.cfg 12 \\\n",
            "  causal_fifo_rank_multiplier_one_bug.cfg 0 \\\n",
            "must invoke causal-fifo-rank-multiplier-one-bug exactly once",
        ),
        (
            '  "State 2: <RemoveEarlierHead" \\\n',
            '  "State 2: <RemoveEarlierHead line" \\\n',
            "causal-fifo-rank-multiplier-one-bug must require exact markers",
        ),
        (
            "  causal_fifo_rank_doubled.cfg 0 \\\n",
            "  causal_fifo_rank_doubled.cfg 12 \\\n",
            "must invoke causal-fifo-rank-doubled exactly once",
        ),
    )
    for old, new, expected_error in mutations:
        assert source.count(old) == 1, old
        runner.write_text(source.replace(old, new, 1), encoding="utf-8")
        errors = module._causal_fifo_rank_mutation_runner_errors(repo_root)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )

    bug_start = source.index("run_case causal-fifo-rank-multiplier-one-bug")
    fixed_start = source.index("run_case causal-fifo-rank-doubled", bug_start)
    next_start = source.index("run_case discovery-debt-bug", fixed_start)
    bug_block = source[bug_start:fixed_start]
    fixed_block = source[fixed_start:next_start]
    runner.write_text(
        source[:bug_start] + fixed_block + bug_block + source[next_start:],
        encoding="utf-8",
    )
    errors = module._causal_fifo_rank_mutation_runner_errors(repo_root)
    assert any(
        "must keep multiplier-one bug before the doubled repair" in error
        for error in errors
    ), errors


def copy_effect_capacity_mutation_fixture(tmp_path: Path, module) -> tuple[Path, Path]:
    """Copy the exact 24-file bounded effect-capacity mutation corpus."""

    repo_root = tmp_path / "repo"
    formal_dir = repo_root / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for name in module.EFFECT_CAPACITY_MUTATION_FORMAL_ARTIFACTS:
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)
    runner = repo_root / module.EFFECT_CAPACITY_MUTATION_RUNNER
    runner.parent.mkdir(parents=True)
    shutil.copy2(ROOT_DIR / module.EFFECT_CAPACITY_MUTATION_RUNNER, runner)
    return repo_root, formal_dir


def test_effect_capacity_mutation_source_seal_covers_exact_corpus(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_effect_capacity_mutation_fixture(tmp_path, module)

    assert len(module.EFFECT_CAPACITY_MUTATION_FORMAL_ARTIFACTS) == 23
    assert (
        sum(
            name.endswith(".tla")
            for name in module.EFFECT_CAPACITY_MUTATION_FORMAL_ARTIFACTS
        )
        == 4
    )
    assert (
        sum(
            name.endswith(".cfg")
            for name in module.EFFECT_CAPACITY_MUTATION_FORMAL_ARTIFACTS
        )
        == 19
    )
    assert len(module.EFFECT_CAPACITY_MUTATION_SHA256) == 24
    assert module._effect_capacity_mutation_source_fidelity_errors(
        formal_dir, repo_root
    ) == []


@pytest.mark.parametrize(
    "artifact_name",
    (
        "SumeragiV2EffectCapacityOwnershipMutation.tla",
        "effect_batch_partial_fifo_fixed.cfg",
        "scripts/formal/run_sumeragi_v2_effect_capacity_ownership_mutation.sh",
    ),
)
def test_effect_capacity_mutation_source_seal_rejects_stale_artifact(
    tmp_path: Path,
    artifact_name: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_effect_capacity_mutation_fixture(tmp_path, module)
    path = (
        repo_root / artifact_name
        if "/" in artifact_name
        else formal_dir / artifact_name
    )
    path.write_text(
        path.read_text(encoding="utf-8") + "\n\\* stale mutation\n",
        encoding="utf-8",
    )

    errors = module._effect_capacity_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(
        str(path) in error and "must match exact reviewed SHA-256" in error
        for error in errors
    ), errors


def test_effect_capacity_mutation_source_seal_rejects_missing_and_extra(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_effect_capacity_mutation_fixture(tmp_path, module)
    missing = formal_dir / "effect_capacity_timeout_sign_fixed.cfg"
    missing.unlink()
    errors = module._effect_capacity_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )
    assert any(
        str(missing) in error
        and "missing effect-capacity mutation artifact" in error
        for error in errors
    ), errors

    shutil.copy2(module.FORMAL_DIR / missing.name, missing)
    extra = formal_dir / "effect_capacity_unreviewed.cfg"
    extra.write_text("SPECIFICATION Spec\n", encoding="utf-8")
    errors = module._effect_capacity_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )
    assert any(
        str(extra) in error and "extra effect-capacity mutation artifact" in error
        for error in errors
    ), errors


def copy_post_decision_timeout_mutation_fixture(
    tmp_path: Path, module
) -> tuple[Path, Path]:
    """Copy the exact ten-file post-Decision timeout mutation corpus."""

    repo_root = tmp_path / "repo"
    formal_dir = repo_root / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for name in module.POST_DECISION_TIMEOUT_MUTATION_FORMAL_ARTIFACTS:
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)
    runner = repo_root / module.POST_DECISION_TIMEOUT_MUTATION_RUNNER
    runner.parent.mkdir(parents=True)
    shutil.copy2(ROOT_DIR / module.POST_DECISION_TIMEOUT_MUTATION_RUNNER, runner)
    return repo_root, formal_dir


def test_post_decision_timeout_mutation_source_seal_covers_exact_corpus(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_post_decision_timeout_mutation_fixture(
        tmp_path, module
    )

    assert len(module.POST_DECISION_TIMEOUT_MUTATION_FORMAL_ARTIFACTS) == 9
    assert (
        sum(
            name.endswith(".tla")
            for name in module.POST_DECISION_TIMEOUT_MUTATION_FORMAL_ARTIFACTS
        )
        == 1
    )
    assert (
        sum(
            name.endswith(".cfg")
            for name in module.POST_DECISION_TIMEOUT_MUTATION_FORMAL_ARTIFACTS
        )
        == 8
    )
    assert len(module.POST_DECISION_TIMEOUT_MUTATION_SHA256) == 10
    assert module._post_decision_timeout_mutation_source_fidelity_errors(
        formal_dir, repo_root
    ) == []


@pytest.mark.parametrize(
    "artifact_name",
    (
        "SumeragiV2PostDecisionTimeoutMutation.tla",
        "post_decision_timeout_successor_bug.cfg",
        "scripts/formal/run_sumeragi_v2_post_decision_timeout_mutation.sh",
    ),
)
def test_post_decision_timeout_mutation_source_seal_rejects_stale_artifact(
    tmp_path: Path,
    artifact_name: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_post_decision_timeout_mutation_fixture(
        tmp_path, module
    )
    path = (
        repo_root / artifact_name
        if "/" in artifact_name
        else formal_dir / artifact_name
    )
    path.write_text(
        path.read_text(encoding="utf-8") + "\n\\* stale mutation\n",
        encoding="utf-8",
    )

    errors = module._post_decision_timeout_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(
        str(path) in error and "must match exact reviewed SHA-256" in error
        for error in errors
    ), errors


def test_post_decision_timeout_mutation_source_seal_rejects_missing_extra_and_symlink(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_post_decision_timeout_mutation_fixture(
        tmp_path, module
    )
    missing = formal_dir / "post_decision_form_tc_guard_bug.cfg"
    missing.unlink()
    errors = module._post_decision_timeout_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )
    assert any(
        str(missing) in error
        and "missing post-Decision timeout mutation artifact" in error
        for error in errors
    ), errors

    shutil.copy2(module.FORMAL_DIR / missing.name, missing)
    extra = formal_dir / "post_decision_unreviewed_bug.cfg"
    extra.write_text("SPECIFICATION Spec\n", encoding="utf-8")
    errors = module._post_decision_timeout_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )
    assert any(
        str(extra) in error
        and "extra post-Decision timeout mutation artifact" in error
        for error in errors
    ), errors

    extra.unlink()
    symlink = formal_dir / "post_decision_timeout_fixed.cfg"
    target = formal_dir / "post_decision_timeout_fixed.target"
    symlink.rename(target)
    symlink.symlink_to(target.name)
    errors = module._post_decision_timeout_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )
    assert any(
        str(symlink) in error and "artifact must be a regular file" in error
        for error in errors
    ), errors


def test_successor_activation_and_exact_recovery_refinement_remains_explicit_debt() -> None:
    module = load_checker()
    ledger = module.load_ledger()
    by_id = {obligation["id"]: obligation for obligation in ledger["obligations"]}

    obligation = by_id[
        "successor-activation-exact-recovery-production-refinement"
    ]
    assert obligation["module"] == "SumeragiV2ChainEpochRefinement"
    assert (
        obligation["symbol"]
        == "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation"
    )
    assert obligation["status"] == "specified_unproved"

    source = (module.FORMAL_DIR / "SumeragiV2ChainEpochRefinement.tla").read_text()
    trace_refinement = module._top_level_operator_body(
        source, "ProductionSuccessorAndExactRecoveryTraceRefinement"
    )
    assert trace_refinement is not None
    assert " ".join(trace_refinement[0].split()) == (
        "/\\ ProductionAppliedSuccessorTraceRefinesIndexedActivation = TRUE "
        "/\\ ProductionRecoveredSuccessorTraceRefinesIndexedActivation = TRUE "
        "/\\ ProductionStartupFailureRefinesFailClosedActivation = TRUE "
        "/\\ ProductionHistoricalCertificateTraceRefinesIndexedAsync = TRUE "
        "/\\ ProductionHistoricalBodyPipelineTraceRefinesIndexedAsync = TRUE "
        "/\\ ProductionTerminalApplicationExcludesActivation = TRUE"
    )
    theorem = module._top_level_theorem_body(
        source,
        "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation",
    )
    assert theorem is not None
    theorem_parts = re.split(
        r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", theorem[0], maxsplit=1
    )
    assert len(theorem_parts) == 1
    assert " ".join(theorem_parts[0].split()) == (
        "/\\ ProductionSuccessorAndExactRecoveryTraceRefinement "
        "/\\ (IndexedChainSpec => []"
        "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant)"
    )
    assert module._chain_source_fidelity_errors(module.FORMAL_DIR) == []


def test_successor_activation_starvation_freedom_remains_explicit_debt() -> None:
    module = load_checker()
    ledger = module.load_ledger()
    by_id = {obligation["id"]: obligation for obligation in ledger["obligations"]}

    assert all("catch-up" not in obligation_id for obligation_id in by_id)
    obligation = by_id["successor-activation-starvation-freedom"]
    assert obligation["module"] == "SumeragiV2SuccessorActivationRefinementProofs"
    assert obligation["symbol"] == "SuccessorActivationStarvationFreedomObligation"
    assert obligation["status"] == "specified_unproved"

    source = (
        module.FORMAL_DIR / "SumeragiV2SuccessorActivationRefinementProofs.tla"
    ).read_text(encoding="utf-8")
    theorem = module._top_level_theorem_body(
        source, "SuccessorActivationStarvationFreedomObligation"
    )
    assert theorem is not None
    theorem_parts = re.split(
        r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", theorem[0], maxsplit=1
    )
    statement = " ".join(theorem_parts[0].split())
    assert statement == (
        "IndexedChainSpec "
        "=> /\\ SuccessorActivationPendingStructureProperty "
        "/\\ SuccessorActivationStepDecreasesRankProperty "
        "/\\ SuccessorActivationPendingIsNotOrphanedProperty "
        "/\\ SuccessorActivationOutcomeIsStableProperty "
        "/\\ SuccessorActivationRankProgressProperty "
        "/\\ SuccessorActivationStarvationFreedomProperty"
    )
    assert len(theorem_parts) == 2
    proof = " ".join(theorem_parts[1].split())
    for dependency in (
        "IndexedChainSpecEstablishesSuccessorActivationPendingStructure",
        "IndexedChainSpecEstablishesSuccessorActivationStepDecrease",
        "IndexedChainSpecEstablishesSuccessorActivationNonOrphaning",
        "IndexedChainSpecEstablishesSuccessorActivationOutcomeStability",
        "IndexedChainSpecEstablishesSuccessorActivationRankProgress",
        "IndexedChainSpecEstablishesSuccessorActivationStarvationFreedom",
    ):
        assert proof.count(dependency) == 1
    assert (
        module._successor_activation_rank_source_fidelity_errors(module.FORMAL_DIR)
        == []
    )


@pytest.mark.parametrize(
    ("relative_path", "region_marker", "old", "new", "error_fragment"),
    (
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn open_ingress_for_active_height(",
            "activation.publish(successor)",
            "drop(activation); let _ = successor",
            "open_ingress_for_active_height must preserve exact production order",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "fn activate_recovered_v2_successor_height_at(",
            "set_v2_status_at(successor, now);",
            "update_v2_successor_work_stage_at(finalized_height, SumeragiV2LocalWorkStage::Running, SumeragiV2LocalWorkStage::Complete, now)?; set_v2_status_at(successor, now);",
            "may not fabricate physical predecessor completion",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_block_sync.rs",
            "fn build_historical_body_response(",
            "binary_search(&responder)",
            "contains(&responder)",
            "build_historical_body_response must preserve exact production order",
        ),
        (
            "scripts/run_sumeragi_v2_release_gates.sh",
            "required_production_liveness_tests=(",
            "sumeragi::v2_block_sync::tests::catch_up_is_strictly_sequential_across_contexts",
            "sumeragi::v2_block_sync::tests::catch_up_is_not_release_bound",
            "production refinement test must be pinned exactly once",
        ),
    ),
)
def test_successor_production_source_mapping_mutations_fail_closed(
    tmp_path: Path,
    relative_path: str,
    region_marker: str,
    old: str,
    new: str,
    error_fragment: str,
) -> None:
    module = load_checker()
    required_sources = (
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "crates/iroha_core/src/sumeragi/status.rs",
        "crates/iroha_core/src/sumeragi/v2.rs",
        "crates/iroha_core/src/sumeragi/v2_runtime.rs",
        "crates/iroha_core/src/sumeragi/v2_block_sync.rs",
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "scripts/run_sumeragi_v2_release_gates.sh",
    )
    for source_name in required_sources:
        destination = tmp_path / source_name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / source_name, destination)

    path = tmp_path / relative_path
    source = path.read_text(encoding="utf-8")
    region_start = source.find(region_marker)
    assert region_start >= 0
    mutation = source.find(old, region_start)
    assert mutation >= 0
    path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )

    errors = module._successor_production_source_fidelity_errors(tmp_path)
    assert any(error_fragment in error for error in errors), errors


def test_proofless_release_theorems_require_exact_explicit_debt() -> None:
    module = load_checker()
    source = r"""---- MODULE SumeragiV2AsyncLivenessProofs ----
THEOREM Complete == TRUE
BY PTL
THEOREM Pending == TRUE
=============================================================================
"""
    sources = {"SumeragiV2AsyncLivenessProofs": source}
    obligations = [
        {
            "id": "pending",
            "module": "SumeragiV2AsyncLivenessProofs",
            "symbol": "Pending",
            "status": "specified_unproved",
        }
    ]

    assert module._proofless_release_theorem_errors(obligations, sources) == []

    unledgered = copy.deepcopy(obligations)
    unledgered[0]["symbol"] = "Pending / Complete"
    errors = module._proofless_release_theorem_errors(unledgered, sources)
    assert any("must have exactly one ledger entry" in error for error in errors)

    falsely_proved = copy.deepcopy(obligations)
    falsely_proved[0]["status"] = "tlaps_proved"
    errors = module._proofless_release_theorem_errors(falsely_proved, sources)
    assert any("must be ledgered specified_unproved" in error for error in errors)

    indented = r"""---- MODULE SumeragiV2AsyncLivenessProofs ----
  THEOREM IndentedPending == TRUE
=============================================================================
"""
    indented_obligation = [
        {
            "id": "indented-pending",
            "module": "SumeragiV2AsyncLivenessProofs",
            "symbol": "IndentedPending",
            "status": "specified_unproved",
        }
    ]
    assert (
        module._proofless_release_theorem_errors(
            indented_obligation, {"SumeragiV2AsyncLivenessProofs": indented}
        )
        == []
    )
    errors = module._proofless_release_theorem_errors(
        [], {"SumeragiV2AsyncLivenessProofs": indented}
    )
    assert any(
        "IndentedPending must have exactly one ledger entry" in error
        for error in errors
    )

    local = r"""---- MODULE SumeragiV2AsyncLivenessProofs ----
LOCAL THEOREM LocalPending == TRUE
=============================================================================
"""
    errors = module._proofless_release_theorem_errors(
        [], {"SumeragiV2AsyncLivenessProofs": local}
    )
    assert any(
        "LocalPending must have exactly one ledger entry" in error
        for error in errors
    )


def test_release_obligations_are_bound_to_direct_production_specs() -> None:
    module = load_checker()

    safety_source = "---- MODULE SumeragiV2Proofs ----\n" + "\n".join(
        f"THEOREM {symbol} ==\n"
        f"  \\A initialContext: "
        f"{module.ARBITRARY_CONTEXT_SAFETY_PROPERTY_WRAPPERS[obligation_id]}"
        "(CoreSpecAt(initialContext))\n"
        "BY PTL"
        for obligation_id, symbol in module.ARBITRARY_CONTEXT_SAFETY_OBLIGATIONS.items()
    )
    liveness_source = (
        "---- MODULE SumeragiV2AsyncLivenessProofs ----\n"
        + "\n".join(
            f"THEOREM {symbol} ==\n"
            f"  \\A initialContext: "
            f"{module.ASYNC_LIVENESS_PROPERTY_WRAPPERS[obligation_id]}"
            "(AsyncSpecAt(initialContext))\n"
            "BY PTL"
            for obligation_id, symbol in module.ASYNC_LIVENESS_OBLIGATIONS.items()
        )
    )
    chain_source = (
        "---- MODULE SumeragiV2ChainEpochProofs ----\n"
        + "\n".join(
            f"THEOREM {symbol} ==\n"
            f"  {property_wrapper}(ChainEpochSpec)\n"
            "BY PTL"
            for symbol, property_wrapper in module.CHAIN_SAFETY_OBLIGATIONS.values()
        )
    )
    obligations = [
        {
            "id": obligation_id,
            "module": "SumeragiV2Proofs",
            "symbol": symbol,
        }
        for obligation_id, symbol in module.ARBITRARY_CONTEXT_SAFETY_OBLIGATIONS.items()
    ] + [
        {
            "id": obligation_id,
            "module": "SumeragiV2AsyncLivenessProofs",
            "symbol": symbol,
        }
        for obligation_id, symbol in module.ASYNC_LIVENESS_OBLIGATIONS.items()
    ] + [
        {
            "id": obligation_id,
            "module": "SumeragiV2ChainEpochProofs",
            "symbol": symbol,
        }
        for obligation_id, (symbol, _) in module.CHAIN_SAFETY_OBLIGATIONS.items()
    ] + [
        {
            "id": obligation_id,
            "module": target[0],
            "symbol": target[1],
        }
        for obligation_id, target in module.FIXED_PROOF_OBLIGATION_TARGETS.items()
    ]
    sources = {
        "SumeragiV2Proofs": safety_source,
        "SumeragiV2AsyncLivenessProofs": liveness_source,
        "SumeragiV2ChainEpochProofs": chain_source,
    }

    assert not module._proof_obligation_architecture_errors(obligations, sources)

    invalid_parameterized_sources = dict(sources)
    first_safety_symbol = next(iter(module.ARBITRARY_CONTEXT_SAFETY_OBLIGATIONS.values()))
    invalid_parameterized_sources["SumeragiV2Proofs"] = safety_source.replace(
        f"THEOREM {first_safety_symbol} ==",
        f"THEOREM {first_safety_symbol}(initialContext) ==",
        1,
    )
    errors = module._proof_obligation_architecture_errors(
        obligations, invalid_parameterized_sources
    )
    assert any(
        "must be one closed theorem universally quantifying initialContext" in error
        for error in errors
    )

    wrong_module = copy.deepcopy(obligations)
    application = next(
        obligation
        for obligation in wrong_module
        if obligation["id"] == "application-liveness"
    )
    application["module"] = "SumeragiV2Proofs"
    errors = module._proof_obligation_architecture_errors(wrong_module, sources)
    assert any(
        "application-liveness must use SumeragiV2AsyncLivenessProofs" in error
        for error in errors
    )

    legacy_sources = dict(sources)
    legacy_sources["SumeragiV2Proofs"] = safety_source.replace(
        "CoreSpecAt(initialContext)", "Spec", 1
    )
    errors = module._proof_obligation_architecture_errors(obligations, legacy_sources)
    assert any("must directly require CoreSpecAt(initialContext)" in error for error in errors)
    assert any("legacy global-barrier operator Spec" in error for error in errors)

    vacuous_safety_sources = dict(sources)
    vacuous_safety_sources["SumeragiV2Proofs"] = safety_source.replace(
        "DurableVoteUniquenessProperty(CoreSpecAt(initialContext))",
        "FALSE /\\ DurableVoteUniquenessProperty(CoreSpecAt(initialContext))",
        1,
    )
    errors = module._proof_obligation_architecture_errors(
        obligations, vacuous_safety_sources
    )
    assert any(
        "DurableVoteUniquenessObligation must state only" in error for error in errors
    )

    vacuous_sources = dict(sources)
    vacuous_sources["SumeragiV2AsyncLivenessProofs"] = liveness_source.replace(
        "TimeoutViewProgressProperty(AsyncSpecAt(initialContext))",
        "FALSE /\\ TimeoutViewProgressProperty(AsyncSpecAt(initialContext))",
        1,
    )
    errors = module._proof_obligation_architecture_errors(obligations, vacuous_sources)
    assert any(
        "TimeoutViewProgressObligation must state only" in error for error in errors
    )

    candidate_only_rank_sources = dict(sources)
    candidate_only_rank_sources["SumeragiV2AsyncLivenessProofs"] = (
        liveness_source.replace(
            "ProtectedServiceRanksProgressProperty(AsyncSpecAt(initialContext))",
            "ProtectedServiceRankProgressProperty(AsyncSpecAt(initialContext))",
            1,
        )
    )
    errors = module._proof_obligation_architecture_errors(
        obligations, candidate_only_rank_sources
    )
    assert any(
        "ProtectedServiceRankProgressObligation must state only" in error
        and "ProtectedServiceRanksProgressProperty" in error
        for error in errors
    )

    global_barrier_sources = dict(sources)
    global_barrier_sources["SumeragiV2ChainEpochProofs"] = chain_source.replace(
        "ChainPrefixProperty(ChainEpochSpec)",
        "ChainPrefixProperty(Spec)",
        1,
    )
    errors = module._proof_obligation_architecture_errors(
        obligations, global_barrier_sources
    )
    assert any("ChainPrefixObligation must state only" in error for error in errors)
    assert any("legacy global-barrier operator Spec" in error for error in errors)


def test_protected_service_rank_contract_cannot_drop_serve_fifo_rank(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "proof_coverage.json",
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
    )
    path = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(
            source,
            "ProtectedServiceRanksProgressProperty",
            "/\\ ProtectedServeRankProgressProperty(specification)",
            "/\\ TRUE",
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)

    assert any(
        "ProtectedServiceRanksProgressProperty must equal only" in error
        for error in errors
    ), errors


def test_async_release_requires_checked_type_closure_and_step_refinement(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    valid = r"""---- MODULE SumeragiV2AsyncLivenessProofs ----
THEOREM AsyncStepRefinementObligation ==
  AsyncNext => [Next]_vars
BY DEF AsyncNext
THEOREM AsyncTypeInvariantObligation ==
  \A initialContext: AsyncSpecAt(initialContext) => []AsyncTypeInvariant
BY PTL
THEOREM AsyncNextPreservesNormalProposalPrepareCandidate ==
  \A candidate:
    /\ NormalProposalPrepareCandidate(candidate)
    /\ AsyncNext
    => NormalProposalPrepareCandidate(candidate)'
BY PTL
=============================================================================
"""
    path.write_text(valid, encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    path.write_text(
        valid.replace(
            "\\A initialContext: AsyncSpecAt(initialContext) => []AsyncTypeInvariant",
            "AsyncTypeInvariant => []AsyncTypeInvariant",
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "AsyncTypeInvariantObligation must state only" in error for error in errors
    )


def test_arbitrary_context_safety_property_bodies_are_pinned(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    path = formal_dir / "SumeragiV2Proofs.tla"
    source = r"""---- MODULE SumeragiV2Proofs ----
DurableVoteUniquenessProperty(specification) ==
  specification => [](/\ HonestPrepareUniqueness
                       /\ HonestCommitUniqueness
                       /\ HonestTimeoutUniqueness)
LockMonotonicityProperty(specification) ==
  specification => [][LockMonotonicityAction]_vars
ExternalValidityProperty(specification) ==
  specification => [](/\ \A qc \in prepareQCs: qc.subject \in ValidSubjects
                       /\ \A qc \in commitQCs: qc.subject \in ValidSubjects
                       /\ \A decision \in decisions:
                            decision.qc.subject \in ValidSubjects)
CertifiedBodyAvailabilityProperty(specification) ==
  specification => [](/\ PrepareCertificateAvailability
                       /\ CommitCertificateAvailability)
CertificateUniquenessProperty(specification) ==
  specification => []CertificateUniquenessInvariant
PotentialCommitVotes(certificateContext, roundView, subject) ==
  {vote \in commitIntents:
    /\ vote.context = certificateContext
    /\ vote.view = roundView
    /\ vote.phase = "Commit"
    /\ vote.subject = subject}
PotentialCommitSigners(certificateContext, roundView, subject) ==
  {vote.signer:
    vote \in PotentialCommitVotes(
      certificateContext, roundView, subject)}
InstalledTcAuthorizedPotentialCommitIntersection(tc, protectedView, subject) ==
  \E timeoutVote \in tc.votes,
      commitVote \in PotentialCommitVotes(
        tc.context, protectedView, subject):
    /\ timeoutVote.signer \in Honest
    /\ commitVote.signer = timeoutVote.signer
    /\ timeoutVote.context = tc.context
    /\ timeoutVote.view = tc.view
    /\ ~TimeoutVoteStrictlyProtectsCommit(timeoutVote, commitVote)
    /\ InstalledTcAuthorizesCommitVote(commitVote)
TCProtectsOrInstalledTcAuthorizesPotentialCommit(tc) ==
  \A protectedView \in 0..tc.view, subject \in Subjects:
    DualQuorum(tc.context.epoch,
      PotentialCommitSigners(tc.context, protectedView, subject))
      => \/ TCProtectsViewSubject(tc, protectedView, subject)
         \/ InstalledTcAuthorizedPotentialCommitIntersection(
              tc, protectedView, subject)
TimeoutProtectionProperty(specification) ==
  specification
    => [](\A tc \in formedTCs:
          TCProtectsOrInstalledTcAuthorizesPotentialCommit(tc))
AgreementProperty(specification) ==
  specification => []DecisionAgreement
NoConflictingCommitCertificatesProperty(specification) ==
  specification => [](\A left, right \in commitQCs:
    left.context = right.context => left.subject = right.subject)
CrashRecoveryProperty(specification) ==
  /\ (specification => []CrashRecoveryStateInvariant)
  /\ (specification => [][CrashPreservesDurableProjection]_vars)
  /\ (specification => [][RestartPreservesDurableProjection]_vars)
  /\ (specification => [][PendingWritesAreUnacknowledged]_vars)
  /\ (specification =>
        [][TypeInvariant => StaleGenerationRejected]_vars)
=============================================================================
"""
    path.write_text(source, encoding="utf-8")

    assert module._safety_property_source_fidelity_errors(formal_dir) == []

    path.write_text(
        source.replace("/\\ HonestTimeoutUniqueness", "/\\ TRUE")
        .replace(
            "/\\ (specification => [][RestartPreservesDurableProjection]_vars)",
            "/\\ TRUE",
        ),
        encoding="utf-8",
    )
    errors = module._safety_property_source_fidelity_errors(formal_dir)
    assert any("DurableVoteUniquenessProperty must equal only" in error for error in errors)
    assert any("CrashRecoveryProperty must equal only" in error for error in errors)

    path.write_text(
        source.replace(
            "/\\ InstalledTcAuthorizesCommitVote(commitVote)",
            "/\\ TRUE",
        ),
        encoding="utf-8",
    )
    errors = module._safety_property_source_fidelity_errors(formal_dir)
    assert any(
        "InstalledTcAuthorizedPotentialCommitIntersection must equal only" in error
        for error in errors
    )

    path.write_text(
        source.replace(
            "\\/ InstalledTcAuthorizedPotentialCommitIntersection(\n"
            "              tc, protectedView, subject)",
            "\\/ TCProtectsViewSubject(tc, protectedView, subject)",
        ),
        encoding="utf-8",
    )
    errors = module._safety_property_source_fidelity_errors(formal_dir)
    assert any(
        "TCProtectsOrInstalledTcAuthorizesPotentialCommit must equal only" in error
        for error in errors
    )


def test_liveness_property_contracts_are_semantically_pinned(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "SumeragiV2AsyncLivenessProofs.tla").write_text(
        r"""---- MODULE SumeragiV2AsyncLivenessProofs ----
THEOREM AsyncStepRefinementObligation ==
  AsyncNext => [Next]_vars
BY DEF AsyncNext
THEOREM AsyncTypeInvariantObligation ==
  \A initialContext: AsyncSpecAt(initialContext) => []AsyncTypeInvariant
BY PTL
THEOREM AsyncNextPreservesNormalProposalPrepareCandidate ==
  \A candidate:
    /\ NormalProposalPrepareCandidate(candidate)
    /\ AsyncNext
    => NormalProposalPrepareCandidate(candidate)'
BY PTL
=============================================================================
""",
        encoding="utf-8",
    )
    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    valid = r"""---- MODULE SumeragiV2LivenessProofs ----
ResponsiveNodesDecide ==
  \A node \in AsyncCurrentResponsiveVoters: NodeHasDecision(node)
ResponsiveNodesApply ==
  \A node \in AsyncCurrentResponsiveVoters: NodeHasApplication(node)
ResponsiveHonestLeaderViewReached ==
  \E leader \in (AsyncCurrentResponsiveVoters \cap Honest):
    /\ ~NodeHasDecision(leader)
    /\ Leader(context, nodeView[leader]) = leader
TimeoutViewProgressProperty(specification) ==
  specification => \A node \in AsyncCurrentResponsiveVoters,
    roundView \in Views:
      (gst /\ nodeView[node] = roundView /\ ~NodeHasDecision(node))
        ~> (nodeView[node] > roundView \/ NodeHasDecision(node))
RotatingLeaderProgressProperty(specification) ==
  specification
    => /\ (gst /\ ~ResponsiveNodesDecide)
             ~> (ResponsiveHonestLeaderViewReached
                   \/ ResponsiveNodesDecide)
       /\ (gst /\ ResponsiveHonestLeaderViewReached
                 /\ ~ResponsiveNodesDecide)
             ~> ResponsiveNodesDecide
ApplicationCompletionProgressProperty(specification) ==
  specification
    => \A node \in AsyncCurrentResponsiveVoters:
         (gst /\ NodeHasDecision(node))
           ~> NodeHasApplication(node)
ApplicationLivenessProperty(specification) ==
  specification
    => /\ \A node \in AsyncCurrentResponsiveVoters:
             (gst /\ NodeHasDecision(node))
               ~> NodeHasApplication(node)
       /\ (gst /\ ResponsiveNodesDecide) ~> ResponsiveNodesApply
PostGstProgressActionEnabled ==
  \E node \in AsyncCurrentResponsiveVoters:
    PostGstCommitCertificateDiscovery(node)
=============================================================================
"""
    vocabulary.write_text(valid, encoding="utf-8")
    (formal_dir / "SumeragiV2Proofs.tla").write_text(
        "---- MODULE SumeragiV2Proofs ----\n=============================================================================\n",
        encoding="utf-8",
    )

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary.write_text(
        valid.replace(
            "(gst /\\ NodeHasDecision(node))",
            "(FALSE /\\ gst /\\ NodeHasDecision(node))",
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any("ApplicationLivenessProperty must equal only" in error for error in errors)

    vocabulary.write_text(
        valid.replace(
            "ApplicationCompletionProgressProperty(specification) ==\n"
            "  specification\n"
            "    => \\A node \\in AsyncCurrentResponsiveVoters:\n",
            "ApplicationCompletionProgressProperty(specification) ==\n"
            "  specification\n"
            "    => \\A node \\in ValidatorIds:\n",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ApplicationCompletionProgressProperty must equal only" in error
        for error in errors
    )

    vocabulary.write_text(
        valid.replace(
            "Leader(context, nodeView[leader]) = leader",
            "Leader(context, nodeView[leader]) \\in AsyncCurrentResponsiveVoters",
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ResponsiveHonestLeaderViewReached must equal only" in error
        for error in errors
    )

    vocabulary.write_text(
        valid.replace(
            "(gst /\\ ResponsiveHonestLeaderViewReached\n"
            "                 /\\ ~ResponsiveNodesDecide)",
            "(gst /\\ ~ResponsiveNodesDecide)",
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "RotatingLeaderProgressProperty must equal only" in error
        for error in errors
    )

    vocabulary.write_text(valid, encoding="utf-8")
    (formal_dir / "SumeragiV2Proofs.tla").write_text(
        "---- MODULE SumeragiV2Proofs ----\n"
        "NodeHasDecision(node) == TRUE\n"
        "HeightLivenessProperty(specification) == specification\n"
        "=============================================================================\n",
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any("asynchronous liveness symbol NodeHasDecision" in error for error in errors)
    assert any(
        "asynchronous liveness symbol HeightLivenessProperty" in error
        for error in errors
    )

    fidelity_dir = tmp_path / "application-fidelity"
    fidelity_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, fidelity_dir / filename)
    assert module._application_completion_source_fidelity_errors(fidelity_dir) == []

    proof_path = fidelity_dir / "SumeragiV2AsyncLivenessProofs.tla"
    proof_source = proof_path.read_text(encoding="utf-8")
    proof_path.write_text(
        proof_source.replace(
            "         ApplicationCompletionReachesEveryResponsivePrefix\n",
            "         ApplicationCompletionProgressAppliesFixedResponsiveNode\n",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._application_completion_source_fidelity_errors(fidelity_dir)
    assert any(
        "proof must compose the reviewed application dependencies in order"
        in error
        for error in errors
    )

    proof_path.write_text(
        mutate_tla_theorem(
            proof_source,
            "ApplicationLivenessObligation",
            "PROOF\n",
            "OBVIOUS\n",
        ),
        encoding="utf-8",
    )
    errors = module._application_completion_source_fidelity_errors(fidelity_dir)
    assert any(
        "must have the reviewed deductive application-completion proof" in error
        for error in errors
    )


def test_scheduler_rank_derivation_cannot_widen_the_owned_carrier(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = (module.FORMAL_DIR / path.name).read_text(encoding="utf-8")
    path.write_text(source, encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    path.write_text(
        source.replace(
            "OwnedServiceRankCarrier",
            "ServiceRankCarrier",
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ScheduledCandidateServiceRankInCarrier must use "
        "OwnedServiceRankCarrier" in error
        for error in errors
    )
    assert any(
        "may not widen scheduler-owned rank proofs" in error for error in errors
    )


def test_liveness_service_ownership_stays_on_the_fair_node_domain(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Proofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    guarded_owner = (
        "ResponsiveProtectedCandidateOwned(candidate) ==\n"
        "  /\\ candidate.node \\in AsyncCurrentResponsiveVoters\n"
        "  /\\ ProtectedCandidateOwned(candidate)"
    )
    assert guarded_owner in source
    vocabulary.write_text(
        source.replace(
            guarded_owner,
            "ResponsiveProtectedCandidateOwned(candidate) ==\n"
            "  ProtectedCandidateOwned(candidate)",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ResponsiveProtectedCandidateOwned must equal only" in error
        for error in errors
    )


def test_protected_service_rank_excludes_transport_and_ingress_stages(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Proofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    assert "stage \\in 2..6, position \\in Nat:" in source
    property_offset = source.index(
        "ProtectedServiceRankProgressProperty(specification) =="
    )
    stage_offset = source.index(
        "stage \\in 2..6, position \\in Nat:", property_offset
    )
    vocabulary.write_text(
        source[:stage_offset]
        + source[stage_offset:].replace(
            "stage \\in 2..6, position \\in Nat:",
            "stage \\in 0..8, position \\in Nat:",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ProtectedServiceRankProgressProperty must equal only" in error
        for error in errors
    )

    vocabulary.write_text(
        source.replace(
            "                           ELSE <<0, 0>>",
            "                           ELSE IF CandidateInIngress(candidate)\n"
            "                                THEN <<7, 1>>\n"
            "                                ELSE <<0, 0>>",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "CandidateServiceRank must be scheduler-owned stages 2..6" in error
        for error in errors
    )


def test_liveness_configuration_typing_premises_are_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Proofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    typed_budget_premise = (
        "THEOREM RetransmissionBudgetCoversEveryClass ==\n"
        "  ModelConfiguration /\\ AsyncConfiguration"
    )
    assert typed_budget_premise in source
    vocabulary.write_text(
        source.replace(
            typed_budget_premise,
            "THEOREM RetransmissionBudgetCoversEveryClass ==\n"
            "  AsyncConfiguration",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "RetransmissionBudgetCoversEveryClass must state only" in error
        for error in errors
    )

    typed_successor_premise = (
        "THEOREM CanonicalSuccessorPreservesAdmissibility ==\n"
        "  ModelConfiguration\n"
        "    => \\A initialContext"
    )
    assert typed_successor_premise in source
    vocabulary.write_text(
        source.replace(
            typed_successor_premise,
            "THEOREM CanonicalSuccessorPreservesAdmissibility ==\n"
            "  \\A initialContext",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "CanonicalSuccessorPreservesAdmissibility must state only" in error
        for error in errors
    )


def test_verus_runner_records_output_without_masking_failures() -> None:
    source = (ROOT_DIR / "scripts" / "verify_sumeragi_v2.sh").read_text(
        encoding="utf-8"
    )

    assert "set -euo pipefail" in source
    assert "target/formal/sumeragi_v2/verus.log" in source
    assert '2>&1 | tee -a "$verus_log_tmp"' in source
    assert 'verus_pipeline_status=("${PIPESTATUS[@]}")' in source
    assert "sumeragi_v2_verus_evidence.py" in source


def test_tla_shortcut_scan_rejects_unchecked_constructs_but_allows_proof_assume(
    tmp_path: Path,
) -> None:
    module = load_checker()
    path = tmp_path / "Example.tla"
    source = r"""---- MODULE Example ----
ASSUME Unsafe
AXIOM Hidden
THEOREM Broken == TRUE BY OMITTED
THEOREM StructuredStatement ==
  ASSUME NEW value \in BOOLEAN
  PROVE value \/ ~value
BY PTL
THEOREM Structured == TRUE
PROOF
  <1>1. ASSUME TRUE
         PROVE TRUE
    OBVIOUS
  <1> QED BY <1>1
=============================================================================
"""

    errors = module.tla_shortcut_errors(path, source)
    assert len(errors) == 3
    assert any("top-level ASSUME" in error for error in errors)
    assert any("top-level AXIOM" in error for error in errors)
    assert any("OMITTED proof" in error for error in errors)


def test_tla_shortcut_scan_does_not_let_unproved_or_misplaced_assumptions_hide(
    tmp_path: Path,
) -> None:
    module = load_checker()
    path = tmp_path / "Adversarial.tla"
    source = r"""---- MODULE Adversarial ----
THEOREM UnprovedStructured ==
  ASSUME NEW value \in BOOLEAN
  PROVE value \/ ~value

THEOREM StatementEnded == TRUE
ASSUME SmuggledBetweenStatementAndProof
BY DEF StatementEnded

ASSUMPTION ModuleLevel
=============================================================================
"""

    errors = module.tla_shortcut_errors(path, source)
    assert len(errors) == 3
    assert any("UnprovedStructured" not in error and ":3:" in error for error in errors)
    assert any(":7:" in error and "ASSUME" in error for error in errors)
    assert any(":10:" in error and "ASSUMPTION" in error for error in errors)


def test_tla_shortcut_scan_ignores_comments_and_nested_comments(tmp_path: Path) -> None:
    module = load_checker()
    path = tmp_path / "Example.tla"
    source = """---- MODULE Example ----
(* ASSUME CommentOnly (* AXIOM Nested *) OMITTED *)
\\* AXIOM line comment
Safe == "OMITTED"
=============================================================================
"""

    assert module.tla_shortcut_errors(path, source) == []


def test_retired_favourable_network_liveness_corridor_is_rejected(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "Example.tla").write_text(
        "---- MODULE Example ----\n"
        "ReliableNext == TRUE\n"
        "StableProgressContracts == TRUE\n"
        "\\* ReliableBeginTimeout in a comment is harmless\n"
        "=============================================================================\n",
        encoding="utf-8",
    )

    errors = module._retired_liveness_errors(formal_dir)
    assert len(errors) == 2
    assert any("ReliableNext" in error for error in errors)
    assert any("StableProgressContracts" in error for error in errors)


def test_deductive_max_view_dependency_is_rejected(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "SumeragiV2Core.tla").write_text(
        "---- MODULE SumeragiV2Core ----\n"
        "CONSTANTS\n"
        "  MaxView,\n"
        "  ViewDomain\n"
        "FiniteViews == 0..MaxView\n"
        "=============================================================================\n",
        encoding="utf-8",
    )
    (formal_dir / "Proof.tla").write_text(
        "---- MODULE Proof ----\n"
        "THEOREM BadBound == tc.view < MaxView\n"
        "=============================================================================\n",
        encoding="utf-8",
    )

    errors = module._bounded_view_dependency_errors(formal_dir)
    assert len(errors) == 1
    assert "Proof.tla:2" in errors[0]
    assert "reserved for FiniteViews/TLC scaffolding" in errors[0]


def test_reachable_core_actions_cannot_assume_proof_history_oracles(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "SumeragiV2Core.tla").write_text(
        "---- MODULE SumeragiV2Core ----\n"
        "FormPrepareQC(node, view, subject) ==\n"
        "  /\\ CertificateHonestIntentBacked(qc, prepareIntents)\n"
        "  /\\ TRUE\n"
        "FormCommitQC(node, view, subject) == TRUE\n"
        "DeliverQC(envelope) == QcValid(envelope.qc)\n"
        "BeginTimeout(node) == HighRefValid(highRank[node], highSubject[node])\n"
        "=============================================================================\n",
        encoding="utf-8",
    )

    errors = module._reachable_oracle_guard_errors(formal_dir)
    assert len(errors) == 3
    assert any(
        "FormPrepareQC" in error and "CertificateHonestIntentBacked" in error
        for error in errors
    )
    assert any("DeliverQC" in error and "QcValid" in error for error in errors)
    assert any(
        "BeginTimeout" in error and "HighRefValid" in error for error in errors
    )


def test_reachable_core_actions_allow_wire_and_local_durable_guards(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "SumeragiV2Core.tla").write_text(
        "---- MODULE SumeragiV2Core ----\n"
        "FormPrepareQC(node, view, subject) == QcWireValid(qc)\n"
        "FormCommitQC(node, view, subject) == QcWireValid(qc)\n"
        "DeliverQC(envelope) == QcWireValid(envelope.qc)\n"
        "BeginTimeout(node) == LocalTimeoutVoteFor(node).highRank = highestRank[node]\n"
        "=============================================================================\n",
        encoding="utf-8",
    )

    assert module._reachable_oracle_guard_errors(formal_dir) == []


def test_async_deductive_and_finite_specs_cannot_be_conflated(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "SumeragiV2Core.tla").write_text(
        "---- MODULE SumeragiV2Core ----\n"
        "vars == <<coreState>>\n"
        "=============================================================================\n",
        encoding="utf-8",
    )
    canonical = """---- MODULE SumeragiV2AsyncNetwork ----
AsyncSchedulerVars == <<schedulerState>>
AsyncRecoveryVars == <<recoveryPhase, recoveryQueue>>
AsyncAllVars == <<vars, AsyncSchedulerVars, AsyncRecoveryVars>>
AsyncFairnessAt(initialContext) == WF_AsyncAllVars(AsyncNext)
AsyncFairness == AsyncFairnessAt(ContextRecord(0, <<>>))
AsyncBaseInitAt(initialContext) == TRUE
AsyncBaseInit == AsyncBaseInitAt(ContextRecord(0, <<>>))
AsyncInitAt(initialContext) == AsyncBaseInitAt(initialContext) /\\ ViewDomain = Nat
AsyncInit == AsyncInitAt(ContextRecord(0, <<>>))
AsyncFiniteInitAt(initialContext) == AsyncBaseInitAt(initialContext) /\\ ViewDomain = FiniteViews
AsyncFiniteInit == AsyncFiniteInitAt(ContextRecord(0, <<>>))
AsyncSpec == AsyncInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairness
AsyncSpecAt(initialContext) == AsyncInitAt(initialContext) /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairnessAt(initialContext)
AsyncFiniteSpec == AsyncFiniteInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairness
AsyncFiniteSpecAt(initialContext) == AsyncFiniteInitAt(initialContext) /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairnessAt(initialContext)
=============================================================================
"""
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(canonical, encoding="utf-8")

    assert module._async_spec_shape_errors(formal_dir) == []

    path.write_text(
        canonical.replace(
            "AsyncSpec == AsyncInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairness",
            "AsyncSpec == AsyncFiniteInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairness",
        ),
        encoding="utf-8",
    )
    errors = module._async_spec_shape_errors(formal_dir)
    assert any("AsyncSpec must equal only" in error for error in errors)


@pytest.mark.parametrize(
    ("symbol", "old", "new", "expected_error"),
    (
        (
            "AsyncAllVars",
            "<<vars, AsyncSchedulerVars, AsyncRecoveryVars>>",
            "<<vars, AsyncSchedulerVars>>",
            "AsyncAllVars must equal only",
        ),
        (
            "AsyncAllVars",
            "<<vars, AsyncSchedulerVars, AsyncRecoveryVars>>",
            "<<vars, AsyncRecoveryVars, AsyncSchedulerVars>>",
            "AsyncAllVars must equal only",
        ),
        (
            "AsyncAllVars",
            "<<vars, AsyncSchedulerVars, AsyncRecoveryVars>>",
            "<<coreState, schedulerState, recoveryPhase, recoveryQueue>>",
            "AsyncAllVars must equal only",
        ),
        (
            "AsyncFiniteSpec",
            "AsyncFiniteInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairness",
            "AsyncFiniteInit /\\ [][AsyncNext]_AsyncTlcAllVars /\\ AsyncFairness",
            "AsyncFiniteSpec must equal only",
        ),
        (
            "AsyncFiniteSpec",
            "AsyncFiniteInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairness",
            "AsyncFiniteInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncTlcFairness",
            "AsyncFiniteSpec must equal only",
        ),
        (
            "AsyncFiniteSpecAt",
            "/\\ AsyncFairnessAt(initialContext)",
            "/\\ AsyncFairness",
            "AsyncFiniteSpecAt must equal only",
        ),
        (
            "AsyncFairnessAt",
            "WF_AsyncAllVars(AsyncSetGST)",
            "WF_AsyncTlcAllVars(AsyncSetGST)",
            "AsyncFairnessAt may use only the public AsyncAllVars subscript",
        ),
        (
            "AsyncSpec",
            "AsyncInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairness",
            "AsyncInit /\\ [][AsyncNext]_AsyncTlcAllVars /\\ AsyncFairness",
            "AsyncSpec must equal only",
        ),
    ),
)
def test_async_canonical_spec_surface_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for name in ("SumeragiV2Core.tla", "SumeragiV2AsyncNetwork.tla"):
        shutil.copyfile(module.FORMAL_DIR / name, formal_dir / name)
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._async_spec_shape_errors(formal_dir)

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    "duplicate",
    (
        "AsyncTlcAllVars == AsyncAllVars",
        "AsyncTlcFairnessAt(initialContext) == AsyncFairnessAt(initialContext)",
        "AsyncTlcFairness == AsyncFairness",
    ),
)
def test_async_tlc_only_duplicate_aliases_are_prohibited(
    tmp_path: Path,
    duplicate: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for name in ("SumeragiV2Core.tla", "SumeragiV2AsyncNetwork.tla"):
        shutil.copyfile(module.FORMAL_DIR / name, formal_dir / name)
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        source.replace(
            "AsyncFairnessAt(initialContext) ==",
            f"{duplicate}\n\nAsyncFairnessAt(initialContext) ==",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_spec_shape_errors(formal_dir)

    symbol = duplicate.split(" ", 1)[0].split("(", 1)[0]
    assert any(
        f"TLC-only duplicate {symbol} is prohibited" in error
        for error in errors
    ), errors


def test_generalized_core_init_cannot_regress_to_genesis_only_or_invalid_lineage(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    path = formal_dir / "SumeragiV2Core.tla"
    canonical = """---- MODULE SumeragiV2Core ----
FrozenContextAdmissible(initialContext) ==
  /\\ initialContext \\in ContextRecords
  /\\ \\A index \\in DOMAIN initialContext.lineage:
       initialContext.lineage[index] \\in ValidSubjects
InitAt(initialContext) == FrozenContextAdmissible(initialContext)
Init == InitAt(ContextRecord(0, <<>>))
=============================================================================
"""
    path.write_text(canonical, encoding="utf-8")
    assert module._generalized_context_init_errors(formal_dir) == []

    path.write_text(
        canonical.replace(
            "InitAt(initialContext) == FrozenContextAdmissible(initialContext)",
            "InitAt(initialContext) == initialContext \\in ContextRecords",
        ),
        encoding="utf-8",
    )
    errors = module._generalized_context_init_errors(formal_dir)
    assert any("InitAt must require FrozenContextAdmissible" in error for error in errors)


def test_async_source_fidelity_rejects_old_progress_shortcuts(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for relative in (
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_effects.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
        Path("crates/iroha_sumeragi_core/src/verus_proofs.rs"),
        Path("crates/iroha_sumeragi_core/VERIFICATION.md"),
        Path("scripts/verify_sumeragi_v2.sh"),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(source, encoding="utf-8")
    assert module._async_source_fidelity_errors(formal_dir) == []

    mutations = (
        (
            '    [] OTHER -> <<>>\n\n(***************************************************************************\nReducer effects',
            '    [] command.kind = "UnmodeledContinuation" -> <<>>\n'
            '    [] OTHER -> <<>>\n\n'
            '(***************************************************************************\nReducer effects',
            "CommandSuccessors parent inventory must be closed",
        ),
        (
            "![command.node] = @ \\o FreshCommandSuccessors(command)",
            "![command.node] = @ \\o CommandSuccessors(command)",
            "AppendCausalSuccessors must equal only",
        ),
        (
            "  IF CandidateScheduled(candidate) THEN <<>> ELSE <<candidate>>",
            "  IF TRUE THEN <<>> ELSE <<candidate>>",
            "FreshCandidateSequence must equal only",
        ),
        (
            "  /\\ AsyncOutstandingCarrierInvariant\n",
            "",
            "AsyncProgressOwnershipInvariant",
        ),
        (
            "    \\/ ENABLED ExecuteChunkDelivery(selectedCommand)\n",
            "",
            "CommandExecutionEnabled must equal only",
        ),
        (
            "  /\\ AsyncCandidateTyped(command)\n",
            "",
            "CommandDispatchable must equal only",
        ),
        (
            "     \\/ (\\E node \\in AsyncCurrentResponsiveVoters:\n"
            "           DirectCommitCertificateDiscoveryStep(node))\n",
            "",
            "AsyncNonRunnerStep omits required production behavior",
        ),
        (
            "  /\\ PublishCommitCertificateRequests(\n"
            "       CommitCertificateRequestOutbox(node))\n",
            "  /\\ UNCHANGED <<asyncSentItems, asyncRetainedControl,\n"
            "                  asyncActiveRequests, asyncTransport>>\n",
            "CommitCertificateDiscoveryStepWork omits required production behavior",
        ),
    )
    for needle, replacement_text, expected_error in mutations:
        assert needle in source
        path.write_text(
            source.replace(needle, replacement_text, 1),
            encoding="utf-8",
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )

    path.write_text(source + "\nnodeHeight == 0\n", encoding="utf-8")
    assert any(
        "shadow chain state nodeHeight" in error
        for error in module._async_source_fidelity_errors(formal_dir)
    )

    path.write_text(source, encoding="utf-8")
    effects_path = tmp_path / "crates/iroha_core/src/sumeragi/v2_effects.rs"
    canonical_effects = effects_path.read_text(encoding="utf-8")

    def mutate_effect_item(name: str, old: str, new: str) -> str:
        item = module.rust_items(canonical_effects, name)[0]
        assert item.source.count(old) == 1, (name, old)
        return canonical_effects.replace(item.source, item.source.replace(old, new, 1), 1)

    effects_path.write_text(
        mutate_effect_item(
            "consume_effects",
            "        if let Err(error) = self.retain_effect_batch(effects) {\n"
            "            return Err(self.close(error, services));\n"
            "        }\n"
            "        self.drain_retained_effect_batch(services)\n"
            "            .map_err(|error| self.close(error, services))",
            "        let count = self.drain_retained_effect_batch(services)?;\n"
            "        if let Err(error) = self.retain_effect_batch(effects) {\n"
            "            return Err(self.close(error, services));\n"
            "        }\n"
            "        Ok(count)",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "consume_effects must retain the complete reducer batch before draining it"
        in error
        for error in errors
    ), errors

    effects_path.write_text(
        mutate_effect_item(
            "drain_retained_effect_batch",
            ".and_then(|batch| batch.effects.front())",
            ".and_then(|batch| batch.effects.back())",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "retained dispatch must clone only the FIFO head" in error
        or "read front, consume_one, and pop_front exactly once in FIFO order" in error
        for error in errors
    ), errors

    effects_path.write_text(
        mutate_effect_item(
            "drain_retained_effect_batch",
            "                    batch.effects.pop_front();\n",
            "",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "read front, consume_one, and pop_front exactly once in FIFO order" in error
        for error in errors
    ), errors

    effects_path.write_text(
        mutate_effect_item(
            "drain_retained_effect_batch",
            "                    debug_assert!(pending_work_producer.is_some());\n"
            "                    break;\n",
            "                    debug_assert!(pending_work_producer.is_some());\n"
            "                    self.retained_effect_batch\n"
            "                        .as_mut()\n"
            "                        .expect(\"capacity-blocked head\")\n"
            "                        .effects\n"
            "                        .pop_front();\n"
            "                    break;\n",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "pending-work capacity retry must leave the FIFO head retained and stop"
        in error
        for error in errors
    ), errors

    effects_path.write_text(
        mutate_effect_item(
            "step",
            "        if self.retained_effect_batch.is_some() {\n",
            "        if false && self.retained_effect_batch.is_some() {\n",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "step must drain retained causal debt and return before runtime stepping"
        in error
        for error in errors
    ), errors
    effects_path.write_text(canonical_effects, encoding="utf-8")


def test_rust_item_scanner_masks_noncode_and_records_fail_closed_context() -> None:
    module = load_checker()
    source = r'''
/* outer /* pub fn comment_fake() {} */ comment */
const RAW: &str = r###"pub fn raw_fake() { { /* not code */ } }"###;
const COOKED: &str = "the cooked string continues
pub fn cooked_fake() { [not_code()] }
across physical source lines";
macro_rules! stuffed {
    () => {
        pub fn macro_fake() {}
    }
}
stuffed_paren!(
    pub fn paren_fake() {}
);
stuffed_bracket![
    pub fn bracket_fake() {}
];
#[cfg(any())]
pub fn gated<'a>(input: &'a str) {
    let brace = '{';
    let byte = b'}';
    let escaped = "\\\"}";
}
pub fn live<'a>(input: &'a str) {
    let raw = br#"} // not code"#;
}
'''

    assert module.rust_items(source, "comment_fake") == ()
    assert module.rust_items(source, "raw_fake") == ()
    assert module.rust_items(source, "cooked_fake") == ()
    macro = module.rust_items(source, "macro_fake")
    assert len(macro) == 1
    assert macro[0].brace_context == (
        ("macro_rules", "!", "stuffed"),
        ("(", ")", "=>"),
    )
    paren = module.rust_items(source, "paren_fake")
    assert len(paren) == 1
    assert tuple(opener for opener, _position, _header in paren[0].delimiter_context) == (
        "(",
    )
    bracket = module.rust_items(source, "bracket_fake")
    assert len(bracket) == 1
    assert tuple(
        opener for opener, _position, _header in bracket[0].delimiter_context
    ) == ("[",)
    gated = module.rust_items(source, "gated")
    assert len(gated) == 1
    assert gated[0].brace_context == ()
    assert gated[0].attributes == ("#[cfg(any())]",)
    live = module.rust_items(source, "live")
    assert len(live) == 1
    assert live[0].brace_context == ()
    assert "'a" in module.rust_code_tokens(live[0].source) or (
        "'" in module.rust_code_tokens(live[0].source)
        and "a" in module.rust_code_tokens(live[0].source)
    )

    duplicate = source + "\npub fn live() {}\n"
    assert len(module.rust_items(duplicate, "live")) == 2

    file_inner = module.rust_items(
        "#![cfg(any())]\nconst MARKER: () = ();\npub fn file_gated() {}\n",
        "file_gated",
    )
    assert len(file_inner) == 1
    assert file_inner[0].ancestor_inner_attributes == ("#![cfg(any())]",)

    module_inner = module.rust_items(
        "mod hidden {\n"
        "    #![cfg_attr(feature = \"ship\", cfg(any()))]\n"
        "    const MARKER: () = ();\n"
        "    pub fn module_gated() {}\n"
        "}\n",
        "module_gated",
    )
    assert len(module_inner) == 1
    assert module_inner[0].ancestor_inner_attributes == (
        "#![cfg_attr(feature = \"ship\", cfg(any()))]",
    )


def test_production_causal_fifo_source_link_rejects_order_and_proof_mutants(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    causal_fifo_errors = module._production_causal_fifo_source_fidelity_errors
    assert causal_fifo_errors(formal_dir) == []
    # Keep this regression scoped to the production causal-FIFO seam; unrelated
    # async source contracts have their own mutation suites.
    module._async_source_fidelity_errors = causal_fifo_errors

    adapter = tmp_path / "crates/iroha_core/src/sumeragi/v2.rs"
    canonical_adapter = adapter.read_text(encoding="utf-8")
    drive_item = module.rust_items(canonical_adapter, "drive_effects")[0]
    drive_start = canonical_adapter.index(drive_item.source)
    drive_end = drive_start + len(drive_item.source)

    def mutate_drive(old: str, new: str) -> str:
        assert drive_item.source.count(old) == 1, old
        return (
            canonical_adapter[:drive_start]
            + drive_item.source.replace(old, new, 1)
            + canonical_adapter[drive_end:]
        )

    helper_call = (
        "                    reducer::prepend_causal_continuation("
        "&mut pending, continuation.into_effects());\n"
    )
    assert canonical_adapter.count(helper_call) == 1
    adapter.write_text(
        canonical_adapter.replace(helper_call, "", 1), encoding="utf-8"
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "drive_effects must contain exactly one reviewed causal-persistence token sequence"
        in error
        for error in errors
    ), errors
    adapter.write_text(canonical_adapter, encoding="utf-8")

    adapter.write_text(
        mutate_drive(
            "        let mut ready = Vec::new();\n",
            "        let mut ready = Vec::new();\n"
            "        if false {\n"
            "            return Ok(Vec::new());\n"
            "        }\n",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "drive_effects declaration, contract, and complete control flow must match"
        in error
        for error in errors
    ), errors
    adapter.write_text(canonical_adapter, encoding="utf-8")

    adapter.write_text(
        canonical_adapter.replace(
            helper_call,
            "                    if false {\n"
            + helper_call
            + "                    }\n",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "drive_effects declaration, contract, and complete control flow must match"
        in error
        for error in errors
    ), errors
    adapter.write_text(canonical_adapter, encoding="utf-8")

    refinement = (
        tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    )
    canonical_refinement = refinement.read_text(encoding="utf-8")
    assert canonical_refinement.count("continuation.into_iter().rev()") == 1
    refinement.write_text(
        canonical_refinement.replace(
            "continuation.into_iter().rev()", "continuation.into_iter()", 1
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "prepend_causal_continuation reverse-iteration/push-front FIFO kernel "
        "must match the exact reviewed Rust/Verus item body" in error
        for error in errors
    ), errors
    refinement.write_text(canonical_refinement, encoding="utf-8")

    refinement.write_text(
        canonical_refinement.replace("pending.push_front(item)", "pending.push_back(item)", 1),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "prepend_causal_continuation reverse-iteration/push-front FIFO kernel "
        "must match the exact reviewed Rust/Verus item body" in error
        for error in errors
    ), errors
    refinement.write_text(canonical_refinement, encoding="utf-8")

    helper_start = canonical_refinement.index(
        "pub fn prepend_causal_continuation<T>("
    )
    helper_end = canonical_refinement.index(
        "\n}\n\n/// Caller-visible reducer action classes", helper_start
    ) + 2
    helper_source = canonical_refinement[helper_start:helper_end]
    refinement.write_text(
        canonical_refinement[:helper_start]
        + canonical_refinement[helper_end:]
        + "\nmacro_rules! stuffed_helper {\n"
        + "    () => {\n"
        + helper_source
        + "\n    };\n}\n",
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "prepend_causal_continuation kernel must have reviewed brace context" in error
        for error in errors
    ), errors
    refinement.write_text(canonical_refinement, encoding="utf-8")

    for opener, closer in (("(", ")"), ("[", "]")):
        refinement.write_text(
            canonical_refinement[:helper_start]
            + canonical_refinement[helper_end:]
            + f"\nstuffed_helper!{opener}\n"
            + helper_source
            + f"\n{closer};\n",
            encoding="utf-8",
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(
            "prepend_causal_continuation kernel must have reviewed all-delimiter context"
            in error
            for error in errors
        ), (opener, errors)
    refinement.write_text(canonical_refinement, encoding="utf-8")

    refinement.write_text(
        canonical_refinement.replace(
            "pub fn prepend_causal_continuation<T>(",
            "#[cfg(any())]\npub fn prepend_causal_continuation<T>(",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "prepend_causal_continuation kernel may not be disabled or replaced" in error
        for error in errors
    ), errors
    refinement.write_text(canonical_refinement, encoding="utf-8")

    refinement.write_text(
        "#![cfg(any())]\n" + canonical_refinement,
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "prepend_causal_continuation kernel may not be suppressed by "
        "file/module/ancestor inner cfg/cfg_attr" in error
        for error in errors
    ), errors
    refinement.write_text(canonical_refinement, encoding="utf-8")

    core = tmp_path / "crates/iroha_core/src/sumeragi/v2_core.rs"
    canonical_core = core.read_text(encoding="utf-8")
    export_start = canonical_core.index("pub(crate) use refinement::{")
    export_end = canonical_core.index("\n};", export_start) + 3
    export_source = canonical_core[export_start:export_end]
    core.write_text(
        canonical_core[:export_start]
        + canonical_core[export_end:]
        + "\nmacro_rules! stuffed_refinement_export {\n"
        + "    () => {\n"
        + export_source
        + "\n    };\n}\n",
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "require exactly one direct top-level pub(crate) use refinement::{...} "
        "export; found 0" in error
        for error in errors
    ), errors
    core.write_text(canonical_core, encoding="utf-8")

    for opener, closer in (("(", ")"), ("[", "]")):
        core.write_text(
            canonical_core[:export_start]
            + canonical_core[export_end:]
            + f"\nstuffed_refinement_export!{opener}\n"
            + export_source
            + f"\n{closer};\n",
            encoding="utf-8",
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(
            "require exactly one direct top-level pub(crate) use refinement::{...} "
            "export; found 0" in error
            for error in errors
        ), (opener, errors)
    core.write_text(canonical_core, encoding="utf-8")

    core.write_text(
        canonical_core.replace(
            "pub(crate) use refinement::{",
            "#[cfg(any())]\npub(crate) use refinement::{",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "require exactly one direct top-level pub(crate) use refinement::{...} "
        "export; found 0" in error
        for error in errors
    ), errors
    core.write_text(canonical_core, encoding="utf-8")

    core.write_text("#![cfg(any())]\n" + canonical_core, encoding="utf-8")
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "real top-level refinement export may not be suppressed by "
        "file/module/ancestor inner cfg/cfg_attr" in error
        for error in errors
    ), errors
    core.write_text(canonical_core, encoding="utf-8")

    verus = tmp_path / "crates/iroha_sumeragi_core/src/verus_proofs.rs"
    canonical_verus = verus.read_text(encoding="utf-8")
    theorem_start = canonical_verus.index(
        "pub proof fn production_reverse_push_front_refines_fifo("
    )
    theorem_end = canonical_verus.index(
        "\n\n/// Stable first-owner filter", theorem_start
    )
    verus.write_text(
        canonical_verus[:theorem_start]
        + "/*\n"
        + canonical_verus[theorem_start:theorem_end]
        + "\n*/\n"
        + canonical_verus[theorem_end + 2 :],
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "require exactly one real Rust/Verus function item named "
        "production_reverse_push_front_refines_fifo; found 0" in error
        for error in errors
    ), errors
    verus.write_text(canonical_verus, encoding="utf-8")

    completeness_item = module.rust_items(
        canonical_verus,
        "production_fresh_causal_successors_keeps_every_fresh_value",
    )[0]
    weakened_completeness = completeness_item.source.replace(
        "successors.contains(candidate) && !owned.contains(candidate)",
        "false",
        1,
    )
    assert weakened_completeness != completeness_item.source
    verus.write_text(
        canonical_verus.replace(
            completeness_item.source,
            weakened_completeness,
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "production_fresh_causal_successors_keeps_every_fresh_value declaration, "
        "contract, and body must match the exact reviewed token digest" in error
        for error in errors
    ), errors
    verus.write_text(canonical_verus, encoding="utf-8")

    assert canonical_verus.count("if owned.contains(candidate) {") >= 3
    verus.write_text(
        canonical_verus.replace(
            "if owned.contains(candidate) {",
            "if !owned.contains(candidate) {",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "stable first-owner causal-successor filter must match the exact reviewed"
        in error
        for error in errors
    ), errors
    verus.write_text(canonical_verus, encoding="utf-8")

    unique_item = module.rust_items(
        canonical_verus,
        "production_fresh_causal_successors_has_unique_values",
    )[0]
    weakened_unique = unique_item.source.replace(
        "production_fresh_causal_successors(owned, successors).no_duplicates(),",
        "production_fresh_causal_successors(owned, successors).len() >= 0,",
        1,
    )
    assert weakened_unique != unique_item.source
    verus.write_text(
        canonical_verus.replace(unique_item.source, weakened_unique, 1),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "production_fresh_causal_successors_has_unique_values declaration, "
        "contract, and body must match the exact reviewed token digest" in error
        for error in errors
    ), errors
    verus.write_text(canonical_verus, encoding="utf-8")

    proof_open = unique_item.source.find("{")
    assert proof_open > 0
    empty_unique = unique_item.source[:proof_open] + "{/* old proof body */}"
    verus.write_text(
        canonical_verus.replace(unique_item.source, empty_unique, 1),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "production_fresh_causal_successors_has_unique_values declaration, "
        "contract, and body must match the exact reviewed token digest" in error
        for error in errors
    ), errors
    verus.write_text(canonical_verus, encoding="utf-8")

    reverse_theorem_source = canonical_verus[theorem_start:theorem_end]
    verus.write_text(
        canonical_verus[:theorem_start]
        + canonical_verus[theorem_end:]
        + "\nmacro_rules! stuffed_verus_theorem {\n"
        + "    () => {\n"
        + reverse_theorem_source
        + "\n    };\n}\n",
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "production reverse-push-front FIFO theorem must have reviewed brace context"
        in error
        for error in errors
    ), errors
    verus.write_text(canonical_verus, encoding="utf-8")

    verus.write_text(
        canonical_verus.replace(
            "pub proof fn production_reverse_push_front_refines_fifo(",
            "#[cfg(any())]\n"
            "pub proof fn production_reverse_push_front_refines_fifo(",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "production reverse-push-front FIFO theorem may not be disabled or replaced"
        in error
        for error in errors
    ), errors
    verus.write_text(canonical_verus, encoding="utf-8")

    adapter.write_text(
        canonical_adapter.replace(
            "#[cfg(test)]\nmod tests {\n",
            "#[cfg(test)]\nmod tests {\n    #![cfg(any())]\n",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "strengthened TC-order regression may not be suppressed by "
        "file/module/ancestor inner cfg/cfg_attr" in error
        for error in errors
    ), errors
    adapter.write_text(canonical_adapter, encoding="utf-8")

    tc_name = "fn tc_promoted_historical_commit_is_fsynced_before_sign_and_status()"
    tc_start = canonical_adapter.index(tc_name)
    no_sign_start = canonical_adapter.index("        assert!(\n            installed", tc_start)
    match_start = canonical_adapter.index(
        "        let fetch_tag = match installed.as_slice() {", no_sign_start
    )
    match_end = canonical_adapter.index(
        "\n\n        assert!(matches!(\n            adapter\n"
        "                .body_available(fetch_tag, manifest)",
        match_start,
    )
    exact_match = canonical_adapter[match_start:match_end]
    commit_witness_start = canonical_adapter.index(
        "        let sign = adapter", match_end
    )
    tc_test_end = canonical_adapter.index(
        "\n    }\n\n    #[test]", commit_witness_start
    )
    tc_source = canonical_adapter[tc_start:tc_test_end]

    def mutate_tc(old: str, new: str) -> str:
        assert tc_source.count(old) == 1, old
        return (
            canonical_adapter[:tc_start]
            + tc_source.replace(old, new, 1)
            + canonical_adapter[tc_test_end:]
        )

    tc_mutations = (
        (
            mutate_tc(
                tc_name + " {",
                tc_name + " {\n        if false { return; }",
            ),
            "strengthened TC regression declaration and complete control flow "
            "must match the exact reviewed token digest",
        ),
        (
            canonical_adapter[:commit_witness_start]
            + "        if false {\n"
            + canonical_adapter[commit_witness_start:tc_test_end]
            + "\n        }"
            + canonical_adapter[tc_test_end:],
            "strengthened TC regression declaration and complete control flow "
            "must match the exact reviewed token digest",
        ),
        (
            canonical_adapter[:no_sign_start]
            + canonical_adapter[match_start:],
            "TC regression must reject signing and exactly match EnterView-before-FetchBody",
        ),
        (
            canonical_adapter[:match_start]
            + "        let fetch_tag = installed\n"
            "            .iter()\n"
            "            .find_map(|effect| match effect {\n"
            "                AdapterEffect::FetchBody { tag, .. } => Some(*tag),\n"
            "                _ => None,\n"
            "            })\n"
            "            .expect(\"fetch body\");"
            + canonical_adapter[match_end:],
            "TC regression must reject signing and exactly match EnterView-before-FetchBody",
        ),
        (
            canonical_adapter[:match_start]
            + exact_match.replace(
                "AdapterEffect::EnterView", "AdapterEffect::__SWAP", 1
            )
            .replace("AdapterEffect::FetchBody", "AdapterEffect::EnterView", 1)
            .replace("AdapterEffect::__SWAP", "AdapterEffect::FetchBody", 1)
            + canonical_adapter[match_end:],
            "TC regression must reject signing and exactly match EnterView-before-FetchBody",
        ),
        (
            canonical_adapter[:match_start]
            + exact_match.replace(
                "                },\n            ] if enter_tag == tag",
                "                },\n                ..\n            ] if enter_tag == tag",
                1,
            )
            + canonical_adapter[match_end:],
            "TC regression must reject signing and exactly match EnterView-before-FetchBody",
        ),
        (
            canonical_adapter.replace(
                "                && *fetched_subject == subject\n",
                "",
                1,
            ),
            "TC regression must reject signing and exactly match EnterView-before-FetchBody",
        ),
        (
            mutate_tc(
                "            }] if *tag == fetch_tag\n"
                "                && *stored_round == round",
                "            }] if *tag == timeout_tag\n"
                "                && *stored_round == round",
            ),
            "TC regression must pin exact StoreBody/ValidateBody tags, rounds, and subjects",
        ),
        (
            mutate_tc(
                "            ] if *tag == fetch_tag\n"
                "                && vote.round == round",
                "            ] if *tag == timeout_tag\n"
                "                && vote.round == round",
            ),
            "TC regression must pin the post-validation Commit signing authority, "
            "WAL, and status witness",
        ),
        (
            mutate_tc(
                "            Some(core_commit_vote.vote()),",
                "            Some(other_core_vote.vote()),",
            ),
            "TC regression must pin the post-validation Commit signing authority, "
            "WAL, and status witness",
        ),
        (
            canonical_adapter.replace(
                ".validation_succeeded(fetch_tag, round, subject, &validated)",
                ".validation_succeeded(fetch_tag, round, subject, &other)",
                1,
            ),
            "strengthened TC regression must contain exactly one "
            "adapter.validation_succeeded(fetch_tag, round, subject, &validated)",
        ),
        (
            canonical_adapter[:commit_witness_start]
            + canonical_adapter[tc_test_end:],
            "TC regression must pin the post-validation Commit signing authority, "
            "WAL, and status witness",
        ),
        (
            canonical_adapter[:commit_witness_start]
            + canonical_adapter[commit_witness_start:tc_test_end].replace(
                "wire::SumeragiV2OutboundIntentStage::PendingSignature",
                "wire::SumeragiV2OutboundIntentStage::Sent",
                1,
            )
            + canonical_adapter[tc_test_end:],
            "TC regression must pin the post-validation Commit signing authority, "
            "WAL, and status witness",
        ),
        (
            canonical_adapter[:commit_witness_start]
            + canonical_adapter[commit_witness_start:tc_test_end].replace(
                "            adapter.wal.recovered_records().len(),\n"
                "            3,\n",
                "            adapter.wal.recovered_records().len(),\n"
                "            2,\n",
                1,
            )
            + canonical_adapter[tc_test_end:],
            "TC regression must pin the post-validation Commit signing authority, "
            "WAL, and status witness",
        ),
    )
    for mutated_adapter, expected_error in tc_mutations:
        adapter.write_text(mutated_adapter, encoding="utf-8")
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
    adapter.write_text(canonical_adapter, encoding="utf-8")
    verus.write_text(canonical_verus, encoding="utf-8")

    stable_prepend = (
        "seq![candidate].add(production_fresh_causal_successors(\n"
        "                owned.insert(candidate),\n"
        "                remaining,\n"
        "            ))"
    )
    stable_reverse = (
        "production_fresh_causal_successors(\n"
        "                owned.insert(candidate),\n"
        "                remaining,\n"
        "            ).add(seq![candidate])"
    )
    assert canonical_verus.count(stable_prepend) == 1
    verus.write_text(
        canonical_verus.replace(stable_prepend, stable_reverse, 1),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "stable first-owner causal-successor filter must match the exact reviewed"
        in error
        for error in errors
    ), errors


def test_progress_witness_source_fidelity_requires_exact_decision_owner(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    path = formal_dir / "SumeragiV2LivenessProofs.tla"
    canonical = r"""---- MODULE SumeragiV2LivenessProofs ----
DecisionPipelineCandidate(node, qc, candidate) ==
  /\ candidate.class = "Completion"
  /\ candidate.node = node
  /\ candidate.height = qc.context.height
  /\ candidate.view = qc.view
  /\ candidate.subject = qc.subject
  /\ candidate.kind \in
       {"FetchBody", "RequestCertifiedBody", "FetchCertifiedBody", "StoreBody",
        "ValidateBody", "Apply"}
  /\ CandidateConsumerCurrent(candidate)
  /\ CandidateScheduled(candidate)

DecisionCompletionWitness(node, qc) ==
  \/ NodeHasApplication(node)
  \/ \E request \in asyncActiveRequests:
       /\ request.kind = "CertifiedRequest"
       /\ request.source = node
       /\ request.envelope.height = qc.context.height
       /\ request.envelope.view = qc.view
       /\ request.envelope.subject = qc.subject
  \/ \E candidate \in AsyncCandidateSet:
       DecisionPipelineCandidate(node, qc, candidate)
=============================================================================
"""
    path.write_text(canonical, encoding="utf-8")
    assert module._progress_witness_source_fidelity_errors(formal_dir) == []

    mutations = (
        ("  /\\ candidate.class = \"Completion\"\n", ""),
        (
            "  /\\ candidate.height = qc.context.height\n",
            "  /\\ candidate.height >= qc.context.height\n",
        ),
        ("  /\\ candidate.view = qc.view\n", ""),
        ("  /\\ candidate.subject = qc.subject\n", ""),
        ('       {"FetchBody", ', "       {"),
        ("  /\\ CandidateConsumerCurrent(candidate)\n", ""),
        ("  /\\ CandidateScheduled(candidate)\n", ""),
        "  /\\ candidate.height = qc.context.height\n",
        "       /\\ request.envelope.height = qc.context.height\n",
        "       /\\ request.envelope.view = qc.view\n",
        "       /\\ request.envelope.subject = qc.subject\n",
    )
    for mutation in mutations:
        if isinstance(mutation, tuple):
            needle, replacement = mutation
        else:
            needle, replacement = mutation, ""
        assert needle in canonical, needle
        path.write_text(canonical.replace(needle, replacement, 1), encoding="utf-8")
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert any(
            "exact current-consumer Decision recovery contract" in error
            for error in errors
        ), errors


def test_progress_witness_source_fidelity_seals_post_decision_timeout_boundary(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    for name in (
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Core.tla",
        "SumeragiV2AsyncNetwork.tla",
    ):
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)

    core_path = formal_dir / "SumeragiV2Core.tla"
    network_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    canonical_core = core_path.read_text(encoding="utf-8")
    canonical_network = network_path.read_text(encoding="utf-8")
    assert module._progress_witness_source_fidelity_errors(formal_dir) == []

    core_mutations = (
        (
            "    /\\ decision.qc.context = context\n",
            "",
            "NoDecisionForNode must equal only",
        ),
        (
            "     /\\ NoDecisionForNode(node)\n",
            "",
            "must have one direct, non-disjunctive NoDecisionForNode guard",
        ),
        (
            "     /\\ NoDecisionForNode(node)\n",
            "     /\\ (NoDecisionForNode(node) \\/ TRUE)\n",
            "must have one direct, non-disjunctive NoDecisionForNode guard",
        ),
        (
            "     /\\ NodeIdle(node)\n"
            "     /\\ NoDecisionForNode(node)\n"
            "     /\\ roundView + 1 \\in Views\n",
            "     /\\ NodeIdle(node)\n"
            "     /\\ roundView + 1 \\in Views\n",
            "FormTC must have one direct, non-disjunctive NoDecisionForNode guard",
        ),
        (
            "     /\\ tc.view >= nodeView[node]\n"
            "     /\\ NodeIdle(node)\n"
            "     /\\ NoDecisionForNode(node)\n"
            "     /\\ pendingInstallTC' = pendingInstallTC \\cup {request}\n",
            "     /\\ tc.view >= nodeView[node]\n"
            "     /\\ NodeIdle(node)\n"
            "     /\\ pendingInstallTC' = pendingInstallTC \\cup {request}\n",
            "BeginInstallTC must have one direct, non-disjunctive NoDecisionForNode guard",
        ),
        (
            "          IF ~NoDecisionForNode(envelope.recipient)\n",
            "          IF FALSE\n",
            "DeliverTimeout must consume the exact authenticated envelope",
        ),
        (
            "     /\\ timeoutNetwork' = timeoutNetwork \\ {envelope}\n",
            "     /\\ timeoutNetwork' = timeoutNetwork\n",
            "DeliverTimeout must consume the exact authenticated envelope",
        ),
        (
            "          IF NoDecisionForNode(envelope.recipient)\n",
            "          IF TRUE\n",
            "DeliverTC must consume the exact authenticated envelope",
        ),
        (
            "     /\\ tcNetwork' = tcNetwork \\ {envelope}\n",
            "     /\\ tcNetwork' = tcNetwork\n",
            "DeliverTC must consume the exact authenticated envelope",
        ),
    )
    for needle, replacement, expected_error in core_mutations:
        assert needle in canonical_core, needle
        core_path.write_text(
            canonical_core.replace(needle, replacement, 1), encoding="utf-8"
        )
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        core_path.write_text(canonical_core, encoding="utf-8")

    network_mutations = (
        (
            '         IF NoDecisionForNode(command.node)\n'
            '         THEN <<CausalCandidate("Progress", "FormTC", command)>>\n'
            "         ELSE <<>>\n",
            '         <<CausalCandidate("Progress", "FormTC", command)>>\n',
            "post-Decision DeliverTimeout must emit no causal successor",
        ),
        (
            '         IF NoDecisionForNode(command.node)\n'
            '         THEN <<CausalCandidate("Progress", "BeginInstallTC", command)>>\n'
            "         ELSE <<>>\n",
            '         <<CausalCandidate("Progress", "BeginInstallTC", command)>>\n',
            "post-Decision DeliverTC must emit no causal successor",
        ),
        (
            "         ELSE <<>>\n    [] command.kind = \"FormTC\" ->",
            '         ELSE <<CausalCandidate("Progress", "FormTC", command)>>\n'
            '    [] command.kind = "FormTC" ->',
            "post-Decision DeliverTimeout must emit no causal successor",
        ),
        (
            "         ELSE <<>>\n    [] command.kind = \"BeginInstallTC\" ->",
            '         ELSE <<CausalCandidate("Progress", "BeginInstallTC", command)>>\n'
            '    [] command.kind = "BeginInstallTC" ->',
            "post-Decision DeliverTC must emit no causal successor",
        ),
    )
    for needle, replacement, expected_error in network_mutations:
        assert needle in canonical_network, needle
        network_path.write_text(
            canonical_network.replace(needle, replacement, 1), encoding="utf-8"
        )
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        network_path.write_text(canonical_network, encoding="utf-8")


def test_progress_witness_source_fidelity_requires_exact_crash_authority(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    for name in (
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2AsyncLivenessProofs.tla",
    ):
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)

    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    canonical = path.read_text(encoding="utf-8")
    assert module._progress_witness_source_fidelity_errors(formal_dir) == []

    mutations = (
        (
            "  \\/ CommitRecoveryAuthority(node)\n",
            "",
            "AsyncCommitIntentProgressWitness must equal only",
        ),
        (
            "  /\\ generation[node] = asyncRecoveryGeneration\n",
            "  /\\ generation[node] <= asyncRecoveryGeneration\n",
            "CommitRecoveryAuthority must equal only",
        ),
        (
            "  /\\ asyncRecoveryNode = node\n",
            "",
            "CommitRecoveryAuthority must equal only",
        ),
        (
            "  /\\ AsyncDurableCommitProgressWitness\n",
            "  /\\ DurableCommitProgressWitness\n",
            "AsyncProgressWitnessInvariant must equal only",
        ),
        (
            "       /\\ CommitRecoveryAuthority(node)'\n",
            "",
            "responsive crash theorem must state only",
        ),
        (
            "AsyncProgressWitnessProperty(AsyncSpecAt(initialContext))",
            "ProgressWitnessProperty(AsyncSpecAt(initialContext))",
            "ProgressWitnessObligation must use the crash-aware async property",
        ),
        (
            "DecisionFetchBodyOwned(node, qc) ==\n"
            "  \\E candidate \\in AsyncCandidateSet:\n"
            "    /\\ candidate.kind = \"FetchBody\"\n",
            "DecisionFetchBodyOwned(node, qc) ==\n"
            "  \\E candidate \\in AsyncCandidateSet:\n"
            "    /\\ candidate.kind = \"StoreBody\"\n",
            "DecisionFetchBodyOwned must equal only",
        ),
        (
            "DecisionSourceRetentionInvariant ==\n"
            "  \\A decision \\in decisions:\n"
            "    (decision.node \\in AsyncCurrentResponsiveVoters\n"
            "      /\\ decision.qc.context = context)\n",
            "DecisionSourceRetentionInvariant ==\n"
            "  \\A decision \\in decisions:\n"
            "    decision.node \\in AsyncCurrentResponsiveVoters\n",
            "DecisionSourceRetentionInvariant must equal only",
        ),
        (
            "THEOREM PersistDecisionRecoveryUsesCompletionFetchBody ==\n"
            "  \\A command:\n"
            "    command.kind = \"PersistDecision\"\n",
            "THEOREM PersistDecisionRecoveryUsesCompletionFetchBody ==\n"
            "  \\A command:\n"
            "    command.kind = \"BeginDecision\"\n",
            "PersistDecision recovery theorem must state only",
        ),
        (
            "         /\\ Len(CommandSuccessors(command)) = 1\n",
            "         /\\ Len(CommandSuccessors(command)) = 3\n",
            "PersistDecision recovery theorem must state only",
        ),
        (
            "BY DEF CommandSuccessors, CausalCandidate, AsyncCandidateFrom,\n"
            "       AsyncCandidateWithIdentity, CandidateConsumerCurrent\n",
            "BY DEF CommandSuccessors, CausalCandidate, AsyncCandidateFrom,\n"
            "       CandidateConsumerCurrent\n",
            "derive the singleton frontier and current-consumer identity",
        ),
        (
            "PendingTimeoutExcludesDecision ==\n"
            "  \\A request \\in pendingTimeout:\n"
            "    NoDecisionForNode(request.node)\n",
            "PendingTimeoutExcludesDecision ==\n"
            "  \\A request \\in pendingTimeout:\n"
            "    TRUE\n",
            "PendingTimeoutExcludesDecision must equal only",
        ),
        (
            "  /\\ PendingDecisionExcludesTimeoutWork\n\n"
            "PostDecisionTimeoutControlExcluded ==",
            "\nPostDecisionTimeoutControlExcluded ==",
            "DecisionTimeoutFrontierInvariant must equal only",
        ),
        (
            "  /\\ specification => []PostDecisionTimeoutCausalSuccessorsExcluded\n",
            "",
            "PostDecisionTimeoutExclusionProperty must equal only",
        ),
        (
            "DecisionsUniqueByNodeContext ==\n"
            "  \\A left, right \\in decisions:\n"
            "    /\\ left.node = right.node\n"
            "    /\\ left.qc.context = right.qc.context\n",
            "DecisionsUniqueByNodeContext ==\n"
            "  \\A left, right \\in decisions:\n"
            "    /\\ left.node = right.node\n",
            "DecisionsUniqueByNodeContext must equal only",
        ),
        (
            "  /\\ generation[node] = asyncRecoveryGeneration\n"
            "  /\\ [node |-> node, qc |-> qc] \\in RestartDecisions(node)\n",
            "  /\\ generation[node] = asyncRecoveryGeneration\n",
            "DecisionRecoveryAuthority must equal only",
        ),
        (
            "AsyncDecisionCompletionWitness(node, qc) ==\n"
            "  \\/ DecisionCompletionWitness(node, qc)\n"
            "  \\/ DecisionRecoveryAuthority(node, qc)\n",
            "AsyncDecisionCompletionWitness(node, qc) ==\n"
            "  DecisionCompletionWitness(node, qc)\n",
            "AsyncDecisionCompletionWitness must equal only",
        ),
        (
            "       /\\ PreGstResponsiveReplay\n"
            "       => DecisionRecoveryStage(node, qc)'\n",
            "       /\\ PreGstResponsiveReplay\n"
            "       => DecisionRecoveryAuthority(node, qc)'\n",
            "DecisionRecoveryLifecycleTransition must equal only",
        ),
        (
            "  /\\ DecisionsUniqueByNodeContext\n"
            "  /\\ AsyncDurableDecisionProgressWitness\n",
            "  /\\ AsyncDurableDecisionProgressWitness\n",
            "AsyncProgressWitnessInvariant must equal only",
        ),
        (
            "  /\\ ProductionApplicationTraceRefinesDecisionCompletion = TRUE\n",
            "",
            "ProductionProgressWitnessTraceRefinement must equal only",
        ),
        (
            "THEOREM ProgressWitnessProductionRefinementObligation ==\n"
            "  ProductionProgressWitnessTraceRefinement\n",
            "THEOREM ProgressWitnessProductionRefinementObligation ==\n"
            "  TRUE\n",
            "production progress-witness refinement must state only",
        ),
    )
    for needle, replacement, expected_error in mutations:
        assert needle in canonical, needle
        path.write_text(canonical.replace(needle, replacement, 1), encoding="utf-8")
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        path.write_text(canonical, encoding="utf-8")


def test_async_source_fidelity_keeps_body_subjects_syntactic(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    (formal_dir / "SumeragiV2AsyncNetwork.tla").write_text(
        source.replace(
            "[node: ValidatorIds, view: Views, subject: Subjects,",
            "[node: ValidatorIds, view: Views, subject: ValidSubjects,",
            1,
        )
        .replace(
            "[recipient: ValidatorIds, height: Heights, view: Views,\n"
            "   subject: Subjects, chunk: 0..AsyncChunkCount,",
            "[recipient: ValidatorIds, height: Heights, view: Views,\n"
            "   subject: ValidSubjects, chunk: 0..AsyncChunkCount,",
            1,
        )
        .replace(
            "  /\\ envelope.subject \\in Subjects",
            "  /\\ envelope.subject \\in ValidSubjects",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("AsyncChunkReceiptSet must equal only" in error for error in errors)
    assert any("AsyncBodyEnvelopeSet must equal only" in error for error in errors)
    assert any(
        "AsyncBodyEnvelopeTyped omits required production behavior" in error
        for error in errors
    )


def test_async_source_fidelity_pins_class_cursor_and_duplicate_aware_rank(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"

    path.write_text(
        source.replace(
            'CASE commandClass = "Completion" -> "Progress"',
            'CASE commandClass = "Completion" -> "Normal"',
            1,
        ).replace(
            "SequenceWithoutIndex(@, NextNodeCommandIndex(node))",
            "Tail(@)",
            1,
        ).replace(
            "3 * Cardinality(SchedulerClassPrefixIndices(node, command))",
            "Cardinality(SchedulerCandidateIndices(node, command))",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("NextCommandClass must equal only" in error for error in errors)
    assert any("RemoveNextNodeCommand must equal only" in error for error in errors)
    assert any("SchedulerServiceRank must equal only" in error for error in errors)


def test_async_source_fidelity_pins_validator_progress_capacity(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    (formal_dir / "SumeragiV2AsyncNetwork.tla").write_text(
        source.replace(
            "AsyncIngressCapacity >= 3 * N + 1",
            "AsyncIngressCapacity >= N + 1",
            1,
        ).replace(
            "/\\ Len(lanes[recipient][source]) = 2\n"
            "           /\\ IngressLaneHasNonTimeoutProgressIn(\n"
            "                lanes, recipient, source)\n"
            "           /\\ IngressLaneHasTimeoutVoteIn(\n"
            "                lanes, recipient, source)",
            "/\\ Len(lanes[recipient][source]) = 3\n"
            "           /\\ IngressLaneHasNonTimeoutProgressIn(\n"
            "                lanes, recipient, source)\n"
            "           /\\ IngressLaneHasTimeoutVoteIn(\n"
            "                lanes, recipient, source)",
            1,
        ).replace(
            "       /\\ \\A source \\in AsyncIngressSources:\n"
            "            IngressLaneDepth(recipient, source) <=\n"
            "              AsyncIngressCapacity\n",
            "",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "AsyncConfiguration omits required production behavior" in error
        for error in errors
    )
    assert any(
        "IngressContinuationProtectedSourcesFor must equal only" in error
        for error in errors
    )
    assert any(
        "AsyncIngressCapacityTypeInvariant must equal only" in error
        for error in errors
    )


def test_ownership_n1_pins_exact_ingress_and_deferred_progress_geometry(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    path = formal_dir / "ownership_n1.cfg"
    source = (module.FORMAL_DIR / path.name).read_text(encoding="utf-8")
    path.write_text(source, encoding="utf-8")
    assert module._ownership_n1_configuration_errors(formal_dir) == []

    path.write_text(
        source.replace("  AsyncIngressCapacity = 4\n", "  AsyncIngressCapacity = 3\n", 1),
        encoding="utf-8",
    )
    errors = module._ownership_n1_configuration_errors(formal_dir)
    assert any("exact 3 * N + 1 geometry (4)" in error for error in errors)

    path.write_text(
        source.replace(
            "  AsyncDeferredProgressCapacity = 5\n",
            "  AsyncDeferredProgressCapacity = 4\n",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._ownership_n1_configuration_errors(formal_dir)
    assert any("exact 2 * N + 3 geometry (5)" in error for error in errors)

    path.write_text(
        source.replace("  N = 1\n", "  N = 2\n", 1),
        encoding="utf-8",
    )
    errors = module._ownership_n1_configuration_errors(formal_dir)
    assert any("must remain the N=1 boundary" in error for error in errors)
    assert any("exact 3 * N + 1 geometry (4)" in error for error in errors)
    assert any("exact 2 * N + 3 geometry (5)" in error for error in errors)


def test_async_source_fidelity_pins_timeout_vote_byte_reserve(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    (formal_dir / "SumeragiV2AsyncNetwork.tla").write_text(
        source.replace(
            "AsyncTimeoutVoteByteReserve == 64 * 1024",
            "AsyncTimeoutVoteByteReserve == 2 * 1024",
            1,
        ).replace(
            "/\\ ~IngressLaneHasTimeoutVoteIn(asyncIngressLanes,\n"
            "                                      item.envelope.recipient, item.source)",
            "/\\ TRUE",
            1,
        ).replace(
            "/\\ AsyncTimeoutVoteByteGateAllows(item)",
            "/\\ TRUE",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "AsyncTimeoutVoteByteReserve must equal only" in error for error in errors
    )
    assert any(
        "AsyncTimeoutVoteByteGateAllows must equal only" in error for error in errors
    )
    assert any("CanAdmitIngressItem must equal only" in error for error in errors)


def test_async_source_fidelity_requires_certificate_first_validation(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    async_source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    core_source = (module.FORMAL_DIR / "SumeragiV2Core.tla").read_text(
        encoding="utf-8"
    )

    (formal_dir / "SumeragiV2AsyncNetwork.tla").write_text(
        async_source.replace(
            "             /\\ ValidateDecidedBody(command.node, qc)",
            '             /\\ command.item.kind = "CertifiedResponse"',
            1,
        ),
        encoding="utf-8",
    )
    (formal_dir / "SumeragiV2Core.tla").write_text(
        core_source.replace(
            "  IN /\\ decision \\in decisions\n"
            '     /\\ qc.phase = "Commit"',
            "  IN /\\ ProposalAt(node, proposal) \\in seenProposals\n"
            '     /\\ qc.phase = "Commit"',
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "RegularCoreCommand ValidateBody branch omits" in error for error in errors
    )
    assert any("must rely on the exact durable decision and body" in error for error in errors)
    assert any("ValidateDecidedBody omits exact durable decision" in error for error in errors)
    assert any("must not fabricate or require leader proposal authority" in error for error in errors)

    (formal_dir / "SumeragiV2AsyncNetwork.tla").write_text(
        async_source, encoding="utf-8"
    )
    (formal_dir / "SumeragiV2Core.tla").write_text(
        core_source.replace(
            "BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)",
            "BodyHeldBy(durableBodies, node, context, qc.subject)",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "ValidateDecidedBody omits exact durable decision" in error
        and "qc.view" in error
        for error in errors
    )


def test_async_source_fidelity_requires_invalid_body_rejection(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )

    (formal_dir / "SumeragiV2AsyncNetwork.tla").write_text(
        source.replace(
            "                     \\/ RejectBody(command.node, proposal)",
            "                     \\/ ValidateBody(command.node, proposal)",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "RegularCoreCommand ValidateBody branch omits" in error
        and "RejectBody(command.node, proposal)" in error
        for error in errors
    )


def test_async_source_fidelity_requires_post_apply_historical_recovery(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace("AsyncNext => [Next]_vars", "AsyncNext => [NextV2]_vars")
        .replace(
            "  /\\ ~NodeHasApplication(node)\n"
            "  /\\ IF ResponsiveReplayQuarantined(node)",
            "  /\\ IF ResponsiveReplayQuarantined(node)",
        )
        .replace("PostGstRunHistoricalServer(node)", "PostGstRunNode(node)"),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("AsyncStepRefinesCore must equal only" in error for error in errors)
    assert any(
        "RunNodeWork omits required production behavior" in error
        for error in errors
    )
    assert any("AsyncFairnessAt omits required production behavior" in error for error in errors)


def test_async_source_fidelity_requires_timeout_signer_deduplication(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for relative in (
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_effects.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
        Path("crates/iroha_sumeragi_core/src/verus_proofs.rs"),
        Path("crates/iroha_sumeragi_core/VERIFICATION.md"),
        Path("scripts/verify_sumeragi_v2.sh"),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)
    for name in (
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
        "liveness.cfg",
    ):
        source = (module.FORMAL_DIR / name).read_text(encoding="utf-8")
        (formal_dir / name).write_text(source, encoding="utf-8")

    assert module._async_source_fidelity_errors(formal_dir) == []

    core_path = formal_dir / "SumeragiV2Core.tla"
    core_source = core_path.read_text(encoding="utf-8")
    core_path.write_text(
        core_source.replace(
            "     /\\ receivedTimeoutVotes' =\n"
            "          IF TimeoutVoteSlotOccupied(envelope.recipient, envelope.vote)\n"
            "          THEN receivedTimeoutVotes\n"
            "          ELSE receivedTimeoutVotes \\cup {received}",
            "     /\\ receivedTimeoutVotes' = receivedTimeoutVotes \\cup {received}",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("DeliverTimeout omits first-vote-per-signer" in error for error in errors)

    core_path.write_text(core_source, encoding="utf-8")
    cfg_path = formal_dir / "liveness.cfg"
    cfg_source = cfg_path.read_text(encoding="utf-8")
    cfg_path.write_text(
        cfg_source.replace(
            "INVARIANT ReceivedTimeoutVotePoolInvariant\n", "", 1
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("timeout-pool uniqueness must remain a TLC invariant" in error for error in errors)

    for invariant, expected_error in (
        (
            "AsyncProgressOwnershipInvariant",
            "scheduler progress ownership must remain a TLC invariant",
        ),
        (
            "AsyncRecoveryTypeInvariant",
            "responsive recovery state must remain a TLC invariant",
        ),
        (
            "AsyncRestartAuthorityInvariant",
            "responsive restart authority must remain a TLC invariant",
        ),
    ):
        cfg_path.write_text(
            cfg_source.replace(f"INVARIANT {invariant}\n", "", 1),
            encoding="utf-8",
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors)


def test_async_source_fidelity_pins_candidate_consumer_and_restart_state(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for relative in (
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_effects.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
        Path("crates/iroha_sumeragi_core/src/verus_proofs.rs"),
        Path("crates/iroha_sumeragi_core/VERIFICATION.md"),
        Path("scripts/verify_sumeragi_v2.sh"),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)

    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = (module.FORMAL_DIR / path.name).read_text(encoding="utf-8")
    path.write_text(source, encoding="utf-8")
    assert module._async_source_fidelity_errors(formal_dir) == []

    mutations = (
        (
            "<<asyncRecoveryPhase, asyncRecoveryNode, asyncRecoveryGeneration,\n"
            "    asyncRecoveryReplayQueue>>",
            "<<asyncRecoveryPhase, asyncRecoveryGeneration, asyncRecoveryNode,\n"
            "    asyncRecoveryReplayQueue>>",
            "AsyncRecoveryVars must equal only",
        ),
        (
            "AsyncAllVars == <<vars, AsyncSchedulerVars, AsyncRecoveryVars>>",
            "AsyncAllVars == <<vars, AsyncSchedulerVars>>",
            "AsyncAllVars must equal only",
        ),
        (
            "  /\\ asyncRecoveryPhase\n"
            '       \\notin {"RestartRequired", "ReplayRequired", "Replaying"}\n',
            "",
            "AsyncSetGST must equal only",
        ),
        (
            "  /\\ CandidateConsumerCurrent(command)\n",
            "",
            "CommandDispatchable must equal only",
        ),
        (
            "    /\\ CandidateConsumerCurrent(candidate)\n",
            "",
            "ItemInScheduledDelivery omits required production behavior",
        ),
    )
    for needle, replacement, expected_error in mutations:
        assert needle in source
        path.write_text(source.replace(needle, replacement, 1), encoding="utf-8")
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )


def test_async_source_fidelity_pins_exact_restart_fifo_and_decision_frontier(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
        "SumeragiV2AsyncLivenessProofs.tla",
    )
    paths = {
        name: formal_dir / name
        for name in (
            "SumeragiV2AsyncNetwork.tla",
            "SumeragiV2Core.tla",
            "SumeragiV2AsyncLivenessProofs.tla",
        )
    }
    sources = {
        name: path.read_text(encoding="utf-8") for name, path in paths.items()
    }
    assert module._async_source_fidelity_errors(formal_dir) == []

    mutations = (
        (
            "SumeragiV2AsyncNetwork.tla",
            "     /\\ RestartTimeoutIntents(node) = {}}\n\n"
            "RestartProposalIntents(node) ==",
            "}\n\nRestartProposalIntents(node) ==",
            "RestartPrepareIntents omits required production behavior",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "ELSE RestartTimeoutOrProposalReplay(node)\n"
            "         \\o RestartPrepareReplayIfActive(node)\n"
            "         \\o RestartLockedCommitReplayIfActive(node)",
            "ELSE RestartTimeoutOrProposalReplay(node)\n"
            "         \\o RestartLockedCommitReplayIfActive(node)\n"
            "         \\o RestartPrepareReplayIfActive(node)",
            "RestartSignatureReplay must equal only",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "         \\o RestartPrepareReplayIfActive(node)\n"
            "         \\o RestartLockedCommitReplayIfActive(node)",
            "         \\o RestartPrepareReplayIfActive(node)",
            "RestartSignatureReplay must equal only",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "          IF Len(signatures) > 0 THEN Tail(signatures) ELSE <<>>",
            "          IF Len(signatures) > 0 THEN <<>> ELSE <<>>",
            "PreGstResponsiveReplay omits required production behavior",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "     /\\ asyncRecoveryReplayQueue' = Tail(asyncRecoveryReplayQueue)",
            "     /\\ asyncRecoveryReplayQueue' = <<>>",
            "DriveResponsiveReplayHead omits required production behavior",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "  /\\ Len(asyncRecoveryReplayQueue) <= 2",
            "  /\\ Len(asyncRecoveryReplayQueue) <= 3",
            "AsyncRecoveryTypeInvariant omits required production behavior",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            'RestartCandidate("Completion", "FetchBody", node,\n'
            "                        qc.view, qc.subject, qc)",
            'RestartCandidate("Completion", "ValidateBody", node,\n'
            "                        qc.view, qc.subject, qc)",
            "RestartDecisionReplay omits required production behavior",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            '    [] command.kind = "PersistDecision" ->\n'
            '         <<CausalCandidate("Completion", "FetchBody", command)>>',
            '    [] command.kind = "PersistDecision" ->\n'
            '         <<CausalCandidate("Completion", "Apply", command)>>',
            "PersistDecision must schedule exactly one FetchBody frontier",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "              THEN <<CausalCandidate(\"Completion\", "
            '"ValidateBody", command)>>\n'
            "              ELSE <<>>\n"
            '         ELSE <<CausalCandidate("Completion", "StoreBody", command)>>',
            "              THEN <<CausalCandidate(\"Completion\", "
            '"ValidateBody", command)>>\n'
            "              ELSE <<CausalCandidate(\"Completion\", "
            '"RequestCertifiedBody", command)>>\n'
            '         ELSE <<CausalCandidate("Completion", "StoreBody", command)>>',
            "FetchBody successors must equal only",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "  \\/ ExecuteDecisionFetch(command)\n",
            "",
            "ExecuteCommand omits required production behavior",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "    \\/ ENABLED ExecuteDecisionFetch(selectedCommand)\n",
            "",
            "CommandExecutionEnabled must equal only",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "     THEN /\\ UNCHANGED vars\n"
            "          /\\ UNCHANGED <<asyncSentItems, asyncRetainedControl,\n"
            "                          asyncActiveRequests, asyncTransport>>",
            "     THEN /\\ ApplyDecision(command.node, command.evidence)\n"
            "          /\\ UNCHANGED <<asyncSentItems, asyncRetainedControl,\n"
            "                          asyncActiveRequests, asyncTransport>>",
            "ExecuteDecisionFetch omits required production behavior",
        ),
        (
            "SumeragiV2Core.tla",
            "     /\\ ~NodeTimedOut(node, vote.view)\n"
            '  \\/ /\\ vote.phase = "Commit"',
            '  \\/ /\\ vote.phase = "Commit"',
            "VoteResumeAuthorized omits TC vote-pool reconstruction behavior",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "      /\\ Len(RestartSignatureReplay(node)) <= 3",
            "      /\\ Len(RestartSignatureReplay(node)) <= 2",
            "RestartSignatureReplayProperties must state only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "    NodeHasApplication(node) => RestartReplay(node) = <<>>",
            "    NodeHasApplication(node) => RestartSignatureReplay(node) = <<>>",
            "AppliedRecoverySchedulesNoSameHeightWork must state only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "BY AsyncEligibleReadyLeadsToGstOrRecovery,\n"
            "   AsyncSpecAlwaysStrongTypeInvariant,\n"
            "   AsyncRecoveryCycleAtBudgetStep, SMTT(45), Isa, PTL",
            "BY AsyncEligibleReadyLeadsToGstOrRecovery,\n"
            "   AsyncRecoveryCycleAtBudgetStep, SMTT(45), Isa, PTL",
            "AsyncRecoveryEligibleAtBudgetLeadsLowerCycleOrRequired proof "
            "must cite AsyncSpecAlwaysStrongTypeInvariant",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "BY AsyncRecoveredReadyLeadsToGstOrEligible,\n"
            "   AsyncSpecAlwaysStrongTypeInvariant,\n"
            "   AsyncRecoveryCycleAtBudgetStep, SMTT(45), Isa, PTL",
            "BY AsyncRecoveredReadyLeadsToGstOrEligible,\n"
            "   AsyncRecoveryCycleAtBudgetStep, SMTT(45), Isa, PTL",
            "AsyncRecoveryRecoveredAtBudgetLeadsLowerCycleOrEligible proof "
            "must cite AsyncSpecAlwaysStrongTypeInvariant",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "BY AsyncRecoveryReplayLeadsToRecoveredReady,\n"
            "   AsyncSpecAlwaysStrongTypeInvariant,\n"
            "   AsyncRecoveryCycleAtBudgetStep, SMTT(45), Isa, PTL",
            "BY AsyncRecoveryReplayLeadsToRecoveredReady,\n"
            "   AsyncRecoveryCycleAtBudgetStep, SMTT(45), Isa, PTL",
            "AsyncRecoveryReplayAtBudgetLeadsLowerCycleOrRecovered proof "
            "must cite AsyncSpecAlwaysStrongTypeInvariant",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "BY AsyncRecoverySignatureDrainObligation,\n"
            "   AsyncSpecAlwaysStrongTypeInvariant,\n"
            "   AsyncRecoveryCycleAtBudgetStep, SMTT(45), Isa, PTL",
            "BY AsyncRecoverySignatureDrainObligation,\n"
            "   AsyncRecoveryCycleAtBudgetStep, SMTT(45), Isa, PTL",
            "AsyncRecoveryReplayingAtBudgetLeadsLowerCycleOrRecovered proof "
            "must cite AsyncSpecAlwaysStrongTypeInvariant",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "BY AsyncEligibleReadyLeadsToGstOrRecovery,\n"
            "   AsyncSpecAlwaysStrongTypeInvariant,\n"
            "   AsyncRecoveryCycleAtBudgetStep, SMTT(45), Isa, PTL",
            "BY AsyncEligibleReadyLeadsToGstOrRecovery,\n"
            "   AsyncSpecAlwaysStrongTypeInvariant, SMTT(45), Isa, PTL",
            "AsyncRecoveryEligibleAtBudgetLeadsLowerCycleOrRequired proof "
            "must cite AsyncRecoveryCycleAtBudgetStep",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "BY AsyncEligibleReadyLeadsToGstOrRecovery,\n"
            "   AsyncSpecAlwaysStrongTypeInvariant,\n"
            "   AsyncRecoveryCycleAtBudgetStep, SMTT(45), Isa, PTL",
            "BY AsyncEligibleReadyLeadsToGstOrRecovery,\n"
            "   (* AsyncSpecAlwaysStrongTypeInvariant *)\n"
            "   AsyncRecoveryCycleAtBudgetStep, SMTT(45), Isa, PTL",
            "AsyncRecoveryEligibleAtBudgetLeadsLowerCycleOrRequired proof "
            "must cite AsyncSpecAlwaysStrongTypeInvariant",
        ),
    )
    for name, needle, replacement, expected_error in mutations:
        source = sources[name]
        assert needle in source, (name, needle)
        paths[name].write_text(source.replace(needle, replacement, 1), encoding="utf-8")
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        paths[name].write_text(source, encoding="utf-8")


def test_async_source_fidelity_pins_recovery_quarantine_rearm_and_fairness(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    assert module._async_source_fidelity_errors(formal_dir) == []

    mutations = (
        (
            '{"ReplayRequired", "Replaying"}',
            '{"ReplayRequired"}',
            "ResponsiveReplayQuarantined must equal only",
        ),
        (
            '{"RestartRequired", "ReplayRequired", "Replaying"} =>\n'
            "    generation[asyncRecoveryNode] = asyncRecoveryGeneration",
            '{"RestartRequired", "ReplayRequired"} =>\n'
            "    generation[asyncRecoveryNode] = asyncRecoveryGeneration",
            "AsyncRestartAuthorityInvariant must equal only",
        ),
        (
            "       \\notin {\"RestartRequired\", \"ReplayRequired\", "
            '"Replaying"}',
            '       \\notin {"RestartRequired", "ReplayRequired"}',
            "AsyncSetGST must equal only",
        ),
        (
            "     /\\ AsyncNonCrashOuterFrame\n\n"
            "ResponsiveReplayServiceIoWorker ==",
            "\nResponsiveReplayServiceIoWorker ==",
            "fair action ResponsiveReplayRunNode must use exactly one "
            "AsyncNonCrashOuterFrame",
        ),
        (
            "  /\\ WF_AsyncAllVars(ResponsiveReplayRunNode)\n",
            "",
            "AsyncFairnessAt omits required production behavior",
        ),
        (
            "ResponsiveReplayServiceIoWorker ==\n",
            "RemovedResponsiveReplayServiceIoWorker ==\n",
            "missing source-fidelity operator ResponsiveReplayServiceIoWorker",
        ),
        (
            "  /\\ WF_AsyncAllVars(ResponsiveReplayServiceIoWorker)\n",
            "",
            "AsyncFairnessAt omits required production behavior",
        ),
        (
            "  \\/ VoteAt(node, vote) \\in receivedVotes\n",
            "  \\/ TRUE\n",
            "ReplayCommitIntentReady must equal only",
        ),
        (
            "  \\A vote \\in RestartLockedCommitIntents(node):\n"
            "    ReplayCommitIntentReady(node, vote)",
            "  \\A vote \\in commitIntents:\n"
            "    ReplayCommitIntentReady(node, vote)",
            "ReplayCommitSourcesReady must equal only",
        ),
        (
            "     /\\ ReplayCommitSourcesReady(node)\n",
            "",
            "FinishResponsiveReplay omits required production behavior",
        ),
        (
            "          /\\ asyncIngressReady[node] = <<>>\n",
            "",
            "RunNodeWork omits required production behavior",
        ),
        (
            "     /\\ ~ResponsiveReplayQuarantined(recipient)\n"
            "     /\\ DueSourcePackets(recipient, source) # {}",
            "     /\\ DueSourcePackets(recipient, source) # {}",
            "AdmitHiddenPacket omits required production behavior",
        ),
        (
            "        /\\ \\A request \\in asyncActiveRequests:\n"
            "             request.source # asyncRecoveryNode\n",
            "",
            "AsyncRecoveryTypeInvariant omits required production behavior",
        ),
        (
            "  \\/ /\\ RearmResponsiveRecovery\n"
            "     /\\ UNCHANGED up",
            "  \\/ /\\ UNCHANGED AsyncAllVars\n"
            "     /\\ UNCHANGED up",
            "AsyncNonCrashStep omits required production behavior",
        ),
        (
            '  /\\ asyncRecoveryPhase\' = "Eligible"\n',
            '  /\\ asyncRecoveryPhase\' = "Recovered"\n',
            "RearmResponsiveRecovery omits required production behavior",
        ),
        (
            "  /\\ generation[node] < MaxGeneration\n"
            "  /\\ Crash(node)",
            "  /\\ Crash(node)",
            "PreGstResponsiveCrash omits required production behavior",
        ),
    )
    for needle, replacement, expected_error in mutations:
        assert needle in source, needle
        path.write_text(source.replace(needle, replacement, 1), encoding="utf-8")
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        path.write_text(source, encoding="utf-8")


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "AsyncCoreOuterFrame",
            "UNCHANGED <<height, context>>",
            "UNCHANGED height",
        ),
        (
            "AsyncNonCrashOuterFrame",
            "/\\ UNCHANGED AsyncRecoveryVars",
            "/\\ UNCHANGED AsyncSchedulerVars",
        ),
        (
            "AsyncNonRunnerOuterFrame",
            "/\\ UNCHANGED asyncNodeServiceDeadlines",
            "/\\ UNCHANGED asyncIoServiceDeadlines",
        ),
        (
            "AsyncRecoveryOuterFrame",
            "/\\ UNCHANGED up",
            "/\\ UNCHANGED AsyncRecoveryVars",
        ),
    ),
)
def test_async_source_fidelity_pins_exact_outer_frame_helpers(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(f"{symbol} must equal only" in error for error in errors), errors


@pytest.mark.parametrize(
    ("old", "new"),
    (
        ("  \\/ AsyncTick\n", ""),
        (
            "  \\/ (\\E node \\in Responsive:\n"
            "        PostGstOpenHistoricalRecovery(node))",
            "  \\/ (\\E node \\in ValidatorIds:\n"
            "        PostGstOpenHistoricalRecovery(node))",
        ),
        (
            "  \\/ (\\E recipient \\in ValidatorIds, source \\in ValidatorIds:\n"
            "        PostGstAdmitHistoricalRecoveryPacket(recipient, source))",
            "",
        ),
    ),
)
def test_async_source_fidelity_pins_exact_fair_action_union(
    tmp_path: Path,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, "AsyncFairActionAt", old, new),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        "AsyncFairActionAt must equal only" in error for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new"),
    (
        (
            "AsyncFairActionAt(initialContext) => AsyncNext",
            "AsyncFairActionAt(initialContext) => TRUE",
        ),
        (
            "\\A initialContext \\in ContextRecords:",
            "\\A initialContext \\in Views:",
        ),
        (
            "/\\ AsyncSchedulerTypeInvariant",
            "/\\ TRUE",
        ),
    ),
)
def test_async_source_fidelity_pins_fair_action_refinement_claim(
    tmp_path: Path,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(
            source,
            "AsyncFairActionsRefineAsyncNext",
            old,
            new,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        "AsyncFairActionsRefineAsyncNext must equal only" in error
        for error in errors
    ), errors


def test_async_source_fidelity_rejects_a_second_model_local_theorem(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        source.replace(
            "\nAsyncFairnessAt(initialContext) ==",
            "\nTHEOREM UnreviewedAsyncEscape == TRUE\n"
            "BY OBVIOUS\n\n"
            "AsyncFairnessAt(initialContext) ==",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        "must declare exactly the reviewed local theorem inventory" in error
        and "UnreviewedAsyncEscape" in error
        for error in errors
    ), errors


def test_async_source_fidelity_pins_fairness_refinement_proof_statement(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2AsyncFairnessRefinementProofs.tla",
        "SumeragiV2Core.tla",
    )
    path = formal_dir / "SumeragiV2AsyncFairnessRefinementProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            "AsyncFairActionsRefineAsyncNextObligation",
            "AsyncFairActionAt(initialContext) => AsyncNext",
            "AsyncFairActionAt(initialContext) => TRUE",
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        "AsyncFairActionsRefineAsyncNextObligation must state only" in error
        for error in errors
    ), errors


def test_async_source_fidelity_rejects_unreviewed_fairness_proof_theorem(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2AsyncFairnessRefinementProofs.tla",
        "SumeragiV2Core.tla",
    )
    path = formal_dir / "SumeragiV2AsyncFairnessRefinementProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        source.replace(
            "\nTHEOREM AsyncFairActionsRefineAsyncNextObligation ==",
            "\nTHEOREM UnreviewedFairnessEscape == TRUE\n"
            "BY OBVIOUS\n\n"
            "THEOREM AsyncFairActionsRefineAsyncNextObligation ==",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        "fairness refinement proof must declare exactly the reviewed "
        "theorem inventory" in error
        and "UnreviewedFairnessEscape" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("action", "expected_frame"),
    (
        ("PreGstResponsiveRestart", "AsyncCoreOuterFrame"),
        ("PreGstResponsiveReplay", "AsyncCoreOuterFrame"),
        ("ResponsiveReplayRunNode", "AsyncNonCrashOuterFrame"),
        ("PostGstRunNode", "AsyncNonCrashOuterFrame"),
        ("PostGstRunHistoricalRecoveryNode", "AsyncNonCrashOuterFrame"),
        ("PostGstRunHistoricalServer", "AsyncNonCrashOuterFrame"),
        ("DriveResponsiveReplayHead", "AsyncRecoveryOuterFrame"),
        ("FinishResponsiveReplay", "AsyncRecoveryOuterFrame"),
        ("AsyncSetGST", "AsyncNonRunnerOuterFrame"),
        ("ResponsiveReplayServiceIoWorker", "AsyncNonRunnerOuterFrame"),
        ("AsyncTick", "AsyncNonRunnerOuterFrame"),
        ("PostGstOpenHistoricalRecovery", "AsyncNonRunnerOuterFrame"),
        ("PostGstCommitCertificateDiscovery", "AsyncNonRunnerOuterFrame"),
        (
            "PostGstHistoricalCommitCertificateDiscovery",
            "AsyncNonRunnerOuterFrame",
        ),
        ("PostGstServiceIoWorker", "AsyncNonRunnerOuterFrame"),
        (
            "PostGstServiceHistoricalRecoveryIoWorker",
            "AsyncNonRunnerOuterFrame",
        ),
        ("PostGstAdmitHiddenPacket", "AsyncNonRunnerOuterFrame"),
        (
            "PostGstAdmitHistoricalRecoveryPacket",
            "AsyncNonRunnerOuterFrame",
        ),
    ),
)
def test_async_source_fidelity_rejects_every_fair_action_frame_misclassification(
    tmp_path: Path,
    action: str,
    expected_frame: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    wrong_frame = (
        "AsyncNonRunnerOuterFrame"
        if expected_frame == "AsyncRecoveryOuterFrame"
        else "AsyncRecoveryOuterFrame"
    )
    path.write_text(
        mutate_tla_operator(source, action, expected_frame, wrong_frame),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        f"fair action {action} must use exactly one {expected_frame}" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("action", "expected_frame"),
    (
        ("PreGstResponsiveRestart", "AsyncCoreOuterFrame"),
        ("ResponsiveReplayRunNode", "AsyncNonCrashOuterFrame"),
        ("DriveResponsiveReplayHead", "AsyncRecoveryOuterFrame"),
        ("AsyncTick", "AsyncNonRunnerOuterFrame"),
    ),
)
def test_async_source_fidelity_rejects_deleted_fair_action_frames(
    tmp_path: Path,
    action: str,
    expected_frame: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, action, expected_frame, ""),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        f"fair action {action} must use exactly one {expected_frame}" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "WF_AsyncAllVars(AsyncSetGST)",
            "WF_AsyncAllVars(AsyncFairAction(AsyncSetGST))",
            "must name exactly the 18 canonical framed actions directly",
        ),
        (
            "\\A node \\in AsyncVotersAt(initialContext):\n"
            "       WF_AsyncAllVars(PostGstRunNode(node))",
            "\\A node \\in Responsive:\n"
            "       WF_AsyncAllVars(PostGstRunNode(node))",
            "canonical domain for every fair action",
        ),
        (
            "WF_AsyncAllVars(AsyncTick)",
            "WF_AsyncAllVars(AsyncTick)\n"
            "  /\\ WF_AsyncAllVars(AsyncSetGST)",
            "must name exactly the 18 canonical framed actions directly",
        ),
    ),
)
def test_async_source_fidelity_pins_raw_fairness_inventory_and_domains(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, "AsyncFairnessAt", old, new),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(expected_error in error for error in errors), errors


def test_async_source_fidelity_pins_restart_reset_and_retained_control(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
    )
    paths = {
        name: formal_dir / name
        for name in ("SumeragiV2AsyncNetwork.tla", "SumeragiV2Core.tla")
    }
    sources = {
        name: path.read_text(encoding="utf-8") for name, path in paths.items()
    }
    assert module._async_source_fidelity_errors(formal_dir) == []

    mutations = (
        (
            "SumeragiV2AsyncNetwork.tla",
            "     /\\ qc.subject = highestSubject[node]}",
            "}",
            "RestartHighestPrepareQCs omits required production behavior",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "     decision \\in {entry \\in decisions:",
            "     decision \\in {entry \\in commitQCs:",
            "RestartDecisionQCs omits required production behavior",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "other.view <= tc.view",
            "other.view >= tc.view",
            "RestartLastInstalledTCs omits required production behavior",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "RememberedControl(withPrepare, RestartDecisionControl(node))",
            "RememberedControl(cleared, RestartDecisionControl(node))",
            "RestartRetainedControl omits required production behavior",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "  /\\ asyncSentItems' = asyncSentItems\n"
            "  /\\ asyncRetainedControl' = RestartRetainedControl(node)",
            "  /\\ asyncSentItems' = {}\n"
            "  /\\ asyncRetainedControl' = RestartRetainedControl(node)",
            "ResetNodeSchedulerForRestart omits required production behavior",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "  /\\ asyncCommandQueues' =\n"
            "       [asyncCommandQueues EXCEPT ![node] = <<>>]",
            "  /\\ asyncCommandQueues' =\n"
            "       [other \\in ValidatorIds |-> <<>>]",
            "ResetNodeSchedulerForRestart omits required production behavior",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "  /\\ asyncHeldChunks' =\n"
            "       {receipt \\in asyncHeldChunks: receipt.node # node}",
            "  /\\ UNCHANGED asyncHeldChunks",
            "must write every and only AsyncSchedulerVars component",
        ),
        (
            "SumeragiV2Core.tla",
            "                 durableBodies, proposalIntents, prepareIntents,",
            "                 proposalIntents, prepareIntents,",
            "Crash may not orphan durable intent",
        ),
        (
            "SumeragiV2Core.tla",
            "  /\\ receivedQCs' = {entry \\in receivedQCs: entry.node # node}",
            "  /\\ receivedQCs' = {}",
            "Crash must reset volatile knowledge only for the crashed node",
        ),
        (
            "SumeragiV2Core.tla",
            "  /\\ generation' = [generation EXCEPT ![node] = @ + 1]",
            "  /\\ generation' = [generation EXCEPT ![node] = @ + 2]",
            "Restart omits authenticated generation",
        ),
    )
    for name, needle, replacement, expected_error in mutations:
        source = sources[name]
        assert needle in source, (name, needle)
        paths[name].write_text(source.replace(needle, replacement, 1), encoding="utf-8")
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        paths[name].write_text(source, encoding="utf-8")


def test_async_source_fidelity_requires_tc_commit_pool_reconstruction(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for relative in (
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_effects.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
        Path("crates/iroha_sumeragi_core/src/verus_proofs.rs"),
        Path("crates/iroha_sumeragi_core/VERIFICATION.md"),
        Path("scripts/verify_sumeragi_v2.sh"),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)
    async_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    core_path = formal_dir / "SumeragiV2Core.tla"
    async_source = (module.FORMAL_DIR / async_path.name).read_text(encoding="utf-8")
    core_source = (module.FORMAL_DIR / core_path.name).read_text(encoding="utf-8")
    async_path.write_text(async_source, encoding="utf-8")
    core_path.write_text(core_source, encoding="utf-8")

    assert module._async_source_fidelity_errors(formal_dir) == []

    async_mutations = (
        (
            "recipient \\in CurrentVoters \\ {request.node}",
            "recipient \\in CurrentVoters",
            "VoteOutbox omits required production behavior",
        ),
        (
            "ELSE <<InstallCommitSignSuccessor(command),\n"
            "         InstallProposalSuccessor(command)>>",
            "ELSE <<InstallProposalSuccessor(command)>>",
            "InstallCommandSuccessors omits required production behavior",
        ),
        (
            "              ELSE <<>>\n"
            '         ELSE <<CausalCandidate("Completion", "StoreBody", command)>>',
            "              ELSE <<CausalCandidate(\"Completion\", "
            '"RequestCertifiedBody", command)>>\n'
            '         ELSE <<CausalCandidate("Completion", "StoreBody", command)>>',
            "FetchBody successors must equal only",
        ),
    )
    for needle, replacement, expected in async_mutations:
        assert needle in async_source
        async_path.write_text(
            async_source.replace(needle, replacement, 1), encoding="utf-8"
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected in error for error in errors)
        async_path.write_text(async_source, encoding="utf-8")

    core_mutations = (
        (
            "recipient \\in CurrentVoters \\ {vote.signer}",
            "recipient \\in CurrentVoters",
            "BroadcastVotes omits TC vote-pool reconstruction behavior",
        ),
        (
            "receivedVotes \\cup {VoteAt(request.node, request.vote)}",
            "receivedVotes",
            "CompleteVoteSignature omits TC vote-pool reconstruction behavior",
        ),
        (
            "\\cup ActiveLockedCommitSignRequestsAfterInstall(node, tc)",
            "\\cup {}",
            "PersistInstallTC omits TC vote-pool reconstruction behavior",
        ),
    )
    for needle, replacement, expected in core_mutations:
        assert needle in core_source
        core_path.write_text(
            core_source.replace(needle, replacement, 1), encoding="utf-8"
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected in error for error in errors)
        core_path.write_text(core_source, encoding="utf-8")


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "  /\\ envelope \\in QcEnvelopeSet\n",
            "",
            "must retain the exact authenticated Commit/context/responsive-up/"
            "idempotence guards",
        ),
        (
            "  /\\ envelope.recipient \\in Responsive \\cap up\n",
            "  /\\ envelope.recipient \\in ValidatorIds\n",
            "must retain the exact authenticated Commit/context/responsive-up/"
            "idempotence guards",
        ),
        (
            "  /\\ envelope.qc \\in commitQCs\n",
            "",
            "must retain the exact authenticated Commit/context/responsive-up/"
            "idempotence guards",
        ),
        (
            "  /\\ envelope.qc.context = context\n",
            "",
            "must retain the exact authenticated Commit/context/responsive-up/"
            "idempotence guards",
        ),
        (
            "  /\\ envelope.qc.context = context\n",
            "  /\\ envelope.qc.context \\in ContextRecords\n",
            "must retain the exact authenticated Commit/context/responsive-up/"
            "idempotence guards",
        ),
        (
            '  /\\ envelope.qc.phase = "Commit"\n',
            "",
            "must retain the exact authenticated Commit/context/responsive-up/"
            "idempotence guards",
        ),
        (
            '  /\\ envelope.qc.phase = "Commit"\n',
            "  /\\ envelope.qc.phase \\in Phases\n",
            "must retain the exact authenticated Commit/context/responsive-up/"
            "idempotence guards",
        ),
        (
            "  /\\ QcWireValid(envelope.qc)\n",
            "",
            "must retain the exact authenticated Commit/context/responsive-up/"
            "idempotence guards",
        ),
        (
            "  /\\ envelope \\notin qcNetwork\n",
            "",
            "must retain the exact authenticated Commit/context/responsive-up/"
            "idempotence guards",
        ),
        (
            "  /\\ qcNetwork' = qcNetwork \\cup {envelope}\n",
            "  /\\ qcNetwork' = qcNetwork \\cup QcEnvelopeSet\n",
            "must write exactly one idempotent qcNetwork envelope insertion",
        ),
        (
            "  /\\ qcNetwork' = qcNetwork \\cup {envelope}\n",
            "  /\\ qcNetwork' = qcNetwork \\cup {envelope}\n"
            "  /\\ gst' = gst\n",
            "must write exactly one idempotent qcNetwork envelope insertion",
        ),
        (
            "                 up, gst, availableBodies, durableBodies,\n",
            "                 up, availableBodies, durableBodies,\n",
            "must frame exactly the 45 non-qcNetwork Core variables",
        ),
        (
            "                 voteNetwork, timeoutNetwork, tcNetwork, decisions, applied>>",
            "                 voteNetwork, qcNetwork, timeoutNetwork, tcNetwork, decisions, applied>>",
            "must frame exactly the 45 non-qcNetwork Core variables",
        ),
    ),
)
def test_core_commit_certificate_import_is_exact_and_fail_closed(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
    )
    core_path = formal_dir / "SumeragiV2Core.tla"
    source = core_path.read_text(encoding="utf-8")
    operator_start = source.index("ImportAuthenticatedCommitCertificate(envelope) ==")
    operator_end = source.index("\nDeliverQC(envelope) ==", operator_start)
    mutation = source.find(old, operator_start, operator_end)
    assert mutation >= 0, old
    core_path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(expected_error in error for error in errors), errors


def test_core_next_must_expose_exact_commit_certificate_import_arm(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
    )
    core_path = formal_dir / "SumeragiV2Core.tla"
    source = core_path.read_text(encoding="utf-8")
    arm = (
        "  \\/ \\E envelope \\in QcEnvelopeSet:\n"
        "       ImportAuthenticatedCommitCertificate(envelope)\n"
    )
    next_start = source.index("Next ==")
    mutation = source.find(arm, next_start)
    assert mutation >= 0
    core_path.write_text(
        source[:mutation] + source[mutation + len(arm) :], encoding="utf-8"
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        "Core Next must expose the exact authenticated Commit-certificate import arm"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new", "expected_error"),
    (
        (
            "HistoricalRecoveryTarget",
            "node \\in asyncHistoricalRecoveryTargets",
            "node \\in ValidatorIds",
            "HistoricalRecoveryTarget must equal only",
        ),
        (
            "HistoricalRecoverySourceReady",
            "  /\\ node \\in Responsive \\cap up\n",
            "",
            "HistoricalRecoverySourceReady must equal only",
        ),
        (
            "HistoricalRecoverySourceReady",
            "  /\\ ~NodeHasDecision(node)\n",
            "",
            "HistoricalRecoverySourceReady must equal only",
        ),
        (
            "HistoricalRecoverySourceReady",
            "  /\\ ~NodeHasApplication(node)\n",
            "",
            "HistoricalRecoverySourceReady must equal only",
        ),
        (
            "HistoricalRecoverySourceReady",
            "       NodeHasApplication(server)",
            "       TRUE",
            "HistoricalRecoverySourceReady must equal only",
        ),
        (
            "OpenHistoricalRecovery",
            "  /\\ gst\n",
            "",
            "OpenHistoricalRecovery must equal only",
        ),
        (
            "OpenHistoricalRecovery",
            "  /\\ HistoricalRecoverySourceReady(node)\n",
            "",
            "OpenHistoricalRecovery must equal only",
        ),
        (
            "OpenHistoricalRecovery",
            "  /\\ ~HistoricalRecoveryTarget(node)\n",
            "",
            "OpenHistoricalRecovery must equal only",
        ),
        (
            "OpenHistoricalRecovery",
            "       asyncHistoricalRecoveryTargets \\cup {node}",
            "       asyncHistoricalRecoveryTargets \\cup Responsive",
            "OpenHistoricalRecovery must equal only",
        ),
        (
            "AsyncTransportInit",
            "  /\\ asyncHistoricalRecoveryTargets = {}\n",
            "",
            "AsyncTransportInit omits required production behavior",
        ),
        (
            "AsyncHistoricalRecoveryTypeInvariant",
            "  /\\ asyncHistoricalRecoveryTargets \\subseteq Responsive \\cap up\n",
            "",
            "AsyncHistoricalRecoveryTypeInvariant must equal only",
        ),
        (
            "AsyncHistoricalRecoveryTypeInvariant",
            "  /\\ (asyncHistoricalRecoveryTargets # {} => gst)\n",
            "",
            "AsyncHistoricalRecoveryTypeInvariant must equal only",
        ),
        (
            "AsyncHistoricalRecoveryTypeInvariant",
            "  /\\ \\A node \\in asyncHistoricalRecoveryTargets:\n"
            "       ~NodeHasApplication(node)",
            "",
            "AsyncHistoricalRecoveryTypeInvariant must equal only",
        ),
        (
            "AsyncSchedulerTypeInvariant",
            "  /\\ AsyncHistoricalRecoveryTypeInvariant\n",
            "",
            "AsyncSchedulerTypeInvariant omits required production behavior",
        ),
        (
            "AsyncSchedulerVars",
            "    asyncIngressLanes, asyncIngressReady, asyncHeldChunks,\n"
            "    asyncHistoricalRecoveryTargets>>",
            "    asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>",
            "AsyncSchedulerVars omits required production behavior",
        ),
        (
            "AsyncSchedulerExceptHistoricalRecoveryTargets",
            "    asyncIngressLanes, asyncIngressReady, asyncHeldChunks>>",
            "    asyncIngressLanes, asyncIngressReady>>",
            "historical recovery ownership must be one exact AsyncSchedulerVars component",
        ),
        (
            "AsyncRunnerStep",
            "  \\/ (\\E node \\in asyncHistoricalRecoveryTargets:\n"
            "        RunHistoricalRecoveryNode(node))\n",
            "",
            "AsyncRunnerStep omits required production behavior",
        ),
        (
            "RunHistoricalRecoveryNode",
            "  /\\ HistoricalRecoveryTarget(node)\n",
            "",
            "RunHistoricalRecoveryNode must equal only",
        ),
        (
            "AsyncNonRunnerStep",
            "     \\/ (\\E node \\in ValidatorIds: OpenHistoricalRecovery(node))\n",
            "",
            "AsyncNonRunnerStep omits required production behavior",
        ),
        (
            "AsyncNonRunnerStep",
            "     \\/ (\\E node \\in asyncHistoricalRecoveryTargets:\n"
            "           DirectHistoricalCommitCertificateDiscoveryStep(node))\n",
            "",
            "AsyncNonRunnerStep omits required production behavior",
        ),
        (
            "AsyncNonRunnerStep",
            "     \\/ (\\E node \\in asyncHistoricalRecoveryTargets:\n"
            "           ServiceHistoricalRecoveryIoWorker(node))\n",
            "",
            "AsyncNonRunnerStep omits required production behavior",
        ),
        (
            "AsyncNonRunnerStep",
            "     \\/ (\\E node \\in asyncHistoricalRecoveryTargets:\n"
            "           EnqueueHistoricalRecoveryIoLocalControl(node))\n",
            "",
            "AsyncNonRunnerStep omits required production behavior",
        ),
        (
            "HistoricalCommitCertificateDiscoveryDue",
            "  /\\ HistoricalRecoveryTarget(node)\n",
            "",
            "HistoricalCommitCertificateDiscoveryDue must equal only",
        ),
        (
            "DirectHistoricalCommitCertificateDiscoveryStep",
            "  /\\ HistoricalCommitCertificateDiscoveryDue(node)\n",
            "",
            "DirectHistoricalCommitCertificateDiscoveryStep must equal only",
        ),
        (
            "ServiceHistoricalRecoveryIoWorker",
            "  /\\ HistoricalRecoveryTarget(node)\n",
            "",
            "ServiceHistoricalRecoveryIoWorker must equal only",
        ),
        (
            "EnqueueHistoricalRecoveryIoLocalControl",
            "  /\\ HistoricalRecoveryTarget(node)\n",
            "",
            "EnqueueHistoricalRecoveryIoLocalControl must equal only",
        ),
        (
            "CommitCertificateRequestAuthorized",
            "       \\in CurrentVoters \\cup asyncHistoricalRecoveryTargets\n",
            "       \\in CurrentVoters\n",
            "CommitCertificateRequestAuthorized omits required production behavior",
        ),
        (
            "AsyncTickEnabled",
            "                       \\cup asyncHistoricalRecoveryTargets:\n",
            ":\n",
            "AsyncTickEnabled omits required production behavior",
        ),
        (
            "HistoricalRecoveryPacketCorridor",
            "  \\/ /\\ HistoricalRecoveryTarget(source)\n"
            "        /\\ recipient \\in AsyncCurrentResponsiveVoters",
            "",
            "HistoricalRecoveryPacketCorridor must equal only",
        ),
        (
            "CommitCertificateResponseAuthorized",
            "  /\\ item.source \\in CurrentVoters\n",
            "",
            "CommitCertificateResponseAuthorized must equal only",
        ),
        (
            "CommitCertificateResponseAuthorized",
            "  /\\ item.envelope.qc \\in commitQCs\n",
            "",
            "CommitCertificateResponseAuthorized must equal only",
        ),
        (
            "CommitCertificateResponseAuthorized",
            "  /\\ item.envelope.qc.context = context\n",
            "",
            "CommitCertificateResponseAuthorized must equal only",
        ),
        (
            "CommitCertificateResponseAuthorized",
            '  /\\ item.envelope.qc.phase = "Commit"\n',
            "",
            "CommitCertificateResponseAuthorized must equal only",
        ),
        (
            "CommitCertificateResponseAuthorized",
            "  /\\ MatchingCommitCertificateRequests(item) # {}",
            "  /\\ TRUE",
            "CommitCertificateResponseAuthorized must equal only",
        ),
        (
            "DrainFairIngressSelected",
            "              /\\ item \\in asyncSentItems\n",
            "",
            "must import only an authorized, sent, not-yet-present Commit-certificate",
        ),
        (
            "DrainFairIngressSelected",
            "              /\\ CommitCertificateResponseAuthorized(item)\n",
            "",
            "must import only an authorized, sent, not-yet-present Commit-certificate",
        ),
        (
            "DrainFairIngressSelected",
            "              /\\ item.envelope \\notin qcNetwork\n",
            "",
            "must import only an authorized, sent, not-yet-present Commit-certificate",
        ),
        (
            "DrainFairIngressSelected",
            "        THEN ImportAuthenticatedCommitCertificate(item.envelope)\n",
            "        THEN UNCHANGED vars\n",
            "must import only an authorized, sent, not-yet-present Commit-certificate",
        ),
        (
            "ExecuteApply",
            "       asyncHistoricalRecoveryTargets \\ {command.node}",
            "       asyncHistoricalRecoveryTargets",
            "ExecuteApply must atomically retire only the applying node's historical recovery target",
        ),
        (
            "ResetNodeSchedulerForRestart",
            "  /\\ asyncHistoricalRecoveryTargets' =\n"
            "       asyncHistoricalRecoveryTargets \\ {node}",
            "",
            "exactly open, Apply retirement, and restart reset may write",
        ),
        (
            "AsyncTcRecordTyped",
            "  /\\ tc.votes \\subseteq TimeoutVoteRecordSet",
            "  /\\ tc \\in TcRecordSet",
            "AsyncTcRecordTyped must equal only",
        ),
        (
            "AsyncItemTyped",
            "            AsyncTcEnvelopeTyped(item.envelope)",
            "            item.envelope \\in TcEnvelopeSet",
            "AsyncItemTyped must use structural finite-value typing",
        ),
        (
            "AsyncEvidenceTyped",
            "  \\/ AsyncTcRecordTyped(evidence)\n",
            "  \\/ evidence \\in TcRecordSet\n",
            "AsyncEvidenceTyped must use structural finite-value typing",
        ),
        (
            "AsyncCandidateTyped",
            "  /\\ AsyncEvidenceTyped(candidate.evidence)\n",
            "  /\\ candidate.evidence \\in AsyncEvidenceSet\n",
            "AsyncCandidateTyped must use structural finite-value typing",
        ),
        (
            "BusyCompletionCandidates",
            "{candidate \\in ActiveBusyCompletionCarrier:",
            "{candidate \\in AsyncCandidateSet:",
            "must filter the finite ActiveBusyCompletionCarrier",
        ),
        (
            "ActiveBusyCompletionCarrier",
            "QueuedCandidates \\cup CausalCandidates \\cup TrackedWorkCandidates",
            "QueuedCandidates \\cup CausalCandidates \\cup "
            "TrackedWorkCandidates \\cup AsyncCandidateSet",
            "ActiveBusyCompletionCarrier must equal only",
        ),
        (
            "BusyCompletionWitnessInvariant",
            "      BusyCompletionCandidates(node) # {}",
            "      BusyCompletionCandidates(node) \\cap AsyncCandidateSet # {}",
            "BusyCompletionWitnessInvariant omits required production behavior",
        ),
    ),
)
def test_async_historical_recovery_and_busy_carrier_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new), encoding="utf-8"
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("symbol", "token"),
    tuple(
        (symbol, token)
        for symbol, tokens in {
            "ChangedRunNodeWorkExecutesCommand": ("RunNodeWork",),
            "ChangedAsyncRunnerExecutesCommand": (
                "RunHistoricalRecoveryNode",
            ),
            "AsyncNonRunnerStepKeepsTimeoutPool": (
                "OpenHistoricalRecovery",
                "DirectHistoricalCommitCertificateDiscoveryStep",
                "ServiceHistoricalRecoveryIoWorker",
                "EnqueueHistoricalRecoveryIoLocalControl",
            ),
            "AsyncRunnerStepLeavesDiscoveryClock": (
                "RunHistoricalRecoveryNode",
                "RunNodeWork",
            ),
            "AsyncNonRunnerStepPreservesDiscoveryClockThreshold": (
                "OpenHistoricalRecovery",
                "DirectHistoricalCommitCertificateDiscoveryStep",
                "ServiceHistoricalRecoveryIoWorker",
                "EnqueueHistoricalRecoveryIoLocalControl",
            ),
            "ReplayingRunNodeWorkPreservesCommitCarrierFrame": (
                "RunNodeWork",
            ),
            "ReplayingNonRunnerStepPreservesCommitCarrierFrame": (
                "OpenHistoricalRecovery",
                "DirectHistoricalCommitCertificateDiscoveryStep",
                "ServiceHistoricalRecoveryIoWorker",
                "EnqueueHistoricalRecoveryIoLocalControl",
            ),
            "ReplayingOrdinaryAsyncStepPreservesCommitCarrierFrame": (
                "RunHistoricalRecoveryNode",
            ),
            "EnqueueIoControlPreservesProgressOwnership": (
                "EnqueueIoLocalControlWork",
            ),
            "ServiceIoWorkerPreservesProgressOwnership": (
                "ServiceIoWorkerWork",
            ),
            "DirectCommitDiscoveryPreservesProgressOwnership": (
                "CommitCertificateDiscoveryStepWork",
            ),
            "RunNodeWorkPreservesProgressOwnership": ("RunNodeWork",),
            "AsyncNonRunnerPreservesProgressOwnership": (
                "OpenHistoricalRecovery",
                "DirectHistoricalCommitCertificateDiscoveryStep",
                "ServiceHistoricalRecoveryIoWorker",
                "EnqueueHistoricalRecoveryIoLocalControl",
            ),
            "AsyncNextPreservesProgressOwnership": (
                "RunHistoricalRecoveryNode",
            ),
            "RunNodeWorkPreservesProgressCommitSlotInvariant": (
                "RunNodeWork",
            ),
            "AsyncNonRunnerStepLeavesProgressCarriers": (
                "OpenHistoricalRecovery",
                "DirectHistoricalCommitCertificateDiscoveryStep",
                "ServiceHistoricalRecoveryIoWorker",
                "EnqueueHistoricalRecoveryIoLocalControl",
            ),
            "AsyncRunnerStepPreservesProgressCommitSlotInvariant": (
                "RunHistoricalRecoveryNode",
            ),
            "RunNodeWorkHasCommitSourceTransition": ("RunNodeWork",),
            "AsyncNextHasCommitSourceTransition": (
                "RunHistoricalRecoveryNode",
                "OpenHistoricalRecovery",
                "DirectHistoricalCommitCertificateDiscoveryStep",
                "ServiceHistoricalRecoveryIoWorker",
                "EnqueueHistoricalRecoveryIoLocalControl",
            ),
            "ProtectedStage5UnlessProgress": (
                "RunHistoricalRecoveryNode",
                "OpenHistoricalRecovery",
                "DirectHistoricalCommitCertificateDiscoveryStep",
                "ServiceHistoricalRecoveryIoWorker",
                "EnqueueHistoricalRecoveryIoLocalControl",
            ),
            "Stage4BlockedAuxStep": (
                "RunHistoricalRecoveryNode",
                "OpenHistoricalRecovery",
                "DirectHistoricalCommitCertificateDiscoveryStep",
                "ServiceHistoricalRecoveryIoWorker",
                "EnqueueHistoricalRecoveryIoLocalControl",
            ),
            "Stage4CapacityBlockedStep": (
                "RunHistoricalRecoveryNode",
                "OpenHistoricalRecovery",
                "DirectHistoricalCommitCertificateDiscoveryStep",
                "ServiceHistoricalRecoveryIoWorker",
                "EnqueueHistoricalRecoveryIoLocalControl",
            ),
            "Stage4ActionableUnlessProgress": (
                "RunHistoricalRecoveryNode",
                "OpenHistoricalRecovery",
                "DirectHistoricalCommitCertificateDiscoveryStep",
                "ServiceHistoricalRecoveryIoWorker",
                "EnqueueHistoricalRecoveryIoLocalControl",
            ),
        }.items()
        for token in tokens
    ),
)
def test_async_liveness_transition_coverage_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    token: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2Core.tla",
    )
    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        delete_tla_theorem_token(source, symbol, token),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        f"{symbol} proof omits required transition coverage" in error
        and token in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "expected_action"),
    (
        (
            "  /\\ \\A node \\in Responsive:\n"
            "       WF_AsyncAllVars(PostGstOpenHistoricalRecovery(node))\n",
            "PostGstOpenHistoricalRecovery",
        ),
        (
            "  /\\ \\A node \\in Responsive:\n"
            "       WF_AsyncAllVars(PostGstRunHistoricalRecoveryNode(node))\n",
            "PostGstRunHistoricalRecoveryNode",
        ),
        (
            "  /\\ \\A node \\in Responsive:\n"
            "       WF_AsyncAllVars(PostGstHistoricalCommitCertificateDiscovery(node))\n",
            "PostGstHistoricalCommitCertificateDiscovery",
        ),
        (
            "  /\\ \\A node \\in Responsive:\n"
            "       WF_AsyncAllVars(PostGstServiceHistoricalRecoveryIoWorker(node))\n",
            "PostGstServiceHistoricalRecoveryIoWorker",
        ),
        (
            "  /\\ \\A recipient \\in ValidatorIds, source \\in ValidatorIds:\n"
            "       WF_AsyncAllVars(\n"
            "         PostGstAdmitHistoricalRecoveryPacket(recipient, source))\n",
            "PostGstAdmitHistoricalRecoveryPacket",
        ),
    ),
)
def test_async_historical_recovery_requires_each_fair_action(
    tmp_path: Path,
    old: str,
    expected_action: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, "AsyncFairnessAt", old, ""),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        "AsyncFairnessAt omits required production behavior" in error
        and expected_action in error
        for error in errors
    ), errors


def test_async_source_fidelity_pins_certified_body_serving_authority(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    async_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    async_source = (module.FORMAL_DIR / async_path.name).read_text(
        encoding="utf-8"
    )
    needle = (
        'CertifiedServeCanRespond(request) ==\n'
        '  /\\ request.kind = "CertifiedRequest"\n'
        '  /\\ BodyHeldBy(durableBodies, request.envelope.recipient, context,\n'
        '                request.envelope.view, request.envelope.subject)'
    )
    assert needle in async_source
    async_path.write_text(
        async_source.replace(
            needle,
            needle
            + "\n  /\\ \\E validation \\in validatedBodies:\n"
            + "       validation.node = request.envelope.recipient",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "CertifiedServeCanRespond must equal only" in error for error in errors
    )


def test_async_source_fidelity_pins_deferred_cursor_and_rank(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for relative in (
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_effects.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
        Path("crates/iroha_sumeragi_core/src/verus_proofs.rs"),
        Path("crates/iroha_sumeragi_core/VERIFICATION.md"),
        Path("scripts/verify_sumeragi_v2.sh"),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)
    for name in (
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Core.tla",
        "liveness.cfg",
    ):
        shutil.copyfile(module.FORMAL_DIR / name, formal_dir / name)

    assert module._async_source_fidelity_errors(formal_dir) == []

    async_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    async_source = async_path.read_text(encoding="utf-8")
    async_path.write_text(
        async_source.replace(
            "  LET first == asyncNextDeferredClass[node]",
            '  LET first == "Completion"',
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("SelectedDeferredClass must equal only" in error for error in errors)

    async_path.write_text(
        async_source.replace(
            "                  THEN /\\ LeaveCausalQueues\n"
            "                       /\\ AdvanceNextDeferredClass(node)",
            "                  THEN /\\ LeaveCausalQueues",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "DeferredDrainStep omits required production behavior" in error
        for error in errors
    )

    async_path.write_text(async_source, encoding="utf-8")
    liveness_path = formal_dir / "SumeragiV2LivenessProofs.tla"
    liveness_source = liveness_path.read_text(encoding="utf-8")
    liveness_path.write_text(
        liveness_source.replace(
            "  3 * Cardinality(\n"
            "        DeferredClassPrefixIndices(candidate.node, candidate))",
            "  Cardinality(\n"
            "    DeferredClassPrefixIndices(candidate.node, candidate))",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("DeferredCandidatePosition must equal only" in error for error in errors)


def test_chain_composition_rejects_global_barrier_and_stale_async_shadows(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    chain = """---- MODULE SumeragiV2ChainEpoch ----
EXTENDS SumeragiV2Core
RecordCertifiedNext(decision) ==
  /\\ certifiedHeight' = nextHeight
  /\\ UNCHANGED <<nodeHeight, nodeContext, durableApplicationEvidence>>
RecordAppliedNext(application) ==
  LET node == application.node
      nextLineage == lineage
  IN /\\ nodeHeight[node] < certifiedHeight
     /\\ nodeHeight' = [nodeHeight EXCEPT ![node] = nextHeight]
     /\\ nodeContext' = [nodeContext EXCEPT ![node] = ContextRecord(nextHeight, nextLineage)]
ChainEpochNext ==
  \\/ \\E decision \\in DecisionEvidenceSet:
       RecordCertifiedNext(decision)
  \\/ \\E decision \\in DecisionEvidenceSet:
       RecordKnownDecision(decision)
  \\/ \\E application \\in DecisionEvidenceSet:
       RecordAppliedNext(application)
  \\/ \\E application \\in DecisionEvidenceSet:
       RecordKnownApplication(application)
ChainEpochSpec ==
  ChainEpochInit /\\ [][ChainEpochNext]_ChainEpochVars
CandidateHistoricalCommitCertificateSet ==
  {QC(qcContext, roundView, "Commit", subject, signers):
    qcContext \\in ContextRecords,
    roundView \\in Views,
    subject \\in ValidSubjects,
    signers \\in SUBSET ValidatorIds}
HistoricalCommitCertificateSet ==
  {qc \\in CandidateHistoricalCommitCertificateSet:
    DualQuorum(qc.context.epoch, qc.signers)}
CandidateDurableDecisionEvidenceSet ==
  {[node |-> node, qc |-> qc]:
    node \\in ValidatorIds, qc \\in HistoricalCommitCertificateSet}
DurableDecisionEvidenceSet ==
  {decision \\in CandidateDurableDecisionEvidenceSet:
    decision \\in DecisionEvidenceSet}
ChainEpochTlcVars == <<vars, ChainEpochVars>>
ChainEpochTlcInit == Init /\\ ChainEpochInit
ChainEpochTlcReceiptNext ==
  \\/ \\E decision \\in DurableDecisionEvidenceSet:
       RecordCertifiedNext(decision)
  \\/ \\E decision \\in DurableDecisionEvidenceSet:
       RecordKnownDecision(decision)
  \\/ \\E application \\in DurableDecisionEvidenceSet:
       RecordAppliedNext(application)
  \\/ \\E application \\in DurableDecisionEvidenceSet:
       RecordKnownApplication(application)
ChainEpochTlcNext == ChainEpochTlcReceiptNext /\\ UNCHANGED vars
ChainEpochTlcSpec == ChainEpochTlcInit /\\ [][ChainEpochTlcNext]_ChainEpochTlcVars
ChainEpochTlcInvariant == TypeInvariant /\\ ChainEpochInvariant
=============================================================================
"""
    chain_path = formal_dir / "SumeragiV2ChainEpoch.tla"
    chain_path.write_text(chain, encoding="utf-8")
    refinement_path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    scheduler_fields = (
        "asyncNow",
        "asyncCommandQueues",
        "asyncNextCommandClass",
        "asyncFifoOwed",
        "asyncTimeoutEmitted",
        "asyncRunnerPhase",
        "asyncRunnerBudget",
        "asyncCausalAdmissionOwed",
        "asyncNextLocalSource",
        "asyncIoQueues",
        "asyncOutstandingWork",
        "asyncIoReadyCompletions",
        "asyncLocalReadyCompletions",
        "asyncNextCompletionSource",
        "asyncIoControlAvailable",
        "asyncDeferredCompletionQueues",
        "asyncDeferredProgressQueues",
        "asyncDeferredNormalQueues",
        "asyncNextDeferredClass",
        "asyncDeferredDrainOwed",
        "asyncCausalQueues",
        "asyncOutstandingTags",
        "asyncNodeDeadlines",
        "asyncRetransmitDeadlines",
        "asyncNodeServiceDeadlines",
        "asyncIoServiceDeadlines",
        "asyncSentItems",
        "asyncRetainedControl",
        "asyncActiveRequests",
        "asyncTransport",
        "asyncIngressLanes",
        "asyncIngressReady",
        "asyncHeldChunks",
        "asyncHistoricalRecoveryTargets",
    )
    scheduler_mapping = ",\n       ".join(
        f"{field} <- IndexedScheduler(initialContext, {index})"
        for index, field in enumerate(scheduler_fields, start=1)
    )
    recovery_fields = (
        "asyncRecoveryPhase",
        "asyncRecoveryNode",
        "asyncRecoveryGeneration",
        "asyncRecoveryReplayQueue",
    )
    recovery_mapping = ",\n       ".join(
        f"{field} <- IndexedRecovery(initialContext, {index})"
        for index, field in enumerate(recovery_fields, start=1)
    )
    core_fields = (
        "height",
        "context",
        "contextHistory",
        "nodeView",
        "generation",
        "up",
        "gst",
        "availableBodies",
        "durableBodies",
        "retainedLockedBodies",
        "validatedBodies",
        "invalidBodies",
        "seenProposals",
        "receivedVotes",
        "receivedQCs",
        "receivedTimeoutVotes",
        "receivedTCs",
        "proposalIntents",
        "prepareIntents",
        "commitIntents",
        "timeoutIntents",
        "prepareQCs",
        "commitQCs",
        "formedTCs",
        "installedTCs",
        "lockRank",
        "lockSubject",
        "highestRank",
        "highestSubject",
        "pendingProposal",
        "pendingPrepare",
        "pendingObservePrepare",
        "pendingLockCommit",
        "pendingTimeout",
        "pendingInstallTC",
        "pendingDecision",
        "signProposals",
        "signVotes",
        "signTimeouts",
        "proposalNetwork",
        "voteNetwork",
        "qcNetwork",
        "timeoutNetwork",
        "tcNetwork",
        "decisions",
        "applied",
    )
    core_mapping = ",\n       ".join(
        f"{field} <- IndexedCore(initialContext, {index})"
        for index, field in enumerate(core_fields, start=1)
    )
    verification_core_mapping = ",\n       ".join(
        f"{field} <- VerificationCore({index})"
        for index, field in enumerate(core_fields, start=1)
    )
    verification_scheduler_mapping = ",\n       ".join(
        f"{field} <- VerificationScheduler({index})"
        for index, field in enumerate(scheduler_fields, start=1)
    )
    verification_recovery_mapping = ",\n       ".join(
        f"{field} <- VerificationRecovery({index})"
        for index, field in enumerate(recovery_fields, start=1)
    )
    refinement = (
        "---- MODULE SumeragiV2ChainEpochRefinement ----\n"
        "CONSTANT VerificationContext\n"
        "IndexedRecovery(initialContext, component) ==\n"
        "  indexedAsyncState[initialContext][3][component]\n"
        "IndexedAsync(initialContext) ==\n"
        "  INSTANCE SumeragiV2AsyncNetwork WITH\n"
        f"       {core_mapping},\n       {scheduler_mapping},\n"
        f"       {recovery_mapping}\n"
        "VerificationCore(component) ==\n"
        "  IndexedCore(VerificationContext, component)\n"
        "VerificationScheduler(component) ==\n"
        "  IndexedScheduler(VerificationContext, component)\n"
        "VerificationRecovery(component) ==\n"
        "  IndexedRecovery(VerificationContext, component)\n"
        "VerificationAsyncProof ==\n"
        "  INSTANCE SumeragiV2AsyncLivenessProofs WITH\n"
        f"       {verification_core_mapping},\n"
        f"       {verification_scheduler_mapping},\n"
        f"       {verification_recovery_mapping}\n"
        "IndexedAsyncStateShape ==\n"
        "  /\\ Len(indexedAsyncState[initialContext]) = 3\n"
        "  /\\ Len(indexedAsyncState[initialContext][1]) = 46\n"
        "  /\\ Len(indexedAsyncState[initialContext][2]) = 34\n"
        "  /\\ Len(indexedAsyncState[initialContext][3]) = 4\n"
        "THEOREM IndexedInstanceVariablesAreExact ==\n"
        "  IndexedAsyncStateShape\n"
        "    => \\A initialContext \\in AdmissibleContextRecords:\n"
        "         IndexedAsync(initialContext)!AsyncAllVars =\n"
        "           IndexedAsyncStateAt(initialContext)\n"
        "BY DEF IndexedAsyncStateShape, IndexedAsyncStateAt,\n"
        "       IndexedCore, IndexedScheduler, IndexedRecovery\n"
        "IndexedJoinedNonRunnerStep(initialContext) ==\n"
        "  /\\ \\/ \\E node \\in IndexedAsync(initialContext)!\n"
        "                  AsyncCurrentResponsiveVoters:\n"
        "       /\\ IndexedNodeCurrentAt(initialContext, node)\n"
        "       /\\ IndexedAsync(initialContext)!\n"
        "            DirectCommitCertificateDiscoveryStep(node)\n"
        "  /\\ UNCHANGED IndexedScheduler(initialContext, 25)\n"
        "IndexedCommitCertificateDiscoveryStep(initialContext, node) ==\n"
        "  /\\ IndexedChainNext\n"
        "  /\\ IndexedNodeCurrentAt(initialContext, node)\n"
        "  /\\ IndexedAsync(initialContext)!\n"
        "       PostGstCommitCertificateDiscovery(node)\n"
        "IndexedFairness ==\n"
        "  \\A initialContext:\n"
        "    /\\ \\A node:\n"
        "         WF_IndexedChainVars(\n"
        "           IndexedCommitCertificateDiscoveryStep(\n"
        "             initialContext, node))\n"
        "=============================================================================\n"
    )
    refinement_path.write_text(refinement, encoding="utf-8")
    proof_path = formal_dir / "SumeragiV2ChainEpochProofs.tla"
    proof = r"""---- MODULE SumeragiV2ChainEpochProofs ----
ChainPrefixProperty(specification) ==
  specification => [](/\ HistoryPrefixComparable
                       /\ NodeAppliedPrefixBacked)
EpochBoundaryProperty(specification) ==
  specification => [](/\ PerNodeFrozenEpoch
                       /\ PerNodeParentFinality
                       /\ ForeignLineageRejected
                       /\ ForeignContextCertificateRejected)
THEOREM ChainEpochTlcReceiptNextRefinesChainEpochNext ==
  ChainEpochTlcReceiptNext => ChainEpochNext
BY DurableDecisionEvidenceSetIsWellTyped
=============================================================================
"""
    proof_path.write_text(proof, encoding="utf-8")
    assert module._chain_source_fidelity_errors(formal_dir) == []

    chain_path.write_text(
        chain.replace(
            "\\E decision \\in DecisionEvidenceSet:",
            "\\E decision \\in DurableDecisionEvidenceSet:",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("ChainEpochNext must equal only" in error for error in errors)

    chain_path.write_text(chain, encoding="utf-8")
    chain_path.write_text(
        chain.replace(
            "ChainEpochTlcNext == ChainEpochTlcReceiptNext",
            "ChainEpochTlcNext == ChainEpochNext",
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("ChainEpochTlcNext must equal only" in error for error in errors)

    chain_path.write_text(chain, encoding="utf-8")
    proof_path.write_text(
        proof.replace(
            "ChainEpochTlcReceiptNext => ChainEpochNext",
            "ChainEpochNext => ChainEpochTlcReceiptNext",
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("TLC receipt refinement must state only" in error for error in errors)

    chain_path.write_text(
        chain.replace("EXTENDS SumeragiV2Core", "EXTENDS SumeragiV2Reconfiguration")
        .replace(
            "/\\ certifiedHeight' = nextHeight",
            "/\\ CommonAppliedSubject(subject)\n  /\\ certifiedHeight' = nextHeight",
        ),
        encoding="utf-8",
    )
    refinement_path.write_text(
        "---- MODULE SumeragiV2ChainEpochRefinement ----\n"
        "BadBridge == asyncCertifiedHeight' = asyncCertifiedHeight /\\ NextV2\n"
        "=============================================================================\n",
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("may not inherit the global application-barrier" in error for error in errors)
    assert any("RecordCertifiedNext may not use global-barrier" in error for error in errors)
    assert any("stale async chain shadow asyncCertifiedHeight" in error for error in errors)
    assert any("chain refinement may not depend on global-barrier" in error for error in errors)

    chain_path.write_text(chain, encoding="utf-8")
    refinement_path.write_text(refinement, encoding="utf-8")
    proof_path.write_text(
        proof.replace("/\\ NodeAppliedPrefixBacked", "/\\ TRUE")
        .replace("/\\ ForeignContextCertificateRejected", "/\\ TRUE"),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("ChainPrefixProperty must equal only" in error for error in errors)
    assert any("EpochBoundaryProperty must equal only" in error for error in errors)


def test_chain_indexed_scheduler_mapping_tracks_async_scheduler_tuple(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    source = (module.FORMAL_DIR / "SumeragiV2ChainEpochRefinement.tla").read_text(
        encoding="utf-8"
    )
    async_source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    async_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    async_path.write_text(async_source, encoding="utf-8")
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    path.write_text(
        source.replace(
            "INSTANCE SumeragiV2AsyncNetwork",
            "INSTANCE SumeragiV2Proofs",
            1,
        )
        .replace(
            "asyncNextCommandClass <- IndexedScheduler(initialContext, 3)",
            "asyncNextCommandClass <- IndexedScheduler(initialContext, 2)",
            1,
        )
        .replace(
            "Len(indexedAsyncState[initialContext][2]) = 34",
            "Len(indexedAsyncState[initialContext][2]) = 33",
            1,
        )
        .replace(
            "asyncRecoveryNode <- IndexedRecovery(initialContext, 2)",
            "asyncRecoveryNode <- IndexedRecovery(initialContext, 1)",
            1,
        )
        .replace(
            "Len(indexedAsyncState[initialContext]) = 3",
            "Len(indexedAsyncState[initialContext]) = 2",
            1,
        )
        .replace(
            "Len(indexedAsyncState[initialContext][3]) = 4",
            "Len(indexedAsyncState[initialContext][3]) = 3",
            1,
        )
        .replace(
            "Len(indexedAsyncState[initialContext][1]) = 46",
            "Len(indexedAsyncState[initialContext][1]) = 45",
            1,
        )
        .replace(
            "UNCHANGED IndexedScheduler(initialContext, 25)",
            "UNCHANGED IndexedScheduler(initialContext, 24)",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("must directly instantiate the authoritative" in error for error in errors)
    assert any("scheduler tuple mapping" in error for error in errors)
    assert any("recovery tuple mapping" in error for error in errors)
    assert any(
        "exact IndexedAsync Core/scheduler/recovery tuple" in error
        for error in errors
    )
    assert any(
        "stale Core/scheduler/recovery tuple arity" in error for error in errors
    )
    assert any("preserve scheduler slot 25" in error for error in errors)

    path.write_text(
        source.replace(
            "asyncNextCommandClass <- VerificationScheduler(3)",
            "asyncNextCommandClass <- VerificationScheduler(2)",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any(
        "VerificationAsyncProof must use the exact IndexedAsync" in error
        for error in errors
    )

    path.write_text(
        source.replace(
            "asyncRecoveryReplayQueue <- VerificationRecovery(4)",
            "asyncRecoveryReplayQueue <- VerificationRecovery(3)",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any(
        "VerificationAsyncProof must use the exact IndexedAsync" in error
        for error in errors
    )

    path.write_text(
        source.replace(
            "asyncRecoveryReplayQueue <- IndexedRecovery(initialContext, 4)",
            "asyncRecoveryReplayQueue <- IndexedRecovery(initialContext, 3)",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("recovery tuple mapping" in error for error in errors)

    path.write_text(
        source.replace(
            "INSTANCE SumeragiV2AsyncLivenessProofs",
            "INSTANCE SumeragiV2Proofs",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any(
        "VerificationAsyncProof must directly instantiate" in error
        for error in errors
    )

    path.write_text(
        source.replace(
            "asyncCausalAdmissionOwed <- IndexedScheduler(initialContext, 8)",
            "asyncCausalAdmissionOwed <- IndexedScheduler(initialContext, 7)",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("scheduler tuple mapping" in error for error in errors)

    path.write_text(
        source.replace(
            "          /\\ IndexedNodeCurrentAt(initialContext, node)\n"
            "          /\\ IndexedAsync(initialContext)!\n"
            "               DirectCommitCertificateDiscoveryStep(node)",
            "          /\\ IndexedAsync(initialContext)!\n"
            "               DirectCommitCertificateDiscoveryStep(node)",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any(
        "restrict the exact DirectCommitCertificateDiscoveryStep" in error
        for error in errors
    )

    path.write_text(
        source.replace(
            "           IndexedCommitCertificateDiscoveryStep(\n"
            "             initialContext, node))",
            "           IndexedRunNodeStep(initialContext, node))",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any(
        "exactly one weak-fair current Commit-certificate discovery" in error
        for error in errors
    )

    path.write_text(source, encoding="utf-8")
    async_path.write_text(
        async_source.replace(
            "    asyncCausalAdmissionOwed, asyncNextLocalSource, asyncIoQueues,",
            "    asyncIoQueues,",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any(
        "AsyncSchedulerVars must match the chain projection's exact ordered"
        in error
        for error in errors
    )

    async_path.write_text(
        async_source.replace(
            "<<asyncRecoveryPhase, asyncRecoveryNode, asyncRecoveryGeneration,\n"
            "    asyncRecoveryReplayQueue>>",
            "<<asyncRecoveryPhase, asyncRecoveryGeneration, asyncRecoveryNode,\n"
            "    asyncRecoveryReplayQueue>>",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any(
        "AsyncRecoveryVars must match the chain projection's exact ordered"
        in error
        for error in errors
    )

    async_path.write_text(
        async_source.replace(
            "AsyncAllVars == <<vars, AsyncSchedulerVars, AsyncRecoveryVars>>",
            "AsyncAllVars == <<vars, AsyncSchedulerVars>>",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("AsyncAllVars must equal only" in error for error in errors)
    async_path.write_text(async_source, encoding="utf-8")

    path.write_text(
        source.replace(
            "IndexedScheduler(VerificationContext, component)",
            "IndexedScheduler(VerificationContext, component + 1)",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("VerificationScheduler must equal only" in error for error in errors)

    path.write_text(
        source.replace(
            "IndexedRecovery(VerificationContext, component)",
            "IndexedRecovery(VerificationContext, component + 1)",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("VerificationRecovery must equal only" in error for error in errors)

    path.write_text(
        source.replace(
            "indexedAsyncState[initialContext][3][component]",
            "indexedAsyncState[initialContext][2][component]",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("IndexedRecovery must equal only" in error for error in errors)

    path.write_text(
        source.replace(
            "IndexedCore, IndexedScheduler, IndexedRecovery",
            "IndexedCore, IndexedScheduler",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any(
        "IndexedInstanceVariablesAreExact must unfold every exact tuple projection"
        in error
        for error in errors
    )

    path.write_text(
        source.replace("CONSTANT VerificationContext\n", "", 1),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("missing proof-only VerificationContext" in error for error in errors)


def _mutate_chain_operator(
    source: str,
    symbol: str,
    old: str,
    new: str,
) -> str:
    """Replace one fragment after an exact top-level chain operator declaration."""

    declaration = re.search(rf"(?m)^{re.escape(symbol)}(?:\(|\s*==)", source)
    assert declaration is not None, symbol
    position = source.find(old, declaration.start())
    assert position >= 0, (symbol, old)
    return source[:position] + new + source[position + len(old) :]


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "ActivateRecoveredSuccessorHeight",
            '"Recovered", parentContext',
            '"Applied", parentContext',
        ),
        (
            "AuthenticateRecoveredSuccessorActivation",
            'successorPredecessorStatusOwnership[parentContext][node] = "Absent"',
            'successorPredecessorStatusOwnership[parentContext][node] = "Published"',
        ),
        (
            "AuthenticateRecoveredSuccessorActivation",
            "ExactDurableParentApplication(parentContext, node, application)",
            "BypassedDurableParentApplication(parentContext, node, application)",
        ),
        (
            "ActivateRecoveredSuccessorHeight",
            "ExactCompleteTipRecoveryAuthority(",
            "BypassedCompleteTipRecoveryAuthority(",
        ),
        (
            "ActivateRecoveredSuccessorHeight",
            "UNCHANGED successorActivationStatus",
            "successorActivationStatus' =\n"
            "          [successorActivationStatus EXCEPT\n"
            '             ![parentContext][node] = "Complete"]',
        ),
        (
            "ExactSuccessorActivationToken",
            "successorContext =\n"
            "       CanonicalIndexedContext(parentContext.height + 1)",
            "successorContext.height = parentContext.height + 1",
        ),
    ),
)
def test_chain_successor_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        _mutate_chain_operator(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any(symbol in error for error in errors), errors


@pytest.mark.parametrize(
    ("old", "new"),
    (
        (
            '                 "Applied", parentContext, node, successorContext)',
            '                 "Recovered", parentContext, node, successorContext)',
        ),
        (
            "     /\\ successorActivationPrerequisites[parentContext][node] = {}\n",
            "",
        ),
        (
            "     /\\ token \\notin successorActivationTokens\n",
            "",
        ),
    ),
)
def test_chain_begin_successor_requires_clean_exact_applied_start(
    tmp_path: Path,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        _mutate_chain_operator(
            source,
            "BeginSuccessorActivation",
            old,
            new,
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any("BeginSuccessorActivation" in error for error in errors), errors


def test_successor_stale_token_mutation_artifacts_are_pinned() -> None:
    module = load_checker()

    assert (
        module._successor_stale_token_mutation_source_fidelity_errors(
            module.FORMAL_DIR
        )
        == []
    )


@pytest.mark.parametrize(
    "artifact",
    (
        "SumeragiV2SuccessorStaleTokenMutation.tla",
        "successor_stale_token_bug.cfg",
        "successor_stale_token_fixed.cfg",
    ),
)
def test_successor_stale_token_mutation_artifacts_are_required(
    tmp_path: Path,
    artifact: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    (formal_dir / artifact).unlink()

    errors = module._successor_stale_token_mutation_source_fidelity_errors(
        formal_dir
    )

    assert any(
        artifact in error and "missing required" in error for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "SuccessorActivationPipelineDistance",
            "  [] OTHER -> 0",
            "  [] OTHER -> 1",
        ),
        (
            "FixedBeginSuccessorActivation",
            "  /\\ activationPrerequisites = {}\n",
            "",
        ),
        (
            "FixedBeginSuccessorActivation",
            "  /\\ AppliedSuccessorActivationToken \\notin activationTokens\n",
            "",
        ),
        (
            "BuggyBeginSuccessorActivation",
            "  /\\ ExactDurableParentApplicationWitness\n",
            "  /\\ ExactDurableParentApplicationWitness\n"
            "  /\\ activationPrerequisites = {}\n",
        ),
        (
            "MutationFailClosedSuccessorStartup",
            "  /\\ activationTokens' = {}\n",
            "  /\\ UNCHANGED activationTokens\n",
        ),
        (
            "FailClosedStrictlyDecreasesRankWitness",
            "    => SuccessorActivationRank < previousRank",
            "    => SuccessorActivationRank <= previousRank",
        ),
    ),
)
def test_successor_stale_token_mutation_model_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2SuccessorStaleTokenMutation.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new), encoding="utf-8"
    )

    errors = module._successor_stale_token_mutation_source_fidelity_errors(
        formal_dir
    )

    assert any(symbol in error for error in errors), errors


@pytest.mark.parametrize(
    ("artifact", "line"),
    (
        (
            "successor_stale_token_bug.cfg",
            "INVARIANT SuccessorActivationProtocolInvariantProjection\n",
        ),
        (
            "successor_stale_token_fixed.cfg",
            "INVARIANT FailClosedStrictlyDecreasesRankWitness\n",
        ),
    ),
)
def test_successor_stale_token_mutation_config_mutations_fail_closed(
    tmp_path: Path,
    artifact: str,
    line: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / artifact
    source = path.read_text(encoding="utf-8")
    assert line in source
    path.write_text(source.replace(line, "", 1), encoding="utf-8")

    errors = module._successor_stale_token_mutation_source_fidelity_errors(
        formal_dir
    )

    assert any(artifact in error and "configuration" in error for error in errors)


@pytest.mark.parametrize(
    ("symbol", "old", "new", "expected_error"),
    (
        (
            "IndexedHistoricalRecoveryTargetReady",
            "  /\\ node \\in Responsive\n",
            "",
            "IndexedHistoricalRecoveryTargetReady must equal only",
        ),
        (
            "IndexedHistoricalRecoveryTargetReady",
            "  /\\ node \\in IndexedCore(initialContext, 6)\n",
            "",
            "IndexedHistoricalRecoveryTargetReady must equal only",
        ),
        (
            "IndexedHistoricalRecoveryTargetReady",
            "  /\\ node \\in joinedByContext[initialContext]\n",
            "",
            "IndexedHistoricalRecoveryTargetReady must equal only",
        ),
        (
            "IndexedHistoricalRecoveryTargetReady",
            "  /\\ ExactNodeLocationAt(initialContext, node)\n",
            "",
            "IndexedHistoricalRecoveryTargetReady must equal only",
        ),
        (
            "IndexedHistoricalRecoveryTargetReady",
            "  /\\ ~IndexedAsync(initialContext)!NodeHasDecision(node)\n",
            "",
            "IndexedHistoricalRecoveryTargetReady must equal only",
        ),
        (
            "IndexedHistoricalRecoveryTargetReady",
            "  /\\ ~IndexedProjectedNodeHasApplication(initialContext, node)\n",
            "",
            "IndexedHistoricalRecoveryTargetReady must equal only",
        ),
        (
            "IndexedHistoricalRecoveryTargetReady",
            "  /\\ ~IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)\n",
            "",
            "IndexedHistoricalRecoveryTargetReady must equal only",
        ),
        (
            "IndexedHistoricalRecoverySourceReady",
            "  /\\ source \\in IndexedCurrentDecisions(initialContext)\n",
            "",
            "IndexedHistoricalRecoverySourceReady omits exact successor/exact-recovery behavior",
        ),
        (
            "IndexedHistoricalRecoverySourceReady",
            "  /\\ source \\in IndexedCurrentApplications(initialContext)\n",
            "",
            "IndexedHistoricalRecoverySourceReady omits exact successor/exact-recovery behavior",
        ),
        (
            "IndexedHistoricalRecoverySourceReady",
            "  /\\ source \\in durableDecisionEvidence\n",
            "",
            "IndexedHistoricalRecoverySourceReady omits exact successor/exact-recovery behavior",
        ),
        (
            "IndexedHistoricalRecoverySourceReady",
            "  /\\ source \\in durableApplicationEvidence\n",
            "",
            "IndexedHistoricalRecoverySourceReady omits exact successor/exact-recovery behavior",
        ),
        (
            "IndexedHistoricalRecoverySourceReady",
            "        /\\ Chain!ReceiptOutsideChainHorizon(source)\n",
            "        /\\ TRUE\n",
            "IndexedHistoricalRecoverySourceReady omits exact successor/exact-recovery behavior",
        ),
        (
            "IndexedHistoricalRecoverySourceReady",
            "  /\\ server \\in source.qc.signers \\cap Honest\n",
            "",
            "IndexedHistoricalRecoverySourceReady omits exact successor/exact-recovery behavior",
        ),
        (
            "IndexedHistoricalRecoverySourceReady",
            "  /\\ BodyHeldBy(IndexedCore(initialContext, 9), server,\n",
            "  /\\ MissingBodyAuthority(IndexedCore(initialContext, 9), server,\n",
            "IndexedHistoricalRecoverySourceReady omits exact successor/exact-recovery behavior",
        ),
        (
            "IndexedHistoricalRecoveryReady",
            "  /\\ node \\in joinedByContext[initialContext]\n",
            "",
            "IndexedHistoricalRecoveryReady must equal only",
        ),
        (
            "IndexedHistoricalRecoveryReady",
            "       IndexedHistoricalRecoverySourceReady(\n"
            "         initialContext, server, source)",
            "       TRUE",
            "IndexedHistoricalRecoveryReady must equal only",
        ),
        (
            "IndexedOpenHistoricalRecovery",
            "  /\\ IndexedHistoricalRecoveryTargetReady(initialContext, node)\n",
            "",
            "IndexedOpenHistoricalRecovery must equal only",
        ),
        (
            "IndexedOpenHistoricalRecovery",
            "  /\\ IndexedHistoricalRecoverySourceReady(\n"
            "       initialContext, server, source)\n",
            "",
            "IndexedOpenHistoricalRecovery must equal only",
        ),
        (
            "IndexedOpenHistoricalRecovery",
            "  /\\ IndexedAsync(initialContext)!OpenHistoricalRecovery(node)",
            "  /\\ TRUE",
            "IndexedOpenHistoricalRecovery must equal only",
        ),
        (
            "IndexedJoinedRunnerStep",
            "  \\/ \\E node \\in Responsive:\n"
            "       IndexedAsync(initialContext)!RunHistoricalRecoveryNode(node)\n",
            "",
            "IndexedJoinedRunnerStep omits exact successor/exact-recovery behavior",
        ),
        (
            "IndexedJoinedNonRunnerStep",
            "     \\/ \\E node \\in Responsive:\n"
            "          IndexedAsync(initialContext)!\n"
            "            DirectHistoricalCommitCertificateDiscoveryStep(node)\n",
            "",
            "IndexedJoinedNonRunnerStep omits exact successor/exact-recovery behavior",
        ),
        (
            "IndexedJoinedNonRunnerStep",
            "     \\/ \\E node \\in Responsive:\n"
            "          IndexedAsync(initialContext)!\n"
            "            ServiceHistoricalRecoveryIoWorker(node)\n",
            "",
            "IndexedJoinedNonRunnerStep omits exact successor/exact-recovery behavior",
        ),
        (
            "IndexedJoinedNonRunnerStep",
            "     \\/ \\E node \\in Responsive:\n"
            "          IndexedAsync(initialContext)!\n"
            "            EnqueueHistoricalRecoveryIoLocalControl(node)\n",
            "",
            "IndexedJoinedNonRunnerStep omits exact successor/exact-recovery behavior",
        ),
        (
            "IndexedJoinedNonRunnerStep",
            "          IndexedOpenHistoricalRecovery(\n"
            "            initialContext, node, server, source)\n",
            "          FALSE\n",
            "IndexedJoinedNonRunnerStep omits exact successor/exact-recovery behavior",
        ),
        (
            "IndexedProductActionAt",
            "  /\\ IndexedJoinedAsyncNext(initialContext)\n",
            "  /\\ TRUE\n",
            "IndexedProductActionAt must equal only",
        ),
        (
            "IndexedHistoricalRecoveryTargetCoherence",
            "      => /\\ node \\in Responsive\n",
            "      => /\\ TRUE\n",
            "IndexedHistoricalRecoveryTargetCoherence must equal only",
        ),
        (
            "IndexedHistoricalRecoveryTargetCoherence",
            "         /\\ node \\in joinedByContext[initialContext]\n",
            "",
            "IndexedHistoricalRecoveryTargetCoherence must equal only",
        ),
        (
            "IndexedHistoricalRecoveryTargetCoherence",
            "         /\\ ExactNodeLocationAt(initialContext, node)\n",
            "",
            "IndexedHistoricalRecoveryTargetCoherence must equal only",
        ),
        (
            "IndexedHistoricalRecoveryTargetCoherence",
            "         /\\ ~IndexedAsync(initialContext)!NodeHasApplication(node)",
            "",
            "IndexedHistoricalRecoveryTargetCoherence must equal only",
        ),
        (
            "HistoricalRecoveryOutstanding",
            "  /\\ node \\in joinedByContext[initialContext]\n",
            "",
            "HistoricalRecoveryOutstanding must equal only",
        ),
        (
            "HistoricalRecoveryOutstanding",
            "  /\\ ~IndexedAsync(initialContext)!NodeHasApplication(node)",
            "  /\\ TRUE",
            "HistoricalRecoveryOutstanding must equal only",
        ),
        (
            "IndexedExactHistoricalRecoveryProgress",
            "     node \\in Responsive:\n",
            "     node \\in IndexedAsync(initialContext)!AsyncVotersAt(initialContext):\n",
            "IndexedExactHistoricalRecoveryProgress must equal only",
        ),
        (
            "IndexedAllResponsiveExactApplicationsAt",
            "  \\A node \\in Responsive:\n",
            "  \\A node \\in IndexedAsync(initialContext)!AsyncVotersAt(initialContext):\n",
            "IndexedAllResponsiveExactApplicationsAt must equal only",
        ),
        (
            "IndexedContextCompleted",
            "  ELSE \\A node \\in Responsive:\n",
            "  ELSE \\A node \\in IndexedAsync(initialContext)!AsyncVotersAt(initialContext):\n",
            "IndexedContextCompleted must equal only",
        ),
        (
            "IndexedContextCompleted",
            "  THEN IndexedAllResponsiveExactApplicationsAt(initialContext)\n",
            "  THEN IndexedAsync(initialContext)!AsyncAllResponsiveAppliedAt(initialContext)\n",
            "IndexedContextCompleted must equal only",
        ),
    ),
)
def test_chain_exact_historical_recovery_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new), encoding="utf-8"
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("old", "action"),
    (
        (
            "    /\\ \\A node \\in Responsive:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedOpenHistoricalRecoveryStep(initialContext, node))\n",
            "IndexedOpenHistoricalRecoveryStep",
        ),
        (
            "    /\\ \\A node \\in Responsive:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedRunHistoricalRecoveryStep(initialContext, node))\n",
            "IndexedRunHistoricalRecoveryStep",
        ),
        (
            "    /\\ \\A node \\in Responsive:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedHistoricalCommitCertificateDiscoveryStep(\n"
            "             initialContext, node))\n",
            "IndexedHistoricalCommitCertificateDiscoveryStep",
        ),
        (
            "    /\\ \\A node \\in Responsive:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedHistoricalRecoveryIoWorkerStep(\n"
            "             initialContext, node))\n",
            "IndexedHistoricalRecoveryIoWorkerStep",
        ),
        (
            "    /\\ \\A recipient \\in ValidatorIds, source \\in ValidatorIds:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedAdmitHistoricalRecoveryPacketStep(\n"
            "             initialContext, recipient, source))\n",
            "IndexedAdmitHistoricalRecoveryPacketStep",
        ),
    ),
)
def test_chain_exact_historical_recovery_requires_each_fair_product_action(
    tmp_path: Path,
    old: str,
    action: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, "IndexedFairness", old, ""),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "IndexedFairness must contain exactly one all-required-node exact "
        "historical-recovery product clause" in error
        and action in error
        for error in errors
    ), errors


def test_chain_rejects_standalone_catch_up_state_and_transition(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        source.replace(
            "IndexedProductActionAt(initialContext) ==\n",
            "HistoricalCatchUpStage == [node \\in ValidatorIds |-> \"Idle\"]\n\n"
            "IndexedHistoricalCatchUpPipelineAction == UNCHANGED indexedAsyncState\n\n"
            "IndexedProductActionAt(initialContext) ==\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "standalone historical catch-up state or transition HistoricalCatchUpStage"
        in error
        for error in errors
    ), errors
    assert any(
        "standalone historical catch-up state or transition "
        "IndexedHistoricalCatchUpPipelineAction" in error
        for error in errors
    ), errors


def test_chain_canonical_exact_recovery_production_obligation_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    canonical = (
        "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation"
    )
    path.write_text(
        source.replace(canonical, "RetiredHistoricalCatchUpObligation", 1),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "missing canonical exact historical-recovery production refinement obligation"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    "claim",
    (
        "ProductionAppliedSuccessorTraceRefinesIndexedActivation",
        "ProductionRecoveredSuccessorTraceRefinesIndexedActivation",
        "ProductionStartupFailureRefinesFailClosedActivation",
        "ProductionHistoricalCertificateTraceRefinesIndexedAsync",
        "ProductionHistoricalBodyPipelineTraceRefinesIndexedAsync",
        "ProductionTerminalApplicationExcludesActivation",
    ),
)
def test_chain_production_trace_refinement_rejects_each_missing_claim(
    tmp_path: Path,
    claim: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(
            source,
            "ProductionSuccessorAndExactRecoveryTraceRefinement",
            f"  /\\ {claim} = TRUE\n",
            "",
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "ProductionSuccessorAndExactRecoveryTraceRefinement must equal only"
        in error
        for error in errors
    ), errors


def test_chain_production_trace_refinement_constant_inventory_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        source.replace(
            "ProductionTerminalApplicationExcludesActivation",
            "ProductionInventedTraceClaim",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "trace constants must equal the exact ordered six-claim inventory" in error
        for error in errors
    ), errors


def test_chain_production_refinement_rejects_abstract_only_theorem(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    symbol = (
        "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation"
    )
    path.write_text(
        mutate_tla_theorem(
            source,
            symbol,
            "  /\\ ProductionSuccessorAndExactRecoveryTraceRefinement\n"
            "  /\\ (IndexedChainSpec\n"
            "        => []SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant)\n",
            "  IndexedChainSpec\n"
            "    => []SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant\n",
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "canonical exact historical-recovery production refinement obligation "
        "must state only" in error
        for error in errors
    ), errors


def test_chain_production_refinement_rejects_easy_abstract_by_proof(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    symbol = (
        "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation"
    )
    closing = (
        "        => []SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant)\n"
    )
    path.write_text(
        mutate_tla_theorem(
            source,
            symbol,
            closing,
            closing
            + "BY AbstractSuccessorActivationAndExactHistoricalRecoveryInvariant\n",
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "must remain proofless until all six external source claims" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("kind", "symbol", "old", "new", "expected_error"),
    (
        (
            "operator",
            "IndexedSuccessorActivationProgress",
            "      ~> SuccessorPublicationOrSuperseded(parentContext, node)",
            "      => SuccessorPublicationOrSuperseded(parentContext, node)",
            "IndexedSuccessorActivationProgress must equal only",
        ),
        (
            "operator",
            "IndexedJoinedThroughLocalHeight",
            "                          ExactDurableParentApplication(\n"
            "                            parentContext, node, application)",
            "                          ExactDurableParentApplication(\n"
            "                            parentContext, node, application)\n"
            "            \\/ /\\ blockHeight = MaxHeight\n"
            "               /\\ IndexedAsync(\n"
            "                    CanonicalIndexedContext(blockHeight))!\n"
            "                    NodeHasApplication(node)",
            "IndexedJoinedThroughLocalHeight must equal only",
        ),
        (
            "operator",
            "IndexedActivationPendingIntoContext",
            "            CanonicalIndexedContext(initialContext.height - 1), node)",
            "            CanonicalIndexedContext(initialContext.height), node)",
            "IndexedActivationPendingIntoContext must equal only",
        ),
        (
            "theorem",
            "IndexedActivationPendingIntoContextEventuallyJoins",
            "         ~> node \\in joinedByContext[initialContext]",
            "         => node \\in joinedByContext[initialContext]",
            "IndexedActivationPendingIntoContextEventuallyJoins must state only",
        ),
        (
            "theorem",
            "IndexedReachedAncestorEventuallyJoinsEveryResponsiveNode",
            "           ~> IndexedAllResponsiveJoined(\n",
            "           => IndexedAllResponsiveJoined(\n",
            "IndexedReachedAncestorEventuallyJoinsEveryResponsiveNode must state only",
        ),
        (
            "theorem",
            "HeightLivenessFromOneHeightAndExactRecoveryProgress",
            "  /\\ IndexedSuccessorActivationProgress\n",
            "",
            "HeightLivenessFromOneHeightAndExactRecoveryProgress must state only",
        ),
    ),
)
def test_chain_activation_to_join_bridge_mutations_fail_closed(
    tmp_path: Path,
    kind: str,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    mutator = mutate_tla_operator if kind == "operator" else mutate_tla_theorem
    path.write_text(mutator(source, symbol, old, new), encoding="utf-8")

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(expected_error in error for error in errors), errors


def test_chain_rejects_retired_static_ancestor_join_theorem(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    marker = "THEOREM IndexedReachedAncestorClassifiesEveryResponsiveNode ==\n"
    path.write_text(
        source.replace(
            marker,
            "THEOREM IndexedReachedAncestorHasEveryResponsiveJoined == TRUE\n"
            "BY Isa\n\n"
            + marker,
            1,
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "retired false static ancestor-join theorem is prohibited" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "SuccessorActivationRankCarrier",
            "0..19",
            "0..20",
        ),
        (
            "SuccessorActivationPipelineDistance",
            "               THEN 9 ELSE 10",
            "               THEN 10 ELSE 10",
        ),
        (
            "SuccessorActivationRank",
            "       THEN 9 + SuccessorActivationPipelineDistance(parentContext, node)",
            "       THEN 8 + SuccessorActivationPipelineDistance(parentContext, node)",
        ),
        (
            "SuccessorActivationPending",
            "  IndexedSuccessorActivationPending(parentContext, node)",
            "  TRUE",
        ),
        (
            "SuccessorActivationHasDurableParentWitness",
            "       ExactDurableParentApplication(parentContext, node, application)",
            "       BypassedDurableParentApplication(parentContext, node, application)",
        ),
        (
            "SuccessorActivationAtRank",
            "  /\\ SuccessorActivationRank(parentContext, node) = rank",
            "  /\\ SuccessorActivationRank(parentContext, node) = rank + 1",
        ),
        (
            "SuccessorActivationPendingStructureProperty",
            "         => /\\ SuccessorActivationHasDurableParentWitness(\n"
            "                  parentContext, node)\n",
            "         => /\\ TRUE\n",
        ),
        (
            "SuccessorActivationPendingStructureProperty",
            "            /\\ ENABLED\n"
            "                 <<IndexedSuccessorActivationProgressStep(\n"
            "                     parentContext, node)>>_(IndexedChainVars)",
            "            /\\ ENABLED <<IndexedChainNext>>_(IndexedChainVars)",
        ),
        (
            "SuccessorActivationStepDecreasesRankProperty",
            "                   < SuccessorActivationRank(parentContext, node)",
            "                   <= SuccessorActivationRank(parentContext, node)",
        ),
        (
            "SuccessorActivationPendingIsNotOrphanedProperty",
            "                   <= SuccessorActivationRank(parentContext, node)",
            "                   < SuccessorActivationRank(parentContext, node)",
        ),
        (
            "SuccessorActivationOutcomeIsStableProperty",
            "        /\\ [IndexedChainNext]_IndexedChainVars\n",
            "        /\\ TRUE\n",
        ),
        (
            "SuccessorActivationRankProgressProperty",
            "      ~> (SuccessorPublicationOrSuperseded(parentContext, node)",
            "      => (SuccessorPublicationOrSuperseded(parentContext, node)",
        ),
        (
            "SuccessorActivationStarvationFreedomProperty",
            "      ~> SuccessorPublicationOrSuperseded(parentContext, node)",
            "      => SuccessorPublicationOrSuperseded(parentContext, node)",
        ),
    ),
)
def test_successor_activation_rank_corridor_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2SuccessorActivationRefinementProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new), encoding="utf-8"
    )

    errors = module._successor_activation_rank_source_fidelity_errors(formal_dir)

    assert any(symbol in error for error in errors), errors


@pytest.mark.parametrize(
    "symbol",
    (
        "SuccessorActivationPendingStructureProperty",
        "SuccessorActivationStepDecreasesRankProperty",
        "SuccessorActivationPendingIsNotOrphanedProperty",
        "SuccessorActivationOutcomeIsStableProperty",
        "SuccessorActivationRankProgressProperty",
        "SuccessorActivationStarvationFreedomProperty",
    ),
)
def test_successor_activation_release_properties_are_responsive_only(
    tmp_path: Path,
    symbol: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2SuccessorActivationRefinementProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(
            source,
            symbol,
            "node \\in Responsive",
            "node \\in ValidatorIds",
        ),
        encoding="utf-8",
    )

    errors = module._successor_activation_rank_source_fidelity_errors(formal_dir)

    assert any(symbol in error for error in errors), errors


def test_chain_successor_activation_progress_is_responsive_only(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(
            source,
            "IndexedSuccessorActivationProgress",
            "node \\in Responsive",
            "node \\in ValidatorIds",
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "IndexedSuccessorActivationProgress must equal only" in error
        for error in errors
    ), errors


def test_chain_successor_activation_fairness_is_responsive_only(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    old = (
        "    /\\ \\A node \\in Responsive:\n"
        "         WF_IndexedChainVars(\n"
        "           IndexedSuccessorActivationProgressStep(\n"
        "             initialContext, node))\n"
    )
    path.write_text(
        mutate_tla_operator(
            source,
            "IndexedFairness",
            old,
            old.replace("Responsive", "ValidatorIds"),
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "must contain exactly one responsive-validator fair "
        "successor-activation pipeline" in error
        for error in errors
    ), errors


def test_chain_successor_activation_join_bridge_is_responsive_only(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            "IndexedActivationPendingIntoContextEventuallyJoins",
            "node \\in Responsive",
            "node \\in ValidatorIds",
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "IndexedActivationPendingIntoContextEventuallyJoins must state only"
        in error
        for error in errors
    ), errors


def test_indexed_successor_activation_pending_mutation_fails_closed(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(
            source,
            "IndexedSuccessorActivationPending",
            "  /\\ ~SuccessorPublicationOrSuperseded(parentContext, node)",
            "  /\\ TRUE",
        ),
        encoding="utf-8",
    )

    errors = module._successor_activation_rank_source_fidelity_errors(formal_dir)

    assert any(
        "IndexedSuccessorActivationPending must equal only" in error
        for error in errors
    ), errors


def test_successor_activation_starvation_obligation_pins_every_conjunct(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2SuccessorActivationRefinementProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        source.replace(
            "       /\\ SuccessorActivationPendingIsNotOrphanedProperty\n",
            "",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._successor_activation_rank_source_fidelity_errors(formal_dir)

    assert any(
        "SuccessorActivationStarvationFreedomObligation must state only" in error
        for error in errors
    ), errors


def test_successor_activation_starvation_obligation_pins_composition_proof(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2SuccessorActivationRefinementProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        source.replace(
            "    BY IndexedChainSpecEstablishesSuccessorActivationNonOrphaning\n",
            "    BY IndexedChainSpecEstablishesSuccessorActivationPendingStructure\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._successor_activation_rank_source_fidelity_errors(formal_dir)

    assert any(
        "proof must compose exactly the six reviewed successor-activation "
        "theorems" in error
        for error in errors
    ), errors


def test_successor_activation_starvation_obligation_rejects_asserted_proof(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2SuccessorActivationRefinementProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            "SuccessorActivationStarvationFreedomObligation",
            "PROOF\n",
            "OBVIOUS\n",
        ),
        encoding="utf-8",
    )

    errors = module._successor_activation_rank_source_fidelity_errors(formal_dir)

    assert any(
        "must have the reviewed deductive composition proof" in error
        for error in errors
    ), errors


def test_successor_activation_starvation_chain_progress_equivalence_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2SuccessorActivationRefinementProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        source.replace(
            "THEOREM SuccessorActivationStarvationMatchesChainProgress ==\n"
            "  SuccessorActivationStarvationFreedomProperty\n"
            "    <=> IndexedSuccessorActivationProgress\n",
            "THEOREM SuccessorActivationStarvationMatchesChainProgress ==\n"
            "  SuccessorActivationStarvationFreedomProperty\n"
            "    => IndexedSuccessorActivationProgress\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._successor_activation_rank_source_fidelity_errors(formal_dir)

    assert any(
        "SuccessorActivationStarvationMatchesChainProgress must state only"
        in error
        for error in errors
    ), errors


def test_deductive_liveness_proof_cannot_import_finite_async_spec(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "SumeragiV2LivenessProofs.tla").write_text(
        "---- MODULE SumeragiV2LivenessProofs ----\n"
        "Bad == AsyncFiniteSpec\n"
        "=============================================================================\n",
        encoding="utf-8",
    )

    errors = module._async_spec_shape_errors(formal_dir)
    assert any("must use unbounded AsyncSpec" in error for error in errors)


def test_verus_shortcut_scan_rejects_assume_admit_and_external_body(
    tmp_path: Path,
) -> None:
    module = load_checker()
    path = tmp_path / "proof.rs"
    source = """
fn bad() { assume(true); admit(); }
#[verifier::external_body]
fn hidden() {}
fn comment_gap() {
    assume/* nested-token gap */(true);
    admit /* gap */ ! /* another gap */ ();
}
#[verifier /* gap */ :: /* gap */ external_body]
fn comment_gapped_hidden() {}
fn harmless() {
    let text = "assume/* string */(true) #[verifier::external_body]";
    // admit/* line comment */();
    /* #[verifier::external_body] */
}
"""

    errors = module.verus_shortcut_errors(path, source)
    assert len(errors) == 6


def test_duplicate_json_keys_are_rejected(tmp_path: Path) -> None:
    module = load_checker()
    path = tmp_path / "ledger.json"
    path.write_text('{"schema_version": 1, "schema_version": 2}', encoding="utf-8")

    with pytest.raises(module.DuplicateKeyError):
        module.load_ledger(path)


def test_checker_cli_has_no_duplicate_option_aliases() -> None:
    module = load_checker()
    aliases = [
        alias
        for action in module._parser()._actions
        for alias in action.option_strings
    ]

    assert len(aliases) == len(set(aliases))


def test_duplicate_obligation_ids_and_unknown_status_are_rejected() -> None:
    module = load_checker()
    ledger = copy.deepcopy(module.load_ledger())
    ledger["obligations"][1]["id"] = ledger["obligations"][0]["id"]
    ledger["obligations"][1]["status"] = "bounded_model_checked"

    errors = module.validate_ledger(ledger).errors
    assert any("duplicate proof obligation id" in error for error in errors)
    assert any("unknown value" in error for error in errors)


def test_checked_in_tool_run_metadata_is_rejected() -> None:
    module = load_checker()
    ledger = copy.deepcopy(module.load_ledger())
    ledger.pop("last_tlaps_run", None)
    ledger["last_tlaps_run"] = {"modules": []}

    errors = module.validate_ledger(ledger).errors
    assert any("tool runs and counts belong only" in error for error in errors)


def test_tlc_runner_cannot_claim_or_mutate_proof_completion() -> None:
    module = load_checker()
    runner = (ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlc.sh").read_text()

    assert "COUNTEREXAMPLE SEARCH ONLY" in runner
    assert "no proof status was changed" in runner
    assert "proof_coverage.json" not in runner
    assert "machine_checked_completion" not in runner
    assert "SumeragiV2ChainEpoch.tla" in runner
    assert "SumeragiV2AsyncNetwork.tla" in runner
    assert "SumeragiV2EffectiveLockAcquisition.tla" in runner
    assert "SumeragiV2ResumeVoteWitness.tla" in runner
    assert '[[ "$tlc_status" -ne 12 ]]' in runner
    assert "Invariant NoRecoveredHistoricalLockedCommitSigning is violated." in runner
    assert "resolve_java.sh" in runner
    assert 'readonly JAVA_BIN="$resolved_java_bin"' in runner
    assert '"$JAVA_BIN" -version' in runner
    assert "simulation_config=1" in runner
    assert 'grep -Ec "^Running Random Simulation with seed ${seed} with 1 worker "' in runner
    assert 'grep -Fxc "Computed 1 initial states..."' in runner
    finish_pattern_match = re.search(
        r"readonly TLC_FINISHED_PATTERN='([^']+)'", runner
    )
    assert finish_pattern_match is not None
    finish_pattern = finish_pattern_match.group(1)
    for accepted_footer in (
        "Finished in 812ms at (2026-07-17 16:30:58)",
        "Finished in 59s at (2026-07-17 16:30:58)",
        "Finished in 01min 05s at (2026-07-17 16:30:58)",
        "Finished in 01h 02min at (2026-07-17 16:30:58)",
        "Finished in 1d 02h 03min 04s at (2026-07-17 16:30:58)",
    ):
        assert subprocess.run(
            ("grep", "-Eq", finish_pattern),
            input=f"{accepted_footer}\n",
            text=True,
            check=False,
        ).returncode == 0
    for rejected_footer in (
        "Finished in  at (2026-07-17 16:30:58)",
        "Finished in 01h 02min  at (2026-07-17 16:30:58)",
        "Finished in 01h 02min at 2026-07-17 16:30:58",
        "Finished in 01h 02min at (2026-07-17 16:30:58) error",
    ):
        assert subprocess.run(
            ("grep", "-Eq", finish_pattern),
            input=f"{rejected_footer}\n",
            text=True,
            check=False,
        ).returncode != 0
    assert 'grep -Ec "$TLC_FINISHED_PATTERN"' in runner
    assert '"$progress_count" -lt 1' in runner
    assert "TLC bounded simulation ${cfg} did not report one exact successful run" in runner
    assert "all exhaustive searches, deterministic simulations, and the recovery witness" in runner
    assert module.REQUIRED_TLC_CONFIG_HEADERS["chain_epoch.cfg"] == (
        "SPECIFICATION ChainEpochTlcSpec"
    )
    assert module.REQUIRED_TLC_CONFIG_HEADERS["liveness.cfg"] == (
        "SPECIFICATION AsyncFiniteSpec"
    )
    assert module.REQUIRED_TLC_CONFIG_HEADERS[
        "effective_lock_acquisition.cfg"
    ] == "SPECIFICATION AcquisitionSpec"
    assert module.REQUIRED_TLC_CONFIG_HEADERS[
        "resume_locked_commit_witness.cfg"
    ] == "SPECIFICATION CoreSpec"
    assert (module.FORMAL_DIR / "chain_epoch.cfg").read_text().startswith(
        "SPECIFICATION ChainEpochTlcSpec\n"
    )
    chain_epoch = (module.FORMAL_DIR / "SumeragiV2ChainEpoch.tla").read_text()
    assert "ChainEpochTlcInit == Init /\\ ChainEpochInit" in chain_epoch
    assert (
        "ChainEpochTlcNext == ChainEpochTlcReceiptNext /\\ UNCHANGED vars"
        in chain_epoch
    )
    assert "ChainEpochTlcVars == <<vars, ChainEpochVars>>" in chain_epoch
    assert (module.FORMAL_DIR / "liveness.cfg").read_text().startswith(
        "SPECIFICATION AsyncFiniteSpec\n"
    )
    assert (
        module.FORMAL_DIR / "effective_lock_acquisition.cfg"
    ).read_text().startswith("SPECIFICATION AcquisitionSpec\n")
    assert (
        module.FORMAL_DIR / "resume_locked_commit_witness.cfg"
    ).read_text().startswith("SPECIFICATION CoreSpec\n")


def test_locked_commit_resume_witness_is_pinned_as_expected_counterexample(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2ResumeVoteWitness.tla",
        "resume_locked_commit_witness.cfg",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)

    assert "SumeragiV2ResumeVoteWitness" in module.REQUIRED_MODEL_MODULES
    assert "resume_locked_commit_witness.cfg" in module.REQUIRED_TLC_CONFIGS
    assert module._resume_vote_witness_errors(formal_dir) == []

    cfg = formal_dir / "resume_locked_commit_witness.cfg"
    cfg.write_text(
        cfg.read_text(encoding="utf-8").replace(
            "INVARIANT NoRecoveredHistoricalLockedCommitSigning",
            "INVARIANT RecoveredHistoricalLockedCommitSigning",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._resume_vote_witness_errors(formal_dir)
    assert any("missing or duplicated" in error for error in errors)

    shutil.copyfile(
        module.FORMAL_DIR / "resume_locked_commit_witness.cfg",
        cfg,
    )
    witness = formal_dir / "SumeragiV2ResumeVoteWitness.tla"
    witness.write_text(
        witness.read_text(encoding="utf-8").replace(
            "  ~RecoveredHistoricalLockedCommitSigning",
            "  RecoveredHistoricalLockedCommitSigning",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._resume_vote_witness_errors(formal_dir)
    assert any("must be exactly the negation" in error for error in errors)


def test_service_rank_replacement_mutation_is_pinned_and_expected_to_fail() -> None:
    runner = (
        ROOT_DIR
        / "scripts"
        / "formal"
        / "run_sumeragi_v2_service_rank_mutation.sh"
    ).read_text(encoding="utf-8")
    assert 'TLA2TOOLS_VERSION="1.7.4"' in runner
    assert (
        'TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"'
        in runner
    )
    assert "old_status -eq 13" in runner
    assert "-fp 96 -seed 139154308881391968" in runner
    assert "Temporal properties were violated." in runner
    assert "Back to state 2" in runner
    assert "deferred_old_status -eq 13" in runner
    assert "old deferred-owner replacement mutation did not fail with TLC status 13" in runner
    assert "Back to state 3" in runner
    assert "deferred_cursor_old_status -eq 13" in runner
    assert "deferred_busy_cursor_old_status -eq 13" in runner
    assert "old strict deferred cursor mutation missed expected marker" in runner
    assert "old Busy deferred cursor mutation missed expected marker" in runner
    assert "busyAttemptParity = TRUE" in runner
    assert "6 distinct states" in runner
    assert "5 distinct states" in runner
    assert "head_only_status -eq 13" in runner
    assert "old head-only ingress mutation did not fail with TLC status 13" in runner
    assert "State 2: Stuttering" in runner
    assert "capacity_old_status -eq 12" in runner
    assert "old ingress capacity removal mutation did not fail with TLC status 12" in runner
    assert "Invariant OldCapacityInvariant is violated." in runner
    assert "completion_capacity_conflated_status -eq 13" in runner
    assert "conflated work/completion capacity mutation missed expected marker" in runner
    assert "completion_capacity_separated.cfg" in runner
    assert "local_admission_producer_first_status -eq 13" in runner
    assert "producer-first local admission mutation missed expected marker" in runner
    assert "local_admission_producer_first_bug.cfg" in runner
    assert "local_admission_alternating.cfg" in runner
    assert "SumeragiV2LocalAdmissionMutation.tla" in runner
    assert "7 distinct states" in runner
    assert "depth of the complete state graph search is 7" in runner
    assert "serve_nonce_reuse_status -eq 13" in runner
    assert "live Serve nonce reuse did not fail with TLC status 13" in runner
    assert "serve_nonce_reuse_bug.cfg" in runner
    assert "serve_nonce_fresh.cfg" in runner
    assert "SumeragiV2ServeNonceMutation.tla" in runner
    assert "4 distinct states" in runner
    assert "depth of the complete state graph search is 3" in runner
    assert "Model checking completed. No error has been found." in runner

    formal_dir = ROOT_DIR / "docs" / "formal" / "sumeragi_v2"
    mutation = (formal_dir / "SumeragiV2ServiceRankMutation.tla").read_text(
        encoding="utf-8"
    )
    assert "EnqueueEqualReplacement" in mutation
    assert "DispatchOldestCopy" in mutation
    assert "AdmitAfterOwnershipEnds" in mutation
    assert "AdmitEqualWhileDeferred" in mutation
    assert "CoalesceEqualWhileDeferred" in mutation
    assert "DeferredReplacementRankProgress" in mutation
    assert (formal_dir / "service_rank_replacement_bug.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION OldSpec\n")
    assert (formal_dir / "service_rank_coalesced.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION CoalescedSpec\n")
    assert (formal_dir / "service_rank_deferred_replacement_bug.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION DeferredReplacementOldSpec\n")
    assert (formal_dir / "service_rank_deferred_coalesced.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION DeferredReplacementCoalescedSpec\n")
    cursor_mutation = (formal_dir / "SumeragiV2DeferredCursorMutation.tla").read_text(
        encoding="utf-8"
    )
    assert "OldStrictService" in cursor_mutation
    assert "CyclicService" in cursor_mutation
    assert "ProgressEventuallyServiced == progressOwned ~> ~progressOwned" in cursor_mutation
    assert (formal_dir / "deferred_cursor_strict_bug.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION OldStrictSpec\n")
    assert (formal_dir / "deferred_cursor_cyclic.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION CyclicSpec\n")
    busy_cursor_mutation = (
        formal_dir / "SumeragiV2DeferredBusyCursorMutation.tla"
    ).read_text(encoding="utf-8")
    assert "OldBusyService" in busy_cursor_mutation
    assert "CyclicBusyService" in busy_cursor_mutation
    assert "busyAttemptParity' = ~busyAttemptParity" in busy_cursor_mutation
    assert (formal_dir / "deferred_busy_cursor_strict_bug.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION OldBusySpec\n")
    assert (formal_dir / "deferred_busy_cursor_cyclic.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION CyclicBusySpec\n")
    ingress_mutation = (formal_dir / "SumeragiV2IngressMutation.tla").read_text(
        encoding="utf-8"
    )
    assert "OldHeadDrain" in ingress_mutation
    assert "FirstProgressIndex" in ingress_mutation
    assert "SequenceWithoutIndex(lane, FirstProgressIndex)" in ingress_mutation
    assert (formal_dir / "ingress_head_blocking_bug.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION OldSpec\n")
    assert (formal_dir / "ingress_indexed_scan.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION IndexedSpec\n")
    capacity_mutation = (
        formal_dir / "SumeragiV2IngressCapacityMutation.tla"
    ).read_text(encoding="utf-8")
    assert "OldCapacityInvariant" in capacity_mutation
    assert "Len(lane) <= Capacity" in capacity_mutation
    assert "OldInit == lane = <<Progress, Auxiliary, Auxiliary>>" in capacity_mutation
    assert (formal_dir / "ingress_capacity_removal_bug.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION OldSpec\n")
    assert (formal_dir / "ingress_capacity_lane_bound.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION BoundedSpec\n")
    completion_capacity_mutation = (
        formal_dir / "SumeragiV2CompletionCapacityMutation.tla"
    ).read_text(encoding="utf-8")
    assert (
        r"ConflatedNext == AdmitWithConflatedCapacity \/ Tick"
        in completion_capacity_mutation
    )
    assert (
        r"SeparatedNext == AdmitWithSeparatedCapacity \/ Tick"
        in completion_capacity_mutation
    )
    assert "RequiredCompletionEventuallyOwnsWork" in completion_capacity_mutation
    assert (formal_dir / "completion_capacity_conflated_bug.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION ConflatedSpec\n")
    assert (formal_dir / "completion_capacity_separated.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION SeparatedSpec\n")
    local_admission_mutation = (
        formal_dir / "SumeragiV2LocalAdmissionMutation.tla"
    ).read_text(encoding="utf-8")
    assert "FairSelectedSource" in local_admission_mutation
    assert "BuggySelectedSource" in local_admission_mutation
    assert "causalAdmissionOwed" in local_admission_mutation
    assert "CausalAdmissionProgress ==" in local_admission_mutation
    assert (formal_dir / "local_admission_producer_first_bug.cfg").read_text(
        encoding="utf-8"
    ).startswith("CONSTANT FairSelection = FALSE\n")
    assert (formal_dir / "local_admission_alternating.cfg").read_text(
        encoding="utf-8"
    ).startswith("CONSTANT FairSelection = TRUE\n")
    causal_replacement_mutation = (
        formal_dir / "SumeragiV2CausalReplacementMutation.tla"
    ).read_text(encoding="utf-8")
    assert "BlindExecuteChunkParent" in causal_replacement_mutation
    assert "CoalescedExecuteChunkParent" in causal_replacement_mutation
    assert (
        "IF CandidateOwned THEN causalCopy ELSE TRUE"
        in causal_replacement_mutation
    )
    assert "RankProgress ==" in causal_replacement_mutation
    assert (formal_dir / "causal_replacement_bug.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION OldSpec\n")
    assert (formal_dir / "causal_replacement_coalesced.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION CoalescedSpec\n")
    causal_fifo_rank_mutation = (
        formal_dir / "SumeragiV2CausalFifoRankMutation.tla"
    ).read_text(encoding="utf-8")
    assert (
        "RankMultiplier * CandidateSequenceIndex(candidate, causalQueue)"
        in causal_fifo_rank_mutation
    )
    assert 'preferredLocalSource\' = "Producer"' in causal_fifo_rank_mutation
    assert (
        "earlierHeadRemoved => TargetRank < InitialTargetRank"
        in causal_fifo_rank_mutation
    )
    assert (formal_dir / "causal_fifo_rank_multiplier_one_bug.cfg").read_text(
        encoding="utf-8"
    ).startswith("CONSTANT RankMultiplier = 1\n")
    assert (formal_dir / "causal_fifo_rank_doubled.cfg").read_text(
        encoding="utf-8"
    ).startswith("CONSTANT RankMultiplier = 2\n")
    serve_nonce_mutation = (
        formal_dir / "SumeragiV2ServeNonceMutation.tla"
    ).read_text(encoding="utf-8")
    assert "LiveNonceOwnership" in serve_nonce_mutation
    assert "CorrectBinderCoversRecord" in serve_nonce_mutation
    assert "CorrectBinderHasRecordInstance" in serve_nonce_mutation
    assert "OldNext == Refill(TargetJob) \\/ Service" in serve_nonce_mutation
    assert (
        "FreshNext == (TargetOwned /\\ Refill(FreshJob)) \\/ Service"
        in serve_nonce_mutation
    )
    assert "TargetEventuallyLeaves == TargetOwned ~> ~TargetOwned" in serve_nonce_mutation
    assert (formal_dir / "serve_nonce_reuse_bug.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION OldSpec\n")
    fresh_nonce_config = (formal_dir / "serve_nonce_fresh.cfg").read_text(
        encoding="utf-8"
    )
    assert fresh_nonce_config.startswith("SPECIFICATION FreshSpec\n")
    assert "INVARIANT LiveNonceOwnership\n" in fresh_nonce_config
    assert "INVARIANT CorrectBinderCoversRecord\n" in fresh_nonce_config
    assert "INVARIANT CorrectBinderHasRecordInstance\n" in fresh_nonce_config

    progress_runner = (
        ROOT_DIR
        / "scripts"
        / "formal"
        / "run_sumeragi_v2_progress_mutations.sh"
    ).read_text(encoding="utf-8")
    assert 'TLA2TOOLS_VERSION="1.7.4"' in progress_runner
    assert "resolve_java.sh" in progress_runner
    assert "causal_debt_completion_bug.cfg 13" in progress_runner
    assert "causal_debt_completion_fixed.cfg 0" in progress_runner
    assert "causal_debt_duplicate_fixed.cfg 0" in progress_runner
    assert "causal_replacement_bug.cfg 13" in progress_runner
    assert "causal_replacement_coalesced.cfg 0" in progress_runner
    assert "causal_fifo_rank_multiplier_one_bug.cfg 12" in progress_runner
    assert "causal_fifo_rank_doubled.cfg 0" in progress_runner
    assert (
        "Invariant EarlierHeadRemovalStrictlyDropsTargetRank is violated."
        in progress_runner
    )
    assert "State 2: <RemoveEarlierHead" in progress_runner
    assert "discovery_debt_bug.cfg 13" in progress_runner
    assert "discovery_debt_fixed.cfg 0" in progress_runner
    assert "io_candidate_index_all_jobs_bug.cfg 12" in progress_runner
    assert "io_candidate_index_consensus_only.cfg 0" in progress_runner
    assert "successor_stale_token_bug.cfg 12" in progress_runner
    assert "successor_stale_token_fixed.cfg 0" in progress_runner
    assert (
        "Invariant SuccessorActivationProtocolInvariantProjection is violated."
        in progress_runner
    )
    assert (
        "2 states generated, 2 distinct states found, 0 states left on queue."
        in progress_runner
    )
    assert "effective_lock_rebind_fixed.cfg 0" in progress_runner
    assert "effective_lock_rebind_bug.cfg 12" in progress_runner
    assert "effective_lock_no_retry_bug.cfg 13" in progress_runner
    assert "effective_lock_future_completion_bug.cfg 12" in progress_runner
    assert "ownership_n1.cfg 0" in progress_runner
    assert "42817 states generated, 6208 distinct states found" in progress_runner
    assert "depth of the complete state graph search is 45" in progress_runner

    causal_debt = (formal_dir / "SumeragiV2CausalDebtMutation.tla").read_text(
        encoding="utf-8"
    )
    assert "TypeInvariant ==" in causal_debt
    assert 'producerReady = (Scenario \\in {"ProducerRefill", "Completion"})' in causal_debt
    assert "IF outstanding > 0 THEN outstanding - 1 ELSE 0" in causal_debt
    for config in formal_dir.glob("causal_debt_*.cfg"):
        assert "INVARIANT TypeInvariant" in config.read_text(encoding="utf-8")
    assert "FreshCommandSuccessors" in (
        formal_dir / "SumeragiV2AsyncNetwork.tla"
    ).read_text(encoding="utf-8")
    assert "FixedDiscoveryPrefix" in (
        formal_dir / "SumeragiV2DiscoveryDebtMutation.tla"
    ).read_text(encoding="utf-8")
    assert "ConsensusTargetIndices" in (
        formal_dir / "SumeragiV2IoCandidateIndexMutation.tla"
    ).read_text(encoding="utf-8")
    acquisition_mutation = (
        formal_dir / "SumeragiV2EffectiveLockAcquisitionMutation.tla"
    ).read_text(encoding="utf-8")
    assert "BuggyRebindSameLock" in acquisition_mutation
    assert "NoRetrySpec" in acquisition_mutation
    assert "BuggyFutureCompletionFailsClosed" in acquisition_mutation
    ownership = (formal_dir / "SumeragiV2OwnershipInvariantCheck.tla").read_text(
        encoding="utf-8"
    )
    assert "OwnershipBoundedSpec" in ownership
    assert "OwnershipInitialClock" in ownership


@pytest.mark.parametrize(
    ("symbol", "correct", "grouped_mutation"),
    (
        (
            "FairProtectedStage5RankDescent",
            "THEOREM FairProtectedStage5RankDescent ==\n"
            "  \\A initialContext, candidate:\n    \\A position \\in Nat:",
            "THEOREM FairProtectedStage5RankDescent ==\n"
            "  \\A initialContext, candidate, position \\in Nat:",
        ),
        (
            "FairProtectedStage4RankDescent",
            "THEOREM FairProtectedStage4RankDescent ==\n"
            "  \\A initialContext, candidate:\n    \\A position \\in Nat:",
            "THEOREM FairProtectedStage4RankDescent ==\n"
            "  \\A initialContext, candidate, position \\in Nat:",
        ),
        (
            "FairStage4AuxOneStep",
            "THEOREM FairStage4AuxOneStep ==\n"
            "  \\A initialContext, candidate, position:\n"
            "    \\A rank \\in ReadyRunAuxCarrier:",
            "THEOREM FairStage4AuxOneStep ==\n"
            "  \\A initialContext, candidate, position,\n"
            "     rank \\in ReadyRunAuxCarrier:",
        ),
        (
            "FairStage4CapacityOneStep",
            "THEOREM FairStage4CapacityOneStep ==\n"
            "  \\A initialContext, candidate, position:\n"
            "    \\A rank \\in Stage4CapacityCarrier:",
            "THEOREM FairStage4CapacityOneStep ==\n"
            "  \\A initialContext, candidate, position,\n"
            "     rank \\in Stage4CapacityCarrier:",
        ),
        (
            "FairProtectedServeStage5RankDescent",
            "THEOREM FairProtectedServeStage5RankDescent ==\n"
            "  \\A initialContext, node, job:\n    \\A position \\in Nat:",
            "THEOREM FairProtectedServeStage5RankDescent ==\n"
            "  \\A initialContext, node, job, position \\in Nat:",
        ),
    ),
)
def test_service_rank_record_binders_cannot_be_grouped_into_rank_carrier(
    tmp_path: Path,
    symbol: str,
    correct: str,
    grouped_mutation: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    proof = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = proof.read_text(encoding="utf-8")
    assert source.count(correct) >= 1
    proof.write_text(source.replace(correct, grouped_mutation, 1), encoding="utf-8")

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        symbol in error and "record-valued" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("correct", "grouped_mutation"),
    (
        (
            "THEOREM EmptyIngressIndexedPairSet ==\n"
            "  \\A sources:\n    \\A capacity \\in Nat:",
            "THEOREM EmptyIngressIndexedPairSet ==\n"
            "  \\A sources, capacity \\in Nat:",
        ),
        (
            "THEOREM AsyncRecoveryRequiredAtBudgetLeadsLowerCycle ==\n"
            "  \\A initialContext:\n    \\A budget \\in Nat:",
            "THEOREM AsyncRecoveryRequiredAtBudgetLeadsLowerCycle ==\n"
            "  \\A initialContext, budget \\in Nat:",
        ),
        (
            "THEOREM ProtectedRankExitHasWellFoundedSuccessor ==\n"
            "  \\A candidate:\n"
            "    \\A rank \\in OwnedServiceRankCarrier:",
            "THEOREM ProtectedRankExitHasWellFoundedSuccessor ==\n"
            "  \\A candidate, rank \\in OwnedServiceRankCarrier:",
        ),
        (
            "THEOREM Stage4LocalAdmissionDecreasesAux ==\n"
            "  \\A candidate, position:\n"
            "    \\A rank \\in ReadyRunAuxCarrier:",
            "THEOREM Stage4LocalAdmissionDecreasesAux ==\n"
            "  \\A candidate, position, rank \\in ReadyRunAuxCarrier:",
        ),
        (
            "THEOREM Stage4CapacityLocalAdmissionStrictlyProgresses ==\n"
            "  \\A candidate, position:\n"
            "    \\A rank \\in Stage4CapacityCarrier:",
            "THEOREM Stage4CapacityLocalAdmissionStrictlyProgresses ==\n"
            "  \\A candidate, position, rank \\in Stage4CapacityCarrier:",
        ),
    ),
)
def test_supporting_rank_proofs_reject_heterogeneous_bounded_groups(
    tmp_path: Path,
    correct: str,
    grouped_mutation: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    proof = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = proof.read_text(encoding="utf-8")
    assert source.count(correct) == 1, correct
    proof.write_text(source.replace(correct, grouped_mutation, 1), encoding="utf-8")

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "heterogeneous grouped bounded quantifier" in error
        for error in errors
    ), errors


def test_scheduler_starvation_composition_requires_both_rank_properties(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    proof = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = proof.read_text(encoding="utf-8")
    correct = (
        "THEOREM ProtectedServiceRankProgressImpliesStarvation ==\n"
        "  \\A initialContext:\n"
        "    /\\ AsyncSpecAt(initialContext)\n"
        "    /\\ ProtectedServiceRanksProgressProperty("
        "AsyncSpecAt(initialContext))"
    )
    candidate_only = correct.replace(
        "ProtectedServiceRanksProgressProperty",
        "ProtectedServiceRankProgressProperty",
    )
    assert source.count(correct) == 1
    proof.write_text(source.replace(correct, candidate_only, 1), encoding="utf-8")

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ProtectedServiceRankProgressImpliesStarvation" in error
        and "complete rank-progress premise" in error
        for error in errors
    ), errors


def test_serve_starvation_composition_requires_natural_rank_induction(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    proof = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = proof.read_text(encoding="utf-8")
    correct = "BY <3>1, NatLessThanWellFounded, WellFoundedLeadsTo"
    weakened = "BY <3>1, OwnedServiceRankOrderingWellFounded, WellFoundedLeadsTo"
    assert source.count(correct) == 1
    proof.write_text(source.replace(correct, weakened, 1), encoding="utf-8")

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ProtectedServeRankProgressImpliesStarvation" in error
        and "NatLessThanWellFounded" in error
        for error in errors
    ), errors


def test_tlc_configs_keep_an_externally_invalid_subject(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    for cfg_name in module.REQUIRED_TLC_CONFIGS:
        if cfg_name == "effective_lock_acquisition.cfg":
            assert (
                "AcquisitionSubjects = "
                "{AcquisitionSubjectA, AcquisitionSubjectB}\n"
                in (formal_dir / cfg_name).read_text(encoding="utf-8")
            )
            continue
        assert '  ValidSubjects = {"A"}\n' in (formal_dir / cfg_name).read_text(
            encoding="utf-8"
        )

    target = formal_dir / "liveness.cfg"
    target.write_text(
        target.read_text(encoding="utf-8").replace(
            '  ValidSubjects = {"A"}',
            '  ValidSubjects = {"A", "B"}',
            1,
        ),
        encoding="utf-8",
    )
    errors = module.validate_ledger(
        module.load_ledger(),
        formal_dir=formal_dir,
        check_retired_paths=False,
    ).errors
    assert any("must keep B externally invalid" in error for error in errors)


def test_candidate_restart_mutations_are_pinned_and_expected_to_fail() -> None:
    formal_dir = ROOT_DIR / "docs" / "formal" / "sumeragi_v2"
    runner = (
        ROOT_DIR
        / "scripts"
        / "formal"
        / "run_sumeragi_v2_candidate_restart_mutation.sh"
    ).read_text(encoding="utf-8")
    assert 'TLA2TOOLS_VERSION="1.7.4"' in runner
    assert (
        'TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"'
        in runner
    )
    assert "-fp 96 -seed 139154308881391968" in runner
    assert "rc -eq 12" in runner
    for marker in (
        "ChangedConsumerViewNotCoalesced",
        "StaleGenerationNotCoalesced",
        "ChangedEvidenceNotCoalesced",
        "ChangedWorkNotCoalesced",
        "ChangedBodyNotCoalesced",
        "ChangedManifestNotCoalesced",
        "ChangedCommitmentNotCoalesced",
        "ExactCandidateAdmitted",
        "VolatileSignatureProgressWitness",
        "DurableWorkHasReplayOrRecovery",
        "NoStaleCompletion",
        "OuterProgressClassAligned",
        "RuntimeProgressClassAligned",
    ):
        assert marker in runner
    for config in (
        "candidate_identity_exact.cfg",
        "candidate_identity_changed_consumer_view_bug.cfg",
        "candidate_identity_stale_generation_bug.cfg",
        "candidate_identity_changed_evidence_bug.cfg",
        "candidate_identity_changed_work_bug.cfg",
        "candidate_identity_changed_body_bug.cfg",
        "candidate_identity_changed_manifest_bug.cfg",
        "candidate_identity_changed_commitment_bug.cfg",
        "candidate_identity_broad_projection_bug.cfg",
        "crash_replay_signature_fixed.cfg",
        "crash_replay_body_fixed.cfg",
        "crash_replay_application_fixed.cfg",
        "crash_replay_signature_volatile_bug.cfg",
        "crash_replay_signature_drop_bug.cfg",
        "crash_replay_body_drop_bug.cfg",
        "crash_replay_application_drop_bug.cfg",
        "crash_replay_stale_completion_bug.cfg",
        "ingress_class_repaired.cfg",
        "ingress_class_outer_timeout_drop_bug.cfg",
        "ingress_class_outer_certified_drop_bug.cfg",
        "ingress_class_outer_commit_drop_bug.cfg",
        "ingress_class_runtime_timeout_drop_bug.cfg",
        "ingress_class_runtime_certified_promotion_bug.cfg",
        "ingress_class_runtime_commit_promotion_bug.cfg",
    ):
        assert config in runner
        if config.startswith("crash_replay_"):
            config_source = (formal_dir / config).read_text(encoding="utf-8")
            assert "INVARIANT AsyncRecoveryTypeInvariant\n" in config_source
            assert "INVARIANT AsyncRestartAuthorityInvariant\n" in config_source
    assert "INVARIANT CrashAwareSignatureProgressWitness\n" in (
        formal_dir / "crash_replay_signature_fixed.cfg"
    ).read_text(encoding="utf-8")
    assert "INVARIANT VolatileSignatureProgressWitness\n" in (
        formal_dir / "crash_replay_signature_volatile_bug.cfg"
    ).read_text(encoding="utf-8")
    assert "39 mutants failed their named invariants" in runner
    ingress_mutation = (formal_dir / "SumeragiV2IngressClassMutation.tla").read_text(
        encoding="utf-8"
    )
    assert "RequiredOuterProgressKinds" in ingress_mutation
    assert "RuntimeProgressKinds" in ingress_mutation


def test_formal_gate_validates_fresh_evidence_before_tlc_and_replay() -> None:
    source = (ROOT_DIR / "ci" / "check_sumeragi_formal.sh").read_text()

    structural = source.index("check_sumeragi_v2_proof_ledger.py")
    tlaps = source.index("run_sumeragi_v2_tlaps.sh")
    release = source.index("--release")
    mutations = source.index("run_sumeragi_v2_service_rank_mutation.sh")
    productive_mutations = source.index("run_sumeragi_v2_productive_mutation.sh")
    candidate_restart = source.index(
        "run_sumeragi_v2_candidate_restart_mutation.sh"
    )
    progress_mutations = source.index("run_sumeragi_v2_progress_mutations.sh")
    effect_capacity_mutations = source.index(
        "run_sumeragi_v2_effect_capacity_ownership_mutation.sh"
    )
    tlc = source.index("run_sumeragi_v2_tlc.sh")
    replay = source.index("check_sumeragi_v2_replay_trace.sh")
    verus = source.index("verify_sumeragi_v2.sh")
    final_release = source.rindex("--release")
    final_marker = source.index("Sumeragi v2 formal gate passed")
    assert (
        structural
        < tlaps
        < release
        < mutations
        < productive_mutations
        < candidate_restart
        < progress_mutations
        < effect_capacity_mutations
        < tlc
        < replay
        < verus
        < final_release
        < final_marker
    )
    assert "proof_evidence.json" in source

    verus_source = (ROOT_DIR / "scripts" / "verify_sumeragi_v2.sh").read_text()
    unit = verus_source.index("--unit")
    fast_network = verus_source.index("--fast-network")
    backend = verus_source.index("cargo verus verify")
    assert unit < fast_network < backend


def test_nightly_chaos_cold_cache_prefetch_is_pinned_and_fail_closed(
    tmp_path: Path,
) -> None:
    module = load_checker()
    relative_paths = (
        Path("scripts/formal/run_sumeragi_v2_harness.sh"),
        Path("scripts/formal/sumeragi_v2_harness.lock"),
        Path("scripts/run_sumeragi_v2_100k_chaos.sh"),
        Path(".github/workflows/nightly_sumeragi_formal.yml"),
    )
    paths: dict[Path, Path] = {}
    sources: dict[Path, str] = {}
    for relative in relative_paths:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)
        paths[relative] = destination
        sources[relative] = destination.read_text(encoding="utf-8")

    assert module._nightly_chaos_cold_cache_errors(tmp_path) == []

    harness = Path("scripts/formal/run_sumeragi_v2_harness.sh")
    launcher = Path("scripts/run_sumeragi_v2_100k_chaos.sh")
    workflow = Path(".github/workflows/nightly_sumeragi_formal.yml")
    mutations = (
        (
            harness,
            "  export CARGO_NET_OFFLINE=false\n",
            "  export CARGO_NET_OFFLINE=true\n",
            "only --fetch may run online",
        ),
        (
            harness,
            "    cargo fetch --locked\n",
            "    cargo fetch --locked --offline\n",
            "exactly one online `cargo fetch --locked`",
        ),
        (
            harness,
            'cp -- "$HARNESS_LOCK" Cargo.lock\n',
            'cp -- "$REPO_ROOT/Cargo.lock" Cargo.lock\n',
            "verified standalone lock must be copied",
        ),
        (
            harness,
            'readonly HARNESS_LOCK_SHA256="9c49a60551d9f66c8786f2497cb107fb3214fb3420c4f5c23ba3d24814b3f97e"',
            'readonly HARNESS_LOCK_SHA256="0000000000000000000000000000000000000000000000000000000000000000"',
            "pinned standalone lock digest disagrees",
        ),
        (
            harness,
            '    readonly ignored_test="accelerated_100_000_block_chaos_preserves_chain_prefix"\n'
            '    ignored_test_list="$(\n'
            "      cargo test --locked --offline -p iroha_sumeragi_core \\\n"
            "        --test network_simulation -- --list --ignored",
            '    readonly ignored_test="accelerated_100_000_block_chaos_preserves_chain_prefix"\n'
            '    ignored_test_list="$(\n'
            "      cargo test --locked -p iroha_sumeragi_core \\\n"
            "        --test network_simulation -- --list --ignored",
            "inventory and execution must both remain --locked --offline",
        ),
        (
            launcher,
            "bash scripts/formal/run_sumeragi_v2_harness.sh --chaos-100k \\\n",
            "bash scripts/formal/run_sumeragi_v2_harness.sh --fast-network \\\n",
            "offline harness gate exactly once",
        ),
        (
            workflow,
            "      - name: Prefetch pinned standalone harness dependencies\n"
            "        run: bash scripts/formal/run_sumeragi_v2_harness.sh --fetch\n",
            "",
            "exactly one cache, pinned prefetch, and source-attested gate",
        ),
        (
            workflow,
            "      - name: Prefetch pinned standalone harness dependencies\n"
            "        run: bash scripts/formal/run_sumeragi_v2_harness.sh --fetch\n"
            "      - name: Sumeragi v2 source-attested 100,000-height chaos gate\n"
            "        run: bash scripts/run_sumeragi_v2_100k_chaos.sh\n",
            "      - name: Sumeragi v2 source-attested 100,000-height chaos gate\n"
            "        run: bash scripts/run_sumeragi_v2_100k_chaos.sh\n"
            "      - name: Prefetch pinned standalone harness dependencies\n"
            "        run: bash scripts/formal/run_sumeragi_v2_harness.sh --fetch\n",
            "nightly --fetch must run after cache restore and before",
        ),
    )
    for relative, needle, replacement, expected_error in mutations:
        source = sources[relative]
        assert needle in source, (relative, needle)
        paths[relative].write_text(
            source.replace(needle, replacement, 1), encoding="utf-8"
        )
        errors = module._nightly_chaos_cold_cache_errors(tmp_path)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        paths[relative].write_text(source, encoding="utf-8")


def test_release_corridor_rejects_network_skips_and_zero_test_filters() -> None:
    seed_source = (
        ROOT_DIR / "scripts" / "run_sumeragi_v2_seed_matrix.sh"
    ).read_text(encoding="utf-8")
    release_source = (
        ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh"
    ).read_text(encoding="utf-8")
    harness_source = (
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_harness.sh"
    ).read_text(encoding="utf-8")
    formal_launcher_source = (
        ROOT_DIR / "scripts" / "run_sumeragi_v2_formal_release.sh"
    ).read_text(encoding="utf-8")
    receipt_source = (
        ROOT_DIR / "scripts" / "write_sumeragi_v2_release_receipt.py"
    ).read_text(encoding="utf-8")
    taira_source = (
        ROOT_DIR / "integration_tests" / "tests" / "taira_public_localnet.rs"
    ).read_text(encoding="utf-8")
    integration_runner_source = (
        ROOT_DIR / "integration_tests" / "tests" / "sumeragi_v2_runner.rs"
    ).read_text(encoding="utf-8")
    lane_work_source = (
        ROOT_DIR / "crates" / "iroha_core" / "src" / "sumeragi" / "v2_lane_work.rs"
    ).read_text(encoding="utf-8")
    runner_source = (
        ROOT_DIR / "crates" / "iroha_core" / "src" / "sumeragi" / "v2_runner.rs"
    ).read_text(encoding="utf-8")
    kura_source = (ROOT_DIR / "crates" / "iroha_core" / "src" / "kura.rs").read_text(
        encoding="utf-8"
    )
    lane_geometry_source = (
        ROOT_DIR / "crates" / "iroha_core" / "src" / "kura" / "lane_geometry.rs"
    ).read_text(encoding="utf-8")
    liveness_doc = (
        ROOT_DIR / "docs" / "source" / "sumeragi_v2_liveness.md"
    ).read_text(encoding="utf-8")

    runner_path = ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh"
    bash = Path(shutil.which("bash") or "").resolve(strict=True)
    for sealed_value in ("0", "1"):
        direct_environment = {
            "HOME": os.environ.get("HOME", str(ROOT_DIR)),
            "IROHA_RELEASE_SEALED_WORKTREE": sealed_value,
            "PATH": os.defpath,
        }
        direct = subprocess.run(
            [str(bash), str(runner_path), "--release"],
            cwd=ROOT_DIR,
            env=direct_environment,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=10,
            check=False,
        )
        assert direct.returncode != 0
        assert (
            "production release requires matching bootstrap path aliases"
            in direct.stderr
        )

    assert "export IROHA_TEST_REQUIRE_NETWORK=1" in seed_source
    assert "export IROHA_TEST_NETWORK_START_ATTEMPTS=1" in seed_source
    assert "-- --list --ignored" in seed_source
    assert "expected exactly one release test named" in seed_source
    assert 'compute_workspace_source_manifest.py --root "$repo_root"' in seed_source
    assert ".seed-matrix.lock" in seed_source
    assert "COMPLETED.tsv" in seed_source

    platform_case = release_source[
        release_source.index('case "$(uname -s)-$(uname -m)" in') :
        release_source.index("  esac", release_source.index('case "$(uname -s)-$(uname -m)" in'))
    ]
    assert platform_case.count("Darwin-arm64)") == 1
    assert platform_case.count("Linux-x86_64)") == 1
    assert "Windows" not in platform_case
    assert "UnsupportedValidatorStoragePlatform" in lane_work_source
    assert "require_validator_storage_platform(" in lane_work_source
    assert "sumeragi_v2_validator_storage_supported()" in lane_work_source
    assert 'cfg!(any(target_os = "linux", target_os = "macos"))' in kura_source
    run_inner = runner_source[runner_source.index("fn run_inner(") :]
    runner_platform_gate = run_inner.index("    require_validator_storage_platform(")
    runner_gate_end = run_inner.index("    )?;", runner_platform_gate)
    runner_gate = run_inner[runner_platform_gate:runner_gate_end]
    assert "config.role == NodeRole::Validator" in runner_gate
    for side_effect in (
        "output_guard\n        .begin_fail_stop_operation()",
        "recover_active_height(",
        "let wal_path =",
        "SumeragiV2Adapter::open",
        "ProductionV2Services::start",
        "V2LaneWorkAdapter::new_with_output_guard",
    ):
        assert runner_platform_gate < run_inner.index(side_effect)
    lane_constructor = lane_work_source[
        lane_work_source.index("    pub(crate) fn new_with_output_guard(") :
    ]
    lane_platform_gate = lane_constructor.index(
        "        require_validator_storage_platform("
    )
    for side_effect in (
        "begin_fail_stop_operation()",
        "context\n            .validate()",
        "MergeSigningGuard::open_with_committed_frontier",
        "NativeAmxSigningGuard::open",
    ):
        assert lane_platform_gate < lane_constructor.index(side_effect)
    assert "local_validator.is_some()," in run_inner
    assert "(NodeRole::Observer, _) => Ok(None)" in runner_source
    assert "let fixed_progress_pairs: [(&Path, &Path, &str); 6]" in lane_geometry_source
    assert "recovery_directory = refreshed_directory;" in lane_geometry_source
    assert (
        "BoundProgressRecoveryFailure::RetryableIo => ErrorKind::WouldBlock"
        in lane_geometry_source
    )
    assert (
        "BoundProgressRecoveryFailure::InvalidData => ErrorKind::InvalidData"
        in lane_geometry_source
    )
    new_production_inventory_additions = (
        (
            "kura::lane_geometry::tests::",
            "first_release_retirement_classifies_recovery_sync_failure_as_retryable",
            lane_geometry_source,
        ),
        (
            "kura::lane_geometry::tests::",
            "first_release_retirement_discards_unpublished_temp_for_every_fixed_pair",
            lane_geometry_source,
        ),
        (
            "kura::lane_geometry::tests::",
            "first_release_retirement_promotes_then_rejects_complete_autonomous_rewrite",
            lane_geometry_source,
        ),
        (
            "kura::lane_geometry::tests::",
            "first_release_retirement_recovers_complete_certified_rewrite_before_snapshot",
            lane_geometry_source,
        ),
        (
            "kura::lane_geometry::tests::",
            "first_release_retirement_recovery_rejects_temp_symlink_without_external_writes",
            lane_geometry_source,
        ),
        (
            "kura::lane_geometry::tests::",
            "first_release_retirement_rejects_directory_substitution_at_pair_refresh",
            lane_geometry_source,
        ),
        (
            "sumeragi::v2_lane_work::tests::",
            "validator_storage_platform_gate_rejects_voters_and_allows_observers",
            lane_work_source,
        ),
        (
            "sumeragi::v2_runner::tests::",
            "unsupported_storage_platform_rejects_runner_voter_and_admits_observer",
            runner_source,
        ),
        (
            "sumeragi_v2_runner::prepare_qc_split_tests::",
            "distinct_prepare_qc_view_zero_wait_covers_deadline_without_masking_view_one",
            integration_runner_source,
        ),
    )
    for _, test_name, source in new_production_inventory_additions:
        assert f"fn {test_name}(" in source
    normalized_liveness_doc = re.sub(r"\s+", " ", liveness_doc.lower())
    assert (
        "other platforms are restricted to non-voting observer or development use"
        in normalized_liveness_doc
    )
    assert (
        "complete observer application and lane-retirement behavior there is not release-certified"
        in normalized_liveness_doc
    )
    assert (
        "no unsupported-platform validator release receipt may be emitted"
        in normalized_liveness_doc
    )

    bootstrap_validation = release_source.index(
        "validate_sumeragi_v2_release_bootstrap.py"
    )
    required_network = release_source.index("export IROHA_TEST_REQUIRE_NETWORK=1")
    production_units = release_source.index("required_production_liveness_tests")
    taira_rust_contracts = release_source.index(
        "required_taira_release_contract_tests=("
    )
    source_contract_preflight = release_source.index(
        "source_manifest_contract_tests=("
    )
    seed_launcher_preflight = release_source.index("seed_launcher_contract_tests=(")
    chaos_launcher_preflight = release_source.index(
        "chaos_launcher_contract_files=("
    )
    receipt_contract_preflight = release_source.index(
        "release_receipt_contract_files=("
    )
    proof_fidelity_preflight = release_source.index(
        "proof_fidelity_contract_files=("
    )
    formal_launcher_preflight = release_source.index(
        "formal_launcher_contract_files=("
    )
    taira_soak_preflight = release_source.index("taira_soak_contract_files=(")
    seed_matrix = release_source.index("run_sumeragi_v2_seed_matrix.sh")
    pr_branch = release_source.index('if [[ "$profile" == "--pr" ]]; then')
    pr_fast_formal = release_source.index(
        "run_sumeragi_v2_harness.sh --unit", pr_branch
    )
    formal_gate = release_source.index("run_sumeragi_v2_formal_release.sh")
    chaos_gate = release_source.index("run_sumeragi_v2_100k_chaos.sh")
    pre_soak_manifest = release_source.index("pre_soak_source_manifest_sha256")
    taira_run = release_source.index("run_taira_v2_24h_soak.sh")
    final_manifest = release_source.index("final_release_source_manifest_sha256")
    final_proof_check = release_source.index(
        "check_sumeragi_v2_proof_ledger.py \\\n  --release \\\n  --evidence"
    )
    aggregate_receipt = release_source.index(
        "write_sumeragi_v2_release_receipt.py"
    )
    assert (
        bootstrap_validation
        < required_network
        < production_units
        < taira_rust_contracts
        < source_contract_preflight
        < seed_launcher_preflight
        < chaos_launcher_preflight
        < receipt_contract_preflight
        < proof_fidelity_preflight
        < formal_launcher_preflight
        < taira_soak_preflight
        < formal_gate
        < seed_matrix
        < chaos_gate
        < pre_soak_manifest
        < taira_run
        < final_manifest
        < final_proof_check
        < aggregate_receipt
    )
    assert seed_matrix < pr_branch < pr_fast_formal
    assert "/tmp/iroha-sumeragi-v2-release-host-" not in release_source
    assert "IROHA_RELEASE_AGGREGATE_RECEIPT_PATH_FILE" not in release_source
    assert 'release_invocation_root="${release_bootstrap_evidence_dir}/release-runner"' in release_source
    assert '--bootstrap-completion "$SUMERAGI_V2_RELEASE_BOOTSTRAP_COMPLETION"' in release_source
    assert '--expected-bootstrap-completion-sha256' in release_source
    assert '--bootstrap-candidate-root "$IROHA_RELEASE_BOOTSTRAP_CANDIDATE_ROOT"' in release_source
    assert '--bootstrap-runner "$IROHA_RELEASE_BOOTSTRAP_RUNNER"' in release_source
    assert 'mv -- "$release_receipt_partial"' not in release_source
    production_inventory_end = release_source.index("\n)", production_units)
    production_inventory = tuple(
        line.strip()
        for line in release_source[
            production_units:production_inventory_end
        ].splitlines()
        if line.strip().startswith(("sumeragi::", "sumeragi_v2_runner::", "kura::"))
    )
    assert len(production_inventory) == 204
    assert len(set(production_inventory)) == 204
    for module, test_name, _ in new_production_inventory_additions:
        assert f"{module}{test_name}" in production_inventory
    for required_test in (
        "kura::tests::progress_witness_durability::"
        "absent_progress_namespace_requires_every_directory_barrier",
        "kura::tests::progress_witness_durability::"
        "bound_progress_recovery_handles_crash_phases_without_path_escape",
        "kura::tests::progress_witness_durability::"
        "direct_receipt_snapshot_preserves_sparse_and_mixed_format_entries",
        "kura::tests::progress_witness_durability::"
        "initial_preindex_data_sync_failure_rolls_back_payload_before_retry",
        "kura::tests::progress_witness_durability::"
        "progress_sidecar_mutation_rejects_symlinks_without_external_writes",
        "kura::tests::progress_witness_durability::"
        "progress_prepend_directory_failure_retries_without_corruption",
        "kura::tests::progress_witness_durability::"
        "unindexed_crash_suffix_is_repaired_before_retry_or_append",
        "kura::lane_geometry::tests::"
        "first_release_retirement_requires_bound_progress_sidecar_durability",
        "kura::lane_geometry::tests::"
        "geometry_gc_requires_bound_merge_receipt_durability_before_deletion",
        "sumeragi::v2_core::tests::"
        "timeout_elapsed_cannot_start_durable_timeout_after_decision",
        "sumeragi::v2_core::tests::"
        "quorum_completing_timeout_vote_cannot_form_tc_after_decision",
    ):
        assert required_test in production_inventory
    assert (
        "sumeragi::v2_effects::tests::"
        "runtime_step_dispatches_entire_effect_batch_before_returning"
        in production_inventory
    )
    assert (
        "sumeragi::v2_apply::tests::"
        "committed_merge_reservation_rejects_bare_norito"
        in production_inventory
    )
    assert (
        "sumeragi::v2_worker::tests::"
        "locked_candidate_future_completion_is_rejected_without_replacing_owner"
        in production_inventory
    )
    assert (
        "sumeragi::v2_worker::tests::"
        "unavailable_locked_candidate_rebinds_latest_consumer_before_retry"
        in production_inventory
    )
    assert (
        "sumeragi::v2_effects::tests::"
        "decision_installed_by_same_runtime_step_retires_stale_terminal_effects"
        in production_inventory
    )
    assert (
        "sumeragi::v2_effects::tests::"
        "decision_installed_by_same_runtime_step_keeps_exact_commit_and_body_work"
        in production_inventory
    )
    production_modules_start = release_source.index("production_liveness_modules=(")
    production_modules_end = release_source.index("\n)", production_modules_start)
    production_modules = tuple(
        line.strip()
        for line in release_source[
            production_modules_start:production_modules_end
        ].splitlines()
        if line.strip().startswith(("sumeragi::", "sumeragi_v2_runner", "kura::"))
    )
    assert len(production_modules) == 17
    assert len(set(production_modules)) == 17
    assert "kura::tests::progress_witness_durability" in production_modules
    assert "kura::lane_geometry::tests" in production_modules
    assert "sumeragi::authoritative_runtime_gate_tests" in production_modules
    assert "sumeragi::v2_block_sync::tests" in production_modules
    assert "sumeragi::v2_apply::tests" in production_modules
    assert "sumeragi_v2_runner" in production_modules
    assert 'for module in "${production_liveness_modules[@]}"; do' in release_source
    assert (
        'cargo test --locked -p iroha_core --lib "$module" -- --test-threads=1'
        in release_source
    )
    assert (
        "cargo test --locked -p integration_tests --test "
        "sumeragi_v2_runner_isolated "
        "sumeragi_v2_runner::prepare_qc_split_tests::"
        "distinct_prepare_qc_view_zero_wait_covers_deadline_without_masking_view_one "
        "-- --exact --test-threads=1"
        in release_source
    )
    assert (
        '_PRODUCTION_INTEGRATION_TEST = (\n'
        '    "sumeragi_v2_runner::prepare_qc_split_tests::"\n'
        '    "distinct_prepare_qc_view_zero_wait_covers_deadline_without_masking_view_one"\n'
        ")"
        in receipt_source
    )
    assert "production_integration_ignored_unit_list=" in release_source
    assert (
        "serialized_runtime_rebinds_busy_deferred_body_completion_before_service"
        in release_source
    )
    assert (
        "tc_body_rebind_preserves_certified_request_ownership_through_signed_response"
        in release_source
    )
    assert "replay_does_not_resign_commit_superseded_by_higher_tc_lock" in release_source
    assert "fetch_consumer_rebind_preserves_live_or_queued_reconstruction_owner" in release_source
    assert "blocker_classifier_has_stable_specific_precedence" in release_source
    assert (
        "current_view_timeout_path_supersedes_prepare_but_not_any_locked_commit"
        in release_source
    )
    assert "missing required production Sumeragi v2 liveness test" in release_source
    assert "production_ignored_unit_list=" in release_source
    assert "required production Sumeragi v2 liveness test is ignored" in release_source
    assert "compute_workspace_source_manifest.py" in release_source
    assert 'compute_workspace_source_manifest.py --root "$repo_root"' in release_source
    assert "IROHA_RELEASE_SOURCE_MANIFEST_SHA256" in release_source
    assert "source_manifest_contract_tests=(" in release_source
    assert "pytests/scripts/workspace_source_manifest_test.py" in release_source
    assert "pytests/scripts/seal_workspace_source_test.py" in release_source
    assert "did not run exactly 30 passing tests" in release_source
    assert "seed_launcher_contract_tests=(" in release_source
    assert "did not run exactly ten passing tests" in release_source
    assert "did not run exactly five passing tests" in release_source
    assert "preflight-chaos-launcher pytest 5" in release_source
    assert "did not run exactly 68 passing tests" in release_source
    assert "preflight-release-identity pytest 68" in release_source
    assert "did not run exactly 71 passing tests" in release_source
    assert "preflight-release-bootstrap pytest 71" in release_source
    assert "did not run exactly 37 passing tests" in release_source
    assert "preflight-release-bootstrap-validator pytest 37" in release_source
    assert "did not run exactly 175 passing tests" in release_source
    assert "preflight-release-receipt pytest 175" in release_source
    assert (
        '"preflight-chaos-launcher",\n                "pytest",\n                5,'
        in receipt_source
    )
    assert (
        '"preflight-release-identity",\n                "pytest",\n                68,'
        in receipt_source
    )
    assert (
        '"preflight-release-bootstrap",\n                "pytest",\n                71,'
        in receipt_source
    )
    assert (
        '"preflight-release-bootstrap-validator",\n                "pytest",\n                37,'
        in receipt_source
    )
    assert (
        '"preflight-release-receipt",\n                "pytest",\n                175,'
        in receipt_source
    )
    assert "did not run exactly 477 passing tests" in release_source
    assert "preflight-proof-fidelity pytest 477" in release_source
    assert (
        "^477 passed in [0-9]+([.][0-9]+)?s( "
        r"\([0-9]+:[0-5][0-9]:[0-5][0-9]\))?$"
        in release_source
    )
    assert (
        r'r"([0-9]+) passed in [0-9]+(?:\.[0-9]+)?s"' in release_source
    )
    assert (
        r'r"(?: \([0-9]+:[0-5][0-9]:[0-5][0-9]\))?"'
        in release_source
    )
    for contract_file in (
        "pytests/scripts/sumeragi_v2_proof_ledger_test.py",
        "pytests/scripts/sumeragi_v2_verus_evidence_test.py",
        "pytests/scripts/sumeragi_v2_tlc_trace_normalizer_test.py",
    ):
        assert contract_file in release_source
        assert contract_file in receipt_source
    assert (
        '"preflight-proof-fidelity",\n                "pytest",\n                477,'
        in receipt_source
    )
    assert "did not run exactly twelve passing tests" in release_source
    assert "taira_release_ignored_contract_list=" in release_source
    assert "required Taira release-evidence contract test is ignored" in release_source
    for test_name in (
        "release_execution_profile_accepts_only_the_exact_positive_profile",
        "release_execution_profile_rejects_wrong_or_blank_build_profiles",
        "release_execution_profile_rejects_cargo_profile_mismatch",
        "release_execution_profile_rejects_non_exact_offline_values",
        "simulation_summary_json_records_release_profile_and_status_evidence",
    ):
        assert test_name in release_source
    assert "taira_soak_contract_files=(" in release_source
    assert "did not run exactly 39 passing tests" in release_source
    assert "expected_corridor_leg_count=36" in release_source
    assert "resolve_java.sh" in formal_launcher_source
    assert '"preflight-formal-launcher"' in receipt_source
    assert 'if [[ "$profile" == "--release" ]]; then' in release_source
    assert "Fail before 160 real-network runs" in release_source
    assert "workspace sources changed during the PR release corridor" in release_source
    assert "workspace sources changed before the Taira production soak" in release_source
    assert "workspace sources changed during the production release corridor" in release_source
    soak_source = (
        ROOT_DIR / "scripts" / "run_taira_v2_24h_soak.sh"
    ).read_text(encoding="utf-8")
    assert "expected exactly one ignored Taira soak" in soak_source
    assert "check_taira_v2_soak_evidence.py" in soak_source
    pinned_taira_profile = {
        "IROHA_TAIRA_SIM_DURATION_SECS": "86400",
        "IROHA_TAIRA_SIM_SEED": "taira-public-sim",
        "IROHA_TAIRA_LOAD_TPS": "5",
        "IROHA_TAIRA_PACKET_LOSS_PERCENT": "10",
        "IROHA_TAIRA_CHURN_INTERVAL_SECS": "300",
        "IROHA_TAIRA_MAX_HEIGHT_SKEW": "2",
        "IROHA_TAIRA_MAX_HEIGHT_SKEW_GRACE_SECS": "30",
        "IROHA_TAIRA_MAX_TRANSIENT_HEIGHT_SKEW": "32",
        "IROHA_TAIRA_STALL_TIMEOUT_SECS": "300",
        "IROHA_TAIRA_MAX_VIEW_CHANGE_RATE": "0.2",
        "IROHA_TAIRA_MAX_LAGGED_CYCLE_RATIO": "0.35",
        "IROHA_TAIRA_MIN_COMMITTED_TPS_RATIO": "0.6",
        "IROHA_TAIRA_KEEP_LOCALNET": "1",
    }
    for name, value in pinned_taira_profile.items():
        assert f"export {name}={value}" in soak_source

    chaos_branch = harness_source.index("--chaos-100k)")
    chaos_inventory = harness_source.index("ignored_test_list=", chaos_branch)
    chaos_run = harness_source.index('"$ignored_test" \\\n', chaos_inventory)
    assert chaos_branch < chaos_inventory < chaos_run
    assert "expected exactly one ignored chaos test" in harness_source

    unit_branch = harness_source.index("--unit)")
    unit_inventory = harness_source.index("unit_test_list=", unit_branch)
    unit_ignored_inventory = harness_source.index(
        "unit_ignored_test_list=", unit_inventory
    )
    unit_run = harness_source.index(
        "--lib -- --test-threads=1", unit_ignored_inventory
    )
    assert unit_branch < unit_inventory < unit_ignored_inventory < unit_run
    assert "expected exactly 96 Sumeragi v2 reducer unit tests" in harness_source
    assert "reducer unit gate requires all 96 tests to be runnable" in harness_source

    replay_branch = harness_source.index("--model-replay)")
    replay_inventory = harness_source.index("model_replay_test_list=", replay_branch)
    replay_ignored_inventory = harness_source.index(
        "replay_ignored_test_list=", replay_inventory
    )
    replay_run = harness_source.index(
        "--test model_trace_replay -- --test-threads=1", replay_ignored_inventory
    )
    assert replay_branch < replay_inventory < replay_ignored_inventory < replay_run
    assert "expected exactly eight Sumeragi v2 model-replay tests" in harness_source
    assert "model-replay gate requires all eight tests to be runnable" in harness_source

    finalizer = taira_source.index("fn finalize_result")
    fail_closed = taira_source.index(
        "sandbox::enforce_network_start_requirement::<()>(None, context)?", finalizer
    )
    successful_skip = taira_source.index("return Ok(());", fail_closed)
    assert finalizer < fail_closed < successful_skip


def test_release_corridor_clears_binary_overrides_and_uses_source_bound_targets() -> None:
    release_source = (
        ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh"
    ).read_text(encoding="utf-8")
    soak_source = (
        ROOT_DIR / "scripts" / "run_taira_v2_24h_soak.sh"
    ).read_text(encoding="utf-8")

    for source in (release_source, soak_source):
        assert "unset TEST_NETWORK_BIN_IROHAD KAGAMI_BIN" in source
        assert "CARGO_BIN_EXE_iroha3d CARGO_BIN_EXE_kagami" in source
        assert "TEST_NETWORK_BIN_IROHAD_MESSAGE_CONTROL TEST_NETWORK_BIN_IROHA" in source
        assert "TEST_NETWORK_IROHAD_FEATURES TEST_NETWORK_CARGO" in source
        assert "CARGO_BIN_EXE_iroha" in source
        assert "IROHA_TEST_SKIP_BUILD=0" in source
        assert "IROHA_TEST_ALLOW_REENTRANT_BUILD=1" in source
        assert "IROHA_TEST_BUILD_TIMEOUT_MS=3600" in source
        assert "sumeragi-v2-release/${" in source
    assert 'export CARGO_TARGET_DIR="${source_bound_root}/test-suite"' in soak_source
    assert 'export IROHA_TEST_TARGET_DIR="${source_bound_root}/programs"' in soak_source


def test_tlaps_runner_rejects_backend_failure_even_when_tlapm_exits_zero() -> None:
    source = (
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlaps.sh"
    ).read_text(encoding="utf-8")

    completion_check = source.index('TLAPM_COMPLETION_PATTERN=')
    exact_count = source.index('grep -Ec "$TLAPM_COMPLETION_PATTERN"')
    final_line = source.index('tail -n 1 "${LOG_DIR}/${module}.log"')
    runner_marker = source.index('"SUMERAGI_TLAPS_BACKEND_COMPLETE module=${module}')
    assert completion_check < exact_count < runner_marker
    assert completion_check < final_line < runner_marker
    assert "TLAPM did not report exact strict completion" in source


def test_tla2tools_and_replay_share_the_same_pin() -> None:
    scripts = [
        ROOT_DIR / "scripts" / "formal" / "install_sumeragi_v2_tla2tools.sh",
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlc.sh",
        ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_replay_trace.sh",
    ]
    sources = [path.read_text(encoding="utf-8") for path in scripts]

    assert all('1.7.4' in source for source in sources)
    assert all(
        "936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"
        in source
        for source in sources
    )
    # `-noGenerateSpecTE` was introduced after the immutable v1.7.4 release.
    # Keep both TLC entry points executable with the toolchain pinned above.
    assert all("-noGenerateSpecTE" not in source for source in sources[1:])


def test_tlc_entrypoints_use_the_pinned_tlapm_function_library() -> None:
    scripts = [
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlc.sh",
        ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_replay_trace.sh",
    ]
    sources = [path.read_text(encoding="utf-8") for path in scripts]

    assert all(
        "763bf3c1826d77a4cf206f43d5aa16775da1da33" in source
        for source in sources
    )
    for expected_hash in (
        "b54ff63b7c76c327525c17c188d5f9f5e53d92f3fd701f5e2ba54f0f54391063",
        "aa59063fd600bb640b2ae24dc85ef770277ef5bf7955092b76b8b471790086da",
    ):
        assert all(expected_hash in source for source in sources)
    assert all("standard-library checksum mismatch" in source for source in sources)
    assert all('"-DTLA-Library=${tlapm_compat_dir}"' in source for source in sources)
    assert all(
        'ln -s "${TLAPM_STDLIB}/Functions.tla"' in source
        and 'ln -s "${TLAPM_STDLIB}/Folds.tla"' in source
        for source in sources
    )
    assert all('readonly TLC_MAX_SET_SIZE="1000000"' in source for source in sources)
    assert all('-maxSetSize "$TLC_MAX_SET_SIZE"' in source for source in sources)


def test_liveness_tlc_ceilings_fit_pinned_evaluator_and_service_budget() -> None:
    source = (
        ROOT_DIR / "docs" / "formal" / "sumeragi_v2" / "liveness.cfg"
    ).read_text(encoding="utf-8")

    def natural(name: str) -> int:
        match = re.search(rf"^  {name} = ([0-9]+)$", source, re.MULTILINE)
        assert match is not None
        return int(match.group(1))

    validator_count = natural("N")
    queue_capacity = natural("AsyncQueueCapacity")
    ingress_capacity = natural("AsyncIngressCapacity")
    progress_reserve = natural("AsyncProgressReserve")
    completion_reserve = natural("AsyncCompletionReserve")
    io_aux_capacity = natural("AsyncIoAuxCapacity")
    io_work_capacity = natural("AsyncIoWorkCapacity")
    deferred_normal_capacity = natural("AsyncDeferredNormalCapacity")
    deferred_progress_capacity = natural("AsyncDeferredProgressCapacity")
    delivery_bound = natural("AsyncDeliveryBound")
    retransmit_period = natural("AsyncRetransmitPeriod")
    chunk_count = natural("AsyncChunkCount")

    runner_cycle_budget = queue_capacity + 2 * ingress_capacity + 3
    runtime_cycle_budget = 3 * queue_capacity * runner_cycle_budget
    io_drain_budget = io_aux_capacity + io_work_capacity + 1
    deferred_drain_budget = (
        deferred_normal_capacity + deferred_progress_capacity + completion_reserve
    )
    retransmit_emission_budget = (
        7 * validator_count
        + validator_count * chunk_count
        + 2 * validator_count
    )
    one_way_transport_budget = delivery_bound * (
        ingress_capacity
        + runtime_cycle_budget
        + retransmit_emission_budget
        + 1
    )
    proposal_pipeline_budget = 4 * validator_count * (
        runtime_cycle_budget
        + io_drain_budget
        + deferred_drain_budget
        + chunk_count
        + 8
    )
    certified_recovery_budget = (
        2 * one_way_transport_budget
        + 2 * io_drain_budget * delivery_bound
        + 3 * runtime_cycle_budget * delivery_bound
    )
    worst_case_service_budget = (
        proposal_pipeline_budget * delivery_bound
        + certified_recovery_budget
        + 4 * retransmit_period
        + progress_reserve
        + completion_reserve
    )

    maximum_timeout = natural("AsyncMaximumRoundTimeout")
    maximum_view = natural("AsyncMaximumView")
    assert natural("MaxEpoch") == 0
    assert natural("MaxHeight") == 0
    assert "EpochRosters <- CountRostersOneEpoch" in source
    assert "EpochPowers <- CountPowersOneEpoch" in source
    assert "LeaderStarts <- StartsByzantineFirst" in source
    assert "LaneHashes <- LaneHashesOneHeight" in source
    assert "DaHashes <- DaHashesOneHeight" in source
    assert worst_case_service_budget < maximum_timeout <= 2_147_483_647
    assert worst_case_service_budget <= maximum_view <= 2_147_483_647


def test_workspace_excluded_harness_names_every_required_fast_simulation() -> None:
    source = (
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_harness.sh"
    ).read_text(encoding="utf-8")
    expected = {
        "lossy_offline_leader_simulations_commit_for_4_7_and_10_validators",
        "two_by_two_partition_cannot_advance_but_healing_retransmits_tc_and_commits",
        "asymmetric_partition_stalls_without_dual_quorum_then_heals_and_applies",
        "leader_crash_after_proposal_broadcast_does_not_block_the_remaining_quorum",
        "leader_crash_with_a_locked_body_rotates_and_rebuilds_the_old_commit_quorum",
        "corrupted_chunks_and_withheld_commit_evidence_recover_by_bounded_retransmission",
        "crash_after_proposal_wal_before_signature_replays_exact_intent",
        "taira_divergent_views_converge_and_commit_within_one_rotation",
        "accelerated_chain_chaos_smoke_preserves_prefix",
    }

    required_block = re.search(r"required_tests=\(\n(?P<body>.*?)\n    \)", source, re.S)
    assert required_block is not None
    listed = {
        line.strip()
        for line in required_block.group("body").splitlines()
        if line.strip()
    }
    assert listed == expected
    assert 'ignored_test="accelerated_100_000_block_chaos_preserves_chain_prefix"' in source
    assert "--list --ignored" in source
    assert "expected exactly nine fast and one ignored" in source
    assert "expected six Sumeragi v2 network simulations" not in source
    assert "--unit" in source
    assert "--model-replay" in source
    assert "--chaos-100k" in source


def test_installers_use_fixed_urls_and_literal_checksums() -> None:
    installers = [
        ROOT_DIR / "scripts" / "formal" / "install_sumeragi_v2_tlapm.sh",
        ROOT_DIR / "scripts" / "formal" / "install_sumeragi_v2_tla2tools.sh",
        ROOT_DIR / "scripts" / "formal" / "install_sumeragi_v2_verus.sh",
    ]
    for installer in installers:
        source = installer.read_text(encoding="utf-8")
        assert "latest" not in source.lower()
        assert re.search(r'readonly [A-Z_]*SHA256="[0-9a-f]{64}"', source)
        assert "curl" in source
        assert "checksum mismatch" in source


def test_ledger_is_canonical_json() -> None:
    module = load_checker()
    source = module.LEDGER_PATH.read_text(encoding="utf-8")
    parsed = json.loads(source)

    assert source == json.dumps(parsed, indent=2, ensure_ascii=False) + "\n"


def test_serve_occurrence_rank_and_starvation_conjunct_are_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Proofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "ServeJobRank(node, job) == <<5, ServeJobIndex(node, job)>>",
            "ServeJobRank(node, job) == <<5, CandidateIoIndex("
            "job.candidate, asyncIoQueues[node])>>",
            1,
        ).replace(
            "     \\/ ProtectedServeRankDecreaseStep\n",
            "",
            1,
        ).replace(
            "  /\\ ProtectedServeStarvationProperty(specification)\n",
            "",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any("ServeJobRank must equal only" in error for error in errors)
    assert any("PostGstProductiveStep must equal only" in error for error in errors)
    assert any("StarvationFreedomProperty must equal only" in error for error in errors)


def test_exact_removal_and_protected_slot_geometry_theorems_are_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Proofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    proofs = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = proofs.read_text(encoding="utf-8")
    removal = source.index("THEOREM OneRemovalIncreasesSourceProtectionByAtMostOne")
    universe = source.index("THEOREM ProtectedProgressSlotUniverseSize")
    mutated = (
        source[:removal]
        + source[removal:universe].replace(
            "LET after == SequenceWithoutIndex(before, selected)",
            "LET after == Tail(before)",
            1,
        )
        + source[universe:].replace(
            "Cardinality(ProtectedProgressSlotUniverse) = 2 * N + 3",
            "Cardinality(ProtectedProgressSlotUniverse) = N + 3",
            1,
        )
    )
    proofs.write_text(mutated, encoding="utf-8")

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "OneRemovalIncreasesSourceProtectionByAtMostOne must state only" in error
        for error in errors
    )
    assert any(
        "ProtectedProgressSlotUniverseSize must state only" in error
        for error in errors
    )


def test_normal_proposal_prepare_protection_contract_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Proofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "     \\/ NormalProposalPrepareCandidate(candidate)\n", "", 1
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ProtectedServiceCandidate must equal only" in error
        for error in errors
    )


def test_normal_proposal_prepare_kind_inventory_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Proofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            '{"Proposal", "PrepareVote", "CommitVote"}',
            '{"Proposal", "PrepareVote"}',
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalProposalPrepareNetworkKinds must equal only" in error
        for error in errors
    )


def test_normal_proposal_prepare_requires_canonical_carrier(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Proofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "ProtectedServiceCandidate(candidate) ==\n"
            "  /\\ candidate \\in AsyncCandidateSet\n",
            "ProtectedServiceCandidate(candidate) ==\n"
            "  /\\ AsyncCandidateTyped(candidate)\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ProtectedServiceCandidate must equal only" in error
        for error in errors
    )


def test_normal_delivery_class_is_frozen_at_admission(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Proofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    frozen_network = (
        "    /\\ candidate = FrozenNormalDeliveryCandidate(\n"
        "                     item, consumerContext, consumerView,\n"
        "                     consumerGeneration)\n"
    )
    assert frozen_network in source
    vocabulary.write_text(
        source.replace(
            frozen_network,
            "    /\\ candidate = NormalDeliveryCandidate(item)\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalProposalPrepareNetworkCandidate must equal only" in error
        for error in errors
    )

    frozen_identity = (
        "       consumerContext, consumerView, consumerGeneration, item,\n"
    )
    assert frozen_identity in source
    vocabulary.write_text(
        source.replace(
            frozen_identity,
            "       context, nodeView[item.envelope.recipient],\n"
            "       generation[item.envelope.recipient], item,\n",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "FrozenNormalDeliveryCandidate must equal only" in error
        for error in errors
    )


def test_normal_install_successor_is_required_and_frozen(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Proofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    install_successor_branch = (
        "     \\/ \\E command \\in AsyncCandidateSet,\n"
        "            installedContext \\in ContextRecords,\n"
        "            priorGeneration \\in Generations,\n"
        "            subject \\in SubjectOrNone:\n"
        "          /\\ command.kind = \"PersistInstallTC\"\n"
        "          /\\ command.view + 1 \\in Views\n"
        "          /\\ candidate = FrozenInstallProposalSuccessor(\n"
        "                           command, installedContext,\n"
        "                           priorGeneration, subject)\n"
    )
    assert install_successor_branch in source
    vocabulary.write_text(
        source.replace(install_successor_branch, "", 1),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalProposalPrepareNoItemCandidate must equal only" in error
        for error in errors
    )

    frozen_generation = "NextCandidateGeneration(priorGeneration)"
    assert frozen_generation in source
    vocabulary.write_text(
        source.replace(
            frozen_generation,
            "generation[command.node]",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "FrozenInstallProposalSuccessor must equal only" in error
        for error in errors
    )


def test_begin_prepare_parent_inventory_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Proofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            '{"DeliverProposal", "ValidateBody"}',
            '{"DeliverProposal"}',
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "NormalBeginPrepareParentKinds must equal only" in error
        for error in errors
    )


def test_normal_candidate_step_stability_theorem_is_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Proofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    proofs = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = proofs.read_text(encoding="utf-8")
    proofs.write_text(
        source.replace(
            "    /\\ AsyncNext\n"
            "    => NormalProposalPrepareCandidate(candidate)'\n",
            "    /\\ PostGstSchedulerActionEnabled\n"
            "    => NormalProposalPrepareCandidate(candidate)'\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "AsyncNextPreservesNormalProposalPrepareCandidate must state only"
        in error
        for error in errors
    )


def test_deadlock_contract_rejects_scheduler_only_enablement(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Proofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    property_offset = source.index("DeadlockFreedomProperty(specification) ==")
    enabled_offset = source.index(
        "PostGstProductiveActionEnabled", property_offset
    )
    vocabulary.write_text(
        source[:enabled_offset]
        + source[enabled_offset:].replace(
            "PostGstProductiveActionEnabled",
            "PostGstSchedulerActionEnabled",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "DeadlockFreedomProperty must equal only" in error
        for error in errors
    )


def test_deadlock_contract_rejects_scheduler_only_productive_alias(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Proofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    vocabulary.write_text(
        source.replace(
            "PostGstProductiveActionEnabled == ENABLED PostGstProductiveStep",
            "PostGstProductiveActionEnabled == PostGstSchedulerActionEnabled",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "PostGstProductiveActionEnabled must equal only" in error
        for error in errors
    )


def test_async_source_fidelity_pins_dual_progress_ingress_geometry(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace(
            '   "TimeoutCertificate", "Chunk", "CertifiedRequest",\n',
            '   "TimeoutCertificate", "Chunk",\n',
            1,
        ).replace(
            "    + Cardinality(\n"
            "        IngressTimeoutVoteProtectedSourcesFor(lanes, recipient))\n",
            "",
            1,
        ).replace(
            '                    "TimeoutCertificate", "Chunk", "CertifiedResponse",\n'
            '                    "CommitCertificateResponse",\n',
            '                    "TimeoutCertificate", "Chunk", "CertifiedRequest",\n'
            '                    "CertifiedResponse", "CommitCertificateRequest",\n'
            '                    "CommitCertificateResponse",\n',
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("IngressProgressKinds must equal only" in error for error in errors)
    assert any(
        "IngressProtectedSlotCountFor must equal only" in error for error in errors
    )
    assert any("DeliveryClass must equal only" in error for error in errors)


def test_async_source_fidelity_pins_timeout_signer_partition_without_displacement(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace(
            "AsyncDeferredProgressCapacity >= 2 * N + 3",
            "AsyncDeferredProgressCapacity >= N + 3",
            1,
        ).replace(
            '    [] command.kind = "DeliverTimeout" ->\n'
            '         command.item.kind = "TimeoutVote"\n',
            "",
            1,
        ).replace(
            "     ELSE IF SameProtectedProgressSlotIndices(node, command) # {}\n"
            "          THEN queue\n",
            "     ELSE IF SameProtectedProgressSlotIndices(node, command) # {}\n"
            "          THEN SequenceWithoutIndex(queue, 1)\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "AsyncConfiguration omits required production behavior" in error
        for error in errors
    )
    assert any("ProtectedProgressCommand must equal only" in error for error in errors)
    assert any("DeferredProgressAfter must equal only" in error for error in errors)


def test_async_source_fidelity_pins_live_serve_occurrence_identity(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(
        source.replace(
            'AsyncIoJob("Serve", candidate, FreshAsyncIoServeNonce(node))',
            'AsyncIoJob("Serve", candidate, 0)',
            1,
        ).replace(
            "    /\\ AsyncIoServeNonceOwnership(asyncIoQueues[node])\n",
            "",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("AsyncIoCertifiedServeJob must equal only" in error for error in errors)
    assert any(
        "AsyncIoQueueContentTypeInvariant must equal only" in error
        for error in errors
    )


def test_async_source_fidelity_pins_timeout_vote_semantic_capacity_bypass(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    (formal_dir / "SumeragiV2AsyncNetwork.tla").write_text(
        (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
            encoding="utf-8"
        ),
        encoding="utf-8",
    )
    rust_path = tmp_path / "crates" / "iroha_core" / "src" / "sumeragi" / "v2.rs"
    rust_path.parent.mkdir(parents=True)
    canonical = "\n".join(
        (
            "const fn semantic_ingress_capacity(roster_len: usize) -> usize",
            "let protected_capacity_bypass =",
            "locked_commit_progress || matches!(key, IngressSemanticKey::TimeoutVote { .. })",
            "matches!(key, IngressSemanticKey::TimeoutVote { .. })",
            "if capacity_bypass && !protected_capacity_bypass",
            "let matches_current_timeout = |key: IngressSemanticKey|",
            "matches_current_lock(*key, record.fingerprint) || matches_current_timeout(*key)",
            "semantic_ingress_capacity(self.wire_context.roster.len())",
            "fn capacity_bypass_records_follow_current_lock_and_timeout_view()",
            "roster_len * 2",
            "assert_eq!(adapter.ingress_equivocations, same_view_equivocations)",
            "fn assert_timeout_vote_owner_rolls_back_across_view_and_retries()",
            "for attempt in 0..2",
            "assert_registry_eq(&adapter.registry, &registry_before)",
            "assert!(!adapter.ingress_deliveries.contains_key(&current_key))",
            "fn full_normal_deferred_lane_cannot_drop_absolute_timeout()",
            "assert_timeout_vote_owner_rolls_back_across_view_and_retries();",
            "MAX_INGRESS_SEMANTIC_KEYS",
            ".is_some_and(|record| record.capacity_bypass)",
            "Some(DeferredProgressClass::TimeoutVote)",
        )
    )
    rust_path.write_text(canonical, encoding="utf-8")
    assert not any(
        "semantic-capacity bypass" in error
        for error in module._async_source_fidelity_errors(formal_dir)
    )

    for required in (
        "matches!(key, IngressSemanticKey::TimeoutVote { .. })",
        "fn capacity_bypass_records_follow_current_lock_and_timeout_view()",
        "assert_timeout_vote_owner_rolls_back_across_view_and_retries();",
    ):
        rust_path.write_text(
            canonical.replace(required, "REMOVED", 1), encoding="utf-8"
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any("semantic-capacity bypass" in error for error in errors)


def test_productive_liveness_mutations_are_pinned() -> None:
    runner = (
        ROOT_DIR
        / "scripts"
        / "formal"
        / "run_sumeragi_v2_productive_mutation.sh"
    ).read_text(encoding="utf-8")
    assert 'TLA2TOOLS_VERSION="1.7.4"' in runner
    assert (
        'TLA2TOOLS_SHA256="936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88"'
        in runner
    )
    assert "-fp 96 -seed 139154308881391968" in runner
    assert "normal_old_status -eq 13" in runner
    assert "normal_dynamic_class_status -eq 12" in runner
    assert "productive_bare_status -eq 12" in runner
    assert "Temporal properties were violated." in runner
    assert "State 2: Stuttering" in runner
    assert "Invariant ProductiveDeadlockClaim is violated by the initial state" in runner
    assert "normal_protected_old.cfg" in runner
    assert "normal_protected_dynamic_class_bug.cfg" in runner
    assert "normal_protected_fixed.cfg" in runner
    assert "productive_deadlock_scheduler_bug.cfg" in runner
    assert "productive_deadlock_bare_rejected.cfg" in runner
    assert "productive_deadlock_fixed.cfg" in runner

    formal_dir = ROOT_DIR / "docs" / "formal" / "sumeragi_v2"
    normal_mutation = (
        formal_dir / "SumeragiV2NormalProtectedMutation.tla"
    ).read_text(encoding="utf-8")
    assert (
        'NormalProposalPrepareKinds ==\n  {"AssembleBody", "DeliverProposal", '
        '"BeginPrepare", "DeliverVote"}'
        in normal_mutation
    )
    assert "ProtectedNormal ==\n  /\\ ProtectNormal" in normal_mutation
    assert "DynamicDeliveryClass ==" in normal_mutation
    assert "StoredNormalRemainsProtected ==" in normal_mutation
    assert "NormalEventuallyServiced == <>~scheduled" in normal_mutation
    assert (formal_dir / "normal_protected_old.cfg").read_text(
        encoding="utf-8"
    ).startswith(
        "CONSTANT ProtectNormal = FALSE\n"
        "CONSTANT RecomputeNormalClass = FALSE\n"
        "SPECIFICATION Spec\n"
    )
    assert (formal_dir / "normal_protected_dynamic_class_bug.cfg").read_text(
        encoding="utf-8"
    ).startswith(
        "CONSTANT ProtectNormal = TRUE\n"
        "CONSTANT RecomputeNormalClass = TRUE\n"
        "SPECIFICATION Spec\n"
    )
    assert (formal_dir / "normal_protected_fixed.cfg").read_text(
        encoding="utf-8"
    ).startswith(
        "CONSTANT ProtectNormal = TRUE\n"
        "CONSTANT RecomputeNormalClass = FALSE\n"
        "SPECIFICATION Spec\n"
    )

    productive_mutation = (
        formal_dir / "SumeragiV2ProductiveDeadlockMutation.tla"
    ).read_text(encoding="utf-8")
    assert "BareSchedulerStep ==" in productive_mutation
    assert "ProductiveStep ==" in productive_mutation
    assert "SchedulerOnlyDeadlockClaim ==" in productive_mutation
    assert "ProductiveDeadlockClaim ==" in productive_mutation
    assert (formal_dir / "productive_deadlock_scheduler_bug.cfg").read_text(
        encoding="utf-8"
    ).endswith("PROPERTY SchedulerOnlyDeadlockClaim\n")
    assert (formal_dir / "productive_deadlock_bare_rejected.cfg").read_text(
        encoding="utf-8"
    ).startswith("CONSTANT ProductiveRepair = FALSE\n")
    assert (formal_dir / "productive_deadlock_fixed.cfg").read_text(
        encoding="utf-8"
    ).startswith("CONSTANT ProductiveRepair = TRUE\n")
