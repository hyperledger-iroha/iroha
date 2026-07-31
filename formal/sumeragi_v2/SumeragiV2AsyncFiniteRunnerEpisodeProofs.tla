---- MODULE SumeragiV2AsyncFiniteRunnerEpisodeProofs ----
EXTENDS SumeragiV2AsyncStage2Proofs,
        SumeragiV2AsyncCausalWorkBudgetProofs,
        SumeragiV2AsyncCandidateProducerContinuationProofs

(***************************************************************************
Sole finite runner-episode provider.

Stage 3, both Stage-4 leaves, and the two runner-owned plus one capacity-owned
Stage-6 leaves deliberately stop at a target-only Serve turn.  This module is
the only temporal provider for those residuals.  It does not assume their
aggregate property and it adds no fairness: the selected action is always the
ordinary `PostGstRunNode` or `PostGstServiceIoWorker` occurrence already named
by `AsyncFairnessAt`.

The outer rank is `AsyncCausalEpisodeStructuralRank`.  Its immutable shared
lifecycle cut prevents a later Candidate, causal successor, Control,
Completion, priority item, or exact retry from joining the predecessor set.
Its radix-four command weight pays for the complete lower-stage successor
batch, and its Serve tokens pay for the frozen I/O and per-source ingress
prefixes.  The inner rank is exactly the existing Ready or capacity rank.
Thus an action either reaches the caller's exact goal, consumes a finite
producer episode, or strictly lowers the occurrence rank.  Replenishment is
never called progress.
***************************************************************************)

AsyncReadyRunnerEpisodeKinds ==
  {"Stage3", "Stage4", "Stage6OwedReady", "Stage6PreAdmission"}

AsyncCapacityRunnerEpisodeKinds ==
  {"Stage4Capacity", "Stage6NonCompletionCapacity"}

AsyncReadyRunnerEpisodeResidual(kind, candidate, position, rank) ==
  CASE kind = "Stage3" ->
         Stage3ServeEpisodeResidual(candidate, position, rank)
    [] kind = "Stage4" ->
         Stage4ServeEpisodeResidual(candidate, position, rank)
    [] kind = "Stage6OwedReady" ->
         Stage6OwedReadyRunnerEpisodeResidual(
           candidate, position, rank)
    [] kind = "Stage6PreAdmission" ->
         Stage6PreAdmissionRunnerEpisodeResidual(
           candidate, position, rank)
    [] OTHER -> FALSE

AsyncReadyRunnerEpisodeGoal(kind, candidate, position, rank) ==
  CASE kind = "Stage3" ->
         Stage3AuxProgress(candidate, position, rank)
    [] kind = "Stage4" ->
         Stage4AuxProgress(candidate, position, rank)
    [] kind = "Stage6OwedReady" ->
         Stage6OwedReadyAuxProgress(candidate, position, rank)
    [] kind = "Stage6PreAdmission" ->
         Stage6PreAdmissionAuxProgress(candidate, position, rank)
    [] OTHER -> FALSE

AsyncReadyRunnerEpisodeReentry(kind, candidate, position, rank) ==
  CASE kind = "Stage3" ->
         Stage3CandidateProducerContinuationReentry(
           candidate, position, rank)
    [] kind = "Stage4" ->
         Stage4CandidateProducerContinuationReentry(
           candidate, position, rank)
    [] kind = "Stage6OwedReady" ->
         Stage6OwedReadyCandidateProducerContinuationReentry(
           candidate, position, rank)
    [] kind = "Stage6PreAdmission" ->
         Stage6PreAdmissionCandidateProducerContinuationReentry(
           candidate, position, rank)
    [] OTHER -> FALSE

AsyncCapacityRunnerEpisodeResidual(kind, candidate, position, rank) ==
  CASE kind = "Stage4Capacity" ->
         Stage4CapacityServeEpisodeResidual(
           candidate, position, rank)
    [] kind = "Stage6NonCompletionCapacity" ->
         Stage6NonCompletionCapacityServeEpisodeResidual(
           candidate, position, rank)
    [] OTHER -> FALSE

AsyncCapacityRunnerEpisodeGoal(kind, candidate, position, rank) ==
  CASE kind = "Stage4Capacity" ->
         Stage4CapacityProgress(candidate, position, rank)
    [] kind = "Stage6NonCompletionCapacity" ->
         Stage6NonCompletionCapacityProgress(
           candidate, position, rank)
    [] OTHER -> FALSE

AsyncCapacityRunnerEpisodeReentry(kind, candidate, position, rank) ==
  CASE kind = "Stage4Capacity" ->
         Stage4CapacityCandidateProducerContinuationReentry(
           candidate, position, rank)
    [] kind = "Stage6NonCompletionCapacity" ->
         Stage6NonCompletionCapacityCandidateProducerContinuationReentry(
           candidate, position, rank)
    [] OTHER -> FALSE

AsyncReadyRunnerEpisodeRankedKernel(
    kind, candidate, position, rank) ==
  /\ \/ AsyncReadyRunnerEpisodeResidual(
          kind, candidate, position, rank)
     \/ AsyncReadyRunnerEpisodeReentry(
          kind, candidate, position, rank)
  /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
       candidate.node)

AsyncCapacityRunnerEpisodeRankedKernel(
    kind, candidate, position, rank) ==
  /\ \/ AsyncCapacityRunnerEpisodeResidual(
          kind, candidate, position, rank)
     \/ AsyncCapacityRunnerEpisodeReentry(
          kind, candidate, position, rank)
  /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
       candidate.node)

AsyncReadyRunnerEpisodeRank(candidate) ==
  LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
  IN <<AsyncCausalEpisodeStructuralRank(
          candidate.node, cutoffOrdinal),
       ReadyRunAuxRank(candidate.node)>>

AsyncReadyRunnerEpisodeRankCarrier ==
  AsyncCausalEpisodeStructuralRankCarrier \X ReadyRunAuxCarrier

AsyncReadyRunnerEpisodeRankOrdering ==
  LexPairOrdering(
    AsyncCausalEpisodeStructuralRankOrdering,
    ReadyRunAuxOrdering,
    AsyncCausalEpisodeStructuralRankCarrier,
    ReadyRunAuxCarrier)

AsyncCapacityRunnerEpisodeRank(candidate) ==
  LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
  IN <<AsyncCausalEpisodeStructuralRank(
          candidate.node, cutoffOrdinal),
       Stage4CapacityRank(candidate.node)>>

AsyncCapacityRunnerEpisodeRankCarrier ==
  AsyncCausalEpisodeStructuralRankCarrier \X Stage4CapacityCarrier

AsyncCapacityRunnerEpisodeRankOrdering ==
  LexPairOrdering(
    AsyncCausalEpisodeStructuralRankOrdering,
    Stage4CapacityOrdering,
    AsyncCausalEpisodeStructuralRankCarrier,
    Stage4CapacityCarrier)

THEOREM AsyncReadyRunnerEpisodeRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    AsyncReadyRunnerEpisodeRankOrdering,
    AsyncReadyRunnerEpisodeRankCarrier)
BY AsyncCausalEpisodeStructuralRankOrderingIsWellFounded,
   ReadyRunAuxOrderingIsWellFounded, WFLexPairOrdering
   DEF AsyncReadyRunnerEpisodeRankOrdering,
       AsyncReadyRunnerEpisodeRankCarrier

THEOREM AsyncCapacityRunnerEpisodeRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    AsyncCapacityRunnerEpisodeRankOrdering,
    AsyncCapacityRunnerEpisodeRankCarrier)
BY AsyncCausalEpisodeStructuralRankOrderingIsWellFounded,
   Stage4CapacityOrderingIsWellFounded, WFLexPairOrdering
   DEF AsyncCapacityRunnerEpisodeRankOrdering,
       AsyncCapacityRunnerEpisodeRankCarrier

THEOREM AsyncReadyRunnerEpisodeRankInCarrier ==
  \A kind \in AsyncReadyRunnerEpisodeKinds,
     candidate, position, baselineRank:
    AsyncReadyRunnerEpisodeRankedKernel(
      kind, candidate, position, baselineRank)
      => AsyncReadyRunnerEpisodeRank(candidate)
           \in AsyncReadyRunnerEpisodeRankCarrier
BY AsyncCausalEpisodeStructuralRankIsFinite,
   ReadyRunAuxRankInCarrier, IsaT(300)
   DEF AsyncReadyRunnerEpisodeKinds,
       AsyncReadyRunnerEpisodeResidual,
       AsyncReadyRunnerEpisodeReentry,
       AsyncReadyRunnerEpisodeRankedKernel,
       AsyncReadyRunnerEpisodeRank,
       AsyncReadyRunnerEpisodeRankCarrier,
       Stage3ServeEpisodeResidual, Stage3KernelPending,
       Stage4ServeEpisodeResidual, ProtectedStage4Pending,
       Stage6OwedReadyRunnerEpisodeResidual,
       Stage6OwedCausalReady,
       Stage6PreAdmissionRunnerEpisodeResidual,
       ProtectedStage6Pending,
       ProtectedOwnedAtServiceRank,
       ResponsiveProtectedCandidateOwned

THEOREM AsyncCapacityRunnerEpisodeRankInCarrier ==
  \A kind \in AsyncCapacityRunnerEpisodeKinds,
     candidate, position, baselineRank:
    AsyncCapacityRunnerEpisodeRankedKernel(
      kind, candidate, position, baselineRank)
      => AsyncCapacityRunnerEpisodeRank(candidate)
           \in AsyncCapacityRunnerEpisodeRankCarrier
BY AsyncCausalEpisodeStructuralRankIsFinite,
   Stage4CapacityRankInCarrier, IsaT(300)
   DEF AsyncCapacityRunnerEpisodeKinds,
       AsyncCapacityRunnerEpisodeResidual,
       AsyncCapacityRunnerEpisodeReentry,
       AsyncCapacityRunnerEpisodeRankedKernel,
       AsyncCapacityRunnerEpisodeRank,
       AsyncCapacityRunnerEpisodeRankCarrier,
       Stage4CapacityServeEpisodeResidual,
       Stage6NonCompletionCapacityServeEpisodeResidual,
       ProtectedStage4Pending, ProtectedStage6Pending,
       ProtectedOwnedAtServiceRank,
       ResponsiveProtectedCandidateOwned

AsyncReadyRunnerEpisodeAtRank(
    kind, candidate, position, baselineRank, episodeRank) ==
  /\ AsyncReadyRunnerEpisodeRankedKernel(
       kind, candidate, position, baselineRank)
  /\ AsyncReadyRunnerEpisodeRank(candidate) = episodeRank

AsyncCapacityRunnerEpisodeAtRank(
    kind, candidate, position, baselineRank, episodeRank) ==
  /\ AsyncCapacityRunnerEpisodeRankedKernel(
       kind, candidate, position, baselineRank)
  /\ AsyncCapacityRunnerEpisodeRank(candidate) = episodeRank

AsyncReadyRunnerEpisodeRankGoal(
    kind, candidate, position, baselineRank, episodeRank) ==
  \/ AsyncReadyRunnerEpisodeGoal(
       kind, candidate, position, baselineRank)
  \/ <<AsyncReadyRunnerEpisodeRank(candidate), episodeRank>>
       \in AsyncReadyRunnerEpisodeRankOrdering

AsyncCapacityRunnerEpisodeRankGoal(
    kind, candidate, position, baselineRank, episodeRank) ==
  \/ AsyncCapacityRunnerEpisodeGoal(
       kind, candidate, position, baselineRank)
  \/ <<AsyncCapacityRunnerEpisodeRank(candidate), episodeRank>>
       \in AsyncCapacityRunnerEpisodeRankOrdering

(***************************************************************************
Exact action classification.

The existing Stage action inductions already classify every bracket step as
goal, caller-rank descent, residual, or stutter.  The only formerly unranked
residual is a producer replenishment.  The shared structural theorem turns
that case into strict outer descent.  Equality of the complete rank preserves
the same ordinary Runner/I/O owner, which lets weak fairness apply without a
new action/intersection fairness clause.
***************************************************************************)

THEOREM AsyncReadyRunnerEpisodeStepIsGoalDescentOrFrame ==
  \A kind \in AsyncReadyRunnerEpisodeKinds,
     candidate, position, baselineRank:
    /\ AsyncReadyRunnerEpisodeRankedKernel(
         kind, candidate, position, baselineRank)
    /\ [AsyncNext]_AsyncAllVars
    => \/ AsyncReadyRunnerEpisodeGoal(
            kind, candidate, position, baselineRank)'
       \/ <<AsyncReadyRunnerEpisodeRank(candidate)',
             AsyncReadyRunnerEpisodeRank(candidate)>>
            \in AsyncReadyRunnerEpisodeRankOrdering
       \/ AsyncReadyRunnerEpisodeRankedKernel(
            kind, candidate, position, baselineRank)'
            /\ AsyncReadyRunnerEpisodeRank(candidate)'
                 = AsyncReadyRunnerEpisodeRank(candidate)
BY AsyncCausalEpisodeStructuralStepIsDescentOrFrame,
   Stage3SameNodeRunAuxOutcomeObligation,
   Stage3OtherStepUnlessAuxDescentObligation,
   Stage4SameNodeRunProducesAuxOutcome,
   Stage4BlockedAuxStep,
   Stage6OwedReadySameNodeRunProducesOutcome,
   Stage6OwedReadyOtherStepPreservesOrProgresses,
   Stage6PreAdmissionSameNodeRunProducesOutcome,
   Stage6PreAdmissionOtherStepPreservesOrProgresses,
   IsaT(1800)
   DEF AsyncReadyRunnerEpisodeKinds,
       AsyncReadyRunnerEpisodeResidual,
       AsyncReadyRunnerEpisodeGoal,
       AsyncReadyRunnerEpisodeReentry,
       AsyncReadyRunnerEpisodeRankedKernel,
       Stage3CandidateProducerContinuationReentry,
       Stage4CandidateProducerContinuationReentry,
       Stage6OwedReadyCandidateProducerContinuationReentry,
       Stage6PreAdmissionCandidateProducerContinuationReentry,
       AsyncReadyRunnerEpisodeRank,
       AsyncReadyRunnerEpisodeRankOrdering,
       AsyncCausalEpisodeStructuralRankOrdering,
       ReadyRunAuxOrdering, LexPairOrdering, AsyncAllVars

THEOREM AsyncCapacityRunnerEpisodeStepIsGoalDescentOrFrame ==
  \A kind \in AsyncCapacityRunnerEpisodeKinds,
     candidate, position, baselineRank:
    /\ AsyncCapacityRunnerEpisodeRankedKernel(
         kind, candidate, position, baselineRank)
    /\ [AsyncNext]_AsyncAllVars
    => \/ AsyncCapacityRunnerEpisodeGoal(
            kind, candidate, position, baselineRank)'
       \/ <<AsyncCapacityRunnerEpisodeRank(candidate)',
             AsyncCapacityRunnerEpisodeRank(candidate)>>
            \in AsyncCapacityRunnerEpisodeRankOrdering
       \/ AsyncCapacityRunnerEpisodeRankedKernel(
            kind, candidate, position, baselineRank)'
            /\ AsyncCapacityRunnerEpisodeRank(candidate)'
                 = AsyncCapacityRunnerEpisodeRank(candidate)
BY AsyncCausalEpisodeStructuralStepIsDescentOrFrame,
   Stage4CapacitySameNodeRunProducesOutcome,
   Stage4CapacityBlockedStep,
   Stage6NonCompletionCapacitySameNodeRunProducesOutcome,
   Stage6NonCompletionCapacityOtherStepPreservesOrProgresses,
   IsaT(1800)
   DEF AsyncCapacityRunnerEpisodeKinds,
       AsyncCapacityRunnerEpisodeResidual,
       AsyncCapacityRunnerEpisodeGoal,
       AsyncCapacityRunnerEpisodeReentry,
       AsyncCapacityRunnerEpisodeRankedKernel,
       Stage4CapacityCandidateProducerContinuationReentry,
       Stage6NonCompletionCapacityCandidateProducerContinuationReentry,
       AsyncCapacityRunnerEpisodeRank,
       AsyncCapacityRunnerEpisodeRankOrdering,
       AsyncCausalEpisodeStructuralRankOrdering,
       Stage4CapacityOrdering, LexPairOrdering, AsyncAllVars

THEOREM AsyncReadyRunnerEpisodeSelectedActionConsumesRankCell ==
  \A kind \in AsyncReadyRunnerEpisodeKinds,
     candidate, position, baselineRank:
    /\ AsyncReadyRunnerEpisodeRankedKernel(
         kind, candidate, position, baselineRank)
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ <<AsyncCausalEpisodeSelectedFairAction(
           candidate.node,
           AsyncCandidateLifecycleOrdinal(candidate))>>_AsyncAllVars
    => \/ AsyncReadyRunnerEpisodeGoal(
            kind, candidate, position, baselineRank)'
       \/ <<AsyncReadyRunnerEpisodeRank(candidate)',
             AsyncReadyRunnerEpisodeRank(candidate)>>
            \in AsyncReadyRunnerEpisodeRankOrdering
