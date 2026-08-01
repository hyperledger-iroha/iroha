# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

# Merge and pending-control shared-config projection fields current in format version 6.
#
# Each entry is:
# (projected field, actual field, user field, default constant, actual type,
#  user type, user-default suffix, user-to-actual suffix).
MERGE_RUNTIME_CONFIG_FIELDS = (
    (
        "merge_sidecar_inbound_session_capacity",
        "merge_sidecar_inbound_session_capacity",
        "merge_sidecar_inbound_session_capacity",
        "V2_MERGE_SIDECAR_INBOUND_SESSION_CAPACITY",
        "NonZeroUsize",
        "NonZeroUsize",
        "",
        "",
    ),
    (
        "merge_sidecar_inbound_sessions_per_peer",
        "merge_sidecar_inbound_sessions_per_peer",
        "merge_sidecar_inbound_sessions_per_peer",
        "V2_MERGE_SIDECAR_INBOUND_SESSIONS_PER_PEER",
        "NonZeroUsize",
        "NonZeroUsize",
        "",
        "",
    ),
    (
        "merge_sidecar_inbound_assembly_bytes",
        "merge_sidecar_inbound_assembly_bytes",
        "merge_sidecar_inbound_assembly_bytes",
        "V2_MERGE_SIDECAR_INBOUND_ASSEMBLY_BYTES",
        "NonZeroUsize",
        "NonZeroUsize",
        "",
        "",
    ),
    (
        "merge_sidecar_inbound_assembly_bytes_per_peer",
        "merge_sidecar_inbound_assembly_bytes_per_peer",
        "merge_sidecar_inbound_assembly_bytes_per_peer",
        "V2_MERGE_SIDECAR_INBOUND_ASSEMBLY_BYTES_PER_PEER",
        "NonZeroUsize",
        "NonZeroUsize",
        "",
        "",
    ),
    (
        "merge_sidecar_deferred_block_capacity",
        "merge_sidecar_deferred_block_capacity",
        "merge_sidecar_deferred_block_capacity",
        "V2_MERGE_SIDECAR_DEFERRED_BLOCK_CAPACITY",
        "NonZeroUsize",
        "NonZeroUsize",
        "",
        "",
    ),
    (
        "merge_sidecar_future_block_distance",
        "merge_sidecar_future_block_distance",
        "merge_sidecar_future_block_distance",
        "V2_MERGE_SIDECAR_FUTURE_BLOCK_DISTANCE",
        "NonZeroU64",
        "NonZeroU64",
        "",
        "",
    ),
    (
        "merge_sidecar_request_timeout_ms",
        "merge_sidecar_request_timeout",
        "merge_sidecar_request_timeout_ms",
        "V2_MERGE_SIDECAR_REQUEST_TIMEOUT",
        "Duration",
        "DurationMs",
        ".into()",
        ".0",
    ),
    (
        "merge_sidecar_outbound_sessions_per_source",
        "merge_sidecar_outbound_sessions_per_source",
        "merge_sidecar_outbound_sessions_per_source",
        "V2_MERGE_SIDECAR_OUTBOUND_SESSIONS_PER_SOURCE",
        "NonZeroUsize",
        "NonZeroUsize",
        "",
        "",
    ),
    (
        "merge_sidecar_outbound_bytes_per_source",
        "merge_sidecar_outbound_bytes_per_source",
        "merge_sidecar_outbound_bytes_per_source",
        "V2_MERGE_SIDECAR_OUTBOUND_BYTES_PER_SOURCE",
        "NonZeroUsize",
        "NonZeroUsize",
        "",
        "",
    ),
    (
        "merge_sidecar_server_request_gates_per_source",
        "merge_sidecar_server_request_gates_per_source",
        "merge_sidecar_server_request_gates_per_source",
        "V2_MERGE_SIDECAR_SERVER_REQUEST_GATES_PER_SOURCE",
        "NonZeroUsize",
        "NonZeroUsize",
        "",
        "",
    ),
    (
        "pending_certified_merge_entry_capacity",
        "pending_certified_merge_entry_capacity",
        "pending_certified_merge_entry_capacity",
        "V2_PENDING_CERTIFIED_MERGE_ENTRY_CAPACITY",
        "NonZeroUsize",
        "NonZeroUsize",
        "",
        "",
    ),
    (
        "pending_queue_plan_admission_capacity",
        "pending_queue_plan_admission_capacity",
        "pending_queue_plan_admission_capacity",
        "V2_PENDING_QUEUE_PLAN_ADMISSION_CAPACITY",
        "NonZeroUsize",
        "NonZeroUsize",
        "",
        "",
    ),
    (
        "pending_control_sidecar_bytes",
        "pending_control_sidecar_bytes",
        "pending_control_sidecar_bytes",
        "V2_PENDING_CONTROL_SIDECAR_BYTES",
        "NonZeroUsize",
        "NonZeroUsize",
        "",
        "",
    ),
    (
        "merge_signing_guard_record_capacity",
        "merge_signing_guard_record_capacity",
        "merge_signing_guard_record_capacity",
        "V2_MERGE_SIGNING_GUARD_RECORD_CAPACITY",
        "NonZeroUsize",
        "NonZeroUsize",
        "",
        "",
    ),
    (
        "merge_signing_guard_record_bytes",
        "merge_signing_guard_record_bytes",
        "merge_signing_guard_record_bytes",
        "V2_MERGE_SIGNING_GUARD_RECORD_BYTES",
        "NonZeroUsize",
        "NonZeroUsize",
        "",
        "",
    ),
    (
        "merge_signing_guard_total_bytes",
        "merge_signing_guard_total_bytes",
        "merge_signing_guard_total_bytes",
        "V2_MERGE_SIGNING_GUARD_TOTAL_BYTES",
        "NonZeroUsize",
        "NonZeroUsize",
        "",
        "",
    ),
)

