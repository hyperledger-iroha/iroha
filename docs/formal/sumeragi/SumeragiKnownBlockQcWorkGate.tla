---- MODULE SumeragiKnownBlockQcWorkGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `prepare_known_block_qc_work(...)`.

This slice covers the actor-side gate that decides whether a locally known
block-sync Commit QC becomes `KnownBlockQcWork` for async aggregate
verification. It keeps the safety-critical early exits for empty commit
topology, QC/block shape mismatch, same-height locked-QC conflict, stale
locked-QC conflict, and missing locked-payload deferral. It also pins the
liveness-preserving retention of non-extending known-block QCs for lock
realignment and the preservation of work fields with `aggregate_ok = None`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

EmptyTopologyDrops == "empty_topology_drops"
HashMismatchDrops == "hash_mismatch_drops"
HeightMismatchDrops == "height_mismatch_drops"
EpochMismatchDrops == "epoch_mismatch_drops"
PhaseMismatchDrops == "phase_mismatch_drops"
SameHeightConflictDeferred == "same_height_conflict_deferred"
SameHeightConflictDropped == "same_height_conflict_dropped"
SameHeightConflictRecoverableWork == "same_height_conflict_recoverable_work"
StaleAgainstLockDrops == "stale_against_lock_drops"
NonextendingDeferred == "nonextending_deferred"
NonextendingRetainedWork == "nonextending_retained_work"
ExtendingWork == "extending_work"
NoLockWork == "no_lock_work"

Cases == {
  EmptyTopologyDrops,
  HashMismatchDrops,
  HeightMismatchDrops,
  EpochMismatchDrops,
  PhaseMismatchDrops,
  SameHeightConflictDeferred,
  SameHeightConflictDropped,
  SameHeightConflictRecoverableWork,
  StaleAgainstLockDrops,
  NonextendingDeferred,
  NonextendingRetainedWork,
  ExtendingWork,
  NoLockWork
}

EnteredPrep == 1
ShapeCheck == 2
ShapeOk == 3
TopologyEmpty == 4
RosterRecovery == 5
HashMismatch == 6
HeightMismatch == 7
EpochMismatch == 8
PhaseMismatch == 9
DropNone == 10
LockPresent == 11
NoLockedQc == 12
SameHeightConflict == 13
SameHeightRecoverable == 14
MissingLockedPayloadDefer == 15
LockedPrefilterMetric == 16
LogLockedConflict == 17
RecordConsensusDrop == 18
StaleAgainstLock == 19
ExtendsComputed == 20
ExtendsLocked == 21
NonextendingRetained == 22
WorkSome == 23
PreserveQc == 24
PreserveBlock == 25
PreserveTopology == 26
PreserveStakeSnapshot == 27
PreserveConsensusMode == 28
PreserveModeTag == 29
PreservePrfSeed == 30
PreserveCommitQcMatch == 31
AggregateOkNone == 32

Actions == 1..32

WorkActions == {
  WorkSome,
  PreserveQc,
  PreserveBlock,
  PreserveTopology,
  PreserveStakeSnapshot,
  PreserveConsensusMode,
  PreserveModeTag,
  PreservePrfSeed,
  PreserveCommitQcMatch,
  AggregateOkNone
}

