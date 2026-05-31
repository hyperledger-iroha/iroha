---- MODULE SumeragiBlockSyncSelectedQcPrefilterGate ----
EXTENDS Integers

(***************************************************************************
A bounded boolean model for the selected-roster BlockSyncUpdate QC prefilter
inside the `block_sync_apply_qc_after_block(...)` closure.

Once the block payload is ready enough to consider an incoming QC, the live path
checks commit-topology availability, QC shape, same-height locked-QC conflicts,
stale locks, and non-extending locked-chain conflicts before signer tally. This
gate pins which cases return early, which locked-QC drops record status/metrics,
which missing locked-payload conflicts are quarantined, and which QCs reach
`tally_qc_against_block_signers(...)` / `process_precommit_qc(...)`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "empty_topology",
  "hash_mismatch",
  "height_mismatch",
  "epoch_mismatch",
  "phase_mismatch",
  "same_height_conflict_drop",
  "same_height_conflict_recoverable",
  "stale_lock_drop",
  "nonextending_defer",
  "nonextending_drop_with_lock",
  "nonextending_drop_without_lock",
  "nonextending_allowed_retain",
  "extending_continues",
  "no_lock_continues"
}

TopologyEmpty(c) ==
  c = "empty_topology"

HashMatches(c) ==
  c # "hash_mismatch"

HeightMatches(c) ==
  c # "height_mismatch"

EpochMatches(c) ==
  c # "epoch_mismatch"

CommitPhase(c) ==
  c # "phase_mismatch"

ShapeOk(c) ==
  /\ ~TopologyEmpty(c)
  /\ HashMatches(c)
  /\ HeightMatches(c)
  /\ EpochMatches(c)
  /\ CommitPhase(c)

LockPresent(c) ==
  c \in {
    "same_height_conflict_drop",
    "same_height_conflict_recoverable",
    "stale_lock_drop",
    "nonextending_defer",
    "nonextending_drop_with_lock",
    "nonextending_allowed_retain",
    "extending_continues"
  }

SameHeightConflict(c) ==
  c \in {"same_height_conflict_drop", "same_height_conflict_recoverable"}

AllowNonextending(c) ==
  c \in {"same_height_conflict_recoverable", "nonextending_allowed_retain"}

SameHeightRecoverable(c) ==
  SameHeightConflict(c) /\ AllowNonextending(c) /\ CommitPhase(c)

StaleAgainstLock(c) ==
  c = "stale_lock_drop"

ExtendsLocked(c) ==
  c \in {"extending_continues", "no_lock_continues"}

DeferMissingLockedPayload(c) ==
  c = "nonextending_defer"

SpecTopologyRecovery(c) ==
  TopologyEmpty(c)

SpecShapeIgnored(c) ==
  /\ ~TopologyEmpty(c)
  /\ (~HashMatches(c) \/ ~HeightMatches(c) \/ ~EpochMatches(c) \/ ~CommitPhase(c))

SpecSameHeightLockedDrop(c) ==
  ShapeOk(c) /\ SameHeightConflict(c) /\ ~SameHeightRecoverable(c)

SpecLockedPrefilterMetric(c) ==
  SpecSameHeightLockedDrop(c)

SpecLogLockedConflict(c) ==
  SpecSameHeightLockedDrop(c) \/ c = "nonextending_drop_with_lock"

SpecStaleLockedDrop(c) ==
  ShapeOk(c) /\ ~SpecSameHeightLockedDrop(c) /\ StaleAgainstLock(c)

SpecExtendsComputed(c) ==
  ShapeOk(c) /\ ~SpecSameHeightLockedDrop(c) /\ ~SpecStaleLockedDrop(c)

SpecNonextendingDefer(c) ==
  /\ SpecExtendsComputed(c)
  /\ ~ExtendsLocked(c)
  /\ ~AllowNonextending(c)
  /\ DeferMissingLockedPayload(c)

SpecNonextendingLockedDrop(c) ==
  /\ SpecExtendsComputed(c)
  /\ ~ExtendsLocked(c)
  /\ ~AllowNonextending(c)
  /\ ~DeferMissingLockedPayload(c)

SpecQuarantineLockedPayload(c) ==
  SpecNonextendingDefer(c)

