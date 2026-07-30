---- MODULE SumeragiV2ChainLivenessProofs ----
EXTENDS SumeragiV2HistoricalRecoveryTemporalClosureProofs, TLAPS

(***************************************************************************
Chain-level temporal composition.

This module is deliberately a child of the historical temporal-closure proof
module.  That parent imports the successor-activation proof and exposes the
three proof-bearing historical residual kernels without importing this
chain-level composition back into their dependency cone.  The indexed product
remains defined by SumeragiV2ChainEpochRefinement.

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
Exact Decision routing at the indexed product boundary.

The standalone Async owner invariant classifies a durable Decision as either
ordinary current-voter work or exact historical-target work.  The product
wrapper must retain the stronger routing fact needed by the corresponding
fair runner: an ordinary owner is also joined at, and authoritative for, this
exact context.  A node can leave that location only through the same durable
application handoff which makes the liveness goal true.  This invariant is a
safety fact about Decision creation and successor handoff; it imports no
application, one-height, historical-recovery, or height liveness theorem.
***************************************************************************)

IndexedResponsiveDecisionRoutedOwner(initialContext, node) ==
  \/ /\ node \in IndexedAsync(initialContext)!
                   AsyncCurrentResponsiveVoters
     /\ IndexedNodeCurrentAt(initialContext, node)
  \/ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)

IndexedResponsiveDecisionRoutingInvariant ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedAsync(initialContext)!NodeHasDecision(node)
    /\ ~IndexedAsync(initialContext)!NodeHasApplication(node)
      => IndexedResponsiveDecisionRoutedOwner(initialContext, node)

IndexedHistoricalRecoveryEligibilityEntry(initialContext, node) ==
  \/ IndexedHistoricalRecoveryOpenable(initialContext, node)
  \/ IndexedHistoricalRecoveryTargetOwned(initialContext, node)
  \/ IndexedHistoricalDecisionOwned(initialContext, node)

(***************************************************************************
Exact indexed height boundary.

`IndexedExactHeightEntrySource` distinguishes the genesis seed from every
later context.  Height zero is entered only through `GenesisContext`; a
non-genesis context is entered only from the pending activation owned by its
canonical height-minus-one predecessor.  The target is the actual membership
set of the canonical successor context, not the weaker observation that a
numeric node height became larger.

The finite terminal horizon remains an exact application boundary because no
successor context exists there.  These operators are stronger than the older
`IndexedContextCompleted` projection used by the reusable parent induction.
The lemmas below lift that projection back to exact successor membership before
the release wrapper consumes it.
***************************************************************************)

IndexedExactHeightEntrySource(initialContext, node) ==
  IF initialContext.height = 0
  THEN initialContext = GenesisContext
  ELSE /\ initialContext =
             CanonicalIndexedContext(initialContext.height)
       /\ IndexedSuccessorActivationPending(
            CanonicalIndexedContext(initialContext.height - 1), node)

IndexedExactHeightEntryProgress ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedExactHeightEntrySource(initialContext, node)
      ~> node \in joinedByContext[initialContext]

IndexedExactContextCompleted(initialContext) ==
  IF initialContext.height = MaxHeight
  THEN IndexedAllResponsiveExactApplicationsAt(initialContext)
  ELSE LET nextContext ==
             CanonicalIndexedContext(initialContext.height + 1)
       IN \A node \in Responsive:
            node \in joinedByContext[nextContext]

IndexedExactHeightLivenessProperty ==
  (/\ VerificationContext \in AdmissibleContextRecords
   /\ VerificationContext \in JoinedContexts
   /\ IndexedCore(VerificationContext, 7))
    ~> IndexedExactContextCompleted(VerificationContext)

IndexedHistoricalRecoveryClosureGap ==
  IndexedChainSpec => IndexedHistoricalRecoveryTemporalPrerequisites

THEOREM IndexedChainSpecAlwaysSeedsExactGenesisJoin ==
  IndexedChainSpec
    => []\A node \in Responsive:
          node \in joinedByContext[GenesisContext]
PROOF
  <1>1. IndexedChainInit
           => \A node \in Responsive:
                node \in joinedByContext[GenesisContext]
    BY Isa
       DEF IndexedChainInit, GenesisContext, AdmissibleContextRecords,
           FrozenContextAdmissible, ContextRecords, LineagesAt, Heights,
           ModelConfiguration, ValidatorIds
  <1>2. \A node \in Responsive:
           node \in joinedByContext[GenesisContext]
             /\ [IndexedChainNext]_IndexedChainVars
             => node \in joinedByContext[GenesisContext]'
    BY IndexedNodeJoinIsStable, Isa
       DEF AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, LineagesAt, Heights, GenesisContext,
           ModelConfiguration, ValidatorIds
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

THEOREM IndexedExactHeightEntrySourceEventuallyJoins ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => IndexedExactHeightEntryProgress
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive
         PROVE IndexedExactHeightEntrySource(initialContext, node)
                 ~> node \in joinedByContext[initialContext]
    <2>1. CASE initialContext.height = 0
      <3>1. []\A currentNode \in Responsive:
               currentNode \in joinedByContext[GenesisContext]
        BY <1>1, IndexedChainSpecAlwaysSeedsExactGenesisJoin
      <3> QED BY <2>1, <3>1, PTL
           DEF IndexedExactHeightEntrySource
    <2>2. CASE initialContext.height # 0
      <3>1. IndexedExactHeightEntrySource(initialContext, node)
               => IndexedActivationPendingIntoContext(
                    initialContext, node)
        BY <2>2 DEF IndexedExactHeightEntrySource,
                       IndexedActivationPendingIntoContext
      <3>2. IndexedActivationPendingIntoContext(initialContext, node)
               ~> node \in joinedByContext[initialContext]
        BY <1>1,
           IndexedActivationPendingIntoContextEventuallyJoins
      <3> QED BY <3>1, <3>2, PTL
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1 DEF IndexedExactHeightEntryProgress

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

