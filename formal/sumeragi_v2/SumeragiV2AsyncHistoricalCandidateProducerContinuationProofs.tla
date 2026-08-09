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
carrier.  If an older physical lifecycle acquires the serialized ingress turn
during either episode, runner eligibility hands off to the explicit ingress-cut
residual; clearing that barrier is not counted as continuation progress.
Equal-rank replacement and later producer work cannot refill the prefix.

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

(***************************************************************************
Ingress-cut residual.

The historical target remains a member of the global durable continuation
set while an earlier admitted ingress lifecycle owns the serialized runner.
It is therefore not a target exit when the runner-eligible projection is
empty.  The residual freezes the original continuation-prefix budget and
hands the earlier physical lifecycle to the separate finite ingress corridor.
That corridor may return only to this same framed prefix, to a strictly lower
prefix, or to the exact target-status exit; clearing the barrier is not itself
advertised as continuation progress.
***************************************************************************)

HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
    node, record, status, budget) ==
  /\ HistoricalCandidateProducerContinuationFrozenPrefixAtBudget(
       node, record, status, budget)
  /\ ~HistoricalCandidateProducerContinuationPrefixDescentGoal(
       node, record, status, budget)
  /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(node)

HistoricalCandidateProducerContinuationFrozenPrefixDescentOrIngressCutProperty(
    specification) ==
  specification
    => \A node \in ValidatorIds,
          record \in AsyncCandidateProducerContinuationRecordSet,
          status \in {"Reserved", "Materialized"},
          budget \in
            AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
         HistoricalCandidateProducerContinuationFrozenPrefixAtBudget(
           node, record, status, budget)
           ~> \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
                    node, record, status, budget)
               \/ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
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
  /\ ~AsyncCandidateProducerContinuationRunnerResolutionReady(node)
  /\ record.sourceClass = "Local"
  /\ distance \in 1..2
  /\ distance =
       HistoricalCandidateProducerContinuationLocalReplayDistance(node)

HistoricalCandidateProducerContinuationLocalReplayProgress(
    node, record, distance) ==
  \/ AsyncCandidateProducerContinuationRunnerResolutionReady(node)
  \/ AsyncCandidateProducerContinuationTargetStatusExit(
       record.identity, "Reserved")
  \/ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
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
           ~> AsyncCandidateProducerContinuationRunnerResolutionReady(node)
                 \/ AsyncCandidateProducerContinuationTargetStatusExit(
                      record.identity, "Reserved")
                 \/ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
                      node)

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
  /\ AsyncCandidateProducerContinuationRunnerResolutionReady(node)

HistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode(
    node, record, status, budget, selected, distance) ==
  /\ HistoricalCandidateProducerContinuationFrozenPrefixAtBudget(
       node, record, status, budget)
  /\ ~HistoricalCandidateProducerContinuationPrefixDescentGoal(
       node, record, status, budget)
  /\ HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
       node, selected, distance)

HistoricalCandidateProducerContinuationFrozenPrefixRunnerEligible(
    node, record, status, budget) ==
  /\ HistoricalCandidateProducerContinuationFrozenPrefixAtBudget(
       node, record, status, budget)
  /\ ~HistoricalCandidateProducerContinuationPrefixDescentGoal(
       node, record, status, budget)
  /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(node)

HistoricalCandidateProducerContinuationIngressCutClosureProperty(
    specification) ==
  specification
    => \A node \in ValidatorIds,
          record \in AsyncCandidateProducerContinuationRecordSet,
          status \in {"Reserved", "Materialized"},
          budget \in
            AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
         HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
           node, record, status, budget)
           ~> \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
                    node, record, status, budget)
               \/ HistoricalCandidateProducerContinuationFrozenPrefixRunnerEligible(
                    node, record, status, budget)

