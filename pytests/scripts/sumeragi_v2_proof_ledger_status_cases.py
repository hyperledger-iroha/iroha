# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.


def test_checker_component_manifest_matches_lexical_execution() -> None:
    """Every reviewed component must execute exactly once from the checker."""

    source = SCRIPT.read_text(encoding="utf-8")
    tree = ast.parse(source, filename=str(SCRIPT))
    calls = []
    for node in tree.body:
        if not isinstance(node, ast.Expr) or not isinstance(node.value, ast.Call):
            continue
        call = node.value
        if not isinstance(call.func, ast.Name):
            continue
        if call.func.id != "_execute_checker_component":
            continue
        assert len(call.args) == 1 and not call.keywords
        calls.append(ast.literal_eval(call.args[0]))

    module = load_checker()
    manifest = tuple(module._CHECKER_COMPONENT_FILES)
    assert len(calls) == len(set(calls))
    assert sorted(calls) == sorted(manifest)


def test_checker_components_cannot_shadow_reviewed_definitions() -> None:
    """Reject late main definitions that silently replace component checks."""

    owners: dict[str, list[str]] = {}
    for path in checker_source_paths():
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
        for node in tree.body:
            if not isinstance(
                node,
                (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef),
            ):
                continue
            owners.setdefault(node.name, []).append(f"{path.name}:{node.lineno}")

    duplicates = {
        name: locations
        for name, locations in owners.items()
        if len(locations) != 1
    }
    assert duplicates == {}


def test_exact_decision_lifecycle_property_contracts_are_current() -> None:
    """The release checker pins the typed and stutter-closed lifecycle surface."""

    module = load_checker()
    module_name = "SumeragiV2ExactDecisionStageServiceClosureProofs"
    source = (module.FORMAL_DIR / f"{module_name}.tla").read_text(
        encoding="utf-8"
    )
    symbols = (
        "ExactDecisionRequestLifecycleRankCellClosureProperty",
        "ExactDecisionRequestLifecycleFiniteProducerEpisodeClosureProperty",
        "ExactDecisionRequestLifecycleConcreteActionOriginProperty",
        "ExactDecisionRequestLifecycleRankDescentProperty",
    )
    for symbol in symbols:
        extracted = module._top_level_operator_body(
            source,
            symbol,
            preserve_string_contents=True,
        )
        assert extracted is not None, symbol
        observed = " ".join(extracted[0].split())
        expected = module.EXACT_FIXED_PROOF_PROPERTY_OPERATOR_BODIES[
            (module_name, symbol)
        ]
        assert observed == expected, symbol


def test_exact_decision_lifecycle_contracts_reject_semantic_weakening() -> None:
    """Collapsed domains and non-stutter-closed step predicates fail closed."""

    module = load_checker()
    module_name = "SumeragiV2ExactDecisionStageServiceClosureProofs"
    source = (module.FORMAL_DIR / f"{module_name}.tla").read_text(
        encoding="utf-8"
    )
    mutations = (
        (
            "ExactDecisionRequestLifecycleRankCellClosureProperty",
            "\\A node, qc, archive, request:\n         \\A rank \\in",
            "\\A node, qc, archive, request,\n         rank \\in",
        ),
        (
            "ExactDecisionRequestLifecycleFiniteProducerEpisodeClosureProperty",
            "\\A node, qc, archive, request:\n         \\A rank \\in",
            "\\A node, qc, archive, request,\n         rank \\in",
        ),
        (
            "ExactDecisionRequestLifecycleConcreteActionOriginProperty",
            "=> [][(\\A node",
            "=> [](\\A node",
        ),
        (
            "ExactDecisionRequestLifecycleRankDescentProperty",
            "=> [][(\\A node",
            "=> [](\\A node",
        ),
    )
    ledger = module.load_ledger()
    for symbol, old, new in mutations:
        mutated = mutate_tla_operator(source, symbol, old, new)
        assert mutated != source, symbol
        errors = module._proof_obligation_architecture_errors(
            ledger["obligations"],
            {module_name: mutated},
        )
        assert any(
            f"{symbol} must equal only" in error for error in errors
        ), errors

    symbol = "ExactDecisionRequestFrozenServeBarrierPreservesTargetIngressCoalescing"
    extracted = module._top_level_theorem_body(
        source, symbol, preserve_string_contents=True
    )
    assert extracted is not None
    expected = module.EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
        (module_name, symbol)
    ]
    assert module._tla_statement_without_proof(extracted[0]) == expected
    mutated = mutate_tla_theorem(
        source,
        symbol,
        "       /\\ \\E source \\in AsyncIngressSources:\n"
        "            request \\in SequenceSet(\n"
        "              IngressLane(archive, source))'\n",
        "       /\\ request \\in SequenceSet(\n"
        "            IngressLane(archive, IngressResourceSource(request)))'\n",
    )
    assert mutated != source
    errors = module._proof_obligation_architecture_errors(
        ledger["obligations"], {module_name: mutated}
    )
    assert any(f"{symbol} must state only" in error for error in errors), errors


def test_historical_certificate_lineage_quantifiers_are_current() -> None:
    """Unbounded certificates precede the responsive-node domain explicitly."""

    module = load_checker()
    module_name = "SumeragiV2HistoricalRecoveryTemporalClosureProofs"
    source = (module.FORMAL_DIR / f"{module_name}.tla").read_text(
        encoding="utf-8"
    )
    symbols = (
        "IndexedHistoricalCertificateReceivedQcLineageInvariantAt",
        "IndexedHistoricalCertificateDecisionWalLineageInvariantAt",
    )
    for symbol in symbols:
        extracted = module._top_level_operator_body(
            source,
            symbol,
            preserve_string_contents=True,
        )
        assert extracted is not None, symbol
        expected = module.EXACT_FIXED_PROOF_PROPERTY_OPERATOR_BODIES[
            (module_name, symbol)
        ]
        assert " ".join(extracted[0].split()) == expected, symbol


