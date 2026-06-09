---- MODULE SumeragiLockRejectedSinkGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for lock-rejected branch sink lifecycle.

This slice captures the deterministic sink around lock-rejected branch hashes:
`note_lock_rejected_block(...)`, `active_lock_rejected_block_sink(...)`,
`should_suppress_lock_rejected_block_fetch(...)`,
`clear_lock_rejected_block_sinks_for_height(...)`, and
`purge_lock_rejected_block_artifacts(...)`. It abstracts hashes, clocks, and
internal maps to finite cases while preserving the helper contract: the locked
hash itself is never inserted, same-lock rejections extend an existing sink,
lock-anchor changes replace it, activity requires TTL/max-dwell freshness,
uncommitted height, and the same locked QC anchor, local payload availability
does not rehabilitate the rejected hash, active sinks suppress fetch and parent
recovery without creating dependencies, replayed BlockCreated messages fast
drop without re-running rejection accounting, and cleanup removes the rejected
branch from consensus, proposal, recovery, validation, and RBC surfaces.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoteSameHashNoInsert == "note_same_hash_no_insert"
NoteVacantInsert == "note_vacant_insert"
NoteExistingSameLock == "note_existing_same_lock"
NoteExistingChangedLock == "note_existing_changed_lock"
ActiveFresh == "active_fresh"
ActiveExpiredTtl == "active_expired_ttl"
ActiveExpiredDwell == "active_expired_dwell"
ActiveCommittedPast == "active_committed_past"
ActiveNoLock == "active_no_lock"
ActiveLockMismatch == "active_lock_mismatch"
ActiveWithLocalPayload == "active_with_local_payload"
SuppressFetchActive == "suppress_fetch_active"
SuppressFetchInactive == "suppress_fetch_inactive"
BlockCreatedReplayFastDrop == "block_created_replay_fast_drop"
MissingParentSuppressed == "missing_parent_suppressed"
HighestDeferMarkerSuppressed == "highest_defer_marker_suppressed"
ClearHeight == "clear_height"
PurgeConsensusArtifacts == "purge_consensus_artifacts"
PurgeProposalArtifacts == "purge_proposal_artifacts"
PurgeRecoveryArtifacts == "purge_recovery_artifacts"

Cases == {
  NoteSameHashNoInsert,
  NoteVacantInsert,
  NoteExistingSameLock,
  NoteExistingChangedLock,
  ActiveFresh,
  ActiveExpiredTtl,
  ActiveExpiredDwell,
  ActiveCommittedPast,
  ActiveNoLock,
  ActiveLockMismatch,
  ActiveWithLocalPayload,
  SuppressFetchActive,
  SuppressFetchInactive,
  BlockCreatedReplayFastDrop,
  MissingParentSuppressed,
  HighestDeferMarkerSuppressed,
  ClearHeight,
  PurgeConsensusArtifacts,
  PurgeProposalArtifacts,
  PurgeRecoveryArtifacts
}

NoInsert == 1
InsertSink == 2
ReplaceSink == 3
RejectionsOne == 4
IncrementRejections == 5
FetchSuppressionsZero == 6
PreserveFirstSeen == 7
PreserveFetchSuppressions == 8
UpdateLastSeen == 9
ActiveSink == 10
InactiveSink == 11
PruneSink == 12
IgnoreLocalPayload == 13
SuppressFetch == 14
AllowFetch == 15
IncrementFetchSuppressions == 16
DropBlockCreated == 17
RejectionsStable == 18
NoPendingBlock == 19
SuppressMissingParent == 20
NoMissingRequest == 21
NoRangePull == 22
SuppressDeferMarker == 23
MarkerPruned == 24
RemoveMatchingHeight == 25
PreserveOtherHeights == 26
PurgeVotes == 27
PurgeQcs == 28
PurgeDeferredQcState == 29
PurgeKnownQcWork == 30
PurgePending == 31
PurgeHints == 32
PurgeProposals == 33
PurgeSlotOwner == 34
ClearProposalLiveness == 35
PurgeFrontierSlot == 36
PurgeVoteRoster == 37
PurgeBlockSigner == 38
PurgeFetchQueues == 39
ClearRecoveryWindows == 40
ClearViewChange == 41
PurgeValidationWhenPendingRemoved == 42
PurgeRbc == 43

ActionUniverse == 1..43

