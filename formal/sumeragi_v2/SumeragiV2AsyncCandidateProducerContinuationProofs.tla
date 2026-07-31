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
      => \/ ~AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
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
       AsyncCandidateProducerContinuationLifecycleCoveredIn,
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
       AsyncCandidateProducerContinuationLifecycleCoveredIn,
       AsyncControlServiceSlotTransition,
       AsyncControlServiceStateAfterReset,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       PreGstResponsiveRestart, PreGstResponsiveReplay

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

THEOREM CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight ==
  \A command \in AsyncCandidateSet:
    AsyncCandidateProducerContinuationSuccessorBatchWeight(command)
      + AsyncCandidateProducerContinuationStatusRank("Reserved")
        < AsyncCandidateProducerContinuationCausalWeight(command.kind)
BY CommandSuccessorsHaveBoundedLength,
   AsyncCommandSuccessorsStrictlyLowerRemainingWorkStage,
   AsyncCommandSuccessorBatchStrictlyConsumesRemainingWork,
   SMTT(240)
   DEF AsyncCandidateProducerContinuationSuccessorBatchWeight,
       AsyncCandidateProducerContinuationCausalWeight,
       AsyncCandidateProducerContinuationStatusRank,
       AsyncCausalRemainingWorkWeight,
       AsyncCausalRemainingWorkStage,
       CommandSuccessors, SequenceSet,
       AsyncCandidateSet, AsyncCandidateTyped,
       AsyncWorkKinds, AsyncCompletionTags,
       AsyncDeliveryKinds, AsyncReducerKinds

AsyncCandidateProducerContinuationFrozenRecords(
    node, targetOrdinal) ==
  {record \in
     AsyncCandidateProducerContinuationResolutionRecordsForNode(node):
     /\ record.ordinal <= targetOrdinal
     /\ record.causalOrigin
          \in AsyncCandidateProducerContinuationFrozenPredecessorOrigins(
               node, targetOrdinal)}

\* A restart-dormant Local continuation is the only producer which may
\* republish its stored parent candidate.  Charge that exact candidate before
\* it becomes physical, but only while neither a concrete/declared handoff nor
\* deterministic retirement already makes the record Ready.  A fresh
\* post-GST service therefore does not create a second parent charge: its
\* atomic successor batch or empty endpoint makes the new record Ready.
\*
\* The latent and physical carriers intentionally use the same candidate
\* value below.  Exact Local replay removes the latent arm at the same instant
\* that it appends the physical carrier, so set union coalesces the transfer
\* instead of replenishing the frozen episode.
AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates(
    node, targetOrdinal) ==
  {candidate \in AsyncCandidateSet:
     \E record \in
          AsyncCandidateProducerContinuationFrozenRecords(
            node, targetOrdinal):
       /\ record.status = "Reserved"
       /\ record.sourceClass = "Local"
       /\ ~AsyncCandidateProducerContinuationConcreteSuccessorOwned(record)
       /\ ~AsyncCandidateProducerContinuationHandoffRetired(record)
       /\ candidate = record.candidate}

\* A leader-wire lifecycle owns its exact future Runtime candidate before the
\* physical ingress carrier drains.  Charge that candidate while the immutable
\* logical scheduler ordinal is inside the target cut, including while an
\* exact retry is Dormant.  Dormant-to-Ingress and Ingress-to-Runtime are then
\* carrier transfers of the same set element, not producer replenishment.
\* Fresh identities receive the current shared high-watermark and cannot enter
\* an older target cut; terminal identities contribute no latent candidate.
\* The strict boundary deliberately omits the target ordinal itself: an
\* equal-ordinal leader-wire/continuation pair is one shared lifecycle cell and
\* its Runtime publication coalesces against the target continuation identity.
AsyncCandidateProducerContinuationFrozenLeaderWireCandidates(
    node, targetOrdinal) ==
  {AsyncLeaderWireRuntimeCandidate(record.item):
     record \in asyncLeaderWireLifecycles,
     /\ record.recipient = node
     /\ record.schedulerOrdinal < targetOrdinal
     /\ \/ AsyncLeaderWireLifecycleDormant(record)
        \/ AsyncLeaderWireLifecycleActive(record)}

AsyncCandidateProducerContinuationFrozenCandidateOwners(
    node, targetOrdinal) ==
  AsyncCausalEpisodeCandidates(node, targetOrdinal)
    \cup
      AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates(
        node, targetOrdinal)
    \cup
      AsyncCandidateProducerContinuationFrozenLeaderWireCandidates(
        node, targetOrdinal)

AsyncCandidateProducerContinuationFrozenCandidateTokens(
    node, targetOrdinal) ==
  {<<"Candidate", candidate, token>>:
     candidate \in
       AsyncCandidateProducerContinuationFrozenCandidateOwners(
         node, targetOrdinal),
     token \in
       1..AsyncCandidateProducerContinuationCausalWeight(candidate.kind)}

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

(***************************************************************************
Claim-specific frozen-source prefix.

The claim stores immutable source sets at its atomic admission cut.  These
operators intersect live carriers with those stored sources; they never
recompute the predecessor universe from an ordinal cutoff.  A Candidate must
match both its frozen causal origin and exact `{origin, lifecycleOrdinal}`
source.  A continuation uses that same inherited source.  A Serve occurrence
must match the exact
`{identity, ingressOrdinal, schedulerOrdinal, lifecycleOrdinal}` admission
token.  The retained lifecycle ordinal is the internal Serve reservation id;
downstream ownership therefore requires both the logical identity and that
immutable id.  Removing an old lifecycle and later admitting the same logical
identity cannot resurrect the old frozen source.
***************************************************************************)

AsyncCandidateProducerContinuationFrozenSourceRecords(
    node, frozenCandidateOrigins, frozenContinuationSources) ==
  {record \in
     AsyncCandidateProducerContinuationResolutionRecordsForNode(node):
     /\ record.causalOrigin \in frozenCandidateOrigins
     /\ AsyncCandidateProducerContinuationLifecycleSource(record)
          \in frozenContinuationSources}

AsyncCandidateProducerContinuationFrozenSourceScheduledCandidates(
    node, frozenCandidateOrigins, frozenContinuationSources) ==
  {candidate \in
       QueuedCandidates \cup DeferredCandidates
         \cup CausalCandidates \cup TrackedWorkCandidates:
     /\ candidate.node = node
     /\ candidate.causalOrigin \in frozenCandidateOrigins
     /\ AsyncCandidateLifecycleSource(
          candidate.causalOrigin,
          AsyncCandidateLifecycleOrdinal(candidate))
          \in frozenContinuationSources}

AsyncCandidateProducerContinuationFrozenSourceDormantLocalReplayCandidates(
    node, frozenCandidateOrigins, frozenContinuationSources) ==
  {candidate \in AsyncCandidateSet:
     \E record \in
          AsyncCandidateProducerContinuationFrozenSourceRecords(
            node, frozenCandidateOrigins, frozenContinuationSources):
       /\ record.status = "Reserved"
       /\ record.sourceClass = "Local"
       /\ ~AsyncCandidateProducerContinuationConcreteSuccessorOwned(record)
       /\ ~AsyncCandidateProducerContinuationHandoffRetired(record)
       /\ candidate = record.candidate}

AsyncCandidateProducerContinuationFrozenSourceCandidateOwners(
    node, frozenCandidateOrigins, frozenContinuationSources) ==
  AsyncCandidateProducerContinuationFrozenSourceScheduledCandidates(
    node, frozenCandidateOrigins, frozenContinuationSources)
    \cup
      AsyncCandidateProducerContinuationFrozenSourceDormantLocalReplayCandidates(
        node, frozenCandidateOrigins, frozenContinuationSources)

AsyncCandidateProducerContinuationFrozenSourceCandidateTokens(
    node, frozenCandidateOrigins, frozenContinuationSources) ==
  {<<"Candidate", candidate, token>>:
     candidate \in
       AsyncCandidateProducerContinuationFrozenSourceCandidateOwners(
         node, frozenCandidateOrigins, frozenContinuationSources),
     token \in
       1..AsyncCandidateProducerContinuationCausalWeight(candidate.kind)}

