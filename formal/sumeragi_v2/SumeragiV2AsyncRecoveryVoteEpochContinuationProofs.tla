---- MODULE SumeragiV2AsyncRecoveryVoteEpochContinuationProofs ----
EXTENDS SumeragiV2AsyncRecoveryVoteEpochProofs

(***************************************************************************
The reviewed recovery-vote shard boundary also carries dependency-safe
induction leaves and the retained-body tail.  They are moved byte-for-byte
after every provider they consume, keeping the aggregate theorem/operator
inventory unchanged while both physical shards remain independently bounded.
***************************************************************************)
THEOREM AsyncServeProducerEpisodeTransitionPreservesTypeInvariant ==
  /\ AsyncServeProducerEpisodeTypeInvariant
  /\ AsyncServeProducerEpisodeTransition
  => AsyncServeProducerEpisodeTypeInvariant'
BY Zenon
   DEF AsyncServeProducerEpisodeTypeInvariant,
       AsyncServeProducerEpisodeTransition

THEOREM AsyncStrongTypeProjectsControlServiceStateType ==
  AsyncStrongTypeInvariant => AsyncControlServiceStateTypeInvariant
BY DEF AsyncStrongTypeInvariant

THEOREM FreshReplayCandidateIsDisjointFromScheduled ==
  \A candidate:
    SequenceSet(FreshCandidateSequence(candidate)) \cap
      (QueuedCandidates \cup DeferredCandidates \cup CausalCandidates
        \cup TrackedWorkCandidates) = {}
BY Isa
   DEF FreshCandidateSequence, CandidateScheduled, SequenceSet

THEOREM ReplayingOrdinaryStepPreservesRecoveryCorridor ==
  /\ StrongInductiveInvariant
  /\ AsyncTypeInvariant
  /\ AsyncRecoveryTypeInvariant
  /\ AsyncRecoveryExecutionInvariant
  /\ asyncRecoveryPhase = "Replaying"
  /\ (AsyncRunnerStep \/ AsyncNonRunnerStep)
  /\ UNCHANGED <<up, AsyncRecoveryVars>>
  => /\ (~NodeHasApplication(asyncRecoveryNode))'
     /\ (RestartDecisions(asyncRecoveryNode) = {})'
     /\ \A request \in asyncActiveRequests':
          \/ request.source # asyncRecoveryNode'
          \/ (RestartLockedCertifiedRequest(
                asyncRecoveryNode, request))'
     /\ \A candidate \in
          ResponsiveReplayScheduledCandidates(asyncRecoveryNode)':
          /\ (CandidateConsumerCurrent(candidate))'
          /\ \/ candidate \in
                   (SequenceSet(
                      RestartSignatureReplay(asyncRecoveryNode)))'
             \/ (RestartLockedBodyPipelineCandidate(
                   asyncRecoveryNode, candidate))'
BY RestartSignatureReplayCommandsAreSignatures,
   RestartLockedBodyReplayCandidateShape,
   RestartReplayReplayingCandidateShape,
   SMTT(180), Isa
   DEF AsyncRunnerStep, RunNode, RunHistoricalRecoveryNode,
       RunNodeWork,
       ResolveRunNodeCandidateProducerContinuation,
       ReplayRunNodeCandidateProducerContinuation,
       AsyncCandidateProducerContinuationExactLocalReplayStep,
       AsyncCandidateProducerContinuationReplayTargetOnlyTurn,
       AsyncCandidateProducerContinuationExactRuntimeReplayStep,
       AsyncCandidateProducerContinuationExactReplayIdentity,
       AsyncCandidateProducerContinuationSelectedLocalCandidate,
       AsyncCandidateProducerContinuationSelectedReplayRecord,
       AsyncCandidateProducerContinuationSelectedResolutionRecord,
       AsyncCandidateProducerContinuationResolutionRequired,
       AsyncCandidateProducerContinuationResolutionRecordsForNode,
       ResponsiveReplayQuarantined,
       AsyncSchedulerExceptCausalControlAndNodeService,
       AsyncSchedulerExceptCausalControlCommandRunnerAndNodeService,
       AsyncSchedulerExceptCausalControlRunnerAndNodeService,
       LocalAdmissionStep, AdmitProducerCompletion,
       AdmitCausalHead, IngressDrainStep, DrainFairIngressSelected,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn,
       RuntimeStep, DeferredDrainStep,
       FifoRuntimeStep, DeferredTagStep, DeferredTimeoutStep,
       DeferredRetransmitStep, DirectTimeoutStep,
       DirectRetransmitStep, IdleRuntimeStep,
       RemoveNextDeferredCommand, RemoveNextNodeCommand,
       DeferCommand, DiscardCommand, AdvanceNextDeferredClass,
       ExecuteCommand, ExecuteRegularCommand, ExecuteDecisionFetch,
       ExecuteSignProposal, ExecuteSignVote, ExecuteFormPrepareQC,
       ExecuteSignTimeout, ExecutePersistInstall,
       ExecutePersistDecision, ExecuteRequestCertifiedBody,
       ExecuteApply, ExecuteCoreDelivery, ExecuteChunkDelivery,
       ExecuteRejectAuthenticatedJunk,
       PublishCertifiedRequests, CertifiedRequestOutbox,
       CertifiedRecoveryFetchFrontier, LockedPrepareFetchFrontier,
       AppendCausalSuccessors, FreshCommandSuccessors,
       CommandSuccessors, FreshCandidateSequence,
       CausalCandidate, AsyncCandidateFrom,
       AsyncNonRunnerStep, AsyncNetworkStep, AdmitIngressPacket,
       AdmitHiddenPacket, CoalesceHiddenPacket,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       CommitCertificateDiscoveryStepWork,
       ResponsiveReplayQuarantined, ResponsiveReplayDraining,
       RestartLockedCertifiedRequest,
       RestartLockedBodyPipelineCandidate,
       RestartLockedPrepareQCs, LockedPrepareRecoverySource,
       ResponsiveReplayScheduledCandidates,
       QueuedCandidates, DeferredCandidates, CausalCandidates,
       TrackedWorkCandidates, CandidateScheduled,
       CandidateConsumerCurrent, NodeHasApplication, RestartDecisions,
       AsyncRecoveryTypeInvariant,
       AsyncRecoveryExecutionInvariant, AsyncRecoveryVars,
       SequenceSet, vars

THEOREM AsyncNextPreservesCandidateLifecycleSchedulerCoverage ==
  /\ AsyncStrongTypeInvariant
  /\ AsyncNext
  => AsyncCandidateLifecycleSchedulerCoverageInvariant'
BY AsyncNextNeverSchedulesAnUnownedCandidateLifecycle,
   FS_CardinalityType, FS_Subset, IsaT(7200)
   DEF AsyncStrongTypeInvariant,
       AsyncCandidateLifecycleSchedulerCoverageInvariant,
       AsyncCandidateLifecycleActiveRecords,
       AsyncCandidateLifecycleRecordCoversScheduledOrigin,
       AsyncScheduledCandidateOriginsForNode,
       AsyncCandidateLifecycleAdmissions,
       AsyncControlServiceSlotTransition,
       AsyncCandidateLifecycleStateAfterCarrierUpdate,
       AsyncCandidateLifecycleCarrierUpdatedAdmissions,
       AsyncCandidateLifecycleStateAfterOrdinaryIngressAdmission,
       AsyncCandidateLifecycleStateAfterServeIngressAdmission,
       AsyncCandidateLifecycleStateAfterCompaction,
       AsyncCandidateLifecycleStateAfterAdmission,
       AsyncCandidateLifecycleStateAfterTimeoutOwnership,
       AsyncCandidateLifecycleNewAdmissions,
       AsyncNewCandidateLifecycleOriginsForNodeIn,
       AsyncCandidateLifecycleOriginsRecordedForNodeIn,
       AsyncCandidateLifecycleRetirementCoveredIn,
       AsyncCandidateLifecycleDormantReservationOwnedAfter,
       AsyncCandidateLifecycleDeparturesThisStep,
       AsyncCandidateServicesThisStep,
       AsyncCandidateSemanticallyAppliedThisStep,
       AsyncCandidateSuccessfullyServicedThisStep,
       AsyncCandidateIgnoredWithoutApplicationThisStepSet,
       AsyncCandidateIgnoredWithoutApplicationThisStep,
       AsyncCandidatePhysicallyDiscardedThisStep,
       AsyncNext, AsyncNonCrashStep, AsyncRunnerStep,
       AsyncNonRunnerStep, AsyncNetworkStep, AsyncFaultStep,
       RunNode, RunHistoricalRecoveryNode, RunHistoricalServer,
       RunNodeWork, LocalAdmissionStep, IngressDrainStep,
       SerializedRuntimeStep, SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       ServiceIoWorkerWork, AppendCausalSuccessors,
       AdmitIngressPacket, AdmitHiddenPacket, CoalesceHiddenPacket,
       DropPolicyRejectedHiddenPacket,
       PreGstResponsiveRestart, PreGstResponsiveReplay,
       ResetNodeSchedulerForRestart,
       CandidateScheduled, CandidateScheduledAfter,
       CandidateScheduledIn, EnqueueCandidate,
       AsyncAllVars, AsyncSchedulerVars

