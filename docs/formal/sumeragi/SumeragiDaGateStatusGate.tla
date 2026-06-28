---- MODULE SumeragiDaGateStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi DA gate status accounting.

This slice captures `record_da_gate_transition(...)`,
`da_gate_missing_local_data_total()`, `snapshot().da_gate`, and the test-only
`reset_da_gate_counters_for_tests()` helper from `status.rs`: missing-local
data and manifest-guard counters, latest reason projection, recovered-data
satisfaction projection, and reset semantics.
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
MissingLocalDataRecord == 2
ManifestMissingRecord == 3
ManifestHashMismatchRecord == 4
ManifestReadFailedRecord == 5
ManifestSpoolScanRecord == 6
CurrentNoneClearsReason == 7
ClearMissingLocalDataSatisfies == 8
ClearManifestDoesNotSatisfy == 9
NoneToNoneDoesNotSatisfy == 10
RepeatedMissingAccumulates == 11
RepeatedManifestAccumulates == 12
LastReasonOverwrites == 13
SatisfactionPersistsAfterUnrelated == 14
SnapshotProjectsReason == 15
SnapshotProjectsSatisfaction == 16
SnapshotProjectsCounters == 17
MissingLocalDataGetterMatches == 18
TopLevelSnapshotIncludesDaGate == 19
ResetAfterRecordsClears == 20

Candidates == 1..20

ResetCounters == 1
ResetReason == 2
ResetSatisfied == 3
IncrementMissingLocalData == 4
IncrementManifestGuard == 5
SameCounterAccumulates == 6
SetReasonMissingLocalData == 7
SetReasonManifestMissing == 8
SetReasonManifestHashMismatch == 9
SetReasonManifestReadFailed == 10
SetReasonManifestSpoolScan == 11
SetReasonNone == 12
LastReasonOverwritesAction == 13
SetSatisfiedMissingDataRecovered == 14
LastSatisfiedUnchanged == 15
CountersPreservedOnClear == 16
SnapshotReasonMatches == 17
SnapshotSatisfiedMatches == 18
SnapshotCountersMatch == 19
GetterMissingLocalDataMatches == 20
TopLevelDaGateMatches == 21

Actions == 1..21

