---- MODULE SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs ----
EXTENDS SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs

(***************************************************************************
Historical Candidate/Serve finite-runner provider under `AsyncSpecAt`.

Historical recovery targets can block the shared scheduler and discovery
clock even when they are outside the current voter roster.  The ordinary
finite-runner theorem therefore cannot discharge their residuals: their fair
owners are `PostGstRunHistoricalRecoveryNode` and
`PostGstServiceHistoricalRecoveryIoWorker`.

The outer rank is the same immutable logical/physical ingress episode used by
the ordinary provider.  Its first stage prepays leader-wire and ordinary
ingress-to-Candidate transfer, its structural component charges Candidate
fanout and exact Serve prefixes, and its dependency tail retains the physical
owner plus mode/capacity/runner/selector/lane/source path.  In particular,
`PopSelectedIngress` consumes an occurrence before any replacement Candidate
can replenish the inner budget.  The final component is the existing Stage
rank.  Replenishment and owner replacement are finite episode consumption,
never progress.
***************************************************************************)

HistoricalRunnerEpisodeKinds ==
  {"Stage3", "Stage4", "Stage6PreAdmission",
   "Stage6Owed", "Stage6NonCompletion"}

HistoricalRunnerEpisodeBaselineCarrier(kind) ==
  CASE kind = "Stage3" -> ReadyRunAuxCarrier
    [] kind = "Stage4" -> HistoricalTemporalStage4EpisodeCarrier
    [] kind \in {"Stage6PreAdmission", "Stage6Owed"} ->
         ReadyRunAuxCarrier
    [] kind = "Stage6NonCompletion" -> Stage4CapacityCarrier
    [] OTHER -> {}

HistoricalRunnerEpisodeTailCarrier(kind) ==
  HistoricalRunnerEpisodeBaselineCarrier(kind)

HistoricalRunnerEpisodeTailOrdering(kind) ==
  CASE kind = "Stage4" -> HistoricalTemporalStage4EpisodeOrdering
    [] kind = "Stage6NonCompletion" -> Stage4CapacityOrdering
    [] OTHER -> ReadyRunAuxOrdering

HistoricalRunnerEpisodeTailRank(kind, candidate) ==
  CASE kind = "Stage4" ->
         HistoricalTemporalStage4EpisodeRank(candidate)
    [] kind = "Stage6NonCompletion" ->
         Stage4CapacityRank(candidate.node)
    [] OTHER -> ReadyRunAuxRank(candidate.node)

HistoricalRunnerEpisodeResidual(
    kind, candidate, position, baselineRank) ==
  CASE kind = "Stage3" ->
         HistoricalTemporalStage3ServeEpisodeResidual(
           candidate, position, baselineRank)
    [] kind = "Stage4" ->
         HistoricalTemporalStage4ServeEpisodeResidual(
           candidate, position, baselineRank)
    [] kind = "Stage6PreAdmission" ->
         HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual(
           candidate, position, baselineRank)
    [] kind = "Stage6Owed" ->
         HistoricalTemporalStage6OwedRunnerEpisodeResidual(
           candidate, position, baselineRank)
    [] kind = "Stage6NonCompletion" ->
         HistoricalTemporalStage6NonCompletionServeEpisodeResidual(
           candidate, position, baselineRank)
    [] OTHER -> FALSE

HistoricalRunnerEpisodeGoal(kind, candidate, position, baselineRank) ==
  CASE kind = "Stage3" ->
         HistoricalTemporalStage3AuxProgress(
           candidate, position, baselineRank)
    [] kind = "Stage4" ->
         HistoricalTemporalStage4Progress(
           candidate, position, baselineRank)
    [] kind = "Stage6PreAdmission" ->
         HistoricalTemporalStage6PreAdmissionAuxProgress(
           candidate, position, baselineRank)
    [] kind = "Stage6Owed" ->
         HistoricalTemporalStage6OwedAuxProgress(
           candidate, position, baselineRank)
    [] kind = "Stage6NonCompletion" ->
         HistoricalTemporalStage6NonCompletionProgress(
           candidate, position, baselineRank)
    [] OTHER -> FALSE

HistoricalRunnerEpisodeRank(kind, candidate) ==
  <<AsyncProtectedCandidateIngressEpisodeRank(candidate),
       HistoricalRunnerEpisodeTailRank(kind, candidate)>>

HistoricalRunnerEpisodeRankCarrier(kind) ==
  AsyncProtectedCandidateIngressEpisodeRankCarrier
    \X HistoricalRunnerEpisodeTailCarrier(kind)

HistoricalRunnerEpisodeRankOrdering(kind) ==
  LexPairOrdering(
    AsyncProtectedCandidateIngressEpisodeRankOrdering,
    HistoricalRunnerEpisodeTailOrdering(kind),
    AsyncProtectedCandidateIngressEpisodeRankCarrier,
    HistoricalRunnerEpisodeTailCarrier(kind))

HistoricalRunnerEpisodeFairOwnerKinds ==
  {"HistoricalRunner", "HistoricalIoWorker"}

HistoricalRunnerEpisodeIoOwnerRequired(candidate) ==
  AsyncProtectedCandidateIoOwnerRequired(candidate)

HistoricalRunnerEpisodeFairOwner(candidate) ==
  IF HistoricalRunnerEpisodeIoOwnerRequired(candidate)
  THEN "HistoricalIoWorker"
  ELSE "HistoricalRunner"

HistoricalRunnerEpisodeFairAction(node, ownerKind) ==
  CASE ownerKind = "HistoricalRunner" ->
         PostGstRunHistoricalRecoveryNode(node)
    [] ownerKind = "HistoricalIoWorker" ->
         PostGstServiceHistoricalRecoveryIoWorker(node)
    [] OTHER -> FALSE

HistoricalRunnerEpisodeAtRank(
    kind, candidate, position, baselineRank, episodeRank) ==
  /\ HistoricalRunnerEpisodeResidual(
       kind, candidate, position, baselineRank)
  /\ HistoricalRunnerEpisodeRank(kind, candidate) = episodeRank

