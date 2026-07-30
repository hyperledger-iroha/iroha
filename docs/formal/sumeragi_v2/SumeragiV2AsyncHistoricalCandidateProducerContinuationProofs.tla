---- MODULE SumeragiV2AsyncHistoricalCandidateProducerContinuationProofs ----
EXTENDS SumeragiV2AsyncCandidateProducerContinuationProofs,
        SumeragiV2AsyncDeadlockProofs

(***************************************************************************
Historical-recovery producer-continuation closure.

The ordinary continuation kernel is intentionally scoped to the frozen
voters.  Historical recovery has a different already-reviewed owner:
`PostGstRunHistoricalRecoveryNode(node)` is weakly fair for every responsive
node.  Production's `step_recovery` validates the selected lifecycle token,
classifies the retained successor/terminal evidence, and acknowledges the
handoff before returning from that same serialized turn.  The Async model
exposes the acknowledgement as the resolution-only branch of `RunNodeWork`;
the branch freezes every later local scheduler owner and directly publishes
the exact terminal tombstone.

The selected record need not be the target record.  The frozen prefix rank
therefore includes every immutable causal predecessor and active continuation
at or before the target ordinal.  A historical recovery turn terminalizes the
minimum selected record, strictly consuming that finite prefix.  Equal-rank
replacement and later producer work cannot refill it.  Well-founded descent
then reaches the target record, rather than silently assuming that it was
already selected.

No generic observer/voter fairness is introduced here.  Membership in
`asyncHistoricalRecoveryTargets` supplies the existing historical-runner
owner, and the ordinary responsive-node fairness clause services it.
***************************************************************************)

HistoricalCandidateProducerContinuationFrozenPrefixAtBudget(
    node, record, status, budget) ==
  /\ HistoricalCandidateProducerContinuationAtStatus(
       node, record, status)
  /\ record.ordinal \in Nat \ {0}
  /\ record.address.stage \in AsyncCandidateServiceStageClasses
  /\ AsyncCandidateProducerContinuationFrozenPrefixAtBudget(
       node, record.identity, record.ordinal, record.address.stage,
       status, budget)

HistoricalCandidateProducerContinuationPrefixDescentGoal(
    node, record, status, budget) ==
  AsyncCandidateProducerContinuationPrefixDescentGoal(
    node, record.identity, record.ordinal, record.address.stage,
    status, budget)

HistoricalCandidateProducerContinuationFrozenPrefixDescentProperty(
    specification) ==
  specification
    => \A node \in ValidatorIds,
          record \in AsyncCandidateProducerContinuationRecordSet,
          status \in {"Reserved", "Materialized"},
          budget \in
            AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
         HistoricalCandidateProducerContinuationFrozenPrefixAtBudget(
           node, record, status, budget)
           ~> HistoricalCandidateProducerContinuationPrefixDescentGoal(
                node, record, status, budget)

HistoricalCandidateProducerContinuationFrozenPrefixClosureProperty(
    specification) ==
  specification
    => \A node \in ValidatorIds,
          record \in AsyncCandidateProducerContinuationRecordSet,
          status \in {"Reserved", "Materialized"},
          budget \in
            AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
         HistoricalCandidateProducerContinuationFrozenPrefixAtBudget(
           node, record, status, budget)
           ~> AsyncCandidateProducerContinuationTargetStatusExit(
                record.identity, status)

HistoricalCandidateProducerContinuationResolutionClosureProperty(
    specification) ==
  specification
    => \A node \in ValidatorIds,
          record \in AsyncCandidateProducerContinuationRecordSet,
          status \in {"Reserved", "Materialized"}:
         HistoricalCandidateProducerContinuationAtStatus(
           node, record, status)
           ~> AsyncCandidateProducerContinuationTargetStatusExit(
                record.identity, status)

