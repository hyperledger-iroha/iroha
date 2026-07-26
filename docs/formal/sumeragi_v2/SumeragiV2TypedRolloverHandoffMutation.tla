---- MODULE SumeragiV2TypedRolloverHandoffMutation ----
EXTENDS SumeragiV2TypedRolloverHandoff

(***************************************************************************
Negative controls for the authority-gated V3 lifecycle and typed handoff.

The controls cover the independently created service/transport owner pair,
immutable receipt inputs, same-roster preservation, ordinary terminality,
process-incarnation loss on crash, installed-versus-parent-synced state/root
ordering, second-crash cleanup safety, bootstrap fidelity, fail-closed startup
validation, exact requester-incarnation restoration, checked counters, and the
prohibition on forged requester-authenticated Close prefixes.
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
   "LoseRequesterIncarnationAfterCrash",
   "EpochOverflowWrap",
   "CrossedLifecycleRootShape",
   "SplitLifecycleGenerationHash",
   "MissingRootSelectedState",
   "CleanupBeforeSemanticValidation",
   "ChangedRosterWithoutGenerationAdvance",
   "CrossServiceTransportOwnerPair",
   "PreserveProcessReceiptAcrossCrash",
   "CleanupRetainsInactiveSlot",
   "WrongBootstrapLifecycleProjection",
   "SkipBootstrapCrashHistory",
   "CleanupBeforeRootParentResync",
   "AcceptSemanticInvalidLifecycleState"}

MutationConfiguration ==
  MutationMode \in MutationModes

PrematureSealAndMint ==
  /\ state.finalityValidated
  /\ state.workerOutstanding > 0
  /\ state.receiptStage = "Absent"
  /\ ExactServiceTransportOwnerPair
  /\ state' =
       [state EXCEPT
          !.ownerSealed = TRUE,
          !.receiptStage = "Minted",
          !.receiptOwnerNonce = state.serviceOwnerNonce,
          !.receiptContext = ExpectedContext,
          !.receiptArtifact = ExpectedArtifact]

RetainForeignOwnerReceipt ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ state.presentedHandoffCandidate = "ForeignOwner"
  /\ state' =
       [state EXCEPT
          !.receiptStage = "Retained",
          !.receiptOwnerNonce = ForeignOwnerNonce,
          !.retainedSuccessor = ExpectedSuccessor]

IgnoreForeignOwnerCandidate ==
  /\ state.presentedHandoffCandidate = "ForeignOwner"
  /\ ~state.foreignReceiptRejected
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT !.foreignReceiptRejected = TRUE]

CleanRejectForeignOwnerReceipt ==
  IgnoreForeignOwnerCandidate

AcceptPredecessorContextMismatch ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ state.presentedHandoffCandidate =
       "PredecessorContextMismatch"
  /\ state' =
       [state EXCEPT
          !.receiptStage = "Retained",
          !.receiptContext = "ContextB",
          !.retainedSuccessor = ExpectedSuccessor]

AcceptPredecessorArtifactMismatch ==
  /\ state.receiptStage = "Minted"
  /\ ExactSuccessorConstruction
  /\ state.presentedHandoffCandidate =
       "PredecessorArtifactMismatch"
  /\ state' =
       [state EXCEPT
          !.receiptStage = "Retained",
          !.receiptArtifact = "ArtifactB",
          !.retainedSuccessor = ExpectedSuccessor]

CleanRejectPredecessorContextMismatch ==
  /\ state.presentedHandoffCandidate =
       "PredecessorContextMismatch"
  /\ ~state.predecessorMismatchRejected
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT !.predecessorMismatchRejected = TRUE]

CleanRejectPredecessorArtifactMismatch ==
  /\ state.presentedHandoffCandidate =
       "PredecessorArtifactMismatch"
  /\ ~state.predecessorMismatchRejected
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT !.predecessorMismatchRejected = TRUE]

RetainForeignSuccessor ==
  /\ state.receiptStage = "Minted"
  /\ ExactPredecessorReceipt
  /\ ExactSuccessorConstruction
  /\ state.presentedHandoffCandidate =
       "ImmediateSuccessorMismatch"
  /\ state' =
       [state EXCEPT
          !.receiptStage = "Retained",
          !.retainedSuccessor = ForeignSuccessor]

