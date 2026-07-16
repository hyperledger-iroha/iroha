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


def test_repository_ledger_pins_exact_current_proof_debt_and_dependencies() -> None:
    module = load_checker()
    ledger = module.load_ledger()

    assert tuple(
        obligation["id"]
        for obligation in ledger["obligations"]
        if obligation["status"] == "specified_unproved"
    ) == (
        "historical-tc-lock-commit",
        "timeout-protection",
        "effective-lock-body-acquisition",
        "async-runner-scheduler-preservation",
        "async-type-invariant",
        "progress-witness-preservation",
        "post-gst-deadlock-freedom",
        "protected-service-rank",
        "post-gst-starvation-freedom",
        "timeout-view-liveness",
        "rotating-leader-liveness",
        "application-liveness",
        "genesis-height-successor-handoff",
        "height-liveness",
    )
    assert module.PROOF_STATUS_DEPENDENCIES == {
        "timeout-protection": ("historical-tc-lock-commit",),
        "async-type-invariant": ("async-runner-scheduler-preservation",),
        "progress-witness-preservation": (
            "async-type-invariant",
            "generation-scoped-vote-delivery",
        ),
        "post-gst-deadlock-freedom": ("async-type-invariant",),
        "protected-service-rank": ("async-type-invariant",),
        "post-gst-starvation-freedom": (
            "async-type-invariant",
            "protected-service-rank",
        ),
        "timeout-view-liveness": (
            "progress-witness-preservation",
            "post-gst-starvation-freedom",
        ),
        "rotating-leader-liveness": (
            "effective-lock-body-acquisition",
            "progress-witness-preservation",
            "post-gst-starvation-freedom",
            "timeout-view-liveness",
        ),
        "application-liveness": (
            "progress-witness-preservation",
            "post-gst-starvation-freedom",
        ),
        "genesis-height-successor-handoff": (
            "rotating-leader-liveness",
            "application-liveness",
        ),
        "height-liveness": (
            "rotating-leader-liveness",
            "application-liveness",
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
    assert by_id["historical-tc-lock-commit"]["status"] == "specified_unproved"
    assert by_id["timeout-protection"]["status"] == "specified_unproved"


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

    assert theorem_modules == present_release_modules


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
        "historical-tc-lock-commit": "specified_unproved",
        "effective-lock-body-acquisition": "specified_unproved",
        "async-type-invariant": "specified_unproved",
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


def test_effective_lock_body_composition_remains_explicit_debt() -> None:
    module = load_checker()
    ledger = module.load_ledger()
    by_id = {obligation["id"]: obligation for obligation in ledger["obligations"]}

    composition = by_id["effective-lock-body-acquisition"]
    assert composition == {
        "id": "effective-lock-body-acquisition",
        "requirement": composition["requirement"],
        "module": "SumeragiV2AsyncLivenessProofs",
        "symbol": "EffectiveLockBodyAcquisitionCompositionObligation",
        "status": "specified_unproved",
    }
    source = (module.FORMAL_DIR / "SumeragiV2AsyncLivenessProofs.tla").read_text()
    theorem = module._top_level_theorem_body(
        source, "EffectiveLockBodyAcquisitionCompositionObligation"
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
ApplicationLivenessProperty(specification) ==
  specification
    => /\ \A node \in AsyncCurrentResponsiveVoters:
             (gst /\ NodeHasDecision(node))
               ~> NodeHasApplication(node)
       /\ (gst /\ ResponsiveNodesDecide) ~> ResponsiveNodesApply
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


def test_scheduler_rank_derivation_cannot_widen_the_owned_carrier(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = r"""---- MODULE SumeragiV2AsyncLivenessProofs ----
THEOREM AsyncStepRefinementObligation ==
  AsyncNext => [Next]_vars
BY DEF AsyncNext
THEOREM AsyncTypeInvariantObligation ==
  \A initialContext: AsyncSpecAt(initialContext) => []AsyncTypeInvariant
BY PTL
THEOREM ScheduledCandidateServiceRankInCarrier ==
  CandidateServiceRank(candidate) \in OwnedServiceRankCarrier
BY PTL
THEOREM ProtectedRankExitHasWellFoundedSuccessor ==
  rank \in OwnedServiceRankCarrier
    => <<CandidateServiceRank(candidate), rank>>
         \in OwnedServiceRankOrdering
BY PTL
THEOREM ProtectedRankProgressSuppliesWellFoundedStep ==
  \A rank \in OwnedServiceRankCarrier:
    rank \in OwnedServiceRankCarrier
BY PTL
THEOREM ProtectedServiceRankProgressImpliesStarvation ==
  IsWellFoundedOn(OwnedServiceRankOrdering, OwnedServiceRankCarrier)
BY OwnedServiceRankOrderingWellFounded
=============================================================================
"""
    path.write_text(source, encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    path.write_text(
        source.replace(
            "CandidateServiceRank(candidate) \\in OwnedServiceRankCarrier",
            "CandidateServiceRank(candidate) \\in ServiceRankCarrier",
            1,
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
    vocabulary.write_text(
        source.replace(
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
    assert '2>&1 | tee "$VERUS_LOG"' in source


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
    canonical = """---- MODULE SumeragiV2AsyncNetwork ----
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
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    canonical = """---- MODULE SumeragiV2AsyncNetwork ----
AsyncChunkReceiptSet ==
  [node: ValidatorIds, view: Views, subject: Subjects,
   chunk: AsyncChunks]
AsyncBodyEnvelopeSet ==
  [recipient: ValidatorIds, height: Heights, view: Views,
   subject: Subjects, chunk: 0..AsyncChunkCount,
   nonce: 0..(AsyncIngressCapacity - 1)]
AsyncBodyEnvelopeTyped(envelope) ==
  /\\ envelope.subject \\in Subjects
AsyncSetGST == /\\ ~gst /\\ SetGST /\\ UNCHANGED AsyncSchedulerVars
RetainedControlEmissionItems(node) == SendableItems(node) \\cup RetainedProposalChunks(node)
CertifiedServeCanRespond(request) ==
  /\\ request.kind = "CertifiedRequest"
  /\\ BodyHeldBy(durableBodies, request.envelope.recipient,
                context, request.envelope.view, request.envelope.subject)
AsyncBaseInit == AsyncBaseInitAt(ContextRecord(0, <<>>))
AsyncStepRefinesCore == AsyncNext => [Next]_vars
VoteOutbox(request) ==
  {item: recipient \\in CurrentVoters \\ {request.node}}
InstallCommandSuccessors(command) ==
  IF InstallCommitSignRequests(command) = {}
  THEN <<InstallProposalSuccessor(command)>>
  ELSE <<InstallCommitSignSuccessor(command), InstallProposalSuccessor(command)>>
CommandSuccessors(command) ==
  CASE command.kind = "PersistDecision" ->
         <<CausalCandidate("Completion", "ValidateBody", command),
           CausalCandidate("Completion", "RequestCertifiedBody", command),
           CausalCandidate("Completion", "Apply", command)>>
    [] OTHER -> <<>>
NextCommandClass(commandClass) ==
  CASE commandClass = "Completion" -> "Progress"
    [] commandClass = "Progress" -> "Normal"
    [] OTHER -> "Completion"
SelectedDeferredClass(node) ==
  LET first == asyncNextDeferredClass[node]
      second == NextCommandClass(first)
      third == NextCommandClass(second)
  IN IF DeferredClassNonempty(node, first)
     THEN first
     ELSE IF DeferredClassNonempty(node, second)
          THEN second
          ELSE third
NextDeferredCommand(node) ==
  Head(DeferredClassQueue(node, SelectedDeferredClass(node)))
AdvanceNextDeferredClass(node) ==
  asyncNextDeferredClass' =
    [asyncNextDeferredClass EXCEPT
       ![node] = NextCommandClass(SelectedDeferredClass(node))]
RemoveNextDeferredCommand(node) ==
  /\\ IF SelectedDeferredClass(node) = "Completion"
     THEN /\\ asyncDeferredCompletionQueues' =
                [asyncDeferredCompletionQueues EXCEPT ![node] = Tail(@)]
          /\\ UNCHANGED <<asyncDeferredProgressQueues,
                         asyncDeferredNormalQueues>>
     ELSE IF SelectedDeferredClass(node) = "Progress"
          THEN /\\ asyncDeferredProgressQueues' =
                     [asyncDeferredProgressQueues EXCEPT ![node] = Tail(@)]
               /\\ UNCHANGED <<asyncDeferredCompletionQueues,
                              asyncDeferredNormalQueues>>
          ELSE /\\ asyncDeferredNormalQueues' =
                     [asyncDeferredNormalQueues EXCEPT ![node] = Tail(@)]
               /\\ UNCHANGED <<asyncDeferredCompletionQueues,
                              asyncDeferredProgressQueues>>
  /\\ AdvanceNextDeferredClass(node)
SelectedCommandClass(node) ==
  LET first == asyncNextCommandClass[node]
      second == NextCommandClass(first)
      third == NextCommandClass(second)
  IN IF CommandClassIndices(node, first) # {}
     THEN first
     ELSE IF CommandClassIndices(node, second) # {}
          THEN second
          ELSE third
NextNodeCommandIndex(node) ==
  FirstCommandClassIndex(node, SelectedCommandClass(node))
RemoveNextNodeCommand(node) ==
  /\\ asyncCommandQueues' =
       [asyncCommandQueues EXCEPT
          ![node] = SequenceWithoutIndex(@, NextNodeCommandIndex(node))]
  /\\ asyncNextCommandClass' =
       [asyncNextCommandClass EXCEPT
          ![node] = NextCommandClass(SelectedCommandClass(node))]
SchedulerClassPrefixIndices(node, command) ==
  {index \\in 1..Len(asyncCommandQueues[node]):
     /\\ asyncCommandQueues[node][index].class = command.class
     /\\ \\E matching \\in SchedulerCandidateIndices(node, command):
          index <= matching}
SchedulerServiceRank(node, command) ==
  3 * Cardinality(SchedulerClassPrefixIndices(node, command))
    + CommandClassDistance(asyncNextCommandClass[node], command.class)
CommandExecutionEnabled(command) ==
  \\E selectedCommand \\in {command}:
    \\/ ENABLED ExecuteRegularCommand(selectedCommand)
    \\/ ENABLED ExecuteSignProposal(selectedCommand)
    \\/ ENABLED ExecuteSignVote(selectedCommand)
    \\/ ENABLED ExecuteFormPrepareQC(selectedCommand)
    \\/ ENABLED ExecuteSignTimeout(selectedCommand)
    \\/ ENABLED ExecutePersistInstall(selectedCommand)
    \\/ ENABLED ExecutePersistDecision(selectedCommand)
    \\/ ENABLED ExecuteRequestCertifiedBody(selectedCommand)
    \\/ ENABLED ExecuteApply(selectedCommand)
    \\/ ENABLED ExecuteCoreDelivery(selectedCommand)
    \\/ ENABLED ExecuteChunkDelivery(selectedCommand)
    \\/ ENABLED ExecuteRejectAuthenticatedJunk(selectedCommand)
CommandDispatchable(command) ==
  /\\ AsyncCandidateTyped(command)
  /\\ CommandExecutionEnabled(command)
  /\\ (NodeIdle(command.node) \\/ command.class = "Completion")
RegularCoreCommand(command) ==
  \\/ /\\ command.kind = "ValidateBody"
     /\\ \\/ \\E proposal \\in SeenProposalValues:
               /\\ CommandMatches(command, command.node, proposal.view,
                                 proposal.subject)
               /\\ (ValidateBody(command.node, proposal)
                     \\/ RejectBody(command.node, proposal))
        \\/ \\E qc \\in DecisionQcValues:
             /\\ CommandMatches(command, command.node, qc.view, qc.subject)
             /\\ ValidateDecidedBody(command.node, qc)
  \\/ /\\ command.kind = "BeginPrepare"
     /\\ TRUE
AsyncSchedulerVars == <<asyncNextCommandClass, asyncNextDeferredClass>>
AsyncRuntimeInit ==
  asyncNextCommandClass = [node \\in ValidatorIds |-> "Completion"]
AsyncRuntimeScalarTypeInvariant ==
  asyncNextCommandClass \\in [ValidatorIds -> AsyncCommandClasses]
AsyncDeferredInit ==
  asyncNextDeferredClass = [node \\in ValidatorIds |-> "Completion"]
AsyncDeferredTopologyTypeInvariant ==
  asyncNextDeferredClass \\in [ValidatorIds -> AsyncCommandClasses]
DeferredDrainStep(node) ==
  /\\ NextDeferredCommand(node) = command
  /\\ RemoveNextDeferredCommand(node)
  /\\ AdvanceNextDeferredClass(node)
  /\\ UNCHANGED <<vars, asyncCommandQueues,
                 asyncDeferredCompletionQueues,
                 asyncDeferredProgressQueues,
                 asyncDeferredNormalQueues,
                 asyncNextCommandClass>>
FifoRuntimeStep(node) ==
  /\\ NextNodeCommand(node) = command
  /\\ RemoveNextNodeCommand(node)
ServiceIoWorker(node) ==
  /\\ asyncIoControlAvailable' =
       [asyncIoControlAvailable EXCEPT ![node] = TRUE]
  /\\ CommitCertificateServeCanRespond(job.candidate.item)
  /\\ CommitCertificateResponseItems(job.candidate.item) # {}
CertifiedRequestOutbox(node, qc) == qc.signers \\ {node}
SendNodeRetransmissions(node) == RetryableItems(node)
AsyncTickEnabled == ~gst \\/ OverdueResponsivePackets = {}
RunNode(node) == ~NodeHasApplication(node)
RunHistoricalServer(node) ==
  /\\ NodeHasApplication(node)
  /\\ DrainHistoricalIngressSelected(node)
HistoricalIngressItemCanDrain(node, item) ==
  item.kind = "CertifiedRequest" \\/ item.kind = "CommitCertificateRequest"
HistoricalDrainableIngressLaneIndices(node, source) ==
  {index \\in 1..Len(IngressLane(node, source)):
     HistoricalIngressItemCanDrain(node, IngressLane(node, source)[index])}
FirstHistoricalDrainableIngressLaneIndex(node, source) ==
  CHOOSE index \\in HistoricalDrainableIngressLaneIndices(node, source): TRUE
HistoricalIngressSourceCanDrain(node, source) ==
  HistoricalDrainableIngressLaneIndices(node, source) # {}
HistoricalSelectedIngressLaneIndex(node, index) ==
  FirstHistoricalDrainableIngressLaneIndex(
    node, asyncIngressReady[node][index])
HistoricalSelectedIngressItemAt(node, index) ==
  IngressLane(node, source)[HistoricalSelectedIngressLaneIndex(node, index)]
ItemInScheduledDelivery(item) ==
  \\E candidate \\in QueuedCandidates \\cup DeferredCandidates
                      \\cup CausalCandidates \\cup TrackedWorkCandidates:
    candidate.item = item
IngressItemCanDrain(node, item) ==
  LET candidate == DeliveryCandidate(item)
  IN CandidateScheduled(candidate) \\/ CanEnqueueClass(node, candidate.class)
DrainableIngressLaneIndices(node, source) ==
  {index \\in 1..Len(IngressLane(node, source)):
     IngressItemCanDrain(node, IngressLane(node, source)[index])}
FirstDrainableIngressLaneIndex(node, source) ==
  CHOOSE index \\in DrainableIngressLaneIndices(node, source): TRUE
IngressSourceCanDrain(node, source) ==
  DrainableIngressLaneIndices(node, source) # {}
SelectedIngressLaneIndex(node, index) ==
  FirstDrainableIngressLaneIndex(node, asyncIngressReady[node][index])
SelectedIngressItemAt(node, index) ==
  IngressLane(node, source)[SelectedIngressLaneIndex(node, index)]
ReadyAfterSelectedDrain(node, index) == asyncIngressReady[node]
PopSelectedIngress(node, index, laneIndex) ==
  /\\ asyncIngressLanes' =
       [asyncIngressLanes EXCEPT ![node][source] =
          SequenceWithoutIndex(@, laneIndex)]
  /\\ asyncIngressReady' =
       [asyncIngressReady EXCEPT ![node] = ReadyAfterSelectedDrain(node, index)]
DirectCommitCertificateDiscoveryStep(node) ==
  /\\ CommitCertificateDiscoveryDue(node)
  /\\ PublishCommitCertificateRequests(CommitCertificateRequestOutbox(node))
CommitCertificateResponseAuthorized(item) ==
  /\\ item.kind = "CommitCertificateResponse"
  /\\ MatchingCommitCertificateRequests(item) # {}
DrainFairIngressSelected(node) ==
  LET index == FirstDrainableIngressIndex(node)
      laneIndex == SelectedIngressLaneIndex(node, index)
      item == SelectedIngressItemAt(node, index)
  IN /\\ PopSelectedIngress(node, index, laneIndex)
     /\\ CommitCertificateResponseAuthorized(item)
     /\\ discoveredCandidate = CommitCertificateResponseCandidate(item)
     /\\ EnqueueCandidate(discoveredCandidate)
     /\\ CandidateScheduled(candidate)
     /\\ asyncActiveRequests' = asyncActiveRequests \\ MatchingCommitCertificateRequests(item)
DrainHistoricalIngressSelected(node) ==
  LET index == FirstHistoricalDrainableIngressIndex(node)
      laneIndex == HistoricalSelectedIngressLaneIndex(node, index)
      item == HistoricalSelectedIngressItemAt(node, index)
  IN PopSelectedIngress(node, index, laneIndex)
AsyncFairnessAt(initialContext) ==
  /\\ WF_vars(PostGstRunNode(node))
  /\\ WF_vars(PostGstRunHistoricalServer(node))
  /\\ WF_vars(PostGstServiceIoWorker(node))
  /\\ WF_vars(PostGstAdmitHiddenPacket(recipient, source))
AsyncRunnerStep ==
  RunNode(node) \\/ RunHistoricalServer(node)
AsyncNonRunnerStep ==
  AsyncSetGST \\/ AsyncTick \\/ ServiceIoWorker(node)
    \\/ EnqueueIoLocalControl(node) \\/ AsyncNetworkStep \\/ AsyncFaultStep
AsyncNext == AsyncNonCrashStep \\/ PreGstCrash(node)
=============================================================================
"""
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(canonical, encoding="utf-8")
    assert module._async_source_fidelity_errors(formal_dir) == []

    regressed = canonical.replace(
        "AsyncSetGST == /\\ ~gst /\\ SetGST /\\ UNCHANGED AsyncSchedulerVars",
        "AsyncSetGST == /\\ ~gst /\\ SetGST /\\ asyncNodeDeadlines' = Reset",
    ).replace(
        "SendableItems(node) \\cup RetainedProposalChunks(node)",
        "SendableItems(node)",
    ).replace(
        "qc.signers \\ {node}",
        "qc.signers",
    ).replace(
        "  /\\ CommitCertificateResponseItems(job.candidate.item) # {}\n",
        "",
    )
    path.write_text(regressed + "\nnodeHeight == 0\n", encoding="utf-8")
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("AsyncSetGST must equal only" in error for error in errors)
    assert any("RetainedControlEmissionItems must equal only" in error for error in errors)
    assert any("CertifiedRequestOutbox omits" in error for error in errors)
    assert any("ServiceIoWorker omits" in error for error in errors)
    assert any("shadow chain state nodeHeight" in error for error in errors)


    path.write_text(
        canonical.replace(
            "    \\/ ENABLED ExecuteChunkDelivery(selectedCommand)\n",
            "",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("CommandExecutionEnabled must equal only" in error for error in errors)

    path.write_text(
        canonical.replace(
            "selectedCommand \\in {command}:",
            "selectedCommand \\in AsyncCandidateSet:",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("CommandExecutionEnabled must equal only" in error for error in errors)

    path.write_text(
        canonical.replace("  /\\ AsyncCandidateTyped(command)\n", "", 1),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("CommandDispatchable must equal only" in error for error in errors)

    path.write_text(
        canonical.replace(
            "     /\\ CandidateScheduled(candidate)\n"
            "     /\\ asyncActiveRequests' =",
            "     /\\ asyncActiveRequests' =",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "DrainFairIngressSelected omits required production behavior" in error
        for error in errors
    )

    path.write_text(
        canonical.replace(
            "  IN CandidateScheduled(candidate) \\/ "
            "CanEnqueueClass(node, candidate.class)",
            "  IN CanEnqueueClass(node, candidate.class)",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "IngressItemCanDrain omits required production behavior" in error
        for error in errors
    )

    path.write_text(
        canonical.replace(
            "QueuedCandidates \\cup DeferredCandidates\n"
            "                      \\cup CausalCandidates \\cup TrackedWorkCandidates",
            "QueuedCandidates \\cup CausalCandidates \\cup TrackedWorkCandidates",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "ItemInScheduledDelivery omits required production behavior" in error
        for error in errors
    )

    path.write_text(
        canonical.replace(
            "SequenceWithoutIndex(@, laneIndex)",
            "Tail(@)",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "PopSelectedIngress omits required production behavior" in error
        for error in errors
    )


def test_progress_witness_source_fidelity_requires_exact_height(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    path = formal_dir / "SumeragiV2LivenessProofs.tla"
    canonical = r"""---- MODULE SumeragiV2LivenessProofs ----
DecisionPipelineCandidate(node, qc, candidate) ==
  /\ candidate.node = node
  /\ candidate.height = qc.context.height
  /\ candidate.view = qc.view
  /\ CandidateScheduled(candidate)

DecisionCompletionWitness(node, qc) ==
  \E request \in asyncActiveRequests:
    /\ request.kind = "CertifiedRequest"
    /\ request.source = node
    /\ request.envelope.height = qc.context.height
    /\ request.envelope.view = qc.view
=============================================================================
"""
    path.write_text(canonical, encoding="utf-8")
    assert module._progress_witness_source_fidelity_errors(formal_dir) == []

    for equality in (
        "  /\\ candidate.height = qc.context.height\n",
        "    /\\ request.envelope.height = qc.context.height\n",
    ):
        path.write_text(canonical.replace(equality, ""), encoding="utf-8")
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert any(
            "must require exact decision-height ownership" in error
            for error in errors
        )


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
            "AsyncIngressCapacity >=\n"
            "       Cardinality(AsyncIngressSources) + Cardinality(ValidatorIds)",
            "AsyncIngressCapacity >= Cardinality(AsyncIngressSources)",
            1,
        ).replace(
            "/\\ Len(lanes[recipient][source]) = 1\n"
            "           /\\ IngressLaneHasProgressIn(lanes, recipient, source)",
            "/\\ Len(lanes[recipient][source]) = 2\n"
            "           /\\ IngressLaneHasProgressIn(lanes, recipient, source)",
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
            "  /\\ ~NodeHasApplication(node)\n  /\\ \\/ LocalAdmissionStep(node)",
            "  /\\ \\/ LocalAdmissionStep(node)",
        )
        .replace("PostGstRunHistoricalServer(node)", "PostGstRunNode(node)"),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("AsyncStepRefinesCore must equal only" in error for error in errors)
    assert any("RunNode omits required production behavior" in error for error in errors)
    assert any("AsyncFairnessAt omits required production behavior" in error for error in errors)


def test_async_source_fidelity_requires_timeout_signer_deduplication(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
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
    cfg_path.write_text(
        cfg_path.read_text(encoding="utf-8").replace(
            "INVARIANT ReceivedTimeoutVotePoolInvariant\n", "", 1
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("timeout-pool uniqueness must remain a TLC invariant" in error for error in errors)


def test_async_source_fidelity_requires_tc_commit_pool_reconstruction(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
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
            'CausalCandidate("Completion", "RequestCertifiedBody", command)',
            'CausalCandidate("Progress", "RequestCertifiedBody", command)',
            "CommandSuccessors omits required production behavior",
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
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
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
    )
    scheduler_mapping = ",\n       ".join(
        f"{field} <- IndexedScheduler(initialContext, {index})"
        for index, field in enumerate(scheduler_fields, start=1)
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
    refinement = (
        "---- MODULE SumeragiV2ChainEpochRefinement ----\n"
        "CONSTANT VerificationContext\n"
        "IndexedAsync(initialContext) ==\n"
        "  INSTANCE SumeragiV2AsyncNetwork WITH\n"
        f"       {core_mapping},\n       {scheduler_mapping}\n"
        "VerificationCore(component) ==\n"
        "  IndexedCore(VerificationContext, component)\n"
        "VerificationScheduler(component) ==\n"
        "  IndexedScheduler(VerificationContext, component)\n"
        "VerificationAsyncProof ==\n"
        "  INSTANCE SumeragiV2AsyncLivenessProofs WITH\n"
        f"       {verification_core_mapping},\n"
        f"       {verification_scheduler_mapping}\n"
        "IndexedAsyncStateShape ==\n"
        "  /\\ Len(indexedAsyncState[initialContext][1]) = 46\n"
        "  /\\ Len(indexedAsyncState[initialContext][2]) = 31\n"
        "IndexedJoinedNonRunnerStep(initialContext) ==\n"
        "  /\\ TRUE\n"
        "  /\\ UNCHANGED IndexedScheduler(initialContext, 23)\n"
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
=============================================================================
"""
    proof_path.write_text(proof, encoding="utf-8")
    assert module._chain_source_fidelity_errors(formal_dir) == []

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
            "Len(indexedAsyncState[initialContext][2]) = 31",
            "Len(indexedAsyncState[initialContext][2]) = 30",
            1,
        )
        .replace(
            "Len(indexedAsyncState[initialContext][1]) = 46",
            "Len(indexedAsyncState[initialContext][1]) = 45",
            1,
        )
        .replace(
            "UNCHANGED IndexedScheduler(initialContext, 23)",
            "UNCHANGED IndexedScheduler(initialContext, 22)",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("must directly instantiate the authoritative" in error for error in errors)
    assert any("scheduler tuple mapping" in error for error in errors)
    assert any("exact IndexedAsync Core/scheduler tuple" in error for error in errors)
    assert any("stale Core/scheduler tuple arity" in error for error in errors)
    assert any("preserve scheduler slot 23" in error for error in errors)

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
            "IndexedScheduler(VerificationContext, component)",
            "IndexedScheduler(VerificationContext, component + 1)",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("VerificationScheduler must equal only" in error for error in errors)

    path.write_text(
        source.replace("CONSTANT VerificationContext\n", "", 1),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("missing proof-only VerificationContext" in error for error in errors)


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
"""

    errors = module.verus_shortcut_errors(path, source)
    assert len(errors) == 3


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
    assert "SumeragiV2ResumeVoteWitness.tla" in runner
    assert '[[ "$tlc_status" -ne 12 ]]' in runner
    assert "Invariant NoRecoveredHistoricalLockedCommitSigning is violated." in runner
    assert "java -version" in runner
    assert module.REQUIRED_TLC_CONFIG_HEADERS["chain_epoch.cfg"] == (
        "SPECIFICATION ChainEpochSpec"
    )
    assert module.REQUIRED_TLC_CONFIG_HEADERS["liveness.cfg"] == (
        "SPECIFICATION AsyncFiniteSpec"
    )
    assert module.REQUIRED_TLC_CONFIG_HEADERS[
        "resume_locked_commit_witness.cfg"
    ] == "SPECIFICATION CoreSpec"
    assert (module.FORMAL_DIR / "chain_epoch.cfg").read_text().startswith(
        "SPECIFICATION ChainEpochSpec\n"
    )
    assert (module.FORMAL_DIR / "liveness.cfg").read_text().startswith(
        "SPECIFICATION AsyncFiniteSpec\n"
    )
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


def test_tlc_configs_keep_an_externally_invalid_subject(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    for cfg_name in module.REQUIRED_TLC_CONFIGS:
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


def test_formal_gate_validates_fresh_evidence_before_tlc_and_replay() -> None:
    source = (ROOT_DIR / "ci" / "check_sumeragi_formal.sh").read_text()

    structural = source.index("check_sumeragi_v2_proof_ledger.py")
    tlaps = source.index("run_sumeragi_v2_tlaps.sh")
    release = source.index("--release")
    mutations = source.index("run_sumeragi_v2_service_rank_mutation.sh")
    tlc = source.index("run_sumeragi_v2_tlc.sh")
    replay = source.index("check_sumeragi_v2_replay_trace.sh")
    verus = source.index("verify_sumeragi_v2.sh")
    assert structural < tlaps < release < mutations < tlc < replay < verus
    assert "proof_evidence.json" in source

    verus_source = (ROOT_DIR / "scripts" / "verify_sumeragi_v2.sh").read_text()
    unit = verus_source.index("--unit")
    fast_network = verus_source.index("--fast-network")
    backend = verus_source.index("cargo verus verify")
    assert unit < fast_network < backend


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
    taira_source = (
        ROOT_DIR / "integration_tests" / "tests" / "taira_public_localnet.rs"
    ).read_text(encoding="utf-8")

    assert "export IROHA_TEST_REQUIRE_NETWORK=1" in seed_source
    assert "export IROHA_TEST_NETWORK_START_ATTEMPTS=1" in seed_source
    assert "-- --list --ignored" in seed_source
    assert "expected exactly one release test named" in seed_source
    assert 'compute_workspace_source_manifest.py --root "$repo_root"' in seed_source
    assert ".seed-matrix.lock" in seed_source
    assert "COMPLETED.tsv" in seed_source

    required_network = release_source.index("export IROHA_TEST_REQUIRE_NETWORK=1")
    production_units = release_source.index("required_production_liveness_tests")
    taira_rust_contracts = release_source.index(
        "required_taira_release_contract_tests=("
    )
    seed_launcher_preflight = release_source.index("seed_launcher_contract_tests=(")
    taira_soak_preflight = release_source.index("taira_soak_contract_files=(")
    seed_matrix = release_source.index("run_sumeragi_v2_seed_matrix.sh")
    pr_branch = release_source.index('if [[ "$profile" == "--pr" ]]; then')
    pr_fast_formal = release_source.index(
        "run_sumeragi_v2_harness.sh --unit", pr_branch
    )
    formal_gate = release_source.index("check_sumeragi_formal.sh")
    chaos_gate = release_source.index("--chaos-100k")
    pre_soak_manifest = release_source.index("pre_soak_source_manifest_sha256")
    taira_run = release_source.index("run_taira_v2_24h_soak.sh")
    final_manifest = release_source.index("final_release_source_manifest_sha256")
    final_proof_check = release_source.rindex("--release")
    assert (
        required_network
        < production_units
        < taira_rust_contracts
        < seed_launcher_preflight
        < taira_soak_preflight
        < formal_gate
        < seed_matrix
        < chaos_gate
        < pre_soak_manifest
        < taira_run
        < final_manifest
        < final_proof_check
    )
    assert seed_matrix < pr_branch < pr_fast_formal
    production_inventory_end = release_source.index("\n)", production_units)
    production_inventory = tuple(
        line.strip()
        for line in release_source[
            production_units:production_inventory_end
        ].splitlines()
        if line.strip().startswith("sumeragi::")
    )
    assert len(production_inventory) == 115
    assert len(set(production_inventory)) == 115
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
    production_modules_start = release_source.index("production_liveness_modules=(")
    production_modules_end = release_source.index("\n)", production_modules_start)
    production_modules = tuple(
        line.strip()
        for line in release_source[
            production_modules_start:production_modules_end
        ].splitlines()
        if line.strip().startswith("sumeragi::")
    )
    assert len(production_modules) == 11
    assert len(set(production_modules)) == 11
    assert "sumeragi::v2_apply::tests" in production_modules
    assert 'for module in "${production_liveness_modules[@]}"; do' in release_source
    assert (
        'cargo test --locked -p iroha_core --lib "$module" -- --test-threads=1'
        in release_source
    )
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
    assert "missing required production Sumeragi v2 liveness test" in release_source
    assert "production_ignored_unit_list=" in release_source
    assert "required production Sumeragi v2 liveness test is ignored" in release_source
    assert "compute_workspace_source_manifest.py" in release_source
    assert 'compute_workspace_source_manifest.py --root "$repo_root"' in release_source
    assert "IROHA_RELEASE_SOURCE_MANIFEST_SHA256" in release_source
    assert "seed_launcher_contract_tests=(" in release_source
    assert "did not run exactly ten passing tests" in release_source
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
    assert "did not run exactly 38 passing tests" in release_source
    assert 'if [[ "$profile" == "--release" ]]; then' in release_source
    assert "Fail before 128 real-network runs" in release_source
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
    assert "expected exactly 86 Sumeragi v2 reducer unit tests" in harness_source
    assert "reducer unit gate requires all 86 tests to be runnable" in harness_source

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