THEOREM IndexedProductNewDecisionHasExactRoutedOwner ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedProductActionAt(initialContext)
    /\ ~IndexedAsync(initialContext)!NodeHasDecision(node)
    /\ IndexedAsync(initialContext)!NodeHasDecision(node)'
      => IndexedResponsiveDecisionRoutedOwner(initialContext, node)'
BY IndexedHistoricalTransport(initialContext)!
     HistoricalRunNodeWorkCreatesDecisionOnlyForRunner,
   IsaT(7200)
   DEF IndexedResponsiveDecisionRoutedOwner,
       IndexedCompositionInvariant,
       IndexedEveryInstanceAsyncStrongTypeInvariant,
       IndexedProductActionAt, IndexedJoinedAsyncNext,
       IndexedJoinedNonCrashStep,
       IndexedJoinedRunnerStep, IndexedJoinedNonRunnerStep,
       IndexedNodeCurrentAt,
       IndexedAsync!AsyncStrongTypeInvariant,
       IndexedAsync!RunNode,
       IndexedAsync!RunHistoricalRecoveryNode,
       IndexedAsync!RunHistoricalServer,
       IndexedAsync!DirectCommitCertificateDiscoveryStep,
       IndexedAsync!DirectHistoricalCommitCertificateDiscoveryStep,
       IndexedAsync!ServiceIoWorker,
       IndexedAsync!ServiceHistoricalRecoveryIoWorker,
       IndexedAsync!EnqueueIoLocalControl,
       IndexedAsync!EnqueueHistoricalRecoveryIoLocalControl,
       IndexedAsync!ResolveCandidateProducerContinuation,
       IndexedAsync!AsyncNetworkStep,
       IndexedAsync!AsyncFaultStep,
       IndexedAsync!PreGstCrash,
       IndexedAsync!RunNodeWork,
       IndexedAsync!NodeHasDecision,
       IndexedAsync!NodeHasApplication,
       IndexedAsync!HistoricalRecoveryTarget,
       IndexedAsync!AsyncCurrentResponsiveVoters,
       IndexedHistoricalTransport!HistoricalNewDecisionOwnedBy,
       IndexedHistoricalTransport!NodeHasDecision,
       IndexedHistoricalTransport!AsyncCurrentResponsiveVoters,
       IndexedHistoricalTransport!HistoricalRecoveryTarget,
       IndexedDecisions, IndexedApplications,
       IndexedCore, IndexedScheduler, IndexedRecovery

THEOREM IndexedChainInitEstablishesResponsiveDecisionRouting ==
  IndexedChainInit => IndexedResponsiveDecisionRoutingInvariant
BY BootstrapParentContextPrecedes, IsaT(1200)
   DEF IndexedResponsiveDecisionRoutingInvariant,
       IndexedResponsiveDecisionRoutedOwner,
       IndexedChainInit, IndexedNodeCurrentAt,
       IndexedAsync!AsyncInitAt, IndexedAsync!AsyncBaseInitAt,
       IndexedAsync!InitAt,
       IndexedAsync!NodeHasDecision,
       IndexedAsync!NodeHasApplication,
       IndexedAsync!HistoricalRecoveryTarget,
       IndexedAsync!AsyncCurrentResponsiveVoters,
       IndexedAsync!CurrentVoters, IndexedAsync!CurrentEpoch,
       IndexedAsync!BootstrapParentDecision,
       IndexedAsync!BootstrapParentCommitQC,
       IndexedAsync!BootstrapParentContext,
       IndexedAsync!ContextRecord,
       GenesisContext, AdmissibleContextRecords,
       FrozenContextAdmissible, ContextRecords, LineagesAt, Heights,
       ModelConfiguration, ValidatorIds

THEOREM IndexedBracketPreservesResponsiveDecisionRouting ==
  /\ IndexedCompositionInvariant
  /\ IndexedDecisionWitnessSupport
  /\ IndexedResponsiveDecisionRoutingInvariant
  /\ [IndexedChainNext]_IndexedChainVars
    => IndexedResponsiveDecisionRoutingInvariant'
