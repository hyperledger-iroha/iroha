---- MODULE SumeragiTimingStatusCountersGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi timing/liveness status counters.

This slice captures `inc_pacemaker_backpressure_deferrals()`,
`note_commit_pipeline_tick(..., has_pending)`, `inc_prevote_timeout()`,
`inc_da_reschedule(...)`, `inc_rbc_deliver_defer_ready()`,
`inc_rbc_deliver_defer_chunks()`, their status/getter projections, and the
test-only pacemaker/prevote reset helpers from `status.rs`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PacemakerResetEmpty == 1
PacemakerRecord == 2
PacemakerRepeatedAccumulates == 3
CommitTickNoPendingNoop == 4
CommitTickPendingRecord == 5
CommitTickRepeatedAccumulates == 6
PrevoteResetEmpty == 7
PrevoteTimeoutRecord == 8
PrevoteTimeoutRepeatedAccumulates == 9
DaRescheduleRecord == 10
DaRescheduleRepeatedAccumulates == 11
RbcReadyRecord == 12
RbcChunksRecord == 13
RbcReadyRepeatedAccumulates == 14
RbcChunksRepeatedAccumulates == 15
RbcCountersDistinct == 16
SnapshotProjectsPacemakerCommit == 17
SnapshotProjectsPrevoteDa == 18
SnapshotProjectsRbc == 19
GetterProjectsCommit == 20
GetterProjectsPrevoteDa == 21
GetterProjectsRbc == 22
PacemakerResetAfterRecord == 23
PrevoteResetAfterRecord == 24

Candidates == 1..24

ResetPacemaker == 1
ResetPrevote == 2
IncrementPacemaker == 3
IncrementCommitTick == 4
CommitTickNoopWithoutPending == 5
IncrementPrevote == 6
IncrementDaReschedule == 7
IncrementRbcReady == 8
IncrementRbcChunks == 9
SameCounterAccumulates == 10
RbcCountersRemainDistinct == 11
SnapshotPacemakerCommitMatches == 12
SnapshotPrevoteDaMatches == 13
SnapshotRbcMatches == 14
GetterCommitMatches == 15
GetterPrevoteDaMatches == 16
GetterRbcMatches == 17

Actions == 1..17

