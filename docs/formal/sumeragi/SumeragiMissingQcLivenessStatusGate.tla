---- MODULE SumeragiMissingQcLivenessStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi missing-QC liveness status accounting.

This slice captures `inc_consensus_missing_block_height_escalation()`,
`inc_consensus_missing_block_height_progress_deferred()`,
`inc_consensus_missing_qc_reacquire_attempt()`,
`inc_consensus_missing_qc_reacquire_success()`,
`inc_consensus_missing_qc_reacquire_exhausted()`,
`inc_consensus_missing_qc_rotation_deferred()`,
`inc_consensus_forced_proposal_attempt()`,
`inc_consensus_forced_proposal_success()`,
`observe_consensus_recovery_stuck_round(...)`, their `snapshot()`
projection, and the relevant subset of the test-only
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
MissingBlockEscalationRecord == 2
MissingBlockProgressDeferredRecord == 3
MissingQcAttemptRecord == 4
MissingQcSuccessRecord == 5
MissingQcExhaustedRecord == 6
MissingQcRotationDeferredRecord == 7
ForcedProposalAttemptRecord == 8
ForcedProposalSuccessRecord == 9
RepeatedMissingQcAttemptAccumulates == 10
RepeatedForcedProposalAttemptAccumulates == 11
ObserveStuckRoundSetsLast == 12
StuckRoundOverwrites == 13
SnapshotProjectsMissingBlock == 14
SnapshotProjectsMissingQc == 15
SnapshotProjectsForcedProposal == 16
SnapshotProjectsStuckRound == 17
ResetAfterRecordsClears == 18

Candidates == 1..18

ResetCounters == 1
ResetStuckRound == 2
IncrementMissingBlockEscalation == 3
IncrementMissingBlockProgressDeferred == 4
IncrementMissingQcAttempt == 5
IncrementMissingQcSuccess == 6
IncrementMissingQcExhausted == 7
IncrementMissingQcRotationDeferred == 8
IncrementForcedProposalAttempt == 9
IncrementForcedProposalSuccess == 10
SameCounterAccumulates == 11
SetStuckRound == 12
StuckRoundOverwritesAction == 13
SnapshotMissingBlockMatches == 14
SnapshotMissingQcMatches == 15
SnapshotForcedProposalMatches == 16
SnapshotStuckRoundMatches == 17

Actions == 1..17

