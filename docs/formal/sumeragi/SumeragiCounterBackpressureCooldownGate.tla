---- MODULE SumeragiCounterBackpressureCooldownGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for counter-driven Sumeragi backpressure helpers.

This slice captures the shared contract of `RelayBackpressure`,
`QueueDropBackpressure`, and `QueueBlockBackpressure` in `main_loop.rs`:
- constructors snapshot the current counter and start inactive,
- queue drop/block counters use only block-payload and RBC-chunk worker queues,
  with saturating addition,
- an observed counter increase stores the new counter, records `now`, and keeps
  the gate active only while elapsed time is strictly below cooldown,
- stale windows expire at the cooldown boundary and after it,
- backward clock samples saturate elapsed age at zero, and
- test-only reset/disable helpers clear the timestamp and fail closed.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

RelayInitSnapshotsCurrent == "relay_init_snapshots_current"
DropInitSnapshotsCurrent == "drop_init_snapshots_current"
BlockInitSnapshotsCurrent == "block_init_snapshots_current"
DropSourceBlockPayload == "drop_source_block_payload"
DropSourceRbcChunk == "drop_source_rbc_chunk"
DropSourceBothSaturating == "drop_source_both_saturating"
DropSourceIgnoresOther == "drop_source_ignores_other"
BlockSourceBlockPayload == "block_source_block_payload"
BlockSourceRbcChunk == "block_source_rbc_chunk"
BlockSourceBothSaturating == "block_source_both_saturating"
NoIncreaseNoTimestampInactive == "no_increase_no_timestamp_inactive"
IncreaseArmsActive == "increase_arms_active"
WithinCooldownActive == "within_cooldown_active"
CooldownBoundaryInactive == "cooldown_boundary_inactive"
AfterCooldownInactive == "after_cooldown_inactive"
BackwardTimeActive == "backward_time_active"
ZeroCooldownInactiveOnIncrease == "zero_cooldown_inactive_on_increase"
ResetSnapshotsCurrent == "reset_snapshots_current"
RelayDisableSuppressesDrop == "relay_disable_suppresses_drop"

Cases == {
  RelayInitSnapshotsCurrent,
  DropInitSnapshotsCurrent,
  BlockInitSnapshotsCurrent,
  DropSourceBlockPayload,
  DropSourceRbcChunk,
  DropSourceBothSaturating,
  DropSourceIgnoresOther,
  BlockSourceBlockPayload,
  BlockSourceRbcChunk,
  BlockSourceBothSaturating,
  NoIncreaseNoTimestampInactive,
  IncreaseArmsActive,
  WithinCooldownActive,
  CooldownBoundaryInactive,
  AfterCooldownInactive,
  BackwardTimeActive,
  ZeroCooldownInactiveOnIncrease,
  ResetSnapshotsCurrent,
  RelayDisableSuppressesDrop
}

RelayCounter == 1
DropCounter == 2
BlockCounter == 3
InitSnapshotsCurrent == 4
TimestampAbsent == 5
TimestampSetNow == 6
TimestampPreserved == 7
Inactive == 8
Active == 9
SourceBlockPayload == 10
SourceRbcChunk == 11
SaturatingAdd == 12
IgnoresOtherQueues == 13
NoIncrease == 14
IncreaseDetected == 15
CountUnchanged == 16
CountUpdatedToCurrent == 17
CooldownStrict == 18
BoundarySample == 19
SaturatingTime == 20
ZeroCooldown == 21
ResetAction == 22
DisableMax == 23
CountStale == 24
CountsOtherQueues == 25
WrappingAdd == 26

Actions == 1..26

