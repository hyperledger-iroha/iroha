---- MODULE SumeragiQcRebuildStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi QC rebuild status counters.

This slice captures the status accounting helpers in `status.rs` for
`inc_qc_rebuild_attempts()`, `inc_qc_rebuild_successes()`,
`inc_qc_missing_votes_accepted()`, `inc_qc_quorum_without_qc()`, their
top-level `snapshot()` projection, and the test-only
`reset_qc_rebuild_counters_for_tests()` helper. The separate
`inc_qc_missing_payload_aggressive_fetch()` counter is covered by
`SumeragiRecoveryStatusCountersGate`.
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
AttemptIncrements == 2
SuccessIncrements == 3
MissingVotesAcceptedIncrements == 4
QuorumWithoutQcIncrements == 5
AttemptDoesNotCountSuccess == 6
SuccessDoesNotCountAttempt == 7
MissingVotesDoesNotCountQuorum == 8
QuorumDoesNotCountMissingVotes == 9
RepeatedAttemptsAccumulate == 10
RepeatedSuccessesAccumulate == 11
RepeatedMissingVotesAccumulate == 12
RepeatedQuorumWithoutQcAccumulate == 13
SnapshotProjectsAttempts == 14
SnapshotProjectsSuccesses == 15
SnapshotProjectsMissingVotes == 16
SnapshotProjectsQuorumWithoutQc == 17
ResetAfterRecordsClears == 18

Candidates == 1..18

ResetAttempts == 1
ResetSuccesses == 2
ResetMissingVotes == 3
ResetQuorumWithoutQc == 4
IncrementAttempt == 5
IncrementSuccess == 6
IncrementMissingVotes == 7
IncrementQuorumWithoutQc == 8
AttemptOnlyAttempts == 9
SuccessOnlySuccesses == 10
MissingVotesOnlyMissingVotes == 11
QuorumOnlyQuorum == 12
AttemptAccumulation == 13
SuccessAccumulation == 14
MissingVotesAccumulation == 15
QuorumAccumulation == 16
SnapshotAttemptsMatch == 17
SnapshotSuccessesMatch == 18
SnapshotMissingVotesMatch == 19
SnapshotQuorumWithoutQcMatch == 20

Actions == 1..20

AllResetActions ==
  {ResetAttempts, ResetSuccesses, ResetMissingVotes, ResetQuorumWithoutQc}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = AttemptIncrements ->
      {IncrementAttempt, SnapshotAttemptsMatch}
    [] candidate = SuccessIncrements ->
      {IncrementSuccess, SnapshotSuccessesMatch}
    [] candidate = MissingVotesAcceptedIncrements ->
      {IncrementMissingVotes, SnapshotMissingVotesMatch}
    [] candidate = QuorumWithoutQcIncrements ->
      {IncrementQuorumWithoutQc, SnapshotQuorumWithoutQcMatch}
    [] candidate = AttemptDoesNotCountSuccess ->
      {AttemptOnlyAttempts}
    [] candidate = SuccessDoesNotCountAttempt ->
      {SuccessOnlySuccesses}
    [] candidate = MissingVotesDoesNotCountQuorum ->
      {MissingVotesOnlyMissingVotes}
    [] candidate = QuorumDoesNotCountMissingVotes ->
      {QuorumOnlyQuorum}
    [] candidate = RepeatedAttemptsAccumulate ->
      {IncrementAttempt, AttemptAccumulation, SnapshotAttemptsMatch}
    [] candidate = RepeatedSuccessesAccumulate ->
      {IncrementSuccess, SuccessAccumulation, SnapshotSuccessesMatch}
    [] candidate = RepeatedMissingVotesAccumulate ->
      {IncrementMissingVotes, MissingVotesAccumulation,
       SnapshotMissingVotesMatch}
    [] candidate = RepeatedQuorumWithoutQcAccumulate ->
      {IncrementQuorumWithoutQc, QuorumAccumulation,
       SnapshotQuorumWithoutQcMatch}
    [] candidate = SnapshotProjectsAttempts ->
      {SnapshotAttemptsMatch}
    [] candidate = SnapshotProjectsSuccesses ->
      {SnapshotSuccessesMatch}
    [] candidate = SnapshotProjectsMissingVotes ->
      {SnapshotMissingVotesMatch}
    [] candidate = SnapshotProjectsQuorumWithoutQc ->
      {SnapshotQuorumWithoutQcMatch}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_attempts" ->
      spec \ {ResetAttempts}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_successes" ->
      spec \ {ResetSuccesses}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_missing_votes" ->
      spec \ {ResetMissingVotes}
    [] candidate = ResetEmpty /\
          Bug = "reset_empty_keeps_quorum_without_qc" ->
      spec \ {ResetQuorumWithoutQc}
    [] candidate = AttemptIncrements /\ Bug = "attempt_not_counted" ->
      spec \ {IncrementAttempt}
    [] candidate = SuccessIncrements /\ Bug = "success_not_counted" ->
      spec \ {IncrementSuccess}
    [] candidate = MissingVotesAcceptedIncrements /\
          Bug = "missing_votes_not_counted" ->
      spec \ {IncrementMissingVotes}
    [] candidate = QuorumWithoutQcIncrements /\
          Bug = "quorum_without_qc_not_counted" ->
      spec \ {IncrementQuorumWithoutQc}
    [] candidate = AttemptDoesNotCountSuccess /\
          Bug = "attempt_counts_success" ->
      (spec \ {AttemptOnlyAttempts}) \cup {IncrementSuccess}
    [] candidate = SuccessDoesNotCountAttempt /\
          Bug = "success_counts_attempt" ->
      (spec \ {SuccessOnlySuccesses}) \cup {IncrementAttempt}
    [] candidate = MissingVotesDoesNotCountQuorum /\
          Bug = "missing_votes_counts_quorum" ->
      (spec \ {MissingVotesOnlyMissingVotes}) \cup {IncrementQuorumWithoutQc}
    [] candidate = QuorumDoesNotCountMissingVotes /\
          Bug = "quorum_counts_missing_votes" ->
      (spec \ {QuorumOnlyQuorum}) \cup {IncrementMissingVotes}
    [] candidate = RepeatedAttemptsAccumulate /\
          Bug = "repeated_attempt_overwrites_count" ->
      spec \ {AttemptAccumulation}
    [] candidate = RepeatedSuccessesAccumulate /\
          Bug = "repeated_success_overwrites_count" ->
      spec \ {SuccessAccumulation}
    [] candidate = RepeatedMissingVotesAccumulate /\
          Bug = "repeated_missing_votes_overwrites_count" ->
      spec \ {MissingVotesAccumulation}
    [] candidate = RepeatedQuorumWithoutQcAccumulate /\
          Bug = "repeated_quorum_without_qc_overwrites_count" ->
      spec \ {QuorumAccumulation}
    [] candidate = SnapshotProjectsAttempts /\
          Bug = "snapshot_attempts_mismatch" ->
      spec \ {SnapshotAttemptsMatch}
    [] candidate = SnapshotProjectsSuccesses /\
          Bug = "snapshot_successes_mismatch" ->
      spec \ {SnapshotSuccessesMatch}
    [] candidate = SnapshotProjectsMissingVotes /\
          Bug = "snapshot_missing_votes_mismatch" ->
      spec \ {SnapshotMissingVotesMatch}
    [] candidate = SnapshotProjectsQuorumWithoutQc /\
          Bug = "snapshot_quorum_without_qc_mismatch" ->
      spec \ {SnapshotQuorumWithoutQcMatch}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_counters" ->
      spec \ AllResetActions
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 18
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..18

QcRebuildStatusActionsMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

QcRebuildStatusExactness ==
  /\ QcRebuildStatusActionsMatchSpec

Safety ==
  QcRebuildStatusExactness

QcRebuildStatusCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ QcRebuildStatusExactness

BugResetEmptyKeepsAttempts ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsSuccesses ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsMissingVotes ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsQuorumWithoutQc ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugAttemptNotCounted ==
  ImplementationActions(AttemptIncrements) = SpecActions(AttemptIncrements)

BugSuccessNotCounted ==
  ImplementationActions(SuccessIncrements) = SpecActions(SuccessIncrements)

BugMissingVotesNotCounted ==
  ImplementationActions(MissingVotesAcceptedIncrements) =
    SpecActions(MissingVotesAcceptedIncrements)

BugQuorumWithoutQcNotCounted ==
  ImplementationActions(QuorumWithoutQcIncrements) =
    SpecActions(QuorumWithoutQcIncrements)

BugAttemptCountsSuccess ==
  ImplementationActions(AttemptDoesNotCountSuccess) =
    SpecActions(AttemptDoesNotCountSuccess)

BugSuccessCountsAttempt ==
  ImplementationActions(SuccessDoesNotCountAttempt) =
    SpecActions(SuccessDoesNotCountAttempt)

BugMissingVotesCountsQuorum ==
  ImplementationActions(MissingVotesDoesNotCountQuorum) =
    SpecActions(MissingVotesDoesNotCountQuorum)

BugQuorumCountsMissingVotes ==
  ImplementationActions(QuorumDoesNotCountMissingVotes) =
    SpecActions(QuorumDoesNotCountMissingVotes)

BugRepeatedAttemptOverwritesCount ==
  ImplementationActions(RepeatedAttemptsAccumulate) =
    SpecActions(RepeatedAttemptsAccumulate)

BugRepeatedSuccessOverwritesCount ==
  ImplementationActions(RepeatedSuccessesAccumulate) =
    SpecActions(RepeatedSuccessesAccumulate)

BugRepeatedMissingVotesOverwritesCount ==
  ImplementationActions(RepeatedMissingVotesAccumulate) =
    SpecActions(RepeatedMissingVotesAccumulate)

BugRepeatedQuorumWithoutQcOverwritesCount ==
  ImplementationActions(RepeatedQuorumWithoutQcAccumulate) =
    SpecActions(RepeatedQuorumWithoutQcAccumulate)

BugSnapshotAttemptsMismatch ==
  ImplementationActions(SnapshotProjectsAttempts) =
    SpecActions(SnapshotProjectsAttempts)

BugSnapshotSuccessesMismatch ==
  ImplementationActions(SnapshotProjectsSuccesses) =
    SpecActions(SnapshotProjectsSuccesses)

BugSnapshotMissingVotesMismatch ==
  ImplementationActions(SnapshotProjectsMissingVotes) =
    SpecActions(SnapshotProjectsMissingVotes)

BugSnapshotQuorumWithoutQcMismatch ==
  ImplementationActions(SnapshotProjectsQuorumWithoutQc) =
    SpecActions(SnapshotProjectsQuorumWithoutQc)

BugResetAfterRecordsKeepsCounters ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
====