BY AsyncCausalEpisodeSelectedOwnerIsConcreteAndEnabled,
   AsyncCausalEpisodeStructuralStepIsDescentOrFrame,
   Stage3SameNodeRunAuxOutcomeObligation,
   Stage4SameNodeRunProducesAuxOutcome,
   Stage6OwedReadySameNodeRunProducesOutcome,
   Stage6PreAdmissionSameNodeRunProducesOutcome,
   ServiceIoWorkerDropsQueueDepth,
   IsaT(1800)
   DEF AsyncReadyRunnerEpisodeKinds,
       AsyncReadyRunnerEpisodeResidual,
       AsyncReadyRunnerEpisodeGoal,
       AsyncReadyRunnerEpisodeReentry,
       AsyncReadyRunnerEpisodeRankedKernel,
       Stage3CandidateProducerContinuationReentry,
       Stage4CandidateProducerContinuationReentry,
       Stage6OwedReadyCandidateProducerContinuationReentry,
       Stage6PreAdmissionCandidateProducerContinuationReentry,
       AsyncReadyRunnerEpisodeRank,
       AsyncReadyRunnerEpisodeRankOrdering,
       AsyncCausalEpisodeSelectedFairAction,
       AsyncCausalEpisodeFairAction,
       AsyncCausalEpisodeFairOwner,
       AsyncCausalEpisodeIoOwnerRequired,
       LexPairOrdering, AsyncAllVars

THEOREM AsyncCapacityRunnerEpisodeSelectedActionConsumesRankCell ==
  \A kind \in AsyncCapacityRunnerEpisodeKinds,
     candidate, position, baselineRank:
    /\ AsyncCapacityRunnerEpisodeRankedKernel(
         kind, candidate, position, baselineRank)
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ <<AsyncCausalEpisodeSelectedFairAction(
           candidate.node,
           AsyncCandidateLifecycleOrdinal(candidate))>>_AsyncAllVars
    => \/ AsyncCapacityRunnerEpisodeGoal(
            kind, candidate, position, baselineRank)'
       \/ <<AsyncCapacityRunnerEpisodeRank(candidate)',
             AsyncCapacityRunnerEpisodeRank(candidate)>>
            \in AsyncCapacityRunnerEpisodeRankOrdering
BY AsyncCausalEpisodeSelectedOwnerIsConcreteAndEnabled,
   AsyncCausalEpisodeStructuralStepIsDescentOrFrame,
   Stage4CapacitySameNodeRunProducesOutcome,
   Stage6NonCompletionCapacitySameNodeRunProducesOutcome,
   ServiceIoWorkerDropsQueueDepth,
   IsaT(1800)
   DEF AsyncCapacityRunnerEpisodeKinds,
       AsyncCapacityRunnerEpisodeResidual,
       AsyncCapacityRunnerEpisodeGoal,
       AsyncCapacityRunnerEpisodeReentry,
       AsyncCapacityRunnerEpisodeRankedKernel,
       Stage4CapacityCandidateProducerContinuationReentry,
       Stage6NonCompletionCapacityCandidateProducerContinuationReentry,
       AsyncCapacityRunnerEpisodeRank,
       AsyncCapacityRunnerEpisodeRankOrdering,
       AsyncCausalEpisodeSelectedFairAction,
       AsyncCausalEpisodeFairAction,
       AsyncCausalEpisodeFairOwner,
       AsyncCausalEpisodeIoOwnerRequired,
       LexPairOrdering, AsyncAllVars

THEOREM AsyncRunnerEpisodeConcreteOwnerPersistsInRankCell ==
  \A candidate \in AsyncCandidateSet:
    LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
        owner == AsyncCausalEpisodeFairOwner(
                   candidate.node, cutoffOrdinal)
        structuralRank == AsyncCausalEpisodeStructuralRank(
                            candidate.node, cutoffOrdinal)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ ProtectedCandidateOwned(candidate)
       /\ [AsyncNext]_AsyncAllVars
       /\ ProtectedCandidateOwned(candidate)'
       /\ AsyncCausalEpisodeStructuralRank(
            candidate.node, cutoffOrdinal)' = structuralRank
       => AsyncCausalEpisodeFairOwner(
            candidate.node, cutoffOrdinal)' = owner
BY AsyncCausalEpisodeTargetLifecycleOrdinalPersists,
   AsyncCausalEpisodeFrozenOriginsCannotReplenish,
   AsyncCausalEpisodeServeCutCannotReplenish,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   IsaT(900)
   DEF AsyncCausalEpisodeFairOwner,
       AsyncCausalEpisodeIoOwnerRequired,
       AsyncCausalEpisodeStructuralRank,
       AsyncCausalEpisodeServeWorkBudget,
       AsyncCausalEpisodeServeWorkTokens,
       AsyncCausalEpisodeServeIngressIdentities,
       AsyncAllVars

THEOREM AsyncRunnerEpisodeConcreteOwnerUsesExistingFairness ==
  \A initialContext, node, ownerKind:
    /\ node \in AsyncVotersAt(initialContext)
    /\ node \in Responsive
    /\ ownerKind \in AsyncCausalEpisodeFairOwnerKinds
    => AsyncSpecAt(initialContext)
         => WF_AsyncAllVars(
              AsyncCausalEpisodeFairAction(node, ownerKind))
BY Isa, PTL
   DEF AsyncSpecAt, AsyncFairnessAt,
       AsyncCausalEpisodeFairOwnerKinds,
       AsyncCausalEpisodeFairAction

AsyncReadyRunnerEpisodeRankStepProperty(specification) ==
  specification
    => \A kind \in AsyncReadyRunnerEpisodeKinds,
          candidate, position, baselineRank,
          episodeRank \in AsyncReadyRunnerEpisodeRankCarrier:
         AsyncReadyRunnerEpisodeAtRank(
           kind, candidate, position, baselineRank, episodeRank)
           ~> AsyncReadyRunnerEpisodeRankGoal(
                kind, candidate, position, baselineRank, episodeRank)

AsyncCapacityRunnerEpisodeRankStepProperty(specification) ==
  specification
    => \A kind \in AsyncCapacityRunnerEpisodeKinds,
          candidate, position, baselineRank,
          episodeRank \in AsyncCapacityRunnerEpisodeRankCarrier:
         AsyncCapacityRunnerEpisodeAtRank(
           kind, candidate, position, baselineRank, episodeRank)
           ~> AsyncCapacityRunnerEpisodeRankGoal(
                kind, candidate, position, baselineRank, episodeRank)

THEOREM AsyncSpecProvidesReadyRunnerEpisodeRankStep ==
  \A initialContext:
    AsyncReadyRunnerEpisodeRankStepProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   AsyncCausalEpisodeSelectedOwnerIsConcreteAndEnabled,
   AsyncReadyRunnerEpisodeStepIsGoalDescentOrFrame,
   AsyncReadyRunnerEpisodeSelectedActionConsumesRankCell,
   AsyncRunnerEpisodeConcreteOwnerPersistsInRankCell,
   AsyncRunnerEpisodeConcreteOwnerUsesExistingFairness,
   PTL, IsaT(900)
   DEF AsyncReadyRunnerEpisodeRankStepProperty,
       AsyncReadyRunnerEpisodeAtRank,
       AsyncReadyRunnerEpisodeRankGoal,
       AsyncReadyRunnerEpisodeRankedKernel,
       AsyncCausalEpisodeFairOwner,
       AsyncCausalEpisodeSelectedFairAction

THEOREM AsyncSpecProvidesCapacityRunnerEpisodeRankStep ==
  \A initialContext:
    AsyncCapacityRunnerEpisodeRankStepProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   AsyncCausalEpisodeSelectedOwnerIsConcreteAndEnabled,
   AsyncCapacityRunnerEpisodeStepIsGoalDescentOrFrame,
   AsyncCapacityRunnerEpisodeSelectedActionConsumesRankCell,
   AsyncRunnerEpisodeConcreteOwnerPersistsInRankCell,
   AsyncRunnerEpisodeConcreteOwnerUsesExistingFairness,
   PTL, IsaT(900)
   DEF AsyncCapacityRunnerEpisodeRankStepProperty,
       AsyncCapacityRunnerEpisodeAtRank,
       AsyncCapacityRunnerEpisodeRankGoal,
       AsyncCapacityRunnerEpisodeRankedKernel,
       AsyncCausalEpisodeFairOwner,
       AsyncCausalEpisodeSelectedFairAction

AsyncReadyRunnerEpisodeClosureProperty(specification) ==
  specification
    => \A kind \in AsyncReadyRunnerEpisodeKinds,
          candidate, position, baselineRank:
         AsyncReadyRunnerEpisodeRankedKernel(
           kind, candidate, position, baselineRank)
           ~> AsyncReadyRunnerEpisodeGoal(
                kind, candidate, position, baselineRank)

AsyncCapacityRunnerEpisodeClosureProperty(specification) ==
  specification
    => \A kind \in AsyncCapacityRunnerEpisodeKinds,
          candidate, position, baselineRank:
         AsyncCapacityRunnerEpisodeRankedKernel(
           kind, candidate, position, baselineRank)
           ~> AsyncCapacityRunnerEpisodeGoal(
                kind, candidate, position, baselineRank)

THEOREM AsyncSpecProvidesReadyRunnerEpisodeClosure ==
  \A initialContext:
    AsyncReadyRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesReadyRunnerEpisodeRankStep,
   AsyncReadyRunnerEpisodeRankInCarrier,
   AsyncReadyRunnerEpisodeRankOrderingIsWellFounded,
   WellFoundedLeadsTo, PTL
   DEF AsyncReadyRunnerEpisodeClosureProperty,
       AsyncReadyRunnerEpisodeRankStepProperty,
       AsyncReadyRunnerEpisodeAtRank,
       AsyncReadyRunnerEpisodeRankGoal

THEOREM AsyncSpecProvidesCapacityRunnerEpisodeClosure ==
  \A initialContext:
    AsyncCapacityRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesCapacityRunnerEpisodeRankStep,
   AsyncCapacityRunnerEpisodeRankInCarrier,
   AsyncCapacityRunnerEpisodeRankOrderingIsWellFounded,
   WellFoundedLeadsTo, PTL
   DEF AsyncCapacityRunnerEpisodeClosureProperty,
       AsyncCapacityRunnerEpisodeRankStepProperty,
       AsyncCapacityRunnerEpisodeAtRank,
       AsyncCapacityRunnerEpisodeRankGoal

(***************************************************************************
Voter producer-continuation runner episode.

This scalar rank closes the continuation episode which `RunNodeWork` must
drain before an ordinary Local/Ingress/Runtime turn may be claimed.  Its
prepaid carrier includes every active continuation for the voter, so releasing
an ingress cut or changing the current view cannot expose an uncharged record.
A ready selected record strictly consumes the weighted continuation budget.
A non-ready external record is impossible under the reachable
external-coverage invariant.  A non-ready Local record either lowers the
exact two-step replay distance or materializes/consumes the selected stage.
The radix of three ensures that a stage descent pays for either replay
corridor being reset to distance two for the next immutable record.
***************************************************************************)

AsyncVoterCandidateProducerContinuationEpisodePending(node) ==
  /\ gst
  /\ node \in AsyncCurrentResponsiveVoters
  /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(node)

AsyncVoterCandidateProducerContinuationFrozenActiveRecordsForNode(node) ==
  {record \in AsyncCandidateProducerContinuations:
     /\ record.node = node
     /\ record.status \in {"Reserved", "Materialized"}}

AsyncVoterCandidateProducerContinuationFrozenPrefixBudget(node) ==
  2 * Cardinality(
        {record \in
           AsyncVoterCandidateProducerContinuationFrozenActiveRecordsForNode(
             node):
           record.status = "Reserved"})
    + Cardinality(
        {record \in
           AsyncVoterCandidateProducerContinuationFrozenActiveRecordsForNode(
             node):
           record.status = "Materialized"})

AsyncVoterCandidateProducerContinuationFrozenCompositeRank(node) ==
  3 * AsyncVoterCandidateProducerContinuationFrozenPrefixBudget(node)
    + AsyncCandidateProducerContinuationExactReplayDistance(node)

AsyncVoterCandidateProducerContinuationFrozenCompositeRankDecreases(node) ==
  AsyncVoterCandidateProducerContinuationFrozenCompositeRank(node)'
    < AsyncVoterCandidateProducerContinuationFrozenCompositeRank(node)

\* The prepaid carrier deliberately includes every active continuation owned
\* by this node, including other views and records currently behind an ingress
\* cut.  A cut release or view change can therefore only reclassify an already
\* charged record.  This set detects a truly new active identity; the writer
\* audit below proves it empty while an eligible continuation owns the
\* serialized runner.
AsyncVoterCandidateProducerContinuationFreshActiveRecordsAfter(node) ==
  {after \in AsyncCandidateProducerContinuations':
     /\ after.node = node
     /\ after.status \in {"Reserved", "Materialized"}
     /\ ~\E before \in AsyncCandidateProducerContinuations:
        before.identity = after.identity}

(***************************************************************************
No non-runner continuation insertion at the Runtime-ready boundary.

An existing continuation can become runner-eligible when this node drains its
own immutable ingress cut, and a new continuation can be installed when this
node consumes a tracked candidate.  Both transitions are `RunNodeWork(node)`.
At GST, a current responsive voter selected through the historical-runner
disjunct still satisfies the same fully framed `PostGstRunNode(node)` action;
the proof therefore classifies the semantic action rather than assuming that
the outer disjunct names are exclusive.  Every other action preserves the
absence of a runner-eligible continuation.  Pre-GST reset/replay is excluded.
***************************************************************************)
THEOREM AsyncVoterRuntimeReadyHasNoNonRunnerContinuationInsertion ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ gst
    /\ node \in AsyncCurrentResponsiveVoters
    /\ ~NodeHasDecision(node)
    /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
    /\ [AsyncNext]_AsyncAllVars
      => \/ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
               node)'
         \/ <<PostGstRunNode(node)>>_AsyncAllVars
BY AppliedNodeHasDecision,
   AsyncCandidateLifecycleDeparturesThisStepIsSingleton,
   AsyncCandidateProducerContinuationGstExcludesResetReplay,
   AsyncCurrentResponsiveVotersAreValidators,
   FS_CardinalityType, FS_Subset, IsaT(3600)
   DEF AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRunnerMayPrecedeIngress,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationLifecycleCoverageInvariant,
       AsyncCandidateProducerContinuationLifecycleCoverageInvariantIn,
       AsyncCandidateProducerContinuationLifecycleCoveredIn,
       AsyncCandidateProducerContinuations,
       AsyncCandidateProducerContinuationStateAfterDeparture,
       AsyncCandidateProducerContinuationSourceAfter,
       AsyncCandidateProducerContinuationDeparture,
       AsyncCandidateProducerContinuationReservationAvailableIn,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuationSelectedForAcknowledgement,
       AsyncCandidateProducerContinuationSelectedForRunnerResolution,
       AsyncCandidateProducerContinuationSelectedForRunnerReplay,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateLifecycleDeparturesThisStep,
       AsyncCandidateServicesThisStep,
       AsyncCandidateIgnoredWithoutApplicationThisStepSet,
       AsyncCandidateIgnoredWithoutApplicationThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncCandidatePhysicallyDiscardedThisStep,
       AsyncCandidateLifecycleStateAfterCarrierUpdate,
       AsyncCandidateLifecycleStateAfterCompaction,
       AsyncCandidateLifecycleStateAfterServeIngressAdmission,
       AsyncCandidateLifecycleStateAfterAdmission,
       AsyncCandidateLifecycleStateAfterTimeoutOwnership,
       AsyncCandidateLifecycleStateAfterServiceSlotTransfer,
       AsyncControlServiceStateAfterReset,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncCertifiedResponseClaimStateAfterRetirement,
       AsyncCertifiedResponseClaimStateAfterAdmission,
       AsyncControlServiceStateAfterTimeoutRetirement,
       AsyncControlServiceSlotTransition,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       PopSelectedIngress, DrainFairIngressSelected,
       PostGstRunNode, RunNode, RunNodeWork,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunHistoricalRecoveryNode, RunHistoricalServer,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       AsyncStrongTypeInvariant,
       AsyncControlServiceStateTypeInvariant,
       AsyncNonCrashOuterFrame, AsyncAllVars

