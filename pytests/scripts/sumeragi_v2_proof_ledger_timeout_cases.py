# Executed lexically in sumeragi_v2_proof_ledger_test.py; do not collect directly.

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

    formal_dir = ROOT_DIR / "formal" / "sumeragi_v2"
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


def test_exact_local_proposal_timeout_kernel_and_verus_proofs_are_source_bound(
    tmp_path: Path,
) -> None:
    """Constants, disconnected proofs, stale replay, and call bypasses fail."""

    module = load_checker()
    assert module._local_proposal_timeout_source_fidelity_errors(ROOT_DIR) == []
    source_paths = (
        Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/wal.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
        Path("crates/iroha_core/src/sumeragi/v2_core/tests.rs"),
        Path("crates/iroha_sumeragi_core/src/verus_proofs.rs"),
    )

    def copy_fixture(case: str) -> Path:
        repo_root = tmp_path / case
        for relative in source_paths:
            destination = repo_root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT_DIR / relative, destination)
        return repo_root

    item_mutants = (
        (
            "constant_production_result",
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "local_proposal_timeout_justification_is_exact",
            "local_proposal_timeout_justification_body!(projection, zero, one, absent_evidence)",
            "true",
            "production exact timeout-justification kernel",
        ),
        (
            "altered_concrete_projection",
            "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
            "local_proposal_timeout_projection",
            "proposal_timeout == durable_timeout",
            "proposal_timeout != durable_timeout",
            "projection builder",
        ),
        (
            "wal_adapter_disconnected",
            "crates/iroha_core/src/sumeragi/v2_core/wal.rs",
            "is_exact_local_proposal_timeout_justification",
            "refinement::local_proposal_timeout_justification_is_exact(",
            "false || refinement::local_proposal_timeout_justification_is_exact(",
            "durable-state exact timeout adapter",
        ),
        (
            "stale_wal_proposal_replay",
            "crates/iroha_core/src/sumeragi/v2_core/wal.rs",
            "apply_in_place",
            "&& self.is_exact_local_proposal_timeout_justification(",
            "&& true || self.is_exact_local_proposal_timeout_justification(",
            "ProposalIntent WAL replay gate",
        ),
        (
            "live_proposal_call_removed",
            "crates/iroha_core/src/sumeragi/v2_core/reducer.rs",
            "on_local_proposal_ready",
            "if !self\n"
            "                .durable\n"
            "                .is_exact_local_proposal_timeout_justification(",
            "if false && !self\n"
            "                .durable\n"
            "                .is_exact_local_proposal_timeout_justification(",
            "live local-proposal construction",
        ),
        (
            "retransmission_call_removed",
            "crates/iroha_core/src/sumeragi/v2_core/reducer.rs",
            "durable_proposal_is_active",
            "ProposalJustification::Timeout(certificate) => durable",
            "ProposalJustification::Timeout(certificate) => true || durable",
            "durable proposal retransmission authorization",
        ),
        (
            "recovery_filter_removed",
            "crates/iroha_core/src/sumeragi/v2_core/reducer.rs",
            "on_resume_after_replay",
            ".filter(|proposal| Self::durable_proposal_is_active(&self.durable, proposal))",
            ".filter(|_| true)",
            "WAL replay resumption filter",
        ),
        (
            "verus_ensures_true",
            "crates/iroha_sumeragi_core/src/verus_proofs.rs",
            "verified_local_proposal_timeout_justification_is_exact",
            "accepted == local_proposal_timeout_justification_is_exact(projection)",
            "true",
            "Verus executable-kernel equivalence theorem",
        ),
        (
            "verus_executable_proof_disconnected",
            "crates/iroha_sumeragi_core/src/verus_proofs.rs",
            "verified_local_proposal_timeout_justification_is_exact",
            "let accepted = local_proposal_timeout_justification_body!(",
            "let accepted = true || local_proposal_timeout_justification_body!(",
            "Verus executable-kernel equivalence theorem",
        ),
        (
            "verus_latest_postcondition_weakened",
            "crates/iroha_sumeragi_core/src/verus_proofs.rs",
            "exact_local_proposal_timeout_justification_binds_latest_durable_tc",
            "projection.current_view > 0,",
            "true,",
            "Verus latest-durable-timeout consequence theorem",
        ),
        (
            "verus_wal_guard_disconnected",
            "crates/iroha_sumeragi_core/src/verus_proofs.rs",
            "wal_frame_admissible",
            "&& local_proposal_timeout_justification_is_exact(",
            "&& true || local_proposal_timeout_justification_is_exact(",
            "Verus ProposalIntent WAL guard",
        ),
        (
            "stale_recovery_regression",
            "crates/iroha_core/src/sumeragi/v2_core/tests.rs",
            "recovery_uses_same_round_timeout_upgrade_as_exact_local_proposal_justification",
            "Err(ReducerError::Replay(ReplayError::InvalidProposalIntent))",
            "Ok(_)",
            "exact-timeout WAL recovery regression",
        ),
    )
    for case, relative, item, old, new, expected in item_mutants:
        repo_root = copy_fixture(case)
        mutate_rust_item_source(module, repo_root / relative, item, old, new)
        errors = module._local_proposal_timeout_source_fidelity_errors(
            repo_root
        )
        assert any(expected in error for error in errors), errors

    repo_root = copy_fixture("verus_wal_consequence_disconnected")
    path = repo_root / "crates/iroha_sumeragi_core/src/verus_proofs.rs"
    source = path.read_text(encoding="utf-8")
    old = """if view > 0 {
                exact_local_proposal_timeout_justification_binds_latest_durable_tc(
                    timeout_justification,
                );
            }"""
    assert source.count(old) == 1
    path.write_text(source.replace(old, "if view > 0 {}", 1), encoding="utf-8")
    errors = module._local_proposal_timeout_source_fidelity_errors(repo_root)
    assert any(
        "Verus WAL consequence must call the nontrivial latest-timeout theorem"
        in error
        for error in errors
    ), errors


