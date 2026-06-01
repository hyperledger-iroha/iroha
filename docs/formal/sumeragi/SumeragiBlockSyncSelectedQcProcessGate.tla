---- MODULE SumeragiBlockSyncSelectedQcProcessGate ----
EXTENDS Integers

(***************************************************************************
A bounded boolean model for the selected-roster BlockSyncUpdate QC processing
path after the post-apply prefilter has admitted the QC to signer tally.

The live path first reuses a cached signer tally when available, otherwise it
validates the QC against the selected block signers. Successful tallies record
precommit signer evidence, cache the validated tally, forward the exact
`block_known_for_commit` and `allow_nonextending_qc` arguments to
`process_precommit_qc(...)`, then only cache/apply the commit QC when that
processing accepts it. Known blocks are committed immediately and request the
commit pipeline; unknown or not-yet-valid pending blocks only remember the
observed QC epoch. If the block payload was accepted but is still unknown, the
outer wrapper must hand the QC to `cache_block_sync_qc_for_unknown_block(...)`.
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
  "cached_tally_hit_known_pending",
  "fresh_tally_known_pending",
  "fresh_tally_known_inflight",
  "fresh_tally_known_kura",
  "fresh_tally_unknown_pending",
  "fresh_tally_unknown_no_pending",
  "process_reject",
  "tally_error",
  "runtime_da_cleanup",
  "runtime_da_disabled",
  "allow_nonextending_forwarded",
  "ready_without_qc",
  "creation_ok_unknown_cache",
  "creation_error_no_cache"
}

ReadyApplyCase(c) ==
  c \in {
    "cached_tally_hit_known_pending",
    "fresh_tally_known_pending",
    "fresh_tally_known_inflight",
    "fresh_tally_known_kura",
    "fresh_tally_unknown_pending",
    "fresh_tally_unknown_no_pending",
    "process_reject",
    "tally_error",
    "runtime_da_cleanup",
    "runtime_da_disabled",
    "allow_nonextending_forwarded"
  }

CachedTallyCase(c) ==
  c = "cached_tally_hit_known_pending"

TallyErrorCase(c) ==
  c = "tally_error"

TallyOk(c) ==
  ReadyApplyCase(c) /\ ~TallyErrorCase(c)

ProcessRejectCase(c) ==
  c = "process_reject"

ProcessOk(c) ==
  TallyOk(c) /\ ~ProcessRejectCase(c)

BlockKnownForCommit(c) ==
  c \in {
    "cached_tally_hit_known_pending",
    "fresh_tally_known_pending",
    "fresh_tally_known_inflight",
    "fresh_tally_known_kura",
    "process_reject",
    "runtime_da_cleanup",
    "runtime_da_disabled",
    "allow_nonextending_forwarded"
  }

PendingEntryExists(c) ==
  c = "fresh_tally_unknown_pending"

RuntimeDaEnabled(c) ==
  c = "runtime_da_cleanup"

AllowNonextendingInput(c) ==
  c = "allow_nonextending_forwarded"

SpecCachedTallyReused(c) ==
  CachedTallyCase(c)

SpecFreshTallyCalled(c) ==
  ReadyApplyCase(c) /\ ~CachedTallyCase(c)

SpecRecordTallyValidationError(c) ==
  TallyErrorCase(c)

SpecRecordPrecommitSigners(c) ==
  TallyOk(c)

SpecNoteValidatedTally(c) ==
  TallyOk(c)

SpecProcessPrecommitAttempted(c) ==
  TallyOk(c)

SpecProcessBlockKnownArg(c) ==
  IF SpecProcessPrecommitAttempted(c) THEN BlockKnownForCommit(c) ELSE FALSE

SpecProcessAllowNonextendingArg(c) ==
  IF SpecProcessPrecommitAttempted(c) THEN AllowNonextendingInput(c) ELSE FALSE

SpecRecordCommitQc(c) ==
  ProcessOk(c)

