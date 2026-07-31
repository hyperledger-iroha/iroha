# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

def test_indexed_chain_spec_cannot_manufacture_generation_budget(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    source = path.read_text(encoding="utf-8")
    insertion = (
        "THEOREM IndexedChainSpecInventsGenerationBudget ==\n"
        "  IndexedChainSpec => IndexedInstallGenerationBudgetPremise\n"
        "BY PTL\n\n"
    )
    path.write_text(
        source.replace(
            "THEOREM IndexedLiveChainSpecProjectsIndexedChainSpec ==\n",
            insertion
            + "THEOREM IndexedLiveChainSpecProjectsIndexedChainSpec ==\n",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)

    assert any(
        "IndexedChainSpecInventsGenerationBudget may not state a finite "
        "install-generation liveness premise" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("relative", "symbol", "old", "new"),
    (
        (
            "SumeragiV2ChainEpochRefinement.tla",
            "GenesisHeightSuccessorHandoffObligation",
            "AsyncLiveChainSpec",
            "AsyncChainSpec",
        ),
        (
            "SumeragiV2ChainLivenessProofs.tla",
            "HeightLivenessObligation",
            "IndexedLiveChainSpec",
            "IndexedChainSpec",
        ),
    ),
)
def test_live_fixed_obligation_statements_are_exact(
    relative: str,
    symbol: str,
    old: str,
    new: str,
) -> None:
    module = load_checker()
    ledger = module.load_ledger()
    target_module = Path(relative).stem
    source = (module.FORMAL_DIR / relative).read_text(encoding="utf-8")
    sources = {
        target_module: mutate_tla_theorem(source, symbol, old, new)
    }

    errors = module._proof_obligation_architecture_errors(
        ledger["obligations"],
        sources,
    )

    assert any(f"{symbol} must state only" in error for error in errors), errors


@pytest.mark.parametrize(
    ("old", "new"),
    (
        (
            "node \\in AsyncCurrentResponsiveVoters",
            "node \\in ValidatorIds",
        ),
        (
            "\\/ HistoricalRecoveryTarget(node)",
            "\\/ FALSE",
        ),
    ),
)
def test_decision_exact_source_owner_generalization_is_exact(
    old: str,
    new: str,
) -> None:
    module = load_checker()
    ledger = module.load_ledger()
    target_module = "SumeragiV2DecisionWitnessPreservationProofs"
    source = (module.FORMAL_DIR / f"{target_module}.tla").read_text(
        encoding="utf-8"
    )
    sources = {
        target_module: mutate_tla_operator(
            source,
            "DecisionExactSourceOwner",
            old,
            new,
        )
    }

    errors = module._proof_obligation_architecture_errors(
        ledger["obligations"],
        sources,
    )

    assert any(
        "DecisionExactSourceOwner must equal only" in error for error in errors
    ), errors


@pytest.mark.parametrize(
    ("target_module", "symbol"),
    (
        (
            "SumeragiV2DecisionWitnessPreservationProofs",
            "DecisionExactRetentionFrame",
        ),
        (
            "SumeragiV2ProgressWitnessFinalClosureProofs",
            "FinalWitnessMonotoneCarrierFrame",
        ),
    ),
)
def test_decision_exact_source_union_frames_cannot_drop_historical_targets(
    target_module: str,
    symbol: str,
) -> None:
    module = load_checker()
    ledger = module.load_ledger()
    source = (module.FORMAL_DIR / f"{target_module}.tla").read_text(
        encoding="utf-8"
    )
    old = (
        "  /\\ (AsyncCurrentResponsiveVoters'\n"
        "        \\cup asyncHistoricalRecoveryTargets')\n"
        "       \\subseteq\n"
        "         (AsyncCurrentResponsiveVoters\n"
        "            \\cup asyncHistoricalRecoveryTargets)"
    )
    new = (
        "  /\\ AsyncCurrentResponsiveVoters'\n"
        "       \\subseteq AsyncCurrentResponsiveVoters"
    )
    sources = {
        target_module: mutate_tla_operator(source, symbol, old, new)
    }

    errors = module._proof_obligation_architecture_errors(
        ledger["obligations"],
        sources,
    )

    assert any(f"{symbol} must equal only" in error for error in errors), errors


def test_open_historical_recovery_decision_preservation_statement_is_exact() -> None:
    module = load_checker()
    ledger = module.load_ledger()
    target_module = "SumeragiV2ProgressWitnessFinalClosureProofs"
    symbol = "OpenHistoricalRecoveryPreservesDecisionExactSource"
    source = (module.FORMAL_DIR / f"{target_module}.tla").read_text(
        encoding="utf-8"
    )
    sources = {
        target_module: mutate_tla_theorem(
            source,
            symbol,
            "    /\\ OpenHistoricalRecovery(node)\n",
            "    /\\ TRUE\n",
        )
    }

    errors = module._proof_obligation_architecture_errors(
        ledger["obligations"],
        sources,
    )

    assert any(f"{symbol} must state only" in error for error in errors), errors


@pytest.mark.parametrize(
    ("target_module", "symbol", "old", "new", "missing_token"),
    (
        (
            "SumeragiV2DecisionWitnessPreservationProofs",
            "DecisionExactRetentionFramePreservesSource",
            "HistoricalRecoveryTarget",
            "RetiredHistoricalOwner",
            "HistoricalRecoveryTarget",
        ),
        (
            "SumeragiV2ProgressWitnessFinalClosureProofs",
            "OpenHistoricalRecoveryPreservesDecisionExactSource",
            "~NodeHasDecision(node)",
            "TRUE",
            "~NodeHasDecision(node)",
        ),
        (
            "SumeragiV2ProgressWitnessFinalClosureProofs",
            "OpenHistoricalRecoveryPreservesFinalProgressWitnessClosure",
            "OpenHistoricalRecoveryPreservesDecisionExactSource",
            "FinalMonotoneCarrierFramePreservesClosure",
            "OpenHistoricalRecoveryPreservesDecisionExactSource",
        ),
        (
            "SumeragiV2ApplicationCompletionProofs",
            "ExactDecisionSourceProjectsPostGstServiceStage",
            "DecisionExactSourceOwner,",
            "GeneralizedSourceOwnerRemoved,",
            "DecisionExactSourceOwner",
        ),
    ),
)
def test_historical_decision_source_owner_dependencies_are_connected(
    target_module: str,
    symbol: str,
    old: str,
    new: str,
    missing_token: str,
) -> None:
    module = load_checker()
    ledger = module.load_ledger()
    source = (module.FORMAL_DIR / f"{target_module}.tla").read_text(
        encoding="utf-8"
    )
    sources = {
        target_module: mutate_tla_theorem(source, symbol, old, new)
    }

    errors = module._proof_obligation_architecture_errors(
        ledger["obligations"],
        sources,
    )

    assert any(
        f"{symbol} must retain reviewed proof dependencies" in error
        and missing_token in error
        for error in errors
    ), errors


def test_open_historical_recovery_cannot_fold_into_monotone_frame_path() -> None:
    module = load_checker()
    ledger = module.load_ledger()
    target_module = "SumeragiV2ProgressWitnessFinalClosureProofs"
    symbol = "AsyncNonRunnerPreservesFinalProgressWitnessClosure"
    source = (module.FORMAL_DIR / f"{target_module}.tla").read_text(
        encoding="utf-8"
    )
    sources = {
        target_module: mutate_tla_theorem(
            source,
            symbol,
            "OpenHistoricalRecoveryPreservesFinalProgressWitnessClosure",
            "FinalMonotoneCarrierFramePreservesClosure",
        )
    }

    errors = module._proof_obligation_architecture_errors(
        ledger["obligations"],
        sources,
    )

    assert any(
        "must keep OpenHistoricalRecovery on its dedicated preservation branch"
        in error
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
            "theorem",
            "IndexedFreshReceiptActionHasProductExtension",
            "/\\ ENABLED IndexedFreshReceiptAsyncAction(initialContext))",
            "/\\ TRUE)",
            "IndexedFreshReceiptActionHasProductExtension must state only",
        ),
        (
            "operator",
            "IndexedTotalReceiptProjection",
            "  /\\ IndexedApplicationReceiptProjection",
            "  /\\ TRUE",
            "IndexedTotalReceiptProjection must equal only",
        ),
        (
            "operator",
            "NewIndexedDecisionReceipt",
            "  /\\ decision \\notin IndexedDecisions(initialContext)",
            "  /\\ FALSE",
            "NewIndexedDecisionReceipt must equal only",
        ),
        (
            "operator",
            "IndexedReceiptClassification",
            "  \\/ \\E decision \\in Chain!DecisionEvidenceSet:\n"
            "       IndexedDecisionReceiptHandoff(initialContext, decision)\n",
            "",
            "IndexedReceiptClassification must equal only",
        ),
        (
            "operator",
            "IndexedFreshReceiptAsyncAction",
            "  /\\ \\/ \\E decision \\in Chain!DecisionEvidenceSet:\n"
            "            NewIndexedDecisionReceipt(initialContext, decision)\n",
            "  /\\ \\/ FALSE\n",
            "IndexedFreshReceiptAsyncAction must equal only",
        ),
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
            "ExactDurableParentApplicationHasAdmissibleSuccessorContext",
            "    /\\ Chain!ChainEpochInvariant\n",
            "    /\\ TRUE\n",
        ),
        (
            "ExactDurableParentApplicationHasAdmissibleSuccessorContext",
            "             Chain!CertifiedPrefixBacked",
            "             DisconnectedPrefixPredicate",
        ),
        (
            "SuccessorActivationProgressPreservesProtocolInvariant",
            "    Chain!ChainEpochInvariant\n",
            "    TRUE\n",
        ),
        (
            "SuccessorActivationProgressPreservesProtocolInvariant",
            "BY ExactDurableParentApplicationHasAdmissibleSuccessorContext,",
            "BY DisconnectedAdmissibleSuccessorContext,",
        ),
        (
            "IndexedActionPreservesSuccessorActivationProtocolInvariant",
            "         SuccessorActivationProgressPreservesProtocolInvariant\n"
            "         DEF IndexedCompositionInvariant",
            "         DisconnectedProgressPreservation\n"
            "         DEF IndexedCompositionInvariant",
        ),
        (
            "IndexedActionPreservesSuccessorActivationProtocolInvariant",
            "         DEF IndexedCompositionInvariant",
            "         DEF SuccessorActivationProtocolInvariant",
        ),
        (
            "SuccessorActivationFailureFreeProgressExitsCurrentRank",
            "    /\\ Chain!ChainEpochInvariant\n",
            "    /\\ TRUE\n",
        ),
        (
            "SuccessorActivationFailureFreeProgressExitsCurrentRank",
            "   SuccessorActivationProgressPreservesProtocolInvariant,",
            "   DisconnectedProgressPreservation,",
        ),
        (
            "FailureFreeSuccessorActivationRankLeadsToExit",
            "    <2>8. /\\ Chain!ChainEpochInvariant\n",
            "    <2>8. /\\ TRUE\n",
        ),
        (
            "FailureFreeSuccessorActivationRankLeadsToExit",
            "         DEF IndexedCompositionInvariant",
            "         DEF SuccessorActivationProtocolInvariant",
        ),
    ),
)
def test_successor_activation_admissible_context_premises_fail_closed(
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
    ("symbol", "old", "new"),
    (
        (
            "SuccessorActivationPendingRankTierClassification",
            "SuccessorActivationRank(parentContext, node) \\in 12..21",
            "SuccessorActivationRank(parentContext, node) \\in 11..21",
        ),
        (
            "RecoveredAuthenticationDescendsAbsentTier",
            "SuccessorActivationRank(parentContext, node)' = 8",
            "SuccessorActivationRank(parentContext, node)' = 9",
        ),
        (
            "IndexedStepRetainsExactDurableParentWitnessOrExits",
            "IndexedStepPreservesSuccessorActivationProtocolInvariant,",
            "DisconnectedProtocolPreservation,",
        ),
        (
            "FailureFreeBracketExcludesSuccessorResetActions",
            "    /\\ SuccessorActivationFailureAbsent(parentContext, node)'\n",
            "    /\\ TRUE\n",
        ),
        (
            "IndexedFailureFreeStepDoesNotRaiseSuccessorActivationRank",
            "OtherOwnerProgressFramesPendingSuccessorRankOrSupersedes,",
            "DisconnectedOtherOwnerFrame,",
        ),
        (
            "EventualFailureFreeSuffixLiftsSuccessorConvergence",
            "SuccessorActivationPendingReachesFailureFreeSuffixOrOutcome",
            "DisconnectedFailureFreeSuffixBridge",
        ),
    ),
)
def test_successor_activation_split_closure_mutations_fail_closed(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
) -> None:
    """Seal rank tiers, exact witnesses, failure brackets, frames, and suffixes."""

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