SpecActions(candidate) ==
  CASE candidate = PacemakerResetEmpty ->
      {ResetPacemaker}
    [] candidate = PacemakerRecord ->
      {IncrementPacemaker}
    [] candidate = PacemakerRepeatedAccumulates ->
      {IncrementPacemaker, SameCounterAccumulates,
       SnapshotPacemakerCommitMatches}
    [] candidate = CommitTickNoPendingNoop ->
      {CommitTickNoopWithoutPending, GetterCommitMatches}
    [] candidate = CommitTickPendingRecord ->
      {IncrementCommitTick, SnapshotPacemakerCommitMatches,
       GetterCommitMatches}
    [] candidate = CommitTickRepeatedAccumulates ->
      {IncrementCommitTick, SameCounterAccumulates,
       SnapshotPacemakerCommitMatches, GetterCommitMatches}
    [] candidate = PrevoteResetEmpty ->
      {ResetPrevote}
    [] candidate = PrevoteTimeoutRecord ->
      {IncrementPrevote, SnapshotPrevoteDaMatches, GetterPrevoteDaMatches}
    [] candidate = PrevoteTimeoutRepeatedAccumulates ->
      {IncrementPrevote, SameCounterAccumulates,
       SnapshotPrevoteDaMatches, GetterPrevoteDaMatches}
    [] candidate = DaRescheduleRecord ->
      {IncrementDaReschedule, SnapshotPrevoteDaMatches,
       GetterPrevoteDaMatches}
    [] candidate = DaRescheduleRepeatedAccumulates ->
      {IncrementDaReschedule, SameCounterAccumulates,
       SnapshotPrevoteDaMatches, GetterPrevoteDaMatches}
    [] candidate = RbcReadyRecord ->
      {IncrementRbcReady, SnapshotRbcMatches, GetterRbcMatches}
    [] candidate = RbcChunksRecord ->
      {IncrementRbcChunks, SnapshotRbcMatches, GetterRbcMatches}
    [] candidate = RbcReadyRepeatedAccumulates ->
      {IncrementRbcReady, SameCounterAccumulates,
       SnapshotRbcMatches, GetterRbcMatches}
    [] candidate = RbcChunksRepeatedAccumulates ->
      {IncrementRbcChunks, SameCounterAccumulates,
       SnapshotRbcMatches, GetterRbcMatches}
    [] candidate = RbcCountersDistinct ->
      {IncrementRbcReady, IncrementRbcChunks, RbcCountersRemainDistinct,
       SnapshotRbcMatches, GetterRbcMatches}
    [] candidate = SnapshotProjectsPacemakerCommit ->
      {SnapshotPacemakerCommitMatches}
    [] candidate = SnapshotProjectsPrevoteDa ->
      {SnapshotPrevoteDaMatches}
    [] candidate = SnapshotProjectsRbc ->
      {SnapshotRbcMatches}
    [] candidate = GetterProjectsCommit ->
      {GetterCommitMatches}
    [] candidate = GetterProjectsPrevoteDa ->
      {GetterPrevoteDaMatches}
    [] candidate = GetterProjectsRbc ->
      {GetterRbcMatches}
    [] candidate = PacemakerResetAfterRecord ->
      {ResetPacemaker}
    [] candidate = PrevoteResetAfterRecord ->
      {ResetPrevote}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = PacemakerResetEmpty /\
          Bug = "reset_empty_keeps_pacemaker" ->
      spec \ {ResetPacemaker}
    [] candidate = PacemakerRecord /\
          Bug = "pacemaker_not_counted" ->
      spec \ {IncrementPacemaker}
    [] candidate = PacemakerRepeatedAccumulates /\
          Bug = "repeated_pacemaker_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotPacemakerCommitMatches}
    [] candidate = CommitTickNoPendingNoop /\
          Bug = "commit_tick_no_pending_increments" ->
      (spec \ {CommitTickNoopWithoutPending}) \cup {IncrementCommitTick}
    [] candidate = CommitTickPendingRecord /\
          Bug = "commit_tick_pending_not_counted" ->
      spec \ {IncrementCommitTick}
    [] candidate = CommitTickRepeatedAccumulates /\
          Bug = "repeated_commit_tick_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotPacemakerCommitMatches,
              GetterCommitMatches}
    [] candidate = PrevoteResetEmpty /\
          Bug = "reset_empty_keeps_prevote" ->
      spec \ {ResetPrevote}
    [] candidate = PrevoteTimeoutRecord /\
          Bug = "prevote_timeout_not_counted" ->
      spec \ {IncrementPrevote}
    [] candidate = PrevoteTimeoutRepeatedAccumulates /\
          Bug = "repeated_prevote_timeout_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotPrevoteDaMatches,
              GetterPrevoteDaMatches}
    [] candidate = DaRescheduleRecord /\
          Bug = "da_reschedule_not_counted" ->
      spec \ {IncrementDaReschedule}
    [] candidate = DaRescheduleRepeatedAccumulates /\
          Bug = "repeated_da_reschedule_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotPrevoteDaMatches,
              GetterPrevoteDaMatches}
    [] candidate = RbcReadyRecord /\
          Bug = "rbc_ready_not_counted" ->
      spec \ {IncrementRbcReady}
    [] candidate = RbcChunksRecord /\
          Bug = "rbc_chunks_not_counted" ->
      spec \ {IncrementRbcChunks}
    [] candidate = RbcReadyRecord /\
          Bug = "rbc_ready_counts_chunks" ->
      (spec \ {IncrementRbcReady}) \cup {IncrementRbcChunks}
    [] candidate = RbcChunksRecord /\
          Bug = "rbc_chunks_counts_ready" ->
      (spec \ {IncrementRbcChunks}) \cup {IncrementRbcReady}
    [] candidate = RbcReadyRepeatedAccumulates /\
          Bug = "repeated_rbc_ready_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotRbcMatches,
              GetterRbcMatches}
    [] candidate = RbcChunksRepeatedAccumulates /\
          Bug = "repeated_rbc_chunks_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotRbcMatches,
              GetterRbcMatches}
    [] candidate = RbcCountersDistinct /\
          Bug = "rbc_counters_collide" ->
      spec \ {RbcCountersRemainDistinct, SnapshotRbcMatches,
              GetterRbcMatches}
    [] candidate = SnapshotProjectsPacemakerCommit /\
          Bug = "snapshot_pacemaker_commit_mismatch" ->
      spec \ {SnapshotPacemakerCommitMatches}
    [] candidate = SnapshotProjectsPrevoteDa /\
          Bug = "snapshot_prevote_da_mismatch" ->
      spec \ {SnapshotPrevoteDaMatches}
    [] candidate = SnapshotProjectsRbc /\
          Bug = "snapshot_rbc_mismatch" ->
      spec \ {SnapshotRbcMatches}
    [] candidate = GetterProjectsCommit /\
          Bug = "getter_commit_mismatch" ->
      spec \ {GetterCommitMatches}
    [] candidate = GetterProjectsPrevoteDa /\
          Bug = "getter_prevote_da_mismatch" ->
      spec \ {GetterPrevoteDaMatches}
    [] candidate = GetterProjectsRbc /\
          Bug = "getter_rbc_mismatch" ->
      spec \ {GetterRbcMatches}
    [] candidate = PacemakerResetAfterRecord /\
          Bug = "reset_after_record_keeps_pacemaker" ->
      spec \ {ResetPacemaker}
    [] candidate = PrevoteResetAfterRecord /\
          Bug = "reset_after_record_keeps_prevote" ->
      spec \ {ResetPrevote}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 24
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..24