AsyncVoterCandidateProducerContinuationEpisodeAtRank(node, rank) ==
  /\ AsyncVoterCandidateProducerContinuationEpisodePending(node)
  /\ rank \in Nat
  /\ rank =
       AsyncVoterCandidateProducerContinuationFrozenCompositeRank(node)

AsyncVoterCandidateProducerContinuationEpisodeRankGoal(node, rank) ==
  \/ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
  \/ \E lower \in SetLessThan(rank, OpToRel(<, Nat), Nat):
       AsyncVoterCandidateProducerContinuationEpisodeAtRank(node, lower)

THEOREM AsyncVoterCandidateProducerContinuationCompositeRankIsNatural ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
      => /\ AsyncCandidateProducerContinuationExactReplayDistance(node)
               \in 0..2
         /\ AsyncVoterCandidateProducerContinuationFrozenCompositeRank(node)
               \in Nat \ {0}
BY FS_CardinalityType, IsaT(600)
   DEF AsyncVoterCandidateProducerContinuationFrozenCompositeRank,
       AsyncVoterCandidateProducerContinuationFrozenPrefixBudget,
       AsyncVoterCandidateProducerContinuationFrozenActiveRecordsForNode,
       AsyncCandidateProducerContinuationExactReplayDistance,
       AsyncCandidateProducerContinuationRuntimeReplayCarrier,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerResolutionReady,
       AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncStrongTypeInvariant, AsyncTypeInvariant,
       AsyncSchedulerTypeInvariant, AsyncRuntimeTypeInvariant,
       AsyncRuntimeScalarTypeInvariant, CandidateScheduled,
       QueuedCandidates, DeferredCandidates

\* Complete writer partition for `producerContinuations` under `AsyncNext`:
\* reset is the only status-increasing writer and is excluded by GST;
\* ingress/control/certified-response/timeout/lifecycle admission preserves
\* the field; reclamation removes records or monotonically lowers status; and
\* departure is the sole insertion writer.  While the eligible voter record
\* owns `RunNodeWork`, that departure is either another node's candidate or
\* the exact stored replay identity, which coalesces against its existing
\* record.  Candidate admission alone therefore cannot mint a continuation,
\* and a serviced or tombstoned identity cannot be resurrected here.
THEOREM AsyncVoterRunnerEpisodeHasNoFreshActiveContinuationReplenishment ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncVoterCandidateProducerContinuationEpisodePending(node)
    /\ AsyncNext
      => AsyncVoterCandidateProducerContinuationFreshActiveRecordsAfter(
           node) = {}
BY AsyncCandidateProducerContinuationGstExcludesResetReplay,
   FS_CardinalityType, FS_Subset, IsaT(3600)
   DEF AsyncVoterCandidateProducerContinuationFreshActiveRecordsAfter,
       AsyncVoterCandidateProducerContinuationEpisodePending,
       AsyncCandidateProducerContinuations,
       AsyncCandidateProducerContinuationStateAfterDeparture,
       AsyncCandidateProducerContinuationSourceAfter,
       AsyncCandidateProducerContinuationDeparture,
       AsyncCandidateProducerContinuationReservationAvailableIn,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuationSelectedForAcknowledgement,
       AsyncCandidateProducerContinuationSelectedForRunnerResolution,
       AsyncCandidateProducerContinuationSelectedForRunnerReplay,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateLifecycleDeparturesThisStep,
       AsyncCandidateServicesThisStep,
       AsyncCandidateIgnoredWithoutApplicationThisStepSet,
       AsyncCandidateIgnoredWithoutApplicationThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncCandidatePhysicallyDiscardedThisStep,
       AsyncControlServiceResetNodesThisStep,
       AsyncCandidateProducerContinuationsAfterReset,
       AsyncCandidateProducerContinuationRecordAfterReset,
       AsyncControlServiceStateAfterReset,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncCertifiedResponseClaimStateAfterRetirement,
       AsyncCertifiedResponseClaimStateAfterAdmission,
       AsyncControlServiceStateAfterTimeoutRetirement,
       AsyncCandidateLifecycleStateAfterServiceSlotTransfer,
       AsyncCandidateLifecycleStateAfterCarrierUpdate,
       AsyncCandidateLifecycleStateAfterCompaction,
       AsyncCandidateLifecycleStateAfterLeaderWireAdmission,
       AsyncCandidateLifecycleStateAfterServeIngressAdmission,
       AsyncCandidateLifecycleStateAfterAdmission,
       AsyncCandidateLifecycleStateAfterTimeoutOwnership,
       AsyncControlServiceSlotTransition,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       AsyncStrongTypeInvariant, AsyncTypeInvariant,
       AsyncSchedulerTypeInvariant

THEOREM AsyncVoterRunNodeWorkStrictlyDecreasesFrozenProducerContinuationCompositeRank ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ AsyncControlServiceSlotTransition
    /\ AsyncNext
    /\ gst
    /\ node \in AsyncCurrentResponsiveVoters
    /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
    /\ RunNodeWork(node)
      => AsyncVoterCandidateProducerContinuationFrozenCompositeRankDecreases(
           node)
BY AsyncVoterRunnerEpisodeHasNoFreshActiveContinuationReplenishment,
   AsyncRunnerResolutionStrictlyConsumesFiniteProducerPrefix,
   AsyncCandidateProducerContinuationRunnerResolutionConsumesExactStage,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   AsyncCandidateProducerContinuationExactLocalReplayRetainsReservation,
   AsyncCandidateProducerContinuationStoredCarrierMakesSelectedRecordReady,
   FS_CardinalityType, FS_Subset, IsaT(3600)
   DEF AsyncVoterCandidateProducerContinuationFrozenCompositeRankDecreases,
       AsyncVoterCandidateProducerContinuationFrozenCompositeRank,
       AsyncVoterCandidateProducerContinuationFrozenPrefixBudget,
       AsyncVoterCandidateProducerContinuationFrozenActiveRecordsForNode,
       AsyncCandidateProducerContinuationExactReplayDistance,
       AsyncCandidateProducerContinuationRunnerPrefixStepOutcome,
       AsyncCandidateProducerContinuationRunnerPrefixAtBudget,
       AsyncCandidateProducerContinuationRunnerPrefixGoal,
       AsyncCandidateProducerContinuationRunnerPrefixBudget,
       AsyncCandidateProducerContinuationExternalCoverageInvariant,
       AsyncCandidateProducerContinuationDurableTerminal,
       AsyncCandidateProducerContinuationLocalReplayCapacityInvariant,
       AsyncCandidateProducerContinuationLocalReplayPrefixCapacityInvariant,
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerResolutionReady,
       AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationSelectedLocalCandidate,
       AsyncCandidateProducerContinuationSelectedRuntimeCandidate,
       AsyncCandidateProducerContinuationRuntimeReplayCarrier,
       AsyncCandidateProducerContinuationExactReplayIdentity,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncCandidateProducerContinuationSelectedForRunnerReplay,
       AsyncCandidateProducerContinuationSelectedForAcknowledgement,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuationConcreteSuccessorOwnedAfterIn,
       AsyncCandidateProducerContinuationHandoffOwnedAfterIn,
       AsyncCandidateProducerContinuationLocalReplayCarrierAfter,
       AsyncCandidateProducerContinuationHandoffRetiredAfterIn,
       AsyncCandidateProducerContinuationDeclaredHandoffRetiredAfterIn,
       AsyncCandidateProducerContinuations,
       AsyncCandidateServiceStateAfterReclamation,
       ReplayRunNodeCandidateProducerContinuation,
       ResolveRunNodeCandidateProducerContinuation,
       RunNodeWork, EnqueueCandidate,
       CandidateScheduledAfter, CandidateScheduledIn,
       QueuedCandidates, DeferredCandidates, SequenceSet,
       AsyncControlServiceSlotTransition, AsyncNext,
       SetLessThan, OpToRel

THEOREM AsyncVoterProducerContinuationCompositeRankStepCannotIncrease ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ AsyncVoterCandidateProducerContinuationEpisodePending(node)
    /\ [AsyncNext]_AsyncAllVars
      => \/ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(node)'
         \/ AsyncVoterCandidateProducerContinuationFrozenCompositeRank(node)'
              <
                AsyncVoterCandidateProducerContinuationFrozenCompositeRank(
                  node)
         \/ /\ AsyncVoterCandidateProducerContinuationEpisodePending(node)'
            /\ AsyncVoterCandidateProducerContinuationFrozenCompositeRank(
                 node)'
                 =
                   AsyncVoterCandidateProducerContinuationFrozenCompositeRank(
                     node)
BY AsyncVoterRunNodeWorkStrictlyDecreasesFrozenProducerContinuationCompositeRank,
   AsyncVoterRunnerEpisodeHasNoFreshActiveContinuationReplenishment,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   FS_CardinalityType, FS_Subset, IsaT(3600)
   DEF AsyncVoterCandidateProducerContinuationEpisodePending,
       AsyncVoterCandidateProducerContinuationFrozenCompositeRank,
       AsyncVoterCandidateProducerContinuationFrozenPrefixBudget,
       AsyncVoterCandidateProducerContinuationFrozenActiveRecordsForNode,
       AsyncVoterCandidateProducerContinuationFreshActiveRecordsAfter,
       AsyncCandidateProducerContinuationExactReplayDistance,
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerResolutionReady,
       AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord,
       AsyncCandidateProducerContinuationRuntimeReplayCarrier,
       AsyncCandidateProducerContinuationExternalCoverageInvariant,
       AsyncCandidateProducerContinuationDurableTerminal,
       AsyncCandidateProducerContinuationLocalReplayCapacityInvariant,
       AsyncCandidateProducerContinuationLocalReplayPrefixCapacityInvariant,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuations,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncControlServiceSlotTransition,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       AsyncAllVars

THEOREM AsyncVoterProducerContinuationFairRunnerIsEnabled ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ AsyncVoterCandidateProducerContinuationEpisodePending(node)
      => ENABLED PostGstRunNode(node)
BY ResponsiveUnappliedRunNodeIsEnabled,
   EnabledRunNodeLiftsPostGst,
   AsyncStrongTypeProjectsAsyncType,
   GstResponsiveNodesAreUp,
   GstExcludesResponsiveReplayQuarantine,
   IsaT(600)
   DEF AsyncVoterCandidateProducerContinuationEpisodePending,
       AsyncStrongTypeInvariant, AsyncCurrentResponsiveVoters,
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
       RecoveryRunNodeGuard

AsyncVoterCandidateProducerContinuationRankStepProperty(
    specification, initialContext) ==
  specification
    => \A node \in AsyncVotersAt(initialContext), rank \in Nat:
         AsyncVoterCandidateProducerContinuationEpisodeAtRank(node, rank)
           ~> AsyncVoterCandidateProducerContinuationEpisodeRankGoal(
                node, rank)

AsyncVoterCandidateProducerContinuationResolutionClosureProperty(
    specification, initialContext) ==
  specification
    => \A node \in AsyncVotersAt(initialContext):
         AsyncVoterCandidateProducerContinuationEpisodePending(node)
           ~>
             ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
                node)

THEOREM AsyncSpecProvidesVoterCandidateProducerContinuationRankStep ==
  \A initialContext:
    AsyncVoterCandidateProducerContinuationRankStepProperty(
      AsyncSpecAt(initialContext), initialContext)
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   AsyncVoterProducerContinuationCompositeRankStepCannotIncrease,
   AsyncVoterProducerContinuationFairRunnerIsEnabled,
   AsyncVoterRunNodeWorkStrictlyDecreasesFrozenProducerContinuationCompositeRank,
   WF1, PTL, IsaT(1800)
   DEF AsyncVoterCandidateProducerContinuationRankStepProperty,
       AsyncVoterCandidateProducerContinuationEpisodeAtRank,
       AsyncVoterCandidateProducerContinuationEpisodeRankGoal,
       AsyncVoterCandidateProducerContinuationEpisodePending,
       AsyncVoterCandidateProducerContinuationFrozenCompositeRankDecreases,
       PostGstRunNode, RunNode,
       AsyncSpecAt, AsyncFairnessAt,
       SetLessThan, OpToRel

THEOREM AsyncSpecProvidesVoterCandidateProducerContinuationResolutionClosure ==
  \A initialContext:
    AsyncVoterCandidateProducerContinuationResolutionClosureProperty(
      AsyncSpecAt(initialContext), initialContext)
BY AsyncSpecProvidesVoterCandidateProducerContinuationRankStep,
   AsyncVoterCandidateProducerContinuationCompositeRankIsNatural,
   NatLessThanWellFounded, WellFoundedLeadsTo, PTL, Isa
   DEF AsyncVoterCandidateProducerContinuationResolutionClosureProperty,
       AsyncVoterCandidateProducerContinuationRankStepProperty,
       AsyncVoterCandidateProducerContinuationEpisodeAtRank,
       AsyncVoterCandidateProducerContinuationEpisodeRankGoal,
       SetLessThan, OpToRel

(***************************************************************************
Height-facing Local/Ingress phase corridor.

Clearing a continuation is not by itself enough for a caller whose protected
rank is framed at a Local or Ingress runner phase.  The only permitted Runtime
detour is the exact stored-candidate replay cell at distance one.  Its next
same-node runner turn consumes that exact carrier and returns to Local; no
timeout, ingress, or later candidate is allowed to use the detour.
***************************************************************************)

AsyncVoterCandidateProducerContinuationLocalIngressResolutionGoal(node) ==
  \/ NodeHasDecision(node)
  \/ /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
     /\ asyncRunnerPhase[node] \in {"Local", "Ingress"}

AsyncVoterCandidateProducerContinuationLocalIngressCorridorState(node) ==
  /\ AsyncVoterCandidateProducerContinuationEpisodePending(node)
  /\ \/ asyncRunnerPhase[node] \in {"Local", "Ingress"}
     \/ /\ asyncRunnerPhase[node] = "Runtime"
        /\ AsyncCandidateProducerContinuationExactReplayDistance(node) = 1

THEOREM AsyncVoterCandidateProducerContinuationRuntimeDetourReturnsLocal ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ AsyncVoterCandidateProducerContinuationEpisodePending(node)
    /\ asyncRunnerPhase[node] = "Runtime"
    /\ AsyncCandidateProducerContinuationExactReplayDistance(node) = 1
    /\ RunNodeWork(node)
      => asyncRunnerPhase'[node] = "Local"
BY AsyncCandidateProducerContinuationRunnerSelectionIsGlobalMinimum,
   AsyncCandidateProducerContinuationReplayDispatchesOnlyExactIdentity,
   AsyncCandidateProducerContinuationExactLocalReplayRetainsReservation,
   AsyncCandidateProducerContinuationStoredCarrierMakesSelectedRecordReady,
   IsaT(1800)
   DEF AsyncVoterCandidateProducerContinuationEpisodePending,
       AsyncCandidateProducerContinuationExactReplayDistance,
       AsyncCandidateProducerContinuationExternalCoverageInvariant,
       AsyncCandidateProducerContinuationDurableTerminal,
       AsyncCandidateProducerContinuationLocalReplayCapacityInvariant,
       AsyncCandidateProducerContinuationLocalReplayPrefixCapacityInvariant,
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerResolutionReady,
       AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationRuntimeReplayCarrier,
       AsyncCandidateProducerContinuationExactReplayIdentity,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       ReplayRunNodeCandidateProducerContinuation,
       ResolveRunNodeCandidateProducerContinuation,
       RunNodeWork, EnqueueCandidate,
       CandidateScheduled, CandidateScheduledIn,
       QueuedCandidates, DeferredCandidates, SequenceSet

THEOREM AsyncVoterCandidateProducerContinuationLocalIngressCorridorStep ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ AsyncVoterCandidateProducerContinuationLocalIngressCorridorState(node)
    /\ [AsyncNext]_AsyncAllVars
      =>
        \/ AsyncVoterCandidateProducerContinuationLocalIngressResolutionGoal(
             node)'
        \/ AsyncVoterCandidateProducerContinuationLocalIngressCorridorState(
             node)'
