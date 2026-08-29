# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

def _successor_stale_token_mutation_source_fidelity_errors(
    formal_dir: Path,
) -> list[str]:
    """Pin the two-state stale-token successor-start mutation witness."""
    model_path = formal_dir / "SumeragiV2SuccessorStaleTokenMutation.tla"
    bug_cfg_path = formal_dir / "successor_stale_token_bug.cfg"
    fixed_cfg_path = formal_dir / "successor_stale_token_fixed.cfg"
    errors: list[str] = []
    for path in (model_path, bug_cfg_path, fixed_cfg_path):
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: missing required successor stale-token mutation artifact"
            )
    if not model_path.is_file() or model_path.is_symlink():
        return errors
    source = model_path.read_text(encoding="utf-8")
    def require_operator(
        symbol: str,
        *,
        required: tuple[str, ...] = (),
        forbidden: tuple[str, ...] = (),
        exact: str | None = None,
    ) -> None:
        extracted = _top_level_operator_body(
            source, symbol, preserve_string_contents=True
        )
        if extracted is None:
            errors.append(f"{model_path}: missing mutation operator {symbol}")
            return
        body, line = extracted
        normalized = " ".join(body.split())
        if exact is not None and normalized != exact:
            errors.append(
                f"{model_path}:{line}: mutation operator {symbol} must equal "
                f"only {exact!r}; found {normalized!r}"
            )
        missing = [token for token in required if token not in normalized]
        if missing:
            errors.append(
                f"{model_path}:{line}: mutation operator {symbol} omits "
                f"required stale-token behavior {missing}"
            )
        present = [token for token in forbidden if token in normalized]
        if present:
            errors.append(
                f"{model_path}:{line}: mutation operator {symbol} contains "
                f"prohibited repaired behavior {present}"
            )
    require_operator(
        "AppliedSuccessorActivationToken",
        exact=(
            '[kind |-> "Applied", parentContext |-> "Parent", '
            'node |-> "Node", successorContext |-> "Successor"]'
        ),
    )
    require_operator(
        "ExactDurableParentApplicationWitness",
        exact="TRUE",
    )
    require_operator(
        "SuccessorActivationPipelineDistance",
        required=(
            'CASE activationStatus = "Queued" -> 10',
            '/\\ activationStatus = "Running" /\\ '
            "~SuccessorActivationCredentialReady -> 9",
            "/\\ SuccessorActivationCredentialReady /\\ "
            "activationPrerequisites = {} -> 8",
            "[] OTHER -> 0",
        ),
    )
    require_operator(
        "SuccessorActivationRank",
        exact="SuccessorActivationPipelineDistance",
    )
    require_operator(
        "MutationTypeInvariant",
        required=(
            "activationPrerequisites \\subseteq "
            "SuccessorActivationRequiredPrerequisites",
            "activationTokens \\subseteq {AppliedSuccessorActivationToken}",
            'lastTransition \\in {"Initial", "BuggyBegin", "FixedBegin", '
            '"FixedReject", "AppliedFailure"}',
            "previousRank \\in 0..10",
        ),
    )
    require_operator(
        "SuccessorActivationProtocolInvariantProjection",
        exact=(
            "/\\ MutationTypeInvariant "
            "/\\ ExactDurableParentApplicationWitness "
            "/\\ (activationFailurePresent => "
            'activationStatus = "Running") '
            "/\\ SuccessorActivationPipelineDistance \\in 1..10"
        ),
    )
    require_operator(
        "StaleAppliedTokenState",
        exact=(
            '/\\ activationStatus = "Queued" '
            '/\\ predecessorOwnership = "Published" '
            '/\\ activationPrerequisites = {"IngressOpen"} '
            "/\\ activationTokens = {AppliedSuccessorActivationToken} "
            "/\\ activationFailurePresent = FALSE "
            "/\\ activationFailureHistoryPresent = FALSE"
        ),
    )
    require_operator(
        "StaleAppliedTokenInit",
        exact=(
            '/\\ StaleAppliedTokenState /\\ lastTransition = "Initial" '
            "/\\ previousRank = 10"
        ),
    )
    require_operator(
        "BuggyBeginSuccessorActivation",
        required=(
            'activationStatus = "Queued"',
            'predecessorOwnership = "Published"',
            "ExactDurableParentApplicationWitness",
            'activationStatus\' = "Running"',
            'lastTransition\' = "BuggyBegin"',
            "previousRank' = SuccessorActivationRank",
        ),
        forbidden=(
            "activationPrerequisites = {}",
            "AppliedSuccessorActivationToken \\notin activationTokens",
        ),
    )
    require_operator(
        "FixedBeginSuccessorActivation",
        required=(
            'activationStatus = "Queued"',
            'predecessorOwnership = "Published"',
            "ExactDurableParentApplicationWitness",
            "activationPrerequisites = {}",
            "AppliedSuccessorActivationToken \\notin activationTokens",
            'activationStatus\' = "Running"',
        ),
    )
    require_operator(
        "FixedRejectStaleSuccessorActivation",
        exact=(
            '/\\ StaleAppliedTokenState '
            '/\\ lastTransition = "Initial" '
            '/\\ lastTransition\' = "FixedReject" '
            "/\\ previousRank' = SuccessorActivationRank "
            "/\\ UNCHANGED <<activationStatus, predecessorOwnership, "
            "activationPrerequisites, activationTokens, "
            "activationFailurePresent, activationFailureHistoryPresent>>"
        ),
    )
    require_operator(
        "MutationLatchAppliedSuccessorStartupFailure",
        required=(
            'activationStatus = "Running"',
            'predecessorOwnership = "Published"',
            "~activationFailurePresent",
            "activationPrerequisites' = {}",
            "activationTokens' = {}",
            "activationFailurePresent' = TRUE",
            "activationFailureHistoryPresent' = TRUE",
            'lastTransition\' = "AppliedFailure"',
            "previousRank' = SuccessorActivationRank",
            "UNCHANGED <<activationStatus, predecessorOwnership>>",
        ),
    )
    exact_operators = {
        "StaleBuggyBeginIsEnabled": (
            "StaleAppliedTokenState => ENABLED "
            "BuggyBeginSuccessorActivation"
        ),
        "StaleFixedBeginIsDisabled": (
            "StaleAppliedTokenState => ~ENABLED "
            "FixedBeginSuccessorActivation"
        ),
        "StaleAppliedFailureIsDisabled": (
            "StaleAppliedTokenState => ~ENABLED "
            "MutationLatchAppliedSuccessorStartupFailure"
        ),
        "InitialStaleRejectionIsEnabled": (
            '(/\\ StaleAppliedTokenState /\\ lastTransition = "Initial") '
            "=> ENABLED FixedRejectStaleSuccessorActivation"
        ),
        "BuggyBeginViolationWitness": (
            'lastTransition = "BuggyBegin" => '
            "~SuccessorActivationProtocolInvariantProjection"
        ),
        "FixedRejectPreservesStaleState": (
            'lastTransition = "FixedReject" => StaleAppliedTokenState'
        ),
        "AppliedFailurePreservesRunningWitness": (
            'lastTransition = "AppliedFailure" => '
            'activationStatus = "Running"'
        ),
        "BugMutationNext": "BuggyBeginSuccessorActivation",
        "BugMutationSpec": (
            "StaleAppliedTokenInit /\\ "
            "[][BugMutationNext]_MutationVars"
        ),
        "FixedMutationNext": (
            "\\/ FixedBeginSuccessorActivation "
            "\\/ FixedRejectStaleSuccessorActivation "
            "\\/ MutationLatchAppliedSuccessorStartupFailure"
        ),
        "FixedMutationSpec": (
            "StaleAppliedTokenInit /\\ "
            "[][FixedMutationNext]_MutationVars"
        ),
    }
    for symbol, exact in exact_operators.items():
        require_operator(symbol, exact=exact)
    cfg_contracts = {
        bug_cfg_path: (
            "SPECIFICATION BugMutationSpec",
            "CHECK_DEADLOCK FALSE",
            "INVARIANT MutationTypeInvariant",
            "INVARIANT StaleBuggyBeginIsEnabled",
            "INVARIANT StaleFixedBeginIsDisabled",
            "INVARIANT StaleAppliedFailureIsDisabled",
            "INVARIANT BuggyBeginViolationWitness",
            "INVARIANT SuccessorActivationProtocolInvariantProjection",
        ),
        fixed_cfg_path: (
            "SPECIFICATION FixedMutationSpec",
            "CHECK_DEADLOCK FALSE",
            "INVARIANT MutationTypeInvariant",
            "INVARIANT StaleFixedBeginIsDisabled",
            "INVARIANT StaleAppliedFailureIsDisabled",
            "INVARIANT InitialStaleRejectionIsEnabled",
            "INVARIANT FixedRejectPreservesStaleState",
            "INVARIANT AppliedFailurePreservesRunningWitness",
            "INVARIANT SuccessorActivationProtocolInvariantProjection",
        ),
    }
    for cfg_path, expected_lines in cfg_contracts.items():
        if not cfg_path.is_file() or cfg_path.is_symlink():
            continue
        actual_lines = tuple(
            line.strip()
            for line in cfg_path.read_text(encoding="utf-8").splitlines()
            if line.strip() and not line.lstrip().startswith("\\*")
        )
        if actual_lines != expected_lines:
            errors.append(
                f"{cfg_path}: successor stale-token mutation configuration "
                f"must equal {expected_lines!r}; found {actual_lines!r}"
            )
    return errors