HistoricalRunnerEpisodeAtRankAndOwner(
    kind, candidate, position, baselineRank, episodeRank, ownerKind) ==
  /\ HistoricalRunnerEpisodeAtRank(
       kind, candidate, position, baselineRank, episodeRank)
  /\ HistoricalRunnerEpisodeFairOwner(candidate) = ownerKind

HistoricalRunnerEpisodeRankGoal(
    kind, candidate, position, baselineRank, episodeRank) ==
  \/ HistoricalRunnerEpisodeGoal(
       kind, candidate, position, baselineRank)
  \/ <<HistoricalRunnerEpisodeRank(kind, candidate), episodeRank>>
       \in HistoricalRunnerEpisodeRankOrdering(kind)

HistoricalRunnerEpisodeRankStepProperty(specification) ==
  specification
    => \A kind \in HistoricalRunnerEpisodeKinds:
         \A candidate, position:
           \A baselineRank \in
                 HistoricalRunnerEpisodeBaselineCarrier(kind),
              episodeRank \in HistoricalRunnerEpisodeRankCarrier(kind):
             HistoricalRunnerEpisodeAtRank(
               kind, candidate, position, baselineRank, episodeRank)
               ~> HistoricalRunnerEpisodeRankGoal(
                    kind, candidate, position, baselineRank, episodeRank)

HistoricalRunnerEpisodeClosureProperty(specification) ==
  specification
    => \A kind \in HistoricalRunnerEpisodeKinds:
         \A candidate, position:
           \A baselineRank \in
                HistoricalRunnerEpisodeBaselineCarrier(kind):
             HistoricalRunnerEpisodeResidual(
               kind, candidate, position, baselineRank)
               ~> HistoricalRunnerEpisodeGoal(
                    kind, candidate, position, baselineRank)

THEOREM HistoricalRunnerEpisodeRankOrderingIsWellFounded ==
  \A kind \in HistoricalRunnerEpisodeKinds:
    IsWellFoundedOn(
      HistoricalRunnerEpisodeRankOrdering(kind),
      HistoricalRunnerEpisodeRankCarrier(kind))
PROOF
  <1>1. ASSUME NEW kind \in HistoricalRunnerEpisodeKinds
         PROVE IsWellFoundedOn(
                 HistoricalRunnerEpisodeRankOrdering(kind),
                 HistoricalRunnerEpisodeRankCarrier(kind))
    <2>1. IsWellFoundedOn(
            AsyncProtectedCandidateIngressEpisodeRankOrdering,
            AsyncProtectedCandidateIngressEpisodeRankCarrier)
      BY AsyncProtectedCandidateIngressEpisodeRankOrderingIsWellFounded
    <2>2. IsWellFoundedOn(
            HistoricalRunnerEpisodeTailOrdering(kind),
            HistoricalRunnerEpisodeTailCarrier(kind))
      <3>1. CASE kind = "Stage4"
        BY <3>1, HistoricalTemporalStage4EpisodeOrderingIsWellFounded
           DEF HistoricalRunnerEpisodeTailOrdering,
               HistoricalRunnerEpisodeTailCarrier,
               HistoricalRunnerEpisodeBaselineCarrier
      <3>2. CASE kind = "Stage6NonCompletion"
        BY <3>2, Stage4CapacityOrderingIsWellFounded
           DEF HistoricalRunnerEpisodeTailOrdering,
               HistoricalRunnerEpisodeTailCarrier,
               HistoricalRunnerEpisodeBaselineCarrier
      <3>3. CASE /\ kind # "Stage4"
                  /\ kind # "Stage6NonCompletion"
        BY <3>3, ReadyRunAuxOrderingIsWellFounded
           DEF HistoricalRunnerEpisodeTailOrdering,
               HistoricalRunnerEpisodeTailCarrier,
               HistoricalRunnerEpisodeBaselineCarrier
      <3> QED BY <1>1, <3>1, <3>2, <3>3
           DEF HistoricalRunnerEpisodeKinds
    <2> QED BY <2>1, <2>2, WFLexPairOrdering
         DEF HistoricalRunnerEpisodeRankOrdering,
             HistoricalRunnerEpisodeRankCarrier
  <1> QED BY <1>1

THEOREM HistoricalRunnerEpisodeResidualFacts ==
  \A kind \in HistoricalRunnerEpisodeKinds:
    \A candidate, position:
      \A baselineRank \in HistoricalRunnerEpisodeBaselineCarrier(kind):
        HistoricalRunnerEpisodeResidual(
          kind, candidate, position, baselineRank)
          => /\ candidate \in AsyncCandidateSet
             /\ HistoricalProtectedCandidateOwned(candidate)
             /\ HistoricalRunnerEpisodeRank(kind, candidate)
                  \in HistoricalRunnerEpisodeRankCarrier(kind)
             /\ HistoricalRunnerEpisodeFairOwner(candidate)
                  \in HistoricalRunnerEpisodeFairOwnerKinds
BY AsyncProtectedCandidateIngressEpisodeRankIsFinite,
   ReadyRunAuxRankInCarrier, Stage4CapacityRankInCarrier,
   HistoricalTemporalStage4CarrierFacts, IsaT(1200)
   DEF HistoricalRunnerEpisodeKinds,
       HistoricalRunnerEpisodeBaselineCarrier,
       HistoricalRunnerEpisodeResidual,
       HistoricalRunnerEpisodeRank,
       HistoricalRunnerEpisodeRankCarrier,
       HistoricalRunnerEpisodeTailRank,
       HistoricalRunnerEpisodeTailCarrier,
       HistoricalRunnerEpisodeFairOwner,
       HistoricalRunnerEpisodeFairOwnerKinds,
       HistoricalRunnerEpisodeIoOwnerRequired,
       HistoricalTemporalStage3ServeEpisodeResidual,
       HistoricalTemporalStage4ServeEpisodeResidual,
       HistoricalTemporalStage6PreAdmissionRunnerEpisodeResidual,
       HistoricalTemporalStage6OwedRunnerEpisodeResidual,
       HistoricalTemporalStage6NonCompletionServeEpisodeResidual,
       HistoricalTemporalStage3Pending,
       HistoricalTemporalStage4Pending,
       HistoricalTemporalStage6Pending,
       HistoricalProtectedOwnedAtServiceRank,
       HistoricalProtectedCandidateOwned, ProtectedCandidateOwned