AsyncCandidateProducerContinuationFrozenSourceStatusTokens(
    node, frozenCandidateOrigins, frozenContinuationSources) ==
  {<<"Continuation", record.identity, token>>:
     record \in
       AsyncCandidateProducerContinuationFrozenSourceRecords(
         node, frozenCandidateOrigins, frozenContinuationSources),
     token \in
       1..AsyncCandidateProducerContinuationStatusRank(record.status)}

AsyncCandidateProducerContinuationFrozenSourceProducerTokens(
    node, frozenCandidateOrigins, frozenContinuationSources) ==
  AsyncCandidateProducerContinuationFrozenSourceCandidateTokens(
    node, frozenCandidateOrigins, frozenContinuationSources)
    \cup
      AsyncCandidateProducerContinuationFrozenSourceStatusTokens(
        node, frozenCandidateOrigins, frozenContinuationSources)

AsyncCandidateProducerContinuationFrozenSourceProducerBudget(
    node, frozenCandidateOrigins, frozenContinuationSources) ==
  Cardinality(
    AsyncCandidateProducerContinuationFrozenSourceProducerTokens(
      node, frozenCandidateOrigins, frozenContinuationSources))

AsyncFrozenServeAdmissionSources(node, frozenServeSources) ==
  {admission \in asyncServeIngressAdmissions:
     /\ admission.node = node
     /\ AsyncServeIngressSourceFor(admission) \in frozenServeSources}

AsyncFrozenServeSourceIdentities(frozenServeSources) ==
  {source.identity: source \in frozenServeSources}

AsyncFrozenServeExactIngressSources(node, frozenServeSources) ==
  {source \in frozenServeSources:
     \E admission \in AsyncFrozenServeAdmissionSources(
          node, frozenServeSources):
       AsyncServeIngressSourceFor(admission) = source}

AsyncFrozenServeExactIngressIdentities(node, frozenServeSources) ==
  {source.identity:
     source \in
       AsyncFrozenServeExactIngressSources(node, frozenServeSources)}

AsyncFrozenServeLifecycleSources(node, frozenServeSources) ==
  {source \in frozenServeSources:
     \E reservation \in
          AsyncServeReservationRecords(node, source.identity):
       reservation.ordinal = source.lifecycleOrdinal}

AsyncFrozenServeLifecycleIdentities(node, frozenServeSources) ==
  {source.identity:
     source \in AsyncFrozenServeLifecycleSources(
       node, frozenServeSources)}

AsyncFrozenServeSourceOwned(node, source) ==
  \/ source
       \in AsyncFrozenServeExactIngressSources(node, {source})
  \/ source
       \in AsyncFrozenServeLifecycleSources(node, {source})

AsyncFrozenServeIngressPrefixTokens(node, frozenServeSources) ==
  UNION {
    {<<"ServeIngress", source, slot>>:
       slot \in
         AsyncServeIngressAdmissionPredecessorDebtSlots(
           node, source.identity)}:
    source \in
      AsyncFrozenServeExactIngressSources(node, frozenServeSources)}

AsyncFrozenServeIoPredecessorTokens(node, frozenServeSources) ==
  UNION {
    {<<"ServeIo", source, job>>:
       job \in
         AsyncServeFrozenPredecessorSet(node, source.identity)}:
    source \in
      AsyncFrozenServeLifecycleSources(node, frozenServeSources)}

AsyncFrozenServeOccurrenceTokens(node, frozenServeSources) ==
  {<<"ServeOccurrence", source>>:
     source \in
       AsyncFrozenServeExactIngressSources(node, frozenServeSources)
         \cup AsyncFrozenServeLifecycleSources(
                node, frozenServeSources)}

AsyncFrozenServeWorkTokens(node, frozenServeSources) ==
  AsyncFrozenServeOccurrenceTokens(node, frozenServeSources)
    \cup AsyncFrozenServeIngressPrefixTokens(node, frozenServeSources)
    \cup AsyncFrozenServeIoPredecessorTokens(node, frozenServeSources)

AsyncFrozenServeWorkBudget(node, frozenServeSources) ==
  Cardinality(AsyncFrozenServeWorkTokens(node, frozenServeSources))

AsyncFrozenServeReachDebt(node, frozenServeSources) ==
  IF AsyncFrozenServeExactIngressSources(node, frozenServeSources) = {}
  THEN 0
  ELSE DrainableIngressTurnReachRank(node)

AsyncFrozenServeIoOwnerRequired(node, frozenServeSources) ==
  \E source \in
       AsyncFrozenServeLifecycleSources(node, frozenServeSources):
    /\ ~AsyncServeJobQueued(node, source.identity)
    /\ ~CanResumeExactServeCapacity(node, source.identity)

THEOREM AsyncFrozenServeSourceCannotResurrectAtGst ==
  \A node \in ValidatorIds, source \in AsyncServeIngressSourceSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ gst
    /\ source.ordinal < AsyncNextIngressPhysicalOrdinal(node)
    /\ source.schedulerOrdinal
         < AsyncNextCandidateLifecycleOrdinal(node)
    /\ \/ source.lifecycleOrdinal = 0
       \/ source.lifecycleOrdinal
            < asyncNextServeAdmissionOrdinal[node]
    /\ ~AsyncFrozenServeSourceOwned(node, source)
    /\ [AsyncNext]_AsyncAllVars
      => ~AsyncFrozenServeSourceOwned(node, source)'
BY AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal,
   AsyncIngressPhysicalHighWatermarkIsMonotone,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   IsaT(1800)
   DEF AsyncFrozenServeSourceOwned,
       AsyncFrozenServeExactIngressSources,
       AsyncFrozenServeAdmissionSources,
       AsyncFrozenServeLifecycleSources,
       AsyncServeIngressSourceFor,
       AsyncServeReservationRecords,
       AsyncAllVars

AsyncCandidateProducerContinuationFrozenSourcePrefixRank(
    node, frozenCandidateOrigins, frozenServeSources,
    frozenContinuationSources) ==
  <<AsyncCandidateProducerContinuationFrozenSourceProducerBudget(
       node, frozenCandidateOrigins, frozenContinuationSources),
    <<AsyncFrozenServeWorkBudget(node, frozenServeSources),
      AsyncFrozenServeReachDebt(node, frozenServeSources)>>>

(***************************************************************************
Frozen leader-wire ingress barrier.

Two callers share this rank but freeze different predecessor cuts:

  * a producer continuation has no physical carrier, so every immutable
    leader-wire identity with a smaller logical scheduler ordinal is in its
    finite owner universe.  A dormant retry retains that identity, consumes
    one prepaid non-descent stage, and receives a fresh physical ordinal;
  * a claimed response already owns a physical carrier.  Only non-dormant
    records whose physical carrier was admitted before the claim are
    predecessors.  A dormant identity replayed later is physically behind the
    claim even when its retained logical scheduler ordinal is smaller.

The stage budget is deliberately the outer component.  Dormant-to-Ingress
replacement consumes 2 -> 1, and Ingress-to-Runtime/terminal transfer consumes
1 -> 0 before the resulting Candidate can increase the inner producer budget.
Neither replacement nor transfer is advertised as semantic progress.  The
dependency tail then reuses the existing physical-owner/frozen-prefix pair and
spells out the mode, capacity, runner-reach, priority selector, lane, and
source components required to drain the selected physical occurrence.
***************************************************************************)

AsyncFrozenLeaderWireBarrierModes == {"Logical", "Physical"}

AsyncFrozenLeaderWireBarrierRecords(
    node, logicalCutoff, physicalCut, barrierMode) ==
  {record \in asyncLeaderWireLifecycles:
     /\ record.recipient = node
     /\ IF barrierMode = "Logical"
        THEN record.schedulerOrdinal <= logicalCutoff
        ELSE /\ barrierMode = "Physical"
             /\ ~AsyncLeaderWireLifecycleDormant(record)
             /\ record.physicalAdmissionOrdinal < physicalCut}

AsyncFrozenLeaderWireBarrierRemainingStage(record, barrierMode) ==
  CASE /\ barrierMode = "Logical"
          /\ AsyncLeaderWireLifecycleDormant(record) -> 2
    [] AsyncLeaderWireLifecycleIngressProtected(record) -> 1
    [] OTHER -> 0

