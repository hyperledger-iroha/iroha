---- MODULE SumeragiV2TypedRolloverHandoff ----
EXTENDS Naturals, TLC

(***************************************************************************
Executable safety model for the move-only exact-output handoff and the sole
first-release merge-sidecar lifecycle schema.

The handoff receipt never grants permission to bypass lifecycle terminality.
A responder generation may advance only for a full unified server-stream
table or a certified roster-geometry replacement, and only after every old
server stream, request gate, transfer, and flush is terminal.  Active-state
exhaustion and checked-counter overflow return Capacity without mutating the
generation, the two responder tables, or the durable snapshot.

There is one durable object: LifecycleSnapshotV2.  It contains the responder
generation, next requester epoch, unified server-stream table, and request-gate
table.  Generation rollover atomically persists the successor generation and
both empty responder tables before memory publication.  A crash may therefore
leave the sole V2 snapshot ahead of memory; restart restores that complete
snapshot.  There is no separate generation marker or high-water artifact.

Requester epochs are persisted by advancing nextStreamEpoch before an epoch is
published for use.  A crash after that persistence discards the unpublished
epoch and restores the advanced counter, so the epoch is skipped, never reused.

The temporal predicates at the end of this module remain specification targets.
No liveness theorem is claimed by this model family.
***************************************************************************)

NoIdentity == "NoIdentity"

RosterRelations == {"SameRoster", "ChangedRoster"}
CompactionCauses ==
  {"NoCompaction", "FullServerTable", "RosterGeometryReplacement"}
LifecycleEntryStates == {"Active", "Terminal", "Empty"}
ReceiptStages == {"Absent", "Minted", "Retained", "Consumed"}
TransitionAuthorities == {"None", "SameRoster", "LifecycleV2"}
LifecycleSnapshotPhases == {"Current", "Persisted", "Restored", "Published"}
RequesterEpochPhases == {"Idle", "Persisted", "InUse", "Completed"}

FailureReasons ==
  {"None",
   "LateExactOutputEnqueue",
   "ForeignOwnerMismatch",
   "PredecessorContextMismatch",
   "PredecessorArtifactMismatch",
   "ImmediateSuccessorMismatch",
   "LifecycleSnapshotV2PersistenceFailure",
   "CrashBeforeLifecycleSnapshotV2Persistence",
   "CrashAfterLifecycleSnapshotV2Persistence",
   "CrashAfterRequesterEpochPersistence"}

OwnerNonces == {"OwnerNonce", "ForeignNonce"}
Contexts == {"ContextA", "ContextB"}
Artifacts == {"ArtifactA", "ArtifactB"}
Parents == {"ParentA", "ParentB"}
Successors == {"SuccessorA", "SuccessorB"}

ServiceOwnerNonce == "OwnerNonce"
TransportOwnerNonce == "OwnerNonce"
ForeignOwnerNonce == "ForeignNonce"
ExpectedContext == "ContextA"
ExpectedArtifact == "ArtifactA"
ExpectedParent == "ParentA"
ExpectedSuccessor == "SuccessorA"
ForeignSuccessor == "SuccessorB"

InitialWorkerOutstanding == 2
InitialGeneration == 1
GenerationLimit == 2
InitialNextStreamEpoch == 1
StreamEpochLimit == 3

LifecycleSnapshotV2(generation, nextStreamEpoch, serverStreams, requestGates) ==
  [version |-> 2,
   generation |-> generation,
   nextStreamEpoch |-> nextStreamEpoch,
   serverStreams |-> serverStreams,
   requestGates |-> requestGates]

LifecycleSnapshotV2Set ==
  [version: {2},
   generation: InitialGeneration..GenerationLimit,
   nextStreamEpoch: InitialNextStreamEpoch..StreamEpochLimit,
   serverStreams: LifecycleEntryStates,
   requestGates: LifecycleEntryStates]

VARIABLE state

typedRolloverVars == <<state>>

LifecycleMemory(s) ==
  <<s.serviceGeneration,
    s.nextStreamEpoch,
    s.serverStreamState,
    s.requestGateState,
    s.transferState,
    s.flushState>>

DurableLifecycle(s) == s.durableLifecycleSnapshotV2

ExactServiceTransportOwnerPair ==
  ServiceOwnerNonce = TransportOwnerNonce

ExactPredecessorReceipt ==
  /\ state.receiptOwnerNonce = ServiceOwnerNonce
  /\ state.receiptContext = ExpectedContext
  /\ state.receiptArtifact = ExpectedArtifact

ExactSuccessorConstruction ==
  /\ state.constructionParent = ExpectedParent
  /\ state.constructionSuccessor = ExpectedSuccessor

ExactRetainedMergeSidecars ==
  /\ state.receiptStage = "Retained"
  /\ ExactPredecessorReceipt
  /\ ExactSuccessorConstruction
  /\ state.retainedSuccessor = ExpectedSuccessor

