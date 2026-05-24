---- MODULE SumeragiCommitInflightTimeoutGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for commit-inflight timeout reporting.

This slice models `Actor::report_inflight_commit_if_timed_out(...)`. A commit
worker that exceeds `commit_inflight_timeout` is only reported once: the actor
marks the existing inflight job as reported, records timeout diagnostics, and
keeps the worker result attachable. Timeout reporting must not requeue or
abort the pending block, prune proposal state, force a view, record a commit
failure, apply an outcome, or kick the pacemaker.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  report_returned,
  \* @type: Bool;
  timeout_mark_present,
  \* @type: Bool;
  timeout_newly_marked,
  \* @type: Bool;
  inflight_preserved,
  \* @type: Bool;
  status_recorded,
  \* @type: Bool;
  warning_recorded,
  \* @type: Bool;
  pending_requeued,
  \* @type: Bool;
  pending_aborted,
  \* @type: Bool;
  proposal_pruned,
  \* @type: Bool;
  view_changed,
  \* @type: Bool;
  forced_view_set,
  \* @type: Bool;
  commit_failure_recorded,
  \* @type: Bool;
  outcome_applied,
  \* @type: Bool;
  pacemaker_kickstarted,
  \* @type: Bool;
  late_result_attachable

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars == <<candidate, report_returned, timeout_mark_present,
  timeout_newly_marked, inflight_preserved, status_recorded,
  warning_recorded, pending_requeued, pending_aborted, proposal_pruned,
  view_changed, forced_view_set, commit_failure_recorded, outcome_applied,
  pacemaker_kickstarted, late_result_attachable>>

Cases == {
  "timeout_zero_with_inflight",
  "no_inflight",
  "clock_before_enqueue",
  "below_timeout",
  "at_timeout_unreported",
  "above_timeout_unreported",
  "at_timeout_already_reported",
  "above_timeout_already_reported"
}

NoInflightCases == {"no_inflight"}
TimeoutZeroCases == {"timeout_zero_with_inflight"}
ClockBeforeCases == {"clock_before_enqueue"}
BelowTimeoutCases == {"below_timeout"}
AtTimeoutNewCases == {"at_timeout_unreported"}
AboveTimeoutNewCases == {"above_timeout_unreported"}
NewReportCases == AtTimeoutNewCases \union AboveTimeoutNewCases
AlreadyReportedCases == {
  "at_timeout_already_reported",
  "above_timeout_already_reported"
}
HasInflightCases == Cases \ NoInflightCases
NoReportCases == Cases \ NewReportCases

HasInflight(c) == c \in HasInflightCases
SpecReportReturned(c) == c \in NewReportCases
SpecTimeoutMarkPresent(c) == HasInflight(c) /\ c \in (NewReportCases \union AlreadyReportedCases)
SpecTimeoutNewlyMarked(c) == c \in NewReportCases
SpecInflightPreserved(c) == HasInflight(c)
SpecStatusRecorded(c) == c \in NewReportCases
SpecWarningRecorded(c) == c \in NewReportCases
SpecLateResultAttachable(c) == HasInflight(c)

ActualReportReturned(c) ==
  \/ /\ SpecReportReturned(c)
     /\ Bug # "timeout_returns_false"
     /\ ~(Bug = "at_timeout_missed" /\ c \in AtTimeoutNewCases)
     /\ ~(Bug = "above_timeout_missed" /\ c \in AboveTimeoutNewCases)
  \/ /\ c \in TimeoutZeroCases
     /\ Bug = "timeout_zero_reports"
  \/ /\ c \in NoInflightCases
     /\ Bug = "no_inflight_reports"
  \/ /\ c \in BelowTimeoutCases
     /\ Bug = "below_timeout_reports"
  \/ /\ c \in ClockBeforeCases
     /\ Bug = "clock_before_enqueue_reports"
  \/ /\ c \in AlreadyReportedCases
     /\ Bug = "already_reported_repeats"
  \/ /\ c \in NoReportCases
     /\ Bug = "non_timeout_returns_true"

ActualTimeoutMarkPresent(c) ==
  \/ /\ SpecTimeoutMarkPresent(c)
     /\ ~(Bug = "at_timeout_missed" /\ c \in AtTimeoutNewCases)
     /\ ~(Bug = "above_timeout_missed" /\ c \in AboveTimeoutNewCases)
     /\ ~(Bug = "already_reported_clears_flag" /\ c \in AlreadyReportedCases)
  \/ /\ ~SpecTimeoutMarkPresent(c)
     /\ Bug = "non_timeout_sets_flag"