SpecInsertQcCache(c) ==
  ProcessOk(c)

SpecApplyCommitQc(c) ==
  ProcessOk(c) /\ BlockKnownForCommit(c)

SpecCleanRbcSessions(c) ==
  SpecApplyCommitQc(c) /\ RuntimeDaEnabled(c)

SpecRequestCommitPipeline(c) ==
  SpecApplyCommitQc(c)

SpecObservePendingEpoch(c) ==
  ProcessOk(c) /\ ~BlockKnownForCommit(c) /\ PendingEntryExists(c)

SpecUnknownBlockCacheCalled(c) ==
  c = "creation_ok_unknown_cache"

SpecWrapperReturnsErr(c) ==
  c = "creation_error_no_cache"

SpecNoQcNoTally(c) ==
  c = "ready_without_qc"

ActualCachedTallyReused(c) ==
  IF Bug = "cached_tally_revalidates"
     /\ c = "cached_tally_hit_known_pending"
  THEN FALSE
  ELSE SpecCachedTallyReused(c)

ActualFreshTallyCalled(c) ==
  IF Bug = "cached_tally_revalidates"
     /\ c = "cached_tally_hit_known_pending"
  THEN TRUE
  ELSE IF Bug = "fresh_tally_skipped"
          /\ c = "fresh_tally_known_pending" THEN FALSE
  ELSE IF Bug = "no_qc_tallies"
          /\ c = "ready_without_qc" THEN TRUE
  ELSE SpecFreshTallyCalled(c)

ActualRecordTallyValidationError(c) ==
  SpecRecordTallyValidationError(c)

ActualRecordPrecommitSigners(c) ==
  IF Bug = "precommit_signers_not_recorded"
     /\ c = "fresh_tally_known_pending"
  THEN FALSE
  ELSE IF Bug = "tally_error_records_signers"
          /\ c = "tally_error" THEN TRUE
  ELSE SpecRecordPrecommitSigners(c)

ActualNoteValidatedTally(c) ==
  IF Bug = "validated_tally_not_noted"
     /\ c = "fresh_tally_known_pending"
  THEN FALSE
  ELSE SpecNoteValidatedTally(c)

ActualProcessPrecommitAttempted(c) ==
  IF Bug = "process_not_called"
     /\ c = "fresh_tally_known_pending"
  THEN FALSE
  ELSE IF Bug = "tally_error_processes"
          /\ c = "tally_error" THEN TRUE
  ELSE SpecProcessPrecommitAttempted(c)

ActualProcessBlockKnownArg(c) ==
  IF Bug = "block_known_arg_wrong"
     /\ c = "fresh_tally_known_inflight"
  THEN FALSE
  ELSE SpecProcessBlockKnownArg(c)

ActualProcessAllowNonextendingArg(c) ==
  IF Bug = "allow_nonextending_not_forwarded"
     /\ c = "allow_nonextending_forwarded"
  THEN FALSE
  ELSE SpecProcessAllowNonextendingArg(c)

ActualRecordCommitQc(c) ==
  IF Bug = "commit_qc_not_recorded"
     /\ c = "fresh_tally_known_pending"
  THEN FALSE
  ELSE IF Bug = "process_reject_records_commit"
          /\ c = "process_reject" THEN TRUE
  ELSE SpecRecordCommitQc(c)

ActualInsertQcCache(c) ==
  IF Bug = "qc_cache_not_inserted"
     /\ c = "fresh_tally_known_pending"
  THEN FALSE
  ELSE IF Bug = "process_reject_inserts_cache"
          /\ c = "process_reject" THEN TRUE
  ELSE SpecInsertQcCache(c)

ActualApplyCommitQc(c) ==
  IF Bug = "known_commit_not_applied"
     /\ c = "fresh_tally_known_kura"
  THEN FALSE
  ELSE IF Bug = "unknown_commit_applied"
          /\ c = "fresh_tally_unknown_no_pending" THEN TRUE
  ELSE SpecApplyCommitQc(c)