def test_historical_certificate_lineage_rejects_mixed_binder_regression() -> None:
    """The reviewed TLAPS-normal binder form remains exact for both sources."""

    module = load_checker()
    module_name = "SumeragiV2HistoricalRecoveryTemporalClosureProofs"
    source = (module.FORMAL_DIR / f"{module_name}.tla").read_text(
        encoding="utf-8"
    )
    symbols = (
        "IndexedHistoricalCertificateReceivedQcLineageInvariantAt",
        "IndexedHistoricalCertificateDecisionWalLineageInvariantAt",
    )
    ledger = module.load_ledger()
    for symbol in symbols:
        mutated = mutate_tla_operator(
            source,
            symbol,
            "\\A qc:\n    \\A node \\in Responsive:",
            "\\A node \\in Responsive, qc:",
        )
        assert mutated != source, symbol
        errors = module._proof_obligation_architecture_errors(
            ledger["obligations"],
            {module_name: mutated},
        )
        assert any(
            f"{symbol} must equal only" in error for error in errors
        ), errors

    response_symbol = "IndexedHistoricalCommitResponseIdentity"
    mutated = mutate_tla_operator(
        source,
        response_symbol,
        "  /\\ response.source = request.envelope.recipient\n",
        "  /\\ response.source = IndexedAsync(initialContext)!AsyncUntrustedSource\n",
    )
    errors = module._proof_obligation_architecture_errors(
        ledger["obligations"], {module_name: mutated}
    )
    assert any(
        f"{response_symbol} must equal only" in error for error in errors
    ), errors


def test_post_retransmit_cut_continuation_quantifier_is_pinned() -> None:
    """The dependent record carrier keeps its explicit nested node binder."""

    module = load_checker()
    module_name = "SumeragiV2AsyncNetwork"
    symbol = (
        "AsyncCandidateProducerContinuationPostRetransmitCutCannotOwnRunnerTurn"
    )
    source = (module.FORMAL_DIR / f"{module_name}.tla").read_text(
        encoding="utf-8"
    )
    extracted = module._top_level_theorem_body(
        source,
        symbol,
        preserve_string_contents=True,
    )
    assert extracted is not None
    expected = module.EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
        (module_name, symbol)
    ]
    assert module._tla_statement_without_proof(extracted[0]) == expected

    mutated = mutate_tla_theorem(
        source,
        symbol,
        "\\A node \\in ValidatorIds:\n    \\A record \\in",
        "\\A node \\in ValidatorIds,\n     record \\in",
    )
    assert mutated != source
    errors = module._proof_obligation_architecture_errors(
        module.load_ledger()["obligations"],
        {module_name: mutated},
    )
    assert any(f"{symbol} must state only" in error for error in errors), errors


def test_reviewed_token_origin_quantifier_is_pinned() -> None:
    """The token carrier remains nested under its explicit validator domain."""

    module = load_checker()
    module_name = "SumeragiV2AsyncNetwork"
    symbol = "AsyncCandidateLifecycleReviewedTokenOwnsOneOrigin"
    source = (module.FORMAL_DIR / f"{module_name}.tla").read_text(
        encoding="utf-8"
    )
    extracted = module._top_level_theorem_body(
        source,
        symbol,
        preserve_string_contents=True,
    )
    assert extracted is not None
    expected = module.EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
        (module_name, symbol)
    ]
    assert module._tla_statement_without_proof(extracted[0]) == expected

    mutated = mutate_tla_theorem(
        source,
        symbol,
        "\\A state, left, right:\n"
        "    \\A node \\in ValidatorIds:\n"
        "      \\A token \\in",
        "\\A state, left, right:\n"
        "    \\A node \\in ValidatorIds,\n"
        "       token \\in",
    )
    assert mutated != source
    errors = module._proof_obligation_architecture_errors(
        module.load_ledger()["obligations"],
        {module_name: mutated},
    )
    assert any(f"{symbol} must state only" in error for error in errors), errors


def test_retransmit_lifecycle_timeout_transfer_endpoint_is_pinned() -> None:
    """Timeout-origin transfer is an exact retransmit lifecycle endpoint."""

    module = load_checker()
    module_name = "SumeragiV2AsyncNetwork"
    source = (module.FORMAL_DIR / f"{module_name}.tla").read_text(
        encoding="utf-8"
    )
    mutations = (
        (
            "AsyncRetransmitLifecycleOwnerAndPhysicalCutPersistUntilEndpoint",
            "    /\\ ~AsyncTimeoutLifecycleTransfersThisStep(node)\n",
        ),
        (
            "AsyncRetransmitLifecycleOwnerAndPhysicalCutClearAtEndpoint",
            "            \\/ AsyncTimeoutLifecycleTransfersThisStep(node)\n",
        ),
    )
    ledger = module.load_ledger()
    for symbol, required_endpoint in mutations:
        extracted = module._top_level_theorem_body(
            source,
            symbol,
            preserve_string_contents=True,
        )
        assert extracted is not None, symbol
        expected = module.EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
            (module_name, symbol)
        ]
        assert module._tla_statement_without_proof(extracted[0]) == expected, symbol

        mutated = mutate_tla_theorem(source, symbol, required_endpoint, "")
        assert mutated != source, symbol
        errors = module._proof_obligation_architecture_errors(
            ledger["obligations"],
            {module_name: mutated},
        )
        assert any(f"{symbol} must state only" in error for error in errors), errors


def test_timeout_physical_control_retransmission_typed_item_binder_is_pinned() -> None:
    """The redundant item carrier remains explicit at the packet handoff."""

    module = load_checker()
    module_name = "SumeragiV2TimeoutViewProgressProofs"
    symbol = "TimeoutPhysicalControlRetransmissionCreatesExactPacket"
    source = (module.FORMAL_DIR / f"{module_name}.tla").read_text(
        encoding="utf-8"
    )
    extracted = module._top_level_theorem_body(
        source, symbol, preserve_string_contents=True
    )
    assert extracted is not None
    expected = module.EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
        (module_name, symbol)
    ]
    assert module._tla_statement_without_proof(extracted[0]) == expected

    mutated = mutate_tla_theorem(
        source,
        symbol,
        "\\A node \\in ValidatorIds, item \\in AsyncNetworkItems:",
        "\\A node \\in ValidatorIds, item:",
    )
    assert mutated != source
    errors = module._proof_obligation_architecture_errors(
        module.load_ledger()["obligations"], {module_name: mutated}
    )
    assert any(f"{symbol} must state only" in error for error in errors), errors

    symbol = "TimeoutPhysicalControlPacketAdmissionPreservesExactHandoff"
    extracted = module._top_level_theorem_body(
        source, symbol, preserve_string_contents=True
    )
    assert extracted is not None
    expected = module.EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[
        (module_name, symbol)
    ]
    assert module._tla_statement_without_proof(extracted[0]) == expected
    mutated = mutate_tla_theorem(
        source,
        symbol,
        "packet.authenticatedSource",
        "item.source",
    )
    assert mutated != source
    errors = module._proof_obligation_architecture_errors(
        module.load_ledger()["obligations"], {module_name: mutated}
    )
    assert any(f"{symbol} must state only" in error for error in errors), errors


