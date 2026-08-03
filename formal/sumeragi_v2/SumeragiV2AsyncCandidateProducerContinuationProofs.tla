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
    /\ record.identity = AsyncCandidateServiceIdentity(record.candidate)
    /\ record.causalOrigin = record.candidate.causalOrigin

THEOREM AsyncCandidateProducerContinuationConstructorIsTyped ==
  \A candidate \in AsyncCandidateSet,
     handoffCandidates \in SUBSET AsyncCandidateSet,
     lifecycleSlot \in AsyncCandidateLifecycleSlots,
     ordinal \in Nat \ {0},
     sourcePhysicalOrdinal \in Nat,
     physicalCut \in Nat,
     status \in AsyncCandidateProducerContinuationStatuses:
    candidate.kind \in AsyncCandidateServiceTrackedKinds
      => /\ AsyncCandidateProducerContinuationRecord(
               candidate, handoffCandidates,
               lifecycleSlot, ordinal, sourcePhysicalOrdinal,
               physicalCut, status)
             \in AsyncCandidateProducerContinuationRecordSet
         /\ (AsyncCandidateProducerContinuationRecord(
                candidate, handoffCandidates,
                lifecycleSlot, ordinal, sourcePhysicalOrdinal,
                physicalCut, status)).sourceClass
              = AsyncCandidateProducerContinuationSourceClass(candidate)
         /\ (AsyncCandidateProducerContinuationRecord(
                candidate, handoffCandidates,
                lifecycleSlot, ordinal, sourcePhysicalOrdinal,
                physicalCut, status)).address
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
               AsyncCandidateProducerContinuationSourcePhysicalOrdinalIn(
                 state, candidate),
               AsyncCandidateProducerContinuationPhysicalCutIn(
                 state, candidate),
               AsyncCandidateProducerContinuationInitialStatusAfter(
                 candidate))
           next ==
             AsyncCandidateProducerContinuationStateAfterDeparture(
               state, candidate)
       IN /\ installed \in next.producerContinuations
          /\ installed.identity =
               AsyncCandidateServiceIdentity(candidate)
          /\ installed.causalOrigin = candidate.causalOrigin
          /\ installed.sourcePhysicalOrdinal =
               AsyncCandidateProducerContinuationSourcePhysicalOrdinalIn(
                 state, candidate)
          /\ installed.physicalCut =
               AsyncCandidateProducerContinuationPhysicalCutIn(
                 state, candidate)
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
       AsyncCandidateProducerContinuationSourcePhysicalOrdinalIn,
       AsyncCandidateProducerContinuationPhysicalCutIn,
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
  \A state:
    \A record \in AsyncCandidateProducerContinuationRecordSet:
    record.status = "Terminal"
      => AsyncCandidateProducerContinuationRecordAfterStep(
           state, record) = record
BY SMT DEF AsyncCandidateProducerContinuationRecordAfterStep

THEOREM AsyncCandidateProducerContinuationStatusIsMonotone ==
  \A state:
    \A record \in AsyncCandidateProducerContinuationRecordSet:
    AsyncCandidateProducerContinuationStatusRank(
      (AsyncCandidateProducerContinuationRecordAfterStep(
         state, record)).status)
      <= AsyncCandidateProducerContinuationStatusRank(record.status)
BY SMT
   DEF AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuationStatusRank

THEOREM AsyncCandidateProducerContinuationResolvedReservedRankStrictlyDrops ==
  \A state:
    \A record \in AsyncCandidateProducerContinuationRecordSet:
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
  \A state:
    \A record \in AsyncCandidateProducerContinuationRecordSet:
    /\ AsyncCandidateProducerContinuationSelectedForResolution(record)
    /\ record.status = "Materialized"
      => (AsyncCandidateProducerContinuationRecordAfterStep(
            state, record)).status = "Terminal"
BY SMT DEF AsyncCandidateProducerContinuationRecordAfterStep

THEOREM AsyncCandidateProducerContinuationUnselectedActiveRecordIsFixed ==
  \A state:
    \A record \in AsyncCandidateProducerContinuationRecordSet:
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
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateProducerContinuationResolutionRequired(node)
      => LET selected ==
               AsyncCandidateProducerContinuationSelectedResolutionRecord(
                 node)
         IN /\ selected
                  \in
                    AsyncCandidateProducerContinuationPhysicallyEligibleResolutionRecordsForNode(
                      node)
            /\ selected
                  \in
                    AsyncCandidateProducerContinuationResolutionRecordsForNode(
                      node)
            /\ AsyncCandidateProducerContinuationResolutionPredecessorsFor(
                 node, selected) = {}
BY AsyncStrongTypeProjectsControlServiceStateType,
   AsyncCandidateProducerContinuationResolutionSelectionIsLogicalMinimum, Isa
   DEF AsyncCandidateProducerContinuationPhysicallyEligibleResolutionRecordsForNode

THEOREM ExternalCandidateProducerContinuationSelectionIsReady ==
  \A node \in ValidatorIds:
    /\ AsyncStrongTypeInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationResolutionRequired(node)
    /\ (AsyncCandidateProducerContinuationSelectedResolutionRecord(node))
         .sourceClass \in {"ConditionalTransport", "VolatileBody"}
      => AsyncCandidateProducerContinuationResolutionReady(node)
BY CandidateProducerContinuationResolutionSelectsMinimumFrozenOwner, Isa
   DEF AsyncStrongTypeInvariant,
       AsyncCandidateProducerContinuationExternalCoverageInvariant,
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
  \A initialContext \in ContextRecords,
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
  \A initialContext \in ContextRecords,
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
  \A initialContext \in ContextRecords,
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
AsyncCandidateProducerContinuationTargetPhysicalCut(
    node, targetOrdinal) ==
  LET targets ==
        {record \in
           AsyncCandidateProducerContinuationResolutionRecordsForNode(node):
           record.ordinal = targetOrdinal}
  IN IF targets = {}
     THEN AsyncCausalEpisodeTargetPhysicalCut(node, targetOrdinal)
     ELSE (CHOOSE record \in targets: TRUE).physicalCut

AsyncCandidateProducerContinuationFrozenPredecessorOrigins(
    node, targetOrdinal) ==
  LET targetPhysicalCut ==
        AsyncCandidateProducerContinuationTargetPhysicalCut(
          node, targetOrdinal)
  IN {record.origin:
        record \in
          {admission \in AsyncCandidateLifecycleAdmissions:
             /\ admission.node = node
             /\ admission.ordinal <= targetOrdinal
             /\ admission.sourcePhysicalOrdinal < targetPhysicalCut}}

AsyncCandidateProducerContinuationFrozenCausalCandidates(
    node, targetOrdinal) ==
  {candidate \in AsyncCausalEpisodeCandidates(node, targetOrdinal):
     candidate.causalOrigin
       \in AsyncCandidateProducerContinuationFrozenPredecessorOrigins(
            node, targetOrdinal)}

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
\* exact carrier is already Active below a rigid physical admission cut.
\* Dormant records never enter this set: an exact retry admitted after the
\* cut receives the cut itself and is physically behind the frozen prefix.
\* Ingress-to-Runtime is a carrier transfer of the same set element.
\* Fresh identities receive the current shared high-watermark and cannot enter
\* an older target cut; terminal identities contribute no latent candidate.
\* The strict boundary deliberately omits the target ordinal itself: an
\* equal-ordinal leader-wire/continuation pair is one shared lifecycle cell and
\* its Runtime publication coalesces against the target continuation identity.
AsyncCandidateProducerContinuationFrozenLeaderWireCandidates(
    node, targetOrdinal, physicalCut) ==
  {AsyncLeaderWireRuntimeCandidate(record.item):
     record \in
       {owned \in asyncLeaderWireLifecycles:
          /\ owned.recipient = node
          /\ owned.schedulerOrdinal < targetOrdinal
          /\ AsyncLeaderWireLifecycleActive(owned)
          /\ owned.physicalAdmissionOrdinal < physicalCut}}

AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates(
    node, targetOrdinal) ==
  LET targetPhysicalCut ==
        AsyncCandidateProducerContinuationTargetPhysicalCut(
          node, targetOrdinal)
  IN {DeliveryCandidate(carrier.item):
        carrier \in
          {owned \in
             asyncControlServiceState.ordinaryIngressCarrierEvidence:
             /\ owned.node = node
             /\ owned.status = "Ingress"
             /\ owned.schedulerOrdinal < targetOrdinal
             /\ owned.physicalOrdinal < targetPhysicalCut}}

