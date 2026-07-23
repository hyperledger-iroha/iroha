"""Tests for the fail-closed Sumeragi v2 formal proof ledger gate."""

from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
from dataclasses import replace
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
        Path("crates/iroha_core/src/sumeragi/mod.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_worker.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_runtime.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_effects.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
        Path("crates/iroha_kagami/src/localnet.rs"),
        Path("crates/iroha_config/src/parameters/actual.rs"),
        Path("crates/iroha_config/src/parameters/defaults.rs"),
        Path("crates/iroha_config/src/parameters/user.rs"),
        Path("crates/iroha_crypto/src/lib.rs"),
        Path("crates/iroha_crypto/src/sm.rs"),
        Path("crates/iroha_data_model/src/block/consensus_v2.rs"),
        Path("crates/iroha_p2p/src/lib.rs"),
        Path("crates/iroha_p2p/src/network.rs"),
        Path("crates/iroha_p2p/src/peer.rs"),
        Path("crates/irohad/src/main.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_sumeragi_core/src/verus_proofs.rs"),
        Path("crates/iroha_sumeragi_core/VERIFICATION.md"),
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("scripts/render_taira_validator_bundle.py"),
        Path("scripts/verify_sumeragi_v2.sh"),
        Path("defaults/kagami/iroha3-taira/config.toml"),
        Path("configs/soranexus/taira/config.toml"),
        Path("configs/soranexus/taira/README.md"),
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


def mutate_rust_item_source(
    module,
    path: Path,
    item_name: str,
    old: str,
    new: str,
) -> None:
    """Mutate one exact fragment inside one real named Rust function item."""

    source = path.read_text(encoding="utf-8")
    items = module.rust_items(source, item_name)
    assert len(items) == 1, item_name
    item = items[0]
    assert item.source.count(old) == 1, (item_name, old)
    mutated_item = item.source.replace(old, new, 1)
    assert source.count(item.source) == 1, item_name
    path.write_text(source.replace(item.source, mutated_item, 1), encoding="utf-8")


def mutate_rust_item_source_in_context(
    module,
    path: Path,
    item_name: str,
    brace_context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
) -> None:
    """Mutate one exact Rust item selected by name and brace context."""

    source = path.read_text(encoding="utf-8")
    items = [
        item
        for item in module.rust_items(source, item_name)
        if item.brace_context == brace_context
    ]
    assert len(items) == 1, (item_name, brace_context)
    item = items[0]
    assert item.source.count(old) == 1, (item_name, old)
    mutated_item = item.source.replace(old, new, 1)
    assert source.count(item.source) == 1, item_name
    path.write_text(source.replace(item.source, mutated_item, 1), encoding="utf-8")


def freeze_cross_tool_claim_call_sites(module, claim, root: Path = ROOT_DIR):
    """Seal a claim's current authoritative call items for mutation fixtures."""

    def freeze(call_sites):
        frozen = []
        for call_site in call_sites:
            source = (root / call_site.source).read_text(encoding="utf-8")
            items = [
                item
                for item in module.rust_items(source, call_site.item)
                if item.brace_context == call_site.brace_context
            ]
            assert len(items) == 1, (call_site.source, call_site.item)
            frozen.append(
                replace(
                    call_site,
                    item_token_sha256=module._rust_sealed_item_token_sha256(
                        items[0]
                    ),
                    unfrozen_reason=None,
                )
            )
        return tuple(frozen)

    supplemental = tuple(
        replace(
            kernel,
            production_call_sites=freeze(kernel.production_call_sites),
        )
        for kernel in claim.supplemental_kernels
    )
    return replace(
        claim,
        production_call_sites=freeze(claim.production_call_sites),
        supplemental_kernels=supplemental,
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
        "module": "SumeragiV2ChainLivenessProofs",
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
        "effective-lock-body-acquisition-production-refinement",
        "progress-witness-preservation",
        "progress-witness-production-refinement",
        "protected-service-rank",
        "post-gst-deadlock-freedom",
        "post-gst-starvation-freedom",
        "timeout-view-liveness",
        "locked-body-reproposal-liveness",
        "rotating-leader-liveness",
        "application-liveness",
        "successor-activation-starvation-freedom",
        "successor-activation-exact-recovery-production-refinement",
        "genesis-height-successor-handoff",
        "height-liveness",
    )
    assert module.PROOF_STATUS_DEPENDENCIES == {
        "timeout-protection": ("historical-tc-lock-commit",),
        "effective-lock-body-acquisition-production-refinement": (
            "effective-lock-body-acquisition-model",
        ),
        "async-type-invariant": ("async-runner-scheduler-preservation",),
        "async-progress-ownership-invariant": ("async-type-invariant",),
        "progress-witness-preservation": (
            "async-type-invariant",
            "generation-scoped-vote-delivery",
            "post-decision-timeout-exclusion",
            "decision-recovery-across-restart",
            "effective-lock-body-acquisition-model",
        ),
        "progress-witness-production-refinement": (
            "async-type-invariant",
            "async-progress-ownership-invariant",
            "generation-scoped-vote-delivery",
            "post-decision-timeout-exclusion",
            "decision-recovery-across-restart",
            "async-fair-action-refinement",
            "progress-witness-preservation",
        ),
        "post-gst-deadlock-freedom": (
            "async-type-invariant",
            "async-fair-action-refinement",
            "progress-witness-preservation",
            "protected-service-rank",
        ),
        "protected-service-rank-stage4-ready-causal": (
            "async-type-invariant",
            "async-progress-ownership-invariant",
            "async-fair-action-refinement",
        ),
        "protected-service-rank-serve-fifo": (
            "async-type-invariant",
            "async-fair-action-refinement",
        ),
        "protected-service-rank-stage5-consensus-fifo": (
            "async-type-invariant",
            "async-progress-ownership-invariant",
            "async-fair-action-refinement",
        ),
        "protected-service-rank": (
            "async-type-invariant",
            "async-progress-ownership-invariant",
            "async-fair-action-refinement",
            "protected-service-rank-stage4-ready-causal",
            "protected-service-rank-serve-fifo",
            "protected-service-rank-stage5-consensus-fifo",
        ),
        "post-gst-starvation-freedom": (
            "async-type-invariant",
            "async-fair-action-refinement",
            "protected-service-rank",
        ),
        "timeout-view-liveness": (
            "async-fair-action-refinement",
            "progress-witness-preservation",
            "post-gst-deadlock-freedom",
            "post-gst-starvation-freedom",
        ),
        "locked-body-reproposal-liveness": (
            "effective-lock-body-acquisition-model",
            "effective-lock-body-acquisition-production-refinement",
            "async-fair-action-refinement",
            "progress-witness-preservation",
            "post-gst-starvation-freedom",
            "timeout-view-liveness",
        ),
        "rotating-leader-liveness": (
            "effective-lock-body-acquisition-model",
            "effective-lock-body-acquisition-production-refinement",
            "async-fair-action-refinement",
            "progress-witness-preservation",
            "post-gst-starvation-freedom",
            "timeout-view-liveness",
            "locked-body-reproposal-liveness",
        ),
        "application-liveness": (
            "async-fair-action-refinement",
            "progress-witness-preservation",
            "post-gst-starvation-freedom",
        ),
        "successor-activation-exact-recovery-production-refinement": (
            "epoch-boundary",
            "decision-recovery-across-restart",
            "successor-activation-starvation-freedom",
        ),
        "genesis-height-successor-handoff": (
            "rotating-leader-liveness",
            "application-liveness",
            "successor-activation-starvation-freedom",
        ),
        "height-liveness": (
            "rotating-leader-liveness",
            "application-liveness",
            "successor-activation-starvation-freedom",
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
        "protected-service-rank-stage4-ready-causal",
        "protected-service-rank-serve-fifo",
        "protected-service-rank-stage5-consensus-fifo",
        "protected-service-rank",
        "post-gst-starvation-freedom",
        "timeout-view-liveness",
        "locked-body-reproposal-liveness",
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

    historical_source = module._top_level_operator_body(
        core_source, "HistoricalLockedPrepareSource"
    )
    historical = module._top_level_operator_body(
        core_source, "HistoricalLockedPrepareForCommit"
    )
    provenance = module._top_level_operator_body(
        core_source, "HistoricalLockedPrepareRecoveryProvenance"
    )
    assert historical_source is not None
    assert historical is not None
    assert provenance is not None
    source_body = " ".join(historical_source[0].split())
    historical_body = " ".join(historical[0].split())
    for required in (
        "qc \\in prepareQCs",
        "qc.view < nodeView[node]",
        "qc.view = lockRank[node]",
        "qc.subject = lockSubject[node]",
    ):
        assert required in source_body
    assert "InstalledTcSelectsPrepareFor(node, qc)" in " ".join(
        provenance[0].split()
    )
    assert "ExactLockedCommitIntents(node, qc.view, qc.subject) = {}" in (
        historical_body
    )
    assert "NoHigherPrepareOriginKnown(node, qc)" in historical_body

    origin_fence = module._top_level_operator_body(
        core_source, "NoHigherPrepareOriginKnown"
    )
    assert origin_fence is not None
    origin_body = " ".join(origin_fence[0].split())
    assert "vote \\in prepareIntents" in origin_body
    assert "vote.view > qc.view" in origin_body
    assert "highestRank[node] > qc.view" in origin_body
    assert "vote.subject" not in origin_body
    assert "highestSubject" not in origin_body

    begin = module._top_level_operator_body(core_source, "BeginLockCommit")
    persist = module._top_level_operator_body(core_source, "PersistLockCommit")
    assert begin is not None
    assert persist is not None
    begin_body = " ".join(begin[0].split())
    persist_body = " ".join(persist[0].split())
    assert "CurrentOpenPrepareForCommit(node, qc)" in begin_body
    assert "HistoricalLockedPrepareForCommit(node, qc)" in begin_body
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
        ("post-gst-deadlock-freedom", "progress-witness-preservation"),
        ("post-gst-deadlock-freedom", "protected-service-rank"),
        ("timeout-view-liveness", "post-gst-deadlock-freedom"),
    ):
        original_dependent_status = by_id[dependent_id]["status"]
        original_prerequisite_status = by_id[prerequisite_id]["status"]
        by_id[dependent_id]["status"] = "tlaps_proved"
        by_id[prerequisite_id]["status"] = "specified_unproved"
        errors = module._proof_status_dependency_errors(obligations)
        assert (
            f"proof obligation {dependent_id} cannot be tlaps_proved before "
            f"prerequisite {prerequisite_id} is tlaps_proved"
        ) in errors
        by_id[dependent_id]["status"] = original_dependent_status
        by_id[prerequisite_id]["status"] = original_prerequisite_status

    by_id["progress-witness-production-refinement"]["status"] = (
        "cross_tool_proved"
    )
    errors = module._proof_status_dependency_errors(obligations)
    assert (
        "proof obligation progress-witness-production-refinement cannot be "
        "cross_tool_proved before prerequisite progress-witness-preservation "
        "is proved"
    ) in errors
    by_id["progress-witness-production-refinement"]["status"] = (
        "specified_unproved"
    )

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
        (
            "progress-witness-production-refinement",
            "progress-witness-preservation",
        ),
        ("post-gst-deadlock-freedom", "protected-service-rank"),
        ("timeout-view-liveness", "post-gst-deadlock-freedom"),
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


def test_chain_liveness_dependencies_do_not_alias_safety_as_recovery_progress() -> None:
    """The production safety bridge is not a temporal recovery theorem."""

    module = load_checker()

    for dependent_id in (
        "genesis-height-successor-handoff",
        "height-liveness",
    ):
        dependencies = module.PROOF_STATUS_DEPENDENCIES[dependent_id]
        assert "successor-activation-starvation-freedom" in dependencies
        assert (
            "successor-activation-exact-recovery-production-refinement"
            not in dependencies
        )


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


def complete_cross_tool_ledger(module):
    """Return a synthetic complete ledger using the reviewed cross-tool status."""

    ledger = copy.deepcopy(module.load_ledger())
    ledger["machine_checked_completion"] = True
    cross_tool_ids = set(module.CROSS_TOOL_REFINEMENT_BY_ID)
    for obligation in ledger["obligations"]:
        if obligation["id"] in cross_tool_ids:
            obligation["status"] = "cross_tool_proved"
        elif obligation["status"] == "specified_unproved":
            obligation["status"] = "tlaps_proved"
    return ledger


def build_cross_tool_fixture(module, tmp_path: Path):
    """Build canonical synthetic component logs for checker-only negative tests."""

    # Materialize compact exact non-vacuous synthetic contracts so the
    # promotion validator and every mutation below run through the full
    # signature/kernel/call-site path without duplicating production sources.
    hardened_contracts = []
    shared_kernel_source = "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS:
        claims = []
        for claim in contract.claims:
            kernel = f"synthetic_{claim.verus_theorem}_kernel"
            projection_builder = f"synthetic_{claim.verus_theorem}_projection"
            projection_builder_source = (
                f"pub closed spec fn {projection_builder}(projection: u64) "
                "-> u64 { projection }"
            )
            projection_builder_sha256 = hashlib.sha256(
                "\0".join(
                    module.rust_code_tokens(projection_builder_source)
                ).encode("utf-8")
            ).hexdigest()
            call_source = claim.production_sources[0]
            call_item = f"enforce_{claim.verus_theorem}"
            call_expression = f"assert!({kernel}(projection));"
            synthetic_call_source = (
                f"fn {call_item}(projection: u64) {{\n"
                f"    {call_expression}\n"
                "}\n"
            )
            extracted_call_items = module.rust_items(
                synthetic_call_source, call_item
            )
            assert len(extracted_call_items) == 1
            call_item_sha256 = module._rust_sealed_item_token_sha256(
                extracted_call_items[0]
            )
            claims.append(
                module.CrossToolClaimContract(
                    constant=claim.constant,
                    verus_theorem=claim.verus_theorem,
                    verus_source=claim.verus_source,
                    production_sources=claim.production_sources,
                    verus_parameters="projection: u64",
                    verus_requires="projection > 0",
                    verus_ensures=(
                        f"{kernel}({projection_builder}(projection)), "
                        f"{projection_builder}(projection) >= 1"
                    ),
                    verified_kernel=kernel,
                    verified_kernel_source=shared_kernel_source,
                    verified_kernel_parameters="projection: u64",
                    verified_kernel_body="projection > 0",
                    theorem_kernel_projection=(
                        f"{projection_builder}(projection)"
                    ),
                    theorem_projection_builder=projection_builder,
                    theorem_projection_builder_parameters="projection: u64",
                    theorem_projection_builder_return="u64",
                    theorem_projection_builder_item_sha256=(
                        projection_builder_sha256
                    ),
                    production_call_sites=(
                        module.CrossToolProductionCallContract(
                            source=call_source,
                            item=call_item,
                            projection="projection",
                            required_expression=call_expression,
                            item_token_sha256=call_item_sha256,
                        ),
                    ),
                )
            )
        hardened_contracts.append(
            module.CrossToolObligationContract(
                obligation_id=contract.obligation_id,
                module=contract.module,
                ledger_symbol=contract.ledger_symbol,
                tla_theorem=contract.tla_theorem,
                tla_statement=contract.tla_statement,
                claims=tuple(claims),
                ledger_declaration_kind=contract.ledger_declaration_kind,
                ledger_statement=contract.ledger_statement,
                tla_proof=contract.tla_proof,
            )
        )
    module.CROSS_TOOL_REFINEMENT_CONTRACTS = tuple(hardened_contracts)
    module.CROSS_TOOL_REFINEMENT_BY_ID = {
        contract.obligation_id: contract
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
    }

    ledger = complete_cross_tool_ledger(module)
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    shutil.copytree(
        module.FORMAL_DIR,
        formal_dir,
        ignore=shutil.ignore_patterns(".tlacache"),
    )

    contracts_by_module = {}
    for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS:
        contracts_by_module.setdefault(contract.module, []).append(contract)
    for module_name, contracts in contracts_by_module.items():
        path = formal_dir / f"{module_name}.tla"
        source = path.read_text(encoding="utf-8")
        model_side_declarations = ""
        for contract in contracts:
            premise = contract.tla_statement.split(" => ", maxsplit=1)[0]
            if module._expanded_tla_alias(
                source, premise
            ) == module._expanded_tla_alias(source, contract.ledger_symbol):
                synthetic = f"{contract.tla_theorem}SyntheticModelSide"
                old = f"THEOREM {contract.ledger_symbol} ==\n  {premise}"
                assert source.count(old) == 1
                source = source.replace(
                    old,
                    f"THEOREM {contract.ledger_symbol} ==\n"
                    f"  /\\ {premise}\n"
                    f"  /\\ {synthetic}",
                    1,
                )
                model_side_declarations += f"\n{synthetic} == FALSE\n"
        end = source.rfind("====")
        assert end >= 0
        declarations = model_side_declarations + "".join(
            "\nTHEOREM "
            f"{contract.tla_theorem} ==\n"
            f"  {contract.tla_statement}\n"
            "PROOF\n"
            "  OBVIOUS\n"
            for contract in contracts
            if module._top_level_theorem_body(source, contract.tla_theorem) is None
        )
        path.write_text(source[:end] + declarations + "\n====\n", encoding="utf-8")

    verus_contract = module._verus_evidence_contract_module()
    production_sources = {
        relative
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
        for claim in contract.claims
        for relative in claim.production_sources
    }
    copied_sources = set(verus_contract.REQUIRED_SOURCE_PATHS) | production_sources
    for relative in sorted(copied_sources):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        source = ROOT_DIR / relative
        if source.is_file():
            shutil.copyfile(source, destination)
        else:
            # The fixture exercises the evidence schema independently of
            # unrelated source-inventory migrations in the shared worktree.
            destination.write_text("// synthetic fixture source\n", encoding="utf-8")

    theorem_claims_by_source = {}
    for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS:
        for claim in contract.claims:
            theorem_claims_by_source.setdefault(claim.verus_source, []).append(
                claim
            )
    for relative, claims in theorem_claims_by_source.items():
        path = tmp_path / relative
        source = ""
        synthetic_proofs = "\nverus! {\n"
        for claim in claims:
            expected_call = (
                f"{claim.verified_kernel}({claim.theorem_kernel_projection})"
            )
            synthetic_proofs += (
                f"pub closed spec fn {claim.theorem_projection_builder}("
                f"{claim.theorem_projection_builder_parameters}) -> "
                f"{claim.theorem_projection_builder_return} {{\n"
                "    projection\n"
                "}\n"
                f"pub closed spec fn {claim.verified_kernel}("
                f"{claim.verified_kernel_parameters}) -> bool {{\n"
                f"    {claim.verified_kernel_body}\n"
                "}\n"
                f"pub proof fn {claim.verus_theorem}({claim.verus_parameters})\n"
                f"    requires {claim.verus_requires},\n"
                f"    ensures {claim.verus_ensures},\n"
                "{\n"
                f"    assert({expected_call});\n"
                "}\n"
            )
        synthetic_proofs += "}\n"
        path.write_text(source + synthetic_proofs, encoding="utf-8")

    kernel_path = tmp_path / shared_kernel_source
    kernel_source = kernel_path.read_text(encoding="utf-8")
    kernel_source += "\n" + "".join(
        f"pub(crate) const fn {claim.verified_kernel}"
        f"({claim.verified_kernel_parameters}) -> bool {{\n"
        f"    {claim.verified_kernel_body}\n"
        "}\n"
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
        for claim in contract.claims
    )
    kernel_path.write_text(kernel_source, encoding="utf-8")

    for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS:
        for claim in contract.claims:
            for call_site in claim.production_call_sites:
                path = tmp_path / call_site.source
                source = path.read_text(encoding="utf-8")
                source += (
                    "\n"
                    f"fn {call_site.item}(projection: u64) {{\n"
                    f"    {call_site.required_expression}\n"
                    "}\n"
                )
                path.write_text(source, encoding="utf-8")

    # Cross-tool release evidence must describe the exact ledger that is part
    # of the source-bound checkout, not a separately supplied archive mutant.
    (formal_dir / "proof_coverage.json").write_text(
        json.dumps(ledger, indent=2) + "\n",
        encoding="utf-8",
    )

    log_dir = tmp_path / "target" / "formal" / "sumeragi_v2" / "tlaps"
    log_dir.mkdir(parents=True)
    formal_manifest_sha256 = module._formal_source_manifest(
        formal_dir, tmp_path
    )["sha256"]
    for name in module.RELEASE_PROOF_MODULES:
        (log_dir / f"{name}.log").write_text(
            "[INFO]: All 1 obligation proved.\n"
            f"{module._tlapm_runner_marker(name, formal_manifest_sha256)}\n",
            encoding="utf-8",
        )
    tlaps_evidence = module.build_release_evidence(
        tlapm_version=module.TLAPM_COMMIT[:7],
        log_dir=log_dir,
        formal_dir=formal_dir,
        root_dir=tmp_path,
    )

    host = verus_contract._host_key()
    if host not in verus_contract.EXPECTED_TOOL_SHA256:
        pytest.skip(f"cross-tool evidence fixture has no pinned Verus host {host}")
    pinned_tool = verus_contract.EXPECTED_TOOL_SHA256[host]
    workspace_manifest_sha256 = "a" * 64
    nonce = "b" * 64
    verus_log = tmp_path / verus_contract.EXPECTED_LOG_PATH
    verus_log.parent.mkdir(parents=True, exist_ok=True)
    verus_log.write_text(
        verus_contract.begin_marker(nonce, workspace_manifest_sha256)
        + "\n"
        + "verification results:: "
        + f"{verus_contract.EXPECTED_DEPENDENCY_VERIFIED} verified, 0 errors\n"
        + "verification results:: "
        + f"{verus_contract.EXPECTED_ROOT_VERIFIED} verified, 0 errors\n"
        + verus_contract.success_marker(nonce, workspace_manifest_sha256)
        + "\n",
        encoding="utf-8",
    )
    verus_evidence = {
        "schema_version": verus_contract.SCHEMA_VERSION,
        "verification_contract_sha256": verus_contract.verification_contract_sha256(),
        "source_manifest_sha256": workspace_manifest_sha256,
        "sources": verus_contract._source_entries(tmp_path),
        "tool": {
            "version": verus_contract.EXPECTED_VERUS_VERSION,
            "platform": pinned_tool["platform"],
            "verus_sha256": pinned_tool["verus"],
            "cargo_verus_sha256": pinned_tool["cargo_verus"],
        },
        "invocation": list(verus_contract.EXPECTED_INVOCATION),
        "log": verus_contract.EXPECTED_LOG_PATH,
        "log_sha256": module._sha256_file(verus_log),
        "nonce": nonce,
        "results": {
            "dependency_verified": verus_contract.EXPECTED_DEPENDENCY_VERIFIED,
            "root_verified": verus_contract.EXPECTED_ROOT_VERIFIED,
            "errors": 0,
        },
        "backend_verification": True,
    }
    cross_tool_evidence = module.build_cross_tool_evidence(
        ledger,
        tlaps_evidence=tlaps_evidence,
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest_sha256,
    )
    return (
        ledger,
        formal_dir,
        tlaps_evidence,
        verus_evidence,
        cross_tool_evidence,
        workspace_manifest_sha256,
    )


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


def test_cross_tool_status_is_fail_closed_and_production_only() -> None:
    module = load_checker()
    ledger = module.load_ledger()

    assert ledger["schema_version"] == module.LEDGER_SCHEMA_VERSION == 2
    assert ledger["status_values"] == list(module.STATUS_VALUES)
    assert [
        len(contract.claims)
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
    ] == [4, 7, 6]
    assert module._cross_tool_contract_errors() == []
    production_call_sites = [
        call_site
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
        for claim in contract.claims
        for call_site in (
            *claim.production_call_sites,
            *(
                call_site
                for kernel in claim.supplemental_kernels
                for call_site in kernel.production_call_sites
            ),
        )
    ]
    assert len(production_call_sites) == 24
    sealed_call_sites = sum(
        call_site.item_token_sha256 is not None
        for call_site in production_call_sites
    )
    assert sealed_call_sites == 17
    promotion_errors = module._cross_tool_promotion_contract_errors(
        module.CROSS_TOOL_REFINEMENT_CONTRACTS
    )
    assert len(promotion_errors) == 7
    assert all(
        "remains intentionally unfrozen" in error for error in promotion_errors
    )
    canonical_contracts = module.CROSS_TOOL_REFINEMENT_CONTRACTS
    first = canonical_contracts[0]
    tautological = module.CrossToolObligationContract(
        obligation_id=first.obligation_id,
        module=first.module,
        ledger_symbol=first.ledger_symbol,
        tla_theorem=first.tla_theorem,
        tla_statement=(
            "ProductionEffectiveLockBodyAcquisitionRefinement => "
            "ProductionEffectiveLockBodyAcquisitionRefinement"
        ),
        claims=first.claims,
    )
    module.CROSS_TOOL_REFINEMENT_CONTRACTS = (
        tautological,
        *canonical_contracts[1:],
    )
    assert any(
        "must imply its exact ledger theorem symbol" in error
        for error in module._cross_tool_contract_errors()
    )
    module.CROSS_TOOL_REFINEMENT_CONTRACTS = canonical_contracts
    by_id = {obligation["id"]: obligation for obligation in ledger["obligations"]}
    assert all(
        by_id[obligation_id]["status"] == "specified_unproved"
        for obligation_id in module.CROSS_TOOL_REFINEMENT_BY_ID
    )
    assert module._cross_tool_evidence_errors(
        ledger,
        {},
        tlaps_evidence=None,
        verus_evidence=None,
    ) == [
        "cross-tool evidence is forbidden when no obligation is cross_tool_proved"
    ]
    with pytest.raises(ValueError, match="no cross_tool_proved obligations"):
        module.build_cross_tool_evidence(
            ledger,
            tlaps_evidence={},
            verus_evidence={},
        )

    forged = copy.deepcopy(ledger)
    next(
        obligation
        for obligation in forged["obligations"]
        if obligation["id"] == "dual-quorum-definition"
    )["status"] = "cross_tool_proved"
    errors = module.validate_ledger(forged, check_retired_paths=False).errors
    assert any(
        "uses cross_tool_proved outside the reviewed production refinement inventory"
        in error
        for error in errors
    )

    tlaps_only = copy.deepcopy(ledger)
    next(
        obligation
        for obligation in tlaps_only["obligations"]
        if obligation["id"]
        == "effective-lock-body-acquisition-production-refinement"
    )["status"] = "tlaps_proved"
    errors = module.validate_ledger(tlaps_only, check_retired_paths=False).errors
    assert any(
        "cannot be promoted with TLAPS evidence alone" in error for error in errors
    )

    legacy_schema = copy.deepcopy(ledger)
    legacy_schema["schema_version"] = 1
    errors = module.validate_ledger(
        legacy_schema,
        check_retired_paths=False,
    ).errors
    assert "proof ledger schema_version must equal 2" in errors


def test_cross_tool_promotion_rejects_vacuity_kernel_only_and_missing_call_sites() -> None:
    module = load_checker()
    contracts = module.CROSS_TOOL_REFINEMENT_CONTRACTS
    obligation = contracts[0]
    claim = obligation.claims[0]
    assert claim.verified_kernel is not None
    assert claim.theorem_kernel_projection is not None
    assert claim.verus_requires is not None

    def errors_for(mutated_claim):
        mutated_obligation = replace(
            obligation,
            claims=(mutated_claim, *obligation.claims[1:]),
        )
        return module._cross_tool_promotion_contract_errors(
            (mutated_obligation, *contracts[1:])
        )

    errors = errors_for(replace(claim, verus_ensures="true"))
    assert any("vacuous or constant Verus ensures clause" in error for error in errors)

    errors = errors_for(replace(claim, verus_ensures=claim.verus_requires))
    assert any("must not restate its exact requires clause" in error for error in errors)

    exact_kernel_only = (
        f"{claim.verified_kernel}({claim.theorem_kernel_projection})"
    )
    errors = errors_for(replace(claim, verus_ensures=exact_kernel_only))
    assert any(
        "reviewed nontrivial postcondition beyond its kernel" in error
        for error in errors
    )

    errors = errors_for(replace(claim, production_call_sites=()))
    assert any("no authoritative production kernel call sites" in error for error in errors)

    call_site = claim.production_call_sites[0]
    errors = errors_for(
        replace(
            claim,
            production_call_sites=(
                replace(
                    call_site,
                    item_token_sha256=None,
                    unfrozen_reason=None,
                ),
            ),
        )
    )
    assert any("lacks an exact item token seal" in error for error in errors)

    errors = errors_for(
        replace(
            claim,
            production_call_sites=(
                replace(call_site, item_token_sha256="0" * 63),
            ),
        )
    )
    assert any("invalid exact item token seal" in error for error in errors)


def test_cross_tool_contract_rejects_call_source_missing_from_verus_inventory() -> None:
    """Every authoritative production call item is part of Verus evidence."""

    module = load_checker()
    verus_contract = module._verus_evidence_contract_module()
    canonical_sources = verus_contract.REQUIRED_SOURCE_PATHS
    assert "crates/irohad/src/main.rs" in canonical_sources
    try:
        verus_contract.REQUIRED_SOURCE_PATHS = tuple(
            source
            for source in canonical_sources
            if source != "crates/irohad/src/main.rs"
        )
        errors = module._cross_tool_contract_errors()
    finally:
        verus_contract.REQUIRED_SOURCE_PATHS = canonical_sources

    assert any(
        "authoritative production call sources outside the Verus evidence inventory"
        in error
        and "crates/irohad/src/main.rs" in error
        for error in errors
    )


def test_cross_tool_obligation_query_is_dormant_canonical_and_fail_closed(
    tmp_path: Path,
) -> None:
    module = load_checker()
    ledger_path = tmp_path / "proof_coverage.json"
    ledger_path.write_text(
        json.dumps(module.load_ledger()),
        encoding="utf-8",
    )
    dormant = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--ledger",
            str(ledger_path),
            "--print-cross-tool-obligations",
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert dormant.returncode == 0, dormant.stderr
    assert dormant.stdout.strip() == ""

    complete = complete_cross_tool_ledger(module)
    ledger_path.write_text(json.dumps(complete), encoding="utf-8")
    required = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--ledger",
            str(ledger_path),
            "--print-cross-tool-obligations",
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert required.returncode == 0, required.stderr
    assert required.stdout.splitlines() == [
        contract.obligation_id
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
    ]

    next(
        obligation
        for obligation in complete["obligations"]
        if obligation["id"] == "dual-quorum-definition"
    )["status"] = "cross_tool_proved"
    ledger_path.write_text(json.dumps(complete), encoding="utf-8")
    unreviewed = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "--ledger",
            str(ledger_path),
            "--print-cross-tool-obligations",
        ],
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    assert unreviewed.returncode == 1
    assert "unreviewed cross_tool_proved obligations" in unreviewed.stderr


def test_cross_tool_release_requires_linked_evidence(tmp_path: Path) -> None:
    module = load_checker()
    (
        ledger,
        formal_dir,
        tlaps_evidence,
        verus_evidence,
        cross_tool_evidence,
        workspace_manifest,
    ) = build_cross_tool_fixture(module, tmp_path)

    assert (
        cross_tool_evidence["schema_version"]
        == module.CROSS_TOOL_EVIDENCE_SCHEMA_VERSION
        == 2
    )
    assert module._release_evidence_errors(
        ledger,
        tlaps_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
    ) == []
    assert module._cross_tool_evidence_errors(
        ledger,
        cross_tool_evidence,
        tlaps_evidence=tlaps_evidence,
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest,
    ) == []
    assert module._cross_tool_evidence_errors(
        ledger,
        None,
        tlaps_evidence=tlaps_evidence,
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest,
    ) == ["release gate requires cross-tool refinement evidence"]
    assert module._cross_tool_evidence_errors(
        ledger,
        cross_tool_evidence,
        tlaps_evidence=tlaps_evidence,
        verus_evidence=None,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest,
    ) == ["cross-tool refinement requires linked Verus evidence"]

    unreviewed = copy.deepcopy(ledger)
    next(
        obligation
        for obligation in unreviewed["obligations"]
        if obligation["id"] == "dual-quorum-definition"
    )["status"] = "cross_tool_proved"
    with pytest.raises(ValueError, match="reviewed canonical selection"):
        module.build_cross_tool_evidence(
            unreviewed,
            tlaps_evidence=tlaps_evidence,
            verus_evidence=verus_evidence,
            formal_dir=formal_dir,
            root_dir=tmp_path,
            expected_verus_source_manifest_sha256=workspace_manifest,
        )

    verus_contract = module._verus_evidence_contract_module()
    canonical_log = tmp_path / verus_contract.EXPECTED_LOG_PATH
    archived_log = tmp_path / "archive" / "verus.log"
    archived_log.parent.mkdir()
    shutil.move(canonical_log, archived_log)
    assert module._cross_tool_evidence_errors(
        ledger,
        cross_tool_evidence,
        tlaps_evidence=tlaps_evidence,
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest,
    )
    assert module._cross_tool_evidence_errors(
        ledger,
        cross_tool_evidence,
        tlaps_evidence=tlaps_evidence,
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest,
        verus_log_path=archived_log,
    ) == []


def test_cross_tool_evidence_rejects_every_missing_or_substituted_claim(
    tmp_path: Path,
) -> None:
    module = load_checker()
    (
        ledger,
        formal_dir,
        tlaps_evidence,
        verus_evidence,
        cross_tool_evidence,
        workspace_manifest,
    ) = build_cross_tool_fixture(module, tmp_path)

    def errors_for(mutant):
        return module._cross_tool_evidence_errors(
            ledger,
            mutant,
            tlaps_evidence=tlaps_evidence,
            verus_evidence=verus_evidence,
            formal_dir=formal_dir,
            root_dir=tmp_path,
            expected_verus_source_manifest_sha256=workspace_manifest,
        )

    for obligation_index, obligation in enumerate(
        cross_tool_evidence["obligations"]
    ):
        for claim_index, _ in enumerate(obligation["claims"]):
            substituted = copy.deepcopy(cross_tool_evidence)
            substituted["obligations"][obligation_index]["claims"][claim_index][
                "constant"
            ] += "Substituted"
            assert errors_for(substituted)

        missing = copy.deepcopy(cross_tool_evidence)
        missing["obligations"][obligation_index]["claims"].pop()
        assert errors_for(missing)

    substituted_theorem = copy.deepcopy(cross_tool_evidence)
    substituted_theorem["obligations"][0]["claims"][0][
        "verus_theorem"
    ] += "_substituted"
    assert errors_for(substituted_theorem)

    mismatched_source = copy.deepcopy(cross_tool_evidence)
    mismatched_source["obligations"][1]["claims"][0]["production_sources"][0][
        "sha256"
    ] = "0" * 64
    assert errors_for(mismatched_source)


def test_cross_tool_evidence_rejects_tool_log_manifest_and_ledger_substitution(
    tmp_path: Path,
) -> None:
    module = load_checker()
    (
        ledger,
        formal_dir,
        tlaps_evidence,
        verus_evidence,
        cross_tool_evidence,
        workspace_manifest,
    ) = build_cross_tool_fixture(module, tmp_path)

    def errors_for(
        *,
        cross=cross_tool_evidence,
        tlaps=tlaps_evidence,
        verus=verus_evidence,
        changed_ledger=ledger,
    ):
        return module._cross_tool_evidence_errors(
            changed_ledger,
            cross,
            tlaps_evidence=tlaps,
            verus_evidence=verus,
            formal_dir=formal_dir,
            root_dir=tmp_path,
            expected_verus_source_manifest_sha256=workspace_manifest,
        )

    missing_field = copy.deepcopy(cross_tool_evidence)
    missing_field.pop("tools")
    assert errors_for(cross=missing_field)

    substituted_link = copy.deepcopy(cross_tool_evidence)
    substituted_link["component_evidence"]["verus_sha256"] = "0" * 64
    assert errors_for(cross=substituted_link)

    substituted_manifest = copy.deepcopy(cross_tool_evidence)
    substituted_manifest["source_manifests"]["formal_sha256"] = "0" * 64
    assert errors_for(cross=substituted_manifest)

    missing_dependency = copy.deepcopy(cross_tool_evidence)
    missing_dependency["obligations"][1]["dependencies"].pop(0)
    assert errors_for(cross=missing_dependency)

    substituted_tlaps = copy.deepcopy(tlaps_evidence)
    substituted_tlaps["tool"]["commit"] = "0" * 40
    assert errors_for(tlaps=substituted_tlaps)

    substituted_verus = copy.deepcopy(verus_evidence)
    substituted_verus["tool"]["version"] = "forged"
    assert errors_for(verus=substituted_verus)

    changed_ledger = copy.deepcopy(ledger)
    changed_ledger["obligations"][0]["requirement"] += " drift"
    assert errors_for(changed_ledger=changed_ledger)
    with pytest.raises(ValueError, match="source-bound canonical proof ledger"):
        module.build_cross_tool_evidence(
            changed_ledger,
            tlaps_evidence=tlaps_evidence,
            verus_evidence=verus_evidence,
            formal_dir=formal_dir,
            root_dir=tmp_path,
            expected_verus_source_manifest_sha256=workspace_manifest,
        )

    first_log = (
        tmp_path
        / cross_tool_evidence["obligations"][0]["tla"]["log"]
    )
    first_log.write_text(first_log.read_text(encoding="utf-8") + "stale\n")
    assert errors_for()


def test_cross_tool_evidence_rejects_stale_production_source(tmp_path: Path) -> None:
    module = load_checker()
    (
        ledger,
        formal_dir,
        tlaps_evidence,
        verus_evidence,
        cross_tool_evidence,
        workspace_manifest,
    ) = build_cross_tool_fixture(module, tmp_path)
    relative = cross_tool_evidence["obligations"][0]["claims"][0][
        "production_sources"
    ][0]["path"]
    path = tmp_path / relative
    path.write_text(path.read_text(encoding="utf-8") + "\n// stale\n", encoding="utf-8")

    errors = module._cross_tool_evidence_errors(
        ledger,
        cross_tool_evidence,
        tlaps_evidence=tlaps_evidence,
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest,
    )
    assert errors


def test_cross_tool_evidence_rejects_named_verus_result_substitution(
    tmp_path: Path,
) -> None:
    module = load_checker()
    (
        ledger,
        formal_dir,
        tlaps_evidence,
        verus_evidence,
        cross_tool_evidence,
        workspace_manifest,
    ) = build_cross_tool_fixture(module, tmp_path)
    claim = module.CROSS_TOOL_REFINEMENT_CONTRACTS[0].claims[0]
    path = tmp_path / claim.verus_source
    source = path.read_text(encoding="utf-8")
    old = (
        f"pub proof fn {claim.verus_theorem}"
        f"({claim.verus_parameters})"
    )
    assert source.count(old) == 1
    path.write_text(
        source.replace(
            old,
            old.replace(claim.verus_theorem, claim.verus_theorem + "_substituted"),
            1,
        ),
        encoding="utf-8",
    )
    for entry in verus_evidence["sources"]:
        if entry["path"] == claim.verus_source:
            entry["sha256"] = module._sha256_file(path)
            break
    else:
        raise AssertionError(claim.verus_source)

    errors = module._cross_tool_evidence_errors(
        ledger,
        cross_tool_evidence,
        tlaps_evidence=tlaps_evidence,
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest,
    )
    assert any("requires exactly one named Verus theorem" in error for error in errors)

    path.write_text(
        source.replace(old, f"#[cfg(any())]\n{old}", 1),
        encoding="utf-8",
    )
    for entry in verus_evidence["sources"]:
        if entry["path"] == claim.verus_source:
            entry["sha256"] = module._sha256_file(path)
            break
    errors = module._cross_tool_evidence_errors(
        ledger,
        cross_tool_evidence,
        tlaps_evidence=tlaps_evidence,
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest,
    )
    assert any("may not be gated or rewritten" in error for error in errors)


def test_cross_tool_evidence_rejects_vacuous_and_disconnected_verus_results(
    tmp_path: Path,
) -> None:
    module = load_checker()
    (
        ledger,
        formal_dir,
        tlaps_evidence,
        verus_evidence,
        cross_tool_evidence,
        workspace_manifest,
    ) = build_cross_tool_fixture(module, tmp_path)
    claim = module.CROSS_TOOL_REFINEMENT_CONTRACTS[0].claims[0]
    path = tmp_path / claim.verus_source
    canonical = path.read_text(encoding="utf-8")

    def errors_for(source: str):
        path.write_text(source, encoding="utf-8")
        for entry in verus_evidence["sources"]:
            if entry["path"] == claim.verus_source:
                entry["sha256"] = module._sha256_file(path)
                break
        else:
            raise AssertionError(claim.verus_source)
        return module._cross_tool_evidence_errors(
            ledger,
            cross_tool_evidence,
            tlaps_evidence=tlaps_evidence,
            verus_evidence=verus_evidence,
            formal_dir=formal_dir,
            root_dir=tmp_path,
            expected_verus_source_manifest_sha256=workspace_manifest,
        )

    exact_ensures = f"ensures {claim.verus_ensures},"
    assert canonical.count(exact_ensures) == 1
    errors = errors_for(canonical.replace(exact_ensures, "ensures true,", 1))
    assert any("exact normalized signature" in error for error in errors)

    exact_assertion = (
        f"assert({claim.verified_kernel}({claim.theorem_kernel_projection}));"
    )
    assert canonical.count(exact_assertion) == 1
    errors = errors_for(
        canonical.replace(exact_assertion, "assert(projection > 0);", 1)
    )
    assert any("must invoke its verified kernel" in error for error in errors)

    exact_parameter = (
        f"pub proof fn {claim.verus_theorem}({claim.verus_parameters})"
    )
    assert canonical.count(exact_parameter) == 1
    errors = errors_for(
        canonical.replace(
            exact_parameter,
            f"pub proof fn {claim.verus_theorem}(projection: u32)",
            1,
        )
    )
    assert any("exact normalized signature" in error for error in errors)


def test_cross_tool_evidence_rejects_constant_shared_kernel(tmp_path: Path) -> None:
    module = load_checker()
    (
        ledger,
        formal_dir,
        tlaps_evidence,
        verus_evidence,
        cross_tool_evidence,
        workspace_manifest,
    ) = build_cross_tool_fixture(module, tmp_path)
    claim = module.CROSS_TOOL_REFINEMENT_CONTRACTS[0].claims[0]
    assert claim.verified_kernel_source is not None
    path = tmp_path / claim.verified_kernel_source
    source = path.read_text(encoding="utf-8")
    old = (
        f"pub(crate) const fn {claim.verified_kernel}"
        f"({claim.verified_kernel_parameters}) -> bool {{\n"
        "    projection > 0\n"
        "}"
    )
    assert source.count(old) == 1
    path.write_text(
        source.replace(old, old.replace("projection > 0", "true"), 1),
        encoding="utf-8",
    )
    for entry in verus_evidence["sources"]:
        if entry["path"] == claim.verified_kernel_source:
            entry["sha256"] = module._sha256_file(path)
            break
    else:
        raise AssertionError(claim.verified_kernel_source)

    errors = module._cross_tool_evidence_errors(
        ledger,
        cross_tool_evidence,
        tlaps_evidence=tlaps_evidence,
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest,
    )
    assert any("constant result" in error for error in errors)


@pytest.mark.parametrize(
    ("macro_name", "reviewed_fragment", "mutation"),
    (
        (
            "pending_round_can_begin_body",
            "$pending.height == $owner.height",
            "$pending.height >= $owner.height",
        ),
        (
            "pending_round_can_acknowledge_body",
            "$owner_after.view == $pending.view + 1u64",
            "$owner_after.view >= $pending.view + 1u64",
        ),
        (
            "persist_slot_matches_boundary_body",
            "$slot.requested.view == $pending.view",
            "$slot.requested.view == $boundary.tag.view",
        ),
        (
            "production_durable_intent_trace_body",
            "$projection.event_tag,\n                        $projection.owner_tag_before",
            "$projection.owner_tag_before,\n                        $projection.owner_tag_before",
        ),
    ),
)
def test_progress_witness_shared_kernel_rejects_owner_record_round_mutations(
    tmp_path: Path,
    macro_name: str,
    reviewed_fragment: str,
    mutation: str,
) -> None:
    """Owner/record round relations remain sealed into the Verus predicate."""

    module = load_checker()
    contract = next(
        contract
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
        if contract.obligation_id == "progress-witness-production-refinement"
    )
    claim = next(
        claim
        for claim in contract.claims
        if claim.constant == "ProductionDurableIntentTraceRefinesProgressWitness"
    )
    assert claim.verified_kernel_source is not None
    source = (ROOT_DIR / claim.verified_kernel_source).read_text(encoding="utf-8")
    assert reviewed_fragment in source
    mutated = source.replace(reviewed_fragment, mutation, 1)

    with pytest.raises(ValueError, match=rf"shared macro .*{macro_name}"):
        module._shared_macro_payload(
            mutated,
            path=tmp_path / "refinement.rs",
            expected=claim.verified_kernel_shared_macro_sha256,
            description="progress-witness refinement mutation",
        )


@pytest.mark.parametrize(
    ("mutation", "expected_error"),
    (
        ("negated_production_call", "exact kernel enforcement and projection"),
        ("constant_production_field", "exact kernel enforcement and projection"),
        ("disconnected_verus_proof", "must invoke its verified kernel"),
        ("altered_verus_projection", "projection builder"),
        ("constant_verus_mirror", "Verus mirror kernel"),
        ("altered_shared_step", "shared macro"),
        ("constant_shared_claim", "shared macro"),
        ("omitted_signer_bitmap", "shared macro"),
        ("omitted_signer_count", "shared macro"),
        ("omitted_voting_power", "shared macro"),
        ("omitted_evidence_class", "shared macro"),
        ("omitted_bitmap_cardinality", "shared macro"),
        ("nonzero_absent_context", "shared macro"),
        ("nonzero_absent_subject", "shared macro"),
        ("constant_projected_bitmap", "EnterView identity item"),
        ("constant_projected_count", "EnterView identity item"),
        ("constant_projected_power", "EnterView identity item"),
        ("constant_projected_evidence", "EnterView identity item"),
        ("altered_effect_anchor", "EnterView identity item"),
        ("reference_only_evidence", "EnterView identity item"),
        ("truncating_signer_shift", "EnterView identity item"),
        ("disconnected_substitution_test", "substitution regression"),
        ("altered_identity_postcondition", "exact normalized signature"),
    ),
)
def test_effective_lock_cross_tool_contract_rejects_real_source_mutations(
    tmp_path: Path,
    mutation: str,
    expected_error: str,
) -> None:
    """The real entry-24 seam rejects disconnected or weakened projections."""

    module = load_checker()
    claim = module.CROSS_TOOL_REFINEMENT_CONTRACTS[0].claims[0]
    assert claim.verified_kernel_source is not None
    paths = set(claim.production_sources)
    paths.add(claim.verus_source)
    paths.add(claim.verified_kernel_source)
    paths.update(call_site.source for call_site in claim.production_call_sites)
    for relative in paths:
        source = ROOT_DIR / relative
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)

    if mutation in {"negated_production_call", "constant_production_field"}:
        relative = claim.production_call_sites[0].source
    elif mutation in {
        "disconnected_verus_proof",
        "altered_verus_projection",
        "constant_verus_mirror",
        "altered_identity_postcondition",
    }:
        relative = claim.verus_source
    elif mutation in {
        "constant_projected_bitmap",
        "constant_projected_count",
        "constant_projected_power",
        "constant_projected_evidence",
        "altered_effect_anchor",
        "reference_only_evidence",
        "truncating_signer_shift",
        "disconnected_substitution_test",
    }:
        relative = module._ENTER_VIEW_IDENTITY_PRODUCTION_SOURCE
    else:
        relative = claim.verified_kernel_source
    path = tmp_path / relative
    source = path.read_text(encoding="utf-8")
    replacements = {
        "negated_production_call": (
            "if !production_enter_view_uses_post_install_effective_lock_kernel("
            "trace, enter_view)",
            "if production_enter_view_uses_post_install_effective_lock_kernel("
            "trace, enter_view)",
        ),
        "constant_production_field": (
            "owner_after: ownership_after,",
            "owner_after: 0,",
        ),
        "disconnected_verus_proof": (
            "assert(production_enter_view_uses_post_install_effective_lock_kernel(\n"
            "        production_enter_view_effective_lock_trace(projection),\n"
            "        projection.enter_view,\n"
            "    ));",
            "assert(production_kernel_relation(projection));",
        ),
        "altered_verus_projection": (
            "protected_before: protected_after,",
            "protected_before: 0u64,",
        ),
        "constant_verus_mirror": (
            "effective_lock_trace_claim_body!(trace, 1u8)\n"
            "        && enter_view_locked_prepare_qc_identity_body!(enter_view)",
            "true",
        ),
        "altered_shared_step": (
            "$projection.owner_after == $projection.owner_before",
            "$projection.owner_after == $projection.protected_after",
        ),
        "constant_shared_claim": (
            "$projection.kind == $kind && effective_lock_trace_step_is_valid($projection)",
            "true",
        ),
        "omitted_signer_bitmap": (
            "&& $left.signer_bitmap == $right.signer_bitmap",
            "&& true",
        ),
        "omitted_signer_count": (
            "&& $left.signer_bitmap_count == $right.signer_bitmap_count\n"
            "                    && $left.signer_count == $right.signer_count",
            "&& $left.signer_bitmap_count == $right.signer_bitmap_count\n"
            "                    && true",
        ),
        "omitted_voting_power": (
            "&& $left.voting_power == $right.voting_power",
            "&& true",
        ),
        "omitted_evidence_class": (
            "&& $left.evidence_class == $right.evidence_class",
            "&& true",
        ),
        "omitted_bitmap_cardinality": (
            "&& $certificate.signer_bitmap_count == $certificate.signer_count",
            "&& true",
        ),
        "nonzero_absent_context": (
            "canonical_identity_is_zero_body!($certificate.context_id)",
            "true",
        ),
        "nonzero_absent_subject": (
            "canonical_identity_is_zero_body!($certificate.subject)",
            "true",
        ),
        "constant_projected_bitmap": (
            "            signer_bitmap,\n"
            "            signer_bitmap_count,\n"
            "            signer_count,\n"
            "            voting_power,\n"
            "            evidence_class:",
            "            signer_bitmap: 1,\n"
            "            signer_bitmap_count,\n"
            "            signer_count,\n"
            "            voting_power,\n"
            "            evidence_class:",
        ),
        "constant_projected_count": (
            "            signer_bitmap_count,\n"
            "            signer_count,\n"
            "            voting_power,\n"
            "            evidence_class:",
            "            signer_bitmap_count,\n"
            "            signer_count: 1,\n"
            "            voting_power,\n"
            "            evidence_class:",
        ),
        "constant_projected_power": (
            "            signer_count,\n"
            "            voting_power,\n"
            "            evidence_class:",
            "            signer_count,\n"
            "            voting_power: 1,\n"
            "            evidence_class:",
        ),
        "constant_projected_evidence": (
            "evidence_class: if signer_projection.is_some() {",
            "evidence_class: if true {",
        ),
        "altered_effect_anchor": (
            "                effect_protected_lock,\n"
            "                local_lock,\n"
            "                incoming_lock,",
            "                effect_protected_lock,\n"
            "                local_lock,\n"
            "                local_lock,",
        ),
        "reference_only_evidence": (
            "if local.is_some_and(|candidate| candidate == certificate) {",
            "if local.is_some_and(|candidate| "
            "candidate.reference() == certificate.reference()) {",
        ),
        "truncating_signer_shift": (
            "let bit = 1u128.checked_shl(shift)?;",
            "let bit = 1u128 << (shift % u128::BITS);",
        ),
        "disconnected_substitution_test": (
            "assert_eq!(\n"
            "            projection.enter_view.effect_protected_lock.evidence_class,\n"
            "            CERTIFICATE_EVIDENCE_FOREIGN\n"
            "        );",
            "assert_ne!(\n"
            "            projection.enter_view.effect_protected_lock.evidence_class,\n"
            "            CERTIFICATE_EVIDENCE_FOREIGN\n"
            "        );",
        ),
        "altered_identity_postcondition": (
            "projection.enter_view.effect_protected_lock.present\n"
            "            == projection.enter_view.durable_lock_after.present,",
            "true,",
        ),
    }
    old, new = replacements[mutation]
    assert source.count(old) == 1
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    evidence_paths = {
        claim.verus_source,
        claim.verified_kernel_source,
        *(call_site.source for call_site in claim.production_call_sites),
    }
    verus_evidence = {
        "sources": [
            {
                "path": relative,
                "sha256": module._sha256_file(tmp_path / relative),
            }
            for relative in sorted(evidence_paths)
        ]
    }
    with pytest.raises(ValueError, match=expected_error):
        module._cross_tool_claim_payload(
            claim,
            verus_evidence=verus_evidence,
            root_dir=tmp_path,
        )


@pytest.mark.parametrize(
    ("mutation", "expected_error"),
    (
        ("missing_live_dequeue", "source seal"),
        ("cfg_test_only_kernel_owner", "may use only the non-gating"),
    ),
)
def test_body_service_cross_tool_claim_requires_live_production_dequeue(
    tmp_path: Path,
    mutation: str,
    expected_error: str,
) -> None:
    """The service theorem stays bound to the live dequeue and dispatch owners."""

    module = load_checker()
    claim = next(
        claim
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
        for claim in contract.claims
        if claim.constant == "ProductionBodyServiceRefinesAsyncFairness"
    )
    claim = freeze_cross_tool_claim_call_sites(module, claim)
    assert claim.verified_kernel_source is not None
    paths = {
        *claim.production_sources,
        claim.verus_source,
        claim.verified_kernel_source,
        *(call_site.source for call_site in claim.production_call_sites),
        *(seal.source for seal in claim.source_item_seals),
    }
    for relative in paths:
        source = ROOT_DIR / relative
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)

    relative = "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    path = tmp_path / relative
    source = path.read_text(encoding="utf-8")
    if mutation == "missing_live_dequeue":
        old = (
            "let (command, candidate) = match "
            "self.ingress.pop_next_with_ownership() {"
        )
        new = (
            "let (command, candidate) = match "
            "self.ingress.ownership_projection() {"
        )
    else:
        old = "    fn pop_next_with_ownership(\n"
        new = "    #[cfg(test)]\n    fn pop_next_with_ownership(\n"
    assert source.count(old) == 1
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    verus_evidence = {
        "sources": [
            {
                "path": source_relative,
                "sha256": module._sha256_file(tmp_path / source_relative),
            }
            for source_relative in sorted(paths)
        ]
    }
    with pytest.raises(ValueError, match=expected_error):
        module._cross_tool_claim_payload(
            claim,
            verus_evidence=verus_evidence,
            root_dir=tmp_path,
        )


@pytest.mark.parametrize(
    (
        "claim_constant",
        "relative",
        "old",
        "new",
        "expected_error",
    ),
    (
        (
            "ProductionDecisionTraceRefinesRecoveryWitness",
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "state_height: self.expected().state_height(),",
            "state_height: self.frozen_height(),",
            "source seal",
        ),
        (
            "ProductionDecisionTraceRefinesRecoveryWitness",
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "$projection.expected_height == $projection.frozen_height",
            "$projection.expected_height == $projection.expected_height",
            "shared macro|source seal",
        ),
        (
            "ProductionDecisionTraceRefinesRecoveryWitness",
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "pub(crate) const IDENTITY_KIND_WIRE_HEIGHT_CONTEXT: u8 = 2;",
            "pub(crate) const IDENTITY_KIND_WIRE_HEIGHT_CONTEXT: u8 = 1;",
            "source seal",
        ),
        (
            "ProductionDecisionTraceRefinesRecoveryWitness",
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "if !production_decision_trace_refines_recovery_witness_kernel(recovery_trace)",
            "if production_decision_trace_refines_recovery_witness_kernel(recovery_trace)",
            "exact kernel enforcement and projection",
        ),
        (
            "ProductionIngressIdentityAndClassTraceRefinesProtectedOwnership",
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            '.expect("successful runtime ingress retains the admitted command");\n'
            "        let ingress_trace = ProductionIngressIdentityAndClassTraceProjection {\n"
            "            incoming_height: incoming_tag.height(),\n"
            "            incoming_view: incoming_tag.view(),\n"
            "            incoming_generation: incoming_tag.generation().get(),\n"
            "            incoming_class,\n"
            "            stored_height: stored.tag.height(),",
            '.expect("successful runtime ingress retains the admitted command");\n'
            "        let ingress_trace = ProductionIngressIdentityAndClassTraceProjection {\n"
            "            incoming_height: incoming_tag.height(),\n"
            "            incoming_view: incoming_tag.view(),\n"
            "            incoming_generation: incoming_tag.generation().get(),\n"
            "            incoming_class,\n"
            "            stored_height: incoming_tag.height(),",
            "exact kernel enforcement and projection",
        ),
        (
            "ProductionIngressIdentityAndClassTraceRefinesProtectedOwnership",
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            '.expect("successful runtime batch ingress retains the admitted command");\n'
            "            let ingress_trace = ProductionIngressIdentityAndClassTraceProjection {\n"
            "                incoming_height: incoming_tag.height(),\n"
            "                incoming_view: incoming_tag.view(),\n"
            "                incoming_generation: incoming_tag.generation().get(),\n"
            "                incoming_class,\n"
            "                stored_height: stored.tag.height(),",
            '.expect("successful runtime batch ingress retains the admitted command");\n'
            "            let ingress_trace = ProductionIngressIdentityAndClassTraceProjection {\n"
            "                incoming_height: incoming_tag.height(),\n"
            "                incoming_view: incoming_tag.view(),\n"
            "                incoming_generation: incoming_tag.generation().get(),\n"
            "                incoming_class,\n"
            "                stored_height: incoming_tag.height(),",
            "exact kernel enforcement and projection",
        ),
        (
            "ProductionIngressIdentityAndClassTraceRefinesProtectedOwnership",
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            '.expect("canonical body commit retains the admitted completion");\n'
            "        let ingress_trace = ProductionIngressIdentityAndClassTraceProjection {\n"
            "            incoming_height: incoming_tag.height(),\n"
            "            incoming_view: incoming_tag.view(),\n"
            "            incoming_generation: incoming_tag.generation().get(),\n"
            "            incoming_class,\n"
            "            stored_height: stored.tag.height(),",
            '.expect("canonical body commit retains the admitted completion");\n'
            "        let ingress_trace = ProductionIngressIdentityAndClassTraceProjection {\n"
            "            incoming_height: incoming_tag.height(),\n"
            "            incoming_view: incoming_tag.view(),\n"
            "            incoming_generation: incoming_tag.generation().get(),\n"
            "            incoming_class,\n"
            "            stored_height: incoming_tag.height(),",
            "exact kernel enforcement and projection",
        ),
        (
            "ProductionReliableFlushTraceRefinesOutboundOwnership",
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "ticket_rank: reliable_flush_usize(evidence.ticket_rank)?,",
            "ticket_rank: 1,",
            "source seal",
        ),
        (
            "ProductionReliableFlushTraceRefinesOutboundOwnership",
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "$projection.chunk_cursor_before == $projection.chunk_index",
            "$projection.chunk_cursor_before == $projection.chunk_cursor_before",
            "shared macro|source seal",
        ),
        (
            "ProductionReliableFlushTraceRefinesOutboundOwnership",
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "pub(crate) const IDENTITY_KIND_SIDECAR_CHUNK: u8 = 12;",
            "pub(crate) const IDENTITY_KIND_SIDECAR_CHUNK: u8 = 11;",
            "source seal",
        ),
        (
            "ProductionReliableFlushTraceRefinesOutboundOwnership",
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "if !production_reliable_flush_trace_refines_outbound_ownership_kernel(flush_trace)",
            "if production_reliable_flush_trace_refines_outbound_ownership_kernel(flush_trace)",
            "exact kernel enforcement and projection",
        ),
        (
            "ProductionApplicationTraceRefinesDecisionCompletion",
            "crates/iroha_core/src/sumeragi/v2_apply.rs",
            "state_height_after: u64::try_from(self.state_height_after()).ok()?,",
            "state_height_after: self.context().height,",
            "source seal",
        ),
        (
            "ProductionApplicationTraceRefinesDecisionCompletion",
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "$projection.context_height == $projection.commit_qc.decision.height",
            "$projection.context_height == $projection.context_height",
            "shared macro|source seal",
        ),
        (
            "ProductionApplicationTraceRefinesDecisionCompletion",
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "pub(crate) const IDENTITY_KIND_FINALITY_ARTIFACT: u8 = 2;",
            "pub(crate) const IDENTITY_KIND_FINALITY_ARTIFACT: u8 = 1;",
            "source seal",
        ),
        (
            "ProductionApplicationTraceRefinesDecisionCompletion",
            "crates/iroha_core/src/sumeragi/v2_apply.rs",
            "if !production_application_trace_refines_decision_completion_kernel(application_trace)",
            "if production_application_trace_refines_decision_completion_kernel(application_trace)",
            "exact kernel enforcement and projection",
        ),
    ),
)
def test_exact_identity_cross_tool_claims_reject_real_source_mutations(
    tmp_path: Path,
    claim_constant: str,
    relative: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Recovery, reliable flush, and application stay source-bound end to end."""

    module = load_checker()
    claim = next(
        claim
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
        for claim in contract.claims
        if claim.constant == claim_constant
    )
    claim = freeze_cross_tool_claim_call_sites(module, claim)
    assert claim.verified_kernel_source is not None
    paths = {
        *claim.production_sources,
        claim.verus_source,
        claim.verified_kernel_source,
        *(call_site.source for call_site in claim.production_call_sites),
    }
    for source_relative in paths:
        source = ROOT_DIR / source_relative
        destination = tmp_path / source_relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)

    path = tmp_path / relative
    source = path.read_text(encoding="utf-8")
    assert source.count(old) == 1
    path.write_text(source.replace(old, new, 1), encoding="utf-8")
    verus_evidence = {
        "sources": [
            {
                "path": source_relative,
                "sha256": module._sha256_file(tmp_path / source_relative),
            }
            for source_relative in sorted(paths)
        ]
    }
    with pytest.raises(ValueError, match=expected_error):
        module._cross_tool_claim_payload(
            claim,
            verus_evidence=verus_evidence,
            root_dir=tmp_path,
        )


def test_reliable_flush_transitive_seals_reject_weakened_identity_helpers(
    tmp_path: Path,
) -> None:
    """Exact writer completion and byte-free chunk identity cannot disconnect."""

    module = load_checker()
    claim = next(
        claim
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
        for claim in contract.claims
        if claim.constant
        == "ProductionReliableFlushTraceRefinesOutboundOwnership"
    )
    required_seals = {
        (
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "ProductionReliableFlushApplicationProjection",
            (),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "production_reliable_flush_application_body",
            (),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "production_reliable_flush_two_phase_link_body",
            (),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "IDENTITY_KIND_REPLY_SOURCE_KEY",
            (),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "IDENTITY_KIND_REPLY_DELIVERY_ROUTE",
            (),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "IDENTITY_KIND_REPLY_WRITER_OCCURRENCE",
            (),
        ),
        (
            "crates/iroha_sumeragi_core/src/verus_proofs.rs",
            "ProductionReliableFlushApplicationProjection",
            (("verus", "!"),),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "ServerPendingChunkIdentity",
            (),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "from_admitted_reply",
            (('impl', 'CertifiedMergeSidecarChunkAdmission'),),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "projection",
            (("impl", "CertifiedMergeSidecarChunkAdmission"),),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "matches_ack_identity",
            (("impl", "CertifiedMergeSidecarChunkAdmission"),),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "projection_matches_identity",
            (("impl", "CertifiedMergeSidecarChunkAdmission"),),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "is_bound_to_source",
            (("impl", "CertifiedMergeSidecarChunkAdmission"),),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "matches_materialized_chunk",
            (("impl", "CertifiedMergeSidecarChunkAdmission"),),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "from_message",
            (("impl", "ServerPendingChunkIdentity"),),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "matches_admission",
            (("impl", "ServerPendingChunkIdentity"),),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "preflight_reliable_flush_gate",
            (),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "ReliableFlushSiblingStateSnapshot",
            (),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "capture",
            (("impl", "ReliableFlushSiblingStateSnapshot"),),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "digest",
            (("impl", "ReliableFlushSiblingStateSnapshot"),),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "reliable_flush_application_occurrence_projection",
            (),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "project_reliable_flush_residuals",
            (),
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "bind_confirmed_worker_trace",
            (("impl", "CertifiedMergeSidecarChunkAdmission"),),
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "reliable_flush_trace_projection",
            (),
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "same_ticket",
            (("impl", "NetworkActorAdmittedTicketIdentity"),),
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "process_local_identity_hash",
            (("impl", "NetworkActorAdmittedTicketIdentity"),),
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "process_local_identity_hash",
            (("impl", "NetworkReplyRoute"),),
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "matches",
            (("impl", "WeakProgressDeliveryAuthority"),),
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "try_reserve_for_source",
            (("impl", "NetworkActorProgressBudget"),),
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "claim_writer_flush_once",
            (("impl", "NetworkReplyFlushIdentity"),),
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "same_ticket_identity",
            (("impl", "NetworkReplyFlushIdentity"),),
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "same_delivery_occurrence",
            (("impl", "NetworkReplyFlushIdentity"),),
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "same_writer_flush_occurrence",
            (("impl", "NetworkReplyFlushIdentity"),),
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "process_local_route_identity_hash",
            (("impl", "NetworkReplyFlushIdentity"),),
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "process_local_writer_occurrence_identity_hash",
            (("impl", "NetworkReplyFlushIdentity"),),
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "process_local_identity_hash",
            (("impl", "NetworkReplySourceKey"),),
        ),
    }
    seals = {
        (seal.source, seal.item, seal.brace_context): seal
        for seal in claim.source_item_seals
    }
    assert required_seals <= seals.keys()
    module._cross_tool_source_item_seal_payload(claim, root_dir=ROOT_DIR)

    mutations = (
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "from_admitted_reply",
            (("impl", "CertifiedMergeSidecarChunkAdmission"),),
            "source_key_identity: source_key.process_local_identity_hash(),",
            "source_key_identity: flush_identity.process_local_route_identity_hash(),",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "projection",
            (("impl", "CertifiedMergeSidecarChunkAdmission"),),
            "&self.projection",
            'unreachable!("projection disconnected")',
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "matches_ack_identity",
            (("impl", "CertifiedMergeSidecarChunkAdmission"),),
            ".same_writer_flush_occurrence(ack_identity)",
            ".same_delivery_occurrence(ack_identity)",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "projection_matches_identity",
            (("impl", "CertifiedMergeSidecarChunkAdmission"),),
            "let projection = &self.projection;",
            "return true;\n        let projection = &self.projection;",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "projection_matches_identity",
            (("impl", "CertifiedMergeSidecarChunkAdmission"),),
            "            && projection.writer_occurrence_identity\n"
            "                == identity.process_local_writer_occurrence_identity_hash()",
            "            && true",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "is_bound_to_source",
            (("impl", "CertifiedMergeSidecarChunkAdmission"),),
            "self.projection.semantic_target == *route.semantic_target()\n"
            "            && self.source_key == route.source_key()",
            "true",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "matches_materialized_chunk",
            (("impl", "CertifiedMergeSidecarChunkAdmission"),),
            "let CertifiedMergeSidecarMessage::Chunk(chunk) = message.as_ref() else {",
            "return true;\n        let CertifiedMergeSidecarMessage::Chunk(chunk) = "
            "message.as_ref() else {",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "from_message",
            (("impl", "ServerPendingChunkIdentity"),),
            "payload_digest: Hash::new_from_chunks(&[\n"
            "                CHUNK_PAYLOAD_DIGEST_DOMAIN,\n"
            "                chunk.bytes.as_slice(),\n"
            "            ]),",
            "payload_digest: Hash::new(chunk.bytes.as_slice()),",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "matches_admission",
            (("impl", "ServerPendingChunkIdentity"),),
            "let projection = admission.projection();",
            "return true;\n        let projection = admission.projection();",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "preflight_reliable_flush_gate",
            (),
            "if !pending_marker.matches_admission(admission) {",
            "if false {",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "reliable_flush_application_occurrence_projection",
            (),
            "        evidence.writer_occurrence_identity,\n",
            "        evidence.delivery_route_identity,\n",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "reliable_flush_application_occurrence_projection",
            (),
            "application.chunk_cursor_after = "
            "reliable_flush_usize(evidence.chunk_cursor_after)?;",
            "application.chunk_cursor_after = application.chunk_cursor_before;",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "project_reliable_flush_residuals",
            (),
            "application.sibling_records_equal =\n"
            "        plan.sibling_state_before == observation.sibling_state_after;",
            "application.sibling_records_equal = true;",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "bind_confirmed_worker_trace",
            (("impl", "CertifiedMergeSidecarChunkAdmission"),),
            "            || !production_reliable_flush_two_phase_link_kernel(trace, occurrence)\n",
            "",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "reliable_flush_trace_projection",
            (),
            "writer_occurrence_identity: reliable_flush_hash_identity(\n"
            "            IDENTITY_DOMAIN_PROCESS_LOCAL,\n"
            "            IDENTITY_KIND_REPLY_WRITER_OCCURRENCE,\n"
            "            evidence.writer_occurrence_identity,\n"
            "        ),",
            "writer_occurrence_identity: reliable_flush_hash_identity(\n"
            "            IDENTITY_DOMAIN_PROCESS_LOCAL,\n"
            "            IDENTITY_KIND_REPLY_WRITER_OCCURRENCE,\n"
            "            evidence.delivery_route_identity,\n"
            "        ),",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "same_ticket",
            (("impl", "NetworkActorAdmittedTicketIdentity"),),
            "Arc::ptr_eq(&self.budget, &other.budget)\n"
            "            && self.id == other.id",
            "self.id == other.id",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "process_local_identity_hash",
            (("impl", "NetworkActorAdmittedTicketIdentity"),),
            "projection.extend_from_slice(&(Arc::as_ptr(&self.budget) as usize as u128).to_le_bytes());",
            "projection.extend_from_slice(&0u128.to_le_bytes());",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "process_local_identity_hash",
            (("impl", "NetworkReplyRoute"),),
            "let tenure = (Arc::as_ptr(&self.tenure) as usize as u128).to_le_bytes();",
            "let tenure = actor;",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "matches",
            (("impl", "WeakProgressDeliveryAuthority"),),
            "Arc::ptr_eq(&retained, &candidate.tenure)",
            "retained.connection_ordinal == candidate.tenure.connection_ordinal",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "try_reserve_for_source",
            (("impl", "NetworkActorProgressBudget"),),
            "(Some(retained), Some(candidate)) => !retained.matches(candidate),",
            "(Some(_), Some(_)) => false,",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "claim_writer_flush_once",
            (("impl", "NetworkReplyFlushIdentity"),),
            "self.completion_claimed\n"
            "            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)\n"
            "            .is_ok()",
            "true",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "same_ticket_identity",
            (("impl", "NetworkReplyFlushIdentity"),),
            "&& self.route.same_source(&other.route)",
            "&& self.route.same_source(&other.route)\n"
            "            && Arc::ptr_eq(&self.completion_claimed, "
            "&other.completion_claimed)",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "same_delivery_occurrence",
            (("impl", "NetworkReplyFlushIdentity"),),
            "self.same_ticket_identity(other) && self.route.same_delivery(&other.route)",
            "self.route.same_delivery(&other.route)",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "same_writer_flush_occurrence",
            (("impl", "NetworkReplyFlushIdentity"),),
            "self.same_delivery_occurrence(other)\n"
            "            && Arc::ptr_eq(&self.completion_claimed, "
            "&other.completion_claimed)",
            "self.same_delivery_occurrence(other)",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "process_local_route_identity_hash",
            (("impl", "NetworkReplyFlushIdentity"),),
            "self.route.process_local_identity_hash()",
            "self.route.source_key().process_local_identity_hash()",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "process_local_writer_occurrence_identity_hash",
            (("impl", "NetworkReplyFlushIdentity"),),
            "Hash::new_from_chunks(&[DOMAIN, ticket.as_ref(), route.as_ref(), &completion_claim])",
            "Hash::new_from_chunks(&[DOMAIN, ticket.as_ref(), route.as_ref()])",
        ),
        (
            "crates/iroha_p2p/src/network.rs",
            "process_local_identity_hash",
            (("impl", "NetworkReplySourceKey"),),
            "Hash::new_from_chunks(&[DOMAIN, &actor, &authenticated_source])",
            "Hash::new(&authenticated_source)",
        ),
    )
    for index, (relative, item, context, old, new) in enumerate(mutations):
        mutation_root = tmp_path / f"mutation-{index}"
        source = ROOT_DIR / relative
        destination = mutation_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
        mutate_rust_item_source_in_context(
            module,
            destination,
            item,
            context,
            old,
            new,
        )
        diagnostic_claim = replace(
            claim,
            source_item_seals=(seals[(relative, item, context)],),
        )
        with pytest.raises(ValueError, match="cross-tool source seal"):
            module._cross_tool_source_item_seal_payload(
                diagnostic_claim,
                root_dir=mutation_root,
            )

    payload_claim = freeze_cross_tool_claim_call_sites(module, claim)
    payload_paths = {
        *payload_claim.production_sources,
        payload_claim.verus_source,
        payload_claim.verified_kernel_source,
        *(site.source for site in payload_claim.production_call_sites),
        *(
            kernel.verified_kernel_source
            for kernel in payload_claim.supplemental_kernels
        ),
        *(
            site.source
            for kernel in payload_claim.supplemental_kernels
            for site in kernel.production_call_sites
        ),
        *(seal.source for seal in payload_claim.source_item_seals),
    }

    def copied_evidence(root: Path):
        return {
            "sources": [
                {
                    "path": relative,
                    "sha256": module._sha256_file(root / relative),
                }
                for relative in sorted(payload_paths)
            ]
        }

    baseline_root = tmp_path / "linked-baseline"
    for relative in payload_paths:
        destination = baseline_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)
    baseline = module._cross_tool_claim_payload(
        payload_claim,
        verus_evidence=copied_evidence(baseline_root),
        root_dir=baseline_root,
    )
    assert len(baseline["supplemental_verified_kernels"]) == 2

    linked_mutations = (
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if !production_reliable_flush_application_refines_source_lane_kernel(application) {",
            "if false {",
            "(exact reviewed item token seal|exact kernel enforcement and projection)",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if !production_reliable_flush_two_phase_link_kernel(worker_trace, application) {",
            "if !production_reliable_flush_two_phase_link_kernel(worker_trace, occurrence) {",
            "(exact reviewed item token seal|exact kernel enforcement and projection)",
        ),
        (
            "crates/iroha_sumeragi_core/src/verus_proofs.rs",
            "pub closed spec fn production_reliable_flush_application_refines_source_lane_kernel(\n"
            "    projection: ProductionReliableFlushApplicationProjection,\n"
            ") -> bool {\n"
            "    production_reliable_flush_application_body!(projection)\n"
            "}",
            "pub closed spec fn production_reliable_flush_application_refines_source_lane_kernel(\n"
            "    projection: ProductionReliableFlushApplicationProjection,\n"
            ") -> bool {\n"
            "    true\n"
            "}",
            "same exact reviewed body as production",
        ),
        (
            "crates/iroha_sumeragi_core/src/verus_proofs.rs",
            "assert(production_reliable_flush_two_phase_link_kernel(\n"
            "        production_reliable_flush_trace_projection(worker),\n"
            "        production_reliable_flush_application_projection(application),\n"
            "    ));",
            "assert(worker.status == 2u8);",
            "supplemental verified kernel .* exact projection once",
        ),
        (
            "crates/iroha_sumeragi_core/src/verus_proofs.rs",
            "pub closed spec fn production_reliable_flush_application_projection(\n"
            "    projection: ProductionReliableFlushApplicationProjection,\n"
            ") -> ProductionReliableFlushApplicationProjection {\n"
            "    projection\n"
            "}",
            "pub closed spec fn production_reliable_flush_application_projection(\n"
            "    projection: ProductionReliableFlushApplicationProjection,\n"
            ") -> ProductionReliableFlushApplicationProjection {\n"
            "    ProductionReliableFlushApplicationProjection::default()\n"
            "}",
            "projection builder .* exact reviewed token seal",
        ),
    )
    for index, (relative, old, new, expected_error) in enumerate(
        linked_mutations
    ):
        mutation_root = tmp_path / f"linked-mutation-{index}"
        for source_relative in payload_paths:
            destination = mutation_root / source_relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT_DIR / source_relative, destination)
        path = mutation_root / relative
        source = path.read_text(encoding="utf-8")
        assert source.count(old) == 1, (relative, old)
        path.write_text(source.replace(old, new, 1), encoding="utf-8")
        with pytest.raises(ValueError, match=expected_error):
            module._cross_tool_claim_payload(
                payload_claim,
                verus_evidence=copied_evidence(mutation_root),
                root_dir=mutation_root,
            )

    macro_mutations = (
        (
            "production_reliable_flush_trace_body",
            "refinement_tag_value!(IDENTITY_KIND_REPLY_SOURCE_KEY)",
            "refinement_tag_value!(IDENTITY_KIND_REPLY_DELIVERY_ROUTE)",
        ),
        (
            "production_reliable_flush_application_body",
            "refinement_tag_value!(IDENTITY_KIND_REPLY_DELIVERY_ROUTE)",
            "refinement_tag_value!(IDENTITY_KIND_REPLY_SOURCE_KEY)",
        ),
        (
            "production_reliable_flush_application_body",
            "refinement_tag_value!(IDENTITY_KIND_REPLY_WRITER_OCCURRENCE)",
            "refinement_tag_value!(IDENTITY_KIND_REPLY_DELIVERY_ROUTE)",
        ),
        (
            "production_reliable_flush_application_body",
            "&& $projection.claim_acquired",
            "&& true",
        ),
        (
            "production_reliable_flush_two_phase_link_body",
            "&& $worker.delivery_ordinal_low == $application.delivery_ordinal_low",
            "&& true",
        ),
        (
            "production_reliable_flush_two_phase_link_body",
            "&& canonical_identity_equal_body!(\n"
            "                $worker.source_key_identity,\n"
            "                $application.source_key_identity\n"
            "            )",
            "&& true",
        ),
        (
            "production_reliable_flush_two_phase_link_body",
            "&& canonical_identity_equal_body!(\n"
            "                $worker.delivery_route_identity,\n"
            "                $application.delivery_route_identity\n"
            "            )",
            "&& true",
        ),
        (
            "production_reliable_flush_two_phase_link_body",
            "&& canonical_identity_equal_body!(\n"
            "                $worker.writer_occurrence_identity,\n"
            "                $application.writer_occurrence_identity\n"
            "            )",
            "&& true",
        ),
    )
    relative = "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    for index, (item, old, new) in enumerate(macro_mutations):
        mutation_root = tmp_path / f"macro-mutation-{index}"
        source = ROOT_DIR / relative
        destination = mutation_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
        text = destination.read_text(encoding="utf-8")
        items = module.rust_macro_items(text, item)
        assert len(items) == 1, item
        macro = items[0]
        assert macro.source.count(old) == 1, (item, old)
        mutated = macro.source.replace(old, new, 1)
        destination.write_text(
            text.replace(macro.source, mutated, 1),
            encoding="utf-8",
        )
        diagnostic_claim = replace(
            claim,
            source_item_seals=(seals[(relative, item, ())],),
        )
        with pytest.raises(ValueError, match="cross-tool source seal"):
            module._cross_tool_source_item_seal_payload(
                diagnostic_claim,
                root_dir=mutation_root,
            )


def test_terminal_application_without_successor_activation_claim_rejects_runner_mutations(
    tmp_path: Path,
) -> None:
    """The authenticated Apply boundary cannot be merged with successor startup."""

    module = load_checker()
    claim = next(
        claim
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
        for claim in contract.claims
        if claim.constant
        == "ProductionTerminalApplicationWithoutSuccessorActivationTraceRefinesIndexedTerminal"
    )
    claim = freeze_cross_tool_claim_call_sites(module, claim)
    assert claim.verified_kernel_source is not None
    paths = {
        *claim.production_sources,
        claim.verus_source,
        claim.verified_kernel_source,
        *(call_site.source for call_site in claim.production_call_sites),
        *(seal.source for seal in claim.source_item_seals),
    }
    mutations = (
        (
            "pending_successor_activation_present: "
            "pending_successor_activation.is_some(),",
            "pending_successor_activation_present: true,",
        ),
        (
            "artifact_context_id: "
            "successor_context_refinement_projection(artifact.context_id()),",
            "artifact_context_id: "
            "successor_context_refinement_projection(receipt.context_id()),",
        ),
        (
            "if !production_terminal_application_without_successor_activation_kernel(\n"
            "            terminal_application,\n"
            "        ) {",
            "if production_terminal_application_without_successor_activation_kernel(\n"
            "            terminal_application,\n"
            "        ) {",
        ),
        (
            "if !production_terminal_application_without_successor_activation_kernel(\n"
            "            terminal_application,\n"
            "        ) {\n"
            "            return Err(V2RunnerError::SuccessorRefinementRejected);\n"
            "        }\n"
            "        let activation = PendingSuccessorConstruction::begin(predecessor)?;",
            "let activation = PendingSuccessorConstruction::begin(predecessor)?;\n"
            "        if !production_terminal_application_without_successor_activation_kernel(\n"
            "            terminal_application,\n"
            "        ) {\n"
            "            return Err(V2RunnerError::SuccessorRefinementRejected);\n"
            "        }",
        ),
    )

    for index, (old, new) in enumerate(mutations):
        mutation_root = tmp_path / f"mutation-{index}"
        for source_relative in paths:
            source = ROOT_DIR / source_relative
            destination = mutation_root / source_relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)

        path = mutation_root / "crates/iroha_core/src/sumeragi/v2_runner.rs"
        source = path.read_text(encoding="utf-8")
        assert source.count(old) == 1
        path.write_text(source.replace(old, new, 1), encoding="utf-8")
        verus_evidence = {
            "sources": [
                {
                    "path": source_relative,
                    "sha256": module._sha256_file(
                        mutation_root / source_relative
                    ),
                }
                for source_relative in sorted(paths)
            ]
        }
        with pytest.raises(
            ValueError,
            match="exact kernel enforcement and projection",
        ):
            module._cross_tool_claim_payload(
                claim,
                verus_evidence=verus_evidence,
                root_dir=mutation_root,
            )


def test_two_stage_relay_retry_claim_rejects_source_fairness_mutations(
    tmp_path: Path,
) -> None:
    """Semantic substitution, unfair rotation, and rank resets stay sealed."""

    module = load_checker()
    claim = next(
        claim
        for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS
        for claim in contract.claims
        if claim.constant
        == "ProductionTwoStageRelayRetryTraceRefinesSourceFairness"
    )
    assert claim.verified_kernel_source is not None
    paths = {
        *claim.production_sources,
        claim.verus_source,
        claim.verified_kernel_source,
        *(call_site.source for call_site in claim.production_call_sites),
    }
    mutations = (
        (
            "daemon_source_capacity_matches_two_upstream_lanes: retry_geometry\n"
            "            .daemon_source_capacity_matches_two_upstream_lanes(),",
            "daemon_source_capacity_matches_two_upstream_lanes: false,",
        ),
        (
            "class_corridor_covers_authenticated_sources: retry_geometry\n"
            "            .class_corridor_covers_authenticated_sources(),",
            "class_corridor_covers_authenticated_sources: false,",
        ),
        (
            "selection.source == retry_source\n"
            "            && retry_route.is_authenticated_via(&selection.source.via),",
            "retry_route.is_authenticated_via(&retry_source.via),",
        ),
        (
            "self.ready.push_back(key.clone());",
            "self.ready.push_front(key.clone());",
        ),
        (
            "selected_eligible: selection.selected_eligible,",
            "selected_eligible: true,",
        ),
        (
            ".rposition(|candidate| candidate.reply_route.same_delivery(&retry_route))",
            ".position(|candidate| candidate.reply_route.same_delivery(&retry_route))",
        ),
    )

    for index, (old, new) in enumerate(mutations):
        mutation_root = tmp_path / f"mutation-{index}"
        for source_relative in paths:
            source = ROOT_DIR / source_relative
            destination = mutation_root / source_relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)

        path = mutation_root / "crates/irohad/src/main.rs"
        source = path.read_text(encoding="utf-8")
        assert source.count(old) == 1
        path.write_text(source.replace(old, new, 1), encoding="utf-8")
        verus_evidence = {
            "sources": [
                {
                    "path": source_relative,
                    "sha256": module._sha256_file(
                        mutation_root / source_relative
                    ),
                }
                for source_relative in sorted(paths)
            ]
        }
        with pytest.raises(
            ValueError,
            match=(
                "source seal|exact kernel enforcement and projection|"
                "exact reviewed item token seal"
            ),
        ):
            module._cross_tool_claim_payload(
                claim,
                verus_evidence=verus_evidence,
                root_dir=mutation_root,
            )


@pytest.mark.parametrize("mutation", ("missing", "altered"))
def test_cross_tool_evidence_rejects_missing_or_altered_production_projection(
    tmp_path: Path, mutation: str
) -> None:
    module = load_checker()
    claim = module.CROSS_TOOL_REFINEMENT_CONTRACTS[0].claims[0]
    call_site = claim.production_call_sites[0]
    paths = {
        *claim.production_sources,
        claim.verus_source,
        claim.verified_kernel_source,
        *(site.source for site in claim.production_call_sites),
    }
    for relative in paths:
        source_path = ROOT_DIR / relative
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source_path, destination)

    path = tmp_path / call_site.source
    source = path.read_text(encoding="utf-8")
    if mutation == "missing":
        exact = (
            "if !production_enter_view_uses_post_install_effective_lock_kernel("
            "trace, enter_view) {\n"
            "            return false;\n"
            "        }"
        )
        replacement = "return false;"
    else:
        exact = "owner_after: ownership_after,"
        replacement = "owner_after: 0,"
    assert source.count(exact) == 1
    path.write_text(source.replace(exact, replacement, 1), encoding="utf-8")
    evidence_paths = {
        claim.verus_source,
        claim.verified_kernel_source,
        *(site.source for site in claim.production_call_sites),
    }
    verus_evidence = {
        "sources": [
            {
                "path": relative,
                "sha256": module._sha256_file(tmp_path / relative),
            }
            for relative in sorted(evidence_paths)
        ]
    }
    with pytest.raises(
        ValueError,
        match="exact kernel enforcement and projection",
    ):
        module._cross_tool_claim_payload(
            claim,
            verus_evidence=verus_evidence,
            root_dir=tmp_path,
        )


def test_cross_tool_evidence_rejects_production_bypass_outside_required_expression(
    tmp_path: Path,
) -> None:
    """A retained kernel snippet cannot hide a new bypass elsewhere in its item."""

    module = load_checker()
    claim = module.CROSS_TOOL_REFINEMENT_CONTRACTS[0].claims[0]
    call_site = claim.production_call_sites[0]
    paths = {
        *claim.production_sources,
        claim.verus_source,
        claim.verified_kernel_source,
        *(site.source for site in claim.production_call_sites),
    }
    for relative in paths:
        source = ROOT_DIR / relative
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)

    path = tmp_path / call_site.source
    source = path.read_text(encoding="utf-8")
    items = [
        item
        for item in module.rust_items(source, call_site.item)
        if item.brace_context == call_site.brace_context
    ]
    assert len(items) == 1
    item = items[0]
    body_start = item.source.find("{")
    assert body_start >= 0
    mutated_item = (
        item.source[: body_start + 1]
        + "\n        if projection.enter_view.active { return true; }"
        + item.source[body_start + 1 :]
    )
    assert source.count(item.source) == 1
    path.write_text(
        source.replace(item.source, mutated_item, 1),
        encoding="utf-8",
    )
    evidence_paths = {
        claim.verus_source,
        claim.verified_kernel_source,
        *(site.source for site in claim.production_call_sites),
    }
    verus_evidence = {
        "sources": [
            {
                "path": relative,
                "sha256": module._sha256_file(tmp_path / relative),
            }
            for relative in sorted(evidence_paths)
        ]
    }
    with pytest.raises(ValueError, match="exact reviewed item token seal"):
        module._cross_tool_claim_payload(
            claim,
            verus_evidence=verus_evidence,
            root_dir=tmp_path,
        )


def test_cross_tool_evidence_rejects_named_theorem_substitution(tmp_path: Path) -> None:
    module = load_checker()
    (
        ledger,
        formal_dir,
        _,
        verus_evidence,
        cross_tool_evidence,
        workspace_manifest,
    ) = build_cross_tool_fixture(module, tmp_path)
    contract = module.CROSS_TOOL_REFINEMENT_CONTRACTS[0]
    tla_path = formal_dir / f"{contract.module}.tla"
    canonical_tla = tla_path.read_text(encoding="utf-8")
    log_dir = tmp_path / "target" / "formal" / "sumeragi_v2" / "tlaps"

    def fresh_tlaps_evidence():
        manifest = module._formal_source_manifest(formal_dir, tmp_path)["sha256"]
        for name in module.RELEASE_PROOF_MODULES:
            (log_dir / f"{name}.log").write_text(
                "[INFO]: All 1 obligation proved.\n"
                f"{module._tlapm_runner_marker(name, manifest)}\n",
                encoding="utf-8",
            )
        return module.build_release_evidence(
            tlapm_version=module.TLAPM_COMMIT[:7],
            log_dir=log_dir,
            formal_dir=formal_dir,
            root_dir=tmp_path,
        )

    first_claim = contract.claims[0]
    premise_conjunct = f"/\\ {first_claim.constant} = TRUE"
    assert canonical_tla.count(premise_conjunct) == 1
    tla_path.write_text(
        canonical_tla.replace(
            premise_conjunct,
            f"/\\ {first_claim.constant}Substituted = TRUE",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._cross_tool_evidence_errors(
        ledger,
        cross_tool_evidence,
        tlaps_evidence=fresh_tlaps_evidence(),
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest,
    )
    assert any("exact ordered claim mapping" in error for error in errors)

    premise = contract.tla_statement.split(" => ", maxsplit=1)[0]
    nontrivial_ledger_statement = (
        f"{contract.ledger_symbol} ==\n"
        f"  /\\ {premise}\n"
        "  /\\ EffectiveLockAcquisitionModelObligation"
    )
    alias_ledger_statement = (
        f"{contract.ledger_symbol} ==\n  {premise}"
    )
    assert canonical_tla.count(nontrivial_ledger_statement) == 1
    tla_path.write_text(
        canonical_tla.replace(
            nontrivial_ledger_statement, alias_ledger_statement, 1
        ),
        encoding="utf-8",
    )
    errors = module._cross_tool_evidence_errors(
        ledger,
        cross_tool_evidence,
        tlaps_evidence=fresh_tlaps_evidence(),
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest,
    )
    assert any("ledger operator" in error and "must state exactly" in error for error in errors)

    tla_path.write_text(
        canonical_tla.replace(
            f"{contract.ledger_symbol} ==",
            f"THEOREM {contract.ledger_symbol} ==",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._cross_tool_evidence_errors(
        ledger,
        cross_tool_evidence,
        tlaps_evidence=fresh_tlaps_evidence(),
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest,
    )
    assert any(
        "must be declared exactly once as a top-level operator" in error
        for error in errors
    )

    tla_path.write_text(
        canonical_tla.replace(
            "BY EffectiveLockAcquisitionModelObligation",
            "BY TRUE",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._cross_tool_evidence_errors(
        ledger,
        cross_tool_evidence,
        tlaps_evidence=fresh_tlaps_evidence(),
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest,
    )
    assert any("exact model-obligation bridge proof" in error for error in errors)

    tla_path.write_text(
        mutate_tla_theorem(
            canonical_tla,
            contract.tla_theorem,
            contract.ledger_symbol,
            premise,
        ),
        encoding="utf-8",
    )
    errors = module._cross_tool_evidence_errors(
        ledger,
        cross_tool_evidence,
        tlaps_evidence=fresh_tlaps_evidence(),
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest,
    )
    assert any("must state exactly" in error for error in errors)

    tla_path.write_text(
        canonical_tla.replace(
            f"THEOREM {contract.tla_theorem} ==",
            f"THEOREM {contract.tla_theorem}Substituted ==",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._cross_tool_evidence_errors(
        ledger,
        cross_tool_evidence,
        tlaps_evidence=fresh_tlaps_evidence(),
        verus_evidence=verus_evidence,
        formal_dir=formal_dir,
        root_dir=tmp_path,
        expected_verus_source_manifest_sha256=workspace_manifest,
    )
    assert any("missing named TLA theorem" in error for error in errors)


def test_cross_tool_evidence_requires_proved_dependency_closure(tmp_path: Path) -> None:
    module = load_checker()
    (
        ledger,
        formal_dir,
        tlaps_evidence,
        verus_evidence,
        cross_tool_evidence,
        workspace_manifest,
    ) = build_cross_tool_fixture(module, tmp_path)

    progress_closure = module._proof_dependency_closure(
        "progress-witness-production-refinement"
    )
    assert "async-runner-scheduler-preservation" in progress_closure
    for contract in module.CROSS_TOOL_REFINEMENT_CONTRACTS:
        for prerequisite_id in module._proof_dependency_closure(
            contract.obligation_id
        ):
            incomplete = copy.deepcopy(ledger)
            next(
                obligation
                for obligation in incomplete["obligations"]
                if obligation["id"] == prerequisite_id
            )["status"] = "specified_unproved"
            errors = module._cross_tool_evidence_errors(
                incomplete,
                cross_tool_evidence,
                tlaps_evidence=tlaps_evidence,
                verus_evidence=verus_evidence,
                formal_dir=formal_dir,
                root_dir=tmp_path,
                expected_verus_source_manifest_sha256=workspace_manifest,
            )
            assert any(
                "requires proved prerequisite" in error for error in errors
            ), (contract.obligation_id, prerequisite_id, errors)


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
    assert "SumeragiV2CertifiedRequestHashAuthorityProofs" in (
        module.REQUIRED_MODEL_MODULES
    )
    assert "SumeragiV2DurableDecisionRecoveryProofs" in (
        module.REQUIRED_MODEL_MODULES
    )
    assert "SumeragiV2TemporalLemmas" in module.REQUIRED_MODEL_MODULES
    assert "SumeragiV2ServiceRankLemmas" in module.REQUIRED_MODEL_MODULES
    assert "SumeragiV2ReplyRouteOwnershipProofs" in module.REQUIRED_MODEL_MODULES
    assert "SumeragiV2ReplyRoutePipeline" in module.REQUIRED_MODEL_MODULES
    assert "SumeragiV2ReplyRoutePipelineProofs" in module.REQUIRED_MODEL_MODULES
    assert (
        "SumeragiV2AsyncNetworkReplyRouteProofs"
        in module.REQUIRED_MODEL_MODULES
    )
    assert "SumeragiV2AsyncLivenessProofs" in module.REQUIRED_MODEL_MODULES
    assert (
        "SumeragiV2AsyncHistoricalRecoveryLivenessProofs"
        in module.REQUIRED_MODEL_MODULES
    )
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
    assert "SumeragiV2ReplyRouteOwnershipProofs" in module.RELEASE_PROOF_MODULES
    assert "SumeragiV2ReplyRoutePipelineProofs" in module.RELEASE_PROOF_MODULES
    assert (
        "SumeragiV2AsyncNetworkReplyRouteProofs"
        in module.RELEASE_PROOF_MODULES
    )
    assert (
        "SumeragiV2AsyncFairnessRefinementProofs"
        in module.RELEASE_PROOF_MODULES
    )
    assert "SumeragiV2AsyncLivenessProofs" in module.RELEASE_PROOF_MODULES
    assert (
        "SumeragiV2AsyncHistoricalRecoveryLivenessProofs"
        in module.RELEASE_PROOF_MODULES
    )
    assert "SumeragiV2CertifiedRequestHashAuthorityProofs" in (
        module.RELEASE_PROOF_MODULES
    )
    assert "SumeragiV2DurableDecisionRecoveryProofs" in (
        module.RELEASE_PROOF_MODULES
    )
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
        "effective-lock-body-acquisition-model": "tlaps_proved",
        "effective-lock-body-acquisition-production-refinement": (
            "specified_unproved"
        ),
        "post-decision-timeout-exclusion": "tlaps_proved",
        "decision-recovery-across-restart": "tlaps_proved",
        "progress-witness-production-refinement": "specified_unproved",
        "async-type-invariant": "tlaps_proved",
        "async-progress-ownership-invariant": "tlaps_proved",
        "protected-service-rank-stage4-ready-causal": "tlaps_proved",
        "protected-service-rank-serve-fifo": "tlaps_proved",
        "protected-service-rank-stage5-consensus-fifo": "tlaps_proved",
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


def test_effective_lock_body_model_is_proved_and_production_refinement_remains_debt() -> None:
    module = load_checker()
    ledger = module.load_ledger()
    by_id = {obligation["id"]: obligation for obligation in ledger["obligations"]}

    model = by_id["effective-lock-body-acquisition-model"]
    assert model == {
        "id": "effective-lock-body-acquisition-model",
        "requirement": model["requirement"],
        "module": "SumeragiV2EffectiveLockAcquisitionProofs",
        "symbol": "EffectiveLockAcquisitionModelObligation",
        "status": "tlaps_proved",
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
    operator = module._top_level_operator_body(
        source, "EffectiveLockBodyAcquisitionProductionRefinementObligation"
    )
    assert operator is not None
    assert module._top_level_theorem_body(
        source, "EffectiveLockBodyAcquisitionProductionRefinementObligation"
    ) is None
    statement = " ".join(operator[0].split())
    assert statement == (
        "/\\ ProductionEffectiveLockBodyAcquisitionRefinement "
        "/\\ EffectiveLockAcquisitionModelObligation"
    )
    bridge = module._top_level_theorem_body(
        source, "EffectiveLockBodyAcquisitionCrossToolRefinement"
    )
    assert bridge is not None
    assert "DEF EffectiveLockBodyAcquisitionProductionRefinementObligation" in bridge[0]
    for proposition in (
        "ProductionEnterViewUsesPostInstallEffectiveLock",
        "ProductionBodyOwnershipPreservesEffectiveLock",
        "ProductionBodyCapacityRetirementPreservesEffectiveLock",
        "ProductionBodyServiceRefinesAsyncFairness",
    ):
        assert f"{proposition} = TRUE" in source
    assert (
        "ProductionLockedPrepareQcIdentityQuotientRefinesExactReference"
        not in source
    )
    historical = module._top_level_operator_body(
        source, "ProductionHistoricalLockedBodyRecoveryRefinement"
    )
    assert historical is not None
    assert " ".join(historical[0].split()) == (
        "/\\ ProductionEffectiveLockBodyAcquisitionRefinement "
        "/\\ ProductionDurableIntentTraceRefinesProgressWitness = TRUE"
    )


def test_audited_progress_and_rank_leaves_are_tlaps_proved() -> None:
    module = load_checker()
    ledger = module.load_ledger()
    obligations = ledger["obligations"]
    by_id = {obligation["id"]: obligation for obligation in obligations}

    assert len(obligations) == 57
    assert sum(
        obligation["status"] == "tlaps_proved"
        for obligation in obligations
    ) == 33
    assert sum(
        obligation["status"] == "specified_unproved"
        for obligation in obligations
    ) == 17
    assert by_id["async-runner-scheduler-preservation"]["status"] == "tlaps_proved"
    assert by_id["async-type-invariant"]["status"] == "tlaps_proved"
    multilane_debt = {
        "autoscale-lifecycle-production-refinement": (
            "SumeragiV2AutoscaleLifecycle",
            "AutoscaleLifecycleProductionRefinementObligation",
        ),
        "native-application-evidence-production-refinement": (
            "SumeragiV2NativeApplicationEvidence",
            "NativeApplicationEvidenceProductionRefinementObligation",
        ),
        "autonomous-reservation-carrier-production-refinement": (
            "SumeragiV2AutonomousReservationCarrier",
            "AutonomousReservationCarrierProductionRefinementObligation",
        ),
    }
    for obligation_id, (formal_module, symbol) in multilane_debt.items():
        obligation = by_id[obligation_id]
        assert obligation["module"] == formal_module
        assert obligation["symbol"] == symbol
        assert obligation["status"] == "specified_unproved"
    expected = {
        "async-progress-ownership-invariant": (
            "AsyncSpecAlwaysProgressOwnershipInvariant",
            "duplicate-free logical candidate ownership",
        ),
        "protected-service-rank-stage4-ready-causal": (
            "ProtectedStage4RankProgressFromFairScheduler",
            "Stage-4 ready-completion position",
        ),
        "protected-service-rank-serve-fifo": (
            "ProtectedServeRankProgressFromFairFifo",
            "nonce-unique Serve occurrence",
        ),
        "protected-service-rank-stage5-consensus-fifo": (
            "ProtectedStage5RankProgressFromFairFifo",
            "Consensus-I/O FIFO position",
        ),
    }
    for obligation_id, (symbol, requirement_fragment) in expected.items():
        obligation = by_id[obligation_id]
        assert obligation == {
            "id": obligation_id,
            "requirement": obligation["requirement"],
            "module": "SumeragiV2AsyncLivenessProofs",
            "symbol": symbol,
            "status": "tlaps_proved",
        }
        assert requirement_fragment in obligation["requirement"]

    reviewed_order = (
        "async-type-invariant",
        "async-progress-ownership-invariant",
        "async-fair-action-refinement",
        "protected-service-rank-stage4-ready-causal",
        "protected-service-rank-serve-fifo",
        "protected-service-rank-stage5-consensus-fifo",
        "protected-service-rank",
    )
    positions = {item["id"]: index for index, item in enumerate(obligations)}
    assert tuple(sorted(reviewed_order, key=positions.__getitem__)) == reviewed_order


@pytest.mark.parametrize(
    ("obligation_id", "reviewed_symbol"),
    (
        (
            "async-progress-ownership-invariant",
            "AsyncSpecAlwaysProgressOwnershipInvariant",
        ),
        (
            "protected-service-rank-stage4-ready-causal",
            "ProtectedStage4RankProgressFromFairScheduler",
        ),
        (
            "protected-service-rank-stage5-consensus-fifo",
            "ProtectedStage5RankProgressFromFairFifo",
        ),
    ),
)
def test_audited_progress_and_rank_leaf_inventory_mutations_fail_closed(
    obligation_id: str,
    reviewed_symbol: str,
) -> None:
    module = load_checker()
    ledger = copy.deepcopy(module.load_ledger())
    ledger["obligations"] = [
        obligation
        for obligation in ledger["obligations"]
        if obligation["id"] != obligation_id
    ]

    errors = module._proof_obligation_inventory_errors(ledger["obligations"])
    assert f"proof ledger is missing reviewed obligation {obligation_id}" in errors
    assert any("must follow the reviewed canonical order" in error for error in errors)

    ledger = copy.deepcopy(module.load_ledger())
    obligation = next(
        item for item in ledger["obligations"] if item["id"] == obligation_id
    )
    obligation["symbol"] = f"Unchecked{reviewed_symbol}"
    errors = module._proof_obligation_inventory_errors(ledger["obligations"])
    assert (
        f"proof obligation {obligation_id} must use reviewed symbol "
        f"{reviewed_symbol}"
    ) in errors


@pytest.mark.parametrize(
    ("obligation_id", "prerequisite_id"),
    (
        ("async-progress-ownership-invariant", "async-type-invariant"),
        (
            "protected-service-rank-stage4-ready-causal",
            "async-progress-ownership-invariant",
        ),
        (
            "protected-service-rank-stage4-ready-causal",
            "async-fair-action-refinement",
        ),
        (
            "protected-service-rank-stage5-consensus-fifo",
            "async-progress-ownership-invariant",
        ),
    ),
)
def test_audited_progress_and_rank_leaf_dependencies_fail_closed(
    obligation_id: str,
    prerequisite_id: str,
) -> None:
    module = load_checker()
    obligations = copy.deepcopy(module.load_ledger()["obligations"])
    by_id = {obligation["id"]: obligation for obligation in obligations}
    by_id[obligation_id]["status"] = "tlaps_proved"
    by_id[prerequisite_id]["status"] = "specified_unproved"

    errors = module._proof_status_dependency_errors(obligations)
    assert (
        f"proof obligation {obligation_id} cannot be tlaps_proved before "
        f"prerequisite {prerequisite_id} is tlaps_proved"
    ) in errors

    obligations = copy.deepcopy(module.load_ledger()["obligations"])
    leaf = next(item for item in obligations if item["id"] == obligation_id)
    obligations.remove(leaf)
    aggregate_index = next(
        index
        for index, item in enumerate(obligations)
        if item["id"] == "protected-service-rank"
    )
    obligations.insert(aggregate_index + 1, leaf)
    errors = module._proof_status_dependency_errors(obligations)
    assert (
        "proof obligation protected-service-rank must appear after prerequisite "
        f"{obligation_id}"
    ) in errors


def test_protected_serve_fifo_rank_is_separate_tlaps_leaf() -> None:
    module = load_checker()
    ledger = module.load_ledger()
    by_id = {obligation["id"]: obligation for obligation in ledger["obligations"]}

    serve_fifo = by_id["protected-service-rank-serve-fifo"]
    assert serve_fifo == {
        "id": "protected-service-rank-serve-fifo",
        "requirement": serve_fifo["requirement"],
        "module": "SumeragiV2AsyncLivenessProofs",
        "symbol": "ProtectedServeRankProgressFromFairFifo",
        "status": "tlaps_proved",
    }
    assert "nonce-unique Serve occurrence" in serve_fifo["requirement"]
    assert "production request admission" in serve_fifo["requirement"]

    aggregate = by_id["protected-service-rank"]
    assert aggregate["status"] == "specified_unproved"
    assert ledger["obligations"].index(serve_fifo) < ledger["obligations"].index(
        aggregate
    )


def test_protected_serve_fifo_rank_dependencies_and_order_fail_closed() -> None:
    module = load_checker()
    obligations = copy.deepcopy(module.load_ledger()["obligations"])
    by_id = {obligation["id"]: obligation for obligation in obligations}
    by_id["protected-service-rank-serve-fifo"]["status"] = "tlaps_proved"
    by_id["async-type-invariant"]["status"] = "specified_unproved"

    errors = module._proof_status_dependency_errors(obligations)
    assert (
        "proof obligation protected-service-rank-serve-fifo cannot be "
        "tlaps_proved before prerequisite async-type-invariant is tlaps_proved"
    ) in errors

    obligations = copy.deepcopy(module.load_ledger()["obligations"])
    serve_fifo = next(
        obligation
        for obligation in obligations
        if obligation["id"] == "protected-service-rank-serve-fifo"
    )
    obligations.remove(serve_fifo)
    aggregate_index = next(
        index
        for index, obligation in enumerate(obligations)
        if obligation["id"] == "protected-service-rank"
    )
    obligations.insert(aggregate_index + 1, serve_fifo)

    errors = module._proof_status_dependency_errors(obligations)
    assert (
        "proof obligation protected-service-rank must appear after prerequisite "
        "protected-service-rank-serve-fifo"
    ) in errors


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


def test_effective_lock_acquisition_proof_dependency_removal_fails_closed(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_effective_lock_acquisition_fixture(tmp_path, module)
    path = formal_dir / "SumeragiV2EffectiveLockAcquisitionProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        delete_tla_theorem_token(
            source,
            "EffectiveLockAcquisitionModelObligation",
            "AcquisitionSpecProvidesEffectiveLockProgress",
        ),
        encoding="utf-8",
    )

    errors = module._effective_lock_acquisition_source_fidelity_errors(formal_dir)
    assert any(
        "must compose the reviewed type, progress, and stable-delivery "
        "proof dependencies"
        in error
        for error in errors
    ), errors


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


def copy_reply_route_ownership_fixture(
    tmp_path: Path, module
) -> tuple[Path, Path]:
    """Copy the exact reply-route ownership model, configs, and runner."""

    repo_root = tmp_path / "repo"
    formal_dir = repo_root / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for name in module._REPLY_ROUTE_FORMAL_SOURCE_SHA256:
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)
    runner = repo_root / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh"
    runner.parent.mkdir(parents=True)
    shutil.copy2(
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh",
        runner,
    )
    return repo_root, formal_dir


def test_reply_route_ownership_source_fidelity_fails_closed(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_reply_route_ownership_fixture(tmp_path, module)

    assert module._reply_route_ownership_source_fidelity_errors(
        formal_dir, repo_root
    ) == []

    mutations = (
        (
            "SumeragiV2ReplyRouteOwnership.tla",
            "          !.ticketTenure = NoReplyTicketTenure]\n",
            "          !.ticketTenure = NoReplyTicketTenure,\n"
            "          !.messageCursor = 0]\n",
            "ReplyAttemptWithRoute may not contain cursor-reset",
        ),
        (
            "SumeragiV2ReplyRouteOwnership.tla",
            "                 /\\ newAttempt.messageCursor >= oldAttempt.messageCursor\n",
            "                 /\\ newAttempt.messageCursor = 0\n",
            "ReplyTenureAwareReplayStep must retain non-regressing",
        ),
        (
            "SumeragiV2ReplyRouteOwnership.tla",
            "                 /\\ ReplyAttemptCursor(newAttempt) =\n"
            "                      ReplyAttemptCursor(oldAttempt)\n",
            "                 /\\ ReplyAttemptCursor(newAttempt) = <<0, 0>>\n",
            "ReplyTenureAwareReplayStep must retain non-regressing",
        ),
        (
            "SumeragiV2ReplyRouteOwnership.tla",
            "        ~> ReplySourceAdvancedFrom(owner, semantic, source,\n",
            "        ~> ReplySourceAtCursor(owner, semantic, source,\n",
            "ReplySourceEventuallyProgresses must retain non-regressing",
        ),
        (
            "SumeragiV2ReplyRouteOwnership.tla",
            "  /\\ left.semantic = right.semantic\n",
            "  /\\ left.semantic = left.semantic\n",
            "SameReplyAttemptIdentity must retain non-regressing",
        ),
        (
            "SumeragiV2ReplyRouteOwnership.tla",
            "  /\\ \\A retainedBefore \\in rrAttempts:\n"
            "       \\E retainedAfter \\in rrAttempts':\n"
            "         SameReplyAttemptIdentity(retainedBefore, retainedAfter)\n",
            "  /\\ TRUE\n",
            "ReplySourceIsolationStep must retain non-regressing",
        ),
        (
            "SumeragiV2ReplyRouteOwnership.tla",
            "             attemptChanged == changedAfter # changedBefore\n",
            "             attemptChanged == FALSE\n",
            "ReplySourceIsolationStep must retain non-regressing",
        ),
        (
            "SumeragiV2ReplyRouteOwnership.tla",
            "                     /\\ ReplyAttemptCursor(otherAfter) =\n"
            "                          ReplyAttemptCursor(otherBefore)\n",
            "                     /\\ ReplyAttemptCursor(otherAfter) = <<0, 0>>\n",
            "ReplySourceIsolationStep must retain non-regressing",
        ),
        (
            "SumeragiV2AsyncNetworkReplyRoutes.tla",
            "AsyncReplySourceIsolation ==\n"
            "  AsyncReplyRoute!ReplySourceIsolation\n",
            "AsyncReplySourceIsolation == TRUE\n",
            "AsyncReplySourceIsolation must retain non-regressing",
        ),
        (
            "SumeragiV2AsyncNetworkReplyRoutes.tla",
            "AsyncReplyTenureAwareReplay ==\n"
            "  AsyncReplyRoute!ReplyTenureAwareReplay\n",
            "AsyncReplyTenureAwareReplay == TRUE\n",
            "AsyncReplyTenureAwareReplay must retain non-regressing",
        ),
        (
            "SumeragiV2ReplyRouteOwnershipMutation.tla",
            "     /\\ attempts' = {updatedAttempt}\n",
            "     /\\ attempts' =\n"
            "          MutationRoute!ReplaceReplyAttempt(oldAttempt, updatedAttempt)\n",
            "BuggyLaterDeliveryReplacesAlternateSource must retain non-regressing",
        ),
        (
            "SumeragiV2ReplyRouteOwnershipMutation.tla",
            "     /\\ MutationRoute!ReplyAttemptOwned(0, RequestA, 1)\n"
            "     /\\ MutationRoute!ReplyAttemptOwned(0, RequestB, 0)\n",
            "     /\\ MutationRoute!ReplyAttemptOwned(0, RequestA, 1)\n"
            "     /\\ MutationRoute!ReplyAttemptOwned(0, RequestA, 0)\n",
            "BuggyLaterDeliveryReplacesAlternateSource must retain non-regressing",
        ),
        (
            "SumeragiV2ReplyRouteOwnershipMutation.tla",
            "  /\\ MutationRoute!ReplyTenureAwareReplay\n",
            "  /\\ TRUE\n",
            "RouteMutationTemporalProperties must retain non-regressing",
        ),
        (
            "SumeragiV2ReplyRouteOwnershipMutation.tla",
            "  /\\ MutationRoute!ReplySourceIsolation\n",
            "  /\\ TRUE\n",
            "RouteMutationTemporalProperties must retain non-regressing",
        ),
        (
            "SumeragiV2ReplyRouteOwnershipMutation.tla",
            "  /\\ BothSemanticAttemptsRetained\n",
            "  /\\ TRUE\n",
            "RouteMutationSafety must retain non-regressing",
        ),
        (
            "reply_route_fixed.cfg",
            "PROPERTY RouteMutationTemporalProperties\n",
            "PROPERTY MutationRoute!ReplyTenureAwareReplay\n",
            "reply-route configuration must retain reviewed fragment",
        ),
    )
    for name, old, new, expected_error in mutations:
        path = formal_dir / name
        canonical = (module.FORMAL_DIR / name).read_text(encoding="utf-8")
        assert canonical.count(old) == 1, (name, old)
        path.write_text(canonical.replace(old, new, 1), encoding="utf-8")
        errors = module._reply_route_ownership_source_fidelity_errors(
            formal_dir, repo_root
        )
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        path.write_text(canonical, encoding="utf-8")

    retired = formal_dir / "reply_route_cursor_preservation_bug.cfg"
    retired.write_text("CHECK_DEADLOCK FALSE\n", encoding="utf-8")
    errors = module._reply_route_ownership_source_fidelity_errors(
        formal_dir, repo_root
    )
    assert any(
        "cursor-preservation mutation name is retired" in error
        for error in errors
    ), errors


def test_reply_route_ownership_mutation_runner_fails_closed(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_reply_route_ownership_fixture(tmp_path, module)
    runner = repo_root / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh"
    source = runner.read_text(encoding="utf-8")

    mutations = (
        (
            "  reply_route_cursor_reset_bug.cfg 12 \\\n",
            "  reply_route_cursor_reset_bug.cfg 0 \\\n",
            "reply-route-cursor-reset-bug exactly once with status 12",
        ),
        (
            "  reply_route_cursor_reset_bug.cfg 12 \\\n",
            "  reply_route_cursor_preservation_bug.cfg 12 \\\n",
            "reply-route-cursor-reset-bug exactly once with status 12",
        ),
        (
            '  "attempts = { [ connectionTenure |-> 1"\n',
            '  "attempts were replaced"\n',
            "must require exact counterexample marker",
        ),
    )
    for old, new, expected_error in mutations:
        assert source.count(old) == 1, old
        runner.write_text(source.replace(old, new, 1), encoding="utf-8")
        errors = module._reply_route_ownership_source_fidelity_errors(
            formal_dir, repo_root
        )
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )

    runner.write_text(
        source + "\n# reply_route_cursor_preservation_bug.cfg\n",
        encoding="utf-8",
    )
    errors = module._reply_route_ownership_source_fidelity_errors(
        formal_dir, repo_root
    )
    assert any(
        "retired reply-route cursor-preservation configuration name" in error
        for error in errors
    ), errors

    fixed_start = source.index("run_case reply-route-fixed")
    reset_start = source.index("run_case reply-route-cursor-reset-bug", fixed_start)
    replacement_start = source.index(
        "run_case reply-route-source-replacement-bug", reset_start
    )
    echo_start = source.index("echo ", replacement_start)
    fixed_block = source[fixed_start:reset_start]
    reset_block = source[reset_start:replacement_start]
    runner.write_text(
        source[:fixed_start]
        + reset_block
        + fixed_block
        + source[replacement_start:echo_start]
        + source[echo_start:],
        encoding="utf-8",
    )
    errors = module._reply_route_ownership_source_fidelity_errors(
        formal_dir, repo_root
    )
    assert any("must retain reviewed order" in error for error in errors), errors


def copy_effect_capacity_mutation_fixture(tmp_path: Path, module) -> tuple[Path, Path]:
    """Copy the exact 35-file effect-capacity corpus and production seam."""

    repo_root = tmp_path / "repo"
    formal_dir = repo_root / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for name in module.EFFECT_CAPACITY_MUTATION_FORMAL_ARTIFACTS:
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)
    runner = repo_root / module.EFFECT_CAPACITY_MUTATION_RUNNER
    runner.parent.mkdir(parents=True)
    shutil.copy2(ROOT_DIR / module.EFFECT_CAPACITY_MUTATION_RUNNER, runner)
    effects = repo_root / "crates/iroha_core/src/sumeragi/v2_effects.rs"
    effects.parent.mkdir(parents=True)
    shutil.copy2(
        ROOT_DIR / "crates/iroha_core/src/sumeragi/v2_effects.rs",
        effects,
    )
    return repo_root, formal_dir


def test_effect_capacity_mutation_source_seal_covers_exact_corpus(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_effect_capacity_mutation_fixture(tmp_path, module)

    assert len(module.EFFECT_CAPACITY_MUTATION_FORMAL_ARTIFACTS) == 34
    assert (
        sum(
            name.endswith(".tla")
            for name in module.EFFECT_CAPACITY_MUTATION_FORMAL_ARTIFACTS
        )
        == 6
    )
    assert (
        sum(
            name.endswith(".cfg")
            for name in module.EFFECT_CAPACITY_MUTATION_FORMAL_ARTIFACTS
        )
        == 28
    )
    assert len(module.EFFECT_CAPACITY_MUTATION_SHA256) == 35
    assert module._effect_capacity_mutation_source_fidelity_errors(
        formal_dir, repo_root
    ) == []

    runner = repo_root / module.EFFECT_CAPACITY_MUTATION_RUNNER
    runner_source = runner.read_text(encoding="utf-8")

    def assert_runner_mutation_rejected(
        mutated_source: str, expected_error: str
    ) -> None:
        assert mutated_source != runner_source
        runner.write_text(mutated_source, encoding="utf-8")
        errors = module._effect_capacity_mutation_runner_errors(repo_root)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        runner.write_text(runner_source, encoding="utf-8")

    assert_runner_mutation_rejected(
        runner_source.replace(
            "effect_capacity_timeout_sign_fixed.cfg 0 \\",
            "effect_capacity_timeout_sign_fixed.cfg 12 \\",
            1,
        ),
        "found repaired=9, mutants=19",
    )
    assert_runner_mutation_rejected(
        runner_source.replace(
            "effect_capacity_timeout_sign_lost_bug.cfg 12 \\",
            "effect_capacity_timeout_sign_lost_bug.cfg 0 \\",
            1,
        ),
        "found repaired=11, mutants=17",
    )
    assert_runner_mutation_rejected(
        runner_source.replace(
            "36 states generated, 36 distinct states found",
            "37 states generated, 36 distinct states found",
            1,
        ),
        "found generated=162, distinct=160, parsed_cases=28",
    )
    assert_runner_mutation_rejected(
        runner_source.replace(
            "36 states generated, 36 distinct states found",
            "36 states generated, 35 distinct states found",
            1,
        ),
        "found generated=161, distinct=159, parsed_cases=28",
    )
    role_mutation = runner_source.replace(
        "certified-request-capacity-lost",
        "certified-request-capacity-role-swap",
        1,
    ).replace(
        "certified-request-capacity-fatal",
        "certified-request-capacity-lost",
        1,
    ).replace(
        "certified-request-capacity-role-swap",
        "certified-request-capacity-fatal",
        1,
    )
    assert_runner_mutation_rejected(
        role_mutation,
        "certified-request role effect_capacity_certified_request_lost_bug.cfg",
    )


@pytest.mark.parametrize(
    "artifact_name",
    (
        "SumeragiV2CertifiedRequestCapacityMutation.tla",
        "SumeragiV2EffectCapacityOuterTransportMutation.tla",
        "SumeragiV2EffectCapacityOwnershipMutation.tla",
        "effect_capacity_certified_request_fixed.cfg",
        "effect_capacity_outer_transport_chunk_class_bug.cfg",
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
    for name in (
        "SumeragiV2CertifiedRequestCapacityMutation.tla",
        "effect_capacity_certified_request_fixed.cfg",
        "effect_capacity_timeout_sign_fixed.cfg",
    ):
        missing = formal_dir / name
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

    for name in (
        "SumeragiV2CertifiedRequestCapacityUnreviewedMutation.tla",
        "effect_capacity_certified_request_unreviewed.cfg",
        "effect_capacity_unreviewed.cfg",
    ):
        extra = formal_dir / name
        extra.write_text("SPECIFICATION Spec\n", encoding="utf-8")
        errors = module._effect_capacity_mutation_source_fidelity_errors(
            formal_dir, repo_root
        )
        assert any(
            str(extra) in error
            and "extra effect-capacity mutation artifact" in error
            for error in errors
        ), errors
        extra.unlink()


def test_effect_capacity_production_source_fidelity_is_green(tmp_path: Path) -> None:
    module = load_checker()
    repo_root, _formal_dir = copy_effect_capacity_mutation_fixture(tmp_path, module)

    assert module._effect_capacity_production_source_fidelity_errors(repo_root) == []


@pytest.mark.parametrize(
    ("item_name", "old", "new", "diagnostic"),
    (
        (
            "drain_retained_effect_batch",
            "Err(error) => return Err(error),",
            """Err(EffectExecutorError::CertifiedRequestCapacity { .. })
                if pending_work_producer == Some(PendingWorkProducer::Fetch) =>
            {
                break;
            }
            Err(error) => return Err(error),""",
            "retained-effect dispatch may not retry CertifiedRequestCapacity",
        ),
        (
            "begin_fetch",
            """"deferred certified Sumeragi v2 body fetch at request capacity"
                    );
                    return Ok(());""",
            """"deferred certified Sumeragi v2 body fetch at request capacity"
                    );
                    return Err(EffectExecutorError::CertifiedRequestCapacity {
                        capacity,
                    });""",
            "new and existing Fetch Q-capacity deferrals must return success",
        ),
        (
            "begin_fetch",
            """"deferred certified Sumeragi v2 body-fetch authority upgrade at request capacity"
                        );
                        return Ok(());""",
            """"deferred certified Sumeragi v2 body-fetch authority upgrade at request capacity"
                        );
                        self.pending_fetches.remove(&existing_id);
                        return Ok(());""",
            "new and existing Fetch Q-capacity deferrals must return success",
        ),
        (
            "network_ingress_requires_reducer_order",
            "wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)",
            "wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)",
            "exhaustive transport/reducer ingress classification",
        ),
        (
            "network_ingress_requires_reducer_order",
            "wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)",
            "wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)",
            "exhaustive transport/reducer ingress classification",
        ),
        (
            "retained_dispatch_allows_network_ingress",
            "|| !Self::network_ingress_requires_reducer_order(payload)",
            "&& !Self::network_ingress_requires_reducer_order(payload)",
            "retained dispatch transport-completion bypass",
        ),
        (
            "can_admit_network_message_with_ingress_ownership",
            "self.retained_dispatch_allows_network_ingress(&message.payload)",
            "true",
            "public ownership-aware retained-debt capacity preflight declaration and complete control flow",
        ),
        (
            "can_admit_network_message_with_ingress_ownership",
            ".can_admit_network_message_with_ingress_ownership(message, ingress_ownership)",
            ".can_admit_network_message(message)",
            "public ownership-aware retained-debt capacity preflight declaration and complete control flow",
        ),
        (
            "enqueue_network_with_ingress_ownership",
            "if matches!(&result, Err(NetworkIngressError::FailClosed))",
            "if false",
            "public ownership-aware network admission and fail-stop latch declaration and complete control flow",
        ),
    ),
)
def test_effect_capacity_production_source_fidelity_rejects_semantic_mutants(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    diagnostic: str,
) -> None:
    module = load_checker()
    repo_root, _formal_dir = copy_effect_capacity_mutation_fixture(tmp_path, module)
    effects_path = repo_root / "crates/iroha_core/src/sumeragi/v2_effects.rs"
    source = effects_path.read_text(encoding="utf-8")
    items = module.rust_items(source, item_name)
    assert len(items) == 1
    item = items[0]
    assert item.source.count(old) == 1, (item_name, old)
    item_start = source.index(item.source)
    item_end = item_start + len(item.source)
    effects_path.write_text(
        source[:item_start]
        + item.source.replace(old, new, 1)
        + source[item_end:],
        encoding="utf-8",
    )

    errors = module._effect_capacity_production_source_fidelity_errors(repo_root)

    assert any(diagnostic in error for error in errors), errors


def copy_post_decision_timeout_mutation_fixture(
    tmp_path: Path, module
) -> tuple[Path, Path]:
    """Copy the exact eleven-file post-Decision timeout mutation corpus."""

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

    assert len(module.POST_DECISION_TIMEOUT_MUTATION_FORMAL_ARTIFACTS) == 10
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
        == 9
    )
    assert len(module.POST_DECISION_TIMEOUT_MUTATION_SHA256) == 11
    assert module._post_decision_timeout_mutation_source_fidelity_errors(
        formal_dir, repo_root
    ) == []


@pytest.mark.parametrize(
    "artifact_name",
    (
        "SumeragiV2PostDecisionTimeoutMutation.tla",
        "post_decision_resume_timeout_guard_bug.cfg",
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


def copy_certified_response_registration_fixture(
    tmp_path: Path, module
) -> tuple[Path, Path]:
    """Copy the exact seven-file corpus and its production TLA+ seam."""

    repo_root = tmp_path / "repo"
    formal_dir = repo_root / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for name in module.CERTIFIED_RESPONSE_REGISTRATION_FORMAL_ARTIFACTS:
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)
    shutil.copy2(
        module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla",
        formal_dir / "SumeragiV2AsyncNetwork.tla",
    )
    runner = repo_root / module.CERTIFIED_RESPONSE_REGISTRATION_RUNNER
    runner.parent.mkdir(parents=True)
    shutil.copy2(ROOT_DIR / module.CERTIFIED_RESPONSE_REGISTRATION_RUNNER, runner)
    return repo_root, formal_dir


def copy_decision_recovery_lifecycle_fixture(
    tmp_path: Path, module
) -> tuple[Path, Path]:
    """Copy the exact eleven-file Decision recovery lifecycle corpus."""

    repo_root = tmp_path / "repo"
    formal_dir = repo_root / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for name in module.DECISION_RECOVERY_LIFECYCLE_FORMAL_ARTIFACTS:
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)
    runner = repo_root / module.DECISION_RECOVERY_LIFECYCLE_RUNNER
    runner.parent.mkdir(parents=True)
    shutil.copy2(ROOT_DIR / module.DECISION_RECOVERY_LIFECYCLE_RUNNER, runner)
    return repo_root, formal_dir


def test_certified_response_registration_source_seal_covers_exact_corpus(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_certified_response_registration_fixture(
        tmp_path, module
    )

    assert len(module.CERTIFIED_RESPONSE_REGISTRATION_FORMAL_ARTIFACTS) == 6
    assert (
        sum(
            name.endswith(".tla")
            for name in module.CERTIFIED_RESPONSE_REGISTRATION_FORMAL_ARTIFACTS
        )
        == 1
    )
    assert (
        sum(
            name.endswith(".cfg")
            for name in module.CERTIFIED_RESPONSE_REGISTRATION_FORMAL_ARTIFACTS
        )
        == 5
    )
    assert len(module.CERTIFIED_RESPONSE_REGISTRATION_SHA256) == 7
    assert (
        "_certified_response_registration_mutation_source_fidelity_errors"
        in module.validate_ledger.__code__.co_names
    )
    assert module._certified_response_registration_mutation_source_fidelity_errors(
        formal_dir, repo_root
    ) == []

    lifecycle_root, lifecycle_formal = copy_decision_recovery_lifecycle_fixture(
        tmp_path / "decision-recovery-lifecycle", module
    )
    assert len(module.DECISION_RECOVERY_LIFECYCLE_FORMAL_ARTIFACTS) == 10
    assert (
        sum(
            name.endswith(".tla")
            for name in module.DECISION_RECOVERY_LIFECYCLE_FORMAL_ARTIFACTS
        )
        == 1
    )
    assert (
        sum(
            name.endswith(".cfg")
            for name in module.DECISION_RECOVERY_LIFECYCLE_FORMAL_ARTIFACTS
        )
        == 9
    )
    assert len(module.DECISION_RECOVERY_LIFECYCLE_SHA256) == 11
    assert (
        "_decision_recovery_lifecycle_mutation_source_fidelity_errors"
        in module.validate_ledger.__code__.co_names
    )
    assert module._decision_recovery_lifecycle_mutation_source_fidelity_errors(
        lifecycle_formal, lifecycle_root
    ) == []


@pytest.mark.parametrize(
    "artifact_name",
    (
        "SumeragiV2CertifiedResponseRegistrationMutation.tla",
        "certified_response_registration_historical_fixed.cfg",
        "certified_response_registration_restart_missing_guard.cfg",
        "scripts/formal/run_sumeragi_v2_certified_response_registration_mutation.sh",
    ),
)
def test_certified_response_registration_source_seal_rejects_stale_artifact(
    tmp_path: Path,
    artifact_name: str,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_certified_response_registration_fixture(
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

    errors = module._certified_response_registration_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )

    assert any(
        str(path) in error and "must match exact reviewed SHA-256" in error
        for error in errors
    ), errors


def test_certified_response_registration_source_seal_rejects_inventory_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, formal_dir = copy_certified_response_registration_fixture(
        tmp_path, module
    )
    missing = formal_dir / "certified_response_registration_duplicate_fixed.cfg"
    missing.unlink()
    errors = module._certified_response_registration_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )
    assert any(
        str(missing) in error
        and "missing certified-response registration artifact" in error
        for error in errors
    ), errors

    shutil.copy2(module.FORMAL_DIR / missing.name, missing)
    extra = formal_dir / "certified_response_registration_unreviewed.cfg"
    extra.write_text("SPECIFICATION Spec\n", encoding="utf-8")
    errors = module._certified_response_registration_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )
    assert any(
        str(extra) in error
        and "extra certified-response registration artifact" in error
        for error in errors
    ), errors

    extra.unlink()
    symlink = formal_dir / "certified_response_registration_restart_fixed.cfg"
    target = formal_dir / "certified_response_registration_restart_fixed.target"
    symlink.rename(target)
    symlink.symlink_to(target.name)
    errors = module._certified_response_registration_mutation_source_fidelity_errors(
        formal_dir, repo_root
    )
    assert any(
        str(symlink) in error and "artifact must be a regular file" in error
        for error in errors
    ), errors
    symlink.unlink()
    target.rename(symlink)

    lifecycle_root, lifecycle_formal = copy_decision_recovery_lifecycle_fixture(
        tmp_path / "decision-recovery-lifecycle", module
    )
    stale = lifecycle_formal / "SumeragiV2DecisionRecoveryLifecycleMutation.tla"
    stale.write_text(
        stale.read_text(encoding="utf-8") + "\n\\* stale mutation\n",
        encoding="utf-8",
    )
    errors = module._decision_recovery_lifecycle_mutation_source_fidelity_errors(
        lifecycle_formal, lifecycle_root
    )
    assert any(
        str(stale) in error and "must match exact reviewed SHA-256" in error
        for error in errors
    ), errors
    shutil.copy2(module.FORMAL_DIR / stale.name, stale)

    missing = lifecycle_formal / "decision_recovery_lifecycle_fixed.cfg"
    missing.unlink()
    errors = module._decision_recovery_lifecycle_mutation_source_fidelity_errors(
        lifecycle_formal, lifecycle_root
    )
    assert any(
        str(missing) in error
        and "missing Decision recovery lifecycle artifact" in error
        for error in errors
    ), errors
    shutil.copy2(module.FORMAL_DIR / missing.name, missing)

    extra = lifecycle_formal / "decision_recovery_lifecycle_unreviewed.cfg"
    extra.write_text("SPECIFICATION Spec\n", encoding="utf-8")
    errors = module._decision_recovery_lifecycle_mutation_source_fidelity_errors(
        lifecycle_formal, lifecycle_root
    )
    assert any(
        str(extra) in error and "extra Decision recovery lifecycle artifact" in error
        for error in errors
    ), errors
    extra.unlink()

    symlink = (
        lifecycle_formal
        / "decision_recovery_lifecycle_stale_executor_generation_bug.cfg"
    )
    target = lifecycle_formal / "decision_recovery_lifecycle_stale_executor.target"
    symlink.rename(target)
    symlink.symlink_to(target.name)
    errors = module._decision_recovery_lifecycle_mutation_source_fidelity_errors(
        lifecycle_formal, lifecycle_root
    )
    assert any(
        str(symlink) in error and "artifact must be a regular file" in error
        for error in errors
    ), errors
    symlink.unlink()
    target.rename(symlink)

    runner = lifecycle_root / module.DECISION_RECOVERY_LIFECYCLE_RUNNER
    runner.unlink()
    errors = module._decision_recovery_lifecycle_mutation_source_fidelity_errors(
        lifecycle_formal, lifecycle_root
    )
    assert any(
        str(runner) in error
        and "missing Decision recovery lifecycle runner" in error
        for error in errors
    ), errors
    shutil.copy2(ROOT_DIR / module.DECISION_RECOVERY_LIFECYCLE_RUNNER, runner)

    extra_runner = (
        runner.parent
        / "run_sumeragi_v2_decision_recovery_lifecycle_unreviewed_mutation.sh"
    )
    extra_runner.write_text("#!/usr/bin/env bash\n", encoding="utf-8")
    errors = module._decision_recovery_lifecycle_mutation_source_fidelity_errors(
        lifecycle_formal, lifecycle_root
    )
    assert any(
        str(extra_runner) in error
        and "extra Decision recovery lifecycle runner" in error
        for error in errors
    ), errors
    extra_runner.unlink()

    runner_target = runner.with_suffix(".target")
    runner.rename(runner_target)
    runner.symlink_to(runner_target.name)
    errors = module._decision_recovery_lifecycle_mutation_source_fidelity_errors(
        lifecycle_formal, lifecycle_root
    )
    assert any(
        str(runner) in error and "runner must be a regular file" in error
        for error in errors
    ), errors


def test_certified_response_registration_runner_rejects_semantic_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    repo_root, _formal_dir = copy_certified_response_registration_fixture(
        tmp_path, module
    )
    runner = repo_root / module.CERTIFIED_RESPONSE_REGISTRATION_RUNNER
    source = runner.read_text(encoding="utf-8")

    mutations = (
        (
            source.replace("if (($#)); then", "if false; then", 1),
            "argument rejection exactly 1 time(s); found 0",
        ),
        (
            source.replace("run_case restart-fixed", "run_case duplicate-fixed", 1),
            "must execute exactly five sealed cases in order",
        ),
        (
            source.replace(
                "11 states generated, 11 distinct states found",
                "12 states generated, 11 distinct states found",
                1,
            ),
            "must report exactly 36 generated states",
        ),
    )
    for mutated_source, expected_error in mutations:
        assert mutated_source != source
        runner.write_text(mutated_source, encoding="utf-8")
        errors = module._certified_response_registration_runner_errors(repo_root)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
    runner.write_text(source, encoding="utf-8")

    lifecycle_root, _lifecycle_formal = copy_decision_recovery_lifecycle_fixture(
        tmp_path / "decision-recovery-lifecycle", module
    )
    lifecycle_runner = lifecycle_root / module.DECISION_RECOVERY_LIFECYCLE_RUNNER
    lifecycle_source = lifecycle_runner.read_text(encoding="utf-8")
    lifecycle_mutations = (
        (
            lifecycle_source.replace("if (($#)); then", "if false; then", 1),
            "argument rejection exactly 1 time(s); found 0",
        ),
        (
            lifecycle_source.replace(
                "run_case stale-executor-generation",
                "run_case prepare-certificate-authority",
                1,
            ),
            "must execute exactly nine sealed cases in order",
        ),
        (
            lifecycle_source.replace(
                "7 states generated, 7 distinct states found",
                "8 states generated, 7 distinct states found",
                1,
            ),
            "must report exactly 42 generated states",
        ),
    )
    for mutated_source, expected_error in lifecycle_mutations:
        assert mutated_source != lifecycle_source
        lifecycle_runner.write_text(mutated_source, encoding="utf-8")
        errors = module._decision_recovery_lifecycle_runner_errors(lifecycle_root)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
    lifecycle_runner.write_text(lifecycle_source, encoding="utf-8")


def test_certified_response_registration_source_fidelity_rejects_guard_and_order_mutants(
    tmp_path: Path,
) -> None:
    module = load_checker()
    _repo_root, formal_dir = copy_certified_response_registration_fixture(
        tmp_path, module
    )
    async_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = async_path.read_text(encoding="utf-8")
    assert (
        module._certified_response_registration_production_source_fidelity_errors(
            formal_dir
        )
        == []
    )

    without_guard = mutate_tla_operator(
        source,
        "CertifiedResponseAuthorized",
        "  /\\ MatchingCertifiedRequests(item) # {}\n",
        "",
    )
    async_path.write_text(without_guard, encoding="utf-8")
    errors = (
        module._certified_response_registration_production_source_fidelity_errors(
            formal_dir
        )
    )
    assert any(
        "must require one exact live matching certified request" in error
        for error in errors
    ), errors

    matching_start = source.index("MatchingCertifiedRequests(response) ==")
    authorization_start = source.index("CertifiedResponseAuthorized(item) ==")
    following_start = source.index(
        "CommitCertificateRequestAuthorized(item) ==", authorization_start
    )
    matching_block = source[matching_start:authorization_start]
    authorization_block = source[authorization_start:following_start]
    reordered = (
        source[:matching_start]
        + authorization_block
        + matching_block
        + source[following_start:]
    )
    async_path.write_text(reordered, encoding="utf-8")
    errors = (
        module._certified_response_registration_production_source_fidelity_errors(
            formal_dir
        )
    )
    assert any(
        "MatchingCertifiedRequests must be defined before "
        "CertifiedResponseAuthorized" in error
        for error in errors
    ), errors

    wrong_identity = mutate_tla_operator(
        source,
        "MatchingCertifiedRequests",
        "request.envelope.subject = response.envelope.subject",
        "request.envelope.subject # response.envelope.subject",
    )
    async_path.write_text(wrong_identity, encoding="utf-8")
    errors = (
        module._certified_response_registration_production_source_fidelity_errors(
            formal_dir
        )
    )
    assert any(
        "must retain exact outstanding-request identity" in error
        for error in errors
    ), errors


def test_successor_activation_and_exact_recovery_refinement_has_reviewed_bridge() -> None:
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
        "/\\ ProductionStartupFailureAndRestartRefinesIndexedLifecycle = TRUE "
        "/\\ ProductionHistoricalCertificateTraceRefinesIndexedAsync = TRUE "
        "/\\ ProductionHistoricalBodyPipelineTraceRefinesIndexedAsync = TRUE"
    )
    ledger_operator = module._top_level_operator_body(
        source,
        "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation",
    )
    assert ledger_operator is not None
    assert " ".join(ledger_operator[0].split()) == (
        "/\\ ProductionSuccessorAndExactRecoveryTraceRefinement "
        "/\\ (IndexedChainSpec => []"
        "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant)"
    )
    assert (
        module._top_level_theorem_body(
            source,
            "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation",
        )
        is None
    )
    bridge = module._top_level_theorem_body(
        source,
        "SuccessorActivationAndExactHistoricalRecoveryCrossToolRefinement",
    )
    assert bridge is not None
    bridge_parts = re.split(
        r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", bridge[0], maxsplit=1
    )
    assert " ".join(bridge_parts[0].split()) == (
        "ProductionSuccessorAndExactRecoveryTraceRefinement => "
        "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation"
    )
    assert len(bridge_parts) == 2
    assert " ".join(bridge_parts[1].split()) == (
        "BY IndexedChainSpecEstablishesSuccessorActivationAndExactHistoricalRecoveryInvariant "
        "DEF SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation"
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
    candidate_proof = " ".join(theorem_parts[1].split())
    assert "IndexedChainSpecEstablishesSuccessorActivationRankProgress" in (
        candidate_proof
    )
    assert (
        "IndexedChainSpecEstablishesSuccessorActivationStarvationFreedom"
        in candidate_proof
    )
    chain_source = (
        module.FORMAL_DIR / "SumeragiV2ChainEpochRefinement.tla"
    ).read_text(encoding="utf-8")
    spec = module._top_level_operator_body(chain_source, "IndexedChainSpec")
    assert spec is not None
    assert "EventualFailureFreeSuccessorStartupSuffix" in spec[0]
    assert (
        module._successor_activation_rank_source_fidelity_errors(module.FORMAL_DIR)
        == []
    )


def test_successor_production_source_is_bound() -> None:
    module = load_checker()
    assert module._successor_production_source_fidelity_errors(ROOT_DIR) == []


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
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn initial_block_sync_deadline(",
            "if eager_recovery {\n        height_started_at\n    } else {",
            "if eager_recovery {\n"
            "        deadline_after(height_started_at, round_timeout)\n"
            "    } else {",
            "recovery-scoped eager block-sync initial_block_sync_deadline "
            "declaration and complete control flow",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn run_inner(",
            "admitted_discovered_commit_qc = true;",
            "admitted_discovered_commit_qc = false;",
            "only authenticated discovered CommitQC admission/coalescing with "
            "serialized reducer ownership may turn an outstanding request from "
            "Some to None and retain eager block-sync",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "const fn retain_eager_block_sync(",
            "recovering_interrupted_tip || admitted_discovered_commit_qc",
            "{ let _ = admitted_discovered_commit_qc; recovering_interrupted_tip }",
            "recovery-scoped eager block-sync retain_eager_block_sync "
            "declaration and complete control flow",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "fn publish_recovered_v2_successor_height_at(",
            "set_v2_status_at(successor, now);",
            "update_v2_successor_work_stage_at(finalized_height, SumeragiV2LocalWorkStage::Running, SumeragiV2LocalWorkStage::Complete, now)?; set_v2_status_at(successor, now);",
            "may not fabricate physical predecessor completion",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "fn activate_v2_successor_height_at(",
            "validate_v2_predecessor_status(\n"
            "        &predecessor_status,\n"
            "        finalized_height,\n"
            "        SumeragiV2LocalWorkStage::Running,\n"
            "    )?;",
            "let _ = &predecessor_status;",
            "activate_v2_successor_height_at omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "fn activate_v2_successor_height_at(",
            "predecessor_status_height: predecessor_status.height,",
            "predecessor_status_height: finalized_height,",
            "activate_v2_successor_height_at omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "fn activate_v2_successor_height_at(",
            "update_v2_successor_work_stage_at(\n"
            "        finalized_height,\n"
            "        SumeragiV2LocalWorkStage::Running,\n"
            "        SumeragiV2LocalWorkStage::Complete,\n"
            "        now,\n"
            "    )?;",
            "update_v2_successor_work_stage_at(finalized_height, SumeragiV2LocalWorkStage::Running, SumeragiV2LocalWorkStage::Running, now)?;",
            "activate_v2_successor_height_at omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "fn publish_recovered_v2_successor_height_at(",
            "published_status_height_before: published.as_ref().map_or(0, |status| status.height),",
            "published_status_height_before: 0,",
            "publish_recovered_v2_successor_height_at omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "fn publish_recovered_v2_successor_height_at(",
            "if let Some(published) = published {\n"
            "        return Err(V2SuccessorActivationError::RecoveredStatusAlreadyPublished(\n"
            "            published.height,\n"
            "        ));\n"
            "    }\n"
            "    set_v2_status_at(successor, now);",
            "set_v2_status_at(successor, now);",
            "publish_recovered_v2_successor_height_at must contain 'if let Some(published) = published' exactly 2 time(s)",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "pub(crate) fn begin_v2_successor_activation(",
            "stage_before: successor_stage_projection(status.liveness.work.successor_height),",
            "stage_before: SUCCESSOR_STAGE_QUEUED,",
            "begin_v2_successor_activation omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/status.rs",
            "pub(crate) fn mark_v2_restart_required()",
            "if !production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(lifecycle) {",
            "if !production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(lifecycle) { return;",
            "mark_v2_restart_required must contain 'return;' exactly 1 time(s)",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn recovered(authority: RecoveredSuccessorActivationAuthority)",
            "let published_height = super::status::v2_status().map_or(0, |status| status.height);",
            "let published_height = 0;",
            "PendingSuccessorActivation omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn bind(\n        self,",
            "authority_predecessor: authority.predecessor().refinement_projection(),",
            "authority_predecessor: self.predecessor.refinement_projection(),",
            "PendingSuccessorConstruction omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_recovery.rs",
            "pub(crate) fn authenticate(\n        artifact: &wire::finality::V2FinalityArtifact,",
            "|| receipt.certificate() != artifact.commit_qc.as_ref()",
            "|| false",
            "DurableV2PredecessorIdentity::authenticate omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_recovery.rs",
            "pub(crate) fn authenticate(\n        artifact: &wire::finality::V2FinalityArtifact,",
            "if !production_durable_predecessor_identity_kernel(identity.refinement_projection()) {",
            "if false {",
            "DurableV2PredecessorIdentity::authenticate omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_recovery.rs",
            "fn new(record: &wire::SnapshotV2BootstrapRecord) -> Self",
            "record_hash: HashOf::new(record),",
            "record_hash: HashOf::new(&wire::SnapshotV2BootstrapRecord::default()),",
            "SnapshotSuccessorActivationAuthority::new omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_recovery.rs",
            "pub(crate) fn recover_active_height(",
            "if record.context() != &bootstrap.context\n"
            "            || record.proofs_of_possession() != bootstrap.validator_set_pops",
            "if false",
            "recover_active_height snapshot authority omits production refinement tokens",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_recovery.rs",
            "pub(crate) fn recover_active_height(",
            "v2_finality_artifact_with_receipt(durable_height)",
            "v2_finality_artifact(durable_height)",
            "recover_active_height complete-tip authority omits production refinement tokens",
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
        "crates/iroha_core/src/sumeragi/v2_recovery.rs",
        "scripts/run_sumeragi_v2_release_gates.sh",
    )
    for source_name in required_sources:
        destination = tmp_path / source_name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / source_name, destination)

    assert module._successor_production_source_fidelity_errors(tmp_path) == []

    path = tmp_path / relative_path
    source = path.read_text(encoding="utf-8")
    region_start = source.find(region_marker)
    assert region_start >= 0
    mutation = source.find(old, region_start)
    assert mutation >= 0
    next_item = re.search(
        r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?(?:async[ \t]+)?fn[ \t]+",
        source[region_start + len(region_marker) :],
    )
    if next_item is not None:
        next_item_start = (
            region_start + len(region_marker) + next_item.start()
        )
        assert mutation < next_item_start, (
            "mutation escaped the production Rust item selected by its region marker",
            relative_path,
            region_marker,
            old,
        )
    path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )

    errors = module._successor_production_source_fidelity_errors(tmp_path)
    assert any(error_fragment in error for error in errors), errors


def test_locked_body_reproposal_source_fidelity_rejects_formal_and_production_mutants(
    tmp_path: Path,
) -> None:
    """The justified-high obligation must remain connected through admission."""

    module = load_checker()
    assert module._locked_body_reproposal_source_fidelity_errors(
        module.FORMAL_DIR, ROOT_DIR
    ) == []

    formal_names = (
        "SumeragiV2Core.tla",
        "SumeragiV2InductiveProofs.tla",
    )
    production_names = (
        "crates/iroha_core/src/sumeragi/v2_core/wal.rs",
        "crates/iroha_core/src/sumeragi/v2.rs",
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "crates/iroha_core/src/sumeragi/v2_candidate.rs",
    )

    def copy_fixture(case: str) -> tuple[Path, Path]:
        repo_root = tmp_path / case
        formal_dir = repo_root / "docs" / "formal" / "sumeragi_v2"
        formal_dir.mkdir(parents=True)
        for name in formal_names:
            shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)
        for name in production_names:
            destination = repo_root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT_DIR / name, destination)
        return repo_root, formal_dir

    formal_mutants = (
        (
            "vacuous_guard",
            "SumeragiV2Core.tla",
            "operator",
            "LocalProposalReproposesJustifiedHigh",
            "\\/ proposal.subject = proposal.justifySubject",
            "\\/ TRUE",
            "exact justified-high subject contract",
        ),
        (
            "disconnected_guard",
            "SumeragiV2Core.tla",
            "operator",
            "BeginLocalProposal",
            "LocalProposalReproposesJustifiedHigh(proposal)",
            "TRUE",
            "must enforce the exact justified-high subject guard",
        ),
        (
            "tautological_theorem",
            "SumeragiV2InductiveProofs.tla",
            "theorem",
            "BeginLocalProposalReproposesExactJustifiedHigh",
            "IN \\/ justification.rank = NoRank\n"
            "            \\/ subject = justification.subject",
            "IN TRUE",
            "exact nontrivial justified-high subject postcondition",
        ),
    )
    for (
        case,
        filename,
        declaration_kind,
        symbol,
        old,
        new,
        error_fragment,
    ) in formal_mutants:
        repo_root, formal_dir = copy_fixture(case)
        path = formal_dir / filename
        source = path.read_text(encoding="utf-8")
        if declaration_kind == "operator":
            mutated = mutate_tla_operator(source, symbol, old, new)
        else:
            mutated = mutate_tla_theorem(source, symbol, old, new)
        path.write_text(mutated, encoding="utf-8")
        errors = module._locked_body_reproposal_source_fidelity_errors(
            formal_dir, repo_root
        )
        assert any(error_fragment in error for error in errors), errors

    rust_mutants = (
        (
            "wal_high_subject",
            "crates/iroha_core/src/sumeragi/v2_core/wal.rs",
            "apply_in_place",
            "highest.subject() == proposal.manifest().subject()",
            "highest.subject() != proposal.manifest().subject()",
            "Timeout justification high certificate",
        ),
        (
            "wal_lock_promotion",
            "crates/iroha_core/src/sumeragi/v2_core/wal.rs",
            "apply_in_place",
            "None => self.locked = Some(highest.clone()),",
            "None => {},",
            "InstallTimeout must promote",
        ),
        (
            "directive_projection",
            "crates/iroha_core/src/sumeragi/v2.rs",
            "local_proposal_directive",
            """let locked_subject = locked
            .map(|certificate| self.registry.subject(certificate.subject()))
            .transpose()?;""",
            "let locked_subject = None;",
            "runner directive must project",
        ),
        (
            "runner_fresh_fallback",
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "schedule_local_proposal",
            "if directive.locked_subject().is_some() {",
            "if directive.locked_subject().is_none() {",
            "locked directive must never fall through",
        ),
        (
            "runner_subject_mismatch",
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "submit_exact_body",
            ".is_some_and(|locked| locked != subject)",
            ".is_some_and(|locked| locked == subject)",
            "exact-body submission must reject",
        ),
        (
            "runner_disconnected_admission",
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "submit_encoded_body",
            "executor.admit_local_proposal(owner.tag, manifest, canonical_wire, services)?;",
            "drop((owner.tag, manifest, canonical_wire, services));",
            "encoded proposal admission must preserve",
        ),
        (
            "candidate_accepts_lock",
            "crates/iroha_core/src/sumeragi/v2_candidate.rs",
            "validate_request",
            "if request.directive.locked_subject().is_some() {",
            "if request.directive.locked_subject().is_none() {",
            "fresh candidate construction must reject",
        ),
    )
    for case, relative, item, old, new, error_fragment in rust_mutants:
        repo_root, formal_dir = copy_fixture(case)
        mutate_rust_item_source(module, repo_root / relative, item, old, new)
        errors = module._locked_body_reproposal_source_fidelity_errors(
            formal_dir, repo_root
        )
        assert any(error_fragment in error for error in errors), errors


def test_exact_output_production_source_is_bound() -> None:
    module = load_checker()
    assert module._exact_output_production_source_fidelity_errors(ROOT_DIR) == []


@pytest.mark.parametrize(
    ("relative_path", "region_marker", "old", "new", "error_fragment"),
    (
        (
            "crates/iroha_core/src/lib.rs",
            "pub enum NetworkMessage",
            "CertifiedMergeSidecar(Arc<CertifiedMergeSidecarMessage>),",
            "CertifiedMergeSidecar(Box<CertifiedMergeSidecarMessage>),",
            "every exact-output network payload class must use an immutable shared carrier",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct OutboundTransfer",
            "chunks: Vec<Arc<CertifiedMergeSidecarMessage>>",
            "chunks: Vec<CertifiedMergeSidecarMessage>",
            "sidecar responses must cache each immutable fixed-boundary payload once for every source cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn drain_outbound_chunks(",
            "let message = Arc::clone(\n"
            "                            transfer\n"
            "                                .chunks\n"
            "                                .get(index)\n"
            "                                .expect(\"bounded sidecar cursor names a cached chunk\"),\n"
            "                        );",
            "let message = Arc::new(\n"
            "                            transfer\n"
            "                                .chunks\n"
            "                                .get(index)\n"
            "                                .expect(\"bounded sidecar cursor names a cached chunk\")\n"
            "                                .as_ref()\n"
            "                                .clone(),\n"
            "                        );",
            "sidecar drainage must clone only the cached Arc",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "if !admission.matches_materialized_chunk(expected_chunk) {",
            "let _rebuilt_payload = expected_chunk.bytes.to_vec();\n"
            "        if !admission.matches_materialized_chunk(expected_chunk) {",
            "per-source sidecar drainage and acknowledgement must never reconstruct cached payload bytes",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) enum V2LaneWorkEffect",
            "message: Arc<CertifiedMergeSidecarMessage>,",
            "message: CertifiedMergeSidecarMessage,",
            "the lane effect must preserve the exact immutable sidecar carrier",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn post_certified_merge_sidecar_with_reply_routes(",
            "let data = NetworkMessage::CertifiedMergeSidecar(message);",
            "let data = NetworkMessage::CertifiedMergeSidecar(Arc::new((*message).clone()));",
            "worker sidecar dispatch must install the existing Arc without reconstruction",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effect(",
            "Arc::clone(&message),",
            "Arc::new((*message).clone()),",
            "runner sidecar dispatch must preserve the exact peer, complete route set, and immutable message pointer",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "const MAX_OUTBOUND_SESSIONS_PER_SOURCE",
            "const MAX_OUTBOUND_SESSIONS_PER_SOURCE: usize = 2;",
            "const MAX_OUTBOUND_SESSIONS_PER_SOURCE: usize = 3;",
            "certified sidecar authenticated-source limits must remain exactly four gates, two sessions, and 16 MiB",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "const MAX_OUTBOUND_BYTES_PER_SOURCE",
            "const MAX_OUTBOUND_BYTES_PER_SOURCE: usize = 16 * 1024 * 1024;",
            "const MAX_OUTBOUND_BYTES_PER_SOURCE: usize = 17 * 1024 * 1024;",
            "certified sidecar authenticated-source limits must remain exactly four gates, two sessions, and 16 MiB",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "const MAX_SERVER_REQUEST_GATES_PER_SOURCE",
            "const MAX_SERVER_REQUEST_GATES_PER_SOURCE: usize = 4;",
            "const MAX_SERVER_REQUEST_GATES_PER_SOURCE: usize = 5;",
            "certified sidecar authenticated-source limits must remain exactly four gates, two sessions, and 16 MiB",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn from_admitted_reply(",
            "semantic_target: flush_identity.semantic_target().clone(),",
            "semantic_target: chunk.responder.clone(),",
            "sidecar writer-flush admission must bind the opaque source, exact route, actor ticket and clone-shared claim with immutable payload and cursors",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn poll_sidecar_flushes(",
            ".bind_confirmed_worker_trace(flush_trace)",
            ".bind_confirmed_worker_trace(ProductionReliableFlushTraceProjection::default())",
            "a successful writer occurrence must bind its exact confirmed worker trace before lane admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn poll_sidecar_flushes(",
            "NetworkReplyFlushAckStatus::Flushed => {\n"
            "                    self.admitted_sidecar_chunks.push_back(completion.admission);\n"
            "                }",
            "NetworkReplyFlushAckStatus::Flushed => {}",
            "only a successful peer-writer flush may create a sidecar cursor receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn poll_sidecar_flushes(",
            "NetworkReplyFlushAckStatus::Closed => {\n"
            "                    // The sidecar transport still owns this unacknowledged",
            "NetworkReplyFlushAckStatus::Closed => {\n"
            "                    self.admitted_sidecar_chunks.push_back(completion.admission.clone());\n"
            "                    // The sidecar transport still owns this unacknowledged",
            "closed writer ownership must not manufacture a sidecar cursor receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn complete_sidecar_targets_with_retained_flush_ownership(",
            "admission.matches_materialized_chunk(chunk)\n"
            "                        && admission.is_bound_to_attempt(route)",
            "admission.matches_materialized_chunk(chunk)",
            "retained sidecar flush completion must match the immutable chunk and exact authenticated-source tenure",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "CertifiedMergeSidecarMessage::Request(_) => None,",
            "CertifiedMergeSidecarMessage::Request(_) => {\n"
            "                        panic!(\"request gained a flush receipt\")\n"
            "                    },",
            "only an immutable certified response chunk may retain the exact route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "self.sidecar_control_units() >= self.sidecar_admission_capacity",
            "self.sidecar_control_units() > self.sidecar_admission_capacity",
            "sidecar receipt capacity must reject at the exact full boundary",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn admit_network_exact_output(",
            ".post_reply_recoverable_with_flush_ack(\n"
            "                    post,\n"
            "                    reply_route,\n"
            "                    ticket,\n"
            "                )?",
            ".post_reply_recoverable(post, reply_route, ticket)\n"
            "                .map(|()| None)?",
            "production sidecar output must retain the exact writer-flush witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn admit_network_exact_output(",
            "Ok(ExactOutputAttemptOutcome::SidecarFlush(flush_ack))",
            "{ drop(flush_ack); Ok(ExactOutputAttemptOutcome::Admitted) }",
            "production sidecar output must retain the exact writer-flush witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn drain_certified_merge_sidecar_chunk_admissions(",
            "limit.min(pending.admitted_sidecar_chunks.len())",
            "limit.min(pending.flushing_sidecar_chunks.len())",
            "receipt drainage may consume only successfully flushed sidecar admissions",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "|| !production_reliable_flush_two_phase_link_kernel(worker_trace, occurrence)",
            "|| false",
            "lane application must reject any projection without the exact accepted worker occurrence before inspecting mutable transport state",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "if !admission.projection_matches_identity(&admission.flush_identity) {",
            "if false {",
            "lane application must reject any projection without the exact accepted worker occurrence before inspecting mutable transport state",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "|| projection.chunk_cursor_before != chunk_index",
            "|| false",
            "lane application must validate the immutable message and chunk cursors before transport preflight",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "preflight_reliable_flush_outbound(self, admission, &gate, chunk_index, count)?",
            "preflight_reliable_flush_outbound(self, admission, &gate, 0, count)?",
            "the exact gate, source route, shared bytes and cursors must preflight into one immutable application plan before claiming completion",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "struct ServerRequestGateAttempt {",
            "cursor: ServerResponseCursor,",
            "cursor: usize,",
            "sidecar request gates must retain exact materialization authority, retry state, and a source-local pending-or-terminal cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "enum ServerResponseCursor {",
            "Complete,",
            "PendingZero,",
            "sidecar source cursors must distinguish pending chunk zero from terminal completion",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn prune_server_gates(",
            "|| attempt.cursor != ServerResponseCursor::Complete",
            "|| false",
            "sidecar gate pruning must retain every incomplete source cursor as a bounded reservation while expiring only terminal no-outbound tombstones",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn route_update(",
            ".source_update_from(prior)",
            ".source_update_from(candidate)",
            "same-source sidecar route update must use the canonical monotonic update kernel",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "if prior.cursor == ServerResponseCursor::Complete {",
            "if false && prior.cursor == ServerResponseCursor::Complete {",
            "an exact, later-delivery, or reconnected completed source must remain terminal while only its observed route may update",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            "let ServerResponseCursor::Pending(resume_chunk) = attempt.cursor else {\n"
            "                continue;\n"
            "            };",
            "let resume_chunk = match attempt.cursor {\n"
            "                ServerResponseCursor::Pending(chunk) => chunk,\n"
            "                ServerResponseCursor::Complete => 0,\n"
            "            };",
            "completed sidecar sources must never regain materialized output",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn drain_outbound_chunks(",
            "cursor = ServerResponseCursor::Complete;",
            "cursor = ServerResponseCursor::Pending(0);",
            "sidecar drainage must persist terminal completion rather than a replayable chunk-zero cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "if !admission.flush_identity.claim_writer_flush_once() {",
            "if false {",
            "the clone-shared writer claim must be the sole linearization point before application-kernel and exact-link postchecks",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "if !production_reliable_flush_two_phase_link_kernel(worker_trace, application) {",
            "if false {",
            "the clone-shared writer claim must be the sole linearization point before application-kernel and exact-link postchecks",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "attempt.materialization_retryable = false;\n"
            "                    return Ok(false);",
            "attempt.materialization_retryable = true;\n"
            "                    return Ok(false);",
            "an exact, later-delivery, or reconnected completed source must remain terminal while only its observed route may update",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "let retry_chunk = attempt.in_flight_chunk.unwrap_or(attempt.next_chunk);",
            "let retry_chunk = 0;",
            "a replacement writer tenure must retry the source's current chunk",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "if attempt.in_flight_chunk.is_none() && !attempt.queued {",
            "if !attempt.queued {",
            "a later delivery with an in-flight chunk must refresh only its source route without queueing a concurrent copy",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "attempt.cursor = ServerResponseCursor::Pending(retry_chunk);",
            "attempt.cursor = ServerResponseCursor::Pending(0);",
            "an observed source update must never reset a retained sidecar cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            "candidate.same_delivery(admitted)",
            "candidate.same_tenure(admitted)",
            "materialization must consume the exact admitted delivery capability",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            "let mut remaining_global_sessions = self\n"
            "            .outbound_session_capacity\n"
            "            .saturating_sub(self.outbound_attempt_count());",
            "let mut remaining_global_sessions = usize::MAX;",
            "sidecar materialization must preflight global and per-source session and byte bounds",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            "self.source_outbound_count(source) >= MAX_OUTBOUND_SESSIONS_PER_SOURCE",
            "self.source_outbound_count(source) > MAX_OUTBOUND_SESSIONS_PER_SOURCE",
            "sidecar materialization must preflight global and per-source session and byte bounds",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn enqueue_response(",
            ".saturating_add(bytes.len())\n"
            "                    > MAX_OUTBOUND_BYTES_PER_SOURCE",
            ".saturating_add(bytes.len())\n                    > usize::MAX",
            "sidecar materialization must preflight global and per-source session and byte bounds",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if admitted_attempts.is_empty()",
            "> self.outbound_byte_capacity",
            "> usize::MAX",
            "sidecar materialization must preflight global and per-source session and byte bounds",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if remaining_global_sessions == 0",
            "capacity_rejected_attempts.push(source.clone());",
            "return Err(MergeSidecarError::Capacity(\"outbound response budget\"));",
            "one saturated sidecar source must not erase independently admissible same-request sources",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if !Self::alternate_source_is_authorized",
            "next_chunk: 0,",
            "next_chunk: prior.resume_chunk,",
            "a newly observed alternate sidecar source must begin at chunk zero",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn drain_outbound_chunks(",
            "attempt.in_flight_chunk = Some(index);",
            "attempt.next_chunk = index.saturating_add(1);",
            "sidecar drainage must preserve the exact source route and mark only an in-flight cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "if existing.request_hash != request_hash {",
            "if false && existing.request_hash != request_hash {",
            "duplicate sidecar admission must preserve canonical request identity",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn alternate_source_is_authorized(",
            "match candidate {",
            "return true; match candidate {",
            "alternate sidecar sources must share canonical request authority",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "if &request.requester != sender || &request.responder != local_peer {",
            "if false && (&request.requester != sender || &request.responder != local_peer) {",
            "sidecar request admission must bind authenticated sender, responder, semantic target, and active route",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn cancel_unmaterialized_server_request(",
            "attempt.materialization_authorized = false;\n"
            "                attempt.authorized_materialization_route = None;\n"
            "                attempt.materialization_retryable = true;",
            "let _ = attempt;",
            "failed sidecar materialization must preserve route/cursor history",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "if !prior.materialization_retryable {",
            "if false && !prior.materialization_retryable {",
            "an exact failed-materialization retry must consume only its source-local retry authorization",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn release_unsent_request(",
            "let attempt = assembly\n"
            "            .current\n"
            "            .take()",
            "return; let attempt = assembly\n            .current\n            .take()",
            "an unsent sidecar request must restore the exact holder cursor and retry rank",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn acknowledge_certified_merge_sidecar_chunk_admission(",
            "if acknowledged {",
            "if true {",
            "lane work may schedule the next chunk only after the exact receipt advances",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn acknowledge_certified_merge_sidecar_chunk_admission(",
            "operation.complete();",
            "drop(operation);",
            "lane sidecar ACK application may complete only after every successor post is retained",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn effect_count(",
            "self.effects\n"
            "            .len()\n"
            "            .saturating_add(self.sidecar_effects.len())",
            "0",
            "lane scan rank must count both ordinary and source-owned sidecar effects",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn requeue_effect(",
            "match effect {",
            "drop(effect); return true; match effect {",
            "lane requeue must return the exact unserviceable occurrence to its bounded owner lane",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_merge_sidecar_post(",
            "peer: post.peer,",
            "peer: self.local_peer.clone(),",
            "lane sidecar post conversion must preserve the exact peer, bounded route authority, and message",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn prune_finalized_merge_sidecars(",
            ".map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;",
            ".ok();",
            "finalized sidecar pruning must remain fail-stop and Kura-bound",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn retain_active_owned_reply_routes(",
            "routes.retain_active() != 0",
            "routes.iter().any(iroha_p2p::network::NetworkReplyRoute::is_active)",
            "runner pruning must retain every live source attempt and its tombstones",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn is_pending(",
            ".any(PendingExactFanout::has_dispatchable_target)",
            ".all(PendingExactFanout::has_dispatchable_target)",
            "pending exact output must include dispatchable fanouts, writer flushes, and undrained receipts without spinning on parked ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn handoff_applied_height_to_durable_reconstruction(",
            "self.flushing_sidecar_chunks.clear();",
            "let _ = &self.flushing_sidecar_chunks;",
            "applied-height handoff must retire every volatile sidecar completion state",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn retain_returned(",
            "if HashOf::new(&post.data) != *expected_hash {",
            "if false && HashOf::new(&post.data) != *expected_hash {",
            "returned actor post must retain the exact pinned payload identity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn handoff_applied_height_to_durable_reconstruction(",
            ".any(|(message, expected_hash)| HashOf::new(message) != *expected_hash)",
            ".any(|(message, expected_hash)| false && HashOf::new(message) != *expected_hash)",
            "applied-height handoff must preflight every pinned payload before classification",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn drive_with_budget_ack<Attempt>(",
            "blocked_sources.insert(attempted_source);",
            "let _ = attempted_source;",
            "exact-output drive_with_budget_ack declaration and complete control flow",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn apply_reply_route_update(",
            "self.current = None;",
            "self.message_index = 0; self.current = None;",
            "a same-source reconnect must not reset its retained exact-output cursor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn coalesce_reservation_additions_for_plan(",
            "ReplyTargetMerge::Update { .. } => 0,",
            "ReplyTargetMerge::Update { .. } => full_mask,",
            "ordinary same-source updates retain reservation ownership while closed-writer reactivation and a new source charge exactly the candidate cursor suffix",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn preview_coalesce_plan(",
            "target.2 = false;",
            "target.1 = 0; target.2 = false;",
            "the coalesce preview must preserve the retained message cursor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn outstanding_sources_excluding(",
            "for (target_index, target) in self.targets.iter().enumerate() {",
            "for (target_index, target) in self.targets.iter().enumerate().filter(|(_, target)| !target.parked) {",
            "parked attempts must retain every outstanding source/FIFO class",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn outstanding_reservation_counts(",
            "for (target_index, target) in self.targets.iter().enumerate() {",
            "for (target_index, target) in self.targets.iter().enumerate().filter(|(_, target)| !target.parked) {",
            "parked attempts must retain their reservation ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan(&self, candidate: &Self)",
            "self.reply_target_merge_plan_with_hooks(candidate, |_| {}, || {})",
            "self.reply_target_merge_plan_after_candidate_prune(candidate, |_| {})",
            "the no-hook production coalescing wrapper must delegate to the receipt-bound route-history kernel",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            ".project_retained_reply_routes(prune_receipt)",
            ".project_retained_reply_routes(prune_receipt.clone())",
            "candidate pruning must remain bound to its ownership receipt and strict route merge must return an opaque exact-history receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            ".merge_with_receipt(&candidate_routes)",
            ".merge(&candidate_routes)",
            "candidate pruning must remain bound to its ownership receipt and strict route merge must return an opaque exact-history receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            ".any(|route| route.same_delivery(candidate_route))",
            ".any(|route| route.same_source(candidate_route))",
            "the authoritative merged route snapshot must select the exact delivery before immutable same-delivery and same-tenure classification",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "                    update,\n                });",
            "                    update: NetworkReplyRouteSourceUpdate::Exact,\n                });",
            "same-source coalescing must reject terminal-candidate cursor regression and restrict reactivation to a reconnected certified-sidecar chunk",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn commit_coalesce_plan(",
            "message_index: candidate_target.message_index,\n"
            "                        current: None,",
            "message_index: 0,\n                        current: None,",
            "an appended source must preserve its candidate cursor and parked state while starting without actor-post or admission-ticket ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn capacity_available_for(",
            "pending.coalesce_reservation_additions_for_plan(fanout, &plan.targets)?",
            "fanout.admission_reservation_counts()?",
            "capacity preflight must enforce route-source geometry before charging only newly appended source ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn coalesced_target_geometry_available(",
            "&& target_count <= plan.reply_routes.source_capacity()",
            "&& true",
            "coalesced reply attempts must fit both the configured fanout bound and the actor-derived source-capacity geometry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "candidate_route.same_tenure(prior_route)",
            "candidate_route.same_delivery(prior_route)",
            "the authoritative merged route snapshot must select the exact delivery before immutable same-delivery and same-tenure classification",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn classified_with_reply_routes(",
            "Self::classified_with_route_history(messages, peers, routes, Some(reply_routes))",
            "Self::classified_with_route_history(messages, peers, routes, None)",
            "reply fanout construction must preserve the complete bounded route history",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn classified_with_route_history(",
            "            reply_routes,\n"
            "            ingress_ownership: None,\n"
            "            current_source_targets: BTreeMap::new(),",
            "            reply_routes: None,\n"
            "            ingress_ownership: None,\n"
            "            current_source_targets: BTreeMap::new(),",
            "fanout construction must store the complete authoritative live-and-tombstone reply history",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "let retained_routes = self.reply_routes.clone().ok_or_else",
            "let retained_routes = candidate.reply_routes.clone().ok_or_else",
            "candidate pruning must remain bound to its ownership receipt and strict route merge must return an opaque exact-history receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "&& candidate.is_certified_sidecar_chunk_fanout()",
            "&& true",
            "same-source coalescing must reject terminal-candidate cursor regression and restrict reactivation to a reconnected certified-sidecar chunk",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "if plan.targets.is_empty()",
            ".commit_coalesce_plan(&fanout, &plan, preview.current_source_targets);",
            ".coalesce_retry(&fanout)?;",
            "a route-history-only update must atomically commit its previewed cursor and FIFO ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "if plan.targets.is_empty()",
            "self.source_fifo_owners = next_source_fifo_owners;",
            "let _ = next_source_fifo_owners;",
            "a route-history-only update must atomically commit its previewed cursor and FIFO ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn commit_coalesce_plan(",
            "self.reply_routes = Some(plan.reply_routes.clone());",
            "self.reply_routes = None;",
            "atomic coalesce commit must install the complete route and fair-ingress histories",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn exact_target_geometry(",
            "Some(reply_routes.clone()),",
            "None,",
            "lane preflight expands every authenticated source into an independent exact target",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn advance_after_attempt(",
            "self.unregister_source_fifo_owner(fifo_id, source)?;",
            "let _ = (fifo_id, source);",
            "admission advances only the completed class/source ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn start(",
            ".map(|entry| entry.validator.clone())",
            ".filter(|_| false).map(|entry| entry.validator.clone())",
            "production derives route fanout through the shared checked configuration kernel",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn enqueue_owned_exact_reply_routes_while_guarded(",
            "PendingExactFanout::claimed_with_reply_routes_and_ingress_ownership(",
            "PendingExactFanout::claimed_with_routes(",
            "exact replies expand all authenticated sources without changing semantic identity and preserve bounded route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn applied_height_reconstruction_covers(",
            "rollover_claim.validate_fanout(messages, peers)?;",
            "let _ = (messages, peers);",
            "durable rollover requires a validated typed claim in the exact creation scope",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn applied_height_reconstruction_covers(",
            "if !scope.covers(artifact) {",
            "if false && !scope.covers(artifact) {",
            "durable rollover requires a validated typed claim in the exact creation scope",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn enqueue_exact_fanout_while_guarded(",
            "PendingExactFanout::claimed(messages, peers, rollover_claim)?",
            "PendingExactFanout::claimed(messages, peers, ExactOutputRolloverClaim::Exact)?",
            "every production exact fanout must enter the corridor with its typed claim",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn applied_height_reconstruction_covers(",
            "durable_history.ok_or_else(|| {",
            "Some(durable_history.unwrap()).ok_or_else(|| {",
            "applied-height handoff must independently reread every durable Kura response source",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn durable_history_source_covers(",
            "|| response.certificate != source.commit_qc",
            "|| false",
            "durable CommitQC response must match its exact Kura finality source",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn durable_history_source_covers(",
            "|| canonical_wire != response.body",
            "|| false",
            "durable body response must match its exact canonical Kura block",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn durable_history_source_covers(",
            "|| certificate.commit_qc != source.commit_qc",
            "|| false",
            "durable lane certificate must match its exact certified Kura source",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn post_durable_history_response_with_routes(",
            "durable_history_source_covers(",
            "durable_history_source_covers_unchecked(",
            "global historical response must validate Kura before exact-output admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn post_durable_lane_certificate_with_routes(",
            "durable_history_source_covers(",
            "durable_history_source_covers_unchecked(",
            "historical lane response must validate Kura before exact-output admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn handoff_applied_height_output_to_durable_reconstruction(",
            "Some(self.kura.as_ref()),",
            "None,",
            "production handoff must pass exact lane and Kura authorities into retirement",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn applied_height_reconstruction_covers(",
            "round.context_id == context_id && round.height == height",
            "round.context_id == context_id",
            "durable rollover classification must bind the exact artifact context and height",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn applied_height_reconstruction_covers(",
            "ProgressReconstruction::Retransmit",
            "ProgressReconstruction::Exact",
            "exact-output applied_height_reconstruction_covers declaration",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn handoff_applied_height_output_to_durable_reconstruction(",
            "|| receipt.artifact_hash() != HashOf::new(artifact)",
            "|| false",
            "applied-height handoff requires the exact Kura receipt and finality artifact",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn covered_source_hash(",
            "self.finality_artifact_hash != HashOf::new(finality_artifact)",
            "false && self.finality_artifact_hash != HashOf::new(finality_artifact)",
            "lane rollover authority must bind the exact finality artifact and height",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn covered_source_hash(",
            "self.durable_sessions.get(&proposal_hash)",
            "self.durable_sessions.values().next()",
            "winning lane output must use its proposal-keyed durable session witness",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn covered_source_hash(",
            "validate_superseded_lane_output(message)?;",
            "let _ = message;",
            "non-winning lane output must be validated before artifact-bound supersession",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn persistent(",
            "application_receipt_hash.as_ref(),",
            "durable_artifact_hash.as_ref(),",
            "lane durable source must commit finality, certificate, and application receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn durable_lane_rollover_authority(",
            "block.header().height().get() != finality_artifact.height\n"
            "            || block.hash() != finality_artifact.block_hash",
            "false",
            "lane authority builder must source its finalized body from the exact "
            "canonical Kura block without treating external-only bodies as lane plans",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn durable_lane_rollover_authority(",
            "|| application_receipt.application_block_hash != finality_artifact.block_hash",
            "|| false",
            "lane authority builder must bind every winner to the exact applied artifact",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn reconstruct_durable_lane_certificate(",
            "self.kura.read_certified_lane_block_artifact(",
            "self.kura.read_certified_lane_block_artifact_unchecked(",
            "lane recovery reconstruction must begin from the exact certified Kura artifact",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn serve_durable_lane_certificate(",
            "reply_routes: Some(reply_routes),",
            "reply_routes: None,",
            "lane recovery emitter retains every authenticated source route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn merge_optional_reply_routes(",
            "let mut merged = queued.clone();",
            "let mut merged = candidate.clone();",
            "lane effect coalescence atomically commits canonical history maintenance",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn merge_optional_reply_routes(",
            "merged.merge_observed(candidate)",
            "merged.merge(candidate)",
            "lane coalescence must use the canonical atomic observed-history reconciliation kernel",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn merge_lane_work_effect_reply_routes(",
            "if !lane_work_effect_reply_routes_have_valid_shape(candidate) {",
            "if !lane_work_effect_reply_routes_are_valid(candidate) {",
            "inactive duplicates still reach maintenance",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn lane_work_effect_key(",
            "encoded.push(4);",
            "encoded.push(0);",
            "durable lane response effect identity must include its distinct tag, peer, and certificate",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn drain_v2_ingress(",
            "services.post_durable_history_response_on_reply_routes_with_permit(",
            "services.post_durable_history_response_with_permit(",
            "historical global responses preserve the complete prevalidated route set",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effect(",
            ".post_durable_lane_certificate_on_reply_routes(\n"
            "                    peer,\n"
            "                    reply_routes,\n"
            "                    ingress_ownership,\n"
            "                    certificate,\n"
            "                )",
            ".post_lane_block(peer, BlockMessage::LaneBlockCertificate(Box::new(certificate)))",
            "historical lane dispatch preserves every authenticated source route",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effects(",
            "let scan_limit = lane_work.effect_count();",
            "let scan_limit = limit.max(1);",
            "lane scheduler must scan past unserviceable heads without losing ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effects(",
            "continue;",
            "break;",
            "lane scheduler must scan past unserviceable heads without losing ownership",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn dispatch_lane_work_effects(",
            "apply_certified_merge_sidecar_chunk_admissions(lane_work, services, limit)?;",
            "let _ = (lane_work, services, limit);",
            "runner lane dispatch must apply writer receipts before selecting owned work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "dispatch_lane_work_effect(services, next_effect)?;",
            "apply_certified_merge_sidecar_chunk_admissions(lane_work, services, limit)?;",
            "let _ = (lane_work, services, limit);",
            "runner lane dispatch must apply writer receipts after every exact handoff",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "lane_work.prune_finalized_merge_sidecars()?;",
            "let _ = retry_exact_output_and_apply_sidecar_admissions(\n"
            "                    &mut lane_work,\n"
            "                    &services,\n"
            "                    control_queue_capacity,\n"
            "                )?;",
            "let _ = services.retry_pending_exact_output();",
            "durable finalization must perform receipt-aware retry, dispatch, and exact handoff before successor activation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn persist_anchored_sessions(",
            "self.hydrate_canonical_lane_artifacts();",
            "let _ = &self.lane_sessions;",
            "late canonical lane hydration must precede committed-session collection",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn hydrate_canonical_lane_artifacts(",
            "let _ = self\n"
            "                .lane_sessions\n"
            "                .insert_recovered_proposal_replacing_uncommitted_conflict(proposal);",
            "let _ = proposal;",
            "late canonical lane hydration must retain the exact proposal as bounded recovery work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "if executor.ready_to_finish() {",
            "lane_work.persist_anchored_sessions()?;",
            "let _ = &lane_work;",
            "durable finality must retire the old exact-output corridor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "if executor.ready_to_finish() {",
            "lane_work.durable_lane_rollover_authority(&durable_artifact)?;",
            "unreachable!(\"skip durable lane authority\");",
            "durable finality must retire the old exact-output corridor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "if executor.ready_to_finish() {",
            "&durable_lane_authority,",
            "&durable_lane_authority.clone(),",
            "durable finality must retire the old exact-output corridor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn with_reply_source_capacity(",
            "reply_source_capacity,\n            outbound_session_capacity,",
            "reply_source_capacity,\n            outbound_session_capacity: 0,",
            "sidecar source geometry must reject zero and install every checked corridor bound",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn park_authorized_server_request_attempts(",
            "for attempt in gate\n            .attempts",
            "return; for attempt in gate\n            .attempts",
            "parking a materialized response must consume retryability while retaining each source route and resume cursor",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "if admitted_attempts.is_empty() && capacity_rejected_attempts.is_empty()",
            "Self::park_authorized_server_request_attempts(gate, now);",
            "let _ = (gate, now);",
            "every rejected, capacity-partitioned, or materialized response must park source history",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "fn retire_inactive_outbound_attempts(",
            "gate_attempt.cursor = ServerResponseCursor::Pending(resume_chunk);",
            "let _ = resume_chunk;",
            "merge-sidecar source-isolated production seam",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn admit_server_request(",
            "next_chunk: resume_chunk,",
            "next_chunk: 0,",
            "merge-sidecar source-isolated production seam",
        ),
        (
            "crates/iroha_core/src/merge_sidecar.rs",
            "pub(crate) fn acknowledge_outbound_chunk(",
            "attempt.queued = true;\n                self.outbound_order.push_back((key, source));",
            "let _ = &attempt.queued;",
            "merge-sidecar source-isolated production seam",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "pub(crate) fn next_effect(",
            "if take_sidecar {",
            "if !take_sidecar {",
            "lane effect peek must clone the exact fairly selected queue head",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn push_effect(",
            "if !lane_work_effect_reply_routes_have_valid_shape(&effect) {",
            "if !lane_work_effect_reply_routes_are_valid(&effect) {",
            "maintenance-only duplicate lane effects must reach canonical reconciliation before live-delivery admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn apply_bounded_sidecar_admissions<T, Error>(",
            "let mut applied = 0usize;",
            "return Ok(0); let mut applied = 0usize;",
            "runner exact-output ownership/ACK production seam",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn has_pending_exact_output(",
            "self.lock_pending_exact_output()",
            "return Ok(false); self.lock_pending_exact_output()",
            "worker exact-output ownership/ACK production seam",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "after_candidate_prune(merge_attempt);",
            "let _ = merge_attempt;",
            "candidate pruning must remain bound to its ownership receipt and strict route merge must return an opaque exact-history receipt",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "if candidate_routes.len() >= live_before_merge {",
            "if false && candidate_routes.len() >= live_before_merge {",
            "candidate pruning must remain bound to its ownership receipt and strict route merge must return an opaque exact-history receipt",
        ),
        (
            "crates/iroha_config/src/parameters/defaults.rs",
            "pub const V2_EXACT_OUTPUT_CLASS_COUNT: usize = 3;",
            "pub const V2_EXACT_OUTPUT_CLASS_COUNT: usize = 3;",
            "pub const V2_EXACT_OUTPUT_CLASS_COUNT: usize = 2;",
            "exact-output defaults must retain the reviewed completion divisor, reducer batch, and three-class geometry",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "pub const MAX_EFFECTS_PER_STEP: usize = 8;",
            "pub const MAX_EFFECTS_PER_STEP: usize = 8;",
            "pub const MAX_EFFECTS_PER_STEP: usize = 7;",
            "the dependency-free reducer refinement must retain the reviewed maximum effect batch",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_core.rs",
            "const _: [(); refinement::MAX_EFFECTS_PER_STEP]",
            "[(); iroha_config::parameters::defaults::sumeragi::V2_MAX_EFFECTS_PER_STEP]",
            "[(); iroha_config::parameters::defaults::sumeragi::V2_EXACT_OUTPUT_CLASS_COUNT]",
            "the production embedded reducer must bind its dependency-free batch to configured exact-output geometry",
        ),
        (
            "crates/iroha_config/src/parameters/actual.rs",
            "pub fn sumeragi_v2_exact_output_shared_ownership_capacity(",
            ".checked_add(certified_request_capacity)",
            ".saturating_add(certified_request_capacity)",
            "the shared exact-output owner must reserve both bounded producers and one complete reducer batch with checked arithmetic",
        ),
        (
            "crates/iroha_config/src/parameters/actual.rs",
            "pub fn validate_sumeragi_v2_exact_output_geometry(",
            ".checked_mul(defaults::sumeragi::V2_EXACT_OUTPUT_CLASS_COUNT)",
            ".saturating_mul(defaults::sumeragi::V2_EXACT_OUTPUT_CLASS_COUNT)",
            "the geometry kernel must reject zero, multiplication overflow, and any corridor smaller than source-count times exact classes",
        ),
        (
            "crates/iroha_config/src/parameters/user.rs",
            "pub fn parse(self) -> Result<actual::Root, ParseError> {",
            ".max_total_connections\n",
            ".max_connections_per_peer\n",
            "root configuration must derive the authenticated-source bound from network geometry and fail parsing",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn start(",
            "validate_shared_ownership_geometry(\n"
            "            shared_pending_ownership_unit_capacity,\n"
            "            reply_route_source_capacity,\n"
            "        )?;",
            "validate_shared_ownership_geometry(\n"
            "            shared_pending_ownership_unit_capacity,\n"
            "            max_peers_per_fanout,\n"
            "        )?;",
            "production bounds protocol fanout by roster and source geometry while charging the shared pool only for the independently reserved authenticated reply sources",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "pub(crate) fn matches_semantic_origin(",
            "self.validate_exact() && self.first.semantic_origin.as_ref() == origin",
            "self.validate_exact()",
            "semantic-origin validation must compare the independently retained canonical request origin",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "pub(crate) fn advance_reply_cursors(",
            "if message_cursor < attempt.message_cursor || chunk_cursor < attempt.chunk_cursor {",
            "if false && (message_cursor < attempt.message_cursor || chunk_cursor < attempt.chunk_cursor) {",
            "a source attempt may advance but never reset either exact-output cursor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(crate) fn accept_payload_chunk_with_ingress_ownership",
            "|| !ingress_ownership.matches_semantic_origin(Some(authenticated_sender))",
            "|| false",
            "payload chunk effect consumption must reject a changed envelope or semantic origin before mutation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "pub(crate) fn accept_certified_body_response_with_ingress_ownership",
            "|| !ingress_ownership.matches_semantic_origin(Some(authenticated_responder))",
            "|| false",
            "certified body response effect consumption must reject a changed envelope or semantic origin before mutation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn claimed_with_reply_routes_and_ingress_ownership(",
            "if !ownership.validate_exact() || !ownership.matches_reply_routes(Some(routes)) {",
            "if false {",
            "exact reply construction must attach only a validated fair-ingress carrier matching the complete per-source route set",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn serve_certified_request_on_routes(",
            "|| !ingress_ownership.matches_semantic_origin(Some(reply_routes.semantic_target()))",
            "|| false",
            "certified request service must bind canonical request, semantic origin, and every return source before queued local work",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn route_payload_chunk<R: EffectRuntime>(",
            "|| !ingress_ownership.matches_semantic_origin(Some(&sender))",
            "|| false",
            "payload chunk routing must bind canonical bytes and semantic sender before buffering or effect mutation",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn buffer_orphan_payload_chunk_inner(",
            "if !retained.merge_downstream(candidate) {",
            "drop(candidate); if false {",
            "orphan chunk duplicates must merge alternate source ownership without replacing canonical semantic identity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "pub(crate) fn replay_buffered_chunks<R: EffectRuntime>(",
            "buffered.ingress_ownership.ok_or_else(|| {",
            "None.ok_or_else(|| {",
            "orphan replay must preserve the exact ownership carrier into live chunk delivery",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
            "fn accept_lane_message_owned(",
            "|| !ownership.matches_semantic_origin(sender.as_ref())",
            "|| false",
            "lane ingress must bind semantic origin, canonical message, and the complete source route set before service",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn drain_v2_ingress(",
            "|| !ingress_ownership.matches_semantic_origin(inbound.sender())",
            "|| false",
            "runner ingress must retain canonical message, semantic origin, and source-isolated routes in one exact ownership carrier",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "fn v2_ingress_head_can_drain(",
            "executor.can_admit_network_message_with_ingress_ownership(message, ingress_ownership)",
            "executor.can_admit_network_message(message)",
            "runner preflight must preserve the exact fair-ingress carrier into owned runtime capacity admission",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "fn reply_target_merge_plan_with_hooks<AfterCandidatePrune, AfterRouteMerge>(",
            "retained.merge_downstream_with_strict_receipt(candidate, merge_receipt)",
            "retained.merge_downstream(candidate)",
            "retained fair-ingress ownership must consume the strict receipt and yield the sole authoritative route snapshot",
        ),
    ),
)
def test_exact_output_production_source_mutations_fail_closed(
    tmp_path: Path,
    relative_path: str,
    region_marker: str,
    old: str,
    new: str,
    error_fragment: str,
) -> None:
    module = load_checker()
    for source_name in (
        "crates/iroha_core/src/lib.rs",
        "crates/iroha_core/src/merge_sidecar.rs",
        "crates/iroha_core/src/sumeragi/v2_worker.rs",
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "crates/iroha_core/src/sumeragi/mod.rs",
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "crates/iroha_core/src/sumeragi/v2_core.rs",
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "crates/iroha_config/src/parameters/defaults.rs",
        "crates/iroha_config/src/parameters/actual.rs",
        "crates/iroha_config/src/parameters/user.rs",
    ):
        destination = tmp_path / source_name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / source_name, destination)

    path = tmp_path / relative_path
    source = path.read_text(encoding="utf-8")
    region_start = source.find(region_marker)
    assert region_start >= 0
    mutation = source.find(old, region_start)
    assert mutation >= 0
    next_item = re.search(
        r"(?m)^[ \t]*(?:pub(?:\([^)]*\))?[ \t]+)?(?:async[ \t]+)?fn[ \t]+",
        source[region_start + len(region_marker) :],
    )
    if next_item is not None:
        next_item_start = region_start + len(region_marker) + next_item.start()
        assert mutation < next_item_start, (
            "mutation escaped the production Rust item selected by its region marker",
            relative_path,
            region_marker,
            old,
        )
    path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )

    errors = module._exact_output_production_source_fidelity_errors(tmp_path)
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

    proof_path.write_text(
        mutate_tla_theorem(
            proof_source,
            "ApplicationCompletionProgressObligation",
            "StarvationFreedomObligation",
            "ApplicationCompletionProgressObligation",
        ),
        encoding="utf-8",
    )
    errors = module._application_completion_source_fidelity_errors(fidelity_dir)
    assert any(
        "must compose the reviewed exact-corridor dependencies in order" in error
        for error in errors
    )

    proof_path.write_text(
        mutate_tla_theorem(
            proof_source,
            "DecisionPipelineStagePersistsUntilExactHandoff",
            "ExecuteApply",
            "ExecuteSignVote",
        ),
        encoding="utf-8",
    )
    errors = module._application_completion_source_fidelity_errors(fidelity_dir)
    assert any(
        "DecisionPipelineStagePersistsUntilExactHandoff proof must retain exact"
        in error
        and "ExecuteApply" in error
        for error in errors
    )

    proof_path.write_text(
        mutate_tla_theorem(
            proof_source,
            "ActiveDecisionCertifiedRequestReachesCertifiedFetch",
            "PostGstAdmitHiddenPacket",
            "PostGstAdmitHistoricalRecoveryPacket",
        ),
        encoding="utf-8",
    )
    errors = module._application_completion_source_fidelity_errors(fidelity_dir)
    assert any(
        "ActiveDecisionCertifiedRequestReachesCertifiedFetch proof must retain exact"
        in error
        and "PostGstAdmitHiddenPacket" in error
        for error in errors
    )

    proof_path.write_text(
        mutate_tla_theorem(
            proof_source,
            "ResponsiveDecisionReachesApplicationFromExactCorridor",
            "RecoveryAwareDecisionWitnessProjectsApplicationFrontier",
            "HistoricalDecisionConcreteLeafProperties",
        ),
        encoding="utf-8",
    )
    errors = module._application_completion_source_fidelity_errors(fidelity_dir)
    assert any(
        "may not rely on the application result itself" in error
        and "HistoricalDecisionConcreteLeafProperties" in error
        for error in errors
    )

    proof_path.write_text(
        mutate_tla_operator(
            proof_source,
            "DecisionPipelineKinds",
            '"Apply"',
            '"SignVote"',
        ),
        encoding="utf-8",
    )
    errors = module._application_completion_source_fidelity_errors(fidelity_dir)
    assert any(
        "DecisionPipelineKinds must equal only" in error for error in errors
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
        Path("crates/iroha_core/src/sumeragi/v2_runtime.rs"),
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
        (
            "~CandidateScheduled(\n"
            "                               CertifiedResponseCandidate(item))",
            "~CandidateInFlight(\n"
            "                               CertifiedResponseCandidate(item))",
            "IngressItemCanDrain CertifiedResponse branch must use exactly one "
            "scheduler-wide CandidateScheduled coalescing arm",
        ),
        (
            "~CandidateScheduled(\n"
            "                                    "
            "CommitCertificateResponseCandidate(item))",
            "~CandidateInFlight(\n"
            "                                    "
            "CommitCertificateResponseCandidate(item))",
            "IngressItemCanDrain CommitCertificateResponse branch must use "
            "exactly one scheduler-wide CandidateScheduled "
            "coalescing arm",
        ),
        (
            "~CandidateScheduled(\n"
            "                               CertifiedResponseCandidate(item))",
            "TRUE",
            "IngressItemCanDrain CertifiedResponse branch must use exactly one "
            "scheduler-wide CandidateScheduled coalescing arm",
        ),
        (
            "~CandidateScheduled(\n"
            "                                    "
            "CommitCertificateResponseCandidate(item))",
            "TRUE",
            "IngressItemCanDrain CommitCertificateResponse branch must use "
            "exactly one scheduler-wide CandidateScheduled "
            "coalescing arm",
        ),
        (
            "                    \\/ CandidateScheduled(\n"
            "                         CertifiedResponseCandidate(item))\n",
            "",
            "IngressItemCanDrain CertifiedResponse branch must use exactly one "
            "scheduler-wide CandidateScheduled coalescing arm",
        ),
        (
            "                         \\/ CandidateScheduled(\n"
            "                              "
            "CommitCertificateResponseCandidate(item))\n",
            "",
            "IngressItemCanDrain CommitCertificateResponse branch must use "
            "exactly one scheduler-wide CandidateScheduled coalescing arm",
        ),
        (
            "IN /\\ IF CandidateScheduled(completion)\n"
            "                                  THEN UNCHANGED",
            "IN /\\ IF FALSE\n"
            "                                  THEN UNCHANGED",
            "DrainFairIngressSelected CertifiedResponse branch must consume "
            "an exact scheduled response",
        ),
        (
            "IN /\\ IF CandidateScheduled(\n"
            "                                               discoveredCandidate)\n"
            "                                        THEN UNCHANGED",
            "IN /\\ IF FALSE\n"
            "                                        THEN UNCHANGED",
            "DrainFairIngressSelected CommitCertificateResponse branch must "
            "consume an exact scheduled response",
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

    path.write_text(
        mutate_tla_operator(
            source,
            "CandidateScheduled",
            " \\cup CausalCandidates",
            "",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any("CandidateScheduled must equal only" in error for error in errors), errors

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

    effects_path.write_text(
        mutate_effect_item(
            "step",
            "        if let Err(reason) = self.runtime.take_scheduler_ownership() {\n",
            "        if false {\n",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "step must consume the exact scheduler owner immediately after the runtime step"
        in error
        or "retained effect FIFO step declaration and complete control flow must match"
        in error
        for error in errors
    ), errors

    effects_path.write_text(
        mutate_effect_item(
            "step_pending_tip_recovery",
            "        if let Err(reason) = self.runtime.take_scheduler_ownership() {\n",
            "        if false {\n",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "step_pending_tip_recovery must consume the exact scheduler owner immediately "
        "after the runtime step" in error
        or "retained effect FIFO step_pending_tip_recovery declaration and complete "
        "control flow must match" in error
        for error in errors
    ), errors

    effects_path.write_text(
        mutate_effect_item(
            "take_scheduler_ownership",
            "SerializedV2Runtime::take_last_scheduler_ownership(self)",
            "None",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "production exact scheduler ownership handoff declaration and complete control "
        "flow must match" in error
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
const fn constant_geometry() -> usize { 4 }
pub async fn asynchronous_start() {}
pub async fn destructured_start(Config { max_frame_bytes, .. }: Config) {
    if max_frame_bytes > MAX_FRAME_BYTES {
        return;
    }
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
    assert len(module.rust_items(source, "constant_geometry")) == 1
    assert len(module.rust_items(source, "asynchronous_start")) == 1
    destructured = module.rust_items(source, "destructured_start")
    assert len(destructured) == 1
    assert "if max_frame_bytes > MAX_FRAME_BYTES" in destructured[0].body

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

    def mutate_adapter_item(name: str, old: str, new: str) -> str:
        item = module.rust_items(canonical_adapter, name)[0]
        assert item.source.count(old) == 1, (name, old)
        start = canonical_adapter.index(item.source)
        end = start + len(item.source)
        return (
            canonical_adapter[:start]
            + item.source.replace(old, new, 1)
            + canonical_adapter[end:]
        )

    adapter.write_text(
        mutate_adapter_item(
            "budget",
            "Self::InstallTimeout => PersistenceMacroStepBudget::new(2, 4),",
            "Self::InstallTimeout => PersistenceMacroStepBudget::new(2, 5),",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "record-specific persistence macro-step budget declaration, contract, "
        "and complete control flow must match" in error
        for error in errors
    ), errors
    adapter.write_text(canonical_adapter, encoding="utf-8")

    for deferred_owner in (
        "            && self.deferred_completions.is_empty()\n",
        "            && self.deferred_progress_inputs.is_empty()\n",
        "            && self.deferred_inputs.is_empty()\n",
    ):
        adapter.write_text(
            mutate_adapter_item("ready_to_finish", deferred_owner, ""),
            encoding="utf-8",
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(
            "terminal adapter deferred-debt readiness fence declaration, "
            "contract, and complete control flow must match" in error
            for error in errors
        ), errors
        adapter.write_text(canonical_adapter, encoding="utf-8")

    deferred_owner_adapter_mutations = (
        (
            "matches_authenticated_runtime_bytes",
            "identity == canonical_bytes",
            "identity != canonical_bytes",
            "exact deferred canonical-envelope comparator declaration, contract, "
            "and complete control flow must match",
        ),
        (
            "deferred_authenticated_message_owner",
            "owned == encoded.as_slice()",
            "owned != encoded.as_slice()",
            "exact Busy-deferred authenticated-envelope owner lookup declaration, contract, and "
            "complete control flow must match",
        ),
        (
            "authenticated_deferred_admission_ordinals",
            ".filter(|input| input.retag_authenticated_ingress)",
            ".filter(|input| !input.retag_authenticated_ingress)",
            "complete authenticated Busy-deferred ordinal snapshot declaration, contract, and "
            "complete control flow must match",
        ),
        (
            "deferred_authenticated_event_matches_wire",
            "message.encode().as_slice() == identity",
            "message.encode().as_slice() != identity",
            "typed deferred event to canonical-envelope comparator declaration, contract, and "
            "complete control flow must match",
        ),
    )
    for item_name, old, new, expected_error in deferred_owner_adapter_mutations:
        adapter.write_text(
            mutate_adapter_item(item_name, old, new),
            encoding="utf-8",
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        adapter.write_text(canonical_adapter, encoding="utf-8")

    adapter.write_text(
        mutate_adapter_item(
            "drain_deferred_with_evidence",
            "self.deferred_authenticated_event_matches_wire(&selection.evidence)",
            "true",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "single-transition adapter deferred ownership dispatcher declaration, contract, "
        "and complete control flow must match" in error
        for error in errors
    ), errors
    adapter.write_text(canonical_adapter, encoding="utf-8")

    adapter.write_text(
        mutate_adapter_item(
            "fail_deferred_service_contract",
            "self.fail_closed = true;",
            "self.fail_closed = false;",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "terminal deferred-service contract failure declaration, contract, and "
        "complete control flow must match" in error
        for error in errors
    ), errors
    adapter.write_text(canonical_adapter, encoding="utf-8")

    helper_call = (
        "                    reducer::prepend_causal_continuation("
        "&mut pending, continuation);\n"
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

    runtime = tmp_path / "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    canonical_runtime = runtime.read_text(encoding="utf-8")

    trait_deferred_method = (
        "    fn authenticated_deferred_admission_ordinals(&self) -> BTreeSet<u128>;\n"
    )
    assert canonical_runtime.count(trait_deferred_method) == 1
    runtime.write_text(
        canonical_runtime.replace(
            trait_deferred_method,
            "    #[cfg(test)]\n" + trait_deferred_method,
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "RuntimeDriver authenticated deferred-owner source, snapshot, and exact "
        "dispatch methods must be adjacent on the production trait surface" in error
        for error in errors
    ), errors
    runtime.write_text(canonical_runtime, encoding="utf-8")

    production_driver_context = (
        ("impl", "RuntimeDriver", "for", "SumeragiV2Adapter"),
    )
    def mutate_runtime_item(name: str, old: str, new: str) -> str:
        item = module.rust_items(canonical_runtime, name)[0]
        assert item.source.count(old) == 1, (name, old)
        start = canonical_runtime.index(item.source)
        end = start + len(item.source)
        return (
            canonical_runtime[:start]
            + item.source.replace(old, new, 1)
            + canonical_runtime[end:]
        )

    def mutate_runtime_item_in_context(
        name: str,
        context: tuple[tuple[str, ...], ...],
        old: str,
        new: str,
    ) -> str:
        items = tuple(
            item
            for item in module.rust_items(canonical_runtime, name)
            if item.brace_context == context
        )
        assert len(items) == 1, (name, context)
        item = items[0]
        assert item.source.count(old) == 1, (name, old)
        start = canonical_runtime.index(item.source)
        end = start + len(item.source)
        return (
            canonical_runtime[:start]
            + item.source.replace(old, new, 1)
            + canonical_runtime[end:]
        )

    production_driver_mutations = (
        (
            "dispatch",
            "if !ownership.matches_authenticated(&message)",
            "if false",
            "production authenticated runtime dispatch bridge declaration and complete control flow must match",
        ),
        (
            "dispatch_deferred",
            "SumeragiV2Adapter::drain_deferred_with_evidence(self)",
            "Ok(None)",
            "production exact deferred ownership dispatch bridge declaration and complete control flow must match",
        ),
    )
    for item_name, old, new, expected_error in production_driver_mutations:
        runtime.write_text(
            mutate_runtime_item_in_context(
                item_name, production_driver_context, old, new
            ),
            encoding="utf-8",
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        runtime.write_text(canonical_runtime, encoding="utf-8")

    deferred_owner_runtime_mutations = (
        (
            "from_fair_ingress",
            "if outer == *message",
            "if outer != *message",
            "canonical fair-ingress ownership constructor declaration and complete control flow must match",
        ),
        (
            "matches_authenticated",
            "self.runtime_bytes.as_ref() == authenticated.canonical_wire_bytes().as_slice()",
            "self.runtime_bytes.as_ref() != authenticated.canonical_wire_bytes().as_slice()",
            "post-authentication canonical payload comparator declaration and complete control flow must match",
        ),
        (
            "can_merge_downstream",
            "self.runtime_bytes != candidate.runtime_bytes",
            "self.runtime_bytes == candidate.runtime_bytes",
            "non-mutating per-source ownership merge preflight declaration and complete control flow must match",
        ),
        (
            "merge_downstream",
            "self.runtime_bytes != candidate.runtime_bytes",
            "self.runtime_bytes == candidate.runtime_bytes",
            "per-source ownership merge transition declaration and complete control flow must match",
        ),
        (
            "reconcile_deferred_ingress_ownership",
            "if !active.contains(&ordinal) || !candidate.validate_exact()",
            "if active.contains(&ordinal) || !candidate.validate_exact()",
            "authenticated deferred carrier reconciliation declaration and complete control flow must match",
        ),
        (
            "accept_driver_dispatch",
            "if !self.reconcile_deferred_ingress_ownership(dispatch.deferred_ingress)",
            "        if false {",
            "driver dispatch ownership acceptance declaration and complete control flow must match",
        ),
        (
            "enqueue_network_with_ingress_ownership",
            "if authenticated_deferred_owner != deferred_owner",
            "if false",
            "authenticated ingress ownership admission and deferred merge declaration and complete control flow must match",
        ),
        (
            "can_admit_network_message_with_ingress_ownership",
            ".is_some_and(|retained| retained.can_merge_downstream(&ownership))",
            ".is_some()",
            "authenticated ingress ownership capacity preflight declaration and complete control flow must match",
        ),
        (
            "take_last_scheduler_ownership",
            "self.last_scheduler_ownership.take()",
            "self.last_scheduler_ownership.clone()",
            "runner scheduler ownership handoff declaration and complete control flow must match",
        ),
        (
            "commit_certificate_response_coalesces_with_exact_busy_deferred_qc",
            "            Some(0),",
            "            None,",
            "Busy-deferred authenticated response coalescing regression declaration and complete control flow must match",
        ),
    )
    for item_name, old, new, expected_error in deferred_owner_runtime_mutations:
        runtime.write_text(
            mutate_runtime_item(item_name, old, new),
            encoding="utf-8",
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        runtime.write_text(canonical_runtime, encoding="utf-8")

    runtime.write_text(
        mutate_runtime_item(
            "dispatch_one_adapter_deferred",
            "if !self.driver.deferred_work_is_serviceable()",
            "if self.driver.deferred_work_is_serviceable()",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "single adapter-deferred runtime dispatcher declaration, contract, and "
        "complete control flow must match" in error
        for error in errors
    ), errors
    runtime.write_text(canonical_runtime, encoding="utf-8")

    runtime.write_text(
        mutate_runtime_item(
            "step",
            "        if let Some(step) = self.dispatch_one_adapter_deferred(now)? {\n",
            "        if false {\n"
            "            return Ok(RuntimeStep::Idle);\n"
            "        }\n"
            "        if let Some(step) = self.dispatch_one_adapter_deferred(now)? {\n",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "live serialized runtime step declaration, contract, and complete "
        "control flow must match" in error
        for error in errors
    ), errors
    runtime.write_text(canonical_runtime, encoding="utf-8")

    refinement = (
        tmp_path / "crates/iroha_core/src/sumeragi/v2_core/refinement.rs"
    )
    canonical_refinement = refinement.read_text(encoding="utf-8")
    refinement.write_text(
        canonical_refinement.replace("#[allow(dead_code)]\n", "", 1),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "prepend_causal_continuation kernel must have exact reviewed attributes"
        in error
        for error in errors
    ), errors
    refinement.write_text(canonical_refinement, encoding="utf-8")

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

ExactLockedCommitTimeoutRecoveryWitness(node, qc) ==
  /\ qc.context = context
  /\ qc.height = height
  /\ qc.view = lockRank[node]
  /\ qc.subject = lockSubject[node]
  /\ qc.view < nodeView[node]
  /\ \E timeoutVote \in timeoutIntents:
       /\ timeoutVote.signer = node
       /\ timeoutVote.context = qc.context
       /\ timeoutVote.height = qc.height
       /\ timeoutVote.view = nodeView[node]

HistoricalLockedCommitRecoveryWitness(node, qc) ==
  \/ ExactLockedCommitIntents(node, qc.view, qc.subject) # {}
  \/ \E request \in pendingLockCommit:
       /\ request.node = node
       /\ request.qc = qc
  \/ \E candidate \in AsyncCandidateSet:
       /\ candidate.node = node
       /\ candidate.height = qc.context.height
       /\ candidate.view = qc.view
       /\ candidate.subject = qc.subject
       /\ candidate.kind = "BeginLockCommit"
       /\ CandidateScheduled(candidate)
  \/ ExactLockedCommitTimeoutRecoveryWitness(node, qc)
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
        "  /\\ qc.context = context\n",
        (
            "  /\\ qc.height = height\n",
            "  /\\ qc.height >= height\n",
        ),
        "  /\\ qc.view < nodeView[node]\n",
        "       /\\ timeoutVote.context = qc.context\n",
        (
            "       /\\ timeoutVote.view = nodeView[node]\n",
            "       /\\ timeoutVote.view >= nodeView[node]\n",
        ),
        "  \\/ ExactLockedCommitTimeoutRecoveryWitness(node, qc)\n",
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
            "exact reviewed progress/recovery contract" in error
            for error in errors
        ), errors


def test_progress_witness_source_fidelity_seals_post_decision_timeout_boundary(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Core.tla",
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2CertifiedRequestHashAuthorityProofs.tla",
        "SumeragiV2DurableDecisionRecoveryProofs.tla",
        "SumeragiV2AsyncLivenessProofs.tla",
    )
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    core_path = formal_dir / "SumeragiV2Core.tla"
    network_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    async_path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    integration_path = tmp_path / "integration_tests/tests/sumeragi_v2_runner.rs"
    canonical_core = core_path.read_text(encoding="utf-8")
    canonical_network = network_path.read_text(encoding="utf-8")
    canonical_async = async_path.read_text(encoding="utf-8")
    canonical_integration = integration_path.read_text(encoding="utf-8")
    baseline_errors = module._progress_witness_source_fidelity_errors(formal_dir)

    def assert_new_contract_error(errors: list[str], expected_error: str) -> None:
        assert not any(expected_error in error for error in baseline_errors), (
            expected_error,
            baseline_errors,
        )
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )

    core_mutations = (
        (
            "    /\\ decision.qc.context = context\n",
            "",
            "NoDecisionForNode must equal only",
        ),
        (
            "     /\\ NoDecisionForNode(node)\n",
            "",
            "must have one direct, NoDecisionForNode guard",
        ),
        (
            "     /\\ NoDecisionForNode(node)\n",
            "     /\\ (NoDecisionForNode(node) \\/ TRUE)\n",
            "must have one direct, NoDecisionForNode guard",
        ),
        (
            "     /\\ NodeIdle(node)\n"
            "     /\\ NoDecisionForNode(node)\n"
            "     /\\ roundView + 1 \\in Views\n"
            "     /\\ TCValid(tc)\n"
            "     /\\ \\/ roundView >= nodeView[node]\n"
            "        \\/ StrictSameRoundTcUpgrade(node, tc)\n",
            "     /\\ NodeIdle(node)\n"
            "     /\\ roundView + 1 \\in Views\n"
            "     /\\ TCValid(tc)\n"
            "     /\\ \\/ roundView >= nodeView[node]\n"
            "        \\/ StrictSameRoundTcUpgrade(node, tc)\n",
            "FormTC must have one direct, NoDecisionForNode guard",
        ),
        (
            "     /\\ tc.view + 1 \\in Views\n"
            "     /\\ \\/ tc.view >= nodeView[node]\n"
            "        \\/ StrictSameRoundTcUpgrade(node, tc)\n"
            "     /\\ NodeIdle(node)\n"
            "     /\\ NoDecisionForNode(node)\n"
            "     /\\ pendingInstallTC' = pendingInstallTC \\cup {request}\n",
            "     /\\ tc.view + 1 \\in Views\n"
            "     /\\ \\/ tc.view >= nodeView[node]\n"
            "        \\/ StrictSameRoundTcUpgrade(node, tc)\n"
            "     /\\ NodeIdle(node)\n"
            "     /\\ pendingInstallTC' = pendingInstallTC \\cup {request}\n",
            "BeginInstallTC must have one direct, NoDecisionForNode guard",
        ),
        (
            "     /\\ NoDecisionForNode(node)\n"
            "     /\\ vote \\in timeoutIntents\n",
            "     /\\ vote \\in timeoutIntents\n",
            "ResumeTimeout must have one direct, NoDecisionForNode guard",
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
        assert_new_contract_error(errors, expected_error)
        core_path.write_text(canonical_core, encoding="utf-8")

    semantic_core_mutations = (
        (
            "NoHigherPrepareOriginKnown",
            "       /\\ vote.view > qc.view\n",
            "       /\\ vote.view > qc.view\n"
            "       /\\ vote.subject # qc.subject\n",
            "NoHigherPrepareOriginKnown must equal only",
        ),
        (
            "StrictSameRoundTcUpgrade",
            "  /\\ generation[node] < MaxGeneration\n",
            "",
            "StrictSameRoundTcUpgrade must equal only",
        ),
        (
            "ProposalJustified",
            "          /\\ TcHighRank(installed.tc) = NoRank\n",
            "          /\\ proposal.justifyRank = TcHighRank(installed.tc)\n",
            "ProposalJustified must equal only",
        ),
        (
            "SafeToPrepare",
            "  \\/ /\\ proposal.view = lockRank[node]\n",
            "  \\/ lockSubject[node] = proposal.subject\n"
            "  \\/ /\\ proposal.view = lockRank[node]\n",
            "SafeToPrepare must equal only",
        ),
        (
            "PersistInstallTC",
            "             IF sameRoundUpgrade THEN @ ELSE tc.view + 1]\n",
            "             tc.view + 1]\n",
            "PersistInstallTC must preserve the strict same-round",
        ),
    )
    for symbol, needle, replacement, expected_error in semantic_core_mutations:
        core_path.write_text(
            mutate_tla_operator(canonical_core, symbol, needle, replacement),
            encoding="utf-8",
        )
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert_new_contract_error(errors, expected_error)
        core_path.write_text(canonical_core, encoding="utf-8")

    integration_helper_mutations = (
        (
            "locked_commit_has_exact_progress_witness",
            "current_view > locked.proposal_round.view",
            "current_view >= locked.proposal_round.view",
        ),
        (
            "validate_locked_commit_progress_witness",
            "snapshot.height,\n            snapshot.view,",
            "snapshot.last_committed_height,\n            snapshot.view,",
        ),
    )
    for symbol, needle, replacement in integration_helper_mutations:
        mutate_rust_item_source(
            module, integration_path, symbol, needle, replacement
        )
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert_new_contract_error(
            errors,
            f"progress-witness helper {symbol} must match exact reviewed",
        )
        integration_path.write_text(canonical_integration, encoding="utf-8")

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
        assert_new_contract_error(errors, expected_error)
        network_path.write_text(canonical_network, encoding="utf-8")

    async_mutations = (
        (
            "  <<context, decisions, pendingTimeout, pendingInstallTC,\n",
            "  <<decisions, pendingTimeout, pendingInstallTC,\n",
            "DecisionTimeoutFrontierVars must equal only",
        ),
        (
            "      BY <1>1, <2>12, ResumeTimeoutPreservesDecisionTimeoutFrontier\n",
            "      BY <1>1, <2>12, CrashPreservesDecisionTimeoutFrontier\n",
            "CoreNextPreservesDecisionTimeoutFrontier must retain the complete",
        ),
        (
            "      BY AsyncBracketPreservesDecisionTimeoutFrontier\n",
            "      BY AsyncInitEstablishesDecisionTimeoutFrontier\n",
            "DecisionTimeoutFrontierInvariantFromAsyncSpec must retain the complete",
        ),
        (
            "      BY DecisionTimeoutFrontierInvariantFromAsyncSpec\n",
            "      BY AsyncTypeInvariantObligation\n",
            "PostDecisionTimeoutExclusionObligation must retain the complete",
        ),
    )
    for needle, replacement, expected_error in async_mutations:
        assert needle in canonical_async, needle
        async_path.write_text(
            canonical_async.replace(needle, replacement, 1), encoding="utf-8"
        )
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert_new_contract_error(errors, expected_error)
        async_path.write_text(canonical_async, encoding="utf-8")


def test_progress_witness_source_fidelity_requires_exact_crash_authority(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2CertifiedRequestHashAuthorityProofs.tla",
        "SumeragiV2DurableDecisionRecoveryProofs.tla",
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2AsyncNetwork.tla",
    )
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    canonical = path.read_text(encoding="utf-8")
    hash_path = formal_dir / "SumeragiV2CertifiedRequestHashAuthorityProofs.tla"
    canonical_hash = hash_path.read_text(encoding="utf-8")
    recovery_path = formal_dir / "SumeragiV2DurableDecisionRecoveryProofs.tla"
    canonical_recovery = recovery_path.read_text(encoding="utf-8")
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
            "AsyncProgressWitnessAndHistoricalRecoveryProperty(\n"
            "      AsyncSpecAt(initialContext))",
            "AsyncProgressWitnessProperty(\n"
            "      AsyncSpecAt(initialContext))",
            "ProgressWitnessObligation must use the crash-aware async plus historical",
        ),
        (
            "DecisionPipelineKindOwned(node, qc, kind) ==\n"
            "  \\E candidate \\in AsyncCandidateSet:\n"
            "    /\\ candidate.kind = kind\n",
            "DecisionPipelineKindOwned(node, qc, kind) ==\n"
            "  \\E candidate \\in AsyncCandidateSet:\n"
            "    /\\ candidate.kind = \"FetchBody\"\n",
            "DecisionPipelineKindOwned must equal only",
        ),
        (
            "DecisionFetchBodyOwned(node, qc) ==\n"
            "  DecisionPipelineKindOwned(node, qc, \"FetchBody\")\n",
            "DecisionFetchBodyOwned(node, qc) ==\n"
            "  DecisionPipelineKindOwned(node, qc, \"StoreBody\")\n",
            "DecisionFetchBodyOwned must equal only",
        ),
        (
            "DecisionRecoveryAuthority(node, qc) ==\n"
            "  /\\ DurableDecisionRecoveryAuthority(node, qc)\n"
            "  /\\ DurableDecisionRecoveryExecutorCurrent(node)\n",
            "DecisionRecoveryAuthority(node, qc) ==\n"
            "  DurableDecisionRecoveryAuthority(node, qc)\n",
            "DecisionRecoveryAuthority must equal only",
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
            "    /\\ command.kind = \"PersistDecision\"\n",
            "THEOREM PersistDecisionRecoveryUsesCompletionFetchBody ==\n"
            "  \\A command:\n"
            "    /\\ command.kind = \"BeginDecision\"\n",
            "PersistDecision recovery theorem must state only",
        ),
        (
            "         /\\ Len(CommandSuccessors(command)) = 1\n",
            "         /\\ Len(CommandSuccessors(command)) = 3\n",
            "PersistDecision recovery theorem must state only",
        ),
        (
            "BY DEF CommandSuccessors, PersistDecisionFetchSuccessor,\n"
            "       AsyncCandidateAtConsumer, AsyncCandidateWithIdentity,\n"
            "       CandidateConsumerCurrent, PersistDecisionRequests\n",
            "BY DEF CommandSuccessors, PersistDecisionFetchSuccessor,\n"
            "       AsyncCandidateAtConsumer,\n"
            "       CandidateConsumerCurrent, PersistDecisionRequests\n",
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
            "AsyncDecisionCompletionWitness(node, qc) ==\n"
            "  \\/ DecisionCompletionWitness(node, qc)\n"
            "  \\/ DecisionRecoveryAuthority(node, qc)\n",
            "AsyncDecisionCompletionWitness(node, qc) ==\n"
            "  DecisionCompletionWitness(node, qc)\n",
            "AsyncDecisionCompletionWitness must equal only",
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
            "ProgressWitnessProductionRefinementObligation ==\n"
            "  /\\ ProductionProgressWitnessTraceRefinement\n"
            "  /\\ ProgressWitnessObligation\n",
            "ProgressWitnessProductionRefinementObligation ==\n"
            "  /\\ TRUE\n"
            "  /\\ ProgressWitnessObligation\n",
            "progress-witness ledger operator must state exactly",
        ),
        (
            "ProgressWitnessProductionRefinementObligation ==\n",
            "THEOREM ProgressWitnessProductionRefinementObligation ==\n",
            "must remain a top-level operator",
        ),
        (
            "    => ProgressWitnessProductionRefinementObligation\n"
            "PROOF\n",
            "    => ProductionProgressWitnessTraceRefinement\n"
            "PROOF\n",
            "progress-witness cross-tool theorem must state exactly",
        ),
        (
            "  BY ProgressWitnessObligation\n"
            "     DEF ProgressWitnessProductionRefinementObligation\n",
            "  BY TRUE\n"
            "     DEF ProgressWitnessProductionRefinementObligation\n",
            "progress-witness cross-tool theorem must retain its exact ",
        ),
        (
            "EffectiveLockBodyAcquisitionProductionRefinementObligation ==\n"
            "  /\\ ProductionEffectiveLockBodyAcquisitionRefinement\n"
            "  /\\ EffectiveLockAcquisitionModelObligation\n",
            "THEOREM EffectiveLockBodyAcquisitionProductionRefinementObligation ==\n"
            "  /\\ ProductionEffectiveLockBodyAcquisitionRefinement\n"
            "  /\\ EffectiveLockAcquisitionModelObligation\n",
            "must remain a top-level operator",
        ),
        (
            "  /\\ EffectiveLockAcquisitionModelObligation\n\n"
            "THEOREM EffectiveLockBodyAcquisitionCrossToolRefinement ==",
            "  /\\ TRUE\n\n"
            "THEOREM EffectiveLockBodyAcquisitionCrossToolRefinement ==",
            "effective-lock ledger operator must state exactly",
        ),
        (
            "    => EffectiveLockBodyAcquisitionProductionRefinementObligation\n"
            "PROOF\n",
            "    => ProductionEffectiveLockBodyAcquisitionRefinement\n"
            "PROOF\n",
            "effective-lock cross-tool theorem must state exactly",
        ),
        (
            "  BY EffectiveLockAcquisitionModelObligation\n"
            "     DEF EffectiveLockBodyAcquisitionProductionRefinementObligation\n",
            "  BY TRUE\n"
            "     DEF EffectiveLockBodyAcquisitionProductionRefinementObligation\n",
            "must retain its exact model-obligation bridge proof",
        ),
        (
            "      BY ExactDurableDecisionRecoveryLifecycleTransition\n",
            "      BY StrongInductiveInvariantProjectsTypeInvariant\n",
            "DecisionRecoveryAcrossRestartObligation must retain its complete ",
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

    async_network_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    canonical_async_network = async_network_path.read_text(encoding="utf-8")
    async_network_mutations = (
        (
            "    /\\ request.qc.subject = command.subject}\n",
            "    /\\ request.qc.subject = command.view}\n",
            "PersistDecisionRequests must equal only",
        ),
        (
            '       "Completion", "FetchBody", request.node, qc.context.height,\n',
            '       "Progress", "FetchBody", request.node, qc.context.height,\n',
            "PersistDecisionFetchSuccessor must equal only",
        ),
    )
    for needle, replacement, expected_error in async_network_mutations:
        assert needle in canonical_async_network, needle
        async_network_path.write_text(
            canonical_async_network.replace(needle, replacement, 1),
            encoding="utf-8",
        )
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        async_network_path.write_text(canonical_async_network, encoding="utf-8")

    recovery_mutations = (
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
            "       /\\ decision.qc.context = request.qc.context\n",
            "       /\\ TRUE\n",
            "PendingDecisionExcludesDurableDecision must equal only",
        ),
        (
            "   subject |-> qc.subject]\n\nDecisionCertifiedRequestRegistered",
            "   subject |-> qc.subject,\n"
            "   generation |-> generation[node]]\n\n"
            "DecisionCertifiedRequestRegistered",
            "DecisionCertifiedRequestIdentityFor must equal only",
        ),
        (
            '  /\\ asyncRecoveryPhase \\in {"RestartRequired", "ReplayRequired"}\n',
            '  /\\ asyncRecoveryPhase \\in '
            '{"RestartRequired", "ReplayRequired", "Replaying"}\n',
            "DurableDecisionRecoveryAuthority must equal only",
        ),
        (
            "  /\\ asyncRecoveryNode = node\n"
            "  /\\ [node |-> node, qc |-> qc] \\in RestartDecisions(node)\n\n"
            "DurableDecisionRecoveryExecutorCurrent",
            "  /\\ asyncRecoveryNode = node\n"
            "  /\\ generation[node] = asyncRecoveryGeneration\n"
            "  /\\ [node |-> node, qc |-> qc] \\in RestartDecisions(node)\n\n"
            "DurableDecisionRecoveryExecutorCurrent",
            "DurableDecisionRecoveryAuthority must equal only",
        ),
        (
            "                     node, qc, nodeView[node], generation[node])>>]\n",
            "                     node, qc, nodeView[node], "
            "asyncRecoveryGeneration)>>]\n",
            "ExactCurrentDecisionFetchUpdate must equal only",
        ),
        (
            '    qc.phase = "Prepare"\n'
            "      => ~DurableDecisionRecoveryAuthority(node, qc)\n",
            '    qc.phase = "Commit"\n'
            "      => ~DurableDecisionRecoveryAuthority(node, qc)\n",
            "PrepareCertificateCannotAuthorizeDurableDecisionRecovery must state only",
        ),
        (
            "      BY <1>1, <2>4,\n"
            "         PersistDecisionPreservesDecisionFrontierUniqueness\n",
            "      BY <1>1, <2>4,\n"
            "         CrashPreservesDecisionFrontierUniqueness\n",
            "CoreNextPreservesDecisionFrontierUniqueness must retain its complete",
        ),
        (
            "       /\\ ExactCurrentDecisionFetchUpdate(node, qc)\n\n"
            "DecisionRecoveryAcrossRestartProperty",
            "       /\\ DecisionRecoveryStage(node, qc)'\n\n"
            "DecisionRecoveryAcrossRestartProperty",
            "DurableDecisionRecoveryLifecycleTransition must equal only",
        ),
        (
            "          /\\ (DecisionRawHashRegistered(node, qc)\n"
            "                <=> DecisionRawHashRegistered(node, qc)')\n"
            "          /\\ (DecisionCertifiedRequestRegistered(node, qc)\n",
            "          /\\ (DecisionCertifiedRequestRegistered(node, qc)\n",
            "DurableDecisionRecoveryLifecycleTransition must equal only",
        ),
        (
            "       => /\\ ~DurableDecisionRecoveryAuthority(node, qc)'\n"
            "          /\\ ~DecisionRawHashRegistered(node, qc)'\n"
            "          /\\ ~DecisionCertifiedRequestRegistered(node, qc)'\n",
            "       => /\\ ~DurableDecisionRecoveryAuthority(node, qc)'\n"
            "          /\\ ~DecisionCertifiedRequestRegistered(node, qc)'\n",
            "DurableDecisionRecoveryLifecycleTransition must equal only",
        ),
        (
            "      BY <1>1, <2>1, <2>2,\n"
            "         ResponsiveCrashPreservesDecisionRegistration, SMT\n",
            "      BY <1>1, <2>1, <2>2, SMT\n",
            "ResponsiveCrashPreservesExactDecisionRegistrations must retain its complete",
        ),
        (
            "      BY <1>1, <2>3, AuthenticatedRestartPreservesRawRegistration\n",
            "      BY <1>1, <2>3, SMT\n",
            "ResponsiveRestartPreservesExactDecisionRegistrations must retain its complete",
        ),
        (
            "      BY <1>1, <2>3, ResponsiveReplayClearsRecoveredNodeRegistration\n",
            "      BY <1>1, <2>3, SMT\n",
            "ResponsiveReplayInstallsExactCurrentDecisionFetchUpdate must retain its complete",
        ),
        (
            "THEOREM ResponsiveRestartPreservesExactDecisionRegistrations ==\n"
            "  \\A node, qc:\n"
            "    /\\ StrongInductiveInvariant\n",
            "THEOREM ResponsiveRestartPreservesExactDecisionRegistrations ==\n"
            "  \\A node, qc:\n"
            "    /\\ TypeInvariant\n",
            "ResponsiveRestartPreservesExactDecisionRegistrations must state only",
        ),
        (
            "THEOREM ExactDurableDecisionRecoveryLifecycleTransition ==\n"
            "  StrongInductiveInvariant => "
            "DurableDecisionRecoveryLifecycleTransition\n",
            "THEOREM ExactDurableDecisionRecoveryLifecycleTransition ==\n"
            "  TypeInvariant => DurableDecisionRecoveryLifecycleTransition\n",
            "ExactDurableDecisionRecoveryLifecycleTransition must state only",
        ),
        (
            "    <2>1. asyncRecoveryNode = node\n"
            "      BY <1>1 DEF DurableDecisionRecoveryAuthority\n"
            "    <2>2. asyncRecoveryNode' = node\n"
            "      BY <1>1, <2>1 DEF PreGstResponsiveRestart\n",
            "    <2>1. TRUE\n"
            "      BY <1>1 DEF DurableDecisionRecoveryAuthority\n"
            "    <2>2. asyncRecoveryNode' = node\n"
            "      BY <1>1, <2>1 DEF PreGstResponsiveRestart\n",
            "ResponsiveRestartPreservesExactDecisionRegistrations must retain its complete",
        ),
        (
            "    <2>2. asyncRecoveryNode' = node\n"
            "      BY <1>1, <2>1 DEF PreGstResponsiveRestart\n",
            "    <2>2. asyncRecoveryNode' = asyncRecoveryNode\n"
            "      BY <1>1, <2>1 DEF PreGstResponsiveRestart\n",
            "ResponsiveRestartPreservesExactDecisionRegistrations must retain its complete",
        ),
        (
            "    <2>1. asyncRecoveryNode = node\n"
            "      BY <1>1 DEF DurableDecisionRecoveryAuthority\n"
            "    <2>2. asyncRecoveryNode' = node\n"
            "      BY <1>1, <2>1 DEF PreGstResponsiveReplay\n",
            "    <2>1. TRUE\n"
            "      BY <1>1 DEF DurableDecisionRecoveryAuthority\n"
            "    <2>2. asyncRecoveryNode' = node\n"
            "      BY <1>1, <2>1 DEF PreGstResponsiveReplay\n",
            "ResponsiveReplayInstallsExactCurrentDecisionFetchUpdate must retain its complete",
        ),
        (
            "    <2>2. asyncRecoveryNode' = node\n"
            "      BY <1>1, <2>1 DEF PreGstResponsiveReplay\n",
            "    <2>2. asyncRecoveryNode' = asyncRecoveryNode\n"
            "      BY <1>1, <2>1 DEF PreGstResponsiveReplay\n",
            "ResponsiveReplayInstallsExactCurrentDecisionFetchUpdate must retain its complete",
        ),
        (
            "      BY ExactDurableDecisionRecoveryLifecycleTransition\n",
            "      BY StrongInductiveInvariantProjectsTypeInvariant\n",
            "DecisionRecoveryAcrossRestartPropertyFromAsyncSpec must retain its complete",
        ),
    )
    for needle, replacement, expected_error in recovery_mutations:
        assert needle in canonical_recovery, needle
        recovery_path.write_text(
            canonical_recovery.replace(needle, replacement, 1), encoding="utf-8"
        )
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        recovery_path.write_text(canonical_recovery, encoding="utf-8")

    hash_mutations = (
        (
            "   subject |-> request.envelope.subject,\n"
            "   requester |-> request.source]\n",
            "   subject |-> request.envelope.subject,\n"
            "   requester |-> request.source,\n"
            "   recipient |-> request.envelope.recipient]\n",
            "CertifiedRequestLogicalIdentity must equal only",
        ),
        (
            "    NoAsyncItem, consumerView, consumerGeneration, qc,\n"
            "    qc.subject, qc.subject, qc.subject)\n",
            "    NoAsyncItem, consumerView, asyncRecoveryGeneration, qc,\n"
            "    qc.subject, qc.subject, qc.subject)\n",
            "DecisionFetchCandidateAt must equal only",
        ),
        (
            '  /\\ qc.phase = "Commit"\n'
            "  /\\ [node |-> node, qc |-> qc] \\in decisions\n",
            '  /\\ qc.phase = "Prepare"\n'
            "  /\\ [node |-> node, qc |-> qc] \\in decisions\n",
            "DecisionCommitAuthority must equal only",
        ),
        (
            "DecisionRawSignedRequest(node, qc) ==\n"
            "  [preimage |-> DecisionRawRequestPreimage(node, qc),\n"
            "   signature |-> DecisionRawRequestSignature(node, qc)]\n",
            "DecisionRawSignedRequest(node, qc) ==\n"
            "  [logicalIdentity |-> DecisionLogicalRequestIdentity(node, qc)]\n",
            "DecisionRawSignedRequest must equal only",
        ),
        (
            "DecisionRawRequestHash(node, qc) ==\n"
            "  [exactSignedRequest |-> DecisionRawSignedRequest(node, qc)]\n",
            "DecisionRawRequestHash(node, qc) ==\n"
            "  [logicalIdentity |-> DecisionLogicalRequestIdentity(node, qc)]\n",
            "DecisionRawRequestHash must equal only",
        ),
        (
            "DecisionRegisteredOccurrences(node, qc) ==\n"
            "  DecisionRequestOccurrences(node, qc) \\cap asyncActiveRequests\n",
            "DecisionRegisteredOccurrences(node, qc) ==\n"
            "  DecisionRequestOccurrences(node, qc)\n",
            "DecisionRegisteredOccurrences must equal only",
        ),
        (
            "DecisionRawHashRegistered(node, qc) ==\n"
            "  /\\ DecisionCommitAuthority(node, qc)\n"
            "  /\\ DecisionRegisteredOccurrences(node, qc) # {}\n",
            "DecisionRawHashRegistered(node, qc) ==\n"
            "  /\\ DecisionCommitAuthority(node, qc)\n"
            "  /\\ DecisionRequestOccurrences(node, qc) # {}\n",
            "DecisionRawHashRegistered must equal only",
        ),
        (
            "BY DEF DecisionFetchCandidateIdentityAt, DecisionFetchCandidateAt,\n"
            "       ExactAsyncCandidateIdentity, AsyncConsumerEventTag,\n",
            "BY DEF DecisionFetchCandidateIdentityAt,\n"
            "       ExactAsyncCandidateIdentity, AsyncConsumerEventTag,\n",
            "DecisionFetchCandidateIdentityHasExactProductionShape must retain its complete",
        ),
        (
            "BY RestartIncrementsSelectedGeneration, SMT\n"
            "   DEF PreGstResponsiveRestart,\n",
            "BY SMT\n   DEF PreGstResponsiveRestart,\n",
            "AuthenticatedRestartRetagsSourceConsumerGeneration must retain its complete",
        ),
        (
            "BY RestartDecisionReplayHasCurrentGeneration, SMT\n"
            "   DEF PreGstResponsiveReplay, ResetNodeSchedulerForRestart,\n",
            "BY SMT\n"
            "   DEF PreGstResponsiveReplay, ResetNodeSchedulerForRestart,\n",
            "ResponsiveReplayQueuesFreshGenerationDecisionFetch must retain its complete",
        ),
        (
            "   DecisionCertifiedPublishAddsRegistrationOccurrences,\n"
            "   DecisionRawRequestHashIsStateIndependent, SMT\n",
            "   DecisionRawRequestHashIsStateIndependent, SMT\n",
            "DecisionCertifiedPublishRegistersExactRawHash must retain its complete",
        ),
    )
    for needle, replacement, expected_error in hash_mutations:
        assert needle in canonical_hash, needle
        hash_path.write_text(
            canonical_hash.replace(needle, replacement, 1), encoding="utf-8"
        )
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        hash_path.write_text(canonical_hash, encoding="utf-8")


def test_progress_witness_source_fidelity_seals_historical_lock_restart_authority(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Core.tla",
        "SumeragiV2CertifiedRequestHashAuthorityProofs.tla",
        "SumeragiV2DurableDecisionRecoveryProofs.tla",
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2AsyncNetwork.tla",
    )
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    network_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    async_path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    reducer_path = (
        tmp_path
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_core"
        / "reducer.rs"
    )
    canonical_network = network_path.read_text(encoding="utf-8")
    canonical_async = async_path.read_text(encoding="utf-8")
    canonical_reducer = reducer_path.read_text(encoding="utf-8")
    assert module._progress_witness_source_fidelity_errors(formal_dir) == []

    network_mutations = (
        (
            "AsyncRecoveryVars",
            ", asyncHistoricalLockRestartAuthorities",
            "",
            "AsyncRecoveryVars must equal only the exact durable-source projection",
        ),
        (
            "AsyncHistoricalLockRestartAuthority",
            "context |-> qc.context",
            "context |-> context",
            "AsyncHistoricalLockRestartAuthority must equal only the exact",
        ),
        (
            "HistoricalLockRestartAuthoritySourceKernel",
            "authority.node, qc)",
            "0, qc)",
            "HistoricalLockRestartAuthoritySourceKernel must equal only the exact",
        ),
        (
            "HistoricalLockRestartAuthoritySourceKernel",
            "qc.context = currentContext",
            "qc.context = context",
            "HistoricalLockRestartAuthoritySourceKernel must equal only the exact",
        ),
        (
            "HistoricalLockRestartAuthoritySourceKernel",
            "qc.view = currentLockRank[authority.node]",
            "qc.view <= currentLockRank[authority.node]",
            "HistoricalLockRestartAuthoritySourceKernel must equal only the exact",
        ),
        (
            "HistoricalLockRestartAuthoritySourceKernel",
            "qc.subject = currentLockSubject[authority.node]",
            "qc.subject # currentLockSubject[authority.node]",
            "HistoricalLockRestartAuthoritySourceKernel must equal only the exact",
        ),
        (
            "AsyncHistoricalLockRestartAuthorityTransition",
            "/\\ ~HistoricalLockRestartExactCurrentFetchOwnerAfter(authority)",
            "/\\ TRUE",
            "AsyncHistoricalLockRestartAuthorityTransition must equal only the exact",
        ),
        (
            "HistoricalLockRestartExactCurrentFetchKernel",
            'candidate.kind = "FetchBody"',
            'candidate.kind = "StoreBody"',
            "HistoricalLockRestartExactCurrentFetchKernel must equal only the exact",
        ),
        (
            "HistoricalLockRestartExactCurrentFetchKernel",
            "currentGeneration[authority.node]",
            "currentGeneration[0]",
            "HistoricalLockRestartExactCurrentFetchKernel must equal only the exact",
        ),
        (
            "AsyncNext",
            "/\\ AsyncHistoricalLockRestartAuthorityTransition",
            "/\\ UNCHANGED asyncHistoricalLockRestartAuthorities",
            "AsyncNext omits the historical-lock restart authority frame",
        ),
    )
    for symbol, old, new, expected_error in network_mutations:
        network_path.write_text(
            mutate_tla_operator(canonical_network, symbol, old, new),
            encoding="utf-8",
        )
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            symbol,
            expected_error,
            errors,
        )
        network_path.write_text(canonical_network, encoding="utf-8")

    async_mutations = (
        (
            "HistoricalLockedBodyRecoveryStage",
            "  \\/ HistoricalLockedBodyRestartAuthority(node, qc)\n",
            "",
            "HistoricalLockedBodyRecoveryStage must equal only the exact",
        ),
        (
            "HistoricalLockRestartAuthoritySourceRetentionInvariant",
            "HistoricalLockRestartAuthoritySource(authority)",
            "TRUE",
            "HistoricalLockRestartAuthoritySourceRetentionInvariant must equal only",
        ),
        (
            "AsyncStrongTypeInvariant",
            "  /\\ HistoricalLockRestartAuthoritySourceRetentionInvariant\n",
            "",
            "AsyncStrongTypeInvariant omits exact historical-lock restart source retention",
        ),
        (
            "HistoricalLockedSemanticPrepareAuthority",
            "authorityQc.context = qc.context",
            "authorityQc.context = context",
            "HistoricalLockedSemanticPrepareAuthority must equal only the exact",
        ),
        (
            "HistoricalLockedCertifiedRequestMatches",
            "request.envelope.recipient\n            \\in authorityQc.signers \\ {node}",
            "request.envelope.recipient \\in qc.signers \\ {node}",
            "HistoricalLockedCertifiedRequestMatches must equal only the exact",
        ),
        (
            "HistoricalLockedBodyServeOwned",
            "SequenceSet(asyncIoQueues[server])",
            "SequenceSet(asyncIoQueues[node])",
            "HistoricalLockedBodyServeOwned must equal only the exact",
        ),
        (
            "HistoricalLockedBodyRecoveryTerminal",
            "     \\/ ~HistoricalLockedPrepareForCommit(node, qc)",
            "     \\/ TRUE",
            "HistoricalLockedBodyRecoveryTerminal must equal only the exact",
        ),
        (
            "HistoricalLockedBodyRuntimeExecutes",
            "           /\\ CommandDispatchable(candidate)",
            "           /\\ TRUE",
            "HistoricalLockedBodyRuntimeExecutes must equal only the exact",
        ),
    )
    for symbol, old, new, expected_error in async_mutations:
        async_path.write_text(
            mutate_tla_operator(canonical_async, symbol, old, new),
            encoding="utf-8",
        )
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            symbol,
            expected_error,
            errors,
        )
        async_path.write_text(canonical_async, encoding="utf-8")

    async_theorem_mutations = (
        (
            "HistoricalLockedFetchExecutionHandsOff",
            "HistoricalLockedBodyValidateOwned(node, qc)'",
            "TRUE",
            "HistoricalLockedFetchExecutionHandsOff must state only the exact",
        ),
        (
            "HistoricalLockedBodyExistingSourceStepPreservation",
            "HistoricalLockedStoreExecutionHandsOff",
            "TRUE",
            "HistoricalLockedBodyExistingSourceStepPreservation must retain the exact non-vacuous",
        ),
        (
            "AsyncBracketPreservesHistoricalLockedBodyRecoveryStage",
            "HistoricalLockedBodyNewSourceStepEstablishment",
            "TRUE",
            "AsyncBracketPreservesHistoricalLockedBodyRecoveryStage must retain the exact non-vacuous",
        ),
        (
            "AsyncSpecAlwaysHistoricalLockedBodyRecoveryStage",
            "AsyncBracketPreservesHistoricalLockedBodyRecoveryStage",
            "TRUE",
            "AsyncSpecAlwaysHistoricalLockedBodyRecoveryStage must retain the exact non-vacuous",
        ),
    )
    for symbol, old, new, expected_error in async_theorem_mutations:
        async_path.write_text(
            mutate_tla_theorem(canonical_async, symbol, old, new),
            encoding="utf-8",
        )
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            symbol,
            expected_error,
            errors,
        )
        async_path.write_text(canonical_async, encoding="utf-8")

    reducer_mutations = (
        (
            "if let Some(certificate) = durable.locked() {",
            "if let Some(certificate) = durable.highest_prepare() {",
            "recovery must retain the exact pre-existing durable locked QC",
        ),
        (
            "effects.push(self.ensure_body_fetch(&locked));",
            "effects.push(self.ensure_body_fetch(&decision));",
            "retransmit must derive FetchBody from the exact durable lock",
        ),
        (
            "self.replay_resumed = true;",
            "self.replay_resumed = true;\n        let _ = self.durable.locked();",
            "must not invent a special crash-time historical-lock owner",
        ),
    )
    for old, new, expected_error in reducer_mutations:
        assert old in canonical_reducer, old
        reducer_path.write_text(
            canonical_reducer.replace(old, new, 1), encoding="utf-8"
        )
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        reducer_path.write_text(canonical_reducer, encoding="utf-8")


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
            "AsyncIngressCapacity >= 4 * N + 2",
            "AsyncIngressCapacity >= N + 2",
            1,
        ).replace(
            "Len(lanes[recipient][source]) = 3",
            "Len(lanes[recipient][source]) = 4",
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
        source.replace("  AsyncIngressCapacity = 6\n", "  AsyncIngressCapacity = 5\n", 1),
        encoding="utf-8",
    )
    errors = module._ownership_n1_configuration_errors(formal_dir)
    assert any("exact 4 * N + 2 geometry (6)" in error for error in errors)

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
    assert any("exact 4 * N + 2 geometry (6)" in error for error in errors)
    assert any("exact 2 * N + 3 geometry (5)" in error for error in errors)

    path.write_text(
        source.replace(
            "  ProductionSchedulerTraceRefinesProtectedOwnership = TRUE\n",
            "",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._ownership_n1_configuration_errors(formal_dir)
    assert any(
        "must assign ProductionSchedulerTraceRefinesProtectedOwnership = TRUE "
        "exactly once" in error
        for error in errors
    )

    for refinement_constant in (
        "ProductionIngressIdentityAndClassTraceRefinesProtectedOwnership",
        "ProductionTwoStageRelayRetryTraceRefinesSourceFairness",
        "ProductionReliableFlushTraceRefinesOutboundOwnership",
    ):
        path.write_text(
            source.replace(
                f"  {refinement_constant} = TRUE\n",
                "",
                1,
            ),
            encoding="utf-8",
        )
        errors = module._ownership_n1_configuration_errors(formal_dir)
        assert any(
            f"must assign {refinement_constant} = TRUE exactly once" in error
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
        Path("crates/iroha_core/src/sumeragi/v2_runtime.rs"),
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
            "             \\/ TimeoutVoteSlotOccupied(envelope.recipient, envelope.vote)\n",
            "",
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
        Path("crates/iroha_core/src/sumeragi/v2_runtime.rs"),
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


def test_async_source_fidelity_rejects_post_gst_responsive_crash(
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
        mutate_tla_operator(
            source,
            "PreGstResponsiveCrash",
            "  /\\ ~gst\n",
            "",
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        "PreGstResponsiveCrash omits required production behavior" in error
        and "~gst" in error
        for error in errors
    ), errors


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
            "/\\ UNCHANGED AsyncRecoveryControlVars",
            "/\\ UNCHANGED AsyncRecoveryVars",
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


def local_runner_service_fixture(tmp_path: Path, module) -> Path:
    """Copy the exact formal and Rust sources owned by the runner contract."""

    return copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2AsyncLivenessProofs.tla",
    )


def test_local_runner_service_contract_source_fidelity_is_current(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = local_runner_service_fixture(tmp_path, module)

    errors = module._local_runner_service_contract_source_fidelity_errors(
        module.load_ledger(),
        repo_root=tmp_path,
        formal_dir=formal_dir,
    )

    assert errors == []


def test_local_runner_service_contract_rejects_broadened_trust_boundary(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = local_runner_service_fixture(tmp_path, module)
    ledger = copy.deepcopy(module.load_ledger())
    runtime = next(
        entry
        for entry in ledger["obligations"]
        if entry["id"] == "runtime-after-gst"
    )
    runtime["requirement"] = "After GST some runner eventually executes"

    errors = module._local_runner_service_contract_source_fidelity_errors(
        ledger,
        repo_root=tmp_path,
        formal_dir=formal_dir,
    )

    assert any(
        "exact per-validator local runner/service trusted contract" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("filename", "symbol", "old", "new", "expected_error"),
    (
        (
            "SumeragiV2AsyncNetwork.tla",
            "LocalRunnerServiceOwners",
            "AsyncCurrentResponsiveVoters \\cup asyncHistoricalRecoveryTargets",
            "ValidatorIds",
            "LocalRunnerServiceOwners must equal only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "LocalRunnerServiceContractDebt",
            "  IF node \\in LocalRunnerServiceOwners\n"
            "       /\\ asyncNodeServiceDeadlines[node] <= asyncNow\n",
            "  IF asyncNodeServiceDeadlines[node] <= asyncNow\n",
            "LocalRunnerServiceContractDebt must equal only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "LocalRunnerServiceContractDecreaseStep",
            "  \\E node \\in LocalRunnerServiceOwners:\n",
            "  \\E node \\in ValidatorIds:\n",
            "LocalRunnerServiceContractDecreaseStep must equal only",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "AsyncTickEnabled",
            "     /\\ \\A node \\in LocalRunnerServiceOwners:\n",
            "     /\\ \\A node \\in AsyncCurrentResponsiveVoters:\n",
            "AsyncTickEnabled must project each independent local runner contract",
        ),
    ),
)
def test_local_runner_service_contract_rejects_formal_owner_mutations(
    tmp_path: Path,
    filename: str,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = local_runner_service_fixture(tmp_path, module)
    path = formal_dir / filename
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._local_runner_service_contract_source_fidelity_errors(
        module.load_ledger(),
        repo_root=tmp_path,
        formal_dir=formal_dir,
    )

    assert any(expected_error in error for error in errors), errors


def test_local_runner_service_contract_rejects_disconnected_deadlock_obligation(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = local_runner_service_fixture(tmp_path, module)
    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            "DeadlockFreedomObligation",
            "    DeadlockFreedomWithLocalWorkProperty(AsyncSpecAt(initialContext),\n"
            "      AsyncTerminatingLocalWorkDecreaseStep)\n",
            "    DeadlockFreedomProperty(AsyncSpecAt(initialContext))\n",
        ),
        encoding="utf-8",
    )

    errors = module._local_runner_service_contract_source_fidelity_errors(
        module.load_ledger(),
        repo_root=tmp_path,
        formal_dir=formal_dir,
    )

    assert any(
        "DeadlockFreedomObligation must bind the exact per-validator" in error
        for error in errors
    ), errors
    architecture_errors = module._proof_obligation_architecture_errors(
        module.load_ledger()["obligations"],
        {"SumeragiV2AsyncLivenessProofs": path.read_text(encoding="utf-8")},
    )
    assert any(
        "DeadlockFreedomObligation must state only" in error
        for error in architecture_errors
    ), architecture_errors


@pytest.mark.parametrize(
    ("relative", "old", "new", "expected_error"),
    (
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "            liveness_watchdog.poll(Instant::now());\n",
            "",
            "every serialized height-loop iteration must poll",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "                    let _ = wake_rx.recv_timeout(IDLE_POLL);\n"
            "                    continue;\n",
            "                    continue;\n",
            "all four serialized height-loop continue edges",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "    for _ in 0..limit.max(1) {\n"
            "        match executor.step(Instant::now(), services)? {",
            "    loop {\n"
            "        match executor.step(Instant::now(), services)? {",
            "ordinary serialized runtime service must be a finite configured turn",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "        while attempts < MAX_COMPLETION_DRAIN_BATCH {\n",
            "        loop {\n",
            "completion service must terminate its local scan",
        ),
    ),
)
def test_local_runner_service_contract_rejects_production_loop_mutations(
    tmp_path: Path,
    relative: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = local_runner_service_fixture(tmp_path, module)
    path = tmp_path / relative
    source = path.read_text(encoding="utf-8")
    assert old in source, old
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._local_runner_service_contract_source_fidelity_errors(
        module.load_ledger(),
        repo_root=tmp_path,
        formal_dir=formal_dir,
    )

    assert any(expected_error in error for error in errors), errors


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
        Path("crates/iroha_core/src/sumeragi/v2_runtime.rs"),
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
            "AsyncFaultStepKeepsTimeoutPool": (
                "InjectUntrustedTransportCompletion",
            ),
            "AsyncFaultStepPreservesSchedulerType": (
                "InjectUntrustedTransportCompletionPreservesSchedulerType",
            ),
            "AsyncFaultStepLeavesDiscoveryClock": (
                "InjectUntrustedTransportCompletion",
            ),
            "AsyncFaultPreservesProgressOwnership": (
                "InjectUntrustedTransportCompletion",
            ),
            "AsyncFaultStepLeavesProgressCarriers": (
                "InjectUntrustedTransportCompletion",
            ),
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
        Path("crates/iroha_core/src/sumeragi/v2_runtime.rs"),
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
        "asyncDeferredHandoffs",
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
        "asyncHistoricalLockRestartAuthorities",
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
        "  /\\ Len(indexedAsyncState[initialContext][2]) = 35\n"
        "  /\\ Len(indexedAsyncState[initialContext][3]) = 5\n"
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
        "  /\\ UNCHANGED IndexedScheduler(initialContext, 26)\n"
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
            "Len(indexedAsyncState[initialContext][2]) = 35",
            "Len(indexedAsyncState[initialContext][2]) = 34",
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
            "Len(indexedAsyncState[initialContext][3]) = 5",
            "Len(indexedAsyncState[initialContext][3]) = 4",
            1,
        )
        .replace(
            "Len(indexedAsyncState[initialContext][1]) = 46",
            "Len(indexedAsyncState[initialContext][1]) = 45",
            1,
        )
        .replace(
            "UNCHANGED IndexedScheduler(initialContext, 26)",
            "UNCHANGED IndexedScheduler(initialContext, 25)",
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
    assert any("preserve scheduler slot 26" in error for error in errors)

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
            "asyncHistoricalLockRestartAuthorities <- VerificationRecovery(5)",
            "asyncHistoricalLockRestartAuthorities <- VerificationRecovery(4)",
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
            "asyncHistoricalLockRestartAuthorities <-\n"
            "         IndexedRecovery(initialContext, 5)",
            "asyncHistoricalLockRestartAuthorities <-\n"
            "         IndexedRecovery(initialContext, 4)",
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
            "    asyncRecoveryReplayQueue, asyncHistoricalLockRestartAuthorities>>",
            "<<asyncRecoveryPhase, asyncRecoveryGeneration, asyncRecoveryNode,\n"
            "    asyncRecoveryReplayQueue, asyncHistoricalLockRestartAuthorities>>",
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
    ("symbol", "old", "new"),
    (
        (
            "CompleteTipRecoveryAuthorityRecord",
            'kind |-> "CompleteTip"',
            'kind |-> "SnapshotBootstrap"',
        ),
        (
            "SnapshotBootstrapRecoveryAuthorityRecord",
            'kind |-> "SnapshotBootstrap"',
            'kind |-> "CompleteTip"',
        ),
        (
            "ExactCompleteTipRecoveryAuthority",
            "CompleteTipRecoveryAuthorityRecord(",
            "SnapshotBootstrapRecoveryAuthorityRecord(",
        ),
        (
            "LatchAppliedSuccessorStartupFailure",
            'successorActivationStatus[parentContext][node] = "Running"',
            'successorActivationStatus[parentContext][node] = "Queued"',
        ),
        (
            "LatchRecoveredSuccessorStartupFailure",
            "owner \\notin successorActivationFailures",
            "owner \\notin successorActivationFailureHistory",
        ),
        (
            "RehydrateCleanCompleteTipSuccessorStartup",
            "ExactDurableParentApplication(parentContext, node, application)",
            "TRUE",
        ),
        (
            "RehydrateFailedSuccessorStartup",
            "successorActivationFailures \\ {owner}",
            "successorActivationFailures",
        ),
        (
            "AuthenticateRecoveredSuccessorActivation",
            "authority \\in successorRecoveryAuthorities",
            "authority \\notin successorRecoveryAuthorities",
        ),
        (
            "EventualFailureFreeSuccessorStartupSuffix",
            "successorActivationFailures",
            "successorActivationFailureHistory",
        ),
        (
            "IndexedChainSpec",
            "  /\\ EventualFailureFreeSuccessorStartupSuffix\n",
            "",
        ),
    ),
)
def test_chain_successor_lifecycle_and_authority_mutations_fail_closed(
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
        mutate_tla_operator(source, symbol, old, new), encoding="utf-8"
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(symbol in error for error in errors), errors


def test_chain_rejects_snapshot_as_complete_tip_authority(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            "SnapshotBootstrapAuthorityIsDistinctFromCompleteTipAuthority",
            "      # CompleteTipRecoveryAuthorityRecord(",
            "      = CompleteTipRecoveryAuthorityRecord(",
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "SnapshotBootstrapAuthorityIsDistinctFromCompleteTipAuthority must state only"
        in error
        for error in errors
    ), errors


def test_chain_rejects_production_terminal_height_claim(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        source.replace(
            "CONSTANT VerificationContext\n",
            "CONSTANT VerificationContext\n"
            "CONSTANT ProductionTerminalApplicationExcludesActivation\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "production terminal claim/kernel" in error for error in errors
    ), errors


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
            "MutationLatchAppliedSuccessorStartupFailure",
            "  /\\ activationTokens' = {}\n",
            "  /\\ UNCHANGED activationTokens\n",
        ),
        (
            "StaleAppliedTokenState",
            "  /\\ activationFailurePresent = FALSE\n",
            "  /\\ ~activationFailurePresent\n",
        ),
        (
            "MutationLatchAppliedSuccessorStartupFailure",
            "  /\\ activationFailurePresent' = TRUE\n",
            "  /\\ activationFailurePresent'\n",
        ),
        (
            "AppliedFailurePreservesRunningWitness",
            '    => activationStatus = "Running"',
            '    => activationStatus = "Queued"',
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
            "INVARIANT AppliedFailurePreservesRunningWitness\n",
        ),
        (
            "successor_stale_token_fixed.cfg",
            "CHECK_DEADLOCK FALSE\n",
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


def test_async_historical_recovery_child_source_fidelity() -> None:
    module = load_checker()

    assert (
        module._async_historical_recovery_source_fidelity_errors(
            module.FORMAL_DIR
        )
        == []
    )


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "HistoricalRecoveryTargetDecisionProgressProperty",
            "         (gst /\\ HistoricalRecoveryTarget(node))\n",
            "         HistoricalRecoveryTarget(node)\n",
        ),
        (
            "HistoricalRecoveryTargetDecisionProgressProperty",
            "    => \\A node \\in Responsive:\n",
            "    => \\A node \\in AsyncCurrentResponsiveVoters:\n",
        ),
        (
            "ResponsiveDecisionApplicationProgressProperty",
            "         (gst /\\ NodeHasDecision(node))\n",
            "         NodeHasDecision(node)\n",
        ),
        (
            "ResponsiveDecisionApplicationProgressProperty",
            "    => \\A node \\in Responsive:\n",
            "    => \\A node \\in AsyncCurrentResponsiveVoters:\n",
        ),
        (
            "HistoricalProtectedCandidateOwned",
            "  /\\ HistoricalRecoveryTarget(candidate.node)\n",
            "  /\\ candidate.node \\in AsyncCurrentResponsiveVoters\n",
        ),
        (
            "HistoricalProtectedStage2RankProgressProperty",
            "  HistoricalProtectedStageRankProgressProperty(specification, 2)",
            "  HistoricalProtectedStageRankProgressProperty(specification, 3)",
        ),
        (
            "HistoricalProtectedServiceRankLeafProperties",
            "  /\\ HistoricalProtectedStage4RankProgressProperty(specification)\n",
            "  /\\ TRUE\n",
        ),
        (
            "HistoricalCommitCertificateDiscoveryPersistenceObligation",
            "         \\/ HistoricalCommitCertificateDiscoveryOutcome(node)'",
            "         \\/ HistoricalCommitCertificateDiscoveryPending(node)'",
        ),
        (
            "HistoricalCommitCertificateDiscoveryPersistenceUnless",
            "           \\/ HistoricalCommitCertificateDiscoveryOutcome(node)'",
            "           \\/ HistoricalCommitCertificateDiscoveryPending(node)'",
        ),
        (
            "HistoricalCommitCertificateDiscoveryPersistenceProperty",
            "HistoricalCommitCertificateDiscoveryPersistenceUnless(node)",
            "HistoricalCommitCertificateDiscoveryPersistenceObligation",
        ),
        (
            "HistoricalRecoveryTargetRemoteServerInvariant",
            "      => CommitCertificateRequestOutbox(node) # {}",
            "      => TRUE",
        ),
        (
            "HistoricalCommitCertificateDiscoveryClockProgressProperty",
            "                         \\/ asyncNow >= AsyncRoundTimeout)",
            "                         \\/ FALSE)",
        ),
        (
            "HistoricalActiveRequestRetransmissionProgressLeaf",
            "           /\\ HistoricalRecoveryTarget(node)\n",
            "           /\\ node \\in AsyncCurrentResponsiveVoters\n",
        ),
        (
            "HistoricalCommitRequestServeProgressLeaf",
            "  StarvationFreedomProperty(specification)\n",
            "  TRUE\n",
        ),
        (
            "HistoricalCommitResponseAdmissionProgressLeaf",
            "                     node, \"DeliverQC\"))",
            "                     node, \"BeginDecision\"))",
        ),
        (
            "HistoricalCommitDeliveryProgressLeaf",
            "  HistoricalProtectedCandidateStarvationProperty(specification)\n",
            "  TRUE\n",
        ),
        (
            "HistoricalDecisionFrontierAvailabilityProperty",
            "           => HistoricalDecisionRecoveryFrontier(node)",
            "           => TRUE",
        ),
        (
            "HistoricalDecisionCertifiedResponseProgressLeaf",
            "   /\\ HistoricalProtectedCandidateStarvationProperty(specification))\n",
            "   /\\ TRUE)\n",
        ),
        (
            "HistoricalDecisionApplyProgressLeaf",
            "                 ~> NodeHasApplication(node))",
            "                 ~> TRUE)",
        ),
        (
            "ResponsiveDecisionServiceOwnershipInvariant",
            "         \\/ HistoricalRecoveryTarget(node)",
            "         \\/ node \\in AsyncCurrentResponsiveVoters",
        ),
        (
            "HistoricalRecoveryAsyncTemporalClosurePremises",
            "  /\\ HistoricalCommitCertificateDiscoveryPersistenceProperty(specification)\n",
            "  /\\ HistoricalCommitCertificateDiscoveryPersistenceObligation\n",
        ),
        (
            "HistoricalRecoveryAsyncTemporalClosurePremises",
            "  /\\ HistoricalRecoveryTargetRemoteServerProperty(specification)\n",
            "  /\\ TRUE\n",
        ),
        (
            "HistoricalRecoveryAsyncTemporalClosurePremises",
            "  /\\ HistoricalCommitCertificateDiscoveryClockProgressProperty(specification)\n",
            "  /\\ TRUE\n",
        ),
        (
            "HistoricalRecoveryAsyncTemporalClosurePremises",
            "  /\\ HistoricalProtectedServiceRankLeafProperties(specification)\n",
            "  /\\ TRUE\n",
        ),
        (
            "HistoricalRecoveryAsyncTemporalClosurePremises",
            "  /\\ HistoricalCommitCertificateConcreteLeafProperties(specification)\n",
            "  /\\ TRUE\n",
        ),
        (
            "HistoricalRecoveryAsyncTemporalClosurePremises",
            "  /\\ HistoricalDecisionFrontierAvailabilityProperty(specification)\n",
            "  /\\ TRUE\n",
        ),
        (
            "HistoricalRecoveryAsyncTemporalClosurePremises",
            "  /\\ HistoricalDecisionConcreteLeafProperties(specification)\n",
            "  /\\ TRUE\n",
        ),
        (
            "HistoricalLockedBodyRecoveryOutcome",
            "  \\/ HistoricalLockedBodyRecoveryTerminal(node, qc)",
            "  \\/ TRUE",
        ),
        (
            "HistoricalLockedActiveRequestProgressLeaf",
            "                \\/ HistoricalLockedBodyCertifiedFetchOwned(node, qc))",
            "                \\/ TRUE)",
        ),
        (
            "HistoricalLockedBodyRecoveryConeLeafProperties",
            "  /\\ HistoricalLockedActiveRequestProgressLeaf(specification)\n",
            "  /\\ TRUE\n",
        ),
        (
            "HistoricalLockedBodyRecoveryConeProperty",
            "           ~> HistoricalLockedBodyRecoveryOutcome(node, qc)",
            "           ~> TRUE",
        ),
    ),
)
def test_async_historical_recovery_operator_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2AsyncHistoricalRecoveryLivenessProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new), encoding="utf-8"
    )

    errors = module._async_historical_recovery_source_fidelity_errors(formal_dir)

    assert any(f"{symbol} must equal only" in error for error in errors), errors


@pytest.mark.parametrize(
    ("symbol", "proof_token"),
    (
        (
            "HistoricalProtectedServiceRankProgressFromStageLeaves",
            "HistoricalProtectedStage4RankProgressProperty",
        ),
        (
            "HistoricalProtectedServiceRankProgressFromStageLeaves",
            "HistoricalProtectedStage5RankProgressProperty",
        ),
        (
            "HistoricalProtectedServiceRankProgressImpliesStarvation",
            "WellFoundedLeadsTo",
        ),
        (
            "HistoricalCommitCertificateDiscoveryReadinessFromClock",
            "DEF HistoricalCommitCertificateDiscoveryClockProgressProperty",
        ),
        (
            "FairHistoricalCommitCertificateDiscoveryFromPersistence",
            "WF_AsyncAllVars(",
        ),
        (
            "FairHistoricalCommitCertificateDiscoveryFromPersistence",
            "HistoricalCommitCertificateDiscoveryPersistenceUnless",
        ),
        (
            "HistoricalActiveCommitCertificateRequestReachesDecision",
            "HistoricalCommitResponseAdmissionProgressLeaf",
        ),
        (
            "HistoricalActiveCommitCertificateRequestReachesDecision",
            "HistoricalPersistDecisionProgressLeaf",
        ),
        (
            "HistoricalTargetDecisionReachesApplicationFromConcreteLeaves",
            "HistoricalDecisionValidateProgressLeaf",
        ),
        (
            "HistoricalTargetDecisionReachesApplicationFromConcreteLeaves",
            "HistoricalDecisionApplyProgressLeaf",
        ),
        (
            "HistoricalRecoveryTargetDecisionFromExactCorridor",
            "HistoricalActiveCommitCertificateRequestReachesDecision",
        ),
        (
            "ResponsiveDecisionApplicationFromExactCorridor",
            "ResponsiveDecisionServiceOwnershipProperty",
        ),
        (
            "HistoricalRecoveryAsyncTemporalPrerequisitesFromExactCorridor",
            "ResponsiveDecisionApplicationFromExactCorridor",
        ),
        (
            "HistoricalRecoveryAsyncTemporalPrerequisitesFromExactCorridor",
            "HistoricalRecoveryTargetDecisionFromExactCorridor",
        ),
        (
            "HistoricalLockedBodyRecoveryConeComposesFromExactLeaves",
            "AsyncSpecAlwaysHistoricalLockedBodyRecoveryStage",
        ),
        (
            "HistoricalLockedBodyRecoveryConeComposesFromExactLeaves",
            "HistoricalLockedActiveRequestProgressLeaf",
        ),
        (
            "HistoricalLockedBodyRecoveryConeComposesFromExactLeaves",
            "HistoricalLockedValidateRecoveryProgressLeaf",
        ),
    ),
)
def test_async_historical_recovery_rejects_disconnected_proofs(
    tmp_path: Path,
    symbol: str,
    proof_token: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2AsyncHistoricalRecoveryLivenessProofs.tla"
    source = path.read_text(encoding="utf-8")
    extracted = module._top_level_theorem_body(
        source, symbol, preserve_string_contents=True
    )
    assert extracted is not None
    body, _ = extracted
    assert proof_token in body
    path.write_text(
        mutate_tla_theorem(source, symbol, proof_token, "TRUE"),
        encoding="utf-8",
    )

    errors = module._async_historical_recovery_source_fidelity_errors(formal_dir)

    assert any(
        f"{symbol} proof must retain exact historical dependencies" in error
        for error in errors
    ), errors


def test_async_historical_recovery_rejects_constants_and_endpoint_theorems(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2AsyncHistoricalRecoveryLivenessProofs.tla"
    source = path.read_text(encoding="utf-8")
    source = source.replace(
        "EXTENDS SumeragiV2AsyncLivenessProofs, TLAPS\n",
        "EXTENDS SumeragiV2AsyncLivenessProofs, TLAPS\n"
        "CONSTANT HistoricalRecoveryOracle\n",
        1,
    )
    source = source.replace(
        "HistoricalRecoveryTargetDecisionProgressProperty(specification) ==\n",
        "THEOREM HistoricalRecoveryTargetDecisionProgressProperty(specification) ==\n",
        1,
    )
    path.write_text(source, encoding="utf-8")

    errors = module._async_historical_recovery_source_fidelity_errors(formal_dir)

    assert any("unconstrained constants" in error for error in errors), errors
    assert any(
        "must remain an operator property" in error for error in errors
    ), errors


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
            "HistoricalRecoveryProgressEligible",
            "  /\\ \\/ IndexedHistoricalRecoveryReady(initialContext, node)\n",
            "  /\\ \\/ FALSE\n",
            "HistoricalRecoveryProgressEligible must equal only",
        ),
        (
            "HistoricalRecoveryProgressEligible",
            "     \\/ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)\n",
            "",
            "HistoricalRecoveryProgressEligible must equal only",
        ),
        (
            "HistoricalRecoveryProgressEligible",
            "     \\/ IndexedAsync(initialContext)!NodeHasDecision(node)",
            "",
            "HistoricalRecoveryProgressEligible must equal only",
        ),
        (
            "IndexedExactHistoricalRecoveryProgress",
            "     node \\in Responsive:\n",
            "     node \\in IndexedAsync(initialContext)!AsyncVotersAt(initialContext):\n",
            "IndexedExactHistoricalRecoveryProgress must equal only",
        ),
        (
            "IndexedExactHistoricalRecoveryProgress",
            "    HistoricalRecoveryOutstanding(initialContext, node)\n",
            "    HistoricalRecoveryProgressEligible(initialContext, node)\n",
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
    ("symbol", "old", "new"),
    (
        (
            "IndexedHistoricalRecoveryTargetDecisionProgress",
            "     node \\in Responsive:\n",
            "     node \\in IndexedAsync(initialContext)!AsyncVotersAt(initialContext):\n",
        ),
        (
            "IndexedResponsiveDecisionApplicationProgress",
            "     node \\in Responsive:\n",
            "     node \\in IndexedAsync(initialContext)!AsyncVotersAt(initialContext):\n",
        ),
        (
            "IndexedHistoricalRecoveryAsyncTemporalPrerequisites",
            "  /\\ IndexedHistoricalRecoveryTargetDecisionProgress\n",
            "  /\\ TRUE\n",
        ),
        (
            "IndexedHistoricalRecoveryAsyncTemporalPrerequisites",
            "  /\\ IndexedResponsiveDecisionApplicationProgress",
            "  /\\ TRUE",
        ),
        (
            "IndexedHistoricalRecoveryEligibilityProgress",
            "    HistoricalRecoveryOutstanding(initialContext, node)\n",
            "    HistoricalRecoveryProgressEligible(initialContext, node)\n",
        ),
        (
            "IndexedHistoricalRecoveryTemporalPrerequisites",
            "  /\\ IndexedHistoricalRecoveryEligibilityProgress\n",
            "  /\\ TRUE\n",
        ),
    ),
)
def test_chain_temporal_prerequisite_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainLivenessProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new), encoding="utf-8"
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(f"{symbol} must equal only" in error for error in errors), errors


@pytest.mark.parametrize(
    ("symbol", "proof_token"),
    (
        (
            "IndexedChainSpecEventuallyOpensReadyHistoricalRecovery",
            "IndexedHistoricalRecoveryReadyEnablesExactOpen",
        ),
        (
            "IndexedExactHistoricalRecoveryFromAsyncTemporalPrerequisites",
            "IndexedHistoricalRecoveryEligibilityProgress",
        ),
        (
            "IndexedExactHistoricalRecoveryFromAsyncTemporalPrerequisites",
            "IndexedHistoricalRecoveryTargetDecisionProgress",
        ),
        (
            "IndexedExactHistoricalRecoveryFromAsyncTemporalPrerequisites",
            "IndexedResponsiveDecisionApplicationProgress",
        ),
        (
            "IndexedSuccessorActivationProgressFromStarvationProof",
            "SuccessorActivationStarvationMatchesChainProgress",
        ),
        (
            "IndexedHeightLivenessFromAsyncHistoricalRecoveryAndSuccessorProofs",
            "IndexedSuccessorActivationProgressFromStarvationProof",
        ),
    ),
)
def test_chain_temporal_composition_rejects_disconnected_proofs(
    tmp_path: Path,
    symbol: str,
    proof_token: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainLivenessProofs.tla"
    source = path.read_text(encoding="utf-8")
    extracted = module._top_level_theorem_body(
        source, symbol, preserve_string_contents=True
    )
    assert extracted is not None
    body, _ = extracted
    assert proof_token in body
    path.write_text(
        mutate_tla_theorem(source, symbol, proof_token, "TRUE"),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        f"{symbol} proof must retain exact temporal dependencies" in error
        for error in errors
    ), errors


def test_chain_temporal_composition_requires_successor_child_direction(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainLivenessProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        source.replace(
            "EXTENDS SumeragiV2SuccessorActivationRefinementProofs, TLAPS",
            "EXTENDS SumeragiV2ChainEpochRefinement, TLAPS",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "chain temporal composition must extend exactly" in error
        for error in errors
    ), errors


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
        "ProductionStartupFailureAndRestartRefinesIndexedLifecycle",
        "ProductionHistoricalCertificateTraceRefinesIndexedAsync",
        "ProductionHistoricalBodyPipelineTraceRefinesIndexedAsync",
        "ProductionTerminalApplicationWithoutSuccessorActivationTraceRefinesIndexedTerminal",
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
            "ProductionStartupFailureAndRestartRefinesIndexedLifecycle",
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


def test_chain_production_refinement_rejects_abstract_only_operator(
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
        mutate_tla_operator(
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


def test_chain_production_refinement_rejects_theorem_and_tautological_bridges(
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
    header = f"{symbol} ==\n"
    assert source.count(header) == 1
    path.write_text(
        source.replace(header, f"THEOREM {header}", 1),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "must be one operator, not a proofless theorem" in error
        for error in errors
    ), errors

    bridge_consequent = (
        "    => SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation\n"
    )
    assert source.count(bridge_consequent) == 1
    path.write_text(
        source.replace(
            bridge_consequent,
            "    => ProductionSuccessorAndExactRecoveryTraceRefinement\n",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any(
        "cross-tool bridge must state only" in error for error in errors
    ), errors

    bridge_proof = (
        "  BY IndexedChainSpecEstablishesSuccessorActivationAndExactHistoricalRecoveryInvariant\n"
        "     DEF SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation\n"
    )
    assert source.count(bridge_proof) == 1
    path.write_text(
        source.replace(
            bridge_proof,
            "  BY TRUE\n"
            "     DEF SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation\n",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any(
        "cross-tool bridge must retain reviewed non-tautological proof" in error
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
            "0..21",
            "0..22",
        ),
        (
            "SuccessorActivationPipelineDistance",
            '  IN CASE successorActivationStatus[parentContext][node] = "Queued" -> 10',
            '  IN CASE successorActivationStatus[parentContext][node] = "Queued" -> 11',
        ),
        (
            "SuccessorActivationRank",
            "  ELSE IF successorPredecessorStatusOwnership[parentContext][node]\n"
            '            = "Published"\n'
            "       THEN 11 + SuccessorActivationPipelineDistance(parentContext, node)\n"
            "       ELSE SuccessorActivationPipelineDistance(parentContext, node)",
            "  ELSE SuccessorActivationPipelineDistance(parentContext, node)",
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
            "        /\\ SuccessorActivationFailureAbsent(parentContext, node)\n",
            "        /\\ TRUE\n",
        ),
        (
            "SuccessorActivationStepDecreasesRankProperty",
            "                   < SuccessorActivationRank(parentContext, node)",
            "                   <= SuccessorActivationRank(parentContext, node)",
        ),
        (
            "SuccessorActivationPendingIsNotOrphanedProperty",
            "           \\/ SuccessorActivationPending(parentContext, node)'",
            "           \\/ TRUE",
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
    ("symbol", "old", "new"),
    (
        (
            "CleanCompleteTipRestartDescendsPublishedTier",
            "            < SuccessorActivationRank(parentContext, node)",
            "            <= SuccessorActivationRank(parentContext, node)",
        ),
        (
            "SuccessorActivationFailureFreeProgressStrictlyDecreasesRank",
            "      /\\ SuccessorActivationFailureAbsent(parentContext, node)'",
            "      /\\ TRUE",
        ),
        (
            "FailureFreeSuccessorActivationRankLeadsToExit",
            "SuccessorActivationFailureFreeProgressExitsCurrentRank",
            "DisconnectedProgressExit",
        ),
        (
            "FailureFreeSuccessorActivationRankConverges",
            "WellFoundedLeadsTo",
            "PTL",
        ),
        (
            "SuccessorActivationTemporalKernelIsSuffixClosed",
            "      => []SuccessorActivationTemporalKernel(parentContext, node)",
            "      => SuccessorActivationTemporalKernel(parentContext, node)",
        ),
        (
            "EventualFailureFreeSuffixLiftsSuccessorConvergence",
            "/\\ <>SuccessorActivationFailureFreeSuffix(parentContext, node)",
            "/\\ SuccessorActivationFailureFreeSuffix(parentContext, node)",
        ),
        (
            "EventualFailureFreeSuffixLiftsSuccessorConvergence",
            "IndexedStepDoesNotOrphanSuccessorActivation",
            "DisconnectedNonOrphaning",
        ),
        (
            "IndexedChainSpecEstablishesSuccessorActivationStarvationFreedom",
            "EventualFailureFreeSuccessorStartupSuffix",
            "UnrelatedFailurePremise",
        ),
    ),
)
def test_successor_activation_failure_free_proof_mutations_fail_closed(
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
        mutate_tla_theorem(source, symbol, old, new), encoding="utf-8"
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


def test_successor_activation_starvation_obligation_rejects_missing_candidate_proof(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2SuccessorActivationRefinementProofs.tla"
    source = path.read_text(encoding="utf-8")
    declaration = source.index(
        "THEOREM SuccessorActivationStarvationFreedomObligation =="
    )
    proof_start = source.index("\nPROOF\n", declaration)
    proof_end = source.index(
        "\nTHEOREM SuccessorActivationStarvationMatchesChainProgress ==",
        proof_start,
    )
    path.write_text(source[:proof_start] + source[proof_end:], encoding="utf-8")

    errors = module._successor_activation_rank_source_fidelity_errors(formal_dir)

    assert any(
        "must retain the explicit candidate TLAPS proof" in error
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
    declaration = source.index(
        "THEOREM SuccessorActivationStarvationFreedomObligation =="
    )
    proof_start = source.index("\nPROOF\n", declaration)
    proof_end = source.index(
        "\nTHEOREM SuccessorActivationStarvationMatchesChainProgress ==",
        proof_start,
    )
    path.write_text(
        source[:proof_start] + "\nOBVIOUS\n" + source[proof_end:],
        encoding="utf-8",
    )

    errors = module._successor_activation_rank_source_fidelity_errors(formal_dir)

    assert any(
        "proof may not use a vacuous assertion" in error for error in errors
    ), errors


def test_successor_activation_starvation_obligation_pins_proof_dependencies(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2SuccessorActivationRefinementProofs.tla"
    source = path.read_text(encoding="utf-8")
    declaration = source.index(
        "THEOREM SuccessorActivationStarvationFreedomObligation =="
    )
    dependency = "IndexedChainSpecEstablishesSuccessorActivationRankProgress"
    position = source.index(dependency, declaration)
    path.write_text(
        source[:position]
        + "DisconnectedRankProgress"
        + source[position + len(dependency) :],
        encoding="utf-8",
    )

    errors = module._successor_activation_rank_source_fidelity_errors(formal_dir)

    assert any(
        f"proof must invoke {dependency} exactly once" in error
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
    assert "deferred_handoff_rebusy_status -eq 13" in runner
    assert "deferred_handoff_rebusy_bug.cfg" in runner
    assert "deferred_handoff_exact.cfg" in runner
    assert "SumeragiV2DeferredHandoffMutation.tla" in runner
    assert "handoff-free deferred retry did not fail with TLC status 13" in runner
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
    handoff_mutation = (
        formal_dir / "SumeragiV2DeferredHandoffMutation.tla"
    ).read_text(encoding="utf-8")
    assert "OldDrain" in handoff_mutation
    assert "HandoffDrain" in handoff_mutation
    assert "HeldTargetEventuallyServed" in handoff_mutation
    assert (formal_dir / "deferred_handoff_rebusy_bug.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION OldSpec\n")
    assert (formal_dir / "deferred_handoff_exact.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION HandoffSpec\n")
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
    assert "983041 states generated, 99328 distinct states found" in progress_runner
    assert "depth of the complete state graph search is 48" in progress_runner

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


def test_deferred_handoff_mutation_fidelity_rejects_semantic_drift(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for filename in (
        "SumeragiV2DeferredHandoffMutation.tla",
        "deferred_handoff_rebusy_bug.cfg",
        "deferred_handoff_exact.cfg",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    runner_dir = tmp_path / "scripts" / "formal"
    runner_dir.mkdir(parents=True)
    runner_path = runner_dir / "run_sumeragi_v2_service_rank_mutation.sh"
    shutil.copyfile(
        ROOT_DIR / "scripts" / "formal" / runner_path.name,
        runner_path,
    )

    assert (
        module._deferred_handoff_mutation_source_fidelity_errors(
            formal_dir, tmp_path
        )
        == []
    )

    mutation_path = formal_dir / "SumeragiV2DeferredHandoffMutation.tla"
    source = mutation_path.read_text(encoding="utf-8")
    exact_skip = (
        "IF handoff /\\ ~busy\n"
        "                        THEN busy' = FALSE"
    )
    assert source.count(exact_skip) == 1
    mutation_path.write_text(
        source.replace(exact_skip, exact_skip.replace("FALSE", "TRUE"), 1),
        encoding="utf-8",
    )
    errors = module._deferred_handoff_mutation_source_fidelity_errors(
        formal_dir, tmp_path
    )
    assert any("HandoffDrain must equal" in error for error in errors), errors

    mutation_path.write_text(source, encoding="utf-8")
    cfg_path = formal_dir / "deferred_handoff_exact.cfg"
    cfg_source = cfg_path.read_text(encoding="utf-8")
    cfg_path.write_text(
        cfg_source.replace(
            "PROPERTY HeldTargetEventuallyServed",
            "PROPERTY TRUE",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._deferred_handoff_mutation_source_fidelity_errors(
        formal_dir, tmp_path
    )
    assert any("exact reviewed TLC contract" in error for error in errors), errors

    cfg_path.write_text(cfg_source, encoding="utf-8")
    runner_source = runner_path.read_text(encoding="utf-8")
    runner_path.write_text(
        runner_source.replace(
            "[[ $deferred_handoff_rebusy_status -eq 13 ]]",
            "[[ $deferred_handoff_rebusy_status -eq 12 ]]",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._deferred_handoff_mutation_source_fidelity_errors(
        formal_dir, tmp_path
    )
    assert any("-eq 13" in error for error in errors), errors


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
    ("symbol", "old", "new", "expected_error"),
    (
        (
            "AsyncNextPreservesRecoveryInvariants",
            "  /\\ AsyncTypeInvariant\n",
            "",
            "AsyncNextPreservesRecoveryInvariants must state only",
        ),
        (
            "AsyncNextPreservesRecoveryExecutionInvariant",
            "  /\\ AsyncTypeInvariant\n",
            "",
            "AsyncNextPreservesRecoveryExecutionInvariant must state only",
        ),
        (
            "AsyncNextPreservesStrongTypeInvariant",
            "BY <1>1, <2>1, <2>2, <2>2a, <2>2b, <2>2c,\n"
            "         AsyncNextPreservesRecoveryInvariants",
            "BY <1>1, <2>1, <2>2a, <2>2b, <2>2c,\n"
            "         AsyncNextPreservesRecoveryInvariants",
            "must pass every named recovery premise projection",
        ),
        (
            "AsyncNextPreservesStrongTypeInvariant",
            "    <2>2. AsyncTypeInvariant\n"
            "      BY <1>1, AsyncStrongTypeProjectsAsyncType\n",
            "",
            "must retain the exact named <2>1 strong-inductive and <2>2 "
            "AsyncTypeInvariant projections",
        ),
    ),
)
def test_async_recovery_type_premise_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    shutil.copyfile(module.FORMAL_DIR / path.name, path)
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("kind", "symbol", "old", "new", "expected_error"),
    (
        (
            "operator",
            "AsyncRecoveryExecutionInvariant",
            "asyncOutstandingTags[asyncRecoveryNode] = {}",
            "asyncOutstandingTags[asyncRecoveryNode] \\subseteq {}",
            "AsyncRecoveryExecutionInvariant must equal only",
        ),
        (
            "operator",
            "AsyncRecoveryExecutionInvariant",
            "    /\\ SequenceHasUniqueValues(asyncRecoveryReplayQueue)\n",
            "",
            "AsyncRecoveryExecutionInvariant must equal only",
        ),
        (
            "operator",
            "AsyncRecoveryExecutionInvariant",
            "    /\\ SequenceSet(asyncRecoveryReplayQueue) \\cap\n"
            "         ResponsiveReplayScheduledCandidates(asyncRecoveryNode) = {}",
            "",
            "AsyncRecoveryExecutionInvariant must equal only",
        ),
        (
            "operator",
            "AsyncRecoveryExecutionInvariant",
            "ResponsiveReplayScheduledCandidates(asyncRecoveryNode)",
            "QueuedCandidates(asyncRecoveryNode)",
            "AsyncRecoveryExecutionInvariant must equal only",
        ),
        (
            "operator",
            "AsyncRecoveryExecutionInvariant",
            "    /\\ asyncOutstandingTags[asyncRecoveryNode] = {}\n"
            "    /\\ SequenceHasUniqueValues(asyncRecoveryReplayQueue)\n"
            "    /\\ SequenceSet(asyncRecoveryReplayQueue) \\cap\n"
            "         ResponsiveReplayScheduledCandidates(asyncRecoveryNode) = {}",
            "    asyncOutstandingTags[asyncRecoveryNode] = {}",
            "AsyncRecoveryExecutionInvariant must equal only",
        ),
        (
            "theorem",
            "AsyncNextPreservesRecoveryInvariants",
            "  /\\ AsyncRecoveryExecutionInvariant\n",
            "",
            "AsyncNextPreservesRecoveryInvariants must state only",
        ),
        (
            "theorem",
            "AsyncNextPreservesRecoveryExecutionInvariant",
            "  /\\ AsyncRecoveryExecutionInvariant\n",
            "",
            "AsyncNextPreservesRecoveryExecutionInvariant must state only",
        ),
        (
            "theorem",
            "AsyncNextPreservesRecoveryExecutionInvariant",
            "  => AsyncRecoveryExecutionInvariant'\nPROOF",
            "  => TRUE\nPROOF",
            "AsyncNextPreservesRecoveryExecutionInvariant must state only",
        ),
        (
            "theorem",
            "AsyncNextPreservesRecoveryExecutionInvariant",
            "  => AsyncRecoveryExecutionInvariant'\nPROOF",
            "  => AsyncRecoveryExecutionInvariant\nPROOF",
            "AsyncNextPreservesRecoveryExecutionInvariant must state only",
        ),
        (
            "theorem",
            "AsyncNextPreservesStrongTypeInvariant",
            "    <2>2c. AsyncRecoveryExecutionInvariant\n"
            "      BY <1>1 DEF AsyncStrongTypeInvariant\n",
            "",
            "must retain the exact named <2>2a recovery-type, <2>2b "
            "restart-authority, and <2>2c recovery-execution projections",
        ),
        (
            "theorem",
            "AsyncNextPreservesStrongTypeInvariant",
            "    <2>7. AsyncRecoveryExecutionInvariant'\n"
            "      BY <1>1, <2>1, <2>2, <2>2a, <2>2b, <2>2c,\n"
            "         AsyncNextPreservesRecoveryExecutionInvariant",
            "    <2>7. AsyncRecoveryExecutionInvariant'\n"
            "      BY <1>1, <2>1, <2>2a, <2>2b, <2>2c,\n"
            "         AsyncNextPreservesRecoveryExecutionInvariant",
            "must pass every named recovery premise projection to the exact "
            "AsyncRecoveryExecutionInvariant-prime preservation step",
        ),
        (
            "theorem",
            "AsyncNextPreservesStrongTypeInvariant",
            "    <2>7. AsyncRecoveryExecutionInvariant'\n"
            "      BY <1>1, <2>1, <2>2, <2>2a, <2>2b, <2>2c,\n"
            "         AsyncNextPreservesRecoveryExecutionInvariant",
            "    <2>7. AsyncRecoveryExecutionInvariant'\n"
            "      BY <1>1, <2>1, <2>2, <2>2a, <2>2b,\n"
            "         AsyncNextPreservesRecoveryExecutionInvariant",
            "must pass every named recovery premise projection to the exact "
            "AsyncRecoveryExecutionInvariant-prime preservation step",
        ),
        (
            "theorem",
            "AsyncNextPreservesStrongTypeInvariant",
            "    <2> QED BY <2>3, <2>4, <2>5, <2>6, <2>7\n"
            "         DEF AsyncStrongTypeInvariant",
            "    <2> QED BY <2>3, <2>4, <2>5, <2>6\n"
            "         DEF AsyncStrongTypeInvariant",
            "must make the <2>7 recovery-execution prime step an exact QED "
            "dependency",
        ),
        (
            "operator",
            "AsyncStrongTypeInvariant",
            "  /\\ AsyncRecoveryExecutionInvariant\n",
            "",
            "AsyncStrongTypeInvariant must include the exact recovery execution premise",
        ),
    ),
)
def test_async_recovery_execution_contract_mutations_fail_closed(
    tmp_path: Path,
    kind: str,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    shutil.copyfile(module.FORMAL_DIR / path.name, path)
    source = path.read_text(encoding="utf-8")
    mutator = mutate_tla_operator if kind == "operator" else mutate_tla_theorem
    path.write_text(mutator(source, symbol, old, new), encoding="utf-8")

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(expected_error in error for error in errors), errors


def test_async_recovery_scheduled_inventory_prime_scope_mutation_fails_closed(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    shutil.copyfile(module.FORMAL_DIR / path.name, path)
    source = path.read_text(encoding="utf-8")
    old = (
        "ResponsiveReplayScheduledCandidates(\n"
        "                       asyncRecoveryNode)'"
    )
    new = (
        "ResponsiveReplayScheduledCandidates(\n"
        "                       asyncRecoveryNode')"
    )
    assert old in source
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "prime ResponsiveReplayScheduledCandidates as a whole state expression"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new", "expected_error"),
    (
        (
            "ExecuteDecisionFetchPreservesTransportContentType",
            "    (/\\ StrongInductiveInvariant\n"
            "     /\\ AsyncTypeInvariant",
            "    (/\\ AsyncTypeInvariant",
            "ExecuteDecisionFetchPreservesTransportContentType must state only",
        ),
        (
            "ExecuteCommandPreservesTransportContentType",
            "         ExecuteDecisionFetchPreservesTransportContentType",
            "         ExecuteRequestCertifiedBodyPreservesTransportContentType",
            "must retain the exact dedicated ExecuteDecisionFetch case",
        ),
    ),
)
def test_execute_decision_fetch_transport_content_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    shutil.copyfile(module.FORMAL_DIR / path.name, path)
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(expected_error in error for error in errors), errors


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
        and "exact reviewed rank-composition statement" in error
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
    correct = (
        "<1>2. IsWellFoundedOn(OpToRel(<, Nat), Nat)\n"
        "    BY NatLessThanWellFounded"
    )
    weakened = (
        "<1>2. IsWellFoundedOn(OpToRel(<, Nat), Nat)\n"
        "    BY OwnedServiceRankOrderingWellFounded"
    )
    assert source.count(correct) == 1
    proof.write_text(source.replace(correct, weakened, 1), encoding="utf-8")

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ProtectedServeWellFoundedRankConvergence" in error
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
    cross_requirement = source.index("--print-cross-tool-obligations")
    cross_generation = source.index("--write-cross-tool-evidence")
    assert cross_requirement < tlaps < verus < cross_generation < final_release
    assert 'rm -f -- "$cross_tool_evidence"' in source
    assert '--verus-evidence "$verus_evidence"' in source
    assert '--cross-tool-evidence "$cross_tool_evidence"' in source

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


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "  peer::shared_byte_budget_tests::frame_retention_coalesces_each_distinct_source_owner_without_reaccounting\n",
            "",
            "must contain exactly 515 tests",
        ),
        (
            "  peer::shared_byte_budget_tests::frame_retention_coalesces_each_distinct_source_owner_without_reaccounting\n",
            "  peer::shared_byte_budget_tests::authenticated_source_count_registry_bounds_identity_churn_and_capacity_drift\n",
            "production liveness inventory repeats tests",
        ),
        (
            "readonly expected_production_liveness_test_count=515",
            "readonly expected_production_liveness_test_count=514",
            "production liveness source count must be sealed as 515",
        ),
        (
            "  block::consensus_v2::finality::tests::header_binding_requires_exact_origin_but_allows_later_certification\n"
            "  block::consensus_v2::finality::tests::genesis_header_binding_accepts_a_later_first_proposal_origin\n",
            "  block::consensus_v2::finality::tests::genesis_header_binding_accepts_a_later_first_proposal_origin\n"
            "  block::consensus_v2::finality::tests::header_binding_requires_exact_origin_but_allows_later_certification\n",
            "canonical module/test inventory SHA-256",
        ),
        (
            'production_p2p_unit_list="$(cargo test --locked -p iroha_p2p --lib -- --list)"',
            'production_p2p_unit_list="$(cargo test --locked -p iroha_p2p --all-features --lib -- --list)"',
            "reviewed P2P corridor must use exact default-feature test discovery",
        ),
        (
            'production_config_unit_list="$(cargo test --locked -p iroha_config --lib -- --list)"',
            'production_config_unit_list="$(cargo test --locked -p iroha_config --all-features --lib -- --list)"',
            "exact-output configuration discovery must use the exact iroha_config library test surface",
        ),
        (
            'elif [[ "$required_test" == parameters::* ]]; then',
            'elif [[ "$required_test" == configuration::* ]]; then',
            "exact-output configuration tests must route through the iroha_config library corridor",
        ),
        (
            'elif [[ "$module" == parameters::* ]]; then\n'
            '    module_command="cargo test --locked -p iroha_config --lib '
            '${module} -- --test-threads=1"',
            'elif [[ "$module" == parameters::* ]]; then\n'
            '    module_command="cargo test --locked -p iroha_core --lib '
            '${module} -- --test-threads=1"',
            "exact-output configuration tests must route through the iroha_config library corridor",
        ),
    ),
)
def test_production_release_inventory_rejects_name_count_and_feature_mutants(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    for relative in (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("docs/formal/sumeragi_v2/README.md"),
        Path("docs/formal/sumeragi_v2/PROOF.md"),
        Path("docs/source/sumeragi_v2_liveness.md"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
    ):
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)

    release_path = tmp_path / "scripts" / "run_sumeragi_v2_release_gates.sh"
    source = release_path.read_text(encoding="utf-8")
    assert source.count(old) == 1, old
    release_path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(expected_error in error for error in errors), errors


def test_production_release_inventory_seals_later_genesis_proposal_origin(
    tmp_path: Path,
) -> None:
    module = load_checker()
    required_paths = (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("docs/formal/sumeragi_v2/README.md"),
        Path("docs/formal/sumeragi_v2/PROOF.md"),
        Path("docs/source/sumeragi_v2_liveness.md"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
    )
    for relative in required_paths:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)

    assert module._production_liveness_release_inventory_errors(tmp_path) == []

    finality_path = (
        tmp_path
        / "crates"
        / "iroha_data_model"
        / "src"
        / "block"
        / "consensus_v2"
        / "finality.rs"
    )
    source = finality_path.read_text(encoding="utf-8")
    exact_call = "artifact_bound_to_header(0, 3, 5)"
    assert source.count(exact_call) == 1
    finality_path.write_text(
        source.replace(exact_call, "artifact_bound_to_header(0, 2, 5)", 1),
        encoding="utf-8",
    )

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(
        "genesis header-binding release regression must match exact reviewed "
        "token digest" in error
        for error in errors
    ), errors


def test_production_release_inventory_seals_contention_tolerant_restart_deadline(
    tmp_path: Path,
) -> None:
    module = load_checker()
    required_paths = (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("docs/formal/sumeragi_v2/README.md"),
        Path("docs/formal/sumeragi_v2/PROOF.md"),
        Path("docs/source/sumeragi_v2_liveness.md"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
    )
    for relative in required_paths:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)

    assert module._production_liveness_release_inventory_errors(tmp_path) == []

    runner_path = tmp_path / "integration_tests" / "tests" / "sumeragi_v2_runner.rs"
    source = runner_path.read_text(encoding="utf-8")
    exact_assertion = "assert_eq!(base_round_timeout_ms, 20_000);"
    assert source.count(exact_assertion) == 1
    runner_path.write_text(
        source.replace(
            exact_assertion,
            "assert_eq!(base_round_timeout_ms, 19_999);",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(
        "contention-tolerant restart release regression must match exact "
        "reviewed token digest" in error
        for error in errors
    ), errors


def test_production_release_inventory_seals_successor_parent_binding(
    tmp_path: Path,
) -> None:
    module = load_checker()
    required_paths = (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("docs/formal/sumeragi_v2/README.md"),
        Path("docs/formal/sumeragi_v2/PROOF.md"),
        Path("docs/source/sumeragi_v2_liveness.md"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
    )
    for relative in required_paths:
        destination = tmp_path / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / relative, destination)

    assert module._production_liveness_release_inventory_errors(tmp_path) == []

    adapter_path = tmp_path / "crates" / "iroha_core" / "src" / "sumeragi" / "v2.rs"
    canonical_source = adapter_path.read_text(encoding="utf-8")
    mutations = (
        (
            "successor_core_context_preserves_the_parent_certificate_binding",
            "assert_ne!(core_parent.context_id(), context_id(successor_id));",
            "assert_eq!(core_parent.context_id(), context_id(successor_id));",
        ),
        (
            "successor_context_requires_the_durable_cryptographic_parent",
            "let admitted = adapter\n            .receive_authenticated(authenticated)",
            "let admitted = adapter\n            .receive_authenticated(proposal)",
        ),
    )
    for test_name, old, new in mutations:
        assert canonical_source.count(old) == 1, old
        adapter_path.write_text(
            canonical_source.replace(old, new, 1),
            encoding="utf-8",
        )
        errors = module._production_liveness_release_inventory_errors(tmp_path)
        assert any(
            "successor parent-binding release regression "
            f"{test_name} must match exact reviewed token digest" in error
            for error in errors
        ), errors
        adapter_path.write_text(canonical_source, encoding="utf-8")


@pytest.mark.parametrize(
    ("relative", "old", "new"),
    (
        (
            Path("docs/formal/sumeragi_v2/PROOF.md"),
            "yielding the current 515-test, 38-module, 61-leg\ninventory",
            "yielding the current 515-test, 38-module, 60-leg\ninventory",
        ),
        (
            Path("docs/source/sumeragi_v2_liveness.md"),
            "receipt binds the 61 pre-network corridor legs and\n"
            "their exact 515-test inventory",
            "receipt binds the 60 pre-network corridor legs and\n"
            "their exact 515-test inventory",
        ),
    ),
)
def test_production_release_inventory_rejects_stale_liveness_corridor_claim(
    tmp_path: Path,
    relative: Path,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    for fixture_relative in (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("docs/formal/sumeragi_v2/README.md"),
        Path("docs/formal/sumeragi_v2/PROOF.md"),
        Path("docs/source/sumeragi_v2_liveness.md"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
    ):
        destination = tmp_path / fixture_relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / fixture_relative, destination)

    document_path = tmp_path / relative
    source = document_path.read_text(encoding="utf-8")
    assert source.count(old) == 1
    document_path.write_text(
        source.replace(old, new, 1),
        encoding="utf-8",
    )

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(
        "release inventory documentation must contain exact claim" in error
        and relative.name in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "old", "new", "expected_error"),
    (
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            "_PRODUCTION_TEST_COUNT = 515",
            "_PRODUCTION_TEST_COUNT = 514",
            "production test count must equal the exact shell inventory count 515",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-merge-sidecar", "merge_sidecar::tests", 30),',
            '("production-merge-sidecar", "merge_sidecar::tests", 29),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/write_sumeragi_v2_release_receipt.py"),
            '("production-v2-worker", "sumeragi::v2_worker::tests", 53),',
            '("production-v2-worker", "sumeragi::v2_worker::tests", 52),',
            "production module receipt tuple must equal the exact shell",
        ),
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            "  readonly expected_corridor_leg_count=61",
            "  readonly expected_corridor_leg_count=60",
            "sealed at sixty-one legs",
        ),
        (
            Path("scripts/run_sumeragi_v2_release_gates.sh"),
            '  source-sealed-workspace-tests command 0 \\\n'
            '  "cargo test --locked --workspace" \\\n'
            "  cargo test --locked --workspace",
            '  source-sealed-workspace-tests command 0 \\\n'
            '  "cargo test --workspace" \\\n'
            "  cargo test --workspace",
            "source-sealed command-success leg source-sealed-workspace-tests",
        ),
    ),
)
def test_production_release_inventory_rejects_receipt_and_command_drift(
    tmp_path: Path,
    relative: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    required_paths = (
        Path("scripts/run_sumeragi_v2_release_gates.sh"),
        Path("scripts/write_sumeragi_v2_release_receipt.py"),
        Path("docs/formal/sumeragi_v2/README.md"),
        Path("docs/formal/sumeragi_v2/PROOF.md"),
        Path("docs/source/sumeragi_v2_liveness.md"),
        Path("crates/iroha_data_model/src/block/consensus_v2/finality.rs"),
        Path("integration_tests/tests/sumeragi_v2_runner.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
    )
    for required in required_paths:
        destination = tmp_path / required
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT_DIR / required, destination)

    path = tmp_path / relative
    source = path.read_text(encoding="utf-8")
    assert source.count(old) == 1, old
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._production_liveness_release_inventory_errors(tmp_path)
    assert any(expected_error in error for error in errors), errors


def test_release_corridor_rejects_network_skips_and_zero_test_filters(
    tmp_path: Path,
) -> None:
    module = load_checker()
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
    sumeragi_source = (
        ROOT_DIR / "crates" / "iroha_core" / "src" / "sumeragi" / "mod.rs"
    ).read_text(encoding="utf-8")
    lane_work_source = (
        ROOT_DIR / "crates" / "iroha_core" / "src" / "sumeragi" / "v2_lane_work.rs"
    ).read_text(encoding="utf-8")
    lane_relay_source = (
        ROOT_DIR / "crates" / "iroha_core" / "src" / "nexus" / "lane_relay.rs"
    ).read_text(encoding="utf-8")
    merge_sidecar_source = (
        ROOT_DIR / "crates" / "iroha_core" / "src" / "merge_sidecar.rs"
    ).read_text(encoding="utf-8")
    runner_source = (
        ROOT_DIR / "crates" / "iroha_core" / "src" / "sumeragi" / "v2_runner.rs"
    ).read_text(encoding="utf-8")
    adapter_source = (
        ROOT_DIR / "crates" / "iroha_core" / "src" / "sumeragi" / "v2.rs"
    ).read_text(encoding="utf-8")
    core_source = (
        ROOT_DIR
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_core"
        / "tests.rs"
    ).read_text(encoding="utf-8")
    refinement_source = (
        ROOT_DIR
        / "crates"
        / "iroha_core"
        / "src"
        / "sumeragi"
        / "v2_core"
        / "refinement.rs"
    ).read_text(encoding="utf-8")
    effects_source = (
        ROOT_DIR / "crates" / "iroha_core" / "src" / "sumeragi" / "v2_effects.rs"
    ).read_text(encoding="utf-8")
    runtime_source = (
        ROOT_DIR / "crates" / "iroha_core" / "src" / "sumeragi" / "v2_runtime.rs"
    ).read_text(encoding="utf-8")
    worker_source = (
        ROOT_DIR / "crates" / "iroha_core" / "src" / "sumeragi" / "v2_worker.rs"
    ).read_text(encoding="utf-8")
    p2p_network_source = (
        ROOT_DIR / "crates" / "iroha_p2p" / "src" / "network.rs"
    ).read_text(encoding="utf-8")
    p2p_peer_source = (
        ROOT_DIR / "crates" / "iroha_p2p" / "src" / "peer.rs"
    ).read_text(encoding="utf-8")
    config_actual_source = (
        ROOT_DIR / "crates" / "iroha_config" / "src" / "parameters" / "actual.rs"
    ).read_text(encoding="utf-8")
    config_user_source = (
        ROOT_DIR / "crates" / "iroha_config" / "src" / "parameters" / "user.rs"
    ).read_text(encoding="utf-8")
    irohad_control_source = (
        ROOT_DIR / "crates" / "irohad" / "src" / "consensus_message_control.rs"
    ).read_text(encoding="utf-8")
    irohad_main_source = (ROOT_DIR / "crates" / "irohad" / "src" / "main.rs").read_text(
        encoding="utf-8"
    )
    kura_source = (ROOT_DIR / "crates" / "iroha_core" / "src" / "kura.rs").read_text(
        encoding="utf-8"
    )
    lane_geometry_source = (
        ROOT_DIR / "crates" / "iroha_core" / "src" / "kura" / "lane_geometry.rs"
    ).read_text(encoding="utf-8")
    liveness_doc = (
        ROOT_DIR / "docs" / "source" / "sumeragi_v2_liveness.md"
    ).read_text(encoding="utf-8")

    fidelity_root = tmp_path / "kura-application-receipt-source-fidelity"
    kura_relative = Path("crates/iroha_core/src/kura.rs")
    release_relative = Path("scripts/run_sumeragi_v2_release_gates.sh")
    for relative in (kura_relative, release_relative):
        destination = fidelity_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT_DIR / relative, destination)
    assert (
        module._kura_application_receipt_production_source_fidelity_errors(
            fidelity_root
        )
        == []
    )

    kura_fidelity_path = fidelity_root / kura_relative
    canonical_kura = kura_fidelity_path.read_text(encoding="utf-8")

    def mutate_kura_item(item_name: str, old: str, new: str) -> None:
        items = module.rust_items(canonical_kura, item_name)
        assert len(items) == 1
        item = items[0]
        assert item.source.count(old) == 1, (item_name, old)
        start = canonical_kura.index(item.source)
        end = start + len(item.source)
        kura_fidelity_path.write_text(
            canonical_kura[:start]
            + item.source.replace(old, new, 1)
            + canonical_kura[end:],
            encoding="utf-8",
        )

    observation_prune = """        if self.prune_recovery_is_required() {
            return None;
        }
"""
    observation_recovery = """        if self.prune_recovery_is_required()
            || !self.recover_bound_progress_sidecar_artifacts(
                &data_path,
                &index_path,
                "lane block application receipt",
            )
        {
            return None;
        }
"""
    kura_mutations = (
        (
            "read_active_lane_block_application_receipt_for_write_observation",
            ".open_bound_progress_sidecar(&data_path, &index_path)",
            ".open_bound_progress_pair(&data_path, &index_path)",
            "must use the structural bound open/read path",
        ),
        (
            "read_active_lane_block_application_receipt_for_write_observation",
            observation_prune,
            observation_recovery,
            "writer observation may not execute sidecar recovery",
        ),
        (
            "read_active_lane_block_application_receipt_for_write_observation",
            observation_prune,
            """        let _ = self
            .read_active_lane_block_application_receipt_durability_attested(
                lane_id,
                lane_block_height,
            );
"""
            + observation_prune,
            "writer observation may not use an attesting reader",
        ),
        (
            "read_active_lane_block_application_receipt_for_write_observation",
            observation_prune,
            """        let _ = self.read_lane_block_application_receipt(
            lane_id,
            lane_block_height,
        );
"""
            + observation_prune,
            "application-receipt writer observation control flow must match the exact reviewed",
        ),
        (
            "read_active_lane_block_application_receipt_for_write_observation",
            """        let artifact = self.read_lane_block_application_receipt_from_bound_locked(
""",
            """        let _ = self.sync_bound_progress_sidecar(
            &bound,
            "lane block application receipt",
        );
        let artifact = self.read_lane_block_application_receipt_from_bound_locked(
""",
            "writer observation may not sync a sidecar",
        ),
        (
            "write_lane_block_application_receipt_artifact",
            "if !self.recover_bound_progress_sidecar_artifacts(",
            "if !self.bound_progress_namespace_unchanged(",
            "sidecar lock and recovery",
        ),
        (
            "write_lane_block_application_receipt_artifact",
            ".sync_bound_progress_sidecar(",
            ".bound_progress_sidecar_unchanged(",
            "exact-existing strict barrier reissue",
        ),
    )
    for item_name, old, new, diagnostic in kura_mutations:
        mutate_kura_item(item_name, old, new)
        errors = module._kura_application_receipt_production_source_fidelity_errors(
            fidelity_root
        )
        assert any(diagnostic in error for error in errors), errors
        kura_fidelity_path.write_text(canonical_kura, encoding="utf-8")

    mutate_kura_item(
        "write_lane_block_application_receipt_artifact",
        """            if existing == *artifact {
""",
        """            if existing == *artifact {
                return Ok(());
            }
            if existing == *artifact {
""",
    )
    errors = module._kura_application_receipt_production_source_fidelity_errors(
        fidelity_root
    )
    for diagnostic in (
        "exact-existing condition must occur exactly 1 time(s)",
        "writer success return must occur exactly 1 time(s)",
    ):
        assert any(diagnostic in error for error in errors), (diagnostic, errors)
    kura_fidelity_path.write_text(canonical_kura, encoding="utf-8")

    release_fidelity_path = fidelity_root / release_relative
    canonical_release = release_fidelity_path.read_text(encoding="utf-8")
    strict_receipt_regression = (
        "kura::tests::progress_witness_durability::"
        "lane_block_application_receipt_strict_retry_reissues_every_barrier"
    )
    assert canonical_release.count(strict_receipt_regression) == 1
    release_fidelity_path.write_text(
        canonical_release.replace(
            strict_receipt_regression,
            "kura::tests::progress_witness_durability::"
            "lane_block_application_receipt_retry_is_not_release_bound",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._kura_application_receipt_production_source_fidelity_errors(
        fidelity_root
    )
    assert any(
        "strict application-receipt retry regression must be pinned exactly once"
        in error
        for error in errors
    ), errors

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
        (
            "sumeragi_v2_runner::prepare_qc_split_tests::",
            "restart_scenario_uses_a_contention_tolerant_view_zero_deadline",
            integration_runner_source,
        ),
        (
            "sumeragi::v2::tests::",
            "successor_core_context_preserves_the_parent_certificate_binding",
            adapter_source,
        ),
    )
    macro_step_production_inventory_additions = (
        (
            "sumeragi::v2::tests::",
            "persistence_macro_step_budgets_have_exact_five_effect_maximum",
            adapter_source,
        ),
        (
            "sumeragi::v2::tests::",
            "drive_effects_rejects_oversized_non_persisting_batch",
            adapter_source,
        ),
        (
            "sumeragi::v2::tests::",
            "drive_effects_rejects_record_specific_overbudget_before_wal_append",
            adapter_source,
        ),
        (
            "sumeragi::v2::tests::",
            "drive_effects_rejects_multiple_persist_owners_before_wal_append",
            adapter_source,
        ),
        (
            "sumeragi::v2::tests::",
            "post_wal_oversized_continuation_fails_closed_and_replays_exact_record",
            adapter_source,
        ),
        (
            "sumeragi::v2::tests::",
            "deferred_dispatch_decreases_rank_by_exactly_one_macro_step_per_turn",
            adapter_source,
        ),
        (
            "sumeragi::v2::tests::",
            "deferred_service_contract_violation_is_terminal",
            adapter_source,
        ),
        (
            "sumeragi::v2::tests::",
            "busy_deferred_input_blocks_terminal_readiness_until_serviced",
            adapter_source,
        ),
        (
            "sumeragi::v2_core::tests::",
            "commit_qc_cannot_overtake_timeout_frontier",
            core_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
                "certified_request_pressure_leaves_higher_authority_upgrade_for_retransmission",
            effects_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
                "reconstructible_new_certified_fetch_acquires_ownership_after_retransmission",
            effects_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
                "production_capacity_saturation_admits_response_and_reconstructible_fetch",
            effects_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
            "retained_producer_suffix_allows_exact_payload_chunk_to_release_fetch_capacity",
            effects_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
            "retained_producer_suffix_allows_exact_certified_response_to_release_fetch_capacity",
            effects_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
            "reconciled_decision_rejects_same_round_subject_commitment_drift",
            effects_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "serviceable_adapter_debt_drains_one_macro_step_before_new_work",
            runtime_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "serviceable_adapter_debt_runs_without_runtime_ingress",
            runtime_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "real_adapter_signature_completion_precedes_deferred_timeout_and_newer_ingress",
            runtime_source,
        ),
    )
    assert len(macro_step_production_inventory_additions) == 18
    latest_production_inventory_additions = (
        (
            "nexus::lane_relay::tests::",
            "actor_backpressure_retains_exact_relay_and_fifo_ticket",
            lane_relay_source,
        ),
        (
            "nexus::lane_relay::tests::",
            "blocked_relay_does_not_starve_a_responsive_relay",
            lane_relay_source,
        ),
        (
            "nexus::lane_relay::tests::",
            "terminal_actor_failures_return_exact_relay_ownership",
            lane_relay_source,
        ),
        (
            "nexus::lane_relay::tests::",
            "saturated_relay_owner_returns_sixty_fifth_without_actor_ticket",
            lane_relay_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "commit_certificate_response_coalesces_with_exact_busy_deferred_qc",
            runtime_source,
        ),
        (
            "sumeragi::v2_lane_work::tests::",
            "applied_lane_certificate_retires_alternative_qc_replays_without_weakening_conflicts",
            lane_work_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_completion_bound_overflow_fails_closed",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_completion_corridor_survives_ordinary_progress_and_timeout_saturation",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_completion_owner_is_source_isolated_and_queue_scoped",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_exact_max_chunk_bound_matches_canonical_wire",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_exact_response_bound_accepts_required_and_rejects_required_minus_one",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_recommended_context_fits_default_disjoint_byte_partitions",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "v2_ingress_rejects_capacity_without_per_validator_progress_reservations",
            sumeragi_source,
        ),
        (
            "sumeragi_v2_runner::prepare_qc_split_tests::",
            "exact_prepare_qc_requires_both_count_and_power_quorum",
            integration_runner_source,
        ),
        (
            "sumeragi_v2_runner::prepare_qc_split_tests::",
            "locked_commit_progress_witness_rejects_inexact_or_empty_ownership",
            integration_runner_source,
        ),
        (
            "sumeragi_v2_runner::prepare_qc_split_tests::",
            "locked_commit_progress_witness_accepts_each_exact_owner",
            integration_runner_source,
        ),
        (
            "sumeragi::v2_worker::tests::",
            "applied_height_handoff_accepts_historical_kura_global_responses_atomically",
            worker_source,
        ),
        (
            "sumeragi::v2_worker::tests::",
            "applied_height_handoff_accepts_only_exact_historical_kura_lane_certificate",
            worker_source,
        ),
        (
            "sumeragi::v2_worker::tests::",
            "ownership_units_reject_reservation_spill_and_release_exact_target",
            worker_source,
        ),
        (
            "network::tests::",
            "reliable_progress_class_matches_actor_reservations_exactly",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "reply_route_survives_peer_message_clone_mapping_and_split",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "reply_source_key_groups_relay_origins_and_orders_actor_instances",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "reply_route_source_updates_are_ordinal_monotonic_and_target_scoped",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "dependent_test_fixture_mints_opaque_tenures_and_delivery_ordinals",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "cancelled_newer_hub_cannot_erase_older_independent_route_attempt",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "dependent_fixture_models_bounded_actor_global_multi_hub_ownership",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "route_cancelled_between_preflight_and_admission_retires_without_queue_ownership",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "reply_admission_rejects_retargeting_foreign_handles_and_wrong_tickets",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "targetized_broadcast_coalesces_only_the_same_digest_and_membership",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "distinct_broadcast_residual_is_target_isolated_and_its_rank_decreases",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "exact_broadcast_retry_coalesces_but_distinct_and_direct_requests_do_not",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "removed_membership_cancels_only_old_broadcast_debt_across_readd",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "cancelled_target_child_with_pending_flush_ack_releases_exactly_once",
            p2p_network_source,
        ),
        (
            "network::tests::",
            "requested_topology_is_not_authority_and_closed_fanout_returns_all_targets",
            p2p_network_source,
        ),
    )
    assert len(latest_production_inventory_additions) == 34
    route_completion_inventory_additions = (
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_coalesces_semantic_request_and_attaches_independent_routes",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_exact_ownership_carrier_tracks_route_actions_and_cursors",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "transport_reply_route_construction_is_fallible_and_target_bound",
            sumeragi_source,
        ),
        *(
            ("merge_sidecar::tests::", test_name, merge_sidecar_source)
            for test_name in (
                "exact_active_delivery_retry_preserves_decreasing_chunk_rank",
                "alternate_source_progress_and_reconnect_preserve_independent_cursors",
                "equal_ordinal_different_tenure_alternate_source_is_rejected_atomically",
                "inactive_source_teardown_releases_budget_and_reconnect_resumes_cursor",
                "later_delivery_preserves_the_current_source_cursor",
                "later_delivery_while_chunk_is_in_flight_waits_for_flush_before_next_emit",
                "late_old_exact_item_receipt_completes_reconnected_attempt_once",
                "later_delivery_updates_pending_work_without_losing_materialized_output",
                "reconnect_during_materialization_keeps_old_authorization_but_emits_new_tenure",
                "conflicting_server_request_id_reuse_is_rejected_before_materialization",
                "failed_materialization_releases_rate_gate_for_exact_retry",
                "response_materialization_requires_and_consumes_its_exact_admission_gate",
                "inactive_reply_route_is_rejected_before_server_gate_admission",
                "completed_source_later_and_reconnect_stay_terminal_while_sibling_progresses",
                "exact_delivery_retry_rematerializes_after_rate_gate_expiry",
                "completed_source_does_not_block_a_new_alternate_source",
                "configured_route_source_capacity_bounds_semantic_attempts",
                "configured_source_geometry_reserves_more_than_eight_independent_attempts",
                "fifth_gate_from_one_hub_is_rejected_while_another_hub_progresses",
                "third_session_from_one_hub_is_rejected_while_another_hub_progresses",
                "source_byte_overflow_is_rejected_while_another_hub_progresses",
                "completed_short_session_replacement_cannot_starve_an_older_long_session",
                "route_retirement_between_admission_and_enqueue_releases_all_response_reservations",
                "saturated_materializer_does_not_erase_same_request_alternate_session",
                "saturated_materializer_does_not_erase_same_request_alternate_bytes",
                "partitioned_materialization_preserves_rejected_source_resume_cursor",
                "sidecar_flush_refinement_advances_only_exact_source_chunk",
            )
        ),
        *(
            ("sumeragi::v2::tests::", test_name, adapter_source)
            for test_name in (
                "deferred_actor_source_never_aliases_across_adapter_instances",
                "deferred_adapter_rejects_foreign_and_replayed_capabilities_before_reducer_step",
                "deferred_authenticated_retry_retains_exact_original_and_effective_tags",
                "deferred_ordinal_exhaustion_fails_adapter_closed_before_wrap",
                "deferred_service_debt_overflow_is_typed_and_fail_closed",
                "deferred_service_evidence_rejects_every_owner_and_rank_mutation",
                "deferred_zero_ordinal_is_exact_single_use_and_never_reminted",
            )
        ),
        *(
            ("sumeragi::v2_effects::tests::", test_name, effects_source)
            for test_name in (
                "live_runtime_step_rejects_missing_scheduler_ownership_before_callbacks",
                "recovery_runtime_step_rejects_invalid_scheduler_ownership_before_callbacks",
            )
        ),
        *(
            ("sumeragi::v2_lane_work::tests::", test_name, lane_work_source)
            for test_name in (
                "native_amx_request_rejects_inactive_reply_route_before_signing",
                "duplicate_reply_effect_preserves_exact_source_delivery",
                "reply_effect_rejects_missing_or_retargeted_route_set",
                "duplicate_reply_effect_updates_only_later_delivery_from_same_source",
                "duplicate_reply_effect_retains_alternate_sources_across_source_update",
                "temporarily_unserviceable_effect_requeues_behind_later_reserved_work",
                "retired_sidecar_route_between_drain_and_lane_queue_preserves_live_sibling",
                "durable_lane_certificate_coalescing_preserves_alternate_ingress_owners",
            )
        ),
        *(
            ("sumeragi::v2_runtime::tests::", test_name, runtime_source)
            for test_name in (
                "adapter_command_identity_is_derived_from_exact_immutable_payload",
                "admission_ordinal_exhaustion_fails_runtime_closed",
                "runtime_rejects_replayed_foreign_and_mutated_deferred_tokens",
                "scheduler_owner_carrier_covers_live_recovery_and_typed_deferred_branches",
                "scheduler_owner_carrier_pins_exact_fifo_identity_and_rank_fields",
                "scheduler_owner_must_be_taken_before_a_later_step_can_enter",
                "selected_owner_without_a_runtime_minted_ordinal_fails_closed",
            )
        ),
        *(
            ("sumeragi::v2_runner::tests::", test_name, runner_source)
            for test_name in (
                "reserved_lane_output_bypasses_unserviceable_head_without_losing_owner",
                "runner_dispatch_preserves_durable_lane_certificate_reply_routes",
                "runner_dispatch_preserves_certified_sidecar_chunk_reply_routes",
                "bounded_sidecar_admission_turn_applies_only_its_budget",
                "runner_dispatch_prunes_retired_sidecar_source_without_losing_live_sibling",
                "runner_dispatch_advances_certified_sidecar_only_after_writer_flush",
                "runner_dispatch_retired_admission_race_emits_no_sidecar_receipt",
                "runner_closed_sidecar_flush_reconnect_retries_same_chunk_then_advances_once",
                "runner_dispatch_rejects_certified_sidecar_chunk_without_reply_route",
                "runner_dispatch_rejects_durable_response_without_reply_routes",
            )
        ),
        *(
            ("sumeragi::v2_worker::tests::", test_name, worker_source)
            for test_name in (
                "actor_backpressure_retains_exact_final_lane_commit_qc_post",
                "actor_backpressure_retains_complete_merge_share_fanout",
                "same_tenure_updates_and_reconnect_preserve_current_item",
                "closed_sidecar_source_reconnect_retries_current_item_while_sibling_backpressures",
                "closed_sidecar_reconnect_is_capacity_checked_then_retries_current_item",
                "later_delivery_cannot_requeue_pending_or_unapplied_sidecar_flush_but_other_attempts_progress",
                "mixed_source_retry_retains_terminal_flush_target_without_resetting_live_siblings",
                "inactive_reply_target_tombstone_rejects_cross_source_equal_ordinal_collision",
                "owned_reply_history_merge_retries_candidate_retirement_after_prune",
                "newly_observed_alternate_hub_starts_at_zero_without_resetting_parked_source",
                "a_b_a_hub_reconnect_preserves_each_source_cursor",
                "owned_reply_transfer_retirement_after_validation_is_atomic",
                "bulk_backpressure_does_not_block_reserved_lane_or_safety_output",
                "non_roster_targets_cannot_consume_frozen_validator_reservations",
                "partial_fanout_progress_releases_only_the_completed_target_unit",
                "ownership_units_reject_reservation_spill_and_release_exact_target",
                "backpressured_source_does_not_block_other_sources_or_consume_their_reserve",
                "response_outputs_without_exact_routes_fail_stop",
                "exact_output_coalescing_preserves_distinct_fair_ingress_admissions",
                "orphan_chunk_coalescing_preserves_alternate_fair_ingress_routes",
                "sidecar_flush_ack_identity_mismatch_fails_closed",
                "sidecar_receipts_use_a_separate_bounded_control_queue",
                "exact_output_retry_rejects_a_different_message_identity",
                "full_exact_output_corridor_does_not_disguise_non_progress_routes_as_backpressure",
                "applied_height_handoff_retires_all_sidecar_flush_states_without_blocking_successor",
                "applied_height_handoff_counts_and_clears_parked_reply_cursor_atomically",
            )
        ),
        *(
            ("network::tests::", test_name, p2p_network_source)
            for test_name in (
                "reliable_progress_class_matches_actor_reservations_exactly",
                "reply_route_survives_peer_message_clone_mapping_and_split",
                "reply_source_key_groups_relay_origins_and_orders_actor_instances",
                "reply_route_source_updates_are_ordinal_monotonic_and_target_scoped",
                "dependent_test_fixture_mints_opaque_tenures_and_delivery_ordinals",
                "cancelled_newer_hub_cannot_erase_older_independent_route_attempt",
                "dependent_fixture_models_bounded_actor_global_multi_hub_ownership",
                "reply_route_pruning_retains_equal_ordinal_tenure_tombstone",
                "reply_route_set_isolates_sources_preserves_cursors_and_prunes_retired_capacity",
                "route_cancelled_between_preflight_and_admission_retires_without_queue_ownership",
                "reply_admission_rejects_retargeting_foreign_handles_and_wrong_tickets",
                "reply_actor_admission_does_not_complete_writer_flush_ack",
                "reply_flush_identity_binds_ticket_tenure_source_payload_and_delivery_occurrence",
                "reply_flush_test_fixture_binds_exact_canonical_post_and_opaque_actor",
                "reply_flush_ack_cancellation_between_precheck_and_budget_lock_returns_none",
                "retired_reply_tenure_closes_flush_ack_without_false_completion",
                "reply_flush_test_fixture_controls_success_and_close_without_false_receipts",
                "reply_flush_ack_completes_only_after_peer_writer_flush",
            )
        ),
        (
            "consensus_message_control::tests::",
            "controlled_v2_admission_preserves_distinct_relay_identity",
            irohad_control_source,
        ),
        *(
            ("consensus_message_control::tests::", test_name, irohad_control_source)
            for test_name in (
                "failed_release_clears_in_flight_ownership_and_latches_fatal",
                "fatal_controller_rejects_an_unchanged_command_poll",
                "retired_release_finishes_drain_without_claiming_delivery",
            )
        ),
        (
            "network_relay_tests::",
            "obsolete_sumeragi_relay_message_completes_as_delivered",
            irohad_main_source,
        ),
        (
            "network_relay_tests::",
            "test_control_hold_release_preserves_live_route_and_retires_canceled_reentry",
            irohad_main_source,
        ),
    )
    assert len(route_completion_inventory_additions) == 114
    source_geometry_inventory_additions = (
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "fair_v2_ingress_projection_distinguishes_identical_bytes_from_distinct_origins",
            sumeragi_source,
        ),
        (
            "merge_sidecar::tests::",
            "authenticated_source_limits_are_fixed_at_four_gates_two_sessions_and_sixteen_mibibytes",
            merge_sidecar_source,
        ),
        (
            "sumeragi::v2::tests::",
            "authenticated_deferred_service_rejects_same_kind_envelope_swap_before_reducer",
            adapter_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
            "owned_payload_chunk_rejects_source_swap_before_service_and_keeps_unknown_work_nonfatal",
            effects_source,
        ),
        (
            "sumeragi::v2_effects::tests::",
            "certified_body_response_carrier_swap_fails_closed_before_fetch_mutation",
            effects_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "runtime_merges_alternate_sources_for_one_semantic_request",
            runtime_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "runtime_keeps_identical_wire_requests_from_distinct_semantic_origins_independent",
            runtime_source,
        ),
        (
            "sumeragi::v2_runtime::tests::",
            "busy_deferred_request_merges_alternate_source_and_services_exact_carrier",
            runtime_source,
        ),
        (
            "sumeragi::v2_worker::tests::",
            "owned_orphan_chunk_replay_preserves_alternate_source_routes_and_cursors",
            worker_source,
        ),
        (
            "network::tests::",
            "peer_message_mints_actor_global_delivery_ordinals_across_connection_tenures",
            p2p_network_source,
        ),
        (
            "parameters::actual::tests::",
            "sumeragi_v2_exact_output_geometry_checks_every_arithmetic_boundary",
            config_actual_source,
        ),
        (
            "parameters::user::duration_clamp_tests::",
            "sumeragi_v2_exact_output_geometry_accepts_network_source_boundary",
            config_user_source,
        ),
        (
            "parameters::user::duration_clamp_tests::",
            "sumeragi_v2_exact_output_geometry_accepts_equal_capacity_boundary",
            config_user_source,
        ),
        (
            "parameters::user::duration_clamp_tests::",
            "sumeragi_v2_exact_output_geometry_rejects_unreservable_network_sources",
            config_user_source,
        ),
    )
    assert len(source_geometry_inventory_additions) == 14
    route_lifecycle_inventory_additions = tuple(
        ("network::tests::", test_name, p2p_network_source)
        for test_name in (
            "reply_route_binding_rejects_evicted_tombstone_collision",
            "network_actor_drop_retires_routes_and_only_its_waiters",
            "peer_message_rehydration_rejects_second_reply_route_without_retargeting",
        )
    )
    assert len(route_lifecycle_inventory_additions) == 3
    latest_h_geometry_and_daemon_inventory_additions = (
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "authenticated_non_validator_source_cap_retries_third_source_until_one_lane_drains",
            sumeragi_source,
        ),
        (
            "sumeragi::authoritative_runtime_gate_tests::",
            "alternate_reply_route_attaches_before_authenticated_source_lane_cap",
            sumeragi_source,
        ),
        (
            "consensus_message_control::tests::",
            "stale_duplicate_reordered_and_unknown_releases_are_atomic",
            irohad_control_source,
        ),
        (
            "consensus_message_control::tests::",
            "hold_capacity_is_bounded_by_count_bytes_and_checked_arithmetic",
            irohad_control_source,
        ),
        (
            "consensus_message_control::tests::",
            "drain_fence_holds_racing_chunks_fifo_until_atomic_cutover",
            irohad_control_source,
        ),
        (
            "tests::relay_fairness::",
            "hold_release_preserves_exact_layered_ownership_until_recorded_terminal",
            irohad_main_source,
        ),
        (
            "parameters::user::duration_clamp_tests::",
            "sumeragi_authenticated_non_validator_sources_must_fit_network_geometry",
            config_user_source,
        ),
        (
            "parameters::user::duration_clamp_tests::",
            "sumeragi_authenticated_non_validator_sources_use_effective_lane_profile_geometry",
            config_user_source,
        ),
        (
            "parameters::actual::tests::",
            "sumeragi_v2_config_format_changes_the_handshake_fingerprint",
            config_actual_source,
        ),
        (
            "sumeragi::v2_core::refinement::tests::",
            "historical_body_pipeline_kernel_rejects_request_subject_and_owner_substitution",
            refinement_source,
        ),
        (
            "sumeragi::v2_core::refinement::tests::",
            "historical_certificate_kernel_rejects_foreign_admission_and_unretired_request",
            refinement_source,
        ),
        (
            "peer::run::tests::",
            "consensus_lane_and_v2_topics_share_authenticated_high_source_credit",
            p2p_peer_source,
        ),
    )
    assert len(latest_h_geometry_and_daemon_inventory_additions) == 12
    production_inventory_additions = (
        new_production_inventory_additions
        + macro_step_production_inventory_additions
        + latest_production_inventory_additions
        + route_completion_inventory_additions
        + source_geometry_inventory_additions
        + route_lifecycle_inventory_additions
        + latest_h_geometry_and_daemon_inventory_additions
    )
    for _, test_name, source in production_inventory_additions:
        assert source.count(f"fn {test_name}(") == 1
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
    final_proof_check = release_source.index("final_proof_evidence_args=(")
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
    final_proof_region = release_source[final_proof_check:aggregate_receipt]
    assert "--release" in final_proof_region
    assert (
        "--evidence target/formal/sumeragi_v2/proof_evidence.json"
        in final_proof_region
    )
    assert (
        "--verus-evidence target/formal/sumeragi_v2/verus_evidence.json"
        in final_proof_region
    )
    assert '--print-cross-tool-obligations' in release_source[
        final_manifest:final_proof_check
    ]
    assert '--cross-tool-evidence "$cross_tool_evidence_path"' in final_proof_region
    assert '"${final_proof_evidence_args[@]}"' in final_proof_region
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
        if line.strip().startswith(
            (
                "sumeragi::",
                "sumeragi_v2_runner::",
                "kura::",
                "nexus::",
                "merge_sidecar::",
                "zk::",
                "block::",
                "offline::",
                "peer::",
                "network::",
                "consensus_message_control::tests::",
                "network_relay_tests::",
                "tests::relay_fairness::",
                "genesis_bootstrap::tests::",
                "parameters::",
            )
        )
    )
    assert len(production_inventory) == 515
    assert len(set(production_inventory)) == 515
    assert "readonly expected_production_liveness_test_count=515" in release_source
    assert "_PRODUCTION_TEST_COUNT = 515" in receipt_source
    receipt_spec = importlib.util.spec_from_file_location(
        "sumeragi_v2_release_receipt_inventory",
        ROOT_DIR / "scripts" / "write_sumeragi_v2_release_receipt.py",
    )
    assert receipt_spec is not None
    assert receipt_spec.loader is not None
    receipt_module = importlib.util.module_from_spec(receipt_spec)
    sys.modules[receipt_spec.name] = receipt_module
    receipt_spec.loader.exec_module(receipt_module)
    assert sum(count for _, _, count in receipt_module._PRODUCTION_MODULES) == 515
    assert (
        receipt_module._PRODUCTION_MODULES
        == module._PRODUCTION_LIVENESS_RELEASE_MODULE_CONTRACTS
    )
    assert len(receipt_module._corridor_legs()) == 61
    assert receipt_module._production_module_command(
        "parameters::actual::tests"
    ) == (
        "cargo test --locked -p iroha_config --lib parameters::actual::tests "
        "-- --test-threads=1"
    )
    assert receipt_module._production_module_command(
        "parameters::user::duration_clamp_tests"
    ) == (
        "cargo test --locked -p iroha_config --lib "
        "parameters::user::duration_clamp_tests -- --test-threads=1"
    )
    assert receipt_module._production_module_command(
        "block::consensus_v2::finality::tests"
    ) == (
        "cargo test --locked -p iroha_data_model --lib "
        "block::consensus_v2::finality::tests -- --test-threads=1"
    )
    for _, module, expected_count in receipt_module._PRODUCTION_MODULES:
        assert (
            sum(test.startswith(f"{module}::") for test in production_inventory)
            == expected_count
        )
    for module, test_name, _ in production_inventory_additions:
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
        "lane_block_application_receipt_strict_retry_reissues_every_barrier",
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
        "sumeragi::status::v2_liveness_watchdog_tests::"
        "active_watchdog_is_deadline_driven_edge_triggered_and_recovers_on_progress",
        "sumeragi::status::v2_liveness_watchdog_tests::"
        "active_watchdog_resets_on_successor_owner_and_status_clear",
        "peer::shared_byte_budget_tests::"
        "frame_retention_coalesces_each_distinct_source_owner_without_reaccounting",
        "peer::shared_byte_budget_tests::"
        "authenticated_source_count_registry_bounds_identity_churn_and_capacity_drift",
        "network::tests::reconnecting_peer_cannot_multiply_retained_source_credits",
        "sumeragi::v2_core::refinement::tests::"
        "two_stage_relay_retry_kernel_rejects_source_rotation_eligibility_and_fifo_mutations",
        "tests::relay_fairness::"
        "daemon_source_credit_layers_over_upstream_and_preserves_the_ninth_exact_owner",
        "tests::relay_fairness::"
        "saturated_sumeragi_dispatch_does_not_hold_normal_worker_permits",
        "tests::relay_fairness::"
        "real_inner_ingress_retry_preserves_a_copies_and_bounds_b_service_rank",
        "block::consensus_v2::finality::tests::"
        "genesis_header_binding_accepts_a_later_first_proposal_origin",
        "sumeragi_v2_runner::prepare_qc_split_tests::"
        "restart_scenario_uses_a_contention_tolerant_view_zero_deadline",
        "sumeragi::v2::tests::"
        "successor_core_context_preserves_the_parent_certificate_binding",
        "sumeragi::v2_lane_work::tests::"
        "decided_lane_ownership_blocks_rollover_until_its_session_is_durable",
        "sumeragi::v2_recovery::tests::"
        "finality_complete_tip_with_incomplete_lane_completion_reopens_same_height",
        "sumeragi::v2_runner::tests::"
        "terminal_ingress_discards_commit_discovery_and_losing_current_body_requests",
    ):
        assert required_test in production_inventory
    assert (
        'required_data_model_lane_certificate_test="block::consensus::tests::'
        'lane_block_certificate_decodes_atomically_from_slice"'
        in release_source
    )
    assert '"lane-certificate-rust"' in receipt_source
    assert "_DATA_LANE_CERTIFICATE_TEST" in receipt_source
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
        if line.strip().startswith(
            (
                "sumeragi::",
                "sumeragi_v2_runner",
                "kura::",
                "nexus::",
                "merge_sidecar::",
                "zk::",
                "block::",
                "offline::",
                "peer::",
                "network::",
                "consensus_message_control::tests",
                "network_relay_tests",
                "tests::relay_fairness",
                "genesis_bootstrap::",
                "parameters::",
            )
        )
    )
    assert len(production_modules) == 38
    assert len(set(production_modules)) == 38
    assert "kura::tests" in production_modules
    assert "kura::lane_geometry::tests" in production_modules
    assert "sumeragi::authoritative_runtime_gate_tests" in production_modules
    assert "sumeragi::v2_block_sync::tests" in production_modules
    assert "sumeragi::v2_apply::tests" in production_modules
    assert "sumeragi_v2_runner" in production_modules
    assert "peer::run::tests" in production_modules
    assert "network::tests" in production_modules
    assert "merge_sidecar::tests" in production_modules
    assert "consensus_message_control::tests" in production_modules
    assert "network_relay_tests" in production_modules
    assert "tests::relay_fairness" in production_modules
    assert "parameters::actual::tests" in production_modules
    assert "parameters::user::duration_clamp_tests" in production_modules
    assert (
        'for module_index in "${!production_liveness_modules[@]}"; do'
        in release_source
    )
    assert (
        'cargo test --locked -p iroha_core --lib "$module" -- --test-threads=1'
        in release_source
    )
    assert (
        'cargo test --locked -p iroha_p2p --lib "$module" -- --test-threads=1'
        in release_source
    )
    assert (
        "cargo test --locked -p irohad --bin irohad --features "
        'test-network-message-control \\\n        "$module" -- --test-threads=1'
        in release_source
    )
    assert (
        'cargo test --locked -p iroha_config --lib "$module" -- --test-threads=1'
        in release_source
    )
    assert (
        'production_config_unit_list="$(cargo test --locked -p iroha_config '
        '--lib -- --list)"'
        in release_source
    )
    assert (
        'production_config_ignored_unit_list="$(\n'
        '  cargo test --locked -p iroha_config --lib -- --list --ignored\n'
        ')"'
        in release_source
    )
    assert 'elif [[ "$required_test" == parameters::* ]]; then' in release_source
    assert (
        "cargo test --locked -p integration_tests --test "
        "sumeragi_v2_runner_isolated "
        "sumeragi_v2_runner::prepare_qc_split_tests "
        "-- --test-threads=1"
        in release_source
    )
    assert (
        '_PRODUCTION_INTEGRATION_MODULE = "sumeragi_v2_runner::prepare_qc_split_tests"'
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
        "current_view_timeout_path_yields_only_to_an_exact_locked_commit_owner"
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
    assert "did not run exactly 11 passing tests" in release_source
    assert "did not run exactly five passing tests" in release_source
    assert "preflight-chaos-launcher pytest 5" in release_source
    assert "did not run exactly 68 passing tests" in release_source
    assert "preflight-release-identity pytest 68" in release_source
    assert "did not run exactly 71 passing tests" in release_source
    assert "preflight-release-bootstrap pytest 71" in release_source
    assert "did not run exactly 37 passing tests" in release_source
    assert "preflight-release-bootstrap-validator pytest 37" in release_source
    assert "did not run exactly 189 passing tests" in release_source
    assert "preflight-release-receipt pytest 189" in release_source
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
        '"preflight-release-receipt",\n                "pytest",\n                189,'
        in receipt_source
    )
    assert "did not run exactly 1045 passing tests" in release_source
    assert "preflight-proof-fidelity pytest 1045" in release_source
    assert (
        "^1045 passed in [0-9]+([.][0-9]+)?s( "
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
        '"preflight-proof-fidelity",\n                "pytest",\n                1045,'
        in receipt_source
    )
    assert "did not run exactly 16 passing tests" in release_source
    assert "preflight-formal-launcher pytest 16" in release_source
    assert (
        '"preflight-formal-launcher",\n                "pytest",\n                16,'
        in receipt_source
    )
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
    assert "expected_corridor_leg_count=61" in release_source
    for leg_id, command in (
        (
            "source-sealed-workspace-clippy",
            "cargo clippy --workspace --all-targets -- -D warnings",
        ),
        ("source-sealed-workspace-tests", "cargo test --locked --workspace"),
        (
            "source-sealed-irohad-tests",
            "cargo test --locked -p irohad --bin irohad "
            "--features test-network-message-control",
        ),
    ):
        assert f"  {leg_id} command 0" in release_source
        assert command in release_source
        assert any(
            receipt_leg_id == leg_id
            and kind == "command"
            and expected_count == 0
            and receipt_command == command
            for receipt_leg_id, kind, expected_count, receipt_command in receipt_module._corridor_legs()
        )
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
    assert "expected exactly 118 Sumeragi v2 reducer unit tests" in harness_source
    assert "reducer unit gate requires all 118 tests to be runnable" in harness_source

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
        "3ab43c7ff31db4ced850619d4746fa4c841a7681" in source
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


def test_tlapm_corridor_uses_one_pinned_identity() -> None:
    commit = "3ab43c7ff31db4ced850619d4746fa4c841a7681"
    exact_identity_paths = (
        ROOT_DIR / "scripts" / "formal" / "install_sumeragi_v2_tlapm.sh",
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlaps.sh",
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlc.sh",
        ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_progress_mutations.sh",
        ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_replay_trace.sh",
        ROOT_DIR / "scripts" / "formal" / "check_sumeragi_v2_proof_ledger.py",
        ROOT_DIR / "scripts" / "run_sumeragi_v2_release_gates.sh",
        ROOT_DIR / ".github" / "workflows" / "nightly_sumeragi_formal.yml",
        ROOT_DIR / ".github" / "workflows" / "pr.yml",
        ROOT_DIR / "docs" / "formal" / "sumeragi_v2" / "README.md",
        ROOT_DIR
        / "docs"
        / "formal"
        / "sumeragi_v2"
        / "CROSS_TOOL_EVIDENCE.md",
    )
    for path in exact_identity_paths:
        assert commit in path.read_text(encoding="utf-8"), path

    proof_source = (
        ROOT_DIR / "docs" / "formal" / "sumeragi_v2" / "PROOF.md"
    ).read_text(encoding="utf-8")
    assert commit[:7] in proof_source


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

    tlapm_source = installers[0].read_text(encoding="utf-8")
    assert "releases/download/${TLAPM_VERSION}" not in tlapm_source
    assert (
        'readonly TLAPM_COMMIT="3ab43c7ff31db4ced850619d4746fa4c841a7681"'
        in tlapm_source
    )
    for asset_id, digest in (
        (
            "482292328",
            "a686da5dc31892edcd02f25bb14061427e29e16317002d43c5b5be970d1d5daf",
        ),
        (
            "482297997",
            "3ca4c39613e58b90e46a385ee61e2c7f17375c19854ea1a35e056d6eb902071c",
        ),
    ):
        assert f'RELEASE_ASSET_ID="{asset_id}"' in tlapm_source
        assert f'ARCHIVE_SHA256="{digest}"' in tlapm_source
    assert "GitHub Actions run 29682668751" in tlapm_source
    assert "TLAPM_ARCHIVE_PATH" in tlapm_source


def test_ledger_is_canonical_json() -> None:
    module = load_checker()
    source = module.LEDGER_PATH.read_text(encoding="utf-8")
    parsed = json.loads(source)

    assert source == json.dumps(parsed, indent=2, ensure_ascii=False) + "\n"


def copy_audited_rank_leaf_contract_fixture(tmp_path: Path, module) -> Path:
    """Install the reviewed Stage-4/5 contracts around the current proof source."""

    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
        "SumeragiV2Proofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    vocabulary_source = vocabulary.read_text(encoding="utf-8")
    property_block = r'''
ProtectedStage4RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<4, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<4, position>>))

ProtectedStage5RankProgressProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet, position \in Nat:
         (gst
           /\ ResponsiveProtectedCandidateOwned(candidate)
           /\ CandidateServiceRank(candidate) = <<5, position>>)
           ~> (~ResponsiveProtectedCandidateOwned(candidate)
                \/ ServiceRankLess(CandidateServiceRank(candidate),
                     <<5, position>>))
'''
    if "ProtectedStage4RankProgressProperty" not in vocabulary_source:
        vocabulary_source = vocabulary_source.replace(
            "=============================================================================\n",
            property_block + "\n=============================================================================\n",
            1,
        )
        vocabulary.write_text(vocabulary_source, encoding="utf-8")

    proof = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    proof_source = proof.read_text(encoding="utf-8")
    wrapper_block = r'''
THEOREM ProtectedStage4RankProgressFromFairScheduler ==
  \A initialContext:
    ProtectedStage4RankProgressProperty(AsyncSpecAt(initialContext))
BY FairProtectedStage4RankDescent
   DEF ProtectedStage4RankProgressProperty

THEOREM ProtectedStage5RankProgressFromFairFifo ==
  \A initialContext:
    ProtectedStage5RankProgressProperty(AsyncSpecAt(initialContext))
BY FairProtectedStage5RankDescent
   DEF ProtectedStage5RankProgressProperty
'''
    if "ProtectedStage4RankProgressFromFairScheduler" not in proof_source:
        proof_source = proof_source.replace(
            "=============================================================================\n",
            wrapper_block + "\n=============================================================================\n",
            1,
        )
        proof.write_text(proof_source, encoding="utf-8")
    return formal_dir


def audited_rank_leaf_contract_errors(module, formal_dir: Path) -> list[str]:
    """Run both source and ledger-target guards for the audited rank leaves."""

    proof_source = (
        formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    ).read_text(encoding="utf-8")
    errors = module._async_proof_architecture_errors(formal_dir)
    errors.extend(
        module._proof_obligation_architecture_errors(
            module.load_ledger()["obligations"],
            {"SumeragiV2AsyncLivenessProofs": proof_source},
        )
    )
    return errors


def test_audited_rank_leaf_synthetic_contract_is_green(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = copy_audited_rank_leaf_contract_fixture(tmp_path, module)

    assert audited_rank_leaf_contract_errors(module, formal_dir) == []


@pytest.mark.parametrize(
    ("filename", "kind", "symbol", "old", "new", "expected_error"),
    (
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "AsyncSpecAlwaysProgressOwnershipInvariant",
            "AsyncSpecAt(initialContext) => []AsyncProgressOwnershipInvariant",
            "AsyncSpecAt(initialContext) => <>AsyncProgressOwnershipInvariant",
            "AsyncSpecAlwaysProgressOwnershipInvariant must state only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "AsyncSpecAlwaysProgressOwnershipInvariant",
            "AsyncBracketNextPreservesProgressOwnership",
            "AsyncBracketNextPreservesStrongTypeInvariant",
            "omits explicit transition/fairness inventory",
        ),
        (
            "SumeragiV2LivenessProofs.tla",
            "operator",
            "ProtectedStage4RankProgressProperty",
            "CandidateServiceRank(candidate) = <<4, position>>",
            "CandidateServiceRank(candidate) = <<5, position>>",
            "ProtectedStage4RankProgressProperty must equal only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage4RankProgressFromFairScheduler",
            "ProtectedStage4RankProgressProperty(AsyncSpecAt(initialContext))",
            "ProtectedStage4RankProgressProperty(AsyncFiniteSpec)",
            "ProtectedStage4RankProgressFromFairScheduler must state only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage4RankProgressFromFairScheduler",
            "BY FairProtectedStage4RankDescent",
            "BY PTL",
            "omits explicit transition/fairness inventory",
        ),
        (
            "SumeragiV2LivenessProofs.tla",
            "operator",
            "ProtectedStage5RankProgressProperty",
            "CandidateServiceRank(candidate) = <<5, position>>",
            "CandidateServiceRank(candidate) = <<4, position>>",
            "ProtectedStage5RankProgressProperty must equal only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage5RankProgressFromFairFifo",
            "ProtectedStage5RankProgressProperty(AsyncSpecAt(initialContext))",
            "ProtectedStage5RankProgressProperty(AsyncFiniteSpec)",
            "ProtectedStage5RankProgressFromFairFifo must state only",
        ),
        (
            "SumeragiV2AsyncLivenessProofs.tla",
            "theorem",
            "ProtectedStage5RankProgressFromFairFifo",
            "BY FairProtectedStage5RankDescent",
            "BY PTL",
            "omits explicit transition/fairness inventory",
        ),
    ),
)
def test_audited_rank_leaf_source_mutations_fail_closed(
    tmp_path: Path,
    filename: str,
    kind: str,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_audited_rank_leaf_contract_fixture(tmp_path, module)
    path = formal_dir / filename
    source = path.read_text(encoding="utf-8")
    mutator = mutate_tla_operator if kind == "operator" else mutate_tla_theorem
    path.write_text(mutator(source, symbol, old, new), encoding="utf-8")

    errors = audited_rank_leaf_contract_errors(module, formal_dir)
    assert any(
        expected_error in error and symbol in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "token"),
    (
        (
            "ProtectedServeStage5CarrierFacts",
            "ServeOccurrenceIndexCharacterization",
        ),
        (
            "ProtectedServeStage5EnablesFairWorker",
            "QueuedIoEnablesPostGstService",
        ),
        (
            "ProtectedServeStage5WorkerStrictlyProgresses",
            "TailRemovesUniqueServeOccurrence",
        ),
        (
            "ProtectedServeStage5UnlessProgress",
            "AsyncBracketNextPreservesStrongTypeInvariant",
        ),
        (
            "FairProtectedServeStage5RankDescent",
            "ProtectedServeStage5EnablesFairWorker",
        ),
        (
            "ProtectedServeRankProgressFromFairFifo",
            "FairProtectedServeStage5RankDescent",
        ),
    ),
)
def test_protected_serve_fifo_proof_dependency_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    token: str,
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

    proof = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    proof.write_text(
        delete_tla_theorem_token(
            proof.read_text(encoding="utf-8"),
            symbol,
            token,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        symbol in error
        and "omits explicit transition/fairness inventory" in error
        and token in error
        for error in errors
    ), errors


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
            '   "TimeoutCertificate", "CertifiedRequest", "CommitCertificateRequest",\n',
            '   "TimeoutCertificate", "CommitCertificateRequest",\n',
            1,
        ).replace(
            "    + Cardinality(\n"
            "        IngressTransportCompletionProtectedSourcesFor(lanes, recipient))\n",
            "",
            1,
        ).replace(
            'IngressTransportCompletionKinds == {"Chunk", "CertifiedResponse"}',
            'IngressTransportCompletionKinds == {"Chunk"}',
            1,
        ).replace(
            "  \\/ ~IngressLaneHasTransportCompletionIn(\n"
            "       asyncIngressLanes, item.envelope.recipient, item.source)\n",
            "  \\/ TRUE\n",
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
    assert any(
        "IngressTransportCompletionKinds must equal only" in error
        for error in errors
    )
    assert any("IngressProgressKinds must equal only" in error for error in errors)
    assert any(
        "IngressProtectedSlotCountFor must equal only" in error for error in errors
    )
    assert any(
        "AsyncTransportCompletionOwnerGateAllows must equal only" in error
        for error in errors
    )
    assert any("DeliveryClass must equal only" in error for error in errors)


def test_async_source_fidelity_pins_untrusted_transport_completion_exclusion(
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
            '          kind \\in {"Chunk", "CertifiedResponse"},\n'
            "          source \\in AsyncIngressSources,\n",
            '          kind \\in {"Chunk", "CertifiedResponse"},\n'
            "          source \\in ValidatorIds,\n",
            1,
        )
        .replace(
            '  /\\ (item.kind \\notin {"Noise", "Chunk", "CertifiedResponse"}\n'
            "        => item.source \\in ValidatorIds)",
            '  /\\ (item.kind # "Noise" => item.source \\in ValidatorIds)',
            1,
        )
        .replace(
            "  IN /\\ kind \\in IngressTransportCompletionKinds\n",
            '  IN /\\ kind = "Chunk"\n',
            1,
        )
        .replace("     /\\ nonce = 0\n", "", 1)
        .replace(
            "       InjectUntrustedTransportCompletion(kind, recipient, nonce)\n",
            "       TRUE\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "AsyncNetworkItems omits required production behavior" in error
        for error in errors
    )
    assert any(
        "AsyncItemTyped omits required production behavior" in error
        for error in errors
    )
    assert any(
        "InjectUntrustedTransportCompletion omits required production behavior"
        in error
        for error in errors
    )
    assert any(
        "AsyncFaultStep omits required production behavior" in error
        for error in errors
    )

    path.write_text(
        source.replace("     /\\ nonce = 0\n", "", 1),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "InjectUntrustedTransportCompletion omits required production behavior"
        in error
        for error in errors
    )


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


@pytest.mark.parametrize(
    ("relative", "item_name", "old", "new", "expected_error"),
    (
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "check_queue_limit",
            ".checked_add(frame_len)",
            ".saturating_add(frame_len)",
            "checked byte/frame queue admission and overflow rejection",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "encrypted_frame_geometry",
            "u32::try_from(encrypted_size).map_err(|_| Error::FrameTooLarge)?",
            "encrypted_size as u32",
            "checked encrypted sender geometry encrypted_frame_geometry",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "data_frame_wire_len_from_payload_len_with_peer_key_bytes",
            "crate::peer::data_message_wire_len_from_payload_len::<RelayMessage<T>>(relay_len)",
            "relay_len",
            "checked P2P transport geometry "
            "data_frame_wire_len_from_payload_len_with_peer_key_bytes",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "enqueue_encrypted",
            "if encrypted_size > "
            "self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "if encrypted_size > self.max_frame_bytes {",
            "checked runtime-clamped encrypted geometry before cap/queue admission",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "account_enqueued",
            "self.queued_safety_bytes = self\n"
            "                        .queued_safety_bytes\n"
            "                        .checked_add(frame_len)",
            "self.queued_safety_bytes = self\n"
            "                        .queued_safety_bytes\n"
            "                        .saturating_add(frame_len)",
            "checked admitted queue-byte accounting",
        ),
        (
            Path("crates/iroha_p2p/src/lib.rs"),
            "frame_plaintext_cap",
            ".min(MAX_ENCRYPTED_FRAME_BYTES)",
            ".min(usize::MAX)",
            "checked P2P transport geometry frame_plaintext_cap",
        ),
        (
            Path("crates/iroha_p2p/src/lib.rs"),
            "frame_queue_charge",
            ".checked_add(P2P_FRAME_LENGTH_PREFIX_BYTES)",
            ".checked_add(0)",
            "checked P2P transport geometry frame_queue_charge",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_short_p2p_frame_math(
    tmp_path: Path,
    relative: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    assert module._transport_geometry_production_source_fidelity_errors(repo_root) == []

    mutate_rust_item_source(module, repo_root / relative, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "checked_encoded_frame_len",
            "let encoded_len = ncore::encoded_frame_len(message)?;",
            "let encoded_len = 0;",
            "exact Norito counting preflight before P2P allocation",
        ),
        (
            "try_send",
            "if encrypted.len() > "
            "self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "if false && encrypted.len() > "
            "self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "QUIC counting preflight and post-encryption runtime-cap check",
        ),
        (
            "reserve_for_frame",
            "if size > self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "if size > self.max_frame_bytes {",
            "runtime-clamped checked and incremental receiver reservation",
        ),
        (
            "reserve_for_frame",
            ".ok_or(Error::FrameTooLarge)?\n                .min(needed);",
            ".ok_or(Error::FrameTooLarge)?\n                .min(usize::MAX);",
            "runtime-clamped checked and incremental receiver reservation",
        ),
        (
            "prepare_message",
            "let encoded_len = "
            "checked_encoded_frame_len::<T, E>(msg, self.max_frame_bytes)?;",
            "let encoded_len = 0;",
            "counting sender preflight before material encoding",
        ),
        (
            "prepare_encoded_buffer",
            "let max_plaintext = frame_plaintext_cap_for::<E>(self.max_frame_bytes);",
            "let max_plaintext = usize::MAX;",
            "generic AEAD cap before sender batching",
        ),
        (
            "enqueue_encrypted",
            "if self.encrypted.len() != encrypted_size {",
            "if false && self.encrypted.len() != encrypted_size {",
            "post-encryption sender geometry agreement",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_runtime_frame_mutants(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    peer_path = repo_root / "crates" / "iroha_p2p" / "src" / "peer.rs"
    mutate_rust_item_source(module, peer_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "merge",
            "other.bytes = 0;",
            "let _released_on_drop = other.bytes;",
            "already-accounted source leases coalesce without release and reacquisition",
        ),
        (
            "credit_owner",
            "if required.len() > self.max_sources {",
            "if false && required.len() > self.max_sources {",
            "shared authenticated-source registry preserves identity, protected sources, and capacity",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_source_owner_mutants(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    peer_path = repo_root / "crates" / "iroha_p2p" / "src" / "peer.rs"
    mutate_rust_item_source(module, peer_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "try_reserve_for_source",
            "(Some(retained), Some(candidate)) => !retained.matches(candidate),",
            "(Some(_), Some(_)) => false,",
            "queued progress tickets must retain the exact weak delivery authority rather than reusing ordinal-equivalent tenure",
        ),
        (
            "try_reserve_for_source",
            "if source_retained.is_some_and(|retained| retained.items >= 1) {",
            "if source_retained.is_some_and(|retained| retained.items >= 2) {",
            "distinct broadcast or direct requests remain FIFO-ranked behind a target owner",
        ),
        (
            "submit_progress_message_to_source",
            "ProgressLeaseAttempt::SameRequestAlreadyOwned => return Ok(None),",
            "ProgressLeaseAttempt::SameRequestAlreadyOwned => "
            "return Ok(Some(NetworkActorAdmittedTicketIdentity::forged())),",
            (
                "same-request and cancelled admission return no new ticket identity, "
                "while invalid ownership cannot substitute for the original request"
            ),
        ),
        (
            "broadcast_recoverable",
            "&& Arc::ptr_eq(&ticket.topology, &self.reliable_broadcast_topology)",
            "&& true",
            "broadcast retry tickets bind digest, actor budget, and topology publication",
        ),
        (
            "broadcast_recoverable",
            "if !target.membership.is_active() {",
            "if false && !target.membership.is_active() {",
            "broadcast fanout admits each active topology authority through an isolated target source",
        ),
        (
            "progress_ticket_request_digest",
            "let metadata = [0_u8, priority_tag(post.priority)];",
            "let metadata = [1_u8, priority_tag(post.priority)];",
            "canonical progress digest keeps Post and Broadcast request identities disjoint",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_local_actor_split_mutants(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    assert module._transport_geometry_production_source_fidelity_errors(repo_root) == []

    network_path = repo_root / "crates" / "iroha_p2p" / "src" / "network.rs"
    mutate_rust_item_source(module, network_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


def test_transport_geometry_rejects_ordinal_equivalent_weak_authority_mutant(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    network_path = repo_root / "crates" / "iroha_p2p" / "src" / "network.rs"
    mutate_rust_item_source_in_context(
        module,
        network_path,
        "matches",
        (("impl", "WeakProgressDeliveryAuthority"),),
        "Arc::ptr_eq(&retained, &candidate.tenure)",
        "retained.connection_ordinal == candidate.tenure.connection_ordinal",
    )

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(
        "weak progress authority matching must preserve exact Arc ownership"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "item_name", "old", "new", "expected_error"),
    (
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "validate_delivery_binding",
            "&& self.delivery_binding.delivery_ordinal == self.delivery_ordinal",
            "&& true",
            "reply-route validation rejects any substituted owner, ordinal, target, or minting tenure",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "peer_message",
            "NetworkReplyRoute::new(origin.clone(), tenure, delivery_ordinal)",
            "NetworkReplyRoute { semantic_target: origin.clone(), tenure, delivery_ordinal, delivery_binding: unreachable!() }",
            "authenticated local delivery mints the immutable binding through the reviewed constructor with one checked actor-global ordinal",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "source_update_from",
            "self.validate_delivery_binding()?;",
            "let _unchecked = &self.delivery_binding;",
            "per-source updates validate both actor-minted delivery bindings before classifying rank",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "try_from_route",
            "route.validate_delivery_binding()?;",
            "let _unchecked = &route.delivery_binding;",
            "route-set construction validates the actor-minted binding before importing a live capability",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "preflight_merge",
            "route.validate_delivery_binding()?;",
            "let _unchecked = &route.delivery_binding;",
            "strict route-set preflight validates every retained and candidate delivery binding",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "attach",
            "route.validate_delivery_binding()?;",
            "let _unchecked = &route.delivery_binding;",
            "single-route attachment validates the actor-minted binding before live-route admission",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "merge_retired_delivery",
            "retired.validate_delivery_binding()?;",
            "let _unchecked = &retired.delivery_binding;",
            "candidate tombstones validate their immutable binding and authority",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "reliable_progress_class",
            "return Some(ReliableProgressClass::Safety);",
            "return Some(ReliableProgressClass::Lane);",
            "public reliable-progress classes exactly refine actor reservations",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "source_key",
            "authenticated_via: self.tenure.delivery_peer.clone(),",
            "authenticated_via: self.semantic_target.clone(),",
            "reply fairness keys bind actor identity and authenticated delivery peer",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "source_update_from",
            "if self.delivery_ordinal < prior.delivery_ordinal {",
            "if false && self.delivery_ordinal < prior.delivery_ordinal {",
            "per-source delivery ordinals reject stale or forged equal-ordinal tenures",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "equal_ordinal_different_tenure",
            "&& !self.same_tenure(other)",
            "&& true",
            (
                "equal actor-global ordinals cannot be replayed under another "
                "connection tenure"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "retain_active",
            "self.attempts.retain(|_, route| route.is_active());",
            "self.attempts.retain(|_, _route| true);",
            (
                "owned route-set maintenance tombstones then releases only "
                "inactive connection tenures"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "retain_active",
            "self.record_retired_delivery(retired);",
            "drop(retired);",
            (
                "owned route-set maintenance tombstones then releases only "
                "inactive connection tenures"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "validate_after_retired_delivery",
            ".any(|retired| retired.equal_ordinal_different_tenure(route))",
            ".any(|_retired| false)",
            (
                "retired route history rejects forged equal ordinals and "
                "non-progressing same-source replay"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "validate_after_retired_delivery",
            "self.retired_attempts.get(&route.source_key())",
            "None",
            (
                "retired route history rejects forged equal ordinals and "
                "non-progressing same-source replay"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "merge",
            "for retired in candidate.retired_attempts.values().cloned() {\n"
            "            merged.merge_retired_delivery(retired)?;\n"
            "        }",
            "let _ = &candidate.retired_attempts;",
            (
                "strict route-set merge preflights then applies tombstones "
                "before live siblings on one atomic shadow copy"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "record_retired_delivery",
            "if retired.delivery_ordinal > current.delivery_ordinal {",
            "if retired.delivery_ordinal < current.delivery_ordinal {",
            (
                "retired route history remains source-bounded and monotonic by "
                "actor-global delivery ordinal"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "record_retired_delivery",
            "if self.retired_attempts.len() >= self.source_capacity",
            "if false && self.retired_attempts.len() >= self.source_capacity",
            (
                "retired route history remains source-bounded and monotonic by "
                "actor-global delivery ordinal"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "preflight_merge",
            ".any(|prior| prior.equal_ordinal_different_tenure(route))",
            ".any(|_prior| false)",
            (
                "strict route-set preflight validates every live and "
                "tombstoned candidate member before mutation"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "preflight_merge",
            ".any(|(_, other)| route.equal_ordinal_different_tenure(other))",
            ".any(|(_, _other)| false)",
            (
                "strict route-set preflight rejects internal equal-ordinal "
                "tenure collisions atomically"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "merge_retired_delivery",
            "retired.delivery_ordinal >= current.delivery_ordinal",
            "retired.delivery_ordinal < current.delivery_ordinal",
            (
                "candidate tombstones validate their immutable binding and authority and can release only "
                "a same-source live attempt at an equal or later ordinal"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "attach",
            ".any(|prior| prior.equal_ordinal_different_tenure(&route))",
            ".any(|_prior| false)",
            (
                "single-route attachment rejects equal actor-global ordinals "
                "under different tenures"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "source_update_from",
            "if !Arc::ptr_eq(&self.tenure.owner, &prior.tenure.owner) {",
            "if false && !Arc::ptr_eq(&self.tenure.owner, &prior.tenure.owner) {",
            "per-source updates reject inactive, foreign, retargeted, and cross-source capabilities",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "source_update_from",
            ".ok_or(NetworkReplyRouteError::EqualOrdinalDifferentTenure);",
            ".ok_or(NetworkReplyRouteError::Stale);",
            "per-source delivery ordinals reject stale or forged equal-ordinal tenures",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "attach",
            "if self.attempts.len() >= self.source_capacity {",
            "if false && self.attempts.len() >= self.source_capacity {",
            (
                "one source update tombstones only its prior delivery and a new "
                "source consumes one bounded attempt"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "merge",
            "let mut merged = self.clone();",
            "let mut merged = candidate.clone();",
            (
                "strict route-set merge preflights then applies tombstones "
                "before live siblings on one atomic shadow copy"
            ),
        ),
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "try_from_transport_with_reply_route",
            "if reply_route.semantic_target() != &sender {",
            "if false && reply_route.semantic_target() != &sender {",
            "transport reply authority must bind both semantic target and independently authenticated hop",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "try_from_transport_with_reply_route",
            "return Err(NetworkReplyRouteError::DifferentSource);",
            "return Err(NetworkReplyRouteError::Retargeted);",
            "transport reply authority must bind both semantic target and independently authenticated hop",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "transport_reply_route_construction_is_fallible_and_target_bound",
            "Err(NetworkReplyRouteError::DifferentSource)",
            "Err(NetworkReplyRouteError::Retargeted)",
            "authoritative transport reply-route regression must match exact reviewed token digest",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "try_push_at",
            "if retained.merge(candidate).is_err() {",
            "if false && retained.merge(candidate).is_err() {",
            "coalesced ingress shadow-merges one source route without mutating the retained owner",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "try_recv_if_at",
            "state.pending_wire_owners.remove(key)",
            "state.pending_wire_owners.get(key).cloned()",
            "semantic request ownership retires only when its queued occurrence is serviced",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "post_reply_recoverable_with_flush_ack_inner",
            "|| reply_route.validate_delivery_binding().is_err()",
            "|| false",
            "reply admission rejects retargeted, foreign-actor, or substituted delivery capabilities",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "post_reply_recoverable_with_flush_ack_inner",
            "target: Some(reply_route.tenure.delivery_peer.clone()),",
            "target: Some(reply_route.semantic_target().clone()),",
            (
                "reply admission accounts by authenticated delivery peer and "
                "transfers its exact flush sender"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "release_cancelled_targets",
            "entries.retain(|entry| !entry.cancelled_progress_authority());",
            "entries.retain(|_entry| true);",
            (
                "authority cancellation releases exact target deliveries and "
                "scheduler membership"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "cancel_reply_route",
            "ProgressAuthorityIdentity::Reply(tenure.connection_ordinal),",
            "ProgressAuthorityIdentity::Reply(tenure.connection_ordinal.saturating_add(1)),",
            "reply-tenure cancellation selects only the authenticated source, exact connection authority",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "cancel_authority_waiters",
            "target: Some(source_peer.clone()),",
            "target: None,",
            "waiter cancellation is isolated to one authenticated source, progress class, delivery kind, and exact authority",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "cancel_all_reply_route_tenures",
            "tenure.cancel();",
            "let _uncancelled = &tenure;",
            "actor teardown atomically takes only its own tenure map, retires each exact tenure",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "run",
            "let _ = self.cancel_all_reply_route_tenures();",
            "let _uncancelled = &self.reply_route_tenures;",
            "normal actor exit publishes exact route and waiter cancellation before terminating",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "cancel_reply_route_tenure",
            "tenure.cancel();",
            "let _uncancelled = &tenure;",
            "connection retirement cancels the exact route and every bound waiter",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "dispatch_reliable_actor_message",
            "if !current_writer || !current_tenure {",
            "if !current_writer && !current_tenure {",
            (
                "reply dispatch requires the exact current writer tenure or retires "
                "its owner"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "reattach_reply_route",
            "if self.reply_route.is_some()",
            "if false",
            (
                "peer-message reply-route reattachment rejects capability overwrite, "
                "retargeted authority, or retired authority"
            ),
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_reply_route_mutants(
    tmp_path: Path,
    relative: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    if item_name == "source_key" and relative == Path(
        "crates/iroha_p2p/src/network.rs"
    ):
        mutate_rust_item_source_in_context(
            module,
            repo_root / relative,
            item_name,
            (("impl", "NetworkReplyRoute"),),
            old,
            new,
        )
    else:
        mutate_rust_item_source(module, repo_root / relative, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "brace_context", "old", "new", "expected_error"),
    (
        (
            "new",
            (("impl", "NetworkReplyRoute"),),
            "owner: Arc::clone(&tenure.owner),",
            "owner: Arc::new(()),",
            "reply-route construction mints one immutable binding from the exact actor owner",
        ),
        (
            "new",
            (("impl", "NetworkReplyRoute"),),
            "minting_tenure: Arc::downgrade(&tenure),",
            "minting_tenure: Weak::new(),",
            "reply-route construction mints one immutable binding from the exact actor owner",
        ),
        (
            "drop",
            (
                (
                    "impl", "<", "T", ":", "Pload", ",", "K", ":", "Kex",
                    ",", "E", ":", "Enc", ">", "Drop", "for", "NetworkBase",
                    "<", "T", ",", "K", ",", "E", ">",
                ),
            ),
            "let _ = self.cancel_all_reply_route_tenures();",
            "let _uncancelled = &self.reply_route_tenures;",
            "network actor Drop reuses the centralized idempotent reply-tenure teardown",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_contextual_route_mutants(
    tmp_path: Path,
    item_name: str,
    brace_context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    network_path = repo_root / "crates/iroha_p2p/src/network.rs"
    mutate_rust_item_source_in_context(
        module,
        network_path,
        item_name,
        brace_context,
        old,
        new,
    )

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "pub fn is_authenticated_via(&self, peer: &PeerId) -> bool {",
            "pub(crate) fn is_authenticated_via(&self, peer: &PeerId) -> bool {",
            "public opaque authenticated-hop binding must remain public",
        ),
        (
            "&self.tenure.delivery_peer == peer",
            "&self.tenure.delivery_peer != peer",
            "public opaque authenticated-hop binding must match exact reviewed token digest",
        ),
        (
            "pub fn merge_observed(&mut self, candidate: &Self)",
            "fn merge_observed(&mut self, candidate: &Self)",
            "public atomic observed-history reconciliation must remain public",
        ),
        (
            "minting_tenure: Weak<ReliableReplyRouteTenure>,",
            "minting_tenure: Arc<ReliableReplyRouteTenure>,",
            "reply delivery occurrences retain an immutable actor, minting-tenure, semantic-target, and actor-global ordinal binding",
        ),
        (
            "delivery_binding: Arc<ReliableReplyDeliveryBinding>,",
            "delivery_binding: Option<Arc<ReliableReplyDeliveryBinding>>,",
            "opaque reply routes carry their immutable actor-minted delivery binding beside the selected tenure",
        ),
    ),
)
def test_transport_geometry_source_fidelity_binds_public_route_helpers(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    path = repo_root / "crates/iroha_p2p/src/network.rs"
    source = path.read_text(encoding="utf-8")
    assert source.count(old) == 1
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            ".release_retired_tenure_binding(peer_id, conn_id);",
            ".clear_generation(peer_id, conn_id);",
            "retired connection-generation-era clear_generation API must remain absent",
        ),
        (
            "retain_after_dispatch_attempt",
            "from_dispatch_parts",
            "retired reconstruction-era from_dispatch_parts API must remain absent",
        ),
        (
            "retired tenure binding lets a later live tenure service the same exact\n"
            "    /// frame without manufacturing or updating a reply capability.",
            "retired tenure binding means a successor session must be allowed to\n"
            "    /// reconstruct delivery instead of treating it as a stale duplicate.",
            "retired connection-generation-era wording",
        ),
        (
            "requester's durable source retains its exact retry state.",
            "requester's durable retry is its reconstruction path",
            "retired connection-generation-era wording",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_retired_generation_terminology(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    path = repo_root / "crates/iroha_p2p/src/network.rs"
    source = path.read_text(encoding="utf-8")
    assert source.count(old) >= 1
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new", "expected_error"),
    (
        (
            "post_reply_recoverable",
            "self.post_reply_recoverable_with_flush_ack(msg, reply_route, ticket)\n"
            "            .map(drop)",
            "self.post_reply_recoverable_with_flush_ack_inner(\n"
            "                msg, reply_route, ticket, || {},\n"
            "            )\n"
            "            .map(drop)",
            (
                "legacy unit-returning reply admission delegates to the flush-aware "
                "path before discarding its caller witness"
            ),
        ),
        (
            "post_reply_recoverable_with_flush_ack",
            "self.post_reply_recoverable_with_flush_ack_inner("
            "msg, reply_route, ticket, || {})",
            "Ok(None)",
            (
                "public reply completion admission delegates without bypassing the "
                "shared preflight and budget path"
            ),
        ),
        (
            "post_reply_recoverable_with_flush_ack_inner",
            "Some(reply_flush_sender),",
            "None,",
            (
                "reply admission accounts by authenticated delivery peer and "
                "transfers its exact flush sender"
            ),
        ),
        (
            "post_reply_recoverable_with_flush_ack_inner",
            "let identity = NetworkReplyFlushIdentity::from_admitted_ticket(ticket)\n"
            "                    .expect(\"reply admission must retain exact reply authority\");\n"
            "                NetworkReplyFlushAck::new(identity, reply_flush_receiver)",
            "let identity = NetworkReplyFlushIdentity::from_admitted_ticket(ticket)\n"
            "                    .expect(\"reply admission must retain exact reply authority\");\n"
            "                NetworkReplyFlushAck::new(forged_identity, reply_flush_receiver)",
            (
                "only a newly admitted exact ticket yields its immutable reply identity "
                "and live flush completion"
            ),
        ),
        (
            "submit_progress_message_to_source",
            "AdmittedNetworkMessage::new_targeted_post("
            "message, lease, authority, reply_flush_ack)",
            "AdmittedNetworkMessage::new_targeted_post("
            "message, lease, authority, None)",
            (
                "accepted direct replies transfer their exact flush sender while "
                "broadcasts cannot impersonate one"
            ),
        ),
        (
            "broadcast_recoverable",
            "target.actor_ticket.take(),\n                None,",
            "target.actor_ticket.take(),\n                Some(reply_flush_sender),",
            (
                "broadcast fanout admits each active topology authority through an "
                "isolated target source without a reply completion"
            ),
        ),
        (
            "into_dispatch_parts",
            "pending_flush_acks,\n            progress_authority,\n            reply_flush_ack,\n        )",
            "pending_flush_acks,\n            progress_authority,\n            None,\n        )",
            "dispatch tuple exports the exact reply completion sender without dropping it",
        ),
        (
            "retain_after_dispatch_attempt",
            "pending_flush_acks,\n            reply_flush_ack,",
            "pending_flush_acks,\n            reply_flush_ack: None,",
            "incomplete dispatch retains the exact reply completion sender without reconstructing authority",
        ),
        (
            "dispatch_reliable_actor_message",
            "if transferred {",
            "if true {",
            (
                "actor reply completion succeeds only after all exact writer flushes "
                "and survives every retry"
            ),
        ),
        (
            "dispatch_reliable_actor_message",
            "pending_flush_acks,\n                progress_authority,\n"
            "                reply_flush_ack,\n            ))",
            "pending_flush_acks,\n                progress_authority,\n"
            "                None,\n            ))",
            (
                "actor reply completion succeeds only after all exact writer flushes "
                "and survives every retry"
            ),
        ),
        (
            "poll",
            "self.terminal = Some(NetworkReplyFlushAckStatus::Closed);",
            "self.terminal = Some(NetworkReplyFlushAckStatus::Flushed);",
            (
                "only a successful writer signal yields Flushed while closure remains "
                "a distinct terminal failure"
            ),
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_reply_flush_ack_mutants(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    network_path = repo_root / "crates" / "iroha_p2p" / "src" / "network.rs"
    mutate_rust_item_source(module, network_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "item_name", "context", "old", "new", "expected_error"),
    (
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "new",
            (("impl", "NetworkReplyFlushAck"),),
            "identity,\n            receiver: Some(receiver),\n            terminal: None,",
            "receiver: Some(receiver),\n            terminal: None,",
            (
                "new reply completion starts pending with the exact admitted identity "
                "and actor-owned receiver"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "new_targeted_post",
            (("impl", "<", "T", ">", "AdmittedNetworkMessage", "<", "T", ">"),),
            "pending_flush_acks: HashMap::new(),\n            reply_flush_ack,",
            "pending_flush_acks: HashMap::new(),\n            reply_flush_ack: None,",
            (
                "targeted actor-post construction keeps the reply completion "
                "beside its exact lease and authority"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "push_back",
            (
                (
                    "impl",
                    "<",
                    "T",
                    ":",
                    "message",
                    "::",
                    "ClassifyTopic",
                    ">",
                    "ReliableActorPending",
                    "<",
                    "T",
                    ">",
                ),
            ),
            "entries.push_back(message);",
            "drop(message);",
            (
                "actor backlog insertion preserves the complete admitted owner "
                "under its exact source"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "retry_back",
            (
                (
                    "impl",
                    "<",
                    "T",
                    ":",
                    "message",
                    "::",
                    "ClassifyTopic",
                    ">",
                    "ReliableActorPending",
                    "<",
                    "T",
                    ">",
                ),
            ),
            "self.push_back(message);",
            "drop(message);",
            "actor retry preserves the same source and opaque completion-bearing owner",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "defer_high_priority_network_message",
            (),
            "if sender.send(message).await.is_err() {",
            "drop(message);\n        if false {",
            (
                "deferred actor admission moves the complete opaque message "
                "owner into its bounded task"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "new",
            (("impl", "OutboundPostOwnership"),),
            "_byte_lease: byte_lease,\n            flush_ack,",
            "_byte_lease: byte_lease,\n            flush_ack: None,",
            (
                "peer-writer ownership constructor keeps the byte lease and "
                "optional flush sender inseparable"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "new",
            (("impl", "<", "T", ">", "RetainedPost", "<", "T", ">"),),
            "message: Some(message),\n            ownership,",
            "message: Some(message),\n            ownership: panic!(),",
            (
                "peer mailbox constructor retains the exact message beside its "
                "writer ownership"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "into_parts",
            (("impl", "<", "T", ">", "RetainedPost", "<", "T", ">"),),
            "ownership,\n        )",
            "panic!(),\n        )",
            (
                "peer mailbox extraction returns the exact message and writer "
                "ownership together"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "prepare_owned_or_defer",
            (
                ("mod", "run"),
                ("impl", "<", "E", ":", "Enc", ">", "MessageSender", "<", "E", ">"),
            ),
            "vec![ownership.into()]",
            "Vec::new()",
            (
                "peer plaintext admission transfers the exact mailbox owner into "
                "one writer-owned vector"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "retry_deferred",
            (
                ("mod", "run"),
                ("impl", "<", "E", ":", "Enc", ">", "MessageSender", "<", "E", ">"),
            ),
            "let scratch_ownership = "
            "core::mem::replace(&mut self.buffer_ownership, ownership);",
            "let scratch_ownership = "
            "core::mem::replace(&mut self.buffer_ownership, Vec::new());",
            "deferred retry restores its exact bytes and flush owners into the encoder",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "send_one_ready_stream",
            (("mod", "run"),),
            "(true, false) => Some((false, high.send().await)),",
            "(true, false) => Some((false, Ok(()))),",
            "single ready peer stream still enters the reviewed writer-flush kernel",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "next_peer_stream_io",
            (("mod", "run"),),
            "let send = send_one_ready_stream(high_sender, low_sender, prefer_low_send);",
            "let send = async { Some((false, Ok(()))) };",
            (
                "full-duplex peer IO delegates its outbound branch to the "
                "reviewed ready-stream writer"
            ),
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "run",
            (("mod", "run"),),
            "stream_io = next_peer_stream_io(",
            "stream_io = unreviewed_peer_stream_io(",
            (
                "peer task routes ready writer work through the reviewed "
                "full-duplex IO seam"
            ),
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_flush_owner_handoff_mutants(
    tmp_path: Path,
    relative: Path,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    mutate_rust_item_source_in_context(
        module,
        repo_root / relative,
        item_name,
        context,
        old,
        new,
    )

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


def test_transport_geometry_source_fidelity_rejects_success_signalling_owner_drop(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    peer_path = repo_root / "crates" / "iroha_p2p" / "src" / "peer.rs"
    source = peer_path.read_text(encoding="utf-8")
    marker = "impl From<SharedByteLease> for OutboundPostOwnership {"
    assert source.count(marker) == 1
    mutant = """
impl Drop for OutboundPostOwnership {
    fn drop(&mut self) {
        if let Some(flush_ack) = self.flush_ack.take() {
            let _ = flush_ack.send(());
        }
    }
}

"""
    peer_path.write_text(source.replace(marker, mutant + marker), encoding="utf-8")

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(
        "OutboundPostOwnership must close a flush witness by ordinary sender drop"
        in error
        for error in errors
    ), errors


def test_transport_geometry_source_fidelity_rejects_progress_lease_drop_digest_mutant(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    assert module._transport_geometry_production_source_fidelity_errors(repo_root) == []

    network_path = repo_root / "crates" / "iroha_p2p" / "src" / "network.rs"
    source = network_path.read_text(encoding="utf-8")
    region_start = source.index("impl Drop for NetworkActorProgressLease")
    mutation = source.index(
        "retained.request_digest, self.request_digest,", region_start
    )
    old = "retained.request_digest, self.request_digest,"
    new = "self.request_digest, self.request_digest,"
    network_path.write_text(
        source[:mutation] + new + source[mutation + len(old) :],
        encoding="utf-8",
    )

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(
        "progress lease drop releases only the same digest, kind, and delivery authority"
        in error
        for error in errors
    ), errors

    # Exercise every H/R split seam in one additional copied workspace so the
    # fixed proof-fidelity test count does not grow with the mutation matrix.
    geometry_formal_dir = copy_async_source_fidelity_fixture(
        tmp_path / "h_geometry", module, "SumeragiV2AsyncNetwork.tla"
    )
    geometry_root = geometry_formal_dir.parents[2]
    core_path = geometry_root / "crates/iroha_core/src/sumeragi/mod.rs"
    core_source = core_path.read_text(encoding="utf-8")
    core_source = core_source.replace(
        "    Authenticated(PeerId),\n    Anonymous,",
        "    Authenticated,\n    Anonymous,",
        1,
    )
    core_path.write_text(core_source, encoding="utf-8")
    mutate_rust_item_source(
        module,
        core_path,
        "fair_v2_ingress_required_capacity",
        "authenticated_non_validator_source_capacity\n"
        "                .checked_mul(2)",
        "authenticated_non_validator_source_capacity\n"
        "                .checked_mul(1)",
    )
    mutate_rust_item_source(
        module,
        core_path,
        "fair_v2_ingress_required_byte_capacity",
        ".checked_add(authenticated_non_validator_source_capacity.unwrap_or(0))",
        ".checked_add(0)",
    )
    mutate_rust_item_source(
        module,
        core_path,
        "try_push_at",
        "let source_lane_is_new = !state.lanes.contains_key(&source);",
        "let source_lane_is_new = false;",
    )
    mutate_rust_item_source(
        module,
        core_path,
        "try_push_at",
        "let retained_authenticated_non_validator_sources = state\n"
        "                .lanes\n"
        "                .keys()\n"
        "                .filter(|source| matches!(source, FairV2IngressSource::Authenticated(_)))\n"
        "                .count();",
        "let retained_authenticated_non_validator_sources = state\n"
        "                .lanes\n"
        "                .keys()\n"
        "                .count();",
    )
    mutate_rust_item_source(
        module,
        core_path,
        "try_recv_if_at",
        "} else if matches!(&source, FairV2IngressSource::Authenticated(_)) {",
        "} else if false && matches!(&source, FairV2IngressSource::Authenticated(_)) {",
    )
    mutate_rust_item_source(
        module,
        core_path,
        "start",
        "let authenticated_non_validator_source_capacity =\n"
        "            config.queues.authenticated_non_validator_sources.get();",
        "let authenticated_non_validator_source_capacity =\n"
        "            network.reply_route_source_capacity();",
    )

    defaults_path = geometry_root / "crates/iroha_config/src/parameters/defaults.rs"
    defaults_source = defaults_path.read_text(encoding="utf-8")
    defaults_source = defaults_source.replace(
        "+ 2 * QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get()",
        "+ QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get()",
        1,
    )
    defaults_path.write_text(defaults_source, encoding="utf-8")

    actual_path = geometry_root / "crates/iroha_config/src/parameters/actual.rs"
    actual_source = actual_path.read_text(encoding="utf-8")
    actual_source = actual_source.replace(
        "body_queue_capacity,\n                "
        "authenticated_non_validator_source_capacity,\n                body_bytes,",
        "body_queue_capacity,\n                "
        "authenticated_non_validator_source_capacity: 0,\n                body_bytes,",
        1,
    )
    actual_path.write_text(actual_source, encoding="utf-8")

    user_path = geometry_root / "crates/iroha_config/src/parameters/user.rs"
    user_source = user_path.read_text(encoding="utf-8")
    user_source = user_source.replace(
        "sumeragi.queues.authenticated_non_validator_sources.get() "
        "> reply_source_capacity",
        "sumeragi.queues.authenticated_non_validator_sources.get() "
        ">= reply_source_capacity",
        1,
    )
    user_source = user_source.replace(
        ".or(lane_profile.derived_limits().max_total_connections)",
        ".or(None)",
        1,
    )
    user_path.write_text(user_source, encoding="utf-8")

    kagami_path = geometry_root / "crates/iroha_kagami/src/localnet.rs"
    mutate_rust_item_source(
        module,
        kagami_path,
        "localnet_sumeragi_body_bytes",
        ".checked_add(LOCALNET_SUMERAGI_AUTHENTICATED_NON_VALIDATOR_SOURCES)",
        ".checked_add(0)",
    )

    renderer_path = geometry_root / "scripts/render_taira_validator_bundle.py"
    renderer_source = renderer_path.read_text(encoding="utf-8")
    renderer_source = renderer_source.replace(
        "validator_count + authenticated_non_validator_sources + 1",
        "validator_count + 1",
        1,
    )
    renderer_path.write_text(renderer_source, encoding="utf-8")

    for relative in (
        Path("defaults/kagami/iroha3-taira/config.toml"),
        Path("configs/soranexus/taira/config.toml"),
    ):
        path = geometry_root / relative
        source = path.read_text(encoding="utf-8")
        path.write_text(
            source.replace("authenticated_non_validator_sources = 2", "", 1),
            encoding="utf-8",
        )
    readme_path = geometry_root / "configs/soranexus/taira/README.md"
    readme_source = readme_path.read_text(encoding="utf-8")
    readme_path.write_text(
        readme_source.replace(
            "validator_count + authenticated_non_validator_sources + 1",
            "validator_count + 1",
            1,
        ),
        encoding="utf-8",
    )

    geometry_errors = module._transport_geometry_production_source_fidelity_errors(
        geometry_root
    )
    for expected_error in (
        "three-way fair-ingress source ownership inventory",
        "semantic duplicate route attachment precedes authenticated non-validator lane-cap admission",
        "authenticated non-validator lane cap excludes validator and anonymous lanes",
        "empty authenticated non-validator lanes release their bounded churn slot",
        "exact default 4N+2H+2 outer-ingress message geometry",
        "production H comes from Sumeragi ingress configuration rather than reply-route R",
        "root configuration derives R from the effective explicit or lane-profile network geometry",
        "root configuration rejects H greater than exact-output reply-source R",
        "shared Sumeragi fingerprint projection carries H beside ingress capacities",
        "localnet aggregate bytes scale by N+H+1",
        "Taira renderer scales aggregate bytes by N+H+1",
        "default Taira profile pins H=2 and seven source partitions",
        "production Taira profile pins H=2 and seven source partitions",
        "Taira operator documentation states N+H+1 byte scaling",
    ):
        assert any(expected_error in error for error in geometry_errors), (
            expected_error,
            geometry_errors,
        )


def test_transport_geometry_source_fidelity_rejects_sm_distid_bit_length_mutant(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    sm_path = repo_root / "crates" / "iroha_crypto" / "src" / "sm.rs"
    mutate_rust_item_source(
        module,
        sm_path,
        "validate_distid",
        ".checked_mul(8)",
        ".checked_mul(1)",
    )

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(
        "SM2 distinguishing-identifier geometry validate_distid" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("item_name", "old", "new"),
    (
        (
            "fair_v2_ingress_required_capacity",
            ".checked_mul(4)",
            ".checked_mul(3)",
        ),
        (
            "fair_v2_ingress_lane_protected_slots",
            "4_usize.saturating_sub(depth)",
            "3_usize.saturating_sub(depth)",
        ),
        (
            "fair_v2_ingress_required_manifest_bytes",
            ".checked_add(228)",
            ".checked_add(227)",
        ),
        (
            "fair_v2_ingress_required_quorum_certificate_bytes",
            ".checked_add(fair_v2_ingress_framed_bytes(signer_vector_bytes)?)?",
            ".checked_add(0)?",
        ),
        (
            "fair_v2_ingress_required_proposal_bytes",
            "let timeout_group_vector_bytes = roster_len\n"
            "            .checked_mul(framed_timeout_group_bytes)?",
            "let timeout_group_vector_bytes = framed_timeout_group_bytes",
        ),
        (
            "fair_v2_ingress_required_p2p_frame_bytes",
            "Some(iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES)",
            "Some(32)",
        ),
        (
            "fair_v2_ingress_required_recovery_request_bytes_for_key",
            "Some(certified_body_request.max(commit_certificate_request))",
            "Some(commit_certificate_request)",
        ),
        (
            "fair_v2_ingress_required_commit_certificate_response_bytes_for_key",
            ".checked_add(responder_bytes)?",
            ".checked_add(0)?",
        ),
        (
            "fair_v2_ingress_required_transport_completion_bytes",
            ".checked_add(fair_v2_ingress_framed_bytes(encoded_body_bytes)?)?",
            ".checked_add(encoded_body_bytes)?",
        ),
        (
            "configure_roster_for_context",
            "required_proposal_bytes.max(required_commit_certificate_response_bytes)",
            "required_proposal_bytes",
        ),
        (
            "configure_roster_for_context",
            ".max(required_recovery_request_bytes),",
            ",",
        ),
        (
            "configure_roster_for_context",
            "required: usize::MAX,",
            "required: 0,",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_short_exact_progress_bound(
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    core_path = repo_root / "crates" / "iroha_core" / "src" / "sumeragi" / "mod.rs"
    mutate_rust_item_source(module, core_path, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(
        "authoritative fair-v2 ingress geometry" in error for error in errors
    ), errors


@pytest.mark.parametrize(
    ("item_name", "context", "old", "new", "expected_error"),
    (
        (
            "classify",
            (("impl", "FairV2IngressClass"),),
            "Self::classify_message(inbound.message())",
            "Self::Auxiliary",
            "authoritative fair-v2 ingress geometry classify",
        ),
        (
            "try_push_at",
            (("impl", "FairV2Ingress"),),
            "        if is_transport_completion && !is_validator_origin {\n"
            "            return Err(FairV2IngressPushError::Rejected(inbound));\n"
            "        }\n",
            "",
            "roster-origin premise for completion relayed through any authenticated hop",
        ),
        (
            "try_recv_if_at",
            (("impl", "FairV2Ingress"),),
            "if entry.class == FairV2IngressClass::TransportCompletion {",
            "if false && entry.class == FairV2IngressClass::TransportCompletion {",
            "exact shared transport-completion owner retirement",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_completion_owner_mutants(
    tmp_path: Path,
    item_name: str,
    context: tuple[tuple[str, ...], ...],
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    core_path = repo_root / "crates" / "iroha_core" / "src" / "sumeragi" / "mod.rs"
    mutate_rust_item_source_in_context(
        module, core_path, item_name, context, old, new
    )

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("item_name", "required_field", "capacity_field", "kind"),
    tuple(
        (item_name, required_field, capacity_field, kind)
        for item_name in ("configure_roster_with_byte_requirements", "open")
        for required_field, capacity_field, kind in (
            (
                "required_consensus_frame_bytes",
                "consensus_frame_byte_capacity",
                "ConsensusFrameBytes",
            ),
            (
                "required_control_frame_bytes",
                "control_frame_byte_capacity",
                "ControlFrameBytes",
            ),
            (
                "required_block_sync_frame_bytes",
                "block_sync_frame_byte_capacity",
                "BlockSyncFrameBytes",
            ),
            (
                "required_outbound_high_frame_bytes",
                "outbound_high_frame_byte_capacity",
                "OutboundHighFrameBytes",
            ),
        )
    ),
)
def test_transport_geometry_source_fidelity_requires_configure_and_open_rechecks(
    tmp_path: Path,
    item_name: str,
    required_field: str,
    capacity_field: str,
    kind: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    core_path = repo_root / "crates" / "iroha_core" / "src" / "sumeragi" / "mod.rs"
    guard = f"""        if state.{required_field} > self.{capacity_field} {{
            return Err(FairV2IngressCapacityError {{
                configured: self.{capacity_field},
                required: state.{required_field},
                kind: FairV2IngressCapacityKind::{kind},
            }});
        }}
"""
    mutate_rust_item_source(module, core_path, item_name, guard, "")

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    stage = "configure" if item_name.startswith("configure") else "open"
    assert any(f"{stage} recheck for {kind}" in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "item_name", "old", "new", "expected_error"),
    (
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "start",
            "iroha_p2p::frame_plaintext_cap(max_frame_bytes)",
            "usize::MAX",
            "encrypted global and three plaintext progress-topic cap intersection",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "start",
            ".min(max_frame_bytes_control)",
            ".min(usize::MAX)",
            "encrypted global and three plaintext progress-topic cap intersection",
        ),
        (
            Path("crates/iroha_core/src/sumeragi/mod.rs"),
            "start",
            "block_sync_frame_byte_capacity,\n"
            "            outbound_frame_queue_max_high_bytes,",
            "block_sync_frame_byte_capacity,\n            usize::MAX,",
            "production fair-ingress construction with configured H and every progress cap",
        ),
        (
            Path("crates/irohad/src/main.rs"),
            "start",
            "max_frame_bytes: config.network.max_frame_bytes,",
            "max_frame_bytes: usize::MAX,",
            "daemon-to-Sumeragi global/topic/high-queue cap hand-off",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "start_with_crypto",
            "let transport_geometry = validate_transport_queue_geometry::<E>(",
            "let transport_geometry = unchecked_transport_queue_geometry::<E>(",
            "complete transport geometry validation must be the first P2P startup action before any listener bind",
        ),
        (
            Path("crates/irohad/src/main.rs"),
            "validate_config",
            "validate_network_frame_runtime_limit(config)?;",
            "let _ = config;",
            "validate_config deterministic frame ceiling before IO/runtime probes",
        ),
        (
            Path("crates/irohad/src/main.rs"),
            "validate_config_offline",
            "validate_network_frame_runtime_limit(config)?;",
            "let _ = config;",
            "validate_config_offline deterministic frame ceiling before IO/runtime probes",
        ),
        (
            Path("crates/irohad/src/main.rs"),
            "start",
            ".p2p_outbound_frame_queue_max_high_bytes\n                .get(),",
            ".p2p_outbound_frame_queue_max_low_bytes\n                .get(),",
            "daemon-to-Sumeragi global/topic/high-queue cap hand-off",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_startup_cap_bypass(
    tmp_path: Path,
    relative: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    mutate_rust_item_source(module, repo_root / relative, item_name, old, new)

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "nonzero!(128 * 1024 * 1024_usize)",
            "nonzero!(16 * 1024 * 1024_usize)",
            "high-priority encrypted-frame byte reserve",
        ),
        (
            "nonzero!(17 * 1024 * 1024_usize)",
            "nonzero!(16 * 1024 * 1024_usize)",
            "encrypted global consensus frame ceiling",
        ),
        (
            "pub const MAX_FRAME_BYTES_CONSENSUS: NonZeroUsize = MAX_FRAME_BYTES;",
            "pub const MAX_FRAME_BYTES_CONSENSUS: NonZeroUsize = "
            "MAX_FRAME_BYTES_CONTROL;",
            "consensus-recovery frame ceiling",
        ),
        (
            "nonzero!(2 * 1024 * 1024_usize)",
            "nonzero!(1024 * 1024_usize)",
            "consensus-safety frame ceiling",
        ),
        (
            "pub const MAX_FRAME_BYTES_BLOCK_SYNC: NonZeroUsize = MAX_FRAME_BYTES;",
            "pub const MAX_FRAME_BYTES_BLOCK_SYNC: NonZeroUsize = "
            "MAX_FRAME_BYTES_CONTROL;",
            "payload-completion frame ceiling",
        ),
        (
            "nonzero!(\n"
            "        4 * MAX_VALIDATORS_PER_HEIGHT\n"
            "            + 2 * QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get()\n"
            "            + 2\n"
            "    )",
            "nonzero!(\n"
            "        3 * MAX_VALIDATORS_PER_HEIGHT\n"
            "            + 2 * QUEUE_AUTHENTICATED_NON_VALIDATOR_SOURCE_CAPACITY.get()\n"
            "            + 2\n"
            "    )",
            "exact default 4N+2H+2 outer-ingress message geometry",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_shortened_default_cap(
    tmp_path: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    defaults_path = (
        repo_root / "crates" / "iroha_config" / "src" / "parameters" / "defaults.rs"
    )
    source = defaults_path.read_text(encoding="utf-8")
    assert source.count(old) == 1, old
    defaults_path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "old", "new", "expected_error"),
    (
        (
            Path("crates/iroha_p2p/src/lib.rs"),
            "pub const MAX_WIRE_ENCRYPTED_FRAME_BYTES: usize = u32::MAX as usize;",
            "pub const MAX_WIRE_ENCRYPTED_FRAME_BYTES: usize = u16::MAX as usize;",
            "exact u32 encrypted-frame wire-body ceiling",
        ),
        (
            Path("crates/iroha_p2p/src/lib.rs"),
            "pub const MAX_ENCRYPTED_FRAME_BYTES: usize = 2_147_483_643;",
            "pub const MAX_ENCRYPTED_FRAME_BYTES: usize = u32::MAX as usize;",
            "deterministic cross-platform encrypted-frame runtime ceiling",
        ),
        (
            Path("crates/iroha_p2p/src/network.rs"),
            "if max_frame_bytes > crate::MAX_ENCRYPTED_FRAME_BYTES {",
            "if max_frame_bytes >= crate::MAX_ENCRYPTED_FRAME_BYTES {",
            "inclusive deterministic encrypted-frame runtime limit",
        ),
        (
            Path("crates/irohad/src/main.rs"),
            "if configured > iroha_p2p::MAX_ENCRYPTED_FRAME_BYTES {",
            "if configured >= iroha_p2p::MAX_ENCRYPTED_FRAME_BYTES {",
            "inclusive daemon deterministic encrypted-frame runtime limit",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            ".min(crate::MAX_ENCRYPTED_FRAME_BYTES)\n"
            "            .saturating_sub(core::mem::size_of::<aead::Nonce<E>>())",
            ".min(usize::MAX)\n"
            "            .saturating_sub(core::mem::size_of::<aead::Nonce<E>>())",
            "generic AEAD P2P preflight frame_plaintext_cap_for",
        ),
        (
            Path("crates/iroha_crypto/src/lib.rs"),
            "pub const MAX_PUBLIC_KEY_PAYLOAD_BYTES: usize = "
            "2 + (u16::MAX as usize / 8) + 65;",
            "pub const MAX_PUBLIC_KEY_PAYLOAD_BYTES: usize = 32;",
            "protocol-wide maximum public-key payload geometry",
        ),
        (
            Path("crates/iroha_data_model/src/block/consensus_v2.rs"),
            "pub const MAX_VALIDATORS_PER_HEIGHT: usize = 128;",
            "pub const MAX_VALIDATORS_PER_HEIGHT: usize = 127;",
            "first-release maximum validator geometry",
        ),
        (
            Path("crates/iroha_p2p/src/peer.rs"),
            "let size = buf.get_u32() as usize;\n"
            "            if size > "
            "self.max_frame_bytes.min(crate::MAX_ENCRYPTED_FRAME_BYTES) {",
            "let size = buf.get_u32() as usize;\n"
            "            if size > self.max_frame_bytes {",
            "runtime-clamped receiver parse boundary",
        ),
    ),
)
def test_transport_geometry_source_fidelity_rejects_cap_threading_mutants(
    tmp_path: Path,
    relative: Path,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    repo_root = formal_dir.parents[2]
    path = repo_root / relative
    source = path.read_text(encoding="utf-8")
    assert old in source, old
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._transport_geometry_production_source_fidelity_errors(repo_root)
    assert any(expected_error in error for error in errors), errors


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