def copy_persistent_recovery_cut_fixture(tmp_path: Path, module) -> Path:
    """Copy the exact Rust and TLA sources reviewed by the recovery-cut checker."""

    repo_root = tmp_path / "repo"
    relatives = (
        "crates/iroha_core/src/sumeragi/v2.rs",
        "crates/iroha_core/src/sumeragi/v2_leader_wire_consumer.rs",
        "crates/iroha_core/src/sumeragi/v2_core/wal.rs",
        "crates/iroha_core/src/sumeragi/v2_core/types.rs",
        "crates/iroha_core/src/sumeragi/v2_runtime.rs",
        "crates/iroha_core/src/sumeragi/v2_effects.rs",
        "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs",
        "crates/iroha_core/src/sumeragi/mod.rs",
        "crates/iroha_core/src/sumeragi/v2_worker.rs",
        "formal/sumeragi_v2/SumeragiV2AsyncNetwork.tla",
    )
    for relative in relatives:
        destination = repo_root / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(module.ROOT_DIR / relative, destination)
    copy_reviewed_rust_include_components(repo_root)
    return repo_root


def _check_effect_capacity_terminal_wrapper_attribute_is_exact(
    module,
    tmp_path: Path,
    replacement: str,
    expected_error: str,
) -> None:
    """The reviewed dead-code allowance cannot be dropped or replaced by a gate."""

    repo_root, _formal_dir = copy_effect_capacity_mutation_fixture(tmp_path, module)
    runtime_path = repo_root / "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    mutate_source_once(
        runtime_path,
        "    #[allow(dead_code)]\n"
        "    pub(crate) fn commit_body_pipeline_candidate_terminal(\n",
        replacement
        + "    pub(crate) fn commit_body_pipeline_candidate_terminal(\n",
    )

    errors = module._effect_capacity_production_source_fidelity_errors(repo_root)
    assert any(
        "checked single body-terminal authority commit wrapper" in error
        and expected_error in error
        and "exact reviewed token digest" not in error
        for error in errors
    ), errors


def _check_certified_fence_capacity_regression_survives_digest_refresh(
    module,
    tmp_path: Path,
    item_name: str,
    old: str,
    new: str,
    expected_error: str,
) -> None:
    """Refreshing either regression seal cannot hide its capacity assertion."""

    runtime_parent = tmp_path / "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    runtime_parent.parent.mkdir(parents=True)
    shutil.copy2(
        module.ROOT_DIR / runtime_parent.relative_to(tmp_path), runtime_parent
    )
    copy_reviewed_rust_include_components(tmp_path)
    runtime_path = (
        runtime_parent.parent
        / "tests/v2_runtime_unsealed_02_owner_retirement_and_fairness.rs"
    )
    mutate_rust_item_source(module, runtime_path, item_name, old, new)
    items = module.rust_items(runtime_path.read_text(encoding="utf-8"), item_name)
    assert len(items) == 1, item_name
    seal_key = f"test::{item_name}"
    expected_sha256 = module._PRODUCTION_RUNTIME_CERTIFIED_FENCE_CAPACITY_ITEM_SHA256[
        seal_key
    ]
    module._PRODUCTION_RUNTIME_CERTIFIED_FENCE_CAPACITY_ITEM_SHA256[seal_key] = (
        module._rust_item_token_sha256(items[0])
    )

    errors = module._runtime_certified_fence_capacity_source_fidelity_errors(
        tmp_path
    )
    module._PRODUCTION_RUNTIME_CERTIFIED_FENCE_CAPACITY_ITEM_SHA256[seal_key] = (
        expected_sha256
    )
    assert any(
        expected_error in error and "exact reviewed token digest" not in error
        for error in errors
    ), errors


def _check_persistent_recovery_cut_binds_capacity_constructor(
    module,
    tmp_path: Path,
) -> None:
    """Restart ordering must live in the capacity-bound production constructor."""

    repo_root = copy_persistent_recovery_cut_fixture(tmp_path, module)
    path = repo_root / "crates/iroha_core/src/sumeragi/v2.rs"
    real_sequence = """        adapter.reconcile_restored_reserved_producer_frontier()?;
        adapter.reclaim_serviced_candidates()?;
        let replay_tag = adapter.reducer.current_tag();"""
    mutate_rust_item_source_in_context(
        module,
        path,
        "open_with_aggregator_and_publication_with_capacity",
        (("impl", "SumeragiV2Adapter"),),
        real_sequence,
        "        let replay_tag = adapter.reducer.current_tag();",
    )
    mutate_rust_item_source_in_context(
        module,
        path,
        "open_with_aggregator",
        (("impl", "SumeragiV2Adapter"),),
        "Self::open_with_aggregator_and_publication(\n",
        real_sequence + "\n        Self::open_with_aggregator_and_publication(\n",
    )

    errors = module._persistent_recovery_cut_source_fidelity_errors(repo_root)

    assert any(
        "restart frontier pruning must precede runtime replay and dormant capacity installation"
        in error
        for error in errors
    ), errors


def _check_persistent_recovery_cut_requires_concrete_runtime_impl(
    module,
    tmp_path: Path,
) -> None:
    """Persistent-body methods cannot be satisfied by a generic lookalike impl."""

    repo_root = copy_persistent_recovery_cut_fixture(tmp_path, module)
    path = repo_root / "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    mutate_source_once(
        path,
        "impl SerializedV2Runtime<SumeragiV2Adapter> {\n"
        "    /// Stage the deferred pending-Kura validation",
        "impl SerializedV2Runtime {\n"
        "    /// Stage the deferred pending-Kura validation",
    )

    errors = module._persistent_recovery_cut_source_fidelity_errors(repo_root)

    for item_name in (
        "body_available_has_persistent_producer",
        "rebind_body_available",
        "retire_restored_body_fetch_parent",
    ):
        assert any(
            item_name in error
            and "SerializedV2Runtime', '<', 'SumeragiV2Adapter', '>'" in error
            and "found 0" in error
            for error in errors
        ), (item_name, errors)


