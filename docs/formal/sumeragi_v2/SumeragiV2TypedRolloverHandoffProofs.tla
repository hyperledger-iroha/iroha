---- MODULE SumeragiV2TypedRolloverHandoffProofs ----
EXTENDS SumeragiV2TypedRolloverHandoff, TLAPS

(***************************************************************************
Proof obligations for the authority-gated, root-anchored V3 typed-rollover
model.

The obligations distinguish ordinary terminal retirement from the two
authorities that may supersede active responder ownership:

* a move-only durable exact-output handoff; and
* restart recovery after validation of the root-selected V3 snapshot.

They also bind the crash corridor in its actual order: inactive state slot,
exact root commit, then memory publication.  Same-roster handoff never rolls a
generation, and active fencing never manufactures a requester-authenticated
Close prefix.  The generation-zero bootstrap root has no selected state; its
first committed root selects the parity slot containing generation one.
Restart cleanup remains disabled until the exact root-selected pair passes
shape, generation/digest, presence, and transport-semantic validation.

All declarations below remain `specified_unproved`.  In particular, this
module does not prove unbounded changed-roster progress, storage or filesystem
semantics, network/writer progress, repeated rollover, or Rust-to-TLA
refinement.  Bounded TLC mutation results cannot promote these obligations.
***************************************************************************)

THEOREM TypedRolloverInitEstablishesSafetyObligation ==
  Init => TypedRolloverSafetyInvariant

THEOREM BootstrapRootHasExactGenerationZeroShapeObligation ==
  /\ Init
  /\ state.durableLifecycleRootV3.shape = "Bootstrap"
  =>
    /\ state.durableLifecycleRootV3 = BootstrapLifecycleRootV3
    /\ state.durableLifecycleRootV3.rootGeneration = 0
    /\ state.durableLifecycleRootV3.snapshotDigest =
         NoLifecycleSnapshot
    /\ \A slot \in StateSlots:
         state.durableLifecycleStateSlotsV3[slot] =
           NoLifecycleSnapshot
    /\ state.lifecycleCommitPhase = "Bootstrap"

