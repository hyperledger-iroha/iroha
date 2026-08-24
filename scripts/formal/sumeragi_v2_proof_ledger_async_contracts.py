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
# This digest binds the current logical pre-split body reconstructed by removing
# every exact shard header/footer in order.  The retained-lock vocabulary lives
# below the shard chain to avoid the debt-to-proof-leaf cycle; its provider and
# bodies are independently pinned by ``_acyclic_liveness_debt_topology_errors``
# before this reviewed global mechanical-body seal is accepted.
ASYNC_LIVENESS_PRE_SPLIT_BODY_SHA256 = (
    "67f49ac8eb83b4227351562d23a614559c385c7540e547dee0d97d51f112f62a"
)
ASYNC_LIVENESS_SHARD_MAX_BYTES = 256 * 1024
ASYNC_LIVENESS_SHARD_MAX_LINES = 5_500
ASYNC_LIVENESS_SHARD_MAX_THEOREMS = 150
# These shards carry the reviewed exact-ingress, producer-continuation, and
# causal-work rank proof seams.  Keep their narrow, source-current ceilings
# explicit so one additional line or theorem still fails instead of silently
# raising the limit for every release shard.
ASYNC_LIVENESS_SHARD_REVIEWED_MAX_LINES = {
    "SumeragiV2AsyncInstallRunnerProofs": 5_879,
    "SumeragiV2AsyncProgressOwnershipProofs": 5_662,
    "SumeragiV2AsyncTemporalRankProofs": 5_514,
}
ASYNC_LIVENESS_SHARD_REVIEWED_MAX_THEOREMS = {
    "SumeragiV2AsyncInstallRunnerProofs": 159,
}
ASYNC_LIVENESS_THEOREM_MAX_LINES = 600
ASYNC_LIVENESS_THEOREM_MAX_STEPS = 256