THEOREM HistoricalCandidateProducerContinuationFrozenPrefixRankIsFinite ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"},
     budget:
    /\ AsyncControlServiceStateTypeInvariant
    /\ HistoricalCandidateProducerContinuationFrozenPrefixAtBudget(
         node, record, status, budget)
      => /\ budget
               \in AsyncCandidateProducerContinuationFrozenPrefixRankCarrier
         /\ budget[1] \in Nat \ {0}
BY CandidateProducerContinuationFrozenPrefixRankIsFiniteAndPositive
   DEF HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       HistoricalCandidateProducerContinuationAtStatus

THEOREM HistoricalCandidateProducerContinuationStepPersistsOrExits ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"}:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ HistoricalCandidateProducerContinuationAtStatus(
         node, record, status)
    /\ [AsyncNext]_AsyncAllVars
      => \/ HistoricalCandidateProducerContinuationAtStatus(
               node, record, status)'
         \/ AsyncCandidateProducerContinuationTargetStatusExit(
              record.identity, status)'
BY ExternalContinuationPersistsOrDescendsOrReplayExits,
   LocalContinuationPersistsOrDescendsOrReplayExits,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   AsyncCandidateProducerContinuationGstExcludesResetReplay,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   IsaT(1800)
   DEF HistoricalCandidateProducerContinuationAtStatus,
       HistoricalRecoveryTarget,
       AsyncCandidateProducerContinuationTargetStatusExit,
       AsyncCandidateProducerContinuationDescendsOrReplayExitsAfter,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationStatusRank,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuationStateAfterDeparture,
       AsyncCandidateProducerContinuationReservationAvailableIn,
       AsyncCandidateProducerContinuationAddressCanAdvanceIn,
       AsyncCandidateProducerContinuationRecordsForAddressIn,
       AsyncControlServiceSlotTransition,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep, AsyncNonRunnerStep,
       PreGstResponsiveRestart, PreGstResponsiveReplay

THEOREM HistoricalCandidateProducerContinuationFrozenPrefixStepCannotReplenish ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"},
     budget \in
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ HistoricalCandidateProducerContinuationFrozenPrefixAtBudget(
         node, record, status, budget)
    /\ [AsyncNext]_AsyncAllVars
      => \/ HistoricalCandidateProducerContinuationFrozenPrefixAtBudget(
               node, record, status, budget)'
         \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
              node, record, status, budget)'
BY CandidateProducerContinuationFrozenPrefixStepCannotReplenish,
   HistoricalCandidateProducerContinuationStepPersistsOrExits,
   IsaT(600)
   DEF HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       HistoricalCandidateProducerContinuationPrefixDescentGoal,
       AsyncCandidateProducerContinuationPrefixDescentGoal,
       AsyncCandidateProducerContinuationTargetStatusExit

THEOREM HistoricalCandidateProducerContinuationFairTurnStrictlyDescendsPrefix ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"},
     budget \in
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncControlServiceSlotTransition
    /\ HistoricalCandidateProducerContinuationFrozenPrefixAtBudget(
         node, record, status, budget)
    /\ PostGstRunHistoricalRecoveryNode(node)
      => HistoricalCandidateProducerContinuationPrefixDescentGoal(
           node, record, status, budget)'