def _check_causal_test_wrapper_attributes(module, tmp_path: Path) -> None:
    """Test-only causal wrappers cannot be cfg-gated into production."""

    formal_dir = copy_async_source_fidelity_fixture(
        tmp_path, module, "SumeragiV2AsyncNetwork.tla"
    )
    for relative, declaration, description in (
        (
            "crates/iroha_core/src/sumeragi/v2.rs",
            "pub(crate) fn drain_deferred_with_evidence(\n",
            "single-transition adapter deferred ownership dispatcher",
        ),
        (
            "crates/iroha_core/src/sumeragi/v2_runtime.rs",
            "fn minimum_active_lifecycle_ordinal_for_deferred(\n",
            "target-relative deferred lifecycle minimum wrapper",
        ),
    ):
        path = tmp_path / relative
        canonical = path.read_text(encoding="utf-8")
        old = "    #[cfg(test)]\n    " + declaration
        assert canonical.count(old) == 1, relative
        path.write_text(
            canonical.replace(
                old, "    #[cfg_attr(not(test), allow(dead_code))]\n    " + declaration, 1
            ),
            encoding="utf-8",
        )
        errors = module._production_causal_fifo_source_fidelity_errors(formal_dir)
        assert any(
            description in error
            and "may not be disabled or replaced" in error
            and "exact reviewed token digest" not in error
            for error in errors
        ), (description, errors)
        path.write_text(canonical, encoding="utf-8")


def test_leader_wire_recovery_cut_uses_shared_physical_highwater(
    tmp_path: Path,
) -> None:
    """The recovery cut preserves the shared physical ingress high-water."""

    module = load_checker()
    for index, (replacement, expected_error) in enumerate(
        (
            ("", "must have exact reviewed attributes"),
            (
                "    #[cfg(test)]\n",
                "may not be disabled or replaced through unreviewed cfg/cfg_attr attributes",
            ),
        )
    ):
        _check_effect_capacity_terminal_wrapper_attribute_is_exact(
            module, tmp_path / f"terminal-attr-{index}", replacement, expected_error
        )
    for index, (item_name, old, new, expected_error) in enumerate(
        (
            (
                "retiring_the_sole_certificate_does_not_fake_completion_headroom",
                """    assert_eq!(
        runtime.remaining_completion_capacity(),
        0,
        "retiring the sole certificate removes its credit as well as its physical owner"
    );""",
                """    assert_eq!(
        runtime.remaining_completion_capacity(),
        1,
        "retiring the sole certificate removes its credit as well as its physical owner"
    );""",
                "sole-certificate retirement must not invent Completion headroom",
            ),
            (
                "unpublished_body_replacement_cannot_overbook_the_certified_slot",
                """    assert_eq!(
        runtime.queued_commands(),
        2,
        "the conflicting proposal must retire before the reservation becomes live"
    );""",
                """    assert_eq!(
        runtime.queued_commands(),
        3,
        "the conflicting proposal must retire before the reservation becomes live"
    );""",
                "unpublished BodyAvailable must atomically replace its conflict",
            ),
        )
    ):
        _check_certified_fence_capacity_regression_survives_digest_refresh(
            module,
            tmp_path / f"certified-fence-{index}",
            item_name,
            old,
            new,
            expected_error,
        )
    _check_persistent_recovery_cut_binds_capacity_constructor(
        module, tmp_path / "capacity-constructor"
    )
    _check_persistent_recovery_cut_requires_concrete_runtime_impl(
        module, tmp_path / "concrete-runtime"
    )
    _check_causal_test_wrapper_attributes(module, tmp_path / "causal-attrs")
    repo_root = copy_persistent_recovery_cut_fixture(
        tmp_path / "leader-wire", module
    )

    marker = "leader-wire recovery-cut high-water theorem"
    errors = module._persistent_recovery_cut_source_fidelity_errors(repo_root)
    assert not any(marker in error for error in errors), errors

    path = repo_root / "formal/sumeragi_v2/SumeragiV2AsyncNetwork.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            "LeaderWireRecoveryCutRetainsOrdinalHighwaters",
            "AsyncNextIngressPhysicalOrdinal(node)' =",
            "AsyncNextLeaderWireIngressOrdinal(node)' =",
        ),
        encoding="utf-8",
    )
    errors = module._persistent_recovery_cut_source_fidelity_errors(repo_root)
    assert any(
        marker in error
        and "AsyncNextIngressPhysicalOrdinal(node)' =" in error
        for error in errors
    ), errors


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


