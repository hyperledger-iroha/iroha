---- MODULE SumeragiV2AsyncTemporalClosureProofs ----
EXTENDS SumeragiV2HeightResetBoundaryClosureProofs,
        SumeragiV2AdequateLeaderServiceClosureProofs,
        SumeragiV2ExactDecisionStageServiceClosureProofs

(***************************************************************************
Release-facing temporal closure.

The rank, starvation, and complete indexed progress-witness preservation
obligations are proved in the lower closure modules.  In particular, the
Decision and historical locked-body witnesses now survive every runner,
non-runner, crash, restart, and replay arm before projecting to the open
progress kernel.  Timeout/view progress and locked-body reproposal are not
independent scheduler debts: both admit Decision as a terminal outcome and
are proved below from the same responsive-Decision convergence.  Rotating
leader progress is reduced to the exact adequate-leader service kernel,
application completion is reduced to the exact Decision-stage service
pipeline retained by the final witness layer.  Height productivity is closed
by the exact tick-blocked Runtime proof: weak fairness returns every undecided
responsive voter to Local, while durable Decision receipts accumulate over the
finite frozen roster.  The two declarations without proof are the complete
remaining abstract temporal debt.  The leader declaration is the conjunction
of exact scheduler-origin readiness, physical transport/ordinary-runner/
timeout-quorum convergence, and fixed-identity semantic phase composition;
the recipient-local certified-response capacity arm is already proved from
the dedicated runtime slot and fair claim runner.  The Decision declaration
contains only its exact off-scheduler residuals.  The Decision-local Stage-2
Busy owner and protected Serve FIFO starvation are proved in their dedicated
closure leaf.  Keeping the open declarations above their eventual
proof-bearing leaves prevents a strict leaf run from accepting either broad
derived claim as an imported fact.
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
  /\ AdequateLeaderSemanticCompositionProperty(specification)

THEOREM AdequateLeaderExactClosureResidualObligation ==
  \A initialContext:
    AdequateLeaderExactClosureResidualProperty(AsyncLiveSpecAt(initialContext))

THEOREM AdequateLeaderServiceKernelObligation ==
  \A initialContext:
    AdequateLeaderServiceKernelProperty(AsyncLiveSpecAt(initialContext))
BY AdequateLeaderExactClosureResidualObligation,
   ExactAdequateLeaderSubkernelsReduceToServiceKernel
   DEF AdequateLeaderExactClosureResidualProperty

THEOREM AsyncTemporalClosureRotatingLeaderProgressObligation ==
  \A initialContext:
    RotatingLeaderProgressProperty(AsyncLiveSpecAt(initialContext))
BY AdequateLeaderServiceKernelObligation,
   AdequateLeaderServiceKernelSuppliesRotatingLeaderProgress

THEOREM AsyncTemporalClosureTimeoutViewProgressObligation ==
  \A initialContext:
    TimeoutViewProgressProperty(AsyncLiveSpecAt(initialContext))
BY AsyncTemporalClosureRotatingLeaderProgressObligation,
   RotatingLeaderProgressPropertyImpliesTimeoutViewProgressProperty

THEOREM AsyncTemporalClosureLockedBodyReproposalProgressObligation ==
  \A initialContext:
    LockedBodyReproposalProgressProperty(AsyncLiveSpecAt(initialContext))
BY AsyncTemporalClosureRotatingLeaderProgressObligation,
   RotatingLeaderProgressClosesLockedBodyReproposal

THEOREM ExactDecisionOffSchedulerResidualConvergenceObligation ==
  \A initialContext:
    ExactDecisionOffSchedulerResidualConvergenceProperty(
      AsyncSpecAt(initialContext))

THEOREM ExactDecisionStageServiceObligation ==
  \A initialContext:
    ExactDecisionStageServiceProperty(AsyncSpecAt(initialContext))
BY ExactDecisionOffSchedulerResidualConvergenceObligation,
   ExactDecisionOffSchedulerResidualConvergenceDischargesStageService

THEOREM AsyncTemporalClosureApplicationCompletionProgressObligation ==
  \A initialContext:
    ApplicationCompletionProgressProperty(AsyncSpecAt(initialContext))
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
