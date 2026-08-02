# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

# The fixed-corridor clock proof is still a static arithmetic layer: these
# contracts pin its non-vacuous finite token and physical-rank interfaces
# without treating them as a live Decision-before-deadline provider.  In
# particular, no entry here changes a proof-ledger status.
QUANTITATIVE_FIXED_CORRIDOR_OPERATOR_BODIES = {
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalExactRemainingOccurrenceBudget",
    ): (
        'CASE kind = "BeginTimeout" -> 19 '
        '[] kind \\in {"PersistTimeout", "DeliverTC"} -> 18 '
        "[] kind \\in "
        '{"SignTimeout", "DeliverTimeout", "BeginInstallTC"} -> 17 '
        '[] kind = "PersistInstallTC" -> 16 '
        '[] kind = "DeliverProposal" -> 14 '
        '[] kind \\in {"DeliverVote", "DeliverQC"} -> 13 '
        '[] kind \\in {"FormCommitQC", "BeginDecision"} -> 12 '
        '[] kind \\in {"DeliverChunk", "PersistDecision"} -> 11 '
        "[] kind \\in "
        '{"FetchBody", "RebindRetainedBody", "FetchCertifiedBody"} -> 10 '
        '[] kind = "StoreBody" -> 9 '
        '[] kind = "ValidateBody" -> 8 '
        '[] kind = "BeginObservePrepare" -> 5 '
        '[] kind \\in {"AssembleBody", "PersistObservePrepare"} -> 4 '
        "[] kind \\in "
        '{"BeginProposal", "BeginPrepare", "BeginLockCommit"} -> 3 '
        "[] kind \\in "
        '{"PersistProposal", "PersistPrepare", "PersistLockCommit"} -> 2 '
        "[] OTHER -> 1"
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCommandExactSuccessorBatchOccurrenceBudget",
    ): (
        "LET successors == CommandSuccessors(command) IN "
        "CASE Len(successors) = 0 -> 0 "
        "[] Len(successors) = 1 -> "
        "AsyncCausalExactRemainingOccurrenceBudget(successors[1].kind) "
        "[] Len(successors) = 2 -> "
        "AsyncCausalExactRemainingOccurrenceBudget(successors[1].kind) "
        "+ AsyncCausalExactRemainingOccurrenceBudget( successors[2].kind) "
        "[] Len(successors) = 3 -> "
        "AsyncCausalExactRemainingOccurrenceBudget(successors[1].kind) "
        "+ AsyncCausalExactRemainingOccurrenceBudget( successors[2].kind) "
        "+ AsyncCausalExactRemainingOccurrenceBudget( successors[3].kind) "
        "[] OTHER -> "
        "AsyncCausalExactRemainingOccurrenceBudget(command.kind)"
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeExactCandidateOccurrenceTokens",
    ): (
        "{<<candidate, token>>: candidate \\in "
        "AsyncCausalEpisodeCandidates(node, cutoffOrdinal), token \\in "
        "1..AsyncCausalExactRemainingOccurrenceBudget(candidate.kind)}"
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeExactCandidateOccurrenceBudget",
    ): (
        "Cardinality( AsyncCausalEpisodeExactCandidateOccurrenceTokens( "
        "node, cutoffOrdinal))"
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeLifecycleCutOwned",
    ): (
        "\\E record \\in AsyncCandidateLifecycleAdmissions: "
        "/\\ record.node = node "
        "/\\ record.origin = origin "
        "/\\ record.ordinal = cutoffOrdinal"
    ),
    ("SumeragiV2AsyncNetwork", "AsyncRunnerCycleBudget"): (
        "AsyncQueueCapacity + 2 * AsyncIngressCapacity + 3"
    ),
    ("SumeragiV2AsyncNetwork", "AsyncRuntimeCycleBudget"): (
        "3 * AsyncQueueCapacity * AsyncRunnerCycleBudget"
    ),
    ("SumeragiV2AsyncNetwork", "AsyncIoDrainBudget"): (
        "AsyncIoAuxCapacity + AsyncIoWorkCapacity + 1"
    ),
    ("SumeragiV2AsyncNetwork", "AsyncDeferredDrainBudget"): (
        "2 * AsyncDeferredNormalCapacity + AsyncDeferredProgressCapacity "
        "+ AsyncCompletionReserve"
    ),
    ("SumeragiV2AsyncNetwork", "AsyncCausalCandidateLifecycleCapacity"): (
        "3 * AsyncQueueCapacity"
    ),
    ("SumeragiV2AsyncNetwork", "AsyncCandidateProducerEpisodeCapacity"): (
        "AsyncQueueCapacity + 2 * AsyncDeferredNormalCapacity "
        "+ AsyncDeferredProgressCapacity "
        "+ AsyncCausalCandidateLifecycleCapacity + AsyncIoWorkCapacity"
    ),
    ("SumeragiV2AsyncNetwork", "AsyncCandidateProducerEpisodeBudget"): (
        "19 * AsyncCandidateProducerEpisodeCapacity"
    ),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateProducerActionEpisodeBudget",
    ): ("72 * AsyncCandidateProducerEpisodeCapacity"),
    (
        "SumeragiV2AsyncNetwork",
        "AsyncCandidateProducerContinuationHandoffCandidatesThisStep",
    ): (
        "IF candidate \\in AsyncCandidateServicesThisStep "
        "THEN SequenceSet(CommandSuccessors(candidate)) ELSE {}"
    ),
    ("SumeragiV2AsyncNetwork", "AsyncCandidatePhysicalServiceBudget"): (
        "AsyncCandidateProducerActionEpisodeBudget + AsyncRuntimeCycleBudget "
        "+ 4 * AsyncDeferredDrainBudget "
        "+ 6 * AsyncIoDrainBudget"
    ),
    ("SumeragiV2AsyncNetwork", "AsyncProposalPipelineBudget"): (
        "4 * N * (AsyncChunkCount + 8) "
        "* (AsyncCandidatePhysicalServiceBudget + 1)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedPipelinePhases",
    ): '{"Proposal", "Prepare", "Commit", "Decision"}',
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedPipelineTokenCarrier",
    ): (
        "AdequateLeaderFrozenResponsiveRoster(leaderContext) "
        "\\X AdequateLeaderFixedPipelinePhases"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedDeferredPositionCeiling",
    ): "3 * AsyncDeferredDrainBudget + 2",
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedRuntimePositionCeiling",
    ): "3 * AsyncQueueCapacity + 2",
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedReadyPositionCeiling",
    ): "4 * AsyncIoWorkCapacity + 3",
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedIoPositionCeiling",
    ): (
        "AsyncIoCapacity + AsyncIoWorkCapacity + AsyncQueueCapacity "
        "+ AsyncDeferredNormalCapacity"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCausalPositionCeiling",
    ): "2 * AsyncCausalCandidateLifecycleCapacity + 1",
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCandidatePhysicalWindowBudget",
    ): (
        "AdequateLeaderFixedDeferredPositionCeiling "
        "+ AdequateLeaderFixedRuntimePositionCeiling "
        "+ AdequateLeaderFixedReadyPositionCeiling "
        "+ AdequateLeaderFixedIoPositionCeiling "
        "+ AdequateLeaderFixedCausalPositionCeiling + 4"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCandidatePhysicalRankFrom",
    ): (
        "CASE rank[1] = 2 -> rank[2] [] rank[1] = 3 -> "
        "AdequateLeaderFixedDeferredPositionCeiling + 1 + rank[2] "
        "[] rank[1] = 4 -> AdequateLeaderFixedDeferredPositionCeiling + 1 "
        "+ AdequateLeaderFixedRuntimePositionCeiling + 1 + rank[2] "
        "[] rank[1] = 5 -> AdequateLeaderFixedDeferredPositionCeiling + 1 "
        "+ AdequateLeaderFixedRuntimePositionCeiling + 1 "
        "+ AdequateLeaderFixedReadyPositionCeiling + 1 + rank[2] "
        "[] rank[1] = 6 -> AdequateLeaderFixedDeferredPositionCeiling + 1 "
        "+ AdequateLeaderFixedRuntimePositionCeiling + 1 "
        "+ AdequateLeaderFixedReadyPositionCeiling + 1 "
        "+ AdequateLeaderFixedIoPositionCeiling + 1 + rank[2] "
        "[] OTHER -> 0"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCandidatePhysicalRank",
    ): (
        "AdequateLeaderFixedCandidatePhysicalRankFrom( "
        "CandidateServiceRank(candidate))"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCrossChildPhysicalDebt",
    ): (
        "AsyncCausalEpisodeExactCandidateOccurrenceBudget( "
        "node, cutoffOrdinal)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedPipelineExactParentDeparture",
    ): (
        "/\\ CandidateScheduled(parent) "
        "/\\ ~CandidateScheduled(parent)' "
        "/\\ \\E rank \\in AdequateLeaderTargetSemanticRankCarrier: "
        "ExactLeaderStaticSemanticRank(parent, rank) "
        "/\\ \\E child \\in AsyncCandidateSet: "
        "/\\ child \\in SequenceSet(CommandSuccessors(parent)) "
        "/\\ CandidateScheduled(child)'"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep",
    ): (
        "LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(parent) "
        "origin == parent.causalOrigin IN "
        "/\\ AsyncCausalEpisodeLifecycleCutOwned( "
        "parent.node, origin, cutoffOrdinal) "
        "/\\ (AsyncCausalEpisodeLifecycleCutOwned( "
        "parent.node, origin, cutoffOrdinal))' "
        "/\\ \\A child \\in AsyncCandidateSet: "
        "/\\ child \\in SequenceSet(CommandSuccessors(parent)) "
        "/\\ CandidateScheduled(child)' => "
        "/\\ child.causalOrigin = origin "
        "/\\ AsyncCandidateLifecycleOrdinal(child)' = cutoffOrdinal"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCrossChildPhysicalRankResetThisStep",
    ): (
        "/\\ CandidateScheduled(parent) "
        "/\\ ~CandidateScheduled(parent)' "
        "/\\ child \\in SequenceSet(CommandSuccessors(parent)) "
        "/\\ CandidateScheduled(child)' "
        "/\\ child.causalOrigin = parent.causalOrigin "
        "/\\ AsyncCandidateLifecycleOrdinal(child)' "
        "= AsyncCandidateLifecycleOrdinal(parent) "
        "/\\ AdequateLeaderFixedCandidatePhysicalRank(child)' "
        "> AdequateLeaderFixedCandidatePhysicalRank(parent)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCrossChildSuccessorBatchConsumes",
    ): (
        "AsyncCommandExactSuccessorBatchOccurrenceBudget(parent) "
        "< AsyncCausalExactRemainingOccurrenceBudget(parent.kind)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedInitialCandidateRouteActionCredit",
    ): ('IF commandClass = "Completion" THEN 4 ELSE 2'),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCandidateSuccessorTailActionCredit",
    ): (
        'CASE kind = "BeginTimeout" -> 68 '
        '[] kind = "PersistTimeout" -> 64 '
        '[] kind = "DeliverTC" -> 62 '
        '[] kind \\in {"SignTimeout", "DeliverTimeout", "BeginInstallTC"} '
        "-> 60 "
        '[] kind = "PersistInstallTC" -> 56 '
        '[] kind = "DeliverProposal" -> 48 '
        '[] kind \\in {"DeliverVote", "DeliverQC"} -> 44 '
        '[] kind \\in {"FormCommitQC", "BeginDecision"} -> 42 '
        '[] kind \\in {"DeliverChunk", "PersistDecision"} -> 38 '
        "[] kind \\in "
        '{"FetchBody", "RebindRetainedBody", "FetchCertifiedBody"} -> 34 '
        '[] kind = "StoreBody" -> 30 '
        '[] kind = "ValidateBody" -> 26 '
        '[] kind = "BeginObservePrepare" -> 16 '
        '[] kind \\in {"AssembleBody", "PersistObservePrepare"} -> 12 '
        "[] kind \\in "
        '{"BeginProposal", "BeginPrepare", "BeginLockCommit"} -> 8 '
        "[] kind \\in "
        '{"PersistProposal", "PersistPrepare", "PersistLockCommit"} -> 4 '
        "[] OTHER -> 0"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedExactCandidateActionCredit",
    ): (
        "AdequateLeaderFixedInitialCandidateRouteActionCredit(commandClass) "
        "+ AdequateLeaderFixedCandidateSuccessorTailActionCredit(kind)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCommandSuccessorBatchActionCredit",
    ): (
        "LET successors == CommandSuccessors(command) IN "
        "CASE Len(successors) = 0 -> 0 "
        "[] Len(successors) = 1 -> "
        "AdequateLeaderFixedExactCandidateActionCredit( "
        "successors[1].class, successors[1].kind) "
        "[] Len(successors) = 2 -> "
        "AdequateLeaderFixedExactCandidateActionCredit( "
        "successors[1].class, successors[1].kind) "
        "+ AdequateLeaderFixedExactCandidateActionCredit( "
        "successors[2].class, successors[2].kind) "
        "[] Len(successors) = 3 -> "
        "AdequateLeaderFixedExactCandidateActionCredit( "
        "successors[1].class, successors[1].kind) "
        "+ AdequateLeaderFixedExactCandidateActionCredit( "
        "successors[2].class, successors[2].kind) "
        "+ AdequateLeaderFixedExactCandidateActionCredit( "
        "successors[3].class, successors[3].kind) "
        "[] OTHER -> "
        "AdequateLeaderFixedCandidateSuccessorTailActionCredit( "
        "command.kind)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedRouteActionCreditFromStage",
    ): (
        "CASE stage = 6 -> "
        "AdequateLeaderFixedInitialCandidateRouteActionCredit(commandClass) "
        "[] stage = 5 -> 3 "
        "[] stage = 4 -> 2 "
        "[] stage \\in 2..3 -> 1 "
        "[] OTHER -> 0"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCandidateRemainingRouteActionCredit",
    ): (
        "AdequateLeaderFixedRouteActionCreditFromStage( "
        "candidate.class, CandidateServiceRank(candidate)[1])"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCandidateRemainingActionCredit",
    ): (
        "AdequateLeaderFixedCandidateRemainingRouteActionCredit(candidate) "
        "+ AdequateLeaderFixedCandidateSuccessorTailActionCredit("
        "candidate.kind)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCutCumulativeActionTokens",
    ): (
        "{<<candidate, token>>: candidate \\in "
        "AsyncCausalEpisodeCandidates(node, cutoffOrdinal), token \\in "
        "1..AdequateLeaderFixedCandidateRemainingActionCredit(candidate)}"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCutCumulativeActionDebt",
    ): (
        "Cardinality( AdequateLeaderFixedCutCumulativeActionTokens("
        "node, cutoffOrdinal))"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedFinalRouteParentDeparture",
    ): (
        "/\\ AdequateLeaderFixedPipelineExactParentDeparture(parent) "
        "/\\ CandidateServiceRank(parent)[1] \\in 2..3 "
        "/\\ AdequateLeaderFixedCandidateRemainingRouteActionCredit(parent) "
        "= 1"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedIntermediateRouteStageMove",
    ): (
        "/\\ commandClass \\in AsyncCommandClasses "
        "/\\ beforeStage \\in 2..6 "
        "/\\ afterStage \\in 2..6 "
        "/\\ \\/ afterStage = beforeStage "
        "\\/ /\\ afterStage < beforeStage "
        '/\\ ~(commandClass # "Completion" '
        "/\\ beforeStage = 6 "
        "/\\ afterStage = 5) "
        "\\/ /\\ beforeStage = 2 "
        "/\\ afterStage = 3"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedIntermediateRouteCarrierMove",
    ): (
        "LET frozenCandidates == "
        "AsyncCausalEpisodeCandidates(node, cutoffOrdinal) "
        "beforeStage == CandidateServiceRank(candidate)[1] "
        "afterStage == CandidateServiceRank(candidate)'[1] IN "
        "/\\ candidate \\in frozenCandidates "
        "/\\ candidate \\in "
        "(AsyncCausalEpisodeCandidates(node, cutoffOrdinal))' "
        "/\\ (AsyncCausalEpisodeCandidates(node, cutoffOrdinal))' "
        "= frozenCandidates "
        "/\\ AdequateLeaderFixedIntermediateRouteStageMove( "
        "candidate.class, beforeStage, afterStage) "
        "/\\ \\A other \\in frozenCandidates \\ {candidate}: "
        "AdequateLeaderFixedCandidateRemainingRouteActionCredit(other)' "
        "= AdequateLeaderFixedCandidateRemainingRouteActionCredit(other)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCumulativeActionDebtCarryAction",
    ): (
        "\\A parent \\in AsyncCandidateSet: "
        "AdequateLeaderFixedFinalRouteParentDeparture(parent) => "
        "LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(parent) IN "
        "/\\ AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep(parent) "
        "/\\ (AdequateLeaderFixedCutCumulativeActionDebt( "
        "parent.node, cutoffOrdinal))' "
        "< AdequateLeaderFixedCutCumulativeActionDebt( "
        "parent.node, cutoffOrdinal)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCumulativeActionDebtCarryProperty",
    ): (
        "specification => "
        "[][AdequateLeaderFixedCumulativeActionDebtCarryAction]_AsyncAllVars"
    ),
}

