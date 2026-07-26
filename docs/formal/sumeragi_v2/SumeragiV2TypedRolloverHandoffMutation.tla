---- MODULE SumeragiV2TypedRolloverHandoffMutation ----
EXTENDS SumeragiV2TypedRolloverHandoff

(***************************************************************************
Negative controls for the authority-gated V3 lifecycle and typed handoff.

The controls cover exact handoff identity, same-roster preservation, ordinary
terminality, durable active-fence authority, the state-slot/root/memory commit
order, crash selection of the root-bound snapshot, both checked generations,
bootstrap/committed root-shape separation, exact root generation/digest
pairing, selected-state presence, validation-before-cleanup,
durable-before-use requester epochs, and the prohibition on forged
requester-authenticated Close prefixes.
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
   "SameRosterGenerationRoll",
   "LateOldCallback",
   "UntypedForce",
   "CleanStateSlotV3PersistenceFailure",
   "CleanCrashAfterLifecycleRootV3Commit",
   "SkipLifecycleRootV3CrashHistory",
   "RecoverUncommittedStateSlot",
   "PublishMemoryBeforeLifecycleRootV3Commit",
   "CommitLifecycleRootV3BeforeStateSlot",
   "ReuseRootSelectedStateSlot",
   "ActiveOrdinaryRoll",
   "ServiceGenerationOverflowWrap",
   "RootGenerationOverflowWrap",
   "ForgeAuthenticatedClosePrefix",
   "EpochUseBeforePersist",
   "EpochReuseAfterCrash",
   "EpochOverflowWrap",
   "CrossedLifecycleRootShape",
   "SplitLifecycleGenerationHash",
   "MissingRootSelectedState",
   "CleanupBeforeSemanticValidation",
   "ChangedRosterWithoutGenerationAdvance"}

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
          !.transitionAuthority = "SameRosterPreserved",
          !.retryableChunk = 0]

SameRosterAdvancesGeneration ==
  /\ state.targetRoster = "SameRoster"
  /\ ExactRetainedMergeSidecars
  /\ state.serviceGeneration < ServiceGenerationLimit
  /\ state.durableLifecycleRootV3.rootGeneration < RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             "SameRoster",
             state.serviceGeneration + 1,
             state.nextStreamEpoch,
             "Empty",
             "Empty",
             0)
     IN state' =
          [state EXCEPT
             !.durableLifecycleStateSlotsV3[
               LifecycleStateSlot(snapshot.rootGeneration)] =
                 snapshot,
             !.durableLifecycleRootV3 =
               LifecycleRootV3(snapshot),
             !.serviceGeneration = state.serviceGeneration + 1,
             !.serverStreamState = "Empty",
             !.requestGateState = "Empty",
             !.serverClosePrefix = 0,
             !.transferState = "Empty",
             !.flushState = "Empty",
             !.retryableChunk = 0,
             !.lifecycleCommitPhase = "Published",
             !.receiptStage = "Consumed",
             !.receiptConsumeCount = 1,
             !.successorActive = TRUE,
             !.transitionAuthority = "SameRosterPreserved"]

LateOldWriterMutatesSuccessor ==
  /\ state.successorActive
  /\ state.transitionAuthority \in ChangedRosterAuthorities
  /\ state.serverStreamState = "Empty"
  /\ state' =
       [state EXCEPT
          !.lateOldCallbackObserved = TRUE,
          !.serverStreamState = "Active"]