BY AsyncVoterCandidateProducerContinuationRuntimeDetourReturnsLocal,
   AsyncVoterRunnerEpisodeHasNoFreshActiveContinuationReplenishment,
   AsyncCandidateProducerContinuationExactLocalReplayRetainsReservation,
   AsyncCandidateProducerContinuationStoredCarrierMakesSelectedRecordReady,
   AsyncStrongTypeProjectsAsyncType,
   FS_CardinalityType, FS_Subset, IsaT(3600)
   DEF AsyncVoterCandidateProducerContinuationLocalIngressResolutionGoal,
       AsyncVoterCandidateProducerContinuationLocalIngressCorridorState,
       AsyncVoterCandidateProducerContinuationEpisodePending,
       AsyncCandidateProducerContinuationExactReplayDistance,
       AsyncCandidateProducerContinuationExternalCoverageInvariant,
       AsyncCandidateProducerContinuationDurableTerminal,
       AsyncCandidateProducerContinuationLocalReplayCapacityInvariant,
       AsyncCandidateProducerContinuationLocalReplayPrefixCapacityInvariant,
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerResolutionReady,
       AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationSelectedLocalCandidate,
       AsyncCandidateProducerContinuationSelectedRuntimeCandidate,
       AsyncCandidateProducerContinuationRuntimeReplayCarrier,
       AsyncCandidateProducerContinuationExactReplayIdentity,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncCandidateProducerContinuationSelectedForRunnerReplay,
       AsyncCandidateProducerContinuationSelectedForAcknowledgement,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuationConcreteSuccessorOwnedAfterIn,
       AsyncCandidateProducerContinuationHandoffOwnedAfterIn,
       AsyncCandidateProducerContinuationLocalReplayCarrierAfter,
       AsyncCandidateProducerContinuationHandoffRetiredAfterIn,
       AsyncCandidateProducerContinuationDeclaredHandoffRetiredAfterIn,
       AsyncCandidateProducerContinuations,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncControlServiceSlotTransition,
       ReplayRunNodeCandidateProducerContinuation,
       ResolveRunNodeCandidateProducerContinuation,
       RunNodeWork, EnqueueCandidate,
       CandidateScheduled, CandidateScheduledAfter, CandidateScheduledIn,
       QueuedCandidates, DeferredCandidates, SequenceSet,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       AsyncAllVars

AsyncVoterCandidateProducerContinuationLocalIngressResolutionCorridorProperty(
    specification, initialContext) ==
  specification
    => \A node \in AsyncVotersAt(initialContext):
         /\ AsyncVoterCandidateProducerContinuationEpisodePending(node)
         /\ asyncRunnerPhase[node] \in {"Local", "Ingress"}
           ~>
             AsyncVoterCandidateProducerContinuationLocalIngressResolutionGoal(
               node)

THEOREM AsyncSpecProvidesVoterCandidateProducerContinuationLocalIngressResolutionCorridor ==
  \A initialContext:
    AsyncVoterCandidateProducerContinuationLocalIngressResolutionCorridorProperty(
      AsyncSpecAt(initialContext), initialContext)
BY AsyncSpecProvidesVoterCandidateProducerContinuationResolutionClosure,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   AsyncVoterCandidateProducerContinuationLocalIngressCorridorStep,
   PTL, IsaT(1200)
   DEF AsyncVoterCandidateProducerContinuationLocalIngressResolutionCorridorProperty,
       AsyncVoterCandidateProducerContinuationResolutionClosureProperty,
       AsyncVoterCandidateProducerContinuationLocalIngressResolutionGoal,
       AsyncVoterCandidateProducerContinuationLocalIngressCorridorState

(***************************************************************************
Protected caller composition.

The six protected leaves use their existing structural/auxiliary occurrence
rank only while no producer continuation owns the serialized runner.  A
continuation residual is instead a target-framed corridor into the scalar
episode above.  Clearing that finite episode returns the unchanged protected
target to the ordinary ranked kernel; the clearance itself is not advertised
as Stage progress.
***************************************************************************)

AsyncReadyRunnerEpisodeContinuationResidual(
    kind, candidate, position, rank) ==
  /\ AsyncReadyRunnerEpisodeResidual(
       kind, candidate, position, rank)
  /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(
       candidate.node)

AsyncCapacityRunnerEpisodeContinuationResidual(
    kind, candidate, position, rank) ==
  /\ AsyncCapacityRunnerEpisodeResidual(
       kind, candidate, position, rank)
  /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(
       candidate.node)

THEOREM AsyncReadyRunnerEpisodeContinuationOwnsVoterEpisode ==
  \A kind \in AsyncReadyRunnerEpisodeKinds,
     candidate, position, rank:
    AsyncReadyRunnerEpisodeContinuationResidual(
      kind, candidate, position, rank)
      => AsyncVoterCandidateProducerContinuationEpisodePending(
           candidate.node)
BY IsaT(600)
   DEF AsyncReadyRunnerEpisodeKinds,
       AsyncReadyRunnerEpisodeContinuationResidual,
       AsyncReadyRunnerEpisodeResidual,
       AsyncVoterCandidateProducerContinuationEpisodePending,
       Stage3ServeEpisodeResidual, Stage3KernelPending,
       Stage4ServeEpisodeResidual, ProtectedStage4Pending,
       Stage6OwedReadyRunnerEpisodeResidual,
       Stage6OwedCausalReady,
       Stage6PreAdmissionRunnerEpisodeResidual,
       ProtectedStage6Pending, ProtectedOwnedAtServiceRank,
       ResponsiveProtectedCandidateOwned,
       AsyncCurrentResponsiveVoters

THEOREM AsyncCapacityRunnerEpisodeContinuationOwnsVoterEpisode ==
  \A kind \in AsyncCapacityRunnerEpisodeKinds,
     candidate, position, rank:
    AsyncCapacityRunnerEpisodeContinuationResidual(
      kind, candidate, position, rank)
      => AsyncVoterCandidateProducerContinuationEpisodePending(
           candidate.node)
BY IsaT(600)
   DEF AsyncCapacityRunnerEpisodeKinds,
       AsyncCapacityRunnerEpisodeContinuationResidual,
       AsyncCapacityRunnerEpisodeResidual,
       AsyncVoterCandidateProducerContinuationEpisodePending,
       Stage4CapacityServeEpisodeResidual,
       Stage6NonCompletionCapacityServeEpisodeResidual,
       ProtectedStage4Pending, ProtectedStage6Pending,
       ProtectedOwnedAtServiceRank,
       ResponsiveProtectedCandidateOwned,
       AsyncCurrentResponsiveVoters

THEOREM AsyncReadyRunnerEpisodeContinuationStepFramesTarget ==
  \A kind \in AsyncReadyRunnerEpisodeKinds,
     candidate, position, rank:
    /\ AsyncReadyRunnerEpisodeContinuationResidual(
         kind, candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
      => \/ AsyncReadyRunnerEpisodeGoal(
               kind, candidate, position, rank)'
         \/ AsyncReadyRunnerEpisodeContinuationResidual(
               kind, candidate, position, rank)'
         \/ AsyncReadyRunnerEpisodeRankedKernel(
               kind, candidate, position, rank)'
BY Stage3SameNodeRunAuxOutcomeObligation,
   Stage3OtherStepUnlessAuxDescentObligation,
   Stage4SameNodeRunProducesAuxOutcome,
   Stage4BlockedAuxStep,
   Stage6OwedReadySameNodeRunProducesOutcome,
   Stage6OwedReadyOtherStepPreservesOrProgresses,
   Stage6PreAdmissionSameNodeRunProducesOutcome,
   Stage6PreAdmissionOtherStepPreservesOrProgresses,
   IsaT(1800)
   DEF AsyncReadyRunnerEpisodeKinds,
       AsyncReadyRunnerEpisodeContinuationResidual,
       AsyncReadyRunnerEpisodeRankedKernel,
       AsyncReadyRunnerEpisodeResidual,
       AsyncReadyRunnerEpisodeGoal,
       AsyncReadyRunnerEpisodeReentry,
       Stage3CandidateProducerContinuationReentry,
       Stage4CandidateProducerContinuationReentry,
       Stage6OwedReadyCandidateProducerContinuationReentry,
       Stage6PreAdmissionCandidateProducerContinuationReentry,
       AsyncAllVars

THEOREM AsyncCapacityRunnerEpisodeContinuationStepFramesTarget ==
  \A kind \in AsyncCapacityRunnerEpisodeKinds,
     candidate, position, rank:
    /\ AsyncCapacityRunnerEpisodeContinuationResidual(
         kind, candidate, position, rank)
    /\ [AsyncNext]_AsyncAllVars
      => \/ AsyncCapacityRunnerEpisodeGoal(
               kind, candidate, position, rank)'
         \/ AsyncCapacityRunnerEpisodeContinuationResidual(
               kind, candidate, position, rank)'
         \/ AsyncCapacityRunnerEpisodeRankedKernel(
               kind, candidate, position, rank)'
BY Stage4CapacitySameNodeRunProducesOutcome,
   Stage4CapacityBlockedStep,
   Stage6NonCompletionCapacitySameNodeRunProducesOutcome,
   Stage6NonCompletionCapacityOtherStepPreservesOrProgresses,
   IsaT(1800)
   DEF AsyncCapacityRunnerEpisodeKinds,
       AsyncCapacityRunnerEpisodeContinuationResidual,
       AsyncCapacityRunnerEpisodeRankedKernel,
       AsyncCapacityRunnerEpisodeResidual,
       AsyncCapacityRunnerEpisodeGoal,
       AsyncCapacityRunnerEpisodeReentry,
       Stage4CapacityCandidateProducerContinuationReentry,
       Stage6NonCompletionCapacityCandidateProducerContinuationReentry,
       AsyncAllVars

AsyncReadyRunnerEpisodeContinuationClosureProperty(
    specification, initialContext) ==
  specification
    => \A kind \in AsyncReadyRunnerEpisodeKinds,
          candidate, position, rank:
         AsyncReadyRunnerEpisodeContinuationResidual(
           kind, candidate, position, rank)
           ~> (AsyncReadyRunnerEpisodeGoal(
                 kind, candidate, position, rank)
                \/ AsyncReadyRunnerEpisodeRankedKernel(
                     kind, candidate, position, rank))

AsyncCapacityRunnerEpisodeContinuationClosureProperty(
    specification, initialContext) ==
  specification
    => \A kind \in AsyncCapacityRunnerEpisodeKinds,
          candidate, position, rank:
         AsyncCapacityRunnerEpisodeContinuationResidual(
           kind, candidate, position, rank)
           ~> (AsyncCapacityRunnerEpisodeGoal(
                 kind, candidate, position, rank)
                \/ AsyncCapacityRunnerEpisodeRankedKernel(
                     kind, candidate, position, rank))

THEOREM AsyncSpecProvidesReadyRunnerEpisodeContinuationClosure ==
  \A initialContext:
    AsyncReadyRunnerEpisodeContinuationClosureProperty(
      AsyncSpecAt(initialContext), initialContext)
BY AsyncSpecProvidesVoterCandidateProducerContinuationResolutionClosure,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   AsyncReadyRunnerEpisodeContinuationOwnsVoterEpisode,
   AsyncReadyRunnerEpisodeContinuationStepFramesTarget,
   PTL, IsaT(1200)
   DEF AsyncReadyRunnerEpisodeContinuationClosureProperty,
       AsyncVoterCandidateProducerContinuationResolutionClosureProperty

THEOREM AsyncSpecProvidesCapacityRunnerEpisodeContinuationClosure ==
  \A initialContext:
    AsyncCapacityRunnerEpisodeContinuationClosureProperty(
      AsyncSpecAt(initialContext), initialContext)
BY AsyncSpecProvidesVoterCandidateProducerContinuationResolutionClosure,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   AsyncCapacityRunnerEpisodeContinuationOwnsVoterEpisode,
   AsyncCapacityRunnerEpisodeContinuationStepFramesTarget,
   PTL, IsaT(1200)
   DEF AsyncCapacityRunnerEpisodeContinuationClosureProperty,
       AsyncVoterCandidateProducerContinuationResolutionClosureProperty

AsyncReadyRunnerEpisodeCompleteClosureProperty(specification) ==
  specification
    => \A kind \in AsyncReadyRunnerEpisodeKinds,
          candidate, position, rank:
         AsyncReadyRunnerEpisodeResidual(
           kind, candidate, position, rank)
           ~> AsyncReadyRunnerEpisodeGoal(
                kind, candidate, position, rank)

AsyncCapacityRunnerEpisodeCompleteClosureProperty(specification) ==
  specification
    => \A kind \in AsyncCapacityRunnerEpisodeKinds,
          candidate, position, rank:
         AsyncCapacityRunnerEpisodeResidual(
           kind, candidate, position, rank)
           ~> AsyncCapacityRunnerEpisodeGoal(
                kind, candidate, position, rank)

THEOREM AsyncSpecProvidesReadyRunnerEpisodeCompleteClosure ==
  \A initialContext:
    AsyncReadyRunnerEpisodeCompleteClosureProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesReadyRunnerEpisodeClosure,
   AsyncSpecProvidesReadyRunnerEpisodeContinuationClosure,
   PTL, Isa
   DEF AsyncReadyRunnerEpisodeCompleteClosureProperty,
       AsyncReadyRunnerEpisodeClosureProperty,
       AsyncReadyRunnerEpisodeContinuationClosureProperty,
       AsyncReadyRunnerEpisodeContinuationResidual,
       AsyncReadyRunnerEpisodeRankedKernel

THEOREM AsyncSpecProvidesCapacityRunnerEpisodeCompleteClosure ==
  \A initialContext:
    AsyncCapacityRunnerEpisodeCompleteClosureProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesCapacityRunnerEpisodeClosure,
   AsyncSpecProvidesCapacityRunnerEpisodeContinuationClosure,
   PTL, Isa
   DEF AsyncCapacityRunnerEpisodeCompleteClosureProperty,
       AsyncCapacityRunnerEpisodeClosureProperty,
       AsyncCapacityRunnerEpisodeContinuationClosureProperty,
       AsyncCapacityRunnerEpisodeContinuationResidual,
       AsyncCapacityRunnerEpisodeRankedKernel

(***************************************************************************
Exact leaf discharge and aggregate provider.

Every projection consumes the complete closure, never the intermediate
continuation hand-back.  The aggregate theorem remains unconditional and
does not assume the property it supplies.
***************************************************************************)

THEOREM AsyncSpecProvidesStage3FiniteServeEpisodeResidual ==
  \A initialContext:
    Stage3FiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesReadyRunnerEpisodeCompleteClosure, PTL
   DEF AsyncReadyRunnerEpisodeCompleteClosureProperty,
       AsyncReadyRunnerEpisodeKinds,
       AsyncReadyRunnerEpisodeResidual,
       AsyncReadyRunnerEpisodeGoal,
       Stage3FiniteServeEpisodeResidualProperty

THEOREM AsyncSpecProvidesStage4FiniteServeEpisodeResidual ==
  \A initialContext:
    Stage4FiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesReadyRunnerEpisodeCompleteClosure, PTL
   DEF AsyncReadyRunnerEpisodeCompleteClosureProperty,
       AsyncReadyRunnerEpisodeKinds,
       AsyncReadyRunnerEpisodeResidual,
       AsyncReadyRunnerEpisodeGoal,
       Stage4FiniteServeEpisodeResidualProperty

THEOREM AsyncSpecProvidesStage4CapacityFiniteServeEpisodeResidual ==
  \A initialContext:
    Stage4CapacityFiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesCapacityRunnerEpisodeCompleteClosure, PTL
   DEF AsyncCapacityRunnerEpisodeCompleteClosureProperty,
       AsyncCapacityRunnerEpisodeKinds,
       AsyncCapacityRunnerEpisodeResidual,
       AsyncCapacityRunnerEpisodeGoal,
       Stage4CapacityFiniteServeEpisodeResidualProperty

THEOREM AsyncSpecProvidesStage6NonCompletionFiniteServeEpisodeResidual ==
  \A initialContext:
    Stage6NonCompletionCapacityFiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesCapacityRunnerEpisodeCompleteClosure, PTL
   DEF AsyncCapacityRunnerEpisodeCompleteClosureProperty,
       AsyncCapacityRunnerEpisodeKinds,
       AsyncCapacityRunnerEpisodeResidual,
       AsyncCapacityRunnerEpisodeGoal,
       Stage6NonCompletionCapacityFiniteServeEpisodeResidualProperty

THEOREM AsyncSpecProvidesStage6OwedReadyFiniteRunnerEpisodeResidual ==
  \A initialContext:
    Stage6OwedReadyFiniteRunnerEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesReadyRunnerEpisodeCompleteClosure, PTL
   DEF AsyncReadyRunnerEpisodeCompleteClosureProperty,
       AsyncReadyRunnerEpisodeKinds,
       AsyncReadyRunnerEpisodeResidual,
       AsyncReadyRunnerEpisodeGoal,
       Stage6OwedReadyFiniteRunnerEpisodeResidualProperty

THEOREM AsyncSpecProvidesStage6PreAdmissionFiniteRunnerEpisodeResidual ==
  \A initialContext:
    Stage6PreAdmissionFiniteRunnerEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesReadyRunnerEpisodeCompleteClosure, PTL
   DEF AsyncReadyRunnerEpisodeCompleteClosureProperty,
       AsyncReadyRunnerEpisodeKinds,
       AsyncReadyRunnerEpisodeResidual,
       AsyncReadyRunnerEpisodeGoal,
       Stage6PreAdmissionFiniteRunnerEpisodeResidualProperty

THEOREM AsyncSpecProvidesProtectedServiceFiniteRunnerEpisodeClosure ==
  \A initialContext:
    ProtectedServiceFiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesStage3FiniteServeEpisodeResidual,
   AsyncSpecProvidesStage4FiniteServeEpisodeResidual,
   AsyncSpecProvidesStage4CapacityFiniteServeEpisodeResidual,
   AsyncSpecProvidesStage6NonCompletionFiniteServeEpisodeResidual,
   AsyncSpecProvidesStage6OwedReadyFiniteRunnerEpisodeResidual,
   AsyncSpecProvidesStage6PreAdmissionFiniteRunnerEpisodeResidual
   DEF ProtectedServiceFiniteRunnerEpisodeClosureProperty,
       Stage6FiniteRunnerEpisodeClosureProperty,
       Stage4RefinementFiniteServeEpisodeResidualProperty

(***************************************************************************
Upstream candidate-tombstone lifecycle provider.

The finite runner and historical continuation modules cannot cite the
downstream adequate-leader closure without creating an import cycle.  Keep
the same init/preservation argument at this layer under distinct names.  The
downstream theorem may continue to package the invariant for its own callers.
***************************************************************************)

THEOREM AsyncFiniteRunnerTerminalRetirementEligibilityIsRestartSafe ==
  \A candidate:
    AsyncCandidateTerminalRetirementEligibleAfterStep(candidate)
      => candidate.kind \notin AsyncRestartScopedCandidateServiceKinds
BY DEF AsyncCandidateTerminalRetirementEligibleAfterStep

THEOREM AsyncFiniteRunnerTerminalTombstoneConstructorPreservesRestartSafety ==
  \A candidate, episodeView, ordinal:
    candidate.kind \notin AsyncRestartScopedCandidateServiceKinds
      => AsyncCandidateServiceTombstone(
           candidate, episodeView, ordinal).phase
             \notin AsyncRestartScopedCandidateServiceKinds
BY DEF AsyncCandidateServiceTombstone

THEOREM AsyncFiniteRunnerInitEstablishesCandidateServiceTombstoneLifecycle ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => AsyncCandidateServiceTombstoneLifecycleInvariant
BY AsyncInitEstablishesLeaderWireContinuationSharedOrdinalNoCollision,
   Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       AsyncRuntimeInit, AsyncIoInit, AsyncDeferredInit,
       AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncLeaderWireContinuationSharedOrdinalNoCollisionInvariant,
       AsyncCandidateProducerSemanticHandoffCoverageInvariant,
       AsyncCandidateLifecycleAdmissions,
       AsyncInitialCandidateLifecycleAdmissions,
       AsyncCandidateLifecycleAdmission,
       AsyncCandidateLifecycleStageIdentityInvariant,
       AsyncCandidateScheduledLifecycleStageIdentityInvariant,
       AsyncCandidateRecordedLifecycleStageIdentityInvariant,
       AsyncControlServiceStateTypeInvariant,
       AsyncCandidateServiceTombstones,
       AsyncCandidateServiceRecordsFor,
       AsyncCandidateServiceRecordsForIdentity,
       QueuedCandidates, DeferredCandidates,
       CausalCandidates, TrackedWorkCandidates,
       SequenceSet

THEOREM AsyncFiniteRunnerNextPreservesCandidateServiceTombstoneLifecycle ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncCandidateServiceTombstoneLifecycleInvariant
  /\ AsyncNext
  => AsyncCandidateServiceTombstoneLifecycleInvariant'
BY AsyncNextPreservesControlServiceStateTypeInvariant,
   AsyncNextPreservesLeaderWireContinuationSharedOrdinalNoCollision,
   AsyncControlServiceTransitionPreservesSemanticHandoffCoverage,
   AsyncCandidateServicesThisStepIsSingleton,
   AsyncCandidateTerminalRetirementsThisStepIsSingleton,
   AsyncFiniteRunnerTerminalRetirementEligibilityIsRestartSafe,
   AsyncFiniteRunnerTerminalTombstoneConstructorPreservesRestartSafety,
   AsyncCandidateSuccessfulServiceInstallsTombstone,
   AsyncCandidateDiscardInstallsTerminalTombstone,
   AsyncCandidateCausalAdmissionTransfersSameOwner,
   AsyncCandidateIoCompletionTransfersSameOwner,
   AsyncCandidateProducerCompletionTransfersSameOwner,
   AsyncCandidateBusyDeferralTransfersSameOwner,
   AsyncCandidateDeferredHandoffRetainsSameOwner,
   AsyncCandidateDiscardIsNotSemanticService,
   AsyncCandidateServiceTombstoneCoalescesFreshCandidate,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateSameHeightRestartPreservesServicedIdentity,
   IsaT(1800)
   DEF AsyncCandidateServiceTombstoneLifecycleInvariant,
       AsyncCandidateServiceLifecycleInvariant,
       AsyncLeaderWireContinuationSharedOrdinalNoCollisionInvariant,
       AsyncLeaderWireRuntimeCandidate,
       AsyncLeaderWireLifecycleTransition,
       AsyncLeaderWireLifecycleIngressAdmissionTransition,
       AsyncLeaderWireLifecycleIngressDrainTransition,
       AsyncLeaderWireLifecycleConsumerTransition,
       AsyncLeaderWireLifecycleTerminalTransition,
       AsyncLeaderWireLifecycleRestartTransition,
       AsyncCandidateProducerContinuations,
       AsyncCandidateProducerContinuationStateAfterDeparture,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateServiceIdentity,
       AsyncCandidateLifecycleStageIdentityInvariant,
       AsyncCandidateScheduledLifecycleStageIdentityInvariant,
       AsyncCandidateRecordedLifecycleStageIdentityInvariant,
       AsyncStrongTypeInvariant,
       AsyncProgressOwnershipInvariant,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncCandidateProducerContinuationExactReplayIdentity,
       AsyncCandidateProducerContinuationSelectedLocalCandidate,
       AsyncCandidateProducerContinuationSelectedRuntimeCandidate,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationSelectedResolutionRecord,
       AsyncCandidateProducerContinuationResolutionRequired,
       AsyncCandidateProducerContinuationResolutionReady,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncSchedulerExceptCausalControlAndNodeService,
       AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
       AsyncSchedulerExceptCausalControlRunnerAndNodeService,
       LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep, RuntimeStep,
       DrainFairIngressSelected, AdmitCausalHead,
       AdmitProducerCompletion, ServiceIoWorkerWork,
       FifoRuntimeStep, DeferredDrainStep,
       AppendCausalSuccessors, FreshCommandSuccessors,
       AsyncCandidateTerminalRetirementsThisStep,
       AsyncCandidateTerminalDiscardsThisStep,
       AsyncCandidateTerminallyDiscardedThisStep,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateTerminalRetirementEligibleAfterStep,
       AsyncCandidateServiceTombstone,
       FreshCandidateSequence, CandidateAdmissionCoalesced,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       DriveResponsiveReplayHead, FinishResponsiveReplay,
       PreGstResponsiveReplay, ResetNodeSchedulerForRestart,
       FreshRestartCandidateSequence,
       CandidateScheduled, CandidateScheduledAfter

THEOREM AsyncFiniteRunnerSpecAlwaysCandidateServiceTombstoneLifecycle ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []AsyncCandidateServiceTombstoneLifecycleInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext)
         PROVE []AsyncCandidateServiceTombstoneLifecycleInvariant
    <2>1. AsyncInitAt(initialContext)
             => AsyncCandidateServiceTombstoneLifecycleInvariant
      BY AsyncFiniteRunnerInitEstablishesCandidateServiceTombstoneLifecycle
    <2>2. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>3. []AsyncProgressOwnershipInvariant
      BY <1>1, AsyncSpecAlwaysProgressOwnershipInvariant
    <2>4. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ AsyncCandidateServiceTombstoneLifecycleInvariant
           /\ [AsyncNext]_AsyncAllVars
          => AsyncCandidateServiceTombstoneLifecycleInvariant'
      BY AsyncFiniteRunnerNextPreservesCandidateServiceTombstoneLifecycle,
         Isa
         DEF AsyncAllVars
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, PTL
         DEF AsyncSpecAt
  <1> QED BY <1>1

