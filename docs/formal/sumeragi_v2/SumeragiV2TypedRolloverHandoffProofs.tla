---- MODULE SumeragiV2TypedRolloverHandoffProofs ----
EXTENDS SumeragiV2TypedRolloverHandoff, TLAPS

(***************************************************************************
Proof obligations for the sole-V2 typed rollover model.

This module deliberately contains declarations, not proof commands.  The old
proof script described a separate generation high-water followed by a second
lifecycle write and allowed typed handoff authority to clear active responder
ownership.  Neither behavior exists in the first-release design.

The replacement obligations bind:

* terminal server-stream, request-gate, transfer, and flush ownership before
  any generation advance;
* one atomic LifecycleSnapshotV2 write containing the checked successor
  generation and both empty responder tables;
* memory publication only from that durable V2 postimage;
* fail-atomic Capacity on active-state and generation/epoch overflow;
* durable-before-use requester epoch allocation and crash-time non-reuse;
* exact move-only handoff identity and late-callback isolation.

All declarations below remain `specified_unproved`.  In particular this module
does not prove changed-roster progress, crash recovery liveness, network/writer
progress, rotating-leader progress, repeated rollover, or Rust-to-TLA
refinement.  Bounded TLC mutation results do not promote these obligations.
***************************************************************************)

THEOREM TypedRolloverInitEstablishesSafetyObligation ==
  Init => TypedRolloverSafetyInvariant

THEOREM TerminalCompactionPersistsAtomicSoleV2SnapshotObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PersistSuccessorLifecycleSnapshotV2
  =>
    /\ AllOldLifecycleTerminal
    /\ state'.durableLifecycleSnapshotV2 =
         LifecycleSnapshotV2(
           state.serviceGeneration + 1,
           state.nextStreamEpoch,
           "Empty",
           "Empty")
    /\ LifecycleMemory(state') = LifecycleMemory(state)

THEOREM ActiveLifecycleCompactionReturnsCapacityAtomicallyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RejectActiveLifecycleCompaction
  =>
    /\ state'.capacityRejected
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)

THEOREM GenerationOverflowReturnsCapacityAtomicallyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RejectGenerationOverflow
  =>
    /\ state'.capacityRejected
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)

THEOREM EpochOverflowReturnsCapacityAtomicallyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RejectRequesterEpochOverflow
  =>
    /\ state'.capacityRejected
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)

THEOREM SnapshotPublicationRequiresDurableV2PreimageObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishPersistedLifecycleSnapshotV2
  =>
    /\ state.durableLifecycleSnapshotV2.version = 2
    /\ state.durableLifecycleSnapshotV2.serverStreams = "Empty"
    /\ state.durableLifecycleSnapshotV2.requestGates = "Empty"
    /\ state'.serviceGeneration =
         state.durableLifecycleSnapshotV2.generation
    /\ state'.lifecycleSnapshotPhase = "Published"

THEOREM CrashBeforeSnapshotPersistencePreservesPredecessorObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ CrashBeforeLifecycleSnapshotV2Persistence
  =>
    /\ state'.restartRequired
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)

THEOREM CrashAfterSnapshotPersistenceRestoresSuccessorObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RestoreSuccessorLifecycleSnapshotV2AfterCrash
  =>
    /\ LifecycleMemoryMatchesDurableSnapshotV2'
    /\ state'.lifecycleSnapshotPhase = "Restored"
    /\ ~state'.successorActive

THEOREM FreshEpochPersistencePrecedesUseObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ PublishFreshRequesterEpoch
  =>
    /\ state.requesterEpochPhase = "Persisted"
    /\ state.durableLifecycleSnapshotV2.nextStreamEpoch >
         state'.activeStreamEpoch
    /\ state'.activeStreamEpoch # state.skippedStreamEpoch

THEOREM CrashAfterEpochPersistenceSkipsEpochObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ RestoreRequesterEpochCounterAfterCrash
  =>
    /\ state'.nextStreamEpoch =
         state.durableLifecycleSnapshotV2.nextStreamEpoch
    /\ state'.activeStreamEpoch = 0
    /\ state'.pendingStreamEpoch = 0
    /\ state'.skippedStreamEpoch # 0

THEOREM SameRosterPreservesTransportWithoutCompactionObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ ActivateSameRosterSuccessor
  =>
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)
    /\ state'.retryableChunk = 1

THEOREM LateOldCallbackCannotMutateSuccessorObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ ObserveLateOldWriterCallback
  =>
    /\ LifecycleMemory(state') = LifecycleMemory(state)
    /\ DurableLifecycle(state') = DurableLifecycle(state)

THEOREM TypedRolloverNextPreservesSafetyObligation ==
  /\ TypedRolloverSafetyInvariant
  /\ Next
  =>
    TypedRolloverSafetyInvariant'

THEOREM TypedRolloverSpecAlwaysSafeObligation ==
  TypedRolloverSpec => []TypedRolloverSafetyInvariant

(***************************************************************************
The temporal target is repeated here without a proof.  It stays explicit debt
and cannot be used as a dependency for rotating-leader or application progress.
***************************************************************************)
THEOREM ResponsiveChangedRosterRolloverLivenessObligation ==
  ResponsiveChangedRosterRolloverLiveness

=============================================================================
