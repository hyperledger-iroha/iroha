---- MODULE SumeragiRosterRecoveryStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi roster-recovery status accounting.

This slice captures `inc_consensus_roster_unavailable_detected()`,
`inc_consensus_roster_unavailable_election_attempt()`,
`inc_consensus_roster_unavailable_election_success()`,
`inc_consensus_roster_unavailable_wait_candidates()`,
`inc_consensus_catchup_isolation_enter()`,
`inc_consensus_catchup_isolation_success()`,
`inc_consensus_catchup_rejoin()`,
`set_consensus_roster_recovery_state(...)`,
`set_consensus_roster_recovery_dwell_ms(...)`, their `snapshot()` projection,
and the relevant subset of the test-only
`reset_missing_block_fetch_counters_for_tests()` helper from `status.rs`.
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
DetectedRecord == 2
ElectionAttemptRecord == 3
ElectionSuccessRecord == 4
WaitCandidatesRecord == 5
CatchupEnterRecord == 6
CatchupSuccessRecord == 7
CatchupRejoinRecord == 8
RepeatedDetectedAccumulates == 9
RepeatedCatchupEnterAccumulates == 10
SetStateRecord == 11
StateOverwrites == 12
SetDwellRecord == 13
DwellOverwrites == 14
SnapshotProjectsRosterCounters == 15
SnapshotProjectsCatchupCounters == 16
SnapshotProjectsState == 17
SnapshotProjectsDwell == 18
ResetAfterRecordsClears == 19

Candidates == 1..19

ResetCounters == 1
ResetState == 2
ResetDwell == 3
IncrementDetected == 4
IncrementElectionAttempt == 5
IncrementElectionSuccess == 6
IncrementWaitCandidates == 7
IncrementCatchupEnter == 8
IncrementCatchupSuccess == 9
IncrementCatchupRejoin == 10
SameCounterAccumulates == 11
SetRecoveryState == 12
StateOverwritesAction == 13
SetDwellMap == 14
DwellOverwritesAction == 15
SnapshotRosterCountersMatch == 16
SnapshotCatchupCountersMatch == 17
SnapshotStateMatches == 18
SnapshotDwellMatches == 19

Actions == 1..19