PROOF
  <1>1. ASSUME IndexedCompositionInvariant,
              IndexedDecisionWitnessSupport,
              IndexedResponsiveDecisionRoutingInvariant,
              [IndexedChainNext]_IndexedChainVars,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              IndexedAsync(initialContext)!NodeHasDecision(node)',
              ~IndexedAsync(initialContext)!NodeHasApplication(node)'
         PROVE IndexedResponsiveDecisionRoutedOwner(
                   initialContext, node)'
    <2>1. IndexedCompositionInvariant'
      BY <1>1, IndexedStepPreservesCompositionInvariant
    <2>2. CASE IndexedAsync(initialContext)!NodeHasDecision(node)
      <3>1. ~IndexedAsync(initialContext)!NodeHasApplication(node)
        BY <1>1, <2>2, IndexedNodeApplicationEvidenceIsStable
      <3>2. IndexedResponsiveDecisionRoutedOwner(
               initialContext, node)
        BY <1>1, <2>2, <3>1
           DEF IndexedResponsiveDecisionRoutingInvariant
      <3>3. CASE IndexedAsync(initialContext)!
                    HistoricalRecoveryTarget(node)
        BY <1>1, <2>1, <3>3,
           IndexedBracketStepProjectsEveryHistoricalTransportStep,
           IndexedHistoricalTransport(initialContext)!
             HistoricalDecisionTargetPersistsUntilApplication,
           Isa
           DEF IndexedResponsiveDecisionRoutedOwner,
               IndexedCompositionInvariant,
               IndexedEveryInstanceAsyncStrongTypeInvariant,
               IndexedAsync!AsyncStrongTypeInvariant,
               IndexedAsync!HistoricalRecoveryTarget,
               IndexedAsync!NodeHasApplication,
               IndexedHistoricalTransport!AsyncStrongTypeInvariant,
               IndexedHistoricalTransport!HistoricalRecoveryTarget,
               IndexedHistoricalTransport!NodeHasApplication,
               IndexedCore, IndexedScheduler, IndexedRecovery
      <3>4. CASE /\ node \in IndexedAsync(initialContext)!
                          AsyncCurrentResponsiveVoters
                  /\ IndexedNodeCurrentAt(initialContext, node)
        <4>1. node \in joinedByContext[initialContext]'
          BY <1>1, <3>4, IndexedNodeJoinIsStable
             DEF IndexedNodeCurrentAt
        <4>2. node \in
                 (IndexedAsync(initialContext)!
                    AsyncCurrentResponsiveVoters)'
          BY <1>1, <3>4,
             IndexedHistoricalTransport(initialContext)!
               HistoricalBracketKeepsResponsiveVoterSet,
             IndexedBracketStepProjectsEveryHistoricalTransportStep,
             Isa
             DEF IndexedAsync!AsyncCurrentResponsiveVoters,
                 IndexedHistoricalTransport!AsyncCurrentResponsiveVoters,
                 IndexedCore, IndexedScheduler, IndexedRecovery
        <4>3. IndexedNodeCurrentAt(initialContext, node)'
          BY <1>1, <2>1, <4>1,
             JoinedNonCurrentHasApplicationEvidence, Isa
             DEF IndexedAsync!NodeHasApplication
        <4> QED BY <4>2, <4>3
             DEF IndexedResponsiveDecisionRoutedOwner
      <3> QED BY <3>2, <3>3, <3>4
           DEF IndexedResponsiveDecisionRoutedOwner
    <2>2. CASE ~IndexedAsync(initialContext)!NodeHasDecision(node)
      <3>1. CASE UNCHANGED IndexedChainVars
        BY <1>1, <2>2, <3>1, Isa
           DEF IndexedChainVars, IndexedAsync!NodeHasDecision,
               IndexedDecisions, IndexedCore
      <3>2. CASE IndexedChainNext
        <4>1. CASE IndexedProductActionAt(initialContext)
          BY <1>1, <2>2, <3>2, <4>1,
             IndexedProductNewDecisionHasExactRoutedOwner
        <4>2. CASE ~IndexedProductActionAt(initialContext)
          BY <1>1, <2>2, <3>2, <4>2, Isa
             DEF IndexedChainNext, IndexedProductActionAt,
                 IndexedSuccessorActivationProgressStep,
                 IndexedAsyncStateAt, IndexedAsync!NodeHasDecision,
                 IndexedDecisions, IndexedCore
        <4> QED BY <4>1, <4>2
      <3> QED BY <3>1, <3>2
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1 DEF IndexedResponsiveDecisionRoutingInvariant

THEOREM IndexedChainSpecAlwaysResponsiveDecisionRouting ==
  IndexedChainSpec => []IndexedResponsiveDecisionRoutingInvariant
PROOF
  <1>1. IndexedChainInit => IndexedResponsiveDecisionRoutingInvariant
    BY IndexedChainInitEstablishesResponsiveDecisionRouting
  <1>2. IndexedChainSpec => []IndexedCompositionInvariant
    BY IndexedChainSpecEstablishesCompositionInvariant
  <1>3. IndexedChainSpec => []IndexedDecisionWitnessSupport
    BY IndexedChainSpecAlwaysDecisionWitnessSupport
  <1>4. /\ IndexedCompositionInvariant
         /\ IndexedDecisionWitnessSupport
         /\ IndexedResponsiveDecisionRoutingInvariant
         /\ [IndexedChainNext]_IndexedChainVars
           => IndexedResponsiveDecisionRoutingInvariant'
    BY IndexedBracketPreservesResponsiveDecisionRouting
  <1> QED BY <1>1, <1>2, <1>3, <1>4, PTL DEF IndexedChainSpec

THEOREM IndexedHistoricalRecoveryEligibilityEntryIsEligible ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalRecoveryEligibilityEntry(initialContext, node)
      => HistoricalRecoveryProgressEligible(initialContext, node)
BY IsaT(900)
   DEF IndexedHistoricalRecoveryEligibilityEntry,
       HistoricalRecoveryProgressEligible,
       HistoricalRecoveryOutstanding,
       IndexedHistoricalRecoveryOpenable,
       IndexedHistoricalRecoveryTargetOwned,
       IndexedHistoricalDecisionOwned,
       IndexedHistoricalRecoveryRunnerOwned,
       IndexedHistoricalRecoveryReady,
       IndexedHistoricalRecoveryTargetReady,
       IndexedHistoricalRecoverySourceReady,
       IndexedProjectedNodeHasApplication,
       HistoricalRecoveryNodeHasApplicationProjection,
       IndexedCompositionInvariant,
       IndexedTotalReceiptProjection,
       IndexedApplicationReceiptProjection,
       IndexedApplicationEvidence,
       IndexedCurrentApplications,
       IndexedApplications,
       IndexedAsync!NodeHasDecision,
       IndexedAsync!NodeHasApplication,
       IndexedAsync!HistoricalRecoveryTarget

