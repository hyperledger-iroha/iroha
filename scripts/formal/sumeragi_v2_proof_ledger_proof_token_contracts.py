# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

FIXED_PROOF_REQUIRED_PROOF_TOKENS = {
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AdequateLeaderFixedSelectedOwnerUsesExactAsyncFairness",
    ): (
        "AdequateLeaderFixedSelectedServiceOwnerSet",
        "AdequateLeaderFixedSelectedServiceOwnerAction",
        "AsyncSpecAt",
        "AsyncFairnessAt",
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AsyncLiveProvidesAdequateLeaderFixedSelectedOwnerFairness",
    ): (
        "AsyncLiveSpecProjectsAsyncSpec",
        "AdequateLeaderFixedSelectedOwnerUsesExactAsyncFairness",
        "AdequateLeaderFixedSelectedOwnerFairnessProperty",
        "PTL",
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AdequateLeaderFixedGlobalSelectionAndPreCandidateServiceSupplyRawRouteStep",
    ): (
        "AdequateLeaderFixedPipelineServiceRankFrontierHasExactCell",
        "AdequateLeaderFixedSelectedExactCellProjectsToServiceFrontier",
        "AdequateLeaderFixedStrictGoalsProjectToServiceRankDescent",
        "AdequateLeaderFixedPreCandidateRawRouteRankStepProperty",
        "AdequateLeaderFixedGlobalBlockerSelectionClosureProperty",
        "AdequateLeaderFixedPreCandidateEntryServiceProperty",
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AdequateLeaderFixedPreCandidateRawRouteStepClosesRank",
    ): (
        "AdequateLeaderFixedPipelineRankOrderingIsWellFounded",
        "WellFoundedLeadsTo",
        "AdequateLeaderFixedPreCandidateRawRouteRankStepProperty",
        "AdequateLeaderFixedPreCandidateRawRouteRankClosureProperty",
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AdequateLeaderFixedPipelineProducerHandoffStartsRawRouteRank",
    ): (
        "AdequateLeaderFixedPipelineProducerHandoffFrontier",
        "AdequateLeaderFixedPreCandidateRawRouteRankFrontier",
        "AdequateLeaderFixedPreCandidateRouteCarrier",
        "AdequateLeaderFixedAnyPipelineTokenCarrier",
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AdequateLeaderFixedCandidateFairnessAndRawRouteClosureSupplyEpisodeStep",
    ): (
        "AdequateLeaderFixedPipelineProducerHandoffStartsRawRouteRank",
        "AdequateLeaderFixedSelectedOwnerFairnessProperty",
        "AdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStepProviderProperty",
        "AdequateLeaderFixedPreCandidateRawRouteRankClosureProperty",
        "AdequateLeaderFixedPipelineOriginNonDescentEpisodeStepProperty",
        "WF1",
        "PTL",
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AsyncLiveProvidesAdequateLeaderFixedPipelineOriginNonDescentEpisodeStep",
    ): (
        "AsyncLiveProvidesAdequateLeaderFixedPipelineOriginEpisodeSelectedOwnerStep",
        "AsyncLiveProvidesAdequateLeaderFixedSelectedOwnerFairness",
        "AsyncLiveProvidesAdequateLeaderFixedGlobalBlockerProviders",
        "AsyncLiveProvidesAdequateLeaderFixedPreCandidateSelectedOwnerStep",
        "AsyncLiveProvidesAdequateLeaderFixedSelectedActionClockCarry",
        "AdequateLeaderFixedGlobalBlockerProvidersSupplyRankStep",
        "AdequateLeaderFixedGlobalBlockerRankClosesOwnerSelection",
        "AdequateLeaderFixedPreCandidateSelectionAndFairnessSupplyEntryService",
        "AdequateLeaderFixedGlobalSelectionAndPreCandidateServiceSupplyRawRouteStep",
        "AdequateLeaderFixedPreCandidateRawRouteStepClosesRank",
        "AdequateLeaderFixedCandidateFairnessAndRawRouteClosureSupplyEpisodeStep",
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        (
            "AsyncLiveSpecSuppliesAdequateLeaderAuthorityDeadline"
            "FreshSelfQuantitativeProviderBundle"
        ),
    ): (
        "AsyncLiveSpecSuppliesAdequateLeaderConfiguredBudget",
        "AsyncLiveSpecProjectsAsyncSpec",
        "AsyncSpecAlwaysStrongTypeInvariant",
        "AsyncLiveProvidesAdequateLeaderFixedSelectedOwnerFairness",
        "AsyncLiveProvidesAdequateLeaderAuthorityDeadlineImmediateSourceEntry",
        "AsyncLiveProvidesAdequateLeaderFixedSubjectReplacementProviders",
        "AsyncLiveProvidesAdequateLeaderFixedPipelineTokenOwnershipAndTailCarry",
        "AsyncLiveProvidesAdequateLeaderFixedPipelineOriginHistory",
        "AsyncLiveProvidesAdequateLeaderFixedCutPerAction",
        "AsyncLiveProvidesAdequateLeaderFixedSelectedActionClockCarry",
        "AsyncLiveProvidesAdequateLeaderFixedPreCandidateSelectedOwnerStep",
        "AsyncLiveProvidesAdequateLeaderFixedPipelineOriginNonDescentEpisodeStep",
        "AsyncLiveProvidesAdequateLeaderRetainedProducerNonDescentEpisodeStep",
        "AdequateLeaderFiniteRetainedProducerBudgetClosesNonDescentEpisode",
        "AsyncLiveProvidesAdequateLeaderFixedGlobalBlockerProviders",
        "AsyncLiveProvidesAdequateLeaderAuthorityDeadlineNoPrematureExitStep",
        "AsyncLiveProvidesAdequateLeaderAuthorityDeadlineDecisionRetention",
        "AdequateLeaderAuthorityDeadlineFreshSelfQuantitativeProviderBundle",
        "PTL",
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AsyncLiveFreshSelfBundleSuppliesFixedDeadlineAndResponsiveDissemination",
    ): (
        "AdequateLeaderAuthorityDeadlineFreshSelfBundleSuppliesFixedDeadlineService",
        "StarvationFreedomObligation",
        "AsyncLiveProvidesResponsiveDecisionDissemination",
        "AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty",
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AsyncLiveSpecSuppliesAdequateLeaderFixedDeadlineAndResponsiveDissemination",
    ): (
        (
            "AsyncLiveSpecSuppliesAdequateLeaderAuthorityDeadline"
            "FreshSelfQuantitativeProviderBundle"
        ),
        "AsyncLiveFreshSelfBundleSuppliesFixedDeadlineAndResponsiveDissemination",
    ),
    (
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
        "AdequateLeaderFixedDeadlineAndDisseminationSupplyLocalTargetConvergence",
    ): (
        "AdequateLeaderFixedDeadlineServiceClosesFreshSelfCorridor",
        "AdequateLeaderLocalFreshSelfCorridorExposureProperty",
        "AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty",
        "AdequateLeaderLocalTargetDecisionConvergenceProperty",
        "AdequateLeaderFreshSynchronizedTargetCorridor",
        "PTL",
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AdequateLeaderFixedDeadlineAndDisseminationSupplyLocalConvergence",
    ): (
        "AsyncLiveProvidesLocalFreshSelfCorridorExposure",
        "AdequateLeaderFixedDeadlineAndDisseminationSupplyLocalTargetConvergence",
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AdequateLeaderFixedDeadlineAndDisseminationCloseExactResidual",
    ): (
        "AsyncLiveProvidesAdequateLeaderExactResidualKernel",
        "AdequateLeaderFixedDeadlineAndDisseminationSupplyLocalConvergence",
        "AdequateLeaderExactClosureResidualProperty",
        "PTL",
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AdequateLeaderExactClosureResidualObligation",
    ): (
        "AsyncLiveSpecSuppliesAdequateLeaderFixedDeadlineAndResponsiveDissemination",
        "AdequateLeaderFixedDeadlineAndDisseminationCloseExactResidual",
        "AdequateLeaderExactClosureResidualProperty",
        "AdequateLeaderExactResidualKernelProperty",
        "AdequateLeaderLocalTargetDecisionConvergenceProperty",
        "PTL",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "AsyncSpecClosesExactDecisionRequestIngressContinuationPrefix",
    ): (
        "AsyncSpecProvidesVoterCandidateProducerContinuationResolutionClosure",
        "AsyncSpecAlwaysUsesFixedResponsiveVoters",
        "ExactDecisionRequestIngressContinuationPrefixClosureProperty",
        "ExactDecisionNormalRequestIngressContinuationPrefixBlocked",
        "ExactDecisionRequestIngressContinuationPrefixGoal",
        "ExactDecisionRequestIngressContinuationPrefixCleared",
        "AsyncVoterCandidateProducerContinuationResolutionClosureProperty",
        "AsyncVoterCandidateProducerContinuationEpisodePending",
        "PTL",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressContinuationPrefixHasFiniteRankWitness",
    ): (
        "CandidateProducerContinuationFrozenPrefixRankIsFiniteAndPositive",
        "ExactDecisionRequestIngressContinuationPrefixAtBudget",
        "ExactDecisionNormalRequestIngressContinuationPrefixBlocked",
        "AsyncCandidateProducerContinuationFrozenPrefixAtBudget",
        "AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord",
        "AsyncCandidateProducerContinuationRunnerMayPrecedeIngress",
        "AsyncCandidateProducerContinuationRecordSet",
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AsyncLiveProvidesAdequateLeaderViewExposureRankStep",
    ): (
        "AsyncLiveSpecProjectsAsyncSpec",
        "AsyncSpecAlwaysStrongTypeInvariant",
        "AsyncLiveProvidesDirectTimeoutViewClosureResidual",
        "DirectTimeoutViewDecompositionClosesTimeoutViewProgress",
        "AsyncLiveProvidesAuthenticatedTcEpisodeBudgetDescent",
        "AdequateLeaderFiniteAuthenticatedTcEpisodeCloses",
        "AdequateLeaderExactTcSynchronizationSourceStartsRank",
        "AsyncLiveProvidesExactTcResponsiveRosterSynchronization",
        "AdequateLeaderResidentTcSkipStrictlyLowersExposureRank",
        "AdequateLeaderResidentOriginSkipStrictlyLowersExposureRank",
        "AsyncLiveAuthenticatedTcCatchupCannotOvertakeFreshSelfWindow",
        "AdequateLeaderFrozenResidentDebtIsFinite",
        "AdequateLeaderViewExposureRankStepProperty",
        "AdequateLeaderViewExposureRankFrontier",
        "AdequateLeaderViewExposureStrictDescentGoal",
        "AdequateLeaderFrozenViewEpisodeSource",
        "AdequateLeaderViewExposureRank",
        "AdequateLeaderTargetFreshSelfCorridorGoal",
        "AdequateLeaderLocalTargetDecisionSource",
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AdequateLeaderViewExposureRankOrderingIsWellFounded",
    ): (
        "NatLessThanWellFounded",
        "WFLexPairOrdering",
        "AdequateLeaderViewExposureRankOrdering",
        "AdequateLeaderViewExposureRankCarrier",
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AsyncLiveProvidesLocalFreshSelfCorridorExposure",
    ): (
        "AsyncLiveSpecProjectsAsyncSpec",
        "AsyncSpecAlwaysStrongTypeInvariant",
        "AsyncLiveProvidesAdequateLeaderViewExposureRankStep",
        "AdequateLeaderFrozenSourceStartsConcreteViewRank",
        "AdequateLeaderViewExposureRankOrderingIsWellFounded",
        "WellFoundedLeadsTo",
        "AdequateLeaderLocalFreshSelfCorridorExposureProperty",
        "AdequateLeaderFrozenViewEpisodeSource",
        "AdequateLeaderViewExposureRankStepProperty",
        "AdequateLeaderViewExposureRankFrontier",
        "AdequateLeaderViewExposureStrictDescentGoal",
        "AdequateLeaderTargetFreshSelfCorridorGoal",
        "AdequateLeaderFreshSynchronizedTargetCorridor",
        "AdequateLeaderLocalTargetDecisionSource",
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedInitEstablishesPostGstResponsiveActiveRosterCoherence",
    ): (
        "IndexedChainInit",
        "IndexedPostGstResponsiveActiveRosterCoherence",
        "IndexedAsync!AsyncInitAt",
        "IndexedAsync!AsyncActiveServiceNodes",
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedNewGstRequiresResponsiveActiveRoster",
    ): (
        "IndexedStepProjectsEveryAsyncStep",
        "IndexedInstanceVariablesAreExact",
        "IndexedAsync!AsyncSetGST",
        "IndexedAsync!AsyncServiceActivationTransition",
        "IndexedAsync!AsyncActiveServiceNodes",
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedPostGstResponsiveActiveRosterSurvivesAction",
    ): (
        "IndexedStepProjectsEveryAsyncStep",
        "IndexedInstanceVariablesAreExact",
        "IndexedAsync!AsyncServiceActivationTransition",
        "IndexedAsync!AsyncServiceActivationClockPristine",
        "IndexedAsync!AsyncActiveServiceNodes",
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedActionPreservesPostGstResponsiveActiveRosterCoherence",
    ): (
        "IndexedNewGstRequiresResponsiveActiveRoster",
        "IndexedPostGstResponsiveActiveRosterSurvivesAction",
        "IndexedAsync!GstAsyncStepIsMonotone",
        "IndexedPostGstResponsiveActiveRosterCoherence",
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedStutterPreservesPostGstResponsiveActiveRosterCoherence",
    ): (
        "IndexedPostGstResponsiveActiveRosterCoherence",
        "IndexedChainVars",
        "IndexedAsyncStateAt",
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedStepPreservesPostGstResponsiveActiveRosterCoherence",
    ): (
        "IndexedActionPreservesPostGstResponsiveActiveRosterCoherence",
        "IndexedStutterPreservesPostGstResponsiveActiveRosterCoherence",
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedChainSpecAlwaysKeepsPostGstResponsiveRosterActive",
    ): (
        "IndexedChainSpecEstablishesCompositionInvariant",
        "IndexedCompositionInvariant",
    ),
    (
        "SumeragiV2ChainEpochRefinement",
        "IndexedPostGstResponsiveRosterIsActive",
    ): (
        "IndexedChainSpecAlwaysKeepsPostGstResponsiveRosterActive",
        "IndexedPostGstResponsiveActiveRosterCoherence",
    ),
    (
        "SumeragiV2AsyncRankClosureProofs",
        "AsyncRankClosureProtectedServiceRankProgressObligation",
    ): (
        "AsyncSpecProvidesProtectedServiceFiniteRunnerEpisodeClosure",
        "ProtectedStage3RankProgressFromFairSchedulerObligation",
        "ProtectedStage4RankProgressFromFairScheduler",
        "ProtectedStage5RankProgressFromFairFifo",
        "ProtectedStage6RankProgressFromFairCausalAdmissionObligation",
        "ProtectedPostDeferredRanksComposeFromLeavesObligation",
        "ProtectedStage2RankProgressWithExactHandoffObligation",
        "ProtectedServeRankProgressFromFairFifo",
        "ProtectedServiceRanksProgressLeafCompositionObligation",
    ),
    (
        "SumeragiV2AsyncRankClosureProofs",
        "AsyncRankClosureStarvationFreedomObligation",
    ): (
        "AsyncRankClosureProtectedServiceRankProgressObligation",
        "AsyncLiveSpecProjectsAsyncSpec",
        "ProtectedServiceRankProgressImpliesStarvation",
        "StarvationFreedomProperty",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncSpecProvidesAdequateLeaderWirePhysicalFrozenCertificateConvergence",
    ): (
        "ExactDecisionTargetNeutralFixedClockOrderingIsWellFounded",
        "ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets",
        "ExactDecisionTargetNeutralLaterWorkCannotAcquirePredecessor",
        "ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank",
        "ExactDecisionTargetNeutralProoflessProducerStepIsDescentOrFrame",
        "ExactDecisionTargetNeutralComposedCausalEpisodeStepIsDescentOrFrame",
        "ExactDecisionTargetNeutralProducerEpisodeStepIsDescentOrFrame",
        "ExactDecisionTargetNeutralProducerEpisodeOrderingIsWellFounded",
        "ExactDecisionTargetNeutralProducerEpisodeBottomHasNoLowerRank",
        "ExactDecisionTargetNeutralProducerEpisodeBottomForcesStrictRankGoal",
        "ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness",
        "LeaderWirePhysicalPacketDependencyRankIsSnapshotScoped",
        "LeaderWirePhysicalFrozenCertificateRetainsPastCut",
        "AsyncProoflessChunkEpisodeBudgetIsFiniteAndCoalesced",
        "AsyncHeldChunkReceiptTombstonesExactProducerEpisode",
        "LeaderWirePacketAdmissionPreservesExactResolution",
        "AdequateLeaderWirePhysicalFrozenCertificateConvergenceProperty",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncSpecProvidesAdequateLeaderWirePhysicalConvergence",
    ): (
        "LeaderWirePhysicalResidualCapturesFrozenCertificate",
        "AsyncSpecProvidesAdequateLeaderWirePhysicalFrozenCertificateConvergence",
        "ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness",
        "ExactDecisionRequestIngressRankOrderingIsWellFounded",
        "LeaderWirePhysicalLifecycleOrdinalOrderingIsWellFounded",
        "LeaderWirePhysicalIngressDependencyOrderingIsWellFounded",
        "LeaderWirePhysicalLifecycleOrdinalRankIsInCarrier",
        "LeaderWirePhysicalIngressDependencyRankIsInCarrier",
        "AsyncLeaderWireIngressTicketExcludesLaterLocalWork",
        "AsyncSelectedLeaderWirePhysicalCarrierDefinesIngressScheduler",
        "AsyncProoflessChunkEpisodeBudgetIsFiniteAndCoalesced",
        "AsyncHeldChunkReceiptTombstonesExactProducerEpisode",
        "LeaderWirePacketAdmissionPreservesExactResolution",
        "LeaderWireIngressDrainPreservesExactHandoff",
        "AdequateLeaderWirePhysicalConvergenceProperty",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncLiveProvidesAdequateLeaderWirePhysicalConvergence",
    ): (
        "AsyncSpecProvidesAdequateLeaderWirePhysicalConvergence",
        "AsyncLiveSpecAt",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdvancedResponsiveNodeHasInstalledTimeoutRotationHandoff",
    ): (
        "TimeoutQuorumViewRotationHandoff",
        "TimeoutViewOwnershipInvariant",
        "ResponsiveViewCertificateAuthority",
        "TimeoutCertificateSemanticIdentity",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncLiveProvidesAdequateLeaderTimeoutRotationConvergence",
    ): (
        "AsyncLiveProvidesDirectTimeoutViewClosureResidual",
        "DirectTimeoutViewDecompositionClosesTimeoutViewProgress",
        "AsyncLiveSpecProjectsAsyncSpec",
        "AsyncSpecAlwaysStrongTypeInvariant",
        "TimeoutViewOwnershipInvariantFromAsyncSpec",
        "AsyncSpecKeepsGstOnceSet",
        "AsyncSpecAlwaysUsesFixedResponsiveVoters",
        "AdvancedResponsiveNodeHasInstalledTimeoutRotationHandoff",
        "AdequateLeaderTimeoutRotationConvergenceProperty",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncLiveProvidesAdequateLeaderOpenPhysicalResidualConvergence",
    ): (
        "AsyncLiveProvidesAdequateLeaderWirePhysicalConvergence",
        "AsyncLiveProvidesAdequateLeaderTimeoutRotationConvergence",
        "AdequateLeaderOpenPhysicalResidualConvergenceProperty",
        "AdequateLeaderWirePhysicalConvergenceProperty",
        "AdequateLeaderTimeoutRotationConvergenceProperty",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "ExactResidualKernelSuppliesExactPhysicalConvergence",
    ): (
        "CertifiedResponsePhysicalDebtConvergence",
        "AdequateLeaderExactResidualKernelProperty",
        "AdequateLeaderExactPhysicalResidualConvergenceProperty",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "ExactResidualKernelSuppliesCandidateSemanticHandoffs",
    ): (
        "SchedulerOriginReadinessReducesToExactLeaderExitSafety",
        "ExactDiscardSafetyClosesAdmittedCandidateHandoffs",
        "AdequateLeaderExactResidualKernelProperty",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncLiveProvidesAdequateLeaderExactResidualKernel",
    ): (
        "AsyncLiveExactLeaderSchedulerOriginReadiness",
        "AsyncLiveProvidesAdequateLeaderOpenPhysicalResidualConvergence",
        "AdequateLeaderExactResidualKernelProperty",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutFixedClockPacketConcreteActionOccurrenceReachesGoal",
    ): (
        "HistoricalDiscoveryFixedClockIngressStrictlyDescends",
        "HistoricalDiscoveryPacketConcreteAction",
        "TimeoutFixedClockPacketConcreteActionPending",
    ),
    (
        "SumeragiV2AsyncDeadlockProofs",
        "DeadlockFreedomObligation",
    ): (
        "PostGstUndecidedEnablesConcreteProductiveAt",
        "DeadlockFreedomWithLocalWorkProperty",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncSpecProvidesTimeoutPhysicalControlTransportKernels",
    ): (
        "ExactDecisionTargetNeutralFixedClockOrderingIsWellFounded",
        "ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets",
        "ExactDecisionTargetNeutralLaterWorkCannotAcquirePredecessor",
        "ExactDecisionTargetNeutralNonDescentConsumesOrdinal",
        "ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness",
        "ExactDecisionRequestIngressRankOrderingIsWellFounded",
        "TimeoutPhysicalControlLifecycleStageOrderingIsWellFounded",
        "TimeoutPhysicalControlPacketRankUsesFrozenExactOccurrence",
        "TimeoutPhysicalControlIngressRankUsesExactAdmissionOrdinal",
        "TimeoutPhysicalControlRetainedClockHasNaturalRankOrIsDue",
        "TimeoutPhysicalControlTickLowersRetainedClockRank",
        "TimeoutPhysicalControlRetransmissionCreatesExactPacket",
        "TimeoutPhysicalControlPacketAdmissionPreservesExactHandoff",
        "TimeoutPhysicalControlIngressDrainPreservesExactHandoff",
        "AsyncRetainedCommitQcRetransmissionCreatesExactPacket",
        "AsyncRetainedCommitQcPacketAdmissionCreatesExactIngressOwner",
        "AsyncRetainedCommitQcIngressCreatesExactDeliverQcOwner",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPhysicalControlTransportKernelsProjectDeclaredLeaves",
    ): (
        "TimeoutPhysicalControlTransportKernelProperties",
        "TimeoutRetainedPacketIngressKernelProperties",
        "TimeoutCertificateSemanticIdentity",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncSpecProvidesTimeoutTcFormationReducerKernel",
    ): (
        "AsyncSpecAlwaysExcludesRetiredFormTcCandidates",
        "RetiredStandaloneFormTcActionIsDisabled",
        "TimeoutTcFormationReducerKernelProperty",
        "TimeoutRetiredFormTcCandidateAbsent",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutDecisionSourceRetainsExactDirectDelivery",
    ): (
        "ResponsiveDecisionCertificateAuthorityInvariant",
        "DecisionCertificateRetainedAuthority",
        "CommitCertificateDelivery",
        "TimeoutDecisionRoundTripTerminalOwner",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncSpecProvidesTimeoutDecisionOriginKernels",
    ): (
        "TimeoutViewOwnershipKernelInvariantFromAsyncSpec",
        "TimeoutDecisionSourceRetainsExactDirectDelivery",
        "TimeoutDecisionAppliedAuthorityOriginKernelProperty",
        "TimeoutDecisionRoundTripPhysicalKernelProperties",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncSpecProvidesDirectTimeoutViewClosureResidual",
    ): (
        "AsyncSpecProvidesTimeoutRetainedPacketIngressKernels",
        "AsyncSpecProvidesTimeoutExactDeliveryCandidateKernels",
        "AsyncSpecProvidesTimeoutImportedCertificateReducerWalKernels",
        "AsyncSpecProvidesTimeoutTcFormationReducerKernel",
        "AsyncSpecProvidesTimeoutDecisionOriginKernels",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncLiveProvidesDirectTimeoutViewClosureResidual",
    ): (
        "AsyncSpecProvidesDirectTimeoutViewClosureResidual",
        "AsyncLiveSpecAt",
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "DirectTimeoutViewClosureResidualObligation",
    ): ("AsyncLiveProvidesDirectTimeoutViewClosureResidual",),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureTimeoutViewProgressObligation",
    ): (
        "DirectTimeoutViewClosureResidualObligation",
        "DirectTimeoutViewDecompositionClosesTimeoutViewProgress",
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalTimeoutSuppliesAdequateLeaderLocalViewStep",
    ): (
        "AsyncTemporalClosureTimeoutViewProgressObligation",
        "TimeoutViewProgressSuppliesAdequateLeaderLocalViewStep",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateScheduledImportOwnersHaveExactProvenance",
    ): (
        "IndexedHistoricalCertificateLocalLineageInvariantAt",
        "IndexedHistoricalCertificateScheduledImportProvenanceInvariantAt",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedDecisionWitnessInitEstablishesHistoricalCertificateLocalLineage",
    ): (
        "IndexedHistoricalCertificateReceivedQcLineageInvariantAt",
        "IndexedHistoricalCertificateDecisionWalLineageInvariantAt",
        "IndexedHistoricalCertificateScheduledImportProvenanceInvariantAt",
        "IndexedDecisionWitness!AsyncCommitImportExecutionNeedsLineage",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedInitEstablishesHistoricalCertificateLocalLineage",
    ): (
        "IndexedInitProjectsEveryDecisionWitnessInit",
        "IndexedDecisionWitnessInitEstablishesHistoricalCertificateLocalLineage",
        "IndexedHistoricalCertificateLocalLineageInvariant",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedDecisionWitnessBracketPreservesHistoricalCertificateLocalLineage",
    ): (
        "DirectCommitQcCandidateHasExactImportLineage",
        "CommitCertificateResponseCandidateHasExactImportLineage",
        "CommitImportCausalSuccessorRetainsExactLineage",
        "AsyncCandidateCausalAdmissionTransfersSameOwner",
        "AsyncCandidateIoCompletionTransfersSameOwner",
        "AsyncCandidateProducerCompletionTransfersSameOwner",
        "AsyncCandidateBusyDeferralTransfersSameOwner",
        "AsyncCandidateDeferredHandoffRetainsSameOwner",
        "IndexedAsyncCommitImportLineageRefinesHistoricalCertificateLineage",
        "IndexedHistoricalCertificateScheduledImportProvenanceInvariantAt",
        "IndexedDecisionWitness!AsyncCommitImportExecutionProvenance",
        "IndexedDecisionWitness!FifoRuntimeStep",
        "IndexedDecisionWitness!DeferredDrainStep",
        "IndexedDecisionWitness!AsyncProducerVars",
        "IndexedProducer",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedBracketStepPreservesHistoricalCertificateLocalLineage",
    ): (
        "IndexedBracketStepProjectsEveryDecisionWitnessStep",
        "IndexedDecisionWitnessBracketPreservesHistoricalCertificateLocalLineage",
        "IndexedHistoricalCertificateLocalLineageInvariant",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecAlwaysHistoricalCertificateLocalLineage",
    ): (
        "IndexedInitEstablishesHistoricalCertificateLocalLineage",
        "IndexedChainSpecAlwaysDecisionWitnessSupport",
        "IndexedChainSpecAlwaysHistoricalTemporalSupport",
        "IndexedBracketStepPreservesHistoricalCertificateLocalLineage",
        "PTL",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalCertificateExactCommandLocalImport",
    ): (
        "IndexedChainSpecAlwaysDecisionWitnessSupport",
        "IndexedHistoricalCertificateExactCommandExposesCandidateEntry",
        "IndexedHistoricalCertificateExactCommandLocalImportAt",
        "PTL",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalCertificateReceivedQcLocalImportEntry",
    ): (
        "IndexedChainSpecAlwaysHistoricalCertificateLocalLineage",
        "IndexedHistoricalCertificateReceivedQcLineageExposesCandidateEntry",
        "IndexedHistoricalCertificateReceivedQcLocalImportEntryProperty",
        "PTL",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalCertificateDecisionWalLocalImportEntry",
    ): (
        "IndexedChainSpecAlwaysHistoricalCertificateLocalLineage",
        "IndexedHistoricalCertificateDecisionWalLineageExposesCandidateEntry",
        "IndexedHistoricalCertificateDecisionWalLocalImportEntryProperty",
        "PTL",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalCertificateLocalImportCandidateEntry",
    ): (
        "IndexedChainSpecClosesHistoricalCertificateReceivedQcLocalImportEntry",
        "IndexedChainSpecClosesHistoricalCertificateDecisionWalLocalImportEntry",
        "IndexedChainSpecClosesHistoricalCertificateExactCommandLocalImport",
        "IndexedHistoricalCertificateLocalImportSplitsPhysicalSources",
        "PTL",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateLocalImportCandidateEntryClosesRankOne",
    ): (
        "IndexedHistoricalCertificateRankOneIsLocalImport",
        "IndexedHistoricalCertificateLocalImportCandidateEntryProperty",
        "IndexedHistoricalCertificateRankOneCandidateEntryProperty",
        "PTL",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "DirectCommitQcCandidateHasExactImportLineage",
    ): (
        "AsyncCommitImportCandidateLineage",
        "AsyncCommitImportDirectEvidence",
        "AsyncDeliveryCandidateCausalOriginAt",
        "DeliveryCandidate",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "CommitCertificateResponseCandidateHasExactImportLineage",
    ): (
        "AsyncCommitImportCandidateLineage",
        "AsyncCommitImportResponseEvidence",
        "AsyncCommitCertificateResponseCandidateCausalOriginAt",
        "CommitCertificateRequestAuthorized",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "CommitImportCausalSuccessorRetainsExactLineage",
    ): (
        "AsyncCommitImportCandidateLineage",
        "AsyncCommitImportDirectEvidence",
        "AsyncCommitImportResponseEvidence",
        "CommandSuccessors",
        "CausalCandidate",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateServiceStageCarrierHasExactlyElevenClasses",
    ): (
        "AsyncCandidateServiceStageCapacity",
        "AsyncCandidateServiceStageClasses",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateServiceTrackedKindProjectionIsCovered",
    ): (
        "AsyncCandidateServiceTrackedKinds",
        "AsyncCandidateServiceStageForKind",
        "AsyncCandidateServiceStageClasses",
        "NoAsyncCandidateServiceStage",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateServiceRecordCapacityMatchesConfiguredGeometry",
    ): (
        "AsyncCandidateServiceStageCarrierHasExactlyElevenClasses",
        "AsyncCandidateServiceRecordCapacity",
        "AsyncCandidateLifecyclePerNodeCapacity",
        "AsyncServicedCandidateLifecycleCapacity",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateServiceLifecycleStageCollisionCoalesces",
    ): (
        "AsyncCandidateServiceOwnerPartitionInvariantIn",
        "AsyncCandidateLifecycleSlotInjectionInvariantIn",
        "AsyncCandidateLifecycleRecordForServiceIn",
        "AsyncCandidateLifecycleServiceRecordCoversIn",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateServiceRecordsInjectIntoLifecycleStageOwners",
    ): (
        "AsyncCandidateServiceLifecycleStageCollisionCoalesces",
        "AsyncCandidateServiceTrackedKindProjectionIsCovered",
        "AsyncCandidateServiceStageOwnerProjectionIn",
        "AsyncCandidateServiceRecordCapacity",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateServiceRecordProducersAreTrackedBoundaryKinds",
    ): (
        "AsyncCandidateServicesThisStep",
        "AsyncCandidateTerminalDiscardsThisStep",
        "AsyncCandidateTerminallyDiscardedThisStep",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateUntrackedInternalContinuationAllocatesNoServiceRecord",
    ): (
        "AsyncCandidateServicesThisStep",
        "AsyncCandidateTerminalDiscardsThisStep",
        "AsyncCandidateTerminallyDiscardedThisStep",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateServiceIdentityIgnoresSchedulerClass",
    ): (
        "AsyncCandidateServiceIdentity",
        "AsyncCandidateServicePayload",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleAndServiceIdentityIgnoreSchedulerClass",
    ): (
        "AsyncCandidateCausalOrigin",
        "AsyncCandidateServiceIdentity",
        "AsyncCandidateServicePayload",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateSameGenerationSuccessfulServiceIdentityPersistsUntilStrictExit",
    ): (
        "AsyncCandidateSameGenerationServicedIdentityCannotReactivateAtGst",
        "AsyncCandidateTransientServiceActive",
        "AsyncCandidateServiceTombstoned",
        "AsyncCandidateServiceCoalesced",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "DirectTimeoutPhysicalKernelsDischargeCompositeSeams",
    ): (
        "TimeoutFixedClockPhysicalKernelsDischargeLifecycleService",
        "AsyncLiveClosesTimeoutFixedOwnerPriorityTicketNonReplenishment",
        "TimeoutFixedOwnerPriorityTicketNonReplenishmentDischargesArmedWalPhysicalKernels",
        "TimeoutArmedWalPhysicalKernelsDischargeExactEndpoint",
        "TimeoutVotePhysicalKernelsDischargeSourceIsolatedDelivery",
        "TimeoutCertificateDecisionPhysicalKernelsDischargeConvergence",
        "DirectTimeoutViewClosureResidualProperty",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "DirectTimeoutViewDecompositionClosesTimeoutViewProgress",
    ): (
        "TimeoutViewOwnershipPreservationObligation",
        "TimeoutFixedClockPhysicalKernelsDischargeLifecycleService",
        "AsyncLiveClosesTimeoutFixedOwnerPriorityTicketNonReplenishment",
        "TimeoutFixedOwnerPriorityTicketNonReplenishmentDischargesArmedWalPhysicalKernels",
        "TimeoutArmedWalPhysicalKernelsDischargeExactEndpoint",
        "TimeoutVotePhysicalKernelsDischargeSourceIsolatedDelivery",
        "TimeoutCertificateDecisionPhysicalKernelsDischargeConvergence",
        "DirectTimeoutViewClosureResidualProperty",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutViewOwnershipPreservationObligation",
    ): (
        "AsyncLiveSpecProjectsAsyncSpec",
        "TimeoutViewOwnershipInvariantFromAsyncSpec",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncLiveTimeoutConcreteOriginContinuation",
    ): (
        "ProtectedServiceFiniteRunnerEpisodeClosureProperty",
        "ExactSigningTimeoutOwnerHasMatchingBusyCandidate",
        "ExactPendingTimeoutOwnerHasMatchingBusyCandidate",
        "SigningTimeoutCandidatePersistsUntilExactOutcome",
        "PendingTimeoutCandidatePersistsUntilSigningOrOutcome",
        "StarvationFreedomObligation",
        "AsyncSpecAlwaysStage2BusyKernelObligation",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutArmedExactWalEndpointClosesRuntimePrefix",
    ): (
        "TimeoutArmedExactWalEndpointProperty",
        "TimeoutArmedRuntimePrefixProperty",
        "TimeoutPendingWalOwner",
        "TimeoutOrigin",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutSemanticOwnerHandoffFromArmedRuntimePrefix",
    ): (
        "AsyncLiveTimeoutConcreteOriginContinuation",
        "TimeoutArmedRuntimePrefixProperty",
        "TimeoutConcreteOriginContinuationProperty",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutSemanticOwnerHandoffFromExactWalEndpoint",
    ): (
        "TimeoutArmedExactWalEndpointClosesRuntimePrefix",
        "TimeoutSemanticOwnerHandoffFromArmedRuntimePrefix",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "BeginTimeoutCreatesExactPendingWalOwner",
    ): (
        "BeginTimeout",
        "TimeoutPendingWalOwner",
        "TimeoutWal",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "DirectTimeoutCreatesExactWalOrDeferredOwner",
    ): (
        "BeginTimeoutCreatesExactPendingWalOwner",
        "DirectTimeoutStep",
        "TimeoutDeferredRuntimeOwner",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "DeferredTimeoutOwnerCreatesExactPendingWalOwner",
    ): (
        "DeferredTimeoutOwnerStepSelectsBeginTimeout",
        "BeginTimeoutCreatesExactPendingWalOwner",
        "TimeoutDeferredRuntimeOwner",
        "TimeoutRoundStable",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "DeferredTimeoutOwnerStepSelectsBeginTimeout",
    ): (
        "TimeoutDeferredRuntimeOwner",
        "DeferredTimeoutStep",
        "DeferredTimeoutExecutable",
        "BeginTimeoutEnabled",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "TimeoutPredeadlineClockSourceHasPositiveNaturalRank",
    ): (
        "TimeoutPredeadlineClockAtRank",
        "TimeoutPredeadlineClockExit",
        "AsyncTransportClockTypeInvariant",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "AsyncTickStrictlyLowersPredeadlineClockRankOrExits",
    ): (
        "TimeoutPredeadlineClockAtRank",
        "TimeoutPredeadlineClockExit",
        "AsyncTick",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "ExecuteExactCurrentViewTimeoutDeliveryRecordsExactReceipt",
    ): (
        "ExecuteCoreTimeoutDeliveryRecordsReceipt",
        "ExactTimeoutVoteDeliveryCommand",
        "TimeoutReceipt",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "ExecuteExactTimeoutCertificateDeliveryCreatesInstallOrGoal",
    ): (
        "ExactTimeoutCertificateDeliveryCommand",
        "TimeoutCertificateInstallOwner",
        "TimeoutViewGoal",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "ExecuteExactCommitCertificateDeliveryRecordsExactReceipt",
    ): (
        "ExactCommitCertificateDeliveryCommand",
        "QcDeliveryCreatesReceipt",
        "QcAt",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "ExecuteExactBeginInstallCreatesExactWalOwner",
    ): (
        "ExactBeginInstallTcCommand",
        "BeginInstallTC",
        "InstallTcWal",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "ExecuteExactPersistInstallReachesMinimumView",
    ): (
        "ExecutePersistInstallAdvancesCertifiedView",
        "ExactPersistInstallTcCommand",
        "TimeoutViewGoal",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "ExecuteTargetBeginDecisionCreatesWalOwner",
    ): (
        "TargetBeginDecisionCommand",
        "BeginDecision",
        "DecisionWal",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "ExecuteTargetPersistDecisionReachesDecision",
    ): (
        "TargetPersistDecisionCommand",
        "PersistDecision",
        "NodeHasDecision",
    ),
    (
        "SumeragiV2TimeoutViewProgressProofs",
        "RetiredStandaloneFormTcActionIsDisabled",
    ): ("FormTC",),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockRankedCandidateBindsFrozenTargetLeaderEpisode",
    ): (
        "RetainedLockExactCandidateRank",
        "RetainedLockFrozenCandidateIdentity",
        "RetainedLockFrozenCausalOriginCarrier",
        "AsyncCandidateCausalOriginTyped",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidateRootConstructionCoversOrigin",
    ): (
        "AdequateLeaderFrozenCandidateRootConstructed",
        "AdequateLeaderFrozenCandidateCausalOriginCarrier",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetNonDescentEpisodeBudgetIsFiniteAndCoalesced",
    ): (
        "AdequateLeaderFrozenOwnerUniverseIsFinite",
        "FS_Subset",
        "AdequateLeaderTargetEpisodeKnownOwnerSet",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFiniteBudgetDescentClosesNonDescentEpisode",
    ): (
        "NatLessThanWellFounded",
        "WellFoundedLeadsTo",
        "AdequateLeaderTargetServiceExitOrBudgetDescentGoal",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncTargetNeutralLifecycleOwnerCarrierIsFinite",
    ): (
        "FS_Product",
        "FS_Union",
        "AsyncTargetNeutralLifecycleOwnerCarrier",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncTargetNeutralLifecycleEpisodeBudgetIsFiniteAndCoalesced",
    ): (
        "AsyncTargetNeutralLifecycleOwnerCarrierIsFinite",
        "FS_Subset",
        "FS_CardinalityType",
        "AsyncTargetNeutralLifecycleEpisodeAtBudget",
        "AsyncTargetNeutralLifecycleKnownOwnerSet",
        "AsyncTargetNeutralLifecycleEpisodeBudget",
        "AsyncTargetNeutralLifecycleDiscoveredOwnerSet",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncTargetNeutralLifecycleDiscoveryStrictlyConsumesBudget",
    ): (
        "AsyncTargetNeutralLifecycleOwnerCarrierIsFinite",
        "FS_Union",
        "FS_Subset",
        "FS_CardinalityType",
        "AsyncTargetNeutralLifecycleKnownAdvanceGoal",
        "AsyncTargetNeutralLifecycleEpisodeAtBudget",
        "AsyncTargetNeutralLifecycleKnownOwnerSet",
        "AsyncTargetNeutralLifecycleEpisodeBudget",
        "AsyncTargetNeutralLifecycleDiscoveredOwnerSet",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncTargetNeutralLifecycleBudgetOrderingIsWellFounded",
    ): (
        "NatLessThanWellFounded",
        "AsyncTargetNeutralLifecycleBudgetOrdering",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleReviewedTokenOwnsOneOrigin",
    ): (
        "AsyncCandidateLifecyclePhysicalOwnerToken",
        "AsyncCandidateLifecycleServiceOwnerToken",
        "AsyncCandidateLifecycleDurableOwnerToken",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecyclePhysicalTokensCoverScheduledOriginsAfter",
    ): (
        "AsyncCandidateLifecyclePhysicalOwnerTokensForNodeIn",
        "AsyncScheduledCandidateOriginsForNodeAfter",
        "AsyncRuntimeTypeInvariant",
        "AsyncIoTypeInvariant",
        "AsyncDeferredTypeInvariant",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleDurableTokensCoverReplayOriginsAfter",
    ): (
        "AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter",
        "AsyncCandidateLifecycleDurableReplayOriginsForNodeAfter",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleServiceOwnerCarrierIsSlotBounded",
    ): (
        "FS_Injection",
        "AsyncCandidateLifecycleServiceOwnerTokensForNodeIn",
        "AsyncCandidateLifecycleServiceOwnerToken",
        "AsyncCandidateServiceOwnerPartitionInvariantIn",
        "AsyncCandidateLifecycleSlotInjectionInvariantIn",
        "AsyncCandidateLifecycleServicedSlots",
        "AsyncServicedCandidateLifecycleCapacity",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecyclePhysicalAndDurableOwnersFitActiveSlots",
    ): (
        "AsyncCandidateLifecycleDurableOwnerCarrierIsBounded",
        "AsyncCandidateLifecyclePhysicalOwnerTokensForNodeAfter",
        "AsyncCandidateLifecycleDurableOwnerTokensForNodeAfter",
        "AsyncReviewedActiveCandidateLifecycleCapacity",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleCompactedStateHasSemanticOwnerCoverage",
    ): (
        "AsyncCandidateLifecyclePhysicalTokensCoverScheduledOriginsAfter",
        "AsyncCandidateLifecycleDurableTokensCoverReplayOriginsAfter",
        "AsyncCandidateLifecycleDormantReservationOwnedAfter",
        "AsyncCandidateLifecycleServiceRecordCoversIn",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleCompactedStateHasActiveOwnerCoverage",
    ): (
        "AsyncCandidateLifecyclePhysicalTokensCoverScheduledOriginsAfter",
        "AsyncCandidateLifecycleDurableTokensCoverReplayOriginsAfter",
        "AsyncCandidateLifecycleReviewedActiveCoverageIn",
        "AsyncCandidateLifecycleActiveOriginsForNodeIn",
        "AsyncCandidateLifecycleStateAfterCompaction",
        "AsyncCandidateLifecycleServiceRecordCoversIn",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleSemanticCoverageGivesOwnerInjection",
    ): (
        "FS_Injection",
        "AsyncCandidateLifecycleSemanticOwnerProjectionIn",
        "AsyncCandidateLifecycleSemanticOwnerForOriginIn",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleActiveCoverageGivesOwnerInjection",
    ): (
        "FS_Injection",
        "AsyncCandidateLifecycleActiveOwnerProjectionIn",
        "AsyncCandidateLifecycleActiveOwnerForOriginIn",
        "AsyncCandidateLifecycleReviewedActiveCoverageIn",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleReviewedSemanticOwnersFitOrdinaryCapacity",
    ): (
        "AsyncCandidateLifecycleServiceOwnerCarrierIsSlotBounded",
        "AsyncCandidateLifecyclePhysicalAndDurableOwnersFitActiveSlots",
        "AsyncCandidateServiceOwnerPartitionInvariantIn",
        "AsyncCandidateLifecycleSlotInjectionInvariantIn",
        "AsyncTerminalCandidateLifecycleCapacity",
        "AsyncServicedCandidateLifecycleCapacity",
        "AsyncReviewedActiveCandidateLifecycleCapacity",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleReviewedOwnerInjectionProvidesReservations",
    ): (
        "FS_Injection",
        "AsyncCandidateLifecycleActiveOwnerInjectionIn",
        "AsyncReviewedActiveCandidateLifecycleCapacity",
        "AsyncCandidateLifecycleFreeOrdinarySlotsForNodeIn",
        "AsyncCandidateLifecycleSlotInjectionInvariantIn",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateLifecycleCompactedStateProvidesFreshReservations",
    ): (
        "AsyncCandidateLifecycleCompactedStateHasActiveOwnerCoverage",
        "AsyncCandidateLifecycleActiveCoverageGivesOwnerInjection",
        "AsyncCandidateLifecyclePhysicalAndDurableOwnersFitActiveSlots",
        "AsyncCandidateLifecycleReviewedOwnerInjectionProvidesReservations",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncIgnoredIngressEpisodeCannotConsumeLifecycleCapacity",
    ): (
        "AsyncCandidateIgnoredWithoutApplicationThisStep",
        "AsyncCandidateSemanticallyAppliedThisStep",
        "AsyncCandidateProducerContinuationSourceAfter",
        "AsyncCandidateProducerContinuationStateAfterDeparture",
        "AsyncCandidateProducerContinuationReservationAvailableIn",
        "AsyncCandidateProducerContinuationActiveForIdentity",
        "AsyncCandidateLifecycleStateAfterCompaction",
        "AsyncCandidateLifecycleRetirementCoveredIn",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncControlServiceTransitionRequiresAtomicLifecycleReservation",
    ): ("AsyncControlServiceSlotTransition",),
    (
        "SumeragiV2AsyncNetwork",
        "CommandSuccessorsRetainCausalOrigin",
    ): (
        "CausalCandidateWithEvidence",
        "InstallLockedFetchSuccessor",
        "InstallCommitSignSuccessor",
        "InstallProposalSuccessor",
        "PersistDecisionRecoverySuccessor",
        "AsyncCandidateWithIdentityAndOrigin",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncGateOpenDueResponsivePacketReentersClockDeadline",
    ): (
        "AsyncPacketOwnsClockDeadline",
        "OverdueResponsivePackets",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncRetainedCommitQcRetransmissionCreatesExactPacket",
    ): (
        "SendNodeRetransmissions",
        "RetryableItems",
        "RetainedControlEmissionItems",
        "PacketForItem",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncRetainedCommitQcPacketAdmissionCreatesExactIngressOwner",
    ): (
        "AdmitIngressPacket",
        "AdmitHiddenPacket",
        "CoalesceHiddenPacket",
        "DropPolicyRejectedHiddenPacket",
        "OldestDueSourcePacket",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncRetainedCommitQcIngressCreatesExactDeliverQcOwner",
    ): (
        "DrainFairIngressSelected",
        "EnqueueCandidate",
        "CandidateScheduledIn",
        "AsyncStrongTypeInvariant",
        "AsyncTransportHistoryTypeInvariant",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncRetainedCommitQcDeliveryRecordsExactReceipt",
    ): (
        "ExecuteCoreDelivery",
        "DeliverQC",
        "QcDeliveryCreatesReceipt",
        "QcAt",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncServiceActivationActionsRefineAsyncNext",
    ): (
        "AsyncNext",
        "AsyncServiceActivationTransition",
        "AsyncEnterIndexedServiceActivation",
        "AsyncActivateServiceNode",
        "AsyncServiceActivationFrameVars",
        "AsyncSchedulerExceptServiceActivation",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTransientMarkerCoalescesFreshCandidate",
    ): (
        "AsyncCandidateServiceTombstoneCoalescesFreshCandidate",
        "AsyncCandidateServiceCoalesced",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTerminalTombstoneCoalescesFreshCandidate",
    ): (
        "AsyncCandidateServiceTombstoneCoalescesFreshCandidate",
        "AsyncCandidateServiceCoalesced",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateSuccessfulServiceInstallsTransientMarker",
    ): (
        "AsyncCandidateServiceStateAfterSuccessfulService",
        "AsyncCandidateServiceStateAfterReclamation",
        "AsyncCandidateTransientServiceActive",
        "AsyncCandidateServiceRecordRetainedAfterStep",
        "AsyncCandidateServiceOwnerPartitionInvariantIn",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateSuccessfulServiceInstallsTombstone",
    ): (
        "AsyncCandidateSuccessfulServiceInstallsTransientMarker",
        "AsyncCandidateServiceTombstoned",
        "AsyncCandidateServiceCoalesced",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateSuccessfulServiceAllocatesExactOrdinal",
    ): (
        "AsyncCandidateServiceStateAfterSuccessfulService",
        "AsyncCandidateServiceMarker",
        "AsyncNextCandidateServiceOrdinal",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTransientMarkerPersistsWithinGeneration",
    ): (
        "AsyncCandidateTransientMarkerExitThisStep",
        "AsyncCandidateTransientServiceActive",
        "AsyncCandidateServiceStateAfterReclamation",
        "AsyncCandidateServiceRecordRetainedAfterStep",
        "AsyncCandidateServiceOwnerPartitionInvariantIn",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTerminalTombstonePersistsWithoutExit",
    ): (
        "AsyncCandidateTerminalTombstoneExitThisStep",
        "AsyncCandidateServiceStateAfterTerminalRetirement",
        "AsyncCandidateTerminalTombstones",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateSameHeightRestartPreservesServicedIdentity",
    ): (
        "AsyncCandidateServiceMarkersAfterReset",
        "AsyncCandidateRestartReplayTombstoned",
        "FreshRestartCandidateSequence",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateSameHeightRestartPreservesTombstone",
    ): (
        "AsyncCandidateServiceMarkersAfterReset",
        "AsyncCandidateTerminalTombstones",
        "ResetNodeSchedulerForRestart",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTransientMarkerDoesNotSuppressRestartReplay",
    ): ("AsyncCandidateRestartReplayTombstoned",),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncRestartScopedCandidateIsNeverReplayTombstoned",
    ): ("AsyncCandidateRestartReplayTombstoned",),
    (
        "SumeragiV2AsyncTimeoutOwnershipProofs",
        "RestartSignatureReplayIsNeverTombstoneSuppressed",
    ): (
        "RestartSignatureReplayCommandsAreSignatures",
        "AsyncRestartScopedCandidateIsNeverReplayTombstoned",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncServeQueuedIdentityDepartureInstallsTombstone",
    ): (
        "AsyncServeLifecyclePartitionInvariant",
        "AsyncServeReservationsAfterIoService",
        "AsyncServeTombstonesWithoutFamily",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncServeRetiredIdentityCannotRequeueAtGst",
    ): (
        "AsyncServeLogicalIdentityRetiredOrSuperseded",
        "AsyncServeFamilyHighWatermarkInvariant",
        "AcceptOrCoalesceExactServeRequest",
        "AsyncServeReservationsAfterIoService",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncServeTombstonedIdentityCannotRequeueAtGst",
    ): ("AsyncServeRetiredIdentityCannotRequeueAtGst",),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTerminalRetirementsThisStepIsSingleton",
    ): (
        "AsyncCandidateTerminalDiscardsThisStep",
        "AsyncCandidateServicesThisStep",
        "AsyncLogicalCandidateOwnershipInvariant",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateDiscardInstallsTerminalTombstone",
    ): (
        "AsyncCandidateTerminalRetirementsThisStepIsSingleton",
        "AsyncCandidateServiceStateAfterSuccessfulService",
        "AsyncCandidateTerminalRetirementEligibleAfterStep",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTerminalDiscardAllocatesExactOrdinal",
    ): (
        "AsyncCandidateTerminalRetirementsThisStepIsSingleton",
        "AsyncCandidateServiceStateAfterTerminalRetirement",
        "AsyncNextCandidateServiceOrdinal",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateDiscardRetiresLogicalLifecycle",
    ): (
        "AsyncCandidateDiscardInstallsTerminalTombstone",
        "AsyncCandidateAdmissionIdentityTerminallyCovered",
        "AsyncCandidateTerminalRetirementEligibleAfterStep",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst",
    ): (
        "AsyncCandidateAdmissionIdentityObsolete",
        "AsyncConsumerEventTag",
        "NodeHasDecision",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateObsoleteAdmissionIdentityCannotReappearAtGst",
    ): (
        "AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst",
        "AsyncCandidateStageRetired",
        "CandidateAdmissionCoalesced",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateTerminalIdentityCannotReactivateAtGst",
    ): (
        "AsyncCandidateTerminalTombstoneCoalescesFreshCandidate",
        "AsyncCandidateServiceTombstoneRejectsTransportReadmission",
        "AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst",
        "AsyncCandidateObsoleteAdmissionIdentityCannotReappearAtGst",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst",
    ): (
        "AsyncCandidateSuccessfulServiceInstallsTransientMarker",
        "AsyncCandidateIgnoredWithoutApplicationThisStep",
        "AsyncCandidateSameOriginPhysicalOrDurableOwnerAfter",
        "AsyncCandidateMonotoneSemanticCoverageAfterIn",
        "AsyncCandidateTerminalTombstoned",
        "AsyncCandidateTerminalRetirementsThisStepIsSingleton",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateSameGenerationServicedIdentityCannotReactivateAtGst",
    ): (
        "AsyncCandidateTransientMarkerPersistsWithinGeneration",
        "AsyncCandidateTransientMarkerCoalescesFreshCandidate",
        "AsyncCandidateServiceTombstoneRejectsTransportReadmission",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateResponsiveRestartPermitsNonterminalReconstruction",
    ): (
        "AsyncCandidateSameHeightRestartPreservesServicedIdentity",
        "AsyncCandidateTransientMarkerDoesNotSuppressRestartReplay",
        "AsyncCandidateServicePacketRetired",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFrozenLifecycleCoveragePersists",
    ): (
        "AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst",
        "ExactDecisionSameGenerationCandidateServiceCannotReactivateAtGst",
        "ExactDecisionTerminalCandidateDiscardCannotReactivateAtGst",
        "AsyncServeQueuedIdentityDepartureInstallsTombstone",
        "AsyncServeRetiredIdentityCannotRequeueAtGst",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetCandidateIdentityHasBoundedPayload",
    ): (
        "AdequateLeaderTargetCandidateIdentity",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidateIdentityHasBoundedPayload",
    ): (
        "AdequateLeaderFrozenTargetCandidateIdentity",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenTargetCandidatePayloadIsInStaticCarrier",
    ): (
        "AdequateLeaderFrozenCandidatePayloadCarrier",
        "AdequateLeaderFrozenCandidateItemPayloadCarrier",
        "AdequateLeaderFrozenCandidateEvidencePayloadCarrier",
        "AdequateLeaderFrozenNetworkItemCarrier",
        "AdequateLeaderFrozenCommitRequestItemCarrier",
        "AsyncCandidateSet",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidateOwnerIdentityIsInjective",
    ): (
        "AdequateLeaderFrozenCandidateOwnerIdentitySeparatesPayload",
        "AdequateLeaderFrozenTargetCandidateIdentity",
        "AdequateLeaderCandidatePayloadWithinFrozenView",
        "AdequateLeaderImmutableCandidatePayload",
        "AsyncCandidateSet",
        "AsyncNetworkItems",
        "AsyncEvidenceSet",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidateRetryIdentityIsStable",
    ): (
        "AdequateLeaderFrozenCandidateOwnerIdentity",
        "AdequateLeaderCandidatePayloadWithinFrozenView",
        "AdequateLeaderImmutableCandidatePayload",
        "AdequateLeaderRouteNeutralCandidateItem",
        "AdequateLeaderRouteNeutralCandidateEvidence",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidatePayloadCarrierIsFinite",
    ): (
        "AdequateLeaderFrozenCandidatePayloadCarrier",
        "AdequateLeaderFrozenNetworkItemCarrier",
        "AdequateLeaderFrozenEvidenceCarrier",
        "AdequateLeaderFrozenCommitRequestItemCarrier",
        "FS_Product",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenOwnerUniverseIsPrimeInvariant",
    ): (
        "AdequateLeaderFrozenOwnerUniverse",
        "AdequateLeaderFrozenCandidateOwnerUniverse",
        "AdequateLeaderFrozenWireOwnerUniverse",
        "AdequateLeaderFrozenCandidateOwnerIdentityFromPayload",
        "AdequateLeaderFrozenCandidatePayloadCarrier",
        "AdequateLeaderFrozenWireOwnerIdentityFromCoordinates",
        "AdequateLeaderFrozenWirePayloadCarrier",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenOwnerUniverseIsFinite",
    ): (
        "AdequateLeaderFrozenOwnerUniverse",
        "AdequateLeaderFrozenCandidateOwnerUniverse",
        "AdequateLeaderFrozenWireOwnerUniverse",
        "AdequateLeaderFrozenCandidatePayloadCarrierIsFinite",
        "AdequateLeaderFrozenWirePayloadCarrier",
        "FS_CardinalityType",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderServicedCandidateIdentityHasServiceWitness",
    ): (
        "AdequateLeaderTargetServicedCandidateOwnerIdentitySet",
        "AsyncCandidateServiceTombstoneLifecycleInvariant",
        "AsyncCandidateServiceTombstoned",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCandidateSuccessfulServiceRetirementInstallsServicedMemory",
    ): (
        "AsyncCandidateSuccessfulServiceInstallsTransientMarker",
        "AsyncCandidateSuccessfulServiceInstallsTombstone",
        "AdequateLeaderTargetCandidateSuccessfulServiceRetirementAction",
        "AdequateLeaderServicedCandidateMemory",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCandidateTerminalRetirementInstallsServicedMemory",
    ): (
        "AsyncCandidateDiscardInstallsTerminalTombstone",
        "AdequateLeaderTargetCandidateTerminalDiscardRetirementAction",
        "AdequateLeaderServicedCandidateMemory",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCandidateRetirementEstablishesClosure",
    ): (
        "AdequateLeaderTargetCandidateOwnerIdentityRetirementAction",
        "AdequateLeaderServicedCandidateClosure",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCandidateSuccessfulServiceRetirementStartsClosedMemory",
    ): (
        "AdequateLeaderCandidateSuccessfulServiceRetirementInstallsServicedMemory",
        "AdequateLeaderCandidateRetirementEstablishesClosure",
        "AdequateLeaderTargetCandidateSuccessfulServiceRetirementAction",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCandidateTerminalRetirementStartsClosedMemory",
    ): (
        "AdequateLeaderCandidateTerminalRetirementInstallsServicedMemory",
        "AdequateLeaderCandidateRetirementEstablishesClosure",
        "AdequateLeaderTargetCandidateTerminalDiscardRetirementAction",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderServicedCandidateMemoryAndClosureAreStepInvariant",
    ): (
        "AsyncNextPreservesCandidateServiceTombstoneLifecycle",
        "AsyncCandidateServicedMarkerPersistsWithoutExit",
        "AsyncCandidateTerminalTombstonePersistsWithoutExit",
        "AsyncCandidateSameGenerationSuccessfulServiceIdentityPersistsUntilStrictExit",
        "AdequateLeaderServicedCandidateIdentityHasServiceWitness",
        "AdequateLeaderLiveAndServicedCandidateIdentitiesAreDisjoint",
        "AdequateLeaderOwnerIdentityDeterminesNetworkServiceIdentity",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncSpecProvidesAdequateLeaderTargetCandidateSuccessfulServiceMemory",
    ): (
        "AsyncSpecAlwaysCandidateServiceTombstoneLifecycle",
        "AdequateLeaderCandidateSuccessfulServiceRetirementInstallsServicedMemory",
        "AdequateLeaderTargetCandidateSuccessfulServiceMemoryProperty",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncSpecProvidesAdequateLeaderTargetCandidateTerminalTombstones",
    ): (
        "AsyncSpecProvidesAdequateLeaderCandidateFrozenIdentityBudgetBridge",
        "AsyncSpecAlwaysStrongTypeInvariant",
        "AsyncSpecAlwaysProgressOwnershipInvariant",
        "AsyncSpecAlwaysCandidateServiceTombstoneLifecycle",
        "AdequateLeaderCandidateTerminalRetirementStartsClosedMemory",
        "AdequateLeaderCandidateRetirementEstablishesClosure",
        "AdequateLeaderServicedCandidateMemoryAndClosureAreStepInvariant",
        "AdequateLeaderTargetCandidateTerminalTombstoneProperty",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncSpecProvidesAdequateLeaderTargetCandidateIdentityTombstones",
    ): (
        "AsyncSpecProvidesAdequateLeaderCandidateFrozenIdentityBudgetBridge",
        "AsyncSpecAlwaysStrongTypeInvariant",
        "AsyncSpecAlwaysProgressOwnershipInvariant",
        "AsyncSpecAlwaysCandidateServiceTombstoneLifecycle",
        "AdequateLeaderCandidateSuccessfulServiceRetirementStartsClosedMemory",
        "AdequateLeaderCandidateTerminalRetirementStartsClosedMemory",
        "AdequateLeaderServicedCandidateMemoryAndClosureAreStepInvariant",
        "AdequateLeaderTargetCandidateIdentityTombstoneProperty",
        "AdequateLeaderTargetCandidateServicedRetirementAction",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderLiveOwnersStayInsideFrozenUniverse",
    ): (
        "AdequateLeaderTargetLiveOwnerIdentitySet",
        "AdequateLeaderTargetLiveCandidateOwnerIdentitySet",
        "AdequateLeaderTargetLiveWireOwnerIdentitySet",
        "AdequateLeaderFrozenOwnerUniverse",
        "AdequateLeaderFrozenTargetCandidateRole",
        "AdequateLeaderFrozenTargetCandidatePayloadIsInStaticCarrier",
        "AdequateLeaderFrozenWirePayloadIdentity",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenWireRetryIdentityIsStable",
    ): (
        "AdequateLeaderFrozenWireOwnerIdentity",
        "AdequateLeaderFrozenWireOwnerIdentityFromCoordinates",
        "AdequateLeaderFrozenWirePayloadIdentity",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetNonDescentEpisodeBudgetIsFiniteAndCoalesced",
    ): (
        "AdequateLeaderFrozenOwnerUniverseIsFinite",
        "AdequateLeaderTargetNonDescentEpisodeBudget",
        "AdequateLeaderTargetEpisodeKnownOwnerSet",
        "FS_Subset",
        "FS_CardinalityType",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCurrentControlOwnerBlocksSameOrLowerRetries",
    ): (
        "AsyncControlServiceSameOrLowerViewCannotReplace",
        "AdequateLeaderTargetSameOrLowerControlRetriesAdmissionBlocked",
        "AdequateLeaderTargetSameOrLowerControlRetry",
        "AsyncControlServiceOccurrenceIsCurrentOwner",
        "CanAdmitIngressItem",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderOffSubjectControlRetirementMemoryIsStepInvariant",
    ): (
        "AsyncControlServiceSameOrLowerViewCannotReplace",
        "AsyncControlServiceServicedIdentityCannotResurrect",
        "AsyncControlServiceTombstoneCannotReactivate",
        "AsyncRetiredControlServiceAdmissionDropsWithoutCandidate",
        "AdequateLeaderTargetOffSubjectControlRetirementMemory",
        "AdequateLeaderTargetOffSubjectControlRetirementClosed",
        "AdequateLeaderTargetSameOrLowerControlRetriesAdmissionBlocked",
        "AsyncControlServiceIdentityServicedOrAdvanced",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderSelectedOccurrenceOwnerIsFrozenAndLive",
    ): (
        "AdequateLeaderLiveOwnersStayInsideFrozenUniverse",
        "AdequateLeaderTargetOccurrenceOwnerSelected",
        "AdequateLeaderTargetOccurrenceOwnerIdentitySet",
        "AdequateLeaderTargetLiveCandidateOwnerIdentitySet",
        "AdequateLeaderFrozenCandidateOwnerUniverse",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCorridorHasProductiveSubjectReentry",
    ): (
        "AdequateLeaderTargetProductiveSubjectReentryGoal",
        "AdequateLeaderTargetProductiveSubjectOpenFrontier",
        "AdequateLeaderTargetProtocolSubjectSource",
        "AdequateLeaderTargetProducerTransportResidual",
        "AdequateLeaderTargetRankFrontier",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderSubjectSwitchNamedOwnerStrictlyConsumesBudget",
    ): (
        "AdequateLeaderFrozenSubjectSwitchOwnerUniverseIsFinite",
        "AdequateLeaderTargetSubjectSwitchDiscoveredOwnerSet",
        "AdequateLeaderTargetSubjectSwitchRemainingBudget",
        "FS_Union",
        "FS_Subset",
        "FS_CardinalityType",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderSubjectSwitchEpisodeStartsNamedOwnerService",
    ): (
        "AdequateLeaderTargetCurrentOwnersInitializeKnownEpisode",
        "AdequateLeaderTargetNonDescentEpisodeBudgetIsFiniteAndCoalesced",
        "AdequateLeaderTargetSubjectSwitchEpisodeAtBudget",
        "AdequateLeaderTargetNonDescentEpisodeAtBudget",
        "AdequateLeaderTargetEpisodeStartsWithCurrentOwners",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderComposedRankDescentClosesOccurrenceService",
    ): (
        "AsyncSpecAlwaysStrongTypeInvariant",
        "AsyncLiveSpecProjectsAsyncSpec",
        "AdequateLeaderTargetOccurrenceFrontierStartsFiniteEpisode",
        "AdequateLeaderKnownAdvanceProjectsToServiceExitBudgetDescent",
        "AdequateLeaderFiniteBudgetDescentClosesNonDescentEpisode",
        "AdequateLeaderTargetOccurrenceRankServiceProperty",
        "AdequateLeaderTargetProducerTransportOccurrenceClosureProperty",
        "AdequateLeaderTargetUniversalOccurrenceServiceGoal",
        "AdequateLeaderTargetProductiveOccurrenceServiceGoal",
        "AdequateLeaderTargetOffSubjectOccurrenceDrainGoal",
        "AdequateLeaderTargetNonDescentKnownAdvanceProperty",
        "AdequateLeaderTargetNonDescentEpisodeClosureProperty",
        "AdequateLeaderTargetNonDescentEpisodeAtBudget",
        "AdequateLeaderTargetNonDescentEpisodeBudgetFrontier",
        "AdequateLeaderTargetRankServiceExitProperty",
        "PTL",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AsyncLiveProvidesAdequateLeaderTargetOffSubjectControlNoReentry",
    ): (
        "AsyncSpecAlwaysStrongTypeInvariant",
        "AsyncSpecAlwaysProgressOwnershipInvariant",
        "AsyncSpecAlwaysCandidateServiceTombstoneLifecycle",
        "AsyncLiveSpecProjectsAsyncSpec",
        "AdequateLeaderOffSubjectControlRetirementMemoryIsStepInvariant",
        "GstAsyncStepIsMonotone",
        "AdequateLeaderTargetOffSubjectControlNoReentryProperty",
        "AdequateLeaderTargetOffSubjectControlRetirementMemory",
        "PTL",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderOwnerIndexedServiceProvidesSubjectSwitchBudgetDescent",
    ): (
        "AsyncLiveProvidesAdequateLeaderTargetOffSubjectControlNoReentry",
        "AsyncLiveProvidesAdequateLeaderTargetInternalBodyAvailableNoReentry",
        "AsyncLiveProvidesAdequateLeaderTargetDurableRetirementCarry",
        "AdequateLeaderTargetOccurrenceRankServiceProperty",
        "AdequateLeaderTargetSubjectSwitchCarryStepProperty",
        "AdequateLeaderTargetSubjectSwitchBudgetDescentProperty",
        "PTL",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderSubjectSwitchCarryStepProvidesAnchoredBudgetDescent",
    ): (
        "AdequateLeaderTargetSubjectSwitchCarryStepProperty",
        "AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentProperty",
        "AdequateLeaderTargetAnchoredSubjectSwitchBudgetFrontier",
        "AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentGoal",
        "AdequateLeaderTargetSubjectSwitchBudgetDescentGoal",
        "AdequateLeaderTargetSubjectSwitchEpisodeAdvanceGoal",
        "AdequateLeaderTargetOffSubjectRetirementAndReentryGoal",
        "AdequateLeaderTargetCarriedOwnerEpisodeAtBudget",
        "PTL",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderSubjectSwitchBudgetDescentClosesNamedOwnerEpisode",
    ): (
        "AdequateLeaderSubjectSwitchCarryStepProvidesAnchoredBudgetDescent",
        "AdequateLeaderTargetSubjectSwitchBudgetDescentProperty",
        "AdequateLeaderTargetSubjectSwitchClosureProperty",
        "AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentProperty",
        "AdequateLeaderTargetAnchoredSubjectSwitchBudgetFrontier",
        "AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentGoal",
        "NatLessThanWellFounded",
        "WellFoundedLeadsTo",
        "PTL",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOccurrenceRankOrderingWellFounded",
    ): (
        "AdequateLeaderTargetSemanticRankOrderingWellFounded",
        "NatLessThanWellFounded",
        "WFLexPairOrdering",
        "AdequateLeaderTargetOccurrenceRankOrdering",
        "AdequateLeaderTargetOccurrenceRankCarrier",
    ),
    (
        "SumeragiV2AdequateLeaderProducerTransportClosureProofs",
        "AdequateLeaderTargetOccurrenceRankFrontierRetainsFrozenCorridor",
    ): (
        "AdequateLeaderTargetOccurrenceRankFrontier",
        "AdequateLeaderTargetRankFrontier",
        "AdequateLeaderTargetCandidateIdentity",
    ),
    (
        "SumeragiV2AdequateLeaderProducerTransportClosureProofs",
        "AdequateLeaderTargetRanksReachIndexedDecision",
    ): (
        "AdequateLeaderTargetOccurrenceRankFrontierRetainsFrozenCorridor",
        "AdequateLeaderAuthorityBoundReceiptAcquisitionProperty",
        "AdequateLeaderAuthorityBoundActiveReceiptServiceProperty",
        "AdequateLeaderTargetAuthorityBoundActiveReceiptSource",
        "PTL",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetConvergenceReachesEveryDecisionPrefix",
    ): (
        "AdequateLeaderTargetConvergenceDecidesFixedFrozenVoter",
        "AdequateLeaderDecisionPrefixAtIsStable",
        "AdequateLeaderAsyncBracketStepPreservesTargetDecision",
        "AdequateLeaderDecisionPrefixAt",
        "NatInduction",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "FrozenContextFullAdequateLeaderDecisionPrefixImpliesResponsiveDecide",
    ): (
        "FrozenContextFixesResponsiveVoters",
        "AdequateLeaderDecisionPrefixAt",
        "ResponsiveNodesDecide",
        "AsyncVotersAt",
        "ValidatorIds",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSemanticCompositionSuppliesTargetConvergence",
    ): (
        "AdequateLeaderSemanticCompositionSuppliesLocalTargetConvergence",
        "AdequateLeaderLocalConvergenceSuppliesReachedViewConvergence",
        "AdequateLeaderSemanticCompositionProperty",
        "AdequateLeaderTargetSemanticCompositionProperty",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetDecisionConvergenceSuppliesDecisionMode",
    ): (
        "AdequateLeaderTargetConvergenceReachesEveryDecisionPrefix",
        "FrozenContextFullAdequateLeaderDecisionPrefixImpliesResponsiveDecide",
        "AdequateLeaderDecisionPrefixAt",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "ExactAdequateLeaderSubkernelsReduceToServiceKernel",
    ): (
        "AdequateLeaderTargetSemanticCompositionSuppliesTargetConvergence",
        "AdequateLeaderSemanticCompositionSuppliesLocalTargetConvergence",
        "AdequateLeaderLocalTargetConvergenceSuppliesDecisionConvergence",
        "AdequateLeaderViewReachCompositionProperty",
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AdequateLeaderServiceKernelObligation",
    ): (
        "AdequateLeaderExactClosureResidualObligation",
        "AdequateLeaderLocalTargetConvergenceSuppliesDecisionConvergence",
        "AdequateLeaderExactClosureResidualProperty",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionCachedRequestIngressRunnerBypassesServeLifecycle",
    ): (
        "ExactDecisionCachedRequestIngressRunnerCreatesResponseOwner",
        "ExactDecisionRequestIngressRunnerAction",
        "ExactDecisionServeTombstoneOwned",
        "AsyncServeLifecycleTombstone",
        "AsyncServeLiveReservationOwned",
        "AsyncServeJobQueued",
        "AsyncServeLifecyclePartitionInvariant",
        "AcceptOrCoalesceExactServeRequest",
        "CoalesceExactServeCapacity",
        "AsyncServeCachedReplayItems",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestFrozenServeBarrierIsSingleton",
    ): (
        "ExactDecisionRequestFrozenServeBarrierIdentities",
        "ExactDecisionRequestFrozenServeBarrierIdentity",
        "AsyncServePreexistingIngressBarrierIdentities",
        "AsyncServeIngressLiveReservations",
        "AsyncServeSingularOffQueueBarrierInvariant",
        "AsyncServeLifecycleTypeInvariant",
        "FS_CardinalityType",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestFrozenServeBarrierMaterializationLowersRank",
    ): (
        "ExactDecisionRequestFrozenServeBarrierIsSingleton",
        "ExactDecisionRequestLifecycleFrozenOwnersDoNotReplenish",
        "ExactDecisionRequestFrozenServeBarrierMaterializationAction",
        "ExactDecisionRequestFrozenServeBarrierIdentities",
        "ExactDecisionRequestLifecycleFrozenPredecessorDebt",
        "ExactDecisionRequestLifecycleIngressRank",
        "AsyncServePreexistingIngressBarrierIdentities",
        "AsyncServeIngressIdentityFrozenByReservation",
        "AsyncServeSingularOffQueueBarrierInvariant",
        "ResumeExactServeCapacity",
        "AcceptOrCoalesceExactServeRequest",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestFrozenServeBarrierPreservesTargetIngressCoalescing",
    ): (
        "ExactDecisionRequestFrozenServeBarrierMaterializationLowersRank",
        "ExactDecisionRequestLifecycleResidual",
        "ExactDecisionRequestIngressOwned",
        "ExactDecisionServeTombstoneOwned",
        "ExactDecisionRequestFrozenServeBarrierMaterializationAction",
        "ExactDecisionRequestFrozenServeBarrierIdentities",
        "AsyncServeLifecycleTombstone",
        "AsyncServeReservationsAfterIngressDrain",
        "AcceptOrCoalesceExactServeRequest",
        "IngressResourceSource",
        "IngressLane",
        "SequenceSet",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleIngressRankOrderingIsWellFounded",
    ): (
        "ExactDecisionRequestIngressRankOrderingIsWellFounded",
        "NatLessThanWellFounded",
        "WFLexPairOrdering",
        "ExactDecisionRequestLifecycleIngressRankOrdering",
        "ExactDecisionRequestLifecycleIngressRankCarrier",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleIngressRankInCarrier",
    ): (
        "ExactDecisionRequestIngressRankInCarrier",
        "ExactDecisionRequestLifecycleFrozenPredecessorSet",
        "ExactDecisionRequestLifecycleNestedIngressRank",
        "ExactDecisionRequestLifecycleIngressRankCarrier",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressProducerEpisodeBudgetIsFinite",
    ): (
        "ExactDecisionRequestIngressProducerEpisodeOwnerSet",
        "ExactDecisionRequestIngressProducerEpisodeBudget",
        "ExactDecisionRequestIngressProducerEpisodeStaticBound",
        "ExactDecisionRequestLifecycleFrozenPredecessorSet",
        "AsyncServePreexistingIngressBarrierIdentities",
        "AsyncServeIngressIdentityFrozenByReservation",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleFrozenOwnerServiceConsumesBudget",
    ): (
        "ExactDecisionRequestLifecycleFrozenOwnerServiceAction",
        "ExactDecisionRequestIngressProducerEpisodeBudget",
        "ExactDecisionRequestIngressProducerEpisodeOwnerSet",
        "AsyncServeReservationsAfterIoService",
        "AsyncServeReservationsAfterIngressDrain",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleFrozenOwnersDoNotReplenish",
    ): (
        "ExactDecisionRequestLifecycleFrozenPredecessorSet",
        "AsyncServePreexistingIngressBarrierIdentities",
        "AsyncServeIngressIdentityFrozenByReservation",
        "AsyncServeReservationsAfterIoService",
        "AsyncServeReservationsAfterIngressDrain",
        "ResetNodeSchedulerForRestart",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleOrdinalCannotResurrect",
    ): (
        "AsyncServeAdmissionOrdinal",
        "AsyncServeLifecycleOwned",
        "AsyncServeLifecycleTombstone",
        "ResetNodeSchedulerForRestart",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleStepClassificationIsExhaustive",
    ): (
        "ExactDecisionRequestLifecycleFrozenOwnersDoNotReplenish",
        "ExactDecisionRequestLifecycleFrozenOwnerServiceConsumesBudget",
        "ExactDecisionRequestLifecycleOrdinalCannotResurrect",
        "ExactDecisionRequestLifecycleStepClassification",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleSelectedActionEnabledAtEpisode",
    ): (
        "QueuedIoEnablesPostGstService",
        "GstResponsiveUnappliedRunNodeIsEnabled",
        "GstHistoricalServerIsEnabled",
        "ExpandENABLED",
        "ExactDecisionRequestLifecycleSelectedConcreteFairAction",
        "ExactDecisionRequestLifecycleConcreteFairAction",
        "ExactDecisionRequestLifecycleConcreteFairOwner",
        "ExactDecisionRequestLifecycleIoOwnerRequired",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleBracketStepPreservesEpisodeOrGoal",
    ): (
        "ExactDecisionRequestLifecycleStepClassification",
        "ExactDecisionRequestIngressFiniteProducerEpisodeAction",
        "ExactDecisionRequestLifecycleNoninterferenceAction",
        "ExactDecisionRequestLifecycleAtRankAndBudget",
        "ExactDecisionRequestLifecycleRankGoal",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleConcreteOwnerPersistsInRankCell",
    ): (
        "ExactDecisionRequestLifecycleFrozenOwnersDoNotReplenish",
        "ExactDecisionRequestLifecycleOrdinalCannotResurrect",
        "ExactDecisionRequestLifecycleConcreteFairOwner",
        "ExactDecisionRequestLifecycleIoOwnerRequired",
        "ExactDecisionRequestLifecycleFrozenPredecessorSet",
        "ExactDecisionRequestIngressProducerEpisodeOwnerSet",
        "ExactDecisionRequestFrozenServeBarrierIdentities",
        "ExactDecisionRequestFrozenServeBarrierIdentity",
        "AsyncServePreexistingIngressBarrierIdentities",
        "AsyncServeIngressIdentityFrozenByReservation",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleSelectedActionConsumesEpisode",
    ): (
        "ExactDecisionRequestIngressIoServicePersistsAndLowers",
        "ExactDecisionRequestIngressRunnerActionCreatesGoal",
        "ExactDecisionRequestIngressLocalProducerCannotAscendRank",
        "ExactDecisionRequestIngressOlderLocalInterleaveCannotAscendRank",
        "ExactDecisionRequestIngressRuntimeActivationCannotAscendRank",
        "ExactDecisionRequestEarlierIngressOwnerServiceLowersRank",
        "ExactDecisionRequestFrozenServeBarrierPredecessorServiceLowersRank",
        "ExactDecisionRequestFrozenServeBarrierMaterializationLowersRank",
        "ExactDecisionRequestLifecycleFrozenOwnerServiceConsumesBudget",
        "ExactDecisionRequestLifecycleFrozenOwnersDoNotReplenish",
        "ExactDecisionRequestLifecycleSelectedConcreteFairAction",
        "ExactDecisionRequestLifecycleConcreteFairAction",
        "ExactDecisionRequestLifecycleConcreteFairOwner",
        "ExactDecisionRequestLifecycleAtRankAndBudget",
        "ExactDecisionRequestLifecycleRankCellOutcome",
        "ExactDecisionRequestEarlierServeMaterializationAction",
        "ExactDecisionRequestFrozenServeBarrierMaterializationAction",
        "ExactDecisionRequestFrozenServeBarrierIdentities",
        "ExactDecisionRequestFrozenServeBarrierIdentity",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleConcreteOwnerUsesAsyncFairness",
    ): (
        "AsyncSpecAt",
        "AsyncFairnessAt",
        "ExactDecisionRequestLifecycleConcreteFairOwnerKinds",
        "ExactDecisionRequestLifecycleConcreteFairAction",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "AsyncSpecProvidesExactDecisionRequestLifecycleConcreteActionOrigin",
    ): (
        "AsyncSpecAlwaysStrongTypeInvariant",
        "AsyncSpecAlwaysProgressOwnershipInvariant",
        "AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage",
        "AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity",
        "ExactDecisionRequestLifecycleSelectedActionEnabledAtEpisode",
        "ExactDecisionRequestLifecycleBracketStepPreservesEpisodeOrGoal",
        "ExactDecisionRequestLifecycleConcreteOwnerPersistsInRankCell",
        "ExactDecisionRequestLifecycleSelectedActionConsumesEpisode",
        "ExactDecisionRequestLifecycleConcreteActionOriginProperty",
        "ExactDecisionRequestLifecycleRankCellOutcome",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "AsyncSpecProvidesExactDecisionRequestLifecycleRankDescent",
    ): (
        "AsyncSpecProvidesExactDecisionRequestLifecycleConcreteActionOrigin",
        "AsyncSpecAlwaysStrongTypeInvariant",
        "AsyncSpecAlwaysProgressOwnershipInvariant",
        "ExactDecisionRequestLifecycleStepClassificationIsExhaustive",
        "ExactDecisionRequestLifecycleRankDescentProperty",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestRankDescentDerivesFiniteEpisodeClosure",
    ): (
        "AsyncSpecAlwaysStrongTypeInvariant",
        "AsyncSpecAlwaysProgressOwnershipInvariant",
        "AsyncSpecAlwaysUsesFixedResponsiveVoters",
        "ExactDecisionRequestLifecycleConcreteOwnerUsesAsyncFairness",
        "ExactDecisionRequestLifecycleRankDescentProperty",
        "ExactDecisionRequestLifecycleConcreteActionOriginProperty",
        "ExactDecisionRequestIngressContinuationPrefixClosureProperty",
        "ExactDecisionRequestIngressContinuationPrefixGoal",
        "ExactDecisionRequestIngressContinuationPrefixCleared",
        "ExactDecisionRequestLifecycleRankCellOutcome",
        "ExactDecisionRequestLifecycleFiniteProducerEpisodeClosureProperty",
        "PTL",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestFiniteProducerEpisodeClosesAtRank",
    ): (
        "ExactDecisionRequestRankDescentDerivesFiniteEpisodeClosure",
        "ExactDecisionRequestIngressProducerEpisodeBudgetIsFinite",
        "ExactDecisionRequestLifecycleFrozenOwnersDoNotReplenish",
        "ExactDecisionRequestLifecycleOrdinalCannotResurrect",
        "NatLessThanWellFounded",
        "WellFoundedLeadsTo",
        "ExactDecisionRequestLifecycleRankCellClosureProperty",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleRankDescentClosesLifecycle",
    ): (
        "ExactDecisionRequestFiniteProducerEpisodeClosesAtRank",
        "ExactDecisionRequestLifecycleIngressRankOrderingIsWellFounded",
        "ExactDecisionRequestLifecycleIngressRankInCarrier",
        "WellFoundedLeadsTo",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressReplenishmentHasConcreteActionWitness",
    ): (
        "ExactDecisionRequestIngressCausalReplenishmentHasConcreteProducer",
        "ExactDecisionRequestIngressServeReplenishmentHasConcreteProducer",
        "ExactDecisionRequestIngressPriorityReplenishmentHasConcreteProducer",
        "ExactDecisionRequestIngressRankReplenishmentResidual",
        "ExactDecisionRequestIngressConcreteReplenishmentAction",
        "ExactDecisionRequestIngressProducerClasses",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestAdmissionCoalescingOutcomeIsDischarged",
    ): (
        "AsyncSpecProvidesExactDecisionRequestLifecycleRankDescent",
        "AsyncSpecClosesExactDecisionRequestIngressContinuationPrefix",
        "ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestAdmissionCoalescingClosesLaneRunner",
    ): (
        "ExactDecisionRequestLifecycleRankDescentClosesLifecycle",
        "ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty",
        "ExactDecisionRequestLifecycleConvergenceProperty",
        "ExactDecisionRequestIngressLaneRunnerConvergenceProperty",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestEmissionKernelsDischargeResidual",
    ): (
        "PTL",
        "ExactDecisionRequestClockOwnerConvergenceProperty",
        "ExactDecisionRequestRuntimePrefixConvergenceProperty",
        "ExactDecisionRequestPacketEmissionResidualConvergenceProperty",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressKernelsDischargeResidual",
    ): (
        "ExactDecisionRequestAdmissionHandoffConvergence",
        "ExactDecisionRequestAdmissionCoalescingClosesLaneRunner",
        "ExactDecisionRequestIngressResidualSplitsAtAdmissionReady",
        "ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty",
        "ExactDecisionRequestIngressResidualConvergenceProperty",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionServeResponseResidualConvergence",
    ): (
        "ExactDecisionServeExitSafetyKernel",
        "ExactDecisionServeExitSafetyDischargesResidual",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionResponseClaimKernelNarrowsNonPhysicalResidual",
    ): (
        "ExactDecisionResponseClaimContentionConvergence",
        "ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergenceProperty",
        "ExactDecisionResponseNonPhysicalHeadGateOwnerConvergenceProperty",
        "PTL",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionResponsePhysicalKernelNarrowsHeadGateResidual",
    ): (
        "ExactDecisionResponsePhysicalCompletionConvergence",
        "ExactDecisionResponseAdmissionResidualSplitsAtReady",
        "ExactDecisionResponseHeadGateOwnerConvergenceProperty",
        "PTL",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionResponseAdmissionKernelsDischargeResidual",
    ): (
        "ExactDecisionResponseAdmissionHandoffConvergence",
        "ExactDecisionResponseClaimIngressRunnerConvergence",
        "ExactDecisionResponseAdmissionResidualSplitsAtReady",
        "ExactDecisionResponseAdmissionResidualConvergenceProperty",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionOffSchedulerResidualsDischargeKernels",
    ): (
        "ExactDecisionRequestEmissionKernelsDischargeResidual",
        "ExactDecisionRequestIngressKernelsDischargeResidual",
        "ExactDecisionServeResponseResidualConvergence",
        "ExactDecisionResponseClaimKernelNarrowsNonPhysicalResidual",
        "ExactDecisionResponsePhysicalKernelNarrowsHeadGateResidual",
        "ExactDecisionResponseAdmissionKernelsDischargeResidual",
        "ExactDecisionOffSchedulerResidualConvergenceProperty",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionResidualKernelsDischargeStageService",
    ): (
        "ExactDecisionServiceSourceDecomposition",
        "ExactDecisionLocalStage2ClosesPhysicalOwnerExit",
        "ExactDecisionPhysicalExitClosesCandidatePipeline",
        "ExactDecisionRequestHasResponsiveBodyHoldingAlias",
        "ExactRequestPacketAdmissionCreatesIngressOwner",
        "NormalExactRequestIngressCreatesFreshServeOwner",
        "HistoricalExactRequestIngressCreatesFreshServeOwner",
        "ExactServeHeadCreatesAuthenticatedResponsePacket",
        "FreshExactResponsePacketAdmissionAcquiresRecipientClaim",
        "ExactResponsePacketCoalescingRetainsRouteNeutralClaim",
        "ExactResponseIngressDrainAtomicallyRetiresAliasesAndCreatesFetchOwner",
        "ExactDecisionFetchHeldBodySchedulesValidation",
        "ExactCertifiedFetchStagesBodyAndSchedulesStore",
        "DecisionStoreSchedulesValidation",
        "DecisionValidationSchedulesApply",
        "DecisionApplyCreatesTerminalStage",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionOffSchedulerResidualConvergenceDischargesStageService",
    ): (
        "ExactDecisionStage2BusyClosure",
        "ExactDecisionOffSchedulerResidualsDischargeKernels",
        "ExactDecisionResidualKernelsDischargeStageService",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionAsyncInitEstablishesCandidateTombstones",
    ): (
        "AsyncInitAt",
        "AsyncBaseInitAt",
        "AsyncCandidateServiceLifecycleInvariant",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionAsyncNextPreservesCandidateTombstones",
    ): (
        "AsyncNextPreservesControlServiceStateTypeInvariant",
        "AsyncCandidateSuccessfulServiceInstallsTransientMarker",
        "AsyncCandidateTerminalRetirementsThisStepIsSingleton",
        "AsyncCandidateDiscardInstallsTerminalTombstone",
        "AsyncCandidateDiscardRetiresLogicalLifecycle",
        "AsyncCandidateTransientMarkerCoalescesFreshCandidate",
        "AsyncCandidateTerminalTombstoneCoalescesFreshCandidate",
        "AsyncCandidateServiceTombstoneRejectsTransportReadmission",
        "AsyncCandidateSameHeightRestartPreservesTombstone",
        "AsyncCandidateResponsiveRestartPermitsNonterminalReconstruction",
        "AsyncNext",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionAsyncSpecAlwaysCandidateTombstones",
    ): (
        "ExactDecisionAsyncInitEstablishesCandidateTombstones",
        "ExactDecisionAsyncNextPreservesCandidateTombstones",
        "AsyncSpecAlwaysStrongTypeInvariant",
        "AsyncSpecAlwaysProgressOwnershipInvariant",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionSameGenerationCandidateServiceCannotReactivateAtGst",
    ): (
        "AsyncCandidateSameGenerationServicedIdentityCannotReactivateAtGst",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTerminalCandidateDiscardCannotReactivateAtGst",
    ): (
        "AsyncCandidateTerminalIdentityCannotReactivateAtGst",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionResponsiveRestartPermitsNonterminalCandidateReconstruction",
    ): (
        "AsyncCandidateResponsiveRestartPermitsNonterminalReconstruction",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralSnapshotIsFinite",
    ): (
        "StrongTypeHasFiniteHistoricalDiscoveryCohorts",
        "StrongTypeHasFiniteHistoricalDiscoveryRankOwners",
        "RuntimeValidatorIdsAreFinite",
        "ExactDecisionTargetNeutralFixedPredecessorSet",
        "AsyncCandidateProducerEpisodeBudget",
        "AsyncCandidateProducerEpisodeCapacity",
        "AsyncServeLifecycleFamilyBudget",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralEpisodeBudgetIsNatural",
    ): (
        "FS_Interval",
        "ExactDecisionTargetNeutralCandidateOrdinalTokens",
        "ExactDecisionTargetNeutralServeOrdinalTokens",
        "ExactDecisionTargetNeutralSnapshotActive",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralPacketDependencyRankInCarrier",
    ): (
        "ScheduledCandidateServiceRankInCarrier",
        "HistoricalDiscoveryPacketServeOwnerRankInCarrier",
        "HistoricalDiscoveryOwnedRankMinimumFacts",
        "IngressGateOwnerDebtsAreFiniteNaturals",
        "Stage4CapacityRankInCarrier",
        "ExactDecisionTargetNeutralCandidateOccurrenceRank",
        "ExactDecisionTargetNeutralServeOccurrenceRank",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank",
    ): (
        "HistoricalDiscoveryFixedClockIngressRemovesOneDuePacket",
        "ExactDecisionTargetNeutralPacketDependencyRankInCarrier",
        "BoundedTransportServiceRank",
        "HistoricalDiscoveryPacketDependencyOrdering",
        "HistoricalDiscoveryPacketDependencyCarrier",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralConcreteRankInCarrier",
    ): (
        "ExactDecisionTargetNeutralPacketDependencyRankInCarrier",
        "HistoricalDiscoveryFixedClockRankShapeInCarrier",
        "HistoricalDiscoveryIngressCounterRankInCarrier",
        "ExactDecisionTargetNeutralConcreteBlockerStage",
        "ExactDecisionTargetNeutralConcreteDependencyRank",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockOrderingIsWellFounded",
    ): (
        "HistoricalDiscoveryFixedClockBlockerOrderingIsWellFounded",
        "ExactDecisionTargetNeutralFixedClockOrdering",
        "ExactDecisionTargetNeutralFixedClockCarrier",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets",
    ): (
        "HistoricalDiscoveryPublicationHelpersHaveFixedClockFrame",
        "HistoricalDiscoveryBroadcastControlHelpersHaveFixedClockFrame",
        "HistoricalDiscoveryRetransmissionHelpersHaveFixedClockFrame",
        "HistoricalDiscoveryDirectRequestPublicationHasFixedClockFrame",
        "HistoricalDiscoveryResponsePublicationHasFixedClockFrame",
        "HistoricalDiscoveryByzantineCertifiedRequestHasFixedClockFrame",
        "HistoricalDiscoverySingletonFaultInjectorsHaveFixedClockFrame",
        "HistoricalDiscoveryFixedClockIngressRemovesOneDuePacket",
        "AsyncNext",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralLaterWorkCannotAcquirePredecessor",
    ): (
        "ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets",
        "AsyncServeIngressTicketExcludesLaterLocalWork",
        "AsyncServeIngressDuplicateDoesNotAllocateOrdinal",
        "SameHeightRestartPreservesServeHighWatermarks",
        "AsyncCandidateTransientMarkerCoalescesFreshCandidate",
        "AsyncCandidateTerminalTombstoneCoalescesFreshCandidate",
        "AsyncCandidateServiceTombstoneRejectsTransportReadmission",
        "ExactDecisionSameGenerationCandidateServiceCannotReactivateAtGst",
        "ExactDecisionTerminalCandidateDiscardCannotReactivateAtGst",
        "ExactDecisionTargetNeutralFrozenLifecycleCoveragePersists",
        "ExactDecisionTargetNeutralFixedPredecessorSet",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralServeOrdinalAdvanceLowersFrozenPacketRank",
    ): (
        "HistoricalDiscoveryFixedClockIngressRemovesOneDuePacket",
        "HistoricalLatentOwnerDebtCannotIncreaseAtFixedClock",
        "HistoricalDiscoveryFixedClockLexStepStrictlyDescends",
        "ExactDecisionRequestHeadGateResidualStepIsSafe",
        "ExactDecisionResponseHeadGateResidualStepIsSafe",
        "PostGstAdmitHiddenPacket",
        "AdmitIngressPacket",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralOrdinalCeilingsCarryUntilStrictRankGoal",
    ): (
        "AsyncCausalEpisodeExactOccurrenceBudgetFitsConfiguredEpisode",
        "AsyncCausalEpisodeOwnedCutServiceConsumesExactOccurrenceBudget",
        "AsyncCausalEpisodeServicedCandidateConsumesExactOccurrenceBudget",
        "AsyncCausalEpisodeSameOriginHandoffRetainsLifecycleCut",
        "ExactDecisionTargetNeutralServeOrdinalAdvanceLowersFrozenPacketRank",
        "ExactDecisionTargetNeutralLaterWorkCannotAcquirePredecessor",
        "AsyncCandidateProducerEpisodeBudget",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralNonGoalEpisodeHasRemainingOrdinal",
    ): (
        "AsyncCausalEpisodeExactOccurrenceBudgetFitsConfiguredEpisode",
        "AsyncCausalEpisodeOwnedCutServiceConsumesExactOccurrenceBudget",
        "AsyncCausalEpisodeServicedCandidateConsumesExactOccurrenceBudget",
        "AsyncCausalEpisodeSameOriginHandoffRetainsLifecycleCut",
        "ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank",
        "ExactDecisionTargetNeutralServeOrdinalAdvanceLowersFrozenPacketRank",
        "ExactDecisionTargetNeutralOrdinalCeilingsCarryUntilStrictRankGoal",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralNonDescentConsumesOrdinal",
    ): (
        "AsyncCandidateSuccessfulServiceAllocatesExactOrdinal",
        "AsyncCandidateTerminalDiscardAllocatesExactOrdinal",
        "ExactDecisionSameGenerationCandidateServiceCannotReactivateAtGst",
        "AsyncCandidateTransientMarkerCoalescesFreshCandidate",
        "AsyncCandidateTerminalTombstoneCoalescesFreshCandidate",
        "AsyncServeIngressDuplicateDoesNotAllocateOrdinal",
        "ExactDecisionRequestLifecycleOrdinalCannotResurrect",
        "ExactDecisionServeTombstoneSurvivesSameHeightReplay",
        "CommandSuccessorsHaveBoundedLength",
        "CommandSuccessorInventoryIsClosed",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralRankCellHasConcreteFairOwner",
    ): (
        "HistoricalDiscoveryFixedClockBlockerCharacterization",
        "AsyncTickEnabledHasConcreteSuccessor",
        "OverdueResponsivePacketEnablesConcreteProgress",
        "DueNodeServiceEnablesConcreteGateProgress",
        "DueIoServiceEnablesConcreteLocalProgress",
        "ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank",
        "ExactDecisionTargetNeutralNonDescentConsumesOrdinal",
        "ExactDecisionTargetNeutralLaterWorkCannotAcquirePredecessor",
        "ExactDecisionTargetNeutralConcreteRankInCarrier",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralSelectedOwnerIsReady",
    ): (
        "ExactDecisionTargetNeutralRankCellHasConcreteFairOwner",
        "ExactDecisionTargetNeutralSelectedFairOwner",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness",
    ): (
        "AsyncSpecAt",
        "AsyncFairnessAt",
        "ExactDecisionTargetNeutralFairAction",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralRankCellStepIsSafe",
    ): (
        "ExactDecisionTargetNeutralLaterWorkCannotAcquirePredecessor",
        "ExactDecisionTargetNeutralAtomicAdmissionLowersPacketRank",
        "ExactDecisionTargetNeutralNonDescentConsumesOrdinal",
        "ExactDecisionRequestHeadGateResidualStepIsSafe",
        "ExactDecisionResponseHeadGateResidualStepIsSafe",
        "ExactDecisionAsyncNextPreservesCandidateTombstones",
        "ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralSelectedOwnerConsumesRankCell",
    ): (
        "ExactDecisionTargetNeutralSelectedOwnerIsReady",
        "ExactDecisionTargetNeutralRankCellHasConcreteFairOwner",
        "ExactDecisionTargetNeutralOrdinalCeilingsCarryUntilStrictRankGoal",
        "ExactDecisionTargetNeutralNonGoalEpisodeHasRemainingOrdinal",
        "ENABLEDaxioms",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralLastOrdinalForcesStrictRankGoal",
    ): (
        "ExactDecisionTargetNeutralSelectedOwnerConsumesRankCell",
        "ExactDecisionTargetNeutralRankCellOutcome",
        "SetLessThan",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFairEpisodeStep",
    ): (
        "AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage",
        "AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity",
        "ExactDecisionAsyncSpecAlwaysCandidateTombstones",
        "ExactDecisionTargetNeutralSelectedOwnerIsReady",
        "ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness",
        "ExactDecisionTargetNeutralRankCellStepIsSafe",
        "ExactDecisionTargetNeutralSelectedOwnerConsumesRankCell",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFiniteEpisodeClosesRankCell",
    ): (
        "ExactDecisionTargetNeutralFairEpisodeStep",
        "ExactDecisionTargetNeutralLastOrdinalForcesStrictRankGoal",
        "NatLessThanWellFounded",
        "WellFoundedLeadsTo",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockRankStep",
    ): (
        "ExactDecisionTargetNeutralEpisodeBudgetIsNatural",
        "ExactDecisionTargetNeutralFiniteEpisodeClosesRankCell",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockConverges",
    ): (
        "ExactDecisionTargetNeutralSnapshotIsFinite",
        "ExactDecisionTargetNeutralConcreteRankInCarrier",
        "ExactDecisionTargetNeutralFixedClockOrderingIsWellFounded",
        "ExactDecisionTargetNeutralFixedClockRankStep",
        "WellFoundedLeadsTo",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralNonTickNonRunnerStepLeavesClock",
    ): (
        "AsyncFaultStepLeavesDiscoveryClock",
        "AsyncNonRunnerStep",
        "AsyncNetworkStep",
        "ServiceIoWorker",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralNonTickAsyncNextLeavesClock",
    ): (
        "ExactDecisionTargetNeutralNonTickNonRunnerStepLeavesClock",
        "AsyncRunnerStepLeavesDiscoveryClock",
        "PreGstResponsiveReplay",
        "AsyncNext",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralEveryNonTickSourceStepLeavesClock",
    ): (
        "ExactDecisionTargetNeutralNonTickAsyncNextLeavesClock",
        "AsyncAllVars",
        "AsyncNext",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralDueHeadDisablesTick",
    ): (
        "ExactDecisionRequestHasResponsiveBodyHoldingAlias",
        "ExactDecisionResponsePacketIsAuthorized",
        "ExactDecisionResponseRemainingHeadGateIsDeadlineOrShadow",
        "AsyncTickEnabled",
        "OverdueResponsivePackets",
        "AsyncPacketOwnsClockDeadline",
        "AsyncServeTransportAdmissionGateAllows",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralDueHeadStepLeavesClockOrGoals",
    ): (
        "ExactDecisionTargetNeutralDueHeadDisablesTick",
        "ExactDecisionTargetNeutralEveryNonTickSourceStepLeavesClock",
        "ExactDecisionRequestHeadGateResidualStepIsSafe",
        "ExactDecisionResponseHeadGateResidualStepIsSafe",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralFixedClockLowersDeadlineBudget",
    ): (
        "ExactDecisionTargetNeutralSnapshotIsFinite",
        "ExactDecisionTargetNeutralFixedClockConverges",
        "ExactDecisionTargetNeutralClockBudgetFrontier",
        "ExactDecisionTargetNeutralClockBudgetGoal",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralDeadlineBudgetConverges",
    ): (
        "ExactDecisionTargetNeutralFixedClockLowersDeadlineBudget",
        "NatLessThanWellFounded",
        "WellFoundedLeadsTo",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralResidualReachesDeadlineOrGoal",
    ): (
        "ExactDecisionTargetNeutralDeadlineBudgetConverges",
        "AsyncSpecAlwaysStrongTypeInvariant",
        "ExactDecisionTargetNeutralClockBudgetFrontier",
        "ExactDecisionTargetNeutralDeadline",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralDueHeadReachesReadyGoal",
    ): (
        "ExactDecisionTargetNeutralSnapshotIsFinite",
        "ExactDecisionTargetNeutralFixedClockConverges",
        "ExactDecisionTargetNeutralDueHeadDisablesTick",
        "ExactDecisionTargetNeutralDueHeadStepLeavesClockOrGoals",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestClockOwnerConvergence",
    ): (
        "ExactDecisionTargetNeutralResidualReachesDeadlineOrGoal",
        "ExactDecisionTargetNeutralDeadline",
        "ExactDecisionRequestRetransmitArmedResidual",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestHeadGateOwnerConvergence",
    ): (
        "ExactDecisionTargetNeutralResidualReachesDeadlineOrGoal",
        "ExactDecisionTargetNeutralDueHeadReachesReadyGoal",
        "ExactDecisionTargetNeutralDeadline",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergence",
    ): (
        "ExactDecisionTargetNeutralResidualReachesDeadlineOrGoal",
        "ExactDecisionTargetNeutralDueHeadReachesReadyGoal",
        "ExactDecisionTargetNeutralDeadline",
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "ExactDecisionOffSchedulerResidualConvergenceObligation",
    ): (
        "ExactDecisionRequestClockOwnerConvergence",
        "ExactDecisionRequestRuntimePrefixConvergence",
        "ExactDecisionRequestHeadGateOwnerConvergence",
        "ExactDecisionRequestAdmissionCoalescingOutcomeIsDischarged",
        "ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergence",
        "ExactDecisionOffSchedulerResidualConvergenceProperty",
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "ExactDecisionStageServiceObligation",
    ): (
        "AsyncSpecProvidesProtectedServiceFiniteRunnerEpisodeClosure",
        "ExactDecisionOffSchedulerResidualConvergenceObligation",
        "ExactDecisionOffSchedulerResidualConvergenceDischargesStageService",
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureApplicationCompletionProgressObligation",
    ): (
        "ExactDecisionStageServiceObligation",
        "ApplicationCompletionProgressReduction",
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureTimeoutViewProgressReduction",
    ): (
        "DirectTimeoutViewDecompositionClosesTimeoutViewProgress",
        "DirectTimeoutViewClosureResidualObligation",
        "AsyncTemporalClosureTimeoutViewProgressObligation",
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureRotatingLeaderProgressObligation",
    ): (
        "AdequateLeaderServiceKernelObligation",
        "AdequateLeaderServiceKernelSuppliesRotatingLeaderProgress",
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "ResponsiveDecisionConvergenceClosesLockedBodyReproposal",
    ): (
        "ResponsiveDecisionConvergenceProperty",
        "AsyncSpecAlwaysUsesFixedResponsiveVoters",
        "LockedBodyReproposalProgressProperty",
        "LockedBodyReproposalOutcome",
    ),
    (
        "SumeragiV2AsyncTemporalClosureProofs",
        "AsyncTemporalClosureLockedBodyReproposalProgressObligation",
    ): (
        "AsyncTemporalClosureRotatingLeaderProgressObligation",
        "RotatingLeaderProgressSuppliesResponsiveDecisionConvergence",
        "ResponsiveDecisionConvergenceClosesLockedBodyReproposal",
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "DirectRetainedLockDecompositionReachesOutcomeOrHigherLeader",
    ): (
        "RetainedLockOwnerNeutralRankHandoffClosesRankedFrontier",
        "RetainedLockSourceAuthorityExposureProperty",
        "RetainedLockPrepareAuthorityTransportProperty",
        "RetainedLockTargetLeaderFreshActivationProperty",
        "RetainedLockLeaderProducerOriginProperty",
        "RetainedLockRankedFrontier",
        "RetainedLockOutcomeOrHigherLeaderProgressProperty",
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockSeparatedProducerProvidersCloseEpisodeResidual",
    ): (
        "RetainedLockSameOriginLifecycleDispositionClosureProperty",
        "RetainedLockCrossOriginProducerReplacementClosureProperty",
        "RetainedLockProducerExactReentryClosureProperty",
        "RetainedLockProducerNonDescentEpisodeClosureProperty",
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockNonDescentClosureClosesRankHandoff",
    ): (
        "AsyncLiveProvidesRetainedLockSameOriginProducerNonDescentClosure",
        "RetainedLockSameOriginProducerNonDescentClosureProperty",
        "RetainedLockProducerNonDescentEpisodeClosureProperty",
        "RetainedLockOwnerNeutralRankHandoffProperty",
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockOwnerNeutralRankHandoffClosesFixedCorridor",
    ): (
        "RetainedLockSemanticRankOrderingWellFounded",
        "WellFoundedLeadsTo",
        "RetainedLockOwnerNeutralRankHandoffProperty",
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "RetainedLockOwnerNeutralRankHandoffClosesRankedFrontier",
    ): (
        "RetainedLockOwnerNeutralRankHandoffClosesFixedCorridor",
        "RetainedLockRankedFrontier",
        "RetainedLockRankedEpisodeFrontier",
        "RetainedLockOwnerNeutralCandidateRankFrontier",
        "RetainedLockStrictHigherFreshLeaderAuthorityFrontier",
    ),
    (
        "SumeragiV2LockedBodyReproposalProgressProofs",
        "DirectRetainedLockOwnerNeutralDecompositionReachesHigherClosure",
    ): (
        "DirectRetainedLockDecompositionReachesOutcomeOrHigherLeader",
    ),
    (
        "SumeragiV2DecisionWitnessPreservationProofs",
        "DecisionExactRetentionFramePreservesSource",
    ): (
        "DecisionExactSourceOwner",
        "HistoricalRecoveryTarget",
        "DecisionExactRetentionFrame",
    ),
    (
        "SumeragiV2ProgressWitnessFinalClosureProofs",
        "FinalMonotoneCarrierFrameEstablishesDecisionExactFrame",
    ): (
        "FinalWitnessMonotoneCarrierFrame",
        "DecisionExactRetentionFrame",
    ),
    (
        "SumeragiV2ProgressWitnessFinalClosureProofs",
        "FinalMonotoneCarrierFramePreservesClosure",
    ): (
        "FinalMonotoneCarrierFrameEstablishesDecisionExactFrame",
        "DecisionExactRetentionFramePreservesSource",
    ),
    (
        "SumeragiV2ProgressWitnessFinalClosureProofs",
        "OpenHistoricalRecoveryPreservesDecisionExactSource",
    ): (
        "StrongDecisionRecordsAreCommit",
        "~NodeHasDecision(node)",
        "asyncHistoricalRecoveryTargets \\cup {node}",
        "DecisionExactSourceOwner",
        "HistoricalRecoveryTarget",
        "AsyncSchedulerExceptHistoricalRecoveryTargets",
    ),
    (
        "SumeragiV2ProgressWitnessFinalClosureProofs",
        "OpenHistoricalRecoveryPreservesFinalProgressWitnessClosure",
    ): (
        "OpenHistoricalRecoveryPreservesDecisionExactSource",
        "OpenHistoricalRecoveryEstablishesHistoricalLineageFrame",
        "HistoricalLockedBodyLineageFramePreservesSourceRetention",
        "HistoricalLineageFramePreservesResponsiveReplayCarrier",
    ),
    (
        "SumeragiV2ApplicationCompletionProofs",
        "ExactDecisionSourceProjectsPostGstServiceStage",
    ): ("DecisionExactSourceOwner",),
    (
        "SumeragiV2ChainReceiptAgreementProofs",
        "IndexedOneHeightDecisionReceiptsAgree",
    ): ("IndexedAsync!DecisionAgreement",),
    (
        "SumeragiV2ChainReceiptAgreementProofs",
        "CompositionAndSourceOwnershipImplyExactReceiptAgreement",
    ): (
        "IndexedApplicationEvidenceIsDecisionEvidence",
        "JoinedContextsAtEqualHeightAreIdentical",
        "IndexedOneHeightDecisionReceiptsAgree",
    ),
    (
        "SumeragiV2ChainReceiptAgreementProofs",
        "IndexedChainSpecEstablishesExactPerSlotReceiptAgreement",
    ): (
        "IndexedChainSpecEstablishesCompositionInvariant",
        "IndexedChainSpecEstablishesDecisionReceiptSourceOwnership",
        "CompositionAndSourceOwnershipImplyExactReceiptAgreement",
    ),
    (
        "SumeragiV2TerminalIngressLifecycleProofs",
        "TerminalIngressProcessLifetimeAbsorbencyObligation",
    ): (
        "TerminalIngressLifecycleSpecAlwaysAbsorbencyInvariant",
        "TerminalIngressLifecycleSpecAlwaysTerminalModeAbsorbingStep",
        "TerminalIngressLifecycleSpecAlwaysTerminalRetiredAbsorbingStep",
        "TerminalIngressLifecycleSpecAlwaysEveryServiceOwnerExitRetiresStep",
        "TerminalIngressLifecycleSpecAlwaysNoPostOwnerAdmissionStep",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionStageOwnershipResidualObligation",
    ): (
        "IndexedChainSpecAlwaysDecisionWitnessSupport",
        "IndexedChainSpecKeepsResponsiveRecoveryDormant",
        "IndexedChainSpecEstablishesCompositionInvariant",
        "IndexedHistoricalDecisionStageOwnershipResidualIsEmpty",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionRankProgressResidualObligation",
    ): (
        "IndexedHistoricalCertificateRankProgressResidualObligation",
        "IndexedHistoricalDecisionTargetOwnerRankProgressObligation",
        "IndexedHistoricalStrictHeightServiceCompositionClosesDecisionRank",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalServiceKernelsDischargeAuthorityReadyProgress",
    ): (
        "IndexedHistoricalCertificateRankProgressResidualProperty",
        "IndexedHistoricalDecisionRankProgressResidualProperty",
        "IndexedHistoricalDecisionStageOwnershipResidualObligation",
        "IndexedChainSpecClosesHistoricalOpenTarget",
        "IndexedHistoricalCertificateRankConvergence",
        "IndexedHistoricalDecisionRankConvergence",
        "IndexedChainSpecClosesHistoricalApplicationReceiptHandoff",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalReleaseResidualsDischargeExactProgress",
    ): (
        "IndexedLiveChainSpecProjectsIndexedChainSpec",
        "IndexedHistoricalCertificateRankProgressResidualObligation",
        "IndexedHistoricalDecisionRankProgressResidualObligation",
        "IndexedHistoricalRecoveryAuthorityAcquisitionResidualObligation",
        "IndexedHistoricalServiceKernelsDischargeEntryCompletion",
        "IndexedExactHistoricalRecoveryProgress",
        "PTL",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalFixedDeadlineDisseminationAndExposureDischargeExactProgress",
    ): (
        "IndexedAdequateLeaderFixedDeadlineDisseminationAnd"
        "ExposureSupplyLocalConvergence",
        "IndexedHistoricalReleaseResidualsDischargeExactProgress",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryResidualKernelsDischargeExactProgress",
    ): (
        "IndexedLiveChainSpecProjectsIndexedChainSpec",
        "IndexedHistoricalRecoveryTemporalResidualKernels",
        "IndexedHistoricalCertificatePhysicalResidualKernels",
        "IndexedHistoricalCertificatePhysicalKernelsCloseRankResidual",
        "IndexedHistoricalDecisionPhysicalKernelsCloseTargetRank",
        "IndexedHistoricalStrictHeightServiceCompositionClosesAuthority",
        "IndexedHistoricalDecisionStageOwnershipResidualObligation",
        "IndexedHistoricalStrictHeightServiceCompositionClosesDecisionRank",
        "IndexedChainSpecClosesHistoricalOpenTarget",
        "IndexedHistoricalCertificateRankConvergence",
        "IndexedHistoricalDecisionRankConvergence",
        "IndexedChainSpecClosesHistoricalApplicationReceiptHandoff",
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalServeExactRetryKeepsAdmissionHighWatermark",
    ): (
        "CoalesceExactServeIngressCapacity",
        "ResumeExactServeCapacity",
        "CoalesceExactServeCapacity",
        "CoalesceSupersededExactServeRequest",
        "RejectConflictingExactServeRequest",
        "AsyncServeLifecycleVars",
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalInitEstablishesIdentityLifecycle",
    ): (
        "HistoricalTemporalInitEstablishesCandidateServiceTombstones",
        "AsyncInitEstablishesStrongTypeInvariant",
        "HistoricalTemporalIdentityLifecycleInvariant",
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "HistoricalTemporalBracketPreservesIdentityLifecycle",
    ): (
        "HistoricalTemporalNextPreservesCandidateServiceTombstones",
        "AsyncBracketNextPreservesStrongTypeInvariant",
        "HistoricalTemporalIdentityLifecycleInvariant",
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "AsyncSpecProvidesHistoricalTemporalCandidateIdentityBridge",
    ): (
        "AsyncSpecAlwaysHistoricalTemporalCandidateServiceTombstones",
        "AsyncCandidateTombstoneSubsetIsBoundedByFrozenOwnerCarrier",
        "AsyncCandidateServicedIdentityCannotReactivate",
        "AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst",
        "AsyncCandidateObsoleteAdmissionIdentityCannotReappearAtGst",
        "AsyncCandidateTerminalIdentityCannotReactivateAtGst",
        "AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst",
        "AsyncCandidateServiceRouteNeutralResponseRetryIsStable",
        "HistoricalTemporalCandidateIdentityBudgetBridgeProperty",
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "AsyncSpecProvidesHistoricalTemporalServeIdentityBridge",
    ): (
        "AsyncServeQueuedIdentityDepartureInstallsTombstone",
        "AsyncServeRetiredIdentityCannotRequeueAtGst",
        "HistoricalTemporalServeExactRetryKeepsAdmissionHighWatermark",
        "HistoricalTemporalServeIdentityBudgetBridgeProperty",
        "AsyncServeLifecyclePartitionInvariant",
        "AsyncServeFamilyHighWatermarkInvariant",
        "AsyncServeReservationOwnershipInvariant",
        "AsyncServeOrdinalInvariant",
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
        "AsyncSpecProvidesHistoricalTemporalCandidateServeIdentityBridge",
    ): (
        "AsyncSpecProvidesHistoricalTemporalCandidateIdentityBridge",
        "AsyncSpecProvidesHistoricalTemporalServeIdentityBridge",
        "HistoricalTemporalCandidateServeIdentityBudgetBridgeProperty",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalTransportVariablesAreExact",
    ): (
        "IndexedAsyncStateShape",
        "IndexedAsyncStateAt",
        "IndexedHistoricalTransport!AsyncAllVars",
        "IndexedHistoricalTransport!AsyncSchedulerVars",
        "IndexedHistoricalTransport!AsyncRecoveryVars",
        "IndexedCore",
        "IndexedScheduler",
        "IndexedRecovery",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedInitEstablishesHistoricalTemporalSupport",
    ): (
        "IndexedInitProjectsEveryHistoricalTransportInit",
        "AsyncInitEstablishesStrongTypeInvariant",
        "AsyncInitEstablishesProgressOwnership",
        "Stage2BusyKernelInitObligation",
        "AsyncInitEstablishesDeferredHandoffOwnership",
        "HistoricalTemporalInitEstablishesIdentityLifecycle",
        "IndexedHistoricalTemporalSupportAt",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedBracketStepPreservesHistoricalTemporalSupport",
    ): (
        "IndexedBracketStepProjectsEveryHistoricalTransportStep",
        "AsyncBracketNextPreservesStrongTypeInvariant",
        "AsyncBracketNextPreservesProgressOwnership",
        "Stage2BusyKernelNextObligation",
        "Stage2AsyncNextPreservesDeferredHandoffOwnership",
        "HistoricalTemporalBracketPreservesIdentityLifecycle",
        "IndexedHistoricalTemporalSupportAt",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecAlwaysHistoricalTemporalSupport",
    ): (
        "IndexedInitEstablishesHistoricalTemporalSupport",
        "IndexedBracketStepPreservesHistoricalTemporalSupport",
        "IndexedChainSpec",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesHistoricalFixedClockIdentityBridge",
    ): (
        "IndexedChainSpecAlwaysHistoricalTemporalSupport",
        "AsyncCandidateTombstoneSubsetIsBoundedByFrozenOwnerCarrier",
        "AsyncCandidateServicedIdentityCannotReactivate",
        "AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst",
        "AsyncCandidateObsoleteAdmissionIdentityCannotReappearAtGst",
        "AsyncCandidateTerminalIdentityCannotReactivateAtGst",
        "AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst",
        "AsyncCandidateServiceRouteNeutralResponseRetryIsStable",
        "AsyncServeQueuedIdentityDepartureInstallsTombstone",
        "AsyncServeRetiredIdentityCannotRequeueAtGst",
        "HistoricalTemporalServeExactRetryKeepsAdmissionHighWatermark",
        "HistoricalTemporalCandidateServeIdentityBudgetBridgeProperty",
        "IndexedHistoricalFixedClockIdentityBridgeProperty",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesHistoricalPostGstTickFairness",
    ): (
        "IndexedPostGstTickFairnessTransfersLocally",
        "IndexedChainSpecAlwaysHasExactHistoricalTransportState",
        "IndexedHistoricalNonPacketActionsMatchIndexedAsync",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesHistoricalRunNodeFairness",
    ): (
        "IndexedPostGstRunNodeFairnessTransfersLocally",
        "IndexedChainSpecAlwaysHasExactHistoricalTransportState",
        "IndexedHistoricalNonPacketActionsMatchIndexedAsync",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesHistoricalOwnerServiceFairness",
    ): (
        "IndexedHistoricalNonPacketOwnerFairnessTransfersLocally",
        "IndexedChainSpecAlwaysHasExactHistoricalTransportState",
        "IndexedHistoricalNonPacketActionsMatchIndexedAsync",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesHistoricalDueNodeModeFairness",
    ): (
        "IndexedChainSpecProvidesHistoricalRunNodeFairness",
        "IndexedChainSpecProvidesHistoricalOwnerServiceFairness",
        "IndexedHistoricalDueNodeModeFairAction",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesHistoricalDueIoModeFairness",
    ): (
        "IndexedChainSpecProvidesHistoricalOwnerServiceFairness",
        "IndexedHistoricalDueIoModeFairAction",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecHistoricalDueNodeModeMakesProgress",
    ): (
        "IndexedChainSpecProvidesHistoricalDueNodeModeFairness",
        "HistoricalDiscoveryDueNodeModeHasEnabledExactFairAction",
        "HistoricalDiscoveryDueNodeModeFairOccurrenceReachesRankGoal",
        "HistoricalDiscoveryDueNodeModeStepPreservesOrProgresses",
        "IndexedHistoricalDueNodeModeProgressGoal",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecHistoricalDueIoModeMakesProgress",
    ): (
        "IndexedChainSpecProvidesHistoricalDueIoModeFairness",
        "HistoricalDiscoveryDueIoModeHasEnabledExactFairAction",
        "HistoricalDiscoveryDueIoModeFairOccurrenceReachesRankGoal",
        "HistoricalDiscoveryDueIoModeStepPreservesOrProgresses",
        "IndexedHistoricalDueIoModeProgressGoal",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecHistoricalDueNodeOwnerReachesRankGoal",
    ): (
        "IndexedChainSpecHistoricalDueNodeModeMakesProgress",
        "HistoricalDiscoveryTimedOwnerModeOrderingIsWellFounded",
        "HistoricalDiscoveryTimedOwnerHasFiniteMode",
        "WellFoundedLeadsTo",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecHistoricalDueIoOwnerReachesRankGoal",
    ): (
        "IndexedChainSpecHistoricalDueIoModeMakesProgress",
        "HistoricalDiscoveryTimedOwnerModeOrderingIsWellFounded",
        "HistoricalDiscoveryTimedOwnerHasFiniteMode",
        "WellFoundedLeadsTo",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecHistoricalTickReachesRankGoal",
    ): (
        "IndexedHistoricalTickBlockedHasEnabledPostGstTick",
        "HistoricalDiscoveryTickStepPreservesOrProgresses",
        "IndexedChainSpecProvidesHistoricalPostGstTickFairness",
        "IndexedBracketStepProjectsEveryHistoricalTransportStep",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalFixedClockNonPacketService",
    ): (
        "IndexedChainSpecHistoricalDueNodeOwnerReachesRankGoal",
        "IndexedChainSpecHistoricalDueIoOwnerReachesRankGoal",
        "IndexedChainSpecHistoricalTickReachesRankGoal",
        "HistoricalDiscoveryFixedClockNonPacketServiceProperty",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalFixedClockLeavesEstablishPrerequisiteSurface",
    ): (
        "IndexedChainSpecProvidesHistoricalFixedClockIdentityBridge",
        "HistoricalTemporalFixedClockLeavesAreExact",
        "IndexedHistoricalFixedClockTemporalLeafProperties",
        "IndexedHistoricalFixedClockPrerequisiteSurface",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalFixedClockPacketResidualClosesPacketLeaves",
    ): (
        "IndexedHistoricalFixedClockPacketCorridorTemporalResidual",
        "HistoricalDiscoveryPacketCorridorResidualClosesPacketLeaves",
        "IndexedHistoricalFixedClockPacketLeafProperties",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalFixedClockExactResidualsEstablishPrerequisiteSurface",
    ): (
        "IndexedChainSpecProvidesHistoricalFixedClockIdentityBridge",
        "IndexedHistoricalFixedClockPacketResidualClosesPacketLeaves",
        "IndexedChainSpecClosesHistoricalFixedClockNonPacketService",
        "HistoricalDiscoveryFixedClockTemporalPrerequisites",
        "IndexedHistoricalFixedClockPrerequisiteSurface",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalCommitTransportKernelsCloseExactLeaf",
    ): (
        "IndexedChainSpecProvidesHistoricalCommitRequestCompleteness",
        "IndexedChainSpecDischargesHistoricalCommitArchiveRouteAvailability",
        "IndexedHistoricalCommitTransportResidualKernelProperties",
        "IndexedChainSpecClosesHistoricalCommitServeResponseKernel",
        "HistoricalCommitTransportKernelsDischargeExactLeaf",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalDecisionTransportKernelsCloseExactLeaf",
    ): (
        "IndexedChainSpecProvidesHistoricalDecisionRequestCompleteness",
        "IndexedChainSpecAlwaysHistoricalTemporalSupport",
        "IndexedHistoricalDecisionTransportResidualKernelProperties",
        "IndexedChainSpecClosesHistoricalDecisionServeResponseKernel",
        "HistoricalDecisionTransportKernelsDischargeExactLeaf",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalFixedClockExactResidualsCloseCertificateDiscoveryRank",
    ): (
        "IndexedChainSpecClosesHistoricalFixedClockNonPacketService",
        "IndexedHistoricalFixedClockExactResidualsEstablishPrerequisiteSurface",
        "IndexedHistoricalFixedClockPrerequisitesCloseDiscoveryClockProgress",
        "IndexedChainSpecClosesHistoricalCertificateDiscoveryRank",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificatePhysicalKernelsCloseRankResidual",
    ): (
        "IndexedChainSpecAndRemainingPacketResidualProvideFixedClockPacketCorridor",
        "IndexedHistoricalFixedClockExactResidualsCloseCertificateDiscoveryRank",
        "IndexedHistoricalCommitTransportKernelsCloseExactLeaf",
        "IndexedHistoricalCommitTransportLeafClosesCertificateRanksTwoThree",
        "IndexedHistoricalCertificateRemainingCorridorClosesRankResidual",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionPhysicalKernelsCloseTargetRank",
    ): (
        "IndexedHistoricalDecisionTransportKernelsCloseExactLeaf",
        "IndexedHistoricalDecisionTransportLeafClosesTargetRankFive",
        "IndexedHistoricalDecisionTargetCertifiedRequestClosesTargetRank",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedDecisionWitnessVariablesAreExact",
    ): (
        "IndexedAsyncStateShape",
        "IndexedAsyncStateAt",
        "IndexedDecisionWitness!AsyncAllVars",
        "IndexedDecisionWitness!AsyncSchedulerVars",
        "IndexedDecisionWitness!AsyncRecoveryVars",
        "IndexedDecisionWitness!AsyncProducerVars",
        "IndexedCore",
        "IndexedScheduler",
        "IndexedRecovery",
        "IndexedProducer",
        "IndexedFixedCorridorDeadlines",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedDecisionServiceWitnessVariablesAreExact",
    ): (
        "IndexedAsyncStateShape",
        "IndexedAsyncStateAt",
        "IndexedDecisionServiceWitness!AsyncAllVars",
        "IndexedDecisionServiceWitness!AsyncSchedulerVars",
        "IndexedDecisionServiceWitness!AsyncRecoveryVars",
        "IndexedDecisionServiceWitness!AsyncProducerVars",
        "IndexedCore",
        "IndexedScheduler",
        "IndexedRecovery",
        "IndexedProducer",
        "IndexedFixedCorridorDeadlines",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedAdequateLeaderWitnessVariablesAreExact",
    ): (
        "IndexedAsyncStateShape",
        "IndexedAsyncStateAt",
        "IndexedAdequateLeaderWitness!AsyncAllVars",
        "IndexedAdequateLeaderWitness!AsyncSchedulerVars",
        "IndexedAdequateLeaderWitness!AsyncRecoveryVars",
        "IndexedAdequateLeaderWitness!AsyncProducerVars",
        "IndexedCore",
        "IndexedScheduler",
        "IndexedRecovery",
        "IndexedProducer",
        "IndexedFixedCorridorDeadlines",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalDecisionTargetCandidateRankResiduals",
    ): (
        "IndexedChainSpecClosesHistoricalDecisionBodyCandidateLeaves",
        "IndexedChainSpecClosesHistoricalProtectedCandidateStarvation",
        "IndexedHistoricalDecisionTargetCandidateRankProgressResidualProperty",
        "IndexedHistoricalDecisionFetchBodyResidualProperty",
        "IndexedHistoricalDecisionFetchCertifiedBodyResidualProperty",
        "IndexedHistoricalDecisionStoreBodyResidualProperty",
        "IndexedHistoricalDecisionValidateBodyResidualProperty",
        "IndexedHistoricalDecisionApplyResidualProperty",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionRankResidualSplitsAtCertifiedRequest",
    ): (
        "IndexedHistoricalDecisionRankProgressResidualProperty",
        "IndexedHistoricalDecisionCandidateRankProgressResidualProperty",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalCertificateCandidateTail",
    ): (
        "IndexedChainSpecClosesHistoricalDecisionCandidateProgressLeaves",
        "HistoricalCommitDeliveryProgressLeaf",
        "HistoricalBeginDecisionProgressLeaf",
        "HistoricalPersistDecisionProgressLeaf",
        "HistoricalProtectedCandidateStarvationProperty",
        "IndexedHistoricalCertificateCandidateTailProgressProperty",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateRankOneIsLocalImport",
    ): (
        "IndexedHistoricalCertificateStageAt",
        "IndexedHistoricalCommitCertificateImported",
        "IndexedHistoricalCertificateLocalImportAt",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalCertificateRankOneEntry",
    ): (
        "IndexedChainSpecClosesHistoricalCertificateLocalImportCandidateEntry",
        "IndexedHistoricalCertificateLocalImportCandidateEntryProperty",
        "IndexedHistoricalCertificateLocalImportCandidateEntryClosesRankOne",
        "IndexedHistoricalCertificateRankOneCandidateEntryProperty",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateRemainingCorridorClosesRankResidual",
    ): (
        "IndexedChainSpecClosesHistoricalCertificateCandidateTail",
        "IndexedHistoricalCertificateRankOneCandidateEntryProperty",
        "IndexedHistoricalCertificateCandidateTailProgressProperty",
        "IndexedHistoricalCertificateRankProgressResidualProperty",
        "IndexedHistoricalCertificateRemainingCorridorProperty",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionTargetCertifiedRequestClosesTargetRank",
    ): (
        "IndexedChainSpecClosesHistoricalDecisionTargetCandidateRankResiduals",
        "IndexedHistoricalDecisionTargetRankResidualSplitsAtCertifiedRequest",
    ),
    (
        "SumeragiV2AsyncHistoricalRecoveryLivenessProofs",
        "HistoricalRecoveryTargetPersistsUnlessApplication",
    ): (
        "HistoricalRecoveryTarget",
        "NodeHasApplication",
        "ExecuteApply",
        "ApplyDecision",
        "ResetNodeSchedulerForRestart",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalDiscoveryCorridor",
    ): (
        "IndexedHistoricalDiscoveryClockProgressProperty",
        "IndexedHistoricalDiscoveryClockReachesPendingOrOutcome",
        "IndexedChainSpecSchedulesHistoricalDiscovery",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesOwnedHistoricalDiscoveryCorridor",
    ): (
        "IndexedHistoricalDiscoveryClockProgressProperty",
        "IndexedHistoricalTargetPersistsUntilApplication",
        "IndexedChainSpecClosesHistoricalDiscoveryCorridor",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalCertificateDiscoveryRank",
    ): (
        "IndexedHistoricalDiscoveryClockProgressProperty",
        "IndexedChainSpecAlwaysHasHistoricalCommitArchiveRoute",
        "IndexedChainSpecClosesOwnedHistoricalDiscoveryCorridor",
        "IndexedHistoricalDiscoveryOwnedOutcomeDropsCertificateRankFour",
    ),
}