AsyncCandidateProducerContinuationFrozenCandidateOwners(
    node, targetOrdinal) ==
  AsyncCandidateProducerContinuationFrozenCausalCandidates(
    node, targetOrdinal)
    \cup
      AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates(
        node, targetOrdinal)
    \cup
      AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates(
        node, targetOrdinal)
    \cup
      AsyncCandidateProducerContinuationFrozenLeaderWireCandidates(
        node, targetOrdinal,
        AsyncCandidateProducerContinuationTargetPhysicalCut(
          node, targetOrdinal))

AsyncCandidateProducerContinuationFrozenServeIngressIdentities(
    node, targetOrdinal) ==
  LET targetPhysicalCut ==
        AsyncCandidateProducerContinuationTargetPhysicalCut(
          node, targetOrdinal)
  IN {identity \in
        AsyncCausalEpisodeServeIngressIdentities(node, targetOrdinal):
        AsyncServeIngressAdmissionOrdinal(node, identity)
          < targetPhysicalCut}

AsyncCandidateProducerContinuationFrozenServeWorkTokens(
    node, targetOrdinal) ==
  {token \in AsyncCausalEpisodeServeWorkTokens(node, targetOrdinal):
     token[2]
       \in AsyncCandidateProducerContinuationFrozenServeIngressIdentities(
            node, targetOrdinal)}

AsyncCandidateProducerContinuationFrozenServeWorkBudget(
    node, targetOrdinal) ==
  Cardinality(
    AsyncCandidateProducerContinuationFrozenServeWorkTokens(
      node, targetOrdinal))

AsyncCandidateProducerContinuationFrozenServeReachDebt(
    node, targetOrdinal) ==
  IF AsyncCandidateProducerContinuationFrozenServeIngressIdentities(
       node, targetOrdinal) = {}
  THEN 0
  ELSE DrainableIngressTurnReachRank(node)

THEOREM CandidateProducerContinuationPostCutCausalRootCannotEnterFrozenPrefix ==
  \A node \in ValidatorIds,
     targetOrdinal \in Nat \ {0},
     candidate \in AsyncCandidateSet:
    LET lifecycle ==
          AsyncCandidateLifecycleRecordFor(
            candidate.node, candidate.causalOrigin)
        targetPhysicalCut ==
          AsyncCandidateProducerContinuationTargetPhysicalCut(
            node, targetOrdinal)
    IN /\ AsyncControlServiceStateTypeInvariant
       /\ candidate.node = node
       /\ candidate \in AsyncCausalEpisodeCandidates(node, targetOrdinal)
       /\ AsyncCandidateLifecycleRecorded(
            candidate.node, candidate.causalOrigin)
       /\ lifecycle.sourcePhysicalOrdinal >= targetPhysicalCut
       => candidate
            \notin
              AsyncCandidateProducerContinuationFrozenCausalCandidates(
                node, targetOrdinal)
BY Isa
   DEF AsyncCandidateProducerContinuationFrozenCausalCandidates,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationTargetPhysicalCut,
       AsyncCandidateLifecycleRecorded,
       AsyncCandidateLifecycleRecordsFor,
       AsyncCandidateLifecycleRecordFor,
       AsyncControlServiceStateTypeInvariant

THEOREM CandidateProducerContinuationCausalSuccessorRetainsFrozenPhysicalClass ==
  \A node \in ValidatorIds,
     targetOrdinal \in Nat \ {0},
     candidate \in
       AsyncCandidateProducerContinuationFrozenCausalCandidates(
         node, targetOrdinal),
     successor \in SequenceSet(CommandSuccessors(candidate)):
    AsyncCandidateLifecycleRecorded(
      candidate.node, candidate.causalOrigin)
      => successor.causalOrigin
           \in AsyncCandidateProducerContinuationFrozenPredecessorOrigins(
                node, targetOrdinal)
BY CommandSuccessorsRetainCausalOrigin, Isa
   DEF AsyncCandidateProducerContinuationFrozenCausalCandidates,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateLifecycleRecorded,
       AsyncCandidateLifecycleRecordsFor,
       SequenceSet

THEOREM CandidateProducerContinuationPostCutServeCannotEnterFrozenPrefix ==
  \A node \in ValidatorIds,
     targetOrdinal \in Nat \ {0},
     identity \in AsyncCausalEpisodeServeIngressIdentities(
                   node, targetOrdinal):
    AsyncCandidateProducerContinuationTargetPhysicalCut(
      node, targetOrdinal)
      <= AsyncServeIngressAdmissionOrdinal(node, identity)
      => identity
           \notin
             AsyncCandidateProducerContinuationFrozenServeIngressIdentities(
               node, targetOrdinal)
BY Isa
   DEF AsyncCandidateProducerContinuationFrozenServeIngressIdentities

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
    <<AsyncCandidateProducerContinuationFrozenServeWorkBudget(
        node, targetOrdinal),
      AsyncCandidateProducerContinuationFrozenServeReachDebt(
        node, targetOrdinal)>>>>

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
      AsyncFrozenServeReachDebt(node, frozenServeSources)>>>>

(***************************************************************************
Frozen shared ingress barrier (leader-wire plus ordinary aggregate carriers).

Two callers share this rank but freeze different predecessor cuts:

  * a producer continuation has no physical carrier, so it freezes the
    receiver's physical high-watermark at rank entry.  Only already-Active
    records below that cut and below the logical scheduler cut are in its
    finite owner universe.  A Dormant retry admitted later receives an ordinal
    at the cut and is physically behind the target;
  * a claimed response already owns a physical carrier.  Only non-dormant
    records whose physical carrier was admitted before the claim are
    predecessors.  A dormant identity replayed later is physically behind the
    claim even when its retained logical scheduler ordinal is smaller.

The stage budget is deliberately the outer component.  Leader-wire
Ingress-to-Runtime and ordinary aggregate Ingress-to-Candidate transfer each
consume 1 -> 0 before the resulting Candidate can increase the inner producer
budget.  Carrier replacement is not advertised as semantic progress.  The
dependency tail then reuses the existing physical-owner/frozen-prefix pair and
spells out the mode, capacity, runner-reach, priority selector, lane, and
source components required to drain the selected physical occurrence.
***************************************************************************)

AsyncFrozenLeaderWireIngressRecords(
    node, logicalCutoff, physicalCut, barrierMode) ==
  {record \in
     AsyncFrozenLeaderWireBarrierRecords(
       node, logicalCutoff, physicalCut, barrierMode):
     AsyncLeaderWireLifecycleIngressProtected(record)}

AsyncFrozenOrdinaryIngressRecords(
    node, logicalCutoff, physicalCut, barrierMode) ==
  AsyncFrozenOrdinaryIngressBarrierRecords(
    node, logicalCutoff, physicalCut, barrierMode)

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

AsyncFrozenOrdinarySelectedIngressCarrier(
    node, logicalCutoff, physicalCut, barrierMode) ==
  CHOOSE carrier \in
    AsyncFrozenOrdinaryIngressRecords(
      node, logicalCutoff, physicalCut, barrierMode):
    \A other \in
         AsyncFrozenOrdinaryIngressRecords(
           node, logicalCutoff, physicalCut, barrierMode):
      carrier.physicalOrdinal <= other.physicalOrdinal

AsyncFrozenIngressBarrierSelectsOrdinary(
    node, logicalCutoff, physicalCut, barrierMode) ==
  LET ordinary ==
        AsyncFrozenOrdinaryIngressRecords(
          node, logicalCutoff, physicalCut, barrierMode)
      leader ==
        AsyncFrozenLeaderWireIngressRecords(
          node, logicalCutoff, physicalCut, barrierMode)
  IN /\ ordinary # {}
     /\ \/ leader = {}
        \/ (AsyncFrozenOrdinarySelectedIngressCarrier(
              node, logicalCutoff, physicalCut,
              barrierMode)).physicalOrdinal
             <
           (AsyncFrozenLeaderWireSelectedIngressRecord(
              node, logicalCutoff, physicalCut,
              barrierMode)).physicalAdmissionOrdinal

AsyncFrozenIngressBarrierSelectedItem(
    node, logicalCutoff, physicalCut, barrierMode) ==
  IF AsyncFrozenIngressBarrierSelectsOrdinary(
       node, logicalCutoff, physicalCut, barrierMode)
  THEN (AsyncFrozenOrdinarySelectedIngressCarrier(
          node, logicalCutoff, physicalCut, barrierMode)).item
  ELSE (AsyncFrozenLeaderWireSelectedIngressRecord(
          node, logicalCutoff, physicalCut, barrierMode)).item