AllResetActions == {ResetCounters, ResetState, ResetDwell}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = DetectedRecord ->
      {IncrementDetected}
    [] candidate = ElectionAttemptRecord ->
      {IncrementElectionAttempt}
    [] candidate = ElectionSuccessRecord ->
      {IncrementElectionSuccess}
    [] candidate = WaitCandidatesRecord ->
      {IncrementWaitCandidates}
    [] candidate = CatchupEnterRecord ->
      {IncrementCatchupEnter}
    [] candidate = CatchupSuccessRecord ->
      {IncrementCatchupSuccess}
    [] candidate = CatchupRejoinRecord ->
      {IncrementCatchupRejoin}
    [] candidate = RepeatedDetectedAccumulates ->
      {IncrementDetected, SameCounterAccumulates,
       SnapshotRosterCountersMatch}
    [] candidate = RepeatedCatchupEnterAccumulates ->
      {IncrementCatchupEnter, SameCounterAccumulates,
       SnapshotCatchupCountersMatch}
    [] candidate = SetStateRecord ->
      {SetRecoveryState, SnapshotStateMatches}
    [] candidate = StateOverwrites ->
      {SetRecoveryState, StateOverwritesAction, SnapshotStateMatches}
    [] candidate = SetDwellRecord ->
      {SetDwellMap, SnapshotDwellMatches}
    [] candidate = DwellOverwrites ->
      {SetDwellMap, DwellOverwritesAction, SnapshotDwellMatches}
    [] candidate = SnapshotProjectsRosterCounters ->
      {SnapshotRosterCountersMatch}
    [] candidate = SnapshotProjectsCatchupCounters ->
      {SnapshotCatchupCountersMatch}
    [] candidate = SnapshotProjectsState ->
      {SnapshotStateMatches}
    [] candidate = SnapshotProjectsDwell ->
      {SnapshotDwellMatches}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_counters" ->
      spec \ {ResetCounters}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_state" ->
      spec \ {ResetState}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_dwell" ->
      spec \ {ResetDwell}
    [] candidate = DetectedRecord /\ Bug = "detected_not_counted" ->
      spec \ {IncrementDetected}
    [] candidate = ElectionAttemptRecord /\
          Bug = "election_attempt_not_counted" ->
      spec \ {IncrementElectionAttempt}
    [] candidate = ElectionSuccessRecord /\
          Bug = "election_success_not_counted" ->
      spec \ {IncrementElectionSuccess}
    [] candidate = WaitCandidatesRecord /\
          Bug = "wait_candidates_not_counted" ->
      spec \ {IncrementWaitCandidates}
    [] candidate = CatchupEnterRecord /\
          Bug = "catchup_enter_not_counted" ->
      spec \ {IncrementCatchupEnter}
    [] candidate = CatchupSuccessRecord /\
          Bug = "catchup_success_not_counted" ->
      spec \ {IncrementCatchupSuccess}
    [] candidate = CatchupRejoinRecord /\
          Bug = "catchup_rejoin_not_counted" ->
      spec \ {IncrementCatchupRejoin}
    [] candidate = DetectedRecord /\ Bug = "detected_counts_attempt" ->
      (spec \ {IncrementDetected}) \cup {IncrementElectionAttempt}
    [] candidate = CatchupEnterRecord /\
          Bug = "catchup_enter_counts_success" ->
      (spec \ {IncrementCatchupEnter}) \cup {IncrementCatchupSuccess}
    [] candidate = RepeatedDetectedAccumulates /\
          Bug = "repeated_detected_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotRosterCountersMatch}
    [] candidate = RepeatedCatchupEnterAccumulates /\
          Bug = "repeated_catchup_enter_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotCatchupCountersMatch}
    [] candidate = SetStateRecord /\ Bug = "state_not_set" ->
      spec \ {SetRecoveryState}
    [] candidate = StateOverwrites /\ Bug = "state_not_overwritten" ->
      spec \ {SetRecoveryState, StateOverwritesAction,
              SnapshotStateMatches}
    [] candidate = SetDwellRecord /\ Bug = "dwell_not_set" ->
      spec \ {SetDwellMap}
    [] candidate = DwellOverwrites /\ Bug = "dwell_not_overwritten" ->
      spec \ {SetDwellMap, DwellOverwritesAction, SnapshotDwellMatches}
    [] candidate = SnapshotProjectsRosterCounters /\
          Bug = "snapshot_roster_counters_mismatch" ->
      spec \ {SnapshotRosterCountersMatch}
    [] candidate = SnapshotProjectsCatchupCounters /\
          Bug = "snapshot_catchup_counters_mismatch" ->
      spec \ {SnapshotCatchupCountersMatch}
    [] candidate = SnapshotProjectsState /\
          Bug = "snapshot_state_mismatch" ->
      spec \ {SnapshotStateMatches}
    [] candidate = SnapshotProjectsDwell /\
          Bug = "snapshot_dwell_mismatch" ->
      spec \ {SnapshotDwellMatches}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_counters" ->
      spec \ {ResetCounters}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_state" ->
      spec \ {ResetState}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_dwell" ->
      spec \ {ResetDwell}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 19
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..19

RosterRecoveryStatusActionsMatchSpec ==
  /\ ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)
  /\ ImplementationActions(DetectedRecord) = SpecActions(DetectedRecord)
  /\ ImplementationActions(ElectionAttemptRecord) =
       SpecActions(ElectionAttemptRecord)
  /\ ImplementationActions(ElectionSuccessRecord) =
       SpecActions(ElectionSuccessRecord)
  /\ ImplementationActions(WaitCandidatesRecord) =
       SpecActions(WaitCandidatesRecord)
  /\ ImplementationActions(CatchupEnterRecord) =
       SpecActions(CatchupEnterRecord)
  /\ ImplementationActions(CatchupSuccessRecord) =
       SpecActions(CatchupSuccessRecord)
  /\ ImplementationActions(CatchupRejoinRecord) =
       SpecActions(CatchupRejoinRecord)
  /\ ImplementationActions(RepeatedDetectedAccumulates) =
       SpecActions(RepeatedDetectedAccumulates)
  /\ ImplementationActions(RepeatedCatchupEnterAccumulates) =
       SpecActions(RepeatedCatchupEnterAccumulates)
  /\ ImplementationActions(SetStateRecord) = SpecActions(SetStateRecord)
  /\ ImplementationActions(StateOverwrites) = SpecActions(StateOverwrites)
  /\ ImplementationActions(SetDwellRecord) = SpecActions(SetDwellRecord)
  /\ ImplementationActions(DwellOverwrites) = SpecActions(DwellOverwrites)
  /\ ImplementationActions(SnapshotProjectsRosterCounters) =
       SpecActions(SnapshotProjectsRosterCounters)
  /\ ImplementationActions(SnapshotProjectsCatchupCounters) =
       SpecActions(SnapshotProjectsCatchupCounters)
  /\ ImplementationActions(SnapshotProjectsState) =
       SpecActions(SnapshotProjectsState)
  /\ ImplementationActions(SnapshotProjectsDwell) =
       SpecActions(SnapshotProjectsDwell)
  /\ ImplementationActions(ResetAfterRecordsClears) =
       SpecActions(ResetAfterRecordsClears)

