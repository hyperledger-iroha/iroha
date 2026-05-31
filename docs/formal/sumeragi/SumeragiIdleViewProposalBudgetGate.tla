---- MODULE SumeragiIdleViewProposalBudgetGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for proposal-side idle-view budget preservation.

This slice models `should_preserve_idle_view_budget_for_proposal(...)` and
`should_retry_idle_view_after_proposal(...)`. A due proposal may borrow the
idle-view repair budget only when queued transaction work exists, no mode flip
or commit job is in flight, and proposal pressure is healthy or pacing-only
(queue saturation and/or consensus ingress pressure). Active pending blocks,
RBC backlog, and relay backpressure are hard stops. If idle repair was skipped
for a due proposal, it is retried after proposal handling only while the
frontier is still empty and no commit job owns it.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  preserve_budget,
  \* @type: Bool;
  idle_repair_deferred,
  \* @type: Bool;
  proposal_slot_reserved,
  \* @type: Bool;
  retry_requested,
  \* @type: Bool;
  retry_after_proposal

\* @type: <<Str, Bool, Bool, Bool, Bool, Bool>>;
vars == <<candidate, preserve_budget, idle_repair_deferred,
  proposal_slot_reserved, retry_requested, retry_after_proposal>>

Cases == {
  "preserve_no_queue",
  "preserve_mode_flip",
  "preserve_commit_inflight",
  "preserve_deadline_not_due",
  "preserve_healthy_due",
  "preserve_queue_saturated_due",
  "preserve_consensus_pacing_due",
  "preserve_combined_pacing_due",
  "preserve_active_pending_due",
  "preserve_rbc_backlog_due",
  "preserve_relay_backpressure_due",
  "preserve_active_pending_with_pacing",
  "preserve_rbc_with_consensus",
  "preserve_relay_with_queue",
  "retry_skipped_frontier_empty",
  "retry_not_skipped",
  "retry_no_queue",
  "retry_pending_blocks",
  "retry_commit_inflight"
}

PreserveCases == {
  "preserve_no_queue",
  "preserve_mode_flip",
  "preserve_commit_inflight",
  "preserve_deadline_not_due",
  "preserve_healthy_due",
  "preserve_queue_saturated_due",
  "preserve_consensus_pacing_due",
  "preserve_combined_pacing_due",
  "preserve_active_pending_due",
  "preserve_rbc_backlog_due",
  "preserve_relay_backpressure_due",
  "preserve_active_pending_with_pacing",
  "preserve_rbc_with_consensus",
  "preserve_relay_with_queue"
}
NoQueuePreserveCases == {"preserve_no_queue"}
ModeFlipCases == {"preserve_mode_flip"}
CommitInflightPreserveCases == {"preserve_commit_inflight"}
DeadlineNotDueCases == {"preserve_deadline_not_due"}
HealthyDueCases == {"preserve_healthy_due"}
QueueSaturatedPacingCases == {"preserve_queue_saturated_due"}
ConsensusPacingCases == {"preserve_consensus_pacing_due"}
CombinedPacingCases == {"preserve_combined_pacing_due"}
PacingAllowedCases ==
  QueueSaturatedPacingCases \union ConsensusPacingCases \union CombinedPacingCases
AllowedPreserveCases == HealthyDueCases \union PacingAllowedCases
ActivePendingCases == {
  "preserve_active_pending_due",
  "preserve_active_pending_with_pacing"
}
RbcBacklogCases == {
  "preserve_rbc_backlog_due",
  "preserve_rbc_with_consensus"
}
RelayBackpressureCases == {
  "preserve_relay_backpressure_due",
  "preserve_relay_with_queue"
}
HardBackpressureCases ==
  ActivePendingCases \union RbcBacklogCases \union RelayBackpressureCases

RetryCases == {
  "retry_skipped_frontier_empty",
  "retry_not_skipped",
  "retry_no_queue",
  "retry_pending_blocks",
  "retry_commit_inflight"
}
RetryAllowedCases == {"retry_skipped_frontier_empty"}
RetryNotSkippedCases == {"retry_not_skipped"}
RetryNoQueueCases == {"retry_no_queue"}
RetryPendingBlocksCases == {"retry_pending_blocks"}
RetryCommitInflightCases == {"retry_commit_inflight"}