# The chain/epoch refinement is a second mechanically reconstructed proof
# family.  Its ledger-facing name remains a declaration-free source-name
# facade, while these ordered physical roots keep each TLAPM invocation to at
# most sixteen top-level theorem declarations.  The digest covers the exact
# pre-split body (everything after the original module header and before its
# footer), so changing a cut cannot authorize source drift or reordering.
CHAIN_EPOCH_REFINEMENT_FACADE = "SumeragiV2ChainEpochRefinement"
CHAIN_EPOCH_REFINEMENT_SHARDS = tuple(
    f"SumeragiV2ChainEpochRefinementShard{index:02d}"
    for index in range(1, 17)
)
CHAIN_EPOCH_REFINEMENT_PRE_SPLIT_BODY_SHA256 = (
    "52e132e02c780b7876a99cd9f1a56339ff1090ee3bcc82119bcd11e0876b1545"
)
CHAIN_EPOCH_REFINEMENT_SHARD_MAX_THEOREMS = 16
ASYNC_NETWORK_RELEASE_THEOREMS = (
    'AsyncCandidateServiceStageCarrierHasExactlyElevenClasses',
    'AsyncCandidateServiceStageOrdinalIsBounded',
    'AsyncCandidateServiceTrackedKindProjectionIsCovered',
    'AsyncCandidateLifecycleCapacityDerivesFromReviewedOwners',
    'AsyncCandidateServiceRecordCapacityMatchesConfiguredGeometry',
    'AsyncLeaderWireExactRetryRetainsServiceIdentity',
    'AsyncControlServiceSlotCarrierIsRosterClassBounded',
    'ImportedCertificateTailIgnoresOnlyLocalIncarnation',
    'AsyncCandidateServiceLifecycleStageCollisionCoalesces',
    'AsyncCandidateServiceRecordsInjectIntoLifecycleStageOwners',
    'AsyncCandidateProducerContinuationsInjectIntoLifecycleStageOwners',
    'AsyncControlServiceTableCardinalityIsSlotBounded',
    'RetainedServeAttemptCannotReserveOrAdvanceExactLifecycle',
    'AsyncInternalCertificateSuccessorsCannotRetainFenceCredit',
    'CommandSuccessorsRetainCausalOrigin',
    'AsyncCandidateProducerSemanticHandoffUsesInheritedLifecycle',
    'AsyncCandidateProducerContinuationActiveLogicalOrdinalIsUnique',
    'AsyncCandidateProducerContinuationPostCutSourceIsGloballyIneligible',
    'AsyncCandidateProducerContinuationEligibleSourcesAreMutuallyPreCut',
    'AsyncCandidateProducerContinuationPostCutSourceCannotPrecede',
    'AsyncCandidateProducerContinuationFrozenOwnerPrecedesPostCutReplay',
    'AsyncCandidateProducerContinuationPreCutLogicalOrderIsRetained',
    'AsyncCandidateProducerContinuationEarliestPhysicalSourceIsEligible',
    'AsyncCandidateProducerContinuationPhysicalEligibilityPoolIsFiniteAndNonempty',
    'AsyncCandidateProducerContinuationLogicalOccurrenceRankIsNatural',
    'AsyncCandidateProducerContinuationLogicalPredecessorStrictlyLowersOccurrenceRank',
    'AsyncCandidateProducerContinuationResolutionSelectionIsLogicalMinimum',
    'AsyncCandidateProducerContinuationRuntimeSelectionIsLogicalMinimum',
    'AsyncCandidateProducerContinuationPostCutReplayCannotReserveAhead',
    'AsyncCandidateProducerContinuationSelectedLocalReplayHasReservedCapacity',
    'CandidateProducerContinuationResolutionSplitsReviewedSourceClass',
    'ValidQcIntersectsResponsiveSignerSet',
    'RemoteResponsiveQcSignerIsInFrozenArchiveFanout',
    'PacketForItemExactRetryRetainsRouteIdentity',
    'ReplyRelayPacketPreservesCanonicalSemanticSender',
    'CommitCertificateReplySemanticBindsCanonicalRequester',
    'PersistDecisionConvertsIncompatibleResponseBeforeRetryOrdinal',
    'PersistDecisionPreservesPreFenceResponseUntilCheckedDrain',
    'SignProposalAtomicallyHandsProducerToSourceFanout',
    'AsyncLeaderWireLifecycleSlotUniverseIsFinite',
    'AsyncLeaderWireLifecycleSlotUniverseIsRosterBounded',
    'DormantLeaderWireOwnsNoIngressSchedulerBarrier',
    'AdmitHiddenPacketReservesFreshSharedPhysicalOrdinal',
    'AdmitHiddenLeaderWireIsAtomicLocalAcceptanceCut',
    'AdmitFreshLeaderWireFreezesCurrentLocalSchedulerOrdinal',
    'AdmitDormantLeaderWireRetainsLifecycleTokenAndFrozenPrefix',
    'AdmitDormantLeaderWirePreservesLogicalPotentialPredecessors',
    'AtomicDormantLeaderWireAdmissionConsumesRealPacketWithFreshCarrier',
    'DormantLeaderWirePhysicalOrdinalExhaustionPublishesNothing',
    'AsyncLeaderWireActionInertDormantHasNoExactAdmissionPacket',
    'AsyncLiveServeIngressDuplicateRetainsSchedulerOrdinal',
    'AsyncUnboundChunkAdmissionDoesNotMintLeaderWireLifecycle',
    'AsyncUnboundChunkExactRetryCoalescesWithoutEpisodeGrowth',
    'AsyncHeldChunkReceiptTombstonesExactProducerEpisode',
    'ExactNegativeRetryConsumesNoServeOrPhysicalOrdinal',
    'NonAdvancingServeFamilyRetryConsumesNoFreshOrdinal',
    'SupersededResponseRetryConvertsBeforeFreshOrdinal',
    'AsyncProducerFirstDistinctEpisodeStrictlyConsumesFiniteRank',
    'AsyncProducerFirstDistinctEpisodeForStrictlyConsumesFiniteRank',
    'AsyncProducerExactRetransmissionIsJournalStutter',
    'AsyncProducerExactRetransmissionPreservesTargetIngressRank',
    'CoalescedDueLeaderWireLifecycleRetryPreservesFrozenOwner',
    'AtomicLeaderWireAdmissionFreezesPrefixBeforeAppend',
    'DirectCommitQcCandidateHasExactImportLineage',
    'CommitCertificateResponseCandidateHasExactImportLineage',
    'CommitImportCausalSuccessorRetainsExactLineage',
    'AsyncTimeoutRecoveryDefinedVoteCandidateOwnerIsMember',
    'AsyncTimeoutRecoveryDefinedVoteCandidateHasEpisodeWitness',
    'AsyncTimeoutRecoveryMatchedVoteCandidateBindsFrozenEpisode',
    'AsyncSelectedOrdinaryPhysicalCarrierDefinesIngressScheduler',
    'AsyncCandidateProducerContinuationLaterOrdinalCannotOwnRunnerTurn',
    'AsyncCandidateProducerContinuationScheduledPredecessorOwnsRunnerFirst',
    'AsyncCandidateProducerContinuationLaterThanTimeoutCannotOwnRunnerTurn',
    'AsyncCandidateProducerContinuationPostTimeoutCutCannotOwnRunnerTurn',
    'AsyncCandidateProducerContinuationLaterThanRetransmitCannotOwnRunnerTurn',
    'AsyncCandidateProducerContinuationPostRetransmitCutCannotOwnRunnerTurn',
    'AsyncCandidateProducerContinuationPostCutIngressCannotBlockRunnerTurn',
    'AsyncCandidateProducerContinuationOnlyPreCutIngressCanBlockRunnerTurn',
    'AsyncCandidateProducerContinuationPostCutOrdinaryIngressCannotBlockRunnerTurn',
    'AsyncCandidateProducerContinuationBlockingOrdinaryIngressIsPreCut',
    'AsyncCandidateProducerContinuationRunnerSelectionRespectsIngressCut',
    'AsyncCandidateProducerContinuationRunnerSelectionIsTwoStageLogicalMinimum',
    'AsyncCandidateProducerContinuationRunnerSelectionIsPairwisePhysicalMinimum',
    'AsyncLeaderWireCarrierCannotBypassFrozenPrefix',
    'AsyncOrdinarySelectorPreservesCertifiedResponseBeforeTimeoutVote',
    'AsyncCertifiedFenceEscapeCrossesSelectedServeBarrier',
    'AsyncCertifiedFenceEscapeCrossesMatchingLeaderWireBarrier',
    'AsyncOrdinaryIngressPhysicalOwnerExcludesDifferentCarrier',
    'DormantLeaderWireOwnsNoPhysicalIngressPredecessor',
    'AdmitDormantLeaderWireAppendsAfterExistingServeCarrier',
    'InterruptedTipServeCarrierTerminalizesBeforePhysicalRemoval',
    'InterruptedTipLeaderWireCarrierPublishesTypedRetirement',
    'LeaderWireIngressDrainNeverInventsRuntimeOwner',
    'ServeWorkerDecisionSupersessionClosesWithoutStaleResponse',
    'DeferredRetransmitConsumesDriveProgramCounter',
    'AsyncServeIngressTicketExcludesLaterLocalWork',
    'AsyncLeaderWireIngressTicketExcludesLaterLocalWork',
    'AsyncOrdinaryIngressTicketExcludesLaterLocalWork',
    'AsyncCandidateProducerContinuationExactLocalReplayPublishesStoredCarrier',
    'AsyncCandidateProducerContinuationStoredCarrierMakesSelectedRecordReady',
    'AsyncCandidateProducerContinuationReplayDispatchesOnlyExactIdentity',
    'AsyncOlderCandidateLifecyclePreventsDueTimeoutOvertake',
    'AsyncOlderRetransmitLifecycleCannotAloneBlockDueTimeout',
    'AsyncOlderCandidateLifecyclePreventsDueRetransmitOvertake',
    'AsyncPostRetransmitCutCandidateCannotBlockDueRetransmit',
    'LocalAdmissionAdvanceSelectsAtomicWork',
    'SerializedLocalPrecedesServeIngressExactFrame',
    'AsyncServeIngressTargetOnlyTurnJumpsToIngress',
    'AsyncServeIngressTargetOnlyCannotOvertakeOlderRuntimeLifecycle',
    'AsyncServeIngressTargetOnlyCannotOvertakeOlderLocalLifecycle',
    'AsyncOlderRuntimeInterleaveRetainsServeTicketAndYieldsLocal',
    'SerializedRuntimeIngressExceptionExecutesSelectedOlderLifecycle',
    'SerializedLocalIngressExceptionExecutesSelectedOlderLifecycle',
    'AsyncEarlierIngressLifecyclePreventsDueTimeoutOvertake',
    'AsyncLaterServeTicketInterleavesOlderRuntimeEpisode',
    'AsyncLaterServeTicketInterleavesOlderLocalEpisode',
    'SameHeightRestartPreservesServeHighWatermarks',
    'SameHeightRestartRetainsOrConvertsUnreplacedServeTombstone',
    'SameHeightRestartDischargesTerminalReplayWaiter',
    'SameHeightRestartDischargesEveryLocalServeLifecycle',
    'SameHeightRestartTerminalOutcomeIsIndependentlyReconstructed',
    'ExactNegativeServeRetryIsRejectedBeforeFreshOrdinal',
    'StrictHigherViewMayReplaceNegativeServeFamily',
    'SameHeightRestartReopensActiveLeaderWireWithoutTerminalizing',
    'SameHeightRestartReopensVolatileLeaderWireTerminal',
    'SameHeightRestartRetainsDormantLeaderWireWithoutBarrier',
    'SameHeightRestartPreservesRestartStableLeaderWireTerminal',
    'SameHeightRestartReopensDurableCertifiedResponseFamily',
    'PendingServeReceiverClosePublishesTypedTerminalWithoutDebt',
    'MaterializedServeReceiverClosePublishesTypedTerminalWithoutDebt',
    'RuntimeLeaderWireCannotRetireMerelyFromIngressPop',
    'RetireLeaderWireLifecycleRetainsTerminalTombstone',
    'RetireLeaderWireLifecycleRecoveryCutPrunesOnlyDormant',
    'AsyncGateOpenDueResponsivePacketReentersClockDeadline',
    'AsyncServeProducerTurnMeasureIsFinite',
    'AsyncServeProducerTurnBlocksFreshServeAdmission',
    'AsyncServeCompletionArmsOneShotProducerTurn',
    'AsyncServeProducerTurnRunnerAttemptStrictlyConsumesDebt',
    'AsyncServeProducerTurnRestartPreservesDebt',
    'AsyncTimeoutRecoveryResetRetiresExactlyResetNodes',
    'AsyncCandidateProducerContinuationResetPreservesExactReservation',
    'AsyncCandidateProducerContinuationUnresetOwnerPreserved',
    'AsyncCandidateProducerContinuationRestartStableTerminalPreserved',
    'AsyncCandidateServiceRecordProducersAreTrackedBoundaryKinds',
    'AsyncCandidateUntrackedInternalContinuationAllocatesNoServiceRecord',
    'AsyncCandidateBusyDeferralTransfersSameOwner',
    'AsyncCandidateDeferredHandoffRetainsSameOwner',
    'AsyncCandidateDiscardIsNotSemanticService',
    'ImportedCertificateTailCannotRetireOnLocalIncarnationChange',
    'AsyncCandidateProducerContinuationStepPreservesPhysicalCut',
    'AsyncCandidateProducerContinuationHighWatermarkAdvanceCannotRefreshPhysicalCut',
    'AsyncCandidateProducerContinuationRunnerResolutionRequiresReadyEvidence',
    'AsyncCandidateProducerContinuationExactLocalReplayRetainsReservation',
    'AsyncCandidateProducerContinuationRunnerResolutionConsumesExactStage',
    'AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck',
    'AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor',
    'AsyncCandidateProducerSemanticHandoffRetirementRequiresAck',
    'AsyncCandidateProducerContinuationDecisionReclamationClearsNode',
    'AsyncCandidateLifecycleProducerContinuationCoverageUsesInheritedToken',
    'OrdinaryIngressPreDequeueCarrierHasNoContinuationCut',
    'OrdinaryIngressDrainFreezesContinuationPhysicalCut',
    'OrdinaryIngressLogicalRebasePreservesContinuationPhysicalOwnership',
    'LeaderWireIgnoredOrServicedLastConsumerTerminalizesAtomically',
    'AsyncCandidateProducerContinuationKindPartition',
    'AsyncCandidateProducerContinuationRemovedReplayClassification',
    'AsyncCandidateProducerContinuationZeroSourceMeansNoIngressCarrier',
    'AsyncCandidateProducerContinuationReservationFreezesPhysicalCut',
    'AsyncCandidateProducerContinuationSourcePhysicalOrdinalIsBeforeCut',
    'AsyncCandidateCausalSuccessorInheritsContinuationPhysicalOwnership',
    'LeaderWireDeliveryCandidateInheritsAdmissionSchedulerOrdinal',
    'OrdinaryIngressDeliveryCandidateInheritsCheckedDequeuePhysicalCut',
    'FreshNonIngressCandidateRootSnapshotsCurrentPhysicalCut',
    'AsyncDormantLeaderWireReactivationConsumesPhysicalNotLifecycleOrdinal',
    'AsyncTimeoutRecoveryRetainedEpisodesContainFramedEpisode',
    'AsyncTimeoutRecoveryNodeEpisodeSetIsSingleton',
    'AsyncTimeoutRecoveryIncumbentsCollapseToMatchedEpisode',
    'AsyncTimeoutRecoveryFirstAdmissionHasNoIncumbent',
    'AsyncTimeoutRecoveryCoalescedRetryHasIncumbent',
    'AsyncTimeoutRecoveryNonCandidateCreatesNoAdmission',
    'AsyncTimeoutRecoveryFirstAdmissionCandidateSlotIsRemaining',
    'AsyncTimeoutRecoveryCoalescedRetryCandidateSlotIsAdmitted',
    'AsyncTimeoutRecoveryProducerEpisodeMeasureIsFinite',
    'AsyncTimeoutRecoveryFreshOwnerRemovesExactlyItsRemainingSlot',
    'AsyncTimeoutRecoveryFirstAdmissionConsumesExactlyOneProducerSlot',
    'AsyncTimeoutRecoveryCoalescedRetryPreservesProducerEpisode',
    'AsyncTimeoutRecoveryFreshReplenishmentConsumesFiniteProducerSlot',
    'AsyncTimeoutRecoveryUpdatedEpisodeIsRetainedByAdmissionState',
    'AsyncTimeoutRecoveryEpisodeAfterVoteAdmissionIsStateIndependent',
    'AsyncRetransmitFreshEpisodeConsumesSharedLifecycleOrdinal',
    'AsyncRetransmitFreshEpisodeAdvancesSharedHighWatermark',
    'AsyncRetransmitCompletedEpisodeClearsActiveOwner',
    'AsyncTimeoutRecoverySupersedesOnlyExactPreTimeoutRetransmit',
    'AsyncRetransmitCompletedOwnedEpisodeDefersFreshAcquisition',
    'AsyncRetransmitFreshEpisodeCannotReuseDrainedPosition',
    'AsyncRetransmitFreshLiveEpisodeRetainsSharedLifecycleOrdinal',
    'AsyncRetransmitFreshLiveEpisodeFreezesIngressPhysicalCut',
    'AsyncRetransmitLiveEpisodeRetainsIngressPhysicalCut',
    'AsyncFreshServeReservationPrecedesSameStepRetransmitAllocation',
    'AsyncTargetNeutralLifecycleOwnerCarrierIsFinite',
    'AsyncTargetNeutralLifecycleEpisodeBudgetIsFiniteAndCoalesced',
    'AsyncTargetNeutralLifecycleDiscoveryStrictlyConsumesBudget',
    'AsyncTargetNeutralLifecycleBudgetOrderingIsWellFounded',
    'AsyncCandidateLifecycleReviewedTokenOwnsOneOrigin',
    'AsyncRunnerResolutionStrictlyConsumesFiniteProducerPrefix',
    'LeaderWireRecoveryCutRetainsOrdinalHighwaters',
    'OrdinaryIngressCarrierRetirementCompactionDoesNotIncreaseEvidence',
    'OrdinaryIngressCarrierAdmissionPreservesConfiguredEvidenceBound',
    'AdmitOrdinaryIngressCarrierReservesImmutableActorGlobalOrdinal',
    'ExactOrdinaryIngressDuplicateCoalescesWithoutCarrierAllocation',
    'OrdinaryIngressCarrierIdentityMismatchPublishesNoAdmission',
    'LaterAcceptedOrdinaryCarrierCannotOvertakeFrozenCarrier',
    'BusyDeferredOlderAggregateRebasesToMinimumCompatibleCarrier',
    'BusyDeferredAggregateIdentityMutationCannotRebaseOwner',
    'AsyncControlServiceTransitionConsumesFreshLeaderWireSchedulerOrdinal',
    'AsyncControlServiceTransitionPreservesSemanticHandoffCoverage',
    'AsyncCandidateProducerContinuationStatusTransitionIsMonotone',
    'AsyncControlServiceTransitionPreservesCandidateProducerContinuationLifecycleCoverage',
    'AsyncCandidateLifecycleReviewedBucketsPartitionRecords',
    'AsyncCandidateLifecycleDormantBucketsSeparateReplayAndService',
    'AsyncCandidateLifecycleActiveRecordsInjectIntoPhysicalOwners',
    'AsyncCandidateLifecycleTransientMarkerRetainsItsReservation',
    'AsyncCandidateLifecycleDormantDurableSourceKeepsReservation',
    'AsyncCandidateLifecycleStrictViewCompactsDormantEpisodeRoot',
    'AsyncCandidateLifecycleReviewedBucketsImplyPerNodeCapacity',
    'AsyncCandidateLifecycleSlotInjectionBoundsGlobalOwners',
    'AsyncCandidateLifecycleCapacityCannotBlockOwnedContinuation',
    'AsyncCandidateLifecycleCarrierInjectionProvidesFreshReservations',
    'AsyncCandidateLifecycleDistinctNewRootsReceiveDistinctOwnership',
    'AsyncCandidateLifecycleHighWatermarkAdvancesByFullFreshSet',
    'AsyncOrdinaryIngressSharedHighWatermarkAdvancesAtAcceptance',
    'AsyncServeIngressSharedHighWatermarkAdvancesByFreshTickets',
    'AsyncLeaderWireIngressHighWatermarkAdvancesByFreshAdmissions',
    'AsyncLeaderWireSharedHighWatermarkAdvancesByFreshAdmissions',
    'AsyncLeaderWireAdmissionPrecedesSameStepCandidateAllocation',
    'AsyncServeIngressReservationPrecedesSameStepCandidateAllocation',
    'AsyncCandidateLifecycleFullOrdinaryTableRejectsBeforeSourcePop',
    'AsyncControlServiceTransitionRequiresAtomicLifecycleReservation',
    'AsyncServeIngressAdmissionConsumesSharedSchedulerOrdinal',
    'AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal',
    'AsyncSharedSchedulerHighWatermarkIsMonotone',
    'AsyncSameHeightRestartRetainsSharedSchedulerHighWatermark',
    'AsyncCandidateSchedulerCoverageExposesBoundedProducerOrigin',
    'AsyncIgnoredIngressEpisodeCannotConsumeLifecycleCapacity',
    'AsyncCandidateServiceIdentityIgnoresConsumerIncarnation',
    'AsyncCandidateServiceIdentityDeterminesSemanticStatement',
    'AsyncCandidateCarrierVariantsCoalesceServiceIdentity',
    'AsyncCandidateServiceIdentityIgnoresSchedulerClass',
    'AsyncCandidateLifecycleAndServiceIdentityIgnoreSchedulerClass',
    'AsyncCandidateServiceRouteNeutralResponseRetryIsStable',
    'AsyncCandidateServiceTombstoneCoalescesFreshCandidate',
    'AsyncCandidateInternalBodyAvailableStageRetirementCoalescesFreshCandidate',
    'AsyncCandidateTransientMarkerCoalescesFreshCandidate',
    'AsyncCandidateTerminalTombstoneCoalescesFreshCandidate',
    'AsyncCandidateServiceTombstoneRejectsTransportReadmission',
    'AsyncCandidateSuccessfulServiceInstallsTransientMarker',
    'AsyncCandidateSuccessfulServiceInstallsTombstone',
    'AsyncCandidateSuccessfulServiceAllocatesExactOrdinal',
    'AsyncCandidateTransientMarkerPersistsWithinGeneration',
    'AsyncCandidateServicedMarkerPersistsWithoutExit',
    'AsyncCandidateTerminalTombstonePersistsWithoutExit',
    'AsyncCandidateSameHeightRestartPreservesServicedIdentity',
    'AsyncCandidateSameHeightRestartPreservesTombstone',
    'AsyncCandidateTransientMarkerDoesNotSuppressRestartReplay',
    'AsyncRestartScopedCandidateIsNeverReplayTombstoned',
    'AsyncCandidateResponsiveRestartPermitsNonterminalReconstruction',
    'AsyncCandidateStrictViewAdvanceReclaimsOlderTombstones',
    'AsyncCandidateDecisionReclaimsNodeTombstones',
    'AsyncCandidateTombstoneSubsetIsBoundedByFrozenOwnerCarrier',
    'AsyncCandidateTombstonesAreBoundedByFrozenOwnerCarrier',
    'AsyncControlServiceExactRetryCoalesces',
    'AsyncControlServiceSameOrLowerViewCannotReplace',
    'AsyncControlServiceLivePredecessorBlocksStrictlyNewerAdmission',
    'AsyncControlServiceConsumedPredecessorAllowsStrictlyNewerReplacement',
    'AsyncControlServiceLivePredecessorClosesFreshAdmissionGate',
    'AsyncControlServiceBlockedNewerPacketCannotPassIngress',
    'AsyncControlServiceReplacementIsStrictlyNewer',
    'AsyncControlServiceConsumedBitIsMonotoneWithoutReplacement',
    'AsyncControlServiceConsumedOccurrenceIsRetired',
    'AsyncControlServiceConsumedIdentityCannotReactivate',
    'AsyncControlServiceServicedIdentityCannotResurrect',
    'AsyncControlServiceTombstoneCannotReactivate',
    'AsyncControlServiceSameHeightRecoveryRetiresVolatileOwners',
    'CertifiedResponseClaimAdmissionAllocatesExactOrdinal',
    'CertifiedResponseExactRetryKeepsOneClaimOrdinal',
    'CertifiedResponseConsumedFamilyCannotRetainClaim',
    'CertifiedResponseSameHeightRecoveryReopensDurableFamily',
    'PostGstLeaderWireLifecycleRestartIsDisabled',
    'LeaderWireIngressAdmissionRefinesLifecycleTransition',
    'LeaderWireIngressDrainRefinesLifecycleTransition',
    'LeaderWireLastConsumerRefinesLifecycleTransition',
    'LeaderWireTerminalRetirementRefinesLifecycleTransition',
    'LeaderWireRestartReopenRefinesLifecycleTransition',
    'AsyncTimeoutVoteFairIngressDrainLeavesCoreState',
    'AsyncPostGstHasNoControlServiceReset',
    'AsyncTimeoutRecoveryEpisodeCurrentBoundaryForNode',
    'AsyncUnchangedCoreStatePreservesTimeoutBoundary',
    'AsyncTimeoutVoteIngressDrainRetainsCurrentEpisodeBoundary',
    'AsyncUnchangedCoreStateExcludesPersistInstall',
    'AsyncFairIngressDrainPreservesRetransmitTimerState',
    'AsyncFairIngressDrainExcludesDirectRetransmit',
    'AsyncTypedOutstandingTagRemovalChangesFunction',
    'AsyncDeferredRetransmitRemovesOutstandingTag',
    'AsyncFairIngressDrainExcludesDeferredRetransmit',
    'AsyncIngressDrainDoesNotCompleteRetransmitLifecycle',
    'AsyncIngressDrainFramesDeferredAndCausalQueues',
    'AsyncTimeoutVoteFairIngressFramesCommandAndWork',
    'AsyncTimeoutVoteIngressDrainFramesSchedulerCarriers',
    'AsyncSequenceSetAfterAppendAddsOnlyValue',
    'AsyncUnionOfSequenceSetsAfterAppendAtAnyKeyAddsOnlyValue',
    'AsyncTimeoutVoteIngressDrainAddsOnlyDeliveryOrigin',
    'AsyncProposedTimeoutCausalOriginHasBeginTimeoutPhase',
    'AsyncOwnedTimeoutLifecycleOriginHasBeginTimeoutPhase',
    'AsyncCurrentTimeoutCausalOriginUsesEffectiveOrigin',
    'AsyncOwnedTimeoutRecoveryCurrentOriginHasBeginTimeoutPhase',
    'AsyncDeliveryCandidateOriginPhaseEqualsDeliveryKind',
    'AsyncTimeoutVoteDeliveryOriginHasDistinctPhase',
    'AsyncTimeoutVoteIngressDrainDoesNotTransferTimeoutLifecycle',
    'AsyncTimeoutVoteIngressDrainEstablishesRecoveryFrame',
    'AsyncTimeoutVoteFairIngressDrainFramesRecoveryEpisode',
    'AsyncControlServiceSlotTransitionPublishesTimeoutRecoveryVoteState',
    'AsyncTimeoutRecoveryVoteAdmissionRetainsUpdatedEpisodeAcrossSlotTransition',
    'AsyncRetainedCommitQcRetransmissionCreatesExactPacket',
    'PostGstStepCannotCreateDormantLeaderWirePotential',
    'AsyncNextProjectsMonotoneProducerJournal',
    'AsyncIngressPhysicalHighWatermarkIsMonotone',
    'AsyncServeAdmissionHighWatermarkIsMonotone',
    'AsyncFixedCorridorDeadlineActionErasureIsExact',
    'AsyncOriginalStepHasFixedCorridorDeadlineReceiptExtension',
    'AsyncServiceActivationActionsRefineAsyncNext',
    'AsyncTickDoesNotFreezeClockLifecycles',
    'AsyncTickDoesNotAcquireFreshClockOwnership',
    'AsyncOrdinaryIngressDoesNotFreezeClockLifecycles',
    'AsyncNonAdmittingIngressBranchesDoNotFreezeClockLifecycles',
    'AsyncIngressPacketFreezesOnlyAtFreshExactServeReservation',
    'AsyncTimeoutLifecycleFreezeBoundaryMintsAfterPriorAdmissions',
    'AsyncTimeoutLifecycleOrdinalPersistsUntilEndpoint',
    'AsyncTimeoutLifecycleOrdinalClearsOnlyAtEndpoint',
    'AsyncRetransmitLifecycleFreezeBoundaryMintsAfterPriorAdmissions',
    'AsyncRetransmitLifecycleOwnerAndPhysicalCutPersistUntilEndpoint',
    'AsyncRetransmitLifecycleOwnerAndPhysicalCutClearAtEndpoint',
    'AsyncTimeoutLifecycleNewOwnershipUsesRecordedOrFreshOrdinal',
    'AsyncNextNeverSchedulesAnUnownedCandidateLifecycle',
    'AsyncCandidateServicesThisStepIsSingleton',
    'AsyncCandidateTerminalRetirementsThisStepIsSingleton',
    'AsyncCandidateDiscardInstallsTerminalTombstone',
    'AsyncCandidateTerminalDiscardAllocatesExactOrdinal',
    'AsyncCandidateDiscardRetiresLogicalLifecycle',
    'AsyncCandidateInternalBodyAvailableStageRetirementIsMonotoneAtGst',
    'AsyncCandidateInternalBodyAvailableServiceIdentityCannotReactivateAtGst',
    'AsyncCandidateAdmissionIdentityObsolescenceIsMonotoneAtGst',
    'AsyncCandidateObsoleteAdmissionIdentityCannotReappearAtGst',
    'AsyncCandidateTerminalIdentityCannotReactivateAtGst',
    'AsyncCandidateSameGenerationServicedIdentityCannotReactivateAtGst',
    'AsyncCandidateSameGenerationSuccessfulServiceIdentityPersistsUntilStrictExit',
    'AsyncCandidateServicedIdentityCannotReactivate',
    'AsyncActiveControlServiceAdmissionPassesSlotGuard',
    'AsyncRetiredControlServiceAdmissionDropsWithoutCandidate',
    'PostGstExactDormantLeaderWireAdmissionUsesFairAtomicAction',
    'AsyncControlServiceRolloverInstanceStartsEmpty',
    'AsyncLeaderWireLifecycleRolloverInstanceStartsEmpty',
    'AsyncTimeoutRecoveryRolloverInstanceStartsEmpty',
    'AsyncCertifiedResponseClaimRolloverInstanceStartsEmpty',
    'AsyncCandidateServiceRolloverInstanceStartsEmpty',
    'AsyncInitEstablishesLeaderWireContinuationSharedOrdinalNoCollision',
    'AsyncCandidateLifecycleRolloverStartsWithRootOwners',
    'AsyncInitEstablishesCandidateProducerContinuationLocalReplayCapacity',
    'AsyncResponsiveRestartPreservesCandidateProducerContinuationLocalReplayCapacity',
    'AsyncCandidateDeparturePreservesProducerContinuationLocalReplayCapacity',
    'AsyncNextPreservesCandidateProducerContinuationLocalReplayCapacity',
    'AsyncBracketNextPreservesCandidateProducerContinuationLocalReplayCapacity',
    'AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity',
    'AsyncLiveSpecUsesRepresentativePeerCount',
    'AsyncFiniteLiveSpecUsesRepresentativePeerCount',
    'AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain',
    'AsyncFreshServeIngressSchedulerOrdinalInjectsAgainstPriorOwners',
    'SameHeightRestartCanonicalDischargeKeysAreLifecycleStable',
    'AsyncServeQueuedIdentityDepartureInstallsTombstone',
    'AsyncServeRetiredIdentityCannotRequeueAtGst',
    'AsyncServeTombstonedIdentityCannotRequeueAtGst',
    'AsyncServeIngressSchedulerOrdinalIsTyped',
    'AsyncServeIngressSharedSchedulerInitIsEmptyAndInjected',
    'AsyncCandidateCausalAdmissionTransfersSameOwner',
    'AsyncCandidateIoCompletionTransfersSameOwner',
    'AsyncCandidateProducerCompletionTransfersSameOwner',
    'AsyncCandidateLifecyclePhysicalTokensCoverScheduledOriginsAfter',
    'AsyncCandidateLifecycleDurableTokensCoverReplayOriginsAfter',
    'AsyncCandidateLifecycleDurableOwnerCarrierIsBounded',
    'AsyncCandidateLifecycleServiceOwnerCarrierIsSlotBounded',
    'AsyncCandidateLifecyclePhysicalAndDurableOwnersFitActiveSlots',
    'AsyncCandidateLifecycleCompactedStateHasSemanticOwnerCoverage',
    'AsyncCandidateLifecycleCompactedStateHasActiveOwnerCoverage',
    'AsyncCandidateLifecycleSemanticCoverageGivesOwnerInjection',
    'AsyncCandidateLifecycleActiveCoverageGivesOwnerInjection',
    'AsyncCandidateLifecycleReviewedSemanticOwnersFitOrdinaryCapacity',
    'AsyncCandidateLifecycleReviewedOwnerInjectionProvidesReservations',
    'AsyncCandidateLifecycleCompactedStateProvidesFreshReservations',
    'AsyncRetransmitProgramCounterIsBounded',
    'CertifiedResponseCompetingResponderCannotDoubleChargeFamily',
    'CertifiedResponseClaimFrozenLifecycleSourceIsPhysicallyPreCut',
    'CertifiedResponseClaimsShareOutstandingRequestCharge',
    'CertifiedResponseFamilyLocalClaimsRemainPhysicallySerialized',
    'LeaderWireIngressDrainFreezesContinuationPhysicalCut',
    'AsyncSelectedLeaderWirePhysicalCarrierDefinesIngressScheduler',
    'AsyncFreshLeaderWireAdmissionProjectionFollowsRetainedOwners',
    'LeaderWirePreDequeueCarrierHasNoContinuationCut',
    'DormantLeaderWireReactivationPublishesOneFreshPhysicalCarrier',
    'AsyncLeaderWirePotentialPredecessorUniverseIsFinite',
    'AsyncProoflessChunkEpisodeBudgetIsFiniteAndCoalesced',
    'AsyncNextNodeCommandOwnsOldestLifecycleOrdinal',
    'AsyncNextDeferredCommandOwnsOldestLifecycleWithoutHandoff',
    'AsyncDeferredHandoffRetainsExactSelectedLifecycle',
    'AsyncRetainedCommitQcPacketAdmissionCreatesExactIngressOwner',
    'AsyncRetainedCommitQcIngressCreatesExactDeliverQcOwner',
    'AsyncRetainedCommitQcDeliveryRecordsExactReceipt',
    'AsyncNextPreservesCandidateProducerContinuationScheduledExclusion',
    'CertifiedResponseClaimAdmissionMatchesPostStateLifecycleCarrier',
    'CertifiedResponseClaimAdmissionFreezesCompletePredecessorSources',
    'CertifiedResponseLiveClaimCannotBeReplacedAtGst',
    'AsyncNextPreservesLeaderWireContinuationSharedOrdinalNoCollision',
    'AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst',
    'CertifiedResponseClaimNewTimeoutSourceIsExcludedOrAboveFrozenCeiling',
)
ASYNC_LIVENESS_SHARDS = (
    ("SumeragiV2AsyncRankAndInitProofs", "AsyncInitEstablishesIngressType"),
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
    ("SumeragiV2AsyncRecoveryVoteEpochBoundaryContinuationProofs", None),
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
    "SumeragiV2AsyncDecisionApplicationProofs": (
        "SumeragiV2AsyncTimeoutOwnershipProofs",
    ),
    "SumeragiV2AsyncTemporalRankProofs": (
        "SumeragiV2AsyncRecoveryProgressWitnessProofs",
        "SumeragiV2AsyncCausalWorkBudgetProofs",
    ),
    "SumeragiV2AsyncDeadlockProofs": (
        "SumeragiV2AsyncFiniteRunnerEpisodeProofs",
        "SumeragiV2AsyncCandidateProducerContinuationProofs",
    ),
}


