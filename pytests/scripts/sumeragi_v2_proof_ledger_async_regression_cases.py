@pytest.mark.parametrize(
    ("symbol", "old", "new", "expected_error"),
    (
        (
            "AsyncAllVars",
            "<<gst, vars, AsyncSchedulerVars, AsyncRecoveryVars, AsyncProducerVars, asyncFixedCorridorDeadlines, asyncServeProducerEpisodeDue>>",
            "<<gst, vars, AsyncSchedulerVars, AsyncRecoveryVars, asyncFixedCorridorDeadlines, asyncServeProducerEpisodeDue>>",
            "AsyncAllVars must equal only",
        ),
        (
            "AsyncAllVars",
            "<<gst, vars, AsyncSchedulerVars, AsyncRecoveryVars, AsyncProducerVars, asyncFixedCorridorDeadlines, asyncServeProducerEpisodeDue>>",
            "<<gst, vars, AsyncRecoveryVars, AsyncSchedulerVars, AsyncProducerVars, asyncFixedCorridorDeadlines, asyncServeProducerEpisodeDue>>",
            "AsyncAllVars must equal only",
        ),
        (
            "AsyncAllVars",
            "<<gst, vars, AsyncSchedulerVars, AsyncRecoveryVars, AsyncProducerVars, asyncFixedCorridorDeadlines, asyncServeProducerEpisodeDue>>",
            "<<gst, vars, AsyncSchedulerVars, AsyncRecoveryVars, AsyncProducerVars, asyncFixedCorridorDeadlines>>",
            "AsyncAllVars must equal only",
        ),
        (
            "AsyncAllVars",
            "<<gst, vars, AsyncSchedulerVars, AsyncRecoveryVars, AsyncProducerVars, asyncFixedCorridorDeadlines, asyncServeProducerEpisodeDue>>",
            "<<gst, coreState, schedulerState, recoveryPhase, recoveryQueue, producerKnown>>",
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


@pytest.mark.parametrize(
    "proof_dependency",
    (
        "TLAPS",
        "FiniteSetTheorems",
        "NaturalsInduction",
        "WellFoundedInduction",
        "SequenceTheorems",
        "SumeragiV2QuorumProofs",
    ),
)
def test_async_source_fidelity_requires_tlaps_aware_module_header(
    tmp_path: Path,
    proof_dependency: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    checker = module._async_source_fidelity_errors
    baseline_errors = checker(formal_dir)
    assert not any(
        "exact TLAPS-aware module header" in error
        for error in baseline_errors
    ), baseline_errors

    exact_header = (
        "EXTENDS SumeragiV2Inductive, Sequences, FiniteSets, Naturals, "
        "Functions, TLAPS, FiniteSetTheorems, NaturalsInduction, "
        "WellFoundedInduction, SequenceTheorems, SumeragiV2QuorumProofs"
    )
    assert source.count(exact_header) == 1
    dependency_token = f", {proof_dependency}"
    assert exact_header.count(dependency_token) == 1
    path.write_text(
        source.replace(
            exact_header,
            exact_header.replace(dependency_token, "", 1),
            1,
        ),
        encoding="utf-8",
    )

    errors = checker(formal_dir)

    assert any(
        "exact TLAPS-aware module header" in error for error in errors
    ), errors


@pytest.mark.parametrize(
    "consumer",
    (
        "AsyncCandidateLifecycleCapacityDerivesFromReviewedOwners",
        "AsyncCandidateServiceRecordCapacityMatchesConfiguredGeometry",
    ),
)
def test_async_source_fidelity_requires_capacity_configuration_dependency(
    tmp_path: Path,
    consumer: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    checker = module._async_source_fidelity_errors
    assert checker(formal_dir) == []

    source = path.read_text(encoding="utf-8")
    declaration = f"THEOREM {consumer} ==\n  AsyncConfiguration"
    assert source.count(declaration) == 1
    path.write_text(
        source.replace(
            declaration,
            f"THEOREM {consumer} ==\n  TRUE",
            1,
        ),
        encoding="utf-8",
    )

    errors = checker(formal_dir)

    assert any(
        consumer in error
        and "must retain its reviewed dependency on AsyncConfiguration"
        in error
        for error in errors
    ), errors


def test_async_source_fidelity_requires_configuration_before_capacity_theorems(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    checker = module._async_source_fidelity_errors
    assert checker(formal_dir) == []

    source = path.read_text(encoding="utf-8")
    provider_start = source.index("AsyncConfiguration ==")
    first_consumer_start = source.index(
        "THEOREM AsyncCandidateLifecycleCapacityDerivesFromReviewedOwners =="
    )
    next_operator_start = source.index("AsyncBodyEnvelope(")
    provider_block = source[provider_start:first_consumer_start]
    source_without_provider = (
        source[:provider_start] + source[first_consumer_start:]
    )
    insertion = source_without_provider.index("AsyncBodyEnvelope(")
    path.write_text(
        source_without_provider[:insertion]
        + provider_block
        + source_without_provider[insertion:],
        encoding="utf-8",
    )
    assert next_operator_start > first_consumer_start

    errors = checker(formal_dir)

    assert any(
        "SANY requires provider AsyncConfiguration to be declared before "
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("provider", "consumer", "arity"),
    (
        (
            "AsyncControlServiceIdentityMatches",
            "AsyncControlServiceIdentityServicedOrAdvancedIn",
            2,
        ),
        ("CommandMatches", "AsyncPersistDecisionCommandThisStep", 4),
        (
            "AsyncProposedTimeoutCausalOrigin",
            "AsyncEffectiveTimeoutLifecycleOrigin",
            1,
        ),
        (
            "AsyncTimeoutLifecycleOwned",
            "AsyncControlServiceStateTypeInvariant",
            1,
        ),
        (
            "AsyncRetransmitLifecycleOwned",
            "AsyncControlServiceStateTypeInvariant",
            1,
        ),
        ("HistoricalLockedCommitItem", "DeliveryClass", 1),
        ("DeliveryKind", "AsyncDeliveryCandidateCausalOriginAt", 1),
        ("DeliveryKind", "DeliveryCandidate", 1),
        ("DeliveryClass", "DeliveryCandidate", 1),
        ("DeliverySubject", "AsyncDeliveryCandidateCausalOriginAt", 1),
        ("DeliverySubject", "DeliveryCandidate", 1),
        ("DeliveryView", "AsyncDeliveryCandidateCausalOriginAt", 1),
        ("DeliveryView", "DeliveryCandidate", 1),
        ("DeliveryHeight", "AsyncDeliveryCandidateCausalOriginAt", 1),
        ("DeliveryHeight", "DeliveryCandidate", 1),
        (
            "AsyncDeliveryCandidateCausalOriginAt",
            "DeliveryCandidate",
            2,
        ),
        (
            "DeliveryCandidate",
            "AsyncOrdinaryIngressCarrierEvidence",
            1,
        ),
        (
            "DeliveryCandidate",
            "AsyncControlServiceStateTypeInvariant",
            1,
        ),
        (
            "AsyncFixedCorridorDeadlineTransition",
            "AsyncCoreOuterFrame",
            0,
        ),
        ("AsyncProducerProjectionStep", "AsyncCoreOuterFrame", 0),
        ("AsyncServeProducerEpisodeTransition", "AsyncCoreOuterFrame", 0),
        ("AsyncCoreOuterFrame", "AsyncNonCrashOuterFrame", 0),
        ("AsyncCoreOuterFrame", "AsyncRecoveryOuterFrame", 0),
        ("AsyncNonCrashOuterFrame", "AsyncNonRunnerOuterFrame", 0),
        (
            "AsyncCandidateLifecycleRecordsForIn",
            "AsyncUnmaterializedTimeoutLifecycleReservationIn",
            3,
        ),
        (
            "AsyncCandidateLifecycleRecordsForNodeIn",
            "AsyncCandidateLifecycleClockRecordBucketIn",
            2,
        ),
        (
            "AsyncCandidateLifecycleRecordsForNodeIn",
            "AsyncCandidateLifecycleSlotInjectionInvariantIn",
            2,
        ),
        (
            "AsyncCandidateLifecycleClockRecordBucketIn",
            "AsyncCandidateLifecycleSlotInjectionInvariantIn",
            2,
        ),
        (
            "AsyncCandidateLifecycleServiceRecordCoversIn",
            "AsyncCandidateLifecycleSlotInjectionInvariantIn",
            2,
        ),
        (
            "AsyncUnmaterializedTimeoutLifecycleReservationIn",
            "AsyncUnmaterializedTimeoutLifecycleReservationNodesIn",
            2,
        ),
        (
            "AsyncUnmaterializedTimeoutLifecycleReservationIn",
            "AsyncCandidateLifecycleSlotInjectionInvariantIn",
            2,
        ),
        (
            "AsyncCandidateLifecycleRecordOwnerToken",
            "AsyncCandidateLifecycleReviewedOwnerTokensIn",
            1,
        ),
        (
            "AsyncCandidateLifecycleClockOwnerToken",
            "AsyncCandidateLifecycleReviewedOwnerTokensIn",
            2,
        ),
        (
            "AsyncUnmaterializedTimeoutLifecycleReservationNodesIn",
            "AsyncCandidateLifecycleReviewedOwnerTokensIn",
            1,
        ),
        (
            "AsyncCandidateLifecycleReviewedOwnerTokensIn",
            "AsyncCandidateLifecycleSlotProjectionIn",
            1,
        ),
        (
            "AsyncCandidateLifecycleReviewedOwnerTokensIn",
            "AsyncCandidateLifecycleSlotInjectionInvariantIn",
            1,
        ),
        (
            "AsyncCandidateLifecycleSlotAddresses",
            "AsyncCandidateLifecycleSlotInjectionInvariantIn",
            0,
        ),
        (
            "AsyncCandidateLifecycleSlotProjectionIn",
            "AsyncCandidateLifecycleSlotInjectionInvariantIn",
            1,
        ),
        (
            "AsyncCandidateLifecycleSlotInjectionInvariantIn",
            "AsyncCandidateLifecycleReviewedCapacityInvariantIn",
            1,
        ),
        (
            "AsyncCandidateLifecycleReviewedCapacityInvariantIn",
            "AsyncControlServiceStateTypeInvariant",
            1,
        ),
        (
            "AsyncServeIngressLifecycleOwnerIdentities",
            "AsyncRecoveryExecutionInvariant",
            1,
        ),
        (
            "SequenceHasUniqueValues",
            "AsyncRecoveryExecutionInvariant",
            1,
        ),
        (
            "ResponsiveReplayScheduledCandidates",
            "AsyncRecoveryExecutionInvariant",
            1,
        ),
        (
            "HistoricalLockRestartAuthoritySource",
            "HistoricalLockRestartAuthoritySourceRetentionInvariant",
            1,
        ),
        ("AsyncSchedulerTypeInvariant", "AsyncStrongTypeInvariant", 0),
        ("AsyncProducerTypeInvariant", "AsyncTypeInvariant", 0),
        ("AsyncProducerTypeInvariant", "AsyncStrongTypeInvariant", 0),
        (
            "AsyncServeProducerEpisodeTypeInvariant",
            "AsyncStrongTypeInvariant",
            0,
        ),
        (
            "AsyncServeProducerEpisodeOwnershipInvariant",
            "AsyncStrongTypeInvariant",
            0,
        ),
        ("AsyncServiceActivationPairInvariant", "AsyncStrongTypeInvariant", 0),
        ("AsyncControlServiceStateTypeInvariant", "AsyncStrongTypeInvariant", 0),
        (
            "AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant",
            "AsyncStrongTypeInvariant",
            0,
        ),
        (
            "AsyncCandidateLifecycleSchedulerCoverageInvariant",
            "AsyncStrongTypeInvariant",
            0,
        ),
        (
            "AsyncCertifiedResponseClaimIngressOwnershipInvariant",
            "AsyncStrongTypeInvariant",
            0,
        ),
        (
            "AsyncLeaderWireIngressCarrierOwnershipInvariant",
            "AsyncStrongTypeInvariant",
            0,
        ),
        (
            "AsyncOrdinaryIngressCarrierOwnershipInvariant",
            "AsyncStrongTypeInvariant",
            0,
        ),
        ("AsyncRecoveryTypeInvariant", "AsyncStrongTypeInvariant", 0),
        ("AsyncRestartAuthorityInvariant", "AsyncStrongTypeInvariant", 0),
        ("AsyncRecoveryExecutionInvariant", "AsyncStrongTypeInvariant", 0),
        (
            "AsyncHistoricalLockRestartAuthorityTypeInvariant",
            "AsyncStrongTypeInvariant",
            0,
        ),
        (
            "HistoricalLockRestartAuthoritySourceRetentionInvariant",
            "AsyncStrongTypeInvariant",
            0,
        ),
        ("AsyncGstRecoveryPhaseInvariant", "AsyncStrongTypeInvariant", 0),
        ("AsyncSerializedBusyKernelInvariant", "AsyncStrongTypeInvariant", 0),
    ),
)
def test_async_source_fidelity_pins_sany_provider_dependency_and_order(
    tmp_path: Path,
    provider: str,
    consumer: str,
    arity: int,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    checker = module._async_source_fidelity_errors
    source = path.read_text(encoding="utf-8")
    provider_span = module._top_level_declaration_span(
        source, provider, kind="operator"
    )
    consumer_span = module._top_level_declaration_span(
        source, consumer, kind="operator"
    )
    assert provider_span is not None
    assert consumer_span is not None
    provider_start, provider_end = provider_span
    consumer_start, consumer_end = consumer_span
    assert provider_start < provider_end <= consumer_start < consumer_end

    provider_block = source[provider_start:provider_end]
    without_provider = source[:provider_start] + source[provider_end:]
    shifted_consumer_end = consumer_end - len(provider_block)
    path.write_text(
        without_provider[:shifted_consumer_end]
        + provider_block
        + without_provider[shifted_consumer_end:],
        encoding="utf-8",
    )
    order_errors = checker(formal_dir)
    assert any(
        f"SANY requires provider {provider} to be declared before consumer "
        f"{consumer}" in error
        for error in order_errors
    ), order_errors

    consumer_block = source[consumer_start:consumer_end]
    assert provider in consumer_block
    mutated_consumer = consumer_block.replace(
        provider,
        "MutationSemanticProvider",
    )
    arguments = ", ".join(f"arg{index}" for index in range(arity))
    mutation_signature = (
        f"MutationSemanticProvider({arguments})"
        if arguments
        else "MutationSemanticProvider"
    )
    mutation_provider = f"{mutation_signature} == TRUE\n\n"
    path.write_text(
        source[:consumer_start]
        + mutation_provider
        + mutated_consumer
        + source[consumer_end:],
        encoding="utf-8",
    )
    dependency_errors = checker(formal_dir)
    assert any(
        consumer in error
        and f"must retain its reviewed dependency on {provider}" in error
        for error in dependency_errors
    ), dependency_errors


@pytest.mark.parametrize(
    "conjunct",
    (
        "AsyncProducerTypeInvariant",
        "AsyncServeProducerEpisodeTypeInvariant",
        "AsyncServeProducerEpisodeOwnershipInvariant",
        "AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant",
    ),
)
def test_async_strong_type_invariant_retains_producer_episode_conjuncts(
    tmp_path: Path,
    conjunct: str,
) -> None:
    """Each new debt/boundary conjunct survives source-seal replacement."""

    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(
            source,
            "AsyncStrongTypeInvariant",
            f"  /\\ {conjunct}\n",
            "",
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        "AsyncStrongTypeInvariant must equal only its exact reviewed base "
        "asynchronous invariant body" in error
        and conjunct in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "AsyncInitEstablishesServeProducerEpisodeInvariants",
            "=> /\\ AsyncServeProducerEpisodeTypeInvariant",
            "=> /\\ TRUE",
        ),
        (
            "AsyncInitEstablishesTimeoutRecoveryCurrentBoundary",
            "=> AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant",
            "=> TRUE",
        ),
        (
            "AsyncNextPreservesServeProducerEpisodeInvariants",
            "=> /\\ AsyncServeProducerEpisodeTypeInvariant'",
            "=> /\\ TRUE",
        ),
        (
            "AsyncNextPreservesTimeoutRecoveryCurrentBoundaryInvariant",
            "PROVE AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant'",
            "PROVE TRUE",
        ),
    ),
)
def test_strong_type_producer_timeout_bridge_statements_fail_closed(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
) -> None:
    """Producer-debt and timeout-boundary bridge conclusions stay exact."""

    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)
    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)

    assert any(f"{symbol} must state only" in error for error in errors), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new", "expected_error"),
    (
        (
            "AsyncInitEstablishesStrongTypeInvariant",
            "AsyncInitEstablishesServeProducerEpisodeInvariants",
            "TRUE",
            "must use the exact finite Serve-producer episode init bridge",
        ),
        (
            "AsyncInitEstablishesStrongTypeInvariant",
            "AsyncInitEstablishesTimeoutRecoveryCurrentBoundary",
            "TRUE",
            "must use the exact timeout-recovery boundary init bridge",
        ),
        (
            "AsyncInitEstablishesStrongTypeInvariant",
            "<2>3d, <2>3e, <2>3p, <2>3t, <2>4,",
            "<2>3d, <2>3e, <2>4,",
            "must retain the exact candidate/Serve/producer/timeout/leader/"
            "ordinary scheduler-coverage QED dependency set",
        ),
        (
            "AsyncAllVarsStutterPreservesStrongTypeInvariant",
            "AsyncServeProducerEpisodeOwnershipInvariant,\n"
            "             AsyncServeIngressLifecycleOwnerIdentities",
            "TRUE,\n             AsyncServeIngressLifecycleOwnerIdentities",
            "must retain the exact Serve-producer episode stutter bridge",
        ),
        (
            "AsyncAllVarsStutterPreservesStrongTypeInvariant",
            "AsyncNodeHasDecisionIn\n",
            "TRUE\n",
            "must retain the exact timeout-recovery boundary stutter bridge",
        ),
        (
            "AsyncAllVarsStutterPreservesStrongTypeInvariant",
            "<2>7, <2>8, <2>8p, <2>8t",
            "<2>7, <2>8",
            "must retain producer-episode and timeout-boundary prime steps "
            "as exact QED dependencies",
        ),
        (
            "AsyncNextPreservesStrongTypeInvariant",
            "AsyncNextPreservesServeProducerEpisodeInvariants",
            "TRUE",
            "must retain the exact finite Serve-producer episode prime step",
        ),
        (
            "AsyncNextPreservesStrongTypeInvariant",
            "AsyncNextPreservesTimeoutRecoveryCurrentBoundaryInvariant",
            "TRUE",
            "must retain the exact timeout-recovery current-boundary prime step",
        ),
        (
            "AsyncNextPreservesStrongTypeInvariant",
            "<2>4b, <2>4c, <2>4d, <2>4e, <2>5",
            "<2>4b, <2>4c, <2>5",
            "candidate-lifecycle, producer-episode, and timeout-boundary "
            "prime step an exact QED dependency",
        ),
    ),
)
def test_strong_type_producer_timeout_bridge_dependencies_fail_closed(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Every new invariant bridge remains a named proof/QED dependency."""

    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)
    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)

    assert any(
        symbol in error and expected_error in error for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "AsyncServeProducerEpisodeTransition",
            "       IF AsyncServeProducerEpisodeRestartStep(node)\n"
            "            \\/ AsyncServeProducerEpisodeReceiverCloseStep(node)\n"
            "       THEN FALSE\n"
            "       ELSE IF AsyncServeProducerEpisodeFinalRetirementStep(node)\n"
            "            THEN TRUE",
            "       IF AsyncServeProducerEpisodeFinalRetirementStep(node)\n"
            "       THEN TRUE\n"
            "       ELSE IF AsyncServeProducerEpisodeRestartStep(node)\n"
            "            \\/ AsyncServeProducerEpisodeReceiverCloseStep(node)\n"
            "            THEN FALSE",
        ),
        (
            "AsyncServeProducerEpisodeTransition",
            "               ELSE asyncServeProducerEpisodeDue[node]",
            "               ELSE FALSE",
        ),
        (
            "AsyncServeProducerEpisodeActiveThisStep",
            "   \\/ RunHistoricalServer(node)",
            "   \\/ RunHistoricalServer(node)\n"
            "   \\/ (\\E source \\in AsyncIngressSources:\n"
            "         CoalesceHiddenPacket(node, source))",
        ),
        (
            "AsyncServeProducerEpisodeOwnershipInvariant",
            "       /\\ AsyncServeOffQueueReservations(node) = {}",
            "       /\\ TRUE",
        ),
    ),
)
def test_serve_producer_episode_transition_mutations_are_rejected(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
) -> None:
    """Restart/close precedence, retry stutter, and ownership stay exact."""

    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        symbol in error
        and "must retain the exact one-shot producer-debt ownership, "
        "classifier, and transition semantics" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    "symbol",
    (
        "ReserveExactServeCapacityVia",
        "AdvanceExactServeCapacityVia",
        "ExactServeTransportAdmissionCanAdvanceVia",
    ),
)
def test_fresh_serve_admission_cannot_bypass_producer_episode_debt(
    tmp_path: Path,
    symbol: str,
) -> None:
    """Every fresh logical/transport admission observes the due boundary."""

    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_operator(
            source,
            symbol,
            "~asyncServeProducerEpisodeDue[node]",
            "TRUE",
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        symbol in error
        and "must block fresh Serve admission exactly once" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "AsyncTypeInvariant",
            "  /\\ AsyncProducerTypeInvariant\n",
            "",
        ),
        (
            "AsyncTypeInvariant",
            "  /\\ AsyncServeProducerEpisodeTypeInvariant\n",
            "",
        ),
        (
            "AsyncServiceActivationFrameVars",
            ", asyncServeProducerEpisodeDue",
            "",
        ),
    ),
)
def test_producer_episode_state_surfaces_cannot_omit_debt(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
) -> None:
    """Typing and activation frames retain every producer-debt coordinate."""

    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
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
    ("provider", "consumer"),
    (
        (
            "AsyncCandidateLifecycleStageIdentityInvariant",
            "AsyncCandidateSchedulerCoverageExposesBoundedProducerOrigin",
        ),
        (
            "AsyncStrongTypeInvariant",
            "AsyncNextNodeCommandOwnsOldestLifecycleOrdinal",
        ),
        (
            "AsyncStrongTypeInvariant",
            "AsyncNextDeferredCommandOwnsOldestLifecycleWithoutHandoff",
        ),
        (
            "AsyncStrongTypeInvariant",
            "AsyncDeferredHandoffRetainsExactSelectedLifecycle",
        ),
        (
            "AsyncStrongTypeInvariant",
            "AsyncRetainedCommitQcPacketAdmissionCreatesExactIngressOwner",
        ),
        (
            "AsyncStrongTypeInvariant",
            "AsyncRetainedCommitQcIngressCreatesExactDeliverQcOwner",
        ),
        (
            "AsyncStrongTypeInvariant",
            "AsyncRetainedCommitQcDeliveryRecordsExactReceipt",
        ),
        (
            "AsyncStrongTypeInvariant",
            "AsyncNextPreservesCandidateProducerContinuationScheduledExclusion",
        ),
        (
            "AsyncStrongTypeInvariant",
            "CertifiedResponseClaimAdmissionMatchesPostStateLifecycleCarrier",
        ),
        (
            "AsyncStrongTypeInvariant",
            "CertifiedResponseClaimAdmissionFreezesCompletePredecessorSources",
        ),
        (
            "AsyncStrongTypeInvariant",
            "CertifiedResponseLiveClaimCannotBeReplacedAtGst",
        ),
        (
            "AsyncStrongTypeInvariant",
            "AsyncNextPreservesLeaderWireContinuationSharedOrdinalNoCollision",
        ),
        (
            "AsyncStrongTypeInvariant",
            "CertifiedResponseClaimNewTimeoutSourceIsExcludedOrAboveFrozenCeiling",
        ),
        (
            "AsyncProgressOwnershipInvariant",
            "AsyncRetainedCommitQcIngressCreatesExactDeliverQcOwner",
        ),
        (
            "AsyncProgressOwnershipInvariant",
            "AsyncNextPreservesCandidateProducerContinuationScheduledExclusion",
        ),
        (
            "AsyncProgressOwnershipInvariant",
            "AsyncNextPreservesLeaderWireContinuationSharedOrdinalNoCollision",
        ),
        (
            "AsyncProgressOwnershipInvariant",
            "AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst",
        ),
    ),
)
def test_async_source_fidelity_pins_scheduler_coverage_theorem_order(
    tmp_path: Path,
    provider: str,
    consumer: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    checker = module._async_source_fidelity_errors
    assert checker(formal_dir) == []
    source = path.read_text(encoding="utf-8")
    provider_span = module._top_level_declaration_span(
        source, provider, kind="operator"
    )
    consumer_span = module._top_level_declaration_span(
        source, consumer, kind="theorem"
    )
    assert provider_span is not None
    assert consumer_span is not None
    provider_start, provider_end = provider_span
    consumer_start, consumer_end = consumer_span
    assert provider_start < provider_end <= consumer_start < consumer_end

    provider_block = source[provider_start:provider_end]
    without_provider = source[:provider_start] + source[provider_end:]
    shifted_consumer_end = consumer_end - len(provider_block)
    path.write_text(
        without_provider[:shifted_consumer_end]
        + provider_block
        + without_provider[shifted_consumer_end:],
        encoding="utf-8",
    )
    order_errors = checker(formal_dir)
    assert any(
        f"SANY requires provider {provider} to be declared before consumer "
        f"{consumer}" in error
        for error in order_errors
    ), order_errors

    consumer_block = source[consumer_start:consumer_end]
    assert provider in consumer_block
    path.write_text(
        source[:consumer_start]
        + "MutationSemanticProvider == TRUE\n\n"
        + consumer_block.replace(provider, "MutationSemanticProvider")
        + source[consumer_end:],
        encoding="utf-8",
    )
    dependency_errors = checker(formal_dir)
    assert any(
        consumer in error
        and f"must retain its reviewed dependency on {provider}" in error
        for error in dependency_errors
    ), dependency_errors


@pytest.mark.parametrize(
    "symbol",
    (
        "AsyncRecoveryExecutionInvariant",
        "HistoricalLockRestartAuthoritySourceRetentionInvariant",
        "AsyncGstRecoveryPhaseInvariant",
        "AsyncStrongTypeInvariant",
    ),
)
def test_async_source_fidelity_rejects_late_liveness_provider_relocation(
    tmp_path: Path,
    symbol: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2AsyncLivenessProofs.tla",
    )
    network_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    liveness_path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    checker = module._async_source_fidelity_errors
    assert checker(formal_dir) == []
    network_source = network_path.read_text(encoding="utf-8")
    provider_span = module._top_level_declaration_span(
        network_source,
        symbol,
        kind="operator",
    )
    assert provider_span is not None
    provider_start, provider_end = provider_span
    provider_block = network_source[provider_start:provider_end]
    network_path.write_text(
        network_source[:provider_start] + network_source[provider_end:],
        encoding="utf-8",
    )
    liveness_path.write_text(
        liveness_path.read_text(encoding="utf-8") + "\n" + provider_block,
        encoding="utf-8",
    )

    errors = checker(formal_dir)
    assert any(
        f"missing base asynchronous invariant provider {symbol}" in error
        for error in errors
    ), errors
    assert any(
        symbol in error
        and "must be provided only by SumeragiV2AsyncNetwork" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("item_accessor", "node_accessor"),
    (
        (
            "CertifiedResponseClaimItemFrozenCandidateOrigins",
            "CertifiedResponseClaimFrozenCandidateOrigins",
        ),
        (
            "CertifiedResponseClaimItemFrozenServeSources",
            "CertifiedResponseClaimFrozenServeSources",
        ),
        (
            "CertifiedResponseClaimItemFrozenContinuationSources",
            "CertifiedResponseClaimFrozenContinuationSources",
        ),
        (
            "CertifiedResponseClaimItemFrozenLeaderWireIdentities",
            "CertifiedResponseClaimFrozenLeaderWireIdentities",
        ),
    ),
)
def test_async_source_fidelity_rejects_claim_accessor_namespace_collision(
    tmp_path: Path,
    item_accessor: str,
    node_accessor: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    checker = module._async_source_fidelity_errors
    assert checker(formal_dir) == []

    source = path.read_text(encoding="utf-8")
    declaration = f"{item_accessor}(item) =="
    assert source.count(declaration) == 1
    path.write_text(
        source.replace(declaration, f"{node_accessor}(item) ==", 1),
        encoding="utf-8",
    )

    errors = checker(formal_dir)

    assert any(
        node_accessor in error
        and "may not collide with the finite-runner node accessor" in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("theorem", "local_rank_name"),
    (
        (
            "AsyncCandidateProducerContinuationResolutionSelectionIsLogicalMinimum",
            "LogicalRanks",
        ),
        (
            "AsyncCandidateProducerContinuationRuntimeSelectionIsLogicalMinimum",
            "RuntimeRanks",
        ),
    ),
)
def test_async_source_fidelity_rejects_core_ranks_shadowing(
    tmp_path: Path,
    theorem: str,
    local_rank_name: str,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    checker = module._async_source_fidelity_errors
    assert checker(formal_dir) == []

    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            theorem,
            f"DEFINE {local_rank_name} ==",
            "DEFINE Ranks ==",
        ),
        encoding="utf-8",
    )

    errors = checker(formal_dir)

    assert any(
        theorem in error and f"namespace {local_rank_name}" in error
        for error in errors
    ), errors
    assert any("may not shadow the imported Core Ranks" in error for error in errors)


def test_async_source_fidelity_rejects_old_progress_shortcuts(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    assert not [
        error
        for error in module._async_source_fidelity_errors(formal_dir)
        if str(path) in error
    ]

    mutations = (
        (
            '    [] OTHER -> <<>>\n\n(***************************************************************************\nClosed constructor audit',
            '    [] command.kind = "UnmodeledContinuation" -> <<>>\n'
            '    [] OTHER -> <<>>\n\n'
            '(***************************************************************************\nClosed constructor audit',
            "CommandSuccessors parent inventory must be closed",
        ),
        (
            "![command.node] = retained \\o FreshCommandSuccessors(command)",
            "![command.node] = retained \\o CommandSuccessors(command)",
            "AppendCausalSuccessors must equal only",
        ),
        (
            "  IF CandidateAdmissionCoalesced(candidate)\n"
            "       \\/ AsyncCandidateInternalBodyAvailableStageRetired(candidate)\n"
            "  THEN <<>>\n"
            "  ELSE <<candidate>>",
            "  IF TRUE THEN <<>> ELSE <<candidate>>",
            "FreshCandidateSequence must equal only",
        ),
        (
            "AsyncCandidateServiceIdentityScheduled(candidate)\n"
            "    \\/ AsyncCandidateServiceCoalesced(candidate)",
            "AsyncCandidateServiceIdentityScheduled(candidate)\n"
            "    \\/ FALSE",
            "CandidateAdmissionCoalesced must equal only",
        ),
        (
            "  /\\ AsyncOutstandingCarrierInvariant\n",
            "",
            "AsyncProgressOwnershipInvariant",
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
            "~CandidateAdmissionCoalesced(\n"
            "                               CertifiedResponseCandidate(item))",
            "~CandidateInFlight(\n"
            "                               CertifiedResponseCandidate(item))",
            "IngressItemCanDrain CertifiedResponse branch must use exactly one "
            "durable CandidateAdmissionCoalesced arm",
        ),
        (
            "~CandidateAdmissionCoalesced(\n"
            "                                    "
            "CommitCertificateResponseCandidate(item))",
            "~CandidateInFlight(\n"
            "                                    "
            "CommitCertificateResponseCandidate(item))",
            "IngressItemCanDrain CommitCertificateResponse branch must use "
            "exactly one durable CandidateAdmissionCoalesced arm",
        ),
        (
            "~CandidateAdmissionCoalesced(\n"
            "                               CertifiedResponseCandidate(item))",
            "TRUE",
            "IngressItemCanDrain CertifiedResponse branch must use exactly one "
            "durable CandidateAdmissionCoalesced arm",
        ),
        (
            "~CandidateAdmissionCoalesced(\n"
            "                                    "
            "CommitCertificateResponseCandidate(item))",
            "TRUE",
            "IngressItemCanDrain CommitCertificateResponse branch must use "
            "exactly one durable CandidateAdmissionCoalesced arm",
        ),
        (
            "                    \\/ CandidateAdmissionCoalesced(\n"
            "                         CertifiedResponseCandidate(item))\n",
            "",
            "IngressItemCanDrain CertifiedResponse branch must use exactly one "
            "durable CandidateAdmissionCoalesced arm",
        ),
        (
            "                         \\/ CandidateAdmissionCoalesced(\n"
            "                              "
            "CommitCertificateResponseCandidate(item))\n",
            "",
            "IngressItemCanDrain CommitCertificateResponse branch must use "
            "exactly one durable CandidateAdmissionCoalesced arm",
        ),
        (
            "IN /\\ IF CandidateAdmissionCoalesced(completion)\n"
            "                                  THEN UNCHANGED",
            "IN /\\ IF FALSE\n"
            "                                  THEN UNCHANGED",
            "DrainFairIngressSelected CertifiedResponse branch must consume "
            "an exact scheduled response",
        ),
        (
            "IN /\\ IF CandidateAdmissionCoalesced(\n"
            "                                               discoveredCandidate)\n"
            "                                        THEN UNCHANGED",
            "IN /\\ IF FALSE\n"
            "                                        THEN UNCHANGED",
            "DrainFairIngressSelected CommitCertificateResponse branch must "
            "consume an exact scheduled response",
        ),
        (
            "  /\\ ~IngressPacketPolicyRejected(item)\n",
            "",
            "CanAdmitIngressItemVia must equal only",
        ),
        (
            "  /\\ ~AsyncControlServiceAdmissionCoalesced(item)\n",
            "",
            "CanAdmitIngressItemVia must equal only",
        ),
        (
            "  /\\ ~AsyncCandidateServicePacketRetired(item)\n",
            "",
            "CanAdmitIngressItemVia must equal only",
        ),
        (
            "  /\\ ~AsyncCandidateStageRetired(item)\n",
            "",
            "CanAdmitIngressItemVia must equal only",
        ),
        (
            "  /\\ AsyncServeTransportAdmissionGateAllowsVia(\n"
            "       item.envelope.recipient, item, authenticatedSource)\n",
            "",
            "CanAdmitIngressItemVia must equal only",
        ),
        (
            "  /\\ server \\in request.envelope.certificate.signers\n",
            "",
            "CertifiedServeCanRespond must equal only",
        ),
        (
            "  /\\ AsyncServeTransportAdmissionGateAllows(\n"
            "       packet.item.envelope.recipient, packet.item)\n",
            "  /\\ CanAdmitIngressItem(packet.item)\n",
            "AsyncPacketOwnsClockDeadline must equal only",
        ),
        (
            "     AsyncPacketOwnsClockDeadline(packet)}",
            "     packet.deadline <= asyncNow}",
            "OverdueResponsivePackets must equal only",
        ),
        (
            "THEN UNCHANGED <<\n"
            "                                                    "
            "AsyncIoExceptServeReservationsVars,\n",
            "THEN UNCHANGED <<\n"
            "                                                    AsyncIoVars,\n",
            "DrainFairIngressSelected CertifiedResponse branch must consume "
            "an exact scheduled response",
        ),
        (
            "                  THEN /\\ AcceptOrCoalesceExactServeRequest(\n"
            "                            node, candidate, source)\n",
            "                  THEN /\\ asyncIoQueues' =\n"
            "                         [asyncIoQueues EXCEPT\n"
            "                            ![node] = Append(\n"
            "                              @, AsyncIoCertifiedServeJob(\n"
            "                                   node, candidate))]\n",
            "DrainFairIngressSelected must route the exact request through "
            "exactly one lifecycle-aware AcceptOrCoalesceExactServeRequest",
        ),
        (
            "        THEN /\\ AcceptOrCoalesceExactServeRequest(\n"
            "                  node, candidate, source)\n",
            "        THEN /\\ asyncIoQueues' =\n"
            "               [asyncIoQueues EXCEPT\n"
            "                  ![node] = Append(\n"
            "                    @, AsyncIoCertifiedServeJob(\n"
            "                         node, candidate))]\n",
            "DrainHistoricalIngressSelected must route the exact request "
            "through exactly one lifecycle-aware "
            "AcceptOrCoalesceExactServeRequest",
        ),
        (
            "      job == AsyncIoCertifiedServeJob(node, candidate)\n",
            "      job == candidate\n",
            "ResumeExactServeCapacityVia must equal only",
        ),
        (
            "               AsyncIoQueueWithResumedServe("
            "node, identity, job)]\n",
            "               Append(@, job)]\n",
            "ResumeExactServeCapacityVia must equal only",
        ),
        (
            "        THEN AcceptOrReserveExactServeIngressVia(\n"
            "               recipient, candidate, source)\n",
            "        THEN UNCHANGED <<AsyncServeLifecycleVars,\n"
            "                          AsyncServeIngressAdmissionVars>>\n",
            "AdmitHiddenPacket must couple the Serve transport gate",
        ),
        (
            "       /\\ ~AsyncServeJobQueued(node, owned.identity)}}\n",
            "       /\\ TRUE}}\n",
            "AsyncServeEarlierLiveReservationIdentities must equal only",
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
            "        if let Err(error) = self.retain_effect_batch_at_frontier(effects, ownership, frontier) {\n"
            "            return Err(self.close(error, services));\n"
            "        }\n"
            "        if let Err(error) = self.commit_reconciliation_frontier(frontier, services) {\n"
            "            return Err(self.close_after_transferring_runtime_terminals(error, services));\n"
            "        }",
            "        if let Err(error) = self.commit_reconciliation_frontier(frontier, services) {\n"
            "            return Err(self.close_after_transferring_runtime_terminals(error, services));\n"
            "        }\n"
            "        if let Err(error) = self.retain_effect_batch_at_frontier(effects, ownership, frontier) {\n"
            "            return Err(self.close(error, services));\n"
            "        }",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "consume_effects must snapshot, retain, and commit the reducer frontier before transferring terminals and draining"
        in error
        for error in errors
    ), errors

    effects_path.write_text(
        mutate_effect_item(
            "retain_effect_batch_at_frontier",
            "                .zip(ownership)\n",
            "                .zip(ownership.into_iter().rev())\n",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "retained effect construction must zip each retained effect with its immutable owner"
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
        "read one owned front, consume its effect and ownership, and pop_front exactly once in FIFO order"
        in error
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
        "both capacity retries must leave the owned FIFO head retained and stop"
        in error
        for error in errors
    ), errors

    effects_path.write_text(
        mutate_effect_item(
            "step",
            "        if self.retained_effect_batch.is_some() || self.parked_effect_batch.is_some() {\n",
            "        if false {\n",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "step must drain retained or parked debt and give blocked ordinary debt one typed pacemaker turn"
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
            "consume_pacemaker_effects",
            "evidence.owner().causal_origin().root_class != SERVICE_CLASS_PROGRESS",
            "evidence.owner().causal_origin().root_class == SERVICE_CLASS_PROGRESS",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "typed pacemaker effect consumption must reject every non-Progress causal owner"
        in error
        for error in errors
    ), errors

    effects_path.write_text(
        mutate_effect_item(
            "consume_pacemaker_effects",
            "        if let Err(error) = self.commit_reconciliation_frontier(frontier, services) {\n",
            "        if false && let Err(error) = self.commit_reconciliation_frontier(frontier, services) {\n",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "typed pacemaker effect consumption must commit even an empty reducer frontier"
        in error
        for error in errors
    ), errors

    effects_path.write_text(
        mutate_effect_item(
            "step_pacemaker_once",
            "self.runtime.step_pacemaker_effects(now)",
            "self.runtime.step_effects(now)",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "typed pacemaker executor turn must invoke only the runtime pacemaker scheduler"
        in error
        or "retained effect FIFO step_pacemaker_once declaration and complete control flow must match"
        in error
        for error in errors
    ), errors

    for old, new in (
        (
            "wire::ConsensusMessageV2Payload::TimeoutCertificate(_) => true",
            "wire::ConsensusMessageV2Payload::TimeoutCertificate(_) => false",
        ),
        (
            "matches!(certificate.phase, wire::GlobalPhase::Commit)",
            "matches!(certificate.phase, wire::GlobalPhase::Prepare | wire::GlobalPhase::Commit)",
        ),
        (
            "matches!(certificate.phase, wire::GlobalPhase::Commit)",
            "false",
        ),
    ):
        effects_path.write_text(
            mutate_effect_item(
                "network_ingress_is_certified_fence_escape",
                old,
                new,
            ),
            encoding="utf-8",
        )
        errors = module._async_source_fidelity_errors(formal_dir)
        assert any(
            "only TC, direct CommitQC, and discovery CommitQC may escape a hung signer"
            in error
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

    runner_path = reviewed_rust_item_provider(
        module, tmp_path, Path("crates/iroha_core/src/sumeragi/v2_runner.rs"),
        "drain_v2_ingress",
    )
    canonical_runner = runner_path.read_text(encoding="utf-8")

    def mutate_runner_item(name: str, old: str, new: str) -> str:
        item = module.rust_items(canonical_runner, name)[0]
        assert item.source.count(old) == 1, (name, old)
        return canonical_runner.replace(item.source, item.source.replace(old, new, 1), 1)

    runner_path.write_text(
        mutate_runner_item(
            "drain_v2_ingress",
            "mode != V2IngressDrainMode::Ordinary && turn != OuterIngressTurn::Ingress",
            "false && turn != OuterIngressTurn::Ingress",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "both non-Ordinary modes must skip Completion and Runtime turns" in error
        for error in errors
    ), errors

    runner_path.write_text(
        mutate_runner_item(
            "drain_v2_ingress",
            "if message.validate_version().is_err() {\n"
            "                        return false;\n"
            "                    }",
            "if false {\n                        return false;\n                    }",
        ),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "non-Ordinary drain must reject wrong-version ingress" in error
        for error in errors
    ), errors
    runner_path.write_text(canonical_runner, encoding="utf-8")


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


def test_async_candidate_producer_continuation_owner_rejects_target_only_narrowing(
    tmp_path: Path,
) -> None:
    """A leader-owned frozen producer continuation remains a concrete owner."""

    module = load_checker()
    module_name = "SumeragiV2AsyncCandidateProducerContinuationProofs"
    source_path = module.FORMAL_DIR / f"{module_name}.tla"
    target_path = tmp_path / source_path.name
    source = source_path.read_text(encoding="utf-8")
    target_path.write_text(source, encoding="utf-8")
    network_name = "SumeragiV2AsyncNetwork.tla"
    shutil.copy2(module.FORMAL_DIR / network_name, tmp_path / network_name)

    assert (
        module._async_candidate_producer_continuation_contract_errors(tmp_path)
        == []
    )

    for old, new in (
        ("record.node \\in {target, leader}", "record.node = target"),
        (
            "record.identity = AsyncCandidateServiceIdentity(record.candidate)",
            "record.identity.leader = record.candidate.leader",
        ),
    ):
        target_path.write_text(
            mutate_tla_operator(
                source, "AsyncCandidateProducerContinuationExactOwner", old, new
            ),
            encoding="utf-8",
        )
        errors = module._async_candidate_producer_continuation_contract_errors(
            tmp_path
        )
        assert any(
            "AsyncCandidateProducerContinuationExactOwner" in error
            and "finite producer-continuation contract" in error
            for error in errors
        ), errors


def test_asyncnetwork_authority_and_order_migrations_fail_closed(
    tmp_path: Path,
) -> None:
    """Reviewed terminal, Via, barrier, and transition authority is immutable."""

    module = load_checker()
    path = tmp_path / "SumeragiV2AsyncNetwork.tla"
    canonical = (module.FORMAL_DIR / path.name).read_text(encoding="utf-8")
    mutations = (
        (
            "AsyncCandidateServiceStateAfterTerminalRetirement",
            "AsyncCandidateEligibleTerminalDiscardsThisStep",
            "AsyncCandidateTerminalDiscardsThisStep",
        ),
        (
            "AsyncCandidateLifecycleNewAdmissions",
            "AsyncCandidateLifecycleSourcePhysicalOrdinalFor(\n"
            "          state, node, origin),\n"
            "        AsyncCandidateLifecyclePhysicalCutFor(\n"
            "          state, node, origin),",
            "0, 0,",
        ),
        (
            "AsyncPacketOwnsClockDeadline",
            "packet.authenticatedSource",
            "packet.item.source",
        ),
        (
            "AsyncIoExceptServeReservationsVars",
            ", asyncServeAttempts",
            "",
        ),
        (
            "CoalesceSupersededExactServeRequest",
            ",\n                  asyncServeAttempts",
            "",
        ),
        (
            "ResumeExactServeCapacityVia",
            "authenticatedSource \\in AsyncAuthenticatedDeliverySources",
            "candidate.item.source \\in AsyncAuthenticatedDeliverySources",
        ),
        (
            "CoalesceExactServeCapacityVia",
            "authenticatedSource \\in AsyncAuthenticatedDeliverySources",
            "candidate.item.source \\in AsyncAuthenticatedDeliverySources",
        ),
        (
            "ResumeExactServeCapacity",
            "candidate.item.source",
            "candidate.item.authenticatedSource",
        ),
        (
            "AsyncTimeoutRecoveryVoteBarrierException",
            "source = item.source",
            "TRUE",
        ),
        (
            "AsyncTimeoutRecoveryVoteCrossesCertifiedResponseBarrier",
            'owner.status = "Ingress"',
            "TRUE",
        ),
        (
            "AsyncFairIngressCoreStateTransition",
            "/\\ item \\in asyncSentItems\n"
            "        /\\ CommitCertificateResponseAuthorized(item)",
            "/\\ CommitCertificateResponseAuthorized(item)\n"
            "        /\\ item \\in asyncSentItems",
        ),
        (
            "TimeoutDue",
            "~AsyncOlderRuntimeLifecycleBlocksTimeout(node)",
            "~AsyncOlderCandidateLifecycleBlocksTimeout(node)",
        ),
        (
            "AdmitHiddenPacket",
            "CanAdmitIngressItemVia(item, source)",
            "CanAdmitIngressItem(item)",
        ),
    )
    assert module._async_network_migration_contract_errors(path, canonical) == []
    for symbol, old, new in mutations:
        mutated = mutate_tla_operator(canonical, symbol, old, new)
        errors = module._async_network_migration_contract_errors(path, mutated)
        assert len(errors) == 1, (symbol, errors)
        assert f"migration contract {symbol} " in errors[0], errors
