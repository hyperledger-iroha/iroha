---- MODULE SumeragiHotspotLogSummaryGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `HotspotLogSummary`.

This slice captures the periodic hotspot-summary accumulator in
`main_loop.rs`. It pins the deterministic helper contract:
- construction and reset set `last_emit` to `now` and clear every counter,
- every warning/suppressed counter is updated with saturating addition,
- `emit_if_due(...)` does nothing while elapsed time is strictly below the
  summary interval, including backward-time samples,
- the interval boundary is due,
- due samples log when any tracked counter is nonzero,
- due samples with no counters refresh `last_emit` without logging, and
- every due/reset path clears all counters and refreshes `last_emit`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

InitClearsCounters == "init_clears_counters"
RecordTickWarn == "record_tick_warn"
RecordTickDebug == "record_tick_debug"
RecordBlockSyncWarn == "record_block_sync_warn"
RecordBlockSyncSuppressed == "record_block_sync_suppressed"
RecordQcWarn == "record_qc_warn"
RecordQcSuppressed == "record_qc_suppressed"
SaturatingWarnAtMax == "saturating_warn_at_max"
SaturatingSuppressedAtMax == "saturating_suppressed_at_max"
EmitBeforeIntervalPreserves == "emit_before_interval_preserves"
EmitBackwardTimePreserves == "emit_backward_time_preserves"
EmitBoundaryTickWarnLogs == "emit_boundary_tick_warn_logs"
EmitBoundaryTickDebugLogs == "emit_boundary_tick_debug_logs"
EmitBoundaryBlockWarnLogs == "emit_boundary_block_warn_logs"
EmitBoundaryBlockSuppressedLogs == "emit_boundary_block_suppressed_logs"
EmitBoundaryQcWarnLogs == "emit_boundary_qc_warn_logs"
EmitBoundaryQcSuppressedLogs == "emit_boundary_qc_suppressed_logs"
EmitBoundaryNoCountsRefreshesNoLog ==
  "emit_boundary_no_counts_refreshes_no_log"
EmitAfterIntervalClearsAll == "emit_after_interval_clears_all"
ResetClearsCounters == "reset_clears_counters"

Cases == {
  InitClearsCounters,
  RecordTickWarn,
  RecordTickDebug,
  RecordBlockSyncWarn,
  RecordBlockSyncSuppressed,
  RecordQcWarn,
  RecordQcSuppressed,
  SaturatingWarnAtMax,
  SaturatingSuppressedAtMax,
  EmitBeforeIntervalPreserves,
  EmitBackwardTimePreserves,
  EmitBoundaryTickWarnLogs,
  EmitBoundaryTickDebugLogs,
  EmitBoundaryBlockWarnLogs,
  EmitBoundaryBlockSuppressedLogs,
  EmitBoundaryQcWarnLogs,
  EmitBoundaryQcSuppressedLogs,
  EmitBoundaryNoCountsRefreshesNoLog,
  EmitAfterIntervalClearsAll,
  ResetClearsCounters
}

LastEmitNow == 1
LastEmitPreserved == 2
CountersCleared == 3
CountersPreserved == 4
TickWarnIncremented == 5
TickDebugIncremented == 6
BlockSyncWarnIncremented == 7
BlockSyncSuppressedAdded == 8
QcWarnIncremented == 9
QcSuppressedAdded == 10
SaturatingAdd == 11
NoLog == 12
LogEmitted == 13
IntervalNotDue == 14
IntervalBoundaryDue == 15
AfterIntervalDue == 16
SaturatingTime == 17
TickWarnCondition == 18
TickDebugCondition == 19
BlockWarnCondition == 20
BlockSuppressedCondition == 21
QcWarnCondition == 22
QcSuppressedCondition == 23
WrappedAdd == 24
WrongCounterUpdated == 25
CountersStale == 26

Actions == 1..26