THEOREM IndexedHistoricalTargetIsExactOwnedRecovery ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedAsync(initialContext)!HistoricalRecoveryTarget(node)
      => IndexedHistoricalRecoveryTargetOwned(initialContext, node)
BY ExactHistoricalRecoveryTargetOwnsCurrentLocation, Isa
   DEF IndexedHistoricalRecoveryTargetOwned,
       HistoricalRecoveryOutstanding,
       IndexedCompositionInvariant,
       IndexedHistoricalRecoveryTargetCoherence,
       IndexedAsync!HistoricalRecoveryTarget

THEOREM IndexedHistoricalCertificateGoalHasDurableDecision ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedHistoricalCertificateGoal(initialContext, node)
      => IndexedAsync(initialContext)!NodeHasDecision(node)
BY IsaT(600)
   DEF IndexedHistoricalCertificateGoal,
       IndexedHistoricalExactApplication,
       IndexedHistoricalDecisionOwned,
       IndexedCompositionInvariant,
       IndexedEveryInstanceStrongInvariant,
       IndexedAsync!StrongInductiveInvariant,
       IndexedAsync!Safety,
       IndexedAsync!AppliedRequiresDecision,
       IndexedAsync!NodeHasDecision,
       IndexedAsync!NodeHasApplication

THEOREM IndexedOutstandingRoutedDecisionIsExactOwnedRecovery ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedResponsiveDecisionRoutingInvariant
    /\ HistoricalRecoveryOutstanding(initialContext, node)
    /\ IndexedAsync(initialContext)!NodeHasDecision(node)
      => IndexedHistoricalDecisionOwned(initialContext, node)
BY Isa
   DEF IndexedResponsiveDecisionRoutingInvariant,
       IndexedResponsiveDecisionRoutedOwner,
       IndexedHistoricalDecisionOwned,
       IndexedHistoricalRecoveryRunnerOwned

THEOREM IndexedRoutedDecisionIsApplicationOrExactOwnedRecovery ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedResponsiveDecisionRoutingInvariant
    /\ IndexedAsync(initialContext)!NodeHasDecision(node)
      => \/ IndexedAsync(initialContext)!NodeHasApplication(node)
         \/ IndexedHistoricalDecisionOwned(initialContext, node)
BY Isa
   DEF IndexedResponsiveDecisionRoutingInvariant,
       IndexedResponsiveDecisionRoutedOwner,
       IndexedHistoricalDecisionOwned,
       IndexedHistoricalRecoveryRunnerOwned,
       HistoricalRecoveryOutstanding,
       IndexedNodeCurrentAt, ExactNodeLocationAt,
       IndexedCompositionInvariant,
       JoinedRoutingInvariant,
       IndexedHistoricalRecoveryTargetCoherence,
       IndexedAsync!HistoricalRecoveryTarget,
       IndexedAsync!NodeHasApplication

THEOREM IndexedOutstandingClassifiesAuthorityResidualOrEligibilityEntry ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ HistoricalRecoveryOutstanding(initialContext, node)
      => \/ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
               initialContext, node)
         \/ IndexedHistoricalRecoveryEligibilityEntry(
              initialContext, node)
BY Isa
   DEF IndexedHistoricalRecoveryAuthorityAcquisitionResidual,
       IndexedHistoricalRecoveryEntryGoal,
       IndexedHistoricalRecoveryEligibilityEntry,
       IndexedHistoricalExactApplication,
       HistoricalRecoveryOutstanding,
       IndexedAsync!NodeHasApplication

THEOREM IndexedHistoricalAuthorityResidualCannotInstallApplication ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedDecisionWitnessSupportAt(initialContext)
    /\ IndexedResponsiveDecisionRoutingInvariant
    /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
         initialContext, node)
    /\ [IndexedChainNext]_IndexedChainVars
      => ~IndexedHistoricalExactApplication(initialContext, node)'
PROOF
  <1>1. ASSUME NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              IndexedCompositionInvariant,
              IndexedDecisionWitnessSupportAt(initialContext),
              IndexedResponsiveDecisionRoutingInvariant,
              IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
                initialContext, node),
              [IndexedChainNext]_IndexedChainVars
         PROVE ~IndexedHistoricalExactApplication(
                    initialContext, node)'
    <2>1. ~IndexedAsync(initialContext)!NodeHasDecision(node)
      BY <1>1, IndexedOutstandingRoutedDecisionIsExactOwnedRecovery
         DEF IndexedHistoricalRecoveryAuthorityAcquisitionResidual,
             IndexedHistoricalRecoveryEntryGoal
    <2>2. IndexedCompositionInvariant'
      BY <1>1, IndexedStepPreservesCompositionInvariant
    <2> QED BY <1>1, <2>1, <2>2, IsaT(1200)
         DEF IndexedHistoricalExactApplication,
             IndexedCompositionInvariant,
             IndexedEveryInstanceStrongInvariant,
             IndexedChainNext, IndexedChainVars,
             IndexedProductActionAt,
             IndexedReceiptClassification,
             IndexedReceiptFreeChainStutter,
             IndexedDecisionReceiptHandoff,
             IndexedApplicationReceiptHandoff,
             NewIndexedDecisionReceipt,
             NewIndexedApplicationReceipt,
             NoNewIndexedDurableReceipt,
             IndexedSuccessorActivationProgressStep,
             IndexedAsync!StrongInductiveInvariant,
             IndexedAsync!Safety,
             IndexedAsync!AppliedRequiresDecision,
             IndexedAsync!NodeHasDecision,
             IndexedAsync!NodeHasApplication,
             IndexedDecisions, IndexedApplications,
             IndexedCore
  <1> QED BY <1>1

