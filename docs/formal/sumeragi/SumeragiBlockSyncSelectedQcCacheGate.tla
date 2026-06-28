---- MODULE SumeragiBlockSyncSelectedQcCacheGate ----
EXTENDS Integers

(***************************************************************************
A bounded boolean model for `cache_block_sync_qc_for_unknown_block(...)`.

The caller has already accepted the block-sync payload enough to keep the QC
for later, but the block is not locally available yet. This helper repeats the
QC shape/lock prefilter, validates the QC against the selected block signers,
calls `process_precommit_qc(..., false, allow_nonextending_qc)`, and only then
caches the commit QC. Because the payload is still missing, successful cached
QCs never apply the commit or advance highest QC; they may only update the
locked QC under the explicit non-extending realignment allowance.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "empty_topology",
  "hash_mismatch",
  "height_mismatch",
  "epoch_mismatch",
  "phase_mismatch",
  "same_height_conflict_drop",
  "same_height_conflict_recoverable",
  "stale_lock_drop",
  "nonextending_defer",
  "nonextending_drop",
  "nonextending_allowed_retain",
  "extending_process_ok",
  "no_lock_process_ok",
  "process_reject_locked",
  "allow_update_no_lock",
  "allow_update_newer",
  "allow_no_update_older",
  "allow_false_no_update",
  "tally_missing_context",
  "tally_final_error"
}

TopologyEmpty(c) ==
  c = "empty_topology"

HashMatches(c) ==
  c # "hash_mismatch"

HeightMatches(c) ==
  c # "height_mismatch"

EpochMatches(c) ==
  c # "epoch_mismatch"

CommitPhase(c) ==
  c # "phase_mismatch"

ShapeOk(c) ==
  /\ ~TopologyEmpty(c)
  /\ HashMatches(c)
  /\ HeightMatches(c)
  /\ EpochMatches(c)
  /\ CommitPhase(c)

LockPresent(c) ==
  c \in {
    "same_height_conflict_drop",
    "same_height_conflict_recoverable",
    "stale_lock_drop",
    "nonextending_defer",
    "nonextending_drop",
    "nonextending_allowed_retain",
    "extending_process_ok",
    "process_reject_locked",
    "allow_update_newer",
    "allow_no_update_older",
    "allow_false_no_update"
  }

SameHeightConflict(c) ==
  c \in {"same_height_conflict_drop", "same_height_conflict_recoverable"}

AllowNonextending(c) ==
  c \in {
    "same_height_conflict_recoverable",
    "nonextending_allowed_retain",
    "allow_update_no_lock",
    "allow_update_newer",
    "allow_no_update_older"
  }

SameHeightRecoverable(c) ==
  SameHeightConflict(c) /\ AllowNonextending(c)

StaleAgainstLock(c) ==
  c = "stale_lock_drop"

ExtendsLocked(c) ==
  c \in {
    "extending_process_ok",
    "no_lock_process_ok",
    "process_reject_locked",
    "allow_update_no_lock",
    "allow_update_newer",
    "allow_no_update_older",
    "allow_false_no_update",
    "tally_missing_context",
    "tally_final_error"
  }

DeferMissingLockedPayload(c) ==
  c = "nonextending_defer"

TallyErrorCase(c) ==
  c \in {"tally_missing_context", "tally_final_error"}

MissingContextError(c) ==
  c = "tally_missing_context"

FinalValidationError(c) ==
  c = "tally_final_error"

ProcessRejectCase(c) ==
  c = "process_reject_locked"

ShouldUpdateLock(c) ==
  c \in {"allow_update_no_lock", "allow_update_newer"}

SpecTopologyRecovery(c) ==
  TopologyEmpty(c)

SpecShapeIgnored(c) ==
  /\ ~TopologyEmpty(c)
  /\ (~HashMatches(c) \/ ~HeightMatches(c) \/ ~EpochMatches(c) \/ ~CommitPhase(c))

