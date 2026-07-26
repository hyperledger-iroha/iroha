---- MODULE SumeragiV2TypedRolloverHandoffProofs ----
EXTENDS SumeragiV2TypedRolloverHandoff, TLAPS

(***************************************************************************
Deductive obligations for the authority-gated, root-anchored V3 typed
rollover model.

The declarations separate move-only durable exact-output authority from
validated restart authority.  They also expose both filesystem layers used by
the model: replacement writes visible to the process and parent-directory
synchronization.  A crash can roll a replacement back to the last synchronized
state/root pair, and cleanup cannot remove the inactive slot until validation
and ordered resynchronization have completed.

All declarations remain specified_unproved.  Bounded mutation evidence does
not discharge them, and neither liveness declaration is a Rust-to-TLA
refinement or an unbounded distributed-progress claim.
***************************************************************************)

THEOREM TypedRolloverInitEstablishesSafetyObligation ==
  Init => TypedRolloverSafetyInvariant

THEOREM BootstrapRootHasExactGenerationZeroShapeObligation ==
  /\ Init
  /\ state.startupMode = "BootstrapStart"
  =>
    /\ state.durableLifecycleRootV3 = BootstrapLifecycleRootV3
    /\ state.syncedLifecycleRootV3 = BootstrapLifecycleRootV3
    /\ \A slot \in StateSlots:
         state.durableLifecycleStateSlotsV3[slot] =
           NoLifecycleSnapshot
    /\ state.durableLifecycleStateSlotsV3 =
         state.syncedLifecycleStateSlotsV3
    /\ state.lifecycleCommitPhase = "Bootstrap"

THEOREM FreshBootstrapUsesTargetGeometryEpochZeroObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishInitialLifecycleStateSlotV3
  =>
    /\ state'.candidateLifecycleSnapshotV3 =
         InitialLifecycleSnapshotV3(state.targetRoster)
    /\ state'.candidateLifecycleSnapshotV3.roster =
         state.targetRoster
    /\ state'.candidateLifecycleSnapshotV3.serviceGeneration =
         InitialServiceGeneration
    /\ state'.candidateLifecycleSnapshotV3.nextStreamEpoch = 0
    /\ state'.candidateLifecycleSnapshotV3.requesterStreamEpoch = 0
    /\ state'.candidateLifecycleSnapshotV3.serverStreams = "Empty"
    /\ state'.candidateLifecycleSnapshotV3.requestGates = "Empty"
    /\ state'.candidateLifecycleSnapshotV3.serverClosePrefix = 0

THEOREM BootstrapStateReplacementRequiresDirectorySyncObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ ReplaceInitialLifecycleRootV3
  =>
    /\ LifecycleStateDirectoryIsSynced(state)
    /\ state.candidateSemanticallyValidated
    /\ state'.durableLifecycleRootV3 =
         LifecycleRootV3(state.candidateLifecycleSnapshotV3)
    /\ state'.syncedLifecycleRootV3 =
         state.syncedLifecycleRootV3

THEOREM BootstrapCrashRecoveryObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ (CrashAfterBootstrapStateReplacement
       \/ CrashAfterBootstrapStatePublication
       \/ CrashAfterBootstrapRootReplacement)
  =>
    /\ state'.restartRequired
    /\ ~state'.durableJournalValidated
    /\ state'.bootstrapCrashObserved
    /\ ~state'.candidatePresent
    /\ state'.serviceOwnerNonce = NoIdentity
    /\ state'.transportOwnerNonce = NoIdentity
    /\ state'.receiptStage \in {"Absent", "Lost"}

