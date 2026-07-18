---- MODULE SumeragiV2SuccessorActivationRefinementProofs ----
EXTENDS SumeragiV2ChainEpochRefinement, TLAPS

(***************************************************************************
The finite verification horizon has no successor context.  A terminal
historical application must therefore remain an observer/application receipt:
none of the production-shaped activation actions may create a predecessor
owner, token, marker, prerequisite, or joined successor for that context.
***************************************************************************)
TerminalSuccessorDormancyInvariant ==
  \A terminalContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    terminalContext.height = MaxHeight
      => /\ successorActivationStatus[terminalContext][node] = "Idle"
         /\ successorPredecessorStatusOwnership[terminalContext][node]
              = "Absent"

THEOREM IndexedInitEstablishesTerminalSuccessorDormancy ==
  IndexedChainInit => TerminalSuccessorDormancyInvariant
BY Isa DEF IndexedChainInit, TerminalSuccessorDormancyInvariant

THEOREM IndexedActionPreservesTerminalSuccessorDormancy ==
  TerminalSuccessorDormancyInvariant /\ IndexedChainNext
    => TerminalSuccessorDormancyInvariant'
BY Isa DEF TerminalSuccessorDormancyInvariant,
           IndexedChainNext, IndexedProductActionAt,
           IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           QueueSuccessorActivation,
           IndexedSuccessorActivationProgressStep,
           BeginSuccessorActivation,
           BindAppliedSuccessorActivationToken,
           FailClosedSuccessorStartup,
           AuthenticateRecoveredSuccessorActivation,
           OpenDeferredSuccessorAdapter,
           ConstructSuccessorRuntime,
           StartSuccessorServices,
           ApplySuccessorStartupEffects,
           ArmSuccessorClocks,
           PrepareSuccessorActivationMarker,
           OpenSuccessorIngress,
           ActivateAppliedSuccessorHeight,
           ActivateRecoveredSuccessorHeight,
           SuccessorActivationEnvironmentStutter,
           CanonicalIndexedContext,
           AdmissibleContextRecords, FrozenContextAdmissible,
           ContextRecords, Heights

THEOREM IndexedStepPreservesTerminalSuccessorDormancy ==
  TerminalSuccessorDormancyInvariant
    /\ [IndexedChainNext]_IndexedChainVars
    => TerminalSuccessorDormancyInvariant'
PROOF
  <1>1. CASE IndexedChainNext
    BY <1>1, IndexedActionPreservesTerminalSuccessorDormancy
  <1>2. CASE UNCHANGED IndexedChainVars
    BY <1>2, Isa
       DEF IndexedChainVars, TerminalSuccessorDormancyInvariant
  <1> QED BY <1>1, <1>2

THEOREM IndexedChainSpecEstablishesTerminalSuccessorDormancy ==
  IndexedChainSpec => []TerminalSuccessorDormancyInvariant
PROOF
  <1>1. IndexedChainInit => TerminalSuccessorDormancyInvariant
    BY IndexedInitEstablishesTerminalSuccessorDormancy
  <1>2. TerminalSuccessorDormancyInvariant
           /\ [IndexedChainNext]_IndexedChainVars
           => TerminalSuccessorDormancyInvariant'
    BY IndexedStepPreservesTerminalSuccessorDormancy
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

(***************************************************************************
Strict discharge of the model-internal production-shaped invariant. Exact
historical recovery is already an ordinary per-context Async transition, so
this adapter needs no shadow receipt or stage action. The separate proofless
source-refinement gate must still bind the successor actions and the exact
OpenHistoricalRecovery/decision/body/store/validate/apply corridor to the
executable Rust transitions before the proof ledger may promote the Rust-to-
TLA production refinement obligation.
***************************************************************************)
THEOREM AbstractSuccessorActivationAndExactHistoricalRecoveryInvariant ==
  IndexedChainSpec
    => []SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE
           []SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []TerminalExactHistoricalRecoveryBoundaryInvariant
      BY <1>1, IndexedChainAlwaysPreservesTerminalExactRecoveryBoundary
    <2> QED BY <2>1, <2>2, PTL, Isa
         DEF SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant,
             IndexedCompositionInvariant,
             TerminalExactHistoricalRecoveryBoundaryInvariant,
             SuccessorHeightActivated
  <1> QED BY <1>1

(***************************************************************************
Successor-activation starvation boundary.

Application first queues an exact parent/node owner.  Before the single
permitted fail-closed reset, 9 is added to the local pipeline distance;
therefore
that reset strictly decreases the rank even if it interrupts the last startup
stage.  After the reset, authenticated complete-tip recovery resumes at the
same ordered pipeline without another failure edge.  The local stages are:

  Queued, Running without credential, credential bound, adapter, runtime,
  services, startup effects, clocks, marker prepared, ingress open, outcome.

The final rank zero is publication or legitimate supersession by a later
height. All temporal rank and starvation clauses quantify Responsive rather
than ValidatorIds: an honest validator outside the responsive set may stop
after queueing local work, and the conditional production target must not
manufacture fairness for it. `SuccessorActivationStarvationFreedomObligation`
deductively composes the exact structure, rank-decrease, non-orphaning,
outcome-stability, well-founded rank-progress, and starvation theorems below.
The ledger remains `specified_unproved` until that complete proof body passes
the pinned strict TLAPS runner.
***************************************************************************)
SuccessorActivationRankCarrier == 0..19

