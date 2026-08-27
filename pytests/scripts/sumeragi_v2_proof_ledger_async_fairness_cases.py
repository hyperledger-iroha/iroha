@pytest.mark.parametrize(
    ("relative", "old", "new", "expected_error"),
    (
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "for _ in 0..limit.max(1) {",
            "loop {",
            "ordinary serialized runtime service must be a finite configured turn",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner.rs",
            "let scan_limit = lane_work.effect_count();",
            "let scan_limit = usize::MAX;",
            "lane service must snapshot one finite scan limit before dispatch",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
            "wake_rx.recv_timeout(IDLE_POLL)",
            "wake_rx.recv()",
            "ordinary lifecycle height must wait only for the finite local poll bound",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
            "wake_rx.recv_timeout(IDLE_POLL)",
            "wake_rx.recv()",
            "pending-Kura lifecycle height must wait only for the finite local poll bound",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_worker.rs",
            "const MAX_COMPLETION_DRAIN_BATCH: usize = 256;",
            "const MAX_COMPLETION_DRAIN_BATCH: usize = usize::MAX;",
            "completion service must retain a fixed finite batch bound",
        ),
    ),
)
def test_local_runner_service_contract_rejects_production_loop_mutations(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    relative: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    module = load_checker()
    formal_dir = local_runner_service_fixture(tmp_path, module)
    copy_serve_lifecycle_production_fixture(tmp_path, module)
    path = tmp_path / relative
    source = path.read_text(encoding="utf-8")
    assert source.count(old) >= 1, old
    path.write_text(source.replace(old, new, 1), encoding="utf-8")
    if relative.endswith("v2_runner/lifecycle_run_inner.rs"):
        items = module.rust_items(
            path.read_text(encoding="utf-8"), "run_lifecycle_active_height"
        )
        assert len(items) == 1
        item_sha256 = module._rust_item_token_sha256(items[0])
        for seals, key in (
            (
                module._PRODUCTION_LIFECYCLE_EXACT_OUTPUT_ITEM_SHA256,
                "ordinary_active",
            ),
            (
                module._LIFECYCLE_CERTIFIED_SERVE_ITEM_SHA256,
                "ordinary:run_lifecycle_active_height",
            ),
            (
                module._LOCKED_BODY_REPROPOSAL_RUST_ITEM_SHA256,
                "run_lifecycle_active_height",
            ),
        ):
            monkeypatch.setitem(seals, key, item_sha256)

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
            "  \\/ (\\E recipient \\in ValidatorIds, "
            "source \\in AsyncIngressSources:\n"
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


@pytest.mark.parametrize(
    ("symbol", "old", "new", "expected_error"),
    (
        (
            "AsyncSchedulerTypeInvariant",
            "  /\\ AsyncHistoricalRecoveryTypeInvariant",
            "  /\\ AsyncHistoricalRecoveryTypeInvariant\n"
            "  /\\ AsyncServiceActivationPairInvariant",
            "AsyncSchedulerTypeInvariant must equal only",
        ),
        (
            "AsyncTypeInvariant",
            "  /\\ AsyncServiceActivationPairInvariant\n",
            "",
            "AsyncTypeInvariant must equal only",
        ),
        (
            "AsyncServiceActivationTransition",
            "  \\/ UNCHANGED asyncServiceActivationState",
            "",
            "AsyncServiceActivationTransition must equal only",
        ),
        (
            "AsyncSetGST",
            "  /\\ Responsive \\subseteq AsyncActiveServiceNodes\n",
            "",
            "AsyncSetGST must equal only",
        ),
        (
            "AsyncFairActionAt",
            "  \\/ (\\E node \\in Responsive:\n"
            "        AsyncActivateServiceNode(node))\n",
            "",
            "AsyncFairActionAt must equal only",
        ),
        (
            "RunNodeWork",
            "  /\\ node \\in AsyncActiveServiceNodes\n",
            "",
            "RunNodeWork omits required production behavior",
        ),
        (
            "ServiceIoWorkerWork",
            "  /\\ node \\in AsyncActiveServiceNodes\n",
            "",
            "ServiceIoWorkerWork omits required production behavior",
        ),
        (
            "AsyncEnterIndexedServiceActivation",
            "  /\\ ~AsyncServiceActivationRestricted\n",
            "",
            "AsyncEnterIndexedServiceActivation must equal only",
        ),
        (
            "AsyncServiceActivationPairInvariant",
            "asyncNodeServiceDeadlines[node] # 0",
            "asyncNodeServiceDeadlines[node] >= 0",
            "AsyncServiceActivationPairInvariant must equal only",
        ),
    ),
)
def test_async_service_activation_source_mutations_fail_closed(
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
        mutate_tla_operator(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(expected_error in error for error in errors), errors


def test_async_source_fidelity_rejects_an_unreviewed_model_local_theorem(
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


@pytest.mark.parametrize(
    "symbol",
    (
        "AsyncCandidateServiceStageOrdinalIsBounded",
        "AsyncCandidateProducerContinuationRunnerSelectionIsTwoStageLogicalMinimum",
        "AsyncRetransmitFreshEpisodeConsumesSharedLifecycleOrdinal",
        "AsyncOlderRetransmitLifecycleCannotAloneBlockDueTimeout",
        "AsyncTimeoutLifecycleFreezeBoundaryMintsAfterPriorAdmissions",
        "AsyncTimeoutRecoveryDefinedVoteCandidateOwnerIsMember",
        "AsyncLeaderWireCarrierCannotBypassFrozenPrefix",
        "RetireLeaderWireLifecycleRecoveryCutPrunesOnlyDormant",
        "AsyncServeProducerTurnMeasureIsFinite",
        "AsyncServeProducerTurnBlocksFreshServeAdmission",
        "AsyncServeCompletionArmsOneShotProducerTurn",
        "AsyncServeProducerTurnRunnerAttemptStrictlyConsumesDebt",
        "AsyncServeProducerTurnRestartPreservesDebt",
        "AsyncTimeoutRecoveryRetainedEpisodesContainFramedEpisode",
        "AsyncTimeoutRecoverySupersedesOnlyExactPreTimeoutRetransmit",
        "LeaderWireRecoveryCutRetainsOrdinalHighwaters",
        "AsyncCandidateProducerContinuationStatusTransitionIsMonotone",
        "AsyncTimeoutVoteFairIngressDrainLeavesCoreState",
        "AsyncTimeoutRecoveryRolloverInstanceStartsEmpty",
    ),
)
def test_async_source_fidelity_rejects_reviewed_theorem_omission(
    tmp_path: Path,
    symbol: str,
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
    span = module._top_level_declaration_span(source, symbol, kind="theorem")
    assert span is not None
    start, end = span
    path.write_text(source[:start] + source[end:], encoding="utf-8")

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        "must declare exactly the reviewed local theorem inventory" in error
        and symbol in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("replacement", "stale_alias"),
    (
        (
            "AsyncCandidateProducerContinuationRunnerSelectionIsTwoStageLogicalMinimum",
            "AsyncCandidateProducerContinuationRunnerSelectionIsGlobalMinimum",
        ),
        (
            "AsyncTimeoutLifecycleFreezeBoundaryMintsAfterPriorAdmissions",
            "AsyncTimeoutLifecycleDueTransitionMintsBeforeLaterAdmissions",
        ),
    ),
)
def test_async_source_fidelity_rejects_stale_reviewed_theorem_alias(
    tmp_path: Path,
    replacement: str,
    stale_alias: str,
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
    declaration = f"THEOREM {replacement} =="
    assert source.count(declaration) == 1
    path.write_text(
        source.replace(declaration, f"THEOREM {stale_alias} ==", 1),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        "must declare exactly the reviewed local theorem inventory" in error
        and replacement in error
        and stale_alias in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("first", "second"),
    (
        (
            "AsyncCandidateServiceStageCarrierHasExactlyElevenClasses",
            "AsyncCandidateServiceStageOrdinalIsBounded",
        ),
        (
            "AsyncServeProducerTurnMeasureIsFinite",
            "AsyncServeProducerTurnBlocksFreshServeAdmission",
        ),
    ),
)
def test_async_source_fidelity_rejects_reviewed_theorem_order_drift(
    tmp_path: Path,
    first: str,
    second: str,
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
    first_span = module._top_level_declaration_span(
        source, first, kind="theorem"
    )
    second_span = module._top_level_declaration_span(
        source, second, kind="theorem"
    )
    assert first_span is not None
    assert second_span is not None
    first_start, first_end = first_span
    second_start, second_end = second_span
    assert first_start < first_end == second_start < second_end
    path.write_text(
        source[:first_start]
        + source[second_start:second_end]
        + source[first_start:first_end]
        + source[second_end:],
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        "must declare exactly the reviewed local theorem inventory" in error
        and first in error
        and second in error
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


@pytest.mark.parametrize(
    ("old", "new"),
    (
        (
            "  /\\ AsyncServeProducerTurnTransition\n",
            "",
        ),
        (
            "  /\\ AsyncProducerProjectionStep\n"
            "  /\\ AsyncServeProducerTurnTransition\n",
            "  /\\ AsyncServeProducerTurnTransition\n"
            "  /\\ AsyncProducerProjectionStep\n",
        ),
    ),
)
def test_async_next_requires_ordered_serve_producer_episode_frame(
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
        mutate_tla_operator(source, "AsyncNext", old, new),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        "AsyncNext must equal only the exact reviewed" in error
        for error in errors
    ), errors


def test_async_next_rejects_extra_disjunct(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = (
        module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla"
    ).read_text(encoding="utf-8")
    extracted = module._top_level_operator_body(
        source,
        "AsyncNext",
        preserve_string_contents=True,
    )
    assert extracted is not None
    path.write_text(
        replace_tla_operator_body(
            source,
            "AsyncNext",
            "TRUE \\/ (" + extracted[0] + ")",
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any("AsyncNext must equal only the exact reviewed" in error for error in errors)


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
        (
            "PostGstResolveLocalCandidateProducerContinuation",
            "AsyncNonRunnerOuterFrame",
        ),
        (
            "PostGstServiceConditionalTransportProducerContinuation",
            "AsyncNonRunnerOuterFrame",
        ),
        (
            "PostGstServiceVolatileBodyProducerContinuation",
            "AsyncNonRunnerOuterFrame",
        ),
        (
            "PostGstRetireLeaderWireLifecycleSlot",
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
            "must name exactly the 23 canonical framed actions directly",
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
            "must name exactly the 23 canonical framed actions directly",
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


@pytest.mark.parametrize(
    ("symbol", "old", "new", "expected_error"),
    (
        (
            "AsyncLeaderWireRetryableDormant",
            "AsyncLeaderWireExactTransportPacketPresent(record)",
            "record.slot.source \\in Responsive",
            "must require a concrete exact transport packet",
        ),
        (
            "AsyncLeaderWirePotentialPredecessorRecordsIn",
            "candidate.schedulerOrdinal < ownerOrdinal",
            "candidate.schedulerOrdinal < ownerOrdinal\n"
            "     /\\ AsyncLeaderWireLifecycleActive(candidate)",
            "must derive every retained lower scheduler owner",
        ),
        (
            "PostGstAdmitExactDormantLeaderWire",
            "DueSourcePackets(recipient, source) # {}",
            "TRUE",
            "must require one real due exact packet",
        ),
        (
            "AsyncLeaderWireLifecycleStateAfterIngressAdmission",
            "!.ingressPredecessors =\n"
            "                       AsyncLeaderWireIngressPrefixSnapshot(\n"
            "                         item.envelope.recipient),",
            "!.ingressPredecessors =\n"
            "                       [source \\in AsyncIngressSources |-> 0],",
            "with a fresh physical ordinal and current physical prefix",
        ),
    ),
)
def test_async_source_fidelity_rejects_dormant_potential_owner_weakening(
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
        mutate_tla_operator(source, symbol, old, new),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(expected_error in error for error in errors), errors


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "AsyncLeaderWireEarliestPhysicalIngressRecord",
            "record.physicalAdmissionOrdinal\n"
            "        <= other.physicalAdmissionOrdinal",
            "record.schedulerOrdinal <= other.schedulerOrdinal",
        ),
        (
            "AsyncServeIngressOwnsSharedPhysicalTurn",
            "node).physicalAdmissionOrdinal",
            "node).schedulerOrdinal",
        ),
        (
            "AsyncLeaderWireIngressOwnsSharedPhysicalTurn",
            "node).physicalAdmissionOrdinal",
            "node).schedulerOrdinal",
        ),
        (
            "AsyncOrdinaryIngressEarliestPhysicalRecord",
            "carrier.physicalOrdinal <= other.physicalOrdinal",
            "carrier.schedulerOrdinal <= other.schedulerOrdinal",
        ),
        (
            "AsyncOrdinaryIngressOwnsSharedPhysicalTurn",
            "AsyncOrdinaryIngressEarliestPhysicalRecord(node).physicalOrdinal",
            "AsyncOrdinaryIngressEarliestPhysicalRecord(node).schedulerOrdinal",
        ),
        (
            "AsyncEarliestIngressPhysicalOrdinal",
            "AsyncOrdinaryIngressEarliestPhysicalRecord(node).physicalOrdinal",
            "AsyncOrdinaryIngressEarliestPhysicalRecord(node).schedulerOrdinal",
        ),
    ),
)
def test_async_source_fidelity_rejects_physical_leader_selector_weakening(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
) -> None:
    """The model may not substitute retained logical order for live carriers."""

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


def test_async_source_fidelity_rejects_durable_ingress_and_restart_weakening(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path,
        module,
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2Core.tla",
    )
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    mutations = (
        (
            "CandidateAdmissionCoalesced",
            "AsyncCandidateServiceCoalesced(candidate)",
            "FALSE",
        ),
        (
            "AsyncCandidateAdmissionIdentity",
            "AsyncConsumerEventTag(candidate)",
            "0",
        ),
        (
            "AsyncCandidateAdmissionIdentityObsolete",
            'identity.service.phase = "DeliverChunk"',
            "TRUE",
        ),
        (
            "AsyncChunkIngressStageRetired",
            "\\/ NodeHasDecision(recipient)",
            "\\/ FALSE",
        ),
        (
            "AsyncCandidatePhysicallyDiscardedThisStep",
            "  /\\ ~CommandDispatchable(candidate)\n",
            "  /\\ TRUE\n",
        ),
        (
            "AsyncCandidateTerminallyDiscardedThisStep",
            "  /\\ candidate.item = NoAsyncItem\n",
            "  /\\ TRUE\n",
        ),
        (
            "AsyncCandidateTerminalRetirementsThisStep",
            "AsyncCandidateTerminalDiscardsThisStep",
            "{}",
        ),
        (
            "AsyncCandidateTerminalRetirementEligibleAfterStep",
            "candidate.kind \\notin AsyncRestartScopedCandidateServiceKinds",
            "TRUE",
        ),
        (
            "AsyncCandidateServiceStateAfterTerminalRetirement",
            "\\/ existing # {}",
            "\\/ TRUE",
        ),
        (
            "AsyncCandidateServiceStateAfterSuccessfulService",
            "!.candidateServiceMarkers =",
            "!.candidateTerminalTombstones =",
        ),
        (
            "AsyncControlServiceSlotTransition",
            "AsyncCandidateTerminalDiscardsThisStep # {}",
            "FALSE",
        ),
        (
            "AsyncServeLogicalIdentityRetiredOrSuperseded",
            "> AsyncServeRequestView(request)",
            ">= AsyncServeRequestView(request)",
        ),
        (
            "ReserveExactServeCapacity",
            "     /\\ asyncNextServeIngressOrdinal' =\n"
            "          [asyncNextServeIngressOrdinal EXCEPT ![node] = @ + 1]\n",
            "     /\\ UNCHANGED asyncNextServeIngressOrdinal\n",
        ),
        (
            "PopSelectedIngress",
            "     /\\ asyncServeIngressAdmissions' =\n"
            "          AsyncServeIngressAdmissionsAfterIngressDrain(\n"
            "            node, source, laneIndex)\n",
            "     /\\ UNCHANGED asyncServeIngressAdmissions\n",
        ),
        (
            "AsyncServeLifecycleTypeInvariant",
            "  /\\ AsyncServeIngressAdmissionInvariant\n",
            "",
        ),
        (
            "AsyncCandidateRestartReplayTombstoned",
            "AsyncCandidateTerminalTombstoned(candidate)",
            "AsyncCandidateServiceCoalesced(candidate)",
        ),
        (
            "FreshRestartCandidateSequence",
            "AsyncCandidateRestartReplayTombstoned(replay[1])",
            "CandidateAdmissionCoalesced(replay[1])",
        ),
        (
            "AsyncCandidateServiceMarkersAfterReset",
            "record.node \\notin resetNodes",
            "TRUE",
        ),
        (
            "AsyncControlServiceStateAfterReset",
            "AsyncCandidateServiceMarkersAfterReset(state, resetNodes)",
            "state.candidateServiceMarkers",
        ),
        (
            "AsyncNext",
            "  /\\ AsyncControlServiceSlotTransition\n",
            "",
        ),
        (
            "CanAdmitIngressItem",
            "  /\\ ~AsyncCandidateServicePacketRetired(item)\n",
            "",
        ),
        (
            "CanAdmitIngressItem",
            "  /\\ ~AsyncCandidateStageRetired(item)\n",
            "",
        ),
    )
    mutated = source
    for symbol, old, new in mutations:
        mutated = mutate_tla_operator(mutated, symbol, old, new)
    path.write_text(mutated, encoding="utf-8")

    errors = module._async_source_fidelity_errors(formal_dir)
    expected = (
        "CandidateAdmissionCoalesced must equal only",
        "AsyncCandidateAdmissionIdentity must equal only",
        "AsyncCandidateAdmissionIdentityObsolete must equal only",
        "AsyncChunkIngressStageRetired must equal only",
        "AsyncCandidatePhysicallyDiscardedThisStep must equal only",
        "AsyncCandidateTerminallyDiscardedThisStep must equal only",
        "AsyncCandidateTerminalRetirementsThisStep must equal only",
        "AsyncCandidateTerminalRetirementEligibleAfterStep must equal only",
        "AsyncCandidateServiceStateAfterTerminalRetirement must equal only",
        "AsyncCandidateServiceStateAfterSuccessfulService must equal only",
        "AsyncControlServiceSlotTransition omits required production behavior",
        "AsyncServeLogicalIdentityRetiredOrSuperseded must equal only",
        "ReserveExactServeCapacity must equal only",
        "PopSelectedIngress must equal only",
        "AsyncServeLifecycleTypeInvariant must equal only",
        "AsyncCandidateRestartReplayTombstoned must equal only",
        "FreshRestartCandidateSequence must equal only",
        "AsyncCandidateServiceMarkersAfterReset must equal only",
        "AsyncControlServiceStateAfterReset omits required production behavior",
        "AsyncNext omits required production behavior",
        "CanAdmitIngressItem must equal only",
    )
    for marker in expected:
        assert any(marker in error for error in errors), (marker, errors)


@pytest.mark.parametrize(
    ("symbol", "old", "new", "expected"),
    (
        (
            "AsyncCandidateServiceMarker",
            "generation |-> episodeGeneration",
            "generation |-> 0",
            "must equal only",
        ),
        (
            "AsyncCandidateServiceMarkerSet",
            "episodeGeneration \\in Generations",
            "TRUE",
            "must equal only",
        ),
        (
            "AsyncCandidateServiceTombstone",
            "phase |-> candidate.kind",
            'phase |-> "DeliverChunk"',
            "must equal only",
        ),
        (
            "AsyncControlServiceStateTypeInvariant",
            "IsFiniteSet(AsyncCandidateTerminalTombstones)",
            "TRUE",
            "must equal only",
        ),
        (
            "AsyncCandidateServiceRecordRetainedAfterStep",
            "~AsyncNodeHasDecisionAfter(record.node)",
            "TRUE",
            "must equal only",
        ),
        (
            "AsyncCandidateServiceEligibleAfterStep",
            "candidate.consumerGeneration = generation'[candidate.node]",
            "TRUE",
            "must equal only",
        ),
        (
            "AsyncCandidateServiceLifecycleInvariant",
            "record.phase \\notin AsyncRestartScopedCandidateServiceKinds",
            "TRUE",
            "must equal only",
        ),
        (
            "AsyncCandidateTransientServiceActive",
            "~CandidateScheduled(candidate)",
            "TRUE",
            "must equal only",
        ),
        (
            "AsyncCandidateTerminalTombstoneActive",
            "~CandidateScheduled(candidate)",
            "TRUE",
            "must equal only",
        ),
        (
            "AsyncControlServiceSlotTransition",
            "ELSE candidateReclamationState",
            "ELSE candidateReclamationState \\/ TRUE",
            "must equal only",
        ),
        (
            "AsyncCandidateRestartReplayTombstoned",
            "AsyncCandidateTerminalTombstoned(candidate)",
            "AsyncCandidateServiceCoalesced(candidate)",
            "must equal only",
        ),
        (
            "AsyncCandidateRestartReplayTombstoned",
            "candidate.kind \\notin AsyncRestartScopedCandidateServiceKinds",
            "TRUE",
            "must equal only",
        ),
        (
            "AsyncCandidateServiceMarkersAfterReset",
            "record.node \\notin resetNodes",
            "TRUE",
            "must equal only",
        ),
        (
            "AsyncControlServiceStateAfterReset",
            "AsyncCandidateServiceMarkersAfterReset(state, resetNodes)",
            "state.candidateServiceMarkers",
            "omits required production behavior",
        ),
    ),
)
def test_async_source_fidelity_rejects_restart_durable_transient_marker(
    tmp_path: Path,
    symbol: str,
    old: str,
    new: str,
    expected: str,
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

    path.write_text(
        mutate_tla_operator(source, symbol, old, new),
        encoding="utf-8",
    )
    errors = module._async_source_fidelity_errors(formal_dir)

    assert any(
        symbol in error and expected in error for error in errors
    ), errors


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
            "signatures == RestartSignatureReplay(node)",
            "signatures == "
            "FreshRestartCandidateSequence(RestartSignatureReplay(node))",
            "PreGstResponsiveReplay omits required production behavior",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "RememberedControl(withoutOwnTc, items)",
            "RememberedControl(retained, items)",
            "InstalledControlAfterTC must equal only",
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
            "must constrain every and only the restart-local "
            "AsyncSchedulerVars components",
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
            "  /\\ generation' = [generation EXCEPT ![node] = 0]",
            "  /\\ generation' = [generation EXCEPT ![node] = 1]",
            "Restart omits authenticated generation",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "     /\\ asyncRecoveryGeneration' = generation[node] + 1\n",
            "     /\\ asyncRecoveryGeneration' = 0\n",
            "PreGstResponsiveRestart omits required production behavior",
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
            "  /\\ (AsyncResponsiveAppliedArchiveServers \\ {node}) # {}",
            "  /\\ TRUE",
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
            "AsyncSchedulerTypeInvariant must equal only",
        ),
        (
            "AsyncSchedulerVars",
            "    asyncHeldChunks,\n"
            "    asyncHistoricalRecoveryTargets,\n",
            "    asyncHeldChunks,\n",
            "AsyncSchedulerVars omits required production behavior",
        ),
        (
            "AsyncSchedulerExceptHistoricalRecoveryTargets",
            "    asyncIngressLanes, asyncIngressReady, asyncLeaderWireLifecycles,\n",
            "    asyncIngressLanes, asyncIngressReady,\n",
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
            "     /\\ \\A node \\in AsyncTimedServiceNodes:\n",
            "     /\\ \\A node \\in AsyncCurrentResponsiveVoters:\n",
            "AsyncTickEnabled omits required production behavior",
        ),
        (
            "HistoricalRecoveryPacketCorridor",
            "  \\/ /\\ HistoricalRecoveryTarget(source)\n"
            "        /\\ recipient \\in AsyncArchiveIoServiceNodes",
            "",
            "HistoricalRecoveryPacketCorridor must equal only",
        ),
        (
            "CommitCertificateResponseAuthorized",
            "  /\\ item.source = item.envelope.request.envelope.recipient\n",
            "  /\\ item.source = AsyncUntrustedSource\n",
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
            "AsyncFairIngressCoreStateTransition",
            "        /\\ item \\in asyncSentItems\n",
            "",
            "must import only an authorized, sent, not-yet-present Commit-certificate",
        ),
        (
            "AsyncFairIngressCoreStateTransition",
            "        /\\ CommitCertificateResponseAuthorized(item)\n",
            "",
            "must import only an authorized, sent, not-yet-present Commit-certificate",
        ),
        (
            "AsyncFairIngressCoreStateTransition",
            "        /\\ DiscoveredCommitQcItem(item).envelope \\notin qcNetwork\n",
            "",
            "must import only an authorized, sent, not-yet-present Commit-certificate",
        ),
        (
            "AsyncFairIngressCoreStateTransition",
            "  THEN ImportAuthenticatedCommitCertificate(\n         DiscoveredCommitQcItem(item).envelope)\n",
            "  THEN UNCHANGED vars\n",
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
            "AsyncTcRecordTyped",
            '{"context", "height", "view", "votes", "highestPrepareQc"}',
            '{"context", "height", "view", "votes"}',
            "AsyncTcRecordTyped must equal only",
        ),
        (
            "AsyncTcRecordTyped",
            "  /\\ tc.highestPrepareQc \\in PrepareQcOptionSet",
            "  /\\ TRUE",
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
            "      \\/ BusyCompletionCandidates(node) # {}",
            "      \\/ BusyCompletionCandidates(node) \\cap AsyncCandidateSet # {}",
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
    if symbol == "CommitCertificateResponseAuthorized" and "request.envelope.recipient" in old:
        for related_symbol, related_old, related_new in (
            (
                "AsyncItemTyped",
                "            /\\ item.source =\n"
                "                 item.envelope.request.envelope.recipient\n",
                "            /\\ item.source = AsyncUntrustedSource\n",
            ),
            (
                "AsyncCommitImportResponseEvidence",
                "  /\\ candidate.evidence.source =\n"
                "       candidate.evidence.envelope.request.envelope.recipient\n",
                "  /\\ candidate.evidence.source = AsyncUntrustedSource\n",
            ),
        ):
            path.write_text(
                mutate_tla_operator(source, related_symbol, related_old, related_new),
                encoding="utf-8",
            )
            related_errors = module._async_source_fidelity_errors(formal_dir)
            assert any(
                f"{related_symbol} must equal only" in error
                or f"{related_symbol} omits required production behavior" in error
                for error in related_errors
            ), related_errors


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
            "  /\\ \\A recipient \\in ValidatorIds, "
            "source \\in AsyncIngressSources:\n"
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
        'CertifiedServeCanRespond(server, request) ==\n'
        '  /\\ request.kind = "CertifiedRequest"\n'
        '  /\\ request.envelope.recipient = server\n'
        '  /\\ \\/ NodeHasApplication(server)\n'
        '     \\/ server \\in request.envelope.certificate.signers\n'
        '  /\\ BodyHeldBy(durableBodies, server, request.envelope.certificate.context,\n'
        '                request.envelope.view, request.envelope.subject)'
    )
    assert needle in async_source
    async_path.write_text(
        async_source.replace(
            needle,
            needle.replace(
                '  /\\ \\/ NodeHasApplication(server)\n'
                '     \\/ server \\in request.envelope.certificate.signers\n',
                '  /\\ server \\in request.envelope.certificate.signers\n',
            ),
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_source_fidelity_errors(formal_dir)
    assert any(
        "CertifiedServeCanRespond must equal only the reviewed normalized "
        "operator body digest" in error
        for error in errors
    ), errors


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
    scheduler_fields = module.HISTORICAL_INDEXED_SCHEDULER_FIELDS
    scheduler_mapping = ",\n       ".join(
        f"{field} <- IndexedScheduler(initialContext, {index})"
        for index, field in enumerate(scheduler_fields, start=1)
    )
    recovery_fields = module.HISTORICAL_INDEXED_RECOVERY_FIELDS
    recovery_mapping = ",\n       ".join(
        f"{field} <- IndexedRecovery(initialContext, {index})"
        for index, field in enumerate(recovery_fields, start=1)
    )
    producer_fields = module.HISTORICAL_INDEXED_PRODUCER_FIELDS
    producer_mapping = ",\n       ".join(
        f"{field} <- IndexedProducer(initialContext, {index})"
        for index, field in enumerate(producer_fields, start=1)
    )
    core_fields = module.HISTORICAL_INDEXED_CORE_FIELDS
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
    verification_producer_mapping = ",\n       ".join(
        f"{field} <- VerificationProducer({index})"
        for index, field in enumerate(producer_fields, start=1)
    )
    refinement = (
        "---- MODULE SumeragiV2ChainEpochRefinement ----\n"
        "CONSTANT VerificationContext\n"
        "IndexedDuplicatedGst(initialContext) ==\n"
        "  indexedAsyncState[initialContext][1]\n"
        "IndexedCore(initialContext, component) ==\n"
        "  indexedAsyncState[initialContext][2][component]\n"
        "IndexedScheduler(initialContext, component) ==\n"
        "  indexedAsyncState[initialContext][3][component]\n"
        "IndexedRecovery(initialContext, component) ==\n"
        "  indexedAsyncState[initialContext][4][component]\n"
        "IndexedProducer(initialContext, component) ==\n"
        "  indexedAsyncState[initialContext][5][component]\n"
        "IndexedFixedCorridorDeadlines(initialContext) ==\n"
        "  indexedAsyncState[initialContext][6]\n"
        "IndexedAsync(initialContext) ==\n"
        "  INSTANCE SumeragiV2AsyncNetwork WITH\n"
        f"       {core_mapping},\n       {scheduler_mapping},\n"
        f"       {recovery_mapping},\n"
        f"       {producer_mapping},\n"
        "       asyncFixedCorridorDeadlines <-\n"
        "         IndexedFixedCorridorDeadlines(initialContext)\n"
        "VerificationCore(component) ==\n"
        "  IndexedCore(VerificationContext, component)\n"
        "VerificationScheduler(component) ==\n"
        "  IndexedScheduler(VerificationContext, component)\n"
        "VerificationRecovery(component) ==\n"
        "  IndexedRecovery(VerificationContext, component)\n"
        "VerificationProducer(component) ==\n"
        "  IndexedProducer(VerificationContext, component)\n"
        "VerificationFixedCorridorDeadlines ==\n"
        "  IndexedFixedCorridorDeadlines(VerificationContext)\n"
        "VerificationAsyncProof ==\n"
        "  INSTANCE SumeragiV2AsyncTemporalClosureProofs WITH\n"
        f"       {verification_core_mapping},\n"
        f"       {verification_scheduler_mapping},\n"
        f"       {verification_recovery_mapping},\n"
        f"       {verification_producer_mapping},\n"
        "       asyncFixedCorridorDeadlines <-\n"
        "         VerificationFixedCorridorDeadlines\n"
        "IndexedAsyncStateShape ==\n"
        "  /\\ Len(indexedAsyncState[initialContext]) = 6\n"
        "  /\\ DOMAIN indexedAsyncState[initialContext] = 1..6\n"
        "  /\\ indexedAsyncState[initialContext][1] =\n"
        "       indexedAsyncState[initialContext][2][7]\n"
        "  /\\ Len(indexedAsyncState[initialContext][2]) = 49\n"
        "  /\\ DOMAIN indexedAsyncState[initialContext][2] = 1..49\n"
        "  /\\ Len(indexedAsyncState[initialContext][3]) = 46\n"
        "  /\\ DOMAIN indexedAsyncState[initialContext][3] = 1..46\n"
        "  /\\ Len(indexedAsyncState[initialContext][4]) = 5\n"
        "  /\\ DOMAIN indexedAsyncState[initialContext][4] = 1..5\n"
        "  /\\ Len(indexedAsyncState[initialContext][5]) = 3\n"
        "  /\\ DOMAIN indexedAsyncState[initialContext][5] = 1..3\n"
        "THEOREM IndexedInstanceVariablesAreExact ==\n"
        "  IndexedAsyncStateShape\n"
        "    => \\A initialContext \\in AdmissibleContextRecords:\n"
        "         IndexedAsync(initialContext)!AsyncAllVars =\n"
        "           IndexedAsyncStateAt(initialContext)\n"
        "BY DEF IndexedAsyncStateShape, IndexedAsyncStateAt,\n"
        "       IndexedDuplicatedGst, IndexedCore, IndexedScheduler,\n"
        "       IndexedRecovery, IndexedProducer,\n"
        "       IndexedFixedCorridorDeadlines\n"
        "IndexedJoinedRunnerStep(initialContext) ==\n"
        "  \\/ \\E node \\in Responsive:\n"
        "       /\\ node \\in joinedByContext[initialContext]\n"
        "       /\\ IndexedAsync(initialContext)!RunHistoricalServer(node)\n"
        "IndexedJoinedNonRunnerStep(initialContext) ==\n"
        "  /\\ (\\/ \\E node \\in IndexedAsync(initialContext)!\n"
        "                   AsyncCurrentResponsiveVoters:\n"
        "        /\\ IndexedNodeCurrentAt(initialContext, node)\n"
        "        /\\ IndexedAsync(initialContext)!\n"
        "             DirectCommitCertificateDiscoveryStep(node)\n"
        "      \\/ \\E node \\in Responsive:\n"
        "        /\\ node \\in joinedByContext[initialContext]\n"
        "        /\\ IndexedAsync(initialContext)!ServiceIoWorker(node)\n"
        "      \\/ \\E node \\in IndexedAsync(initialContext)!"
        "AsyncCurrentResponsiveVoters:\n"
        "        /\\ node \\in joinedByContext[initialContext]\n"
        "        /\\ IndexedAsync(initialContext)!EnqueueIoLocalControl(node))\n"
        "  /\\ UNCHANGED IndexedScheduler(initialContext, 33)\n"
        "IndexedJoinedNonCrashStep(initialContext) ==\n"
        "  /\\ (IndexedJoinedRunnerStep(initialContext)\n"
        "       \\/ IndexedJoinedNonRunnerStep(initialContext))\n"
        "  /\\ UNCHANGED <<IndexedCore(initialContext, 6),\n"
        "                 IndexedAsync(initialContext)!\n"
        "                   AsyncRecoveryControlVars>>\n"
        "IndexedJoinedAsyncNext(initialContext) ==\n"
        "  /\\ (IndexedJoinedNonCrashStep(initialContext)\n"
        "       \\/ \\E node \\in ValidatorIds:\n"
        "            IndexedAsync(initialContext)!PreGstCrash(node))\n"
        "  /\\ IndexedAsync(initialContext)!\n"
        "       AsyncHistoricalLockRestartAuthorityTransition\n"
        "  /\\ IndexedAsync(initialContext)!AsyncProducerProjectionStep\n"
        "  /\\ UNCHANGED IndexedScheduler(initialContext, 46)\n"
        "  /\\ UNCHANGED <<IndexedCore(initialContext, 1),\n"
        "                 IndexedCore(initialContext, 2)>>\n"
        "  /\\ [IndexedAsync(initialContext)!Next]_(\n"
        "       IndexedAsync(initialContext)!vars)\n"
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
            "Len(indexedAsyncState[initialContext][2]) = 49",
            "Len(indexedAsyncState[initialContext][2]) = 48",
            1,
        )
        .replace(
            "asyncRecoveryNode <- IndexedRecovery(initialContext, 2)",
            "asyncRecoveryNode <- IndexedRecovery(initialContext, 1)",
            1,
        )
        .replace(
            "Len(indexedAsyncState[initialContext]) = 6",
            "Len(indexedAsyncState[initialContext]) = 5",
            1,
        )
        .replace(
            "Len(indexedAsyncState[initialContext][3]) = 46",
            "Len(indexedAsyncState[initialContext][3]) = 45",
            1,
        )
        .replace(
            "Len(indexedAsyncState[initialContext][5]) = 3",
            "Len(indexedAsyncState[initialContext][5]) = 2",
            1,
        )
        .replace(
            "UNCHANGED IndexedScheduler(initialContext, 33)",
            "UNCHANGED IndexedScheduler(initialContext, 32)",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("must directly instantiate the authoritative" in error for error in errors)
    assert any("scheduler tuple mapping" in error for error in errors)
    assert any("recovery tuple mapping" in error for error in errors)
    assert any(
        "IndexedAsync must use exactly the reviewed ordered" in error
        for error in errors
    )
    assert any(
        "stale Core/scheduler/recovery/producer tuple arity" in error
        for error in errors
    )
    assert any("preserve scheduler slot 33" in error for error in errors)

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
        "VerificationAsyncProof must use exactly the reviewed ordered" in error
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
        "VerificationAsyncProof must use exactly the reviewed ordered" in error
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
        "VerificationAsyncProof must use exactly the reviewed ordered" in error
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
            "INSTANCE SumeragiV2AsyncTemporalClosureProofs",
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
            "    asyncFixedCorridorDeadlines>>",
            "    asyncHistoricalLockRestartAuthorities>>",
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
            "indexedAsyncState[initialContext][4][component]",
            "indexedAsyncState[initialContext][3][component]",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._chain_source_fidelity_errors(formal_dir)
    assert any("IndexedRecovery must equal only" in error for error in errors)

    path.write_text(
        source.replace(
            "           IndexedRecovery, IndexedProducer,\n"
            "           IndexedFixedCorridorDeadlines\n",
            "           IndexedRecovery,\n"
            "           IndexedFixedCorridorDeadlines\n",
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


@pytest.mark.parametrize(
    ("symbol", "old", "new"),
    (
        (
            "IndexedSevenFieldServeLifecycleProjectionIsExact",
            "IndexedScheduler(initialContext, 11)",
            "IndexedScheduler(initialContext, 12)",
        ),
        (
            "IndexedSevenFieldServeLifecycleProjectionIsExact",
            "IndexedScheduler(initialContext, 14)",
            "IndexedScheduler(initialContext, 13)",
        ),
        (
            "IndexedSevenFieldServeLifecycleProjectionIsExact",
            "IndexedScheduler(initialContext, 15)",
            "IndexedScheduler(initialContext, 16)",
        ),
        (
            "IndexedSevenFieldServeLifecycleProjectionIsExact",
            "IndexedScheduler(initialContext, 16)",
            "IndexedScheduler(initialContext, 15)",
        ),
        (
            "IndexedSevenFieldServeLifecycleProjectionIsExact",
            "IndexedScheduler(initialContext, 17)",
            "IndexedScheduler(initialContext, 16)",
        ),
        (
            "IndexedSevenFieldServeLifecycleProjectionIsExact",
            "IndexedScheduler(initialContext, 12)",
            "IndexedScheduler(initialContext, 11)",
        ),
        (
            "IndexedSevenFieldServeLifecycleProjectionIsExact",
            "IndexedScheduler(initialContext, 13)",
            "IndexedScheduler(initialContext, 14)",
        ),
        (
            "IndexedSevenFieldServeLifecycleProjectionIsExact",
            "                IndexedScheduler(initialContext, 15),\n",
            "",
        ),
        (
            "IndexedSevenFieldServeLifecycleProjectionIsExact",
            "           IndexedAsync!AsyncServeIngressAdmissionVars,\n",
            "",
        ),
        (
            "VerificationSevenFieldServeLifecycleProjectionIsExact",
            "VerificationScheduler(11)",
            "VerificationScheduler(12)",
        ),
        (
            "VerificationSevenFieldServeLifecycleProjectionIsExact",
            "VerificationScheduler(14)",
            "VerificationScheduler(13)",
        ),
        (
            "VerificationSevenFieldServeLifecycleProjectionIsExact",
            "VerificationScheduler(15)",
            "VerificationScheduler(16)",
        ),
        (
            "VerificationSevenFieldServeLifecycleProjectionIsExact",
            "VerificationScheduler(16)",
            "VerificationScheduler(15)",
        ),
        (
            "VerificationSevenFieldServeLifecycleProjectionIsExact",
            "VerificationScheduler(17)",
            "VerificationScheduler(16)",
        ),
        (
            "VerificationSevenFieldServeLifecycleProjectionIsExact",
            "VerificationScheduler(12)",
            "VerificationScheduler(11)",
        ),
        (
            "VerificationSevenFieldServeLifecycleProjectionIsExact",
            "VerificationScheduler(13)",
            "VerificationScheduler(14)",
        ),
        (
            "VerificationSevenFieldServeLifecycleProjectionIsExact",
            "             VerificationScheduler(15), ",
            "             ",
        ),
        (
            "VerificationSevenFieldServeLifecycleProjectionIsExact",
            "           VerificationAsyncProof!AsyncServeIngressAdmissionVars,\n",
            "",
        ),
    ),
)
def test_chain_seven_field_serve_projection_mutations_fail_closed(
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