THEOREM BootstrapFirstCommitSelectsExactInitialPairObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ CommitInitialLifecycleRootV3
  =>
    /\ state'.durableLifecycleRootV3 =
         LifecycleRootV3(
           InitialLifecycleSnapshotV3(state.targetRoster))
    /\ state'.syncedLifecycleRootV3 =
         state'.durableLifecycleRootV3
    /\ SelectedLifecycleStateSlot(state') =
         LifecycleStateSlot(InitialRootGeneration)
    /\ SelectedLifecycleSnapshotV3(state') =
         InitialLifecycleSnapshotV3(state.targetRoster)
    /\ RootSelectedLifecyclePairMatches(state')
    /\ LifecycleMemory(state') = LifecycleMemory(state)

THEOREM ExactOwnerPairRequiredForRetainedHandoffObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RetainExactHandoffReceipt
  =>
    /\ state.serviceOwnerNonce # NoIdentity
    /\ state.serviceOwnerNonce = state.transportOwnerNonce
    /\ state'.receiptOwnerNonce = state.serviceOwnerNonce
    /\ state'.receiptStage = "Retained"

THEOREM EveryCrashDropsProcessLocalAuthorityObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ Next
  /\ state.durableJournalValidated
  /\ ~state'.durableJournalValidated
  /\ state'.restartRequired
  =>
    /\ ~state'.finalityValidated
    /\ ~state'.workerIngressClosed
    /\ state'.workerOutstanding = InitialWorkerOutstanding
    /\ ~state'.ownerSealed
    /\ state'.serviceOwnerNonce = NoIdentity
    /\ state'.transportOwnerNonce = NoIdentity
    /\ state'.constructionParent = NoIdentity
    /\ state'.constructionSuccessor = NoIdentity
    /\ state'.presentedHandoffCandidate = "None"
    /\ state'.receiptStage \in {"Absent", "Lost"}
    /\ state'.receiptOwnerNonce = NoIdentity
    /\ state'.receiptContext = NoIdentity
    /\ state'.receiptArtifact = NoIdentity
    /\ state'.retainedSuccessor = NoIdentity
    /\ ~state'.candidatePresent
    /\ state'.pendingRolloverAuthority = "None"
    /\ state'.transitionAuthority = "None"
    /\ ~state'.restartFenceAuthorized

THEOREM OnlyValidatedRestartMayFenceAfterCrashObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ Next
  /\ ~state.restartFenceAuthorized
  /\ state'.restartFenceAuthorized
  =>
    /\ state.durableJournalValidated
    /\ state.cleanupPerformed
    /\ state.restartStateDirectoryResynced
    /\ state.restartRootDirectoryResynced
    /\ state'.validatedRestartObserved
    /\ state'.targetRoster # state'.currentRoster

THEOREM StateReplacementRequiresDirectorySyncBeforeRootReplacementObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ ReplaceSuccessorLifecycleRootV3
  =>
    /\ state.lifecycleCommitPhase = "StateSlotPublished"
    /\ LifecycleStateDirectoryIsSynced(state)
    /\ state.candidateSemanticallyValidated
    /\ state'.durableLifecycleRootV3 =
         LifecycleRootV3(state.candidateLifecycleSnapshotV3)
    /\ state'.syncedLifecycleRootV3 =
         state.syncedLifecycleRootV3
    /\ LifecycleMemory(state') = LifecycleMemory(state)

THEOREM RootReplacementRequiresStoreSyncBeforeMemoryPublicationObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishCommittedLifecycleV3ToMemory
  =>
    /\ state.lifecycleCommitPhase = "RootCommitted"
    /\ LifecycleStateDirectoryIsSynced(state)
    /\ LifecycleRootDirectoryIsSynced(state)
    /\ RootSelectedLifecyclePairMatches(state)
    /\ state'.serviceGeneration =
         DurableSnapshot(state).serviceGeneration
    /\ state'.lifecycleCommitPhase = "Published"

THEOREM RootSelectedPairBindsGenerationAndDigestObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ state.durableLifecycleRootV3.shape = "Committed"
  =>
    /\ RootSelectedLifecyclePairIsPresent(state)
    /\ SelectedLifecycleSnapshotV3(state).rootGeneration =
         state.durableLifecycleRootV3.rootGeneration
    /\ state.durableLifecycleRootV3.snapshotDigest =
         LifecycleSnapshotDigest(
           SelectedLifecycleSnapshotV3(state))
    /\ LifecycleSnapshotSemanticallyValid(
         SelectedLifecycleSnapshotV3(state))

THEOREM MissingRootSelectedStateCannotValidateOrCleanupObligation ==
  /\ ~RootSelectedLifecyclePairIsPresent(state)
  /\ (ValidateRootSelectedLifecycleV3
       \/ CleanupValidatedLifecycleArtifactsV3)
  =>
    FALSE

THEOREM ValidationFailurePreservesArtifactsObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ ~state.validationFailureObserved
  /\ state'.validationFailureObserved
  /\ Next
  =>
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ CandidateLifecycle(state') = CandidateLifecycle(state)
    /\ state'.crashArtifactsPresent
    /\ ~state'.cleanupPerformed
    /\ ~state'.durableJournalValidated
    /\ ~state'.successorActive

THEOREM SemanticValidationPrecedesArtifactCleanupObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ CleanupValidatedLifecycleArtifactsV3
  =>
    /\ state.durableJournalValidated
    /\ RootSelectedLifecyclePairMatches(state)
    /\ LifecycleSnapshotSemanticallyValid(DurableSnapshot(state))
    /\ state.restartStateDirectoryResynced
    /\ state.restartRootDirectoryResynced
    /\ state'.cleanupPerformed
    /\ ~state'.crashArtifactsPresent

THEOREM ValidatedCleanupRemovesInactiveSlotObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ CleanupValidatedLifecycleArtifactsV3
  =>
    /\ state'.durableLifecycleStateSlotsV3[
         InactiveLifecycleStateSlot(state')] =
         NoLifecycleSnapshot
    /\ state'.syncedLifecycleStateSlotsV3[
         InactiveLifecycleStateSlot(state')] =
         NoLifecycleSnapshot
    /\ LifecycleStateDirectoryIsSynced(state')
    /\ LifecycleRootDirectoryIsSynced(state')

THEOREM SecondCrashBeforeRootResyncPreservesPredecessorObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ CrashDuringValidatedRestartBeforeRootResyncV3
  =>
    /\ state'.durableLifecycleRootV3 =
         state.syncedLifecycleRootV3
    /\ state'.durableLifecycleStateSlotsV3 =
         state.syncedLifecycleStateSlotsV3
    /\ state'.durableLifecycleRootV3 =
         state'.syncedLifecycleRootV3
    /\ state'.durableLifecycleStateSlotsV3 =
         state'.syncedLifecycleStateSlotsV3
    /\ state'.restartRequired
    /\ ~state'.durableJournalValidated
    /\ state'.secondCrashObserved
    /\ ~state'.cleanupPerformed

THEOREM RootGenerationAdvancesExactlyOnceAndAlternatesSlotObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ Next
  /\ state'.syncedLifecycleRootV3.rootGeneration >
       state.syncedLifecycleRootV3.rootGeneration
  =>
    /\ state'.syncedLifecycleRootV3.rootGeneration =
         state.syncedLifecycleRootV3.rootGeneration + 1
    /\ SyncedSelectedLifecycleStateSlot(state') =
         LifecycleStateSlot(
           state'.syncedLifecycleRootV3.rootGeneration)
    /\ SyncedSelectedLifecycleStateSlot(state') #
         SyncedSelectedLifecycleStateSlot(state)
    /\ state'.syncedLifecycleRootV3.snapshotDigest =
         LifecycleSnapshotDigest(
           SyncedSelectedLifecycleSnapshotV3(state'))

THEOREM MemoryPublicationRequiresCommittedV3RootObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishCommittedLifecycleV3ToMemory
  =>
    /\ DurableSnapshot(state).version = 3
    /\ DurableSnapshot(state).serverStreams = "Empty"
    /\ DurableSnapshot(state).requestGates = "Empty"
    /\ DurableSnapshot(state).serverClosePrefix = 0
    /\ state'.serviceGeneration =
         DurableSnapshot(state).serviceGeneration
    /\ state'.lifecycleCommitPhase = "Published"

THEOREM OrdinaryRolloverRequiresAuthenticatedTerminalityObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishSuccessorLifecycleStateSlotV3
  /\ state'.pendingRolloverAuthority = "AuthenticatedTerminal"
  =>
    AllOldLifecycleTerminal

THEOREM DurableExactOutputAuthorityMayFenceActiveStateObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishDurableExactOutputSuccessorLifecycleStateSlotV3
  /\ ~AllOldLifecycleTerminal
  =>
    /\ ExactRetainedMergeSidecars
    /\ state.durableJournalValidated
    /\ state'.candidateLifecycleSnapshotV3.serverStreams = "Empty"
    /\ state'.candidateLifecycleSnapshotV3.requestGates = "Empty"
    /\ state'.candidateLifecycleSnapshotV3.serverClosePrefix = 0

THEOREM ValidatedRestartAuthorityMayFenceActiveStateObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishRestartRestoreSuccessorLifecycleStateSlotV3
  /\ ~AllOldLifecycleTerminal
  =>
    /\ state.durableJournalValidated
    /\ state.validatedRestartObserved
    /\ state.restartFenceAuthorized
    /\ state.receiptStage \in {"Absent", "Lost"}

THEOREM ActiveOrdinaryRolloverReturnsCapacityAtomicallyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RejectActiveOrdinaryRollover
  =>
    /\ state'.capacityRejected
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ CandidateLifecycle(state') = CandidateLifecycle(state)

THEOREM SameRosterFullTableReturnsCapacityAtomicallyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RejectSameRosterFullTable
  =>
    /\ state'.capacityRejected
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ CandidateLifecycle(state') = CandidateLifecycle(state)

THEOREM ServiceGenerationOverflowReturnsCapacityAtomicallyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RejectServiceGenerationOverflow
  =>
    /\ state'.capacityRejected
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ CandidateLifecycle(state') = CandidateLifecycle(state)

THEOREM RootGenerationExhaustionPoisonsJournalFailAtomicallyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ FailLifecycleRootGenerationExhaustion
  =>
    /\ state'.restartRequired
    /\ state'.failureReason = "LifecycleRootGenerationOverflow"
    /\ state'.capacityRejected = state.capacityRejected
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ ~state'.durableJournalValidated
    /\ ~state'.successorActive

THEOREM EpochOverflowReturnsCapacityAtomicallyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RejectRequesterEpochOverflow
  =>
    /\ state'.capacityRejected
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ CandidateLifecycle(state') = CandidateLifecycle(state)

THEOREM CrashBeforeRootCommitRestoresPredecessorObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RecoverPredecessorLifecycleV3
  =>
    /\ state'.currentRoster = DurableSnapshot(state).roster
    /\ state'.serviceGeneration =
         DurableSnapshot(state).serviceGeneration
    /\ ~state'.candidatePresent
    /\ state'.lifecycleCommitPhase = "Current"
    /\ state'.validatedRestartObserved
    /\ (state'.targetRoster # state'.currentRoster =>
          state'.restartFenceAuthorized)

THEOREM CrashAfterRootCommitRestoresSuccessorObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RestoreSuccessorLifecycleV3AfterCrash
  =>
    /\ state'.currentRoster = DurableSnapshot(state).roster
    /\ state'.serviceGeneration =
         DurableSnapshot(state).serviceGeneration
    /\ state'.nextStreamEpoch =
         DurableSnapshot(state).nextStreamEpoch
    /\ state'.lifecycleCommitPhase = "Restored"
    /\ state'.transitionAuthority = "RestartRestore"
    /\ ~state'.successorActive

THEOREM FreshEpochPersistencePrecedesExactUseObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishFreshRequesterEpoch
  =>
    /\ state.requesterEpochPhase = "Persisted"
    /\ state.pendingStreamEpoch =
         DurableSnapshot(state).nextStreamEpoch
    /\ DurableSnapshot(state).requesterStreamEpoch =
         DurableSnapshot(state).nextStreamEpoch
    /\ state'.activeStreamEpoch =
         DurableSnapshot(state).requesterStreamEpoch
    /\ state'.nextStreamEpoch = state'.activeStreamEpoch

THEOREM CrashRestoresExactRequesterIncarnationObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RestoreRequesterEpochCounterAfterCrash
  =>
    /\ state'.nextStreamEpoch =
         DurableSnapshot(state).nextStreamEpoch
    /\ state'.activeStreamEpoch =
         DurableSnapshot(state).requesterStreamEpoch
    /\ state'.activeStreamEpoch = state'.nextStreamEpoch
    /\ state'.pendingStreamEpoch = 0
    /\ state'.requesterEpochReplacementRestored

THEOREM SameRosterPreservesTransportWithoutGenerationRollObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ ActivateSameRosterSuccessor
  =>
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ CandidateLifecycle(state') = CandidateLifecycle(state)
    /\ state'.serviceGeneration = state.serviceGeneration
    /\ state'.retryableChunk = 1

THEOREM ForcedFenceCannotForgeAuthenticatedClosePrefixObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ (PublishCommittedLifecycleV3ToMemory
       \/ RestoreSuccessorLifecycleV3AfterCrash)
  /\ state'.transitionAuthority \in
       {"DurableExactOutput", "RestartRestore"}
  =>
    /\ state'.recordedRetiredClosePrefix =
         state.recordedRetiredClosePrefix
    /\ state'.recordedRetiredClosePrefix <=
         state'.authenticatedCloseHistory

THEOREM LateOldCallbackCannotMutateSuccessorObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ ObserveLateOldWriterCallback
  =>
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ CandidateLifecycle(state') = CandidateLifecycle(state)

THEOREM TypedRolloverNextPreservesSafetyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ Next
  =>
    TypedRolloverSafetyInvariant'

THEOREM TypedRolloverSpecAlwaysSafeObligation ==
  TypedRolloverSpec => []TypedRolloverSafetyInvariant

(***************************************************************************
These are separate temporal debts.  The first is the healthy process-local
durable exact-output corridor.  The second begins with no process-local
receipt and requires validated restart, ordered resynchronization, cleanup,
and the distinct RestartRestore authority.
***************************************************************************)
THEOREM ResponsiveDurableExactOutputRolloverLivenessObligation ==
  ResponsiveDurableExactOutputRolloverLiveness

THEOREM ResponsiveRestartRestoreRolloverLivenessObligation ==
  ResponsiveRestartRestoreRolloverLiveness

=============================================================================
