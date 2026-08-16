---- MODULE SumeragiV2ChainEpochRefinementShard09 ----
EXTENDS SumeragiV2ChainEpochRefinementShard08

THEOREM IndexedStepPreservesServiceActivationCoherence ==
  IndexedCompositionInvariant
    /\ [IndexedChainNext]_IndexedChainVars
    => IndexedServiceActivationCoherence'
BY IndexedActionPreservesServiceActivationCoherence,
   IndexedStutterPreservesServiceActivationCoherence, Isa

THEOREM IndexedNewGstRequiresJoinedContext ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedChainNext
    /\ ~IndexedCore(initialContext, 7)
    /\ (IndexedCore(initialContext, 7))'
    => /\ initialContext \in JoinedContexts
       /\ initialContext \in JoinedContexts'
BY JoinedMembershipIsMonotone, Isa
   DEF IndexedChainNext, IndexedProductActionAt,
       IndexedJoinedAsyncNext,
       IndexedSuccessorActivationProgressStep,
       SuccessorActivationEnvironmentStutter,
       SuccessorActivationEnvironmentActivatesNode,
       IndexedAsync!AsyncEnterIndexedServiceActivation,
       IndexedAsync!AsyncActivateServiceNode,
       IndexedAsync!AsyncServiceActivationFrameVars,
       IndexedAsyncStateAt, IndexedCore,
       JoinedContexts

THEOREM IndexedActionPreservesPostGstContextJoinedCoherence ==
  IndexedCompositionInvariant /\ IndexedChainNext
    => IndexedPostGstContextJoinedCoherence'
BY IndexedNewGstRequiresJoinedContext,
   JoinedMembershipIsMonotone,
   IndexedGstAsyncStepIsMonotone,
   IndexedStepProjectsEveryAsyncStep,
   IndexedInstanceVariablesAreExact, Isa
   DEF IndexedCompositionInvariant,
       IndexedPostGstContextJoinedCoherence,
       IndexedChainNext, IndexedChainVars

THEOREM IndexedStutterPreservesPostGstContextJoinedCoherence ==
  IndexedPostGstContextJoinedCoherence
    /\ UNCHANGED IndexedChainVars
    => IndexedPostGstContextJoinedCoherence'
BY Isa
   DEF IndexedPostGstContextJoinedCoherence,
       IndexedChainVars, JoinedContexts,
       IndexedAsyncStateAt, IndexedCore,
       IndexedScheduler, IndexedRecovery

THEOREM IndexedStepPreservesPostGstContextJoinedCoherence ==
  IndexedCompositionInvariant
    /\ [IndexedChainNext]_IndexedChainVars
    => IndexedPostGstContextJoinedCoherence'
BY IndexedActionPreservesPostGstContextJoinedCoherence,
   IndexedStutterPreservesPostGstContextJoinedCoherence, Isa

THEOREM IndexedNewGstRequiresResponsiveActiveRoster ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedCompositionInvariant
    /\ IndexedChainNext
    /\ ~IndexedCore(initialContext, 7)
    /\ (IndexedCore(initialContext, 7))'
    => /\ Responsive \subseteq
             IndexedAsync(initialContext)!AsyncActiveServiceNodes
       /\ Responsive \subseteq
             (IndexedAsync(initialContext)!AsyncActiveServiceNodes)'
BY IndexedStepProjectsEveryAsyncStep,
   IndexedInstanceVariablesAreExact, Isa
   DEF IndexedChainNext, IndexedProductActionAt,
       IndexedJoinedAsyncNext,
       IndexedSuccessorActivationProgressStep,
       SuccessorActivationEnvironmentStutter,
       SuccessorActivationEnvironmentActivatesNode,
       IndexedAsync!AsyncNext,
       IndexedAsync!AsyncNonCrashStep,
       IndexedAsync!AsyncSetGST,
       IndexedAsync!AsyncServiceActivationTransition,
       IndexedAsync!AsyncEnterIndexedServiceActivation,
       IndexedAsync!AsyncActivateServiceNode,
       IndexedAsync!AsyncServiceActivationFrameVars,
       IndexedAsync!AsyncActiveServiceNodes,
       IndexedAsyncStateAt, IndexedCore, IndexedScheduler

