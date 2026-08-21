"""Focused tests for bounded Sumeragi v2 TLAPS proof sharding."""

from __future__ import annotations

import hashlib
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

RECOVERY_BOUNDARY_CONTINUATION = (
    "SumeragiV2AsyncRecoveryVoteEpochBoundaryContinuationProofs"
)
RECOVERY_BOUNDARY_CONTINUATION_THEOREMS = (
    "AsyncTimeoutRecoveryEpisodeAfterTransitionPreservesCurrentBoundary",
    "AsyncTimeoutRecoveryRetainedEpisodesHaveCurrentBoundary",
    "AsyncTimeoutRecoveryNewEpisodeDecomposition",
    "AsyncTimeoutRecoveryNewBaseEpisodeInHasCurrentBoundary",
    "AsyncTimeoutRecoveryNewEpisodeInHasCurrentBoundary",
    "AsyncTimeoutRecoveryNewEpisodesHaveCurrentBoundary",
    "AsyncTimeoutRecoveryEpisodeUnionEstablishesCurrentBoundary",
    "AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionPreservesCurrentBoundary",
    "AsyncTimeoutRecoveryVoteOwnerImagePreservesCurrentBoundary",
    "AsyncControlServiceTypeProjectsTimeoutRecoveryEpisodeSet",
    "AsyncControlServiceResetPreservesTimeoutRecoveryEpisodeSet",
    "AsyncControlServiceSlotTransitionEstablishesTimeoutRecoveryCurrentBoundary",
    "AsyncNextPreservesTimeoutRecoveryCurrentBoundaryInvariant",
    "AsyncNextPreservesStrongTypeInvariant",
    "AsyncBracketNextPreservesStrongTypeInvariant",
    "RetainedBodyRebindCandidateIsTypedAndOwned",
    "DeliverProposalSchedulesRetainedBodyRebind",
    "RebindSchedulesCurrentRoundStore",
    "StoreSchedulesCurrentRoundValidation",
    "ValidationSchedulesPrepareAndLockedCommitAttempts",
    "PrepareQcDeliverySchedulesCompletionLockedCommitAttempt",
    "PersistedPrepareObservationSchedulesCompletionLockedCommitAttempt",
    "ReadyRetainedBodyRebindEnablesExecution",
    "ReadyRetainedBodyRebindIsDispatchable",
    "RebindCommandSelectsRetainedRebind",
    "ExecuteRebindStagesCurrentRoundBody",
    "ValidationCommandSelectsValidationAction",
    "ExecuteValidationBindsCurrentViewAndGeneration",
    "PrepareVoteAdmissionIsCurrentView",
    "CommitVoteAdmissionIsExactLockedCommit",
    "CommitFormationIsExactLockedRound",
)


def _sources() -> dict[str, str]:
    modules = [name for name, _ in checker.ASYNC_LIVENESS_SHARDS]
    modules.append(checker.ASYNC_LIVENESS_FACADE)
    return {
        module: (checker.FORMAL_DIR / f"{module}.tla").read_text(encoding="utf-8")
        for module in modules
    }


def test_checked_in_shards_are_bounded_ordered_and_uniquely_resolved() -> None:
    sources = _sources()
    errors, providers = checker._async_liveness_shard_contract(sources)

    assert errors == []
    assert sum(
        kind == "theorem"
        for _, kind, _, _ in checker._top_level_declarations(
            sources["SumeragiV2AsyncRankAndInitProofs"]
        )
    ) == checker.ASYNC_LIVENESS_SHARD_MAX_THEOREMS
    assert providers["ModelResponsiveValidators"] == (
        "SumeragiV2AsyncRankAndInitContinuationProofs"
    )
    assert providers["AsyncTypeInvariantObligation"] == (
        "SumeragiV2AsyncProtectedSlotProofs"
    )
    assert providers["ApplicationCompletionProgressObligation"] == (
        "SumeragiV2AsyncDecisionApplicationProofs"
    )
    assert providers["ApplicationLivenessObligation"] == (
        "SumeragiV2AsyncDecisionApplicationProofs"
    )
    assert "TimeoutViewProgressObligation" not in providers
    assert "RotatingLeaderProgressObligation" not in providers
    assert "LockedBodyReproposalProgressObligation" not in providers
    assert providers["HistoricalRecoveryFramePreservesType"] == (
        "SumeragiV2AsyncRankAndInitContinuationProofs"
    )
    assert providers["IngressAdmissionRunnerPreservesSchedulerType"] == (
        "SumeragiV2AsyncRuntimeAdmissionTypeContinuationProofs"
    )
    assert providers["AsyncSpecAlwaysKeepsFrozenContext"] == (
        "SumeragiV2AsyncInstallRunnerContinuationProofs"
    )
    assert providers["HistoricalLockedBodyRecoveryProperty"] == (
        "SumeragiV2AsyncRecoveryVoteEpochContinuationProofs"
    )


