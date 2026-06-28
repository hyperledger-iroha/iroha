---- MODULE SumeragiPendingRbcStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for pending-RBC status snapshot accounting.

This slice complements `SumeragiPendingRbcStashGate`: it does not re-model
stash admission/replay. Instead it pins the status-facing projection in
`status.rs` and `update_rbc_backlog_snapshot(...)`: reset behavior, drop/stash
counter overlays in `pending_rbc_snapshot()`, reason bucket separation,
eviction accounting, and per-entry pending-RBC snapshot fields.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ResetClearsSnapshot == 1
ResetClearsDropCounters == 2
ResetClearsStashCounters == 3
SetSnapshotStoresStructure == 4
SnapshotOverlaysDropCounters == 5
SnapshotOverlaysStashCounters == 6
SnapshotPreservesEntries == 7
DropCapFramesFloored == 8
DropSessionCapCountsAsCap == 9
DropTtlSeparateBucket == 10
DropUnknownOnlyTotals == 11
DropBytesAccumulate == 12
DropRepeatedAccumulates == 13
EvictedAccumulates == 14
StashChunkOnly == 15
StashReadyNoReason == 16
StashReadyReasons == 17
StashDeliverNoReason == 18
StashDeliverReasons == 19
DifferentReasonsDistinct == 20
UpdateSnapshotAggregatesPending == 21
UpdateSnapshotRecordsCapsTtl == 22
UpdateSnapshotProjectsEntryKey == 23
UpdateSnapshotProjectsEntryBufferedCounts == 24
UpdateSnapshotProjectsEntryDropBreakdown == 25
UpdateSnapshotProjectsEntryAge == 26
UpdateSnapshotKeepsAtomicCounters == 27

Candidates == 1..27

ResetSnapshot == 1
ResetDrops == 2
ResetStash == 3
StoreStructure == 4
OverlayDropCounters == 5
OverlayStashCounters == 6
PreserveEntries == 7
FloorDropFrames == 8
CapBucket == 9
TtlBucket == 10
UnknownNoReasonBucket == 11
DropBytesAccumulated == 12
RepeatedDropsAccumulated == 13
EvictedCounterAccumulated == 14
StashChunkCounter == 15
StashReadyTotal == 16
StashReadyReasonCounter == 17
StashDeliverTotal == 18
StashDeliverReasonCounter == 19
ReasonBucketsDistinct == 20
AggregatePendingSessions == 21
AggregatePendingChunksBytes == 22
StoreCapsTtl == 23
EntryKeyProjected == 24
EntryBufferedCountsProjected == 25
EntryDropBreakdownProjected == 26
EntryAgeProjected == 27
AtomicCountersPreservedOnUpdate == 28

Actions == 1..28