THEOREM AsyncNextPreservesStrongTypeInvariant ==
  AsyncStrongTypeInvariant /\ AsyncNext
    => AsyncStrongTypeInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              AsyncNext
         PROVE AsyncStrongTypeInvariant'
    <2>1. StrongInductiveInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2. AsyncTypeInvariant
      BY <1>1, AsyncStrongTypeProjectsAsyncType
    <2>2a. AsyncRecoveryTypeInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2b. AsyncRestartAuthorityInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2c. AsyncRecoveryExecutionInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2d. /\ AsyncHistoricalLockRestartAuthorityTypeInvariant
            /\ HistoricalLockRestartAuthoritySourceRetentionInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2e. AsyncGstRecoveryPhaseInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2f. AsyncSerializedBusyKernelInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2g. AsyncCertifiedResponseClaimIngressOwnershipInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2j. AsyncLeaderWireIngressCarrierOwnershipInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2k. AsyncOrdinaryIngressCarrierOwnershipInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2h. AsyncControlServiceStateTypeInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2i. AsyncServiceActivationPairInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2l. AsyncCandidateLifecycleSchedulerCoverageInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>2m. /\ AsyncProducerTypeInvariant
            /\ AsyncServeProducerEpisodeTypeInvariant
            /\ AsyncServeProducerEpisodeOwnershipInvariant
            /\ AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant
      BY <1>1 DEF AsyncStrongTypeInvariant
    <2>3. StrongInductiveInvariant'
      BY <1>1, <2>1, AsyncNextPreservesStrongInductiveInvariant
    <2>4. AsyncSchedulerTypeInvariant'
      BY <1>1, <2>1, <2>2, <2>2a,
         AsyncNextPreservesSchedulerType
    <2>4a. AsyncServiceActivationPairInvariant'
      BY <1>1, <2>2,
         AsyncNextPreservesServiceActivationPairInvariant
    <2>4b. AsyncControlServiceStateTypeInvariant'
      BY <1>1, <2>2, <2>2h,
         AsyncNextPreservesControlServiceStateTypeInvariant
    <2>4c. AsyncCandidateLifecycleSchedulerCoverageInvariant'
      BY <1>1, AsyncNextPreservesCandidateLifecycleSchedulerCoverage
    <2>4d. /\ AsyncServeProducerEpisodeTypeInvariant'
             /\ AsyncServeProducerEpisodeOwnershipInvariant'
      BY <1>1, AsyncNextPreservesServeProducerEpisodeInvariants
    <2>4e. AsyncTimeoutRecoveryEpisodeCurrentBoundaryInvariant'
      BY <1>1, <2>2h,
         AsyncNextPreservesTimeoutRecoveryCurrentBoundaryInvariant
    <2>4f. AsyncProducerTypeInvariant'
      BY <1>1, <2>2, AsyncProducerProjectionPreservesTypeInvariant
         DEF AsyncNext
    <2>5. ReceivedTimeoutVotePoolInvariant'
      BY <1>1, <2>2, AsyncNextPreservesTimeoutPoolInvariant
    <2>6. /\ AsyncRecoveryTypeInvariant'
           /\ AsyncRestartAuthorityInvariant'
      BY <1>1, <2>1, <2>2, <2>2a, <2>2b, <2>2c,
         AsyncNextPreservesRecoveryInvariants
    <2>7. AsyncRecoveryExecutionInvariant'
      BY <1>1, <2>1, <2>2, <2>2a, <2>2b, <2>2c,
         AsyncNextPreservesRecoveryExecutionInvariant
    <2>8. /\ AsyncHistoricalLockRestartAuthorityTypeInvariant'
           /\ HistoricalLockRestartAuthoritySourceRetentionInvariant'
      BY <1>1, <2>1, <2>2d,
         AsyncNextPreservesHistoricalLockRestartAuthorityInvariants
    <2>9. AsyncSerializedBusyKernelInvariant'
      BY <1>1, <2>1, <2>2f,
         AsyncNextPreservesSerializedBusyKernelInvariant
    <2>10. AsyncGstRecoveryPhaseInvariant'
      BY <1>1, <2>2e,
         AsyncNextPreservesGstRecoveryPhaseInvariant
    <2>11. AsyncCertifiedResponseClaimIngressOwnershipInvariant'
      BY <1>1, <2>2, <2>2g,
         AsyncNextPreservesCertifiedResponseClaimIngressOwnershipInvariant
    <2>12. AsyncLeaderWireIngressCarrierOwnershipInvariant'
      BY <1>1, <2>2j,
         AsyncNextPreservesLeaderWireIngressCarrierOwnership
    <2>13. AsyncOrdinaryIngressCarrierOwnershipInvariant'
      BY <1>1, <2>2k,
         AsyncNextPreservesOrdinaryIngressCarrierOwnership
    <2> QED BY <2>2l, <2>2m, <2>3, <2>4, <2>4a, <2>4b, <2>4c, <2>4d,
                <2>4e, <2>4f, <2>5, <2>6, <2>7, <2>8, <2>9, <2>10,
                <2>11, <2>12, <2>13
         DEF AsyncStrongTypeInvariant
  <1> QED BY <1>1

THEOREM AsyncBracketNextPreservesStrongTypeInvariant ==
  AsyncStrongTypeInvariant /\ [AsyncNext]_AsyncAllVars
    => AsyncStrongTypeInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              [AsyncNext]_AsyncAllVars
         PROVE AsyncStrongTypeInvariant'
    <2>1. CASE AsyncNext
      BY <1>1, <2>1, AsyncNextPreservesStrongTypeInvariant
    <2>2. CASE UNCHANGED AsyncAllVars
      BY <1>1, <2>2,
         AsyncAllVarsStutterPreservesStrongTypeInvariant
    <2> QED BY <1>1, <2>1, <2>2
  <1> QED BY <1>1

(***************************************************************************
Retained locked-body round rebinding.  View-independent retained authority may
survive a view change, but it is usable only by `RebindRetainedBody`; it is not
durable, validated, or applicable target-round evidence.  Proposal delivery
therefore emits a completion-class rebind candidate that materializes an exact
target-view Available record.  The ordinary StoreBody -> ValidateBody chain
then writes exact-view durable and validation evidence.
***************************************************************************)

RetainedBodyRebindReady(command) ==
  /\ command.kind = "RebindRetainedBody"
  /\ command.class = "Completion"
  /\ CandidateConsumerCurrent(command)
  /\ lockRank[command.node] # NoRank
  /\ lockSubject[command.node] = command.subject
  /\ RetainedLockedBodyHeldBy(
       retainedLockedBodies, command.node, context, command.subject)
  /\ BodyRecord(command.node, context, command.view, command.subject)
       \in BodyRecordSet
  /\ BodyRecord(command.node, context, command.view, command.subject)
       \notin availableBodies
  /\ \E proposal \in SeenProposalValues:
       /\ CommandMatches(command, command.node, proposal.view,
                         proposal.subject)
       /\ ProposalAt(command.node, proposal) \in seenProposals

RetainedBodyRebindAction(command, proposal) ==
  /\ command.kind = "RebindRetainedBody"
  /\ CommandMatches(command, command.node, proposal.view,
                    proposal.subject)
  /\ RebindRetainedBody(command.node, proposal)
  /\ UNCHANGED AsyncAuxVars

THEOREM RetainedBodyRebindCandidateIsTypedAndOwned ==
  \A command:
    (AsyncTypeInvariant /\ AsyncCandidateTyped(command))
      => /\ AsyncCandidateTyped(
               RetainedBodyRebindCandidate(command))
         /\ RetainedBodyRebindCandidate(command)
              \in AsyncCandidateSet
         /\ RetainedBodyRebindCandidate(command).node = command.node
         /\ RetainedBodyRebindCandidate(command).class = "Completion"
         /\ RetainedBodyRebindCandidate(command).kind =
              "RebindRetainedBody"