SpecSameHeightLockedDrop(c) ==
  ShapeOk(c) /\ SameHeightConflict(c) /\ ~SameHeightRecoverable(c)

SpecLockedPrefilterMetric(c) ==
  SpecSameHeightLockedDrop(c)

SpecStaleLockedDrop(c) ==
  ShapeOk(c) /\ ~SpecSameHeightLockedDrop(c) /\ StaleAgainstLock(c)

SpecExtendsComputed(c) ==
  ShapeOk(c) /\ ~SpecSameHeightLockedDrop(c) /\ ~SpecStaleLockedDrop(c)

SpecNonextendingDefer(c) ==
  /\ SpecExtendsComputed(c)
  /\ ~ExtendsLocked(c)
  /\ ~AllowNonextending(c)
  /\ DeferMissingLockedPayload(c)

SpecNonextendingDrop(c) ==
  /\ SpecExtendsComputed(c)
  /\ ~ExtendsLocked(c)
  /\ ~AllowNonextending(c)
  /\ ~DeferMissingLockedPayload(c)

SpecQuarantineLockedPayload(c) ==
  SpecNonextendingDefer(c)

SpecRecordLockedDrop(c) ==
  SpecSameHeightLockedDrop(c)
    \/ SpecStaleLockedDrop(c)
    \/ SpecNonextendingDefer(c)
    \/ SpecNonextendingDrop(c)

SpecRetainNonextending(c) ==
  /\ SpecExtendsComputed(c)
  /\ ~ExtendsLocked(c)
  /\ AllowNonextending(c)

SpecTallyAttempted(c) ==
  SpecExtendsComputed(c) /\ (ExtendsLocked(c) \/ AllowNonextending(c))

SpecTallyValidationErrorRecorded(c) ==
  SpecTallyAttempted(c) /\ TallyErrorCase(c)

SpecMissingContextQuarantined(c) ==
  SpecTallyAttempted(c) /\ MissingContextError(c)

SpecFinalDrop(c) ==
  SpecTallyAttempted(c) /\ FinalValidationError(c)

SpecTallyOk(c) ==
  SpecTallyAttempted(c) /\ ~TallyErrorCase(c)

SpecRecordPrecommitSigners(c) ==
  SpecTallyOk(c)

SpecNoteValidatedTally(c) ==
  SpecTallyOk(c)

SpecProcessPrecommitAttempted(c) ==
  SpecTallyOk(c)

SpecProcessBlockKnownFalse(c) ==
  SpecProcessPrecommitAttempted(c)

SpecProcessAllowNonextendingArg(c) ==
  IF SpecProcessPrecommitAttempted(c) THEN AllowNonextending(c) ELSE FALSE

SpecProcessRejected(c) ==
  SpecTallyOk(c) /\ ProcessRejectCase(c)

SpecProcessRejectLogsConflict(c) ==
  SpecProcessRejected(c) /\ LockPresent(c)

SpecProcessOk(c) ==
  SpecTallyOk(c) /\ ~ProcessRejectCase(c)

SpecUpdateLockedQc(c) ==
  SpecProcessOk(c) /\ AllowNonextending(c) /\ ShouldUpdateLock(c)

SpecPrunePrecommitVotes(c) ==
  SpecUpdateLockedQc(c)

SpecHighestQcUnchanged(c) ==
  SpecProcessOk(c)

SpecRemoveQuarantinedQc(c) ==
  SpecProcessOk(c)

SpecRecordCommitQc(c) ==
  SpecProcessOk(c)

SpecInsertQcCache(c) ==
  SpecProcessOk(c)

ActualTopologyRecovery(c) ==
  IF Bug = "empty_topology_no_recovery" /\ c = "empty_topology" THEN FALSE
  ELSE SpecTopologyRecovery(c)

