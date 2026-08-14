def test_arbitrary_context_safety_property_bodies_are_pinned(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    path = formal_dir / "SumeragiV2Proofs.tla"
    source = r"""---- MODULE SumeragiV2Proofs ----
DurableVoteUniquenessProperty(specification) ==
  specification => [](/\ HonestPrepareUniqueness
                       /\ HonestCommitUniqueness
                       /\ HonestTimeoutUniqueness)
LockMonotonicityProperty(specification) ==
  specification => [][LockMonotonicityAction]_vars
ExternalValidityProperty(specification) ==
  specification => [](/\ \A qc \in prepareQCs: qc.subject \in ValidSubjects
                       /\ \A qc \in commitQCs: qc.subject \in ValidSubjects
                       /\ \A decision \in decisions:
                            decision.qc.subject \in ValidSubjects)
CertifiedBodyAvailabilityProperty(specification) ==
  specification => [](/\ PrepareCertificateAvailability
                       /\ CommitCertificateAvailability)
CertificateUniquenessProperty(specification) ==
  specification => []CertificateUniquenessInvariant
PotentialCommitVotes(certificateContext, roundView, subject) ==
  {vote \in commitIntents:
    /\ vote.context = certificateContext
    /\ vote.view = roundView
    /\ vote.phase = "Commit"
    /\ vote.subject = subject}
PotentialCommitSigners(certificateContext, roundView, subject) ==
  {vote.signer:
    vote \in PotentialCommitVotes(
      certificateContext, roundView, subject)}
InstalledTcAuthorizedPotentialCommitIntersection(tc, protectedView, subject) ==
  \E timeoutVote \in tc.votes,
      commitVote \in PotentialCommitVotes(
        tc.context, protectedView, subject):
    /\ timeoutVote.signer \in Honest
    /\ commitVote.signer = timeoutVote.signer
    /\ timeoutVote.context = tc.context
    /\ timeoutVote.view = tc.view
    /\ ~TimeoutVoteStrictlyProtectsCommit(timeoutVote, commitVote)
    /\ InstalledTcAuthorizesCommitVote(commitVote)
TCProtectsOrInstalledTcAuthorizesPotentialCommit(tc) ==
  \A protectedView \in 0..tc.view, subject \in Subjects:
    DualQuorum(tc.context.epoch,
      PotentialCommitSigners(tc.context, protectedView, subject))
      => \/ TCProtectsViewSubject(tc, protectedView, subject)
         \/ InstalledTcAuthorizedPotentialCommitIntersection(
              tc, protectedView, subject)
TimeoutProtectionProperty(specification) ==
  specification
    => [](\A tc \in formedTCs:
          TCProtectsOrInstalledTcAuthorizesPotentialCommit(tc))
AgreementProperty(specification) ==
  specification => []DecisionAgreement
NoConflictingCommitCertificatesProperty(specification) ==
  specification => [](\A left, right \in commitQCs:
    left.context = right.context => left.subject = right.subject)
CrashRecoveryProperty(specification) ==
  /\ (specification => []CrashRecoveryStateInvariant)
  /\ (specification => [][CrashPreservesDurableProjection]_vars)
  /\ (specification => [][RestartPreservesDurableProjection]_vars)
  /\ (specification => [][PendingWritesAreUnacknowledged]_vars)
  /\ (specification =>
        [][TypeInvariant => StaleGenerationRejected]_vars)
=============================================================================
"""
    path.write_text(source, encoding="utf-8")

    assert module._safety_property_source_fidelity_errors(formal_dir) == []

    path.write_text(
        source.replace("/\\ HonestTimeoutUniqueness", "/\\ TRUE")
        .replace(
            "/\\ (specification => [][RestartPreservesDurableProjection]_vars)",
            "/\\ TRUE",
        ),
        encoding="utf-8",
    )
    errors = module._safety_property_source_fidelity_errors(formal_dir)
    assert any("DurableVoteUniquenessProperty must equal only" in error for error in errors)
    assert any("CrashRecoveryProperty must equal only" in error for error in errors)

    path.write_text(
        source.replace(
            "/\\ InstalledTcAuthorizesCommitVote(commitVote)",
            "/\\ TRUE",
        ),
        encoding="utf-8",
    )
    errors = module._safety_property_source_fidelity_errors(formal_dir)
    assert any(
        "InstalledTcAuthorizedPotentialCommitIntersection must equal only" in error
        for error in errors
    )

    path.write_text(
        source.replace(
            "\\/ InstalledTcAuthorizedPotentialCommitIntersection(\n"
            "              tc, protectedView, subject)",
            "\\/ TCProtectsViewSubject(tc, protectedView, subject)",
        ),
        encoding="utf-8",
    )
    errors = module._safety_property_source_fidelity_errors(formal_dir)
    assert any(
        "TCProtectsOrInstalledTcAuthorizesPotentialCommit must equal only" in error
        for error in errors
    )


@pytest.mark.parametrize(
    ("relative", "old", "new", "expected"),
    (
        (
            "SumeragiV2CrashRecovery.tla",
            "    Restart(node) => generation'[node] > generation[node]\n",
            "    Restart(node) => generation'[node] = generation[node]\n",
            "StaleGenerationRejected must equal only",
        ),
        (
            "SumeragiV2Proofs.tla",
            "      => generation'[node] = generation[node] + 1\n",
            "      => generation'[node] = generation[node]\n",
            "RestartIncrementsSelectedGeneration must state only",
        ),
        (
            "SumeragiV2Proofs.tla",
            "BY Isa DEF TypeInvariant, Restart\n",
            "BY SMT DEF TypeInvariant, Restart\n",
            "RestartIncrementsSelectedGeneration must retain its exact reviewed",
        ),
    ),
)
def test_restart_generation_safety_contract_is_pinned(
    tmp_path: Path,
    relative: str,
    old: str,
    new: str,
    expected: str,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    shutil.copytree(module.FORMAL_DIR, formal_dir)
    path = formal_dir / relative
    source = path.read_text(encoding="utf-8")
    assert old in source
    path.write_text(source.replace(old, new, 1), encoding="utf-8")

    errors = module._safety_property_source_fidelity_errors(formal_dir)

    assert any(expected in error for error in errors), errors


def test_liveness_property_contracts_are_semantically_pinned(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "SumeragiV2AsyncLivenessProofs.tla").write_text(
        r"""---- MODULE SumeragiV2AsyncLivenessProofs ----
THEOREM AsyncStepRefinementObligation ==
  AsyncNext => [Next]_vars
BY DEF AsyncNext
THEOREM AsyncTypeInvariantObligation ==
  \A initialContext: AsyncSpecAt(initialContext) => []AsyncTypeInvariant
BY PTL
THEOREM AsyncNextPreservesNormalProposalPrepareCandidate ==
  \A candidate:
    /\ NormalProposalPrepareCandidate(candidate)
    /\ AsyncNext
    => NormalProposalPrepareCandidate(candidate)'
BY PTL
=============================================================================
""",
        encoding="utf-8",
    )
    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    valid = r"""---- MODULE SumeragiV2LivenessProofs ----
ResponsiveNodesDecide ==
  \A node \in AsyncCurrentResponsiveVoters: NodeHasDecision(node)
ResponsiveNodesApply ==
  \A node \in AsyncCurrentResponsiveVoters: NodeHasApplication(node)
ResponsiveHonestLeaderViewReached ==
  \E leader \in (AsyncCurrentResponsiveVoters \cap Honest):
    /\ ~NodeHasDecision(leader)
    /\ Leader(context, nodeView[leader]) = leader
TimeoutViewProgressProperty(specification) ==
  specification => \A node \in AsyncCurrentResponsiveVoters,
    roundView \in Views:
      (gst /\ nodeView[node] = roundView /\ ~NodeHasDecision(node))
        ~> (nodeView[node] > roundView \/ NodeHasDecision(node))
RotatingLeaderProgressProperty(specification) ==
  specification
    => /\ (gst /\ ~ResponsiveNodesDecide)
             ~> (ResponsiveHonestLeaderViewReached
                   \/ ResponsiveNodesDecide)
       /\ (gst /\ ResponsiveHonestLeaderViewReached
                 /\ ~ResponsiveNodesDecide)
             ~> ResponsiveNodesDecide
ApplicationCompletionProgressProperty(specification) ==
  specification
    => \A node \in AsyncCurrentResponsiveVoters:
         (gst /\ NodeHasDecision(node))
           ~> NodeHasApplication(node)
ApplicationLivenessProperty(specification) ==
  specification
    => /\ \A node \in AsyncCurrentResponsiveVoters:
             (gst /\ NodeHasDecision(node))
               ~> NodeHasApplication(node)
       /\ (gst /\ ResponsiveNodesDecide) ~> ResponsiveNodesApply
PostGstProgressActionEnabled ==
  \E node \in AsyncCurrentResponsiveVoters:
    PostGstCommitCertificateDiscovery(node)
=============================================================================
"""
    vocabulary.write_text(valid, encoding="utf-8")
    (formal_dir / "SumeragiV2Proofs.tla").write_text(
        "---- MODULE SumeragiV2Proofs ----\n=============================================================================\n",
        encoding="utf-8",
    )

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary.write_text(
        valid.replace(
            "(gst /\\ NodeHasDecision(node))",
            "(FALSE /\\ gst /\\ NodeHasDecision(node))",
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any("ApplicationLivenessProperty must equal only" in error for error in errors)

    vocabulary.write_text(
        valid.replace(
            "ApplicationCompletionProgressProperty(specification) ==\n"
            "  specification\n"
            "    => \\A node \\in AsyncCurrentResponsiveVoters:\n",
            "ApplicationCompletionProgressProperty(specification) ==\n"
            "  specification\n"
            "    => \\A node \\in ValidatorIds:\n",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ApplicationCompletionProgressProperty must equal only" in error
        for error in errors
    )

    vocabulary.write_text(
        valid.replace(
            "Leader(context, nodeView[leader]) = leader",
            "Leader(context, nodeView[leader]) \\in AsyncCurrentResponsiveVoters",
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ResponsiveHonestLeaderViewReached must equal only" in error
        for error in errors
    )

    vocabulary.write_text(
        valid.replace(
            "(gst /\\ ResponsiveHonestLeaderViewReached\n"
            "                 /\\ ~ResponsiveNodesDecide)",
            "(gst /\\ ~ResponsiveNodesDecide)",
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "RotatingLeaderProgressProperty must equal only" in error
        for error in errors
    )

    vocabulary.write_text(valid, encoding="utf-8")
    (formal_dir / "SumeragiV2Proofs.tla").write_text(
        "---- MODULE SumeragiV2Proofs ----\n"
        "NodeHasDecision(node) == TRUE\n"
        "HeightLivenessProperty(specification) == specification\n"
        "=============================================================================\n",
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any("asynchronous liveness symbol NodeHasDecision" in error for error in errors)
    assert any(
        "asynchronous liveness symbol HeightLivenessProperty" in error
        for error in errors
    )

    fidelity_dir = tmp_path / "application-fidelity"
    fidelity_dir.mkdir()
    for filename in (
        "SumeragiV2AsyncLivenessProofs.tla",
        "SumeragiV2LivenessProofs.tla",
    ):
        shutil.copyfile(module.FORMAL_DIR / filename, fidelity_dir / filename)
    assert module._application_completion_source_fidelity_errors(fidelity_dir) == []

    proof_path = fidelity_dir / "SumeragiV2AsyncLivenessProofs.tla"
    proof_source = proof_path.read_text(encoding="utf-8")
    proof_path.write_text(
        proof_source.replace(
            "         ApplicationCompletionReachesEveryResponsivePrefix\n",
            "         ApplicationCompletionProgressAppliesFixedResponsiveNode\n",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._application_completion_source_fidelity_errors(fidelity_dir)
    assert any(
        "proof must compose the reviewed application dependencies in order"
        in error
        for error in errors
    )

    proof_path.write_text(
        mutate_tla_theorem(
            proof_source,
            "ApplicationLivenessObligation",
            "PROOF\n",
            "OBVIOUS\n",
        ),
        encoding="utf-8",
    )
    errors = module._application_completion_source_fidelity_errors(fidelity_dir)
    assert any(
        "must have the reviewed deductive application-completion proof" in error
        for error in errors
    )

    proof_path.write_text(
        mutate_tla_theorem(
            proof_source,
            "ApplicationCompletionProgressObligation",
            "StarvationFreedomObligation",
            "ApplicationCompletionProgressObligation",
        ),
        encoding="utf-8",
    )
    errors = module._application_completion_source_fidelity_errors(fidelity_dir)
    assert any(
        "must compose the reviewed exact-corridor dependencies in order" in error
        for error in errors
    )

    proof_path.write_text(
        mutate_tla_theorem(
            proof_source,
            "DecisionPipelineStagePersistsUntilExactHandoff",
            "ExecuteApply",
            "ExecuteSignVote",
        ),
        encoding="utf-8",
    )
    errors = module._application_completion_source_fidelity_errors(fidelity_dir)
    assert any(
        "DecisionPipelineStagePersistsUntilExactHandoff proof must retain exact"
        in error
        and "ExecuteApply" in error
        for error in errors
    )

    proof_path.write_text(
        mutate_tla_theorem(
            proof_source,
            "ActiveDecisionCertifiedRequestReachesCertifiedFetch",
            "PostGstAdmitHiddenPacket",
            "PostGstAdmitHistoricalRecoveryPacket",
        ),
        encoding="utf-8",
    )
    errors = module._application_completion_source_fidelity_errors(fidelity_dir)
    assert any(
        "ActiveDecisionCertifiedRequestReachesCertifiedFetch proof must retain exact"
        in error
        and "PostGstAdmitHiddenPacket" in error
        for error in errors
    )

    proof_path.write_text(
        mutate_tla_theorem(
            proof_source,
            "ResponsiveDecisionReachesApplicationFromExactCorridor",
            "RecoveryAwareDecisionWitnessProjectsApplicationFrontier",
            "HistoricalDecisionConcreteLeafProperties",
        ),
        encoding="utf-8",
    )
    errors = module._application_completion_source_fidelity_errors(fidelity_dir)
    assert any(
        "may not rely on the application result itself" in error
        and "HistoricalDecisionConcreteLeafProperties" in error
        for error in errors
    )

    proof_path.write_text(
        mutate_tla_operator(
            proof_source,
            "DecisionPipelineKinds",
            '"Apply"',
            '"SignVote"',
        ),
        encoding="utf-8",
    )
    errors = module._application_completion_source_fidelity_errors(fidelity_dir)
    assert any(
        "DecisionPipelineKinds must equal only" in error for error in errors
    )


def test_scheduler_rank_derivation_cannot_widen_the_owned_carrier(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_async_liveness_shard_fixture(tmp_path, module)
    (formal_dir / "proof_coverage.json").write_text("{}\n", encoding="utf-8")
    path = async_liveness_symbol_path(
        formal_dir,
        module,
        "ScheduledCandidateServiceRankInCarrier",
    )
    source = path.read_text(encoding="utf-8")

    assert module._async_proof_architecture_errors(formal_dir) == []

    path.write_text(
        source.replace(
            "OwnedServiceRankCarrier",
            "ServiceRankCarrier",
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ScheduledCandidateServiceRankInCarrier must use "
        "OwnedServiceRankCarrier" in error
        for error in errors
    )
    assert any(
        "may not widen scheduler-owned rank proofs" in error for error in errors
    )


def test_liveness_service_ownership_stays_on_the_fair_node_domain(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    guarded_owner = (
        "ResponsiveProtectedCandidateOwned(candidate) ==\n"
        "  /\\ candidate.node \\in AsyncCurrentResponsiveVoters\n"
        "  /\\ ProtectedCandidateOwned(candidate)"
    )
    assert guarded_owner in source
    vocabulary.write_text(
        source.replace(
            guarded_owner,
            "ResponsiveProtectedCandidateOwned(candidate) ==\n"
            "  ProtectedCandidateOwned(candidate)",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ResponsiveProtectedCandidateOwned must equal only" in error
        for error in errors
    )


def test_protected_service_rank_excludes_transport_and_ingress_stages(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    assert "stage \\in 2..6, position \\in Nat:" in source
    property_offset = source.index(
        "ProtectedServiceRankProgressProperty(specification) =="
    )
    stage_offset = source.index(
        "stage \\in 2..6, position \\in Nat:", property_offset
    )
    vocabulary.write_text(
        source[:stage_offset]
        + source[stage_offset:].replace(
            "stage \\in 2..6, position \\in Nat:",
            "stage \\in 0..8, position \\in Nat:",
            1,
        ),
        encoding="utf-8",
    )

    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "ProtectedServiceRankProgressProperty must equal only" in error
        for error in errors
    )

    vocabulary.write_text(
        source.replace(
            "                           ELSE <<0, 0>>",
            "                           ELSE IF CandidateInIngress(candidate)\n"
            "                                THEN <<7, 1>>\n"
            "                                ELSE <<0, 0>>",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "CandidateServiceRank must be scheduler-owned stages 2..6" in error
        for error in errors
    )


def test_liveness_configuration_typing_premises_are_pinned(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = copy_flat_async_architecture_fixture(tmp_path, module)

    assert module._async_proof_architecture_errors(formal_dir) == []

    vocabulary = formal_dir / "SumeragiV2LivenessProofs.tla"
    source = vocabulary.read_text(encoding="utf-8")
    typed_budget_premise = (
        "THEOREM RetransmissionBudgetCoversEveryClass ==\n"
        "  ModelConfiguration /\\ AsyncConfiguration"
    )
    assert typed_budget_premise in source
    vocabulary.write_text(
        source.replace(
            typed_budget_premise,
            "THEOREM RetransmissionBudgetCoversEveryClass ==\n"
            "  AsyncConfiguration",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "RetransmissionBudgetCoversEveryClass must state only" in error
        for error in errors
    )

    typed_successor_premise = (
        "THEOREM CanonicalSuccessorPreservesAdmissibility ==\n"
        "  ModelConfiguration\n"
        "    => \\A initialContext"
    )
    assert typed_successor_premise in source
    vocabulary.write_text(
        source.replace(
            typed_successor_premise,
            "THEOREM CanonicalSuccessorPreservesAdmissibility ==\n"
            "  \\A initialContext",
            1,
        ),
        encoding="utf-8",
    )
    errors = module._async_proof_architecture_errors(formal_dir)
    assert any(
        "CanonicalSuccessorPreservesAdmissibility must state only" in error
        for error in errors
    )


def test_verus_runner_records_output_without_masking_failures() -> None:
    source = (ROOT_DIR / "scripts" / "verify_sumeragi_v2.sh").read_text(
        encoding="utf-8"
    )

    assert "set -euo pipefail" in source
    assert "target/formal/sumeragi_v2/verus.log" in source
    assert '2>&1 | tee -a "$verus_log_tmp"' in source
    assert 'verus_pipeline_status=("${PIPESTATUS[@]}")' in source
    assert "sumeragi_v2_verus_evidence.py" in source


def test_tla_shortcut_scan_rejects_unchecked_constructs_but_allows_proof_assume(
    tmp_path: Path,
) -> None:
    module = load_checker()
    path = tmp_path / "Example.tla"
    source = r"""---- MODULE Example ----
ASSUME Unsafe
AXIOM Hidden
THEOREM Broken == TRUE BY OMITTED
THEOREM StructuredStatement ==
  ASSUME NEW value \in BOOLEAN
  PROVE value \/ ~value
BY PTL
THEOREM Structured == TRUE
PROOF
  <1>1. ASSUME TRUE
         PROVE TRUE
    OBVIOUS
  <1> QED BY <1>1
=============================================================================
"""

    errors = module.tla_shortcut_errors(path, source)
    assert len(errors) == 3
    assert any("top-level ASSUME" in error for error in errors)
    assert any("top-level AXIOM" in error for error in errors)
    assert any("OMITTED proof" in error for error in errors)


def test_tla_shortcut_scan_does_not_let_unproved_or_misplaced_assumptions_hide(
    tmp_path: Path,
) -> None:
    module = load_checker()
    path = tmp_path / "Adversarial.tla"
    source = r"""---- MODULE Adversarial ----
THEOREM UnprovedStructured ==
  ASSUME NEW value \in BOOLEAN
  PROVE value \/ ~value

THEOREM StatementEnded == TRUE
ASSUME SmuggledBetweenStatementAndProof
BY DEF StatementEnded

ASSUMPTION ModuleLevel
=============================================================================
"""

    errors = module.tla_shortcut_errors(path, source)
    assert len(errors) == 3
    assert any("UnprovedStructured" not in error and ":3:" in error for error in errors)
    assert any(":7:" in error and "ASSUME" in error for error in errors)
    assert any(":10:" in error and "ASSUMPTION" in error for error in errors)


def test_tla_shortcut_scan_ignores_comments_and_nested_comments(tmp_path: Path) -> None:
    module = load_checker()
    path = tmp_path / "Example.tla"
    source = """---- MODULE Example ----
(* ASSUME CommentOnly (* AXIOM Nested *) OMITTED *)
\\* AXIOM line comment
Safe == "OMITTED"
=============================================================================
"""

    assert module.tla_shortcut_errors(path, source) == []


def test_tla_shortcut_scan_rejects_obsolete_unsound_tlaps_rules(
    tmp_path: Path,
) -> None:
    module = load_checker()
    path = tmp_path / "ObsoleteRules.tla"
    source = r"""---- MODULE ObsoleteRules ----
THEOREM BareFairness == TRUE BY WF1
THEOREM UndefinedFairness == TRUE BY RuleWF1
THEOREM UndefinedInvariant == TRUE BY RuleINV1
\* WF1 RuleWF1 RuleINV1 in comments must not trigger the scanner.
IgnoredStrings == "WF1 RuleWF1 RuleINV1"
=============================================================================
"""

    errors = module.tla_shortcut_errors(path, source)
    assert len(errors) == 3
    assert all("obsolete or undefined TLAPS rule" in error for error in errors)
    for token in ("WF1", "RuleWF1", "RuleINV1"):
        assert any(f"rule {token} is prohibited" in error for error in errors)


def test_retired_favourable_network_liveness_corridor_is_rejected(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "Example.tla").write_text(
        "---- MODULE Example ----\n"
        "ReliableNext == TRUE\n"
        "StableProgressContracts == TRUE\n"
        "\\* ReliableBeginTimeout in a comment is harmless\n"
        "=============================================================================\n",
        encoding="utf-8",
    )

    errors = module._retired_liveness_errors(formal_dir)
    assert len(errors) == 2
    assert any("ReliableNext" in error for error in errors)
    assert any("StableProgressContracts" in error for error in errors)


def test_deductive_max_view_dependency_is_rejected(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "SumeragiV2Core.tla").write_text(
        "---- MODULE SumeragiV2Core ----\n"
        "CONSTANTS\n"
        "  MaxView,\n"
        "  ViewDomain\n"
        "FiniteViews == 0..MaxView\n"
        "=============================================================================\n",
        encoding="utf-8",
    )
    (formal_dir / "Proof.tla").write_text(
        "---- MODULE Proof ----\n"
        "THEOREM BadBound == tc.view < MaxView\n"
        "=============================================================================\n",
        encoding="utf-8",
    )

    errors = module._bounded_view_dependency_errors(formal_dir)
    assert len(errors) == 1
    assert "Proof.tla:2" in errors[0]
    assert "reserved for FiniteViews/TLC scaffolding" in errors[0]


def test_reachable_core_actions_cannot_assume_proof_history_oracles(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "SumeragiV2Core.tla").write_text(
        "---- MODULE SumeragiV2Core ----\n"
        "FormPrepareQC(node, view, subject) ==\n"
        "  /\\ CertificateHonestIntentBacked(qc, prepareIntents)\n"
        "  /\\ TRUE\n"
        "FormCommitQC(node, view, subject) == TRUE\n"
        "DeliverQC(envelope) == QcValid(envelope.qc)\n"
        "BeginTimeout(node) == HighRefValid(highRank[node], highSubject[node])\n"
        "=============================================================================\n",
        encoding="utf-8",
    )

    errors = module._reachable_oracle_guard_errors(formal_dir)
    assert len(errors) == 3
    assert any(
        "FormPrepareQC" in error and "CertificateHonestIntentBacked" in error
        for error in errors
    )
    assert any("DeliverQC" in error and "QcValid" in error for error in errors)
    assert any(
        "BeginTimeout" in error and "HighRefValid" in error for error in errors
    )


def test_reachable_core_actions_allow_wire_and_local_durable_guards(
    tmp_path: Path,
) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "SumeragiV2Core.tla").write_text(
        "---- MODULE SumeragiV2Core ----\n"
        "FormPrepareQC(node, view, subject) == QcWireValid(qc)\n"
        "FormCommitQC(node, view, subject) == QcWireValid(qc)\n"
        "DeliverQC(envelope) == QcWireValid(envelope.qc)\n"
        "BeginTimeout(node) == LocalTimeoutVoteFor(node).highRank = highestRank[node]\n"
        "=============================================================================\n",
        encoding="utf-8",
    )

    assert module._reachable_oracle_guard_errors(formal_dir) == []


def test_async_deductive_and_finite_specs_cannot_be_conflated(tmp_path: Path) -> None:
    module = load_checker()
    formal_dir = tmp_path / "formal"
    formal_dir.mkdir()
    (formal_dir / "SumeragiV2Core.tla").write_text(
        "---- MODULE SumeragiV2Core ----\n"
        "vars == <<coreState>>\n"
        "=============================================================================\n",
        encoding="utf-8",
    )
    canonical = """---- MODULE SumeragiV2AsyncNetwork ----
AsyncSchedulerVars == <<schedulerState>>
AsyncRecoveryVars == <<recoveryPhase, recoveryQueue>>
AsyncProducerVars == <<producerKnown, producerConsumed, producerHistory>>
AsyncAllVars == <<gst, vars, AsyncSchedulerVars, AsyncRecoveryVars, AsyncProducerVars, asyncFixedCorridorDeadlines, asyncServeProducerEpisodeDue>>
AsyncFairnessAt(initialContext) == WF_AsyncAllVars(AsyncNext)
AsyncFairness == AsyncFairnessAt(ContextRecord(0, <<>>))
AsyncBaseInitAt(initialContext) == TRUE
AsyncBaseInit == AsyncBaseInitAt(ContextRecord(0, <<>>))
AsyncInitAt(initialContext) == AsyncBaseInitAt(initialContext) /\\ ViewDomain = Nat
AsyncInit == AsyncInitAt(ContextRecord(0, <<>>))
AsyncFiniteInitAt(initialContext) == AsyncBaseInitAt(initialContext) /\\ ViewDomain = FiniteViews
AsyncFiniteInit == AsyncFiniteInitAt(ContextRecord(0, <<>>))
AsyncSpec == AsyncInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairness
AsyncSpecAt(initialContext) == AsyncInitAt(initialContext) /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairnessAt(initialContext)
AsyncFiniteSpec == AsyncFiniteInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairness
AsyncFiniteSpecAt(initialContext) == AsyncFiniteInitAt(initialContext) /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairnessAt(initialContext)
=============================================================================
"""
    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    path.write_text(canonical, encoding="utf-8")

    assert module._async_spec_shape_errors(formal_dir) == []

    path.write_text(
        canonical.replace(
            "AsyncSpec == AsyncInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairness",
            "AsyncSpec == AsyncFiniteInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairness",
        ),
        encoding="utf-8",
    )
    errors = module._async_spec_shape_errors(formal_dir)
    assert any("AsyncSpec must equal only" in error for error in errors)
