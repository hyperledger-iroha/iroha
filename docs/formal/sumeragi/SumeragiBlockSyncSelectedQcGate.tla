---- MODULE SumeragiBlockSyncSelectedQcGate ----
EXTENDS Integers

(***************************************************************************
A bounded boolean model for the selected-roster QC candidate/evidence gate in
`handle_block_sync_update(...)`.

After selected-roster signature handling, the live path chooses a commit-QC
candidate in this order:

  incoming QC, selected commit QC, checkpoint-derived QC, world-derived QC,
  cached QC.

The candidate must match the block height, hash, epoch, and COMMIT phase before
validation.  Validation errors with missing local context quarantine the QC;
other validation errors are final drops.  Incoming-QC validation failures may
recover from an already-valid cached QC or, if the original candidate survived
shape filtering, from aggregate-signature fallback.  Hard locked-QC conflicts
strip QC evidence, validated usable QCs are cached, commit-cert presence depends
on a selected commit-QC hint plus validated QC evidence, and invalid incoming-QC
updates with neither quorum nor checkpoint evidence are dropped as invalid
payloads.
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
  "incoming_preempts_selection",
  "selection_preempts_checkpoint",
  "checkpoint_preempts_world",
  "world_preempts_cached",
  "cached_valid",
  "no_source_cached_recovery",
  "incoming_shape_height",
  "incoming_shape_hash",
  "incoming_shape_epoch",
  "incoming_shape_phase",
  "incoming_missing_context",
  "incoming_final_invalid_cached_recovery",
  "incoming_final_invalid_aggregate_fallback",
  "incoming_final_invalid_no_recovery_drop",
  "selection_final_invalid_no_recovery",
  "cached_match_skips_aggregate",
  "selection_match_skips_aggregate",
  "hard_lock_conflict",
  "invalid_qc_block_quorum",
  "invalid_qc_checkpoint"
}

IncomingHint(c) ==
  c \in {
    "incoming_preempts_selection",
    "incoming_shape_height",
    "incoming_shape_hash",
    "incoming_shape_epoch",
    "incoming_shape_phase",
    "incoming_missing_context",
    "incoming_final_invalid_cached_recovery",
    "incoming_final_invalid_aggregate_fallback",
    "incoming_final_invalid_no_recovery_drop",
    "cached_match_skips_aggregate",
    "hard_lock_conflict",
    "invalid_qc_block_quorum",
    "invalid_qc_checkpoint"
  }

SelectionHint(c) ==
  c \in {
    "incoming_preempts_selection",
    "selection_preempts_checkpoint",
    "selection_final_invalid_no_recovery",
    "selection_match_skips_aggregate"
  }

CheckpointHint(c) ==
  c \in {
    "selection_preempts_checkpoint",
    "checkpoint_preempts_world",
    "invalid_qc_checkpoint"
  }

CheckpointConverts(c) ==
  c = "checkpoint_preempts_world"

WorldAvailable(c) ==
  c \in {"checkpoint_preempts_world", "world_preempts_cached"}

CachedSourceAvailable(c) ==
  c \in {"world_preempts_cached", "cached_valid"}

CachedRecoveryAvailable(c) ==
  c \in {
    "no_source_cached_recovery",
    "incoming_final_invalid_cached_recovery"
  }

ShapeValid(c) ==
  ~(c \in {
      "incoming_shape_height",
      "incoming_shape_hash",
      "incoming_shape_epoch",
      "incoming_shape_phase",
      "invalid_qc_block_quorum",
      "invalid_qc_checkpoint"
    })

ValidationOk(c) ==
  c \in {
    "incoming_preempts_selection",
    "selection_preempts_checkpoint",
    "checkpoint_preempts_world",
    "world_preempts_cached",
    "cached_valid",
    "cached_match_skips_aggregate",
    "selection_match_skips_aggregate",
    "hard_lock_conflict"
  }

MissingContextError(c) ==
  c = "incoming_missing_context"

FinalValidationError(c) ==
  c \in {
    "incoming_final_invalid_cached_recovery",
    "incoming_final_invalid_aggregate_fallback",
    "incoming_final_invalid_no_recovery_drop",
    "selection_final_invalid_no_recovery"
  }

CachedMatch(c) ==
  c = "cached_match_skips_aggregate"

SelectionMatch(c) ==
  c \in {
    "selection_preempts_checkpoint",
    "selection_match_skips_aggregate"
  }