Safety ==
  /\ ImplementationActions(PacemakerResetEmpty) =
       SpecActions(PacemakerResetEmpty)
  /\ ImplementationActions(PacemakerRecord) = SpecActions(PacemakerRecord)
  /\ ImplementationActions(PacemakerRepeatedAccumulates) =
       SpecActions(PacemakerRepeatedAccumulates)
  /\ ImplementationActions(CommitTickNoPendingNoop) =
       SpecActions(CommitTickNoPendingNoop)
  /\ ImplementationActions(CommitTickPendingRecord) =
       SpecActions(CommitTickPendingRecord)
  /\ ImplementationActions(CommitTickRepeatedAccumulates) =
       SpecActions(CommitTickRepeatedAccumulates)
  /\ ImplementationActions(PrevoteResetEmpty) =
       SpecActions(PrevoteResetEmpty)
  /\ ImplementationActions(PrevoteTimeoutRecord) =
       SpecActions(PrevoteTimeoutRecord)
  /\ ImplementationActions(PrevoteTimeoutRepeatedAccumulates) =
       SpecActions(PrevoteTimeoutRepeatedAccumulates)
  /\ ImplementationActions(DaRescheduleRecord) =
       SpecActions(DaRescheduleRecord)
  /\ ImplementationActions(DaRescheduleRepeatedAccumulates) =
       SpecActions(DaRescheduleRepeatedAccumulates)
  /\ ImplementationActions(RbcReadyRecord) = SpecActions(RbcReadyRecord)
  /\ ImplementationActions(RbcChunksRecord) = SpecActions(RbcChunksRecord)
  /\ ImplementationActions(RbcReadyRepeatedAccumulates) =
       SpecActions(RbcReadyRepeatedAccumulates)
  /\ ImplementationActions(RbcChunksRepeatedAccumulates) =
       SpecActions(RbcChunksRepeatedAccumulates)
  /\ ImplementationActions(RbcCountersDistinct) =
       SpecActions(RbcCountersDistinct)
  /\ ImplementationActions(SnapshotProjectsPacemakerCommit) =
       SpecActions(SnapshotProjectsPacemakerCommit)
  /\ ImplementationActions(SnapshotProjectsPrevoteDa) =
       SpecActions(SnapshotProjectsPrevoteDa)
  /\ ImplementationActions(SnapshotProjectsRbc) =
       SpecActions(SnapshotProjectsRbc)
  /\ ImplementationActions(GetterProjectsCommit) =
       SpecActions(GetterProjectsCommit)
  /\ ImplementationActions(GetterProjectsPrevoteDa) =
       SpecActions(GetterProjectsPrevoteDa)
  /\ ImplementationActions(GetterProjectsRbc) =
       SpecActions(GetterProjectsRbc)
  /\ ImplementationActions(PacemakerResetAfterRecord) =
       SpecActions(PacemakerResetAfterRecord)
  /\ ImplementationActions(PrevoteResetAfterRecord) =
       SpecActions(PrevoteResetAfterRecord)

