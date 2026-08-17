---- MODULE SumeragiV2ChainEpochRefinementShard08 ----
EXTENDS SumeragiV2ChainEpochRefinementShard07

THEOREM IndexedInitEstablishesEveryInstanceStrongInvariant ==
  IndexedChainInit => IndexedEveryInstanceStrongInvariant
PROOF
  <1>1. ASSUME IndexedChainInit,
               NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedAsync(initialContext)!StrongInductiveInvariant
    <2>1. IndexedAsync(initialContext)!AsyncInitAt(initialContext)
      BY <1>1 DEF IndexedChainInit
    <2>2. IndexedAsync(initialContext)!InitAt(initialContext)
      BY <2>1 DEF IndexedAsync!AsyncInitAt,
                    IndexedAsync!AsyncBaseInitAt
    <2> QED BY <2>2, SMT
  <1> QED BY <1>1 DEF IndexedEveryInstanceStrongInvariant

THEOREM IndexedInitEstablishesEveryInstanceAsyncStrongTypeInvariant ==
  IndexedChainInit => IndexedEveryInstanceAsyncStrongTypeInvariant
PROOF
  <1>1. ASSUME IndexedChainInit,
               NEW initialContext \in AdmissibleContextRecords
         PROVE IndexedAsync(initialContext)!AsyncStrongTypeInvariant
    <2>1. IndexedAsync(initialContext)!AsyncInitAt(initialContext)
      BY <1>1, IndexedInitProjectsEveryAsyncInit
    <2> QED BY <2>1,
         IndexedAsyncInitEstablishesStrongTypeInvariant
  <1> QED BY <1>1
       DEF IndexedEveryInstanceAsyncStrongTypeInvariant

THEOREM IndexedInitEstablishesServiceActivationCoherence ==
  IndexedChainInit => IndexedServiceActivationCoherence
PROOF
  <1>1. ASSUME IndexedChainInit,
               NEW initialContext \in AdmissibleContextRecords
         PROVE
           /\ IndexedAsync(initialContext)!
                AsyncServiceActivationPairInvariant
           /\ IF IndexedAsync(initialContext)!
                   AsyncServiceActivationRestricted
              THEN /\ joinedByContext[initialContext] # {}
                   /\ IndexedAsync(initialContext)!
                        AsyncActiveServiceNodes
                        = joinedByContext[initialContext]
              ELSE /\ IndexedAsync(initialContext)!
                        AsyncActiveServiceNodes = ValidatorIds
                   /\ \/ initialContext = GenesisContext
                      \/ /\ joinedByContext[initialContext] = {}
                         /\ IndexedAsync(initialContext)!
                              AsyncServiceActivationClockPristine
    <2>1. IndexedAsync(initialContext)!AsyncStrongTypeInvariant
      BY <1>1,
         IndexedInitEstablishesEveryInstanceAsyncStrongTypeInvariant
         DEF IndexedEveryInstanceAsyncStrongTypeInvariant
    <2>2. IndexedAsync(initialContext)!
             AsyncServiceActivationPairInvariant
      BY <2>1
         DEF IndexedAsync!AsyncStrongTypeInvariant
    <2>3. IndexedAsync(initialContext)!AsyncInitAt(initialContext)
      BY <1>1, IndexedInitProjectsEveryAsyncInit
    <2> QED BY <1>1, <2>2, <2>3, Isa
         DEF IndexedChainInit, GenesisContext,
             IndexedAsync!AsyncInitAt,
             IndexedAsync!AsyncBaseInitAt,
             IndexedAsync!AsyncRuntimeInit,
             IndexedAsync!AsyncTransportInit,
             IndexedAsync!AsyncServiceActivationRestricted,
             IndexedAsync!AsyncActiveServiceNodes,
             IndexedAsync!AsyncServiceActivationClockPristine
  <1> QED BY <1>1
       DEF IndexedServiceActivationCoherence,
           IndexedServiceActivationMembershipCoherenceAt

THEOREM IndexedInitEstablishesPostGstContextJoinedCoherence ==
  IndexedChainInit => IndexedPostGstContextJoinedCoherence
BY Isa
   DEF IndexedChainInit,
       IndexedPostGstContextJoinedCoherence,
       IndexedAsync!AsyncInitAt,
       IndexedAsync!AsyncBaseInitAt,
       IndexedAsync!InitAt,
       JoinedContexts

