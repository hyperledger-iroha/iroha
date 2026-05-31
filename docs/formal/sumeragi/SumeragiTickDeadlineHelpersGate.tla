---- MODULE SumeragiTickDeadlineHelpersGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for Sumeragi tick/deadline scheduling helpers.

This slice pins:
`merge_deadline(...)`,
`tick_work_budget_deadline(...)`,
`tick_budget_exhausted(...)`,
`commit_pipeline_tick_deadline(...)`,
`tick_heartbeat_log_due(...)`,
`tick_heartbeat_log_interval(...)`,
`pacemaker_queue_nudge_due(...)`, and the immediate/min-deadline structure of
`next_tick_deadline(...)`.

Deadlines are represented as bounded integer instants. `NoDeadline` stands for
`Option::None`; past concrete deadlines are clamped to `Now`, matching the Rust
helpers that use `.max(now)` before returning a future tick deadline.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoDeadline == -1
Now == 3
MaxTime == 6
ActiveHeartbeatInterval == 10
IdleHeartbeatInterval == 30
MinNudgeInterval == 2

MinInt(a, b) == IF a <= b THEN a ELSE b
MaxInt(a, b) == IF a >= b THEN a ELSE b

SaturatingAge(now, last) ==
  IF now >= last THEN now - last ELSE 0

ClampDue(deadline) ==
  IF deadline = NoDeadline THEN NoDeadline ELSE MaxInt(deadline, Now)

MergeCases == {
  "merge_both",
  "merge_current_only",
  "merge_candidate_only",
  "merge_none"
}

CurrentDeadline(c) ==
  CASE c = "merge_both" -> 5
    [] c = "merge_current_only" -> 4
    [] OTHER -> NoDeadline

CandidateDeadline(c) ==
  CASE c = "merge_both" -> 2
    [] c = "merge_candidate_only" -> 4
    [] OTHER -> NoDeadline

MergeDeadline(current, candidate) ==
  CASE current = NoDeadline -> candidate
    [] candidate = NoDeadline -> current
    [] OTHER -> MinInt(current, candidate)

SpecMerge(c) ==
  MergeDeadline(CurrentDeadline(c), CandidateDeadline(c))

ActualMerge(c) ==
  CASE Bug = "merge_uses_latest_deadline"
       /\ c = "merge_both" -> MaxInt(CurrentDeadline(c), CandidateDeadline(c))
    [] Bug = "merge_drops_current"
       /\ c = "merge_current_only" -> NoDeadline
    [] Bug = "merge_drops_candidate"
       /\ c = "merge_candidate_only" -> NoDeadline
    [] OTHER -> SpecMerge(c)

WorkCases == {
  "work_zero",
  "work_nonzero",
  "work_overflow"
}

TickStart(c) ==
  IF c = "work_overflow" THEN 5 ELSE 1

WorkCap(c) ==
  CASE c = "work_zero" -> 0
    [] c = "work_overflow" -> 2
    [] OTHER -> 2

SpecWorkDeadline(c) ==
  IF WorkCap(c) = 0 THEN
    NoDeadline
  ELSE IF TickStart(c) + WorkCap(c) > MaxTime THEN
    TickStart(c)
  ELSE
    TickStart(c) + WorkCap(c)

ActualWorkDeadline(c) ==
  CASE Bug = "work_zero_returns_start"
       /\ c = "work_zero" -> TickStart(c)
    [] Bug = "work_nonzero_disabled"
       /\ c = "work_nonzero" -> NoDeadline
    [] Bug = "work_overflow_saturates"
       /\ c = "work_overflow" -> MaxTime
    [] OTHER -> SpecWorkDeadline(c)

BudgetCases == {
  "budget_none",
  "budget_boundary",
  "budget_before",
  "budget_after"
}

BudgetDeadline(c) ==
  CASE c = "budget_none" -> NoDeadline
    [] c = "budget_before" -> 4
    [] OTHER -> 3

BudgetNow(c) ==
  CASE c = "budget_before" -> 3
    [] c = "budget_after" -> 4
    [] OTHER -> 3

SpecBudgetExhausted(c) ==
  BudgetDeadline(c) # NoDeadline /\ BudgetNow(c) >= BudgetDeadline(c)

ActualBudgetExhausted(c) ==
  CASE Bug = "budget_no_deadline_exhausted"
       /\ c = "budget_none" -> TRUE
    [] Bug = "budget_strict_boundary"
       /\ c = "budget_boundary" -> BudgetNow(c) > BudgetDeadline(c)
    [] Bug = "budget_future_deadline_exhausted"
       /\ c = "budget_before" -> TRUE
    [] OTHER -> SpecBudgetExhausted(c)