AsyncFrozenLeaderWireBarrierStageTokens(
    node, logicalCutoff, physicalCut, barrierMode) ==
  {<<"LeaderWireBarrier", AsyncLeaderWirePotentialOwnerIdentity(record),
     token>>:
     record \in AsyncFrozenLeaderWireBarrierRecords(
                   node, logicalCutoff, physicalCut, barrierMode),
     token \in
       1..AsyncFrozenLeaderWireBarrierRemainingStage(
            record, barrierMode)}

AsyncFrozenLeaderWireBarrierStageBudget(
    node, logicalCutoff, physicalCut, barrierMode) ==
  Cardinality(
    AsyncFrozenLeaderWireBarrierStageTokens(
      node, logicalCutoff, physicalCut, barrierMode))

AsyncFrozenLeaderWireIngressRecords(
    node, logicalCutoff, physicalCut, barrierMode) ==
  {record \in
     AsyncFrozenLeaderWireBarrierRecords(
       node, logicalCutoff, physicalCut, barrierMode):
     AsyncLeaderWireLifecycleIngressProtected(record)}

AsyncFrozenLeaderWireSelectedIngressRecord(
    node, logicalCutoff, physicalCut, barrierMode) ==
  CHOOSE record \in
    AsyncFrozenLeaderWireIngressRecords(
      node, logicalCutoff, physicalCut, barrierMode):
    \A other \in
         AsyncFrozenLeaderWireIngressRecords(
           node, logicalCutoff, physicalCut, barrierMode):
      record.physicalAdmissionOrdinal
        <= other.physicalAdmissionOrdinal

AsyncFrozenLeaderWireIngressModeRank(node) ==
  IF NodeHasApplication(node) THEN 0 ELSE 1

AsyncFrozenLeaderWireIngressCapacityRank(node, item) ==
  IF IngressItemCanDrain(node, item)
  THEN 0
  ELSE AsyncQueueDepth(node) + AsyncIoQueueDepth(node) + 1

AsyncFrozenLeaderWireIngressPriorityOwners(node) ==
  {pair \in AsyncIngressSources \X (1..AsyncIngressCapacity):
     \/ pair[2] \in
          DrainableClaimedResponseLaneIndices(node, pair[1])
     \/ pair[2] \in
          DrainableRequestFencedCompletionLaneIndices(node, pair[1])}

AsyncFrozenLeaderWireIngressPriorityRank(node) ==
  Cardinality(AsyncFrozenLeaderWireIngressPriorityOwners(node))

AsyncFrozenLeaderWireIngressLaneIndices(node, item) ==
  {index \in
       1..Len(IngressLane(node, IngressResourceSource(item))):
     IngressLane(node, IngressResourceSource(item))[index] = item}

AsyncFrozenLeaderWireIngressLanePosition(node, item) ==
  CHOOSE least \in AsyncFrozenLeaderWireIngressLaneIndices(node, item):
    \A other \in AsyncFrozenLeaderWireIngressLaneIndices(node, item):
      least <= other

AsyncFrozenLeaderWireIngressSourcePosition(node, item) ==
  IngressSourceServiceRank(node, IngressResourceSource(item))

AsyncFrozenLeaderWireIngressRunnerRank(node) ==
  IF NodeHasApplication(node)
  THEN 0
  ELSE DrainableIngressTurnReachRank(node)

AsyncFrozenLeaderWireIngressLaneRank(node, item) ==
  <<AsyncFrozenLeaderWireIngressLanePosition(node, item),
    AsyncFrozenLeaderWireIngressSourcePosition(node, item)>>

AsyncFrozenLeaderWireIngressSelectorRank(node, item) ==
  <<AsyncFrozenLeaderWireIngressPriorityRank(node),
    AsyncFrozenLeaderWireIngressLaneRank(node, item)>>

AsyncFrozenLeaderWireIngressRunnerSelectorRank(node, item) ==
  <<AsyncFrozenLeaderWireIngressRunnerRank(node),
    AsyncFrozenLeaderWireIngressSelectorRank(node, item)>>

AsyncFrozenLeaderWireIngressCapacitySelectorRank(node, item) ==
  <<AsyncFrozenLeaderWireIngressCapacityRank(node, item),
    AsyncFrozenLeaderWireIngressRunnerSelectorRank(node, item)>>

AsyncFrozenLeaderWireIngressRank(node, item) ==
  <<AsyncFrozenLeaderWireIngressModeRank(node),
    AsyncFrozenLeaderWireIngressCapacitySelectorRank(node, item)>>

AsyncFrozenLeaderWireIngressLaneCarrier == Nat \X Nat
AsyncFrozenLeaderWireIngressSelectorCarrier ==
  Nat \X AsyncFrozenLeaderWireIngressLaneCarrier
AsyncFrozenLeaderWireIngressRunnerSelectorCarrier ==
  Nat \X AsyncFrozenLeaderWireIngressSelectorCarrier
AsyncFrozenLeaderWireIngressCapacitySelectorCarrier ==
  Nat \X AsyncFrozenLeaderWireIngressRunnerSelectorCarrier
AsyncFrozenLeaderWireIngressRankCarrier ==
  (0..1) \X AsyncFrozenLeaderWireIngressCapacitySelectorCarrier

AsyncFrozenLeaderWireIngressLaneOrdering ==
  LexPairOrdering(OpToRel(<, Nat), OpToRel(<, Nat), Nat, Nat)

AsyncFrozenLeaderWireIngressSelectorOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    AsyncFrozenLeaderWireIngressLaneOrdering,
    Nat, AsyncFrozenLeaderWireIngressLaneCarrier)

AsyncFrozenLeaderWireIngressRunnerSelectorOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    AsyncFrozenLeaderWireIngressSelectorOrdering,
    Nat, AsyncFrozenLeaderWireIngressSelectorCarrier)

AsyncFrozenLeaderWireIngressCapacitySelectorOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    AsyncFrozenLeaderWireIngressRunnerSelectorOrdering,
    Nat, AsyncFrozenLeaderWireIngressRunnerSelectorCarrier)

AsyncFrozenLeaderWireIngressRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    AsyncFrozenLeaderWireIngressCapacitySelectorOrdering,
    0..1, AsyncFrozenLeaderWireIngressCapacitySelectorCarrier)

AsyncFrozenLeaderWirePhysicalRankCarrier == Nat \X Nat

AsyncFrozenLeaderWirePhysicalRankOrdering ==
  LexPairOrdering(OpToRel(<, Nat), OpToRel(<, Nat), Nat, Nat)

AsyncFrozenLeaderWireIngressDependencyRankCarrier ==
  AsyncFrozenLeaderWirePhysicalRankCarrier
    \X AsyncFrozenLeaderWireIngressRankCarrier

AsyncFrozenLeaderWireIngressDependencyRank(
    node, logicalCutoff, physicalCut, barrierMode) ==
  LET records ==
        AsyncFrozenLeaderWireIngressRecords(
          node, logicalCutoff, physicalCut, barrierMode)
  IN IF records = {}
     THEN CHOOSE rank \in
            AsyncFrozenLeaderWireIngressDependencyRankCarrier: TRUE
     ELSE LET selected ==
                AsyncFrozenLeaderWireSelectedIngressRecord(
                  node, logicalCutoff, physicalCut, barrierMode)
          IN <<AsyncLeaderWirePhysicalIngressRank(selected),
               AsyncFrozenLeaderWireIngressRank(node, selected.item)>>

AsyncFrozenLeaderWireIngressDependencyRankOrdering ==
  LexPairOrdering(
    AsyncFrozenLeaderWirePhysicalRankOrdering,
    AsyncFrozenLeaderWireIngressRankOrdering,
    AsyncFrozenLeaderWirePhysicalRankCarrier,
    AsyncFrozenLeaderWireIngressRankCarrier)

AsyncFrozenLeaderWireBarrierTailRank(
    node, prefixCutoff, logicalCutoff, physicalCut, barrierMode) ==
  <<AsyncCandidateProducerContinuationFrozenPrefixRank(
      node, prefixCutoff),
    AsyncFrozenLeaderWireIngressDependencyRank(
      node, logicalCutoff, physicalCut, barrierMode)>>