FinalExactOutputSeal ==
  /\ state.finalityValidated
  /\ state.workerIngressClosed
  /\ state.workerOutstanding = 0
  /\ state.ownerSealed

CompactionNeeded ==
  state.compactionCause \in
    {"FullServerTable", "RosterGeometryReplacement"}

CompactionCauseMatchesGeometry ==
  /\ (state.targetRoster = "ChangedRoster" =>
        state.compactionCause = "RosterGeometryReplacement")
  /\ (state.compactionCause = "RosterGeometryReplacement" =>
        state.targetRoster = "ChangedRoster")
  /\ (state.compactionCause = "FullServerTable" =>
        state.targetRoster = "SameRoster")

AllOldLifecycleTerminal ==
  /\ state.serverStreamState = "Terminal"
  /\ state.requestGateState = "Terminal"
  /\ state.transferState = "Terminal"
  /\ state.flushState = "Terminal"

LifecycleMemoryMatchesDurableSnapshotV2 ==
  /\ state.serviceGeneration =
       state.durableLifecycleSnapshotV2.generation
  /\ state.nextStreamEpoch =
       state.durableLifecycleSnapshotV2.nextStreamEpoch
  /\ state.serverStreamState =
       state.durableLifecycleSnapshotV2.serverStreams
  /\ state.requestGateState =
       state.durableLifecycleSnapshotV2.requestGates

LifecycleSnapshotV2AheadOfMemory ==
  /\ state.lifecycleSnapshotPhase = "Persisted"
  /\ state.durableLifecycleSnapshotV2.generation =
       state.serviceGeneration + 1
  /\ state.durableLifecycleSnapshotV2.serverStreams = "Empty"
  /\ state.durableLifecycleSnapshotV2.requestGates = "Empty"

RequesterEpochPersistenceAheadOfMemory ==
  /\ state.requesterEpochPhase = "Persisted"
  /\ state.pendingStreamEpoch = state.nextStreamEpoch
  /\ state.durableLifecycleSnapshotV2.nextStreamEpoch =
       state.nextStreamEpoch + 1

Init ==
  \E roster \in RosterRelations,
     cause \in CompactionCauses:
    /\ (roster = "ChangedRoster" =>
          cause = "RosterGeometryReplacement")
    /\ (cause = "RosterGeometryReplacement" =>
          roster = "ChangedRoster")
    /\ (cause = "FullServerTable" =>
          roster = "SameRoster")
    /\ state =
         [targetRoster |-> roster,
          compactionCause |-> cause,
          finalityValidated |-> FALSE,
          workerIngressClosed |-> FALSE,
          workerOutstanding |-> InitialWorkerOutstanding,
          ownerSealed |-> FALSE,
          constructionParent |-> NoIdentity,
          constructionSuccessor |-> NoIdentity,
          receiptStage |-> "Absent",
          receiptOwnerNonce |-> NoIdentity,
          receiptContext |-> NoIdentity,
          receiptArtifact |-> NoIdentity,
          retainedSuccessor |-> NoIdentity,
          receiptConsumeCount |-> 0,
          serviceGeneration |-> InitialGeneration,
          nextStreamEpoch |-> InitialNextStreamEpoch,
          requesterEpochPhase |-> "Idle",
          pendingStreamEpoch |-> 0,
          activeStreamEpoch |-> 0,
          lastCompletedStreamEpoch |-> 0,
          skippedStreamEpoch |-> 0,
          serverStreamState |-> "Active",
          requestGateState |-> "Active",
          transferState |-> "Active",
          flushState |-> "Active",
          durableLifecycleSnapshotV2 |->
            LifecycleSnapshotV2(
              InitialGeneration,
              InitialNextStreamEpoch,
              "Active",
              "Active"),
          lifecycleSnapshotPhase |-> "Current",
          retryableChunk |-> 1,
          successorActive |-> FALSE,
          transitionAuthority |-> "None",
          capacityRejected |-> FALSE,
          restartRequired |-> FALSE,
          failureReason |-> "None",
          lateEnqueueRejected |-> FALSE,
          foreignReceiptRejected |-> FALSE,
          predecessorMismatchRejected |-> FALSE,
          wrongSuccessorRejected |-> FALSE,
          lateOldCallbackObserved |-> FALSE,
          snapshotCrashObserved |-> FALSE,
          epochCrashObserved |-> FALSE]

ValidateFinality ==
  /\ ~state.finalityValidated
  /\ state' = [state EXCEPT !.finalityValidated = TRUE]

CloseWorkerIngress ==
  /\ state.finalityValidated
  /\ ~state.workerIngressClosed
  /\ state' = [state EXCEPT !.workerIngressClosed = TRUE]

ClearOneWorkerExactOutput ==
  /\ state.workerIngressClosed
  /\ state.workerOutstanding > 0
  /\ state' =
       [state EXCEPT !.workerOutstanding = @ - 1]