AggregateFallbackOk(c) ==
  c = "incoming_final_invalid_aggregate_fallback"

HardLockConflict(c) ==
  c = "hard_lock_conflict"

BlockQuorumMet(c) ==
  c = "invalid_qc_block_quorum"

SpecSourceIncoming(c) == IncomingHint(c)
SpecSourceSelection(c) == ~IncomingHint(c) /\ SelectionHint(c)
SpecSourceCheckpoint(c) == ~IncomingHint(c) /\ ~SelectionHint(c) /\ CheckpointConverts(c)
SpecSourceWorld(c) ==
  /\ ~IncomingHint(c)
  /\ ~SelectionHint(c)
  /\ ~CheckpointConverts(c)
  /\ WorldAvailable(c)
SpecSourceCached(c) ==
  /\ ~IncomingHint(c)
  /\ ~SelectionHint(c)
  /\ ~CheckpointConverts(c)
  /\ ~WorldAvailable(c)
  /\ CachedSourceAvailable(c)

SpecAnySource(c) ==
  \/ SpecSourceIncoming(c)
  \/ SpecSourceSelection(c)
  \/ SpecSourceCheckpoint(c)
  \/ SpecSourceWorld(c)
  \/ SpecSourceCached(c)

SpecCandidateKept(c) == SpecAnySource(c) /\ ShapeValid(c)
SpecValidatesCandidate(c) == SpecCandidateKept(c)
SpecAggregateOkCached(c) == SpecCandidateKept(c) /\ CachedMatch(c)
SpecAggregateOkSelection(c) == SpecCandidateKept(c) /\ ~CachedMatch(c) /\ SelectionMatch(c)
SpecQuarantinesMissingContext(c) == SpecCandidateKept(c) /\ MissingContextError(c)
SpecFinalDropsQc(c) == SpecCandidateKept(c) /\ FinalValidationError(c)
SpecQcReplacedMetric(c) ==
  IncomingHint(c) /\ (MissingContextError(c) \/ FinalValidationError(c))
SpecCandidateValidated(c) == SpecCandidateKept(c) /\ ValidationOk(c)
SpecDerivesCachedQc(c) ==
  /\ (~SpecCandidateKept(c) \/ (SpecCandidateKept(c) /\ ~ValidationOk(c) /\ IncomingHint(c)))
  /\ CachedRecoveryAvailable(c)
SpecIncomingQcValidated(c) == SpecCandidateValidated(c) \/ SpecDerivesCachedQc(c)
SpecAggregateFallbackAttempted(c) == IncomingHint(c) /\ ~SpecIncomingQcValidated(c)
SpecAggregateFallbackAccepted(c) ==
  /\ SpecAggregateFallbackAttempted(c)
  /\ SpecCandidateKept(c)
  /\ AggregateFallbackOk(c)
SpecEvidenceBeforeLock(c) == SpecIncomingQcValidated(c) \/ SpecAggregateFallbackAccepted(c)
SpecLockedConflictDrop(c) == SpecEvidenceBeforeLock(c) /\ HardLockConflict(c)
SpecQcEvidencePresent(c) == SpecEvidenceBeforeLock(c) /\ ~HardLockConflict(c)
SpecUsableQcCached(c) == SpecIncomingQcValidated(c) /\ ~HardLockConflict(c)
SpecQuarantineCleared(c) == SpecUsableQcCached(c)
SpecCommitCertPresent(c) ==
  SelectionHint(c) /\ SpecIncomingQcValidated(c) /\ ~HardLockConflict(c)
SpecInvalidQcPresent(c) ==
  IncomingHint(c) /\ ~SpecIncomingQcValidated(c) /\ ~SpecQcEvidencePresent(c)
SpecDropsInvalidPayload(c) ==
  /\ SpecInvalidQcPresent(c)
  /\ ~BlockQuorumMet(c)
  /\ ~SpecCommitCertPresent(c)
  /\ ~CheckpointHint(c)
SpecInvalidPayloadReturnsOk(c) == SpecDropsInvalidPayload(c)
SpecClearsMissing(c) == FALSE

ActualSourceIncoming(c) ==
  IF Bug = "incoming_not_preferred"
     /\ c = "incoming_preempts_selection"
  THEN FALSE
  ELSE SpecSourceIncoming(c)

ActualSourceSelection(c) ==
  IF Bug = "incoming_not_preferred"
     /\ c = "incoming_preempts_selection"
  THEN TRUE
  ELSE IF Bug = "selection_not_preferred"
          /\ c = "selection_preempts_checkpoint" THEN FALSE
  ELSE SpecSourceSelection(c)

