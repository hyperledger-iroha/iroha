---- MODULE SumeragiV2AsyncFiniteRunnerEpisodeProofs ----
EXTENDS SumeragiV2AsyncStage2Proofs,
        SumeragiV2AsyncCausalWorkBudgetProofs

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
    AsyncReadyRunnerEpisodeResidual(
      kind, candidate, position, baselineRank)
      => AsyncReadyRunnerEpisodeRank(candidate)
           \in AsyncReadyRunnerEpisodeRankCarrier
BY AsyncCausalEpisodeStructuralRankIsFinite,
   ReadyRunAuxRankInCarrier, IsaT(300)
   DEF AsyncReadyRunnerEpisodeKinds,
       AsyncReadyRunnerEpisodeResidual,
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
    AsyncCapacityRunnerEpisodeResidual(
      kind, candidate, position, baselineRank)
      => AsyncCapacityRunnerEpisodeRank(candidate)
           \in AsyncCapacityRunnerEpisodeRankCarrier
BY AsyncCausalEpisodeStructuralRankIsFinite,
   Stage4CapacityRankInCarrier, IsaT(300)
   DEF AsyncCapacityRunnerEpisodeKinds,
       AsyncCapacityRunnerEpisodeResidual,
       AsyncCapacityRunnerEpisodeRank,
       AsyncCapacityRunnerEpisodeRankCarrier,
       Stage4CapacityServeEpisodeResidual,
       Stage6NonCompletionCapacityServeEpisodeResidual,
       ProtectedStage4Pending, ProtectedStage6Pending,
       ProtectedOwnedAtServiceRank,
       ResponsiveProtectedCandidateOwned

AsyncReadyRunnerEpisodeAtRank(
    kind, candidate, position, baselineRank, episodeRank) ==
  /\ AsyncReadyRunnerEpisodeResidual(
       kind, candidate, position, baselineRank)
  /\ AsyncReadyRunnerEpisodeRank(candidate) = episodeRank