ASYNC_LIVENESS_FACADE = "SumeragiV2AsyncLivenessProofs"
# The retained-lock vocabulary moved below the shard chain to remove the
# debt-to-proof-leaf cycle.  Its exact provider and bodies are independently
# pinned by ``_acyclic_liveness_debt_topology_errors`` before this reviewed
# mechanical shard seal is accepted.
ASYNC_LIVENESS_PRE_SPLIT_BODY_SHA256 = (
    "9c0edd76ed2f516a7ac657927ef32ff6c637d088ca3183c00b01e3e1b97001da"
)
ASYNC_LIVENESS_SHARD_MAX_BYTES = 256 * 1024
ASYNC_LIVENESS_SHARD_MAX_LINES = 5_500
ASYNC_LIVENESS_SHARD_MAX_THEOREMS = 150
# These two shards carry the reviewed exact-ingress and producer-continuation
# proof seams.  Keep their narrow, source-current ceilings explicit so one
# additional line or theorem still fails instead of silently raising the
# limit for every release shard.
ASYNC_LIVENESS_SHARD_REVIEWED_MAX_LINES = {
    "SumeragiV2AsyncInstallRunnerProofs": 5_775,
    "SumeragiV2AsyncProgressOwnershipProofs": 5_662,
}
ASYNC_LIVENESS_SHARD_REVIEWED_MAX_THEOREMS = {
    "SumeragiV2AsyncInstallRunnerProofs": 156,
}
ASYNC_LIVENESS_THEOREM_MAX_LINES = 600
ASYNC_LIVENESS_THEOREM_MAX_STEPS = 256
ASYNC_NETWORK_RELEASE_THEOREMS = (
    "AsyncCandidateServiceStageCarrierHasExactlyElevenClasses",
    "AsyncCandidateServiceTrackedKindProjectionIsCovered",
    "AsyncCandidateLifecycleCapacityDerivesFromReviewedOwners",
    "AsyncCandidateServiceRecordCapacityMatchesConfiguredGeometry",
    "AsyncLeaderWireExactRetryRetainsServiceIdentity",
    "AsyncControlServiceSlotCarrierIsRosterClassBounded",
    "ImportedCertificateTailIgnoresOnlyLocalIncarnation",
    "AsyncCandidateServiceLifecycleStageCollisionCoalesces",
    "AsyncCandidateServiceRecordsInjectIntoLifecycleStageOwners",
    "AsyncCandidateProducerContinuationsInjectIntoLifecycleStageOwners",
    "AsyncControlServiceTableCardinalityIsSlotBounded",
    "AsyncCandidateSchedulerCoverageExposesBoundedProducerOrigin",
    "AsyncNextNodeCommandOwnsOldestLifecycleOrdinal",
    "CommandSuccessorsRetainCausalOrigin",
    "AsyncCandidateProducerSemanticHandoffUsesInheritedLifecycle",
    "AsyncCandidateProducerContinuationSelectedLocalReplayHasReservedCapacity",
    "CandidateProducerContinuationResolutionSplitsReviewedSourceClass",
    "ValidQcIntersectsResponsiveSignerSet",
    "RemoteResponsiveQcSignerIsInFrozenArchiveFanout",
    "PacketForItemExactRetryRetainsRouteIdentity",
    "PersistDecisionConvertsIncompatibleResponseBeforeRetryOrdinal",
    "PersistDecisionPreservesPreFenceResponseUntilCheckedDrain",
    "SignProposalAtomicallyHandsProducerToSourceFanout",
    "AsyncNextDeferredCommandOwnsOldestLifecycleWithoutHandoff",
    "AsyncDeferredHandoffRetainsExactSelectedLifecycle",
    "AsyncLeaderWireLifecycleSlotUniverseIsFinite",
    "AsyncLeaderWireLifecycleSlotUniverseIsRosterBounded",
    "DormantLeaderWireOwnsNoIngressSchedulerBarrier",
    "AdmitHiddenPacketReservesFreshSharedPhysicalOrdinal",
    "AdmitHiddenLeaderWireIsAtomicLocalAcceptanceCut",
    "AdmitFreshLeaderWireFreezesCurrentLocalSchedulerOrdinal",
    "AdmitDormantLeaderWireRetainsLifecycleTokenAndFrozenPrefix",
    "AdmitDormantLeaderWirePreservesLogicalPotentialPredecessors",
    "AtomicDormantLeaderWireAdmissionConsumesRealPacketWithFreshCarrier",
    "DormantLeaderWirePhysicalOrdinalExhaustionPublishesNothing",
    "AsyncLeaderWireActionInertDormantHasNoExactAdmissionPacket",
    "AsyncLiveServeIngressDuplicateRetainsSchedulerOrdinal",
    "AsyncUnboundChunkAdmissionDoesNotMintLeaderWireLifecycle",
    "AsyncUnboundChunkExactRetryCoalescesWithoutEpisodeGrowth",
    "AsyncHeldChunkReceiptTombstonesExactProducerEpisode",
    "ExactNegativeRetryConsumesNoServeOrPhysicalOrdinal",
    "NonAdvancingServeFamilyRetryConsumesNoFreshOrdinal",
    "SupersededResponseRetryConvertsBeforeFreshOrdinal",
    "CoalescedDueLeaderWireLifecycleRetryPreservesFrozenOwner",
    "AtomicLeaderWireAdmissionFreezesPrefixBeforeAppend",
    "DirectCommitQcCandidateHasExactImportLineage",
    "CommitCertificateResponseCandidateHasExactImportLineage",
    "CommitImportCausalSuccessorRetainsExactLineage",
    "AsyncCandidateProducerContinuationLaterOrdinalCannotOwnRunnerTurn",
    "AsyncCandidateProducerContinuationPostRetransmitCutCannotOwnRunnerTurn",
    "AsyncPostRetransmitCutCandidateCannotBlockDueRetransmit",
    "AsyncCandidateProducerContinuationRunnerSelectionRespectsIngressCut",
    "AsyncCandidateProducerContinuationRunnerSelectionIsGlobalMinimum",
    "DormantLeaderWireOwnsNoPhysicalIngressPredecessor",
    "AdmitDormantLeaderWireAppendsAfterExistingServeCarrier",
    "InterruptedTipServeCarrierTerminalizesBeforePhysicalRemoval",
    "InterruptedTipLeaderWireCarrierPublishesTypedRetirement",
    "LeaderWireIngressDrainNeverInventsRuntimeOwner",
    "ServeWorkerDecisionSupersessionClosesWithoutStaleResponse",
    "DeferredRetransmitConsumesDriveProgramCounter",
    "AsyncServeIngressTicketExcludesLaterLocalWork",
    "AsyncLeaderWireIngressTicketExcludesLaterLocalWork",
    "AsyncSelectedLeaderWirePhysicalCarrierDefinesIngressScheduler",
    "AsyncCandidateProducerContinuationExactLocalReplayPublishesStoredCarrier",
    "AsyncCandidateProducerContinuationStoredCarrierMakesSelectedRecordReady",
    "AsyncCandidateProducerContinuationReplayDispatchesOnlyExactIdentity",
    "AsyncOlderCandidateLifecyclePreventsDueTimeoutOvertake",
    "AsyncEarlierIngressLifecyclePreventsDueTimeoutOvertake",
    "LocalAdmissionAdvanceSelectsAtomicWork",
    "SerializedLocalPrecedesServeIngressExactFrame",
    "AsyncServeIngressTargetOnlyTurnJumpsToIngress",
    "AsyncServeIngressTargetOnlyCannotOvertakeOlderRuntimeLifecycle",
    "AsyncServeIngressTargetOnlyCannotOvertakeOlderLocalLifecycle",
    "AsyncOlderRuntimeInterleaveRetainsServeTicketAndYieldsLocal",
    "SerializedRuntimeIngressExceptionExecutesSelectedOlderLifecycle",
    "SerializedLocalIngressExceptionExecutesSelectedOlderLifecycle",
    "AsyncLaterServeTicketInterleavesOlderRuntimeEpisode",
    "AsyncLaterServeTicketInterleavesOlderLocalEpisode",
    "SameHeightRestartPreservesServeHighWatermarks",
    "SameHeightRestartRetainsOrConvertsUnreplacedServeTombstone",
    "SameHeightRestartDischargesTerminalReplayWaiter",
    "SameHeightRestartDischargesEveryLocalServeLifecycle",
    "SameHeightRestartTerminalOutcomeIsIndependentlyReconstructed",
    "SameHeightRestartCanonicalDischargeKeysAreLifecycleStable",
    "ExactNegativeServeRetryIsRejectedBeforeFreshOrdinal",
    "StrictHigherViewMayReplaceNegativeServeFamily",
    "SameHeightRestartReopensActiveLeaderWireWithoutTerminalizing",
    "SameHeightRestartReopensVolatileLeaderWireTerminal",
    "SameHeightRestartRetainsDormantLeaderWireWithoutBarrier",
    "SameHeightRestartPreservesRestartStableLeaderWireTerminal",
    "SameHeightRestartReopensDurableCertifiedResponseFamily",
    "PendingServeReceiverClosePublishesTypedTerminalWithoutDebt",
    "MaterializedServeReceiverClosePublishesTypedTerminalWithoutDebt",
    "RuntimeLeaderWireCannotRetireMerelyFromIngressPop",
    "RetireLeaderWireLifecycleRetainsTerminalTombstone",
    "AsyncGateOpenDueResponsivePacketReentersClockDeadline",
    "AsyncRetainedCommitQcRetransmissionCreatesExactPacket",
    "AsyncRetainedCommitQcPacketAdmissionCreatesExactIngressOwner",
    "AsyncRetainedCommitQcIngressCreatesExactDeliverQcOwner",
    "AsyncRetainedCommitQcDeliveryRecordsExactReceipt",
    "AsyncCandidateProducerContinuationResetPreservesExactReservation",
    "AsyncCandidateProducerContinuationUnresetOwnerPreserved",
    "AsyncCandidateProducerContinuationRestartStableTerminalPreserved",
    "AsyncCandidateServiceRecordProducersAreTrackedBoundaryKinds",
    "AsyncCandidateUntrackedInternalContinuationAllocatesNoServiceRecord",
    "AsyncCandidateCausalAdmissionTransfersSameOwner",
    "AsyncCandidateIoCompletionTransfersSameOwner",
    "AsyncCandidateProducerCompletionTransfersSameOwner",
    "AsyncCandidateBusyDeferralTransfersSameOwner",
    "AsyncCandidateDeferredHandoffRetainsSameOwner",
    "AsyncCandidateDiscardIsNotSemanticService",
    "ImportedCertificateTailCannotRetireOnLocalIncarnationChange",
    "AsyncCandidateProducerContinuationRunnerResolutionRequiresReadyEvidence",
    "AsyncCandidateProducerContinuationExactLocalReplayRetainsReservation",
    "AsyncCandidateProducerContinuationRunnerResolutionConsumesExactStage",
    "AsyncRunnerResolutionStrictlyConsumesFiniteProducerPrefix",
    "AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck",
    "AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor",
    "AsyncCandidateProducerSemanticHandoffRetirementRequiresAck",
    "AsyncCandidateProducerContinuationDecisionReclamationClearsNode",
    "AsyncCandidateLifecycleProducerContinuationCoverageUsesInheritedToken",
    "LeaderWireIgnoredOrServicedLastConsumerTerminalizesAtomically",
    "AsyncCandidateProducerContinuationKindPartition",
    "AsyncCandidateProducerContinuationRemovedReplayClassification",
    "LeaderWireDeliveryCandidateInheritsAdmissionSchedulerOrdinal",
    "AsyncDormantLeaderWireReactivationConsumesPhysicalNotLifecycleOrdinal",
    "AsyncTargetNeutralLifecycleOwnerCarrierIsFinite",
    "AsyncTargetNeutralLifecycleEpisodeBudgetIsFiniteAndCoalesced",
    "AsyncTargetNeutralLifecycleDiscoveryStrictlyConsumesBudget",
    "AsyncTargetNeutralLifecycleBudgetOrderingIsWellFounded",
    "AsyncCandidateLifecycleReviewedTokenOwnsOneOrigin",
    "OrdinaryIngressCarrierRetirementCompactionDoesNotIncreaseEvidence",
    "OrdinaryIngressCarrierAdmissionPreservesConfiguredEvidenceBound",
    "AdmitOrdinaryIngressCarrierReservesImmutableActorGlobalOrdinal",
    "ExactOrdinaryIngressDuplicateCoalescesWithoutCarrierAllocation",
    "OrdinaryIngressCarrierIdentityMismatchPublishesNoAdmission",
    "LaterAcceptedOrdinaryCarrierCannotOvertakeFrozenCarrier",
    "BusyDeferredOlderAggregateRebasesToMinimumCompatibleCarrier",
    "BusyDeferredAggregateIdentityMutationCannotRebaseOwner",
    "AsyncControlServiceTransitionConsumesFreshLeaderWireSchedulerOrdinal",
    "AsyncControlServiceTransitionPreservesSemanticHandoffCoverage",
    "AsyncControlServiceTransitionPreservesCandidateProducerContinuationLifecycleCoverage",
    "AsyncCandidateLifecycleReviewedBucketsPartitionRecords",
    "AsyncCandidateLifecycleDormantBucketsSeparateReplayAndService",
    "AsyncCandidateLifecycleActiveRecordsInjectIntoPhysicalOwners",
    "AsyncCandidateLifecycleTransientMarkerRetainsItsReservation",
    "AsyncCandidateLifecycleDormantDurableSourceKeepsReservation",
    "AsyncCandidateLifecycleStrictViewCompactsDormantEpisodeRoot",
    "AsyncCandidateLifecycleReviewedBucketsImplyPerNodeCapacity",
    "AsyncCandidateLifecycleSlotInjectionBoundsGlobalOwners",
    "AsyncCandidateLifecyclePhysicalTokensCoverScheduledOriginsAfter",
    "AsyncCandidateLifecycleDurableTokensCoverReplayOriginsAfter",
    "AsyncCandidateLifecycleDurableOwnerCarrierIsBounded",
    "AsyncCandidateLifecycleServiceOwnerCarrierIsSlotBounded",
    "AsyncCandidateLifecyclePhysicalAndDurableOwnersFitActiveSlots",
    "AsyncCandidateLifecycleCompactedStateHasSemanticOwnerCoverage",
    "AsyncCandidateLifecycleCompactedStateHasActiveOwnerCoverage",
    "AsyncCandidateLifecycleSemanticCoverageGivesOwnerInjection",
    "AsyncCandidateLifecycleActiveCoverageGivesOwnerInjection",
    "AsyncCandidateLifecycleReviewedSemanticOwnersFitOrdinaryCapacity",
    "AsyncCandidateLifecycleReviewedOwnerInjectionProvidesReservations",
    "AsyncCandidateLifecycleCompactedStateProvidesFreshReservations",
    "AsyncCandidateLifecycleCapacityCannotBlockOwnedContinuation",
    "AsyncCandidateLifecycleCarrierInjectionProvidesFreshReservations",
    "AsyncCandidateLifecycleDistinctNewRootsReceiveDistinctOwnership",
    "AsyncCandidateLifecycleHighWatermarkAdvancesByFullFreshSet",
    "AsyncOrdinaryIngressSharedHighWatermarkAdvancesAtAcceptance",
    "AsyncServeIngressSharedHighWatermarkAdvancesByFreshTickets",
    "AsyncLeaderWireIngressHighWatermarkAdvancesByFreshAdmissions",
    "AsyncLeaderWireSharedHighWatermarkAdvancesByFreshAdmissions",
    "AsyncLeaderWireAdmissionPrecedesSameStepCandidateAllocation",
    "AsyncFreshLeaderWireAdmissionProjectionFollowsRetainedOwners",
    "AsyncServeIngressReservationPrecedesSameStepCandidateAllocation",
    "AsyncCandidateLifecycleFullOrdinaryTableRejectsBeforeSourcePop",
    "AsyncControlServiceTransitionRequiresAtomicLifecycleReservation",
    "AsyncServeIngressAdmissionConsumesSharedSchedulerOrdinal",
    "AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal",
    "AsyncFreshServeIngressSchedulerOrdinalInjectsAgainstPriorOwners",
    "AsyncSharedSchedulerHighWatermarkIsMonotone",
    "AsyncSameHeightRestartRetainsSharedSchedulerHighWatermark",
    "AsyncIgnoredIngressEpisodeCannotConsumeLifecycleCapacity",
    "AsyncNextPreservesCandidateProducerContinuationScheduledExclusion",
    "AsyncCandidateServiceIdentityIgnoresConsumerIncarnation",
    "AsyncCandidateServiceIdentityIgnoresSchedulerClass",
    "AsyncCandidateLifecycleAndServiceIdentityIgnoreSchedulerClass",
    "AsyncCandidateServiceRouteNeutralResponseRetryIsStable",
    "AsyncCandidateServiceTombstoneCoalescesFreshCandidate",
    "AsyncCandidateInternalBodyAvailableStageRetirementCoalescesFreshCandidate",
    "AsyncCandidateTransientMarkerCoalescesFreshCandidate",
    "AsyncCandidateTerminalTombstoneCoalescesFreshCandidate",
    "AsyncCandidateServiceTombstoneRejectsTransportReadmission",
    "AsyncCandidateSuccessfulServiceInstallsTransientMarker",
    "AsyncCandidateSuccessfulServiceInstallsTombstone",
    "AsyncCandidateSuccessfulServiceAllocatesExactOrdinal",
    "AsyncCandidateTransientMarkerPersistsWithinGeneration",
    "AsyncCandidateServicedMarkerPersistsWithoutExit",
    "AsyncCandidateTerminalTombstonePersistsWithoutExit",
    "AsyncCandidateSameHeightRestartPreservesServicedIdentity",
    "AsyncCandidateSameHeightRestartPreservesTombstone",
    "AsyncCandidateTransientMarkerDoesNotSuppressRestartReplay",
    "AsyncRestartScopedCandidateIsNeverReplayTombstoned",
    "AsyncCandidateResponsiveRestartPermitsNonterminalReconstruction",
    "AsyncCandidateStrictViewAdvanceReclaimsOlderTombstones",
    "AsyncCandidateDecisionReclaimsNodeTombstones",
    "AsyncCandidateTombstoneSubsetIsBoundedByFrozenOwnerCarrier",
    "AsyncCandidateTombstonesAreBoundedByFrozenOwnerCarrier",
    "AsyncControlServiceExactRetryCoalesces",
    "AsyncControlServiceSameOrLowerViewCannotReplace",
    "AsyncControlServiceLivePredecessorBlocksStrictlyNewerAdmission",
    "AsyncControlServiceConsumedPredecessorAllowsStrictlyNewerReplacement",
    "AsyncControlServiceLivePredecessorClosesFreshAdmissionGate",
    "AsyncControlServiceBlockedNewerPacketCannotPassIngress",
    "AsyncControlServiceReplacementIsStrictlyNewer",
    "AsyncControlServiceConsumedBitIsMonotoneWithoutReplacement",
    "AsyncControlServiceConsumedOccurrenceIsRetired",
    "AsyncControlServiceConsumedIdentityCannotReactivate",
    "AsyncControlServiceServicedIdentityCannotResurrect",
    "AsyncControlServiceTombstoneCannotReactivate",
    "AsyncControlServiceSameHeightRecoveryRetiresVolatileOwners",
    "CertifiedResponseClaimAdmissionAllocatesExactOrdinal",
    "CertifiedResponseClaimAdmissionMatchesPostStateLifecycleCarrier",
    "CertifiedResponseClaimAdmissionFreezesCompletePredecessorSources",
    "CertifiedResponseExactRetryKeepsOneClaimOrdinal",
    "CertifiedResponseLiveClaimCannotBeReplacedAtGst",
    "CertifiedResponseCompetingResponderCannotDoubleChargeFamily",
    "CertifiedResponseConsumedFamilyCannotRetainClaim",
    "CertifiedResponseSameHeightRecoveryReopensDurableFamily",
    "PostGstLeaderWireLifecycleRestartIsDisabled",
    "PostGstStepCannotCreateDormantLeaderWirePotential",
    "LeaderWireIngressAdmissionRefinesLifecycleTransition",
    "LeaderWireIngressDrainRefinesLifecycleTransition",
    "LeaderWireLastConsumerRefinesLifecycleTransition",
    "LeaderWireTerminalRetirementRefinesLifecycleTransition",
    "LeaderWireRestartReopenRefinesLifecycleTransition",
    "AsyncIngressPhysicalHighWatermarkIsMonotone",
    "AsyncServeAdmissionHighWatermarkIsMonotone",
    "AsyncNextPreservesLeaderWireContinuationSharedOrdinalNoCollision",
    "AsyncFixedCorridorDeadlineActionErasureIsExact",
    "AsyncOriginalStepHasFixedCorridorDeadlineReceiptExtension",
    "AsyncServiceActivationActionsRefineAsyncNext",
    "AsyncTimeoutLifecycleDueTransitionMintsBeforeLaterAdmissions",
    "AsyncTimeoutLifecycleOrdinalPersistsUntilEndpoint",
    "AsyncTimeoutLifecycleOrdinalClearsOnlyAtEndpoint",
    "AsyncTimeoutLifecycleNewOwnershipUsesRecordedOrFreshOrdinal",
    "AsyncRetransmitFreshLiveEpisodeFreezesIngressPhysicalCut",
    "AsyncRetransmitLiveEpisodeRetainsIngressPhysicalCut",
    "AsyncRetransmitLifecycleFreezeBoundaryMintsAfterPriorAdmissions",
    "AsyncRetransmitLifecycleOwnerAndPhysicalCutPersistUntilEndpoint",
    "AsyncRetransmitLifecycleOwnerAndPhysicalCutClearAtEndpoint",
    "AsyncNextNeverSchedulesAnUnownedCandidateLifecycle",
    "AsyncServeQueuedIdentityDepartureInstallsTombstone",
    "AsyncServeRetiredIdentityCannotRequeueAtGst",
    "AsyncServeTombstonedIdentityCannotRequeueAtGst",
    "AsyncCandidateServicesThisStepIsSingleton",
    "AsyncCandidateTerminalRetirementsThisStepIsSingleton",
    "AsyncCandidateDiscardInstallsTerminalTombstone",
    "AsyncCandidateTerminalDiscardAllocatesExactOrdinal",
    "AsyncCandidateDiscardRetiresLogicalLifecycle",
    "AsyncCandidateInternalBodyAvailableStageRetirementIsMonotoneAtGst",
    "AsyncCandidateInternalBodyAvailableServiceIdentityCannotReactivateAtGst",
    "AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst",
    "AsyncCandidateObsoleteAdmissionIdentityCannotReappearAtGst",
    "AsyncCandidateTerminalIdentityCannotReactivateAtGst",
    "AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst",
    "AsyncCandidateSameGenerationServicedIdentityCannotReactivateAtGst",
    "AsyncCandidateSameGenerationSuccessfulServiceIdentityPersistsUntilStrictExit",
    "AsyncCandidateServicedIdentityCannotReactivate",
    "AsyncActiveControlServiceAdmissionPassesSlotGuard",
    "AsyncRetiredControlServiceAdmissionDropsWithoutCandidate",
    "PostGstExactDormantLeaderWireAdmissionUsesFairAtomicAction",
    "AsyncControlServiceRolloverInstanceStartsEmpty",
    "AsyncLeaderWireLifecycleRolloverInstanceStartsEmpty",
    "AsyncCertifiedResponseClaimRolloverInstanceStartsEmpty",
    "AsyncCandidateServiceRolloverInstanceStartsEmpty",
    "AsyncInitEstablishesLeaderWireContinuationSharedOrdinalNoCollision",
    "AsyncCandidateLifecycleRolloverStartsWithRootOwners",
    "AsyncInitEstablishesCandidateProducerContinuationLocalReplayCapacity",
    "AsyncResponsiveRestartPreservesCandidateProducerContinuationLocalReplayCapacity",
    "AsyncCandidateDeparturePreservesProducerContinuationLocalReplayCapacity",
    "AsyncNextPreservesCandidateProducerContinuationLocalReplayCapacity",
    "AsyncBracketNextPreservesCandidateProducerContinuationLocalReplayCapacity",
    "AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity",
    "AsyncLiveSpecUsesRepresentativePeerCount",
    "AsyncFiniteLiveSpecUsesRepresentativePeerCount",
    "AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain",
    "AsyncServeIngressSchedulerOrdinalIsTyped",
    "AsyncServeIngressSharedSchedulerInitIsEmptyAndInjected",
    "AsyncRetransmitProgramCounterIsBounded",
    "CertifiedResponseClaimNewTimeoutSourceIsExcludedOrAboveFrozenCeiling",
    "CertifiedResponseClaimsShareOutstandingRequestCharge",
    "CertifiedResponseFamilyLocalClaimsRemainPhysicallySerialized",
    "DormantLeaderWireReactivationPublishesOneFreshPhysicalCarrier",
    "AsyncLeaderWirePotentialPredecessorUniverseIsFinite",
    "AsyncProoflessChunkEpisodeBudgetIsFiniteAndCoalesced",
)
ASYNC_LIVENESS_SHARDS = (
    ("SumeragiV2AsyncRankAndInitProofs", "ModelResponsiveValidators"),
    (
        "SumeragiV2AsyncRankAndInitContinuationProofs",
        "AsyncInitEstablishesSchedulerType",
    ),
    (
        "SumeragiV2AsyncSchedulerPrimitiveTypeProofs",
        "HistoricalRecoveryOnlyChangePreservesSchedulerType",
    ),
    ("SumeragiV2AsyncIngressRunnerTypeProofs", None),
    ("SumeragiV2AsyncRuntimeAdmissionTypeProofs", None),
    ("SumeragiV2AsyncRuntimeAdmissionTypeContinuationProofs", None),
    ("SumeragiV2AsyncInstallRunnerProofs", None),
    ("SumeragiV2AsyncInstallRunnerContinuationProofs", None),
    ("SumeragiV2AsyncTimeoutKernelProofs", None),
    ("SumeragiV2AsyncRecoveryVoteEpochProofs", None),
    ("SumeragiV2AsyncRecoveryVoteEpochContinuationProofs", None),
    ("SumeragiV2AsyncFairServiceProofs", None),
    ("SumeragiV2AsyncProgressOwnershipProofs", None),
    ("SumeragiV2AsyncProtectedSlotProofs", None),
    ("SumeragiV2AsyncSchedulerCompositionProofs", None),
    ("SumeragiV2AsyncRecoveryProgressWitnessProofs", None),
    ("SumeragiV2AsyncTemporalRankProofs", None),
    ("SumeragiV2AsyncStage4RefinementProofs", None),
    ("SumeragiV2AsyncStage3Proofs", None),
    ("SumeragiV2AsyncStage6Proofs", None),
    ("SumeragiV2AsyncStage2Proofs", None),
    ("SumeragiV2AsyncDeadlockProofs", None),
    ("SumeragiV2AsyncTimeoutOwnershipProofs", None),
    ("SumeragiV2AsyncOutstandingLivenessDebt", None),
    (
        "SumeragiV2AsyncDecisionApplicationProofs",
        "ApplicationLivenessObligation",
    ),
)
# The finite-runner closure is an acyclic side cone over Stage2 and the causal
# work budget.  Deadlock imports that proved closure plus the producer-
# continuation coverage needed by its concrete readiness theorem rather than
# the immediately preceding chain shard; every other non-root shard imports
# its chain predecessor.
ASYNC_LIVENESS_EXTENDS_OVERRIDES = {
    "SumeragiV2AsyncDeadlockProofs": (
        "SumeragiV2AsyncFiniteRunnerEpisodeProofs",
        "SumeragiV2AsyncCandidateProducerContinuationProofs",
    ),
}
ASYNC_LIVENESS_DEBT_SHARD = "SumeragiV2AsyncOutstandingLivenessDebt"
ASYNC_LIVENESS_PROOF_SHARDS = tuple(
    module for module, _ in ASYNC_LIVENESS_SHARDS if module != ASYNC_LIVENESS_DEBT_SHARD
)
ASYNC_LIVENESS_DEBT_THEOREMS = (
)
ASYNC_TEMPORAL_CLOSURE_PROOF_MODULES = (
    "SumeragiV2AsyncRankClosureProofs",
    "SumeragiV2ProgressWitnessPreservationProofs",
    "SumeragiV2DecisionWitnessPreservationProofs",
    "SumeragiV2HistoricalLockedBodyWitnessPreservationProofs",
    "SumeragiV2ProgressWitnessFinalClosureProofs",
    "SumeragiV2HeightProductivityFrontierProofs",
    "SumeragiV2HeightResetBoundaryClosureProofs",
    "SumeragiV2LockedBodyReproposalProgressProofs",
    "SumeragiV2TimeoutViewProgressProofs",
    "SumeragiV2RotatingLeaderProgressProofs",
    "SumeragiV2AdequateLeaderServiceClosureProofs",
    "SumeragiV2ApplicationCompletionProofs",
    "SumeragiV2ExactDecisionStageServiceClosureProofs",
    "SumeragiV2AsyncTemporalClosureProofs",
    "SumeragiV2AsyncHistoricalRecoveryServiceClosureProofs",
    "SumeragiV2AsyncHistoricalRecoveryClockActionProofs",
    "SumeragiV2AsyncHistoricalRecoveryClockOwnerActionProofs",
    "SumeragiV2AsyncHistoricalRecoveryClockRankActionProofs",
    "SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs",
    "SumeragiV2AsyncHistoricalRecoveryTransportClosureProofs",
    "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
    "SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs",
    "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
)