CommitCases == {
  "commit_wakeup_active",
  "commit_saturated_active",
  "commit_no_active",
  "commit_normal_active"
}

CommitInputDeadline(c) == 4

CommitActive(c) ==
  IF c = "commit_no_active" THEN 0 ELSE 1

CommitWakeup(c) ==
  c \in {"commit_wakeup_active", "commit_no_active"}

CommitSaturated(c) ==
  c \in {"commit_saturated_active", "commit_no_active"}

SpecCommitDeadline(c) ==
  IF CommitActive(c) > 0 /\ (CommitWakeup(c) \/ CommitSaturated(c)) THEN
    NoDeadline
  ELSE
    CommitInputDeadline(c)

ActualCommitDeadline(c) ==
  CASE Bug = "commit_bypass_without_active"
       /\ c = "commit_no_active" -> NoDeadline
    [] Bug = "commit_skip_wakeup_bypass"
       /\ c = "commit_wakeup_active" -> CommitInputDeadline(c)
    [] Bug = "commit_skip_saturation_bypass"
       /\ c = "commit_saturated_active" -> CommitInputDeadline(c)
    [] Bug = "commit_bypass_normal"
       /\ c = "commit_normal_active" -> NoDeadline
    [] OTHER -> SpecCommitDeadline(c)

HeartbeatDueCases == {
  "heartbeat_not_due",
  "heartbeat_boundary",
  "heartbeat_future_last",
  "heartbeat_zero_interval"
}

HeartbeatNow(c) ==
  CASE c = "heartbeat_not_due" -> 2
    [] c = "heartbeat_boundary" -> 3
    [] c = "heartbeat_future_last" -> 1
    [] OTHER -> 1

HeartbeatLast(c) ==
  CASE c = "heartbeat_future_last" -> 3
    [] c = "heartbeat_zero_interval" -> 1
    [] OTHER -> 1

HeartbeatInterval(c) ==
  IF c = "heartbeat_zero_interval" THEN 0 ELSE 2

SpecHeartbeatDue(c) ==
  SaturatingAge(HeartbeatNow(c), HeartbeatLast(c)) >= HeartbeatInterval(c)

ActualHeartbeatDue(c) ==
  CASE Bug = "heartbeat_due_strict_boundary"
       /\ c = "heartbeat_boundary" ->
       SaturatingAge(HeartbeatNow(c), HeartbeatLast(c)) > HeartbeatInterval(c)
    [] Bug = "heartbeat_due_future_underflows"
       /\ c = "heartbeat_future_last" ->
       HeartbeatLast(c) - HeartbeatNow(c) >= HeartbeatInterval(c)
    [] Bug = "heartbeat_due_zero_suppressed"
       /\ c = "heartbeat_zero_interval" -> FALSE
    [] OTHER -> SpecHeartbeatDue(c)

HeartbeatIntervalCases == {
  "heartbeat_idle",
  "heartbeat_busy",
  "heartbeat_saturated"
}

HeartbeatQueueLen(c) ==
  IF c = "heartbeat_busy" THEN 1 ELSE 0

HeartbeatSaturated(c) ==
  c = "heartbeat_saturated"

SpecHeartbeatInterval(c) ==
  IF HeartbeatSaturated(c) \/ HeartbeatQueueLen(c) > 0 THEN
    ActiveHeartbeatInterval
  ELSE
    IdleHeartbeatInterval

ActualHeartbeatInterval(c) ==
  CASE Bug = "heartbeat_interval_busy_idle"
       /\ c = "heartbeat_busy" -> IdleHeartbeatInterval
    [] Bug = "heartbeat_interval_saturated_idle"
       /\ c = "heartbeat_saturated" -> IdleHeartbeatInterval
    [] OTHER -> SpecHeartbeatInterval(c)

NudgeCases == {
  "nudge_first",
  "nudge_before_floor",
  "nudge_at_floor",
  "nudge_propose_long",
  "nudge_future_last"
}

NudgeHasLast(c) ==
  c # "nudge_first"

NudgeNow(c) ==
  CASE c = "nudge_before_floor" -> 1
    [] c = "nudge_at_floor" -> 2
    [] c = "nudge_propose_long" -> 3
    [] c = "nudge_future_last" -> 2
    [] OTHER -> 0