THEOREM IndexedHistoricalAuthorityResidualPersistsOrEntersEligibility ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedDecisionWitnessSupportAt(initialContext)
    /\ IndexedResponsiveDecisionRoutingInvariant
    /\ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
         initialContext, node)
    /\ [IndexedChainNext]_IndexedChainVars
      => \/ IndexedHistoricalRecoveryAuthorityAcquisitionResidual(
               initialContext, node)'
         \/ IndexedHistoricalRecoveryEligibilityEntry(
              initialContext, node)'
BY IndexedHistoricalAuthorityResidualPersistsOrEnters,
   IndexedHistoricalAuthorityResidualCannotInstallApplication, Isa
   DEF IndexedHistoricalRecoveryEntryGoal,
       IndexedHistoricalRecoveryEligibilityEntry

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

(***************************************************************************
Lift numeric completion back to the exact indexed successor.

`IndexedJoinedThroughLocalHeight` says that a node numerically beyond a
nonterminal context either already belongs to the canonical next instance or
is at that exact height with a pending activation.  The latter pending owner
is necessarily rooted at the canonical height-minus-one context.  Successor
starvation freedom therefore joins that exact next instance.  A finite prefix
induction performs the pointwise-to-all-Responsive temporal lift.
***************************************************************************)

THEOREM IndexedCompletedNonterminalClassifiesExactSuccessorEntry ==
  \A initialContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ IndexedJoinedThroughLocalHeight
    /\ IndexedTargetJoined(initialContext)
    /\ initialContext.height < MaxHeight
    /\ IndexedContextCompleted(initialContext)
    => /\ CanonicalIndexedContext(initialContext.height + 1)
              \in AdmissibleContextRecords
       /\ \/ node \in joinedByContext[
                      CanonicalIndexedContext(initialContext.height + 1)]
          \/ IndexedExactHeightEntrySource(
               CanonicalIndexedContext(initialContext.height + 1), node)
BY Isa
   DEF IndexedCompositionInvariant, JoinedContextCertificationInvariant,
       IndexedJoinedThroughLocalHeight, IndexedTargetJoined, JoinedContexts,
       IndexedContextCompleted, IndexedExactHeightEntrySource,
       IndexedSuccessorActivationPending,
       SuccessorPublicationOrSuperseded, SuccessorHeightActivated,
       CanonicalIndexedContext, AdmissibleContextRecords,
       FrozenContextAdmissible, ContextRecords, LineagesAt, Heights,
       ModelConfiguration, ValidatorIds

THEOREM IndexedCompletedNonterminalEventuallyJoinsExactSuccessorNode ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A initialContext \in AdmissibleContextRecords,
       node \in Responsive:
       (/\ IndexedTargetJoined(initialContext)
        /\ initialContext.height < MaxHeight
        /\ IndexedContextCompleted(initialContext))
         ~> node \in joinedByContext[
              CanonicalIndexedContext(initialContext.height + 1)]
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW initialContext \in AdmissibleContextRecords,
              NEW node \in Responsive
         PROVE (/\ IndexedTargetJoined(initialContext)
                /\ initialContext.height < MaxHeight
                /\ IndexedContextCompleted(initialContext))
                 ~> node \in joinedByContext[
                      CanonicalIndexedContext(initialContext.height + 1)]
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []IndexedJoinedThroughLocalHeight
      BY <1>1, IndexedChainSpecJoinsEveryNodeThroughLocalHeight, PTL
    <2>3. IndexedExactHeightEntryProgress
      BY <1>1, IndexedExactHeightEntrySourceEventuallyJoins
    <2>4. (/\ IndexedCompositionInvariant
            /\ IndexedJoinedThroughLocalHeight
            /\ IndexedTargetJoined(initialContext)
            /\ initialContext.height < MaxHeight
            /\ IndexedContextCompleted(initialContext))
             => /\ CanonicalIndexedContext(initialContext.height + 1)
                       \in AdmissibleContextRecords
                /\ \/ node \in joinedByContext[
                               CanonicalIndexedContext(
                                 initialContext.height + 1)]
                   \/ IndexedExactHeightEntrySource(
                        CanonicalIndexedContext(
                          initialContext.height + 1), node)
      BY <1>1, IndexedCompletedNonterminalClassifiesExactSuccessorEntry
    <2>5. (/\ IndexedCompositionInvariant
            /\ IndexedJoinedThroughLocalHeight
            /\ IndexedTargetJoined(initialContext)
            /\ initialContext.height < MaxHeight
            /\ IndexedContextCompleted(initialContext)
            /\ CanonicalIndexedContext(initialContext.height + 1)
                 \in AdmissibleContextRecords
            /\ IndexedExactHeightEntrySource(
                 CanonicalIndexedContext(
                   initialContext.height + 1), node))
             ~> node \in joinedByContext[
                  CanonicalIndexedContext(initialContext.height + 1)]
      BY <1>1, <2>3, <2>4, PTL
         DEF IndexedExactHeightEntryProgress
    <2>6. /\ IndexedCompositionInvariant
           /\ IndexedJoinedThroughLocalHeight
           /\ IndexedTargetJoined(initialContext)
           /\ initialContext.height < MaxHeight
           /\ IndexedContextCompleted(initialContext)
           /\ CanonicalIndexedContext(initialContext.height + 1)
                \in AdmissibleContextRecords
           /\ node \in joinedByContext[
                CanonicalIndexedContext(initialContext.height + 1)]
           /\ [IndexedChainNext]_IndexedChainVars
             => node \in joinedByContext[
                  CanonicalIndexedContext(initialContext.height + 1)]'
      BY <1>1, <2>4, IndexedNodeJoinIsStable, Isa
         DEF IndexedCompositionInvariant,
             Chain!ChainEpochInvariant,
             Chain!ChainEpochTypeInvariant,
             Chain!ContextsMatchLocalHistories,
             IndexedJoinedThroughLocalHeight,
             IndexedTargetJoined, JoinedContexts,
             IndexedContextCompleted, CanonicalIndexedContext,
             AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, LineagesAt, Heights,
             ModelConfiguration, ValidatorIds
    <2> QED BY <2>1, <2>2, <2>4, <2>5, <2>6, PTL
  <1> QED BY <1>1