# This service-layer composition does not carry the authority-receipt
# acquisition/service premises.  It must therefore remain independent of the
# conditional producer-layer rank bridge instead of silently consuming that
# bridge as though it were an AsyncLive provider.
FIXED_PROOF_FORBIDDEN_PROOF_TOKENS = {
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AsyncLiveProvidesAdequateLeaderViewExposureRankStep",
    ): (
        "AdequateLeaderAuthorityDeadlineQuantitativeProviderBundle",
        "AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty",
        "AdequateLeaderAuthorityDeadlineActiveReceiptDecisionCarryProperty",
        "AdequateLeaderAuthorityBoundReceiptCarrySuppliesFreshSelfLeaderDecision",
        "AdequateLeaderLocalSemanticKernelProperty",
        "RotatingLeaderProgressProperty",
        "ResponsiveDecisionConvergenceProperty",
        "HistoricalRecoveryTargetDecisionProgressProperty",
        "ApplicationLivenessProperty",
        "OneHeightCompletionLiveness",
        "IndexedHeightLivenessProperty",
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AdequateLeaderViewExposureRankOrderingIsWellFounded",
    ): (
        "AdequateLeaderAuthorityDeadlineQuantitativeProviderBundle",
        "AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty",
        "AdequateLeaderAuthorityDeadlineActiveReceiptDecisionCarryProperty",
        "AdequateLeaderAuthorityBoundReceiptCarrySuppliesFreshSelfLeaderDecision",
        "AdequateLeaderLocalSemanticKernelProperty",
        "RotatingLeaderProgressProperty",
        "ResponsiveDecisionConvergenceProperty",
        "HistoricalRecoveryTargetDecisionProgressProperty",
        "ApplicationLivenessProperty",
        "OneHeightCompletionLiveness",
        "IndexedHeightLivenessProperty",
    ),
    (
        "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
        "AsyncLiveProvidesLocalFreshSelfCorridorExposure",
    ): (
        "AdequateLeaderAuthorityDeadlineQuantitativeProviderBundle",
        "AdequateLeaderAuthorityBoundActiveReceiptDecisionCarryProperty",
        "AdequateLeaderAuthorityDeadlineActiveReceiptDecisionCarryProperty",
        "AdequateLeaderAuthorityBoundReceiptCarrySuppliesFreshSelfLeaderDecision",
        "AdequateLeaderLocalSemanticKernelProperty",
        "RotatingLeaderProgressProperty",
        "ResponsiveDecisionConvergenceProperty",
        "HistoricalRecoveryTargetDecisionProgressProperty",
        "ApplicationLivenessProperty",
        "OneHeightCompletionLiveness",
        "IndexedHeightLivenessProperty",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSemanticCompositionSuppliesTargetConvergence",
    ): ("AdequateLeaderTargetRanksReachIndexedDecision",),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionTargetNeutralNonGoalEpisodeHasRemainingOrdinal",
    ): (
        "ExactDecisionTargetNeutralSelectedOwnerConsumesRankCell",
        "ExactDecisionTargetNeutralLastOrdinalForcesStrictRankGoal",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateRankProgressResidualObligation",
    ): (
        "IndexedHistoricalCertificateRankProgressResidualObligation",
        "IndexedHistoricalDecisionRankProgressResidualObligation",
        "IndexedHistoricalRecoveryAuthorityAcquisitionResidualObligation",
        "IndexedHistoricalReleaseResidualsDischargeExactProgress",
        "IndexedHistoricalRecoveryResidualKernelsDischargeExactProgress",
        "IndexedExactHistoricalRecoveryProgress",
        "ApplicationLivenessProperty",
        "OneHeightCompletionLiveness",
        "IndexedHeightLivenessProperty",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionStageOwnershipResidualObligation",
    ): (
        "IndexedHistoricalDecisionStageOwnershipResidualObligation",
        "IndexedHistoricalDecisionRankProgressResidualObligation",
        "IndexedHistoricalRecoveryAuthorityAcquisitionResidualObligation",
        "IndexedHistoricalReleaseResidualsDischargeExactProgress",
        "IndexedHistoricalRecoveryResidualKernelsDischargeExactProgress",
        "IndexedExactHistoricalRecoveryProgress",
        "ApplicationLivenessProperty",
        "OneHeightCompletionLiveness",
        "IndexedHeightLivenessProperty",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionRankProgressResidualObligation",
    ): (
        "IndexedHistoricalDecisionRankProgressResidualObligation",
        "IndexedHistoricalRecoveryAuthorityAcquisitionResidualObligation",
        "IndexedHistoricalReleaseResidualsDischargeExactProgress",
        "IndexedHistoricalRecoveryResidualKernelsDischargeExactProgress",
        "IndexedExactHistoricalRecoveryProgress",
        "ApplicationLivenessProperty",
        "OneHeightCompletionLiveness",
        "IndexedHeightLivenessProperty",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalRecoveryAuthorityAcquisitionResidualObligation",
    ): (
        "IndexedHistoricalDecisionRankProgressResidualObligation",
        "IndexedHistoricalRecoveryAuthorityAcquisitionResidualObligation",
        "IndexedHistoricalReleaseResidualsDischargeExactProgress",
        "IndexedHistoricalRecoveryResidualKernelsDischargeExactProgress",
        "IndexedExactHistoricalRecoveryProgress",
        "ApplicationLivenessProperty",
        "OneHeightCompletionLiveness",
        "IndexedHeightLivenessProperty",
    ),
}

