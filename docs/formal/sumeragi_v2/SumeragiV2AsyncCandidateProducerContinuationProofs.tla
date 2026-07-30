---- MODULE SumeragiV2AsyncCandidateProducerContinuationProofs ----
EXTENDS SumeragiV2AsyncCausalWorkBudgetProofs

(***************************************************************************
Source, type, and transition facts for the internal producer-continuation
table.  This module proves only safety and atomic state transfer.  It does not
claim that every adequate-leader semantic branch exposes a resolvable exact
successor, and therefore does not provide the wider producer/transport
temporal closure.
***************************************************************************)

AsyncCandidateProducerContinuationExactOwner(
    target, leaderContext, leader, leaderView, subject) ==
  \E record \in AsyncCandidateProducerContinuations:
    /\ record.status \in {"Reserved", "Materialized"}
    /\ record.node \in {target, leader}
    /\ record.context = leaderContext
    /\ record.height = leaderContext.height
    /\ record.view = leaderView
    /\ record.identity.leader = leader
    /\ record.subject = subject
    /\ record.phase \in AsyncCandidateServiceTrackedKinds
    /\ (record.phase \in {"BeginDecision", "PersistDecision"}
          => record.node = target)
    /\ record.identity.payload.causalOrigin = record.causalOrigin

THEOREM AsyncCandidateProducerContinuationConstructorIsTyped ==
  \A candidate \in AsyncCandidateSet,
     handoffCandidates \in SUBSET AsyncCandidateSet,
     lifecycleSlot \in AsyncCandidateLifecycleSlots,
     ordinal \in Nat \ {0},
     status \in AsyncCandidateProducerContinuationStatuses:
    candidate.kind \in AsyncCandidateServiceTrackedKinds
      => /\ AsyncCandidateProducerContinuationRecord(
               candidate, handoffCandidates,
               lifecycleSlot, ordinal, status)
             \in AsyncCandidateProducerContinuationRecordSet
         /\ (AsyncCandidateProducerContinuationRecord(
                candidate, handoffCandidates,
                lifecycleSlot, ordinal, status)).sourceClass
              = AsyncCandidateProducerContinuationSourceClass(candidate)
         /\ (AsyncCandidateProducerContinuationRecord(
                candidate, handoffCandidates,
                lifecycleSlot, ordinal, status)).address
              \in AsyncCandidateServiceStageOwnerAddresses
BY AsyncCandidateServiceTrackedKindProjectionIsCovered, SMT
   DEF AsyncCandidateProducerContinuationRecord,
       AsyncCandidateProducerContinuationRecordSet,
       AsyncCandidateServiceStageOwnerAddresses

THEOREM AsyncCandidateProducerContinuationStatusRankIsNatural ==
  \A status \in AsyncCandidateProducerContinuationStatuses:
    AsyncCandidateProducerContinuationStatusRank(status) \in Nat
BY SMT
   DEF AsyncCandidateProducerContinuationStatuses,
       AsyncCandidateProducerContinuationStatusRank

THEOREM AsyncCandidateProducerContinuationHandoffRetainsExactLifecycle ==
  \A candidate \in AsyncCandidateSet,
     successor \in
       AsyncCandidateProducerContinuationHandoffCandidatesThisStep(
         candidate):
    /\ successor \in AsyncCandidateSet
    /\ successor.node = candidate.node
    /\ successor.causalOrigin = candidate.causalOrigin
BY CommandSuccessorsRetainCausalOrigin, Isa
   DEF AsyncCandidateProducerContinuationHandoffCandidatesThisStep,
       AsyncCandidateServicesThisStep, SequenceSet

THEOREM AsyncCandidateIgnoredDepartureDeclaresNoReplayHandoff ==
  \A candidate \in AsyncCandidateSet:
    candidate \in AsyncCandidateIgnoredWithoutApplicationThisStepSet
      => AsyncCandidateProducerContinuationHandoffCandidatesThisStep(
           candidate) = {}
BY Isa
   DEF AsyncCandidateProducerContinuationHandoffCandidatesThisStep,
       AsyncCandidateIgnoredWithoutApplicationThisStepSet,
       AsyncCandidateIgnoredWithoutApplicationThisStep,
       AsyncCandidateServicesThisStep,
       AsyncCandidateSemanticallyAppliedThisStep

THEOREM AsyncCandidateProducerContinuationDepartureSplitsSourceOrGoal ==
  \A candidate:
    AsyncCandidateProducerContinuationDeparture(candidate)
      => \/ AsyncCandidateProducerContinuationGoalAfter(candidate)
         \/ AsyncCandidateProducerContinuationSourceAfter(candidate)
BY Isa
   DEF AsyncCandidateProducerContinuationSourceAfter,
       AsyncCandidateProducerContinuationDeparture

\* Compatibility name retained while downstream proofs expose the physical
\* replay class. Every non-goal tracked departure now installs a continuation;
\* the third arm refines its external replay obligation rather than excluding
\* it from the continuation table.
THEOREM AsyncCandidateProducerContinuationDepartureSplitsSourceResidualOrGoal ==
  \A candidate:
    AsyncCandidateProducerContinuationDeparture(candidate)
      => \/ AsyncCandidateProducerContinuationGoalAfter(candidate)
         \/ AsyncCandidateProducerContinuationSourceAfter(candidate)
         \/ AsyncCandidateProducerTransportResidualAfter(candidate)
BY AsyncCandidateProducerContinuationDepartureSplitsSourceOrGoal

THEOREM AsyncCandidateProducerContinuationLocalSourceExcludesTransportResidual ==
  \A candidate:
    /\ AsyncCandidateProducerContinuationSourceAfter(candidate)
    /\ AsyncCandidateProducerContinuationSourceClass(candidate) = "Local"
      => ~AsyncCandidateProducerTransportResidualAfter(candidate)
BY AsyncCandidateProducerContinuationKindPartition, Isa
   DEF AsyncCandidateProducerContinuationSourceAfter,
       AsyncCandidateProducerTransportResidualAfter,
       AsyncCandidateProducerContinuationSourceClass,
       AsyncCandidateProducerContinuationLocallyReconstructibleKinds,
       AsyncCandidateProducerContinuationExternalResidualKinds

THEOREM AsyncCandidateProducerTransportResidualIsContinuationSource ==
  \A candidate:
    AsyncCandidateProducerTransportResidualAfter(candidate)
      => /\ AsyncCandidateProducerContinuationSourceAfter(candidate)
         /\ AsyncCandidateProducerContinuationSourceClass(candidate)
              \in {"ConditionalTransport", "VolatileBody"}
BY AsyncCandidateProducerContinuationKindPartition, Isa
   DEF AsyncCandidateProducerContinuationSourceAfter,
       AsyncCandidateProducerTransportResidualAfter,
       AsyncCandidateProducerContinuationSourceClass,
       AsyncCandidateProducerContinuationExternalResidualKinds,
       AsyncCandidateProducerContinuationConditionalResponsiveTransportKinds,
       AsyncCandidateProducerContinuationVolatileBodyReconstructionKinds

THEOREM AsyncCandidateProducerTransportResidualSplitsPhysicalClass ==
  \A candidate:
    AsyncCandidateProducerTransportResidualAfter(candidate)
      => /\ \/ AsyncCandidateConditionalResponsiveTransportResidualAfter(
                   candidate)
             \/ AsyncCandidateVolatileBodyReconstructionResidualAfter(
                  candidate)
         /\ ~( /\ AsyncCandidateConditionalResponsiveTransportResidualAfter(
                       candidate)
                 /\ AsyncCandidateVolatileBodyReconstructionResidualAfter(
                      candidate))
BY Isa
   DEF AsyncCandidateProducerTransportResidualAfter,
       AsyncCandidateConditionalResponsiveTransportResidualAfter,
       AsyncCandidateVolatileBodyReconstructionResidualAfter,
       AsyncCandidateProducerContinuationExternalResidualKinds,
       AsyncCandidateProducerContinuationConditionalResponsiveTransportKinds,
       AsyncCandidateProducerContinuationVolatileBodyReconstructionKinds

THEOREM AsyncCandidateLifecycleDeparturesThisStepIsSingleton ==
  /\ AsyncLogicalCandidateOwnershipInvariant
  /\ AsyncNext
  => Cardinality(AsyncCandidateLifecycleDeparturesThisStep) <= 1
