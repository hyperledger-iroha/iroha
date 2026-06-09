---- MODULE SumeragiCommitPipelineSchedulingGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi commit-pipeline scheduling.

This slice models the small but consensus-critical scheduling contract around
`should_run_commit_pipeline_on_tick(...)`,
`Actor::commit_pipeline_tick_deadline(...)`, and the entry ordering in
`process_commit_candidates_with_trigger_inner(...)`. The recovery model covers
what happens after candidate processing is entered; this model covers whether
the pipeline is entered, whether recovery candidates are included, when the
shared tick budget may be bypassed, and which side effects occur before or
after budget exhaustion.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  pipeline_entered,
  \* @type: Bool;
  recovery_included,
  \* @type: Bool;
  deadline_bypassed,
  \* @type: Bool;
  budget_set_wakeup,
  \* @type: Bool;
  wakeup_cleared,
  \* @type: Bool;
  event_rescheduled,
  \* @type: Bool;
  reschedule_before_candidate,
  \* @type: Bool;
  backlog_observed,
  \* @type: Bool;
  candidate_processed,
  \* @type: Bool;
  last_run_updated,
  \* @type: Bool;
  idle_budget_preserved

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars == <<candidate, pipeline_entered, recovery_included,
  deadline_bypassed, budget_set_wakeup, wakeup_cleared, event_rescheduled,
  reschedule_before_candidate, backlog_observed, candidate_processed,
  last_run_updated, idle_budget_preserved>>

Cases == {
  "tick_no_work",
  "tick_active_candidate",
  "tick_inflight_only",
  "tick_wakeup_no_candidate",
  "tick_wakeup_active_candidate",
  "tick_queue_saturated_active",
  "tick_queue_saturated_no_candidate",
  "tick_budget_exhausted_before_candidates",
  "tick_budget_exhausted_during_candidates",
  "event_no_candidate",
  "event_backlogged_candidate",
  "event_backlogged_no_candidate",
  "event_budget_exhausted",
  "candidate_active_without_recovery",
  "candidate_recovery_without_include",
  "candidate_recovery_with_wakeup",
  "candidate_recovery_with_saturation",
  "candidate_recovery_filtered_by_active_without_qc",
  "candidate_recovery_with_active_and_qc",
  "idle_view_wakeup_candidate",
  "idle_view_wakeup_no_candidate",
  "idle_view_candidate_no_wakeup"
}

TickCases == {
  "tick_no_work",
  "tick_active_candidate",
  "tick_inflight_only",
  "tick_wakeup_no_candidate",
  "tick_wakeup_active_candidate",
  "tick_queue_saturated_active",
  "tick_queue_saturated_no_candidate",
  "tick_budget_exhausted_before_candidates",
  "tick_budget_exhausted_during_candidates"
}

EventCases == {
  "event_no_candidate",
  "event_backlogged_candidate",
  "event_backlogged_no_candidate",
  "event_budget_exhausted"
}

CandidateFilterCases == {
  "candidate_active_without_recovery",
  "candidate_recovery_without_include",
  "candidate_recovery_with_wakeup",
  "candidate_recovery_with_saturation",
  "candidate_recovery_filtered_by_active_without_qc",
  "candidate_recovery_with_active_and_qc"
}

IdleBudgetCases == {
  "idle_view_wakeup_candidate",
  "idle_view_wakeup_no_candidate",
  "idle_view_candidate_no_wakeup"
}

ActiveCandidateCases == {
  "tick_active_candidate",
  "tick_wakeup_active_candidate",
  "tick_queue_saturated_active",
  "tick_budget_exhausted_before_candidates",
  "tick_budget_exhausted_during_candidates",
  "event_backlogged_candidate",
  "candidate_active_without_recovery",
  "idle_view_wakeup_candidate",
  "idle_view_candidate_no_wakeup"
}

InflightCases == {"tick_inflight_only"}

CommitWakeupCases == {
  "tick_wakeup_no_candidate",
  "tick_wakeup_active_candidate",
  "idle_view_wakeup_candidate",
  "idle_view_wakeup_no_candidate",
  "candidate_recovery_with_wakeup"
}

