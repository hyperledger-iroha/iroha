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
            "FixedRejectStaleSuccessorActivation",
            '  /\\ lastTransition = "Initial"\n',
            "",
        ),
        (
            "FixedRejectStaleSuccessorActivation",
            "  /\\ UNCHANGED <<activationStatus,\n",
            "  /\\ UNCHANGED <<predecessorOwnership,\n",
        ),
        (
            "InitialStaleRejectionIsEnabled",
            "    => ENABLED FixedRejectStaleSuccessorActivation\n",
            "    => ~ENABLED FixedRejectStaleSuccessorActivation\n",
        ),
        (
            "FixedRejectPreservesStaleState",
            "    => StaleAppliedTokenState\n",
            "    => TRUE\n",
        ),
        (
            "FixedMutationNext",
            "  \\/ FixedRejectStaleSuccessorActivation\n",
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
            "INVARIANT InitialStaleRejectionIsEnabled\n",
        ),
        (
            "successor_stale_token_fixed.cfg",
            "INVARIANT FixedRejectPreservesStaleState\n",
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
            "HistoricalCommitDecisionDirectEvidence",
            (
                "  /\\ candidate.causalOrigin =\n"
                "       AsyncDeliveryCandidateCausalOriginAt("
                "candidate.evidence, context)"
            ),
            "  /\\ TRUE",
        ),
        (
            "HistoricalCommitDecisionResponseEvidence",
            (
                "  /\\ candidate.causalOrigin =\n"
                "       AsyncCommitCertificateResponseCandidateCausalOriginAt(\n"
                "         candidate.evidence, context)"
            ),
            "  /\\ TRUE",
        ),
        (
            "HistoricalCommitDecisionCandidateOwned",
            "       ELSE candidate.item = NoAsyncItem",
            '       ELSE candidate.item.kind = "CommitQC"',
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
            "HistoricalRecoveryAsyncTemporalClosurePremises",
            "  /\\ ApplicationCompletionProgressProperty(specification)\n",
            "  /\\ TRUE\n",
        ),
        (
            "HistoricalRecoveryAsyncRemainingCorridorPremises",
            "  /\\ ApplicationCompletionProgressProperty(specification)\n",
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
            "ResponsiveDecisionApplicationFromExactCorridor",
            "ApplicationCompletionProgressProperty",
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



@pytest.mark.parametrize(
    ("old", "new", "expected"),
    (
        ("context |->", "wireContext |->", "fields must equal exactly"),
        ("round |->", "wireRound |->", "fields must equal exactly"),
        (
            "proposalRound |->",
            "signerRound |->",
            "fields must equal exactly",
        ),
        ("subject |->", "wireSubject |->", "fields must equal exactly"),
        ("phase |->", "wirePhase |->", "fields must equal exactly"),
        (
            "executionCommitment |->",
            "manifest |->",
            "fields must equal exactly",
        ),
        (
            "executionCommitment |-> candidate.commitmentIdentity]",
            "executionCommitment |-> candidate.commitmentIdentity, "
            "signer |-> candidate.node]",
            "fields must equal exactly",
        ),
    ),
)
def test_candidate_semantic_statement_rejects_field_and_carrier_mutations(
    tmp_path: Path,
    old: str,
    new: str,
    expected: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    source_path = module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla"
    target_path = formal_dir / source_path.name
    source = source_path.read_text(encoding="utf-8")
    target_path.write_text(source, encoding="utf-8")

    check = module._async_candidate_semantic_identity_contract_errors
    assert check(formal_dir) == []
    target_path.write_text(
        mutate_tla_operator(
            source,
            "AsyncCandidateSemanticStatement",
            old,
            new,
        ),
        encoding="utf-8",
    )

    errors = check(formal_dir)

    assert any(
        "AsyncCandidateSemanticStatement" in error and expected in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("operator", "old", "new"),
    (
        (
            "NoAsyncCandidateSemanticPhase",
            '"NoCandidateSemanticPhase"',
            '"Prepare"',
        ),
        (
            "AsyncCandidateSemanticPhases",
            "Phases \\cup {NoAsyncCandidateSemanticPhase}",
            '{"Prepare"} \\cup {NoAsyncCandidateSemanticPhase}',
        ),
        (
            "AsyncCandidatePrepareQcSemanticPhase",
            "ELSE qc.phase",
            'ELSE "Commit"',
        ),
        (
            "AsyncCandidateItemSemanticPhase",
            'item.kind \\in {"PrepareVote", "PrepareQC"} -> "Prepare"',
            'item.kind \\in {"PrepareVote", "PrepareQC"} -> "Commit"',
        ),
        (
            "AsyncCandidateEvidenceSemanticPhase",
            "ELSE IF evidence \\in VoteRecordSet\n"
            "            THEN evidence.phase",
            "ELSE IF evidence \\in VoteRecordSet\n"
            "            THEN NoAsyncCandidateSemanticPhase",
        ),
        (
            "AsyncCandidateSemanticPhase",
            '"BeginDecision", "PersistDecision", "Apply"} -> "Commit"',
            '"BeginDecision", "PersistDecision", "Apply"} -> "Prepare"',
        ),
        (
            "AsyncCandidateSuccessorSemanticPhase",
            '/\\ kind = "SignVote"\n'
            '          /\\ command.kind = "PersistLockCommit"\n'
            '       THEN "Commit"',
            '/\\ kind = "SignVote"\n'
            '          /\\ command.kind = "PersistLockCommit"\n'
            '       THEN "Prepare"',
        ),
        (
            "AsyncCandidateSuccessorSemanticPhase",
            '/\\ kind = "SignTimeout"\n'
            "               /\\ AsyncCandidateSignTimeoutRequests(command) # {}",
            '/\\ kind = "SignTimeout"\n'
            "               /\\ AsyncCandidateSignTimeoutRequests(command) = {}",
        ),
    ),
)
def test_candidate_semantic_phase_rejects_closed_table_mutations(
    tmp_path: Path,
    operator: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    source_path = module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla"
    target_path = formal_dir / source_path.name
    source = source_path.read_text(encoding="utf-8")
    target_path.write_text(
        mutate_tla_operator(source, operator, old, new),
        encoding="utf-8",
    )

    errors = module._async_candidate_semantic_identity_contract_errors(
        formal_dir
    )

    assert any(
        operator in error and "exact closed candidate semantic-phase" in error
        for error in errors
    ), errors


def test_candidate_semantic_identity_rejects_concrete_projection(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    source_path = module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla"
    target_path = formal_dir / source_path.name
    source = source_path.read_text(encoding="utf-8")
    target_path.write_text(
        replace_tla_operator_body(
            source,
            "AsyncCandidateServiceIdentity",
            "ExactAsyncCandidateIdentity(candidate)",
        ),
        encoding="utf-8",
    )

    errors = module._async_candidate_semantic_identity_contract_errors(
        formal_dir
    )

    assert any(
        "AsyncCandidateServiceIdentity must project" in error
        for error in errors
    ), errors


def test_concrete_candidate_identity_must_retain_full_bytes(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    source_path = module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla"
    target_path = formal_dir / source_path.name
    source = source_path.read_text(encoding="utf-8")
    target_path.write_text(
        mutate_tla_operator(
            source,
            "ExactAsyncCandidateIdentity",
            "candidate.item",
            "NoAsyncItem",
        ),
        encoding="utf-8",
    )

    errors = module._async_candidate_semantic_identity_contract_errors(
        formal_dir
    )

    assert any(
        "ExactAsyncCandidateIdentity must retain full concrete effect bytes"
        in error
        and "candidate.item" in error
        for error in errors
    ), errors


def copy_candidate_proposal_round_contract_sources(
    module: object, formal_dir: Path
) -> None:
    for name in (
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
        "SumeragiV2LivenessProofs.tla",
    ):
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)


def test_candidate_proposal_round_contract_is_current(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    copy_candidate_proposal_round_contract_sources(module, formal_dir)

    assert module._async_candidate_proposal_round_contract_errors(
        formal_dir
    ) == []


@pytest.mark.parametrize(
    ("operator", "old", "new", "expected"),
    (
        (
            "AsyncCandidateItemProposalRound",
            "proposal.context, proposal.height, proposal.view",
            "proposal.context, proposal.height, defaultRound.view",
            "exact authenticated network",
        ),
        (
            "AsyncCandidateItemProposalRound",
            "ELSE LET qc == vote.highestPrepareQc\n"
            "                 IN AsyncCandidateRound("
            "qc.context, qc.height, qc.view)",
            "ELSE AsyncCandidateRound("
            "vote.context, vote.height, vote.view)",
            "exact authenticated network",
        ),
        (
            "AsyncCandidateEvidenceProposalRound",
            "THEN AsyncCandidateRound(\n"
            "         evidence.context, evidence.height, evidence.view)",
            "THEN defaultRound",
            "closed exact evidence table",
        ),
        (
            "AsyncCandidateEvidenceProposalRound",
            "ELSE IF evidence \\in TcRecordSet",
            "ELSE IF evidence \\in ProposalRecordSet",
            "closed exact evidence table",
        ),
        (
            "AsyncCandidateWithIdentityAndOrigin",
            "AsyncCandidateRound(consumerContext, blockHeight, roundView),\n"
            "       evidence)",
            "AsyncCandidateRound(consumerContext, blockHeight, roundView),\n"
            "       NoAsyncItem)",
            "derive exactly one internal proposalRound",
        ),
    ),
)
def test_candidate_proposal_round_rejects_root_evidence_weakening(
    tmp_path: Path,
    operator: str,
    old: str,
    new: str,
    expected: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    copy_candidate_proposal_round_contract_sources(module, formal_dir)
    source_path = module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla"
    target_path = formal_dir / source_path.name
    source = source_path.read_text(encoding="utf-8")
    target_path.write_text(
        mutate_tla_operator(source, operator, old, new),
        encoding="utf-8",
    )

    errors = module._async_candidate_proposal_round_contract_errors(
        formal_dir
    )

    assert any(operator in error and expected in error for error in errors), errors


@pytest.mark.parametrize(
    ("operator", "old", "new", "expected"),
    (
        (
            "AsyncCandidateSuccessorProposalRound",
            "ELSE command.proposalRound",
            "ELSE AsyncCandidateRound("
            "command.consumerContext, command.height, command.view)",
            "exact reviewed proposal-round derivation",
        ),
        (
            "AsyncCandidateSignTimeoutProposalRound",
            "ELSE LET qc == vote.highestPrepareQc",
            "ELSE LET qc == NoPrepareQC",
            "exact reviewed proposal-round derivation",
        ),
        (
            "AsyncCandidateCausalSuccessorWithIdentityAndOrigin",
            "AsyncCandidateSuccessorProposalRound(kind, command)",
            "AsyncCandidateEvidenceProposalRound("
            "candidate.proposalRound, evidence)",
            "overwrite only proposalRound",
        ),
    ),
)
def test_candidate_proposal_round_rejects_causal_inheritance_weakening(
    tmp_path: Path,
    operator: str,
    old: str,
    new: str,
    expected: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    copy_candidate_proposal_round_contract_sources(module, formal_dir)
    source_path = module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla"
    target_path = formal_dir / source_path.name
    source = source_path.read_text(encoding="utf-8")
    target_path.write_text(
        mutate_tla_operator(source, operator, old, new),
        encoding="utf-8",
    )

    errors = module._async_candidate_proposal_round_contract_errors(
        formal_dir
    )

    assert any(operator in error and expected in error for error in errors), errors


@pytest.mark.parametrize(
    "operator",
    (
        "AsyncCandidateFrom",
        "CausalCandidateWithEvidence",
        "InstallCommitSignSuccessor",
        "InstallLockedFetchSuccessor",
        "InstallProposalSuccessor",
        "PersistDecisionRecoverySuccessor",
    ),
)
def test_every_causal_constructor_must_use_proposal_round_seam(
    tmp_path: Path,
    operator: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    copy_candidate_proposal_round_contract_sources(module, formal_dir)
    source_path = module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla"
    target_path = formal_dir / source_path.name
    source = source_path.read_text(encoding="utf-8")
    target_path.write_text(
        mutate_tla_operator(
            source,
            operator,
            "AsyncCandidateCausalSuccessorWithIdentityAndOrigin",
            "AsyncCandidateWithIdentityAndOrigin",
        ),
        encoding="utf-8",
    )

    errors = module._async_candidate_proposal_round_contract_errors(
        formal_dir
    )

    assert any(
        operator in error and "must inherit proposalRound" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    "operator",
    (
        "FrozenInstallProposalSuccessor",
        "FrozenNormalBeginPrepareCandidate",
    ),
)
def test_frozen_causal_helpers_must_retain_proposal_round_seam(
    tmp_path: Path,
    operator: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    copy_candidate_proposal_round_contract_sources(module, formal_dir)
    target_path = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = target_path.read_text(encoding="utf-8")
    target_path.write_text(
        mutate_tla_operator(
            source,
            operator,
            "AsyncCandidateCausalSuccessorWithIdentityAndOrigin",
            "AsyncCandidateWithIdentityAndOrigin",
        ),
        encoding="utf-8",
    )

    errors = module._async_candidate_proposal_round_contract_errors(
        formal_dir
    )

    assert any(
        operator in error
        and "must use the reviewed causal-successor seam" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    "surface",
    ("wire", "nested_wire", "configuration", "constant", "core", "cfg"),
)
def test_internal_candidate_proposal_round_rejects_surface_exposure(
    tmp_path: Path,
    surface: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    copy_candidate_proposal_round_contract_sources(module, formal_dir)
    source_path = module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla"
    source = source_path.read_text(encoding="utf-8")

    if surface == "wire":
        source = mutate_tla_operator(
            source,
            "AsyncNetworkItem",
            "[kind |-> kind, source |-> source, envelope |-> envelope]",
            "[kind |-> kind, source |-> source, envelope |-> envelope, "
            "proposalRound |-> NoAsyncItem]",
        )
    elif surface == "nested_wire":
        source = mutate_tla_operator(
            source,
            "AsyncCertifiedRequestEnvelope",
            "signatureNonce |-> signatureNonce]",
            "signatureNonce |-> signatureNonce, "
            "proposalRound |-> NoAsyncItem]",
        )
    elif surface == "configuration":
        source = mutate_tla_operator(
            source,
            "AsyncConfiguration",
            "/\\ AsyncServiceBoundRepresentable",
            "/\\ AsyncServiceBoundRepresentable\n"
            "  /\\ proposalRound \\in AsyncCandidateRoundSet",
        )
    elif surface == "constant":
        old = "  AsyncMaximumView,\n  AsyncChunkCount\n\nAsyncCompletionTags"
        new = (
            "  AsyncMaximumView,\n  AsyncChunkCount,\n"
            "  proposalRound\n\nAsyncCompletionTags"
        )
        assert source.count(old) == 1
        source = source.replace(old, new)
    elif surface == "core":
        core_path = formal_dir / "SumeragiV2Core.tla"
        core_source = (module.FORMAL_DIR / core_path.name).read_text(
            encoding="utf-8"
        )
        core_path.write_text(
            mutate_tla_operator(
                core_source,
                "VoteRecordSet",
                "signer: ValidatorIds]",
                "signer: ValidatorIds, proposalRound: Views]",
            ),
            encoding="utf-8",
        )
    else:
        (formal_dir / "proposal_round.cfg").write_text(
            "CONSTANT proposalRound = 0\n",
            encoding="utf-8",
        )

    (formal_dir / source_path.name).write_text(source, encoding="utf-8")
    errors = module._async_candidate_proposal_round_contract_errors(
        formal_dir
    )

    expected = {
        "wire": "wire/API/config operator AsyncNetworkItem",
        "nested_wire": (
            "wire/API/config operator AsyncCertifiedRequestEnvelope"
        ),
        "configuration": "wire/API/config operator AsyncConfiguration",
        "constant": "model configuration constant",
        "core": "Core wire/API records",
        "cfg": "configuration parameter",
    }[surface]
    assert any(expected in error for error in errors), errors


@pytest.mark.parametrize(
    ("operator", "old", "new", "expected"),
    (
        (
            "AdequateLeaderFrozenCandidatePayload",
            "executionCommitment |-> candidate.commitmentIdentity]",
            "executionCommitment |-> candidate.commitmentIdentity, "
            "signer |-> candidate.node]",
            "six carrier-free semantic fields",
        ),
        (
            "AdequateLeaderFrozenCandidatePayloadCarrier",
            "executionCommitment: SubjectOrNone]",
            "executionCommitment: SubjectOrNone, signer: ValidatorIds]",
            "only over the six semantic coordinates",
        ),
        (
            "AdequateLeaderFrozenCandidateOwnerUniverseAtPhase",
            "owner.payload.phase = semanticPhase",
            "TRUE",
            "omits reviewed coordinates",
        ),
    ),
)
def test_adequate_leader_frozen_semantic_universe_rejects_carrier_mutations(
    tmp_path: Path,
    operator: str,
    old: str,
    new: str,
    expected: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    names = (
        "SumeragiV2AdequateLeaderServiceClosureProofs.tla",
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
    )
    for name in names:
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)
    service_path = formal_dir / names[0]
    source = service_path.read_text(encoding="utf-8")
    service_path.write_text(
        mutate_tla_operator(source, operator, old, new),
        encoding="utf-8",
    )

    errors = module._adequate_leader_three_way_service_outcome_contract_errors(
        formal_dir
    )

    assert any(operator in error and expected in error for error in errors), errors


@pytest.mark.parametrize(
    "branch",
    (
        "AdequateLeaderTargetDecisionOrStrictlyLowerOccurrenceAction",
        "AdequateLeaderTargetEqualCountOwnerReplacementAction",
        "AdequateLeaderTargetCountIncreasingReplenishmentAction",
    ),
)
def test_adequate_leader_three_way_outcome_rejects_missing_branch(
    tmp_path: Path,
    branch: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    names = (
        "SumeragiV2AdequateLeaderServiceClosureProofs.tla",
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
    )
    for name in names:
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)
    service_path = formal_dir / names[0]
    source = service_path.read_text(encoding="utf-8")

    check = module._adequate_leader_three_way_service_outcome_contract_errors
    assert check(formal_dir) == []
    service_path.write_text(
        mutate_tla_operator(
            source,
            "AdequateLeaderTargetServiceOutcomeAction",
            branch,
            "MissingReviewedOutcomeBranch",
        ),
        encoding="utf-8",
    )

    errors = check(formal_dir)

    assert any(
        "must contain exactly one Decision/strict-lower branch" in error
        and branch in error
        for error in errors
    ), errors


def test_adequate_leader_three_way_disjointness_rejects_missing_pair(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    names = (
        "SumeragiV2AdequateLeaderServiceClosureProofs.tla",
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
    )
    for name in names:
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)
    service_path = formal_dir / names[0]
    source = service_path.read_text(encoding="utf-8")
    branch = "AdequateLeaderTargetDecisionOrStrictlyLowerOccurrenceAction"
    service_path.write_text(
        mutate_tla_theorem(
            source,
            "AdequateLeaderTargetServiceOutcomeIsThreeWayDisjoint",
            branch,
            "MissingFirstPairBranch",
        ),
        encoding="utf-8",
    )

    errors = module._adequate_leader_three_way_service_outcome_contract_errors(
        formal_dir
    )

    assert any(
        "must state all three pairwise exclusions" in error
        and branch in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("replacement", "expected"),
    (
        ("MissingExhaustiveOutcome", "must expose"),
        (
            "AdequateLeaderTargetCountIncreasingReplenishmentAction",
            "may not promote equal replacement or replenishment to progress",
        ),
    ),
)
def test_fixed_selected_service_outcome_rejects_weakening_and_fake_progress(
    tmp_path: Path,
    replacement: str,
    expected: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    names = (
        "SumeragiV2AdequateLeaderServiceClosureProofs.tla",
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
    )
    for name in names:
        shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)
    for name in names:
        path = formal_dir / name
        source = path.read_text(encoding="utf-8")
        if "THEOREM AdequateLeaderFixedSelectedServiceHasExhaustiveOutcome" not in source:
            continue
        path.write_text(
            mutate_tla_theorem(
                source,
                "AdequateLeaderFixedSelectedServiceHasExhaustiveOutcome",
                "AdequateLeaderTargetServiceOutcomeAction",
                replacement,
            ),
            encoding="utf-8",
        )
        break
    else:
        raise AssertionError("missing exhaustive selected-service theorem")

    errors = module._adequate_leader_three_way_service_outcome_contract_errors(
        formal_dir
    )

    assert any(expected in error for error in errors), errors


@pytest.mark.parametrize(
    ("symbol", "critical_filter", "weakened_filter"),
    (
        (
            "AsyncCandidateLifecycleOriginsRecordedForNodeIn",
            "candidateRecord.node = node",
            "TRUE",
        ),
        (
            "AsyncCandidateLifecycleDurableReplayOriginsForNode",
            "SequenceSet(\n"
            "            FreshRestartCandidateSequence(RestartReplay(node))):\n"
            "          replayCandidate.causalOrigin\n"
            "            \\notin AsyncScheduledCandidateOriginsForNode(node)",
            "SequenceSet(\n"
            "            FreshRestartCandidateSequence(RestartReplay(node))):\n"
            "          TRUE",
        ),
        (
            "AsyncCandidateLifecycleDurableReplayOriginsForNode",
            "SequenceSet(HistoricalLockedRetransmitSuccessors(node)):\n"
            "          replayCandidate.causalOrigin\n"
            "            \\notin AsyncScheduledCandidateOriginsForNode(node)",
            "SequenceSet(HistoricalLockedRetransmitSuccessors(node)):\n"
            "          TRUE",
        ),
        (
            "AsyncCandidateLifecycleDurableReplayOriginsForNodeAfter",
            "SequenceSet(\n"
            "            FreshRestartCandidateSequence(RestartReplay(node))'):\n"
            "          replayCandidate.causalOrigin\n"
            "            \\notin "
            "AsyncScheduledCandidateOriginsForNodeAfter(node)",
            "SequenceSet(\n"
            "            FreshRestartCandidateSequence(RestartReplay(node))'):\n"
            "          TRUE",
        ),
        (
            "AsyncCandidateLifecycleDurableReplayOriginsForNodeAfter",
            "SequenceSet(HistoricalLockedRetransmitSuccessors(node)'):\n"
            "          replayCandidate.causalOrigin\n"
            "            \\notin "
            "AsyncScheduledCandidateOriginsForNodeAfter(node)",
            "SequenceSet(HistoricalLockedRetransmitSuccessors(node)'):\n"
            "          TRUE",
        ),
        (
            "AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter",
            "FreshRestartCandidateSequence(\n"
            "            RestartReplay(node))'[candidateIndex].causalOrigin\n"
            "            \\notin "
            "AsyncScheduledCandidateOriginsForNodeAfter(node)",
            "TRUE",
        ),
        (
            "AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter",
            "HistoricalLockedRetransmitSuccessors(node)'[\n"
            "                    candidateIndex].causalOrigin\n"
            "                    \\notin "
            "AsyncScheduledCandidateOriginsForNodeAfter(node)",
            "TRUE",
        ),
        (
            "AsyncCandidateLifecycleActiveOriginsForNodeIn",
            "candidateRecord.slot \\in AsyncCandidateLifecycleActiveSlots",
            "TRUE",
        ),
        (
            "AsyncLeaderWireIngressCarrierCoordinates",
            "AsyncLeaderWireAdmissionMatchesRecord(\n"
            "            IngressLane(record.recipient, source)[laneIndex], "
            "record)",
            "TRUE",
        ),
        (
            "AsyncOrdinaryIngressCarrierCoordinates",
            "ExactAsyncCandidateIdentity(\n"
            "            DeliveryCandidate(\n"
            "              IngressLane(carrier.node, source)[laneIndex]))\n"
            "            = carrier.carrierIdentity",
            "TRUE",
        ),
        (
            "AsyncLeaderWireIngressCarrierCoordinatesAfter",
            "AsyncLeaderWireAdmissionMatchesRecord(\n"
            "            asyncIngressLanes'[record.recipient][source]"
            "[laneIndex],\n"
            "            record)",
            "TRUE",
        ),
    ),
)
def test_async_source_fidelity_rejects_weakened_filtered_projection(
    tmp_path: Path,
    symbol: str,
    critical_filter: str,
    weakened_filter: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    assert module._async_source_fidelity_errors(formal_dir) == []
    extracted = module._top_level_operator_body(
        source,
        symbol,
        preserve_string_contents=True,
    )
    assert extracted is not None
    assert extracted[0].count(critical_filter) == 1, (
        symbol,
        critical_filter,
    )

    path.write_text(
        mutate_tla_operator(
            source,
            symbol,
            critical_filter,
            weakened_filter,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        symbol in error and "exact filtered projection" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "exact_consumer"),
    (
        (
            "CertifiedResponseClaimAdmissionMatchesPostStateLifecycleCarrier",
            "AsyncLeaderWireIngressCarrierCoordinatesAfter(\n"
            "                           record)",
        ),
        (
            "DormantLeaderWireReactivationPublishesOneFreshPhysicalCarrier",
            "AsyncLeaderWireIngressCarrierCoordinatesAfter(after)",
        ),
    ),
)
def test_async_source_fidelity_rejects_bypassed_post_state_coordinates(
    tmp_path: Path,
    symbol: str,
    exact_consumer: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    assert module._async_source_fidelity_errors(formal_dir) == []
    extracted = module._top_level_theorem_body(
        source,
        symbol,
        preserve_string_contents=True,
    )
    assert extracted is not None
    assert extracted[0].count(exact_consumer) == 1, (symbol, exact_consumer)

    path.write_text(
        mutate_tla_theorem(source, symbol, exact_consumer, "{}"),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        symbol in error
        and "exact post-state filtered-coordinate projection" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("filename", "symbol", "critical_filter"),
    (
        (
            "SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs.tla",
            "HistoricalDiscoveryPacketCandidateCoveredIdentitySet",
            "candidateRecord.causalOrigin\n"
            "            \\in "
            "HistoricalDiscoveryPacketCandidateCausalOriginCarrier(\n"
            "                 packet)",
        ),
        (
            "SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs.tla",
            "HistoricalDiscoveryPacketCandidateCoveredIdentitySet",
            "candidateRecord.origin\n"
            "            \\in "
            "HistoricalDiscoveryPacketCandidateCausalOriginCarrier(\n"
            "                 packet)",
        ),
        (
            "SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs.tla",
            "HistoricalDiscoveryPacketServeCoveredIdentitySet",
            "serveReservation.identity \\in carrier",
        ),
        (
            "SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs.tla",
            "HistoricalDiscoveryPacketServeCoveredIdentitySet",
            "serveTombstone.identity \\in carrier",
        ),
        (
            "SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs.tla",
            "HistoricalDiscoveryPacketServeCoveredIdentitySet",
            "rollbackTombstone.identity \\in carrier",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralLiveCandidateIdentitySet",
            "scheduledCandidate.node \\in Responsive",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralLiveServeIdentitySet",
            'serveJob.class = "Serve"',
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralServeEpisodeUniverse",
            "requestPacket.item.kind \\in AsyncReplyRequestKinds",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeCandidateOriginsAt",
            "owned.node = node",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeCandidateOriginsAt",
            "owned.ordinal < schedulerCeiling",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeCandidateOriginsAt",
            "owned.sourcePhysicalOrdinal < physicalCut",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeCandidateOriginsAt",
            "owned.recipient = node",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeCandidateOriginsAt",
            "owned.schedulerOrdinal < schedulerCeiling",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeCandidateOriginsAt",
            "owned.physicalAdmissionOrdinal < physicalCut",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeCandidateOriginsAt",
            "AsyncLeaderWireLifecycleActive(owned)",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeContinuationSourcesAt",
            "owned.node = node",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeContinuationSourcesAt",
            "owned.ordinal < schedulerCeiling",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeContinuationSourcesAt",
            "owned.sourcePhysicalOrdinal < physicalCut",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeContinuationSourcesAt",
            "owned.recipient = node",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeContinuationSourcesAt",
            "owned.schedulerOrdinal < schedulerCeiling",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeContinuationSourcesAt",
            "owned.physicalAdmissionOrdinal < physicalCut",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeContinuationSourcesAt",
            "AsyncLeaderWireLifecycleActive(owned)",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeServeSourcesAt",
            "owned.node = node",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeServeSourcesAt",
            "owned.schedulerOrdinal < schedulerCeiling",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeServeSourcesAt",
            "owned.ordinal < physicalCut",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeLeaderWireIdentitiesAt",
            "owned.recipient = node",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeLeaderWireIdentitiesAt",
            "owned.schedulerOrdinal < schedulerCeiling",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeLeaderWireIdentitiesAt",
            "owned.physicalAdmissionOrdinal < physicalCut",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeLeaderWireIdentitiesAt",
            "AsyncLeaderWireLifecycleActive(owned)",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralLiveCandidateCausalRoots",
            "scheduled.node \\in Responsive",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralLifecycleRecordCausalRoots",
            "owned.node \\in Responsive",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralLifecycleRecordCausalRoots",
            "~owned.retired",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralProducerContinuationCausalRoots",
            "owned.node \\in Responsive",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralProducerContinuationCausalRoots",
            'owned.status \\in {"Reserved", "Materialized"}',
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralOrdinaryIngressCausalRoots",
            "owned.node \\in Responsive",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralLeaderWireCausalRoots",
            "owned.recipient \\in Responsive",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralLeaderWireCausalRoots",
            "AsyncLeaderWireLifecycleActive(owned)",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralTimeoutReservationCausalRoots",
            "AsyncUnmaterializedTimeoutLifecycleReservationIn(\n"
            "            asyncControlServiceState, owner)",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralDuePacketCausalRoots",
            "due.item.envelope.recipient \\in Responsive",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralCandidateEpisodeUniverse",
            "ExactDecisionTargetNeutralCausalRoot(\n"
            "            scheduled.node, scheduled.causalOrigin)\n"
            "            \\in "
            "ExactDecisionTargetNeutralFrozenCausalRoots(clockValue)",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot",
            "owned.node = node",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot",
            "owned.ordinal <= snapshot.schedulerCuts[node]",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionTargetNeutralFrozenPredecessorOriginsForSnapshot",
            "owned.sourcePhysicalOrdinal < snapshot.physicalCuts[node]",
        ),
        (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
            "AdequateLeaderFixedPreAdmissionSubjectReplacementRoutes",
            "retainedItem.source\n"
            "               \\in "
            "AdequateLeaderFrozenResponsiveRoster(leaderContext)",
        ),
        (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
            "AdequateLeaderFixedPreAdmissionSubjectReplacementRoutes",
            "AdequateLeaderFixedSubjectReplacementOrigin(\n"
            "               AsyncLeaderWireLifecycleCausalOriginAt(\n"
            "                 retainedItem, leaderContext),\n"
            "               target, leaderContext, leader, leaderView)",
        ),
        (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
            "AdequateLeaderFixedActiveWireSubjectReplacementOwners",
            "AsyncLeaderWireLifecycleActive(activeRecord)",
        ),
        (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
            "AdequateLeaderFixedActiveWireSubjectReplacementOwners",
            "AdequateLeaderFixedSubjectReplacementOrigin(\n"
            "                   activeRecord.causalOrigin,\n"
            "                   target, leaderContext, leader, leaderView)",
        ),
        (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
            "AdequateLeaderFixedDormantWireSubjectReplacementOwners",
            "AsyncLeaderWireLifecycleDormant(dormantRecord)",
        ),
        (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
            "AdequateLeaderFixedDormantWireSubjectReplacementOwners",
            "AdequateLeaderFixedSubjectReplacementOrigin(\n"
            "                   dormantRecord.causalOrigin,\n"
            "                   target, leaderContext, leader, leaderView)",
        ),
        (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
            "AdequateLeaderFixedProducerSubjectReplacementOwners",
            'producerRecord.status \\in {"Reserved", "Materialized"}',
        ),
        (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
            "AdequateLeaderFixedProducerSubjectReplacementOwners",
            "wireLifecycle.recipient = record.node",
        ),
        (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
            "AdequateLeaderFixedProducerSubjectReplacementOwners",
            "wireLifecycle.causalOrigin = record.causalOrigin",
        ),
        (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
            "AdequateLeaderFixedProducerSubjectReplacementOwners",
            "wireLifecycle.schedulerOrdinal = record.ordinal",
        ),
        (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
            "AdequateLeaderFixedProducerSubjectReplacementOwners",
            "AdequateLeaderFixedSubjectReplacementOrigin(\n"
            "                   record.causalOrigin,\n"
            "                   target, leaderContext, leader, leaderView)",
        ),
        (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
            "AdequateLeaderFixedDiscoveredPipelineOriginPairs",
            "AdequateLeaderFixedOriginIsExactPipelineEpisode(\n"
            "              pipelineOrigin, leaderContext,\n"
            "              leader, leaderView, subject)",
        ),
        (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
            "AdequateLeaderFixedLivePipelineOriginPairs",
            "AdequateLeaderFixedOriginIsExactPipelineEpisode(\n"
            "              pipelineOrigin, leaderContext,\n"
            "              leader, leaderView, subject)",
        ),
        (
            "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs.tla",
            "AdequateLeaderAuthenticatedTcEpisodePhysicalOwners",
            "TimeoutTcInstallWalOwner(\n"
            "                stageOwner[2], tc, tc.view)",
        ),
    ),
)
def test_nested_filtered_projection_contract_rejects_removed_filter(
    tmp_path: Path,
    filename: str,
    symbol: str,
    critical_filter: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    path = formal_dir / filename
    shutil.copy2(module.FORMAL_DIR / filename, path)
    assert module._nested_filtered_projection_contract_errors(formal_dir) == []

    source = path.read_text(encoding="utf-8")
    extracted = module._top_level_operator_body(
        source,
        symbol,
        preserve_string_contents=True,
    )
    assert extracted is not None
    assert extracted[0].count(critical_filter) == 1, (
        symbol,
        critical_filter,
    )
    path.write_text(
        mutate_tla_operator(source, symbol, critical_filter, "TRUE"),
        encoding="utf-8",
    )

    errors = module._nested_filtered_projection_contract_errors(formal_dir)

    assert any(
        symbol in error and "exact nested filtered projection" in error
        for error in errors
    ), errors


def test_historical_clock_witness_implication_requires_parenthesized_frontier(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    filename = "SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs.tla"
    path = formal_dir / filename
    shutil.copy2(module.FORMAL_DIR / filename, path)
    assert module._non_vacuous_async_quantifier_contract_errors(formal_dir) == []

    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            "HistoricalDiscoveryFixedClockClosureLowersClockBudgetFromSupport",
            "<3>6. (/\\ AsyncStrongTypeInvariant\n"
            "              /\\ "
            "HistoricalDiscoveryClockBudgetFrontier(node, budget))\n"
            "             => \\E clockValue \\in Nat:",
            "<3>6. /\\ AsyncStrongTypeInvariant\n"
            "              /\\ "
            "HistoricalDiscoveryClockBudgetFrontier(node, budget)\n"
            "             => \\E clockValue \\in Nat:",
        ),
        encoding="utf-8",
    )

    errors = module._non_vacuous_async_quantifier_contract_errors(formal_dir)

    assert any(
        "HistoricalDiscoveryFixedClockClosureLowersClockBudgetFromSupport"
        in error
        and "parenthesized budget-frontier witness implication" in error
        for error in errors
    ), errors


def test_historical_clock_release_witness_requires_parenthesized_frontier(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    filename = "SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs.tla"
    path = formal_dir / filename
    shutil.copy2(module.FORMAL_DIR / filename, path)
    assert module._non_vacuous_async_quantifier_contract_errors(formal_dir) == []

    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            "HistoricalDiscoveryClockBudgetClosureReachesReleaseGoalFromSupport",
            "<3>3. (/\\ AsyncStrongTypeInvariant\n"
            "              /\\ gst\n"
            "              /\\ HistoricalRecoveryTarget(node)\n"
            "              /\\ "
            "~HistoricalDiscoveryClockProgressGoal(node))\n"
            "             => \\E budget \\in Nat:",
            "<3>3. /\\ AsyncStrongTypeInvariant\n"
            "              /\\ gst\n"
            "              /\\ HistoricalRecoveryTarget(node)\n"
            "              /\\ "
            "~HistoricalDiscoveryClockProgressGoal(node)\n"
            "             => \\E budget \\in Nat:",
        ),
        encoding="utf-8",
    )

    errors = module._non_vacuous_async_quantifier_contract_errors(formal_dir)

    assert any(
        "HistoricalDiscoveryClockBudgetClosureReachesReleaseGoalFromSupport"
        in error
        and "parenthesized budget-frontier witness implication" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "parenthesized", "chained"),
    (
        (
            "HistoricalRunnerEpisodeOwnerUsesAsyncFairness",
            "  \\A initialContext, node, ownerKind:\n"
            "    (/\\ node \\in Responsive\n"
            "     /\\ ownerKind \\in HistoricalRunnerEpisodeFairOwnerKinds)\n"
            "      => (AsyncSpecAt(initialContext)\n"
            "            => WF_AsyncAllVars(\n"
            "                 HistoricalRunnerEpisodeFairAction("
            "node, ownerKind)))",
            "  \\A initialContext, node, ownerKind:\n"
            "    /\\ node \\in Responsive\n"
            "    /\\ ownerKind \\in HistoricalRunnerEpisodeFairOwnerKinds\n"
            "    => AsyncSpecAt(initialContext)\n"
            "         => WF_AsyncAllVars(\n"
            "              HistoricalRunnerEpisodeFairAction("
            "node, ownerKind))",
        ),
        (
            "HistoricalDiscoveryServeExactWorkerUsesAsyncFairness",
            "  \\A initialContext, recipient, workerKind:\n"
            "    (/\\ recipient \\in Responsive\n"
            "     /\\ workerKind\n"
            "          \\in "
            "HistoricalDiscoveryServeExactWorkerActionKindCarrier)\n"
            "      => (AsyncSpecAt(initialContext)\n"
            "            => WF_AsyncAllVars(\n"
            '                 CASE workerKind = "ServiceIo" ->\n'
            "                        PostGstServiceIoWorker(recipient)\n"
            '                   [] workerKind = "ServiceHistoricalIo" ->\n'
            "                        "
            "PostGstServiceHistoricalRecoveryIoWorker(recipient)\n"
            "                   [] OTHER -> FALSE))",
            "  \\A initialContext, recipient, workerKind:\n"
            "    /\\ recipient \\in Responsive\n"
            "    /\\ workerKind\n"
            "         \\in "
            "HistoricalDiscoveryServeExactWorkerActionKindCarrier\n"
            "    => AsyncSpecAt(initialContext)\n"
            "         => WF_AsyncAllVars(\n"
            '              CASE workerKind = "ServiceIo" ->\n'
            "                     PostGstServiceIoWorker(recipient)\n"
            '                [] workerKind = "ServiceHistoricalIo" ->\n'
            "                     "
            "PostGstServiceHistoricalRecoveryIoWorker(recipient)\n"
            "                [] OTHER -> FALSE)",
        ),
    ),
)
def test_historical_runner_fairness_rejects_chained_implication(
    tmp_path: Path,
    symbol: str,
    parenthesized: str,
    chained: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    filename = "SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs.tla"
    path = formal_dir / filename
    shutil.copy2(module.FORMAL_DIR / filename, path)
    assert module._non_vacuous_async_quantifier_contract_errors(formal_dir) == []

    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(source, symbol, parenthesized, chained),
        encoding="utf-8",
    )

    errors = module._non_vacuous_async_quantifier_contract_errors(formal_dir)

    assert any(
        symbol in error
        and "exact parenthesized historical fairness implication" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("filename", "symbol", "declaration_kind", "old", "grouped"),
    (
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestClockPrefixStepIsDescentOrFrame",
            "theorem",
            "  \\A kind \\in ExactDecisionRequestClockPrefixKinds:\n"
            "    \\A snapshot, node, qc, ownerOrdinal:\n"
            "      \\A rank \\in "
            "ExactDecisionRequestRuntimeFrozenPrefixCarrier:",
            "  \\A kind \\in ExactDecisionRequestClockPrefixKinds,\n"
            "     snapshot, node, qc, ownerOrdinal,\n"
            "     rank \\in "
            "ExactDecisionRequestRuntimeFrozenPrefixCarrier:",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestClockPrefixContinuationClosureProperty",
            "operator",
            "  specification\n"
            "    => \\A kind \\in ExactDecisionRequestClockPrefixKinds:\n"
            "         \\A snapshot, qc, ownerOrdinal:\n"
            "           \\A node \\in AsyncVotersAt(initialContext):\n"
            "             \\A rank \\in "
            "ExactDecisionRequestRuntimeFrozenPrefixCarrier:",
            "  specification\n"
            "    => \\A kind \\in ExactDecisionRequestClockPrefixKinds,\n"
            "          snapshot,\n"
            "          node \\in AsyncVotersAt(initialContext), qc, "
            "ownerOrdinal,\n"
            "          rank \\in "
            "ExactDecisionRequestRuntimeFrozenPrefixCarrier:",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestClockPrefixResolvedOwnerIsEnabled",
            "theorem",
            "  \\A kind \\in ExactDecisionRequestClockPrefixKinds:\n"
            "    \\A snapshot, node, qc, ownerOrdinal:\n"
            "      \\A rank \\in "
            "ExactDecisionRequestRuntimeFrozenPrefixCarrier:",
            "  \\A kind \\in ExactDecisionRequestClockPrefixKinds,\n"
            "     snapshot, node, qc, ownerOrdinal,\n"
            "     rank \\in "
            "ExactDecisionRequestRuntimeFrozenPrefixCarrier:",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestClockPrefixResolvedOwnerConsumesRankCell",
            "theorem",
            "  \\A kind \\in ExactDecisionRequestClockPrefixKinds:\n"
            "    \\A snapshot, node, qc, ownerOrdinal:\n"
            "      \\A rank \\in "
            "ExactDecisionRequestRuntimeFrozenPrefixCarrier:",
            "  \\A kind \\in ExactDecisionRequestClockPrefixKinds,\n"
            "     snapshot, node, qc, ownerOrdinal,\n"
            "     rank \\in "
            "ExactDecisionRequestRuntimeFrozenPrefixCarrier:",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestClockPrefixResolvedFairOwnerIsStable",
            "theorem",
            "  \\A kind \\in ExactDecisionRequestClockPrefixKinds:\n"
            "    \\A snapshot, node, qc, ownerOrdinal:\n"
            "      \\A rank \\in "
            "ExactDecisionRequestRuntimeFrozenPrefixCarrier:",
            "  \\A kind \\in ExactDecisionRequestClockPrefixKinds,\n"
            "     snapshot, node, qc, ownerOrdinal,\n"
            "     rank \\in "
            "ExactDecisionRequestRuntimeFrozenPrefixCarrier:",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestClockPrefixResolvedRankStepProperty",
            "operator",
            "  specification\n"
            "    => \\A kind \\in ExactDecisionRequestClockPrefixKinds:\n"
            "         \\A snapshot, qc, ownerOrdinal:\n"
            "           \\A node \\in AsyncVotersAt(initialContext):\n"
            "             \\A rank \\in "
            "ExactDecisionRequestRuntimeFrozenPrefixCarrier:",
            "  specification\n"
            "    => \\A kind \\in ExactDecisionRequestClockPrefixKinds,\n"
            "          snapshot,\n"
            "          node \\in AsyncVotersAt(initialContext), qc, "
            "ownerOrdinal,\n"
            "          rank \\in "
            "ExactDecisionRequestRuntimeFrozenPrefixCarrier:",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestClockPrefixRankStepProperty",
            "operator",
            "  specification\n"
            "    => \\A kind \\in ExactDecisionRequestClockPrefixKinds:\n"
            "         \\A snapshot, qc, ownerOrdinal:\n"
            "           \\A node \\in AsyncVotersAt(initialContext):\n"
            "             \\A rank \\in "
            "ExactDecisionRequestRuntimeFrozenPrefixCarrier:",
            "  specification\n"
            "    => \\A kind \\in ExactDecisionRequestClockPrefixKinds,\n"
            "          snapshot,\n"
            "          node \\in AsyncVotersAt(initialContext), qc, "
            "ownerOrdinal,\n"
            "          rank \\in "
            "ExactDecisionRequestRuntimeFrozenPrefixCarrier:",
        ),
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestClockPrefixClosureProperty",
            "operator",
            "  specification\n"
            "    => \\A kind \\in ExactDecisionRequestClockPrefixKinds:\n"
            "         \\A snapshot, qc, ownerOrdinal:\n"
            "           \\A node \\in AsyncVotersAt(initialContext):",
            "  specification\n"
            "    => \\A kind \\in ExactDecisionRequestClockPrefixKinds,\n"
            "          snapshot,\n"
            "          node \\in AsyncVotersAt(initialContext), qc, "
            "ownerOrdinal:",
        ),
        (
            "SumeragiV2TimeoutViewProgressProofs.tla",
            "TimeoutPhysicalControlTickLowersRetainedClockRank",
            "theorem",
            "  \\A item:\n    \\A rank \\in Nat:",
            "  \\A item, rank \\in Nat:",
        ),
        (
            "SumeragiV2LockedBodyProposalActionProofs.tla",
            "LockedBodyIgnoredProposalProducerHasDurableDisposition",
            "theorem",
            "    \\A prepareQc:\n"
            "      \\A candidate \\in AsyncCandidateSet:",
            "    \\A prepareQc, candidate \\in AsyncCandidateSet:",
        ),
        (
            "SumeragiV2LockedBodyProposalActionProofs.tla",
            "LockedBodyScheduledProposalProducerDepartureIsClassified",
            "theorem",
            "    \\A prepareQc:\n"
            "      \\A candidate \\in AsyncCandidateSet:",
            "    \\A prepareQc, candidate \\in AsyncCandidateSet:",
        ),
        (
            "SumeragiV2AsyncDeadlockProofs.tla",
            "Stage2BusyLocalWorkDecreaseStep",
            "operator",
            "  \\E target, witness \\in AsyncCandidateSet, "
            "phase \\in 1..2:",
            "  \\E target, witness, phase \\in 1..2:",
        ),
        (
            "SumeragiV2AdequateLeaderServiceClosureProofs.tla",
            "FabricatedStaleUnownedPersistDecisionCannotTriggerRankStep",
            "theorem",
            "  \\A candidate:\n"
            "    \\A mode \\in AdequateLeaderCompositionModes,",
            "  \\A mode \\in AdequateLeaderCompositionModes, candidate,",
        ),
        (
            "SumeragiV2AsyncFiniteRunnerEpisodeProofs.tla",
            "AsyncReadyRunnerEpisodeRankStepProperty",
            "operator",
            "  specification\n"
            "    => \\A candidate, position, baselineRank:\n"
            "         \\A kind \\in AsyncReadyRunnerEpisodeKinds:\n"
            "           \\A episodeRank \\in "
            "AsyncReadyRunnerEpisodeRankCarrier:",
            "  specification\n"
            "    => \\A kind \\in AsyncReadyRunnerEpisodeKinds,\n"
            "         candidate, position, baselineRank,\n"
            "         episodeRank \\in AsyncReadyRunnerEpisodeRankCarrier:",
        ),
        (
            "SumeragiV2AsyncFiniteRunnerEpisodeProofs.tla",
            "AsyncCapacityRunnerEpisodeRankStepProperty",
            "operator",
            "  specification\n"
            "    => \\A candidate, position, baselineRank:\n"
            "         \\A kind \\in AsyncCapacityRunnerEpisodeKinds:\n"
            "           \\A episodeRank \\in "
            "AsyncCapacityRunnerEpisodeRankCarrier:",
            "  specification\n"
            "    => \\A kind \\in AsyncCapacityRunnerEpisodeKinds,\n"
            "         candidate, position, baselineRank,\n"
            "         episodeRank \\in AsyncCapacityRunnerEpisodeRankCarrier:",
        ),
        (
            "SumeragiV2AsyncDecisionApplicationProofs.tla",
            "DecisionPipelineStagePendingIsProtected",
            "theorem",
            "  \\A qc:\n"
            "    \\A node \\in AsyncCurrentResponsiveVoters,\n"
            "       kind \\in DecisionPipelineKinds,",
            "  \\A node \\in AsyncCurrentResponsiveVoters,\n"
            "     qc, kind \\in DecisionPipelineKinds,",
        ),
        (
            "SumeragiV2AsyncDecisionApplicationProofs.tla",
            "DecisionPipelineStagePersistsUntilExactHandoff",
            "theorem",
            "  \\A qc:\n"
            "    \\A node \\in AsyncCurrentResponsiveVoters,\n"
            "       kind \\in DecisionPipelineKinds,",
            "  \\A node \\in AsyncCurrentResponsiveVoters,\n"
            "     qc, kind \\in DecisionPipelineKinds,",
        ),
        (
            "SumeragiV2AsyncDecisionApplicationProofs.tla",
            "DecisionPipelineStageReachesExactHandoff",
            "theorem",
            "  \\A initialContext:\n"
            "    \\A qc:\n"
            "      \\A node \\in AsyncVotersAt(initialContext),\n"
            "         kind \\in DecisionPipelineKinds,",
            "  \\A initialContext:\n"
            "    \\A node \\in AsyncVotersAt(initialContext),\n"
            "       qc, kind \\in DecisionPipelineKinds,",
        ),
        (
            "SumeragiV2AsyncCausalWorkBudgetProofs.tla",
            "AsyncCausalEpisodeOwnedCutServiceConsumesExactOccurrenceBudget",
            "theorem",
            "  \\A origin:\n"
            "    \\A node \\in ValidatorIds,\n"
            "       cutoffOrdinal \\in Nat \\ {0},",
            "  \\A node \\in ValidatorIds,\n"
            "     origin,\n"
            "     cutoffOrdinal \\in Nat \\ {0},",
        ),
        (
            "SumeragiV2AsyncCausalWorkBudgetProofs.tla",
            "AsyncCausalEpisodeOwnedLifecycleCutCannotReplenish",
            "theorem",
            "  \\A origin:\n"
            "    \\A node \\in ValidatorIds, "
            "cutoffOrdinal \\in Nat \\ {0}:",
            "  \\A node \\in ValidatorIds, origin, "
            "cutoffOrdinal \\in Nat \\ {0}:",
        ),
        (
            "SumeragiV2AsyncCausalWorkBudgetProofs.tla",
            "AsyncCausalEpisodeOwnedLifecycleServeCutCannotReplenish",
            "theorem",
            "  \\A origin:\n"
            "    \\A node \\in ValidatorIds, "
            "cutoffOrdinal \\in Nat \\ {0}:",
            "  \\A node \\in ValidatorIds, origin, "
            "cutoffOrdinal \\in Nat \\ {0}:",
        ),
    ),
)
def test_non_vacuous_async_quantifier_contracts_reject_grouped_binders(
    tmp_path: Path,
    filename: str,
    symbol: str,
    declaration_kind: str,
    old: str,
    grouped: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    path = formal_dir / filename
    shutil.copy2(module.FORMAL_DIR / filename, path)
    checker = module._non_vacuous_async_quantifier_contract_errors
    assert checker(formal_dir) == []

    source = path.read_text(encoding="utf-8")
    mutate = (
        mutate_tla_theorem
        if declaration_kind == "theorem"
        else mutate_tla_operator
    )
    path.write_text(mutate(source, symbol, old, grouped), encoding="utf-8")

    errors = checker(formal_dir)

    assert any(
        symbol in error and "exact non-vacuous binder" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "parenthesized", "chained"),
    (
        (
            "ExactDecisionRequestClockPrefixFairOwnerUsesExistingFairness",
            "    (/\\ snapshot.node \\in AsyncVotersAt(initialContext)\n"
            "     /\\ ExactDecisionRequestClockPrefixFairOwner(snapshot)\n"
            "          \\in AsyncCausalEpisodeFairOwnerKinds)\n"
            "      => (AsyncSpecAt(initialContext)\n"
            "            => WF_AsyncAllVars(\n"
            "                 ExactDecisionRequestClockPrefixFairAction("
            "snapshot)))",
            "    /\\ snapshot.node \\in AsyncVotersAt(initialContext)\n"
            "    /\\ ExactDecisionRequestClockPrefixFairOwner(snapshot)\n"
            "         \\in AsyncCausalEpisodeFairOwnerKinds\n"
            "      => AsyncSpecAt(initialContext)\n"
            "           => WF_AsyncAllVars(\n"
            "                ExactDecisionRequestClockPrefixFairAction("
            "snapshot))",
        ),
        (
            "ExactDecisionRequestLifecycleConcreteOwnerUsesAsyncFairness",
            "    (/\\ archive \\in AsyncVotersAt(initialContext)\n"
            "     /\\ archive \\in Responsive\n"
            "     /\\ ownerKind\n"
            "          \\in "
            "ExactDecisionRequestLifecycleConcreteFairOwnerKinds)\n"
            "      => (AsyncSpecAt(initialContext)\n"
            "            => WF_AsyncAllVars(\n"
            "                 "
            "ExactDecisionRequestLifecycleConcreteFairAction(\n"
            "                   archive, ownerKind)))",
            "    /\\ archive \\in AsyncVotersAt(initialContext)\n"
            "    /\\ archive \\in Responsive\n"
            "    /\\ ownerKind\n"
            "         \\in "
            "ExactDecisionRequestLifecycleConcreteFairOwnerKinds\n"
            "    => AsyncSpecAt(initialContext)\n"
            "         => WF_AsyncAllVars(\n"
            "              "
            "ExactDecisionRequestLifecycleConcreteFairAction(\n"
            "                archive, ownerKind))",
        ),
        (
            "ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness",
            "    owner \\in "
            "ExactDecisionTargetNeutralFairOwnerSet(initialContext)\n"
            "      => (AsyncSpecAt(initialContext)\n"
            "            => WF_AsyncAllVars(\n"
            "                 ExactDecisionTargetNeutralFairAction(owner)))",
            "    owner \\in "
            "ExactDecisionTargetNeutralFairOwnerSet(initialContext)\n"
            "      => AsyncSpecAt(initialContext)\n"
            "           => WF_AsyncAllVars(\n"
            "                ExactDecisionTargetNeutralFairAction(owner))",
        ),
    ),
)
def test_exact_decision_fairness_rejects_unparenthesized_implication_chain(
    tmp_path: Path,
    symbol: str,
    parenthesized: str,
    chained: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    filename = "SumeragiV2ExactDecisionStageServiceClosureProofs.tla"
    path = formal_dir / filename
    shutil.copy2(module.FORMAL_DIR / filename, path)
    checker = module._non_vacuous_async_quantifier_contract_errors
    assert checker(formal_dir) == []

    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(source, symbol, parenthesized, chained),
        encoding="utf-8",
    )

    errors = checker(formal_dir)

    assert any(
        symbol in error
        and "exact parenthesized antecedent/consequent" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("filename", "symbol", "flattened"),
    (
        (
            "SumeragiV2ExactDecisionStageServiceClosureProofs.tla",
            "ExactDecisionRequestRuntimeFrozenPrefixRank",
            (
                "<<ExactDecisionRequestRuntimeOlderTimeoutStage(snapshot),\n"
                "  ExactDecisionRequestRuntimeFrozenIngressStage(snapshot),\n"
                "  <<ExactDecisionRequestRuntimeFrozenSourceRank(snapshot),\n"
                "    ExactDecisionRequestRuntimeFrozenIngressDependencyRank("
                "snapshot)"
                + ">>" * 2
            ),
        ),
        (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
            "AdequateLeaderFixedPipelineRank",
            (
                "LET windows ==\n"
                "      AdequateLeaderFixedAuthorityPipelineWindowsRemaining(\n"
                "        leaderContext, leader, leaderView, subject)\n"
                "    liveSlotDebt ==\n"
                "      Cardinality(\n"
                "        AdequateLeaderFixedLivePipelineOriginSlotsForToken(\n"
                "          token, leaderContext, leader, leaderView, subject))\n"
                "    actionDebt ==\n"
                "      AdequateLeaderFixedPerTokenCumulativeActionDebt(\n"
                "        candidate, candidate.node, cutoffOrdinal, semanticRank)\n"
                "    serviceSlack ==\n"
                "      AdequateLeaderFixedCandidateSelectedServiceSlack(\n"
                "        owner, packet, candidate)\n"
                "IN <<windows, liveSlotDebt, <<actionDebt, serviceSlack"
                + ">>" * 2
            ),
        ),
        (
            "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs.tla",
            "AdequateLeaderFixedPreCandidateEntryRank",
            (
                "<<AdequateLeaderFixedAuthorityPipelineWindowsRemaining(\n"
                "    leaderContext, leader, leaderView, subject),\n"
                "  AdequateLeaderFixedPreCandidateReservedLiveSlotDebt(\n"
                "    token, leaderContext, leader, leaderView, subject),\n"
                "  <<entryDebt,\n"
                "    AdequateLeaderFixedEntryServiceSlack("
                "owner, packet, leader)"
                + ">>" * 2
            ),
        ),
    ),
)
def test_lexicographic_rank_contract_rejects_flattened_cell(
    tmp_path: Path,
    filename: str,
    symbol: str,
    flattened: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    path = formal_dir / filename
    shutil.copy2(module.FORMAL_DIR / filename, path)
    checker = module._lexicographic_rank_shape_contract_errors
    assert checker.__name__ in module.validate_ledger.__code__.co_names
    assert checker(formal_dir) == []

    source = path.read_text(encoding="utf-8")
    path.write_text(
        replace_tla_operator_body(source, symbol, flattened),
        encoding="utf-8",
    )

    errors = checker(formal_dir)

    assert any(
        symbol in error and "finite lexicographic rank nesting" in error
        for error in errors
    ), errors