BuildImmediateSuccessor ==
  /\ state.finalityValidated
  /\ state.constructionParent = NoIdentity
  /\ state.constructionSuccessor = NoIdentity
  /\ state' =
       [state EXCEPT
          !.constructionParent = ExpectedParent,
          !.constructionSuccessor = ExpectedSuccessor]

SealAppliedHeightOutputHandoff ==
  /\ state.finalityValidated
  /\ state.workerIngressClosed
  /\ state.workerOutstanding = 0
  /\ ~state.ownerSealed
  /\ state.receiptStage = "Absent"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.ownerSealed = TRUE,
          !.receiptStage = "Minted",
          !.receiptOwnerNonce = ServiceOwnerNonce,
          !.receiptContext = ExpectedContext,
          !.receiptArtifact = ExpectedArtifact]

RejectLateExactOutputEnqueue ==
  /\ state.ownerSealed
  /\ state.receiptStage = "Minted"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.lateEnqueueRejected = TRUE,
          !.restartRequired = TRUE,
          !.failureReason = "LateExactOutputEnqueue"]

RejectForeignOwnerReceipt ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ ~state.foreignReceiptRejected
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.foreignReceiptRejected = TRUE,
          !.restartRequired = TRUE,
          !.failureReason = "ForeignOwnerMismatch"]

RejectPredecessorContextMismatch ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ ~state.predecessorMismatchRejected
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.predecessorMismatchRejected = TRUE,
          !.restartRequired = TRUE,
          !.failureReason = "PredecessorContextMismatch"]

RejectPredecessorArtifactMismatch ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ ~state.predecessorMismatchRejected
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.predecessorMismatchRejected = TRUE,
          !.restartRequired = TRUE,
          !.failureReason = "PredecessorArtifactMismatch"]

RejectWrongImmediateSuccessor ==
  /\ state.receiptStage = "Minted"
  /\ ~state.wrongSuccessorRejected
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.wrongSuccessorRejected = TRUE,
          !.restartRequired = TRUE,
          !.failureReason = "ImmediateSuccessorMismatch"]

RetainExactHandoffReceipt ==
  /\ state.receiptStage = "Minted"
  /\ ExactPredecessorReceipt
  /\ ExactSuccessorConstruction
  /\ state.retainedSuccessor = NoIdentity
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.receiptStage = "Retained",
          !.retainedSuccessor = ExpectedSuccessor]

(***************************************************************************
Requester epoch allocation.  The durable V2 counter advances first.  Only the
following publication action may expose the allocated epoch for use.
***************************************************************************)
PersistFreshRequesterEpoch ==
  /\ state.requesterEpochPhase = "Idle"
  /\ state.nextStreamEpoch < StreamEpochLimit
  /\ state.lifecycleSnapshotPhase = "Current"
  /\ LifecycleMemoryMatchesDurableSnapshotV2
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.durableLifecycleSnapshotV2 =
            LifecycleSnapshotV2(
              state.serviceGeneration,
              state.nextStreamEpoch + 1,
              state.serverStreamState,
              state.requestGateState),
          !.requesterEpochPhase = "Persisted",
          !.pendingStreamEpoch = state.nextStreamEpoch,
          !.capacityRejected = FALSE]

PublishFreshRequesterEpoch ==
  /\ RequesterEpochPersistenceAheadOfMemory
  /\ ~state.restartRequired
  /\ state.pendingStreamEpoch > state.lastCompletedStreamEpoch
  /\ state.pendingStreamEpoch # state.skippedStreamEpoch
  /\ state' =
       [state EXCEPT
          !.nextStreamEpoch =
            state.durableLifecycleSnapshotV2.nextStreamEpoch,
          !.requesterEpochPhase = "InUse",
          !.activeStreamEpoch = state.pendingStreamEpoch,
          !.pendingStreamEpoch = 0]

CompleteRequesterEpoch ==
  /\ state.requesterEpochPhase = "InUse"
  /\ state.activeStreamEpoch > state.lastCompletedStreamEpoch
  /\ state' =
       [state EXCEPT
          !.requesterEpochPhase = "Completed",
          !.lastCompletedStreamEpoch = state.activeStreamEpoch,
          !.activeStreamEpoch = 0]

ReopenRequesterEpochAllocator ==
  /\ state.requesterEpochPhase = "Completed"
  /\ state' =
       [state EXCEPT !.requesterEpochPhase = "Idle"]

CrashAfterRequesterEpochPersistence ==
  /\ RequesterEpochPersistenceAheadOfMemory
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.successorActive = FALSE,
          !.restartRequired = TRUE,
          !.failureReason = "CrashAfterRequesterEpochPersistence",
          !.skippedStreamEpoch = state.pendingStreamEpoch,
          !.epochCrashObserved = TRUE]