SpecPreserveBudget(c) == c \in AllowedPreserveCases
SpecIdleRepairDeferred(c) == SpecPreserveBudget(c)
SpecProposalSlotReserved(c) == SpecPreserveBudget(c)
SpecRetryRequested(c) == c \in RetryAllowedCases
SpecRetryAfterProposal(c) == SpecRetryRequested(c)

ActualPreserveBudget(c) ==
  \/ /\ SpecPreserveBudget(c)
     /\ ~(Bug = "skip_healthy_due" /\ c \in HealthyDueCases)
     /\ ~(Bug = "skip_queue_saturated_pacing"
          /\ c \in QueueSaturatedPacingCases)
     /\ ~(Bug = "skip_consensus_pacing"
          /\ c \in ConsensusPacingCases)
     /\ ~(Bug = "skip_combined_pacing"
          /\ c \in CombinedPacingCases)
  \/ /\ c \in NoQueuePreserveCases
     /\ Bug = "preserve_without_queue"
  \/ /\ c \in ModeFlipCases
     /\ Bug = "preserve_during_mode_flip"
  \/ /\ c \in CommitInflightPreserveCases
     /\ Bug = "preserve_during_commit_inflight"
  \/ /\ c \in DeadlineNotDueCases
     /\ Bug = "preserve_before_deadline"
  \/ /\ c \in {"preserve_active_pending_due"}
     /\ Bug = "preserve_active_pending"
  \/ /\ c \in {"preserve_rbc_backlog_due"}
     /\ Bug = "preserve_rbc_backlog"
  \/ /\ c \in {"preserve_relay_backpressure_due"}
     /\ Bug = "preserve_relay_backpressure"
  \/ /\ c \in {"preserve_active_pending_with_pacing"}
     /\ Bug = "ignore_active_pending_with_pacing"
  \/ /\ c \in {"preserve_rbc_with_consensus"}
     /\ Bug = "ignore_rbc_with_pacing"
  \/ /\ c \in {"preserve_relay_with_queue"}
     /\ Bug = "ignore_relay_with_pacing"

ActualIdleRepairDeferred(c) ==
  /\ SpecIdleRepairDeferred(c)
  /\ ActualPreserveBudget(c)
  /\ Bug # "run_idle_repair_when_preserved"

ActualProposalSlotReserved(c) ==
  \/ /\ SpecProposalSlotReserved(c)
     /\ ActualPreserveBudget(c)
     /\ Bug # "preserve_without_proposal_slot"
  \/ /\ ~SpecProposalSlotReserved(c)
     /\ Bug = "reserve_proposal_without_preserve"

ActualRetryRequested(c) ==
  \/ /\ SpecRetryRequested(c)
     /\ Bug # "skip_retry_frontier_empty"
  \/ /\ c \in RetryNotSkippedCases
     /\ Bug = "retry_without_skip"
  \/ /\ c \in RetryNoQueueCases
     /\ Bug = "retry_without_queue"
  \/ /\ c \in RetryPendingBlocksCases
     /\ Bug = "retry_with_pending_blocks"
  \/ /\ c \in RetryCommitInflightCases
     /\ Bug = "retry_with_commit_inflight"

ActualRetryAfterProposal(c) ==
  /\ ActualRetryRequested(c)
  /\ Bug # "retry_runs_before_proposal"

Init ==
  /\ candidate = "none"
  /\ preserve_budget = FALSE
  /\ idle_repair_deferred = FALSE
  /\ proposal_slot_reserved = FALSE
  /\ retry_requested = FALSE
  /\ retry_after_proposal = FALSE

CheckCase(c) ==
  /\ candidate = "none"
  /\ candidate' = c
  /\ preserve_budget' = ActualPreserveBudget(c)
  /\ idle_repair_deferred' = ActualIdleRepairDeferred(c)
  /\ proposal_slot_reserved' = ActualProposalSlotReserved(c)
  /\ retry_requested' = ActualRetryRequested(c)
  /\ retry_after_proposal' = ActualRetryAfterProposal(c)

Next ==
  \/ \E c \in Cases : CheckCase(c)
  \/ /\ candidate # "none"
     /\ UNCHANGED vars

TypeInvariant ==
  /\ candidate \in Cases \union {"none"}
  /\ preserve_budget \in BOOLEAN
  /\ idle_repair_deferred \in BOOLEAN
  /\ proposal_slot_reserved \in BOOLEAN
  /\ retry_requested \in BOOLEAN
  /\ retry_after_proposal \in BOOLEAN