ActualTimeoutNewlyMarked(c) ==
  \/ /\ SpecTimeoutNewlyMarked(c)
     /\ ~(Bug = "at_timeout_missed" /\ c \in AtTimeoutNewCases)
     /\ ~(Bug = "above_timeout_missed" /\ c \in AboveTimeoutNewCases)
  \/ /\ c \in AlreadyReportedCases
     /\ Bug = "already_reported_repeats"
  \/ /\ c \in NoReportCases
     /\ Bug = "non_timeout_sets_flag"

ActualInflightPreserved(c) ==
  /\ SpecInflightPreserved(c)
  /\ ~(Bug = "timeout_clears_inflight" /\ c \in NewReportCases)

ActualStatusRecorded(c) ==
  \/ /\ SpecStatusRecorded(c)
     /\ Bug # "timeout_without_status_record"
     /\ ~(Bug = "at_timeout_missed" /\ c \in AtTimeoutNewCases)
     /\ ~(Bug = "above_timeout_missed" /\ c \in AboveTimeoutNewCases)
  \/ /\ ~SpecStatusRecorded(c)
     /\ Bug = "status_without_new_report"

ActualWarningRecorded(c) ==
  \/ /\ SpecWarningRecorded(c)
     /\ Bug # "timeout_without_warning"
     /\ ~(Bug = "at_timeout_missed" /\ c \in AtTimeoutNewCases)
     /\ ~(Bug = "above_timeout_missed" /\ c \in AboveTimeoutNewCases)
  \/ /\ ~SpecWarningRecorded(c)
     /\ Bug = "warning_without_new_report"

ActualPendingRequeued(c) ==
  /\ c \in NewReportCases
  /\ Bug = "timeout_requeues_pending"

ActualPendingAborted(c) ==
  /\ c \in NewReportCases
  /\ Bug = "timeout_marks_pending_aborted"

ActualProposalPruned(c) ==
  /\ c \in NewReportCases
  /\ Bug = "timeout_prunes_proposal"

ActualViewChanged(c) ==
  /\ c \in NewReportCases
  /\ Bug = "timeout_triggers_view_change"

ActualForcedViewSet(c) ==
  /\ c \in NewReportCases
  /\ Bug = "timeout_forces_view"

ActualCommitFailureRecorded(c) ==
  /\ c \in NewReportCases
  /\ Bug = "timeout_records_commit_failure"

ActualOutcomeApplied(c) ==
  /\ c \in NewReportCases
  /\ Bug = "timeout_applies_outcome"

ActualPacemakerKickstarted(c) ==
  /\ c \in NewReportCases
  /\ Bug = "timeout_kickstarts_pacemaker"

ActualLateResultAttachable(c) ==
  /\ SpecLateResultAttachable(c)
  /\ ActualInflightPreserved(c)
  /\ ~(Bug = "timeout_detaches_late_result" /\ c \in NewReportCases)

Init ==
  /\ candidate = "none"
  /\ report_returned = FALSE
  /\ timeout_mark_present = FALSE
  /\ timeout_newly_marked = FALSE
  /\ inflight_preserved = FALSE
  /\ status_recorded = FALSE
  /\ warning_recorded = FALSE
  /\ pending_requeued = FALSE
  /\ pending_aborted = FALSE
  /\ proposal_pruned = FALSE
  /\ view_changed = FALSE
  /\ forced_view_set = FALSE
  /\ commit_failure_recorded = FALSE
  /\ outcome_applied = FALSE
  /\ pacemaker_kickstarted = FALSE
  /\ late_result_attachable = FALSE

CheckCase(c) ==
  /\ candidate = "none"
  /\ candidate' = c
  /\ report_returned' = ActualReportReturned(c)
  /\ timeout_mark_present' = ActualTimeoutMarkPresent(c)
  /\ timeout_newly_marked' = ActualTimeoutNewlyMarked(c)
  /\ inflight_preserved' = ActualInflightPreserved(c)
  /\ status_recorded' = ActualStatusRecorded(c)
  /\ warning_recorded' = ActualWarningRecorded(c)
  /\ pending_requeued' = ActualPendingRequeued(c)
  /\ pending_aborted' = ActualPendingAborted(c)
  /\ proposal_pruned' = ActualProposalPruned(c)
  /\ view_changed' = ActualViewChanged(c)
  /\ forced_view_set' = ActualForcedViewSet(c)
  /\ commit_failure_recorded' = ActualCommitFailureRecorded(c)
  /\ outcome_applied' = ActualOutcomeApplied(c)
  /\ pacemaker_kickstarted' = ActualPacemakerKickstarted(c)
  /\ late_result_attachable' = ActualLateResultAttachable(c)

Next ==
  \/ \E c \in Cases : CheckCase(c)
  \/ /\ candidate # "none"
     /\ UNCHANGED vars

