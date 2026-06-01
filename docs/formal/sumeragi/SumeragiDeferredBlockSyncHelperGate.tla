---- MODULE SumeragiDeferredBlockSyncHelperGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for deferred BlockSyncUpdate helper behavior:

- `validation_inflight_blocks_block_sync_update(...)` only blocks when
  validation work exists and either the update is not the contiguous frontier
  block or a same/lower in-flight pending block can still conflict,
- `block_sync_update_deferral_reason(...)` gives commit work priority over
  validation work, validation priority over pending-processing work, and
  suppresses all reasons when the certified exact-frontier bypass is allowed,
- `merge_deferred_block_sync_update(...)` fills missing commit evidence
  sidecars without overwriting existing sidecars and replaces the sender only
  when the incoming sender is present,
- `deferred_block_sync_has_commit_evidence(...)` treats commit QC, validator
  checkpoint, and stake snapshot as evidence, and
- `enforce_deferred_block_sync_cap(...)` treats cap zero as unlimited, removes
  entries until length is within cap, prefers retaining evidence, then newer
  view/height/hash, and increments the eviction metric only when it removed
  entries.
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
  "validation_no_inflight",
  "validation_non_contiguous",
  "validation_contiguous_missing_pending",
  "validation_contiguous_lower_pending",
  "validation_contiguous_equal_pending",
  "validation_contiguous_higher_pending",
  "deferral_commit_priority",
  "deferral_validation",
  "deferral_pending",
  "deferral_bypass",
  "deferral_none",
  "merge_fill_missing",
  "merge_preserve_existing",
  "merge_sender_none_preserves",
  "merge_sender_some_replaces",
  "evidence_commit",
  "evidence_checkpoint",
  "evidence_stake",
  "evidence_none",
  "cap_zero_over_limit",
  "cap_under_limit",
  "cap_evict_no_evidence",
  "cap_evict_oldest_view",
  "cap_evict_oldest_height",
  "cap_evict_lowest_hash",
  "cap_evict_until_cap",
  "cap_metric_incremented",
  "cap_no_metric_without_eviction"
}

SpecValidationBlocks(c) ==
  CASE c = "validation_no_inflight" -> FALSE
    [] c = "validation_non_contiguous" -> TRUE
    [] c = "validation_contiguous_missing_pending" -> TRUE
    [] c = "validation_contiguous_lower_pending" -> TRUE
    [] c = "validation_contiguous_equal_pending" -> TRUE
    [] c = "validation_contiguous_higher_pending" -> FALSE
    [] OTHER -> FALSE

CommitInflight(c) ==
  c \in {"deferral_commit_priority", "deferral_bypass"}

ValidationInflightBlocks(c) ==
  c \in {"deferral_commit_priority", "deferral_validation", "deferral_bypass"}

PendingProcessing(c) ==
  c \in {"deferral_commit_priority", "deferral_pending", "deferral_bypass"}

AllowCertifiedBypass(c) ==
  c = "deferral_bypass"

SpecDeferralReason(c) ==
  IF AllowCertifiedBypass(c)
  THEN "none"
  ELSE IF CommitInflight(c)
  THEN "commit_inflight"
  ELSE IF ValidationInflightBlocks(c)
  THEN "validation_inflight"
  ELSE IF PendingProcessing(c)
  THEN "pending_processing"
  ELSE "none"

ExistingCommit(c) ==
  IF c = "merge_preserve_existing" THEN "existing" ELSE "none"

IncomingCommit(c) ==
  IF c \in {"merge_fill_missing", "merge_preserve_existing"} THEN "incoming" ELSE "none"

ExistingCheckpoint(c) ==
  IF c = "merge_preserve_existing" THEN "existing" ELSE "none"

IncomingCheckpoint(c) ==
  IF c \in {"merge_fill_missing", "merge_preserve_existing"} THEN "incoming" ELSE "none"

ExistingStake(c) ==
  IF c = "merge_preserve_existing" THEN "existing" ELSE "none"