ActualSourceCheckpoint(c) ==
  IF Bug = "selection_not_preferred"
     /\ c = "selection_preempts_checkpoint"
  THEN TRUE
  ELSE IF Bug = "checkpoint_not_preferred"
          /\ c = "checkpoint_preempts_world" THEN FALSE
  ELSE SpecSourceCheckpoint(c)

ActualSourceWorld(c) ==
  IF Bug = "checkpoint_not_preferred"
     /\ c = "checkpoint_preempts_world"
  THEN TRUE
  ELSE IF Bug = "world_not_preferred"
          /\ c = "world_preempts_cached" THEN FALSE
  ELSE SpecSourceWorld(c)

ActualSourceCached(c) ==
  IF Bug = "world_not_preferred"
     /\ c = "world_preempts_cached"
  THEN TRUE
  ELSE IF Bug = "cached_source_ignored"
          /\ c = "cached_valid" THEN FALSE
  ELSE SpecSourceCached(c)

ActualCandidateKept(c) ==
  IF Bug = "bad_height_kept"
     /\ c = "incoming_shape_height"
  THEN TRUE
  ELSE IF Bug = "bad_hash_kept"
          /\ c = "incoming_shape_hash" THEN TRUE
  ELSE IF Bug = "bad_epoch_kept"
          /\ c = "incoming_shape_epoch" THEN TRUE
  ELSE IF Bug = "bad_phase_kept"
          /\ c = "incoming_shape_phase" THEN TRUE
  ELSE SpecCandidateKept(c)

ActualValidatesCandidate(c) ==
  IF Bug = "shape_drop_validates"
     /\ c = "incoming_shape_height"
  THEN TRUE
  ELSE SpecValidatesCandidate(c)

ActualAggregateOkCached(c) ==
  IF Bug = "cached_match_no_aggregate_ok"
     /\ c = "cached_match_skips_aggregate"
  THEN FALSE
  ELSE SpecAggregateOkCached(c)

ActualAggregateOkSelection(c) ==
  IF Bug = "selection_match_no_aggregate_ok"
     /\ c = "selection_match_skips_aggregate"
  THEN FALSE
  ELSE SpecAggregateOkSelection(c)

ActualQuarantinesMissingContext(c) ==
  IF Bug = "missing_context_not_quarantined"
     /\ c = "incoming_missing_context"
  THEN FALSE
  ELSE SpecQuarantinesMissingContext(c)

ActualFinalDropsQc(c) ==
  IF Bug = "missing_context_final_dropped"
     /\ c = "incoming_missing_context"
  THEN TRUE
  ELSE IF Bug = "final_invalid_not_dropped"
          /\ c = "incoming_final_invalid_no_recovery_drop" THEN FALSE
  ELSE SpecFinalDropsQc(c)

ActualQcReplacedMetric(c) ==
  IF Bug = "validation_error_no_replaced_metric"
     /\ c = "incoming_final_invalid_no_recovery_drop"
  THEN FALSE
  ELSE SpecQcReplacedMetric(c)

ActualDerivesCachedQc(c) ==
  IF Bug = "no_source_skips_cached_recovery"
     /\ c = "no_source_cached_recovery"
  THEN FALSE
  ELSE IF Bug = "incoming_invalid_no_cached_recovery"
          /\ c = "incoming_final_invalid_cached_recovery" THEN FALSE
  ELSE IF Bug = "selection_invalid_uses_cached_recovery"
          /\ c = "selection_final_invalid_no_recovery" THEN TRUE
  ELSE SpecDerivesCachedQc(c)

ActualIncomingQcValidated(c) ==
  IF Bug = "selection_invalid_uses_cached_recovery"
     /\ c = "selection_final_invalid_no_recovery"
  THEN TRUE
  ELSE SpecIncomingQcValidated(c)

ActualAggregateFallbackAttempted(c) ==
  IF Bug = "aggregate_fallback_not_attempted"
     /\ c = "incoming_final_invalid_aggregate_fallback"
  THEN FALSE
  ELSE SpecAggregateFallbackAttempted(c)

ActualAggregateFallbackAccepted(c) ==
  IF Bug = "aggregate_fallback_without_original"
     /\ c = "incoming_shape_height"
  THEN TRUE
  ELSE IF Bug = "aggregate_fallback_not_accepted"
          /\ c = "incoming_final_invalid_aggregate_fallback" THEN FALSE
  ELSE SpecAggregateFallbackAccepted(c)

