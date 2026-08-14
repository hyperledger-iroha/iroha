# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.



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


def _lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors(
    repo_root: Path,
) -> list[str]:
    """Pin the queue-owned ordinary/Serve ingress turn prerequisite."""

    errors: list[str] = []

    def load(relative: str, label: str) -> tuple[Path, str]:
        return _read_reviewed_rust_source(repo_root, relative, errors, label)

    paths: dict[str, Path] = {}
    sources: dict[str, str] = {}
    for name, relative in (
        (
            "ingress",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ingress_position.rs",
        ),
        (
            "selector",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs",
        ),
        (
            "driver",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        ),
        ("runtime", "crates/iroha_core/src/sumeragi/v2_runtime.rs"),
        (
            "launch",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        ),
        (
            "launch_tests",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_tests.rs",
        ),
        (
            "ledger",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
        ),
        (
            "preactivation",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_preactivation.rs",
        ),
        (
            "pending_lifecycle",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_pending_kura.rs",
        ),
        (
            "pending_kura",
            "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        ),
        ("effects", "crates/iroha_core/src/sumeragi/v2_effects.rs"),
        ("apply_tests", "crates/iroha_core/src/sumeragi/v2_apply_tests.rs"),
        ("worker", "crates/iroha_core/src/sumeragi/v2_worker.rs"),
        ("lane_work", "crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
        ("adapter", "crates/iroha_core/src/sumeragi/v2.rs"),
        (
            "scheduler",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        ),
        (
            "schema",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_schema.rs",
        ),
        (
            "coordinator",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs",
        ),
        (
            "coordinator_support",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator_support.rs",
        ),
        ("runner", "crates/iroha_core/src/sumeragi/v2_runner.rs"),
        (
            "ordinary_consumer",
            "crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs",
        ),
        (
            "height_driver",
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_height_driver.rs",
        ),
        (
            "lifecycle_run_inner",
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        ),
        (
            "pending_runner",
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        ),
        (
            "runner_authority",
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_runner_authority.rs",
        ),
        (
            "preactivation_ingress",
            "crates/iroha_core/src/sumeragi/v2_runner/preactivation_ingress.rs",
        ),
        (
            "startup_test",
            "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        ),
        (
            "wal_test",
            "crates/iroha_core/src/sumeragi/tests/v2_adapter_04_wal_recovery.rs",
        ),
    ):
        path, source = load(relative, f"queue-owned ordinary ingress {name}")
        paths[name] = path
        sources[name] = source
    if any(not source for source in sources.values()):
        return errors

    for source_name, child_path, declaration in (
        (
            "adapter",
            "v2_pending_kura_recovery.rs",
            'mod pending_kura_recovery;',
        ),
        (
            "launch",
            "v2_lifecycle_preactivation.rs",
            'mod preactivation;',
        ),
        (
            "launch",
            "v2_lifecycle_pending_kura.rs",
            'mod pending_kura;',
        ),
        (
            "runner",
            "v2_runner/ordinary_ingress_consumer.rs",
            'pub(in crate::sumeragi) mod ordinary_ingress_consumer;',
        ),
        (
            "runner",
            "v2_runner/lifecycle_height_driver.rs",
            'mod lifecycle_height_driver;',
        ),
        (
            "runner",
            "v2_runner/lifecycle_run_inner.rs",
            'pub(in crate::sumeragi) mod lifecycle_run_inner;',
        ),
        (
            "runner",
            "v2_runner/lifecycle_pending_kura.rs",
            'mod lifecycle_pending_kura;',
        ),
        (
            "runner",
            "v2_runner/lifecycle_runner_authority.rs",
            'mod lifecycle_runner_authority;',
        ),
        (
            "runner",
            "v2_runner/preactivation_ingress.rs",
            'mod preactivation_ingress;',
        ),
        (
            "coordinator",
            "v2_lifecycle_coordinator_support.rs",
            'mod coordinator_support;',
        ),
    ):
        path_token = f'#[path = "{child_path}"]'
        source = sources[source_name]
        if source.count(path_token) != 1 or source.count(declaration) != 1:
            errors.append(
                f"{paths[source_name]}: sealed lifecycle child module wiring "
                f"must retain exactly one {path_token!r} and {declaration!r}"
            )

    def item(source_name: str, name: str) -> RustItem | None:
        return _require_rust_item(
            paths[source_name], sources[source_name], name, errors
        )

    def require_tokens(
        source_name: str,
        rust_item: RustItem | None,
        label: str,
        tokens: tuple[str, ...],
    ) -> None:
        for token in tokens:
            _require_rust_token_sequence(
                paths[source_name], rust_item, token, label, errors
            )

    def require_order(
        source_name: str,
        rust_item: RustItem | None,
        label: str,
        markers: tuple[str, ...],
    ) -> None:
        if rust_item is None:
            return
        body = rust_code_tokens(rust_item.source)
        cursor = 0
        for marker in markers:
            needle = rust_code_tokens(marker)
            position = next(
                (
                    index
                    for index in range(cursor, len(body) - len(needle) + 1)
                    if body[index : index + len(needle)] == needle
                ),
                -1,
            )
            if position < 0:
                errors.append(
                    f"{paths[source_name]}:{rust_item.line}: {label} must "
                    f"preserve exact order {markers!r}"
                )
                return
            cursor = position + len(needle)

    def reject_tokens(
        source_name: str,
        rust_item: RustItem | None,
        label: str,
        forbidden: tuple[str, ...],
    ) -> None:
        if rust_item is None:
            return
        body = rust_code_tokens(rust_item.source)
        observed = tuple(
            token
            for token in forbidden
            if _token_sequence_count(body, rust_code_tokens(token))
        )
        if observed:
            errors.append(
                f"{paths[source_name]}:{rust_item.line}: {label} retains "
                f"forbidden ordinary-height authority {observed!r}"
            )

    launched_fields = sources["launch"]
    launched_start = launched_fields.find(
        "pub(in crate::sumeragi) struct LaunchedProductionLifecycleV1"
    )
    launched_end = launched_fields.find(
        "/// Result of draining one dedicated recovered Apply worker completion.",
        launched_start,
    )
    launched_region = (
        launched_fields[launched_start:launched_end]
        if launched_start >= 0 and launched_end > launched_start
        else ""
    )
    launched_cursor = 0
    for token in (
        "services: ProductionV2Services",
        "pending_kura_apply_replay: Option<super::super::v2::PreparedRecoveredPendingKuraApplyReplayV1>",
        "recovered_local_proposal_attempt:",
        "Option<super::super::v2::RecoveredLifecycleLocalProposalAttemptV1>",
        "recovered_decision_apply_deferred: Option<RetainedRecoveredDecisionApplyDeferredV1>",
        "recovered_decision_fetch_body_completion:",
        "Option<PreparedRecoveredDecisionFetchBodyCompletionV1>",
        "recovered_lifecycle_sign_completion: Option<PreparedRecoveredLifecycleSignCompletionV1>",
        "recovered_ingress_capacity_wait: Option<super::PreparedProductionIngressCapacityWait>",
        "leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1",
    ):
        position = launched_region.find(token, launched_cursor)
        if position < 0:
            errors.append(
                f"{paths['launch']}: launched recovered Sign Drop order must "
                f"retain ordered field {token!r}"
            )
            break
        launched_cursor = position + len(token)

    aperture_open_candidates = [
        rust_item
        for rust_item in rust_items(
            sources["preactivation_ingress"], "open_canonical_recovery_ingress"
        )
        if not rust_item.brace_context
    ]
    if len(aperture_open_candidates) != 1:
        errors.append(
            f"{paths['preactivation_ingress']}: canonical-recovery aperture "
            f"must retain one free open constructor; found "
            f"{len(aperture_open_candidates)}"
        )
        aperture_open = None
    else:
        aperture_open = aperture_open_candidates[0]
    require_order(
        "preactivation_ingress",
        aperture_open,
        "preactivation canonical-recovery ingress open",
        (
            "Arc::ptr_eq(block_ingress, launched_ingress)",
            "ingress_ready.load(Ordering::Acquire)",
            "block_ingress.state.lock().open",
            "block_ingress.open()",
            "ingress_ready.store(true, Ordering::Release)",
            "ProductionLifecycleCanonicalRecoveryIngressV1",
        ),
    )
    aperture_drop_items = [
        rust_item
        for rust_item in rust_items(sources["preactivation_ingress"], "drop")
        if len(rust_item.brace_context) == 1
        and rust_item.brace_context[0][:3] == ("impl", "Drop", "for")
        and "ProductionLifecycleCanonicalRecoveryIngressV1"
        in rust_item.brace_context[0]
    ]
    if len(aperture_drop_items) != 1:
        errors.append(
            f"{paths['preactivation_ingress']}: canonical-recovery aperture must "
            f"retain one RAII Drop; found {len(aperture_drop_items)}"
        )
    else:
        require_tokens(
            "preactivation_ingress",
            aperture_drop_items[0],
            "preactivation canonical-recovery ingress RAII close",
            ("self.close()",),
        )
    aperture_close = item("preactivation_ingress", "close")
    require_order(
        "preactivation_ingress",
        aperture_close,
        "preactivation canonical-recovery ingress close",
        (
            "self.ingress_ready.store(false, Ordering::Release)",
            "self.block_ingress.close()",
            "self.open = false",
        ),
    )
    aperture_transaction = item(
        "preactivation", "with_canonical_body_recovery_ingress_transaction"
    )
    require_order(
        "preactivation",
        aperture_transaction,
        "launched canonical-recovery aperture transaction",
        (
            "self.with_runner_setup_transaction",
            "activation.open_canonical_recovery_ingress(&launched_ingress)",
            "operation(&aperture, executor, services)",
            "aperture.close_and_verify()",
            "result",
        ),
    )
    require_tokens(
        "preactivation",
        item("preactivation", "with_canonical_body_recovery_ingress"),
        "ordinary preactivation canonical-recovery aperture",
        (
            "self.with_canonical_body_recovery_ingress_transaction(runner, activation, operation)",
        ),
    )
    if sources["lifecycle_run_inner"].count(
        "recover_canonical_bodies_before_activation("
    ) != 3:
        errors.append(
            f"{paths['lifecycle_run_inner']}: lifecycle startup must retain one "
            "canonical recovery helper and exactly two startup repair call sites"
        )

    runner_ingress_retire = item("runner", "retire_lifecycle_runner_ingress")
    require_order(
        "runner",
        runner_ingress_retire,
        "shared lifecycle runner ingress retirement",
        (
            "ingress_ready.store(false, Ordering::Release)",
            "block_ingress.close()",
            "Arc::ptr_eq(block_ingress, launched_ingress)",
        ),
    )
    for owner in (
        "ProductionLifecycleRunnerActivationV1",
        "ProductionLifecycleCompleteTipRunnerActivationV1",
    ):
        matches = [
            rust_item
            for rust_item in rust_items(
                sources["runner_authority"], "retire_unpublished"
            )
            if rust_item.brace_context == (("impl", owner),)
        ]
        if len(matches) != 1:
            errors.append(
                f"{paths['runner_authority']}: {owner} must retain one consuming "
                f"unpublished retirement; found {len(matches)}"
            )
        else:
            require_tokens(
                "runner_authority",
                matches[0],
                f"{owner} unpublished retirement",
                ("retire_lifecycle_runner_ingress(",),
            )

    shutdown_finish = item("launch", "finish_clean_shutdown")
    require_order(
        "launch",
        shutdown_finish,
        "lifecycle clean-shutdown tail",
        (
            "self.leader_wire_ingress_binding.retire()",
            "runner_retirement",
            "ingress_retirement",
            "let Some(operation) = operation",
            "self.services.allow_clean_shutdown()",
            "operation.complete()",
        ),
    )
    complete_tip_shutdown_tail = [
        rust_item
        for rust_item in rust_items(
            sources["launch"], "into_complete_tip_clean_shutdown"
        )
        if rust_item.brace_context
        == (("impl", "LaunchedProductionLifecycleV1"),)
    ]
    if len(complete_tip_shutdown_tail) != 1:
        errors.append(
            f"{paths['launch']}: CompleteTip lifecycle shutdown must retain "
            f"one sealed inner tail; found {len(complete_tip_shutdown_tail)}"
        )
    else:
        require_order(
            "launch",
            complete_tip_shutdown_tail[0],
            "CompleteTip lifecycle clean-shutdown tail",
            (
                "output_guard.begin_fail_stop_operation()",
                "runner.retire_unpublished(&self.leader_wire_ingress_binding.ingress)",
                "drop(retirement)",
                "self.finish_clean_shutdown(operation, runner_retirement)",
            ),
        )
    launched_shutdowns = [
        rust_item
        for rust_item in rust_items(sources["launch"], "into_clean_shutdown")
        if rust_item.brace_context == (("impl", "LaunchedProductionLifecycleV1"),)
    ]
    active_shutdowns = [
        rust_item
        for rust_item in rust_items(sources["launch"], "into_clean_shutdown")
        if rust_item.brace_context == (("impl", "ActivatedProductionLifecycleV1"),)
    ]
    for label, candidates, markers in (
        (
            "unpublished lifecycle clean shutdown",
            launched_shutdowns,
            (
                "output_guard.begin_fail_stop_operation()",
                "runner.retire_unpublished(&self.leader_wire_ingress_binding.ingress)",
                "self.finish_clean_shutdown(operation, runner_retirement)",
            ),
        ),
        (
            "active lifecycle clean shutdown",
            active_shutdowns,
            (
                "let Self { launched, local_proposal, runner_activation, } = self",
                "output_guard.begin_fail_stop_operation()",
                "runner_activation.retire(&launched.leader_wire_ingress_binding.ingress)",
                "drop(local_proposal)",
                "launched.finish_clean_shutdown(operation, runner_retirement)",
            ),
        ),
    ):
        if len(candidates) != 1:
            errors.append(
                f"{paths['launch']}: {label} must retain one consuming method; "
                f"found {len(candidates)}"
            )
            continue
        require_order("launch", candidates[0], label, markers)
        for forbidden in (
            "into_finalized_parts",
            "rollover_finalized_height_outputs",
            "stage_finalized_height_all_row_retirement",
            "finish_height(",
        ):
            if _token_sequence_count(
                rust_code_tokens(candidates[0].source), rust_code_tokens(forbidden)
            ):
                errors.append(
                    f"{paths['launch']}:{candidates[0].line}: {label} must not "
                    f"claim finality through {forbidden!r}"
                )

    complete_tip_setup = [
        rust_item
        for rust_item in rust_items(sources["ledger"], "with_runner_setup")
        if rust_item.brace_context
        == (("impl", "LaunchedRecoveredCompleteTipSuccessorLifecycleV1"),)
    ]
    if len(complete_tip_setup) != 1:
        errors.append(
            f"{paths['ledger']}: CompleteTip closed-ingress runner setup must "
            f"remain one sealed delegate; found {len(complete_tip_setup)}"
        )
    else:
        require_order(
            "ledger",
            complete_tip_setup[0],
            "CompleteTip sealed closed-ingress runner setup",
            (
                "runner",
                "operation",
                "E: From<super::launch::ProductionLifecyclePreActivationErrorV1>",
                "self.launched.with_runner_setup(runner, operation)",
            ),
        )

    complete_tip_shutdown = [
        rust_item
        for rust_item in rust_items(sources["ledger"], "into_clean_shutdown")
        if rust_item.brace_context
        == (("impl", "LaunchedRecoveredCompleteTipSuccessorLifecycleV1"),)
    ]
    if len(complete_tip_shutdown) != 1:
        errors.append(
            f"{paths['ledger']}: CompleteTip clean shutdown must remain one "
            f"sealed delegate; found {len(complete_tip_shutdown)}"
        )
    else:
        require_order(
            "ledger",
            complete_tip_shutdown[0],
            "CompleteTip sealed clean shutdown",
            (
                "let Self {",
                "launched",
                "retirement",
                "} = self",
                "launched.into_complete_tip_clean_shutdown(runner, retirement)",
            ),
        )

    shutdown_behavior = item(
        "startup_test",
        "production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies",
    )
    require_tokens(
        "startup_test",
        shutdown_behavior,
        "production lifecycle clean-shutdown behavior",
        (
            ".with_runner_setup(&mut setup_runner",
            "launch_non_pending_lifecycle_height_and_shutdown_for_test(",
            ".into_clean_shutdown(&mut runner)",
        ),
    )
    require_order(
        "startup_test",
        shutdown_behavior,
        "production unpublished lifecycle clean-shutdown behavior",
        (
            "if shutdown_before_activation",
            "launch_non_pending_lifecycle_height_and_shutdown_for_test(",
            "None",
            "assert!(!ingress_ready.load(Ordering::Acquire))",
            "assert!(!leader_wire_ingress.state.lock().open)",
            "assert!(!output_guard.restart_required())",
            "continue",
        ),
    )
    complete_tip_shutdown_behavior = item(
        "startup_test",
        "complete_tip_launched_lifecycle_shuts_down_without_publishing_successor",
    )
    require_order(
        "startup_test",
        complete_tip_shutdown_behavior,
        "production CompleteTip lifecycle clean-shutdown behavior",
        (
            "complete_tip_lifecycle_shutdown_fixture()",
            "launch_non_pending_lifecycle_height_and_shutdown_for_test(",
            "Some(retirement)",
            "assert!(!ingress_ready.load(Ordering::Acquire))",
            "assert!(!ingress_state.open)",
            "assert!(ingress_state.certified_serve_gate.is_none())",
            "assert!(ingress_state.leader_wire_lifecycle_gate.is_none())",
            "assert!(!output_guard.restart_required())",
            "assert!(crate::sumeragi::status::v2_status().is_none())",
        ),
    )
    require_order(
        "startup_test",
        shutdown_behavior,
        "production active lifecycle clean-shutdown behavior",
        (
            "if shutdown_after_activation",
            ".into_clean_shutdown(&mut runner)",
            "assert!(!ingress_ready.load(Ordering::Acquire))",
            "assert!(!leader_wire_ingress.state.lock().open)",
            "assert!(!output_guard.restart_required())",
            "crate::sumeragi::status::clear_v2_status()",
            "continue",
        ),
    )

    outcome_source = sources["driver"]
    completion_outcome_start = outcome_source.find(
        "pub(in crate::sumeragi) enum ProductionLifecycleCompletionTurnV1<'cursor>"
    )
    ingress_outcome_end = outcome_source.find(
        "impl LaunchedProductionLifecycleV1", completion_outcome_start
    )
    outcomes = (
        outcome_source[completion_outcome_start:ingress_outcome_end]
        if completion_outcome_start >= 0 and ingress_outcome_end > completion_outcome_start
        else ""
    )
    outcome_tokens = rust_code_tokens(outcomes)
    for token, count in (
        ("PassThrough(LifecycleCurrentRunnerTurn<'cursor>)", 2),
        ("Selected(ProductionLifecycleCompletionSelectionV1)", 1),
        ("Selected(ProductionLifecycleIngressSelectionV1)", 1),
        ("Ordinary(ProductionPreparedOrdinaryIngressTurnV1)", 1),
    ):
        observed = _token_sequence_count(outcome_tokens, rust_code_tokens(token))
        if observed != count:
            errors.append(
                f"{paths['driver']}: borrow-bound lifecycle turn outcomes must "
                f"contain {token!r} exactly {count} time(s); found {observed}"
            )
    for forbidden in (
        "LifecycleRunnerRankSnapshot",
        "derive(Clone)",
        "derive(Copy)",
        "fn into_parts(",
    ):
        if _token_sequence_count(outcome_tokens, rust_code_tokens(forbidden)):
            errors.append(
                f"{paths['driver']}: borrow-bound lifecycle turn outcomes expose "
                f"forbidden token {forbidden!r}"
            )

    completion_items = [
        rust_item
        for rust_item in rust_items(sources["driver"], "drive_completion_turn")
        if rust_item.brace_context
        == (("impl", "LaunchedProductionLifecycleV1"),)
    ]
    if len(completion_items) != 1:
        errors.append(
            f"{paths['driver']}: unified lifecycle Completion turn driver must "
            f"have one launched owner; found {len(completion_items)}"
        )
        completion = None
    else:
        completion = completion_items[0]
    require_order(
        "driver",
        completion,
        "unified lifecycle Completion parked-owner and Ready order",
        (
            "self.recovered_decision_apply_deferred.take()",
            "self.recovered_lifecycle_sign_completion.is_some()",
            "self.recovered_decision_fetch_body_completion.is_some()",
            "self.services.take_next_recovered_lifecycle_completion()",
            "self.owner.classify_completion_ready_work()",
        ),
    )
    if completion is not None:
        completion_tokens = rust_code_tokens(completion.source)
        for token, count, label in (
            (
                "self.services.take_next_recovered_lifecycle_completion()",
                1,
                "unified lifecycle Completion single-head classifier",
            ),
            (
                "if result.is_err() { self.close_output_for_restart(); }",
                2,
                "unified lifecycle Completion selected-dispatch failure closure",
            ),
        ):
            observed = _token_sequence_count(completion_tokens, rust_code_tokens(token))
            if observed != count:
                errors.append(
                    f"{paths['driver']}:{completion.line}: {label} must contain "
                    f"{token!r} exactly {count} time(s); found {observed}"
                )
    require_tokens(
        "driver",
        completion,
        "unified lifecycle Completion pass-through ownership",
        (
            "RecoveredLifecycleCompletionTakeV1::PassThrough",
            "ProductionCompletionReadyWorkV1::PassThrough",
        ),
    )
    if completion is not None:
        completion_pass_throughs = _token_sequence_count(
            rust_code_tokens(completion.source),
            rust_code_tokens(
                "return ProductionLifecycleCompletionTurnV1::PassThrough(runner)"
            ),
        )
        if completion_pass_throughs != 3:
            errors.append(
                f"{paths['driver']}:{completion.line}: unified lifecycle Completion "
                "pass-through ownership must preserve exactly three early-return "
                f"sites; found {completion_pass_throughs}"
            )

    completion_head = item("worker", "take_next_recovered_lifecycle_completion")
    require_order(
        "worker",
        completion_head,
        "unified physical Completion ordinary-head restoration",
        (
            "self.held_io_completion.take()",
            "match completion",
            "ordinary =>",
            "self.held_io_completion = Some(ordinary)",
            "RecoveredLifecycleCompletionTakeV1::PassThrough",
        ),
    )
    if completion_head is not None:
        for forbidden in ("acknowledge_completion(&ordinary)", "ordinary.into_parts()"):
            if _token_sequence_count(
                rust_code_tokens(completion_head.source), rust_code_tokens(forbidden)
            ):
                errors.append(
                    f"{paths['worker']}:{completion_head.line}: unified physical "
                    f"Completion ordinary-head restoration found {forbidden!r}"
                )

    settlement_family = item("adapter", "settlement_family")
    require_tokens(
        "adapter",
        settlement_family,
        "publication-inert recovered Sign settlement family",
        (
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::Broadcast",
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalPrepareWal",
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::VoteBroadcastAndSign",
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalBroadcastAndSign",
            "wire::ConsensusMessageV2Payload::Proposal(proposal)",
            "wire::ConsensusMessageV2Payload::Vote(vote)",
            "_ => None",
        ),
    )
    sign_settlement = item("driver", "settle_parked_recovered_sign_completion")
    require_order(
        "driver",
        sign_settlement,
        "unified recovered Sign settlement routing",
        (
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::Broadcast",
            "self.settle_recovered_lifecycle_sign_broadcast()",
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalPrepareWal",
            "self.settle_recovered_lifecycle_proposal_prepare_wal()",
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::VoteBroadcastAndSign",
            "self.settle_recovered_lifecycle_vote_broadcast_and_sign()",
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalBroadcastAndSign",
            "self.settle_recovered_lifecycle_proposal_broadcast_and_sign()",
        ),
    )
    sign_classification = item("driver", "classify_parked_recovered_sign_completion")
    require_order(
        "driver",
        sign_classification,
        "single-preview recovered Sign structural classification",
        (
            "completion.project_adapter_completion_authority()",
            "prepare_recovered_lifecycle_sign_completion(authority)",
            "preview.settlement_family()",
            "drop(preview)",
            "class",
        ),
    )

    fetch_phase_a = item("driver", "drive_recovered_ingress_selector")
    require_order(
        "driver",
        fetch_phase_a,
        "recovered Fetch Phase-A service failure",
        (
            "ProductionRecoveredDecisionFetchPersistenceErrorV1::Service",
            "drop(prepared)",
            "self.close_output_for_restart()",
            "ProductionLifecycleIngressSelectionV1::RestartRequired",
        ),
    )
    capacity_retry_items = [
        rust_item
        for rust_item in rust_items(sources["scheduler"], "retry")
        if rust_item.brace_context
        == (("impl", "PreparedProductionIngressCapacityWait"),)
    ]
    if len(capacity_retry_items) != 1:
        errors.append(
            f"{paths['scheduler']}: retained ingress capacity wait consuming "
            f"retry must have one owner; found {len(capacity_retry_items)}"
        )
        capacity_retry = None
    else:
        capacity_retry = capacity_retry_items[0]
    require_order(
        "scheduler",
        capacity_retry,
        "retained ingress capacity wait consuming retry",
        (
            "if self.mode != executor.lifecycle_mode_rank_snapshot()",
            "LifecycleIoCapacityWaitStatus::SamePending",
            "ProductionIngressCapacityRetry::Pending(self)",
            "LifecycleIoCapacityWaitStatus::Released",
            "ProductionIngressCapacityRetry::Released(selector)",
        ),
    )
    capacity_struct_start = sources["scheduler"].find(
        "pub(crate) struct PreparedProductionIngressCapacityWait"
    )
    capacity_struct_end = sources["scheduler"].find(
        "/// Opaque status of one service-owned capacity-generation wait.",
        capacity_struct_start,
    )
    capacity_region = (
        sources["scheduler"][capacity_struct_start:capacity_struct_end]
        if capacity_struct_start >= 0 and capacity_struct_end > capacity_struct_start
        else ""
    )
    for forbidden in (
        "#[derive(Clone)]",
        "pub(crate) selector: PreparedLifecycleIngressSelector",
        "fn selector(",
        "fn into_parts(",
    ):
        if forbidden in capacity_region:
            errors.append(
                f"{paths['scheduler']}: retained ingress capacity wait must "
                f"remain sealed; found {forbidden!r}"
            )
    ready_classifier = item("scheduler", "classify_completion_ready_classes")
    require_order(
        "scheduler",
        ready_classifier,
        "unified Completion Ready supported-coexistence order",
        (
            "LifecycleWorkClass::CertifiedServe",
            "LifecycleWorkClass::ProducerTurn",
            "ProductionCompletionReadyWorkV1::PassThrough",
            "LifecycleWorkClass::Broadcast",
            "ProductionCompletionReadyWorkV1::RecoveredLifecycleBroadcast",
            "LifecycleWorkClass::Validate",
            "if classes.iter().all(|class|",
            "LifecycleWorkClass::Apply",
            "LifecycleWorkClass::Fetch",
            "ProductionCompletionReadyWorkV1::RecoveredIo",
        ),
    )

    capture = item("ingress", "capture_next_ingress_turn_cut")
    require_tokens(
        "ingress",
        capture,
        "queue-owned fair winner capture",
        (
            "let service_guard = self.service_lock.lock()",
            "let mut state = self.state.lock()",
            "select_fair_v2_ingress_candidate(",
            "Ok(Some(FairIngressTurnCut {",
            "_service_guard: service_guard",
        ),
    )
    if capture is not None:
        capture_tokens = rust_code_tokens(capture.source)
        for token, count in (
            ("selected_physical_ordinal", 3),
            ("selected_disposition", 2),
        ):
            observed = _token_sequence_count(capture_tokens, rust_code_tokens(token))
            if observed != count:
                errors.append(
                    f"{paths['ingress']}:{capture.line}: queue-owned fair winner "
                    f"capture must contain {token!r} exactly {count} time(s); "
                    f"found {observed}"
                )
    require_order(
        "ingress",
        capture,
        "queue-owned fair winner lock and selection order",
        (
            "self.service_lock.lock()",
            "self.state.lock()",
            "freeze_live_geometry(",
            "drop(state)",
            "validate_frozen_ownership_outside_state(",
            "select_fair_v2_ingress_candidate(",
            "FairIngressTurnCut {",
        ),
    )

    narrow = item("ingress", "narrow_to_lifecycle")
    require_tokens(
        "ingress",
        narrow,
        "exact winner context narrowing",
        (
            "FairIngressTurnContextCut::Ordinary(self)",
            "mint_pending_identities(bound_context, &self.geometry)",
            "FairIngressTurnContextCut::Lifecycle(cut)",
        ),
    )
    exact_dequeue = item("ingress", "dequeue_exact_retaining")
    require_order(
        "ingress",
        exact_dequeue,
        "exact queue-owned physical dequeue",
        (
            "drop(std::mem::take(&mut self.selector_occurrences))",
            "let mut state = self.queue.state.lock()",
            "self.queue.dequeue_selected_locked(",
            "self.selected_source_index",
            "self.selected_physical_ordinal",
            "self.selected_disposition",
        ),
    )

    driver_items = [
        rust_item
        for rust_item in rust_items(sources["driver"], "drive_ingress_turn")
        if rust_item.brace_context
        == (("impl", "LaunchedProductionLifecycleV1"),)
    ]
    if len(driver_items) != 1:
        errors.append(
            f"{paths['driver']}: require exactly one launched queue-owned "
            f"drive_ingress_turn; found {len(driver_items)}"
        )
        driver = None
    else:
        driver = driver_items[0]
    require_order(
        "driver",
        driver,
        "ordinary/recovered ingress owner order",
        (
            "self.recovered_ingress_capacity_wait.take()",
            "self.executor.lifecycle_terminal_subject()",
            "capture_next_ingress_turn_cut(",
            "v2_ingress_head_can_drain(",
            "FairV2IngressDequeueDisposition::RetireObsolete",
            "selected_ingress_is_current_certified_serve(",
            "selected_ingress_is_certified_body_response(",
            "cut.narrow_to_lifecycle(expected_context)",
            "selected_cut_is_recovered_decision_fetch(&cut)",
            "prepare_recovered_decision_fetch_from_selected_cut(cut)",
            "self.drive_recovered_ingress_selector(selector, runner)",
        ),
    )
    require_tokens(
        "driver",
        driver,
        "ordinary exact-winner handoff",
        (
            "cut.into_ordinary_turn_cut()",
        ),
    )
    if driver is not None:
        driver_tokens = rust_code_tokens(driver.source)
        for token, count in (
            (
                "dequeue_prepared_ordinary_ingress(",
                4,
            ),
            ("ProductionLifecycleIngressTurnV1::PassThrough(runner)", 2),
        ):
            observed = _token_sequence_count(driver_tokens, rust_code_tokens(token))
            if observed != count:
                errors.append(
                    f"{paths['driver']}:{driver.line}: ordinary exact-winner "
                    f"handoff must contain {token!r} exactly {count} time(s); "
                    f"found {observed}"
                )
    _require_rust_token_sequence(
        paths["driver"],
        driver,
        """
if !selected_ingress_is_certified_body_response(cut.selected_occurrence().inbound()) {
    return dequeue_prepared_ordinary_ingress(
        &ingress,
        cut,
        runner,
        None,
        terminal_subject,
        &self.services,
    );
}
""",
        "selected non-response winner bypasses response census",
        errors,
    )
    if driver is not None:
        capture_start = driver.source.find(".capture_next_ingress_turn_cut(")
        capture_end = driver.source.find("let Some(cut)", capture_start)
        pure_capture = (
            driver.source[capture_start:capture_end]
            if capture_start >= 0 and capture_end > capture_start
            else ""
        )
        if (
            "v2_ingress_head_can_drain" not in pure_capture
            or "prepare_certified_request" in pure_capture
            or "stage_certified_serve_rejection" in pure_capture
        ):
            errors.append(
                f"{paths['driver']}:{driver.line}: physical winner selection "
                "must use only the shared pure drain predicate"
            )

    serve = item("driver", "prepare_and_dequeue_current_certified_serve")
    require_order(
        "driver",
        serve,
        "stateful selected Serve fail-stop transaction",
        (
            "output_guard.begin_fail_stop_operation()",
            "executor.authenticate_certified_body_request(",
            "services.stage_certified_serve_rejection(",
            "services.prepare_certified_request(",
            "CertifiedServePrepareError::Backpressure",
            "operation.complete()",
            "drop(cut)",
            "ProductionLifecycleIngressSelectionV1::OrdinaryRetained",
            "cut.dequeue_exact_retaining()",
            "prepared_ordinary_ingress_turn(",
            "Some(prepared)",
        ),
    )
    require_tokens(
        "driver",
        serve,
        "selected Serve admitted/rejected reservation transfer",
        (
            "ProductionPreparedCertifiedServeV1::Admitted(admission)",
            "ProductionPreparedCertifiedServeV1::Rejected(error.to_string())",
            "drop(operation); drop(retained); drop(runner)",
        ),
    )
    if serve is not None:
        service_outcomes = _token_sequence_count(
            rust_code_tokens(serve.source),
            rust_code_tokens("ProductionPreparedCertifiedServeV1::Service(reason)"),
        )
        if service_outcomes != 4:
            errors.append(
                f"{paths['driver']}:{serve.line}: selected Serve transfer must "
                "retain all four service-failure owners; "
                f"found {service_outcomes}"
            )

    token_source = sources["driver"]
    token_start = token_source.find(
        "pub(in crate::sumeragi) struct ProductionPreparedOrdinaryIngressTurnV1"
    )
    token_end = token_source.find(
        "pub(in crate::sumeragi) enum ProductionLifecycleIngressTurnV1", token_start
    )
    token_region = (
        token_source[token_start:token_end]
        if token_start >= 0 and token_end > token_start
        else ""
    )
    for required in (
        "handoff: Option<PreparedDequeuedV2IngressV1>",
        "impl Drop for ProductionPreparedOrdinaryIngressTurnV1",
        "handoff.close_output_for_restart()",
    ):
        if required not in token_region:
            errors.append(
                f"{paths['driver']}: opaque ordinary token omits {required!r}"
            )
    for forbidden in (
        "pub handoff:",
        "pub(crate) handoff:",
        "pub(in crate::sumeragi) handoff:",
        "fn into_parts(",
        "fn services(",
        "fn executor(",
        "derive(Clone)",
        "derive(Copy)",
    ):
        if forbidden in token_region:
            errors.append(
                f"{paths['driver']}: opaque ordinary token exposes forbidden "
                f"surface {forbidden!r}"
            )

    selected_family = item("selector", "selected_cut_is_recovered_decision_fetch")
    require_tokens(
        "selector",
        selected_family,
        "selected response-family-only recovery census",
        (
            "let Some(selected_request_hash) = selected_request_hash else { return Ok(false); }",
            "if response.request_hash != selected_request_hash { continue; }",
            "lowest_physical_ordinal_per_family(",
            "cut.pre_cut_is_intact()",
        ),
    )
    family_prepare = item(
        "selector", "prepare_recovered_decision_fetch_from_selected_cut"
    )
    require_tokens(
        "selector",
        family_prepare,
        "selected-family Phase-A preparation",
        (
            "capture_lifecycle_ingress_selector_for_response_family( cut, Some(selected_request_hash), )",
            "PreparedLifecycleIngressIoTarget::RecoveredDecisionFetchBodyPersistence",
        ),
    )

    for source_name, test_name in (
        (
            "ingress",
            "shared_selector_keeps_strict_dependency_blocked_and_obsolete_ordering",
        ),
        (
            "ingress",
            "turn_cut_dequeues_exact_winner_once_and_preserves_ready_rotation",
        ),
        ("ingress", "foreign_winner_dequeues_as_ordinary_without_reselection"),
        ("ingress", "ordinary_head_ignores_later_unowned_invalid_response"),
        (
            "driver",
            "armed_token_closes_output_before_releasing_dequeued_carrier_and_serve_result",
        ),
    ):
        item(source_name, test_name)
    wal_fetch = item(
        "wal_test", "bls_decision_fetch_repairs_and_coalesces_without_rewrite"
    )
    require_order(
        "wal_test",
        wal_fetch,
        "genuine recovered Fetch composite-dispatch behavior",
        (
            "add_recovered_next_vote_completion_for_test(0xCD)",
            "mixed_sign_ordinal > first_summary.0",
            "bind_body_store_to_recovered_completion_io_for_test(",
            "install_local_signer_for_test(",
            "dispatch_recovered_completion_for_test(",
            "ProductionRecoveredCompletionDispatchV1::SignQueued",
            "recovered_completion_selection_is_exact_for_test(",
            "output_guard.close_admission_for_restart()",
            "output_guard.restart_required()",
            "drop(first)",
            "let mut reopened = reopened",
            "bind_body_store_to_recovered_completion_io_for_test(",
            "dispatch_recovered_completion_for_test(",
            "ProductionRecoveredCompletionDispatchV1::FetchDispatched",
            "services.has_pending_exact_output()",
            "planner_io.detach(&mut services)",
        ),
    )
    composite_capture = item("worker", "capture_recovered_completion_capacity_census")
    require_order(
        "worker",
        composite_capture,
        "joint recovered Completion physical-corridor census",
        (
            "for probe in probes",
            "let fanout = self.recovered_decision_fetch_fanout(&owner)?",
            "begin_fail_stop_operation()",
            "let pending = self.lock_pending_exact_output()?",
            "let state = io.command_tx.queue.lock()",
            "for candidate in census.candidates.values_mut()",
        ),
    )
    require_tokens(
        "worker",
        composite_capture,
        "joint recovered Completion physical-corridor census",
        (
            "RecoveredCompletionCapacityProbeV1::Apply",
            "RecoveredCompletionCapacityProbeV1::Sign",
            "RecoveredCompletionCapacityProbeV1::Fetch",
            "pending.can_enqueue(fanout)",
        ),
    )
    composite_dispatch = item(
        "scheduler", "dispatch_recovered_completion_with_runner_debt"
    )
    require_order(
        "scheduler",
        composite_dispatch,
        "all-row recovered Completion authentication and selection",
        (
            "let exact_ready = self.coordinator.ready_index.clone()",
            "for ordinal in &exact_ready",
            "capture_recovered_completion_capacity_census(probes)",
            "authenticated_ready_row_with_physical_capacity(",
            "let inputs = authenticated_scheduler_inputs(",
            "self.coordinator.plan_turn(inputs)",
            "let ordinal = lease.ordinal()",
            "match expected_class",
        ),
    )
    require_tokens(
        "scheduler",
        composite_dispatch,
        "all-row recovered Completion authentication and selection",
        (
            "census.complete_without_selection()",
            "census.select_apply(ordinal)",
            "census.select_sign(ordinal)",
            "census.select_fetch(ordinal)",
            "registration.commit(prepared)",
            "output.commit()",
        ),
    )
    if composite_dispatch is not None:
        physical_rows = _token_sequence_count(
            rust_code_tokens(composite_dispatch.source),
            rust_code_tokens("authenticated_ready_row_with_physical_capacity("),
        )
        if physical_rows != 3:
            errors.append(
                f"{paths['scheduler']}:{composite_dispatch.line}: all-row recovered "
                f"Completion authentication and selection must project exactly three "
                f"physical row classes; found {physical_rows}"
            )
    physical_row = item("schema", "from_authenticated_with_physical_capacity")
    require_tokens(
        "schema",
        physical_row,
        "authenticated Ready physical-capacity bit",
        (
            "Self::from_authenticated(",
            "row.physical_capacity_available = physical_capacity_available",
        ),
    )
    if physical_row is not None:
        physical_capacity_tokens = _token_sequence_count(
            rust_code_tokens(physical_row.source),
            rust_code_tokens("physical_capacity_available"),
        )
        if physical_capacity_tokens != 3:
            errors.append(
                f"{paths['schema']}:{physical_row.line}: authenticated Ready "
                "physical-capacity bit must remain parameter, assignment target, "
                f"and assignment source; found {physical_capacity_tokens}"
            )
    require_order(
        "driver",
        completion,
        "unified recovered Completion composite dispatch",
        (
            "ProductionCompletionReadyWorkV1::RecoveredIo",
            "owner.dispatch_recovered_completion_with_runner_debt(",
            "if result.is_err()",
            "ProductionLifecycleCompletionSelectionV1::RecoveredIoDispatch(result)",
        ),
    )
    behavior_items = {}
    for source_name, test_name in (
        (
            "worker",
            "recovered_completion_capacity_census_selects_once_and_drops_fail_stop",
        ),
        (
            "scheduler",
            "composite_recovered_completion_dispatches_one_ranked_sign_and_preserves_the_other",
        ),
        (
            "scheduler",
            "composite_recovered_completion_capacity_unavailable_claims_no_ready_sign",
        ),
        (
            "wal_test",
            "bls_decision_fetch_repairs_and_coalesces_without_rewrite",
        ),
    ):
        behavior_items[test_name] = item(source_name, test_name)
    require_order(
        "scheduler",
        behavior_items[
            "composite_recovered_completion_dispatches_one_ranked_sign_and_preserves_the_other"
        ],
        "composite recovered Completion Sign selection behavior",
        (
            "dispatch_recovered_completion_with_runner_debt(&services, &mut executor, 0,)",
            "ProductionRecoveredCompletionDispatchV1::SignQueued { ordinal: paired }",
            "state.records[&paired].state",
            "LifecycleState::Claimed(_)",
            "state.records[&unrelated].state",
            "LifecycleState::Ready",
            "state.active_lease.is_some()",
            "state.fault.is_none()",
            "!output_guard.restart_required()",
        ),
    )
    require_order(
        "scheduler",
        behavior_items[
            "composite_recovered_completion_capacity_unavailable_claims_no_ready_sign"
        ],
        "composite recovered Completion capacity-unavailable behavior",
        (
            "planner_io.saturate_consensus_prefix(&services)",
            "let before = owner.recovered_broadcast_scheduler_state_for_test(broadcast)",
            "ProductionRecoveredCompletionDispatchV1::CapacityUnavailable",
            "owner.recovered_broadcast_scheduler_state_for_test(broadcast)",
            "before.records[&paired].state",
            "LifecycleState::Ready",
            "before.records[&unrelated].state",
            "LifecycleState::Ready",
            "!output_guard.restart_required()",
        ),
    )
    require_order(
        "worker",
        behavior_items[
            "recovered_completion_capacity_census_selects_once_and_drops_fail_stop"
        ],
        "composite recovered Completion worker Fetch ownership behavior",
        (
            "RecoveredCompletionCapacityProbeV1::Fetch",
            "fetch_census.select_fetch(13)",
            "returned_owner.dispatch_key()",
            "output.abort_before_claim()",
            "!output_guard.restart_required()",
        ),
    )
    startup_source = sources["startup_test"]
    for token in (
        "an exact ordinary winner cannot return the unchanged cursor",
        "an ordinary head cannot be poisoned by a later response family",
        "consume_prepared_ordinary_ingress_turn",
        "invalid-signature response is a drainable ordinary winner",
        "current certified Serve rejection must own ingress",
        "backpressured certified Serve remains lifecycle-owned",
        "released auxiliary capacity must admit exact Serve",
        "ProductionPreparedCertifiedServeTestSettlementV1::Rejected(reason)",
        "consume exact admitted Serve handoff",
        "drain_lifecycle_v2_ingress(",
        "drain one exact lifecycle-owned ordinary batch",
    ):
        if token not in startup_source:
            errors.append(
                f"{paths['startup_test']}: real-cursor ordinary ingress "
                f"regression omits {token!r}"
            )
    if ".drive_ingress_turn(" in sources["runner"]:
        errors.append(
            f"{paths['runner']}: run_inner must enter the lifecycle child instead "
            "of bypassing its owner through a direct ingress-driver call"
        )

    prepared_owner_start = sources["ordinary_consumer"].find(
        "pub(in crate::sumeragi) struct PreparedDequeuedV2IngressV1"
    )
    prepared_owner_end = sources["ordinary_consumer"].find(
        "/// Non-permit fail-stop scope", prepared_owner_start
    )
    prepared_owner = (
        sources["ordinary_consumer"][prepared_owner_start:prepared_owner_end]
        if prepared_owner_start >= 0 and prepared_owner_end > prepared_owner_start
        else ""
    )
    for required in (
        "ingress: Arc<FairV2Ingress>",
        "inbound: Option<InboundBlockMessage>",
        "disposition: FairV2IngressDequeueDisposition",
        "prepared_serve: Option<ProductionPreparedCertifiedServeV1>",
        "terminal_subject: Option<wire::BlockSubject>",
        "output_guard: Arc<ConsensusOutputGuard>",
        "armed: bool",
        "impl Drop for PreparedDequeuedV2IngressV1",
        "self.output_guard.close_admission_for_restart()",
    ):
        if required not in prepared_owner:
            errors.append(
                f"{paths['ordinary_consumer']}: opaque already-dequeued ordinary "
                f"owner omits {required!r}"
            )
    for forbidden in (
        "derive(Clone)",
        "derive(Copy)",
        "pub ingress:",
        "pub inbound:",
        "pub disposition:",
        "pub prepared_serve:",
        "pub terminal_subject:",
        "pub output_guard:",
        "pub armed:",
        "fn into_parts(",
        "fn inbound(",
        "fn prepared_serve(",
    ):
        if forbidden in prepared_owner:
            errors.append(
                f"{paths['ordinary_consumer']}: opaque already-dequeued ordinary "
                f"owner exposes forbidden surface {forbidden!r}"
            )

    consumer_fail_stop_start = sources["ordinary_consumer"].find(
        "struct PreparedDequeuedV2IngressFailStopScopeV1"
    )
    consumer_fail_stop_end = sources["ordinary_consumer"].find(
        "/// Settle a prepared Serve", consumer_fail_stop_start
    )
    consumer_fail_stop = (
        sources["ordinary_consumer"][consumer_fail_stop_start:consumer_fail_stop_end]
        if consumer_fail_stop_start >= 0
        and consumer_fail_stop_end > consumer_fail_stop_start
        else ""
    )
    for required in (
        "output_guard: Arc<ConsensusOutputGuard>",
        "armed: bool",
        "impl Drop for PreparedDequeuedV2IngressFailStopScopeV1",
        "if self.armed",
        "self.output_guard.close_admission_for_restart()",
    ):
        if required not in consumer_fail_stop:
            errors.append(
                f"{paths['ordinary_consumer']}: ordinary runner-tail non-permit "
                f"fail-stop scope omits {required!r}"
            )
    if "ConsensusFailStopOperation" in consumer_fail_stop:
        errors.append(
            f"{paths['ordinary_consumer']}: ordinary runner-tail fail-stop scope "
            "must not retain an output read permit across nested service work"
        )

    ordinary_consumer = item(
        "ordinary_consumer", "consume_prepared_dequeued_v2_ingress"
    )
    require_order(
        "ordinary_consumer",
        ordinary_consumer,
        "single exact ordinary post-dequeue runner tail",
        (
            "prepared.matches_output_guard(&services_output_guard)",
            "prepared.matches_ingress(receiver)",
            "let initial_admission = services_output_guard.acquire()",
            "drop(initial_admission)",
            "let mut inbound = prepared.inbound.take()",
            "let mut prepared_serve = prepared.prepared_serve.take()",
            "PreparedDequeuedV2IngressFailStopScopeV1::new",
            "let final_admission = services_output_guard.acquire()",
            "fail_stop.complete()",
            "prepared.complete()",
            "drop(final_admission)",
        ),
    )
    require_tokens(
        "ordinary_consumer",
        ordinary_consumer,
        "single exact ordinary post-dequeue runner tail",
        (
            "BlockMessage::KuraReplicaAdvert(_)",
            "inbound.message().is_lane_local()",
            "FairV2IngressDequeueDisposition::RetireObsolete",
            "wire::ConsensusMessageV2Payload::Proposal(proposal)",
            "wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request)",
            "ProductionPreparedCertifiedServeV1::Admitted(admission)",
            "wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response)",
            "ProductionPreparedOrdinaryIngressConsumptionV1::StopBatch",
            "wire::ConsensusMessageV2Payload::CommitCertificateRequest(request)",
            "wire::ConsensusMessageV2Payload::CommitCertificateResponse(response)",
        ),
    )
    if ordinary_consumer is not None:
        ordinary_tokens = rust_code_tokens(ordinary_consumer.source)
        for token in (
            "ProductionPreparedCertifiedServeV1::Rejected(reason)",
            "ProductionPreparedCertifiedServeV1::Service(reason)",
        ):
            observed = _token_sequence_count(ordinary_tokens, rust_code_tokens(token))
            if observed != 2:
                errors.append(
                    f"{paths['ordinary_consumer']}:{ordinary_consumer.line}: single "
                    f"exact ordinary post-dequeue runner tail must contain {token!r} "
                    f"exactly twice; found {observed}"
                )
        for forbidden in ("FnOnce", "callback", "into_parts("):
            if forbidden in ordinary_consumer.source:
                errors.append(
                    f"{paths['ordinary_consumer']}:{ordinary_consumer.line}: "
                    f"ordinary runner tail exposes forbidden seam {forbidden!r}"
                )

    legacy_drain = item("runner", "drain_v2_ingress")
    require_order(
        "runner",
        legacy_drain,
        "legacy and lifecycle ordinary ingress share one post-dequeue tail",
        (
            "PreparedDequeuedV2IngressV1::new(",
            "consume_prepared_dequeued_v2_ingress(",
            "ProductionPreparedOrdinaryIngressConsumptionV1::Continue",
            "ProductionPreparedOrdinaryIngressConsumptionV1::StopBatch",
        ),
    )
    lifecycle_consumer = item("driver", "consume_prepared_ordinary_ingress_turn")
    require_order(
        "driver",
        lifecycle_consumer,
        "activated lifecycle ordinary ingress shares the runner tail",
        (
            "turn.handoff.take()",
            "self.launched.close_output_for_restart()",
            "let LaunchedProductionLifecycleV1 { executor, services, leader_wire_ingress_binding, .. }",
            "consume_prepared_dequeued_v2_ingress(",
        ),
    )
    if lifecycle_consumer is not None:
        for forbidden in ("FnOnce", "callback", "into_parts("):
            if forbidden in lifecycle_consumer.source:
                errors.append(
                    f"{paths['driver']}:{lifecycle_consumer.line}: activated ordinary "
                    f"consumer exposes forbidden seam {forbidden!r}"
                )

    lifecycle_height_driver = item("height_driver", "drain_lifecycle_v2_ingress")
    require_order(
        "height_driver",
        lifecycle_height_driver,
        "activated lifecycle ordinary Completion/Runtime/Ingress batch",
        (
            "executor.has_retained_certified_body_response()",
            "services.lifecycle_output_guard()",
            "outer_ingress_turns(limit, context_id, height)",
            "LifecycleRunnerRankTarget::Completion",
            "services.certified_serve_barrier_request_hash()",
            "activated.drive_completion_turn(current_turn, lane_work)",
            "services.drain_completions(executor)?",
            "LifecycleRunnerRankTarget::Runtime",
            "services.certified_serve_barrier()",
            "advance_executor(receiver, executor, services, 1)?",
            "LifecycleRunnerRankTarget::Ingress",
            "activated.drive_ingress_turn(current_turn)",
            "activated.consume_prepared_ordinary_ingress_turn(",
        ),
    )
    require_tokens(
        "height_driver",
        lifecycle_height_driver,
        "activated lifecycle ordinary batch selected outcomes",
        (
            "ProductionLifecycleCompletionTurnV1::PassThrough(ordinary_turn,)",
            "ProductionLifecycleCompletionTurnV1::Selected(_selected,)",
            "ProductionPreparedOrdinaryIngressConsumptionV1::Continue",
            "ProductionPreparedOrdinaryIngressConsumptionV1::StopBatch",
            "ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchQueued",
            "ProductionLifecycleIngressSelectionV1::CapacityPending",
            "ProductionLifecycleIngressSelectionV1::Retry",
            "ProductionLifecycleIngressSelectionV1::OrdinaryRetained",
            "ProductionLifecycleIngressSelectionV1::RestartRequired",
        ),
    )
    if lifecycle_height_driver is not None:
        height_driver_tokens = rust_code_tokens(lifecycle_height_driver.source)
        for forbidden in (
            "output_guard: &Arc<ConsensusOutputGuard>",
            "drain_v2_ingress(",
            "V2IngressDrainMode",
        ):
            if _token_sequence_count(height_driver_tokens, rust_code_tokens(forbidden)):
                errors.append(
                    f"{paths['height_driver']}:{lifecycle_height_driver.line}: "
                    "activated lifecycle ordinary batch exposes obsolete or "
                    f"caller-substitutable surface {forbidden!r}"
                )
    if sources["lifecycle_run_inner"].count("drain_lifecycle_v2_ingress(") != 2:
        errors.append(
            f"{paths['lifecycle_run_inner']}: activated lifecycle loop must route "
            "exactly its Serve-barrier and main ordinary batches through the "
            "shared lifecycle height driver"
        )

    preactivation_start = sources["runner"].find(
        "pub(in crate::sumeragi) struct ProductionLifecyclePreActivationRunnerBorrowV1"
    )
    preactivation_end = sources["runner"].find(
        "/// Exact reducer facts which own one local proposal-side work item.",
        preactivation_start,
    )
    preactivation_region = (
        sources["runner"][preactivation_start:preactivation_end]
        if preactivation_start >= 0 and preactivation_end > preactivation_start
        else ""
    )
    for required in (
        "_seal: ProductionLifecyclePreActivationRunnerBorrowSealV1",
        "local_proposal: Option<ProductionLifecycleLocalProposalStateV1>",
        "struct ProductionLifecyclePreActivationRunnerBorrowSealV1;",
        "impl Drop for ProductionLifecyclePreActivationRunnerBorrowSealV1",
        "fn mint_for_recovered_runner() -> Self",
        "local_proposal: Some(ProductionLifecycleLocalProposalStateV1::fresh())",
        "#[cfg(test)]",
        "pub(in crate::sumeragi) fn for_test() -> Self",
        "fn bind_recovered_local_proposal(",
        "let Some(local_proposal) = self.local_proposal.as_mut()",
        "if !local_proposal.state.is_pristine()",
        "LocalProposalState::from_recovered_lifecycle_attempt(true, directive)",
        "fn local_proposal_state_is_pristine(",
        "fn prepared_local_proposal_exactly_matches(",
        "fn prepared_local_proposal_mut(",
        "self.local_proposal.as_mut()",
    ):
        if required not in preactivation_region:
            errors.append(
                f"{paths['runner']}: sealed lifecycle preactivation runner borrow "
                f"omits {required!r}"
            )
    for forbidden in (
        "derive(Clone)",
        "derive(Copy)",
        "pub _seal:",
        "pub(crate) _seal:",
        "pub(in crate::sumeragi) _seal:",
        "pub local_proposal:",
        "pub(crate) local_proposal:",
        "pub(in crate::sumeragi) local_proposal:",
        "pub(in crate::sumeragi) fn mint_for_recovered_runner",
        "fn into_parts(",
    ):
        if forbidden in preactivation_region:
            errors.append(
                f"{paths['runner']}: sealed lifecycle preactivation runner borrow "
                f"exposes forbidden surface {forbidden!r}"
            )

    proposal_state_start = sources["runner"].find(
        "pub(in crate::sumeragi) struct ProductionLifecycleLocalProposalStateV1"
    )
    proposal_state_end = sources["runner"].find(
        "/// Run the v2-only worker until shutdown", proposal_state_start
    )
    proposal_state_region = (
        sources["runner"][proposal_state_start:proposal_state_end]
        if proposal_state_start >= 0 and proposal_state_end > proposal_state_start
        else ""
    )
    for required in (
        "state: LocalProposalState",
        "fn fresh() -> Self",
        "fn already_attempted(",
    ):
        if required not in proposal_state_region:
            errors.append(
                f"{paths['runner']}: opaque lifecycle local-Proposal state "
                f"omits {required!r}"
            )

    prepared_state_start = sources["launch"].find(
        "pub(in crate::sumeragi) struct ProductionLifecyclePreparedLocalProposalStateV1"
    )
    prepared_state_end = sources["launch"].find(
        "/// Opaque lifecycle stack after clocks", prepared_state_start
    )
    prepared_state_region = (
        sources["launch"][prepared_state_start:prepared_state_end]
        if prepared_state_start >= 0 and prepared_state_end > prepared_state_start
        else ""
    )
    for required in (
        "runner: super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1",
        "context_id: wire::HeightContextId",
        "directive: super::super::v2::LocalProposalDirective",
        "fn exactly_matches(",
        "self.context_id == context_id",
        "self.directive == directive",
        "prepared_local_proposal_exactly_matches(directive)",
    ):
        if required not in prepared_state_region:
            errors.append(
                f"{paths['launch']}: affine prepared local-Proposal state omits {required!r}"
            )
    for forbidden in (
        "derive(Clone)",
        "derive(Copy)",
        "pub runner:",
        "pub context_id:",
        "pub directive:",
        "fn into_parts(",
    ):
        if forbidden in prepared_state_region:
            errors.append(
                f"{paths['launch']}: affine prepared local-Proposal state exposes "
                f"forbidden surface {forbidden!r}"
            )
    prepared_state_behavior = item(
        "launch_tests", "prepared_local_proposal_state_is_affine_and_context_directive_bound"
    )
    require_order(
        "launch_tests",
        prepared_state_behavior,
        "affine prepared local-Proposal state behavior",
        (
            "prepared.exactly_matches(context_id, directive)",
            "!prepared.exactly_matches(foreign_context, directive)",
            "!prepared.exactly_matches(context_id, foreign_directive)",
        ),
    )
    for forbidden in ("pub state:", "fn into_parts(", "derive(Clone)", "derive(Copy)"):
        if forbidden in proposal_state_region:
            errors.append(
                f"{paths['runner']}: opaque lifecycle local-Proposal state "
                f"exposes forbidden surface {forbidden!r}"
            )
    live_proposal_behavior = item(
        "startup_test", "production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout"
    )
    require_order(
        "launch_tests",
        live_proposal_behavior,
        "activated lifecycle retains the exact runner local-Proposal owner",
        (
            "activated.with_runner_runtime(",
            "services.matches_lifecycle_executor_output_guard(executor)",
            "assert!(local_proposal.already_attempted(directive))",
        ),
    )

    runtime_clock = item("runtime", "lifecycle_live_clocks_are_armed")
    require_tokens(
        "runtime",
        runtime_clock,
        "preactivation live-clock state oracle",
        ("self.clocks_armed",),
    )
    effects_clock = item("effects", "lifecycle_live_clocks_are_unarmed")
    require_tokens(
        "effects",
        effects_clock,
        "preactivation executor live-clock state oracle",
        ("!self.runtime.lifecycle_live_clocks_are_armed()",),
    )
    fail_stop_start = sources["preactivation"].find(
        "struct ProductionLifecyclePreActivationFailStopScopeV1"
    )
    fail_stop_end = sources["preactivation"].find(
        "impl LaunchedProductionLifecycleV1", fail_stop_start
    )
    fail_stop_region = (
        sources["preactivation"][fail_stop_start:fail_stop_end]
        if fail_stop_start >= 0 and fail_stop_end > fail_stop_start
        else ""
    )
    for required in (
        "output_guard: Arc<ConsensusOutputGuard>",
        "armed: bool",
        "impl Drop for ProductionLifecyclePreActivationFailStopScopeV1",
        "self.output_guard.close_admission_for_restart()",
    ):
        if required not in fail_stop_region:
            errors.append(
                f"{paths['preactivation']}: lifecycle preactivation non-permit fail-stop "
                f"scope omits {required!r}"
            )
    if "ConsensusFailStopOperation" in fail_stop_region:
        errors.append(
            f"{paths['preactivation']}: lifecycle preactivation fail-stop scope must not "
            "hold an output read permit across nested setup"
        )
    setup = item("preactivation", "with_runner_setup_transaction")
    require_order(
        "preactivation",
        setup,
        "fail-stop closed-ingress lifecycle preactivation setup",
        (
            "let output_guard = self.services.lifecycle_output_guard()",
            "let initial_admission = output_guard.acquire()",
            "ProductionLifecyclePreActivationFailStopScopeV1::new",
            "drop(initial_admission)",
            "matches_lifecycle_executor_output_guard(&self.executor)",
            "self.leader_wire_ingress_binding.ingress.state.lock().open",
            "self.completion_observer_activation.is_none()",
            "self.executor.lifecycle_live_clocks_are_unarmed()",
            "operation(&mut self.executor, &mut self.services)?",
            "matches_lifecycle_executor_output_guard(&self.executor)",
            "self.leader_wire_ingress_binding.ingress.state.lock().open",
            "self.completion_observer_activation.is_none()",
            "self.executor.lifecycle_live_clocks_are_unarmed()",
            "let final_admission = output_guard.acquire()",
            "setup.complete()",
            "drop(final_admission)",
        ),
    )
    if setup is not None:
        setup_tokens = rust_code_tokens(setup.source)
        for token, expected in (
            ("ProductionLifecyclePreActivationErrorV1::OutputClosed", 2),
            ("ProductionLifecyclePreActivationErrorV1::OwnershipMismatch", 2),
            ("ProductionLifecyclePreActivationErrorV1::IngressAlreadyOpen", 2),
            ("ProductionLifecyclePreActivationErrorV1::CompletionObserverMissing", 2),
            ("ProductionLifecyclePreActivationErrorV1::ClocksAlreadyArmed", 2),
        ):
            observed = _token_sequence_count(setup_tokens, rust_code_tokens(token))
            if observed != expected:
                errors.append(
                    f"{paths['preactivation']}:{setup.line}: fail-stop closed-ingress "
                    f"lifecycle preactivation setup must retain {token!r} exactly "
                    f"{expected} time(s); found {observed}"
                )
        for forbidden in (
            "&mut self.owner",
            "ProductionLifecyclePreActivationRunnerBorrowV1",
            "bind_recovered_local_proposal",
            "begin_fail_stop_operation(",
            "arm_live_clocks(",
            "activate_effect_completion_observer(",
            "open_and_publish(",
            "into_parts(",
        ):
            if forbidden in setup.source:
                errors.append(
                    f"{paths['preactivation']}:{setup.line}: preactivation setup exposes "
                    f"forbidden transition {forbidden!r}"
                )
    public_setup = item("preactivation", "with_runner_setup")
    require_tokens(
        "preactivation",
        public_setup,
        "public preactivation runner aperture",
        ("self.with_runner_setup_transaction(operation)",),
    )
    if public_setup is not None:
        for forbidden in (
            "bind_recovered_local_proposal",
            "operation(&mut self.executor",
        ):
            if forbidden in public_setup.source:
                errors.append(
                    f"{paths['preactivation']}:{public_setup.line}: public setup aperture "
                    f"exposes forbidden Proposal mutation {forbidden!r}"
                )
    fail_stop_behavior = item(
        "launch_tests",
        "preactivation_fail_stop_scope_closes_on_drop_and_disarms_on_complete",
    )
    require_order(
        "startup_test",
        fail_stop_behavior,
        "preactivation non-permit fail-stop behavior",
        (
            "ProductionLifecyclePreActivationFailStopScopeV1::new( Arc::clone(&dropped_guard), )",
            "dropped_guard.restart_required()",
            "ProductionLifecyclePreActivationFailStopScopeV1::new(Arc::clone(&completed_guard)) .complete()",
            "!completed_guard.restart_required()",
        ),
    )
    require_tokens(
        "startup_test",
        fail_stop_behavior,
        "preactivation non-permit fail-stop behavior",
        (
            "assert!(dropped_guard.restart_required())",
            "assert!(!completed_guard.restart_required())",
        ),
    )

    run_inner = item("runner", "run_inner")
    require_order(
        "runner",
        run_inner,
        "PendingKura and ordinary heights split into sealed lifecycle loops",
        (
            "let pending_kura_apply = recovered.pending_kura_apply()",
            "match pending_kura_apply",
            "None => lifecycle_run_inner::run_non_pending_lifecycle_loop(",
            "Some(pending) => lifecycle_pending_kura::run_pending_kura_lifecycle_height(",
        ),
    )
    lifecycle_loop = item("lifecycle_run_inner", "run_non_pending_lifecycle_loop")
    require_order(
        "lifecycle_run_inner",
        lifecycle_loop,
        "sealed non-Pending lifecycle startup and activation",
        (
            "V2BodyStore::open_with_policy(",
            ".into_quarantined_recovered_startup()",
            "SumeragiV2Adapter::open_recovered_startup_with_capacity_geometry(",
            ".authenticate_final_wal_startup_authority()",
            "bind_production_lifecycle_owner_factory_inputs_v1(",
            "open_production_lifecycle_owner_v1(",
            "launch_non_pending_lifecycle_height(",
            "ProductionLifecyclePreActivationRunnerBorrowV1::mint_for_recovered_runner()",
            "recover_canonical_bodies_before_activation(",
            "initialize_recovered_local_proposal(setup_runner)",
            "let height_started_at = Instant::now()",
            "preactivation.activate(height_started_at, local_proposal)",
            "run_lifecycle_active_height(",
        ),
    )
    lifecycle_active = item("lifecycle_run_inner", "run_lifecycle_active_height")
    require_order(
        "lifecycle_run_inner",
        lifecycle_active,
        "lifecycle live-height finalization and successor storage handoff",
        (
            "drain_lifecycle_v2_ingress(",
            "finalize_lifecycle_height(",
            "Some(certified_serve_producer_episode)",
            "DurableV2PredecessorIdentity::authenticate(artifact, receipt)",
            "build_verified_successor(",
            "into_parts_with_lifecycle_storage_authority(",
        ),
    )
    lifecycle_finalization = item("lifecycle_run_inner", "finalize_lifecycle_height")
    require_order(
        "lifecycle_run_inner",
        lifecycle_finalization,
        "lifecycle finalization output/store/cleanup transaction",
        (
            "activated.into_finalized_rollover(active_runner)",
            "drop(producer_episode)",
            "finalized.finality()",
            "prepare_successor(receipt, artifact, &mut lane_work)",
            "finalized.rollover_outputs(",
            "post_output.retire_lifecycle_stores()",
            "cleanup_ready.finish_cleanup(Duration::ZERO, cleanup_supervisor)",
        ),
    )
    require_order(
        "lifecycle_run_inner",
        lifecycle_active,
        "certified-Serve producer episode spans the complete live runner turn",
        (
            "let certified_serve_barrier = services.certified_serve_barrier()",
            "service_retained_certified_response(",
            "service_certified_serve_barrier( certified_serve_barrier,",
            "let certified_serve_producer_episode = activated.with_runner_runtime(",
            "let ready_to_finish = activated.with_runner_runtime(",
            "if ready_to_finish",
            "finalize_lifecycle_height(",
            "Some(certified_serve_producer_episode)",
            "schedule_local_proposal(",
        ),
    )
    if lifecycle_active is not None and "V2IngressDrainMode::Ordinary" in lifecycle_active.source:
        errors.append(
            f"{paths['lifecycle_run_inner']}:{lifecycle_active.line}: lifecycle "
            "live height must not bypass owner settlement through the legacy "
            "ordinary ingress mode"
        )
    startup_setup = item(
        "startup_test",
        "production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies",
    )
    require_order(
        "startup_test",
        startup_setup,
        "production-shaped closed-ingress preactivation setup behavior",
        (
            "ProductionLifecyclePreActivationRunnerBorrowV1::for_test()",
            ".with_runner_setup(&mut setup_runner",
            "services.matches_lifecycle_executor_output_guard(executor)",
            "executor.current_tag()",
            "setup_tag.height()",
            "!leader_wire_ingress.state.lock().open",
        ),
    )

    proposal_type_start = sources["adapter"].find(
        "pub(in crate::sumeragi) struct RecoveredLifecycleLocalProposalAttemptV1"
    )
    proposal_type_end = sources["adapter"].find(
        "/// Adapter and residual replay effects retained", proposal_type_start
    )
    proposal_type = (
        sources["adapter"][proposal_type_start:proposal_type_end]
        if proposal_type_start >= 0 and proposal_type_end > proposal_type_start
        else ""
    )
    for required in (
        "tag: reducer::EventTag",
        "round: wire::ConsensusRound",
        "subject: wire::BlockSubject",
        "fn from_control(control: &RecoveredWalControlSign) -> Option<Self>",
        "request: SignRequest::Proposal(proposal)",
        "fn exactly_matches_directive(",
        "self.tag == current.tag()",
        "current.decided_subject().is_none()",
        ".locked_body()",
    ):
        if required not in proposal_type:
            errors.append(
                f"{paths['adapter']}: opaque recovered local-Proposal owner "
                f"omits {required!r}"
            )
    for forbidden in (
        "derive(Clone)",
        "derive(Copy)",
        "pub tag:",
        "pub round:",
        "pub subject:",
        "fn into_parts(",
        "fn tag(",
        "fn round(",
        "fn subject(",
        "fn effect(",
    ):
        if forbidden in proposal_type:
            errors.append(
                f"{paths['adapter']}: opaque recovered local-Proposal owner "
                f"exposes forbidden surface {forbidden!r}"
            )

    proposal_factory = item(
        "adapter", "open_production_lifecycle_owner_v1_at_authenticated_roots"
    )
    require_order(
        "adapter",
        proposal_factory,
        "recovered local-Proposal owner factory handoff",
        (
            "RecoveredWalStartupAuthorityV1::ControlSign(control)",
            "RecoveredLifecycleLocalProposalAttemptV1::from_control(&control)",
            "project_recovered_wal_control_sign(&verified, control,)",
            "PreparedRecoveredWalStartupAuthorityV1::ControlSign { projection: projected, local_proposal_attempt, }",
            "PreparedRecoveredWalStartupAuthorityV1::ControlSign { projection: control, local_proposal_attempt, }",
            "ProductionLifecycleAdapterStartupV1::recovered_with_local_proposal_attempt( adapter, effects, local_proposal_attempt, )",
        ),
    )
    proposal_runtime = item("pending_kura", "into_serialized_runtime")
    require_order(
        "pending_kura",
        proposal_runtime,
        "recovered local-Proposal runtime ownership handoff",
        (
            "local_proposal_attempt",
            "pending_kura_apply.is_none() || local_proposal_attempt.is_none()",
            "Ok((runtime, replay, local_proposal_attempt))",
        ),
    )
    proposal_initialize = item(
        "preactivation", "initialize_recovered_local_proposal"
    )
    require_order(
        "preactivation",
        proposal_initialize,
        "closed-ingress recovered local-Proposal initialization",
        (
            "self.recovered_local_proposal_attempt.take()",
            "self.with_runner_setup_transaction(",
            "executor.local_proposal_directive()",
            "recovered.exactly_matches_directive(directive)",
            "runner.bind_recovered_local_proposal(directive)",
            "ProductionLifecyclePreActivationErrorV1::RunnerProposalStateNotPristine",
            "ProductionLifecyclePreActivationErrorV1::RecoveredProposalMismatch",
            "ProductionLifecyclePreparedLocalProposalStateV1 { runner, context_id, directive, }",
            "Ok((directive, prepared))",
        ),
    )
    if proposal_initialize is not None:
        for forbidden in (
            "into_parts(",
            "recovered.tag",
            "recovered.round",
            "recovered.subject",
            "AdapterEffect",
        ):
            if forbidden in proposal_initialize.source:
                errors.append(
                    f"{paths['preactivation']}:{proposal_initialize.line}: recovered "
                    f"local-Proposal initialization exposes forbidden seam {forbidden!r}"
                )
    proposal_bind_call = "runner.bind_recovered_local_proposal(directive)"
    proposal_bind_count = sources["preactivation"].count(proposal_bind_call)
    if proposal_bind_count != 1:
        errors.append(
            f"{paths['preactivation']}: only the WAL-authenticated initializer may "
            f"bind runner local-Proposal state; found {proposal_bind_count} calls"
        )

    complete_tip_proposal_initialize = item(
        "ledger", "initialize_recovered_local_proposal"
    )
    require_order(
        "ledger",
        complete_tip_proposal_initialize,
        "CompleteTip recovered local-Proposal initialization delegation",
        (
            "runner: super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1",
            "self.launched.initialize_recovered_local_proposal(runner)",
        ),
    )

    proposal_activation_blocker = item(
        "launch", "lifecycle_activation_recovery_blocker"
    )
    require_order(
        "launch",
        proposal_activation_blocker,
        "ordinary activation recovery preflight",
        (
            "pending_kura_replay || pending_kura_evidence",
            "ProductionLifecycleActivationErrorV1::PendingKuraApply",
            "else if recovered_local_proposal",
            "ProductionLifecycleActivationErrorV1::LocalProposalReplayUninitialized",
            "None",
        ),
    )
    activation = item("launch", "activate_with")
    require_order(
        "launch",
        activation,
        "ordinary activation rejects incomplete recovered local-Proposal setup",
        (
            "lifecycle_activation_recovery_blocker(",
            "self.pending_kura_apply_replay.is_some()",
            "self.executor.pending_kura_apply_recovery_evidence().is_some()",
            "self.recovered_local_proposal_attempt.is_some()",
            "close_admission_for_restart()",
            "return Err(error)",
            "self.executor.local_proposal_directive()",
            "local_proposal.exactly_matches( self.executor.context().id(), current_directive )",
            "ProductionLifecycleActivationErrorV1::LocalProposalPreparationMismatch",
            "let clock_activation = ProductionLifecycleLiveClockActivationPermitV1",
            "self.executor.arm_live_clocks(clock_activation, now)",
        ),
    )
    clock_permit_start = sources["launch"].find(
        "pub(in crate::sumeragi) struct ProductionLifecycleLiveClockActivationPermitV1"
    )
    clock_permit_end = sources["launch"].find(
        "/// Move-only authority for refreshing the live Certified-Serve retirement cut.",
        clock_permit_start,
    )
    clock_permit = (
        sources["launch"][clock_permit_start:clock_permit_end]
        if clock_permit_start >= 0 and clock_permit_end > clock_permit_start
        else ""
    )
    for required in (
        "_seal: ProductionLifecycleLiveClockActivationPermitSealV1",
        "struct ProductionLifecycleLiveClockActivationPermitSealV1;",
        "impl Drop for ProductionLifecycleLiveClockActivationPermitSealV1",
        "#[cfg(test)]",
        "pub(in crate::sumeragi) fn for_test() -> Self",
    ):
        if required not in clock_permit:
            errors.append(
                f"{paths['launch']}: ordinary live-clock permit omits {required!r}"
            )
    for forbidden in (
        "derive(Clone)",
        "derive(Copy)",
        "pub _seal:",
        "pub(crate) _seal:",
        "pub(in crate::sumeragi) _seal:",
    ):
        if forbidden in clock_permit:
            errors.append(
                f"{paths['launch']}: ordinary live-clock permit exposes {forbidden!r}"
            )
    clock_arm = item("effects", "arm_live_clocks")
    require_order(
        "effects",
        clock_arm,
        "affine ordinary live-clock arming",
        (
            "_permit: ProductionLifecycleLiveClockActivationPermitV1",
            "if self.pending_tip_recovery.is_some()",
            "return Err(RuntimeClockError::PendingKuraRecovery)",
            "self.runtime.arm_live_clocks(now)",
        ),
    )
    pending_status = item("effects", "pending_kura_activation_status_snapshot")
    require_order(
        "effects",
        pending_status,
        "completed pending-Kura no-clock status snapshot",
        (
            "self.ready_to_finish()",
            "self.lifecycle_live_clocks_are_unarmed()",
            "PendingKuraApplyRecoveryStage::Completed",
            "return Err(AdapterError::PendingKuraActivationNotReady)",
            "self.runtime.pending_kura_activation_status_snapshot()",
        ),
    )
    proposal_behavior = item(
        "startup_test", "production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout"
    )
    require_order(
        "startup_test",
        proposal_behavior,
        "production-shaped recovered local-Proposal initialization behavior",
        (
            "RecoveredLifecycleLocalProposalAttemptV1::for_test(",
            "retain_recovered_local_proposal_attempt_for_test(recovered_attempt)",
            "initialize_recovered_local_proposal(setup_runner)",
            "local_proposal_state.already_attempted(directive)",
            ".activate(Instant::now(), activation, local_proposal_state)",
        ),
    )

    pending_source = sources["pending_kura"]
    pending_types_start = pending_source.find(
        "pub(crate) struct PendingKuraRecoveredAdapterStartupV1"
    )
    pending_types_end = pending_source.find(
        "impl InstalledPendingKuraApplyV1", pending_types_start
    )
    pending_types = (
        pending_source[pending_types_start:pending_types_end]
        if pending_types_start >= 0 and pending_types_end > pending_types_start
        else ""
    )
    for required in (
        "startup: RecoveredAdapterStartup",
        "expected: crate::sumeragi::v2_recovery::PendingKuraApply",
        "startup: AuthenticatedRecoveredAdapterStartup",
        "replay: RecoveredPendingKuraApplyReplayV1",
        "fetch: RecoveredWalDecisionFetch",
        "verified: VerifiedHeightContext",
        "wal_identity: RecoveredWalFrameIdentity",
        "replay_evidence: RecoveredWalDecisionFetchReplayEvidenceV1",
        "effect: AdapterEffect",
        "pub(in crate::sumeragi) struct InstalledPendingKuraApplyV1",
        "genesis: Option<crate::sumeragi::v2_effects::VerifiedPendingGenesisNexusAmxContext>",
    ):
        if required not in pending_types:
            errors.append(
                f"{paths['pending_kura']}: opaque pending-Kura replay types "
                f"omit {required!r}"
            )
    for forbidden in (
        "derive(Clone)",
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
        if forbidden in pending_types:
            errors.append(
                f"{paths['pending_kura']}: opaque pending-Kura replay types "
                f"expose forbidden surface {forbidden!r}"
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
        "pending-Kura exact Decision-Fetch authentication",
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
        "pending-Kura runtime sidecar ownership",
        (
            "let RecoveredPendingKuraApplyReplayV1 { expected, fetch } = replay",
            "let RecoveredWalDecisionFetch { wal_identity, replay_evidence, effect, } = fetch",
            "replay_evidence.exactly_matches_recovered_decision_fetch",
            "vec![effect]",
            "SerializedV2Runtime::new_with_lifecycle_ordinals(",
            "returned_effects.len() == 1",
            "replay_evidence.exactly_matches_recovered_decision_fetch",
            "PreparedRecoveredPendingKuraApplyReplayV1 { expected, verified, wal_identity, replay_evidence, effect, }",
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
    pending_install = item("pending_kura", "install")
    require_order(
        "pending_kura",
        pending_install,
        "pending-Kura verification-before-dispatch install",
        (
            "executor.context() != verified.context()",
            "replay_evidence.exactly_matches_recovered_decision_fetch",
            "let effects = vec![effect]",
            "executor.verify_pending_kura_apply_replay(expected, &effects)?",
            "executor.consume_pending_tip_recovery_effects(effects, services)?",
        ),
    )
    pending_owner = item("coordinator_support", "with_pending_kura_apply_replay")
    require_order(
        "coordinator_support",
        pending_owner,
        "storage-only pending-Kura lifecycle owner retention",
        (
            "self.adapter_startup.take()",
            "startup.with_pending_kura_apply_replay(replay)",
        ),
    )
    pending_factory = item(
        "pending_kura", "open_production_lifecycle_owner_v1"
    )
    require_order(
        "pending_kura",
        pending_factory,
        "storage-only pending-Kura owner factory",
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
    pending_apply_turn = item("pending_lifecycle", "drive_apply_recovery_turn")
    require_order(
        "pending_lifecycle",
        pending_apply_turn,
        "closed-ingress pending-Kura Apply recovery turn",
        (
            "self.launched.pending_kura_apply_replay.is_some()",
            "close_admission_for_restart()",
            "self.launched.with_runner_setup(runner",
            "executor.pending_kura_apply_recovery_evidence()",
            "services.drain_completions(executor)?",
            "executor.step_pending_tip_recovery(Instant::now(), services)?",
            "PendingKuraApplyRecoveryStage::Completed",
            "executor.ready_to_finish()",
            "ProductionPendingKuraApplyRecoveryProgressV1::Completed",
        ),
    )
    reject_tokens(
        "pending_lifecycle",
        pending_apply_turn,
        "closed-ingress pending-Kura Apply recovery turn",
        (
            "arm_live_clocks(",
            "schedule_local_proposal(",
            "drive_ingress_turn(",
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
    pending_finalization = item("pending_lifecycle", "into_finalized_rollover")
    require_order(
        "pending_lifecycle",
        pending_finalization,
        "pending-Kura affine lane finalization",
        (
            "self.launched.executor.ready_to_finish()",
            "PendingKuraApplyRecoveryStage::Completed",
            "matches_installed_pending_kura_tip(self.installed.expected())",
            "matches_lifecycle_lane_work(&self.lane_work)",
            "exactly_covers_finalization_work(&self.launched.owner.coordinator)",
            "runner_activation.retire(&launched.leader_wire_ingress_binding.ingress)",
            "drop(installed)",
            "launched.leader_wire_ingress_binding.retire()",
            "executor.into_finalized_parts()",
            "finish_height(&receipt, &artifact)",
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
            "pending.drive_apply_recovery_turn(&mut setup_runner, control_queue_capacity)?",
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
            "executor.has_retained_certified_body_response()",
            "services.certified_serve_barrier()",
            "service_pending_certified_serve_barrier(",
            "services.try_begin_certified_serve_producer_episode()",
            "retry_exact_output_and_apply_sidecar_admissions(",
            "services.service_kura_replica_advert_refresh_turn(Instant::now())",
            "services.drain_completions(executor)?",
            "retry_exact_output_and_apply_sidecar_admissions(",
            "reconcile_executor_locked_body(executor, services)?",
            "drain_decided_lane_recovery_ingress(",
            "dispatch_lane_work_effects(lane_work, services, control_queue_capacity)?",
            "activated.into_finalized_rollover(&mut active_runner)?",
            "drop(certified_serve_producer_episode)",
            "finalized.finality()",
            "into_parts_with_lifecycle_storage_authority(",
            "finalized.rollover_outputs(",
            "post_output.retire_lifecycle_stores()?",
            "cleanup_ready.finish_cleanup(Duration::ZERO, cleanup_supervisor)",
        ),
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
    pending_barrier = item("pending_runner", "service_pending_certified_serve_barrier")
    require_order(
        "pending_runner",
        pending_barrier,
        "pending-Kura Serve barrier forbids pacemaker work",
        (
            "service_certified_serve_barrier_liveness_turn(true, |action| match action",
            "CertifiedServeBarrierLivenessAction::TimeoutRecoveryPrefix",
            "drain_timeout_recovery_prefix_completion(executor, timeout_recovery_cut)?",
            "CertifiedServeBarrierLivenessAction::TimeoutVoteEpisode",
            "CertifiedServeBarrierLivenessAction::Pacemaker",
            "output_guard.close_admission_for_restart()",
            "Err(V2RunnerError::Service(",
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
            ".drive_apply_recovery_turn(&mut setup_runner, 64)",
            "ProductionPendingKuraApplyRecoveryProgressV1::Completed",
            ".prepare_lane_recovery(",
            "pending_kura_lifecycle_fixture_for_test(",
            ".activate_no_clock(activation)",
            "executor.lifecycle_live_clocks_are_unarmed()",
            "if !finalize",
            ".into_clean_shutdown(&mut active_runner)",
            "services.try_begin_certified_serve_producer_episode()",
            ".into_finalized_rollover(&mut active_runner)",
            "drop(producer_episode)",
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
            "mismatched_pending",
            "AdapterError::RecoveredPendingKuraApplyMismatch",
            ".into_serialized_runtime(",
            "prepared.is_exact_for_test()",
            "runtime.take_effect_ownership(1)",
            "prepared.install(&mut executor, &mut services)",
            "EffectExecutorError::PendingApplyRecoveryMismatch(_)",
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
    return errors
