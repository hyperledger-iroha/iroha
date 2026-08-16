---- MODULE SumeragiV2ChainEpochRefinementShard07 ----
EXTENDS SumeragiV2ChainEpochRefinementShard06

THEOREM JoinedNodeNeverWaitsForAllPeers ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in IndexedAsync(initialContext)!
                  AsyncCurrentResponsiveVoters:
      (/\ IndexedServiceActivationCoherence
       /\ IndexedNodeCurrentAt(initialContext, node)
       /\ IndexedAsync(initialContext)!RunNode(node))
        => /\ node \in IndexedAsync(initialContext)!
                       AsyncActiveServiceNodes
           /\ IndexedJoinedRunnerStep(initialContext)
BY Isa
   DEF IndexedServiceActivationCoherence,
       IndexedServiceActivationMembershipCoherenceAt,
       IndexedNodeCurrentAt, IndexedJoinedRunnerStep,
       IndexedAsync!AsyncServiceActivationRestricted,
       IndexedAsync!AsyncActiveServiceNodes

THEOREM HistoricalServiceSurvivesLocalAdvance ==
  \A initialContext \in AdmissibleContextRecords:
    \A node \in IndexedAsync(initialContext)!
                  AsyncCurrentResponsiveVoters:
      (/\ node \in joinedByContext[initialContext]
       /\ IndexedAsync(initialContext)!RunHistoricalServer(node))
        => IndexedJoinedRunnerStep(initialContext)
BY DEF IndexedJoinedRunnerStep

(***************************************************************************
Historical recovery always copies the source QC exactly.  Slot identity is
split at the finite horizon: nonterminal contexts name the canonical successor
slot, while a terminal context can only carry outside-horizon receipt identity.
There is no DecisionSlots member at MaxHeight + 1.
***************************************************************************)

THEOREM HistoricalRecoveryOpenCopiesExactIdentity ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds, server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    IndexedOpenHistoricalRecovery(initialContext, node, server, source)
      => /\ HistoricalRecoveryRecord(node, source).qc.context
                = initialContext
         /\ HistoricalRecoveryRecord(node, source).qc = source.qc
         /\ HistoricalRecoveryRecord(node, source).qc.subject
                = source.qc.subject
BY DEF IndexedOpenHistoricalRecovery,
       IndexedHistoricalRecoverySourceReady, HistoricalRecoveryRecord,
       IndexedCurrentDecisions

THEOREM NonterminalHistoricalRecoveryCopiesCanonicalSlotIdentity ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds, server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    initialContext.height < MaxHeight
      /\ IndexedOpenHistoricalRecovery(
           initialContext, node, server, source)
      => /\ HistoricalRecoveryRecord(node, source).qc.context
                = initialContext
         /\ HistoricalRecoveryRecord(node, source).qc = source.qc
         /\ Chain!CanonicalCommitForSlot(
              HistoricalRecoveryRecord(node, source).qc,
              initialContext.height + 1)
BY Isa DEF IndexedOpenHistoricalRecovery,
           IndexedHistoricalRecoverySourceReady, HistoricalRecoveryRecord,
           IndexedCurrentDecisions

THEOREM TerminalHistoricalRecoveryCopiesOutsideHorizonIdentity ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds, server \in ValidatorIds,
     source \in Chain!DecisionEvidenceSet:
    initialContext.height = MaxHeight
      /\ IndexedOpenHistoricalRecovery(
           initialContext, node, server, source)
      => /\ HistoricalRecoveryRecord(node, source).qc.context
                = initialContext
         /\ HistoricalRecoveryRecord(node, source).qc = source.qc
         /\ Chain!ReceiptOutsideChainHorizon(
              HistoricalRecoveryRecord(node, source))
BY Isa DEF IndexedOpenHistoricalRecovery,
           IndexedHistoricalRecoverySourceReady, HistoricalRecoveryRecord,
           IndexedCurrentDecisions,
           Chain!ReceiptOutsideChainHorizon

THEOREM SuccessorRosterEntrantIsHistoricalRecoveryEligible ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    ( /\ initialContext.height < MaxHeight
      /\ node \in Responsive
      /\ node \in VotingRoster(ExpectedEpoch(initialContext.height + 1))
      /\ node \notin IndexedAsync(initialContext)!
                       AsyncVotersAt(initialContext)
      /\ nodeHeight[node] = initialContext.height
      /\ nodeContext[node] = initialContext
      /\ ~IndexedAsync(initialContext)!NodeHasApplication(node))
      => /\ node \in Responsive
         /\ ExactNodeLocationAt(initialContext, node)
         /\ ~IndexedProjectedNodeHasApplication(initialContext, node)
BY DEF ExactNodeLocationAt, IndexedProjectedNodeHasApplication,
       HistoricalRecoveryNodeHasApplicationProjection,
       IndexedAsync!NodeHasApplication

(***************************************************************************
Regression witness for the production restart path: old-roster membership is
not a requester exclusion.  A responsive validator that restarts at its exact
old context and lacks that context's application is eligible for the same
authenticated CommitQC/body recovery as an observer or successor entrant.
***************************************************************************)
THEOREM RestartedCurrentRosterValidatorIsHistoricalRecoveryEligible ==
  \A initialContext \in AdmissibleContextRecords,
     node \in ValidatorIds:
    ( /\ initialContext.height < MaxHeight
      /\ node \in Responsive
      /\ node \in IndexedAsync(initialContext)!
                   AsyncVotersAt(initialContext)
      /\ nodeHeight[node] = initialContext.height
      /\ nodeContext[node] = initialContext
      /\ ~IndexedAsync(initialContext)!NodeHasApplication(node))
      => /\ node \in Responsive
         /\ ExactNodeLocationAt(initialContext, node)
         /\ ~IndexedProjectedNodeHasApplication(initialContext, node)
