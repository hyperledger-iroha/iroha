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

    def require_item_order(
        source_name: str,
        item: RustItem | None,
        markers: tuple[str, ...],
        description: str,
    ) -> None:
        if item is None:
            return
        item_tokens = rust_code_tokens(item.source)
        cursor = 0
        for marker in markers:
            marker_tokens = rust_code_tokens(marker)
            position = next(
                (
                    index
                    for index in range(
                        cursor, len(item_tokens) - len(marker_tokens) + 1
                    )
                    if item_tokens[index : index + len(marker_tokens)]
                    == marker_tokens
                ),
                -1,
            )
            if position < 0:
                errors.append(
                    f"{paths[source_name]}:{item.line}: {description} must "
                    "preserve the exact reviewed production order"
                )
                return
            cursor = position + len(marker_tokens)

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
    for sequence, description in (
        (
            """
let next = Self {
    durable_view,
    protected_lock,
    ..self
};
if protected_lock.is_some_and(|lock| !next.protected_lock_is_well_formed(lock)) {
    return Err(
        "leader-wire recovery authority carried a future protected lock"
            .to_owned(),
    );
}
if !next.protected_lock_monotonically_extends(self) {
    return Err("leader-wire recovery authority regressed its protected lock".to_owned());
}
""",
            "leader-wire view authority must carry only a well-formed monotone protected lock",
        ),
    ):
        _require_rust_token_sequence(
            paths["store"], authority_advance, sequence, description, errors
        )

    protected_lock_monotonicity = _require_rust_item(
        paths["store"],
        sources["store"],
        "protected_lock_monotonically_extends",
        errors,
    )
    _require_rust_item_context(
        paths["store"],
        protected_lock_monotonicity,
        (("impl", "LeaderWireRecoveryAuthority"),),
        "monotone protected-lock authority",
        errors,
    )
    _require_rust_token_sequence(
        paths["store"],
        protected_lock_monotonicity,
        """
match (previous.protected_lock, self.protected_lock) {
    (None, _) => true,
    (Some(_), None) => false,
    (Some(previous), Some(next)) => next == previous || next.0.view > previous.0.view,
}
""",
        "protected-lock authority must permit only introduction, exact reuse, or a higher round",
        errors,
    )

    protected_lock_shape = _require_rust_item(
        paths["store"],
        sources["store"],
        "protected_lock_is_well_formed",
        errors,
    )
    _require_rust_token_sequence(
        paths["store"],
        protected_lock_shape,
        """
round.context_id == self.context_id
    && round.height == self.height
    && round.view <= self.durable_view
""",
        "protected-lock authority must retain exact context, height, and non-future view",
        errors,
    )

    protected_commit = _require_rust_item(
        paths["store"], sources["store"], "protects_commit_vote", errors
    )
    _require_rust_item_context(
        paths["store"],
        protected_commit,
        (("impl", "LeaderWireRecoveryAuthority"),),
        "exact historical protected-Commit classifier",
        errors,
    )
    _require_rust_token_sequence(
        paths["store"],
        protected_commit,
        """
identity.phase == FairV2IngressLeaderWirePhase::CommitVote
    && self.protected_lock.is_some_and(|(round, subject)| {
        identity.context_id == round.context_id
            && identity.height == round.height
            && identity.view == round.view
            && identity.subject_hash == Hash::new(subject.encode())
    })
""",
        "historical Commit-vote admission must exact-match phase, round, and subject",
        errors,
    )

    retire_identity = _require_rust_item(
        paths["store"], sources["store"], "retires_stored_identity", errors
    )
    _require_rust_token_sequence(
        paths["store"],
        retire_identity,
        """
self.decision_durable
    || (identity.view < self.durable_view
        && identity.phase != FairV2IngressLeaderWirePhase::CommitQc
        && !self.protects_commit_vote(identity))
""",
        "view cuts must retain only exact protected Commit votes and historical CommitQCs",
        errors,
    )

    admit_identity = _require_rust_item(
        paths["store"], sources["store"], "admits_ingress_identity", errors
    )
    _require_rust_token_sequence(
        paths["store"],
        admit_identity,
        """
if self.decision_durable {
    return false;
}
identity.phase == FairV2IngressLeaderWirePhase::CommitQc
    || self.protects_commit_vote(identity)
    || identity.view >= self.durable_view
""",
        "Decision must close control while pre-Decision cuts admit protected Commit progress",
        errors,
    )

    replayed_recovery_authority = require_context_item(
        "adapter",
        "leader_wire_recovery_authority",
        adapter_context,
        "replayed protected-lock recovery authority",
    )
    _require_rust_token_sequence(
        paths["adapter"],
        replayed_recovery_authority,
        """
let protected_lock = self
    .reducer
    .durable_state()
    .locked()
    .map(|certificate| -> Result<_, AdapterError> {
        Ok((
            self.registry
                .round_to_wire(certificate.proposal_round()),
            self.registry.subject(certificate.subject())?,
        ))
    })
    .transpose()?;
""",
        "startup recovery authority must derive the exact durable lock from replayed state",
        errors,
    )
    _require_rust_token_sequence(
        paths["adapter"],
        replayed_recovery_authority,
        """
.with_protected_lock(protected_lock)
.map_err(AdapterError::ServicedCandidateStore)
""",
        "startup recovery authority must fail closed while attaching the replayed lock",
        errors,
    )
    require_item_order(
        "adapter",
        replayed_recovery_authority,
        (
            ".durable_state()",
            ".locked()",
            ".round_to_wire(certificate.proposal_round())",
            ".transpose()?",
            "LeaderWireRecoveryAuthority::from_replayed_adapter(",
            ".with_protected_lock(protected_lock)",
            ".map_err(AdapterError::ServicedCandidateStore)",
        ),
        "startup recovery authority lock authentication and attachment",
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
    .advance_view(tag.view(), protected_lock)?;
self.leader_wire_ingress
    .advance_leader_wire_recovery_cut(next_recovery_authority)?;
self.leader_wire_recovery_authority = next_recovery_authority;
""",
        "certified EnterView must publish its protected-lock gate cut before exposing authority",
        errors,
    )

    executor_install_view = require_context_item(
        "effects",
        "install_view",
        executor_context,
        "validated EnterView protected-lock handoff",
    )
    _require_rust_token_sequence(
        paths["effects"],
        executor_install_view,
        """
protected.validate(&self.context).map_err(|error| {
    EffectExecutorError::Contract(format!(
        "EnterView protected lock is invalid: {error}"
    ))
})?;
if protected.phase != wire::GlobalPhase::Prepare
    || protected.proposal_round.context_id != self.context.id()
    || protected.proposal_round.height != self.context.height
    || protected.proposal_round.view >= tag.view()
""",
        "effect execution must validate the protected Prepare lock before projection",
        errors,
    )
    _require_rust_token_sequence(
        paths["effects"],
        executor_install_view,
        """
let protected_body = protected_lock_body(protected_lock.as_ref());
""",
        "effect execution must project the validated protected certificate to exact coordinates",
        errors,
    )
    _require_rust_token_sequence(
        paths["effects"],
        executor_install_view,
        """
services
    .entered_view(tag, certificate, protected_body)
    .map_err(service_error)?;
""",
        "effect execution must hand the validated protected lock to production services",
        errors,
    )
    require_item_order(
        "effects",
        executor_install_view,
        (
            "protected.validate(&self.context)",
            "let protected_body = protected_lock_body(protected_lock.as_ref());",
            "self.reconcile_protected_lock(tag, protected_body, highest_prepare_body, services)?;",
            ".reconcile_active_view_producer(tag, retain_local_producer)",
            ".entered_view(tag, certificate, protected_body)",
            "self.reconciled_tag = Some(tag);",
        ),
        "validated protected-lock reconciliation and installed-view exposure",
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
            "store",
            "leader_wire_protected_lock_cut_is_exact_monotone_and_decision_closed",
            "checks exact protected Commit admission and monotone Decision closure",
        ),
        (
            "adapter",
            "recovered_current_timeout_then_historical_commit_keeps_intrinsic_vote_round",
            "checks startup recovery authority preserves the replayed protected Commit round",
        ),
        (
            "ingress",
            "certified_view_cut_preserves_exact_locked_commit_and_historical_commit_qc",
            "checks the fair-ingress cut admits only exact historical Commit progress",
        ),
        (
            "worker",
            "entered_view_advances_live_leader_wire_recovery_cut",
            "rejects stale wire after live EnterView",
        ),
        (
            "worker",
            "entered_view_publishes_the_exact_protected_commit_vote_cut",
            "checks live EnterView publishes the exact protected Commit coordinates",
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

    protected_cut_regression = _require_rust_item(
        paths["store"],
        sources["store"],
        "leader_wire_protected_lock_cut_is_exact_monotone_and_decision_closed",
        errors,
    )
    for sequence, description in (
        (
            """
assert!(advanced.retires(&wrong_subject));
assert!(!advanced.admits_ingress_identity(&wrong_subject.identity));
""",
            "protected-lock regression must reject a wrong-subject Commit vote",
        ),
        (
            """
assert!(advanced.retires(&wrong_round));
assert!(!advanced.admits_ingress_identity(&wrong_round.identity));
""",
            "protected-lock regression must reject a wrong-round Commit vote",
        ),
        (
            """
assert!(advanced.retires(&wrong_phase));
assert!(!advanced.admits_ingress_identity(&wrong_phase.identity));
""",
            "protected-lock regression must reject a non-Commit phase",
        ),
        (
            """
assert!(!advanced.retires(&historical_commit_qc));
assert!(advanced.admits_ingress_identity(&historical_commit_qc.identity));
""",
            "protected-lock regression must retain historical CommitQC before Decision",
        ),
        (
            """
advanced.advance_view(6, None).is_err()
""",
            "protected-lock regression must reject lock loss",
        ),
        (
            """
.advance_view(6, Some((protected_round, conflicting_subject)))
    .is_err()
""",
            "protected-lock regression must reject a same-round conflicting subject",
        ),
        (
            """
.advance_view(6, Some((lower_round, protected_subject)))
    .is_err()
""",
            "protected-lock regression must reject lock regression",
        ),
        (
            """
let decision = advanced.with_durable_decision();
assert!(decision.retires(&protected_commit));
assert!(!decision.admits_ingress_identity(&protected_commit.identity));
assert!(decision.retires(&historical_commit_qc));
assert!(!decision.admits_ingress_identity(&historical_commit_qc.identity));
""",
            "protected-lock regression must make Decision dominant over both exceptions",
        ),
    ):
        _require_rust_token_sequence(
            paths["store"],
            protected_cut_regression,
            sequence,
            description,
            errors,
        )

    live_protected_cut_regression = _require_rust_item(
        paths["worker"],
        sources["worker"],
        "entered_view_publishes_the_exact_protected_commit_vote_cut",
        errors,
    )
    _require_rust_token_sequence(
        paths["worker"],
        live_protected_cut_regression,
        """
.entered_view(
    next,
    timeout_certificate_at_view(&service, initial.view()),
    Some((protected_round, protected_subject)),
)
""",
        "live EnterView regression must publish the exact protected-lock coordinates",
        errors,
    )
    _require_rust_token_sequence(
        paths["worker"],
        live_protected_cut_regression,
        """
Ok(super::super::FairV2IngressPushDisposition::Enqueued)
""",
        "live EnterView regression must enqueue the exact historical Commit vote",
        errors,
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
            "item.kind \\in AsyncControlKinds",
            "DeliveryView(item) < nodeView[item.envelope.recipient]",
            'item.kind # "CommitQC"',
            "~HistoricalLockedCommitItem(item)",
            "NodeHasDecision(item.envelope.recipient)",
        ),
        "AsyncLeaderWireAtomicAdmissionAllows": (
            "~AsyncLeaderWireRecoveryCutObsoletesItem(item)",
        ),
        "AsyncControlIngressStageRetired": (
            "item.kind \\in AsyncControlKinds",
            "NodeHasDecision(item.envelope.recipient)",
            "AsyncControlServiceConsumed(item)",
            'item.kind # "CommitQC"',
            "~HistoricalLockedCommitItem(item)",
            "AsyncControlServiceIdentityServicedOrAdvanced(item)",
        ),
        "AsyncLeaderWireDrainDeterministicallyRetired": (
            "record.view < nodeView[record.recipient]",
            'record.item.kind # "CommitQC"',
            "~HistoricalLockedCommitItem(record.item)",
            "NodeHasDecision(record.recipient)",
            "AsyncCandidateStageRetired(item)",
            "AsyncControlServiceOccurrenceRetired(item)",
        ),
        "AsyncLeaderWireLifecycleDurableControlServiceReceipt": (
            "AsyncControlIngressStageRetired(record.item)",
        ),
        "AsyncLeaderWireLifecycleDurableCoreRetirement": (
            "record.view < nodeView[record.recipient]",
            'record.item.kind # "CommitQC"',
            "~HistoricalLockedCommitItem(record.item)",
            "NodeHasDecision(record.recipient)",
        ),
        "AsyncLeaderWireLifecycleStaleOrDecision": (
            "record.item.kind \\in AsyncControlKinds",
            "record.view < nodeView[record.recipient]",
            'record.item.kind # "CommitQC"',
            "~HistoricalLockedCommitItem(record.item)",
            "NodeHasDecision(record.recipient)",
        ),
        "AsyncLeaderWireLifecycleRecoveryCutObsolete": (
            "AsyncLeaderWireLifecycleDormant(record)",
            "record.item.kind \\in AsyncControlKinds",
            "record.view < nodeView[record.recipient]",
            'record.item.kind # "CommitQC"',
            "~HistoricalLockedCommitItem(record.item)",
            "NodeHasDecision(record.recipient)",
        ),
        "AsyncLeaderWireLifecycleConsumerTerminal": (
            "AsyncControlIngressStageRetired(record.item)",
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


def _successor_recovery_pending_kura_tail_source_fidelity_errors(
    paths,
    sources,
    errors: list[str],
    item,
    require_order,
    reject_tokens,
    require_tokens,
) -> None:
    pending_source = sources["pending_kura"]
    pending_replay_types_start = pending_source.find(
        "pub(crate) struct PendingKuraRecoveredAdapterStartupV1"
    )
    pending_replay_types_end = pending_source.find(
        "/// Move-only exact validation marker withheld from ordinary reducer replay.",
        pending_replay_types_start,
    )
    pending_replay_types = (
        pending_source[pending_replay_types_start:pending_replay_types_end]
        if pending_replay_types_start >= 0
        and pending_replay_types_end > pending_replay_types_start
        else ""
    )
    pending_replay_type_tokens = rust_code_tokens(pending_replay_types)
    for required in (
        "startup: RecoveredAdapterStartup",
        "expected: crate::sumeragi::v2_recovery::PendingKuraApply",
        "startup: AuthenticatedRecoveredAdapterStartup",
        "replay: RecoveredPendingKuraApplyReplayV1",
        "pub(in crate::sumeragi) struct RecoveredPendingKuraApplyReplayV1",
        "fetch: RecoveredWalDecisionFetch",
        "pub(in crate::sumeragi) struct PreparedRecoveredPendingKuraApplyReplayV1",
        "wal_identity: RecoveredWalFrameIdentity",
        "replay_evidence: RecoveredWalDecisionFetchReplayEvidenceV1",
        "effect: AdapterEffect",
        "verified: VerifiedHeightContext",
        "deferred_validated_marker: Option<DeferredPendingKuraValidatedMarkerV1>",
    ):
        if not _token_sequence_count(
            pending_replay_type_tokens, rust_code_tokens(required)
        ):
            errors.append(
                f"{paths['pending_kura']}: opaque pending-Kura replay types "
                f"omit {required!r}"
            )
    for forbidden in (
        "#[derive(Clone)]",
        "derive(Copy)",
        "pub startup:",
        "pub expected:",
        "pub replay:",
        "pub fetch:",
        "pub verified:",
        "pub wal_identity:",
        "pub replay_evidence:",
        "pub effect:",
        "pub genesis:",
        "fn into_parts(",
        "fn effect(",
        "fn fetch(",
    ):
        if forbidden in pending_replay_types:
            errors.append(
                f"{paths['pending_kura']}: opaque pending-Kura replay types "
                f"expose forbidden surface {forbidden!r}"
            )

    pending_marker_types_start = pending_source.find(
        "pub(crate) struct DeferredPendingKuraValidatedMarkerV1"
    )
    pending_marker_types_end = pending_source.find(
        "impl InstalledPendingKuraApplyV1", pending_marker_types_start
    )
    pending_marker_types = (
        pending_source[pending_marker_types_start:pending_marker_types_end]
        if pending_marker_types_start >= 0
        and pending_marker_types_end > pending_marker_types_start
        else ""
    )
    pending_marker_type_tokens = rust_code_tokens(pending_marker_types)
    for required in (
        "pub(crate) struct DeferredPendingKuraValidatedMarkerV1",
        "pub(in crate::sumeragi) struct PreparedPendingKuraValidatedApplyV1<'a>",
        "prepared: super::PreparedDirectValidationSucceededApply<'a>",
        "child_ownership: crate::sumeragi::v2_runtime::RuntimeEffectOwnership",
        "_marker: DeferredPendingKuraValidatedMarkerV1",
        "pub(crate) struct PendingKuraValidatedApplySuccessorV1",
        "effect: AdapterEffect",
        "ownership: crate::sumeragi::v2_runtime::RuntimeEffectOwnership",
        "pub(in crate::sumeragi) struct InstalledPendingKuraApplyV1",
        "genesis: Option<crate::sumeragi::v2_effects::VerifiedPendingGenesisNexusAmxContext>",
    ):
        if not _token_sequence_count(
            pending_marker_type_tokens, rust_code_tokens(required)
        ):
            errors.append(
                f"{paths['pending_kura']}: move-only pending-Kura marker/child "
                f"types omit {required!r}"
            )
    for forbidden in (
        "fn into_parts(",
        "pub effect:",
        "pub(crate) effect:",
        "pub(in crate::sumeragi) effect:",
        "pub(crate) child_ownership:",
        "pub(in crate::sumeragi) child_ownership:",
        "pub(crate) _marker:",
        "pub(in crate::sumeragi) _marker:",
    ):
        if _token_sequence_count(
            pending_marker_type_tokens, rust_code_tokens(forbidden)
        ):
            errors.append(
                f"{paths['pending_kura']}: move-only pending-Kura marker/child "
                f"types expose forbidden surface {forbidden!r}"
            )
    bind_pending = item("pending_kura", "bind_pending_kura_apply")
    require_order(
        "pending_kura",
        bind_pending,
        "pending-Kura startup context binding",
        (
            "expected.context_id() != self.adapter.wire_context.id()",
            "expected.height() != self.adapter.wire_context.height",
            "Err((AdapterError::RecoveredPendingKuraApplyMismatch, self))",
            "Ok(PendingKuraRecoveredAdapterStartupV1 { startup: self, expected, })",
        ),
    )
    pending_auth = item(
        "pending_kura", "authenticate_final_wal_startup_authority"
    )
    require_order(
        "pending_kura",
        pending_auth,
        "pending-Kura Decision-Fetch ownership transfer into storage-only startup",
        (
            "startup.authenticate_final_wal_startup_authority()",
            "let RecoveredWalStartupAuthorityV1::DecisionFetch(fetch) = authority",
            "if !effects.is_empty()",
            "AdapterEffect::FetchBody { subject, .. } if subject.block_hash == expected.block_hash()",
            "authority: RecoveredWalStartupAuthorityV1::None",
            "replay: RecoveredPendingKuraApplyReplayV1 { expected, fetch }",
        ),
    )
    pending_runtime = item("pending_kura", "into_serialized_runtime")
    require_order(
        "pending_kura",
        pending_runtime,
        "pending-Kura exact Fetch ownership roundtrip through runtime startup",
        (
            "let pending = pending_kura_apply",
            "let RecoveredPendingKuraApplyReplayV1 { expected, fetch } = replay",
            "let RecoveredWalDecisionFetch { wal_identity, replay_evidence, effect, } = fetch",
            "replay_evidence.exactly_matches_recovered_decision_fetch",
            "Ok((expected, verified, wal_identity, replay_evidence, effect))",
            ".transpose()?",
            "let (startup_effects, pending) = match pending",
            "vec![effect]",
            "SerializedV2Runtime::new_with_lifecycle_ordinals(",
            "adapter, startup_effects,",
            "returned_effects.len() == 1",
            "returned_effects.pop()",
            "PreparedRecoveredPendingKuraApplyReplayV1",
            "deferred_validated_marker: None",
            "Ok((runtime, replay, local_proposal_attempt))",
        ),
    )
    pending_attach = item("pending_kura", "with_pending_kura_apply_replay")
    require_order(
        "pending_kura",
        pending_attach,
        "pending-Kura pristine storage-only startup attachment",
        (
            "ProductionLifecycleAdapterStartupStateV1::Recovered {",
            "leader_wire_launch_prepared: false",
            "if effects.is_empty() && pending_kura_apply.is_none()",
            "*pending_kura_apply = Some(replay)",
            "ProductionLifecycleAdapterStartupStateV1::Recovered { .. }",
            "panic!(",
        ),
    )

    pending_marker_defer = item(
        "pending_kura", "classify_and_defer_validated_marker"
    )
    require_order(
        "pending_kura",
        pending_marker_defer,
        "pending-Kura validated-marker deferral",
        (
            "replay_evidence.exactly_matches_recovered_decision_fetch(",
            "if key != expected_key",
            "if self.deferred_validated_marker.is_some()",
            "self.expected.context_id() != context.id()",
            "self.expected.height() != context.height",
            "self.expected.block_hash() != subject.block_hash",
            "certificate.phase != crate::sumeragi::v2::wire::GlobalPhase::Commit",
            "certificate.proposal_round != *round",
            "certificate.subject != *subject",
            "certificate.validate(context).is_err()",
            "manifest.validate(context).is_err()",
            "manifest.round != *round",
            "manifest.subject != *subject",
            "advertised_manifest.as_ref()",
            "durable.context_id() != context.id()",
            "durable.round() != *round",
            "durable.subject() != *subject",
            "durable.manifest_hash() != iroha_crypto::HashOf::new(manifest)",
            "validated.durable() != durable",
            "validated.execution_commitment() != certificate.execution_commitment",
            "self.deferred_validated_marker = Some(DeferredPendingKuraValidatedMarkerV1",
        ),
    )
    pending_marker_exact = item("pending_kura", "exactly_matches_recovery")
    require_order(
        "pending_kura",
        pending_marker_exact,
        "pending-Kura exact deferred marker",
        (
            "self.tag == replay_tag",
            "self.manifest_hash == iroha_crypto::HashOf::new(manifest)",
            "self.validated.durable() == &self.durable",
            "self.certificate.proposal_round == self.round",
            "self.certificate.subject == self.subject",
            "self.certificate.execution_commitment == self.validated.execution_commitment()",
            "self.certificate.validate(context).is_ok()",
        ),
    )
    pending_marker_prepare = item("pending_kura", "prepare_apply")
    require_order(
        "pending_kura",
        pending_marker_prepare,
        "pending-Kura marker-owned direct Validate-to-Apply preview",
        (
            "let AdapterEffect::ValidateBody",
            "if *tag != self.tag || *round != self.round || *subject != self.subject",
            "ownership.binds_durable_decision_authority(",
            "self.certificate.round",
            "self.certificate.proposal_round",
            "self.subject",
            "self.certificate.execution_commitment",
            "ownership.exact_pending_adapter_effect_binding(predecessor)",
            "adapter.prepare_direct_validation_succeeded(",
            "DirectValidationSucceededPreparation::Apply(prepared)",
            "validate_pending.project_validate_apply_successor(predecessor, &apply_effect)",
            "ownership.rebind_as_inherited_adapter_effect(&apply_effect)",
            "PreparedPendingKuraValidatedApplyV1",
        ),
    )
    reject_tokens(
        "pending_kura",
        pending_marker_prepare,
        "pending-Kura marker-owned direct Validate-to-Apply preview",
        (
            "reducer::Event::ValidationCompleted",
            "periodic_timer",
        ),
    )
    pending_marker_commit = item("pending_kura", "commit")
    require_order(
        "pending_kura",
        pending_marker_commit,
        "pending-Kura deferred validation commit",
        (
            "let super::PreparedDirectValidationSucceededApply",
            "adapter.reducer = next_reducer",
            "adapter.registry = next_registry",
            "adapter.reducer_fence_generation = next_fence_generation",
            "PendingKuraValidatedApplySuccessorV1 { effect: apply_effect, ownership: child_ownership, }",
        ),
    )
    pending_child_release = item("pending_kura", "consume_for_executor")
    require_order(
        "pending_kura",
        pending_child_release,
        "pending-Kura executor-only Apply child release",
        (
            "PendingKuraApplySuccessorExecutorPermitV1",
            "(self.effect, self.ownership)",
        ),
    )

    pending_install = item("pending_kura", "install")
    require_order(
        "pending_kura",
        pending_install,
        "pending-Kura marker-verified direct pipeline install",
        (
            "let Some(deferred_validated_marker) = deferred_validated_marker",
            "executor.context() != verified.context()",
            "replay_evidence.exactly_matches_recovered_decision_fetch",
            "let effects = vec![effect]",
            "executor.verify_pending_kura_apply_replay( expected, &effects, deferred_validated_marker, )?",
            "executor.consume_pending_tip_recovery_effects(effects, services)?",
            "Ok(InstalledPendingKuraApplyV1 { expected, genesis })",
        ),
    )
    reject_tokens(
        "pending_kura",
        pending_install,
        "pending-Kura marker-aware verification-before-dispatch install",
        ("executor.verify_pending_kura_apply_replay_unchecked(",),
    )
    adapter_source = sources["adapter"]
    decision_fetch_start = adapter_source.find(
        "pub(crate) struct RecoveredWalDecisionFetch"
    )
    decision_fetch_end = adapter_source.find(
        "impl RecoveredWalControlSign", decision_fetch_start
    )
    decision_fetch_declaration = (
        adapter_source[decision_fetch_start:decision_fetch_end]
        if decision_fetch_start >= 0 and decision_fetch_end > decision_fetch_start
        else ""
    )
    decision_fetch_tokens = rust_code_tokens(decision_fetch_declaration)
    for required in (
        "pub(crate) struct RecoveredWalDecisionFetch",
        "wal_identity: RecoveredWalFrameIdentity",
        "replay_evidence: RecoveredWalDecisionFetchReplayEvidenceV1",
        "effect: AdapterEffect",
    ):
        if not _token_sequence_count(
            decision_fetch_tokens, rust_code_tokens(required)
        ):
            errors.append(
                f"{paths['adapter']}: move-only recovered Decision Fetch "
                f"declaration omits {required!r}"
            )
    for forbidden in (
        "derive(Clone)",
        "derive(Copy)",
        "pub wal_identity:",
        "pub replay_evidence:",
        "pub effect:",
    ):
        if _token_sequence_count(
            decision_fetch_tokens, rust_code_tokens(forbidden)
        ):
            errors.append(
                f"{paths['adapter']}: move-only recovered Decision Fetch "
                f"exposes forbidden surface {forbidden!r}"
            )

    pending_owner = item("coordinator_support", "with_pending_kura_apply_replay")
    require_order(
        "coordinator_support",
        pending_owner,
        "storage-only pending-Kura replay attachment",
        (
            "self.adapter_startup.take()",
            ".expect(",
            "self.adapter_startup = Some(startup.with_pending_kura_apply_replay(replay))",
            "self",
        ),
    )
    reject_tokens(
        "coordinator_support",
        pending_owner,
        "storage-only pending-Kura replay attachment",
        (
            "RecoveredPendingKuraApplyCarrierPermitV1",
            "bind_recovered_apply_carrier(",
        ),
    )
    pending_factory = item(
        "pending_kura", "open_production_lifecycle_owner_v1"
    )
    require_order(
        "pending_kura",
        pending_factory,
        "recovered Decision-Apply pending-Kura owner factory",
        (
            "startup.open_production_lifecycle_owner_v1(",
            "owner.with_pending_kura_apply_replay(replay)",
        ),
    )
    launched_pending_install = item("pending_lifecycle", "install_pending_kura_apply")
    require_order(
        "pending_lifecycle",
        launched_pending_install,
        "fail-stop pending-Kura preactivation install",
        (
            "self.pending_kura_apply_replay.take()",
            "self.services.lifecycle_output_guard()",
            "super::preactivation::missing_pending_kura_replay(output_guard.as_ref(),)",
            "self.with_runner_setup(runner",
            "replay.install(executor, services)",
            "PendingKuraProductionLifecycleV1 { installed, launched: self, }",
        ),
    )
    pending_apply_startup = item(
        "effects", "verify_pending_kura_apply_replay"
    )
    require_order(
        "effects",
        pending_apply_startup,
        "pending-Kura replay verification with deferred marker",
        (
            "self.ensure_open()?",
            "self.pending_tip_recovery.is_some()",
            "let decision = self.runtime.replayed_decision_key()",
            "let [ AdapterEffect::FetchBody",
            "let owner_tag = self.current_tag()",
            "if *tag != owner_tag",
            "let expected_sources = self.frozen_archive_sources()",
            "self.runtime.verify_certificate(&self.context, certificate)",
            "let (genesis_context, evidence) = verify_pending_kura_apply_parts_with_marker(",
            "deferred_validated_marker",
            "if evidence.durable_round() != *round",
            "self.pending_tip_recovery = Some(evidence)",
        ),
    )
    evidence_source = sources["effects"]
    evidence_start = evidence_source.find(
        "pub(crate) struct PendingKuraApplyRecoveryEvidence"
    )
    evidence_end = evidence_source.find(
        "impl PendingKuraApplyRecoveryEvidence", evidence_start
    )
    evidence_declaration = (
        evidence_source[evidence_start:evidence_end]
        if evidence_start >= 0 and evidence_end > evidence_start
        else ""
    )
    evidence_tokens = rust_code_tokens(evidence_declaration)
    for required in (
        "pub(crate) struct PendingKuraApplyRecoveryEvidence",
        "deferred_validated_marker: Option<super::v2::DeferredPendingKuraValidatedMarkerV1>",
        "stage: PendingKuraApplyRecoveryStage",
    ):
        if not _token_sequence_count(evidence_tokens, rust_code_tokens(required)):
            errors.append(
                f"{paths['effects']}: direct pending-Kura evidence omits {required!r}"
            )

    evidence_impl_end = evidence_source.find(
        "/// Explicit bounds for outstanding effect work", evidence_end
    )
    evidence_impl = (
        evidence_source[evidence_end:evidence_impl_end]
        if evidence_end >= 0 and evidence_impl_end > evidence_end
        else ""
    )
    evidence_impl_tokens = rust_code_tokens(evidence_impl)
    for required in (
        "PendingKuraApplyRecoveryStage::CertifiedFetch | PendingKuraApplyRecoveryStage::DurableStore | PendingKuraApplyRecoveryStage::DeterministicValidation",
        "marker.exactly_matches_recovery(",
        "PendingKuraApplyRecoveryStage::Apply | PendingKuraApplyRecoveryStage::ApplicationDispatched | PendingKuraApplyRecoveryStage::Completed",
        "self.deferred_validated_marker.is_none()",
        "self.deferred_validated_marker.take()",
        "self.deferred_validated_marker = Some(marker)",
    ):
        if not _token_sequence_count(evidence_impl_tokens, rust_code_tokens(required)):
            errors.append(
                f"{paths['effects']}: pending-Kura evidence marker lifecycle "
                f"omits {required!r}"
            )

    pending_runtime_prepare = item(
        "runtime", "prepare_pending_kura_validated_apply"
    )
    require_order(
        "runtime",
        pending_runtime_prepare,
        "pending-Kura no-clock marker preparation",
        (
            "self.fail_closed",
            "self.clocks_armed",
            "self.ingress.len() != 0",
            "self.pending_effect_ownership.is_some()",
            "self.last_scheduler_ownership.is_some()",
            "self.pending_leader_wire_terminals.is_empty()",
            "marker.prepare_apply(&mut self.driver, predecessor, ownership)",
        ),
    )

    default_runtime_commit_candidates = tuple(
        rust_item
        for rust_item in rust_items(
            sources["effects"], "commit_pending_kura_validated_apply"
        )
        if rust_item.brace_context
        == (("pub", "(", "crate", ")", "trait", "EffectRuntime"),)
    )
    if len(default_runtime_commit_candidates) != 1:
        errors.append(
            f"{paths['effects']}: pending-Kura marker commit must retain one "
            "EffectRuntime default; found "
            f"{len(default_runtime_commit_candidates)}"
        )
        default_runtime_commit = None
    else:
        default_runtime_commit = default_runtime_commit_candidates[0]
    require_order(
        "effects",
        default_runtime_commit,
        "generic runtime pending-Kura marker fail-closed default",
        (
            "Err((",
            "marker",
            '"runtime cannot commit a deferred pending-Kura validation marker"',
        ),
    )

    serialized_runtime_commit_candidates = tuple(
        rust_item
        for rust_item in rust_items(
            sources["effects"], "commit_pending_kura_validated_apply"
        )
        if rust_item.brace_context
        == (("impl", "EffectRuntime", "for", "SerializedV2Runtime"),)
    )
    if len(serialized_runtime_commit_candidates) != 1:
        errors.append(
            f"{paths['effects']}: pending-Kura marker commit must retain one "
            "SerializedV2Runtime implementation; found "
            f"{len(serialized_runtime_commit_candidates)}"
        )
        serialized_runtime_commit = None
    else:
        serialized_runtime_commit = serialized_runtime_commit_candidates[0]
    require_order(
        "effects",
        serialized_runtime_commit,
        "serialized pending-Kura marker commit",
        (
            "self.prepare_pending_kura_validated_apply(marker, predecessor, ownership)",
            "Ok(prepared) => Ok(prepared.commit())",
            "Err((marker, error)) => Err((marker, error.to_string()))",
        ),
    )

    pending_validate_child = item("effects", "validate_body")
    require_order(
        "effects",
        pending_validate_child,
        "pending-Kura Validate exact Apply child",
        (
            "recovery.stage() != PendingKuraApplyRecoveryStage::DeterministicValidation",
            "recovery.replay_tag() != tag",
            "recovery.durable_round() != round",
            "recovery.durable_subject() != subject",
            "recovery.durable_receipt() != &receipt",
            "self.ensure_pending_slot()?",
            "let _next_apply_work = self.plan_work_id()?",
            "take_deferred_validated_marker()?",
            "commit_pending_kura_validated_apply(marker, &effect, &ownership)",
            "restore_deferred_validated_marker(marker)",
            "return Ok(Some(successor))",
        ),
    )

    pending_consume_child = item("effects", "consume_one")
    require_order(
        "effects",
        pending_consume_child,
        "pending-Kura stage-before-child dispatch",
        (
            "result?",
            "if let Some(stage) = recovery_transition",
            ".stage = stage",
            "if let Some(successor) = pending_kura_successor",
            "successor.consume_for_executor(",
            "PendingKuraApplySuccessorExecutorPermitV1::new()",
            "self.ensure_pending_tip_recovery_effect_is_local(&effect)",
            "self.consume_one(effect, ownership, None, services)",
            "EffectExecutorError::Contract(format!(",
        ),
    )
    reject_tokens(
        "effects",
        pending_consume_child,
        "pending-Kura stage-before-child dispatch",
        ("periodic_timer",),
    )

    pending_recovery_step = item("effects", "step_pending_tip_recovery")
    require_order(
        "effects",
        pending_recovery_step,
        "pending-Kura exact local stage consumer",
        (
            "self.finish_runtime_step_reconciliation(services)",
            "RuntimeStep::Idle",
            "RuntimeStep::Advanced(effects)",
            "self.consume_pending_tip_recovery_effects(effects, services)?",
            "PendingTipRecoveryAttemptResult::Advanced",
            "self.publish_status(services)",
            "EffectExecutorStep::Advanced { effects: count }",
        ),
    )
    reject_tokens(
        "effects",
        pending_recovery_step,
        "stage-complete direct-marker pending-tip recovery step",
        (
            "self.consume_effects(effects, services)?",
            "let stage = PendingKuraApplyRecoveryStage::Apply",
            "stage != PendingKuraApplyRecoveryStage::Apply",
        ),
    )
    apply_runtime_readiness = item(
        "runtime", "lifecycle_decision_apply_dispatch_available"
    )
    require_order(
        "runtime",
        apply_runtime_readiness,
        "lifecycle Apply runtime mutation-frontier readiness",
        (
            "!self.fail_closed",
            "self.pending_effect_ownership.is_none()",
            "self.last_scheduler_ownership.is_none()",
            "self.pending_leader_wire_terminals.is_empty()",
        ),
    )
    apply_dispatch_readiness = item(
        "effects", "lifecycle_decision_apply_dispatch_available"
    )
    require_order(
        "effects",
        apply_dispatch_readiness,
        "lifecycle Apply dispatch quiescence gate",
        (
            "self.ensure_open()?",
            "let successor_debt_is_exact = match successor_outputs",
            "self.pending_lifecycle_output_admissions.is_empty()",
            "attestation.pending_count() == self.pending_lifecycle_output_admissions.len()",
            "attestation.exactly_matches_pending_keys(",
            "attestation.exactly_matches_retransmit_apply(&owned.effect)",
            "pending_output.exactly_precedes_periodic_retransmit_apply(",
            "self.pending_work() == self.pending_lifecycle_output_admissions.len()",
            "successor_debt_is_exact",
            "self.recovered_decision_fetch_request_index_is_exact_and_empty()",
            "self.parked_effect_batch.is_none()",
            "self.finality_completion.is_none()",
            "self.runtime.queued_commands() == 0",
            "self.runtime.lifecycle_decision_apply_dispatch_available()",
        ),
    )
    pending_apply_registry_projection = item(
        "registry", "exactly_matches_pending_kura_recovery"
    )
    require_order(
        "registry",
        pending_apply_registry_projection,
        "exact pending-Kura lifecycle Apply registry projection",
        (
            "self.key.matches_height_context(context)",
            "self.task.as_ref().is_some_and(|task|",
            "task.dispatch_key() == self.key",
            "task.exact_tag() == tag",
            "task.subject() == subject",
            "task.certificate() == certificate",
            "task.validated_receipt() == validated_receipt",
        ),
    )
    pending_apply_executor_dispatch = item(
        "effects", "prepare_lifecycle_decision_apply_executor_dispatch"
    )
    require_order(
        "effects",
        pending_apply_executor_dispatch,
        "exact pending-Kura lifecycle Apply executor dispatch preflight",
        (
            "self.ensure_open()?",
            "successor_outputs.as_ref().is_some_and(|attestation| attestation.dispatch_key() != prepared.dispatch_key())",
            "self.lifecycle_decision_apply_dispatch_available(successor_outputs.as_ref())?",
            "PendingLifecycleDecisionApplySuccessorOutputsTransitionV1",
            "installed: &mut self.lifecycle_decision_apply_successor_outputs",
            "retained_effect_batch: &mut self.retained_effect_batch",
            "if successor_outputs.is_some()",
            "evidence.stage() == PendingKuraApplyRecoveryStage::Apply",
            "evidence.is_exact(&self.context)",
            "evidence.replay_tag() == self.current_tag()",
            "prepared.exactly_matches_pending_kura_recovery(",
            "PendingKuraApplyDispatchTransitionV1 { evidence, last_result:",
        ),
    )
    pending_apply_scheduler = item(
        "scheduler", "dispatch_completion_with_runner_debt_and_required_ordinal"
    )
    require_order(
        "scheduler",
        pending_apply_scheduler,
        "lifecycle Apply executor-readiness capacity probe",
        (
            "LifecycleWorkClass::Apply",
            "let executor_available = executor",
            ".lifecycle_decision_apply_dispatch_available(",
            "live_apply_successor_outputs.get(ordinal)",
            "LifecycleCompletionCapacityProbeV1::Apply",
            "executor_available",
            "capture_lifecycle_completion_capacity_census(probes)",
        ),
    )
    require_order(
        "scheduler",
        pending_apply_scheduler,
        "required Ready ordinal scheduler selection",
        (
            "let plan = match required_ordinal",
            "Some(ordinal) => self.coordinator.plan_turn_requiring_ordinal(inputs, ordinal)",
            "None => self.coordinator.plan_turn(inputs)",
            "let lease = match plan",
        ),
    )
    require_order(
        "scheduler",
        pending_apply_scheduler,
        "joined lifecycle Apply scheduler and worker publication",
        (
            "LifecycleWorkClass::Apply",
            "prepare_lifecycle_decision_apply_dispatch(&self.coordinator, &lease)",
            "reservation.preflight(&prepared)",
            "let successor_outputs = live_apply_successor_outputs.remove(&ordinal)",
            "executor.prepare_lifecycle_decision_apply_executor_dispatch(",
            "&prepared, successor_outputs",
            "reservation.commit(prepared, executor_dispatch)",
            "ProductionCompletionDispatchV1::ApplyQueued { ordinal }",
        ),
    )
    apply_capacity_census = item(
        "worker_services", "capture_lifecycle_completion_capacity_census"
    )
    require_order(
        "worker_services",
        apply_capacity_census,
        "lifecycle Apply executor/worker capacity conjunction",
        (
            "LifecycleCompletionCapacityProbeV1::Apply { ordinal, key, executor_available, }",
            "LifecycleCompletionPreparedCapacityV1::Apply { key, available: executor_available, }",
            "LifecycleCompletionPreparedCapacityV1::Apply { key, available }",
            "*available = *available && io.command_tx.queue.lifecycle_completion_worker_capacity(state)",
        ),
    )
    pending_worker_commit_context = (
        (
            "impl",
            "LifecycleDecisionApplyCapacityReservationV1",
            "<",
            "'",
            "_",
            ">",
        ),
    )
    pending_worker_commits = tuple(
        rust_item
        for rust_item in rust_items(sources["worker"], "commit")
        if rust_item.brace_context == pending_worker_commit_context
    )
    if len(pending_worker_commits) != 1:
        errors.append(
            f"{paths['worker']}: lifecycle Apply worker publication must "
            "retain exactly one qualified reservation commit; found "
            f"{len(pending_worker_commits)}"
        )
        pending_worker_commit = None
    else:
        pending_worker_commit = pending_worker_commits[0]
    require_order(
        "worker",
        pending_worker_commit,
        "lifecycle Apply worker publication before stage advance",
        (
            "let task = prepared.commit_for_worker()",
            "state.lifecycle_decision_applies.insert(",
            ".push_back(V2IoCommand::LifecycleDecisionApply(task))",
            "executor_dispatch.commit_after_worker_dispatch()",
            "drop(state)",
            "self.queue.ready.notify_all()",
            "operation.complete()",
        ),
    )
    pending_dispatch_commit = item("effects", "commit_after_worker_dispatch")
    require_order(
        "effects",
        pending_dispatch_commit,
        "physical lifecycle Apply publication advances pending stage once",
        (
            "if let Some(successor_outputs) = self.successor_outputs",
            "successor_outputs.retained_effect_batch.take()",
            "successor_outputs.attestation.exactly_matches_retransmit_apply(&owned.effect)",
            "*successor_outputs.installed = Some(successor_outputs.attestation)",
            "if let Some(pending) = self.pending",
            "PendingKuraApplyRecoveryStage::Apply",
            "pending.evidence.stage = PendingKuraApplyRecoveryStage::ApplicationDispatched",
        ),
    )
    pending_apply_turn = item("pending_lifecycle", "drive_apply_recovery_turn")
    _require_rust_item_context(
        paths["pending_lifecycle"],
        pending_apply_turn,
        (("impl", "PendingKuraProductionLifecycleV1"),),
        "unconditional lifecycle-owned pending-Kura Apply recovery turn",
        errors,
        expected_attributes=("#[allow(dead_code, clippy::result_large_err)]",),
    )
    require_order(
        "pending_lifecycle",
        pending_apply_turn,
        "bounded closed-ingress pending-Kura direct-pipeline turn",
        (
            "self.launched.pending_kura_apply_replay.is_some()",
            "close_admission_for_restart()",
            "self.launched.with_runner_setup(runner, |executor, services|",
            "executor.pending_kura_apply_recovery_evidence()",
            "PendingKuraApplyRecoveryStage::Completed",
            "executor.ready_to_finish()",
            "let completions = services.drain_completions(executor)?",
            "for _ in 0..limit.max(1)",
            "executor.step_pending_tip_recovery(Instant::now(), services)?",
            "let evidence = executor.pending_kura_apply_recovery_evidence()",
            "let attempts = executor.pending_tip_recovery_attempts()",
            "if completions == 0 && effects == 0",
            "ProductionPendingKuraApplyRecoveryProgressV1::Waiting",
            "ProductionPendingKuraApplyRecoveryProgressV1::Advanced",
        ),
    )
    reject_tokens(
        "pending_lifecycle",
        pending_apply_turn,
        "closed-ingress pending-Kura direct-pipeline turn",
        (
            "arm_live_clocks(",
            "schedule_local_proposal(",
            "drive_ingress_turn(",
            "drain_effects(limit)",
        ),
    )
    pending_apply_finality = item(
        "effects", "commit_lifecycle_decision_apply_finality"
    )
    require_order(
        "effects",
        pending_apply_finality,
        "exact pending-Kura lifecycle Apply reaches Completed after finality ownership",
        (
            "finality.consume_for_executor(",
            "evidence.stage() == PendingKuraApplyRecoveryStage::ApplicationDispatched",
            "evidence.is_exact(&self.context)",
            "tag == evidence.replay_tag()",
            "artifact.subject == evidence.commit_subject()",
            "receipt.block_hash() == evidence.commit_subject().block_hash",
            "self.finality_completion = Some(FinalityCompletion",
            "FinalityCompletionOwner::LifecycleDecisionApply(dispatch_key)",
            "evidence.stage = PendingKuraApplyRecoveryStage::Completed",
        ),
    )
    pending_lane = item("pending_lifecycle", "prepare_lane_recovery")
    require_order(
        "pending_lifecycle",
        pending_lane,
        "affine pending-Kura lane preparation",
        (
            "let expected = self.installed.expected()",
            "services.matches_installed_pending_kura_tip(expected)",
            "let mut lane_work = operation(expected, executor, services)?",
            "services.matches_lifecycle_lane_work(&lane_work)",
            "lane_work.install_lane_drain_queue(Arc::clone(&queue))?",
            "lane_work.activate_after_lane_drain_queue_install(&queue)?",
            "let _ = self.installed.take_genesis()",
            "PreparedPendingKuraLaneRecoveryV1 { installed, lane_work, launched, }",
        ),
    )
    pending_activation = item("pending_lifecycle", "activate_no_clock")
    require_order(
        "pending_lifecycle",
        pending_activation,
        "pending-Kura no-clock status and ingress activation",
        (
            "launched.executor.lifecycle_live_clocks_are_unarmed()",
            "launched.executor.ready_to_finish()",
            "matches_installed_pending_kura_tip(installed.expected())",
            "PendingKuraApplyRecoveryStage::Completed",
            "begin_fail_stop_operation()",
            "pending_kura_activation_status_snapshot()",
            "completion_observer_activation.take()",
            "activate_effect_completion_observer(observer)",
            "runner.open_and_publish_recovered_height(",
            "activation.complete()",
            "PendingKuraActivatedProductionLifecycleV1 { runner_activation, installed, lane_work, launched, }",
        ),
    )
    reject_tokens(
        "pending_lifecycle",
        pending_activation,
        "pending-Kura no-clock status and ingress activation",
        (
            "arm_live_clocks(",
            "successor_activation_status_snapshot(",
            "schedule_local_proposal(",
        ),
    )
    pending_local_finalization = item(
        "pending_lifecycle", "locally_ready_for_finalized_rollover"
    )
    require_order(
        "pending_lifecycle",
        pending_local_finalization,
        "pending-Kura local finalization census",
        (
            "self.launched.executor.ready_to_finish()",
            "self.launched.pending_kura_apply_replay.is_none()",
            "self.launched.recovered_local_proposal_attempt.is_none()",
            "pending_kura_apply_recovery_evidence()",
            "PendingKuraApplyRecoveryStage::Completed",
            "self.launched.pending_lifecycle_completion.is_none()",
            "self.launched.pending_ingress_capacity.is_none()",
            "self.launched.completion_observer_activation.is_none()",
            "matches_installed_pending_kura_tip(self.installed.expected())",
            "matches_lifecycle_lane_work(&self.lane_work)",
            "exactly_covers_finalization_work(&self.launched.owner.coordinator)",
        ),
    )
    pending_finalization = item("pending_lifecycle", "into_finalized_rollover")
    require_order(
        "pending_lifecycle",
        pending_finalization,
        "pending-Kura affine lane finalization",
        (
            "self.locally_ready_for_finalized_rollover()",
            "verify_published_store_marker_finalization_census()",
            "runner_activation.retire(&launched.leader_wire_ingress_binding.ingress)",
            "drop(installed)",
            "launched.leader_wire_ingress_binding.retire()",
            "ProductionLifecycleRetiredIngressPermitV1",
            "executor.into_finalized_parts()",
            "begin_fail_stop_operation()",
            "finish_height(&receipt, &artifact)",
            "operation.complete()",
            "FinalizedProductionLifecycleRolloverV1",
            "lane_work",
        ),
    )
    missing_pending = item("preactivation", "missing_pending_kura_replay")
    require_order(
        "preactivation",
        missing_pending,
        "missing pending-Kura replay fail-stop",
        (
            "output_guard.close_admission_for_restart()",
            "ProductionPendingKuraApplyInstallErrorV1::MissingReplay",
        ),
    )
    missing_pending_behavior = item(
        "preactivation", "missing_pending_kura_replay_closes_canonical_output"
    )
    require_order(
        "preactivation",
        missing_pending_behavior,
        "missing pending-Kura replay behavior",
        (
            "missing_pending_kura_replay(output_guard.as_ref())",
            "ProductionPendingKuraApplyInstallErrorV1::MissingReplay",
            "output_guard.restart_required()",
        ),
    )
    activation = item("launch", "activate_with")
    require_order(
        "launch",
        activation,
        "ordinary activation rejects pending-Kura recovery",
        (
            "lifecycle_activation_recovery_blocker(",
            "close_admission_for_restart()",
            "let clock_activation = ProductionLifecycleLiveClockActivationPermitV1",
            "self.executor.arm_live_clocks(clock_activation, now)",
        ),
    )
    _lifecycle_turn_driver_pending_kura_runner_source_fidelity_errors(
        paths, errors, item, require_order, reject_tokens, require_tokens
    )


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
            "let body_store_capacity = V2BodyStoreCapacity::new(",
            "let body_store = if emergency_fast",
            "V2BodyStore::open_emergency_fast_read_only(",
            "else",
            "V2BodyStore::open_with_policy_and_capacity(",
            ".into_quarantined_recovered_startup()",
            "SumeragiV2Adapter::open_recovered_startup_with_capacity_geometry(",
            ".bind_pending_kura_apply(pending_kura_apply)",
            ".authenticate_final_wal_startup_authority()?",
            "bind_production_lifecycle_owner_factory_inputs_v1(",
            "open_production_lifecycle_owner_v1(",
            "let launched = owner.launch(launch_inputs)?",
            "ProductionLifecyclePendingKuraRunnerActivationV1::mint_for_recovered_runner(",
            "launched.install_pending_kura_apply(&mut setup_runner)?",
            "pending.with_runner_setup(",
            "reconcile_executor_locked_body(executor, services)",
            "pending.drive_apply_recovery_turn(&mut setup_runner, control_queue_capacity)?",
            "require_committed_kagemusha_runtime_effective_config()",
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
            ".with_runner_setup(&mut setup_runner",
            "reconcile_executor_locked_body_for_pending_kura_test(",
            "loop",
            ".drive_apply_recovery_turn(&mut setup_runner, 64)",
            "ProductionPendingKuraApplyRecoveryProgressV1::Advanced",
            "ProductionPendingKuraApplyRecoveryProgressV1::Waiting",
            "ProductionPendingKuraApplyRecoveryProgressV1::Completed",
            "executor.lifecycle_live_clocks_are_unarmed()",
            "recovery_stages.ends_with(&[Stage::ApplicationDispatched, Stage::Completed])",
            ".prepare_lane_recovery(",
            "pending_kura_lifecycle_fixture_for_test(",
            ".activate_no_clock(activation)",
            "executor.lifecycle_live_clocks_are_unarmed()",
            "executor.ready_to_finish()",
            "services.matches_installed_pending_kura_tip(expected)",
            "services.matches_lifecycle_lane_work(lane_work)",
            "if !finalize",
            ".into_clean_shutdown(&mut active_runner)",
            ".into_finalized_rollover(&mut active_runner)",
            "finalized.finality()",
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
            "pending.is_storage_only_for_test()",
            "pending.expected_for_test()",
            "mismatched_pending",
            ".bind_pending_kura_apply(mismatched_pending)",
            ".authenticate_final_wal_startup_authority()",
            "AdapterError::RecoveredPendingKuraApplyMismatch",
            "pending.into_runtime_startup_for_test()",
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
