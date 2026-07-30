---- MODULE SumeragiV2SuccessorActivationRefinementProofs ----
EXTENDS SumeragiV2ChainEpochRefinement, TLAPS

(***************************************************************************
The finite verification horizon has no successor context. An application at
that boundary projects to stuttering of the bounded successor state: none of
the activation actions may create a predecessor owner, token, marker,
prerequisite, or joined successor for that context. This is not a production
terminal-height claim; production continues beyond the arbitrary `MaxHeight`.
***************************************************************************)
FiniteHorizonSuccessorDormancyInvariant ==
  \A terminalContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    terminalContext.height = MaxHeight
      => /\ successorActivationStatus[terminalContext][node] = "Idle"
         /\ successorPredecessorStatusOwnership[terminalContext][node]
              = "Absent"

THEOREM IndexedInitEstablishesTerminalSuccessorDormancy ==
  IndexedChainInit => FiniteHorizonSuccessorDormancyInvariant
BY Isa DEF IndexedChainInit, FiniteHorizonSuccessorDormancyInvariant

THEOREM IndexedActionPreservesTerminalSuccessorDormancy ==
  FiniteHorizonSuccessorDormancyInvariant /\ IndexedChainNext
    => FiniteHorizonSuccessorDormancyInvariant'
BY Isa DEF FiniteHorizonSuccessorDormancyInvariant,
           IndexedChainNext, IndexedProductActionAt,
           IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           QueueSuccessorActivation,
           IndexedSuccessorActivationProgressStep,
           BeginSuccessorActivation,
           BindAppliedSuccessorActivationToken,
           LatchAppliedSuccessorStartupFailure,
           LatchRecoveredSuccessorStartupFailure,
           RehydrateCleanCompleteTipSuccessorStartup,
           RehydrateFailedSuccessorStartup,
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
  FiniteHorizonSuccessorDormancyInvariant
    /\ [IndexedChainNext]_IndexedChainVars
    => FiniteHorizonSuccessorDormancyInvariant'
PROOF
  <1>1. CASE IndexedChainNext
    BY <1>1, IndexedActionPreservesTerminalSuccessorDormancy
  <1>2. CASE UNCHANGED IndexedChainVars
    BY <1>2, Isa
       DEF IndexedChainVars, FiniteHorizonSuccessorDormancyInvariant
  <1> QED BY <1>1, <1>2

THEOREM IndexedChainSpecEstablishesTerminalSuccessorDormancy ==
  IndexedChainSpec => []FiniteHorizonSuccessorDormancyInvariant
PROOF
  <1>1. IndexedChainInit => FiniteHorizonSuccessorDormancyInvariant
    BY IndexedInitEstablishesTerminalSuccessorDormancy
  <1>2. FiniteHorizonSuccessorDormancyInvariant
           /\ [IndexedChainNext]_IndexedChainVars
           => FiniteHorizonSuccessorDormancyInvariant'
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
    <2> QED BY <2>1, PTL, Isa
         DEF SuccessorActivationAndExactHistoricalRecoveryProductionRefinementInvariant,
             IndexedCompositionInvariant,
             SuccessorHeightActivated
  <1> QED BY <1>1

(***************************************************************************
Successor-activation starvation boundary.

Application first queues an exact parent/node owner. The rank is the finite
lexicographic product of predecessor lifecycle ownership and the ordered
startup pipeline:

  Published predecessor tier, recovered/absent predecessor tier;
  Queued, Running without credential, credential bound, adapter, runtime,
  services, startup effects, clocks, marker prepared, ingress open, outcome.

Failure may reset this rank arbitrarily often: Applied failure preserves
Running until restart, and a Recovered attempt may fail again. Therefore no
failure-history bit is used as a one-shot ranking counter. The chain spec
instead states the reviewed runtime premise that every responsive owner has an
eventual failure-free suffix. In that suffix every fair progress transition
decreases rank: in particular, a clean complete-tip restart descends from the
Published tier into a fresh absent-owner attempt. Failed resets require a
currently latched failure and therefore precede the suffix. The final rank
zero is publication or legitimate supersession by a later height. All
temporal rank and starvation clauses quantify Responsive rather than
ValidatorIds. The proof is retained as explicit proof debt until its strict
TLAPS run succeeds; source checks alone do not promote the ledger.
***************************************************************************)
SuccessorActivationRankCarrier == 0..21

SuccessorActivationPipelineDistance(parentContext, node) ==
  LET successorContext ==
        CanonicalIndexedContext(parentContext.height + 1)
      marker ==
        SuccessorActivationMarker(parentContext, node, successorContext)
  IN CASE successorActivationStatus[parentContext][node] = "Queued" -> 10
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
  ELSE IF successorPredecessorStatusOwnership[parentContext][node]
            = "Published"
       THEN 11 + SuccessorActivationPipelineDistance(parentContext, node)
       ELSE SuccessorActivationPipelineDistance(parentContext, node)