AllResetActions == {ResetCounters, ResetReason, ResetSatisfied}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = MissingLocalDataRecord ->
      {IncrementMissingLocalData, SetReasonMissingLocalData}
    [] candidate = ManifestMissingRecord ->
      {IncrementManifestGuard, SetReasonManifestMissing}
    [] candidate = ManifestHashMismatchRecord ->
      {IncrementManifestGuard, SetReasonManifestHashMismatch}
    [] candidate = ManifestReadFailedRecord ->
      {IncrementManifestGuard, SetReasonManifestReadFailed}
    [] candidate = ManifestSpoolScanRecord ->
      {IncrementManifestGuard, SetReasonManifestSpoolScan}
    [] candidate = CurrentNoneClearsReason ->
      {SetReasonNone, CountersPreservedOnClear}
    [] candidate = ClearMissingLocalDataSatisfies ->
      {SetReasonNone, SetSatisfiedMissingDataRecovered,
       CountersPreservedOnClear}
    [] candidate = ClearManifestDoesNotSatisfy ->
      {SetReasonNone, LastSatisfiedUnchanged, CountersPreservedOnClear}
    [] candidate = NoneToNoneDoesNotSatisfy ->
      {SetReasonNone, LastSatisfiedUnchanged, CountersPreservedOnClear}
    [] candidate = RepeatedMissingAccumulates ->
      {IncrementMissingLocalData, SameCounterAccumulates,
       SnapshotCountersMatch}
    [] candidate = RepeatedManifestAccumulates ->
      {IncrementManifestGuard, SameCounterAccumulates,
       SnapshotCountersMatch}
    [] candidate = LastReasonOverwrites ->
      {SetReasonManifestHashMismatch, LastReasonOverwritesAction,
       SnapshotReasonMatches}
    [] candidate = SatisfactionPersistsAfterUnrelated ->
      {LastSatisfiedUnchanged, SnapshotSatisfiedMatches}
    [] candidate = SnapshotProjectsReason ->
      {SnapshotReasonMatches}
    [] candidate = SnapshotProjectsSatisfaction ->
      {SnapshotSatisfiedMatches}
    [] candidate = SnapshotProjectsCounters ->
      {SnapshotCountersMatch}
    [] candidate = MissingLocalDataGetterMatches ->
      {GetterMissingLocalDataMatches, SnapshotCountersMatch}
    [] candidate = TopLevelSnapshotIncludesDaGate ->
      {TopLevelDaGateMatches}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_counters" ->
      spec \ {ResetCounters}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_status" ->
      spec \ {ResetReason, ResetSatisfied}
    [] candidate = MissingLocalDataRecord /\
          Bug = "missing_local_data_not_counted" ->
      spec \ {IncrementMissingLocalData}
    [] candidate = MissingLocalDataRecord /\
          Bug = "missing_local_data_reason_not_recorded" ->
      spec \ {SetReasonMissingLocalData}
    [] candidate = ManifestMissingRecord /\
          Bug = "manifest_guard_not_counted" ->
      spec \ {IncrementManifestGuard}
    [] candidate = ManifestMissingRecord /\
          Bug = "manifest_missing_reason_wrong" ->
      (spec \ {SetReasonManifestMissing}) \cup
        {SetReasonManifestHashMismatch}
    [] candidate = ManifestHashMismatchRecord /\
          Bug = "manifest_hash_mismatch_reason_wrong" ->
      (spec \ {SetReasonManifestHashMismatch}) \cup
        {SetReasonManifestMissing}
    [] candidate = ManifestReadFailedRecord /\
          Bug = "manifest_read_failed_reason_wrong" ->
      (spec \ {SetReasonManifestReadFailed}) \cup
        {SetReasonManifestMissing}
    [] candidate = ManifestSpoolScanRecord /\
          Bug = "manifest_spool_scan_reason_wrong" ->
      (spec \ {SetReasonManifestSpoolScan}) \cup
        {SetReasonManifestMissing}
    [] candidate = CurrentNoneClearsReason /\
          Bug = "current_none_keeps_reason" ->
      spec \ {SetReasonNone}
    [] candidate = ClearMissingLocalDataSatisfies /\
          Bug = "missing_recovery_not_satisfied" ->
      spec \ {SetSatisfiedMissingDataRecovered}
    [] candidate = ClearMissingLocalDataSatisfies /\
          Bug = "missing_recovery_clears_counter" ->
      spec \ {CountersPreservedOnClear}
    [] candidate = ClearManifestDoesNotSatisfy /\
          Bug = "manifest_clear_sets_satisfied" ->
      spec \cup {SetSatisfiedMissingDataRecovered}
    [] candidate = NoneToNoneDoesNotSatisfy /\
          Bug = "none_to_none_sets_satisfied" ->
      spec \cup {SetSatisfiedMissingDataRecovered}
    [] candidate = RepeatedMissingAccumulates /\
          Bug = "repeated_missing_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotCountersMatch}
    [] candidate = RepeatedManifestAccumulates /\
          Bug = "repeated_manifest_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotCountersMatch}
    [] candidate = LastReasonOverwrites /\
          Bug = "last_reason_not_overwritten" ->
      spec \ {SetReasonManifestHashMismatch, LastReasonOverwritesAction,
              SnapshotReasonMatches}
    [] candidate = SatisfactionPersistsAfterUnrelated /\
          Bug = "satisfied_overwritten_by_unrelated" ->
      (spec \ {LastSatisfiedUnchanged, SnapshotSatisfiedMatches}) \cup
        {ResetSatisfied}
    [] candidate = SnapshotProjectsReason /\
          Bug = "snapshot_reason_mismatch" ->
      spec \ {SnapshotReasonMatches}
    [] candidate = SnapshotProjectsSatisfaction /\
          Bug = "snapshot_satisfied_mismatch" ->
      spec \ {SnapshotSatisfiedMatches}
    [] candidate = SnapshotProjectsCounters /\
          Bug = "snapshot_counters_mismatch" ->
      spec \ {SnapshotCountersMatch}
    [] candidate = MissingLocalDataGetterMatches /\
          Bug = "getter_missing_local_data_mismatch" ->
      spec \ {GetterMissingLocalDataMatches}
    [] candidate = TopLevelSnapshotIncludesDaGate /\
          Bug = "top_level_snapshot_drops_da_gate" ->
      spec \ {TopLevelDaGateMatches}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_counters" ->
      spec \ {ResetCounters}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_status" ->
      spec \ {ResetReason, ResetSatisfied}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 20
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..20

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

