---- MODULE SumeragiV2AsyncHistoricalCandidateProducerContinuationProofs ----
EXTENDS SumeragiV2AsyncCandidateProducerContinuationProofs,
        SumeragiV2AsyncDeadlockProofs

(***************************************************************************
Historical-recovery producer-continuation closure.

The ordinary continuation kernel is intentionally scoped to the frozen
voters.  Historical recovery has a different already-reviewed owner:
`PostGstRunHistoricalRecoveryNode(node)` is weakly fair while its selected
continuation has Ready evidence or a deterministic Local replay step.
Production's `step_recovery` validates the selected lifecycle token before it
acknowledges the handoff.  A non-Ready Local reservation first rematerializes
the stored exact carrier and retains Reserved status; the following turn sees
that carrier as Ready.  ConditionalTransport and VolatileBody reservations
remain owned by their existing transport/body corridors until they become
Ready, so runner fairness is never invented while the runner is disabled.

The selected record need not be the target record.  The frozen prefix rank
therefore includes every immutable causal predecessor and active continuation
at or before the target ordinal.  A Ready historical recovery turn consumes
one lifecycle stage of the minimum selected record; a Local replay turn is a
separate finite non-descent episode which only publishes that record's stored
carrier.  Equal-rank replacement and later producer work cannot refill the
prefix.

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

HistoricalCandidateProducerContinuationLocalReplayDistance(node) ==
  IF asyncRunnerPhase[node] = "Local" THEN 1 ELSE 2

HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
    node, record, distance) ==
  /\ HistoricalCandidateProducerContinuationSelectedAtStatus(
       node, record, "Reserved")
  /\ ~AsyncCandidateProducerContinuationResolutionReady(node)
  /\ record.sourceClass = "Local"
  /\ distance \in 1..2
  /\ distance =
       HistoricalCandidateProducerContinuationLocalReplayDistance(node)

HistoricalCandidateProducerContinuationLocalReplayProgress(
    node, record, distance) ==
  \/ AsyncCandidateProducerContinuationResolutionReady(node)
  \/ AsyncCandidateProducerContinuationTargetStatusExit(
       record.identity, "Reserved")
  \/ \E lower \in SetLessThan(distance, OpToRel(<, Nat), Nat):
       HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
         node, record, lower)

HistoricalCandidateProducerContinuationLocalReplayClosureProperty(
    specification) ==
  specification
    => \A node \in ValidatorIds,
          record \in AsyncCandidateProducerContinuationRecordSet,
          distance \in 1..2:
         HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
           node, record, distance)
           ~> AsyncCandidateProducerContinuationResolutionReady(node)
                 \/ AsyncCandidateProducerContinuationTargetStatusExit(
                      record.identity, "Reserved")

HistoricalCandidateProducerContinuationLocalReplayDescentProperty(
    specification) ==
  specification
    => \A node \in ValidatorIds,
          record \in AsyncCandidateProducerContinuationRecordSet,
          distance \in 1..2:
         HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
           node, record, distance)
           ~> HistoricalCandidateProducerContinuationLocalReplayProgress(
                node, record, distance)

HistoricalCandidateProducerContinuationFrozenPrefixReady(
    node, record, status, budget) ==
  /\ HistoricalCandidateProducerContinuationFrozenPrefixAtBudget(
       node, record, status, budget)
  /\ AsyncCandidateProducerContinuationResolutionReady(node)

HistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode(
    node, record, status, budget, selected, distance) ==
  /\ HistoricalCandidateProducerContinuationFrozenPrefixAtBudget(
       node, record, status, budget)
  /\ ~HistoricalCandidateProducerContinuationPrefixDescentGoal(
       node, record, status, budget)
  /\ HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
       node, selected, distance)

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

THEOREM HistoricalCandidateProducerContinuationLocalReplayStepCannotReplenish ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     distance \in 1..2:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
         node, record, distance)
    /\ [AsyncNext]_AsyncAllVars
      => \/ HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
               node, record, distance)'
         \/ HistoricalCandidateProducerContinuationLocalReplayProgress(
              node, record, distance)'