AllResetActions == {ResetCounters, ResetStuckRound}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = MissingBlockEscalationRecord ->
      {IncrementMissingBlockEscalation}
    [] candidate = MissingBlockProgressDeferredRecord ->
      {IncrementMissingBlockProgressDeferred}
    [] candidate = MissingQcAttemptRecord ->
      {IncrementMissingQcAttempt}
    [] candidate = MissingQcSuccessRecord ->
      {IncrementMissingQcSuccess}
    [] candidate = MissingQcExhaustedRecord ->
      {IncrementMissingQcExhausted}
    [] candidate = MissingQcRotationDeferredRecord ->
      {IncrementMissingQcRotationDeferred}
    [] candidate = ForcedProposalAttemptRecord ->
      {IncrementForcedProposalAttempt}
    [] candidate = ForcedProposalSuccessRecord ->
      {IncrementForcedProposalSuccess}
    [] candidate = RepeatedMissingQcAttemptAccumulates ->
      {IncrementMissingQcAttempt, SameCounterAccumulates,
       SnapshotMissingQcMatches}
    [] candidate = RepeatedForcedProposalAttemptAccumulates ->
      {IncrementForcedProposalAttempt, SameCounterAccumulates,
       SnapshotForcedProposalMatches}
    [] candidate = ObserveStuckRoundSetsLast ->
      {SetStuckRound, SnapshotStuckRoundMatches}
    [] candidate = StuckRoundOverwrites ->
      {SetStuckRound, StuckRoundOverwritesAction,
       SnapshotStuckRoundMatches}
    [] candidate = SnapshotProjectsMissingBlock ->
      {SnapshotMissingBlockMatches}
    [] candidate = SnapshotProjectsMissingQc ->
      {SnapshotMissingQcMatches}
    [] candidate = SnapshotProjectsForcedProposal ->
      {SnapshotForcedProposalMatches}
    [] candidate = SnapshotProjectsStuckRound ->
      {SnapshotStuckRoundMatches}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_counters" ->
      spec \ {ResetCounters}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_stuck" ->
      spec \ {ResetStuckRound}
    [] candidate = MissingBlockEscalationRecord /\
          Bug = "missing_block_escalation_not_counted" ->
      spec \ {IncrementMissingBlockEscalation}
    [] candidate = MissingBlockProgressDeferredRecord /\
          Bug = "missing_block_progress_deferred_not_counted" ->
      spec \ {IncrementMissingBlockProgressDeferred}
    [] candidate = MissingQcAttemptRecord /\
          Bug = "missing_qc_attempt_not_counted" ->
      spec \ {IncrementMissingQcAttempt}
    [] candidate = MissingQcSuccessRecord /\
          Bug = "missing_qc_success_not_counted" ->
      spec \ {IncrementMissingQcSuccess}
    [] candidate = MissingQcExhaustedRecord /\
          Bug = "missing_qc_exhausted_not_counted" ->
      spec \ {IncrementMissingQcExhausted}
    [] candidate = MissingQcRotationDeferredRecord /\
          Bug = "missing_qc_rotation_deferred_not_counted" ->
      spec \ {IncrementMissingQcRotationDeferred}
    [] candidate = ForcedProposalAttemptRecord /\
          Bug = "forced_proposal_attempt_not_counted" ->
      spec \ {IncrementForcedProposalAttempt}
    [] candidate = ForcedProposalSuccessRecord /\
          Bug = "forced_proposal_success_not_counted" ->
      spec \ {IncrementForcedProposalSuccess}
    [] candidate = MissingQcAttemptRecord /\
          Bug = "missing_qc_attempt_counts_success" ->
      (spec \ {IncrementMissingQcAttempt}) \cup
        {IncrementMissingQcSuccess}
    [] candidate = ForcedProposalAttemptRecord /\
          Bug = "forced_proposal_attempt_counts_success" ->
      (spec \ {IncrementForcedProposalAttempt}) \cup
        {IncrementForcedProposalSuccess}
    [] candidate = RepeatedMissingQcAttemptAccumulates /\
          Bug = "repeated_missing_qc_attempt_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotMissingQcMatches}
    [] candidate = RepeatedForcedProposalAttemptAccumulates /\
          Bug = "repeated_forced_proposal_attempt_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotForcedProposalMatches}
    [] candidate = ObserveStuckRoundSetsLast /\
          Bug = "stuck_round_not_set" ->
      spec \ {SetStuckRound}
    [] candidate = StuckRoundOverwrites /\
          Bug = "stuck_round_not_overwritten" ->
      spec \ {SetStuckRound, StuckRoundOverwritesAction,
              SnapshotStuckRoundMatches}
    [] candidate = SnapshotProjectsMissingBlock /\
          Bug = "snapshot_missing_block_mismatch" ->
      spec \ {SnapshotMissingBlockMatches}
    [] candidate = SnapshotProjectsMissingQc /\
          Bug = "snapshot_missing_qc_mismatch" ->
      spec \ {SnapshotMissingQcMatches}
    [] candidate = SnapshotProjectsForcedProposal /\
          Bug = "snapshot_forced_proposal_mismatch" ->
      spec \ {SnapshotForcedProposalMatches}
    [] candidate = SnapshotProjectsStuckRound /\
          Bug = "snapshot_stuck_round_mismatch" ->
      spec \ {SnapshotStuckRoundMatches}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_counters" ->
      spec \ {ResetCounters}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_stuck" ->
      spec \ {ResetStuckRound}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 18
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..18