DaGateStatusExactness ==
  Safety

DaGateStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ DaGateStatusExactness

BugResetEmptyKeepsCounters ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsStatus ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugMissingLocalDataNotCounted ==
  ImplementationActions(MissingLocalDataRecord) =
    SpecActions(MissingLocalDataRecord)

BugMissingLocalDataReasonNotRecorded ==
  ImplementationActions(MissingLocalDataRecord) =
    SpecActions(MissingLocalDataRecord)

BugManifestGuardNotCounted ==
  ImplementationActions(ManifestMissingRecord) =
    SpecActions(ManifestMissingRecord)

BugManifestMissingReasonWrong ==
  ImplementationActions(ManifestMissingRecord) =
    SpecActions(ManifestMissingRecord)

BugManifestHashMismatchReasonWrong ==
  ImplementationActions(ManifestHashMismatchRecord) =
    SpecActions(ManifestHashMismatchRecord)

BugManifestReadFailedReasonWrong ==
  ImplementationActions(ManifestReadFailedRecord) =
    SpecActions(ManifestReadFailedRecord)

BugManifestSpoolScanReasonWrong ==
  ImplementationActions(ManifestSpoolScanRecord) =
    SpecActions(ManifestSpoolScanRecord)

BugCurrentNoneKeepsReason ==
  ImplementationActions(CurrentNoneClearsReason) =
    SpecActions(CurrentNoneClearsReason)

BugMissingRecoveryNotSatisfied ==
  ImplementationActions(ClearMissingLocalDataSatisfies) =
    SpecActions(ClearMissingLocalDataSatisfies)

BugMissingRecoveryClearsCounter ==
  ImplementationActions(ClearMissingLocalDataSatisfies) =
    SpecActions(ClearMissingLocalDataSatisfies)

BugManifestClearSetsSatisfied ==
  ImplementationActions(ClearManifestDoesNotSatisfy) =
    SpecActions(ClearManifestDoesNotSatisfy)

BugNoneToNoneSetsSatisfied ==
  ImplementationActions(NoneToNoneDoesNotSatisfy) =
    SpecActions(NoneToNoneDoesNotSatisfy)

BugRepeatedMissingOverwritesCount ==
  ImplementationActions(RepeatedMissingAccumulates) =
    SpecActions(RepeatedMissingAccumulates)

BugRepeatedManifestOverwritesCount ==
  ImplementationActions(RepeatedManifestAccumulates) =
    SpecActions(RepeatedManifestAccumulates)

BugLastReasonNotOverwritten ==
  ImplementationActions(LastReasonOverwrites) =
    SpecActions(LastReasonOverwrites)

BugSatisfiedOverwrittenByUnrelated ==
  ImplementationActions(SatisfactionPersistsAfterUnrelated) =
    SpecActions(SatisfactionPersistsAfterUnrelated)

BugSnapshotReasonMismatch ==
  ImplementationActions(SnapshotProjectsReason) =
    SpecActions(SnapshotProjectsReason)

BugSnapshotSatisfiedMismatch ==
  ImplementationActions(SnapshotProjectsSatisfaction) =
    SpecActions(SnapshotProjectsSatisfaction)

BugSnapshotCountersMismatch ==
  ImplementationActions(SnapshotProjectsCounters) =
    SpecActions(SnapshotProjectsCounters)

BugGetterMissingLocalDataMismatch ==
  ImplementationActions(MissingLocalDataGetterMatches) =
    SpecActions(MissingLocalDataGetterMatches)

BugTopLevelSnapshotDropsDaGate ==
  ImplementationActions(TopLevelSnapshotIncludesDaGate) =
    SpecActions(TopLevelSnapshotIncludesDaGate)

BugResetAfterRecordsKeepsCounters ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsStatus ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
====
