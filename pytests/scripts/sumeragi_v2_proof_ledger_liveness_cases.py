# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

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
    formal_dir = copy_async_liveness_shard_fixture(tmp_path, module)
    shutil.copy2(
        module.FORMAL_DIR / "proof_coverage.json",
        formal_dir / "proof_coverage.json",
    )
    proof = async_liveness_symbol_path(formal_dir, module, symbol)
    source = proof.read_text(encoding="utf-8")
    assert source.count(correct) >= 1
    proof.write_text(source.replace(correct, grouped_mutation, 1), encoding="utf-8")

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        symbol in error and "record-valued" in error
        for error in errors
    ), errors


def copy_async_liveness_shard_fixture(tmp_path: Path, module) -> Path:
    """Copy the virtual async proof facade and every physical proof shard."""

    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    modules = (
        module.ASYNC_LIVENESS_FACADE,
        *(name for name, _ in module.ASYNC_LIVENESS_SHARDS),
    )
    for name in modules:
        shutil.copy2(module.FORMAL_DIR / f"{name}.tla", formal_dir / f"{name}.tla")
    return formal_dir


def async_liveness_symbol_path(
    formal_dir: Path,
    module,
    symbol: str,
) -> Path:
    """Resolve one virtual-façade symbol to its unique physical proof shard."""

    providers = []
    for name, _ in module.ASYNC_LIVENESS_SHARDS:
        path = formal_dir / f"{name}.tla"
        source = path.read_text(encoding="utf-8")
        if (
            module._top_level_theorem_body(source, symbol) is not None
            or module._top_level_operator_body(source, symbol) is not None
        ):
            providers.append(path)
    assert len(providers) == 1, (symbol, providers)
    return providers[0]


