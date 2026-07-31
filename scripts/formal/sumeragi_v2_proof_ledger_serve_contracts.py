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
        "ReserveExactServeCapacity",
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
        "ReserveExactServeCapacity",
    ): (
        "AsyncServeOffQueueReservations",
        "AsyncServeIngressPredecessorCounts",
        "asyncNextServeAdmissionOrdinal",
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AdvanceExactServeCapacity",
    ): (
        "AsyncServeOffQueueReservations",
        "AsyncServeFamilyTombstoneRecords",
        "AsyncServeIngressPredecessorCounts",
        "asyncNextServeAdmissionOrdinal",
    ),
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
        "AdequateLeaderRouteNeutralCandidateItem",
        "AdequateLeaderRouteNeutralCandidateEvidence",
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
        "AdequateLeaderFrozenCandidateItemPayloadCarrier",
        "AdequateLeaderFrozenCandidateEvidencePayloadCarrier",
        "AsyncCommandClasses",
        "AsyncWorkKinds",
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
        "AdequateLeaderTargetComposedRankDescentProperty",
    ): (
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
        "AdequateLeaderTargetOccurrenceOwnerSelected",
        "AdequateLeaderTargetLiveCandidateOwnerIdentitySet",
        "AdequateLeaderFrozenSubjectSwitchOwnerUniverse",
        "retired",
        "AdequateLeaderTargetStrictOccurrenceDescentGoal",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetOffSubjectRetirementAndReentryGoal",
    ): (
        "owner",
        "retired",
        "AdequateLeaderTargetOccurrenceOwnerRetirementClosed",
        "AdequateLeaderTargetProductiveSubjectReentryGoal",
    ),
    (
        "SumeragiV2AdequateLeaderServiceClosureProofs",
        "AdequateLeaderTargetSubjectSwitchEpisodeAdvanceGoal",
    ): (
        "AdequateLeaderTargetSubjectSwitchDiscoveredOwnerSet",
        "AdequateLeaderTargetOccurrenceOwnerRetirementClosed",
        "AdequateLeaderTargetSubjectSwitchRemainingBudget",
        "AdequateLeaderTargetSubjectSwitchEpisodeAtBudget",
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
        "AdequateLeaderTargetSubjectSwitchEpisodeAtBudget",
        "AdequateLeaderTargetSubjectSwitchBudgetDescentGoal",
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
            "owner \\in AdequateLeaderFrozenSubjectSwitchOwnerUniverse( "
            "target, leaderContext, leader, leaderView) \\ retired"
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
        (
            "AdequateLeaderTargetOccurrenceOwnerRetirementClosed( target, "
            "leaderContext, leader, leaderView, subject, occurrenceRank, "
            "owner)"
        ),
        "retired2 = retired \\cup discovered",
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

_SERVE_LIFECYCLE_REGRESSION_TEST_SHA256 = {
    "certified_serve_receiver_close_rolls_back_pending_capacity_replacement": (
        "468d01287d92542ffe440414bdaf4313375ea37aafb34e6fa9ac5fe1800107b3"
    ),
    "certified_serve_receiver_close_rolls_back_materialized_unclaimed_replacement": (
        "0b152887eee3cd1b7f556159e6222ce9598eeded46c1e4b427ebfd45b793db4d"
    ),
    "certified_serve_shutdown_rolls_back_materialized_unclaimed_replacement": (
        "66c1018a861b7dc18f36b3d2e9fbbca9ed8cbb68be314f4fb35edf5d215d5d25"
    ),
    "certified_serve_terminal_replay_source_retains_retired_route_and_reconnects": (
        "fcd7b536f2c8c37c759a4bc7594a4aeb96c85b673ad765551cf35fd9a0bee72a"
    ),
    "certified_serve_terminal_replay_waits_for_barrier_then_bypasses_full_serve_fifo": (
        "f2287c813fb08f857814177a37dfbe19e7ad95047ab23ef1a3c5587d008b082a"
    ),
}
_SERVE_INGRESS_ORDINAL_STRUCT_SHA256 = {
    "FairV2IngressState": (
        "d4c2679003b4ca3474033e2048fb4c94a59a3eaccbc13f0876a15e47f04134ca"
    ),
    "FairV2IngressEntry": (
        "81d3ce89a1b7ea4f13c481084e5097c583cbad2c50a1c74929261f42c41c839e"
    ),
}
_SERVE_INGRESS_ORDINAL_REGRESSION_TEST_SHA256 = {
    "fair_v2_ingress_certified_request_cutoff_blocks_later_same_source_serve": (
        "4f1c1269e476e1b9c116044c4cc4a3e4934c57a94dfbc43b7e6d6bdeed187dcc"
    ),
    "fair_v2_ingress_certified_request_cutoff_blocks_later_churn": (
        "ac57535058bc5dd4d724d1e6d56c0fe15e41424e7bbf578ba2eb99ae4111b8ac"
    ),
    "fair_v2_ingress_occurrence_ordinal_coalesces_and_overflow_closes": (
        "d428ca8360f6fac1dae8e57e260980fe8fae39f0501adb255d03222a695c89c7"
    ),
    "restored_productive_retry_stays_behind_an_earlier_certified_request_carrier": (
        "447627f22757f044e5cb8007715375b2bc46f7ddb23398469b02d90368250556"
    ),
    "restored_productive_retry_ordinal_exhaustion_keeps_the_owner_dormant": (
        "e2850b0677b5219e0960641d23b79a679c1fcc84faaaa7c83db0cfe79a3e6e3b"
    ),
}
_SERVE_INGRESS_GATE_IMPL_ITEM_SHA256 = {
    "require_certified_serve_gate": (
        "96b2844effe8a496a06b247860b8790227f1e0c31a4468bff52eeb0631cbe10b"
    ),
    "bind_certified_serve_gate": (
        "ed0997516f643a29a4da048b9983d26f2a8b64123888555b7bec50319efbda66"
    ),
    "unbind_certified_serve_gate": (
        "16c1655f5a4fc1e1cfd65133e09a9dc336eed6cceb7470bd3bc83993d52ee034"
    ),
}
_SERVE_INGRESS_GATE_REGRESSION_TEST_SHA256 = {
    "fair_v2_ingress_required_serve_gate_precedes_open": (
        "3b929bb4ca3548c93685c1b7c2bad0036d666cff96d8c19aa5f9fe93a74c1e23"
    ),
}
_SERVE_INGRESS_GATE_WORKER_REGRESSION_TEST_SHA256 = {
    "fair_ingress_exact_ticket_coalesces_and_commits_before_later_io_producers": (
        "f330e8cbb207fd3872c81d79817209a35cf7acfc2f033cc32f8c1b37ec232f19"
    ),
    "fair_ingress_gate_overflow_closes_without_partial_admission": (
        "820e286fc897de3e36b4463d707faee647de8db649554e8a48ce2efe48afc419"
    ),
    "fair_ingress_rollover_retires_ticket_before_old_service_teardown": (
        "16712f671a24fd8c0bc00543719eb8ac4b4022789274803292fb1f7219ed27c3"
    ),
    "selected_serve_physical_carrier_precedes_reactivated_older_leader_lifecycle": (
        "7d4298be7b35579fb36dd79bf8224325aa63e4ad084763e02c8978263fe4726f"
    ),
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
        "42d86efe92417da13ef59dcd3c2d8b82514ce1f8504e167299743269e18431c1"
    ),
    "open_deferred_status_with_capacity_geometry": (
        "2762f10d6cf6f6d9d90418d25fdb5019e4a6de872fe9b9ffcdb0f6e20dd6a770"
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
_SERVICED_CANDIDATE_RUNNER_ITEM_SHA256 = {
    "run_inner": (
        "e106642979361941b3819c18034c57a0bf506981bb6cf05b6ff472b22b3989d9"
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
        "0211f040af62cc365cba5896c4b9479a60cbee09a41b1fbdc61d295000ae1494"
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
    "PersistedServicedCandidatesV3": (
        "bb1e47bffc2d32ebcea3f96f98072eca3713fb508264bd6624380af71e061d67"
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
        "7098d19291be10396014a59d8083053dcbbb28bad1e80e1be329f63beb503791"
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
        "5565b0ee9e26b188138e5dc31ed5da9766cdc249b5c49df42f0fd237a18a0f63"
    ),
    "store_open": (
        "89ddfaf9974da9f175de80d61767eb020c49b5672c2a73de8610fddd8d0b0e76"
    ),
    "store_open_with_capacities": (
        "5b754c14060c56c57d91ec48147d0afd45df29e1215f543703298a606469cfc3"
    ),
    "store_load": (
        "8ad2fa2d46a62e9428cd175f7869c8a247fe53776b1a9c0c01c7fb028a9799d5"
    ),
    "store_reserve_producer_continuation": (
        "452de9ed84f0d1f83c17df83d58f87f7530b60e98d022f11694e9896a5181b4e"
    ),
    "store_persist_with_producer_continuations": (
        "6e358bdb8fe65c77795efd88adc49a6054b95e812217854d2757e07aa028b942"
    ),
    "encode_payload_frame": (
        "7cb4861444e43a3904493bfcb15ff8eb92f5ee6fdbe763eec691701a11efa71f"
    ),
    "encode_frame_v4": (
        "14296fa8c429ee49f9ccba935bdc06f984941eefda1b0e995d091741e6130ad2"
    ),
    "encode_frame_v3": (
        "bd12646c0dfab04045c7d8527c2c920e31fc7bf8200058ffe87589b86ef0b195"
    ),
    "decode_frame": (
        "c6d86c1a78e3df5299570c3a409750c852ba7552ce54fcbff48b5e4833bf7d48"
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
    "producer_parent_replay_source_for_stage": (
        "de61146d932b8f0f65a799f8102e83013fce7f875e82d591b22528c75caab41f"
    ),
    "producer_parent_is_locally_reconstructible": (
        "6d46edcfbf32039e6c1f17bd7a1fa0203a8faa6069f2b69fcf5f2684c0bcd458"
    ),
    "producer_parent_has_exact_local_replay_binding": (
        "235feed79743cfcdff3ad31fd607c05546ad593ee8d5bd76dbdf6eaf40b2120c"
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
        "1daf3c9bf5c7ddb904080ee76f476d33d4ca50e252fe906fdfae7dc5080c13b6"
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
        "c171f50e2609887556ec8dd796d59b1b064787d2d972f3c0c939910c87f1a122"
    ),
    "persist_producer_lifecycles": (
        "923100ada06b4b713c15ed273d9306f4d592e9a25194d3be5d355d6077ea94b9"
    ),
    "rollback_producer_reservation": (
        "f6a2ae48d02fa974e796886695810d7518cd0530fc3578f9b36f3dee3896fb08"
    ),
    "release_unrecorded_producer": (
        "f76e88fba43eb1e40b178a5b6817d35c381bfb629666779e7a632af3f3bf60bf"
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
        "f08d28569d85834d2f0b86053673075e60f8db38c9b7bd6708002f7f55b3e9b0"
    ),
    "producer_handoff_evidence": (
        "2b553732a9df3b84f8d4b8495b063129278080a7a775624d13c6ac9b87278a39"
    ),
    "acknowledge_producer_handoff": (
        "038ccb3a1d6c463f3b10f01491139d5fcbe4827d5a8efad09a7c54de6134bc79"
    ),
    "reclaim_serviced_candidates": (
        "4b6695268578047cfb71599953c31e4e601049e0de55340616944d1dd6fc28d0"
    ),
    "step_with_defer_policy": (
        "ce2ac96788d35d6b7592c9896871d3e7abc24a0896aa76588f0d6c5497570ae4"
    ),
    "drain_deferred_with_handoff_for_ordinals": (
        "a3e97e5ef3e2f39b2a563512d858e586633353900933be31dcceff811433bcfd"
    ),
    "drive_effects": (
        "98ff14c212f7c5f8b1756e7d8661a3ea502dfc334fc22ca592a7f85958909f29"
    ),
}
_SERVICED_CANDIDATE_V4_RUNTIME_STRUCT_SHA256 = {
    "RuntimeDormantLocalFifoReservation": (
        "6e85865f52ff4ce41ec7d2d3a363927035b2e2f3a87ad34424943030bcc34ad6"
    ),
    "BoundedIngress": (
        "a4d78a51426a19aef155655baaaef40487fc64e23eb01ba89d02581020d02516"
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
        "42c699bccbd2f051c8300cc714dab9b3f32c0268a37d95226f0ecb466dabc2b2"
    ),
    "dormant_local_fifo_replacement": (
        "349d197fc00128460c723ecb4cc4fc7b3253390d9847b61dac38e7157fb9af61"
    ),
    "occupied_with_dormant_reservations": (
        "2a57306de4cfb2074082a0f67c34ffe9cb3d99ce2021c31256272d268a1bbe0e"
    ),
    "oldest_active_lifecycle_ordinal": (
        "fa6fbffb7201f69529caec900bafba3eb4f9433f00ec31289ca3e23e2363c7d4"
    ),
    "with_driver_and_lifecycle_ordinals": (
        "8b34854604799e519f0e40d85bac7f72564ee67ae94c6731222f45af13bcc863"
    ),
    "minimum_active_lifecycle_ordinal": (
        "0c38e2c8725a12a05c06884f8ae22d3ea7c6395243dc08800afdbf4d1190d812"
    ),
    "complete_leader_wire_runtime_owner": (
        "e17e62beccb6e2e219f3aac01c126456531ffb0448930b83026d9cf02da6695c"
    ),
    "step": (
        "25956cf04532441fca915496116cc9c6913df8743292c857fd478a2ba7b07bcc"
    ),
    "dispatch_one_adapter_deferred": (
        "20d4d698e8566e7df3ba96d0f6cc136ad87c05f10d95931b886aa174ae45b7b9"
    ),
}
_SERVICED_CANDIDATE_V4_RUNNER_ITEM_SHA256 = {
    "run_inner": (
        "bfef5a8070c6c26f13509e97f0c744f856c68d454165657043195fe1b122b307"
    ),
}
_SERVICED_CANDIDATE_V4_STORE_REGRESSION_TEST_SHA256 = {
    "snapshot_roundtrips_and_rejects_a_b_a_resurrection": (
        "d19e863f444d25bb405fcfc183438c9abd6e64fd7ce82453c91059bb2874007d"
    ),
    "v4_roundtrips_terminal_producer_continuations_and_v3_upgrades_canonically": (
        "10a637f7e21143470bdd72792630231d35801c52bbfa745786c836067b7dd244"
    ),
    "leader_wire_gate_reconciles_producer_first_terminal_crash": (
        "f225f861684b5a468fd1e45a9424e614b91b8bfebfd75e10e429c91dbd32b58d"
    ),
    "leader_wire_gate_rejects_producer_terminal_from_foreign_view_or_phase": (
        "8db1b977e059aadd6bbce9e3746e4fea23167446b3fb64e058616eb41d7cb96a"
    ),
    "snapshot_rejects_corruption_stale_context_and_capacity_exhaustion": (
        "9092b4ebab5ec88f8ad99a573352aedb05fe29385f5ff01215bf088c35511059"
    ),
    "decision_reclamation_is_canonical_only_for_an_empty_snapshot": (
        "ef03bf7b54f58eb0ee9e358a780c023aedce3abc15e0e453041c6b1a905c9d9a"
    ),
    "snapshot_rejects_truncation_version_ordering_duplicates_and_oversize": (
        "11a74c656afed6467421484b12819a41ae85d8f974e5baf22deaac81d6f218ac"
    ),
    "v4_rejects_noncanonical_or_over_capacity_producer_tables": (
        "5067bd4555946a59ee5abe37e5b9f473df832bb3f0337f15f14c269d78b8b578"
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
        "4222e8a1cfb308f0fb76cec9ca1fc8e0f57c19c70cf425dad9648afa402e63d9"
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
        "09a0afa0c3ec41ef59ba88d093180fff339408e66a5325d3f3086485c2498381"
    ),
    "terminal_producer_tombstone_survives_restart_blocks_aba_and_advances_shared_source": (
        "3c64469fa90eddc9df2a6fa0f63abc93e403761ae64797a7ef6d96cee4e4f122"
    ),
    "serviced_candidate_reclaim_failure_fail_stops_then_replay_reclaims": (
        "0bcfb8cd24bde2a60cbe0132dfc76bbf7633734ec8121d796c55a7cf8a62a72e"
    ),
    "serviced_candidate_snapshot_is_bound_to_the_local_validator_owner": (
        "e2d69a7243f71d2818ffabcbdd78e9585c6245b447a882e8f6dc319067a956ee"
    ),
    "aggregate_carrier_and_priority_variants_coalesce_to_one_semantic_candidate": (
        "ba375e6b8c6f36c2499442618aac887719c64f89a44e7c9739bfd6e06b16fda9"
    ),
    "serviced_candidate_capacity_exhaustion_never_evicts_an_old_owner": (
        "ed7172e200b6ebe76d6c6048a08e6610b83e709b66dc8e0dd972998d5e8f2b0f"
    ),
    "post_wal_oversized_continuation_fails_closed_and_replays_exact_record": (
        "a357f97d00052efa6f464ac0571f3440c8ff94f8a459883be3d864adfb3ec91f"
    ),
}
_SERVICED_CANDIDATE_V4_RUNTIME_REGRESSION_TEST_SHA256 = {
    "restart_dormant_local_fifo_reservation_survives_full_class_churn": (
        "653fac9ef34bfb376711f643fe231ee9860cb7c1ef29570d514a3f870a531c1c"
    ),
    "restart_dormant_completion_batch_atomically_replaces_latent_slots": (
        "873666090e3d2a5ddb8e06524080c9b7eefb120b35a0af25775b7f00ed827397"
    ),
    "dormant_local_fifo_metadata_rejects_wrong_stage_ordinal_and_capacity": (
        "456409603047b47666be7596841a38ea734112a9554722274a09582d32d1ec69"
    ),
    "restored_exact_stage_coalesces_at_full_capacity_without_aliasing_successors": (
        "48b4fb3b9c32dd7b8230e757d04c4b6b88de00a9ccaef6d44e6fcdc37c71fe96"
    ),
    "restored_producer_preflight_cannot_change_completion_service_class": (
        "bd1273f883c17b13d555ed690c5cfaec2b62ab159c20e2be49def98a3058e7e2"
    ),
    "restored_serve_high_watermark_precedes_startup_runtime_owner": (
        "73e82a3200d1ad1bc6f7a678f3632e3d885a9ae8d0367fb9aa907feb10584aa0"
    ),
    "busy_deferred_older_aggregate_rebases_owner_and_rejects_identity_mutation": (
        "f29534831f806e2bad3c57424aaa768f206f74e4615008397b96d3e2f137a104"
    ),
}
_SERVICED_CANDIDATE_V4_WORKER_REGRESSION_TEST_SHA256 = {
    "invalid_requester_signed_qc_quarantines_one_family_without_consuming_honest_capacity": (
        "a64e65c344cd7a423ff744f2e651356d0aae41e5878097c8df18af92e61cf66b"
    ),
}
_TIMEOUT_VOTE_SEMANTIC_CAPACITY_ITEM_SHA256 = {
    "semantic_ingress_capacity": (
        "b65b195193821ca2203d872d41c34e052346380a00ca7aca1b8ec8e743cffb96"
    ),
    "admit_authenticated_payload": (
        "2048303131d791463528a4449228116952b25eab8e473175f4a5d65578e16e24"
    ),
    "prune_ingress_records": (
        "abc6c048ae87f9b14930fe3b35019c470b55d42a99dc4c65daeb0678137baa7d"
    ),
}
_TIMEOUT_VOTE_SEMANTIC_CAPACITY_REGRESSION_TEST_SHA256 = {
    "capacity_bypass_records_follow_current_lock_and_timeout_view": (
        "de482b2922e449c2a988e2125d1505c521d176921d61611f6f78eb25db4301ae"
    ),
    "full_normal_deferred_lane_cannot_drop_absolute_timeout": (
        "db11edaeae4b12ef18085718d49f792ee0d285f0bea71e1773174c6f5b6f4384"
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