PROOF
  <1>1. ASSUME NEW command,
                AsyncTypeInvariant,
                AsyncCandidateTyped(command)
         PROVE /\ AsyncCandidateTyped(
                      RetainedBodyRebindCandidate(command))
                /\ RetainedBodyRebindCandidate(command)
                     \in AsyncCandidateSet
                /\ RetainedBodyRebindCandidate(command).node =
                     command.node
                /\ RetainedBodyRebindCandidate(command).class =
                     "Completion"
                /\ RetainedBodyRebindCandidate(command).kind =
                     "RebindRetainedBody"
    <2>1. /\ AsyncCandidateTyped(
                  RetainedBodyRebindCandidate(command))
           /\ RetainedBodyRebindCandidate(command).node = command.node
      BY <1>1, CausalCandidateFromTypedCommand
         DEF RetainedBodyRebindCandidate,
             AsyncCommandClasses, AsyncWorkKinds, AsyncReducerKinds
    <2>2. RetainedBodyRebindCandidate(command) \in AsyncCandidateSet
      BY <2>1, SMT DEF AsyncCandidateTyped, AsyncCandidateSet
    <2> QED BY <2>1, <2>2
       DEF RetainedBodyRebindCandidate, CausalCandidate,
           AsyncCandidateFrom,
           AsyncCandidateCausalSuccessorWithIdentityAndOrigin,
           AsyncCandidateSuccessorSemanticPhase,
           AsyncCandidateSuccessorProposalRound,
           AsyncCandidateWithIdentityAndOrigin
  <1> QED BY <1>1

THEOREM DeliverProposalSchedulesRetainedBodyRebind ==
  \A command:
    command.kind = "DeliverProposal"
      => CommandSuccessors(command) =
           <<RetainedBodyRebindCandidate(command),
             CausalCandidate("Normal", "BeginPrepare", command)>>
BY DEF CommandSuccessors

THEOREM RebindSchedulesCurrentRoundStore ==
  \A command:
    command.kind = "RebindRetainedBody"
      => CommandSuccessors(command) =
           <<CausalCandidate("Completion", "StoreBody", command)>>
BY DEF CommandSuccessors

THEOREM StoreSchedulesCurrentRoundValidation ==
  \A command:
    command.kind = "StoreBody"
      => CommandSuccessors(command) =
           <<CausalCandidate("Completion", "ValidateBody", command)>>
BY DEF CommandSuccessors

THEOREM ValidationSchedulesPrepareAndLockedCommitAttempts ==
  \A command:
    command.kind = "ValidateBody"
      => CommandSuccessors(command) =
           <<CausalCandidate("Normal", "BeginPrepare", command),
             CausalCandidate("Completion", "BeginLockCommit", command),
             CausalCandidate("Completion", "Apply", command)>>
BY DEF CommandSuccessors

(***************************************************************************
The production adapter classifies `ValidationCompleted` as Completion, and
the reducer calls `persist_commit_intent` inside that event.  PrepareQC
processing likewise calls the same persistence routine directly when the
body is already validated.  The split Core commands therefore keep every
internal BeginLockCommit continuation in the Completion lane; treating one
as independent Progress could defer the exact persistence completion behind
an unrelated Progress-capacity fence.
***************************************************************************)
THEOREM PrepareQcDeliverySchedulesCompletionLockedCommitAttempt ==
  \A command:
    /\ command.kind = "DeliverQC"
    /\ command.item.envelope.qc.phase = "Prepare"
    => CommandSuccessors(command) =
         <<CausalCandidate("Progress", "BeginObservePrepare", command),
           CausalCandidate("Completion", "BeginLockCommit", command)>>
BY DEF CommandSuccessors

THEOREM PersistedPrepareObservationSchedulesCompletionLockedCommitAttempt ==
  \A command:
    command.kind = "PersistObservePrepare"
      => CommandSuccessors(command) =
           <<CausalCandidate("Completion", "BeginLockCommit", command)>>
BY DEF CommandSuccessors

THEOREM ReadyRetainedBodyRebindEnablesExecution ==
  \A command:
    RetainedBodyRebindReady(command)
      => ENABLED ExecuteCommand(command)
PROOF
  <1>1. ASSUME NEW command,
                RetainedBodyRebindReady(command)
         PROVE ENABLED ExecuteCommand(command)
    <2>1. PICK proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ ProposalAt(command.node, proposal) \in seenProposals
      BY <1>1 DEF RetainedBodyRebindReady
    <2>2. ENABLED RetainedBodyRebindAction(command, proposal)
      BY <1>1, <2>1, ExpandENABLED, Isa
         DEF RetainedBodyRebindReady, RetainedBodyRebindAction,
             CommandMatches, RebindRetainedBody, AsyncAuxVars
    <2>3. RetainedBodyRebindAction(command, proposal) \in BOOLEAN
      BY Isa DEF RetainedBodyRebindAction
    <2>4. ExecuteCommand(command) \in BOOLEAN
      BY Isa DEF ExecuteCommand
    <2>5. RetainedBodyRebindAction(command, proposal)
             => ExecuteCommand(command)
      BY Isa
         DEF RetainedBodyRebindAction, ExecuteCommand,
             ExecuteRegularCommand, RegularCoreCommand
    <2>6. (ENABLED RetainedBodyRebindAction(command, proposal))
             => ENABLED ExecuteCommand(command)
      BY <2>3, <2>4, <2>5, ENABLEDaxioms
    <2> QED BY <2>2, <2>6
  <1> QED BY <1>1

THEOREM ReadyRetainedBodyRebindIsDispatchable ==
  \A command:
    (RetainedBodyRebindReady(command)
      /\ command \in AsyncCandidateSet)
      => CommandDispatchable(command)
PROOF
  <1>1. ASSUME NEW command,
                RetainedBodyRebindReady(command),
                command \in AsyncCandidateSet
         PROVE \E selectedCommand \in AsyncCandidateSet:
                   /\ selectedCommand = command
                   /\ ENABLED ExecuteCommand(selectedCommand)
                   /\ (NodeIdle(selectedCommand.node)
                         \/ selectedCommand.class = "Completion")
    <2>1. ENABLED ExecuteCommand(command)
      BY <1>1, ReadyRetainedBodyRebindEnablesExecution
    <2>2. command.class = "Completion"
      BY <1>1 DEF RetainedBodyRebindReady
    <2>3. CandidateConsumerCurrent(command)
      BY <1>1 DEF RetainedBodyRebindReady
    <2>4. WITNESS command \in AsyncCandidateSet
    <2> QED BY <2>1, <2>2, <2>3
  <1> QED BY <1>1 DEF CommandDispatchable

THEOREM RebindCommandSelectsRetainedRebind ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "RebindRetainedBody")
      => \E proposal \in SeenProposalValues:
           /\ CommandMatches(command, command.node, proposal.view,
                             proposal.subject)
           /\ RebindRetainedBody(command.node, proposal)
BY IsaT(60) DEF RegularCoreCommand

THEOREM ExecuteRebindStagesCurrentRoundBody ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "RebindRetainedBody")
      => /\ BodyRecord(command.node, context', command.view,
                       command.subject)
                \in availableBodies'
         /\ RetainedLockedBodyHeldBy(
              retainedLockedBodies', command.node, context',
              command.subject)
PROOF
  <1>1. ASSUME NEW command,
                RegularCoreCommand(command),
                command.kind = "RebindRetainedBody"
         PROVE /\ BodyRecord(command.node, context', command.view,
                             command.subject)
                       \in availableBodies'
                /\ RetainedLockedBodyHeldBy(
                     retainedLockedBodies', command.node, context',
                     command.subject)
    <2>1. \E proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ RebindRetainedBody(command.node, proposal)
      BY <1>1, RebindCommandSelectsRetainedRebind
    <2>2. PICK proposal \in SeenProposalValues:
             /\ CommandMatches(command, command.node, proposal.view,
                               proposal.subject)
             /\ RebindRetainedBody(command.node, proposal)
      BY <2>1
    <2>3. /\ command.view = proposal.view
           /\ command.subject = proposal.subject
           /\ context' = context
           /\ retainedLockedBodies' = retainedLockedBodies
           /\ BodyRecord(command.node, context, proposal.view,
                         proposal.subject)
                \in availableBodies'
           /\ RetainedLockedBodyHeldBy(
                retainedLockedBodies, command.node, context,
                command.subject)
      BY <1>1, <2>2, Isa
         DEF CommandMatches, RebindRetainedBody, RegularCoreCommand
    <2> QED BY <2>3 DEF RetainedLockedBodyHeldBy
  <1> QED BY <1>1