QueueSaturatedCases == {
  "tick_queue_saturated_active",
  "tick_queue_saturated_no_candidate",
  "candidate_recovery_with_saturation"
}

BacklogCases == {
  "event_backlogged_candidate",
  "event_backlogged_no_candidate"
}

BudgetStartExhaustedCases == {
  "tick_budget_exhausted_before_candidates",
  "event_budget_exhausted"
}

BudgetDuringCandidateCases == {"tick_budget_exhausted_during_candidates"}

RecoveryCandidateCases == {
  "candidate_recovery_without_include",
  "candidate_recovery_with_wakeup",
  "candidate_recovery_with_saturation",
  "candidate_recovery_filtered_by_active_without_qc",
  "candidate_recovery_with_active_and_qc"
}

ActiveRecoveryBlockerCases == {"candidate_recovery_filtered_by_active_without_qc"}

ActiveRecoveryWithQcCases == {"candidate_recovery_with_active_and_qc"}

HasActiveCandidate(c) == c \in ActiveCandidateCases
HasInflight(c) == c \in InflightCases
HasCommitWakeup(c) == c \in CommitWakeupCases
QueueSaturated(c) == c \in QueueSaturatedCases
BudgetStartExhausted(c) == c \in BudgetStartExhaustedCases
BudgetDuringCandidate(c) == c \in BudgetDuringCandidateCases

SpecPipelineEntered(c) ==
  IF c \in TickCases THEN
    HasActiveCandidate(c) \/ HasInflight(c) \/ HasCommitWakeup(c)
  ELSE IF c \in EventCases THEN
    TRUE
  ELSE
    FALSE

ActualPipelineEntered(c) ==
  \/ /\ SpecPipelineEntered(c)
     /\ ~(Bug = "skip_tick_active_candidate" /\ c = "tick_active_candidate")
     /\ ~(Bug = "skip_tick_inflight" /\ c = "tick_inflight_only")
     /\ ~(Bug = "skip_tick_wakeup" /\ c \in {"tick_wakeup_no_candidate",
                                             "tick_wakeup_active_candidate"})
     /\ ~(Bug = "skip_event_entry" /\ c \in EventCases)
     /\ ~(Bug = "event_backlog_suppresses" /\ c = "event_backlogged_candidate")
  \/ /\ c = "tick_no_work"
     /\ Bug = "run_tick_without_work"
  \/ /\ c = "tick_queue_saturated_no_candidate"
     /\ Bug = "run_tick_on_saturation_only"

SpecRecoveryIncluded(c) ==
  /\ c \in RecoveryCandidateCases
  /\ (HasCommitWakeup(c) \/ QueueSaturated(c) \/ c \in ActiveRecoveryWithQcCases)
  /\ c \notin ActiveRecoveryBlockerCases

ActualRecoveryIncluded(c) ==
  \/ /\ SpecRecoveryIncluded(c)
     /\ ~(Bug = "exclude_recovery_with_wakeup"
          /\ c = "candidate_recovery_with_wakeup")
     /\ ~(Bug = "exclude_recovery_with_saturation"
          /\ c = "candidate_recovery_with_saturation")
     /\ ~(Bug = "exclude_recovery_with_active_qc"
          /\ c = "candidate_recovery_with_active_and_qc")
  \/ /\ c = "candidate_recovery_without_include"
     /\ Bug = "include_recovery_without_wakeup_or_saturation"
  \/ /\ c = "candidate_recovery_filtered_by_active_without_qc"
     /\ Bug = "include_recovery_when_active_without_qc"

SpecDeadlineBypassed(c) ==
  /\ HasActiveCandidate(c)
  /\ (HasCommitWakeup(c) \/ QueueSaturated(c))

ActualDeadlineBypassed(c) ==
  \/ /\ SpecDeadlineBypassed(c)
     /\ ~(Bug = "miss_deadline_bypass_wakeup"
          /\ c = "tick_wakeup_active_candidate")
     /\ ~(Bug = "miss_deadline_bypass_saturation"
          /\ c = "tick_queue_saturated_active")
  \/ /\ c = "tick_active_candidate"
     /\ Bug = "bypass_deadline_without_pressure"
  \/ /\ c = "tick_wakeup_no_candidate"
     /\ Bug = "bypass_deadline_without_candidate"