IncomingStake(c) ==
  IF c \in {"merge_fill_missing", "merge_preserve_existing"} THEN "incoming" ELSE "none"

ExistingSender(c) ==
  IF c \in {"merge_sender_none_preserves", "merge_sender_some_replaces"}
  THEN "existing"
  ELSE "none"

IncomingSender(c) ==
  IF c = "merge_sender_some_replaces" THEN "incoming" ELSE "none"

SpecFinalCommit(c) ==
  IF ExistingCommit(c) # "none" THEN ExistingCommit(c) ELSE IncomingCommit(c)

SpecFinalCheckpoint(c) ==
  IF ExistingCheckpoint(c) # "none" THEN ExistingCheckpoint(c) ELSE IncomingCheckpoint(c)

SpecFinalStake(c) ==
  IF ExistingStake(c) # "none" THEN ExistingStake(c) ELSE IncomingStake(c)

SpecFinalSender(c) ==
  IF IncomingSender(c) # "none" THEN IncomingSender(c) ELSE ExistingSender(c)

EvidenceCommit(c) ==
  c = "evidence_commit"

EvidenceCheckpoint(c) ==
  c = "evidence_checkpoint"

EvidenceStake(c) ==
  c = "evidence_stake"

SpecHasEvidence(c) ==
  EvidenceCommit(c) \/ EvidenceCheckpoint(c) \/ EvidenceStake(c)

InitialLen(c) ==
  CASE c = "cap_zero_over_limit" -> 3
    [] c = "cap_under_limit" -> 2
    [] c \in {
       "cap_evict_no_evidence",
       "cap_evict_oldest_view",
       "cap_evict_oldest_height",
       "cap_evict_lowest_hash",
       "cap_metric_incremented"
     } -> 3
    [] c = "cap_evict_until_cap" -> 3
    [] c = "cap_no_metric_without_eviction" -> 2
    [] OTHER -> 0

Cap(c) ==
  CASE c = "cap_zero_over_limit" -> 0
    [] c = "cap_under_limit" -> 3
    [] c = "cap_evict_until_cap" -> 1
    [] c = "cap_no_metric_without_eviction" -> 2
    [] c \in {
       "cap_evict_no_evidence",
       "cap_evict_oldest_view",
       "cap_evict_oldest_height",
       "cap_evict_lowest_hash",
       "cap_metric_incremented"
     } -> 2
    [] OTHER -> 0

SpecEvictionCount(c) ==
  IF Cap(c) = 0 \/ InitialLen(c) <= Cap(c) THEN 0 ELSE InitialLen(c) - Cap(c)

SpecEvictedFirst(c) ==
  CASE SpecEvictionCount(c) = 0 -> "none"
    [] c = "cap_evict_no_evidence" -> "no_evidence"
    [] c = "cap_evict_oldest_view" -> "old_view"
    [] c = "cap_evict_oldest_height" -> "old_height"
    [] c = "cap_evict_lowest_hash" -> "low_hash"
    [] c \in {"cap_evict_until_cap", "cap_metric_incremented"} -> "first_candidate"
    [] OTHER -> "none"

SpecFinalLen(c) ==
  InitialLen(c) - SpecEvictionCount(c)

SpecMetricIncremented(c) ==
  SpecEvictionCount(c) > 0

ActualValidationBlocks(c) ==
  CASE Bug = "validation_empty_blocks"
       /\ c = "validation_no_inflight" -> TRUE
    [] Bug = "validation_non_contiguous_allowed"
       /\ c = "validation_non_contiguous" -> FALSE
    [] Bug = "validation_missing_pending_allowed"
       /\ c = "validation_contiguous_missing_pending" -> FALSE
    [] Bug = "validation_equal_pending_allowed"
       /\ c = "validation_contiguous_equal_pending" -> FALSE
    [] Bug = "validation_higher_pending_blocks"
       /\ c = "validation_contiguous_higher_pending" -> TRUE
    [] OTHER -> SpecValidationBlocks(c)