def _successor_activation_rank_source_fidelity_errors(
    formal_dir: Path,
) -> list[str]:
    """Pin the exact finite-rank corridor used by successor liveness."""

    proof_path = formal_dir / "SumeragiV2SuccessorActivationRefinementProofs.tla"
    if not proof_path.is_file():
        return []

    source = proof_path.read_text(encoding="utf-8")
    errors: list[str] = []
    operator_contracts = {
        "SuccessorActivationRankCarrier": "0..21",
        "SuccessorActivationPipelineDistance": " ".join(
            r'''
            LET successorContext ==
                  CanonicalIndexedContext(parentContext.height + 1)
                marker ==
                  SuccessorActivationMarker(parentContext, node, successorContext)
            IN CASE successorActivationStatus[parentContext][node] = "Queued" -> 10
               [] /\ successorActivationStatus[parentContext][node] = "Running"
                  /\ ~SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                      -> 9
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node] = {}
                      -> 8
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node]
                       = SuccessorActivationAdapterPrerequisites
                      -> 7
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node]
                       = SuccessorActivationRuntimePrerequisites
                      -> 6
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node]
                       = SuccessorActivationServicePrerequisites
                      -> 5
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node]
                       = SuccessorActivationStartupPrerequisites
                      -> 4
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node]
                       = SuccessorActivationClockPrerequisites
                  /\ marker \notin preparedSuccessorActivationMarkers
                      -> 3
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node]
                       = SuccessorActivationClockPrerequisites
                  /\ marker \in preparedSuccessorActivationMarkers
                      -> 2
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node]
                       = SuccessorActivationRequiredPrerequisites
                      -> 1
               [] OTHER -> 0
            '''.split()
        ),
        "SuccessorActivationRank": (
            "IF SuccessorPublicationOrSuperseded(parentContext, node) THEN 0 "
            "ELSE IF successorPredecessorStatusOwnership[parentContext][node] "
            '= "Published" THEN 11 + '
            "SuccessorActivationPipelineDistance(parentContext, node) "
            "ELSE SuccessorActivationPipelineDistance(parentContext, node)"
        ),
        "SuccessorActivationPending": (
            "IndexedSuccessorActivationPending(parentContext, node)"
        ),
        "SuccessorActivationHasDurableParentWitness": (
            "/\\ \\E application \\in Chain!DecisionEvidenceSet: "
            "ExactDurableParentApplication(parentContext, node, application)"
        ),
        "SuccessorActivationAtRank": (
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationRank(parentContext, node) = rank"
        ),
        "SuccessorActivationFailureAbsent": (
            "SuccessorActivationOwner(parentContext, node) "
            "\\notin successorActivationFailures"
        ),
        "SuccessorActivationPendingStructureProperty": (
            "[](\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "SuccessorActivationPending(parentContext, node) "
            "=> /\\ SuccessorActivationHasDurableParentWitness( "
            "parentContext, node) "
            "/\\ SuccessorActivationPipelineDistance(parentContext, node) "
            "\\in 1..10 "
            "/\\ SuccessorActivationRank(parentContext, node) "
            "\\in SuccessorActivationRankCarrier "
            "/\\ ENABLED <<IndexedSuccessorActivationProgressStep( "
            "parentContext, node)>>_(IndexedChainVars))"
        ),
        "SuccessorActivationStepDecreasesRankProperty": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "[][ /\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)' "
            "/\\ IndexedSuccessorActivationProgressStep(parentContext, node) "
            "=> \\/ SuccessorPublicationOrSuperseded(parentContext, node)' "
            "\\/ /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationRank(parentContext, node)' "
            "< SuccessorActivationRank(parentContext, node) "
            "]_IndexedChainVars"
        ),
        "SuccessorActivationPendingIsNotOrphanedProperty": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "[][ /\\ SuccessorActivationPending(parentContext, node) "
            "/\\ [IndexedChainNext]_IndexedChainVars "
            "=> \\/ SuccessorPublicationOrSuperseded(parentContext, node)' "
            "\\/ SuccessorActivationPending(parentContext, node)' "
            "]_IndexedChainVars"
        ),
        "SuccessorActivationOutcomeIsStableProperty": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "[][ /\\ SuccessorPublicationOrSuperseded(parentContext, node) "
            "/\\ [IndexedChainNext]_IndexedChainVars "
            "=> SuccessorPublicationOrSuperseded(parentContext, node)' "
            "]_IndexedChainVars"
        ),
        "SuccessorActivationRankProgressProperty": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "rank \\in SuccessorActivationRankCarrier: "
            "SuccessorActivationAtRank(parentContext, node, rank) "
            "~> (SuccessorPublicationOrSuperseded(parentContext, node) "
            "\\/ \\E lower \\in SetLessThan( rank, OpToRel(<, Nat), "
            "SuccessorActivationRankCarrier): "
            "SuccessorActivationAtRank(parentContext, node, lower))"
        ),
        "SuccessorActivationStarvationFreedomProperty": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "SuccessorActivationPending(parentContext, node) "
            "~> SuccessorPublicationOrSuperseded(parentContext, node)"
        ),
        "SuccessorActivationTemporalKernel": (
            "/\\ []IndexedCompositionInvariant "
            "/\\ []SuccessorActivationProtocolInvariant "
            "/\\ [][IndexedChainNext]_IndexedChainVars "
            "/\\ WF_IndexedChainVars( "
            "IndexedSuccessorActivationProgressStep(parentContext, node))"
        ),
        "SuccessorActivationFailureFreeSuffix": (
            "[]SuccessorActivationFailureAbsent(parentContext, node)"
        ),
        "FailedSuccessorStartupRestartStep": (
            "\\E successorContext \\in AdmissibleContextRecords, "
            "application \\in Chain!DecisionEvidenceSet: "
            "RehydrateFailedSuccessorStartup( "
            "parentContext, node, successorContext, application)"
        ),
    }
    for symbol, exact_body in operator_contracts.items():
        extracted = _top_level_operator_body(
            source, symbol, preserve_string_contents=True
        )
        if extracted is None:
            errors.append(f"{proof_path}: missing successor-rank operator {symbol}")
            continue
        body, line = extracted
        normalized = " ".join(body.split())
        if normalized != exact_body:
            errors.append(
                f"{proof_path}:{line}: {symbol} must equal only "
                f"{exact_body!r}; found {normalized!r}"
            )

    theorem_contracts = {
        "SuccessorActivationPendingRankTierClassification": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ SuccessorActivationShape "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "=> \\/ /\\ successorPredecessorStatusOwnership"
            "[parentContext][node] = \"Published\" "
            "/\\ SuccessorActivationRank(parentContext, node) \\in 12..21 "
            "\\/ /\\ successorPredecessorStatusOwnership"
            "[parentContext][node] = \"Absent\" "
            "/\\ SuccessorActivationRank(parentContext, node) \\in 1..10",
            (
                "SuccessorActivationShape",
                "SuccessorActivationProtocolInvariant",
                "SuccessorActivationRank",
                "Isa",
            ),
        ),
        "ExactDurableParentApplicationHasAdmissibleSuccessorContext": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in ValidatorIds, "
            "application \\in Chain!DecisionEvidenceSet: "
            "/\\ Chain!ChainEpochInvariant "
            "/\\ ExactDurableParentApplication(parentContext, node, application) "
            "=> CanonicalIndexedContext(parentContext.height + 1) "
            "\\in AdmissibleContextRecords",
            (
                "Chain!ChainEpochTypeInvariant",
                "Chain!NodesDoNotOutrunCertificates",
                "Chain!CertifiedPrefixBacked",
                "FrozenContextAdmissible",
                "Isa",
            ),
        ),
        "SuccessorActivationProgressPreservesProtocolInvariant": (
            "\\A selectedParent \\in AdmissibleContextRecords, "
            "selectedNode \\in ValidatorIds: "
            "Chain!ChainEpochInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ IndexedSuccessorActivationProgressStep( "
            "selectedParent, selectedNode) "
            "=> SuccessorActivationProtocolInvariant'",
            (
                "ExactDurableParentApplicationHasAdmissibleSuccessorContext",
                "ExpandENABLED",
                "Isa",
            ),
        ),
        "IndexedActionPreservesSuccessorActivationProtocolInvariant": (
            "IndexedCompositionInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ IndexedChainNext "
            "=> SuccessorActivationProtocolInvariant'",
            (
                "IndexedProductActionPreservesSuccessorActivationProtocolInvariant",
                "SuccessorActivationProgressPreservesProtocolInvariant",
                "DEF IndexedCompositionInvariant",
            ),
        ),
        "CleanCompleteTipRestartDescendsPublishedTier": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "successorContext \\in AdmissibleContextRecords, "
            "application \\in Chain!DecisionEvidenceSet: "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ RehydrateCleanCompleteTipSuccessorStartup( "
            "parentContext, node, successorContext, application) "
            "=> /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationRank(parentContext, node)' "
            "< SuccessorActivationRank(parentContext, node)",
            (
                "CleanCompleteTipRestartCrossesPublishedToAbsentTier",
                "Isa",
            ),
        ),
        "FailureFreeBracketExcludesSuccessorResetActions": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)' "
            "=> /\\ ~SuccessorStartupFailureStep(parentContext, node) "
            "/\\ ~FailedSuccessorStartupRestartStep(parentContext, node)",
            (
                "SuccessorStartupFailureStep",
                "FailedSuccessorStartupRestartStep",
                "LatchAppliedSuccessorStartupFailure",
                "LatchRecoveredSuccessorStartupFailure",
                "RehydrateFailedSuccessorStartup",
                "Isa",
            ),
        ),
        "CleanCompleteTipRestartCrossesPublishedToAbsentTier": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "successorContext \\in AdmissibleContextRecords, "
            "application \\in Chain!DecisionEvidenceSet: "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ RehydrateCleanCompleteTipSuccessorStartup( "
            "parentContext, node, successorContext, application) "
            "=> /\\ SuccessorActivationRank(parentContext, node) \\in 12..21 "
            "/\\ successorPredecessorStatusOwnership'[parentContext][node] "
            "= \"Absent\" "
            "/\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationRank(parentContext, node)' = 10 "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)'",
            (
                "SuccessorActivationRank",
                "SuccessorActivationPipelineDistance",
                "RehydrateCleanCompleteTipSuccessorStartup",
                "ExactDurableParentApplication",
                "Isa",
            ),
        ),
        "RecoveredAuthenticationDescendsAbsentTier": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "successorContext \\in AdmissibleContextRecords, "
            "application \\in Chain!DecisionEvidenceSet: "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ AuthenticateRecoveredSuccessorActivation( "
            "parentContext, node, successorContext, application) "
            "=> /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ successorPredecessorStatusOwnership'[parentContext][node] "
            "= \"Absent\" "
            "/\\ SuccessorActivationRank(parentContext, node) = 10 "
            "/\\ SuccessorActivationRank(parentContext, node)' = 8 "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)'",
            (
                "AuthenticateRecoveredSuccessorActivation",
                "SuccessorActivationCredentialReady",
                "ExactSuccessorActivationToken",
                "ExactCompleteTipRecoveryAuthority",
                "SuccessorActivationRank",
                "SuccessorActivationPipelineDistance",
                "Isa",
            ),
        ),
        "SuccessorActivationFailureFreeProgressStrictlyDecreasesRank": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)' "
            "/\\ IndexedSuccessorActivationProgressStep(parentContext, node) "
            "=> \\/ SuccessorPublicationOrSuperseded(parentContext, node)' "
            "\\/ /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationRank(parentContext, node)' "
            "< SuccessorActivationRank(parentContext, node)",
            (
                "FailureFreeBracketExcludesSuccessorResetActions",
                "SuccessorActivationPendingRankTierClassification",
                "RecoveredAuthenticationDescendsAbsentTier",
                "CleanCompleteTipRestartDescendsPublishedTier",
                "LatchAppliedSuccessorStartupFailure",
                "LatchRecoveredSuccessorStartupFailure",
                "RehydrateFailedSuccessorStartup",
                "Isa",
            ),
        ),
        "IndexedProductActionDoesNotRaisePendingSuccessorRank": (
            "\\A initialContext \\in JoinedContexts, "
            "parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ IndexedCompositionInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ IndexedProductActionAt(initialContext) "
            "=> \\/ SuccessorPublicationOrSuperseded(parentContext, node)' "
            "\\/ /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationRank(parentContext, node)' "
            "<= SuccessorActivationRank(parentContext, node)",
            (
                "IndexedStepDoesNotOrphanSuccessorActivation",
                "IndexedProductActionAt",
                "IndexedReceiptClassification",
                "QueueSuccessorActivation",
                "Isa",
            ),
        ),
        "OtherOwnerProgressFramesPendingSuccessorRankOrSupersedes": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "selectedParent \\in AdmissibleContextRecords, "
            "selectedNode \\in ValidatorIds: "
            "/\\ IndexedCompositionInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationOwner(selectedParent, selectedNode) "
            "# SuccessorActivationOwner(parentContext, node) "
            "/\\ IndexedSuccessorActivationProgressStep( "
            "selectedParent, selectedNode) "
            "=> \\/ SuccessorPublicationOrSuperseded(parentContext, node)' "
            "\\/ /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationRank(parentContext, node)' "
            "= SuccessorActivationRank(parentContext, node)",
            (
                "IndexedStepDoesNotOrphanSuccessorActivation",
                "SuccessorActivationOwner",
                "IndexedSuccessorActivationProgressStep",
                "Isa",
            ),
        ),
        "IndexedStepRetainsExactDurableParentWitnessOrExits": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ IndexedCompositionInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ [IndexedChainNext]_IndexedChainVars "
            "=> \\/ SuccessorPublicationOrSuperseded(parentContext, node)' "
            "\\/ /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationHasDurableParentWitness( "
            "parentContext, node)'",
            (
                "IndexedStepDoesNotOrphanSuccessorActivation",
                "IndexedStepPreservesSuccessorActivationProtocolInvariant",
                "SuccessorActivationHasDurableParentWitness",
                "Isa",
            ),
        ),
        "IndexedFailureFreeStepDoesNotRaiseSuccessorActivationRank": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ IndexedCompositionInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)' "
            "/\\ [IndexedChainNext]_IndexedChainVars "
            "=> \\/ SuccessorPublicationOrSuperseded(parentContext, node)' "
            "\\/ /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationRank(parentContext, node)' "
            "<= SuccessorActivationRank(parentContext, node)",
            (
                "IndexedStepDoesNotOrphanSuccessorActivation",
                "IndexedProductActionDoesNotRaisePendingSuccessorRank",
                "OtherOwnerProgressFramesPendingSuccessorRankOrSupersedes",
                "FailureFreeBracketExcludesSuccessorResetActions",
                "SuccessorActivationFailureFreeProgressStrictlyDecreasesRank",
                "IndexedChainNext",
                "Isa",
            ),
        ),
        "SuccessorActivationFailureFreeRankPersistsOrExits": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "rank \\in SuccessorActivationRankCarrier: "
            "/\\ IndexedCompositionInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationAtRank(parentContext, node, rank) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)' "
            "/\\ [IndexedChainNext]_IndexedChainVars "
            "=> \\/ SuccessorActivationAtRank(parentContext, node, rank)' "
            "\\/ SuccessorActivationRankExit(parentContext, node, rank)'",
            (
                "IndexedFailureFreeStepDoesNotRaiseSuccessorActivationRank",
                "IndexedStepRetainsExactDurableParentWitnessOrExits",
                "IndexedStepPreservesSuccessorActivationProtocolInvariant",
                "Isa",
            ),
        ),
        "SuccessorActivationFailureFreeProgressExitsCurrentRank": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "rank \\in SuccessorActivationRankCarrier: "
            "/\\ Chain!ChainEpochInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationAtRank(parentContext, node, rank) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)' "
            "/\\ <<IndexedSuccessorActivationProgressStep( "
            "parentContext, node)>>_(IndexedChainVars) "
            "=> SuccessorActivationRankExit(parentContext, node, rank)'",
            (
                "SuccessorActivationFailureFreeProgressStrictlyDecreasesRank",
                "SuccessorActivationProgressPreservesProtocolInvariant",
                "Isa",
            ),
        ),
        "FailureFreeSuccessorActivationRankLeadsToExit": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "rank \\in SuccessorActivationRankCarrier: "
            "/\\ SuccessorActivationTemporalKernel(parentContext, node) "
            "/\\ SuccessorActivationFailureFreeSuffix(parentContext, node) "
            "=> (SuccessorActivationAtRank(parentContext, node, rank) "
            "~> SuccessorActivationRankExit(parentContext, node, rank))",
            (
                "SuccessorActivationFailureFreeRankPersistsOrExits",
                "SuccessorActivationAtRankEnablesFairProgress",
                "SuccessorActivationFailureFreeProgressExitsCurrentRank",
                "Chain!ChainEpochInvariant",
                "DEF IndexedCompositionInvariant",
                "WF_IndexedChainVars",
                "PTL",
            ),
        ),
        "FailureFreeSuccessorActivationRankConverges": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ SuccessorActivationTemporalKernel(parentContext, node) "
            "/\\ SuccessorActivationFailureFreeSuffix(parentContext, node) "
            "=> \\A rank \\in SuccessorActivationRankCarrier: "
            "SuccessorActivationAtRank(parentContext, node, rank) "
            "~> SuccessorPublicationOrSuperseded(parentContext, node)",
            (
                "SuccessorActivationRankOrderingIsWellFounded",
                "FailureFreeSuccessorActivationRankLeadsToExit",
                "WellFoundedLeadsTo",
            ),
        ),
        "FailureFreeSuccessorActivationConverges": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ SuccessorActivationTemporalKernel(parentContext, node) "
            "/\\ SuccessorActivationFailureFreeSuffix(parentContext, node) "
            "=> (SuccessorActivationPending(parentContext, node) "
            "~> SuccessorPublicationOrSuperseded(parentContext, node))",
            (
                "FailureFreeSuccessorActivationRankConverges",
                "SuccessorActivationRankExistentialLift",
                "SuccessorActivationPendingHasRankWitness",
                "PTL",
            ),
        ),
        "SuccessorActivationTemporalKernelIsSuffixClosed": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "SuccessorActivationTemporalKernel(parentContext, node) "
            "=> []SuccessorActivationTemporalKernel(parentContext, node)",
            ("PTL", "SuccessorActivationTemporalKernel"),
        ),
        "FailureFreeSuccessorActivationConvergenceAtEverySuffix": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "[]( /\\ SuccessorActivationTemporalKernel(parentContext, node) "
            "/\\ SuccessorActivationFailureFreeSuffix(parentContext, node) "
            "=> (SuccessorActivationPending(parentContext, node) "
            "~> SuccessorPublicationOrSuperseded(parentContext, node)))",
            ("FailureFreeSuccessorActivationConverges", "PTL"),
        ),
        "SuccessorActivationPendingReachesFailureFreeSuffixOrOutcome": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ SuccessorActivationTemporalKernel(parentContext, node) "
            "/\\ <>SuccessorActivationFailureFreeSuffix(parentContext, node) "
            "=> (SuccessorActivationPending(parentContext, node) "
            "~> (SuccessorPublicationOrSuperseded(parentContext, node) "
            "\\/ /\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationFailureFreeSuffix( "
            "parentContext, node)))",
            (
                "IndexedStepRetainsExactDurableParentWitnessOrExits",
                "SuccessorActivationTemporalKernel",
                "SuccessorActivationFailureFreeSuffix",
                "PTL",
            ),
        ),
        "EventualFailureFreeSuffixLiftsSuccessorConvergence": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ SuccessorActivationTemporalKernel(parentContext, node) "
            "/\\ <>SuccessorActivationFailureFreeSuffix(parentContext, node) "
            "=> (SuccessorActivationPending(parentContext, node) "
            "~> SuccessorPublicationOrSuperseded(parentContext, node))",
            (
                "SuccessorActivationTemporalKernelIsSuffixClosed",
                "FailureFreeSuccessorActivationConvergenceAtEverySuffix",
                "SuccessorActivationPendingReachesFailureFreeSuffixOrOutcome",
                "PTL",
            ),
        ),
        "IndexedChainSpecEstablishesSuccessorActivationStarvationFreedom": (
            "IndexedChainSpec => "
            "SuccessorActivationStarvationFreedomProperty",
            (
                "IndexedChainSpecEstablishesSuccessorActivationTemporalKernel",
                "EventualFailureFreeSuccessorStartupSuffix",
                "EventualFailureFreeSuffixLiftsSuccessorConvergence",
            ),
        ),
        "IndexedChainSpecEstablishesSuccessorActivationRankProgress": (
            "IndexedChainSpec => SuccessorActivationRankProgressProperty",
            (
                "IndexedChainSpecEstablishesSuccessorActivationStarvationFreedom",
                "SuccessorActivationRankProgressProperty",
                "PTL",
            ),
        ),
    }
    exact_proof_token_counts = {
        "IndexedActionPreservesSuccessorActivationProtocolInvariant": {
            "DEF IndexedCompositionInvariant": 2,
        },
        "FailureFreeSuccessorActivationRankLeadsToExit": {
            "Chain!ChainEpochInvariant": 1,
            "DEF IndexedCompositionInvariant": 1,
        },
    }
    for symbol, (exact_statement, required_proof_tokens) in (
        theorem_contracts.items()
    ):
        theorem = _top_level_theorem_body(
            source, symbol, preserve_string_contents=True
        )
        if theorem is None:
            errors.append(f"{proof_path}: missing successor-rank theorem {symbol}")
            continue
        theorem_body, line = theorem
        observed_statement = _tla_statement_without_proof(theorem_body)
        if observed_statement != exact_statement:
            errors.append(
                f"{proof_path}:{line}: {symbol} must state only "
                f"{exact_statement!r}; found {observed_statement!r}"
            )
        theorem_parts = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
            theorem_body,
            maxsplit=1,
        )
        if len(theorem_parts) != 2:
            errors.append(
                f"{proof_path}:{line}: {symbol} must retain an explicit "
                "non-vacuous proof body"
            )
            continue
        observed_proof = theorem_parts[1]
        for required_token in required_proof_tokens:
            if not _tla_dependency_present(observed_proof, required_token):
                errors.append(
                    f"{proof_path}:{line}: {symbol} proof must invoke "
                    f"{required_token}"
                )
        for exact_token, exact_count in exact_proof_token_counts.get(
            symbol, {}
        ).items():
            observed_count = len(
                _tla_dependency_positions(observed_proof, exact_token)
            )
            if observed_count != exact_count:
                errors.append(
                    f"{proof_path}:{line}: {symbol} proof must contain "
                    f"{exact_token!r} exactly {exact_count} time(s); found "
                    f"{observed_count}"
                )
        if re.search(
            r"(?:\bOBVIOUS\b|\bASSUME\s+FALSE\b|\bBY\s+TRUE\b|"
            r"\bPROVE\s+TRUE\b)",
            observed_proof,
        ):
            errors.append(
                f"{proof_path}:{line}: {symbol} proof may not use a "
                "vacuous assertion"
            )

    chain_path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    if chain_path.is_file():
        chain_source = _chain_epoch_refinement_source(formal_dir)
        pending = _top_level_operator_body(
            chain_source,
            "IndexedSuccessorActivationPending",
            preserve_string_contents=True,
        )
        exact_pending = (
            "/\\ parentContext \\in AdmissibleContextRecords "
            "/\\ node \\in ValidatorIds "
            "/\\ parentContext.height < MaxHeight "
            "/\\ successorActivationStatus[parentContext][node] "
            '\\in {"Queued", "Running"} '
            "/\\ ~SuccessorPublicationOrSuperseded(parentContext, node)"
        )
        if pending is None:
            errors.append(
                f"{chain_path}: missing IndexedSuccessorActivationPending"
            )
        else:
            body, line = pending
            normalized = " ".join(body.split())
            if normalized != exact_pending:
                errors.append(
                    f"{chain_path}:{line}: IndexedSuccessorActivationPending "
                    f"must equal only {exact_pending!r}; found {normalized!r}"
                )

    theorem_symbol = "SuccessorActivationStarvationFreedomObligation"
    theorem = _top_level_theorem_body(
        source, theorem_symbol, preserve_string_contents=True
    )
    exact_statement = (
        "IndexedChainSpec "
        "=> /\\ SuccessorActivationPendingStructureProperty "
        "/\\ SuccessorActivationStepDecreasesRankProperty "
        "/\\ SuccessorActivationPendingIsNotOrphanedProperty "
        "/\\ SuccessorActivationOutcomeIsStableProperty "
        "/\\ SuccessorActivationRankProgressProperty "
        "/\\ SuccessorActivationStarvationFreedomProperty"
    )
    if theorem is None:
        errors.append(f"{proof_path}: missing {theorem_symbol}")
    else:
        body, line = theorem
        theorem_parts = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
        )
        statement = theorem_parts[0]
        normalized = " ".join(statement.split())
        if normalized != exact_statement:
            errors.append(
                f"{proof_path}:{line}: {theorem_symbol} must state only "
                f"{exact_statement!r}; found {normalized!r}"
            )
        if len(theorem_parts) != 2:
            errors.append(
                f"{proof_path}:{line}: {theorem_symbol} must retain the "
                "explicit candidate TLAPS proof while strict verification "
                "remains pending"
            )
        else:
            aggregate_proof = theorem_parts[1]
            required_aggregate_dependencies = (
                "IndexedChainSpecEstablishesSuccessorActivationPendingStructure",
                "IndexedChainSpecEstablishesSuccessorActivationStepDecrease",
                "IndexedChainSpecEstablishesSuccessorActivationNonOrphaning",
                "IndexedChainSpecEstablishesSuccessorActivationOutcomeStability",
                "IndexedChainSpecEstablishesSuccessorActivationRankProgress",
                "IndexedChainSpecEstablishesSuccessorActivationStarvationFreedom",
            )
            for dependency in required_aggregate_dependencies:
                if len(
                    _tla_dependency_positions(aggregate_proof, dependency)
                ) != 1:
                    errors.append(
                        f"{proof_path}:{line}: {theorem_symbol} proof must "
                        f"invoke {dependency} exactly once"
                    )
            if re.search(
                r"(?:\bOBVIOUS\b|\bASSUME\s+FALSE\b|\bBY\s+TRUE\b|"
                r"\bPROVE\s+TRUE\b)",
                aggregate_proof,
            ):
                errors.append(
                    f"{proof_path}:{line}: {theorem_symbol} proof may not "
                    "use a vacuous assertion"
                )

    equivalence_symbol = "SuccessorActivationStarvationMatchesChainProgress"
    equivalence = _top_level_theorem_body(
        source, equivalence_symbol, preserve_string_contents=True
    )
    exact_equivalence = (
        "SuccessorActivationStarvationFreedomProperty "
        "<=> IndexedSuccessorActivationProgress"
    )
    if equivalence is None:
        errors.append(f"{proof_path}: missing {equivalence_symbol}")
    else:
        body, line = equivalence
        statement = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
        )[0]
        normalized = " ".join(statement.split())
        if normalized != exact_equivalence:
            errors.append(
                f"{proof_path}:{line}: {equivalence_symbol} must state only "
                f"{exact_equivalence!r}; found {normalized!r}"
            )
    return errors