SuccessorActivationPipelineDistance(parentContext, node) ==
  LET successorContext ==
        CanonicalIndexedContext(parentContext.height + 1)
      marker ==
        SuccessorActivationMarker(parentContext, node, successorContext)
  IN CASE successorActivationStatus[parentContext][node] = "Queued"
            -> IF SuccessorActivationOwner(parentContext, node)
                    \in successorActivationFailureHistory
               THEN 9 ELSE 10
     [] /\ successorActivationStatus[parentContext][node] = "Running"
        /\ ~SuccessorActivationCredentialReady(
              parentContext, node, successorContext)
            -> 9
     [] /\ SuccessorActivationCredentialReady(
              parentContext, node, successorContext)
        /\ successorActivationPrerequisites[parentContext][node] = {}
            -> 8
     [] /\ SuccessorActivationCredentialReady(
              parentContext, node, successorContext)
        /\ successorActivationPrerequisites[parentContext][node]
             = SuccessorActivationAdapterPrerequisites
            -> 7
     [] /\ SuccessorActivationCredentialReady(
              parentContext, node, successorContext)
        /\ successorActivationPrerequisites[parentContext][node]
             = SuccessorActivationRuntimePrerequisites
            -> 6
     [] /\ SuccessorActivationCredentialReady(
              parentContext, node, successorContext)
        /\ successorActivationPrerequisites[parentContext][node]
             = SuccessorActivationServicePrerequisites
            -> 5
     [] /\ SuccessorActivationCredentialReady(
              parentContext, node, successorContext)
        /\ successorActivationPrerequisites[parentContext][node]
             = SuccessorActivationStartupPrerequisites
            -> 4
     [] /\ SuccessorActivationCredentialReady(
              parentContext, node, successorContext)
        /\ successorActivationPrerequisites[parentContext][node]
             = SuccessorActivationClockPrerequisites
        /\ marker \notin preparedSuccessorActivationMarkers
            -> 3
     [] /\ SuccessorActivationCredentialReady(
              parentContext, node, successorContext)
        /\ successorActivationPrerequisites[parentContext][node]
             = SuccessorActivationClockPrerequisites
        /\ marker \in preparedSuccessorActivationMarkers
            -> 2
     [] /\ SuccessorActivationCredentialReady(
              parentContext, node, successorContext)
        /\ successorActivationPrerequisites[parentContext][node]
             = SuccessorActivationRequiredPrerequisites
            -> 1
     [] OTHER -> 0

SuccessorActivationRank(parentContext, node) ==
  IF SuccessorPublicationOrSuperseded(parentContext, node)
  THEN 0
  ELSE IF SuccessorActivationOwner(parentContext, node)
            \notin successorActivationFailureHistory
       THEN 9 + SuccessorActivationPipelineDistance(parentContext, node)
       ELSE SuccessorActivationPipelineDistance(parentContext, node)

SuccessorActivationPending(parentContext, node) ==
  IndexedSuccessorActivationPending(parentContext, node)

SuccessorActivationHasDurableParentWitness(parentContext, node) ==
  /\ \E application \in Chain!DecisionEvidenceSet:
       ExactDurableParentApplication(parentContext, node, application)

SuccessorActivationAtRank(parentContext, node, rank) ==
  /\ SuccessorActivationPending(parentContext, node)
  /\ SuccessorActivationRank(parentContext, node) = rank

(***************************************************************************
Reachable successor-activation protocol state.

`SuccessorActivationShape` deliberately permits every well-typed combination
of status, ownership, prerequisite, token, and failure-history fields.  That
type shape alone is too weak for the rank proof: in particular, an abstract
Idle owner carrying stale durable failure history could otherwise be queued
as Published and take a non-decreasing Begin transition.  The first clause
below records the actual one-shot lifecycle: once durable failure history is
present, the owner is Queued or Running and its process-visible predecessor
ownership is Absent.  Since no action removes failure history, this also
proves that an Idle owner which can be queued has never crossed the failure
boundary.

The second clause is the progress witness for every reachable responsive
pending activation.  It retains the exact durable parent application, excludes
the CASE fallback at pipeline distance zero, and exposes enabledness of the
same full product action to which `IndexedFairness` attaches weak fairness.
***************************************************************************)
SuccessorActivationProtocolInvariant ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    LET owner == SuccessorActivationOwner(parentContext, node)
    IN /\ (owner \in successorActivationFailureHistory
              => /\ successorActivationStatus[parentContext][node]
                       \in {"Queued", "Running"}
                 /\ successorPredecessorStatusOwnership[
                      parentContext][node] = "Absent")
       /\ (SuccessorActivationPending(parentContext, node)
              => /\ SuccessorActivationHasDurableParentWitness(
                       parentContext, node)
                 /\ SuccessorActivationPipelineDistance(
                       parentContext, node) \in 1..10
                 /\ ENABLED
                      <<IndexedSuccessorActivationProgressStep(
                          parentContext, node)>>_(IndexedChainVars))

THEOREM SuccessorActivationProtocolPendingRankIsInCarrier ==
  SuccessorActivationProtocolInvariant
    => \A parentContext \in AdmissibleContextRecords,
          node \in Responsive:
         SuccessorActivationPending(parentContext, node)
           => SuccessorActivationRank(parentContext, node)
                \in SuccessorActivationRankCarrier
BY Isa DEF SuccessorActivationProtocolInvariant,
           SuccessorActivationRank, SuccessorActivationRankCarrier