SuccessorActivationPending(parentContext, node) ==
  IndexedSuccessorActivationPending(parentContext, node)

SuccessorActivationHasDurableParentWitness(parentContext, node) ==
  /\ \E application \in Chain!DecisionEvidenceSet:
       ExactDurableParentApplication(parentContext, node, application)

SuccessorActivationAtRank(parentContext, node, rank) ==
  /\ SuccessorActivationPending(parentContext, node)
  /\ SuccessorActivationRank(parentContext, node) = rank

SuccessorActivationFailureAbsent(parentContext, node) ==
  SuccessorActivationOwner(parentContext, node)
    \notin successorActivationFailures

(***************************************************************************
Reachable successor-activation protocol state.

`SuccessorActivationShape` deliberately permits every well-typed combination
of status, ownership, prerequisite, token, and failure-history fields. The
first clause records the split lifecycle: a currently latched failure retains
the visible Running status until an explicit restart action, regardless of
whether predecessor ownership is Published (Applied) or Absent (Recovered).
Durable failure history is diagnostic and imposes no one-shot restriction.

The second clause is the progress witness for every reachable responsive
pending activation.  It retains the exact durable parent application, excludes
the CASE fallback at pipeline distance zero, and exposes enabledness of the
same full product action to which `IndexedFairness` attaches weak fairness.
***************************************************************************)
SuccessorActivationProtocolInvariant ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    LET owner == SuccessorActivationOwner(parentContext, node)
    IN /\ (owner \in successorActivationFailures
              => successorActivationStatus[parentContext][node]
                    = "Running")
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

(***************************************************************************
An exact durable application identifies the canonical next context, but its
subject is known to be valid only through the ChainEpoch receipt invariant.
Keeping that premise explicit prevents the failure latch from using an
arbitrary admissible witness for the outer progress-step existential and then
leaving a failed owner whose exact recovery context is outside the admissible
indexed product.
***************************************************************************)
THEOREM ExactDurableParentApplicationHasAdmissibleSuccessorContext ==
  \A parentContext \in AdmissibleContextRecords,
     node \in ValidatorIds,
     application \in Chain!DecisionEvidenceSet:
    /\ Chain!ChainEpochInvariant
    /\ ExactDurableParentApplication(parentContext, node, application)
    => CanonicalIndexedContext(parentContext.height + 1)
         \in AdmissibleContextRecords
PROOF
  <1>1. ASSUME NEW parentContext \in AdmissibleContextRecords,
              NEW node \in ValidatorIds,
              NEW application \in Chain!DecisionEvidenceSet,
              Chain!ChainEpochInvariant,
              ExactDurableParentApplication(
                parentContext, node, application)
         PROVE CanonicalIndexedContext(parentContext.height + 1)
                  \in AdmissibleContextRecords
    <2>1. parentContext.height + 1 \in Heights
      BY <1>1, Isa
         DEF ExactDurableParentApplication,
             AdmissibleContextRecords, FrozenContextAdmissible,
             ContextRecords, LineagesAt, Heights
    <2>2. CanonicalIndexedContext(parentContext.height + 1)
             \in ContextRecords
      BY <1>1, Isa
         DEF ExactDurableParentApplication,
             Chain!ChainEpochInvariant,
             Chain!ChainEpochTypeInvariant
    <2>3. parentContext.height + 1 <= certifiedHeight
      BY <1>1, Isa
         DEF ExactDurableParentApplication,
             Chain!ChainEpochInvariant,
             Chain!NodesDoNotOutrunCertificates
    <2>4. \A index \in 1..(parentContext.height + 1):
             decidedAt[index] \in ValidSubjects
      BY <1>1, <2>3, Isa
         DEF Chain!ChainEpochInvariant,
             Chain!CertifiedPrefixBacked
    <2>5. \A index \in
               DOMAIN CanonicalIndexedContext(
                 parentContext.height + 1).lineage:
             CanonicalIndexedContext(
               parentContext.height + 1).lineage[index]
               \in ValidSubjects
      BY <2>1, <2>4, Isa
         DEF CanonicalIndexedContext,
             Chain!ContextRecord, Chain!HistoryThrough
    <2> QED BY <2>2, <2>5
         DEF AdmissibleContextRecords, FrozenContextAdmissible
  <1> QED BY <1>1

THEOREM SuccessorActivationProgressPreservesProtocolInvariant ==
  \A selectedParent \in AdmissibleContextRecords,
     selectedNode \in ValidatorIds:
    Chain!ChainEpochInvariant
      /\ SuccessorActivationProtocolInvariant
      /\ IndexedSuccessorActivationProgressStep(
           selectedParent, selectedNode)
      => SuccessorActivationProtocolInvariant'
