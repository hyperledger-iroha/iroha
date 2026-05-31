---- MODULE SumeragiKuraStoreStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi Kura persistence status accounting.

This slice captures the Kura telemetry helpers in `status.rs`:
`record_kura_store_failure(...)`, `record_kura_store_retry(...)`,
`record_kura_post_commit_sidecar_failure(...)`, `record_kura_stage(...)`,
`record_kura_stage_rollback(...)`, `record_kura_lock_reset(...)`,
`inc_kura_store_abort()`, their `snapshot().kura_store` projection, and the
test-only `reset_kura_store_counters_for_tests()` helper.
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
FailureRecord == 2
RetryRecord == 3
PostCommitSidecarRecord == 4
StageRecord == 5
RollbackRecord == 6
LockResetWithHash == 7
LockResetWithoutHash == 8
AbortIncrements == 9
RepeatedFailureAccumulates == 10
RepeatedStageAccumulates == 11
LastFailureOverwrites == 12
LastStageOverwrites == 13
RollbackReasonUpdates == 14
LockResetReasonUpdates == 15
SnapshotProjectsFailure == 16
SnapshotProjectsRetry == 17
SnapshotProjectsSidecar == 18
SnapshotProjectsStage == 19
SnapshotProjectsRollback == 20
SnapshotProjectsLockReset == 21
SnapshotProjectsAbort == 22
TopLevelSnapshotIncludesKura == 23
ResetAfterRecordsClears == 24

Candidates == 1..24

ResetTotals == 1
ResetFailureLast == 2
ResetRetry == 3
ResetSidecarLast == 4
ResetStageLast == 5
ResetRollbackLast == 6
ResetLockResetLast == 7
IncrementFailure == 8
SetFailureSlot == 9
SetFailureHash == 10
SetRetryAttempt == 11
SetRetryBackoff == 12
IncrementSidecar == 13
SetSidecarSlot == 14
SetSidecarHash == 15
IncrementStage == 16
SetStageSlot == 17
SetStageHash == 18
IncrementRollback == 19
SetRollbackSlot == 20
SetRollbackHash == 21
SetRollbackReason == 22
IncrementLockReset == 23
SetLockResetSlot == 24
SetLockResetHashSome == 25
SetLockResetHashNone == 26
SetLockResetReason == 27
IncrementAbort == 28
SameCounterAccumulates == 29
LastRecordOverwrites == 30
SnapshotFailureMatches == 31
SnapshotRetryMatches == 32
SnapshotSidecarMatches == 33
SnapshotStageMatches == 34
SnapshotRollbackMatches == 35
SnapshotLockResetMatches == 36
SnapshotAbortMatches == 37
TopLevelKuraMatches == 38
SnapshotPreservesCounts == 39

Actions == 1..39