# These theorem-bearing side cones are imported by release modules but are not
# members of the mechanically reconstructed async-liveness shard sequence.
# TLAPM treats theorems from an imported module as available facts, so each
# module must receive its own strict backend run rather than being trusted
# merely because a consumer module imports it.
ASYNC_CAUSAL_EPISODE_PROOF_MODULES = (
    "SumeragiV2AsyncCausalWorkBudgetProofs",
    "SumeragiV2AsyncFiniteRunnerEpisodeProofs",
    "SumeragiV2AsyncCandidateProducerContinuationProofs",
    "SumeragiV2AsyncHistoricalCandidateProducerContinuationProofs",
)
ADEQUATE_LEADER_CONTINUATION_PROOF_MODULES = (
    "SumeragiV2AdequateLeaderRetainedProducerClosureProofs",
    "SumeragiV2AdequateLeaderProducerTransportClosureProofs",
    "SumeragiV2AdequateLeaderSelectedOwnerContinuationProofs",
    "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
    "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
    "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
)
ADEQUATE_LEADER_RETAINED_PRODUCER_RELEASE_SOURCE_SHA256 = {
    "SumeragiV2AdequateLeaderRetainedProducerClosureProofs.tla": (
        "74b800bb99f1c5acb2d625c18c1787db0101186249a559caea14a07cb2079399"
    ),
}

