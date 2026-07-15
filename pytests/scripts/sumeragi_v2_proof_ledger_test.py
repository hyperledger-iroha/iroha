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
    assert "SumeragiV2TemporalLemmas" in module.RELEASE_PROOF_MODULES
    assert "SumeragiV2ServiceRankLemmas" in module.RELEASE_PROOF_MODULES
    assert "SumeragiV2AsyncLivenessProofs" in module.RELEASE_PROOF_MODULES
    assert "SumeragiV2AsyncNetwork" not in module.RELEASE_PROOF_MODULES


def test_first_release_type_and_height_debt_targets_are_pinned() -> None:
    module = load_checker()
    ledger = module.load_ledger()
    by_id = {obligation["id"]: obligation for obligation in ledger["obligations"]}

    expected_status = {
        "async-type-invariant": "tlaps_proved",
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
TimeoutProtectionProperty(specification) ==
  specification => [](\A tc \in formedTCs: TCProtectsPotentialCommit(tc))
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
TimeoutViewProgressProperty(specification) ==
  specification => \A node \in AsyncCurrentResponsiveVoters,
    roundView \in Views:
      (gst /\ nodeView[node] = roundView /\ ~NodeHasDecision(node))
        ~> (nodeView[node] > roundView \/ NodeHasDecision(node))
RotatingLeaderProgressProperty(specification) ==
  specification => (gst /\ ~ResponsiveNodesDecide) ~> ResponsiveNodesDecide
ApplicationLivenessProperty(specification) ==
  specification => (gst /\ ResponsiveNodesDecide) ~> ResponsiveNodesApply
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
            "specification => (gst /\\ ResponsiveNodesDecide)",
            "specification => (FALSE /\\ gst /\\ ResponsiveNodesDecide)",
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any("ApplicationLivenessProperty must equal only" in error for error in errors)

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
    source = """---- MODULE Example ----
ASSUME Unsafe
AXIOM Hidden
THEOREM Broken == TRUE BY OMITTED
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
AsyncBaseInit == AsyncBaseInitAt(ContextRecord(0, <<>>))
AsyncStepRefinesCore == AsyncNext => [Next]_vars
NextCommandClass(commandClass) ==
  CASE commandClass = "Completion" -> "Progress"
    [] commandClass = "Progress" -> "Normal"
    [] OTHER -> "Completion"
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
AsyncSchedulerVars == <<asyncNextCommandClass>>
AsyncRuntimeInit ==
  asyncNextCommandClass = [node \\in ValidatorIds |-> "Completion"]
AsyncRuntimeScalarTypeInvariant ==
  asyncNextCommandClass \\in [ValidatorIds -> AsyncCommandClasses]
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
IngressItemCanDrain(node, item) ==
  LET candidate == DeliveryCandidate(item)
  IN candidate \\in QueuedCandidates \\/ CanEnqueueClass(node, candidate.class)
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
     /\\ candidate \\in QueuedCandidates
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
            "     /\\ candidate \\in QueuedCandidates\n"
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
            "  IN candidate \\in QueuedCandidates \\/ "
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
    target_core_mapping = core_mapping.replace(
        "initialContext", "LivenessTargetContext"
    )
    target_scheduler_mapping = scheduler_mapping.replace(
        "initialContext", "LivenessTargetContext"
    )
    refinement = (
        "---- MODULE SumeragiV2ChainEpochRefinement ----\n"
        "IndexedAsync(initialContext) ==\n"
        "  INSTANCE SumeragiV2AsyncNetwork WITH\n"
        f"       {core_mapping},\n       {scheduler_mapping}\n"
        "LivenessTargetAsyncProof ==\n"
        "  INSTANCE SumeragiV2AsyncLivenessProofs WITH\n"
        f"       {target_core_mapping},\n"
        f"       {target_scheduler_mapping}\n"
        "IndexedAsyncStateShape ==\n"
        "  /\\ Len(indexedAsyncState[initialContext][1]) = 46\n"
        "  /\\ Len(indexedAsyncState[initialContext][2]) = 30\n"
        "IndexedJoinedNonRunnerStep(initialContext) ==\n"
        "  /\\ TRUE\n"
        "  /\\ UNCHANGED IndexedScheduler(initialContext, 22)\n"
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
            "Len(indexedAsyncState[initialContext][2]) = 30",
            "Len(indexedAsyncState[initialContext][2]) = 29",
            1,
        )
        .replace(
            "Len(indexedAsyncState[initialContext][1]) = 46",
            "Len(indexedAsyncState[initialContext][1]) = 45",
            1,
        )
        .replace(
            "UNCHANGED IndexedScheduler(initialContext, 22)",
            "UNCHANGED IndexedScheduler(initialContext, 21)",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("must directly instantiate the authoritative" in error for error in errors)
    assert any("scheduler tuple mapping" in error for error in errors)
    assert any("exact IndexedAsync Core/scheduler tuple" in error for error in errors)
    assert any("stale Core/scheduler tuple arity" in error for error in errors)
    assert any("preserve scheduler slot 22" in error for error in errors)

    path.write_text(
        source.replace(
            "asyncNextCommandClass <- "
            "IndexedScheduler(LivenessTargetContext, 3)",
            "asyncNextCommandClass <- "
            "IndexedScheduler(LivenessTargetContext, 2)",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any(
        "LivenessTargetAsyncProof must use the exact IndexedAsync" in error
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
        "LivenessTargetAsyncProof must directly instantiate" in error
        for error in errors
    )


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
    assert "old_status -ne 0" in runner
    assert "-fp 96 -seed 139154308881391968" in runner
    assert "Temporal properties were violated." in runner
    assert "Back to state 2" in runner
    assert "old head-only ingress mutation unexpectedly passed" in runner
    assert "State 2: Stuttering" in runner
    assert "old ingress capacity removal mutation unexpectedly passed" in runner
    assert "Invariant OldCapacityInvariant is violated." in runner
    assert "Model checking completed. No error has been found." in runner

    formal_dir = ROOT_DIR / "docs" / "formal" / "sumeragi_v2"
    mutation = (formal_dir / "SumeragiV2ServiceRankMutation.tla").read_text(
        encoding="utf-8"
    )
    assert "EnqueueEqualReplacement" in mutation
    assert "DispatchOldestCopy" in mutation
    assert "AdmitAfterOwnershipEnds" in mutation
    assert (formal_dir / "service_rank_replacement_bug.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION OldSpec\n")
    assert (formal_dir / "service_rank_coalesced.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION CoalescedSpec\n")
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

    required_network = release_source.index("export IROHA_TEST_REQUIRE_NETWORK=1")
    seed_matrix = release_source.index("run_sumeragi_v2_seed_matrix.sh")
    formal_gate = release_source.index("check_sumeragi_formal.sh")
    chaos_gate = release_source.index("--chaos-100k")
    taira_run = release_source.index("run_taira_v2_24h_soak.sh")
    final_manifest = release_source.index("final_release_source_manifest_sha256")
    final_proof_check = release_source.rindex("--release")
    assert (
        required_network
        < seed_matrix
        < formal_gate
        < chaos_gate
        < taira_run
        < final_manifest
        < final_proof_check
    )
    assert "compute_workspace_source_manifest.py" in release_source
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
    assert "expected exactly 66 Sumeragi v2 reducer unit tests" in harness_source
    assert "reducer unit gate requires all 66 tests to be runnable" in harness_source

    replay_branch = harness_source.index("--model-replay)")
    replay_inventory = harness_source.index("model_replay_test_list=", replay_branch)
    replay_ignored_inventory = harness_source.index(
        "replay_ignored_test_list=", replay_inventory
    )
    replay_run = harness_source.index(
        "--test model_trace_replay -- --test-threads=1", replay_ignored_inventory
    )
    assert replay_branch < replay_inventory < replay_ignored_inventory < replay_run
    assert "expected exactly seven Sumeragi v2 model-replay tests" in harness_source
    assert "model-replay gate requires all seven tests to be runnable" in harness_source

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
