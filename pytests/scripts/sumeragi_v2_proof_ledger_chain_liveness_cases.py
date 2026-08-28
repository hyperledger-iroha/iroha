"""Chain liveness and historical-recovery cases executed by the parent suite."""


def test_historical_body_response_phase_authority_is_exact(
    tmp_path: Path,
) -> None:
    test_successor_production_source_mapping_mutations_fail_closed(
        tmp_path,
        "crates/iroha_core/src/sumeragi/v2_block_sync.rs",
        "fn build_historical_body_response(",
        "wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit => {}",
        "wire::GlobalPhase::Commit => {}",
        "build_historical_body_response must preserve exact production order",
    )
    test_successor_production_source_mapping_mutations_fail_closed(
        tmp_path,
        "crates/iroha_core/src/sumeragi/v2_block_sync.rs",
        "fn build_historical_body_response(",
        "authenticate_certified_body_request_with_validator_pops(",
        "authenticate_certified_body_request(",
        "build_historical_body_response must preserve exact production order",
    )

def test_async_historical_recovery_rejects_constants_and_endpoint_theorems(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2AsyncHistoricalRecoveryLivenessProofs.tla"
    source = path.read_text(encoding="utf-8")
    source = source.replace(
        "EXTENDS SumeragiV2AsyncTimeoutOwnershipProofs, TLAPS\n",
        "EXTENDS SumeragiV2AsyncTimeoutOwnershipProofs, TLAPS\n"
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
            "  /\\ server \\in IndexedAsync(initialContext)!\n"
            "                 AsyncCurrentResponsiveVoters\n",
            "",
            "IndexedHistoricalRecoverySourceReady omits exact successor/exact-recovery behavior",
        ),
        (
            "IndexedHistoricalRecoverySourceReady",
            "  /\\ server \\in joinedByContext[initialContext]\n",
            "  /\\ server \\in joinedByContext[initialContext]\n"
            "  /\\ server \\in source.qc.signers\n",
            "IndexedHistoricalRecoverySourceReady contains prohibited successor/exact-recovery behavior",
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
        (
            "IndexedHeightLivenessFromHistoricalReleaseResidualsAndSuccessorProofs",
            "IndexedHistoricalReleaseResidualsDischargeExactProgress",
        ),
        (
            "IndexedHeightLivenessFromAuthorityCarryAndExposureProofs",
            "IndexedAdequateLeaderAuthorityCarryAndExposureSupplyLocalSemanticKernel",
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


def test_chain_temporal_composition_requires_historical_parent_direction(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainLivenessProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        source.replace(
            "EXTENDS SumeragiV2HistoricalRecoveryTemporalClosureProofs, TLAPS",
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
            "    /\\ \\A recipient \\in ValidatorIds,\n"
            "          source \\in IndexedAsync(initialContext)!"
            "AsyncIngressSources:\n"
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


@pytest.mark.parametrize(
    ("action", "clause"),
    (
        (
            "IndexedResolveLocalProducerContinuationStep",
            "three current-voter producer continuations",
        ),
        (
            "IndexedServiceConditionalProducerContinuationStep",
            "three current-voter producer continuations",
        ),
        (
            "IndexedServiceVolatileProducerContinuationStep",
            "three current-voter producer continuations",
        ),
        (
            "IndexedRetireLeaderWireLifecycleStep",
            "bounded leader-wire retirement",
        ),
    ),
)
def test_chain_adequate_leader_fairness_rejects_missing_action(
    tmp_path: Path,
    action: str,
    clause: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(
            source,
            "IndexedFairness",
            action,
            f"Weakened{action}",
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        clause in error or "17 canonical indexed product fair actions" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new", "expected_error"),
    (
        (
            "IndexedJoinedRunnerStep",
            "  \\/ \\E node \\in Responsive:\n"
            "       /\\ node \\in joinedByContext[initialContext]\n"
            "       /\\ IndexedAsync(initialContext)!RunHistoricalServer(node)",
            "  \\/ \\E node \\in ValidatorIds:\n"
            "       /\\ node \\in joinedByContext[initialContext]\n"
            "       /\\ IndexedAsync(initialContext)!RunHistoricalServer(node)",
            "exactly one Responsive, joined-context RunHistoricalServer branch",
        ),
        (
            "IndexedJoinedNonRunnerStep",
            "     \\/ \\E node \\in Responsive:\n"
            "          /\\ node \\in joinedByContext[initialContext]\n"
            "          /\\ IndexedAsync(initialContext)!ServiceIoWorker(node)",
            "     \\/ \\E node \\in ValidatorIds:\n"
            "          /\\ node \\in joinedByContext[initialContext]\n"
            "          /\\ IndexedAsync(initialContext)!ServiceIoWorker(node)",
            "exactly one Responsive, joined-context ServiceIoWorker branch",
        ),
        (
            "IndexedJoinedNonRunnerStep",
            "     \\/ \\E node \\in IndexedAsync(initialContext)!"
            "AsyncCurrentResponsiveVoters:\n"
            "          /\\ node \\in joinedByContext[initialContext]\n"
            "          /\\ IndexedAsync(initialContext)!"
            "EnqueueIoLocalControl(node)",
            "     \\/ \\E node \\in Responsive:\n"
            "          /\\ node \\in joinedByContext[initialContext]\n"
            "          /\\ IndexedAsync(initialContext)!"
            "EnqueueIoLocalControl(node)",
            "EnqueueIoLocalControl branch restricted to "
            "AsyncCurrentResponsiveVoters",
        ),
    ),
)
def test_chain_joined_service_domains_are_pinned(
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
        mutate_tla_operator(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("old", "new"),
    (
        (
            "IndexedAsync(initialContext)!AsyncRecoveryControlVars",
            "IndexedRecovery(initialContext, 1)",
        ),
        (
            "IndexedCore(initialContext, 6)",
            "IndexedCore(initialContext, 5)",
        ),
    ),
)
def test_chain_joined_non_crash_requires_complete_recovery_frame(
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
        mutate_tla_operator(
            source,
            "IndexedJoinedNonCrashStep",
            old,
            new,
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "IndexedJoinedNonCrashStep must retain the complete non-crash "
        "recovery-control frame" in error
        for error in errors
    ), errors


def test_chain_joined_async_next_requires_global_historical_lock_transition(
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
            "IndexedJoinedAsyncNext",
            "  /\\ IndexedAsync(initialContext)!\n"
            "       AsyncHistoricalLockRestartAuthorityTransition\n",
            "",
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "IndexedJoinedAsyncNext must contain only joined non-crash work" in error
        and "global historical-lock restart-authority transition" in error
        for error in errors
    ), errors


def test_chain_joined_async_next_rejects_responsive_crash_insertion(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    old = (
        "        \\/ \\E node \\in ValidatorIds:\n"
        "             IndexedAsync(initialContext)!PreGstCrash(node))"
    )
    new = (
        "        \\/ \\E node \\in ValidatorIds:\n"
        "             IndexedAsync(initialContext)!PreGstCrash(node)\n"
        "        \\/ \\E node \\in Responsive:\n"
        "             IndexedAsync(initialContext)!"
        "PreGstResponsiveCrash(node))"
    )
    path.write_text(
        mutate_tla_operator(
            source,
            "IndexedJoinedAsyncNext",
            old,
            new,
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "IndexedJoinedAsyncNext must contain only joined non-crash work" in error
        and "PreGstResponsiveCrash" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    "proof_token",
    (
        "JoinedRunnerIsExactAsyncWork",
        "JoinedNonRunnerIsExactAsyncWork",
    ),
)
def test_chain_joined_async_refinement_uses_exact_branch_projections(
    tmp_path: Path,
    proof_token: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            "JoinedAsyncStepRefinesExactAsyncStep",
            proof_token,
            "DisconnectedBranchProjection",
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "JoinedAsyncStepRefinesExactAsyncStep proof must retain the exact "
        "indexed fairness dependencies" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("old", "new", "expected_error"),
    (
        (
            "    /\\ \\A node \\in Responsive:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedHistoricalServerStep(initialContext, node))",
            "    /\\ \\A node \\in ValidatorIds:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedHistoricalServerStep(initialContext, node))",
            "Responsive joined archive-service product clause",
        ),
        (
            "    /\\ \\A node \\in Responsive:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedIoWorkerStep(initialContext, node))",
            "    /\\ \\A node \\in ValidatorIds:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedIoWorkerStep(initialContext, node))",
            "Responsive joined archive-service product clause",
        ),
        (
            "    /\\ \\A recipient \\in Responsive,\n"
            "          source \\in IndexedAsync(initialContext)!\n"
            "                     AsyncIngressSources:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedAdmitPacketStep("
            "initialContext, recipient, source))",
            "    /\\ \\A recipient \\in ValidatorIds,\n"
            "          source \\in IndexedAsync(initialContext)!\n"
            "                     AsyncIngressSources:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedAdmitPacketStep("
            "initialContext, recipient, source))",
            "ordinary packet clause over Responsive x AsyncIngressSources",
        ),
        (
            "          source \\in IndexedAsync(initialContext)!\n"
            "                     AsyncIngressSources:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedAdmitPacketStep("
            "initialContext, recipient, source))",
            "          source \\in ValidatorIds:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedAdmitPacketStep("
            "initialContext, recipient, source))",
            "ordinary packet clause over Responsive x AsyncIngressSources",
        ),
        (
            "    /\\ \\A recipient \\in ValidatorIds,\n"
            "          source \\in IndexedAsync(initialContext)!"
            "AsyncIngressSources:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedAdmitHistoricalRecoveryPacketStep(\n"
            "             initialContext, recipient, source))",
            "    /\\ \\A recipient \\in Responsive,\n"
            "          source \\in IndexedAsync(initialContext)!"
            "AsyncIngressSources:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedAdmitHistoricalRecoveryPacketStep(\n"
            "             initialContext, recipient, source))",
            "all-required-node exact historical-recovery product clause",
        ),
        (
            "          source \\in IndexedAsync(initialContext)!"
            "AsyncIngressSources:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedAdmitHistoricalRecoveryPacketStep(\n"
            "             initialContext, recipient, source))",
            "          source \\in ValidatorIds:\n"
            "         WF_IndexedChainVars(\n"
            "           IndexedAdmitHistoricalRecoveryPacketStep(\n"
            "             initialContext, recipient, source))",
            "all-required-node exact historical-recovery product clause",
        ),
    ),
)
def test_chain_indexed_fairness_domains_reject_stale_expansions(
    tmp_path: Path,
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
        mutate_tla_operator(source, "IndexedFairness", old, new),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    "symbol",
    (
        "IndexedFairActionsRemainEnabledInProduct",
        "IndexedFairProductStepsProjectExactOccurrences",
        "IndexedFairExactOccurrencesEnableProductOccurrences",
    ),
)
@pytest.mark.parametrize(
    ("domain_index", "recipient_old", "recipient_new"),
    (
        (0, "\\A recipient \\in Responsive,", "\\A recipient \\in ValidatorIds,"),
        (1, None, None),
    ),
)
def test_chain_fairness_bridges_reject_stale_packet_domains(
    tmp_path: Path,
    symbol: str,
    domain_index: int,
    recipient_old: str | None,
    recipient_new: str | None,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    if recipient_old is not None:
        mutated = mutate_tla_theorem(
            source,
            symbol,
            recipient_old,
            recipient_new,
        )
    else:
        extracted = module._top_level_theorem_body(
            source,
            symbol,
            preserve_string_contents=True,
        )
        assert extracted is not None
        theorem_body, _ = extracted
        source_domains = tuple(
            match.group(0)
            for match in re.finditer(
                r"source \\in IndexedAsync\(initialContext\)!"
                r"\s*AsyncIngressSources:",
                theorem_body,
            )
        )
        assert len(source_domains) == 2
        mutated = mutate_tla_theorem(
            source,
            symbol,
            source_domains[domain_index],
            "source \\in ValidatorIds:",
        )
    path.write_text(mutated, encoding="utf-8")

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        f"{symbol} must retain the exact indexed fairness domains" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "IndexedNodeFairnessTransfers",
            "PostGstCommitCertificateDiscovery(node)",
            "PostGstRunHistoricalServer(node)",
        ),
        (
            "IndexedResponsiveServiceFairnessTransfers",
            "\\A node \\in Responsive:",
            "\\A node \\in ValidatorIds:",
        ),
        (
            "IndexedPacketFairnessTransfers",
            "\\A recipient \\in Responsive,",
            "\\A recipient \\in ValidatorIds,",
        ),
        (
            "IndexedHistoricalRecoveryPacketFairnessTransfers",
            "source \\in IndexedAsync(initialContext)!"
            "AsyncIngressSources:",
            "source \\in ValidatorIds:",
        ),
    ),
)
def test_chain_fairness_transfers_keep_exact_action_partitions_and_domains(
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
        mutate_tla_theorem(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(symbol in error for error in errors), errors


def test_chain_responsive_recovery_dormancy_is_exact(
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
            "IndexedResponsiveRecoveryDormant",
            'IndexedRecovery(initialContext, 1) = "Eligible"',
            'IndexedRecovery(initialContext, 1) = "Recovered"',
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "IndexedResponsiveRecoveryDormant must pin every indexed instance"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    "action",
    (
        "PreGstResponsiveRestart",
        "PreGstResponsiveReplay",
        "ResponsiveReplayRunNode",
        "ResponsiveReplayServiceIoWorker",
        "DriveResponsiveReplayHead",
        "FinishResponsiveReplay",
    ),
)
def test_chain_always_disabled_recovery_action_inventory_is_exact(
    tmp_path: Path,
    action: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(
            source,
            "IndexedResponsiveRecoveryActionsDisabled",
            action,
            f"Removed{action}",
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "must contain exactly the six reviewed always-disabled recovery actions"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    "action",
    (
        "PreGstResponsiveRestart",
        "PreGstResponsiveReplay",
        "ResponsiveReplayRunNode",
        "ResponsiveReplayServiceIoWorker",
        "DriveResponsiveReplayHead",
        "FinishResponsiveReplay",
    ),
)
def test_chain_vacuous_recovery_fairness_names_every_exact_action(
    tmp_path: Path,
    action: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            "IndexedResponsiveRecoveryFairnessIsVacuous",
            action,
            f"Removed{action}",
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "IndexedResponsiveRecoveryFairnessIsVacuous must state only" in error
        or (
            "IndexedResponsiveRecoveryFairnessIsVacuous must retain the exact "
            "indexed fairness domains" in error
        )
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "proof_token"),
    (
        (
            "IndexedInitEstablishesResponsiveRecoveryDormancy",
            "IndexedAsync!AsyncRecoveryInit",
        ),
        (
            "IndexedJoinedAsyncStepPreservesResponsiveRecoveryEligibility",
            "IndexedAsync!AsyncRecoveryControlVars",
        ),
        (
            "IndexedProductActionPreservesResponsiveRecoveryDormancy",
            "IndexedJoinedAsyncStepPreservesResponsiveRecoveryEligibility",
        ),
        (
            "IndexedSuccessorActivationStepPreservesRecoveryState",
            "SuccessorActivationEnvironmentStutter",
        ),
        (
            "IndexedActionPreservesResponsiveRecoveryDormancy",
            "IndexedProductActionPreservesResponsiveRecoveryDormancy",
        ),
        (
            "IndexedStepPreservesResponsiveRecoveryDormancy",
            "IndexedActionPreservesResponsiveRecoveryDormancy",
        ),
        (
            "IndexedChainSpecKeepsResponsiveRecoveryDormant",
            "IndexedInitEstablishesResponsiveRecoveryDormancy",
        ),
        (
            "IndexedResponsiveRecoveryDormancyDisablesFairActions",
            "ExpandENABLED",
        ),
        (
            "IndexedChainSpecAlwaysDisablesResponsiveRecoveryActions",
            "IndexedChainSpecKeepsResponsiveRecoveryDormant",
        ),
        (
            "IndexedResponsiveRecoveryFairnessIsVacuous",
            "IndexedChainSpecAlwaysDisablesResponsiveRecoveryActions",
        ),
        (
            "IndexedInstanceActivationObligation",
            "IndexedResponsiveRecoveryFairnessIsVacuous",
        ),
    ),
)
def test_chain_recovery_dormancy_dependency_chain_is_connected(
    tmp_path: Path,
    symbol: str,
    proof_token: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(source, symbol, proof_token, "DisconnectedProof"),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        f"{symbol} proof must retain the exact indexed fairness dependencies"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "AsyncRepresentativeLiveConfiguration",
            "N >= 4",
            "N >= 3",
        ),
        (
            "AsyncInstallGenerationBudget",
            "GenerationCanIncrement(generation[request.node])",
            "TRUE",
        ),
        (
            "AsyncLiveSpecAt",
            "AsyncRepresentativeLiveConfiguration /\\ AsyncSpecAt(initialContext)",
            "AsyncRepresentativeLiveConfiguration "
            "/\\ AsyncSpecAt(initialContext) "
            "/\\ []AsyncInstallGenerationBudget",
        ),
        (
            "AsyncFiniteLiveSpec",
            "AsyncRepresentativeLiveConfiguration /\\ AsyncFiniteSpec",
            "AsyncRepresentativeLiveConfiguration "
            "/\\ AsyncFiniteSpec /\\ []AsyncInstallGenerationBudget",
        ),
    ),
)
def test_async_live_spec_rejects_assumption_weakening(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._async_spec_shape_errors(formal_dir)

    assert any(f"{symbol} must equal only" in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "symbol", "old", "new"),
    (
        (
            "SumeragiV2AsyncNetwork.tla",
            "PersistInstallTCReady",
            "GenerationCanIncrement(generation[request.node])",
            "TRUE",
        ),
        (
            "SumeragiV2Core.tla",
            "PersistInstallTC",
            "GenerationCanIncrement(generation[node])",
            "TRUE",
        ),
        (
            "SumeragiV2Core.tla",
            "PersistInstallTC",
            "IF sameRoundUpgrade THEN @ + 1 ELSE 0",
            "IF sameRoundUpgrade THEN @ ELSE 0",
        ),
    ),
)
def test_install_generation_transaction_source_fidelity_fails_closed(
    tmp_path: Path,
    relative: str,
    symbol: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / relative
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        f"{symbol} must retain the exact fail-closed InstallTC generation "
        "transaction" in error
        for error in errors
    ), errors


def test_liveness_cfg_cannot_filter_generation_exhaustion(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "liveness.cfg"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        source.replace(
            "INVARIANT DecisionAgreement\n",
            "INVARIANT AsyncInstallGenerationBudget\n"
            "INVARIANT DecisionAgreement\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._progress_witness_source_fidelity_errors(formal_dir)

    assert any(
        "diagnostic AsyncInstallGenerationBudget may not filter the liveness "
        "behavior set" in error
        for error in errors
    ), errors


def test_retired_rotating_leader_facade_theorem_is_rejected() -> None:
    module = load_checker()
    sources = {
        name: (module.FORMAL_DIR / f"{name}.tla").read_text(encoding="utf-8")
        for name, _ in module.ASYNC_LIVENESS_SHARDS
    }
    sources[module.ASYNC_LIVENESS_FACADE] = (
        module.FORMAL_DIR / f"{module.ASYNC_LIVENESS_FACADE}.tla"
    ).read_text(encoding="utf-8")
    source = sources[module.ASYNC_LIVENESS_DEBT_SHARD]
    sources[module.ASYNC_LIVENESS_DEBT_SHARD] = source.replace(
        "=============================================================================",
        r"""THEOREM RotatingLeaderProgressObligation ==
  \A initialContext:
    RotatingLeaderProgressProperty(AsyncLiveSpecAt(initialContext))

=============================================================================""",
        1,
    )

    errors, _ = module._async_liveness_shard_contract(sources)

    assert any("proofless theorems must equal" in error for error in errors), errors


def _current_async_partition_sources(module) -> dict[str, str]:
    sources = {
        name: (module.FORMAL_DIR / f"{name}.tla").read_text(encoding="utf-8")
        for name, _ in module.ASYNC_LIVENESS_SHARDS
    }
    sources[module.ASYNC_LIVENESS_FACADE] = (
        module.FORMAL_DIR / f"{module.ASYNC_LIVENESS_FACADE}.tla"
    ).read_text(encoding="utf-8")
    return sources


def test_async_partition_exact_body_seal_rejects_one_provider_mutation() -> None:
    module = load_checker()
    sources = _current_async_partition_sources(module)
    errors, providers = module._async_liveness_shard_contract(sources)
    assert errors == []
    provider = providers["ModelResponsiveValidators"]
    source = sources[provider]
    assert source.count("ModelResponsiveValidators") >= 1
    sources[provider] = source.replace(
        "ModelResponsiveValidators", "ModelResponsiveValidatorsMutated", 1
    )
    errors, _ = module._async_liveness_shard_contract(sources)
    assert any("mechanical partition of the reviewed pre-split body" in error for error in errors)


def test_async_partition_semantics_survive_digest_refresh(monkeypatch: pytest.MonkeyPatch) -> None:
    module = load_checker()
    sources = _current_async_partition_sources(module)
    corrected = (
        "  \\A source \\in AsyncCurrentResponsiveVoters,\n"
        "     recipient \\in CurrentVoters:\n"
        "    \\A minimumView:\n"
        "      ResponsiveViewCertificateAuthority(source, minimumView)\n"
        "        => TcFrontier(recipient, minimumView)"
    )
    grouped = (
        "  \\A source \\in AsyncCurrentResponsiveVoters,\n"
        "     recipient \\in CurrentVoters, minimumView:\n"
        "    ResponsiveViewCertificateAuthority(source, minimumView)\n"
        "      => TcFrontier(recipient, minimumView)"
    )
    providers = [name for name, source in sources.items() if corrected in source]
    assert len(providers) == 1
    sources[providers[0]] = sources[providers[0]].replace(corrected, grouped, 1)
    bodies, framing_errors = module._async_liveness_shard_bodies(sources)
    assert framing_errors == []
    monkeypatch.setattr(
        module,
        "ASYNC_LIVENESS_PRE_SPLIT_BODY_SHA256",
        hashlib.sha256("".join(bodies).encode("utf-8")).hexdigest(),
    )
    errors, _ = module._async_liveness_shard_contract(sources)
    assert any("corrected nested" in error for error in errors), errors
    assert not any("mechanical partition" in error for error in errors), errors


@pytest.mark.parametrize(
    ("target_module", "reviewed_limit"),
    (
        ("SumeragiV2AsyncInstallRunnerProofs", 5_879),
        ("SumeragiV2AsyncProgressOwnershipProofs", 5_663),
    ),
)
def test_reviewed_async_shard_line_ceiling_rejects_one_more_line(
    target_module: str,
    reviewed_limit: int,
) -> None:
    module = load_checker()
    sources = {
        name: (module.FORMAL_DIR / f"{name}.tla").read_text(encoding="utf-8")
        for name, _ in module.ASYNC_LIVENESS_SHARDS
    }
    source = sources[target_module]
    assert len(source.splitlines()) == reviewed_limit
    footer = "=============================================================================\n"
    assert footer in source
    sources[target_module] = source.replace(footer, f"\n{footer}", 1)

    errors, _ = module._async_liveness_shard_contract(sources)

    assert any(
        f"{target_module}.tla exceeds {reviewed_limit} lines: "
        f"found {reviewed_limit + 1}" in error
        for error in errors
    ), errors


def test_reviewed_install_runner_theorem_ceiling_rejects_one_more_theorem(
) -> None:
    module = load_checker()
    target_module = "SumeragiV2AsyncInstallRunnerProofs"
    sources = {
        name: (module.FORMAL_DIR / f"{name}.tla").read_text(encoding="utf-8")
        for name, _ in module.ASYNC_LIVENESS_SHARDS
    }
    source = sources[target_module]
    declarations = module._top_level_declarations(source)
    theorem_count = sum(kind == "theorem" for _, kind, _, _ in declarations)
    assert theorem_count == 156
    footer = "=============================================================================\n"
    assert footer in source
    sources[target_module] = source.replace(
        footer,
        "THEOREM ReviewedCeilingMutation == TRUE\n"
        "BY PTL\n\n"
        f"{footer}",
        1,
    )

    errors, _ = module._async_liveness_shard_contract(sources)

    assert any(
        f"{target_module}.tla exceeds 156 top-level theorems: found 157"
        in error
        for error in errors
    ), errors


def test_rotating_leader_release_obligation_requires_async_live_spec() -> None:
    module = load_checker()
    ledger = module.load_ledger()
    source = r"""---- MODULE SumeragiV2AsyncTemporalClosureProofs ----
THEOREM AsyncTemporalClosureRotatingLeaderProgressObligation ==
  \A initialContext:
    RotatingLeaderProgressProperty(AsyncSpecAt(initialContext))
=============================================================================
"""

    errors = module._proof_obligation_architecture_errors(
        ledger["obligations"],
        {"SumeragiV2AsyncTemporalClosureProofs": source},
    )

    assert any(
        "AsyncTemporalClosureRotatingLeaderProgressObligation must state only"
        in error
        and "AsyncLiveSpecAt" in error
        for error in errors
    ), errors
    assert any(
        "AsyncTemporalClosureRotatingLeaderProgressObligation must directly require "
        "AsyncLiveSpecAt(initialContext)" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "symbol", "old", "expected_error"),
    (
        (
            "SumeragiV2AsyncTemporalClosureProofs.tla",
            "AsyncTemporalClosureRotatingLeaderProgressObligation",
            "AsyncLiveSpecAt(initialContext)",
            "AsyncTemporalClosureRotatingLeaderProgressObligation must state only",
        ),
    ),
)
def test_async_live_release_shard_fails_closed(
    tmp_path: Path,
    relative: str,
    symbol: str,
    old: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / relative
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            symbol,
            old,
            "AsyncSpecAt(initialContext)",
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "AsyncLiveChainSpec",
            "AsyncChainSpec",
            "/\\ AsyncChainSpec /\\ []AsyncInstallGenerationBudget",
        ),
        (
            "IndexedLiveChainSpec",
            "/\\ AsyncRepresentativeLiveConfiguration\n"
            "  /\\ IndexedChainSpec",
            "/\\ AsyncRepresentativeLiveConfiguration\n"
            "  /\\ IndexedChainSpec "
            "/\\ []IndexedAsync(VerificationContext)!"
            "AsyncInstallGenerationBudget",
        ),
    ),
)
def test_chain_live_specs_reject_finite_generation_assumptions(
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
        mutate_tla_operator(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        f"{symbol} must equal only" in error for error in errors
    ), errors


def test_indexed_live_spec_requires_representative_peer_boundary(
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
            "IndexedLiveChainSpec",
            "AsyncRepresentativeLiveConfiguration",
            "TRUE",
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "IndexedLiveChainSpec must equal only" in error
        for error in errors
    ), errors


def test_chain_rejects_legacy_indexed_generation_budget_premise(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    legacy = (
        "IndexedInstallGenerationBudgetPremise ==\n"
        "  \\A initialContext \\in AdmissibleContextRecords:\n"
        "    []IndexedAsync(initialContext)!AsyncInstallGenerationBudget\n\n"
    )
    path.write_text(
        source.replace("IndexedLiveChainSpec ==", legacy + "IndexedLiveChainSpec ==", 1),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "IndexedInstallGenerationBudgetPremise is an illicit finite-counter "
        "liveness assumption" in error
        for error in errors
    ), errors


def test_indexed_gst_condition_is_explicit_environmental_premise(
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
            "IndexedGstEventuallyCondition",
            "IndexedAsync(initialContext)!AsyncLiveSpecAt(initialContext)\n"
            "      => <>IndexedCore(initialContext, 7)",
            "IndexedAsync(initialContext)!AsyncLiveSpecAt(initialContext)\n"
            "      => RecoveryGenerationBudget(initialContext)",
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "IndexedGstEventuallyCondition must equal only" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "symbol"),
    (
        (
            "SumeragiV2ChainEpochRefinement.tla",
            "AsyncLiveChainSpecProjectsGenesisAsyncLiveSpec",
        ),
        (
            "SumeragiV2ChainEpochRefinement.tla",
            "GenesisHeightSuccessorHandoffFromOneHeightCompletion",
        ),
        (
            "SumeragiV2ChainEpochRefinement.tla",
            "GenesisHeightSuccessorHandoffObligation",
        ),
        (
            "SumeragiV2ChainEpochRefinement.tla",
            "IndexedLiveChainSpecProjectsIndexedChainSpec",
        ),
        (
            "SumeragiV2ChainEpochRefinement.tla",
            "IndexedLiveInstanceActivationObligation",
        ),
        (
            "SumeragiV2ChainEpochRefinement.tla",
            "VerificationFrontierActivatedInstanceEventuallyApplies",
        ),
        (
            "SumeragiV2ChainEpochRefinement.tla",
            "VerificationActivatedFrontierEventuallyEscapes",
        ),
        (
            "SumeragiV2ChainEpochRefinement.tla",
            "VerificationJoinedTargetEventuallyReachesAndEscapes",
        ),
        (
            "SumeragiV2ChainEpochRefinement.tla",
            "HeightLivenessFromOneHeightAndExactRecoveryProgress",
        ),
        (
            "SumeragiV2ChainLivenessProofs.tla",
            "IndexedExactHeightLivenessFromOneHeightAndExactRecoveryProgress",
        ),
        (
            "SumeragiV2ChainLivenessProofs.tla",
            "IndexedExactHeightLivenessFromAsyncHistoricalRecoveryAndSuccessorProofs",
        ),
        (
            "SumeragiV2ChainLivenessProofs.tla",
            "IndexedHeightLivenessFromAsyncHistoricalRecoveryAndSuccessorProofs",
        ),
        (
            "SumeragiV2ChainLivenessProofs.tla",
            "IndexedHeightLivenessFromHistoricalReleaseResidualsAndSuccessorProofs",
        ),
        (
            "SumeragiV2ChainLivenessProofs.tla",
            "IndexedHeightLivenessFromAuthorityCarryAndExposureProofs",
        ),
        (
            "SumeragiV2ChainLivenessProofs.tla",
            "HeightLivenessObligation",
        ),
    ),
)
def test_chain_live_theorem_antecedents_cannot_fall_back_to_safety_spec(
    tmp_path: Path,
    relative: str,
    symbol: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / relative
    source = path.read_text(encoding="utf-8")
    live_spec = (
        "AsyncLiveChainSpec"
        if symbol.startswith("AsyncLiveChainSpecProjects")
        or symbol.startswith("Genesis")
        else "IndexedLiveChainSpec"
    )
    safety_spec = (
        "AsyncChainSpec"
        if live_spec == "AsyncLiveChainSpec"
        else "IndexedChainSpec"
    )
    path.write_text(
        mutate_tla_theorem(source, symbol, live_spec, safety_spec),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(symbol in error and "must state only" in error for error in errors), (
        errors
    )


@pytest.mark.parametrize(
    ("symbol", "proof_token"),
    (
        (
            "AsyncLiveChainSpecProjectsGenesisAsyncLiveSpec",
            "AsyncChainSpecProjectsAsyncSpec",
        ),
        (
            "GenesisHeightSuccessorHandoffFromOneHeightCompletion",
            "AsyncLiveChainSpecProjectsGenesisAsyncLiveSpec",
        ),
        (
            "GenesisHeightSuccessorHandoffObligation",
            "AsyncTemporalClosureOneHeightCompletionObligation",
        ),
        (
            "IndexedLiveInstanceActivationObligation",
            "IndexedInstanceActivationObligation",
        ),
        (
            "IndexedLiveInstanceActivationObligation",
            "AsyncRepresentativeLiveConfiguration",
        ),
        (
            "VerificationOneHeightCompletionObligation",
            "VerificationAsyncProof!"
            "AsyncTemporalClosureOneHeightCompletionObligation",
        ),
        (
            "VerificationFrontierActivatedInstanceEventuallyApplies",
            "IndexedLiveInstanceActivationObligation",
        ),
    ),
)
def test_chain_live_proof_dependencies_are_connected(
    tmp_path: Path,
    symbol: str,
    proof_token: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            symbol,
            proof_token,
            "DisconnectedLiveProof",
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        f"{symbol} proof must retain the exact indexed fairness dependencies"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "exact_dependency", "legacy_dependency"),
    (
        (
            "GenesisHeightSuccessorHandoffObligation",
            "AsyncTemporalClosureOneHeightCompletionObligation",
            "OneHeightCompletionObligation",
        ),
        (
            "VerificationOneHeightCompletionObligation",
            "VerificationAsyncProof!"
            "AsyncTemporalClosureOneHeightCompletionObligation",
            "VerificationAsyncProof!OneHeightCompletionObligation",
        ),
    ),
)
def test_chain_release_rejects_legacy_one_height_facade(
    tmp_path: Path,
    symbol: str,
    exact_dependency: str,
    legacy_dependency: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            symbol,
            exact_dependency,
            legacy_dependency,
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        symbol in error and "prohibited" in error
        for error in errors
    ), errors


def test_chain_composition_invariant_cannot_embed_generation_budget(
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
            "IndexedCompositionInvariant",
            "  /\\ IndexedEveryInstanceStrongInvariant\n",
            "  /\\ IndexedEveryInstanceStrongInvariant\n"
            "  /\\ IndexedInstallGenerationBudgetPremise\n",
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "IndexedCompositionInvariant may not embed a live spec or the "
        "diagnostic install-generation boundary" in error
        for error in errors
    ), errors