RestoreRequesterEpochCounterAfterCrash ==
  /\ state.restartRequired
  /\ state.failureReason = "CrashAfterRequesterEpochPersistence"
  /\ RequesterEpochPersistenceAheadOfMemory
  /\ state.epochCrashObserved
  /\ state' =
       [state EXCEPT
          !.nextStreamEpoch =
            state.durableLifecycleSnapshotV2.nextStreamEpoch,
          !.requesterEpochPhase = "Idle",
          !.pendingStreamEpoch = 0,
          !.activeStreamEpoch = 0,
          !.restartRequired = FALSE,
          !.failureReason = "None"]

RejectRequesterEpochOverflow ==
  /\ state.requesterEpochPhase = "Idle"
  /\ state.nextStreamEpoch = StreamEpochLimit
  /\ ~state.capacityRejected
  /\ state' =
       [state EXCEPT !.capacityRejected = TRUE]

(***************************************************************************
Lifecycle terminalization.  Stream and gate terminality is persisted in the
same sole V2 object.  Transfer and flush ownership is process-local but must
also be terminal before compaction.
***************************************************************************)
RejectActiveLifecycleCompaction ==
  /\ ExactRetainedMergeSidecars
  /\ CompactionNeeded
  /\ ~AllOldLifecycleTerminal
  /\ ~state.capacityRejected
  /\ state.lifecycleSnapshotPhase = "Current"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT !.capacityRejected = TRUE]

TerminalizeServerStream ==
  /\ ExactRetainedMergeSidecars
  /\ CompactionNeeded
  /\ state.serverStreamState = "Active"
  /\ state.lifecycleSnapshotPhase = "Current"
  /\ state.requesterEpochPhase # "Persisted"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.serverStreamState = "Terminal",
          !.durableLifecycleSnapshotV2 =
            LifecycleSnapshotV2(
              state.serviceGeneration,
              state.nextStreamEpoch,
              "Terminal",
              state.requestGateState),
          !.capacityRejected = FALSE]

TerminalizeRequestGate ==
  /\ ExactRetainedMergeSidecars
  /\ CompactionNeeded
  /\ state.requestGateState = "Active"
  /\ state.lifecycleSnapshotPhase = "Current"
  /\ state.requesterEpochPhase # "Persisted"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.requestGateState = "Terminal",
          !.durableLifecycleSnapshotV2 =
            LifecycleSnapshotV2(
              state.serviceGeneration,
              state.nextStreamEpoch,
              state.serverStreamState,
              "Terminal"),
          !.capacityRejected = FALSE]

TerminalizeTransfer ==
  /\ ExactRetainedMergeSidecars
  /\ CompactionNeeded
  /\ state.transferState = "Active"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.transferState = "Terminal",
          !.capacityRejected = FALSE]

TerminalizeFlush ==
  /\ ExactRetainedMergeSidecars
  /\ CompactionNeeded
  /\ state.flushState = "Active"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.flushState = "Terminal",
          !.capacityRejected = FALSE]

PersistSuccessorLifecycleSnapshotV2 ==
  /\ ExactRetainedMergeSidecars
  /\ CompactionNeeded
  /\ AllOldLifecycleTerminal
  /\ state.serviceGeneration < GenerationLimit
  /\ state.lifecycleSnapshotPhase = "Current"
  /\ state.requesterEpochPhase # "Persisted"
  /\ LifecycleMemoryMatchesDurableSnapshotV2
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.durableLifecycleSnapshotV2 =
            LifecycleSnapshotV2(
              state.serviceGeneration + 1,
              state.nextStreamEpoch,
              "Empty",
              "Empty"),
          !.lifecycleSnapshotPhase = "Persisted",
          !.capacityRejected = FALSE]

FailSuccessorLifecycleSnapshotV2Persistence ==
  /\ ExactRetainedMergeSidecars
  /\ CompactionNeeded
  /\ AllOldLifecycleTerminal
  /\ state.serviceGeneration < GenerationLimit
  /\ state.lifecycleSnapshotPhase = "Current"
  /\ state.requesterEpochPhase # "Persisted"
  /\ LifecycleMemoryMatchesDurableSnapshotV2
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.restartRequired = TRUE,
          !.failureReason = "LifecycleSnapshotV2PersistenceFailure"]

CrashBeforeLifecycleSnapshotV2Persistence ==
  /\ ExactRetainedMergeSidecars
  /\ CompactionNeeded
  /\ AllOldLifecycleTerminal
  /\ state.serviceGeneration < GenerationLimit
  /\ state.lifecycleSnapshotPhase = "Current"
  /\ LifecycleMemoryMatchesDurableSnapshotV2
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.restartRequired = TRUE,
          !.failureReason =
            "CrashBeforeLifecycleSnapshotV2Persistence"]