SpecRecordLockedDrop(c) ==
  SpecSameHeightLockedDrop(c)
    \/ SpecStaleLockedDrop(c)
    \/ SpecNonextendingDefer(c)
    \/ SpecNonextendingLockedDrop(c)

SpecRetainNonextending(c) ==
  /\ SpecExtendsComputed(c)
  /\ ~ExtendsLocked(c)
  /\ AllowNonextending(c)

SpecTallyAttempted(c) ==
  SpecExtendsComputed(c) /\ (ExtendsLocked(c) \/ AllowNonextending(c))

SpecProcessPrecommitAttempted(c) ==
  SpecTallyAttempted(c)

SpecReturnsOkBeforeTally(c) ==
  SpecTopologyRecovery(c)
    \/ SpecShapeIgnored(c)
    \/ SpecSameHeightLockedDrop(c)
    \/ SpecStaleLockedDrop(c)
    \/ SpecNonextendingDefer(c)
    \/ SpecNonextendingLockedDrop(c)

SpecReturnsOk(c) ==
  SpecReturnsOkBeforeTally(c)

ActualTopologyRecovery(c) ==
  IF Bug = "empty_topology_no_recovery"
     /\ c = "empty_topology"
  THEN FALSE
  ELSE SpecTopologyRecovery(c)

ActualShapeIgnored(c) ==
  IF Bug = "hash_mismatch_tallies"
     /\ c = "hash_mismatch"
  THEN FALSE
  ELSE IF Bug = "height_mismatch_tallies"
          /\ c = "height_mismatch" THEN FALSE
  ELSE IF Bug = "epoch_mismatch_tallies"
          /\ c = "epoch_mismatch" THEN FALSE
  ELSE IF Bug = "phase_mismatch_tallies"
          /\ c = "phase_mismatch" THEN FALSE
  ELSE SpecShapeIgnored(c)

ActualSameHeightLockedDrop(c) ==
  IF Bug = "same_height_conflict_tallies"
     /\ c = "same_height_conflict_drop"
  THEN FALSE
  ELSE IF Bug = "recoverable_same_height_dropped"
          /\ c = "same_height_conflict_recoverable" THEN TRUE
  ELSE SpecSameHeightLockedDrop(c)

ActualLockedPrefilterMetric(c) ==
  IF Bug = "same_height_conflict_no_metric"
     /\ c = "same_height_conflict_drop"
  THEN FALSE
  ELSE SpecLockedPrefilterMetric(c)

ActualLogLockedConflict(c) ==
  SpecLogLockedConflict(c)

ActualStaleLockedDrop(c) ==
  IF Bug = "stale_lock_tallies"
     /\ c = "stale_lock_drop"
  THEN FALSE
  ELSE SpecStaleLockedDrop(c)

ActualExtendsComputed(c) ==
  SpecExtendsComputed(c)

ActualNonextendingDefer(c) ==
  IF Bug = "nonextending_defer_tallies"
     /\ c = "nonextending_defer"
  THEN FALSE
  ELSE SpecNonextendingDefer(c)

ActualNonextendingLockedDrop(c) ==
  IF Bug = "nonextending_drop_tallies"
     /\ c = "nonextending_drop_with_lock"
  THEN FALSE
  ELSE SpecNonextendingLockedDrop(c)

ActualQuarantineLockedPayload(c) ==
  IF Bug = "nonextending_defer_not_quarantined"
     /\ c = "nonextending_defer"
  THEN FALSE
  ELSE SpecQuarantineLockedPayload(c)

ActualRecordLockedDrop(c) ==
  IF Bug = "same_height_conflict_not_recorded"
     /\ c = "same_height_conflict_drop"
  THEN FALSE
  ELSE IF Bug = "stale_lock_no_status"
          /\ c = "stale_lock_drop" THEN FALSE
  ELSE IF Bug = "nonextending_drop_no_status"
          /\ c = "nonextending_drop_with_lock" THEN FALSE
  ELSE IF Bug = "nonextending_defer_not_recorded"
          /\ c = "nonextending_defer" THEN FALSE
  ELSE SpecRecordLockedDrop(c)

ActualRetainNonextending(c) ==
  IF Bug = "nonextending_allowed_dropped"
     /\ c = "nonextending_allowed_retain"
  THEN FALSE
  ELSE SpecRetainNonextending(c)