def test_recovery_vote_epoch_boundary_is_exact_and_provider_safe() -> None:
    sources = _sources()
    recovery = "SumeragiV2AsyncRecoveryVoteEpochProofs"
    boundary_continuation = RECOVERY_BOUNDARY_CONTINUATION
    continuation = "SumeragiV2AsyncRecoveryVoteEpochContinuationProofs"
    fair_service = "SumeragiV2AsyncFairServiceProofs"
    footer = "=============================================================================\n"

    assert checker.ASYNC_LIVENESS_SHARD_MAX_THEOREMS == 150
    assert recovery not in checker.ASYNC_LIVENESS_SHARD_REVIEWED_MAX_THEOREMS
    assert (
        boundary_continuation not in checker.ASYNC_LIVENESS_SHARD_REVIEWED_MAX_THEOREMS
    )
    assert continuation not in checker.ASYNC_LIVENESS_SHARD_REVIEWED_MAX_THEOREMS
    assert (
        sum(
            kind == "theorem"
            for _, kind, _, _ in checker._top_level_declarations(sources[recovery])
        )
        == 146
    )
    boundary_continuation_theorems = tuple(
        name
        for name, kind, _, _ in checker._top_level_declarations(
            sources[boundary_continuation]
        )
        if kind == "theorem"
    )
    assert boundary_continuation_theorems == RECOVERY_BOUNDARY_CONTINUATION_THEOREMS
    assert (
        sum(
            kind == "theorem"
            for _, kind, _, _ in checker._top_level_declarations(sources[continuation])
        )
        == 28
    )
    corridor_theorems = [
        name
        for module in (recovery, boundary_continuation, continuation)
        for name, kind, _, _ in checker._top_level_declarations(sources[module])
        if kind == "theorem"
    ]
    assert len(corridor_theorems) == len(set(corridor_theorems))

    recovery_header = (
        f"---- MODULE {recovery} ----\nEXTENDS SumeragiV2AsyncTimeoutKernelProofs\n\n"
    )
    boundary_continuation_header = (
        f"---- MODULE {boundary_continuation} ----\nEXTENDS {recovery}\n\n"
    )
    continuation_header = (
        f"---- MODULE {continuation} ----\nEXTENDS {boundary_continuation}\n\n"
    )
    assert sources[recovery].startswith(recovery_header)
    assert sources[boundary_continuation].startswith(boundary_continuation_header)
    assert sources[continuation].startswith(continuation_header)
    assert sources[recovery].endswith(footer)
    assert sources[boundary_continuation].endswith(footer)
    assert sources[continuation].endswith(footer)
    combined_body = (
        sources[recovery][len(recovery_header) : -len(footer)]
        + sources[boundary_continuation][
            len(boundary_continuation_header) : -len(footer)
        ]
        + sources[continuation][len(continuation_header) : -len(footer)]
    )
    assert hashlib.sha256(combined_body.encode("utf-8")).hexdigest() == (
        "b3c47ede1eb792052dc7519df34ee070186415793364178666269592bbe63505"
    )

    errors, providers = checker._async_liveness_shard_contract(sources)
    assert errors == []
    assert all(
        providers[theorem] == boundary_continuation
        for theorem in RECOVERY_BOUNDARY_CONTINUATION_THEOREMS
    )
    assert providers["AsyncNextPreservesCandidateLifecycleSchedulerCoverage"] == recovery
    for theorem in (
        "AsyncTimeoutRecoveryEpisodeAfterTransitionPreservesCurrentBoundary",
        "AsyncNextPreservesStrongTypeInvariant",
        "RetainedBodyRebindCandidateIsTypedAndOwned",
        "CommitFormationIsExactLockedRound",
    ):
        assert providers[theorem] == boundary_continuation
    for theorem in (
        "AsyncServeProducerTurnTransitionPreservesTypeInvariant",
        "AsyncStrongTypeProjectsControlServiceStateType",
        "FreshReplayCandidateIsDisjointFromScheduled",
        "ReplayingOrdinaryStepPreservesRecoveryCorridor",
        "HistoricalVoteAdmissionIsExactLockedCommit",
        "HistoricalCommitFormationIsExactLockedRound",
        "HistoricalLockedCommitUsesProgressReserve",
        "HistoricalBeginLockExecutionCreatesSameRefPending",
    ):
        assert providers[theorem] == continuation
    assert checker._module_extends(sources[boundary_continuation]) == (recovery,)
    assert checker._module_extends(sources[continuation]) == (boundary_continuation,)
    assert checker._module_extends(sources[fair_service]) == (continuation,)
    sources[recovery] = sources[recovery].replace(
        footer,
        "".join(
                f"THEOREM ReviewedRecoveryCeilingMutation{index} == TRUE\nBY PTL\n\n"
                for index in range(5)
        ) + footer,
        1,
    )
    errors, _ = checker._async_liveness_shard_contract(sources)
    assert any(
        f"{recovery}.tla exceeds 150 top-level theorems: found 151" in error
        for error in errors
    ), errors

    base_seals = checker._TIMEOUT_VOTE_EPISODE_TLA_THEOREM_SHA256[f"{recovery}.tla"]
    boundary_continuation_seals = checker._TIMEOUT_VOTE_EPISODE_TLA_THEOREM_SHA256[
        f"{boundary_continuation}.tla"
    ]
    assert set(boundary_continuation_seals) == set(
        RECOVERY_BOUNDARY_CONTINUATION_THEOREMS[:13]
    )
    assert not set(base_seals).intersection(RECOVERY_BOUNDARY_CONTINUATION_THEOREMS)
    base_operator_seals = checker._TIMEOUT_VOTE_EPISODE_TLA_OPERATOR_SHA256[
        f"{recovery}.tla"
    ]
    boundary_continuation_operator_seals = (
        checker._TIMEOUT_VOTE_EPISODE_TLA_OPERATOR_SHA256[
            f"{boundary_continuation}.tla"
        ]
    )
    assert set(boundary_continuation_operator_seals) == {
        "AsyncTimeoutRecoveryNewBaseEpisodeIn",
        "AsyncTimeoutRecoveryNewEpisodeClearsFrozenPredecessorIn",
    }
    assert not set(base_operator_seals).intersection(
        boundary_continuation_operator_seals
    )