SpecActions(c) ==
  CASE c = NoteSameHashNoInsert ->
      {NoInsert}
    [] c = NoteVacantInsert ->
      {InsertSink, RejectionsOne, FetchSuppressionsZero, UpdateLastSeen}
    [] c = NoteExistingSameLock ->
      {IncrementRejections, PreserveFirstSeen, PreserveFetchSuppressions,
       UpdateLastSeen}
    [] c = NoteExistingChangedLock ->
      {ReplaceSink, RejectionsOne, FetchSuppressionsZero, UpdateLastSeen}
    [] c = ActiveFresh ->
      {ActiveSink, UpdateLastSeen}
    [] c \in {ActiveExpiredTtl, ActiveExpiredDwell, ActiveCommittedPast,
              ActiveNoLock, ActiveLockMismatch} ->
      {InactiveSink, PruneSink}
    [] c = ActiveWithLocalPayload ->
      {ActiveSink, IgnoreLocalPayload, UpdateLastSeen}
    [] c = SuppressFetchActive ->
      {SuppressFetch, IncrementFetchSuppressions}
    [] c = SuppressFetchInactive ->
      {AllowFetch}
    [] c = BlockCreatedReplayFastDrop ->
      {DropBlockCreated, RejectionsStable, NoPendingBlock, PurgeVotes,
       PurgeQcs, PurgePending}
    [] c = MissingParentSuppressed ->
      {SuppressMissingParent, NoMissingRequest, NoRangePull}
    [] c = HighestDeferMarkerSuppressed ->
      {SuppressDeferMarker, MarkerPruned}
    [] c = ClearHeight ->
      {RemoveMatchingHeight, PreserveOtherHeights}
    [] c = PurgeConsensusArtifacts ->
      {PurgeVotes, PurgeQcs, PurgeDeferredQcState, PurgeKnownQcWork,
       MarkerPruned}
    [] c = PurgeProposalArtifacts ->
      {PurgePending, PurgeHints, PurgeProposals, PurgeSlotOwner,
       ClearProposalLiveness, PurgeFrontierSlot}
    [] c = PurgeRecoveryArtifacts ->
      {PurgeVoteRoster, PurgeBlockSigner, PurgeFetchQueues,
       ClearRecoveryWindows, ClearViewChange, PurgeValidationWhenPendingRemoved,
       PurgeRbc}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "note_same_hash_inserts"
       /\ c = NoteSameHashNoInsert ->
      {InsertSink, RejectionsOne, FetchSuppressionsZero}
    [] Bug = "note_vacant_rejections_not_one"
       /\ c = NoteVacantInsert ->
      spec \ {RejectionsOne}
    [] Bug = "note_vacant_fetch_not_zero"
       /\ c = NoteVacantInsert ->
      spec \ {FetchSuppressionsZero}
    [] Bug = "note_existing_same_lock_resets_first_seen"
       /\ c = NoteExistingSameLock ->
      (spec \ {PreserveFirstSeen}) \cup {ReplaceSink}
    [] Bug = "note_existing_same_lock_resets_fetch"
       /\ c = NoteExistingSameLock ->
      (spec \ {PreserveFetchSuppressions}) \cup {FetchSuppressionsZero}
    [] Bug = "note_existing_same_lock_skips_rejection_increment"
       /\ c = NoteExistingSameLock ->
      spec \ {IncrementRejections}
    [] Bug = "note_changed_lock_preserves_old"
       /\ c = NoteExistingChangedLock ->
      (spec \ {ReplaceSink, RejectionsOne, FetchSuppressionsZero}) \cup
        {PreserveFirstSeen, PreserveFetchSuppressions}
    [] Bug = "active_ignores_ttl"
       /\ c = ActiveExpiredTtl ->
      {ActiveSink, UpdateLastSeen}
    [] Bug = "active_ignores_max_dwell"
       /\ c = ActiveExpiredDwell ->
      {ActiveSink, UpdateLastSeen}
    [] Bug = "active_ignores_committed_height"
       /\ c = ActiveCommittedPast ->
      {ActiveSink, UpdateLastSeen}
    [] Bug = "active_without_lock"
       /\ c = ActiveNoLock ->
      {ActiveSink, UpdateLastSeen}
    [] Bug = "active_ignores_lock_mismatch"
       /\ c = ActiveLockMismatch ->
      {ActiveSink, UpdateLastSeen}
    [] Bug = "active_payload_deactivates"
       /\ c = ActiveWithLocalPayload ->
      {InactiveSink, PruneSink}
    [] Bug = "suppress_fetch_inactive"
       /\ c = SuppressFetchInactive ->
      {SuppressFetch, IncrementFetchSuppressions}
    [] Bug = "suppress_fetch_skips_counter"
       /\ c = SuppressFetchActive ->
      spec \ {IncrementFetchSuppressions}
    [] Bug = "block_created_replay_rejects_again"
       /\ c = BlockCreatedReplayFastDrop ->
      (spec \ {RejectionsStable}) \cup {IncrementRejections}
    [] Bug = "block_created_replay_keeps_pending"
       /\ c = BlockCreatedReplayFastDrop ->
      spec \ {NoPendingBlock}
    [] Bug = "missing_parent_reintroduced"
       /\ c = MissingParentSuppressed ->
      spec \ {NoMissingRequest}
    [] Bug = "missing_parent_range_pull"
       /\ c = MissingParentSuppressed ->
      spec \ {NoRangePull}
    [] Bug = "highest_defer_marker_reintroduced"
       /\ c = HighestDeferMarkerSuppressed ->
      spec \ {MarkerPruned}
    [] Bug = "clear_height_removes_other_heights"
       /\ c = ClearHeight ->
      spec \ {PreserveOtherHeights}
    [] Bug = "purge_skips_consensus_artifacts"
       /\ c = PurgeConsensusArtifacts ->
      spec \ {PurgeVotes, PurgeQcs, PurgeDeferredQcState,
              PurgeKnownQcWork, MarkerPruned}
    [] Bug = "purge_skips_proposal_artifacts"
       /\ c = PurgeProposalArtifacts ->
      spec \ {PurgePending, PurgeHints, PurgeProposals, PurgeSlotOwner,
              ClearProposalLiveness, PurgeFrontierSlot}
    [] Bug = "purge_skips_recovery_artifacts"
       /\ c = PurgeRecoveryArtifacts ->
      spec \ {PurgeVoteRoster, PurgeBlockSigner, PurgeFetchQueues,
              ClearRecoveryWindows, ClearViewChange, PurgeRbc}
    [] Bug = "purge_validation_without_pending"
       /\ c = PurgeRecoveryArtifacts ->
      spec \ {PurgeValidationWhenPendingRemoved}
    [] OTHER -> spec