QUANTITATIVE_FIXED_CORRIDOR_THEOREM_STATEMENTS = {
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalExactRemainingOccurrenceBudgetIsBounded",
    ): (
        "\\A kind \\in AsyncWorkKinds: "
        "AsyncCausalExactRemainingOccurrenceBudget(kind) \\in 1..19"
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCommandExactSuccessorBatchStrictlyConsumesOccurrenceBudget",
    ): (
        "\\A command \\in AsyncCandidateSet: "
        "AsyncCommandExactSuccessorBatchOccurrenceBudget(command) "
        "< AsyncCausalExactRemainingOccurrenceBudget(command.kind)"
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeCandidateCarrierHasConfiguredBound",
    ): (
        "\\A node \\in ValidatorIds, cutoffOrdinal \\in Nat: "
        "AsyncStrongTypeInvariant => "
        "/\\ IsFiniteSet( AsyncCausalEpisodeCandidates("
        "node, cutoffOrdinal)) "
        "/\\ Cardinality( AsyncCausalEpisodeCandidates("
        "node, cutoffOrdinal)) "
        "<= AsyncCandidateProducerEpisodeCapacity"
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeExactOccurrenceBudgetFitsConfiguredEpisode",
    ): (
        "\\A node \\in ValidatorIds, cutoffOrdinal \\in Nat: "
        "AsyncStrongTypeInvariant => "
        "/\\ IsFiniteSet( "
        "AsyncCausalEpisodeExactCandidateOccurrenceTokens( "
        "node, cutoffOrdinal)) "
        "/\\ AsyncCausalEpisodeExactCandidateOccurrenceBudget( "
        "node, cutoffOrdinal) "
        "<= AsyncCandidateProducerEpisodeBudget"
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeServicedCandidateConsumesExactOccurrenceBudget",
    ): (
        "\\A target, serviced \\in AsyncCandidateSet: "
        "LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(target) IN "
        "/\\ gst "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ ProtectedCandidateOwned(target) "
        "/\\ serviced \\in AsyncCausalEpisodeCandidates( "
        "target.node, cutoffOrdinal) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ ~CandidateScheduled(serviced)' "
        "/\\ ProtectedCandidateOwned(target)' "
        "=> AsyncCausalEpisodeExactCandidateOccurrenceBudget( "
        "target.node, cutoffOrdinal)' "
        "< AsyncCausalEpisodeExactCandidateOccurrenceBudget( "
        "target.node, cutoffOrdinal)"
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeSameOriginHandoffRetainsLifecycleCut",
    ): (
        "\\A parent, child \\in AsyncCandidateSet: "
        "LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(parent) IN "
        "/\\ gst "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateLifecycleSchedulerCoverageInvariant "
        "/\\ CandidateScheduled(parent) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ AsyncCandidateLifecycleSchedulerCoverageInvariant' "
        "/\\ child.node = parent.node "
        "/\\ child.causalOrigin = parent.causalOrigin "
        "/\\ CandidateScheduled(child)' "
        "=> /\\ AsyncCausalEpisodeLifecycleCutOwned( "
        "parent.node, parent.causalOrigin, cutoffOrdinal) "
        "/\\ (AsyncCausalEpisodeLifecycleCutOwned( "
        "parent.node, parent.causalOrigin, cutoffOrdinal))' "
        "/\\ AsyncCandidateLifecycleOrdinal(child)' = cutoffOrdinal"
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeOwnedLifecycleCutCannotReplenish",
    ): (
        "\\A node \\in ValidatorIds, origin, cutoffOrdinal \\in Nat \\ {0}: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCausalEpisodeLifecycleCutOwned( "
        "node, origin, cutoffOrdinal) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ (AsyncCausalEpisodeLifecycleCutOwned( "
        "node, origin, cutoffOrdinal))' "
        "=> (AsyncCausalEpisodeFrozenPredecessorOrigins( "
        "node, cutoffOrdinal))' \\subseteq "
        "AsyncCausalEpisodeFrozenPredecessorOrigins( "
        "node, cutoffOrdinal)"
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeOwnedLifecycleServeCutCannotReplenish",
    ): (
        "\\A node \\in ValidatorIds, origin, cutoffOrdinal \\in Nat \\ {0}: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCausalEpisodeLifecycleCutOwned( "
        "node, origin, cutoffOrdinal) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ (AsyncCausalEpisodeLifecycleCutOwned( "
        "node, origin, cutoffOrdinal))' "
        "=> (AsyncCausalEpisodeServeIngressIdentities( "
        "node, cutoffOrdinal))' \\subseteq "
        "AsyncCausalEpisodeServeIngressIdentities( "
        "node, cutoffOrdinal)"
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeOwnedCutServiceConsumesExactOccurrenceBudget",
    ): (
        "\\A node \\in ValidatorIds, origin, "
        "cutoffOrdinal \\in Nat \\ {0}, "
        "serviced \\in AsyncCandidateSet: "
        "/\\ gst "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCausalEpisodeLifecycleCutOwned( "
        "node, origin, cutoffOrdinal) "
        "/\\ serviced \\in "
        "AsyncCausalEpisodeCandidates(node, cutoffOrdinal) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ ~CandidateScheduled(serviced)' "
        "/\\ (AsyncCausalEpisodeLifecycleCutOwned( "
        "node, origin, cutoffOrdinal))' "
        "=> AsyncCausalEpisodeExactCandidateOccurrenceBudget( "
        "node, cutoffOrdinal)' "
        "< AsyncCausalEpisodeExactCandidateOccurrenceBudget( "
        "node, cutoffOrdinal)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedPipelinePhasesHaveCardinalityFour",
    ): (
        "/\\ IsFiniteSet(AdequateLeaderFixedPipelinePhases) "
        "/\\ Cardinality(AdequateLeaderFixedPipelinePhases) = 4"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedPipelineTokenCarrierHasFourNBound",
    ): (
        "\\A leaderContext \\in ContextRecords: ModelConfiguration => "
        "/\\ IsFiniteSet( AdequateLeaderFixedPipelineTokenCarrier("
        "leaderContext)) /\\ Cardinality( "
        "AdequateLeaderFixedPipelineTokenCarrier(leaderContext)) <= 4 * N"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedPipelineRemainingTokensHaveFourNBound",
    ): (
        "\\A leaderContext \\in ContextRecords, leader \\in ValidatorIds, "
        "leaderView \\in Views, subject \\in Subjects: ModelConfiguration => "
        "/\\ IsFiniteSet( AdequateLeaderFixedPipelineRemainingTokens( "
        "leaderContext, leader, leaderView, subject)) "
        "/\\ AdequateLeaderFixedPipelineWindowsRemaining( leaderContext, "
        "leader, leaderView, subject) \\in Nat "
        "/\\ AdequateLeaderFixedPipelineWindowsRemaining( leaderContext, "
        "leader, leaderView, subject) <= 4 * N"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCandidatePhysicalWindowFitsConfiguredBudget",
    ): (
        "AsyncConfiguration => "
        "AdequateLeaderFixedCandidatePhysicalWindowBudget "
        "<= AsyncCandidatePhysicalServiceBudget"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedProducerAndPhysicalWindowFitConfiguredBudget",
    ): (
        "AsyncConfiguration => "
        "AsyncCandidateProducerActionEpisodeBudget "
        "+ AdequateLeaderFixedCandidatePhysicalWindowBudget "
        "<= AsyncCandidatePhysicalServiceBudget"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCrossChildPhysicalDebtFitsProducerEpisode",
    ): (
        "\\A node \\in ValidatorIds, cutoffOrdinal \\in Nat: "
        "AsyncStrongTypeInvariant => "
        "/\\ IsFiniteSet( "
        "AsyncCausalEpisodeExactCandidateOccurrenceTokens( "
        "node, cutoffOrdinal)) "
        "/\\ AdequateLeaderFixedCrossChildPhysicalDebt( "
        "node, cutoffOrdinal) "
        "<= AsyncCandidateProducerEpisodeBudget"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCrossChildSuccessorBatchStrictlyConsumes",
    ): (
        "\\A parent \\in AsyncCandidateSet: "
        "AdequateLeaderFixedCrossChildSuccessorBatchConsumes(parent)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCommandSuccessorsRetainNodeAndOrigin",
    ): (
        "\\A parent \\in AsyncCandidateSet: "
        "\\A child \\in SequenceSet(CommandSuccessors(parent)): "
        "/\\ child.node = parent.node "
        "/\\ child.causalOrigin = parent.causalOrigin"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedExactParentDepartureCarriesLifecycleCut",
    ): (
        "\\A parent \\in AsyncCandidateSet: "
        "/\\ gst "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateLifecycleSchedulerCoverageInvariant "
        "/\\ AdequateLeaderFixedPipelineExactParentDeparture(parent) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ AsyncCandidateLifecycleSchedulerCoverageInvariant' "
        "=> AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep(parent)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedOwnedParentDepartureConsumesCrossChildDebt",
    ): (
        "\\A parent \\in AsyncCandidateSet: "
        "LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(parent) "
        "origin == parent.causalOrigin IN "
        "/\\ gst "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AdequateLeaderFixedPipelineExactParentDeparture(parent) "
        "/\\ cutoffOrdinal \\in Nat \\ {0} "
        "/\\ parent \\in AsyncCausalEpisodeCandidates( "
        "parent.node, cutoffOrdinal) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep(parent) "
        "=> (AdequateLeaderFixedCrossChildPhysicalDebt( "
        "parent.node, cutoffOrdinal))' "
        "< AdequateLeaderFixedCrossChildPhysicalDebt( "
        "parent.node, cutoffOrdinal)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderScheduledCandidatePositionHasCapacityBound",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ CandidateScheduled(candidate) => "
        "LET rank == CandidateServiceRank(candidate) IN "
        "/\\ rank[1] \\in 2..6 /\\ rank[2] \\in Nat "
        "/\\ CASE rank[1] = 2 -> rank[2] <= "
        "AdequateLeaderFixedDeferredPositionCeiling "
        "[] rank[1] = 3 -> rank[2] <= "
        "AdequateLeaderFixedRuntimePositionCeiling "
        "[] rank[1] = 4 -> rank[2] <= "
        "AdequateLeaderFixedReadyPositionCeiling "
        "[] rank[1] = 5 -> rank[2] <= "
        "AdequateLeaderFixedIoPositionCeiling "
        "[] rank[1] = 6 -> rank[2] <= "
        "AdequateLeaderFixedCausalPositionCeiling [] OTHER -> FALSE"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderScheduledCandidatePhysicalRankIsBounded",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ CandidateScheduled(candidate) => "
        "/\\ AdequateLeaderFixedCandidatePhysicalRank(candidate) "
        "\\in Nat \\ {0} "
        "/\\ AdequateLeaderFixedCandidatePhysicalRank(candidate) "
        "<= AdequateLeaderFixedCandidatePhysicalWindowBudget"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderStrictServiceRankDescentLowersPhysicalRank",
    ): (
        "\\A beforeRank, afterRank \\in OwnedServiceRankCarrier: "
        "/\\ beforeRank[1] \\in 2..6 /\\ afterRank[1] \\in 2..6 "
        "/\\ CASE beforeRank[1] = 2 -> beforeRank[2] <= "
        "AdequateLeaderFixedDeferredPositionCeiling "
        "[] beforeRank[1] = 3 -> beforeRank[2] <= "
        "AdequateLeaderFixedRuntimePositionCeiling "
        "[] beforeRank[1] = 4 -> beforeRank[2] <= "
        "AdequateLeaderFixedReadyPositionCeiling "
        "[] beforeRank[1] = 5 -> beforeRank[2] <= "
        "AdequateLeaderFixedIoPositionCeiling "
        "[] beforeRank[1] = 6 -> beforeRank[2] <= "
        "AdequateLeaderFixedCausalPositionCeiling [] OTHER -> FALSE "
        "/\\ CASE afterRank[1] = 2 -> afterRank[2] <= "
        "AdequateLeaderFixedDeferredPositionCeiling "
        "[] afterRank[1] = 3 -> afterRank[2] <= "
        "AdequateLeaderFixedRuntimePositionCeiling "
        "[] afterRank[1] = 4 -> afterRank[2] <= "
        "AdequateLeaderFixedReadyPositionCeiling "
        "[] afterRank[1] = 5 -> afterRank[2] <= "
        "AdequateLeaderFixedIoPositionCeiling "
        "[] afterRank[1] = 6 -> afterRank[2] <= "
        "AdequateLeaderFixedCausalPositionCeiling [] OTHER -> FALSE "
        "/\\ ServiceRankLess(afterRank, beforeRank) => "
        "AdequateLeaderFixedCandidatePhysicalRankFrom(afterRank) < "
        "AdequateLeaderFixedCandidatePhysicalRankFrom(beforeRank)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedExactCandidateActionCreditIsBounded",
    ): (
        "\\A commandClass \\in AsyncCommandClasses, "
        "kind \\in AsyncWorkKinds: "
        "AdequateLeaderFixedExactCandidateActionCredit(commandClass, kind) "
        "\\in 2..72"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedSuccessorBatchFitsReservedActionTail",
    ): (
        "\\A command \\in AsyncCandidateSet: "
        "AdequateLeaderFixedCommandSuccessorBatchActionCredit(command) "
        "<= AdequateLeaderFixedCandidateSuccessorTailActionCredit("
        "command.kind)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedSuccessorBatchStrictlyConsumesActionCredit",
    ): (
        "\\A command \\in AsyncCandidateSet: "
        "AdequateLeaderFixedCommandSuccessorBatchActionCredit(command) "
        "< AdequateLeaderFixedExactCandidateActionCredit( "
        "command.class, command.kind)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedScheduledCandidateRemainingActionCreditIsBounded",
    ): (
        "\\A candidate \\in AsyncCandidateSet: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ CandidateScheduled(candidate) => "
        "AdequateLeaderFixedCandidateRemainingActionCredit(candidate) "
        "\\in 1..72"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCutCumulativeActionDebtFitsEpisodeBudget",
    ): (
        "\\A node \\in ValidatorIds, cutoffOrdinal \\in Nat: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant => "
        "/\\ IsFiniteSet( "
        "AdequateLeaderFixedCutCumulativeActionTokens( "
        "node, cutoffOrdinal)) "
        "/\\ AdequateLeaderFixedCutCumulativeActionDebt( "
        "node, cutoffOrdinal) \\in Nat "
        "/\\ AdequateLeaderFixedCutCumulativeActionDebt( "
        "node, cutoffOrdinal) "
        "<= AsyncCandidateProducerActionEpisodeBudget"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCutCumulativeActionDebtFitsPhysicalBudget",
    ): (
        "\\A node \\in ValidatorIds, cutoffOrdinal \\in Nat: "
        "/\\ AsyncConfiguration "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant => "
        "/\\ AdequateLeaderFixedCutCumulativeActionDebt( "
        "node, cutoffOrdinal) \\in Nat "
        "/\\ AdequateLeaderFixedCutCumulativeActionDebt( "
        "node, cutoffOrdinal) "
        "<= AsyncCandidatePhysicalServiceBudget"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedOwnedFinalRouteParentConsumesCumulativeDebt",
    ): (
        "\\A parent \\in AsyncCandidateSet: "
        "LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(parent) "
        "origin == parent.causalOrigin IN "
        "/\\ gst "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AsyncCandidateLifecycleSchedulerCoverageInvariant "
        "/\\ AdequateLeaderFixedFinalRouteParentDeparture(parent) "
        "/\\ cutoffOrdinal \\in Nat \\ {0} "
        "/\\ parent \\in AsyncCausalEpisodeCandidates( "
        "parent.node, cutoffOrdinal) "
        "/\\ [AsyncNext]_AsyncAllVars "
        "/\\ AsyncCandidateLifecycleSchedulerCoverageInvariant' => "
        "/\\ AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep(parent) "
        "/\\ (AdequateLeaderFixedCutCumulativeActionDebt( "
        "parent.node, cutoffOrdinal))' "
        "< AdequateLeaderFixedCutCumulativeActionDebt( "
        "parent.node, cutoffOrdinal)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedIntermediateRouteStageCannotRecharge",
    ): (
        "\\A commandClass \\in AsyncCommandClasses, "
        "beforeStage, afterStage \\in 2..6: "
        "AdequateLeaderFixedIntermediateRouteStageMove( "
        "commandClass, beforeStage, afterStage) => "
        "AdequateLeaderFixedRouteActionCreditFromStage( "
        "commandClass, afterStage) "
        "<= AdequateLeaderFixedRouteActionCreditFromStage( "
        "commandClass, beforeStage)"
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedIntermediateRouteCarrierCannotRechargeCut",
    ): (
        "\\A candidate \\in AsyncCandidateSet, "
        "node \\in ValidatorIds, cutoffOrdinal \\in Nat: "
        "/\\ AsyncStrongTypeInvariant "
        "/\\ AsyncProgressOwnershipInvariant "
        "/\\ AdequateLeaderFixedIntermediateRouteCarrierMove( "
        "candidate, node, cutoffOrdinal) => "
        "/\\ AdequateLeaderFixedCutCumulativeActionTokens( "
        "node, cutoffOrdinal)' \\subseteq "
        "AdequateLeaderFixedCutCumulativeActionTokens( "
        "node, cutoffOrdinal) "
        "/\\ AdequateLeaderFixedCutCumulativeActionDebt( "
        "node, cutoffOrdinal)' "
        "<= AdequateLeaderFixedCutCumulativeActionDebt( "
        "node, cutoffOrdinal)"
    ),
}

