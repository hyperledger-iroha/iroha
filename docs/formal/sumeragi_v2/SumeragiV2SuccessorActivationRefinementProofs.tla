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
deliberately remains proofless: it is the exact deadlock/rank/fairness theorem
which must be checked before queued or running activation may be used as
evidence that a responsive validator has joined the successor context.
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

THEOREM SuccessorActivationStarvationFreedomObligation ==
  IndexedChainSpec
    => /\ SuccessorActivationPendingStructureProperty
       /\ SuccessorActivationStepDecreasesRankProperty
       /\ SuccessorActivationPendingIsNotOrphanedProperty
       /\ SuccessorActivationOutcomeIsStableProperty
       /\ SuccessorActivationRankProgressProperty
       /\ SuccessorActivationStarvationFreedomProperty

THEOREM SuccessorActivationStarvationMatchesChainProgress ==
  SuccessorActivationStarvationFreedomProperty
    <=> IndexedSuccessorActivationProgress
BY DEF SuccessorActivationStarvationFreedomProperty,
       SuccessorActivationPending,
       IndexedSuccessorActivationProgress

=============================================================================
