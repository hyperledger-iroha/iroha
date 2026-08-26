# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

# The replenishment-lasso repair has a deliberately named source surface.
# Keeping this list code-owned makes deleting or renaming one of the ownership
# operators a checker failure even while the full normalized operator bodies
# are pinned below.  In particular, an arbitrary proof parameter must never
# return as a substitute for durable protocol state.
SERVE_LIFECYCLE_REQUIRED_OPERATORS = {
    "SumeragiV2AsyncNetwork": (
        "AsyncServeLogicalRequestIdentity",
        "AsyncServeAdmissionOrdinal",
        "ReserveExactServeCapacityVia",
        "ReserveExactServeCapacity",
        "AdvanceExactServeCapacityVia",
        "AdvanceExactServeCapacity",
        "AsyncServeLifecycleOwned",
        "AsyncServeLifecycleTombstone",
        "AsyncServeFrozenPredecessorSet",
        "AsyncServeEarlierLiveReservationIdentities",
        "AsyncServeOffQueueReservations",
        "AsyncServeIngressLiveReservations",
        "AsyncServeIngressIdentityFrozenByReservation",
        "AsyncServePreexistingIngressBarrierIdentities",
        "AsyncServeSingularOffQueueBarrierInvariant",
        "AsyncServeLifecycleTypeInvariant",
        "AsyncServeTombstoneOutputMatchesIdentity",
        "AsyncServeTombstoneOutputBindingInvariant",
        "AsyncRetransmitLifecyclePhysicalCut",
        "AsyncRetransmitLifecyclePhysicalCutForStep",
        "AsyncEffectiveRetransmitLifecyclePhysicalCut",
        "AsyncCandidateProducerContinuationMayPrecedeOwnedRetransmit",
        "AsyncRetransmitPriorityPrecedesCandidate",
    ),
    "SumeragiV2ExactDecisionStageServiceClosureProofs": (
        "ExactDecisionRequestLifecycleFrozenPredecessorSet",
        "ExactDecisionRequestIngressProducerEpisodeOwnerSet",
        "ExactDecisionRequestIngressProducerEpisodeBudget",
        "ExactDecisionRequestLifecycleIngressRank",
        "ExactDecisionRequestLifecycleStepClassification",
        "ExactDecisionRequestFrozenServeBarrierIdentities",
        "ExactDecisionRequestFrozenServeBarrierIdentity",
        "ExactDecisionRequestFrozenServeBarrierMaterializationAction",
        "ExactDecisionRequestLifecycleConcreteFairOwnerKinds",
        "ExactDecisionRequestLifecycleIoOwnerRequired",
        "ExactDecisionRequestLifecycleConcreteFairOwner",
        "ExactDecisionRequestLifecycleConcreteFairAction",
        "ExactDecisionRequestLifecycleSelectedConcreteFairAction",
        "ExactDecisionRequestLifecycleRankCellOutcome",
        "ExactDecisionRequestLifecycleRankCellClosureProperty",
        "ExactDecisionRequestLifecycleFiniteProducerEpisodeClosureProperty",
        "ExactDecisionRequestLifecycleConcreteActionOriginProperty",
        "ExactDecisionRequestLifecycleRankDescentProperty",
        "ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty",
        "ExactDecisionRequestIngressRankReplenishmentResidual",
        "ExactDecisionRequestRuntimePrefixSnapshot",
        "ExactDecisionRequestClockPrefixSnapshotBinding",
        "ExactDecisionRequestOwnedRuntimeAtRank",
    ),
    "SumeragiV2AdequateLeaderServiceClosureProofs": (
        "AdequateLeaderFrozenTargetCandidateRole",
        "AdequateLeaderCandidatePayloadWithinFrozenView",
        "AdequateLeaderFrozenTargetCandidateIdentity",
        "AdequateLeaderFrozenCommitRequestItemPayload",
        "AdequateLeaderFrozenCandidatePayload",
        "AdequateLeaderImmutableCandidatePayload",
        "AdequateLeaderFrozenQcRecordCarrier",
        "AdequateLeaderFrozenVoteRecordCarrier",
        "AdequateLeaderFrozenTimeoutVoteRecordCarrier",
        "AdequateLeaderFrozenTcRecordCarrier",
        "AdequateLeaderFrozenProposalRecordCarrier",
        "AdequateLeaderFrozenBodyRecordCarrier",
        "AdequateLeaderFrozenBodyEnvelopeCarrier",
        "AdequateLeaderFrozenCertifiedRequestItemCarrier",
        "AdequateLeaderFrozenCertifiedRequestHashCarrier",
        "AdequateLeaderFrozenCommitRequestItemCarrier",
        "AdequateLeaderFrozenNetworkItemCarrier",
        "AdequateLeaderFrozenEvidenceCarrier",
        "AdequateLeaderFrozenCandidateItemPayloadCarrier",
        "AdequateLeaderFrozenCandidateEvidencePayloadCarrier",
        "AdequateLeaderFrozenCandidatePayloadCarrier",
        "AdequateLeaderFrozenCandidateOwnerIdentityFromPayload",
        "AdequateLeaderFrozenTargetWireIdentity",
        "AdequateLeaderFrozenWirePayloadIdentity",
        "AdequateLeaderFrozenWirePayloadCarrier",
        "AdequateLeaderFrozenWireOwnerIdentityFromCoordinates",
        "AdequateLeaderFrozenCandidateOwnerUniverse",
        "AdequateLeaderFrozenWireOwnerUniverse",
        "AdequateLeaderFrozenOwnerUniverse",
        "AdequateLeaderTargetLiveCandidateOwnerIdentitySet",
        "AdequateLeaderTargetLiveWireOwnerIdentitySet",
        "AdequateLeaderTargetLiveOwnerIdentitySet",
        "AdequateLeaderPeriodicLifecyclePredecessorOwned",
        "AdequateLeaderProtectedPeriodicLifecycleOwned",
        "AdequateLeaderProtectedPeriodicOwnerIdentity",
        "AdequateLeaderProtectedPeriodicSnapshotIdentity",
        "AdequateLeaderProtectedPeriodicIdentityActive",
        "AdequateLeaderProtectedPeriodicSnapshot",
        "AdequateLeaderProtectedPeriodicRetirementReceipt",
        "AdequateLeaderProtectedPeriodicSnapshotRetired",
        "AdequateLeaderProtectedPeriodicSnapshotDrained",
        "AdequateLeaderProtectedPeriodicIdentityStage",
        "AdequateLeaderProtectedPeriodicSnapshotTokens",
        "AdequateLeaderProtectedPeriodicSnapshotBudget",
        "AdequateLeaderProtectedPeriodicEpisodeResidual",
        "AdequateLeaderProtectedPeriodicEpisodeGoal",
        "AdequateLeaderProtectedPeriodicEpisodeClosureProperty",
        "AdequateLeaderTargetOccurrenceAwaitingFiniteEpisode",
        "AdequateLeaderTargetOccurrenceFiniteEpisodeOrExitGoal",
        "AdequateLeaderTargetPeriodicPrefixThenFiniteEpisodeProperty",
        "AdequateLeaderCandidateProducerContinuationRetirementMemory",
        "AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet",
        "AdequateLeaderTargetDurablyRetiredOwnerIdentitySet",
        "AdequateLeaderTargetEqualCountOwnerReplacementAction",
        "AdequateLeaderTargetCountIncreasingReplenishmentAction",
        "AdequateLeaderTargetCandidateIdentityTombstoneProperty",
        "AdequateLeaderTargetCandidateSuccessfulServiceMemoryProperty",
        "AdequateLeaderTargetCandidateTerminalTombstoneProperty",
        "AdequateLeaderTargetEpisodeKnownOwnerSet",
        "AdequateLeaderTargetNonDescentDiscoveredOwnerIdentitySet",
        "AdequateLeaderTargetNonDescentEpisodeResidual",
        "AdequateLeaderTargetNonDescentEpisodeBudget",
        "AdequateLeaderTargetNonDescentEpisodeFrontier",
        "AdequateLeaderTargetNonDescentEpisodeAtBudget",
        "AdequateLeaderTargetProducerTransportOccurrenceClosureProperty",
        "AdequateLeaderTargetOccurrenceRankServiceProperty",
        "AdequateLeaderTargetNonDescentKnownAdvanceProperty",
        "AdequateLeaderTargetNonDescentEpisodeBudgetFrontier",
        "AdequateLeaderTargetNonDescentEpisodeBudgetDescentProperty",
        "AdequateLeaderTargetNonDescentEpisodeClosureProperty",
        "AdequateLeaderTargetComposedRankDescentProperty",
        "AdequateLeaderTargetOffSubjectControlOccurrenceIdentity",
        "AdequateLeaderTargetSameOrLowerControlRetry",
        "AdequateLeaderTargetSameOrLowerControlRetriesAdmissionBlocked",
        "AdequateLeaderTargetOffSubjectControlCandidateOwnerIdentity",
        "AdequateLeaderTargetOffSubjectControlRetirementMemory",
        "AdequateLeaderTargetOffSubjectControlRetirementClosed",
        "AdequateLeaderTargetOffSubjectControlClosedOwnerIdentitySet",
        "AdequateLeaderTargetOffSubjectControlNoReentryProperty",
        "AdequateLeaderTargetOccurrenceOwnerIdentitySet",
        "AdequateLeaderTargetOccurrenceOwnerSelected",
        "AdequateLeaderTargetOccurrenceOwnerRetirementClosed",
        "AdequateLeaderTargetProductiveSubjectOpenFrontier",
        "AdequateLeaderTargetProductiveSubjectReentryGoal",
        "AdequateLeaderTargetCarriedOwnerEpisodeAtBudget",
        "AdequateLeaderTargetSubjectSwitchEpisodeAtBudget",
        "AdequateLeaderTargetProductiveOwnerEpisodeAtBudget",
        "AdequateLeaderTargetOffSubjectRetirementAndReentryGoal",
        "AdequateLeaderTargetSubjectSwitchDiscoveredOwnerSet",
        "AdequateLeaderTargetSubjectSwitchEpisodeAdvanceGoal",
        "AdequateLeaderTargetSubjectSwitchBudgetDescentGoal",
        "AdequateLeaderTargetSubjectSwitchCarryStepProperty",
        "AdequateLeaderTargetAnchoredSubjectSwitchBudgetFrontier",
        "AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentGoal",
        "AdequateLeaderTargetAnchoredSubjectSwitchBudgetDescentProperty",
        "AdequateLeaderTargetProductiveOccurrenceServiceGoal",
        "AdequateLeaderTargetOffSubjectOccurrenceDrainGoal",
        "AdequateLeaderTargetUniversalOccurrenceServiceGoal",
        "AdequateLeaderTargetOccurrenceRankOwnerServiceExitGoal",
        "AdequateLeaderTargetOccurrenceRankServiceExitGoal",
        "AdequateLeaderTargetSubjectSwitchBudgetDescentProperty",
        "AdequateLeaderTargetSubjectSwitchClosureProperty",
        "AdequateLeaderTargetRankServiceExitProperty",
    ),
}
SERVE_LIFECYCLE_REQUIRED_OPERATOR_TOKENS = {
    (
        "SumeragiV2AsyncNetwork",
        "AsyncServeTombstoneOutputBindingInvariant",
    ): (
        "asyncServeTombstones",
        "AsyncServeTombstoneOutputMatchesIdentity",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncServeEarlierLiveReservationIdentities",
    ): (
        "AsyncServeLifecycleOwned",
        "AsyncServeAdmissionOrdinal",
        "AsyncServeJobQueued",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "ReserveExactServeCapacityVia",
    ): (
        "AsyncServeSourceAttemptRecords",
        "AsyncServeOffQueueReservations",
        "AsyncServeIngressPredecessorCounts",
        "asyncNextServeAdmissionOrdinal",
        "asyncServeAttempts",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "ReserveExactServeCapacity",
    ): ("ReserveExactServeCapacityVia",),
    (
        "SumeragiV2AsyncNetwork",
        "AdvanceExactServeCapacityVia",
    ): (
        "AsyncServeSourceAttemptRecords",
        "AsyncServeOffQueueReservations",
        "AsyncServeFamilyTombstoneRecords",
        "AsyncServeIngressPredecessorCounts",
        "asyncNextServeAdmissionOrdinal",
        "asyncServeAttempts",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AdvanceExactServeCapacity",
    ): ("AdvanceExactServeCapacityVia",),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncServeIngressIdentityFrozenByReservation",
    ): (
        "AsyncIngressSources",
        "reservation.ingressPredecessors",
        "AsyncReplyRequestKinds",
        "AsyncServeLogicalRequestIdentity",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncServePreexistingIngressBarrierIdentities",
    ): (
        "AsyncServeIngressLiveReservations",
        "AsyncServeIngressIdentityFrozenByReservation",
        "owned.identity",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncServeSingularOffQueueBarrierInvariant",
    ): (
        "ValidatorIds",
        "Cardinality",
        "AsyncServeOffQueueReservations",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncServeLifecycleTypeInvariant",
    ): ("AsyncServeSingularOffQueueBarrierInvariant",),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncRetransmitLifecyclePhysicalCut",
    ): ("asyncControlServiceState.retransmitLifecyclePhysicalCut",),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncRetransmitLifecyclePhysicalCutForStep",
    ): (
        "state.retransmitLifecyclePhysicalCut",
        "AsyncNextIngressPhysicalOrdinal",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncEffectiveRetransmitLifecyclePhysicalCut",
    ): (
        "AsyncRetransmitLifecycleOwned",
        "AsyncRetransmitLifecyclePhysicalCut",
        "AsyncNextIngressPhysicalOrdinal",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateProducerContinuationMayPrecedeOwnedRetransmit",
    ): (
        "record.sourcePhysicalOrdinal",
        "AsyncRetransmitLifecyclePhysicalCut",
        "record.ordinal",
        "AsyncRetransmitLifecycleOrdinal",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncRetransmitPriorityPrecedesCandidate",
    ): (
        "AsyncRetransmitLifecyclePhysicalCut",
        "AsyncCandidateLifecycleSourcePhysicalOrdinal",
        "AsyncRetransmitLifecycleOrdinal",
        "AsyncCandidateLifecycleOrdinal",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleFrozenPredecessorSet",
    ): (
        "AsyncServeFrozenPredecessorSet",
        "AsyncServeIngressAdmissionPredecessorDebtSlots",
        "AsyncServePreexistingIngressOwnerPredecessorDebtSet",
        "AsyncServePreexistingIngressBarrierPredecessorDebtSet",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestRuntimePrefixSnapshot",
    ): (
        "schedulerCeiling",
        "physicalCut",
        "ExactDecisionRequestRuntimeCandidateOriginsAt",
        "ExactDecisionRequestRuntimeServeSourcesAt",
        "ExactDecisionRequestRuntimeContinuationSourcesAt",
        "ExactDecisionRequestRuntimeLeaderWireIdentitiesAt",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestClockPrefixSnapshotBinding",
    ): (
        'kind = "Owned"',
        "snapshot.schedulerCeiling",
        "ownerOrdinal",
        "snapshot.physicalCut",
        "AsyncRetransmitLifecyclePhysicalCut",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestOwnedRuntimeAtRank",
    ): (
        "ExactDecisionRequestOwnedRetransmitEpisode",
        "ExactDecisionRequestClockPrefixSnapshotBinding",
        '"Owned"',
        "ExactDecisionRequestRuntimeFrozenPrefixDrained",
        "ExactDecisionRequestOwnedRuntimeRank",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestFrozenServeBarrierIdentities",
    ): (
        "AsyncServePreexistingIngressBarrierIdentities",
        "ExactDecisionServeLifecycleIdentity",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestFrozenServeBarrierIdentity",
    ): (
        "CHOOSE",
        "ExactDecisionRequestFrozenServeBarrierIdentities",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestFrozenServeBarrierMaterializationAction",
    ): (
        "ExactDecisionRequestFrozenServeBarrierIdentities",
        "PostGstRunNode",
        "DrainFairIngressSelected",
        "PostGstRunHistoricalServer",
        "DrainHistoricalIngressSelected",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleRankDescentProperty",
    ): (
        "ExactDecisionRequestLifecycleStepClassification",
        "ExactDecisionRequestLifecycleConcreteActionOriginProperty",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleConcreteActionOriginProperty",
    ): (
        "ExactDecisionRequestLifecycleConcreteFairOwnerKinds",
        "ExactDecisionRequestLifecycleSelectedConcreteFairAction",
        "ExactDecisionRequestLifecycleRankCellOutcome",
        "ENABLED",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestAdmissionCoalescingOutcomeConvergenceProperty",
    ): ("ExactDecisionRequestLifecycleRankDescentProperty",),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestIngressRankReplenishmentResidual",
    ): (
        "ExactDecisionRequestIngressCausalReplenishmentResidual",
        "ExactDecisionRequestIngressServeReplenishmentResidual",
        "ExactDecisionRequestIngressPriorityReplenishmentResidual",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenOwnerUniverse",
    ): (
        "AdequateLeaderFrozenCandidateOwnerUniverse",
        "AdequateLeaderFrozenWireOwnerUniverse",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderProtectedIngressLifecycleOwned",
    ): (
        "AsyncLeaderWireIngressOwnsSharedPhysicalTurn",
        "AsyncLeaderWireEarliestPhysicalIngressRecord",
        "AsyncEffectiveTimeoutLifecycleOrdinal",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderProtectedIngressLifecyclePrecedesTimeout",
    ): (
        "AsyncSelectedLeaderWirePhysicalCarrierDefinesIngressScheduler",
        "AdequateLeaderProtectedIngressLifecycleOwned",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCommitRequestItemPayload",
    ): (
        "source",
        "item.source",
        "item.envelope",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderImmutableCandidatePayload",
    ): (
        "AsyncCandidateSemanticStatement",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenNetworkItemCarrier",
    ): (
        "AdequateLeaderFrozenCertifiedRequestItemCarrier",
        "AdequateLeaderFrozenCommitRequestItemCarrier",
        "AdequateLeaderFrozenQcRecordCarrier",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidatePayloadCarrier",
    ): (
        "context",
        "round",
        "proposalRound",
        "subject",
        "AsyncCandidateSemanticPhases",
        "executionCommitment",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenCandidateOwnerUniverse",
    ): (
        "AdequateLeaderFrozenCandidateOwnerIdentityFromPayload",
        "AdequateLeaderFrozenCandidatePayloadCarrier",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetNonDescentEpisodeBudget",
    ): (
        "Cardinality",
        "AdequateLeaderFrozenOwnerUniverse",
        "known",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetCandidateIdentityTombstoneProperty",
    ): (
        "AdequateLeaderCandidateFrozenIdentityBudgetBridgeProperty",
        "AdequateLeaderTargetCandidateServicedRetirementAction",
        "AdequateLeaderServicedCandidateMemory",
        "AdequateLeaderServicedCandidateClosure",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetCandidateSuccessfulServiceMemoryProperty",
    ): (
        "AdequateLeaderTargetCandidateSuccessfulServiceRetirementAction",
        "AdequateLeaderServicedCandidateMemory",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetCandidateTerminalTombstoneProperty",
    ): (
        "AdequateLeaderCandidateFrozenIdentityBudgetBridgeProperty",
        "AdequateLeaderTargetCandidateTerminalDiscardRetirementAction",
        "AdequateLeaderServicedCandidateMemory",
        "AdequateLeaderServicedCandidateClosure",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetNonDescentEpisodeClosureProperty",
    ): (
        "AdequateLeaderTargetNonDescentEpisodeBudgetFrontier",
        "AdequateLeaderTargetProtocolSubjectSource",
        "AdequateLeaderTargetOccurrenceRankServiceExitGoal",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderPeriodicLifecyclePredecessorOwned",
    ): (
        "AsyncTimeoutLifecycleOwned",
        "AsyncOlderRetransmitLifecycleBlocksTimeout",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderProtectedPeriodicLifecycleOwned",
    ): (
        "AdequateLeaderPeriodicLifecyclePredecessorOwned",
        "AsyncOlderCandidateLifecycleBlocksRetransmit",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderProtectedPeriodicOwnerIdentity",
    ): (
        "AdequateLeaderCorridorAuthorityReceipt",
        "retransmitOrdinal",
        "timeoutOrdinalCeiling",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderProtectedPeriodicSnapshot",
    ): (
        "AdequateLeaderProtectedPeriodicOwnerIdentity",
        "AdequateLeaderPeriodicLifecyclePredecessorOwned",
        "AsyncRetransmitLifecycleOrdinal",
        "AsyncTimeoutLifecycleOrdinal",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderProtectedPeriodicRetirementReceipt",
    ): (
        "AsyncRetransmitLifecycleOwned",
        "AsyncRetransmitLifecycleOrdinal",
        "AsyncNextCandidateLifecycleOrdinal",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderProtectedPeriodicSnapshotDrained",
    ): (
        "AdequateLeaderProtectedPeriodicSnapshotRetired",
        "AdequateLeaderProtectedPeriodicSnapshot",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderProtectedPeriodicEpisodeGoal",
    ): (
        "AdequateLeaderTargetOccurrenceRankServiceExitGoal",
        "AdequateLeaderProtectedPeriodicSnapshotDrained",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetPeriodicPrefixThenFiniteEpisodeProperty",
    ): (
        "AdequateLeaderTargetOccurrenceAwaitingFiniteEpisode",
        "AdequateLeaderTargetOccurrenceFiniteEpisodeOrExitGoal",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetComposedRankDescentProperty",
    ): (
        "AdequateLeaderProtectedPeriodicEpisodeClosureProperty",
        "AdequateLeaderTargetOccurrenceRankServiceProperty",
        "AdequateLeaderTargetProducerTransportOccurrenceClosureProperty",
        "AdequateLeaderTargetNonDescentKnownAdvanceProperty",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOffSubjectControlRetirementMemory",
    ): (
        "AdequateLeaderTargetOffSubjectControlCandidateOwnerIdentity",
        "AdequateLeaderTargetLiveCandidateOwnerIdentitySet",
        "AsyncControlServiceOccurrenceIsCurrentOwner",
        "AsyncControlServiceIdentityServicedOrAdvanced",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOffSubjectControlNoReentryProperty",
    ): (
        "gst",
        "AdequateLeaderTargetOffSubjectControlOccurrenceIdentity",
        "AdequateLeaderTargetOffSubjectControlRetirementMemory",
        "AdequateLeaderTargetOffSubjectControlRetirementClosed",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOccurrenceOwnerSelected",
    ): (
        "AdequateLeaderTargetOccurrenceRankFrontier",
        "AdequateLeaderTargetOccurrenceOwnerIdentitySet",
        "owner",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCandidateProducerContinuationRetirementMemory",
    ): (
        "AsyncCandidateProducerContinuationTerminalForIdentity",
        "AsyncCandidateServiceIdentity",
        "AsyncCandidateProducerContinuations",
        "record.node",
        "candidate.node",
        "record.context",
        "candidate.consumerContext",
        "record.height",
        "candidate.height",
        "record.address.stage",
        "AsyncCandidateServiceStageForKind",
        "record.view",
        "candidate.view",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet",
    ): (
        "AdequateLeaderFrozenCandidateOwnerIdentity",
        "AsyncCandidateSet",
        "AdequateLeaderTargetSemanticRankCarrier",
        "AdequateLeaderFrozenTargetCandidateIdentity",
        "AdequateLeaderCandidateProducerContinuationRetirementMemory",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOccurrenceOwnerRetirementClosed",
    ): (
        "AdequateLeaderFrozenCandidateOwnerUniverse",
        "AdequateLeaderTargetLiveCandidateOwnerIdentitySet",
        "AdequateLeaderTargetServicedCandidateOwnerIdentitySet",
        "AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet",
        "AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet",
        "AdequateLeaderTargetOffSubjectControlClosedOwnerIdentitySet",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetDurablyRetiredOwnerIdentitySet",
    ): (
        "AdequateLeaderFrozenSubjectSwitchOwnerUniverse",
        "AdequateLeaderTargetServicedCandidateOwnerIdentitySet",
        "AdequateLeaderTargetInternalBodyAvailableRetiredOwnerIdentitySet",
        "AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet",
        "AdequateLeaderTargetOffSubjectControlClosedOwnerIdentitySet",
        "AdequateLeaderFrozenCandidateOwnerUniverse",
        "NodeHasDecision",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSubjectSwitchEpisodeAtBudget",
    ): (
        "AdequateLeaderTargetCarriedOwnerEpisodeAtBudget",
        "AsyncProposalSubject",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOffSubjectRetirementAndReentryGoal",
    ): (
        "NodeHasDecision",
        "AdequateLeaderTargetOccurrenceCorridorExitHandoff",
        "owner",
        "retired",
        "AdequateLeaderFrozenSubjectSwitchOwnerUniverse",
        "SetLessThan",
        "AdequateLeaderTargetSubjectSwitchRemainingBudget",
        "AdequateLeaderTargetProductiveOwnerEpisodeAtBudget",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSubjectSwitchEpisodeAdvanceGoal",
    ): (
        "AdequateLeaderTargetSubjectSwitchDiscoveredOwnerSet",
        "AdequateLeaderFrozenSubjectSwitchOwnerUniverse",
        "AdequateLeaderTargetSubjectSwitchRemainingBudget",
        "AdequateLeaderTargetSubjectSwitchEpisodeAtBudget",
        "owner",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOccurrenceRankServiceProperty",
    ): (
        "AdequateLeaderTargetOccurrenceOwnerSelected",
        "AdequateLeaderTargetUniversalOccurrenceServiceGoal",
        "AdequateLeaderTargetNonDescentEpisodeAtBudget",
        "AdequateLeaderTargetSameOrHigherOccurrenceFrontier",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSubjectSwitchBudgetDescentProperty",
    ): (
        "AdequateLeaderTargetOccurrenceRankServiceProperty",
        "AdequateLeaderTargetOffSubjectControlNoReentryProperty",
        "AdequateLeaderTargetInternalBodyAvailableNoReentryProperty",
        "AdequateLeaderTargetDurableRetirementCarryProperty",
        "AdequateLeaderTargetSubjectSwitchCarryStepProperty",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleIoOwnerRequired",
    ): (
        (
            "AsyncServeLiveReservationOwned(archive, identity) "
            "/\\ ~AsyncServeJobQueued(archive, identity) "
            "/\\ ~CanResumeExactServeCapacity(archive, identity)"
        ),
        (
            "barriers # {} /\\ ~CanResumeExactServeCapacity("
            "archive, ExactDecisionRequestFrozenServeBarrierIdentity("
            "archive, request))"
        ),
    ),
}
SERVE_LIFECYCLE_REQUIRED_OPERATOR_TOKEN_SEQUENCES = {
    (
        "SumeragiV2AsyncNetwork",
        "ReserveExactServeCapacityVia",
    ): (
        "AsyncServeSourceAttemptRecords(node, identity) = {}",
        "~AsyncServeIngressAdmissionOwned(node, identity)",
        "AsyncServeIngressLifecycleOwnerIdentities(node) = {}",
        "AsyncServeOffQueueReservations(node) = {}",
        (
            "asyncNextServeAdmissionOrdinal' = "
            "[asyncNextServeAdmissionOrdinal EXCEPT ![node] = @ + 1]"
        ),
        "AsyncServeIngressPredecessorCounts(node)",
        (
            "AsyncServeAttemptForRequestAtStage( "
            'node, candidate.item, authenticatedSource, ordinal, "Ingress")'
        ),
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AdvanceExactServeCapacityVia",
    ): (
        "AsyncServeSourceAttemptRecords(node, identity) = {}",
        "~AsyncServeIngressAdmissionOwned(node, identity)",
        "AsyncServeIngressLifecycleOwnerIdentities(node) = {}",
        "AsyncServeOffQueueReservations(node) = {}",
        "AsyncServeFamilyTombstoneRecords(node, family) # {}",
        "roundView > AsyncServeFamilyHighWatermark(node, family)",
        (
            "asyncNextServeAdmissionOrdinal' = "
            "[asyncNextServeAdmissionOrdinal EXCEPT ![node] = @ + 1]"
        ),
        "AsyncServeIngressPredecessorCounts(node)",
        "AsyncServeTombstonesWithoutFamily(node, family)",
        (
            "AsyncServeAttemptForRequestAtStage( "
            'node, candidate.item, authenticatedSource, ordinal, "Ingress")'
        ),
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncRetransmitLifecyclePhysicalCutForStep",
    ): ("ELSE AsyncNextIngressPhysicalOrdinal(node)'",),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateProducerContinuationMayPrecedeOwnedRetransmit",
    ): (
        (
            "record.sourcePhysicalOrdinal < "
            "AsyncRetransmitLifecyclePhysicalCut(node)"
        ),
        "record.ordinal < AsyncRetransmitLifecycleOrdinal(node)",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncRetransmitPriorityPrecedesCandidate",
    ): (
        (
            "AsyncRetransmitLifecyclePhysicalCut(node) <= "
            "AsyncCandidateLifecycleSourcePhysicalOrdinal(candidate)"
        ),
        (
            "AsyncCandidateLifecycleSourcePhysicalOrdinal(candidate) < "
            "AsyncRetransmitLifecyclePhysicalCut(node)"
        ),
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestRuntimePrefixSnapshot",
    ): (
        "physicalCut |-> physicalCut",
        (
            "ExactDecisionRequestRuntimeCandidateOriginsAt( "
            "node, schedulerCeiling, physicalCut)"
        ),
        (
            "ExactDecisionRequestRuntimeServeSourcesAt( "
            "node, schedulerCeiling, physicalCut)"
        ),
        (
            "ExactDecisionRequestRuntimeContinuationSourcesAt( "
            "node, schedulerCeiling, physicalCut)"
        ),
        (
            "ExactDecisionRequestRuntimeLeaderWireIdentitiesAt( "
            "node, schedulerCeiling, physicalCut)"
        ),
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestClockPrefixSnapshotBinding",
    ): (
        "snapshot.schedulerCeiling = ownerOrdinal",
        (
            "snapshot.physicalCut = "
            "AsyncRetransmitLifecyclePhysicalCut(node)"
        ),
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestOwnedRuntimeAtRank",
    ): (
        (
            "ExactDecisionRequestClockPrefixSnapshotBinding( "
            '"Owned", snapshot, node, ownerOrdinal)'
        ),
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleConcreteFairAction",
    ): (
        (
            'ownerKind = "NormalRunner" -> '
            "PostGstRunNode(archive)"
        ),
        (
            'ownerKind = "HistoricalServer" -> '
            "PostGstRunHistoricalServer(archive)"
        ),
        (
            'ownerKind = "IoWorker" -> '
            "PostGstServiceIoWorker(archive)"
        ),
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleConcreteActionOriginProperty",
    ): (
        (
            "ExactDecisionRequestLifecycleConcreteFairOwner("
            "archive, request)' = "
            "ExactDecisionRequestLifecycleConcreteFairOwner("
            "archive, request)"
        ),
        (
            "<<ExactDecisionRequestLifecycleSelectedConcreteFairAction("
            "archive, request)>>_AsyncAllVars => "
            "ExactDecisionRequestLifecycleRankCellOutcome("
            "node, qc, archive, request, rank, budget)'"
        ),
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenNetworkItemCarrier",
    ): (
        (
            "\\cup "
            "AdequateLeaderFrozenCertifiedRequestItemCarrier(leaderView)"
        ),
        "\\cup AdequateLeaderFrozenCommitRequestItemCarrier(leaderView)",
        (
            "request \\in "
            "AdequateLeaderFrozenCommitRequestItemCarrier(leaderView)"
        ),
        "qc \\in AdequateLeaderFrozenQcRecordCarrier(leaderView)",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOffSubjectControlRetirementMemory",
    ): (
        (
            "AdequateLeaderTargetOffSubjectControlCandidateOwnerIdentity( "
            "item, target, leaderContext, leader, leaderView, subject, "
            "occurrenceRank) \\notin "
            "AdequateLeaderTargetLiveCandidateOwnerIdentitySet( target, "
            "leaderContext, leader, leaderView, subject)"
        ),
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSubjectSwitchEpisodeAtBudget",
    ): (
        (
            "/\\ AdequateLeaderTargetCarriedOwnerEpisodeAtBudget( "
            "target, leaderContext, leader, leaderView, subject, "
            "occurrenceRank, owner, retired, budget) "
            "/\\ subject # AsyncProposalSubject(leader)"
        ),
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSubjectSwitchDiscoveredOwnerSet",
    ): ("{owner} \\ retired",),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSubjectSwitchEpisodeAdvanceGoal",
    ): (
        "retired \\cup discovered \\subseteq retired2",
        "owner \\in retired2",
        "budget2 < budget",
    ),
}
SERVE_LIFECYCLE_FORBIDDEN_OPERATOR_TOKENS = {
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderCandidateProducerContinuationRetirementMemory",
    ): (
        "AsyncCandidateProducerContinuationActiveForIdentity",
        "AdequateLeaderTargetUniversalOccurrenceServiceGoal",
        "AdequateLeaderTargetOccurrenceRankServiceProperty",
        "AdequateLeaderTargetCountIncreasingReplenishmentAction",
        "AdequateLeaderTargetRankReplenishmentAction",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetProducerContinuationRetiredOwnerIdentitySet",
    ): (
        "AdequateLeaderTargetUniversalOccurrenceServiceGoal",
        "AdequateLeaderTargetOccurrenceRankServiceProperty",
        "AdequateLeaderTargetCountIncreasingReplenishmentAction",
        "AdequateLeaderTargetRankReplenishmentAction",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleRankDescentProperty",
    ): (
        "WF_AsyncAllVars",
        "ExactDecisionRequestLifecycleOwnedFairAction",
        "ExactDecisionRequestLifecycleFiniteProducerEpisodeClosureProperty",
        "ExactDecisionRequestLifecycleOwnedFairActionClosureProperty",
        "producerBudget",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleFrozenPredecessorSet",
    ): (
        "EarlierServe",
        "AsyncServeEarlierLiveReservationIdentities",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleIoOwnerRequired",
    ): (
        "CanEnqueueIoClass",
        "AsyncIoEffectiveQueueDepth",
    ),
    (
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
        "ExactDecisionRequestLifecycleConcreteActionOriginProperty",
    ): (
        "WF_AsyncAllVars",
        "ExactDecisionRequestLifecycleOwnedFairAction",
        "producerBudget",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderFrozenOwnerUniverse",
    ): (
        "AsyncCandidateSet",
        "asyncSentItems",
        "NodeHasDecision",
        "AdequateLeaderTargetCandidateRole",
        "AdequateLeaderTargetWireIdentity",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetComposedRankDescentProperty",
    ): ("AdequateLeaderTargetRankDescentProperty",),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOffSubjectControlRetirementMemory",
    ): (
        "~CandidateScheduled(DeliveryCandidate(item))",
        "AsyncCandidateSet = {}",
        "AsyncNetworkItems = {}",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSubjectSwitchDiscoveredOwnerSet",
    ): (
        "AdequateLeaderTargetLiveOwnerIdentitySet",
        "AsyncCandidateSet",
        "AsyncNetworkItems",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOccurrenceRankServiceProperty",
    ): (
        "AdequateLeaderTargetPhysicalPacketSet = {}",
        "AsyncNetworkItems = {}",
        "AsyncCandidateSet = {}",
    ),
}
SERVE_LIFECYCLE_FORBIDDEN_OPERATOR_TOKENS.update(
    {
        (
            "SumeragiV2AdequateLeaderServiceClosureProofs",
            operator,
        ): (
            "AsyncCandidateSet",
            "AsyncNetworkItems",
            "AsyncEvidenceSet",
            "asyncSentItems",
        )
        for operator in (
            "AdequateLeaderFrozenQcRecordCarrier",
            "AdequateLeaderFrozenVoteRecordCarrier",
            "AdequateLeaderFrozenTimeoutVoteRecordCarrier",
            "AdequateLeaderFrozenTcRecordCarrier",
            "AdequateLeaderFrozenProposalRecordCarrier",
            "AdequateLeaderFrozenBodyRecordCarrier",
            "AdequateLeaderFrozenBodyEnvelopeCarrier",
            "AdequateLeaderFrozenCertifiedRequestItemCarrier",
            "AdequateLeaderFrozenCertifiedRequestHashCarrier",
            "AdequateLeaderFrozenCommitRequestItemCarrier",
            "AdequateLeaderFrozenNetworkItemCarrier",
            "AdequateLeaderFrozenEvidenceCarrier",
            "AdequateLeaderFrozenCandidateItemPayloadCarrier",
            "AdequateLeaderFrozenCandidateEvidencePayloadCarrier",
            "AdequateLeaderFrozenCandidatePayloadCarrier",
        )
    }
)