# Exact source seal for the finite exact-ingress and adequate-leader ownership
# mutation corpus.  These TLC pairs are regression evidence only: they may
# expose a repaired transition or liveness rank, but never promote a bounded
# model-checking result to deductive proof status.
LIVENESS_OWNERSHIP_MUTATION_FORMAL_ARTIFACTS = (
    "SumeragiV2LocalIngressSchedulerReservationMutation.tla",
    "SumeragiV2RestartTerminalDurabilityMutation.tla",
    "SumeragiV2ExactIngressTicketPriorityMutation.tla",
    "SumeragiV2ExactServeRestartTombstoneMutation.tla",
    "SumeragiV2ExactResponseClaimLifecycleMutation.tla",
    "SumeragiV2ExactServeFrozenPredecessorMutation.tla",
    "SumeragiV2ExactInstalledTcRetentionMutation.tla",
    "SumeragiV2ControlLivePredecessorMutation.tla",
    "SumeragiV2ImportedCertificateTailMutation.tla",
    "SumeragiV2ImportedTcTailMutation.tla",
    "SumeragiV2TimeoutLifecycleStageClassifierMutation.tla",
    "SumeragiV2PersistInstallTimeoutTagMutation.tla",
    "SumeragiV2PersistInstallTimeoutRootRetirementMutation.tla",
    "SumeragiV2AdequateLeaderWireTombstoneMutation.tla",
    "SumeragiV2AdequateLeaderCandidateTombstoneMutation.tla",
    "SumeragiV2ExternalProducerContinuationMutation.tla",
    "SumeragiV2EmptyProducerHandoffMutation.tla",
    "SumeragiV2ProducerOriginReservationMutation.tla",
    "SumeragiV2ProducerContinuationCausalRankMutation.tla",
    "SumeragiV2ProducerReplayCapacityMutation.tla",
    "SumeragiV2RepresentativeLiveScopeMutation.tla",
    "SumeragiV2FixedCorridorPhysicalBudgetMutation.tla",
    "SumeragiV2FixedCorridorActionCreditMutation.tla",
    "SumeragiV2ProposalPipelineBudgetMutation.tla",
    "SumeragiV2AuthorityDeadlineCarryMutation.tla",
    "SumeragiV2AdequateLeaderDeadlineAuthorityMutation.tla",
    "SumeragiV2AdequateLeaderSelectedLifecycleEpisodeMutation.tla",
    "SumeragiV2FixedCorridorReceiptAcquisitionMutation.tla",
    "SumeragiV2OrdinaryIngressCarrierRebaseMutation.tla",
    "SumeragiV2ServeRestartTerminalDischargeMutation.tla",
    "exact_ingress_ticket_priority_fixed.cfg",
    "exact_ingress_ticket_runtime_first_bug.cfg",
    "exact_serve_restart_tombstone_fixed.cfg",
    "exact_serve_restart_tombstone_bug.cfg",
    "exact_response_claim_lifecycle_fixed.cfg",
    "exact_response_claim_duplicate_bug.cfg",
    "exact_response_claim_competing_responder_bug.cfg",
    "exact_response_claim_resurrection_bug.cfg",
    "exact_response_claim_restart_reopen_bug.cfg",
    "exact_serve_frozen_predecessor_fixed.cfg",
    "exact_serve_frozen_predecessor_churn_bug.cfg",
    "exact_installed_tc_retention_fixed.cfg",
    "exact_installed_tc_view_only_bug.cfg",
    "control_live_predecessor_fixed.cfg",
    "control_live_predecessor_bug.cfg",
    "imported_certificate_tail_fixed.cfg",
    "imported_certificate_tail_bug.cfg",
    "imported_tc_tail_fixed.cfg",
    "imported_tc_tail_bug.cfg",
    "timeout_lifecycle_stage_classifier_fixed.cfg",
    "timeout_lifecycle_stage_classifier_bug.cfg",
    "persist_install_timeout_tag_fixed.cfg",
    "persist_install_timeout_tag_bug.cfg",
    "persist_install_timeout_root_retirement_fixed.cfg",
    "persist_install_timeout_root_retirement_bug.cfg",
    "local_ingress_scheduler_reservation_fixed.cfg",
    "local_ingress_scheduler_reservation_mutable_next_bug.cfg",
    "restart_terminal_durability_fixed.cfg",
    "restart_terminal_durability_blanket_terminal_bug.cfg",
    "adequate_leader_wire_tombstone_fixed.cfg",
    "adequate_leader_wire_slot_cardinality_bug.cfg",
    "adequate_leader_wire_same_view_replacement_bug.cfg",
    "adequate_leader_wire_retry_coalescing_bug.cfg",
    "adequate_leader_wire_tombstone_bug.cfg",
    "adequate_leader_wire_restart_resurrection_bug.cfg",
    "adequate_leader_wire_restart_reopen_owner_bug.cfg",
    "adequate_leader_wire_restart_packet_synthesis_bug.cfg",
    "adequate_leader_wire_restart_ordinal_reallocation_bug.cfg",
    "adequate_leader_wire_restart_prefix_recharge_bug.cfg",
    "adequate_leader_wire_dormant_potential_precharge_bug.cfg",
    "adequate_leader_wire_restart_capacity_bypass_bug.cfg",
    "adequate_leader_wire_unconsumed_completion_bug.cfg",
    "adequate_leader_wire_rollover_reset_bug.cfg",
    "adequate_leader_wire_terminal_identity_bug.cfg",
    "adequate_leader_candidate_tombstone_fixed.cfg",
    "adequate_leader_candidate_resurrection_bug.cfg",
    "adequate_leader_candidate_terminal_discard_resurrection_bug.cfg",
    "adequate_leader_candidate_retired_chunk_view_bug.cfg",
    "adequate_leader_candidate_retired_chunk_decision_bug.cfg",
    "adequate_leader_candidate_restart_resurrection_bug.cfg",
    "adequate_leader_candidate_restart_volatile_owner_loss_bug.cfg",
    "adequate_leader_candidate_signed_restart_suppression_bug.cfg",
    "adequate_leader_candidate_aggregate_evidence_identity_explosion_bug.cfg",
    "adequate_leader_candidate_strict_view_reclamation_bug.cfg",
    "adequate_leader_candidate_rollover_reclamation_bug.cfg",
    "external_producer_continuation_fixed.cfg",
    "external_producer_continuation_missing_conditional_bug.cfg",
    "external_producer_continuation_missing_volatile_bug.cfg",
    "external_producer_continuation_synthetic_carrier_bug.cfg",
    "external_producer_continuation_resurrection_bug.cfg",
    "external_producer_continuation_missing_conditional_fairness_bug.cfg",
    "external_producer_continuation_missing_volatile_fairness_bug.cfg",
    "empty_producer_handoff_fixed.cfg",
    "empty_producer_handoff_missing_reservation_bug.cfg",
    "producer_origin_reservation_fixed.cfg",
    "producer_origin_reservation_missing_owner_bug.cfg",
    "producer_origin_reservation_new_ordinal_bug.cfg",
    "producer_origin_reservation_duplicate_retry_bug.cfg",
    "producer_continuation_causal_rank_fixed.cfg",
    "producer_continuation_causal_rank_stage_only_bug.cfg",
    "producer_replay_capacity_fixed.cfg",
    "producer_replay_capacity_blind_invariant_bug.cfg",
    "producer_replay_capacity_non_atomic_replay_bug.cfg",
    "producer_replay_capacity_replenishment_lasso_bug.cfg",
    "representative_live_scope_fixed.cfg",
    "representative_live_scope_missing_premise_bug.cfg",
    "fixed_corridor_physical_budget_fixed.cfg",
    "fixed_corridor_physical_budget_omitted_lane_cursor_bug.cfg",
    "fixed_corridor_action_credit_fixed.cfg",
    "fixed_corridor_action_credit_per_child_recharge_bug.cfg",
    "proposal_pipeline_budget_fixed.cfg",
    "proposal_pipeline_budget_additive_bug.cfg",
    "authority_deadline_carry_fixed.cfg",
    "authority_deadline_carry_expired_receipt_bug.cfg",
    "authority_deadline_carry_kernel_recharge_bug.cfg",
    "adequate_leader_deadline_authority_fixed.cfg",
    "adequate_leader_deadline_authority_omitted_roster_bound_bug.cfg",
    "adequate_leader_selected_lifecycle_episode_fixed.cfg",
    "adequate_leader_selected_lifecycle_episode_semantic_shortcut_bug.cfg",
    "fixed_corridor_receipt_acquisition_fixed.cfg",
    "fixed_corridor_receipt_acquisition_prestate_only_bug.cfg",
    "fixed_corridor_receipt_acquisition_global_retire_bug.cfg",
    "ordinary_ingress_carrier_rebase_fixed.cfg",
    "ordinary_ingress_carrier_rebase_identity_bug.cfg",
    "ordinary_ingress_carrier_rebase_minimum_bug.cfg",
    "serve_restart_terminal_discharge_fixed.cfg",
    "serve_restart_terminal_discharge_body_fail_open_bug.cfg",
    "serve_restart_terminal_discharge_crash_resume_incomplete_union_bug.cfg",
    "serve_restart_terminal_discharge_crash_resume_order_bug.cfg",
    "serve_restart_terminal_discharge_duplicate_outcome_bug.cfg",
    "serve_restart_terminal_discharge_family_coexistence_bug.cfg",
    "serve_restart_terminal_discharge_incomplete_union_bug.cfg",
    "serve_restart_terminal_discharge_live_decision_conversion_bug.cfg",
    "serve_restart_terminal_discharge_mismatched_waiter_bug.cfg",
    "serve_restart_terminal_discharge_negative_terminal_sign_bug.cfg",
    "serve_restart_terminal_discharge_negative_retry_ordinal_bug.cfg",
    "serve_restart_terminal_discharge_negative_waiter_bug.cfg",
    "serve_restart_terminal_discharge_order_bug.cfg",
    "serve_restart_terminal_discharge_orphan_waiter_bug.cfg",
    "serve_restart_terminal_discharge_owner_request_mismatch_bug.cfg",
    "serve_restart_terminal_discharge_persistence_bug.cfg",
    "serve_restart_terminal_discharge_prefence_decision_rewrite_bug.cfg",
    "serve_restart_terminal_discharge_prepared_decision_drain_bug.cfg",
    "serve_restart_terminal_discharge_producer_exposure_bug.cfg",
    "serve_restart_terminal_discharge_raw_context_gate_bug.cfg",
    "serve_restart_terminal_discharge_receiver_close_bug.cfg",
    "serve_restart_terminal_discharge_restart_decision_conversion_bug.cfg",
    "serve_restart_terminal_discharge_resurrection_bug.cfg",
    "serve_restart_terminal_discharge_roster_fanout_bug.cfg",
    "serve_restart_terminal_discharge_signer_authority_bug.cfg",
    "serve_restart_terminal_discharge_terminal_replay_resign_bug.cfg",
)
LIVENESS_OWNERSHIP_MUTATION_RUNNER = (
    "scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh"
)
# The V5 restart-discharge kernel has one reviewed mutant per Boolean control.
# Keep the fixed theorem surface and every one-bit failure target explicit so
# a resealed config cannot silently change mode, property, or deadlock status.
SERVE_RESTART_TERMINAL_DISCHARGE_BITS = (
    "DischargeCompleteUnion",
    "UseCanonicalStartupOrder",
    "ResumeCompleteUnionAfterCrash",
    "ResumeCanonicalOrderAfterCrash",
    "PersistTerminalBeforeAdvance",
    "BlockProducerWhileStartupPending",
    "RequireExactReplayBinding",
    "RejectOrphanTerminalWaiter",
    "RejectNegativeTerminalWaiter",
    "RejectOwnerRequestMismatchWaiter",
    "RejectAdmissionTerminalDuplicate",
    "ConvertRestartResponseOnDecision",
    "ConvertLiveResponseBeforeOrdinal",
    "PreservePreFenceResponseUntilCheckedDrain",
    "RejectNegativeRetryBeforeOrdinal",
    "BlockTerminalResurrection",
    "RequireCanonicalBodyAtStartup",
    "PrunePredecessorFamily",
    "TerminalizeReceiverClose",
    "UseFullFrozenRosterFanout",
    "RequireQcSignerResponseAuthority",
    "EnforceRawContextGate",
    "AvoidTerminalReplayResigning",
    "SignOnlyResponseStartupTerminals",
    "CompletePreparedCarrierDecisionDrain",
)
SERVE_RESTART_TERMINAL_DISCHARGE_FIXED_INVARIANTS = (
    "TypeInvariant",
    "RestartUnionDischargesEveryAdmission",
    "RestartUnionUsesCanonicalOrder",
    "InterruptedDischargePersistsBeforeAdvance",
    "InterruptedRestartResumesWithoutReopening",
    "CrashResumeDischargesEveryRemainingAdmission",
    "CrashResumeUsesCanonicalRemainingOrder",
    "ProducerHiddenUntilStartupDischarged",
    "TerminalReplayIsExactAndOrdinalStable",
    "RestartDecisionSupersessionConvertsResponseAtomically",
    "LiveDecisionSupersessionConvertsResponseBeforeOrdinal",
    "PreFenceCarrierDefersDecisionRewriteUntilCheckedDrain",
    "PreparedCarrierDecisionDrainIsAtomicAndOrdinalStable",
    "CorruptTerminalWaiterFailStops",
    "OwnerRequestMismatchWaiterFailStops",
    "DuplicateAdmissionTerminalFailsStartupAndPreservesState",
    "NegativeRetryConsumesNoFreshOrdinal",
    "TerminalResponseRetryUsesFreshCarrierWithoutLifecycleResurrection",
    "MissingOrCorruptBodyFailStopsAndPreservesState",
    "SuccessorTerminalPrunesPredecessorFamily",
    "ReceiverClosePublishesTypedTerminalWithoutDebt",
    "CertifiedRequestFansOutToFullFrozenRoster",
    "OnlyFrozenQcSignersCanRespond",
    "RawContextGateSeparatesLifecycleAuthority",
    "TerminalReplayAndDecisionConversionDoNotResignOrMintOrdinal",
    "UnsealedRestartResponsesSignExactlyOnce",
)
SERVE_RESTART_TERMINAL_DISCHARGE_MUTATIONS = {
    "serve_restart_terminal_discharge_body_fail_open_bug.cfg": (
        "RequireCanonicalBodyAtStartup",
        "MissingOrCorruptBodyFailStopsAndPreservesState",
    ),
    "serve_restart_terminal_discharge_crash_resume_incomplete_union_bug.cfg": (
        "ResumeCompleteUnionAfterCrash",
        "CrashResumeDischargesEveryRemainingAdmission",
    ),
    "serve_restart_terminal_discharge_crash_resume_order_bug.cfg": (
        "ResumeCanonicalOrderAfterCrash",
        "CrashResumeUsesCanonicalRemainingOrder",
    ),
    "serve_restart_terminal_discharge_duplicate_outcome_bug.cfg": (
        "RejectAdmissionTerminalDuplicate",
        "DuplicateAdmissionTerminalFailsStartupAndPreservesState",
    ),
    "serve_restart_terminal_discharge_family_coexistence_bug.cfg": (
        "PrunePredecessorFamily",
        "SuccessorTerminalPrunesPredecessorFamily",
    ),
    "serve_restart_terminal_discharge_incomplete_union_bug.cfg": (
        "DischargeCompleteUnion",
        "RestartUnionDischargesEveryAdmission",
    ),
    "serve_restart_terminal_discharge_live_decision_conversion_bug.cfg": (
        "ConvertLiveResponseBeforeOrdinal",
        "LiveDecisionSupersessionConvertsResponseBeforeOrdinal",
    ),
    "serve_restart_terminal_discharge_mismatched_waiter_bug.cfg": (
        "RequireExactReplayBinding",
        "CorruptTerminalWaiterFailStops",
    ),
    "serve_restart_terminal_discharge_negative_terminal_sign_bug.cfg": (
        "SignOnlyResponseStartupTerminals",
        "UnsealedRestartResponsesSignExactlyOnce",
    ),
    "serve_restart_terminal_discharge_negative_retry_ordinal_bug.cfg": (
        "RejectNegativeRetryBeforeOrdinal",
        "NegativeRetryConsumesNoFreshOrdinal",
    ),
    "serve_restart_terminal_discharge_negative_waiter_bug.cfg": (
        "RejectNegativeTerminalWaiter",
        "CorruptTerminalWaiterFailStops",
    ),
    "serve_restart_terminal_discharge_order_bug.cfg": (
        "UseCanonicalStartupOrder",
        "RestartUnionUsesCanonicalOrder",
    ),
    "serve_restart_terminal_discharge_orphan_waiter_bug.cfg": (
        "RejectOrphanTerminalWaiter",
        "CorruptTerminalWaiterFailStops",
    ),
    "serve_restart_terminal_discharge_owner_request_mismatch_bug.cfg": (
        "RejectOwnerRequestMismatchWaiter",
        "OwnerRequestMismatchWaiterFailStops",
    ),
    "serve_restart_terminal_discharge_persistence_bug.cfg": (
        "PersistTerminalBeforeAdvance",
        "InterruptedDischargePersistsBeforeAdvance",
    ),
    "serve_restart_terminal_discharge_prefence_decision_rewrite_bug.cfg": (
        "PreservePreFenceResponseUntilCheckedDrain",
        "PreFenceCarrierDefersDecisionRewriteUntilCheckedDrain",
    ),
    "serve_restart_terminal_discharge_prepared_decision_drain_bug.cfg": (
        "CompletePreparedCarrierDecisionDrain",
        "PreparedCarrierDecisionDrainIsAtomicAndOrdinalStable",
    ),
    "serve_restart_terminal_discharge_producer_exposure_bug.cfg": (
        "BlockProducerWhileStartupPending",
        "ProducerHiddenUntilStartupDischarged",
    ),
    "serve_restart_terminal_discharge_raw_context_gate_bug.cfg": (
        "EnforceRawContextGate",
        "RawContextGateSeparatesLifecycleAuthority",
    ),
    "serve_restart_terminal_discharge_receiver_close_bug.cfg": (
        "TerminalizeReceiverClose",
        "ReceiverClosePublishesTypedTerminalWithoutDebt",
    ),
    "serve_restart_terminal_discharge_restart_decision_conversion_bug.cfg": (
        "ConvertRestartResponseOnDecision",
        "RestartDecisionSupersessionConvertsResponseAtomically",
    ),
    "serve_restart_terminal_discharge_resurrection_bug.cfg": (
        "BlockTerminalResurrection",
        "TerminalResponseRetryUsesFreshCarrierWithoutLifecycleResurrection",
    ),
    "serve_restart_terminal_discharge_roster_fanout_bug.cfg": (
        "UseFullFrozenRosterFanout",
        "CertifiedRequestFansOutToFullFrozenRoster",
    ),
    "serve_restart_terminal_discharge_signer_authority_bug.cfg": (
        "RequireQcSignerResponseAuthority",
        "OnlyFrozenQcSignersCanRespond",
    ),
    "serve_restart_terminal_discharge_terminal_replay_resign_bug.cfg": (
        "AvoidTerminalReplayResigning",
        "TerminalReplayAndDecisionConversionDoNotResignOrMintOrdinal",
    ),
}
SERVE_SCHEDULER_ORDINAL_MUTATION_FORMAL_ARTIFACTS = (
    "SumeragiV2ServeSchedulerOrdinalMutation.tla",
    "serve_scheduler_shared_ordinal_fixed.cfg",
    "serve_scheduler_separate_ordinal_bug.cfg",
    "serve_scheduler_older_runtime_fixed.cfg",
    "serve_scheduler_always_target_first_bug.cfg",
    "serve_scheduler_older_local_fixed.cfg",
    "serve_scheduler_local_target_first_bug.cfg",
    "serve_scheduler_continuation_cut_fixed.cfg",
    "serve_scheduler_continuation_overtake_bug.cfg",
    "serve_scheduler_claim_physical_cut_fixed.cfg",
    "serve_scheduler_claim_logical_overtake_bug.cfg",
    "serve_scheduler_claim_ranked_reentry_fixed.cfg",
    "serve_scheduler_claim_raw_descent_bug.cfg",
)
SERVE_SCHEDULER_ORDINAL_MUTATION_RUNNER = (
    "scripts/formal/run_sumeragi_v2_serve_scheduler_ordinal_mutations.sh"
)
SERVE_SCHEDULER_ORDINAL_MUTATION_SHA256 = {
    "SumeragiV2ServeSchedulerOrdinalMutation.tla": (
        "7261a89801226b308edcf25f56e70292ea4585fb728ce1d0432f7fbf96763ced"
    ),
    "serve_scheduler_shared_ordinal_fixed.cfg": (
        "fc89024f227baa0f464ecdacdfd0541ea45badbf864de83f1153ad65f778ce35"
    ),
    "serve_scheduler_separate_ordinal_bug.cfg": (
        "bbd0e518c0e46a9e5b0b67241ee890f38167add7d9b4cf05bfc9052c0f97978d"
    ),
    "serve_scheduler_older_runtime_fixed.cfg": (
        "7c28b165796aa7712860fd108dba5e5b1e299bb20b941f65b9544da218d33412"
    ),
    "serve_scheduler_always_target_first_bug.cfg": (
        "b615cdabdbf49c8b9cdfb17b05e2e8797a3554e38956617404d8778c21bd79c2"
    ),
    "serve_scheduler_older_local_fixed.cfg": (
        "aa3d9ccf7343191f466c02a5ae3d2d6201d0edc70e5c3967517f3a3e3ae74abd"
    ),
    "serve_scheduler_local_target_first_bug.cfg": (
        "4b94b098b88f5dc7cc4ebd7f124e1dc4482774b8f937f01af7ee53e78396d975"
    ),
    "serve_scheduler_continuation_cut_fixed.cfg": (
        "8c1438e15e94ea36dba6c0d7e74665cd642ad69ea93cec72c418515cdd18e6ff"
    ),
    "serve_scheduler_continuation_overtake_bug.cfg": (
        "3a431c32acb86b99982b9725e177a1d1e8f68d23269bda97835ff619fb43f80d"
    ),
    "serve_scheduler_claim_physical_cut_fixed.cfg": (
        "c810fdb21b437594d4874cd0bd3267cf8ebc73ce9a4b892be991e2989ecb5255"
    ),
    "serve_scheduler_claim_logical_overtake_bug.cfg": (
        "fd027c222c6a7a49d0988943bbbc2fe7369f07c0c68e1de1f9b2be9b214befb9"
    ),
    "serve_scheduler_claim_ranked_reentry_fixed.cfg": (
        "4fd95ff86607654b53791658883fd191a6511c88aa74350ca23ab21a59e3268f"
    ),
    "serve_scheduler_claim_raw_descent_bug.cfg": (
        "965b2f2b71534f09b8b2bcbc701729b44549edcd51f5d0d7ce3fe89da6c93584"
    ),
    SERVE_SCHEDULER_ORDINAL_MUTATION_RUNNER: (
        "26dde32bedb3b54cdfa7ead672fa1498b853a936b6620a7715f9ba9ea77ea8e2"
    ),
}
SERVE_SCHEDULER_ORDINAL_RUNNER_SECTION_SHA256 = (
    (
        "preflight and SANY execution",
        "if (($#)); then",
        "run_tlc() {",
        "cd4e26166d5fefa8422eb53db4bbe68a6304f75b73d9b1c32be9bf7a94525d39",
    ),
    (
        "TLC status and shared-result-contract checker",
        "run_tlc() {",
        'shared_fixed_log="$(run_tlc shared-ordinal-fixed "$SHARED_FIXED_CONFIG" 0)"',
        "9c825d3ac384408982f9130f7f7418723e727c6e0f69650d769bbe2a491d0380",
    ),
    (
        "six-repaired then six-mutant execution tail",
        'shared_fixed_log="$(run_tlc shared-ordinal-fixed "$SHARED_FIXED_CONFIG" 0)"',
        None,
        "2e086ef27315e606c56df8247b9fdf3ee1199d19acd73d403a636ce58a31f35e",
    ),
)
SERVE_SCHEDULER_ORDINAL_MUTATION_FORMAL_GLOBS = (
    "SumeragiV2ServeSchedulerOrdinalMutation*.tla",
    "serve_scheduler_*.cfg",
)
SERVE_SCHEDULER_ORDINAL_RELEASE_SOURCE_SHA256 = {
    "SumeragiV2AsyncNetwork.tla": (
        "b44ac5545d981f4bf87aaab328f7fa42b5948bc406a39e51b032a2d80f771ba4"
    ),
}
PRODUCER_CONTINUATION_PHYSICAL_CUT_MUTATION_FORMAL_ARTIFACTS = (
    "SumeragiV2ProducerContinuationPhysicalCutMutation.tla",
    "current_ingress_physical_cut_fixed.cfg",
    "current_ingress_replenishment_churn_bug.cfg",
    "producer_continuation_physical_cut_fixed.cfg",
    "producer_continuation_logical_only_replay_bug.cfg",
    "producer_continuation_timeout_cut_fixed.cfg",
    "producer_continuation_timeout_cut_logical_minimum_bug.cfg",
    "SumeragiV2AdequateLeaderPeriodicPrefixMutation.tla",
    "adequate_leader_periodic_prefix_fixed.cfg",
    "adequate_leader_periodic_hidden_prefix_bug.cfg",
    "adequate_leader_periodic_replenishment_bug.cfg",
)
PRODUCER_CONTINUATION_PHYSICAL_CUT_MUTATION_RUNNER = (
    "scripts/formal/"
    "run_sumeragi_v2_producer_continuation_physical_cut_mutations.sh"
)
PRODUCER_CONTINUATION_PHYSICAL_CUT_MUTATION_SHA256 = {
    "SumeragiV2ProducerContinuationPhysicalCutMutation.tla": (
        "dc59e10845828599650fed19b864f56b6ce63546709e1101284c2abe30760eba"
    ),
    "current_ingress_physical_cut_fixed.cfg": (
        "48d9c8247cfa9b92a74738326f2cfac62b03b08d296b438b038c58606565f960"
    ),
    "current_ingress_replenishment_churn_bug.cfg": (
        "39a99eceacb2949f0db838824f5264183dbb270035a710c07ee30782029e2a4e"
    ),
    "producer_continuation_physical_cut_fixed.cfg": (
        "c9c23bca0dd6812c2e48429845ddb47be1283548e392d46f2a3dab71a66bc19d"
    ),
    "producer_continuation_logical_only_replay_bug.cfg": (
        "1721de0fa881994fa5cfd125310ed9d0a0ab2481719990c0b7f1be6507bffbbe"
    ),
    "producer_continuation_timeout_cut_fixed.cfg": (
        "08619e5e2a8cca04fb1b9d07677a6da5bfdc8a4e53529acc394fdea077a3b3d4"
    ),
    "producer_continuation_timeout_cut_logical_minimum_bug.cfg": (
        "bac68e669638812332bb9ed2eec054e4d8eeef399c1d537e83a07f0d9991342e"
    ),
    "SumeragiV2AdequateLeaderPeriodicPrefixMutation.tla": (
        "099ee25e46fa1a20dff86d046c4140fe8df0b54c6ddaad1c2134365b49582cb9"
    ),
    "adequate_leader_periodic_prefix_fixed.cfg": (
        "c367ff9409884904f28dd93ca742d7098d9e002f8b0f017d057b27af1c54d0b6"
    ),
    "adequate_leader_periodic_hidden_prefix_bug.cfg": (
        "5c9e99f59a0ee4ec06afbe8945462642776ae1708b1ee0cbc3c96923811fee4a"
    ),
    "adequate_leader_periodic_replenishment_bug.cfg": (
        "0185e5ec0fe0de3b927bfdee57db59cc598b3b288d0e84df21658e78d71eac10"
    ),
    PRODUCER_CONTINUATION_PHYSICAL_CUT_MUTATION_RUNNER: (
        "454a44541d5b4c1a5eaaf5b23e34053c111f76eadab75521683713bf773a56bb"
    ),
}
COMMIT_IMPORT_PROVENANCE_MUTATION_FORMAL_ARTIFACTS = (
    "SumeragiV2CommitImportProvenanceMutation.tla",
    "commit_import_provenance_execution_bug.cfg",
    "commit_import_provenance_fixed.cfg",
    "commit_import_successor_fixed.cfg",
    "commit_import_successor_replacement_bug.cfg",
)
COMMIT_IMPORT_PROVENANCE_MUTATION_RUNNER = (
    "scripts/formal/run_sumeragi_v2_commit_import_provenance_mutations.sh"
)
COMMIT_IMPORT_PROVENANCE_MUTATION_SHA256 = {
    "SumeragiV2CommitImportProvenanceMutation.tla": (
        "46cc9b83d9b70048538a3b95d68720c61b0d6619c98ca512968004934d20f72a"
    ),
    "commit_import_provenance_execution_bug.cfg": (
        "b0fb299eb218f0f71c9c9e25644922c257738155466559d77310848e0bf4a964"
    ),
    "commit_import_provenance_fixed.cfg": (
        "5efc0459cf473655f0206d7361ce2dc37c05ac5b622018d0b7c8ac94ea10c772"
    ),
    "commit_import_successor_fixed.cfg": (
        "83661ad58693907ba12a1d6e72696c7b39feb6be0b7348349ba285c2c87881e2"
    ),
    "commit_import_successor_replacement_bug.cfg": (
        "dabf0dfd92215a53e7acc51e974af2188aaa542bb437d7e81da77596a76300bc"
    ),
    COMMIT_IMPORT_PROVENANCE_MUTATION_RUNNER: (
        "4bd77eed89bce194b308769b0267a814628a6596ab298d68bf1d3ca8f198eaaa"
    ),
}
COMMIT_IMPORT_PROVENANCE_MUTATION_FORMAL_GLOBS = (
    "SumeragiV2CommitImportProvenanceMutation*.tla",
    "commit_import_provenance_*.cfg",
    "commit_import_successor_*.cfg",
)
COMMIT_IMPORT_PROVENANCE_RELEASE_SOURCE_SHA256 = {
    "SumeragiV2AsyncNetwork.tla": (
        "ddb06b1b1659b8e9b18a0c59e01293f99ac945df1f06dbd32bc217bc611cee12"
    ),
    "SumeragiV2HistoricalRecoveryTemporalClosureProofs.tla": (
        "7bbc3620ad8bb0dc7bc3e3288e01c254563d54fce7b271bd05e393d938b924c0"
    ),
}
_FORMAL_CI_NEW_MUTATION_RUNNER_INVOCATIONS = (
    "bash scripts/formal/run_sumeragi_v2_indexed_service_activation_mutations.sh",
    "bash scripts/formal/run_sumeragi_v2_adequate_leader_readiness_mutations.sh",
    "bash scripts/formal/run_sumeragi_v2_indexed_height_mutation.sh",
    "bash scripts/formal/run_sumeragi_v2_item_carrier_typing_mutation.sh",
    "bash scripts/formal/run_sumeragi_v2_reply_writer_deadline_mutations.sh",
)
SHARED_TLC_RESULT_CONTRACT = (
    "scripts/formal/sumeragi_v2_tlc_result_contract.sh"
)
SHARED_TLC_RESULT_CONTRACT_CALLERS = (
    "scripts/formal/check_sumeragi_v2_replay_trace.sh",
    "scripts/formal/run_sumeragi_v2_adequate_leader_readiness_mutations.sh",
    "scripts/formal/run_sumeragi_v2_applied_phase_admission_mutations.sh",
    "scripts/formal/run_sumeragi_v2_apply_authority_mutation.sh",
    "scripts/formal/run_sumeragi_v2_candidate_restart_mutation.sh",
    "scripts/formal/run_sumeragi_v2_certificate_ref_recovery_mutation.sh",
    "scripts/formal/run_sumeragi_v2_certified_response_identity_separation_mutation.sh",
    "scripts/formal/run_sumeragi_v2_certified_response_registration_mutation.sh",
    "scripts/formal/run_sumeragi_v2_certified_response_source_lineage_mutation.sh",
    "scripts/formal/run_sumeragi_v2_commit_import_provenance_mutations.sh",
    "scripts/formal/run_sumeragi_v2_decision_recovery_lifecycle_mutation.sh",
    "scripts/formal/run_sumeragi_v2_effect_capacity_ownership_mutation.sh",
    "scripts/formal/run_sumeragi_v2_historical_discovery_occurrence_rank_mutation.sh",
    "scripts/formal/run_sumeragi_v2_indexed_height_mutation.sh",
    "scripts/formal/run_sumeragi_v2_indexed_service_activation_mutations.sh",
    "scripts/formal/run_sumeragi_v2_inflight_first_release.sh",
    "scripts/formal/run_sumeragi_v2_ingress_causal_freshness_mutation.sh",
    "scripts/formal/run_sumeragi_v2_item_carrier_typing_mutation.sh",
    "scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh",
    "scripts/formal/run_sumeragi_v2_multilane_mutations.sh",
    "scripts/formal/run_sumeragi_v2_persist_install_generation_mutation.sh",
    "scripts/formal/run_sumeragi_v2_persist_install_validation_mutation.sh",
    "scripts/formal/run_sumeragi_v2_post_decision_timeout_mutation.sh",
    PRODUCER_CONTINUATION_PHYSICAL_CUT_MUTATION_RUNNER,
    "scripts/formal/run_sumeragi_v2_productive_mutation.sh",
    "scripts/formal/run_sumeragi_v2_progress_mutations.sh",
    "scripts/formal/run_sumeragi_v2_replay_locked_body_carrier_mutation.sh",
    "scripts/formal/run_sumeragi_v2_reply_writer_deadline_mutations.sh",
    "scripts/formal/run_sumeragi_v2_restart_locked_fetch_order_mutation.sh",
    "scripts/formal/run_sumeragi_v2_serve_scheduler_ordinal_mutations.sh",
    "scripts/formal/run_sumeragi_v2_service_rank_mutation.sh",
    "scripts/formal/run_sumeragi_v2_tlc.sh",
    "scripts/formal/run_sumeragi_v2_typed_rollover_handoff_mutations.sh",
)
SHARED_TLC_RESULT_SPECIALIZED_CALLERS = (
    "scripts/formal/check_sumeragi_v2_replay_trace.sh",
    "scripts/formal/run_sumeragi_v2_item_carrier_typing_mutation.sh",
    "scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh",
    "scripts/formal/run_sumeragi_v2_tlc.sh",
)
SHARED_TLC_RESULT_CONTRACT_SHA256 = {
    SHARED_TLC_RESULT_CONTRACT: (
        "f89449d336dc4b6f3a9e7c78e3b05780e0d59ca7de200cce5669ca1fb9715f74"
    ),
    "scripts/formal/check_sumeragi_v2_replay_trace.sh": (
        "ccdae44c2fc9d12dfe7a1239988de036a3181aea7f76c522a83ce4809f300118"
    ),
    "scripts/formal/run_sumeragi_v2_adequate_leader_readiness_mutations.sh": (
        "b81171eb4105b8aa8a66c840b3123b8a0cddc8c7515fec23c1a5c0c6ff7541d4"
    ),
    "scripts/formal/run_sumeragi_v2_applied_phase_admission_mutations.sh": (
        "77368edad02e9ff8a03d1da3c6bb019f0a5d466db6801b30d106fc1b89c5c953"
    ),
    "scripts/formal/run_sumeragi_v2_apply_authority_mutation.sh": (
        "a9bad39fa0caa6761cf7fa5aee259cba9e932234a79e4401ccecc380ca072200"
    ),
    "scripts/formal/run_sumeragi_v2_candidate_restart_mutation.sh": (
        "28e4512bd6f856175e5dc5c601afa41f95e4a511f0e9d9aeac5127a309c1b610"
    ),
    "scripts/formal/run_sumeragi_v2_certificate_ref_recovery_mutation.sh": (
        "da7d91db8c4b7123cda949b50601b2bf23db1f257bb66d1929860346b61bbffc"
    ),
    "scripts/formal/run_sumeragi_v2_certified_response_identity_separation_mutation.sh": (
        "2b8dc5db20be959526ca0a5fe9d57ac0d34afb209cb5d7a9d80aeb6e55740569"
    ),
    "scripts/formal/run_sumeragi_v2_certified_response_registration_mutation.sh": (
        "166fbeb948d2e05015c6deed7c4d45e512bf9f29b8825f5feccb7608c0ef2eae"
    ),
    "scripts/formal/run_sumeragi_v2_certified_response_source_lineage_mutation.sh": (
        "dbaba12f18e9f48a490891625ca6f5556f0b1517b22f8d5fabaa57e7835bb1d7"
    ),
    "scripts/formal/run_sumeragi_v2_commit_import_provenance_mutations.sh": (
        "4bd77eed89bce194b308769b0267a814628a6596ab298d68bf1d3ca8f198eaaa"
    ),
    "scripts/formal/run_sumeragi_v2_decision_recovery_lifecycle_mutation.sh": (
        "252ee943a2392f866d93cd1c62af4e17be63c22dcdfcfb0f73cd6691ac7bd3a4"
    ),
    "scripts/formal/run_sumeragi_v2_effect_capacity_ownership_mutation.sh": (
        "c12af944bccd6587374576ecc0abf6779a468ac0162d59b3c6762ab09aab9017"
    ),
    "scripts/formal/run_sumeragi_v2_historical_discovery_occurrence_rank_mutation.sh": (
        "cb393225ef5b6b793367f05fd66f807b55ab06f31db94784be238c4261a7c161"
    ),
    "scripts/formal/run_sumeragi_v2_indexed_height_mutation.sh": (
        "2ab6aa430893197888a69b4ec56a120556e44e7465893dc5155a3941d236519e"
    ),
    "scripts/formal/run_sumeragi_v2_indexed_service_activation_mutations.sh": (
        "88c03a575317b85ec59660d4a8e7164798d0ce11b59c8e8da5809f4d5b8132af"
    ),
    "scripts/formal/run_sumeragi_v2_inflight_first_release.sh": (
        "25b658d7dd2866f04caf679fca9b3274091ce202f96c9f269d5e5d136ba1c466"
    ),
    "scripts/formal/run_sumeragi_v2_ingress_causal_freshness_mutation.sh": (
        "7aaac40fa9cb9fd603f8809baba4152d8266ae19adad6a967aa04290e2a2af6b"
    ),
    "scripts/formal/run_sumeragi_v2_item_carrier_typing_mutation.sh": (
        "4bf965e65183469fc3434cdf96c486c921aa5f270a192580761ab6a0e7f6d7f8"
    ),
    "scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh": (
        "3f3738a6351e7f7fe78ac3f6a4a4e738f8b6fd2b098d7466674197c783783d25"
    ),
    "scripts/formal/run_sumeragi_v2_multilane_mutations.sh": (
        "267220790e6ac52ef6872319185e2de987e3757af9d1b7323ec2179e4eace49b"
    ),
    "scripts/formal/run_sumeragi_v2_persist_install_generation_mutation.sh": (
        "fc841bb679feaf1ded305f1ab0e39ba388e49876bcbc1e3cb39436b09be74a78"
    ),
    "scripts/formal/run_sumeragi_v2_persist_install_validation_mutation.sh": (
        "73dab6c69caa1ecba6da0aa7f8c0aec30f449aab9c39810c23ed928df0ed2bdb"
    ),
    "scripts/formal/run_sumeragi_v2_post_decision_timeout_mutation.sh": (
        "9e8a9f07e50230929712a84d01abf4a75f134727a254a0fd21b57e19c93a4e8b"
    ),
    PRODUCER_CONTINUATION_PHYSICAL_CUT_MUTATION_RUNNER: (
        "454a44541d5b4c1a5eaaf5b23e34053c111f76eadab75521683713bf773a56bb"
    ),
    "scripts/formal/run_sumeragi_v2_productive_mutation.sh": (
        "5cffb84945d844b38e8d5cc492ef1381519111750592c96a412ddcf170dde072"
    ),
    "scripts/formal/run_sumeragi_v2_progress_mutations.sh": (
        "a3060703cbf35f08e0a60b35e9d2b3efcb4a72a4e6524d117726e365a3c89551"
    ),
    "scripts/formal/run_sumeragi_v2_replay_locked_body_carrier_mutation.sh": (
        "6316f98a9bba2c0bc36155f14f720b60148c1d4d6eb866b7600ce80d0bc8aaea"
    ),
    "scripts/formal/run_sumeragi_v2_reply_writer_deadline_mutations.sh": (
        "b18a14944ca8ece675b70307f3d091112d07b92ca31513612c47cdb152722729"
    ),
    "scripts/formal/run_sumeragi_v2_restart_locked_fetch_order_mutation.sh": (
        "404ddb1393d21d33e83f0c01afb5bacf4e2d761420cbf6905351c043464a0596"
    ),
    "scripts/formal/run_sumeragi_v2_serve_scheduler_ordinal_mutations.sh": (
        "26dde32bedb3b54cdfa7ead672fa1498b853a936b6620a7715f9ba9ea77ea8e2"
    ),
    "scripts/formal/run_sumeragi_v2_service_rank_mutation.sh": (
        "e20a740c92cc9edb951699f8e3247b61d0c92701fcde39fca60baad493bb4796"
    ),
    "scripts/formal/run_sumeragi_v2_tlc.sh": (
        "bceb90990f6ca635bf9d48ad23d140f1a9088954f5bf81952646ec756d517424"
    ),
    "scripts/formal/run_sumeragi_v2_typed_rollover_handoff_mutations.sh": (
        "e1ac1e03c10f2667eb4254b99fa9cd60955417fc39540b7697f8721364313340"
    ),
}
SHARED_TLC_RESULT_BRANCH_PROFILES = {
    "scripts/formal/check_sumeragi_v2_replay_trace.sh": (0, 0, 0, 0, 1),
    "scripts/formal/run_sumeragi_v2_adequate_leader_readiness_mutations.sh": (
        9,
        7,
        0,
        2,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_applied_phase_admission_mutations.sh": (
        1,
        5,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_apply_authority_mutation.sh": (
        1,
        1,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_candidate_restart_mutation.sh": (
        6,
        40,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_certificate_ref_recovery_mutation.sh": (
        1,
        1,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_certified_response_identity_separation_mutation.sh": (
        1,
        2,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_certified_response_registration_mutation.sh": (
        4,
        3,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_certified_response_source_lineage_mutation.sh": (
        1,
        1,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_commit_import_provenance_mutations.sh": (
        2,
        2,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_decision_recovery_lifecycle_mutation.sh": (
        1,
        8,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_effect_capacity_ownership_mutation.sh": (
        10,
        11,
        0,
        7,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_historical_discovery_occurrence_rank_mutation.sh": (
        2,
        1,
        0,
        1,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_indexed_height_mutation.sh": (
        4,
        0,
        0,
        2,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_indexed_service_activation_mutations.sh": (
        2,
        1,
        0,
        1,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_inflight_first_release.sh": (
        1,
        9,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_ingress_causal_freshness_mutation.sh": (
        1,
        1,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_item_carrier_typing_mutation.sh": (
        0,
        0,
        0,
        0,
        2,
    ),
    "scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh": (
        30,
        73,
        0,
        1,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_multilane_mutations.sh": (
        0,
        37,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_persist_install_generation_mutation.sh": (
        1,
        1,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_persist_install_validation_mutation.sh": (
        1,
        1,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_post_decision_timeout_mutation.sh": (
        1,
        9,
        0,
        0,
        0,
    ),
    PRODUCER_CONTINUATION_PHYSICAL_CUT_MUTATION_RUNNER: (3, 1, 0, 2, 0),
    "scripts/formal/run_sumeragi_v2_productive_mutation.sh": (
        3,
        2,
        0,
        1,
        1,
    ),
    "scripts/formal/run_sumeragi_v2_progress_mutations.sh": (
        22,
        24,
        2,
        6,
        2,
    ),
    "scripts/formal/run_sumeragi_v2_replay_locked_body_carrier_mutation.sh": (
        1,
        1,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_reply_writer_deadline_mutations.sh": (
        4,
        11,
        1,
        1,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_restart_locked_fetch_order_mutation.sh": (
        1,
        2,
        0,
        0,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_serve_scheduler_ordinal_mutations.sh": (
        6,
        0,
        0,
        6,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_service_rank_mutation.sh": (
        20,
        9,
        0,
        12,
        0,
    ),
    "scripts/formal/run_sumeragi_v2_tlc.sh": (7, 1, 0, 0, 4),
    "scripts/formal/run_sumeragi_v2_typed_rollover_handoff_mutations.sh": (
        3,
        43,
        0,
        2,
        0,
    ),
}
SHARED_TLC_RESULT_ASSERTION_SITE_PROFILES = {
    caller: (1, 1, 2, 1, 1)
    for caller in SHARED_TLC_RESULT_CONTRACT_CALLERS
}
SHARED_TLC_RESULT_ASSERTION_SITE_PROFILES.update(
    {
        "scripts/formal/check_sumeragi_v2_replay_trace.sh": (0, 0, 0, 0, 0),
        "scripts/formal/run_sumeragi_v2_historical_discovery_occurrence_rank_mutation.sh": (
            1,
            1,
            1,
            1,
            1,
        ),
        "scripts/formal/run_sumeragi_v2_indexed_height_mutation.sh": (
            1,
            1,
            1,
            1,
            1,
        ),
        "scripts/formal/run_sumeragi_v2_item_carrier_typing_mutation.sh": (
            0,
            0,
            1,
            1,
            0,
        ),
        "scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh": (
            1,
            1,
            3,
            1,
            0,
        ),
        "scripts/formal/run_sumeragi_v2_multilane_mutations.sh": (
            0,
            1,
            2,
            1,
            1,
        ),
        PRODUCER_CONTINUATION_PHYSICAL_CUT_MUTATION_RUNNER: (
            1,
            1,
            5,
            1,
            4,
        ),
        "scripts/formal/run_sumeragi_v2_productive_mutation.sh": (
            3,
            1,
            1,
            1,
            1,
        ),
        "scripts/formal/run_sumeragi_v2_serve_scheduler_ordinal_mutations.sh": (
            1,
            1,
            1,
            1,
            1,
        ),
        "scripts/formal/run_sumeragi_v2_service_rank_mutation.sh": (
            18,
            1,
            2,
            1,
            1,
        ),
        "scripts/formal/run_sumeragi_v2_tlc.sh": (1, 1, 2, 2, 0),
        "scripts/formal/run_sumeragi_v2_typed_rollover_handoff_mutations.sh": (
            1,
            1,
            0,
            1,
            1,
        ),
    }
)
SHARED_TLC_RESULT_BRANCH_PROFILES_SHA256 = (
    "90b7e94020ba1522babe116165e2904401a53f1eda5f267abae59c64e841db31"
)
SHARED_TLC_RESULT_ASSERTION_SITE_PROFILES_SHA256 = (
    "79df8c9846f4e5059d81fdadc5e8898a2063993a8c12aa6418c85e5e44b7df1d"
)
LIVENESS_OWNERSHIP_MUTATION_SHA256 = {
    "SumeragiV2LocalIngressSchedulerReservationMutation.tla": (
        "d5083b4dcd0e1fbd3b55a22b98a7ea41b6b20b646a717fc36beb41fae7788ab5"
    ),
    "SumeragiV2RestartTerminalDurabilityMutation.tla": (
        "7eae8edd73c78803cf5829c056ee29ee1e1f39cd1622d775df5c7ab90845d30b"
    ),
    "SumeragiV2ExactIngressTicketPriorityMutation.tla": (
        "5a318e82bc083a462840278c90ea78d7db755f3d78502f2df992e99dc98af969"
    ),
    "SumeragiV2ExactServeRestartTombstoneMutation.tla": (
        "d5d145bf3b8080484a0309ac60de67bcb7b015e61e50ff891906329969182cac"
    ),
    "SumeragiV2ExactResponseClaimLifecycleMutation.tla": (
        "7404185ead32a48757956da2c852c40f641e90ff1707a1ada87d8b33f145492f"
    ),
    "SumeragiV2ExactServeFrozenPredecessorMutation.tla": (
        "9d20b3f129e9bf3ad2b9cd39eabbf2f4200f411aca64d548c042081e4570cc1a"
    ),
    "SumeragiV2ExactInstalledTcRetentionMutation.tla": (
        "6a67f962a092c40159e0be2e8a07f47661aecbfe05c9ba2827e55a7e93394389"
    ),
    "SumeragiV2ControlLivePredecessorMutation.tla": (
        "a41b96b82eb8998a988745a0a6c103a6e7f97d0d012c4765796304ee1f25f328"
    ),
    "SumeragiV2ImportedCertificateTailMutation.tla": (
        "1d570d5e21fa0ea61c03eeab0087b98dd4d2c358e8cc72ac414f2be54cf928f0"
    ),
    "SumeragiV2ImportedTcTailMutation.tla": (
        "a02de16f8f1538fb965b176ce6f8d8b9f56d08d90215c140b3daea379f5223d5"
    ),
    "SumeragiV2TimeoutLifecycleStageClassifierMutation.tla": (
        "ea064f1e0075da903140c740bbc1ffb803a67b9c0c41b32997f5293125517be4"
    ),
    "SumeragiV2PersistInstallTimeoutTagMutation.tla": (
        "3fdc3d63a9ab6afd2a284bdbfa776e07664cd39b442479aeba4d748c6a1b3bef"
    ),
    "SumeragiV2PersistInstallTimeoutRootRetirementMutation.tla": (
        "82f16d90647b5061be3d40444aabe68d91cdb4cb9a76adaec713bf4cfbd9efa3"
    ),
    "SumeragiV2AdequateLeaderWireTombstoneMutation.tla": (
        "f009b5bfe9ac7a98c3fbdfea1348b699e0be08419388949aa53a0d06819aff60"
    ),
    "SumeragiV2AdequateLeaderCandidateTombstoneMutation.tla": (
        "ee8c810d32b03e1469fb15a4a7f421ff0ade25abbb2863c7f38a9993391fc42a"
    ),
    "exact_ingress_ticket_priority_fixed.cfg": (
        "2529761511261f8028f0843232f7bd342025718fefc553634b38fcfb53faeed6"
    ),
    "exact_ingress_ticket_runtime_first_bug.cfg": (
        "5cdd996dd0e3e982d1fd755d89713540079ef0a2a92e581869790094dd2498bd"
    ),
    "exact_serve_restart_tombstone_fixed.cfg": (
        "2ec0271cc0a45f798903faffe678f08925269bd88e811db01230a641845d5d65"
    ),
    "exact_serve_restart_tombstone_bug.cfg": (
        "9815b50b005dbd7e1ecc761a2026b7535f8537f1a51e7e34682d211495a501b6"
    ),
    "exact_response_claim_lifecycle_fixed.cfg": (
        "41a115fe5c08e6ef48a01f391ebd374c71983a910aa7c06d35b8a2a6e1ecf743"
    ),
    "exact_response_claim_duplicate_bug.cfg": (
        "c7f94b2f24610cb2d0518db4f2e7a1a09bd0cc67d4921cb37a183fc936938ff7"
    ),
    "exact_response_claim_competing_responder_bug.cfg": (
        "845789ad5f66388ba39bd4136015806b5c21b1b0d73150e124c85ffb71466ea0"
    ),
    "exact_response_claim_resurrection_bug.cfg": (
        "e7d847760565114559368a555f2a14ef859f523e81d9a0ee3c32b799a94a9e38"
    ),
    "exact_response_claim_restart_reopen_bug.cfg": (
        "022e7201d6dff5be8c15d50d29c7cf82886b634fa291e6101c27cc1e3edea5c8"
    ),
    "exact_serve_frozen_predecessor_fixed.cfg": (
        "6fe006467fc18fe6a9977dd17c9b0ee277cbd7fc6eb134e0f1149b97d1c9abff"
    ),
    "exact_serve_frozen_predecessor_churn_bug.cfg": (
        "b1b8479fbd63354d3ace4ad65e263bbac880f0b08cacbfb65c6706042511a6a5"
    ),
    "exact_installed_tc_retention_fixed.cfg": (
        "9d3cca98987fac04c2ac299eb7c06a290ec026f64438fd59151e0047a37641c2"
    ),
    "exact_installed_tc_view_only_bug.cfg": (
        "b1b58094e561aa81637353b513087b0381855c8c510f96a22bab7fd63aadaf5d"
    ),
    "control_live_predecessor_fixed.cfg": (
        "7605f45314f9bdb487be25a7f7dadd9c1a601a14bb439378b78f6e4316c9296d"
    ),
    "control_live_predecessor_bug.cfg": (
        "8d6fe07f6f5089376222bae54b8b334c91c45cd16a1d7ea5239eb56f4aed14a0"
    ),
    "imported_certificate_tail_fixed.cfg": (
        "9a580cb30bbe382fc1395082adf91c039de922efddb34d676cbf542852a860e0"
    ),
    "imported_certificate_tail_bug.cfg": (
        "4fc37a5d837e7a866cf93c8209ee8865f73833e5b6a1fe5ccd899c3182f75e89"
    ),
    "imported_tc_tail_fixed.cfg": (
        "921f83621de53f5afb271e899e58da4808038159e49eddf06c716a80360cac96"
    ),
    "imported_tc_tail_bug.cfg": (
        "350667c819a07f1914a297d78d46c8006774b44efeaa22a824b8850920a266a6"
    ),
    "timeout_lifecycle_stage_classifier_fixed.cfg": (
        "b323c813ce084bc6c85c64f594bf7bd55ae6ebdcbe1be3c408b830e5014f9035"
    ),
    "timeout_lifecycle_stage_classifier_bug.cfg": (
        "9e34e285ced97f50410472ad286c45204da06d7d8799c13253e2283a6f786f27"
    ),
    "persist_install_timeout_tag_fixed.cfg": (
        "c1e7de9502836c9788734813e23349fd1797090bda2b01eb5cc4983a4d11ae2e"
    ),
    "persist_install_timeout_tag_bug.cfg": (
        "2fba91cff8430b97696d74363185f7a998638be3600ba9f02b991df01b2b65c1"
    ),
    "persist_install_timeout_root_retirement_fixed.cfg": (
        "6043662a4f107115e9c9924b4bda1580a1641e4169473309aaabaf8bf4dcd243"
    ),
    "persist_install_timeout_root_retirement_bug.cfg": (
        "5b6cecf701049e6e23ee2c92f13395625fe12ae5de1316b620581d81448eb2f7"
    ),
    "local_ingress_scheduler_reservation_fixed.cfg": (
        "b8e4fe52c9d7484f429962e638f104af23015f9b6ee48d9469021cfe0a0d4ff4"
    ),
    "local_ingress_scheduler_reservation_mutable_next_bug.cfg": (
        "385f27f3b9d9f4f74d4110dfba0d8d17c721f99ca44abaaa85f367a5e84de5e6"
    ),
    "restart_terminal_durability_fixed.cfg": (
        "a405e56a818aa31efbdd685480d1eba379ac4500df0e1cea00d0b97de9548e7c"
    ),
    "restart_terminal_durability_blanket_terminal_bug.cfg": (
        "2d8a68e618db861bde394461e1ff4f5da620181e42e95b09ba508e11abbe25a7"
    ),
    "adequate_leader_wire_tombstone_fixed.cfg": (
        "86914100ad289ec7d2bd649899ebe364571633db7e640a675cef2ee174bf372d"
    ),
    "adequate_leader_wire_slot_cardinality_bug.cfg": (
        "c9df85b21f862a2303857d981b110852289c3ce94b9db71223784ff678ba7f8a"
    ),
    "adequate_leader_wire_same_view_replacement_bug.cfg": (
        "18f455655e9161409622acea31f581440337ec7861c990372dedcaa00fb24b6a"
    ),
    "adequate_leader_wire_retry_coalescing_bug.cfg": (
        "c2a9ee8632b5a50ee6bc12ee778c9f962aa430e70e78ee78184011c784dbe93f"
    ),
    "adequate_leader_wire_tombstone_bug.cfg": (
        "c99a9cbf5de2dd947ee9c36d5b81425e10dd5b715604fc2c9890353bd0a2a359"
    ),
    "adequate_leader_wire_restart_resurrection_bug.cfg": (
        "a601b2821bde4eef40fff2678d3d62a6f5d327f7215c2794858f0292ef3c4410"
    ),
    "adequate_leader_wire_restart_reopen_owner_bug.cfg": (
        "3261e5e2b949f6f2c81e453e2f416053f0e61138050367735f84ae9761b5a02f"
    ),
    "adequate_leader_wire_restart_packet_synthesis_bug.cfg": (
        "cc2557767c265c31626ac1667c12ff8b600ca837b12bbaf631de81ac3a18f4d7"
    ),
    "adequate_leader_wire_restart_ordinal_reallocation_bug.cfg": (
        "0739fa5ed3cc7db39ed6cea96ce616c44224d129b681e823814eee8da3d67b41"
    ),
    "adequate_leader_wire_restart_prefix_recharge_bug.cfg": (
        "a3b2e3faebf838b619d8a566c005651a6d9fe0aaa21c85a97cd67a66f4ddf4fc"
    ),
    "adequate_leader_wire_dormant_potential_precharge_bug.cfg": (
        "29e2fc2d5e035d61e3dac19e859279cf6548f753cba547379003e60da16cdfc5"
    ),
    "adequate_leader_wire_restart_capacity_bypass_bug.cfg": (
        "55e58a4e4b37c8963d519092bcaad671f58ab20c9324efe3255f90875f6985fc"
    ),
    "adequate_leader_wire_unconsumed_completion_bug.cfg": (
        "69f4c45ab376bd0bee8780ce45c4a0ecbe64147f56ba4db5c24761ba1fab4da3"
    ),
    "adequate_leader_wire_rollover_reset_bug.cfg": (
        "81146c732a03279f405e3b177e912cb54d8b1da74410f9f639cd9776231ba692"
    ),
    "adequate_leader_candidate_tombstone_fixed.cfg": (
        "38ce760903e3dc175ecd2a0452a9c9dfee682e5c575bba33d1ca0776dc49f1a3"
    ),
    "adequate_leader_candidate_resurrection_bug.cfg": (
        "738382cd3dba12196ac5c102b789fbf993625d262eb79b9571565c357e2e1244"
    ),
    "adequate_leader_candidate_terminal_discard_resurrection_bug.cfg": (
        "5fedb098c87e359fd7088ec606ae918f215633eb821a1543e29e46e8440c1b06"
    ),
    "adequate_leader_candidate_retired_chunk_view_bug.cfg": (
        "dd713232b0dadffd6b3df927f50e9e2e9399b75e93164aa59ad06bed771d42be"
    ),
    "adequate_leader_candidate_retired_chunk_decision_bug.cfg": (
        "a901275bbb7d9614ae9962a5fe45b5d9f9bf7a60930cf6f29880325d09017659"
    ),
    "adequate_leader_candidate_restart_resurrection_bug.cfg": (
        "06c278f4b1b923a890ce90b9f241def196ac3ba39e966a4e54de83ed9a6e9b40"
    ),
    "adequate_leader_candidate_restart_volatile_owner_loss_bug.cfg": (
        "0cc6ea08454b17d26ec3d40c81c0b96f1b4f1f54d08df551c1b45302543b64aa"
    ),
    "adequate_leader_candidate_signed_restart_suppression_bug.cfg": (
        "a9cc8ebd7dc4fd03cc6cf324934784cf9c15f3972ae45fc36d559b4d7a00e413"
    ),
    "adequate_leader_candidate_aggregate_evidence_identity_explosion_bug.cfg": (
        "9dcda56594d8a39f1eb7054fbec984e4bdceec12b0bc11a5eb0e39dfcaa2fbc5"
    ),
    "adequate_leader_candidate_strict_view_reclamation_bug.cfg": (
        "1f88be0a94a26385fe46c9f8fe8ea074d80068f41a66e857ce9b85825af77d5d"
    ),
    "adequate_leader_candidate_rollover_reclamation_bug.cfg": (
        "cb537e05878351e50108268cb8a462f1856e66c931f2dadc4d306da89afcca63"
    ),
    "SumeragiV2ExternalProducerContinuationMutation.tla": (
        "894cc8016042071da3811025955be63bbf6c3a63c3a89db94399cd80e1519a32"
    ),
    "SumeragiV2EmptyProducerHandoffMutation.tla": (
        "ab46b7e8a0aa0ad162185e067647264bc252371090dda133a44e4dc7694cb5f6"
    ),
    "SumeragiV2ProducerOriginReservationMutation.tla": (
        "3a0c8fc62c67b4bad96692fcbe4c75c2b2a3f9f7b3b95a2a502e28ecb081fd19"
    ),
    "SumeragiV2ProducerContinuationCausalRankMutation.tla": (
        "cdf86251806c6fec7583293fe80d2e6da723421ef7fb477f54d5f7ebf7e385ae"
    ),
    "SumeragiV2RepresentativeLiveScopeMutation.tla": (
        "77f33922981965ab4de19bdf5e5ef7a88fbd820474482c6a95f55f2580d5b0ed"
    ),
    "SumeragiV2FixedCorridorPhysicalBudgetMutation.tla": (
        "474364c3251ab2b24838d1e7034c3d4f6a28928581ada184c7b2a7479ef69860"
    ),
    "SumeragiV2FixedCorridorActionCreditMutation.tla": (
        "c209e7f2ce212d3f7ea8620f7458c79d1a932e32d0b2b4c2ef1b5831979c5f71"
    ),
    "SumeragiV2ProposalPipelineBudgetMutation.tla": (
        "76c174b34dc721ce5874e2190270372f74e931532783b13f0991c3e2b592b77f"
    ),
    "adequate_leader_wire_terminal_identity_bug.cfg": (
        "ff7cc8d7cf34fc43a858aeb4acd3c0edd142f4aa69bec0f5d4e9c9f88953587f"
    ),
    "external_producer_continuation_fixed.cfg": (
        "4e022e0641229d03ddd68b54c2ecb1f8d54401014587b50e1167848289e14fc7"
    ),
    "external_producer_continuation_missing_conditional_bug.cfg": (
        "22195415cc53f4f35eb2cf4c4632def9fdfcb572ba747469d75880b80e12a2a1"
    ),
    "external_producer_continuation_missing_volatile_bug.cfg": (
        "414fcd8421a0ff093cc78f7cfbb03c24229a765378a4ccfb91110931c8953b5c"
    ),
    "external_producer_continuation_synthetic_carrier_bug.cfg": (
        "84b5ba1d8cdde7fbf6f3fbb017330b54787c762e09d4c26ee7e6406f8f76f95d"
    ),
    "external_producer_continuation_resurrection_bug.cfg": (
        "f82e714091ba3f8eb136b217cc9bdb483800c7a2fda71e2166e671d520a5343e"
    ),
    "external_producer_continuation_missing_conditional_fairness_bug.cfg": (
        "bd69a4736e55da69dc298787f794f06a045eb5e9815fee155b3ae038647c46e2"
    ),
    "external_producer_continuation_missing_volatile_fairness_bug.cfg": (
        "af3428dfd55a43dcf73209423666da2983c8798b9d34c8cf0687d06d38ee2322"
    ),
    "empty_producer_handoff_fixed.cfg": (
        "0846fb92977462e752d4f1737e7e5852ce7ae971b709e04a792ffb456bcdac02"
    ),
    "empty_producer_handoff_missing_reservation_bug.cfg": (
        "4b50f4b6f0a96f2b7b34d629efed6f8753a1ac493f6ae42b84e688375c40ceee"
    ),
    "producer_origin_reservation_fixed.cfg": (
        "10776d04ec4a12ad716e9576ef536783e9ea0d9850a3baa322a618467d2841ed"
    ),
    "producer_origin_reservation_missing_owner_bug.cfg": (
        "c7eab325f626886412334d227e8636c3e66a0dc9e889bce1b247e1024bcd6840"
    ),
    "producer_origin_reservation_new_ordinal_bug.cfg": (
        "77122d6803a4b3fcd98404dc61fa5e4027c942a1f834b1cb0fe268478d24486e"
    ),
    "producer_origin_reservation_duplicate_retry_bug.cfg": (
        "1c3823482603a8925eb0c45d50ec81bcca579748dcf5932b15c7fc239f86b5f1"
    ),
    "producer_continuation_causal_rank_fixed.cfg": (
        "c06054308bb71c88014ca8a02f2c63dc340758ec675d4ebaa9cb40d119f0f072"
    ),
    "producer_continuation_causal_rank_stage_only_bug.cfg": (
        "ab55fd59f1653601cf19b79d24d26a875bbc3f44503586c70bfb89f2abead3d4"
    ),
    "SumeragiV2ProducerReplayCapacityMutation.tla": (
        "7308867958c4e51d170d73000c9c02b41a871ba6a056b282485ac3c8b742152a"
    ),
    "producer_replay_capacity_fixed.cfg": (
        "0b08365ef5269da08fc37c20e191b5a531e62a947db51e65c5ada9d794b1bbcd"
    ),
    "producer_replay_capacity_blind_invariant_bug.cfg": (
        "ab13127ba49f160b486ce0496eb7c7a40497a52026ce262dd1b8910235b60c04"
    ),
    "producer_replay_capacity_non_atomic_replay_bug.cfg": (
        "671a654d498fbcf0db02f1ab5fdb5619f503ad8fe225cbda60d3d56e43ec7fd2"
    ),
    "producer_replay_capacity_replenishment_lasso_bug.cfg": (
        "cb50abfe31bf3c24454e3f3a0b055f2a889b6ce7070aa0b54533485684bf098f"
    ),
    "representative_live_scope_fixed.cfg": (
        "3579a888b90aa2c08be038a7d09b4bde18a9abb066206a13c2cdcf9ccf3e5c4c"
    ),
    "representative_live_scope_missing_premise_bug.cfg": (
        "4459bd5c261edcb289d18e01f662232b34dc95279757ad4f1a836ee81a767a53"
    ),
    "fixed_corridor_physical_budget_fixed.cfg": (
        "64d5bede198e93095ac4ebd78e95bb67492f2ba56bd510078095201eac7cfbbf"
    ),
    "fixed_corridor_physical_budget_omitted_lane_cursor_bug.cfg": (
        "c96633d9cb41fa777f6e4e05513b182f4a5018af81756e20851feffc69b9eb10"
    ),
    "fixed_corridor_action_credit_fixed.cfg": (
        "2818f636e9ee68c67022179e05ec82da942772336d2f3eba312f4e1838507ee2"
    ),
    "fixed_corridor_action_credit_per_child_recharge_bug.cfg": (
        "cc0dbf42553c4a08b7db2965aebed4fd18c4fc99ca2488c1f4a306c133340f60"
    ),
    "proposal_pipeline_budget_fixed.cfg": (
        "a51fa680a285fe56fcd81ede0b4eaf42390b9dcc75dead6101a34cf8910aa4c4"
    ),
    "proposal_pipeline_budget_additive_bug.cfg": (
        "49b0376d1a46952ce3b4564a4020189808ce3cc4af89761b0b180f877a7ca17b"
    ),
    "SumeragiV2AuthorityDeadlineCarryMutation.tla": (
        "f574dc972bd94a8d780c908b0f6e2e4d6db633f5dc871304556f42bedfa9c674"
    ),
    "SumeragiV2AdequateLeaderDeadlineAuthorityMutation.tla": (
        "c2a17bc53f7cfa52cfb40bc4d4c97b59b8615fcf363c01ec1a4b0788785e0e6f"
    ),
    "SumeragiV2AdequateLeaderSelectedLifecycleEpisodeMutation.tla": (
        "b812e0d3a0cbdb3300979e12c1e2a63198bf092f9d20c817e83587a268853ae7"
    ),
    "authority_deadline_carry_fixed.cfg": (
        "2b01485673bfed56d68a82fab2d4856051a68ef44eb754cae71e003b753a8973"
    ),
    "authority_deadline_carry_expired_receipt_bug.cfg": (
        "5b5306c35f7691270ea2e8340d55787770caaeb868c814f8344031014c0d1a09"
    ),
    "authority_deadline_carry_kernel_recharge_bug.cfg": (
        "306ffcd5a1199fb0857627842bafc68221ca1c61fb01141e96eff69b6d616857"
    ),
    "adequate_leader_deadline_authority_fixed.cfg": (
        "cd06ab8ad3f9230aa0b89e80e696deb8b92c69e5f75de0d6c560aa9050869916"
    ),
    "adequate_leader_deadline_authority_omitted_roster_bound_bug.cfg": (
        "baa7b9385882f260ba15c9db6c7e8c29102ef0f7c0cb3a8e3da558709d54cc6e"
    ),
    "adequate_leader_selected_lifecycle_episode_fixed.cfg": (
        "c47dfb2365f45d19fba12f6ae12ec3b7ad74081b058c678ae4b5768ef432e0ce"
    ),
    "adequate_leader_selected_lifecycle_episode_semantic_shortcut_bug.cfg": (
        "f1829446aa850f90a5958f2df2c311cd10b512ec2267b1984e8d08844febc7d8"
    ),
    "SumeragiV2FixedCorridorReceiptAcquisitionMutation.tla": (
        "20808fe38c7b518cf50f3fdc70544be91debc1c27b631ac9aa94567eb5890115"
    ),
    "fixed_corridor_receipt_acquisition_fixed.cfg": (
        "9e1279aff050d6f4fdd8403b71cb471f50030e1041e939cd2b3ef5871ea750c3"
    ),
    "fixed_corridor_receipt_acquisition_prestate_only_bug.cfg": (
        "adb35799b58cd2bb8ec621f44595ccd4d8bdba097508c48a3b78cce47ac0691d"
    ),
    "fixed_corridor_receipt_acquisition_global_retire_bug.cfg": (
        "a94a946e62230d7a656c0945737b744945e73051f812c7ece7d6c35f107a7e2a"
    ),
    "SumeragiV2OrdinaryIngressCarrierRebaseMutation.tla": (
        "ceb29032619c062b17357c4a963de60e1119bcdc038981689afe43e079a5f653"
    ),
    "ordinary_ingress_carrier_rebase_fixed.cfg": (
        "58e1ebceef08ce1e5d8f1dc5bb34777bfd158d68e3891ba67af7bf49b5049d98"
    ),
    "ordinary_ingress_carrier_rebase_identity_bug.cfg": (
        "2db01fb1de0ffce2d91b0bc1fdc737ea9b837925d298b8ea13470b6344e53e00"
    ),
    "ordinary_ingress_carrier_rebase_minimum_bug.cfg": (
        "14f5911140e645af8371cc282006fea6b879968845839389b550c057e8df0cda"
    ),
    "SumeragiV2ServeRestartTerminalDischargeMutation.tla": (
        "15502349b0214b15c96dbc0eb6c8b62f8e43d2059b0ca11974ffa7ceabb24a4e"
    ),
    "serve_restart_terminal_discharge_body_fail_open_bug.cfg": (
        "c4ed94aa1890354d4f262da0ec9b6ea51cbb606d8527f9419c266e7ee2c21691"
    ),
    "serve_restart_terminal_discharge_crash_resume_incomplete_union_bug.cfg": (
        "3058fd7a71710a7e0e3824ade7b7a6314129f231296f173a37b1a6d80621d9ac"
    ),
    "serve_restart_terminal_discharge_crash_resume_order_bug.cfg": (
        "3645cd2d4083a963d56ca751d64cd0e0b1c1291d0ac005590645aa39eee2576f"
    ),
    "serve_restart_terminal_discharge_duplicate_outcome_bug.cfg": (
        "b37ff1837d8d6913d68b29089f3a9264d552b9b7b7344d45d1ca1ea693c773d8"
    ),
    "serve_restart_terminal_discharge_family_coexistence_bug.cfg": (
        "79f2ca284cec0a61048694783dd570a5721fc330bb1f82e381316301e3440641"
    ),
    "serve_restart_terminal_discharge_fixed.cfg": (
        "0dd46a7a6ed21d470ee4160c353f8c026a6cb09a183374f56c10aee84062b030"
    ),
    "serve_restart_terminal_discharge_incomplete_union_bug.cfg": (
        "f199296d65c2e14ea80837ec67aafed00ef612e6f2aa31b16cdf4e17da8d431f"
    ),
    "serve_restart_terminal_discharge_live_decision_conversion_bug.cfg": (
        "7d7a6f76ee67e9edf4792f0f1d334eb6a524ca6f01840afa6f7cf484698541db"
    ),
    "serve_restart_terminal_discharge_mismatched_waiter_bug.cfg": (
        "72ff08dddf37d2b196e5ba8e2b2585296f5314b0254d6104fa785f38b7cc41b6"
    ),
    "serve_restart_terminal_discharge_negative_retry_ordinal_bug.cfg": (
        "16290fed1624e8ae9b76997aca7d6f52b94f96cc924ae0b8aee6d41c590a7d5d"
    ),
    "serve_restart_terminal_discharge_negative_terminal_sign_bug.cfg": (
        "29a0950516f2e1bcf9df95003d239c99f17b2f9103f437d519e7553d3f924aab"
    ),
    "serve_restart_terminal_discharge_negative_waiter_bug.cfg": (
        "02ad40ef2a311cba8f1cccbdc1c1ea8076c59bb2f4d0bea0f7fc060f1cff36d2"
    ),
    "serve_restart_terminal_discharge_order_bug.cfg": (
        "4f6362c1ed3bc68b89b53c476a7c0796bbed078da1a0248ab2529e370e6d9368"
    ),
    "serve_restart_terminal_discharge_orphan_waiter_bug.cfg": (
        "a7252a436b270213768e11a0563c4919bb9e7555a4c6ecd1fb8f44207aa71135"
    ),
    "serve_restart_terminal_discharge_owner_request_mismatch_bug.cfg": (
        "e69e5926363f37ba97adbf1ed9c6d05ef3882ef2e21111acd029da7af225305c"
    ),
    "serve_restart_terminal_discharge_persistence_bug.cfg": (
        "020980fdfd362c88529f2744cbeeb2f4eb59d9240b41a002ebc424b28f7b602e"
    ),
    "serve_restart_terminal_discharge_prefence_decision_rewrite_bug.cfg": (
        "779e319ce87de871670bc190167582577d399f47ae617e8f351e0b8d0dd763dd"
    ),
    "serve_restart_terminal_discharge_prepared_decision_drain_bug.cfg": (
        "67dcf0855c151034d87349e2fd06edbd7ee345cadd74cbf6ccacef19d72cc572"
    ),
    "serve_restart_terminal_discharge_producer_exposure_bug.cfg": (
        "25290e7420eae0fd379869070bbd04a1ae6cae3380d37fbd38ed0c5106001d16"
    ),
    "serve_restart_terminal_discharge_raw_context_gate_bug.cfg": (
        "52cbdb9148f195f757e58cfb659c636dbf242fc2fde75b6d340ebc752b54cb06"
    ),
    "serve_restart_terminal_discharge_receiver_close_bug.cfg": (
        "9e8e9e2a4c351656da12695356a1a84843742849f0de78041f8f42930f0d7763"
    ),
    "serve_restart_terminal_discharge_restart_decision_conversion_bug.cfg": (
        "f34bbfd72a45155f92189da2a0942bb92433a2e30915ba9094408a3d28046da7"
    ),
    "serve_restart_terminal_discharge_resurrection_bug.cfg": (
        "9f4f926c0d184219842ffae3842c26e2b536d08ec1ed0de39d35cabb52bfc059"
    ),
    "serve_restart_terminal_discharge_roster_fanout_bug.cfg": (
        "81c968a71c55a6cce65bf41439eae47509bf557a419ca1afc4fa254ae84cc40c"
    ),
    "serve_restart_terminal_discharge_signer_authority_bug.cfg": (
        "2bdf01fd1ce3884f64802217a9b1b5cf3c86b2c1831964056373629bfd3875f2"
    ),
    "serve_restart_terminal_discharge_terminal_replay_resign_bug.cfg": (
        "d842c8552094eff841e3c9c159399d680a50edbd63ff2c1b96ad6a3330a8f644"
    ),
    LIVENESS_OWNERSHIP_MUTATION_RUNNER: (
        "3f3738a6351e7f7fe78ac3f6a4a4e738f8b6fd2b098d7466674197c783783d25"
    ),
}
LIVENESS_OWNERSHIP_MUTATION_FORMAL_GLOBS = (
    "SumeragiV2LocalIngressSchedulerReservation*.tla",
    "SumeragiV2RestartTerminalDurability*.tla",
    "SumeragiV2ExactIngressTicketPriority*.tla",
    "SumeragiV2ExactServeRestartTombstone*.tla",
    "SumeragiV2ExactResponseClaimLifecycle*.tla",
    "SumeragiV2ExactServeFrozenPredecessor*.tla",
    "SumeragiV2ExactInstalledTcRetention*.tla",
    "SumeragiV2ControlLivePredecessor*.tla",
    "SumeragiV2ImportedCertificateTail*.tla",
    "SumeragiV2ImportedTcTail*.tla",
    "SumeragiV2TimeoutLifecycleStageClassifier*.tla",
    "SumeragiV2PersistInstallTimeoutTag*.tla",
    "SumeragiV2PersistInstallTimeoutRootRetirement*.tla",
    "SumeragiV2AdequateLeaderWireTombstone*.tla",
    "SumeragiV2AdequateLeaderCandidateTombstone*.tla",
    "SumeragiV2ExternalProducerContinuationMutation*.tla",
    "SumeragiV2EmptyProducerHandoffMutation*.tla",
    "SumeragiV2ProducerOriginReservationMutation*.tla",
    "SumeragiV2ProducerContinuationCausalRankMutation*.tla",
    "SumeragiV2ProducerReplayCapacityMutation*.tla",
    "SumeragiV2RepresentativeLiveScopeMutation*.tla",
    "SumeragiV2FixedCorridorPhysicalBudgetMutation*.tla",
    "SumeragiV2FixedCorridorActionCreditMutation*.tla",
    "SumeragiV2ProposalPipelineBudgetMutation*.tla",
    "SumeragiV2AuthorityDeadlineCarryMutation*.tla",
    "SumeragiV2AdequateLeaderDeadlineAuthorityMutation*.tla",
    "SumeragiV2AdequateLeaderSelectedLifecycleEpisodeMutation*.tla",
    "SumeragiV2FixedCorridorReceiptAcquisitionMutation*.tla",
    "SumeragiV2OrdinaryIngressCarrierRebaseMutation*.tla",
    "SumeragiV2ServeRestartTerminalDischargeMutation*.tla",
    "exact_ingress_ticket_*.cfg",
    "exact_serve_restart_tombstone_*.cfg",
    "exact_response_claim_*.cfg",
    "exact_serve_frozen_predecessor_*.cfg",
    "exact_installed_tc_*.cfg",
    "control_live_predecessor_*.cfg",
    "imported_certificate_tail_*.cfg",
    "imported_tc_tail_*.cfg",
    "timeout_lifecycle_stage_classifier_*.cfg",
    "persist_install_timeout_tag_*.cfg",
    "persist_install_timeout_root_retirement_*.cfg",
    "local_ingress_scheduler_reservation_*.cfg",
    "restart_terminal_durability_*.cfg",
    "adequate_leader_wire_*.cfg",
    "adequate_leader_candidate_*.cfg",
    "external_producer_continuation_*.cfg",
    "empty_producer_handoff_*.cfg",
    "producer_origin_reservation_*.cfg",
    "producer_continuation_causal_rank_*.cfg",
    "producer_replay_capacity_*.cfg",
    "representative_live_scope_*.cfg",
    "fixed_corridor_physical_budget_*.cfg",
    "fixed_corridor_action_credit_*.cfg",
    "proposal_pipeline_budget_*.cfg",
    "authority_deadline_carry_*.cfg",
    "adequate_leader_deadline_authority_*.cfg",
    "adequate_leader_selected_lifecycle_episode_*.cfg",
    "fixed_corridor_receipt_acquisition_*.cfg",
    "ordinary_ingress_carrier_rebase_*.cfg",
    "serve_restart_terminal_discharge_*.cfg",
)

