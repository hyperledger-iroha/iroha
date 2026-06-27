---- MODULE SumeragiRbcStoreStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi RBC store status accounting.

This slice captures `set_rbc_store_pressure(...)`,
`inc_rbc_store_backpressure_deferrals()`, `inc_rbc_store_persist_drops()`,
`inc_rbc_store_evictions(...)`, `record_rbc_store_evictions(...)`, their
`snapshot()` projection, and the test-only
`reset_rbc_store_evictions_for_tests()` helper from `status.rs`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ResetEmpty == 1
SetPressureStoresExact == 2
SetPressureZero == 3
SetPressureOverwrites == 4
BackpressureDeferralIncrements == 5
RepeatedBackpressureAccumulates == 6
PersistDropIncrements == 7
RepeatedPersistDropsAccumulate == 8
IncEvictionsZeroNoop == 9
IncEvictionsAddsCount == 10
RecordEvictionsEmptyNoop == 11
RecordEvictionsAddsCount == 12
RecordEvictionsRecordsRecent == 13
RecentEvictionsNewestFirst == 14
RecentEvictionsCapsOldest == 15
SnapshotProjectsPressure == 16
SnapshotProjectsCounters == 17
SnapshotProjectsRecent == 18
ResetAfterRecordsClears == 19

Candidates == 1..19

ResetPressure == 1
ResetBackpressure == 2
ResetPersistDrops == 3
ResetEvictionsTotal == 4
ResetRecentEvictions == 5
PressureTupleStored == 6
PressureZeroStored == 7
PressureOverwrite == 8
BackpressureIncrement == 9
BackpressureAccumulation == 10
PersistDropIncrement == 11
PersistDropAccumulation == 12
EvictionZeroNoop == 13
EvictionCountAdded == 14
RecordEmptyNoop == 15
RecordEvictionCountAdded == 16
RecentEvictionStored == 17
RecentNewestFirst == 18
RecentCapDropsOldest == 19
SnapshotPressureMatch == 20
SnapshotCountersMatch == 21
SnapshotRecentMatch == 22

Actions == 1..22