_WORKER_TEST_INCLUDE_SOURCE_SHA256 = {
    "v2_worker_reply_route_cases.rs": (
        "c742bc186239447a039bc823aa2efd3bd654291811f7508538bb0194aa660217"
    ),
    "v2_worker_backpressure_cases.rs": (
        "fdbcc73d2d6dbe68e0ffa683f9a6413043e58a7b5013df9c8f080ca4185b26e5"
    ),
    "v2_worker_recovered_lifecycle_output_cases.rs": (
        "101004202bb87e98d9372d80563277db29df21889f6df957fc3a131b7f8d3bc4"
    ),
    "v2_worker_nonzero_view_restart.rs": (
        "b4e023c528ce068df688c6470a27a25ba14df45fb7553f053f8877cdda253fa9"
    ),
    "v2_worker_serve_unsealed_cases.rs": (
        "aaeeebba50f95dff140bda57b82be1c2379dc2e7c5330639e9cb0a02fe77b842"
    ),
    "v2_worker_serve_decision_restart_cases.rs": (
        "4756497a5ce66a906a0b1e8195afe6e6978c55f88acb1ea7ca0ae0bbce131de6"
    ),
    "v2_worker_certified_serve_budget_cases.rs": (
        "0bad66535d74184e0576248ed79fd63754bc38f82c765126459bdcf5489a61ff"
    ),
}