BY HistoricalCandidateProducerContinuationTurnIsResolutionOnly,
   HistoricalCandidateProducerContinuationTurnTerminalizesSelected,
   CandidateProducerContinuationResolutionSelectsMinimumFrozenOwner,
   CandidateProducerContinuationFrozenOriginsCannotReplenish,
   FS_CardinalityType, IsaT(2400)
   DEF HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       HistoricalCandidateProducerContinuationAtStatus,
       HistoricalCandidateProducerContinuationPrefixDescentGoal,
       AsyncCandidateProducerContinuationPrefixDescentGoal,
       AsyncCandidateProducerContinuationTargetStatusExit,
       AsyncCandidateProducerContinuationSelectedForRunnerResolution,
       AsyncCandidateProducerContinuationFrozenPrefixRank,
       AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
       AsyncCandidateProducerContinuationFrozenProducerBudget,
       AsyncCandidateProducerContinuationFrozenProducerTokens,
       AsyncCandidateProducerContinuationFrozenCandidateTokens,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationCausalWeight,
       AsyncCandidateProducerContinuationTargetAtStatus,
       AsyncCandidateProducerContinuationResolutionRequired,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationResolutionPredecessorsFor,
       AsyncCandidateProducerContinuationSelectedResolutionRecord,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationStatusRank,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuations,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncControlServiceSlotTransition,
       AsyncCausalEpisodeCandidates,
       AsyncCausalEpisodeFrozenPredecessorOrigins,
       AsyncCausalEpisodeServeWorkBudget,
       AsyncCausalEpisodeServeWorkTokens,
       AsyncCausalEpisodeServeReachDebt,
       AsyncCausalEpisodeStructuralRankOrdering,
       AsyncCausalEpisodeStructuralRankCarrier,
       AsyncCausalEpisodeServeRankOrdering,
       AsyncCausalEpisodeServeRankCarrier,
       SetLessThan, OpToRel

THEOREM HistoricalCandidateProducerContinuationSourceEnablesFairRunner ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"}:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCandidateProducerContinuationAtStatus(
         node, record, status)
      => ENABLED
           <<PostGstRunHistoricalRecoveryNode(node)>>_AsyncAllVars
BY HistoricalRecoveryRunnerEnabledAfterGst,
   AsyncStrongTypeProjectsAsyncType, Isa
   DEF HistoricalCandidateProducerContinuationAtStatus,
       HistoricalRecoveryTarget, AsyncStrongTypeInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncHistoricalRecoveryTypeInvariant

THEOREM AsyncLiveProvidesHistoricalCandidateProducerContinuationFrozenPrefixDescent ==
  \A initialContext:
    HistoricalCandidateProducerContinuationFrozenPrefixDescentProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   HistoricalCandidateProducerContinuationFrozenPrefixStepCannotReplenish,
   HistoricalCandidateProducerContinuationFairTurnStrictlyDescendsPrefix,
   HistoricalCandidateProducerContinuationSourceEnablesFairRunner,
   AsyncLiveSpecProjectsAsyncSpec,
   WF1, PTL, IsaT(1800)
   DEF HistoricalCandidateProducerContinuationFrozenPrefixDescentProperty,
       HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       HistoricalCandidateProducerContinuationPrefixDescentGoal,
       AsyncLiveSpecAt, AsyncFairnessAt

THEOREM AsyncLiveClosesHistoricalCandidateProducerContinuationFrozenPrefix ==
  \A initialContext:
    HistoricalCandidateProducerContinuationFrozenPrefixClosureProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesHistoricalCandidateProducerContinuationFrozenPrefixDescent,
   CandidateProducerContinuationFrozenPrefixRankOrderingIsWellFounded,
   WellFoundedLeadsTo
   DEF HistoricalCandidateProducerContinuationFrozenPrefixDescentProperty,
       HistoricalCandidateProducerContinuationFrozenPrefixClosureProperty,
       HistoricalCandidateProducerContinuationPrefixDescentGoal,
       AsyncCandidateProducerContinuationPrefixDescentGoal,
       AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier

THEOREM AsyncLiveProvidesHistoricalCandidateProducerContinuationResolutionClosure ==
  \A initialContext:
    HistoricalCandidateProducerContinuationResolutionClosureProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   HistoricalCandidateProducerContinuationFrozenPrefixRankIsFinite,
   AsyncLiveClosesHistoricalCandidateProducerContinuationFrozenPrefix,
   AsyncLiveSpecProjectsAsyncSpec,
   PTL, IsaT(600)
   DEF HistoricalCandidateProducerContinuationResolutionClosureProperty,
       HistoricalCandidateProducerContinuationFrozenPrefixClosureProperty,
       HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       AsyncCandidateProducerContinuationFrozenPrefixRank

=============================================================================
