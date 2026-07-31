# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

# Exact intermediate theorem statements and reviewed proof dependencies keep
# the release wrappers connected to their advertised non-circular arguments.
# In particular, receipt subject equality must come from one-height
# DecisionAgreement, not from the write-once chain projection.
EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS = {
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AdequateLeaderFixedSelectedOwnerUsesExactAsyncFairness",
    ): (
        "\\A initialContext, owner: "
        "owner \\in AdequateLeaderFixedSelectedServiceOwnerSet(initialContext) "
        "=> AsyncSpecAt(initialContext) "
        "=> WF_AsyncAllVars( "
        "AdequateLeaderFixedSelectedServiceOwnerAction(owner))"
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AsyncLiveProvidesAdequateLeaderFixedSelectedOwnerFairness",
    ): (
        "\\A initialContext: "
        "AdequateLeaderFixedSelectedOwnerFairnessProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AdequateLeaderFixedGlobalSelectionAndPreCandidateServiceSupplyRawRouteStep",
    ): (
        "\\A specification: "
        "/\\ AdequateLeaderFixedGlobalBlockerSelectionClosureProperty( "
        "specification) "
        "/\\ AdequateLeaderFixedPreCandidateEntryServiceProperty(specification) "
        "=> AdequateLeaderFixedPreCandidateRawRouteRankStepProperty( "
        "specification)"
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AdequateLeaderFixedPreCandidateRawRouteStepClosesRank",
    ): (
        "\\A specification: "
        "AdequateLeaderFixedPreCandidateRawRouteRankStepProperty(specification) "
        "=> AdequateLeaderFixedPreCandidateRawRouteRankClosureProperty( "
        "specification)"
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AdequateLeaderFixedPipelineProducerHandoffStartsRawRouteRank",
    ): (
        "\\A sourceDormantPotential, knownDormantPotential: "
        "\\A initialContext, target, leaderContext, leader, leaderView, "
        "receipt, route, token, episodeTarget, sourceOccurrenceRank, "
        "sourceOccurrenceOwner, sourceCutoffOrdinal, known, sourceRank, "
        "budget: "
        "AdequateLeaderFixedPipelineProducerHandoffFrontier( "
        "initialContext, target, leaderContext, leader, leaderView, receipt, "
        "route, token, episodeTarget, sourceOccurrenceRank, "
        "sourceOccurrenceOwner, sourceCutoffOrdinal, sourceDormantPotential, "
        "knownDormantPotential, known, sourceRank, budget) "
        "=> AdequateLeaderFixedPreCandidateRawRouteRankFrontier( "
        "initialContext, target, leaderContext, leader, leaderView, receipt, "
        "sourceRank, sourceRank)"
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AdequateLeaderFixedCandidateFairnessAndRawRouteClosureSupplyEpisodeStep",
    ): (
        "\\A specification: "
        "/\\ AdequateLeaderFixedSelectedOwnerFairnessProperty(specification) "
        "/\\ "
        "AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepProviderProperty( "
        "specification) "
        "/\\ AdequateLeaderFixedPreCandidateRawRouteRankClosureProperty( "
        "specification) "
        "=> AdequateLeaderFixedPipelineOriginNonDescentEpisodeStepProperty( "
        "specification)"
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AsyncLiveProvidesAdequateLeaderFixedPipelineOriginNonDescentEpisodeStep",
    ): (
        "\\A initialContext: "
        "AdequateLeaderFixedPipelineOriginNonDescentEpisodeStepProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        (
            "AsyncLiveSpecSuppliesAdequateLeaderAuthorityDeadline"
            "FreshSelfQuantitativeProviderBundle"
        ),
    ): (
        "\\A initialContext: AsyncLiveSpecAt(initialContext) "
        "=> AdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AsyncLiveFreshSelfBundleSuppliesFixedDeadlineAndResponsiveDissemination",
    ): (
        "\\A initialContext: "
        "AdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle( "
        "AsyncLiveSpecAt(initialContext)) "
        "=> AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AsyncLiveSpecSuppliesAdequateLeaderFixedDeadlineAndResponsiveDissemination",
    ): (
        "\\A initialContext: AsyncLiveSpecAt(initialContext) "
        "=> AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AdequateLeaderFixedDeadlineAndDisseminationSupplyLocalTargetConvergence",
    ): (
        "\\A specification: "
        "/\\ AdequateLeaderLocalFreshSelfCorridorExposureProperty(specification) "
        "/\\ AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty( "
        "specification) "
        "=> AdequateLeaderLocalTargetDecisionConvergenceProperty(specification)"
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AdequateLeaderFixedDeadlineAndDisseminationSupplyLocalConvergence",
    ): (
        "\\A initialContext: "
        "AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty( "
        "AsyncLiveSpecAt(initialContext)) "
        "=> AdequateLeaderLocalTargetDecisionConvergenceProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AdequateLeaderFixedDeadlineAndDisseminationCloseExactResidual",
    ): (
        "\\A initialContext: "
        "AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty( "
        "AsyncLiveSpecAt(initialContext)) "
        "=> AdequateLeaderExactClosureResidualProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AdequateLeaderExactClosureResidualObligation",
    ): (
        "\\A initialContext: "
        "AdequateLeaderExactClosureResidualProperty("
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "AsyncSpecClosesExactDecisionRequestIngressContinuationPrefix",
    ): (
        "\\A initialContext: "
        "ExactDecisionRequestIngressContinuationPrefixClosureProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressContinuationPrefixHasFiniteRankWitness",
    ): (
        "\\A node, qc, archive, request: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ ExactDecisionNormalRequestIngressContinuationPrefixBlocked( "
        "node, qc, archive, request) "
        "=> \\E record \\in AsyncCandidateProducerContinuationRecordSet, "
        'status \\in {"Reserved", "Materialized"}, '
        "budget \\in AsyncCandidateProducerContinuationFrozenPrefixRankCarrier: "
        "ExactDecisionRequestIngressContinuationPrefixAtBudget( "
        "node, qc, archive, request, record, status, budget)"
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AsyncLiveProvidesAdequateLeaderViewExposureRankStep",
    ): (
        "\\A initialContext: "
        "AdequateLeaderViewExposureRankStepProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AdequateLeaderViewExposureRankOrderingIsWellFounded",
    ): (
        "IsWellFoundedOn( "
        "AdequateLeaderViewExposureRankOrdering, "
        "AdequateLeaderViewExposureRankCarrier)"
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AsyncLiveProvidesLocalFreshSelfCorridorExposure",
    ): (
        "\\A initialContext: "
        "AdequateLeaderLocalFreshSelfCorridorExposureProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedInitEstablishesPostGstResponsiveActiveRosterCoherence",
    ): (
        "IndexedChainInit "
        "=> IndexedPostGstResponsiveActiveRosterCoherence"
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedNewGstRequiresResponsiveActiveRoster",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "/\\ IndexedCompositionInvariant "
        "/\\ IndexedChainNext "
        "/\\ ~IndexedAsync(initialContext)!gst "
        "/\\ (IndexedAsync(initialContext)!gst)' "
        "=> /\\ Responsive \\subseteq "
        "IndexedAsync(initialContext)!AsyncActiveServiceNodes "
        "/\\ Responsive \\subseteq "
        "(IndexedAsync(initialContext)!AsyncActiveServiceNodes)'"
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedPostGstResponsiveActiveRosterSurvivesAction",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "/\\ IndexedCompositionInvariant "
        "/\\ IndexedChainNext "
        "/\\ IndexedAsync(initialContext)!gst "
        "/\\ Responsive \\subseteq "
        "IndexedAsync(initialContext)!AsyncActiveServiceNodes "
        "=> Responsive \\subseteq "
        "(IndexedAsync(initialContext)!AsyncActiveServiceNodes)'"
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedActionPreservesPostGstResponsiveActiveRosterCoherence",
    ): (
        "IndexedCompositionInvariant /\\ IndexedChainNext "
        "=> IndexedPostGstResponsiveActiveRosterCoherence'"
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedStutterPreservesPostGstResponsiveActiveRosterCoherence",
    ): (
        "IndexedPostGstResponsiveActiveRosterCoherence "
        "/\\ UNCHANGED IndexedChainVars "
        "=> IndexedPostGstResponsiveActiveRosterCoherence'"
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedStepPreservesPostGstResponsiveActiveRosterCoherence",
    ): (
        "IndexedCompositionInvariant "
        "/\\ [IndexedChainNext]_IndexedChainVars "
        "=> IndexedPostGstResponsiveActiveRosterCoherence'"
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedChainSpecAlwaysKeepsPostGstResponsiveRosterActive",
    ): (
        "IndexedChainSpec "
        "=> []IndexedPostGstResponsiveActiveRosterCoherence"
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedPostGstResponsiveRosterIsActive",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedChainSpec "
        "=> [](IndexedAsync(initialContext)!gst "
        "=> Responsive \\subseteq "
        "IndexedAsync(initialContext)!AsyncActiveServiceNodes)"
    ),
    (
        "SumeragiV2AsyncRankClosureProofs",
        "AsyncRankClosureProtectedServiceRankProgressObligation",
    ): (
        "\\A initialContext: "
        "ProtectedServiceRanksProgressProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AsyncRankClosureProofs",
        "AsyncRankClosureStarvationFreedomObligation",
    ): (
        "\\A initialContext: "
        "StarvationFreedomProperty(AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlRetransmissionCreatesExactPacket",
    ): (
        "\\A node \\in ValidatorIds, item: "
        "/\\ TimeoutPhysicalControlRetainedDueOwner(item) "
        "/\\ item.source = node /\\ UNCHANGED vars "
        "/\\ SendNodeRetransmissions(node) "
        "=> TimeoutPhysicalControlPacketOwner(item)'"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlPacketAdmissionPreservesExactHandoff",
    ): (
        "\\A item, packet: LET recipient == item.envelope.recipient "
        "IN /\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ TimeoutPhysicalControlPacketOwner(item) "
        "/\\ packet \\in TimeoutPhysicalControlExactPackets(item) "
        "/\\ packet = OldestDueSourcePacket(recipient, item.source) "
        "/\\ AdmitIngressPacket(recipient, item.source) "
        "=> \\/ TimeoutPhysicalControlTerminal(item)' "
        "\\/ TimeoutPhysicalControlIngressOwner(item)' "
        "\\/ TimeoutPhysicalControlCandidateOwner(item)'"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlIngressDrainPreservesExactHandoff",
    ): (
        "\\A item: LET node == item.envelope.recipient "
        "IN /\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ TimeoutPhysicalControlIngressOwner(item) "
        "/\\ SelectedIngressItemAt( "
        "node, FirstDrainableIngressIndex(node)) = item "
        "/\\ DrainFairIngressSelected(node) "
        "=> \\/ TimeoutPhysicalControlTerminal(item)' "
        "\\/ TimeoutPhysicalControlCandidateOwner(item)'"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlLifecycleStageOrderingIsWellFounded",
    ): (
        "IsWellFoundedOn( "
        "TimeoutPhysicalControlLifecycleStageOrdering, "
        "TimeoutPhysicalControlLifecycleStageCarrier)"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncSpecProvidesTimeoutPhysicalControlTransportKernels",
    ): (
        "\\A initialContext: "
        "TimeoutPhysicalControlTransportKernelProperties( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlTransportKernelsProjectDeclaredLeaves",
    ): (
        "\\A specification: "
        "TimeoutPhysicalControlTransportKernelProperties(specification) "
        "=> TimeoutRetainedPacketIngressKernelProperties(specification)"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncSpecProvidesTimeoutRetainedPacketIngressKernels",
    ): (
        "\\A initialContext: "
        "TimeoutRetainedPacketIngressKernelProperties( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncSpecAlwaysExcludesRetiredFormTcCandidates",
    ): (
        "\\A initialContext: AsyncSpecAt(initialContext) "
        "=> []TimeoutRetiredFormTcCandidateAbsent"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncSpecProvidesTimeoutTcFormationReducerKernel",
    ): (
        "\\A initialContext: "
        "TimeoutTcFormationReducerKernelProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutDecisionSourceRetainsExactDirectDelivery",
    ): (
        "\\A source, target, qc: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ TimeoutViewOwnershipKernelInvariant "
        "/\\ TimeoutDecisionKernelSource(source, target, qc) "
        "=> /\\ TimeoutDecisionRetainedControlOwner(source, target, qc) "
        "/\\ CommitCertificateDelivery(source, target, qc) "
        "/\\ TimeoutDecisionRoundTripTerminalOwner(source, target, qc)"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncSpecProvidesTimeoutDecisionOriginKernels",
    ): (
        "\\A initialContext: "
        "TimeoutDecisionOriginKernelProperties( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncSpecProvidesDirectTimeoutViewClosureResidual",
    ): (
        "\\A initialContext: "
        "DirectTimeoutViewClosureResidualProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncLiveProvidesDirectTimeoutViewClosureResidual",
    ): (
        "\\A initialContext: "
        "DirectTimeoutViewClosureResidualProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "DirectTimeoutViewClosureResidualObligation",
    ): (
        "\\A initialContext: "
        "DirectTimeoutViewClosureResidualProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureTimeoutViewProgressObligation",
    ): (
        "\\A initialContext: "
        "TimeoutViewProgressProperty(AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalTimeoutSuppliesAdequateLeaderLocalViewStep",
    ): (
        "\\A initialContext: "
        "AdequateLeaderLocalTimeoutViewStepProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateScheduledImportOwnersHaveExactProvenance",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, candidate: "
        "/\\ IndexedHistoricalCertificateLocalLineageInvariantAt(initialContext) "
        "/\\ candidate \\in "
        "IndexedDecisionWitness(initialContext)!AsyncCandidateSet "
        "/\\ IndexedDecisionWitness(initialContext)! "
        "CandidateConsumerCurrent(candidate) "
        "/\\ IndexedDecisionWitness(initialContext)!CandidateScheduled(candidate) "
        "/\\ IndexedDecisionWitness(initialContext)! "
        "AsyncCommitImportExecutionNeedsLineage(candidate) => "
        "IndexedDecisionWitness(initialContext)! "
        "AsyncCommitImportExecutionProvenance(candidate)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedDecisionWitnessInitEstablishesHistoricalCertificateLocalLineage",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedDecisionWitness(initialContext)!AsyncInitAt(initialContext) "
        "=> IndexedHistoricalCertificateLocalLineageInvariantAt( "
        "initialContext)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedInitEstablishesHistoricalCertificateLocalLineage",
    ): (
        "IndexedChainInit => "
        "IndexedHistoricalCertificateLocalLineageInvariant"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedDecisionWitnessBracketPreservesHistoricalCertificateLocalLineage",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "/\\ IndexedDecisionWitnessSupportAt(initialContext) "
        "/\\ IndexedHistoricalTemporalSupportAt(initialContext) "
        "/\\ IndexedHistoricalCertificateLocalLineageInvariantAt(initialContext) "
        "/\\ [IndexedDecisionWitness(initialContext)!AsyncNext]_( "
        "IndexedDecisionWitness(initialContext)!AsyncAllVars) => "
        "IndexedHistoricalCertificateLocalLineageInvariantAt( "
        "initialContext)'"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedBracketStepPreservesHistoricalCertificateLocalLineage",
    ): (
        "/\\ IndexedDecisionWitnessSupport "
        "/\\ \\A initialContext \\in AdmissibleContextRecords: "
        "IndexedHistoricalTemporalSupportAt(initialContext) "
        "/\\ IndexedHistoricalCertificateLocalLineageInvariant "
        "/\\ [IndexedChainNext]_IndexedChainVars => "
        "IndexedHistoricalCertificateLocalLineageInvariant'"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecAlwaysHistoricalCertificateLocalLineage",
    ): (
        "IndexedChainSpec => "
        "[]IndexedHistoricalCertificateLocalLineageInvariant"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalCertificateExactCommandLocalImport",
    ): (
        "IndexedChainSpec => \\A initialContext \\in "
        "AdmissibleContextRecords, node \\in Responsive: "
        "IndexedHistoricalCertificateExactCommandLocalImportAt( "
        "initialContext, node) ~> "
        "IndexedHistoricalCertificateCandidateEntryGoal( "
        "initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalCertificateReceivedQcLocalImportEntry",
    ): (
        "IndexedChainSpec => "
        "IndexedHistoricalCertificateReceivedQcLocalImportEntryProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalCertificateDecisionWalLocalImportEntry",
    ): (
        "IndexedChainSpec => "
        "IndexedHistoricalCertificateDecisionWalLocalImportEntryProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalCertificateLocalImportCandidateEntry",
    ): (
        "IndexedChainSpec => "
        "IndexedHistoricalCertificateLocalImportCandidateEntryProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateLocalImportCandidateEntryClosesRankOne",
    ): (
        "IndexedHistoricalCertificateLocalImportCandidateEntryProperty => "
        "IndexedHistoricalCertificateRankOneCandidateEntryProperty"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "DirectCommitQcCandidateHasExactImportLineage",
    ): (
        "\\A item: /\\ item \\in asyncSentItems "
        '/\\ item.kind = "CommitQC" '
        "/\\ item.envelope.qc \\in commitQCs "
        "/\\ item.envelope.qc.context = context => "
        "AsyncCommitImportCandidateLineage( DeliveryCandidate(item), "
        "item.envelope.qc)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "CommitCertificateResponseCandidateHasExactImportLineage",
    ): (
        "\\A item: /\\ item \\in asyncSentItems "
        "/\\ CommitCertificateResponseAuthorized(item) => "
        "AsyncCommitImportCandidateLineage( "
        "CommitCertificateResponseCandidate(item), item.envelope.qc)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "CommitImportCausalSuccessorRetainsExactLineage",
    ): (
        "\\A candidate, qc, successor: "
        "/\\ AsyncCommitImportCandidateLineage(candidate, qc) "
        "/\\ successor \\in SequenceSet(CommandSuccessors(candidate)) "
        "/\\ successor.kind \\in "
        '{"BeginDecision", "PersistDecision"} => '
        "AsyncCommitImportCandidateLineage(successor, qc)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateServiceStageCarrierHasExactlyElevenClasses",
    ): "AsyncCandidateServiceStageCapacity = 11",
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateServiceTrackedKindProjectionIsCovered",
    ): (
        "\\A kind \\in AsyncCandidateServiceTrackedKinds: "
        "AsyncCandidateServiceStageForKind(kind) \\in "
        "AsyncCandidateServiceStageClasses"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateServiceRecordCapacityMatchesConfiguredGeometry",
    ): (
        "AsyncConfiguration => AsyncCandidateServiceRecordCapacity = N * "
        "(AsyncSemanticIngressLifecycleCapacity + 2 * "
        "AsyncDeferredNormalCapacity + AsyncDeferredProgressCapacity + 4 * "
        "AsyncQueueCapacity + AsyncIoWorkCapacity + "
        "AsyncDormantDurableLifecycleCapacity + 1) * 11"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateServiceLifecycleStageCollisionCoalesces",
    ): (
        "\\A state, left, right: "
        "/\\ AsyncCandidateServiceOwnerPartitionInvariantIn(state) "
        "/\\ AsyncCandidateLifecycleSlotInjectionInvariantIn(state) "
        "/\\ left \\in state.candidateServiceMarkers \\cup "
        "state.candidateTerminalTombstones "
        "/\\ right \\in state.candidateServiceMarkers \\cup "
        "state.candidateTerminalTombstones "
        "/\\ left.node = right.node "
        "/\\ (AsyncCandidateLifecycleRecordForServiceIn(state, left)).slot "
        "= (AsyncCandidateLifecycleRecordForServiceIn(state, right)).slot "
        "/\\ AsyncCandidateServiceStageForKind(left.phase) = "
        "AsyncCandidateServiceStageForKind(right.phase) => left = right"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateServiceRecordsInjectIntoLifecycleStageOwners",
    ): (
        "AsyncControlServiceStateTypeInvariant => "
        "/\\ AsyncCandidateServiceStageOwnerProjectionIn( "
        "asyncControlServiceState) \\in Injection( "
        "AsyncCandidateServiceTombstones, "
        "AsyncCandidateServiceStageOwnerAddresses) "
        "/\\ Cardinality(AsyncCandidateServiceTombstones) <= "
        "AsyncCandidateServiceRecordCapacity"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateServiceRecordProducersAreTrackedBoundaryKinds",
    ): (
        "\\A candidate \\in AsyncCandidateSet: candidate \\in "
        "AsyncCandidateServicesThisStep \\cup "
        "AsyncCandidateTerminalDiscardsThisStep => candidate.kind \\in "
        "AsyncCandidateServiceTrackedKinds"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateUntrackedInternalContinuationAllocatesNoServiceRecord",
    ): (
        "\\A candidate \\in AsyncCandidateSet: candidate.kind \\in "
        "AsyncWorkKinds \\ AsyncCandidateServiceTrackedKinds => candidate "
        "\\notin AsyncCandidateServicesThisStep \\cup "
        "AsyncCandidateTerminalDiscardsThisStep"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateServiceIdentityIgnoresSchedulerClass",
    ): (
        "\\A candidate \\in AsyncCandidateSet, commandClass \\in "
        "AsyncCommandClasses: AsyncCandidateServiceIdentity( "
        "[candidate EXCEPT !.class = commandClass]) = "
        "AsyncCandidateServiceIdentity(candidate)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleAndServiceIdentityIgnoreSchedulerClass",
    ): (
        "\\A leftClass, rightClass, kind, node, blockHeight, roundView, "
        "subject, item, consumerView, consumerGeneration, evidence, "
        "bodyIdentity, manifestIdentity, commitmentIdentity: LET left == "
        "AsyncCandidateAtConsumer( leftClass, kind, node, blockHeight, "
        "roundView, subject, item, consumerView, consumerGeneration, "
        "evidence, bodyIdentity, manifestIdentity, commitmentIdentity) "
        "right == AsyncCandidateAtConsumer( rightClass, kind, node, "
        "blockHeight, roundView, subject, item, consumerView, "
        "consumerGeneration, evidence, bodyIdentity, manifestIdentity, "
        "commitmentIdentity) IN /\\ left.causalOrigin = right.causalOrigin "
        "/\\ AsyncCandidateServiceIdentity(left) = "
        "AsyncCandidateServiceIdentity(right)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateSameGenerationSuccessfulServiceIdentityPersistsUntilStrictExit",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ AsyncCandidateTransientServiceActive(candidate) "
        "/\\ candidate.consumerGeneration = generation[candidate.node] "
        "/\\ gst /\\ [AsyncNext]_AsyncAllVars "
        "/\\ ~AsyncCandidateTransientMarkerExitThisStep(candidate) => "
        "/\\ AsyncCandidateServiceTombstoned(candidate)' "
        "/\\ ~CandidateScheduled(candidate)'"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "DirectTimeoutPhysicalKernelsDischargeCompositeSeams",
    ): (
        "\\A initialContext: DirectTimeoutViewClosureResidualProperty( "
        "AsyncLiveSpecAt(initialContext)) => /\\ "
        "TimeoutFixedClockLifecycleOwnerServiceProperty( "
        "AsyncLiveSpecAt(initialContext)) /\\ "
        "TimeoutArmedExactWalEndpointProperty( "
        "AsyncLiveSpecAt(initialContext)) /\\ "
        "TimeoutSourceIsolatedDeliveryConvergenceProperty( "
        "AsyncLiveSpecAt(initialContext)) /\\ "
        "TimeoutCertificateAndDecisionConvergenceProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "DirectTimeoutViewDecompositionClosesTimeoutViewProgress",
    ): (
        "\\A initialContext: DirectTimeoutViewClosureResidualProperty( "
        "AsyncLiveSpecAt(initialContext)) => TimeoutViewProgressProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutViewOwnershipPreservationObligation",
    ): (
        "\\A initialContext: TimeoutViewOwnershipPreservationProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncLiveTimeoutConcreteOriginContinuation",
    ): (
        "\\A initialContext: "
        "ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
        "AsyncSpecAt(initialContext)) "
        "=> TimeoutConcreteOriginContinuationProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutArmedExactWalEndpointClosesRuntimePrefix",
    ): (
        "\\A specification: "
        "TimeoutArmedExactWalEndpointProperty(specification) => "
        "TimeoutArmedRuntimePrefixProperty(specification)"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutSemanticOwnerHandoffFromArmedRuntimePrefix",
    ): (
        "\\A initialContext: "
        "/\\ ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ TimeoutArmedRuntimePrefixProperty( "
        "AsyncLiveSpecAt(initialContext)) => "
        "TimeoutSemanticOwnerHandoffProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutSemanticOwnerHandoffFromExactWalEndpoint",
    ): (
        "\\A initialContext: "
        "/\\ ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ TimeoutArmedExactWalEndpointProperty( "
        "AsyncLiveSpecAt(initialContext)) => "
        "TimeoutSemanticOwnerHandoffProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "BeginTimeoutCreatesExactPendingWalOwner",
    ): (
        "\\A source, sourceView: /\\ nodeView[source] = sourceView "
        "/\\ BeginTimeout(source) => \\E vote \\in TimeoutVoteRecordSet: "
        "TimeoutPendingWalOwner(source, sourceView, vote)'"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "DirectTimeoutCreatesExactWalOrDeferredOwner",
    ): (
        "\\A source, sourceView: "
        "/\\ TimeoutDeadlineArmedOwner(source, sourceView) "
        "/\\ DirectTimeoutStep(source) => "
        "\\/ \\E vote \\in TimeoutVoteRecordSet: "
        "TimeoutPendingWalOwner(source, sourceView, vote)' "
        "\\/ TimeoutDeferredRuntimeOwner(source, sourceView)'"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "DeferredTimeoutOwnerCreatesExactPendingWalOwner",
    ): (
        "\\A source, sourceView: "
        "/\\ TimeoutDeferredRuntimeOwner(source, sourceView) "
        "/\\ DeferredTimeoutStep(source) => "
        "\\E vote \\in TimeoutVoteRecordSet: "
        "TimeoutPendingWalOwner(source, sourceView, vote)'"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "DeferredTimeoutOwnerStepSelectsBeginTimeout",
    ): (
        "\\A source, sourceView: "
        "/\\ TimeoutDeferredRuntimeOwner(source, sourceView) "
        "/\\ DeferredTimeoutStep(source) => BeginTimeout(source)"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPredeadlineClockSourceHasPositiveNaturalRank",
    ): (
        "\\A source \\in AsyncCurrentResponsiveVoters, "
        "sourceView \\in Views: /\\ AsyncStrongTypeInvariant "
        "/\\ TimeoutRoundTrigger(source, sourceView) "
        "/\\ ~TimeoutPredeadlineClockExit(source, sourceView) => "
        "\\E rank \\in Nat: TimeoutPredeadlineClockAtRank( source, "
        "sourceView, rank)"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncTickStrictlyLowersPredeadlineClockRankOrExits",
    ): (
        "\\A source \\in AsyncCurrentResponsiveVoters, "
        "sourceView \\in Views, rank \\in Nat: "
        "/\\ TimeoutPredeadlineClockAtRank(source, sourceView, rank) "
        "/\\ AsyncTick => \\/ TimeoutPredeadlineClockExit("
        "source, sourceView)' \\/ \\E lowerRank \\in Nat: "
        "/\\ lowerRank < rank /\\ TimeoutPredeadlineClockAtRank( "
        "source, sourceView, lowerRank)'"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "ExecuteExactCurrentViewTimeoutDeliveryRecordsExactReceipt",
    ): (
        "\\A vote \\in TimeoutVoteRecordSet, "
        "recipient \\in AsyncCurrentResponsiveVoters, command: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ vote.signer \\in AsyncCurrentResponsiveVoters "
        "/\\ vote \\in timeoutIntents "
        "/\\ nodeView[recipient] = vote.view "
        "/\\ ExactTimeoutVoteDeliveryCommand(vote, recipient, command) "
        "/\\ ExecuteCoreDelivery(command) => TimeoutReceipt("
        "vote, recipient)'"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "ExecuteExactTimeoutCertificateDeliveryCreatesInstallOrGoal",
    ): (
        "\\A source, recipient, tc, minimumView, command: "
        "/\\ TimeoutCertificateSemanticIdentity(tc, minimumView) "
        "/\\ ExactTimeoutCertificateDeliveryCommand( source, recipient, "
        "tc, command) /\\ ExecuteCoreDelivery(command) => "
        "\\/ TimeoutCertificateInstallOwner(recipient, tc)' "
        "\\/ TimeoutViewGoal(recipient, minimumView)'"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "ExecuteExactCommitCertificateDeliveryRecordsExactReceipt",
    ): (
        "\\A source, target, qc, command: /\\ qc.phase = \"Commit\" "
        "/\\ ExactCommitCertificateDeliveryCommand( source, target, qc, "
        "command) /\\ ExecuteCoreDelivery(command) => "
        "QcAt(target, qc) \\in receivedQCs'"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "ExecuteExactBeginInstallCreatesExactWalOwner",
    ): (
        "\\A recipient, tc, command: "
        "/\\ ExactBeginInstallTcCommand(recipient, tc, command) "
        "/\\ ExecuteRegularCommand(command) => "
        "TimeoutCertificateInstallOwner(recipient, tc)'"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "ExecuteExactPersistInstallReachesMinimumView",
    ): (
        "\\A recipient, tc, minimumView, command: /\\ TypeInvariant "
        "/\\ TimeoutCertificateSemanticIdentity(tc, minimumView) "
        "/\\ ExactPersistInstallTcCommand(recipient, tc, command) "
        "/\\ ExecutePersistInstall(command) => "
        "TimeoutViewGoal(recipient, minimumView)'"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "ExecuteTargetBeginDecisionCreatesWalOwner",
    ): (
        "\\A target, command: "
        "/\\ TargetBeginDecisionCommand(target, command) "
        "/\\ ExecuteRegularCommand(command) => "
        "TimeoutTargetDecisionWalOwner(target)'"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "ExecuteTargetPersistDecisionReachesDecision",
    ): (
        "\\A target, command: "
        "/\\ TargetPersistDecisionCommand(target, command) "
        "/\\ ExecutePersistDecision(command) => NodeHasDecision(target)'"
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "RetiredStandaloneFormTcActionIsDisabled",
    ): "\\A node, roundView: ~FormTC(node, roundView)",
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockRankedCandidateBindsFrozenTargetLeaderEpisode",
    ): (
        "\\A target, leader \\in ValidatorIds, lockedRound \\in Views, "
        "subject \\in Subjects, leaderView \\in Views: \\A prepareQc, "
        "causalOrigin, candidate, rank: RetainedLockExactCandidateRank( "
        "target, leader, lockedRound, subject, prepareQc, leaderView, "
        "causalOrigin, candidate, rank) => "
        "/\\ candidate.causalOrigin = causalOrigin "
        "/\\ causalOrigin.target = candidate.node "
        "/\\ causalOrigin.owner = candidate.node "
        "/\\ causalOrigin.context = context "
        "/\\ causalOrigin.height = context.height "
        "/\\ causalOrigin.leader = Leader(causalOrigin.context, "
        "causalOrigin.view) /\\ causalOrigin.view \\in Views "
        "/\\ causalOrigin.subject \\in {NoSubject, subject} "
        "/\\ causalOrigin.phase \\in AsyncWorkKinds "
        "/\\ causalOrigin.payload.workKind = causalOrigin.phase "
        "/\\ candidate.consumerContext = context "
        "/\\ candidate.height = context.height "
        "/\\ candidate.subject = subject"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidateRootConstructionCoversOrigin",
    ): (
        "\\A candidate, target, leaderContext, leader, leaderView, subject: "
        "/\\ target \\in ValidatorIds "
        "/\\ leaderContext \\in ContextRecords "
        "/\\ leader \\in ValidatorIds /\\ leaderView \\in Nat "
        "/\\ subject \\in Subjects "
        "/\\ AdequateLeaderFrozenCandidateRootConstructed( candidate, "
        "target, leaderContext, leader, leaderView) => "
        "candidate.causalOrigin \\in "
        "AdequateLeaderFrozenCandidateCausalOriginCarrier( target, "
        "leaderContext, leader, leaderView, subject)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetNonDescentEpisodeBudgetIsFiniteAndCoalesced",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject, known: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AdequateLeaderFrozenTargetCorridor( target, leaderContext, "
        "leader, leaderView) "
        "/\\ AdequateLeaderTargetEpisodeKnownOwnerSet( target, "
        "leaderContext, leader, leaderView, subject, known) => "
        "/\\ AdequateLeaderTargetNonDescentEpisodeBudget( target, "
        "leaderContext, leader, leaderView, subject, known) \\in Nat "
        "/\\ AdequateLeaderTargetNonDescentEpisodeBudget( target, "
        "leaderContext, leader, leaderView, subject, known) <= Cardinality( "
        "AdequateLeaderFrozenOwnerUniverse( target, leaderContext, leader, "
        "leaderView, subject))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderServicedCandidateIdentityHasServiceWitness",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject, identity: "
        "/\\ AsyncCandidateServiceTombstoneLifecycleInvariant /\\ identity "
        "\\in AdequateLeaderTargetServicedCandidateOwnerIdentitySet( target, "
        "leaderContext, leader, leaderView, subject) => \\E candidate \\in "
        "AsyncCandidateSet, rank \\in "
        "AdequateLeaderTargetSemanticRankCarrier: /\\ "
        "AdequateLeaderFrozenTargetCandidateIdentity( candidate, rank, target, "
        "leaderContext, leader, leaderView, subject) /\\ identity = "
        "AdequateLeaderFrozenCandidateOwnerIdentity( candidate, rank, target, "
        "leaderContext, leader, leaderView, subject) /\\ "
        "AsyncCandidateServiceTombstoned(candidate)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCandidateSuccessfulServiceRetirementInstallsServicedMemory",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject, "
        "occurrenceRank, identity: /\\ "
        "AsyncCandidateServiceTombstoneLifecycleInvariant /\\ "
        "AdequateLeaderTargetCandidateSuccessfulServiceRetirementAction( "
        "target, leaderContext, leader, leaderView, subject, occurrenceRank, "
        "identity) => \\E candidate \\in AsyncCandidateSet, rank \\in "
        "AdequateLeaderTargetSemanticRankCarrier: /\\ "
        "AdequateLeaderTargetCandidateOwnerIdentityWitness( target, "
        "leaderContext, leader, leaderView, subject, identity, candidate, rank) "
        "/\\ AsyncCandidateServiceTombstoned(candidate)' /\\ "
        "~CandidateScheduled(candidate)' /\\ "
        "AdequateLeaderServicedCandidateMemory( target, leaderContext, leader, "
        "leaderView, subject, identity)'"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCandidateTerminalRetirementInstallsServicedMemory",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject, "
        "occurrenceRank, identity: /\\ AsyncStrongTypeInvariant /\\ "
        "AsyncProgressOwnershipInvariant /\\ "
        "AsyncCandidateServiceTombstoneLifecycleInvariant /\\ "
        "AdequateLeaderTargetCandidateTerminalDiscardRetirementAction( target, "
        "leaderContext, leader, leaderView, subject, occurrenceRank, identity) "
        "=> AdequateLeaderServicedCandidateMemory( target, leaderContext, "
        "leader, leaderView, subject, identity)'"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCandidateRetirementEstablishesClosure",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject, "
        "occurrenceRank, identity: "
        "AdequateLeaderTargetCandidateOwnerIdentityRetirementAction( target, "
        "leaderContext, leader, leaderView, subject, occurrenceRank, identity) "
        "=> AdequateLeaderServicedCandidateClosure( target, leaderContext, "
        "leader, leaderView, subject, occurrenceRank, identity)'"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCandidateSuccessfulServiceRetirementStartsClosedMemory",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject, "
        "occurrenceRank, identity: /\\ "
        "AsyncCandidateServiceTombstoneLifecycleInvariant /\\ "
        "AdequateLeaderTargetCandidateSuccessfulServiceRetirementAction( "
        "target, leaderContext, leader, leaderView, subject, occurrenceRank, "
        "identity) => /\\ AdequateLeaderServicedCandidateMemory( target, "
        "leaderContext, leader, leaderView, subject, identity)' /\\ "
        "AdequateLeaderServicedCandidateClosure( target, leaderContext, leader, "
        "leaderView, subject, occurrenceRank, identity)'"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCandidateTerminalRetirementStartsClosedMemory",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject, "
        "occurrenceRank, identity: /\\ AsyncStrongTypeInvariant /\\ "
        "AsyncProgressOwnershipInvariant /\\ "
        "AsyncCandidateServiceTombstoneLifecycleInvariant /\\ "
        "AdequateLeaderTargetCandidateTerminalDiscardRetirementAction( target, "
        "leaderContext, leader, leaderView, subject, occurrenceRank, identity) "
        "=> /\\ AdequateLeaderServicedCandidateMemory( target, leaderContext, "
        "leader, leaderView, subject, identity)' /\\ "
        "AdequateLeaderServicedCandidateClosure( target, leaderContext, leader, "
        "leaderView, subject, occurrenceRank, identity)'"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderServicedCandidateMemoryAndClosureAreStepInvariant",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject, "
        "occurrenceRank, identity: /\\ AsyncStrongTypeInvariant /\\ "
        "AsyncProgressOwnershipInvariant /\\ "
        "AsyncCandidateServiceTombstoneLifecycleInvariant "
        "/\\ identity \\in AdequateLeaderFrozenCandidateOwnerUniverse( "
        "target, leaderContext, leader, leaderView, subject) /\\ gst /\\ "
        "AdequateLeaderServicedCandidateMemory( target, leaderContext, leader, "
        "leaderView, subject, identity) /\\ "
        "AdequateLeaderServicedCandidateClosure( target, leaderContext, leader, "
        "leaderView, subject, occurrenceRank, identity) /\\ AsyncNext => /\\ "
        "AdequateLeaderServicedCandidateMemory( target, leaderContext, leader, "
        "leaderView, subject, identity)' /\\ "
        "AdequateLeaderServicedCandidateClosure( target, leaderContext, leader, "
        "leaderView, subject, occurrenceRank, identity)'"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncSpecProvidesAdequateLeaderTargetCandidateSuccessfulServiceMemory",
    ): (
        "\\A initialContext: "
        "AdequateLeaderTargetCandidateSuccessfulServiceMemoryProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncSpecProvidesAdequateLeaderTargetCandidateTerminalTombstones",
    ): (
        "\\A initialContext: "
        "AdequateLeaderTargetCandidateTerminalTombstoneProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncSpecProvidesAdequateLeaderTargetCandidateIdentityTombstones",
    ): (
        "\\A initialContext: "
        "AdequateLeaderTargetCandidateIdentityTombstoneProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFiniteBudgetDescentClosesNonDescentEpisode",
    ): (
        "\\A initialContext: "
        "AdequateLeaderTargetNonDescentEpisodeBudgetDescentProperty( "
        "AsyncLiveSpecAt(initialContext)) => "
        "AdequateLeaderTargetNonDescentEpisodeClosureProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncTargetNeutralLifecycleOwnerCarrierIsFinite",
    ): (
        "\\A candidateCarrier, serveCarrier: "
        "/\\ IsFiniteSet(candidateCarrier) "
        "/\\ IsFiniteSet(serveCarrier) => IsFiniteSet( "
        "AsyncTargetNeutralLifecycleOwnerCarrier( "
        "candidateCarrier, serveCarrier))"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncTargetNeutralLifecycleEpisodeBudgetIsFiniteAndCoalesced",
    ): (
        "\\A candidateCarrier, serveCarrier, liveOwners, known, budget: "
        "AsyncTargetNeutralLifecycleEpisodeAtBudget( candidateCarrier, "
        "serveCarrier, liveOwners, known, budget) => /\\ budget \\in Nat "
        "/\\ budget <= Cardinality( "
        "AsyncTargetNeutralLifecycleOwnerCarrier( candidateCarrier, "
        "serveCarrier)) /\\ (liveOwners \\subseteq known <=> "
        "AsyncTargetNeutralLifecycleDiscoveredOwnerSet( liveOwners, known) "
        "= {})"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncTargetNeutralLifecycleDiscoveryStrictlyConsumesBudget",
    ): (
        "\\A candidateCarrier, serveCarrier, liveOwners, known, budget: "
        "/\\ AsyncTargetNeutralLifecycleEpisodeAtBudget( candidateCarrier, "
        "serveCarrier, liveOwners, known, budget) "
        "/\\ AsyncTargetNeutralLifecycleDiscoveredOwnerSet( liveOwners, "
        "known) # {} => AsyncTargetNeutralLifecycleKnownAdvanceGoal( "
        "candidateCarrier, serveCarrier, liveOwners, known, budget)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncTargetNeutralLifecycleBudgetOrderingIsWellFounded",
    ): (
        "IsWellFoundedOn(AsyncTargetNeutralLifecycleBudgetOrdering, Nat)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleReviewedTokenOwnsOneOrigin",
    ): (
        "\\A state, node, token \\in "
        "AsyncCandidateLifecycleReviewedSemanticOwnerTokensIn(state, node), "
        "left, right: /\\ token.origin = left /\\ token.origin = right "
        "=> left = right"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecyclePhysicalTokensCoverScheduledOriginsAfter",
    ): (
        "\\A node \\in ValidatorIds: /\\ AsyncRuntimeTypeInvariant' "
        "/\\ AsyncIoTypeInvariant' /\\ AsyncDeferredTypeInvariant' => "
        "{token.origin: token \\in "
        "AsyncCandidateLifecyclePhysicalOwnerTokensForNodeAfter(node)} = "
        "AsyncScheduledCandidateOriginsForNodeAfter(node)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleDurableTokensCoverReplayOriginsAfter",
    ): (
        "\\A node \\in ValidatorIds: {token.origin: token \\in "
        "AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter(node)} = "
        "AsyncCandidateLifecycleDurableReplayOriginsForNodeAfter(node)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleServiceOwnerCarrierIsSlotBounded",
    ): (
        "\\A state, node: /\\ node \\in ValidatorIds "
        "/\\ AsyncCandidateServiceOwnerPartitionInvariantIn(state) "
        "/\\ AsyncCandidateLifecycleSlotInjectionInvariantIn(state) => "
        "Cardinality( "
        "AsyncCandidateLifecycleServiceOwnerTokensForNodeIn( state, node)) "
        "<= AsyncServicedCandidateLifecycleCapacity"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecyclePhysicalAndDurableOwnersFitActiveSlots",
    ): (
        "\\A node \\in ValidatorIds: /\\ AsyncRuntimeTypeInvariant' "
        "/\\ AsyncIoTypeInvariant' /\\ AsyncDeferredTypeInvariant' => "
        "Cardinality( "
        "AsyncCandidateLifecyclePhysicalOwnerTokensForNodeAfter(node) "
        "\\cup AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter(node)) "
        "<= AsyncReviewedActiveCandidateLifecycleCapacity"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleCompactedStateHasSemanticOwnerCoverage",
    ): (
        "\\A state, node: LET carrierState == "
        "AsyncCandidateLifecycleStateAfterCarrierUpdate(state) "
        "compactedState == "
        "AsyncCandidateLifecycleStateAfterCompaction(carrierState) IN "
        "/\\ node \\in ValidatorIds /\\ AsyncRuntimeTypeInvariant' "
        "/\\ AsyncIoTypeInvariant' /\\ AsyncDeferredTypeInvariant' => "
        "AsyncCandidateLifecycleReviewedSemanticCoverageIn( "
        "compactedState, node)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleCompactedStateHasActiveOwnerCoverage",
    ): (
        "\\A state, node: LET carrierState == "
        "AsyncCandidateLifecycleStateAfterCarrierUpdate(state) "
        "compactedState == "
        "AsyncCandidateLifecycleStateAfterCompaction(carrierState) IN "
        "/\\ node \\in ValidatorIds /\\ AsyncRuntimeTypeInvariant' "
        "/\\ AsyncIoTypeInvariant' /\\ AsyncDeferredTypeInvariant' "
        "/\\ AsyncCandidateServiceOwnerPartitionInvariantIn(compactedState) "
        "=> AsyncCandidateLifecycleReviewedActiveCoverageIn( "
        "compactedState, node)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleSemanticCoverageGivesOwnerInjection",
    ): (
        "\\A state, node: /\\ node \\in ValidatorIds /\\ IsFiniteSet( "
        "AsyncCandidateLifecycleLiveOrdinaryOriginCarrierIn(state, node)) "
        "/\\ IsFiniteSet( "
        "AsyncCandidateLifecycleReviewedSemanticOwnerTokensIn(state, node)) "
        "/\\ AsyncCandidateLifecycleReviewedSemanticCoverageIn(state, node) "
        "=> AsyncCandidateLifecycleSemanticOwnerInjectionIn(state, node)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleActiveCoverageGivesOwnerInjection",
    ): (
        "\\A state, node: /\\ node \\in ValidatorIds /\\ IsFiniteSet( "
        "AsyncCandidateLifecycleLiveActiveOriginCarrierIn(state, node)) "
        "/\\ IsFiniteSet( "
        "AsyncCandidateLifecycleReviewedActiveOwnerTokensForNodeAfter(node)) "
        "/\\ AsyncCandidateLifecycleReviewedActiveCoverageIn(state, node) "
        "=> AsyncCandidateLifecycleActiveOwnerInjectionIn(state, node)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleReviewedSemanticOwnersFitOrdinaryCapacity",
    ): (
        "\\A state, node: /\\ node \\in ValidatorIds "
        "/\\ AsyncRuntimeTypeInvariant' /\\ AsyncIoTypeInvariant' "
        "/\\ AsyncDeferredTypeInvariant' "
        "/\\ AsyncCandidateServiceOwnerPartitionInvariantIn(state) "
        "/\\ AsyncCandidateLifecycleSlotInjectionInvariantIn(state) => "
        "Cardinality( "
        "AsyncCandidateLifecycleReviewedSemanticOwnerTokensIn( state, node)) "
        "<= AsyncCandidateLifecycleOrdinaryCapacity"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleReviewedOwnerInjectionProvidesReservations",
    ): (
        "\\A state, node: /\\ node \\in ValidatorIds "
        "/\\ AsyncCandidateLifecycleSlotInjectionInvariantIn(state) "
        "/\\ AsyncCandidateLifecycleActiveOwnerInjectionIn(state, node) "
        "/\\ Cardinality( "
        "AsyncCandidateLifecycleReviewedActiveOwnerTokensForNodeAfter(node)) "
        "<= AsyncReviewedActiveCandidateLifecycleCapacity => Cardinality( "
        "AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn(state, node)) "
        "<= Cardinality( "
        "AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn( state, node))"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleCompactedStateProvidesFreshReservations",
    ): (
        "\\A state, node: LET carrierState == "
        "AsyncCandidateLifecycleStateAfterCarrierUpdate(state) "
        "compactedState == "
        "AsyncCandidateLifecycleStateAfterCompaction(carrierState) IN "
        "/\\ node \\in ValidatorIds "
        "/\\ IsFiniteSet(state.candidateLifecycleAdmissions) "
        "/\\ IsFiniteSet(state.candidateServiceMarkers) "
        "/\\ IsFiniteSet(state.candidateTerminalTombstones) "
        "/\\ AsyncRuntimeTypeInvariant' /\\ AsyncIoTypeInvariant' "
        "/\\ AsyncDeferredTypeInvariant' "
        "/\\ AsyncCandidateLifecycleSlotInjectionInvariantIn(compactedState) "
        "/\\ AsyncCandidateServiceOwnerPartitionInvariantIn(compactedState) "
        "=> Cardinality( "
        "AsyncOrdinaryNewCandidateLifecycleOriginsForNodeIn( compactedState, "
        "node)) <= Cardinality( "
        "AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn( compactedState, "
        "node))"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncIgnoredIngressEpisodeCannotConsumeLifecycleCapacity",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ AsyncCandidateIgnoredWithoutApplicationThisStep(candidate) "
        "/\\ candidate.item # NoAsyncItem "
        "/\\ AsyncCandidateLifecycleRecorded( candidate.node, "
        "candidate.causalOrigin) /\\ AsyncControlServiceSlotTransition => "
        "/\\ AsyncNextCandidateServiceOrdinal(candidate.node)' = "
        "AsyncNextCandidateServiceOrdinal(candidate.node) "
        "/\\ AsyncCandidateServiceRecordsForIdentity( "
        "AsyncCandidateServiceIdentity(candidate))' = {} "
        "/\\ IF AsyncCandidateProducerContinuationSourceAfter(candidate) "
        "THEN /\\ AsyncCandidateLifecycleRecorded( candidate.node, "
        "candidate.causalOrigin)' "
        "/\\ AsyncCandidateProducerContinuationActiveForIdentity( "
        "AsyncCandidateServiceIdentity(candidate))' "
        "ELSE ~AsyncCandidateLifecycleRecorded( candidate.node, "
        "candidate.causalOrigin)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncControlServiceTransitionRequiresAtomicLifecycleReservation",
    ): (
        "AsyncControlServiceSlotTransition => LET resetState == "
        "AsyncControlServiceStateAfterReset( asyncControlServiceState, "
        "AsyncControlServiceResetNodesThisStep) admittedState == IF "
        "AsyncControlServiceAdmissionsThisStep = {} THEN resetState ELSE "
        "AsyncControlServiceStateAfterAdmission( resetState, CHOOSE item "
        "\\in AsyncControlServiceAdmissionsThisStep: TRUE) servicedState == "
        "IF AsyncControlServicesThisStep = {} THEN admittedState ELSE "
        "AsyncControlServiceStateAfterService( admittedState, CHOOSE item "
        "\\in AsyncControlServicesThisStep: TRUE) responseRetirementState == "
        "AsyncCertifiedResponseClaimStateAfterRetirement(servicedState) "
        "responseState == IF AsyncCertifiedResponseClaimAdmissionsThisStep = "
        "{} THEN responseRetirementState ELSE "
        "AsyncCertifiedResponseClaimStateAfterAdmission( "
        "responseRetirementState, CHOOSE item \\in "
        "AsyncCertifiedResponseClaimAdmissionsThisStep: TRUE) "
        "timeoutRetirementState == "
        "AsyncControlServiceStateAfterTimeoutRetirement(responseState) "
        "candidateReclamationState == "
        "AsyncCandidateServiceStateAfterReclamation( timeoutRetirementState) "
        "candidateMarkedState == IF AsyncCandidateServicesThisStep # {} "
        "THEN AsyncCandidateServiceStateAfterSuccessfulService( "
        "candidateReclamationState, CHOOSE candidate \\in "
        "AsyncCandidateServicesThisStep: TRUE) ELSE IF "
        "AsyncCandidateTerminalDiscardsThisStep # {} THEN "
        "AsyncCandidateServiceStateAfterTerminalRetirement( "
        "candidateReclamationState, CHOOSE candidate \\in "
        "AsyncCandidateTerminalDiscardsThisStep: TRUE) ELSE "
        "candidateReclamationState candidateOwnedState == IF "
        "AsyncCandidateLifecycleDeparturesThisStep # {} THEN "
        "AsyncCandidateLifecycleStateAfterServiceSlotTransfer( "
        "candidateMarkedState, CHOOSE candidate \\in "
        "AsyncCandidateLifecycleDeparturesThisStep: TRUE) ELSE "
        "candidateMarkedState candidateServiceState == IF "
        "AsyncCandidateLifecycleDeparturesThisStep # {} THEN "
        "AsyncCandidateProducerContinuationStateAfterDeparture( "
        "candidateOwnedState, CHOOSE candidate \\in "
        "AsyncCandidateLifecycleDeparturesThisStep: TRUE) ELSE "
        "candidateOwnedState carrierState == "
        "AsyncCandidateLifecycleStateAfterCarrierUpdate( "
        "candidateServiceState) compactedState == "
        "AsyncCandidateLifecycleStateAfterCompaction(carrierState) "
        "leaderWireState == "
        "AsyncCandidateLifecycleStateAfterLeaderWireAdmission( compactedState) "
        "serveIngressState == "
        "AsyncCandidateLifecycleStateAfterServeIngressAdmission( "
        "leaderWireState) IN "
        "/\\ AsyncFreshLeaderWireLifecycleAdmissionsAreSingularThisStep "
        "/\\ AsyncFreshLeaderWireLifecycleAdmissionOrdinalMatchesIn( "
        "compactedState) "
        "/\\ AsyncFreshLeaderWireLifecycleSchedulerOrdinalMatchesIn( "
        "compactedState) "
        "/\\ AsyncFreshServeIngressAdmissionsAreSingularThisStep "
        "/\\ AsyncFreshServeIngressSchedulerReservationMatchesIn( "
        "leaderWireState) "
        "/\\ AsyncCandidateLifecycleReservationsAvailableIn( serveIngressState) "
        "/\\ AsyncCandidateTerminalServiceReservationAvailableIn( "
        "candidateReclamationState) "
        "/\\ AsyncCandidateServiceReservationAvailableIn(candidateMarkedState) "
        "/\\ AsyncCandidateProducerContinuationReservationAvailableAfterDepartureIn( "
        "candidateOwnedState)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "CommandSuccessorsRetainCausalOrigin",
    ): (
        "\\A command: \\A successor \\in "
        "SequenceSet(CommandSuccessors(command)): "
        "successor.causalOrigin = command.causalOrigin"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetCandidateIdentityHasBoundedPayload",
    ): (
        "\\A candidate, rank, target, leaderContext, "
        "leader, leaderView, subject: "
        "AdequateLeaderTargetCandidateIdentity( "
        "candidate, rank, target, leaderContext, "
        "leader, leaderView, subject) "
        "=> AdequateLeaderCandidatePayloadWithinFrozenView( "
        "candidate, leaderView)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidateIdentityHasBoundedPayload",
    ): (
        "\\A candidate, rank, target, leaderContext, "
        "leader, leaderView, subject: "
        "AdequateLeaderFrozenTargetCandidateIdentity( "
        "candidate, rank, target, leaderContext, "
        "leader, leaderView, subject) "
        "=> AdequateLeaderCandidatePayloadWithinFrozenView( "
        "candidate, leaderView)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenTargetCandidatePayloadIsInStaticCarrier",
    ): (
        "\\A candidate, rank, target, leaderContext, "
        "leader, leaderView, subject: "
        "/\\ candidate \\in AsyncCandidateSet "
        "/\\ target \\in ValidatorIds "
        "/\\ leaderContext \\in ContextRecords "
        "/\\ leader \\in ValidatorIds "
        "/\\ leaderView \\in Nat "
        "/\\ subject \\in Subjects "
        "/\\ AdequateLeaderFrozenTargetCandidateIdentity( "
        "candidate, rank, target, leaderContext, "
        "leader, leaderView, subject) "
        "=> AdequateLeaderFrozenCandidatePayload(candidate, leaderView) "
        "\\in AdequateLeaderFrozenCandidatePayloadCarrier( "
        "target, leaderContext, leader, leaderView, subject)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidateOwnerIdentityIsInjective",
    ): (
        "\\A left, right, rank, target, leaderContext, "
        "leader, leaderView, subject: "
        "/\\ left \\in AsyncCandidateSet "
        "/\\ right \\in AsyncCandidateSet "
        "/\\ AdequateLeaderFrozenTargetCandidateIdentity( "
        "left, rank, target, leaderContext, "
        "leader, leaderView, subject) "
        "/\\ AdequateLeaderFrozenTargetCandidateIdentity( "
        "right, rank, target, leaderContext, "
        "leader, leaderView, subject) "
        "/\\ AdequateLeaderFrozenCandidateOwnerIdentity( "
        "left, rank, target, leaderContext, "
        "leader, leaderView, subject) "
        "= AdequateLeaderFrozenCandidateOwnerIdentity( "
        "right, rank, target, leaderContext, "
        "leader, leaderView, subject) "
        "=> AdequateLeaderImmutableCandidatePayload(left) "
        "= AdequateLeaderImmutableCandidatePayload(right)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidateRetryIdentityIsStable",
    ): (
        "\\A left, right, rank, target, leaderContext, "
        "leader, leaderView, subject: "
        "/\\ left.node = right.node "
        "/\\ AdequateLeaderCandidatePayloadWithinFrozenView( "
        "left, leaderView) "
        "/\\ AdequateLeaderCandidatePayloadWithinFrozenView( "
        "right, leaderView) "
        "/\\ AdequateLeaderImmutableCandidatePayload(left) "
        "= AdequateLeaderImmutableCandidatePayload(right) "
        "=> AdequateLeaderFrozenCandidateOwnerIdentity( "
        "left, rank, target, leaderContext, "
        "leader, leaderView, subject) "
        "= AdequateLeaderFrozenCandidateOwnerIdentity( "
        "right, rank, target, leaderContext, "
        "leader, leaderView, subject)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidatePayloadCarrierIsFinite",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject: "
        "/\\ target \\in ValidatorIds "
        "/\\ leaderContext \\in ContextRecords "
        "/\\ leader \\in ValidatorIds "
        "/\\ leaderView \\in Nat "
        "/\\ subject \\in Subjects "
        "=> IsFiniteSet( "
        "AdequateLeaderFrozenCandidatePayloadCarrier( "
        "target, leaderContext, leader, leaderView, subject))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenOwnerUniverseIsPrimeInvariant",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject: "
        "AdequateLeaderFrozenOwnerUniverse( "
        "target, leaderContext, leader, leaderView, subject)' "
        "= AdequateLeaderFrozenOwnerUniverse( "
        "target, leaderContext, leader, leaderView, subject)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenOwnerUniverseIsFinite",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject: "
        "/\\ target \\in ValidatorIds "
        "/\\ leaderContext \\in ContextRecords "
        "/\\ leader \\in ValidatorIds "
        "/\\ leaderView \\in Nat "
        "/\\ subject \\in Subjects "
        "=> /\\ IsFiniteSet( AdequateLeaderFrozenOwnerUniverse( "
        "target, leaderContext, leader, leaderView, subject)) "
        "/\\ Cardinality( AdequateLeaderFrozenOwnerUniverse( "
        "target, leaderContext, leader, leaderView, subject)) "
        "<= 2 * Cardinality( "
        "AdequateLeaderTargetSemanticRankCarrier) "
        "* Cardinality( "
        "AdequateLeaderFrozenCandidatePayloadCarrier( "
        "target, leaderContext, leader, leaderView, subject)) "
        "+ 2 * Cardinality(LeaderWireKinds) "
        "* Cardinality( AdequateLeaderFrozenWirePayloadCarrier)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderLiveOwnersStayInsideFrozenUniverse",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AdequateLeaderFrozenTargetCorridor( "
        "target, leaderContext, leader, leaderView) "
        "=> AdequateLeaderTargetLiveOwnerIdentitySet( "
        "target, leaderContext, leader, leaderView, subject) "
        "\\subseteq AdequateLeaderFrozenOwnerUniverse( "
        "target, leaderContext, leader, leaderView, subject)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenWireRetryIdentityIsStable",
    ): (
        "\\A left, right, target, leaderContext, "
        "leader, leaderView, subject: "
        "/\\ AdequateLeaderFrozenTargetWireIdentity( "
        "left, target, leaderContext, leader, leaderView, subject) "
        "/\\ AdequateLeaderFrozenTargetWireIdentity( "
        "right, target, leaderContext, leader, leaderView, subject) "
        "/\\ left.kind = right.kind "
        '/\\ (left.kind = "CertifiedResponse" '
        "\\/ left.source = right.source) "
        "/\\ left.envelope = right.envelope "
        "=> AdequateLeaderFrozenWireOwnerIdentity( "
        "left, target, leaderContext, leader, leaderView, subject) "
        "= AdequateLeaderFrozenWireOwnerIdentity( "
        "right, target, leaderContext, leader, leaderView, subject)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetNonDescentEpisodeBudgetIsFiniteAndCoalesced",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject, known: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AdequateLeaderFrozenTargetCorridor( "
        "target, leaderContext, leader, leaderView) "
        "/\\ AdequateLeaderTargetEpisodeKnownOwnerSet( "
        "target, leaderContext, leader, leaderView, subject, known) "
        "=> /\\ AdequateLeaderTargetNonDescentEpisodeBudget( "
        "target, leaderContext, leader, leaderView, subject, known) \\in Nat "
        "/\\ AdequateLeaderTargetNonDescentEpisodeBudget( "
        "target, leaderContext, leader, leaderView, subject, known) "
        "<= Cardinality( AdequateLeaderFrozenOwnerUniverse( "
        "target, leaderContext, leader, leaderView, subject))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCurrentControlOwnerBlocksSameOrLowerRetries",
    ): (
        "\\A item \\in AsyncNetworkItems: /\\ item.kind \\in "
        "AsyncControlKinds /\\ AsyncControlServiceOccurrenceIsCurrentOwner(item) "
        "=> AdequateLeaderTargetSameOrLowerControlRetriesAdmissionBlocked(item)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderOffSubjectControlRetirementMemoryIsStepInvariant",
    ): (
        "\\A item, target, leaderContext, leader, leaderView, subject, "
        "occurrenceRank: /\\ AsyncStrongTypeInvariant /\\ "
        "AsyncProgressOwnershipInvariant /\\ "
        "AsyncCandidateServiceTombstoneLifecycleInvariant /\\ gst /\\ "
        "AdequateLeaderTargetOffSubjectControlRetirementMemory( item, target, "
        "leaderContext, leader, leaderView, subject, occurrenceRank) /\\ "
        "[AsyncNext]_AsyncAllVars => "
        "AdequateLeaderTargetOffSubjectControlRetirementClosed( item, target, "
        "leaderContext, leader, leaderView, subject, occurrenceRank)'"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderSelectedOccurrenceOwnerIsFrozenAndLive",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject, "
        "occurrenceRank, owner: /\\ AsyncStrongTypeInvariant /\\ "
        "AdequateLeaderFrozenTargetCorridor( target, leaderContext, leader, "
        "leaderView) /\\ AdequateLeaderTargetOccurrenceOwnerSelected( target, "
        "leaderContext, leader, leaderView, subject, occurrenceRank, owner) "
        "=> /\\ owner \\in AdequateLeaderFrozenCandidateOwnerUniverse( target, "
        "leaderContext, leader, leaderView, subject) /\\ owner \\in "
        "AdequateLeaderTargetLiveCandidateOwnerIdentitySet( target, "
        "leaderContext, leader, leaderView, subject)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCorridorHasProductiveSubjectReentry",
    ): (
        "\\A target, leaderContext, leader, leaderView, retiredSubject: /\\ "
        "AsyncStrongTypeInvariant /\\ retiredSubject \\in Subjects /\\ "
        "AdequateLeaderFrozenTargetCorridor( target, leaderContext, leader, "
        "leaderView) => AdequateLeaderTargetProductiveSubjectReentryGoal( "
        "target, leaderContext, leader, leaderView, retiredSubject)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderSubjectSwitchNamedOwnerStrictlyConsumesBudget",
    ): (
        "\\A target, leaderContext, leader, leaderView, owner, retired, "
        "retired2, budget: /\\ target \\in ValidatorIds "
        "/\\ leaderContext \\in ContextRecords "
        "/\\ leader \\in ValidatorIds /\\ leaderView \\in Nat "
        "/\\ retired \\subseteq "
        "AdequateLeaderFrozenSubjectSwitchOwnerUniverse( "
        "target, leaderContext, leader, leaderView) "
        "/\\ retired2 \\subseteq "
        "AdequateLeaderFrozenSubjectSwitchOwnerUniverse( "
        "target, leaderContext, leader, leaderView) "
        "/\\ owner \\in "
        "AdequateLeaderFrozenSubjectSwitchOwnerUniverse( "
        "target, leaderContext, leader, leaderView) \\ retired "
        "/\\ retired \\cup {owner} \\subseteq retired2 "
        "/\\ budget = AdequateLeaderTargetSubjectSwitchRemainingBudget( "
        "target, leaderContext, leader, leaderView, retired) "
        "=> /\\ AdequateLeaderTargetSubjectSwitchDiscoveredOwnerSet( owner, "
        "retired) = {owner} "
        "/\\ AdequateLeaderTargetSubjectSwitchRemainingBudget( "
        "target, leaderContext, leader, leaderView, retired2) < budget"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderSubjectSwitchEpisodeStartsNamedOwnerService",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject, "
        "occurrenceRank, owner, retired, budget: /\\ "
        "AsyncStrongTypeInvariant /\\ "
        "AdequateLeaderTargetSubjectSwitchEpisodeAtBudget( target, "
        "leaderContext, leader, leaderView, subject, occurrenceRank, owner, "
        "retired, budget) => \\E known \\in SUBSET "
        "AdequateLeaderFrozenOwnerUniverse( target, leaderContext, leader, "
        "leaderView, subject), serviceBudget \\in Nat: /\\ known = "
        "AdequateLeaderTargetLiveOwnerIdentitySet( target, leaderContext, "
        "leader, leaderView, subject) /\\ "
        "AdequateLeaderTargetNonDescentEpisodeAtBudget( target, leaderContext, "
        "leader, leaderView, subject, occurrenceRank, known, serviceBudget) "
        "/\\ AdequateLeaderTargetSameOrHigherOccurrenceFrontier( target, "
        "leaderContext, leader, leaderView, subject, occurrenceRank) /\\ "
        "AdequateLeaderTargetOccurrenceOwnerSelected( target, leaderContext, "
        "leader, leaderView, subject, occurrenceRank, owner)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderComposedRankDescentClosesOccurrenceService",
    ): (
        "\\A initialContext: "
        "AdequateLeaderTargetComposedRankDescentProperty( "
        "AsyncLiveSpecAt(initialContext)) "
        "=> AdequateLeaderTargetRankServiceExitProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncSpecProvidesAdequateLeaderWirePhysicalConvergence",
    ): (
        "\\A initialContext: "
        "AdequateLeaderWirePhysicalConvergenceProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncLiveProvidesAdequateLeaderWirePhysicalConvergence",
    ): (
        "\\A initialContext: "
        "AdequateLeaderWirePhysicalConvergenceProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdvancedResponsiveNodeHasInstalledTimeoutRotationHandoff",
    ): (
        "\\A node \\in AsyncCurrentResponsiveVoters, "
        "roundView \\in Views: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ TimeoutViewOwnershipInvariant "
        "/\\ gst "
        "/\\ nodeView[node] > roundView "
        "/\\ ~NodeHasDecision(node) "
        "=> TimeoutQuorumViewRotationHandoff(node, roundView)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncLiveProvidesAdequateLeaderTimeoutRotationConvergence",
    ): (
        "\\A initialContext: "
        "AdequateLeaderTimeoutRotationConvergenceProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncLiveProvidesAdequateLeaderOpenPhysicalResidualConvergence",
    ): (
        "\\A initialContext: "
        "AdequateLeaderOpenPhysicalResidualConvergenceProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "ExactResidualKernelSuppliesExactPhysicalConvergence",
    ): (
        "\\A initialContext: "
        "AdequateLeaderExactResidualKernelProperty( "
        "AsyncLiveSpecAt(initialContext)) "
        "=> AdequateLeaderExactPhysicalResidualConvergenceProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "ExactResidualKernelSuppliesCandidateSemanticHandoffs",
    ): (
        "\\A initialContext: "
        "/\\ ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ AdequateLeaderExactResidualKernelProperty( "
        "AsyncLiveSpecAt(initialContext)) "
        "=> ExactLeaderCandidateSemanticHandoffProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncLiveProvidesAdequateLeaderExactResidualKernel",
    ): (
        "\\A initialContext: "
        "AdequateLeaderExactResidualKernelProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncLiveProvidesAdequateLeaderTargetOffSubjectControlNoReentry",
    ): (
        "\\A initialContext: "
        "AdequateLeaderTargetOffSubjectControlNoReentryProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderOwnerIndexedServiceProvidesSubjectSwitchBudgetDescent",
    ): (
        "\\A initialContext: "
        "/\\ AdequateLeaderTargetOccurrenceRankServiceProperty( "
        "AsyncLiveSpecAt(initialContext)) "
        "/\\ AdequateLeaderTargetSubjectSwitchCarryStepProperty( "
        "AsyncLiveSpecAt(initialContext)) => "
        "AdequateLeaderTargetSubjectSwitchBudgetDescentProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderSubjectSwitchCarryStepProvidesAnchoredBudgetDescent",
    ): (
        "\\A initialContext: "
        "AdequateLeaderTargetSubjectSwitchCarryStepProperty( "
        "AsyncLiveSpecAt(initialContext)) "
        "=> AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderSubjectSwitchBudgetDescentClosesNamedOwnerEpisode",
    ): (
        "\\A initialContext: "
        "AdequateLeaderTargetSubjectSwitchBudgetDescentProperty( "
        "AsyncLiveSpecAt(initialContext)) => "
        "AdequateLeaderTargetSubjectSwitchClosureProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOccurrenceRankOrderingWellFounded",
    ): (
        "IsWellFoundedOn( AdequateLeaderTargetOccurrenceRankOrdering, "
        "AdequateLeaderTargetOccurrenceRankCarrier)"
    ),
    (
        "SumeragiV2AdequateLeaderProducerTransportClosureProofs",
        "AdequateLeaderTargetOccurrenceRankFrontierRetainsFrozenCorridor",
    ): (
        "\\A target, leaderContext, leader, leaderView, subject, occurrenceRank: "
        "AdequateLeaderTargetOccurrenceRankFrontier( "
        "target, leaderContext, leader, leaderView, subject, occurrenceRank) "
        "=> AdequateLeaderFrozenTargetCorridor( "
        "target, leaderContext, leader, leaderView)"
    ),
    (
        "SumeragiV2AdequateLeaderProducerTransportClosureProofs",
        "AdequateLeaderTargetRanksReachIndexedDecision",
    ): (
        "\\A specification: "
        "/\\ AdequateLeaderAuthorityBoundReceiptAcquisitionProperty(specification) "
        "/\\ AdequateLeaderAuthorityBoundActiveReceiptServiceProperty(specification) "
        "=> (specification => \\A target \\in ValidatorIds, "
        "leaderContext \\in ContextRecords, leader \\in ValidatorIds, "
        "leaderView \\in Views, subject \\in Subjects, occurrenceRank \\in "
        "AdequateLeaderTargetOccurrenceRankCarrier: "
        "/\\ AdequateLeaderTargetProtocolSubjectSource( "
        "target, leaderContext, leader, leaderView, subject) "
        "/\\ AdequateLeaderTargetOccurrenceRankFrontier( "
        "target, leaderContext, leader, leaderView, subject, occurrenceRank) "
        "~> NodeHasDecision(target))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetConvergenceReachesEveryDecisionPrefix",
    ): (
        "\\A initialContext: "
        "/\\ AsyncLiveSpecAt(initialContext) "
        "/\\ AdequateLeaderTargetDecisionConvergenceProperty( "
        "AsyncLiveSpecAt(initialContext)) "
        "=> \\A limit \\in Nat: "
        "(gst /\\ AdequateResponsiveHonestLeaderViewReached "
        "/\\ ~ResponsiveNodesDecide) "
        "~> AdequateLeaderDecisionPrefixAt(initialContext, limit)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "FrozenContextFullAdequateLeaderDecisionPrefixImpliesResponsiveDecide",
    ): (
        "\\A initialContext: "
        "/\\ ModelConfiguration "
        "/\\ AsyncFrozenContextAt(initialContext) "
        "/\\ AdequateLeaderDecisionPrefixAt(initialContext, N - 1) "
        "=> ResponsiveNodesDecide"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSemanticCompositionSuppliesTargetConvergence",
    ): (
        "\\A initialContext: "
        "AdequateLeaderSemanticCompositionProperty( "
        "AsyncLiveSpecAt(initialContext)) "
        "=> AdequateLeaderTargetDecisionConvergenceProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetDecisionConvergenceSuppliesDecisionMode",
    ): (
        "\\A initialContext: "
        "AdequateLeaderTargetDecisionConvergenceProperty( "
        "AsyncLiveSpecAt(initialContext)) "
        "=> (AsyncLiveSpecAt(initialContext) "
        "=> (gst /\\ AdequateResponsiveHonestLeaderViewReached "
        "/\\ ~ResponsiveNodesDecide) ~> ResponsiveNodesDecide)"
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "ExactAdequateLeaderSubkernelsReduceToServiceKernel",
    ): (
        "\\A initialContext: "
        "/\\ AdequateLeaderExactResidualKernelProperty( "
        "AsyncLiveSpecAt(initialContext)) "
        "/\\ AdequateLeaderSemanticCompositionProperty( "
        "AsyncLiveSpecAt(initialContext)) "
        "=> AdequateLeaderServiceKernelProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AdequateLeaderServiceKernelObligation",
    ): (
        "\\A initialContext: "
        "AdequateLeaderServiceKernelProperty(AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionCachedRequestIngressRunnerBypassesServeLifecycle",
    ): (
        "\\A node, qc, archive, request: "
        "LET identity == "
        "ExactDecisionServeLifecycleIdentity(archive, request) "
        "IN /\\ AsyncStrongTypeInvariant "
        "/\\ ExactDecisionRequestIngressLaneResidual( "
        "node, qc, archive, request) "
        "/\\ ExactDecisionServeTombstoneOwned( "
        "node, qc, archive, request) "
        "/\\ ExactDecisionRequestIngressRunnerAction(archive, request) "
        "=> /\\ asyncIoQueues'[archive] = asyncIoQueues[archive] "
        "/\\ AsyncServeLifecycleTombstone(archive, identity)' "
        "/\\ ~AsyncServeLiveReservationOwned(archive, identity)' "
        "/\\ ~AsyncServeJobQueued(archive, identity)' "
        "/\\ ~AsyncServeIngressAdmissionOwned(archive, identity)' "
        "/\\ \\E response, packet: "
        "ExactDecisionResponsePacketOwned( "
        "node, qc, archive, request, response, packet)'"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestFrozenServeBarrierIsSingleton",
    ): (
        "\\A node, qc, archive, request: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ ExactDecisionRequestLifecycleResidual( "
        "node, qc, archive, request) "
        "=> /\\ IsFiniteSet( "
        "ExactDecisionRequestFrozenServeBarrierIdentities( "
        "archive, request)) "
        "/\\ Cardinality( "
        "ExactDecisionRequestFrozenServeBarrierIdentities( "
        "archive, request)) <= 1 "
        "/\\ (ExactDecisionRequestFrozenServeBarrierIdentities( "
        "archive, request) # {} "
        "=> ExactDecisionRequestFrozenServeBarrierIdentity( "
        "archive, request) "
        "\\in ExactDecisionRequestFrozenServeBarrierIdentities( "
        "archive, request))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestFrozenServeBarrierMaterializationLowersRank",
    ): (
        "\\A node, qc, archive, request: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ ExactDecisionRequestLifecycleResidual( "
        "node, qc, archive, request) "
        "/\\ ExactDecisionRequestFrozenServeBarrierMaterializationAction( "
        "archive, request) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "=> /\\ ExactDecisionRequestLifecycleResidual( "
        "node, qc, archive, request)' "
        "/\\ ExactDecisionRequestFrozenServeBarrierIdentities( "
        "archive, request)' = {} "
        "/\\ ExactDecisionRequestLifecycleFrozenPredecessorDebt( "
        "archive, request)' "
        "< ExactDecisionRequestLifecycleFrozenPredecessorDebt( "
        "archive, request) "
        "/\\ <<ExactDecisionRequestLifecycleIngressRank( "
        "node, qc, archive, request)', "
        "ExactDecisionRequestLifecycleIngressRank( "
        "node, qc, archive, request)>> "
        "\\in ExactDecisionRequestLifecycleIngressRankOrdering"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestFrozenServeBarrierPreservesTargetIngressCoalescing",
    ): (
        "\\A node, qc, archive, request: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ ExactDecisionRequestLifecycleResidual( "
        "node, qc, archive, request) "
        "/\\ ExactDecisionServeTombstoneOwned( "
        "node, qc, archive, request) "
        "/\\ ExactDecisionRequestFrozenServeBarrierMaterializationAction( "
        "archive, request) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "=> /\\ ExactDecisionRequestIngressOwned( "
        "node, qc, archive, request)' "
        "/\\ ExactDecisionServeTombstoneOwned( "
        "node, qc, archive, request)' "
        "/\\ AsyncServeIngressAdmissionOwned( "
        "archive, ExactDecisionServeLifecycleIdentity( "
        "archive, request))' "
        "/\\ request \\in SequenceSet( "
        "IngressLane( archive, IngressResourceSource(request)))' "
        "/\\ ExactDecisionRequestFrozenServeBarrierIdentities( "
        "archive, request)' = {}"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleIngressRankOrderingIsWellFounded",
    ): (
        "IsWellFoundedOn( "
        "ExactDecisionRequestLifecycleIngressRankOrdering, "
        "ExactDecisionRequestLifecycleIngressRankCarrier)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleIngressRankInCarrier",
    ): (
        "\\A node, qc, archive, request: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ ExactDecisionRequestLifecycleResidual( "
        "node, qc, archive, request) "
        "=> ExactDecisionRequestLifecycleIngressRank( "
        "node, qc, archive, request) "
        "\\in ExactDecisionRequestLifecycleIngressRankCarrier"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressProducerEpisodeBudgetIsFinite",
    ): (
        "\\A node, qc, archive, request: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ ExactDecisionRequestLifecycleResidual( "
        "node, qc, archive, request) "
        "=> /\\ IsFiniteSet( "
        "ExactDecisionRequestIngressProducerEpisodeOwnerSet( "
        "node, qc, archive, request)) "
        "/\\ ExactDecisionRequestIngressProducerEpisodeBudget( "
        "node, qc, archive, request) \\in Nat "
        "/\\ ExactDecisionRequestIngressProducerEpisodeBudget( "
        "node, qc, archive, request) "
        "<= ExactDecisionRequestIngressProducerEpisodeStaticBound"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleFrozenOwnerServiceConsumesBudget",
    ): (
        "\\A node, qc, archive, request: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ ExactDecisionRequestLifecycleResidual( "
        "node, qc, archive, request) "
        "/\\ ExactDecisionRequestLifecycleFrozenOwnerServiceAction( "
        "archive, request) "
        "/\\ ExactDecisionRequestLifecycleResidual( "
        "node, qc, archive, request)' "
        "=> ExactDecisionRequestIngressProducerEpisodeBudget( "
        "node, qc, archive, request)' "
        "< ExactDecisionRequestIngressProducerEpisodeBudget( "
        "node, qc, archive, request)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleFrozenOwnersDoNotReplenish",
    ): (
        "\\A node, qc, archive, request: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ ExactDecisionRequestLifecycleResidual( "
        "node, qc, archive, request) "
        "/\\ AsyncNext "
        "/\\ ExactDecisionRequestLifecycleResidual( "
        "node, qc, archive, request)' "
        "=> ExactDecisionRequestLifecycleFrozenPredecessorSet( "
        "archive, request)' "
        "\\subseteq ExactDecisionRequestLifecycleFrozenPredecessorSet( "
        "archive, request)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleOrdinalCannotResurrect",
    ): (
        "\\A node, qc, archive, request: "
        "LET identity == "
        "ExactDecisionServeLifecycleIdentity(archive, request) "
        "IN /\\ AsyncStrongTypeInvariant "
        "/\\ ExactDecisionRequestLifecycleResidual( "
        "node, qc, archive, request) "
        "/\\ AsyncServeLifecycleOwned(archive, identity) "
        "/\\ AsyncNext "
        "/\\ AsyncServeLifecycleOwned(archive, identity)' "
        "=> AsyncServeAdmissionOrdinal(archive, identity)' "
        "= AsyncServeAdmissionOrdinal(archive, identity)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleStepClassificationIsExhaustive",
    ): (
        "\\A node, qc, archive, request: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ ExactDecisionRequestLifecycleResidual( "
        "node, qc, archive, request) "
        "/\\ AsyncNext "
        "=> ExactDecisionRequestLifecycleStepClassification( "
        "node, qc, archive, request)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleSelectedActionEnabledAtEpisode",
    ): (
        "\\A node, qc, archive, request, rank, budget: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateProducerContinuationExternalCoverageInvariant "
        "/\\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant "
        "/\\ ExactDecisionRequestIngressContinuationPrefixCleared(archive) "
        "/\\ ExactDecisionRequestLifecycleAtRankAndBudget( "
        "node, qc, archive, request, rank, budget) "
        "/\\ ~ExactDecisionRequestLifecycleRankGoal( "
        "node, qc, archive, request, rank) "
        "=> ENABLED "
        "<<ExactDecisionRequestLifecycleSelectedConcreteFairAction( "
        "archive, request)>>_AsyncAllVars"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleBracketStepPreservesEpisodeOrGoal",
    ): (
        "\\A node, qc, archive, request, rank, budget: "
        "/\\ ExactDecisionRequestLifecycleStepClassification( "
        "node, qc, archive, request) "
        "/\\ ExactDecisionRequestLifecycleAtRankAndBudget( "
        "node, qc, archive, request, rank, budget) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "=> \\/ ExactDecisionRequestLifecycleAtRankAndBudget( "
        "node, qc, archive, request, rank, budget)' "
        "\\/ ExactDecisionRequestLifecycleRankGoal( "
        "node, qc, archive, request, rank)' "
        "\\/ \\E lowerBudget \\in SetLessThan( "
        "budget, OpToRel(<, Nat), Nat): "
        "ExactDecisionRequestLifecycleAtRankAndBudget( "
        "node, qc, archive, request, rank, lowerBudget)'"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleConcreteOwnerPersistsInRankCell",
    ): (
        "\\A node, qc, archive, request, rank, budget: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ ExactDecisionRequestLifecycleAtRankAndBudget( "
        "node, qc, archive, request, rank, budget) "
        "/\\ ~ExactDecisionRequestLifecycleRankGoal( "
        "node, qc, archive, request, rank) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ ExactDecisionRequestLifecycleAtRankAndBudget( "
        "node, qc, archive, request, rank, budget)' "
        "=> ExactDecisionRequestLifecycleConcreteFairOwner( "
        "archive, request)' "
        "= ExactDecisionRequestLifecycleConcreteFairOwner( "
        "archive, request)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleSelectedActionConsumesEpisode",
    ): (
        "\\A node, qc, archive, request, rank, budget: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateProducerContinuationExternalCoverageInvariant "
        "/\\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant "
        "/\\ ExactDecisionRequestIngressContinuationPrefixCleared(archive) "
        "/\\ ExactDecisionRequestLifecycleAtRankAndBudget( "
        "node, qc, archive, request, rank, budget) "
        "/\\ ~ExactDecisionRequestLifecycleRankGoal( "
        "node, qc, archive, request, rank) "
        "/\\ <<ExactDecisionRequestLifecycleSelectedConcreteFairAction( "
        "archive, request)>>_AsyncAllVars "
        "=> ExactDecisionRequestLifecycleRankCellOutcome( "
        "node, qc, archive, request, rank, budget)'"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleConcreteOwnerUsesAsyncFairness",
    ): (
        "\\A initialContext, archive, ownerKind: "
        "/\\ archive \\in AsyncVotersAt(initialContext) "
        "/\\ archive \\in Responsive "
        "/\\ ownerKind "
        "\\in ExactDecisionRequestLifecycleConcreteFairOwnerKinds "
        "=> AsyncSpecAt(initialContext) "
        "=> WF_AsyncAllVars( "
        "ExactDecisionRequestLifecycleConcreteFairAction( "
        "archive, ownerKind))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "AsyncSpecProvidesExactDecisionRequestLifecycleConcreteActionOrigin",
    ): (
        "\\A initialContext: "
        "AsyncSpecAt(initialContext) "
        "=> ExactDecisionRequestLifecycleConcreteActionOriginProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "AsyncSpecProvidesExactDecisionRequestLifecycleRankDescent",
    ): (
        "\\A initialContext: "
        "ExactDecisionRequestLifecycleRankDescentProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestRankDescentDerivesFiniteEpisodeClosure",
    ): (
        "\\A initialContext: "
        "/\\ ExactDecisionRequestLifecycleRankDescentProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ ExactDecisionRequestIngressContinuationPrefixClosureProperty( "
        "AsyncSpecAt(initialContext)) "
        "=> ExactDecisionRequestLifecycleFiniteProducerEpisodeClosureProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestFiniteProducerEpisodeClosesAtRank",
    ): (
        "\\A initialContext: "
        "/\\ ExactDecisionRequestLifecycleRankDescentProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ ExactDecisionRequestIngressContinuationPrefixClosureProperty( "
        "AsyncSpecAt(initialContext)) "
        "=> ExactDecisionRequestLifecycleRankCellClosureProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleRankDescentClosesLifecycle",
    ): (
        "\\A initialContext: "
        "/\\ ExactDecisionRequestLifecycleRankDescentProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ ExactDecisionRequestIngressContinuationPrefixClosureProperty( "
        "AsyncSpecAt(initialContext)) "
        "=> ExactDecisionRequestLifecycleConvergenceProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressReplenishmentHasConcreteActionWitness",
    ): (
        "\\A node, qc, archive, request: /\\ AsyncStrongTypeInvariant /\\ "
        "ExactDecisionRequestIngressRankReplenishmentResidual( node, qc, "
        "archive, request) => \\E producerClass \\in "
        "ExactDecisionRequestIngressProducerClasses: ENABLED "
        "<<ExactDecisionRequestIngressConcreteReplenishmentAction( node, qc, "
        "archive, request, producerClass)>>_AsyncAllVars"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestAdmissionCoalescingOutcomeIsDischarged",
    ): (
        "\\A initialContext: "
        "ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestAdmissionCoalescingClosesLaneRunner",
    ): (
        "\\A initialContext: "
        "ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty( "
        "AsyncSpecAt(initialContext)) "
        "=> ExactDecisionRequestIngressLaneRunnerConvergenceProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestEmissionKernelsDischargeResidual",
    ): (
        "\\A initialContext: "
        "/\\ ExactDecisionRequestClockOwnerConvergenceProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ ExactDecisionRequestRuntimePrefixConvergenceProperty( "
        "AsyncSpecAt(initialContext)) "
        "=> ExactDecisionRequestPacketEmissionResidualConvergenceProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressKernelsDischargeResidual",
    ): (
        "\\A initialContext: "
        "/\\ ExactDecisionRequestHeadGateOwnerConvergenceProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty( "
        "AsyncSpecAt(initialContext)) "
        "=> ExactDecisionRequestIngressResidualConvergenceProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionServeResponseResidualConvergence",
    ): (
        "\\A initialContext: "
        "ExactDecisionServeResponseResidualConvergenceProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionResponseClaimKernelNarrowsNonPhysicalResidual",
    ): (
        "\\A initialContext: "
        "ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergenceProperty( "
        "AsyncSpecAt(initialContext)) "
        "=> ExactDecisionResponseNonPhysicalHeadGateOwnerConvergenceProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionResponsePhysicalKernelNarrowsHeadGateResidual",
    ): (
        "\\A initialContext: "
        "ExactDecisionResponseNonPhysicalHeadGateOwnerConvergenceProperty( "
        "AsyncSpecAt(initialContext)) "
        "=> ExactDecisionResponseHeadGateOwnerConvergenceProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionResponseAdmissionKernelsDischargeResidual",
    ): (
        "\\A initialContext: "
        "ExactDecisionResponseHeadGateOwnerConvergenceProperty( "
        "AsyncSpecAt(initialContext)) "
        "=> ExactDecisionResponseAdmissionResidualConvergenceProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionOffSchedulerResidualsDischargeKernels",
    ): (
        "\\A initialContext: "
        "ExactDecisionOffSchedulerResidualConvergenceProperty( "
        "AsyncSpecAt(initialContext)) "
        "=> /\\ ExactDecisionRequestPacketEmissionKernelProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ ExactDecisionRequestIngressKernelProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ ExactDecisionServeResponseKernelProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ ExactDecisionResponseAdmissionKernelProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionOffSchedulerResidualConvergenceDischargesStageService",
    ): (
        "\\A initialContext: "
        "/\\ ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ ExactDecisionOffSchedulerResidualConvergenceProperty( "
        "AsyncSpecAt(initialContext)) "
        "=> ExactDecisionStageServiceProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionResidualKernelsDischargeStageService",
    ): (
        "\\A initialContext: "
        "/\\ ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ ExactDecisionStage2BusyClosureProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ ExactDecisionRequestPacketEmissionKernelProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ ExactDecisionRequestIngressKernelProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ ExactDecisionServeResponseKernelProperty( "
        "AsyncSpecAt(initialContext)) "
        "/\\ ExactDecisionResponseAdmissionKernelProperty( "
        "AsyncSpecAt(initialContext)) "
        "=> ExactDecisionStageServiceProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank",
    ): (
        "\\A packet \\in OverdueResponsivePackets, "
        "recipient \\in Responsive, source \\in AsyncIngressSources: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ packet.item.envelope.recipient = recipient "
        "/\\ packet.item.source = source "
        "/\\ PostGstAdmitHiddenPacket(recipient, source) "
        "=> ExactDecisionTargetNeutralPacketDependencyRank(packet)' "
        "\\in SetLessThan( "
        "ExactDecisionTargetNeutralPacketDependencyRank(packet), "
        "HistoricalDiscoveryPacketDependencyOrdering, "
        "HistoricalDiscoveryPacketDependencyCarrier)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets",
    ): (
        "\\A clockValue \\in Nat: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ asyncNow = clockValue "
        "/\\ asyncNow' = clockValue "
        "=> HistoricalDiscoveryDuePacketsAt(clockValue)' "
        "\\subseteq HistoricalDiscoveryDuePacketsAt(clockValue)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralLaterWorkCannotAcquirePredecessor",
    ): (
        "\\A snapshot, mode, node, qc, archive, "
        "request, response, packet: "
        "\\A clockValue \\in Nat: "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ ExactDecisionTargetNeutralFixedClockPending( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ ExactDecisionTargetNeutralFixedClockPending( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue)' "
        "=> /\\ HistoricalDiscoveryDuePacketsAt(clockValue)' "
        "\\subseteq snapshot.packets "
        "/\\ (ExactDecisionTargetNeutralFixedPredecessorSet(clockValue)' "
        "\\cap snapshot.predecessors) "
        "\\subseteq "
        "(ExactDecisionTargetNeutralFixedPredecessorSet(clockValue) "
        "\\cap snapshot.predecessors) "
        "/\\ ((ExactDecisionTargetNeutralLiveProducerIdentitySet' "
        "\\ ExactDecisionTargetNeutralLiveProducerIdentitySet) "
        "\\cap (snapshot.candidateIdentities "
        "\\cup snapshot.serveIdentities)) = {}"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralNonDescentConsumesOrdinal",
    ): (
        "\\A snapshot, mode, node, qc, archive, "
        "request, response, packet: "
        "\\A clockValue \\in Nat, "
        "sourceRank \\in ExactDecisionTargetNeutralFixedClockCarrier, "
        "budget \\in Nat: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ ExactDecisionTargetNeutralProducerEpisodeAtBudget( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ ~ExactDecisionTargetNeutralFixedClockStrictRankGoal( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank)' "
        "/\\ ExactDecisionTargetNeutralProducerPrefix( "
        "ExactDecisionTargetNeutralConcreteFixedClockRank(clockValue)') "
        "= ExactDecisionTargetNeutralProducerPrefix(sourceRank) "
        "/\\ ExactDecisionTargetNeutralLiveProducerIdentitySet' "
        "# ExactDecisionTargetNeutralLiveProducerIdentitySet "
        "=> ExactDecisionTargetNeutralProducerEpisodeBudget(snapshot)' "
        "< budget"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralRankCellHasConcreteFairOwner",
    ): (
        "\\A initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet: "
        "\\A clockValue \\in Nat, "
        "sourceRank \\in ExactDecisionTargetNeutralFixedClockCarrier, "
        "budget \\in Nat: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ AsyncCandidateProducerContinuationExternalCoverageInvariant "
        "/\\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant "
        "/\\ PostGstReplayQuarantineExcluded "
        "/\\ initialContext \\in ContextRecords "
        "/\\ AsyncCurrentResponsiveVoters = "
        "AsyncVotersAt(initialContext) "
        "/\\ ExactDecisionTargetNeutralProducerEpisodeAtBudget( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget) "
        "=> \\E owner \\in "
        "ExactDecisionTargetNeutralFairOwnerSet(initialContext): "
        "ExactDecisionTargetNeutralOwnerReadyForRankCell( "
        "initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet, clockValue, "
        "sourceRank, budget, owner)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness",
    ): (
        "\\A initialContext, owner: "
        "owner \\in ExactDecisionTargetNeutralFairOwnerSet(initialContext) "
        "=> AsyncSpecAt(initialContext) "
        "=> WF_AsyncAllVars( "
        "ExactDecisionTargetNeutralFairAction(owner))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFiniteEpisodeClosesRankCell",
    ): (
        "\\A initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet: "
        "\\A clockValue \\in Nat, "
        "sourceRank \\in ExactDecisionTargetNeutralFixedClockCarrier: "
        "AsyncSpecAt(initialContext) "
        "=> \\A budget \\in Nat: "
        "ExactDecisionTargetNeutralProducerEpisodeAtBudget( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget) "
        "~> ExactDecisionTargetNeutralFixedClockStrictRankGoal( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockRankStep",
    ): (
        "\\A initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet: "
        "\\A clockValue \\in Nat, "
        "sourceRank \\in ExactDecisionTargetNeutralFixedClockCarrier: "
        "AsyncSpecAt(initialContext) "
        "=> (ExactDecisionTargetNeutralFixedClockBlockedAtRank( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank) "
        "~> ExactDecisionTargetNeutralFixedClockStrictRankGoal( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockConverges",
    ): (
        "\\A initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet: "
        "\\A clockValue \\in Nat: "
        "AsyncSpecAt(initialContext) "
        "=> ExactDecisionTargetNeutralFixedClockPending( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue) "
        "~> ExactDecisionTargetNeutralFixedClockExit( "
        "mode, node, qc, archive, request, response, "
        "packet, clockValue)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralDueHeadDisablesTick",
    ): (
        '\\A mode \\in {"RequestHead", "ResponseHead"}: '
        "\\A node, qc, archive, request, response, packet: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ DecisionFrontierUniquenessInvariant "
        "/\\ DecisionTimeoutFrontierInvariant "
        "/\\ ResponsiveRecoveryValidationClearedInvariant "
        "/\\ FinalProgressWitnessClosureInvariant "
        "/\\ ExactDecisionFanoutRetentionInvariant "
        "/\\ ExactDecisionRequestAuthorityIsolationInvariant "
        "/\\ gst "
        "/\\ ExactDecisionTargetNeutralResidual( "
        "mode, node, qc, archive, request, response, packet) "
        "/\\ ~ExactDecisionTargetNeutralGoal( "
        "mode, node, qc, archive, request, response, packet) "
        "/\\ packet.deadline <= asyncNow "
        "=> /\\ packet \\in OverdueResponsivePackets "
        "/\\ ~AsyncTickEnabled "
        "/\\ ~ENABLED <<AsyncTick>>_AsyncAllVars"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockLowersDeadlineBudget",
    ): (
        "\\A initialContext, mode, node, qc, archive, "
        "request, response, packet, budget: "
        "AsyncSpecAt(initialContext) "
        "=> (ExactDecisionTargetNeutralClockBudgetFrontier( "
        "mode, node, qc, archive, request, response, packet, budget) "
        "~> ExactDecisionTargetNeutralClockBudgetGoal( "
        "mode, node, qc, archive, request, response, packet, budget))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralDeadlineBudgetConverges",
    ): (
        "\\A initialContext, mode, node, qc, archive, "
        "request, response, packet: "
        "AsyncSpecAt(initialContext) "
        "=> \\A budget \\in Nat: "
        "ExactDecisionTargetNeutralClockBudgetFrontier( "
        "mode, node, qc, archive, request, response, packet, budget) "
        "~> ExactDecisionTargetNeutralGoal( "
        "mode, node, qc, archive, request, response, packet)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralResidualReachesDeadlineOrGoal",
    ): (
        "\\A initialContext, mode, node, qc, archive, "
        "request, response, packet: "
        "AsyncSpecAt(initialContext) "
        "=> (ExactDecisionTargetNeutralResidual( "
        "mode, node, qc, archive, request, response, packet) "
        "~> (ExactDecisionTargetNeutralGoal( "
        "mode, node, qc, archive, request, response, packet) "
        "\\/ asyncNow >= ExactDecisionTargetNeutralDeadline( "
        "mode, node, packet)))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralDueHeadReachesReadyGoal",
    ): (
        "\\A initialContext, mode, node, qc, archive, "
        "request, response, packet: "
        '/\\ mode \\in {"RequestHead", "ResponseHead"} '
        "/\\ AsyncSpecAt(initialContext) "
        "=> (/\\ ExactDecisionTargetNeutralResidual( "
        "mode, node, qc, archive, request, response, packet) "
        "/\\ packet.deadline <= asyncNow) "
        "~> ExactDecisionTargetNeutralGoal( "
        "mode, node, qc, archive, request, response, packet)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestClockOwnerConvergence",
    ): (
        "\\A initialContext: "
        "ExactDecisionRequestClockOwnerConvergenceProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestHeadGateOwnerConvergence",
    ): (
        "\\A initialContext: "
        "ExactDecisionRequestHeadGateOwnerConvergenceProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergence",
    ): (
        "\\A initialContext: "
        "ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergenceProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionAsyncInitEstablishesCandidateTombstones",
    ): (
        "\\A initialContext: "
        "AsyncInitAt(initialContext) "
        "=> AsyncCandidateServiceLifecycleInvariant"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionAsyncNextPreservesCandidateTombstones",
    ): (
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ AsyncNext "
        "=> AsyncCandidateServiceLifecycleInvariant'"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionAsyncSpecAlwaysCandidateTombstones",
    ): (
        "\\A initialContext: "
        "AsyncSpecAt(initialContext) "
        "=> []AsyncCandidateServiceLifecycleInvariant"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionSameGenerationCandidateServiceCannotReactivateAtGst",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ AsyncCandidateTransientServiceActive(candidate) "
        "/\\ candidate.consumerGeneration = generation[candidate.node] "
        "/\\ gst "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ ~AsyncCandidateTransientMarkerExitThisStep(candidate) "
        "=> /\\ ~AsyncCandidateTransientServiceMarked(candidate)' "
        "/\\ ~CandidateScheduled(candidate)'"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTerminalCandidateDiscardCannotReactivateAtGst",
    ): (
        "\\A identity \\in AsyncCandidateAdmissionIdentitySet: "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ AsyncCandidateTerminalIdentityTombstoned(identity.service) "
        "/\\ identity \\notin AsyncScheduledCandidateAdmissionIdentities "
        "/\\ gst "
        "/\\ [AsyncNext]_AsyncAllVars "
        "=> /\\ AsyncCandidateAdmissionIdentityTerminallyCovered(identity)' "
        "/\\ identity \\notin AsyncScheduledCandidateAdmissionIdentities'"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionResponsiveRestartPermitsNonterminalCandidateReconstruction",
    ): (
        "\\A item \\in AsyncNetworkItems: "
        "LET candidate == DeliveryCandidate(item) "
        "IN /\\ candidate.node = asyncRecoveryNode "
        "/\\ AsyncCandidateTransientServiceActive(candidate) "
        "/\\ ~AsyncCandidateTerminalTombstoned(candidate) "
        "/\\ PreGstResponsiveReplay "
        "/\\ AsyncControlServiceSlotTransition "
        "=> /\\ ~AsyncCandidateTransientServiceMarked(candidate)' "
        "/\\ ~AsyncCandidateServicePacketRetired(item)'"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralSnapshotIsFinite",
    ): (
        "\\A clockValue \\in Nat: "
        "AsyncStrongTypeInvariant "
        "=> LET snapshot == "
        "ExactDecisionTargetNeutralFixedClockSnapshot(clockValue) "
        "IN /\\ IsFiniteSet(snapshot.packets) "
        "/\\ IsFiniteSet(snapshot.predecessors) "
        "/\\ IsFiniteSet(snapshot.candidateIdentities) "
        "/\\ IsFiniteSet(snapshot.serveIdentities) "
        "/\\ AsyncCandidateProducerEpisodeBudget "
        "\\in Nat \\ {0} "
        "/\\ AsyncServeLifecycleFamilyBudget "
        "\\in Nat \\ {0} "
        "/\\ \\A node \\in Responsive: "
        "/\\ snapshot.candidateCeiling[node] = "
        "snapshot.candidateStart[node] "
        "+ AsyncCandidateProducerEpisodeBudget "
        "/\\ snapshot.serveCeiling[node] = "
        "snapshot.serveStart[node] "
        "+ AsyncServeLifecycleFamilyBudget "
        "/\\ snapshot.candidateStart[node] = "
        "AsyncNextCandidateServiceOrdinal(node) "
        "/\\ snapshot.serveStart[node] = "
        "asyncNextServeAdmissionOrdinal[node]"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralEpisodeBudgetIsNatural",
    ): (
        "\\A snapshot: "
        "\\A clockValue \\in Nat: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ ExactDecisionTargetNeutralSnapshotActive("
        "snapshot, clockValue) "
        "=> ExactDecisionTargetNeutralProducerEpisodeBudget(snapshot) "
        "\\in Nat"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralServeOrdinalAdvanceLowersFrozenPacketRank",
    ): (
        "\\A snapshot, mode, node, qc, archive, "
        "request, response, packet: "
        "\\A clockValue \\in Nat, "
        "sourceRank \\in ExactDecisionTargetNeutralFixedClockCarrier, "
        "budget \\in Nat: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ DecisionFrontierUniquenessInvariant "
        "/\\ DecisionTimeoutFrontierInvariant "
        "/\\ ResponsiveRecoveryValidationClearedInvariant "
        "/\\ FinalProgressWitnessClosureInvariant "
        "/\\ ExactDecisionFanoutRetentionInvariant "
        "/\\ ExactDecisionRequestAuthorityIsolationInvariant "
        "/\\ ExactDecisionTargetNeutralProducerEpisodeAtBudget( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ \\E recipient \\in Responsive: "
        "asyncNextServeAdmissionOrdinal[recipient]' "
        "> asyncNextServeAdmissionOrdinal[recipient] "
        "=> /\\ \\E recipient \\in Responsive, "
        "source \\in AsyncIngressSources: "
        "AdmitIngressPacket(recipient, source) "
        "/\\ (ExactDecisionTargetNeutralFixedClockStrictRankGoal( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank))'"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralOrdinalCeilingsCarryUntilStrictRankGoal",
    ): (
        "\\A snapshot, mode, node, qc, archive, "
        "request, response, packet: "
        "\\A clockValue \\in Nat, "
        "sourceRank \\in ExactDecisionTargetNeutralFixedClockCarrier, "
        "budget \\in Nat: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ ExactDecisionTargetNeutralProducerEpisodeAtBudget( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ ~(ExactDecisionTargetNeutralFixedClockStrictRankGoal( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank))' "
        "=> /\\ (ExactDecisionTargetNeutralSnapshotActive( "
        "snapshot, clockValue))' "
        "/\\ \\A responsiveNode \\in Responsive: "
        "/\\ snapshot.candidateStart[responsiveNode] "
        "<= AsyncNextCandidateServiceOrdinal(responsiveNode)' "
        "/\\ AsyncNextCandidateServiceOrdinal(responsiveNode)' "
        "<= snapshot.candidateCeiling[responsiveNode] "
        "/\\ snapshot.serveStart[responsiveNode] "
        "<= asyncNextServeAdmissionOrdinal[responsiveNode]' "
        "/\\ asyncNextServeAdmissionOrdinal[responsiveNode]' "
        "<= snapshot.serveCeiling[responsiveNode]"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralNonGoalEpisodeHasRemainingOrdinal",
    ): (
        "\\A snapshot, mode, node, qc, archive, "
        "request, response, packet: "
        "\\A clockValue \\in Nat, "
        "sourceRank \\in ExactDecisionTargetNeutralFixedClockCarrier, "
        "budget \\in Nat: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ ExactDecisionTargetNeutralProducerEpisodeAtBudget( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ ~(ExactDecisionTargetNeutralFixedClockStrictRankGoal( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank))' "
        "=> ExactDecisionTargetNeutralProducerEpisodeBudget(snapshot)' "
        "\\in Nat \\ {0}"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralPacketDependencyRankInCarrier",
    ): (
        "\\A packet \\in OverdueResponsivePackets: "
        "AsyncStrongTypeInvariant "
        "=> ExactDecisionTargetNeutralPacketDependencyRank(packet) "
        "\\in HistoricalDiscoveryPacketDependencyCarrier"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralConcreteRankInCarrier",
    ): (
        "\\A snapshot, mode, node, qc, archive, "
        "request, response, packet: "
        "\\A clockValue \\in Nat: "
        "ExactDecisionTargetNeutralFixedClockPending( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue) "
        "=> ExactDecisionTargetNeutralConcreteFixedClockRank(clockValue) "
        "\\in ExactDecisionTargetNeutralFixedClockCarrier"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockOrderingIsWellFounded",
    ): (
        "IsWellFoundedOn( "
        "ExactDecisionTargetNeutralFixedClockOrdering, "
        "ExactDecisionTargetNeutralFixedClockCarrier)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralSelectedOwnerIsReady",
    ): (
        "\\A initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet: "
        "\\A clockValue \\in Nat, "
        "sourceRank \\in ExactDecisionTargetNeutralFixedClockCarrier, "
        "budget \\in Nat: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ AsyncCandidateProducerContinuationExternalCoverageInvariant "
        "/\\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant "
        "/\\ PostGstReplayQuarantineExcluded "
        "/\\ initialContext \\in ContextRecords "
        "/\\ AsyncCurrentResponsiveVoters = "
        "AsyncVotersAt(initialContext) "
        "/\\ ExactDecisionTargetNeutralProducerEpisodeAtBudget( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget) "
        "=> ExactDecisionTargetNeutralOwnerReadyForRankCell( "
        "initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet, clockValue, sourceRank, budget, "
        "ExactDecisionTargetNeutralSelectedFairOwner( "
        "initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet, clockValue, "
        "sourceRank, budget))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralRankCellStepIsSafe",
    ): (
        "\\A initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet: "
        "\\A clockValue \\in Nat, "
        "sourceRank \\in ExactDecisionTargetNeutralFixedClockCarrier, "
        "budget \\in Nat: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ DecisionFrontierUniquenessInvariant "
        "/\\ DecisionTimeoutFrontierInvariant "
        "/\\ ResponsiveRecoveryValidationClearedInvariant "
        "/\\ FinalProgressWitnessClosureInvariant "
        "/\\ ExactDecisionFanoutRetentionInvariant "
        "/\\ ExactDecisionRequestAuthorityIsolationInvariant "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ AsyncCurrentResponsiveVoters = "
        "AsyncVotersAt(initialContext) "
        "/\\ ExactDecisionTargetNeutralProducerEpisodeAtBudget( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "=> \\/ ExactDecisionTargetNeutralRankCellOutcome( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget)' "
        "\\/ /\\ ExactDecisionTargetNeutralProducerEpisodeAtBudget( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget)' "
        "/\\ ExactDecisionTargetNeutralSelectedFairOwner( "
        "initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet, clockValue, "
        "sourceRank, budget)' "
        "= ExactDecisionTargetNeutralSelectedFairOwner( "
        "initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet, clockValue, sourceRank, budget)"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralSelectedOwnerConsumesRankCell",
    ): (
        "\\A initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet: "
        "\\A clockValue \\in Nat, "
        "sourceRank \\in ExactDecisionTargetNeutralFixedClockCarrier, "
        "budget \\in Nat: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ AsyncCandidateProducerContinuationExternalCoverageInvariant "
        "/\\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant "
        "/\\ PostGstReplayQuarantineExcluded "
        "/\\ initialContext \\in ContextRecords "
        "/\\ AsyncCurrentResponsiveVoters = "
        "AsyncVotersAt(initialContext) "
        "/\\ ExactDecisionTargetNeutralProducerEpisodeAtBudget( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget) "
        "/\\ <<ExactDecisionTargetNeutralFairAction( "
        "ExactDecisionTargetNeutralSelectedFairOwner( "
        "initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet, clockValue, "
        "sourceRank, budget))>>_AsyncAllVars "
        "=> ExactDecisionTargetNeutralRankCellOutcome( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget)'"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralLastOrdinalForcesStrictRankGoal",
    ): (
        "\\A initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet: "
        "\\A clockValue \\in Nat, "
        "sourceRank \\in ExactDecisionTargetNeutralFixedClockCarrier: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ AsyncCandidateProducerContinuationExternalCoverageInvariant "
        "/\\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant "
        "/\\ PostGstReplayQuarantineExcluded "
        "/\\ initialContext \\in ContextRecords "
        "/\\ AsyncCurrentResponsiveVoters = "
        "AsyncVotersAt(initialContext) "
        "/\\ ExactDecisionTargetNeutralProducerEpisodeAtBudget( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, 1) "
        "/\\ <<ExactDecisionTargetNeutralFairAction( "
        "ExactDecisionTargetNeutralSelectedFairOwner( "
        "initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet, clockValue, "
        "sourceRank, 1))>>_AsyncAllVars "
        "=> ExactDecisionTargetNeutralFixedClockStrictRankGoal( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank)'"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFairEpisodeStep",
    ): (
        "\\A initialContext, snapshot, mode, node, qc, archive, "
        "request, response, packet: "
        "\\A clockValue \\in Nat, "
        "sourceRank \\in ExactDecisionTargetNeutralFixedClockCarrier, "
        "budget \\in Nat: "
        "AsyncSpecAt(initialContext) "
        "=> (ExactDecisionTargetNeutralProducerEpisodeAtBudget( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget) "
        "~> ExactDecisionTargetNeutralRankCellOutcome( "
        "snapshot, mode, node, qc, archive, request, response, "
        "packet, clockValue, sourceRank, budget))"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralNonTickNonRunnerStepLeavesClock",
    ): (
        "/\\ AsyncNonRunnerStep "
        "/\\ ~AsyncTick "
        "=> asyncNow' = asyncNow"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralNonTickAsyncNextLeavesClock",
    ): (
        "/\\ AsyncNext "
        "/\\ ~AsyncTick "
        "=> asyncNow' = asyncNow"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralEveryNonTickSourceStepLeavesClock",
    ): (
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ ~AsyncTick "
        "=> asyncNow' = asyncNow"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralDueHeadStepLeavesClockOrGoals",
    ): (
        '\\A mode \\in {"RequestHead", "ResponseHead"}: '
        "\\A node, qc, archive, request, response, packet: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ DecisionFrontierUniquenessInvariant "
        "/\\ DecisionTimeoutFrontierInvariant "
        "/\\ ResponsiveRecoveryValidationClearedInvariant "
        "/\\ FinalProgressWitnessClosureInvariant "
        "/\\ ExactDecisionFanoutRetentionInvariant "
        "/\\ ExactDecisionRequestAuthorityIsolationInvariant "
        "/\\ gst "
        "/\\ ExactDecisionTargetNeutralResidual( "
        "mode, node, qc, archive, request, response, packet) "
        "/\\ ~ExactDecisionTargetNeutralGoal( "
        "mode, node, qc, archive, request, response, packet) "
        "/\\ packet.deadline <= asyncNow "
        "/\\ [AsyncNext]_AsyncAllVars "
        "=> \\/ ExactDecisionTargetNeutralGoal( "
        "mode, node, qc, archive, request, response, packet)' "
        "\\/ /\\ ExactDecisionTargetNeutralResidual( "
        "mode, node, qc, archive, request, response, packet)' "
        "/\\ asyncNow' = asyncNow"
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "ExactDecisionStageServiceObligation",
    ): (
        "\\A initialContext: "
        "ExactDecisionStageServiceProperty(AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureApplicationCompletionProgressObligation",
    ): (
        "\\A initialContext: ApplicationCompletionProgressProperty("
        " AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureTimeoutViewProgressReduction",
    ): (
        "/\\ (\\A initialContext: "
        "ProtectedServiceFiniteRunnerEpisodeClosureProperty("
        " AsyncSpecAt(initialContext))) "
        "/\\ DirectTimeoutViewClosureResidualObligation "
        "=> AsyncTemporalClosureTimeoutViewProgressObligation"
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureRotatingLeaderProgressObligation",
    ): (
        "\\A initialContext: RotatingLeaderProgressProperty("
        " AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "ResponsiveDecisionConvergenceClosesLockedBodyReproposal",
    ): (
        "\\A initialContext: ResponsiveDecisionConvergenceProperty("
        " AsyncLiveSpecAt(initialContext)) "
        "=> LockedBodyReproposalProgressProperty("
        " AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureLockedBodyReproposalProgressObligation",
    ): (
        "\\A initialContext: LockedBodyReproposalProgressProperty("
        " AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "DirectRetainedLockDecompositionReachesOutcomeOrHigherLeader",
    ): (
        "\\A initialContext: "
        "/\\ TimeoutViewProgressProperty( AsyncLiveSpecAt(initialContext)) "
        "/\\ RetainedLockSourceAuthorityExposureProperty("
        " AsyncLiveSpecAt(initialContext)) "
        "/\\ RetainedLockPrepareAuthorityTransportProperty("
        " AsyncLiveSpecAt(initialContext)) "
        "/\\ RetainedLockTargetLeaderFreshActivationProperty("
        " AsyncLiveSpecAt(initialContext)) "
        "/\\ RetainedLockLeaderProducerOriginProperty("
        " AsyncLiveSpecAt(initialContext)) "
        "/\\ RetainedLockOwnerNeutralRankHandoffProperty("
        " AsyncLiveSpecAt(initialContext)) "
        "=> RetainedLockOutcomeOrHigherLeaderProgressProperty("
        " AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockSeparatedProducerProvidersCloseEpisodeResidual",
    ): (
        "\\A initialContext: /\\ "
        "RetainedLockSameOriginLifecycleDispositionClosureProperty( "
        "AsyncLiveSpecAt(initialContext)) /\\ "
        "RetainedLockCrossOriginProducerReplacementClosureProperty( "
        "AsyncLiveSpecAt(initialContext)) /\\ "
        "RetainedLockProducerExactReentryClosureProperty( "
        "AsyncLiveSpecAt(initialContext)) => "
        "RetainedLockProducerNonDescentEpisodeClosureProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockNonDescentClosureClosesRankHandoff",
    ): (
        "\\A initialContext: "
        "RetainedLockProducerNonDescentEpisodeClosureProperty( "
        "AsyncLiveSpecAt(initialContext)) => "
        "RetainedLockOwnerNeutralRankHandoffProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockOwnerNeutralRankHandoffClosesFixedCorridor",
    ): (
        "\\A initialContext: RetainedLockOwnerNeutralRankHandoffProperty( "
        "AsyncLiveSpecAt(initialContext)) => (AsyncLiveSpecAt(initialContext) "
        "=> \\A target, leader \\in ValidatorIds, lockedRound \\in Views, "
        "subject \\in Subjects, prepareQc \\in QcRecordSet, leaderView \\in "
        "Views, rank \\in ExactLeaderSemanticRankCarrier: "
        "RetainedLockOwnerNeutralCandidateRankFrontier( target, leader, "
        "lockedRound, subject, prepareQc, leaderView, rank) ~> "
        "(RetainedLockModeGoal( target, lockedRound, subject) "
        "\\/ RetainedLockStrictHigherFreshLeaderAuthorityFrontierFor( target, "
        "lockedRound, subject, prepareQc, leaderView)))"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockOwnerNeutralRankHandoffClosesRankedFrontier",
    ): (
        "\\A initialContext: RetainedLockOwnerNeutralRankHandoffProperty( "
        "AsyncLiveSpecAt(initialContext)) => (AsyncLiveSpecAt(initialContext) "
        "=> \\A target \\in ValidatorIds, lockedRound \\in Views, subject \\in "
        "Subjects: RetainedLockRankedFrontier( target, lockedRound, subject) "
        "~> (RetainedLockModeGoal( target, lockedRound, subject) "
        "\\/ RetainedLockStrictHigherFreshLeaderAuthorityFrontier( target, "
        "lockedRound, subject)))"
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "DirectRetainedLockOwnerNeutralDecompositionReachesHigherClosure",
    ): (
        "\\A initialContext: /\\ TimeoutViewProgressProperty( "
        "AsyncLiveSpecAt(initialContext)) /\\ "
        "RetainedLockSourceAuthorityExposureProperty( "
        "AsyncLiveSpecAt(initialContext)) /\\ "
        "RetainedLockPrepareAuthorityTransportProperty( "
        "AsyncLiveSpecAt(initialContext)) /\\ "
        "RetainedLockTargetLeaderFreshActivationProperty( "
        "AsyncLiveSpecAt(initialContext)) /\\ "
        "RetainedLockLeaderProducerOriginProperty( "
        "AsyncLiveSpecAt(initialContext)) /\\ "
        "RetainedLockOwnerNeutralRankHandoffProperty( "
        "AsyncLiveSpecAt(initialContext)) => "
        "RetainedLockOutcomeOrHigherLeaderProgressProperty( "
        "AsyncLiveSpecAt(initialContext))"
    ),
    (
        "SumeragiV2ProgressWitnessFinalClosureProofs",
        "OpenHistoricalRecoveryPreservesDecisionExactSource",
    ): (
        "\\A node \\in ValidatorIds: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ DecisionExactSourceRetentionInvariant "
        "/\\ OpenHistoricalRecovery(node) "
        "=> DecisionExactSourceRetentionInvariant'"
    ),
    (
        "SumeragiV2ProgressWitnessFinalClosureProofs",
        "OpenHistoricalRecoveryPreservesFinalProgressWitnessClosure",
    ): (
        "\\A node \\in ValidatorIds: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ FinalProgressWitnessClosureInvariant "
        "/\\ OpenHistoricalRecovery(node) "
        "=> FinalProgressWitnessClosureInvariant'"
    ),
    (
        "SumeragiV2ProgressWitnessFinalClosureProofs",
        "AsyncNonRunnerPreservesFinalProgressWitnessClosure",
    ): (
        "/\\ AsyncStrongTypeInvariant "
        "/\\ FinalProgressWitnessClosureInvariant "
        "/\\ AsyncNonRunnerStep "
        "/\\ UNCHANGED AsyncRecoveryVars "
        "=> FinalProgressWitnessClosureInvariant'"
    ),
    (
        "SumeragiV2ChainReceiptAgreementProofs",
        "IndexedChainSpecEstablishesDecisionReceiptSourceOwnership",
    ): "IndexedChainSpec => []IndexedDecisionReceiptSourceOwnership",
    (
        "SumeragiV2ChainReceiptAgreementProofs",
        "IndexedOneHeightDecisionReceiptsAgree",
    ): (
        "IndexedEveryInstanceStrongInvariant "
        "=> \\A initialContext \\in AdmissibleContextRecords: "
        "\\A left, right \\in IndexedCurrentDecisions(initialContext): "
        "left.qc.context = right.qc.context "
        "=> left.qc.subject = right.qc.subject"
    ),
    (
        "SumeragiV2ChainReceiptAgreementProofs",
        "IndexedApplicationEvidenceIsDecisionEvidence",
    ): (
        "IndexedCompositionInvariant "
        "=> durableApplicationEvidence \\subseteq durableDecisionEvidence"
    ),
    (
        "SumeragiV2ChainReceiptAgreementProofs",
        "JoinedContextsAtEqualHeightAreIdentical",
    ): (
        "JoinedContextCertificationInvariant "
        "=> \\A leftContext, rightContext \\in JoinedContexts: "
        "leftContext.height = rightContext.height "
        "=> leftContext = rightContext"
    ),
    (
        "SumeragiV2ChainReceiptAgreementProofs",
        "CompositionAndSourceOwnershipImplyExactReceiptAgreement",
    ): (
        "IndexedCompositionInvariant "
        "/\\ IndexedDecisionReceiptSourceOwnership "
        "=> ExactPerSlotDurableCommitReceiptSubjectAgreement"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncDormantExactReplyRequestPacketIsRetained",
    ): (
        "\\A packet: "
        "AsyncDormantExactReplyRequestPacket(packet) "
        "=> packet \\in asyncTransport"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncGateOpenDueResponsivePacketReentersClockDeadline",
    ): (
        "\\A packet: "
        "/\\ packet \\in asyncTransport "
        "/\\ AsyncServeTransportAdmissionGateAllows( "
        "packet.item.envelope.recipient, packet.item) "
        "/\\ packet.item.envelope.recipient "
        "\\in AsyncTimedServiceNodes "
        "/\\ \\/ packet.item.source \\in AsyncTimedServiceNodes "
        "\\/ /\\ packet.item.kind "
        '\\in {"CertifiedResponse", "CommitCertificateResponse"} '
        "/\\ IngressItemHasAuthenticatedHistory(packet.item) "
        "/\\ packet.deadline <= asyncNow "
        "=> /\\ AsyncPacketOwnsClockDeadline(packet) "
        "/\\ packet \\in OverdueResponsivePackets"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncRetainedCommitQcRetransmissionCreatesExactPacket",
    ): (
        "\\A node \\in ValidatorIds: \\A item: "
        "LET packet == PacketForItem(item) "
        "IN /\\ AsyncExactCommitQcRetainedOwner(item) "
        "/\\ item.source = node "
        "/\\ UNCHANGED vars "
        "/\\ SendNodeRetransmissions(node) "
        "=> AsyncExactCommitQcPacketOwner(item, packet)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncRetainedCommitQcPacketAdmissionCreatesExactIngressOwner",
    ): (
        "\\A item, packet: "
        "LET recipient == item.envelope.recipient "
        "IN /\\ AsyncStrongTypeInvariant "
        "/\\ AsyncExactCommitQcPacketOwner(item, packet) "
        "/\\ packet = OldestDueSourcePacket(recipient, item.source) "
        "/\\ ~IngressHasCoalescingOwner(item) "
        "/\\ ~IngressPacketPolicyRejected(item) "
        "/\\ AdmitIngressPacket(recipient, item.source) "
        "=> AsyncExactCommitQcIngressOwner(item)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncRetainedCommitQcIngressCreatesExactDeliverQcOwner",
    ): (
        "\\A item: LET node == item.envelope.recipient "
        "candidate == DeliveryCandidate(item) "
        "IN /\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncExactCommitQcIngressOwner(item) "
        "/\\ SelectedIngressItemAt( "
        "node, FirstDrainableIngressIndex(node)) = item "
        "/\\ ~AsyncControlServiceOccurrenceRetired(item) "
        "/\\ ~CandidateAdmissionCoalesced(candidate) "
        "/\\ DrainFairIngressSelected(node) "
        "=> AsyncExactCommitQcDeliverOwner(item)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncRetainedCommitQcDeliveryRecordsExactReceipt",
    ): (
        "\\A item: /\\ AsyncStrongTypeInvariant "
        "/\\ AsyncExactCommitQcRetainedOwner(item) "
        "/\\ ExecuteCoreDelivery(DeliveryCandidate(item)) "
        "=> AsyncExactCommitQcReceipt(item)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncServiceActivationActionsRefineAsyncNext",
    ): (
        "\\A node \\in ValidatorIds: "
        "(AsyncEnterIndexedServiceActivation(node) "
        "\\/ AsyncActivateServiceNode(node)) => AsyncNext"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncServeQueuedIdentityDepartureInstallsTombstone",
    ): (
        "\\A node \\in ValidatorIds, "
        "identity \\in AsyncServeLogicalRequestIdentities: "
        "/\\ AsyncServeLifecycleTypeInvariant "
        "/\\ gst "
        "/\\ AsyncServeJobQueued(node, identity) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ ~AsyncServeJobQueued(node, identity)' "
        "=> AsyncServeLifecycleTombstone(node, identity)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncServeRetiredIdentityCannotRequeueAtGst",
    ): (
        "\\A node \\in ValidatorIds, "
        "identity \\in AsyncServeLogicalRequestIdentities: "
        "/\\ AsyncServeLifecycleTypeInvariant "
        "/\\ AsyncServeLogicalIdentityRetiredOrSuperseded(node, identity) "
        "/\\ gst "
        "/\\ [AsyncNext]_AsyncAllVars "
        "=> /\\ AsyncServeLogicalIdentityRetiredOrSuperseded( "
        "node, identity)' "
        "/\\ ~AsyncServeJobQueued(node, identity)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncServeTombstonedIdentityCannotRequeueAtGst",
    ): (
        "\\A node \\in ValidatorIds, "
        "identity \\in AsyncServeLogicalRequestIdentities: "
        "/\\ AsyncServeLifecycleTypeInvariant "
        "/\\ AsyncServeLifecycleTombstone(node, identity) "
        "/\\ gst "
        "/\\ [AsyncNext]_AsyncAllVars "
        "=> /\\ AsyncServeLogicalIdentityRetiredOrSuperseded( "
        "node, identity)' "
        "/\\ ~AsyncServeJobQueued(node, identity)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTransientMarkerCoalescesFreshCandidate",
    ): (
        "\\A candidate: "
        "AsyncCandidateTransientServiceMarked(candidate) "
        "=> /\\ CandidateAdmissionCoalesced(candidate) "
        "/\\ FreshCandidateSequence(candidate) = <<>> "
        "/\\ ~ENABLED EnqueueCandidate(candidate)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTerminalTombstoneCoalescesFreshCandidate",
    ): (
        "\\A candidate: "
        "AsyncCandidateTerminalTombstoned(candidate) "
        "=> /\\ CandidateAdmissionCoalesced(candidate) "
        "/\\ FreshCandidateSequence(candidate) = <<>> "
        "/\\ ~ENABLED EnqueueCandidate(candidate)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateSuccessfulServiceInstallsTransientMarker",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncCandidateServicesThisStep = {candidate} "
        "/\\ AsyncCandidateServiceEligibleAfterStep(candidate) "
        "/\\ AsyncControlServiceSlotTransition "
        "=> /\\ AsyncCandidateTransientServiceMarked(candidate)' "
        "/\\ ~CandidateScheduled(candidate)' "
        "/\\ AsyncCandidateTransientServiceActive(candidate)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateSuccessfulServiceInstallsTombstone",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ AsyncCandidateServicesThisStep = {candidate} "
        "/\\ AsyncCandidateServiceEligibleAfterStep(candidate) "
        "/\\ AsyncControlServiceSlotTransition "
        "=> /\\ AsyncCandidateServiceTombstoned(candidate)' "
        "/\\ ~CandidateScheduled(candidate)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateSuccessfulServiceAllocatesExactOrdinal",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncCandidateServicesThisStep = {candidate} "
        "/\\ AsyncCandidateServiceEligibleAfterStep(candidate) "
        "/\\ ~AsyncCandidateServiceCoalesced(candidate) "
        "/\\ AsyncControlServiceSlotTransition "
        "=> LET node == candidate.node "
        "ordinal == AsyncNextCandidateServiceOrdinal(node) "
        "IN /\\ AsyncCandidateServiceMarker( "
        "candidate, nodeView[node], "
        "candidate.consumerGeneration, ordinal) "
        "\\in AsyncCandidateServiceMarkers' "
        "/\\ AsyncNextCandidateServiceOrdinal(node)' = ordinal + 1"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTransientMarkerPersistsWithinGeneration",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncCandidateTransientServiceActive(candidate) "
        "/\\ AsyncControlServiceSlotTransition "
        "/\\ ~AsyncCandidateTransientMarkerExitThisStep(candidate) "
        "=> AsyncCandidateTransientServiceActive(candidate)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTerminalTombstonePersistsWithoutExit",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncCandidateTerminalTombstoneActive(candidate) "
        "/\\ AsyncControlServiceSlotTransition "
        "/\\ ~AsyncCandidateTerminalTombstoneExitThisStep(candidate) "
        "=> AsyncCandidateTerminalTombstoneActive(candidate)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateSameHeightRestartPreservesServicedIdentity",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncCandidateTransientServiceActive(candidate) "
        "/\\ candidate.node = asyncRecoveryNode "
        "/\\ PreGstResponsiveReplay "
        "/\\ AsyncControlServiceSlotTransition "
        "=> ~AsyncCandidateTransientServiceMarked(candidate)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateSameHeightRestartPreservesTombstone",
    ): (
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ PreGstResponsiveReplay "
        "/\\ AsyncControlServiceSlotTransition "
        "=> /\\ AsyncCandidateServiceMarkers' = "
        "{record \\in AsyncCandidateServiceMarkers: "
        "/\\ record.node # asyncRecoveryNode "
        "/\\ AsyncCandidateServiceRecordRetainedAfterStep(record)} "
        "/\\ AsyncCandidateTerminalTombstones' = "
        "{record \\in AsyncCandidateTerminalTombstones: "
        "AsyncCandidateServiceRecordRetainedAfterStep(record)} "
        "/\\ asyncControlServiceState'.candidateServiceNextOrdinal "
        "= asyncControlServiceState.candidateServiceNextOrdinal"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTransientMarkerDoesNotSuppressRestartReplay",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncCandidateTransientServiceMarked(candidate) "
        "/\\ ~AsyncCandidateTerminalTombstoned(candidate) "
        "=> ~AsyncCandidateRestartReplayTombstoned(candidate)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncRestartScopedCandidateIsNeverReplayTombstoned",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "candidate.kind \\in AsyncRestartScopedCandidateServiceKinds "
        "=> ~AsyncCandidateRestartReplayTombstoned(candidate)"
    ),
    (
        "SumeragiV2AsyncTimeoutOwnershipProofs",
        "RestartSignatureReplayIsNeverTombstoneSuppressed",
    ): (
        "\\A node: "
        "\\A candidate \\in SequenceSet(RestartSignatureReplay(node)): "
        "~AsyncCandidateRestartReplayTombstoned(candidate)"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTerminalRetirementsThisStepIsSingleton",
    ): (
        "/\\ AsyncLogicalCandidateOwnershipInvariant "
        "/\\ AsyncNext "
        "=> Cardinality(AsyncCandidateTerminalRetirementsThisStep) <= 1"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateDiscardInstallsTerminalTombstone",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncLogicalCandidateOwnershipInvariant "
        "/\\ AsyncCandidateTerminallyDiscardedThisStep(candidate) "
        "/\\ AsyncCandidateTerminalRetirementEligibleAfterStep(candidate) "
        "/\\ AsyncNext "
        "=> AsyncCandidateTerminalTombstoned(candidate)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTerminalDiscardAllocatesExactOrdinal",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncLogicalCandidateOwnershipInvariant "
        "/\\ AsyncCandidateTerminallyDiscardedThisStep(candidate) "
        "/\\ AsyncCandidateTerminalRetirementEligibleAfterStep(candidate) "
        "/\\ ~AsyncCandidateServiceCoalesced(candidate) "
        "/\\ AsyncNext "
        "=> LET node == candidate.node "
        "ordinal == AsyncNextCandidateServiceOrdinal(node) "
        "IN /\\ AsyncCandidateServiceTombstone( "
        "candidate, nodeView[node], ordinal) "
        "\\in AsyncCandidateTerminalTombstones' "
        "/\\ AsyncNextCandidateServiceOrdinal(node)' = ordinal + 1"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateDiscardRetiresLogicalLifecycle",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncLogicalCandidateOwnershipInvariant "
        "/\\ AsyncCandidateTerminallyDiscardedThisStep(candidate) "
        "/\\ AsyncCandidateTerminalRetirementEligibleAfterStep(candidate) "
        "/\\ AsyncNext "
        "=> AsyncCandidateAdmissionIdentityTerminallyCovered( "
        "AsyncCandidateAdmissionIdentity(candidate))'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst",
    ): (
        "\\A identity \\in AsyncCandidateAdmissionIdentitySet: "
        "/\\ AsyncCandidateAdmissionIdentityObsolete(identity) "
        "/\\ gst "
        "/\\ [AsyncNext]_AsyncAllVars "
        "=> AsyncCandidateAdmissionIdentityObsolete(identity)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateObsoleteAdmissionIdentityCannotReappearAtGst",
    ): (
        "\\A identity \\in AsyncCandidateAdmissionIdentitySet: "
        "/\\ AsyncCandidateAdmissionIdentityObsolete(identity) "
        "/\\ identity \\notin AsyncScheduledCandidateAdmissionIdentities "
        "/\\ gst "
        "/\\ [AsyncNext]_AsyncAllVars "
        "=> identity \\notin AsyncScheduledCandidateAdmissionIdentities'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTerminalIdentityCannotReactivateAtGst",
    ): (
        "\\A identity \\in AsyncCandidateAdmissionIdentitySet: "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ AsyncCandidateTerminalIdentityTombstoned(identity.service) "
        "/\\ identity \\notin AsyncScheduledCandidateAdmissionIdentities "
        "/\\ gst "
        "/\\ [AsyncNext]_AsyncAllVars "
        "=> /\\ AsyncCandidateAdmissionIdentityTerminallyCovered(identity)' "
        "/\\ identity \\notin AsyncScheduledCandidateAdmissionIdentities'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncLogicalCandidateOwnershipInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ gst "
        "/\\ AsyncNext /\\ CandidateScheduled(candidate) "
        "/\\ ~CandidateScheduledAfter(candidate) => "
        "\\/ AsyncCandidateIgnoredWithoutApplicationThisStep(candidate) "
        "\\/ AsyncCandidateServiceTombstoned(candidate)' "
        "\\/ AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter(candidate) "
        "\\/ AsyncCandidateMonotoneSemanticCoverageAfterIn( "
        "asyncControlServiceState', candidate) "
        "\\/ AsyncCandidateTerminalTombstoned(candidate)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateSameGenerationServicedIdentityCannotReactivateAtGst",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ AsyncCandidateTransientServiceActive(candidate) "
        "/\\ candidate.consumerGeneration = generation[candidate.node] "
        "/\\ gst "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ ~AsyncCandidateTransientMarkerExitThisStep(candidate) "
        "=> /\\ AsyncCandidateTransientServiceActive(candidate)' "
        "/\\ ~CandidateScheduled(candidate)'"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateResponsiveRestartPermitsNonterminalReconstruction",
    ): (
        "\\A item \\in AsyncNetworkItems: "
        "LET candidate == DeliveryCandidate(item) "
        "IN /\\ candidate.node = asyncRecoveryNode "
        "/\\ AsyncCandidateTransientServiceActive(candidate) "
        "/\\ ~AsyncCandidateTerminalTombstoned(candidate) "
        "/\\ PreGstResponsiveReplay "
        "/\\ AsyncControlServiceSlotTransition "
        "=> /\\ ~AsyncCandidateTransientServiceMarked(candidate)' "
        "/\\ ~AsyncCandidateServicePacketRetired(item)'"
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFrozenLifecycleCoveragePersists",
    ): (
        "\\A snapshot: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateServiceLifecycleInvariant "
        "/\\ gst "
        "/\\ snapshot.candidateIdentities "
        "\\subseteq "
        "ExactDecisionTargetNeutralFrozenCandidateOwnerIdentitySet "
        "/\\ snapshot.serveIdentities "
        "\\subseteq ExactDecisionTargetNeutralServeOwnerIdentitySet "
        "/\\ ExactDecisionTargetNeutralFrozenCandidateLifecycleCovered("
        "snapshot) "
        "/\\ ExactDecisionTargetNeutralFrozenServeLifecycleCovered("
        "snapshot) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "=> /\\ ExactDecisionTargetNeutralFrozenCandidateLifecycleCovered( "
        "snapshot)' "
        "/\\ ExactDecisionTargetNeutralFrozenServeLifecycleCovered("
        "snapshot)'"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalServiceKernelsDischargeAuthorityReadyProgress",
    ): (
        "/\\ IndexedChainSpec "
        "/\\ IndexedHistoricalCertificateRankProgressResidualProperty "
        "/\\ IndexedHistoricalDecisionRankProgressResidualProperty "
        "=> IndexedExactHistoricalRecoveryFromAuthorityProgress"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryResidualKernelsDischargeExactProgress",
    ): (
        "/\\ IndexedLiveChainSpec "
        "/\\ IndexedLocalAdequateLeaderDecisionConvergenceProperty "
        "/\\ IndexedHistoricalRecoveryTemporalResidualKernels "
        "=> IndexedExactHistoricalRecoveryProgress"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalReleaseResidualsDischargeExactProgress",
    ): (
        "/\\ IndexedLiveChainSpec "
        "/\\ IndexedLocalAdequateLeaderDecisionConvergenceProperty "
        "=> IndexedExactHistoricalRecoveryProgress"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalFixedDeadlineDisseminationAndExposureDischargeExactProgress",
    ): (
        "/\\ IndexedLiveChainSpec "
        "/\\ IndexedLocalAdequateLeaderFixedDeadlineAnd"
        "ResponsiveDisseminationProperty "
        "/\\ IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty "
        "=> IndexedExactHistoricalRecoveryProgress"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalServeExactRetryKeepsAdmissionHighWatermark",
    ): (
        "\\A node, candidate: "
        "HistoricalTemporalServeExactRetryCoalescingAction(node, candidate) "
        "=> UNCHANGED asyncNextServeAdmissionOrdinal"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalInitEstablishesIdentityLifecycle",
    ): (
        "\\A initialContext: AsyncInitAt(initialContext) => "
        "HistoricalTemporalIdentityLifecycleInvariant"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalBracketPreservesIdentityLifecycle",
    ): (
        "/\\ AsyncStrongTypeInvariant /\\ AsyncProgressOwnershipInvariant /\\ "
        "HistoricalTemporalIdentityLifecycleInvariant /\\ "
        "[AsyncNext]_AsyncAllVars => "
        "HistoricalTemporalIdentityLifecycleInvariant'"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "AsyncSpecProvidesHistoricalTemporalCandidateIdentityBridge",
    ): (
        "\\A initialContext: "
        "HistoricalTemporalCandidateIdentityBudgetBridgeProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "AsyncSpecProvidesHistoricalTemporalServeIdentityBridge",
    ): (
        "\\A initialContext: "
        "HistoricalTemporalServeIdentityBudgetBridgeProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "AsyncSpecProvidesHistoricalTemporalCandidateServeIdentityBridge",
    ): (
        "\\A initialContext: "
        "HistoricalTemporalCandidateServeIdentityBudgetBridgeProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalTransportVariablesAreExact",
    ): (
        "IndexedAsyncStateShape => \\A initialContext \\in "
        "AdmissibleContextRecords: "
        "IndexedHistoricalTransport(initialContext)!AsyncAllVars = "
        "IndexedAsyncStateAt(initialContext)"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedInitEstablishesHistoricalTemporalSupport",
    ): (
        "IndexedChainInit => \\A initialContext \\in AdmissibleContextRecords: "
        "IndexedHistoricalTemporalSupportAt(initialContext)"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedBracketStepPreservesHistoricalTemporalSupport",
    ): (
        "/\\ \\A initialContext \\in AdmissibleContextRecords: "
        "IndexedHistoricalTemporalSupportAt(initialContext) /\\ "
        "[IndexedChainNext]_IndexedChainVars => \\A initialContext \\in "
        "AdmissibleContextRecords: "
        "IndexedHistoricalTemporalSupportAt(initialContext)'"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecAlwaysHistoricalTemporalSupport",
    ): (
        "IndexedChainSpec => [](\\A initialContext \\in "
        "AdmissibleContextRecords: "
        "IndexedHistoricalTemporalSupportAt(initialContext))"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesHistoricalFixedClockIdentityBridge",
    ): (
        "IndexedChainSpec => "
        "IndexedHistoricalFixedClockIdentityBridgeProperty"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesHistoricalPostGstTickFairness",
    ): (
        "IndexedChainSpec => \\A initialContext \\in "
        "AdmissibleContextRecords: WF_(IndexedHistoricalTransport("
        "initialContext)!AsyncAllVars)( IndexedHistoricalPostGstTick("
        "initialContext))"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesHistoricalRunNodeFairness",
    ): (
        "IndexedChainSpec => \\A initialContext \\in "
        "AdmissibleContextRecords: \\A node \\in "
        "IndexedHistoricalTransport(initialContext)! AsyncVotersAt("
        "initialContext): WF_(IndexedHistoricalTransport(initialContext)!"
        "AsyncAllVars)( IndexedHistoricalTransport(initialContext)! "
        "PostGstRunNode(node))"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesHistoricalOwnerServiceFairness",
    ): (
        "IndexedChainSpec => \\A initialContext \\in "
        "AdmissibleContextRecords: \\A node \\in Responsive: /\\ "
        "WF_(IndexedHistoricalTransport(initialContext)!AsyncAllVars)( "
        "IndexedHistoricalTransport(initialContext)! "
        "PostGstRunHistoricalServer(node)) /\\ WF_("
        "IndexedHistoricalTransport(initialContext)!AsyncAllVars)( "
        "IndexedHistoricalTransport(initialContext)! "
        "PostGstServiceIoWorker(node)) /\\ WF_("
        "IndexedHistoricalTransport(initialContext)!AsyncAllVars)( "
        "IndexedHistoricalTransport(initialContext)! "
        "PostGstRunHistoricalRecoveryNode(node)) /\\ WF_("
        "IndexedHistoricalTransport(initialContext)!AsyncAllVars)( "
        "IndexedHistoricalTransport(initialContext)! "
        "PostGstServiceHistoricalRecoveryIoWorker(node))"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesHistoricalDueNodeModeFairness",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, owner, mode \\in "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryTimedOwnerModeCarrier: /\\ IndexedChainSpec /\\ "
        "owner \\in Responsive /\\ (mode = 2 => owner \\in "
        "IndexedHistoricalTransport(initialContext)! AsyncVotersAt("
        "initialContext)) => WF_(IndexedHistoricalTransport(initialContext)!"
        "AsyncAllVars)( IndexedHistoricalDueNodeModeFairAction( "
        "initialContext, owner, mode))"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesHistoricalDueIoModeFairness",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, owner \\in "
        "Responsive, mode \\in IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryTimedOwnerModeCarrier: IndexedChainSpec => "
        "WF_(IndexedHistoricalTransport(initialContext)!AsyncAllVars)( "
        "IndexedHistoricalDueIoModeFairAction( initialContext, owner, mode))"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecHistoricalDueNodeModeMakesProgress",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, node \\in "
        "Responsive, clockValue \\in Nat, sourceRank \\in "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryFixedClockBlockerCarrier, owner, mode \\in "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryTimedOwnerModeCarrier: IndexedChainSpec => "
        "(IndexedHistoricalDueNodeOwnerAtMode( initialContext, node, "
        "clockValue, sourceRank, owner, mode) ~> "
        "IndexedHistoricalDueNodeModeProgressGoal( initialContext, node, "
        "clockValue, sourceRank, owner, mode))"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecHistoricalDueIoModeMakesProgress",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, node \\in "
        "Responsive, clockValue \\in Nat, sourceRank \\in "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryFixedClockBlockerCarrier, owner, mode \\in "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryTimedOwnerModeCarrier: IndexedChainSpec => "
        "(IndexedHistoricalDueIoOwnerAtMode( initialContext, node, "
        "clockValue, sourceRank, owner, mode) ~> "
        "IndexedHistoricalDueIoModeProgressGoal( initialContext, node, "
        "clockValue, sourceRank, owner, mode))"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecHistoricalDueNodeOwnerReachesRankGoal",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, node \\in "
        "Responsive, clockValue \\in Nat, sourceRank \\in "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryFixedClockBlockerCarrier, owner: IndexedChainSpec "
        "=> ((/\\ IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryFixedClockBlockedAtRank( node, clockValue, "
        "sourceRank) /\\ IndexedHistoricalTransport(initialContext)! "
        "OverdueResponsivePackets = {} /\\ owner \\in "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryNodeBlockersAt(clockValue)) ~> "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryFixedClockStrictRankGoal( node, clockValue, "
        "sourceRank))"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecHistoricalDueIoOwnerReachesRankGoal",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, node \\in "
        "Responsive, clockValue \\in Nat, sourceRank \\in "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryFixedClockBlockerCarrier, owner: IndexedChainSpec "
        "=> ((/\\ IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryFixedClockBlockedAtRank( node, clockValue, "
        "sourceRank) /\\ IndexedHistoricalTransport(initialContext)! "
        "OverdueResponsivePackets = {} /\\ "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryNodeBlockersAt(clockValue) = {} /\\ owner \\in "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryActiveIoBlockersAt( clockValue)) ~> "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryFixedClockStrictRankGoal( node, clockValue, "
        "sourceRank))"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalTickBlockedHasEnabledPostGstTick",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, node \\in "
        "Responsive, clockValue \\in Nat, sourceRank \\in "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryFixedClockBlockerCarrier: "
        "IndexedHistoricalTickBlockedAtRank( initialContext, node, clockValue, "
        "sourceRank) => ENABLED <<IndexedHistoricalPostGstTick("
        "initialContext)>>_( IndexedHistoricalTransport(initialContext)!"
        "AsyncAllVars)"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecHistoricalTickReachesRankGoal",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, node \\in "
        "Responsive, clockValue \\in Nat, sourceRank \\in "
        "IndexedHistoricalTransport(initialContext)! "
        "HistoricalDiscoveryFixedClockBlockerCarrier: IndexedChainSpec => "
        "(IndexedHistoricalTickBlockedAtRank( initialContext, node, "
        "clockValue, sourceRank) ~> IndexedHistoricalTransport("
        "initialContext)! HistoricalDiscoveryFixedClockStrictRankGoal( node, "
        "clockValue, sourceRank))"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalFixedClockNonPacketService",
    ): (
        "IndexedChainSpec => "
        "IndexedHistoricalFixedClockNonPacketServiceProperty"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalFixedClockLeavesEstablishPrerequisiteSurface",
    ): (
        "/\\ IndexedChainSpec /\\ "
        "IndexedHistoricalFixedClockTemporalLeafProperties => "
        "IndexedHistoricalFixedClockPrerequisiteSurface"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalFixedClockPacketResidualClosesPacketLeaves",
    ): (
        "IndexedHistoricalFixedClockPacketCorridorTemporalResidual => "
        "IndexedHistoricalFixedClockPacketLeafProperties"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalFixedClockExactResidualsEstablishPrerequisiteSurface",
    ): (
        "/\\ IndexedChainSpec /\\ "
        "IndexedHistoricalFixedClockPacketCorridorTemporalResidual => "
        "IndexedHistoricalFixedClockPrerequisiteSurface"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalFixedClockPacketRemainingResidual",
    ): (
        "IndexedChainSpec => "
        "IndexedHistoricalFixedClockPacketRemainingTemporalResidual"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecAndRemainingPacketResidualProvideFixedClockPacketCorridor",
    ): (
        "/\\ IndexedChainSpec "
        "/\\ IndexedHistoricalFixedClockPacketRemainingTemporalResidual "
        "=> IndexedHistoricalFixedClockPacketCorridorTemporalResidual"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalFixedClockPacketCorridor",
    ): (
        "IndexedChainSpec => "
        "IndexedHistoricalFixedClockPacketCorridorTemporalResidual"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalCommitTransportKernelsCloseExactLeaf",
    ): (
        "/\\ IndexedChainSpec /\\ "
        "IndexedHistoricalCommitTransportResidualKernelProperties => "
        "IndexedHistoricalCommitCertificateTransportLeafProperty"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalDecisionTransportKernelsCloseExactLeaf",
    ): (
        "/\\ IndexedChainSpec /\\ "
        "IndexedHistoricalDecisionTransportResidualKernelProperties => "
        "IndexedHistoricalDecisionCertifiedBodyTransportLeafProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalFixedClockExactResidualsCloseCertificateDiscoveryRank",
    ): (
        "/\\ IndexedChainSpec /\\ "
        "IndexedHistoricalFixedClockPacketCorridorTemporalResidual => "
        "IndexedHistoricalCertificateDiscoveryRunnerResidualProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificatePhysicalKernelsCloseRankResidual",
    ): (
        "/\\ IndexedChainSpec /\\ "
        "IndexedHistoricalCertificatePhysicalResidualKernels => "
        "IndexedHistoricalCertificateRankProgressResidualProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedJoinedResponsiveActiveRosterIsStable",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "/\\ IndexedCompositionInvariant "
        "/\\ initialContext \\in JoinedContexts "
        "/\\ IndexedResponsiveActiveRosterAt(initialContext) "
        "/\\ [IndexedChainNext]_IndexedChainVars "
        "=> /\\ initialContext \\in JoinedContexts' "
        "/\\ IndexedResponsiveActiveRosterAt(initialContext)'"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalStrictAncestorRecoveryClosesActivationAt",
    ): (
        "/\\ IndexedChainSpec "
        "/\\ IndexedSuccessorActivationProgress "
        "=> \\A targetContext \\in AdmissibleContextRecords: "
        "IndexedStrictAncestorRecoveryAdvance(targetContext) "
        "=> \\A node \\in Responsive: "
        "IndexedHistoricalRecoveryAuthorityAcquisitionResidual( "
        "targetContext, node) "
        "~> (IndexedHistoricalRecoveryEntryGoal(targetContext, node) "
        "\\/ /\\ IndexedHistoricalRecoveryArchiveOwnerJoined( targetContext) "
        "/\\ IndexedResponsiveActiveRosterAt(targetContext) "
        "/\\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual( "
        "targetContext, node))"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedLiveStrictAncestorsCloseOrdinaryDecisionOwnerRanks",
    ): (
        "IndexedChainSpec "
        "=> \\A initialContext \\in AdmissibleContextRecords: "
        "IndexedStrictAncestorRecoveryAdvance(initialContext) "
        "=> \\A node \\in Responsive, rank \\in 1..6: "
        "IndexedHistoricalDecisionOrdinaryOwnerRankProgressAt( "
        "initialContext, node, rank)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalJointProgressProjectsReleaseProperties",
    ): (
        "IndexedHistoricalRecoveryJointProgressProperty "
        "=> /\\ IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty "
        "/\\ IndexedHistoricalRecoveryEntryCompletionProperty "
        "/\\ IndexedHistoricalDecisionRankProgressResidualProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalStrictHeightServiceCompositionClosesAuthority",
    ): (
        "/\\ IndexedLiveChainSpec "
        "/\\ IndexedLocalAdequateLeaderDecisionConvergenceProperty "
        "/\\ IndexedHistoricalCertificateRankProgressResidualProperty "
        "/\\ IndexedHistoricalDecisionTargetOwnerRankProgressProperty "
        "=> IndexedHistoricalRecoveryAuthorityAcquisitionResidualProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalStrictHeightServiceCompositionClosesDecisionRank",
    ): (
        "/\\ IndexedLiveChainSpec "
        "/\\ IndexedLocalAdequateLeaderDecisionConvergenceProperty "
        "/\\ IndexedHistoricalCertificateRankProgressResidualProperty "
        "/\\ IndexedHistoricalDecisionTargetOwnerRankProgressProperty "
        "=> IndexedHistoricalDecisionRankProgressResidualProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionPhysicalKernelsCloseTargetRank",
    ): (
        "/\\ IndexedChainSpec /\\ "
        "IndexedHistoricalDecisionTransportResidualKernelProperties => "
        "IndexedHistoricalDecisionTargetOwnerRankProgressProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedDecisionWitnessVariablesAreExact",
    ): (
        "IndexedAsyncStateShape => \\A initialContext \\in "
        "AdmissibleContextRecords: "
        "IndexedDecisionWitness(initialContext)!AsyncAllVars = "
        "IndexedAsyncStateAt(initialContext)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalDecisionTargetCandidateRankResiduals",
    ): (
        "IndexedChainSpec => "
        "IndexedHistoricalDecisionTargetCandidateRankProgressResidualProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionRankResidualSplitsAtCertifiedRequest",
    ): (
        "IndexedHistoricalDecisionRankProgressResidualProperty <=> /\\ "
        "IndexedHistoricalDecisionCandidateRankProgressResidualProperty /\\ "
        "IndexedHistoricalDecisionCertifiedRequestResidualProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalCertificateCandidateTail",
    ): (
        "IndexedChainSpec => "
        "IndexedHistoricalCertificateCandidateTailProgressProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateRankOneIsLocalImport",
    ): (
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "IndexedHistoricalCertificateStageAt(initialContext, node, 1) => "
        "IndexedHistoricalCertificateLocalImportAt( initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalCertificateRankOneEntry",
    ): (
        "IndexedChainSpec => "
        "IndexedHistoricalCertificateRankOneCandidateEntryProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateRemainingCorridorClosesRankResidual",
    ): (
        "/\\ IndexedChainSpec /\\ "
        "IndexedHistoricalCertificateRemainingCorridorProperty => "
        "IndexedHistoricalCertificateRankProgressResidualProperty"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionTargetCertifiedRequestClosesTargetRank",
    ): (
        "/\\ IndexedChainSpec /\\ "
        "IndexedHistoricalDecisionTargetCertifiedRequestResidualProperty => "
        "IndexedHistoricalDecisionTargetOwnerRankProgressProperty"
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryLivenessProofs",
        "HistoricalRecoveryTargetPersistsUnlessApplication",
    ): (
        "\\A node: /\\ AsyncStrongTypeInvariant "
        "/\\ HistoricalRecoveryTarget(node) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ ~NodeHasApplication(node)' => "
        "HistoricalRecoveryTarget(node)'"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalDiscoveryCorridor",
    ): (
        "/\\ IndexedChainSpec "
        "/\\ IndexedHistoricalDiscoveryClockProgressProperty => "
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "(/\\ IndexedHistoricalTransport(initialContext)!gst "
        "/\\ IndexedHistoricalTransport(initialContext)!"
        "HistoricalRecoveryTarget(node)) ~> "
        "IndexedHistoricalDiscoveryOutcome(initialContext, node)"
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesOwnedHistoricalDiscoveryCorridor",
    ): (
        "/\\ IndexedChainSpec "
        "/\\ IndexedHistoricalDiscoveryClockProgressProperty => "
        "\\A initialContext \\in AdmissibleContextRecords, "
        "node \\in Responsive: "
        "(/\\ IndexedHistoricalTransport(initialContext)!gst "
        "/\\ IndexedHistoricalTransport(initialContext)!"
        "HistoricalRecoveryTarget(node)) ~> "
        "IndexedHistoricalDiscoveryOwnedOutcome( initialContext, node)"
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalCertificateDiscoveryRank",
    ): (
        "/\\ IndexedChainSpec "
        "/\\ IndexedHistoricalDiscoveryClockProgressProperty => "
        "IndexedHistoricalCertificateDiscoveryRunnerResidualProperty"
    ),
}