AsyncFrozenLeaderWireBarrierTailRankCarrier ==
  AsyncCandidateProducerContinuationFrozenPrefixRankCarrier
    \X AsyncFrozenLeaderWireIngressDependencyRankCarrier

AsyncFrozenLeaderWireBarrierTailRankOrdering ==
  LexPairOrdering(
    AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
    AsyncFrozenLeaderWireIngressDependencyRankOrdering,
    AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
    AsyncFrozenLeaderWireIngressDependencyRankCarrier)

AsyncFrozenLeaderWireBarrierRank(
    node, prefixCutoff, logicalCutoff, physicalCut, barrierMode) ==
  <<AsyncFrozenLeaderWireBarrierStageBudget(
      node, logicalCutoff, physicalCut, barrierMode),
    AsyncFrozenLeaderWireBarrierTailRank(
      node, prefixCutoff, logicalCutoff, physicalCut, barrierMode)>>

AsyncFrozenLeaderWireBarrierRankCarrier ==
  Nat \X AsyncFrozenLeaderWireBarrierTailRankCarrier

AsyncFrozenLeaderWireBarrierRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    AsyncFrozenLeaderWireBarrierTailRankOrdering,
    Nat, AsyncFrozenLeaderWireBarrierTailRankCarrier)

AsyncCandidateProducerContinuationIngressBarrierRank(
    node, targetOrdinal) ==
  AsyncFrozenLeaderWireBarrierRank(
    node, targetOrdinal, targetOrdinal - 1, 0, "Logical")

THEOREM CandidateProducerContinuationStrictLeaderWireCutMatchesLogicalBarrier ==
  \A node \in ValidatorIds,
     targetOrdinal \in Nat \ {0}:
    AsyncStrongTypeInvariant
      => AsyncCandidateProducerContinuationFrozenLeaderWireCandidates(
           node, targetOrdinal)
           =
         {AsyncLeaderWireRuntimeCandidate(record.item):
            record \in
              AsyncFrozenLeaderWireBarrierRecords(
                node, targetOrdinal - 1, 0, "Logical"),
            /\ AsyncLeaderWireLifecycleDormant(record)
               \/ AsyncLeaderWireLifecycleActive(record)}
BY Isa
   DEF AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncFrozenLeaderWireBarrierRecords,
       AsyncLeaderWireLifecycleDormant,
       AsyncLeaderWireLifecycleActive,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncLeaderWireLifecycleTypeInvariant,
       AsyncLeaderWireLifecycleTyped

(***************************************************************************
The claimed-response physical barrier is selected only from the immutable
leader-wire owner tokens stored in the claim and still below its physical
cut.  Retrying a dormant identity allocates a fresh physical ordinal and
cannot rejoin this set.  Ingress-to-Candidate handoff remains prepaid because
the same record's `{causalOrigin, schedulerOrdinal}` source was copied into
the Candidate/continuation snapshots at claim admission.
***************************************************************************)
AsyncCertifiedResponseFrozenLeaderWireRecords(
    node, physicalCut, frozenLeaderWireIdentities) ==
  {record \in asyncLeaderWireLifecycles:
     /\ record.recipient = node
     /\ AsyncLeaderWirePotentialOwnerIdentity(record)
          \in frozenLeaderWireIdentities
     /\ record.physicalAdmissionOrdinal < physicalCut}

AsyncCertifiedResponseFrozenLeaderWireStageTokens(
    node, physicalCut, frozenLeaderWireIdentities) ==
  {<<"LeaderWireBarrier", AsyncLeaderWirePotentialOwnerIdentity(record),
     token>>:
     record \in
       AsyncCertifiedResponseFrozenLeaderWireRecords(
         node, physicalCut, frozenLeaderWireIdentities),
     token \in
       1..AsyncFrozenLeaderWireBarrierRemainingStage(record, "Physical")}

AsyncCertifiedResponseFrozenLeaderWireStageBudget(
    node, physicalCut, frozenLeaderWireIdentities) ==
  Cardinality(
    AsyncCertifiedResponseFrozenLeaderWireStageTokens(
      node, physicalCut, frozenLeaderWireIdentities))

AsyncCertifiedResponseFrozenLeaderWireIngressRecords(
    node, physicalCut, frozenLeaderWireIdentities) ==
  {record \in
     AsyncCertifiedResponseFrozenLeaderWireRecords(
       node, physicalCut, frozenLeaderWireIdentities):
     AsyncLeaderWireLifecycleIngressProtected(record)}

AsyncCertifiedResponseFrozenLeaderWireSelectedIngressRecord(
    node, physicalCut, frozenLeaderWireIdentities) ==
  CHOOSE record \in
    AsyncCertifiedResponseFrozenLeaderWireIngressRecords(
      node, physicalCut, frozenLeaderWireIdentities):
    \A other \in
         AsyncCertifiedResponseFrozenLeaderWireIngressRecords(
           node, physicalCut, frozenLeaderWireIdentities):
      record.physicalAdmissionOrdinal
        <= other.physicalAdmissionOrdinal

AsyncCertifiedResponseFrozenLeaderWireIngressDependencyRank(
    node, physicalCut, frozenLeaderWireIdentities) ==
  LET records ==
        AsyncCertifiedResponseFrozenLeaderWireIngressRecords(
          node, physicalCut, frozenLeaderWireIdentities)
  IN IF records = {}
     THEN CHOOSE rank \in
            AsyncFrozenLeaderWireIngressDependencyRankCarrier: TRUE
     ELSE LET selected ==
                AsyncCertifiedResponseFrozenLeaderWireSelectedIngressRecord(
                  node, physicalCut, frozenLeaderWireIdentities)
          IN <<AsyncLeaderWirePhysicalIngressRank(selected),
               AsyncFrozenLeaderWireIngressRank(node, selected.item)>>

AsyncCertifiedResponseFrozenSourceBarrierTailRank(
    node, physicalCut, frozenCandidateOrigins, frozenServeSources,
    frozenContinuationSources, frozenLeaderWireIdentities) ==
  <<AsyncCandidateProducerContinuationFrozenSourcePrefixRank(
      node, frozenCandidateOrigins, frozenServeSources,
      frozenContinuationSources),
    AsyncCertifiedResponseFrozenLeaderWireIngressDependencyRank(
      node, physicalCut, frozenLeaderWireIdentities)>>

AsyncCertifiedResponsePhysicalBarrierRank(
    node, physicalCut, frozenCandidateOrigins, frozenServeSources,
    frozenContinuationSources, frozenLeaderWireIdentities) ==
  <<AsyncCertifiedResponseFrozenLeaderWireStageBudget(
      node, physicalCut, frozenLeaderWireIdentities),
    AsyncCertifiedResponseFrozenSourceBarrierTailRank(
      node, physicalCut, frozenCandidateOrigins, frozenServeSources,
      frozenContinuationSources, frozenLeaderWireIdentities)>>

THEOREM AsyncFrozenLeaderWireIngressRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    AsyncFrozenLeaderWireIngressRankOrdering,
    AsyncFrozenLeaderWireIngressRankCarrier)
BY NatLessThanWellFounded, WFLexPairOrdering
   DEF AsyncFrozenLeaderWireIngressRankOrdering,
       AsyncFrozenLeaderWireIngressRankCarrier,
       AsyncFrozenLeaderWireIngressCapacitySelectorOrdering,
       AsyncFrozenLeaderWireIngressCapacitySelectorCarrier,
       AsyncFrozenLeaderWireIngressRunnerSelectorOrdering,
       AsyncFrozenLeaderWireIngressRunnerSelectorCarrier,
       AsyncFrozenLeaderWireIngressSelectorOrdering,
       AsyncFrozenLeaderWireIngressSelectorCarrier,
       AsyncFrozenLeaderWireIngressLaneOrdering,
       AsyncFrozenLeaderWireIngressLaneCarrier

THEOREM AsyncFrozenLeaderWireIngressDependencyOrderingIsWellFounded ==
  IsWellFoundedOn(
    AsyncFrozenLeaderWireIngressDependencyRankOrdering,
    AsyncFrozenLeaderWireIngressDependencyRankCarrier)