THEOREM IndexedInitEstablishesSuccessorActivationProtocolInvariant ==
  IndexedChainInit => SuccessorActivationProtocolInvariant
BY Isa DEF IndexedChainInit, SuccessorActivationProtocolInvariant,
           SuccessorActivationPending,
           IndexedSuccessorActivationPending

THEOREM SuccessorActivationProgressPreservesProtocolInvariant ==
  \A selectedParent \in AdmissibleContextRecords,
     selectedNode \in ValidatorIds:
    SuccessorActivationProtocolInvariant
      /\ IndexedSuccessorActivationProgressStep(
           selectedParent, selectedNode)
      => SuccessorActivationProtocolInvariant'
BY ExpandENABLED, Isa
   DEF SuccessorActivationProtocolInvariant,
       SuccessorActivationPending,
       SuccessorActivationHasDurableParentWitness,
       SuccessorActivationPipelineDistance,
       IndexedSuccessorActivationPending,
       SuccessorPublicationOrSuperseded,
       SuccessorHeightActivated,
       IndexedSuccessorActivationProgressStep,
       BeginSuccessorActivation,
       BindAppliedSuccessorActivationToken,
       FailClosedSuccessorStartup,
       AuthenticateRecoveredSuccessorActivation,
       OpenDeferredSuccessorAdapter,
       ConstructSuccessorRuntime,
       StartSuccessorServices,
       ApplySuccessorStartupEffects,
       ArmSuccessorClocks,
       PrepareSuccessorActivationMarker,
       OpenSuccessorIngress,
       ActivateAppliedSuccessorHeight,
       ActivateRecoveredSuccessorHeight,
       SuccessorActivationCredentialReady,
       ExactSuccessorActivationToken,
       ExactCompleteTipRecoveryAuthority,
       ExactDurableParentApplication,
       SuccessorActivationOwner,
       SuccessorActivationToken,
       CompleteTipRecoveryAuthorityRecord,
       SuccessorActivationMarker,
       SuccessorActivationEnvironmentStutter,
       SuccessorActivationVars, IndexedChainVars,
       Chain!ChainEpochVars

THEOREM IndexedProductActionPreservesSuccessorActivationProtocolInvariant ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCompositionInvariant
      /\ SuccessorActivationProtocolInvariant
      /\ IndexedProductActionAt(initialContext)
      => SuccessorActivationProtocolInvariant'
BY ExpandENABLED, Isa
   DEF SuccessorActivationProtocolInvariant,
       SuccessorActivationPending,
       SuccessorActivationHasDurableParentWitness,
       SuccessorActivationPipelineDistance,
       IndexedSuccessorActivationPending,
       SuccessorPublicationOrSuperseded,
       SuccessorHeightActivated,
       IndexedProductActionAt,
       IndexedReceiptClassification,
       IndexedReceiptFreeChainStutter,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       NewIndexedDecisionReceipt,
       NewIndexedApplicationReceipt,
       NoNewIndexedDurableReceipt,
       QueueSuccessorActivation,
       IndexedSuccessorActivationProgressStep,
       BeginSuccessorActivation,
       BindAppliedSuccessorActivationToken,
       FailClosedSuccessorStartup,
       AuthenticateRecoveredSuccessorActivation,
       OpenDeferredSuccessorAdapter,
       ConstructSuccessorRuntime,
       StartSuccessorServices,
       ApplySuccessorStartupEffects,
       ArmSuccessorClocks,
       PrepareSuccessorActivationMarker,
       OpenSuccessorIngress,
       ActivateAppliedSuccessorHeight,
       ActivateRecoveredSuccessorHeight,
       SuccessorActivationCredentialReady,
       ExactSuccessorActivationToken,
       ExactCompleteTipRecoveryAuthority,
       ExactDurableParentApplication,
       SuccessorActivationOwner,
       SuccessorActivationToken,
       CompleteTipRecoveryAuthorityRecord,
       SuccessorActivationMarker,
       SuccessorActivationEnvironmentStutter,
       ExactNodeLocationAt,
       Chain!ApplicationHasRecordedDecision,
       Chain!RecordCertifiedNext,
       Chain!RecordKnownDecision,
       Chain!RecordAppliedNext,
       Chain!RecordKnownApplication,
       Chain!CanonicalCommitForSlot,
       Chain!ChainEpochVars,
       SuccessorActivationVars, IndexedChainVars,
       IndexedCompositionInvariant,
       Chain!ChainEpochInvariant,
       Chain!ChainEpochTypeInvariant,
       Chain!ModelConfiguration

THEOREM IndexedActionPreservesSuccessorActivationProtocolInvariant ==
  IndexedCompositionInvariant
    /\ SuccessorActivationProtocolInvariant
    /\ IndexedChainNext
    => SuccessorActivationProtocolInvariant'
PROOF
  <1>1. ASSUME IndexedCompositionInvariant,
              SuccessorActivationProtocolInvariant,
              IndexedChainNext
         PROVE SuccessorActivationProtocolInvariant'
    <2>1. CASE \E initialContext \in JoinedContexts:
                  IndexedProductActionAt(initialContext)
      BY <1>1, <2>1,
         IndexedProductActionPreservesSuccessorActivationProtocolInvariant,
         Isa
         DEF IndexedCompositionInvariant, JoinedByContextShape,
             JoinedContexts
    <2>2. CASE \E parentContext \in AdmissibleContextRecords,
                  node \in ValidatorIds:
                  IndexedSuccessorActivationProgressStep(
                    parentContext, node)
      BY <1>1, <2>2,
         SuccessorActivationProgressPreservesProtocolInvariant
    <2> QED BY <1>1, <2>1, <2>2 DEF IndexedChainNext
  <1> QED BY <1>1