ActualShapeIgnored(c) ==
  IF Bug = "hash_mismatch_tallies" /\ c = "hash_mismatch" THEN FALSE
  ELSE IF Bug = "height_mismatch_tallies" /\ c = "height_mismatch" THEN FALSE
  ELSE IF Bug = "epoch_mismatch_tallies" /\ c = "epoch_mismatch" THEN FALSE
  ELSE IF Bug = "phase_mismatch_tallies" /\ c = "phase_mismatch" THEN FALSE
  ELSE SpecShapeIgnored(c)

ActualSameHeightLockedDrop(c) ==
  IF Bug = "same_height_conflict_tallies" /\ c = "same_height_conflict_drop" THEN FALSE
  ELSE SpecSameHeightLockedDrop(c)

ActualLockedPrefilterMetric(c) ==
  IF Bug = "same_height_conflict_no_metric" /\ c = "same_height_conflict_drop" THEN FALSE
  ELSE SpecLockedPrefilterMetric(c)

ActualStaleLockedDrop(c) ==
  IF Bug = "stale_lock_tallies" /\ c = "stale_lock_drop" THEN FALSE
  ELSE SpecStaleLockedDrop(c)

ActualNonextendingDefer(c) ==
  IF Bug = "nonextending_defer_tallies" /\ c = "nonextending_defer" THEN FALSE
  ELSE SpecNonextendingDefer(c)

ActualNonextendingDrop(c) ==
  IF Bug = "nonextending_drop_tallies" /\ c = "nonextending_drop" THEN FALSE
  ELSE SpecNonextendingDrop(c)

ActualQuarantineLockedPayload(c) ==
  IF Bug = "nonextending_defer_not_quarantined" /\ c = "nonextending_defer" THEN FALSE
  ELSE SpecQuarantineLockedPayload(c)

ActualRecordLockedDrop(c) ==
  IF Bug = "nonextending_drop_no_status" /\ c = "nonextending_drop" THEN FALSE
  ELSE SpecRecordLockedDrop(c)

ActualRetainNonextending(c) ==
  IF Bug = "nonextending_allowed_dropped" /\ c = "nonextending_allowed_retain" THEN FALSE
  ELSE SpecRetainNonextending(c)

ActualTallyAttempted(c) ==
  IF Bug = "empty_topology_tallies" /\ c = "empty_topology" THEN TRUE
  ELSE IF Bug = "hash_mismatch_tallies" /\ c = "hash_mismatch" THEN TRUE
  ELSE IF Bug = "height_mismatch_tallies" /\ c = "height_mismatch" THEN TRUE
  ELSE IF Bug = "epoch_mismatch_tallies" /\ c = "epoch_mismatch" THEN TRUE
  ELSE IF Bug = "phase_mismatch_tallies" /\ c = "phase_mismatch" THEN TRUE
  ELSE IF Bug = "same_height_conflict_tallies" /\ c = "same_height_conflict_drop" THEN TRUE
  ELSE IF Bug = "stale_lock_tallies" /\ c = "stale_lock_drop" THEN TRUE
  ELSE IF Bug = "nonextending_defer_tallies" /\ c = "nonextending_defer" THEN TRUE
  ELSE IF Bug = "nonextending_drop_tallies" /\ c = "nonextending_drop" THEN TRUE
  ELSE IF Bug = "nonextending_allowed_dropped" /\ c = "nonextending_allowed_retain" THEN FALSE
  ELSE IF Bug = "fresh_tally_skipped" /\ c = "extending_process_ok" THEN FALSE
  ELSE SpecTallyAttempted(c)

ActualTallyValidationErrorRecorded(c) ==
  SpecTallyValidationErrorRecorded(c)

ActualMissingContextQuarantined(c) ==
  IF Bug = "missing_context_not_quarantined" /\ c = "tally_missing_context" THEN FALSE
  ELSE SpecMissingContextQuarantined(c)