THEOREM IndexedInitEstablishesPostGstResponsiveActiveRosterCoherence ==
  IndexedChainInit => IndexedPostGstResponsiveActiveRosterCoherence
BY Isa
   DEF IndexedChainInit,
       IndexedPostGstResponsiveActiveRosterCoherence,
       IndexedAsync!AsyncInitAt,
       IndexedAsync!AsyncBaseInitAt,
       IndexedAsync!InitAt,
       IndexedAsync!AsyncActiveServiceNodes

THEOREM IndexedInitEstablishesCompositionInvariant ==
  IndexedChainInit => IndexedCompositionInvariant
BY Isa, Chain!GenesisEstablishesChainEpochInvariant,
   IndexedChainInitHasEmptyCurrentReceiptUnion,
   IndexedInitEstablishesEveryInstanceStrongInvariant,
   IndexedInitEstablishesEveryInstanceAsyncStrongTypeInvariant,
   IndexedInitEstablishesServiceActivationCoherence,
   IndexedInitEstablishesPostGstContextJoinedCoherence,
   IndexedInitEstablishesPostGstResponsiveActiveRosterCoherence
   DEF IndexedChainInit, IndexedCompositionInvariant,
       IndexedTotalReceiptProjection,
       IndexedDecisionReceiptProjection,
       IndexedApplicationReceiptProjection,
       JoinedContextCertificationInvariant, JoinedRoutingInvariant,
       IndexedApplicationsRespectNodeHeight,
       IndexedEveryInstanceAsyncStrongTypeInvariant,
       IndexedServiceActivationCoherence,
       IndexedPostGstContextJoinedCoherence,
       IndexedPostGstResponsiveActiveRosterCoherence,
       JoinedContexts,
       IndexedNodeCurrentAt, GenesisContext,
       IndexedAsync!NodeHasApplication,
       IndexedAsync!AsyncVotersAt, IndexedAsync!InitAt,
       IndexedAsync!BootstrapParentDecision

THEOREM IndexedActionPreservesEveryInstanceStrongInvariant ==
  IndexedCompositionInvariant /\ IndexedChainNext
    => IndexedEveryInstanceStrongInvariant'