ActualLockedConflictDrop(c) ==
  IF Bug = "hard_lock_no_status"
     /\ c = "hard_lock_conflict"
  THEN FALSE
  ELSE SpecLockedConflictDrop(c)

ActualQcEvidencePresent(c) ==
  IF Bug = "hard_lock_keeps_evidence"
     /\ c = "hard_lock_conflict"
  THEN TRUE
  ELSE SpecQcEvidencePresent(c)

ActualUsableQcCached(c) ==
  IF Bug = "usable_qc_not_cached"
     /\ c = "incoming_preempts_selection"
  THEN FALSE
  ELSE IF Bug = "unusable_qc_cached"
          /\ c = "incoming_final_invalid_aggregate_fallback" THEN TRUE
  ELSE SpecUsableQcCached(c)

ActualQuarantineCleared(c) ==
  IF Bug = "usable_qc_not_cached"
     /\ c = "incoming_preempts_selection"
  THEN FALSE
  ELSE IF Bug = "unusable_qc_cached"
          /\ c = "incoming_final_invalid_aggregate_fallback" THEN TRUE
  ELSE SpecQuarantineCleared(c)

ActualCommitCertPresent(c) ==
  IF Bug = "commit_cert_dropped_for_valid_selection"
     /\ c = "selection_preempts_checkpoint"
  THEN FALSE
  ELSE IF Bug = "commit_cert_without_validation"
          /\ c = "selection_final_invalid_no_recovery" THEN TRUE
  ELSE SpecCommitCertPresent(c)

ActualInvalidQcPresent(c) ==
  SpecInvalidQcPresent(c)

ActualDropsInvalidPayload(c) ==
  IF Bug = "invalid_qc_not_dropped"
     /\ c = "incoming_final_invalid_no_recovery_drop"
  THEN FALSE
  ELSE IF Bug = "invalid_qc_drop_with_block_quorum"
          /\ c = "invalid_qc_block_quorum" THEN TRUE
  ELSE IF Bug = "invalid_qc_drop_with_checkpoint"
          /\ c = "invalid_qc_checkpoint" THEN TRUE
  ELSE SpecDropsInvalidPayload(c)

ActualInvalidPayloadReturnsOk(c) ==
  IF Bug = "invalid_qc_returns_error"
     /\ c = "incoming_final_invalid_no_recovery_drop"
  THEN FALSE
  ELSE SpecInvalidPayloadReturnsOk(c)

ActualClearsMissing(c) ==
  IF Bug = "invalid_qc_clears_missing"
     /\ c = "incoming_final_invalid_no_recovery_drop"
  THEN TRUE
  ELSE SpecClearsMissing(c)

SpecTrace(c) ==
  [
    source_incoming |-> SpecSourceIncoming(c),
    source_selection |-> SpecSourceSelection(c),
    source_checkpoint |-> SpecSourceCheckpoint(c),
    source_world |-> SpecSourceWorld(c),
    source_cached |-> SpecSourceCached(c),
    candidate_kept |-> SpecCandidateKept(c),
    validates_candidate |-> SpecValidatesCandidate(c),
    aggregate_ok_cached |-> SpecAggregateOkCached(c),
    aggregate_ok_selection |-> SpecAggregateOkSelection(c),
    quarantines_missing_context |-> SpecQuarantinesMissingContext(c),
    final_drops_qc |-> SpecFinalDropsQc(c),
    qc_replaced_metric |-> SpecQcReplacedMetric(c),
    derives_cached_qc |-> SpecDerivesCachedQc(c),
    incoming_qc_validated |-> SpecIncomingQcValidated(c),
    aggregate_fallback_attempted |-> SpecAggregateFallbackAttempted(c),
    aggregate_fallback_accepted |-> SpecAggregateFallbackAccepted(c),
    locked_conflict_drop |-> SpecLockedConflictDrop(c),
    qc_evidence_present |-> SpecQcEvidencePresent(c),
    usable_qc_cached |-> SpecUsableQcCached(c),
    quarantine_cleared |-> SpecQuarantineCleared(c),
    commit_cert_present |-> SpecCommitCertPresent(c),
    invalid_qc_present |-> SpecInvalidQcPresent(c),
    drops_invalid_payload |-> SpecDropsInvalidPayload(c),
    invalid_payload_returns_ok |-> SpecInvalidPayloadReturnsOk(c),
    clears_missing |-> SpecClearsMissing(c)
  ]