_WORKER_TEST_INCLUDE_TEST_COUNT = {
    "v2_worker_reply_route_cases.rs": 22,
    "v2_worker_backpressure_cases.rs": 21,
    "v2_worker_recovered_lifecycle_output_cases.rs": 8,
    "v2_worker_nonzero_view_restart.rs": 1,
    "v2_worker_serve_unsealed_cases.rs": 23,
    "v2_worker_serve_decision_restart_cases.rs": 11,
    "v2_worker_certified_serve_budget_cases.rs": 13,
}

_SERVICED_CANDIDATE_STORE_STRUCT_SHA256 = {
    "ServicedCandidateKey": (
        "7ac3f413238c7b14472d7353286b7f4cd8a066be9a454ab8f398911024f1997e"
    ),
    "PersistedServicedCandidates": (
        "d6f6930bed0ea65978ef0f55c2513c394f5a1d48e427df953b5f1bac63324ca5"
    ),
    "ServicedCandidateStore": (
        "a2f2ca101f4e9a59d10e0a4d538c1fefb0d20e527c9917e7423e4b38f99f634f"
    ),
}
_SERVICED_CANDIDATE_STORE_ITEM_SHA256 = {
    "snapshot_metadata_is_safe": (
        "4a8bd8613fea736eac0363046ce5da31ccbc25b556cc5f7c6847a35d1f927406"
    ),
    "snapshot_metadata_unchanged": (
        "6fb1e92bda15ec6c6aa5d838af8ec3471f59670e19faaef47aeb60ede95e51f4"
    ),
    "open_bound_snapshot": (
        "0df492c60299c0583ac6ac6286e8f21679934b8328022a597a456837e21b8172"
    ),
    "open": (
        "a8f9d5a006c511b44fa9d476cc8067cfa8af8dedee03a80030274b4ccf304de0"
    ),
    "load": (
        "c63e3f952e8fcaa243df9f60a4f401b34b075ac3e3d6faec893e08df4a10cf6b"
    ),
    "persist": (
        "3a42fc43341e7cc1129f39551195eadd97e224df7be96a4bed0dd18abd7453d3"
    ),
    "retire": (
        "59818f9a08a8c7d21a2193e2e12ead888cd180bac9a9f643637191da66bdfe74"
    ),
    "encode_frame": (
        "6aff73c18c2a4ab25777428f049f9fbdcf72253f532ca4e433b7ab213f4444d1"
    ),
    "decode_frame": (
        "563814465dac42c5d017476396f1bed45f346673347916ed9b3b6382f1869489"
    ),
}
_SERVICED_CANDIDATE_STORE_REGRESSION_TEST_SHA256 = {
    "snapshot_roundtrips_and_rejects_a_b_a_resurrection": (
        "d19e863f444d25bb405fcfc183438c9abd6e64fd7ce82453c91059bb2874007d"
    ),
    "snapshot_rejects_corruption_stale_context_and_capacity_exhaustion": (
        "0d502725ec449fb4ed1f5209b4e02c96baf524bdb61986be8f8fa38d32390aee"
    ),
    "decision_reclamation_is_canonical_only_for_an_empty_snapshot": (
        "45f6bfcf60d7bc3c49cde2a6f0d3cd48592f09ee8aee166da7bf484907e5c246"
    ),
    "snapshot_rejects_truncation_version_ordering_duplicates_and_oversize": (
        "1b232cf5e02893c4ae7acc2972fe9dda14335643bd25375c50eb82fe09f7e99a"
    ),
    "snapshot_rejects_nonregular_artifacts": (
        "39491e581cd4fb191c2c2e69732fb4a0f43affa5d8858341f57d5481619a55f1"
    ),
    "snapshot_load_and_retire_never_follow_substituted_symlinks": (
        "0bf9947be38b34e9b2c7f6b6bd843891f94d3395cf9534dcfb94b50a55664cb2"
    ),
    "finalized_snapshot_retirement_leaves_successor_rollover_empty": (
        "f55ac4fcb721c3de07ef1c69375778e8f8bdbe56f94aff5c1b934c84c2fc8aab"
    ),
}
_SERVICED_CANDIDATE_PRODUCTION_ITEM_SHA256 = {
    "append_serviced_candidate_certificate": (
        "6f457b1960bd8b23ba16c25ed6eccaa2c4ea49526563446005e874e23702951e"
    ),
    "append_serviced_candidate_timeout_certificate": (
        "0fc795aaf4f2b81758fbc30bb230714f1f93f79c853e8b79d09791f4a52e48a3"
    ),
    "serviced_candidate_stage": (
        "3434e4b4176f7359597f9df17e06412e2e79cc0dd7b808adb4833afcc683632a"
    ),
    "serviced_candidate_policy": (
        "c97b7a274f8c1084c6ad10c2f92278ba44b959e06019ebac6823fe01424ae8f0"
    ),
    "is_authenticated_ingress_event": (
        "0408babdc67c1e2be458f42b3dd3f9421c8de0e24c4d850f514272316e42d5cf"
    ),
    "serviced_candidate_record_kind": (
        "df98895bd5deed6e3cb2af8c7153211e4e6a23808a4b60769300d77536ff6f6c"
    ),
    "append_serviced_candidate_event": (
        "75f1e7dd2d8d8730f60ee982a0d71935e654a18552a736ad1af35e08c7b2ce09"
    ),
    "serviced_candidate_event_fields": (
        "cd58c4418811c2cd19c84e81757d09d4bc5bc67fee30afb3ce10db432f077a62"
    ),
    "candidate_lifecycle_capacity": (
        "fabc9cf096cec0f7c03ea8a13ebbdb00c9f6556feec6544fa2f4b719662b62f8"
    ),
    "serviced_candidate_capacity_with_geometry": (
        "a05d73fac4af1c5882f1961e138e35cf9b94e7bac5a00d02d7f995e6d39c1d9e"
    ),
    "serviced_candidate_capacity": (
        "b72eb8693dfcb3e037edb5239e04f5f5de5eebcfc1c4d54ce3ef1540b2323c3c"
    ),
    "capacity_geometry_new": (
        "6d78f9cdb1e3fe81e4e4c1b619985044eef53e87e288d133d10a986cda1b0250"
    ),
    "open_with_capacity_geometry": (
        "42a35d83d69940bbf7d92da8075c5c42516d93396c24b4661917a2417cae4e1c"
    ),
    "open_deferred_status_with_capacity_geometry": (
        "609ac6927368b3c869f083fac425ebba12631345cfd2eba3f28e189a70c5cb1a"
    ),
    "open_with_aggregator_and_publication": (
        "b65815cfabc0ed8407c65adba6012c84de9f4b69db0bf021eab09738f54c9329"
    ),
    "open_with_aggregator_and_publication_with_capacity": (
        "b34e23f97090a9f85932e68b0ca7489d09257273933c25d08006330e220534cc"
    ),
    "serviced_candidate": (
        "a5f7c1722916144f07b7ba9da462b6309aeacf460c7a33452125aca1cac64d96"
    ),
    "ensure_serviced_candidate_capacity_before_step": (
        "91ef40b02e85a0afd3182474bd76abbf582d7dd5ec4de6d3da4999cc1385f175"
    ),
    "record_serviced_candidate": (
        "6d9a8a1c61fe5a9742e884f1386363020a029e96011dc5a8175592c922d50511"
    ),
    "reclaim_serviced_candidates": (
        "6d377a2195262b847f4bb58cf16eae17d41be3b3f8f6d8bd4f5d9ff2af946a98"
    ),
    "step_with_defer_policy": (
        "365635da1a537205bb8fe78030c7b6764377a4f640c626d51dd29d55b2025a10"
    ),
    "drain_deferred_with_evidence": (
        "1ad4a360bdadd18ea58c7d1cbbdc549e7505a229d5d6903156aa47c0f2813cde"
    ),
    "drain_deferred_with_evidence_for_ordinals": (
        "d509d85d807ce3b04f0ccb12682f54114f64aa948c2f7b49ca6da9e11ad55341"
    ),
    "retain_failed_serviced_deferred_owner": (
        "ed4c65e5419812874e501ecc6213a3a5b804872a66f3c09e86918632019e7c32"
    ),
}
_SERVICED_CANDIDATE_PRODUCTION_STRUCT_SHA256 = {
    "ServicedCandidateCapacityGeometry": (
        "ad19d0d5b27cff9a9eb103cdf47593b159a4fe1793db06d31e5a4c117aaebbb2"
    ),
    "DeferredServiceEvidence": (
        "f988f1de9f3d9d6b89880bc8ad31585ea848d0d098a2abb8bb07f874fdde7812"
    ),
}
_SERVICED_CANDIDATE_REGRESSION_TEST_SHA256 = {
    "direct_internal_discard_tombstones_a_b_a_and_survives_restart": (
        "268e4e8fd26c5b3b62932dd41b4236280da417d5ee2a1177a7d481c8896b722c"
    ),
    "nonquorum_vote_retransmission_rebuilds_volatile_pool_after_restart": (
        "7eae3ba682981dc555f9a9b47d68a51c82aedaa16c93df529e12f9aa000b1415"
    ),
    "deferred_discard_tombstones_before_owner_release_and_restart": (
        "6c8a8889a5bde297808772feef4b1785e48f1c62190382f8995d8ee69c410874"
    ),
    "serviced_candidate_write_failure_is_fail_closed_and_retains_deferred_owner": (
        "36092769fa443044ada01878360c825f359e5d0eae1626a785d15316d8fd97cd"
    ),
    "serviced_candidate_snapshot_is_bound_to_the_local_validator_owner": (
        "e2d69a7243f71d2818ffabcbdd78e9585c6245b447a882e8f6dc319067a956ee"
    ),
    "aggregate_carrier_and_priority_variants_coalesce_to_one_semantic_candidate": (
        "f0e4b3f696d8f60ab210ceaceb27b37566fda8ade17ce4712ad9fe3f6598f027"
    ),
    "serviced_candidate_capacity_exhaustion_never_evicts_an_old_owner": (
        "ed7172e200b6ebe76d6c6048a08e6610b83e709b66dc8e0dd972998d5e8f2b0f"
    ),
    "busy_deferred_source_identity_coalesces_across_consumer_view_change": (
        "3d0feb5f05f48838a00ff845c8bd16e884bcf7212bc630c32fb83cf5e712fa00"
    ),
    "serviced_candidate_reclaim_failure_fail_stops_then_replay_reclaims": (
        "c021ecccfdd60ea3ff9e4cced99dbb2b489adccfd1fe65874f81953b3989f3ab"
    ),
    "prelock_current_commit_is_readmitted_with_priority_neutral_service_identity": (
        "16a1bc5bca9454576a32f14b7e6cd94889ece97f97f194f1ff28a0d361b6c16d"
    ),
    "proposal_signed_callback_is_restart_scoped_before_control_delivery": (
        "3e5610142981579ef62b6fe4369e7d177f438a7352e563d9231623cd8036fce6"
    ),
    "vote_signed_callback_is_restart_scoped_before_control_delivery": (
        "5c90a0c9a02623697eb7d68bf673d2948fb5d75cd63170ca873b9caab61c4879"
    ),
    "timeout_signed_callback_is_restart_scoped_before_control_delivery": (
        "edcc41a7ae22aa8599821e7d7e14bb69257163f2d411df628af057cf431c7b6b"
    ),
}
_SERVICED_CANDIDATE_V4_STORE_STRUCT_SHA256 = {
    "ServicedCandidateKey": (
        "7ac3f413238c7b14472d7353286b7f4cd8a066be9a454ab8f398911024f1997e"
    ),
    "PersistedServicedCandidate": (
        "b270b68c01fa0b0f10f8415fd36c9f2fa80f8ff627fee866ca0bf3c35b86b207"
    ),
    "ProducerContinuationAddress": (
        "7fd1d0c883e804f99c21932bdee838f7916ba6b40a199bba8dfd9d8a9754b4fd"
    ),
    "ProducerContinuationIdentity": (
        "97c0e3f9975d0cca536ec1a3f10e2cc69ede8bbf46af872fb0e77e2197b847f6"
    ),
    "ProducerContinuationHandoffToken": (
        "b032bb2a7c2cd3780d412ce491850ecbe93e509afbf6f71fe3b5d3b37ee8f26d"
    ),
    "ProducerContinuationTerminalToken": (
        "050296874d4404b103aa3dde9b19c85e9d1bd20420b060830cfbb732e19a58e2"
    ),
    "ProducerContinuationRecord": (
        "39f1a443523f864b560701817bb47a49562279bb7c418193513e00114a6a3025"
    ),
    "PersistedProducerContinuation": (
        "31b05bf0c63cd416ef90421cc08b6b0787749a8ee36cd60682fe11440c313947"
    ),
    "PersistedServicedCandidatesV4": (
        "a57ae940f4b09d1a20d4ad1e70b3e42bdcdfefd57f5f6d2bb76de521796bdf82"
    ),
    "DecodedServicedCandidates": (
        "a48fe3bc044d00aed0f51dfe9eec473faf28cf42895c563449836fcfecb594a5"
    ),
    "RestoredServicedCandidates": (
        "513bcbab69e770685d1c32b901aba58927aaeb681c4bd114efb196ed333aad5e"
    ),
    "ServicedCandidateStore": (
        "abc4a9edd831d00f0296143653aeb340c018aad9e4e8d9a4372e35ff759bf796"
    ),
}
_SERVICED_CANDIDATE_V4_STORE_ITEM_SHA256 = {
    "serviced_candidate_stage_for_kind_code": (
        "a070c60ab3c6db44c662f78c421d68f10091bd46996233e2d8b34b866fb1bd6c"
    ),
    "producer_continuation_source_class_for_kind_code": (
        "66595513df39f7abcac6fd0bcb65d80b610a07578b828b8a359314c10a82a7de"
    ),
    "identity_new": (
        "9343fe68e2fc3c9b379bd172cfd37b8360c6cf12ec941fdb3edada4c8c3d480b"
    ),
    "identity_address": (
        "ca643f22795e66325b0f2bf8a2c353cbc0ee88db23e68ca787a62559fac62e57"
    ),
    "identity_has_exact_stage": (
        "6979be6b70abbc3b31e225deb4d012d7cf93cf2629d892808ec7ce6f92b0063b"
    ),
    "record_new": (
        "0af698eefd7b9740f7e89c73bbd74ad562cc504618057a302ccad5548aae6d4c"
    ),
    "record_handoff_token": (
        "cf8495dc2c9c36c24925d318bf5f531d87576d16a8ac8a35c42ae0c314f995e4"
    ),
    "record_terminal_token": (
        "240867fd2bee3e1e0336daee1b91922146bf787a7c076dec15d038b113887b08"
    ),
    "producer_continuations_are_valid": (
        "80eef029cbd13f7d277b04ac833a74b7781fc77cd5b950d03c4a2653e63400b5"
    ),
    "leader_wire_terminal_matches_runtime": (
        "9efc97b58851cedd570b0659917aa1906aa31f653cc385e43de3cb600e8c044d"
    ),
    "leader_wire_control_phase_matches_candidate": (
        "c200345c7008a00d68732f2a235b3950f04c0e63fbe78b51478a9e7fdc88f220"
    ),
    "leader_wire_stable_terminal_matches_runtime": (
        "3e025a26fdd14c00e5664a2d34355169bc7b650de2534939586f3a7b50ebb9d4"
    ),
    "leader_wire_load_and_reconcile": (
        "dfdd5e26961ac56d3cdb189d65ddc634a31f0163d125511325cc8a850749e710"
    ),
    "store_open": (
        "a87131cdea8fcf204ab3a04992f93e05fc32400f475abcda450f2636d08b97de"
    ),
    "store_open_with_capacities": (
        "8abe37b42d072f5a50b54c37025e6042bdafd6cecaec48e7f3fc217a41537b95"
    ),
    "store_open_with_storage_and_capacities": (
        "efd1503685bf7b1871785e2dcc66f7b24286035c043015f0faef8ac7679d6199"
    ),
    "store_load": (
        "7d11d50560c963ae104e22e07e706dc6625f39263047391023a715127fb57de6"
    ),
    "store_reserve_producer_continuation": (
        "452de9ed84f0d1f83c17df83d58f87f7530b60e98d022f11694e9896a5181b4e"
    ),
    "store_persist_with_producer_continuations": (
        "5f238302d23375639a031d0276d9abd06a52c16b004540d4749cbf0767dca3d5"
    ),
    "encode_payload_frame": (
        "7cb4861444e43a3904493bfcb15ff8eb92f5ee6fdbe763eec691701a11efa71f"
    ),
    "encode_frame_v4": (
        "14296fa8c429ee49f9ccba935bdc06f984941eefda1b0e995d091741e6130ad2"
    ),
    "decode_frame": (
        "97f0a238c245499678a700ffb079238580d5998b8fb73858245a2bffa51ddf92"
    ),
}
_SERVICED_CANDIDATE_V4_ADAPTER_STRUCT_SHA256 = {
    "SelectedProducerLifecycle": (
        "240a5920ce99612d24578a815f697889d1c06d8beb9a1e300ab4c5338f998ec6"
    ),
    "ProducerReservationToken": (
        "3f01d4fabf54fd84371442ca9c6b6b499902ef2eda684eb13234fc4d1e6e46d3"
    ),
    "PendingProducerHandoff": (
        "3c9f5ad566a113a3c007f67ab3a86a8ac33495cbc6e158ae187f3a617a0b0f6b"
    ),
}
_SERVICED_CANDIDATE_V4_ADAPTER_ITEM_SHA256 = {
    "prepare_leader_wire_launch": "e312bc9b0e4a97d11a8d195875c476e9bb37fa988551ecdf2cda4a536495df60",
    "producer_parent_replay_source_for_stage": (
        "de61146d932b8f0f65a799f8102e83013fce7f875e82d591b22528c75caab41f"
    ),
    "producer_parent_is_locally_reconstructible": (
        "6d46edcfbf32039e6c1f17bd7a1fa0203a8faa6069f2b69fcf5f2684c0bcd458"
    ),
    "producer_parent_has_exact_local_replay_binding": (
        "a70ccef5d263dff9464f9292e354039e840b8258c35e9c053245fdd1629d5eb7"
    ),
    "serviced_candidate_stage": (
        "af1e74ad7671148c9dfe5fdade32ff32608e17cdf60c7e4c268095d8ff0f1060"
    ),
    "serviced_candidate_policy": (
        "c97b7a274f8c1084c6ad10c2f92278ba44b959e06019ebac6823fe01424ae8f0"
    ),
    "is_authenticated_ingress_event": (
        "0408babdc67c1e2be458f42b3dd3f9421c8de0e24c4d850f514272316e42d5cf"
    ),
    "serviced_candidate_record_kind": (
        "df98895bd5deed6e3cb2af8c7153211e4e6a23808a4b60769300d77536ff6f6c"
    ),
    "candidate_lifecycle_capacity": (
        "fabc9cf096cec0f7c03ea8a13ebbdb00c9f6556feec6544fa2f4b719662b62f8"
    ),
    "open_with_aggregator_and_publication_with_capacity": (
        "61dab9fb549388aaff80edf3ae6b568c9d806b1d59469bff831ab85ae67b93b6"
    ),
    "dormant_producer_lifecycle": (
        "3103fd04b61950b06212a9e50f4ae43b4b8ad35949566680afb8f721c133155e"
    ),
    "dormant_local_fifo_reservations": (
        "e2dac966c3f6ca19c566c952433e598c749e1e55793a6db460df4f14a26b0aa6"
    ),
    "bind_selected_producer_lifecycle": (
        "e7824df390bb5cbce2a5d9721e9aa0b70fb410f0446ac606f1ac06a44522b90d"
    ),
    "clear_selected_producer_lifecycle": (
        "31da2a7879318a3bdd6de93f996576fc9a192743dbfbcb3f4e39dca4dfbdabc4"
    ),
    "producer_lifecycle_slot": (
        "087e97f8f57945f92a9709d230014f7692ab110f8e8b7657984c403019d9cbe0"
    ),
    "reserve_selected_producer_continuation": (
        "6a2aae48b85aaf9ca7ce2a880d19b8fb108f06aec50774044de3f2d874f73c67"
    ),
    "persist_producer_lifecycles": (
        "923100ada06b4b713c15ed273d9306f4d592e9a25194d3be5d355d6077ea94b9"
    ),
    "rollback_producer_reservation": (
        "f6a2ae48d02fa974e796886695810d7518cd0530fc3578f9b36f3dee3896fb08"
    ),
    "release_unrecorded_producer": (
        "55b89e3ae49d75fe9a36de6575f378788331c552067ceb9f41e1fbdee43599b1"
    ),
    "terminalize_producer_continuation": (
        "7f885aee1e1ef617dbdd47906175f080b877a355a7f3f7c5d5f706480f04f221"
    ),
    "release_goal_reached_producer": (
        "c36862b2029a3d819de48bcc9b3cedf43bf751a3402d0e9c7d90478108dd353f"
    ),
    "serviced_candidate": (
        "a5f7c1722916144f07b7ba9da462b6309aeacf460c7a33452125aca1cac64d96"
    ),
    "ensure_serviced_candidate_capacity_before_step": (
        "91ef40b02e85a0afd3182474bd76abbf582d7dd5ec4de6d3da4999cc1385f175"
    ),
    "record_serviced_candidate": (
        "da6671debcefb22ad5eb658891a99e8862c01114bd0d19a0730baf33da2bb59b"
    ),
    "producer_handoff_evidence": (
        "2b553732a9df3b84f8d4b8495b063129278080a7a775624d13c6ac9b87278a39"
    ),
    "acknowledge_producer_handoff": (
        "038ccb3a1d6c463f3b10f01491139d5fcbe4827d5a8efad09a7c54de6134bc79"
    ),
    "reclaim_serviced_candidates": (
        "e198c896e7ea6c3cc6862b48d6dc9432f578dc4e95231f621c3079d79855c77d"
    ),
    "step_with_defer_policy": (
        "f61873fdbedf7727cc0c0575a67721e089fda4c9aac2d7e31eb45bbb096befc6"
    ),
    "drain_deferred_with_handoff_for_ordinals": (
        "b0f80b64612e436979c51bef1a607147f2fcc3c4c5b127a3738a4ca3410cc40f"
    ),
    "drive_effects": (
        "228291c466d581465006e2b731591caea95750c08dd0c5cea836b8d3c7de6e95"
    ),
}
_SERVICED_CANDIDATE_V4_RUNTIME_STRUCT_SHA256 = {
    "RuntimeDormantLocalFifoReservation": (
        "6e85865f52ff4ce41ec7d2d3a363927035b2e2f3a87ad34424943030bcc34ad6"
    ),
    "BoundedIngress": (
        "826d82f8fcae56b9a0ca7b86225b64ada55b98ffd052022eabf0df5f5cba3bb0"
    ),
}
_SERVICED_CANDIDATE_V4_RUNTIME_ITEM_SHA256 = {
    "dormant_completion": (
        "be7724361d11298229e3d807cc4ea6d4cc42eaf52e2bf236e5f405324aecf95c"
    ),
    "dormant_is_local_fifo_stage": (
        "b362acc1a6152e4b6c9bbf0b8bcea133791c89c8ae4033c5048f10e09e175388"
    ),
    "install_dormant_local_fifo_reservations": (
        "98bd201936448b4a3779801de7fbb70f470f557f8ade4ce372ed963ac34bd42f"
    ),
    "dormant_local_fifo_replacement": (
        "650e241b2095ea2701108807d750c057e9b7c6363d0094b80817f381b6bb2ff9"
    ),
    "dormant_local_fifo_replacement_inner": (
        "018584c25ed2c434bdeed8f66773cba47dee3fe637167a57f8b44e62032f92c4"
    ),
    "occupied_with_dormant_reservations": (
        "d70ffa45df19675f7fd158ab1fbfb36d198615463dbaac214900bacc023b163e"
    ),
    "active_dormant_local_fifo_reservation_count": (
        "e42231cb6d1c5f5c63e6181003cb3032b1c2fd069ccad4b3cbd3a497c9f000d9"
    ),
    "oldest_active_lifecycle_ordinal": (
        "a9dcc40ab11d2af33c91c5449a24bd524289d8a00e89ea2cdfafe99b27ed2a86"
    ),
    "with_driver_and_lifecycle_ordinals": (
        "1696486255d16af1779a46c10dee8e213104aa4c900d089f3ab0f962b11607eb"
    ),
    "freeze_due_clock_owners": (
        "d1c028eb58483adffe8eb1415b431d3f031714167535af7382d3ee39b5cf4027"
    ),
    "minimum_active_lifecycle_ordinal": (
        "bb4ac2c885dce0086aed3df676af4b5d4c45ea00c9d93e06521242058ef85c9d"
    ),
    "minimum_active_lifecycle_ordinal_excluding": (
        "bb53e945b76e2bdbd83e60c3e259d6a946eaf83be9685ca099c01fc8da5dbdf8"
    ),
    "complete_leader_wire_runtime_owner": (
        "e17e62beccb6e2e219f3aac01c126456531ffb0448930b83026d9cf02da6695c"
    ),
    "observe_effects": (
        "a046c2022e0ae5bec78701605451d8ef99c2c474345ebddd0b49d8c18f274e49"
    ),
    "step": "aaa41e0366ae660537780528c97e763e8f292e9a80234c21d4cc37a390eea414",
    "finish_dispatched_step": (
        "79b15a8142c81629b078a64822c4bba3a7cc930d1da75eb816c7f64609021285"
    ),
    "try_step_pacemaker_escape": (
        "aa0a41501d13d502e119566bb4fc55e202f820ece97cf66e486ac851b458c64d"
    ),
    "dispatch_one_pacemaker_progress": (
        "fc7d5f12b41f703242826e8926dd91f4267b78d775b1d8c2453d85006a3ac965"
    ),
    "dispatch_one_fence_dependency": (
        "539239fa96fca8ea08dc56ac89041b7be0bb6f5f3d33f65259ad8e7833173b69"
    ),
    "dispatch_one_adapter_deferred": (
        "a4c901cdd676731f6cfd3c4dcb52718df366f65bc7ca5e8d1a54a841ec30cdab"
    ),
}
_SERVICED_CANDIDATE_V4_LIFECYCLE_ITEM_SHA256 = {
    "launch": "d3b9a9f68ce361cb3609c5b181869ae483ca76ac3eea33c0a72ad6f57ba76b48",
    "into_serialized_runtime": "87068a8d2dee32dfba73139870cb7c2cd3128d452ad38f5c478be9c4503bd192",
}
_SAFETY_WAL_DIRECTORY_CAPABILITY_REGRESSION_TEST_SHA256 = {
    "open_rejects_a_preexisting_symlink_for_the_owned_wal_directory": (
        "b7c12ab1ab457087eb7f0dbf0b8fa2611953ed92e2b6a97428ee4826d641d8a1"
    ),
    "parent_substitution_poisoning_prevents_wal_append_acknowledgement": (
        "7f1cc2e7b64e61b0c979167871c0cc3427bd24ae4c35c94f72d9164866e53aea"
    ),
    "adjacent_authorities_reject_parent_substitution_without_path_fallback": (
        "71f4a45dd544def294eb0a663a2fee199130996a3db769fd5db1468e10a69687"
    ),
    "adjacent_authority_bounds_publish_read_and_retirement": (
        "a9cfdc40522e1b1bf9243607d0fece4d38c407ac1a84117b13c05517dfcabcdb"
    ),
}
_SERVICED_CANDIDATE_V4_STORE_REGRESSION_TEST_SHA256 = {
    "snapshot_roundtrips_and_rejects_a_b_a_resurrection": (
        "d19e863f444d25bb405fcfc183438c9abd6e64fd7ce82453c91059bb2874007d"
    ),
    "serviced_candidate_recovery_rejects_substituted_wal_directory": (
        "bef464622aa061ccf1d5e580647f03a3222d3897069acf4a581d7cdcc9bf1d01"
    ),
    "v4_roundtrips_terminal_producer_continuations": (
        "c67045239305a98a9110b7e71cd665dc6fc6e86b775a8e8b9733897a8a5c6c51"
    ),
    "leader_wire_gate_reconciles_producer_first_terminal_crash": (
        "f225f861684b5a468fd1e45a9424e614b91b8bfebfd75e10e429c91dbd32b58d"
    ),
    "leader_wire_gate_rejects_producer_terminal_from_foreign_view_or_phase": (
        "064f2e9ca4a8fc96c2f26ae5735d5a057785acf57c5fd9dc9c55230c0c1055e1"
    ),
    "leader_wire_gate_rejects_substituted_wal_directory": (
        "908d8f626ae3cb7eca6f0e8946afa2459deec32802007e0903c4c379dc7828b2"
    ),
    "snapshot_rejects_corruption_stale_context_and_capacity_exhaustion": (
        "9092b4ebab5ec88f8ad99a573352aedb05fe29385f5ff01215bf088c35511059"
    ),
    "decision_reclamation_is_canonical_only_for_an_empty_snapshot": (
        "ef03bf7b54f58eb0ee9e358a780c023aedce3abc15e0e453041c6b1a905c9d9a"
    ),
    "snapshot_rejects_truncation_version_ordering_duplicates_and_oversize": (
        "42c3d3e1698a4bb437bb37e61e363b7ff77930814d2cf218bea09c9bdac684fd"
    ),
    "v4_rejects_noncanonical_or_over_capacity_producer_tables": (
        "78776cdef7779455e547040a79b4a32c2b188f416b5875cbd88d93b896a14679"
    ),
    "producer_identity_stage_projection_rejects_foreign_root_and_successor_stages": (
        "ec16de15110425ee5c28d0dbaf66cd52445dc5f2cd97439877f079aede5ad2a8"
    ),
    "bounded_slot_reuse_requires_terminal_strict_view_and_ordinal_advance": (
        "4017b4a9c58712e9a2b2caa0a711743dbb7396d9b8cf4377efb3cdcfff1dea65"
    ),
    "one_logical_candidate_cannot_resurrect_at_another_bounded_address": (
        "46e731b36475000ff68a866a46d0bfcd0bf58c3e4cf33ec44dbd214ec04a8b5a"
    ),
    "snapshot_rejects_nonregular_artifacts": (
        "39491e581cd4fb191c2c2e69732fb4a0f43affa5d8858341f57d5481619a55f1"
    ),
    "snapshot_load_and_retire_never_follow_substituted_symlinks": (
        "0bf9947be38b34e9b2c7f6b6bd843891f94d3395cf9534dcfb94b50a55664cb2"
    ),
    "finalized_snapshot_retirement_leaves_successor_rollover_empty": (
        "9dfc3ea78bfb71d17057a004f0b83684da96a9196108c707b563167b47d1cf0c"
    ),
}
_SERVICED_CANDIDATE_V4_ADAPTER_REGRESSION_TEST_SHA256 = {
    "direct_internal_discard_tombstones_a_b_a_and_survives_restart": (
        "268e4e8fd26c5b3b62932dd41b4236280da417d5ee2a1177a7d481c8896b722c"
    ),
    "nonquorum_vote_retransmission_rebuilds_volatile_pool_after_restart": (
        "7eae3ba682981dc555f9a9b47d68a51c82aedaa16c93df529e12f9aa000b1415"
    ),
    "deferred_discard_tombstones_before_owner_release_and_restart": (
        "6c8a8889a5bde297808772feef4b1785e48f1c62190382f8995d8ee69c410874"
    ),
    "serviced_candidate_write_failure_is_fail_closed_and_retains_deferred_owner": (
        "36092769fa443044ada01878360c825f359e5d0eae1626a785d15316d8fd97cd"
    ),
    "restored_producer_reuses_runtime_key_and_ordinal_and_does_not_resurrect": (
        "b13646ed6219f0c11276f6ab64fd2270353015c8183da26c61524570f1b279de"
    ),
    "live_producer_owner_cannot_replace_immutable_identity": (
        "fe58056b871ed225ceabf68f95ff99b6ea7882da5ba9e384c45d0864a74eb697"
    ),
    "restored_producer_rejects_a_mismatched_replay_identity_without_mutation": (
        "5d60f0048536308e5941ed4d3b2fb29582aaec680779fb08018679c53f3b52c7"
    ),
    "conditional_transport_service_reserves_and_coalesces_a_producer_lifecycle": (
        "568344c3d6fdf01c1b581b660d29745582535719ce7e8ce769e687fa791afa56"
    ),
    "retired_empty_handoff_terminalizes_once_and_exact_replay_coalesces": (
        "e274d206d3ddcb9b20ce007ad054edb6ae186ab3e820805a8ff79b2d82896c0a"
    ),
    "every_producer_stage_has_an_explicit_replay_parent_contract": (
        "b8e89dc784c68b18af0655cf038f10c7b5a00ba0ff82a7b4b93bb86f56968469"
    ),
    "speculative_producer_rollback_restores_free_and_terminal_slots": (
        "e3f51f4cd516672cd6f6df3be78aac042301bb729969bb372c7f9fa4879d75f2"
    ),
    "process_only_producer_replacement_rollback_stays_volatile_across_restart": (
        "9fbe52a9ad219c3e55150f5a28eb56476a7281df064b9597634be378fb55037c"
    ),
    "process_only_producer_replacement_release_stays_volatile_across_restart": (
        "871f97d2c3f706e03e24a886af3c6a2c71ab1b012b07fe100c5b8305f3672f6d"
    ),
    "process_only_producer_replacement_handoff_does_not_resurrect_predecessor": (
        "b7254054777aec82f239fb2b9368e03d9901bb4d7290d4e6037e2c8cfaf6c883"
    ),
    "retiring_busy_local_parent_releases_unacknowledged_producer_owner": (
        "764b251be94a4d7566bb5e68b10d9b1cd6fb92daa15838415104ca36d8ee2ac5"
    ),
    "terminal_producer_tombstone_survives_restart_blocks_aba_and_advances_shared_source": (
        "3c64469fa90eddc9df2a6fa0f63abc93e403761ae64797a7ef6d96cee4e4f122"
    ),
    "serviced_candidate_reclaim_failure_fail_stops_then_replay_reclaims": (
        "de3ee4ae3ebba0b2b55fc1183b95bb9fd268c827774b6967d94b27f456054460"
    ),
    "serviced_candidate_snapshot_is_bound_to_the_local_validator_owner": (
        "e2d69a7243f71d2818ffabcbdd78e9585c6245b447a882e8f6dc319067a956ee"
    ),
    "aggregate_carrier_and_priority_variants_coalesce_to_one_semantic_candidate": (
        "5101761ebb164505896814130b9260cce1644306e914d27f2a72fe5e5f701dd6"
    ),
    "serviced_candidate_capacity_exhaustion_never_evicts_an_old_owner": (
        "ed7172e200b6ebe76d6c6048a08e6610b83e709b66dc8e0dd972998d5e8f2b0f"
    ),
    "post_wal_oversized_continuation_fails_closed_and_replays_exact_record": (
        "16780721067a755dc5d00dd752f5fb94f0038dff96878577ba8f2c94bcbc1a3b"
    ),
}
_SERVICED_CANDIDATE_V4_RUNTIME_REGRESSION_TEST_SHA256 = {
    "same_view_generation_upgrade_restarts_timeout_with_a_fresh_owner": (
        "9c2930e2bec6e33904f20f6135300bcce4164b36a2cd03d9c8e516729ab84666"
    ),
    "dormant_fresh_owner_cache_is_derived_bounded_and_purged_by_round_tag": (
        "fe0bed714176dd7a285e7dcdd448f9468c60b13aee7dcac8a8c67affc2a3c333"
    ),
    "restart_dormant_local_fifo_reservation_survives_full_class_churn": (
        "22e6b7d6b7620598c67f3e942898d695e717bca1625f3af3572dcce450d43093"
    ),
    "dormant_local_fifo_metadata_rejects_wrong_stage_ordinal_and_capacity": (
        "e882762fdaf772c5a57d5277f4b29f0d6ae71ffb9d4374edd3c7dd1fdb42117d"
    ),
    "restored_exact_stage_coalesces_at_full_capacity_without_aliasing_successors": (
        "00813fcc26022a771dcbd4637380247c9a97a08f94f43ad97aadef923f1097cf"
    ),
    "restored_producer_preflight_cannot_change_completion_service_class": (
        "bd1273f883c17b13d555ed690c5cfaec2b62ab159c20e2be49def98a3058e7e2"
    ),
    "busy_deferred_older_aggregate_rebases_owner_and_rejects_identity_mutation": (
        "738c238a671607d0a0033f16839bd648eaeb3eefebae3348016414858d2869bd"
    ),
}
_SERVICED_CANDIDATE_V4_WORKER_REGRESSION_TEST_SHA256 = {}
_CORE_RUNTIME_TRANSPORT_TOKEN_SEQUENCES = {
    "outer_ingress_cursor": """
self.next_turn = match turn {
    OuterIngressTurn::Completion => OuterIngressTurn::Runtime,
    OuterIngressTurn::Runtime => OuterIngressTurn::Ingress,
    OuterIngressTurn::Ingress => {
        self.cycles_remaining -= 1;
        OuterIngressTurn::Completion
    }
};
Some(turn)
""",
    "route_shadow": """
let routes_before = queued.inbound.reply_routes.clone();
let routes_candidate = inbound.reply_routes.clone();
let prior_evidence = queued
    .inbound
    .ingress_ownership
    .as_ref()
    .expect("every queued semantic owner retains ingress evidence");
let action = match fair_v2_ingress_route_action(
    &prior_evidence.attempts,
    routes_candidate.as_ref(),
) {
    Ok(action) => action,
    Err(_) => {
        return Err(FairV2IngressPushError::rejected(
            inbound,
            FairV2IngressRejectReason::RouteOwnershipInvalid,
        ));
    }
};
let routes_after = match (&routes_before, &routes_candidate) {
    (Some(retained), Some(candidate)) => {
        let mut merged = retained.clone();
        let Ok(receipt) = merged.merge_with_receipt(candidate) else {
            return Err(FairV2IngressPushError::rejected(
                inbound,
                FairV2IngressRejectReason::RouteOwnershipInvalid,
            ));
        };
        let Some(receipt_output) = receipt.into_output(retained, candidate) else {
            return Err(FairV2IngressPushError::rejected(
                inbound,
                FairV2IngressRejectReason::RouteOwnershipInvalid,
            ));
        };
        Some(receipt_output)
    }
    (None, Some(candidate)) => Some(candidate.clone()),
    (Some(retained), None) => Some(retained.clone()),
    (None, None) => None,
};
let Some(route_capacity) = fair_v2_ingress_route_capacity(
    routes_before.as_ref(),
    routes_candidate.as_ref(),
    routes_after.as_ref(),
) else {
    return Err(FairV2IngressPushError::rejected(
        inbound,
        FairV2IngressRejectReason::RouteOwnershipInvalid,
    ));
};
let attempts_before = prior_evidence.attempts.clone();
let candidate_attempts =
    fair_v2_ingress_attempts_for_routes(&attempts_before, routes_candidate.as_ref());
let Some(attempts_after) = fair_v2_ingress_merge_attempt_cursors(
    &attempts_before,
    &candidate_attempts,
    routes_after.as_ref(),
) else {
    return Err(FairV2IngressPushError::rejected(
        inbound,
        FairV2IngressRejectReason::AttemptCursorInvalid,
    ));
};
let attempts_before_hash = fair_v2_ingress_attempt_cursor_hash(&attempts_before);
let attempts_after_hash = fair_v2_ingress_attempt_cursor_hash(&attempts_after);
""",
    "route_atomic_commit": """
if !occurrence.validate_exact() {
    return Err(FairV2IngressPushError::rejected(
        inbound,
        FairV2IngressRejectReason::OwnershipEvidenceInvalid,
    ));
}
let Some(evidence) = prior_evidence.merged(occurrence) else {
    return Err(FairV2IngressPushError::rejected(
        inbound,
        FairV2IngressRejectReason::OwnershipEvidenceInvalid,
    ));
};
let ownership_snapshot = Arc::new(evidence.clone());
let lane = state
    .lanes
    .get_mut(&owner_source)
    .expect("globally indexed fair-ingress owner lane remains present");
let queued = lane
    .entries
    .iter_mut()
    .find(|entry| entry.wire_key.as_ref() == Some(key))
    .expect("global pending wire key has one queued owner");
let queued_inbound = Arc::make_mut(&mut queued.inbound);
queued_inbound.reply_routes = routes_after;
queued_inbound.ingress_ownership = Some(evidence);
queued.ownership_snapshot = ownership_snapshot;
return Ok(FairV2IngressPushDisposition::Coalesced);
""",
    "queue_gate": """
let has_live_control_predecessor = lane
    .entries
    .iter()
    .take(index)
    .any(|prior| fair_v2_ingress_same_control_slot(&prior.inbound, &entry.inbound));
""",
    "queue_gate_verdict": """
if has_live_control_predecessor || (!ingress_barrier_allows && !dependency_bypass) {
    FairV2IngressQueueGateVerdict::Blocked
} else if dependency_bypass {
    FairV2IngressQueueGateVerdict::Dependency
} else {
    FairV2IngressQueueGateVerdict::Strict
}
""",
    "scheduler_blockers": """
let ordinary_candidate =
    if timers_enabled && fifo_ready && !self.driver.deferred_work_is_serviceable() {
        self.ingress
            .ordinary_candidate_owner_and_fence_state(|queued| {
                (
                    self.driver
                        .command_is_blocked_by_deferred_fence(queued.tag, &queued.command)
                        || self.fence_retry_blocked_fifo_owners.iter().any(|owner| {
                            owner.matches_queued(queue_source_identity, queued)
                        }),
                    self.driver
                        .command_matches_deferred_authenticated_owner(&queued.command),
                )
            })?
    } else {
        None
    };
let deferred_owner_blocks_fifo =
    ordinary_candidate
        .as_ref()
        .is_some_and(|(candidate, blocked, deferred_alias)| {
            self.deferred_lifecycle_ownership.values().any(|target| {
                let post_cut = candidate.is_post_physical_cut(target.physical_cut);
                (*blocked && !post_cut) || (*deferred_alias && post_cut)
            })
        });
let older_signer_blocks_fifo =
    ordinary_candidate
        .as_ref()
        .is_some_and(|(candidate, _, _)| {
            self.driver.signature_fence_is_active()
                && self.external_lifecycle_owners.iter().any(|owner| {
                    owner.lifecycle_ordinal() < candidate.lifecycle_ordinal()
                        || owner
                            .causal_origin()
                            .root_ingress_physical_ownership
                            .is_some_and(|root| {
                                candidate.is_post_physical_cut(root.physical_cut)
                            })
                })
        });
if ordinary_candidate.is_some() && (deferred_owner_blocks_fifo || older_signer_blocks_fifo)
{
    fifo_ready = false;
    completion_ready = false;
    progress_ready = false;
    normal_ready = false;
}
""",
}
_TIMEOUT_VOTE_SEMANTIC_CAPACITY_ITEM_SHA256 = {
    "semantic_ingress_capacity": (
        "30e1e0f718530767610746544385b9ff5a86c4d10ab2988deb6841bc67ac3d0e"
    ),
    "admit_authenticated_payload": (
        "390d2b42206ed4055e4b3907561fa8aa94f49c54018950a6c47074e9661efe2f"
    ),
    "prune_ingress_records": (
        "7aabe892539e8914e9fe3486e439efe6e04e8b625183897f8c9d5d128bc0e73b"
    ),
}
_LOCKED_REPROPOSAL_PREPARE_PROGRESS_ITEM_SHA256 = {
    "deferred_progress_capacity": (
        "91e89a872e4d612b933f7852d4829ee9003c5d0e9390e0627915cd3057f40ed6"
    ),
    "deferred_progress_owner": (
        "52034d48d09afae8a6dc842c0986b41ad42794cb0d8f9bea6623daa5c55993c6"
    ),
    "DeferredProgressOwner::class": (
        "20671a4c6258e8ee23db86701ef84c8f0fd10c196d4c8c1c8e971c29a5e72243"
    ),
    "append_deferred_projection_admission": (
        "40f8ec33c6ae190899cb965946100cf9f94a3a9b5f17f616f3d4bfadfac16bd7"
    ),
    "deferred_service_projection_hash": (
        "9315322199c53abe3b1ac6b9b81a9a1c12025aac1b283fc75375cf7c0c28ccf2"
    ),
    "DeferredServiceEvidence::validate_exact": (
        "09f86a10667e6cafd2ab628798b199e72fde0df4aa73f7650ff549b6404d4e93"
    ),
    "authenticated_ingress_is_progress": (
        "3a958aed971bd3666f7d9d0187ae6f57c67613736f949426a19f4b851c48915f"
    ),
    "is_exact_locked_reproposal_prepare_vote": (
        "8070823c16ab326640700867865cc707948db5cfa9e45ff0dcbf8fa18dbe5fe8"
    ),
    "step_authenticated_ingress_with_ownership": (
        "fd2a239e9a2bb08e81a7ef8d07ec10eee80ef75f6cb8a06415778808ca6b6b55"
    ),
    "record_ingress_delivery": (
        "118156a73c589bcf758e527ca285330007b76f18d1a16482ad5b9270074ac86d"
    ),
    "enqueue_deferred": (
        "f22c283acd092ce9f4646caa7081f51c8295a669c951609af5dde2cc91b6c979"
    ),
    "wire_ingress_may_use_progress": (
        "95fa01a0cb82428feb19a8f4a632b8d87b86b7b16c82077c1350c06a10c2da64"
    ),
}
_LOCKED_REPROPOSAL_PREPARE_PROGRESS_REGRESSION_TEST_SHA256 = {
    "current_locked_reproposal_prepare_uses_progress_only_after_local_binding": (
        "3d615bc5bdd11430409291cf03d50a873916b592620bbdb88159fcab70b174bf"
    ),
    "deferred_progress_capacity_matches_partition_geometry": (
        "e6a2c477fb64d4bd6fea8bd377ae8bde4f1a2c07def2b53f27a2d1e82dc62bdc"
    ),
    "deferred_progress_partition_owns_every_vote_and_certificate_class": (
        "2f3a00157a704f7cc78705daf13f56abe52a865c58f215e6ffba558388957322"
    ),
}
_TIMEOUT_VOTE_SEMANTIC_CAPACITY_REGRESSION_TEST_SHA256 = {
    "capacity_bypass_records_follow_current_lock_and_timeout_view": (
        "428b3ade733368fb5f3b0538791055d4c0df9402476902a360bfceab633f5dc5"
    ),
    "certified_timeout_bypasses_hung_signer_and_opens_adjacent_vote": (
        "b4daaad31d1dff26abf5581a80142973caa2f2ce108475debfda12f62ee3947e"
    ),
    "full_normal_deferred_lane_cannot_drop_absolute_timeout": (
        "adf5d6b807867607244f20da26031ca985061e59363830a91f91735c896c70b1"
    ),
}
_TIMEOUT_VOTE_VIEW_WINDOW_ITEM_SHA256 = {
    "timeout_vote_view_is_admissible": (
        "8db7d1f2d356168f58c0a06133c5673b09f90095fc2296240fb96f4107672c10"
    ),
}
_TIMEOUT_VOTE_VIEW_WINDOW_REGRESSION_TEST_SHA256 = {
    "adjacent_future_timeout_votes_form_a_catch_up_certificate": (
        "93e8e493ba5b51262032990d4326960b458fd6d30e2eabc005372b213cff8aae"
    ),
    "timeout_install_preserves_adjacent_shares_for_the_new_current_view": (
        "ad2debc2b510584534a4ca39283b89fa76986004f0dea08dafe130003af7d637"
    ),
    "timeout_votes_beyond_adjacent_lookahead_are_ignored": (
        "f20c28d79268f12a9cfd9c452b496a7b2750210617493a531e9e29f105b92038"
    ),
}
RETIRED_ARBITRARY_PRODUCER_BUDGET_SYMBOLS = (
    "ExactDecisionRequestIngressProducerBudgetResidual",
)
RETIRED_SYNTHETIC_FAIRNESS_SYMBOLS = (
    "ExactDecisionRequestLifecycleOwnedFairAction",
    "ExactDecisionRequestLifecycleOwnedFairActionClosureProperty",
    "ExactDecisionRequestLifecycleOwnedFairActionEnabledAtEpisode",
    "ExactDecisionRequestLifecycleOwnedFairActionConsumesEpisode",
)


