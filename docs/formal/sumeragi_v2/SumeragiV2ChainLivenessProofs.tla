---- MODULE SumeragiV2ChainLivenessProofs ----
EXTENDS SumeragiV2SuccessorActivationRefinementProofs, TLAPS

(***************************************************************************
Chain-level temporal composition.

This module is deliberately a child of the successor-activation proof module.
The indexed chain product is defined by SumeragiV2ChainEpochRefinement, while
the child imported above proves successor-activation starvation freedom.  The
former parent module therefore no longer needs an impossible dependency on a
theorem declared only by its child.

Historical recovery has three real phases at this boundary:

  authenticated source ready --fair OpenHistoricalRecovery--> exact target
  exact target              --Async recovery corridor-------> Decision
  durable Decision          --Async completion corridor-----> application

Only the first arrow is a chain-wrapper action.  Its weak fairness is already
part of IndexedFairness and is proved below.  The latter two arrows are the
exact missing Async temporal prerequisites.  They are named as properties,
not constants or assumptions, so this module cannot manufacture them from a
source-fidelity check or from the successor safety/refinement seam.

SumeragiV2AsyncHistoricalRecoveryLivenessProofs states the corresponding
post-GST, all-Responsive Async properties and proves only their conditional
rank/discovery composition.  This indexed module deliberately keeps the
stronger wrapper properties as prerequisites until that child closes its named
corridor premises and the parameterized exact-instance lift is proved.
***************************************************************************)

HistoricalRecoveryOpenOutcome(initialContext, node) ==
  \/ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
  \/ IndexedAsync(initialContext)!NodeHasDecision(node)
  \/ IndexedAsync(initialContext)!NodeHasApplication(node)

IndexedHistoricalRecoveryTargetDecisionProgress ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
      ~> IndexedAsync(initialContext)!NodeHasDecision(node)

IndexedResponsiveDecisionApplicationProgress ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedAsync(initialContext)!NodeHasDecision(node)
      ~> IndexedAsync(initialContext)!NodeHasApplication(node)

IndexedHistoricalRecoveryAsyncTemporalPrerequisites ==
  /\ IndexedHistoricalRecoveryTargetDecisionProgress
  /\ IndexedResponsiveDecisionApplicationProgress

IndexedHistoricalRecoveryEligibilityProgress ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    HistoricalRecoveryOutstanding(initialContext, node)
      ~> HistoricalRecoveryProgressEligible(initialContext, node)

IndexedHistoricalRecoveryTemporalPrerequisites ==
  /\ IndexedHistoricalRecoveryEligibilityProgress
  /\ IndexedHistoricalRecoveryAsyncTemporalPrerequisites

(***************************************************************************
An authenticated exact source is durable, joined membership is monotone, and
responsive nodes cannot crash after GST.  Consequently the exact open guard
persists until either the target is opened or a later recovery phase has
already produced Decision/application evidence.
***************************************************************************)
THEOREM IndexedHistoricalRecoveryReadyPersistsOrOpens ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedCompositionInvariant
      /\ IndexedHistoricalRecoveryReady(initialContext, node)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ IndexedHistoricalRecoveryReady(initialContext, node)'
         \/ HistoricalRecoveryOpenOutcome(initialContext, node)'
BY Isa, IndexedStepPreservesCompositionInvariant,
   JoinedMembershipIsMonotone,
   IndexedBracketStepKeepsNodeHeightsMonotone
   DEF IndexedCompositionInvariant,
       IndexedHistoricalRecoveryReady,
       IndexedHistoricalRecoveryTargetReady,
       IndexedHistoricalRecoverySourceReady,
       HistoricalRecoveryOpenOutcome,
       IndexedChainNext, IndexedChainVars,
       IndexedProductActionAt, IndexedReceiptClassification,
       IndexedReceiptFreeChainStutter,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       IndexedSuccessorActivationProgressStep,
       IndexedJoinedAsyncNext, IndexedJoinedNonCrashStep,
       IndexedJoinedRunnerStep, IndexedJoinedNonRunnerStep,
       IndexedOpenHistoricalRecovery,
       IndexedAsync!NodeHasDecision,
       IndexedAsync!NodeHasApplication,
       IndexedAsync!HistoricalRecoveryTarget,
       ExactNodeLocationAt

