---- MODULE SumeragiViewChangeCauseStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi view-change cause status accounting.

This slice captures `record_view_change_cause(...)`, the internal
`view_change_cause_snapshot()` projection used by `snapshot()`, and the
test-only `reset_view_change_cause_counters_for_tests()` helper from
`status.rs`: per-cause counters, unknown-label behavior, generic and
per-cause timestamp updates, last-cause tracking, top-level status projection,
and reset semantics.
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
CommitFailureRecord == 2
QuorumTimeoutRecord == 3
StakeQuorumTimeoutRecord == 4
RosterUnavailableRecord == 5
DaGateRecord == 6
CensorshipEvidenceRecord == 7
MissingPayloadRecord == 8
MissingQcRecord == 9
ValidationRejectRecord == 10
UnknownCauseRecord == 11
RepeatedSameCauseAccumulates == 12
DifferentCausesIndependent == 13
LastCauseUpdates == 14
LastTimestampSet == 15
PerCauseTimestampSet == 16
SnapshotProjectsCounters == 17
SnapshotProjectsTimestamps == 18
SnapshotProjectsLastCause == 19
TopLevelSnapshotIncludesCauses == 20
ResetAfterRecordsClears == 21

Candidates == 1..21

ResetCounters == 1
ResetLastCause == 2
ResetLastTimestamp == 3
ResetPerCauseTimestamps == 4
IncrementCommitFailure == 5
IncrementQuorumTimeout == 6
IncrementStakeQuorumTimeout == 7
IncrementRosterUnavailable == 8
IncrementDaGate == 9
IncrementCensorshipEvidence == 10
IncrementMissingPayload == 11
IncrementMissingQc == 12
IncrementValidationReject == 13
NoKnownCounterIncrement == 14
NoPerCauseTimestamp == 15
SameCauseAccumulates == 16
BucketsIndependent == 17
SetLastCause == 18
SetLastTimestamp == 19
LastTimestampPositive == 20
SetCommitFailureTimestamp == 21
SetQuorumTimeoutTimestamp == 22
SetStakeQuorumTimeoutTimestamp == 23
SetRosterUnavailableTimestamp == 24
SetDaGateTimestamp == 25
SetCensorshipEvidenceTimestamp == 26
SetMissingPayloadTimestamp == 27
SetMissingQcTimestamp == 28
SetValidationRejectTimestamp == 29
PerCauseTimestampPositive == 30
SnapshotCountersMatch == 31
SnapshotTimestampsMatch == 32
SnapshotLastCauseMatches == 33
TopLevelViewChangeCausesMatch == 34
SnapshotPreservesCounts == 35

Actions == 1..35

AllResetActions ==
  {ResetCounters, ResetLastCause, ResetLastTimestamp,
   ResetPerCauseTimestamps}

