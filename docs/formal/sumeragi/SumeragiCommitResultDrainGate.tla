---- MODULE SumeragiCommitResultDrainGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for asynchronous commit-result draining.

This slice models the safety boundary in `Actor::drain_commit_results(...)`.
Commit worker results are allowed to mutate consensus state only when the
result id matches the current inflight commit. Stale results are ignored while
preserving the real inflight job, ownerless results are ignored, disconnected
worker channels clear worker state and fall back to inline execution only when
an inflight job exists, and the pacemaker is kickstarted only after a commit
outcome is actually applied and reports durable commit progress.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  result_applied,
  \* @type: Bool;
  result_ignored,
  \* @type: Bool;
  inflight_restored,
  \* @type: Bool;
  inflight_cleared,
  \* @type: Bool;
  summary_recorded,
  \* @type: Bool;
  progress_recorded,
  \* @type: Bool;
  kickstarted,
  \* @type: Bool;
  worker_state_cleared,
  \* @type: Bool;
  inline_executed,
  \* @type: Bool;
  signature_recovery_allowed,
  \* @type: Bool;
  loop_stopped

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars == <<candidate, result_applied, result_ignored, inflight_restored,
  inflight_cleared, summary_recorded, progress_recorded, kickstarted,
  worker_state_cleared, inline_executed, signature_recovery_allowed,
  loop_stopped>>

Cases == {
  "no_result_rx",
  "empty_result",
  "matching_success",
  "matching_rejected",
  "matching_kura_retry",
  "id_mismatch",
  "no_inflight_result",
  "disconnected_no_inflight",
  "disconnected_success",
  "disconnected_rejected",
  "disconnected_local_outside_with_qc",
  "disconnected_local_outside_without_qc",
  "disconnected_local_inside_with_qc"
}

MatchingCases == {
  "matching_success",
  "matching_rejected",
  "matching_kura_retry"
}

DisconnectedCases == {
  "disconnected_no_inflight",
  "disconnected_success",
  "disconnected_rejected",
  "disconnected_local_outside_with_qc",
  "disconnected_local_outside_without_qc",
  "disconnected_local_inside_with_qc"
}

DisconnectedWithInflightCases ==
  DisconnectedCases \ {"disconnected_no_inflight"}

AppliedCases ==
  MatchingCases \union DisconnectedWithInflightCases

IgnoredResultCases == {"id_mismatch", "no_inflight_result"}

CommittedCases == {
  "matching_success",
  "disconnected_success",
  "disconnected_local_outside_with_qc"
}

LocalOutsideCases == {
  "disconnected_local_outside_with_qc",
  "disconnected_local_outside_without_qc"
}

HasCommitQcCases == {
  "disconnected_local_outside_with_qc",
  "disconnected_local_inside_with_qc"
}

SpecResultApplied(c) == c \in AppliedCases

ActualResultApplied(c) ==
  \/ /\ SpecResultApplied(c)
     /\ ~(Bug = "skip_apply_matching" /\ c \in MatchingCases)
     /\ ~(Bug = "skip_apply_disconnected" /\ c \in DisconnectedWithInflightCases)
  \/ /\ c = "no_result_rx"
     /\ Bug = "apply_without_result_rx"
  \/ /\ c = "empty_result"
     /\ Bug = "apply_on_empty"
  \/ /\ c = "id_mismatch"
     /\ Bug = "apply_id_mismatch"
  \/ /\ c = "no_inflight_result"
     /\ Bug = "apply_without_inflight"
  \/ /\ c = "disconnected_no_inflight"
     /\ Bug = "inline_without_inflight"

SpecResultIgnored(c) == c \in IgnoredResultCases

ActualResultIgnored(c) ==
  IF SpecResultIgnored(c)
  THEN Bug # "skip_ignore_stale_result"
  ELSE FALSE

SpecInflightRestored(c) == c = "id_mismatch"

ActualInflightRestored(c) ==
  \/ /\ SpecInflightRestored(c)
     /\ Bug # "drop_inflight_on_id_mismatch"
  \/ /\ c \in AppliedCases
     /\ Bug = "restore_inflight_after_apply"

SpecInflightCleared(c) == c \in AppliedCases

ActualInflightCleared(c) ==
  IF SpecInflightCleared(c)
  THEN Bug # "fail_to_clear_inflight_after_apply"
  ELSE FALSE

SpecSummaryRecorded(c) == c \in AppliedCases