THEOREM IndexedHistoricalRecoveryReadyEnablesExactOpen ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedCompositionInvariant
      /\ IndexedHistoricalRecoveryReady(initialContext, node)
      => ENABLED
           <<IndexedOpenHistoricalRecoveryStep(initialContext, node)>>_(
             IndexedChainVars)
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              IndexedCompositionInvariant,
              IndexedHistoricalRecoveryReady(initialContext, node)
         PROVE ENABLED
                 <<IndexedOpenHistoricalRecoveryStep(
                     initialContext, node)>>_(IndexedChainVars)
    <2>1. initialContext \in JoinedContexts
      BY <1>1 DEF IndexedHistoricalRecoveryReady,
                    IndexedHistoricalRecoverySourceReady
    <2>2. ENABLED IndexedAsync(initialContext)!
                    PostGstOpenHistoricalRecovery(node)
      BY <1>1, ExpandENABLED, Isa
         DEF IndexedHistoricalRecoveryReady,
             IndexedHistoricalRecoveryTargetReady,
             IndexedHistoricalRecoverySourceReady,
             IndexedAsync!PostGstOpenHistoricalRecovery,
             IndexedAsync!OpenHistoricalRecovery,
             IndexedAsync!HistoricalRecoverySourceReady,
             IndexedAsync!HistoricalRecoveryTarget,
             IndexedAsync!NodeHasDecision,
             IndexedAsync!NodeHasApplication,
             IndexedAsync!AsyncNonRunnerOuterFrame,
             IndexedAsync!AsyncAllVars,
             IndexedAsync!AsyncSchedulerExceptHistoricalRecoveryTargets,
             IndexedAsync!vars
    <2>3. ENABLED IndexedOpenHistoricalRecoveryStep(
                    initialContext, node)
      BY <1>1, <2>1, <2>2, IndexedFairActionsRemainEnabledInProduct
    <2> QED BY <2>3 DEF IndexedChainVars
  <1> QED BY <1>1

THEOREM IndexedExactOpenRecordsHistoricalRecoveryTarget ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedHistoricalRecoveryReady(initialContext, node)
      /\ IndexedOpenHistoricalRecoveryStep(initialContext, node)
      => HistoricalRecoveryOpenOutcome(initialContext, node)'
BY Isa DEF IndexedOpenHistoricalRecoveryStep,
           IndexedOpenHistoricalRecovery,
           IndexedHistoricalRecoveryReady,
           IndexedHistoricalRecoveryTargetReady,
           IndexedHistoricalRecoverySourceReady,
           HistoricalRecoveryOpenOutcome,
           IndexedAsync!OpenHistoricalRecovery,
           IndexedAsync!HistoricalRecoveryTarget

THEOREM IndexedChainSpecEventuallyOpensReadyHistoricalRecovery ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedChainSpec
      => (IndexedHistoricalRecoveryReady(initialContext, node)
            ~> HistoricalRecoveryOpenOutcome(initialContext, node))
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              IndexedChainSpec
         PROVE IndexedHistoricalRecoveryReady(initialContext, node)
                 ~> HistoricalRecoveryOpenOutcome(initialContext, node)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. [][IndexedChainNext]_IndexedChainVars
      BY <1>1, PTL DEF IndexedChainSpec
    <2>3. WF_IndexedChainVars(
             IndexedOpenHistoricalRecoveryStep(initialContext, node))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2>4. IndexedCompositionInvariant
             /\ IndexedHistoricalRecoveryReady(initialContext, node)
             /\ [IndexedChainNext]_IndexedChainVars
             => \/ IndexedHistoricalRecoveryReady(
                       initialContext, node)'
                \/ HistoricalRecoveryOpenOutcome(
                     initialContext, node)'
      BY <1>1, IndexedHistoricalRecoveryReadyPersistsOrOpens
    <2>5. IndexedCompositionInvariant
             /\ IndexedHistoricalRecoveryReady(initialContext, node)
             => ENABLED
                  <<IndexedOpenHistoricalRecoveryStep(
                      initialContext, node)>>_(IndexedChainVars)
      BY <1>1, IndexedHistoricalRecoveryReadyEnablesExactOpen
    <2>6. IndexedHistoricalRecoveryReady(initialContext, node)
             /\ IndexedOpenHistoricalRecoveryStep(initialContext, node)
             => HistoricalRecoveryOpenOutcome(initialContext, node)'
      BY <1>1, IndexedExactOpenRecordsHistoricalRecoveryTarget
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM IndexedNodeApplicationEvidenceIsStable ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedAsync(initialContext)!NodeHasApplication(node)
      /\ [IndexedChainNext]_IndexedChainVars
      => (IndexedAsync(initialContext)!NodeHasApplication(node))'
BY Isa DEF IndexedChainNext, IndexedChainVars,
           IndexedProductActionAt, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           NewIndexedApplicationReceipt,
           NoNewIndexedDurableReceipt,
           IndexedApplications, IndexedAsync!NodeHasApplication

THEOREM IndexedApplicationEvidenceCompletesHistoricalRecovery ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedCompositionInvariant
      /\ IndexedAsync(initialContext)!NodeHasApplication(node)
      => HistoricalRecoveryComplete(initialContext, node)
BY Isa DEF IndexedCompositionInvariant,
           IndexedApplicationsRespectNodeHeight,
           HistoricalRecoveryComplete