THEOREM IndexedStutterPreservesSuccessorActivationProtocolInvariant ==
  SuccessorActivationProtocolInvariant
    /\ UNCHANGED IndexedChainVars
    => SuccessorActivationProtocolInvariant'
BY Isa DEF SuccessorActivationProtocolInvariant,
           SuccessorActivationPending,
           SuccessorActivationHasDurableParentWitness,
           SuccessorActivationPipelineDistance,
           IndexedSuccessorActivationPending,
           SuccessorPublicationOrSuperseded,
           SuccessorHeightActivated,
           IndexedChainVars, SuccessorActivationVars,
           Chain!ChainEpochVars

THEOREM IndexedStepPreservesSuccessorActivationProtocolInvariant ==
  IndexedCompositionInvariant
    /\ SuccessorActivationProtocolInvariant
    /\ [IndexedChainNext]_IndexedChainVars
    => SuccessorActivationProtocolInvariant'
PROOF
  <1>1. ASSUME IndexedCompositionInvariant,
              SuccessorActivationProtocolInvariant,
              [IndexedChainNext]_IndexedChainVars
         PROVE SuccessorActivationProtocolInvariant'
    <2>1. CASE IndexedChainNext
      BY <1>1, <2>1,
         IndexedActionPreservesSuccessorActivationProtocolInvariant
    <2>2. CASE UNCHANGED IndexedChainVars
      BY <1>1, <2>2,
         IndexedStutterPreservesSuccessorActivationProtocolInvariant
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM IndexedChainSpecEstablishesSuccessorActivationProtocolInvariant ==
  IndexedChainSpec => []SuccessorActivationProtocolInvariant
PROOF
  <1>1. IndexedChainInit => SuccessorActivationProtocolInvariant
    BY IndexedInitEstablishesSuccessorActivationProtocolInvariant
  <1>2. IndexedChainSpec => []IndexedCompositionInvariant
    BY IndexedChainSpecEstablishesCompositionInvariant
  <1>3. IndexedCompositionInvariant
           /\ SuccessorActivationProtocolInvariant
           /\ [IndexedChainNext]_IndexedChainVars
           => SuccessorActivationProtocolInvariant'
    BY IndexedStepPreservesSuccessorActivationProtocolInvariant
  <1> QED BY <1>1, <1>2, <1>3, PTL DEF IndexedChainSpec

THEOREM SuccessorActivationPipelineDistanceIsBounded ==
  \A parentContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    SuccessorActivationPipelineDistance(parentContext, node) \in 0..10
BY Isa DEF SuccessorActivationPipelineDistance

THEOREM SuccessorActivationRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    OpToRel(<, Nat), SuccessorActivationRankCarrier)
BY NatLessThanWellFounded, IsWellFoundedOnSubset, Isa
   DEF SuccessorActivationRankCarrier

SuccessorActivationPendingStructureProperty ==
  [](\A parentContext \in AdmissibleContextRecords,
       node \in Responsive:
       SuccessorActivationPending(parentContext, node)
         => /\ SuccessorActivationHasDurableParentWitness(
                  parentContext, node)
            /\ SuccessorActivationPipelineDistance(parentContext, node)
                  \in 1..10
            /\ SuccessorActivationRank(parentContext, node)
                  \in SuccessorActivationRankCarrier
            /\ ENABLED
                 <<IndexedSuccessorActivationProgressStep(
                     parentContext, node)>>_(IndexedChainVars))

SuccessorActivationStepDecreasesRankProperty ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    [][ /\ SuccessorActivationPending(parentContext, node)
        /\ IndexedSuccessorActivationProgressStep(parentContext, node)
        => \/ SuccessorPublicationOrSuperseded(parentContext, node)'
           \/ /\ SuccessorActivationPending(parentContext, node)'
              /\ SuccessorActivationRank(parentContext, node)'
                   < SuccessorActivationRank(parentContext, node)
     ]_IndexedChainVars

SuccessorActivationPendingIsNotOrphanedProperty ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    [][ /\ SuccessorActivationPending(parentContext, node)
        /\ [IndexedChainNext]_IndexedChainVars
        => \/ SuccessorPublicationOrSuperseded(parentContext, node)'
           \/ /\ SuccessorActivationPending(parentContext, node)'
              /\ SuccessorActivationRank(parentContext, node)'
                   <= SuccessorActivationRank(parentContext, node)
     ]_IndexedChainVars

SuccessorActivationOutcomeIsStableProperty ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    [][ /\ SuccessorPublicationOrSuperseded(parentContext, node)
        /\ [IndexedChainNext]_IndexedChainVars
        => SuccessorPublicationOrSuperseded(parentContext, node)'
     ]_IndexedChainVars

SuccessorActivationRankProgressProperty ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in SuccessorActivationRankCarrier:
    SuccessorActivationAtRank(parentContext, node, rank)
      ~> (SuccessorPublicationOrSuperseded(parentContext, node)
           \/ \E lower \in SetLessThan(
                rank, OpToRel(<, Nat), SuccessorActivationRankCarrier):
                SuccessorActivationAtRank(parentContext, node, lower))