AsyncFrozenIngressBarrierSelectedPhysicalRank(
    node, logicalCutoff, physicalCut, barrierMode) ==
  IF AsyncFrozenIngressBarrierSelectsOrdinary(
       node, logicalCutoff, physicalCut, barrierMode)
  THEN <<0, 0>>
  ELSE AsyncLeaderWirePhysicalIngressRank(
         AsyncFrozenLeaderWireSelectedIngressRecord(
           node, logicalCutoff, physicalCut, barrierMode))

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
  LET leaderRecords ==
        AsyncFrozenLeaderWireIngressRecords(
          node, logicalCutoff, physicalCut, barrierMode)
      ordinaryRecords ==
        AsyncFrozenOrdinaryIngressRecords(
          node, logicalCutoff, physicalCut, barrierMode)
  IN IF /\ leaderRecords = {}
        /\ ordinaryRecords = {}
     THEN CHOOSE rank \in
            AsyncFrozenLeaderWireIngressDependencyRankCarrier: TRUE
     ELSE LET item ==
                AsyncFrozenIngressBarrierSelectedItem(
                  node, logicalCutoff, physicalCut, barrierMode)
          IN <<AsyncFrozenIngressBarrierSelectedPhysicalRank(
                 node, logicalCutoff, physicalCut, barrierMode),
               AsyncFrozenLeaderWireIngressRank(node, item)>>

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
    node, targetOrdinal, physicalCut) ==
  AsyncFrozenLeaderWireBarrierRank(
    node, targetOrdinal, targetOrdinal - 1, physicalCut, "Logical")

(***************************************************************************
Protected-Candidate shared-ingress episode.

The generic Stage-3/4/6 runner residual may not yet own a producer
continuation record, so its immutable boundary comes from the Candidate
lifecycle admission itself.  The outer stage charges pre-cut leader-wire and
ordinary ingress carrier transfer.  The middle frozen prefix charges every
pre-cut Candidate, continuation status, and Serve lifecycle.  The final
dependency rank is the existing physical-owner plus
mode/capacity/runner/priority/lane/source path to the selected ingress
occurrence.  Latent leader-wire and ordinary carriers are already represented
by their exact future Candidate in the middle prefix, while the outer stage
separately records consumption of the physical transfer episode.
***************************************************************************)

AsyncProtectedCandidateIngressEpisodeTailRank(candidate) ==
  LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
      physicalCut ==
        AsyncCausalEpisodeTargetPhysicalCut(
          candidate.node, cutoffOrdinal)
  IN <<AsyncCandidateProducerContinuationFrozenPrefixRank(
          candidate.node, cutoffOrdinal),
       AsyncFrozenLeaderWireIngressDependencyRank(
         candidate.node, cutoffOrdinal - 1,
         physicalCut, "Logical")>>

AsyncProtectedCandidateIngressEpisodeTailCarrier ==
  AsyncCandidateProducerContinuationFrozenPrefixRankCarrier
    \X AsyncFrozenLeaderWireIngressDependencyRankCarrier

AsyncProtectedCandidateIngressEpisodeTailOrdering ==
  LexPairOrdering(
    AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
    AsyncFrozenLeaderWireIngressDependencyRankOrdering,
    AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
    AsyncFrozenLeaderWireIngressDependencyRankCarrier)

AsyncProtectedCandidateIngressEpisodeRank(candidate) ==
  LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
  IN <<AsyncCausalEpisodeFrozenIngressBarrierStageBudget(
          candidate.node, cutoffOrdinal),
       AsyncProtectedCandidateIngressEpisodeTailRank(candidate)>>

AsyncProtectedCandidateIngressEpisodeRankCarrier ==
  Nat \X AsyncProtectedCandidateIngressEpisodeTailCarrier

AsyncProtectedCandidateIngressEpisodeRankOrdering ==
  LexPairOrdering(
    OpToRel(<, Nat),
    AsyncProtectedCandidateIngressEpisodeTailOrdering,
    Nat, AsyncProtectedCandidateIngressEpisodeTailCarrier)

\* The fair owner is selected from the same physically frozen Serve prefix
\* that is charged by the rank.  Using the wider logical-only causal set here
\* would let a dormant post-cut carrier change Runner/I/O ownership while the
\* protected rank remained equal.
AsyncProtectedCandidateIoOwnerRequired(candidate) ==
  LET node == candidate.node
      cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
      identities ==
        AsyncCandidateProducerContinuationFrozenServeIngressIdentities(
          node, cutoffOrdinal)
  IN identities # {}
       /\ LET identity ==
                CHOOSE owned \in identities:
                  \A other \in identities:
                    AsyncServeIngressAdmissionOrdinal(node, owned)
                      <= AsyncServeIngressAdmissionOrdinal(node, other)
          IN /\ AsyncServeLiveReservationOwned(node, identity)
             /\ ~AsyncServeJobQueued(node, identity)
             /\ ~CanResumeExactServeCapacity(node, identity)

AsyncProtectedCandidateFairOwnerKinds == {"Runner", "IoWorker"}

AsyncProtectedCandidateFairOwner(candidate) ==
  IF AsyncProtectedCandidateIoOwnerRequired(candidate)
  THEN "IoWorker"
  ELSE "Runner"

AsyncProtectedCandidateFairAction(node, ownerKind) ==
  AsyncCausalEpisodeFairAction(node, ownerKind)

AsyncProtectedCandidateSelectedFairAction(candidate) ==
  AsyncProtectedCandidateFairAction(
    candidate.node, AsyncProtectedCandidateFairOwner(candidate))

THEOREM AsyncProtectedCandidateTargetPhysicalCutMatchesLifecycle ==
  \A candidate \in AsyncCandidateSet:
    LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
    IN /\ AsyncStrongTypeInvariant
       /\ ProtectedCandidateOwned(candidate)
       => AsyncCandidateProducerContinuationTargetPhysicalCut(
            candidate.node, cutoffOrdinal)
            = AsyncCausalEpisodeTargetPhysicalCut(
                candidate.node, cutoffOrdinal)
BY FS_CardinalityType, IsaT(900)
   DEF AsyncCandidateProducerContinuationTargetPhysicalCut,
       AsyncCausalEpisodeTargetPhysicalCut,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationLifecycleCoverageInvariant,
       AsyncCandidateProducerContinuationLifecycleCoverageInvariantIn,
       AsyncCandidateProducerContinuationLifecycleCoveredIn,
       AsyncCandidateLifecycleSchedulerCoverageInvariant,
       AsyncCandidateLifecycleActiveRecords,
       AsyncCandidateLifecycleRecordCoversScheduledOrigin,
       AsyncScheduledCandidateOriginsForNode,
       AsyncCandidateLifecycleOrdinal,
       AsyncCandidateLifecycleRecordsFor,
       ProtectedCandidateOwned, CandidateScheduled,
       AsyncStrongTypeInvariant, AsyncControlServiceStateTypeInvariant

THEOREM AsyncProtectedCandidateTargetPhysicalCutPersists ==
  \A candidate \in AsyncCandidateSet:
    LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
    IN /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ ProtectedCandidateOwned(candidate)
       /\ [AsyncNext]_AsyncAllVars
       /\ ProtectedCandidateOwned(candidate)'
       => (AsyncCandidateProducerContinuationTargetPhysicalCut(
              candidate.node, cutoffOrdinal))'
            = AsyncCandidateProducerContinuationTargetPhysicalCut(
                candidate.node, cutoffOrdinal)
BY AsyncCausalEpisodeTargetLifecycleOrdinalPersists,
   AsyncCausalEpisodeTargetPhysicalCutPersists,
   AsyncProtectedCandidateTargetPhysicalCutMatchesLifecycle,
   AsyncBracketNextPreservesStrongTypeInvariant,
   IsaT(600)
   DEF AsyncAllVars

THEOREM AsyncProtectedCandidateSelectedServeOwnerGeometryIsComplete ==
  \A candidate \in AsyncCandidateSet:
    LET node == candidate.node
        cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
        identities ==
          AsyncCandidateProducerContinuationFrozenServeIngressIdentities(
            node, cutoffOrdinal)
    IN identities # {}
         => LET identity ==
                  CHOOSE owned \in identities:
                    \A other \in identities:
                      AsyncServeIngressAdmissionOrdinal(node, owned)
                        <= AsyncServeIngressAdmissionOrdinal(node, other)
            IN \/ /\ AsyncServeJobQueued(node, identity)
                   /\ AsyncProtectedCandidateFairOwner(candidate) = "Runner"
               \/ /\ AsyncServeLiveReservationOwned(node, identity)
                   /\ ~AsyncServeJobQueued(node, identity)
                   /\ CanResumeExactServeCapacity(node, identity)
                   /\ AsyncProtectedCandidateFairOwner(candidate) = "Runner"
               \/ /\ AsyncServeLiveReservationOwned(node, identity)
                   /\ ~AsyncServeJobQueued(node, identity)
                   /\ ~CanResumeExactServeCapacity(node, identity)
                   /\ AsyncProtectedCandidateFairOwner(candidate) = "IoWorker"
               \/ /\ ~AsyncServeLiveReservationOwned(node, identity)
                   /\ AsyncProtectedCandidateFairOwner(candidate) = "Runner"
