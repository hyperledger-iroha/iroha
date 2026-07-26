---- MODULE SumeragiV2TypedRolloverHandoffMutation ----
EXTENDS SumeragiV2TypedRolloverHandoff

(***************************************************************************
Negative controls for the sole-V2 lifecycle and typed handoff boundary.

Each mode introduces one forbidden transition.  The lifecycle controls cover
active-state compaction, publication before the one atomic V2 snapshot,
snapshot-crash recovery without restoration, generation overflow, epoch use
before persistence, post-crash epoch reuse, and epoch overflow.  No mutation
reintroduces a separate generation marker or migration schema.
***************************************************************************)

CONSTANT MutationMode

MutationModes ==
  {"PrematureSeal",
   "ForeignOwnerNonce",
   "IgnoreForeignCandidate",
   "CleanForeignOwnerReject",
   "AcceptPredecessorContextMismatch",
   "AcceptPredecessorArtifactMismatch",
   "CleanPredecessorContextReject",
   "CleanPredecessorArtifactReject",
   "ForeignSuccessor",
   "CleanWrongSuccessorReject",
   "LateEnqueue",
   "CleanLateEnqueueReject",
   "LoseSameRosterRetry",
   "LateOldCallback",
   "UntypedForce",
   "CleanAtomicV2PersistenceFailure",
   "CleanCrashAfterAtomicV2Persist",
   "SkipSnapshotCrashHistory",
   "OpenSnapshotAheadWithoutRestore",
   "PublishBeforeAtomicV2Persist",
   "ActiveStateRoll",
   "GenerationOverflowWrap",
   "EpochUseBeforePersist",
   "EpochReuseAfterCrash",
   "EpochOverflowWrap"}

MutationConfiguration ==
  MutationMode \in MutationModes

PrematureSealAndMint ==
  /\ state.finalityValidated
  /\ state.workerOutstanding > 0
  /\ state.receiptStage = "Absent"
  /\ state' =
       [state EXCEPT
          !.ownerSealed = TRUE,
          !.receiptStage = "Minted",
          !.receiptOwnerNonce = ServiceOwnerNonce,
          !.receiptContext = ExpectedContext,
          !.receiptArtifact = ExpectedArtifact]

RetainForeignOwnerReceipt ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ state' =
       [state EXCEPT !.receiptOwnerNonce = ForeignOwnerNonce]

IgnoreForeignOwnerCandidate ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ ~state.foreignReceiptRejected
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT !.foreignReceiptRejected = TRUE]

CleanRejectForeignOwnerReceipt ==
  IgnoreForeignOwnerCandidate

AcceptPredecessorContextMismatch ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ state' =
       [state EXCEPT !.receiptContext = "ContextB"]

AcceptPredecessorArtifactMismatch ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ state' =
       [state EXCEPT !.receiptArtifact = "ArtifactB"]

CleanRejectPredecessorContextMismatch ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ ~state.predecessorMismatchRejected
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT !.predecessorMismatchRejected = TRUE]

CleanRejectPredecessorArtifactMismatch ==
  CleanRejectPredecessorContextMismatch

RetainForeignSuccessor ==
  /\ state.receiptStage = "Minted"
  /\ ExactPredecessorReceipt
  /\ ExactSuccessorConstruction
  /\ state' =
       [state EXCEPT
          !.receiptStage = "Retained",
          !.retainedSuccessor = ForeignSuccessor]

CleanRejectWrongImmediateSuccessor ==
  /\ state.receiptStage = "Minted"
  /\ ~state.wrongSuccessorRejected
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT !.wrongSuccessorRejected = TRUE]

EnqueueAfterOwnerSeal ==
  /\ state.ownerSealed
  /\ state.workerOutstanding = 0
  /\ state' =
       [state EXCEPT !.workerOutstanding = 1]

CleanRejectLateExactOutputEnqueue ==
  /\ state.ownerSealed
  /\ state.receiptStage = "Minted"
  /\ ~state.lateEnqueueRejected
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT !.lateEnqueueRejected = TRUE]

SameRosterDropsRetryableChunk ==
  /\ state.targetRoster = "SameRoster"
  /\ state.compactionCause = "NoCompaction"
  /\ ExactRetainedMergeSidecars
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.receiptStage = "Consumed",
          !.receiptConsumeCount = 1,
          !.successorActive = TRUE,
          !.transitionAuthority = "SameRoster",
          !.retryableChunk = 0]