THEOREM IndexedPostGstResponsiveActiveRosterSurvivesAction ==
  \A initialContext \in AdmissibleContextRecords:
    /\ IndexedCompositionInvariant
    /\ IndexedChainNext
    /\ IndexedCore(initialContext, 7)
    /\ Responsive \subseteq
         IndexedAsync(initialContext)!AsyncActiveServiceNodes
    => Responsive \subseteq
         (IndexedAsync(initialContext)!AsyncActiveServiceNodes)'
BY IndexedStepProjectsEveryAsyncStep,
   IndexedInstanceVariablesAreExact, Isa
   DEF IndexedChainNext, IndexedProductActionAt,
       IndexedJoinedAsyncNext,
       IndexedSuccessorActivationProgressStep,
       SuccessorActivationEnvironmentStutter,
       SuccessorActivationEnvironmentActivatesNode,
       IndexedAsync!AsyncNext,
       IndexedAsync!AsyncNonCrashStep,
       IndexedAsync!AsyncServiceActivationTransition,
       IndexedAsync!AsyncEnterIndexedServiceActivation,
       IndexedAsync!AsyncActivateServiceNode,
       IndexedAsync!AsyncServiceActivationClockPristine,
       IndexedAsync!AsyncServiceActivationFrameVars,
       IndexedAsync!AsyncActiveServiceNodes,
       IndexedAsyncStateAt, IndexedCore, IndexedScheduler

THEOREM IndexedActionPreservesPostGstResponsiveActiveRosterCoherence ==
  IndexedCompositionInvariant /\ IndexedChainNext
    => IndexedPostGstResponsiveActiveRosterCoherence'
BY IndexedNewGstRequiresResponsiveActiveRoster,
   IndexedPostGstResponsiveActiveRosterSurvivesAction,
   IndexedGstAsyncStepIsMonotone,
   IndexedStepProjectsEveryAsyncStep,
   IndexedInstanceVariablesAreExact, Isa
   DEF IndexedCompositionInvariant,
       IndexedPostGstResponsiveActiveRosterCoherence,
       IndexedChainNext, IndexedChainVars

THEOREM IndexedStutterPreservesPostGstResponsiveActiveRosterCoherence ==
  IndexedPostGstResponsiveActiveRosterCoherence
    /\ UNCHANGED IndexedChainVars
    => IndexedPostGstResponsiveActiveRosterCoherence'
BY Isa
   DEF IndexedPostGstResponsiveActiveRosterCoherence,
       IndexedChainVars, IndexedAsyncStateAt,
       IndexedCore, IndexedScheduler, IndexedRecovery

THEOREM IndexedStepPreservesPostGstResponsiveActiveRosterCoherence ==
  IndexedCompositionInvariant
    /\ [IndexedChainNext]_IndexedChainVars
    => IndexedPostGstResponsiveActiveRosterCoherence'
BY IndexedActionPreservesPostGstResponsiveActiveRosterCoherence,
   IndexedStutterPreservesPostGstResponsiveActiveRosterCoherence, Isa

THEOREM IndexedActionPreservesCompositionInvariant ==
  IndexedCompositionInvariant /\ IndexedChainNext
    => IndexedCompositionInvariant'
BY Isa, AppliedSuccessorIsAdmissible,
   IndexedStepProjectsChainEpochStep,
   Chain!ChainEpochInductiveStep,
   IndexedStepPreservesReceiptProjection,
   IndexedActionPreservesEveryInstanceStrongInvariant,
   IndexedActionPreservesEveryInstanceAsyncStrongTypeInvariant,
   IndexedActionPreservesServiceActivationCoherence,
   IndexedActionPreservesPostGstContextJoinedCoherence,
   IndexedActionPreservesPostGstResponsiveActiveRosterCoherence
   DEF IndexedCompositionInvariant, IndexedChainNext,
       IndexedProductActionAt, IndexedReceiptClassification,
       IndexedReceiptFreeChainStutter,
       IndexedDecisionReceiptHandoff,
       IndexedApplicationReceiptHandoff,
       NewIndexedDecisionReceipt, NewIndexedApplicationReceipt,
       NoNewIndexedDurableReceipt,
       IndexedEveryInstanceStrongInvariant,
       IndexedEveryInstanceAsyncStrongTypeInvariant,
       IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedPostGstContextJoinedCoherence,
       IndexedPostGstResponsiveActiveRosterCoherence,
       JoinedContextCertificationInvariant, JoinedRoutingInvariant,
       IndexedApplicationsRespectNodeHeight,
       IndexedNodeCurrentAt, ExactNodeLocationAt,
       JoinedContexts, SuccessorContextFor,
       IndexedCurrentDecisions, IndexedCurrentApplications,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!ContextsMatchLocalHistories,
       Chain!RecordCertifiedNext, Chain!RecordKnownDecision,
       Chain!RecordAppliedNext, Chain!RecordKnownApplication,
       IndexedAsync!StrongInductiveInvariant,
       IndexedAsync!Safety, IndexedAsync!TypeInvariant,
       IndexedAsync!DecisionAgreement,
       IndexedAsync!AppliedRequiresDecision,
       IndexedAsync!NodeHasApplication