AsyncCapacityRunnerEpisodeAtRank(
    kind, candidate, position, baselineRank, episodeRank) ==
  /\ AsyncCapacityRunnerEpisodeResidual(
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
    /\ AsyncReadyRunnerEpisodeResidual(
         kind, candidate, position, baselineRank)
    /\ [AsyncNext]_AsyncAllVars
    => \/ AsyncReadyRunnerEpisodeGoal(
            kind, candidate, position, baselineRank)'
       \/ <<AsyncReadyRunnerEpisodeRank(candidate)',
             AsyncReadyRunnerEpisodeRank(candidate)>>
            \in AsyncReadyRunnerEpisodeRankOrdering
       \/ AsyncReadyRunnerEpisodeResidual(
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
       AsyncReadyRunnerEpisodeRank,
       AsyncReadyRunnerEpisodeRankOrdering,
       AsyncCausalEpisodeStructuralRankOrdering,
       ReadyRunAuxOrdering, LexPairOrdering, AsyncAllVars

THEOREM AsyncCapacityRunnerEpisodeStepIsGoalDescentOrFrame ==
  \A kind \in AsyncCapacityRunnerEpisodeKinds,
     candidate, position, baselineRank:
    /\ AsyncCapacityRunnerEpisodeResidual(
         kind, candidate, position, baselineRank)
    /\ [AsyncNext]_AsyncAllVars
    => \/ AsyncCapacityRunnerEpisodeGoal(
            kind, candidate, position, baselineRank)'
       \/ <<AsyncCapacityRunnerEpisodeRank(candidate)',
             AsyncCapacityRunnerEpisodeRank(candidate)>>
            \in AsyncCapacityRunnerEpisodeRankOrdering
       \/ AsyncCapacityRunnerEpisodeResidual(
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
       AsyncCapacityRunnerEpisodeRank,
       AsyncCapacityRunnerEpisodeRankOrdering,
       AsyncCausalEpisodeStructuralRankOrdering,
       Stage4CapacityOrdering, LexPairOrdering, AsyncAllVars

THEOREM AsyncReadyRunnerEpisodeSelectedActionConsumesRankCell ==
  \A kind \in AsyncReadyRunnerEpisodeKinds,
     candidate, position, baselineRank:
    /\ AsyncReadyRunnerEpisodeResidual(
         kind, candidate, position, baselineRank)
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
    /\ AsyncCapacityRunnerEpisodeResidual(
         kind, candidate, position, baselineRank)
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
       AsyncReadyRunnerEpisodeResidual,
       AsyncCausalEpisodeFairOwner,
       AsyncCausalEpisodeSelectedFairAction

THEOREM AsyncSpecProvidesCapacityRunnerEpisodeRankStep ==
  \A initialContext:
    AsyncCapacityRunnerEpisodeRankStepProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
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
       AsyncCapacityRunnerEpisodeResidual,
       AsyncCausalEpisodeFairOwner,
       AsyncCausalEpisodeSelectedFairAction

AsyncReadyRunnerEpisodeClosureProperty(specification) ==
  specification
    => \A kind \in AsyncReadyRunnerEpisodeKinds,
          candidate, position, baselineRank:
         AsyncReadyRunnerEpisodeResidual(
           kind, candidate, position, baselineRank)
           ~> AsyncReadyRunnerEpisodeGoal(
                kind, candidate, position, baselineRank)

AsyncCapacityRunnerEpisodeClosureProperty(specification) ==
  specification
    => \A kind \in AsyncCapacityRunnerEpisodeKinds,
          candidate, position, baselineRank:
         AsyncCapacityRunnerEpisodeResidual(
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
Exact leaf discharge and aggregate provider.

These six theorems are projections of the two generic closures.  The final
theorem is intentionally unconditional and mentions every Stage-3/4/6 leaf
through the aggregate definitions.  No theorem in this module may use
`ProtectedServiceFiniteRunnerEpisodeClosureProperty` as a premise.
***************************************************************************)

THEOREM AsyncSpecProvidesStage3FiniteServeEpisodeResidual ==
  \A initialContext:
    Stage3FiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesReadyRunnerEpisodeClosure, PTL
   DEF AsyncReadyRunnerEpisodeClosureProperty,
       AsyncReadyRunnerEpisodeKinds,
       AsyncReadyRunnerEpisodeResidual,
       AsyncReadyRunnerEpisodeGoal,
       Stage3FiniteServeEpisodeResidualProperty

THEOREM AsyncSpecProvidesStage4FiniteServeEpisodeResidual ==
  \A initialContext:
    Stage4FiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesReadyRunnerEpisodeClosure, PTL
   DEF AsyncReadyRunnerEpisodeClosureProperty,
       AsyncReadyRunnerEpisodeKinds,
       AsyncReadyRunnerEpisodeResidual,
       AsyncReadyRunnerEpisodeGoal,
       Stage4FiniteServeEpisodeResidualProperty

THEOREM AsyncSpecProvidesStage4CapacityFiniteServeEpisodeResidual ==
  \A initialContext:
    Stage4CapacityFiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesCapacityRunnerEpisodeClosure, PTL
   DEF AsyncCapacityRunnerEpisodeClosureProperty,
       AsyncCapacityRunnerEpisodeKinds,
       AsyncCapacityRunnerEpisodeResidual,
       AsyncCapacityRunnerEpisodeGoal,
       Stage4CapacityFiniteServeEpisodeResidualProperty

THEOREM AsyncSpecProvidesStage6NonCompletionFiniteServeEpisodeResidual ==
  \A initialContext:
    Stage6NonCompletionCapacityFiniteServeEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesCapacityRunnerEpisodeClosure, PTL
   DEF AsyncCapacityRunnerEpisodeClosureProperty,
       AsyncCapacityRunnerEpisodeKinds,
       AsyncCapacityRunnerEpisodeResidual,
       AsyncCapacityRunnerEpisodeGoal,
       Stage6NonCompletionCapacityFiniteServeEpisodeResidualProperty

THEOREM AsyncSpecProvidesStage6OwedReadyFiniteRunnerEpisodeResidual ==
  \A initialContext:
    Stage6OwedReadyFiniteRunnerEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesReadyRunnerEpisodeClosure, PTL
   DEF AsyncReadyRunnerEpisodeClosureProperty,
       AsyncReadyRunnerEpisodeKinds,
       AsyncReadyRunnerEpisodeResidual,
       AsyncReadyRunnerEpisodeGoal,
       Stage6OwedReadyFiniteRunnerEpisodeResidualProperty

THEOREM AsyncSpecProvidesStage6PreAdmissionFiniteRunnerEpisodeResidual ==
  \A initialContext:
    Stage6PreAdmissionFiniteRunnerEpisodeResidualProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesReadyRunnerEpisodeClosure, PTL
   DEF AsyncReadyRunnerEpisodeClosureProperty,
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

=============================================================================