SpecActions(candidate) ==
  CASE candidate = ResetClearsSnapshot ->
      {ResetSnapshot, ResetDrops, ResetStash}
    [] candidate = ResetClearsDropCounters ->
      {ResetDrops}
    [] candidate = ResetClearsStashCounters ->
      {ResetStash}
    [] candidate = SetSnapshotStoresStructure ->
      {StoreStructure}
    [] candidate = SnapshotOverlaysDropCounters ->
      {OverlayDropCounters, StoreStructure}
    [] candidate = SnapshotOverlaysStashCounters ->
      {OverlayStashCounters, StoreStructure}
    [] candidate = SnapshotPreservesEntries ->
      {PreserveEntries, StoreStructure}
    [] candidate = DropCapFramesFloored ->
      {FloorDropFrames, CapBucket, DropBytesAccumulated}
    [] candidate = DropSessionCapCountsAsCap ->
      {CapBucket, DropBytesAccumulated}
    [] candidate = DropTtlSeparateBucket ->
      {TtlBucket, DropBytesAccumulated}
    [] candidate = DropUnknownOnlyTotals ->
      {UnknownNoReasonBucket, DropBytesAccumulated}
    [] candidate = DropBytesAccumulate ->
      {DropBytesAccumulated}
    [] candidate = DropRepeatedAccumulates ->
      {RepeatedDropsAccumulated, DropBytesAccumulated}
    [] candidate = EvictedAccumulates ->
      {EvictedCounterAccumulated}
    [] candidate = StashChunkOnly ->
      {StashChunkCounter}
    [] candidate = StashReadyNoReason ->
      {StashReadyTotal}
    [] candidate = StashReadyReasons ->
      {StashReadyTotal, StashReadyReasonCounter}
    [] candidate = StashDeliverNoReason ->
      {StashDeliverTotal}
    [] candidate = StashDeliverReasons ->
      {StashDeliverTotal, StashDeliverReasonCounter}
    [] candidate = DifferentReasonsDistinct ->
      {StashReadyReasonCounter, StashDeliverReasonCounter,
       ReasonBucketsDistinct}
    [] candidate = UpdateSnapshotAggregatesPending ->
      {AggregatePendingSessions, AggregatePendingChunksBytes}
    [] candidate = UpdateSnapshotRecordsCapsTtl ->
      {StoreCapsTtl}
    [] candidate = UpdateSnapshotProjectsEntryKey ->
      {EntryKeyProjected}
    [] candidate = UpdateSnapshotProjectsEntryBufferedCounts ->
      {EntryBufferedCountsProjected}
    [] candidate = UpdateSnapshotProjectsEntryDropBreakdown ->
      {EntryDropBreakdownProjected}
    [] candidate = UpdateSnapshotProjectsEntryAge ->
      {EntryAgeProjected}
    [] candidate = UpdateSnapshotKeepsAtomicCounters ->
      {AtomicCountersPreservedOnUpdate}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetClearsSnapshot /\
          Bug = "reset_keeps_snapshot" ->
      spec \ {ResetSnapshot}
    [] candidate = ResetClearsDropCounters /\
          Bug = "reset_keeps_drop_counters" ->
      spec \ {ResetDrops}
    [] candidate = ResetClearsStashCounters /\
          Bug = "reset_keeps_stash_counters" ->
      spec \ {ResetStash}
    [] candidate = SetSnapshotStoresStructure /\
          Bug = "set_snapshot_drops_structure" ->
      spec \ {StoreStructure}
    [] candidate = SnapshotOverlaysDropCounters /\
          Bug = "snapshot_drops_drop_overlay" ->
      spec \ {OverlayDropCounters}
    [] candidate = SnapshotOverlaysStashCounters /\
          Bug = "snapshot_drops_stash_overlay" ->
      spec \ {OverlayStashCounters}
    [] candidate = SnapshotPreservesEntries /\
          Bug = "snapshot_drops_entries" ->
      spec \ {PreserveEntries}
    [] candidate = DropCapFramesFloored /\
          Bug = "drop_zero_frames_not_floored" ->
      spec \ {FloorDropFrames}
    [] candidate = DropSessionCapCountsAsCap /\
          Bug = "session_cap_not_cap_bucket" ->
      spec \ {CapBucket}
    [] candidate = DropTtlSeparateBucket /\
          Bug = "ttl_counts_as_cap" ->
      (spec \ {TtlBucket}) \cup {CapBucket}
    [] candidate = DropUnknownOnlyTotals /\
          Bug = "unknown_reason_counts_bucket" ->
      (spec \ {UnknownNoReasonBucket}) \cup {CapBucket}
    [] candidate = DropBytesAccumulate /\
          Bug = "drop_bytes_not_counted" ->
      spec \ {DropBytesAccumulated}
    [] candidate = DropRepeatedAccumulates /\
          Bug = "repeated_drop_overwrites" ->
      spec \ {RepeatedDropsAccumulated}
    [] candidate = EvictedAccumulates /\
          Bug = "evicted_not_counted" ->
      spec \ {EvictedCounterAccumulated}
    [] candidate = StashChunkOnly /\
          Bug = "chunk_increments_ready" ->
      (spec \ {StashChunkCounter}) \cup {StashReadyTotal}
    [] candidate = StashReadyNoReason /\
          Bug = "ready_no_reason_sets_reason" ->
      spec \cup {StashReadyReasonCounter}
    [] candidate = StashReadyReasons /\
          Bug = "ready_reason_not_counted" ->
      spec \ {StashReadyReasonCounter}
    [] candidate = StashDeliverNoReason /\
          Bug = "deliver_no_reason_sets_reason" ->
      spec \cup {StashDeliverReasonCounter}
    [] candidate = StashDeliverReasons /\
          Bug = "deliver_reason_not_counted" ->
      spec \ {StashDeliverReasonCounter}
    [] candidate = DifferentReasonsDistinct /\
          Bug = "reason_buckets_collide" ->
      spec \ {ReasonBucketsDistinct}
    [] candidate = UpdateSnapshotAggregatesPending /\
          Bug = "update_drops_pending_aggregates" ->
      spec \ {AggregatePendingSessions, AggregatePendingChunksBytes}
    [] candidate = UpdateSnapshotRecordsCapsTtl /\
          Bug = "update_drops_caps_ttl" ->
      spec \ {StoreCapsTtl}
    [] candidate = UpdateSnapshotProjectsEntryKey /\
          Bug = "entry_key_not_projected" ->
      spec \ {EntryKeyProjected}
    [] candidate = UpdateSnapshotProjectsEntryBufferedCounts /\
          Bug = "entry_buffered_counts_missing" ->
      spec \ {EntryBufferedCountsProjected}
    [] candidate = UpdateSnapshotProjectsEntryDropBreakdown /\
          Bug = "entry_drop_breakdown_missing" ->
      spec \ {EntryDropBreakdownProjected}
    [] candidate = UpdateSnapshotProjectsEntryAge /\
          Bug = "entry_age_missing" ->
      spec \ {EntryAgeProjected}
    [] candidate = UpdateSnapshotKeepsAtomicCounters /\
          Bug = "update_resets_atomic_counters" ->
      spec \ {AtomicCountersPreservedOnUpdate}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "reset_keeps_snapshot",
       "reset_keeps_drop_counters",
       "reset_keeps_stash_counters",
       "set_snapshot_drops_structure",
       "snapshot_drops_drop_overlay",
       "snapshot_drops_stash_overlay",
       "snapshot_drops_entries",
       "drop_zero_frames_not_floored",
       "session_cap_not_cap_bucket",
       "ttl_counts_as_cap",
       "unknown_reason_counts_bucket",
       "drop_bytes_not_counted",
       "repeated_drop_overwrites",
       "evicted_not_counted",
       "chunk_increments_ready",
       "ready_no_reason_sets_reason",
       "ready_reason_not_counted",
       "deliver_no_reason_sets_reason",
       "deliver_reason_not_counted",
       "reason_buckets_collide",
       "update_drops_pending_aggregates",
       "update_drops_caps_ttl",
       "entry_key_not_projected",
       "entry_buffered_counts_missing",
       "entry_drop_breakdown_missing",
       "entry_age_missing",
       "update_resets_atomic_counters"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