BY NatLessThanWellFounded,
   AsyncFrozenLeaderWireIngressRankOrderingIsWellFounded,
   WFLexPairOrdering
   DEF AsyncFrozenLeaderWireIngressDependencyRankOrdering,
       AsyncFrozenLeaderWireIngressDependencyRankCarrier,
       AsyncFrozenLeaderWirePhysicalRankOrdering,
       AsyncFrozenLeaderWirePhysicalRankCarrier

THEOREM AsyncFrozenLeaderWireBarrierRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    AsyncFrozenLeaderWireBarrierRankOrdering,
    AsyncFrozenLeaderWireBarrierRankCarrier)
BY CandidateProducerContinuationFrozenPrefixRankOrderingIsWellFounded,
   AsyncFrozenLeaderWireIngressDependencyOrderingIsWellFounded,
   NatLessThanWellFounded, WFLexPairOrdering
   DEF AsyncFrozenLeaderWireBarrierRankOrdering,
       AsyncFrozenLeaderWireBarrierRankCarrier,
       AsyncFrozenLeaderWireBarrierTailRankOrdering,
       AsyncFrozenLeaderWireBarrierTailRankCarrier

THEOREM AsyncFrozenLeaderWireBarrierRankIsFinite ==
  \A node \in ValidatorIds,
     prefixCutoff, logicalCutoff \in Nat,
     physicalCut \in Nat,
     barrierMode \in AsyncFrozenLeaderWireBarrierModes:
    AsyncStrongTypeInvariant
      => /\ IsFiniteSet(
               AsyncFrozenLeaderWireBarrierRecords(
                 node, logicalCutoff, physicalCut, barrierMode))
         /\ IsFiniteSet(
               AsyncFrozenLeaderWireBarrierStageTokens(
                 node, logicalCutoff, physicalCut, barrierMode))
         /\ AsyncFrozenLeaderWireBarrierRank(
              node, prefixCutoff, logicalCutoff,
              physicalCut, barrierMode)
              \in AsyncFrozenLeaderWireBarrierRankCarrier
BY AsyncCausalRemainingWorkWeightIsPositive,
   AsyncLeaderWirePotentialPredecessorUniverseIsFinite,
   CandidateProducerContinuationStrictLeaderWireCutMatchesLogicalBarrier,
   DrainableIngressTurnReachRankIsNatural,
   FS_Product, FS_Image, FS_Union, FS_Subset, FS_Interval,
   FS_CardinalityType, IsaT(1800)
   DEF AsyncFrozenLeaderWireBarrierModes,
       AsyncFrozenLeaderWireBarrierRecords,
       AsyncFrozenLeaderWireBarrierRemainingStage,
       AsyncFrozenLeaderWireBarrierStageTokens,
       AsyncFrozenLeaderWireBarrierStageBudget,
       AsyncFrozenLeaderWireIngressRecords,
       AsyncFrozenLeaderWireSelectedIngressRecord,
       AsyncFrozenLeaderWireIngressModeRank,
       AsyncFrozenLeaderWireIngressCapacityRank,
       AsyncFrozenLeaderWireIngressPriorityOwners,
       AsyncFrozenLeaderWireIngressPriorityRank,
       AsyncFrozenLeaderWireIngressLaneIndices,
       AsyncFrozenLeaderWireIngressLanePosition,
       AsyncFrozenLeaderWireIngressSourcePosition,
       AsyncFrozenLeaderWireIngressRunnerRank,
       AsyncFrozenLeaderWireIngressRank,
       AsyncFrozenLeaderWireIngressCapacitySelectorRank,
       AsyncFrozenLeaderWireIngressRunnerSelectorRank,
       AsyncFrozenLeaderWireIngressSelectorRank,
       AsyncFrozenLeaderWireIngressLaneRank,
       AsyncFrozenLeaderWireIngressRankCarrier,
       AsyncFrozenLeaderWireIngressCapacitySelectorCarrier,
       AsyncFrozenLeaderWireIngressRunnerSelectorCarrier,
       AsyncFrozenLeaderWireIngressSelectorCarrier,
       AsyncFrozenLeaderWireIngressLaneCarrier,
       AsyncFrozenLeaderWirePhysicalRankCarrier,
       AsyncFrozenLeaderWireIngressDependencyRank,
       AsyncFrozenLeaderWireIngressDependencyRankCarrier,
       AsyncFrozenLeaderWireBarrierTailRank,
       AsyncFrozenLeaderWireBarrierTailRankCarrier,
       AsyncFrozenLeaderWireBarrierRank,
       AsyncFrozenLeaderWireBarrierRankCarrier,
       AsyncCandidateProducerContinuationFrozenPrefixRank,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
       AsyncCandidateProducerContinuationFrozenProducerBudget,
       AsyncCandidateProducerContinuationFrozenProducerTokens,
       AsyncCandidateProducerContinuationFrozenCandidateTokens,
       AsyncCandidateProducerContinuationFrozenCandidateOwners,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCausalEpisodeServeWorkBudget,
       AsyncCausalEpisodeServeWorkTokens,
       AsyncCausalEpisodeServeReachDebt,
       AsyncLeaderWirePhysicalIngressRank,
       AsyncLeaderWireEarlierPhysicalOwners,
       AsyncLeaderWireFrozenIngressPredecessorDebtSet,
       AsyncLeaderWireLifecycleDormant,
       AsyncLeaderWireLifecycleIngressProtected,
       AsyncLeaderWirePotentialOwnerIdentity,
       IngressLane, IngressResourceSource, SequenceSet,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIngressTypeInvariant, AsyncLeaderWireLifecycleTypeInvariant

THEOREM AsyncCertifiedResponsePhysicalBarrierRankIsFinite ==
  \A node \in ValidatorIds,
     physicalCut \in Nat,
     frozenCandidateOrigins \in SUBSET AsyncCandidateCausalOriginSet,
     frozenServeSources \in SUBSET AsyncServeIngressSourceSet,
     frozenContinuationSources \in SUBSET AsyncCandidateLifecycleSourceSet,
     frozenLeaderWireIdentities
       \in SUBSET AsyncLeaderWirePotentialOwnerIdentitySet:
    /\ AsyncStrongTypeInvariant
    /\ IsFiniteSet(frozenCandidateOrigins)
    /\ IsFiniteSet(frozenServeSources)
    /\ IsFiniteSet(frozenContinuationSources)
    /\ IsFiniteSet(frozenLeaderWireIdentities)
      => /\ IsFiniteSet(
               AsyncCertifiedResponseFrozenLeaderWireRecords(
                 node, physicalCut, frozenLeaderWireIdentities))
         /\ IsFiniteSet(
               AsyncCertifiedResponseFrozenLeaderWireStageTokens(
                 node, physicalCut, frozenLeaderWireIdentities))
         /\ AsyncCertifiedResponsePhysicalBarrierRank(
              node, physicalCut, frozenCandidateOrigins,
              frozenServeSources, frozenContinuationSources,
              frozenLeaderWireIdentities)
              \in AsyncFrozenLeaderWireBarrierRankCarrier