ActualSummaryRecorded(c) ==
  \/ /\ SpecSummaryRecorded(c)
     /\ Bug # "skip_summary_matching"
  \/ /\ c \in IgnoredResultCases
     /\ Bug = "record_summary_on_ignored_result"
  \/ /\ ~SpecResultApplied(c)
     /\ Bug = "summary_without_apply"

SpecProgressRecorded(c) == c \in AppliedCases

ActualProgressRecorded(c) ==
  \/ /\ SpecProgressRecorded(c)
     /\ Bug # "skip_progress_matching"
  \/ /\ ~SpecResultApplied(c)
     /\ Bug = "progress_without_apply"

SpecKickstarted(c) == c \in CommittedCases

ActualKickstarted(c) ==
  \/ /\ SpecKickstarted(c)
     /\ Bug # "skip_kickstart_committed"
  \/ /\ c \in {"matching_rejected", "matching_kura_retry", "disconnected_rejected"}
     /\ Bug = "kickstart_rejected"
  \/ /\ ~SpecResultApplied(c)
     /\ Bug = "kickstart_without_apply"

SpecWorkerStateCleared(c) == c \in DisconnectedCases

ActualWorkerStateCleared(c) ==
  \/ /\ SpecWorkerStateCleared(c)
     /\ Bug # "keep_worker_on_disconnected"
  \/ /\ c = "empty_result"
     /\ Bug = "clear_worker_on_empty"

SpecInlineExecuted(c) == c \in DisconnectedWithInflightCases

ActualInlineExecuted(c) ==
  \/ /\ SpecInlineExecuted(c)
     /\ Bug # "skip_inline_disconnected"
  \/ /\ c = "disconnected_no_inflight"
     /\ Bug = "inline_without_inflight"

SpecSignatureRecoveryAllowed(c) ==
  /\ c \in LocalOutsideCases
  /\ c \in HasCommitQcCases

ActualSignatureRecoveryAllowed(c) ==
  \/ /\ SpecSignatureRecoveryAllowed(c)
     /\ Bug # "deny_recovery_with_local_outside_qc"
  \/ /\ c = "disconnected_local_inside_with_qc"
     /\ Bug = "allow_recovery_without_local_outside"
  \/ /\ c = "disconnected_local_outside_without_qc"
     /\ Bug = "allow_recovery_without_commit_qc"

SpecLoopStopped(c) ==
  c \in {"empty_result"} \union DisconnectedCases

ActualLoopStopped(c) ==
  \/ /\ SpecLoopStopped(c)
     /\ ~(Bug = "no_loop_stop_on_empty" /\ c = "empty_result")
     /\ ~(Bug = "continue_after_disconnected" /\ c \in DisconnectedCases)

Init ==
  /\ candidate = "none"
  /\ result_applied = FALSE
  /\ result_ignored = FALSE
  /\ inflight_restored = FALSE
  /\ inflight_cleared = FALSE
  /\ summary_recorded = FALSE
  /\ progress_recorded = FALSE
  /\ kickstarted = FALSE
  /\ worker_state_cleared = FALSE
  /\ inline_executed = FALSE
  /\ signature_recovery_allowed = FALSE
  /\ loop_stopped = FALSE

CheckCase(c) ==
  /\ candidate = "none"
  /\ candidate' = c
  /\ result_applied' = ActualResultApplied(c)
  /\ result_ignored' = ActualResultIgnored(c)
  /\ inflight_restored' = ActualInflightRestored(c)
  /\ inflight_cleared' = ActualInflightCleared(c)
  /\ summary_recorded' = ActualSummaryRecorded(c)
  /\ progress_recorded' = ActualProgressRecorded(c)
  /\ kickstarted' = ActualKickstarted(c)
  /\ worker_state_cleared' = ActualWorkerStateCleared(c)
  /\ inline_executed' = ActualInlineExecuted(c)
  /\ signature_recovery_allowed' = ActualSignatureRecoveryAllowed(c)
  /\ loop_stopped' = ActualLoopStopped(c)

Next ==
  \/ \E c \in Cases : CheckCase(c)
  \/ /\ candidate # "none"
     /\ UNCHANGED vars