BY ExactDurableParentApplicationHasAdmissibleSuccessorContext,
   ExpandENABLED, Isa
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
       LatchAppliedSuccessorStartupFailure,
       LatchRecoveredSuccessorStartupFailure,
       RehydrateCleanCompleteTipSuccessorStartup,
       RehydrateFailedSuccessorStartup,
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
       LatchAppliedSuccessorStartupFailure,
       LatchRecoveredSuccessorStartupFailure,
       RehydrateCleanCompleteTipSuccessorStartup,
       RehydrateFailedSuccessorStartup,
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
         DEF IndexedCompositionInvariant
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
        /\ SuccessorActivationFailureAbsent(parentContext, node)
        /\ SuccessorActivationFailureAbsent(parentContext, node)'
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
           \/ SuccessorActivationPending(parentContext, node)'
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

SuccessorActivationTemporalKernel(parentContext, node) ==
  /\ []IndexedCompositionInvariant
  /\ []SuccessorActivationProtocolInvariant
  /\ [][IndexedChainNext]_IndexedChainVars
  /\ WF_IndexedChainVars(
       IndexedSuccessorActivationProgressStep(parentContext, node))

SuccessorActivationFailureFreeSuffix(parentContext, node) ==
  []SuccessorActivationFailureAbsent(parentContext, node)

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

THEOREM CleanCompleteTipRestartDescendsPublishedTier ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive,
     successorContext \in AdmissibleContextRecords,
     application \in Chain!DecisionEvidenceSet:
    /\ SuccessorActivationProtocolInvariant
    /\ SuccessorActivationPending(parentContext, node)
    /\ SuccessorActivationFailureAbsent(parentContext, node)
    /\ RehydrateCleanCompleteTipSuccessorStartup(
         parentContext, node, successorContext, application)
    => /\ SuccessorActivationPending(parentContext, node)'
       /\ SuccessorActivationRank(parentContext, node)'
            < SuccessorActivationRank(parentContext, node)
BY Isa
   DEF SuccessorActivationProtocolInvariant,
       SuccessorActivationPending,
       SuccessorActivationRank,
       SuccessorActivationPipelineDistance,
       SuccessorActivationFailureAbsent,
       IndexedSuccessorActivationPending,
       SuccessorPublicationOrSuperseded,
       SuccessorHeightActivated,
       RehydrateCleanCompleteTipSuccessorStartup,
       ExactDurableParentApplication,
       CompleteTipRecoveryAuthorityRecord,
       SuccessorActivationOwner,
       SuccessorActivationToken,
       SuccessorActivationMarker,
       SuccessorActivationEnvironmentStutter,
       IndexedChainVars, SuccessorActivationVars,
       Chain!ChainEpochVars

THEOREM SuccessorActivationFailureFreeProgressStrictlyDecreasesRank ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    SuccessorActivationProtocolInvariant
      /\ SuccessorActivationPending(parentContext, node)
      /\ SuccessorActivationFailureAbsent(parentContext, node)
      /\ SuccessorActivationFailureAbsent(parentContext, node)'
      /\ IndexedSuccessorActivationProgressStep(parentContext, node)
      => \/ SuccessorPublicationOrSuperseded(parentContext, node)'
         \/ /\ SuccessorActivationPending(parentContext, node)'
            /\ SuccessorActivationRank(parentContext, node)'
                 < SuccessorActivationRank(parentContext, node)
BY CleanCompleteTipRestartDescendsPublishedTier, Isa
   DEF SuccessorActivationProtocolInvariant,
       SuccessorActivationPending,
       SuccessorActivationRank,
       SuccessorActivationPipelineDistance,
       SuccessorActivationFailureAbsent,
       IndexedSuccessorActivationPending,
       SuccessorPublicationOrSuperseded,
       SuccessorHeightActivated,
       IndexedSuccessorActivationProgressStep,
       BeginSuccessorActivation,
       BindAppliedSuccessorActivationToken,
       LatchAppliedSuccessorStartupFailure,
       LatchRecoveredSuccessorStartupFailure,
       RehydrateCleanCompleteTipSuccessorStartup,
       RehydrateFailedSuccessorStartup,
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
           /\ SuccessorActivationFailureAbsent(parentContext, node)
           /\ SuccessorActivationFailureAbsent(parentContext, node)'
           /\ IndexedSuccessorActivationProgressStep(parentContext, node)
           => \/ SuccessorPublicationOrSuperseded(parentContext, node)'
              \/ /\ SuccessorActivationPending(parentContext, node)'
                 /\ SuccessorActivationRank(parentContext, node)'
                      < SuccessorActivationRank(parentContext, node)
    BY SuccessorActivationFailureFreeProgressStrictlyDecreasesRank
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
         \/ SuccessorActivationPending(parentContext, node)'
BY IndexedStepPreservesSuccessorActivationProtocolInvariant,
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
       LatchAppliedSuccessorStartupFailure,
       LatchRecoveredSuccessorStartupFailure,
       RehydrateCleanCompleteTipSuccessorStartup,
       RehydrateFailedSuccessorStartup,
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