THEOREM AsyncFiniteRunnerInitEstablishesCertifiedResponseClaimFrozenSource ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => AsyncCertifiedResponseClaimFrozenSourceInvariant
BY FS_EmptySet, Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       AsyncCertifiedResponseClaimFrozenSourceInvariant,
       AsyncCertifiedResponseClaimRecords,
       AsyncCertifiedResponseClaimRecord

THEOREM AsyncFiniteRunnerNextPreservesCertifiedResponseClaimFrozenSource ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncCandidateServiceLifecycleInvariant
  /\ AsyncCertifiedResponseClaimFrozenSourceInvariant
  /\ [AsyncNext]_AsyncAllVars
  => AsyncCertifiedResponseClaimFrozenSourceInvariant'
BY CertifiedResponseClaimAdmissionFreezesCompletePredecessorSources,
   CertifiedResponseLiveClaimCannotBeReplacedAtGst,
   CertifiedResponseClaimNewTimeoutSourceIsExcludedOrAboveFrozenCeiling,
   AsyncTimeoutLifecycleNewOwnershipUsesRecordedOrFreshOrdinal,
   AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal,
   AsyncServeAdmissionHighWatermarkIsMonotone,
   AsyncIngressPhysicalHighWatermarkIsMonotone,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   PostGstLeaderWireLifecycleRestartIsDisabled,
   AdmitDormantLeaderWireRetainsLifecycleTokenAndFrozenPrefix,
   RetireLeaderWireLifecycleRetainsTerminalTombstone,
   FS_CardinalityType, FS_Subset, IsaT(7200)
   DEF AsyncCertifiedResponseClaimFrozenSourceInvariant,
       AsyncCertifiedResponseClaimRecords,
       AsyncCertifiedResponseClaimAdmissionsThisStep,
       AsyncCertifiedResponseClaimStateAfterAdmission,
       AsyncCertifiedResponseClaimStateAfterRetirement,
       AsyncControlServiceSlotTransition,
       AsyncControlServiceStateAfterReset,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncLeaderWireLifecycleActive,
       AsyncLeaderWireLifecycleDormant,
       AsyncLeaderWirePotentialOwnerIdentity,
       AsyncLeaderWireLifecycleTransition,
       AsyncLeaderWireLifecycleIngressAdmissionTransition,
       AsyncLeaderWireLifecycleIngressDrainTransition,
       AsyncLeaderWireLifecycleConsumerTransition,
       AsyncLeaderWireLifecycleTerminalTransition,
       AsyncLeaderWireLifecycleRestartTransition,
       AsyncCandidateLifecycleSource,
       AsyncTimeoutCandidateLifecycleOriginSet,
       AsyncTimeoutCandidateLifecycleSourceSet,
       AsyncOwnedTimeoutCandidateLifecycleOriginSet,
       AsyncOwnedTimeoutCandidateLifecycleSourceSet,
       AsyncTimeoutLifecycleUsesRecordedOriginOrdinal,
       AsyncTimeoutLifecycleOwned,
       AsyncTimeoutLifecycleOrigin,
       AsyncTimeoutLifecycleOrdinal,
       AsyncServeIngressSourceFor,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncServeReservationsAfterIngressDrain,
       AsyncServeReservationsAfterIoService,
       ResetNodeSchedulerForRestart,
       PreGstPendingServeReceiverCloseRollback,
       PreGstMaterializedServeReceiverCloseRollback,
       PreGstServeReceiverCloseRollback,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, ServiceIoWorkerWork,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       AsyncAllVars

THEOREM AsyncFiniteRunnerSpecAlwaysCertifiedResponseClaimFrozenSource ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []AsyncCertifiedResponseClaimFrozenSourceInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext)
         PROVE []AsyncCertifiedResponseClaimFrozenSourceInvariant
    <2>1. AsyncInitAt(initialContext)
             => AsyncCertifiedResponseClaimFrozenSourceInvariant
      BY AsyncFiniteRunnerInitEstablishesCertifiedResponseClaimFrozenSource
    <2>2. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>3. []AsyncProgressOwnershipInvariant
      BY <1>1, AsyncSpecAlwaysProgressOwnershipInvariant
    <2>4. []AsyncCandidateServiceLifecycleInvariant
      BY <1>1,
         AsyncFiniteRunnerSpecAlwaysCandidateServiceTombstoneLifecycle,
         Isa
         DEF AsyncCandidateServiceTombstoneLifecycleInvariant
    <2>5. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ AsyncCandidateServiceLifecycleInvariant
           /\ AsyncCertifiedResponseClaimFrozenSourceInvariant
           /\ [AsyncNext]_AsyncAllVars
          => AsyncCertifiedResponseClaimFrozenSourceInvariant'
      BY AsyncFiniteRunnerNextPreservesCertifiedResponseClaimFrozenSource
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, PTL
         DEF AsyncSpecAt
  <1> QED BY <1>1

(***************************************************************************
Claimed-response finite Serve episode.

The receiver's retained certified response owns a target lifecycle ordinal,
a distinct fresh episode scheduler ceiling, a physical admission cut, and
immutable Candidate, Serve, continuation, and leader-wire predecessor source
sets.  The target ordinal identifies the response carrier only; the rank
intersects live work with the frozen source sets and never uses that possibly
old ordinal as an episode cutoff.  A dormant replay therefore remains outside
its own predecessor universe, while every owner admitted before the fresh
episode boundary is prepaid.

The outer barrier rank pays for dormant/Ingress lifecycle stages before the
inner producer rank.  Thus a leader-wire ingress-to-Candidate transfer or a
producer-continuation materialization cannot replenish the rank.  These are
finite episode steps, not claim progress.  Exact retransmission coalesces with
the same claim and preserves both cuts.
***************************************************************************)

CertifiedResponseClaimFrozenTargetLifecycleOrdinal(node) ==
  CertifiedResponseClaimTargetLifecycleOrdinalAt(node)

CertifiedResponseClaimFrozenEpisodeSchedulerCeiling(node) ==
  CertifiedResponseClaimEpisodeSchedulerCeilingAt(node)

CertifiedResponseClaimFrozenEpisodeSchedulerCutoff(node) ==
  CertifiedResponseClaimLifecycleCutoffOrdinal(node)

CertifiedResponseClaimFrozenPhysicalCut(node) ==
  CertifiedResponseClaimPhysicalCutAt(node)

CertifiedResponseClaimFrozenCandidateOrigins(node) ==
  CertifiedResponseClaimFrozenCandidateOriginsAt(node)

CertifiedResponseClaimFrozenServeSources(node) ==
  CertifiedResponseClaimFrozenServeSourcesAt(node)

CertifiedResponseClaimFrozenContinuationSources(node) ==
  CertifiedResponseClaimFrozenContinuationSourcesAt(node)

CertifiedResponseClaimFrozenLeaderWireIdentities(node) ==
  CertifiedResponseClaimFrozenLeaderWireIdentitiesAt(node)

CertifiedResponseClaimBarrierRank(node) ==
  AsyncCertifiedResponsePhysicalBarrierRank(
    node,
    CertifiedResponseClaimFrozenPhysicalCut(node),
    CertifiedResponseClaimFrozenCandidateOrigins(node),
    CertifiedResponseClaimFrozenServeSources(node),
    CertifiedResponseClaimFrozenContinuationSources(node),
    CertifiedResponseClaimFrozenLeaderWireIdentities(node))

CertifiedResponseClaimEpisodeRank(node) ==
  <<CertifiedResponseClaimBarrierRank(node),
    CertifiedResponseClaimAuxRank(node)>>

CertifiedResponseClaimEpisodeRankCarrier ==
  AsyncFrozenLeaderWireBarrierRankCarrier
    \X CertifiedResponseClaimAuxCarrier

CertifiedResponseClaimEpisodeRankOrdering ==
  LexPairOrdering(
    AsyncFrozenLeaderWireBarrierRankOrdering,
    CertifiedResponseClaimAuxOrdering,
    AsyncFrozenLeaderWireBarrierRankCarrier,
    CertifiedResponseClaimAuxCarrier)

CertifiedResponseClaimRankedServeKernel(node, rank) ==
  /\ \/ CertifiedResponseClaimServeEpisodeResidual(node, rank)
     \/ CertifiedResponseClaimCandidateProducerContinuationReentry(
          node, rank)
  /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(node)

CertifiedResponseClaimContinuationResidual(node, rank) ==
  /\ CertifiedResponseClaimServeEpisodeResidual(node, rank)
  /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(node)

THEOREM CertifiedResponseClaimEpisodeRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    CertifiedResponseClaimEpisodeRankOrdering,
    CertifiedResponseClaimEpisodeRankCarrier)
BY AsyncFrozenLeaderWireBarrierRankOrderingIsWellFounded,
   CertifiedResponseClaimAuxOrderingIsWellFounded,
   WFLexPairOrdering
   DEF CertifiedResponseClaimEpisodeRankOrdering,
       CertifiedResponseClaimEpisodeRankCarrier

THEOREM CertifiedResponseClaimFrozenCutIsNatural ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ CertifiedResponseClaimRunnerOwned(node)
      => /\ CertifiedResponseClaimFrozenTargetLifecycleOrdinal(node)
               \in Nat \ {0}
         /\ CertifiedResponseClaimFrozenEpisodeSchedulerCeiling(node)
              \in Nat \ {0}
         /\ CertifiedResponseClaimFrozenTargetLifecycleOrdinal(node)
              <=
                CertifiedResponseClaimFrozenEpisodeSchedulerCeiling(node)
         /\ CertifiedResponseClaimFrozenEpisodeSchedulerCutoff(node)
              \in Nat
         /\ CertifiedResponseClaimFrozenPhysicalCut(node)
              \in Nat \ {0}
         /\ IsFiniteSet(
              CertifiedResponseClaimFrozenCandidateOrigins(node))
         /\ IsFiniteSet(
              CertifiedResponseClaimFrozenServeSources(node))
         /\ IsFiniteSet(
              CertifiedResponseClaimFrozenContinuationSources(node))
         /\ IsFiniteSet(
              CertifiedResponseClaimFrozenLeaderWireIdentities(node))
BY FS_CardinalityType, IsaT(600)
   DEF CertifiedResponseClaimFrozenTargetLifecycleOrdinal,
       CertifiedResponseClaimFrozenEpisodeSchedulerCeiling,
       CertifiedResponseClaimFrozenEpisodeSchedulerCutoff,
       CertifiedResponseClaimFrozenPhysicalCut,
       CertifiedResponseClaimFrozenCandidateOrigins,
       CertifiedResponseClaimFrozenServeSources,
       CertifiedResponseClaimFrozenContinuationSources,
       CertifiedResponseClaimFrozenLeaderWireIdentities,
       CertifiedResponseClaimLifecycleCutoffOrdinal,
       CertifiedResponseClaimTargetLifecycleOrdinalAt,
       CertifiedResponseClaimEpisodeSchedulerCeilingAt,
       CertifiedResponseClaimPhysicalCutAt,
       CertifiedResponseClaimFrozenCandidateOriginsAt,
       CertifiedResponseClaimFrozenServeSourcesAt,
       CertifiedResponseClaimFrozenContinuationSourcesAt,
       CertifiedResponseClaimFrozenLeaderWireIdentitiesAt,
       CertifiedResponseClaimSelectedRecord,
       CertifiedResponseClaimRecordsAt,
       CertifiedResponseClaimRunnerOwned,
       AsyncStrongTypeInvariant, AsyncTypeInvariant,
       AsyncSchedulerTypeInvariant,
       AsyncControlServiceStateTypeInvariant

THEOREM CertifiedResponseClaimEpisodeRankInCarrier ==
  \A node \in ValidatorIds,
     rank \in CertifiedResponseClaimAuxCarrier:
    (\/ CertifiedResponseClaimRankedServeKernel(node, rank)
     \/ CertifiedResponseClaimContinuationResidual(node, rank))
      => CertifiedResponseClaimEpisodeRank(node)
           \in CertifiedResponseClaimEpisodeRankCarrier
BY CertifiedResponseClaimFrozenCutIsNatural,
   AsyncCertifiedResponsePhysicalBarrierRankIsFinite,
   CertifiedResponseClaimAuxRankInCarrier, IsaT(600)
   DEF CertifiedResponseClaimRankedServeKernel,
       CertifiedResponseClaimContinuationResidual,
       CertifiedResponseClaimServeEpisodeResidual,
       CertifiedResponseClaimCandidateProducerContinuationReentry,
       CertifiedResponseClaimBarrierRank,
       CertifiedResponseClaimEpisodeRank,
       CertifiedResponseClaimEpisodeRankCarrier,
       AsyncCertifiedResponsePhysicalBarrierRank,
       CertifiedResponseClaimFrozenPhysicalCut,
       CertifiedResponseClaimFrozenCandidateOrigins,
       CertifiedResponseClaimFrozenServeSources,
       CertifiedResponseClaimFrozenContinuationSources,
       CertifiedResponseClaimFrozenLeaderWireIdentities

THEOREM CertifiedResponseClaimFrozenCutPersistsWhileOwned ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ gst
    /\ CertifiedResponseClaimRunnerOwned(node)
    /\ [AsyncNext]_AsyncAllVars
    /\ CertifiedResponseClaimRunnerOwned(node)'
      => /\ CertifiedResponseClaimSelectedRecord(node)'
              = CertifiedResponseClaimSelectedRecord(node)
         /\ CertifiedResponseClaimFrozenTargetLifecycleOrdinal(node)'
              = CertifiedResponseClaimFrozenTargetLifecycleOrdinal(node)
         /\ CertifiedResponseClaimFrozenEpisodeSchedulerCeiling(node)'
              =
                CertifiedResponseClaimFrozenEpisodeSchedulerCeiling(node)
         /\ CertifiedResponseClaimFrozenEpisodeSchedulerCutoff(node)'
              =
                CertifiedResponseClaimFrozenEpisodeSchedulerCutoff(node)
         /\ CertifiedResponseClaimFrozenPhysicalCut(node)'
              = CertifiedResponseClaimFrozenPhysicalCut(node)
         /\ CertifiedResponseClaimFrozenCandidateOrigins(node)'
              = CertifiedResponseClaimFrozenCandidateOrigins(node)
         /\ CertifiedResponseClaimFrozenServeSources(node)'
              = CertifiedResponseClaimFrozenServeSources(node)
         /\ CertifiedResponseClaimFrozenContinuationSources(node)'
              = CertifiedResponseClaimFrozenContinuationSources(node)
         /\ CertifiedResponseClaimFrozenLeaderWireIdentities(node)'
              = CertifiedResponseClaimFrozenLeaderWireIdentities(node)
BY CertifiedResponseLiveClaimCannotBeReplacedAtGst, Isa
   DEF CertifiedResponseClaimFrozenTargetLifecycleOrdinal,
       CertifiedResponseClaimFrozenEpisodeSchedulerCeiling,
       CertifiedResponseClaimFrozenEpisodeSchedulerCutoff,
       CertifiedResponseClaimFrozenPhysicalCut,
       CertifiedResponseClaimFrozenCandidateOrigins,
       CertifiedResponseClaimFrozenServeSources,
       CertifiedResponseClaimFrozenContinuationSources,
       CertifiedResponseClaimFrozenLeaderWireIdentities,
       CertifiedResponseClaimLifecycleCutoffOrdinal,
       CertifiedResponseClaimRunnerOwned,
       AsyncAllVars

CertifiedResponseClaimPhysicalLeaderWirePredecessorRecords(node) ==
  AsyncCertifiedResponseFrozenLeaderWireRecords(
    node,
    CertifiedResponseClaimFrozenPhysicalCut(node),
    CertifiedResponseClaimFrozenLeaderWireIdentities(node))

CertifiedResponseClaimPhysicalLeaderWirePredecessorIdentities(node) ==
  {AsyncLeaderWirePotentialOwnerIdentity(record):
     record \in
       CertifiedResponseClaimPhysicalLeaderWirePredecessorRecords(node)}

CertifiedResponseClaimPhysicalLeaderWireIngressRecords(node) ==
  AsyncCertifiedResponseFrozenLeaderWireIngressRecords(
    node,
    CertifiedResponseClaimFrozenPhysicalCut(node),
    CertifiedResponseClaimFrozenLeaderWireIdentities(node))

THEOREM CertifiedResponseClaimFrozenPredecessorSetsCannotReplenish ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ gst
    /\ CertifiedResponseClaimRunnerOwned(node)
    /\ [AsyncNext]_AsyncAllVars
    /\ CertifiedResponseClaimRunnerOwned(node)'
      => /\ CertifiedResponseClaimFrozenCandidateOrigins(node)'
              = CertifiedResponseClaimFrozenCandidateOrigins(node)
         /\ CertifiedResponseClaimFrozenServeSources(node)'
              = CertifiedResponseClaimFrozenServeSources(node)
         /\ CertifiedResponseClaimFrozenContinuationSources(node)'
              = CertifiedResponseClaimFrozenContinuationSources(node)
         /\ CertifiedResponseClaimFrozenLeaderWireIdentities(node)'
              = CertifiedResponseClaimFrozenLeaderWireIdentities(node)
         /\ (AsyncFrozenServeAdmissionSources(
                   node,
                   CertifiedResponseClaimFrozenServeSources(node)))'
                \subseteq
                  AsyncFrozenServeAdmissionSources(
                    node,
                    CertifiedResponseClaimFrozenServeSources(node))
         /\ (AsyncFrozenServeLifecycleSources(
                   node,
                   CertifiedResponseClaimFrozenServeSources(node)))'
                \subseteq
                  AsyncFrozenServeLifecycleSources(
                    node,
                    CertifiedResponseClaimFrozenServeSources(node))
         /\ (CertifiedResponseClaimPhysicalLeaderWirePredecessorIdentities(
                  node))'
              \subseteq
                CertifiedResponseClaimPhysicalLeaderWirePredecessorIdentities(
                  node)
BY CertifiedResponseClaimFrozenCutPersistsWhileOwned,
   AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal,
   AsyncServeIngressAdmissionConsumesSharedSchedulerOrdinal,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncIngressPhysicalHighWatermarkIsMonotone,
   PostGstLeaderWireLifecycleRestartIsDisabled,
   IsaT(1800)
   DEF CertifiedResponseClaimFrozenCandidateOrigins,
       CertifiedResponseClaimFrozenServeSources,
       CertifiedResponseClaimFrozenContinuationSources,
       CertifiedResponseClaimFrozenLeaderWireIdentities,
       AsyncFrozenServeAdmissionSources,
       AsyncFrozenServeLifecycleSources,
       AsyncFrozenServeSourceOwned,
       AsyncServeIngressSourceFor,
       AsyncServeReservationRecords,
       AsyncFreshServeIngressAdmissionsForNodeThisStep,
       AsyncServeIngressLifecycleOwnerIdentities,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionSchedulerOrdinal,
       AsyncServeIngressAdmissionRecord,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       CertifiedResponseClaimPhysicalLeaderWirePredecessorIdentities,
       CertifiedResponseClaimPhysicalLeaderWirePredecessorRecords,
       CertifiedResponseClaimFrozenPhysicalCut,
       AsyncCertifiedResponseFrozenLeaderWireRecords,
       AsyncLeaderWirePotentialOwnerIdentity,
       AsyncLeaderWireLifecycleDormant,
       AsyncLeaderWireLifecycleTransition,
       AsyncLeaderWireLifecycleIngressAdmissionTransition,
       AsyncLeaderWireLifecycleIngressDrainTransition,
       AsyncLeaderWireLifecycleConsumerTransition,
       AsyncLeaderWireLifecycleTerminalTransition,
       AsyncLeaderWireLifecycleRestartTransition,
       CandidateScheduled, AsyncAllVars

THEOREM CertifiedResponseClaimFrozenLeaderWireStageBudgetCannotIncrease ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ gst
    /\ CertifiedResponseClaimRunnerOwned(node)
    /\ [AsyncNext]_AsyncAllVars
    /\ CertifiedResponseClaimRunnerOwned(node)'
      => AsyncCertifiedResponseFrozenLeaderWireStageBudget(
           node,
           CertifiedResponseClaimFrozenPhysicalCut(node),
           CertifiedResponseClaimFrozenLeaderWireIdentities(node))'
           <=
             AsyncCertifiedResponseFrozenLeaderWireStageBudget(
               node,
               CertifiedResponseClaimFrozenPhysicalCut(node),
               CertifiedResponseClaimFrozenLeaderWireIdentities(node))
BY CertifiedResponseClaimFrozenCutPersistsWhileOwned,
   CertifiedResponseClaimFrozenPredecessorSetsCannotReplenish,
   AdmitDormantLeaderWireRetainsLifecycleTokenAndFrozenPrefix,
   LeaderWireIngressDrainNeverInventsRuntimeOwner,
   RuntimeLeaderWireCannotRetireMerelyFromIngressPop,
   RetireLeaderWireLifecycleRetainsTerminalTombstone,
   PostGstStepCannotCreateDormantLeaderWirePotential,
   FS_CardinalityType, FS_Subset, IsaT(2400)
   DEF AsyncCertifiedResponseFrozenLeaderWireStageBudget,
       AsyncCertifiedResponseFrozenLeaderWireStageTokens,
       AsyncCertifiedResponseFrozenLeaderWireRecords,
       AsyncFrozenLeaderWireBarrierRemainingStage,
       CertifiedResponseClaimFrozenPhysicalCut,
       CertifiedResponseClaimFrozenLeaderWireIdentities,
       AsyncLeaderWireLifecycleDormant,
       AsyncLeaderWireLifecycleIngressProtected,
       AsyncLeaderWireLifecycleRecordAfterIngressDrain,
       AsyncLeaderWireLifecyclesAfterIngressDrain,
       AsyncLeaderWireLifecycleStateAfterIngressAdmission,
       AsyncLeaderWireLifecycleTransition,
       AsyncLeaderWireLifecycleIngressAdmissionTransition,
       AsyncLeaderWireLifecycleIngressDrainTransition,
       AsyncLeaderWireLifecycleConsumerTransition,
       AsyncLeaderWireLifecycleTerminalTransition,
       AsyncLeaderWireLifecycleRestartTransition,
       AsyncLeaderWirePotentialOwnerIdentity,
       AsyncNext, AsyncAllVars

THEOREM CertifiedResponseClaimBarrierStepIsContinuationDescentOrFrame ==
  \A node \in ValidatorIds:
    LET barrierRank == CertifiedResponseClaimBarrierRank(node)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncCandidateServiceLifecycleInvariant
       /\ AsyncCertifiedResponseClaimFrozenSourceInvariant
       /\ gst
       /\ CertifiedResponseClaimRunnerOwned(node)
       /\ [AsyncNext]_AsyncAllVars
       /\ CertifiedResponseClaimRunnerOwned(node)'
       => \/ AsyncCandidateProducerContinuationRunnerResolutionRequired(
                node)'
          \/ <<CertifiedResponseClaimBarrierRank(node)', barrierRank>>
               \in AsyncFrozenLeaderWireBarrierRankOrdering
          \/ CertifiedResponseClaimBarrierRank(node)' = barrierRank