def test_reviewed_checker_contract_dicts_have_no_duplicate_literal_keys() -> None:
    reviewed_names = {
        "REQUIRED_PROOF_OBLIGATION_INVENTORY",
        "FIXED_PROOF_OBLIGATION_TARGETS",
        "PROOF_STATUS_DEPENDENCIES",
    }
    reviewed: dict[str, ast.Dict] = {}
    for path in checker_source_paths():
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
        for statement in tree.body:
            if not isinstance(statement, ast.Assign) or len(statement.targets) != 1:
                continue
            target = statement.targets[0]
            if (
                isinstance(target, ast.Name)
                and target.id in reviewed_names
                and isinstance(statement.value, ast.Dict)
            ):
                assert target.id not in reviewed
                reviewed[target.id] = statement.value

    assert set(reviewed) == reviewed_names
    for name, dictionary in reviewed.items():
        keys = [ast.literal_eval(key) for key in dictionary.keys if key is not None]
        duplicates = sorted({key for key in keys if keys.count(key) > 1})
        assert duplicates == [], f"{name} has duplicate literal keys: {duplicates}"


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
    assert (
        "all exhaustive searches, deterministic simulations, the recovery "
        "witness, the layout-only in-flight carrier corpus, and the pinned "
        "multilane Apalache gate" in runner
    )
    assert (
        "all requested exhaustive searches, deterministic simulations, and "
        "recovery witnesses" in runner
    )
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
    assert "deferred_busy_priority_old_status -eq 13" in runner
    assert "deferred_handoff_rebusy_status -eq 13" in runner
    assert "deferred_handoff_rebusy_bug.cfg" in runner
    assert "deferred_handoff_exact.cfg" in runner
    assert "SumeragiV2DeferredHandoffMutation.tla" in runner
    assert "handoff-free deferred retry did not fail with TLC status 13" in runner
    assert "old strict deferred cursor mutation missed expected marker" in runner
    assert "old Busy/deferred priority mutation missed expected marker" in runner
    assert "attemptParity = TRUE" in runner
    assert "deferred_busy_priority_bug.cfg" in runner
    assert "deferred_busy_fence.cfg" in runner
    assert "SumeragiV2DeferredBusyFenceMutation.tla" in runner
    assert "6 distinct states" in runner
    assert "3 distinct states" in runner
    assert "depth of the complete state graph search is 3" in runner
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

    formal_dir = ROOT_DIR / "formal" / "sumeragi_v2"
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
    busy_fence_mutation = (
        formal_dir / "SumeragiV2DeferredBusyFenceMutation.tla"
    ).read_text(encoding="utf-8")
    assert "BusyDeferredRetry" in busy_fence_mutation
    assert "ServiceOrdinaryCompletion" in busy_fence_mutation
    assert "DrainDeferredProgress" in busy_fence_mutation
    assert "attemptParity' = ~attemptParity" in busy_fence_mutation
    assert (formal_dir / "deferred_busy_priority_bug.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION RetryPrioritySpec\n")
    assert (formal_dir / "deferred_busy_fence.cfg").read_text(
        encoding="utf-8"
    ).startswith("SPECIFICATION FencedSpec\n")
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
    assert "616705 states generated, 62464 distinct states found" in progress_runner
    assert "depth of the complete state graph search is 37" in progress_runner

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