SpecActions(c) ==
  CASE c = InitClearsCounters ->
      {LastEmitNow, CountersCleared, NoLog}
    [] c = RecordTickWarn ->
      {TickWarnIncremented, SaturatingAdd, LastEmitPreserved}
    [] c = RecordTickDebug ->
      {TickDebugIncremented, SaturatingAdd, LastEmitPreserved}
    [] c = RecordBlockSyncWarn ->
      {BlockSyncWarnIncremented, SaturatingAdd, LastEmitPreserved}
    [] c = RecordBlockSyncSuppressed ->
      {BlockSyncSuppressedAdded, SaturatingAdd, LastEmitPreserved}
    [] c = RecordQcWarn ->
      {QcWarnIncremented, SaturatingAdd, LastEmitPreserved}
    [] c = RecordQcSuppressed ->
      {QcSuppressedAdded, SaturatingAdd, LastEmitPreserved}
    [] c = SaturatingWarnAtMax ->
      {TickWarnIncremented, SaturatingAdd, LastEmitPreserved}
    [] c = SaturatingSuppressedAtMax ->
      {BlockSyncSuppressedAdded, SaturatingAdd, LastEmitPreserved}
    [] c = EmitBeforeIntervalPreserves ->
      {IntervalNotDue, CountersPreserved, LastEmitPreserved, NoLog}
    [] c = EmitBackwardTimePreserves ->
      {IntervalNotDue, SaturatingTime, CountersPreserved, LastEmitPreserved,
       NoLog}
    [] c = EmitBoundaryTickWarnLogs ->
      {IntervalBoundaryDue, TickWarnCondition, LogEmitted, CountersCleared,
       LastEmitNow}
    [] c = EmitBoundaryTickDebugLogs ->
      {IntervalBoundaryDue, TickDebugCondition, LogEmitted, CountersCleared,
       LastEmitNow}
    [] c = EmitBoundaryBlockWarnLogs ->
      {IntervalBoundaryDue, BlockWarnCondition, LogEmitted, CountersCleared,
       LastEmitNow}
    [] c = EmitBoundaryBlockSuppressedLogs ->
      {IntervalBoundaryDue, BlockSuppressedCondition, LogEmitted,
       CountersCleared, LastEmitNow}
    [] c = EmitBoundaryQcWarnLogs ->
      {IntervalBoundaryDue, QcWarnCondition, LogEmitted, CountersCleared,
       LastEmitNow}
    [] c = EmitBoundaryQcSuppressedLogs ->
      {IntervalBoundaryDue, QcSuppressedCondition, LogEmitted,
       CountersCleared, LastEmitNow}
    [] c = EmitBoundaryNoCountsRefreshesNoLog ->
      {IntervalBoundaryDue, NoLog, CountersCleared, LastEmitNow}
    [] c = EmitAfterIntervalClearsAll ->
      {AfterIntervalDue, LogEmitted, CountersCleared, LastEmitNow}
    [] c = ResetClearsCounters ->
      {CountersCleared, LastEmitNow, NoLog}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "init_keeps_counts"
       /\ c = InitClearsCounters ->
      (spec \ {CountersCleared}) \cup {CountersStale}
    [] Bug = "init_keeps_last_emit"
       /\ c = InitClearsCounters ->
      (spec \ {LastEmitNow}) \cup {LastEmitPreserved}
    [] Bug = "tick_warn_not_incremented"
       /\ c = RecordTickWarn ->
      spec \ {TickWarnIncremented}
    [] Bug = "tick_debug_updates_warn"
       /\ c = RecordTickDebug ->
      (spec \ {TickDebugIncremented}) \cup {TickWarnIncremented,
                                            WrongCounterUpdated}
    [] Bug = "block_warn_not_incremented"
       /\ c = RecordBlockSyncWarn ->
      spec \ {BlockSyncWarnIncremented}
    [] Bug = "block_suppressed_overwrites"
       /\ c = RecordBlockSyncSuppressed ->
      spec \ {SaturatingAdd}
    [] Bug = "qc_warn_updates_block"
       /\ c = RecordQcWarn ->
      (spec \ {QcWarnIncremented}) \cup {BlockSyncWarnIncremented,
                                         WrongCounterUpdated}
    [] Bug = "qc_suppressed_wraps"
       /\ c = RecordQcSuppressed ->
      (spec \ {SaturatingAdd}) \cup {WrappedAdd}
    [] Bug = "saturating_warn_wraps"
       /\ c = SaturatingWarnAtMax ->
      (spec \ {SaturatingAdd}) \cup {WrappedAdd}
    [] Bug = "saturating_suppressed_wraps"
       /\ c = SaturatingSuppressedAtMax ->
      (spec \ {SaturatingAdd}) \cup {WrappedAdd}
    [] Bug = "pre_interval_emits"
       /\ c = EmitBeforeIntervalPreserves ->
      (spec \ {NoLog}) \cup {LogEmitted}
    [] Bug = "pre_interval_resets"
       /\ c = EmitBeforeIntervalPreserves ->
      (spec \ {CountersPreserved, LastEmitPreserved}) \cup
        {CountersCleared, LastEmitNow}
    [] Bug = "backward_time_emits"
       /\ c = EmitBackwardTimePreserves ->
      (spec \ {SaturatingTime, NoLog}) \cup {LogEmitted}
    [] Bug = "boundary_not_due"
       /\ c = EmitBoundaryTickWarnLogs ->
      (spec \ {IntervalBoundaryDue, LogEmitted, CountersCleared,
               LastEmitNow}) \cup {IntervalNotDue, NoLog, CountersPreserved,
                                   LastEmitPreserved}
    [] Bug = "suppressed_only_no_log"
       /\ c = EmitBoundaryBlockSuppressedLogs ->
      (spec \ {LogEmitted}) \cup {NoLog}
    [] Bug = "qc_suppressed_only_no_log"
       /\ c = EmitBoundaryQcSuppressedLogs ->
      (spec \ {LogEmitted}) \cup {NoLog}
    [] Bug = "no_counts_logs"
       /\ c = EmitBoundaryNoCountsRefreshesNoLog ->
      (spec \ {NoLog}) \cup {LogEmitted}
    [] Bug = "due_does_not_refresh_last_emit"
       /\ c = EmitAfterIntervalClearsAll ->
      (spec \ {LastEmitNow}) \cup {LastEmitPreserved}
    [] Bug = "due_keeps_counts"
       /\ c = EmitAfterIntervalClearsAll ->
      (spec \ {CountersCleared}) \cup {CountersPreserved}
    [] Bug = "reset_keeps_counts"
       /\ c = ResetClearsCounters ->
      (spec \ {CountersCleared}) \cup {CountersStale}
    [] Bug = "reset_keeps_last_emit"
       /\ c = ResetClearsCounters ->
      (spec \ {LastEmitNow}) \cup {LastEmitPreserved}
    [] OTHER -> spec

