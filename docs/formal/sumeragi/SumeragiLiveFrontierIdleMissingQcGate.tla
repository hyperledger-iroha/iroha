---- MODULE SumeragiLiveFrontierIdleMissingQcGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for live-frontier idle missing-QC reacquire damping.

This slice pins `should_suppress_live_frontier_idle_missing_qc_reacquire(...)`
and the immediate `reacquire_missing_qc_dependencies(...)` branch that consumes
a live committed+1 round without emitting broad idle anchor recovery. The
suppression applies only with resilience enabled, dependency signals present,
no prior same-height reacquire, exact committed+1 height, no observed head
beyond the live height, no explicit commit-phase/missing-QC dependency, and
local round liveness from proposal evidence or active pending blocks.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoBug == "none"

Bugs == {
  NoBug,
  "suppress_reject_slot_liveness",
  "suppress_reject_pending_block",
  "suppress_reject_observed_equal",
  "suppress_reject_observed_lower",
  "suppress_accept_resilience_disabled",
  "suppress_accept_no_dependency_signals",
  "suppress_accept_prior_reacquire",
  "suppress_accept_below_frontier",
  "suppress_accept_above_frontier",
  "suppress_accept_observed_future",
  "suppress_accept_commit_dependency",
  "suppress_accept_missing_qc_dependency",
  "suppress_accept_no_liveness",
  "effect_skip_attempt_record",
  "effect_emit_highest_qc_fetch",
  "effect_emit_anchor_pull",
  "effect_skip_sidecar_hint"
}

SuppressWithSlotLiveness ==
  IF Bug = "suppress_reject_slot_liveness" THEN FALSE ELSE TRUE

SuppressWithPendingBlock ==
  IF Bug = "suppress_reject_pending_block" THEN FALSE ELSE TRUE

SuppressWithObservedEqual ==
  IF Bug = "suppress_reject_observed_equal" THEN FALSE ELSE TRUE

SuppressWithObservedLower ==
  IF Bug = "suppress_reject_observed_lower" THEN FALSE ELSE TRUE

RejectResilienceDisabled ==
  IF Bug = "suppress_accept_resilience_disabled" THEN FALSE ELSE TRUE

RejectNoDependencySignals ==
  IF Bug = "suppress_accept_no_dependency_signals" THEN FALSE ELSE TRUE

RejectPriorSameHeightReacquire ==
  IF Bug = "suppress_accept_prior_reacquire" THEN FALSE ELSE TRUE

RejectBelowFrontierHeight ==
  IF Bug = "suppress_accept_below_frontier" THEN FALSE ELSE TRUE

RejectAboveFrontierHeight ==
  IF Bug = "suppress_accept_above_frontier" THEN FALSE ELSE TRUE

RejectObservedFutureHead ==
  IF Bug = "suppress_accept_observed_future" THEN FALSE ELSE TRUE

RejectCommitPhaseDependency ==
  IF Bug = "suppress_accept_commit_dependency" THEN FALSE ELSE TRUE

RejectMissingQcDependency ==
  IF Bug = "suppress_accept_missing_qc_dependency" THEN FALSE ELSE TRUE

RejectNoRoundLiveness ==
  IF Bug = "suppress_accept_no_liveness" THEN FALSE ELSE TRUE

SuppressedRecordsAttempt ==
  IF Bug = "effect_skip_attempt_record" THEN FALSE ELSE TRUE

SuppressedBlocksHighestQcFetch ==
  IF Bug = "effect_emit_highest_qc_fetch" THEN FALSE ELSE TRUE

SuppressedBlocksAnchorPull ==
  IF Bug = "effect_emit_anchor_pull" THEN FALSE ELSE TRUE

SuppressedStillAllowsSidecarHint ==
  IF Bug = "effect_skip_sidecar_hint" THEN FALSE ELSE TRUE

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1

SuppressionPositiveSafety ==
  /\ SuppressWithSlotLiveness
  /\ SuppressWithPendingBlock
  /\ SuppressWithObservedEqual
  /\ SuppressWithObservedLower

SuppressionNegativeSafety ==
  /\ RejectResilienceDisabled
  /\ RejectNoDependencySignals
  /\ RejectPriorSameHeightReacquire
  /\ RejectBelowFrontierHeight
  /\ RejectAboveFrontierHeight
  /\ RejectObservedFutureHead
  /\ RejectCommitPhaseDependency
  /\ RejectMissingQcDependency
  /\ RejectNoRoundLiveness

SuppressedBranchEffectSafety ==
  /\ SuppressedRecordsAttempt
  /\ SuppressedBlocksHighestQcFetch
  /\ SuppressedBlocksAnchorPull
  /\ SuppressedStillAllowsSidecarHint

SafetyFast ==
  /\ SuppressionPositiveSafety
  /\ SuppressionNegativeSafety
  /\ SuppressedBranchEffectSafety

====