def test_tlc_runner_cannot_claim_or_mutate_proof_completion(
    tmp_path: Path,
) -> None:
    module = load_checker()
    runner = (ROOT_DIR / "scripts" / "formal" / "run_sumeragi_v2_tlc.sh").read_text()

    assert module._FINITE_TLC_OPERATOR_OVERRIDE_CONFIGS == (
        "quorum_count.cfg",
        "quorum_stake.cfg",
        "safety_count.cfg",
        "safety_stake.cfg",
        "chain_epoch.cfg",
        "liveness.cfg",
        "resume_locked_commit_witness.cfg",
    )
    assert module._FINITE_TLC_OPERATOR_OVERRIDES == (
        "  Generations <- FiniteGenerations\n",
        "  GenerationCanIncrement <- FiniteGenerationCanIncrement\n",
        "  ModelConfiguration <- FiniteModelConfiguration\n",
        "  ByzantineProposalJustificationDomain <- FiniteByzantineProposalJustificationDomain\n",
    )
    for cfg_name in module._FINITE_TLC_OPERATOR_OVERRIDE_CONFIGS:
        source = (module.FORMAL_DIR / cfg_name).read_text(encoding="utf-8")
        for override in module._FINITE_TLC_OPERATOR_OVERRIDES:
            assert source.count(override) == 1, (cfg_name, override)
        max_view = re.findall(r"(?m)^  MaxView = ([0-9]+)$", source)
        max_generation = re.findall(
            r"(?m)^  MaxGeneration = ([0-9]+)$", source
        )
        assert len(max_view) == len(max_generation) == 1, cfg_name
        assert int(max_generation[0]) >= int(max_view[0]), cfg_name
    assert module._FINITE_TLC_ASYNC_OVERRIDES == (
        "  AsyncIngressPhysicalOrdinalMaximum <- FiniteAsyncIngressPhysicalOrdinalMaximum\n",
        "  AsyncNetworkItems <- FiniteAsyncNetworkItems\n",
    )
    assert module._BYZANTINE_PROPOSAL_DOMAIN_PROOF_BINDINGS == {
        "SumeragiV2Proofs.tla": (
            ("NextLockFootprintClassification", "Next"),
        ),
        "SumeragiV2AsyncRecoveryVoteEpochProofs.tla": (
            ("CoreNextSerializedBusyActionClassification", "Next"),
            ("AsyncFaultStepLeavesOutstandingTags", "AsyncFaultStep"),
        ),
        "SumeragiV2AsyncRecoveryProgressWitnessProofs.tla": (
            ("CoreNextPreservesDecisionTimeoutFrontier", "Next"),
        ),
        "SumeragiV2AsyncFairServiceProofs.tla": (
            ("AsyncFaultStepLeavesDiscoveryClock", "AsyncFaultStep"),
        ),
        "SumeragiV2AsyncProgressOwnershipProofs.tla": (
            ("AsyncFaultPreservesProgressOwnership", "AsyncFaultStep"),
        ),
        "SumeragiV2AsyncTimeoutKernelProofs.tla": (
            ("CoreNextKeepsPrepareQcsMonotone", "Next"),
            ("AsyncFaultStepKeepsTimeoutPool", "AsyncFaultStep"),
            ("AsyncFaultStepPreservesSchedulerType", "AsyncFaultStep"),
        ),
    }
    liveness_source = (module.FORMAL_DIR / "liveness.cfg").read_text(encoding="utf-8")
    for override in module._FINITE_TLC_ASYNC_OVERRIDES:
        assert liveness_source.count(override) == 1

    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    for cfg_name in module.REQUIRED_TLC_CONFIGS:
        shutil.copyfile(module.FORMAL_DIR / cfg_name, formal_dir / cfg_name)
    assert module._finite_tlc_configuration_errors(formal_dir) == []

    pair_override = module._FINITE_TLC_OPERATOR_OVERRIDES[-1]
    mutations = (
        (
            "safety_count.cfg",
            pair_override,
            "",
            "canonical operator override",
        ),
        (
            "safety_count.cfg",
            pair_override,
            pair_override + pair_override,
            "found 2",
        ),
        (
            "safety_count.cfg",
            pair_override,
            pair_override.replace("  Byzantine", "   Byzantine", 1),
            "canonical operator override",
        ),
        (
            "safety_count.cfg",
            pair_override,
            "(* " + pair_override.rstrip("\n") + " *)\n",
            "canonical operator override",
        ),
        (
            "safety_count.cfg",
            pair_override,
            pair_override
            + pair_override.replace("  Byzantine", "   Byzantine", 1),
            "ByzantineProposalJustificationDomain finite override inventory",
        ),
        (
            "safety_count.cfg",
            "  MaxGeneration = 3\n",
            "  MaxGeneration = 2\n",
            "must cover MaxView 3",
        ),
        (
            "safety_count.cfg",
            "  MaxGeneration = 3\n",
            "(*  MaxGeneration = 3 *)\n",
            "must declare exactly one decimal MaxView and MaxGeneration",
        ),
        (
            "liveness.cfg",
            module._FINITE_TLC_ASYNC_OVERRIDES[0],
            "",
            "finite async operator override must occur exactly 1",
        ),
        (
            "liveness.cfg",
            module._FINITE_TLC_ASYNC_OVERRIDES[1],
            "",
            "finite async operator override must occur exactly 1",
        ),
        (
            "safety_count.cfg",
            "CHECK_DEADLOCK FALSE\n",
            "CHECK_DEADLOCK FALSE\n"
            + module._FINITE_TLC_ASYNC_OVERRIDES[1],
            "finite async operator override must occur exactly 0",
        ),
        (
            "effective_lock_acquisition.cfg",
            "CHECK_DEADLOCK FALSE\n",
            "CHECK_DEADLOCK FALSE\n" + pair_override,
            "ByzantineProposalJustificationDomain finite override inventory",
        ),
    )
    for cfg_name, needle, replacement, expected_error in mutations:
        path = formal_dir / cfg_name
        canonical = path.read_text(encoding="utf-8")
        assert canonical.count(needle) == 1, (cfg_name, needle)
        path.write_text(canonical.replace(needle, replacement, 1), encoding="utf-8")
        errors = module._finite_tlc_configuration_errors(formal_dir)
        assert any(expected_error in error for error in errors), errors
        path.write_text(canonical, encoding="utf-8")

    assert "COUNTEREXAMPLE SEARCH ONLY" in runner
    assert "no proof status was changed" in runner
    assert "proof_coverage.json" not in runner
    assert "machine_checked_completion" not in runner
    assert "SumeragiV2ChainEpoch.tla" in runner
    assert "SumeragiV2AsyncNetwork.tla" in runner
    assert runner.count("SumeragiV2Inductive.tla") == 2
    assert " SumeragiV2.tla" not in runner
    assert not (module.FORMAL_DIR / "SumeragiV2.tla").exists()
    assert "SumeragiV2" not in module.REQUIRED_MODEL_MODULES
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

    retired_entry = formal_dir / "SumeragiV2.tla"
    retired_entry.write_text(
        "---- MODULE SumeragiV2 ----\nEXTENDS SumeragiV2Inductive\n",
        encoding="utf-8",
    )
    errors = module._resume_vote_witness_errors(formal_dir)
    assert any("retired compatibility model entry point" in error for error in errors)
    retired_entry.unlink()

    witness = formal_dir / "SumeragiV2ResumeVoteWitness.tla"
    canonical_witness = witness.read_text(encoding="utf-8")
    witness.write_text(
        canonical_witness.replace(
            "EXTENDS SumeragiV2Inductive",
            "EXTENDS SumeragiV2",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._resume_vote_witness_errors(formal_dir)
    assert any("extend the canonical inductive model directly" in error for error in errors)
    witness.write_text(canonical_witness, encoding="utf-8")

    for canonical, replacement, expected_error in (
        (
            "ResumeWitnessRosters == <<<<0>>>>",
            "ResumeWitnessRosters == <<<<0, 1>>>>",
            "carrier ResumeWitnessRosters must equal only",
        ),
        (
            "ResumeWitnessPowers == <<<<1>>>>",
            "ResumeWitnessPowers == <<<<1, 1>>>>",
            "carrier ResumeWitnessPowers must equal only",
        ),
    ):
        witness.write_text(
            canonical_witness.replace(canonical, replacement, 1),
            encoding="utf-8",
        )
        errors = module._resume_vote_witness_errors(formal_dir)
        assert any(expected_error in error for error in errors)
    witness.write_text(canonical_witness, encoding="utf-8")

    cfg = formal_dir / "resume_locked_commit_witness.cfg"
    canonical_cfg = cfg.read_text(encoding="utf-8")
    for canonical, replacement, expected_error in (
        (
            "  EpochRosters <- ResumeWitnessRosters",
            "  EpochRosters <- CountRostersOneEpoch",
            "substitution EpochRosters must equal the exact local carrier",
        ),
        (
            "  EpochPowers <- ResumeWitnessPowers",
            "  EpochPowers <- CountPowersOneEpoch",
            "substitution EpochPowers must equal the exact local carrier",
        ),
    ):
        cfg.write_text(
            canonical_cfg.replace(canonical, replacement, 1),
            encoding="utf-8",
        )
        errors = module._resume_vote_witness_errors(formal_dir)
        assert any(expected_error in error for error in errors)
    cfg.write_text(canonical_cfg, encoding="utf-8")

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


@pytest.fixture(scope="module")
def worker_actual_wal_enter_view_sources():
    """Read the real reviewed include closures once for the fixture-owner controls."""
    module = load_checker()
    paths = []
    sources = []
    errors: list[str] = []
    for name in ("v2.rs", "v2_worker.rs"):
        path, source = module._read_reviewed_rust_source(
            ROOT_DIR, "crates/iroha_core/src/sumeragi/" + name,
            errors, "worker actual-WAL regression fixture",
        )
        paths.append(path)
        sources.append(source)
    assert not errors, errors
    assert module._worker_actual_wal_enter_view_fixture_errors(*sources, *paths) == []
    return module, tuple(sources), tuple(paths)


def test_worker_actual_wal_enter_view_fixture_accepts_current_source(
    worker_actual_wal_enter_view_sources,
) -> None:
    module, sources, paths = worker_actual_wal_enter_view_sources
    assert module._worker_actual_wal_enter_view_fixture_errors(*sources, *paths) == []


@pytest.mark.parametrize(
    ("owner", "name", "old", "new"),
    (
        (0, "worker_view_adapter_with_replayed_commit", "WalRecordV2::LockAndCommit", "WalRecordV2::ObservePrepare"),
        (0, "worker_view_adapter_with_replayed_commit", ".append(&payload)", ".skip_append(&payload)"),
        (0, "worker_view_adapter_with_replayed_commit", "drop(adapter);", "let _ = &adapter;"),
        (0, "worker_view_adapter_with_replayed_commit", "if replayed == &vote", "if true"),
        (0, "worker_view_adapter_with_replayed_commit", "durable.commit_intent_for_lock(locked).is_some()", "true"),
        (1, "new", "service.context.clone()", "fixture().0.context.clone()"),
        (1, "new", "adapter.leader_wire_recovery_authority()", "adapter.synthetic_recovery_authority()"),
        (1, "stage_timeout", ".authenticate(wire::ConsensusMessageV2::new(", ".trust_unverified(wire::ConsensusMessageV2::new("),
        (1, "stage_timeout", ".receive_authenticated(authenticated)", ".skip_wal_persistence(authenticated)"),
        (1, "stage_timeout", "(prepare.proposal_round, prepare.subject)", "(prepare.round, prepare.subject)"),
        (1, "publish", "Some(authority)", "Some(service.leader_wire_recovery_authority)"),
        (1, "worker_view_prepare_certificate", "signers: vec![0, 1, 2]", "signers: vec![0, 1, 1]"),
        (1, "worker_view_timeout_certificate", "worker_view_quorum_signature(keys, &preimage)", "Vec::new()"),
        (1, "worker_view_quorum_signature", "keys[..3]", "keys[..2]"),
        (1, "entered_view_publishes_the_exact_protected_commit_vote_cut", "Some((prepare, commit_intent))", "None"),
        (1, "entered_view_publishes_the_exact_protected_commit_vote_cut", "wal.publish(&mut service);", ""),
        (1, "entered_view_publishes_the_exact_protected_commit_vote_cut", ".entered_view(next, certificate, protected_lock)", ".entered_view(EventTag::new(next.height(), next.view(), next.generation()), certificate, protected_lock)"),
    ),
)
def test_worker_actual_wal_enter_view_fixture_rejects_weakened_owners(
    worker_actual_wal_enter_view_sources, owner: int, name: str, old: str, new: str,
) -> None:
    module, originals, paths = worker_actual_wal_enter_view_sources
    sources = list(originals)
    items = module.rust_items(sources[owner], name)
    if name in ("new", "publish"):
        items = tuple(item for item in items if item.brace_context[-1] == ("impl", "WorkerViewWalFixture"))
    assert len(items) == 1
    item = items[0]
    # Normalized Rust tokens let source formatting change without weakening the mutation.
    tokens = module.rust_code_tokens(item.source)
    old_tokens = module.rust_code_tokens(old)
    positions = [i for i in range(len(tokens) - len(old_tokens) + 1)
                 if tokens[i:i + len(old_tokens)] == old_tokens]
    assert len(positions) == 1, (name, old)
    index = positions[0]
    replacement = " ".join(tokens[:index] + module.rust_code_tokens(new) + tokens[index + len(old_tokens):])
    sources[owner] = sources[owner].replace(item.source, replacement, 1)
    errors = module._worker_actual_wal_enter_view_fixture_errors(*sources, *paths)
    assert any("worker actual-WAL fixture " + name in error for error in errors), errors


@pytest.fixture(scope="module")
def persistent_recovery_canonical_items():
    """Extract current real owners once; every mutation starts from this valid baseline."""
    module = load_checker()
    paths, sources, errors = {}, {}, []
    for owner, name in {
        "adapter": "v2.rs", "consumer": "v2_leader_wire_consumer.rs",
        "core_wal": "v2_core/wal.rs", "core_types": "v2_core/types.rs",
        "effects": "v2_effects.rs", "store": "serviced_candidate_store.rs",
        "ingress": "mod.rs", "worker": "v2_worker.rs",
    }.items():
        paths[owner], sources[owner] = module._read_reviewed_rust_source(
            ROOT_DIR, "crates/iroha_core/src/sumeragi/" + name,
            errors, "canonical persistent recovery-cut source",
        )
    assert not errors, errors
    items = module._persistent_recovery_cut_canonical_items(paths, sources, errors)
    assert not errors, errors
    assert module._persistent_recovery_cut_canonical_item_errors(paths, items) == []
    return module, paths, items


def test_persistent_recovery_canonical_owners_accept_current_source(
    persistent_recovery_canonical_items,
) -> None:
    module, paths, items = persistent_recovery_canonical_items
    assert module._persistent_recovery_cut_canonical_item_errors(paths, items) == []


@pytest.mark.parametrize(
    ("key", "old", "new"),
    (
        ("factory", "pub(super)", "pub(crate)"),
        ("factory", "adapter.ensure_ingress()?;", ""),
        ("factory", "adapter.reducer.current_tag()", "reducer::EventTag::new(1, 0, reducer::Generation::INITIAL)"),
        ("factory", "certificate.proposal_round()", "certificate.round()"),
        ("factory", "durable.commit_intent_for_lock(locked).is_some()", "true"),
        ("factory", "adapter.registry.execution_commitment(locked.round(), locked.subject())?", "wire::ExecutionCommitment::default()"),
        ("factory", "wal_id: durable.last_id()", "wal_id: reducer::PersistenceId::ZERO"),
        ("factory", "decision_durable: durable.decision().is_some()", "decision_durable: false"),
        ("geometry", "self.owner == owner", "true"),
        ("geometry", "self.context_id == context_id", "true"),
        ("geometry", "self.height == height", "true"),
        ("monotonicity", "self.wal_id >= previous.wal_id", "true"),
        ("monotonicity", "self.consumer_tag.strictly_advances(previous.consumer_tag)", "true"),
        ("monotonicity", "!previous.decision_durable || self.decision_durable", "true"),
        ("monotonicity", "self.highest_prepare_view >= previous.highest_prepare_view", "true"),
        ("monotonicity", "(Some(_), None) => false", "(Some(_), None) => true"),
        ("monotonicity", "new.0.view > old.0.view", "new.0.view >= old.0.view"),
        ("statement", "(proposal_round, subject, *execution_commitment)", "(proposal_round, subject)"),
        ("protected_commit", "identity.context_id == round.context_id", "true"),
        ("protected_commit", "identity.height == round.height", "true"),
        ("protected_commit", "identity.view == round.view", "true"),
        ("protected_commit", "identity.subject_hash == Hash::new(subject.encode())", "true"),
        ("protected_commit", "identity.vote_statement_hash == self.protected_commit_statement", "true"),
        ("protected_commit", "self.protected_commit_statement.is_some()", "true"),
        ("admission", "if self.decision_durable", "if false"),
        ("admission", "view <= current_view", "true"),
        ("admission", "view.checked_add(1).is_some()", "true"),
        ("admission", "installed_same_round: self.installed_timeout_view == Some(view)", "installed_same_round: true"),
        ("payload_admission", "vote.round == vote.proposal_round", "true"),
        ("retirement", "&& !self.admits_ingress_identity(&token.identity)", "&& true"),
        ("rearm", "self.consumer_tag.strictly_advances(consumed_by)", "true"),
        ("rearm", "self.admits_ingress_identity(&token.identity)", "true"),
        ("wal_apply", "next.apply_in_place(context, local_validator, entry)?; *self = next;", "*self = next.clone(); next.apply_in_place(context, local_validator, entry)?;"),
        ("wal_locks", "validate_qc(context, prepare, Phase::Prepare)?;", ""),
        ("wal_locks", "Self::validate_local_vote(context, local_validator, *vote, Phase::Commit)?;", ""),
        ("wal_locks", "vote.subject() != prepare.subject()", "false"),
        ("wal_locks", "insert_unique_vote(&mut self.commit_intents, *vote)?;", ""),
        ("wal_locks", "certificate.validate(context).map_err(|_| ReplayError::InvalidCertificate)?;", ""),
        ("wal_vote", "Some(vote.signer()) != local_validator", "false"),
        ("wal_commit_intent", "vote.proposal_round() == round", "true"),
        ("wal_commit_intent", "vote.subject() == locked.subject()", "true"),
        ("qc_geometry", "self.reference.proposal_round != self.reference.round", "false"),
        ("qc_geometry", "Quorum::require(context, &signers)", "Quorum::calculate(context, &signers)"),
        ("tc_geometry", "certificate.round().view() > self.round.view()", "false"),
        ("tc_geometry", "certificate.validate(context)?;", ""),
        ("adapter_factory", "LeaderWireRecoveryAuthority::from_adapter(self)", "LeaderWireRecoveryAuthority::from_replayed_adapter(self)"),
        ("store", "retiring != *expected_retiring_slots", "!retiring.is_subset(expected_retiring_slots)"),
        ("store", "LeaderWireLifecycleStatus::Dormant | LeaderWireLifecycleStatus::VolatileTerminal", "LeaderWireLifecycleStatus::Dormant | LeaderWireLifecycleStatus::VolatileTerminal | LeaderWireLifecycleStatus::Ingress"),
        ("store", "next.rearms(&record.token, *tag)", "true"),
        ("store", "!retiring.is_empty() || !rearming.is_empty()", "!retiring.is_empty()"),
        ("store", "*state = previous;", ""),
        ("store", "Ok(rearming)", "state.records.clear(); Ok(rearming)"),
        ("store", "Ok(rearming)", "state.runtime_consumer_epochs.clear(); Ok(rearming)"),
        ("mirror", "Ok(retiring.len())", "state.leader_wire_lifecycles.clear(); Ok(retiring.len())"),
        ("store", "state.replay_dormant.insert(slot.clone());", "state.records.get_mut(slot).unwrap().token.lifecycle_ordinal = 0; state.replay_dormant.insert(slot.clone());"),
        ("mirror", "gate.advance_recovery_cut(next, &retiring)?", "gate.advance_recovery_cut(next, &retiring).unwrap_or_default()"),
        ("mirror", "record.ingress_predecessors.clear();", "record.token.lifecycle_ordinal = 0; record.ingress_predecessors.clear();"),
        ("runtime_factory", "self.driver().leader_wire_recovery_authority()", "self.synthetic_authority()"),
        ("executor_publish", "decided_subject, authority", "decided_subject, None"),
        ("service_publish", "self.leader_wire_ingress.advance_leader_wire_recovery_cut(next)?; self.leader_wire_recovery_authority = next;", "self.leader_wire_recovery_authority = next; self.leader_wire_ingress.advance_leader_wire_recovery_cut(next)?;"),
        ("service_publish", "self.leader_wire_ingress.advance_leader_wire_recovery_cut(next)?;", "let _ = self.leader_wire_ingress.advance_leader_wire_recovery_cut(next);"),
        ("enter", "tag != self.leader_wire_recovery_authority.consumer_tag()", "tag.view() != self.leader_wire_recovery_authority.consumer_tag().view()"),
    ),
)
def test_persistent_recovery_canonical_rejects_weakened_predicates(
    persistent_recovery_canonical_items, key: str, old: str, new: str,
) -> None:
    module, paths, originals = persistent_recovery_canonical_items
    items = dict(originals)
    item = items[key]
    tokens = module.rust_code_tokens(item.source)
    old_tokens = module.rust_code_tokens(old)
    positions = module._token_sequence_positions(tokens, old_tokens)
    assert len(positions) == 1, (key, old, positions)
    position = positions[0]
    source = " ".join(tokens[:position] + module.rust_code_tokens(new) + tokens[position + len(old_tokens):])
    items[key] = replace(item, source=source, body=source[source.index("{") + 1:source.rfind("}")],
                         structural_source=module.mask_rust_comments_and_literals(source))
    errors = module._persistent_recovery_cut_canonical_item_errors(paths, items)
    assert any("canonical recovery-cut " + key in error for error in errors), errors


@pytest.mark.parametrize("key", ("publish_apply", "publish_pre_timeout", "publish_pacemaker", "publish_capacity", "publish_step", "publish_recovery"))
@pytest.mark.parametrize("mutation", ("omit", "before_wal", "after_consume", "duplicate"))
def test_persistent_recovery_canonical_rejects_publication_order_drift(
    persistent_recovery_canonical_items, key: str, mutation: str,
) -> None:
    module, paths, originals = persistent_recovery_canonical_items
    items = dict(originals)
    item = items[key]
    tokens = list(module.rust_code_tokens(item.source))
    publish = module.rust_code_tokens("if let Err(error) = self.finish_runtime_step_reconciliation(services) { return Err(self.close(error, services)); }")
    positions = module._token_sequence_positions(tuple(tokens), publish)
    assert len(positions) == 1
    position = positions[0]
    del tokens[position:position + len(publish)]
    if mutation == "before_wal":
        wal = module._token_sequence_positions(tuple(tokens), module.rust_code_tokens("wal_step.complete();"))
        assert len(wal) == 1
        tokens[wal[0]:wal[0]] = publish
    elif mutation == "after_consume":
        # All original predicates remain present, but publication is causally late.
        tokens[-1:-1] = publish
    elif mutation == "duplicate":
        tokens[position:position] = publish + publish
    source = " ".join(tokens)
    items[key] = replace(item, source=source, body=source[source.index("{") + 1:source.rfind("}")],
                         structural_source=module.mask_rust_comments_and_literals(source))
    errors = module._persistent_recovery_cut_canonical_item_errors(paths, items)
    assert any("canonical recovery-cut " + key in error for error in errors), errors


@pytest.mark.parametrize("mutation", ("cfg", "cfg_attr", "wrong_path", "public", "duplicate", "comment", "alias", "inner_cfg"))
def test_persistent_recovery_canonical_rejects_module_edge_substitution(
    persistent_recovery_canonical_items, mutation: str,
) -> None:
    module, paths, originals = persistent_recovery_canonical_items
    items = dict(originals)
    edge, export = items["module_statements"]
    source = edge.source
    if mutation == "cfg":
        source = "#[cfg(test)]\n" + source
    elif mutation == "cfg_attr":
        source = "#[cfg_attr(not(test), path = \"unreviewed.rs\")]\n" + source
    elif mutation == "wrong_path":
        source = source.replace('"v2_leader_wire_consumer.rs"', '"unreviewed.rs"')
    elif mutation == "public":
        source = source.replace("mod leader_wire_consumer;", "pub(crate) mod leader_wire_consumer;")
    elif mutation == "duplicate":
        source += source
    elif mutation == "comment":
        source = "/*" + source + "*/"
    elif mutation == "alias":
        source = source.replace("mod leader_wire_consumer;", "mod other_consumer;")
    elif mutation == "inner_cfg":
        source = "#![cfg(test)]\n" + source
    items["module_statements"] = module.rust_top_level_statements(source) + (export,)
    errors = module._persistent_recovery_cut_canonical_item_errors(paths, items)
    assert any("exact private ungated module edge" in error for error in errors), errors


@pytest.mark.parametrize("key", ("factory", "monotonicity", "wal_locks", "store", "mirror", "runtime_factory", "executor_publish", "service_publish", "enter"))
def test_persistent_recovery_canonical_rejects_disabled_production_owners(
    persistent_recovery_canonical_items, key: str,
) -> None:
    module, paths, originals = persistent_recovery_canonical_items
    items = dict(originals)
    items[key] = replace(items[key], attributes=("#[cfg(test)]",))
    errors = module._persistent_recovery_cut_canonical_item_errors(paths, items)
    assert any("canonical recovery-cut " + key in error and "cfg" in error for error in errors), errors


@pytest.mark.parametrize("name", ("from_replayed_adapter", "with_protected_lock", "advance_view", "with_durable_decision"))
def test_persistent_recovery_canonical_rejects_scalar_fixture_as_production(
    persistent_recovery_canonical_items, name: str,
) -> None:
    module, paths, originals = persistent_recovery_canonical_items
    items = dict(originals)
    fixture, = items["fixture_" + name]
    items["fixture_" + name] = (replace(fixture, attributes=()),)
    errors = module._persistent_recovery_cut_canonical_item_errors(paths, items)
    assert any("scalar authority fixture " + name in error for error in errors), errors


def test_persistent_recovery_canonical_requires_private_wal_fields(
    persistent_recovery_canonical_items,
) -> None:
    module, paths, originals = persistent_recovery_canonical_items
    items = dict(originals)
    authority, = items["authority_structs"]
    assert authority.body.count("consumer_tag:") == 1
    items["authority_structs"] = (replace(authority, body=authority.body.replace("consumer_tag:", "pub(crate) consumer_tag:", 1)),)
    errors = module._persistent_recovery_cut_canonical_item_errors(paths, items)
    assert any("private actual-WAL ownership" in error for error in errors), errors


def test_persistent_recovery_cut_current_source_full_integration() -> None:
    """Keep the complete producer, startup, cut, high-watermark, and formal binding independent."""
    module = load_checker()
    assert module._persistent_recovery_cut_source_fidelity_errors(ROOT_DIR) == []


@pytest.mark.parametrize("insertion", (
    "let tag = reducer::EventTag::new(1, 0, reducer::Generation::INITIAL);",
    "let durable = fabricated_durable_state();",
    "let protected_lock = None;",
    "let protected_commit_statement = None;",
))
def test_persistent_recovery_canonical_rejects_shadowed_factory_ownership(
    persistent_recovery_canonical_items, insertion: str,
) -> None:
    module, paths, originals = persistent_recovery_canonical_items
    items = dict(originals)
    item = items["factory"]
    # Keep every original required fragment present and ordered. Shadowing between
    # fragments must still fail the complete current-factory construction contract.
    anchor = "let protected_lock" if "let tag" in insertion or "let durable" in insertion else "Ok(Self {"
    assert item.source.count(anchor) == 1
    source = item.source.replace(anchor, insertion + "\n" + anchor, 1)
    items["factory"] = replace(item, source=source, body=source[source.index("{") + 1:source.rfind("}")],
                               structural_source=module.mask_rust_comments_and_literals(source))
    errors = module._persistent_recovery_cut_canonical_item_errors(paths, items)
    assert any("complete actual-WAL construction" in error for error in errors), errors