LateOldWriterMutatesSuccessor ==
  /\ state.successorActive
  /\ state.transitionAuthority = "LifecycleV2"
  /\ state.serverStreamState = "Empty"
  /\ state' =
       [state EXCEPT
          !.lateOldCallbackObserved = TRUE,
          !.serverStreamState = "Active"]

UntypedActiveLifecycleForce ==
  /\ ExactRetainedMergeSidecars
  /\ CompactionNeeded
  /\ ~AllOldLifecycleTerminal
  /\ state.serviceGeneration < GenerationLimit
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.durableLifecycleSnapshotV2 =
            LifecycleSnapshotV2(
              state.serviceGeneration + 1,
              state.nextStreamEpoch,
              "Empty",
              "Empty"),
          !.serviceGeneration = state.serviceGeneration + 1,
          !.serverStreamState = "Empty",
          !.requestGateState = "Empty",
          !.transferState = "Empty",
          !.flushState = "Empty",
          !.retryableChunk = 0,
          !.lifecycleSnapshotPhase = "Published",
          !.receiptStage = "Consumed",
          !.receiptConsumeCount = 1,
          !.successorActive = TRUE,
          !.transitionAuthority = "SameRoster"]

CleanFailAtomicV2Persistence ==
  /\ ExactRetainedMergeSidecars
  /\ CompactionNeeded
  /\ AllOldLifecycleTerminal
  /\ state.serviceGeneration < GenerationLimit
  /\ state.lifecycleSnapshotPhase = "Current"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.failureReason =
            "LifecycleSnapshotV2PersistenceFailure"]

CleanCrashAfterAtomicV2Persist ==
  /\ LifecycleSnapshotV2AheadOfMemory
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.lifecycleSnapshotPhase = "Restored",
          !.snapshotCrashObserved = TRUE]

CrashWithoutSnapshotHistory ==
  /\ LifecycleSnapshotV2AheadOfMemory
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.restartRequired = TRUE,
          !.failureReason =
            "CrashAfterLifecycleSnapshotV2Persistence"]

OpenSnapshotAheadWithoutRestore ==
  /\ LifecycleSnapshotV2AheadOfMemory
  /\ state.restartRequired
  /\ state.failureReason =
       "CrashAfterLifecycleSnapshotV2Persistence"
  /\ state' =
       [state EXCEPT
          !.restartRequired = FALSE,
          !.failureReason = "None",
          !.receiptStage = "Consumed",
          !.receiptConsumeCount = 1,
          !.successorActive = TRUE,
          !.transitionAuthority = "LifecycleV2"]

PublishBeforeAtomicV2Persist ==
  /\ ExactRetainedMergeSidecars
  /\ CompactionNeeded
  /\ AllOldLifecycleTerminal
  /\ state.serviceGeneration < GenerationLimit
  /\ state.lifecycleSnapshotPhase = "Current"
  /\ LifecycleMemoryMatchesDurableSnapshotV2
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.serviceGeneration = state.serviceGeneration + 1,
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

RollActiveLifecycleState ==
  /\ ExactRetainedMergeSidecars
  /\ CompactionNeeded
  /\ ~AllOldLifecycleTerminal
  /\ state.serviceGeneration < GenerationLimit
  /\ state.lifecycleSnapshotPhase = "Current"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.durableLifecycleSnapshotV2 =
            LifecycleSnapshotV2(
              state.serviceGeneration + 1,
              state.nextStreamEpoch,
              "Empty",
              "Empty"),
          !.lifecycleSnapshotPhase = "Persisted"]

WrapGenerationCounter ==
  /\ state.successorActive
  /\ state.serviceGeneration = GenerationLimit
  /\ state' =
       [state EXCEPT
          !.serviceGeneration = GenerationLimit + 1,
          !.durableLifecycleSnapshotV2 =
            LifecycleSnapshotV2(
              GenerationLimit + 1,
              state.nextStreamEpoch,
              "Empty",
              "Empty")]

UseRequesterEpochBeforePersistence ==
  /\ state.requesterEpochPhase = "Idle"
  /\ state.nextStreamEpoch < StreamEpochLimit
  /\ state.activeStreamEpoch = 0
  /\ state' =
       [state EXCEPT
          !.requesterEpochPhase = "InUse",
          !.activeStreamEpoch = state.nextStreamEpoch]