UntypedActiveLifecycleForce ==
  /\ ExactRetainedMergeSidecars
  /\ ChangedRosterReplacementNeeded
  /\ ~AllOldLifecycleTerminal
  /\ state.serviceGeneration < ServiceGenerationLimit
  /\ state.durableLifecycleRootV3.rootGeneration < RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.restartRequired
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             "ChangedRoster",
             state.serviceGeneration + 1,
             state.nextStreamEpoch,
             "Empty",
             "Empty",
             0)
     IN state' =
          [state EXCEPT
             !.durableLifecycleStateSlotsV3[
               LifecycleStateSlot(snapshot.rootGeneration)] =
                 snapshot,
             !.durableLifecycleRootV3 =
               LifecycleRootV3(snapshot),
             !.currentRoster = "ChangedRoster",
             !.serviceGeneration = state.serviceGeneration + 1,
             !.serverStreamState = "Empty",
             !.requestGateState = "Empty",
             !.serverClosePrefix = 0,
             !.transferState = "Empty",
             !.flushState = "Empty",
             !.retryableChunk = 0,
             !.lifecycleCommitPhase = "Published",
             !.receiptStage = "Consumed",
             !.receiptConsumeCount = 1,
             !.successorActive = TRUE,
             !.transitionAuthority = "AuthenticatedTerminal"]

CleanFailStateSlotV3Persistence ==
  /\ ChangedRosterReplacementNeeded
  /\ ExactRetainedMergeSidecars
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.failureReason =
            "LifecycleStateSlotV3PersistenceFailure"]

CleanCrashAfterLifecycleRootV3Commit ==
  /\ RootCommittedSuccessorAheadOfMemory
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.lifecycleCommitPhase = "Restored",
          !.rootCrashObserved = TRUE]

CrashWithoutLifecycleRootV3History ==
  /\ RootCommittedSuccessorAheadOfMemory
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.restartRequired = TRUE,
          !.failureReason = "CrashAfterLifecycleRootV3Commit"]

RecoverUncommittedStateSlotAsSuccessor ==
  /\ DurableCandidateStateSlotAheadOfRoot
  /\ state.restartRequired
  /\ state.failureReason =
       "CrashAfterLifecycleStateSlotV3Publication"
  /\ state' =
       [state EXCEPT
          !.currentRoster =
            state.candidateLifecycleSnapshotV3.roster,
          !.serviceGeneration =
            state.candidateLifecycleSnapshotV3.serviceGeneration,
          !.nextStreamEpoch =
            state.candidateLifecycleSnapshotV3.nextStreamEpoch,
          !.serverStreamState = "Empty",
          !.requestGateState = "Empty",
          !.serverClosePrefix = 0,
          !.transferState = "Empty",
          !.flushState = "Empty",
          !.retryableChunk = 0,
          !.candidatePresent = FALSE,
          !.lifecycleCommitPhase = "Published",
          !.pendingRolloverAuthority = "None",
          !.transitionAuthority = "RestartRestore",
          !.successorActive = TRUE,
          !.restartRequired = FALSE,
          !.failureReason = "None"]

PublishMemoryBeforeLifecycleRootV3Commit ==
  /\ ValidatedCandidateSuccessorStateSlotAheadOfRoot
  /\ state.pendingRolloverAuthority \in ChangedRosterAuthorities
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.currentRoster =
            state.candidateLifecycleSnapshotV3.roster,
          !.serviceGeneration =
            state.candidateLifecycleSnapshotV3.serviceGeneration,
          !.nextStreamEpoch =
            state.candidateLifecycleSnapshotV3.nextStreamEpoch,
          !.serverStreamState = "Empty",
          !.requestGateState = "Empty",
          !.serverClosePrefix = 0,
          !.transferState = "Empty",
          !.flushState = "Empty",
          !.retryableChunk = 0,
          !.candidatePresent = FALSE,
          !.lifecycleCommitPhase = "Published",
          !.receiptStage =
            IF state.receiptStage = "Retained" THEN "Consumed"
            ELSE state.receiptStage,
          !.receiptConsumeCount =
            IF state.receiptStage = "Retained" THEN 1
            ELSE state.receiptConsumeCount,
          !.successorActive = TRUE,
          !.transitionAuthority = state.pendingRolloverAuthority,
          !.pendingRolloverAuthority = "None"]