THEOREM IndexedFailureFreeStepDoesNotRaiseSuccessorActivationRank ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ IndexedCompositionInvariant
    /\ SuccessorActivationProtocolInvariant
    /\ SuccessorActivationPending(parentContext, node)
    /\ SuccessorActivationFailureAbsent(parentContext, node)
    /\ SuccessorActivationFailureAbsent(parentContext, node)'
    /\ [IndexedChainNext]_IndexedChainVars
    => \/ SuccessorPublicationOrSuperseded(parentContext, node)'
       \/ /\ SuccessorActivationPending(parentContext, node)'
          /\ SuccessorActivationRank(parentContext, node)'
               <= SuccessorActivationRank(parentContext, node)
BY IndexedStepDoesNotOrphanSuccessorActivation,
   SuccessorActivationFailureFreeProgressStrictlyDecreasesRank,
   Isa
   DEF SuccessorActivationProtocolInvariant,
       SuccessorActivationPending,
       SuccessorActivationRank,
       SuccessorActivationPipelineDistance,
       SuccessorActivationFailureAbsent,
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
       LatchAppliedSuccessorStartupFailure,
       LatchRecoveredSuccessorStartupFailure,
       RehydrateCleanCompleteTipSuccessorStartup,
       RehydrateFailedSuccessorStartup,
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

THEOREM SuccessorActivationFailureFreeRankPersistsOrExits ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in SuccessorActivationRankCarrier:
    /\ IndexedCompositionInvariant
    /\ SuccessorActivationProtocolInvariant
    /\ SuccessorActivationAtRank(parentContext, node, rank)
    /\ SuccessorActivationFailureAbsent(parentContext, node)
    /\ SuccessorActivationFailureAbsent(parentContext, node)'
    /\ [IndexedChainNext]_IndexedChainVars
    => \/ SuccessorActivationAtRank(parentContext, node, rank)'
       \/ SuccessorActivationRankExit(parentContext, node, rank)'
BY IndexedFailureFreeStepDoesNotRaiseSuccessorActivationRank,
   IndexedStepPreservesSuccessorActivationProtocolInvariant,
   Isa
   DEF SuccessorActivationAtRank, SuccessorActivationRankExit,
       SuccessorActivationProtocolInvariant,
       SuccessorActivationRankCarrier, SetLessThan, OpToRel

THEOREM SuccessorActivationFailureFreeProgressExitsCurrentRank ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in SuccessorActivationRankCarrier:
    /\ Chain!ChainEpochInvariant
    /\ SuccessorActivationProtocolInvariant
    /\ SuccessorActivationAtRank(parentContext, node, rank)
    /\ SuccessorActivationFailureAbsent(parentContext, node)
    /\ SuccessorActivationFailureAbsent(parentContext, node)'
    /\ <<IndexedSuccessorActivationProgressStep(
           parentContext, node)>>_(IndexedChainVars)
    => SuccessorActivationRankExit(parentContext, node, rank)'
BY SuccessorActivationFailureFreeProgressStrictlyDecreasesRank,
   SuccessorActivationProgressPreservesProtocolInvariant,
   Isa
   DEF SuccessorActivationAtRank, SuccessorActivationRankExit,
       SuccessorActivationProtocolInvariant,
       SuccessorActivationRankCarrier, SetLessThan, OpToRel,
       IndexedChainVars

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
              \/ SuccessorActivationPending(parentContext, node)'
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
       LatchAppliedSuccessorStartupFailure,
       LatchRecoveredSuccessorStartupFailure,
       RehydrateCleanCompleteTipSuccessorStartup,
       RehydrateFailedSuccessorStartup,
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

THEOREM SuccessorActivationAtRankEnablesFairProgress ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in SuccessorActivationRankCarrier:
    SuccessorActivationProtocolInvariant
      /\ SuccessorActivationAtRank(parentContext, node, rank)
      => ENABLED
           <<IndexedSuccessorActivationProgressStep(
               parentContext, node)>>_(IndexedChainVars)
BY Isa DEF SuccessorActivationProtocolInvariant,
           SuccessorActivationAtRank,
           SuccessorActivationPending

THEOREM SuccessorActivationPendingHasRankWitness ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    SuccessorActivationProtocolInvariant
      /\ SuccessorActivationPending(parentContext, node)
      => \E rank \in SuccessorActivationRankCarrier:
           SuccessorActivationAtRank(parentContext, node, rank)
BY SuccessorActivationProtocolPendingRankIsInCarrier, Isa
   DEF SuccessorActivationAtRank

THEOREM IndexedChainSpecEstablishesSuccessorActivationTemporalKernel ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    IndexedChainSpec
      => SuccessorActivationTemporalKernel(parentContext, node)