RecoverPredecessorLifecycleSnapshotV2 ==
  /\ state.restartRequired
  /\ state.failureReason \in
       {"LifecycleSnapshotV2PersistenceFailure",
        "CrashBeforeLifecycleSnapshotV2Persistence"}
  /\ state.lifecycleSnapshotPhase = "Current"
  /\ LifecycleMemoryMatchesDurableSnapshotV2
  /\ state' =
       [state EXCEPT
          !.restartRequired = FALSE,
          !.failureReason = "None"]

CrashAfterLifecycleSnapshotV2Persistence ==
  /\ LifecycleSnapshotV2AheadOfMemory
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.restartRequired = TRUE,
          !.failureReason =
            "CrashAfterLifecycleSnapshotV2Persistence",
          !.snapshotCrashObserved = TRUE]

RestoreSuccessorLifecycleSnapshotV2AfterCrash ==
  /\ LifecycleSnapshotV2AheadOfMemory
  /\ state.restartRequired
  /\ state.failureReason =
       "CrashAfterLifecycleSnapshotV2Persistence"
  /\ state.snapshotCrashObserved
  /\ state' =
       [state EXCEPT
          !.serviceGeneration =
            state.durableLifecycleSnapshotV2.generation,
          !.nextStreamEpoch =
            state.durableLifecycleSnapshotV2.nextStreamEpoch,
          !.serverStreamState =
            state.durableLifecycleSnapshotV2.serverStreams,
          !.requestGateState =
            state.durableLifecycleSnapshotV2.requestGates,
          !.transferState = "Empty",
          !.flushState = "Empty",
          !.retryableChunk = 0,
          !.lifecycleSnapshotPhase = "Restored",
          !.restartRequired = FALSE,
          !.failureReason = "None"]

PublishPersistedLifecycleSnapshotV2 ==
  /\ ExactRetainedMergeSidecars
  /\ CompactionNeeded
  /\ ~state.restartRequired
  /\ state.lifecycleSnapshotPhase \in {"Persisted", "Restored"}
  /\ IF state.lifecycleSnapshotPhase = "Persisted"
        THEN LifecycleSnapshotV2AheadOfMemory
        ELSE LifecycleMemoryMatchesDurableSnapshotV2
  /\ state.durableLifecycleSnapshotV2.serverStreams = "Empty"
  /\ state.durableLifecycleSnapshotV2.requestGates = "Empty"
  /\ state' =
       [state EXCEPT
          !.serviceGeneration =
            state.durableLifecycleSnapshotV2.generation,
          !.nextStreamEpoch =
            state.durableLifecycleSnapshotV2.nextStreamEpoch,
          !.serverStreamState = "Empty",
          !.requestGateState = "Empty",
          !.transferState = "Empty",
          !.flushState = "Empty",
          !.retryableChunk = 0,
          !.lifecycleSnapshotPhase = "Published",
          !.receiptStage = "Consumed",
          !.receiptConsumeCount = 1,
          !.successorActive = TRUE,
          !.transitionAuthority = "LifecycleV2"]

ActivateSameRosterSuccessor ==
  /\ state.targetRoster = "SameRoster"
  /\ state.compactionCause = "NoCompaction"
  /\ ExactRetainedMergeSidecars
  /\ state.lifecycleSnapshotPhase = "Current"
  /\ LifecycleMemoryMatchesDurableSnapshotV2
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.receiptStage = "Consumed",
          !.receiptConsumeCount = 1,
          !.successorActive = TRUE,
          !.transitionAuthority = "SameRoster"]

RejectGenerationOverflow ==
  /\ state.successorActive
  /\ state.serviceGeneration = GenerationLimit
  /\ ~state.capacityRejected
  /\ state' =
       [state EXCEPT !.capacityRejected = TRUE]

ObserveLateOldWriterCallback ==
  /\ state.successorActive
  /\ state.transitionAuthority = "LifecycleV2"
  /\ ~state.lateOldCallbackObserved
  /\ state' =
       [state EXCEPT !.lateOldCallbackObserved = TRUE]

Next ==
  \/ ValidateFinality
  \/ CloseWorkerIngress
  \/ ClearOneWorkerExactOutput
  \/ BuildImmediateSuccessor
  \/ SealAppliedHeightOutputHandoff
  \/ RejectLateExactOutputEnqueue
  \/ RejectForeignOwnerReceipt
  \/ RejectPredecessorContextMismatch
  \/ RejectPredecessorArtifactMismatch
  \/ RejectWrongImmediateSuccessor
  \/ RetainExactHandoffReceipt
  \/ PersistFreshRequesterEpoch
  \/ PublishFreshRequesterEpoch
  \/ CompleteRequesterEpoch
  \/ ReopenRequesterEpochAllocator
  \/ CrashAfterRequesterEpochPersistence
  \/ RestoreRequesterEpochCounterAfterCrash
  \/ RejectRequesterEpochOverflow
  \/ RejectActiveLifecycleCompaction
  \/ TerminalizeServerStream
  \/ TerminalizeRequestGate
  \/ TerminalizeTransfer
  \/ TerminalizeFlush
  \/ PersistSuccessorLifecycleSnapshotV2
  \/ FailSuccessorLifecycleSnapshotV2Persistence
  \/ CrashBeforeLifecycleSnapshotV2Persistence
  \/ RecoverPredecessorLifecycleSnapshotV2
  \/ CrashAfterLifecycleSnapshotV2Persistence
  \/ RestoreSuccessorLifecycleSnapshotV2AfterCrash
  \/ PublishPersistedLifecycleSnapshotV2
  \/ ActivateSameRosterSuccessor
  \/ RejectGenerationOverflow
  \/ ObserveLateOldWriterCallback