ActualDeferralReason(c) ==
  CASE Bug = "deferral_commit_not_priority"
       /\ c = "deferral_commit_priority" -> "validation_inflight"
    [] Bug = "deferral_validation_skipped"
       /\ c = "deferral_validation" -> "none"
    [] Bug = "deferral_pending_skipped"
       /\ c = "deferral_pending" -> "none"
    [] Bug = "deferral_bypass_ignored"
       /\ c = "deferral_bypass" -> "commit_inflight"
    [] Bug = "deferral_without_work"
       /\ c = "deferral_none" -> "pending_processing"
    [] OTHER -> SpecDeferralReason(c)

ActualFinalCommit(c) ==
  CASE Bug = "merge_drops_incoming_commit"
       /\ c = "merge_fill_missing" -> "none"
    [] Bug = "merge_overwrites_existing_commit"
       /\ c = "merge_preserve_existing" -> "incoming"
    [] OTHER -> SpecFinalCommit(c)

ActualFinalCheckpoint(c) ==
  CASE Bug = "merge_drops_incoming_checkpoint"
       /\ c = "merge_fill_missing" -> "none"
    [] OTHER -> SpecFinalCheckpoint(c)

ActualFinalStake(c) ==
  CASE Bug = "merge_drops_incoming_stake"
       /\ c = "merge_fill_missing" -> "none"
    [] OTHER -> SpecFinalStake(c)

ActualFinalSender(c) ==
  CASE Bug = "merge_sender_none_clears"
       /\ c = "merge_sender_none_preserves" -> "none"
    [] Bug = "merge_sender_some_ignored"
       /\ c = "merge_sender_some_replaces" -> "existing"
    [] OTHER -> SpecFinalSender(c)

ActualHasEvidence(c) ==
  CASE Bug = "evidence_ignores_commit"
       /\ c = "evidence_commit" -> FALSE
    [] Bug = "evidence_ignores_checkpoint"
       /\ c = "evidence_checkpoint" -> FALSE
    [] Bug = "evidence_ignores_stake"
       /\ c = "evidence_stake" -> FALSE
    [] Bug = "evidence_none_true"
       /\ c = "evidence_none" -> TRUE
    [] OTHER -> SpecHasEvidence(c)

ActualEvictionCount(c) ==
  CASE Bug = "cap_zero_evicts"
       /\ c = "cap_zero_over_limit" -> 1
    [] Bug = "cap_under_evicts"
       /\ c = "cap_under_limit" -> 1
    [] Bug = "cap_stops_after_one"
       /\ c = "cap_evict_until_cap" -> 1
    [] OTHER -> SpecEvictionCount(c)

ActualEvictedFirst(c) ==
  CASE Bug = "cap_evicts_evidence_first"
       /\ c = "cap_evict_no_evidence" -> "evidence"
    [] Bug = "cap_evicts_newer_view"
       /\ c = "cap_evict_oldest_view" -> "new_view"
    [] Bug = "cap_evicts_higher_height"
       /\ c = "cap_evict_oldest_height" -> "new_height"
    [] Bug = "cap_evicts_higher_hash"
       /\ c = "cap_evict_lowest_hash" -> "high_hash"
    [] OTHER -> SpecEvictedFirst(c)

ActualFinalLen(c) ==
  InitialLen(c) - ActualEvictionCount(c)

ActualMetricIncremented(c) ==
  CASE Bug = "cap_metric_missing"
       /\ c = "cap_metric_incremented" -> FALSE
    [] Bug = "cap_metric_without_eviction"
       /\ c = "cap_no_metric_without_eviction" -> TRUE
    [] OTHER -> ActualEvictionCount(c) > 0