THEOREM HistoricalCandidateProducerContinuationFrozenPrefixRankIsFinite ==
  \A budget:
    \A node \in ValidatorIds,
       record \in AsyncCandidateProducerContinuationRecordSet,
       status \in {"Reserved", "Materialized"}:
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
    /\ AsyncProgressOwnershipInvariant
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
       AsyncCandidateProducerContinuationRunnerResolutionReady,
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord,
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
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord

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
    /\ AsyncCandidateProducerContinuationRunnerResolutionReady(node)
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
       AsyncCandidateProducerContinuationFrozenCandidateOwners,
       AsyncCandidateProducerContinuationFrozenCausalCandidates,
       AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationTargetPhysicalCut,
       AsyncCandidateProducerContinuationFrozenServeIngressIdentities,
       AsyncCandidateProducerContinuationFrozenServeWorkTokens,
       AsyncCandidateProducerContinuationFrozenServeWorkBudget,
       AsyncCandidateProducerContinuationFrozenServeReachDebt,
       AsyncCandidateProducerContinuationCausalWeight,
       AsyncCandidateProducerContinuationTargetAtStatus,
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerResolutionReady,
       AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationResolutionPredecessorsFor,
       AsyncCandidateProducerContinuationLogicalPrecedes,
       AsyncCandidateProducerContinuationPhysicallyEligibleResolutionRecordsForNode,
       AsyncCandidateProducerContinuationPhysicallyBehindActiveTarget,
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
    /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
    /\ ~HistoricalRecoveryRunnerBlockedOnExternalContinuation(node)
      => ENABLED
           <<PostGstRunHistoricalRecoveryNode(node)>>_AsyncAllVars
BY HistoricalRecoveryRunnerEnabledOrAwaitsExternalContinuationAfterGst,
   AsyncStrongTypeProjectsAsyncType, Isa
   DEF HistoricalCandidateProducerContinuationAtStatus,
       HistoricalRecoveryTarget, AsyncStrongTypeInvariant,
       AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncHistoricalRecoveryTypeInvariant

THEOREM HistoricalCandidateProducerContinuationFrozenPrefixClassifiesRunnerOrIngressCut ==
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
         \/ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
              node, record, status, budget)
BY CandidateProducerContinuationResolutionSelectsMinimumFrozenOwner,
   AsyncStrongTypeProjectsControlServiceStateType,
   AsyncCandidateProducerContinuationRunnerSelectionIsTwoStageLogicalMinimum,
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
       HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual,
       AsyncCandidateProducerContinuationRunnerResolutionReady,
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord,
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
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ HistoricalCandidateProducerContinuationFrozenPrefixReady(
         node, record, status, budget)
    /\ [AsyncNext]_AsyncAllVars
      => \/ HistoricalCandidateProducerContinuationFrozenPrefixReady(
               node, record, status, budget)'
         \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
              node, record, status, budget)'
         \/ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
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
       HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual,
       AsyncCandidateProducerContinuationRunnerResolutionReady,
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord,
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
       AsyncCandidateProducerContinuationFrozenCandidateOwners,
       AsyncCandidateProducerContinuationFrozenCausalCandidates,
       AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationTargetPhysicalCut,
       AsyncCandidateProducerContinuationFrozenServeIngressIdentities,
       AsyncCandidateProducerContinuationFrozenServeWorkTokens,
       AsyncCandidateProducerContinuationFrozenServeWorkBudget,
       AsyncCandidateProducerContinuationFrozenServeReachDebt,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationStatusRank,
       SetLessThan, OpToRel