def _mechanical_shard_source_prefix(
    module: str,
    index: int,
    shards: tuple[str, ...],
    *,
    extends_overrides: dict[str, tuple[str, ...]] | None = None,
    multiline_extends: frozenset[str] = frozenset(),
) -> str:
    """Return exact header/import framing for an ordered mechanical shard."""

    header = f"---- MODULE {module} ----\n"
    if index == 0:
        return header
    overrides = extends_overrides or {}
    dependencies = overrides.get(
        module, (shards[index - 1],)
    )
    if module in multiline_extends:
        return header + "EXTENDS " + ",\n        ".join(dependencies) + "\n\n"
    return header + f"EXTENDS {', '.join(dependencies)}\n\n"


def _mechanical_shard_bodies(
    sources: dict[str, str],
    shards: tuple[str, ...],
    prefix_for: Any,
    *,
    family: str,
) -> tuple[list[str], list[str]]:
    """Strip exact mechanical framing, failing closed on any family drift."""

    bodies: list[str] = []
    errors: list[str] = []
    footer = "=============================================================================\n"
    for index, module in enumerate(shards):
        source = sources.get(module)
        if source is None:
            errors.append(f"missing required {family} shard {module}.tla")
            continue
        prefix = prefix_for(module, index)
        prefix_matches = source.startswith(prefix)
        footer_matches = source.endswith(footer)
        if not prefix_matches:
            errors.append(
                f"{module}.tla must start with its exact reviewed {family} shard prefix"
            )
        if not footer_matches:
            errors.append(
                f"{module}.tla must end with its exact reviewed {family} shard footer"
            )
        if prefix_matches and footer_matches:
            bodies.append(source[len(prefix) : -len(footer)])
    return bodies, errors


def _async_liveness_shard_source_prefix(module: str, index: int) -> str:
    """Return the exact reviewed header/import prefix for one proof shard."""

    return _mechanical_shard_source_prefix(
        module,
        index,
        tuple(item[0] for item in ASYNC_LIVENESS_SHARDS),
        extends_overrides=ASYNC_LIVENESS_EXTENDS_OVERRIDES,
        multiline_extends=frozenset({"SumeragiV2AsyncTemporalRankProofs"}),
    )


def _async_liveness_shard_bodies(
    sources: dict[str, str],
) -> tuple[list[str], list[str]]:
    """Strip exact reviewed shard framing, failing closed on any drift."""

    return _mechanical_shard_bodies(
        sources,
        tuple(item[0] for item in ASYNC_LIVENESS_SHARDS),
        _async_liveness_shard_source_prefix,
        family="async liveness",
    )


def _chain_epoch_refinement_shard_source_prefix(module: str, index: int) -> str:
    """Return exact framing for one physical chain/epoch refinement shard."""

    return _mechanical_shard_source_prefix(
        module,
        index,
        CHAIN_EPOCH_REFINEMENT_SHARDS,
    )


def _chain_epoch_refinement_shard_bodies(
    sources: dict[str, str],
) -> tuple[list[str], list[str]]:
    """Strip exact chain/epoch shard framing, failing closed on any drift."""

    return _mechanical_shard_bodies(
        sources,
        CHAIN_EPOCH_REFINEMENT_SHARDS,
        _chain_epoch_refinement_shard_source_prefix,
        family="chain/epoch refinement",
    )


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
        "00d100b5d15a29984794367d88c0de070207276168cd980c22faa92b91029a8e"
    ),
}

# Exact source seal for the finite exact-ingress and adequate-leader ownership
# mutation corpus.  These TLC pairs are regression evidence only: they may
# expose a repaired transition or liveness rank, but never promote a bounded
# model-checking result to deductive proof status.
LIVENESS_OWNERSHIP_MUTATION_FORMAL_ARTIFACTS = (
    "SumeragiV2Revision4CertifiedFenceReservation.tla",
    "revision4_certified_fence_reservation_fixed.cfg",
    "revision4_certified_fence_reservation_blocked_bug.cfg",
    "revision4_certified_fence_reservation_arrival_order_bug.cfg",
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
        "d392e465b526ffbb4b4aac0857582098a5c260d86db94dab9319f2cb8f35daaa"
    ),
}
SERVE_SCHEDULER_ORDINAL_RUNNER_SECTION_SHA256 = (
    (
        "configuration and shared result-contract binding",
        'readonly REPO_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"',
        "if (($#)); then",
        "7ac3d1b857f36ed39d8e85247e3e083aebe574b7e7ba9fadc2aa3ad3d3989e4a",
    ),
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
        "2dfa7f1222e5421c1a38bfe7481b140e8962df6637bf30dab9b272ef9471aad9"
    ),
    "SumeragiV2AsyncRankAndInitProofs.tla": (
        "c6f6eade349f107e0572cb381690ad054b61493ef274a9127eed247f4f7b75ca"
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
        "101133a786105b5e2bfd24e7f41192fd2d16b938ddc43f1b7314381c153b687f"
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
        "75466cd1d485d26237931a11ad62ba9f277f63fb162d2a49648e22731a2a420e"
    ),
}
COMMIT_IMPORT_PROVENANCE_MUTATION_FORMAL_GLOBS = (
    "SumeragiV2CommitImportProvenanceMutation*.tla",
    "commit_import_provenance_*.cfg",
    "commit_import_successor_*.cfg",
)
COMMIT_IMPORT_PROVENANCE_RELEASE_SOURCE_SHA256 = {
    "SumeragiV2AsyncNetwork.tla": (
        "2dfa7f1222e5421c1a38bfe7481b140e8962df6637bf30dab9b272ef9471aad9"
    ),
    "SumeragiV2HistoricalRecoveryTemporalClosureProofs.tla": (
        "fac37875375b831db6ed1bd1f64e194fd5e7eebde51019c345a85c4083664752"
    ),
}

COMMIT_IMPORT_RELEASE_SUPPORTING_THEOREMS = (
    (
        "SumeragiV2AsyncNetwork",
        "DirectCommitQcCandidateHasExactImportLineage",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "CommitCertificateResponseCandidateHasExactImportLineage",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "CommitImportCausalSuccessorRetainsExactLineage",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesHistoricalCertificateLocalImportCandidateEntry",
    ),
)


def _commit_import_release_statement_errors(formal_dir: Path) -> list[str]:
    """Keep Commit-import release statements exact after any digest refresh."""

    errors: list[str] = []
    for module, symbol in COMMIT_IMPORT_RELEASE_SUPPORTING_THEOREMS:
        path = formal_dir / f"{module}.tla"
        if not path.is_file() or path.is_symlink():
            continue
        source = path.read_text(encoding="utf-8")
        extracted = _top_level_theorem_body(
            source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(f"{path}: missing Commit-import release theorem {symbol}")
            continue
        body, line = extracted
        statement = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
            body,
            maxsplit=1,
        )[0]
        normalized = " ".join(statement.split())
        expected = EXACT_FIXED_PROOF_SUPPORTING_THEOREM_STATEMENTS[(module, symbol)]
        if normalized != expected:
            errors.append(
                f"{path}:{line}: Commit-import release theorem {symbol} "
                f"must state only {expected!r}; found {normalized!r}"
            )
    return errors