Matches(c) ==
  /\ ActualValidationBlocks(c) = SpecValidationBlocks(c)
  /\ ActualDeferralReason(c) = SpecDeferralReason(c)
  /\ ActualFinalCommit(c) = SpecFinalCommit(c)
  /\ ActualFinalCheckpoint(c) = SpecFinalCheckpoint(c)
  /\ ActualFinalStake(c) = SpecFinalStake(c)
  /\ ActualFinalSender(c) = SpecFinalSender(c)
  /\ ActualHasEvidence(c) = SpecHasEvidence(c)
  /\ ActualEvictionCount(c) = SpecEvictionCount(c)
  /\ ActualEvictedFirst(c) = SpecEvictedFirst(c)
  /\ ActualFinalLen(c) = SpecFinalLen(c)
  /\ ActualMetricIncremented(c) = SpecMetricIncremented(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "validation_empty_blocks",
       "validation_non_contiguous_allowed",
       "validation_missing_pending_allowed",
       "validation_equal_pending_allowed",
       "validation_higher_pending_blocks",
       "deferral_commit_not_priority",
       "deferral_validation_skipped",
       "deferral_pending_skipped",
       "deferral_bypass_ignored",
       "deferral_without_work",
       "merge_drops_incoming_commit",
       "merge_overwrites_existing_commit",
       "merge_drops_incoming_checkpoint",
       "merge_drops_incoming_stake",
       "merge_sender_none_clears",
       "merge_sender_some_ignored",
       "evidence_ignores_commit",
       "evidence_ignores_checkpoint",
       "evidence_ignores_stake",
       "evidence_none_true",
       "cap_zero_evicts",
       "cap_under_evicts",
       "cap_evicts_evidence_first",
       "cap_evicts_newer_view",
       "cap_evicts_higher_height",
       "cap_evicts_higher_hash",
       "cap_stops_after_one",
       "cap_metric_missing",
       "cap_metric_without_eviction"
     }
  /\ checked = 0

SafetyFast ==
  \A c \in Cases: Matches(c)

ValidationNoInflightAllows ==
  Matches("validation_no_inflight")

ValidationNonContiguousBlocks ==
  Matches("validation_non_contiguous")

ValidationMissingPendingBlocks ==
  Matches("validation_contiguous_missing_pending")

ValidationEqualPendingBlocks ==
  Matches("validation_contiguous_equal_pending")

ValidationHigherPendingAllows ==
  Matches("validation_contiguous_higher_pending")

DeferralCommitPriority ==
  Matches("deferral_commit_priority")

DeferralValidationReason ==
  Matches("deferral_validation")

DeferralPendingReason ==
  Matches("deferral_pending")

DeferralBypassSuppresses ==
  Matches("deferral_bypass")

DeferralNoWorkNone ==
  Matches("deferral_none")

MergeFillsIncomingCommit ==
  Matches("merge_fill_missing")

MergePreservesExistingCommit ==
  Matches("merge_preserve_existing")

MergeFillsIncomingCheckpoint ==
  Matches("merge_fill_missing")

MergeFillsIncomingStake ==
  Matches("merge_fill_missing")

MergeSenderNonePreserves ==
  Matches("merge_sender_none_preserves")

MergeSenderSomeReplaces ==
  Matches("merge_sender_some_replaces")

EvidenceCommitDetected ==
  Matches("evidence_commit")

EvidenceCheckpointDetected ==
  Matches("evidence_checkpoint")

EvidenceStakeDetected ==
  Matches("evidence_stake")

EvidenceNoneFalse ==
  Matches("evidence_none")

CapZeroUnlimited ==
  Matches("cap_zero_over_limit")

CapUnderLimitNoEvict ==
  Matches("cap_under_limit")

CapEvictsNoEvidenceFirst ==
  Matches("cap_evict_no_evidence")

CapEvictsOldestView ==
  Matches("cap_evict_oldest_view")

CapEvictsOldestHeight ==
  Matches("cap_evict_oldest_height")

CapEvictsLowestHash ==
  Matches("cap_evict_lowest_hash")

CapEvictsUntilWithinLimit ==
  Matches("cap_evict_until_cap")

CapMetricWhenEvicted ==
  Matches("cap_metric_incremented")

CapNoMetricWithoutEviction ==
  Matches("cap_no_metric_without_eviction")

=============================================================================
====