PendingRbcStatusActionsMatchSpec ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

PendingRbcStatusExactness ==
  /\ PendingRbcStatusActionsMatchSpec

PendingRbcStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ PendingRbcStatusExactness

BugResetKeepsSnapshot ==
  ImplementationActions(ResetClearsSnapshot) = SpecActions(ResetClearsSnapshot)

BugResetKeepsDropCounters ==
  ImplementationActions(ResetClearsDropCounters) =
    SpecActions(ResetClearsDropCounters)

BugResetKeepsStashCounters ==
  ImplementationActions(ResetClearsStashCounters) =
    SpecActions(ResetClearsStashCounters)

BugSetSnapshotDropsStructure ==
  ImplementationActions(SetSnapshotStoresStructure) =
    SpecActions(SetSnapshotStoresStructure)

BugSnapshotDropsDropOverlay ==
  ImplementationActions(SnapshotOverlaysDropCounters) =
    SpecActions(SnapshotOverlaysDropCounters)

BugSnapshotDropsStashOverlay ==
  ImplementationActions(SnapshotOverlaysStashCounters) =
    SpecActions(SnapshotOverlaysStashCounters)

BugSnapshotDropsEntries ==
  ImplementationActions(SnapshotPreservesEntries) =
    SpecActions(SnapshotPreservesEntries)