QUANTITATIVE_FIXED_CORRIDOR_PROOF_DEPENDENCIES = {
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalExactRemainingOccurrenceBudgetIsBounded",
    ): (
        "AsyncCausalExactRemainingOccurrenceBudget",
        "AsyncWorkKinds",
        "AsyncCompletionTags",
        "AsyncDeliveryKinds",
        "AsyncReducerKinds",
        "SMT",
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCommandExactSuccessorBatchStrictlyConsumesOccurrenceBudget",
    ): (
        "CommandSuccessorsHaveBoundedLength",
        "AsyncCommandExactSuccessorBatchOccurrenceBudget",
        "AsyncCausalExactRemainingOccurrenceBudget",
        "CommandSuccessors",
        "SMTT",
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeCandidateCarrierHasConfiguredBound",
    ): (
        "AsyncCausalEpisodeCandidates",
        "AsyncCausalEpisodeFrozenPredecessorOrigins",
        "AsyncCandidateProducerEpisodeCapacity",
        "QueuedCandidates",
        "DeferredCandidates",
        "CausalCandidates",
        "TrackedWorkCandidates",
        "FS_Subset",
        "FS_CardinalityType",
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeExactOccurrenceBudgetFitsConfiguredEpisode",
    ): (
        "AsyncCausalEpisodeCandidateCarrierHasConfiguredBound",
        "AsyncCausalExactRemainingOccurrenceBudgetIsBounded",
        "AsyncCausalEpisodeExactCandidateOccurrenceBudget",
        "AsyncCausalEpisodeExactCandidateOccurrenceTokens",
        "AsyncCandidateProducerEpisodeBudget",
        "FS_Product",
        "FS_Subset",
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeServicedCandidateConsumesExactOccurrenceBudget",
    ): (
        "AsyncCausalEpisodeOwnedCutServiceConsumesExactOccurrenceBudget",
        "AsyncCausalEpisodeTargetLifecycleOrdinalPersists",
        "AsyncCausalEpisodeLifecycleCutOwned",
        "ProtectedCandidateOwned",
        "AsyncCausalEpisodeExactCandidateOccurrenceBudget",
        "AsyncCausalEpisodeExactCandidateOccurrenceTokens",
        "AsyncCausalEpisodeCandidates",
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeSameOriginHandoffRetainsLifecycleCut",
    ): (
        "AsyncNextNeverSchedulesAnUnownedCandidateLifecycle",
        "AsyncSharedSchedulerHighWatermarkIsMonotone",
        "AsyncCausalEpisodeLifecycleCutOwned",
        "AsyncCandidateLifecycleSchedulerCoverageInvariant",
        "AsyncCandidateLifecycleRecordCoversScheduledOrigin",
        "AsyncCandidateLifecycleOrdinal",
        "AsyncCandidateLifecycleCarrierUpdatedAdmissions",
        "AsyncCandidateLifecycleNewAdmissions",
        "AsyncAllVars",
        "IsaT",
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeOwnedLifecycleCutCannotReplenish",
    ): (
        # The physical-cut refinement supersedes the narrower GST departure
        # lemma: strong-type preservation plus both monotone high-watermarks
        # prevent any pre-cut origin from being re-admitted.
        "AsyncNextNeverSchedulesAnUnownedCandidateLifecycle",
        "AsyncBracketNextPreservesStrongTypeInvariant",
        "AsyncSharedSchedulerHighWatermarkIsMonotone",
        "AsyncIngressPhysicalHighWatermarkIsMonotone",
        "AsyncCausalEpisodeLifecycleCutOwned",
        "AsyncCausalEpisodeFrozenPredecessorOrigins",
        "AsyncCausalEpisodeTargetPhysicalCut",
        "AsyncAllVars",
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeOwnedLifecycleServeCutCannotReplenish",
    ): (
        "AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal",
        "AsyncServeIngressAdmissionConsumesSharedSchedulerOrdinal",
        "AsyncSharedSchedulerHighWatermarkIsMonotone",
        "AsyncCausalEpisodeLifecycleCutOwned",
        "AsyncCausalEpisodeServeIngressIdentities",
        "AsyncFreshServeIngressAdmissionsForNodeThisStep",
        "AsyncAllVars",
    ),
    (
        "SumeragiV2AsyncCausalWorkBudgetProofs",
        "AsyncCausalEpisodeOwnedCutServiceConsumesExactOccurrenceBudget",
    ): (
        "AsyncCausalEpisodeOwnedLifecycleCutCannotReplenish",
        "AsyncCommandExactSuccessorBatchStrictlyConsumesOccurrenceBudget",
        "AsyncNextNeverSchedulesAnUnownedCandidateLifecycle",
        "AsyncCausalEpisodeLifecycleCutOwned",
        "AsyncCausalEpisodeExactCandidateOccurrenceBudget",
        "AsyncCausalEpisodeExactCandidateOccurrenceTokens",
        "AsyncCausalEpisodeCandidates",
        "FS_Subset",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedPipelinePhasesHaveCardinalityFour",
    ): ("FS_EmptySet", "FS_AddElement", "SMT"),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedPipelineTokenCarrierHasFourNBound",
    ): (
        "AdequateLeaderFrozenResponsiveRosterHasConfiguredBound",
        "AdequateLeaderFixedPipelinePhasesHaveCardinalityFour",
        "FS_Product",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedPipelineRemainingTokensHaveFourNBound",
    ): (
        "AdequateLeaderFixedPipelineTokenCarrierHasFourNBound",
        "FS_Subset",
        "FS_CardinalityType",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCandidatePhysicalWindowFitsConfiguredBudget",
    ): (
        "AdequateLeaderFixedCandidatePhysicalWindowBudget",
        "AsyncCandidatePhysicalServiceBudget",
        "AsyncCandidateProducerActionEpisodeBudget",
        "AsyncCandidateProducerEpisodeBudget",
        "AsyncConfiguration",
        "SMT",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedProducerAndPhysicalWindowFitConfiguredBudget",
    ): (
        "AdequateLeaderFixedCandidatePhysicalWindowBudget",
        "AsyncCandidatePhysicalServiceBudget",
        "AsyncCandidateProducerActionEpisodeBudget",
        "AsyncCandidateProducerEpisodeCapacity",
        "AsyncConfiguration",
        "SMT",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCrossChildPhysicalDebtFitsProducerEpisode",
    ): (
        "AsyncCausalEpisodeExactOccurrenceBudgetFitsConfiguredEpisode",
        "AdequateLeaderFixedCrossChildPhysicalDebt",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCrossChildSuccessorBatchStrictlyConsumes",
    ): (
        "AsyncCommandExactSuccessorBatchStrictlyConsumesOccurrenceBudget",
        "AdequateLeaderFixedCrossChildSuccessorBatchConsumes",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCommandSuccessorsRetainNodeAndOrigin",
    ): (
        "CommandSuccessorsRetainCausalOrigin",
        "CommandSuccessors",
        "AsyncCandidateFrom",
        "AsyncCandidateWithIdentityAndOrigin",
        "SequenceSet",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedExactParentDepartureCarriesLifecycleCut",
    ): (
        "AdequateLeaderFixedCommandSuccessorsRetainNodeAndOrigin",
        "AsyncCausalEpisodeSameOriginHandoffRetainsLifecycleCut",
        "AdequateLeaderFixedPipelineExactParentDeparture",
        "AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep",
        "IsaT",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedOwnedParentDepartureConsumesCrossChildDebt",
    ): (
        "AsyncCausalEpisodeOwnedCutServiceConsumesExactOccurrenceBudget",
        "AdequateLeaderFixedPipelineExactParentDeparture",
        "AdequateLeaderFixedCrossChildLifecycleCutCarryThisStep",
        "AdequateLeaderFixedCrossChildPhysicalDebt",
        "AsyncCandidateSet",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderScheduledCandidatePositionHasCapacityBound",
    ): (
        "ScheduledCandidateServiceRankInCarrier",
        "SchedulerClassPrefixRankBound",
        "AdequateLeaderFixedDeferredPositionCeiling",
        "AdequateLeaderFixedRuntimePositionCeiling",
        "AdequateLeaderFixedReadyPositionCeiling",
        "AdequateLeaderFixedIoPositionCeiling",
        "AdequateLeaderFixedCausalPositionCeiling",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderScheduledCandidatePhysicalRankIsBounded",
    ): (
        "AdequateLeaderScheduledCandidatePositionHasCapacityBound",
        "AdequateLeaderFixedCandidatePhysicalRankFrom",
        "AdequateLeaderFixedCandidatePhysicalWindowBudget",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderStrictServiceRankDescentLowersPhysicalRank",
    ): (
        "AdequateLeaderFixedCandidatePhysicalRankFrom",
        "ServiceRankLess",
        "SMT",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedExactCandidateActionCreditIsBounded",
    ): (
        "AdequateLeaderFixedExactCandidateActionCredit",
        "AdequateLeaderFixedInitialCandidateRouteActionCredit",
        "AdequateLeaderFixedCandidateSuccessorTailActionCredit",
        "AsyncCommandClasses",
        "AsyncWorkKinds",
        "SMT",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedSuccessorBatchFitsReservedActionTail",
    ): (
        "CommandSuccessorsHaveBoundedLength",
        "AdequateLeaderFixedCommandSuccessorBatchActionCredit",
        "AdequateLeaderFixedExactCandidateActionCredit",
        "AdequateLeaderFixedInitialCandidateRouteActionCredit",
        "AdequateLeaderFixedCandidateSuccessorTailActionCredit",
        "CommandSuccessors",
        "InstallCommandSuccessors",
        "PersistDecisionRecoverySuccessor",
        "SMTT",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedSuccessorBatchStrictlyConsumesActionCredit",
    ): (
        "AdequateLeaderFixedSuccessorBatchFitsReservedActionTail",
        "AdequateLeaderFixedExactCandidateActionCredit",
        "AdequateLeaderFixedInitialCandidateRouteActionCredit",
        "AsyncCandidateTyped",
        "SMT",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedScheduledCandidateRemainingActionCreditIsBounded",
    ): (
        "AdequateLeaderScheduledCandidatePositionHasCapacityBound",
        "AdequateLeaderFixedCandidateRemainingActionCredit",
        "AdequateLeaderFixedCandidateRemainingRouteActionCredit",
        "AdequateLeaderFixedRouteActionCreditFromStage",
        "AdequateLeaderFixedInitialCandidateRouteActionCredit",
        "AdequateLeaderFixedCandidateSuccessorTailActionCredit",
        "AsyncStrongTypeInvariant",
        "AsyncSchedulerTypeInvariant",
        "SMT",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCutCumulativeActionDebtFitsEpisodeBudget",
    ): (
        "AsyncCausalEpisodeCandidateCarrierHasConfiguredBound",
        "AdequateLeaderFixedScheduledCandidateRemainingActionCreditIsBounded",
        "AdequateLeaderFixedCutCumulativeActionDebt",
        "AdequateLeaderFixedCutCumulativeActionTokens",
        "AdequateLeaderFixedCandidateRemainingActionCredit",
        "AsyncCandidateProducerActionEpisodeBudget",
        "FS_Product",
        "FS_Subset",
        "FS_CardinalityType",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedCutCumulativeActionDebtFitsPhysicalBudget",
    ): (
        "AdequateLeaderFixedCutCumulativeActionDebtFitsEpisodeBudget",
        "AsyncCandidatePhysicalServiceBudget",
        "AsyncCandidateProducerActionEpisodeBudget",
        "AsyncConfiguration",
        "SMT",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedOwnedFinalRouteParentConsumesCumulativeDebt",
    ): (
        "AdequateLeaderFixedExactParentDepartureCarriesLifecycleCut",
        "AdequateLeaderFixedSuccessorBatchFitsReservedActionTail",
        "AsyncCausalEpisodeOwnedLifecycleCutCannotReplenish",
        "AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst",
        "AsyncNextNeverSchedulesAnUnownedCandidateLifecycle",
        "AdequateLeaderFixedFinalRouteParentDeparture",
        "AdequateLeaderFixedCutCumulativeActionDebt",
        "AdequateLeaderFixedCommandSuccessorBatchActionCredit",
        "AsyncCausalEpisodeCandidates",
        "FS_CardinalityType",
        "FS_Subset",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedIntermediateRouteStageCannotRecharge",
    ): (
        "AdequateLeaderFixedIntermediateRouteStageMove",
        "AdequateLeaderFixedRouteActionCreditFromStage",
        "AdequateLeaderFixedInitialCandidateRouteActionCredit",
        "AsyncCommandClasses",
        "SMT",
    ),
    (
        "SumeragiV2AdequateLeaderFixedCorridorClockProofs",
        "AdequateLeaderFixedIntermediateRouteCarrierCannotRechargeCut",
    ): (
        "AsyncCausalEpisodeCandidateCarrierHasConfiguredBound",
        "AdequateLeaderFixedIntermediateRouteStageCannotRecharge",
        "AdequateLeaderFixedIntermediateRouteCarrierMove",
        "AdequateLeaderFixedCutCumulativeActionDebt",
        "AdequateLeaderFixedCutCumulativeActionTokens",
        "AdequateLeaderFixedCandidateRemainingActionCredit",
        "AdequateLeaderFixedCandidateRemainingRouteActionCredit",
        "AdequateLeaderFixedRouteActionCreditFromStage",
        "FS_Product",
        "FS_Subset",
        "FS_CardinalityType",
    ),
}