TypedRolloverSpec ==
  /\ Init
  /\ [][Next]_typedRolloverVars

ResponsiveTypedRolloverSpec ==
  /\ TypedRolloverSpec
  /\ WF_typedRolloverVars(CloseWorkerIngress)
  /\ WF_typedRolloverVars(ClearOneWorkerExactOutput)
  /\ WF_typedRolloverVars(BuildImmediateSuccessor)
  /\ WF_typedRolloverVars(SealAppliedHeightOutputHandoff)
  /\ WF_typedRolloverVars(RetainExactHandoffReceipt)
  /\ WF_typedRolloverVars(TerminalizeServerStream)
  /\ WF_typedRolloverVars(TerminalizeRequestGate)
  /\ WF_typedRolloverVars(TerminalizeTransfer)
  /\ WF_typedRolloverVars(TerminalizeFlush)
  /\ WF_typedRolloverVars(PersistSuccessorLifecycleSnapshotV2)
  /\ WF_typedRolloverVars(PublishPersistedLifecycleSnapshotV2)
  /\ WF_typedRolloverVars(ActivateSameRosterSuccessor)

TypedRolloverTypeInvariant ==
  state \in
    [targetRoster: RosterRelations,
     compactionCause: CompactionCauses,
     finalityValidated: BOOLEAN,
     workerIngressClosed: BOOLEAN,
     workerOutstanding: 0..InitialWorkerOutstanding,
     ownerSealed: BOOLEAN,
     constructionParent: Parents \cup {NoIdentity},
     constructionSuccessor: Successors \cup {NoIdentity},
     receiptStage: ReceiptStages,
     receiptOwnerNonce: OwnerNonces \cup {NoIdentity},
     receiptContext: Contexts \cup {NoIdentity},
     receiptArtifact: Artifacts \cup {NoIdentity},
     retainedSuccessor: Successors \cup {NoIdentity},
     receiptConsumeCount: 0..1,
     serviceGeneration: InitialGeneration..GenerationLimit,
     nextStreamEpoch: InitialNextStreamEpoch..StreamEpochLimit,
     requesterEpochPhase: RequesterEpochPhases,
     pendingStreamEpoch: 0..StreamEpochLimit,
     activeStreamEpoch: 0..StreamEpochLimit,
     lastCompletedStreamEpoch: 0..StreamEpochLimit,
     skippedStreamEpoch: 0..StreamEpochLimit,
     serverStreamState: LifecycleEntryStates,
     requestGateState: LifecycleEntryStates,
     transferState: LifecycleEntryStates,
     flushState: LifecycleEntryStates,
     durableLifecycleSnapshotV2: LifecycleSnapshotV2Set,
     lifecycleSnapshotPhase: LifecycleSnapshotPhases,
     retryableChunk: 0..1,
     successorActive: BOOLEAN,
     transitionAuthority: TransitionAuthorities,
     capacityRejected: BOOLEAN,
     restartRequired: BOOLEAN,
     failureReason: FailureReasons,
     lateEnqueueRejected: BOOLEAN,
     foreignReceiptRejected: BOOLEAN,
     predecessorMismatchRejected: BOOLEAN,
     wrongSuccessorRejected: BOOLEAN,
     lateOldCallbackObserved: BOOLEAN,
     snapshotCrashObserved: BOOLEAN,
     epochCrashObserved: BOOLEAN]

CompactionGeometryInvariant ==
  CompactionCauseMatchesGeometry

ReceiptLifecycleInvariant ==
  /\ (state.receiptStage = "Absent" =>
        /\ state.receiptOwnerNonce = NoIdentity
        /\ state.receiptContext = NoIdentity
        /\ state.receiptArtifact = NoIdentity)
  /\ (state.receiptStage # "Absent" =>
        /\ state.ownerSealed
        /\ ExactPredecessorReceipt)
  /\ (state.receiptStage \in {"Retained", "Consumed"} =>
        /\ ExactSuccessorConstruction
        /\ state.retainedSuccessor = ExpectedSuccessor)
  /\ (state.receiptStage = "Consumed" =>
        state.receiptConsumeCount = 1)