TypeInvariant ==
  /\ candidate \in Cases \union {"none"}
  /\ result_applied \in BOOLEAN
  /\ result_ignored \in BOOLEAN
  /\ inflight_restored \in BOOLEAN
  /\ inflight_cleared \in BOOLEAN
  /\ summary_recorded \in BOOLEAN
  /\ progress_recorded \in BOOLEAN
  /\ kickstarted \in BOOLEAN
  /\ worker_state_cleared \in BOOLEAN
  /\ inline_executed \in BOOLEAN
  /\ signature_recovery_allowed \in BOOLEAN
  /\ loop_stopped \in BOOLEAN

AppliedResultsMatchSpec ==
  candidate = "none" \/ result_applied = SpecResultApplied(candidate)

StaleResultsAreIgnored ==
  candidate = "id_mismatch" =>
    /\ result_ignored
    /\ ~result_applied

OwnerlessResultsAreIgnored ==
  candidate = "no_inflight_result" =>
    /\ result_ignored
    /\ ~result_applied

IgnoredResultsDoNotRecordProgress ==
  result_ignored =>
    /\ ~summary_recorded
    /\ ~progress_recorded
    /\ ~kickstarted

InflightRestorationMatchesSpec ==
  candidate = "none" \/ inflight_restored = SpecInflightRestored(candidate)

IdMismatchPreservesRealInflight ==
  candidate = "id_mismatch" => inflight_restored

InflightClearMatchesSpec ==
  candidate = "none" \/ inflight_cleared = SpecInflightCleared(candidate)

AppliedResultsClearInflight ==
  result_applied => inflight_cleared

SummaryMatchesSpec ==
  candidate = "none" \/ summary_recorded = SpecSummaryRecorded(candidate)

ProgressMatchesSpec ==
  candidate = "none" \/ progress_recorded = SpecProgressRecorded(candidate)

SummaryAndProgressRequireApply ==
  (summary_recorded \/ progress_recorded) => result_applied

KickstartMatchesSpec ==
  candidate = "none" \/ kickstarted = SpecKickstarted(candidate)

KickstartRequiresCommittedApply ==
  kickstarted =>
    /\ result_applied
    /\ candidate \in CommittedCases

WorkerClearMatchesSpec ==
  candidate = "none" \/ worker_state_cleared = SpecWorkerStateCleared(candidate)

DisconnectedClearsWorkerState ==
  candidate \in DisconnectedCases => worker_state_cleared

InlineExecutionMatchesSpec ==
  candidate = "none" \/ inline_executed = SpecInlineExecuted(candidate)

InlineFallbackRequiresInflight ==
  inline_executed => candidate \in DisconnectedWithInflightCases

SignatureRecoveryMatchesSpec ==
  candidate = "none" \/
    signature_recovery_allowed = SpecSignatureRecoveryAllowed(candidate)

SignatureRecoveryRequiresLocalOutsideAndCommitQc ==
  signature_recovery_allowed =>
    /\ candidate \in LocalOutsideCases
    /\ candidate \in HasCommitQcCases

LoopStopMatchesSpec ==
  candidate = "none" \/ loop_stopped = SpecLoopStopped(candidate)

EmptyAndDisconnectedStopDrainLoop ==
  candidate \in ({"empty_result"} \union DisconnectedCases) => loop_stopped

NoReceiverDoesNothing ==
  candidate = "no_result_rx" =>
    /\ ~result_applied
    /\ ~result_ignored
    /\ ~worker_state_cleared
    /\ ~summary_recorded
    /\ ~progress_recorded

Safety ==
  /\ AppliedResultsMatchSpec
  /\ StaleResultsAreIgnored
  /\ OwnerlessResultsAreIgnored
  /\ IgnoredResultsDoNotRecordProgress
  /\ InflightRestorationMatchesSpec
  /\ IdMismatchPreservesRealInflight
  /\ InflightClearMatchesSpec
  /\ AppliedResultsClearInflight
  /\ SummaryMatchesSpec
  /\ ProgressMatchesSpec
  /\ SummaryAndProgressRequireApply
  /\ KickstartMatchesSpec
  /\ KickstartRequiresCommittedApply
  /\ WorkerClearMatchesSpec
  /\ DisconnectedClearsWorkerState
  /\ InlineExecutionMatchesSpec
  /\ InlineFallbackRequiresInflight
  /\ SignatureRecoveryMatchesSpec
  /\ SignatureRecoveryRequiresLocalOutsideAndCommitQc
  /\ LoopStopMatchesSpec
  /\ EmptyAndDisconnectedStopDrainLoop
  /\ NoReceiverDoesNothing

=============================================================================