AllResetActions ==
  {ResetPressure, ResetBackpressure, ResetPersistDrops, ResetEvictionsTotal,
   ResetRecentEvictions}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = SetPressureStoresExact ->
      {PressureTupleStored, SnapshotPressureMatch}
    [] candidate = SetPressureZero ->
      {PressureZeroStored, SnapshotPressureMatch}
    [] candidate = SetPressureOverwrites ->
      {PressureTupleStored, PressureOverwrite, SnapshotPressureMatch}
    [] candidate = BackpressureDeferralIncrements ->
      {BackpressureIncrement, SnapshotCountersMatch}
    [] candidate = RepeatedBackpressureAccumulates ->
      {BackpressureIncrement, BackpressureAccumulation, SnapshotCountersMatch}
    [] candidate = PersistDropIncrements ->
      {PersistDropIncrement, SnapshotCountersMatch}
    [] candidate = RepeatedPersistDropsAccumulate ->
      {PersistDropIncrement, PersistDropAccumulation, SnapshotCountersMatch}
    [] candidate = IncEvictionsZeroNoop ->
      {EvictionZeroNoop, SnapshotCountersMatch}
    [] candidate = IncEvictionsAddsCount ->
      {EvictionCountAdded, SnapshotCountersMatch}
    [] candidate = RecordEvictionsEmptyNoop ->
      {RecordEmptyNoop, SnapshotCountersMatch, SnapshotRecentMatch}
    [] candidate = RecordEvictionsAddsCount ->
      {RecordEvictionCountAdded, SnapshotCountersMatch}
    [] candidate = RecordEvictionsRecordsRecent ->
      {RecentEvictionStored, SnapshotRecentMatch}
    [] candidate = RecentEvictionsNewestFirst ->
      {RecentNewestFirst, SnapshotRecentMatch}
    [] candidate = RecentEvictionsCapsOldest ->
      {RecentCapDropsOldest, SnapshotRecentMatch}
    [] candidate = SnapshotProjectsPressure ->
      {SnapshotPressureMatch}
    [] candidate = SnapshotProjectsCounters ->
      {SnapshotCountersMatch}
    [] candidate = SnapshotProjectsRecent ->
      {SnapshotRecentMatch}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_pressure" ->
      spec \ {ResetPressure}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_counters" ->
      spec \ {ResetBackpressure, ResetPersistDrops, ResetEvictionsTotal}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_recent" ->
      spec \ {ResetRecentEvictions}
    [] candidate = SetPressureStoresExact /\ Bug = "pressure_not_stored" ->
      spec \ {PressureTupleStored}
    [] candidate = SetPressureZero /\ Bug = "pressure_zero_rejected" ->
      spec \ {PressureZeroStored}
    [] candidate = SetPressureOverwrites /\ Bug = "pressure_overwrite_ignored" ->
      spec \ {PressureOverwrite}
    [] candidate = BackpressureDeferralIncrements /\
          Bug = "backpressure_not_counted" ->
      spec \ {BackpressureIncrement}
    [] candidate = RepeatedBackpressureAccumulates /\
          Bug = "repeated_backpressure_overwrites_count" ->
      spec \ {BackpressureAccumulation}
    [] candidate = PersistDropIncrements /\ Bug = "persist_drop_not_counted" ->
      spec \ {PersistDropIncrement}
    [] candidate = RepeatedPersistDropsAccumulate /\
          Bug = "repeated_persist_drops_overwrite_count" ->
      spec \ {PersistDropAccumulation}
    [] candidate = IncEvictionsZeroNoop /\ Bug = "eviction_zero_increments" ->
      (spec \ {EvictionZeroNoop}) \cup {EvictionCountAdded}
    [] candidate = IncEvictionsAddsCount /\ Bug = "eviction_count_not_added" ->
      spec \ {EvictionCountAdded}
    [] candidate = RecordEvictionsEmptyNoop /\
          Bug = "record_empty_increments_total" ->
      (spec \ {RecordEmptyNoop}) \cup {RecordEvictionCountAdded}
    [] candidate = RecordEvictionsAddsCount /\
          Bug = "record_evictions_no_count" ->
      spec \ {RecordEvictionCountAdded}
    [] candidate = RecordEvictionsRecordsRecent /\
          Bug = "record_evictions_skips_recent" ->
      spec \ {RecentEvictionStored, SnapshotRecentMatch}
    [] candidate = RecentEvictionsNewestFirst /\
          Bug = "recent_evictions_oldest_first" ->
      spec \ {RecentNewestFirst}
    [] candidate = RecentEvictionsCapsOldest /\
          Bug = "recent_evictions_keeps_oldest_over_cap" ->
      spec \ {RecentCapDropsOldest}
    [] candidate = SnapshotProjectsPressure /\
          Bug = "snapshot_pressure_mismatch" ->
      spec \ {SnapshotPressureMatch}
    [] candidate = SnapshotProjectsCounters /\
          Bug = "snapshot_counters_mismatch" ->
      spec \ {SnapshotCountersMatch}
    [] candidate = SnapshotProjectsRecent /\ Bug = "snapshot_recent_mismatch" ->
      spec \ {SnapshotRecentMatch}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_pressure" ->
      spec \ {ResetPressure}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_counters" ->
      spec \ {ResetBackpressure, ResetPersistDrops, ResetEvictionsTotal}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_recent" ->
      spec \ {ResetRecentEvictions}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  \/ /\ checked < 19
     /\ checked' = checked + 1
  \/ /\ checked = 19
     /\ checked' = checked

TypeInvariant ==
  checked \in 0..19

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

AllCandidatesMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

AllSpecActionsWithinDomain ==
  \A candidate \in Candidates:
    SpecActions(candidate) \subseteq Actions

AllImplementationActionsWithinDomain ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) \subseteq Actions

ResetAnchors ==
  /\ ImplementationActions(ResetEmpty) = AllResetActions
  /\ ImplementationActions(ResetAfterRecordsClears) = AllResetActions

PressureAnchors ==
  /\ PressureTupleStored \in ImplementationActions(SetPressureStoresExact)
  /\ SnapshotPressureMatch \in ImplementationActions(SetPressureStoresExact)
  /\ PressureZeroStored \in ImplementationActions(SetPressureZero)
  /\ SnapshotPressureMatch \in ImplementationActions(SetPressureZero)
  /\ PressureTupleStored \in ImplementationActions(SetPressureOverwrites)
  /\ PressureOverwrite \in ImplementationActions(SetPressureOverwrites)
  /\ SnapshotPressureMatch \in ImplementationActions(SetPressureOverwrites)

CounterAnchors ==
  /\ BackpressureIncrement \in
       ImplementationActions(BackpressureDeferralIncrements)
  /\ BackpressureAccumulation \in
       ImplementationActions(RepeatedBackpressureAccumulates)
  /\ PersistDropIncrement \in ImplementationActions(PersistDropIncrements)
  /\ PersistDropAccumulation \in
       ImplementationActions(RepeatedPersistDropsAccumulate)
  /\ SnapshotCountersMatch \in
       ImplementationActions(BackpressureDeferralIncrements)
  /\ SnapshotCountersMatch \in ImplementationActions(PersistDropIncrements)