CommonRecordActions ==
  {SetLastCause, SetLastTimestamp, LastTimestampPositive,
   PerCauseTimestampPositive}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = CommitFailureRecord ->
      CommonRecordActions \cup
        {IncrementCommitFailure, SetCommitFailureTimestamp}
    [] candidate = QuorumTimeoutRecord ->
      CommonRecordActions \cup
        {IncrementQuorumTimeout, SetQuorumTimeoutTimestamp}
    [] candidate = StakeQuorumTimeoutRecord ->
      CommonRecordActions \cup
        {IncrementStakeQuorumTimeout, SetStakeQuorumTimeoutTimestamp}
    [] candidate = RosterUnavailableRecord ->
      CommonRecordActions \cup
        {IncrementRosterUnavailable, SetRosterUnavailableTimestamp}
    [] candidate = DaGateRecord ->
      CommonRecordActions \cup {IncrementDaGate, SetDaGateTimestamp}
    [] candidate = CensorshipEvidenceRecord ->
      CommonRecordActions \cup
        {IncrementCensorshipEvidence, SetCensorshipEvidenceTimestamp}
    [] candidate = MissingPayloadRecord ->
      CommonRecordActions \cup
        {IncrementMissingPayload, SetMissingPayloadTimestamp}
    [] candidate = MissingQcRecord ->
      CommonRecordActions \cup {IncrementMissingQc, SetMissingQcTimestamp}
    [] candidate = ValidationRejectRecord ->
      CommonRecordActions \cup
        {IncrementValidationReject, SetValidationRejectTimestamp}
    [] candidate = UnknownCauseRecord ->
      {NoKnownCounterIncrement, NoPerCauseTimestamp, SetLastCause,
       SetLastTimestamp, LastTimestampPositive}
    [] candidate = RepeatedSameCauseAccumulates ->
      {SameCauseAccumulates, SetLastCause, SetLastTimestamp,
       SetMissingQcTimestamp, SnapshotPreservesCounts}
    [] candidate = DifferentCausesIndependent ->
      {BucketsIndependent, SetLastCause, SnapshotPreservesCounts}
    [] candidate = LastCauseUpdates ->
      {SetLastCause}
    [] candidate = LastTimestampSet ->
      {SetLastTimestamp, LastTimestampPositive}
    [] candidate = PerCauseTimestampSet ->
      {SetMissingPayloadTimestamp, PerCauseTimestampPositive}
    [] candidate = SnapshotProjectsCounters ->
      {SnapshotCountersMatch, SnapshotPreservesCounts}
    [] candidate = SnapshotProjectsTimestamps ->
      {SnapshotTimestampsMatch}
    [] candidate = SnapshotProjectsLastCause ->
      {SnapshotLastCauseMatches}
    [] candidate = TopLevelSnapshotIncludesCauses ->
      {TopLevelViewChangeCausesMatch}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_counters" ->
      spec \ {ResetCounters}
    [] candidate = CommitFailureRecord /\
          Bug = "commit_failure_not_counted" ->
      spec \ {IncrementCommitFailure}
    [] candidate = QuorumTimeoutRecord /\
          Bug = "quorum_timeout_not_counted" ->
      spec \ {IncrementQuorumTimeout}
    [] candidate = StakeQuorumTimeoutRecord /\
          Bug = "stake_quorum_timeout_not_counted" ->
      spec \ {IncrementStakeQuorumTimeout}
    [] candidate = RosterUnavailableRecord /\
          Bug = "roster_unavailable_not_counted" ->
      spec \ {IncrementRosterUnavailable}
    [] candidate = DaGateRecord /\ Bug = "da_gate_not_counted" ->
      spec \ {IncrementDaGate}
    [] candidate = CensorshipEvidenceRecord /\
          Bug = "censorship_evidence_not_counted" ->
      spec \ {IncrementCensorshipEvidence}
    [] candidate = MissingPayloadRecord /\
          Bug = "missing_payload_not_counted" ->
      spec \ {IncrementMissingPayload}
    [] candidate = MissingQcRecord /\ Bug = "missing_qc_not_counted" ->
      spec \ {IncrementMissingQc}
    [] candidate = ValidationRejectRecord /\
          Bug = "validation_reject_not_counted" ->
      spec \ {IncrementValidationReject}
    [] candidate = CommitFailureRecord /\ Bug = "known_cause_wrong_bucket" ->
      (spec \ {IncrementCommitFailure}) \cup {IncrementQuorumTimeout}
    [] candidate = UnknownCauseRecord /\
          Bug = "unknown_increments_known_bucket" ->
      (spec \ {NoKnownCounterIncrement}) \cup {IncrementCommitFailure}
    [] candidate = UnknownCauseRecord /\
          Bug = "unknown_sets_known_timestamp" ->
      (spec \ {NoPerCauseTimestamp}) \cup {SetCommitFailureTimestamp}
    [] candidate = RepeatedSameCauseAccumulates /\
          Bug = "same_cause_overwrites_count" ->
      spec \ {SameCauseAccumulates, SnapshotPreservesCounts}
    [] candidate = DifferentCausesIndependent /\
          Bug = "different_causes_collide" ->
      (spec \ {BucketsIndependent, SnapshotPreservesCounts}) \cup
        {SameCauseAccumulates}
    [] candidate = LastCauseUpdates /\ Bug = "last_cause_not_updated" ->
      spec \ {SetLastCause}
    [] candidate = LastTimestampSet /\ Bug = "last_timestamp_zero" ->
      spec \ {SetLastTimestamp, LastTimestampPositive}
    [] candidate = PerCauseTimestampSet /\
          Bug = "per_cause_timestamp_missing" ->
      spec \ {SetMissingPayloadTimestamp, PerCauseTimestampPositive}
    [] candidate = PerCauseTimestampSet /\
          Bug = "wrong_per_cause_timestamp" ->
      (spec \ {SetMissingPayloadTimestamp}) \cup {SetMissingQcTimestamp}
    [] candidate = SnapshotProjectsCounters /\
          Bug = "snapshot_counts_mismatch" ->
      spec \ {SnapshotCountersMatch}
    [] candidate = SnapshotProjectsTimestamps /\
          Bug = "snapshot_timestamp_mismatch" ->
      spec \ {SnapshotTimestampsMatch}
    [] candidate = SnapshotProjectsLastCause /\
          Bug = "snapshot_last_cause_mismatch" ->
      spec \ {SnapshotLastCauseMatches}
    [] candidate = TopLevelSnapshotIncludesCauses /\
          Bug = "top_level_snapshot_drops_causes" ->
      spec \ {TopLevelViewChangeCausesMatch}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_counters" ->
      spec \ {ResetCounters}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_last" ->
      spec \ {ResetLastCause, ResetLastTimestamp, ResetPerCauseTimestamps}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  \/ /\ checked < 21
     /\ checked' = checked + 1
  \/ /\ checked = 21
     /\ checked' = checked