AllResetActions ==
  {ResetTotals, ResetFailureLast, ResetRetry, ResetSidecarLast,
   ResetStageLast, ResetRollbackLast, ResetLockResetLast}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = FailureRecord ->
      {IncrementFailure, SetFailureSlot, SetFailureHash}
    [] candidate = RetryRecord ->
      {SetRetryAttempt, SetRetryBackoff}
    [] candidate = PostCommitSidecarRecord ->
      {IncrementSidecar, SetSidecarSlot, SetSidecarHash}
    [] candidate = StageRecord ->
      {IncrementStage, SetStageSlot, SetStageHash}
    [] candidate = RollbackRecord ->
      {IncrementRollback, SetRollbackSlot, SetRollbackHash,
       SetRollbackReason}
    [] candidate = LockResetWithHash ->
      {IncrementLockReset, SetLockResetSlot, SetLockResetHashSome,
       SetLockResetReason}
    [] candidate = LockResetWithoutHash ->
      {IncrementLockReset, SetLockResetSlot, SetLockResetHashNone,
       SetLockResetReason}
    [] candidate = AbortIncrements ->
      {IncrementAbort}
    [] candidate = RepeatedFailureAccumulates ->
      {SameCounterAccumulates, SetFailureSlot, SetFailureHash,
       LastRecordOverwrites, SnapshotPreservesCounts}
    [] candidate = RepeatedStageAccumulates ->
      {SameCounterAccumulates, SetStageSlot, SetStageHash,
       LastRecordOverwrites, SnapshotPreservesCounts}
    [] candidate = LastFailureOverwrites ->
      {SetFailureSlot, SetFailureHash, LastRecordOverwrites}
    [] candidate = LastStageOverwrites ->
      {SetStageSlot, SetStageHash, LastRecordOverwrites}
    [] candidate = RollbackReasonUpdates ->
      {SetRollbackReason, LastRecordOverwrites}
    [] candidate = LockResetReasonUpdates ->
      {SetLockResetReason, LastRecordOverwrites}
    [] candidate = SnapshotProjectsFailure ->
      {SnapshotFailureMatches, SnapshotPreservesCounts}
    [] candidate = SnapshotProjectsRetry ->
      {SnapshotRetryMatches}
    [] candidate = SnapshotProjectsSidecar ->
      {SnapshotSidecarMatches, SnapshotPreservesCounts}
    [] candidate = SnapshotProjectsStage ->
      {SnapshotStageMatches, SnapshotPreservesCounts}
    [] candidate = SnapshotProjectsRollback ->
      {SnapshotRollbackMatches, SnapshotPreservesCounts}
    [] candidate = SnapshotProjectsLockReset ->
      {SnapshotLockResetMatches, SnapshotPreservesCounts}
    [] candidate = SnapshotProjectsAbort ->
      {SnapshotAbortMatches, SnapshotPreservesCounts}
    [] candidate = TopLevelSnapshotIncludesKura ->
      {TopLevelKuraMatches}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_totals" ->
      spec \ {ResetTotals}
    [] candidate = FailureRecord /\ Bug = "failure_not_counted" ->
      spec \ {IncrementFailure}
    [] candidate = FailureRecord /\ Bug = "failure_slot_not_recorded" ->
      spec \ {SetFailureSlot}
    [] candidate = FailureRecord /\ Bug = "failure_hash_not_recorded" ->
      spec \ {SetFailureHash}
    [] candidate = RetryRecord /\ Bug = "retry_attempt_not_recorded" ->
      spec \ {SetRetryAttempt}
    [] candidate = RetryRecord /\ Bug = "retry_backoff_not_recorded" ->
      spec \ {SetRetryBackoff}
    [] candidate = PostCommitSidecarRecord /\
          Bug = "sidecar_not_counted" ->
      spec \ {IncrementSidecar}
    [] candidate = PostCommitSidecarRecord /\
          Bug = "sidecar_slot_not_recorded" ->
      spec \ {SetSidecarSlot}
    [] candidate = PostCommitSidecarRecord /\
          Bug = "sidecar_hash_not_recorded" ->
      spec \ {SetSidecarHash}
    [] candidate = StageRecord /\ Bug = "stage_not_counted" ->
      spec \ {IncrementStage}
    [] candidate = StageRecord /\ Bug = "stage_slot_not_recorded" ->
      spec \ {SetStageSlot}
    [] candidate = StageRecord /\ Bug = "stage_hash_not_recorded" ->
      spec \ {SetStageHash}
    [] candidate = RollbackRecord /\ Bug = "rollback_not_counted" ->
      spec \ {IncrementRollback}
    [] candidate = RollbackRecord /\
          Bug = "rollback_reason_not_recorded" ->
      spec \ {SetRollbackReason}
    [] candidate = LockResetWithHash /\ Bug = "lock_reset_not_counted" ->
      spec \ {IncrementLockReset}
    [] candidate = LockResetWithoutHash /\
          Bug = "lock_reset_none_keeps_hash" ->
      (spec \ {SetLockResetHashNone}) \cup {SetLockResetHashSome}
    [] candidate = AbortIncrements /\ Bug = "abort_not_counted" ->
      spec \ {IncrementAbort}
    [] candidate = RepeatedFailureAccumulates /\
          Bug = "failure_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotPreservesCounts}
    [] candidate = RepeatedStageAccumulates /\
          Bug = "stage_overwrites_count" ->
      spec \ {SameCounterAccumulates, SnapshotPreservesCounts}
    [] candidate = LastFailureOverwrites /\
          Bug = "last_failure_not_overwritten" ->
      spec \ {SetFailureSlot, SetFailureHash, LastRecordOverwrites}
    [] candidate = LastStageOverwrites /\
          Bug = "last_stage_not_overwritten" ->
      spec \ {SetStageSlot, SetStageHash, LastRecordOverwrites}
    [] candidate = SnapshotProjectsFailure /\
          Bug = "snapshot_failure_mismatch" ->
      spec \ {SnapshotFailureMatches}
    [] candidate = SnapshotProjectsRetry /\
          Bug = "snapshot_retry_mismatch" ->
      spec \ {SnapshotRetryMatches}
    [] candidate = SnapshotProjectsSidecar /\
          Bug = "snapshot_sidecar_mismatch" ->
      spec \ {SnapshotSidecarMatches}
    [] candidate = SnapshotProjectsStage /\
          Bug = "snapshot_stage_mismatch" ->
      spec \ {SnapshotStageMatches}
    [] candidate = SnapshotProjectsRollback /\
          Bug = "snapshot_rollback_mismatch" ->
      spec \ {SnapshotRollbackMatches}
    [] candidate = SnapshotProjectsLockReset /\
          Bug = "snapshot_lock_reset_mismatch" ->
      spec \ {SnapshotLockResetMatches}
    [] candidate = SnapshotProjectsAbort /\
          Bug = "snapshot_abort_mismatch" ->
      spec \ {SnapshotAbortMatches}
    [] candidate = TopLevelSnapshotIncludesKura /\
          Bug = "top_level_snapshot_drops_kura" ->
      spec \ {TopLevelKuraMatches}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_counters" ->
      spec \ {ResetTotals}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_last" ->
      spec \ {ResetFailureLast, ResetRetry, ResetSidecarLast,
              ResetStageLast, ResetRollbackLast, ResetLockResetLast}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 24
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..24

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugResetEmptyKeepsTotals ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugFailureNotCounted ==
  ImplementationActions(FailureRecord) = SpecActions(FailureRecord)