ActualFinalDrop(c) ==
  IF Bug = "missing_context_final_dropped" /\ c = "tally_missing_context" THEN TRUE
  ELSE IF Bug = "final_error_not_dropped" /\ c = "tally_final_error" THEN FALSE
  ELSE SpecFinalDrop(c)

ActualRecordPrecommitSigners(c) ==
  IF Bug = "precommit_signers_not_recorded" /\ c = "extending_process_ok" THEN FALSE
  ELSE SpecRecordPrecommitSigners(c)

ActualNoteValidatedTally(c) ==
  IF Bug = "validated_tally_not_noted" /\ c = "extending_process_ok" THEN FALSE
  ELSE SpecNoteValidatedTally(c)

ActualProcessPrecommitAttempted(c) ==
  IF Bug = "process_not_called" /\ c = "extending_process_ok" THEN FALSE
  ELSE IF Bug = "tally_error_processes" /\ c = "tally_final_error" THEN TRUE
  ELSE SpecProcessPrecommitAttempted(c)

ActualProcessBlockKnownFalse(c) ==
  IF Bug = "process_block_known_true" /\ c = "extending_process_ok" THEN FALSE
  ELSE SpecProcessBlockKnownFalse(c)

ActualProcessAllowNonextendingArg(c) ==
  IF Bug = "allow_nonextending_not_forwarded" /\ c = "allow_update_newer" THEN FALSE
  ELSE SpecProcessAllowNonextendingArg(c)

ActualProcessRejectLogsConflict(c) ==
  SpecProcessRejectLogsConflict(c)

ActualUpdateLockedQc(c) ==
  IF Bug = "lock_update_without_allow" /\ c = "allow_false_no_update" THEN TRUE
  ELSE IF Bug = "lock_update_skipped_when_newer" /\ c = "allow_update_newer" THEN FALSE
  ELSE IF Bug = "lock_update_on_older" /\ c = "allow_no_update_older" THEN TRUE
  ELSE SpecUpdateLockedQc(c)

ActualPrunePrecommitVotes(c) ==
  IF Bug = "lock_update_skipped_when_newer" /\ c = "allow_update_newer" THEN FALSE
  ELSE SpecPrunePrecommitVotes(c)

ActualHighestQcUnchanged(c) ==
  SpecHighestQcUnchanged(c)

ActualRemoveQuarantinedQc(c) ==
  IF Bug = "quarantine_not_removed" /\ c = "extending_process_ok" THEN FALSE
  ELSE SpecRemoveQuarantinedQc(c)

ActualRecordCommitQc(c) ==
  IF Bug = "process_reject_records_commit" /\ c = "process_reject_locked" THEN TRUE
  ELSE IF Bug = "commit_qc_not_recorded" /\ c = "extending_process_ok" THEN FALSE
  ELSE SpecRecordCommitQc(c)

ActualInsertQcCache(c) ==
  IF Bug = "process_reject_inserts_cache" /\ c = "process_reject_locked" THEN TRUE
  ELSE IF Bug = "qc_cache_not_inserted" /\ c = "extending_process_ok" THEN FALSE
  ELSE SpecInsertQcCache(c)

ActualFinalErrorQuarantined(c) ==
  IF Bug = "final_error_quarantined" /\ c = "tally_final_error" THEN TRUE
  ELSE FALSE

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "empty_topology_no_recovery",
       "empty_topology_tallies",
       "hash_mismatch_tallies",
       "height_mismatch_tallies",
       "epoch_mismatch_tallies",
       "phase_mismatch_tallies",
       "same_height_conflict_no_metric",
       "same_height_conflict_tallies",
       "stale_lock_tallies",
       "nonextending_defer_not_quarantined",
       "nonextending_defer_tallies",
       "nonextending_drop_no_status",
       "nonextending_drop_tallies",
       "nonextending_allowed_dropped",
       "fresh_tally_skipped",
       "precommit_signers_not_recorded",
       "validated_tally_not_noted",
       "process_not_called",
       "process_block_known_true",
       "allow_nonextending_not_forwarded",
       "process_reject_records_commit",
       "process_reject_inserts_cache",
       "lock_update_without_allow",
       "lock_update_skipped_when_newer",
       "lock_update_on_older",
       "quarantine_not_removed",
       "commit_qc_not_recorded",
       "qc_cache_not_inserted",
       "missing_context_not_quarantined",
       "missing_context_final_dropped",
       "final_error_not_dropped",
       "final_error_quarantined",
       "tally_error_processes"
     }
  /\ checked = 0