CleanRejectWrongImmediateSuccessor ==
  /\ state.presentedHandoffCandidate =
       "ImmediateSuccessorMismatch"
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
  /\ state.targetRoster = state.currentRoster
  /\ state.compactionCause = "NoCompaction"
  /\ state.startupMode # "BootstrapStart"
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
  /\ state.targetRoster = state.currentRoster
  /\ state.currentRoster = state.baselineRoster
  /\ state.startupMode # "BootstrapStart"
  /\ ExactRetainedMergeSidecars
  /\ state.serviceGeneration < ServiceGenerationLimit
  /\ state.durableLifecycleRootV3.rootGeneration <
       RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             state.currentRoster,
             state.serviceGeneration + 1,
             state.nextStreamEpoch,
             DurableSnapshot(state).requesterStreamEpoch,
             "Empty",
             "Empty",
             0)
     IN state' =
       [state EXCEPT
          !.durableLifecycleStateSlotsV3[
            SelectedLifecycleStateSlot(state)] =
              NoLifecycleSnapshot,
          !.durableLifecycleStateSlotsV3[
            LifecycleStateSlot(snapshot.rootGeneration)] =
              snapshot,
          !.syncedLifecycleStateSlotsV3[
            SelectedLifecycleStateSlot(state)] =
              NoLifecycleSnapshot,
          !.syncedLifecycleStateSlotsV3[
            LifecycleStateSlot(snapshot.rootGeneration)] =
              snapshot,
          !.durableLifecycleRootV3 = LifecycleRootV3(snapshot),
          !.syncedLifecycleRootV3 = LifecycleRootV3(snapshot),
          !.serviceGeneration = state.serviceGeneration + 1,
          !.serverStreamState = "Empty",
          !.requestGateState = "Empty",
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
  /\ state.durableLifecycleRootV3.rootGeneration <
       RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.restartRequired
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             state.targetRoster,
             state.serviceGeneration + 1,
             state.nextStreamEpoch,
             DurableSnapshot(state).requesterStreamEpoch,
             "Empty",
             "Empty",
             0)
     IN state' =
       [state EXCEPT
          !.durableLifecycleStateSlotsV3[
            SelectedLifecycleStateSlot(state)] =
              NoLifecycleSnapshot,
          !.durableLifecycleStateSlotsV3[
            LifecycleStateSlot(snapshot.rootGeneration)] =
              snapshot,
          !.syncedLifecycleStateSlotsV3[
            SelectedLifecycleStateSlot(state)] =
              NoLifecycleSnapshot,
          !.syncedLifecycleStateSlotsV3[
            LifecycleStateSlot(snapshot.rootGeneration)] =
              snapshot,
          !.durableLifecycleRootV3 = LifecycleRootV3(snapshot),
          !.syncedLifecycleRootV3 = LifecycleRootV3(snapshot),
          !.currentRoster = state.targetRoster,
          !.compactionCause = "NoCompaction",
          !.serviceGeneration = state.serviceGeneration + 1,
          !.serverStreamState = "Empty",
          !.requestGateState = "Empty",
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
  /\ state.restartRequired
  /\ state.durableLifecycleRootV3.shape = "Committed"
  /\ InactiveLifecycleArtifactPresent(state)
  /\ LET uncommitted ==
           state.durableLifecycleStateSlotsV3[
             InactiveLifecycleStateSlot(state)]
     IN state' =
       [state EXCEPT
          !.currentRoster = uncommitted.roster,
          !.compactionCause = "NoCompaction",
          !.serviceGeneration = uncommitted.serviceGeneration,
          !.nextStreamEpoch = uncommitted.nextStreamEpoch,
          !.serverStreamState = uncommitted.serverStreams,
          !.requestGateState = uncommitted.requestGates,
          !.serverClosePrefix = uncommitted.serverClosePrefix,
          !.transferState = "Empty",
          !.flushState = "Empty",
          !.retryableChunk = 0,
          !.lifecycleCommitPhase = "Published",
          !.transitionAuthority = "RestartRestore",
          !.successorActive = TRUE,
          !.restartRequired = FALSE,
          !.failureReason = "None"]

PublishMemoryBeforeLifecycleRootV3Commit ==
  /\ ValidatedCandidateSuccessorStateSlotAheadOfRoot
  /\ state.lifecycleCommitPhase \in
       {"StateSlotReplaced", "StateSlotPublished"}
  /\ state.pendingRolloverAuthority \in ChangedRosterAuthorities
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.currentRoster =
            state.candidateLifecycleSnapshotV3.roster,
          !.compactionCause = "NoCompaction",
          !.serviceGeneration =
            state.candidateLifecycleSnapshotV3.serviceGeneration,
          !.nextStreamEpoch =
            state.candidateLifecycleSnapshotV3.nextStreamEpoch,
          !.serverStreamState = "Empty",
          !.requestGateState = "Empty",
          !.transferState = "Empty",
          !.flushState = "Empty",
          !.retryableChunk = 0,
          !.candidatePresent = FALSE,
          !.lifecycleCommitPhase = "Published",
          !.receiptStage =
            IF state.receiptStage = "Retained"
              THEN "Consumed"
              ELSE state.receiptStage,
          !.receiptConsumeCount =
            IF state.receiptStage = "Retained"
              THEN 1
              ELSE state.receiptConsumeCount,
          !.successorActive = TRUE,
          !.transitionAuthority = state.pendingRolloverAuthority,
          !.pendingRolloverAuthority = "None"]