SpecActions(c) ==
  CASE c = RelayInitSnapshotsCurrent ->
      {RelayCounter, InitSnapshotsCurrent, TimestampAbsent, Inactive}
    [] c = DropInitSnapshotsCurrent ->
      {DropCounter, InitSnapshotsCurrent, TimestampAbsent, Inactive}
    [] c = BlockInitSnapshotsCurrent ->
      {BlockCounter, InitSnapshotsCurrent, TimestampAbsent, Inactive}
    [] c = DropSourceBlockPayload ->
      {DropCounter, SourceBlockPayload}
    [] c = DropSourceRbcChunk ->
      {DropCounter, SourceRbcChunk}
    [] c = DropSourceBothSaturating ->
      {DropCounter, SourceBlockPayload, SourceRbcChunk, SaturatingAdd}
    [] c = DropSourceIgnoresOther ->
      {DropCounter, IgnoresOtherQueues}
    [] c = BlockSourceBlockPayload ->
      {BlockCounter, SourceBlockPayload}
    [] c = BlockSourceRbcChunk ->
      {BlockCounter, SourceRbcChunk}
    [] c = BlockSourceBothSaturating ->
      {BlockCounter, SourceBlockPayload, SourceRbcChunk, SaturatingAdd}
    [] c = NoIncreaseNoTimestampInactive ->
      {NoIncrease, CountUnchanged, TimestampAbsent, Inactive}
    [] c = IncreaseArmsActive ->
      {IncreaseDetected, CountUpdatedToCurrent, TimestampSetNow, Active}
    [] c = WithinCooldownActive ->
      {NoIncrease, CountUnchanged, TimestampPreserved, CooldownStrict, Active}
    [] c = CooldownBoundaryInactive ->
      {NoIncrease, CountUnchanged, TimestampPreserved, CooldownStrict,
       BoundarySample, Inactive}
    [] c = AfterCooldownInactive ->
      {NoIncrease, CountUnchanged, TimestampPreserved, Inactive}
    [] c = BackwardTimeActive ->
      {NoIncrease, CountUnchanged, TimestampPreserved, SaturatingTime, Active}
    [] c = ZeroCooldownInactiveOnIncrease ->
      {IncreaseDetected, CountUpdatedToCurrent, TimestampSetNow, ZeroCooldown,
       Inactive}
    [] c = ResetSnapshotsCurrent ->
      {ResetAction, CountUpdatedToCurrent, TimestampAbsent, Inactive}
    [] c = RelayDisableSuppressesDrop ->
      {RelayCounter, DisableMax, TimestampAbsent, Inactive}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "init_not_snapshotted"
       /\ c = RelayInitSnapshotsCurrent ->
      (spec \ {InitSnapshotsCurrent}) \cup {CountStale}
    [] Bug = "no_increase_active"
       /\ c = NoIncreaseNoTimestampInactive ->
      (spec \ {Inactive}) \cup {Active}
    [] Bug = "increase_inactive"
       /\ c = IncreaseArmsActive ->
      (spec \ {Active}) \cup {Inactive}
    [] Bug = "increase_count_stale"
       /\ c = IncreaseArmsActive ->
      (spec \ {CountUpdatedToCurrent}) \cup {CountStale}
    [] Bug = "within_cooldown_inactive"
       /\ c = WithinCooldownActive ->
      (spec \ {Active}) \cup {Inactive}
    [] Bug = "boundary_active"
       /\ c = CooldownBoundaryInactive ->
      (spec \ {Inactive}) \cup {Active}
    [] Bug = "after_cooldown_active"
       /\ c = AfterCooldownInactive ->
      (spec \ {Inactive}) \cup {Active}
    [] Bug = "backward_time_inactive"
       /\ c = BackwardTimeActive ->
      (spec \ {SaturatingTime, Active}) \cup {Inactive}
    [] Bug = "zero_cooldown_active"
       /\ c = ZeroCooldownInactiveOnIncrease ->
      (spec \ {Inactive}) \cup {Active}
    [] Bug = "reset_keeps_timestamp"
       /\ c = ResetSnapshotsCurrent ->
      (spec \ {TimestampAbsent}) \cup {TimestampPreserved}
    [] Bug = "reset_keeps_old_count"
       /\ c = ResetSnapshotsCurrent ->
      (spec \ {CountUpdatedToCurrent}) \cup {CountUnchanged}
    [] Bug = "disable_allows_drop"
       /\ c = RelayDisableSuppressesDrop ->
      (spec \ {Inactive}) \cup {Active}
    [] Bug = "drop_omits_block_payload"
       /\ c = DropSourceBlockPayload ->
      (spec \ {SourceBlockPayload}) \cup {CountStale}
    [] Bug = "drop_omits_rbc_chunk"
       /\ c = DropSourceRbcChunk ->
      (spec \ {SourceRbcChunk}) \cup {CountStale}
    [] Bug = "drop_counts_unrelated_queue"
       /\ c = DropSourceIgnoresOther ->
      (spec \ {IgnoresOtherQueues}) \cup {CountsOtherQueues}
    [] Bug = "drop_wraps_overflow"
       /\ c = DropSourceBothSaturating ->
      (spec \ {SaturatingAdd}) \cup {WrappingAdd}
    [] Bug = "block_uses_dropped_total"
       /\ c = BlockSourceBlockPayload ->
      (spec \ {BlockCounter}) \cup {DropCounter}
    [] Bug = "block_omits_rbc_chunk"
       /\ c = BlockSourceRbcChunk ->
      (spec \ {SourceRbcChunk}) \cup {CountStale}
    [] OTHER -> spec

Bugs == {
  "none",
  "init_not_snapshotted",
  "no_increase_active",
  "increase_inactive",
  "increase_count_stale",
  "within_cooldown_inactive",
  "boundary_active",
  "after_cooldown_active",
  "backward_time_inactive",
  "zero_cooldown_active",
  "reset_keeps_timestamp",
  "reset_keeps_old_count",
  "disable_allows_drop",
  "drop_omits_block_payload",
  "drop_omits_rbc_chunk",
  "drop_counts_unrelated_queue",
  "drop_wraps_overflow",
  "block_uses_dropped_total",
  "block_omits_rbc_chunk"
}

Init == checked = 0

Next == UNCHANGED vars

TypeInvariant ==
  /\ checked = 0
  /\ Bug \in Bugs
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

CounterBackpressureCooldownCoreSafety ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

CounterBackpressureCooldownExactness ==
  CounterBackpressureCooldownCoreSafety

CounterBackpressureCooldownCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ CounterBackpressureCooldownExactness

NoBugInvariant == CounterBackpressureCooldownExactness

SafetyFast == CounterBackpressureCooldownExactness

BugInitNotSnapshotted == NoBugInvariant
BugNoIncreaseActive == NoBugInvariant
BugIncreaseInactive == NoBugInvariant
BugIncreaseCountStale == NoBugInvariant
BugWithinCooldownInactive == NoBugInvariant
BugBoundaryActive == NoBugInvariant
BugAfterCooldownActive == NoBugInvariant
BugBackwardTimeInactive == NoBugInvariant
BugZeroCooldownActive == NoBugInvariant
BugResetKeepsTimestamp == NoBugInvariant
BugResetKeepsOldCount == NoBugInvariant
BugDisableAllowsDrop == NoBugInvariant
BugDropOmitsBlockPayload == NoBugInvariant
BugDropOmitsRbcChunk == NoBugInvariant
BugDropCountsUnrelatedQueue == NoBugInvariant
BugDropWrapsOverflow == NoBugInvariant
BugBlockUsesDroppedTotal == NoBugInvariant
BugBlockOmitsRbcChunk == NoBugInvariant

====