# The reviewed `CommandSuccessors` body is bound in full, not sampled at one
# counterexample.  The batch inventory below is then evaluated as an exact
# recurrence: each child pays its class-specific physical route and inherits
# only its already-reserved successor tail.  This prevents a future branch
# from silently restoring a per-child physical window while leaving the
# headline maximum unchanged.
QUANTITATIVE_FIXED_CORRIDOR_COMMAND_SUCCESSORS_SHA256 = (
    "eeefd8c630c2d8a3594cfb67ccc7fe48103e51c3dd419b6e4fdfdb17c3f3cc04"
)

QUANTITATIVE_FIXED_CORRIDOR_ROUTE_ACTION_CREDITS = {
    "Normal": 2,
    "Progress": 2,
    "Completion": 4,
}

QUANTITATIVE_FIXED_CORRIDOR_SUCCESSOR_BATCHES = {
    "AssembleBody": (
        (("Completion", "Apply"),),
        (("Completion", "BeginProposal"),),
    ),
    "BeginProposal": ((("Completion", "PersistProposal"),),),
    "PersistProposal": ((("Completion", "SignProposal"),),),
    "DeliverProposal": (
        (
            ("Completion", "RebindRetainedBody"),
            ("Normal", "BeginPrepare"),
        ),
    ),
    "DeliverChunk": ((("Completion", "FetchBody"),),),
    "FetchBody": (
        (),
        (("Completion", "ValidateBody"),),
        (("Completion", "StoreBody"),),
    ),
    "RebindRetainedBody": ((("Completion", "StoreBody"),),),
    "FetchCertifiedBody": ((("Completion", "StoreBody"),),),
    "StoreBody": ((("Completion", "ValidateBody"),),),
    "ValidateBody": (
        (
            ("Normal", "BeginPrepare"),
            ("Completion", "BeginLockCommit"),
            ("Completion", "Apply"),
        ),
    ),
    "BeginPrepare": ((("Completion", "PersistPrepare"),),),
    "PersistPrepare": ((("Completion", "SignVote"),),),
    "DeliverVote": (
        (("Progress", "FormPrepareQC"),),
        (("Progress", "FormCommitQC"),),
    ),
    "DeliverQC": (
        (),
        (
            ("Progress", "BeginObservePrepare"),
            ("Completion", "BeginLockCommit"),
        ),
        (("Progress", "BeginDecision"),),
    ),
    "BeginObservePrepare": (
        (("Completion", "PersistObservePrepare"),),
    ),
    "PersistObservePrepare": ((("Completion", "BeginLockCommit"),),),
    "BeginLockCommit": ((("Completion", "PersistLockCommit"),),),
    "PersistLockCommit": ((("Completion", "SignVote"),),),
    "FormCommitQC": ((("Completion", "PersistDecision"),),),
    "BeginDecision": ((("Completion", "PersistDecision"),),),
    "PersistDecision": (
        (),
        (("Completion", "FetchBody"),),
        (("Completion", "StoreBody"),),
        (("Completion", "ValidateBody"),),
        (("Completion", "Apply"),),
    ),
    "BeginTimeout": ((("Completion", "PersistTimeout"),),),
    "PersistTimeout": ((("Completion", "SignTimeout"),),),
    "SignTimeout": (
        (),
        (("Completion", "PersistInstallTC"),),
    ),
    "DeliverTimeout": (
        (),
        (("Completion", "PersistInstallTC"),),
    ),
    "DeliverTC": (
        (),
        (("Progress", "BeginInstallTC"),),
    ),
    "BeginInstallTC": ((("Completion", "PersistInstallTC"),),),
    "PersistInstallTC": (
        (("Normal", "AssembleBody"),),
        (
            ("Completion", "FetchBody"),
            ("Normal", "AssembleBody"),
        ),
        (
            ("Completion", "SignVote"),
            ("Normal", "AssembleBody"),
        ),
        (
            ("Completion", "FetchBody"),
            ("Completion", "SignVote"),
            ("Normal", "AssembleBody"),
        ),
    ),
}

