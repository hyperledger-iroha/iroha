---- MODULE SumeragiRangePullStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi range-pull status accounting.

This slice captures `inc_blocksync_range_pull_escalation()`,
`inc_blocksync_range_pull_success()`, `inc_blocksync_range_pull_failure()`,
`inc_blocksync_range_pull_candidate_exhausted()`,
`observe_blocksync_range_pull_expiry_streak(...)`, their `snapshot()`
projection, and the range-pull subset of the test-only
`reset_missing_block_fetch_counters_for_tests()` and
`reset_block_sync_counters_for_tests()` helpers from `status.rs`.
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
EscalationRecord == 2
SuccessRecord == 3
FailureRecord == 4
CandidateExhaustedRecord == 5
RepeatedEscalationAccumulates == 6
RepeatedCandidateExhaustedAccumulates == 7
ObserveFirstStreakSetsLastAndMax == 8
ObserveHigherStreakUpdatesMax == 9
ObserveLowerStreakKeepsMax == 10
ObserveZeroStreakRecordsLast == 11
SnapshotProjectsCounters == 12
SnapshotProjectsStreaks == 13
ResetAfterRecordsClears == 14

Candidates == 1..14

ResetCounters == 1
ResetStreaks == 2
IncrementEscalation == 3
IncrementSuccess == 4
IncrementFailure == 5
IncrementCandidateExhausted == 6
SameCounterAccumulates == 7
SetStreakLast == 8
SetStreakMax == 9
MaxPreservesOnLower == 10
SnapshotCountersMatch == 11
SnapshotStreaksMatch == 12

Actions == 1..12

AllResetActions == {ResetCounters, ResetStreaks}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = EscalationRecord ->
      {IncrementEscalation}
    [] candidate = SuccessRecord ->
      {IncrementSuccess}
    [] candidate = FailureRecord ->
      {IncrementFailure}
    [] candidate = CandidateExhaustedRecord ->
      {IncrementCandidateExhausted}
    [] candidate = RepeatedEscalationAccumulates ->
      {IncrementEscalation, SameCounterAccumulates,
       SnapshotCountersMatch}
    [] candidate = RepeatedCandidateExhaustedAccumulates ->
      {IncrementCandidateExhausted, SameCounterAccumulates,
       SnapshotCountersMatch}
    [] candidate = ObserveFirstStreakSetsLastAndMax ->
      {SetStreakLast, SetStreakMax, SnapshotStreaksMatch}
    [] candidate = ObserveHigherStreakUpdatesMax ->
      {SetStreakLast, SetStreakMax, SnapshotStreaksMatch}
    [] candidate = ObserveLowerStreakKeepsMax ->
      {SetStreakLast, MaxPreservesOnLower, SnapshotStreaksMatch}
    [] candidate = ObserveZeroStreakRecordsLast ->
      {SetStreakLast, MaxPreservesOnLower, SnapshotStreaksMatch}
    [] candidate = SnapshotProjectsCounters ->
      {SnapshotCountersMatch}
    [] candidate = SnapshotProjectsStreaks ->
      {SnapshotStreaksMatch}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_counters" ->
      spec \ {ResetCounters}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_streaks" ->
      spec \ {ResetStreaks}
    [] candidate = EscalationRecord /\ Bug = "escalation_not_counted" ->
      spec \ {IncrementEscalation}
    [] candidate = SuccessRecord /\ Bug = "success_not_counted" ->
      spec \ {IncrementSuccess}
    [] candidate = FailureRecord /\ Bug = "failure_not_counted" ->
      spec \ {IncrementFailure}
    [] candidate = CandidateExhaustedRecord /\
          Bug = "candidate_exhausted_not_counted" ->
      spec \ {IncrementCandidateExhausted}
    [] candidate = EscalationRecord /\ Bug = "escalation_counts_success" ->
      (spec \ {IncrementEscalation}) \cup {IncrementSuccess}
    [] candidate = RepeatedEscalationAccumulates /\
          Bug = "repeated_escalation_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotCountersMatch}
    [] candidate = RepeatedCandidateExhaustedAccumulates /\
          Bug = "repeated_candidate_exhausted_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotCountersMatch}
    [] candidate = ObserveFirstStreakSetsLastAndMax /\
          Bug = "first_streak_last_not_set" ->
      spec \ {SetStreakLast}
    [] candidate = ObserveFirstStreakSetsLastAndMax /\
          Bug = "first_streak_max_not_set" ->
      spec \ {SetStreakMax}
    [] candidate = ObserveHigherStreakUpdatesMax /\
          Bug = "higher_streak_max_not_updated" ->
      spec \ {SetStreakMax}
    [] candidate = ObserveHigherStreakUpdatesMax /\
          Bug = "higher_streak_last_not_updated" ->
      spec \ {SetStreakLast}
    [] candidate = ObserveLowerStreakKeepsMax /\
          Bug = "lower_streak_decreases_max" ->
      (spec \ {MaxPreservesOnLower}) \cup {SetStreakMax}
    [] candidate = ObserveLowerStreakKeepsMax /\
          Bug = "lower_streak_last_not_updated" ->
      spec \ {SetStreakLast}
    [] candidate = ObserveZeroStreakRecordsLast /\
          Bug = "zero_streak_ignored" ->
      spec \ {SetStreakLast}
    [] candidate = SnapshotProjectsCounters /\
          Bug = "snapshot_counters_mismatch" ->
      spec \ {SnapshotCountersMatch}
    [] candidate = SnapshotProjectsStreaks /\
          Bug = "snapshot_streaks_mismatch" ->
      spec \ {SnapshotStreaksMatch}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_counters" ->
      spec \ {ResetCounters}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_streaks" ->
      spec \ {ResetStreaks}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 14
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..14

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