THEOREM ValidationCommandSelectsValidationAction ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "ValidateBody")
      => (\E proposal \in SeenProposalValues:
            /\ CommandMatches(command, command.node, proposal.view,
                              proposal.subject)
            /\ ValidateBody(command.node, proposal))
         \/ (\E proposal \in SeenProposalValues:
               /\ CommandMatches(command, command.node, proposal.view,
                                 proposal.subject)
               /\ RejectBody(command.node, proposal))
         \/ (\E qc \in DecisionQcValues:
               /\ CommandMatches(command, command.node, qc.view, qc.subject)
               /\ ValidateDecidedBody(command.node, qc))
         \/ (\E qc \in prepareQCs:
               /\ CommandMatches(command, command.node, qc.view, qc.subject)
               /\ ValidateLockedBody(command.node, qc))
BY Isa DEF RegularCoreCommand

THEOREM ExecuteValidationBindsCurrentViewAndGeneration ==
  \A command:
    (RegularCoreCommand(command) /\ command.kind = "ValidateBody")
      => \/ BodyValidatedBy(
               validatedBodies', command.node, context', command.view,
               generation'[command.node], command.subject)
         \/ BodyRecord(command.node, context', command.view,
                       command.subject)
               \in invalidBodies'
PROOF
  <1>1. ASSUME NEW command,
                RegularCoreCommand(command),
                command.kind = "ValidateBody"
         PROVE \/ BodyValidatedBy(
                      validatedBodies', command.node, context', command.view,
                      generation'[command.node], command.subject)
                \/ BodyRecord(command.node, context', command.view,
                              command.subject)
                      \in invalidBodies'
    <2>1. (\E proposal \in SeenProposalValues:
              /\ CommandMatches(command, command.node, proposal.view,
                                proposal.subject)
              /\ ValidateBody(command.node, proposal))
           \/ (\E proposal \in SeenProposalValues:
                 /\ CommandMatches(command, command.node, proposal.view,
                                   proposal.subject)
                 /\ RejectBody(command.node, proposal))
           \/ (\E qc \in DecisionQcValues:
                 /\ CommandMatches(command, command.node, qc.view,
                                   qc.subject)
                 /\ ValidateDecidedBody(command.node, qc))
           \/ (\E qc \in prepareQCs:
                 /\ CommandMatches(command, command.node, qc.view,
                                   qc.subject)
                 /\ ValidateLockedBody(command.node, qc))
      BY <1>1, ValidationCommandSelectsValidationAction
    <2>2. CASE \E proposal \in SeenProposalValues:
                    /\ CommandMatches(
                         command, command.node, proposal.view,
                         proposal.subject)
                    /\ ValidateBody(command.node, proposal)
      <3>1. PICK proposal \in SeenProposalValues:
               /\ CommandMatches(command, command.node, proposal.view,
                                 proposal.subject)
               /\ ValidateBody(command.node, proposal)
        BY <2>2
      <3> QED BY <3>1, Isa
           DEF CommandMatches, ValidateBody, BodyValidatedBy
    <2>3. CASE \E proposal \in SeenProposalValues:
                    /\ CommandMatches(
                         command, command.node, proposal.view,
                         proposal.subject)
                    /\ RejectBody(command.node, proposal)
      <3>1. PICK proposal \in SeenProposalValues:
               /\ CommandMatches(command, command.node, proposal.view,
                                 proposal.subject)
               /\ RejectBody(command.node, proposal)
        BY <2>3
      <3> QED BY <3>1, Isa
           DEF CommandMatches, RejectBody, BodyRecord
    <2>4. CASE \E qc \in DecisionQcValues:
                    /\ CommandMatches(
                         command, command.node, qc.view, qc.subject)
                    /\ ValidateDecidedBody(command.node, qc)
      <3>1. PICK qc \in DecisionQcValues:
               /\ CommandMatches(command, command.node, qc.view, qc.subject)
               /\ ValidateDecidedBody(command.node, qc)
        BY <2>4
      <3> QED BY <3>1, Isa
           DEF CommandMatches, ValidateDecidedBody, BodyValidatedBy
    <2>5. CASE \E qc \in prepareQCs:
                    /\ CommandMatches(
                         command, command.node, qc.view, qc.subject)
                    /\ ValidateLockedBody(command.node, qc)
      <3>1. PICK qc \in prepareQCs:
               /\ CommandMatches(command, command.node, qc.view, qc.subject)
               /\ ValidateLockedBody(command.node, qc)
        BY <2>5
      <3> QED BY <3>1, Isa
           DEF CommandMatches, ValidateLockedBody, BodyValidatedBy
    <2> QED BY <2>1, <2>2, <2>3, <2>4, <2>5
  <1> QED BY <1>1

(***************************************************************************
Locked-round CommitVote recovery after a TC install.  Prepare admission remains
current-view-only.  The install clears only the installing node's volatile
vote receipts.  Retained CommitVote control is still retryable, and every
Commit delivery or locally formed CommitQC requires the exact durable Prepare
lock.  Persisting a replacement lock retires the superseded historical pool
while preserving current-view work and the new exact locked Commit pool.
***************************************************************************)

THEOREM PrepareVoteAdmissionIsCurrentView ==
  \A node, vote:
    (vote.phase = "Prepare" /\ VoteRoundAdmissible(node, vote))
      => vote.view = nodeView[node]
BY DEF VoteRoundAdmissible

THEOREM CommitVoteAdmissionIsExactLockedCommit ==
  \A node, vote:
    (vote.phase = "Commit" /\ VoteRoundAdmissible(node, vote))
      => LockedPrepareRound(node, vote.view, vote.subject)
BY DEF VoteRoundAdmissible

THEOREM CommitFormationIsExactLockedRound ==
  \A node, roundView, subject:
    CommitRoundAdmissible(node, roundView, subject)
      => LockedPrepareRound(node, roundView, subject)
BY DEF CommitRoundAdmissible


(***************************************************************************
Historical locked-Commit continuations moved intact from the preceding shard
so that its local theorem inventory remains within the aggregate release cap.
These statements depend only on the imported epoch proof surface; no theorem
or transition is weakened by the physical split.
***************************************************************************)
THEOREM HistoricalVoteAdmissionIsExactLockedCommit ==
  \A node, vote:
    (VoteRoundAdmissible(node, vote)
      /\ vote.view # nodeView[node])
      => /\ vote.phase = "Commit"
         /\ LockedPrepareRound(node, vote.view, vote.subject)
BY CommitVoteAdmissionIsExactLockedCommit

THEOREM HistoricalCommitFormationIsExactLockedRound ==
  \A node, roundView, subject:
    (CommitRoundAdmissible(node, roundView, subject)
      /\ roundView # nodeView[node])
      => LockedPrepareRound(node, roundView, subject)
BY CommitFormationIsExactLockedRound

THEOREM HistoricalLockedCommitUsesProgressReserve ==
  \A item:
    HistoricalLockedCommitItem(item)
      => DeliveryClass(item) = "Progress"
BY DEF DeliveryClass

(***************************************************************************
Executing a scheduled historical BeginLockCommit may select a different
valid Prepare QcRecord than the candidate's concrete evidence when both
records have the same production CertificateRef.  The action persists the
selected exact record, while progress ownership transfers by the stable
Prepare reference.  StrongInductiveInvariant supplies the redundant
`height = context.height` fact for both authenticated QCs; coordinate matching
alone would not establish the full reference over the broad QcRecord carrier.
***************************************************************************)
THEOREM HistoricalBeginLockExecutionCreatesSameRefPending ==
  \A node \in ValidatorIds, sourceQc \in QcRecordSet,
     command \in AsyncCandidateSet:
    /\ StrongInductiveInvariant
    /\ HistoricalLockedPrepareForCommit(node, sourceQc)
    /\ HistoricalBeginLockRecoveryCandidate(node, sourceQc, command)
    /\ ExecuteCommand(command)
    => \E request \in pendingLockCommit':
         /\ request.node = node
         /\ SamePrepareRecoveryRef(request.qc, sourceQc)
PROOF
  <1>1. ASSUME NEW node \in ValidatorIds,
                NEW sourceQc \in QcRecordSet,
                NEW command \in AsyncCandidateSet,
                StrongInductiveInvariant,
                HistoricalLockedPrepareForCommit(node, sourceQc),
                HistoricalBeginLockRecoveryCandidate(
                  node, sourceQc, command),
                ExecuteCommand(command)
         PROVE \E request \in pendingLockCommit':
                 /\ request.node = node
                 /\ SamePrepareRecoveryRef(request.qc, sourceQc)
    <2>1. PICK selectedQc \in LockCommitQcValues:
             /\ CommandMatches(command, command.node,
                               selectedQc.view, selectedQc.subject)
             /\ BeginLockCommit(command.node, selectedQc)
      BY <1>1, IsaT(60)
         DEF HistoricalBeginLockRecoveryCandidate,
             ExecuteCommand, ExecuteRegularCommand, RegularCoreCommand
    <2>2. /\ command.node = node
           /\ command.view = sourceQc.view
           /\ command.subject = sourceQc.subject
      BY <1>1 DEF HistoricalBeginLockRecoveryCandidate
    <2>3. /\ selectedQc.context = context
           /\ selectedQc.phase = "Prepare"
           /\ pendingLockCommit' =
                pendingLockCommit
                  \cup {LockCommitWal(
                          command.node, selectedQc,
                          Vote(context, selectedQc.view, "Commit",
                               selectedQc.subject, command.node))}
      BY <2>1 DEF BeginLockCommit
    <2>4. selectedQc \in prepareQCs
      BY <1>1, <2>1, IsaT(90)
         DEF StrongInductiveInvariant, ReducerProvenanceInvariant,
             LineageInvariant, QcTransportBacked,
             CertificatePhasesCorrect, LockCommitQcValues,
             ReceivedQcValues, CurrentOpenPrepareForCommit,
             HistoricalLockedPrepareForCommit,
             HistoricalLockedPrepareSource, LockedPrepareRecoverySource,
             BeginLockCommit
    <2>5. /\ sourceQc.context = context
           /\ sourceQc \in prepareQCs
           /\ sourceQc.height = sourceQc.context.height
           /\ selectedQc.height = selectedQc.context.height
           /\ selectedQc \in QcRecordSet
      BY <1>1, <2>4, IsaT(90)
         DEF StrongInductiveInvariant, Safety, TypeInvariant,
             ReducerProvenanceInvariant, CertificatesBackedByIntents,
             HistoricalQcValid, HistoricalLockedPrepareForCommit,
             HistoricalLockedPrepareSource, LockedPrepareRecoverySource
    <2>6. SamePrepareRecoveryRef(selectedQc, sourceQc)
      BY <1>1, <2>1, <2>2, <2>3, <2>5, SMT
         DEF CommandMatches, SamePrepareRecoveryRef,
             SameCertificateRef, CertificateRefOf
    <2> DEFINE SelectedVote ==
           Vote(context, selectedQc.view, "Commit",
                selectedQc.subject, command.node)
    <2> DEFINE SelectedRequest ==
           LockCommitWal(command.node, selectedQc, SelectedVote)
    <2>7. /\ SelectedRequest \in pendingLockCommit'
           /\ SelectedRequest.node = node
           /\ SamePrepareRecoveryRef(SelectedRequest.qc, sourceQc)
      BY <2>2, <2>3, <2>6, Isa
         DEF SelectedRequest, SelectedVote, LockCommitWal
    <2> QED BY <2>7
  <1> QED BY <1>1

(***************************************************************************
The imported historical-lock witness begins only after the locked body has
been durably validated.  Keep the earlier certified-body pipeline visible as
a separate, source-neutral obligation.  A scheduled occurrence counts only
for the current consumer epoch; an outstanding request retains one concrete
QcRecord and is matched to the source by its full stable Prepare reference.
Exact wire authentication and exact WAL bytes remain cross-tool obligations;
the reference quotient itself is explicit above.  This invariant is specified
below but intentionally not added to the proved progress bundle: preservation
across every fetch/serve/ingress/store/validate transition remains proof debt.
***************************************************************************)

HistoricalLockedSemanticPrepareAuthority(node, qc, authorityQc) ==
  /\ HistoricalLockedPrepareSource(node, authorityQc)
  /\ authorityQc.context = qc.context
  /\ authorityQc.view = qc.view
  /\ authorityQc.subject = qc.subject

HistoricalLockedCertifiedRequestMatches(node, qc, request) ==
  /\ request.kind = "CertifiedRequest"
  /\ request.source = node
  /\ request.envelope.height = qc.context.height
  /\ request.envelope.view = qc.view
  /\ request.envelope.subject = qc.subject
  /\ \E authorityQc \in prepareQCs:
       /\ HistoricalLockedSemanticPrepareAuthority(
            node, qc, authorityQc)
       /\ request.envelope.recipient
            \in authorityQc.signers \ {node}

HistoricalLockedCertifiedResponseMatches(node, qc, item) ==
  /\ item.kind = "CertifiedResponse"
  /\ item.envelope.recipient = node
  /\ item.envelope.height = qc.context.height
  /\ item.envelope.view = qc.view
  /\ item.envelope.subject = qc.subject
  /\ \E authorityQc \in prepareQCs:
       /\ HistoricalLockedSemanticPrepareAuthority(
            node, qc, authorityQc)
       /\ item.source \in authorityQc.signers

HistoricalLockedBodyPipelineCandidate(node, qc, candidate) ==
  /\ candidate \in AsyncCandidateSet
  /\ candidate.class = "Completion"
  /\ candidate.node = node
  /\ candidate.height = qc.context.height
  /\ candidate.view = qc.view
  /\ candidate.subject = qc.subject
  /\ candidate.kind \in
       {"FetchBody", "RequestCertifiedBody", "FetchCertifiedBody",
        "StoreBody", "ValidateBody"}
  /\ CASE candidate.kind \in {"FetchBody", "RequestCertifiedBody"} ->
            /\ candidate.item = NoAsyncItem
            /\ candidate.evidence \in prepareQCs
            /\ HistoricalLockedSemanticPrepareAuthority(
                 node, qc, candidate.evidence)
       [] candidate.kind = "FetchCertifiedBody" ->
            HistoricalLockedCertifiedResponseMatches(
              node, qc, candidate.item)
       [] OTHER -> TRUE
  /\ CandidateConsumerCurrent(candidate)
  /\ CandidateScheduled(candidate)

HistoricalLockedBodyRecoveryAuthority(node, qc) ==
  /\ asyncRecoveryPhase
       \in {"RestartRequired", "ReplayRequired", "Replaying"}
  /\ asyncRecoveryNode = node
  /\ generation[node] = asyncRecoveryGeneration
  /\ HistoricalLockedPrepareSource(node, qc)

HistoricalLockedCertifiedRequestActive(node, qc) ==
  \E request \in asyncActiveRequests:
    HistoricalLockedCertifiedRequestMatches(node, qc, request)

HistoricalLockedBodyPipelineKindOwned(node, qc, kind) ==
  \E candidate \in AsyncCandidateSet:
    /\ candidate.kind = kind
    /\ HistoricalLockedBodyPipelineCandidate(node, qc, candidate)

HistoricalLockedBodyFetchOwned(node, qc) ==
  HistoricalLockedBodyPipelineKindOwned(node, qc, "FetchBody")

HistoricalLockedBodyRequestOwned(node, qc) ==
  HistoricalLockedBodyPipelineKindOwned(
    node, qc, "RequestCertifiedBody")

HistoricalLockedBodyCertifiedFetchOwned(node, qc) ==
  HistoricalLockedBodyPipelineKindOwned(
    node, qc, "FetchCertifiedBody")

HistoricalLockedBodyStoreOwned(node, qc) ==
  /\ BodyRecord(node, qc.context, qc.view, qc.subject)
       \in availableBodies
  /\ HistoricalLockedBodyPipelineKindOwned(node, qc, "StoreBody")

HistoricalLockedBodyValidateOwned(node, qc) ==
  /\ BodyHeldBy(durableBodies, node, qc.context, qc.view, qc.subject)
  /\ HistoricalLockedBodyPipelineKindOwned(node, qc, "ValidateBody")

HistoricalLockedBodyServeOwned(node, qc) ==
  \E server \in ValidatorIds:
    \E job \in SequenceSet(asyncIoQueues[server]):
      /\ job \in AsyncServeJobSet
      /\ HistoricalLockedCertifiedRequestMatches(
           node, qc, job.candidate.item)

HistoricalLockedBodyResponseInFlight(node, qc) ==
  \E item \in AsyncNetworkItems:
    /\ HistoricalLockedCertifiedResponseMatches(node, qc, item)
    /\ \/ \E packet \in asyncTransport: packet.item = item
       \/ \E source \in AsyncIngressSources:
            item \in SequenceSet(IngressLane(node, source))

HistoricalLockedBodyRestartAuthority(node, qc) ==
  AsyncHistoricalLockRestartAuthority(node, qc)
    \in asyncHistoricalLockRestartAuthorities

(***************************************************************************
Validation is the terminal boundary of the ordinary body-recovery cone, not
an unconditional promise to cast a late historical Commit.  If the exact
lock remains eligible for its old-round Commit, the existing progress-witness
invariant must already own that Commit continuation.  A higher conflicting
Prepare legitimately fences the late Commit, but it does not undo the durable
validation which the later locked-body reproposal obligation consumes.
***************************************************************************)

HistoricalLockedBodyValidated(node, qc) ==
  /\ BodyHeldBy(durableBodies, node, qc.context, qc.view, qc.subject)
  /\ BodyValidatedBy(validatedBodies, node, qc.context, qc.view,
                      generation[node], qc.subject)

HistoricalLockedBodyRecoveryTerminal(node, qc) ==
  /\ HistoricalLockedBodyValidated(node, qc)
  /\ \/ HistoricalLockedCommitRecoveryWitness(node, qc)
     \/ ~HistoricalLockedPrepareForCommit(node, qc)

HistoricalLockedBodyRecoveryStage(node, qc) ==
  \/ HistoricalLockedBodyRecoveryTerminal(node, qc)
  \/ HistoricalLockedCommitRecoveryWitness(node, qc)
  \/ HistoricalLockedBodyRecoveryAuthority(node, qc)
  \/ HistoricalLockedCertifiedRequestActive(node, qc)
  \/ HistoricalLockedBodyRestartAuthority(node, qc)
  \/ HistoricalLockedBodyFetchOwned(node, qc)
  \/ HistoricalLockedBodyRequestOwned(node, qc)
  \/ HistoricalLockedBodyCertifiedFetchOwned(node, qc)
  \/ HistoricalLockedBodyStoreOwned(node, qc)
  \/ HistoricalLockedBodyValidateOwned(node, qc)

HistoricalLockedBodyRecoveryStageInvariant ==
  \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
    HistoricalLockedPrepareSource(node, qc)
      => HistoricalLockedBodyRecoveryStage(node, qc)

HistoricalLockedBodyRecoveryProperty(specification) ==
  specification => []HistoricalLockedBodyRecoveryStageInvariant

THEOREM HistoricalLockRestartAuthorityEstablishesRecoveryStage ==
  \A node \in ValidatorIds, qc \in prepareQCs:
    HistoricalLockedBodyRestartAuthority(node, qc)
      => HistoricalLockedBodyRecoveryStage(node, qc)
BY DEF HistoricalLockedBodyRecoveryStage

THEOREM HistoricalLockRestartAuthorityRetirementRequiresExactFetch ==
  \A authority \in asyncHistoricalLockRestartAuthorities:
    /\ AsyncHistoricalLockRestartAuthorityTransition
    /\ HistoricalLockRestartAuthoritySourceAfter(authority)
    /\ authority \notin asyncHistoricalLockRestartAuthorities'
    => HistoricalLockRestartExactCurrentFetchOwnerAfter(authority)
BY Isa
   DEF AsyncHistoricalLockRestartAuthorityTransition,
       ResponsiveCrashRecoveryRegistration,
       HistoricalLockRestartAuthoritySourceAfter,
       HistoricalLockRestartExactCurrentFetchOwnerAfter

THEOREM HistoricalLockRestartAuthoritySurvivesGenerationAndReplayReset ==
  \A authority \in asyncHistoricalLockRestartAuthorities:
    /\ AsyncHistoricalLockRestartAuthorityTransition
    /\ HistoricalLockRestartAuthoritySourceAfter(authority)
    /\ ~HistoricalLockRestartExactCurrentFetchOwnerAfter(authority)
    => authority \in asyncHistoricalLockRestartAuthorities'
BY Isa
   DEF AsyncHistoricalLockRestartAuthorityTransition,
       ResponsiveCrashRecoveryRegistration,
       HistoricalLockRestartAuthoritySourceAfter,
       HistoricalLockRestartExactCurrentFetchOwnerAfter

THEOREM ResponsiveCrashRegistersExactHistoricalLockProjection ==
  \A node \in ValidatorIds, qc \in prepareQCs:
    /\ PreGstResponsiveCrash(node)
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ AsyncNext
    => AsyncHistoricalLockRestartAuthority(node, qc)
         \in asyncHistoricalLockRestartAuthorities'
BY Isa
   DEF AsyncNext, AsyncHistoricalLockRestartAuthorityTransition,
       ResponsiveCrashRecoveryRegistration,
       ResponsiveCrashHistoricalLockRestartAuthorities,
       HistoricalLockRestartAuthoritySourceKernel,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoDecisionForNode, PreGstResponsiveCrash

(***************************************************************************
Action-by-action ownership handoffs for the ordinary historical locked-body
cone.  `HistoricalLockedBodyRuntimeExecutes` names only a successful removal
from one of the two serialized reducer carriers; a blocked command remains in
its exact runtime/deferred owner and is handled by the preservation lemmas
below.  The request and response selectors name the actual fair-ingress item,
and Serve ownership names the remote signer's queue rather than the requesting
validator's queue.
***************************************************************************)

HistoricalLockedBodySourceRetired(node, qc) ==
  ~HistoricalLockedPrepareSource(node, qc)

HistoricalLockedBodyRuntimeExecutes(candidate) ==
  /\ [AsyncNext]_AsyncAllVars
  /\ \/ /\ candidate = NextNodeCommand(candidate.node)
           /\ FifoRuntimeStep(candidate.node)
           /\ CommandDispatchable(candidate)
     \/ /\ DeferredQueueNonempty(candidate.node)
           /\ candidate = NextDeferredCommand(candidate.node)
           /\ DeferredDrainStep(candidate.node)
           /\ DeferredHandoffAllowsExecution(candidate.node, candidate)

HistoricalLockedCertifiedRequestSelected(server, node, qc) ==
  /\ asyncIngressReady[server] # <<>>
  /\ DrainableIngressIndices(server) # {}
  /\ HistoricalLockedCertifiedRequestMatches(
       node, qc,
       SelectedIngressItemAt(
         server, FirstDrainableIngressIndex(server)))

HistoricalLockedCertifiedResponseSelected(node, qc) ==
  /\ asyncIngressReady[node] # <<>>
  /\ DrainableIngressIndices(node) # {}
  /\ HistoricalLockedCertifiedResponseMatches(
       node, qc,
       SelectedIngressItemAt(
         node, FirstDrainableIngressIndex(node)))

HistoricalLockedBodyServeHeadOwned(server, node, qc) ==
  /\ AsyncIoQueueDepth(server) > 0
  /\ Head(asyncIoQueues[server]) \in AsyncServeJobSet
  /\ HistoricalLockedCertifiedRequestMatches(
       node, qc, Head(asyncIoQueues[server]).candidate.item)

THEOREM HistoricalLockedFetchExecutionHandsOff ==
  \A node \in ValidatorIds, qc \in prepareQCs,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ candidate.kind = "FetchBody"
    /\ HistoricalLockedBodyPipelineCandidate(node, qc, candidate)
    /\ HistoricalLockedBodyRuntimeExecutes(candidate)
    => \/ HistoricalLockedBodySourceRetired(node, qc)'
       \/ HistoricalLockedCertifiedRequestActive(node, qc)'
       \/ HistoricalLockedBodyValidateOwned(node, qc)'
       \/ HistoricalLockedBodyRecoveryTerminal(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedBodyRuntimeExecutes,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedSemanticPrepareAuthority,
       HistoricalLockedBodySourceRetired,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedCertifiedRequestMatches,
       HistoricalLockedBodyValidateOwned,
       HistoricalLockedBodyPipelineKindOwned,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       ExecuteDecisionFetch, PublishCertifiedRequests,
       CertifiedRequestOutbox, CommandSuccessors,
       FreshCommandSuccessors, FreshCandidateSequence,
       AppendCausalSuccessors, FifoRuntimeStep, DeferredDrainStep,
       HistoricalLockedPrepareSource, CertifiedBodyRecoveryAuthority,
       CandidateScheduled, CandidateConsumerCurrent,
       AsyncAllVars

THEOREM HistoricalLockedRequestExecutionHandsOff ==
  \A node \in ValidatorIds, qc \in prepareQCs,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ candidate.kind = "RequestCertifiedBody"
    /\ HistoricalLockedBodyPipelineCandidate(node, qc, candidate)
    /\ HistoricalLockedBodyRuntimeExecutes(candidate)
    => \/ HistoricalLockedBodySourceRetired(node, qc)'
       \/ HistoricalLockedCertifiedRequestActive(node, qc)'
       \/ HistoricalLockedBodyRecoveryTerminal(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedBodyRuntimeExecutes,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedSemanticPrepareAuthority,
       HistoricalLockedBodySourceRetired,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedCertifiedRequestMatches,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       ExecuteRequestCertifiedBody, PublishCertifiedRequests,
       CertifiedRequestOutbox, FifoRuntimeStep, DeferredDrainStep,
       HistoricalLockedPrepareSource, CertifiedBodyRecoveryAuthority,
       CandidateScheduled, CandidateConsumerCurrent,
       AsyncAllVars

THEOREM HistoricalLockedCertifiedFetchExecutionHandsOff ==
  \A node \in ValidatorIds, qc \in prepareQCs,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ candidate.kind = "FetchCertifiedBody"
    /\ HistoricalLockedBodyPipelineCandidate(node, qc, candidate)
    /\ HistoricalLockedBodyRuntimeExecutes(candidate)
    => \/ HistoricalLockedBodySourceRetired(node, qc)'
       \/ HistoricalLockedBodyStoreOwned(node, qc)'
       \/ HistoricalLockedBodyRecoveryTerminal(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedBodyRuntimeExecutes,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedCertifiedResponseMatches,
       HistoricalLockedSemanticPrepareAuthority,
       HistoricalLockedBodySourceRetired,
       HistoricalLockedBodyStoreOwned,
       HistoricalLockedBodyPipelineKindOwned,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       ExecuteRegularCommand, RegularCoreCommand,
       FetchCertifiedBody, CommandSuccessors,
       FreshCommandSuccessors, FreshCandidateSequence,
       AppendCausalSuccessors, FifoRuntimeStep, DeferredDrainStep,
       HistoricalLockedPrepareSource, CandidateScheduled,
       CandidateConsumerCurrent, AsyncAllVars

THEOREM HistoricalLockedStoreExecutionHandsOff ==
  \A node \in ValidatorIds, qc \in prepareQCs,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ candidate.kind = "StoreBody"
    /\ HistoricalLockedBodyPipelineCandidate(node, qc, candidate)
    /\ HistoricalLockedBodyRuntimeExecutes(candidate)
    => \/ HistoricalLockedBodySourceRetired(node, qc)'
       \/ HistoricalLockedBodyValidateOwned(node, qc)'
       \/ HistoricalLockedBodyRecoveryTerminal(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedBodyRuntimeExecutes,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodySourceRetired,
       HistoricalLockedBodyValidateOwned,
       HistoricalLockedBodyPipelineKindOwned,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       ExecuteRegularCommand, RegularCoreCommand, StoreBody,
       CommandSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, AppendCausalSuccessors,
       FifoRuntimeStep, DeferredDrainStep,
       HistoricalLockedPrepareSource, CandidateScheduled,
       CandidateConsumerCurrent, AsyncAllVars

THEOREM HistoricalLockedValidateExecutionHandsOff ==
  \A node \in ValidatorIds, qc \in prepareQCs,
     candidate \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ candidate.kind = "ValidateBody"
    /\ HistoricalLockedBodyPipelineCandidate(node, qc, candidate)
    /\ HistoricalLockedBodyRuntimeExecutes(candidate)
    => \/ HistoricalLockedBodySourceRetired(node, qc)'
       \/ HistoricalLockedBodyRecoveryTerminal(node, qc)'
       \/ HistoricalLockedCommitRecoveryWitness(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedBodyRuntimeExecutes,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodySourceRetired,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       HistoricalLockedCommitRecoveryWitness,
       ExecuteRegularCommand, RegularCoreCommand, ValidateLockedBody,
       CommandSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, AppendCausalSuccessors,
       FifoRuntimeStep, DeferredDrainStep,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareForCommit,
       CandidateScheduled, CandidateConsumerCurrent,
       AsyncAllVars

THEOREM HistoricalLockedRequestIngressHandsOffToRemoteServe ==
  \A server, node \in ValidatorIds, qc \in prepareQCs:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ HistoricalLockedCertifiedRequestActive(node, qc)
    /\ HistoricalLockedCertifiedRequestSelected(server, node, qc)
    /\ [AsyncNext]_AsyncAllVars
    /\ IngressDrainStep(server)
    => /\ HistoricalLockedCertifiedRequestActive(node, qc)'
       /\ HistoricalLockedBodyServeOwned(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedCertifiedRequestSelected,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedCertifiedRequestMatches,
       HistoricalLockedBodyServeOwned,
       IngressDrainStep, DrainFairIngressSelected,
       CertifiedRequestAuthorized, AsyncIoCertifiedServeJob,
       CandidateConsumerCurrent, SequenceSet,
       AsyncAllVars

THEOREM HistoricalLockedServeExecutionPublishesResponse ==
  \A server, node \in ValidatorIds, qc \in prepareQCs:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ HistoricalLockedCertifiedRequestActive(node, qc)
    /\ HistoricalLockedBodyServeHeadOwned(server, node, qc)
    /\ CertifiedServeCanRespond(
         server, Head(asyncIoQueues[server]).candidate.item)
    /\ [AsyncNext]_AsyncAllVars
    /\ ServiceIoWorkerWork(server)
    => /\ HistoricalLockedCertifiedRequestActive(node, qc)'
       /\ HistoricalLockedBodyResponseInFlight(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedBodyServeHeadOwned,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedCertifiedRequestMatches,
       HistoricalLockedBodyResponseInFlight,
       HistoricalLockedCertifiedResponseMatches,
       HistoricalLockedSemanticPrepareAuthority,
       ServiceIoWorkerWork, CertifiedServeCanRespond,
       CertifiedResponseItem, PublishEphemeralItems,
       PacketsForItems, AsyncAllVars

THEOREM HistoricalLockedResponseIngressHandsOffToCertifiedFetch ==
  \A node \in ValidatorIds, qc \in prepareQCs:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ HistoricalLockedCertifiedRequestActive(node, qc)
    /\ HistoricalLockedCertifiedResponseSelected(node, qc)
    /\ [AsyncNext]_AsyncAllVars
    /\ IngressDrainStep(node)
    => \/ HistoricalLockedBodySourceRetired(node, qc)'
       \/ HistoricalLockedBodyCertifiedFetchOwned(node, qc)'
       \/ HistoricalLockedBodyRecoveryTerminal(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedCertifiedResponseSelected,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedCertifiedRequestMatches,
       HistoricalLockedCertifiedResponseMatches,
       HistoricalLockedSemanticPrepareAuthority,
       HistoricalLockedBodySourceRetired,
       HistoricalLockedBodyCertifiedFetchOwned,
       HistoricalLockedBodyPipelineKindOwned,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       IngressDrainStep, DrainFairIngressSelected,
       CertifiedResponseAuthorized, MatchingCertifiedRequests,
       CertifiedResponseCandidate, CandidateScheduled,
       CandidateConsumerCurrent, AsyncAllVars

THEOREM HistoricalLockedPrepareSourceRetiresOnlyLegitimately ==
  \A node \in ValidatorIds, qc \in prepareQCs:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalLockedBodySourceRetired(node, qc)'
    => \/ NodeHasDecision(node)'
       \/ lockRank[node]' > qc.view
       \/ lockSubject[node]' # qc.subject
BY IsaT(180)
   DEF HistoricalLockedBodySourceRetired,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoDecisionForNode, AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep, AsyncAllVars

(***************************************************************************
Preservation closes two different proof cases.  An existing durable source
must retain its current owner or take one of the concrete handoffs above.  A
new source can arise only when the reducer durably installs the selected TC
lock (which atomically appends its semantic FetchBody owner) or persists the
old-round Commit intent (which is already terminal).  Keeping those cases
separate prevents a proof from assuming the source in the pre-state and then
vacuously ignoring the install transition which creates it.
***************************************************************************)

THEOREM HistoricalLockedPersistInstallEstablishesSemanticFetch ==
  \A node \in ValidatorIds, qc \in prepareQCs,
     command \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ command.kind = "PersistInstallTC"
    /\ command.node = node
    /\ HistoricalLockedBodyRuntimeExecutes(command)
    /\ HistoricalLockedPrepareSource(node, qc)'
    => \/ HistoricalLockedBodyFetchOwned(node, qc)'
       \/ HistoricalLockedCommitRecoveryWitness(node, qc)'
       \/ HistoricalLockedBodyRecoveryTerminal(node, qc)'
BY IsaT(240)
   DEF HistoricalLockedBodyRuntimeExecutes,
       HistoricalLockedBodyFetchOwned,
       HistoricalLockedBodyPipelineKindOwned,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedSemanticPrepareAuthority,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       ExecutePersistInstall, PersistInstallTC,
       InstallResultingLockedPrepareQCs,
       InstallLockedFetchSuccessor,
       InstallLockedFetchSuccessors,
       InstallCommitSignSuccessors, InstallCommandSuccessors,
       CommandSuccessors, FreshCommandSuccessors,
       FreshCandidateSequence, AppendCausalSuccessors,
       CandidateScheduled, CandidateConsumerCurrent,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       AsyncAllVars

THEOREM HistoricalLockedPersistCommitEstablishesTerminalWitness ==
  \A node \in ValidatorIds, qc \in prepareQCs,
     command \in AsyncCandidateSet:
    /\ AsyncStrongTypeInvariant
    /\ command.kind = "PersistLockCommit"
    /\ command.node = node
    /\ command.view = qc.view
    /\ command.subject = qc.subject
    /\ HistoricalLockedBodyRuntimeExecutes(command)
    /\ HistoricalLockedPrepareSource(node, qc)'
    => HistoricalLockedCommitRecoveryWitness(node, qc)'
BY IsaT(180)
   DEF HistoricalLockedBodyRuntimeExecutes,
       HistoricalLockedCommitRecoveryWitness,
       ExecuteRegularCommand, RegularCoreCommand,
       PersistLockCommit, ExactLockedCommitIntents,
       HistoricalLockedPrepareSource,
       CandidateScheduled, AsyncAllVars

THEOREM HistoricalLockedBodyExistingSourceStepPreservation ==
  \A node \in ValidatorIds, qc \in prepareQCs:
    /\ AsyncStrongTypeInvariant
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ HistoricalLockedBodyRecoveryStage(node, qc)
    /\ [AsyncNext]_AsyncAllVars
    => \/ HistoricalLockedBodySourceRetired(node, qc)'
       \/ HistoricalLockedBodyRecoveryStage(node, qc)'
BY HistoricalLockedFetchExecutionHandsOff,
   HistoricalLockedRequestExecutionHandsOff,
   HistoricalLockedCertifiedFetchExecutionHandsOff,
   HistoricalLockedStoreExecutionHandsOff,
   HistoricalLockedValidateExecutionHandsOff,
   HistoricalLockedRequestIngressHandsOffToRemoteServe,
   HistoricalLockedServeExecutionPublishesResponse,
   HistoricalLockedResponseIngressHandsOffToCertifiedFetch,
   HistoricalLockRestartAuthorityRetirementRequiresExactFetch,
   HistoricalLockRestartAuthoritySurvivesGenerationAndReplayReset,
   ResponsiveCrashRegistersExactHistoricalLockProjection,
   IsaT(300)
   DEF HistoricalLockedBodyRecoveryStage,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       HistoricalLockedCertifiedRequestActive,
       HistoricalLockedBodyRestartAuthority,
       HistoricalLockedBodyFetchOwned,
       HistoricalLockedBodyRequestOwned,
       HistoricalLockedBodyCertifiedFetchOwned,
       HistoricalLockedBodyStoreOwned,
       HistoricalLockedBodyValidateOwned,
       HistoricalLockedBodyPipelineKindOwned,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedBodySourceRetired,
       HistoricalLockedCommitRecoveryWitness,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       RunHistoricalServer, OpenHistoricalRecovery,
       DirectCommitCertificateDiscoveryStep,
       DirectHistoricalCommitCertificateDiscoveryStep,
       ServiceIoWorker, ServiceHistoricalRecoveryIoWorker,
       EnqueueIoLocalControl, EnqueueHistoricalRecoveryIoLocalControl,
       AsyncNetworkStep, AdmitIngressPacket, AsyncFaultStep,
       PreGstCrash, PreGstResponsiveCrash, PreGstResponsiveRestart,
       PreGstResponsiveReplay, DriveResponsiveReplayHead,
       FinishResponsiveReplay, RearmResponsiveRecovery,
       AsyncTick, AsyncAllVars

THEOREM HistoricalLockedBodyNewSourceStepEstablishment ==
  \A node \in ValidatorIds, qc \in prepareQCs:
    /\ AsyncStrongTypeInvariant
    /\ ~HistoricalLockedPrepareSource(node, qc)
    /\ [AsyncNext]_AsyncAllVars
    /\ HistoricalLockedPrepareSource(node, qc)'
    => HistoricalLockedBodyRecoveryStage(node, qc)'
BY HistoricalLockedPersistInstallEstablishesSemanticFetch,
   HistoricalLockedPersistCommitEstablishesTerminalWitness,
   IsaT(300)
   DEF HistoricalLockedBodyRecoveryStage,
       HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyValidated,
       HistoricalLockedCommitRecoveryWitness,
       HistoricalLockedBodyFetchOwned,
       HistoricalLockedBodyPipelineKindOwned,
       HistoricalLockedBodyPipelineCandidate,
       HistoricalLockedSemanticPrepareAuthority,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       AsyncNext, AsyncNonCrashStep,
       AsyncRunnerStep, AsyncNonRunnerStep,
       RunNode, RunHistoricalRecoveryNode, RunNodeWork,
       SerializedRuntimeStep,
       SerializedRuntimePrecedesServeIngressStep,
       SerializedLocalPrecedesServeIngressStep,
       SelectedLocalAdmissionAdvance,
       AsyncServeIngressTargetOnlyTurn, RuntimeStep,
       FifoRuntimeStep, DeferredDrainStep,
       AsyncAllVars

THEOREM AsyncInitEstablishesHistoricalLockedBodyRecoveryStage ==
  \A initialContext:
    AsyncInitAt(initialContext)
      => HistoricalLockedBodyRecoveryStageInvariant
BY IsaT(120)
   DEF AsyncInitAt, AsyncBaseInitAt,
       HistoricalLockedBodyRecoveryStageInvariant,
       HistoricalLockedPrepareSource,
       HistoricalLockedPrepareRecoveryProvenance,
       InstalledTcSelectsPrepareFor, ExactLockedCommitIntents,
       NoDecisionForNode

THEOREM AsyncBracketPreservesHistoricalLockedBodyRecoveryStage ==
  /\ AsyncStrongTypeInvariant
  /\ HistoricalLockedBodyRecoveryStageInvariant
  /\ [AsyncNext]_AsyncAllVars
  => HistoricalLockedBodyRecoveryStageInvariant'
PROOF
  <1>1. ASSUME AsyncStrongTypeInvariant,
              HistoricalLockedBodyRecoveryStageInvariant,
              [AsyncNext]_AsyncAllVars
         PROVE HistoricalLockedBodyRecoveryStageInvariant'
    <2>1. AsyncCurrentResponsiveVoters'
             = AsyncCurrentResponsiveVoters
      BY <1>1, Isa
         DEF AsyncNext, AsyncCurrentResponsiveVoters,
             CurrentVoters, CurrentEpoch, AsyncAllVars
    <2>2. ASSUME NEW node \in AsyncCurrentResponsiveVoters',
                  NEW qc \in prepareQCs',
                  HistoricalLockedPrepareSource(node, qc)'
           PROVE HistoricalLockedBodyRecoveryStage(node, qc)'
      <3>1. qc \in prepareQCs
        BY <1>1, <2>2, Isa
           DEF AsyncNext, AsyncAllVars
      <3>2. CASE HistoricalLockedPrepareSource(node, qc)
        <4>1. HistoricalLockedBodyRecoveryStage(node, qc)
          BY <1>1, <2>1, <2>2, <3>2, <3>1
             DEF HistoricalLockedBodyRecoveryStageInvariant
        <4>2. \/ HistoricalLockedBodySourceRetired(node, qc)'
               \/ HistoricalLockedBodyRecoveryStage(node, qc)'
          BY <1>1, <3>1, <3>2, <4>1,
             HistoricalLockedBodyExistingSourceStepPreservation
        <4>3. ~HistoricalLockedBodySourceRetired(node, qc)'
          BY <2>2 DEF HistoricalLockedBodySourceRetired
        <4> QED BY <4>2, <4>3
      <3>3. CASE ~HistoricalLockedPrepareSource(node, qc)
        BY <1>1, <2>2, <3>1, <3>3,
           HistoricalLockedBodyNewSourceStepEstablishment
      <3> QED BY <3>2, <3>3
    <2> QED BY <2>2
         DEF HistoricalLockedBodyRecoveryStageInvariant
  <1> QED BY <1>1
THEOREM HistoricalHigherConflictValidationIsTerminal ==
  \A node \in AsyncCurrentResponsiveVoters, qc \in prepareQCs:
    /\ HistoricalLockedPrepareSource(node, qc)
    /\ BodyHeldBy(durableBodies, node, context, qc.view, qc.subject)
    /\ BodyValidatedBy(validatedBodies, node, context, qc.view,
                       generation[node], qc.subject)
    /\ ~NoHigherConflictingPrepareKnown(node, qc)
      => /\ HistoricalLockedBodyRecoveryTerminal(node, qc)
         /\ HistoricalLockedBodyRecoveryStage(node, qc)
BY DEF HistoricalLockedBodyRecoveryTerminal,
       HistoricalLockedBodyRecoveryStage

=============================================================================