FIXED_PROOF_FORBIDDEN_MODULE_TOKENS = {
    "SumeragiV2ChainReceiptAgreementProofs": (
        "CanonicalCommitForSlot",
        "decidedAt",
    ),
}

# The exact target-neutral scheduler kernel must remain local to the final
# Async transition and its action-level rank facts.  These contracts prevent a
# textually intact leaf from importing a stronger temporal result, one of its
# own advertised conclusions, or a higher-level liveness theorem.
EXACT_TARGET_NEUTRAL_REVIEWED_EXTENDS = (
    "SumeragiV2ApplicationCompletionProofs",
    "SumeragiV2HeightResetBoundaryClosureProofs",
    "SumeragiV2AsyncHistoricalRecoveryClockOwnerActionProofs",
)

EXACT_TARGET_NEUTRAL_FORBIDDEN_TOKENS = (
    "AsyncLiveSpecAt",
    "HistoricalDiscoveryFixedClockTemporalPrerequisites",
    "HistoricalDiscoveryFixedClockRankDescentProperty",
    "HistoricalDiscoveryTemporalPrerequisitesCloseOneRankStep",
    "HistoricalDiscoveryFixedClockClosureProperty",
    "HistoricalDiscoveryTemporalPrerequisitesCloseFixedClock",
    "HistoricalDiscoveryFixedClockClosureLowersClockBudget",
    "HistoricalDiscoveryClockBudgetClosureProperty",
    "HistoricalDiscoveryClockBudgetClosureReachesReleaseGoal",
    "HistoricalDiscoveryTemporalPrerequisitesCloseClockProgress",
    "ExactDecisionRequestClockOwnerConvergenceProperty",
    "ExactDecisionRequestHeadGateOwnerConvergenceProperty",
    "ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergenceProperty",
    "ExactDecisionOffSchedulerResidualConvergenceProperty",
    "ExactDecisionStageServiceProperty",
    "ApplicationCompletionProgressProperty",
    "ApplicationLivenessProperty",
    "AdequateLeaderTargetDecisionConvergenceProperty",
    "RotatingLeaderProgressProperty",
    "ResponsiveDecisionConvergenceProperty",
    "HistoricalRecoveryTargetDecisionProgressProperty",
    "IndexedExactHistoricalRecoveryProgress",
    "IndexedHeightLivenessProperty",
)