def _successor_production_recovery_finalization_tail(
    launch_path: Path,
    launch_source: str,
    lifecycle_run_inner_path: Path,
    lifecycle_run_inner_source: str,
    finalized_output_path: Path,
    finalized_output_source: str,
    ledger_path: Path,
    ledger_source: str,
    lifecycle_startup_test_path: Path,
    lifecycle_startup_test_source: str,
    registry_validate_path: Path,
    registry_validate_source: str,
    registry_path: Path,
    registry_source: str,
    wal_recovery_path: Path,
    wal_recovery_source: str,
    scheduler_path: Path,
    scheduler_source: str,
    region: Any,
    require_order: Any,
    require_tokens: Any,
    reject_tokens: Any,
    require_literals: Any,
) -> None:
    """Bind production finalization publication and recovered refanout."""

    finalization_readiness = region(
        launch_path,
        launch_source,
        "shared lifecycle finalization readiness",
        "fn ready_for_finalized_rollover(\n        &mut self,\n    ) -> Result<bool, ProductionLifecycleFinalizationErrorV1> {",
        "impl ActivatedProductionLifecycleV1",
    )
    require_order(
        launch_path,
        "shared lifecycle finalization readiness",
        finalization_readiness,
        (
            "let locally_ready =",
            "self.executor.ready_to_finish()",
            "!self.owner.has_recovered_lifecycle_outputs()",
            "self.pending_kura_apply_replay.is_none()",
            "self.recovered_local_proposal_attempt.is_none()",
            "self.pending_lifecycle_completion.is_none()",
            "self.pending_ingress_capacity.is_none()",
            "self.completion_observer_activation.is_none()",
            "exactly_covers_finalization_work(&self.owner.coordinator)",
            "if !locally_ready",
            "return Ok(false)",
            "self.verify_published_store_marker_finalization_census()",
            "ProductionLifecycleFinalizationErrorV1::StoreMarkerCensus",
            "Ok(true)",
        ),
    )
    activated_finalization = region(
        launch_path,
        launch_source,
        "activated lifecycle finalization",
        "fn into_finalized_rollover(",
        "pub(in crate::sumeragi) fn retire_lifecycle_stores_for_test(",
    )
    require_order(
        launch_path,
        "activated lifecycle finalization",
        activated_finalization,
        (
            "self.launched.ready_for_finalized_rollover()",
            "let Self { mut launched, local_proposal, runner_activation, } = self",
            "runner_activation.retire(&launched.leader_wire_ingress_binding.ingress)",
            "drop(local_proposal)",
            "launched.leader_wire_ingress_binding.retire()",
            "executor.into_finalized_parts()",
            "begin_fail_stop_operation()",
            "runtime.into_driver().finish_height(&receipt, &artifact)",
            "operation.complete()",
            "FinalizedProductionLifecycleRolloverV1 {",
        ),
    )
    require_tokens(
        launch_path,
        "activated lifecycle finalization quiescence",
        activated_finalization,
        (
            "ProductionLifecycleFinalizationErrorV1::NotReady",
            "finalized_adapter: finalized",
        ),
    )
    require_order(
        lifecycle_run_inner_path,
        "runner lifecycle finalization preflight",
        lifecycle_run_inner_source,
        (
            "let terminal_planning_fenced =",
            "terminal_finalization_fenced || producer_claim.apply_terminal_settled()",
            "if terminal_planning_fenced && !ready_to_finish",
            "let producer_turn = if terminal_planning_fenced",
            "if !terminal_planning_fenced",
            "schedule_local_proposal(",
            "let finalization_ready = if ready_to_finish",
            "activated.ready_for_finalized_rollover(&mut active_runner)?",
            "if ready_to_finish && !finalization_ready",
            "let rollover_ready = if finalization_ready",
            "preflight_finalized_lane_rollover(",
            "if finalization_ready && !rollover_ready",
            "if rollover_ready && terminal_exact_output_pending",
            "close_runner_ingress_for_finalized_drain(",
            "drain_decided_lane_recovery_ingress(",
            "ensure_closed_drained_cut()",
            "finalize_lifecycle_height(",
        ),
    )
    output_rollover = region(
        launch_path,
        launch_source,
        "typed lifecycle finalized-output rollover",
        "impl FinalizedProductionLifecycleRolloverV1",
        "impl ProductionLifecyclePostOutputHandoffV1",
    )
    require_order(
        launch_path,
        "typed lifecycle finalized-output rollover",
        output_rollover,
        (
            "rollover_finalized_height_outputs_for_lifecycle(",
            "ProductionLifecycleOutputRolloverPermitV1 {",
            "finalized_adapter.retire_after_output_handoff()",
            "refresh_live_serve_retirement_cut(&services, &retired_ingress)",
            "ProductionLifecyclePostOutputHandoffV1 {",
        ),
    )
    require_tokens(
        finalized_output_path,
        "sealed runner finalized-output reuse",
        finalized_output_source,
        (
            "fn rollover_finalized_height_outputs_for_lifecycle(",
            "_permit: super::v2_lifecycle_coordinator::ProductionLifecycleOutputRolloverPermitV1",
            "rollover_finalized_height_outputs(",
        ),
    )
    store_retirement = region(
        launch_path,
        launch_source,
        "post-output lifecycle-store retirement",
        "impl ProductionLifecyclePostOutputHandoffV1",
        "impl ProductionLifecycleCleanupReadyV1",
    )
    require_order(
        launch_path,
        "post-output lifecycle-store retirement",
        store_retirement,
        (
            "begin_fail_stop_operation()",
            "retire_authenticated_cut(serve_payloads, &retained_serve_payloads)",
            "reconcile_complete_tip_serve_retirement(&current, refreshed)",
            "stage_finalized_height_all_row_retirement(reconciliation)",
            "persist_exact_finalization_successor(staged)",
            "publication.consume_owners(registry)",
            "operation.complete()",
            "ProductionLifecycleCleanupReadyV1 {",
        ),
    )
    cleanup_ready = region(
        launch_path,
        launch_source,
        "cleanup-ready lifecycle service teardown",
        "impl ProductionLifecycleCleanupReadyV1",
        "impl ProductionLifecycleOwnerV1",
    )
    require_order(
        launch_path,
        "cleanup-ready lifecycle service teardown",
        cleanup_ready,
        (
            "self.services.allow_clean_shutdown()",
            "self.services.finish_height(self.receipt, cleanup_timeout, supervisor)",
            "ProductionLifecycleFinalizationOutcomeV1 {",
        ),
    )

    finalization_publication = region(
        ledger_path,
        ledger_source,
        "opaque all-row finalization publication",
        "fn persist_exact_finalization_successor(",
        "#[cfg(test)]",
    )
    require_order(
        ledger_path,
        "opaque all-row finalization publication",
        finalization_publication,
        (
            "self,",
            "StagedFinalizationRetirementV1 { current, retired }",
            "LifecycleLedgerV1::from_coordinator(&self)? != current",
            "store.persist_exact_successor(&current, &retired)?",
            "store.load()? != retired",
            "coordinator: self",
        ),
    )
    require_tokens(
        ledger_path,
        "opaque all-row finalization ownership",
        ledger_source,
        (
            "struct StagedFinalizationRetirementV1 { current: LifecycleLedgerV1, retired: LifecycleLedgerV1, }",
            "struct PublishedFinalizationRetirementV1 { coordinator: LifecycleCoordinator, current: LifecycleLedgerV1, retired: LifecycleLedgerV1, retained_floor: PublishedFinalizedLifecycleRetainedFloorV1, }",
            "fn consume_owners( self, mut registry: LifecycleWorkRegistryHolder, )",
            "registry.registry_mut().exactly_covers_finalization_work(&self.coordinator)",
            "let retained_floor = self.retained_floor",
            "drop(self.coordinator)",
            "retained_floor",
        ),
    )
    reject_tokens(
        ledger_path,
        "opaque all-row finalization ownership",
        ledger_source,
        (
            "impl Clone for StagedFinalizationRetirementV1",
            "impl Copy for StagedFinalizationRetirementV1",
            "impl Clone for PublishedFinalizationRetirementV1",
            "impl Copy for PublishedFinalizationRetirementV1",
            "pub coordinator: LifecycleCoordinator",
            "pub current: LifecycleLedgerV1",
            "pub retired: LifecycleLedgerV1",
        ),
    )
    require_tokens(
        lifecycle_startup_test_path,
        "production lifecycle all-row finalization behavior",
        lifecycle_startup_test_source,
        (
            "fn production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout()",
            ".retire_lifecycle_stores_for_test(finality_receipt)",
            "cleanup_ready.finish_cleanup(Duration::ZERO, &mut cleanup_supervisor)",
            "fn recovered_lifecycle_factory_inputs_bind_exact_state_kura_and_network()",
            "let placeholder_cadence = exact_state.sumeragi_block_cadence()",
            "placeholder_cadence.checked_add(Duration::from_millis(1))",
            "assert_eq!(cadence_inputs.block_cadence, authenticated_cadence)",
            "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
            "lifecycle_run_inner::finalize_lifecycle_height(",
            "assert_eq!(receipt.context_id(), recovered_context.id())",
            "assert_eq!(artifact.subject, subject)",
            ".retain_merge_sidecars_for_global_view(",
            "successor.parent_commit_qc = Some(artifact.commit_qc.clone())",
            "drop(retained_sidecars)",
            "outcome.cleanup().warnings().is_empty()",
        ),
    )
    finalization_registry = region(
        registry_validate_path,
        registry_validate_source,
        "finalization-only recovered registry census",
        "fn exactly_covers_finalization_work(",
        "fn exactly_covers_ready_work_with_extra(",
    )
    require_tokens(
        registry_validate_path,
        "finalization-only recovered registry census",
        finalization_registry,
        (
            "coordinator.fault.is_some() || coordinator.active_lease.is_some()",
            "DurableRecoveredLifecycleSignedBroadcast(_)",
            ".collect::<std::collections::BTreeSet<_>>()",
            "self.exact_recovered_wal_registry_slot()",
            "RecoveredWalRegistrySlotV1::None",
            "self.exactly_covers_ready_work_with_extra( coordinator, extra, &std::collections::BTreeSet::new(), None, &refanned_broadcasts, )",
        ),
    )
    finalization_pair_link = region(
        registry_validate_path,
        registry_validate_source,
        "finalization recovered Broadcast pair link",
        "fn exact_optional_recovered_wal_authority(",
        "/// Install one work value without overwriting an incumbent address.",
    )
    require_tokens(
        registry_validate_path,
        "finalization recovered Broadcast pair link",
        finalization_pair_link,
        (
            "broadcast.is_unpaired()",
            "carrier.pairs_exact_next_sign(next_sign, next_sign_digest)",
            "refanned_broadcasts.iter().all(|ordinal|",
            "LifecycleLedgerV1::from_coordinator(coordinator)",
            "!matches!(record.state, super::LifecycleState::Waiting(_))",
            "broadcast.validates_in_ledger(exact_ledger)",
            "broadcast.paired_next_sign_matches_terminal_record( coordinator, exact_ledger, )",
            "broadcast.matches_current_finalization_record(",
            "!refanned_broadcasts.contains(&record.ordinal)",
        ),
    )
    terminal_pair_link = region(
        registry_path,
        registry_source,
        "retained recovered Broadcast terminal next-Sign link",
        "fn paired_next_sign_matches_terminal_record(",
        "fn validates_at(",
    )
    require_tokens(
        registry_path,
        "retained recovered Broadcast terminal next-Sign link",
        terminal_pair_link,
        (
            "LifecycleState::Terminal(outcome)",
            "exact_single_record_slot(",
            "LifecycleWorkClass::SignVote.capacity_class()",
            "ConcreteWorkAddress::new(next_record.owner, next_record.ordinal, next_slot) == Some(next_address)",
            "installed_digest == next_digest",
            "coordinator.key_index.get(&next_record.key) == Some(&next_address.ordinal)",
            "coordinator.owner_index.get(&next_record.owner.causal_root()) == Some(&next_record.owner)",
            "!coordinator.ready_index.contains(&next_address.ordinal)",
            "durable_next.terminal() == Some(Some(outcome))",
            "continuation.matches_record( LifecycleWorkClass::SignVote, Some(outcome), next_address.ordinal, ledger.high_water(), )",
        ),
    )
    require_tokens(
        wal_recovery_path,
        "volatile refanned Broadcast finalization state",
        wal_recovery_source,
        (
            "fn matches_current_finalization_record(",
            "WaitSource::Recovery(digest)",
            "coordinator.observed_generation.get(&expected_source) == Some(&wait.observed_generation())",
            "!coordinator.ready_index.contains(&address.ordinal)",
        ),
    )
    require_literals(
        scheduler_path,
        "volatile refanned Broadcast finalization behavior",
        scheduler_source,
        (
            '"finalization accepts the exact volatile refanout wait after its next Sign retires"',
            '"all exact post-output waits form one bounded finalization census"',
            '"finalization rejects a corrupted retained digest after paired Sign retirement"',
            '"finalization must reject the corrupted exact next-Sign link"',
        ),
    )
    require_tokens(
        scheduler_path,
        "volatile refanned Broadcast finalization behavior",
        scheduler_source,
        (
            "fn recovered_broadcast_refanout_ranks_exact_pair_before_unrelated_ready_sign()",
            "fn finalization_waits_for_every_authenticated_recovered_broadcast_refanout()",
            "finalization_registry_census_is_exact_for_test()",
        ),
    )


