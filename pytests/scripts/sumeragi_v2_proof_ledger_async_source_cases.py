def test_effect_candidate_and_completion_capacity_tla_seals_reject_weakening(
    tmp_path: Path,
) -> None:
    """Candidate identity, successor capacity, and Stage 6 closure are sealed."""

    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    check = module._production_causal_fifo_source_fidelity_errors
    assert check(formal_dir) == []
    network = formal_dir / "SumeragiV2AsyncNetwork.tla"
    stage6 = formal_dir / "SumeragiV2AsyncStage6Proofs.tla"
    canonical_network = network.read_text(encoding="utf-8")
    canonical_stage6 = stage6.read_text(encoding="utf-8")

    mutations = (
        (
            network,
            canonical_network,
            "AsyncCausalCandidateLifecycleCapacity ==\n  3 * AsyncQueueCapacity",
            "AsyncCausalCandidateLifecycleCapacity ==\n  4 * AsyncQueueCapacity",
            "AsyncCausalCandidateLifecycleCapacity",
        ),
        (
            network,
            canonical_network,
            "   causalOrigin |-> candidate.causalOrigin,",
            "   causalOrigin |-> NoAsyncCausalOrigin,",
            "ExactAsyncCandidateIdentity",
        ),
        (
            network,
            canonical_network,
            "      successor.causalOrigin = command.causalOrigin",
            "      TRUE",
            "CommandSuccessorsRetainCausalOrigin",
        ),
        (
            network,
            canonical_network,
            "FreshCommandSuccessors(command) ==\n"
            "  LET successors == CommandSuccessors(command)",
            "FreshCommandSuccessors(command) ==\n"
            "  LET successors == <<>>",
            "FreshCommandSuccessors",
        ),
        (
            stage6,
            canonical_stage6,
            "Stage6CompletionCapacityGoal(candidate, position) ==\n"
            "  \\/ ProtectedRankProgressExit(candidate, <<6, position>>)",
            "Stage6CompletionCapacityGoal(candidate, position) ==\n"
            "  \\/ TRUE",
            "Stage6CompletionCapacityGoal",
        ),
        (
            stage6,
            canonical_stage6,
            "THEOREM FairStage6CompletionCapacityOpens ==\n"
            "  \\A initialContext, candidate, position:\n"
            "    Stage4RefinementFiniteServeEpisodeResidualProperty(",
            "THEOREM FairStage6CompletionCapacityOpens ==\n"
            "  \\A initialContext, candidate, position:\n"
            "    TRUE \\/ Stage4RefinementFiniteServeEpisodeResidualProperty(",
            "FairStage6CompletionCapacityOpens",
        ),
    )
    for path, canonical, old, new, symbol in mutations:
        assert canonical.count(old) == 1, (path, symbol)
        path.write_text(canonical.replace(old, new, 1), encoding="utf-8")
        errors = check(formal_dir)
        assert any(symbol in error for error in errors), (symbol, errors)
        path.write_text(canonical, encoding="utf-8")


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
       HistoricalLockedCommitWalMatches(node, qc, request)
  \/ \E candidate \in AsyncCandidateSet:
       HistoricalBeginLockRecoveryCandidate(node, qc, candidate)
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
            "     /\\ NodeIdle(node)\n"
            "     /\\ NoDecisionForNode(node)\n"
            "     /\\ pendingInstallTC' = pendingInstallTC \\cup {request}\n",
            "     /\\ NodeIdle(node)\n"
            "     /\\ (NoDecisionForNode(node) \\/ TRUE)\n"
            "     /\\ pendingInstallTC' = pendingInstallTC \\cup {request}\n",
            "must have one direct, NoDecisionForNode guard",
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
            "     /\\ timeoutNetwork' = timeoutNetwork \\ {envelope}\n",
            "     /\\ timeoutNetwork' = timeoutNetwork\n",
            "DeliverTimeout must preserve the reviewed atomic timeout",
        ),
        (
            "          IF NoDecisionForNode(envelope.recipient)\n",
            "          IF TRUE\n",
            "DeliverTC must preserve the reviewed atomic timeout",
        ),
        (
            "     /\\ tcNetwork' = tcNetwork \\ {envelope}\n",
            "     /\\ tcNetwork' = tcNetwork\n",
            "DeliverTC must preserve the reviewed atomic timeout",
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
            "Generations",
            " IF ViewDomain = Nat THEN Nat ELSE 0..MaxGeneration\n",
            " 0..MaxGeneration\n",
            "Generations must equal only",
        ),
        (
            "GenerationCanIncrement",
            "  ViewDomain = Nat \\/ value < MaxGeneration\n",
            "  TRUE\n",
            "GenerationCanIncrement must equal only",
        ),
        (
            "TypeInvariant",
            "  /\\ generation \\in [ValidatorIds -> Generations]\n",
            "  /\\ generation \\in [ValidatorIds -> Generations]\n"
            "  /\\ \\A node \\in ValidatorIds:\n"
            "       generation[node] <= highestRank[node] + 1\n",
            "TypeInvariant must not couple same-view executor restart "
            "generations to Prepare rank",
        ),
        (
            "GenerationCanIncrement",
            "  ViewDomain = Nat \\/ value < MaxGeneration\n",
            "  value < MaxGeneration\n",
            "GenerationCanIncrement must equal only",
        ),
        (
            "NoHigherPrepareOriginKnown",
            "       /\\ vote.view > qc.view\n",
            "       /\\ vote.view > qc.view\n"
            "       /\\ vote.subject # qc.subject\n",
            "NoHigherPrepareOriginKnown must equal only",
        ),
        (
            "StrictSameRoundTcUpgrade",
            "  /\\ TcHighRank(tc) > lockRank[node]\n",
            "  /\\ TcHighRank(tc) >= lockRank[node]\n",
            "StrictSameRoundTcUpgrade must equal only",
        ),
        (
            "StrictSameRoundTcUpgrade",
            "  /\\ GenerationCanIncrement(generation[node])\n",
            "",
            "StrictSameRoundTcUpgrade must equal only",
        ),
        (
            "TimeoutReceiptAdmitted",
            "  /\\ vote.view <= nodeView[node] + 1\n",
            "  /\\ vote.view <= nodeView[node] + 2\n",
            "TimeoutReceiptAdmitted must equal only",
        ),
        (
            "ProposalJustified",
            "     /\\ proposal.justifyRank < proposal.view\n",
            "     /\\ proposal.justifyRank <= proposal.view\n",
            "ProposalJustified must equal only",
        ),
        (
            "SafeToPrepare",
            "  \\/ proposal.subject = lockSubject[node]\n",
            "  \\/ TRUE\n",
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
            "| SumeragiV2LivenessBlocker::SuccessorActivationPending",
            "",
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
            '         THEN <<CausalCandidate("Progress", "BeginInstallTC", command)>>\n'
            "         ELSE <<>>\n",
            '         <<CausalCandidate("Progress", "BeginInstallTC", command)>>\n',
            "post-Decision DeliverTC must emit no causal successor",
        ),
        (
            "         ELSE <<>>\n    [] command.kind = \"DeliverTC\" ->",
            '         ELSE <<CausalCandidate("Completion", "PersistInstallTC", command)>>\n'
            '    [] command.kind = "DeliverTC" ->',
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
            "      BY <1>1, <2>13, ResumeTimeoutPreservesDecisionTimeoutFrontier\n",
            "      BY <1>1, <2>13, CrashPreservesDecisionTimeoutFrontier\n",
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
    temporal_path = formal_dir / "SumeragiV2AsyncTemporalClosureProofs.tla"
    canonical_temporal = temporal_path.read_text(encoding="utf-8")
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
            "CommitRecoveryAuthority(node) ==\n"
            "  /\\ asyncRecoveryPhase\n"
            '       \\in {"RestartRequired", "ReplayRequired", "Replaying"}\n'
            "  /\\ asyncRecoveryNode = node\n"
            "  /\\ generation[node] = asyncRecoveryGeneration\n",
            "CommitRecoveryAuthority(node) ==\n"
            "  /\\ asyncRecoveryPhase\n"
            '       \\in {"RestartRequired", "ReplayRequired", "Replaying"}\n'
            "  /\\ asyncRecoveryNode = node\n"
            "  /\\ generation[node] <= asyncRecoveryGeneration\n",
            "CommitRecoveryAuthority must equal only",
        ),
        (
            "CommitRecoveryAuthority(node) ==\n"
            "  /\\ asyncRecoveryPhase\n"
            '       \\in {"RestartRequired", "ReplayRequired", "Replaying"}\n'
            "  /\\ asyncRecoveryNode = node\n"
            "  /\\ generation[node] = asyncRecoveryGeneration\n",
            "CommitRecoveryAuthority(node) ==\n"
            "  /\\ asyncRecoveryPhase\n"
            '       \\in {"RestartRequired", "ReplayRequired", "Replaying"}\n'
            "  /\\ generation[node] = asyncRecoveryGeneration\n",
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
            "AsyncProgressWitnessAndHistoricalRecoveryProperty(AsyncSpecAt(initialContext))",
            "AsyncProgressWitnessProperty(AsyncSpecAt(initialContext))",
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
            "THEOREM PersistDecisionRecoveryUsesBodyStateCompletion ==\n"
            "  \\A command:\n"
            "    /\\ command.kind = \"PersistDecision\"\n",
            "THEOREM PersistDecisionRecoveryUsesBodyStateCompletion ==\n"
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
            "BY DEF CommandSuccessors, PersistDecisionRecoverySuccessor,\n"
            "       PersistDecisionRecoveryKind, PersistDecisionBody,\n"
            "       PersistDecisionValidationHeld, PersistDecisionRequest,\n"
            "       AsyncCandidateAtConsumerWithOrigin,\n"
            "       AsyncCandidateWithIdentityAndOrigin,\n"
            "       CandidateConsumerCurrent, PersistDecisionRequests\n",
            "BY DEF CommandSuccessors, PersistDecisionRecoverySuccessor,\n"
            "       PersistDecisionRecoveryKind, PersistDecisionBody,\n"
            "       PersistDecisionValidationHeld, PersistDecisionRequest,\n"
            "       AsyncCandidateAtConsumerWithOrigin,\n"
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
        target_path, target_source = (
            (path, canonical)
            if needle in canonical
            else (temporal_path, canonical_temporal)
        )
        assert needle in target_source, needle
        target_path.write_text(
            target_source.replace(needle, replacement, 1), encoding="utf-8"
        )
        errors = module._progress_witness_source_fidelity_errors(formal_dir)
        assert any(expected_error in error for error in errors), (
            expected_error,
            errors,
        )
        target_path.write_text(target_source, encoding="utf-8")

    async_network_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    canonical_async_network = async_network_path.read_text(encoding="utf-8")
    async_network_mutations = (
        (
            "    /\\ request.qc.subject = command.subject}\n",
            "    /\\ request.qc.subject = command.view}\n",
            "PersistDecisionRequests must equal only",
        ),
        (
            '       "Completion", PersistDecisionRecoveryKind(command),\n',
            '       "Progress", PersistDecisionRecoveryKind(command),\n',
            "PersistDecisionRecoverySuccessor must equal only",
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
            "THEOREM ResponsiveRestartAdvancesExactDurableDecisionAuthority ==\n"
            "  \\A node, qc:\n"
            "    /\\ TypeInvariant\n"
            "    /\\ DurableDecisionRecoveryAuthority(node, qc)\n"
            "    /\\ PreGstResponsiveRestart\n"
            "    => /\\ generation'[node] = generation[node] + 1\n",
            "THEOREM ResponsiveRestartAdvancesExactDurableDecisionAuthority ==\n"
            "  \\A node, qc:\n"
            "    /\\ TypeInvariant\n"
            "    /\\ DurableDecisionRecoveryAuthority(node, qc)\n"
            "    /\\ PreGstResponsiveRestart\n"
            "    => /\\ generation'[node] = generation[node]\n",
            "ResponsiveRestartAdvancesExactDurableDecisionAuthority must state only",
        ),
        (
            "BY RestartIncrementsSelectedGeneration, SMT\n"
            "   DEF DurableDecisionRecoveryAuthority,\n",
            "BY SMT\n"
            "   DEF DurableDecisionRecoveryAuthority,\n",
            "ResponsiveRestartAdvancesExactDurableDecisionAuthority must retain its complete",
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
            "  AsyncCertifiedSignedRequest(node, qc, 0)\n",
            "DecisionRawSignedRequest(node, qc) ==\n"
            "  AsyncCertifiedSignedRequest(node, qc, 1)\n",
            "DecisionRawSignedRequest must equal only",
        ),
        (
            "DecisionRawRequestHash(node, qc) ==\n"
            "  AsyncCertifiedRequestHashOf(node, qc, 0)\n",
            "DecisionRawRequestHash(node, qc) ==\n"
            "  AsyncCertifiedRequestHashOf(node, qc, 1)\n",
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
            "    => /\\ CurrentDecisionRequestConsumerGeneration(request)'\n"
            "             = CurrentDecisionRequestConsumerGeneration(request) + 1\n"
            "       /\\ CurrentDecisionRequestConsumerGeneration(request)'\n"
            "             # CurrentDecisionRequestConsumerGeneration(request)\n",
            "    => CurrentDecisionRequestConsumerGeneration(request)'\n"
            "         = CurrentDecisionRequestConsumerGeneration(request)\n",
            "AuthenticatedRestartRetagsSourceConsumerGeneration must state only",
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
            "BodyState::Missing => Some(self.ensure_body_fetch(&locked)),",
            "BodyState::Missing => Some(self.ensure_body_fetch(&decision)),",
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
            "AsyncIngressCapacity >= 5 * N + 2",
            "AsyncIngressCapacity >= N + 2",
            1,
        ).replace(
            "           /\\ Len(lanes[recipient][source]) =\n"
            "                Cardinality(\n"
            "                  IngressProtectedClassesPresentIn(\n"
            "                    lanes, recipient, source))\n",
            "           /\\ Len(lanes[recipient][source]) = 4\n",
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
    shutil.copyfile(
        module.FORMAL_DIR / "SumeragiV2OwnershipInvariantCheck.tla",
        formal_dir / "SumeragiV2OwnershipInvariantCheck.tla",
    )
    assert module._ownership_n1_configuration_errors(formal_dir) == []
    assert len(module._OWNERSHIP_N1_DEFINITION_OVERRIDES) == 15
    assert all(
        helper in module._OWNERSHIP_N1_STRUCTURAL_OPERATOR_SHA256
        for _, helper in module._OWNERSHIP_N1_DEFINITION_OVERRIDES
    )

    path.write_text(
        source.replace("  AsyncIngressCapacity = 7\n", "  AsyncIngressCapacity = 6\n", 1),
        encoding="utf-8",
    )
    errors = module._ownership_n1_configuration_errors(formal_dir)
    assert any("exact 5 * N + 2 geometry (7)" in error for error in errors)

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
    assert any("exact 5 * N + 2 geometry (7)" in error for error in errors)
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

    for old, new in (
        ('  ValidSubjects = {"A"}\n', '  ValidSubjects = {"B"}\n'),
        (
            "  AcquisitionSubjects = {AcquisitionSubjectA}\n",
            "  AcquisitionSubjects = {}\n",
        ),
        (
            "  InitialAcquisitionSubject = AcquisitionSubjectA\n",
            "  InitialAcquisitionSubject = AcquisitionSubjectB\n",
        ),
        ("  MaxAcquisitionId = 4\n", "  MaxAcquisitionId = 5\n"),
    ):
        path.write_text(source.replace(old, new, 1), encoding="utf-8")
        errors = module._ownership_n1_configuration_errors(formal_dir)
        assert any(
            "ownership search must retain exact closed assignment" in error
            for error in errors
        ), errors

    for old, new in (
        (
            "  InstallTcFromEvidence <- OwnershipInstallTcFromEvidence\n",
            "",
        ),
        (
            "  InstallTcFromEvidence <- OwnershipInstallTcFromEvidence\n",
            "  InstallTcFromEvidence <- OwnershipInstallTcEvidenceMatches\n",
        ),
    ):
        path.write_text(source.replace(old, new, 1), encoding="utf-8")
        errors = module._ownership_n1_configuration_errors(formal_dir)
        assert any(
            "ordered fifteen-entry structural definition inventory" in error
            for error in errors
        ), errors

    for invariant in (
        "AsyncTypeInvariant",
        "AsyncProgressOwnershipInvariant",
    ):
        path.write_text(
            source.replace(f"INVARIANT {invariant}\n", "", 1),
            encoding="utf-8",
        )
        errors = module._ownership_n1_configuration_errors(formal_dir)
        assert any(
            f"exact closed assignment 'INVARIANT {invariant}' once" in error
            for error in errors
        ), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "OwnershipBoundedInit",
            "  /\\ AcquisitionInit\n",
            "",
        ),
        (
            "OwnershipBoundedNext",
            "  /\\ UNCHANGED acquisitionVars\n",
            "  /\\ UNCHANGED AsyncAllVars\n",
        ),
        (
            "OwnershipBoundedNext",
            "  /\\ OwnershipAsyncNext\n",
            "  /\\ AsyncNext\n",
        ),
        (
            "OwnershipBoundedSpec",
            "[][OwnershipBoundedNext]_OwnershipAllVars",
            "[][OwnershipBoundedNext]_AsyncAllVars",
        ),
    ),
)
def test_ownership_n1_model_closes_inherited_acquisition_state(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    shutil.copyfile(
        module.FORMAL_DIR / "ownership_n1.cfg",
        formal_dir / "ownership_n1.cfg",
    )
    model_path = formal_dir / "SumeragiV2OwnershipInvariantCheck.tla"
    model_source = (
        module.FORMAL_DIR / "SumeragiV2OwnershipInvariantCheck.tla"
    ).read_text(encoding="utf-8")
    model_path.write_text(
        mutate_tla_operator(model_source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._ownership_n1_configuration_errors(formal_dir)
    assert any(
        f"ownership model operator {symbol} must equal only" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "OwnershipControlItemsTyped",
            "  \\A item \\in items:\n"
            "    /\\ AsyncItemTyped(item)\n"
            "    /\\ item.kind \\in AsyncControlKinds\n",
            "  TRUE\n",
        ),
        (
            "OwnershipAsyncCertifiedResponseClaimValues",
            "candidate \\in asyncSentItems",
            "candidate \\in AsyncNetworkItems",
        ),
        (
            "OwnershipHistoricalLockRestartExactCurrentFetchOwner",
            "\\E qc \\in prepareQCs:",
            "\\E candidate \\in AsyncCandidateSet, qc \\in prepareQCs:",
        ),
    ),
)
def test_ownership_n1_structural_helpers_fail_closed(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    shutil.copyfile(
        module.FORMAL_DIR / "ownership_n1.cfg",
        formal_dir / "ownership_n1.cfg",
    )
    model_path = formal_dir / "SumeragiV2OwnershipInvariantCheck.tla"
    model_source = (
        module.FORMAL_DIR / "SumeragiV2OwnershipInvariantCheck.tla"
    ).read_text(encoding="utf-8")
    model_path.write_text(
        mutate_tla_operator(model_source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._ownership_n1_configuration_errors(formal_dir)
    assert any(
        f"ownership structural helper {symbol} must match exact reviewed "
        "body digest" in error
        for error in errors
    ), errors


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


@pytest.mark.parametrize(
    ("file_name", "operator", "expected_error"),
    (
        (
            "SumeragiV2Core.tla",
            "ApplyDecision",
            "ApplyDecision must require the exact current-context Commit "
            "Decision authority once",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "ApplyDecisionReady",
            "ApplyDecisionReady must require the exact current-context Commit "
            "Decision authority once",
        ),
    ),
)
def test_async_source_fidelity_requires_apply_decision_authority(
    tmp_path: Path,
    file_name: str,
    operator: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for canonical_name in (
        "SumeragiV2Core.tla",
        "SumeragiV2AsyncNetwork.tla",
    ):
        shutil.copy2(
            module.FORMAL_DIR / canonical_name,
            formal_dir / canonical_name,
        )

    path = formal_dir / file_name
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(
            source,
            operator,
            "DecisionCertifiedBodyRecoveryAuthority(node, qc)",
            "application \\in decisions",
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("old", "new"),
    (
        (
            "       /\\ ApplyDecision(command.node, qc)",
            "       /\\ command.evidence = qc\n"
            "       /\\ ApplyDecision(command.node, qc)",
        ),
        (
            "ApplyDecision(command.node, qc)",
            "ApplyDecision(command.node, command.evidence)",
        ),
    ),
)
def test_async_source_fidelity_keeps_apply_evidence_as_provenance(
    tmp_path: Path,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    source = (module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla").read_text(
        encoding="utf-8"
    )
    (formal_dir / "SumeragiV2AsyncNetwork.tla").write_text(
        mutate_tla_operator(source, "ExecuteApply", old, new),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "ExecuteApply must resolve application authority from the durable "
        "current Decision and may not overload causal command evidence"
        in error
        for error in errors
    ), errors


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
            "AsyncAllVars ==\n"
            "  <<gst, vars, AsyncSchedulerVars, AsyncRecoveryVars, "
            "AsyncProducerVars,\n"
            "    asyncFixedCorridorDeadlines>>",
            "AsyncAllVars ==\n"
            "  <<gst, vars, AsyncSchedulerVars, AsyncRecoveryVars,\n"
            "    asyncFixedCorridorDeadlines>>",
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


def test_async_source_fidelity_requires_parenthesized_candidate_carriers(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    source = path.read_text(encoding="utf-8")
    assert module._async_source_fidelity_errors(formal_dir) == []
    parenthesized = (
        "    (UNION {SequenceSet(commandQueues[node]): node \\in ValidatorIds})\n"
    )
    assert parenthesized in source
    path.write_text(
        source.replace(
            parenthesized,
            "    UNION {SequenceSet(commandQueues[node]): node \\in ValidatorIds}\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        "CandidateScheduledIn must equal only" in error for error in errors
    ), errors


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
            "  /\\ node \\in Responsive \\cap up\n"
            "  /\\ Crash(node)",
            "  /\\ node \\in Responsive \\cap up",
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

    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    (formal_dir / "SumeragiV2AsyncLivenessProofs.tla").write_text(
        module._async_liveness_source(module.FORMAL_DIR),
        encoding="utf-8",
    )
    return formal_dir


def reviewed_run_inner_fixture(
    tmp_path: Path, module, checker_name: str
) -> Path:
    """Copy the canonical source fixture owned by one run-loop checker."""

    if checker_name != "timeout_vote_episode":
        return local_runner_service_fixture(tmp_path, module)
    return copy_timeout_vote_episode_fixture(tmp_path, module)


def reviewed_run_inner_source_fidelity_errors(
    module, repo_root: Path, formal_dir: Path, checker_name: str
) -> list[str]:
    """Dispatch one run-loop mutation to its sole semantic owner."""

    if checker_name == "timeout_vote_episode":
        return module._timeout_vote_episode_source_fidelity_errors(
            repo_root, formal_dir
        )
    if checker_name == "local_runner":
        return module._local_runner_service_contract_source_fidelity_errors(
            module.load_ledger(), repo_root=repo_root, formal_dir=formal_dir
        )
    if checker_name == "retained_response":
        return module._retained_response_escape_latch_source_fidelity_errors(
            repo_root
        )
    if checker_name == "locked_body":
        return module._locked_body_reproposal_source_fidelity_errors(
            module.FORMAL_DIR, repo_root
        )
    return module._exact_output_production_source_fidelity_errors(repo_root)


def test_exact_serve_runtime_episode_production_contract_is_current(
    tmp_path: Path,
) -> None:
    """The final queue, runner, executor, and runtime episode form one seal."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)

    errors = (
        module._exact_serve_runtime_episode_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert errors == []


def test_leader_wire_physical_ingress_production_contract_is_current(
    tmp_path: Path,
) -> None:
    """Logical replay identity and physical carrier order remain distinct."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)

    errors = (
        module._leader_wire_physical_ingress_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert errors == []


@pytest.mark.parametrize(
    ("relative", "old", "new", "expected_error"),
    (
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "                incumbent.ingress_predecessors = ingress_predecessors;\n",
            "                incumbent.ingress_predecessors.clear();\n",
            "freshly frozen physical prefix",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            ".map(|(source, lane)| (source.clone(), lane.entries.len()))\n",
            ".map(|(source, _lane)| (source.clone(), 0))\n",
            "complete current physical source prefix",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "                if durable_ordinals != active_ordinals {\n",
            "                if false {\n",
            "complete durable and in-memory logical Ingress owner sets",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "            active_leader_wire_carriers.sort_by_key(|(_, ordinal)| *ordinal);\n",
            "            active_leader_wire_carriers\n"
            "                .sort_by_key(|(owner, _)| owner.token.scheduler_ordinal);\n",
            "ordering by physical ordinal",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "                    .remove(&owner.token)\n",
            "                    .get(&owner.token)\n"
            "                    .copied()\n",
            "consume its one exact physical carrier",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "            if !leader_wire_carrier_ordinals.is_empty() {\n",
            "            if false {\n",
            "correspondence must be total before ordering",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "match active_leader_wire_carriers.into_iter().next() {\n",
            "match active_leader_wire_carriers.into_iter().last() {\n",
            "minimum physical carrier",
        ),
        (
            "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs",
            ".filter(|record| record.status == LeaderWireLifecycleStatus::Ingress)\n",
            ".filter(|record| record.status != LeaderWireLifecycleStatus::Terminal)\n",
            "every active logical scheduler owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs",
            ".map(|record| record.token.scheduler_ordinal)\n"
            "            .collect())\n",
            ".map(|record| record.token.scheduler_ordinal)\n"
            "            .take(1)\n"
            "            .collect())\n",
            "every active logical scheduler owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "token.admission_ordinal > restore.last_admission_ordinal()",
            "token.admission_ordinal < restore.last_admission_ordinal()",
            "every restored durable token must remain at or below the restored physical admission high-watermark",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            ".max(restore.last_admission_ordinal());",
            ".max(0);",
            "restart binding must preserve the durable physical admission high-watermark",
        ),
        (
            "crates/iroha_core/src/sumeragi/mod.rs",
            "let admission_ordinal = state\n"
            "        .last_admission_ordinal\n"
            "        .checked_add(1)\n"
            "        .ok_or(FairV2IngressLeaderWireAdmissionError::Exhausted)?;",
            "let admission_ordinal = state\n"
            "        .last_admission_ordinal\n"
            "        .wrapping_add(1);",
            "fresh leader-wire lifecycle admission must use the next physical high-watermark ordinal",
        ),
    ),
)
def test_leader_wire_physical_ingress_rejects_semantic_mutations(
    tmp_path: Path,
    relative: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Every replay-ordering boundary fails the aggregate checker closed."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / relative
    source = path.read_text(encoding="utf-8")
    assert source.count(old) == 1, old
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = (
        module._leader_wire_physical_ingress_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    "name",
    (
        "restored_productive_retry_freezes_the_current_physical_source_prefix",
        "restored_older_logical_owner_cannot_cross_an_earlier_physical_leader_wire",
    ),
)
def test_leader_wire_physical_ingress_regressions_cannot_be_deleted(
    tmp_path: Path,
    name: str,
) -> None:
    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/mod.rs"
    source = path.read_text(encoding="utf-8")
    declaration = f"fn {name}("
    assert source.count(declaration) == 1
    path.write_text(
        source.replace(declaration, f"fn removed_{name}(", 1),
        encoding="utf-8",
    )

    errors = (
        module._leader_wire_physical_ingress_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert any(f"named {name}; found 0" in error for error in errors), errors


@pytest.mark.parametrize(
    ("relative", "old", "new", "expected_error"),
    (
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "enum CertifiedServeRuntimeEpisodeState {\n"
            "    Ready,\n"
            "    Claimed {\n",
            "enum CertifiedServeRuntimeEpisodeState {\n"
            "    Claimed {\n",
            "distinct ready, one-owner claimed, and irreversible complete states",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "self.projection.request_hash == barrier.request_hash",
            "true",
            "episode claims must retain the exact Serve request hash",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "self.carrier_ordinal == Some(barrier.carrier_ordinal)",
            "self.carrier_ordinal.is_some()",
            "episode claims must retain the selected physical carrier occurrence",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "&& self.handed_off.is_some()",
            "&& true",
            "episode claims must retain the live fair-ingress handoff",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "                    if command_ordinal >= reservation.id.0 {\n",
            "                    if command_ordinal > reservation.id.0 {\n",
            "equal or later causal work must not enter",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "                        } if existing == command_ordinal => Some(command_ordinal),\n",
            "                        } if existing <= command_ordinal => Some(command_ordinal),\n",
            "already-selected owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "        if exact_target_active && exact_predecessor_ordinal.is_none() {\n",
            "        if false {\n",
            "later causal, Control, Completion, and priority work must be blocked",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "                        .certified_serve_runtime_predecessor_capacity_available(serve_barrier)\n",
            "                        .certified_serve_runtime_predecessor_capacity_unchecked(serve_barrier)\n",
            "serialized predecessor step must require both an older owner and physical capacity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "                        .finish_certified_serve_runtime_episode_turn(\n",
            "                        .finish_certified_serve_runtime_episode_turn_unchecked(\n",
            "re-publish/recheck the full owner set before settlement",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "self.ingress.oldest_active_lifecycle_ordinal()?",
            "self.ingress.oldest_lifecycle_ordinal()?",
            "complete runtime minimum must include latent Local FIFO reservations",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "        for reservation in &self.dormant_local_fifo_reservations {\n"
            "            if reservation.admission_ordinal == 0\n"
            "                || !self\n"
            "                    .lifecycle_ordinals\n"
            "                    .recognizes_minted(reservation.admission_ordinal)\n"
            "                    .map_err(|_| EnqueueError::FailClosed)?\n"
            "            {\n"
            "                return Err(EnqueueError::FailClosed);\n"
            "            }\n"
            "        }\n"
            "        // Dormant replay reservations are passive capacity claims, not\n",
            "        for reservation in &self.dormant_local_fifo_reservations {\n"
            "            if false && reservation.admission_ordinal == 0\n"
            "                || !self\n"
            "                    .lifecycle_ordinals\n"
            "                    .recognizes_minted(reservation.admission_ordinal)\n"
            "                    .map_err(|_| EnqueueError::FailClosed)?\n"
            "            {\n"
            "                return Err(EnqueueError::FailClosed);\n"
            "            }\n"
            "        }\n"
            "        // Dormant replay reservations are passive capacity claims, not\n",
            "latent Local FIFO reservations must retain exact minted identity but remain passive until a runnable occurrence materializes",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "if self\n"
            "            .dormant_local_fifo_reservations\n"
            "            .iter()\n"
            "            .any(|reservation| reservation.admission_ordinal == lifecycle_ordinal)",
            "if self\n"
            "            .commands\n"
            "            .iter()\n"
            "            .any(|reservation| reservation.admission_ordinal == lifecycle_ordinal)",
            "latent Local FIFO reservations must collide with reused exact-Serve ordinals",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "                let claimed_older_runtime_episode = services\n"
            "                    .claim_certified_serve_runtime_episode(serve_barrier)\n",
            "                let claimed_older_runtime_episode = services\n"
            "                    .claim_certified_serve_runtime_episode_unchecked(serve_barrier)\n",
            "an exact target turn must claim before selecting one completed predecessor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "                    services.drain_exact_serve_runtime_predecessor(\n"
            "                        &mut executor,\n"
            "                        serve_barrier.scheduler_ordinal(),\n"
            "                    )?;\n",
            "                    services.drain_exact_serve_runtime_predecessor(\n"
            "                        &mut executor,\n"
            "                        serve_barrier.lifecycle_ordinal(),\n"
            "                    )?;\n",
            "an exact target turn must claim before selecting one completed predecessor",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "                    if predecessor_witness.is_some()\n"
            "                        && services\n",
            "                    if predecessor_witness.is_none()\n"
            "                        && services\n",
            "serialized predecessor step must require both an older owner and physical capacity",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "                    older_predecessor_remains = predecessor_witness.is_some();\n",
            "                    older_predecessor_remains = predecessor_witness.is_none();\n",
            "every claimed turn must re-publish/recheck the full owner set before settlement",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "            else {\n"
            "                // Exact admission won the queue-locked race after the\n"
            "                // observation above. Restart at the dedicated target turn.\n"
            "                let _ = wake_rx.recv_timeout(IDLE_POLL);\n"
            "                continue;\n"
            "            };\n",
            "            else {\n"
            "                continue;\n"
            "            };\n",
            "queue-locked handoff to an exact target which won the admission race must retain the finite wake bound",
        ),
    ),
)
def test_exact_serve_runtime_episode_rejects_semantic_mutations(
    tmp_path: Path,
    relative: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Every lasso-closing ownership boundary fails closed under mutation."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / relative
    source = path.read_text(encoding="utf-8")
    assert source.count(old) == 1, old
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = (
        module._exact_serve_runtime_episode_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert any(expected_error in error for error in errors), errors


def test_exact_serve_runtime_episode_regression_cannot_be_deleted(
    tmp_path: Path,
) -> None:
    """The full-Control-prefix regression is part of the release source seal."""

    module = load_checker()
    local_runner_service_fixture(tmp_path, module)
    path = tmp_path / "crates/iroha_core/src/sumeragi/v2_worker.rs"
    source = path.read_text(encoding="utf-8")
    name = (
        "exact_serve_claim_waits_out_full_control_prefix_before_older_causal_admission"
    )
    declaration = f"fn {name}("
    assert source.count(declaration) == 1
    path.write_text(
        source.replace(declaration, f"fn removed_{name}(", 1),
        encoding="utf-8",
    )

    errors = (
        module._exact_serve_runtime_episode_production_source_fidelity_errors(
            tmp_path
        )
    )

    assert any(f"named {name}; found 0" in error for error in errors), errors


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
            "     /\\ \\A node \\in AsyncTimedServiceNodes:\n",
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
            "      ENABLED PostGstProductiveStepWith(\n"
            "        AsyncTerminatingLocalWorkDecreaseStep))\n",
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
    (
        "relative_path",
        "item_name",
        "old",
        "new",
        "diagnostic",
        "lifecycle_digest_key",
    ),
    (
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "adopt_effect_ownership",
            "production_adapter_effect_candidate_binding(effect, Some(&retained_statement))?",
            "production_adapter_effect_candidate_binding(effect, None)?",
            "body-terminal adoption must derive its candidate from the retained effective authority",
            "",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "reserve_body_available_with_owner",
            "let candidate_statement = ownership.candidate_semantic_statement();",
            "let candidate_statement = None;",
            "owned BodyAvailable reservation must receive the incumbent effect statement",
            "reserve_body_available_with_owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "unpublished_body_token_rebinds_retries_and_retires_as_one_exact_owner",
            "foreign_subject,",
            "manifest.subject,",
            "unpublished-token regression must reject foreign coordinates without selecting the token",
            "unpublished_body_token_rebinds_retries_and_retires_as_one_exact_owner",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_effects.rs",
            "commit_pending_fetch_retirement",
            "if !retired_completion {",
            "if false && !retired_completion {",
            "pending Fetch retirement must release its token or restored stage-7 parent before local ownership",
            "commit_pending_fetch_retirement",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "retire_restored_producer_continuation",
            "self.persist_restored_body_producer_retirement(*address, record)?;",
            "let _unretired = (address, record);",
            "persistent producer retirement must delegate its exact matched owner",
            "",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "persist_restored_body_producer_retirement",
            "record.identity().address() != address",
            "record.identity().address() == address",
            "persistent producer retirement must own one exact dormant durable volatile-body record",
            "",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "persist_restored_body_producer_retirement",
            "self.restored_dormant_producer_continuations.insert(address);",
            "self.restored_dormant_producer_continuations.remove(&address);",
            "roll all memory back on persistence failure",
            "",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "assert_restored_stage_seven_retirement_does_not_resurrect",
            """if !reserve_completion {
            assert!(
                runtime
                    .retire_restored_body_fetch_parent(&reconstructed_fetch, &fetch_ownership)
                    .expect("persist terminal restored fetch-parent retirement")
            );
            assert_eq!(runtime.remaining_completion_capacity(), capacity_before);
            assert!(
                !runtime""",
            """if !reserve_completion {
            assert!(
                runtime
                    .retire_restored_body_fetch_parent(&reconstructed_fetch, &fetch_ownership)
                    .expect("persist terminal restored fetch-parent retirement")
            );
            assert_eq!(runtime.remaining_completion_capacity(), capacity_before);
            assert!(
                runtime""",
            "must observe both terminal-fetch and reserved-token process/durable/dormant removal cuts",
            "",
        ),
    ),
)
def test_effect_capacity_reconciled_semantics_survive_digest_refresh(
    tmp_path: Path,
    relative_path: str,
    item_name: str,
    old: str,
    new: str,
    diagnostic: str,
    lifecycle_digest_key: str,
) -> None:
    """Extracted ownership contracts remain semantic after any item reseal."""

    module = load_checker()
    repo_root, _formal_dir = copy_effect_capacity_mutation_fixture(tmp_path, module)
    path = repo_root / relative_path
    mutate_rust_item_source(module, path, item_name, old, new)
    if lifecycle_digest_key:
        items = module.rust_items(path.read_text(encoding="utf-8"), item_name)
        assert len(items) == 1, item_name
        module._EFFECT_CAPACITY_LIFECYCLE_RUST_ITEM_SHA256[
            lifecycle_digest_key
        ] = module._rust_item_token_sha256(items[0])

    errors = module._effect_capacity_production_source_fidelity_errors(repo_root)

    assert any(diagnostic in error for error in errors), errors