TopologyAndShape ==
  /\ ActualTopologyRecovery("empty_topology") = SpecTopologyRecovery("empty_topology")
  /\ ActualTallyAttempted("empty_topology") = SpecTallyAttempted("empty_topology")
  /\ ActualShapeIgnored("hash_mismatch") = SpecShapeIgnored("hash_mismatch")
  /\ ActualShapeIgnored("height_mismatch") = SpecShapeIgnored("height_mismatch")
  /\ ActualShapeIgnored("epoch_mismatch") = SpecShapeIgnored("epoch_mismatch")
  /\ ActualShapeIgnored("phase_mismatch") = SpecShapeIgnored("phase_mismatch")
  /\ ActualTallyAttempted("hash_mismatch") = SpecTallyAttempted("hash_mismatch")
  /\ ActualTallyAttempted("height_mismatch") = SpecTallyAttempted("height_mismatch")
  /\ ActualTallyAttempted("epoch_mismatch") = SpecTallyAttempted("epoch_mismatch")
  /\ ActualTallyAttempted("phase_mismatch") = SpecTallyAttempted("phase_mismatch")

LockedPrefilter ==
  /\ ActualSameHeightLockedDrop("same_height_conflict_drop")
       = SpecSameHeightLockedDrop("same_height_conflict_drop")
  /\ ActualLockedPrefilterMetric("same_height_conflict_drop")
       = SpecLockedPrefilterMetric("same_height_conflict_drop")
  /\ ActualTallyAttempted("same_height_conflict_drop")
       = SpecTallyAttempted("same_height_conflict_drop")
  /\ ActualSameHeightLockedDrop("same_height_conflict_recoverable")
       = SpecSameHeightLockedDrop("same_height_conflict_recoverable")
  /\ ActualTallyAttempted("same_height_conflict_recoverable")
       = SpecTallyAttempted("same_height_conflict_recoverable")
  /\ ActualStaleLockedDrop("stale_lock_drop") = SpecStaleLockedDrop("stale_lock_drop")
  /\ ActualTallyAttempted("stale_lock_drop") = SpecTallyAttempted("stale_lock_drop")

NonextendingPrefilter ==
  /\ ActualNonextendingDefer("nonextending_defer") = SpecNonextendingDefer("nonextending_defer")
  /\ ActualQuarantineLockedPayload("nonextending_defer")
       = SpecQuarantineLockedPayload("nonextending_defer")
  /\ ActualTallyAttempted("nonextending_defer") = SpecTallyAttempted("nonextending_defer")
  /\ ActualNonextendingDrop("nonextending_drop") = SpecNonextendingDrop("nonextending_drop")
  /\ ActualRecordLockedDrop("nonextending_drop") = SpecRecordLockedDrop("nonextending_drop")
  /\ ActualTallyAttempted("nonextending_drop") = SpecTallyAttempted("nonextending_drop")
  /\ ActualRetainNonextending("nonextending_allowed_retain")
       = SpecRetainNonextending("nonextending_allowed_retain")
  /\ ActualTallyAttempted("nonextending_allowed_retain")
       = SpecTallyAttempted("nonextending_allowed_retain")

