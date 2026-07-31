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
    "58304eaddaf96ca5b5053c96b6c48603110d3b2b6570b1e05d765db03e7f19dc"
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
    "SumeragiV2AsyncProgressOwnershipProofs": 5_658,
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
    "CandidateProducerContinuationResolutionSplitsReviewedSourceClass",
    "PacketForItemExactRetryRetainsRouteIdentity",
    "SignProposalAtomicallyHandsProducerToSourceFanout",
    "AsyncNextDeferredCommandOwnsOldestLifecycleWithoutHandoff",
    "AsyncDeferredHandoffRetainsExactSelectedLifecycle",
    "AsyncLeaderWireLifecycleSlotUniverseIsFinite",
    "AsyncLeaderWireLifecycleSlotUniverseIsRosterBounded",
    "DormantLeaderWireOwnsNoIngressSchedulerBarrier",
    "AdmitHiddenLeaderWireIsAtomicLocalAcceptanceCut",
    "AdmitFreshLeaderWireFreezesCurrentLocalSchedulerOrdinal",
    "AdmitDormantLeaderWireRetainsLifecycleTokenAndFrozenPrefix",
    "AdmitDormantLeaderWirePreservesLogicalPotentialPredecessors",
    "AtomicDormantLeaderWireAdmissionConsumesRealPacketWithFreshCarrier",
    "AsyncLeaderWireActionInertDormantHasNoExactAdmissionPacket",
    "AsyncLiveServeIngressDuplicateRetainsSchedulerOrdinal",
    "AsyncUnboundChunkAdmissionDoesNotMintLeaderWireLifecycle",
    "AsyncUnboundChunkExactRetryCoalescesWithoutEpisodeGrowth",
    "AsyncHeldChunkReceiptTombstonesExactProducerEpisode",
    "CoalescedDueLeaderWireLifecycleRetryPreservesFrozenOwner",
    "AtomicLeaderWireAdmissionFreezesPrefixBeforeAppend",
    "DirectCommitQcCandidateHasExactImportLineage",
    "CommitCertificateResponseCandidateHasExactImportLineage",
    "CommitImportCausalSuccessorRetainsExactLineage",
    "LeaderWireIngressDrainNeverInventsRuntimeOwner",
    "DeferredRetransmitConsumesDriveProgramCounter",
    "AsyncServeIngressTicketExcludesLaterLocalWork",
    "AsyncLeaderWireIngressTicketExcludesLaterLocalWork",
    "AsyncSelectedLeaderWirePhysicalCarrierDefinesIngressScheduler",
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
    "SameHeightRestartReopensActiveLeaderWireWithoutTerminalizing",
    "SameHeightRestartReopensVolatileLeaderWireTerminal",
    "SameHeightRestartRetainsDormantLeaderWireWithoutBarrier",
    "SameHeightRestartPreservesRestartStableLeaderWireTerminal",
    "SameHeightRestartReopensDurableCertifiedResponseFamily",
    "RuntimeLeaderWireCannotRetireMerelyFromIngressPop",
    "RetireLeaderWireLifecycleRetainsTerminalTombstone",
    "AsyncDormantExactReplyRequestPacketIsRetained",
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
    "AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck",
    "AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor",
    "AsyncCandidateProducerSemanticHandoffRetirementRequiresAck",
    "AsyncCandidateProducerContinuationDecisionReclamationClearsNode",
    "AsyncCandidateLifecycleProducerContinuationCoverageUsesInheritedToken",
    "LeaderWireIgnoredOrServicedLastConsumerTerminalizesAtomically",
    "AsyncCandidateProducerContinuationKindPartition",
    "AsyncCandidateProducerContinuationRemovedReplayClassification",
    "LeaderWireDeliveryCandidateInheritsAdmissionSchedulerOrdinal",
    "AsyncDormantLeaderWireReactivationConsumesNoFreshHighWatermark",
    "AsyncTargetNeutralLifecycleOwnerCarrierIsFinite",
    "AsyncTargetNeutralLifecycleEpisodeBudgetIsFiniteAndCoalesced",
    "AsyncTargetNeutralLifecycleDiscoveryStrictlyConsumesBudget",
    "AsyncTargetNeutralLifecycleBudgetOrderingIsWellFounded",
    "AsyncCandidateLifecycleReviewedTokenOwnsOneOrigin",
    "AsyncControlServiceTransitionConsumesFreshLeaderWireSchedulerOrdinal",
    "AsyncControlServiceTransitionPreservesSemanticHandoffCoverage",
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
    "CertifiedResponseExactRetryKeepsOneClaimOrdinal",
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
    "AsyncFixedCorridorDeadlineActionErasureIsExact",
    "AsyncOriginalStepHasFixedCorridorDeadlineReceiptExtension",
    "AsyncServiceActivationActionsRefineAsyncNext",
    "AsyncTimeoutLifecycleDueTransitionMintsBeforeLaterAdmissions",
    "AsyncTimeoutLifecycleOrdinalPersistsUntilEndpoint",
    "AsyncTimeoutLifecycleOrdinalClearsOnlyAtEndpoint",
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
    "AsyncCandidateLifecycleRolloverStartsWithRootOwners",
    "AsyncLiveSpecUsesRepresentativePeerCount",
    "AsyncFiniteLiveSpecUsesRepresentativePeerCount",
    "AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain",
    "AsyncServeIngressSchedulerOrdinalIsTyped",
    "AsyncServeIngressSharedSchedulerInitIsEmptyAndInjected",
    "AsyncRetransmitProgramCounterIsBounded",
    "CertifiedResponseClaimsShareOutstandingRequestCharge",
    "CertifiedResponseFamilyLocalClaimsRemainPhysicallySerialized",
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
    "SumeragiV2AdequateLeaderProducerTransportClosureProofs",
    "SumeragiV2AdequateLeaderSelectedOwnerContinuationProofs",
    "SumeragiV2AdequateLeaderCorridorEntryContinuationProofs",
    "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
    "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
)

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
)
LIVENESS_OWNERSHIP_MUTATION_RUNNER = (
    "scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh"
)
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
        "556a595942ccc943ffc0acc896ca967c70f9c363fda24b55bfa235f17e85ad10"
    ),
}
SERVE_SCHEDULER_ORDINAL_RUNNER_SECTION_SHA256 = (
    (
        "preflight and SANY execution",
        "if (($#)); then",
        "assert_nonzero_state_space() {",
        "cd4e26166d5fefa8422eb53db4bbe68a6304f75b73d9b1c32be9bf7a94525d39",
    ),
    (
        "nonzero state-space checker",
        "assert_nonzero_state_space() {",
        "run_tlc() {",
        "37270a8ab03ab36ace3d8582867a16bdc00b93072b2be85f01f0c7f89a1ae6fe",
    ),
    (
        "TLC status and terminal-marker checker",
        "run_tlc() {",
        'shared_fixed_log="$(run_tlc shared-ordinal-fixed "$SHARED_FIXED_CONFIG" 0)"',
        "2aadc6b5c6b1a8949af6540d9d0cd9c59bc4d4c1b565439a375f71b22ca8e943",
    ),
    (
        "six-repaired then six-mutant execution tail",
        'shared_fixed_log="$(run_tlc shared-ordinal-fixed "$SHARED_FIXED_CONFIG" 0)"',
        None,
        "32263ec888f11ba49a9ae953043ec71ec06da855f2524fa1e271ae79c896d15c",
    ),
)
SERVE_SCHEDULER_ORDINAL_MUTATION_FORMAL_GLOBS = (
    "SumeragiV2ServeSchedulerOrdinalMutation*.tla",
    "serve_scheduler_*.cfg",
)
SERVE_SCHEDULER_ORDINAL_RELEASE_SOURCE_SHA256 = {
    "SumeragiV2AsyncNetwork.tla": (
        "4486937d3ffe88832a4d5f113ac87875cffc1970227c1579714f77864b4229fd"
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
        "129dbbcb237a97a636f0a52cfba725be87eee5dab6fa0833852891181271a899"
    ),
}
COMMIT_IMPORT_PROVENANCE_MUTATION_FORMAL_GLOBS = (
    "SumeragiV2CommitImportProvenanceMutation*.tla",
    "commit_import_provenance_*.cfg",
    "commit_import_successor_*.cfg",
)
COMMIT_IMPORT_PROVENANCE_RELEASE_SOURCE_SHA256 = {
    "SumeragiV2AsyncNetwork.tla": (
        "313fb65887b91ae0e9b340c8cab7130ad1ea016727845d78d6c5fb28b4709bea"
    ),
    "SumeragiV2HistoricalRecoveryTemporalClosureProofs.tla": (
        "f9a5d50bef36931cb48591f6adc2b624c14872338ccba861574d7010bd567621"
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
        "f9db394da086d89622c1459a7dd497c495b52774acacb1b498c1dd3efd60769e"
    ),
    "scripts/formal/run_sumeragi_v2_adequate_leader_readiness_mutations.sh": (
        "e219267868924ac2318757007855d090f26d3613d69f5b0894ec50fe0180757b"
    ),
    "scripts/formal/run_sumeragi_v2_apply_authority_mutation.sh": (
        "1ca1f4aa0c48e43a8d671959621359fe935e8f3e641ac9a45c9ed71edfe40e84"
    ),
    "scripts/formal/run_sumeragi_v2_candidate_restart_mutation.sh": (
        "a57cbd496d759f43f1082cdce6e65a69a241f2a3abfb25f0ea6a931c72fdc4dd"
    ),
    "scripts/formal/run_sumeragi_v2_certificate_ref_recovery_mutation.sh": (
        "b9b428904d383c42d1a7777ac33ec7fd0922ea5bca17f046883b8ab47b280f8d"
    ),
    "scripts/formal/run_sumeragi_v2_certified_response_identity_separation_mutation.sh": (
        "8f527b34e8e0bfc805aa308eafc1f0c76a07e87e77f73d5e4071bb436f471e1b"
    ),
    "scripts/formal/run_sumeragi_v2_certified_response_registration_mutation.sh": (
        "c0194e00332dd0b9dfcee1298a04dc45821b192913e1156a24f4938f4690b120"
    ),
    "scripts/formal/run_sumeragi_v2_certified_response_source_lineage_mutation.sh": (
        "8c2ebdb33f53f243e33c611dede252b42165da9e0f81fa89d75d9298b9cfda8f"
    ),
    "scripts/formal/run_sumeragi_v2_commit_import_provenance_mutations.sh": (
        "129dbbcb237a97a636f0a52cfba725be87eee5dab6fa0833852891181271a899"
    ),
    "scripts/formal/run_sumeragi_v2_decision_recovery_lifecycle_mutation.sh": (
        "afec183a6cc8c5d1ff118e5e1676eb10877c0b43d6580fdfdd8574fa3b2a7932"
    ),
    "scripts/formal/run_sumeragi_v2_effect_capacity_ownership_mutation.sh": (
        "0c521f2d552215a2112254990d614b0e205fd134ddf2c78ce7525e1146efc137"
    ),
    "scripts/formal/run_sumeragi_v2_historical_discovery_occurrence_rank_mutation.sh": (
        "46cc895ff6a6cbd410ab557dd887c264831fd65d5162df107d9736e71f2b9bc3"
    ),
    "scripts/formal/run_sumeragi_v2_indexed_height_mutation.sh": (
        "260779cbee48cb30ebc35573d3d6f5f86520487f34463cd1b8450ab8793d0c09"
    ),
    "scripts/formal/run_sumeragi_v2_indexed_service_activation_mutations.sh": (
        "aa819a23d9ec07154c9a60534fcd191a4e39674ca197f6452b1e37480c58ef6f"
    ),
    "scripts/formal/run_sumeragi_v2_inflight_first_release.sh": (
        "65702851d065468bc417ee2dc28feaef00cfc5f439b229a9643038d10e4de58c"
    ),
    "scripts/formal/run_sumeragi_v2_ingress_causal_freshness_mutation.sh": (
        "0b18255d2f7124f60097267b269f17a835ab20ea1cc8dec742e37b5b8adc4d14"
    ),
    "scripts/formal/run_sumeragi_v2_item_carrier_typing_mutation.sh": (
        "320f160009a54db1529e8e03290486931076ab778cfecb01431912fe3ca7c1d6"
    ),
    "scripts/formal/run_sumeragi_v2_liveness_ownership_mutations.sh": (
        "7d7c3e43d9ac792c3cb648ead7fef3a602cfa4df9818239987834d1227f846fe"
    ),
    "scripts/formal/run_sumeragi_v2_multilane_mutations.sh": (
        "3c6c0a192bcba0ec20519f913df82c1287d621d0a40ef2066dc8434ed62bd660"
    ),
    "scripts/formal/run_sumeragi_v2_persist_install_generation_mutation.sh": (
        "0a6edde7fd8f827a7af32005fc2436a3370d02d00801e7a0a974477f6a8f554f"
    ),
    "scripts/formal/run_sumeragi_v2_persist_install_validation_mutation.sh": (
        "e0d59013a307e6693d447ab8ef80f4b3a67fcbb43bd5161eb99ec4ee90969e2e"
    ),
    "scripts/formal/run_sumeragi_v2_post_decision_timeout_mutation.sh": (
        "a0ff98fc89be2c5095b0c98e7a56fde83f48dd9b9ef53496d2603dc395080d03"
    ),
    "scripts/formal/run_sumeragi_v2_productive_mutation.sh": (
        "a1dd1eb6d9e9c925210c55ba0ced6b63669cc0b1f33f4a8319e6645f5cdb8de5"
    ),
    "scripts/formal/run_sumeragi_v2_progress_mutations.sh": (
        "80c4d36d5aef2aed9d5ddd500cc3500344e7cf8b044a329dbe815b65689daa17"
    ),
    "scripts/formal/run_sumeragi_v2_replay_locked_body_carrier_mutation.sh": (
        "04cf6200d9a8f0db91518087b3918c6037ab92b56ccb5e86ae9bda4f77c9fb14"
    ),
    "scripts/formal/run_sumeragi_v2_reply_writer_deadline_mutations.sh": (
        "b1f66896acf7b68d2e2648bfd73e23d69d430ad50a475047b07811e30da06aa0"
    ),
    "scripts/formal/run_sumeragi_v2_restart_locked_fetch_order_mutation.sh": (
        "2647986ff71237568e0e4a6f08e2e906006cbd579bf61c7891ec3eb6036ac675"
    ),
    "scripts/formal/run_sumeragi_v2_serve_scheduler_ordinal_mutations.sh": (
        "878b9b930f1b348d4122a9323dce4d0a08b8899418d3e3f41e8bd451f2ae1274"
    ),
    "scripts/formal/run_sumeragi_v2_service_rank_mutation.sh": (
        "2628cc89a4e15ae1981ce0822a6133f5248b779a1e494f4eadfce1c58f8bbd88"
    ),
    "scripts/formal/run_sumeragi_v2_tlc.sh": (
        "30b3b26e59a8f0d1792211f6d987498dee848f48865151c7fc36933f83e65abd"
    ),
    "scripts/formal/run_sumeragi_v2_typed_rollover_handoff_mutations.sh": (
        "b3cbbcf89f2706b63aca2b5ea78ed118107b881b46f7ca293858ffb8d7f98ed2"
    ),
}
SHARED_TLC_RESULT_BRANCH_PROFILES = {
    "scripts/formal/check_sumeragi_v2_replay_trace.sh": (0, 0, 0, 0, 1),
    "scripts/formal/run_sumeragi_v2_adequate_leader_readiness_mutations.sh": (
        6,
        5,
        0,
        1,
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
        1,
        1,
        0,
        0,
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
        17,
        6,
        0,
        11,
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
            0,
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
            17,
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
    "a9aac15ecae31c9acc872efc62338fa3ae80165288d224f1dfb156cdcecc6458"
)
SHARED_TLC_RESULT_ASSERTION_SITE_PROFILES_SHA256 = (
    "c0f77106efde5d9d5a7158e15d988a387ca90a5732f899e6ba052ce2ff3d6346"
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
    LIVENESS_OWNERSHIP_MUTATION_RUNNER: (
        "7d7c3e43d9ac792c3cb648ead7fef3a602cfa4df9818239987834d1227f846fe"
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
        "aa819a23d9ec07154c9a60534fcd191a4e39674ca197f6452b1e37480c58ef6f"
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
        "82075c8d6b565dfa18f0f7582aa3a3c4da1fae3b0cf8ef95c7f91ab214190f2b"
    ),
}
HISTORICAL_DISCOVERY_OCCURRENCE_RANK_MUTATION_FORMAL_GLOBS = (
    "SumeragiV2HistoricalDiscoveryOccurrenceRankMutation*.tla",
    "historical_discovery_occurrence_rank_*.cfg",
    "historical_discovery_plain_minimum_*.cfg",
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
        "e89b0abf648979dcf7a3263c787e848bc3a7248c461c798d65c247f3b0d431a1"
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