CasePartitionExact ==
  /\ Cases = PreserveCases \union RetryCases
  /\ PreserveCases \intersect RetryCases = {}
  /\ PreserveCases =
       NoQueuePreserveCases \union ModeFlipCases
       \union CommitInflightPreserveCases \union DeadlineNotDueCases
       \union AllowedPreserveCases \union HardBackpressureCases
  /\ AllowedPreserveCases = HealthyDueCases \union PacingAllowedCases
  /\ AllowedPreserveCases \intersect HardBackpressureCases = {}
  /\ RetryCases =
       RetryAllowedCases \union RetryNotSkippedCases \union RetryNoQueueCases
       \union RetryPendingBlocksCases \union RetryCommitInflightCases

PreserveMatchesSpec ==
  candidate = "none" \/ preserve_budget = SpecPreserveBudget(candidate)

IdleRepairDeferralMatchesSpec ==
  candidate = "none" \/
    idle_repair_deferred = SpecIdleRepairDeferred(candidate)

ProposalSlotReservationMatchesSpec ==
  candidate = "none" \/
    proposal_slot_reserved = SpecProposalSlotReserved(candidate)

RetryMatchesSpec ==
  candidate = "none" \/ retry_requested = SpecRetryRequested(candidate)

RetryOrderingMatchesSpec ==
  candidate = "none" \/ retry_after_proposal = SpecRetryAfterProposal(candidate)

AllowedDueProposalPreservesBudget ==
  candidate \in AllowedPreserveCases =>
    /\ preserve_budget
    /\ idle_repair_deferred
    /\ proposal_slot_reserved

NoQueueModeFlipCommitOrEarlyDeadlineDoNotPreserve ==
  candidate \in (NoQueuePreserveCases \union ModeFlipCases
    \union CommitInflightPreserveCases \union DeadlineNotDueCases) =>
    /\ ~preserve_budget
    /\ ~idle_repair_deferred
    /\ ~proposal_slot_reserved

HardBackpressureDoesNotPreserveBudget ==
  candidate \in HardBackpressureCases =>
    /\ ~preserve_budget
    /\ ~idle_repair_deferred
    /\ ~proposal_slot_reserved

PreserveDefersIdleRepairAndReservesProposal ==
  preserve_budget =>
    /\ idle_repair_deferred
    /\ proposal_slot_reserved

NoPreserveDoesNotReserveProposal ==
  ~preserve_budget => ~proposal_slot_reserved

RetryOnlyAfterSkippedDueProposalWithEmptyFrontier ==
  candidate \in RetryAllowedCases =>
    /\ retry_requested
    /\ retry_after_proposal

RetryRequiresSkippedProposal ==
  retry_requested => candidate \notin RetryNotSkippedCases

RetryRequiresQueuedWork ==
  retry_requested => candidate \notin RetryNoQueueCases

RetryRequiresEmptyFrontier ==
  retry_requested => candidate \notin RetryPendingBlocksCases

RetryRequiresNoCommitInflight ==
  retry_requested => candidate \notin RetryCommitInflightCases

RetryRunsAfterProposalHandling ==
  retry_requested => retry_after_proposal

RetrySuppressorsStayQuiet ==
  candidate \in (RetryNotSkippedCases \union RetryNoQueueCases
    \union RetryPendingBlocksCases \union RetryCommitInflightCases) =>
    /\ ~retry_requested
    /\ ~retry_after_proposal

Safety ==
  /\ CasePartitionExact
  /\ PreserveMatchesSpec
  /\ IdleRepairDeferralMatchesSpec
  /\ ProposalSlotReservationMatchesSpec
  /\ RetryMatchesSpec
  /\ RetryOrderingMatchesSpec
  /\ AllowedDueProposalPreservesBudget
  /\ NoQueueModeFlipCommitOrEarlyDeadlineDoNotPreserve
  /\ HardBackpressureDoesNotPreserveBudget
  /\ PreserveDefersIdleRepairAndReservesProposal
  /\ NoPreserveDoesNotReserveProposal
  /\ RetryOnlyAfterSkippedDueProposalWithEmptyFrontier
  /\ RetryRequiresSkippedProposal
  /\ RetryRequiresQueuedWork
  /\ RetryRequiresEmptyFrontier
  /\ RetryRequiresNoCommitInflight
  /\ RetryRunsAfterProposalHandling
  /\ RetrySuppressorsStayQuiet

=============================================================================