TallyAndProcess ==
  /\ ActualTallyAttempted("extending_process_ok") = SpecTallyAttempted("extending_process_ok")
  /\ ActualRecordPrecommitSigners("extending_process_ok")
       = SpecRecordPrecommitSigners("extending_process_ok")
  /\ ActualNoteValidatedTally("extending_process_ok")
       = SpecNoteValidatedTally("extending_process_ok")
  /\ ActualProcessPrecommitAttempted("extending_process_ok")
       = SpecProcessPrecommitAttempted("extending_process_ok")
  /\ ActualProcessBlockKnownFalse("extending_process_ok")
       = SpecProcessBlockKnownFalse("extending_process_ok")
  /\ ActualProcessAllowNonextendingArg("allow_update_newer")
       = SpecProcessAllowNonextendingArg("allow_update_newer")
  /\ ActualProcessPrecommitAttempted("tally_final_error")
       = SpecProcessPrecommitAttempted("tally_final_error")

ProcessReject ==
  /\ ActualProcessRejectLogsConflict("process_reject_locked")
       = SpecProcessRejectLogsConflict("process_reject_locked")
  /\ ActualRecordCommitQc("process_reject_locked") = SpecRecordCommitQc("process_reject_locked")
  /\ ActualInsertQcCache("process_reject_locked") = SpecInsertQcCache("process_reject_locked")

LockUpdateAndCache ==
  /\ ActualUpdateLockedQc("allow_false_no_update") = SpecUpdateLockedQc("allow_false_no_update")
  /\ ActualUpdateLockedQc("allow_update_newer") = SpecUpdateLockedQc("allow_update_newer")
  /\ ActualPrunePrecommitVotes("allow_update_newer") = SpecPrunePrecommitVotes("allow_update_newer")
  /\ ActualUpdateLockedQc("allow_no_update_older") = SpecUpdateLockedQc("allow_no_update_older")
  /\ ActualHighestQcUnchanged("extending_process_ok") = SpecHighestQcUnchanged("extending_process_ok")
  /\ ActualRemoveQuarantinedQc("extending_process_ok")
       = SpecRemoveQuarantinedQc("extending_process_ok")
  /\ ActualRecordCommitQc("extending_process_ok") = SpecRecordCommitQc("extending_process_ok")
  /\ ActualInsertQcCache("extending_process_ok") = SpecInsertQcCache("extending_process_ok")

ValidationError ==
  /\ ActualTallyValidationErrorRecorded("tally_missing_context")
       = SpecTallyValidationErrorRecorded("tally_missing_context")
  /\ ActualMissingContextQuarantined("tally_missing_context")
       = SpecMissingContextQuarantined("tally_missing_context")
  /\ ActualFinalDrop("tally_missing_context") = SpecFinalDrop("tally_missing_context")
  /\ ActualTallyValidationErrorRecorded("tally_final_error")
       = SpecTallyValidationErrorRecorded("tally_final_error")
  /\ ActualFinalDrop("tally_final_error") = SpecFinalDrop("tally_final_error")
  /\ ActualFinalErrorQuarantined("tally_final_error") = FALSE