IndexedExactSuccessorJoinPrefixAt(initialContext, limit) ==
  \A node \in Responsive \cap (0..limit):
    node \in joinedByContext[
      CanonicalIndexedContext(initialContext.height + 1)]

THEOREM IndexedExactSuccessorJoinPrefixIsStable ==
  \A initialContext \in AdmissibleContextRecords,
     limit \in Nat:
    /\ initialContext.height < MaxHeight
    /\ IndexedExactSuccessorJoinPrefixAt(initialContext, limit)
    /\ [IndexedChainNext]_IndexedChainVars
    => IndexedExactSuccessorJoinPrefixAt(initialContext, limit)'
BY Isa, IndexedNodeJoinIsStable
   DEF IndexedExactSuccessorJoinPrefixAt,
       CanonicalIndexedContext, AdmissibleContextRecords,
       FrozenContextAdmissible, ContextRecords, LineagesAt, Heights,
       ModelConfiguration, ValidatorIds

THEOREM IndexedCompletedNonterminalEventuallyJoinsExactSuccessorPrefix ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A initialContext \in AdmissibleContextRecords:
       \A limit \in Nat:
         (/\ IndexedTargetJoined(initialContext)
          /\ initialContext.height < MaxHeight
          /\ IndexedContextCompleted(initialContext))
           ~> IndexedExactSuccessorJoinPrefixAt(initialContext, limit)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW initialContext \in AdmissibleContextRecords
         PROVE \A limit \in Nat:
                 (/\ IndexedTargetJoined(initialContext)
                  /\ initialContext.height < MaxHeight
                  /\ IndexedContextCompleted(initialContext))
                   ~> IndexedExactSuccessorJoinPrefixAt(
                        initialContext, limit)
    <2> DEFINE Antecedent ==
           /\ IndexedTargetJoined(initialContext)
           /\ initialContext.height < MaxHeight
           /\ IndexedContextCompleted(initialContext)
    <2> DEFINE P(limit) ==
           Antecedent
             ~> IndexedExactSuccessorJoinPrefixAt(initialContext, limit)
    <2>1. P(0)
      <3>1. CASE 0 \in Responsive
        BY <1>1, <3>1,
           IndexedCompletedNonterminalEventuallyJoinsExactSuccessorNode,
           PTL DEF P, Antecedent,
                   IndexedExactSuccessorJoinPrefixAt
      <3>2. CASE 0 \notin Responsive
        BY <3>2, PTL DEF P, Antecedent,
                       IndexedExactSuccessorJoinPrefixAt
      <3> QED BY <3>1, <3>2
    <2>2. ASSUME NEW limit \in Nat, P(limit)
           PROVE P(limit + 1)
      <3>1. CASE limit + 1 \in Responsive
        <4>1. Antecedent
                 ~> limit + 1 \in joinedByContext[
                      CanonicalIndexedContext(initialContext.height + 1)]
          BY <1>1, <3>1,
             IndexedCompletedNonterminalEventuallyJoinsExactSuccessorNode
             DEF Antecedent
        <4>2. /\ initialContext.height < MaxHeight
               /\ IndexedExactSuccessorJoinPrefixAt(
                    initialContext, limit)
               /\ [IndexedChainNext]_IndexedChainVars
              => IndexedExactSuccessorJoinPrefixAt(
                   initialContext, limit)'
          BY <1>1, <2>2, IndexedExactSuccessorJoinPrefixIsStable
        <4>3. limit + 1 \in joinedByContext[
                 CanonicalIndexedContext(initialContext.height + 1)]
                 /\ [IndexedChainNext]_IndexedChainVars
                 => limit + 1 \in joinedByContext[
                      CanonicalIndexedContext(initialContext.height + 1)]'
          BY <1>1, IndexedNodeJoinIsStable, Isa
             DEF AdmissibleContextRecords, FrozenContextAdmissible,
                 ContextRecords, LineagesAt, Heights,
                 ModelConfiguration, ValidatorIds
        <4>4. IndexedExactSuccessorJoinPrefixAt(
                 initialContext, limit + 1)
                 <=> /\ IndexedExactSuccessorJoinPrefixAt(
                           initialContext, limit)
                     /\ limit + 1 \in joinedByContext[
                           CanonicalIndexedContext(
                             initialContext.height + 1)]
          BY <2>2, <3>1, Isa
             DEF IndexedExactSuccessorJoinPrefixAt
        <4> QED BY <2>2, <4>1, <4>2, <4>3, <4>4, PTL
             DEF P, Antecedent
      <3>2. CASE limit + 1 \notin Responsive
        <4>1. IndexedExactSuccessorJoinPrefixAt(
                 initialContext, limit)
                 => IndexedExactSuccessorJoinPrefixAt(
                      initialContext, limit + 1)
          BY <2>2, <3>2, Isa
             DEF IndexedExactSuccessorJoinPrefixAt
        <4> QED BY <2>2, <4>1, PTL DEF P
      <3> QED BY <3>1, <3>2
    <2>3. \A limit \in Nat: P(limit)
      BY <2>1, <2>2, NatInduction
    <2> QED BY <2>3 DEF P
  <1> QED BY <1>1