def test_installed_tc_selector_proof_and_call_paths_are_source_bound(
    tmp_path: Path, monkeypatch
) -> None:
    """The exact last-installed certificate, PrepareQC, and callers stay connected."""

    module = load_checker()
    assert module._installed_tc_selector_source_fidelity_errors(
        module.FORMAL_DIR
    ) == []
    formal_names = (
        "SumeragiV2Core.tla",
        "SumeragiV2Inductive.tla",
        "SumeragiV2AsyncNetwork.tla",
        "SumeragiV2InstalledTcSelectorProofs.tla",
    )

    def copy_fixture(case: str) -> Path:
        formal_dir = tmp_path / case
        formal_dir.mkdir()
        for name in formal_names:
            shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)
        return formal_dir

    operator_mutants = (
        (
            "proposal_tc_retargeted",
            "SumeragiV2Core.tla",
            "LocalProposalJustification",
            "tc == lastInstalledTc[node]",
            "tc == NoTimeoutCertificate",
            "direct selector call path",
        ),
        (
            "proposal_prepare_qc_reconstructed",
            "SumeragiV2Core.tla",
            "ProposalJustified",
            "proposal.highestPrepareQc =\n"
            "          lastInstalledTc[node].highestPrepareQc",
            "proposal.highestPrepareQc = NoPrepareQC",
            "full-certificate identity",
        ),
        (
            "proof_selector_chooses_history",
            "SumeragiV2InstalledTcSelectorProofs.tla",
            "ExactSelectedInstalledTcForRound",
            "LastInstalledTcEntry(node)",
            "CHOOSE installed \\in installedTCs: TRUE",
            "direct selector call path",
        ),
        (
            "restart_uses_history",
            "SumeragiV2AsyncNetwork.tla",
            "RestartLastInstalledTCs",
            "ELSE {lastInstalledTc[node]}",
            "ELSE RestartInstalledTCs(node)",
            "direct selector call path",
        ),
        (
            "proof_drops_prepare_qc_identity",
            "SumeragiV2InstalledTcSelectorProofs.tla",
            "InstalledTcExactSelectionInvariant",
            "lastInstalledTc[node].highestPrepareQc\n"
            "              \\in PrepareQcOptionSet",
            "PrepareQcRank(lastInstalledTc[node].highestPrepareQc)\n"
            "              \\in Ranks",
            "full-certificate identity",
        ),
    )
    for case, filename, symbol, old, new, expected in operator_mutants:
        formal_dir = copy_fixture(case)
        path = formal_dir / filename
        path.write_text(
            mutate_tla_operator(
                path.read_text(encoding="utf-8"), symbol, old, new
            ),
            encoding="utf-8",
        )
        errors = module._installed_tc_selector_source_fidelity_errors(
            formal_dir
        )
        assert any(expected in error for error in errors), errors

    formal_dir = copy_fixture("retired_history_selector_reintroduced")
    core = formal_dir / "SumeragiV2Core.tla"
    source = core.read_text(encoding="utf-8")
    core.write_text(
        source.replace(
            "\nLocalProposalJustification(node) ==",
            "\nInstalledTcAtLeastAsRecent(candidate, other) == TRUE\n\n"
            "LocalProposalJustification(node) ==",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._installed_tc_selector_source_fidelity_errors(formal_dir)
    assert any(
        "retired history/rank installed-TC selector symbols are prohibited"
        in error
        for error in errors
    ), errors

    formal_dir = copy_fixture("tautological_uniqueness_theorem")
    proof = formal_dir / "SumeragiV2InstalledTcSelectorProofs.tla"
    proof.write_text(
        mutate_tla_theorem(
            proof.read_text(encoding="utf-8"),
            "ExactInstalledTcSelectorIsUnique",
            "IN /\\ selected \\in current\n"
            "            /\\ \\A other \\in current: other = selected\n"
            "            /\\ selected.tc = lastInstalledTc[node]\n"
            "            /\\ selected.tc.highestPrepareQc =\n"
            "                 lastInstalledTc[node].highestPrepareQc",
            "IN TRUE",
        ),
        encoding="utf-8",
    )
    errors = module._installed_tc_selector_source_fidelity_errors(formal_dir)
    assert any(
        "nontrivial selector uniqueness/proposal postcondition" in error
        for error in errors
    ), errors

    formal_dir = copy_fixture("omitted_proof_module")
    (formal_dir / "SumeragiV2InstalledTcSelectorProofs.tla").unlink()
    errors = module._installed_tc_selector_source_fidelity_errors(formal_dir)
    assert any("selector source must be a regular file" in error for error in errors)

    monkeypatch.setattr(
        module,
        "RELEASE_PROOF_MODULES",
        tuple(
            name
            for name in module.RELEASE_PROOF_MODULES
            if name != "SumeragiV2InstalledTcSelectorProofs"
        ),
    )
    errors = module._installed_tc_selector_source_fidelity_errors(
        module.FORMAL_DIR
    )
    assert any("release proof inventory must execute" in error for error in errors)


def test_local_runner_service_contract_rejects_split_deadlock_call_symbol(
    tmp_path: Path,
) -> None:
    """Whitespace tolerance must not concatenate a malformed call symbol."""

    module = load_checker()
    formal_dir = local_runner_service_fixture(tmp_path, module)
    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            "DeadlockFreedomObligation",
            "      ENABLED PostGstProductiveStepWith(\n"
            "        AsyncTerminatingLocalWorkDecreaseStep))\n",
            "      ENABLED PostGstProductiveStep With(\n"
            "        AsyncTerminatingLocalWorkDecreaseStep))\n",
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


def test_locked_assembly_retention_mutation_is_source_bound_and_complete(
    tmp_path: Path,
) -> None:
    """The Busy assembly dispatch repair and red/green pair must remain exact."""

    module = load_checker()

    def copy_fixture(case: str) -> tuple[Path, Path, Path, Path]:
        repo_root = tmp_path / case
        formal_dir = repo_root / "docs" / "formal" / "sumeragi_v2"
        formal_dir.mkdir(parents=True)
        for name in module._LOCKED_ASSEMBLY_RETENTION_MUTATION_SOURCE_SHA256:
            shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)
        async_path = formal_dir / "SumeragiV2AsyncNetwork.tla"
        shutil.copy2(
            module.FORMAL_DIR / "SumeragiV2AsyncNetwork.tla", async_path
        )
        runner = (
            repo_root
            / "scripts"
            / "formal"
            / "run_sumeragi_v2_progress_mutations.sh"
        )
        runner.parent.mkdir(parents=True)
        shutil.copy2(
            ROOT_DIR
            / "scripts"
            / "formal"
            / "run_sumeragi_v2_progress_mutations.sh",
            runner,
        )
        return repo_root, formal_dir, async_path, runner

    repo_root, formal_dir, _, _ = copy_fixture("exact")
    assert module._locked_assembly_retention_mutation_errors(
        formal_dir, repo_root
    ) == []

    repo_root, formal_dir, async_path, _ = copy_fixture("kernel-drift")
    source = async_path.read_text(encoding="utf-8")
    async_path.write_text(
        mutate_tla_operator(
            source,
            "LocalAssemblyBusyDispatchAllowed",
            '"AssembleBody"',
            '"BeginPrepare"',
        ),
        encoding="utf-8",
    )
    errors = module._locked_assembly_retention_mutation_errors(
        formal_dir, repo_root
    )
    assert any(
        "LocalAssemblyBusyDispatchAllowed must equal the exact "
        "locked-assembly Busy dispatch contract" in error
        for error in errors
    ), errors

    repo_root, formal_dir, async_path, _ = copy_fixture("disconnected-kernel")
    source = async_path.read_text(encoding="utf-8")
    async_path.write_text(
        mutate_tla_operator(
            source,
            "CommandDispatchable",
            "\\/ LocalAssemblyBusyDispatchAllowed(command)",
            "\\/ FALSE",
        ),
        encoding="utf-8",
    )
    errors = module._locked_assembly_retention_mutation_errors(
        formal_dir, repo_root
    )
    assert any(
        "CommandDispatchable must equal the exact locked-assembly Busy "
        "dispatch contract" in error
        for error in errors
    ), errors

    repo_root, formal_dir, _, _ = copy_fixture("config-drift")
    config = formal_dir / "locked_assembly_retention_old.cfg"
    source = config.read_text(encoding="utf-8")
    assert source.count("AllowBusyLockedAssembly = FALSE") == 1
    config.write_text(
        source.replace(
            "AllowBusyLockedAssembly = FALSE",
            "AllowBusyLockedAssembly = TRUE",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._locked_assembly_retention_mutation_errors(
        formal_dir, repo_root
    )
    assert any("must match exact reviewed SHA-256" in error for error in errors)

    runner_mutants = (
        (
            "wrong-status",
            "locked_assembly_retention_fixed.cfg 0",
            "locked_assembly_retention_fixed.cfg 12",
            "locked-assembly-retention-fixed exactly once with status 0",
        ),
        (
            "weakened-marker",
            '"State 2: <DispatchRuntimeHead"',
            '"State 2: <DispatchRuntimeHeadMaybe"',
            "locked-assembly-retention-old must require exact markers",
        ),
    )
    for case, old, new, expected_error in runner_mutants:
        repo_root, formal_dir, _, runner = copy_fixture(case)
        source = runner.read_text(encoding="utf-8")
        assert source.count(old) == 1, old
        runner.write_text(source.replace(old, new, 1), encoding="utf-8")
        errors = module._locked_assembly_retention_mutation_errors(
            formal_dir, repo_root
        )
        assert any(expected_error in error for error in errors), errors

    repo_root, formal_dir, _, runner = copy_fixture("reordered-pair")
    source = runner.read_text(encoding="utf-8")
    old_start = source.index("run_case locked-assembly-retention-old")
    fixed_start = source.index("run_case locked-assembly-retention-fixed")
    next_start = source.index("run_case locked-body-reproposal", fixed_start)
    old_block = source[old_start:fixed_start]
    fixed_block = source[fixed_start:next_start]
    runner.write_text(
        source[:old_start] + fixed_block + old_block + source[next_start:],
        encoding="utf-8",
    )
    errors = module._locked_assembly_retention_mutation_errors(
        formal_dir, repo_root
    )
    assert any("must keep old-before-fixed order" in error for error in errors)


def test_locked_body_reproposal_mutation_matrix_is_source_bound_and_complete(
    tmp_path: Path,
) -> None:
    """The strict same-round red/green matrix must remain exact and runnable."""

    module = load_checker()

    def copy_fixture(case: str) -> tuple[Path, Path, Path]:
        repo_root = tmp_path / case
        formal_dir = repo_root / "docs" / "formal" / "sumeragi_v2"
        formal_dir.mkdir(parents=True)
        for name in module._LOCKED_BODY_REPROPOSAL_MUTATION_SOURCE_SHA256:
            shutil.copy2(module.FORMAL_DIR / name, formal_dir / name)
        runner = (
            repo_root
            / "scripts"
            / "formal"
            / "run_sumeragi_v2_progress_mutations.sh"
        )
        runner.parent.mkdir(parents=True)
        shutil.copy2(
            ROOT_DIR
            / "scripts"
            / "formal"
            / "run_sumeragi_v2_progress_mutations.sh",
            runner,
        )
        return repo_root, formal_dir, runner

    repo_root, formal_dir, _ = copy_fixture("exact")
    assert module._locked_body_reproposal_mutation_runner_errors(
        formal_dir, repo_root
    ) == []

    repo_root, formal_dir, _ = copy_fixture("config-drift")
    config = formal_dir / "historical_locked_recovery_fresh_commit_bug.cfg"
    source = config.read_text(encoding="utf-8")
    assert source.count("AllowFreshHistoricalCommitBug = TRUE") == 1
    config.write_text(
        source.replace(
            "AllowFreshHistoricalCommitBug = TRUE",
            "AllowFreshHistoricalCommitBug = FALSE",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._locked_body_reproposal_mutation_runner_errors(
        formal_dir, repo_root
    )
    assert any("must match exact reviewed SHA-256" in error for error in errors)

    runner_mutants = (
        (
            "wrong-status",
            "locked_body_reproposal_high_fixed.cfg 0",
            "locked_body_reproposal_high_fixed.cfg 12",
            "locked-body-reproposal-high-fixed exactly once with status 0",
        ),
        (
            "weakened-marker",
            '"Invariant LaterHighReproposalAccepted is violated."',
            '"Invariant LaterHighReproposalAccepted might be violated."',
            "locked-body-reproposal-no-high-bug must require exact markers",
        ),
        (
            "missing-fresh-commit",
            "run_case historical-locked-recovery-fresh-commit-bug \\\n"
            "  SumeragiV2HistoricalLockedRecoveryMutation.tla \\\n"
            "  historical_locked_recovery_fresh_commit_bug.cfg 12 \\\n"
            '  "Invariant ExactIntentDoesNotAuthorizeFreshCommit is violated." \\\n'
            '  "4 states generated, 3 distinct states found, 0 states left on queue."\n',
            "",
            "historical-locked-recovery-fresh-commit-bug exactly once with status 12",
        ),
    )
    for case, old, new, expected_error in runner_mutants:
        repo_root, formal_dir, runner = copy_fixture(case)
        source = runner.read_text(encoding="utf-8")
        assert source.count(old) == 1, old
        runner.write_text(source.replace(old, new, 1), encoding="utf-8")
        errors = module._locked_body_reproposal_mutation_runner_errors(
            formal_dir, repo_root
        )
        assert any(expected_error in error for error in errors), errors


def test_proposal_timeout_exact_regressions_reject_case_removal_mutants(
    tmp_path: Path,
) -> None:
    """Omitted, invented, and alternate evidence remain pinned in both tests."""

    module = load_checker()
    source_paths = (
        Path("crates/iroha_data_model/src/block/consensus_v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
    )
    test_context = (
        ("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),
    )

    def copy_fixture(case: str) -> Path:
        repo_root = tmp_path / case
        for relative in source_paths:
            destination = repo_root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT_DIR / relative, destination)
        return repo_root

    wire_mutants = (
        (
            "wire_invented_rejection_neutralized",
            """assert_eq!(
            proposal.validate(&context),
            Err(ValidationError::InvalidProposalJustification),
            "a proposal cannot invent a repeated high absent from its TC"
        );""",
            """assert_eq!(
            proposal.validate(&context),
            Ok(()),
            "a proposal cannot invent a repeated high absent from its TC"
        );""",
            "wire regression must reject an invented repeated PrepareQC",
        ),
        (
            "wire_omitted_rejection_neutralized",
            """assert_eq!(
            proposal.validate(&context),
            Err(ValidationError::InvalidProposalJustification),
            "a proposal cannot omit the exact high selected by its TC"
        );""",
            """assert_eq!(
            proposal.validate(&context),
            Ok(()),
            "a proposal cannot omit the exact high selected by its TC"
        );""",
            "wire regression must reject an omitted repeated PrepareQC",
        ),
        (
            "wire_alternate_evidence_rejection_neutralized",
            """assert_eq!(
            proposal.validate(&context),
            Err(ValidationError::InvalidProposalJustification),
            "the repeated high must preserve the TC-selected full evidence"
        );""",
            """assert_eq!(
            proposal.validate(&context),
            Ok(()),
            "the repeated high must preserve the TC-selected full evidence"
        );""",
            "wire regression must reject same-reference alternate PrepareQC evidence",
        ),
    )
    wire_path = Path("crates/iroha_data_model/src/block/consensus_v2.rs")
    wire_item = "timeout_proposal_accepts_only_the_selected_prepare_subject"
    for case, old, new, expected in wire_mutants:
        repo_root = copy_fixture(case)
        mutate_rust_item_source_in_context(
            module,
            repo_root / wire_path,
            wire_item,
            test_context,
            old,
            new,
        )
        errors = module._proposal_timeout_exactness_source_fidelity_errors(
            repo_root
        )
        assert any(expected in error for error in errors), errors

    def remove_adapter_case(
        repo_root: Path, start: str, end: str
    ) -> None:
        path = repo_root / "crates/iroha_core/src/sumeragi/v2.rs"
        source = path.read_text(encoding="utf-8")
        items = [
            item
            for item in module.rust_items(
                source,
                "locked_subject_reproposal_and_strict_higher_prepare_are_safe",
            )
            if item.brace_context == test_context
        ]
        assert len(items) == 1
        item = items[0]
        start_offset = item.source.index(start)
        end_offset = item.source.index(end, start_offset)
        mutated_item = item.source[:start_offset] + item.source[end_offset:]
        assert source.count(item.source) == 1
        path.write_text(
            source.replace(item.source, mutated_item, 1), encoding="utf-8"
        )

    adapter_mutants = (
        (
            "adapter_omitted_rejection_removed",
            "        let mut missing_repeated_high = prepared_proposal.clone();",
            "        let mut invented_repeated_high = prepared_proposal.clone();",
            "adapter regression must reject omitted evidence at safe-value admission",
        ),
        (
            "adapter_invented_rejection_removed",
            "        let mut invented_repeated_high = prepared_proposal.clone();",
            "        let mut alternate_evidence = prepared_proposal.clone();",
            "adapter regression must reject invented evidence at safe-value admission",
        ),
        (
            "adapter_alternate_evidence_rejection_removed",
            "        let mut alternate_evidence = prepared_proposal.clone();",
            "        let mut equal_rank = prepared_proposal.clone();",
            "adapter regression must reject same-reference alternate evidence at safe-value admission",
        ),
    )
    for case, start, end, expected in adapter_mutants:
        repo_root = copy_fixture(case)
        remove_adapter_case(repo_root, start, end)
        errors = module._proposal_timeout_exactness_source_fidelity_errors(
            repo_root
        )
        assert any(expected in error for error in errors), errors


def test_proposal_timeout_full_evidence_production_gates_are_source_bound(
    tmp_path: Path,
) -> None:
    """Every production consumer must compare full repeated PrepareQC evidence."""

    module = load_checker()
    assert (
        module._proposal_timeout_exactness_source_fidelity_errors(ROOT_DIR)
        == []
    )
    source_paths = (
        Path("crates/iroha_data_model/src/block/consensus_v2.rs"),
        Path("crates/iroha_core/src/sumeragi/v2.rs"),
    )

    def copy_fixture(case: str) -> Path:
        repo_root = tmp_path / case
        for relative in source_paths:
            destination = repo_root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT_DIR / relative, destination)
        return repo_root

    production_mutants = (
        (
            "proposal_validate_reference_only",
            Path("crates/iroha_data_model/src/block/consensus_v2.rs"),
            "validate",
            (("impl", "Proposal"),),
            "selected_highest != timeout.highest_prepare_qc.as_ref()",
            """selected_highest.map(QuorumCertificate::as_ref)
                    != timeout
                        .highest_prepare_qc
                        .as_ref()
                        .map(QuorumCertificate::as_ref)""",
            "Proposal::validate must compare the complete TC-selected",
        ),
        (
            "wire_registry_reference_only",
            Path("crates/iroha_core/src/sumeragi/v2.rs"),
            "justification_to_core",
            (("impl", "WireRegistry"),),
            "selected != timeout.highest_prepare_qc.as_ref()",
            """selected.map(wire::QuorumCertificate::as_ref)
                    != timeout
                        .highest_prepare_qc
                        .as_ref()
                        .map(wire::QuorumCertificate::as_ref)""",
            "WireRegistry must reject unequal complete PrepareQC evidence",
        ),
        (
            "safe_value_reference_only",
            Path("crates/iroha_core/src/sumeragi/v2.rs"),
            "proposal_is_safe_for_lock",
            (),
            "selected == highest",
            "selected.as_ref() == highest.as_ref()",
            "safe-value admission must compare the complete selected PrepareQC",
        ),
    )
    for case, relative, item, context, old, new, expected in production_mutants:
        repo_root = copy_fixture(case)
        mutate_rust_item_source_in_context(
            module,
            repo_root / relative,
            item,
            context,
            old,
            new,
        )
        errors = module._proposal_timeout_exactness_source_fidelity_errors(
            repo_root
        )
        assert any(expected in error for error in errors), errors


def test_protected_service_rank_aggregate_cannot_drop_exact_leaf(
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
    path = formal_dir / "SumeragiV2AsyncLivenessProofs.tla"
    source = path.read_text(encoding="utf-8")
    path.write_text(
        mutate_tla_theorem(
            source,
            "ProtectedServiceRankProgressObligation",
            "ProtectedStage6RankProgressFromFairCausalAdmissionObligation",
            "ProtectedStage4RankProgressFromFairScheduler",
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)

    assert any(
        "ProtectedServiceRankProgressObligation must retain exact "
        "protected-rank proof dependency "
        "'ProtectedStage6RankProgressFromFairCausalAdmissionObligation' once; "
        "found 0"
        in error
        for error in errors
    ), errors


@pytest.mark.parametrize(
    ("current_id", "retired_id", "retired_symbol"),
    (
        (
            "same-round-lock-and-commit-authorization",
            "historical-tc-lock-commit",
            "HistoricalTcLockedCommitAuthorizationObligation",
        ),
        (
            "locked-body-reproposal",
            "locked-body-reproposal-liveness",
            "LockedBodyReproposalProgressObligation",
        ),
    ),
)
def test_reviewed_obligation_inventory_rejects_retired_lock_mappings(
    current_id: str,
    retired_id: str,
    retired_symbol: str,
) -> None:
    """Legacy IDs cannot reauthorize split-round Commit or vacuous liveness."""

    module = load_checker()
    ledger = copy.deepcopy(module.load_ledger())
    obligation = next(
        item for item in ledger["obligations"] if item["id"] == current_id
    )
    obligation["id"] = retired_id
    obligation["symbol"] = retired_symbol
    obligation["status"] = "tlaps_proved"

    errors = module.validate_ledger(ledger).errors

    assert any(
        f"proof ledger is missing reviewed obligation {current_id}" in error
        for error in errors
    ), errors
    assert retired_id not in module.REQUIRED_PROOF_OBLIGATION_INVENTORY


def test_same_round_lock_and_commit_authorization_rejects_fresh_historical_commit(
    tmp_path: Path,
) -> None:
    module = load_checker()
    core_source = (module.FORMAL_DIR / "SumeragiV2Core.tla").read_text()
    inductive_source = (module.FORMAL_DIR / "SumeragiV2Inductive.tla").read_text()

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
    assert historical_body == "FALSE"

    begin = module._top_level_operator_body(
        core_source, "BeginLockCommit", preserve_string_contents=True
    )
    persist = module._top_level_operator_body(
        core_source, "PersistLockCommit", preserve_string_contents=True
    )
    assert begin is not None
    assert persist is not None
    begin_body = " ".join(begin[0].split())
    persist_body = " ".join(persist[0].split())
    assert "CurrentOpenPrepareForCommit(node, qc)" in begin_body
    assert "HistoricalLockedPrepareForCommit(node, qc)" not in begin_body
    assert 'Vote(context, qc.view, "Commit", qc.subject, node)' in begin_body
    assert "pendingLockCommit' = pendingLockCommit \\cup {request}" in begin_body
    assert "commitIntents' = commitIntents \\cup {request.vote}" in persist_body
    assert "signVotes' = signVotes \\cup {signRequest}" in persist_body

    authorization = module._top_level_operator_body(
        inductive_source, "SameRoundLockAndCommitAuthorizationInvariant"
    )
    assert authorization is not None
    authorization_body = " ".join(authorization[0].split())
    assert "request.vote.view = nodeView[request.node]" in authorization_body
    assert (
        "CurrentOpenPrepareForCommit(request.node, request.qc)"
        in authorization_body
    )
    assert "TimeoutVoteStrictlyProtectsCommit(timeoutVote, commitVote)" in (
        authorization_body
    )

    by_id = {
        obligation["id"]: obligation
        for obligation in module.load_ledger()["obligations"]
    }
    assert by_id["same-round-lock-and-commit-authorization"]["status"] == (
        "tlaps_proved"
    )
    assert by_id["timeout-protection"]["status"] == "tlaps_proved"

    # The promoted theorem stays source-bound and non-vacuous. In particular,
    # disconnecting the exact-current-round helper from its defining predicate
    # is rejected independently of the complete-file seal.
    module = load_checker()
    assert module._same_round_strict_tla_source_fidelity_errors(
        module.FORMAL_DIR
    ) == []
    mutations = (
        (
            "missing_current_open_dependency",
            "SumeragiV2InductiveProofs.tla",
            "PendingLockCommitUsesExactCurrentRound",
            ", CurrentOpenPrepareForCommit",
            "",
            "PendingLockCommitUsesExactCurrentRound must retain the exact reviewed",
            True,
        ),
        (
            "disconnected_wrapper",
            "SumeragiV2Proofs.tla",
            "SameRoundLockAndCommitAuthorizationObligation",
            "ReducerProvenanceImpliesSameRoundLockAndCommitAuthorization",
            "DurableTimeoutProtectionIsDirect",
            "SameRoundLockAndCommitAuthorizationObligation must retain the "
            "exact reviewed",
            True,
        ),
        (
            "tautological_property",
            "SumeragiV2Proofs.tla",
            "SameRoundLockAndCommitAuthorizationProperty",
            "specification => []SameRoundLockAndCommitAuthorizationInvariant",
            "specification => []TRUE",
            "must equal only the exact non-vacuous property",
            False,
        ),
        (
            "weakened_current_round",
            "SumeragiV2Core.tla",
            "CurrentOpenPrepareForCommit",
            "qc.view = nodeView[node]",
            "qc.view <= nodeView[node]",
            "CurrentOpenPrepareForCommit must equal only the exact reviewed",
            False,
        ),
        (
            "disconnected_inductive_invariant",
            "SumeragiV2Inductive.tla",
            "SameRoundLockAndCommitAuthorizationInvariant",
            "CurrentOpenPrepareForCommit(request.node, request.qc)",
            "TRUE",
            "SameRoundLockAndCommitAuthorizationInvariant must equal only the "
            "exact reviewed",
            False,
        ),
    )
    for case, name, symbol, old, new, error_fragment, theorem in mutations:
        formal_dir = tmp_path / case
        formal_dir.mkdir()
        for source_name in (
            *module._SAME_ROUND_STRICT_TLA_SOURCE_SHA256,
            "SumeragiV2Core.tla",
            "SumeragiV2Inductive.tla",
        ):
            shutil.copy2(module.FORMAL_DIR / source_name, formal_dir / source_name)
        path = formal_dir / name
        source = path.read_text(encoding="utf-8")
        mutate = mutate_tla_theorem if theorem else mutate_tla_operator
        path.write_text(mutate(source, symbol, old, new), encoding="utf-8")
        errors = module._same_round_strict_tla_source_fidelity_errors(formal_dir)
        assert any(error_fragment in error for error in errors), errors


def test_same_round_semantic_kernel_sources_and_callers_are_fail_closed(
    tmp_path: Path,
) -> None:
    """Shared vote/Commit kernels cannot be bypassed in production or Verus."""

    module = load_checker()
    assert module._same_round_semantic_kernel_source_fidelity_errors(ROOT_DIR) == []
    source_paths = tuple(
        Path(relative)
        for relative in module._SAME_ROUND_SEMANTIC_KERNEL_SOURCE_SHA256
    )
    mutations = (
        (
            "vote_signer_reintroduced",
            Path("crates/iroha_core/src/sumeragi/v2_core/types.rs"),
            "same_statement",
            "vote_statement_identity_equal_body!(",
            "self.signer == other.signer && vote_statement_identity_equal_body!(",
            "Vote::same_statement must invoke the shared identity kernel without signer",
        ),
        (
            "commit_phase_weakened",
            Path("crates/iroha_core/src/sumeragi/v2_core/types.rs"),
            "same_commit_decision",
            "self.phase == Phase::Commit",
            "self.phase == Phase::Prepare",
            "same_commit_decision must require Commit phase",
        ),
        (
            "vote_call_disconnected",
            Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
            "on_vote",
            "vote.same_statement(intent)",
            "vote.signer() == intent.signer()",
            "Commit vote admission must compare the exact signer-independent statement",
        ),
        (
            "prepare_lock_veto_reintroduced",
            Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
            "on_commit_certificate",
            "let effect = self.start_persistence(",
            """if self.durable.locked().is_some_and(|locked| {
            locked.subject() != certificate.subject()
        }) {
            return Err(ReducerError::ConflictingDecision);
        }
        let effect = self.start_persistence(""",
            "Commit admission must let the first validated CommitQC supersede any Prepare lock",
        ),
        (
            "wal_prepare_lock_veto_reintroduced",
            Path("crates/iroha_core/src/sumeragi/v2_core/wal.rs"),
            "apply_in_place",
            "if let Some(existing) = &self.decision {",
            """if self.locked.as_ref().is_some_and(|locked| {
                    locked.subject() != certificate.subject()
                }) {
                    return Err(ReplayError::InvalidCertificate);
                }
                if let Some(existing) = &self.decision {""",
            "WAL Decision replay must let the first validated CommitQC supersede any Prepare lock",
        ),
        (
            "effect_prepare_lock_veto_reintroduced",
            Path("crates/iroha_core/src/sumeragi/v2_effects.rs"),
            "reconcile_decision_work",
            "match self.protected_decision {",
            """if self
            .protected_lock
            .is_some_and(|(_, locked_subject)| locked_subject != decision_subject)
        {
            return Err(EffectExecutorError::Contract(
                "durable Decision differs from protected lock".to_owned(),
            ));
        }
        match self.protected_decision {""",
            "effect reconciliation must let the first durable Decision supersede any protected Prepare lock",
        ),
        (
            "production_gate_disconnected",
            Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
            "accepts",
            "accepts_facts(transition_facts(projection))",
            "true",
            "production commit gate must consume facts derived from the exact projection",
        ),
        (
            "strict_upgrade_kernel_constant_result",
            Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
            "strict_same_round_timeout_upgrade_is_allowed",
            "strict_same_round_timeout_upgrade_body!(projection, zero, one)",
            "true",
            "the executable strict timeout-upgrade kernel must invoke the shared proof body",
        ),
        (
            "strict_upgrade_lock_rank_weakened",
            Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
            "strict_same_round_timeout_upgrade_body",
            "projection.selected_prepare_view > projection.locked_prepare_view",
            "projection.selected_prepare_view >= projection.locked_prepare_view",
            "production and Verus must share one nontrivial strict same-round timeout-upgrade predicate",
        ),
        (
            "enter_view_retranscribes_upgrade",
            Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
            "enter_view_projection_gate_body",
            "$projection.before_tag.view <= $projection.after_tag.view",
            "$projection.before_tag.view <= timeout.view",
            "EnterView must consume the admitted monotonic post-view instead of transcribing a second strict-upgrade predicate",
        ),
        (
            "live_timeout_admission_bypasses_kernel",
            Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
            "on_timeout_certificate",
            """!self
                .durable
                .is_strict_same_round_timeout_upgrade(&certificate)""",
            "false",
            "live timeout-certificate admission must invoke the durable source-shared strict-upgrade adapter",
        ),
        (
            "timeout_ack_classification_bypasses_kernel",
            Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
            "on_persisted",
            """self
                    .durable
                    .is_strict_same_round_timeout_upgrade(certificate)""",
            "certificate.highest_prepare().is_some()",
            "InstallTimeout acknowledgement must classify the exact strict same-round lock-only upgrade before applying durable state",
        ),
        (
            "timeout_ack_generation_preflight_removed",
            Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
            "on_persisted",
            """let next_generation =
            if matches!(&pending.continuation, Continuation::InstallTimeout { .. }) {
                self.generation
                    .next()
                    .ok_or(ReducerError::GenerationOverflow)?
            } else {
                self.generation
            };""",
            "let next_generation = self.generation;",
            "InstallTimeout acknowledgement must reject generation exhaustion before applying WAL state or releasing its pending owner",
        ),
        (
            "timeout_ack_ignores_preflighted_generation",
            Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
            "on_persisted",
            "self.generation = next_generation;",
            "self.generation = self.generation.next().unwrap_or(self.generation);",
            "InstallTimeout acknowledgement must commit only the preflighted next generation",
        ),
        (
            "ack_refinement_skips_durable_apply",
            Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
            "acknowledgement_is_exact",
            """.apply(&self.context, self.local_validator, &pending.entry)
            .is_err()""",
            ".apply(&self.context, self.local_validator, &pending.entry)\n            .is_ok()",
            "the acknowledgement refinement must re-run the source-shared durable WAL admission before accepting EnterView effects",
        ),
        (
            "wal_upgrade_adapter_uses_view_only_owner",
            Path("crates/iroha_core/src/sumeragi/v2_core/wal.rs"),
            "is_strict_same_round_timeout_upgrade",
            "installed.round() == certificate.round()",
            "installed.round().view() == certificate.round().view()",
            "the durable adapter must project exact installed-round identity into the shared strict-upgrade kernel",
        ),
        (
            "wal_replay_bypasses_upgrade_kernel",
            Path("crates/iroha_core/src/sumeragi/v2_core/wal.rs"),
            "apply_in_place",
            "self.is_strict_same_round_timeout_upgrade(certificate)",
            "certificate.highest_prepare().is_some()",
            "WAL replay must classify strict timeout upgrades only through the shared durable adapter",
        ),
        (
            "verus_upgrade_kernel_disconnected",
            Path("crates/iroha_sumeragi_core/src/verus_proofs.rs"),
            "strict_same_round_timeout_upgrade",
            "strict_same_round_timeout_upgrade_body!(",
            "true || strict_same_round_timeout_upgrade_body!(",
            "Verus must instantiate the shared strict timeout-upgrade body from its primitive WAL projection",
        ),
        (
            "strict_same_round_timeout_pool_erased",
            Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
            "on_persisted",
            "if strict_same_round_timeout_upgrade {",
            "if false && strict_same_round_timeout_upgrade {",
            "strict same-round InstallTimeout must preserve only the exact current-round timeout pool and formed marker",
        ),
        (
            "timeout_generation_overflow_regression_deleted",
            Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
            "timeout_install_generation_overflow_preserves_the_complete_state",
            "fn timeout_install_generation_overflow_preserves_the_complete_state() {",
            "fn removed_timeout_install_generation_overflow_preserves_the_complete_state() {",
            "generation overflow must retain a regression for complete reducer-state non-mutation",
        ),
        (
            "timeout_generation_overflow_public_state_weakened",
            Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
            "timeout_install_generation_overflow_preserves_the_complete_state",
            "assert_eq!(pending, before);",
            "assert_eq!(pending.generation, before.generation);",
            "the public reducer step must preserve every durable, pending, and volatile owner on generation overflow",
        ),
        (
            "timeout_generation_overflow_in_place_state_weakened",
            Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
            "timeout_install_generation_overflow_preserves_the_complete_state",
            "assert_eq!(in_place, before);",
            "assert_eq!(in_place.generation, before.generation);",
            "the in-place acknowledgement callback must preserve the complete reducer state on generation overflow",
        ),
        (
            "strict_same_round_timeout_control_dropped",
            Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
            "on_persisted",
            "| OutboundControlClass::TimeoutVote",
            "| OutboundControlClass::CommitQc",
            "active exact TimeoutVote and highest PrepareQC control owners",
        ),
        (
            "same_size_timeout_pool_substitution_accepted",
            Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
            "transition_branch_constraints_body",
            "$facts.timeout_vote_pool_unchanged",
            "true",
            "the production gate must reject same-size timeout-pool substitution and advancing-view retention",
        ),
        (
            "timeout_control_substitution_accepted",
            Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
            "transition_branch_constraints_body",
            "$facts.timeout_control_unchanged",
            "true",
            "the production gate must preserve exact timeout-control identity across a lock-only install and require absence after advance",
        ),
        (
            "advancing_timeout_control_retention_accepted",
            Path("crates/iroha_core/src/sumeragi/v2_core/refinement.rs"),
            "transition_branch_constraints_body",
            "$facts.timeout_control_after_absent",
            "true",
            "the production gate must preserve exact timeout-control identity across a lock-only install and require absence after advance",
        ),
        (
            "timeout_control_key_projection_retargeted",
            Path("crates/iroha_core/src/sumeragi/v2_core/reducer.rs"),
            "transition_projection",
            """timeout_control_before: self
                .outbound_control
                .get(&OutboundControlClass::TimeoutVote),""",
            """timeout_control_before: self
                .outbound_control
                .get(&OutboundControlClass::CommitQc),""",
            "the transition projection must preserve direct timeout-control key occupancy and full message identity",
        ),
        (
            "verus_timeout_owner_projection_disconnected",
            Path("crates/iroha_sumeragi_core/src/verus_proofs.rs"),
            "verified_delta_facts_from_projection",
            "transition_delta_facts_from_projection_body!(",
            "transition_facts_from_components_body!(",
            "Verus must prove the executable delta facts through the same timeout-owner projection used by production",
        ),
        (
            "verus_timeout_owner_extensionality_omitted",
            Path("crates/iroha_sumeragi_core/src/verus_proofs.rs"),
            "production_delta_facts_equal",
            "&& left.timeout_control_after_absent == right.timeout_control_after_absent",
            "&& true",
            "Verus transition-fact extensionality must include every timeout-owner delta field",
        ),
        (
            "verus_same_round_timeout_pool_forced_empty",
            Path("crates/iroha_sumeragi_core/src/verus_proofs.rs"),
            "production_action_preserves_volatile_bounds",
            """&& (if facts.install_view_unchanged {
                    facts.timeout_vote_pool_unchanged
                        && facts.volatile_after.timeout_vote_pools
                            == facts.volatile_before.timeout_vote_pools
                        && facts.volatile_after.timeout_vote_entries
                            == facts.volatile_before.timeout_vote_entries
                } else {
                    facts.volatile_after.timeout_vote_pools == 0
                        && facts.volatile_after.timeout_vote_entries == 0
                })""",
            """&& facts.volatile_after.timeout_vote_pools == 0
                && facts.volatile_after.timeout_vote_entries == 0""",
            "Verus volatile preservation must distinguish lock-only timeout ownership from advancing-view reset",
        ),
        (
            "verus_advancing_timeout_control_retention_accepted",
            Path("crates/iroha_sumeragi_core/src/verus_proofs.rs"),
            "production_action_preserves_volatile_bounds",
            """&& (if facts.install_view_unchanged {
                    facts.timeout_control_unchanged
                } else {
                    facts.timeout_control_after_absent
                })""",
            "&& true",
            "Verus volatile preservation must prove exact timeout control retention or advancing-view absence",
        ),
        (
            "verus_vote_tautology",
            Path("crates/iroha_sumeragi_core/src/verus_proofs.rs"),
            "same_vote_statement",
            "vote_statement_identity_equal_body!(",
            "true || vote_statement_identity_equal_body!(",
            "Verus vote identity must invoke the shared production macro",
        ),
    )
    for case, relative, item, old, new, error_fragment in mutations:
        repo_root = tmp_path / case
        for source_path in source_paths:
            destination = repo_root / source_path
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT_DIR / source_path, destination)
        path = repo_root / relative
        source = path.read_text(encoding="utf-8")
        marker = f"macro_rules! {item} {{"
        start = source.find(marker)
        if start >= 0:
            assert source.count(old) == 1, (item, old)
            mutation = source.find(old)
            assert mutation > start, (item, old)
            path.write_text(
                source[:mutation] + new + source[mutation + len(old) :],
                encoding="utf-8",
            )
        else:
            mutate_rust_item_source(module, path, item, old, new)
        errors = module._same_round_semantic_kernel_source_fidelity_errors(
            repo_root
        )
        assert any(error_fragment in error for error in errors), errors


def test_atomic_timeout_completion_contract_rejects_split_or_stale_projection(
    tmp_path: Path,
) -> None:
    """The local timeout receipt, TC formation, and readiness stay atomic."""

    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    paths = {
        name: formal_dir / name
        for name in ("SumeragiV2Core.tla", "SumeragiV2AsyncNetwork.tla")
    }
    canonical = {}
    for name, destination in paths.items():
        source = (module.FORMAL_DIR / name).read_text(encoding="utf-8")
        destination.write_text(source, encoding="utf-8")
        canonical[name] = source

    assert module._atomic_timeout_completion_source_fidelity_errors(formal_dir) == []

    mutations = (
        (
            "SumeragiV2Core.tla",
            "     /\\ vote.highestPrepareQc = highestPrepareQc[node]\n",
            "",
            "LocalTimeoutCompletionGuard must equal only",
        ),
        (
            "SumeragiV2Core.tla",
            "     /\\ ExactPrepareQcMatchesRef(\n"
            "          vote.highestPrepareQc, vote.highRank, vote.highSubject)\n",
            "     /\\ AuthenticatedHighRef(vote.highRank, vote.highSubject)\n",
            "LocalTimeoutCompletionGuard must equal only",
        ),
        (
            "SumeragiV2Core.tla",
            "  IN /\\ LocalTimeoutCompletionGuard(request)\n",
            "  IN /\\ TRUE\n",
            "CompleteTimeoutSignature must invoke",
        ),
        (
            "SumeragiV2Core.tla",
            "      /\\ entry.node = node\n"
            "      /\\ entry.vote.context = context\n"
            "      /\\ entry.vote.view = roundView}}\n",
            "      /\\ entry.node = node\n"
            "      /\\ entry.vote.view = roundView}}\n",
            "TimeoutVotesIn must equal only",
        ),
        (
            "SumeragiV2AsyncNetwork.tla",
            "CompleteTimeoutSignatureReady(request) ==\n"
            "  LocalTimeoutCompletionGuard(request)\n",
            "CompleteTimeoutSignatureReady(request) == TRUE\n",
            "CompleteTimeoutSignatureReady must equal only",
        ),
    )
    for name, needle, replacement, expected_error in mutations:
        source = canonical[name]
        assert source.count(needle) == 1, needle
        paths[name].write_text(
            source.replace(needle, replacement, 1), encoding="utf-8"
        )
        errors = module._atomic_timeout_completion_source_fidelity_errors(
            formal_dir
        )
        assert any(expected_error in error for error in errors), errors
        paths[name].write_text(source, encoding="utf-8")


def test_adequate_leader_scheduler_readiness_contract_is_bound(
    tmp_path: Path,
) -> None:
    """The exact-leader induction seam accepts the reviewed source."""

    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    shutil.copy2(
        module.FORMAL_DIR
        / "SumeragiV2AdequateLeaderServiceClosureProofs.tla",
        formal_dir / "SumeragiV2AdequateLeaderServiceClosureProofs.tla",
    )

    assert (
        module._adequate_leader_scheduler_readiness_source_fidelity_errors(
            formal_dir
        )
        == []
    )


@pytest.mark.parametrize(
    ("kind", "symbol", "old", "new"),
    (
        (
            "operator",
            "ExactLeaderSchedulerReadinessFrame",
            "  /\\ UNCHANGED up\n",
            "",
        ),
        (
            "operator",
            "ExactLeaderSchedulerReadinessFrame",
            "                 asyncHeldChunks,\n"
            "                 asyncControlServiceState>>",
            "                 asyncHeldChunks>>",
        ),
        (
            "operator",
            "AdequateLeaderTargetNonDescentEpisodeClosureProperty",
            (
                "         /\\ AdequateLeaderTargetProtocolSubjectSource(\n"
                "              target, leaderContext, leader, leaderView, subject)"
            ),
            "         /\\ TRUE",
        ),
        (
            "operator",
            "AdequateLeaderTargetNonDescentEpisodeClosureProperty",
            "           ~> AdequateLeaderTargetOccurrenceRankServiceExitGoal(\n",
            "           ~> AdequateLeaderTargetStrictOccurrenceDescentGoal(\n",
        ),
        (
            "theorem",
            "AsyncNetworkStepPreservesExactLeaderSchedulerOriginReadiness",
            "  /\\ AsyncNext\n  /\\ AsyncNetworkStep\n",
            "  /\\ AsyncNetworkStep\n",
        ),
        (
            "theorem",
            "AsyncNetworkReplacementRetiresReadyOccurrenceIntoAuthenticatedProvenance",
            "    => AuthenticatedLeaderDiscardProvenance(candidate)'\n",
            "    => TRUE\n",
        ),
        (
            "theorem",
            "AuthenticatedExactLeaderTerminalDiscardInstallsClosedTombstone",
            "    /\\ candidate.kind \\notin "
            "AsyncRestartScopedCandidateServiceKinds\n"
            "    /\\ SameConsumerLeaderDiscard(candidate)",
            "    /\\ TRUE\n"
            "    /\\ SameConsumerLeaderDiscard(candidate)",
        ),
        (
            "theorem",
            "AsyncLiveExactLeaderSchedulerOriginReadiness",
            "    ExactLeaderSchedulerOriginReadinessProperty(\n"
            "      AsyncLiveSpecAt(initialContext))",
            "    TRUE",
        ),
        (
            "theorem",
            "AsyncLiveResponsiveExactLeaderSchedulerSourcesAreUp",
            "             /\\ gst\n"
            "             /\\ ExactLeaderCandidateRank(candidate, rank)\n"
            "             => candidate.node \\in up)",
            "             /\\ gst\n"
            "             /\\ ExactLeaderCandidateRank(candidate, rank)\n"
            "             /\\ candidate.node \\in AsyncCurrentResponsiveVoters\n"
            "             => candidate.node \\in up)",
        ),
    ),
)
def test_adequate_leader_scheduler_readiness_contract_rejects_weakening(
    tmp_path: Path,
    kind: str,
    symbol: str,
    old: str,
    new: str,
) -> None:
    """Frame, outer-transition, provenance, and source-scope drift fail."""

    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    path = formal_dir / "SumeragiV2AdequateLeaderServiceClosureProofs.tla"
    source = (
        module.FORMAL_DIR
        / "SumeragiV2AdequateLeaderServiceClosureProofs.tla"
    ).read_text(encoding="utf-8")
    mutate = mutate_tla_operator if kind == "operator" else mutate_tla_theorem
    path.write_text(mutate(source, symbol, old, new), encoding="utf-8")

    errors = (
        module._adequate_leader_scheduler_readiness_source_fidelity_errors(
            formal_dir
        )
    )
    assert any(symbol in error for error in errors), errors