BY FS_Singleton, FS_Subset, FS_Union, IsaT(900)
   DEF AsyncCandidateLifecycleDeparturesThisStep,
       AsyncCandidateServicesThisStep,
       AsyncCandidateSemanticallyAppliedThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncCandidateIgnoredWithoutApplicationThisStepSet,
       AsyncCandidateIgnoredWithoutApplicationThisStep,
       AsyncCandidatePhysicallyDiscardedThisStep,
       AsyncLogicalCandidateOwnershipInvariant,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       EnqueueCandidate,
       AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       CandidateScheduled, CandidateScheduledAfter,
       CandidateScheduledIn

THEOREM AsyncCandidateIgnoredExactProtocolDepartureIsContinuationSourceOrGoal ==
  \A candidate \in AsyncCandidateSet:
    /\ candidate \in AsyncCandidateIgnoredWithoutApplicationThisStepSet
    /\ candidate.kind \in AsyncCandidateServiceTrackedKinds
    /\ candidate.subject \in Subjects
    /\ candidate.subject
         = AsyncProposalSubject(
             Leader(candidate.consumerContext, candidate.view))
      => \/ AsyncCandidateProducerContinuationGoalAfter(candidate)
         \/ AsyncCandidateProducerContinuationSourceAfter(candidate)
         \/ AsyncCandidateProducerTransportResidualAfter(candidate)
BY AsyncCandidateProducerContinuationDepartureSplitsSourceOrGoal, Isa
   DEF AsyncCandidateProducerContinuationDeparture,
       AsyncCandidateLifecycleDeparturesThisStep

THEOREM AsyncCandidateSuccessfulExactProtocolServiceIsContinuationSourceOrGoal ==
  \A candidate \in AsyncCandidateSet:
    /\ candidate \in AsyncCandidateServicesThisStep
    /\ candidate.subject \in Subjects
    /\ candidate.subject
         = AsyncProposalSubject(
             Leader(candidate.consumerContext, candidate.view))
      => \/ AsyncCandidateProducerContinuationGoalAfter(candidate)
         \/ AsyncCandidateProducerContinuationSourceAfter(candidate)
         \/ AsyncCandidateProducerTransportResidualAfter(candidate)
BY AsyncCandidateProducerContinuationDepartureSplitsSourceOrGoal, Isa
   DEF AsyncCandidateProducerContinuationDeparture,
       AsyncCandidateLifecycleDeparturesThisStep,
       AsyncCandidateServicesThisStep

THEOREM AsyncCandidateProducerContinuationStateInstallsExactSourceRecord ==
  \A state, candidate:
    /\ AsyncCandidateProducerContinuationSourceAfter(candidate)
    /\ AsyncCandidateLifecycleRecordedIn(
         state, candidate.node, candidate.causalOrigin)
    /\ AsyncCandidateProducerContinuationRecordsForIdentityIn(
         state, AsyncCandidateServiceIdentity(candidate)) = {}
    /\ AsyncCandidateProducerContinuationReservationAvailableIn(
         state, candidate)
    => LET lifecycle ==
             AsyncCandidateProducerContinuationLifecycleRecordIn(
               state, candidate)
           installed ==
             AsyncCandidateProducerContinuationRecord(
               candidate,
               AsyncCandidateProducerContinuationHandoffCandidatesThisStep(
                 candidate),
               lifecycle.slot, lifecycle.ordinal,
               AsyncCandidateProducerContinuationInitialStatusAfter(
                 candidate))
           next ==
             AsyncCandidateProducerContinuationStateAfterDeparture(
               state, candidate)
       IN /\ installed \in next.producerContinuations
          /\ installed.identity =
               AsyncCandidateServiceIdentity(candidate)
          /\ installed.causalOrigin = candidate.causalOrigin
         /\ installed.phase = candidate.kind
         /\ installed.sourceClass =
              AsyncCandidateProducerContinuationSourceClass(candidate)
         /\ installed.ordinal = lifecycle.ordinal
          /\ installed.status = "Reserved"
          /\ AsyncCandidateProducerContinuationActiveForIdentityIn(
               next, AsyncCandidateServiceIdentity(candidate))
BY Isa
   DEF AsyncCandidateProducerContinuationStateAfterDeparture,
       AsyncCandidateProducerContinuationLifecycleRecordIn,
       AsyncCandidateProducerContinuationAddressForIn,
       AsyncCandidateProducerContinuationOrdinalForIn,
       AsyncCandidateProducerContinuationReservationAvailableIn,
       AsyncCandidateProducerContinuationAddressCanAdvanceIn,
       AsyncCandidateProducerContinuationInitialStatusAfter,
       AsyncCandidateProducerContinuationHandoffCandidatesThisStep,
       AsyncCandidateProducerContinuationRecord,
       AsyncCandidateProducerContinuationActiveForIdentityIn,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationRecordsForAddressIn,
       AsyncCandidateProducerContinuationAddressOwnedIn,
       AsyncCandidateLifecycleRecordedIn

THEOREM AsyncCandidateProducerDepartureCreatesContinuationOrGoal ==
  \A state, candidate:
    /\ AsyncCandidateProducerContinuationDeparture(candidate)
    /\ AsyncCandidateLifecycleRecordedIn(
         state, candidate.node, candidate.causalOrigin)
    /\ AsyncCandidateProducerContinuationRecordsForIdentityIn(
         state, AsyncCandidateServiceIdentity(candidate)) = {}
    /\ AsyncCandidateProducerContinuationReservationAvailableIn(
         state, candidate)
    => \/ AsyncCandidateProducerContinuationGoalAfter(candidate)
       \/ AsyncCandidateProducerContinuationActiveForIdentityIn(
            AsyncCandidateProducerContinuationStateAfterDeparture(
              state, candidate),
            AsyncCandidateServiceIdentity(candidate))
BY AsyncCandidateProducerContinuationDepartureSplitsSourceOrGoal,
   AsyncCandidateProducerContinuationStateInstallsExactSourceRecord, Isa
   DEF AsyncCandidateProducerContinuationSourceAfter

THEOREM AsyncCandidateProducerSourceTransitionInstallsExactContinuation ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncLogicalCandidateOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncNext
    /\ AsyncCandidateProducerContinuationSourceAfter(candidate)
    /\ AsyncCandidateLifecycleRecorded(
         candidate.node, candidate.causalOrigin)
    /\ ~AsyncCandidateProducerContinuationRecorded(candidate)
    => AsyncCandidateProducerContinuationActiveForIdentity(
         AsyncCandidateServiceIdentity(candidate))'
BY AsyncCandidateLifecycleDeparturesThisStepIsSingleton,
   AsyncCandidateProducerContinuationStateInstallsExactSourceRecord,
   FS_Singleton, IsaT(1200)
   DEF AsyncNext, AsyncControlServiceSlotTransition,
       AsyncCandidateProducerContinuationSourceAfter,
       AsyncCandidateProducerContinuationRecorded,
       AsyncCandidateProducerContinuationActiveForIdentity,
       AsyncCandidateProducerContinuationActiveForIdentityIn,
       AsyncCandidateProducerContinuationRecordsFor,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationReservationAvailableAfterDepartureIn,
       AsyncCandidateProducerContinuationReservationAvailableIn,
       AsyncCandidateProducerContinuationAddressCanAdvanceIn,
       AsyncCandidateProducerContinuationAddressOwnedIn,
       AsyncCandidateProducerContinuationRecordsForAddressIn,
       AsyncCandidateProducerContinuationStateAfterDeparture,
       AsyncCandidateProducerContinuationLifecycleRecordIn,
       AsyncCandidateProducerContinuationAddressForIn,
       AsyncCandidateProducerContinuationOrdinalForIn,
       AsyncCandidateProducerContinuationInitialStatusAfter,
       AsyncCandidateProducerContinuationHandoffCandidatesThisStep,
       AsyncCandidateProducerContinuationRecord,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateServiceStateAfterSuccessfulService,
       AsyncCandidateServiceStateAfterTerminalRetirement,
       AsyncCandidateLifecycleStateAfterServiceSlotTransfer,
       AsyncCandidateLifecycleStateAfterCarrierUpdate,
       AsyncCandidateLifecycleStateAfterCompaction,
       AsyncCandidateLifecycleStateAfterServeIngressAdmission,
       AsyncCandidateLifecycleStateAfterAdmission,
       AsyncCandidateLifecycleStateAfterTimeoutOwnership,
       AsyncControlServiceStateAfterReset,
       AsyncControlServiceStateAfterAdmission,
       AsyncControlServiceStateAfterService,
       AsyncCertifiedResponseClaimStateAfterRetirement,
       AsyncCertifiedResponseClaimStateAfterAdmission,
       AsyncCandidateLifecycleRecorded,
       AsyncCandidateLifecycleRecordedIn,
       AsyncCandidateLifecycleRecordsFor,
       AsyncCandidateLifecycleRecordsForIn,
       AsyncCandidateServiceLifecycleInvariant