CommitLifecycleRootV3BeforeStateSlot ==
  /\ ChangedRosterReplacementNeeded
  /\ ExactRetainedMergeSidecars
  /\ state.serviceGeneration < ServiceGenerationLimit
  /\ state.durableLifecycleRootV3.rootGeneration < RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.candidatePresent
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             "ChangedRoster",
             state.serviceGeneration + 1,
             state.nextStreamEpoch,
             "Empty",
             "Empty",
             0)
     IN state' =
          [state EXCEPT
             !.durableLifecycleRootV3 =
               LifecycleRootV3(snapshot),
             !.lifecycleCommitPhase = "RootCommitted",
             !.pendingRolloverAuthority = "DurableExactOutput"]

PublishSuccessorIntoRootSelectedStateSlot ==
  /\ ChangedRosterReplacementNeeded
  /\ ExactRetainedMergeSidecars
  /\ state.serviceGeneration < ServiceGenerationLimit
  /\ state.durableLifecycleRootV3.rootGeneration < RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.candidatePresent
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ ~state.restartRequired
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             "ChangedRoster",
             state.serviceGeneration + 1,
             state.nextStreamEpoch,
             "Empty",
             "Empty",
             0)
     IN state' =
          [state EXCEPT
             !.durableLifecycleStateSlotsV3[
               SelectedLifecycleStateSlot(state)] =
                 snapshot,
             !.candidateLifecycleSnapshotV3 = snapshot,
             !.candidatePresent = TRUE,
             !.candidateStateSlot =
               SelectedLifecycleStateSlot(state),
             !.candidateSemanticallyValidated = TRUE,
             !.lifecycleCommitPhase = "StateSlotPublished",
             !.pendingRolloverAuthority = "DurableExactOutput"]

RollActiveLifecycleStateAsOrdinary ==
  /\ ExactRetainedMergeSidecars
  /\ ChangedRosterReplacementNeeded
  /\ ~AllOldLifecycleTerminal
  /\ state.serviceGeneration < ServiceGenerationLimit
  /\ state.durableLifecycleRootV3.rootGeneration < RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.candidatePresent
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ ~state.restartRequired
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             "ChangedRoster",
             state.serviceGeneration + 1,
             state.nextStreamEpoch,
             "Empty",
             "Empty",
             0)
     IN state' =
          [state EXCEPT
             !.durableLifecycleStateSlotsV3[
               LifecycleStateSlot(snapshot.rootGeneration)] =
                 snapshot,
             !.candidateLifecycleSnapshotV3 = snapshot,
             !.candidatePresent = TRUE,
             !.candidateStateSlot =
               LifecycleStateSlot(snapshot.rootGeneration),
             !.candidateSemanticallyValidated = TRUE,
             !.lifecycleCommitPhase = "StateSlotPublished",
             !.pendingRolloverAuthority = "AuthenticatedTerminal"]

WrapServiceGenerationCounter ==
  /\ state.successorActive
  /\ state.serviceGeneration = ServiceGenerationLimit
  /\ state.durableLifecycleRootV3.rootGeneration <
       RootGenerationLimit
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             state.currentRoster,
             ServiceGenerationLimit + 1,
             state.nextStreamEpoch,
             state.serverStreamState,
             state.requestGateState,
             state.serverClosePrefix)
     IN state' =
          [state EXCEPT
             !.serviceGeneration = ServiceGenerationLimit + 1,
             !.durableLifecycleStateSlotsV3[
               LifecycleStateSlot(snapshot.rootGeneration)] =
                 snapshot,
             !.durableLifecycleRootV3 =
               LifecycleRootV3(snapshot)]

WrapLifecycleRootGenerationCounter ==
  /\ state.durableLifecycleRootV3.rootGeneration =
       RootGenerationLimit
  /\ LET snapshot ==
           [DurableSnapshot(state) EXCEPT
              !.rootGeneration = InitialRootGeneration - 1]
     IN state' =
          [state EXCEPT
             !.durableLifecycleStateSlotsV3[
               LifecycleStateSlot(snapshot.rootGeneration)] =
                 snapshot,
             !.durableLifecycleRootV3 =
               LifecycleRootV3(snapshot)]