ActualCleanRbcSessions(c) ==
  IF Bug = "runtime_da_cleanup_skipped"
     /\ c = "runtime_da_cleanup"
  THEN FALSE
  ELSE IF Bug = "runtime_da_cleanup_without_da"
          /\ c = "runtime_da_disabled" THEN TRUE
  ELSE SpecCleanRbcSessions(c)

ActualRequestCommitPipeline(c) ==
  IF Bug = "commit_pipeline_not_requested"
     /\ c = "fresh_tally_known_pending"
  THEN FALSE
  ELSE SpecRequestCommitPipeline(c)

ActualObservePendingEpoch(c) ==
  IF Bug = "pending_epoch_not_observed"
     /\ c = "fresh_tally_unknown_pending"
  THEN FALSE
  ELSE IF Bug = "pending_epoch_observed_for_known"
          /\ c = "fresh_tally_known_pending" THEN TRUE
  ELSE SpecObservePendingEpoch(c)

ActualUnknownBlockCacheCalled(c) ==
  IF Bug = "unknown_cache_skipped"
     /\ c = "creation_ok_unknown_cache"
  THEN FALSE
  ELSE IF Bug = "creation_error_caches"
          /\ c = "creation_error_no_cache" THEN TRUE
  ELSE SpecUnknownBlockCacheCalled(c)

ActualWrapperReturnsErr(c) ==
  SpecWrapperReturnsErr(c)

Matches(c) ==
  /\ ActualCachedTallyReused(c) = SpecCachedTallyReused(c)
  /\ ActualFreshTallyCalled(c) = SpecFreshTallyCalled(c)
  /\ ActualRecordTallyValidationError(c) = SpecRecordTallyValidationError(c)
  /\ ActualRecordPrecommitSigners(c) = SpecRecordPrecommitSigners(c)
  /\ ActualNoteValidatedTally(c) = SpecNoteValidatedTally(c)
  /\ ActualProcessPrecommitAttempted(c) = SpecProcessPrecommitAttempted(c)
  /\ ActualProcessBlockKnownArg(c) = SpecProcessBlockKnownArg(c)
  /\ ActualProcessAllowNonextendingArg(c) = SpecProcessAllowNonextendingArg(c)
  /\ ActualRecordCommitQc(c) = SpecRecordCommitQc(c)
  /\ ActualInsertQcCache(c) = SpecInsertQcCache(c)
  /\ ActualApplyCommitQc(c) = SpecApplyCommitQc(c)
  /\ ActualCleanRbcSessions(c) = SpecCleanRbcSessions(c)
  /\ ActualRequestCommitPipeline(c) = SpecRequestCommitPipeline(c)
  /\ ActualObservePendingEpoch(c) = SpecObservePendingEpoch(c)
  /\ ActualUnknownBlockCacheCalled(c) = SpecUnknownBlockCacheCalled(c)
  /\ ActualWrapperReturnsErr(c) = SpecWrapperReturnsErr(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "cached_tally_revalidates",
       "fresh_tally_skipped",
       "no_qc_tallies",
       "precommit_signers_not_recorded",
       "validated_tally_not_noted",
       "tally_error_records_signers",
       "tally_error_processes",
       "process_not_called",
       "block_known_arg_wrong",
       "allow_nonextending_not_forwarded",
       "process_reject_records_commit",
       "process_reject_inserts_cache",
       "commit_qc_not_recorded",
       "qc_cache_not_inserted",
       "known_commit_not_applied",
       "unknown_commit_applied",
       "runtime_da_cleanup_skipped",
       "runtime_da_cleanup_without_da",
       "commit_pipeline_not_requested",
       "pending_epoch_not_observed",
       "pending_epoch_observed_for_known",
       "unknown_cache_skipped",
       "creation_error_caches"
     }
  /\ checked = 0