# Exact source seal for the indexed successor service-activation mutation
# matrix.  The unjoined-clock case is a temporal lasso (TLC status 13), while
# restriction re-entry is an invariant counterexample (TLC status 12); the
# runner and checker deliberately keep those two failure classes distinct.
INDEXED_SERVICE_ACTIVATION_MUTATION_FORMAL_ARTIFACTS = (
    "SumeragiV2IndexedServiceActivationMutation.tla",
    "indexed_service_activation_fixed.cfg",
    "indexed_service_activation_unjoined_clock_bug.cfg",
    "indexed_service_activation_reentry_fixed.cfg",
    "indexed_service_activation_reentry_bug.cfg",
)
INDEXED_SERVICE_ACTIVATION_MUTATION_RUNNER = (
    "scripts/formal/run_sumeragi_v2_indexed_service_activation_mutations.sh"
)
INDEXED_SERVICE_ACTIVATION_MUTATION_SHA256 = {
    "SumeragiV2IndexedServiceActivationMutation.tla": (
        "1d5ec33b5ab602b5f0ffa647e7e3edab9f91b0e7e8de94103d6bd05de4a02287"
    ),
    "indexed_service_activation_fixed.cfg": (
        "02dfcea8c0d6aa493d3c1db35a6d9be63ce4792ad2810ba80505f62a9081a8b9"
    ),
    "indexed_service_activation_unjoined_clock_bug.cfg": (
        "a7ade2d1ba060e4ad3ef045bb944f8a6ef29cbff7013612022d75e1f8367ebdb"
    ),
    "indexed_service_activation_reentry_fixed.cfg": (
        "18070e16a82fc310b9ccde6fc31a86b40b2e938929f0a4a6a312d66163307639"
    ),
    "indexed_service_activation_reentry_bug.cfg": (
        "11091b99cbaa5ebce85065da07dfb46eb326243da412e715e6d5bb774f162d81"
    ),
    INDEXED_SERVICE_ACTIVATION_MUTATION_RUNNER: (
        "88c03a575317b85ec59660d4a8e7164798d0ce11b59c8e8da5809f4d5b8132af"
    ),
}
INDEXED_SERVICE_ACTIVATION_MUTATION_FORMAL_GLOBS = (
    "SumeragiV2IndexedServiceActivationMutation*.tla",
    "indexed_service_activation_*.cfg",
)