ForgeClosePrefixDuringDurableFence ==
  /\ RootCommittedSuccessorAheadOfMemory
  /\ state.pendingRolloverAuthority = "DurableExactOutput"
  /\ state.authenticatedCloseHistory = 0
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.currentRoster = DurableSnapshot(state).roster,
          !.serviceGeneration =
            DurableSnapshot(state).serviceGeneration,
          !.nextStreamEpoch =
            DurableSnapshot(state).nextStreamEpoch,
          !.serverStreamState = "Empty",
          !.requestGateState = "Empty",
          !.serverClosePrefix = 0,
          !.recordedRetiredClosePrefix =
            HighestSemanticSequence,
          !.transferState = "Empty",
          !.flushState = "Empty",
          !.retryableChunk = 0,
          !.candidatePresent = FALSE,
          !.lifecycleCommitPhase = "Published",
          !.receiptStage = "Consumed",
          !.receiptConsumeCount = 1,
          !.successorActive = TRUE,
          !.transitionAuthority = "DurableExactOutput",
          !.pendingRolloverAuthority = "None"]

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
            DurableSnapshot(state).nextStreamEpoch,
          !.requesterEpochPhase = "InUse",
          !.pendingStreamEpoch = 0,
          !.activeStreamEpoch = state.skippedStreamEpoch,
          !.restartRequired = FALSE,
          !.failureReason = "None"]

WrapRequesterEpochCounter ==
  /\ state.requesterEpochPhase = "Idle"
  /\ state.nextStreamEpoch = StreamEpochLimit
  /\ state.durableLifecycleRootV3.rootGeneration <
       RootGenerationLimit
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             state.currentRoster,
             state.serviceGeneration,
             InitialNextStreamEpoch,
             state.serverStreamState,
             state.requestGateState,
             state.serverClosePrefix)
     IN state' =
          [state EXCEPT
             !.nextStreamEpoch = InitialNextStreamEpoch,
             !.durableLifecycleStateSlotsV3[
               LifecycleStateSlot(snapshot.rootGeneration)] =
                 snapshot,
             !.durableLifecycleRootV3 =
               LifecycleRootV3(snapshot),
             !.requesterEpochPhase = "InUse",
             !.activeStreamEpoch = StreamEpochLimit]

CrossLifecycleRootShape ==
  /\ state.durableLifecycleRootV3.shape = "Committed"
  /\ state' =
       [state EXCEPT
          !.durableLifecycleRootV3.shape = "Bootstrap"]

SplitLifecycleGenerationAndHash ==
  /\ state.durableLifecycleRootV3.shape = "Committed"
  /\ state.durableLifecycleRootV3.rootGeneration <
       RootGenerationLimit
  /\ LET nextGeneration ==
           state.durableLifecycleRootV3.rootGeneration + 1
         selectedSnapshot ==
           [DurableSnapshot(state) EXCEPT
              !.rootGeneration = nextGeneration]
     IN state' =
          [state EXCEPT
             !.durableLifecycleStateSlotsV3[
               LifecycleStateSlot(nextGeneration)] =
                 selectedSnapshot,
             !.durableLifecycleRootV3.rootGeneration =
               nextGeneration]

RemoveRootSelectedLifecycleState ==
  /\ state.durableLifecycleRootV3.shape = "Committed"
  /\ state' =
       [state EXCEPT
          !.durableLifecycleStateSlotsV3[
            SelectedLifecycleStateSlot(state)] =
              NoLifecycleSnapshot]

CleanupLifecycleArtifactsBeforeValidation ==
  /\ ~state.durableJournalValidated
  /\ state.crashArtifactsPresent
  /\ ~state.cleanupPerformed
  /\ state' =
       [state EXCEPT
          !.crashArtifactsPresent = FALSE,
          !.cleanupPerformed = TRUE]