ActualTallyAttempted(c) ==
  IF Bug = "empty_topology_tallies"
     /\ c = "empty_topology"
  THEN TRUE
  ELSE IF Bug = "hash_mismatch_tallies"
          /\ c = "hash_mismatch" THEN TRUE
  ELSE IF Bug = "height_mismatch_tallies"
          /\ c = "height_mismatch" THEN TRUE
  ELSE IF Bug = "epoch_mismatch_tallies"
          /\ c = "epoch_mismatch" THEN TRUE
  ELSE IF Bug = "phase_mismatch_tallies"
          /\ c = "phase_mismatch" THEN TRUE
  ELSE IF Bug = "same_height_conflict_tallies"
          /\ c = "same_height_conflict_drop" THEN TRUE
  ELSE IF Bug = "stale_lock_tallies"
          /\ c = "stale_lock_drop" THEN TRUE
  ELSE IF Bug = "nonextending_defer_tallies"
          /\ c = "nonextending_defer" THEN TRUE
  ELSE IF Bug = "nonextending_drop_tallies"
          /\ c = "nonextending_drop_with_lock" THEN TRUE
  ELSE IF Bug = "nonextending_allowed_dropped"
          /\ c = "nonextending_allowed_retain" THEN FALSE
  ELSE IF Bug = "extending_dropped"
          /\ c = "extending_continues" THEN FALSE
  ELSE SpecTallyAttempted(c)

ActualProcessPrecommitAttempted(c) ==
  ActualTallyAttempted(c)

ActualReturnsOkBeforeTally(c) ==
  IF Bug = "shape_drop_returns_error"
     /\ c = "hash_mismatch"
  THEN FALSE
  ELSE SpecReturnsOkBeforeTally(c)

ActualReturnsOk(c) ==
  ActualReturnsOkBeforeTally(c)

Matches(c) ==
  /\ ActualTopologyRecovery(c) = SpecTopologyRecovery(c)
  /\ ActualShapeIgnored(c) = SpecShapeIgnored(c)
  /\ ActualSameHeightLockedDrop(c) = SpecSameHeightLockedDrop(c)
  /\ ActualLockedPrefilterMetric(c) = SpecLockedPrefilterMetric(c)
  /\ ActualLogLockedConflict(c) = SpecLogLockedConflict(c)
  /\ ActualStaleLockedDrop(c) = SpecStaleLockedDrop(c)
  /\ ActualExtendsComputed(c) = SpecExtendsComputed(c)
  /\ ActualNonextendingDefer(c) = SpecNonextendingDefer(c)
  /\ ActualNonextendingLockedDrop(c) = SpecNonextendingLockedDrop(c)
  /\ ActualQuarantineLockedPayload(c) = SpecQuarantineLockedPayload(c)
  /\ ActualRecordLockedDrop(c) = SpecRecordLockedDrop(c)
  /\ ActualRetainNonextending(c) = SpecRetainNonextending(c)
  /\ ActualTallyAttempted(c) = SpecTallyAttempted(c)
  /\ ActualProcessPrecommitAttempted(c) = SpecProcessPrecommitAttempted(c)
  /\ ActualReturnsOkBeforeTally(c) = SpecReturnsOkBeforeTally(c)
  /\ ActualReturnsOk(c) = SpecReturnsOk(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "empty_topology_no_recovery",
       "empty_topology_tallies",
       "hash_mismatch_tallies",
       "height_mismatch_tallies",
       "epoch_mismatch_tallies",
       "phase_mismatch_tallies",
       "shape_drop_returns_error",
       "same_height_conflict_no_metric",
       "same_height_conflict_not_recorded",
       "same_height_conflict_tallies",
       "recoverable_same_height_dropped",
       "stale_lock_tallies",
       "stale_lock_no_status",
       "nonextending_defer_not_quarantined",
       "nonextending_defer_not_recorded",
       "nonextending_defer_tallies",
       "nonextending_drop_no_status",
       "nonextending_drop_tallies",
       "nonextending_allowed_dropped",
       "extending_dropped"
     }
  /\ checked = 0