# Exact source seal for the historical-discovery count-first occurrence rank.
# The repaired model and its plain-minimum mutant are bounded TLC regression
# evidence only; they cannot promote the historical temporal obligation.
HISTORICAL_DISCOVERY_OCCURRENCE_RANK_MUTATION_FORMAL_ARTIFACTS = (
    "SumeragiV2HistoricalDiscoveryOccurrenceRankMutation.tla",
    "historical_discovery_occurrence_rank_fixed.cfg",
    "historical_discovery_plain_minimum_bug.cfg",
)
HISTORICAL_DISCOVERY_OCCURRENCE_RANK_MUTATION_RUNNER = (
    "scripts/formal/"
    "run_sumeragi_v2_historical_discovery_occurrence_rank_mutation.sh"
)
HISTORICAL_DISCOVERY_OCCURRENCE_RANK_MUTATION_SHA256 = {
    "SumeragiV2HistoricalDiscoveryOccurrenceRankMutation.tla": (
        "c6f42ea84dcc1b34924a489630ce05fcf997c27b67c65ba9a38464b436285459"
    ),
    "historical_discovery_occurrence_rank_fixed.cfg": (
        "1037f1bc4236cb8ecc391dc315ed695ea4aee23cbd88b7ec03e4b331ac4a56d0"
    ),
    "historical_discovery_plain_minimum_bug.cfg": (
        "bdd9613583893b49e8f5762ea3342a50f5cc289689b225ef8b55c2aac0e8c49d"
    ),
    HISTORICAL_DISCOVERY_OCCURRENCE_RANK_MUTATION_RUNNER: (
        "cb393225ef5b6b793367f05fd66f807b55ab06f31db94784be238c4261a7c161"
    ),
}
HISTORICAL_DISCOVERY_OCCURRENCE_RANK_MUTATION_FORMAL_GLOBS = (
    "SumeragiV2HistoricalDiscoveryOccurrenceRankMutation*.tla",
    "historical_discovery_occurrence_rank_*.cfg",
    "historical_discovery_plain_minimum_*.cfg",
)