PROOF
  <1>1. ASSUME NEW parentContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              IndexedChainSpec
         PROVE SuccessorActivationTemporalKernel(parentContext, node)
    <2>1. []IndexedCompositionInvariant
      BY <1>1, IndexedChainSpecEstablishesCompositionInvariant, PTL
    <2>2. []SuccessorActivationProtocolInvariant
      BY <1>1,
         IndexedChainSpecEstablishesSuccessorActivationProtocolInvariant,
         PTL
    <2>3. [][IndexedChainNext]_IndexedChainVars
      BY <1>1, PTL DEF IndexedChainSpec
    <2>4. WF_IndexedChainVars(
             IndexedSuccessorActivationProgressStep(parentContext, node))
      BY <1>1 DEF IndexedChainSpec, IndexedFairness
    <2> QED BY <2>1, <2>2, <2>3, <2>4
         DEF SuccessorActivationTemporalKernel
  <1> QED BY <1>1

THEOREM FailureFreeSuccessorActivationRankLeadsToExit ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive,
     rank \in SuccessorActivationRankCarrier:
    /\ SuccessorActivationTemporalKernel(parentContext, node)
    /\ SuccessorActivationFailureFreeSuffix(parentContext, node)
    => (SuccessorActivationAtRank(parentContext, node, rank)
          ~> SuccessorActivationRankExit(parentContext, node, rank))
PROOF
  <1>1. ASSUME NEW parentContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              NEW rank \in SuccessorActivationRankCarrier,
              SuccessorActivationTemporalKernel(parentContext, node),
              SuccessorActivationFailureFreeSuffix(parentContext, node)
         PROVE SuccessorActivationAtRank(parentContext, node, rank)
                 ~> SuccessorActivationRankExit(
                      parentContext, node, rank)
    <2>1. []IndexedCompositionInvariant
      BY <1>1 DEF SuccessorActivationTemporalKernel
    <2>2. []SuccessorActivationProtocolInvariant
      BY <1>1 DEF SuccessorActivationTemporalKernel
    <2>3. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF SuccessorActivationTemporalKernel
    <2>4. WF_IndexedChainVars(
             IndexedSuccessorActivationProgressStep(parentContext, node))
      BY <1>1 DEF SuccessorActivationTemporalKernel
    <2>5. []SuccessorActivationFailureAbsent(parentContext, node)
      BY <1>1 DEF SuccessorActivationFailureFreeSuffix
    <2>6. /\ IndexedCompositionInvariant
            /\ SuccessorActivationProtocolInvariant
            /\ SuccessorActivationAtRank(parentContext, node, rank)
            /\ SuccessorActivationFailureAbsent(parentContext, node)
            /\ SuccessorActivationFailureAbsent(parentContext, node)'
            /\ [IndexedChainNext]_IndexedChainVars
           => \/ SuccessorActivationAtRank(
                    parentContext, node, rank)'
              \/ SuccessorActivationRankExit(
                   parentContext, node, rank)'
      BY <1>1, SuccessorActivationFailureFreeRankPersistsOrExits
    <2>7. SuccessorActivationProtocolInvariant
             /\ SuccessorActivationAtRank(parentContext, node, rank)
           => ENABLED
                <<IndexedSuccessorActivationProgressStep(
                    parentContext, node)>>_(IndexedChainVars)
      BY <1>1, SuccessorActivationAtRankEnablesFairProgress
    <2>8. /\ Chain!ChainEpochInvariant
            /\ SuccessorActivationProtocolInvariant
            /\ SuccessorActivationAtRank(parentContext, node, rank)
            /\ SuccessorActivationFailureAbsent(parentContext, node)
            /\ SuccessorActivationFailureAbsent(parentContext, node)'
            /\ <<IndexedSuccessorActivationProgressStep(
                   parentContext, node)>>_(IndexedChainVars)
           => SuccessorActivationRankExit(parentContext, node, rank)'
      BY <1>1, <2>1,
         SuccessorActivationFailureFreeProgressExitsCurrentRank, PTL
         DEF IndexedCompositionInvariant
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, <2>7,
                  <2>8, PTL
  <1> QED BY <1>1

THEOREM FailureFreeSuccessorActivationRankConverges ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ SuccessorActivationTemporalKernel(parentContext, node)
    /\ SuccessorActivationFailureFreeSuffix(parentContext, node)
    => \A rank \in SuccessorActivationRankCarrier:
         SuccessorActivationAtRank(parentContext, node, rank)
           ~> SuccessorPublicationOrSuperseded(parentContext, node)
PROOF
  <1>1. ASSUME NEW parentContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              SuccessorActivationTemporalKernel(parentContext, node),
              SuccessorActivationFailureFreeSuffix(parentContext, node)
         PROVE \A rank \in SuccessorActivationRankCarrier:
                 SuccessorActivationAtRank(parentContext, node, rank)
                   ~> SuccessorPublicationOrSuperseded(
                        parentContext, node)
    <2>1. IsWellFoundedOn(
             OpToRel(<, Nat), SuccessorActivationRankCarrier)
      BY SuccessorActivationRankOrderingIsWellFounded
    <2>2. \A rank \in SuccessorActivationRankCarrier:
            SuccessorActivationAtRank(parentContext, node, rank)
              ~> SuccessorActivationRankExit(
                   parentContext, node, rank)
      BY <1>1, FailureFreeSuccessorActivationRankLeadsToExit
    <2>3. \A rank \in SuccessorActivationRankCarrier:
            SuccessorActivationAtRank(parentContext, node, rank)
              ~> (SuccessorPublicationOrSuperseded(parentContext, node)
                   \/ \E lower \in SetLessThan(
                        rank, OpToRel(<, Nat),
                        SuccessorActivationRankCarrier):
                        SuccessorActivationAtRank(
                          parentContext, node, lower))
      BY <2>2 DEF SuccessorActivationRankExit
    <2> QED BY <2>1, <2>3, WellFoundedLeadsTo
  <1> QED BY <1>1