SuccessorActivationStarvationFreedomProperty ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    SuccessorActivationPending(parentContext, node)
      ~> SuccessorPublicationOrSuperseded(parentContext, node)

SuccessorActivationRankExit(parentContext, node, rank) ==
  \/ SuccessorPublicationOrSuperseded(parentContext, node)
  \/ \E lower \in SetLessThan(
       rank, OpToRel(<, Nat), SuccessorActivationRankCarrier):
       SuccessorActivationAtRank(parentContext, node, lower)

THEOREM IndexedChainSpecEstablishesSuccessorActivationPendingStructure ==
  IndexedChainSpec => SuccessorActivationPendingStructureProperty
PROOF
  <1>1. IndexedChainSpec => []SuccessorActivationProtocolInvariant
    BY IndexedChainSpecEstablishesSuccessorActivationProtocolInvariant
  <1>2. SuccessorActivationProtocolInvariant
           => \A parentContext \in AdmissibleContextRecords,
                 node \in Responsive:
                SuccessorActivationPending(parentContext, node)
                  => /\ SuccessorActivationHasDurableParentWitness(
                           parentContext, node)
                     /\ SuccessorActivationPipelineDistance(
                           parentContext, node) \in 1..10
                     /\ SuccessorActivationRank(parentContext, node)
                           \in SuccessorActivationRankCarrier
                     /\ ENABLED
                          <<IndexedSuccessorActivationProgressStep(
                              parentContext, node)>>_(IndexedChainVars)
    BY SuccessorActivationProtocolPendingRankIsInCarrier,
       Isa DEF SuccessorActivationProtocolInvariant
  <1> QED BY <1>1, <1>2, PTL
       DEF SuccessorActivationPendingStructureProperty

THEOREM SuccessorActivationProgressStepStrictlyDecreasesRank ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    SuccessorActivationProtocolInvariant
      /\ SuccessorActivationPending(parentContext, node)
      /\ IndexedSuccessorActivationProgressStep(parentContext, node)
      => \/ SuccessorPublicationOrSuperseded(parentContext, node)'
         \/ /\ SuccessorActivationPending(parentContext, node)'
            /\ SuccessorActivationRank(parentContext, node)'
                 < SuccessorActivationRank(parentContext, node)
BY Isa
   DEF SuccessorActivationProtocolInvariant,
       SuccessorActivationPending,
       SuccessorActivationRank,
       SuccessorActivationPipelineDistance,
       IndexedSuccessorActivationPending,
       SuccessorPublicationOrSuperseded,
       SuccessorHeightActivated,
       IndexedSuccessorActivationProgressStep,
       BeginSuccessorActivation,
       BindAppliedSuccessorActivationToken,
       FailClosedSuccessorStartup,
       AuthenticateRecoveredSuccessorActivation,
       OpenDeferredSuccessorAdapter,
       ConstructSuccessorRuntime,
       StartSuccessorServices,
       ApplySuccessorStartupEffects,
       ArmSuccessorClocks,
       PrepareSuccessorActivationMarker,
       OpenSuccessorIngress,
       ActivateAppliedSuccessorHeight,
       ActivateRecoveredSuccessorHeight,
       SuccessorActivationCredentialReady,
       ExactSuccessorActivationToken,
       ExactCompleteTipRecoveryAuthority,
       SuccessorActivationOwner,
       SuccessorActivationToken,
       SuccessorActivationMarker,
       SuccessorActivationEnvironmentStutter,
       IndexedChainVars, SuccessorActivationVars,
       Chain!ChainEpochVars

THEOREM IndexedChainSpecEstablishesSuccessorActivationStepDecrease ==
  IndexedChainSpec => SuccessorActivationStepDecreasesRankProperty
PROOF
  <1>1. IndexedChainSpec => []SuccessorActivationProtocolInvariant
    BY IndexedChainSpecEstablishesSuccessorActivationProtocolInvariant
  <1>2. \A parentContext \in AdmissibleContextRecords,
          node \in Responsive:
         SuccessorActivationProtocolInvariant
           /\ SuccessorActivationPending(parentContext, node)
           /\ IndexedSuccessorActivationProgressStep(parentContext, node)
           => \/ SuccessorPublicationOrSuperseded(parentContext, node)'
              \/ /\ SuccessorActivationPending(parentContext, node)'
                 /\ SuccessorActivationRank(parentContext, node)'
                      < SuccessorActivationRank(parentContext, node)
    BY SuccessorActivationProgressStepStrictlyDecreasesRank
  <1> QED BY <1>1, <1>2, PTL
       DEF SuccessorActivationStepDecreasesRankProperty

THEOREM IndexedStepDoesNotOrphanSuccessorActivation ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedCompositionInvariant
      /\ SuccessorActivationProtocolInvariant
      /\ SuccessorActivationPending(parentContext, node)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ SuccessorPublicationOrSuperseded(parentContext, node)'
         \/ /\ SuccessorActivationPending(parentContext, node)'
            /\ SuccessorActivationRank(parentContext, node)'
                 <= SuccessorActivationRank(parentContext, node)