BY HistoricalCandidateProducerContinuationStepPersistsOrExits,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   IsaT(1200)
   DEF HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance,
       HistoricalCandidateProducerContinuationLocalReplayProgress,
       HistoricalCandidateProducerContinuationLocalReplayDistance,
       HistoricalCandidateProducerContinuationSelectedAtStatus,
       HistoricalCandidateProducerContinuationAtStatus,
       AsyncCandidateProducerContinuationTargetStatusExit,
       AsyncCandidateProducerContinuationResolutionReady,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncAllVars, SetLessThan, OpToRel

THEOREM HistoricalCandidateProducerContinuationFairLocalReplayDescends ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     distance \in 1..2:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncControlServiceSlotTransition
    /\ HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
         node, record, distance)
    /\ PostGstRunHistoricalRecoveryNode(node)
      => HistoricalCandidateProducerContinuationLocalReplayProgress(
           node, record, distance)'
BY HistoricalCandidateProducerContinuationLocalReplayTurnApproachesReady,
   IsaT(900)
   DEF HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance,
       HistoricalCandidateProducerContinuationLocalReplayProgress,
       HistoricalCandidateProducerContinuationLocalReplayDistance,
       HistoricalCandidateProducerContinuationSelectedAtStatus,
       HistoricalCandidateProducerContinuationAtStatus,
       AsyncCandidateProducerContinuationTargetStatusExit,
       SetLessThan, OpToRel

THEOREM HistoricalCandidateProducerContinuationLocalReplayEnablesFairRunner ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     distance \in 1..2:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
         node, record, distance)
      => ENABLED
           <<PostGstRunHistoricalRecoveryNode(node)>>_AsyncAllVars
BY HistoricalRecoveryRunnerEnabledOrAwaitsExternalContinuationAfterGst,
   HistoricalRecoveryTargetsAreValidators, Isa
   DEF HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance,
       HistoricalCandidateProducerContinuationSelectedAtStatus,
       HistoricalCandidateProducerContinuationAtStatus,
       HistoricalRecoveryRunnerBlockedOnExternalContinuation,
       AsyncCandidateProducerContinuationSelectedResolutionRecord

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
    /\ AsyncCandidateProducerContinuationResolutionReady(node)
    /\ PostGstRunHistoricalRecoveryNode(node)
      => HistoricalCandidateProducerContinuationPrefixDescentGoal(
           node, record, status, budget)'
BY HistoricalCandidateProducerContinuationTurnIsResolutionOrExactReplay,
   HistoricalCandidateProducerContinuationReadyTurnConsumesExactStage,
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
    /\ ~HistoricalRecoveryRunnerBlockedOnExternalContinuation(node)
      => ENABLED
           <<PostGstRunHistoricalRecoveryNode(node)>>_AsyncAllVars
BY HistoricalRecoveryRunnerEnabledOrAwaitsExternalContinuationAfterGst,
   AsyncStrongTypeProjectsAsyncType, Isa
   DEF HistoricalCandidateProducerContinuationAtStatus,
       HistoricalRecoveryTarget, AsyncStrongTypeInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncHistoricalRecoveryTypeInvariant

THEOREM HistoricalCandidateProducerContinuationFrozenPrefixClassifiesReadyOrLocalReplay ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"},
     budget \in
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ HistoricalCandidateProducerContinuationFrozenPrefixAtBudget(
         node, record, status, budget)
      => \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
              node, record, status, budget)
         \/ HistoricalCandidateProducerContinuationFrozenPrefixReady(
              node, record, status, budget)
         \/ \E selected
                  \in AsyncCandidateProducerContinuationRecordSet,
               distance \in 1..2:
              HistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode(
                node, record, status, budget, selected, distance)
