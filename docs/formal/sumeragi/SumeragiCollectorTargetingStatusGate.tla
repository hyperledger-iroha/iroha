---- MODULE SumeragiCollectorTargetingStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi collector-targeting status counters.

This slice captures `set_collectors_targeted_current(...)`,
`observe_collectors_targeted_per_block(...)`, `inc_redundant_sends()`, their
top-level `snapshot()` projection, and the test-only
`reset_collectors_targeting_for_tests()` helper.
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
SetCurrentStoresExact == 2
SetCurrentZero == 3
SetCurrentOverwrites == 4
ObserveLastStoresExact == 5
ObserveLastZero == 6
ObserveLastOverwrites == 7
RedundantSendIncrements == 8
RepeatedRedundantSendsAccumulate == 9
CurrentDoesNotCountLast == 10
LastDoesNotCountCurrent == 11
RedundantDoesNotTouchTargets == 12
SnapshotProjectsCurrent == 13
SnapshotProjectsLast == 14
SnapshotProjectsRedundant == 15
ResetAfterRecordsClears == 16

Candidates == 1..16

ResetCurrent == 1
ResetLast == 2
ResetRedundant == 3
CurrentStored == 4
CurrentZeroStored == 5
CurrentOverwrite == 6
LastStored == 7
LastZeroStored == 8
LastOverwrite == 9
RedundantIncrement == 10
RedundantAccumulation == 11
CurrentOnlyCurrent == 12
LastOnlyLast == 13
RedundantOnlyRedundant == 14
SnapshotCurrentMatch == 15
SnapshotLastMatch == 16
SnapshotRedundantMatch == 17

Actions == 1..17

AllResetActions == {ResetCurrent, ResetLast, ResetRedundant}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = SetCurrentStoresExact ->
      {CurrentStored, SnapshotCurrentMatch}
    [] candidate = SetCurrentZero ->
      {CurrentZeroStored, SnapshotCurrentMatch}
    [] candidate = SetCurrentOverwrites ->
      {CurrentStored, CurrentOverwrite, SnapshotCurrentMatch}
    [] candidate = ObserveLastStoresExact ->
      {LastStored, SnapshotLastMatch}
    [] candidate = ObserveLastZero ->
      {LastZeroStored, SnapshotLastMatch}
    [] candidate = ObserveLastOverwrites ->
      {LastStored, LastOverwrite, SnapshotLastMatch}
    [] candidate = RedundantSendIncrements ->
      {RedundantIncrement, SnapshotRedundantMatch}
    [] candidate = RepeatedRedundantSendsAccumulate ->
      {RedundantIncrement, RedundantAccumulation, SnapshotRedundantMatch}
    [] candidate = CurrentDoesNotCountLast ->
      {CurrentOnlyCurrent}
    [] candidate = LastDoesNotCountCurrent ->
      {LastOnlyLast}
    [] candidate = RedundantDoesNotTouchTargets ->
      {RedundantOnlyRedundant}
    [] candidate = SnapshotProjectsCurrent ->
      {SnapshotCurrentMatch}
    [] candidate = SnapshotProjectsLast ->
      {SnapshotLastMatch}
    [] candidate = SnapshotProjectsRedundant ->
      {SnapshotRedundantMatch}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_current" ->
      spec \ {ResetCurrent}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_last" ->
      spec \ {ResetLast}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_redundant" ->
      spec \ {ResetRedundant}
    [] candidate = SetCurrentStoresExact /\ Bug = "current_not_stored" ->
      spec \ {CurrentStored}
    [] candidate = SetCurrentZero /\ Bug = "current_zero_rejected" ->
      spec \ {CurrentZeroStored}
    [] candidate = SetCurrentOverwrites /\
          Bug = "current_accumulates_instead_of_overwrites" ->
      spec \ {CurrentOverwrite}
    [] candidate = ObserveLastStoresExact /\ Bug = "last_not_stored" ->
      spec \ {LastStored}
    [] candidate = ObserveLastZero /\ Bug = "last_zero_rejected" ->
      spec \ {LastZeroStored}
    [] candidate = ObserveLastOverwrites /\
          Bug = "last_accumulates_instead_of_overwrites" ->
      spec \ {LastOverwrite}
    [] candidate = RedundantSendIncrements /\
          Bug = "redundant_not_counted" ->
      spec \ {RedundantIncrement}
    [] candidate = RepeatedRedundantSendsAccumulate /\
          Bug = "repeated_redundant_overwrites_count" ->
      spec \ {RedundantAccumulation}
    [] candidate = CurrentDoesNotCountLast /\
          Bug = "current_counts_last" ->
      (spec \ {CurrentOnlyCurrent}) \cup {LastStored}
    [] candidate = LastDoesNotCountCurrent /\
          Bug = "last_counts_current" ->
      (spec \ {LastOnlyLast}) \cup {CurrentStored}
    [] candidate = RedundantDoesNotTouchTargets /\
          Bug = "redundant_updates_current" ->
      (spec \ {RedundantOnlyRedundant}) \cup {CurrentStored}
    [] candidate = SnapshotProjectsCurrent /\
          Bug = "snapshot_current_mismatch" ->
      spec \ {SnapshotCurrentMatch}
    [] candidate = SnapshotProjectsLast /\
          Bug = "snapshot_last_mismatch" ->
      spec \ {SnapshotLastMatch}
    [] candidate = SnapshotProjectsRedundant /\
          Bug = "snapshot_redundant_mismatch" ->
      spec \ {SnapshotRedundantMatch}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_counters" ->
      spec \ AllResetActions
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 16
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..16

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugResetEmptyKeepsCurrent ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsLast ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsRedundant ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugCurrentNotStored ==
  ImplementationActions(SetCurrentStoresExact) =
    SpecActions(SetCurrentStoresExact)