SpecActions(c) ==
  CASE c = EmptyTopologyDrops ->
      {EnteredPrep, TopologyEmpty, RosterRecovery, DropNone}
    [] c = HashMismatchDrops ->
      {EnteredPrep, ShapeCheck, HashMismatch, DropNone}
    [] c = HeightMismatchDrops ->
      {EnteredPrep, ShapeCheck, HeightMismatch, DropNone}
    [] c = EpochMismatchDrops ->
      {EnteredPrep, ShapeCheck, EpochMismatch, DropNone}
    [] c = PhaseMismatchDrops ->
      {EnteredPrep, ShapeCheck, PhaseMismatch, DropNone}
    [] c = SameHeightConflictDeferred ->
      {EnteredPrep, ShapeCheck, ShapeOk, LockPresent, SameHeightConflict,
       MissingLockedPayloadDefer, DropNone}
    [] c = SameHeightConflictDropped ->
      {EnteredPrep, ShapeCheck, ShapeOk, LockPresent, SameHeightConflict,
       LockedPrefilterMetric, LogLockedConflict, RecordConsensusDrop,
       DropNone}
    [] c = SameHeightConflictRecoverableWork ->
      {EnteredPrep, ShapeCheck, ShapeOk, LockPresent, SameHeightConflict,
       SameHeightRecoverable, ExtendsComputed, NonextendingRetained}
        \cup WorkActions
    [] c = StaleAgainstLockDrops ->
      {EnteredPrep, ShapeCheck, ShapeOk, LockPresent, StaleAgainstLock,
       RecordConsensusDrop, DropNone}
    [] c = NonextendingDeferred ->
      {EnteredPrep, ShapeCheck, ShapeOk, LockPresent, ExtendsComputed,
       MissingLockedPayloadDefer, DropNone}
    [] c = NonextendingRetainedWork ->
      {EnteredPrep, ShapeCheck, ShapeOk, LockPresent, ExtendsComputed,
       NonextendingRetained} \cup WorkActions
    [] c = ExtendingWork ->
      {EnteredPrep, ShapeCheck, ShapeOk, LockPresent, ExtendsComputed,
       ExtendsLocked} \cup WorkActions
    [] c = NoLockWork ->
      {EnteredPrep, ShapeCheck, ShapeOk, NoLockedQc, ExtendsComputed,
       ExtendsLocked} \cup WorkActions
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "empty_topology_returns_work"
       /\ c = EmptyTopologyDrops ->
      (spec \ {RosterRecovery, DropNone}) \cup WorkActions
    [] Bug = "empty_topology_skips_recovery"
       /\ c = EmptyTopologyDrops ->
      spec \ {RosterRecovery}
    [] Bug = "hash_mismatch_returns_work"
       /\ c = HashMismatchDrops ->
      (spec \ {DropNone}) \cup WorkActions
    [] Bug = "height_mismatch_returns_work"
       /\ c = HeightMismatchDrops ->
      (spec \ {DropNone}) \cup WorkActions
    [] Bug = "epoch_mismatch_returns_work"
       /\ c = EpochMismatchDrops ->
      (spec \ {DropNone}) \cup WorkActions
    [] Bug = "phase_mismatch_returns_work"
       /\ c = PhaseMismatchDrops ->
      (spec \ {DropNone}) \cup WorkActions
    [] Bug = "same_height_defer_returns_work"
       /\ c = SameHeightConflictDeferred ->
      (spec \ {MissingLockedPayloadDefer, DropNone}) \cup WorkActions
    [] Bug = "same_height_defer_skips_deferral"
       /\ c = SameHeightConflictDeferred ->
      spec \ {MissingLockedPayloadDefer}
    [] Bug = "same_height_drop_skips_metric"
       /\ c = SameHeightConflictDropped ->
      spec \ {LockedPrefilterMetric}
    [] Bug = "same_height_drop_skips_consensus"
       /\ c = SameHeightConflictDropped ->
      spec \ {RecordConsensusDrop}
    [] Bug = "same_height_drop_returns_work"
       /\ c = SameHeightConflictDropped ->
      (spec \ {LockedPrefilterMetric, LogLockedConflict, RecordConsensusDrop,
        DropNone}) \cup WorkActions
    [] Bug = "same_height_recoverable_dropped"
       /\ c = SameHeightConflictRecoverableWork ->
      (spec \ WorkActions) \cup {DropNone}
    [] Bug = "stale_returns_work"
       /\ c = StaleAgainstLockDrops ->
      (spec \ {RecordConsensusDrop, DropNone}) \cup WorkActions
    [] Bug = "stale_skips_consensus"
       /\ c = StaleAgainstLockDrops ->
      spec \ {RecordConsensusDrop}
    [] Bug = "nonextending_defer_returns_work"
       /\ c = NonextendingDeferred ->
      (spec \ {MissingLockedPayloadDefer, DropNone}) \cup WorkActions
    [] Bug = "nonextending_defer_skips_deferral"
       /\ c = NonextendingDeferred ->
      spec \ {MissingLockedPayloadDefer}
    [] Bug = "nonextending_retained_dropped"
       /\ c = NonextendingRetainedWork ->
      (spec \ WorkActions) \cup {DropNone}
    [] Bug = "extending_dropped"
       /\ c = ExtendingWork ->
      (spec \ WorkActions) \cup {DropNone}
    [] Bug = "no_lock_dropped"
       /\ c = NoLockWork ->
      (spec \ WorkActions) \cup {DropNone}
    [] Bug = "work_drops_stake_snapshot"
       /\ c = ExtendingWork ->
      spec \ {PreserveStakeSnapshot}
    [] Bug = "work_keeps_aggregate_ok"
       /\ c = ExtendingWork ->
      spec \ {AggregateOkNone}
    [] Bug = "work_drops_commit_qc_match"
       /\ c = NoLockWork ->
      spec \ {PreserveCommitQcMatch}
    [] OTHER -> spec