ReuseRequesterEpochAfterCrash ==
  /\ state.restartRequired
  /\ state.failureReason = "CrashAfterRequesterEpochPersistence"
  /\ state.epochCrashObserved
  /\ state.skippedStreamEpoch # 0
  /\ state' =
       [state EXCEPT
          !.nextStreamEpoch =
            state.durableLifecycleSnapshotV2.nextStreamEpoch,
          !.requesterEpochPhase = "InUse",
          !.pendingStreamEpoch = 0,
          !.activeStreamEpoch = state.skippedStreamEpoch,
          !.restartRequired = FALSE,
          !.failureReason = "None"]

WrapRequesterEpochCounter ==
  /\ state.requesterEpochPhase = "Idle"
  /\ state.nextStreamEpoch = StreamEpochLimit
  /\ state' =
       [state EXCEPT
          !.nextStreamEpoch = InitialNextStreamEpoch,
          !.durableLifecycleSnapshotV2 =
            LifecycleSnapshotV2(
              state.serviceGeneration,
              InitialNextStreamEpoch,
              state.serverStreamState,
              state.requestGateState),
          !.requesterEpochPhase = "InUse",
          !.activeStreamEpoch = StreamEpochLimit]

SelectedMutationAction ==
  CASE MutationMode = "PrematureSeal" ->
         PrematureSealAndMint
    [] MutationMode = "ForeignOwnerNonce" ->
         RetainForeignOwnerReceipt
    [] MutationMode = "IgnoreForeignCandidate" ->
         IgnoreForeignOwnerCandidate
    [] MutationMode = "CleanForeignOwnerReject" ->
         CleanRejectForeignOwnerReceipt
    [] MutationMode = "AcceptPredecessorContextMismatch" ->
         AcceptPredecessorContextMismatch
    [] MutationMode = "AcceptPredecessorArtifactMismatch" ->
         AcceptPredecessorArtifactMismatch
    [] MutationMode = "CleanPredecessorContextReject" ->
         CleanRejectPredecessorContextMismatch
    [] MutationMode = "CleanPredecessorArtifactReject" ->
         CleanRejectPredecessorArtifactMismatch
    [] MutationMode = "ForeignSuccessor" ->
         RetainForeignSuccessor
    [] MutationMode = "CleanWrongSuccessorReject" ->
         CleanRejectWrongImmediateSuccessor
    [] MutationMode = "LateEnqueue" ->
         EnqueueAfterOwnerSeal
    [] MutationMode = "CleanLateEnqueueReject" ->
         CleanRejectLateExactOutputEnqueue
    [] MutationMode = "LoseSameRosterRetry" ->
         SameRosterDropsRetryableChunk
    [] MutationMode = "LateOldCallback" ->
         LateOldWriterMutatesSuccessor
    [] MutationMode = "UntypedForce" ->
         UntypedActiveLifecycleForce
    [] MutationMode = "CleanAtomicV2PersistenceFailure" ->
         CleanFailAtomicV2Persistence
    [] MutationMode = "CleanCrashAfterAtomicV2Persist" ->
         CleanCrashAfterAtomicV2Persist
    [] MutationMode = "SkipSnapshotCrashHistory" ->
         CrashWithoutSnapshotHistory
    [] MutationMode = "OpenSnapshotAheadWithoutRestore" ->
         OpenSnapshotAheadWithoutRestore
    [] MutationMode = "PublishBeforeAtomicV2Persist" ->
         PublishBeforeAtomicV2Persist
    [] MutationMode = "ActiveStateRoll" ->
         RollActiveLifecycleState
    [] MutationMode = "GenerationOverflowWrap" ->
         WrapGenerationCounter
    [] MutationMode = "EpochUseBeforePersist" ->
         UseRequesterEpochBeforePersistence
    [] MutationMode = "EpochReuseAfterCrash" ->
         ReuseRequesterEpochAfterCrash
    [] MutationMode = "EpochOverflowWrap" ->
         WrapRequesterEpochCounter

MutationNext ==
  \/ Next
  \/ SelectedMutationAction

MutationSpec ==
  /\ MutationConfiguration
  /\ Init
  /\ [][MutationNext]_typedRolloverVars

=============================================================================