Bugs == {
  "none",
  "init_keeps_counts",
  "init_keeps_last_emit",
  "tick_warn_not_incremented",
  "tick_debug_updates_warn",
  "block_warn_not_incremented",
  "block_suppressed_overwrites",
  "qc_warn_updates_block",
  "qc_suppressed_wraps",
  "saturating_warn_wraps",
  "saturating_suppressed_wraps",
  "pre_interval_emits",
  "pre_interval_resets",
  "backward_time_emits",
  "boundary_not_due",
  "suppressed_only_no_log",
  "qc_suppressed_only_no_log",
  "no_counts_logs",
  "due_does_not_refresh_last_emit",
  "due_keeps_counts",
  "reset_keeps_counts",
  "reset_keeps_last_emit"
}

Init == checked = 0

Next == UNCHANGED vars

TypeInvariant ==
  /\ checked = 0
  /\ Bug \in Bugs
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

NoBugInvariant ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

SafetyFast == NoBugInvariant

BugInitKeepsCounts == NoBugInvariant
BugInitKeepsLastEmit == NoBugInvariant
BugTickWarnNotIncremented == NoBugInvariant
BugTickDebugUpdatesWarn == NoBugInvariant
BugBlockWarnNotIncremented == NoBugInvariant
BugBlockSuppressedOverwrites == NoBugInvariant
BugQcWarnUpdatesBlock == NoBugInvariant
BugQcSuppressedWraps == NoBugInvariant
BugSaturatingWarnWraps == NoBugInvariant
BugSaturatingSuppressedWraps == NoBugInvariant
BugPreIntervalEmits == NoBugInvariant
BugPreIntervalResets == NoBugInvariant
BugBackwardTimeEmits == NoBugInvariant
BugBoundaryNotDue == NoBugInvariant
BugSuppressedOnlyNoLog == NoBugInvariant
BugQcSuppressedOnlyNoLog == NoBugInvariant
BugNoCountsLogs == NoBugInvariant
BugDueDoesNotRefreshLastEmit == NoBugInvariant
BugDueKeepsCounts == NoBugInvariant
BugResetKeepsCounts == NoBugInvariant
BugResetKeepsLastEmit == NoBugInvariant

====