BY AsyncCausalRemainingWorkWeightIsPositive,
   DrainableIngressTurnReachRankIsNatural,
   FS_Product, FS_Image, FS_Union, FS_Subset, FS_Interval,
   FS_CardinalityType, IsaT(1800)
   DEF AsyncCertifiedResponsePhysicalBarrierRank,
       AsyncCertifiedResponseFrozenSourceBarrierTailRank,
       AsyncCertifiedResponseFrozenLeaderWireRecords,
       AsyncCertifiedResponseFrozenLeaderWireStageTokens,
       AsyncCertifiedResponseFrozenLeaderWireStageBudget,
       AsyncCertifiedResponseFrozenLeaderWireIngressRecords,
       AsyncCertifiedResponseFrozenLeaderWireSelectedIngressRecord,
       AsyncCertifiedResponseFrozenLeaderWireIngressDependencyRank,
       AsyncCandidateProducerContinuationFrozenSourcePrefixRank,
       AsyncCandidateProducerContinuationFrozenSourceProducerBudget,
       AsyncCandidateProducerContinuationFrozenSourceProducerTokens,
       AsyncCandidateProducerContinuationFrozenSourceCandidateTokens,
       AsyncCandidateProducerContinuationFrozenSourceCandidateOwners,
       AsyncCandidateProducerContinuationFrozenSourceScheduledCandidates,
       AsyncCandidateProducerContinuationFrozenSourceDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenSourceStatusTokens,
       AsyncCandidateProducerContinuationFrozenSourceRecords,
       AsyncFrozenServeWorkBudget,
       AsyncFrozenServeWorkTokens,
       AsyncFrozenServeOccurrenceTokens,
       AsyncFrozenServeIngressPrefixTokens,
       AsyncFrozenServeIoPredecessorTokens,
       AsyncFrozenServeReachDebt,
       AsyncFrozenServeIoOwnerRequired,
       AsyncFrozenServeAdmissionSources,
       AsyncFrozenServeSourceIdentities,
       AsyncFrozenServeExactIngressSources,
       AsyncFrozenServeExactIngressIdentities,
       AsyncFrozenServeLifecycleSources,
       AsyncFrozenServeLifecycleIdentities,
       AsyncFrozenLeaderWireBarrierRemainingStage,
       AsyncFrozenLeaderWireIngressModeRank,
       AsyncFrozenLeaderWireIngressCapacityRank,
       AsyncFrozenLeaderWireIngressPriorityOwners,
       AsyncFrozenLeaderWireIngressPriorityRank,
       AsyncFrozenLeaderWireIngressLaneIndices,
       AsyncFrozenLeaderWireIngressLanePosition,
       AsyncFrozenLeaderWireIngressSourcePosition,
       AsyncFrozenLeaderWireIngressRunnerRank,
       AsyncFrozenLeaderWireIngressRank,
       AsyncFrozenLeaderWireIngressCapacitySelectorRank,
       AsyncFrozenLeaderWireIngressRunnerSelectorRank,
       AsyncFrozenLeaderWireIngressSelectorRank,
       AsyncFrozenLeaderWireIngressLaneRank,
       AsyncFrozenLeaderWireBarrierRankCarrier,
       AsyncFrozenLeaderWireBarrierTailRankCarrier,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
       AsyncFrozenLeaderWireIngressDependencyRankCarrier,
       AsyncFrozenLeaderWireIngressRankCarrier,
       AsyncFrozenLeaderWireIngressCapacitySelectorCarrier,
       AsyncFrozenLeaderWireIngressRunnerSelectorCarrier,
       AsyncFrozenLeaderWireIngressSelectorCarrier,
       AsyncFrozenLeaderWireIngressLaneCarrier,
       AsyncFrozenLeaderWirePhysicalRankCarrier,
       AsyncLeaderWirePhysicalIngressRank,
       AsyncLeaderWireEarlierPhysicalOwners,
       AsyncLeaderWireFrozenIngressPredecessorDebtSet,
       AsyncLeaderWireLifecycleIngressProtected,
       AsyncLeaderWirePotentialOwnerIdentity,
       IngressLane, IngressResourceSource, SequenceSet,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncIngressTypeInvariant, AsyncLeaderWireLifecycleTypeInvariant

THEOREM CandidateProducerContinuationFrozenCandidateCarrierHasConfiguredBound ==
  \A node \in ValidatorIds, targetOrdinal \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
      => /\ IsFiniteSet(
               AsyncCandidateProducerContinuationFrozenCandidateOwners(
                 node, targetOrdinal))
         /\ Cardinality(
              AsyncCandidateProducerContinuationFrozenCandidateOwners(
                node, targetOrdinal))
              <= AsyncCandidateProducerEpisodeCapacity
                   + AsyncCandidateProducerContinuationCapacity
                   + Cardinality(AsyncLeaderWireLifecycleSlotSet)
BY AsyncCausalEpisodeCandidateCarrierHasConfiguredBound,
   AsyncCandidateProducerContinuationsInjectIntoLifecycleStageOwners,
   CandidateProducerContinuationStrictLeaderWireCutMatchesLogicalBarrier,
   AsyncLeaderWireLifecycleSlotUniverseIsFinite,
   FS_Image, FS_Union, FS_Subset, FS_CardinalityType, IsaT(1200)
   DEF AsyncCandidateProducerContinuationFrozenCandidateOwners,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuations,
       AsyncCandidateProducerContinuationCapacity,
       AsyncLeaderWireRuntimeCandidate,
       AsyncLeaderWireLifecycleDormant,
       AsyncLeaderWireLifecycleActive,
       AsyncLeaderWireLifecycleSlotSet,
       AsyncCausalEpisodeCandidates,
       AsyncStrongTypeInvariant

THEOREM CandidateProducerContinuationDormantLocalReplayChargeCannotAppearAtGst ==
  \A node \in ValidatorIds, targetOrdinal \in Nat:
    /\ gst
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncNext
      => (AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates(
             node, targetOrdinal))'
           \subseteq
             AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates(
               node, targetOrdinal)
BY AsyncCandidateProducerContinuationGstExcludesResetReplay,
   AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck,
   AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor,
   AsyncCandidateProducerSemanticHandoffRetirementRequiresAck,
   AsyncCandidateProducerContinuationPreservedOrTerminal,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   IsaT(1800)
   DEF AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationConcreteSuccessorOwned,
       AsyncCandidateProducerContinuationHandoffOwned,
       AsyncCandidateProducerContinuationLocalReplayCarrier,
       AsyncCandidateProducerContinuationHandoffRetired,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuationStateAfterDeparture,
       AsyncCandidateProducerContinuationReservationAvailableIn,
       AsyncCandidateProducerContinuationAddressCanAdvanceIn,
       AsyncCandidateProducerContinuationRecordsForAddressIn,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateProducerContinuations,
       AsyncCausalEpisodeFrozenPredecessorOrigins,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       PreGstResponsiveRestart, PreGstResponsiveReplay

THEOREM CandidateProducerContinuationEqualOrdinalLeaderWireCoalescesTargetCell ==
  \A node \in ValidatorIds,
     targetOrdinal \in Nat \ {0},
     leaderRecord \in asyncLeaderWireLifecycles,
     continuation \in AsyncCandidateProducerContinuations:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ continuation.status \in {"Reserved", "Materialized"}
    /\ continuation.node = node
    /\ continuation.ordinal = targetOrdinal
    /\ leaderRecord.recipient = node
    /\ leaderRecord.schedulerOrdinal = targetOrdinal
      => /\ leaderRecord.causalOrigin = continuation.causalOrigin
         /\ AsyncCandidateServiceIdentity(
              AsyncLeaderWireRuntimeCandidate(leaderRecord.item))
              = continuation.identity
         /\ CandidateAdmissionCoalesced(
              AsyncLeaderWireRuntimeCandidate(leaderRecord.item))
BY Isa
   DEF AsyncCandidateServiceLifecycleInvariant,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncLeaderWireLifecycleTypeInvariant,
       AsyncLeaderWireLifecycleSharedOrdinalInvariant,
       AsyncControlServiceStateTypeInvariant,
       AsyncCandidateProducerContinuationLifecycleCoverageInvariant,
       AsyncCandidateProducerContinuationLifecycleCoverageInvariantIn,
       AsyncCandidateProducerContinuationLifecycleCoveredIn,
       AsyncCandidateLifecycleAdmissions,
       AsyncLeaderWireContinuationSharedOrdinalNoCollisionInvariant,
       CandidateAdmissionCoalesced,
       AsyncCandidateProducerContinuationBlocks,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuations

THEOREM CandidateProducerContinuationFrozenLeaderWireChargeCannotAppearAtGst ==
  \A node \in ValidatorIds, targetOrdinal \in Nat:
    /\ gst
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ targetOrdinal < AsyncNextCandidateLifecycleOrdinal(node)
    /\ [AsyncNext]_AsyncAllVars
      => (AsyncCandidateProducerContinuationFrozenLeaderWireCandidates(
             node, targetOrdinal))'
           \subseteq
             AsyncCandidateProducerContinuationFrozenLeaderWireCandidates(
               node, targetOrdinal)