PROOF
  <1>1. ASSUME IndexedCompositionInvariant,
              IndexedChainNext,
              NEW initialContext \in AdmissibleContextRecords
         PROVE (IndexedAsync(initialContext)!
                  StrongInductiveInvariant)'
    <2>1. IndexedAsync(initialContext)!StrongInductiveInvariant
      BY <1>1 DEF IndexedCompositionInvariant,
                    IndexedEveryInstanceStrongInvariant
    <2>2. IndexedAsync(initialContext)!AsyncAllVars
               = IndexedAsyncStateAt(initialContext)
      BY <1>1, IndexedInstanceVariablesAreExact
         DEF IndexedCompositionInvariant
    <2>3. [IndexedAsync(initialContext)!AsyncNext]_(
             IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedStepProjectsEveryAsyncStep
    <2>4. [IndexedAsync(initialContext)!Next]_(
             IndexedAsync(initialContext)!vars)
      BY <2>2, <2>3, Isa
         DEF IndexedAsync!AsyncNext, IndexedAsync!AsyncAllVars
    <2> QED BY <2>1, <2>4, SMT
  <1> QED BY <1>1 DEF IndexedEveryInstanceStrongInvariant

THEOREM IndexedActionPreservesEveryInstanceAsyncStrongTypeInvariant ==
  IndexedCompositionInvariant /\ IndexedChainNext
    => IndexedEveryInstanceAsyncStrongTypeInvariant'
PROOF
  <1>1. ASSUME IndexedCompositionInvariant,
              IndexedChainNext,
              NEW initialContext \in AdmissibleContextRecords
         PROVE (IndexedAsync(initialContext)!
                  AsyncStrongTypeInvariant)'
    <2>1. IndexedAsync(initialContext)!AsyncStrongTypeInvariant
      BY <1>1
         DEF IndexedCompositionInvariant,
             IndexedEveryInstanceAsyncStrongTypeInvariant
    <2>2. IndexedAsync(initialContext)!AsyncAllVars
               = IndexedAsyncStateAt(initialContext)
      BY <1>1, IndexedInstanceVariablesAreExact
         DEF IndexedCompositionInvariant
    <2>3. [IndexedAsync(initialContext)!AsyncNext]_(
             IndexedAsyncStateAt(initialContext))
      BY <1>1, IndexedStepProjectsEveryAsyncStep
    <2>4. [IndexedAsync(initialContext)!AsyncNext]_(
             IndexedAsync(initialContext)!AsyncAllVars)
      BY <2>2, <2>3, Isa
    <2> QED BY <2>1, <2>4,
         IndexedAsyncBracketNextPreservesStrongTypeInvariant
  <1> QED BY <1>1
       DEF IndexedEveryInstanceAsyncStrongTypeInvariant

(***************************************************************************
Atomic join/activation guard audit.

The branch selector reads unprimed joined membership.  The same final action
then primes both joinedByContext and scheduler component 46.  Consequently a
first join can only burn the restriction tombstone and install the singleton
active owner, while a later join can only monotonically add and rearm that
exact node.  Neither publication path can expose joined membership one step
before its service clocks become active.
***************************************************************************)
SuccessorFinalPublicationAction(parentContext, node, successorContext) ==
  \/ ActivateAppliedSuccessorHeight(
       parentContext, node, successorContext)
  \/ ActivateRecoveredSuccessorHeight(
       parentContext, node, successorContext)

THEOREM FirstSuccessorJoinAtomicallyRestrictsServiceActivation ==
  \A parentContext, successorContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    /\ joinedByContext[successorContext] = {}
    /\ SuccessorFinalPublicationAction(
         parentContext, node, successorContext)
    => /\ joinedByContext'[successorContext] = {node}
       /\ (IndexedAsync(successorContext)!
             AsyncServiceActivationRestricted)'
       /\ (IndexedAsync(successorContext)!AsyncActiveServiceNodes)'
            = {node}
       /\ indexedAsyncState'[successorContext][3][33][node]
            = AsyncDeliveryBound
       /\ indexedAsyncState'[successorContext][3][34][node]
            = AsyncDeliveryBound
BY Isa
   DEF SuccessorFinalPublicationAction,
       ActivateAppliedSuccessorHeight,
       ActivateRecoveredSuccessorHeight,
       SuccessorActivationEnvironmentActivatesNode,
       IndexedAsync!AsyncEnterIndexedServiceActivation,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes,
       IndexedScheduler

THEOREM LaterSuccessorJoinAtomicallyRearmsServiceActivation ==
  \A parentContext, successorContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    /\ joinedByContext[successorContext] # {}
    /\ SuccessorFinalPublicationAction(
         parentContext, node, successorContext)
    => /\ IndexedAsync(successorContext)!
             AsyncServiceActivationRestricted
       /\ (IndexedAsync(successorContext)!
             AsyncServiceActivationRestricted)'
       /\ joinedByContext'[successorContext]
            = joinedByContext[successorContext] \cup {node}
       /\ (IndexedAsync(successorContext)!AsyncActiveServiceNodes)'
            = IndexedAsync(successorContext)!AsyncActiveServiceNodes
                \cup {node}
       /\ node \in
            (IndexedAsync(successorContext)!AsyncActiveServiceNodes)'
       /\ indexedAsyncState'[successorContext][3][33][node]
            = IndexedScheduler(successorContext, 1)
                + AsyncDeliveryBound
       /\ indexedAsyncState'[successorContext][3][34][node]
            = IndexedScheduler(successorContext, 1)
                + AsyncDeliveryBound
BY Isa
   DEF SuccessorFinalPublicationAction,
       ActivateAppliedSuccessorHeight,
       ActivateRecoveredSuccessorHeight,
       SuccessorActivationEnvironmentActivatesNode,
       IndexedAsync!AsyncActivateServiceNode,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes,
       IndexedScheduler

THEOREM IndexedProductActionPreservesServiceActivationMembership ==
  \A selectedContext \in AdmissibleContextRecords:
    /\ IndexedCompositionInvariant
    /\ IndexedProductActionAt(selectedContext)
    => \A initialContext \in AdmissibleContextRecords:
         (IndexedServiceActivationMembershipCoherenceAt(
            initialContext))'
BY Isa
   DEF IndexedCompositionInvariant,
       IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedProductActionAt,
       IndexedJoinedAsyncNext,
       IndexedReceiptClassification,
       IndexedReceiptFreeChainStutter,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes,
       IndexedAsync!AsyncServiceActivationClockPristine,
       IndexedScheduler

THEOREM IndexedSuccessorActivationActionPreservesServiceActivationMembership ==
  \A parentContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    /\ IndexedCompositionInvariant
    /\ IndexedSuccessorActivationProgressStep(parentContext, node)
    => \A initialContext \in AdmissibleContextRecords:
         (IndexedServiceActivationMembershipCoherenceAt(
            initialContext))'
BY FirstSuccessorJoinAtomicallyRestrictsServiceActivation,
   LaterSuccessorJoinAtomicallyRearmsServiceActivation, Isa
   DEF IndexedCompositionInvariant,
       IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
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
       SuccessorActivationEnvironmentActivatesNode,
       IndexedAsync!AsyncEnterIndexedServiceActivation,
       IndexedAsync!AsyncActivateServiceNode,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes,
       IndexedAsync!AsyncServiceActivationClockPristine,
       IndexedAsyncStateAt, IndexedScheduler

THEOREM IndexedActionPreservesServiceActivationCoherence ==
  IndexedCompositionInvariant /\ IndexedChainNext
    => IndexedServiceActivationCoherence'
PROOF
  <1>1. ASSUME IndexedCompositionInvariant,
              IndexedChainNext
         PROVE IndexedServiceActivationCoherence'
    <2>1. IndexedEveryInstanceAsyncStrongTypeInvariant'
      BY <1>1,
         IndexedActionPreservesEveryInstanceAsyncStrongTypeInvariant
    <2>2. \A initialContext \in AdmissibleContextRecords:
             (IndexedAsync(initialContext)!
                AsyncServiceActivationPairInvariant)'
      BY <2>1
         DEF IndexedEveryInstanceAsyncStrongTypeInvariant,
             IndexedAsync!AsyncStrongTypeInvariant
    <2>3. \A initialContext \in AdmissibleContextRecords:
             (IndexedServiceActivationMembershipCoherenceAt(
                initialContext))'
      BY <1>1,
         IndexedProductActionPreservesServiceActivationMembership,
         IndexedSuccessorActivationActionPreservesServiceActivationMembership,
         Isa DEF IndexedChainNext
    <2> QED BY <2>2, <2>3
         DEF IndexedServiceActivationCoherence
  <1> QED BY <1>1

THEOREM IndexedActionKeepsServiceActivationRestrictionIrreversible ==
  IndexedCompositionInvariant /\ IndexedChainNext
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!AsyncServiceActivationRestricted
           => (IndexedAsync(initialContext)!
                 AsyncServiceActivationRestricted)'
BY FirstSuccessorJoinAtomicallyRestrictsServiceActivation,
   LaterSuccessorJoinAtomicallyRearmsServiceActivation, Isa
   DEF IndexedCompositionInvariant,
       IndexedServiceActivationCoherence,
       IndexedChainNext, IndexedProductActionAt,
       IndexedJoinedAsyncNext,
       IndexedSuccessorActivationProgressStep,
       SuccessorFinalPublicationAction,
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
       SuccessorActivationEnvironmentStutter,
       SuccessorActivationEnvironmentActivatesNode,
       ActivateAppliedSuccessorHeight,
       ActivateRecoveredSuccessorHeight,
       IndexedAsync!AsyncEnterIndexedServiceActivation,
       IndexedAsync!AsyncActivateServiceNode,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedScheduler

THEOREM IndexedStepKeepsServiceActivationRestrictionIrreversible ==
  IndexedCompositionInvariant
    /\ [IndexedChainNext]_IndexedChainVars
    => \A initialContext \in AdmissibleContextRecords:
         IndexedAsync(initialContext)!AsyncServiceActivationRestricted
           => (IndexedAsync(initialContext)!
                 AsyncServiceActivationRestricted)'
BY IndexedActionKeepsServiceActivationRestrictionIrreversible, Isa
   DEF IndexedChainVars, IndexedAsyncStateAt,
       IndexedCore, IndexedScheduler, IndexedRecovery

THEOREM IndexedStutterPreservesServiceActivationCoherence ==
  IndexedServiceActivationCoherence
    /\ UNCHANGED IndexedChainVars
    => IndexedServiceActivationCoherence'
BY Isa
   DEF IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedChainVars, IndexedAsyncStateAt,
       IndexedCore, IndexedScheduler, IndexedRecovery

=============================================================================