FinalSealRejectsLateEnqueueInvariant ==
  /\ (state.ownerSealed => FinalExactOutputSeal)
  /\ (state.lateEnqueueRejected =>
        /\ state.restartRequired
        /\ ~state.successorActive
        /\ state.failureReason = "LateExactOutputEnqueue")

MismatchRejectionInvariant ==
  /\ (state.foreignReceiptRejected =>
        /\ state.restartRequired
        /\ state.failureReason = "ForeignOwnerMismatch")
  /\ (state.predecessorMismatchRejected =>
        /\ state.restartRequired
        /\ state.failureReason \in
             {"PredecessorContextMismatch",
              "PredecessorArtifactMismatch"})
  /\ (state.wrongSuccessorRejected =>
        /\ state.restartRequired
        /\ state.failureReason = "ImmediateSuccessorMismatch")

FailureLatchInvariant ==
  /\ (state.restartRequired => ~state.successorActive)
  /\ (~state.restartRequired => state.failureReason = "None")

SoleLifecycleSnapshotV2Invariant ==
  /\ state.durableLifecycleSnapshotV2 \in LifecycleSnapshotV2Set
  /\ state.durableLifecycleSnapshotV2.version = 2
  /\ state.serviceGeneration <=
       state.durableLifecycleSnapshotV2.generation
  /\ state.nextStreamEpoch <=
       state.durableLifecycleSnapshotV2.nextStreamEpoch

LifecycleSnapshotPhaseInvariant ==
  CASE state.lifecycleSnapshotPhase = "Current" ->
         /\ state.serviceGeneration =
              state.durableLifecycleSnapshotV2.generation
         /\ state.serverStreamState =
              state.durableLifecycleSnapshotV2.serverStreams
         /\ state.requestGateState =
              state.durableLifecycleSnapshotV2.requestGates
         /\ (state.nextStreamEpoch =
               state.durableLifecycleSnapshotV2.nextStreamEpoch
             \/ RequesterEpochPersistenceAheadOfMemory)
    [] state.lifecycleSnapshotPhase = "Persisted" ->
         /\ LifecycleSnapshotV2AheadOfMemory
         /\ AllOldLifecycleTerminal
    [] state.lifecycleSnapshotPhase = "Restored" ->
         /\ LifecycleMemoryMatchesDurableSnapshotV2
         /\ state.serverStreamState = "Empty"
         /\ state.requestGateState = "Empty"
         /\ state.transferState = "Empty"
         /\ state.flushState = "Empty"
    [] state.lifecycleSnapshotPhase = "Published" ->
         /\ LifecycleMemoryMatchesDurableSnapshotV2
         /\ state.serverStreamState = "Empty"
         /\ state.requestGateState = "Empty"
         /\ state.transferState = "Empty"
         /\ state.flushState = "Empty"

TerminalOnlyGenerationAdvanceInvariant ==
  /\ (state.durableLifecycleSnapshotV2.generation >
        state.serviceGeneration =>
        /\ AllOldLifecycleTerminal
        /\ state.durableLifecycleSnapshotV2.serverStreams = "Empty"
        /\ state.durableLifecycleSnapshotV2.requestGates = "Empty")
  /\ (state.serviceGeneration > InitialGeneration =>
        /\ state.serverStreamState = "Empty"
        /\ state.requestGateState = "Empty"
        /\ state.transferState = "Empty"
        /\ state.flushState = "Empty")
  /\ (state.transitionAuthority = "LifecycleV2" =>
        /\ state.lifecycleSnapshotPhase = "Published"
        /\ state.serviceGeneration =
             state.durableLifecycleSnapshotV2.generation
        /\ state.receiptStage = "Consumed")

SameRosterTransportPreservationInvariant ==
  state.transitionAuthority = "SameRoster" =>
    /\ state.targetRoster = "SameRoster"
    /\ state.compactionCause = "NoCompaction"
    /\ state.serviceGeneration = InitialGeneration
    /\ state.serverStreamState = "Active"
    /\ state.requestGateState = "Active"
    /\ state.transferState = "Active"
    /\ state.flushState = "Active"
    /\ state.retryableChunk = 1

