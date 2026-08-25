# Executed lexically before the successor recovery tail contracts; do not import directly.


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


def _check_successor_snapshot_authority(
    recovery_path: Path,
    recovery_source: str,
    region: Any,
    require_tokens: Any,
    require_order: Any,
) -> None:
    """Bind snapshot and complete-tip successor activation authority."""

    snapshot_authority = region(
        recovery_path,
        recovery_source,
        "SnapshotSuccessorActivationAuthority::new",
        "fn new(record: &wire::SnapshotV2BootstrapRecord) -> Self",
        "\n    /// Imported snapshot height which anchors the first executable context.",
    )
    require_tokens(
        recovery_path,
        "SnapshotSuccessorActivationAuthority::new",
        snapshot_authority,
        (
            "record.context.snapshot_bootstrap.as_ref()",
            "expect(\"verified snapshot activation authority retains its anchor\")",
            "record_hash: HashOf::new(record), snapshot_height: anchor.snapshot_height, snapshot_block_hash: anchor.snapshot_block_hash, successor_context_id: record.context.id(),",
        ),
    )
    recovery = region(
        recovery_path,
        recovery_source,
        "recover_active_height_with_plan",
        "pub(crate) fn recover_active_height_with_plan(",
        "\nfn verify_state_kura_prefix(",
    )
    require_tokens(
        recovery_path,
        "recover_active_height_with_plan snapshot authority",
        recovery,
        (
            "authenticate_v2_snapshot_replay_boundary(kura, state, &replay_plan)?;",
            "if record.context() != &bootstrap.context || record.proofs_of_possession() != bootstrap.validator_set_pops",
            "let verified_context = VerifiedHeightContext::snapshot_bootstrap(bootstrap)?;",
            "RecoveredSuccessorActivationAuthority::SnapshotBootstrap( SnapshotSuccessorActivationAuthority::new(bootstrap), )",
        ),
    )
    require_order(
        recovery_path,
        "recover_active_height_with_plan snapshot authority",
        recovery,
        (
            "authenticate_v2_snapshot_replay_boundary(",
            "is_entirely_audited_snapshot_import()",
            "authenticated_snapshot_v2_bootstrap()",
            "record.context() != &bootstrap.context",
            "VerifiedHeightContext::snapshot_bootstrap(bootstrap)",
            "SnapshotSuccessorActivationAuthority::new(bootstrap)",
        ),
    )
    require_tokens(
        recovery_path,
        "recover_active_height_with_plan complete-tip authority",
        recovery,
        (
            "kura.v2_finality_artifact_with_receipt(durable_height)?",
            "let predecessor_record = context_store.load(durable_height)?",
            "let verified_predecessor = verify_persisted_height( kura, state, &context_store, predecessor_record, durable_height, )?;",
            "let predecessor_signature_policy = if durable_height == 1 { BlockSignaturePolicy::GenesisAuthority(genesis_public_key.clone()) } else { BlockSignaturePolicy::RotatingLeader };",
            "build_verified_successor(state, &context_store, &parent_artifact, &parent_receipt)?;",
            "let (verified_context, activation) = successor.into_parts();",
            "RecoveredCompleteTipActivationAuthority::authenticate( parent_artifact, parent_receipt, verified_predecessor, predecessor_signature_policy, &verified_context, activation, kura, )?;",
            "RecoveredSuccessorActivationAuthority::CompleteTip( complete_tip_activation, )",
        ),
    )
    require_order(
        recovery_path,
        "recover_active_height_with_plan complete-tip authority",
        recovery,
        (
            "verify_persisted_height(",
            "build_verified_successor(",
            "successor.into_parts()",
            "RecoveredCompleteTipActivationAuthority::authenticate(",
            "RecoveredSuccessorActivationAuthority::CompleteTip(",
        ),
    )
    verified_successor = region(
        recovery_path,
        recovery_source,
        "build_verified_successor",
        "pub(crate) fn build_verified_successor(",
        "\nfn verify_persisted_height(",
    )
    require_tokens(
        recovery_path,
        "build_verified_successor",
        verified_successor,
        (
            "DurableV2PredecessorIdentity::authenticate(parent_artifact, parent_receipt)?;",
            "if state_height != parent_height || state_block_hash != Some(predecessor.block_hash)",
            "if parent_record.context() != &parent_artifact.height_context",
            "VerifiedHeightContext::successor( expected, proofs, parent_artifact, parent_receipt, parent_record.proofs_of_possession(), )?;",
            "DurableSuccessorActivationAuthority { predecessor, successor_context_id: verified.context().id(), }",
            "DurableSuccessorActivationAuthority { predecessor, successor_context_id: verified_context.context().id(), }",
        ),
    )
    require_order(
        recovery_path,
        "build_verified_successor",
        verified_successor,
        (
            "DurableV2PredecessorIdentity::authenticate(",
            "state_height != parent_height",
            "parent_record.context() != &parent_artifact.height_context",
            "VerifiedHeightContext::successor(",
            "DurableSuccessorActivationAuthority",
        ),
    )


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
    for name, path in tuple(paths.items()):
        if path.suffix == ".rs":
            loaded_path, source = _read_reviewed_rust_source(
                repo_root,
                path.relative_to(repo_root).as_posix(),
                errors,
                f"persistent recovery-cut {name} source",
            )
            paths[name] = loaded_path
            sources[name] = source
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
if !state
    .recovery_authority
    .admits_ingress_identity(&token.identity)
{
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
    return Ok(FairV2IngressLeaderWireAdmission::Coalesced);
}
let durable_exact = gate
    .lookup_exact(&identity, &slot)