BY IndexedStepPreservesSuccessorActivationProtocolInvariant,
   SuccessorActivationProgressStepStrictlyDecreasesRank,
   IndexedBracketStepKeepsNodeHeightsMonotone,
   Isa
   DEF SuccessorActivationProtocolInvariant,
       SuccessorActivationPending,
       SuccessorActivationRank,
       SuccessorActivationPipelineDistance,
       IndexedSuccessorActivationPending,
       SuccessorPublicationOrSuperseded,
       SuccessorHeightActivated,
       IndexedChainNext,
       IndexedProductActionAt,
       IndexedReceiptClassification,
       IndexedReceiptFreeChainStutter,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       QueueSuccessorActivation,
       IndexedSuccessorActivationProgressStep,
       BeginSuccessorActivation,
       BindAppliedSuccessorActivationToken,
       FailClosedSuccessorStartup,
       AuthenticateRecoveredSuccessorActivation,
       OpenDeferredSuccessorAdapter,
       ConstructSuccessorRuntime,
       StartSuccessorServices,
       ApplySuccessorStartupEffects,
       ArmSuccessorClocks,
       PrepareSuccessorActivationMarker,
       OpenSuccessorIngress,
       ActivateAppliedSuccessorHeight,
       ActivateRecoveredSuccessorHeight,
       SuccessorActivationCredentialReady,
       ExactSuccessorActivationToken,
       ExactCompleteTipRecoveryAuthority,
       SuccessorActivationOwner,
       SuccessorActivationToken,
       SuccessorActivationMarker,
       SuccessorActivationEnvironmentStutter,
       IndexedChainVars, SuccessorActivationVars,
       Chain!ChainEpochVars,
       Chain!RecordCertifiedNext,
       Chain!RecordKnownDecision,
       Chain!RecordAppliedNext,
       Chain!RecordKnownApplication

THEOREM IndexedChainSpecEstablishesSuccessorActivationNonOrphaning ==
  IndexedChainSpec => SuccessorActivationPendingIsNotOrphanedProperty
PROOF
  <1>1. IndexedChainSpec => []IndexedCompositionInvariant
    BY IndexedChainSpecEstablishesCompositionInvariant
  <1>2. IndexedChainSpec => []SuccessorActivationProtocolInvariant
    BY IndexedChainSpecEstablishesSuccessorActivationProtocolInvariant
  <1>3. [][IndexedChainNext]_IndexedChainVars
    BY PTL DEF IndexedChainSpec
  <1>4. \A parentContext \in AdmissibleContextRecords,
          node \in Responsive:
         IndexedCompositionInvariant
           /\ SuccessorActivationProtocolInvariant
           /\ SuccessorActivationPending(parentContext, node)
           /\ [IndexedChainNext]_IndexedChainVars
           => \/ SuccessorPublicationOrSuperseded(parentContext, node)'
              \/ /\ SuccessorActivationPending(parentContext, node)'
                 /\ SuccessorActivationRank(parentContext, node)'
                      <= SuccessorActivationRank(parentContext, node)
    BY IndexedStepDoesNotOrphanSuccessorActivation
  <1> QED BY <1>1, <1>2, <1>3, <1>4, PTL
       DEF SuccessorActivationPendingIsNotOrphanedProperty

THEOREM IndexedStepPreservesSuccessorActivationOutcome ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedCompositionInvariant
      /\ SuccessorPublicationOrSuperseded(parentContext, node)
      /\ [IndexedChainNext]_IndexedChainVars
      => SuccessorPublicationOrSuperseded(parentContext, node)'
BY JoinedMembershipIsMonotone,
   IndexedBracketStepKeepsNodeHeightsMonotone,
   Isa
   DEF SuccessorPublicationOrSuperseded,
       SuccessorHeightActivated,
       IndexedChainNext,
       IndexedProductActionAt,
       IndexedReceiptClassification,
       IndexedReceiptFreeChainStutter,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       QueueSuccessorActivation,
       IndexedSuccessorActivationProgressStep,
       BeginSuccessorActivation,
       BindAppliedSuccessorActivationToken,
       FailClosedSuccessorStartup,
       AuthenticateRecoveredSuccessorActivation,
       OpenDeferredSuccessorAdapter,
       ConstructSuccessorRuntime,
       StartSuccessorServices,
       ApplySuccessorStartupEffects,
       ArmSuccessorClocks,
       PrepareSuccessorActivationMarker,
       OpenSuccessorIngress,
       ActivateAppliedSuccessorHeight,
       ActivateRecoveredSuccessorHeight,
       SuccessorActivationCredentialReady,
       ExactSuccessorActivationToken,
       ExactCompleteTipRecoveryAuthority,
       SuccessorActivationToken,
       SuccessorActivationMarker,
       SuccessorActivationEnvironmentStutter,
       IndexedChainVars, SuccessorActivationVars,
       Chain!ChainEpochVars,
       Chain!RecordCertifiedNext,
       Chain!RecordKnownDecision,
       Chain!RecordAppliedNext,
       Chain!RecordKnownApplication

THEOREM IndexedChainSpecEstablishesSuccessorActivationOutcomeStability ==
  IndexedChainSpec => SuccessorActivationOutcomeIsStableProperty
PROOF
  <1>1. IndexedChainSpec => []IndexedCompositionInvariant
    BY IndexedChainSpecEstablishesCompositionInvariant
  <1>2. [][IndexedChainNext]_IndexedChainVars
    BY PTL DEF IndexedChainSpec
  <1>3. \A parentContext \in AdmissibleContextRecords,
          node \in Responsive:
         IndexedCompositionInvariant
           /\ SuccessorPublicationOrSuperseded(parentContext, node)
           /\ [IndexedChainNext]_IndexedChainVars
           => SuccessorPublicationOrSuperseded(parentContext, node)'
    BY IndexedStepPreservesSuccessorActivationOutcome
  <1> QED BY <1>1, <1>2, <1>3, PTL
       DEF SuccessorActivationOutcomeIsStableProperty