BY CandidateProducerContinuationResolutionSelectsMinimumFrozenOwner,
   ExternalCandidateProducerContinuationSelectionIsReady,
   IsaT(1200)
   DEF HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       HistoricalCandidateProducerContinuationAtStatus,
       HistoricalCandidateProducerContinuationPrefixDescentGoal,
       HistoricalCandidateProducerContinuationFrozenPrefixReady,
       HistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode,
       HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance,
       HistoricalCandidateProducerContinuationSelectedAtStatus,
       HistoricalCandidateProducerContinuationLocalReplayDistance,
       AsyncCandidateProducerContinuationResolutionReady,
       AsyncCandidateProducerContinuationResolutionRequired,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationSelectedResolutionRecord,
       AsyncCandidateProducerContinuationRecordSet,
       AsyncCandidateProducerContinuationRecord,
       AsyncCandidateProducerContinuationSourceClasses

THEOREM HistoricalCandidateProducerContinuationFrozenPrefixReadyStepCannotReplenish ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"},
     budget \in
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ HistoricalCandidateProducerContinuationFrozenPrefixReady(
         node, record, status, budget)
    /\ [AsyncNext]_AsyncAllVars
      => \/ HistoricalCandidateProducerContinuationFrozenPrefixReady(
               node, record, status, budget)'
         \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
              node, record, status, budget)'
BY HistoricalCandidateProducerContinuationFrozenPrefixStepCannotReplenish,
   HistoricalCandidateProducerContinuationStepPersistsOrExits,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck,
   AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor,
   AsyncCandidateProducerSemanticHandoffRetirementRequiresAck,
   IsaT(1800)
   DEF HistoricalCandidateProducerContinuationFrozenPrefixReady,
       HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       HistoricalCandidateProducerContinuationAtStatus,
       HistoricalCandidateProducerContinuationPrefixDescentGoal,
       AsyncCandidateProducerContinuationResolutionReady,
       AsyncCandidateProducerContinuationResolutionRequired,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationSelectedResolutionRecord,
       AsyncCandidateProducerContinuationTargetStatusExit

THEOREM HistoricalCandidateProducerContinuationSelectedLocalExitDropsFrozenPrefix ==
  \A node \in ValidatorIds,
     record, selected \in
       AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"},
     budget \in
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
     distance \in 1..2:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ HistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode(
         node, record, status, budget, selected, distance)
    /\ [AsyncNext]_AsyncAllVars
    /\ AsyncCandidateProducerContinuationTargetStatusExit(
         selected.identity, "Reserved")'
      => HistoricalCandidateProducerContinuationPrefixDescentGoal(
           node, record, status, budget)'
BY HistoricalCandidateProducerContinuationFrozenPrefixStepCannotReplenish,
   CandidateProducerContinuationFrozenOriginsCannotReplenish,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   FS_CardinalityType, IsaT(1800)
   DEF HistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode,
       HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       HistoricalCandidateProducerContinuationPrefixDescentGoal,
       AsyncCandidateProducerContinuationPrefixDescentGoal,
       AsyncCandidateProducerContinuationTargetStatusExit,
       AsyncCandidateProducerContinuationFrozenPrefixRank,
       AsyncCandidateProducerContinuationFrozenProducerBudget,
       AsyncCandidateProducerContinuationFrozenProducerTokens,
       AsyncCandidateProducerContinuationFrozenCandidateTokens,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationStatusRank,
       SetLessThan, OpToRel

THEOREM AsyncLiveProvidesHistoricalCandidateProducerContinuationLocalReplayDescent ==
  \A initialContext:
    HistoricalCandidateProducerContinuationLocalReplayDescentProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   HistoricalCandidateProducerContinuationLocalReplayStepCannotReplenish,
   HistoricalCandidateProducerContinuationFairLocalReplayDescends,
   HistoricalCandidateProducerContinuationLocalReplayEnablesFairRunner,
   AsyncLiveSpecProjectsAsyncSpec,
   WF1, PTL, IsaT(1200)
   DEF HistoricalCandidateProducerContinuationLocalReplayDescentProperty,
       HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance,
       HistoricalCandidateProducerContinuationLocalReplayProgress,
       AsyncLiveSpecAt, AsyncFairnessAt