BY CertifiedResponseClaimFrozenCutPersistsWhileOwned,
   CertifiedResponseClaimFrozenPredecessorSetsCannotReplenish,
   CertifiedResponseClaimFrozenLeaderWireStageBudgetCannotIncrease,
   CandidateProducerContinuationFrozenSourcePrefixStepCannotReplenish,
   CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight,
   CandidateProducerContinuationDormantLocalReplayChargeCannotAppearAtGst,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck,
   AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor,
   AsyncCandidateProducerSemanticHandoffRetirementRequiresAck,
   AsyncCommandSuccessorsStrictlyLowerRemainingWorkStage,
   AsyncCommandSuccessorBatchStrictlyConsumesRemainingWork,
   AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   AsyncServeIngressTargetOnlyCannotOvertakeOlderRuntimeLifecycle,
   AsyncServeIngressTargetOnlyCannotOvertakeOlderLocalLifecycle,
   ExactTicketTurnDecreasesDrainableIngressTurnReach,
   ExhaustedIngressStepDecreasesDrainableIngressTurnReach,
   LocalStepDecreasesDrainableIngressTurnReach,
   SerializedLocalPredecessorDecreasesDrainableIngressTurnReach,
   RuntimeStepDecreasesDrainableIngressTurnReach,
   OlderRuntimeInterleaveDecreasesDrainableIngressTurnReach,
   AdmitDormantLeaderWireRetainsLifecycleTokenAndFrozenPrefix,
   LeaderWireIngressDrainNeverInventsRuntimeOwner,
   RuntimeLeaderWireCannotRetireMerelyFromIngressPop,
   RetireLeaderWireLifecycleRetainsTerminalTombstone,
   PostGstStepCannotCreateDormantLeaderWirePotential,
   FS_CardinalityType, FS_Subset, IsaT(5400)
   DEF CertifiedResponseClaimBarrierRank,
       CertifiedResponseClaimFrozenEpisodeSchedulerCeiling,
       CertifiedResponseClaimFrozenPhysicalCut,
       CertifiedResponseClaimFrozenCandidateOrigins,
       CertifiedResponseClaimFrozenServeSources,
       CertifiedResponseClaimFrozenContinuationSources,
       CertifiedResponseClaimFrozenLeaderWireIdentities,
       CertifiedResponseClaimEpisodeSchedulerCeilingAt,
       CertifiedResponseClaimPhysicalCutAt,
       CertifiedResponseClaimFrozenCandidateOriginsAt,
       CertifiedResponseClaimFrozenServeSourcesAt,
       CertifiedResponseClaimFrozenContinuationSourcesAt,
       CertifiedResponseClaimFrozenLeaderWireIdentitiesAt,
       CertifiedResponseClaimSelectedRecord,
       CertifiedResponseClaimRecordsAt,
       AsyncCertifiedResponsePhysicalBarrierRank,
       AsyncCertifiedResponseFrozenLeaderWireStageBudget,
       AsyncCertifiedResponseFrozenLeaderWireStageTokens,
       AsyncFrozenLeaderWireBarrierRemainingStage,
       AsyncCertifiedResponseFrozenLeaderWireRecords,
       AsyncCertifiedResponseFrozenSourceBarrierTailRank,
       AsyncCertifiedResponseFrozenLeaderWireIngressDependencyRank,
       AsyncCertifiedResponseFrozenLeaderWireIngressRecords,
       AsyncCertifiedResponseFrozenLeaderWireSelectedIngressRecord,
       AsyncFrozenLeaderWireIngressRank,
       AsyncFrozenLeaderWireIngressModeRank,
       AsyncFrozenLeaderWireIngressCapacityRank,
       AsyncFrozenLeaderWireIngressRunnerRank,
       AsyncFrozenLeaderWireIngressPriorityRank,
       AsyncFrozenLeaderWireIngressPriorityOwners,
       AsyncFrozenLeaderWireIngressLaneRank,
       AsyncFrozenLeaderWireIngressLanePosition,
       AsyncFrozenLeaderWireIngressLaneIndices,
       AsyncFrozenLeaderWireIngressSourcePosition,
       AsyncFrozenLeaderWireBarrierRankOrdering,
       AsyncFrozenLeaderWireBarrierTailRankOrdering,
       AsyncFrozenLeaderWireIngressDependencyRankOrdering,
       AsyncFrozenLeaderWirePhysicalRankOrdering,
       AsyncFrozenLeaderWireIngressRankOrdering,
       AsyncCandidateProducerContinuationFrozenSourcePrefixRank,
       AsyncCandidateProducerContinuationFrozenSourceProducerBudget,
       AsyncCandidateProducerContinuationFrozenSourceProducerTokens,
       AsyncCandidateProducerContinuationFrozenSourceCandidateTokens,
       AsyncCandidateProducerContinuationFrozenSourceCandidateOwners,
       AsyncCandidateProducerContinuationFrozenSourceScheduledCandidates,
       AsyncCandidateProducerContinuationFrozenSourceDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenSourceStatusTokens,
       AsyncCandidateProducerContinuationFrozenSourceRecords,
       AsyncCandidateProducerContinuationCausalWeight,
       AsyncFrozenServeWorkBudget,
       AsyncFrozenServeWorkTokens,
       AsyncFrozenServeOccurrenceTokens,
       AsyncFrozenServeIngressPrefixTokens,
       AsyncFrozenServeIoPredecessorTokens,
       AsyncFrozenServeReachDebt,
       AsyncFrozenServeAdmissionSources,
       AsyncFrozenServeSourceIdentities,
       AsyncFrozenServeExactIngressSources,
       AsyncFrozenServeExactIngressIdentities,
       AsyncFrozenServeLifecycleSources,
       AsyncFrozenServeLifecycleIdentities,
       AsyncLeaderWirePhysicalIngressRank,
       AsyncLeaderWireEarlierPhysicalOwners,
       AsyncLeaderWireFrozenIngressPredecessorDebtSet,
       AsyncLeaderWireLifecycleDormant,
       AsyncLeaderWireLifecycleIngressProtected,
       AsyncLeaderWireLifecycleRecordAfterIngressDrain,
       AsyncLeaderWireLifecyclesAfterIngressDrain,
       AsyncLeaderWireLifecycleStateAfterIngressAdmission,
       AsyncLeaderWireIngressPrefixSnapshot,
       AsyncServeIngressTargetOnlyTurn,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncServeReservationsAfterIoService,
       AsyncServeReservationsAfterIngressDrain,
       ServiceIoWorkerWork, PopSelectedIngress,
       DrainFairIngressSelected,
       LocalAdmissionStep, IngressDrainStep,
       SerializedLocalPrecedesServeIngressStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncNext, AsyncAllVars,
       LexPairOrdering, OpToRel

THEOREM CertifiedResponseClaimContinuationInstallationCannotIncreaseEpisodeRank ==
  \A node \in ValidatorIds:
    \A rank \in CertifiedResponseClaimAuxCarrier:
    /\ CertifiedResponseClaimRankedServeKernel(node, rank)
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCertifiedResponseClaimFrozenSourceInvariant
    /\ [AsyncNext]_AsyncAllVars
    /\ CertifiedResponseClaimContinuationResidual(node, rank)'
      => \/ <<CertifiedResponseClaimEpisodeRank(node)',
               CertifiedResponseClaimEpisodeRank(node)>>
              \in CertifiedResponseClaimEpisodeRankOrdering
         \/ CertifiedResponseClaimEpisodeRank(node)'
              = CertifiedResponseClaimEpisodeRank(node)
BY CertifiedResponseClaimBarrierStepIsContinuationDescentOrFrame,
   CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight,
   ClaimedResponseSameNodeRunProducesAuxOutcome,
   ClaimedResponseBlockedAuxStep,
   FS_CardinalityType, IsaT(2400)
   DEF CertifiedResponseClaimRankedServeKernel,
       CertifiedResponseClaimContinuationResidual,
       CertifiedResponseClaimEpisodeRank,
       CertifiedResponseClaimEpisodeRankOrdering,
       CertifiedResponseClaimBarrierRank,
       AsyncCertifiedResponsePhysicalBarrierRank,
       AsyncFrozenLeaderWireBarrierRankOrdering,
       AsyncFrozenLeaderWireBarrierTailRankOrdering,
       AsyncCertifiedResponseFrozenSourceBarrierTailRank,
       AsyncCertifiedResponseFrozenLeaderWireStageBudget,
       AsyncCertifiedResponseFrozenLeaderWireStageTokens,
       AsyncCertifiedResponseFrozenLeaderWireIngressDependencyRank,
       AsyncCandidateProducerContinuationFrozenSourcePrefixRank,
       AsyncCandidateProducerContinuationFrozenSourceProducerBudget,
       AsyncCandidateProducerContinuationFrozenSourceProducerTokens,
       AsyncCandidateProducerContinuationFrozenSourceCandidateTokens,
       AsyncCandidateProducerContinuationFrozenSourceStatusTokens,
       AsyncCandidateProducerContinuationFrozenSourceRecords,
       AsyncFrozenServeWorkBudget,
       AsyncFrozenServeReachDebt,
       CertifiedResponseClaimCandidateProducerContinuationReentry,
       CertifiedResponseClaimAuxOrdering,
       LexPairOrdering, AsyncAllVars

THEOREM CertifiedResponseClaimRankedKernelStepIsGoalDescentOrFrame ==
  \A node \in ValidatorIds:
    \A rank \in CertifiedResponseClaimAuxCarrier:
    /\ CertifiedResponseClaimRankedServeKernel(node, rank)
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCertifiedResponseClaimFrozenSourceInvariant
    /\ [AsyncNext]_AsyncAllVars
      => \/ CertifiedResponseClaimAuxProgress(node, rank)'
         \/ /\ CertifiedResponseClaimContinuationResidual(node, rank)'
            /\ \/ <<CertifiedResponseClaimEpisodeRank(node)',
                     CertifiedResponseClaimEpisodeRank(node)>>
                    \in CertifiedResponseClaimEpisodeRankOrdering
               \/ CertifiedResponseClaimEpisodeRank(node)'
                    = CertifiedResponseClaimEpisodeRank(node)
         \/ /\ CertifiedResponseClaimRankedServeKernel(node, rank)'
            /\ <<CertifiedResponseClaimEpisodeRank(node)',
                  CertifiedResponseClaimEpisodeRank(node)>>
                 \in CertifiedResponseClaimEpisodeRankOrdering
         \/ /\ CertifiedResponseClaimRankedServeKernel(node, rank)'
            /\ CertifiedResponseClaimEpisodeRank(node)'
                 = CertifiedResponseClaimEpisodeRank(node)
BY CertifiedResponseClaimBarrierStepIsContinuationDescentOrFrame,
   CertifiedResponseClaimContinuationInstallationCannotIncreaseEpisodeRank,
   ClaimedResponseSameNodeRunProducesAuxOutcome,
   ClaimedResponseBlockedAuxStep,
   IsaT(1800)
   DEF CertifiedResponseClaimRankedServeKernel,
       CertifiedResponseClaimContinuationResidual,
       CertifiedResponseClaimCandidateProducerContinuationReentry,
       CertifiedResponseClaimEpisodeRank,
       CertifiedResponseClaimEpisodeRankOrdering,
       AsyncFrozenLeaderWireBarrierRankOrdering,
       CertifiedResponseClaimAuxOrdering,
       LexPairOrdering, AsyncAllVars

CertifiedResponseClaimPhysicalLeaderWireBarrierActive(node) ==
  CertifiedResponseClaimPhysicalLeaderWireIngressRecords(node) # {}

CertifiedResponseClaimPhysicalLeaderWireOwnsSharedTurn(node) ==
  /\ CertifiedResponseClaimPhysicalLeaderWireBarrierActive(node)
  /\ AsyncLeaderWireIngressOwnsSharedPhysicalTurn(node)

CertifiedResponseClaimFrozenServeIoOwnerRequired(node) ==
  AsyncFrozenServeIoOwnerRequired(
    node, CertifiedResponseClaimFrozenServeSources(node))

CertifiedResponseClaimFairOwner(node) ==
  IF CertifiedResponseClaimPhysicalLeaderWireOwnsSharedTurn(node)
  THEN "Runner"
  ELSE IF CertifiedResponseClaimFrozenServeIoOwnerRequired(node)
       THEN "IoWorker"
       ELSE "Runner"

CertifiedResponseClaimFairAction(node) ==
  AsyncCausalEpisodeFairAction(
    node, CertifiedResponseClaimFairOwner(node))

THEOREM CertifiedResponseClaimSelectedOwnerIsConcreteAndEnabled ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ gst
    /\ CertifiedResponseClaimRunnerOwned(node)
      => /\ CertifiedResponseClaimFairOwner(node)
               \in AsyncCausalEpisodeFairOwnerKinds
         /\ ENABLED
              <<CertifiedResponseClaimFairAction(node)>>_AsyncAllVars
BY QueuedIoEnablesPostGstService,
   QueuedIoServiceIsNonstuttering,
   ResponsiveUnappliedRunNodeIsEnabled,
   EnabledRunNodeLiftsPostGst,
   ExpandENABLED, ENABLEDaxioms, IsaT(900)
   DEF CertifiedResponseClaimFairOwner,
       CertifiedResponseClaimFairAction,
       CertifiedResponseClaimPhysicalLeaderWireOwnsSharedTurn,
       CertifiedResponseClaimPhysicalLeaderWireBarrierActive,
       CertifiedResponseClaimPhysicalLeaderWireIngressRecords,
       CertifiedResponseClaimFrozenServeIoOwnerRequired,
       CertifiedResponseClaimFrozenServeSources,
       AsyncFrozenServeIoOwnerRequired,
       AsyncFrozenServeLifecycleSources,
       AsyncCausalEpisodeFairAction,
       AsyncCausalEpisodeFairOwnerKinds,
       CertifiedResponseClaimRunnerOwned,
       AsyncCurrentResponsiveVoters,
       AsyncArchiveIoServiceNodes,
       PostGstRunNode, RunNode, RunNodeWork,
       AsyncAllVars

THEOREM CertifiedResponseClaimSelectedActionConsumesRankCell ==
  \A node \in ValidatorIds:
    \A rank \in CertifiedResponseClaimAuxCarrier:
    /\ CertifiedResponseClaimRankedServeKernel(node, rank)
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCertifiedResponseClaimFrozenSourceInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ <<CertifiedResponseClaimFairAction(node)>>_AsyncAllVars
      => \/ CertifiedResponseClaimAuxProgress(node, rank)'
         \/ /\ CertifiedResponseClaimContinuationResidual(node, rank)'
            /\ \/ <<CertifiedResponseClaimEpisodeRank(node)',
                     CertifiedResponseClaimEpisodeRank(node)>>
                    \in CertifiedResponseClaimEpisodeRankOrdering
               \/ CertifiedResponseClaimEpisodeRank(node)'
                    = CertifiedResponseClaimEpisodeRank(node)
         \/ /\ CertifiedResponseClaimRankedServeKernel(node, rank)'
            /\ <<CertifiedResponseClaimEpisodeRank(node)',
                  CertifiedResponseClaimEpisodeRank(node)>>
                 \in CertifiedResponseClaimEpisodeRankOrdering
BY CertifiedResponseClaimSelectedOwnerIsConcreteAndEnabled,
   CertifiedResponseClaimBarrierStepIsContinuationDescentOrFrame,
   CertifiedResponseClaimContinuationInstallationCannotIncreaseEpisodeRank,
   ClaimedResponseSameNodeRunProducesAuxOutcome,
   ServiceIoWorkerDropsQueueDepth,
   IsaT(1800)
   DEF CertifiedResponseClaimRankedServeKernel,
       CertifiedResponseClaimContinuationResidual,
       CertifiedResponseClaimCandidateProducerContinuationReentry,
       CertifiedResponseClaimEpisodeRank,
       CertifiedResponseClaimEpisodeRankOrdering,
       CertifiedResponseClaimFairAction,
       CertifiedResponseClaimFairOwner,
       CertifiedResponseClaimPhysicalLeaderWireOwnsSharedTurn,
       CertifiedResponseClaimPhysicalLeaderWireBarrierActive,
       AsyncCausalEpisodeFairAction,
       CertifiedResponseClaimFrozenServeIoOwnerRequired,
       AsyncFrozenServeIoOwnerRequired,
       LexPairOrdering, AsyncAllVars

THEOREM CertifiedResponseClaimConcreteOwnerPersistsInRankCell ==
  \A node \in ValidatorIds:
    LET owner == CertifiedResponseClaimFairOwner(node)
        barrierRank == CertifiedResponseClaimBarrierRank(node)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncCandidateServiceLifecycleInvariant
       /\ AsyncCertifiedResponseClaimFrozenSourceInvariant
       /\ gst
       /\ CertifiedResponseClaimRunnerOwned(node)
       /\ [AsyncNext]_AsyncAllVars
       /\ CertifiedResponseClaimRunnerOwned(node)'
       /\ CertifiedResponseClaimBarrierRank(node)' = barrierRank
       /\ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(
            node)'
       => CertifiedResponseClaimFairOwner(node)' = owner
BY CertifiedResponseClaimFrozenCutPersistsWhileOwned,
   CertifiedResponseClaimFrozenPredecessorSetsCannotReplenish,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   PostGstStepCannotCreateDormantLeaderWirePotential,
   IsaT(1800)
   DEF CertifiedResponseClaimFairOwner,
       CertifiedResponseClaimPhysicalLeaderWireOwnsSharedTurn,
       CertifiedResponseClaimPhysicalLeaderWireBarrierActive,
       CertifiedResponseClaimPhysicalLeaderWireIngressRecords,
       CertifiedResponseClaimFrozenServeIoOwnerRequired,
       CertifiedResponseClaimBarrierRank,
       AsyncCertifiedResponsePhysicalBarrierRank,
       AsyncCertifiedResponseFrozenLeaderWireStageBudget,
       AsyncCertifiedResponseFrozenLeaderWireStageTokens,
       AsyncCertifiedResponseFrozenLeaderWireRecords,
       AsyncCertifiedResponseFrozenLeaderWireIngressRecords,
       AsyncFrozenServeIoOwnerRequired,
       AsyncFrozenServeLifecycleSources,
       AsyncAllVars

CertifiedResponseClaimEpisodeAtRank(node, baselineRank, episodeRank) ==
  /\ CertifiedResponseClaimRankedServeKernel(node, baselineRank)
  /\ CertifiedResponseClaimEpisodeRank(node) = episodeRank

CertifiedResponseClaimContinuationAtEpisodeRank(
    node, baselineRank, episodeRank) ==
  /\ CertifiedResponseClaimContinuationResidual(node, baselineRank)
  /\ CertifiedResponseClaimEpisodeRank(node) = episodeRank