THEOREM BootstrapFirstCommitSelectsExactInitialPairObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ CommitInitialLifecycleRootV3
  =>
    /\ state'.durableLifecycleRootV3 =
         LifecycleRootV3(InitialLifecycleSnapshotV3)
    /\ state'.durableLifecycleRootV3.rootGeneration =
         InitialRootGeneration
    /\ SelectedLifecycleStateSlot(state') =
         LifecycleStateSlot(InitialRootGeneration)
    /\ SelectedLifecycleSnapshotV3(state') =
         InitialLifecycleSnapshotV3
    /\ RootSelectedLifecyclePairMatches(state')
    /\ LifecycleMemory(state') = LifecycleMemory(state)

THEOREM SuccessorStateSlotPrecedesRootCommitObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishSuccessorLifecycleStateSlotV3
  =>
    /\ state'.candidatePresent
    /\ state'.candidateSemanticallyValidated
    /\ state'.candidateStateSlot =
         LifecycleStateSlot(
           state'.candidateLifecycleSnapshotV3.rootGeneration)
    /\ state'.candidateStateSlot #
         SelectedLifecycleStateSlot(state)
    /\ state'.candidateStateSlot =
         1 - SelectedLifecycleStateSlot(state)
    /\ state'.candidateLifecycleSnapshotV3.rootGeneration =
         state.durableLifecycleRootV3.rootGeneration + 1
    /\ state'.candidateLifecycleSnapshotV3.serviceGeneration =
         state.serviceGeneration + 1
    /\ state'.candidateLifecycleSnapshotV3.serverStreams = "Empty"
    /\ state'.candidateLifecycleSnapshotV3.requestGates = "Empty"
    /\ state'.candidateLifecycleSnapshotV3.serverClosePrefix = 0
    /\ state'.durableLifecycleRootV3 =
         state.durableLifecycleRootV3
    /\ state'.durableLifecycleStateSlotsV3[
         state'.candidateStateSlot] =
         state'.candidateLifecycleSnapshotV3
    /\ \A slot \in StateSlots \ {state'.candidateStateSlot}:
         state'.durableLifecycleStateSlotsV3[slot] =
           state.durableLifecycleStateSlotsV3[slot]
    /\ SelectedLifecycleSnapshotV3(state') =
         SelectedLifecycleSnapshotV3(state)
    /\ LifecycleMemory(state') = LifecycleMemory(state)

THEOREM RootCommitSelectsExactSuccessorSlotObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ CommitSuccessorLifecycleRootV3
  =>
    /\ state'.durableLifecycleRootV3 =
         LifecycleRootV3(state.candidateLifecycleSnapshotV3)
    /\ SelectedLifecycleStateSlot(state') =
         state.candidateStateSlot
    /\ state'.durableLifecycleStateSlotsV3 =
         state.durableLifecycleStateSlotsV3
    /\ RootSelectedLifecyclePairMatches(state')
    /\ LifecycleSnapshotSemanticallyValid(
         SelectedLifecycleSnapshotV3(state'))
    /\ state'.lifecycleCommitPhase = "RootCommitted"
    /\ LifecycleMemory(state') = LifecycleMemory(state)

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

THEOREM SemanticValidationPrecedesArtifactCleanupObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ CleanupValidatedLifecycleArtifactsV3
  =>
    /\ state.durableJournalValidated
    /\ RootSelectedLifecyclePairMatches(state)
    /\ LifecycleSnapshotSemanticallyValid(DurableSnapshot(state))
    /\ state'.cleanupPerformed
    /\ ~state'.crashArtifactsPresent

THEOREM RootGenerationAdvancesExactlyOnceAndAlternatesSlotObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ Next
  /\ state'.durableLifecycleRootV3.rootGeneration >
       state.durableLifecycleRootV3.rootGeneration
  =>
    /\ state'.durableLifecycleRootV3.rootGeneration =
         state.durableLifecycleRootV3.rootGeneration + 1
    /\ SelectedLifecycleStateSlot(state') =
         LifecycleStateSlot(
           state'.durableLifecycleRootV3.rootGeneration)
    /\ SelectedLifecycleStateSlot(state') #
         SelectedLifecycleStateSlot(state)
    /\ SelectedLifecycleStateSlot(state') =
         1 - SelectedLifecycleStateSlot(state)
    /\ state'.durableLifecycleRootV3.snapshotDigest =
         LifecycleSnapshotDigest(
           SelectedLifecycleSnapshotV3(state'))

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
  /\ PublishSuccessorLifecycleStateSlotV3
  /\ state'.pendingRolloverAuthority = "DurableExactOutput"
  /\ ~AllOldLifecycleTerminal
  =>
    /\ ExactRetainedMergeSidecars
    /\ state.durableJournalValidated
    /\ state'.candidateLifecycleSnapshotV3.serverStreams = "Empty"
    /\ state'.candidateLifecycleSnapshotV3.requestGates = "Empty"
    /\ state'.candidateLifecycleSnapshotV3.serverClosePrefix = 0

THEOREM ValidatedRestartAuthorityMayFenceActiveStateObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishSuccessorLifecycleStateSlotV3
  /\ state'.pendingRolloverAuthority = "RestartRestore"
  /\ ~AllOldLifecycleTerminal
  =>
    /\ state.durableJournalValidated
    /\ state.validatedRestartObserved
    /\ state.restartFenceAuthorized

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

THEOREM RootGenerationOverflowReturnsCapacityAtomicallyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RejectLifecycleRootGenerationOverflow
  =>
    /\ state'.capacityRejected
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ CandidateLifecycle(state') = CandidateLifecycle(state)

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
    /\ LifecycleMemoryMatchesDurableSnapshotV3'
    /\ ~state'.candidatePresent
    /\ state'.lifecycleCommitPhase = "Current"
    /\ state'.validatedRestartObserved
    /\ state'.restartFenceAuthorized

THEOREM CrashAfterRootCommitRestoresSuccessorObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RestoreSuccessorLifecycleV3AfterCrash
  =>
    /\ LifecycleMemoryMatchesDurableSnapshotV3'
    /\ state'.lifecycleCommitPhase = "Restored"
    /\ state'.transitionAuthority = "RestartRestore"
    /\ ~state'.successorActive

THEOREM FreshEpochPersistencePrecedesUseObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishFreshRequesterEpoch
  =>
    /\ state.requesterEpochPhase = "Persisted"
    /\ DurableSnapshot(state).nextStreamEpoch >
         state'.activeStreamEpoch
    /\ state'.activeStreamEpoch # state.skippedStreamEpoch

THEOREM CrashAfterEpochPersistenceSkipsEpochObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RestoreRequesterEpochCounterAfterCrash
  =>
    /\ state'.nextStreamEpoch =
         DurableSnapshot(state).nextStreamEpoch
    /\ state'.activeStreamEpoch = 0
    /\ state'.pendingStreamEpoch = 0
    /\ state'.skippedStreamEpoch # 0

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
The temporal target is repeated without proof over ResponsiveNoFailureNext,
the explicit durable-exact-output, no-crash corridor.  It does not claim
ordinary-terminalization or crash-recovery liveness, remains explicit debt,
and cannot be used as a dependency for rotating-leader or application
progress.
***************************************************************************)
THEOREM ResponsiveChangedRosterRolloverLivenessObligation ==
  ResponsiveChangedRosterRolloverLiveness

=============================================================================