# Exact source seal for the seven replenishment/retirement regressions which
# predate their release-runner registration. These finite TLC models preserve
# their repaired and failing outcomes as regression evidence only.
REPLENISHMENT_REGRESSION_MUTATION_FORMAL_ARTIFACTS = (
    "SumeragiV2AdequateLeaderPreAdmissionRouteMutation.tla",
    "adequate_leader_pre_admission_route_fixed.cfg",
    "adequate_leader_pre_admission_route_identity_bug.cfg",
    "SumeragiV2BusyConsumerMutation.tla",
    "busy_consumer_fixed.cfg",
    "busy_consumer_stale.cfg",
    "SumeragiV2CorridorExitAuthorityReceiptMutation.tla",
    "corridor_exit_authority_receipt_fixed.cfg",
    "corridor_exit_authority_receipt_bug.cfg",
    "SumeragiV2DeferredBusyCursorMutation.tla",
    "deferred_busy_cursor_cyclic.cfg",
    "deferred_busy_cursor_strict_bug.cfg",
    "SumeragiV2DeferredCursorEffectMutation.tla",
    "deferred_cursor_completion_progress_fixed.cfg",
    "deferred_cursor_completion_progress_bug.cfg",
    "deferred_busy_rank_nonregression_bug.cfg",
    "SumeragiV2FixedCorridorDeadlineMutation.tla",
    "fixed_corridor_deadline_reservation_fixed.cfg",
    "fixed_corridor_deadline_owner_refresh_bug.cfg",
    "SumeragiV2HistoricalProducerContinuationMutation.tla",
    "historical_producer_continuation_fixed.cfg",
    "historical_producer_continuation_voter_only_bug.cfg",
)
REPLENISHMENT_REGRESSION_MUTATION_RUNNERS = (
    "scripts/formal/run_sumeragi_v2_adequate_leader_readiness_mutations.sh",
    "scripts/formal/run_sumeragi_v2_service_rank_mutation.sh",
    "scripts/formal/run_sumeragi_v2_historical_discovery_occurrence_rank_mutation.sh",
)
REPLENISHMENT_REGRESSION_MUTATION_SHA256 = {
    "SumeragiV2AdequateLeaderPreAdmissionRouteMutation.tla": (
        "99616466a79971977c84bcd026b78361f007f4ceee4b6d33b6959908dec3d8a6"
    ),
    "adequate_leader_pre_admission_route_fixed.cfg": (
        "b1bc2b6cf01ea358b0402cd5e7778cfdc5d36628c8a0ad195a2ea342da3b0abc"
    ),
    "adequate_leader_pre_admission_route_identity_bug.cfg": (
        "ffe0b1f63fa7071f727307e6e0d8c591a58a12ad974c2d419666ce7e0ebacce3"
    ),
    "SumeragiV2BusyConsumerMutation.tla": (
        "5ba544a00cf6bc893b67385ae68beaa8be13e874129d11d57feb01ef36a538b0"
    ),
    "busy_consumer_fixed.cfg": (
        "e4c1e37d02f469a7c9f61c231f246eb682c057d8d29d206c295303d5d80c2a6c"
    ),
    "busy_consumer_stale.cfg": (
        "11fd50ca8c142db82f3081fffe965f182af17c42c8a7a65a5726853801002abd"
    ),
    "SumeragiV2CorridorExitAuthorityReceiptMutation.tla": (
        "2e5399e931679f7ea00690f0a5da480963b3c0586d8412c439a7bce4167fa7dc"
    ),
    "corridor_exit_authority_receipt_fixed.cfg": (
        "d298c71cfecbc7f21d1168487b7d154f1293921beed3739d5f118f010cfaa007"
    ),
    "corridor_exit_authority_receipt_bug.cfg": (
        "168e957dca0314ac5158e58e2372856243bb2c8918ee9a0cc4a95f11f7b6e18c"
    ),
    "SumeragiV2DeferredBusyCursorMutation.tla": (
        "46dacb621bea28b388d7ffaad160824858557bd01864653051053c64f2882d68"
    ),
    "deferred_busy_cursor_cyclic.cfg": (
        "25385a8c57ed4e91e9bb9db6cf413638a10a00ec502bb0f07cad32c136dd7260"
    ),
    "deferred_busy_cursor_strict_bug.cfg": (
        "9adaa3e8877312e36132823f8e228699db8c068391abf1a1b916155aad6e6628"
    ),
    "SumeragiV2DeferredCursorEffectMutation.tla": (
        "3139bdebfa3bcdce3b6b13daa38f174aa83144aca7a664e609d376580775d338"
    ),
    "deferred_cursor_completion_progress_fixed.cfg": (
        "b579031595e3e9c1230704357071392a7b9e5729a36d0e1f9d35db6a69151ce4"
    ),
    "deferred_cursor_completion_progress_bug.cfg": (
        "11b8809b9818738661cc2a32a451355091c5f4e7ac7ea933beb1ce77ecf63d21"
    ),
    "deferred_busy_rank_nonregression_bug.cfg": (
        "6b7e55b90794a875aa41a8d41ee342f7eddc139711dc3d0c70426a4d262b6580"
    ),
    "SumeragiV2FixedCorridorDeadlineMutation.tla": (
        "1cdaf4f32d4353d884d3e4cf9a01b86a37bf4b4e6c4c2c2431cffee76f7e87d7"
    ),
    "fixed_corridor_deadline_reservation_fixed.cfg": (
        "058f759d88ce9d74d729eb2747b20207cdcbd9a5a28d91bfcf3fa1d01d0a96cc"
    ),
    "fixed_corridor_deadline_owner_refresh_bug.cfg": (
        "8a91acb46c18a1d842577b1a6154b090cf2c18f74d22b561816f6506f18a801a"
    ),
    "SumeragiV2HistoricalProducerContinuationMutation.tla": (
        "6c7e30317f4eda1ba837ce60e035f0713f0312d62301ae25d3ba08d9194f225e"
    ),
    "historical_producer_continuation_fixed.cfg": (
        "3a9a90d05f9c1ee246bf9379d7e7f4132a18fdcc424419c432c2a41cdefb2d42"
    ),
    "historical_producer_continuation_voter_only_bug.cfg": (
        "7e8fb414bea25a44bb89319b4609b62746f315240293d7ba47c90d338c93f837"
    ),
    "scripts/formal/run_sumeragi_v2_adequate_leader_readiness_mutations.sh": (
        "b81171eb4105b8aa8a66c840b3123b8a0cddc8c7515fec23c1a5c0c6ff7541d4"
    ),
    "scripts/formal/run_sumeragi_v2_service_rank_mutation.sh": (
        "e20a740c92cc9edb951699f8e3247b61d0c92701fcde39fca60baad493bb4796"
    ),
    "scripts/formal/run_sumeragi_v2_historical_discovery_occurrence_rank_mutation.sh": (
        "cb393225ef5b6b793367f05fd66f807b55ab06f31db94784be238c4261a7c161"
    ),
}
REPLENISHMENT_REGRESSION_MUTATION_FORMAL_GLOBS = (
    "SumeragiV2AdequateLeaderPreAdmissionRouteMutation*.tla",
    "adequate_leader_pre_admission_route_*.cfg",
    "SumeragiV2BusyConsumerMutation*.tla",
    "busy_consumer_*.cfg",
    "SumeragiV2CorridorExitAuthorityReceiptMutation*.tla",
    "corridor_exit_authority_receipt_*.cfg",
    "SumeragiV2DeferredBusyCursorMutation*.tla",
    "deferred_busy_cursor_*.cfg",
    "SumeragiV2DeferredCursorEffectMutation*.tla",
    "deferred_cursor_completion_progress_*.cfg",
    "deferred_busy_rank_nonregression_*.cfg",
    "SumeragiV2FixedCorridorDeadlineMutation*.tla",
    "fixed_corridor_deadline_*.cfg",
    "SumeragiV2HistoricalProducerContinuationMutation*.tla",
    "historical_producer_continuation_*.cfg",
)