THEOREM AsyncLiveClosesHistoricalCandidateProducerContinuationLocalReplay ==
  \A initialContext:
    HistoricalCandidateProducerContinuationLocalReplayClosureProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesHistoricalCandidateProducerContinuationLocalReplayDescent,
   NatLessThanWellFounded, WellFoundedLeadsTo, Isa
   DEF HistoricalCandidateProducerContinuationLocalReplayDescentProperty,
       HistoricalCandidateProducerContinuationLocalReplayClosureProperty,
       HistoricalCandidateProducerContinuationLocalReplayProgress,
       HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance,
       SetLessThan, OpToRel

THEOREM AsyncLiveClosesHistoricalCandidateProducerContinuationFrozenPrefixReadyEpisode ==
  \A initialContext:
    AsyncLiveSpecAt(initialContext)
      => \A node \in ValidatorIds,
            record \in AsyncCandidateProducerContinuationRecordSet,
            status \in {"Reserved", "Materialized"},
            budget \in
              AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
           HistoricalCandidateProducerContinuationFrozenPrefixReady(
             node, record, status, budget)
             ~> HistoricalCandidateProducerContinuationPrefixDescentGoal(
                  node, record, status, budget)
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   HistoricalCandidateProducerContinuationFrozenPrefixReadyStepCannotReplenish,
   HistoricalCandidateProducerContinuationFairTurnStrictlyDescendsPrefix,
   HistoricalCandidateProducerContinuationSourceEnablesFairRunner,
   AsyncLiveSpecProjectsAsyncSpec,
   WF1, PTL, IsaT(1200)
   DEF HistoricalCandidateProducerContinuationFrozenPrefixReady,
       HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       HistoricalCandidateProducerContinuationAtStatus,
       HistoricalCandidateProducerContinuationPrefixDescentGoal,
       HistoricalRecoveryRunnerBlockedOnExternalContinuation,
       AsyncLiveSpecAt, AsyncFairnessAt

THEOREM AsyncLiveClosesHistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode ==
  \A initialContext:
    AsyncLiveSpecAt(initialContext)
      => \A node \in ValidatorIds,
            record, selected
              \in AsyncCandidateProducerContinuationRecordSet,
            status \in {"Reserved", "Materialized"},
            budget \in
              AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
            distance \in 1..2:
           HistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode(
             node, record, status, budget, selected, distance)
             ~> \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
                       node, record, status, budget)
                 \/ HistoricalCandidateProducerContinuationFrozenPrefixReady(
                      node, record, status, budget)
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateServiceTombstoneLifecycle,
   HistoricalCandidateProducerContinuationFrozenPrefixStepCannotReplenish,
   HistoricalCandidateProducerContinuationSelectedLocalExitDropsFrozenPrefix,
   AsyncLiveClosesHistoricalCandidateProducerContinuationLocalReplay,
   AsyncLiveSpecProjectsAsyncSpec,
   PTL, IsaT(1800)
   DEF HistoricalCandidateProducerContinuationLocalReplayClosureProperty,
       HistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode,
       HistoricalCandidateProducerContinuationFrozenPrefixReady,
       HistoricalCandidateProducerContinuationPrefixDescentGoal,
       AsyncLiveSpecAt

THEOREM AsyncLiveProvidesHistoricalCandidateProducerContinuationFrozenPrefixDescent ==
  \A initialContext:
    HistoricalCandidateProducerContinuationFrozenPrefixDescentProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   HistoricalCandidateProducerContinuationFrozenPrefixClassifiesReadyOrLocalReplay,
   AsyncLiveClosesHistoricalCandidateProducerContinuationFrozenPrefixReadyEpisode,
   AsyncLiveClosesHistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode,
   AsyncLiveSpecProjectsAsyncSpec,
   PTL, IsaT(1800)
   DEF HistoricalCandidateProducerContinuationFrozenPrefixDescentProperty,
       HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       HistoricalCandidateProducerContinuationPrefixDescentGoal,
       HistoricalCandidateProducerContinuationFrozenPrefixReady,
       HistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode,
       AsyncLiveSpecAt

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