THEOREM SuccessorActivationRankQuantifierEquivalence ==
  \A parentContext, node:
    (\A rank \in SuccessorActivationRankCarrier:
       SuccessorActivationAtRank(parentContext, node, rank)
         => <>SuccessorPublicationOrSuperseded(parentContext, node))
      <=> ((\E rank \in SuccessorActivationRankCarrier:
               SuccessorActivationAtRank(parentContext, node, rank))
             => <>SuccessorPublicationOrSuperseded(parentContext, node))
BY SMT

THEOREM AlwaysSuccessorActivationRankQuantifierEquivalence ==
  []SuccessorActivationRankQuantifierEquivalence
BY SuccessorActivationRankQuantifierEquivalence, PTL

THEOREM BoxedSuccessorActivationRankQuantifierEquivalence ==
  \A parentContext, node:
    [](\A rank \in SuccessorActivationRankCarrier:
         SuccessorActivationAtRank(parentContext, node, rank)
           => <>SuccessorPublicationOrSuperseded(parentContext, node))
      <=> []((\E rank \in SuccessorActivationRankCarrier:
                SuccessorActivationAtRank(parentContext, node, rank))
               => <>SuccessorPublicationOrSuperseded(parentContext, node))
PROOF
  <1>1. \A parentContext, node:
          []((\A rank \in SuccessorActivationRankCarrier:
                SuccessorActivationAtRank(parentContext, node, rank)
                  => <>SuccessorPublicationOrSuperseded(
                       parentContext, node))
             <=> ((\E rank \in SuccessorActivationRankCarrier:
                      SuccessorActivationAtRank(
                        parentContext, node, rank))
                    => <>SuccessorPublicationOrSuperseded(
                         parentContext, node)))
    BY AlwaysSuccessorActivationRankQuantifierEquivalence, Isa
  <1>2. ASSUME NEW parentContext, NEW node
         PROVE [](\A rank \in SuccessorActivationRankCarrier:
                    SuccessorActivationAtRank(
                      parentContext, node, rank)
                      => <>SuccessorPublicationOrSuperseded(
                           parentContext, node))
                 <=> []((\E rank \in SuccessorActivationRankCarrier:
                            SuccessorActivationAtRank(
                              parentContext, node, rank))
                           => <>SuccessorPublicationOrSuperseded(
                                parentContext, node))
    <2>1. []((\A rank \in SuccessorActivationRankCarrier:
                SuccessorActivationAtRank(parentContext, node, rank)
                  => <>SuccessorPublicationOrSuperseded(
                       parentContext, node))
               <=> ((\E rank \in SuccessorActivationRankCarrier:
                        SuccessorActivationAtRank(
                          parentContext, node, rank))
                      => <>SuccessorPublicationOrSuperseded(
                           parentContext, node)))
      BY <1>1, SMT
    <2> QED BY <2>1, PTL
  <1> QED BY <1>2

THEOREM SuccessorActivationRankExistentialLift ==
  ASSUME NEW parentContext,
         NEW node,
         \A rank \in SuccessorActivationRankCarrier:
           SuccessorActivationAtRank(parentContext, node, rank)
             ~> SuccessorPublicationOrSuperseded(parentContext, node)
  PROVE (\E rank \in SuccessorActivationRankCarrier:
           SuccessorActivationAtRank(parentContext, node, rank))
          ~> SuccessorPublicationOrSuperseded(parentContext, node)
PROOF
  <1>1. (\A rank \in SuccessorActivationRankCarrier:
            [](SuccessorActivationAtRank(parentContext, node, rank)
                 => <>SuccessorPublicationOrSuperseded(
                       parentContext, node)))
          <=> [](\A rank \in SuccessorActivationRankCarrier:
                   SuccessorActivationAtRank(parentContext, node, rank)
                     => <>SuccessorPublicationOrSuperseded(
                          parentContext, node))
    OBVIOUS
  <1>2. [](\A rank \in SuccessorActivationRankCarrier:
             SuccessorActivationAtRank(parentContext, node, rank)
               => <>SuccessorPublicationOrSuperseded(parentContext, node))
          <=> []((\E rank \in SuccessorActivationRankCarrier:
                     SuccessorActivationAtRank(parentContext, node, rank))
                   => <>SuccessorPublicationOrSuperseded(
                        parentContext, node))
    BY ONLY BoxedSuccessorActivationRankQuantifierEquivalence, SMT
  <1> QED BY <1>1, <1>2, PTL