QUANTITATIVE_FIXED_CORRIDOR_SUCCESSOR_TAIL_ACTION_CREDITS = {
    "BeginTimeout": 68,
    "PersistTimeout": 64,
    "DeliverTC": 62,
    "SignTimeout": 60,
    "DeliverTimeout": 60,
    "BeginInstallTC": 60,
    "PersistInstallTC": 56,
    "DeliverProposal": 48,
    "DeliverVote": 44,
    "DeliverQC": 44,
    "FormCommitQC": 42,
    "BeginDecision": 42,
    "DeliverChunk": 38,
    "PersistDecision": 38,
    "FetchBody": 34,
    "RebindRetainedBody": 34,
    "FetchCertifiedBody": 34,
    "StoreBody": 30,
    "ValidateBody": 26,
    "BeginObservePrepare": 16,
    "AssembleBody": 12,
    "PersistObservePrepare": 12,
    "BeginProposal": 8,
    "BeginPrepare": 8,
    "BeginLockCommit": 8,
    "PersistProposal": 4,
    "PersistPrepare": 4,
    "PersistLockCommit": 4,
}

HISTORICAL_LOCAL_AUTHORITY_REQUIRED_OPERATORS = (
    "IndexedHistoricalRecoveryArchiveOwner",
    "IndexedHistoricalRecoveryArchiveOwnerJoined",
    "IndexedHistoricalRecoveryTypedArchiveAuthority",
    "IndexedResponsiveActiveRosterAt",
    "IndexedLocalAdequateLeaderSemanticKernelProperty",
    "IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty",
    "IndexedLocalAdequateLeaderFreshSelfLeaderDecisionProperty",
    "IndexedLocalAdequateLeaderTargetProofInvariantsProperty",
    "IndexedLocalAdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty",
    "IndexedLocalAdequateLeaderProducerTransportClosureProperty",
    "IndexedLocalAdequateLeaderProducerTransportOccurrenceClosureProperty",
    (
        "IndexedLocalAdequateLeaderRetainedProducer"
        "NonDescentEpisodeStepProperty"
    ),
    "IndexedLocalAdequateLeaderRetainedProducerOccurrenceClosureProperty",
    "IndexedLocalAdequateLeaderProductiveEpisodeRankStepProperty",
    "IndexedAdequateLeaderLocalFairBehaviorAt",
    "IndexedLocalAdequateLeaderDecisionConvergenceProperty",
    "IndexedLocalExactDecisionStageServiceProperty",
)