SpecBudgetSetsWakeup(c) ==
  /\ SpecPipelineEntered(c)
  /\ (BudgetStartExhausted(c) \/ BudgetDuringCandidate(c))

ActualBudgetSetsWakeup(c) ==
  IF SpecBudgetSetsWakeup(c)
  THEN Bug # "skip_budget_wakeup"
  ELSE FALSE

SpecWakeupCleared(c) ==
  /\ c \in TickCases
  /\ SpecPipelineEntered(c)
  /\ HasCommitWakeup(c)

ActualWakeupCleared(c) ==
  \/ /\ SpecWakeupCleared(c)
     /\ Bug # "keep_wakeup_after_tick_entry"
  \/ /\ c = "tick_no_work"
     /\ Bug = "clear_wakeup_without_entry"

SpecEventRescheduled(c) ==
  /\ c \in EventCases
  /\ ~BudgetStartExhausted(c)

ActualEventRescheduled(c) ==
  IF SpecEventRescheduled(c)
  THEN Bug # "skip_event_reschedule"
  ELSE FALSE

SpecRescheduleBeforeCandidate(c) ==
  /\ SpecEventRescheduled(c)
  /\ HasActiveCandidate(c)

ActualRescheduleBeforeCandidate(c) ==
  IF SpecRescheduleBeforeCandidate(c)
  THEN Bug # "reschedule_after_candidate"
  ELSE FALSE

SpecBacklogObserved(c) ==
  /\ c \in BacklogCases
  /\ SpecEventRescheduled(c)

ActualBacklogObserved(c) ==
  IF SpecBacklogObserved(c)
  THEN Bug # "skip_backlog_observation"
  ELSE FALSE

SpecCandidateProcessed(c) ==
  /\ SpecPipelineEntered(c)
  /\ HasActiveCandidate(c)
  /\ ~BudgetStartExhausted(c)
  /\ ~BudgetDuringCandidate(c)

ActualCandidateProcessed(c) ==
  \/ /\ SpecCandidateProcessed(c)
     /\ ~(Bug = "skip_candidate_processing"
          /\ c \in {"tick_active_candidate", "tick_wakeup_active_candidate",
                    "tick_queue_saturated_active", "event_backlogged_candidate"})
     /\ ~(Bug = "event_backlog_suppresses" /\ c = "event_backlogged_candidate")
  \/ /\ c \in {"tick_budget_exhausted_before_candidates",
               "event_budget_exhausted",
               "tick_budget_exhausted_during_candidates"}
     /\ Bug = "process_after_budget_exhausted"
  \/ /\ c = "event_backlogged_no_candidate"
     /\ Bug = "event_backlog_fabricates_candidate"
  \/ /\ ~SpecPipelineEntered(c)
     /\ Bug = "process_candidate_without_pipeline"

SpecLastRunUpdated(c) ==
  /\ SpecPipelineEntered(c)
  /\ HasActiveCandidate(c)
  /\ ~BudgetStartExhausted(c)

ActualLastRunUpdated(c) ==
  \/ /\ SpecLastRunUpdated(c)
     /\ Bug # "skip_last_run_with_candidates"
  \/ /\ c = "tick_budget_exhausted_before_candidates"
     /\ Bug = "update_last_run_on_budget_exhausted_before_candidates"
  \/ /\ ~HasActiveCandidate(c)
     /\ Bug = "last_run_without_candidates"
  \/ /\ c = "event_backlogged_no_candidate"
     /\ Bug = "event_backlog_fabricates_candidate"

SpecIdleBudgetPreserved(c) ==
  /\ c \in IdleBudgetCases
  /\ HasCommitWakeup(c)
  /\ HasActiveCandidate(c)

ActualIdleBudgetPreserved(c) ==
  \/ /\ SpecIdleBudgetPreserved(c)
     /\ Bug # "skip_idle_preserve"
  \/ /\ c = "idle_view_candidate_no_wakeup"
     /\ Bug = "preserve_idle_without_wakeup"
  \/ /\ c = "idle_view_wakeup_no_candidate"
     /\ Bug = "preserve_idle_without_candidate"

