---- MODULE SumeragiV2AsyncTemporalClosureProofs ----
EXTENDS SumeragiV2HeightResetBoundaryClosureProofs,
        SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs,
        SumeragiV2ExactDecisionStageServiceClosureProofs

(***************************************************************************
Release-facing temporal closure.

The rank, starvation, and complete indexed progress-witness preservation
obligations are proved in the lower closure modules.  In particular, the
Decision and historical locked-body witnesses now survive every runner,
non-runner, crash, restart, and replay arm before projecting to the open
progress kernel.  Timeout/view progress is the first explicit temporal leaf.
The direct retained-lock shard closes one finite semantic producer episode and
returns a strict higher-view handoff without using a bounded maximum-view
rank.  Rotating-leader progress is reduced independently to the exact
adequate-leader service kernel; only after that acyclic convergence proof
closes does this module discharge retained-lock reproposal through its
legitimate Decision outcome.  Thus an unbounded sequence of higher views is
not called local rank progress, and the adequate-leader kernel cannot consume
the retained-lock release theorem.  Separately, application completion is
reduced to the exact Decision-stage service
pipeline retained by the final witness layer.  Height productivity is closed
by the exact tick-blocked Runtime proof: weak fairness returns every undecided
responsive voter to Local, while durable Decision receipts accumulate over the
finite frozen roster.  The leader closure below is the conjunction
of exact scheduler-origin readiness, physical transport/ordinary-runner/
timeout-quorum convergence, and target/context/leader/view/subject-indexed
semantic phase composition.  Its occurrence rank counts every distinct
target/leader owner at the frozen semantic rank; equal-count replacement and
count-increasing producer replenishment remain explicit non-progress debt.
The exact Decision residual below is composed only from the five direct proof
leaves: request clock owner, Runtime prefix, request head/gate owner, request
admission/coalescing, and nonphysical nonclaim response head/gate owner.  The
Decision-local Stage-2 Busy owner and protected Serve FIFO starvation remain
proved in their dedicated closure leaf.  No aggregate Decision claim is fed
back into those leaves.
***************************************************************************)

THEOREM ProgressWitnessObligation ==
  \A initialContext:
    AsyncProgressWitnessAndHistoricalRecoveryProperty(AsyncSpecAt(initialContext))
BY FinalProgressWitnessObligation

(***************************************************************************
The production refinement is declared only after the complete model-level
progress witness has closed.  This placement avoids using the earlier
model-projection seam as a substitute for the temporal theorem.
***************************************************************************)
ProgressWitnessProductionRefinementObligation ==
  /\ ProductionProgressWitnessTraceRefinement
  /\ ProgressWitnessObligation

THEOREM ProgressWitnessCrossToolRefinement ==
  ProductionProgressWitnessTraceRefinement
    => ProgressWitnessProductionRefinementObligation
PROOF
  BY ProgressWitnessObligation
     DEF ProgressWitnessProductionRefinementObligation

THEOREM HistoricalLockedBodyRecoveryProductionRefinementFromReviewedSeams ==
  /\ EffectiveLockBodyAcquisitionProductionRefinementObligation
  /\ ProgressWitnessProductionRefinementObligation
  => ProductionHistoricalLockedBodyRecoveryRefinement
PROOF
  BY DEF EffectiveLockBodyAcquisitionProductionRefinementObligation,
         ProgressWitnessProductionRefinementObligation,
         ProductionEffectiveLockBodyAcquisitionRefinement,
         ProductionProgressWitnessTraceRefinement,
         ProductionHistoricalLockedBodyRecoveryRefinement

THEOREM HeightResetIngressOwnershipResidualConvergenceObligation ==
  \A initialContext:
    HeightResetIngressOwnershipResidualProperty(
      AsyncLiveSpecAt(initialContext))
BY HeightResetIngressOwnershipResidualConvergence

THEOREM HeightProductivityResetBoundaryObligation ==
  \A initialContext:
    HeightProductivityResetBoundaryProperty(
      AsyncLiveSpecAt(initialContext))