Bugs == {
  "none",
  "note_same_hash_inserts",
  "note_vacant_rejections_not_one",
  "note_vacant_fetch_not_zero",
  "note_existing_same_lock_resets_first_seen",
  "note_existing_same_lock_resets_fetch",
  "note_existing_same_lock_skips_rejection_increment",
  "note_changed_lock_preserves_old",
  "active_ignores_ttl",
  "active_ignores_max_dwell",
  "active_ignores_committed_height",
  "active_without_lock",
  "active_ignores_lock_mismatch",
  "active_payload_deactivates",
  "suppress_fetch_inactive",
  "suppress_fetch_skips_counter",
  "block_created_replay_rejects_again",
  "block_created_replay_keeps_pending",
  "missing_parent_reintroduced",
  "missing_parent_range_pull",
  "highest_defer_marker_reintroduced",
  "clear_height_removes_other_heights",
  "purge_skips_consensus_artifacts",
  "purge_skips_proposal_artifacts",
  "purge_skips_recovery_artifacts",
  "purge_validation_without_pending"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

ActionsMatchSpec ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

ActivityGatesFailClosed ==
  \A c \in {ActiveExpiredTtl, ActiveExpiredDwell, ActiveCommittedPast,
            ActiveNoLock, ActiveLockMismatch}:
    /\ InactiveSink \in ImplementationActions(c)
    /\ PruneSink \in ImplementationActions(c)

SuppressionDoesNotCreateDependencies ==
  /\ NoMissingRequest \in ImplementationActions(MissingParentSuppressed)
  /\ NoRangePull \in ImplementationActions(MissingParentSuppressed)
  /\ MarkerPruned \in ImplementationActions(HighestDeferMarkerSuppressed)
  /\ NoPendingBlock \in ImplementationActions(BlockCreatedReplayFastDrop)

PurgeCoversAllSurfaces ==
  /\ {PurgeVotes, PurgeQcs, PurgeDeferredQcState, PurgeKnownQcWork,
      MarkerPruned} \subseteq ImplementationActions(PurgeConsensusArtifacts)
  /\ {PurgePending, PurgeHints, PurgeProposals, PurgeSlotOwner,
      ClearProposalLiveness, PurgeFrontierSlot}
        \subseteq ImplementationActions(PurgeProposalArtifacts)
  /\ {PurgeVoteRoster, PurgeBlockSigner, PurgeFetchQueues,
      ClearRecoveryWindows, ClearViewChange, PurgeValidationWhenPendingRemoved,
      PurgeRbc} \subseteq ImplementationActions(PurgeRecoveryArtifacts)

LockRejectedSinkCoreSafety ==
  /\ ActionsMatchSpec
  /\ ActivityGatesFailClosed
  /\ SuppressionDoesNotCreateDependencies
  /\ PurgeCoversAllSurfaces

NoBugInvariant == LockRejectedSinkCoreSafety

SafetyFast == LockRejectedSinkCoreSafety

====