# The target-neutral kernel has a deliberately large local helper inventory.
# Seal the token-normalized operator bodies and theorem statements as two
# canonical sorted aggregates: this keeps every helper fail-closed without
# duplicating thousands of lines of normalized TLA+ text in this component.
# Critical rank/fairness/dependency seams are additionally checked
# structurally by ``check_exact_target_neutral_contract``.
EXACT_TARGET_NEUTRAL_OPERATOR_CONTRACT_COUNT = 106
EXACT_TARGET_NEUTRAL_OPERATOR_CONTRACT_SHA256 = (
    "8efcab64f3ddde5e109d8bc0cc4f6ceb979ecd20b9721b51316e50c8c2871067"
)
EXACT_TARGET_NEUTRAL_THEOREM_CONTRACT_COUNT = 48
EXACT_TARGET_NEUTRAL_THEOREM_CONTRACT_SHA256 = (
    "72743c037968dee0cd1ab65bc94f3d3bb3540836f12001481bd1697c6e24f23f"
)

EXACT_TARGET_NEUTRAL_RETIRED_SYMBOLS = (
    "ExactDecisionTargetNeutralPacketDependencyRank",
    "ExactDecisionTargetNeutralCandidateOwners",
    "ExactDecisionTargetNeutralServeOwners",
    "ExactDecisionTargetNeutralCandidateRanks",
    "ExactDecisionTargetNeutralServeRanks",
    "ExactDecisionTargetNeutralCandidateDebtRank",
    "ExactDecisionTargetNeutralServeDebtRank",
    "ExactDecisionTargetNeutralCandidateOccurrenceRank",
    "ExactDecisionTargetNeutralServeOccurrenceRank",
    "ExactDecisionTargetNeutralSelectedPacketDependencyRank",
    "ExactDecisionTargetNeutralConcreteDependencyRank",
    "ExactDecisionTargetNeutralConcreteFixedClockRank",
    "ExactDecisionTargetNeutralCandidateOrdinalTokens",
    "ExactDecisionTargetNeutralServeOrdinalTokens",
    "ExactDecisionTargetNeutralOrdinalCeilingsCarryUntilStrictRankGoal",
    "ExactDecisionTargetNeutralNonGoalEpisodeHasRemainingOrdinal",
    "ExactDecisionTargetNeutralNonDescentConsumesOrdinal",
    "ExactDecisionTargetNeutralLastOrdinalForcesStrictRankGoal",
    "ExactDecisionTargetNeutralLiveRetainedLeaderWireProducerIdentitySet",
    (
        "ExactDecisionTargetNeutralRetainedLeaderWireProducerIdentities"
        "ForSnapshot"
    ),
    "ExactDecisionTargetNeutralRetainedLeaderWireProducerPrepaid",
    "ExactDecisionTargetNeutralDormantLeaderWireChargeableForSnapshot",
)