RangePullStatusExactness ==
  Safety

RangePullStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RangePullStatusExactness

BugResetEmptyKeepsCounters ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsStreaks ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugEscalationNotCounted ==
  ImplementationActions(EscalationRecord) =
    SpecActions(EscalationRecord)

BugSuccessNotCounted ==
  ImplementationActions(SuccessRecord) = SpecActions(SuccessRecord)

BugFailureNotCounted ==
  ImplementationActions(FailureRecord) = SpecActions(FailureRecord)

BugCandidateExhaustedNotCounted ==
  ImplementationActions(CandidateExhaustedRecord) =
    SpecActions(CandidateExhaustedRecord)

BugEscalationCountsSuccess ==
  ImplementationActions(EscalationRecord) =
    SpecActions(EscalationRecord)

BugRepeatedEscalationOverwritesCount ==
  ImplementationActions(RepeatedEscalationAccumulates) =
    SpecActions(RepeatedEscalationAccumulates)

BugRepeatedCandidateExhaustedOverwritesCount ==
  ImplementationActions(RepeatedCandidateExhaustedAccumulates) =
    SpecActions(RepeatedCandidateExhaustedAccumulates)

BugFirstStreakLastNotSet ==
  ImplementationActions(ObserveFirstStreakSetsLastAndMax) =
    SpecActions(ObserveFirstStreakSetsLastAndMax)

BugFirstStreakMaxNotSet ==
  ImplementationActions(ObserveFirstStreakSetsLastAndMax) =
    SpecActions(ObserveFirstStreakSetsLastAndMax)

BugHigherStreakMaxNotUpdated ==
  ImplementationActions(ObserveHigherStreakUpdatesMax) =
    SpecActions(ObserveHigherStreakUpdatesMax)

BugHigherStreakLastNotUpdated ==
  ImplementationActions(ObserveHigherStreakUpdatesMax) =
    SpecActions(ObserveHigherStreakUpdatesMax)

BugLowerStreakDecreasesMax ==
  ImplementationActions(ObserveLowerStreakKeepsMax) =
    SpecActions(ObserveLowerStreakKeepsMax)

BugLowerStreakLastNotUpdated ==
  ImplementationActions(ObserveLowerStreakKeepsMax) =
    SpecActions(ObserveLowerStreakKeepsMax)

BugZeroStreakIgnored ==
  ImplementationActions(ObserveZeroStreakRecordsLast) =
    SpecActions(ObserveZeroStreakRecordsLast)

BugSnapshotCountersMismatch ==
  ImplementationActions(SnapshotProjectsCounters) =
    SpecActions(SnapshotProjectsCounters)

BugSnapshotStreaksMismatch ==
  ImplementationActions(SnapshotProjectsStreaks) =
    SpecActions(SnapshotProjectsStreaks)

BugResetAfterRecordsKeepsCounters ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsStreaks ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
====