Init ==
  /\ candidate = "none"
  /\ pipeline_entered = FALSE
  /\ recovery_included = FALSE
  /\ deadline_bypassed = FALSE
  /\ budget_set_wakeup = FALSE
  /\ wakeup_cleared = FALSE
  /\ event_rescheduled = FALSE
  /\ reschedule_before_candidate = FALSE
  /\ backlog_observed = FALSE
  /\ candidate_processed = FALSE
  /\ last_run_updated = FALSE
  /\ idle_budget_preserved = FALSE

CheckCase(c) ==
  /\ candidate = "none"
  /\ candidate' = c
  /\ pipeline_entered' = ActualPipelineEntered(c)
  /\ recovery_included' = ActualRecoveryIncluded(c)
  /\ deadline_bypassed' = ActualDeadlineBypassed(c)
  /\ budget_set_wakeup' = ActualBudgetSetsWakeup(c)
  /\ wakeup_cleared' = ActualWakeupCleared(c)
  /\ event_rescheduled' = ActualEventRescheduled(c)
  /\ reschedule_before_candidate' = ActualRescheduleBeforeCandidate(c)
  /\ backlog_observed' = ActualBacklogObserved(c)
  /\ candidate_processed' = ActualCandidateProcessed(c)
  /\ last_run_updated' = ActualLastRunUpdated(c)
  /\ idle_budget_preserved' = ActualIdleBudgetPreserved(c)

Next ==
  \/ \E c \in Cases : CheckCase(c)
  \/ /\ candidate # "none"
     /\ UNCHANGED vars

TypeInvariant ==
  /\ candidate \in Cases \union {"none"}
  /\ pipeline_entered \in BOOLEAN
  /\ recovery_included \in BOOLEAN
  /\ deadline_bypassed \in BOOLEAN
  /\ budget_set_wakeup \in BOOLEAN
  /\ wakeup_cleared \in BOOLEAN
  /\ event_rescheduled \in BOOLEAN
  /\ reschedule_before_candidate \in BOOLEAN
  /\ backlog_observed \in BOOLEAN
  /\ candidate_processed \in BOOLEAN
  /\ last_run_updated \in BOOLEAN
  /\ idle_budget_preserved \in BOOLEAN

PipelineEntryMatchesSpec ==
  candidate = "none" \/ pipeline_entered = SpecPipelineEntered(candidate)

TickQueueSaturationAloneDoesNotEnterPipeline ==
  candidate = "tick_queue_saturated_no_candidate" => ~pipeline_entered

EventEntryIsUnconditionalBeforeBudget ==
  candidate \in (EventCases \ {"event_budget_exhausted"}) => pipeline_entered

RecoveryInclusionMatchesSpec ==
  candidate = "none" \/ recovery_included = SpecRecoveryIncluded(candidate)

RecoveryCandidatesNeedWakeupOrSaturation ==
  candidate = "candidate_recovery_without_include" => ~recovery_included

RecoveryCandidatesRespectActiveBlocker ==
  candidate = "candidate_recovery_filtered_by_active_without_qc" => ~recovery_included

DeadlineBypassMatchesSpec ==
  candidate = "none" \/ deadline_bypassed = SpecDeadlineBypassed(candidate)

DeadlineBypassRequiresActivePendingWork ==
  deadline_bypassed => HasActiveCandidate(candidate)

BudgetExhaustionSetsWakeup ==
  candidate = "none" \/ budget_set_wakeup = SpecBudgetSetsWakeup(candidate)

BudgetExhaustionStopsCandidateProcessing ==
  (BudgetStartExhausted(candidate) \/ BudgetDuringCandidate(candidate)) => ~candidate_processed

WakeupClearMatchesTickEntry ==
  candidate = "none" \/ wakeup_cleared = SpecWakeupCleared(candidate)

EventRescheduleMatchesSpec ==
  candidate = "none" \/ event_rescheduled = SpecEventRescheduled(candidate)

EventReschedulesBeforeCandidateProcessing ==
  candidate = "none" \/ reschedule_before_candidate = SpecRescheduleBeforeCandidate(candidate)

BacklogObservationMatchesSpec ==
  candidate = "none" \/ backlog_observed = SpecBacklogObserved(candidate)

