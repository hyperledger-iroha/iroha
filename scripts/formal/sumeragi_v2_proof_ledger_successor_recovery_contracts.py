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
        chain_source = chain_path.read_text(encoding="utf-8")
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
def _async_historical_recovery_source_fidelity_errors(
    formal_dir: Path,
) -> list[str]:
    """Pin the exact all-responsive historical-recovery proof boundary."""
    path = formal_dir / "SumeragiV2AsyncHistoricalRecoveryLivenessProofs.tla"
    if not path.is_file():
        return [f"{path}: missing Async historical-recovery liveness child"]
    raw_source = path.read_text(encoding="utf-8")
    source = strip_tla_comments(raw_source, preserve_string_contents=True)
    errors: list[str] = []
    extends = re.search(r"(?m)^EXTENDS\s+([^\n]+)$", source)
    exact_extends = "SumeragiV2AsyncTimeoutOwnershipProofs, TLAPS"
    if extends is None or " ".join(extends.group(1).split()) != exact_extends:
        errors.append(
            f"{path}: Async historical-recovery child must extend exactly "
            f"{exact_extends!r}"
        )
    operator_contracts = {
        "HistoricalRecoveryTargetDecisionProgressProperty": (
            "specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalRecoveryTarget(node)) "
            "~> NodeHasDecision(node)"
        ),
        "ResponsiveDecisionApplicationProgressProperty": (
            "specification => \\A node \\in Responsive: "
            "(gst /\\ NodeHasDecision(node)) "
            "~> NodeHasApplication(node)"
        ),
        "HistoricalRecoveryAsyncTemporalPrerequisites": (
            "/\\ HistoricalRecoveryTargetDecisionProgressProperty(specification) "
            "/\\ ResponsiveDecisionApplicationProgressProperty(specification)"
        ),
        "HistoricalProtectedCandidateOwned": (
            "/\\ candidate.node \\in Responsive "
            "/\\ HistoricalRecoveryTarget(candidate.node) "
            "/\\ ProtectedCandidateOwned(candidate)"
        ),
        "HistoricalProtectedOwnedAtServiceRank": (
            "/\\ gst /\\ HistoricalProtectedCandidateOwned(candidate) "
            "/\\ CandidateServiceRank(candidate) = rank"
        ),
        "HistoricalProtectedServiceOwnershipExit": (
            "~HistoricalProtectedCandidateOwned(candidate)"
        ),
        "HistoricalProtectedServiceRankProgressProperty": (
            "specification => \\A candidate \\in AsyncCandidateSet, "
            "rank \\in OwnedServiceRankCarrier: "
            "HistoricalProtectedOwnedAtServiceRank(candidate, rank) "
            "~> (HistoricalProtectedServiceOwnershipExit(candidate) "
            "\\/ \\E lower \\in SetLessThan( rank, "
            "OwnedServiceRankOrdering, OwnedServiceRankCarrier): "
            "HistoricalProtectedOwnedAtServiceRank(candidate, lower))"
        ),
        "HistoricalProtectedStageRankProgressProperty": (
            "specification => \\A candidate \\in AsyncCandidateSet, "
            "position \\in Nat: (gst "
            "/\\ HistoricalProtectedCandidateOwned(candidate) "
            "/\\ CandidateServiceRank(candidate) = <<stage, position>>) "
            "~> (HistoricalProtectedServiceOwnershipExit(candidate) "
            "\\/ \\E lower \\in SetLessThan( <<stage, position>>, "
            "OwnedServiceRankOrdering, OwnedServiceRankCarrier): "
            "HistoricalProtectedOwnedAtServiceRank(candidate, lower))"
        ),
        "HistoricalProtectedStage2RankProgressProperty": (
            "HistoricalProtectedStageRankProgressProperty(specification, 2)"
        ),
        "HistoricalProtectedStage3RankProgressProperty": (
            "HistoricalProtectedStageRankProgressProperty(specification, 3)"
        ),
        "HistoricalProtectedStage4RankProgressProperty": (
            "HistoricalProtectedStageRankProgressProperty(specification, 4)"
        ),
        "HistoricalProtectedStage5RankProgressProperty": (
            "HistoricalProtectedStageRankProgressProperty(specification, 5)"
        ),
        "HistoricalProtectedStage6RankProgressProperty": (
            "HistoricalProtectedStageRankProgressProperty(specification, 6)"
        ),
        "HistoricalProtectedServiceRankLeafProperties": (
            "/\\ HistoricalProtectedStage2RankProgressProperty(specification) "
            "/\\ HistoricalProtectedStage3RankProgressProperty(specification) "
            "/\\ HistoricalProtectedStage4RankProgressProperty(specification) "
            "/\\ HistoricalProtectedStage5RankProgressProperty(specification) "
            "/\\ HistoricalProtectedStage6RankProgressProperty(specification)"
        ),
        "HistoricalProtectedCandidateStarvationProperty": (
            "specification => \\A candidate \\in AsyncCandidateSet: "
            "(gst /\\ HistoricalProtectedCandidateOwned(candidate)) "
            "~> HistoricalProtectedServiceOwnershipExit(candidate)"
        ),
        "HistoricalCommitCertificateDiscoveryPending": (
            "/\\ AsyncStrongTypeInvariant /\\ gst "
            "/\\ HistoricalCommitCertificateDiscoveryDue(node)"
        ),
        "HistoricalCommitCertificateDiscoveryOutcome": (
            "\\/ NodeHasDecision(node) "
            "\\/ /\\ HistoricalRecoveryTarget(node) "
            "/\\ ActiveCommitCertificateRequests(node) # {}"
        ),
        "HistoricalCommitCertificateDiscoveryPersistenceObligation": (
            "\\A node \\in Responsive: "
            "HistoricalCommitCertificateDiscoveryPending(node) "
            "/\\ [AsyncNext]_AsyncAllVars "
            "=> HistoricalCommitCertificateDiscoveryPending(node)' "
            "\\/ HistoricalCommitCertificateDiscoveryOutcome(node)'"
        ),
        "HistoricalCommitCertificateDiscoveryPersistenceUnless": (
            "[][HistoricalCommitCertificateDiscoveryPending(node) "
            "/\\ ~HistoricalCommitCertificateDiscoveryOutcome(node) "
            "=> HistoricalCommitCertificateDiscoveryPending(node)' "
            "\\/ HistoricalCommitCertificateDiscoveryOutcome(node)']_AsyncAllVars"
        ),
        "HistoricalCommitCertificateDiscoveryPersistenceProperty": (
            "specification => \\A node \\in Responsive: "
            "HistoricalCommitCertificateDiscoveryPersistenceUnless(node)"
        ),
        "HistoricalRecoveryTargetRemoteServerInvariant": (
            "\\A node \\in Responsive: HistoricalRecoveryTarget(node) "
            "=> CommitCertificateRequestOutbox(node) # {}"
        ),
        "HistoricalRecoveryTargetRemoteServerProperty": (
            "specification => []HistoricalRecoveryTargetRemoteServerInvariant"
        ),
        "HistoricalCommitCertificateDiscoveryClockProgressProperty": (
            "specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalRecoveryTarget(node)) "
            "~> (NodeHasDecision(node) "
            "\\/ /\\ HistoricalRecoveryTarget(node) "
            "/\\ \\/ ActiveCommitCertificateRequests(node) # {} "
            "\\/ asyncNow >= AsyncRoundTimeout)"
        ),
        "HistoricalCommitCertificateRequestScheduled": (
            "/\\ HistoricalRecoveryTarget(node) "
            "/\\ \\E request \\in ActiveCommitCertificateRequests(node): "
            "ItemScheduled(request)"
        ),
        "HistoricalCommitCertificateResponseScheduled": (
            "/\\ HistoricalRecoveryTarget(node) "
            "/\\ \\E response \\in AsyncNetworkItems: "
            "/\\ response.kind = \"CommitCertificateResponse\" "
            "/\\ response.envelope.recipient = node "
            "/\\ CommitCertificateResponseAuthorized(response) "
            "/\\ ItemScheduled(response)"
        ),
        "HistoricalCommitDecisionDirectEvidence": (
            "/\\ candidate.evidence \\in asyncSentItems "
            '/\\ candidate.evidence.kind = "CommitQC" '
            "/\\ candidate.evidence.envelope = "
            "QcEnvelope(candidate.node, qc) "
            "/\\ candidate.causalOrigin = "
            "AsyncDeliveryCandidateCausalOriginAt("
            "candidate.evidence, context)"
        ),
        "HistoricalCommitDecisionResponseEvidence": (
            "/\\ candidate.evidence \\in asyncSentItems "
            '/\\ candidate.evidence.kind = "CommitCertificateResponse" '
            "/\\ candidate.evidence.source = "
            "candidate.evidence.envelope.request.envelope.recipient "
            "/\\ candidate.evidence.envelope.recipient = candidate.node "
            "/\\ candidate.evidence.envelope.qc = qc "
            "/\\ CommitCertificateRequestAuthorized( "
            "candidate.evidence.envelope.request) "
            "/\\ candidate.causalOrigin = "
            "AsyncCommitCertificateResponseCandidateCausalOriginAt( "
            "candidate.evidence, context)"
        ),
        "HistoricalCommitDecisionCandidateOwned": (
            "\\E candidate \\in AsyncCandidateSet, qc \\in commitQCs: "
            "/\\ candidate.node = node /\\ candidate.kind = kind "
            '/\\ kind \\in {"DeliverQC", "BeginDecision", '
            '"PersistDecision"} '
            "/\\ qc.context = context /\\ qc.phase = \"Commit\" "
            "/\\ candidate.consumerContext = context "
            "/\\ candidate.view = qc.view "
            "/\\ candidate.subject = qc.subject "
            "/\\ HistoricalProtectedCandidateOwned(candidate) "
            "/\\ \\/ HistoricalCommitDecisionDirectEvidence(candidate, qc) "
            "\\/ HistoricalCommitDecisionResponseEvidence(candidate, qc) "
            '/\\ IF kind = "DeliverQC" THEN candidate.item = '
            'IF candidate.evidence.kind = "CommitQC" '
            "THEN candidate.evidence "
            "ELSE DiscoveredCommitQcItem(candidate.evidence) "
            "ELSE candidate.item = NoAsyncItem"
        ),
        "HistoricalActiveRequestRetransmissionProgressLeaf": (
            "specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalRecoveryTarget(node) "
            "/\\ ActiveCommitCertificateRequests(node) # {}) "
            "~> (NodeHasDecision(node) "
            "\\/ HistoricalCommitCertificateRequestScheduled(node))"
        ),
        "HistoricalCommitRequestServeProgressLeaf": (
            "StarvationFreedomProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalCommitCertificateRequestScheduled(node)) "
            "~> (NodeHasDecision(node) "
            "\\/ HistoricalCommitCertificateResponseScheduled(node)))"
        ),
        "HistoricalCommitResponseAdmissionProgressLeaf": (
            "specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalCommitCertificateResponseScheduled(node)) "
            "~> (NodeHasDecision(node) "
            "\\/ HistoricalCommitDecisionCandidateOwned( "
            "node, \"DeliverQC\"))"
        ),
        "HistoricalCommitDeliveryProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalCommitDecisionCandidateOwned(node, \"DeliverQC\")) "
            "~> (NodeHasDecision(node) "
            "\\/ HistoricalCommitDecisionCandidateOwned( "
            "node, \"BeginDecision\")))"
        ),
        "HistoricalBeginDecisionProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalCommitDecisionCandidateOwned( "
            "node, \"BeginDecision\")) "
            "~> (NodeHasDecision(node) "
            "\\/ HistoricalCommitDecisionCandidateOwned( "
            "node, \"PersistDecision\")))"
        ),
        "HistoricalPersistDecisionProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalCommitDecisionCandidateOwned( "
            "node, \"PersistDecision\")) ~> NodeHasDecision(node))"
        ),
        "HistoricalCommitCertificateConcreteLeafProperties": (
            "/\\ HistoricalActiveRequestRetransmissionProgressLeaf(specification) "
            "/\\ HistoricalCommitRequestServeProgressLeaf(specification) "
            "/\\ HistoricalCommitResponseAdmissionProgressLeaf(specification) "
            "/\\ HistoricalCommitDeliveryProgressLeaf(specification) "
            "/\\ HistoricalBeginDecisionProgressLeaf(specification) "
            "/\\ HistoricalPersistDecisionProgressLeaf(specification)"
        ),
        "HistoricalDecisionRecordMatches": (
            "/\\ decision \\in decisions /\\ decision.node = node "
            "/\\ decision.qc.context = context "
            "/\\ decision.qc.phase = \"Commit\""
        ),
        "HistoricalDecisionPipelineKindOwned": (
            "/\\ HistoricalRecoveryTarget(node) "
            "/\\ \\E decision \\in decisions: "
            "/\\ HistoricalDecisionRecordMatches(node, decision) "
            "/\\ DecisionPipelineKindOwned(node, decision.qc, kind)"
        ),
        "HistoricalDecisionCertifiedRequestActive": (
            "/\\ HistoricalRecoveryTarget(node) "
            "/\\ \\E decision \\in decisions: "
            "/\\ HistoricalDecisionRecordMatches(node, decision) "
            "/\\ DecisionCertifiedRequestActive(node, decision.qc)"
        ),
        "HistoricalDecisionRecoveryFrontier": (
            "\\/ NodeHasApplication(node) "
            "\\/ HistoricalDecisionPipelineKindOwned(node, \"FetchBody\") "
            "\\/ HistoricalDecisionPipelineKindOwned("
            "node, \"RequestCertifiedBody\") "
            "\\/ HistoricalDecisionCertifiedRequestActive(node) "
            "\\/ HistoricalDecisionPipelineKindOwned("
            "node, \"FetchCertifiedBody\") "
            "\\/ HistoricalDecisionPipelineKindOwned(node, \"StoreBody\") "
            "\\/ HistoricalDecisionPipelineKindOwned(node, \"ValidateBody\") "
            "\\/ HistoricalDecisionPipelineKindOwned(node, \"Apply\")"
        ),
        "HistoricalDecisionFrontierAvailabilityProperty": (
            "specification => []\\A node \\in Responsive: "
            "(gst /\\ HistoricalRecoveryTarget(node) "
            "/\\ NodeHasDecision(node)) "
            "=> HistoricalDecisionRecoveryFrontier(node)"
        ),
        "HistoricalDecisionFetchProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalDecisionPipelineKindOwned(node, \"FetchBody\")) "
            "~> (NodeHasApplication(node) "
            "\\/ HistoricalDecisionPipelineKindOwned( "
            "node, \"RequestCertifiedBody\") "
            "\\/ HistoricalDecisionCertifiedRequestActive(node) "
            "\\/ HistoricalDecisionPipelineKindOwned( "
            "node, \"ValidateBody\")))"
        ),
        "HistoricalDecisionRequestBodyProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalDecisionPipelineKindOwned( "
            "node, \"RequestCertifiedBody\")) "
            "~> (NodeHasApplication(node) "
            "\\/ HistoricalDecisionCertifiedRequestActive(node)))"
        ),
        "HistoricalDecisionCertifiedResponseProgressLeaf": (
            "(/\\ StarvationFreedomProperty(specification) "
            "/\\ HistoricalProtectedCandidateStarvationProperty(specification)) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalDecisionCertifiedRequestActive(node)) "
            "~> (NodeHasApplication(node) "
            "\\/ HistoricalDecisionPipelineKindOwned( "
            "node, \"FetchCertifiedBody\")))"
        ),
        "HistoricalDecisionFetchCertifiedProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalDecisionPipelineKindOwned( "
            "node, \"FetchCertifiedBody\")) "
            "~> (NodeHasApplication(node) "
            "\\/ HistoricalDecisionPipelineKindOwned( "
            "node, \"StoreBody\")))"
        ),
        "HistoricalDecisionStoreProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalDecisionPipelineKindOwned(node, \"StoreBody\")) "
            "~> (NodeHasApplication(node) "
            "\\/ HistoricalDecisionPipelineKindOwned( "
            "node, \"ValidateBody\")))"
        ),
        "HistoricalDecisionValidateProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalDecisionPipelineKindOwned("
            "node, \"ValidateBody\")) "
            "~> (NodeHasApplication(node) "
            "\\/ HistoricalDecisionPipelineKindOwned(node, \"Apply\")))"
        ),
        "HistoricalDecisionApplyProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalDecisionPipelineKindOwned(node, \"Apply\")) "
            "~> NodeHasApplication(node))"
        ),
        "HistoricalDecisionConcreteLeafProperties": (
            "/\\ HistoricalDecisionFetchProgressLeaf(specification) "
            "/\\ HistoricalDecisionRequestBodyProgressLeaf(specification) "
            "/\\ HistoricalDecisionCertifiedResponseProgressLeaf(specification) "
            "/\\ HistoricalDecisionFetchCertifiedProgressLeaf(specification) "
            "/\\ HistoricalDecisionStoreProgressLeaf(specification) "
            "/\\ HistoricalDecisionValidateProgressLeaf(specification) "
            "/\\ HistoricalDecisionApplyProgressLeaf(specification)"
        ),
        "ResponsiveDecisionServiceOwnershipInvariant": (
            "\\A node \\in Responsive: "
            "(gst /\\ NodeHasDecision(node) /\\ ~NodeHasApplication(node)) "
            "=> \\/ node \\in AsyncCurrentResponsiveVoters "
            "\\/ HistoricalRecoveryTarget(node)"
        ),
        "ResponsiveDecisionServiceOwnershipProperty": (
            "specification => []ResponsiveDecisionServiceOwnershipInvariant"
        ),
        "HistoricalRecoveryAsyncTemporalClosurePremises": (
            "/\\ HistoricalCommitCertificateDiscoveryPersistenceProperty(specification) "
            "/\\ HistoricalRecoveryTargetRemoteServerProperty(specification) "
            "/\\ HistoricalCommitCertificateDiscoveryClockProgressProperty(specification) "
            "/\\ HistoricalProtectedServiceRankLeafProperties(specification) "
            "/\\ HistoricalCommitCertificateConcreteLeafProperties(specification) "
            "/\\ HistoricalDecisionFrontierAvailabilityProperty(specification) "
            "/\\ HistoricalDecisionConcreteLeafProperties(specification) "
            "/\\ ResponsiveDecisionServiceOwnershipProperty(specification) "
            "/\\ ApplicationCompletionProgressProperty(specification)"
        ),
        "HistoricalRecoveryAsyncRemainingCorridorPremises": (
            "/\\ HistoricalCommitCertificateDiscoveryClockProgressProperty(specification) "
            "/\\ HistoricalProtectedServiceRankLeafProperties(specification) "
            "/\\ HistoricalCommitCertificateConcreteLeafProperties(specification) "
            "/\\ HistoricalDecisionFrontierAvailabilityProperty(specification) "
            "/\\ HistoricalDecisionConcreteLeafProperties(specification) "
            "/\\ ApplicationCompletionProgressProperty(specification)"
        ),
        "HistoricalLockedBodyRecoveryOutcome": (
            "\\/ HistoricalLockedBodySourceRetired(node, qc) "
            "\\/ HistoricalLockedBodyRecoveryTerminal(node, qc)"
        ),
        "HistoricalLockedCommitCarrierRecoveryProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedCommitRecoveryWitness(node, qc) "
            "/\\ ~HistoricalLockedBodyValidated(node, qc)) "
            "~> (HistoricalLockedBodyRecoveryOutcome(node, qc) "
            "\\/ HistoricalLockedBodyRestartAuthority(node, qc) "
            "\\/ HistoricalLockedBodyFetchOwned(node, qc) "
            "\\/ HistoricalLockedCertifiedRequestActive(node, qc) "
            "\\/ HistoricalLockedBodyValidateOwned(node, qc))"
        ),
        "HistoricalLockedRestartRecoveryProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedBodyRestartAuthority(node, qc)) "
            "~> (HistoricalLockedBodyRecoveryOutcome(node, qc) "
            "\\/ HistoricalLockedBodyFetchOwned(node, qc))"
        ),
        "HistoricalLockedFetchRecoveryProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedBodyFetchOwned(node, qc)) "
            "~> (HistoricalLockedBodyRecoveryOutcome(node, qc) "
            "\\/ HistoricalLockedCertifiedRequestActive(node, qc) "
            "\\/ HistoricalLockedBodyValidateOwned(node, qc))"
        ),
        "HistoricalLockedRequestCandidateProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedBodyRequestOwned(node, qc)) "
            "~> (HistoricalLockedBodyRecoveryOutcome(node, qc) "
            "\\/ HistoricalLockedCertifiedRequestActive(node, qc))"
        ),
        "HistoricalLockedActiveRequestProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedCertifiedRequestActive(node, qc)) "
            "~> (HistoricalLockedBodyRecoveryOutcome(node, qc) "
            "\\/ HistoricalLockedBodyCertifiedFetchOwned(node, qc))"
        ),
        "HistoricalLockedCertifiedFetchProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedBodyCertifiedFetchOwned(node, qc)) "
            "~> (HistoricalLockedBodyRecoveryOutcome(node, qc) "
            "\\/ HistoricalLockedBodyStoreOwned(node, qc))"
        ),
        "HistoricalLockedStoreRecoveryProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedBodyStoreOwned(node, qc)) "
            "~> (HistoricalLockedBodyRecoveryOutcome(node, qc) "
            "\\/ HistoricalLockedBodyValidateOwned(node, qc))"
        ),
        "HistoricalLockedValidateRecoveryProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedBodyValidateOwned(node, qc)) "
            "~> HistoricalLockedBodyRecoveryOutcome(node, qc)"
        ),
        "HistoricalLockedBodyRecoveryConeLeafProperties": (
            "/\\ HistoricalLockedCommitCarrierRecoveryProgressLeaf(specification) "
            "/\\ HistoricalLockedRestartRecoveryProgressLeaf(specification) "
            "/\\ HistoricalLockedFetchRecoveryProgressLeaf(specification) "
            "/\\ HistoricalLockedRequestCandidateProgressLeaf(specification) "
            "/\\ HistoricalLockedActiveRequestProgressLeaf(specification) "
            "/\\ HistoricalLockedCertifiedFetchProgressLeaf(specification) "
            "/\\ HistoricalLockedStoreRecoveryProgressLeaf(specification) "
            "/\\ HistoricalLockedValidateRecoveryProgressLeaf(specification)"
        ),
        "HistoricalLockedBodyRecoveryConeProperty": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(gst /\\ HistoricalLockedPrepareSource(node, qc)) "
            "~> HistoricalLockedBodyRecoveryOutcome(node, qc)"
        ),
    }
    for symbol, exact_body in operator_contracts.items():
        extracted = _top_level_operator_body(
            raw_source, symbol, preserve_string_contents=True
        )
        if extracted is None:
            errors.append(f"{path}: missing Async historical operator {symbol}")
            continue
        body, line = extracted
        normalized = " ".join(body.split())
        if normalized != exact_body:
            errors.append(
                f"{path}:{line}: {symbol} must equal only "
                f"{exact_body!r}; found {normalized!r}"
            )
    endpoint_symbols = (
        "HistoricalRecoveryTargetDecisionProgressProperty",
        "ResponsiveDecisionApplicationProgressProperty",
        "HistoricalRecoveryAsyncTemporalPrerequisites",
    )
    for symbol in endpoint_symbols:
        if _symbol_exists(source, symbol, theorem_only=True):
            errors.append(
                f"{path}: {symbol} must remain an operator property until its "
                "exact corridor is proved without extra premises"
            )
    if re.search(r"(?m)^CONSTANTS?\b", source):
        errors.append(
            f"{path}: Async historical-recovery child may not replace exact "
            "temporal predicates with unconstrained constants"
        )
    if re.search(r"\bResponsiveProtectedCandidateOwned\b", source):
        errors.append(
            f"{path}: historical rank may not reuse the current-voter-only "
            "ResponsiveProtectedCandidateOwned predicate"
        )
    theorem_contracts = {
        "HistoricalProtectedServiceRankProgressFromStageLeaves": (
            "\\A specification: "
            "HistoricalProtectedServiceRankLeafProperties(specification) "
            "=> HistoricalProtectedServiceRankProgressProperty(specification)",
            (
                "HistoricalProtectedStage2RankProgressProperty",
                "HistoricalProtectedStage3RankProgressProperty",
                "HistoricalProtectedStage4RankProgressProperty",
                "HistoricalProtectedStage5RankProgressProperty",
                "HistoricalProtectedStage6RankProgressProperty",
                "HistoricalProtectedStageRankProgressProperty",
            ),
        ),
        "HistoricalProtectedCandidateHasServiceRank": (
            "\\A candidate: /\\ AsyncTypeInvariant /\\ gst "
            "/\\ HistoricalProtectedCandidateOwned(candidate) "
            "=> \\E rank \\in OwnedServiceRankCarrier: "
            "HistoricalProtectedOwnedAtServiceRank(candidate, rank)",
            (
                "ScheduledCandidateServiceRankInCarrier",
                "HistoricalProtectedOwnedAtServiceRank",
            ),
        ),
        "HistoricalProtectedServiceRankProgressImpliesStarvation": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ HistoricalProtectedServiceRankProgressProperty( "
            "AsyncSpecAt(initialContext)) "
            "=> HistoricalProtectedCandidateStarvationProperty( "
            "AsyncSpecAt(initialContext))",
            (
                "OwnedServiceRankOrderingWellFounded",
                "WellFoundedLeadsTo",
                "HistoricalProtectedCandidateHasServiceRank",
            ),
        ),
        "HistoricalCommitCertificateDiscoveryReadinessFromClock": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ HistoricalRecoveryTargetRemoteServerProperty( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalCommitCertificateDiscoveryClockProgressProperty( "
            "AsyncSpecAt(initialContext)) "
            "=> \\A node \\in Responsive: "
            "(gst /\\ HistoricalRecoveryTarget(node)) "
            "~> (HistoricalCommitCertificateDiscoveryPending(node) "
            "\\/ HistoricalCommitCertificateDiscoveryOutcome(node))",
            (
                "DEF HistoricalRecoveryTargetRemoteServerProperty",
                "DEF HistoricalCommitCertificateDiscoveryClockProgressProperty",
                "HistoricalRecoveryTargetRemoteServerInvariant",
                "HistoricalCommitCertificateDiscoveryPending",
                "HistoricalCommitCertificateDiscoveryOutcome",
            ),
        ),
        "DirectHistoricalCommitCertificateDiscoveryPublishes": (
            "\\A node \\in ValidatorIds: "
            "DirectHistoricalCommitCertificateDiscoveryStep(node) "
            "=> /\\ HistoricalRecoveryTarget(node)' "
            "/\\ ActiveCommitCertificateRequests(node)' # {}",
            (
                "CommitCertificateDiscoveryStepWork",
                "PublishCommitCertificateRequests",
                "ActiveCommitCertificateRequests",
            ),
        ),
        "HistoricalCommitCertificateDiscoveryPrefixIsEnabled": (
            "\\A node \\in ValidatorIds: "
            "HistoricalCommitCertificateDiscoveryDue(node) "
            "=> ENABLED DirectHistoricalCommitCertificateDiscoveryStep(node)",
            (
                "ExpandENABLED",
                "DirectHistoricalCommitCertificateDiscoveryStep",
            ),
        ),
        "HistoricalCommitCertificateDiscoveryPendingEnablesFairPrefix": (
            "\\A node \\in Responsive: "
            "HistoricalCommitCertificateDiscoveryPending(node) "
            "=> ENABLED "
            "<<PostGstHistoricalCommitCertificateDiscovery(node)>>_AsyncAllVars",
            (
                "HistoricalRecoveryTargetsAreValidators",
                "HistoricalCommitCertificateDiscoveryPrefixIsEnabled",
                "DirectHistoricalCommitCertificateDiscoveryPublishes",
                "ENABLEDaxioms",
            ),
        ),
        "HistoricalCommitCertificateDiscoveryFairStepPublishes": (
            "\\A node \\in Responsive: "
            "/\\ HistoricalCommitCertificateDiscoveryPending(node) "
            "/\\ <<PostGstHistoricalCommitCertificateDiscovery(node)>>_AsyncAllVars "
            "=> HistoricalCommitCertificateDiscoveryOutcome(node)'",
            (
                "DirectHistoricalCommitCertificateDiscoveryPublishes",
                "HistoricalCommitCertificateDiscoveryOutcome",
            ),
        ),
        "FairHistoricalCommitCertificateDiscoveryFromPersistence": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ HistoricalCommitCertificateDiscoveryPersistenceProperty( "
            "AsyncSpecAt(initialContext)) "
            "=> \\A node \\in Responsive: "
            "HistoricalCommitCertificateDiscoveryPending(node) "
            "~> HistoricalCommitCertificateDiscoveryOutcome(node)",
            (
                "HistoricalCommitCertificateDiscoveryPendingEnablesFairPrefix",
                "HistoricalCommitCertificateDiscoveryFairStepPublishes",
                "HistoricalCommitCertificateDiscoveryPersistenceUnless",
                "HistoricalCommitCertificateDiscoveryPersistenceProperty",
                "WF_AsyncAllVars(",
                "PostGstHistoricalCommitCertificateDiscovery(node)",
            ),
        ),
        "HistoricalActiveCommitCertificateRequestReachesDecision": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalProtectedServiceRankLeafProperties( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalCommitCertificateConcreteLeafProperties( "
            "AsyncSpecAt(initialContext)) "
            "=> \\A node \\in Responsive: "
            "(gst /\\ HistoricalRecoveryTarget(node) "
            "/\\ ActiveCommitCertificateRequests(node) # {}) "
            "~> NodeHasDecision(node)",
            (
                "HistoricalProtectedServiceRankProgressFromStageLeaves",
                "HistoricalProtectedServiceRankProgressImpliesStarvation",
                "StarvationFreedomObligation",
                "HistoricalActiveRequestRetransmissionProgressLeaf",
                "HistoricalCommitRequestServeProgressLeaf",
                "HistoricalCommitResponseAdmissionProgressLeaf",
                "HistoricalCommitDeliveryProgressLeaf",
                "HistoricalBeginDecisionProgressLeaf",
                "HistoricalPersistDecisionProgressLeaf",
            ),
        ),
        "HistoricalTargetDecisionReachesApplicationFromConcreteLeaves": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalProtectedServiceRankLeafProperties( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalDecisionFrontierAvailabilityProperty( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalDecisionConcreteLeafProperties( "
            "AsyncSpecAt(initialContext)) "
            "=> \\A node \\in Responsive: "
            "(gst /\\ HistoricalRecoveryTarget(node) "
            "/\\ NodeHasDecision(node)) ~> NodeHasApplication(node)",
            (
                "HistoricalProtectedServiceRankProgressFromStageLeaves",
                "HistoricalProtectedServiceRankProgressImpliesStarvation",
                "StarvationFreedomObligation",
                "HistoricalDecisionFrontierAvailabilityProperty",
                "HistoricalDecisionFetchProgressLeaf",
                "HistoricalDecisionRequestBodyProgressLeaf",
                "HistoricalDecisionCertifiedResponseProgressLeaf",
                "HistoricalDecisionFetchCertifiedProgressLeaf",
                "HistoricalDecisionStoreProgressLeaf",
                "HistoricalDecisionValidateProgressLeaf",
                "HistoricalDecisionApplyProgressLeaf",
            ),
        ),
        "HistoricalRecoveryTargetDecisionFromExactCorridor": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalRecoveryAsyncTemporalClosurePremises( "
            "AsyncSpecAt(initialContext)) "
            "=> HistoricalRecoveryTargetDecisionProgressProperty( "
            "AsyncSpecAt(initialContext))",
            (
                "FairHistoricalCommitCertificateDiscoveryFromPersistence",
                "HistoricalCommitCertificateDiscoveryReadinessFromClock",
                "HistoricalActiveCommitCertificateRequestReachesDecision",
                "HistoricalCommitCertificateDiscoveryOutcome",
            ),
        ),
        "ResponsiveDecisionApplicationFromExactCorridor": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalRecoveryAsyncTemporalClosurePremises( "
            "AsyncSpecAt(initialContext)) "
            "=> ResponsiveDecisionApplicationProgressProperty( "
            "AsyncSpecAt(initialContext))",
            (
                "HistoricalTargetDecisionReachesApplicationFromConcreteLeaves",
                "ApplicationCompletionProgressProperty",
                "ResponsiveDecisionServiceOwnershipProperty",
                "AsyncSpecAlwaysUsesFixedResponsiveVoters",
            ),
        ),
        "HistoricalRecoveryAsyncTemporalPrerequisitesFromExactCorridor": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalRecoveryAsyncTemporalClosurePremises( "
            "AsyncSpecAt(initialContext)) "
            "=> HistoricalRecoveryAsyncTemporalPrerequisites( "
            "AsyncSpecAt(initialContext))",
            (
                "HistoricalRecoveryTargetDecisionFromExactCorridor",
                "ResponsiveDecisionApplicationFromExactCorridor",
            ),
        ),
        "HistoricalLockedBodyRecoveryConeComposesFromExactLeaves": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ HistoricalLockedBodyRecoveryConeLeafProperties( "
            "AsyncSpecAt(initialContext)) "
            "=> HistoricalLockedBodyRecoveryConeProperty( "
            "AsyncSpecAt(initialContext))",
            (
                "AsyncSpecAlwaysHistoricalLockedBodyRecoveryStage",
                "HistoricalLockedCommitCarrierRecoveryProgressLeaf",
                "HistoricalLockedRestartRecoveryProgressLeaf",
                "HistoricalLockedFetchRecoveryProgressLeaf",
                "HistoricalLockedRequestCandidateProgressLeaf",
                "HistoricalLockedActiveRequestProgressLeaf",
                "HistoricalLockedCertifiedFetchProgressLeaf",
                "HistoricalLockedStoreRecoveryProgressLeaf",
                "HistoricalLockedValidateRecoveryProgressLeaf",
                "HistoricalLockedBodyRecoveryStageInvariant",
                "HistoricalLockedBodyRecoveryOutcome",
                "PTL",
            ),
        ),
    }
    for symbol, (exact_statement, proof_tokens) in theorem_contracts.items():
        extracted = _top_level_theorem_body(
            raw_source, symbol, preserve_string_contents=True
        )
        if extracted is None:
            errors.append(f"{path}: missing Async historical theorem {symbol}")
            continue
        body, line = extracted
        parts = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
        )
        statement = " ".join(parts[0].split())
        if statement != exact_statement:
            errors.append(
                f"{path}:{line}: {symbol} must state only "
                f"{exact_statement!r}; found {statement!r}"
            )
        proof = parts[1] if len(parts) == 2 else ""
        missing = tuple(
            token
            for token in proof_tokens
            if not _tla_dependency_present(proof, token)
        )
        vacuous = re.search(
            r"(?:\bASSUME\s+FALSE\b|\bPROVE\s+TRUE\b|\bBY\s+TRUE\b)",
            proof,
        )
        if len(parts) != 2 or missing or vacuous is not None:
            errors.append(
                f"{path}:{line}: {symbol} proof must retain exact historical "
                "dependencies without a vacuous proof; "
                f"missing={missing!r}, vacuous={vacuous is not None}, "
                f"has_proof={len(parts) == 2}"
            )
    return errors
def _persistent_recovery_cut_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Bind crash-safe producer and live leader-wire recovery cuts to Rust."""
    base = repo_root / "crates" / "iroha_core" / "src" / "sumeragi"
    paths = {
        "adapter": base / "v2.rs",
        "runtime": base / "v2_runtime.rs",
        "effects": base / "v2_effects.rs",
        "store": base / "serviced_candidate_store.rs",
        "ingress": base / "mod.rs",
        "worker": base / "v2_worker.rs",
        "formal": repo_root
        / "formal"
        / "sumeragi_v2"
        / "SumeragiV2AsyncNetwork.tla",
    }
    errors: list[str] = []
    for path in paths.values():
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: persistent recovery-cut source must be a regular file"
            )
    if errors:
        return errors
    sources: dict[str, str] = {}
    for name, path in paths.items():
        if name in {"adapter", "runtime", "effects", "worker"}:
            _, sources[name] = _read_reviewed_rust_source(
                repo_root,
                path.relative_to(repo_root).as_posix(),
                errors,
                f"persistent recovery-cut {name} source",
            )
        else:
            sources[name] = path.read_text(encoding="utf-8")
    if errors:
        return errors
    def require_context_item(
        source_name: str,
        item_name: str,
        context: tuple[tuple[str, ...], ...],
        description: str,
    ) -> RustItem | None:
        path = paths[source_name]
        matches = [
            item
            for item in rust_items(sources[source_name], item_name)
            if item.brace_context == context
        ]
        if len(matches) != 1:
            errors.append(
                f"{path}: require exactly one {description} item {item_name} "
                f"in context {context!r}; found {len(matches)}"
            )
            return None
        return matches[0]

    adapter_context = (("impl", "SumeragiV2Adapter"),)
    runtime_context = (
        ("impl", "SerializedV2Runtime", "<", "SumeragiV2Adapter", ">"),
    )
    executor_context = (
        (
            "impl",
            "<",
            "R",
            ":",
            "EffectRuntime",
            ">",
            "V2EffectExecutor",
            "<",
            "R",
            ">",
        ),
    )
    store_context = (("impl", "LeaderWireLifecycleStoreGate"),)
    ingress_context = (("impl", "FairV2Ingress"),)
    worker_services_context = (
        ("impl", "V2EffectServices", "for", "ProductionV2Services"),
    )

    persist_release = require_context_item(
        "adapter",
        "persist_unrecorded_producer_releases",
        adapter_context,
        "atomic persistent producer release",
    )
    for sequence, description in (
        (
            """
if self.ensure_canonical_reclaimed_producer_state_after_decision()? {
    return Ok(());
}
""",
            "producer release must not resurrect an epoch reclaimed by durable Decision",
        ),
        (
            """
if !addresses.insert(token.address) {
    return Err(self.fail_serviced_candidate_store(
        "one producer address had multiple simultaneous release authorities"
            .to_owned(),
    ));
}
""",
            "producer release must reject duplicate durable addresses",
        ),
        (
            """
if current.status() != ProducerContinuationStatus::Reserved
    || current.identity().address() != token.address
    || self.durable_producer_continuations.get(&token.address) != Some(current)
    || self.pending_producer_handoffs.contains_key(&token.address)
{
""",
            "producer release must exact-match process and durable aliases before mutation",
        ),
        (
            """
let process_previous = self.producer_continuations.clone();
let durable_previous = self.durable_producer_continuations.clone();
let dormant_previous = self.restored_dormant_producer_continuations.clone();
let handoffs_previous = self.pending_producer_handoffs.clone();
""",
            "producer release must retain one complete rollback image",
        ),
        (
            """
if let Err(reason) = self
    .serviced_candidate_store
    .persist_with_producer_continuations(
        &self.durable_serviced_candidates,
        &self.durable_producer_continuations,
        self.serviced_candidates_decision_reclaimed,
    )
{
    self.producer_continuations = process_previous;
    self.durable_producer_continuations = durable_previous;
    self.restored_dormant_producer_continuations = dormant_previous;
    self.pending_producer_handoffs = handoffs_previous;
""",
            "failed producer persistence must roll every alias back",
        ),
    ):
        _require_rust_token_sequence(
            paths["adapter"], persist_release, sequence, description, errors
        )

    deferred_release = require_context_item(
        "adapter",
        "release_deferred_producer_continuations_before_owner_removal",
        adapter_context,
        "persist-before-remove Busy producer release",
    )
    _require_rust_token_sequence(
        paths["adapter"],
        deferred_release,
        """
let active = self.all_deferred_admission_ordinals();
if !retiring.is_subset(&active)
    || !self
        .deferred_producer_continuations
        .keys()
        .all(|ordinal| active.contains(ordinal))
{
""",
        "deferred release must retain one exact Busy owner for every producer",
        errors,
    )
    _require_rust_token_sequence(
        paths["adapter"],
        deferred_release,
        """
self.persist_unrecorded_producer_releases(&tokens)?;
for ordinal in retiring {
    self.deferred_producer_continuations.remove(ordinal);
}
""",
        "deferred release must persist the batch before dropping ownership aliases",
        errors,
    )

    retire_deferred_body = require_context_item(
        "adapter",
        "retire_deferred_body_available",
        adapter_context,
        "exact deferred BodyAvailable retirement",
    )
    _require_rust_token_sequence(
        paths["adapter"],
        retire_deferred_body,
        """
self.release_deferred_producer_continuations_before_owner_removal(&retiring)?;
let before = self.deferred_completions.len();
self.deferred_completions.retain(|input| !matches(input));
""",
        "deferred BodyAvailable retirement must persist before queue removal",
        errors,
    )

    retire_deferred_pipeline = require_context_item(
        "adapter",
        "retire_deferred_body_pipeline_completions",
        adapter_context,
        "transactional deferred body-pipeline retirement",
    )
    _require_rust_token_sequence(
        paths["adapter"],
        retire_deferred_pipeline,
        """
if retiring.len() != retirements.len() {
    return Err(self.fail_serviced_candidate_store(
        "one deferred body occurrence occupied multiple serialized queues".to_owned(),
    ));
}
self.release_deferred_producer_continuations_before_owner_removal(&retiring)?;
""",
        "pipeline retirement must reject duplicate owners before persistent release",
        errors,
    )

    frontier = require_context_item(
        "adapter",
        "reconcile_restored_reserved_producer_frontier",
        adapter_context,
        "restart-only Reserved producer frontier reconciliation",
    )
    for sequence, description in (
        (
            """
if self.reducer.durable_state().decision().is_some() {
    return Ok(());
}
let current_view = self.reducer.current_tag().view();
let protected = self.reducer.durable_state().locked().map(|certificate| {
""",
            "restart reconciliation must derive its cut from replayed WAL state",
        ),
        (
            """
if candidate.source_view() > current_view {
    return Err(self.fail_serviced_candidate_store(
        "restored producer originated beyond the replayed durable view".to_owned(),
    ));
}
if candidate.source_view() == current_view {
    continue;
}
""",
            "restart reconciliation must reject the future and retain current-view producers",
        ),
        (
            """
let protects_body_pipeline = protected.is_some_and(|(view, subject)| {
    candidate.source_view() == view
        && candidate.target() == Some(subject)
        && matches!(
            stage,
            ServicedCandidateStage::LocalProposalReady
                | ServicedCandidateStage::BodyAvailable
                | ServicedCandidateStage::BodyStored
                | ServicedCandidateStage::ValidationCompleted
        )
});
if !protects_body_pipeline {
    retiring.push(address);
}
""",
            "only the exact protected-lock body pipeline may survive an older restart frontier",
        ),
        (
            """
if let Err(reason) = self
    .serviced_candidate_store
    .persist_with_producer_continuations(
        &self.durable_serviced_candidates,
        &self.durable_producer_continuations,
        self.serviced_candidates_decision_reclaimed,
    )
{
    self.producer_continuations = process_previous;
    self.durable_producer_continuations = durable_previous;
    self.restored_dormant_producer_continuations = dormant_previous;
""",
            "restart frontier pruning must roll back all aliases on persistence failure",
        ),
    ):
        _require_rust_token_sequence(
            paths["adapter"], frontier, sequence, description, errors
        )

    adapter_open = require_context_item(
        "adapter",
        "open_with_aggregator_and_publication_with_capacity",
        adapter_context,
        "capacity-bound restart constructor",
    )
    _require_rust_item_context(
        paths["adapter"],
        adapter_open,
        adapter_context,
        "capacity-bound restart constructor",
        errors,
        expected_attributes=("#[allow(clippy::too_many_arguments)]",),
    )
    _require_rust_token_sequence(
        paths["adapter"],
        adapter_open,
        """
adapter.reconcile_restored_reserved_producer_frontier()?;
adapter.reclaim_serviced_candidates()?;
let replay_tag = adapter.reducer.current_tag();
""",
        "restart frontier pruning must precede runtime replay and dormant capacity installation",
        errors,
    )

    persistent_deferred = require_context_item(
        "adapter",
        "deferred_body_available_has_persistent_producer",
        adapter_context,
        "Busy-deferred persistent body-owner classifier",
    )
    _require_rust_token_sequence(
        paths["adapter"],
        persistent_deferred,
        """
if record.status() != ProducerContinuationStatus::Reserved
    || record.source_class() != ProducerContinuationSourceClass::VolatileBody
    || record.identity().address() != address
    || record.identity().stage() != ServicedCandidateStage::BodyAvailable as u8
    || self.durable_producer_continuations.get(&address) != Some(record)
""",
        "persistent deferred body classification must exact-match the stage-7 durable root",
        errors,
    )

    persistent_body = require_context_item(
        "runtime",
        "body_available_has_persistent_producer",
        runtime_context,
        "serialized persistent body-owner classifier",
    )
    _require_rust_token_sequence(
        paths["runtime"],
        persistent_body,
        """
if ingress && deferred {
    self.latch_fail_closed("one body completion retained two persistent producer carriers");
    return Err(
        "Sumeragi v2 body completion has duplicate persistent producer ownership"
            .to_owned(),
    );
}
Ok(ingress || deferred)
""",
        "one serialized body owner may have at most one persistent producer carrier",
        errors,
    )

    rebind_body = require_context_item(
        "runtime",
        "rebind_body_available",
        runtime_context,
        "persistent-root-preserving body rebind",
    )
    for sequence, description in (
        (
            """
let source_persistent =
    self.body_available_has_persistent_producer(previous, manifest)?;
let destination_persistent =
    self.body_available_has_persistent_producer(rebound, manifest)?;
if source_persistent && destination_persistent {
""",
            "rebind must classify both persistent roots before mutation",
        ),
        (
            """
if source_persistent {
    if !self.retire_body_available(rebound, manifest)? {
""",
            "a sole persistent source must retire the ordinary destination",
        ),
        (
            """
let ingress = self
    .ingress
    .rebind_canonical_body_available(previous, rebound, manifest);
let deferred = self
    .driver
    .rebind_deferred_body_available(previous, rebound, manifest);
ingress.saturating_add(deferred)
} else {
""",
            "a sole persistent source must be retagged rather than retired",
        ),
        (
            """
let deferred = match self
    .driver
    .retire_deferred_body_available(previous, manifest)
{
""",
            "a nonpersistent source must persist deferred release before coalescence",
        ),
    ):
        _require_rust_token_sequence(
            paths["runtime"], rebind_body, sequence, description, errors
        )

    retire_fetch_parent = require_context_item(
        "runtime",
        "retire_restored_body_fetch_parent",
        runtime_context,
        "pre-BodyAvailable restored Fetch retirement",
    )
    _require_rust_token_sequence(
        paths["runtime"],
        retire_fetch_parent,
        """
let AdapterEffect::FetchBody {
    round,
    subject,
    manifest,
    ..
} = effect
else {
""",
        "restored Fetch retirement must extract only exact FetchBody coordinates",
        errors,
    )
    _require_rust_token_sequence(
        paths["runtime"],
        retire_fetch_parent,
        """
if !ownership.exactly_binds_adapter_effect(effect) {
    self.latch_fail_closed(
        "restored body-fetch retirement changed its exact effect binding",
    );
""",
        "restored Fetch retirement must exact-match the bound Fetch effect",
        errors,
    )
    _require_rust_token_sequence(
        paths["runtime"],
        retire_fetch_parent,
        """
match self.driver.retire_restored_body_fetch_parent(
    *round,
    *subject,
    manifest.as_ref()
) {
""",
        "restored Fetch retirement must delegate durable parent lookup to the adapter",
        errors,
    )

    adapter_fetch_parent = require_context_item(
        "adapter",
        "retire_restored_body_fetch_parent",
        adapter_context,
        "durable coordinate-bound Fetch-parent retirement",
    )
    for sequence, description in (
        (
            """
if self.selected_producer_lifecycle.is_some()
    || round.context_id != self.wire_context.id()
    || round.height != self.wire_context.height
{
""",
            "Fetch-parent retirement must retain immutable height geometry",
        ),
        (
            """
manifest.validate(&self.wire_context)?;
if manifest.round != round || manifest.subject != subject {
    return Err(AdapterError::DurableBodyMismatch);
}
""",
            "a supplied Fetch manifest must exact-match its round and subject",
        ),
        (
            """
let [(address, record)] = coordinate_matches.as_slice() else {
    return match coordinate_matches.len() {
        0 => Ok(false),
        _ => Err(self.fail_serviced_candidate_store(
""",
            "manifest-less Fetch-parent lookup must select at most one dormant stage-7 record",
        ),
        (
            """
if expected_candidate.is_some_and(|expected| record.identity().candidate() != expected) {
    return Err(self.fail_serviced_candidate_store(
        "restored body-fetch manifest changed its persisted producer identity".to_owned(),
    ));
}
self.persist_restored_body_producer_retirement(*address, record)?;
""",
            "Fetch-parent retirement must bind full manifest identity before persistence",
        ),
    ):
        _require_rust_token_sequence(
            paths["adapter"], adapter_fetch_parent, sequence, description, errors
        )

    commit_fetch_retirement = require_context_item(
        "effects",
        "commit_pending_fetch_retirement",
        executor_context,
        "terminal pending-Fetch retirement",
    )
    _require_rust_token_sequence(
        paths["effects"],
        commit_fetch_retirement,
        """
if !retired_completion {
    let effect = plan.pending.task.adapter_effect();
    self.runtime
        .retire_restored_body_fetch_parent(&effect, plan.pending.task.ownership())
        .map_err(EffectExecutorError::Runtime)?;
}
let work_id = plan.pending.task.id();
let removed = self.pending_fetches.remove(&work_id);
""",
        "a terminal Fetch without a token must retire its restored parent before P/Q ownership",
        errors,
    )

    production_fetch_parent_retirement = require_context_item(
        "effects",
        "retire_restored_body_fetch_parent",
        (("impl", "EffectRuntime", "for", "SerializedV2Runtime"),),
        "production restored Fetch-parent retirement delegate",
    )
    _require_rust_token_sequence(
        paths["effects"],
        production_fetch_parent_retirement,
        """
SerializedV2Runtime::retire_restored_body_fetch_parent(self, effect, ownership)
""",
        "production EffectRuntime must not use the ordinary no-op Fetch-parent default",
        errors,
    )

    authority_advance = _require_rust_item(
        paths["store"], sources["store"], "advance_view", errors
    )
    _require_rust_item_context(
        paths["store"],
        authority_advance,
        (("impl", "LeaderWireRecoveryAuthority"),),
        "monotone leader-wire view authority",
        errors,
    )
    _require_rust_token_sequence(
        paths["store"],
        authority_advance,
        """
if durable_view < self.durable_view {
    return Err("leader-wire recovery authority regressed its durable view".to_owned());
}
""",
        "leader-wire view authority must reject regression",
        errors,
    )

    store_cut = require_context_item(
        "store",
        "advance_recovery_cut",
        store_context,
        "durable leader-wire live recovery cut",
    )
    for sequence, description in (
        (
            """
if !next.matches_geometry(self.context_id, self.height, self.owner) {
    return Err("leader-wire recovery cut changed immutable geometry".to_owned());
}
""",
            "leader-wire cut must retain frozen geometry",
        ),
        (
            """
if !next.monotonically_extends(state.recovery_authority) {
    return Err("leader-wire recovery cut is not monotone".to_owned());
}
""",
            "leader-wire cut must monotonically extend the WAL authority",
        ),
        (
            """
if retiring != *expected_dormant_slots || !retiring.is_subset(&state.replay_dormant) {
""",
            "durable and mirrored obsolete Dormant sets must be exactly equal",
        ),
        (
            """
let previous = state.clone();
state.recovery_authority = next;
for slot in &retiring {
""",
            "leader-wire cut must retain one complete rollback image before removal",
        ),
        (
            """
if !retiring.is_empty()
    && let Err(error) = self.persist_locked(&state)
{
    *state = previous;
    return Err(error);
}
""",
            "failed leader-wire persistence must restore authority and records",
        ),
    ):
        _require_rust_token_sequence(
            paths["store"], store_cut, sequence, description, errors
        )

    admit_ingress = require_context_item(
        "store",
        "admit_ingress",
        store_context,
        "durable leader-wire ingress admission",
    )
    _require_rust_token_sequence(
        paths["store"],
        admit_ingress,
        """
if state.recovery_authority.obsoletes(&token) {
    return Err(
        "leader-wire admission is obsolete under the durable recovery cut".to_owned(),
    );
}
""",
        "durable admission must reject an obsolete identity before lookup or mutation",
        errors,
    )

    fair_admission = _require_rust_item(
        paths["ingress"],
        sources["ingress"],
        "fair_v2_ingress_admit_leader_wire",
        errors,
    )
    _require_rust_token_sequence(
        paths["ingress"],
        fair_admission,
        """
if gate
    .identity_is_obsolete(&identity)
    .map_err(|_| FairV2IngressLeaderWireAdmissionError::Exhausted)?
{
    return Err(FairV2IngressLeaderWireAdmissionError::Rejected);
}
let durable_exact = gate
    .lookup_exact(&identity, &slot)
""",
        "fair ingress must reject below-cut wire before durable exact lookup",
        errors,
    )

    fair_cut = require_context_item(
        "ingress",
        "advance_leader_wire_recovery_cut",
        ingress_context,
        "gate-first fair-ingress recovery cut",
    )
    _require_rust_token_sequence(
        paths["ingress"],
        fair_cut,
        """
gate.advance_recovery_cut(next, &retiring)?;
for slot in &retiring {
    let removed = state
        .leader_wire_lifecycles
        .remove(slot)
""",
        "persistent gate publication must precede mirror pruning",
        errors,
    )

    entered_view = require_context_item(
        "worker",
        "entered_view",
        worker_services_context,
        "production certified-view recovery cut",
    )
    _require_rust_token_sequence(
        paths["worker"],
        entered_view,
        """
let next_recovery_authority = self
    .leader_wire_recovery_authority
    .advance_view(tag.view())?;
self.leader_wire_ingress
    .advance_leader_wire_recovery_cut(next_recovery_authority)?;
self.leader_wire_recovery_authority = next_recovery_authority;
""",
        "certified EnterView must publish the gate cut before exposing its authority",
        errors,
    )

    finish_decision = require_context_item(
        "worker",
        "finish_decision_serve_reconciliation",
        worker_services_context,
        "production durable-Decision recovery cut",
    )
    _require_rust_token_sequence(
        paths["worker"],
        finish_decision,
        """
if decided_subject.is_some() {
    let next = self.leader_wire_recovery_authority.with_durable_decision();
    self.leader_wire_ingress
        .advance_leader_wire_recovery_cut(next)?;
    self.leader_wire_recovery_authority = next;
}
""",
        "durable Decision must publish the all-wire gate cut before service reconciliation",
        errors,
    )

    regression_contracts = (
        (
            "adapter",
            "failed_busy_parent_retirement_retains_queue_and_durable_owner",
            "restores the exact queue and durable producer after injected failure",
        ),
        (
            "adapter",
            "strict_view_advance_retains_live_producer_admission_until_owner_release",
            "distinguishes live handoff retention from restart-only frontier pruning",
        ),
        (
            "adapter",
            "restart_frontier_retains_all_four_stages_of_the_protected_body_pipeline",
            "retains every exact protected-lock body-pipeline stage",
        ),
        (
            "adapter",
            "restart_frontier_rejects_reserved_producer_beyond_the_durable_view",
            "rejects a future-view producer during replay reconciliation",
        ),
        (
            "adapter",
            "durable_decision_release_does_not_restore_stale_process_only_predecessor",
            "keeps the canonical empty producer epoch after durable Decision",
        ),
        (
            "adapter",
            "body_rebind_coalescence_preserves_the_only_persistent_producer",
            "keeps the sole persistent rebind root across a second restart",
        ),
        (
            "runtime",
            "body_available_rebind_coalesces_exact_busy_deferred_destination_owner",
            "retains a persistent destination while retiring an ordinary source",
        ),
        (
            "runtime",
            "body_available_rebind_rejects_two_persistent_roots_before_mutation",
            "rejects two durable roots before either serialized owner changes",
        ),
        (
            "store",
            "leader_wire_live_recovery_cut_retires_only_dormant_records_and_is_monotone",
            "checks live Dormant-only cut, rollback, and high-water retention",
        ),
        (
            "worker",
            "entered_view_advances_live_leader_wire_recovery_cut",
            "rejects stale wire after live EnterView",
        ),
        (
            "worker",
            "durable_decision_advances_live_leader_wire_recovery_cut",
            "rejects every wire after durable Decision",
        ),
    )
    for source_name, item_name, _description in regression_contracts:
        _require_rust_item(
            paths[source_name], sources[source_name], item_name, errors
        )

    live_cut_regression = _require_rust_item(
        paths["store"],
        sources["store"],
        "leader_wire_live_recovery_cut_retires_only_dormant_records_and_is_monotone",
        errors,
    )
    _require_rust_token_sequence(
        paths["store"],
        live_cut_regression,
        """
assert!(restored.records().is_empty(), "{label}");
assert_eq!(restored.last_admission_ordinal(), 11, "{label}");
assert_eq!(restored.scheduler_ordinal_high_watermark(), 73, "{label}");
assert!(
    gate.identity_is_obsolete(&token.identity)
        .expect("inspect live recovery cut"),
""",
        "live leader-wire regression must retain both high-waters and reject the retired identity",
        errors,
    )
    _require_rust_token_sequence(
        paths["store"],
        live_cut_regression,
        """
assert!(
    gate.advance_recovery_cut(regressed, &BTreeSet::new())
        .is_err(),
    "{label} cannot regress durable view/Decision authority"
);
""",
        "live leader-wire regression must reject view and Decision regression",
        errors,
    )
    _require_rust_token_sequence(
        paths["store"],
        live_cut_regression,
        """
for retained_status in [
    LeaderWireLifecycleStatus::Ingress,
    LeaderWireLifecycleStatus::Runtime,
] {
""",
        "live leader-wire regression must retain active Ingress and Runtime owners",
        errors,
    )
    _require_rust_token_sequence(
        paths["store"],
        live_cut_regression,
        """
std::fs::create_dir(&gate.path).expect("block recovery-cut publication");
assert!(
    gate.advance_recovery_cut(
""",
        "live leader-wire regression must inject and observe persistent-cut rollback",
        errors,
    )

    stage_seven_regression = _require_rust_item(
        paths["adapter"],
        sources["adapter"],
        "restored_body_available_terminal_retirement_is_persistent_before_token_release",
        errors,
    )
    _require_rust_token_sequence(
        paths["adapter"],
        stage_seven_regression,
        """
assert_restored_stage_seven_retirement_does_not_resurrect(0xBB, false, false, false);
assert_restored_stage_seven_retirement_does_not_resurrect(0xBD, false, false, false);
""",
        "stage-7 regression must cover manifest-bound and manifest-less terminal Fetch retirement before reservation",
        errors,
    )

    formal_source = sources["formal"]
    formal_contracts = {
        "AsyncLeaderWireRecoveryCutObsoletesItem": (
            "DeliveryView(item) < nodeView[item.envelope.recipient]",
            "NodeHasDecision(item.envelope.recipient)",
        ),
        "AsyncLeaderWireAtomicAdmissionAllows": (
            "~AsyncLeaderWireRecoveryCutObsoletesItem(item)",
        ),
        "AsyncLeaderWireLifecycleRecoveryCutObsolete": (
            "AsyncLeaderWireLifecycleDormant(record)",
            "record.view < nodeView[record.recipient]",
            "NodeHasDecision(record.recipient)",
        ),
        "AsyncLeaderWireLifecycleCanTerminal": (
            "AsyncLeaderWireLifecycleRecoveryCutObsolete(record)",
        ),
        "RetireLeaderWireLifecycleSlot": (
            "IF AsyncLeaderWireLifecycleRecoveryCutObsolete(record)",
            "THEN asyncLeaderWireLifecycles \\ {record}",
        ),
    }
    for operator, required in formal_contracts.items():
        extracted = _top_level_operator_body(
            formal_source, operator, preserve_string_contents=True
        )
        if extracted is None:
            errors.append(
                f"{paths['formal']}: missing persistent recovery-cut operator {operator}"
            )
            continue
        body, line = extracted
        for token in required:
            if token not in body:
                errors.append(
                    f"{paths['formal']}:{line}: {operator} must retain "
                    f"persistent recovery-cut token {token!r}"
                )

    highwater_theorem = _top_level_theorem_body(
        formal_source,
        "LeaderWireRecoveryCutRetainsOrdinalHighwaters",
        preserve_string_contents=True,
    )
    if highwater_theorem is None:
        errors.append(
            f"{paths['formal']}: missing leader-wire recovery-cut high-water theorem"
        )
    else:
        body, line = highwater_theorem
        for token in (
            "RetireLeaderWireLifecycleSlot(slot)",
            "AsyncLeaderWireLifecycleRecoveryCutObsolete(record)",
            "AsyncNextIngressPhysicalOrdinal(node)' =",
            "AsyncNextCandidateLifecycleOrdinal(node)' =",
        ):
            if token not in body:
                errors.append(
                    f"{paths['formal']}:{line}: leader-wire recovery-cut "
                    f"high-water theorem must retain token {token!r}"
                )

    return errors