TypeInvariant ==
  checked \in 0..21

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugResetEmptyKeepsCounters ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugCommitFailureNotCounted ==
  ImplementationActions(CommitFailureRecord) = SpecActions(CommitFailureRecord)

BugQuorumTimeoutNotCounted ==
  ImplementationActions(QuorumTimeoutRecord) = SpecActions(QuorumTimeoutRecord)

BugStakeQuorumTimeoutNotCounted ==
  ImplementationActions(StakeQuorumTimeoutRecord) =
    SpecActions(StakeQuorumTimeoutRecord)

BugRosterUnavailableNotCounted ==
  ImplementationActions(RosterUnavailableRecord) =
    SpecActions(RosterUnavailableRecord)

BugDaGateNotCounted ==
  ImplementationActions(DaGateRecord) = SpecActions(DaGateRecord)

BugCensorshipEvidenceNotCounted ==
  ImplementationActions(CensorshipEvidenceRecord) =
    SpecActions(CensorshipEvidenceRecord)

BugMissingPayloadNotCounted ==
  ImplementationActions(MissingPayloadRecord) =
    SpecActions(MissingPayloadRecord)

BugMissingQcNotCounted ==
  ImplementationActions(MissingQcRecord) = SpecActions(MissingQcRecord)

BugValidationRejectNotCounted ==
  ImplementationActions(ValidationRejectRecord) =
    SpecActions(ValidationRejectRecord)

BugKnownCauseWrongBucket ==
  ImplementationActions(CommitFailureRecord) = SpecActions(CommitFailureRecord)

BugUnknownIncrementsKnownBucket ==
  ImplementationActions(UnknownCauseRecord) = SpecActions(UnknownCauseRecord)

BugUnknownSetsKnownTimestamp ==
  ImplementationActions(UnknownCauseRecord) = SpecActions(UnknownCauseRecord)

BugSameCauseOverwritesCount ==
  ImplementationActions(RepeatedSameCauseAccumulates) =
    SpecActions(RepeatedSameCauseAccumulates)

BugDifferentCausesCollide ==
  ImplementationActions(DifferentCausesIndependent) =
    SpecActions(DifferentCausesIndependent)

BugLastCauseNotUpdated ==
  ImplementationActions(LastCauseUpdates) = SpecActions(LastCauseUpdates)

BugLastTimestampZero ==
  ImplementationActions(LastTimestampSet) = SpecActions(LastTimestampSet)

BugPerCauseTimestampMissing ==
  ImplementationActions(PerCauseTimestampSet) =
    SpecActions(PerCauseTimestampSet)

BugWrongPerCauseTimestamp ==
  ImplementationActions(PerCauseTimestampSet) =
    SpecActions(PerCauseTimestampSet)

BugSnapshotCountsMismatch ==
  ImplementationActions(SnapshotProjectsCounters) =
    SpecActions(SnapshotProjectsCounters)

BugSnapshotTimestampMismatch ==
  ImplementationActions(SnapshotProjectsTimestamps) =
    SpecActions(SnapshotProjectsTimestamps)

BugSnapshotLastCauseMismatch ==
  ImplementationActions(SnapshotProjectsLastCause) =
    SpecActions(SnapshotProjectsLastCause)

BugTopLevelSnapshotDropsCauses ==
  ImplementationActions(TopLevelSnapshotIncludesCauses) =
    SpecActions(TopLevelSnapshotIncludesCauses)

BugResetAfterRecordsKeepsCounters ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsLast ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

AllViewChangeCauseCandidatesMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

ResetEmptyClearsAllAnchors ==
  /\ ResetCounters \in ImplementationActions(ResetEmpty)
  /\ ResetLastCause \in ImplementationActions(ResetEmpty)
  /\ ResetLastTimestamp \in ImplementationActions(ResetEmpty)
  /\ ResetPerCauseTimestamps \in ImplementationActions(ResetEmpty)