THEOREM AsyncLiveProvidesHistoricalCandidateProducerContinuationLocalReplayDescent ==
  \A initialContext:
    HistoricalCandidateProducerContinuationLocalReplayDescentProperty(
      AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncLiveSpecAt(initialContext)
         PROVE \A node \in ValidatorIds,
                   record \in AsyncCandidateProducerContinuationRecordSet,
                   distance \in 1..2:
                 HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
                   node, record, distance)
                   ~> HistoricalCandidateProducerContinuationLocalReplayProgress(
                        node, record, distance)
    <2>1. [](/\ AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant
              /\ AsyncCandidateServiceLifecycleInvariant)
      BY <1>1, AsyncLiveSpecProjectsAsyncSpec,
         AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncFiniteRunnerSpecAlwaysCandidateServiceTombstoneLifecycle, PTL
         DEF AsyncCandidateServiceTombstoneLifecycleInvariant
    <2>2. ASSUME NEW node \in ValidatorIds,
                  NEW record
                    \in AsyncCandidateProducerContinuationRecordSet,
                  NEW distance \in 1..2
           PROVE HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
                   node, record, distance)
                   ~> HistoricalCandidateProducerContinuationLocalReplayProgress(
                        node, record, distance)
      <3>1. CASE node \in Responsive
        <4>1. [](HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
                    node, record, distance)
                  /\ ~HistoricalCandidateProducerContinuationLocalReplayProgress(
                       node, record, distance)
                 => ENABLED
                      <<PostGstRunHistoricalRecoveryNode(node)>>_AsyncAllVars)
          BY <2>1,
             HistoricalCandidateProducerContinuationLocalReplayEnablesFairRunner,
             PTL
        <4>2. (HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
                    node, record, distance)
                  /\ ~HistoricalCandidateProducerContinuationLocalReplayProgress(
                       node, record, distance)
                  /\ <<PostGstRunHistoricalRecoveryNode(node)>>_AsyncAllVars
                  /\ [AsyncNext]_AsyncAllVars
                 => HistoricalCandidateProducerContinuationLocalReplayProgress(
                      node, record, distance)')
          BY <2>1,
             HistoricalCandidateProducerContinuationFairLocalReplayDescends,
             PTL
             DEF PostGstRunHistoricalRecoveryNode,
                 RunHistoricalRecoveryNode, RunNodeWork,
                 AsyncNext, AsyncAllVars
        <4>3. (HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
                    node, record, distance)
                  /\ ~HistoricalCandidateProducerContinuationLocalReplayProgress(
                       node, record, distance)
                  /\ [AsyncNext]_AsyncAllVars
                 => \/ HistoricalCandidateProducerContinuationLocalReplayProgress(
                          node, record, distance)'
                    \/ HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
                         node, record, distance)')
          BY <2>1,
             HistoricalCandidateProducerContinuationLocalReplayStepCannotReplenish,
             PTL
        <4>4. WF_AsyncAllVars(PostGstRunHistoricalRecoveryNode(node))
          BY <1>1, <3>1, AsyncLiveSpecProjectsAsyncSpec
             DEF AsyncSpecAt, AsyncFairnessAt
        <4>5. [][AsyncNext]_AsyncAllVars
          BY <1>1, AsyncLiveSpecProjectsAsyncSpec DEF AsyncSpecAt
        <4> QED BY <4>1, <4>2, <4>3, <4>4, <4>5, PTL
      <3>2. CASE node \notin Responsive
        <4>1. []~HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance(
                    node, record, distance)
          BY <2>1, <3>2, AsyncStrongTypeProjectsAsyncType, PTL
             DEF HistoricalCandidateProducerContinuationSelectedLocalReplayAtDistance,
                 HistoricalCandidateProducerContinuationSelectedAtStatus,
                 HistoricalCandidateProducerContinuationAtStatus,
                 HistoricalRecoveryTarget,
                 AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncHistoricalRecoveryTypeInvariant
        <4> QED BY <4>1, PTL
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>2
  <1> QED BY <1>1
     DEF HistoricalCandidateProducerContinuationLocalReplayDescentProperty

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
             ~> \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
                      node, record, status, budget)
                 \/ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
                      node, record, status, budget)
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncLiveSpecAt(initialContext)
         PROVE \A node \in ValidatorIds,
                   record \in AsyncCandidateProducerContinuationRecordSet,
                   status \in {"Reserved", "Materialized"},
                   budget \in
                     AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
                 HistoricalCandidateProducerContinuationFrozenPrefixReady(
                   node, record, status, budget)
                   ~> \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
                            node, record, status, budget)
                       \/ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
                            node, record, status, budget)
    <2>1. [](/\ AsyncStrongTypeInvariant
              /\ AsyncProgressOwnershipInvariant
              /\ AsyncCandidateServiceLifecycleInvariant
              /\ AsyncCandidateProducerContinuationExternalCoverageInvariant)
      BY <1>1, AsyncLiveSpecProjectsAsyncSpec,
         AsyncSpecAlwaysStrongTypeInvariant,
         AsyncSpecAlwaysProgressOwnershipInvariant,
         AsyncFiniteRunnerSpecAlwaysCandidateServiceTombstoneLifecycle,
         AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage, PTL
         DEF AsyncCandidateServiceTombstoneLifecycleInvariant
    <2>2. ASSUME NEW node \in ValidatorIds,
                  NEW record
                    \in AsyncCandidateProducerContinuationRecordSet,
                  NEW status \in {"Reserved", "Materialized"},
                  NEW budget \in
                    AsyncCandidateProducerContinuationFrozenPrefixRankCarrier
           PROVE HistoricalCandidateProducerContinuationFrozenPrefixReady(
                   node, record, status, budget)
                   ~> \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
                            node, record, status, budget)
                       \/ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
                            node, record, status, budget)
      <3> DEFINE Goal ==
             \/ HistoricalCandidateProducerContinuationPrefixDescentGoal(
                  node, record, status, budget)
             \/ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
                  node, record, status, budget)
      <3>1. CASE node \in Responsive
        <4>1. [](HistoricalCandidateProducerContinuationFrozenPrefixReady(
                    node, record, status, budget)
                  /\ ~Goal
                 => ENABLED
                      <<PostGstRunHistoricalRecoveryNode(node)>>_AsyncAllVars)
          BY <2>1,
             HistoricalCandidateProducerContinuationSourceEnablesFairRunner,
             PTL
             DEF HistoricalCandidateProducerContinuationFrozenPrefixReady,
                 HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
                 HistoricalCandidateProducerContinuationAtStatus,
                 HistoricalRecoveryRunnerBlockedOnExternalContinuation
        <4>2. (HistoricalCandidateProducerContinuationFrozenPrefixReady(
                    node, record, status, budget)
                  /\ ~Goal
                  /\ <<PostGstRunHistoricalRecoveryNode(node)>>_AsyncAllVars
                  /\ [AsyncNext]_AsyncAllVars
                 => Goal')
          BY <2>1,
             HistoricalCandidateProducerContinuationFairTurnStrictlyDescendsPrefix,
             PTL
             DEF HistoricalCandidateProducerContinuationFrozenPrefixReady,
                 PostGstRunHistoricalRecoveryNode,
                 RunHistoricalRecoveryNode, RunNodeWork,
                 AsyncNext, AsyncAllVars, Goal
        <4>3. (HistoricalCandidateProducerContinuationFrozenPrefixReady(
                    node, record, status, budget)
                  /\ ~Goal
                  /\ [AsyncNext]_AsyncAllVars
                 => \/ Goal'
                    \/ HistoricalCandidateProducerContinuationFrozenPrefixReady(
                         node, record, status, budget)')
          BY <2>1,
             HistoricalCandidateProducerContinuationFrozenPrefixReadyStepCannotReplenish,
             PTL DEF Goal
        <4>4. WF_AsyncAllVars(PostGstRunHistoricalRecoveryNode(node))
          BY <1>1, <3>1, AsyncLiveSpecProjectsAsyncSpec
             DEF AsyncSpecAt, AsyncFairnessAt
        <4>5. [][AsyncNext]_AsyncAllVars
          BY <1>1, AsyncLiveSpecProjectsAsyncSpec DEF AsyncSpecAt
        <4> QED BY <4>1, <4>2, <4>3, <4>4, <4>5, PTL
      <3>2. CASE node \notin Responsive
        <4>1. []~HistoricalCandidateProducerContinuationFrozenPrefixReady(
                    node, record, status, budget)
          BY <2>1, <3>2, AsyncStrongTypeProjectsAsyncType, PTL
             DEF HistoricalCandidateProducerContinuationFrozenPrefixReady,
                 HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
                 HistoricalCandidateProducerContinuationAtStatus,
                 HistoricalRecoveryTarget,
                 AsyncTypeInvariant, AsyncSchedulerTypeInvariant,
                 AsyncHistoricalRecoveryTypeInvariant
        <4> QED BY <4>1, PTL
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>2
  <1> QED BY <1>1

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
                 \/ HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual(
                      node, record, status, budget)
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncFiniteRunnerSpecAlwaysCandidateServiceTombstoneLifecycle,
   HistoricalCandidateProducerContinuationFrozenPrefixStepCannotReplenish,
   HistoricalCandidateProducerContinuationSelectedLocalExitDropsFrozenPrefix,
   AsyncLiveClosesHistoricalCandidateProducerContinuationLocalReplay,
   AsyncLiveSpecProjectsAsyncSpec,
   PTL, IsaT(1800)
   DEF HistoricalCandidateProducerContinuationLocalReplayClosureProperty,
       HistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode,
       HistoricalCandidateProducerContinuationFrozenPrefixReady,
       HistoricalCandidateProducerContinuationPrefixDescentGoal,
       HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual,
       HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncLiveSpecAt

THEOREM AsyncLiveProvidesHistoricalCandidateProducerContinuationFrozenPrefixDescentOrIngressCut ==
  \A initialContext:
    HistoricalCandidateProducerContinuationFrozenPrefixDescentOrIngressCutProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   HistoricalCandidateProducerContinuationFrozenPrefixClassifiesRunnerOrIngressCut,
   AsyncLiveClosesHistoricalCandidateProducerContinuationFrozenPrefixReadyEpisode,
   AsyncLiveClosesHistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode,
   AsyncLiveSpecProjectsAsyncSpec,
   PTL, IsaT(1800)
   DEF HistoricalCandidateProducerContinuationFrozenPrefixDescentOrIngressCutProperty,
       HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       HistoricalCandidateProducerContinuationPrefixDescentGoal,
       HistoricalCandidateProducerContinuationFrozenPrefixReady,
       HistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode,
       HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual,
       AsyncLiveSpecAt

THEOREM HistoricalCandidateProducerContinuationIngressCutClosureDischargesFrozenPrefixDescent ==
  \A initialContext:
    HistoricalCandidateProducerContinuationIngressCutClosureProperty(
      AsyncLiveSpecAt(initialContext))
      => HistoricalCandidateProducerContinuationFrozenPrefixDescentProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesHistoricalCandidateProducerContinuationFrozenPrefixDescentOrIngressCut,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   HistoricalCandidateProducerContinuationFrozenPrefixClassifiesRunnerOrIngressCut,
   AsyncLiveClosesHistoricalCandidateProducerContinuationFrozenPrefixReadyEpisode,
   AsyncLiveClosesHistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode,
   AsyncLiveSpecProjectsAsyncSpec,
   PTL, IsaT(1800)
   DEF HistoricalCandidateProducerContinuationIngressCutClosureProperty,
       HistoricalCandidateProducerContinuationFrozenPrefixDescentProperty,
       HistoricalCandidateProducerContinuationFrozenPrefixDescentOrIngressCutProperty,
       HistoricalCandidateProducerContinuationFrozenPrefixRunnerEligible,
       HistoricalCandidateProducerContinuationFrozenPrefixIngressCutResidual,
       HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       HistoricalCandidateProducerContinuationPrefixDescentGoal,
       HistoricalCandidateProducerContinuationFrozenPrefixReady,
       HistoricalCandidateProducerContinuationFrozenPrefixLocalReplayEpisode,
       AsyncLiveSpecAt

THEOREM HistoricalCandidateProducerContinuationIngressCutClosureClosesFrozenPrefix ==
  \A initialContext:
    HistoricalCandidateProducerContinuationIngressCutClosureProperty(
      AsyncLiveSpecAt(initialContext))
      => HistoricalCandidateProducerContinuationFrozenPrefixClosureProperty(
           AsyncLiveSpecAt(initialContext))
BY HistoricalCandidateProducerContinuationIngressCutClosureDischargesFrozenPrefixDescent,
   CandidateProducerContinuationFrozenPrefixRankOrderingIsWellFounded,
   WellFoundedLeadsTo
   DEF HistoricalCandidateProducerContinuationIngressCutClosureProperty,
       HistoricalCandidateProducerContinuationFrozenPrefixDescentProperty,
       HistoricalCandidateProducerContinuationFrozenPrefixClosureProperty,
       HistoricalCandidateProducerContinuationPrefixDescentGoal,
       AsyncCandidateProducerContinuationPrefixDescentGoal,
       AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier

THEOREM HistoricalCandidateProducerContinuationIngressCutClosureClosesResolution ==
  \A initialContext:
    HistoricalCandidateProducerContinuationIngressCutClosureProperty(
      AsyncLiveSpecAt(initialContext))
      => HistoricalCandidateProducerContinuationResolutionClosureProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   HistoricalCandidateProducerContinuationFrozenPrefixRankIsFinite,
   HistoricalCandidateProducerContinuationIngressCutClosureClosesFrozenPrefix,
   AsyncLiveSpecProjectsAsyncSpec,
   PTL, IsaT(600)
   DEF HistoricalCandidateProducerContinuationIngressCutClosureProperty,
       HistoricalCandidateProducerContinuationResolutionClosureProperty,
       HistoricalCandidateProducerContinuationFrozenPrefixClosureProperty,
       HistoricalCandidateProducerContinuationFrozenPrefixAtBudget,
       AsyncCandidateProducerContinuationFrozenPrefixRank

=============================================================================