NudgeLast(c) ==
  IF c = "nudge_future_last" THEN 4 ELSE 0

NudgeProposeInterval(c) ==
  IF c = "nudge_propose_long" THEN 3 ELSE 0

NudgeInterval(c) ==
  MaxInt(NudgeProposeInterval(c), MinNudgeInterval)

SpecNudgeDue(c) ==
  \/ ~NudgeHasLast(c)
  \/ SaturatingAge(NudgeNow(c), NudgeLast(c)) >= NudgeInterval(c)

ActualNudgeDue(c) ==
  CASE Bug = "nudge_first_suppressed"
       /\ c = "nudge_first" -> FALSE
    [] Bug = "nudge_skips_min_floor"
       /\ c = "nudge_before_floor" ->
       SaturatingAge(NudgeNow(c), NudgeLast(c)) >= NudgeProposeInterval(c)
    [] Bug = "nudge_strict_boundary"
       /\ c = "nudge_at_floor" ->
       SaturatingAge(NudgeNow(c), NudgeLast(c)) > NudgeInterval(c)
    [] Bug = "nudge_future_underflows"
       /\ c = "nudge_future_last" ->
       NudgeLast(c) - NudgeNow(c) >= NudgeInterval(c)
    [] OTHER -> SpecNudgeDue(c)

NextCases == {
  "next_idle_none",
  "next_queue_immediate",
  "next_mode_flip_immediate",
  "next_pending_wakeup_immediate",
  "next_deferred_immediate",
  "next_min_candidate",
  "next_past_candidate"
}

NextImmediate(c) ==
  c \in {
    "next_queue_immediate",
    "next_mode_flip_immediate",
    "next_pending_wakeup_immediate",
    "next_deferred_immediate"
  }

SpecNextDeadline(c) ==
  CASE NextImmediate(c) -> Now
    [] c = "next_min_candidate" -> 2
    [] c = "next_past_candidate" -> Now
    [] OTHER -> NoDeadline

ActualNextDeadline(c) ==
  CASE Bug = "next_queue_waits"
       /\ c = "next_queue_immediate" -> 5
    [] Bug = "next_mode_flip_ignored"
       /\ c = "next_mode_flip_immediate" -> NoDeadline
    [] Bug = "next_pending_wakeup_ignored"
       /\ c = "next_pending_wakeup_immediate" -> NoDeadline
    [] Bug = "next_deferred_ignored"
       /\ c = "next_deferred_immediate" -> 5
    [] Bug = "next_uses_latest_due"
       /\ c = "next_min_candidate" -> 5
    [] Bug = "next_past_not_clamped"
       /\ c = "next_past_candidate" -> 1
    [] OTHER -> SpecNextDeadline(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "merge_uses_latest_deadline",
       "merge_drops_current",
       "merge_drops_candidate",
       "work_zero_returns_start",
       "work_nonzero_disabled",
       "work_overflow_saturates",
       "budget_no_deadline_exhausted",
       "budget_strict_boundary",
       "budget_future_deadline_exhausted",
       "commit_bypass_without_active",
       "commit_skip_wakeup_bypass",
       "commit_skip_saturation_bypass",
       "commit_bypass_normal",
       "heartbeat_due_strict_boundary",
       "heartbeat_due_future_underflows",
       "heartbeat_due_zero_suppressed",
       "heartbeat_interval_busy_idle",
       "heartbeat_interval_saturated_idle",
       "nudge_first_suppressed",
       "nudge_skips_min_floor",
       "nudge_strict_boundary",
       "nudge_future_underflows",
       "next_queue_waits",
       "next_mode_flip_ignored",
       "next_pending_wakeup_ignored",
       "next_deferred_ignored",
       "next_uses_latest_due",
       "next_past_not_clamped"
     }
  /\ checked = 0

SafetyFast ==
  /\ \A c \in MergeCases:
       ActualMerge(c) = SpecMerge(c)
  /\ \A c \in WorkCases:
       ActualWorkDeadline(c) = SpecWorkDeadline(c)
  /\ \A c \in BudgetCases:
       ActualBudgetExhausted(c) = SpecBudgetExhausted(c)
  /\ \A c \in CommitCases:
       ActualCommitDeadline(c) = SpecCommitDeadline(c)
  /\ \A c \in HeartbeatDueCases:
       ActualHeartbeatDue(c) = SpecHeartbeatDue(c)
  /\ \A c \in HeartbeatIntervalCases:
       ActualHeartbeatInterval(c) = SpecHeartbeatInterval(c)
  /\ \A c \in NudgeCases:
       ActualNudgeDue(c) = SpecNudgeDue(c)
  /\ \A c \in NextCases:
       ActualNextDeadline(c) = SpecNextDeadline(c)