EXACT_TARGET_NEUTRAL_REQUIRED_PROOF_TOKENS = {
    "ExactDecisionTargetNeutralExactOccurrenceStructuralStepIsDescentOrFrame": (
        "ExactDecisionTargetNeutralFrozenPastCutOriginsCannotReplenish",
        "ExactDecisionTargetNeutralFrozenPastCutServeCannotReplenish",
        "ExactDecisionTargetNeutralFrozenPastCutCandidateServiceConsumesExactOccurrence",
        "AsyncCausalEpisodeExactCandidateOccurrenceBudget",
        "AsyncCausalEpisodeServeWorkBudget",
        "AsyncCausalEpisodeStructuralRankOrdering",
    ),
    "ExactDecisionTargetNeutralProoflessProducerStepIsDescentOrFrame": (
        "CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight",
        "CandidateProducerContinuationDormantLocalReplayChargeCannotAppearAtGst",
        "ExactDecisionTargetNeutralFrozenActiveLeaderWireCandidatesCannotReplenish",
        "ExactDecisionTargetNeutralActionInertDormantHasZeroProoflessCharge",
        "ExactDecisionTargetNeutralPostCutLeaderWireAdmissionCannotEnterFrozenPrefix",
        "ExactDecisionTargetNeutralDropPolicyRejectedIsFrozenPhysicalPrefixFrame",
        "CandidateProducerContinuationPreCutIngressToRuntimeConsumesBarrierStage",
        "CandidateProducerContinuationExactLocalReplayReplacesFrozenCharge",
        "ExactDecisionTargetNeutralLeaderWireStageBudgetForSnapshot",
        "ExactDecisionTargetNeutralChargeableLeaderWireCandidatesForSnapshot",
    ),
    "ExactDecisionTargetNeutralComposedCausalEpisodeStepIsDescentOrFrame": (
        "ExactDecisionTargetNeutralProoflessProducerStepIsDescentOrFrame",
        "ExactDecisionTargetNeutralExactOccurrenceStructuralStepIsDescentOrFrame",
    ),
    "ExactDecisionTargetNeutralLaterWorkCannotAcquirePredecessor": (
        "ExactDecisionTargetNeutralFixedClockDoesNotAddDuePackets",
        "ExactDecisionTargetNeutralMaterializedEpisodeIdentitiesDoNotResurrect",
        "ExactDecisionTargetNeutralFrozenSnapshotCarriersArePrimeInvariant",
        "AsyncCandidateProducerContinuationLaterOrdinalCannotOwnRunnerTurn",
    ),
    "ExactDecisionTargetNeutralServeOrdinalAdvanceLowersFrozenPacketRank": (
        "HistoricalDiscoveryFixedClockIngressRemovesOneDuePacket",
        "ExactDecisionTargetNeutralConcreteRankForSnapshotInCarrier",
    ),
    "ExactDecisionTargetNeutralProducerEpisodeStepIsDescentOrFrame": (
        "ExactDecisionTargetNeutralFirstDistinctIngressConsumesFrozenRank",
        "ExactDecisionTargetNeutralComposedCausalEpisodeStepIsDescentOrFrame",
        "ExactDecisionTargetNeutralProducerEpisodeOrdering",
        "ExactDecisionTargetNeutralProducerEpisodeCarrier",
    ),
    "ExactDecisionTargetNeutralRetainedEpisodesDoNotReplenish": (
        "ExactDecisionTargetNeutralProducerEpisodeStepIsDescentOrFrame",
        "ExactDecisionTargetNeutralMaterializedEpisodeIdentitiesDoNotResurrect",
        "ExactDecisionTargetNeutralLaterWorkCannotAcquirePredecessor",
    ),
    "ExactDecisionTargetNeutralRetainedEpisodeConsumptionLowersRank": (
        "ExactDecisionTargetNeutralRetainedEpisodesDoNotReplenish",
        "ExactDecisionTargetNeutralProducerEpisodeOrdering",
        "ExactDecisionTargetNeutralProducerEpisodeCarrier",
    ),
    "ExactDecisionTargetNeutralNonGoalEpisodeRankRemainsInCarrier": (
        "ExactDecisionTargetNeutralEpisodeRankIsInCarrier",
        "ExactDecisionTargetNeutralRetainedEpisodesDoNotReplenish",
    ),
    "ExactDecisionTargetNeutralRankCellHasConcreteFairOwner": (
        "ExactDecisionTargetNeutralRetainedEpisodeConsumptionLowersRank",
        "ExactDecisionTargetNeutralConcreteRankForSnapshotInCarrier",
    ),
    "ExactDecisionTargetNeutralRankCellStepIsSafe": (
        "ExactDecisionTargetNeutralRetainedEpisodeConsumptionLowersRank",
        "ExactDecisionTargetNeutralFirstDistinctIngressConsumesFrozenRank",
    ),
    "ExactDecisionTargetNeutralSelectedOwnerConsumesRankCell": (
        "ExactDecisionTargetNeutralRetainedEpisodesDoNotReplenish",
        "ExactDecisionTargetNeutralNonGoalEpisodeRankRemainsInCarrier",
    ),
    "ExactDecisionTargetNeutralProducerEpisodeBottomForcesStrictRankGoal": (
        "ExactDecisionTargetNeutralSelectedOwnerConsumesRankCell",
        "ExactDecisionTargetNeutralProducerEpisodeBottomHasNoLowerRank",
    ),
    "ExactDecisionTargetNeutralFiniteEpisodeClosesRankCell": (
        "ExactDecisionTargetNeutralFairEpisodeStep",
        "ExactDecisionTargetNeutralProducerEpisodeBottomForcesStrictRankGoal",
        "ExactDecisionTargetNeutralProducerEpisodeOrderingIsWellFounded",
        "WellFoundedLeadsTo",
    ),
    "ExactDecisionTargetNeutralFixedClockRankStep": (
        "ExactDecisionTargetNeutralFiniteEpisodeClosesRankCell",
        "ExactDecisionTargetNeutralEpisodeRankIsInCarrier",
    ),
    "ExactDecisionTargetNeutralFixedClockConverges": (
        "ExactDecisionTargetNeutralFixedClockRankStep",
        "ExactDecisionTargetNeutralFixedClockOrderingIsWellFounded",
        "ExactDecisionTargetNeutralConcreteRankForSnapshotInCarrier",
    ),
    "ExactDecisionTargetNeutralFairEpisodeStep": (
        "ExactDecisionTargetNeutralRankCellStepIsSafe",
        "ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness",
        "ExactDecisionTargetNeutralSelectedOwnerConsumesRankCell",
    ),
    "ExactDecisionTargetNeutralDueHeadDisablesTick": (
        "AsyncPacketOwnsClockDeadline",
    ),
    "ExactDecisionTargetNeutralFairOwnerUsesAsyncFairness": (
        "LocalCandidateProducerContinuationResolutionUsesReviewedFairAction",
        "ConditionalTransportProducerContinuationServiceUsesReviewedFairAction",
        "VolatileBodyProducerContinuationServiceUsesReviewedFairAction",
        "ExactDecisionTargetNeutralFairOwnerSet",
        "ExactDecisionTargetNeutralFairAction",
        "AsyncFairnessAt",
    ),
}