TallySource ==
  /\ ActualCachedTallyReused("cached_tally_hit_known_pending")
       = SpecCachedTallyReused("cached_tally_hit_known_pending")
  /\ ActualFreshTallyCalled("cached_tally_hit_known_pending")
       = SpecFreshTallyCalled("cached_tally_hit_known_pending")
  /\ ActualFreshTallyCalled("fresh_tally_known_pending")
       = SpecFreshTallyCalled("fresh_tally_known_pending")
  /\ ActualFreshTallyCalled("ready_without_qc")
       = SpecFreshTallyCalled("ready_without_qc")

TallyResult ==
  /\ ActualRecordPrecommitSigners("fresh_tally_known_pending")
       = SpecRecordPrecommitSigners("fresh_tally_known_pending")
  /\ ActualNoteValidatedTally("fresh_tally_known_pending")
       = SpecNoteValidatedTally("fresh_tally_known_pending")
  /\ ActualRecordTallyValidationError("tally_error")
       = SpecRecordTallyValidationError("tally_error")
  /\ ActualRecordPrecommitSigners("tally_error")
       = SpecRecordPrecommitSigners("tally_error")

ProcessArgs ==
  /\ ActualProcessPrecommitAttempted("fresh_tally_known_pending")
       = SpecProcessPrecommitAttempted("fresh_tally_known_pending")
  /\ ActualProcessPrecommitAttempted("tally_error")
       = SpecProcessPrecommitAttempted("tally_error")
  /\ ActualProcessBlockKnownArg("fresh_tally_known_inflight")
       = SpecProcessBlockKnownArg("fresh_tally_known_inflight")
  /\ ActualProcessAllowNonextendingArg("allow_nonextending_forwarded")
       = SpecProcessAllowNonextendingArg("allow_nonextending_forwarded")

ProcessOutcome ==
  /\ ActualRecordCommitQc("fresh_tally_known_pending")
       = SpecRecordCommitQc("fresh_tally_known_pending")
  /\ ActualInsertQcCache("fresh_tally_known_pending")
       = SpecInsertQcCache("fresh_tally_known_pending")
  /\ ActualRecordCommitQc("process_reject") = SpecRecordCommitQc("process_reject")
  /\ ActualInsertQcCache("process_reject") = SpecInsertQcCache("process_reject")

CommitApply ==
  /\ ActualApplyCommitQc("fresh_tally_known_kura") = SpecApplyCommitQc("fresh_tally_known_kura")
  /\ ActualApplyCommitQc("fresh_tally_unknown_no_pending")
       = SpecApplyCommitQc("fresh_tally_unknown_no_pending")
  /\ ActualCleanRbcSessions("runtime_da_cleanup") = SpecCleanRbcSessions("runtime_da_cleanup")
  /\ ActualCleanRbcSessions("runtime_da_disabled") = SpecCleanRbcSessions("runtime_da_disabled")
  /\ ActualRequestCommitPipeline("fresh_tally_known_pending")
       = SpecRequestCommitPipeline("fresh_tally_known_pending")
  /\ ActualObservePendingEpoch("fresh_tally_unknown_pending")
       = SpecObservePendingEpoch("fresh_tally_unknown_pending")
  /\ ActualObservePendingEpoch("fresh_tally_known_pending")
       = SpecObservePendingEpoch("fresh_tally_known_pending")

WrapperCache ==
  /\ ActualUnknownBlockCacheCalled("creation_ok_unknown_cache")
       = SpecUnknownBlockCacheCalled("creation_ok_unknown_cache")
  /\ ActualUnknownBlockCacheCalled("creation_error_no_cache")
       = SpecUnknownBlockCacheCalled("creation_error_no_cache")
  /\ ActualWrapperReturnsErr("creation_error_no_cache")
       = SpecWrapperReturnsErr("creation_error_no_cache")

SafetyFast ==
  /\ TallySource
  /\ TallyResult
  /\ ProcessArgs
  /\ ProcessOutcome
  /\ CommitApply
  /\ WrapperCache

=============================================================================
====