BugFailureSlotNotRecorded ==
  ImplementationActions(FailureRecord) = SpecActions(FailureRecord)

BugFailureHashNotRecorded ==
  ImplementationActions(FailureRecord) = SpecActions(FailureRecord)

BugRetryAttemptNotRecorded ==
  ImplementationActions(RetryRecord) = SpecActions(RetryRecord)

BugRetryBackoffNotRecorded ==
  ImplementationActions(RetryRecord) = SpecActions(RetryRecord)

BugSidecarNotCounted ==
  ImplementationActions(PostCommitSidecarRecord) =
    SpecActions(PostCommitSidecarRecord)

BugSidecarSlotNotRecorded ==
  ImplementationActions(PostCommitSidecarRecord) =
    SpecActions(PostCommitSidecarRecord)

BugSidecarHashNotRecorded ==
  ImplementationActions(PostCommitSidecarRecord) =
    SpecActions(PostCommitSidecarRecord)

BugStageNotCounted ==
  ImplementationActions(StageRecord) = SpecActions(StageRecord)

BugStageSlotNotRecorded ==
  ImplementationActions(StageRecord) = SpecActions(StageRecord)

BugStageHashNotRecorded ==
  ImplementationActions(StageRecord) = SpecActions(StageRecord)

BugRollbackNotCounted ==
  ImplementationActions(RollbackRecord) = SpecActions(RollbackRecord)

BugRollbackReasonNotRecorded ==
  ImplementationActions(RollbackRecord) = SpecActions(RollbackRecord)

BugLockResetNotCounted ==
  ImplementationActions(LockResetWithHash) = SpecActions(LockResetWithHash)

BugLockResetNoneKeepsHash ==
  ImplementationActions(LockResetWithoutHash) =
    SpecActions(LockResetWithoutHash)

BugAbortNotCounted ==
  ImplementationActions(AbortIncrements) = SpecActions(AbortIncrements)

BugFailureOverwritesCount ==
  ImplementationActions(RepeatedFailureAccumulates) =
    SpecActions(RepeatedFailureAccumulates)

BugStageOverwritesCount ==
  ImplementationActions(RepeatedStageAccumulates) =
    SpecActions(RepeatedStageAccumulates)

BugLastFailureNotOverwritten ==
  ImplementationActions(LastFailureOverwrites) =
    SpecActions(LastFailureOverwrites)

BugLastStageNotOverwritten ==
  ImplementationActions(LastStageOverwrites) =
    SpecActions(LastStageOverwrites)

BugSnapshotFailureMismatch ==
  ImplementationActions(SnapshotProjectsFailure) =
    SpecActions(SnapshotProjectsFailure)

BugSnapshotRetryMismatch ==
  ImplementationActions(SnapshotProjectsRetry) =
    SpecActions(SnapshotProjectsRetry)

BugSnapshotSidecarMismatch ==
  ImplementationActions(SnapshotProjectsSidecar) =
    SpecActions(SnapshotProjectsSidecar)

BugSnapshotStageMismatch ==
  ImplementationActions(SnapshotProjectsStage) =
    SpecActions(SnapshotProjectsStage)

BugSnapshotRollbackMismatch ==
  ImplementationActions(SnapshotProjectsRollback) =
    SpecActions(SnapshotProjectsRollback)

BugSnapshotLockResetMismatch ==
  ImplementationActions(SnapshotProjectsLockReset) =
    SpecActions(SnapshotProjectsLockReset)

BugSnapshotAbortMismatch ==
  ImplementationActions(SnapshotProjectsAbort) =
    SpecActions(SnapshotProjectsAbort)

BugTopLevelSnapshotDropsKura ==
  ImplementationActions(TopLevelSnapshotIncludesKura) =
    SpecActions(TopLevelSnapshotIncludesKura)

BugResetAfterRecordsKeepsCounters ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsLast ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