THEOREM IndexedCompletedNonterminalEventuallyJoinsExactSuccessor ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A initialContext \in AdmissibleContextRecords:
       (/\ IndexedTargetJoined(initialContext)
        /\ initialContext.height < MaxHeight
        /\ IndexedContextCompleted(initialContext))
         ~> \A node \in Responsive:
              node \in joinedByContext[
                CanonicalIndexedContext(initialContext.height + 1)]
BY IndexedCompletedNonterminalEventuallyJoinsExactSuccessorPrefix, SMT
   DEF IndexedExactSuccessorJoinPrefixAt,
       ModelConfiguration, ValidatorIds

THEOREM IndexedProjectedCompletionReachesExactCompletion ==
  /\ IndexedChainSpec
  /\ IndexedSuccessorActivationProgress
  => \A initialContext \in AdmissibleContextRecords:
       (IndexedTargetJoined(initialContext)
         /\ IndexedContextCompleted(initialContext))
         ~> IndexedExactContextCompleted(initialContext)
PROOF
  <1>1. ASSUME IndexedChainSpec,
              IndexedSuccessorActivationProgress,
              NEW initialContext \in AdmissibleContextRecords
         PROVE (IndexedTargetJoined(initialContext)
                  /\ IndexedContextCompleted(initialContext))
                 ~> IndexedExactContextCompleted(initialContext)
    <2>1. CASE initialContext.height = MaxHeight
      BY <2>1, PTL
         DEF IndexedContextCompleted, IndexedExactContextCompleted
    <2>2. CASE initialContext.height # MaxHeight
      <3>1. initialContext.height < MaxHeight
        BY <1>1, <2>2, Isa
           DEF AdmissibleContextRecords, FrozenContextAdmissible,
               ContextRecords, Heights
      <3>2. (/\ IndexedTargetJoined(initialContext)
              /\ initialContext.height < MaxHeight
              /\ IndexedContextCompleted(initialContext))
               ~> \A node \in Responsive:
                    node \in joinedByContext[
                      CanonicalIndexedContext(initialContext.height + 1)]
        BY <1>1,
           IndexedCompletedNonterminalEventuallyJoinsExactSuccessor
      <3> QED BY <2>2, <3>1, <3>2, PTL
           DEF IndexedExactContextCompleted
    <2> QED BY <2>1, <2>2
  <1> QED BY <1>1

THEOREM IndexedExactContextCompletionImpliesProjectedCompletion ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCompositionInvariant
      /\ IndexedExactContextCompleted(initialContext)
      => IndexedContextCompleted(initialContext)
BY Isa
   DEF IndexedCompositionInvariant, JoinedRoutingInvariant,
       IndexedExactContextCompleted, IndexedContextCompleted,
       IndexedAllResponsiveExactApplicationsAt,
       IndexedNodeCurrentAt, CanonicalIndexedContext,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!ContextsMatchLocalHistories,
       AdmissibleContextRecords, FrozenContextAdmissible,
       ContextRecords, Heights, ModelConfiguration, ValidatorIds

THEOREM IndexedExactHeightLivenessFromOneHeightAndExactRecoveryProgress ==
  /\ IndexedLiveChainSpec
  /\ IndexedGstEventuallyCondition
  /\ IndexedExactHistoricalRecoveryProgress
  /\ IndexedSuccessorActivationProgress
  /\ VerificationOneHeightCompletion
  => IndexedExactHeightLivenessProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedGstEventuallyCondition,
              IndexedExactHistoricalRecoveryProgress,
              IndexedSuccessorActivationProgress,
              VerificationOneHeightCompletion
         PROVE IndexedExactHeightLivenessProperty
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. IndexedHeightLivenessProperty
      BY <1>1, HeightLivenessFromOneHeightAndExactRecoveryProgress
    <2>2. CASE VerificationContext \in AdmissibleContextRecords
      <3>1. (IndexedTargetJoined(VerificationContext)
               /\ IndexedContextCompleted(VerificationContext))
               ~> IndexedExactContextCompleted(VerificationContext)
        BY <1>1, <2>0, <2>2,
           IndexedProjectedCompletionReachesExactCompletion
      <3>2. IndexedTargetJoined(VerificationContext)
               /\ [IndexedChainNext]_IndexedChainVars
               => IndexedTargetJoined(VerificationContext)'
        BY <2>2, IndexedTargetJoinedIsStable
      <3> QED BY <1>1, <2>1, <2>2, <3>1, <3>2, PTL
           DEF IndexedHeightLivenessProperty,
               IndexedExactHeightLivenessProperty,
               IndexedTargetJoined
    <2>3. CASE VerificationContext \notin AdmissibleContextRecords
      BY <2>3 DEF IndexedExactHeightLivenessProperty
    <2> QED BY <2>2, <2>3
  <1> QED BY <1>1

THEOREM IndexedExactHeightLivenessFromAsyncHistoricalRecoveryAndSuccessorProofs ==
  /\ IndexedLiveChainSpec
  /\ IndexedGstEventuallyCondition
  /\ IndexedHistoricalRecoveryTemporalPrerequisites
  => IndexedExactHeightLivenessProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedGstEventuallyCondition,
              IndexedHistoricalRecoveryTemporalPrerequisites
         PROVE IndexedExactHeightLivenessProperty
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. IndexedExactHistoricalRecoveryProgress
      BY <1>1, <2>0,
         IndexedExactHistoricalRecoveryFromAsyncTemporalPrerequisites
    <2>2. IndexedSuccessorActivationProgress
      BY <2>0, IndexedSuccessorActivationProgressFromStarvationProof
    <2>3. VerificationOneHeightCompletion
      BY VerificationOneHeightCompletionObligation
    <2> QED BY <1>1, <2>1, <2>2, <2>3,
         IndexedExactHeightLivenessFromOneHeightAndExactRecoveryProgress
  <1> QED BY <1>1