KnownCauseBucketAnchors ==
  /\ IncrementCommitFailure \in ImplementationActions(CommitFailureRecord)
  /\ ~(IncrementQuorumTimeout \in ImplementationActions(CommitFailureRecord))
  /\ IncrementQuorumTimeout \in ImplementationActions(QuorumTimeoutRecord)
  /\ IncrementStakeQuorumTimeout \in
       ImplementationActions(StakeQuorumTimeoutRecord)
  /\ IncrementRosterUnavailable \in
       ImplementationActions(RosterUnavailableRecord)
  /\ IncrementDaGate \in ImplementationActions(DaGateRecord)
  /\ IncrementCensorshipEvidence \in
       ImplementationActions(CensorshipEvidenceRecord)
  /\ IncrementMissingPayload \in
       ImplementationActions(MissingPayloadRecord)
  /\ IncrementMissingQc \in ImplementationActions(MissingQcRecord)
  /\ IncrementValidationReject \in
       ImplementationActions(ValidationRejectRecord)

UnknownCauseAnchors ==
  /\ NoKnownCounterIncrement \in ImplementationActions(UnknownCauseRecord)
  /\ NoPerCauseTimestamp \in ImplementationActions(UnknownCauseRecord)
  /\ SetLastCause \in ImplementationActions(UnknownCauseRecord)
  /\ SetLastTimestamp \in ImplementationActions(UnknownCauseRecord)
  /\ LastTimestampPositive \in ImplementationActions(UnknownCauseRecord)
  /\ ~(IncrementCommitFailure \in ImplementationActions(UnknownCauseRecord))
  /\ ~(SetCommitFailureTimestamp \in
       ImplementationActions(UnknownCauseRecord))

AccumulationAnchors ==
  /\ SameCauseAccumulates \in
       ImplementationActions(RepeatedSameCauseAccumulates)
  /\ SnapshotPreservesCounts \in
       ImplementationActions(RepeatedSameCauseAccumulates)
  /\ SetMissingQcTimestamp \in
       ImplementationActions(RepeatedSameCauseAccumulates)
  /\ BucketsIndependent \in
       ImplementationActions(DifferentCausesIndependent)
  /\ SnapshotPreservesCounts \in
       ImplementationActions(DifferentCausesIndependent)
  /\ ~(SameCauseAccumulates \in
       ImplementationActions(DifferentCausesIndependent))

LastCauseAndTimestampAnchors ==
  /\ SetLastCause \in ImplementationActions(LastCauseUpdates)
  /\ SetLastTimestamp \in ImplementationActions(LastTimestampSet)
  /\ LastTimestampPositive \in ImplementationActions(LastTimestampSet)

PerCauseTimestampAnchors ==
  /\ SetMissingPayloadTimestamp \in
       ImplementationActions(PerCauseTimestampSet)
  /\ PerCauseTimestampPositive \in
       ImplementationActions(PerCauseTimestampSet)
  /\ ~(SetMissingQcTimestamp \in
       ImplementationActions(PerCauseTimestampSet))

SnapshotProjectionAnchors ==
  /\ SnapshotCountersMatch \in ImplementationActions(SnapshotProjectsCounters)
  /\ SnapshotPreservesCounts \in
       ImplementationActions(SnapshotProjectsCounters)
  /\ SnapshotTimestampsMatch \in
       ImplementationActions(SnapshotProjectsTimestamps)
  /\ SnapshotLastCauseMatches \in
       ImplementationActions(SnapshotProjectsLastCause)
  /\ TopLevelViewChangeCausesMatch \in
       ImplementationActions(TopLevelSnapshotIncludesCauses)

ResetAfterRecordsClearsAllAnchors ==
  /\ ResetCounters \in ImplementationActions(ResetAfterRecordsClears)
  /\ ResetLastCause \in ImplementationActions(ResetAfterRecordsClears)
  /\ ResetLastTimestamp \in ImplementationActions(ResetAfterRecordsClears)
  /\ ResetPerCauseTimestamps \in
       ImplementationActions(ResetAfterRecordsClears)

SafetyAnchors ==
  /\ AllViewChangeCauseCandidatesMatchSpec
  /\ ResetEmptyClearsAllAnchors
  /\ KnownCauseBucketAnchors
  /\ UnknownCauseAnchors
  /\ AccumulationAnchors
  /\ LastCauseAndTimestampAnchors
  /\ PerCauseTimestampAnchors
  /\ SnapshotProjectionAnchors
  /\ ResetAfterRecordsClearsAllAnchors

=============================================================================