THEOREM HistoricalRunnerEpisodeStepIsGoalDescentOrFrame ==
  \A kind \in HistoricalRunnerEpisodeKinds:
    \A candidate, position:
      \A baselineRank \in HistoricalRunnerEpisodeBaselineCarrier(kind):
        /\ HistoricalRunnerEpisodeResidual(
             kind, candidate, position, baselineRank)
        /\ [AsyncNext]_AsyncAllVars
        => \/ HistoricalRunnerEpisodeGoal(
                kind, candidate, position, baselineRank)'
           \/ <<HistoricalRunnerEpisodeRank(kind, candidate)',
                 HistoricalRunnerEpisodeRank(kind, candidate)>>
                \in HistoricalRunnerEpisodeRankOrdering(kind)
           \/ /\ HistoricalRunnerEpisodeResidual(
                    kind, candidate, position, baselineRank)'
              /\ HistoricalRunnerEpisodeRank(kind, candidate)'
                   = HistoricalRunnerEpisodeRank(kind, candidate)
BY AsyncProtectedCandidateIngressEpisodeStepIsDescentOrFrame,
   AsyncCausalEpisodeIngressOwnerDepartureStrictlyDescends,
   HistoricalTemporalStage3SameRunnerAuxOutcome,
   HistoricalTemporalStage3OtherStepUnlessAuxDescent,
   HistoricalTemporalStage4SameRunnerProducesOutcome,
   HistoricalTemporalStage4OtherStepUnlessProgress,
   HistoricalTemporalStage6PreAdmissionSameRunnerOutcome,
   HistoricalTemporalStage6PreAdmissionOtherStep,
   HistoricalTemporalStage6OwedSameRunnerOutcome,
   HistoricalTemporalStage6OwedOtherStep,
   HistoricalTemporalStage6NonCompletionSameRunnerOutcome,
   HistoricalTemporalStage6NonCompletionOtherStep,
   IsaT(3600)
   DEF HistoricalRunnerEpisodeKinds,
       HistoricalRunnerEpisodeBaselineCarrier,
       HistoricalRunnerEpisodeResidual,
       HistoricalRunnerEpisodeGoal,
       HistoricalRunnerEpisodeRank,
       HistoricalRunnerEpisodeRankOrdering,
       HistoricalRunnerEpisodeTailRank,
       HistoricalRunnerEpisodeTailOrdering,
       LexPairOrdering, AsyncAllVars

THEOREM HistoricalRunnerEpisodeSelectedOwnerIsConcreteAndEnabled ==
  \A kind \in HistoricalRunnerEpisodeKinds:
    \A candidate, position:
      \A baselineRank \in HistoricalRunnerEpisodeBaselineCarrier(kind):
        HistoricalRunnerEpisodeResidual(
          kind, candidate, position, baselineRank)
          => /\ HistoricalRunnerEpisodeFairOwner(candidate)
                   \in HistoricalRunnerEpisodeFairOwnerKinds
             /\ ENABLED
                  <<HistoricalRunnerEpisodeFairAction(
                      candidate.node,
                      HistoricalRunnerEpisodeFairOwner(candidate))>>_AsyncAllVars
BY HistoricalRunnerEpisodeResidualFacts,
   HistoricalTemporalProtectedOwnerEnablesFairRunner,
   HistoricalRecoveryIoWorkerEnabledAfterGst,
   HistoricalTemporalQueuedIoServiceIsNonstuttering,
   ENABLEDaxioms, IsaT(1800)
   DEF HistoricalRunnerEpisodeFairOwner,
       HistoricalRunnerEpisodeFairOwnerKinds,
       HistoricalRunnerEpisodeFairAction,
       HistoricalRunnerEpisodeIoOwnerRequired,
       AsyncProtectedCandidateIoOwnerRequired,
       AsyncCandidateProducerContinuationFrozenServeIngressIdentities,
       CanResumeExactServeCapacity, AsyncServeJobQueued,
       AsyncServeLiveReservationOwned,
       AsyncIoQueueDepth, AsyncIoCapacity,
       HistoricalRecoveryTarget,
       PostGstServiceHistoricalRecoveryIoWorker,
       ServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, AsyncAllVars

THEOREM HistoricalRunnerEpisodeSelectedActionConsumesCell ==
  \A kind \in HistoricalRunnerEpisodeKinds:
    \A candidate, position:
      \A baselineRank \in HistoricalRunnerEpisodeBaselineCarrier(kind):
        /\ HistoricalRunnerEpisodeResidual(
             kind, candidate, position, baselineRank)
        /\ <<HistoricalRunnerEpisodeFairAction(
               candidate.node,
               HistoricalRunnerEpisodeFairOwner(candidate))>>_AsyncAllVars
        => \/ HistoricalRunnerEpisodeGoal(
                kind, candidate, position, baselineRank)'
           \/ <<HistoricalRunnerEpisodeRank(kind, candidate)',
                 HistoricalRunnerEpisodeRank(kind, candidate)>>
                \in HistoricalRunnerEpisodeRankOrdering(kind)