BugResetEmptyKeepsPacemaker ==
  ImplementationActions(PacemakerResetEmpty) =
    SpecActions(PacemakerResetEmpty)

BugPacemakerNotCounted ==
  ImplementationActions(PacemakerRecord) = SpecActions(PacemakerRecord)

BugRepeatedPacemakerOverwritesCount ==
  ImplementationActions(PacemakerRepeatedAccumulates) =
    SpecActions(PacemakerRepeatedAccumulates)

BugCommitTickNoPendingIncrements ==
  ImplementationActions(CommitTickNoPendingNoop) =
    SpecActions(CommitTickNoPendingNoop)

BugCommitTickPendingNotCounted ==
  ImplementationActions(CommitTickPendingRecord) =
    SpecActions(CommitTickPendingRecord)

BugRepeatedCommitTickOverwritesCount ==
  ImplementationActions(CommitTickRepeatedAccumulates) =
    SpecActions(CommitTickRepeatedAccumulates)

BugResetEmptyKeepsPrevote ==
  ImplementationActions(PrevoteResetEmpty) =
    SpecActions(PrevoteResetEmpty)

BugPrevoteTimeoutNotCounted ==
  ImplementationActions(PrevoteTimeoutRecord) =
    SpecActions(PrevoteTimeoutRecord)

BugRepeatedPrevoteTimeoutOverwritesCount ==
  ImplementationActions(PrevoteTimeoutRepeatedAccumulates) =
    SpecActions(PrevoteTimeoutRepeatedAccumulates)

BugDaRescheduleNotCounted ==
  ImplementationActions(DaRescheduleRecord) =
    SpecActions(DaRescheduleRecord)

BugRepeatedDaRescheduleOverwritesCount ==
  ImplementationActions(DaRescheduleRepeatedAccumulates) =
    SpecActions(DaRescheduleRepeatedAccumulates)

BugRbcReadyNotCounted ==
  ImplementationActions(RbcReadyRecord) = SpecActions(RbcReadyRecord)

BugRbcChunksNotCounted ==
  ImplementationActions(RbcChunksRecord) = SpecActions(RbcChunksRecord)

BugRbcReadyCountsChunks ==
  ImplementationActions(RbcReadyRecord) = SpecActions(RbcReadyRecord)

BugRbcChunksCountsReady ==
  ImplementationActions(RbcChunksRecord) = SpecActions(RbcChunksRecord)

BugRepeatedRbcReadyOverwritesCount ==
  ImplementationActions(RbcReadyRepeatedAccumulates) =
    SpecActions(RbcReadyRepeatedAccumulates)

BugRepeatedRbcChunksOverwritesCount ==
  ImplementationActions(RbcChunksRepeatedAccumulates) =
    SpecActions(RbcChunksRepeatedAccumulates)

BugRbcCountersCollide ==
  ImplementationActions(RbcCountersDistinct) =
    SpecActions(RbcCountersDistinct)

BugSnapshotPacemakerCommitMismatch ==
  ImplementationActions(SnapshotProjectsPacemakerCommit) =
    SpecActions(SnapshotProjectsPacemakerCommit)

BugSnapshotPrevoteDaMismatch ==
  ImplementationActions(SnapshotProjectsPrevoteDa) =
    SpecActions(SnapshotProjectsPrevoteDa)

BugSnapshotRbcMismatch ==
  ImplementationActions(SnapshotProjectsRbc) =
    SpecActions(SnapshotProjectsRbc)

BugGetterCommitMismatch ==
  ImplementationActions(GetterProjectsCommit) =
    SpecActions(GetterProjectsCommit)

BugGetterPrevoteDaMismatch ==
  ImplementationActions(GetterProjectsPrevoteDa) =
    SpecActions(GetterProjectsPrevoteDa)

BugGetterRbcMismatch ==
  ImplementationActions(GetterProjectsRbc) =
    SpecActions(GetterProjectsRbc)

BugResetAfterRecordKeepsPacemaker ==
  ImplementationActions(PacemakerResetAfterRecord) =
    SpecActions(PacemakerResetAfterRecord)

BugResetAfterRecordKeepsPrevote ==
  ImplementationActions(PrevoteResetAfterRecord) =
    SpecActions(PrevoteResetAfterRecord)

=============================================================================
====