BugDropZeroFramesNotFloored ==
  ImplementationActions(DropCapFramesFloored) =
    SpecActions(DropCapFramesFloored)

BugSessionCapNotCapBucket ==
  ImplementationActions(DropSessionCapCountsAsCap) =
    SpecActions(DropSessionCapCountsAsCap)

BugTtlCountsAsCap ==
  ImplementationActions(DropTtlSeparateBucket) =
    SpecActions(DropTtlSeparateBucket)

BugUnknownReasonCountsBucket ==
  ImplementationActions(DropUnknownOnlyTotals) =
    SpecActions(DropUnknownOnlyTotals)

BugDropBytesNotCounted ==
  ImplementationActions(DropBytesAccumulate) = SpecActions(DropBytesAccumulate)

BugRepeatedDropOverwrites ==
  ImplementationActions(DropRepeatedAccumulates) =
    SpecActions(DropRepeatedAccumulates)

BugEvictedNotCounted ==
  ImplementationActions(EvictedAccumulates) = SpecActions(EvictedAccumulates)

BugChunkIncrementsReady ==
  ImplementationActions(StashChunkOnly) = SpecActions(StashChunkOnly)

BugReadyNoReasonSetsReason ==
  ImplementationActions(StashReadyNoReason) = SpecActions(StashReadyNoReason)

BugReadyReasonNotCounted ==
  ImplementationActions(StashReadyReasons) = SpecActions(StashReadyReasons)

BugDeliverNoReasonSetsReason ==
  ImplementationActions(StashDeliverNoReason) =
    SpecActions(StashDeliverNoReason)

BugDeliverReasonNotCounted ==
  ImplementationActions(StashDeliverReasons) = SpecActions(StashDeliverReasons)

BugReasonBucketsCollide ==
  ImplementationActions(DifferentReasonsDistinct) =
    SpecActions(DifferentReasonsDistinct)

BugUpdateDropsPendingAggregates ==
  ImplementationActions(UpdateSnapshotAggregatesPending) =
    SpecActions(UpdateSnapshotAggregatesPending)

BugUpdateDropsCapsTtl ==
  ImplementationActions(UpdateSnapshotRecordsCapsTtl) =
    SpecActions(UpdateSnapshotRecordsCapsTtl)

BugEntryKeyNotProjected ==
  ImplementationActions(UpdateSnapshotProjectsEntryKey) =
    SpecActions(UpdateSnapshotProjectsEntryKey)

BugEntryBufferedCountsMissing ==
  ImplementationActions(UpdateSnapshotProjectsEntryBufferedCounts) =
    SpecActions(UpdateSnapshotProjectsEntryBufferedCounts)

BugEntryDropBreakdownMissing ==
  ImplementationActions(UpdateSnapshotProjectsEntryDropBreakdown) =
    SpecActions(UpdateSnapshotProjectsEntryDropBreakdown)

BugEntryAgeMissing ==
  ImplementationActions(UpdateSnapshotProjectsEntryAge) =
    SpecActions(UpdateSnapshotProjectsEntryAge)

BugUpdateResetsAtomicCounters ==
  ImplementationActions(UpdateSnapshotKeepsAtomicCounters) =
    SpecActions(UpdateSnapshotKeepsAtomicCounters)

====