BY HistoricalRunnerEpisodeStepIsGoalDescentOrFrame,
   AsyncProtectedCandidateIngressEpisodeStepIsDescentOrFrame,
   AsyncCausalEpisodeIngressOwnerDepartureStrictlyDescends,
   AsyncProtectedCandidateSelectedServeOwnerGeometryIsComplete,
   ServiceIoWorkerDropsQueueDepth,
   HistoricalTemporalStage3SameRunnerAuxOutcome,
   HistoricalTemporalStage4SameRunnerProducesOutcome,
   HistoricalTemporalStage6PreAdmissionSameRunnerOutcome,
   HistoricalTemporalStage6OwedSameRunnerOutcome,
   HistoricalTemporalStage6NonCompletionSameRunnerOutcome,
   IsaT(3600)
   DEF HistoricalRunnerEpisodeKinds,
       HistoricalRunnerEpisodeBaselineCarrier,
       HistoricalRunnerEpisodeResidual,
       HistoricalRunnerEpisodeGoal,
       HistoricalRunnerEpisodeRank,
       HistoricalRunnerEpisodeRankOrdering,
       HistoricalRunnerEpisodeTailRank,
       HistoricalRunnerEpisodeTailOrdering,
       HistoricalRunnerEpisodeFairAction,
       HistoricalRunnerEpisodeFairOwner,
       HistoricalRunnerEpisodeIoOwnerRequired,
       AsyncProtectedCandidateIoOwnerRequired,
       PostGstRunHistoricalRecoveryNode,
       PostGstServiceHistoricalRecoveryIoWorker,
       ServiceIoWorkerWork, LexPairOrdering, AsyncAllVars

THEOREM HistoricalRunnerEpisodeOwnerPersistsInRankCell ==
  \A kind \in HistoricalRunnerEpisodeKinds:
    \A candidate, position:
      \A baselineRank \in HistoricalRunnerEpisodeBaselineCarrier(kind):
        /\ HistoricalRunnerEpisodeResidual(
             kind, candidate, position, baselineRank)
        /\ [AsyncNext]_AsyncAllVars
        /\ HistoricalRunnerEpisodeResidual(
             kind, candidate, position, baselineRank)'
        /\ HistoricalRunnerEpisodeRank(kind, candidate)'
             = HistoricalRunnerEpisodeRank(kind, candidate)
        => HistoricalRunnerEpisodeFairOwner(candidate)'
             = HistoricalRunnerEpisodeFairOwner(candidate)
BY AsyncCausalEpisodeTargetLifecycleOrdinalPersists,
   AsyncProtectedCandidateTargetPhysicalCutPersists,
   AsyncCausalEpisodeFrozenOriginsCannotReplenish,
   CandidateProducerContinuationFrozenServeCutCannotReplenish,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   IsaT(1800)
   DEF HistoricalRunnerEpisodeKinds,
       HistoricalRunnerEpisodeResidual,
       HistoricalRunnerEpisodeRank,
       HistoricalRunnerEpisodeFairOwner,
       HistoricalRunnerEpisodeIoOwnerRequired,
       AsyncProtectedCandidateIoOwnerRequired,
       AsyncProtectedCandidateIngressEpisodeRank,
       AsyncProtectedCandidateIngressEpisodeTailRank,
       AsyncCausalEpisodeFrozenIngressBarrierStageBudget,
       AsyncFrozenLeaderWireBarrierStageBudget,
       AsyncFrozenLeaderWireBarrierStageTokens,
       AsyncCandidateProducerContinuationFrozenPrefixRank,
       AsyncCandidateProducerContinuationFrozenProducerBudget,
       AsyncCandidateProducerContinuationFrozenProducerTokens,
       AsyncCandidateProducerContinuationFrozenCandidateTokens,
       AsyncCandidateProducerContinuationFrozenCandidateOwners,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenServeWorkBudget,
       AsyncCandidateProducerContinuationFrozenServeWorkTokens,
       AsyncCandidateProducerContinuationFrozenServeIngressIdentities,
       AsyncAllVars

THEOREM HistoricalRunnerEpisodeOwnerUsesAsyncFairness ==
  \A initialContext, node, ownerKind:
    /\ node \in Responsive
    /\ ownerKind \in HistoricalRunnerEpisodeFairOwnerKinds
    => AsyncSpecAt(initialContext)
         => WF_AsyncAllVars(
              HistoricalRunnerEpisodeFairAction(node, ownerKind))
BY Isa
   DEF HistoricalRunnerEpisodeFairOwnerKinds,
       HistoricalRunnerEpisodeFairAction,
       AsyncSpecAt, AsyncFairnessAt

THEOREM AsyncSpecProvidesHistoricalRunnerEpisodeRankStep ==
  \A initialContext:
    HistoricalRunnerEpisodeRankStepProperty(AsyncSpecAt(initialContext))
BY AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   HistoricalRunnerEpisodeResidualFacts,
   HistoricalRunnerEpisodeStepIsGoalDescentOrFrame,
   HistoricalRunnerEpisodeSelectedOwnerIsConcreteAndEnabled,
   HistoricalRunnerEpisodeSelectedActionConsumesCell,
   HistoricalRunnerEpisodeOwnerPersistsInRankCell,
   HistoricalRunnerEpisodeOwnerUsesAsyncFairness,
   PTL, IsaT(1200)
   DEF HistoricalRunnerEpisodeRankStepProperty,
       HistoricalRunnerEpisodeAtRank,
       HistoricalRunnerEpisodeAtRankAndOwner,
       HistoricalRunnerEpisodeRankGoal,
       HistoricalRunnerEpisodeFairOwnerKinds,
       HistoricalRunnerEpisodeFairAction

THEOREM AsyncSpecProvidesHistoricalRunnerEpisodeClosure ==
  \A initialContext:
    HistoricalRunnerEpisodeClosureProperty(AsyncSpecAt(initialContext))
BY AsyncSpecProvidesHistoricalRunnerEpisodeRankStep,
   HistoricalRunnerEpisodeResidualFacts,
   HistoricalRunnerEpisodeRankOrderingIsWellFounded,
   WellFoundedLeadsTo, PTL
   DEF HistoricalRunnerEpisodeClosureProperty,
       HistoricalRunnerEpisodeRankStepProperty,
       HistoricalRunnerEpisodeAtRank,
       HistoricalRunnerEpisodeRankGoal