BugMergeUsesLatestDeadline ==
  ActualMerge("merge_both") = SpecMerge("merge_both")

BugMergeDropsCurrent ==
  ActualMerge("merge_current_only") = SpecMerge("merge_current_only")

BugMergeDropsCandidate ==
  ActualMerge("merge_candidate_only") = SpecMerge("merge_candidate_only")

BugWorkZeroReturnsStart ==
  ActualWorkDeadline("work_zero") = SpecWorkDeadline("work_zero")

BugWorkNonzeroDisabled ==
  ActualWorkDeadline("work_nonzero") = SpecWorkDeadline("work_nonzero")

BugWorkOverflowSaturates ==
  ActualWorkDeadline("work_overflow") = SpecWorkDeadline("work_overflow")

BugBudgetNoDeadlineExhausted ==
  ActualBudgetExhausted("budget_none") = SpecBudgetExhausted("budget_none")

BugBudgetStrictBoundary ==
  ActualBudgetExhausted("budget_boundary") = SpecBudgetExhausted("budget_boundary")

BugBudgetFutureDeadlineExhausted ==
  ActualBudgetExhausted("budget_before") = SpecBudgetExhausted("budget_before")

BugCommitBypassWithoutActive ==
  ActualCommitDeadline("commit_no_active") = SpecCommitDeadline("commit_no_active")

BugCommitSkipWakeupBypass ==
  ActualCommitDeadline("commit_wakeup_active") =
    SpecCommitDeadline("commit_wakeup_active")

BugCommitSkipSaturationBypass ==
  ActualCommitDeadline("commit_saturated_active") =
    SpecCommitDeadline("commit_saturated_active")

BugCommitBypassNormal ==
  ActualCommitDeadline("commit_normal_active") =
    SpecCommitDeadline("commit_normal_active")

BugHeartbeatDueStrictBoundary ==
  ActualHeartbeatDue("heartbeat_boundary") = SpecHeartbeatDue("heartbeat_boundary")

BugHeartbeatDueFutureUnderflows ==
  ActualHeartbeatDue("heartbeat_future_last") =
    SpecHeartbeatDue("heartbeat_future_last")

BugHeartbeatDueZeroSuppressed ==
  ActualHeartbeatDue("heartbeat_zero_interval") =
    SpecHeartbeatDue("heartbeat_zero_interval")

BugHeartbeatIntervalBusyIdle ==
  ActualHeartbeatInterval("heartbeat_busy") =
    SpecHeartbeatInterval("heartbeat_busy")

BugHeartbeatIntervalSaturatedIdle ==
  ActualHeartbeatInterval("heartbeat_saturated") =
    SpecHeartbeatInterval("heartbeat_saturated")

BugNudgeFirstSuppressed ==
  ActualNudgeDue("nudge_first") = SpecNudgeDue("nudge_first")

BugNudgeSkipsMinFloor ==
  ActualNudgeDue("nudge_before_floor") = SpecNudgeDue("nudge_before_floor")

BugNudgeStrictBoundary ==
  ActualNudgeDue("nudge_at_floor") = SpecNudgeDue("nudge_at_floor")

BugNudgeFutureUnderflows ==
  ActualNudgeDue("nudge_future_last") = SpecNudgeDue("nudge_future_last")

BugNextQueueWaits ==
  ActualNextDeadline("next_queue_immediate") =
    SpecNextDeadline("next_queue_immediate")

BugNextModeFlipIgnored ==
  ActualNextDeadline("next_mode_flip_immediate") =
    SpecNextDeadline("next_mode_flip_immediate")

BugNextPendingWakeupIgnored ==
  ActualNextDeadline("next_pending_wakeup_immediate") =
    SpecNextDeadline("next_pending_wakeup_immediate")

BugNextDeferredIgnored ==
  ActualNextDeadline("next_deferred_immediate") =
    SpecNextDeadline("next_deferred_immediate")

BugNextUsesLatestDue ==
  ActualNextDeadline("next_min_candidate") =
    SpecNextDeadline("next_min_candidate")

BugNextPastNotClamped ==
  ActualNextDeadline("next_past_candidate") =
    SpecNextDeadline("next_past_candidate")

====