ChangeRosterWithoutServiceGenerationAdvance ==
  /\ ChangedRosterReplacementNeeded
  /\ state.durableLifecycleRootV3.shape = "Committed"
  /\ state.durableLifecycleRootV3.rootGeneration <
       RootGenerationLimit
  /\ state.serviceGeneration = InitialServiceGeneration
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.candidatePresent
  /\ LifecycleJournalReady(state)
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ ~state.restartRequired
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             "ChangedRoster",
             InitialServiceGeneration,
             state.nextStreamEpoch,
             state.serverStreamState,
             state.requestGateState,
             state.serverClosePrefix)
     IN state' =
          [state EXCEPT
             !.durableLifecycleStateSlotsV3[
               LifecycleStateSlot(snapshot.rootGeneration)] =
                 snapshot,
             !.durableLifecycleRootV3 =
               LifecycleRootV3(snapshot),
             !.currentRoster = "ChangedRoster"]

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
    [] MutationMode = "SameRosterGenerationRoll" ->
         SameRosterAdvancesGeneration
    [] MutationMode = "LateOldCallback" ->
         LateOldWriterMutatesSuccessor
    [] MutationMode = "UntypedForce" ->
         UntypedActiveLifecycleForce
    [] MutationMode = "CleanStateSlotV3PersistenceFailure" ->
         CleanFailStateSlotV3Persistence
    [] MutationMode = "CleanCrashAfterLifecycleRootV3Commit" ->
         CleanCrashAfterLifecycleRootV3Commit
    [] MutationMode = "SkipLifecycleRootV3CrashHistory" ->
         CrashWithoutLifecycleRootV3History
    [] MutationMode = "RecoverUncommittedStateSlot" ->
         RecoverUncommittedStateSlotAsSuccessor
    [] MutationMode = "PublishMemoryBeforeLifecycleRootV3Commit" ->
         PublishMemoryBeforeLifecycleRootV3Commit
    [] MutationMode = "CommitLifecycleRootV3BeforeStateSlot" ->
         CommitLifecycleRootV3BeforeStateSlot
    [] MutationMode = "ReuseRootSelectedStateSlot" ->
         PublishSuccessorIntoRootSelectedStateSlot
    [] MutationMode = "ActiveOrdinaryRoll" ->
         RollActiveLifecycleStateAsOrdinary
    [] MutationMode = "ServiceGenerationOverflowWrap" ->
         WrapServiceGenerationCounter
    [] MutationMode = "RootGenerationOverflowWrap" ->
         WrapLifecycleRootGenerationCounter
    [] MutationMode = "ForgeAuthenticatedClosePrefix" ->
         ForgeClosePrefixDuringDurableFence
    [] MutationMode = "EpochUseBeforePersist" ->
         UseRequesterEpochBeforePersistence
    [] MutationMode = "EpochReuseAfterCrash" ->
         ReuseRequesterEpochAfterCrash
    [] MutationMode = "EpochOverflowWrap" ->
         WrapRequesterEpochCounter
    [] MutationMode = "CrossedLifecycleRootShape" ->
         CrossLifecycleRootShape
    [] MutationMode = "SplitLifecycleGenerationHash" ->
         SplitLifecycleGenerationAndHash
    [] MutationMode = "MissingRootSelectedState" ->
         RemoveRootSelectedLifecycleState
    [] MutationMode = "CleanupBeforeSemanticValidation" ->
         CleanupLifecycleArtifactsBeforeValidation
    [] MutationMode = "ChangedRosterWithoutGenerationAdvance" ->
         ChangeRosterWithoutServiceGenerationAdvance

MutationNext ==
  \/ Next
  \/ SelectedMutationAction

MutationSpec ==
  /\ MutationConfiguration
  /\ Init
  /\ [][MutationNext]_typedRolloverVars

=============================================================================