THEOREM FailureFreeSuccessorActivationConverges ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ SuccessorActivationTemporalKernel(parentContext, node)
    /\ SuccessorActivationFailureFreeSuffix(parentContext, node)
    => (SuccessorActivationPending(parentContext, node)
          ~> SuccessorPublicationOrSuperseded(parentContext, node))
PROOF
  <1>1. ASSUME NEW parentContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              SuccessorActivationTemporalKernel(parentContext, node),
              SuccessorActivationFailureFreeSuffix(parentContext, node)
         PROVE SuccessorActivationPending(parentContext, node)
                 ~> SuccessorPublicationOrSuperseded(
                      parentContext, node)
    <2>1. \A rank \in SuccessorActivationRankCarrier:
            SuccessorActivationAtRank(parentContext, node, rank)
              ~> SuccessorPublicationOrSuperseded(parentContext, node)
      BY <1>1, FailureFreeSuccessorActivationRankConverges
    <2>2. (\E rank \in SuccessorActivationRankCarrier:
             SuccessorActivationAtRank(parentContext, node, rank))
            ~> SuccessorPublicationOrSuperseded(parentContext, node)
      BY <2>1, SuccessorActivationRankExistentialLift
    <2>3. []SuccessorActivationProtocolInvariant
      BY <1>1 DEF SuccessorActivationTemporalKernel
    <2>4. SuccessorActivationProtocolInvariant
             /\ SuccessorActivationPending(parentContext, node)
           => \E rank \in SuccessorActivationRankCarrier:
                SuccessorActivationAtRank(parentContext, node, rank)
      BY <1>1, SuccessorActivationPendingHasRankWitness
    <2> QED BY <2>2, <2>3, <2>4, PTL
  <1> QED BY <1>1

THEOREM SuccessorActivationTemporalKernelIsSuffixClosed ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    SuccessorActivationTemporalKernel(parentContext, node)
      => []SuccessorActivationTemporalKernel(parentContext, node)
BY PTL DEF SuccessorActivationTemporalKernel

THEOREM FailureFreeSuccessorActivationConvergenceAtEverySuffix ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    []( /\ SuccessorActivationTemporalKernel(parentContext, node)
        /\ SuccessorActivationFailureFreeSuffix(parentContext, node)
        => (SuccessorActivationPending(parentContext, node)
              ~> SuccessorPublicationOrSuperseded(parentContext, node)))
PROOF
  <1>1. ASSUME NEW parentContext \in AdmissibleContextRecords,
              NEW node \in Responsive
         PROVE []( /\ SuccessorActivationTemporalKernel(
                         parentContext, node)
                    /\ SuccessorActivationFailureFreeSuffix(
                         parentContext, node)
                    => (SuccessorActivationPending(parentContext, node)
                          ~> SuccessorPublicationOrSuperseded(
                               parentContext, node)))
    <2>1. /\ SuccessorActivationTemporalKernel(parentContext, node)
            /\ SuccessorActivationFailureFreeSuffix(parentContext, node)
           => (SuccessorActivationPending(parentContext, node)
                 ~> SuccessorPublicationOrSuperseded(parentContext, node))
      BY <1>1, FailureFreeSuccessorActivationConverges
    <2> QED BY <2>1, PTL
  <1> QED BY <1>1

THEOREM EventualFailureFreeSuffixLiftsSuccessorConvergence ==
  \A parentContext \in AdmissibleContextRecords,
     node \in Responsive:
    /\ SuccessorActivationTemporalKernel(parentContext, node)
    /\ <>SuccessorActivationFailureFreeSuffix(parentContext, node)
    => (SuccessorActivationPending(parentContext, node)
          ~> SuccessorPublicationOrSuperseded(parentContext, node))
PROOF
  <1>1. ASSUME NEW parentContext \in AdmissibleContextRecords,
              NEW node \in Responsive,
              SuccessorActivationTemporalKernel(parentContext, node),
              <>SuccessorActivationFailureFreeSuffix(parentContext, node)
         PROVE SuccessorActivationPending(parentContext, node)
                 ~> SuccessorPublicationOrSuperseded(
                      parentContext, node)
    <2>1. []SuccessorActivationTemporalKernel(parentContext, node)
      BY <1>1, SuccessorActivationTemporalKernelIsSuffixClosed
    <2>2. []( /\ SuccessorActivationTemporalKernel(parentContext, node)
              /\ SuccessorActivationFailureFreeSuffix(parentContext, node)
              => (SuccessorActivationPending(parentContext, node)
                    ~> SuccessorPublicationOrSuperseded(
                         parentContext, node)))
      BY <1>1,
         FailureFreeSuccessorActivationConvergenceAtEverySuffix
    <2>3. []IndexedCompositionInvariant
      BY <1>1 DEF SuccessorActivationTemporalKernel
    <2>4. []SuccessorActivationProtocolInvariant
      BY <1>1 DEF SuccessorActivationTemporalKernel
    <2>5. [][IndexedChainNext]_IndexedChainVars
      BY <1>1 DEF SuccessorActivationTemporalKernel
    <2>6. /\ IndexedCompositionInvariant
            /\ SuccessorActivationProtocolInvariant
            /\ SuccessorActivationPending(parentContext, node)
            /\ [IndexedChainNext]_IndexedChainVars
           => \/ SuccessorPublicationOrSuperseded(parentContext, node)'
              \/ SuccessorActivationPending(parentContext, node)'
      BY <1>1, IndexedStepDoesNotOrphanSuccessorActivation
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, <2>5, <2>6, PTL
  <1> QED BY <1>1