@pytest.mark.parametrize(
    ("relative", "kind", "symbol", "old", "new", "expected_error"),
    (
        (
            "SumeragiV2AsyncFairServiceProofs.tla",
            "operator",
            "LocalServeSchedulerStep",
            "  \\/ LocalAdmissionStep(node)\n"
            "  \\/ SerializedLocalPrecedesServeIngressStep(node)\n"
            "  \\/ AsyncServeIngressTargetOnlyTurn(node)",
            "  \\/ LocalAdmissionStep(node)\n"
            "  \\/ AsyncServeIngressTargetOnlyTurn(node)",
            "LocalServeSchedulerStep must equal only the exact",
        ),
        (
            "SumeragiV2AsyncFairServiceProofs.tla",
            "operator",
            "RuntimeServeSchedulerStep",
            "  \\/ SerializedRuntimeStep(node)\n"
            "  \\/ SerializedRuntimePrecedesServeIngressStep(node)\n"
            "  \\/ AsyncServeIngressTargetOnlyTurn(node)",
            "  \\/ SerializedRuntimeStep(node)\n"
            "  \\/ AsyncServeIngressTargetOnlyTurn(node)",
            "RuntimeServeSchedulerStep must equal only the exact",
        ),
        (
            "SumeragiV2AsyncInstallRunnerProofs.tla",
            "operator",
            "SerializedRunnerRuntimeStep",
            "  \\/ SerializedRuntimeStep(node)\n"
            "  \\/ SerializedRuntimePrecedesServeIngressStep(node)\n"
            "  \\/ AsyncCandidateProducerContinuationExactRuntimeReplayStep(node)",
            "  \\/ SerializedRuntimeStep(node)\n"
            "  \\/ SerializedRuntimePrecedesServeIngressStep(node)",
            "SerializedRunnerRuntimeStep must equal only the exact",
        ),
        (
            "SumeragiV2AsyncFairServiceProofs.tla",
            "operator",
            "RecoveryRunNodeGuard",
            "     /\\ \\/ ~AsyncIngressSchedulerBarrierActive(node)\n"
            "        \\/ asyncRunnerPhase[node] = \"Ingress\"",
            "     /\\ asyncRunnerPhase[node] = \"Ingress\"",
            "RecoveryRunNodeGuard must equal only the exact",
        ),
        (
            "SumeragiV2AsyncFairServiceProofs.tla",
            "theorem",
            "LocalAdmissionStepIsEnabled",
            "    /\\ ~AsyncIngressSchedulerBarrierActive(node)\n",
            "",
            "LocalAdmissionStepIsEnabled must state only",
        ),
        (
            "SumeragiV2AsyncFairServiceProofs.tla",
            "theorem",
            "NoServeIngressTicketSerializedRuntimeIsEnabled",
            "    /\\ ~AsyncIngressSchedulerBarrierActive(node)\n",
            "",
            "NoServeIngressTicketSerializedRuntimeIsEnabled must state only",
        ),
        (
            "SumeragiV2AsyncFairServiceProofs.tla",
            "theorem",
            "OlderRuntimePrecedesServeIngressStepIsEnabled",
            "    /\\ AsyncIngressSchedulerBarrierActive(node)\n",
            "",
            "OlderRuntimePrecedesServeIngressStepIsEnabled must state only",
        ),
        (
            "SumeragiV2AsyncFairServiceProofs.tla",
            "theorem",
            "OlderRuntimePrecedesServeIngressStepIsEnabled",
            "    /\\ AsyncOlderRuntimeLifecyclePrecedesServeIngress(node)\n",
            "",
            "OlderRuntimePrecedesServeIngressStepIsEnabled must state only",
        ),
        (
            "SumeragiV2AsyncFairServiceProofs.tla",
            "theorem",
            "OlderLocalPrecedesServeIngressStepIsEnabled",
            "    /\\ AsyncOlderLocalLifecyclePrecedesServeIngress(node)\n",
            "",
            "OlderLocalPrecedesServeIngressStepIsEnabled must state only",
        ),
        (
            "SumeragiV2AsyncFairServiceProofs.tla",
            "theorem",
            "ServeIngressTargetOnlyTurnIsEnabled",
            "    /\\ AsyncIngressSchedulerBarrierActive(node)\n",
            "",
            "ServeIngressTargetOnlyTurnIsEnabled must state only",
        ),
        (
            "SumeragiV2AsyncFairServiceProofs.tla",
            "theorem",
            "ServeIngressTargetOnlyTurnIsEnabled",
            "    /\\ ~( /\\ asyncRunnerPhase[node] = \"Runtime\"\n"
            "           /\\ AsyncOlderRuntimeLifecyclePrecedesServeIngress(node))\n",
            "",
            "ServeIngressTargetOnlyTurnIsEnabled must state only",
        ),
        (
            "SumeragiV2AsyncFairServiceProofs.tla",
            "theorem",
            "ServeIngressTargetOnlyTurnIsEnabled",
            "    /\\ ~( /\\ asyncRunnerPhase[node] = \"Local\"\n"
            "           /\\ AsyncOlderLocalLifecyclePrecedesServeIngress(node))\n",
            "",
            "ServeIngressTargetOnlyTurnIsEnabled must state only",
        ),
        (
            "SumeragiV2AsyncFairServiceProofs.tla",
            "theorem",
            "ResponsiveUnappliedRunNodeIsEnabled",
            "    /\\ AsyncStrongTypeInvariant\n",
            "    /\\ AsyncTypeInvariant\n",
            "ResponsiveUnappliedRunNodeIsEnabled must state only",
        ),
        (
            "SumeragiV2AsyncFairServiceProofs.tla",
            "theorem",
            "ResponsiveUnappliedRunNodeIsEnabled",
            (
                "    /\\ "
                "AsyncCandidateProducerContinuationExternalCoverageInvariant\n"
            ),
            "",
            "ResponsiveUnappliedRunNodeIsEnabled must state only",
        ),
        (
            "SumeragiV2AsyncFairServiceProofs.tla",
            "theorem",
            "ResponsiveUnappliedRunNodeIsEnabled",
            (
                "    /\\ "
                "AsyncCandidateProducerContinuationLocalReplayCapacityInvariant\n"
            ),
            "",
            "ResponsiveUnappliedRunNodeIsEnabled must state only",
        ),
        (
            "SumeragiV2AsyncInstallRunnerProofs.tla",
            "theorem",
            "RunNodeWorkConcreteActionCaseSplit",
            (
                "      => \\/ "
                "ResolveRunNodeCandidateProducerContinuation(node)\n"
            ),
            "      => \\/ LocalAdmissionStep(node)\n",
            "RunNodeWorkConcreteActionCaseSplit must state only",
        ),
        (
            "SumeragiV2AsyncInstallRunnerProofs.tla",
            "theorem",
            "RunNodeWorkConcreteActionCaseSplit",
            (
                "         \\/ "
                "ReplayRunNodeCandidateProducerContinuation(node)\n"
            ),
            "",
            "RunNodeWorkConcreteActionCaseSplit must state only",
        ),
        (
            "SumeragiV2AsyncInstallRunnerProofs.tla",
            "theorem",
            "RunNodeWorkConcreteActionCaseSplit",
            "         \\/ AsyncServeIngressTargetOnlyTurn(node)\n",
            "",
            "RunNodeWorkConcreteActionCaseSplit must state only",
        ),
        (
            "SumeragiV2AsyncInstallRunnerProofs.tla",
            "theorem",
            "RunNodeWorkConcreteActionCaseSplit",
            "         \\/ SerializedLocalPrecedesServeIngressStep(node)\n",
            "",
            "RunNodeWorkConcreteActionCaseSplit must state only",
        ),
        (
            "SumeragiV2AsyncDeadlockProofs.tla",
            "theorem",
            "DirectHistoricalRecoveryNoTicketLocalRunnerCaller",
            "    /\\ ~AsyncIngressSchedulerBarrierActive(node)\n",
            "",
            "DirectHistoricalRecoveryNoTicketLocalRunnerCaller must state only",
        ),
        (
            "SumeragiV2AsyncDeadlockProofs.tla",
            "theorem",
            "HistoricalRecoveryRunnerEnabledAfterGst",
            (
                "    /\\ "
                "AsyncCandidateProducerContinuationExternalCoverageInvariant\n"
            ),
            "",
            "HistoricalRecoveryRunnerEnabledAfterGst must state only",
        ),
        (
            "SumeragiV2AsyncDeadlockProofs.tla",
            "theorem",
            "HistoricalRecoveryRunnerEnabledAfterGst",
            (
                "    /\\ "
                "AsyncCandidateProducerContinuationLocalReplayCapacityInvariant\n"
            ),
            "",
            "HistoricalRecoveryRunnerEnabledAfterGst must state only",
        ),
        (
            "SumeragiV2AsyncDeadlockProofs.tla",
            "theorem",
            "HistoricalRecoveryRunnerEnabledAfterGst",
            "    /\\ AsyncStrongTypeInvariant\n",
            "    /\\ AsyncTypeInvariant\n",
            "HistoricalRecoveryRunnerEnabledAfterGst must state only",
        ),
    ),
)
def test_serve_scheduler_gate_proof_mutations_fail_closed(
    tmp_path: Path,
    relative: str,
    kind: str,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_liveness_shard_fixture(tmp_path, module)
    path = formal_dir / relative
    source = path.read_text(encoding="utf-8")
    mutator = mutate_tla_operator if kind == "operator" else mutate_tla_theorem
    path.write_text(mutator(source, symbol, old, new), encoding="utf-8")

    errors = module._async_proof_architecture_errors(formal_dir)

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("kind", "symbol", "old", "new", "expected_error"),
    (
        (
            "theorem",
            "AsyncInitEstablishesServiceActivationPairInvariant",
            "      => AsyncServiceActivationPairInvariant",
            "      => TRUE",
            "AsyncInitEstablishesServiceActivationPairInvariant must state only",
        ),
        (
            "theorem",
            "AsyncInitEstablishesLeaderWireIngressCarrierOwnership",
            "      => AsyncLeaderWireIngressCarrierOwnershipInvariant",
            "      => TRUE",
            "AsyncInitEstablishesLeaderWireIngressCarrierOwnership must state only",
        ),
        (
            "theorem",
            "AsyncInitEstablishesLeaderWireIngressCarrierOwnership",
            "       AsyncLeaderWireLifecycleIdentityDerivable,\n",
            "",
            "must retain the exact leader-wire ingress-carrier proof dependencies",
        ),
        (
            "theorem",
            "AsyncInitEstablishesOrdinaryIngressCarrierOwnership",
            "      => AsyncOrdinaryIngressCarrierOwnershipInvariant",
            "      => TRUE",
            "AsyncInitEstablishesOrdinaryIngressCarrierOwnership must state only",
        ),
        (
            "theorem",
            "AsyncInitEstablishesOrdinaryIngressCarrierOwnership",
            "       AsyncOrdinaryIngressCarrierOwnershipInvariant\n",
            "",
            "must retain the exact ordinary-ingress and candidate-lifecycle "
            "scheduler-coverage proof dependencies",
        ),
        (
            "theorem",
            "AsyncNextPreservesServiceActivationPairInvariant",
            "  /\\ AsyncNext\n",
            "  /\\ AsyncRunnerStep\n",
            "AsyncNextPreservesServiceActivationPairInvariant must state only",
        ),
        (
            "theorem",
            "AsyncNextPreservesServiceActivationPairInvariant",
            "       AsyncActivateServiceNode,\n",
            "",
            "must cover the exact init/full-AsyncNext activation and "
            "deadline-writer boundary",
        ),
        (
            "theorem",
            "AsyncNextPreservesControlServiceStateTypeFromPrimedSchedulerType",
            "  /\\ AsyncSchedulerTypeInvariant'\n",
            "  /\\ AsyncSchedulerTypeInvariant\n",
            "AsyncNextPreservesControlServiceStateTypeFromPrimedSchedulerType "
            "must state only",
        ),
        (
            "theorem",
            "AsyncNextPreservesControlServiceStateTypeInvariant",
            "  /\\ AsyncStrongTypeInvariant\n",
            "  /\\ AsyncTypeInvariant\n",
            "AsyncNextPreservesControlServiceStateTypeInvariant must state only",
        ),
        (
            "theorem",
            "AsyncNextPreservesControlServiceStateTypeInvariant",
            "         AsyncNextPreservesControlServiceStateTypeFromPrimedSchedulerType\n",
            "",
            "must derive primed scheduler typing before the lifecycle-state "
            "transformer",
        ),
        (
            "theorem",
            "AsyncNextPreservesLeaderWireIngressCarrierOwnership",
            "   LeaderWireIngressDrainNeverInventsRuntimeOwner,\n",
            "",
            "must retain the exact leader-wire ingress-carrier proof dependencies",
        ),
        (
            "theorem",
            "AsyncNextPreservesOrdinaryIngressCarrierOwnership",
            "  => AsyncOrdinaryIngressCarrierOwnershipInvariant'\n",
            "  => TRUE\n",
            "AsyncNextPreservesOrdinaryIngressCarrierOwnership must state only",
        ),
        (
            "theorem",
            "AsyncNextPreservesOrdinaryIngressCarrierOwnership",
            "BY ExactOrdinaryIngressDuplicateCoalescesWithoutCarrierAllocation,\n",
            "BY ",
            "must retain the exact ordinary-ingress and candidate-lifecycle "
            "scheduler-coverage proof dependencies",
        ),
        (
            "theorem",
            "AsyncNextPreservesCandidateLifecycleSchedulerCoverage",
            "  => AsyncCandidateLifecycleSchedulerCoverageInvariant'\n",
            "  => TRUE\n",
            "AsyncNextPreservesCandidateLifecycleSchedulerCoverage must state only",
        ),
        (
            "theorem",
            "AsyncNextPreservesCandidateLifecycleSchedulerCoverage",
            "       AsyncCandidateLifecycleStateAfterServeIngressAdmission,\n",
            "",
            "must retain the exact ordinary-ingress and candidate-lifecycle "
            "scheduler-coverage proof dependencies",
        ),
        (
            "operator",
            "AsyncStrongTypeInvariant",
            "  /\\ AsyncServiceActivationPairInvariant\n",
            "",
            "AsyncStrongTypeInvariant must include the exact recovery "
            "execution premise",
        ),
        (
            "operator",
            "AsyncStrongTypeInvariant",
            "  /\\ AsyncCandidateLifecycleSchedulerCoverageInvariant\n",
            "",
            "AsyncStrongTypeInvariant must include the exact recovery "
            "execution premise",
        ),
        (
            "operator",
            "AsyncStrongTypeInvariant",
            "  /\\ AsyncLeaderWireIngressCarrierOwnershipInvariant\n",
            "",
            "AsyncStrongTypeInvariant must include the exact recovery "
            "execution premise",
        ),
        (
            "operator",
            "AsyncStrongTypeInvariant",
            "  /\\ AsyncOrdinaryIngressCarrierOwnershipInvariant\n",
            "",
            "AsyncStrongTypeInvariant must include the exact recovery "
            "execution premise",
        ),
        (
            "theorem",
            "AsyncInitEstablishesStrongTypeInvariant",
            "    <2>3c. AsyncServiceActivationPairInvariant\n"
            "      BY <1>1, AsyncInitEstablishesServiceActivationPairInvariant\n",
            "",
            "must use the exact service-activation pair init bridge",
        ),
        (
            "theorem",
            "AsyncInitEstablishesStrongTypeInvariant",
            "    <2>3d. AsyncLeaderWireIngressCarrierOwnershipInvariant\n"
            "      BY <1>1,\n"
            "         AsyncInitEstablishesLeaderWireIngressCarrierOwnership\n",
            "",
            "must use the exact leader-wire ingress-carrier init bridge",
        ),
        (
            "theorem",
            "AsyncInitEstablishesStrongTypeInvariant",
            "    <2>3e. AsyncOrdinaryIngressCarrierOwnershipInvariant\n"
            "      BY <1>1,\n"
            "         AsyncInitEstablishesOrdinaryIngressCarrierOwnership\n",
            "",
            "must use the exact ordinary-ingress carrier init bridge",
        ),
        (
            "theorem",
            "AsyncInitEstablishesStrongTypeInvariant",
            "    <2>3bb. AsyncCandidateLifecycleSchedulerCoverageInvariant\n",
            "    <2>3bb. TRUE\n",
            "must establish the exact candidate-lifecycle scheduler-coverage "
            "init projection",
        ),
        (
            "theorem",
            "AsyncInitEstablishesStrongTypeInvariant",
            "    <2> QED BY <2>1, <2>3, <2>3a, <2>3b, <2>3bb, <2>3c, <2>3d, <2>3e, <2>4,\n"
            "                <2>5, <2>6, <2>7\n",
            "    <2> QED BY <2>1, <2>3, <2>3a, <2>3b, <2>3c, <2>3d, <2>4,\n"
            "                <2>5, <2>6, <2>7\n",
            "must retain the exact candidate/Serve/leader/ordinary "
            "scheduler-coverage QED dependency set",
        ),
        (
            "theorem",
            "AsyncNextPreservesStrongTypeInvariant",
            "    <2>4a. AsyncServiceActivationPairInvariant'\n"
            "      BY <1>1, <2>2,\n"
            "         AsyncNextPreservesServiceActivationPairInvariant\n",
            "",
            "pass AsyncTypeInvariant to the exact full-AsyncNext "
            "service-activation pair-preservation step",
        ),
        (
            "theorem",
            "AsyncNextPreservesStrongTypeInvariant",
            "    <2>2j. AsyncLeaderWireIngressCarrierOwnershipInvariant\n"
            "      BY <1>1 DEF AsyncStrongTypeInvariant\n",
            "",
            "retain the exact GST-recovery, serialized-busy, "
            "certified-response claim-ingress, leader-wire ingress",
        ),
        (
            "theorem",
            "AsyncNextPreservesStrongTypeInvariant",
            "    <2>12. AsyncLeaderWireIngressCarrierOwnershipInvariant'\n"
            "      BY <1>1, <2>2j,\n"
            "         AsyncNextPreservesLeaderWireIngressCarrierOwnership\n",
            "    <2>12. AsyncLeaderWireIngressCarrierOwnershipInvariant'\n"
            "      BY <1>1,\n"
            "         AsyncNextPreservesLeaderWireIngressCarrierOwnership\n",
            "pass the leader-wire ingress-carrier projection to its exact "
            "preservation step",
        ),
        (
            "theorem",
            "AsyncNextPreservesStrongTypeInvariant",
            "    <2>2k. AsyncOrdinaryIngressCarrierOwnershipInvariant\n"
            "      BY <1>1 DEF AsyncStrongTypeInvariant\n",
            "",
            "retain the exact GST-recovery, serialized-busy, "
            "certified-response claim-ingress, leader-wire ingress",
        ),
        (
            "theorem",
            "AsyncNextPreservesStrongTypeInvariant",
            "    <2>2l. AsyncCandidateLifecycleSchedulerCoverageInvariant\n"
            "      BY <1>1 DEF AsyncStrongTypeInvariant\n",
            "",
            "retain the exact GST-recovery, serialized-busy, "
            "certified-response claim-ingress, leader-wire ingress",
        ),
        (
            "theorem",
            "AsyncNextPreservesStrongTypeInvariant",
            "    <2>4c. AsyncCandidateLifecycleSchedulerCoverageInvariant'\n"
            "      BY <1>1, AsyncNextPreservesCandidateLifecycleSchedulerCoverage\n",
            "",
            "retain the exact candidate-lifecycle scheduler-coverage prime step",
        ),
        (
            "theorem",
            "AsyncNextPreservesStrongTypeInvariant",
            "    <2>13. AsyncOrdinaryIngressCarrierOwnershipInvariant'\n"
            "      BY <1>1, <2>2k,\n"
            "         AsyncNextPreservesOrdinaryIngressCarrierOwnership\n",
            "",
            "pass the ordinary-ingress carrier projection to its exact "
            "preservation step",
        ),
        (
            "theorem",
            "AsyncNextPreservesStrongTypeInvariant",
            "<2>4a, <2>4b",
            "<2>4b",
            "make the service-activation pair, control-service",
        ),
    ),
)
def test_async_service_activation_pair_proof_mutations_fail_closed(
    tmp_path: Path,
    kind: str,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_liveness_shard_fixture(tmp_path, module)
    path = formal_dir / "SumeragiV2AsyncRecoveryVoteEpochProofs.tla"
    source = path.read_text(encoding="utf-8")
    mutator = mutate_tla_operator if kind == "operator" else mutate_tla_theorem
    path.write_text(mutator(source, symbol, old, new), encoding="utf-8")

    errors = module._async_proof_architecture_errors(formal_dir)

    assert any(expected_error in error for error in errors), errors


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
    formal_dir = copy_async_liveness_shard_fixture(tmp_path, module)
    path = async_liveness_symbol_path(formal_dir, module, symbol)
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
            "    /\\ AsyncServeIngressLifecycleOwnerIdentities(\n"
            "         asyncRecoveryNode) = {}\n",
            "",
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
            "    /\\ AsyncServeIngressLifecycleOwnerIdentities(\n"
            "         asyncRecoveryNode) = {}\n"
            "    /\\ SequenceHasUniqueValues(asyncRecoveryReplayQueue)\n"
            "    /\\ SequenceSet(asyncRecoveryReplayQueue) \\cap\n"
            "         ResponsiveReplayScheduledCandidates(asyncRecoveryNode) = {}",
            "    asyncOutstandingTags[asyncRecoveryNode] = {}",
            "AsyncRecoveryExecutionInvariant must equal only",
        ),
        (
            "theorem",
            "PopSelectedIngressDoesNotCreateServeIngressOwners",
            "      => AsyncServeIngressLifecycleOwnerIdentities(owner)'\n"
            "           \\subseteq AsyncServeIngressLifecycleOwnerIdentities(owner)",
            "      => AsyncServeIngressLifecycleOwnerIdentities(owner)'\n"
            "           = AsyncServeIngressLifecycleOwnerIdentities(owner)",
            "PopSelectedIngressDoesNotCreateServeIngressOwners must state only",
        ),
        (
            "theorem",
            "HiddenIngressAdmissionPreservesOtherNodeOwners",
            "    /\\ recipient # owner\n",
            "",
            "HiddenIngressAdmissionPreservesOtherNodeOwners must state only",
        ),
        (
            "theorem",
            "ResetNodeSchedulerForRestartClearsServeIngressOwners",
            "      => AsyncServeIngressLifecycleOwnerIdentities(node)' = {}",
            "      => AsyncServeIngressLifecycleOwnerIdentities(node)' "
            "\\subseteq ValidatorIds",
            "ResetNodeSchedulerForRestartClearsServeIngressOwners must state only",
        ),
        (
            "theorem",
            "ReplayingOrdinaryStepPreservesEmptyServeIngressOwners",
            "  /\\ AsyncRecoveryExecutionInvariant\n",
            "",
            "ReplayingOrdinaryStepPreservesEmptyServeIngressOwners must state only",
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
            "    <2> QED BY <2>2l, <2>3, <2>4, <2>4a, <2>4b, <2>4c, <2>5, <2>6, <2>7,\n"
            "                <2>8, <2>9, <2>10, <2>11, <2>12, <2>13\n"
            "         DEF AsyncStrongTypeInvariant",
            "    <2> QED BY <2>3, <2>4, <2>4a, <2>4b, <2>4c, <2>5, <2>6, <2>7,\n"
            "                <2>8, <2>9, <2>10, <2>11, <2>12, <2>13\n"
            "         DEF AsyncStrongTypeInvariant",
            "make the service-activation pair, control-service",
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
    formal_dir = copy_async_liveness_shard_fixture(tmp_path, module)
    path = async_liveness_symbol_path(formal_dir, module, symbol)
    source = path.read_text(encoding="utf-8")
    mutator = mutate_tla_operator if kind == "operator" else mutate_tla_theorem
    path.write_text(mutator(source, symbol, old, new), encoding="utf-8")

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("symbol", "token"),
    (
        (
            "ReplayingNetworkStepPreservesEmptyRecoveryIngressOwners",
            "HiddenIngressAdmissionPreservesOtherNodeOwners",
        ),
        (
            "FaultStepPreservesEmptyServeIngressOwners",
            "ServeReceiverCloseRollbackDoesNotCreateIngressOwners",
        ),
        (
            "NonRunnerStepPreservesEmptyReplayingIngressOwners",
            "ReplayingNetworkStepPreservesEmptyRecoveryIngressOwners",
        ),
        (
            "RunNodeWorkPreservesEmptyServeIngressOwners",
            "PopSelectedIngressDoesNotCreateServeIngressOwners",
        ),
        (
            "RunNodeWorkPreservesEmptyServeIngressOwners",
            "SerializedLocalPrecedesServeIngressStep",
        ),
        (
            "RunnerStepPreservesEmptyServeIngressOwners",
            "RunNodeWorkPreservesEmptyServeIngressOwners",
        ),
        (
            "ReplayingOrdinaryStepPreservesEmptyServeIngressOwners",
            "RunnerStepPreservesEmptyServeIngressOwners",
        ),
        (
            "DriveResponsiveReplayPreservesRecoveryExecutionInvariant",
            "ServeIngressAdmissionStutterPreservesOwnerIdentities",
        ),
        (
            "PreGstResponsiveReplayEstablishesRecoveryExecutionInvariant",
            "ResetNodeSchedulerForRestartClearsServeIngressOwners",
        ),
        (
            "AsyncNextPreservesRecoveryExecutionInvariant",
            "ReplayingOrdinaryStepPreservesEmptyServeIngressOwners",
        ),
        (
            "AsyncNextPreservesRecoveryExecutionInvariant",
            "AsyncEnterIndexedServiceActivation",
        ),
        (
            "AsyncNextPreservesRecoveryExecutionInvariant",
            "AsyncActivateServiceNode",
        ),
    ),
)
def test_replay_quarantine_ingress_owner_dependency_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    token: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_liveness_shard_fixture(tmp_path, module)
    path = async_liveness_symbol_path(formal_dir, module, symbol)
    source = path.read_text(encoding="utf-8")
    path.write_text(
        delete_tla_theorem_token(source, symbol, token),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        symbol in error
        and "replay-quarantine ingress-owner preservation chain" in error
        and token in error
        for error in errors
    ), errors