THEOREM IndexedHeightLivenessFromAsyncHistoricalRecoveryAndSuccessorProofs ==
  /\ IndexedLiveChainSpec
  /\ IndexedGstEventuallyCondition
  /\ IndexedHistoricalRecoveryTemporalPrerequisites
  => IndexedHeightLivenessProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedGstEventuallyCondition,
              IndexedHistoricalRecoveryTemporalPrerequisites
         PROVE IndexedHeightLivenessProperty
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. IndexedExactHistoricalRecoveryProgress
      BY <1>1, <2>0,
         IndexedExactHistoricalRecoveryFromAsyncTemporalPrerequisites
    <2>2. IndexedSuccessorActivationProgress
      BY <2>0, IndexedSuccessorActivationProgressFromStarvationProof
    <2>3. VerificationOneHeightCompletion
      BY VerificationOneHeightCompletionObligation
    <2>4. IndexedExactHeightLivenessProperty
      BY <1>1, <2>1, <2>2, <2>3,
         IndexedExactHeightLivenessFromOneHeightAndExactRecoveryProgress
    <2>5. []IndexedCompositionInvariant
      BY <2>0, IndexedChainSpecEstablishesCompositionInvariant
    <2>6. VerificationContext \in AdmissibleContextRecords
             => (IndexedExactContextCompleted(VerificationContext)
                  => IndexedContextCompleted(VerificationContext))
      BY <2>5, IndexedExactContextCompletionImpliesProjectedCompletion, PTL
    <2> QED BY <1>1, <2>4, <2>6, PTL
         DEF IndexedExactHeightLivenessProperty,
             IndexedHeightLivenessProperty
  <1> QED BY <1>1

THEOREM IndexedHeightLivenessFromHistoricalReleaseResidualsAndSuccessorProofs ==
  /\ IndexedLiveChainSpec
  /\ IndexedGstEventuallyCondition
  /\ IndexedLocalAdequateLeaderDecisionConvergenceProperty
    => IndexedHeightLivenessProperty
PROOF
  <1>1. ASSUME IndexedLiveChainSpec,
              IndexedGstEventuallyCondition,
              IndexedLocalAdequateLeaderDecisionConvergenceProperty
         PROVE IndexedHeightLivenessProperty
    <2>0. IndexedChainSpec
      BY <1>1, IndexedLiveChainSpecProjectsIndexedChainSpec
    <2>1. IndexedExactHistoricalRecoveryProgress
      BY <1>1, IndexedHistoricalReleaseResidualsDischargeExactProgress
    <2>2. IndexedSuccessorActivationProgress
      BY <2>0, IndexedSuccessorActivationProgressFromStarvationProof
    <2>3. VerificationOneHeightCompletion
      BY VerificationOneHeightCompletionObligation
    <2>4. IndexedExactHeightLivenessProperty
      BY <1>1, <2>1, <2>2, <2>3,
         IndexedExactHeightLivenessFromOneHeightAndExactRecoveryProgress
    <2>5. []IndexedCompositionInvariant
      BY <2>0, IndexedChainSpecEstablishesCompositionInvariant
    <2>6. VerificationContext \in AdmissibleContextRecords
             => (IndexedExactContextCompleted(VerificationContext)
                  => IndexedContextCompleted(VerificationContext))
      BY <2>5, IndexedExactContextCompletionImpliesProjectedCompletion, PTL
    <2> QED BY <1>1, <2>4, <2>6, PTL
         DEF IndexedExactHeightLivenessProperty,
             IndexedHeightLivenessProperty
  <1> QED BY <1>1

THEOREM IndexedHeightLivenessFromFixedDeadlineDisseminationAndExposureProofs ==
  /\ IndexedLiveChainSpec
  /\ IndexedGstEventuallyCondition
  /\ IndexedLocalAdequateLeaderFixedDeadlineAndResponsiveDisseminationProperty
  /\ IndexedLocalAdequateLeaderFreshSelfCorridorExposureProperty
    => IndexedHeightLivenessProperty
BY IndexedAdequateLeaderFixedDeadlineDisseminationAndExposureSupplyLocalConvergence,
   IndexedHeightLivenessFromHistoricalReleaseResidualsAndSuccessorProofs

(***************************************************************************
Release-facing declaration.

This remains proofless until the indexed product supplies the fresh-self
quantitative provider bundle, the previously closed starvation dependency
needed by qualitative DirectTimeout dissemination, and target-local
fresh-self corridor exposure.  The theorem above routes the resulting local
Decision convergence through the three reviewed historical residuals,
successor starvation freedom, and the separately closed rotating-leader/
application one-height result.  The live chain premise is exactly the indexed
product and the explicit environmental `IndexedGstEventuallyCondition`.
Bounded TLC generation exhaustion is diagnostic only; the live model uses
`Nat`, and production physical nonexhaustion follows from strict same-view
Prepare-rank ascent.  Keeping the declaration here makes the remaining
provider debts visible without adding a synthetic ledger entry or pretending
the production safety/refinement seam is a temporal theorem.
***************************************************************************)
THEOREM HeightLivenessObligation ==
  /\ IndexedLiveChainSpec
  /\ IndexedGstEventuallyCondition
  => IndexedHeightLivenessProperty


=============================================================================