def _timeout_capacity_regression_source_fidelity_errors(
    path: Path,
    source: str,
) -> list[str]:
    """Bind externalized adapter capacity regressions through reviewed source."""

    errors: list[str] = []
    test_context = (
        ("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),
    )
    long_tests = {
        "capacity_bypass_records_follow_current_lock_and_timeout_view",
        "certified_timeout_bypasses_hung_signer_and_opens_adjacent_vote",
    }
    items: dict[str, RustItem | None] = {}
    for name, expected_sha256 in (
        _TIMEOUT_VOTE_SEMANTIC_CAPACITY_REGRESSION_TEST_SHA256.items()
    ):
        item = _require_rust_item(path, source, name, errors)
        items[name] = item
        attributes = (
            ("#[test]", "#[allow(clippy::too_many_lines)]")
            if name in long_tests
            else ("#[test]",)
        )
        _require_rust_item_context(
            path,
            item,
            test_context,
            f"TimeoutVote semantic-capacity regression {name}",
            errors,
            expected_attributes=attributes,
        )
        _require_rust_item_token_sha256(
            path,
            item,
            expected_sha256,
            f"TimeoutVote semantic-capacity regression {name}",
            errors,
        )

    for name, expected_sha256 in (
        _LOCKED_REPROPOSAL_PREPARE_PROGRESS_REGRESSION_TEST_SHA256.items()
    ):
        item = _require_rust_item(path, source, name, errors)
        items[name] = item
        _require_rust_item_context(
            path,
            item,
            test_context,
            f"locked-reproposal Prepare Progress regression {name}",
            errors,
            expected_attributes=(
                ("#[test]", "#[allow(clippy::too_many_lines)]")
                if name
                == "deferred_progress_partition_owns_every_vote_and_certificate_class"
                else ("#[test]",)
            ),
        )
        _require_rust_item_token_sha256(
            path,
            item,
            expected_sha256,
            f"locked-reproposal Prepare Progress regression {name}",
            errors,
        )

    _require_rust_token_sequence(
        path,
        items.get("capacity_bypass_records_follow_current_lock_and_timeout_view"),
        """
assert_eq!(
    adapter
        .ingress_equivocations
        .values()
        .filter(|record| record.capacity_bypass)
        .count(),
    roster_len * 4
);
""",
        "capacity-bypass regression must realize exactly one locked Commit, one current locked-reproposal Prepare, and current and adjacent TimeoutVote rosters",
        errors,
    )
    _require_rust_token_sequence(
        path,
        items.get(
            "current_locked_reproposal_prepare_uses_progress_only_after_local_binding"
        ),
        """
assert!(
    !adapter.is_exact_locked_reproposal_prepare_vote(&exact_prepare),
    "remote wire data cannot bootstrap its own local execution binding"
);
adapter
    .registry
    .register_execution_commitment(
        reducer::Round::new(current_tag.height(), current_tag.view()),
        core_subject,
        locked_execution_commitment,
    )
    .expect("bind the locally validated current-round reproposal");
assert!(adapter.is_exact_locked_reproposal_prepare_vote(&exact_prepare));
""",
        "locked-reproposal Prepare regression must deny remote bootstrap and admit only after the pre-existing local current-round binding",
        errors,
    )
    _require_rust_token_sequence(
        path,
        items.get("certified_timeout_bypasses_hung_signer_and_opens_adjacent_vote"),
        """
assert_eq!(adapter.current_tag().view(), current_round.view + 1);
assert!(adapter.deferred_progress_inputs.is_empty());
""",
        "certified-timeout regression must advance the hung signer exactly one view and drain its deferred progress fence",
        errors,
    )

    cross_view_name = (
        "busy_deferred_source_identity_coalesces_across_consumer_view_change"
    )
    cross_view = _require_rust_item(path, source, cross_view_name, errors)
    _require_rust_item_context(
        path,
        cross_view,
        test_context,
        "cross-view Busy-deferred semantic-owner regression",
        errors,
        expected_attributes=("#[test]", "#[allow(clippy::too_many_lines)]"),
    )
    _require_rust_item_token_sha256(
        path,
        cross_view,
        _SERVICED_CANDIDATE_REGRESSION_TEST_SHA256[cross_view_name],
        "cross-view Busy-deferred semantic-owner regression",
        errors,
    )
    _require_rust_token_sequence(
        path,
        cross_view,
        """
assert_eq!(retagged_candidate.0, original_candidate.0);
assert_eq!(retagged_candidate.0.source_view(), first_round.view);
assert_eq!(retagged_candidate.1, adapter.current_tag().view());
""",
        "cross-view Busy-deferred regression must retain source identity while advancing only its consumer episode",
        errors,
    )
    return errors