def test_global_blocker_cell_mutation_fidelity_rejects_same_rank_swap_weakening(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "docs" / "formal" / "sumeragi_v2"
    formal_dir.mkdir(parents=True)
    for filename in (
        "SumeragiV2AdequateLeaderGlobalBlockerCellMutation.tla",
        "adequate_leader_global_blocker_same_rank_swap_bug.cfg",
        "adequate_leader_global_blocker_exact_cell.cfg",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, formal_dir / filename)
    runner_dir = tmp_path / "scripts" / "formal"
    runner_dir.mkdir(parents=True)
    runner_path = runner_dir / "run_sumeragi_v2_service_rank_mutation.sh"
    shutil.copy2(
        ROOT_DIR / "scripts" / "formal" / runner_path.name,
        runner_path,
    )

    check = (
        module
        ._adequate_leader_global_blocker_cell_mutation_source_fidelity_errors
    )
    checker_source = (
        ROOT_DIR
        / "scripts"
        / "formal"
        / "check_sumeragi_v2_proof_ledger.py"
    ).read_text(encoding="utf-8")
    assert (
        checker_source.count(
            "_adequate_leader_global_blocker_cell_mutation_source_fidelity_errors("
        )
        == 2
    )
    assert check(formal_dir, tmp_path) == []

    mutation_path = (
        formal_dir / "SumeragiV2AdequateLeaderGlobalBlockerCellMutation.tla"
    )
    source = mutation_path.read_text(encoding="utf-8")
    exact_selection = (
        "SelectFrozenOriginal ==\n"
        "  /\\ originalOwned\n"
        '  /\\ selectedCell = "Unselected"\n'
        '  /\\ selectedCell\' = "Original"\n'
        "  /\\ UNCHANGED <<originalOwned, replacementGeneration>>"
    )
    assert source.count(exact_selection) == 1
    mutation_path.write_text(
        source.replace(
            exact_selection,
            exact_selection.replace(
                'selectedCell\' = "Original"',
                'selectedCell\' = "Replacement"',
            ),
            1,
        ),
        encoding="utf-8",
    )
    errors = check(formal_dir, tmp_path)
    assert any(
        "SelectFrozenOriginal must equal" in error for error in errors
    ), errors

    mutation_path.write_text(source, encoding="utf-8")
    vars_tuple = (
        "vars == "
        "<<originalOwned, replacementGeneration, selectedCell>>"
    )
    assert source.count(vars_tuple) == 1
    mutation_path.write_text(
        source.replace(
            vars_tuple,
            "vars == "
            "<<originalOwned, selectedCell, replacementGeneration>>",
            1,
        ),
        encoding="utf-8",
    )
    errors = check(formal_dir, tmp_path)
    assert any("vars must equal" in error for error in errors), errors

    mutation_path.write_text(source, encoding="utf-8")
    bug_config = (
        formal_dir
        / "adequate_leader_global_blocker_same_rank_swap_bug.cfg"
    )
    config_source = bug_config.read_text(encoding="utf-8")
    bug_config.write_text(
        config_source.replace(
            "PROPERTY OriginalCellEventuallyReleased",
            "PROPERTY TRUE",
            1,
        ),
        encoding="utf-8",
    )
    errors = check(formal_dir, tmp_path)
    assert any(
        "same_rank_swap_bug.cfg: global-blocker mutation config" in error
        for error in errors
    ), errors

    bug_config.write_text(config_source, encoding="utf-8")
    runner_source = runner_path.read_text(encoding="utf-8")
    runner_path.write_text(
        runner_source.replace(
            "[[ $global_blocker_same_rank_swap_status -eq 13 ]]",
            "[[ $global_blocker_same_rank_swap_status -eq 0 ]]",
            1,
        ),
        encoding="utf-8",
    )
    errors = check(formal_dir, tmp_path)
    assert any(
        "global-blocker cell mutation runner omits" in error
        for error in errors
    ), errors

    runner_path.write_text(runner_source, encoding="utf-8")
    active_config_line = (
        "    -config "
        "adequate_leader_global_blocker_same_rank_swap_bug.cfg \\\n"
    )
    assert runner_source.count(active_config_line) == 1
    runner_path.write_text(
        runner_source.replace(
            active_config_line,
            f"#{active_config_line}",
            1,
        ),
        encoding="utf-8",
    )
    errors = check(formal_dir, tmp_path)
    assert any(
        "global-blocker red TLC execution block" in error
        for error in errors
    ), errors


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