CommitLifecycleRootV3BeforeStateSlot ==
  /\ ChangedRosterReplacementNeeded
  /\ ExactRetainedMergeSidecars
  /\ state.serviceGeneration < ServiceGenerationLimit
  /\ state.durableLifecycleRootV3.rootGeneration <
       RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.candidatePresent
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             state.targetRoster,
             state.serviceGeneration + 1,
             state.nextStreamEpoch,
             DurableSnapshot(state).requesterStreamEpoch,
             "Empty",
             "Empty",
             0)
     IN state' =
       [state EXCEPT
          !.durableLifecycleRootV3 = LifecycleRootV3(snapshot),
          !.syncedLifecycleRootV3 = LifecycleRootV3(snapshot),
          !.lifecycleCommitPhase = "RootCommitted",
          !.pendingRolloverAuthority = "DurableExactOutput"]

PublishSuccessorIntoRootSelectedStateSlot ==
  /\ ChangedRosterReplacementNeeded
  /\ ExactRetainedMergeSidecars
  /\ state.serviceGeneration < ServiceGenerationLimit
  /\ state.durableLifecycleRootV3.rootGeneration <
       RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.candidatePresent
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ ~state.restartRequired
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             state.targetRoster,
             state.serviceGeneration + 1,
             state.nextStreamEpoch,
             DurableSnapshot(state).requesterStreamEpoch,
             "Empty",
             "Empty",
             0)
     IN state' =
       [state EXCEPT
          !.durableLifecycleStateSlotsV3[
            SelectedLifecycleStateSlot(state)] =
              snapshot,
          !.syncedLifecycleStateSlotsV3[
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
  /\ state.durableLifecycleRootV3.rootGeneration <
       RootGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.candidatePresent
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ ~state.restartRequired
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             state.targetRoster,
             state.serviceGeneration + 1,
             state.nextStreamEpoch,
             DurableSnapshot(state).requesterStreamEpoch,
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
          !.lifecycleCommitPhase = "StateSlotReplaced",
          !.pendingRolloverAuthority = "AuthenticatedTerminal"]

WrapServiceGenerationCounter ==
  /\ ChangedRosterReplacementNeeded
  /\ ExactRetainedMergeSidecars
  /\ state.serviceGeneration = ServiceGenerationLimit
  /\ state.lifecycleCommitPhase = "Current"
  /\ state' =
       [state EXCEPT
          !.serviceGeneration = ServiceGenerationLimit + 1]

WrapLifecycleRootGenerationCounter ==
  /\ state.syncedLifecycleRootV3.rootGeneration =
       RootGenerationLimit
  /\ LET snapshot ==
           [SyncedLifecycleSnapshot(state) EXCEPT
              !.rootGeneration = InitialRootGeneration - 1]
     IN state' =
       [state EXCEPT
          !.durableLifecycleStateSlotsV3[
            LifecycleStateSlot(snapshot.rootGeneration)] =
              snapshot,
          !.syncedLifecycleStateSlotsV3[
            LifecycleStateSlot(snapshot.rootGeneration)] =
              snapshot,
          !.durableLifecycleRootV3 = LifecycleRootV3(snapshot),
          !.syncedLifecycleRootV3 = LifecycleRootV3(snapshot)]

ForgeClosePrefixDuringDurableFence ==
  /\ RootCommittedSuccessorAheadOfMemory
  /\ state.pendingRolloverAuthority = "DurableExactOutput"
  /\ state.authenticatedCloseHistory = 0
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.currentRoster = DurableSnapshot(state).roster,
          !.compactionCause = "NoCompaction",
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
          !.activeStreamEpoch = state.nextStreamEpoch + 1]