Bugs == {
  "none",
  "empty_topology_returns_work",
  "empty_topology_skips_recovery",
  "hash_mismatch_returns_work",
  "height_mismatch_returns_work",
  "epoch_mismatch_returns_work",
  "phase_mismatch_returns_work",
  "same_height_defer_returns_work",
  "same_height_defer_skips_deferral",
  "same_height_drop_skips_metric",
  "same_height_drop_skips_consensus",
  "same_height_drop_returns_work",
  "same_height_recoverable_dropped",
  "stale_returns_work",
  "stale_skips_consensus",
  "nonextending_defer_returns_work",
  "nonextending_defer_skips_deferral",
  "nonextending_retained_dropped",
  "extending_dropped",
  "no_lock_dropped",
  "work_drops_stake_snapshot",
  "work_keeps_aggregate_ok",
  "work_drops_commit_qc_match"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ WorkActions \subseteq Actions
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

KnownBlockQcWorkCoreSafety ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

NoBugInvariant == KnownBlockQcWorkCoreSafety

SafetyFast == KnownBlockQcWorkCoreSafety

BugEmptyTopologyReturnsWork == NoBugInvariant
BugEmptyTopologySkipsRecovery == NoBugInvariant
BugHashMismatchReturnsWork == NoBugInvariant
BugHeightMismatchReturnsWork == NoBugInvariant
BugEpochMismatchReturnsWork == NoBugInvariant
BugPhaseMismatchReturnsWork == NoBugInvariant
BugSameHeightDeferReturnsWork == NoBugInvariant
BugSameHeightDeferSkipsDeferral == NoBugInvariant
BugSameHeightDropSkipsMetric == NoBugInvariant
BugSameHeightDropSkipsConsensus == NoBugInvariant
BugSameHeightDropReturnsWork == NoBugInvariant
BugSameHeightRecoverableDropped == NoBugInvariant
BugStaleReturnsWork == NoBugInvariant
BugStaleSkipsConsensus == NoBugInvariant
BugNonextendingDeferReturnsWork == NoBugInvariant
BugNonextendingDeferSkipsDeferral == NoBugInvariant
BugNonextendingRetainedDropped == NoBugInvariant
BugExtendingDropped == NoBugInvariant
BugNoLockDropped == NoBugInvariant
BugWorkDropsStakeSnapshot == NoBugInvariant
BugWorkKeepsAggregateOk == NoBugInvariant
BugWorkDropsCommitQcMatch == NoBugInvariant

=============================================================================
====