THEOREM AsyncCandidateProducerContinuationTerminalRecordIsFixed ==
  \A state,
     record \in AsyncCandidateProducerContinuationRecordSet:
    record.status = "Terminal"
      => AsyncCandidateProducerContinuationRecordAfterStep(
           state, record) = record
BY SMT DEF AsyncCandidateProducerContinuationRecordAfterStep

THEOREM AsyncCandidateProducerContinuationStatusIsMonotone ==
  \A state,
     record \in AsyncCandidateProducerContinuationRecordSet:
    AsyncCandidateProducerContinuationStatusRank(
      (AsyncCandidateProducerContinuationRecordAfterStep(
         state, record)).status)
      <= AsyncCandidateProducerContinuationStatusRank(record.status)
BY SMT
   DEF AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuationStatusRank

THEOREM AsyncCandidateProducerContinuationResolvedReservedRankStrictlyDrops ==
  \A state,
     record \in AsyncCandidateProducerContinuationRecordSet:
    /\ AsyncCandidateProducerContinuationSelectedForResolution(record)
    /\ record.status = "Reserved"
    /\ \/ AsyncCandidateProducerContinuationConcreteSuccessorOwnedAfterIn(
             state, record)
       \/ AsyncCandidateProducerContinuationHandoffRetiredAfterIn(
            state, record)
      => AsyncCandidateProducerContinuationStatusRank(
           (AsyncCandidateProducerContinuationRecordAfterStep(
              state, record)).status)
           < AsyncCandidateProducerContinuationStatusRank(record.status)
BY AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor,
   SMT
   DEF AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuationConcreteSuccessorOwnedAfterIn,
       AsyncCandidateProducerContinuationHandoffOwnedAfterIn,
       AsyncCandidateProducerContinuationHandoffRetiredAfterIn,
       AsyncCandidateProducerContinuationStatusRank

THEOREM AsyncCandidateProducerContinuationMaterializedIsOneStep ==
  \A state,
     record \in AsyncCandidateProducerContinuationRecordSet:
    /\ AsyncCandidateProducerContinuationSelectedForResolution(record)
    /\ record.status = "Materialized"
      => (AsyncCandidateProducerContinuationRecordAfterStep(
            state, record)).status = "Terminal"
BY SMT DEF AsyncCandidateProducerContinuationRecordAfterStep

THEOREM AsyncCandidateProducerContinuationUnselectedActiveRecordIsFixed ==
  \A state,
     record \in AsyncCandidateProducerContinuationRecordSet:
    /\ record.status \in {"Reserved", "Materialized"}
    /\ ~AsyncCandidateProducerContinuationTerminalAfter(record)
    /\ ~AsyncCandidateProducerContinuationSelectedForResolution(record)
    /\ ~AsyncCandidateProducerContinuationSelectedForAcknowledgement(record)
      => AsyncCandidateProducerContinuationRecordAfterStep(
           state, record) = record
BY SMT
   DEF AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuationSelectedForAcknowledgement

THEOREM ResolveCandidateProducerContinuationNeverReplaysDrainedParent ==
  \A node \in ValidatorIds:
    ResolveCandidateProducerContinuation(node)
      => /\ asyncCausalQueues' = asyncCausalQueues
         /\ vars' = vars
BY DEF ResolveCandidateProducerContinuation

THEOREM CandidateProducerContinuationBlocksRunnerUntilHandoffResolution ==
  \A node \in ValidatorIds:
    RunNodeWork(node)
      => \/ ~AsyncCandidateProducerContinuationResolutionRequired(node)
         \/ ResolveRunNodeCandidateProducerContinuation(node)
         \/ ReplayRunNodeCandidateProducerContinuation(node)
BY DEF RunNodeWork

THEOREM CandidateProducerContinuationResolutionSelectsMinimumFrozenOwner ==
  \A node \in ValidatorIds:
    AsyncCandidateProducerContinuationResolutionRequired(node)
      => LET selected ==
               AsyncCandidateProducerContinuationSelectedResolutionRecord(
                 node)
         IN /\ selected
                  \in
                    AsyncCandidateProducerContinuationResolutionRecordsForNode(
                      node)
            /\ AsyncCandidateProducerContinuationResolutionPredecessorsFor(
                 node, selected) = {}
BY Isa
   DEF AsyncCandidateProducerContinuationResolutionRequired,
       AsyncCandidateProducerContinuationSelectedResolutionRecord

THEOREM ExternalCandidateProducerContinuationSelectionIsReady ==
  \A node \in ValidatorIds:
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationResolutionRequired(node)
    /\ (AsyncCandidateProducerContinuationSelectedResolutionRecord(node))
         .sourceClass \in {"ConditionalTransport", "VolatileBody"}
      => AsyncCandidateProducerContinuationResolutionReady(node)
BY Isa
   DEF AsyncCandidateProducerContinuationExternalCoverageInvariant,
       AsyncCandidateProducerContinuationResolutionReady,
       AsyncCandidateProducerContinuationResolutionRequired,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationDurableTerminal

AsyncCandidateProducerContinuationDescendsOrReplayExitsAfter(record) ==
  \/ AsyncCandidateProducerContinuationRecordsForIdentityIn(
       asyncControlServiceState', record.identity) = {}
  \/ \E nextRecord \in
       AsyncCandidateProducerContinuationRecordsForIdentityIn(
         asyncControlServiceState', record.identity):
       AsyncCandidateProducerContinuationStatusRank(nextRecord.status)
         < AsyncCandidateProducerContinuationStatusRank(record.status)
  \/ /\ record.status = "Materialized"
     /\ (PreGstResponsiveRestart \/ PreGstResponsiveReplay)
     /\ \E nextRecord \in
          AsyncCandidateProducerContinuationRecordsForIdentityIn(
            asyncControlServiceState', record.identity):
          /\ nextRecord.status = "Reserved"
          /\ AsyncCandidateProducerSemanticHandoffReservationToken(nextRecord)
               = AsyncCandidateProducerSemanticHandoffReservationToken(record)

THEOREM AsyncCandidateProducerContinuationGstExcludesResetReplay ==
  gst => ~(PreGstResponsiveRestart \/ PreGstResponsiveReplay)
BY DEF PreGstResponsiveRestart, PreGstResponsiveReplay

THEOREM ConditionalTransportContinuationReadyEnablesFairService ==
  \A initialContext,
     node \in AsyncVotersAt(initialContext):
    /\ gst
    /\ AsyncCandidateProducerContinuationResolutionReady(node)
    /\ AsyncCandidateProducerContinuationSelectedSourceClass(
         node, "ConditionalTransport")
      => ENABLED
           <<PostGstServiceConditionalTransportProducerContinuation(
               node)>>_AsyncAllVars
BY ExpandENABLED, IsaT(300)
   DEF PostGstServiceConditionalTransportProducerContinuation,
       ServiceConditionalTransportProducerContinuation,
       ResolveCandidateProducerContinuation,
       AsyncCandidateProducerContinuationResolutionReady,
       AsyncCandidateProducerContinuationSelectedSourceClass,
       AsyncNonRunnerOuterFrame, AsyncAllVars,
       AsyncSchedulerVars, AsyncLocalAdmissionVars,
       AsyncDeferredVars, vars

THEOREM VolatileBodyContinuationReadyEnablesFairService ==
  \A initialContext,
     node \in AsyncVotersAt(initialContext):
    /\ gst
    /\ AsyncCandidateProducerContinuationResolutionReady(node)
    /\ AsyncCandidateProducerContinuationSelectedSourceClass(
         node, "VolatileBody")
      => ENABLED
           <<PostGstServiceVolatileBodyProducerContinuation(
               node)>>_AsyncAllVars
BY ExpandENABLED, IsaT(300)
   DEF PostGstServiceVolatileBodyProducerContinuation,
       ServiceVolatileBodyProducerContinuation,
       ResolveCandidateProducerContinuation,
       AsyncCandidateProducerContinuationResolutionReady,
       AsyncCandidateProducerContinuationSelectedSourceClass,
       AsyncNonRunnerOuterFrame, AsyncAllVars,
       AsyncSchedulerVars, AsyncLocalAdmissionVars,
       AsyncDeferredVars, vars

THEOREM LocalContinuationReadyEnablesFairResolution ==
  \A initialContext,
     node \in AsyncVotersAt(initialContext):
    /\ gst
    /\ AsyncCandidateProducerContinuationResolutionReady(node)
    /\ AsyncCandidateProducerContinuationSelectedSourceClass(node, "Local")
      => ENABLED
           <<PostGstResolveLocalCandidateProducerContinuation(
               node)>>_AsyncAllVars
BY ExpandENABLED, IsaT(300)
   DEF PostGstResolveLocalCandidateProducerContinuation,
       ResolveLocalCandidateProducerContinuation,
       ResolveCandidateProducerContinuation,
       AsyncCandidateProducerContinuationResolutionReady,
       AsyncCandidateProducerContinuationSelectedSourceClass,
       AsyncNonRunnerOuterFrame, AsyncAllVars,
       AsyncSchedulerVars, AsyncLocalAdmissionVars,
       AsyncDeferredVars, vars

THEOREM ExternalContinuationFairServiceStrictlyDropsStatusRank ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet:
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    /\ record =
         AsyncCandidateProducerContinuationSelectedResolutionRecord(node)
    /\ record.status \in {"Reserved", "Materialized"}
    /\ \/ /\ record.sourceClass = "ConditionalTransport"
           /\ PostGstServiceConditionalTransportProducerContinuation(node)
       \/ /\ record.sourceClass = "VolatileBody"
           /\ PostGstServiceVolatileBodyProducerContinuation(node)
      => AsyncCandidateProducerContinuationDescendsOrReplayExitsAfter(record)
BY AsyncCandidateProducerContinuationResolvedReservedRankStrictlyDrops,
   AsyncCandidateProducerContinuationMaterializedIsOneStep,
   IsaT(600)
   DEF AsyncCandidateProducerContinuationDescendsOrReplayExitsAfter,
       PostGstServiceConditionalTransportProducerContinuation,
       PostGstServiceVolatileBodyProducerContinuation,
       ServiceConditionalTransportProducerContinuation,
       ServiceVolatileBodyProducerContinuation,
       ResolveCandidateProducerContinuation,
       AsyncCandidateProducerContinuationSelectedSourceClass,
       AsyncCandidateProducerContinuationSelectedForResolution,
       AsyncCandidateProducerContinuationResolutionReady,
       AsyncCandidateProducerContinuationHandoffOwned,
       AsyncCandidateProducerContinuationHandoffRetired,
       AsyncCandidateProducerContinuationHandoffOwnedAfterIn,
       AsyncCandidateProducerContinuationHandoffRetiredAfterIn,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncControlServiceSlotTransition,
       AsyncCandidateProducerContinuationRecordsForIdentityIn

THEOREM LocalContinuationFairResolutionStrictlyDropsStatusRank ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet:
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    /\ record =
         AsyncCandidateProducerContinuationSelectedResolutionRecord(node)
    /\ record.status \in {"Reserved", "Materialized"}
    /\ record.sourceClass = "Local"
    /\ PostGstResolveLocalCandidateProducerContinuation(node)
      => AsyncCandidateProducerContinuationDescendsOrReplayExitsAfter(record)
BY AsyncCandidateProducerContinuationResolvedReservedRankStrictlyDrops,
   AsyncCandidateProducerContinuationMaterializedIsOneStep,
   IsaT(600)
   DEF AsyncCandidateProducerContinuationDescendsOrReplayExitsAfter,
       PostGstResolveLocalCandidateProducerContinuation,
       ResolveLocalCandidateProducerContinuation,
       ResolveCandidateProducerContinuation,
       AsyncCandidateProducerContinuationSelectedSourceClass,
       AsyncCandidateProducerContinuationSelectedForResolution,
       AsyncCandidateProducerContinuationResolutionReady,
       AsyncCandidateProducerContinuationHandoffOwned,
       AsyncCandidateProducerContinuationHandoffRetired,
       AsyncCandidateProducerContinuationHandoffOwnedAfterIn,
       AsyncCandidateProducerContinuationHandoffRetiredAfterIn,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncControlServiceSlotTransition,
       AsyncCandidateProducerContinuationRecordsForIdentityIn

THEOREM ExternalContinuationPersistsOrDescendsOrReplayExits ==
  \A record \in AsyncCandidateProducerContinuationRecordSet:
    /\ AsyncControlServiceStateTypeInvariant
    /\ record \in AsyncCandidateProducerContinuations
    /\ record.status \in {"Reserved", "Materialized"}
    /\ record.sourceClass \in {"ConditionalTransport", "VolatileBody"}
    /\ AsyncNext
      => \/ record \in AsyncCandidateProducerContinuations'
         \/ AsyncCandidateProducerContinuationDescendsOrReplayExitsAfter(record)
BY AsyncCandidateProducerContinuationResetPreservesExactReservation,
   IsaT(1200)
   DEF AsyncCandidateProducerContinuationDescendsOrReplayExitsAfter,
       AsyncCandidateProducerSemanticHandoffReservationToken,
       AsyncCandidateProducerContinuations,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateProducerContinuationsAfterReset,
       AsyncCandidateProducerContinuationRecordAfterReset,
       AsyncCandidateProducerContinuationRestartStableTerminalIn,
       AsyncControlServiceSlotTransition,
       AsyncControlServiceStateAfterReset,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       PreGstResponsiveRestart, PreGstResponsiveReplay

THEOREM LocalContinuationPersistsOrDescendsOrReplayExits ==
  \A record \in AsyncCandidateProducerContinuationRecordSet:
    /\ AsyncControlServiceStateTypeInvariant
    /\ record \in AsyncCandidateProducerContinuations
    /\ record.status \in {"Reserved", "Materialized"}
    /\ record.sourceClass = "Local"
    /\ AsyncNext
      => \/ record \in AsyncCandidateProducerContinuations'
         \/ AsyncCandidateProducerContinuationDescendsOrReplayExitsAfter(record)
BY AsyncCandidateProducerContinuationResetPreservesExactReservation,
   IsaT(1200)
   DEF AsyncCandidateProducerContinuationDescendsOrReplayExitsAfter,
       AsyncCandidateProducerSemanticHandoffReservationToken,
       AsyncCandidateProducerContinuations,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateProducerContinuationsAfterReset,
       AsyncCandidateProducerContinuationRecordAfterReset,
       AsyncCandidateProducerContinuationRestartStableTerminalIn,
       AsyncControlServiceSlotTransition,
       AsyncControlServiceStateAfterReset,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       PreGstResponsiveRestart, PreGstResponsiveReplay

THEOREM AsyncInitEstablishesCandidateProducerContinuationExternalCoverage ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => AsyncCandidateProducerContinuationExternalCoverageInvariant
BY Isa
   DEF AsyncInitAt, AsyncBaseInitAt, AsyncTransportInit,
       AsyncCandidateProducerContinuationExternalCoverageInvariant,
       AsyncCandidateProducerContinuations

THEOREM AsyncNextPreservesCandidateProducerContinuationExternalCoverage ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncProgressOwnershipInvariant
  /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
  /\ AsyncNext
  => AsyncCandidateProducerContinuationExternalCoverageInvariant'
BY AsyncCandidateProducerContinuationResetPreservesExactReservation,
   AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck,
   AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor,
   AsyncCandidateProducerSemanticHandoffRetirementRequiresAck,
   IsaT(2400)
   DEF AsyncCandidateProducerContinuationExternalCoverageInvariant,
       AsyncCandidateProducerContinuationConcreteSuccessorOwned,
       AsyncCandidateProducerContinuationHandoffOwned,
       AsyncCandidateProducerContinuationHandoffRetired,
       AsyncCandidateProducerContinuationConditionalResponsiveTransportKinds,
       AsyncCandidateProducerContinuationVolatileBodyReconstructionKinds,
       AsyncCandidateConditionalTransportCarrier,
       AsyncCandidateConditionalTransportRetired,
       AsyncCandidateVolatileBodyCarrier,
       AsyncCandidateVolatileBodyRetired,
       AsyncCandidateProducerContinuationDeclaredHandoffOwned,
       AsyncCandidateProducerContinuationDeclaredHandoffRetired,
       AsyncCandidateProducerContinuationLocalReplayCarrier,
       AsyncCandidateProducerContinuationDurableTerminal,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuationSelectedForAcknowledgement,
       AsyncCandidateProducerContinuations,
       AsyncCandidateProducerContinuationsAfterReset,
       AsyncCandidateProducerContinuationRecordAfterReset,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateProducerContinuationStateAfterDeparture,
       AsyncCandidateProducerContinuationInitialStatusAfter,
       AsyncControlServiceSlotTransition,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       EnqueueCandidate,
       AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       RuntimeStep, FifoRuntimeStep, DeferredDrainStep,
       AdmitIngressPacket, AdmitHiddenPacket,
       CoalesceHiddenPacket, DropPolicyRejectedHiddenPacket,
       ServiceIoWorkerWork,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart, FreshRestartCandidateSequence

THEOREM AsyncSpecAlwaysCandidateProducerContinuationExternalCoverage ==
  \A initialContext:
    AsyncSpecAt(initialContext)
      => []AsyncCandidateProducerContinuationExternalCoverageInvariant
PROOF
  <1>1. ASSUME NEW initialContext,
                AsyncSpecAt(initialContext)
         PROVE []AsyncCandidateProducerContinuationExternalCoverageInvariant
    <2>1. AsyncInitAt(initialContext)
             => AsyncCandidateProducerContinuationExternalCoverageInvariant
      BY AsyncInitEstablishesCandidateProducerContinuationExternalCoverage
    <2>2. []AsyncStrongTypeInvariant
      BY <1>1, AsyncSpecAlwaysStrongTypeInvariant
    <2>3. []AsyncProgressOwnershipInvariant
      BY <1>1, AsyncSpecAlwaysProgressOwnershipInvariant
    <2>4. /\ AsyncStrongTypeInvariant
           /\ AsyncProgressOwnershipInvariant
           /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
           /\ [AsyncNext]_AsyncAllVars
          => AsyncCandidateProducerContinuationExternalCoverageInvariant'
      BY AsyncNextPreservesCandidateProducerContinuationExternalCoverage,
         Isa DEF AsyncAllVars
    <2> QED BY <1>1, <2>1, <2>2, <2>3, <2>4, PTL
         DEF AsyncSpecAt
  <1> QED BY <1>1

(***************************************************************************
Frozen producer-continuation episode rank.

Service-stage order is not a causal order.  In particular, PersistInstallTC
can emit an AssembleBody successor at the same immutable lifecycle ordinal,
moving from the TimeoutCertificateReceived adapter stage to the numerically
lower LocalProposalReady stage.  The frozen set is therefore the exact set of
causal origins whose inherited admission ordinal is at or before the target.
Every currently scheduled member of those lineages is charged by the closed
topological command weight; every active continuation is charged by its
two-step status rank.  The existing Serve predecessor/capacity/selector rank
is retained as the lower pair.

The augmented command weight leaves two credits for the atomic transfer from
a drained tracked candidate into Reserved.  Radix eight is sufficient for
the exact three-child command graph, so a same-ordinal lower-stage successor
consumes the frozen episode instead of replenishing it.  Later lifecycle,
causal, Control, Completion, priority, and retry work receives a larger
shared ordinal and cannot enter the frozen predecessor-origin set.
***************************************************************************)
AsyncCandidateProducerContinuationFrozenPredecessorOrigins(
    node, targetOrdinal) ==
  AsyncCausalEpisodeFrozenPredecessorOrigins(node, targetOrdinal)

AsyncCandidateProducerContinuationCausalWeight(kind) ==
  8 * AsyncCausalRemainingWorkWeight(kind) + 2

AsyncCandidateProducerContinuationSuccessorBatchWeight(command) ==
  LET successors == CommandSuccessors(command)
  IN CASE Len(successors) = 0 -> 0
       [] Len(successors) = 1 ->
            AsyncCandidateProducerContinuationCausalWeight(
              successors[1].kind)
       [] Len(successors) = 2 ->
            AsyncCandidateProducerContinuationCausalWeight(
              successors[1].kind)
              + AsyncCandidateProducerContinuationCausalWeight(
                  successors[2].kind)
       [] Len(successors) = 3 ->
            AsyncCandidateProducerContinuationCausalWeight(
              successors[1].kind)
              + AsyncCandidateProducerContinuationCausalWeight(
                  successors[2].kind)
              + AsyncCandidateProducerContinuationCausalWeight(
                  successors[3].kind)
       [] OTHER ->
            AsyncCandidateProducerContinuationCausalWeight(command.kind)

THEOREM CandidateProducerContinuationSuccessorBatchConsumesFrozenWeight ==
  \A command \in AsyncCandidateSet:
    AsyncCandidateProducerContinuationSuccessorBatchWeight(command)
      < AsyncCandidateProducerContinuationCausalWeight(command.kind)
BY CommandSuccessorsHaveBoundedLength,
   AsyncCommandSuccessorsStrictlyLowerRemainingWorkStage,
   AsyncCommandSuccessorBatchStrictlyConsumesRemainingWork,
   SMTT(180)
   DEF AsyncCandidateProducerContinuationSuccessorBatchWeight,
       AsyncCandidateProducerContinuationCausalWeight,
       AsyncCausalRemainingWorkWeight,
       AsyncCausalRemainingWorkStage,
       CommandSuccessors, SequenceSet,
       AsyncCandidateSet, AsyncCandidateTyped,
       AsyncWorkKinds, AsyncCompletionTags,
       AsyncDeliveryKinds, AsyncReducerKinds

AsyncCandidateProducerContinuationFrozenCandidateTokens(
    node, targetOrdinal) ==
  {<<"Candidate", candidate, token>>:
     candidate \in AsyncCausalEpisodeCandidates(node, targetOrdinal),
     token \in
       1..AsyncCandidateProducerContinuationCausalWeight(candidate.kind)}

AsyncCandidateProducerContinuationFrozenRecords(
    node, targetOrdinal) ==
  {record \in
     AsyncCandidateProducerContinuationResolutionRecordsForNode(node):
     /\ record.ordinal <= targetOrdinal
     /\ record.causalOrigin
          \in AsyncCandidateProducerContinuationFrozenPredecessorOrigins(
               node, targetOrdinal)}

AsyncCandidateProducerContinuationFrozenStatusTokens(
    node, targetOrdinal) ==
  {<<"Continuation", record.identity, token>>:
     record \in
       AsyncCandidateProducerContinuationFrozenRecords(
         node, targetOrdinal),
     token \in
       1..AsyncCandidateProducerContinuationStatusRank(record.status)}

AsyncCandidateProducerContinuationFrozenProducerTokens(
    node, targetOrdinal) ==
  AsyncCandidateProducerContinuationFrozenCandidateTokens(
    node, targetOrdinal)
    \cup AsyncCandidateProducerContinuationFrozenStatusTokens(
           node, targetOrdinal)

AsyncCandidateProducerContinuationFrozenProducerBudget(
    node, targetOrdinal) ==
  Cardinality(
    AsyncCandidateProducerContinuationFrozenProducerTokens(
      node, targetOrdinal))

AsyncCandidateProducerContinuationFrozenPrefixRank(
    node, targetOrdinal) ==
  <<AsyncCandidateProducerContinuationFrozenProducerBudget(
       node, targetOrdinal),
    <<AsyncCausalEpisodeServeWorkBudget(node, targetOrdinal),
      AsyncCausalEpisodeServeReachDebt(node, targetOrdinal)>>>

AsyncCandidateProducerContinuationFrozenPrefixRankCarrier ==
  AsyncCausalEpisodeStructuralRankCarrier

AsyncCandidateProducerContinuationFrozenPrefixRankOrdering ==
  AsyncCausalEpisodeStructuralRankOrdering

THEOREM CandidateProducerContinuationFrozenPrefixRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
    AsyncCandidateProducerContinuationFrozenPrefixRankCarrier)
BY AsyncCausalEpisodeStructuralRankOrderingIsWellFounded
   DEF AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier

AsyncCandidateProducerContinuationTargetAtStatus(
    node, identity, targetOrdinal, targetStage, status) ==
  \E record \in
       AsyncCandidateProducerContinuationResolutionRecordsForNode(node):
    /\ record.identity = identity
    /\ record.ordinal = targetOrdinal
    /\ record.address.stage = targetStage
    /\ record.status = status

AsyncCandidateProducerContinuationTargetStatusExit(identity, status) ==
  \/ AsyncCandidateProducerContinuationRecordsForIdentity(identity) = {}
  \/ \E record \in
       AsyncCandidateProducerContinuationRecordsForIdentity(identity):
       AsyncCandidateProducerContinuationStatusRank(record.status)
         < AsyncCandidateProducerContinuationStatusRank(status)

HistoricalCandidateProducerContinuationAtStatus(node, record, status) ==
  /\ gst
  /\ HistoricalRecoveryTarget(node)
  /\ status \in {"Reserved", "Materialized"}
  /\ record.status = status
  /\ record
       \in AsyncCandidateProducerContinuationResolutionRecordsForNode(node)

HistoricalCandidateProducerContinuationSelectedAtStatus(
    node, record, status) ==
  /\ HistoricalCandidateProducerContinuationAtStatus(
       node, record, status)
  /\ record =
       AsyncCandidateProducerContinuationSelectedResolutionRecord(node)

THEOREM HistoricalCandidateProducerContinuationTurnIsResolutionOrExactReplay ==
  \A node \in ValidatorIds:
    /\ AsyncCandidateProducerContinuationResolutionRequired(node)
    /\ RunHistoricalRecoveryNode(node)
      => \/ /\ AsyncCandidateProducerContinuationResolutionReady(node)
               /\ ResolveRunNodeCandidateProducerContinuation(node)
               /\ UNCHANGED vars
               /\ UNCHANGED asyncCausalQueues
               /\ UNCHANGED
                    AsyncSchedulerExceptCausalControlAndNodeService
         \/ /\ ~AsyncCandidateProducerContinuationResolutionReady(node)
               /\ ReplayRunNodeCandidateProducerContinuation(node)
BY DEF RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation

THEOREM HistoricalCandidateProducerContinuationNonreadyTurnUsesLocalReplay ==
  \A node \in ValidatorIds:
    /\ AsyncCandidateProducerContinuationResolutionRequired(node)
    /\ ~AsyncCandidateProducerContinuationResolutionReady(node)
    /\ RunHistoricalRecoveryNode(node)
      => /\ (AsyncCandidateProducerContinuationSelectedResolutionRecord(node))
               .sourceClass = "Local"
         /\ ReplayRunNodeCandidateProducerContinuation(node)
BY HistoricalCandidateProducerContinuationTurnIsResolutionOrExactReplay,
   Isa
   DEF ReplayRunNodeCandidateProducerContinuation

THEOREM HistoricalCandidateProducerContinuationLocalReplayTurnApproachesReady ==
  \A node \in ValidatorIds:
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    /\ AsyncCandidateProducerContinuationResolutionRequired(node)
    /\ ~AsyncCandidateProducerContinuationResolutionReady(node)
    /\ RunHistoricalRecoveryNode(node)
      => \/ (AsyncCandidateProducerContinuationResolutionReady(node))'
         \/ /\ asyncRunnerPhase[node] \in {"Ingress", "Runtime"}
               /\ asyncRunnerPhase'[node] = "Local"
BY HistoricalCandidateProducerContinuationNonreadyTurnUsesLocalReplay,
   AsyncCandidateProducerContinuationExactLocalReplayRetainsReservation,
   AsyncCandidateProducerContinuationStoredCarrierMakesSelectedRecordReady,
   IsaT(1200)
   DEF ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncCandidateProducerContinuationRuntimeReplayCarrier,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationResolutionRequired,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationSelectedResolutionRecord,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuations,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncControlServiceSlotTransition

THEOREM HistoricalCandidateProducerContinuationReadyTurnConsumesExactStage ==
  \A state,
     record \in AsyncCandidateProducerContinuationRecordSet:
    /\ record.status \in {"Reserved", "Materialized"}
    /\ AsyncCandidateProducerContinuationSelectedForRunnerResolution(
         record)
      => \/ /\ record.status = "Materialized"
               /\ (AsyncCandidateProducerContinuationRecordAfterStep(
                     state, record)).status = "Terminal"
         \/ /\ record.status = "Reserved"
               /\ AsyncCandidateProducerContinuationHandoffRetiredAfterIn(
                    state, record)
               /\ (AsyncCandidateProducerContinuationRecordAfterStep(
                     state, record)).status = "Terminal"
         \/ /\ record.status = "Reserved"
               /\ AsyncCandidateProducerContinuationConcreteSuccessorOwnedAfterIn(
                    state, record)
               /\ (AsyncCandidateProducerContinuationRecordAfterStep(
                     state, record)).status = "Materialized"
BY AsyncCandidateProducerContinuationRunnerResolutionConsumesExactStage

THEOREM HistoricalCandidateProducerContinuationReadyTurnExitsSelectedStatus ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet,
     status \in {"Reserved", "Materialized"}:
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    /\ HistoricalCandidateProducerContinuationSelectedAtStatus(
         node, record, status)
    /\ AsyncCandidateProducerContinuationResolutionReady(node)
    /\ PostGstRunHistoricalRecoveryNode(node)
      => AsyncCandidateProducerContinuationTargetStatusExit(
           record.identity, status)'
BY HistoricalCandidateProducerContinuationTurnIsResolutionOrExactReplay,
   HistoricalCandidateProducerContinuationReadyTurnConsumesExactStage,
   IsaT(900)
   DEF HistoricalCandidateProducerContinuationSelectedAtStatus,
       HistoricalCandidateProducerContinuationAtStatus,
       AsyncCandidateProducerContinuationSelectedForRunnerResolution,
       AsyncCandidateProducerContinuationTargetStatusExit,
       AsyncCandidateProducerContinuationStatusRank,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncControlServiceSlotTransition,
       PostGstRunHistoricalRecoveryNode

AsyncCandidateProducerContinuationFrozenPrefixAtBudget(
    node, identity, targetOrdinal, targetStage, status, budget) ==
  /\ gst
  /\ node \in
       AsyncCurrentResponsiveVoters
         \cup asyncHistoricalRecoveryTargets
  /\ status \in {"Reserved", "Materialized"}
  /\ AsyncCandidateProducerContinuationTargetAtStatus(
       node, identity, targetOrdinal, targetStage, status)
  /\ budget =
       AsyncCandidateProducerContinuationFrozenPrefixRank(
         node, targetOrdinal)

AsyncCandidateProducerContinuationPrefixDescentGoal(
    node, identity, targetOrdinal, targetStage, status, budget) ==
  \/ AsyncCandidateProducerContinuationTargetStatusExit(identity, status)
  \/ \E lower \in
       SetLessThan(
         budget,
         AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
         AsyncCandidateProducerContinuationFrozenPrefixRankCarrier):
       AsyncCandidateProducerContinuationFrozenPrefixAtBudget(
         node, identity, targetOrdinal, targetStage, status, lower)

THEOREM CandidateProducerContinuationFrozenPrefixRankIsFiniteAndPositive ==
  \A node \in ValidatorIds,
     identity,
     targetOrdinal \in Nat \ {0},
     targetStage \in AsyncCandidateServiceStageClasses,
     status \in {"Reserved", "Materialized"},
     budget:
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncCandidateProducerContinuationFrozenPrefixAtBudget(
         node, identity, targetOrdinal, targetStage, status, budget)
      => /\ budget
               \in AsyncCandidateProducerContinuationFrozenPrefixRankCarrier
         /\ budget[1] \in Nat \ {0}
BY AsyncCausalRemainingWorkWeightIsPositive,
   FS_Product, FS_Image, FS_Union, FS_Subset,
   FS_Interval, FS_CardinalityType, IsaT(900)
   DEF AsyncCandidateProducerContinuationFrozenPrefixAtBudget,
       AsyncCandidateProducerContinuationFrozenPrefixRank,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
       AsyncCandidateProducerContinuationFrozenProducerBudget,
       AsyncCandidateProducerContinuationFrozenProducerTokens,
       AsyncCandidateProducerContinuationFrozenCandidateTokens,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationCausalWeight,
       AsyncCandidateProducerContinuationTargetAtStatus,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCausalEpisodeCandidates,
       AsyncCausalEpisodeFrozenPredecessorOrigins,
       AsyncCausalEpisodeServeWorkBudget,
       AsyncCausalEpisodeServeWorkTokens,
       AsyncCausalEpisodeServeReachDebt,
       AsyncCausalEpisodeStructuralRankCarrier,
       AsyncCausalEpisodeServeRankCarrier,
       AsyncCandidateProducerContinuationStatusRank,
       AsyncControlServiceStateTypeInvariant,
       AsyncCandidateProducerContinuations

THEOREM CandidateProducerContinuationFrozenOriginsCannotReplenish ==
  \A node \in ValidatorIds,
     identity,
     targetOrdinal \in Nat \ {0},
     targetStage \in AsyncCandidateServiceStageClasses,
     status \in {"Reserved", "Materialized"},
     budget:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCandidateProducerContinuationFrozenPrefixAtBudget(
         node, identity, targetOrdinal, targetStage, status, budget)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~(AsyncCandidateProducerContinuationTargetStatusExit(
            identity, status))'
      => (AsyncCandidateProducerContinuationFrozenPredecessorOrigins(
             node, targetOrdinal))'
           \subseteq
             AsyncCandidateProducerContinuationFrozenPredecessorOrigins(
               node, targetOrdinal)
BY AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   AsyncCandidateScheduledIdentityDepartureRetiresLifecycleAtGst,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncCandidateProducerContinuationResetPreservesExactReservation,
   IsaT(1200)
   DEF AsyncCandidateProducerContinuationFrozenPrefixAtBudget,
       AsyncCandidateProducerContinuationTargetAtStatus,
       AsyncCandidateProducerContinuationTargetStatusExit,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCausalEpisodeFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncAllVars

THEOREM CandidateProducerContinuationFrozenPrefixStepCannotReplenish ==
  \A node \in ValidatorIds,
     identity,
     targetOrdinal \in Nat \ {0},
     targetStage \in AsyncCandidateServiceStageClasses,
     status \in {"Reserved", "Materialized"},
     budget \in
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCandidateProducerContinuationFrozenPrefixAtBudget(
         node, identity, targetOrdinal, targetStage, status, budget)
    /\ [AsyncNext]_AsyncAllVars
      => \/ AsyncCandidateProducerContinuationFrozenPrefixAtBudget(
               node, identity, targetOrdinal, targetStage, status, budget)'
         \/ (AsyncCandidateProducerContinuationPrefixDescentGoal(
               node, identity, targetOrdinal, targetStage,
               status, budget))'
BY CandidateProducerContinuationFrozenOriginsCannotReplenish,
   CandidateProducerContinuationSuccessorBatchConsumesFrozenWeight,
   ExternalContinuationPersistsOrDescendsOrReplayExits,
   LocalContinuationPersistsOrDescendsOrReplayExits,
   AsyncCandidateProducerContinuationGstExcludesResetReplay,
   AsyncCausalEpisodeServeCutCannotReplenish,
   AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck,
   AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor,
   AsyncCandidateProducerSemanticHandoffRetirementRequiresAck,
   IsaT(2400)
   DEF AsyncCandidateProducerContinuationFrozenPrefixAtBudget,
       AsyncCandidateProducerContinuationPrefixDescentGoal,
       AsyncCandidateProducerContinuationTargetStatusExit,
       AsyncCandidateProducerContinuationDescendsOrReplayExitsAfter,
       AsyncCandidateProducerContinuationFrozenPrefixRank,
       AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
       AsyncCandidateProducerContinuationFrozenProducerBudget,
       AsyncCandidateProducerContinuationFrozenProducerTokens,
       AsyncCandidateProducerContinuationFrozenCandidateTokens,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationCausalWeight,
       AsyncCandidateProducerContinuationTargetAtStatus,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuationStateAfterDeparture,
       AsyncCandidateProducerContinuationReservationAvailableIn,
       AsyncCandidateProducerContinuationAddressCanAdvanceIn,
       AsyncCandidateProducerContinuationRecordsForAddressIn,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationStatusRank,
       AsyncCausalEpisodeCandidates,
       AsyncCausalEpisodeFrozenPredecessorOrigins,
       AsyncCausalEpisodeServeWorkBudget,
       AsyncCausalEpisodeServeWorkTokens,
       AsyncCausalEpisodeServeReachDebt,
       AsyncCausalEpisodeStructuralRankOrdering,
       AsyncCausalEpisodeStructuralRankCarrier,
       AsyncCausalEpisodeServeRankOrdering,
       AsyncCausalEpisodeServeRankCarrier,
       SetLessThan, OpToRel

THEOREM CandidateProducerContinuationFairResolutionStrictlyDescendsFrozenPrefix ==
  \A node \in ValidatorIds,
     identity,
     targetOrdinal \in Nat \ {0},
     targetStage \in AsyncCandidateServiceStageClasses,
     status \in {"Reserved", "Materialized"},
     budget \in
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncControlServiceSlotTransition
    /\ AsyncCandidateProducerContinuationFrozenPrefixAtBudget(
         node, identity, targetOrdinal, targetStage, status, budget)
    /\ \/ PostGstResolveLocalCandidateProducerContinuation(node)
       \/ PostGstServiceConditionalTransportProducerContinuation(node)
       \/ PostGstServiceVolatileBodyProducerContinuation(node)
      => (AsyncCandidateProducerContinuationPrefixDescentGoal(
            node, identity, targetOrdinal, targetStage,
            status, budget))'
BY ExternalContinuationFairServiceStrictlyDropsStatusRank,
   LocalContinuationFairResolutionStrictlyDropsStatusRank,
   CandidateProducerContinuationResolutionSelectsMinimumFrozenOwner,
   FS_CardinalityType, IsaT(2400)
   DEF AsyncCandidateProducerContinuationFrozenPrefixAtBudget,
       AsyncCandidateProducerContinuationPrefixDescentGoal,
       AsyncCandidateProducerContinuationTargetStatusExit,
       AsyncCandidateProducerContinuationFrozenPrefixRank,
       AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
       AsyncCandidateProducerContinuationFrozenProducerBudget,
       AsyncCandidateProducerContinuationFrozenProducerTokens,
       AsyncCandidateProducerContinuationFrozenCandidateTokens,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationCausalWeight,
       AsyncCandidateProducerContinuationTargetAtStatus,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationResolutionPredecessorsFor,
       AsyncCandidateProducerContinuationSelectedResolutionRecord,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationStatusRank,
       AsyncCausalEpisodeCandidates,
       AsyncCausalEpisodeFrozenPredecessorOrigins,
       AsyncCausalEpisodeServeWorkBudget,
       AsyncCausalEpisodeServeWorkTokens,
       AsyncCausalEpisodeServeReachDebt,
       AsyncCausalEpisodeStructuralRankOrdering,
       AsyncCausalEpisodeStructuralRankCarrier,
       AsyncCausalEpisodeServeRankOrdering,
       AsyncCausalEpisodeServeRankCarrier,
       SetLessThan, OpToRel

AsyncCandidateProducerContinuationFrozenPrefixDescentProperty(
    specification, initialContext) ==
  specification
    => \A node \in AsyncVotersAt(initialContext),
          identity,
          targetOrdinal \in Nat \ {0},
          targetStage \in AsyncCandidateServiceStageClasses,
          status \in {"Reserved", "Materialized"},
          budget \in
            AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
         AsyncCandidateProducerContinuationFrozenPrefixAtBudget(
           node, identity, targetOrdinal, targetStage, status, budget)
           ~> AsyncCandidateProducerContinuationPrefixDescentGoal(
                node, identity, targetOrdinal, targetStage,
                status, budget)

AsyncCandidateProducerContinuationFrozenPrefixClosureProperty(
    specification, initialContext) ==
  specification
    => \A node \in AsyncVotersAt(initialContext),
          identity,
          targetOrdinal \in Nat \ {0},
          targetStage \in AsyncCandidateServiceStageClasses,
          status \in {"Reserved", "Materialized"},
          budget \in
            AsyncCandidateProducerContinuationFrozenPrefixRankCarrier:
         AsyncCandidateProducerContinuationFrozenPrefixAtBudget(
           node, identity, targetOrdinal, targetStage, status, budget)
           ~> AsyncCandidateProducerContinuationTargetStatusExit(
                identity, status)

AsyncCandidateProducerContinuationDormantReservationGoal(record) ==
  \/ AsyncCandidateProducerContinuationConcreteSuccessorOwned(record)
  \/ AsyncCandidateProducerContinuationHandoffRetired(record)
  \/ AsyncCandidateProducerContinuationDurableTerminal(record)
  \/ AsyncCandidateProducerContinuationTargetStatusExit(
       record.identity, "Reserved")

\* This leaf is deliberately post-GST and scoped to the frozen voters for
\* which AsyncFairnessAt provides the three producer-continuation actions.
\* Adequate-leader callers prove that their selected immutable candidate and
\* its retained continuation have the same voter owner before applying it.
\* Historical recovery targets outside the frozen roster use their separate
\* indexed recovery corridor and are not silently granted voter fairness.
\*
\* Before GST a volatile Terminal may be reopened by responsive
\* restart/replay, and that recovery is discharged by the separate
\* reset/replay kernel rather than by this fixed reservation episode.
AsyncCandidateProducerContinuationDormantReservationClosureProperty(
    specification, initialContext) ==
  specification
    => \A node \in AsyncVotersAt(initialContext),
          record \in AsyncCandidateProducerContinuationRecordSet:
         /\ gst
         /\ record =
              AsyncCandidateProducerContinuationSelectedResolutionRecord(node)
         /\ record.status = "Reserved"
         /\ record \in
              AsyncCandidateProducerContinuationResolutionRecordsForNode(node)
           ~> AsyncCandidateProducerContinuationDormantReservationGoal(record)

THEOREM CandidateProducerContinuationDormantGoalIsReadyOrExited ==
  \A node \in ValidatorIds,
     record \in AsyncCandidateProducerContinuationRecordSet:
    /\ record =
         AsyncCandidateProducerContinuationSelectedResolutionRecord(node)
    /\ record.status = "Reserved"
    /\ record \in
         AsyncCandidateProducerContinuationResolutionRecordsForNode(node)
    /\ AsyncCandidateProducerContinuationDormantReservationGoal(record)
      => \/ AsyncCandidateProducerContinuationResolutionReady(node)
         \/ AsyncCandidateProducerContinuationTargetStatusExit(
              record.identity, "Reserved")
BY Isa
   DEF AsyncCandidateProducerContinuationDormantReservationGoal,
       AsyncCandidateProducerContinuationResolutionReady,
       AsyncCandidateProducerContinuationHandoffRetired,
       AsyncCandidateProducerContinuationDurableTerminal,
       AsyncCandidateProducerContinuationTargetStatusExit,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn

THEOREM AsyncCandidateProducerContinuationReclamationPreservesIdentity ==
  \A state,
     record \in state.producerContinuations:
    ~AsyncNodeHasDecisionAfter(record.node)
      => AsyncCandidateProducerContinuationRecordAfterStep(state, record)
           \in (AsyncCandidateServiceStateAfterReclamation(state))
                .producerContinuations
BY Isa DEF AsyncCandidateServiceStateAfterReclamation

THEOREM AsyncCandidateProducerContinuationPreservedOrTerminal ==
  \A state, identity:
    AsyncCandidateProducerContinuationActiveForIdentityIn(state, identity)
      => \/ \E record \in
                 AsyncCandidateProducerContinuationRecordsForIdentityIn(
                   state, identity):
               AsyncNodeHasDecisionAfter(record.node)
         \/ AsyncCandidateProducerContinuationActiveForIdentityIn(
              AsyncCandidateServiceStateAfterReclamation(state), identity)
         \/ AsyncCandidateProducerContinuationTerminalForIdentityIn(
              AsyncCandidateServiceStateAfterReclamation(state), identity)
BY AsyncCandidateProducerContinuationReclamationPreservesIdentity,
   AsyncCandidateProducerContinuationDecisionReclamationClearsNode, Isa
   DEF AsyncCandidateProducerContinuationActiveForIdentityIn,
       AsyncCandidateProducerContinuationTerminalForIdentityIn,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationRecordAfterStep

THEOREM AsyncCandidateProducerContinuationSameHeightRestartPreserved ==
  \A state, resetNodes:
    LET resetState ==
          AsyncControlServiceStateAfterReset(state, resetNodes)
    IN {record \in resetState.producerContinuations:
          AsyncCandidateProducerContinuationRestartStableTerminalIn(
            resetState, record)}
         =
       {record \in state.producerContinuations:
          AsyncCandidateProducerContinuationRestartStableTerminalIn(
            state, record)}
BY Isa
   DEF AsyncControlServiceStateAfterReset,
       AsyncCandidateProducerContinuationsAfterReset,
       AsyncCandidateProducerContinuationRestartStableTerminalIn

THEOREM AsyncCandidateProducerContinuationResetPreservesActiveReservation ==
  \A state, resetNodes, record:
    /\ record \in state.producerContinuations
    /\ record.status \in {"Reserved", "Materialized"}
      => \E after \in
           AsyncCandidateProducerContinuationsAfterReset(
             state, resetNodes):
           /\ after.identity = record.identity
           /\ after.address = record.address
           /\ after.causalOrigin = record.causalOrigin
           /\ after.sourceClass = record.sourceClass
           /\ after.ordinal = record.ordinal
           /\ IF /\ record.node \in resetNodes
                  /\ record.status = "Materialized"
              THEN after.status = "Reserved"
              ELSE after.status = record.status
BY AsyncCandidateProducerContinuationResetPreservesExactReservation

THEOREM AsyncCandidateProducerContinuationResetReopensOnlyUnstableTerminal ==
  \A state, resetNodes, record:
    /\ record \in state.producerContinuations
    /\ record.node \in resetNodes
    /\ record.status = "Terminal"
    /\ ~AsyncCandidateProducerContinuationRestartStableTerminalIn(
         state, record)
      => \E after \in
           AsyncCandidateProducerContinuationsAfterReset(
             state, resetNodes):
           /\ after.identity = record.identity
           /\ after.address = record.address
           /\ after.causalOrigin = record.causalOrigin
           /\ after.sourceClass = record.sourceClass
           /\ after.ordinal = record.ordinal
           /\ after.status = "Reserved"
BY AsyncCandidateProducerContinuationResetPreservesExactReservation

THEOREM AsyncCandidateProducerContinuationResetCannotResurrectDifferentOwner ==
  \A state, resetNodes, record:
    record \in state.producerContinuations
      => \E after \in
           AsyncCandidateProducerContinuationsAfterReset(
             state, resetNodes):
           AsyncCandidateProducerSemanticHandoffReservationToken(after)
             = AsyncCandidateProducerSemanticHandoffReservationToken(record)
BY AsyncCandidateProducerContinuationResetPreservesExactReservation,
   Isa
   DEF AsyncCandidateProducerSemanticHandoffReservationToken

THEOREM AsyncCandidateProducerContinuationReplacementRetiresOnlyTerminal ==
  \A state, candidate,
     record \in state.producerContinuations:
    /\ AsyncCandidateProducerContinuationSourceAfter(candidate)
    /\ AsyncCandidateLifecycleRecordedIn(
         state, candidate.node, candidate.causalOrigin)
    /\ AsyncCandidateProducerContinuationRecordsForIdentityIn(
         state, AsyncCandidateServiceIdentity(candidate)) = {}
    /\ AsyncCandidateProducerContinuationReservationAvailableIn(
         state, candidate)
    /\ record \notin
         (AsyncCandidateProducerContinuationStateAfterDeparture(
            state, candidate)).producerContinuations
      => /\ record.status = "Terminal"
         /\ record.address =
              AsyncCandidateProducerContinuationAddressForIn(
                state, candidate)
         /\ record.context = candidate.consumerContext
         /\ record.height = candidate.height
         /\ record.view < candidate.view
         /\ record.ordinal <
              AsyncCandidateProducerContinuationOrdinalForIn(
                state, candidate)
BY Isa
   DEF AsyncCandidateProducerContinuationStateAfterDeparture,
       AsyncCandidateProducerContinuationReservationAvailableIn,
       AsyncCandidateProducerContinuationAddressCanAdvanceIn,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationRecordsForAddressIn,
       AsyncCandidateProducerContinuationAddressOwnedIn

THEOREM AsyncCandidateProducerContinuationExactRetryCoalesces ==
  \A candidate \in AsyncCandidateSet:
    AsyncCandidateProducerContinuationRecorded(candidate)
      => CandidateAdmissionCoalesced(candidate)
BY DEF AsyncCandidateProducerContinuationRecorded,
       AsyncCandidateProducerContinuationBlocks,
       CandidateAdmissionCoalesced

THEOREM AsyncCandidateProducerContinuationHighWatermarkBlocksOldStage ==
  \A candidate \in AsyncCandidateSet,
     record \in AsyncCandidateProducerContinuations:
    /\ record.node = candidate.node
    /\ record.context = candidate.consumerContext
    /\ record.height = candidate.height
    /\ record.address.stage
         = AsyncCandidateServiceStageForKind(candidate.kind)
    /\ record.view > candidate.view
      => CandidateAdmissionCoalesced(candidate)
BY DEF AsyncCandidateProducerContinuationBlocks,
       CandidateAdmissionCoalesced

THEOREM AsyncCandidateProducerContinuationRolloverOnlyStartsEmpty ==
  AsyncTransportInit
    => AsyncCandidateProducerContinuations = {}
BY DEF AsyncTransportInit, AsyncCandidateProducerContinuations

THEOREM LocalCandidateProducerContinuationResolutionUsesReviewedFairAction ==
  \A initialContext,
     node \in AsyncVotersAt(initialContext):
    PostGstResolveLocalCandidateProducerContinuation(node)
      => AsyncFairActionAt(initialContext)
BY DEF AsyncFairActionAt

THEOREM ConditionalTransportProducerContinuationServiceUsesReviewedFairAction ==
  \A initialContext,
     node \in AsyncVotersAt(initialContext):
    PostGstServiceConditionalTransportProducerContinuation(node)
      => AsyncFairActionAt(initialContext)
BY DEF AsyncFairActionAt

THEOREM VolatileBodyProducerContinuationServiceUsesReviewedFairAction ==
  \A initialContext,
     node \in AsyncVotersAt(initialContext):
    PostGstServiceVolatileBodyProducerContinuation(node)
      => AsyncFairActionAt(initialContext)
BY DEF AsyncFairActionAt

=============================================================================