# Temporal composition may be promoted only after its proof dependencies.  The
# ledger order is intentional as well: reviewers should encounter each
# prerequisite before the theorem which consumes it.
PROOF_STATUS_DEPENDENCIES = {
    "effective-lock-body-acquisition-production-refinement": (
        "effective-lock-body-acquisition-model",
    ),
    "async-type-invariant": ("async-runner-scheduler-preservation",),
    "async-progress-ownership-invariant": ("async-type-invariant",),
    "progress-witness-preservation": (
        "async-type-invariant",
        "generation-scoped-vote-delivery",
        "post-decision-timeout-exclusion",
        "decision-recovery-across-restart",
        "effective-lock-body-acquisition-model",
    ),
    "progress-witness-production-refinement": (
        "async-type-invariant",
        "async-progress-ownership-invariant",
        "generation-scoped-vote-delivery",
        "post-decision-timeout-exclusion",
        "decision-recovery-across-restart",
        "async-fair-action-refinement",
        "progress-witness-preservation",
    ),
    "post-gst-deadlock-freedom": (
        "async-type-invariant",
        "async-fair-action-refinement",
        "progress-witness-preservation",
        "protected-service-rank",
    ),
    "protected-service-rank-stage4-ready-causal": (
        "async-type-invariant",
        "async-progress-ownership-invariant",
        "async-fair-action-refinement",
    ),
    "protected-service-rank-serve-fifo": (
        "async-type-invariant",
        "async-fair-action-refinement",
    ),
    "protected-service-rank-stage5-consensus-fifo": (
        "async-type-invariant",
        "async-progress-ownership-invariant",
        "async-fair-action-refinement",
    ),
    "protected-service-rank": (
        "async-type-invariant",
        "async-progress-ownership-invariant",
        "async-fair-action-refinement",
        "protected-service-rank-stage4-ready-causal",
        "protected-service-rank-serve-fifo",
        "protected-service-rank-stage5-consensus-fifo",
    ),
    "post-gst-starvation-freedom": (
        "async-type-invariant",
        "async-fair-action-refinement",
        "protected-service-rank",
    ),
    "timeout-view-liveness": (
        "async-fair-action-refinement",
        "progress-witness-preservation",
        "post-gst-deadlock-freedom",
        "post-gst-starvation-freedom",
    ),
    "rotating-leader-liveness": (
        "effective-lock-body-acquisition-model",
        "effective-lock-body-acquisition-production-refinement",
        "async-fair-action-refinement",
        "progress-witness-preservation",
        "post-gst-starvation-freedom",
        "timeout-view-liveness",
    ),
    "locked-body-reproposal": (
        "effective-lock-body-acquisition-model",
        "effective-lock-body-acquisition-production-refinement",
        "async-fair-action-refinement",
        "progress-witness-preservation",
        "post-gst-starvation-freedom",
        "timeout-view-liveness",
        "rotating-leader-liveness",
    ),
    "application-liveness": (
        "async-fair-action-refinement",
        "progress-witness-preservation",
        "post-gst-starvation-freedom",
    ),
    "successor-activation-exact-recovery-production-refinement": (
        "epoch-boundary",
        "decision-recovery-across-restart",
        "successor-activation-starvation-freedom",
    ),
    "genesis-height-successor-handoff": (
        "rotating-leader-liveness",
        "application-liveness",
        "successor-activation-starvation-freedom",
    ),
    "height-liveness": (
        "agreement",
        "no-conflicting-commit-qcs",
        "rotating-leader-liveness",
        "application-liveness",
        "successor-activation-starvation-freedom",
        "successor-activation-exact-recovery-production-refinement",
        "genesis-height-successor-handoff",
    ),
}

# Multi-height safety belongs to the receipt-driven, per-node chain model.
# Binding these obligations to the one-height Core theorem module would prove
# only a fixed context and could silently reintroduce the retired global apply
# barrier through the old reconfiguration wrapper.
CHAIN_SAFETY_OBLIGATIONS = {
    "chain-prefix": ("ChainPrefixObligation", "ChainPrefixProperty"),
    "epoch-boundary": ("EpochBoundaryObligation", "EpochBoundaryProperty"),
}