ActualTrace(c) ==
  [
    source_incoming |-> ActualSourceIncoming(c),
    source_selection |-> ActualSourceSelection(c),
    source_checkpoint |-> ActualSourceCheckpoint(c),
    source_world |-> ActualSourceWorld(c),
    source_cached |-> ActualSourceCached(c),
    candidate_kept |-> ActualCandidateKept(c),
    validates_candidate |-> ActualValidatesCandidate(c),
    aggregate_ok_cached |-> ActualAggregateOkCached(c),
    aggregate_ok_selection |-> ActualAggregateOkSelection(c),
    quarantines_missing_context |-> ActualQuarantinesMissingContext(c),
    final_drops_qc |-> ActualFinalDropsQc(c),
    qc_replaced_metric |-> ActualQcReplacedMetric(c),
    derives_cached_qc |-> ActualDerivesCachedQc(c),
    incoming_qc_validated |-> ActualIncomingQcValidated(c),
    aggregate_fallback_attempted |-> ActualAggregateFallbackAttempted(c),
    aggregate_fallback_accepted |-> ActualAggregateFallbackAccepted(c),
    locked_conflict_drop |-> ActualLockedConflictDrop(c),
    qc_evidence_present |-> ActualQcEvidencePresent(c),
    usable_qc_cached |-> ActualUsableQcCached(c),
    quarantine_cleared |-> ActualQuarantineCleared(c),
    commit_cert_present |-> ActualCommitCertPresent(c),
    invalid_qc_present |-> ActualInvalidQcPresent(c),
    drops_invalid_payload |-> ActualDropsInvalidPayload(c),
    invalid_payload_returns_ok |-> ActualInvalidPayloadReturnsOk(c),
    clears_missing |-> ActualClearsMissing(c)
  ]

Matches(c) ==
  ActualTrace(c) = SpecTrace(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "incoming_not_preferred",
       "selection_not_preferred",
       "checkpoint_not_preferred",
       "world_not_preferred",
       "cached_source_ignored",
       "no_source_skips_cached_recovery",
       "bad_height_kept",
       "bad_hash_kept",
       "bad_epoch_kept",
       "bad_phase_kept",
       "shape_drop_validates",
       "missing_context_not_quarantined",
       "missing_context_final_dropped",
       "validation_error_no_replaced_metric",
       "final_invalid_not_dropped",
       "incoming_invalid_no_cached_recovery",
       "selection_invalid_uses_cached_recovery",
       "aggregate_fallback_not_attempted",
       "aggregate_fallback_without_original",
       "aggregate_fallback_not_accepted",
       "cached_match_no_aggregate_ok",
       "selection_match_no_aggregate_ok",
       "hard_lock_keeps_evidence",
       "hard_lock_no_status",
       "usable_qc_not_cached",
       "unusable_qc_cached",
       "commit_cert_dropped_for_valid_selection",
       "commit_cert_without_validation",
       "invalid_qc_not_dropped",
       "invalid_qc_drop_with_block_quorum",
       "invalid_qc_drop_with_checkpoint",
       "invalid_qc_returns_error",
       "invalid_qc_clears_missing"
     }
  /\ checked = 0

SafetyFast ==
  \A c \in Cases: Matches(c)

SourcePrecedence ==
  Matches("incoming_preempts_selection")
    /\ Matches("selection_preempts_checkpoint")
    /\ Matches("checkpoint_preempts_world")
    /\ Matches("world_preempts_cached")
    /\ Matches("cached_valid")

ShapeAndValidation ==
  Matches("incoming_shape_height")
    /\ Matches("incoming_shape_hash")
    /\ Matches("incoming_shape_epoch")
    /\ Matches("incoming_shape_phase")
    /\ Matches("incoming_missing_context")
    /\ Matches("incoming_final_invalid_no_recovery_drop")
    /\ Matches("selection_final_invalid_no_recovery")

RecoveryAndFallback ==
  Matches("no_source_cached_recovery")
    /\ Matches("incoming_final_invalid_cached_recovery")
    /\ Matches("incoming_final_invalid_aggregate_fallback")
    /\ Matches("cached_match_skips_aggregate")
    /\ Matches("selection_match_skips_aggregate")

LockedAndInvalidDrop ==
  Matches("hard_lock_conflict")
    /\ Matches("invalid_qc_block_quorum")
    /\ Matches("invalid_qc_checkpoint")

=============================================================================