LoseRequesterIncarnationAfterCrash ==
  /\ state.restartRequired
  /\ state.failureReason =
       "CrashAfterRequesterEpochPersistence"
  /\ state.epochCrashObserved
  /\ state.durableJournalValidated
  /\ state.cleanupPerformed
  /\ state' =
       [state EXCEPT
          !.nextStreamEpoch =
            DurableSnapshot(state).nextStreamEpoch,
          !.requesterEpochPhase = "Idle",
          !.pendingStreamEpoch = 0,
          !.activeStreamEpoch = 0,
          !.requesterEpochReplacementRestored = FALSE,
          !.lifecycleCommitPhase = "Current",
          !.restartRequired = FALSE,
          !.failureReason = "None"]

WrapRequesterEpochCounter ==
  /\ state.requesterEpochPhase = "Idle"
  /\ state.nextStreamEpoch = StreamEpochLimit
  /\ state' =
       [state EXCEPT
          !.nextStreamEpoch = InitialNextStreamEpoch,
          !.requesterEpochPhase = "InUse",
          !.activeStreamEpoch = StreamEpochLimit]

AcceptInvalidLifecycleStartup(reason) ==
  /\ state.startupMode = "UnvalidatedRestart"
  /\ state.startupValidationFault = reason
  /\ state.restartRequired
  /\ ~state.durableJournalValidated
  /\ state.crashArtifactsPresent
  /\ state' =
       [state EXCEPT
          !.durableJournalValidated = TRUE,
          !.validatedRestartObserved = TRUE,
          !.restartStateDirectoryResynced = TRUE,
          !.restartRootDirectoryResynced = TRUE,
          !.crashArtifactsPresent = FALSE,
          !.cleanupPerformed = TRUE,
          !.lifecycleCommitPhase = "Current",
          !.restartRequired = FALSE,
          !.failureReason = "None"]

CrossLifecycleRootShape ==
  AcceptInvalidLifecycleStartup("LifecycleRootShapeMismatch")

SplitLifecycleGenerationAndHash ==
  AcceptInvalidLifecycleStartup(
    "LifecycleGenerationHashMismatch")

RemoveRootSelectedLifecycleState ==
  AcceptInvalidLifecycleStartup("LifecycleSelectedStateMissing")

AcceptSemanticInvalidLifecycleState ==
  AcceptInvalidLifecycleStartup(
    "LifecycleSemanticValidationFailure")

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
  /\ state.lifecycleCommitPhase = "Current"
  /\ ~state.candidatePresent
  /\ LifecycleJournalReady(state)
  /\ LifecycleMemoryMatchesDurableSnapshotV3
  /\ ~state.restartRequired
  /\ LET snapshot ==
           LifecycleSnapshotV3(
             state.durableLifecycleRootV3.rootGeneration + 1,
             state.targetRoster,
             state.serviceGeneration,
             state.nextStreamEpoch,
             DurableSnapshot(state).requesterStreamEpoch,
             state.serverStreamState,
             state.requestGateState,
             state.serverClosePrefix)
     IN state' =
       [state EXCEPT
          !.durableLifecycleStateSlotsV3[
            SelectedLifecycleStateSlot(state)] =
              NoLifecycleSnapshot,
          !.durableLifecycleStateSlotsV3[
            LifecycleStateSlot(snapshot.rootGeneration)] =
              snapshot,
          !.syncedLifecycleStateSlotsV3[
            SelectedLifecycleStateSlot(state)] =
              NoLifecycleSnapshot,
          !.syncedLifecycleStateSlotsV3[
            LifecycleStateSlot(snapshot.rootGeneration)] =
              snapshot,
          !.durableLifecycleRootV3 = LifecycleRootV3(snapshot),
          !.syncedLifecycleRootV3 = LifecycleRootV3(snapshot),
          !.currentRoster = state.targetRoster,
          !.compactionCause = "NoCompaction"]

CrossServiceTransportOwnerPair ==
  /\ ExactServiceTransportOwnerPair
  /\ state.receiptStage = "Absent"
  /\ state' =
       [state EXCEPT
          !.transportOwnerNonce = ForeignOwnerNonce]