TopologyAndShape ==
  /\ ActualTopologyRecovery("empty_topology") = SpecTopologyRecovery("empty_topology")
  /\ ActualTallyAttempted("empty_topology") = SpecTallyAttempted("empty_topology")
  /\ ActualShapeIgnored("hash_mismatch") = SpecShapeIgnored("hash_mismatch")
  /\ ActualShapeIgnored("height_mismatch") = SpecShapeIgnored("height_mismatch")
  /\ ActualShapeIgnored("epoch_mismatch") = SpecShapeIgnored("epoch_mismatch")
  /\ ActualShapeIgnored("phase_mismatch") = SpecShapeIgnored("phase_mismatch")
  /\ ActualTallyAttempted("hash_mismatch") = SpecTallyAttempted("hash_mismatch")
  /\ ActualTallyAttempted("height_mismatch") = SpecTallyAttempted("height_mismatch")
  /\ ActualTallyAttempted("epoch_mismatch") = SpecTallyAttempted("epoch_mismatch")
  /\ ActualTallyAttempted("phase_mismatch") = SpecTallyAttempted("phase_mismatch")
  /\ ActualReturnsOk("hash_mismatch") = SpecReturnsOk("hash_mismatch")

LockedConflict ==
  /\ ActualSameHeightLockedDrop("same_height_conflict_drop")
       = SpecSameHeightLockedDrop("same_height_conflict_drop")
  /\ ActualLockedPrefilterMetric("same_height_conflict_drop")
       = SpecLockedPrefilterMetric("same_height_conflict_drop")
  /\ ActualRecordLockedDrop("same_height_conflict_drop")
       = SpecRecordLockedDrop("same_height_conflict_drop")
  /\ ActualTallyAttempted("same_height_conflict_drop")
       = SpecTallyAttempted("same_height_conflict_drop")
  /\ ActualSameHeightLockedDrop("same_height_conflict_recoverable")
       = SpecSameHeightLockedDrop("same_height_conflict_recoverable")
  /\ ActualTallyAttempted("same_height_conflict_recoverable")
       = SpecTallyAttempted("same_height_conflict_recoverable")
  /\ ActualStaleLockedDrop("stale_lock_drop") = SpecStaleLockedDrop("stale_lock_drop")
  /\ ActualRecordLockedDrop("stale_lock_drop") = SpecRecordLockedDrop("stale_lock_drop")
  /\ ActualTallyAttempted("stale_lock_drop") = SpecTallyAttempted("stale_lock_drop")

NonextendingLock ==
  /\ ActualNonextendingDefer("nonextending_defer") = SpecNonextendingDefer("nonextending_defer")
  /\ ActualQuarantineLockedPayload("nonextending_defer")
       = SpecQuarantineLockedPayload("nonextending_defer")
  /\ ActualRecordLockedDrop("nonextending_defer") = SpecRecordLockedDrop("nonextending_defer")
  /\ ActualTallyAttempted("nonextending_defer") = SpecTallyAttempted("nonextending_defer")
  /\ ActualNonextendingLockedDrop("nonextending_drop_with_lock")
       = SpecNonextendingLockedDrop("nonextending_drop_with_lock")
  /\ ActualRecordLockedDrop("nonextending_drop_with_lock")
       = SpecRecordLockedDrop("nonextending_drop_with_lock")
  /\ ActualTallyAttempted("nonextending_drop_with_lock")
       = SpecTallyAttempted("nonextending_drop_with_lock")
  /\ ActualNonextendingLockedDrop("nonextending_drop_without_lock")
       = SpecNonextendingLockedDrop("nonextending_drop_without_lock")
  /\ ActualRetainNonextending("nonextending_allowed_retain")
       = SpecRetainNonextending("nonextending_allowed_retain")
  /\ ActualTallyAttempted("nonextending_allowed_retain")
       = SpecTallyAttempted("nonextending_allowed_retain")

Continuation ==
  /\ ActualTallyAttempted("extending_continues") = SpecTallyAttempted("extending_continues")
  /\ ActualProcessPrecommitAttempted("extending_continues")
       = SpecProcessPrecommitAttempted("extending_continues")
  /\ ActualTallyAttempted("no_lock_continues") = SpecTallyAttempted("no_lock_continues")
  /\ ActualProcessPrecommitAttempted("no_lock_continues")
       = SpecProcessPrecommitAttempted("no_lock_continues")

SafetyFast ==
  /\ TopologyAndShape
  /\ LockedConflict
  /\ NonextendingLock
  /\ Continuation

=============================================================================
