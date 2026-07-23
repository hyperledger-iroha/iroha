---- MODULE SumeragiV2ProgressWitnessCrossToolScratch ----
EXTENDS SumeragiV2AsyncLivenessProofs

(***************************************************************************
Scratch model-side half of the production progress-witness refinement.

The six production propositions are discharged independently by source-bound
Rust/Verus evidence.  This conjunct records the non-vacuous abstract target
which those production traces refine: typed and disjoint scheduler ownership,
the exact durable-Commit and durable-Decision lifecycles, and stability of a
terminal per-node application receipt.  It intentionally does not assume the
still-open temporal ProgressWitnessObligation.
***************************************************************************)
ProgressWitnessAbstractOwnerProjection ==
  /\ \A initialContext:
       AsyncSpecAt(initialContext) => []StrongInductiveInvariant
  /\ \A initialContext:
       AsyncSpecAt(initialContext) => []AsyncProgressOwnershipInvariant
  /\ \A initialContext:
       GenerationScopedVoteDeliveryProperty(AsyncSpecAt(initialContext))
  /\ \A initialContext:
       DecisionRecoveryAcrossRestartProperty(AsyncSpecAt(initialContext))
  /\ \A node:
       NodeHasApplication(node)
         /\ [AsyncNext]_AsyncAllVars
         => NodeHasApplication(node)'

THEOREM ProgressWitnessAbstractOwnerProjectionObligation ==
  ProgressWitnessAbstractOwnerProjection
PROOF
  <1>1. \A initialContext:
           AsyncSpecAt(initialContext) => []StrongInductiveInvariant
    BY StrongInductiveInvariantFromAsyncSpec
  <1>2. \A initialContext:
           AsyncSpecAt(initialContext) => []AsyncProgressOwnershipInvariant
    BY AsyncSpecAlwaysProgressOwnershipInvariant
  <1>3. \A initialContext:
           GenerationScopedVoteDeliveryProperty(
             AsyncSpecAt(initialContext))
    BY GenerationScopedVoteDeliveryObligation
  <1>4. \A initialContext:
           DecisionRecoveryAcrossRestartProperty(
             AsyncSpecAt(initialContext))
    BY DecisionRecoveryAcrossRestartObligation
  <1>5. \A node:
           NodeHasApplication(node)
             /\ [AsyncNext]_AsyncAllVars
             => NodeHasApplication(node)'
    BY AsyncBracketStepPreservesNodeApplication
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5
       DEF ProgressWitnessAbstractOwnerProjection

ProgressWitnessProductionRefinementTarget ==
  /\ ProductionProgressWitnessTraceRefinement
  /\ ProgressWitnessAbstractOwnerProjection

THEOREM ProgressWitnessCrossToolRefinement ==
  ProductionProgressWitnessTraceRefinement
    => ProgressWitnessProductionRefinementTarget
PROOF
  BY ProgressWitnessAbstractOwnerProjectionObligation
     DEF ProgressWitnessProductionRefinementTarget

=============================================================================