BugWitness ==
  IF Bug = "empty_topology_no_recovery" THEN
    ActualTopologyRecovery("empty_topology")
  ELSE IF Bug = "empty_topology_tallies" THEN
    ~ActualTallyAttempted("empty_topology")
  ELSE IF Bug = "hash_mismatch_tallies" THEN
    ~ActualTallyAttempted("hash_mismatch")
  ELSE IF Bug = "height_mismatch_tallies" THEN
    ~ActualTallyAttempted("height_mismatch")
  ELSE IF Bug = "epoch_mismatch_tallies" THEN
    ~ActualTallyAttempted("epoch_mismatch")
  ELSE IF Bug = "phase_mismatch_tallies" THEN
    ~ActualTallyAttempted("phase_mismatch")
  ELSE IF Bug = "same_height_conflict_no_metric" THEN
    ActualLockedPrefilterMetric("same_height_conflict_drop")
  ELSE IF Bug = "same_height_conflict_tallies" THEN
    ~ActualTallyAttempted("same_height_conflict_drop")
  ELSE IF Bug = "stale_lock_tallies" THEN
    ~ActualTallyAttempted("stale_lock_drop")
  ELSE IF Bug = "nonextending_defer_not_quarantined" THEN
    ActualQuarantineLockedPayload("nonextending_defer")
  ELSE IF Bug = "nonextending_defer_tallies" THEN
    ~ActualTallyAttempted("nonextending_defer")
  ELSE IF Bug = "nonextending_drop_no_status" THEN
    ActualRecordLockedDrop("nonextending_drop")
  ELSE IF Bug = "nonextending_drop_tallies" THEN
    ~ActualTallyAttempted("nonextending_drop")
  ELSE IF Bug = "nonextending_allowed_dropped" THEN
    ActualTallyAttempted("nonextending_allowed_retain")
  ELSE IF Bug = "fresh_tally_skipped" THEN
    ActualTallyAttempted("extending_process_ok")
  ELSE IF Bug = "precommit_signers_not_recorded" THEN
    ActualRecordPrecommitSigners("extending_process_ok")
  ELSE IF Bug = "validated_tally_not_noted" THEN
    ActualNoteValidatedTally("extending_process_ok")
  ELSE IF Bug = "process_not_called" THEN
    ActualProcessPrecommitAttempted("extending_process_ok")
  ELSE IF Bug = "process_block_known_true" THEN
    ActualProcessBlockKnownFalse("extending_process_ok")
  ELSE IF Bug = "allow_nonextending_not_forwarded" THEN
    ActualProcessAllowNonextendingArg("allow_update_newer")
  ELSE IF Bug = "process_reject_records_commit" THEN
    ~ActualRecordCommitQc("process_reject_locked")
  ELSE IF Bug = "process_reject_inserts_cache" THEN
    ~ActualInsertQcCache("process_reject_locked")
  ELSE IF Bug = "lock_update_without_allow" THEN
    ~ActualUpdateLockedQc("allow_false_no_update")
  ELSE IF Bug = "lock_update_skipped_when_newer" THEN
    ActualUpdateLockedQc("allow_update_newer")
  ELSE IF Bug = "lock_update_on_older" THEN
    ~ActualUpdateLockedQc("allow_no_update_older")
  ELSE IF Bug = "quarantine_not_removed" THEN
    ActualRemoveQuarantinedQc("extending_process_ok")
  ELSE IF Bug = "commit_qc_not_recorded" THEN
    ActualRecordCommitQc("extending_process_ok")
  ELSE IF Bug = "qc_cache_not_inserted" THEN
    ActualInsertQcCache("extending_process_ok")
  ELSE IF Bug = "missing_context_not_quarantined" THEN
    ActualMissingContextQuarantined("tally_missing_context")
  ELSE IF Bug = "missing_context_final_dropped" THEN
    ~ActualFinalDrop("tally_missing_context")
  ELSE IF Bug = "final_error_not_dropped" THEN
    ActualFinalDrop("tally_final_error")
  ELSE IF Bug = "final_error_quarantined" THEN
    ActualFinalErrorQuarantined("tally_final_error") = FALSE
  ELSE IF Bug = "tally_error_processes" THEN
    ~ActualProcessPrecommitAttempted("tally_final_error")
  ELSE TRUE

BlockSyncSelectedQcCacheExactness ==
  /\ TopologyAndShape
  /\ LockedPrefilter
  /\ NonextendingPrefilter
  /\ TallyAndProcess
  /\ ProcessReject
  /\ LockUpdateAndCache
  /\ ValidationError

BlockSyncSelectedQcCacheCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncSelectedQcCacheExactness

SafetyFast ==
  BlockSyncSelectedQcCacheExactness

=============================================================================
====