Safety ==
  /\ ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)
  /\ ImplementationActions(MissingBlockEscalationRecord) =
       SpecActions(MissingBlockEscalationRecord)
  /\ ImplementationActions(MissingBlockProgressDeferredRecord) =
       SpecActions(MissingBlockProgressDeferredRecord)
  /\ ImplementationActions(MissingQcAttemptRecord) =
       SpecActions(MissingQcAttemptRecord)
  /\ ImplementationActions(MissingQcSuccessRecord) =
       SpecActions(MissingQcSuccessRecord)
  /\ ImplementationActions(MissingQcExhaustedRecord) =
       SpecActions(MissingQcExhaustedRecord)
  /\ ImplementationActions(MissingQcRotationDeferredRecord) =
       SpecActions(MissingQcRotationDeferredRecord)
  /\ ImplementationActions(ForcedProposalAttemptRecord) =
       SpecActions(ForcedProposalAttemptRecord)
  /\ ImplementationActions(ForcedProposalSuccessRecord) =
       SpecActions(ForcedProposalSuccessRecord)
  /\ ImplementationActions(RepeatedMissingQcAttemptAccumulates) =
       SpecActions(RepeatedMissingQcAttemptAccumulates)
  /\ ImplementationActions(RepeatedForcedProposalAttemptAccumulates) =
       SpecActions(RepeatedForcedProposalAttemptAccumulates)
  /\ ImplementationActions(ObserveStuckRoundSetsLast) =
       SpecActions(ObserveStuckRoundSetsLast)
  /\ ImplementationActions(StuckRoundOverwrites) =
       SpecActions(StuckRoundOverwrites)
  /\ ImplementationActions(SnapshotProjectsMissingBlock) =
       SpecActions(SnapshotProjectsMissingBlock)
  /\ ImplementationActions(SnapshotProjectsMissingQc) =
       SpecActions(SnapshotProjectsMissingQc)
  /\ ImplementationActions(SnapshotProjectsForcedProposal) =
       SpecActions(SnapshotProjectsForcedProposal)
  /\ ImplementationActions(SnapshotProjectsStuckRound) =
       SpecActions(SnapshotProjectsStuckRound)
  /\ ImplementationActions(ResetAfterRecordsClears) =
       SpecActions(ResetAfterRecordsClears)

BugResetEmptyKeepsCounters ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsStuck ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugMissingBlockEscalationNotCounted ==
  ImplementationActions(MissingBlockEscalationRecord) =
    SpecActions(MissingBlockEscalationRecord)

BugMissingBlockProgressDeferredNotCounted ==
  ImplementationActions(MissingBlockProgressDeferredRecord) =
    SpecActions(MissingBlockProgressDeferredRecord)

BugMissingQcAttemptNotCounted ==
  ImplementationActions(MissingQcAttemptRecord) =
    SpecActions(MissingQcAttemptRecord)

BugMissingQcSuccessNotCounted ==
  ImplementationActions(MissingQcSuccessRecord) =
    SpecActions(MissingQcSuccessRecord)

BugMissingQcExhaustedNotCounted ==
  ImplementationActions(MissingQcExhaustedRecord) =
    SpecActions(MissingQcExhaustedRecord)

BugMissingQcRotationDeferredNotCounted ==
  ImplementationActions(MissingQcRotationDeferredRecord) =
    SpecActions(MissingQcRotationDeferredRecord)

BugForcedProposalAttemptNotCounted ==
  ImplementationActions(ForcedProposalAttemptRecord) =
    SpecActions(ForcedProposalAttemptRecord)

BugForcedProposalSuccessNotCounted ==
  ImplementationActions(ForcedProposalSuccessRecord) =
    SpecActions(ForcedProposalSuccessRecord)

BugMissingQcAttemptCountsSuccess ==
  ImplementationActions(MissingQcAttemptRecord) =
    SpecActions(MissingQcAttemptRecord)

BugForcedProposalAttemptCountsSuccess ==
  ImplementationActions(ForcedProposalAttemptRecord) =
    SpecActions(ForcedProposalAttemptRecord)

BugRepeatedMissingQcAttemptOverwritesCount ==
  ImplementationActions(RepeatedMissingQcAttemptAccumulates) =
    SpecActions(RepeatedMissingQcAttemptAccumulates)

BugRepeatedForcedProposalAttemptOverwritesCount ==
  ImplementationActions(RepeatedForcedProposalAttemptAccumulates) =
    SpecActions(RepeatedForcedProposalAttemptAccumulates)

BugStuckRoundNotSet ==
  ImplementationActions(ObserveStuckRoundSetsLast) =
    SpecActions(ObserveStuckRoundSetsLast)

BugStuckRoundNotOverwritten ==
  ImplementationActions(StuckRoundOverwrites) =
    SpecActions(StuckRoundOverwrites)

BugSnapshotMissingBlockMismatch ==
  ImplementationActions(SnapshotProjectsMissingBlock) =
    SpecActions(SnapshotProjectsMissingBlock)

BugSnapshotMissingQcMismatch ==
  ImplementationActions(SnapshotProjectsMissingQc) =
    SpecActions(SnapshotProjectsMissingQc)

BugSnapshotForcedProposalMismatch ==
  ImplementationActions(SnapshotProjectsForcedProposal) =
    SpecActions(SnapshotProjectsForcedProposal)

BugSnapshotStuckRoundMismatch ==
  ImplementationActions(SnapshotProjectsStuckRound) =
    SpecActions(SnapshotProjectsStuckRound)

BugResetAfterRecordsKeepsCounters ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsStuck ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
====