BY AsyncSharedSchedulerHighWatermarkIsMonotone,
   CandidateProducerContinuationStrictLeaderWireCutMatchesLogicalBarrier,
   CandidateProducerContinuationEqualOrdinalLeaderWireCoalescesTargetCell,
   AdmitDormantLeaderWireRetainsLifecycleTokenAndFrozenPrefix,
   PostGstStepCannotCreateDormantLeaderWirePotential,
   RuntimeLeaderWireCannotRetireMerelyFromIngressPop,
   RetireLeaderWireLifecycleRetainsTerminalTombstone,
   FS_Image, FS_Subset, IsaT(2400)
   DEF AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncLeaderWireRuntimeCandidate,
       AsyncLeaderWireLifecycleDormant,
       AsyncLeaderWireLifecycleActive,
       AsyncLeaderWireLifecycleRecordAfterIngressDrain,
       AsyncLeaderWireLifecyclesAfterIngressDrain,
       AsyncLeaderWireLifecycleStateAfterIngressAdmission,
       AsyncLeaderWireLifecycleTransition,
       AsyncLeaderWireLifecycleIngressAdmissionTransition,
       AsyncLeaderWireLifecycleIngressDrainTransition,
       AsyncLeaderWireLifecycleConsumerTransition,
       AsyncLeaderWireLifecycleTerminalTransition,
       AsyncLeaderWireLifecycleRestartTransition,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       AsyncAllVars

THEOREM CandidateProducerContinuationExactLocalReplayReplacesFrozenCharge ==
  \A node \in ValidatorIds, targetOrdinal \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncNext
    /\ AsyncControlServiceSlotTransition
    /\ AsyncCandidateProducerContinuationExactLocalReplayStep(node)
      => (AsyncCandidateProducerContinuationFrozenCandidateOwners(
             node, targetOrdinal))'
           =
             AsyncCandidateProducerContinuationFrozenCandidateOwners(
               node, targetOrdinal)
BY AsyncCandidateProducerContinuationExactLocalReplayRetainsReservation,
   AsyncCandidateProducerContinuationExactLocalReplayPublishesStoredCarrier,
   AsyncNextPreservesCandidateProducerContinuationScheduledExclusion,
   IsaT(1800)
   DEF AsyncCandidateProducerContinuationFrozenCandidateOwners,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationConcreteSuccessorOwned,
       AsyncCandidateProducerContinuationHandoffOwned,
       AsyncCandidateProducerContinuationLocalReplayCarrier,
       AsyncCandidateProducerContinuationHandoffRetired,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateProducerContinuations,
       AsyncCausalEpisodeCandidates,
       AsyncCausalEpisodeFrozenPredecessorOrigins,
       AsyncControlServiceSlotTransition,
       CandidateScheduled, CandidateScheduledAfter

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
  /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
  /\ record =
       AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord(node)

THEOREM HistoricalCandidateProducerContinuationTurnIsResolutionOrExactReplay ==
  \A node \in ValidatorIds:
    /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
    /\ RunHistoricalRecoveryNode(node)
      => \/ /\ AsyncCandidateProducerContinuationRunnerResolutionReady(node)
               /\ ResolveRunNodeCandidateProducerContinuation(node)
               /\ UNCHANGED vars
               /\ UNCHANGED asyncCausalQueues
               /\ UNCHANGED
                    AsyncSchedulerExceptCausalControlAndNodeService
         \/ /\ ~AsyncCandidateProducerContinuationRunnerResolutionReady(node)
               /\ ReplayRunNodeCandidateProducerContinuation(node)
BY DEF RunHistoricalRecoveryNode, RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation

THEOREM HistoricalCandidateProducerContinuationNonreadyTurnUsesLocalReplay ==
  \A node \in ValidatorIds:
    /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
    /\ ~AsyncCandidateProducerContinuationRunnerResolutionReady(node)
    /\ RunHistoricalRecoveryNode(node)
      => /\ (AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord(
                 node)).sourceClass = "Local"
         /\ ReplayRunNodeCandidateProducerContinuation(node)
BY HistoricalCandidateProducerContinuationTurnIsResolutionOrExactReplay,
   Isa
   DEF ReplayRunNodeCandidateProducerContinuation

THEOREM HistoricalCandidateProducerContinuationLocalReplayTurnApproachesReady ==
  \A node \in ValidatorIds:
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncControlServiceSlotTransition
    /\ AsyncCandidateProducerContinuationRunnerResolutionRequired(node)
    /\ ~AsyncCandidateProducerContinuationRunnerResolutionReady(node)
    /\ RunHistoricalRecoveryNode(node)
      => \/ (AsyncCandidateProducerContinuationRunnerResolutionReady(node))'
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
       AsyncCandidateProducerContinuationRunnerResolutionRequired,
       AsyncCandidateProducerContinuationRunnerResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRunnerSelectedResolutionRecord,
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
    /\ AsyncCandidateProducerContinuationRunnerResolutionReady(node)
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
       AsyncCandidateProducerContinuationFrozenCandidateOwners,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
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
   CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight,
   CandidateProducerContinuationDormantLocalReplayChargeCannotAppearAtGst,
   CandidateProducerContinuationStrictLeaderWireCutMatchesLogicalBarrier,
   CandidateProducerContinuationEqualOrdinalLeaderWireCoalescesTargetCell,
   CandidateProducerContinuationFrozenLeaderWireChargeCannotAppearAtGst,
   CandidateProducerContinuationExactLocalReplayReplacesFrozenCharge,
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
       AsyncCandidateProducerContinuationFrozenCandidateOwners,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
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

THEOREM CandidateProducerContinuationFrozenSourcePrefixStepCannotReplenish ==
  \A node \in ValidatorIds,
     physicalCut \in Nat,
     episodeSchedulerCeiling \in Nat,
     frozenCandidateOrigins \in SUBSET AsyncCandidateCausalOriginSet,
     frozenServeSources \in SUBSET AsyncServeIngressSourceSet,
     frozenContinuationSources \in SUBSET AsyncCandidateLifecycleSourceSet,
     frozenLeaderWireIdentities
       \in SUBSET AsyncLeaderWirePotentialOwnerIdentitySet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncCertifiedResponseClaimInvariant
    /\ AsyncCertifiedResponseClaimFrozenSourceInvariant
    /\ gst
    /\ physicalCut <= AsyncNextIngressPhysicalOrdinal(node)
    /\ episodeSchedulerCeiling
         <= AsyncNextCandidateLifecycleOrdinal(node)
    /\ IsFiniteSet(frozenCandidateOrigins)
    /\ IsFiniteSet(frozenServeSources)
    /\ IsFiniteSet(frozenContinuationSources)
    /\ IsFiniteSet(frozenLeaderWireIdentities)
    /\ \A source \in frozenServeSources:
         /\ source.ordinal < physicalCut
         /\ source.schedulerOrdinal < episodeSchedulerCeiling
         /\ \/ source.lifecycleOrdinal = 0
            \/ source.lifecycleOrdinal
                 < asyncNextServeAdmissionOrdinal[node]
    /\ \A source \in frozenContinuationSources:
         /\ source.origin \in frozenCandidateOrigins
         /\ source.ordinal < episodeSchedulerCeiling
    /\ AsyncTimeoutLifecycleOwned(node)
         => AsyncCandidateLifecycleSource(
              AsyncTimeoutLifecycleOrigin(node),
              AsyncTimeoutLifecycleOrdinal(node))
              \notin frozenContinuationSources
    /\ \A record \in asyncLeaderWireLifecycles:
         /\ record.recipient = node
         /\ AsyncLeaderWireLifecycleActive(record)
         /\ record.physicalAdmissionOrdinal < physicalCut
           => /\ AsyncLeaderWirePotentialOwnerIdentity(record)
                    \in frozenLeaderWireIdentities
              /\ record.causalOrigin \in frozenCandidateOrigins
              /\ AsyncCandidateLifecycleSource(
                   record.causalOrigin, record.schedulerOrdinal)
                    \in frozenContinuationSources
    /\ \A owner \in frozenLeaderWireIdentities:
         \A record \in asyncLeaderWireLifecycles:
           AsyncLeaderWirePotentialOwnerIdentity(record) = owner
             => ~AsyncLeaderWireLifecycleDormant(record)
    /\ [AsyncNext]_AsyncAllVars
    /\ \E claimRecord \in CertifiedResponseClaimRecordsAt(node):
         /\ claimRecord \in CertifiedResponseClaimRecordsAt(node)'
         /\ claimRecord.physicalCut = physicalCut
         /\ claimRecord.episodeSchedulerCeiling =
              episodeSchedulerCeiling
         /\ claimRecord.frozenCandidateOrigins =
              frozenCandidateOrigins
         /\ claimRecord.frozenServeSources = frozenServeSources
         /\ claimRecord.frozenContinuationSources =
              frozenContinuationSources
         /\ claimRecord.frozenLeaderWireIdentities =
              frozenLeaderWireIdentities
    /\ AsyncCertifiedResponseFrozenLeaderWireStageBudget(
         node, physicalCut, frozenLeaderWireIdentities)'
         =
           AsyncCertifiedResponseFrozenLeaderWireStageBudget(
             node, physicalCut, frozenLeaderWireIdentities)
      => \/ <<AsyncCandidateProducerContinuationFrozenSourcePrefixRank(
                  node, frozenCandidateOrigins, frozenServeSources,
                  frozenContinuationSources)',
               AsyncCandidateProducerContinuationFrozenSourcePrefixRank(
                  node, frozenCandidateOrigins, frozenServeSources,
                  frozenContinuationSources)>>
              \in
                AsyncCandidateProducerContinuationFrozenPrefixRankOrdering
         \/ AsyncCandidateProducerContinuationFrozenSourcePrefixRank(
              node, frozenCandidateOrigins, frozenServeSources,
              frozenContinuationSources)'
              =
                AsyncCandidateProducerContinuationFrozenSourcePrefixRank(
                  node, frozenCandidateOrigins, frozenServeSources,
                  frozenContinuationSources)