_FORMAL_CI_NEW_MUTATION_RUNNER_INVOCATIONS = (
    "run_formal_script scripts/formal/run_sumeragi_v2_indexed_service_activation_mutations.sh",
    "run_formal_script scripts/formal/run_sumeragi_v2_adequate_leader_readiness_mutations.sh",
    "run_formal_script scripts/formal/run_sumeragi_v2_indexed_height_mutation.sh",
    "run_formal_script scripts/formal/run_sumeragi_v2_item_carrier_typing_mutation.sh",
    "run_formal_script scripts/formal/run_sumeragi_v2_reply_writer_deadline_mutations.sh",
)
SHARED_TLC_RESULT_CONTRACT = (
    "scripts/formal/sumeragi_v2_tlc_result_contract.sh"
)
SHARED_TLC_RESULT_CONTRACT_CALLERS = (
    "scripts/formal/run_sumeragi_v2_adequate_leader_readiness_mutations.sh",
    "scripts/formal/run_sumeragi_v2_applied_phase_admission_mutations.sh",
    "scripts/formal/run_sumeragi_v2_durable_validate_lifecycle_mutations.sh",
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
    "scripts/formal/run_sumeragi_v2_item_carrier_typing_mutation.sh",
    "scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh",
    "scripts/formal/run_sumeragi_v2_tlc.sh",
)
SHARED_TLC_RESULT_CONTRACT_SHA256 = {
    SHARED_TLC_RESULT_CONTRACT: (
        "f74b5f668830daf9f434fb8ad3234cac13ffafd44a3e8341841d616a27ec03ea"
    ),
    "scripts/formal/run_sumeragi_v2_adequate_leader_readiness_mutations.sh": (
        "47175c5815cb5073b5ef97cc5f23d2609d15f4cf6caae6068633250c648d79ba"
    ),
    "scripts/formal/run_sumeragi_v2_applied_phase_admission_mutations.sh": (
        "d163bc19b62ed9f23e4f08433a7bb8080407cb8d31b9666ccf5c5d9491034ca6"
    ),
    "scripts/formal/run_sumeragi_v2_durable_validate_lifecycle_mutations.sh": (
        "43eeb61545f5c9caa5365ee97f61fed4f56cd574ac09ca70d848d41b5ca36f6e"
    ),
    "scripts/formal/run_sumeragi_v2_apply_authority_mutation.sh": (
        "a69da1a117bb8e6f3a73d5c6ce99eb88286b1372b59238e0da2ccf555eeafb0e"
    ),
    "scripts/formal/run_sumeragi_v2_candidate_restart_mutation.sh": (
        "1400aee6798b10b0cba2d33663c39a4a36b65e009e5ae070896c0b2dd10b199a"
    ),
    "scripts/formal/run_sumeragi_v2_certificate_ref_recovery_mutation.sh": (
        "2c88484904aa3fb104ea8d33cc9d7ba53859fb51c50ad2aeb30cd0e35fb506b5"
    ),
    "scripts/formal/run_sumeragi_v2_certified_response_identity_separation_mutation.sh": (
        "98aabccfa0af05a5090bb344adcd69115e107d5d3ecfcae5b22c62643793d79d"
    ),
    "scripts/formal/run_sumeragi_v2_certified_response_registration_mutation.sh": (
        "97f30ef35ce90e5a67193da0cdbdcc13c3de2f1c2535fd50ecdd081a697313a5"
    ),
    "scripts/formal/run_sumeragi_v2_certified_response_source_lineage_mutation.sh": (
        "6d465561af8d5f95e56aa92f0fda37cfbeea660c8d05aa2846f6eef0f234001a"
    ),
    "scripts/formal/run_sumeragi_v2_commit_import_provenance_mutations.sh": (
        "75466cd1d485d26237931a11ad62ba9f277f63fb162d2a49648e22731a2a420e"
    ),
    "scripts/formal/run_sumeragi_v2_decision_recovery_lifecycle_mutation.sh": (
        "aab9015dc476a567d81ce7ddc4f217578825da06a9cfc713f35186386459feeb"
    ),
    "scripts/formal/run_sumeragi_v2_effect_capacity_ownership_mutation.sh": (
        "98193358163d37c5169aa777ca043021cb7b1261fb34493563dc0307ddcf6a84"
    ),
    "scripts/formal/run_sumeragi_v2_historical_discovery_occurrence_rank_mutation.sh": (
        "96e2a255b699b3d60ba4fcd66cd212e7a8958adde9551b3fffc32118e9e31e02"
    ),
    "scripts/formal/run_sumeragi_v2_indexed_height_mutation.sh": (
        "6c666896aa2d602965cc91ad7e4dcdb5112b3a7c0dadd14dc3f3c54c1bde8258"
    ),
    "scripts/formal/run_sumeragi_v2_indexed_service_activation_mutations.sh": (
        "a167d46e931ebe08e8933c27a1cc2efb26898cd7fd5dbeb2b006924c590d31f6"
    ),
    "scripts/formal/run_sumeragi_v2_inflight_first_release.sh": (
        "91ed2f7ef62c446dfcc08836aeacf151f4f75464858be540e418b8d45f11a35d"
    ),
    "scripts/formal/run_sumeragi_v2_ingress_causal_freshness_mutation.sh": (
        "1cd0bee3b981d6a0cc814587543b82c02ef6d220ae92a12c2afc55e0faf4032a"
    ),
    "scripts/formal/run_sumeragi_v2_item_carrier_typing_mutation.sh": (
        "945fea6569cfe901286abc6cd3736fb78eb4bf82d73ca9d079d2f21c4f379753"
    ),
    "scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh": (
        "c3427e8773ca3955fd0a81e3d17787230ca19a4672e42fea69c2ea4ec395a04a"
    ),
    "scripts/formal/run_sumeragi_v2_multilane_mutations.sh": (
        "3c9bb248f68cc4d9106cdadedaf3d6b1ef82a265c2e76e5dfc7c85cc3e59ad05"
    ),
    "scripts/formal/run_sumeragi_v2_persist_install_generation_mutation.sh": (
        "1391403cbf0f1b64f78bee9ba6ccf5ee043d157a49e33cbaec7ece949633ae39"
    ),
    "scripts/formal/run_sumeragi_v2_persist_install_validation_mutation.sh": (
        "1b397e9c0a869e73b2b78b20b8839714c1f257ce7ad06be1587f554d767f04b2"
    ),
    "scripts/formal/run_sumeragi_v2_post_decision_timeout_mutation.sh": (
        "ff5d34f872f1250f98e11945781cc2cc25d17c3870d7fb415d0636dab7a76c1b"
    ),
    PRODUCER_CONTINUATION_PHYSICAL_CUT_MUTATION_RUNNER: (
        "101133a786105b5e2bfd24e7f41192fd2d16b938ddc43f1b7314381c153b687f"
    ),
    "scripts/formal/run_sumeragi_v2_productive_mutation.sh": (
        "b0f7ef06fa72f635e36c9e360b7409c8cd3c29576ab826702671e3e4b612e3b3"
    ),
    "scripts/formal/run_sumeragi_v2_progress_mutations.sh": (
        "62dbd78aa70b0d7e2682dd052e3eaeba782c4061a4a28b4ebb5fc1fc7b2052b9"
    ),
    "scripts/formal/run_sumeragi_v2_replay_locked_body_carrier_mutation.sh": (
        "051fb1a64a311fc881c2770af7d98c669685679f86dda5d66534d2c78771fdc1"
    ),
    "scripts/formal/run_sumeragi_v2_reply_writer_deadline_mutations.sh": (
        "bd50f7a1923a0f3afa6038cbb1c8f9f3d654b32f596038852735ea35e1e4f74a"
    ),
    "scripts/formal/run_sumeragi_v2_restart_locked_fetch_order_mutation.sh": (
        "686fefc387ec254d9eca2b2fa0f99860fb97f16a580273fe074ba571b7168dcd"
    ),
    "scripts/formal/run_sumeragi_v2_serve_scheduler_ordinal_mutations.sh": (
        "d392e465b526ffbb4b4aac0857582098a5c260d86db94dab9319f2cb8f35daaa"
    ),
    "scripts/formal/run_sumeragi_v2_service_rank_mutation.sh": (
        "41a0c3d8cfda9400fcdd26e27cabbbeeeae4e06f13fd9bb5819f38af50fe6a9e"
    ),
    "scripts/formal/run_sumeragi_v2_tlc.sh": (
        "d046eb57d9a2a99dfc318980707614f96058fc3f5143becdce6a7b9cf3822511"
    ),
    "scripts/formal/run_sumeragi_v2_typed_rollover_handoff_mutations.sh": (
        "c8baa9b718c40c26062b2a54107da0ae46f20ae333304f386be08e5dc40b7645"
    ),
}
SHARED_TLC_RESULT_BRANCH_PROFILES = {
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
    "scripts/formal/run_sumeragi_v2_durable_validate_lifecycle_mutations.sh": (
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
        31,
        89,
        0,
        4,
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
        "scripts/formal/run_sumeragi_v2_applied_phase_admission_mutations.sh": (
            1,
            1,
            1,
            1,
            1,
        ),
        "scripts/formal/run_sumeragi_v2_durable_validate_lifecycle_mutations.sh": (
            1,
            1,
            1,
            1,
            1,
        ),
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
    "7d61a1a77e8289a2a59d7a614349a7a4b5ee9fb4e92a0d926b74e99aecf14640"
)
SHARED_TLC_RESULT_ASSERTION_SITE_PROFILES_SHA256 = (
    "1f802a03156468b1edd273fe1048db5be71c46c83cfa56b6535fa0131d286d71"
)
LIVENESS_OWNERSHIP_MUTATION_SHA256 = {
    "SumeragiV2Revision4CertifiedFenceReservation.tla": (
        "484545fdd06b3b60deccd195ee4a8b8c766c70126af405764957b1029a0f7b00"
    ),
    "revision4_certified_fence_reservation_fixed.cfg": (
        "1daca6e3e7db42e4665e7664a941fe80525ab737d1a6b30c479f2c76e7016039"
    ),
    "revision4_certified_fence_reservation_blocked_bug.cfg": (
        "8e204663a764ca03afdee20dad96060bda28d16d372ce8c8dd5eb7e61034944e"
    ),
    "revision4_certified_fence_reservation_arrival_order_bug.cfg": (
        "0f1408e6ecee28eae2765394cef118755a0a4e3ba94017fb250b28477a573657"
    ),
    "SumeragiV2LocalIngressSchedulerReservationMutation.tla": (
        "678d739d45ebb6b6d5dee70a56f8396e7d75ed954f03faf49252d112f5f4a194"
    ),
    "SumeragiV2RestartTerminalDurabilityMutation.tla": (
        "fee6d70c01d87567b150fd8aee76d2e764cb4b58ab400390a3b7e6a0e5ea765a"
    ),
    "SumeragiV2ExactIngressTicketPriorityMutation.tla": (
        "8a68a5945ac65ea7d57bcbca90cac552bfb9a073eb26b6cd7b3bc328d566c376"
    ),
    "SumeragiV2ExactServeRestartTombstoneMutation.tla": (
        "d42e74fd85d0c64193b1afb474cc55f826ad01182fe59763d2edae43db7cd4a0"
    ),
    "SumeragiV2ExactResponseClaimLifecycleMutation.tla": (
        "6ad54d19000f76931935cc103b509c7073b9a82c2dc6ebe8f31b664c269240b2"
    ),
    "SumeragiV2ExactServeFrozenPredecessorMutation.tla": (
        "794ae1da2fa36db865c756180bad6e1e1c06ccdaa778ff067ece7e9a4ee38ada"
    ),
    "SumeragiV2ExactInstalledTcRetentionMutation.tla": (
        "6a67f962a092c40159e0be2e8a07f47661aecbfe05c9ba2827e55a7e93394389"
    ),
    "SumeragiV2ControlLivePredecessorMutation.tla": (
        "b0f119d1649abeb44e0f8ae0448438fedaac800a8a335230d1130964cca271bc"
    ),
    "SumeragiV2ImportedCertificateTailMutation.tla": (
        "752b4cd4165e56f02637bfe1afdda95d308089532221825c909ae82ee71e2d41"
    ),
    "SumeragiV2ImportedTcTailMutation.tla": (
        "04ca7d6f4f1ffec5cac9f15acb610ba69d98675efc062eab5d026ba6c3b49a1a"
    ),
    "SumeragiV2TimeoutLifecycleStageClassifierMutation.tla": (
        "273154c42c51175197e17060ae6ef1517002f8ff7778841dd5b8e2d4371a7a81"
    ),
    "SumeragiV2PersistInstallTimeoutTagMutation.tla": (
        "13fc590b72d0b97bac3dbabb1ba1b66ca105d1ed8fa4d01e66186d0aca19aa8a"
    ),
    "SumeragiV2PersistInstallTimeoutRootRetirementMutation.tla": (
        "6a5dfddb0c8cae062a4152fa235865061c250b54d704e6b3046822747f5d2925"
    ),
    "SumeragiV2AdequateLeaderWireTombstoneMutation.tla": (
        "d8e07f8473672e7600ca8081abe94376bb9f52acdbd42ef484f893bd23bab4be"
    ),
    "SumeragiV2AdequateLeaderCandidateTombstoneMutation.tla": (
        "75213093f15e29e35ccada58bc543e90ff1240c7ca172e54884516bac6750f17"
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
        "6d0fce697a044d8acf984ae395f64c4bfb78f532e5258589803fdf7d310fa8f3"
    ),
    "imported_certificate_tail_bug.cfg": (
        "cb2f1d16487b030556dc197c72bed912e28b1a8a13462f14e3db54be163ea87b"
    ),
    "imported_tc_tail_fixed.cfg": (
        "d464df4ba82163bce917ebb5ca9c1d84fa61c529be73115570ea3c9ba7ba16ea"
    ),
    "imported_tc_tail_bug.cfg": (
        "305c09791e706de8f7068822e30f527b43789e15971df234068cecc99ad46b46"
    ),
    "timeout_lifecycle_stage_classifier_fixed.cfg": (
        "5a1008c92a2015bdf39cedb566064cc4b28bd0e6da01059d8c3e2d8540f45aaa"
    ),
    "timeout_lifecycle_stage_classifier_bug.cfg": (
        "9e34e285ced97f50410472ad286c45204da06d7d8799c13253e2283a6f786f27"
    ),
    "persist_install_timeout_tag_fixed.cfg": (
        "18c0be476f6f037732ce8f7836651babb30d93f3e6e41401fede8828589b4f9e"
    ),
    "persist_install_timeout_tag_bug.cfg": (
        "2fba91cff8430b97696d74363185f7a998638be3600ba9f02b991df01b2b65c1"
    ),
    "persist_install_timeout_root_retirement_fixed.cfg": (
        "6e7f1531fba78d78b3c9b8ead4f5c4686ff355e87d18be3303310282a4d16222"
    ),
    "persist_install_timeout_root_retirement_bug.cfg": (
        "5b6cecf701049e6e23ee2c92f13395625fe12ae5de1316b620581d81448eb2f7"
    ),
    "local_ingress_scheduler_reservation_fixed.cfg": (
        "963b6db59f7731b4d70b316592277d25676944dab4c5acb233f851411692cdb6"
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
        "24ef6f0e080c4e64fd19d4263ce82fe369d42c3e4cd43da9c0aab3517ceaddce"
    ),
    "SumeragiV2EmptyProducerHandoffMutation.tla": (
        "b03fa34b133d69b33e4885c479ac8e6f44fd30382f451f15ab3d11ca721cde7b"
    ),
    "SumeragiV2ProducerOriginReservationMutation.tla": (
        "3a0c8fc62c67b4bad96692fcbe4c75c2b2a3f9f7b3b95a2a502e28ecb081fd19"
    ),
    "SumeragiV2ProducerContinuationCausalRankMutation.tla": (
        "cdf86251806c6fec7583293fe80d2e6da723421ef7fb477f54d5f7ebf7e385ae"
    ),
    "SumeragiV2RepresentativeLiveScopeMutation.tla": (
        "edead4bb5d0d9801eb7d3e9c4fb96a0cd3fb2daf3c5adec1fccecd36d1aa086a"
    ),
    "SumeragiV2FixedCorridorPhysicalBudgetMutation.tla": (
        "474364c3251ab2b24838d1e7034c3d4f6a28928581ada184c7b2a7479ef69860"
    ),
    "SumeragiV2FixedCorridorActionCreditMutation.tla": (
        "c209e7f2ce212d3f7ea8620f7458c79d1a932e32d0b2b4c2ef1b5831979c5f71"
    ),
    "SumeragiV2ProposalPipelineBudgetMutation.tla": (
        "33f478e137dccb253c2569b7856af27ae3bab54dffda6d10a7c9ddd38d27e8bf"
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
        "a1f4aaf0190357a03d1cb1644b07bff17a5004bc8e1d116e99294ff7713a45a4"
    ),
    "fixed_corridor_physical_budget_omitted_lane_cursor_bug.cfg": (
        "c96633d9cb41fa777f6e4e05513b182f4a5018af81756e20851feffc69b9eb10"
    ),
    "fixed_corridor_action_credit_fixed.cfg": (
        "fdd5c5fa275f1dcbe34c28241fd2f1ebc4d301d1b57a7b64deb3d91ebc4007da"
    ),
    "fixed_corridor_action_credit_per_child_recharge_bug.cfg": (
        "cc0dbf42553c4a08b7db2965aebed4fd18c4fc99ca2488c1f4a306c133340f60"
    ),
    "proposal_pipeline_budget_fixed.cfg": (
        "662cd733ed2b92b14d8a24c8a773eff16cc02462ebe038261ac6289a3b7ded75"
    ),
    "proposal_pipeline_budget_additive_bug.cfg": (
        "49b0376d1a46952ce3b4564a4020189808ce3cc4af89761b0b180f877a7ca17b"
    ),
    "SumeragiV2AuthorityDeadlineCarryMutation.tla": (
        "52d39ae42f519e145fc099021d362472bbed903c5d971eac58707189e5a5c507"
    ),
    "SumeragiV2AdequateLeaderDeadlineAuthorityMutation.tla": (
        "2299cc4fdde3e1aa9567e7d3636c31afdec3d92815d61df5905040325febf7a1"
    ),
    "SumeragiV2AdequateLeaderSelectedLifecycleEpisodeMutation.tla": (
        "625e00ff55020a907d82754b5a92224de23bb83caa66a581359cde331d908392"
    ),
    "authority_deadline_carry_fixed.cfg": (
        "2bb0f19785f4c0466417a70fc225d4b1e17ee53d065959d4f96adbbc931e23ff"
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
        "4c49378074b10908a8dca290ebbb4565e8e87443741450c4556c150512940878"
    ),
    "fixed_corridor_receipt_acquisition_fixed.cfg": (
        "d3aff24a1305203debfc0ab9f70525efb42662063581b5989d9fe36c88a38e13"
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
        "c0172b14b0f8e4061bf416d41920af477aea5c0432de13e897ba70697ff03191"
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
        "c3427e8773ca3955fd0a81e3d17787230ca19a4672e42fea69c2ea4ec395a04a"
    ),
}
LIVENESS_OWNERSHIP_MUTATION_FORMAL_GLOBS = (
    "SumeragiV2Revision4CertifiedFenceReservation*.tla",
    "revision4_certified_fence_reservation_*.cfg",
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
        "a167d46e931ebe08e8933c27a1cc2efb26898cd7fd5dbeb2b006924c590d31f6"
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
        "96e2a255b699b3d60ba4fcd66cd212e7a8958adde9551b3fffc32118e9e31e02"
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
        "47175c5815cb5073b5ef97cc5f23d2609d15f4cf6caae6068633250c648d79ba"
    ),
    "scripts/formal/run_sumeragi_v2_service_rank_mutation.sh": (
        "41a0c3d8cfda9400fcdd26e27cabbbeeeae4e06f13fd9bb5819f38af50fe6a9e"
    ),
    "scripts/formal/run_sumeragi_v2_historical_discovery_occurrence_rank_mutation.sh": (
        "96e2a255b699b3d60ba4fcd66cd212e7a8958adde9551b3fffc32118e9e31e02"
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
        "98193358163d37c5169aa777ca043021cb7b1261fb34493563dc0307ddcf6a84"
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


def _revision4_model_contract_errors(
    formal_dir: Path,
    root_dir: Path = ROOT_DIR,
) -> list[str]:
    """Pin the executable revision-4 routing and conditional-progress surface."""

    errors: list[str] = []
    module_path = formal_dir / "SumeragiV2Revision4.tla"
    if not module_path.is_file() or module_path.is_symlink():
        return [f"{module_path}: revision-4 model must be a regular file"]
    source = module_path.read_text(encoding="utf-8")

    required_operator_tokens = {
        "Views": ("0..(N - 1)",),
        "ConstantOK": (
            "N = 3 * F + 1",
            "Cardinality(Faulty) <= F",
            "\\E candidateView \\in Views : UsableView(candidateView)",
        ),
        "Init": ("ConstantOK",),
        "Propose": (
            "manifestTargets' = Validators",
            "bodyTargets' = SetA(view)",
        ),
        "EnterFallback": (
            "fallback' = TRUE",
            "bodyTargets' = Validators",
        ),
        "ChangeView": (
            "view < N - 1",
            "fallback' = FALSE",
            "prepareVoteRoutes' = {}",
            "commitVoteRoutes' = {}",
            "timeoutVoteRoutes' = {}",
        ),
        "ManifestCommitteeFanout": (
            "manifestTargets = Validators",
        ),
        "FastPathAndFallbackBodyFanout": (
            "IF fallback THEN Validators ELSE SetA(view)",
        ),
        "PrepareVotesRouteToProxyTail": (
            "Validators \\X {ProxyTail(view)}",
            "VoteSigners(prepareVotes)",
        ),
        "CommitVotesRouteToProxyTail": (
            "Validators \\X {ProxyTail(view)}",
            "VoteSigners(commitVotes)",
        ),
        "TimeoutVotesBypassProxyTail": (
            "RouteSources(timeoutVoteRoutes) = timeoutVotes",
            "= Validators",
        ),
        "PostGSTSendTimeout": (
            "~UsableView(view)",
            "SendTimeout(validator)",
        ),
        "ConditionalPostGSTProgress": (
            "decisions /= {}",
            "applied",
            "successorActive",
        ),
        "FinalizedOutputDebtDoesNotBlockSuccessor": (
            "applied",
            "finalizedOutputDebt",
            "~> successorActive",
        ),
    }
    for operator, required in required_operator_tokens.items():
        extracted = _top_level_operator_body(
            source,
            operator,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(
                f"{module_path}: missing revision-4 operator {operator}"
            )
            continue
        body, line = extracted
        normalized = " ".join(body.split())
        missing = [token for token in required if token not in normalized]
        if missing:
            errors.append(
                f"{module_path}:{line}: revision-4 operator {operator} is "
                f"missing {missing}"
            )

    fairness = _top_level_operator_body(source, "PostGSTFairness")
    if fairness is None:
        errors.append(f"{module_path}: missing revision-4 PostGSTFairness")
    else:
        body, line = fairness
        normalized = " ".join(body.split())
        required_fair_actions = (
            "HonestLeaderProposes",
            "HonestBodyService",
            "EnterFallback",
            "HonestPrepareService",
            "HonestTailPrepareQCService",
            "HonestCommitService",
            "HonestTailDecisionService",
            "HonestTimeoutService",
            "ChangeView",
            "LocalDecisionBodyRecovery",
            "LocalDecisionApplication",
            "ActivateSuccessor",
        )
        missing = [
            action
            for action in required_fair_actions
            if f"WF_vars({action})" not in normalized
        ]
        if missing:
            errors.append(
                f"{module_path}:{line}: PostGSTFairness is missing weakly fair "
                f"services {missing}"
            )
        if "WF_vars(RepairFinalizedOutput)" in normalized:
            errors.append(
                f"{module_path}:{line}: finalized-output repair may not be a "
                "revision-4 successor-progress fairness prerequisite"
            )

    config_contracts = {
        "SumeragiV2Revision4.cfg": (
            "SPECIFICATION Spec",
            "INVARIANT ManifestCommitteeFanout",
            "INVARIANT FastPathAndFallbackBodyFanout",
            "INVARIANT PrepareVotesRouteToProxyTail",
            "INVARIANT CommitVotesRouteToProxyTail",
            "INVARIANT TimeoutVotesBypassProxyTail",
            "INVARIANT NonblockingSuccessorActivation",
        ),
        "SumeragiV2Revision4Liveness.cfg": (
            "SPECIFICATION PostGSTSpec",
            "PROPERTY ConditionalPostGSTProgress",
            "PROPERTY FinalizedOutputDebtDoesNotBlockSuccessor",
        ),
    }
    for filename, required in config_contracts.items():
        path = formal_dir / filename
        if not path.is_file() or path.is_symlink():
            errors.append(f"{path}: revision-4 TLC config must be a regular file")
            continue
        config_source = path.read_text(encoding="utf-8")
        missing = [token for token in required if token not in config_source]
        if missing:
            errors.append(
                f"{path}: revision-4 TLC configuration is missing {missing}"
            )

    runner_path = root_dir / "scripts" / "formal" / "run_sumeragi_v2_tlc.sh"
    if not runner_path.is_file() or runner_path.is_symlink():
        errors.append(f"{runner_path}: revision-4 TLC runner must be a regular file")
    else:
        runner_source = runner_path.read_text(encoding="utf-8")
        required_runner_tokens = (
            "revision4_safety",
            "revision4_liveness",
            'revision4_safety) cfg="SumeragiV2Revision4.cfg"',
            'revision4_liveness) cfg="SumeragiV2Revision4Liveness.cfg"',
            "revision4_safety|revision4_liveness)",
            "SumeragiV2Revision4.tla",
        )
        missing = [
            token for token in required_runner_tokens if token not in runner_source
        ]
        if missing:
            errors.append(
                f"{runner_path}: focused revision-4 TLC runner is missing {missing}"
            )
    return errors


def _revision4_adversarial_safety_contract_errors(
    formal_dir: Path,
    root_dir: Path = ROOT_DIR,
) -> list[str]:
    """Pin the non-vacuous four-validator/two-body adversarial safety search."""

    errors: list[str] = []
    module_path = formal_dir / "SumeragiV2Revision4AdversarialSafety.tla"
    if not module_path.is_file() or module_path.is_symlink():
        return [
            f"{module_path}: revision-4 adversarial safety model must be a "
            "regular file"
        ]
    source = module_path.read_text(encoding="utf-8")

    required_operator_tokens = {
        "ConstantOK": (
            "N = 4",
            "F = 1",
            "Q = 3",
            "Cardinality(Faulty) = 1",
            "Cardinality(Bodies) = 2",
        ),
        "HonestCommitVote": (
            "validator \\in Honest",
            "<<validator, body>> \\in fullBodies",
            "VoteBodies(validator) = {}",
            "commitVotes' = commitVotes \\cup {<<validator, body>>}",
        ),
        "ByzantineCommitVote": (
            "validator \\in Faulty",
            "<<validator, body>> \\notin commitVotes",
            "commitVotes' = commitVotes \\cup {<<validator, body>>}",
        ),
        "FormCommitQC": (
            "body \\notin commitQCs",
            "VoteCount(body) >= Q",
            "commitQCs' = commitQCs \\cup {body}",
        ),
        "Decide": (
            "body \\in commitQCs",
            "body \\notin decisions",
            "decisions' = decisions \\cup {body}",
        ),
        "ProtocolNext": (
            "DeliverFullBody(validator, body)",
            "HonestCommitVote(validator, body)",
            "ByzantineCommitVote(validator, body)",
            "FormCommitQC(body)",
            "Decide(body)",
        ),
        "Next": ("ProtocolNext", "TerminalComplete"),
        "FixedAdversarialGeometry": (
            "Cardinality(Validators) = 4",
            "Cardinality(Honest) = 3",
            "Cardinality(Faulty) = 1",
            "Q = 3",
        ),
        "HonestSignOncePerRound": (
            "validator \\in Honest",
            "Cardinality(VoteBodies(validator)) <= 1",
        ),
        "ByzantineEquivocationRemainsEnabled": (
            "validator \\in Faulty",
            "<<validator, body>> \\notin commitVotes",
            "ENABLED ByzantineCommitVote(validator, body)",
        ),
        "CommitQCsHaveQuorum": (
            "body \\in commitQCs",
            "VoteCount(body) >= Q",
        ),
        "DecisionsHaveCommitQC": ("decisions \\subseteq commitQCs",),
        "PostQCExecutionRemainsOpen": (
            "commitQCs /= {}",
            "ENABLED DeliverFullBody(validator, body)",
            "ENABLED HonestCommitVote(validator, body)",
            "ENABLED ByzantineCommitVote(validator, body)",
            "ENABLED FormCommitQC(body)",
            "ENABLED Decide(body)",
        ),
        "ConflictingCommitQCsImpossible": (
            "Cardinality(commitQCs) <= 1",
        ),
        "DecisionAgreement": ("Cardinality(decisions) <= 1",),
    }
    operator_bodies: dict[str, tuple[str, int]] = {}
    for operator, required in required_operator_tokens.items():
        extracted = _top_level_operator_body(
            source,
            operator,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(
                f"{module_path}: missing revision-4 adversarial operator "
                f"{operator}"
            )
            continue
        operator_bodies[operator] = extracted
        body, line = extracted
        normalized = " ".join(body.split())
        missing = [token for token in required if token not in normalized]
        if missing:
            errors.append(
                f"{module_path}:{line}: revision-4 adversarial operator "
                f"{operator} is missing {missing}"
            )

    byzantine_body = operator_bodies.get("ByzantineCommitVote")
    if byzantine_body is not None:
        body, line = byzantine_body
        normalized = " ".join(body.split())
        if "VoteBodies(" in normalized:
            errors.append(
                f"{module_path}:{line}: ByzantineCommitVote must permit the "
                "faulty validator to vote for both bodies"
            )

    for operator in (
        "DeliverFullBody",
        "HonestCommitVote",
        "ByzantineCommitVote",
        "FormCommitQC",
        "Decide",
    ):
        extracted = operator_bodies.get(operator)
        if extracted is None:
            continue
        body, line = extracted
        normalized = " ".join(body.split())
        forbidden = (
            "commitQCs = {}",
            "decisions = {}",
            "Cardinality(commitQCs) = 0",
            "Cardinality(decisions) = 0",
        )
        present = [token for token in forbidden if token in normalized]
        if present:
            errors.append(
                f"{module_path}:{line}: {operator} must remain enabled after "
                f"the first QC or decision; found global stop guards {present}"
            )

    config_path = formal_dir / "SumeragiV2Revision4AdversarialSafety.cfg"
    if not config_path.is_file() or config_path.is_symlink():
        errors.append(
            f"{config_path}: revision-4 adversarial TLC config must be a "
            "regular file"
        )
    else:
        config_source = config_path.read_text(encoding="utf-8")
        required_config_tokens = (
            "SPECIFICATION Spec",
            "Validators = {v1, v2, v3, v4}",
            "Faulty = {v4}",
            "Bodies = {b1, b2}",
            "INVARIANT FixedAdversarialGeometry",
            "INVARIANT HonestSignOncePerRound",
            "INVARIANT ByzantineEquivocationRemainsEnabled",
            "INVARIANT CommitQCsHaveQuorum",
            "INVARIANT DecisionsHaveCommitQC",
            "INVARIANT PostQCExecutionRemainsOpen",
            "INVARIANT ConflictingCommitQCsImpossible",
            "INVARIANT DecisionAgreement",
        )
        missing = [
            token
            for token in required_config_tokens
            if token not in config_source
        ]
        if missing:
            errors.append(
                f"{config_path}: revision-4 adversarial TLC configuration is "
                f"missing {missing}"
            )

    runner_path = root_dir / "scripts" / "formal" / "run_sumeragi_v2_tlc.sh"
    if not runner_path.is_file() or runner_path.is_symlink():
        errors.append(
            f"{runner_path}: revision-4 adversarial TLC runner must be a "
            "regular file"
        )
    else:
        runner_source = runner_path.read_text(encoding="utf-8")
        required_runner_tokens = (
            "revision4_adversarial_safety",
            (
                'revision4_adversarial_safety) cfg='
                '"SumeragiV2Revision4AdversarialSafety.cfg"'
            ),
            "SumeragiV2Revision4AdversarialSafety.tla",
        )
        missing = [
            token for token in required_runner_tokens if token not in runner_source
        ]
        if missing:
            errors.append(
                f"{runner_path}: focused revision-4 adversarial TLC runner is "
                f"missing {missing}"
            )
    return errors


def _revision4_certified_fence_reservation_contract_errors(
    formal_dir: Path,
    root_dir: Path = ROOT_DIR,
) -> list[str]:
    """Pin the bounded revision-4 certified-fence reservation kernel."""

    errors: list[str] = []
    module_path = formal_dir / "SumeragiV2Revision4CertifiedFenceReservation.tla"
    if not module_path.is_file() or module_path.is_symlink():
        return [
            f"{module_path}: revision-4 certified-fence reservation model "
            "must be a regular file"
        ]
    source = module_path.read_text(encoding="utf-8")
    stripped_source = strip_tla_comments(source)
    for retired_symbol in ("escapePhase", "CertifiedEscapeEpisodeIsOneShot"):
        if _symbol_exists(stripped_source, retired_symbol):
            errors.append(
                f"{module_path}: generic certified credit must not retain "
                f"response-local latch symbol {retired_symbol}"
            )

    required_operator_tokens = {
        "BarrierKinds": ('{"Serve", "LeaderWire"}',),
        "CertifiedKinds": (
            '{"TimeoutCertificate", "CommitQC", '
            '"CommitCertificateResponse"}',
        ),
        "IneligibleKinds": ('{"PrepareQC", "TimeoutVote"}',),
        "IngressKinds": ("CertifiedKinds \\cup IneligibleKinds",),
        "CommandClasses": ('{"Normal", "Progress", "Completion"}',),
        "Stages": ('{"Ingress", "Runtime", "TrustedTail", "Handled"}',),
        "RuntimeCapacity": ("4",),
        "NormalLimit": ("1",),
        "ProgressLimit": ("2",),
        "OrdinaryCompletionLimit": ("3",),
        "CompletionReserve": (
            "OrdinaryCompletionLimit - ProgressLimit",
        ),
        "OrdinaryRuntimePrefix": (
            '<<"Progress", "Progress", "Completion">>',
        ),
        "QueueClassCount": (
            "Cardinality(",
            "queue[index] = commandClass",
        ),
        "QueueCertifiedCount": (
            "CertifiedFenceEscapeKind(queue[index])",
        ),
        "QueueCertifiedCredit": (
            "QueueCertifiedCount(queue) = 0",
            "THEN 0 ELSE 1",
        ),
        "QueueNoncompletionCount": (
            'queue[index] # "Completion"',
        ),
        "ExternalOwnerCount": (
            "unpublishedBodyAvailable",
            "conflictingProposalQueued",
        ),
        "OwnedRuntimeDepth": (
            "Len(queue) + ExternalOwnerCount",
        ),
        "OwnedClassCount": (
            'commandClass = "Progress"',
            'QueueClassCount(queue, "Progress") + QueueCertifiedCount(queue)',
            "QueueClassCount(queue, commandClass)",
            'commandClass = "Completion" /\\ unpublishedBodyAvailable',
            'commandClass = "Normal" /\\ conflictingProposalQueued',
        ),
        "OwnedNoncompletionCount": (
            'OwnedClassCount(queue, "Normal")',
            'OwnedClassCount(queue, "Progress")',
        ),
        "CertifiedCreditIn": (
            "incomingCertified",
            "RetainedCertifiedCreditEnabled",
            "QueueCertifiedCount(queue) > 0",
        ),
        "CanAppendClass": (
            "CertifiedCreditIn(queue, incomingCertified)",
            "OwnedRuntimeDepth(queue) < RuntimeCapacity",
            "OwnedRuntimeDepth(queue) + 1",
            "<= OrdinaryCompletionLimit + credit",
            "normalAfter <= NormalLimit",
            "noncompletionAfter <= ProgressLimit + credit",
        ),
        "Init": (
            "ownerIdentity \\in OwnerIdentities",
            "ownerSnapshot = ownerIdentity",
            "ownerRetained = TRUE",
            "offeredKind \\in IngressKinds",
            "authenticated \\in BOOLEAN",
            'stage = "Ingress"',
            "runtimeQueue = OrdinaryRuntimePrefix",
            "runtimeQueue = <<>>",
            "pendingProgress = 0",
            "pendingProgress = 2",
            "pendingProgress = 1",
            "pendingCompletion = 0",
            "pendingCompletion = 1",
            "pendingCertified =",
            "CertifiedKinds \\ {offeredKind}",
            'runtimeQueue = <<"Progress">>',
            "conflictingProposalQueued",
        ),
        "CertifiedFenceEscapeKind": ("kind \\in CertifiedKinds",),
        "OfferAdvancesRetainedOwner": (
            "OfferContext = ownerIdentity.context",
            "OfferHeight = ownerIdentity.height",
            "OfferView >= ownerIdentity.view",
        ),
        "CanUseCertifiedFinalSlot": (
            "CertifiedFenceEscapeEnabled",
            "OwnedRuntimeDepth(runtimeQueue) = RuntimeCapacity - 1",
            "OwnedRuntimeDepth(runtimeQueue) < RuntimeCapacity",
            'CanAppendClass(runtimeQueue, offeredKind, "Progress", TRUE)',
        ),
        "CanUseCertifiedEarlySlot": (
            "CertifiedFenceEscapeEnabled",
            "OwnedRuntimeDepth(runtimeQueue) < RuntimeCapacity - 1",
            'CanAppendClass(runtimeQueue, offeredKind, "Progress", TRUE)',
        ),
        "AdmitCertifiedEscape": (
            'stage = "Ingress"',
            "ownerRetained",
            "authenticated",
            "CertifiedFenceEscapeKind(offeredKind)",
            "OfferAdvancesRetainedOwner",
            "CanUseCertifiedFinalSlot",
            'stage\' = "Runtime"',
            "runtimeQueue' = Append(runtimeQueue, offeredKind)",
            "UNCHANGED <<ownerIdentity, ownerSnapshot, ownerRetained",
        ),
        "AdmitCertifiedEscapeEarly": (
            'stage = "Ingress"',
            "CertifiedFenceEscapeKind(offeredKind)",
            "CanUseCertifiedEarlySlot",
            'stage\' = "Runtime"',
            "runtimeQueue' = Append(runtimeQueue, offeredKind)",
        ),
        "AdmitOrdinaryProgress": (
            "pendingProgress > 0",
            'CanAppendClass(runtimeQueue, "Progress", "Progress", FALSE)',
            'runtimeQueue\' = Append(runtimeQueue, "Progress")',
            "pendingProgress' = pendingProgress - 1",
        ),
        "AdmitOrdinaryCompletion": (
            "pendingCompletion > 0",
            'CanAppendClass(runtimeQueue, "Completion", "Completion", FALSE)',
            'runtimeQueue\' = Append(runtimeQueue, "Completion")',
            "pendingCompletion' = pendingCompletion - 1",
        ),
        "AdmitAdditionalCertified": (
            'stage = "Runtime"',
            "kind \\in pendingCertified",
            'CanAppendClass(runtimeQueue, kind, "Progress", TRUE)',
            "runtimeQueue' = Append(runtimeQueue, kind)",
            "pendingCertified' = pendingCertified \\ {kind}",
        ),
        "ReserveUnpublishedBodyAvailable": (
            "conflictingProposalQueued",
            "~unpublishedBodyAvailable",
            "unpublishedBodyAvailable' = TRUE",
            "conflictingProposalQueued' = FALSE",
        ),
        "DispatchCertifiedEscape": (
            'stage = "Runtime"',
            "ownerRetained",
            "CertifiedFenceEscapeEnabled",
            "OwnedRuntimeDepth(runtimeQueue) = RuntimeCapacity",
            "FirstCertifiedQueueIndex = Len(runtimeQueue)",
            "runtimeQueue[FirstCertifiedQueueIndex] = offeredKind",
            "CertifiedFenceEscapeKind(offeredKind)",
            'stage\' = "TrustedTail"',
            "runtimeQueue' = SubSeq(runtimeQueue, 1, Len(runtimeQueue) - 1)",
            "UNCHANGED <<ownerIdentity, ownerSnapshot, ownerRetained",
        ),
        "DispatchEarlyCertifiedEscape": (
            'stage = "Runtime"',
            "CertifiedQueueIndices # {}",
            "FirstCertifiedQueueIndex # Len(runtimeQueue)",
            "runtimeQueue' = RemoveAt(runtimeQueue, FirstCertifiedQueueIndex)",
        ),
        "RunCertifiedTrustedTail": (
            'stage = "TrustedTail"',
            "ownerRetained",
            "CertifiedFenceEscapeEnabled",
            "CertifiedFenceEscapeKind(offeredKind)",
            'stage\' = "Handled"',
            "ownerRetained' = FALSE",
            'installedTC\' = (offeredKind = "TimeoutCertificate")',
            'decided\' = (offeredKind \\in '
            '{"CommitQC", "CommitCertificateResponse"})',
            "UNCHANGED <<ownerIdentity, ownerSnapshot, offeredKind",
        ),
        "Next": (
            "AdmitCertifiedEscape",
            "AdmitCertifiedEscapeEarly",
            "AdmitOrdinaryProgress",
            "AdmitOrdinaryCompletion",
            "AdmitAdditionalCertified(kind)",
            "ReserveUnpublishedBodyAvailable",
            "DispatchCertifiedEscape",
            "DispatchEarlyCertifiedEscape",
            "RunCertifiedTrustedTail",
        ),
        "Spec": (
            "Init",
            "[][Next]_vars",
            "WF_vars(AdmitCertifiedEscape)",
            "WF_vars(AdmitCertifiedEscapeEarly)",
            "WF_vars(AdmitOrdinaryProgress)",
            "WF_vars(AdmitOrdinaryCompletion)",
            "WF_vars(ReserveUnpublishedBodyAvailable)",
            "WF_vars(AdmitAdditionalCertified(kind))",
            "WF_vars(DispatchCertifiedEscape)",
            "WF_vars(DispatchEarlyCertifiedEscape)",
            "WF_vars(RunCertifiedTrustedTail)",
        ),
        "OwnerIdentityNeverReplaced": ("ownerIdentity = ownerSnapshot",),
        "OwnerRetainedAcrossEscape": (
            'stage \\in {"Runtime", "TrustedTail"} '
            "=> ownerRetained",
        ),
        "NoOrdinaryRuntimeDisplacement": (
            "QueueCertifiedCredit(runtimeQueue)",
            'OwnedClassCount(runtimeQueue, "Normal") <= NormalLimit',
            "OwnedNoncompletionCount(runtimeQueue) - credit <= ProgressLimit",
            'OwnedClassCount(runtimeQueue, "Completion") <= CompletionReserve',
        ),
        "ReservedSlotOnlyCertified": (
            "OwnedRuntimeDepth(runtimeQueue) = RuntimeCapacity",
            "authenticated",
            "QueueCertifiedCount(runtimeQueue) >= 1",
            "CertifiedFenceEscapeKind(offeredKind)",
        ),
        "SingleCertifiedCredit": (
            "QueueCertifiedCredit(runtimeQueue) \\in {0, 1}",
            "QueueCertifiedCount(runtimeQueue) > 0",
            "<=> QueueCertifiedCredit(runtimeQueue) = 1",
        ),
        "OrdinaryCapacityGeometry": (
            "CompletionReserve = 1",
            "OwnedRuntimeDepth(runtimeQueue) - credit <= OrdinaryCompletionLimit",
            "OwnedNoncompletionCount(runtimeQueue) - credit <= ProgressLimit",
        ),
        "CertifiedFirstCompletionCorridor": (
            'stage = "Runtime"',
            "QueueCertifiedCount(runtimeQueue) >= 1",
            "pendingCompletion = 1",
            "OwnedNoncompletionCount(runtimeQueue) - 1 = ProgressLimit",
            "OwnedRuntimeDepth(runtimeQueue) < RuntimeCapacity",
            'CanAppendClass(runtimeQueue, "Completion", "Completion", FALSE)',
        ),
        "CertifiedFirstProgressCorridor": (
            'stage = "Runtime"',
            "QueueCertifiedCount(runtimeQueue) >= 1",
            "pendingProgress > 0",
            "OwnedNoncompletionCount(runtimeQueue) - 1 < ProgressLimit",
            "OwnedRuntimeDepth(runtimeQueue) < RuntimeCapacity",
            'CanAppendClass(runtimeQueue, "Progress", "Progress", FALSE)',
        ),
        "PrepareQcCannotUseEscape": (
            'offeredKind = "PrepareQC" => stage = "Ingress"',
        ),
        "RawTimeoutVoteCannotUseEscape": (
            'offeredKind = "TimeoutVote" => stage = "Ingress"',
        ),
        "AuthenticationRequiredForEscape": (
            'stage \\in {"Runtime", "TrustedTail", "Handled"} '
            "=> authenticated",
        ),
        "HandledOutcomeExact": (
            'stage = "Handled"',
            'offeredKind = "TimeoutCertificate"',
            "installedTC /\\ ~decided",
            "~installedTC /\\ decided",
        ),
        "UnpublishedBodyAvailableOwnsOrdinaryCompletion": (
            "~(unpublishedBodyAvailable /\\ conflictingProposalQueued)",
            'OwnedClassCount(runtimeQueue, "Completion") >= 1',
            'OwnedClassCount(runtimeQueue, "Normal") >= 1',
        ),
        "CertifiedEscapeEventuallyHandled": (
            "authenticated",
            "CertifiedFenceEscapeKind(offeredKind)",
            "ownerRetained",
            '~> (stage = "Handled")',
        ),
    }
    operator_bodies: dict[str, tuple[str, int]] = {}
    for operator, required in required_operator_tokens.items():
        extracted = _top_level_operator_body(
            source,
            operator,
            preserve_string_contents=True,
        )
        if extracted is None:
            errors.append(
                f"{module_path}: missing revision-4 certified-fence operator "
                f"{operator}"
            )
            continue
        operator_bodies[operator] = extracted
        body, line = extracted
        normalized = " ".join(body.split())
        missing = [token for token in required if token not in normalized]
        if missing:
            errors.append(
                f"{module_path}:{line}: revision-4 certified-fence operator "
                f"{operator} is missing {missing}"
            )

    exact_kind_bodies = {
        "CertifiedKinds": (
            '{"TimeoutCertificate", "CommitQC", '
            '"CommitCertificateResponse"}'
        ),
        "IneligibleKinds": '{"PrepareQC", "TimeoutVote"}',
        "CommandClasses": '{"Normal", "Progress", "Completion"}',
        "CertifiedFenceEscapeKind": "kind \\in CertifiedKinds",
    }
    for operator, expected in exact_kind_bodies.items():
        extracted = operator_bodies.get(operator)
        if extracted is None:
            continue
        body, line = extracted
        normalized = " ".join(body.split())
        if normalized != expected:
            errors.append(
                f"{module_path}:{line}: revision-4 certified-fence operator "
                f"{operator} must equal only {expected!r}; found "
                f"{normalized!r}"
            )

    exact_numeric_bodies = {
        "RuntimeCapacity": "4",
        "NormalLimit": "1",
        "ProgressLimit": "2",
        "OrdinaryCompletionLimit": "3",
        "CompletionReserve": "OrdinaryCompletionLimit - ProgressLimit",
        "QueueCertifiedCredit": (
            "IF QueueCertifiedCount(queue) = 0 THEN 0 ELSE 1"
        ),
    }
    for operator, expected in exact_numeric_bodies.items():
        extracted = operator_bodies.get(operator)
        if extracted is None:
            continue
        body, line = extracted
        normalized = " ".join(body.split())
        if normalized != expected:
            errors.append(
                f"{module_path}:{line}: revision-4 certified-fence operator "
                f"{operator} must equal only {expected!r}; found "
                f"{normalized!r}"
            )

    atomic_replacement = _top_level_theorem_body(
        source,
        "BodyAvailableReservationAtomicallyReplacesConflict",
        preserve_string_contents=True,
    )
    if atomic_replacement is None:
        errors.append(
            f"{module_path}: missing atomic unpublished BodyAvailable replacement theorem"
        )
    else:
        body, line = atomic_replacement
        normalized = " ".join(body.split())
        required = (
            "ReserveUnpublishedBodyAvailable",
            "unpublishedBodyAvailable'",
            "~conflictingProposalQueued'",
            "OwnedRuntimeDepth(runtimeQueue') = OwnedRuntimeDepth(runtimeQueue)",
            "BY DEF ReserveUnpublishedBodyAvailable",
            "OwnedRuntimeDepth",
            "ExternalOwnerCount",
        )
        missing = [token for token in required if token not in normalized]
        if missing:
            errors.append(
                f"{module_path}:{line}: atomic BodyAvailable replacement theorem "
                f"is missing {missing!r}"
            )

    config_contracts = {
        "revision4_certified_fence_reservation_fixed.cfg": (
            "SPECIFICATION Spec",
            "CONSTANTS",
            "CertifiedFenceEscapeEnabled = TRUE",
            "RetainedCertifiedCreditEnabled = TRUE",
            "INVARIANT TypeOK",
            "INVARIANT OwnerIdentityNeverReplaced",
            "INVARIANT OwnerRetainedAcrossEscape",
            "INVARIANT NoOrdinaryRuntimeDisplacement",
            "INVARIANT ReservedSlotOnlyCertified",
            "INVARIANT SingleCertifiedCredit",
            "INVARIANT OrdinaryCapacityGeometry",
            "INVARIANT CertifiedFirstCompletionCorridor",
            "INVARIANT CertifiedFirstProgressCorridor",
            "INVARIANT PrepareQcCannotUseEscape",
            "INVARIANT RawTimeoutVoteCannotUseEscape",
            "INVARIANT AuthenticationRequiredForEscape",
            "INVARIANT HandledOutcomeExact",
            "INVARIANT UnpublishedBodyAvailableOwnsOrdinaryCompletion",
            "PROPERTY CertifiedEscapeEventuallyHandled",
            "CHECK_DEADLOCK FALSE",
        ),
        "revision4_certified_fence_reservation_blocked_bug.cfg": (
            "SPECIFICATION Spec",
            "CONSTANTS",
            "CertifiedFenceEscapeEnabled = FALSE",
            "RetainedCertifiedCreditEnabled = TRUE",
            "INVARIANT TypeOK",
            "INVARIANT OwnerIdentityNeverReplaced",
            "INVARIANT NoOrdinaryRuntimeDisplacement",
            "INVARIANT PrepareQcCannotUseEscape",
            "INVARIANT RawTimeoutVoteCannotUseEscape",
            "PROPERTY CertifiedEscapeEventuallyHandled",
            "CHECK_DEADLOCK FALSE",
        ),
        "revision4_certified_fence_reservation_arrival_order_bug.cfg": (
            "SPECIFICATION Spec",
            "CONSTANTS",
            "CertifiedFenceEscapeEnabled = TRUE",
            "RetainedCertifiedCreditEnabled = FALSE",
            "INVARIANT TypeOK",
            "INVARIANT CertifiedFirstProgressCorridor",
            "CHECK_DEADLOCK FALSE",
        ),
    }
    for filename, required in config_contracts.items():
        config_path = formal_dir / filename
        if not config_path.is_file() or config_path.is_symlink():
            errors.append(
                f"{config_path}: revision-4 certified-fence TLC config must "
                "be a regular file"
            )
            continue
        config_source = config_path.read_text(encoding="utf-8")
        if "CertifiedEscapeEpisodeIsOneShot" in config_source:
            errors.append(
                f"{config_path}: generic certified-credit configuration must "
                "not retain the response-local one-shot latch"
            )
        missing = [token for token in required if token not in config_source]
        if missing:
            errors.append(
                f"{config_path}: revision-4 certified-fence TLC "
                f"configuration is missing {missing}"
            )

    runner_path = root_dir / "scripts" / "formal" / "run_sumeragi_v2_tlc.sh"
    if not runner_path.is_file() or runner_path.is_symlink():
        errors.append(
            f"{runner_path}: revision-4 certified-fence TLC runner must be a "
            "regular file"
        )
    else:
        runner_source = runner_path.read_text(encoding="utf-8")
        required_runner_tokens = (
            "revision4_certified_fence_reservation",
            (
                'revision4_certified_fence_reservation) cfg='
                '"revision4_certified_fence_reservation_fixed.cfg"'
            ),
            "SumeragiV2Revision4CertifiedFenceReservation.tla",
        )
        missing = [
            token for token in required_runner_tokens if token not in runner_source
        ]
        if missing:
            errors.append(
                f"{runner_path}: focused revision-4 certified-fence TLC "
                f"runner is missing {missing}"
            )

    mutation_runner_path = (
        root_dir
        / "scripts"
        / "formal"
        / "run_sumeragi_v2_liveness_ownership_mutations.sh"
    )
    if not mutation_runner_path.is_file() or mutation_runner_path.is_symlink():
        errors.append(
            f"{mutation_runner_path}: certified-fence mutation runner must be "
            "a regular file"
        )
    else:
        runner_source = mutation_runner_path.read_text(encoding="utf-8")
        required_runner_tokens = (
            "SumeragiV2Revision4CertifiedFenceReservation.tla",
            (
                "revision4-certified-fence-reservation|"
                "SumeragiV2Revision4CertifiedFenceReservation.tla|"
                "revision4_certified_fence_reservation_fixed.cfg"
            ),
            (
                "revision4-certified-fence-reservation-blocked|"
                "SumeragiV2Revision4CertifiedFenceReservation.tla|"
                "revision4_certified_fence_reservation_blocked_bug.cfg"
            ),
            (
                "revision4-certified-fence-arrival-order|"
                "SumeragiV2Revision4CertifiedFenceReservation.tla|"
                "revision4_certified_fence_reservation_arrival_order_bug.cfg|"
                "CertifiedFirstProgressCorridor"
            ),
        )
        missing = [
            token for token in required_runner_tokens if token not in runner_source
        ]
        if missing:
            errors.append(
                f"{mutation_runner_path}: certified-fence mutation runner is "
                f"missing {missing}"
            )

    documentation_contracts = {
        formal_dir / "README.md": (
            "There is no response-local phase or resettable certificate latch.",
            "A selected `CertifiedResponse` remains ordinary FIFO work: a later\n"
            "`TimeoutVote` cannot cross it, and timeout control is a dependency only when it\n"
            "advances the current Proposal, vote, QC, or TimeoutVote owner.",
            "claimed-response rank therefore counts the exact direct roots already inside\n"
            "its frozen prefix plus their strictly decreasing trusted causal tail; later\n"
            "timeout traffic cannot replenish that prefix.",
            "The\nstandalone revision-4 kernel also charges an unpublished `BodyAvailable` token\n"
            "as an ordinary Completion owner and replaces its conflicting proposal owner in\n"
            "one atomic transition, preserving physical occupancy throughout the swap.",
        ),
        formal_dir / "PROOF.md": (
            "There is no response-local phase or resettable\n"
            "certificate latch.",
            "A selected `CertifiedResponse` remains ordinary FIFO work:\n"
            "a later `TimeoutVote` cannot cross it, while timeout control is a dependency\n"
            "only when it advances the current Proposal, vote, QC, or TimeoutVote owner.",
            "claimed-response rank counts the exact direct roots already inside its frozen\n"
            "prefix and their strictly decreasing trusted causal tail, so later timeout\n"
            "traffic cannot replenish that prefix.",
            "The standalone revision-4 kernel charges\n"
            "an unpublished `BodyAvailable` token as an ordinary Completion owner and\n"
            "atomically replaces its conflicting proposal owner without changing physical\n"
            "occupancy.",
        ),
    }
    for documentation_path, claims in documentation_contracts.items():
        if not documentation_path.is_file() or documentation_path.is_symlink():
            errors.append(
                f"{documentation_path}: revision-4 certified-credit documentation must be regular"
            )
            continue
        documentation = documentation_path.read_text(encoding="utf-8")
        for claim in claims:
            if documentation.count(claim) != 1:
                errors.append(
                    f"{documentation_path}: revision-4 certified-credit documentation must "
                    f"contain exact claim {claim!r}"
                )
    return errors


def _runtime_certified_fence_capacity_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Pin production's retained one-credit C/P/K runtime admission."""

    errors: list[str] = []
    runtime_path = repo_root / "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    runtime_path, source = _read_reviewed_rust_source(
        repo_root,
        runtime_path.relative_to(repo_root).as_posix(),
        errors,
        "certified-fence runtime capacity source",
    )
    if not source:
        return errors

    config_items: dict[str, RustItem | None] = {}
    for item_name in (
        "validate",
        "normal_limit",
        "progress_limit",
        "ordinary_total_limit",
    ):
        item = _require_qualified_rust_item(
            runtime_path,
            source,
            "RuntimeQueueConfig",
            item_name,
            errors,
            f"RuntimeQueueConfig::{item_name} C/P/K geometry",
        )
        config_items[item_name] = item
        _require_rust_item_token_sha256(
            runtime_path,
            item,
            _PRODUCTION_RUNTIME_CERTIFIED_FENCE_CAPACITY_ITEM_SHA256[
                f"RuntimeQueueConfig::{item_name}"
            ],
            f"RuntimeQueueConfig::{item_name} C/P/K geometry",
            errors,
        )

    _require_exact_rust_tokens(
        runtime_path,
        config_items.get("normal_limit"),
        """
const fn normal_limit(self) -> usize {
    self.capacity - self.progress_reserve - self.completion_reserve - 1
}
""",
        "normal limit must equal C-P-K-1",
        errors,
    )
    _require_exact_rust_tokens(
        runtime_path,
        config_items.get("progress_limit"),
        """
const fn progress_limit(self) -> usize {
    self.capacity - self.completion_reserve - 1
}
""",
        "ordinary noncompletion limit must equal C-K-1",
        errors,
    )
    _require_exact_rust_tokens(
        runtime_path,
        config_items.get("ordinary_total_limit"),
        """
const fn ordinary_total_limit(self) -> usize {
    self.capacity - 1
}
""",
        "ordinary total limit must equal C-1",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        config_items.get("validate"),
        """
self.progress_reserve
    .checked_add(self.completion_reserve)
    .and_then(|reserved| reserved.checked_add(1))
    .is_none_or(|reserved| reserved >= self.capacity)
""",
        "queue validation must reserve P+K+1 strictly below C",
        errors,
    )

    bounded_context = (
        (
            "impl",
            "<",
            "C",
            ":",
            "ExactRuntimeCommandIdentity",
            ">",
            "BoundedIngress",
            "<",
            "C",
            ">",
        ),
    )
    bounded_items: dict[str, RustItem | None] = {}
    for item_name in (
        "certified_fence_escape_credit",
        "check_capacity_change_inner",
        "remaining_capacity",
    ):
        item = _require_rust_item(runtime_path, source, item_name, errors)
        bounded_items[item_name] = item
        _require_rust_item_context(
            runtime_path,
            item,
            bounded_context,
            f"BoundedIngress::{item_name} retained-credit admission",
            errors,
        )
        _require_rust_item_token_sha256(
            runtime_path,
            item,
            _PRODUCTION_RUNTIME_CERTIFIED_FENCE_CAPACITY_ITEM_SHA256[
                f"BoundedIngress::{item_name}"
            ],
            f"BoundedIngress::{item_name} retained-credit admission",
            errors,
        )

    _require_exact_rust_tokens(
        runtime_path,
        bounded_items.get("certified_fence_escape_credit"),
        """
fn certified_fence_escape_credit(&self) -> usize {
    usize::from(
        self.commands
            .iter()
            .any(|queued| queued.command.is_certified_fence_escape()),
    )
}
""",
        "queued immutable certificate roots must retain exactly one credit",
        errors,
    )
    capacity_change = bounded_items.get("check_capacity_change_inner")
    for required, description in (
        (
            """
if certified_fence_escape
    && (class != CommandClass::Progress
        || additions != 1
        || dormant_replacements != 0)
{
    return Err(EnqueueError::FailClosed);
}
""",
            "certified credit is restricted to one exact Progress addition",
        ),
        (
            """
let (normal_before, progress_before, retained_certified) =
    self.commands.iter().try_fold(
        (0usize, 0usize, false),
        |(normal, progress, certified), queued| {
            let normal = normal
                .checked_add(usize::from(queued.class == CommandClass::Normal))
                .ok_or(EnqueueError::FailClosed)?;
            let progress = progress
                .checked_add(usize::from(queued.class == CommandClass::Progress))
                .ok_or(EnqueueError::FailClosed)?;
            Ok::<_, EnqueueError>((
                normal,
                progress,
                certified || queued.command.is_certified_fence_escape(),
            ))
        },
    )?;
let certified_credit = usize::from(retained_certified || certified_fence_escape);
""",
            "one scan derives class occupancy and retained certificate credit",
        ),
        (
            """
let ordinary_occupied_after = occupied_after
    .checked_sub(certified_credit)
    .ok_or(EnqueueError::FailClosed)?;
if ordinary_occupied_after > self.config.ordinary_total_limit() {
    return Err(EnqueueError::Full);
}
""",
            "ordinary total occupancy excludes the one retained credit",
        ),
        (
            """
let ordinary_noncompletion_after = noncompletion_after
    .checked_sub(certified_credit)
    .ok_or(EnqueueError::FailClosed)?;
if normal_after > self.config.normal_limit()
    || ordinary_noncompletion_after > self.config.progress_limit()
{
    return Err(EnqueueError::ReservedCapacity);
}
""",
            "normal and noncompletion limits retain disjoint P/K capacity",
        ),
    ):
        _require_rust_token_sequence(
            runtime_path,
            capacity_change,
            required,
            description,
            errors,
        )
    _require_exact_rust_tokens(
        runtime_path,
        bounded_items.get("remaining_capacity"),
        """
fn remaining_capacity(&self) -> usize {
    let ordinary_occupied = self
        .occupied_with_dormant_reservations()
        .unwrap_or(usize::MAX)
        .saturating_sub(self.certified_fence_escape_credit());
    self.config
        .ordinary_total_limit()
        .saturating_sub(ordinary_occupied)
}
""",
        "remaining ordinary capacity must retain queued certificate credit",
        errors,
    )

    classifier = _require_rust_item(
        runtime_path,
        source,
        "wire_payload_is_certified_fence_escape",
        errors,
    )
    _require_rust_item_context(
        runtime_path,
        classifier,
        (),
        "closed runtime wire-payload certified-fence classifier",
        errors,
    )
    _require_rust_item_token_sha256(
        runtime_path,
        classifier,
        _PRODUCTION_RUNTIME_CERTIFIED_FENCE_CAPACITY_ITEM_SHA256[
            "wire_payload_is_certified_fence_escape"
        ],
        "closed runtime wire-payload certified-fence classifier",
        errors,
    )
    _require_exact_rust_tokens(
        runtime_path,
        classifier,
        """
pub(crate) const fn wire_payload_is_certified_fence_escape(
    payload: &wire::ConsensusMessageV2Payload,
) -> bool {
    matches!(
        payload,
        wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
            | wire::ConsensusMessageV2Payload::QuorumCertificate(wire::QuorumCertificate {
                phase: wire::GlobalPhase::Commit,
                ..
            })
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(
                wire::CommitCertificateResponse {
                    certificate: wire::QuorumCertificate {
                        phase: wire::GlobalPhase::Commit,
                        ..
                    },
                    ..
                }
            )
    )
}
""",
        "only TC, direct CommitQC, and CommitQC recovery response receive credit",
        errors,
    )
    _require_rust_source_token_sequence(
        runtime_path,
        source,
        """
mod exact_runtime_command_identity_sealed {
    pub trait Sealed {}
}

pub(crate) trait ExactRuntimeCommandIdentity:
    exact_runtime_command_identity_sealed::Sealed
{
""",
        "certified-credit classifier must remain module sealed",
        errors,
    )
    sealed_impl_prefix = rust_code_tokens(
        "impl exact_runtime_command_identity_sealed::Sealed for"
    )
    sealed_impl_count = _token_sequence_count(
        rust_code_tokens(source), sealed_impl_prefix
    )
    if sealed_impl_count != 3:
        errors.append(
            f"{runtime_path}: exact runtime command identity must retain "
            "exactly the authenticated, adapter, and test-only sealed "
            f"implementations; found {sealed_impl_count}"
        )
    for command_type in (
        "AuthenticatedConsensusMessage",
        "AdapterCommand",
        "FakeCommand",
    ):
        _require_rust_source_token_sequence(
            runtime_path,
            source,
            (
                "impl exact_runtime_command_identity_sealed::Sealed for "
                f"{command_type} {{}}"
            ),
            f"sealed exact runtime identity implementation for {command_type}",
            errors,
        )
    _require_rust_source_token_sequence(
        runtime_path,
        source,
        """
impl ExactRuntimeCommandIdentity for AuthenticatedConsensusMessage {
    fn exact_runtime_command_identity(&self) -> RuntimeCommandIdentity {
""",
        "authenticated runtime identity implementation",
        errors,
    )
    _require_rust_source_token_sequence(
        runtime_path,
        source,
        """
fn is_certified_fence_escape(&self) -> bool {
    wire_payload_is_certified_fence_escape(self.payload())
}
""",
        "authenticated commands derive credit from exact wire payload",
        errors,
    )
    _require_rust_source_token_sequence(
        runtime_path,
        source,
        """
fn is_certified_fence_escape(&self) -> bool {
    matches!(self, Self::Authenticated(message) if message.is_certified_fence_escape())
}
""",
        "adapter commands cannot assert certificate credit independently",
        errors,
    )

    tests = (
        "certified_commit_uses_physical_slot_reserved_from_completions",
        "certified_commit_arriving_first_preserves_every_ordinary_reserve",
        "distinct_certificates_share_exactly_one_physical_credit",
        "invalid_configuration_is_rejected",
        "queue_configuration_excludes_one_certified_credit_from_ordinary_limits",
        "prepare_qc_cannot_spend_the_certified_physical_credit",
        "retiring_the_sole_certificate_does_not_fake_completion_headroom",
        "unpublished_body_replacement_cannot_overbook_the_certified_slot",
    )
    test_context = (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),)
    test_items: dict[str, RustItem | None] = {}
    for item_name in tests:
        item = _require_rust_item(runtime_path, source, item_name, errors)
        test_items[item_name] = item
        _require_rust_item_context(
            runtime_path,
            item,
            test_context,
            f"certified-fence capacity regression {item_name}",
            errors,
            expected_attributes=("#[test]",),
        )
        _require_rust_item_token_sha256(
            runtime_path,
            item,
            _PRODUCTION_RUNTIME_CERTIFIED_FENCE_CAPACITY_ITEM_SHA256[
                f"test::{item_name}"
            ],
            f"certified-fence capacity regression {item_name}",
            errors,
        )
    _require_rust_token_sequence(
        runtime_path,
        test_items.get(
            "certified_commit_arriving_first_preserves_every_ordinary_reserve"
        ),
        """
assert_eq!(
    runtime.remaining_completion_capacity(),
    7,
    "charging the CommitQC to its own slot leaves every ordinary position free"
);
""",
        "certificate-first regression preserves every ordinary slot",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        test_items.get("distinct_certificates_share_exactly_one_physical_credit"),
        """
assert!(matches!(
    runtime.enqueue_network(commit(0xC3)),
    Err(NetworkIngressError::Backpressure(
        EnqueueError::ReservedCapacity
    ))
));
""",
        "distinct certificate roots cannot mint a second physical credit",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        test_items.get("invalid_configuration_is_rejected"),
        "RuntimeQueueConfig::new(3, 1, 1).validate()",
        "P+K+1 equal to C must fail closed",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        test_items.get(
            "queue_configuration_excludes_one_certified_credit_from_ordinary_limits"
        ),
        """
assert_eq!(config.normal_limit(), 3);
assert_eq!(config.progress_limit(), 5);
assert_eq!(config.ordinary_total_limit(), 7);
assert_eq!(
    config.normal_limit() + config.progress_reserve + config.completion_reserve + 1,
    config.capacity
);
""",
        "C=8/P=2/K=2 must expose C-P-K-1, C-K-1, C-1, and one credit",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        test_items.get("prepare_qc_cannot_spend_the_certified_physical_credit"),
        """
assert!(matches!(
    runtime.enqueue_network(certificate(0xB2, wire::GlobalPhase::Prepare)),
    Err(NetworkIngressError::Backpressure(
        EnqueueError::ReservedCapacity
    ))
));

runtime
    .enqueue_network(certificate(0xB3, wire::GlobalPhase::Commit))
    .expect("only the CommitQC receives the certified physical credit");
""",
        "PrepareQC cannot spend the exact CommitQC/TC physical credit",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        test_items.get(
            "retiring_the_sole_certificate_does_not_fake_completion_headroom"
        ),
        """
assert_eq!(
    runtime.remaining_completion_capacity(),
    0,
    "retiring the only certificate also retires its physical credit"
);
""",
        "sole-certificate retirement must not invent Completion headroom",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        test_items.get(
            "unpublished_body_replacement_cannot_overbook_the_certified_slot"
        ),
        """
let reservation = runtime
    .ingress
    .reserve_canonical_body_available(owner_tag, canonical)
    .expect("the unpublished body atomically replaces its conflicting proposal");
assert_eq!(
    runtime.queued_commands(),
    2,
    "the conflicting proposal must retire before the reservation becomes live"
);
assert_eq!(runtime.remaining_completion_capacity(), 0);
""",
        "unpublished BodyAvailable must atomically replace its conflict",
        errors,
    )

    reserve_body = _require_rust_item(
        runtime_path,
        source,
        "reserve_canonical_body_available_internal",
        errors,
    )
    commit_body = _require_rust_item(
        runtime_path,
        source,
        "commit_canonical_body_available",
        errors,
    )
    _require_rust_token_sequence(
        runtime_path,
        reserve_body,
        """
ingress.discard_proposals_conflicting_with(reservation.manifest());
ingress.reserved_body_available = Some(reservation.clone());
""",
        "reservation publication must atomically retire the conflicting proposal first",
        errors,
    )
    if commit_body is not None and _token_sequence_count(
        rust_code_tokens(commit_body.source),
        rust_code_tokens("self.discard_proposals_conflicting_with(reservation.manifest())"),
    ):
        errors.append(
            f"{runtime_path}:{commit_body.line}: BodyAvailable materialization "
            "must not defer conflicting-proposal retirement past reservation publication"
        )
    expected_capacity_seal_keys = {
        "RuntimeQueueConfig::validate",
        "RuntimeQueueConfig::normal_limit",
        "RuntimeQueueConfig::progress_limit",
        "RuntimeQueueConfig::ordinary_total_limit",
        "BoundedIngress::certified_fence_escape_credit",
        "BoundedIngress::check_capacity_change_inner",
        "BoundedIngress::remaining_capacity",
        "wire_payload_is_certified_fence_escape",
        *(f"test::{item_name}" for item_name in tests),
    }
    observed_capacity_seal_keys = set(
        _PRODUCTION_RUNTIME_CERTIFIED_FENCE_CAPACITY_ITEM_SHA256
    )
    if observed_capacity_seal_keys != expected_capacity_seal_keys:
        errors.append(
            "runtime certified-fence source-seal inventory must contain "
            "exactly eight production items and eight regressions; "
            f"missing={sorted(expected_capacity_seal_keys - observed_capacity_seal_keys)}, "
            f"extra={sorted(observed_capacity_seal_keys - expected_capacity_seal_keys)}"
        )
    return errors


def _async_spec_shape_errors(formal_dir: Path) -> list[str]:
    """Keep deductive and finite specs on one canonical state/fairness surface."""

    path = formal_dir / "SumeragiV2AsyncNetwork.tla"
    expected = {
        "AsyncBaseInit": "AsyncBaseInitAt(ContextRecord(0, <<>>))",
        "AsyncInitAt": "AsyncBaseInitAt(initialContext) /\\ ViewDomain = Nat",
        "AsyncInit": "AsyncInitAt(ContextRecord(0, <<>>))",
        "AsyncFiniteInitAt": (
            "AsyncBaseInitAt(initialContext) /\\ ViewDomain = FiniteViews"
        ),
        "AsyncFiniteInit": "AsyncFiniteInitAt(ContextRecord(0, <<>>))",
        "AsyncAllVars": (
            "<<gst, vars, AsyncSchedulerVars, AsyncRecoveryVars, "
            "AsyncProducerVars, asyncFixedCorridorDeadlines, "
            "asyncServeProducerTurnReady>>"
        ),
        "AsyncSpec": "AsyncInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairness",
        "AsyncSpecAt": (
            "AsyncInitAt(initialContext) /\\ [][AsyncNext]_AsyncAllVars "
            "/\\ AsyncFairnessAt(initialContext)"
        ),
        "AsyncFiniteSpec": (
            "AsyncFiniteInit /\\ [][AsyncNext]_AsyncAllVars /\\ AsyncFairness"
        ),
        "AsyncFiniteSpecAt": (
            "AsyncFiniteInitAt(initialContext) "
            "/\\ [][AsyncNext]_AsyncAllVars "
            "/\\ AsyncFairnessAt(initialContext)"
        ),
        "AsyncInstallGenerationBudget": (
            "\\A request \\in pendingInstallTC: "
            "(StrictSameRoundTcUpgrade(request.node, request.tc) "
            "=> GenerationCanIncrement(generation[request.node]))"
        ),
        "AsyncRepresentativeLiveConfiguration": "N >= 4",
        "AsyncLiveSpecAt": (
            "AsyncRepresentativeLiveConfiguration /\\ AsyncSpecAt(initialContext)"
        ),
        "AsyncFiniteLiveSpec": (
            "AsyncRepresentativeLiveConfiguration /\\ AsyncFiniteSpec"
        ),
        "AsyncFairness": "AsyncFairnessAt(ContextRecord(0, <<>>))",
    }
    errors: list[str] = []
    if path.is_file():
        source = path.read_text(encoding="utf-8")
        for symbol, exact_body in expected.items():
            extracted = _top_level_operator_body(source, symbol)
            if extracted is None:
                errors.append(f"{path}: missing required asynchronous operator {symbol}")
                continue
            body, line = extracted
            normalized = " ".join(body.split())
            if normalized != exact_body:
                errors.append(
                    f"{path}:{line}: {symbol} must equal only {exact_body!r}; "
                    f"found {normalized!r}"
                )

        stripped = strip_tla_comments(source)
        for forbidden in (
            "AsyncTlcAllVars",
            "AsyncTlcFairnessAt",
            "AsyncTlcFairness",
        ):
            for match in re.finditer(rf"\b{re.escape(forbidden)}\b", stripped):
                line = stripped.count("\n", 0, match.start()) + 1
                errors.append(
                    f"{path}:{line}: TLC-only duplicate {forbidden} is prohibited; "
                    "finite and deductive specs must share AsyncAllVars and "
                    "AsyncFairnessAt"
                )

        public_fairness = _top_level_operator_body(
            source, "AsyncFairnessAt", preserve_string_contents=True
        )
        if public_fairness is None:
            errors.append(f"{path}: missing required asynchronous operator AsyncFairnessAt")
        else:
            public_body, public_line = public_fairness
            public_subscripts = set(
                re.findall(r"\bWF_([A-Za-z][A-Za-z0-9_]*)\s*\(", public_body)
            )
            if public_subscripts != {"AsyncAllVars"}:
                errors.append(
                    f"{path}:{public_line}: AsyncFairnessAt may use only the "
                    f"public AsyncAllVars subscript; found {sorted(public_subscripts)}"
                )

    for module in ("SumeragiV2LivenessProofs", "SumeragiV2AsyncLivenessProofs"):
        proof_path = formal_dir / f"{module}.tla"
        if not proof_path.is_file():
            continue
        stripped = strip_tla_comments(proof_path.read_text(encoding="utf-8"))
        for match in re.finditer(r"\bAsyncFiniteSpec\b", stripped):
            line = stripped.count("\n", 0, match.start()) + 1
            errors.append(
                f"{proof_path}:{line}: deductive liveness proofs must use "
                "unbounded AsyncSpec, not the finite TLC instance"
            )
    return errors


def _acyclic_liveness_debt_topology_errors(formal_dir: Path) -> list[str]:
    """Keep retained-lock proof leaves strictly below the async debt shard."""

    vocabulary_module = "SumeragiV2LivenessProofs"
    lower_consumers = ("SumeragiV2LockedBodyProposalActionProofs",)
    historical_kernel_module = "SumeragiV2AsyncHistoricalRecoveryLivenessProofs"
    topology_modules = (
        vocabulary_module,
        *lower_consumers,
        historical_kernel_module,
        ASYNC_LIVENESS_DEBT_SHARD,
    )
    topology_present = any(
        (formal_dir / f"{module}.tla").is_file()
        for module in (
            *lower_consumers,
            historical_kernel_module,
            ASYNC_LIVENESS_DEBT_SHARD,
        )
    )
    if not topology_present and not (formal_dir / "proof_coverage.json").is_file():
        return []

    errors: list[str] = []
    sources: dict[str, str] = {}
    for module in topology_modules:
        module_path = formal_dir / f"{module}.tla"
        if not module_path.is_file():
            errors.append(
                f"missing required acyclic liveness-debt topology module "
                f"{module}.tla"
            )
            continue
        sources[module] = module_path.read_text(encoding="utf-8")

    expected_extends = {
        vocabulary_module: (
            "SumeragiV2AsyncNetwork",
            "SumeragiV2Proofs",
        ),
        **{
            module: ("SumeragiV2AsyncTimeoutOwnershipProofs",)
            for module in lower_consumers
        },
        historical_kernel_module: (
            "SumeragiV2AsyncTimeoutOwnershipProofs",
            "TLAPS",
        ),
    }
    forbidden_dependencies = (
        ASYNC_LIVENESS_DEBT_SHARD,
        ASYNC_LIVENESS_FACADE,
    )
    for module, expected in expected_extends.items():
        source = sources.get(module)
        if source is None:
            continue
        actual = _module_extends(source)
        if actual != expected:
            errors.append(
                f"{module}.tla must EXTEND exactly {list(expected)} to remain "
                f"below the proofless async liveness debt; found {list(actual)}"
            )
        stripped = strip_tla_comments(source)
        for forbidden in forbidden_dependencies:
            if re.search(rf"\b{re.escape(forbidden)}\b", stripped):
                errors.append(
                    f"{module}.tla must not depend on {forbidden}; retained-lock "
                    "proof leaves must remain below their async debt consumers"
                )

    temporal_root = "SumeragiV2AsyncTemporalClosureProofs"
    temporal_root_path = formal_dir / f"{temporal_root}.tla"
    if temporal_root_path.is_file():
        import_graph = {
            path.stem: _module_extends(path.read_text(encoding="utf-8"))
            for path in sorted(formal_dir.glob("*.tla"))
        }
        for forbidden in forbidden_dependencies:
            queue: list[tuple[str, tuple[str, ...]]] = [
                (temporal_root, (temporal_root,))
            ]
            visited = {temporal_root}
            found_path: tuple[str, ...] | None = None
            while queue and found_path is None:
                module, module_path = queue.pop(0)
                for dependency in import_graph.get(module, ()):
                    dependency_path = (*module_path, dependency)
                    if dependency == forbidden:
                        found_path = dependency_path
                        break
                    if dependency in import_graph and dependency not in visited:
                        visited.add(dependency)
                        queue.append((dependency, dependency_path))
            if found_path is not None:
                errors.append(
                    f"{temporal_root}.tla must not transitively depend on "
                    f"{forbidden}; found import path {' -> '.join(found_path)}"
                )

    expected_operator_bodies = {
        "StableAvailableRetainedLock": r"""
            /\ gst
            /\ node \in AsyncCurrentResponsiveVoters \cap up
            /\ lockedRound \in Views
            /\ subject \in Subjects
            /\ lockRank[node] = lockedRound
            /\ lockSubject[node] = subject
            /\ BodyHeldBy(durableBodies, node, context, lockedRound, subject)
            /\ RetainedLockedBodyHeldBy(
                 retainedLockedBodies, node, context, subject)
        """,
        "LockedBodyCommittedInOldRound": r"""
            \E qc \in commitQCs:
              /\ qc.context = context
              /\ qc.phase = "Commit"
              /\ qc.view = lockedRound
              /\ qc.subject = subject
              /\ node \in qc.signers
        """,
        "LockedBodyReproposedUnchangedLater": r"""
            \E envelope \in proposalNetwork:
              /\ envelope.proposal.context = context
              /\ envelope.proposal.view > lockedRound
              /\ envelope.proposal.subject = subject
        """,
        "LockedBodyLegitimatelyDecidedOrSuperseded": r"""
            \/ NodeHasDecision(node)
            \/ /\ lockRank[node] > lockedRound
               /\ \E qc \in prepareQCs:
                    /\ qc.context = context
                    /\ qc.phase = "Prepare"
                    /\ qc.view = lockRank[node]
                    /\ qc.subject = lockSubject[node]
        """,
        "LockedBodyReproposalOutcome": r"""
            \/ LockedBodyCommittedInOldRound(node, lockedRound, subject)
            \/ LockedBodyReproposedUnchangedLater(lockedRound, subject)
            \/ LockedBodyLegitimatelyDecidedOrSuperseded(
                 node, lockedRound, subject)
        """,
        "LockedBodyReproposalProgressProperty": r"""
            specification
              => \A node \in ValidatorIds, lockedRound \in Views,
                    subject \in Subjects:
                   StableAvailableRetainedLock(node, lockedRound, subject)
                     ~> LockedBodyReproposalOutcome(node, lockedRound, subject)
        """,
    }
    all_sources = {
        path.stem: path.read_text(encoding="utf-8")
        for path in sorted(formal_dir.glob("*.tla"))
    }
    vocabulary_source = sources.get(vocabulary_module)
    for symbol, expected_body in expected_operator_bodies.items():
        providers = sorted(
            module
            for module, source in all_sources.items()
            if _top_level_operator_body(source, symbol) is not None
        )
        if providers != [vocabulary_module]:
            errors.append(
                f"acyclic liveness vocabulary operator {symbol} must have "
                f"exactly one lower provider {vocabulary_module}.tla; found "
                f"{[provider + '.tla' for provider in providers]}"
            )
        if vocabulary_source is None:
            continue
        extracted = _top_level_operator_body(
            vocabulary_source,
            symbol,
            preserve_string_contents=True,
        )
        if extracted is None:
            continue
        body, line = extracted
        normalized = " ".join(body.split())
        expected_normalized = " ".join(expected_body.split())
        if normalized != expected_normalized:
            errors.append(
                f"{vocabulary_module}.tla:{line}: {symbol} must equal only "
                f"{expected_normalized!r}; found {normalized!r}"
            )

    debt_source = sources.get(ASYNC_LIVENESS_DEBT_SHARD)
    if debt_source is not None:
        for symbol in expected_operator_bodies:
            if _top_level_operator_body(debt_source, symbol) is not None:
                errors.append(
                    f"{ASYNC_LIVENESS_DEBT_SHARD}.tla must not redeclare lower "
                    f"liveness vocabulary operator {symbol}"
                )
    return errors


def _chain_epoch_refinement_shard_contract(
    sources: dict[str, str],
) -> tuple[list[str], dict[str, str]]:
    """Authenticate the bounded physical chain/epoch refinement sequence."""

    errors: list[str] = []
    providers: dict[str, str] = {}
    provider_indices: dict[str, int] = {}
    facade = sources.get(CHAIN_EPOCH_REFINEMENT_FACADE)
    expected_facade = (
        f"---- MODULE {CHAIN_EPOCH_REFINEMENT_FACADE} ----\n"
        f"EXTENDS {CHAIN_EPOCH_REFINEMENT_SHARDS[-1]}\n\n"
        "=============================================================================\n"
    )
    if facade is not None and facade != expected_facade:
        errors.append(
            f"{CHAIN_EPOCH_REFINEMENT_FACADE}.tla must be the exact theorem-free "
            "ledger-facing facade over the final physical refinement shard"
        )

    reconstructed_parts, framing_errors = (
        _chain_epoch_refinement_shard_bodies(sources)
    )
    errors.extend(framing_errors)
    if len(reconstructed_parts) == len(CHAIN_EPOCH_REFINEMENT_SHARDS):
        reconstructed = "".join(reconstructed_parts)
        actual_digest = hashlib.sha256(reconstructed.encode("utf-8")).hexdigest()
        if actual_digest != CHAIN_EPOCH_REFINEMENT_PRE_SPLIT_BODY_SHA256:
            errors.append(
                "chain/epoch refinement shards are not an exact ordered "
                "reconstruction of the reviewed pre-split body: expected "
                f"SHA-256 {CHAIN_EPOCH_REFINEMENT_PRE_SPLIT_BODY_SHA256}, "
                f"found {actual_digest}"
            )

    expected_base_extends = (
        "SumeragiV2AsyncTemporalClosureProofs",
        "TLAPS",
    )
    identifier = re.compile(r"\b[A-Za-z_][A-Za-z0-9_]*\b")
    shard_identifiers: list[set[str]] = []
    for index, module in enumerate(CHAIN_EPOCH_REFINEMENT_SHARDS):
        source = sources.get(module)
        if source is None:
            shard_identifiers.append(set())
            continue
        expected_extends = (
            expected_base_extends
            if index == 0
            else (CHAIN_EPOCH_REFINEMENT_SHARDS[index - 1],)
        )
        actual_extends = _module_extends(source)
        if actual_extends != expected_extends:
            errors.append(
                f"{module}.tla must EXTEND exactly {list(expected_extends)}, "
                f"found {list(actual_extends)}"
            )

        declarations = _top_level_declarations(source)
        theorem_count = sum(kind == "theorem" for _, kind, _, _ in declarations)
        if theorem_count > CHAIN_EPOCH_REFINEMENT_SHARD_MAX_THEOREMS:
            errors.append(
                f"{module}.tla exceeds "
                f"{CHAIN_EPOCH_REFINEMENT_SHARD_MAX_THEOREMS} top-level "
                f"theorems: found {theorem_count}"
            )
        if theorem_count == 0:
            errors.append(
                f"{module}.tla must remain a theorem-bearing physical release root"
            )
        for name, _kind, _start, _end in declarations:
            prior = providers.get(name)
            if prior is not None:
                errors.append(
                    f"chain/epoch refinement declaration {name} is duplicated by "
                    f"{prior}.tla and {module}.tla"
                )
                continue
            providers[name] = module
            provider_indices[name] = index
        shard_identifiers.append(
            set(identifier.findall(strip_tla_comments(source)))
        )

    # Original declaration order is part of the digest.  This additional
    # dependency check proves that no shard relies on a declaration hidden in
    # a later physical root, which would make the textual partition invalid as
    # a sequential EXTENDS chain even though concatenation still matched.
    for index, (module, symbols) in enumerate(
        zip(CHAIN_EPOCH_REFINEMENT_SHARDS, shard_identifiers, strict=True)
    ):
        for symbol in sorted(symbols):
            provider_index = provider_indices.get(symbol)
            if provider_index is not None and provider_index > index:
                errors.append(
                    f"{module}.tla has forward chain/epoch-family reference "
                    f"{symbol} provided by "
                    f"{CHAIN_EPOCH_REFINEMENT_SHARDS[provider_index]}.tla"
                )
    return errors, providers


def _chain_epoch_refinement_source(formal_dir: Path) -> str:
    """Read the virtual pre-split chain/epoch source for source contracts."""

    shard_paths = [
        formal_dir / f"{module}.tla"
        for module in CHAIN_EPOCH_REFINEMENT_SHARDS
    ]
    if all(path.is_file() for path in shard_paths):
        sources = {
            module: path.read_text(encoding="utf-8")
            for module, path in zip(
                CHAIN_EPOCH_REFINEMENT_SHARDS, shard_paths, strict=True
            )
        }
        bodies, errors = _chain_epoch_refinement_shard_bodies(sources)
        if errors:
            raise ValueError("; ".join(errors))
        return (
            f"---- MODULE {CHAIN_EPOCH_REFINEMENT_FACADE} ----\n"
            + "".join(bodies)
            + "=============================================================================\n"
        )
    return (
        formal_dir / f"{CHAIN_EPOCH_REFINEMENT_FACADE}.tla"
    ).read_text(encoding="utf-8")


def _async_liveness_source(formal_dir: Path) -> str:
    """Read the virtual façade source, falling back for compact test fixtures."""

    shard_paths = [formal_dir / f"{module}.tla" for module, _ in ASYNC_LIVENESS_SHARDS]
    if all(path.is_file() for path in shard_paths):
        return "\n".join(path.read_text(encoding="utf-8") for path in shard_paths)
    return (formal_dir / f"{ASYNC_LIVENESS_FACADE}.tla").read_text(encoding="utf-8")


def _facade_provider_entries(
    formal_dir: Path, root_dir: Path = ROOT_DIR
) -> list[dict[str, Any]]:
    """Resolve every ledger-facing façade symbol to its unique physical shard."""

    async_sources = {
        module: (formal_dir / f"{module}.tla").read_text(encoding="utf-8")
        for module, _ in ASYNC_LIVENESS_SHARDS
    }
    async_errors, async_providers = _async_liveness_shard_contract(
        {
            **async_sources,
            ASYNC_LIVENESS_FACADE: (
                formal_dir / f"{ASYNC_LIVENESS_FACADE}.tla"
            ).read_text(encoding="utf-8"),
        }
    )
    if async_errors:
        raise ValueError(
            "invalid async liveness shard contract: " + "; ".join(async_errors)
        )
    chain_sources = {
        module: (formal_dir / f"{module}.tla").read_text(encoding="utf-8")
        for module in CHAIN_EPOCH_REFINEMENT_SHARDS
    }
    chain_errors, chain_providers = _chain_epoch_refinement_shard_contract(
        {
            **chain_sources,
            CHAIN_EPOCH_REFINEMENT_FACADE: (
                formal_dir / f"{CHAIN_EPOCH_REFINEMENT_FACADE}.tla"
            ).read_text(encoding="utf-8"),
        }
    )
    if chain_errors:
        raise ValueError(
            "invalid chain/epoch refinement shard contract: "
            + "; ".join(chain_errors)
        )
    facade_providers = {
        ASYNC_LIVENESS_FACADE: async_providers,
        CHAIN_EPOCH_REFINEMENT_FACADE: chain_providers,
    }
    ledger = load_ledger(formal_dir / "proof_coverage.json")
    obligations = ledger.get("obligations")
    if not isinstance(obligations, list):
        raise ValueError("proof coverage obligations must be an array")
    entries: list[dict[str, Any]] = []
    seen: set[str] = set()
    for obligation in obligations:
        if not isinstance(obligation, dict):
            continue
        ledger_module = obligation.get("module")
        providers = facade_providers.get(ledger_module)
        if providers is None:
            continue
        for symbol in _symbol_names(obligation.get("symbol", "")):
            qualified_symbol = f"{ledger_module}!{symbol}"
            if qualified_symbol in seen:
                continue
            seen.add(qualified_symbol)
            provider = providers.get(symbol)
            if provider is None:
                raise ValueError(
                    f"facade ledger symbol {qualified_symbol} has no unique "
                    "physical shard provider"
                )
            log = (
                _formal_evidence_logical_path("tlaps", f"{provider}.log")
                if provider in RELEASE_PROOF_MODULES
                else None
            )
            entries.append({"symbol": symbol, "module": provider, "log": log})
    return entries