THEOREM AsyncSpecProvidesHistoricalFiniteRunnerEpisodeClosure ==
  \A initialContext:
    HistoricalTemporalFiniteRunnerEpisodeClosureProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesHistoricalRunnerEpisodeClosure, PTL
   DEF HistoricalRunnerEpisodeClosureProperty,
       HistoricalRunnerEpisodeKinds,
       HistoricalRunnerEpisodeBaselineCarrier,
       HistoricalRunnerEpisodeResidual,
       HistoricalRunnerEpisodeGoal,
       HistoricalTemporalFiniteRunnerEpisodeClosureProperty,
       HistoricalTemporalStage6FiniteRunnerEpisodeClosureProperty

THEOREM AsyncSpecProvidesHistoricalProtectedServiceRankLeaves ==
  \A initialContext:
    HistoricalProtectedServiceRankLeafProperties(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesHistoricalFiniteRunnerEpisodeClosure,
   AsyncSpecClosesAllHistoricalTemporalCandidateStageLeaves,
   HistoricalTemporalCandidateStageLeavesAreExact
   DEF HistoricalTemporalCandidateStageLeaves

(***************************************************************************
Whole-origin fixed-clock Candidate closure.

The Stage projections above follow one immutable physical candidate.  The
fixed-clock packet kernel instead follows its logical causal origin, so it
must also account for a serviced parent being replaced by a finite batch of
strictly lower causal children.  The two frame lemmas below retain the exact
packet, logical owner, frozen physical set, and source work budget.  A frame
can persist only while the same protected candidate remains live.  Candidate
starvation therefore exposes either the exact lifecycle goal or a strict
causal-work decrease.  Equal-count replacement and count-increasing
replenishment remain the explicit non-descent residual; they are never called
progress.
***************************************************************************)

HistoricalDiscoveryCandidateCausalDagIntroducedOwners(
    packet, physicalKnown) ==
  {candidate \in HistoricalDiscoveryPacketCandidateOwners(packet):
     HistoricalDiscoveryCandidateExactPhysicalIdentity(candidate)
       \notin physicalKnown}

HistoricalDiscoveryCandidateCausalDagWitnessEpisode(
    node, clockValue, sourceRank, packet, known, budget,
    identity, candidate, occurrenceRank, physicalKnown,
    workBudget, witness) ==
  /\ HistoricalDiscoveryCandidateCausalDagBudgetFrontier(
       node, clockValue, sourceRank, packet, known, budget,
       identity, candidate, occurrenceRank, physicalKnown, workBudget)
  /\ witness
       \in HistoricalDiscoveryCandidateCausalDagIntroducedOwners(
            packet, physicalKnown)

THEOREM AsyncSpecProvidesHistoricalProtectedCandidateStarvation ==
  \A initialContext:
    HistoricalProtectedCandidateStarvationProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesHistoricalProtectedServiceRankLeaves,
   HistoricalProtectedServiceRankProgressFromStageLeaves,
   HistoricalProtectedServiceRankProgressImpliesStarvation, PTL

HistoricalDiscoveryTimedProtectedCandidateOwned(candidate) ==
  /\ candidate.node \in AsyncTimedServiceNodes
  /\ ProtectedCandidateOwned(candidate)

HistoricalDiscoveryTimedCandidateStarvationProperty(specification) ==
  specification
    => \A candidate \in AsyncCandidateSet:
         (gst /\ HistoricalDiscoveryTimedProtectedCandidateOwned(candidate))
           ~> ~HistoricalDiscoveryTimedProtectedCandidateOwned(candidate)

(***************************************************************************
Route-neutral Candidate starvation.

The selected global overdue packet need not target the historical-recovery
node whose clock is being proved.  A physical Candidate at its recipient is
therefore serviced by exactly one of two separately fair runner families:
the current-voter `PostGstRunNode` arm or the historical-target runner arm.
Once application moves the owner to archive mode, `ProtectedCandidateOwned`
is already false.  The proof composes those two starvation results across the
monotone owner-mode handoff; it never assumes fairness of their union.
***************************************************************************)

THEOREM AsyncSpecProvidesHistoricalDiscoveryTimedCandidateStarvation ==
  \A initialContext:
    HistoricalDiscoveryTimedCandidateStarvationProperty(
      AsyncSpecAt(initialContext))
BY StarvationFreedomObligation,
   AsyncSpecProvidesHistoricalProtectedCandidateStarvation,
   AsyncSpecAlwaysStrongTypeInvariant,
   HistoricalDiscoveryTimedOwnerModeCannotIncreaseAfterGst,
   PTL, IsaT(1800)
   DEF HistoricalDiscoveryTimedCandidateStarvationProperty,
       HistoricalDiscoveryTimedProtectedCandidateOwned,
       StarvationFreedomProperty,
       ResponsiveProtectedCandidateOwned,
       HistoricalProtectedCandidateStarvationProperty,
       HistoricalProtectedCandidateOwned,
       ProtectedCandidateOwned,
       HistoricalDiscoveryTimedOwnerMode,
       AsyncTimedServiceNodes, AsyncArchiveIoServiceNodes,
       AsyncResponsiveAppliedArchiveServers,
       AsyncResponsiveOnlineArchiveServers,
       AsyncResponsiveArchiveServers

THEOREM HistoricalDiscoveryExactRunnerStepIsGoalNonDescentOrFrame ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known:
      \A budget \in Nat:
        \A identity, candidate, occurrenceRank, physicalKnown:
          /\ HistoricalDiscoveryCandidateExactActionOwnerAtRank(
               node, clockValue, sourceRank, packet, known, budget,
               identity, candidate, occurrenceRank, physicalKnown)
          /\ [AsyncNext]_AsyncAllVars
          => \/ HistoricalDiscoveryCandidateServeLifecycleGoal(
                  node, clockValue, sourceRank,
                  packet, known, budget)'
             \/ HistoricalDiscoveryCandidateNonDescentEpisodeResidual(
                  node, clockValue, sourceRank, packet, known, budget,
                  identity, candidate,
                  occurrenceRank, physicalKnown)'
             \/ HistoricalDiscoveryCandidateExactActionOwnerAtRank(
                  node, clockValue, sourceRank, packet, known, budget,
                  identity, candidate,
                  occurrenceRank, physicalKnown)'
BY HistoricalDiscoveryTimedOwnerModeCannotIncreaseAfterGst,
   HistoricalDiscoveryRetainedPacketMinimumStepCases,
   HistoricalDiscoveryCandidateMinimumPersistenceKeepsTail,
   HistoricalDiscoveryCandidateMinimumExitClassifiesTail,
   HistoricalDiscoveryCandidateExitClassifiesOccurrenceDebt,
   HistoricalDiscoveryLowerCandidateInsertionReselectsLower,
   HistoricalDiscoveryCandidateDepartureRetainsLifecycleCoverage,
   HistoricalDiscoverySameGenerationCandidateServiceBlocksReentry,
   HistoricalDiscoveryServicedCandidateIdentityBlocksReentry,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateTerminalIdentityCannotReactivateAtGst,
   AsyncCausalEpisodeFrozenOriginsCannotReplenish,
   AsyncCommandSuccessorsStrictlyLowerRemainingWorkStage,
   IsaT(3600)
   DEF HistoricalDiscoveryCandidateExactActionOwnerAtRank,
       HistoricalDiscoveryCandidateNonDescentEpisodeResidual,
       HistoricalDiscoveryCandidateEqualCountOwnerReplacementResidual,
       HistoricalDiscoveryCandidateCountIncreasingReplenishmentResidual,
       HistoricalDiscoveryCandidateServeLifecycleGoal,
       HistoricalDiscoveryCandidateServeLifecycleDiscovery,
       HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget,
       HistoricalDiscoveryCandidateIntroducedPhysicalIdentitySet,
       HistoricalDiscoveryPacketCandidateExactPhysicalIdentitySet,
       HistoricalDiscoveryCandidateExactPhysicalIdentity,
       HistoricalDiscoveryCandidateFrozenPhysicalCoordinates,
       HistoricalDiscoveryCandidateExactRunnerAction,
       HistoricalDiscoveryCandidateExactRunnerActionKindCarrier,
       HistoricalDiscoveryCandidateExactRunnerKindForMode,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryTimedOwnerModeCarrier,
       HistoricalDiscoveryPacketCandidateOccurrenceDebtRank,
       HistoricalDiscoveryOccurrenceDebtOrdering,
       HistoricalDiscoveryOccurrenceDebtCarrier,
       HistoricalDiscoveryFixedClockStrictRankGoal,
       HistoricalDiscoveryConcreteFixedClockRank,
       AsyncTargetNeutralLifecycleDiscoveredOwnerSet,
       HistoricalDiscoveryTimedProtectedCandidateOwned,
       AsyncTimedServiceNodes,
       ProtectedCandidateOwned, CandidateScheduled,
       CandidateServiceRank, ServiceRankLess,
       LexPairOrdering, OpToRel, AsyncAllVars

THEOREM AsyncSpecProvidesHistoricalDiscoveryCandidateExactRunnerStep ==
  \A initialContext:
    HistoricalDiscoveryCandidateExactRunnerStepProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesHistoricalDiscoveryTimedCandidateStarvation,
   HistoricalDiscoveryExactRunnerStepIsGoalNonDescentOrFrame,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   PTL, IsaT(1800)
   DEF HistoricalDiscoveryCandidateExactRunnerStepProperty,
       HistoricalDiscoveryTimedCandidateStarvationProperty,
       HistoricalDiscoveryCandidateExactActionOwnerAtRank,
       HistoricalDiscoveryTimedProtectedCandidateOwned,
       ProtectedCandidateOwned

THEOREM HistoricalDiscoveryCausalDagFrontierHasProtectedWitness ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known:
      \A budget \in Nat:
        \A identity, candidate, occurrenceRank, physicalKnown:
          \A workBudget \in Nat:
            HistoricalDiscoveryCandidateCausalDagBudgetFrontier(
              node, clockValue, sourceRank, packet, known, budget,
              identity, candidate, occurrenceRank,
              physicalKnown, workBudget)
              => \E witness:
                   /\ HistoricalDiscoveryCandidateCausalDagWitnessEpisode(
                        node, clockValue, sourceRank, packet, known, budget,
                        identity, candidate, occurrenceRank, physicalKnown,
                        workBudget, witness)
                   /\ witness \in AsyncCandidateSet
                   /\ HistoricalDiscoveryTimedProtectedCandidateOwned(
                        witness)
BY StrongTypeHasFiniteHistoricalDiscoveryRankOwners,
   HistoricalDiscoveryPacketProducerCoverageStaysInFrozenCarrier,
   IsaT(1200)
   DEF HistoricalDiscoveryCandidateCausalDagWitnessEpisode,
       HistoricalDiscoveryCandidateCausalDagIntroducedOwners,
       HistoricalDiscoveryCandidateCausalDagBudgetFrontier,
       HistoricalDiscoveryCandidateNonDescentEpisodeResidual,
       HistoricalDiscoveryCandidateEqualCountOwnerReplacementResidual,
       HistoricalDiscoveryCandidateCountIncreasingReplenishmentResidual,
       HistoricalDiscoveryCandidateIntroducedPhysicalIdentitySet,
       HistoricalDiscoveryPacketCandidateExactPhysicalIdentitySet,
       HistoricalDiscoveryCandidateExactPhysicalIdentity,
       HistoricalDiscoveryPacketCandidateOwners,
       HistoricalDiscoveryTimedProtectedCandidateOwned,
       AsyncTimedServiceNodes, ProtectedCandidateOwned,
       HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget,
       HistoricalDiscoveryFixedClockPending

THEOREM HistoricalDiscoveryCausalDagWitnessStepIsGoalDescentOrFrame ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known:
      \A budget \in Nat:
        \A identity, candidate, occurrenceRank, physicalKnown:
          \A workBudget \in Nat:
            \A witness:
              /\ HistoricalDiscoveryCandidateCausalDagWitnessEpisode(
                   node, clockValue, sourceRank, packet, known, budget,
                   identity, candidate, occurrenceRank, physicalKnown,
                   workBudget, witness)
              /\ [AsyncNext]_AsyncAllVars
              => \/ HistoricalDiscoveryCandidateStrictCausalDagBudgetGoal(
                      node, clockValue, sourceRank, packet, known, budget,
                      identity, candidate, occurrenceRank,
                      physicalKnown, workBudget)'
                 \/ HistoricalDiscoveryCandidateCausalDagWitnessEpisode(
                      node, clockValue, sourceRank, packet, known, budget,
                      identity, candidate, occurrenceRank,
                      physicalKnown, workBudget, witness)'
BY HistoricalDiscoveryTimedOwnerModeCannotIncreaseAfterGst,
   HistoricalDiscoveryRetainedPacketMinimumStepCases,
   HistoricalDiscoveryCandidateDepartureRetainsLifecycleCoverage,
   HistoricalDiscoverySameGenerationCandidateServiceBlocksReentry,
   HistoricalDiscoveryServicedCandidateIdentityBlocksReentry,
   AsyncCandidateServiceTombstoneRejectsTransportReadmission,
   AsyncCandidateTerminalIdentityCannotReactivateAtGst,
   AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   CommandSuccessorsRetainCausalOrigin,
   AsyncCommandSuccessorsStrictlyLowerRemainingWorkStage,
   AsyncCommandSuccessorBatchStrictlyConsumesRemainingWork,
   AsyncCausalEpisodeFrozenOriginsCannotReplenish,
   AsyncCausalEpisodeServicedCandidateConsumesTopologicalWeight,
   FS_CardinalityType, FS_Subset, IsaT(3600)
   DEF HistoricalDiscoveryCandidateCausalDagWitnessEpisode,
       HistoricalDiscoveryCandidateCausalDagIntroducedOwners,
       HistoricalDiscoveryCandidateCausalDagBudgetFrontier,
       HistoricalDiscoveryCandidateStrictCausalDagBudgetGoal,
       HistoricalDiscoveryCandidateCausalWorkBudget,
       HistoricalDiscoveryCandidateCausalWorkTokenSet,
       HistoricalDiscoveryCandidateNonDescentEpisodeResidual,
       HistoricalDiscoveryCandidateEqualCountOwnerReplacementResidual,
       HistoricalDiscoveryCandidateCountIncreasingReplenishmentResidual,
       HistoricalDiscoveryCandidateServeLifecycleGoal,
       HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget,
       HistoricalDiscoveryCandidateIntroducedPhysicalIdentitySet,
       HistoricalDiscoveryPacketCandidateExactPhysicalIdentitySet,
       HistoricalDiscoveryCandidateExactPhysicalIdentity,
       HistoricalDiscoveryPacketCandidateOwners,
       AsyncCausalEpisodeCandidateWorkBudget,
       AsyncCausalEpisodeCandidateWorkTokens,
       AsyncCausalEpisodeCandidates,
       AsyncCausalEpisodeFrozenPredecessorOrigins,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryTimedOwnerModeCarrier,
       CandidateScheduled, CommandSuccessors,
       LexPairOrdering, OpToRel, SetLessThan, AsyncAllVars

THEOREM AsyncSpecProvidesHistoricalDiscoveryCandidateCausalDagBudgetDescent ==
  \A initialContext:
    HistoricalDiscoveryCandidateCausalDagBudgetDescentProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesHistoricalDiscoveryTimedCandidateStarvation,
   HistoricalDiscoveryCausalDagFrontierHasProtectedWitness,
   HistoricalDiscoveryCausalDagWitnessStepIsGoalDescentOrFrame,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   PTL, IsaT(2400)
   DEF HistoricalDiscoveryCandidateCausalDagBudgetDescentProperty,
       HistoricalDiscoveryCandidateCausalDagWitnessEpisode,
       HistoricalDiscoveryTimedCandidateStarvationProperty,
       HistoricalDiscoveryTimedProtectedCandidateOwned,
       ProtectedCandidateOwned

THEOREM AsyncSpecProvidesHistoricalDiscoveryCandidateExactRunnerService ==
  \A initialContext:
    HistoricalDiscoveryCandidateExactRunnerServiceProperty(
      AsyncSpecAt(initialContext))
BY AsyncSpecProvidesHistoricalDiscoveryCandidateExactRunnerStep,
   AsyncSpecProvidesHistoricalDiscoveryCandidateCausalDagBudgetDescent
   DEF HistoricalDiscoveryCandidateExactRunnerServiceProperty

(***************************************************************************
Exact fixed-clock Serve worker.

The worker kind is frozen by the source occurrence.  A historical-to-archive
handoff is a strict decrease of the existing finite mode rank; it cannot
silently switch the fair action being awaited.  At equal mode, the fixed
ordinary or historical I/O occurrence either consumes the exact FIFO debt or
leaves the same enabled owner framed for its existing weak-fairness clause.
***************************************************************************)

THEOREM HistoricalDiscoveryServeExactWorkerStepIsModeGoalOrFrame ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known:
      \A budget \in Nat:
        \A identity, job, occurrenceRank, workerKind, workerMode:
          /\ HistoricalDiscoveryServeExactActionOwnerAtRank(
               node, clockValue, sourceRank, packet, known, budget,
               identity, job, occurrenceRank, workerKind, workerMode)
          /\ [AsyncNext]_AsyncAllVars
          => \/ HistoricalDiscoveryServeExactWorkerModeProgressGoal(
                  node, clockValue, sourceRank, packet, known, budget,
                  identity, job, occurrenceRank, workerMode)'
             \/ HistoricalDiscoveryServeExactActionOwnerAtRank(
                  node, clockValue, sourceRank, packet, known, budget,
                  identity, job, occurrenceRank, workerKind, workerMode)'
BY HistoricalDiscoveryTimedOwnerModeCannotIncreaseAfterGst,
   HistoricalDiscoveryRetainedPacketMinimumStepCases,
   HistoricalDiscoveryServeMinimumPersistenceKeepsTail,
   HistoricalDiscoveryServeMinimumExitClassifiesTail,
   HistoricalDiscoveryServeExitEitherLowersOrReplenishes,
   HistoricalDiscoveryRetiredServeIdentityBlocksReentry,
   HistoricalDiscoveryServeDepartureInstallsDurableCoverage,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   IsaT(2400)
   DEF HistoricalDiscoveryServeExactActionOwnerAtRank,
       HistoricalDiscoveryServeExactWorkerModeProgressGoal,
       HistoricalDiscoveryServeExactWorkerModeHandoffResidual,
       HistoricalDiscoveryServeExactWorkerModeFrontier,
       HistoricalDiscoveryServeExactWorkerAction,
       HistoricalDiscoveryServeExactWorkerKindForMode,
       HistoricalDiscoveryServeExactWorkerModeOrdering,
       HistoricalDiscoveryServeExactWorkerModeCarrier,
       HistoricalDiscoveryCandidateServeLifecycleGoal,
       HistoricalDiscoveryCandidateServeLifecycleEpisodeAtBudget,
       HistoricalDiscoveryPacketServeOccurrenceDebtRank,
       HistoricalDiscoveryOccurrenceDebtOrdering,
       HistoricalDiscoveryOccurrenceDebtCarrier,
       HistoricalDiscoveryTimedOwnerMode,
       HistoricalDiscoveryTimedOwnerModeOrdering,
       HistoricalDiscoveryPacketServeOwners,
       HistoricalDiscoveryServeJobOwned,
       AsyncServeJobQueued, AsyncServeLifecycleTombstone,
       SetLessThan, OpToRel, LexPairOrdering, AsyncAllVars

THEOREM HistoricalDiscoveryServeExactFairActionConsumesModeCell ==
  \A node \in Responsive,
     clockValue \in Nat,
     sourceRank \in HistoricalDiscoveryFixedClockBlockerCarrier:
    \A packet, known:
      \A budget \in Nat:
        \A identity, job, occurrenceRank, workerKind, workerMode:
          /\ HistoricalDiscoveryServeExactActionOwnerAtRank(
               node, clockValue, sourceRank, packet, known, budget,
               identity, job, occurrenceRank, workerKind, workerMode)
          /\ HistoricalDiscoveryServeExactWorkerAction(packet, workerKind)
          => HistoricalDiscoveryServeExactWorkerModeProgressGoal(
               node, clockValue, sourceRank, packet, known, budget,
               identity, job, occurrenceRank, workerMode)'
BY HistoricalDiscoveryServeFairActionLowersOccurrenceDebt,
   HistoricalDiscoveryServeHeadFairServiceLowersOccurrenceDebt,
   HistoricalDiscoveryServeNonOwnerHeadFairServiceLowersMinimum,
   HistoricalDiscoveryServeExactRemovalLowersOccurrenceDebt,
   IsaT(1800)
   DEF HistoricalDiscoveryServeExactActionOwnerAtRank,
       HistoricalDiscoveryServeExactWorkerModeProgressGoal,
       HistoricalDiscoveryServeExactWorkerModeHandoffResidual,
       HistoricalDiscoveryServeExactWorkerModeFrontier,
       HistoricalDiscoveryServeExactWorkerAction,
       HistoricalDiscoveryPacketServeDebtFairAction,
       HistoricalDiscoveryCandidateServeLifecycleGoal,
       HistoricalDiscoveryPacketServeOccurrenceDebtRank,
       HistoricalDiscoveryOccurrenceDebtOrdering,
       HistoricalDiscoveryOccurrenceDebtCarrier,
       HistoricalDiscoveryPacketServeHeadIsOwner,
       HistoricalDiscoveryPacketServeHeadIsNotOwner,
       SetLessThan, OpToRel, LexPairOrdering, AsyncAllVars

THEOREM HistoricalDiscoveryServeExactWorkerUsesAsyncFairness ==
  \A initialContext, recipient, workerKind:
    /\ recipient \in Responsive
    /\ workerKind
         \in HistoricalDiscoveryServeExactWorkerActionKindCarrier
    => AsyncSpecAt(initialContext)
         => WF_AsyncAllVars(
              CASE workerKind = "ServiceIo" ->
                     PostGstServiceIoWorker(recipient)
                [] workerKind = "ServiceHistoricalIo" ->
                     PostGstServiceHistoricalRecoveryIoWorker(recipient)
                [] OTHER -> FALSE)
BY Isa
   DEF HistoricalDiscoveryServeExactWorkerActionKindCarrier,
       AsyncSpecAt, AsyncFairnessAt

THEOREM AsyncSpecProvidesHistoricalDiscoveryServeExactWorkerStep ==
  \A initialContext:
    HistoricalDiscoveryServeExactWorkerStepProperty(
      AsyncSpecAt(initialContext))
BY HistoricalDiscoveryServeExactWorkerStepIsModeGoalOrFrame,
   HistoricalDiscoveryServeExactFairActionConsumesModeCell,
   HistoricalDiscoveryServeExactWorkerUsesAsyncFairness,
   AsyncSpecAlwaysStrongTypeInvariant,
   AsyncSpecAlwaysProgressOwnershipInvariant,
   PTL, IsaT(1800)
   DEF HistoricalDiscoveryServeExactWorkerStepProperty,
       HistoricalDiscoveryServeExactActionOwnerAtRank,
       HistoricalDiscoveryServeExactWorkerAction,
       HistoricalDiscoveryServeExactWorkerActionKindCarrier

=============================================================================