BY CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight,
   CandidateProducerContinuationDormantLocalReplayChargeCannotAppearAtGst,
   CandidateProducerContinuationExactLocalReplayReplacesFrozenCharge,
   ClaimedResponseCapacityCreatesPrioritySource,
   PrioritySourceSelectsClaimedResponse,
   DrainFairIngressSelectedClaimPopShape,
   AsyncFrozenServeSourceCannotResurrectAtGst,
   AsyncFreshServeIngressCannotReacquirePriorSchedulerOrdinal,
   AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck,
   AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor,
   AsyncCandidateProducerSemanticHandoffRetirementRequiresAck,
   FS_CardinalityType, FS_Subset, IsaT(3600)
   DEF AsyncCandidateProducerContinuationFrozenSourcePrefixRank,
       AsyncCandidateProducerContinuationFrozenSourceProducerBudget,
       AsyncCandidateProducerContinuationFrozenSourceProducerTokens,
       AsyncCandidateProducerContinuationFrozenSourceCandidateTokens,
       AsyncCandidateProducerContinuationFrozenSourceCandidateOwners,
       AsyncCandidateProducerContinuationFrozenSourceScheduledCandidates,
       AsyncCandidateProducerContinuationFrozenSourceDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenSourceStatusTokens,
       AsyncCandidateProducerContinuationFrozenSourceRecords,
       AsyncCandidateProducerContinuationCausalWeight,
       AsyncFrozenServeWorkBudget,
       AsyncFrozenServeWorkTokens,
       AsyncFrozenServeOccurrenceTokens,
       AsyncFrozenServeIngressPrefixTokens,
       AsyncFrozenServeIoPredecessorTokens,
       AsyncFrozenServeReachDebt,
       AsyncFrozenServeAdmissionSources,
       AsyncFrozenServeExactIngressSources,
       AsyncFrozenServeLifecycleSources,
       AsyncServeIngressSourceFor,
       AsyncCertifiedResponseFrozenLeaderWireStageBudget,
       AsyncCertifiedResponseFrozenLeaderWireStageTokens,
       AsyncCertifiedResponseFrozenLeaderWireRecords,
       AsyncFrozenLeaderWireBarrierRemainingStage,
       AsyncLeaderWireLifecycleActive,
       AsyncLeaderWirePotentialOwnerIdentity,
       AsyncCandidateLifecycleSource,
       AsyncCertifiedResponseClaimFrozenSourceInvariant,
       CertifiedResponseClaimRecordsAt,
       CertifiedResponseClaimsAt,
       CertifiedResponseClaimMatches,
       CanEnqueueCertifiedResponse,
       DrainableClaimedResponseReadyIndices,
       SelectedIngressItemAt,
       FirstDrainableIngressIndex,
       DrainFairIngressSelected,
       AsyncTimeoutLifecycleOwned,
       AsyncTimeoutLifecycleOrigin,
       AsyncTimeoutLifecycleOrdinal,
       AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
       AsyncCausalEpisodeStructuralRankOrdering,
       AsyncCausalEpisodeStructuralRankCarrier,
       AsyncCausalEpisodeServeRankOrdering,
       AsyncCausalEpisodeServeRankCarrier,
       AsyncAllVars, SetLessThan, OpToRel

THEOREM CandidateProducerContinuationFrozenSourceFairResolutionStrictlyDescends ==
  \A node \in ValidatorIds,
     frozenCandidateOrigins \in SUBSET AsyncCandidateCausalOriginSet,
     frozenServeSources \in SUBSET AsyncServeIngressSourceSet,
     frozenContinuationSources \in SUBSET AsyncCandidateLifecycleSourceSet:
    LET selected ==
          AsyncCandidateProducerContinuationSelectedResolutionRecord(node)
        before ==
          AsyncCandidateProducerContinuationFrozenSourcePrefixRank(
            node, frozenCandidateOrigins, frozenServeSources,
            frozenContinuationSources)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ AsyncCandidateServiceLifecycleInvariant
       /\ AsyncCandidateProducerContinuationResolutionRequired(node)
       /\ selected.causalOrigin \in frozenCandidateOrigins
       /\ AsyncCandidateProducerContinuationLifecycleSource(selected)
            \in frozenContinuationSources
       /\ \/ PostGstResolveLocalCandidateProducerContinuation(node)
          \/ PostGstServiceConditionalTransportProducerContinuation(node)
          \/ PostGstServiceVolatileBodyProducerContinuation(node)
       => <<AsyncCandidateProducerContinuationFrozenSourcePrefixRank(
                node, frozenCandidateOrigins, frozenServeSources,
                frozenContinuationSources)',
              before>>
            \in
              AsyncCandidateProducerContinuationFrozenPrefixRankOrdering
BY ExternalContinuationFairServiceStrictlyDropsStatusRank,
   LocalContinuationFairResolutionStrictlyDropsStatusRank,
   CandidateProducerContinuationResolutionSelectsMinimumFrozenOwner,
   FS_CardinalityType, IsaT(2400)
   DEF AsyncCandidateProducerContinuationFrozenSourcePrefixRank,
       AsyncCandidateProducerContinuationFrozenSourceProducerBudget,
       AsyncCandidateProducerContinuationFrozenSourceProducerTokens,
       AsyncCandidateProducerContinuationFrozenSourceCandidateTokens,
       AsyncCandidateProducerContinuationFrozenSourceCandidateOwners,
       AsyncCandidateProducerContinuationFrozenSourceDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenSourceStatusTokens,
       AsyncCandidateProducerContinuationFrozenSourceRecords,
       AsyncCandidateProducerContinuationCausalWeight,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationResolutionPredecessorsFor,
       AsyncCandidateProducerContinuationSelectedResolutionRecord,
       AsyncCandidateProducerContinuationStatusRank,
       AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
       AsyncCausalEpisodeStructuralRankOrdering,
       AsyncCausalEpisodeServeRankOrdering,
       LexPairOrdering, SetLessThan, OpToRel

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
       AsyncCandidateProducerContinuationFrozenCandidateOwners,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
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
       AsyncCandidateProducerContinuationRestartStableTerminalIn,
       AsyncCandidateProducerContinuationLifecycleCoveredIn

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