def test_recovery_boundary_partition_rejects_wrong_ownership() -> None:
    sources = _sources()
    recovery = "SumeragiV2AsyncRecoveryVoteEpochProofs"
    boundary_continuation = RECOVERY_BOUNDARY_CONTINUATION
    footer = "=============================================================================\n"
    first_marker = f"THEOREM {RECOVERY_BOUNDARY_CONTINUATION_THEOREMS[0]} =="
    second_marker = f"THEOREM {RECOVERY_BOUNDARY_CONTINUATION_THEOREMS[1]} =="
    first_start = sources[boundary_continuation].index(first_marker)
    first_end = sources[boundary_continuation].index(second_marker)
    first_theorem = sources[boundary_continuation][first_start:first_end]
    seventh_marker = f"THEOREM {RECOVERY_BOUNDARY_CONTINUATION_THEOREMS[6]} =="
    first_six_end = sources[boundary_continuation].index(seventh_marker)
    first_six_theorems = sources[boundary_continuation][first_start:first_six_end]

    omitted = dict(sources)
    omitted[boundary_continuation] = omitted[boundary_continuation].replace(
        first_theorem, "", 1
    )
    errors, providers = checker._async_liveness_shard_contract(omitted)
    assert RECOVERY_BOUNDARY_CONTINUATION_THEOREMS[0] not in providers
    assert any("not a mechanical partition" in error for error in errors)

    duplicated = dict(sources)
    duplicated[recovery] = duplicated[recovery].replace(
        footer, first_theorem + footer, 1
    )
    errors, _ = checker._async_liveness_shard_contract(duplicated)
    assert any(
        f"declaration {RECOVERY_BOUNDARY_CONTINUATION_THEOREMS[0]} is duplicated"
        in error
        for error in errors
    )

    wrong_shard = dict(sources)
    wrong_shard[recovery] = wrong_shard[recovery].replace(
        footer, first_six_theorems + footer, 1
    )
    wrong_shard[boundary_continuation] = wrong_shard[boundary_continuation].replace(
        first_six_theorems, "", 1
    )
    errors, providers = checker._async_liveness_shard_contract(wrong_shard)
    assert providers[RECOVERY_BOUNDARY_CONTINUATION_THEOREMS[0]] == recovery
    assert not any("not a mechanical partition" in error for error in errors)
    assert any(
        f"{recovery}.tla exceeds 150 top-level theorems" in error for error in errors
    )