CertifiedResponseClaimEpisodeRankGoal(
    node, baselineRank, episodeRank) ==
  \/ CertifiedResponseClaimAuxProgress(node, baselineRank)
  \/ CertifiedResponseClaimContinuationAtEpisodeRank(
       node, baselineRank, episodeRank)
  \/ \E lower \in
       SetLessThan(
         episodeRank,
         CertifiedResponseClaimEpisodeRankOrdering,
         CertifiedResponseClaimEpisodeRankCarrier):
       \/ CertifiedResponseClaimEpisodeAtRank(
            node, baselineRank, lower)
          \/ CertifiedResponseClaimContinuationAtEpisodeRank(
               node, baselineRank, lower)

CertifiedResponseClaimEpisodeRankStepProperty(specification) ==
  specification
    => \A node \in ValidatorIds,
          baselineRank \in CertifiedResponseClaimAuxCarrier,
          episodeRank \in CertifiedResponseClaimEpisodeRankCarrier:
         CertifiedResponseClaimEpisodeAtRank(
           node, baselineRank, episodeRank)
           ~> CertifiedResponseClaimEpisodeRankGoal(
                node, baselineRank, episodeRank)

THEOREM AsyncSpecProvidesCertifiedResponseClaimEpisodeRankStep ==
  \A initialContext:
    CertifiedResponseClaimEpisodeRankStepProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncFiniteRunnerSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncFiniteRunnerSpecAlwaysCertifiedResponseClaimFrozenSource,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   CertifiedResponseClaimSelectedOwnerIsConcreteAndEnabled,
   CertifiedResponseClaimRankedKernelStepIsGoalDescentOrFrame,
   CertifiedResponseClaimSelectedActionConsumesRankCell,
   CertifiedResponseClaimConcreteOwnerPersistsInRankCell,
   AsyncRunnerEpisodeConcreteOwnerUsesExistingFairness,
   PTL, IsaT(1200)
   DEF CertifiedResponseClaimEpisodeRankStepProperty,
       CertifiedResponseClaimEpisodeAtRank,
       CertifiedResponseClaimEpisodeRankGoal,
       CertifiedResponseClaimContinuationAtEpisodeRank,
       CertifiedResponseClaimFairOwner,
       CertifiedResponseClaimFairAction

CertifiedResponseClaimRankedServeClosureProperty(specification) ==
  specification
    => \A node \in ValidatorIds:
         \A rank \in CertifiedResponseClaimAuxCarrier:
           CertifiedResponseClaimRankedServeKernel(node, rank)
             ~> (CertifiedResponseClaimAuxProgress(node, rank)
                  \/ CertifiedResponseClaimContinuationResidual(node, rank))

THEOREM AsyncSpecProvidesCertifiedResponseClaimRankedServeClosure ==
  \A initialContext:
    CertifiedResponseClaimRankedServeClosureProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesCertifiedResponseClaimEpisodeRankStep,
   CertifiedResponseClaimEpisodeRankInCarrier,
   CertifiedResponseClaimEpisodeRankOrderingIsWellFounded,
   WellFoundedLeadsTo, PTL
   DEF CertifiedResponseClaimRankedServeClosureProperty,
       CertifiedResponseClaimEpisodeRankStepProperty,
       CertifiedResponseClaimEpisodeAtRank,
       CertifiedResponseClaimEpisodeRankGoal,
       CertifiedResponseClaimContinuationAtEpisodeRank,
       CertifiedResponseClaimContinuationResidual

THEOREM CertifiedResponseClaimContinuationOwnsVoterEpisode ==
  \A node \in ValidatorIds:
    \A rank \in CertifiedResponseClaimAuxCarrier:
      CertifiedResponseClaimContinuationResidual(node, rank)
        => AsyncVoterCandidateProducerContinuationEpisodePending(node)
BY Isa
   DEF CertifiedResponseClaimContinuationResidual,
       CertifiedResponseClaimServeEpisodeResidual,
       CertifiedResponseClaimRunnerOwned,
       AsyncVoterCandidateProducerContinuationEpisodePending

THEOREM CertifiedResponseClaimContinuationStepFramesTarget ==
  \A node \in ValidatorIds:
    \A rank \in CertifiedResponseClaimAuxCarrier:
    /\ CertifiedResponseClaimContinuationResidual(node, rank)
    /\ [AsyncNext]_AsyncAllVars
      => \/ CertifiedResponseClaimAuxProgress(node, rank)'
         \/ CertifiedResponseClaimContinuationResidual(node, rank)'
         \/ CertifiedResponseClaimRankedServeKernel(node, rank)'
BY ClaimedResponseSameNodeRunProducesAuxOutcome,
   ClaimedResponseBlockedAuxStep, IsaT(1200)
   DEF CertifiedResponseClaimContinuationResidual,
       CertifiedResponseClaimRankedServeKernel,
       CertifiedResponseClaimCandidateProducerContinuationReentry,
       CertifiedResponseClaimAuxStrictResult,
       CertifiedResponseClaimAuxStepResult,
       AsyncAllVars

CertifiedResponseClaimContinuationWithinEpisodeRank(
    node, baselineRank, entryRank) ==
  /\ CertifiedResponseClaimContinuationResidual(node, baselineRank)
  /\ \/ CertifiedResponseClaimEpisodeRank(node) = entryRank
     \/ <<CertifiedResponseClaimEpisodeRank(node), entryRank>>
          \in CertifiedResponseClaimEpisodeRankOrdering

CertifiedResponseClaimStrictlyLowerRankedEpisode(
    node, baselineRank, entryRank) ==
  \E lower \in
       SetLessThan(
         entryRank,
         CertifiedResponseClaimEpisodeRankOrdering,
         CertifiedResponseClaimEpisodeRankCarrier):
       CertifiedResponseClaimEpisodeAtRank(
         node, baselineRank, lower)

CertifiedResponseClaimContinuationRankedExitGoal(
    node, baselineRank, entryRank) ==
  \/ CertifiedResponseClaimAuxProgress(node, baselineRank)
  \/ CertifiedResponseClaimStrictlyLowerRankedEpisode(
       node, baselineRank, entryRank)

(***************************************************************************
The continuation handoff is a finite proofless episode, not claim progress.
Its entry rank is frozen as a theorem parameter.  Replay may preserve that
rank while consuming the existing two-step local replay distance; resolving
or materializing the selected continuation consumes a prepaid frozen-prefix
token.  Therefore the episode can remain at or below its entry cell, but it
may leave continuation ownership only through auxiliary claim progress or a
strictly lower ranked claim kernel.  It cannot return to the same ranked cell.
***************************************************************************)
THEOREM CertifiedResponseClaimContinuationStepStaysBelowEntryOrExits ==
  \A node \in ValidatorIds,
     baselineRank \in CertifiedResponseClaimAuxCarrier,
     entryRank \in CertifiedResponseClaimEpisodeRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCertifiedResponseClaimFrozenSourceInvariant
    /\ CertifiedResponseClaimContinuationWithinEpisodeRank(
         node, baselineRank, entryRank)
    /\ [AsyncNext]_AsyncAllVars
      => \/ CertifiedResponseClaimAuxProgress(node, baselineRank)'
         \/ CertifiedResponseClaimContinuationWithinEpisodeRank(
              node, baselineRank, entryRank)'
         \/ CertifiedResponseClaimStrictlyLowerRankedEpisode(
              node, baselineRank, entryRank)'
BY CertifiedResponseClaimContinuationStepFramesTarget,
   CertifiedResponseClaimFrozenCutPersistsWhileOwned,
   CertifiedResponseClaimFrozenPredecessorSetsCannotReplenish,
   CertifiedResponseClaimBarrierStepIsContinuationDescentOrFrame,
   CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight,
   CandidateProducerContinuationFrozenSourceFairResolutionStrictlyDescends,
   AsyncVoterProducerContinuationCompositeRankStepCannotIncrease,
   AsyncVoterRunNodeWorkStrictlyDecreasesFrozenProducerContinuationCompositeRank,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck,
   AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor,
   AsyncCandidateProducerSemanticHandoffRetirementRequiresAck,
   FS_CardinalityType, FS_Subset, IsaT(5400)
   DEF CertifiedResponseClaimContinuationWithinEpisodeRank,
       CertifiedResponseClaimStrictlyLowerRankedEpisode,
       CertifiedResponseClaimEpisodeAtRank,
       CertifiedResponseClaimRankedServeKernel,
       CertifiedResponseClaimContinuationResidual,
       CertifiedResponseClaimCandidateProducerContinuationReentry,
       CertifiedResponseClaimEpisodeRank,
       CertifiedResponseClaimEpisodeRankOrdering,
       CertifiedResponseClaimBarrierRank,
       AsyncCertifiedResponsePhysicalBarrierRank,
       AsyncFrozenLeaderWireBarrierRankOrdering,
       AsyncFrozenLeaderWireBarrierTailRankOrdering,
       AsyncCertifiedResponseFrozenSourceBarrierTailRank,
       AsyncCertifiedResponseFrozenLeaderWireStageBudget,
       AsyncCertifiedResponseFrozenLeaderWireStageTokens,
       AsyncCertifiedResponseFrozenLeaderWireIngressDependencyRank,
       AsyncCandidateProducerContinuationFrozenSourcePrefixRank,
       AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
       AsyncCandidateProducerContinuationFrozenSourceProducerBudget,
       AsyncCandidateProducerContinuationFrozenSourceProducerTokens,
       AsyncCandidateProducerContinuationFrozenSourceCandidateTokens,
       AsyncCandidateProducerContinuationFrozenSourceStatusTokens,
       AsyncCandidateProducerContinuationFrozenSourceRecords,
       AsyncVoterCandidateProducerContinuationFrozenCompositeRank,
       AsyncVoterCandidateProducerContinuationFrozenPrefixBudget,
       AsyncVoterCandidateProducerContinuationFrozenActiveRecordsForNode,
       AsyncCandidateProducerContinuationExactReplayDistance,
       CertifiedResponseClaimAuxOrdering,
       SetLessThan, LexPairOrdering, AsyncAllVars

CertifiedResponseClaimContinuationRankedExitProperty(
    specification, initialContext) ==
  specification
    => \A node \in ValidatorIds,
          baselineRank \in CertifiedResponseClaimAuxCarrier,
          entryRank \in CertifiedResponseClaimEpisodeRankCarrier:
         CertifiedResponseClaimContinuationAtEpisodeRank(
           node, baselineRank, entryRank)
           ~> CertifiedResponseClaimContinuationRankedExitGoal(
                node, baselineRank, entryRank)

THEOREM AsyncSpecProvidesCertifiedResponseClaimContinuationRankedExit ==
  \A initialContext:
    CertifiedResponseClaimContinuationRankedExitProperty(
      AsyncSpecAt(initialContext), initialContext)
BY AsyncSpecProvidesVoterCandidateProducerContinuationResolutionClosure,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   AsyncFiniteRunnerSpecAlwaysCandidateServiceTombstoneLifecycle,
   AsyncFiniteRunnerSpecAlwaysCertifiedResponseClaimFrozenSource,
   AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage,
   AsyncSpecAlwaysCandidateProducerContinuationLocalReplayCapacity,
   AsyncSpecAlwaysUsesFixedResponsiveVoters,
   CertifiedResponseClaimContinuationOwnsVoterEpisode,
   CertifiedResponseClaimContinuationStepFramesTarget,
   CertifiedResponseClaimContinuationStepStaysBelowEntryOrExits,
   PTL, IsaT(2400)
   DEF CertifiedResponseClaimContinuationRankedExitProperty,
       CertifiedResponseClaimContinuationRankedExitGoal,
       CertifiedResponseClaimContinuationAtEpisodeRank,
       CertifiedResponseClaimContinuationWithinEpisodeRank,
       CertifiedResponseClaimStrictlyLowerRankedEpisode,
       AsyncVoterCandidateProducerContinuationResolutionClosureProperty

CertifiedResponseClaimCompleteEpisodeAtRank(
    node, baselineRank, episodeRank) ==
  \/ CertifiedResponseClaimEpisodeAtRank(
       node, baselineRank, episodeRank)
  \/ CertifiedResponseClaimContinuationAtEpisodeRank(
       node, baselineRank, episodeRank)

CertifiedResponseClaimCompleteEpisodeRankGoal(
    node, baselineRank, episodeRank) ==
  \/ CertifiedResponseClaimAuxProgress(node, baselineRank)
  \/ \E lower \in
       SetLessThan(
         episodeRank,
         CertifiedResponseClaimEpisodeRankOrdering,
         CertifiedResponseClaimEpisodeRankCarrier):
       CertifiedResponseClaimCompleteEpisodeAtRank(
         node, baselineRank, lower)

CertifiedResponseClaimCompleteEpisodeRankStepProperty(specification) ==
  specification
    => \A node \in ValidatorIds,
          baselineRank \in CertifiedResponseClaimAuxCarrier,
          episodeRank \in CertifiedResponseClaimEpisodeRankCarrier:
         CertifiedResponseClaimCompleteEpisodeAtRank(
           node, baselineRank, episodeRank)
           ~> CertifiedResponseClaimCompleteEpisodeRankGoal(
                node, baselineRank, episodeRank)

THEOREM AsyncSpecProvidesCertifiedResponseClaimCompleteEpisodeRankStep ==
  \A initialContext:
    CertifiedResponseClaimCompleteEpisodeRankStepProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesCertifiedResponseClaimEpisodeRankStep,
   AsyncSpecProvidesCertifiedResponseClaimContinuationRankedExit,
   PTL, IsaT(1200)
   DEF CertifiedResponseClaimCompleteEpisodeRankStepProperty,
       CertifiedResponseClaimCompleteEpisodeRankGoal,
       CertifiedResponseClaimCompleteEpisodeAtRank,
       CertifiedResponseClaimEpisodeRankStepProperty,
       CertifiedResponseClaimEpisodeRankGoal,
       CertifiedResponseClaimContinuationRankedExitProperty,
       CertifiedResponseClaimContinuationRankedExitGoal,
       CertifiedResponseClaimStrictlyLowerRankedEpisode

CertifiedResponseClaimCompleteEpisodeClosureProperty(specification) ==
  specification
    => \A node \in ValidatorIds,
          baselineRank \in CertifiedResponseClaimAuxCarrier,
          episodeRank \in CertifiedResponseClaimEpisodeRankCarrier:
         CertifiedResponseClaimCompleteEpisodeAtRank(
           node, baselineRank, episodeRank)
           ~> CertifiedResponseClaimAuxProgress(node, baselineRank)

THEOREM AsyncSpecProvidesCertifiedResponseClaimCompleteEpisodeClosure ==
  \A initialContext:
    CertifiedResponseClaimCompleteEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesCertifiedResponseClaimCompleteEpisodeRankStep,
   CertifiedResponseClaimEpisodeRankOrderingIsWellFounded,
   WellFoundedLeadsTo, PTL
   DEF CertifiedResponseClaimCompleteEpisodeClosureProperty,
       CertifiedResponseClaimCompleteEpisodeRankStepProperty,
       CertifiedResponseClaimCompleteEpisodeAtRank,
       CertifiedResponseClaimCompleteEpisodeRankGoal

THEOREM CertifiedResponseClaimServeResidualStartsCompleteEpisode ==
  \A node \in ValidatorIds,
     rank \in CertifiedResponseClaimAuxCarrier:
    CertifiedResponseClaimServeEpisodeResidual(node, rank)
      => /\ CertifiedResponseClaimEpisodeRank(node)
               \in CertifiedResponseClaimEpisodeRankCarrier
         /\ CertifiedResponseClaimCompleteEpisodeAtRank(
              node, rank, CertifiedResponseClaimEpisodeRank(node))
BY CertifiedResponseClaimEpisodeRankInCarrier, Isa
   DEF CertifiedResponseClaimCompleteEpisodeAtRank,
       CertifiedResponseClaimEpisodeAtRank,
       CertifiedResponseClaimContinuationAtEpisodeRank,
       CertifiedResponseClaimRankedServeKernel,
       CertifiedResponseClaimContinuationResidual,
       CertifiedResponseClaimCandidateProducerContinuationReentry

THEOREM AsyncSpecProvidesCertifiedResponseClaimFiniteServeEpisodeResidual ==
  \A initialContext:
    CertifiedResponseClaimFiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesCertifiedResponseClaimCompleteEpisodeClosure,
   CertifiedResponseClaimServeResidualStartsCompleteEpisode,
   PTL
   DEF CertifiedResponseClaimFiniteServeEpisodeResidualProperty,
       CertifiedResponseClaimCompleteEpisodeClosureProperty

=============================================================================