THEOREM SuccessorActivationAtRankPersistsOrExits ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in SuccessorActivationRankCarrier:
    IndexedCompositionInvariant
      /\ SuccessorActivationProtocolInvariant
      /\ SuccessorActivationAtRank(parentContext, node, rank)
      /\ [IndexedChainNext]_IndexedChainVars
      => \/ SuccessorActivationAtRank(parentContext, node, rank)'
         \/ SuccessorActivationRankExit(parentContext, node, rank)'
BY IndexedStepPreservesSuccessorActivationProtocolInvariant,
   IndexedStepDoesNotOrphanSuccessorActivation,
   Isa
   DEF SuccessorActivationProtocolInvariant,
       SuccessorActivationAtRank,
       SuccessorActivationRankExit,
       SuccessorActivationRank,
       SuccessorActivationRankCarrier,
       SetLessThan, OpToRel

THEOREM SuccessorActivationProgressStepExitsRank ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in SuccessorActivationRankCarrier:
    IndexedCompositionInvariant
      /\ SuccessorActivationProtocolInvariant
      /\ SuccessorActivationAtRank(parentContext, node, rank)
      /\ IndexedSuccessorActivationProgressStep(parentContext, node)
      => SuccessorActivationRankExit(parentContext, node, rank)'
BY SuccessorActivationProgressPreservesProtocolInvariant,
   SuccessorActivationProgressStepStrictlyDecreasesRank,
   Isa
   DEF IndexedCompositionInvariant,
       Chain!ChainEpochInvariant,
       Chain!ChainEpochTypeInvariant,
       Chain!ModelConfiguration,
       SuccessorActivationProtocolInvariant,
       SuccessorActivationAtRank,
       SuccessorActivationRankExit,
       SuccessorActivationRank,
       SuccessorActivationRankCarrier,
       SetLessThan, OpToRel

THEOREM SuccessorActivationAtRankEnablesProgress ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in SuccessorActivationRankCarrier:
    SuccessorActivationProtocolInvariant
      /\ SuccessorActivationAtRank(parentContext, node, rank)
      => ENABLED
           <<IndexedSuccessorActivationProgressStep(
               parentContext, node)>>_(IndexedChainVars)
BY DEF SuccessorActivationProtocolInvariant,
       SuccessorActivationAtRank

THEOREM IndexedChainSpecMakesEverySuccessorActivationRankExit ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in SuccessorActivationRankCarrier:
    IndexedChainSpec
      => (SuccessorActivationAtRank(parentContext, node, rank)
            ~> SuccessorActivationRankExit(parentContext, node, rank))
PROOF
  <1>1. ASSUME NEW parentContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              NEW rank \in SuccessorActivationRankCarrier,
              IndexedChainSpec
         PROVE SuccessorActivationAtRank(parentContext, node, rank)
                 ~> SuccessorActivationRankExit(
                      parentContext, node, rank)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []SuccessorActivationProtocolInvariant
      BY <1>1,
         IndexedChainSpecEstablishesSuccessorActivationProtocolInvariant,
         PTL
    <2>3. [][IndexedChainNext]_IndexedChainVars
      BY <1>1, PTL DEF IndexedChainSpec
    <2>4. WF_IndexedChainVars(
             IndexedSuccessorActivationProgressStep(
               parentContext, node))
      BY <1>1, PTL DEF IndexedChainSpec, IndexedFairness
    <2>5. IndexedCompositionInvariant
             /\ SuccessorActivationProtocolInvariant
             /\ SuccessorActivationAtRank(
                  parentContext, node, rank)
             /\ [IndexedChainNext]_IndexedChainVars
             => \/ SuccessorActivationAtRank(
                       parentContext, node, rank)'
                \/ SuccessorActivationRankExit(
                     parentContext, node, rank)'
      BY <1>1, SuccessorActivationAtRankPersistsOrExits
    <2>6. SuccessorActivationProtocolInvariant
             /\ SuccessorActivationAtRank(parentContext, node, rank)
             => ENABLED
                  <<IndexedSuccessorActivationProgressStep(
                      parentContext, node)>>_(IndexedChainVars)
      BY <1>1, SuccessorActivationAtRankEnablesProgress
    <2>7. IndexedCompositionInvariant
             /\ SuccessorActivationProtocolInvariant
             /\ SuccessorActivationAtRank(
                  parentContext, node, rank)
             /\ IndexedSuccessorActivationProgressStep(
                  parentContext, node)
             => SuccessorActivationRankExit(
                  parentContext, node, rank)'
      BY <1>1, SuccessorActivationProgressStepExitsRank
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7, PTL
  <1> QED BY <1>1

THEOREM IndexedChainSpecEstablishesSuccessorActivationRankProgress ==
  IndexedChainSpec => SuccessorActivationRankProgressProperty
PROOF
  <1>1. ASSUME IndexedChainSpec,
              NEW parentContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              NEW rank \in SuccessorActivationRankCarrier
         PROVE SuccessorActivationAtRank(parentContext, node, rank)
                 ~> (SuccessorPublicationOrSuperseded(
                       parentContext, node)
                      \/ \E lower \in SetLessThan(
                           rank, OpToRel(<, Nat),
                           SuccessorActivationRankCarrier):
                           SuccessorActivationAtRank(
                             parentContext, node, lower))
    <2>1. SuccessorActivationAtRank(parentContext, node, rank)
             ~> SuccessorActivationRankExit(
                  parentContext, node, rank)
      BY <1>1, IndexedChainSpecMakesEverySuccessorActivationRankExit
    <2> QED BY <2>1 DEF SuccessorActivationRankExit
  <1> QED BY <1>1
       DEF SuccessorActivationRankProgressProperty