BY DEF ExactNodeLocationAt, IndexedProjectedNodeHasApplication,
       HistoricalRecoveryNodeHasApplicationProjection,
       IndexedAsync!NodeHasApplication

THEOREM JoinedMembershipIsMonotone ==
  IndexedChainNext
    => \A initialContext \in AdmissibleContextRecords:
         joinedByContext[initialContext]
           \subseteq joinedByContext'[initialContext]
BY Isa DEF IndexedChainNext, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff

THEOREM IndexedNodeHeightsAreMonotone ==
  IndexedChainNext
    => \A node \in ValidatorIds: nodeHeight[node] <= nodeHeight'[node]
BY Isa DEF IndexedChainNext, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           Chain!RecordCertifiedNext, Chain!RecordKnownDecision,
           Chain!RecordAppliedNext, Chain!RecordKnownApplication

THEOREM IndexedBracketStepKeepsNodeHeightsMonotone ==
  [IndexedChainNext]_IndexedChainVars
    => \A node \in ValidatorIds: nodeHeight[node] <= nodeHeight'[node]
BY Isa, IndexedNodeHeightsAreMonotone
   DEF IndexedChainVars, Chain!ChainEpochVars

THEOREM IndexedStepProjectsChainEpochStep ==
  IndexedChainNext => [Chain!ChainEpochNext]_Chain!ChainEpochVars
BY Isa DEF IndexedChainNext, IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           Chain!ChainEpochNext

(***************************************************************************
Final successor publication is an exact Async service-activation action.
For the selected successor instance, use the production theorem which embeds
both restriction and monotone rearm into AsyncNext.  Every other pre-created
instance stutters extensionally.  This is the component-46 projection seam;
ordinary joined work is handled separately by JoinedAsyncStepRefinesExact.
***************************************************************************)
THEOREM SuccessorActivationEnvironmentProjectsEveryAsyncStep ==
  \A successorContext \in AdmissibleContextRecords,
     node \in ValidatorIds,
     observedContext \in AdmissibleContextRecords:
    /\ IndexedAsyncStateShape
    /\ SuccessorActivationEnvironmentActivatesNode(
         successorContext, node)
    => [IndexedAsync(observedContext)!AsyncNext]_(
         IndexedAsyncStateAt(observedContext))
PROOF
  <1>1. ASSUME NEW successorContext \in AdmissibleContextRecords,
              NEW node \in ValidatorIds,
              NEW observedContext \in AdmissibleContextRecords,
              IndexedAsyncStateShape,
              SuccessorActivationEnvironmentActivatesNode(
                successorContext, node)
         PROVE [IndexedAsync(observedContext)!AsyncNext]_(
                 IndexedAsyncStateAt(observedContext))
    <2> QED BY <1>1,
         IndexedAsync(successorContext)!
           AsyncServiceActivationActionsRefineAsyncNext,
         IndexedInstanceVariablesAreExact, Isa
         DEF SuccessorActivationEnvironmentActivatesNode,
             IndexedAsyncStateAt
  <1> QED BY <1>1

THEOREM IndexedSuccessorActivationStepProjectsEveryAsyncStep ==
  \A parentContext \in AdmissibleContextRecords,
     node \in ValidatorIds,
     observedContext \in AdmissibleContextRecords:
    /\ IndexedAsyncStateShape
    /\ IndexedSuccessorActivationProgressStep(parentContext, node)
      => [IndexedAsync(observedContext)!AsyncNext]_(
           IndexedAsyncStateAt(observedContext))
BY SuccessorActivationEnvironmentProjectsEveryAsyncStep, Isa
   DEF IndexedSuccessorActivationProgressStep,
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
       IndexedAsyncStateAt

THEOREM IndexedStepProjectsEveryAsyncStep ==
  \A observedContext \in AdmissibleContextRecords:
    IndexedChainNext
      => [IndexedAsync(observedContext)!AsyncNext]_(
           IndexedAsyncStateAt(observedContext))
BY IndexedSuccessorActivationStepProjectsEveryAsyncStep,
   JoinedAsyncStepRefinesExactAsyncStep,
   IndexedInstanceVariablesAreExact, Isa
   DEF IndexedChainNext, IndexedProductActionAt

THEOREM IndexedInitEstablishesReceiptProjection ==
  IndexedChainInit => IndexedTotalReceiptProjection
BY DEF IndexedChainInit

THEOREM IndexedStepPreservesReceiptProjection ==
  IndexedCompositionInvariant /\ [IndexedChainNext]_IndexedChainVars
    => IndexedTotalReceiptProjection'
BY Isa DEF IndexedChainNext, IndexedChainVars,
           IndexedCompositionInvariant,
           IndexedReceiptClassification,
           IndexedReceiptFreeChainStutter,
           IndexedDecisionReceiptHandoff,
           IndexedApplicationReceiptHandoff,
           IndexedTotalReceiptProjection,
           IndexedDecisionReceiptProjection,
           IndexedApplicationReceiptProjection,
           IndexedDecisionEvidence, IndexedApplicationEvidence,
           IndexedCurrentDecisions, IndexedCurrentApplications,
           NewIndexedDecisionReceipt, NewIndexedApplicationReceipt,
           NoNewIndexedDurableReceipt

(***************************************************************************
The initialized and step-preserved product invariant closes the two state
seams used by the temporal composition: routing after a local advance, and
canonical/certified activation of a successor instance.
***************************************************************************)
=============================================================================