EvictionAnchors ==
  /\ EvictionZeroNoop \in ImplementationActions(IncEvictionsZeroNoop)
  /\ ~(EvictionCountAdded \in ImplementationActions(IncEvictionsZeroNoop))
  /\ EvictionCountAdded \in ImplementationActions(IncEvictionsAddsCount)
  /\ RecordEmptyNoop \in ImplementationActions(RecordEvictionsEmptyNoop)
  /\ ~(RecordEvictionCountAdded \in
       ImplementationActions(RecordEvictionsEmptyNoop))
  /\ RecordEvictionCountAdded \in
       ImplementationActions(RecordEvictionsAddsCount)

RecentEvictionAnchors ==
  /\ RecentEvictionStored \in
       ImplementationActions(RecordEvictionsRecordsRecent)
  /\ RecentNewestFirst \in ImplementationActions(RecentEvictionsNewestFirst)
  /\ RecentCapDropsOldest \in
       ImplementationActions(RecentEvictionsCapsOldest)
  /\ SnapshotRecentMatch \in
       ImplementationActions(RecordEvictionsRecordsRecent)
  /\ SnapshotRecentMatch \in
       ImplementationActions(RecentEvictionsNewestFirst)
  /\ SnapshotRecentMatch \in
       ImplementationActions(RecentEvictionsCapsOldest)

SnapshotAnchors ==
  /\ SnapshotPressureMatch \in ImplementationActions(SnapshotProjectsPressure)
  /\ SnapshotCountersMatch \in ImplementationActions(SnapshotProjectsCounters)
  /\ SnapshotRecentMatch \in ImplementationActions(SnapshotProjectsRecent)

StatusSafetyAnchors ==
  /\ AllCandidatesMatchSpec
  /\ AllSpecActionsWithinDomain
  /\ AllImplementationActionsWithinDomain
  /\ ResetAnchors
  /\ PressureAnchors
  /\ CounterAnchors
  /\ EvictionAnchors
  /\ RecentEvictionAnchors
  /\ SnapshotAnchors

RbcStoreStatusExactness ==
  Safety

RbcStoreStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcStoreStatusExactness
  /\ StatusSafetyAnchors

BugResetEmptyKeepsPressure ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsCounters ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsRecent ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugPressureNotStored ==
  ImplementationActions(SetPressureStoresExact) =
    SpecActions(SetPressureStoresExact)

BugPressureZeroRejected ==
  ImplementationActions(SetPressureZero) = SpecActions(SetPressureZero)

BugPressureOverwriteIgnored ==
  ImplementationActions(SetPressureOverwrites) =
    SpecActions(SetPressureOverwrites)

BugBackpressureNotCounted ==
  ImplementationActions(BackpressureDeferralIncrements) =
    SpecActions(BackpressureDeferralIncrements)

BugRepeatedBackpressureOverwritesCount ==
  ImplementationActions(RepeatedBackpressureAccumulates) =
    SpecActions(RepeatedBackpressureAccumulates)

BugPersistDropNotCounted ==
  ImplementationActions(PersistDropIncrements) =
    SpecActions(PersistDropIncrements)

BugRepeatedPersistDropsOverwriteCount ==
  ImplementationActions(RepeatedPersistDropsAccumulate) =
    SpecActions(RepeatedPersistDropsAccumulate)

BugEvictionZeroIncrements ==
  ImplementationActions(IncEvictionsZeroNoop) =
    SpecActions(IncEvictionsZeroNoop)

BugEvictionCountNotAdded ==
  ImplementationActions(IncEvictionsAddsCount) =
    SpecActions(IncEvictionsAddsCount)

BugRecordEmptyIncrementsTotal ==
  ImplementationActions(RecordEvictionsEmptyNoop) =
    SpecActions(RecordEvictionsEmptyNoop)

BugRecordEvictionsNoCount ==
  ImplementationActions(RecordEvictionsAddsCount) =
    SpecActions(RecordEvictionsAddsCount)

BugRecordEvictionsSkipsRecent ==
  ImplementationActions(RecordEvictionsRecordsRecent) =
    SpecActions(RecordEvictionsRecordsRecent)

BugRecentEvictionsOldestFirst ==
  ImplementationActions(RecentEvictionsNewestFirst) =
    SpecActions(RecentEvictionsNewestFirst)

BugRecentEvictionsKeepsOldestOverCap ==
  ImplementationActions(RecentEvictionsCapsOldest) =
    SpecActions(RecentEvictionsCapsOldest)

BugSnapshotPressureMismatch ==
  ImplementationActions(SnapshotProjectsPressure) =
    SpecActions(SnapshotProjectsPressure)

BugSnapshotCountersMismatch ==
  ImplementationActions(SnapshotProjectsCounters) =
    SpecActions(SnapshotProjectsCounters)

BugSnapshotRecentMismatch ==
  ImplementationActions(SnapshotProjectsRecent) =
    SpecActions(SnapshotProjectsRecent)

BugResetAfterRecordsKeepsPressure ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsCounters ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsRecent ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
====