BY HeightResetIngressOwnershipResidualConvergenceObligation,
   IngressResidualCoverageImpliesResetBoundaryCoverage

THEOREM HeightProductivityFrontierObligation ==
  \A initialContext:
    HeightProductivityFrontierProperty(AsyncLiveSpecAt(initialContext))
BY HeightProductivityResetBoundaryObligation,
   ResetBoundaryCoverageImpliesHeightProductivityFrontier

AdequateLeaderExactClosureResidualProperty(specification) ==
  /\ AdequateLeaderExactResidualKernelProperty(specification)
  /\ AdequateLeaderLocalTargetDecisionConvergenceProperty(specification)

THEOREM AdequateLeaderFixedDeadlineAndDisseminationSupplyLocalConvergence ==
  \A initialContext:
    AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderLocalTargetDecisionConvergenceProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesLocalFreshSelfCorridorExposure,
   AdequateLeaderFixedDeadlineAndDisseminationSupplyLocalTargetConvergence

THEOREM AdequateLeaderFixedDeadlineAndDisseminationCloseExactResidual ==
  \A initialContext:
    AdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty(
      AsyncLiveSpecAt(initialContext))
      => AdequateLeaderExactClosureResidualProperty(
           AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesAdequateLeaderExactResidualKernel,
   AdequateLeaderFixedDeadlineAndDisseminationSupplyLocalConvergence,
   PTL
   DEF AdequateLeaderExactClosureResidualProperty

THEOREM AdequateLeaderExactClosureResidualObligation ==
  \A initialContext:
    AdequateLeaderExactClosureResidualProperty(AsyncLiveSpecAt(initialContext))
BY AsyncLiveSpecSuppliesAdequateLeaderFixedDeadlineAndResponsiveDissemination,
   AdequateLeaderFixedDeadlineAndDisseminationCloseExactResidual,
   PTL
   DEF AdequateLeaderExactClosureResidualProperty,
       AdequateLeaderExactResidualKernelProperty,
       AdequateLeaderLocalTargetDecisionConvergenceProperty

THEOREM AdequateLeaderServiceKernelObligation ==
  \A initialContext:
    AdequateLeaderServiceKernelProperty(AsyncLiveSpecAt(initialContext))
BY AdequateLeaderExactClosureResidualObligation,
   AdequateLeaderLocalTargetConvergenceSuppliesDecisionConvergence,
   PTL
   DEF AdequateLeaderExactClosureResidualProperty,
       ResponsiveDecisionConvergenceProperty,
       AdequateLeaderServiceKernelProperty

THEOREM AsyncTemporalClosureRotatingLeaderProgressObligation ==
  \A initialContext:
    RotatingLeaderProgressProperty(
      AsyncLiveSpecAt(initialContext))
BY AdequateLeaderServiceKernelObligation,
   AdequateLeaderServiceKernelSuppliesRotatingLeaderProgress

THEOREM DirectTimeoutViewClosureResidualObligation ==
  \A initialContext:
    DirectTimeoutViewClosureResidualProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncLiveProvidesDirectTimeoutViewClosureResidual

THEOREM AsyncTemporalClosureTimeoutViewProgressObligation ==
  \A initialContext:
    TimeoutViewProgressProperty(AsyncLiveSpecAt(initialContext))
BY DirectTimeoutViewClosureResidualObligation,
   DirectTimeoutViewDecompositionClosesTimeoutViewProgress

THEOREM AsyncTemporalClosureTimeoutViewProgressReduction ==
  /\ (\A initialContext:
        ProtectedServiceFiniteRunnerEpisodeClosureProperty(
          AsyncSpecAt(initialContext)))
  /\ DirectTimeoutViewClosureResidualObligation
    => AsyncTemporalClosureTimeoutViewProgressObligation
BY DirectTimeoutViewDecompositionClosesTimeoutViewProgress
   DEF DirectTimeoutViewClosureResidualObligation,
       AsyncTemporalClosureTimeoutViewProgressObligation

(***************************************************************************
Timeout contributes one target-local step to rotating-view exposure.  It
does not by itself claim that an adequate leader window has been reached:
that finite-roster composition remains in the adequate-leader closure.  The
explicit boundary below prevents consumers from silently replacing repeated
strict view advance with an already-frozen leader corridor.
***************************************************************************)

AdequateLeaderLocalTimeoutViewStepProperty(specification) ==
  specification
    => \A target \in AsyncCurrentResponsiveVoters,
          roundView \in Views:
         (/\ AdequateLeaderLocalTargetDecisionSource(target)
          /\ nodeView[target] = roundView)
           ~> (NodeHasDecision(target)
                \/ nodeView[target] > roundView)

THEOREM TimeoutViewProgressSuppliesAdequateLeaderLocalViewStep ==
  \A specification:
    TimeoutViewProgressProperty(specification)
      => AdequateLeaderLocalTimeoutViewStepProperty(specification)
BY Isa, PTL
   DEF TimeoutViewProgressProperty,
       AdequateLeaderLocalTimeoutViewStepProperty,
       AdequateLeaderLocalTargetDecisionSource

THEOREM AsyncTemporalTimeoutSuppliesAdequateLeaderLocalViewStep ==
  \A initialContext:
    AdequateLeaderLocalTimeoutViewStepProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncTemporalClosureTimeoutViewProgressObligation,
   TimeoutViewProgressSuppliesAdequateLeaderLocalViewStep

THEOREM ResponsiveDecisionConvergenceClosesLockedBodyReproposal ==
  \A initialContext:
    ResponsiveDecisionConvergenceProperty(
      AsyncLiveSpecAt(initialContext))
      => LockedBodyReproposalProgressProperty(
           AsyncLiveSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext,
                ResponsiveDecisionConvergenceProperty(
                  AsyncLiveSpecAt(initialContext))
         PROVE LockedBodyReproposalProgressProperty(
                 AsyncLiveSpecAt(initialContext))
    <2>1. ASSUME AsyncLiveSpecAt(initialContext)
           PROVE \A node \in ValidatorIds,
                     lockedRound \in Views,
                     subject \in Subjects:
                  StableAvailableRetainedLock(
                    node, lockedRound, subject)
                    ~> LockedBodyReproposalOutcome(
                         node, lockedRound, subject)
      <3>1. AsyncSpecAt(initialContext)
        BY <2>1, AsyncLiveSpecProjectsAsyncSpec
      <3>2. [](AsyncCurrentResponsiveVoters
                 = AsyncVotersAt(initialContext))
        BY <3>1, AsyncSpecAlwaysUsesFixedResponsiveVoters
      <3>3. ASSUME NEW node \in ValidatorIds,
                    NEW lockedRound \in Views,
                    NEW subject \in Subjects
             PROVE StableAvailableRetainedLock(
                     node, lockedRound, subject)
                     ~> LockedBodyReproposalOutcome(
                          node, lockedRound, subject)
        <4>1. (gst /\ ~ResponsiveNodesDecide)
                 ~> ResponsiveNodesDecide
          BY <1>1, <2>1
             DEF ResponsiveDecisionConvergenceProperty
        <4>2. [](StableAvailableRetainedLock(
                    node, lockedRound, subject)
                   => node \in AsyncVotersAt(initialContext))
          BY <3>2, PTL
             DEF StableAvailableRetainedLock
        <4>3. []((node \in AsyncVotersAt(initialContext)
                    /\ ResponsiveNodesDecide)
                   => LockedBodyReproposalOutcome(
                        node, lockedRound, subject))
          BY <3>2, PTL
             DEF ResponsiveNodesDecide,
                 LockedBodyReproposalOutcome,
                 LockedBodyLegitimatelyDecidedOrSuperseded
        <4>4. (StableAvailableRetainedLock(
                    node, lockedRound, subject)
                   /\ ~ResponsiveNodesDecide)
                 ~> LockedBodyReproposalOutcome(
                      node, lockedRound, subject)
          BY <4>1, <4>2, <4>3, PTL
             DEF StableAvailableRetainedLock
        <4>5. (StableAvailableRetainedLock(
                    node, lockedRound, subject)
                   /\ ResponsiveNodesDecide)
                 => LockedBodyReproposalOutcome(
                      node, lockedRound, subject)
          BY <3>2
             DEF StableAvailableRetainedLock,
                 ResponsiveNodesDecide,
                 LockedBodyReproposalOutcome,
                 LockedBodyLegitimatelyDecidedOrSuperseded
        <4> QED BY <4>4, <4>5, PTL
      <3> QED BY <3>3
    <2> QED BY <2>1
         DEF LockedBodyReproposalProgressProperty
  <1> QED BY <1>1

THEOREM AsyncTemporalClosureLockedBodyReproposalProgressObligation ==
  \A initialContext:
    LockedBodyReproposalProgressProperty(
      AsyncLiveSpecAt(initialContext))
BY AsyncTemporalClosureRotatingLeaderProgressObligation,
   RotatingLeaderProgressSuppliesResponsiveDecisionConvergence,
   ResponsiveDecisionConvergenceClosesLockedBodyReproposal

THEOREM ExactDecisionOffSchedulerResidualConvergenceObligation ==
  \A initialContext:
    ExactDecisionOffSchedulerResidualConvergenceProperty(
      AsyncSpecAt(initialContext))
BY ExactDecisionRequestClockOwnerConvergence,
   ExactDecisionRequestRuntimePrefixConvergence,
   ExactDecisionRequestHeadGateOwnerConvergence,
   ExactDecisionRequestAdmissionCoalescingOutcomeIsDischarged,
   ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergence
   DEF ExactDecisionOffSchedulerResidualConvergenceProperty

THEOREM ExactDecisionStageServiceObligation ==
  \A initialContext:
    ExactDecisionStageServiceProperty(AsyncSpecAt(initialContext))
BY AsyncSpecProvidesProtectedServiceFiniteRunnerEpisodeClosure,
   ExactDecisionOffSchedulerResidualConvergenceObligation,
   ExactDecisionOffSchedulerResidualConvergenceDischargesStageService

THEOREM AsyncTemporalClosureApplicationCompletionProgressObligation ==
  \A initialContext:
    ApplicationCompletionProgressProperty(
      AsyncSpecAt(initialContext))
BY ExactDecisionStageServiceObligation,
   ApplicationCompletionProgressReduction

THEOREM AsyncTemporalClosureApplicationLivenessObligation ==
  \A initialContext:
    ApplicationLivenessProperty(AsyncSpecAt(initialContext))
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE ApplicationLivenessProperty(AsyncSpecAt(initialContext))
    <2>1. ApplicationCompletionProgressProperty(
             AsyncSpecAt(initialContext))
      BY AsyncTemporalClosureApplicationCompletionProgressObligation
    <2>2. AsyncSpecAt(initialContext)
            => (gst /\ ResponsiveNodesDecide) ~> ResponsiveNodesApply
      BY <2>1, ApplicationCompletionProgressImpliesAggregateApplication, PTL
    <2> QED BY <2>1, <2>2, PTL
         DEF ApplicationCompletionProgressProperty,
             ApplicationLivenessProperty
  <1> QED BY <1>1

THEOREM AsyncTemporalClosureOneHeightCompletionObligation ==
  \A initialContext:
    OneHeightCompletionLiveness(initialContext)
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE OneHeightCompletionLiveness(initialContext)
    <2>1. RotatingLeaderProgressProperty(
             AsyncLiveSpecAt(initialContext))
      BY AsyncTemporalClosureRotatingLeaderProgressObligation
    <2>2. ApplicationLivenessProperty(AsyncSpecAt(initialContext))
      BY AsyncTemporalClosureApplicationLivenessObligation
    <2> QED BY <2>1, <2>2,
         OneHeightCompletionFromProgressProperties, PTL
         DEF OneHeightCompletionLiveness
  <1> QED BY <1>1

=============================================================================