TypeInvariant ==
  /\ candidate \in Cases \union {"none"}
  /\ report_returned \in BOOLEAN
  /\ timeout_mark_present \in BOOLEAN
  /\ timeout_newly_marked \in BOOLEAN
  /\ inflight_preserved \in BOOLEAN
  /\ status_recorded \in BOOLEAN
  /\ warning_recorded \in BOOLEAN
  /\ pending_requeued \in BOOLEAN
  /\ pending_aborted \in BOOLEAN
  /\ proposal_pruned \in BOOLEAN
  /\ view_changed \in BOOLEAN
  /\ forced_view_set \in BOOLEAN
  /\ commit_failure_recorded \in BOOLEAN
  /\ outcome_applied \in BOOLEAN
  /\ pacemaker_kickstarted \in BOOLEAN
  /\ late_result_attachable \in BOOLEAN

ReportReturnMatchesSpec ==
  candidate = "none" \/ report_returned = SpecReportReturned(candidate)

TimeoutMarkMatchesSpec ==
  candidate = "none" \/ timeout_mark_present = SpecTimeoutMarkPresent(candidate)

TimeoutNewlyMarkedMatchesSpec ==
  candidate = "none" \/ timeout_newly_marked = SpecTimeoutNewlyMarked(candidate)

InflightPreservationMatchesSpec ==
  candidate = "none" \/ inflight_preserved = SpecInflightPreserved(candidate)

StatusRecordedMatchesSpec ==
  candidate = "none" \/ status_recorded = SpecStatusRecorded(candidate)

WarningRecordedMatchesSpec ==
  candidate = "none" \/ warning_recorded = SpecWarningRecorded(candidate)

LateResultAttachabilityMatchesSpec ==
  candidate = "none" \/ late_result_attachable = SpecLateResultAttachable(candidate)

ReportTrueOnlyForNewTimeout ==
  report_returned => candidate \in NewReportCases

NewTimeoutReportsExactlyOnce ==
  candidate \in NewReportCases =>
    /\ report_returned
    /\ timeout_mark_present
    /\ timeout_newly_marked
    /\ status_recorded
    /\ warning_recorded

AlreadyReportedDoesNotDuplicateDiagnostics ==
  candidate \in AlreadyReportedCases =>
    /\ ~report_returned
    /\ timeout_mark_present
    /\ ~timeout_newly_marked
    /\ ~status_recorded
    /\ ~warning_recorded

NonTimeoutDoesNotReport ==
  candidate \in (NoReportCases \ AlreadyReportedCases) =>
    /\ ~report_returned
    /\ ~timeout_newly_marked
    /\ ~status_recorded
    /\ ~warning_recorded

NoInflightDoesNotPreservePhantomOwner ==
  candidate \in NoInflightCases =>
    /\ ~inflight_preserved
    /\ ~late_result_attachable

TimeoutReportingKeepsInflightAttachable ==
  candidate \in NewReportCases =>
    /\ inflight_preserved
    /\ late_result_attachable

TimeoutReportingDoesNotRequeueOrAbort ==
  /\ ~pending_requeued
  /\ ~pending_aborted

TimeoutReportingDoesNotPruneOrForceView ==
  /\ ~proposal_pruned
  /\ ~view_changed
  /\ ~forced_view_set

TimeoutReportingDoesNotRecordCommitFailure ==
  ~commit_failure_recorded

TimeoutReportingDoesNotApplyCommitOutcome ==
  /\ ~outcome_applied
  /\ ~pacemaker_kickstarted

DiagnosticsRequireNewTimeout ==
  (status_recorded \/ warning_recorded) => report_returned

NewMarkRequiresReturnTrue ==
  timeout_newly_marked => report_returned

Safety ==
  /\ ReportReturnMatchesSpec
  /\ TimeoutMarkMatchesSpec
  /\ TimeoutNewlyMarkedMatchesSpec
  /\ InflightPreservationMatchesSpec
  /\ StatusRecordedMatchesSpec
  /\ WarningRecordedMatchesSpec
  /\ LateResultAttachabilityMatchesSpec
  /\ ReportTrueOnlyForNewTimeout
  /\ NewTimeoutReportsExactlyOnce
  /\ AlreadyReportedDoesNotDuplicateDiagnostics
  /\ NonTimeoutDoesNotReport
  /\ NoInflightDoesNotPreservePhantomOwner
  /\ TimeoutReportingKeepsInflightAttachable
  /\ TimeoutReportingDoesNotRequeueOrAbort
  /\ TimeoutReportingDoesNotPruneOrForceView
  /\ TimeoutReportingDoesNotRecordCommitFailure
  /\ TimeoutReportingDoesNotApplyCommitOutcome
  /\ DiagnosticsRequireNewTimeout
  /\ NewMarkRequiresReturnTrue

=============================================================================