RequesterEpochInvariant ==
  /\ state.lastCompletedStreamEpoch < state.nextStreamEpoch
  /\ (state.pendingStreamEpoch # 0 =>
        /\ state.requesterEpochPhase = "Persisted"
        /\ state.pendingStreamEpoch > state.lastCompletedStreamEpoch)
  /\ (state.activeStreamEpoch # 0 =>
        /\ state.requesterEpochPhase = "InUse"
        /\ state.activeStreamEpoch > state.lastCompletedStreamEpoch
        /\ state.activeStreamEpoch < state.nextStreamEpoch
        /\ state.activeStreamEpoch # state.skippedStreamEpoch)
  /\ (state.skippedStreamEpoch # 0 =>
        state.skippedStreamEpoch <
          state.durableLifecycleSnapshotV2.nextStreamEpoch)

CrashRecoveryInvariant ==
  /\ (state.snapshotCrashObserved =>
        state.durableLifecycleSnapshotV2.generation =
          GenerationLimit)
  /\ (state.epochCrashObserved =>
        state.skippedStreamEpoch # 0)
  /\ (state.failureReason =
        "CrashAfterLifecycleSnapshotV2Persistence" =>
        /\ state.snapshotCrashObserved
        /\ LifecycleSnapshotV2AheadOfMemory)
  /\ (state.failureReason =
        "CrashAfterRequesterEpochPersistence" =>
        /\ state.epochCrashObserved
        /\ RequesterEpochPersistenceAheadOfMemory)

LateOldCallbackIsolationInvariant ==
  state.lateOldCallbackObserved =>
    /\ state.successorActive
    /\ state.transitionAuthority = "LifecycleV2"
    /\ state.lifecycleSnapshotPhase = "Published"
    /\ state.serverStreamState = "Empty"
    /\ state.requestGateState = "Empty"

TypedRolloverSafetyInvariant ==
  /\ TypedRolloverTypeInvariant
  /\ CompactionGeometryInvariant
  /\ ReceiptLifecycleInvariant
  /\ FinalSealRejectsLateEnqueueInvariant
  /\ MismatchRejectionInvariant
  /\ FailureLatchInvariant
  /\ SoleLifecycleSnapshotV2Invariant
  /\ LifecycleSnapshotPhaseInvariant
  /\ TerminalOnlyGenerationAdvanceInvariant
  /\ SameRosterTransportPreservationInvariant
  /\ RequesterEpochInvariant
  /\ CrashRecoveryInvariant
  /\ LateOldCallbackIsolationInvariant

CapacityRejectionStepSafety ==
  (/\ ~state.capacityRejected
   /\ state'.capacityRejected)
    =>
      /\ LifecycleMemory(state') = LifecycleMemory(state)
      /\ DurableLifecycle(state') = DurableLifecycle(state)

AtomicLifecycleSnapshotV2PersistenceStepSafety ==
  state'.durableLifecycleSnapshotV2.generation >
    state.durableLifecycleSnapshotV2.generation
    =>
      /\ AllOldLifecycleTerminal
      /\ state'.durableLifecycleSnapshotV2 =
           LifecycleSnapshotV2(
             state.serviceGeneration + 1,
             state.nextStreamEpoch,
             "Empty",
             "Empty")
      /\ LifecycleMemory(state') = LifecycleMemory(state)

LifecycleSnapshotV2PublicationStepSafety ==
  state'.serviceGeneration > state.serviceGeneration
    =>
      /\ state.durableLifecycleSnapshotV2.generation =
           state'.serviceGeneration
      /\ state.durableLifecycleSnapshotV2.serverStreams = "Empty"
      /\ state.durableLifecycleSnapshotV2.requestGates = "Empty"
      /\ state'.serverStreamState = "Empty"
      /\ state'.requestGateState = "Empty"
      /\ state'.transferState = "Empty"
      /\ state'.flushState = "Empty"

RequesterEpochUseStepSafety ==
  (/\ state.activeStreamEpoch = 0
   /\ state'.activeStreamEpoch # 0)
    =>
      /\ state.requesterEpochPhase = "Persisted"
      /\ state.durableLifecycleSnapshotV2.nextStreamEpoch >
           state'.activeStreamEpoch
      /\ state'.nextStreamEpoch =
           state.durableLifecycleSnapshotV2.nextStreamEpoch
      /\ state'.activeStreamEpoch # state.skippedStreamEpoch

CapacityRejectionActionProperty ==
  [][CapacityRejectionStepSafety]_typedRolloverVars

AtomicLifecycleSnapshotV2PersistenceActionProperty ==
  [][AtomicLifecycleSnapshotV2PersistenceStepSafety]_typedRolloverVars

LifecycleSnapshotV2PublicationActionProperty ==
  [][LifecycleSnapshotV2PublicationStepSafety]_typedRolloverVars

RequesterEpochUseActionProperty ==
  [][RequesterEpochUseStepSafety]_typedRolloverVars

NoRolloverFailure ==
  /\ ~state.restartRequired
  /\ state.failureReason = "None"

ChangedRosterSuccessorActiveWithoutRestart ==
  /\ state.targetRoster = "ChangedRoster"
  /\ state.successorActive
  /\ ~state.restartRequired
  /\ state.transitionAuthority = "LifecycleV2"

ResponsiveChangedRosterRolloverLiveness ==
  ResponsiveTypedRolloverSpec =>
    ((/\ state.targetRoster = "ChangedRoster"
      /\ state.finalityValidated)
      ~> ChangedRosterSuccessorActiveWithoutRestart)

=============================================================================