THEOREM IndexedStepPreservesCompositionInvariant ==
  IndexedCompositionInvariant /\ [IndexedChainNext]_IndexedChainVars
    => IndexedCompositionInvariant'
PROOF
  <1>1. ASSUME IndexedCompositionInvariant,
              [IndexedChainNext]_IndexedChainVars
         PROVE IndexedCompositionInvariant'
    <2>1. CASE IndexedChainNext
      BY <1>1, <2>1, IndexedActionPreservesCompositionInvariant
    <2>2. CASE UNCHANGED IndexedChainVars
      BY <1>1, <2>2, Isa
         DEF IndexedChainVars, IndexedCompositionInvariant,
             IndexedEveryInstanceStrongInvariant,
             IndexedEveryInstanceAsyncStrongTypeInvariant,
             JoinedContextCertificationInvariant, JoinedRoutingInvariant,
             IndexedApplicationsRespectNodeHeight,
             IndexedServiceActivationCoherence,
             IndexedServiceActivationMembershipCoherenceAt,
             IndexedPostGstContextJoinedCoherence,
             IndexedPostGstResponsiveActiveRosterCoherence,
             IndexedTotalReceiptProjection,
             IndexedDecisionReceiptProjection,
             IndexedApplicationReceiptProjection,
             IndexedDecisionEvidence, IndexedApplicationEvidence,
             IndexedCurrentDecisions, IndexedCurrentApplications,
             IndexedAsyncStateAt, IndexedCore, IndexedScheduler,
             IndexedRecovery,
             JoinedContexts, IndexedNodeCurrentAt,
             Chain!ChainEpochVars
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

THEOREM JoinedCanonicalDescendantIsStable ==
  \A initialContext \in AdmissibleContextRecords:
    IndexedCompositionInvariant
      /\ JoinedCanonicalDescendant(initialContext)
      /\ [IndexedChainNext]_IndexedChainVars
      => JoinedCanonicalDescendant(initialContext)'
BY Isa, JoinedMembershipIsMonotone,
   IndexedStepPreservesCompositionInvariant
   DEF JoinedCanonicalDescendant, JoinedContexts,
       IndexedChainVars, IndexedCompositionInvariant,
       JoinedContextCertificationInvariant,
       Chain!ChainEpochInvariant, Chain!ChainEpochTypeInvariant,
       Chain!ContextRecord, Chain!HistoryThrough

THEOREM IndexedChainSpecEstablishesCompositionInvariant ==
  IndexedChainSpec => []IndexedCompositionInvariant
PROOF
  <1>1. IndexedChainInit => IndexedCompositionInvariant
    BY IndexedInitEstablishesCompositionInvariant
  <1>2. IndexedCompositionInvariant
           /\ [IndexedChainNext]_IndexedChainVars
           => IndexedCompositionInvariant'
    BY IndexedStepPreservesCompositionInvariant
  <1> QED BY <1>1, <1>2, PTL DEF IndexedChainSpec

THEOREM IndexedChainSpecAlwaysKeepsPostGstContextsJoined ==
  IndexedChainSpec
    => []IndexedPostGstContextJoinedCoherence
BY IndexedChainSpecEstablishesCompositionInvariant, PTL
   DEF IndexedCompositionInvariant

THEOREM IndexedChainSpecAlwaysKeepsPostGstResponsiveRosterActive ==
  IndexedChainSpec
    => []IndexedPostGstResponsiveActiveRosterCoherence
BY IndexedChainSpecEstablishesCompositionInvariant, PTL
   DEF IndexedCompositionInvariant

=============================================================================