THEOREM SuccessorActivationRankProgressImpliesStarvationFreedom ==
  /\ SuccessorActivationPendingStructureProperty
  /\ SuccessorActivationRankProgressProperty
  => SuccessorActivationStarvationFreedomProperty
PROOF
  <1>1. ASSUME SuccessorActivationPendingStructureProperty,
              SuccessorActivationRankProgressProperty
         PROVE SuccessorActivationStarvationFreedomProperty
    <2>1. ASSUME NEW parentContext \in AdmissibleContextRecords,
                NEW node \in Responsive
           PROVE SuccessorActivationPending(parentContext, node)
                   ~> SuccessorPublicationOrSuperseded(
                        parentContext, node)
      <3>1. \A rank \in SuccessorActivationRankCarrier:
               SuccessorActivationAtRank(parentContext, node, rank)
                 ~> (SuccessorPublicationOrSuperseded(
                       parentContext, node)
                      \/ \E lower \in SetLessThan(
                           rank, OpToRel(<, Nat),
                           SuccessorActivationRankCarrier):
                           SuccessorActivationAtRank(
                             parentContext, node, lower))
        BY <1>1 DEF SuccessorActivationRankProgressProperty
      <3>2. \A rank \in SuccessorActivationRankCarrier:
               SuccessorActivationAtRank(parentContext, node, rank)
                 ~> SuccessorPublicationOrSuperseded(
                      parentContext, node)
        BY <3>1, SuccessorActivationRankOrderingIsWellFounded,
           WellFoundedLeadsTo
      <3>3. (\E rank \in SuccessorActivationRankCarrier:
                SuccessorActivationAtRank(
                  parentContext, node, rank))
               ~> SuccessorPublicationOrSuperseded(
                    parentContext, node)
        BY <3>2, PTL
      <3>4. [](SuccessorActivationPending(parentContext, node)
                 => SuccessorActivationRank(parentContext, node)
                      \in SuccessorActivationRankCarrier)
        BY <1>1, PTL
           DEF SuccessorActivationPendingStructureProperty
      <3>5. [](SuccessorActivationPending(parentContext, node)
                 => \E rank \in SuccessorActivationRankCarrier:
                      SuccessorActivationAtRank(
                        parentContext, node, rank))
        BY <3>4, PTL DEF SuccessorActivationAtRank
      <3> QED BY <3>3, <3>5, PTL
    <2> QED BY <2>1
         DEF SuccessorActivationStarvationFreedomProperty
  <1> QED BY <1>1

THEOREM IndexedChainSpecEstablishesSuccessorActivationStarvationFreedom ==
  IndexedChainSpec => SuccessorActivationStarvationFreedomProperty
PROOF
  <1>1. IndexedChainSpec => SuccessorActivationPendingStructureProperty
    BY IndexedChainSpecEstablishesSuccessorActivationPendingStructure
  <1>2. IndexedChainSpec => SuccessorActivationRankProgressProperty
    BY IndexedChainSpecEstablishesSuccessorActivationRankProgress
  <1> QED BY <1>1, <1>2,
       SuccessorActivationRankProgressImpliesStarvationFreedom

THEOREM SuccessorActivationStarvationFreedomObligation ==
  IndexedChainSpec
    => /\ SuccessorActivationPendingStructureProperty
       /\ SuccessorActivationStepDecreasesRankProperty
       /\ SuccessorActivationPendingIsNotOrphanedProperty
       /\ SuccessorActivationOutcomeIsStableProperty
       /\ SuccessorActivationRankProgressProperty
       /\ SuccessorActivationStarvationFreedomProperty
PROOF
  <1>1. IndexedChainSpec
           => SuccessorActivationPendingStructureProperty
    BY IndexedChainSpecEstablishesSuccessorActivationPendingStructure
  <1>2. IndexedChainSpec
           => SuccessorActivationStepDecreasesRankProperty
    BY IndexedChainSpecEstablishesSuccessorActivationStepDecrease
  <1>3. IndexedChainSpec
           => SuccessorActivationPendingIsNotOrphanedProperty
    BY IndexedChainSpecEstablishesSuccessorActivationNonOrphaning
  <1>4. IndexedChainSpec
           => SuccessorActivationOutcomeIsStableProperty
    BY IndexedChainSpecEstablishesSuccessorActivationOutcomeStability
  <1>5. IndexedChainSpec
           => SuccessorActivationRankProgressProperty
    BY IndexedChainSpecEstablishesSuccessorActivationRankProgress
  <1>6. IndexedChainSpec
           => SuccessorActivationStarvationFreedomProperty
    BY IndexedChainSpecEstablishesSuccessorActivationStarvationFreedom
  <1> QED BY <1>1, <1>2, <1>3, <1>4, <1>5, <1>6

THEOREM SuccessorActivationStarvationMatchesChainProgress ==
  SuccessorActivationStarvationFreedomProperty
    <=> IndexedSuccessorActivationProgress
BY DEF SuccessorActivationStarvationFreedomProperty,
       SuccessorActivationPending,
       IndexedSuccessorActivationProgress

=============================================================================