(***************************************************************************
This is the exact chain-level derivation that was previously missing. It is
conditional on the separately named eligibility leadsto and the two Async
temporal properties above. The eligibility leadsto is chain-composition debt:
current-height consensus must first produce a durable certified source when a
joined node has none. In particular, neither the production successor
refinement invariant nor source fidelity is accepted as temporal recovery
evidence.
***************************************************************************)
THEOREM IndexedExactHistoricalRecoveryFromAsyncTemporalPrerequisites ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalRecoveryTemporalPrerequisites
  => IndexedExactHistoricalRecoveryProgress
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedHistoricalRecoveryTemporalPrerequisites,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive
         PROVE HistoricalRecoveryOutstanding(initialContext, node)
                 ~> HistoricalRecoveryComplete(initialContext, node)
    <2>1. HistoricalRecoveryOutstanding(initialContext, node)
             ~> HistoricalRecoveryProgressEligible(initialContext, node)
      BY <1>1 DEF IndexedHistoricalRecoveryTemporalPrerequisites,
                    IndexedHistoricalRecoveryEligibilityProgress
    <2>2. IndexedHistoricalRecoveryReady(initialContext, node)
             ~> HistoricalRecoveryOpenOutcome(initialContext, node)
      BY <1>1, IndexedChainSpecEventuallyOpensReadyHistoricalRecovery
    <2>3. IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
             ~> IndexedAsync(initialContext)!NodeHasDecision(node)
      BY <1>1 DEF IndexedHistoricalRecoveryTemporalPrerequisites,
                    IndexedHistoricalRecoveryAsyncTemporalPrerequisites,
                    IndexedHistoricalRecoveryTargetDecisionProgress
    <2>4. IndexedAsync(initialContext)!NodeHasDecision(node)
             ~> IndexedAsync(initialContext)!NodeHasApplication(node)
      BY <1>1 DEF IndexedHistoricalRecoveryTemporalPrerequisites,
                    IndexedHistoricalRecoveryAsyncTemporalPrerequisites,
                    IndexedResponsiveDecisionApplicationProgress
    <2>5. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>6. IndexedAsync(initialContext)!NodeHasApplication(node)
             /\ [IndexedChainNext]_IndexedChainVars
             => (IndexedAsync(initialContext)!
                   NodeHasApplication(node))'
      BY <1>1, IndexedNodeApplicationEvidenceIsStable
    <2>7. [](IndexedAsync(initialContext)!NodeHasApplication(node)
               => HistoricalRecoveryComplete(initialContext, node))
      BY <2>5, IndexedApplicationEvidenceCompletesHistoricalRecovery, PTL
    <2>8. HistoricalRecoveryProgressEligible(initialContext, node)
             => \/ IndexedHistoricalRecoveryReady(initialContext, node)
                \/ IndexedAsync(initialContext)!
                     HistoricalRecoveryTarget(node)
                \/ IndexedAsync(initialContext)!NodeHasDecision(node)
      BY DEF HistoricalRecoveryProgressEligible
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>6, <2>7,
                 <2>8, PTL
         DEF HistoricalRecoveryOpenOutcome,
             HistoricalRecoveryProgressEligible,
             HistoricalRecoveryOutstanding
  <1> QED BY <1>1 DEF IndexedExactHistoricalRecoveryProgress

(***************************************************************************
The successor theorem is now usable in the correct module direction.  Its
well-founded starvation result is definitionally the chain progress property
consumed by the finite-height induction.
***************************************************************************)
THEOREM IndexedSuccessorActivationProgressFromStarvationProof ==
  IndexedChainSpec => IndexedSuccessorActivationProgress
PROOF
  <1>1. IndexedChainSpec
           => SuccessorActivationStarvationFreedomProperty
    BY SuccessorActivationStarvationFreedomObligation
  <1>2. SuccessorActivationStarvationFreedomProperty
           <=> IndexedSuccessorActivationProgress
    BY SuccessorActivationStarvationMatchesChainProgress
  <1> QED BY <1>1, <1>2

THEOREM IndexedHeightLivenessFromAsyncHistoricalRecoveryAndSuccessorProofs ==
  /\ IndexedChainSpec
  /\ IndexedHistoricalRecoveryTemporalPrerequisites
  => IndexedHeightLivenessProperty
PROOF
  <1>1. IndexedExactHistoricalRecoveryProgress
    BY IndexedExactHistoricalRecoveryFromAsyncTemporalPrerequisites
  <1>2. IndexedSuccessorActivationProgress
    BY IndexedSuccessorActivationProgressFromStarvationProof
  <1>3. VerificationOneHeightCompletion
    BY VerificationOneHeightCompletionObligation
  <1> QED BY <1>1, <1>2, <1>3,
       HeightLivenessFromOneHeightAndExactRecoveryProgress

(***************************************************************************
Release-facing declaration.

This remains proofless until the chain composition proves eventual recovery
eligibility and the Async application-liveness work proves both
target-to-Decision and responsive Decision-to-application for the exact
indexed product. Keeping the declaration here makes those debts visible
without adding a 55th ledger entry or pretending the production
safety/refinement seam is a temporal theorem.
***************************************************************************)
THEOREM HeightLivenessObligation ==
  IndexedChainSpec => IndexedHeightLivenessProperty


=============================================================================