""",
        "fair ingress must coalesce below-cut wire before durable exact lookup",
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

    finish_runtime_step = require_context_item(
        "worker",
        "finish_runtime_step_reconciliation",
        worker_services_context,
        "production durable-Decision recovery cut",
    )
    _require_rust_token_sequence(
        paths["worker"],
        finish_runtime_step,
        """
if decided_subject.is_some() {
    let next = self.leader_wire_recovery_authority.with_durable_decision();
    self.leader_wire_ingress
        .advance_leader_wire_recovery_cut(next)?;
    self.leader_wire_recovery_authority = next;
}
""",
        "durable Decision must publish the all-wire gate cut during runtime-step reconciliation",
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
            "restart_frontier_retains_all_three_stages_of_the_protected_body_pipeline",
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
            "body_available_rejects_second_persistent_lifecycle_before_mutation",
            "rejects a second durable producer lifecycle before the original owner changes",
        ),
        (
            "runtime",
            "body_available_rebind_rejects_busy_source_and_restored_ingress_destination_before_mutation",
            "rejects Busy and restored-ingress durable roots before either carrier changes",
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


def _lifecycle_turn_driver_pending_kura_runner_source_fidelity_errors(
    paths, errors, item, require_order, reject_tokens, require_tokens
) -> None:
    pending_runner = item("pending_runner", "run_pending_kura_lifecycle_height")
    require_order(
        "pending_runner",
        pending_runner,
        "sealed pending-Kura lifecycle startup and ordinary successor handoff",
        (
            "close_ingress_for_rollover(&ingress_ready, &block_rx)",
            "V2BodyStore::open_with_policy(",
            ".into_quarantined_recovered_startup()",
            "SumeragiV2Adapter::open_recovered_startup_with_capacity_geometry(",
            ".bind_pending_kura_apply(pending_kura_apply)",
            ".authenticate_final_wal_startup_authority()?",
            "bind_production_lifecycle_owner_factory_inputs_v1(",
            "open_production_lifecycle_owner_v1(",
            "let launched = owner.launch(launch_inputs)?",
            "ProductionLifecyclePendingKuraRunnerActivationV1::mint_for_recovered_runner(",
            "launched.install_pending_kura_apply(&mut setup_runner)?",
            "pending.drive_apply_recovery_turn(&mut setup_runner)?",
            "reconcile_pending_lane_startup(",
            "pending.prepare_lane_recovery(",
            "prepared.activate_no_clock(activation)?",
            "run_pending_active_height(",
            "super::lifecycle_run_inner::run_non_pending_lifecycle_loop(",
        ),
    )
    reject_tokens(
        "pending_runner",
        pending_runner,
        "sealed pending-Kura lifecycle startup and ordinary successor handoff",
        (
            "arm_live_clocks(",
            "schedule_local_proposal(",
            "V2NposVrfLifecycle::new(",
            "V2BlockSyncDiscovery::new(",
            "drain_lifecycle_v2_ingress(",
            "step_pacemaker_once(",
        ),
    )
    pending_live = item("pending_runner", "run_pending_active_height")
    require_order(
        "pending_runner",
        pending_live,
        "restricted pending-Kura live recovery and finalization",
        (
            "settle_certified_serve_completion_for_no_clock_recovery(&mut active_runner)",
            "claim_producer_turn_for_no_clock_recovery(&mut active_runner)",
            "retry_exact_output_and_apply_sidecar_admissions(",
            "services.service_kura_replica_advert_refresh_turn(Instant::now())",
            "services.drain_completions(executor)?",
            "retry_exact_output_and_apply_sidecar_admissions(",
            "reconcile_executor_locked_body(executor, services)?",
            "drain_decided_lane_recovery_ingress(",
            "dispatch_lane_work_effects(lane_work, services, control_queue_capacity)?",
            "claimed.into_attempted(super::producer_turn_attempt_permit(&mut active_runner))",
            "settle_producer_turn_after_no_clock_recovery(&mut active_runner, attempted)",
            "activated.into_finalized_rollover(&mut active_runner)?",
            "finalized.finality()",
            "into_parts_with_lifecycle_storage_authority(",
            "finalized.rollover_outputs(",
            "post_output.retire_lifecycle_stores()?",
            "cleanup_ready.finish_cleanup(Duration::ZERO, cleanup_supervisor)",
        ),
    )
    require_order(
        "pending_runner",
        pending_live,
        "pending-Kura finalization must close ingress and finitely drain terminal recovery before consuming finalized rollover",
        (
            "let mut finalized_ingress_closed = false",
            "loop",
            "if !rollover_ready",
            "continue",
            "if !finalized_ingress_closed",
            "activated.close_runner_ingress_for_finalized_drain(&mut active_runner, receiver)?",
            "finalized_ingress_closed = true",
            "drain_decided_lane_recovery_ingress(",
            "dispatch_lane_work_effects(",
            "drained.is_some()",
            "if drained_terminal_ingress",
            "continue",
            "receiver.ensure_closed_drained_cut()",
            "activated.into_finalized_rollover(&mut active_runner)?",
        ),
    )
    _require_rust_token_sequence(
        paths["pending_runner"],
        pending_live,
        "let mut finalized_ingress_closed = false;",
        "pending-Kura finalization ingress close state must be initialized exactly once and never reopened",
        errors,
    )
    reject_tokens(
        "pending_runner",
        pending_live,
        "restricted pending-Kura live recovery and finalization",
        (
            "arm_live_clocks(",
            "schedule_local_proposal(",
            "drain_lifecycle_v2_ingress(",
            "step_pacemaker_once(",
        ),
    )
    pending_crash_hook = item("apply_tests", "fail_after_kura_store_for_test")
    require_tokens(
        "apply_tests",
        pending_crash_hook,
        "production pending-Kura Kura-first crash hook",
        (
            "self.test_failures.kura_store.store(true, std::sync::atomic::Ordering::Relaxed)",
        ),
    )
    pending_lane_fixture = item("lane_work", "pending_kura_lifecycle_fixture_for_test")
    require_order(
        "lane_work",
        pending_lane_fixture,
        "unactivated affine pending-Kura lane fixture",
        (
            "Self::new_with_output_guard_and_transport_for_test(",
            "None",
            "Some(expected)",
            "output_guard",
            "exact_output_handoff_owner",
        ),
    )
    reject_tokens(
        "lane_work",
        pending_lane_fixture,
        "unactivated affine pending-Kura lane fixture",
        (
            "activate_for_test_without_lane_drain_queue(",
            "activate_after_lane_drain_queue_install(",
        ),
    )
    pending_lifecycle_behavior = item(
        "startup_test", "exercise_pending_kura_production_lifecycle"
    )
    require_order(
        "startup_test",
        pending_lifecycle_behavior,
        "executable pending-Kura lifecycle shutdown and finalization",
        (
            "owner.launch(launch_inputs)",
            ".install_pending_kura_apply(&mut setup_runner)",
            "installed_status.pending_tip_recovery_stage",
            "Some(Stage::Apply)",
            "installed_status.pending_fetches, 0",
            "installed_status.pending_stores, 0",
            "installed_status.pending_validations, 0",
            "installed_status.pending_applications, 0",
            ".drive_apply_recovery_turn(&mut setup_runner)",
            "ProductionPendingKuraApplyRecoveryProgressV1::Completed",
            "recovery_stages.ends_with(&[Stage::ApplicationDispatched, Stage::Completed])",
            ".prepare_lane_recovery(",
            "pending_kura_lifecycle_fixture_for_test(",
            ".activate_no_clock(activation)",
            "executor.lifecycle_live_clocks_are_unarmed()",
            "if !finalize",
            ".into_clean_shutdown(&mut active_runner)",
            ".into_finalized_rollover(&mut active_runner)",
            ".rollover_outputs(&mut active_runner, lane_work, &successor, 64)",
            ".retire_lifecycle_stores()",
            "cleanup_ready.finish_cleanup(Duration::ZERO, &mut cleanup_supervisor)",
        ),
    )
    if pending_lifecycle_behavior is not None:
        unarmed_count = _token_sequence_count(
            rust_code_tokens(pending_lifecycle_behavior.source),
            rust_code_tokens("executor.lifecycle_live_clocks_are_unarmed()"),
        )
        if unarmed_count != 2:
            errors.append(
                f"{paths['startup_test']}:{pending_lifecycle_behavior.line}: executable "
                "pending-Kura lifecycle must assert unarmed clocks before and after "
                f"activation; found {unarmed_count} assertions"
            )
    pending_lifecycle_fixture = item(
        "startup_test",
        "production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies",
    )
    require_order(
        "startup_test",
        pending_lifecycle_fixture,
        "production Kura-first pending-Kura lifecycle fixture",
        (
            "(0xB5_u8, true, false, false, Some(false))",
            "(0xB6_u8, true, false, false, Some(true))",
            "semantic_probe.fail_after_kura_store_for_test()",
            "V2ApplyError::InjectedCrashAfterKuraStore",
            "drop(body_store)",
            ".into_quarantined_recovered_startup()",
            ".bind_pending_kura_apply(expected)",
            ".authenticate_final_wal_startup_authority()",
            ".open_production_lifecycle_owner_v1(",
            "exercise_pending_kura_production_lifecycle(",
        ),
    )
    pending_behavior = item(
        "wal_test",
        "recovered_decision_fetch_classifier_authenticates_exact_absent_manifest_and_sources",
    )
    require_order(
        "wal_test",
        pending_behavior,
        "pending-Kura bridge behavior",
        (
            "PendingKuraApply::for_test( context.id(), context.height, decision.subject.block_hash, )",
            ".bind_pending_kura_apply(expected_pending)",
            ".authenticate_final_wal_startup_authority()",
            "pending.retains_decision_fetch_for_test()",
            "pending.expected_for_test()",
            "mismatched_pending",
            "AdapterError::RecoveredPendingKuraApplyMismatch",
            "drop(pending)",
            "let Err(empty_error) = empty_pending",
            "let Err((error, retained)) = foreign_startup.bind_pending_kura_apply(foreign_pending)",
        ),
    )
    if pending_behavior is not None:
        for message in (
            "a same-height foreign Kura block must fail before owner launch",
            "pending Kura startup without a Decision Fetch must fail closed",
            "a foreign pending Kura height must not bind recovered startup",
        ):
            if message not in pending_behavior.source:
                errors.append(
                    f"{paths['wal_test']}:{pending_behavior.line}: pending-Kura bridge "
                    f"behavior omits exact fail-closed assertion {message!r}"
                )