BacklogDoesNotCreateCandidates ==
  candidate = "event_backlogged_no_candidate" =>
    /\ ~candidate_processed
    /\ ~last_run_updated

CandidateProcessingMatchesSpec ==
  candidate = "none" \/ candidate_processed = SpecCandidateProcessed(candidate)

CandidateProcessingRequiresPipelineEntry ==
  candidate_processed => pipeline_entered

LastRunUpdateMatchesSpec ==
  candidate = "none" \/ last_run_updated = SpecLastRunUpdated(candidate)

LastRunUpdatesOnlyForCandidatePasses ==
  last_run_updated => HasActiveCandidate(candidate)

IdleBudgetPreservationMatchesSpec ==
  candidate = "none" \/ idle_budget_preserved = SpecIdleBudgetPreserved(candidate)

IdleBudgetRequiresWakeupAndCandidate ==
  idle_budget_preserved =>
    /\ HasCommitWakeup(candidate)
    /\ HasActiveCandidate(candidate)

CommitPipelineEntryExact ==
  \A c \in Cases:
    ActualPipelineEntered(c) = SpecPipelineEntered(c)

CommitPipelineRecoveryScopeExact ==
  \A c \in Cases:
    ActualRecoveryIncluded(c) = SpecRecoveryIncluded(c)

CommitPipelineDeadlineBudgetExact ==
  \A c \in Cases:
    /\ ActualDeadlineBypassed(c) = SpecDeadlineBypassed(c)
    /\ ActualBudgetSetsWakeup(c) = SpecBudgetSetsWakeup(c)

CommitPipelineWakeupEventExact ==
  \A c \in Cases:
    /\ ActualWakeupCleared(c) = SpecWakeupCleared(c)
    /\ ActualEventRescheduled(c) = SpecEventRescheduled(c)
    /\ ActualRescheduleBeforeCandidate(c) = SpecRescheduleBeforeCandidate(c)
    /\ ActualBacklogObserved(c) = SpecBacklogObserved(c)

CommitPipelineCandidateProgressExact ==
  \A c \in Cases:
    /\ ActualCandidateProcessed(c) = SpecCandidateProcessed(c)
    /\ ActualLastRunUpdated(c) = SpecLastRunUpdated(c)

CommitPipelineIdleBudgetExact ==
  \A c \in Cases:
    ActualIdleBudgetPreserved(c) = SpecIdleBudgetPreserved(c)

CommitPipelineSchedulingExactness ==
  /\ CommitPipelineEntryExact
  /\ CommitPipelineRecoveryScopeExact
  /\ CommitPipelineDeadlineBudgetExact
  /\ CommitPipelineWakeupEventExact
  /\ CommitPipelineCandidateProgressExact
  /\ CommitPipelineIdleBudgetExact

Safety ==
  /\ PipelineEntryMatchesSpec
  /\ TickQueueSaturationAloneDoesNotEnterPipeline
  /\ EventEntryIsUnconditionalBeforeBudget
  /\ RecoveryInclusionMatchesSpec
  /\ RecoveryCandidatesNeedWakeupOrSaturation
  /\ RecoveryCandidatesRespectActiveBlocker
  /\ DeadlineBypassMatchesSpec
  /\ DeadlineBypassRequiresActivePendingWork
  /\ BudgetExhaustionSetsWakeup
  /\ BudgetExhaustionStopsCandidateProcessing
  /\ WakeupClearMatchesTickEntry
  /\ EventRescheduleMatchesSpec
  /\ EventReschedulesBeforeCandidateProcessing
  /\ BacklogObservationMatchesSpec
  /\ BacklogDoesNotCreateCandidates
  /\ CandidateProcessingMatchesSpec
  /\ CandidateProcessingRequiresPipelineEntry
  /\ LastRunUpdateMatchesSpec
  /\ LastRunUpdatesOnlyForCandidatePasses
  /\ IdleBudgetPreservationMatchesSpec
  /\ IdleBudgetRequiresWakeupAndCandidate

CommitPipelineSchedulingFastSafety ==
  /\ Safety
  /\ CommitPipelineSchedulingExactness

SafetyFast ==
  CommitPipelineSchedulingFastSafety

=============================================================================
====