def test_deadlock_shard_exact_finite_runner_dependency_is_acyclic() -> None:
    deadlock = "SumeragiV2AsyncDeadlockProofs"
    finite_runner = "SumeragiV2AsyncFiniteRunnerEpisodeProofs"
    candidate_continuation = "SumeragiV2AsyncCandidateProducerContinuationProofs"
    expected_dependencies = (finite_runner, candidate_continuation)
    sources = _sources()

    assert checker._module_extends(sources[deadlock]) == expected_dependencies
    assert checker.ASYNC_LIVENESS_EXTENDS_OVERRIDES[deadlock] == expected_dependencies

    def reaches(module: str, target: str, seen: set[str]) -> bool:
        if module == target:
            return True
        if module in seen:
            return False
        seen.add(module)
        path = checker.FORMAL_DIR / f"{module}.tla"
        if not path.is_file():
            return False
        source = path.read_text(encoding="utf-8")
        return any(reaches(dependency, target, seen) for dependency in checker._module_extends(source))

    for dependency in expected_dependencies:
        assert not reaches(dependency, deadlock, set())


def test_global_mechanical_body_reconstruction_is_exact() -> None:
    sources = _sources()
    footer = "=============================================================================\n"
    deadlock = "SumeragiV2AsyncDeadlockProofs"
    deadlock_index = next(
        index
        for index, (module, _) in enumerate(checker.ASYNC_LIVENESS_SHARDS)
        if module == deadlock
    )
    expected_deadlock_prefix = (
        "---- MODULE SumeragiV2AsyncDeadlockProofs ----\n"
        "EXTENDS SumeragiV2AsyncFiniteRunnerEpisodeProofs, "
        "SumeragiV2AsyncCandidateProducerContinuationProofs\n\n"
    )
    assert checker._async_liveness_shard_source_prefix(
        deadlock, deadlock_index
    ) == expected_deadlock_prefix
    assert sources[deadlock].startswith(expected_deadlock_prefix)

    reconstructed_parts = []
    for index, (module, _) in enumerate(checker.ASYNC_LIVENESS_SHARDS):
        prefix = checker._async_liveness_shard_source_prefix(module, index)
        source = sources[module]
        assert source.startswith(prefix)
        assert source.endswith(footer)
        reconstructed_parts.append(source[len(prefix) : -len(footer)])
    reconstructed = "".join(reconstructed_parts)
    corrected_quantifier = (
        "  \\A source \\in AsyncCurrentResponsiveVoters,\n"
        "     recipient \\in CurrentVoters:\n"
        "    \\A minimumView:\n"
        "      ResponsiveViewCertificateAuthority(source, minimumView)\n"
        "        => TcFrontier(recipient, minimumView)"
    )
    original_quantifier = (
        "  \\A source \\in AsyncCurrentResponsiveVoters,\n"
        "     recipient \\in CurrentVoters, minimumView:\n"
        "    ResponsiveViewCertificateAuthority(source, minimumView)\n"
        "      => TcFrontier(recipient, minimumView)"
    )
    assert reconstructed.count(corrected_quantifier) == 1
    sealed_reconstruction = reconstructed.replace(
        corrected_quantifier, original_quantifier
    )
    assert hashlib.sha256(sealed_reconstruction.encode("utf-8")).hexdigest() == (
        checker.ASYNC_LIVENESS_PRE_SPLIT_BODY_SHA256
    )
    assert checker.ASYNC_LIVENESS_PRE_SPLIT_BODY_SHA256 == (
        "67f49ac8eb83b4227351562d23a614559c385c7540e547dee0d97d51f112f62a"
    )