PreserveProcessReceiptAcrossCrash ==
  /\ RequesterEpochPersistenceAheadOfMemory
  /\ ExactRetainedMergeSidecars
  /\ ~state.restartRequired
  /\ state' =
       [state EXCEPT
          !.lifecycleCommitPhase = "Restarting",
          !.requesterEpochPhase = "Restarting",
          !.pendingStreamEpoch = 0,
          !.durableJournalValidated = FALSE,
          !.validatedRestartObserved = FALSE,
          !.restartStateDirectoryResynced = FALSE,
          !.restartRootDirectoryResynced = FALSE,
          !.crashArtifactsPresent = TRUE,
          !.cleanupPerformed = FALSE,
          !.restartRequired = TRUE,
          !.failureReason =
            "CrashAfterRequesterEpochPersistence",
          !.epochCrashObserved = TRUE]

CleanupRetainsInactiveSlot ==
  /\ state.restartRequired
  /\ state.durableJournalValidated
  /\ state.restartStateDirectoryResynced
  /\ state.restartRootDirectoryResynced
  /\ InactiveLifecycleArtifactPresent(state)
  /\ ~state.cleanupPerformed
  /\ state' =
       [state EXCEPT
          !.crashArtifactsPresent = FALSE,
          !.cleanupPerformed = TRUE]

WrongBootstrapLifecycleProjection ==
  /\ state.lifecycleCommitPhase = "BootstrapStatePublished"
  /\ state.durableLifecycleRootV3 = BootstrapLifecycleRootV3
  /\ state.candidatePresent
  /\ state.candidateSemanticallyValidated
  /\ LifecycleStateDirectoryIsSynced(state)
  /\ state.targetRoster = state.currentRoster
  /\ state.currentRoster = state.baselineRoster
  /\ state.compactionCause = "NoCompaction"
  /\ ~state.restartRequired
  /\ LET wrongTarget ==
           IF state.targetRoster = "RosterA"
             THEN "RosterB"
             ELSE "RosterA"
     IN state' =
       [state EXCEPT
          !.targetRoster = wrongTarget,
          !.compactionCause = "RosterGeometryReplacement",
          !.durableLifecycleRootV3 =
            LifecycleRootV3(state.candidateLifecycleSnapshotV3),
          !.lifecycleCommitPhase = "BootstrapRootReplaced"]

SkipBootstrapCrashHistory ==
  /\ state.lifecycleCommitPhase = "BootstrapStatePublished"
  /\ LifecycleStateDirectoryIsSynced(state)
  /\ ~state.restartRequired
  /\ state' =
       [CrashClearedProcessLocalRolloverState(state) EXCEPT
          !.lifecycleCommitPhase = "Restarting",
          !.durableJournalValidated = FALSE,
          !.validatedRestartObserved = FALSE,
          !.restartRequired = TRUE,
          !.failureReason =
            "CrashAfterBootstrapStatePublication"]

CleanupBeforeRootParentResync ==
  /\ state.restartRequired
  /\ state.durableJournalValidated
  /\ state.restartStateDirectoryResynced
  /\ ~state.restartRootDirectoryResynced
  /\ state.durableLifecycleRootV3.shape = "Committed"
  /\ InactiveLifecycleArtifactPresent(state)
  /\ state' =
       [state EXCEPT
          !.durableLifecycleStateSlotsV3[
            InactiveLifecycleStateSlot(state)] =
              NoLifecycleSnapshot,
          !.syncedLifecycleStateSlotsV3[
            InactiveLifecycleStateSlot(state)] =
              NoLifecycleSnapshot,
          !.crashArtifactsPresent = FALSE,
          !.cleanupPerformed = TRUE]

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
    [] MutationMode = "LoseRequesterIncarnationAfterCrash" ->
         LoseRequesterIncarnationAfterCrash
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
    [] MutationMode = "CrossServiceTransportOwnerPair" ->
         CrossServiceTransportOwnerPair
    [] MutationMode = "PreserveProcessReceiptAcrossCrash" ->
         PreserveProcessReceiptAcrossCrash
    [] MutationMode = "CleanupRetainsInactiveSlot" ->
         CleanupRetainsInactiveSlot
    [] MutationMode = "WrongBootstrapLifecycleProjection" ->
         WrongBootstrapLifecycleProjection
    [] MutationMode = "SkipBootstrapCrashHistory" ->
         SkipBootstrapCrashHistory
    [] MutationMode = "CleanupBeforeRootParentResync" ->
         CleanupBeforeRootParentResync
    [] MutationMode = "AcceptSemanticInvalidLifecycleState" ->
         AcceptSemanticInvalidLifecycleState

MutationNext ==
  \/ Next
  \/ SelectedMutationAction

MutationSpec ==
  /\ MutationConfiguration
  /\ Init
  /\ [][MutationNext]_typedRolloverVars

=============================================================================