RosterRecoveryStatusExactness ==
  /\ RosterRecoveryStatusActionsMatchSpec

Safety ==
  RosterRecoveryStatusExactness

RosterRecoveryStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RosterRecoveryStatusExactness

BugResetEmptyKeepsCounters ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsState ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsDwell ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugDetectedNotCounted ==
  ImplementationActions(DetectedRecord) = SpecActions(DetectedRecord)

BugElectionAttemptNotCounted ==
  ImplementationActions(ElectionAttemptRecord) =
    SpecActions(ElectionAttemptRecord)

BugElectionSuccessNotCounted ==
  ImplementationActions(ElectionSuccessRecord) =
    SpecActions(ElectionSuccessRecord)

BugWaitCandidatesNotCounted ==
  ImplementationActions(WaitCandidatesRecord) =
    SpecActions(WaitCandidatesRecord)

BugCatchupEnterNotCounted ==
  ImplementationActions(CatchupEnterRecord) =
    SpecActions(CatchupEnterRecord)

BugCatchupSuccessNotCounted ==
  ImplementationActions(CatchupSuccessRecord) =
    SpecActions(CatchupSuccessRecord)

BugCatchupRejoinNotCounted ==
  ImplementationActions(CatchupRejoinRecord) =
    SpecActions(CatchupRejoinRecord)

BugDetectedCountsAttempt ==
  ImplementationActions(DetectedRecord) = SpecActions(DetectedRecord)

BugCatchupEnterCountsSuccess ==
  ImplementationActions(CatchupEnterRecord) =
    SpecActions(CatchupEnterRecord)

BugRepeatedDetectedOverwritesCount ==
  ImplementationActions(RepeatedDetectedAccumulates) =
    SpecActions(RepeatedDetectedAccumulates)

BugRepeatedCatchupEnterOverwritesCount ==
  ImplementationActions(RepeatedCatchupEnterAccumulates) =
    SpecActions(RepeatedCatchupEnterAccumulates)

BugStateNotSet ==
  ImplementationActions(SetStateRecord) = SpecActions(SetStateRecord)

BugStateNotOverwritten ==
  ImplementationActions(StateOverwrites) = SpecActions(StateOverwrites)

BugDwellNotSet ==
  ImplementationActions(SetDwellRecord) = SpecActions(SetDwellRecord)

BugDwellNotOverwritten ==
  ImplementationActions(DwellOverwrites) = SpecActions(DwellOverwrites)

BugSnapshotRosterCountersMismatch ==
  ImplementationActions(SnapshotProjectsRosterCounters) =
    SpecActions(SnapshotProjectsRosterCounters)

BugSnapshotCatchupCountersMismatch ==
  ImplementationActions(SnapshotProjectsCatchupCounters) =
    SpecActions(SnapshotProjectsCatchupCounters)

BugSnapshotStateMismatch ==
  ImplementationActions(SnapshotProjectsState) =
    SpecActions(SnapshotProjectsState)

BugSnapshotDwellMismatch ==
  ImplementationActions(SnapshotProjectsDwell) =
    SpecActions(SnapshotProjectsDwell)

BugResetAfterRecordsKeepsCounters ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsState ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsDwell ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
====