def test_async_recovery_scheduled_inventory_prime_scope_mutation_fails_closed(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_liveness_shard_fixture(tmp_path, module)
    path = async_liveness_symbol_path(
        formal_dir,
        module,
        "AsyncNextPreservesRecoveryExecutionInvariant",
    )
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
    formal_dir = copy_async_liveness_shard_fixture(tmp_path, module)
    path = async_liveness_symbol_path(formal_dir, module, symbol)
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
    formal_dir = copy_async_liveness_shard_fixture(tmp_path, module)
    shutil.copy2(
        module.FORMAL_DIR / "proof_coverage.json",
        formal_dir / "proof_coverage.json",
    )
    symbol_match = re.search(r"THEOREM\s+([A-Za-z_][A-Za-z0-9_]*)", correct)
    assert symbol_match is not None
    proof = async_liveness_symbol_path(
        formal_dir,
        module,
        symbol_match.group(1),
    )
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
    formal_dir = copy_async_liveness_shard_fixture(tmp_path, module)
    shutil.copy2(
        module.FORMAL_DIR / "proof_coverage.json",
        formal_dir / "proof_coverage.json",
    )
    proof = async_liveness_symbol_path(
        formal_dir,
        module,
        "ProtectedServiceRankProgressImpliesStarvation",
    )
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
    formal_dir = copy_async_liveness_shard_fixture(tmp_path, module)
    shutil.copy2(
        module.FORMAL_DIR / "proof_coverage.json",
        formal_dir / "proof_coverage.json",
    )
    proof = async_liveness_symbol_path(
        formal_dir,
        module,
        "ProtectedServeWellFoundedRankConvergence",
    )
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
    formal_dir = ROOT_DIR / "formal" / "sumeragi_v2"
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
            "    run_cargo fetch --locked\n",
            "    run_cargo fetch --locked --offline\n",
            "guarded `run_cargo fetch --locked`",
        ),
        (
            harness,
            "    ps -axo pid,etime,command\n",
            "    ps -axo pid,command\n",
            "exact `ps -axo pid,etime,command` snapshot",
        ),
        (
            harness,
            'run_cargo() {\n'
            "  wait_for_external_cargo\n"
            '  command cargo "$@"\n'
            "}\n",
            'run_cargo() {\n'
            '  command cargo "$@"\n'
            "}\n",
            "exact wait_for_external_cargo/run_cargo wrapper",
        ),
        (
            harness,
            "  --*)\n"
            '    echo "unknown harness mode: $1" >&2\n',
            "  --escape)\n"
            '    "$@"\n'
            "    ;;\n"
            "  --*)\n"
            '    echo "unknown harness mode: $1" >&2\n',
            "fixed-mode inventory is not exact",
        ),
        (
            harness,
            '    echo "positional harness commands are unsupported; '
            'select one fixed mode" >&2\n'
            "    exit 2\n",
            '    "$@"\n',
            "argument vector may be forwarded only",
        ),
        (
            harness,
            "    run_cargo verus verify --locked --offline -p "
            "iroha_sumeragi_core --features verus \\\n",
            "    run_cargo verus verify --locked -p "
            "iroha_sumeragi_core --features verus \\\n",
            "exact reviewed Verus and Clippy command branches",
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
            "      run_cargo test --locked --offline -p iroha_sumeragi_core \\\n"
            "        --test network_simulation -- --list --ignored",
            '    readonly ignored_test="accelerated_100_000_block_chaos_preserves_chain_prefix"\n'
            '    ignored_test_list="$(\n'
            "      run_cargo test --locked -p iroha_sumeragi_core \\\n"
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
