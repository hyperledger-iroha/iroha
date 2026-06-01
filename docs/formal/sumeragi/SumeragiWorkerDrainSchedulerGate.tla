---- MODULE SumeragiWorkerDrainSchedulerGate ----

(***************************************************************************
A bounded abstract model for the Sumeragi worker-loop drain scheduler.

This slice models the consensus-facing decisions in `run_worker_iteration`,
`drain_mailbox`, and `select_next_tier`. It does not model message payload
semantics; the vote, QC, block, RBC, and validation handlers are covered by
other models. Instead it proves that queue scheduling preserves the intended
consensus liveness priorities: bounded vote preference, frontier body repair,
quorum-recovery vote drain, one non-vote payload escape after a vote-only time
slice, block backlog escape, low-priority service after high queues are empty,
result polling before ticks, tick-gap bypass for explicit wakeups, and
post-tick suppression after a budget-exhausted pre-tick drain.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Str;
  selectedTier,
  \* @type: Bool;
  handledMessage,
  \* @type: Bool;
  queueDrainRecorded,
  \* @type: Bool;
  budgetConsumed,
  \* @type: Bool;
  phaseProgress,
  \* @type: Bool;
  queueBudgetExhausted,
  \* @type: Bool;
  budgetExceeded,
  \* @type: Bool;
  postTickAllowed,
  \* @type: Bool;
  pollCommit,
  \* @type: Bool;
  pollValidation,
  \* @type: Bool;
  pollQcVerify,
  \* @type: Bool;
  pollVoteVerify,
  \* @type: Bool;
  pollRbcPersist,
  \* @type: Bool;
  syncHints,
  \* @type: Bool;
  tickRun,
  \* @type: Bool;
  tickUsesBusyGap,
  \* @type: Bool;
  tickBypassesGap

\* @type: <<Str, Str, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool, Bool>>;
vars ==
  <<candidate,
    selectedTier,
    handledMessage,
    queueDrainRecorded,
    budgetConsumed,
    phaseProgress,
    queueBudgetExhausted,
    budgetExceeded,
    postTickAllowed,
    pollCommit,
    pollValidation,
    pollQcVerify,
    pollVoteVerify,
    pollRbcPersist,
    syncHints,
    tickRun,
    tickUsesBusyGap,
    tickBypassesGap>>

Tiers == {
  "None",
  "Votes",
  "BlockPayload",
  "RbcChunks",
  "Blocks",
  "Consensus",
  "LaneRelay",
  "Background"
}

Cases == {
  "idle",
  "vote_only",
  "vote_with_payload_backlog",
  "after_vote_burst_payload",
  "frontier_body_repair_payload",
  "frontier_body_repair_block",
  "quorum_vote_over_payload",
  "overtime_payload_turn",
  "block_urgent_no_payload",
  "starved_payload_preempts_after_progress",
  "starved_payload_suppressed_first_turn",
  "force_vote_over_starved_payload",
  "starved_block_preempts_vote",
  "low_consensus_after_high_empty",
  "budget_zero_vote_skips",
  "budget_exhausted_pending_vote",
  "pre_tick_deadline_first_turn",
  "post_tick_deadline_stops",
  "result_poll_before_tick",
  "tick_due_busy_gap",
  "tick_due_bypass_gap",
  "post_tick_skipped_when_budget_exceeded"
}

FrontierRepairCases == {
  "frontier_body_repair_payload",
  "frontier_body_repair_block"
}

NoSelectionCases == {
  "idle",
  "budget_exhausted_pending_vote",
  "post_tick_deadline_stops",
  "result_poll_before_tick",
  "tick_due_busy_gap",
  "tick_due_bypass_gap",
  "post_tick_skipped_when_budget_exceeded"
}

SpecSelectedTier(c) ==
  CASE c = "idle" -> "None"
    [] c = "vote_only" -> "Votes"
    [] c = "vote_with_payload_backlog" -> "Votes"
    [] c = "after_vote_burst_payload" -> "BlockPayload"
    [] c = "frontier_body_repair_payload" -> "BlockPayload"
    [] c = "frontier_body_repair_block" -> "Blocks"
    [] c = "quorum_vote_over_payload" -> "Votes"
    [] c = "overtime_payload_turn" -> "BlockPayload"
    [] c = "block_urgent_no_payload" -> "Blocks"
    [] c = "starved_payload_preempts_after_progress" -> "RbcChunks"
    [] c = "starved_payload_suppressed_first_turn" -> "Votes"
    [] c = "force_vote_over_starved_payload" -> "Votes"
    [] c = "starved_block_preempts_vote" -> "Blocks"
    [] c = "low_consensus_after_high_empty" -> "Consensus"
    [] c = "budget_zero_vote_skips" -> "BlockPayload"
    [] c = "pre_tick_deadline_first_turn" -> "Votes"
    [] OTHER -> "None"

SpecHandled(c) ==
  SpecSelectedTier(c) # "None"

SpecQueueDrainRecorded(c) ==
  SpecHandled(c)

SpecBudgetConsumed(c) ==
  SpecHandled(c)

SpecPhaseProgress(c) ==
  SpecHandled(c)

SpecQueueBudgetExhausted(c) ==
  c = "budget_exhausted_pending_vote"

SpecBudgetExceeded(c) ==
  c = "post_tick_skipped_when_budget_exceeded"

SpecPostTickAllowed(c) ==
  c # "post_tick_skipped_when_budget_exceeded"

SpecPollCommit(c) == TRUE
SpecPollValidation(c) == TRUE
SpecPollQcVerify(c) == TRUE
SpecPollVoteVerify(c) == TRUE
SpecPollRbcPersist(c) == TRUE
SpecSyncHints(c) == TRUE

SpecTickRun(c) ==
  c \in {"tick_due_busy_gap", "tick_due_bypass_gap"}

SpecTickUsesBusyGap(c) ==
  c = "tick_due_busy_gap"

SpecTickBypassesGap(c) ==
  c = "tick_due_bypass_gap"

ActualSelectedTier(c) ==
  CASE c = "idle" /\ Bug = "idle_selects_message" -> "Votes"
    [] c = "vote_with_payload_backlog" /\ Bug = "select_payload_before_vote" -> "BlockPayload"
    [] c = "after_vote_burst_payload" /\ Bug = "ignore_vote_burst_limit" -> "Votes"
    [] c \in FrontierRepairCases /\ Bug = "ignore_frontier_body_repair" -> "Votes"
    [] c = "frontier_body_repair_block" /\ Bug = "frontier_body_chooses_wrong_tier" -> "BlockPayload"
    [] c = "quorum_vote_over_payload" /\ Bug = "ignore_quorum_vote_priority" -> "BlockPayload"
    [] c = "overtime_payload_turn" /\ Bug = "skip_overtime_payload_turn" -> "None"
    [] c = "block_urgent_no_payload" /\ Bug = "block_urgent_ignored" -> "Votes"
    [] c = "starved_payload_preempts_after_progress" /\ Bug = "starved_payload_ignored" -> "Votes"
    [] c = "starved_payload_suppressed_first_turn" /\ Bug = "starved_payload_not_suppressed_first" -> "BlockPayload"
    [] c = "force_vote_over_starved_payload" /\ Bug = "force_vote_over_payload_ignored" -> "RbcChunks"
    [] c = "starved_block_preempts_vote" /\ Bug = "starved_block_overridden_by_vote" -> "Votes"
    [] c = "low_consensus_after_high_empty" /\ Bug = "low_priority_starves" -> "None"
    [] c = "budget_zero_vote_skips" /\ Bug = "budget_zero_votes_selected" -> "Votes"
    [] c = "pre_tick_deadline_first_turn" /\ Bug = "pre_tick_deadline_blocks_first_turn" -> "None"
    [] c = "post_tick_deadline_stops" /\ Bug = "post_tick_deadline_processes" -> "Votes"
    [] OTHER -> SpecSelectedTier(c)

ActualHandled(c) ==
  CASE SpecHandled(c) /\ Bug = "selected_not_handled" -> FALSE
    [] c \in NoSelectionCases /\ Bug = "spurious_handle_without_selection" -> TRUE
    [] OTHER -> ActualSelectedTier(c) # "None"

ActualQueueDrainRecorded(c) ==
  CASE ActualHandled(c) /\ Bug = "missing_queue_drain_record" -> FALSE
    [] OTHER -> ActualHandled(c)

ActualBudgetConsumed(c) ==
  CASE ActualHandled(c) /\ Bug = "missing_budget_consume" -> FALSE
    [] OTHER -> ActualHandled(c)

ActualPhaseProgress(c) ==
  CASE ActualHandled(c) /\ Bug = "no_phase_progress" -> FALSE
    [] OTHER -> ActualHandled(c)

ActualQueueBudgetExhausted(c) ==
  CASE c = "budget_exhausted_pending_vote" /\ Bug = "budget_exhausted_not_flagged" -> FALSE
    [] OTHER -> SpecQueueBudgetExhausted(c)

ActualBudgetExceeded(c) ==
  CASE c = "post_tick_skipped_when_budget_exceeded" /\ Bug = "time_budget_exceeded_not_flagged" -> FALSE
    [] OTHER -> SpecBudgetExceeded(c)

ActualPostTickAllowed(c) ==
  CASE c = "post_tick_skipped_when_budget_exceeded" /\ Bug = "post_tick_runs_after_budget_exceeded" -> TRUE
    [] c = "idle" /\ Bug = "post_tick_skips_without_budget_exceeded" -> FALSE
    [] OTHER -> SpecPostTickAllowed(c)

ActualPollCommit(c) ==
  CASE Bug = "skip_commit_result_poll" -> FALSE
    [] OTHER -> SpecPollCommit(c)

ActualPollValidation(c) ==
  CASE Bug = "skip_validation_result_poll" -> FALSE
    [] OTHER -> SpecPollValidation(c)

ActualPollQcVerify(c) ==
  CASE Bug = "skip_qc_result_poll" -> FALSE
    [] OTHER -> SpecPollQcVerify(c)

ActualPollVoteVerify(c) ==
  CASE Bug = "skip_vote_result_poll" -> FALSE
    [] OTHER -> SpecPollVoteVerify(c)

ActualPollRbcPersist(c) ==
  CASE Bug = "skip_rbc_persist_poll" -> FALSE
    [] OTHER -> SpecPollRbcPersist(c)

ActualSyncHints(c) ==
  CASE Bug = "skip_sync_hints" -> FALSE
    [] OTHER -> SpecSyncHints(c)

ActualTickRun(c) ==
  CASE c = "tick_due_busy_gap" /\ Bug = "tick_busy_due_not_run" -> FALSE
    [] c = "tick_due_bypass_gap" /\ Bug = "tick_bypass_ignored" -> FALSE
    [] OTHER -> SpecTickRun(c)

ActualTickUsesBusyGap(c) ==
  CASE c = "tick_due_busy_gap" /\ Bug = "tick_busy_uses_idle_gap" -> FALSE
    [] OTHER -> SpecTickUsesBusyGap(c)

ActualTickBypassesGap(c) ==
  CASE c = "tick_due_bypass_gap" /\ Bug = "tick_bypass_ignored" -> FALSE
    [] OTHER -> SpecTickBypassesGap(c)

BugModes == {
  "none",
  "idle_selects_message",
  "select_payload_before_vote",
  "ignore_vote_burst_limit",
  "ignore_frontier_body_repair",
  "frontier_body_chooses_wrong_tier",
  "ignore_quorum_vote_priority",
  "skip_overtime_payload_turn",
  "block_urgent_ignored",
  "starved_payload_ignored",
  "starved_payload_not_suppressed_first",
  "force_vote_over_payload_ignored",
  "starved_block_overridden_by_vote",
  "low_priority_starves",
  "budget_zero_votes_selected",
  "budget_exhausted_not_flagged",
  "pre_tick_deadline_blocks_first_turn",
  "post_tick_deadline_processes",
  "selected_not_handled",
  "spurious_handle_without_selection",
  "missing_queue_drain_record",
  "missing_budget_consume",
  "no_phase_progress",
  "skip_commit_result_poll",
  "skip_validation_result_poll",
  "skip_qc_result_poll",
  "skip_vote_result_poll",
  "skip_rbc_persist_poll",
  "skip_sync_hints",
  "tick_busy_due_not_run",
  "tick_busy_uses_idle_gap",
  "tick_bypass_ignored",
  "time_budget_exceeded_not_flagged",
  "post_tick_runs_after_budget_exceeded",
  "post_tick_skips_without_budget_exceeded"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ candidate \in Cases
  /\ selectedTier \in Tiers
  /\ handledMessage \in BOOLEAN
  /\ queueDrainRecorded \in BOOLEAN
  /\ budgetConsumed \in BOOLEAN
  /\ phaseProgress \in BOOLEAN
  /\ queueBudgetExhausted \in BOOLEAN
  /\ budgetExceeded \in BOOLEAN
  /\ postTickAllowed \in BOOLEAN
  /\ pollCommit \in BOOLEAN
  /\ pollValidation \in BOOLEAN
  /\ pollQcVerify \in BOOLEAN
  /\ pollVoteVerify \in BOOLEAN
  /\ pollRbcPersist \in BOOLEAN
  /\ syncHints \in BOOLEAN
  /\ tickRun \in BOOLEAN
  /\ tickUsesBusyGap \in BOOLEAN
  /\ tickBypassesGap \in BOOLEAN

Init ==
  /\ candidate = "idle"
  /\ selectedTier = "None"
  /\ handledMessage = FALSE
  /\ queueDrainRecorded = FALSE
  /\ budgetConsumed = FALSE
  /\ phaseProgress = FALSE
  /\ queueBudgetExhausted = FALSE
  /\ budgetExceeded = FALSE
  /\ postTickAllowed = TRUE
  /\ pollCommit = TRUE
  /\ pollValidation = TRUE
  /\ pollQcVerify = TRUE
  /\ pollVoteVerify = TRUE
  /\ pollRbcPersist = TRUE
  /\ syncHints = TRUE
  /\ tickRun = FALSE
  /\ tickUsesBusyGap = FALSE
  /\ tickBypassesGap = FALSE

Apply(c) ==
  /\ candidate' = c
  /\ selectedTier' = ActualSelectedTier(c)
  /\ handledMessage' = ActualHandled(c)
  /\ queueDrainRecorded' = ActualQueueDrainRecorded(c)
  /\ budgetConsumed' = ActualBudgetConsumed(c)
  /\ phaseProgress' = ActualPhaseProgress(c)
  /\ queueBudgetExhausted' = ActualQueueBudgetExhausted(c)
  /\ budgetExceeded' = ActualBudgetExceeded(c)
  /\ postTickAllowed' = ActualPostTickAllowed(c)
  /\ pollCommit' = ActualPollCommit(c)
  /\ pollValidation' = ActualPollValidation(c)
  /\ pollQcVerify' = ActualPollQcVerify(c)
  /\ pollVoteVerify' = ActualPollVoteVerify(c)
  /\ pollRbcPersist' = ActualPollRbcPersist(c)
  /\ syncHints' = ActualSyncHints(c)
  /\ tickRun' = ActualTickRun(c)
  /\ tickUsesBusyGap' = ActualTickUsesBusyGap(c)
  /\ tickBypassesGap' = ActualTickBypassesGap(c)

Stable ==
  UNCHANGED vars

Next ==
  \/ \E c \in Cases: Apply(c)
  \/ Stable

MatchesSpec ==
  /\ selectedTier = SpecSelectedTier(candidate)
  /\ handledMessage = SpecHandled(candidate)
  /\ queueDrainRecorded = SpecQueueDrainRecorded(candidate)
  /\ budgetConsumed = SpecBudgetConsumed(candidate)
  /\ phaseProgress = SpecPhaseProgress(candidate)
  /\ queueBudgetExhausted = SpecQueueBudgetExhausted(candidate)
  /\ budgetExceeded = SpecBudgetExceeded(candidate)
  /\ postTickAllowed = SpecPostTickAllowed(candidate)
  /\ pollCommit = SpecPollCommit(candidate)
  /\ pollValidation = SpecPollValidation(candidate)
  /\ pollQcVerify = SpecPollQcVerify(candidate)
  /\ pollVoteVerify = SpecPollVoteVerify(candidate)
  /\ pollRbcPersist = SpecPollRbcPersist(candidate)
  /\ syncHints = SpecSyncHints(candidate)
  /\ tickRun = SpecTickRun(candidate)
  /\ tickUsesBusyGap = SpecTickUsesBusyGap(candidate)
  /\ tickBypassesGap = SpecTickBypassesGap(candidate)

IdleDoesNotSelect ==
  candidate = "idle" =>
    /\ selectedTier = "None"
    /\ ~handledMessage

VotePriorityWithBacklog ==
  candidate \in {"vote_only", "vote_with_payload_backlog"} =>
    selectedTier = "Votes"

VoteBurstYieldsPayloadAfterCap ==
  candidate = "after_vote_burst_payload" =>
    selectedTier = "BlockPayload"

FrontierRepairPreemptsVotes ==
  candidate \in FrontierRepairCases =>
    /\ selectedTier \in {"BlockPayload", "RbcChunks", "Blocks"}
    /\ selectedTier # "Votes"

QuorumRecoveryVoteDrainPreemptsPayload ==
  candidate = "quorum_vote_over_payload" =>
    selectedTier = "Votes"

OvertimeGrantsNonVoteProgress ==
  candidate = "overtime_payload_turn" =>
    /\ selectedTier = "BlockPayload"
    /\ handledMessage
    /\ queueDrainRecorded
    /\ budgetConsumed
    /\ phaseProgress

BlockBacklogEscapesVoteBias ==
  candidate \in {"block_urgent_no_payload", "starved_block_preempts_vote"} =>
    selectedTier = "Blocks"

StarvedPayloadPreemptsAfterProgress ==
  candidate = "starved_payload_preempts_after_progress" =>
    selectedTier = "RbcChunks"

FirstTurnPayloadStarvationSuppressed ==
  candidate = "starved_payload_suppressed_first_turn" =>
    selectedTier = "Votes"

ForceVoteDrainOverridesStarvedPayloadOnly ==
  /\ candidate = "force_vote_over_starved_payload" => selectedTier = "Votes"
  /\ candidate = "starved_block_preempts_vote" => selectedTier = "Blocks"

LowPrioritySelectedWhenHighEmpty ==
  candidate = "low_consensus_after_high_empty" =>
    selectedTier = "Consensus"

ZeroVoteBudgetCannotSelectVotes ==
  candidate = "budget_zero_vote_skips" =>
    selectedTier # "Votes"

QueueBudgetExhaustionRecorded ==
  candidate = "budget_exhausted_pending_vote" =>
    /\ queueBudgetExhausted
    /\ ~handledMessage

DeadlineBehavior ==
  /\ candidate = "pre_tick_deadline_first_turn" => handledMessage
  /\ candidate = "post_tick_deadline_stops" => ~handledMessage

HandledMessagesAdvanceAccounting ==
  handledMessage =>
    /\ queueDrainRecorded
    /\ budgetConsumed
    /\ phaseProgress

ResultPollingAlwaysRuns ==
  /\ pollCommit
  /\ pollValidation
  /\ pollQcVerify
  /\ pollVoteVerify
  /\ pollRbcPersist
  /\ syncHints

TickBusyGapAndBypass ==
  /\ candidate = "tick_due_busy_gap" =>
       /\ tickRun
       /\ tickUsesBusyGap
       /\ ~tickBypassesGap
  /\ candidate = "tick_due_bypass_gap" =>
       /\ tickRun
       /\ tickBypassesGap

BudgetExceededSuppressesPostTick ==
  candidate = "post_tick_skipped_when_budget_exceeded" =>
    /\ budgetExceeded
    /\ ~postTickAllowed

Safety ==
  /\ MatchesSpec
  /\ IdleDoesNotSelect
  /\ VotePriorityWithBacklog
  /\ VoteBurstYieldsPayloadAfterCap
  /\ FrontierRepairPreemptsVotes
  /\ QuorumRecoveryVoteDrainPreemptsPayload
  /\ OvertimeGrantsNonVoteProgress
  /\ BlockBacklogEscapesVoteBias
  /\ StarvedPayloadPreemptsAfterProgress
  /\ FirstTurnPayloadStarvationSuppressed
  /\ ForceVoteDrainOverridesStarvedPayloadOnly
  /\ LowPrioritySelectedWhenHighEmpty
  /\ ZeroVoteBudgetCannotSelectVotes
  /\ QueueBudgetExhaustionRecorded
  /\ DeadlineBehavior
  /\ HandledMessagesAdvanceAccounting
  /\ ResultPollingAlwaysRuns
  /\ TickBusyGapAndBypass
  /\ BudgetExceededSuppressesPostTick

=============================================================================
====