# Exact source seal for the bounded effect-capacity ownership mutation matrix.
# This matrix is finite regression evidence only; it is intentionally separate
# from the deductive release-proof inventory below.
EFFECT_CAPACITY_MUTATION_FORMAL_ARTIFACTS = (
    "SumeragiV2CertifiedRequestCapacityMutation.tla",
    "SumeragiV2EffectCapacityOwnershipMutation.tla",
    "SumeragiV2EffectCapacityOuterTransportMutation.tla",
    "SumeragiV2EffectCapacityRetirementMutation.tla",
    "SumeragiV2EffectPreemptionPriorityMutation.tla",
    "SumeragiV2RetainedEffectBatchMutation.tla",
    "effect_batch_bound_fixed.cfg",
    "effect_batch_decision_filter_fixed.cfg",
    "effect_batch_decision_no_filter_bug.cfg",
    "effect_batch_oversize_accepted_bug.cfg",
    "effect_batch_partial_fifo_fixed.cfg",
    "effect_batch_partial_fifo_reverse_bug.cfg",
    "effect_batch_second_accepted_bug.cfg",
    "effect_batch_second_rejected_fixed.cfg",
    "effect_capacity_certified_request_duplicate_bug.cfg",
    "effect_capacity_certified_request_fatal_bug.cfg",
    "effect_capacity_certified_request_fixed.cfg",
    "effect_capacity_certified_request_lost_bug.cfg",
    "effect_capacity_certified_request_overtake_bug.cfg",
    "effect_capacity_certified_request_partial_pq_bug.cfg",
    "effect_capacity_certified_request_substitute_bug.cfg",
    "effect_capacity_certified_request_upgrade_barrier_lost_bug.cfg",
    "effect_capacity_certified_response_blocked_bug.cfg",
    "effect_capacity_certified_response_byte_reserve_bug.cfg",
    "effect_capacity_certified_response_count_reserve_bug.cfg",
    "effect_capacity_decided_retirement_fixed.cfg",
    "effect_capacity_full_fetch_hol_bug.cfg",
    "effect_capacity_non_fetch_retirement_fixed.cfg",
    "effect_capacity_outer_transport_chunk_class_bug.cfg",
    "effect_capacity_outer_transport_class_fixed.cfg",
    "effect_capacity_outer_transport_response_class_bug.cfg",
    "effect_capacity_retirement_disabled_bug.cfg",
    "effect_capacity_timeout_sign_fixed.cfg",
    "effect_capacity_timeout_sign_lost_bug.cfg",
    "effect_capacity_timeout_sign_refill_bug.cfg",
    "effect_preemption_decided_victim_bug.cfg",
    "effect_preemption_priority_fixed.cfg",
    "effect_preemption_wrong_class_bug.cfg",
    "effect_preemption_wrong_work_id_bug.cfg",
)
EFFECT_CAPACITY_MUTATION_RUNNER = (
    "scripts/formal/run_sumeragi_v2_effect_capacity_ownership_mutation.sh"
)
EFFECT_CAPACITY_MUTATION_SHA256 = {
    "SumeragiV2CertifiedRequestCapacityMutation.tla": (
        "00a69a2a34a7971de3eee14d6e2bb3391913c321922ee46a161d96c626d26483"
    ),
    "SumeragiV2EffectCapacityOwnershipMutation.tla": (
        "de3b89fc0946f138f3ed9d62505f8aab592b907fd8e97b13f3056086edf051d9"
    ),
    "SumeragiV2EffectCapacityOuterTransportMutation.tla": (
        "488ef5fddf49e97894cf6692ce54cd881aa6323d6bd08275ac752a1233921464"
    ),
    "SumeragiV2EffectCapacityRetirementMutation.tla": (
        "72bdbd0799656e9e5a331adeb356239ca572d7efc25e5c6bf59503831a0733e9"
    ),
    "SumeragiV2EffectPreemptionPriorityMutation.tla": (
        "32396b744ca2d2c8fec54f91de4d1e78656dc6e5364da46a44bcb43a3c783522"
    ),
    "SumeragiV2RetainedEffectBatchMutation.tla": (
        "26cf90833350a80b96510202c504c3891d528996e871d30a4bbdf2d54e182028"
    ),
    "effect_batch_bound_fixed.cfg": (
        "3438875c639e882c745e61a2f75bf7202c81dd08c1f48f1de3931c67bad74498"
    ),
    "effect_batch_decision_filter_fixed.cfg": (
        "6aad405ab805b96e1a46097c09a0f6d75ec92734592251076a70dd278ccbbdbf"
    ),
    "effect_batch_decision_no_filter_bug.cfg": (
        "26330118d0840577dbd517f1471a13bcbcfad5470128d5e51b5ea752b3c836ba"
    ),
    "effect_batch_oversize_accepted_bug.cfg": (
        "ddf015a1f351a6ca9b6720efb9341db4672b58fb18cb65df5c1e56118da69b7e"
    ),
    "effect_batch_partial_fifo_fixed.cfg": (
        "61cec19cb64d13e8de940b9e1c4b990df954fdb8f4c24e022b91f693ec60b649"
    ),
    "effect_batch_partial_fifo_reverse_bug.cfg": (
        "4fc57917c8126fb53f014bf892b28cdd9b190da9a1e531b0d1d80edd44919d4e"
    ),
    "effect_batch_second_accepted_bug.cfg": (
        "8e79c97babe1d4cb198986174d1081d5a35d7420ed6187410db65df2876aa938"
    ),
    "effect_batch_second_rejected_fixed.cfg": (
        "463cc383fe9230f44c7f8d78c13acaa1d31994a797d33a37890167a834bc600e"
    ),
    "effect_capacity_certified_request_duplicate_bug.cfg": (
        "cd2af5a19d292fd35eb8daa57d3dffcb2997538dc7bb84ab8d1d34b6774e43d7"
    ),
    "effect_capacity_certified_request_fatal_bug.cfg": (
        "75f3bd2428d7c42cb9f668277d8dbd6110cc8be15b85b229665c3d2ae0a500b5"
    ),
    "effect_capacity_certified_request_fixed.cfg": (
        "3d587733d4bb7f2f91eb893e68dcc337ccfc265dcc510e6c9c1b35821e8626e1"
    ),
    "effect_capacity_certified_request_lost_bug.cfg": (
        "7615c216abb2cc48e43ae09614a96ae4565bb1a0c712a54d34d2d0f7fdad101e"
    ),
    "effect_capacity_certified_request_overtake_bug.cfg": (
        "156273e8686d77e26d831d184164f4061fe26512efc24f076975eb6d24a39835"
    ),
    "effect_capacity_certified_request_partial_pq_bug.cfg": (
        "f5f81bcb147702596cf90f60501854b9a1bd191dd4192784937f33f7ea6e913b"
    ),
    "effect_capacity_certified_request_substitute_bug.cfg": (
        "f0eb876446f3d56712a27eaa43e651ef1c4709d37e20f16074d25f5b0ce85fcb"
        ),
    "effect_capacity_certified_request_upgrade_barrier_lost_bug.cfg": (
        "69c307645f56571da5ec8b96b5561b241db66f634639dab6b55d427d87902d25"
    ),
    "effect_capacity_certified_response_blocked_bug.cfg": (
        "0c1b416d6b8ec459944d202d221470cdbb4b7d1bb2c7e3ae302161dd026b411a"
    ),
    "effect_capacity_certified_response_byte_reserve_bug.cfg": (
        "dcc3cfbed7fe7318f96db0b165d6ad8d44f114d3988a9b925751bb0e6a5cc987"
    ),
    "effect_capacity_certified_response_count_reserve_bug.cfg": (
        "2c7b0cac92afcbf612577e3af0662bafc1b6803f2d2ba1ac97bb3472d68b9b2f"
    ),
    "effect_capacity_decided_retirement_fixed.cfg": (
        "5a340c3f1bfe9b2f626781994e2b449117f02f3f70ef08a42cb132dfa960de8e"
    ),
    "effect_capacity_full_fetch_hol_bug.cfg": (
        "d55cb48d22f4e0f60b8c0e4e37826cac2e8191ec72da5a4eebbb64c0b25c3487"
    ),
    "effect_capacity_non_fetch_retirement_fixed.cfg": (
        "f14d63f9cb0c78b656d77257b609a98fda05e71cf46db588c73180e03a4261b2"
    ),
    "effect_capacity_outer_transport_chunk_class_bug.cfg": (
        "ed3603ca1a3ad860dca48083650c382d811551b7984b3551b712477ec43fcc18"
    ),
    "effect_capacity_outer_transport_class_fixed.cfg": (
        "c0caace3ece90fcd93f3153997945d963dd3259b9422ba30fdcaa194cd5560ba"
    ),
    "effect_capacity_outer_transport_response_class_bug.cfg": (
        "0657a58fb40a3cb694236a497324500f268e2acc8068700d68131271fc220f55"
    ),
    "effect_capacity_retirement_disabled_bug.cfg": (
        "eab66435ee1e925d6943ef9080c1adc079da2db42ff59fdebb43d9b8b1dbd53e"
    ),
    "effect_capacity_timeout_sign_fixed.cfg": (
        "c873052717fc8e1da6655775e49d7eb63b9376cefa09b83ed05df95c842313af"
    ),
    "effect_capacity_timeout_sign_lost_bug.cfg": (
        "cb08947e12b74abc3778186e81da968430f7fd56efb034a3668bc0e6b2200e29"
    ),
    "effect_capacity_timeout_sign_refill_bug.cfg": (
        "1d0fbde5d0ee2cd5ba75524567126e5c63981732b94cbbac5479cd7a21dc275b"
    ),
    "effect_preemption_decided_victim_bug.cfg": (
        "9e10343b44b3c4297e94729ce56390e1b420764cdf9664131464dde003646e48"
    ),
    "effect_preemption_priority_fixed.cfg": (
        "550fe02c86e36e146e7e2f84099fa85eda0a130c2407db06e1ba0e87b53c85d7"
    ),
    "effect_preemption_wrong_class_bug.cfg": (
        "379a44761f19a7df7aaa0cd114fbb6968a241ab8e34085e3ad8b71317f8f5609"
    ),
    "effect_preemption_wrong_work_id_bug.cfg": (
        "15cded2fae4c33a3276314718793e9a46b1f59571c8f2501f49eae114cc67524"
    ),
    EFFECT_CAPACITY_MUTATION_RUNNER: (
        "c12af944bccd6587374576ecc0abf6779a468ac0162d59b3c6762ab09aab9017"
    ),
}
EFFECT_CAPACITY_MUTATION_FORMAL_GLOBS = (
    "SumeragiV2CertifiedRequestCapacity*.tla",
    "SumeragiV2EffectCapacity*.tla",
    "SumeragiV2EffectPreemption*.tla",
    "SumeragiV2RetainedEffectBatch*.tla",
    "effect_capacity_*.cfg",
    "effect_preemption_*.cfg",
    "effect_batch_*.cfg",
)

# Load the bounded admission and recovery mutation source contracts by path.