BY Isa
   DEF AsyncProtectedCandidateFairOwner,
       AsyncProtectedCandidateIoOwnerRequired

THEOREM AsyncProtectedCandidateSelectedOwnerIsConcreteAndEnabled ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateProducerContinuationExternalCoverageInvariant
    /\ AsyncCandidateProducerContinuationLocalReplayCapacityInvariant
    /\ gst
    /\ ResponsiveProtectedCandidateOwned(candidate)
      => /\ AsyncProtectedCandidateFairOwner(candidate)
               \in AsyncProtectedCandidateFairOwnerKinds
         /\ ENABLED
              <<AsyncProtectedCandidateSelectedFairAction(candidate)>>_AsyncAllVars
BY QueuedIoEnablesPostGstService,
   QueuedIoServiceIsNonstuttering,
   AsyncProtectedCandidateSelectedServeOwnerGeometryIsComplete,
   ResponsiveUnappliedRunNodeIsEnabled,
   EnabledRunNodeLiftsPostGst,
   ExpandENABLED, ENABLEDaxioms, IsaT(900)
   DEF AsyncProtectedCandidateSelectedFairAction,
       AsyncProtectedCandidateFairAction,
       AsyncProtectedCandidateFairOwner,
       AsyncProtectedCandidateFairOwnerKinds,
       AsyncProtectedCandidateIoOwnerRequired,
       AsyncCandidateProducerContinuationFrozenServeIngressIdentities,
       ResponsiveProtectedCandidateOwned, ProtectedCandidateOwned,
       AsyncCurrentResponsiveVoters,
       AsyncArchiveIoServiceNodes,
       AsyncCausalEpisodeFairAction,
       PostGstRunNode, RunNode, RunNodeWork,
       AsyncAllVars

THEOREM CandidateProducerContinuationStrictLeaderWireCutMatchesLogicalBarrier ==
  \A node \in ValidatorIds,
     targetOrdinal \in Nat \ {0},
     physicalCut \in Nat:
    AsyncStrongTypeInvariant
      => AsyncCandidateProducerContinuationFrozenLeaderWireCandidates(
           node, targetOrdinal, physicalCut)
           =
         {AsyncLeaderWireRuntimeCandidate(record.item):
            record \in
              AsyncFrozenLeaderWireBarrierRecords(
                node, targetOrdinal - 1, physicalCut, "Logical")}
BY Isa
   DEF AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncFrozenLeaderWireBarrierRecords,
       AsyncLeaderWireLifecycleDormant,
       AsyncLeaderWireLifecycleActive,
       AsyncStrongTypeInvariant, AsyncSchedulerTypeInvariant,
       AsyncTransportTypeInvariant,
       AsyncLeaderWireLifecycleTypeInvariant,
       AsyncLeaderWireLifecycleTyped

THEOREM CandidateProducerContinuationActionInertDormantHasZeroFrozenStage ==
  \A record \in asyncLeaderWireLifecycles,
     node \in ValidatorIds,
     logicalCutoff, physicalCut \in Nat,
     barrierMode \in AsyncFrozenLeaderWireBarrierModes:
    AsyncLeaderWireActionInertDormant(record)
      => /\ AsyncFrozenLeaderWireBarrierRemainingStage(
               record, barrierMode) = 0
         /\ record
              \notin
                AsyncFrozenLeaderWireBarrierRecords(
                  node, logicalCutoff, physicalCut, barrierMode)
BY Isa
   DEF AsyncFrozenLeaderWireBarrierRemainingStage,
       AsyncFrozenLeaderWireBarrierRecords,
       AsyncLeaderWireActionInertDormant,
       AsyncLeaderWireLifecycleActive,
       AsyncLeaderWireLifecycleDormant

THEOREM CandidateProducerContinuationPostCutAdmissionCannotEnterFrozenPrefix ==
  \A node \in ValidatorIds,
     targetOrdinal, physicalCut \in Nat,
     recipient \in ValidatorIds,
     source \in AsyncIngressSources:
    /\ physicalCut <= AsyncNextIngressPhysicalOrdinal(node)
    /\ recipient = node
    /\ AdmitHiddenPacket(recipient, source)
      => \A record \in asyncLeaderWireLifecycles':
           record.physicalAdmissionOrdinal >= physicalCut
             => record
                  \notin
                    AsyncFrozenLeaderWireBarrierRecords(
                      node, targetOrdinal, physicalCut, "Logical")'
BY AdmitHiddenPacketReservesFreshSharedPhysicalOrdinal,
   AsyncIngressPhysicalHighWatermarkIsMonotone, Isa
   DEF AsyncFrozenLeaderWireBarrierRecords,
       AsyncLeaderWireLifecycleActive

THEOREM CandidateProducerContinuationPostCutOrdinaryAdmissionCannotEnterFrozenPrefix ==
  \A node \in ValidatorIds,
     targetOrdinal, physicalCut \in Nat,
     recipient \in ValidatorIds,
     source \in AsyncIngressSources:
    /\ physicalCut <= AsyncNextIngressPhysicalOrdinal(node)
    /\ recipient = node
    /\ AdmitHiddenPacket(recipient, source)
      => \A carrier \in
           asyncControlServiceState'.ordinaryIngressCarrierEvidence:
           carrier.physicalOrdinal >= physicalCut
             => carrier
                  \notin
                    AsyncFrozenOrdinaryIngressBarrierRecords(
                      node, targetOrdinal, physicalCut, "Logical")'
BY AdmitHiddenPacketReservesFreshSharedPhysicalOrdinal,
   AsyncIngressPhysicalHighWatermarkIsMonotone, Isa
   DEF AsyncFrozenOrdinaryIngressBarrierRecords

THEOREM CandidateProducerContinuationDropPolicyRejectedIsFrozenPhysicalPrefixFrame ==
  \A node \in ValidatorIds,
     targetOrdinal, physicalCut \in Nat,
     recipient \in ValidatorIds,
     source \in AsyncIngressSources:
    DropPolicyRejectedHiddenPacket(recipient, source)
      => /\ (AsyncCandidateProducerContinuationFrozenLeaderWireCandidates(
                node, targetOrdinal, physicalCut))'
              = AsyncCandidateProducerContinuationFrozenLeaderWireCandidates(
                  node, targetOrdinal, physicalCut)
         /\ (AsyncFrozenLeaderWireBarrierStageTokens(
                node, targetOrdinal - 1, physicalCut, "Logical"))'
              = AsyncFrozenLeaderWireBarrierStageTokens(
                  node, targetOrdinal - 1, physicalCut, "Logical")
BY Isa
   DEF DropPolicyRejectedHiddenPacket,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncFrozenLeaderWireBarrierStageTokens,
       AsyncFrozenLeaderWireBarrierRecords,
       AsyncFrozenOrdinaryIngressBarrierRecords,
       AsyncFrozenLeaderWireBarrierRemainingStage

THEOREM CandidateProducerContinuationPreCutIngressToRuntimeConsumesBarrierStage ==
  \A node \in ValidatorIds,
     targetOrdinal \in Nat \ {0},
     physicalCut \in Nat,
     source \in AsyncIngressSources,
     laneIndex \in 1..Len(IngressLane(node, source)),
     record \in asyncLeaderWireLifecycles:
    LET item == IngressLane(node, source)[laneIndex]
    IN /\ AsyncStrongTypeInvariant
       /\ Len(asyncIngressReady[node]) > 0
       /\ source = asyncIngressReady[node][1]
       /\ record
            \in AsyncFrozenLeaderWireBarrierRecords(
                 node, targetOrdinal - 1, physicalCut, "Logical")
       /\ AsyncLeaderWireLifecycleIngressProtected(record)
       /\ AsyncLeaderWireAdmissionMatchesRecord(item, record)
       /\ AsyncLeaderWireDrainInstallsRuntimeOwner(item)
       /\ PopSelectedIngress(node, 1, laneIndex)
       /\ AsyncControlServiceSlotTransition
       => /\ AsyncLeaderWireRuntimeCandidate(item)
                \in
                  AsyncCandidateProducerContinuationFrozenLeaderWireCandidates(
                    node, targetOrdinal, physicalCut)'
          /\ AsyncFrozenLeaderWireBarrierStageBudget(
               node, targetOrdinal - 1, physicalCut, "Logical")'
               < AsyncFrozenLeaderWireBarrierStageBudget(
                   node, targetOrdinal - 1, physicalCut, "Logical")