def test_shard_contract_rejects_prefix_and_footer_drift() -> None:
    prefix_drift = _sources()
    deadlock = "SumeragiV2AsyncDeadlockProofs"
    prefix_drift[deadlock] = prefix_drift[deadlock].replace(
        "SumeragiV2AsyncFiniteRunnerEpisodeProofs, "
        "SumeragiV2AsyncCandidateProducerContinuationProofs\n\n",
        "SumeragiV2AsyncFiniteRunnerEpisodeProofs,  "
        "SumeragiV2AsyncCandidateProducerContinuationProofs\n\n",
        1,
    )
    errors, _ = checker._async_liveness_shard_contract(prefix_drift)
    assert any("exact reviewed async liveness shard prefix" in error for error in errors)

    footer_drift = _sources()
    decision = "SumeragiV2AsyncDecisionApplicationProofs"
    footer_drift[decision] = footer_drift[decision].removesuffix("\n")
    errors, _ = checker._async_liveness_shard_contract(footer_drift)
    assert any("exact reviewed async liveness shard footer" in error for error in errors)


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

    obsolete_deadlock = _sources()
    obsolete_deadlock["SumeragiV2AsyncDeadlockProofs"] = obsolete_deadlock[
        "SumeragiV2AsyncDeadlockProofs"
    ].replace(
        "EXTENDS SumeragiV2AsyncFiniteRunnerEpisodeProofs",
        "EXTENDS SumeragiV2AsyncStage2Proofs",
        1,
    )
    errors, _ = checker._async_liveness_shard_contract(obsolete_deadlock)
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
        "ForwardReferenceProbe == ApplicationLivenessObligation\n\n"
        "=============================================================================",
    )
    errors, _ = checker._async_liveness_shard_contract(forward)
    assert any(
        "forward async-family reference ApplicationLivenessObligation" in error
        for error in errors
    )


def test_debt_shard_rejects_every_proofless_theorem() -> None:
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
    boundary_continuation = "SumeragiV2AsyncRecoveryVoteEpochBoundaryContinuationProofs"
    prepare_admission = 'vote.phase = "Prepare" /\\ VoteRoundAdmissible(node, vote)'
    commit_admission = 'vote.phase = "Commit" /\\ VoteRoundAdmissible(node, vote)'
    assert sources[boundary_continuation].count(prepare_admission) == 1
    sources[boundary_continuation] = sources[boundary_continuation].replace(
        prepare_admission, commit_admission, 1
    )

    errors, _ = checker._async_liveness_shard_contract(sources)

    assert any("not a mechanical partition" in error for error in errors)


def test_shard_contract_rejects_stale_reconstruction_digest() -> None:
    current_digest = checker.ASYNC_LIVENESS_PRE_SPLIT_BODY_SHA256
    try:
        checker.ASYNC_LIVENESS_PRE_SPLIT_BODY_SHA256 = (
            "946223a56f68896ae0484ec2d46acc7d3337e17beeb1409f3167d39ab294a52a"
        )
        errors, _ = checker._async_liveness_shard_contract(_sources())
    finally:
        checker.ASYNC_LIVENESS_PRE_SPLIT_BODY_SHA256 = current_digest

    assert any("not a mechanical partition" in error for error in errors)


def test_shard_contract_rejects_reconstruction_header_bypass() -> None:
    sources = _sources()
    deadlock = "SumeragiV2AsyncDeadlockProofs"
    sources[deadlock] = sources[deadlock].replace(
        "EXTENDS SumeragiV2AsyncFiniteRunnerEpisodeProofs, "
        "SumeragiV2AsyncCandidateProducerContinuationProofs\n",
        "EXTENDS SumeragiV2AsyncFiniteRunnerEpisodeProofs,\n"
        "        SumeragiV2AsyncCandidateProducerContinuationProofs\n",
        1,
    )

    errors, _ = checker._async_liveness_shard_contract(sources)

    assert any(
        f"{deadlock}.tla must start with its exact reviewed async liveness "
        "shard prefix" in error
        for error in errors
    )


def test_facade_evidence_maps_symbols_to_provider_logs() -> None:
    entries = checker._facade_provider_entries(checker.FORMAL_DIR, checker.ROOT_DIR)
    by_symbol = {entry["symbol"]: entry for entry in entries}

    assert by_symbol["AsyncTypeInvariantObligation"] == {
        "symbol": "AsyncTypeInvariantObligation",
        "module": "SumeragiV2AsyncProtectedSlotProofs",
        "log": (
            "formal/sumeragi_v2/tlaps/"
            "SumeragiV2AsyncProtectedSlotProofs.log"
        ),
    }
    assert "TimeoutViewProgressObligation" not in by_symbol