def _successor_recovery_finalization_tail(
    ledger_path: Path,
    ledger_source: str,
    lifecycle_startup_test_path: Path,
    lifecycle_startup_test_source: str,
    registry_validate_path: Path,
    registry_validate_source: str,
    registry_validate_impl_path: Path,
    registry_validate_impl_source: str,
    registry_path: Path,
    registry_source: str,
    wal_recovery_path: Path,
    wal_recovery_source: str,
    scheduler_path: Path,
    scheduler_source: str,
    region: Any,
    require_order: Any,
    require_tokens: Any,
    reject_tokens: Any,
    require_literal_count: Any,
) -> None:
    """Bind standalone finalization publication and recovered refanout."""

    finalization_publication = region(
        ledger_path,
        ledger_source,
        "opaque all-row finalization publication",
        "fn persist_exact_finalization_successor(",
        "#[cfg(test)]",
    )
    require_order(
        ledger_path,
        "opaque all-row finalization publication",
        finalization_publication,
        (
            "self,",
            "StagedFinalizationRetirementV1 { current, retired }",
            "LifecycleLedgerV1::from_coordinator(&self)? != current",
            "store.persist_exact_successor(&current, &retired)?",
            "store.load()? != retired",
            "coordinator: self",
        ),
    )
    require_tokens(
        ledger_path,
        "opaque all-row finalization ownership",
        ledger_source,
        (
            "struct StagedFinalizationRetirementV1 { current: LifecycleLedgerV1, retired: LifecycleLedgerV1, }",
            "struct PublishedFinalizationRetirementV1 { coordinator: LifecycleCoordinator, current: LifecycleLedgerV1, retired: LifecycleLedgerV1, retained_floor: PublishedFinalizedLifecycleRetainedFloorV1, }",
            "fn consume_owners( self, mut registry: LifecycleWorkRegistryHolder, )",
            "registry.registry_mut().exactly_covers_finalization_work(&self.coordinator)",
            "let retained_floor = self.retained_floor",
            "drop(self.coordinator)",
            "retained_floor",
        ),
    )
    reject_tokens(
        ledger_path,
        "opaque all-row finalization ownership",
        ledger_source,
        (
            "impl Clone for StagedFinalizationRetirementV1",
            "impl Copy for StagedFinalizationRetirementV1",
            "impl Clone for PublishedFinalizationRetirementV1",
            "impl Copy for PublishedFinalizationRetirementV1",
            "pub coordinator: LifecycleCoordinator",
            "pub current: LifecycleLedgerV1",
            "pub retired: LifecycleLedgerV1",
        ),
    )
    require_tokens(
        lifecycle_startup_test_path,
        "production lifecycle all-row finalization behavior",
        lifecycle_startup_test_source,
        (
            "fn production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout()",
            ".retire_lifecycle_stores_for_test(finality_receipt)",
            "cleanup_ready.finish_cleanup(Duration::ZERO, &mut cleanup_supervisor)",
            "fn recovered_lifecycle_factory_inputs_bind_exact_state_kura_and_network()",
            "let placeholder_cadence = exact_state.sumeragi_block_cadence()",
            "placeholder_cadence.checked_add(Duration::from_millis(1))",
            "assert_eq!(cadence_inputs.block_cadence, authenticated_cadence)",
            "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
            ".claim_producer_turn_for_local_proposal(&mut serve_runner)",
            ".settle_producer_turn_after_local_proposal(&mut serve_runner, attempted_producer)",
            "super::super::v2_runner::lifecycle_run_inner::finalize_lifecycle_height(",
            "assert_eq!(receipt.context_id(), recovered_context.id())",
            "assert_eq!(artifact.subject, subject)",
            "Ok::<_, super::super::v2_runner::V2RunnerError>((successor, ()))",
            "outcome.cleanup().warnings().is_empty()",
            "outcome.wal_retirement_warning().is_none()",
        ),
    )
    finalization_registry = region(
        registry_validate_path,
        registry_validate_source,
        "finalization-only recovered registry census",
        "fn exactly_covers_finalization_work(",
        "fn exactly_covers_ready_work_with_extra(",
    )
    require_tokens(
        registry_validate_path,
        "finalization-only recovered registry census",
        finalization_registry,
        (
            "coordinator.fault.is_some() || coordinator.active_lease.is_some()",
            "DurableRecoveredLifecycleSignedBroadcast(_)",
            ".collect::<std::collections::BTreeSet<_>>()",
            "self.exact_recovered_wal_registry_slot()",
            "RecoveredWalRegistrySlotV1::None",
            "self.exactly_covers_ready_work_with_extra( coordinator, extra, &std::collections::BTreeSet::new(), None, &refanned_broadcasts, )",
        ),
    )
    finalization_pair_link = region(
        registry_validate_impl_path,
        registry_validate_impl_source,
        "finalization recovered Broadcast pair link",
        "fn exact_optional_recovered_wal_authority(",
        "/// Install one work value without overwriting an incumbent address.",
    )
    require_tokens(
        registry_validate_impl_path,
        "finalization recovered Broadcast pair link",
        finalization_pair_link,
        (
            "broadcast.is_unpaired()",
            "carrier.pairs_exact_next_sign(next_sign, next_sign_digest)",
            "refanned_broadcasts.iter().all(|ordinal|",
            "LifecycleLedgerV1::from_coordinator(coordinator)",
            "!matches!(record.state, super::LifecycleState::Waiting(_))",
            "broadcast.validates_in_ledger(exact_ledger)",
            "broadcast.paired_next_sign_matches_terminal_record( coordinator, exact_ledger, )",
            "broadcast.matches_current_finalization_record(",
            "!refanned_broadcasts.contains(&record.ordinal)",
        ),
    )
    terminal_pair_link = region(
        registry_path,
        registry_source,
        "retained recovered Broadcast terminal next-Sign link",
        "fn paired_next_sign_matches_terminal_record(",
        "fn validates_at(",
    )
    require_tokens(
        registry_path,
        "retained recovered Broadcast terminal next-Sign link",
        terminal_pair_link,
        (
            "LifecycleState::Terminal(outcome)",
            "exact_single_record_slot(",
            "LifecycleWorkClass::SignVote.capacity_class()",
            "ConcreteWorkAddress::new(next_record.owner, next_record.ordinal, next_slot) == Some(next_address)",
            "installed_digest == next_digest",
            "coordinator.key_index.get(&next_record.key) == Some(&next_address.ordinal)",
            "coordinator.owner_index.get(&next_record.owner.causal_root()) == Some(&next_record.owner)",
            "!coordinator.ready_index.contains(&next_address.ordinal)",
            "durable_next.terminal() == Some(Some(outcome))",
            "continuation.matches_record( LifecycleWorkClass::SignVote, Some(outcome), next_address.ordinal, ledger.high_water(), )",
        ),
    )
    require_tokens(
        wal_recovery_path,
        "volatile refanned Broadcast finalization state",
        wal_recovery_source,
        (
            "fn matches_current_finalization_record(",
            "WaitSource::Recovery(digest)",
            "coordinator.observed_generation.get(&expected_source) == Some(&wait.observed_generation())",
            "!coordinator.ready_index.contains(&address.ordinal)",
        ),
    )
    require_tokens(
        scheduler_path,
        "volatile refanned Broadcast finalization behavior",
        scheduler_source,
        (
            "fn recovered_broadcast_refanout_ranks_exact_pair_before_unrelated_ready_sign()",
            "fn finalization_waits_for_every_authenticated_recovered_broadcast_refanout()",
            "finalization_registry_census_is_exact_for_test()",
        ),
    )
    for literal in (
        '"finalization accepts the exact volatile refanout wait after its next Sign retires"',
        '"all exact post-output waits form one bounded finalization census"',
        '"finalization rejects a corrupted retained digest after paired Sign retirement"',
        '"finalization must reject the corrupted exact next-Sign link"',
    ):
        require_literal_count(
            scheduler_path,
            "volatile refanned Broadcast finalization behavior",
            scheduler_source,
            literal,
            1,
        )