# Every local historical authority alias is a proof boundary, not merely a
# name-presence contract.  In particular, weakening any one of the semantic
# kernel, Decision convergence, exact Decision service, or fresh-exposure
# aliases to TRUE would make the indexed recovery composition tautological
# while leaving every consuming theorem text unchanged.
HISTORICAL_LOCAL_AUTHORITY_EXACT_OPERATOR_BODIES = {
    "IndexedHistoricalRecoveryArchiveOwner": (
        "CHOOSE server \\in "
        "IndexedAsync(initialContext)!AsyncVotersAt(initialContext): TRUE"
    ),
    "IndexedHistoricalRecoveryArchiveOwnerJoined": (
        "IndexedHistoricalRecoveryArchiveOwner(initialContext) "
        "\\in joinedByContext[initialContext]"
    ),
    "IndexedHistoricalRecoveryTypedArchiveAuthority": (
        "/\\ IndexedCore(initialContext, 7) "
        "/\\ \\E server \\in ValidatorIds, "
        "source \\in Chain!DecisionEvidenceSet: "
        "IndexedHistoricalRecoverySourceReady( "
        "initialContext, server, source)"
    ),
    "IndexedResponsiveActiveRosterAt": (
        "Responsive \\subseteq "
        "IndexedAsync(initialContext)!AsyncActiveServiceNodes"
    ),
    "IndexedLocalAdequateLeaderSemanticKernelProperty": (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "AdequateLeaderLocalSemanticKernelProperty(IndexedChainSpec)"
    ),
    "IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty": (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "AdequateLeaderLocalFreshSelfCorridorExposureProperty( "
        "IndexedChainSpec)"
    ),
    "IndexedLocalAdequateLeaderFreshSelfLeaderDecisionProperty": (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "AdequateLeaderFreshSelfLeaderDecisionProperty(IndexedChainSpec)"
    ),
    "IndexedLocalAdequateLeaderTargetProofInvariantsProperty": (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "AdequateLeaderTargetProofInvariantsProperty(IndexedChainSpec)"
    ),
    "IndexedLocalAdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty": (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty( "
        "IndexedChainSpec)"
    ),
    "IndexedLocalAdequateLeaderProducerTransportClosureProperty": (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "AdequateLeaderTargetProducerTransportClosureProperty( "
        "IndexedChainSpec)"
    ),
    "IndexedLocalAdequateLeaderProducerTransportOccurrenceClosureProperty": (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "AdequateLeaderTargetProducerTransportOccurrenceClosureProperty( "
        "IndexedChainSpec)"
    ),
    (
        "IndexedLocalAdequateLeaderRetainedProducer"
        "NonDescentEpisodeStepProperty"
    ): (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "AdequateLeaderRetainedProducerNonDescentEpisodeStepProperty( "
        "IndexedChainSpec)"
    ),
    "IndexedLocalAdequateLeaderRetainedProducerOccurrenceClosureProperty": (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "AdequateLeaderTargetRetainedProducerOccurrenceClosureProperty( "
        "IndexedChainSpec)"
    ),
    "IndexedLocalAdequateLeaderProductiveEpisodeRankStepProperty": (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "AdequateLeaderTargetProductiveEpisodeRankStepProperty( "
        "IndexedChainSpec)"
    ),
    "IndexedAdequateLeaderLocalFairBehaviorAt": (
        "/\\ [][IndexedAdequateLeaderWitness(initialContext)!AsyncNext]_( "
        "IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars) "
        "/\\ WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)( "
        "IndexedAdequateLeaderWitness(initialContext)!gst "
        "/\\ IndexedAdequateLeaderWitness(initialContext)!AsyncTick) "
        "/\\ \\A node \\in IndexedAdequateLeaderWitness(initialContext)! "
        "AsyncVotersAt(initialContext): "
        "/\\ WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)( "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "PostGstRunNode(node)) "
        "/\\ WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)( "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "PostGstResolveLocalCandidateProducerContinuation(node)) "
        "/\\ WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)( "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "PostGstServiceConditionalTransportProducerContinuation(node)) "
        "/\\ WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)( "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "PostGstServiceVolatileBodyProducerContinuation(node)) "
        "/\\ \\A node \\in Responsive: "
        "WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)( "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "PostGstServiceIoWorker(node)) "
        "/\\ \\A recipient \\in Responsive, "
        "source \\in IndexedAdequateLeaderWitness(initialContext)! "
        "AsyncIngressSources: "
        "WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)( "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "PostGstAdmitHiddenPacket(recipient, source)) "
        "/\\ \\A slot \\in IndexedAdequateLeaderWitness(initialContext)! "
        "AsyncLeaderWireLifecycleSlotSet: "
        "WF_(IndexedAdequateLeaderWitness(initialContext)!AsyncAllVars)( "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "PostGstRetireLeaderWireLifecycleSlot(slot))"
    ),
    "IndexedLocalAdequateLeaderDecisionConvergenceProperty": (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedAdequateLeaderWitness(initialContext)! "
        "AdequateLeaderLocalTargetDecisionConvergenceProperty( "
        "IndexedChainSpec)"
    ),
    "IndexedLocalExactDecisionStageServiceProperty": (
        "\\A initialContext \\in AdmissibleContextRecords: "
        "IndexedDecisionServiceWitness(initialContext)! "
        "ExactDecisionStageServiceProperty(IndexedChainSpec)"
    ),
}

HISTORICAL_LOCAL_AUTHORITY_REQUIRED_PROOF_TOKENS = {
    "IndexedChainSpecResponsiveActiveRosterEventuallySetsExactGst": (
        "IndexedResponsiveActiveRosterAt",
        "IndexedFairActionsRemainEnabledInProduct",
        "IndexedFairProductStepsProjectExactOccurrences",
    ),
    "IndexedJoinedResponsiveActiveRosterIsStable": (
        "IndexedStepPreservesCompositionInvariant",
        "JoinedMembershipIsMonotone",
        "IndexedServiceActivationMembershipCoherenceAt",
    ),
    "IndexedLiveChainSpecProvidesLocalAdequateLeaderProofInvariants": (
        "IndexedLiveChainSpecProjectsIndexedChainSpec",
        "IndexedChainSpecEstablishesCompositionInvariant",
        "IndexedAdequateLeaderWitnessVariablesAreExact",
        "AdequateLeaderTargetProofInvariantsProperty",
    ),
    "IndexedLiveChainSpecProvidesAdequateLeaderLocalFairBehavior": (
        "IndexedBracketStepProjectsEveryAdequateLeaderWitnessStep",
        "IndexedPostGstTickFairnessTransfersLocally",
        "IndexedPostGstRunNodeFairnessTransfersLocally",
        "IndexedAdequateLeaderNonRunnerFairnessTransfersLocally",
        "IndexedHistoricalNonPacketOwnerFairnessTransfersLocally",
    ),
    "IndexedAsyncLiveSpecProjectsAdequateLeaderWitnessLiveSpec": (
        "IndexedAsync!AsyncLiveSpecAt",
        "IndexedAdequateLeaderWitness!AsyncLiveSpecAt",
        "IndexedCore",
        "IndexedScheduler",
        "IndexedRecovery",
    ),
    "IndexedAdequateLeaderLocalSourceJoinsResponsiveRoster": (
        "AdequateLeaderLocalTargetDecisionSource",
        "IndexedPostGstResponsiveActiveRosterCoherence",
        "IndexedPostGstActiveServiceOwnerHasJoinedProductInstance",
        "IndexedAllResponsiveJoined",
    ),
    "IndexedLiveChainSpecProvidesLocalAdequateLeaderFreshSelfCorridorExposure": (
        "IndexedLiveChainSpecProjectsIndexedChainSpec",
        "IndexedChainSpecEstablishesCompositionInvariant",
        "IndexedAdequateLeaderLocalSourceJoinsResponsiveRoster",
        "IndexedAllResponsiveJoinedIsStable",
        "IndexedLiveInstanceActivationObligation",
        "IndexedAsyncLiveSpecProjectsAdequateLeaderWitnessLiveSpec",
        "AsyncLiveProvidesLocalFreshSelfCorridorExposure",
        "PTL",
    ),
    "IndexedAdequateLeaderCompletedProvidersSupplyLocalSemanticKernel": (
        "IndexedLiveChainSpecProvidesLocalAdequateLeaderProofInvariants",
        "IndexedAdequateLeaderRetainedProducerStepAndOccurrenceClosureCompose",
        "IndexedLocalAdequateLeaderRetainedProducerOccurrenceClosureProperty",
        "AdequateLeaderCompletedLocalProviderKernelSuppliesSemanticKernel",
    ),
    "IndexedAdequateLeaderRetainedProducerStepAndOccurrenceClosureCompose": (
        "AdequateLeaderRetainedProducerStepAndOccurrenceClosureCompose",
        "IndexedLocalAdequateLeaderProducerTransportOccurrenceClosureProperty",
        (
            "IndexedLocalAdequateLeaderRetainedProducer"
            "NonDescentEpisodeStepProperty"
        ),
        "IndexedLocalAdequateLeaderRetainedProducerOccurrenceClosureProperty",
    ),
    "IndexedAdequateLeaderFixedDeadlineSourceJoinsResponsiveRoster": (
        "IndexedAdequateLeaderLocalSourceJoinsResponsiveRoster",
        "AdequateLeaderFixedCorridorDeadlineSource",
        "AdequateLeaderFreshSynchronizedTargetCorridor",
        "AdequateLeaderLocalTargetDecisionSource",
    ),
    (
        "IndexedLiveChainSpecProvidesLocalAdequateLeader"
        "FixedDeadlineAndResponsiveDissemination"
    ): (
        "IndexedLiveChainSpecProjectsIndexedChainSpec",
        "IndexedChainSpecEstablishesCompositionInvariant",
        "IndexedAdequateLeaderWitnessVariablesAreExact",
        "IndexedLiveChainSpecProvidesAdequateLeaderLocalFairBehavior",
        "IndexedLiveChainSpecProvidesLocalAdequateLeaderProofInvariants",
        "IndexedAdequateLeaderLocalSourceJoinsResponsiveRoster",
        "IndexedAdequateLeaderFixedDeadlineSourceJoinsResponsiveRoster",
        "IndexedAllResponsiveJoinedIsStable",
        "IndexedLiveInstanceActivationObligation",
        "IndexedAsyncLiveSpecProjectsAdequateLeaderWitnessLiveSpec",
        (
            "AsyncLiveSpecSuppliesAdequateLeaderFixedDeadline"
            "AndResponsiveDissemination"
        ),
        "PTL",
    ),
    (
        "IndexedAdequateLeaderFixedDeadlineDissemination"
        "AndExposureSupplyLocalConvergence"
    ): (
        "AdequateLeaderFixedDeadlineAndDisseminationSupplyLocalTargetConvergence",
        "IndexedLocalAdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty",
        "IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty",
        "IndexedLocalAdequateLeaderDecisionConvergenceProperty",
    ),
    "IndexedChainSpecClosesLocalExactDecisionOffSchedulerCorridor": (
        "ExactDecisionRequestClockOwnerConvergence",
        "ExactDecisionRequestRuntimePrefixConvergence",
        "ExactDecisionRequestHeadGateOwnerConvergence",
        "ExactDecisionRequestAdmissionCoalescingOutcomeIsDischarged",
        "ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergence",
    ),
    "IndexedChainSpecProvidesLocalExactDecisionStageService": (
        "IndexedChainSpecProvidesCurrentVoterFiniteRunnerEpisodeClosure",
        "IndexedChainSpecClosesLocalExactDecisionOffSchedulerCorridor",
        "ExactDecisionOffSchedulerResidualConvergenceDischargesStageService",
    ),
    "IndexedLocalAdequateLeaderSemanticKernelProvidesDecisionConvergence": (
        "AdequateLeaderLocalSemanticKernelSuppliesTargetDecisionConvergence",
    ),
    "IndexedLocalAppliedVoterSuppliesTypedArchiveAuthority": (
        "GstResponsiveNodesAreUp",
    ),
    "IndexedChainSpecJoinedArchiveOwnerProducesTypedAuthority": (
        "IndexedChainSpecResponsiveActiveRosterEventuallySetsExactGst",
        "IndexedLocalAdequateLeaderDecisionConvergenceProperty",
        "IndexedChainSpecProvidesLocalExactDecisionStageService",
        "PostGstResponsiveDecisionHasExactServiceSource",
        "IndexedLocalAppliedVoterSuppliesTypedArchiveAuthority",
    ),
    "IndexedHistoricalStrictAncestorRecoveryClosesActivationAt": (
        "IndexedJoinedTargetReachesEveryAncestorFromStrictRecovery",
        "IndexedReachedAncestorEventuallyJoinsResponsiveNode",
        "IndexedHistoricalRecoveryArchiveOwnerIsResponsive",
        "IndexedStrictAncestorRecoveryEventuallyActivatesResponsiveRoster",
        "IndexedJoinedResponsiveActiveRosterIsStable",
    ),
    "IndexedLiveStrictAncestorsCloseOrdinaryDecisionOwnerRanks": (
        "IndexedChainSpecClosesSuccessorActivationForHistoricalInduction",
        "IndexedStrictAncestorRecoveryEventuallyActivatesResponsiveRoster",
        "IndexedChainSpecResponsiveActiveRosterEventuallySetsExactGst",
        "IndexedHistoricalDecisionOrdinaryOwnerPersistsOrGoals",
        "IndexedHistoricalDecisionOrdinaryStageHasExactServiceSourceAtGst",
        "IndexedChainSpecProvidesLocalExactDecisionStageService",
    ),
    "IndexedHistoricalAuthorityProgressAtHeightFromStrictAncestors": (
        "IndexedHistoricalLowerAuthorityProgressGivesStrictAncestorAdvance",
        "IndexedHistoricalStrictAncestorRecoveryClosesActivationAt",
        "IndexedHistoricalRecoveryActivatedArchiveProducerResidualProperty",
        "IndexedHistoricalRecoveryTypedArchiveEntryResidualProperty",
    ),
    "IndexedHistoricalJointProgressAtHeightFromStrictAncestors": (
        "IndexedHistoricalJointProgressBelowProjectsStrictAncestorInputs",
        "IndexedHistoricalLowerAuthorityProgressGivesStrictAncestorAdvance",
        "IndexedLiveStrictAncestorsCloseOrdinaryDecisionOwnerRanks",
        "IndexedHistoricalDecisionOwnerClassesCloseRankProgressAtContext",
        "IndexedHistoricalServiceKernelsDischargeEntryCompletionAt",
        "IndexedHistoricalAuthorityProgressAtHeightFromStrictAncestors",
    ),
    "IndexedHistoricalStrictHeightMutualInductionClosesJointProgress": (
        "IndexedHistoricalJointProgressStartsAtHeightZero",
        "IndexedHistoricalJointProgressAdvancesOneHeight",
        "NatInduction",
    ),
    "IndexedHistoricalStrictHeightServiceCompositionClosesAuthority": (
        "IndexedHistoricalStrictHeightMutualInductionClosesJointProgress",
        "IndexedHistoricalJointProgressProjectsReleaseProperties",
    ),
    "IndexedHistoricalStrictHeightServiceCompositionClosesDecisionRank": (
        "IndexedHistoricalStrictHeightMutualInductionClosesJointProgress",
        "IndexedHistoricalJointProgressProjectsReleaseProperties",
    ),
    "IndexedLiveChainSpecClosesActivatedArchiveProducerResidual": (
        "IndexedChainSpecJoinedArchiveOwnerProducesTypedAuthority",
    ),
    "IndexedHistoricalRecoveryAuthorityAcquisitionResidualObligation": (
        "IndexedHistoricalCertificateRankProgressResidualObligation",
        "IndexedHistoricalDecisionTargetOwnerRankProgressObligation",
        "IndexedHistoricalStrictHeightServiceCompositionClosesAuthority",
    ),
    "IndexedHistoricalReleaseResidualsDischargeExactProgress": (
        "IndexedHistoricalCertificateRankProgressResidualObligation",
        "IndexedHistoricalDecisionRankProgressResidualObligation",
        "IndexedHistoricalRecoveryAuthorityAcquisitionResidualObligation",
        "IndexedHistoricalServiceKernelsDischargeEntryCompletion",
        "IndexedExactHistoricalRecoveryProgress",
        "PTL",
    ),
}

HISTORICAL_LOCAL_AUTHORITY_FORBIDDEN_DEPENDENCIES = (
    "IndexedAllResponsiveJoined",
    "AsyncAllResponsiveAppliedAt",
    "IndexedAllResponsiveExactApplicationsAt",
    "IndexedGstEventuallyCondition",
    "IndexedOneHeightProof",
    "IndexedOneHeightTemporalClosureIsExact",
    "ApplicationCompletionProgressProperty",
    "ApplicationLivenessProperty",
    "IndexedResponsiveDecisionApplicationProgress",
    "OneHeightCompletionLiveness",
    "VerificationOneHeightCompletion",
    "VerificationOneHeightCompletionObligation",
    "AsyncTemporalClosureOneHeightCompletionObligation",
    "IndexedHeightLivenessProperty",
    "IndexedExactHeightLivenessFromOneHeightAndExactRecoveryProgress",
)