BY LeaderWireIngressDrainNeverInventsRuntimeOwner,
   FS_CardinalityType, FS_Subset, IsaT(1800)
   DEF AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncFrozenLeaderWireBarrierRecords,
       AsyncFrozenLeaderWireBarrierStageBudget,
       AsyncFrozenLeaderWireBarrierStageTokens,
       AsyncFrozenOrdinaryIngressBarrierRecords,
       AsyncFrozenLeaderWireBarrierRemainingStage,
       AsyncOrdinaryIngressCarrierStateAfterTransition,
       AsyncOrdinaryIngressCarrierEvidenceAfterPhysicalTransition,
       AsyncOrdinaryIngressCarrierAfterPhysicalTransition,
       AsyncLeaderWireLifecycleRecordAfterIngressDrain,
       AsyncLeaderWireLifecyclesAfterIngressDrain,
       PopSelectedIngress, IngressLane

THEOREM CandidateProducerContinuationPreCutOrdinaryIngressConsumesBarrierStage ==
  \A node \in ValidatorIds,
     targetOrdinal \in Nat \ {0},
     physicalCut \in Nat,
     source \in AsyncIngressSources,
     laneIndex \in 1..Len(IngressLane(node, source)),
     carrier \in
       asyncControlServiceState.ordinaryIngressCarrierEvidence:
    LET item == IngressLane(node, source)[laneIndex]
    IN /\ AsyncStrongTypeInvariant
       /\ Len(asyncIngressReady[node]) > 0
       /\ source = asyncIngressReady[node][1]
       /\ carrier
            \in AsyncFrozenOrdinaryIngressBarrierRecords(
                 node, targetOrdinal - 1, physicalCut, "Logical")
       /\ ExactAsyncCandidateIdentity(DeliveryCandidate(item))
            = carrier.carrierIdentity
       /\ PopSelectedIngress(node, 1, laneIndex)
       /\ AsyncControlServiceSlotTransition
       => /\ carrier
                \notin
                  AsyncFrozenOrdinaryIngressBarrierRecords(
                    node, targetOrdinal - 1,
                    physicalCut, "Logical")'
          /\ AsyncFrozenLeaderWireBarrierStageBudget(
               node, targetOrdinal - 1, physicalCut, "Logical")'
               < AsyncFrozenLeaderWireBarrierStageBudget(
                   node, targetOrdinal - 1, physicalCut, "Logical")