THEOREM IndexedChainSpecEstablishesSuccessorActivationStarvationFreedom ==
  IndexedChainSpec => SuccessorActivationStarvationFreedomProperty
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE SuccessorActivationStarvationFreedomProperty
    <2>1. ASSUME NEW parentContext \in AdmissibleContextRecords,
                NEW node \in Responsive
           PROVE SuccessorActivationPending(parentContext, node)
                   ~> SuccessorPublicationOrSuperseded(
                        parentContext, node)
      <3>1. SuccessorActivationTemporalKernel(parentContext, node)
        BY <1>1, <2>1,
           IndexedChainSpecEstablishesSuccessorActivationTemporalKernel
      <3>2. <>SuccessorActivationFailureFreeSuffix(parentContext, node)
        BY <1>1, <2>1, PTL
           DEF IndexedChainSpec,
               EventualFailureFreeSuccessorStartupSuffix,
               SuccessorActivationFailureFreeSuffix,
               SuccessorActivationFailureAbsent
      <3> QED BY <3>1, <3>2,
           EventualFailureFreeSuffixLiftsSuccessorConvergence
    <2> QED BY <2>1 DEF SuccessorActivationStarvationFreedomProperty
  <1> QED BY <1>1

THEOREM IndexedChainSpecEstablishesSuccessorActivationRankProgress ==
  IndexedChainSpec => SuccessorActivationRankProgressProperty
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE SuccessorActivationRankProgressProperty
    <2>1. SuccessorActivationStarvationFreedomProperty
      BY <1>1,
         IndexedChainSpecEstablishesSuccessorActivationStarvationFreedom
    <2> QED BY <2>1, PTL
         DEF SuccessorActivationRankProgressProperty,
             SuccessorActivationStarvationFreedomProperty,
             SuccessorActivationAtRank,
             SuccessorActivationRankExit
  <1> QED BY <1>1

THEOREM SuccessorActivationStarvationFreedomObligation ==
  IndexedChainSpec
    => /\ SuccessorActivationPendingStructureProperty
       /\ SuccessorActivationStepDecreasesRankProperty
       /\ SuccessorActivationPendingIsNotOrphanedProperty
       /\ SuccessorActivationOutcomeIsStableProperty
       /\ SuccessorActivationRankProgressProperty
       /\ SuccessorActivationStarvationFreedomProperty
PROOF
  <1>1. ASSUME IndexedChainSpec
         PROVE /\ SuccessorActivationPendingStructureProperty
               /\ SuccessorActivationStepDecreasesRankProperty
               /\ SuccessorActivationPendingIsNotOrphanedProperty
               /\ SuccessorActivationOutcomeIsStableProperty
               /\ SuccessorActivationRankProgressProperty
               /\ SuccessorActivationStarvationFreedomProperty
    <2>1. SuccessorActivationPendingStructureProperty
      BY <1>1,
         IndexedChainSpecEstablishesSuccessorActivationPendingStructure
    <2>2. SuccessorActivationStepDecreasesRankProperty
      BY <1>1, IndexedChainSpecEstablishesSuccessorActivationStepDecrease
    <2>3. SuccessorActivationPendingIsNotOrphanedProperty
      BY <1>1,
         IndexedChainSpecEstablishesSuccessorActivationNonOrphaning
    <2>4. SuccessorActivationOutcomeIsStableProperty
      BY <1>1,
         IndexedChainSpecEstablishesSuccessorActivationOutcomeStability
    <2>5. SuccessorActivationRankProgressProperty
      BY <1>1,
         IndexedChainSpecEstablishesSuccessorActivationRankProgress
    <2>6. SuccessorActivationStarvationFreedomProperty
      BY <1>1,
         IndexedChainSpecEstablishesSuccessorActivationStarvationFreedom
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5, <2>6
  <1> QED BY <1>1

THEOREM SuccessorActivationStarvationMatchesChainProgress ==
  SuccessorActivationStarvationFreedomProperty
    <=> IndexedSuccessorActivationProgress
BY DEF SuccessorActivationStarvationFreedomProperty,
       SuccessorActivationPending,
       IndexedSuccessorActivationProgress

=============================================================================