BugCurrentZeroRejected ==
  ImplementationActions(SetCurrentZero) = SpecActions(SetCurrentZero)

BugCurrentAccumulatesInsteadOfOverwrites ==
  ImplementationActions(SetCurrentOverwrites) =
    SpecActions(SetCurrentOverwrites)

BugLastNotStored ==
  ImplementationActions(ObserveLastStoresExact) =
    SpecActions(ObserveLastStoresExact)

BugLastZeroRejected ==
  ImplementationActions(ObserveLastZero) = SpecActions(ObserveLastZero)

BugLastAccumulatesInsteadOfOverwrites ==
  ImplementationActions(ObserveLastOverwrites) =
    SpecActions(ObserveLastOverwrites)

BugRedundantNotCounted ==
  ImplementationActions(RedundantSendIncrements) =
    SpecActions(RedundantSendIncrements)

BugRepeatedRedundantOverwritesCount ==
  ImplementationActions(RepeatedRedundantSendsAccumulate) =
    SpecActions(RepeatedRedundantSendsAccumulate)

BugCurrentCountsLast ==
  ImplementationActions(CurrentDoesNotCountLast) =
    SpecActions(CurrentDoesNotCountLast)

BugLastCountsCurrent ==
  ImplementationActions(LastDoesNotCountCurrent) =
    SpecActions(LastDoesNotCountCurrent)

BugRedundantUpdatesCurrent ==
  ImplementationActions(RedundantDoesNotTouchTargets) =
    SpecActions(RedundantDoesNotTouchTargets)

BugSnapshotCurrentMismatch ==
  ImplementationActions(SnapshotProjectsCurrent) =
    SpecActions(SnapshotProjectsCurrent)

BugSnapshotLastMismatch ==
  ImplementationActions(SnapshotProjectsLast) =
    SpecActions(SnapshotProjectsLast)

BugSnapshotRedundantMismatch ==
  ImplementationActions(SnapshotProjectsRedundant) =
    SpecActions(SnapshotProjectsRedundant)

BugResetAfterRecordsKeepsCounters ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
====