BY OrdinaryIngressDrainFreezesContinuationPhysicalCut,
   CandidateProducerContinuationPostCutOrdinaryAdmissionCannotEnterFrozenPrefix,
   FS_CardinalityType, FS_Subset, IsaT(2400)
   DEF AsyncFrozenLeaderWireBarrierStageBudget,
       AsyncFrozenLeaderWireBarrierStageTokens,
       AsyncFrozenLeaderWireBarrierRecords,
       AsyncFrozenOrdinaryIngressBarrierRecords,
       AsyncFrozenLeaderWireBarrierRemainingStage,
       AsyncControlServiceSlotTransition,
       AsyncOrdinaryIngressCarrierStateAfterTransition,
       AsyncOrdinaryIngressCarrierEvidenceAfterPhysicalTransition,
       AsyncOrdinaryIngressCarrierAfterPhysicalTransition,
       AsyncOrdinaryIngressCarrierStillPhysicalAfter,
       PopSelectedIngress, IngressLane

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
              AsyncFrozenOrdinaryIngressBarrierRecords(
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
       AsyncFrozenOrdinaryIngressBarrierRecords,
       AsyncFrozenLeaderWireBarrierRemainingStage,
       AsyncFrozenLeaderWireBarrierStageTokens,
       AsyncFrozenLeaderWireBarrierStageBudget,
       AsyncFrozenLeaderWireIngressRecords,
       AsyncFrozenOrdinaryIngressRecords,
       AsyncFrozenLeaderWireSelectedIngressRecord,
       AsyncFrozenOrdinarySelectedIngressCarrier,
       AsyncFrozenIngressBarrierSelectsOrdinary,
       AsyncFrozenIngressBarrierSelectedItem,
       AsyncFrozenIngressBarrierSelectedPhysicalRank,
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
       AsyncCandidateProducerContinuationFrozenCausalCandidates,
       AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates,
       AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationTargetPhysicalCut,
       AsyncCandidateProducerContinuationFrozenServeIngressIdentities,
       AsyncCandidateProducerContinuationFrozenServeWorkTokens,
       AsyncCandidateProducerContinuationFrozenServeWorkBudget,
       AsyncCandidateProducerContinuationFrozenServeReachDebt,
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

THEOREM AsyncProtectedCandidateIngressEpisodeRankOrderingIsWellFounded ==
  IsWellFoundedOn(
    AsyncProtectedCandidateIngressEpisodeRankOrdering,
    AsyncProtectedCandidateIngressEpisodeRankCarrier)
BY CandidateProducerContinuationFrozenPrefixRankOrderingIsWellFounded,
   AsyncFrozenLeaderWireIngressDependencyOrderingIsWellFounded,
   NatLessThanWellFounded, WFLexPairOrdering
   DEF AsyncProtectedCandidateIngressEpisodeRankOrdering,
       AsyncProtectedCandidateIngressEpisodeRankCarrier,
       AsyncProtectedCandidateIngressEpisodeTailOrdering,
       AsyncProtectedCandidateIngressEpisodeTailCarrier

THEOREM AsyncProtectedCandidateIngressEpisodeRankIsFinite ==
  \A candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ ProtectedCandidateOwned(candidate)
      => AsyncProtectedCandidateIngressEpisodeRank(candidate)
           \in AsyncProtectedCandidateIngressEpisodeRankCarrier
BY AsyncFrozenLeaderWireBarrierRankIsFinite,
   FS_CardinalityType, IsaT(900)
   DEF AsyncProtectedCandidateIngressEpisodeRank,
       AsyncProtectedCandidateIngressEpisodeRankCarrier,
       AsyncProtectedCandidateIngressEpisodeTailRank,
       AsyncProtectedCandidateIngressEpisodeTailCarrier,
       AsyncCandidateProducerContinuationFrozenPrefixRank,
       AsyncCandidateProducerContinuationFrozenPrefixRankCarrier,
       AsyncCausalEpisodeFrozenIngressBarrierStageBudget,
       AsyncCausalEpisodeTargetPhysicalCut,
       AsyncFrozenLeaderWireBarrierRank,
       AsyncFrozenLeaderWireBarrierTailRank

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
                   + AsyncOrdinaryIngressCarrierEvidenceCapacity
BY AsyncCausalEpisodeCandidateCarrierHasConfiguredBound,
   AsyncCandidateProducerContinuationsInjectIntoLifecycleStageOwners,
   FS_Image, FS_Union, FS_Subset, FS_CardinalityType, IsaT(1200)
   DEF AsyncCandidateProducerContinuationFrozenCandidateOwners,
       AsyncCandidateProducerContinuationFrozenCausalCandidates,
       AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates,
       AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationTargetPhysicalCut,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuations,
       AsyncCandidateProducerContinuationCapacity,
       AsyncCausalEpisodeCandidates,
       AsyncStrongTypeInvariant

THEOREM CandidateProducerContinuationDormantLocalReplayReplacementConsumesFrozenCausalCharge ==
  \A node \in ValidatorIds, targetOrdinal \in Nat:
    /\ gst
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ (AsyncCandidateProducerContinuationTargetPhysicalCut(
           node, targetOrdinal))'
         = AsyncCandidateProducerContinuationTargetPhysicalCut(
             node, targetOrdinal)
    /\ AsyncNext
      => ((AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates(
              node, targetOrdinal))'
            \ AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates(
                node, targetOrdinal))
           \subseteq
             AsyncCandidateProducerContinuationFrozenCausalCandidates(
               node, targetOrdinal)
BY AsyncCandidateProducerContinuationGstExcludesResetReplay,
   AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck,
   AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor,
   AsyncCandidateProducerSemanticHandoffRetirementRequiresAck,
   AsyncCandidateProducerContinuationPreservedOrTerminal,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   IsaT(1800)
   DEF AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenCausalCandidates,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationTargetPhysicalCut,
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
  \A node \in ValidatorIds,
     targetOrdinal \in Nat,
     physicalCut \in Nat:
    /\ gst
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ targetOrdinal <= AsyncNextCandidateLifecycleOrdinal(node)
    /\ physicalCut <= AsyncNextIngressPhysicalOrdinal(node)
    /\ [AsyncNext]_AsyncAllVars
      => (AsyncCandidateProducerContinuationFrozenLeaderWireCandidates(
             node, targetOrdinal, physicalCut))'
           \subseteq
             AsyncCandidateProducerContinuationFrozenLeaderWireCandidates(
               node, targetOrdinal, physicalCut)
BY AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncIngressPhysicalHighWatermarkIsMonotone,
   CandidateProducerContinuationStrictLeaderWireCutMatchesLogicalBarrier,
   AtomicDormantLeaderWireAdmissionConsumesRealPacketWithFreshCarrier,
   AdmitHiddenPacketReservesFreshSharedPhysicalOrdinal,
   RuntimeLeaderWireCannotRetireMerelyFromIngressPop,
   RetireLeaderWireLifecycleRetainsTerminalTombstone,
   FS_Image, FS_Subset, IsaT(2400)
   DEF AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncLeaderWireRuntimeCandidate,
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

THEOREM CandidateProducerContinuationFrozenOrdinaryIngressChargeCannotAppearAtGst ==
  \A node \in ValidatorIds, targetOrdinal \in Nat:
    /\ gst
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ targetOrdinal
         <= AsyncNextCandidateLifecycleOrdinal(node)
    /\ (AsyncCandidateProducerContinuationTargetPhysicalCut(
           node, targetOrdinal))'
         = AsyncCandidateProducerContinuationTargetPhysicalCut(
             node, targetOrdinal)
    /\ [AsyncNext]_AsyncAllVars
      => (AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates(
             node, targetOrdinal))'
           \subseteq
             AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates(
               node, targetOrdinal)
BY AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncFreshOrdinaryIngressCarrierAdmissionsAreSingularThisStep,
   LaterAcceptedOrdinaryCarrierCannotOvertakeFrozenCarrier,
   FS_Image, FS_Subset, IsaT(1800)
   DEF AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates,
       AsyncCandidateProducerContinuationTargetPhysicalCut,
       AsyncControlServiceSlotTransition,
       AsyncCandidateLifecycleStateAfterOrdinaryIngressAdmission,
       AsyncFreshOrdinaryIngressCarrierEvidenceForNodeIn,
       AsyncOrdinaryIngressCarrierEvidence,
       AsyncOrdinaryIngressCarrierStateAfterTransition,
       AsyncOrdinaryIngressCarrierEvidenceAfterPhysicalTransition,
       AsyncOrdinaryIngressCarrierAfterPhysicalTransition,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       AsyncAllVars

THEOREM CandidateProducerContinuationFrozenServeCutCannotReplenish ==
  \A node \in ValidatorIds, targetOrdinal \in Nat:
    /\ gst
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ targetOrdinal <= AsyncNextCandidateLifecycleOrdinal(node)
    /\ (AsyncCandidateProducerContinuationTargetPhysicalCut(
           node, targetOrdinal))'
         = AsyncCandidateProducerContinuationTargetPhysicalCut(
             node, targetOrdinal)
    /\ [AsyncNext]_AsyncAllVars
      => (AsyncCandidateProducerContinuationFrozenServeIngressIdentities(
             node, targetOrdinal))'
           \subseteq
             AsyncCandidateProducerContinuationFrozenServeIngressIdentities(
               node, targetOrdinal)
BY AsyncCausalEpisodeServeCutCannotReplenish,
   AsyncServeIngressAdmissionConsumesSharedSchedulerOrdinal,
   AsyncSharedSchedulerHighWatermarkIsMonotone,
   AsyncIngressPhysicalHighWatermarkIsMonotone,
   FS_Subset, IsaT(1200)
   DEF AsyncCandidateProducerContinuationFrozenServeIngressIdentities,
       AsyncCandidateProducerContinuationTargetPhysicalCut,
       AsyncCausalEpisodeServeIngressIdentities,
       AsyncServeIngressAdmissionOwned,
       AsyncServeIngressAdmissionOrdinal,
       AsyncServeIngressAdmissionRecords,
       AsyncServeIngressAdmissionRecord,
       AsyncAllVars

THEOREM CandidateProducerContinuationExactLocalReplayReplacesFrozenCharge ==
  \A node \in ValidatorIds, targetOrdinal \in Nat:
    /\ AsyncStrongTypeInvariant
    /\ AsyncProgressOwnershipInvariant
    /\ AsyncCandidateServiceLifecycleInvariant
    /\ AsyncNext
    /\ AsyncControlServiceSlotTransition
    /\ AsyncCandidateProducerContinuationExactLocalReplayStep(node)
    /\ (AsyncCandidateProducerContinuationTargetPhysicalCut(
           node, targetOrdinal))'
         = AsyncCandidateProducerContinuationTargetPhysicalCut(
             node, targetOrdinal)
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
       AsyncCandidateProducerContinuationFrozenCausalCandidates,
       AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationTargetPhysicalCut,
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

(***************************************************************************
Protected-Candidate frozen-prefix classification.

Unlike the continuation-status closure below, this theorem freezes its cut
from a still-scheduled Candidate lifecycle.  It therefore does not assume a
particular continuation identity or status.  A serviced frozen Candidate may
atomically install successors and one Reserved continuation, but the augmented
topological weight prepays that transfer.  Exact Local replay replaces its
latent Candidate with the identical scheduled value, and every later physical
root is excluded by the immutable cut.
***************************************************************************)
THEOREM AsyncProtectedCandidateFrozenPrefixStepIsDescentOrFrame ==
  \A candidate \in AsyncCandidateSet:
    LET cutoffOrdinal == AsyncCandidateLifecycleOrdinal(candidate)
        rank == AsyncCandidateProducerContinuationFrozenPrefixRank(
                  candidate.node, cutoffOrdinal)
    IN /\ gst
       /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ ProtectedCandidateOwned(candidate)
       /\ [AsyncNext]_AsyncAllVars
       /\ ProtectedCandidateOwned(candidate)'
       => \/ <<AsyncCandidateProducerContinuationFrozenPrefixRank(
                   candidate.node, cutoffOrdinal)', rank>>
                  \in
                    AsyncCandidateProducerContinuationFrozenPrefixRankOrdering
          \/ AsyncCandidateProducerContinuationFrozenPrefixRank(
               candidate.node, cutoffOrdinal)' = rank
BY AsyncCausalEpisodeTargetLifecycleOrdinalPersists,
   AsyncCausalEpisodeTargetPhysicalCutPersists,
   AsyncProtectedCandidateTargetPhysicalCutPersists,
   AsyncCausalEpisodeFrozenOriginsCannotReplenish,
   CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight,
   CandidateProducerContinuationCausalSuccessorRetainsFrozenPhysicalClass,
   CandidateProducerContinuationPostCutCausalRootCannotEnterFrozenPrefix,
   CandidateProducerContinuationFrozenLeaderWireChargeCannotAppearAtGst,
   CandidateProducerContinuationFrozenOrdinaryIngressChargeCannotAppearAtGst,
   CandidateProducerContinuationFrozenServeCutCannotReplenish,
   CandidateProducerContinuationPostCutServeCannotEnterFrozenPrefix,
   AsyncCandidateProducerContinuationExactLocalReplayRetainsReservation,
   AsyncCandidateProducerContinuationExactLocalReplayPublishesStoredCarrier,
   ExternalContinuationPersistsOrDescendsOrReplayExits,
   LocalContinuationPersistsOrDescendsOrReplayExits,
   AsyncCandidateProducerContinuationGstExcludesResetReplay,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   AsyncCandidateProducerSemanticHandoffReservedPersistsWithoutAck,
   AsyncCandidateProducerSemanticHandoffMaterializationRequiresSuccessor,
   AsyncCandidateProducerSemanticHandoffRetirementRequiresAck,
   AsyncServeIngressFrozenPredecessorPrefixNeverReplenishesOnDrain,
   AsyncServeQueuedIdentityDepartureInstallsTombstone,
   AsyncServeTombstonedIdentityCannotRequeueAtGst,
   FS_CardinalityType, FS_Subset, IsaT(7200)
   DEF AsyncCandidateProducerContinuationFrozenPrefixRank,
       AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
       AsyncCandidateProducerContinuationFrozenProducerBudget,
       AsyncCandidateProducerContinuationFrozenProducerTokens,
       AsyncCandidateProducerContinuationFrozenCandidateTokens,
       AsyncCandidateProducerContinuationFrozenCandidateOwners,
       AsyncCandidateProducerContinuationFrozenCausalCandidates,
       AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationTargetPhysicalCut,
       AsyncCandidateProducerContinuationFrozenServeIngressIdentities,
       AsyncCandidateProducerContinuationFrozenServeWorkTokens,
       AsyncCandidateProducerContinuationFrozenServeWorkBudget,
       AsyncCandidateProducerContinuationFrozenServeReachDebt,
       AsyncCandidateProducerContinuationCausalWeight,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuationStateAfterDeparture,
       AsyncCandidateProducerContinuationReservationAvailableIn,
       AsyncCandidateProducerContinuationAddressCanAdvanceIn,
       AsyncCandidateProducerContinuationRecordsForAddressIn,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationStatusRank,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationConcreteSuccessorOwned,
       AsyncCandidateProducerContinuationHandoffOwned,
       AsyncCandidateProducerContinuationHandoffRetired,
       AsyncCandidateProducerContinuationLocalReplayCarrier,
       AsyncCandidateServiceStateAfterReclamation,
       AsyncCandidateProducerContinuations,
       AsyncCausalEpisodeCandidates,
       AsyncCausalEpisodeFrozenPredecessorOrigins,
       AsyncCausalEpisodeServeWorkTokens,
       AsyncCausalEpisodeServeReachDebt,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncServeReservationsAfterIoService,
       AsyncServeReservationsAfterIngressDrain,
       AsyncControlServiceSlotTransition,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       CandidateScheduled, CandidateScheduledAfter,
       AsyncAllVars, LexPairOrdering, OpToRel

THEOREM AsyncProtectedCandidateIngressEpisodeStepIsDescentOrFrame ==
  \A candidate \in AsyncCandidateSet:
    LET rank == AsyncProtectedCandidateIngressEpisodeRank(candidate)
    IN /\ gst
       /\ AsyncStrongTypeInvariant
       /\ AsyncProgressOwnershipInvariant
       /\ ProtectedCandidateOwned(candidate)
       /\ [AsyncNext]_AsyncAllVars
       /\ ProtectedCandidateOwned(candidate)'
       => \/ <<AsyncProtectedCandidateIngressEpisodeRank(candidate)', rank>>
                  \in
                    AsyncProtectedCandidateIngressEpisodeRankOrdering
          \/ AsyncProtectedCandidateIngressEpisodeRank(candidate)' = rank
BY AsyncProtectedCandidateFrozenPrefixStepIsDescentOrFrame,
   AsyncCausalEpisodeTargetLifecycleOrdinalPersists,
   AsyncCausalEpisodeTargetPhysicalCutPersists,
   AsyncProtectedCandidateTargetPhysicalCutPersists,
   AsyncCausalEpisodeFrozenOriginsCannotReplenish,
   AsyncCausalEpisodeServeCutCannotReplenish,
   CandidateProducerContinuationFrozenLeaderWireChargeCannotAppearAtGst,
   CandidateProducerContinuationFrozenOrdinaryIngressChargeCannotAppearAtGst,
   CandidateProducerContinuationActionInertDormantHasZeroFrozenStage,
   CandidateProducerContinuationPostCutAdmissionCannotEnterFrozenPrefix,
   CandidateProducerContinuationPostCutOrdinaryAdmissionCannotEnterFrozenPrefix,
   CandidateProducerContinuationDropPolicyRejectedIsFrozenPhysicalPrefixFrame,
   CandidateProducerContinuationPreCutIngressToRuntimeConsumesBarrierStage,
   CandidateProducerContinuationPreCutOrdinaryIngressConsumesBarrierStage,
   AsyncCandidateProducerContinuationPostCutIngressCannotBlockRunnerTurn,
   LeaderWireIngressDrainNeverInventsRuntimeOwner,
   RuntimeLeaderWireCannotRetireMerelyFromIngressPop,
   RetireLeaderWireLifecycleRetainsTerminalTombstone,
   ExactTicketTurnDecreasesDrainableIngressTurnReach,
   ExhaustedIngressStepDecreasesDrainableIngressTurnReach,
   LocalStepDecreasesDrainableIngressTurnReach,
   SerializedLocalPredecessorDecreasesDrainableIngressTurnReach,
   RuntimeStepDecreasesDrainableIngressTurnReach,
   OlderRuntimeInterleaveDecreasesDrainableIngressTurnReach,
   ServiceIoWorkerDropsQueueDepth,
   FS_CardinalityType, FS_Subset, IsaT(7200)
   DEF AsyncProtectedCandidateIngressEpisodeRank,
       AsyncProtectedCandidateIngressEpisodeRankOrdering,
       AsyncProtectedCandidateIngressEpisodeTailRank,
       AsyncProtectedCandidateIngressEpisodeTailOrdering,
       AsyncCausalEpisodeFrozenIngressBarrierStageBudget,
       AsyncCausalEpisodeTargetPhysicalCut,
       AsyncFrozenLeaderWireBarrierStageBudget,
       AsyncFrozenLeaderWireBarrierStageTokens,
       AsyncFrozenLeaderWireBarrierRemainingStage,
       AsyncFrozenLeaderWireBarrierRecords,
       AsyncFrozenOrdinaryIngressBarrierRecords,
       AsyncFrozenLeaderWireIngressDependencyRank,
       AsyncFrozenLeaderWireIngressRecords,
       AsyncFrozenOrdinaryIngressRecords,
       AsyncFrozenLeaderWireSelectedIngressRecord,
       AsyncFrozenOrdinarySelectedIngressCarrier,
       AsyncFrozenIngressBarrierSelectsOrdinary,
       AsyncFrozenIngressBarrierSelectedItem,
       AsyncFrozenIngressBarrierSelectedPhysicalRank,
       AsyncFrozenLeaderWireIngressRank,
       AsyncFrozenLeaderWireIngressModeRank,
       AsyncFrozenLeaderWireIngressCapacityRank,
       AsyncFrozenLeaderWireIngressRunnerRank,
       AsyncFrozenLeaderWireIngressPriorityRank,
       AsyncFrozenLeaderWireIngressPriorityOwners,
       AsyncFrozenLeaderWireIngressLanePosition,
       AsyncFrozenLeaderWireIngressLaneIndices,
       AsyncFrozenLeaderWireIngressSourcePosition,
       AsyncCandidateProducerContinuationFrozenPrefixRank,
       AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
       AsyncCandidateProducerContinuationFrozenProducerBudget,
       AsyncCandidateProducerContinuationFrozenProducerTokens,
       AsyncCandidateProducerContinuationFrozenCandidateTokens,
       AsyncCandidateProducerContinuationFrozenCandidateOwners,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenServeWorkBudget,
       AsyncCandidateProducerContinuationFrozenServeReachDebt,
       AsyncLeaderWirePhysicalIngressRank,
       AsyncLeaderWireEarlierPhysicalOwners,
       AsyncLeaderWireFrozenIngressPredecessorDebtSet,
       AsyncLeaderWireLifecycleStateAfterIngressAdmission,
       AsyncLeaderWireLifecycleRecordAfterIngressDrain,
       AsyncLeaderWireLifecyclesAfterIngressDrain,
       AsyncLeaderWireIngressPrefixSnapshot,
       AsyncServeIngressAdmissionsAfterIngressDrain,
       AsyncServeIngressAdmissionsWithout,
       AsyncServeReservationsAfterIoService,
       AsyncServeReservationsAfterIngressDrain,
       ServiceIoWorkerWork, PopSelectedIngress,
       DrainFairIngressSelected, DrainHistoricalIngressSelected,
       LocalAdmissionStep, IngressDrainStep,
       SerializedLocalPrecedesServeIngressStep,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       AsyncServeIngressTargetOnlyTurn,
       AsyncNext, AsyncAllVars,
       LexPairOrdering, OpToRel

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

THEOREM CandidateProducerContinuationTargetPhysicalCutIsStableUntilStatusExit ==
  \A node \in ValidatorIds,
     identity \in AsyncCandidateServiceIdentities,
     targetOrdinal \in Nat \ {0},
     targetStage \in AsyncCandidateServiceStageClasses,
     status \in {"Reserved", "Materialized"}:
    /\ AsyncControlServiceStateTypeInvariant
    /\ AsyncCandidateProducerContinuationTargetAtStatus(
         node, identity, targetOrdinal, targetStage, status)
    /\ [AsyncNext]_AsyncAllVars
    /\ ~(AsyncCandidateProducerContinuationTargetStatusExit(
            identity, status))'
      => (AsyncCandidateProducerContinuationTargetPhysicalCut(
             node, targetOrdinal))'
           = AsyncCandidateProducerContinuationTargetPhysicalCut(
               node, targetOrdinal)
BY AsyncCandidateProducerContinuationStepPreservesPhysicalCut,
   AsyncCandidateProducerContinuationStatusIsMonotone,
   ExternalContinuationPersistsOrDescendsOrReplayExits,
   LocalContinuationPersistsOrDescendsOrReplayExits,
   IsaT(1200)
   DEF AsyncCandidateProducerContinuationTargetPhysicalCut,
       AsyncCandidateProducerContinuationTargetAtStatus,
       AsyncCandidateProducerContinuationTargetStatusExit,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncCandidateProducerContinuationStatusRank,
       AsyncCandidateProducerContinuationRecordAfterStep,
       AsyncCandidateProducerContinuations,
       AsyncControlServiceSlotTransition,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       AsyncAllVars

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
  \A state:
    \A record \in AsyncCandidateProducerContinuationRecordSet:
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
  \A budget:
    \A node \in ValidatorIds,
       identity \in AsyncCandidateServiceIdentities,
       targetOrdinal \in Nat \ {0},
       targetStage \in AsyncCandidateServiceStageClasses,
       status \in {"Reserved", "Materialized"}:
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
       AsyncCandidateProducerContinuationFrozenCausalCandidates,
       AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationTargetPhysicalCut,
       AsyncCandidateProducerContinuationFrozenServeIngressIdentities,
       AsyncCandidateProducerContinuationFrozenServeWorkTokens,
       AsyncCandidateProducerContinuationFrozenServeWorkBudget,
       AsyncCandidateProducerContinuationFrozenServeReachDebt,
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
  \A budget:
    \A node \in ValidatorIds,
       identity \in AsyncCandidateServiceIdentities,
       targetOrdinal \in Nat \ {0},
       targetStage \in AsyncCandidateServiceStageClasses,
       status \in {"Reserved", "Materialized"}:
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
   AsyncIngressPhysicalHighWatermarkIsMonotone,
   AsyncCandidateProducerContinuationResetPreservesExactReservation,
   CandidateProducerContinuationTargetPhysicalCutIsStableUntilStatusExit,
   IsaT(1200)
   DEF AsyncCandidateProducerContinuationFrozenPrefixAtBudget,
       AsyncCandidateProducerContinuationTargetAtStatus,
       AsyncCandidateProducerContinuationTargetStatusExit,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationTargetPhysicalCut,
       AsyncCausalEpisodeFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationRecordsForIdentity,
       AsyncCandidateProducerContinuationRecordsForIdentityIn,
       AsyncAllVars

THEOREM CandidateProducerContinuationFrozenPrefixStepCannotReplenish ==
  \A node \in ValidatorIds,
     identity \in AsyncCandidateServiceIdentities,
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
   CandidateProducerContinuationTargetPhysicalCutIsStableUntilStatusExit,
   CandidateProducerContinuationSuccessorBatchAndReservationConsumeFrozenWeight,
   CandidateProducerContinuationCausalSuccessorRetainsFrozenPhysicalClass,
   CandidateProducerContinuationPostCutCausalRootCannotEnterFrozenPrefix,
   CandidateProducerContinuationDormantLocalReplayReplacementConsumesFrozenCausalCharge,
   CandidateProducerContinuationFrozenLeaderWireChargeCannotAppearAtGst,
   CandidateProducerContinuationFrozenOrdinaryIngressChargeCannotAppearAtGst,
   CandidateProducerContinuationActionInertDormantHasZeroFrozenStage,
   CandidateProducerContinuationPostCutAdmissionCannotEnterFrozenPrefix,
   CandidateProducerContinuationDropPolicyRejectedIsFrozenPhysicalPrefixFrame,
   CandidateProducerContinuationPreCutIngressToRuntimeConsumesBarrierStage,
   CandidateProducerContinuationEqualOrdinalLeaderWireCoalescesTargetCell,
   CandidateProducerContinuationExactLocalReplayReplacesFrozenCharge,
   ExternalContinuationPersistsOrDescendsOrReplayExits,
   LocalContinuationPersistsOrDescendsOrReplayExits,
   AsyncCandidateProducerContinuationGstExcludesResetReplay,
   CandidateProducerContinuationFrozenServeCutCannotReplenish,
   CandidateProducerContinuationPostCutServeCannotEnterFrozenPrefix,
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
       AsyncCandidateProducerContinuationFrozenCausalCandidates,
       AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationTargetPhysicalCut,
       AsyncCandidateProducerContinuationFrozenServeIngressIdentities,
       AsyncCandidateProducerContinuationFrozenServeWorkTokens,
       AsyncCandidateProducerContinuationFrozenServeWorkBudget,
       AsyncCandidateProducerContinuationFrozenServeReachDebt,
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
   CandidateProducerContinuationDormantLocalReplayReplacementConsumesFrozenCausalCharge,
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
       AsyncCandidateProducerContinuationLogicalPrecedes,
       AsyncCandidateProducerContinuationPhysicallyEligibleResolutionRecordsForNode,
       AsyncCandidateProducerContinuationPhysicallyBehindActiveTarget,
       AsyncCandidateProducerContinuationSelectedResolutionRecord,
       AsyncCandidateProducerContinuationStatusRank,
       AsyncCandidateProducerContinuationFrozenPrefixRankOrdering,
       AsyncCausalEpisodeStructuralRankOrdering,
       AsyncCausalEpisodeServeRankOrdering,
       LexPairOrdering, SetLessThan, OpToRel

THEOREM CandidateProducerContinuationFairResolutionStrictlyDescendsFrozenPrefix ==
  \A node \in ValidatorIds,
     identity \in AsyncCandidateServiceIdentities,
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
       AsyncCandidateProducerContinuationFrozenCausalCandidates,
       AsyncCandidateProducerContinuationFrozenOrdinaryIngressCandidates,
       AsyncCandidateProducerContinuationFrozenLeaderWireCandidates,
       AsyncCandidateProducerContinuationFrozenDormantLocalReplayCandidates,
       AsyncCandidateProducerContinuationFrozenStatusTokens,
       AsyncCandidateProducerContinuationFrozenRecords,
       AsyncCandidateProducerContinuationFrozenPredecessorOrigins,
       AsyncCandidateProducerContinuationTargetPhysicalCut,
       AsyncCandidateProducerContinuationFrozenServeIngressIdentities,
       AsyncCandidateProducerContinuationFrozenServeWorkTokens,
       AsyncCandidateProducerContinuationFrozenServeWorkBudget,
       AsyncCandidateProducerContinuationFrozenServeReachDebt,
       AsyncCandidateProducerContinuationCausalWeight,
       AsyncCandidateProducerContinuationTargetAtStatus,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       AsyncCandidateProducerContinuationResolutionPredecessorsFor,
       AsyncCandidateProducerContinuationLogicalPrecedes,
       AsyncCandidateProducerContinuationPhysicallyEligibleResolutionRecordsForNode,
       AsyncCandidateProducerContinuationPhysicallyBehindActiveTarget,
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
          identity \in AsyncCandidateServiceIdentities,
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
          identity \in AsyncCandidateServiceIdentities,
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
  \A state:
    \A record \in state.producerContinuations:
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
           /\ after.sourcePhysicalOrdinal = record.sourcePhysicalOrdinal
           /\ after.physicalCut = record.physicalCut
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
           /\ after.sourcePhysicalOrdinal = record.sourcePhysicalOrdinal
           /\ after.physicalCut = record.physicalCut
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
  \A state, candidate:
    \A record \in state.producerContinuations:
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
  \A initialContext \in ContextRecords,
     node \in AsyncVotersAt(initialContext):
    PostGstResolveLocalCandidateProducerContinuation(node)
      => AsyncFairActionAt(initialContext)
BY DEF AsyncFairActionAt

THEOREM ConditionalTransportProducerContinuationServiceUsesReviewedFairAction ==
  \A initialContext \in ContextRecords,
     node \in AsyncVotersAt(initialContext):
    PostGstServiceConditionalTransportProducerContinuation(node)
      => AsyncFairActionAt(initialContext)
BY DEF AsyncFairActionAt

THEOREM VolatileBodyProducerContinuationServiceUsesReviewedFairAction ==
  \A initialContext \in ContextRecords,
     node \in AsyncVotersAt(initialContext):
    PostGstServiceVolatileBodyProducerContinuation(node)
      => AsyncFairActionAt(initialContext)
BY DEF AsyncFairActionAt

=============================================================================
