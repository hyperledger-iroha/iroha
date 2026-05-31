---- MODULE SumeragiProposalBackpressureGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for proposal backpressure classification helpers.

This slice captures `ProposalBackpressure::{should_defer,
only_pacing_backpressure}` and the `proposal_backpressure_allows_queue_work(...)`
bridge. Queue saturation and consensus worker-queue pressure are pacing
signals: they defer immediately but still allow queued proposal work after the
pacemaker deadline. Active pending blocks, RBC backlog, and relay pressure are
hard stops; any hard stop suppresses the pacing-only classification even when
queue/consensus pressure is also present.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "healthy_none",
  "queue_saturated",
  "consensus_queue",
  "queue_and_consensus",
  "active_pending",
  "rbc_backlog",
  "relay_backpressure",
  "active_pending_with_queue",
  "active_pending_with_consensus",
  "rbc_backlog_with_queue",
  "rbc_backlog_with_consensus",
  "relay_backpressure_with_queue",
  "relay_backpressure_with_consensus",
  "all_signals"
}

QueueSaturatedCases == {
  "queue_saturated",
  "queue_and_consensus",
  "active_pending_with_queue",
  "rbc_backlog_with_queue",
  "relay_backpressure_with_queue",
  "all_signals"
}
ConsensusQueueCases == {
  "consensus_queue",
  "queue_and_consensus",
  "active_pending_with_consensus",
  "rbc_backlog_with_consensus",
  "relay_backpressure_with_consensus",
  "all_signals"
}
ActivePendingCases == {
  "active_pending",
  "active_pending_with_queue",
  "active_pending_with_consensus",
  "all_signals"
}
RbcBacklogCases == {
  "rbc_backlog",
  "rbc_backlog_with_queue",
  "rbc_backlog_with_consensus",
  "all_signals"
}
RelayBackpressureCases == {
  "relay_backpressure",
  "relay_backpressure_with_queue",
  "relay_backpressure_with_consensus",
  "all_signals"
}

QueueSaturated(c) == c \in QueueSaturatedCases
ConsensusQueue(c) == c \in ConsensusQueueCases
ActivePending(c) == c \in ActivePendingCases
RbcBacklog(c) == c \in RbcBacklogCases
RelayBackpressure(c) == c \in RelayBackpressureCases

HardBackpressure(c) ==
  ActivePending(c) \/ RbcBacklog(c) \/ RelayBackpressure(c)

SpecShouldDefer(c) ==
  QueueSaturated(c)
    \/ ConsensusQueue(c)
    \/ HardBackpressure(c)

SpecOnlyPacingBackpressure(c) ==
  /\ QueueSaturated(c) \/ ConsensusQueue(c)
  /\ ~HardBackpressure(c)

SpecAllowsQueueWork(c) ==
  ~SpecShouldDefer(c) \/ SpecOnlyPacingBackpressure(c)

ActualShouldDefer(c) ==
  CASE Bug = "should_defer_ignores_queue" ->
       ConsensusQueue(c) \/ HardBackpressure(c)
    [] Bug = "should_defer_ignores_consensus" ->
       QueueSaturated(c) \/ HardBackpressure(c)
    [] Bug = "should_defer_ignores_active_pending" ->
       QueueSaturated(c) \/ ConsensusQueue(c) \/ RbcBacklog(c)
         \/ RelayBackpressure(c)
    [] Bug = "should_defer_ignores_rbc_backlog" ->
       QueueSaturated(c) \/ ConsensusQueue(c) \/ ActivePending(c)
         \/ RelayBackpressure(c)
    [] Bug = "should_defer_ignores_relay" ->
       QueueSaturated(c) \/ ConsensusQueue(c) \/ ActivePending(c)
         \/ RbcBacklog(c)
    [] Bug = "should_defer_requires_all_signals" ->
       QueueSaturated(c) /\ ConsensusQueue(c) /\ ActivePending(c)
         /\ RbcBacklog(c) /\ RelayBackpressure(c)
    [] OTHER -> SpecShouldDefer(c)

ActualOnlyPacingBackpressure(c) ==
  CASE Bug = "only_pacing_ignores_queue" ->
       ConsensusQueue(c) /\ ~HardBackpressure(c)
    [] Bug = "only_pacing_ignores_consensus" ->
       QueueSaturated(c) /\ ~HardBackpressure(c)
    [] Bug = "only_pacing_requires_both_pacing" ->
       QueueSaturated(c) /\ ConsensusQueue(c) /\ ~HardBackpressure(c)
    [] Bug = "only_pacing_allows_active_pending" ->
       (QueueSaturated(c) \/ ConsensusQueue(c))
         /\ ~(RbcBacklog(c) \/ RelayBackpressure(c))
    [] Bug = "only_pacing_allows_rbc_backlog" ->
       (QueueSaturated(c) \/ ConsensusQueue(c))
         /\ ~(ActivePending(c) \/ RelayBackpressure(c))
    [] Bug = "only_pacing_allows_relay_backpressure" ->
       (QueueSaturated(c) \/ ConsensusQueue(c))
         /\ ~(ActivePending(c) \/ RbcBacklog(c))
    [] Bug = "only_pacing_true_without_pacing" ->
       ~HardBackpressure(c)
    [] Bug = "only_pacing_treats_any_defer_as_pacing" ->
       ActualShouldDefer(c)
    [] OTHER -> SpecOnlyPacingBackpressure(c)

ActualAllowsQueueWork(c) ==
  CASE Bug = "allows_queue_blocks_pacing" ->
       ~ActualShouldDefer(c)
    [] Bug = "allows_queue_requires_pacing_signal" ->
       ActualOnlyPacingBackpressure(c)
    [] Bug = "allows_queue_allows_hard_with_pacing" ->
       ~ActualShouldDefer(c) \/ QueueSaturated(c) \/ ConsensusQueue(c)
    [] Bug = "allows_queue_ignores_queue_pacing" ->
       ~ActualShouldDefer(c) \/
         (ConsensusQueue(c) /\ ~HardBackpressure(c))
    [] Bug = "allows_queue_ignores_consensus_pacing" ->
       ~ActualShouldDefer(c) \/
         (QueueSaturated(c) /\ ~HardBackpressure(c))
    [] OTHER ->
       ~ActualShouldDefer(c) \/ ActualOnlyPacingBackpressure(c)

Bugs == {
  "none",
  "should_defer_ignores_queue",
  "should_defer_ignores_consensus",
  "should_defer_ignores_active_pending",
  "should_defer_ignores_rbc_backlog",
  "should_defer_ignores_relay",
  "should_defer_requires_all_signals",
  "only_pacing_ignores_queue",
  "only_pacing_ignores_consensus",
  "only_pacing_requires_both_pacing",
  "only_pacing_allows_active_pending",
  "only_pacing_allows_rbc_backlog",
  "only_pacing_allows_relay_backpressure",
  "only_pacing_true_without_pacing",
  "only_pacing_treats_any_defer_as_pacing",
  "allows_queue_blocks_pacing",
  "allows_queue_requires_pacing_signal",
  "allows_queue_allows_hard_with_pacing",
  "allows_queue_ignores_queue_pacing",
  "allows_queue_ignores_consensus_pacing"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ ActualShouldDefer(c) \in BOOLEAN
       /\ ActualOnlyPacingBackpressure(c) \in BOOLEAN
       /\ ActualAllowsQueueWork(c) \in BOOLEAN

ShouldDeferMatchesSpec ==
  \A c \in Cases:
    ActualShouldDefer(c) = SpecShouldDefer(c)

OnlyPacingMatchesSpec ==
  \A c \in Cases:
    ActualOnlyPacingBackpressure(c) = SpecOnlyPacingBackpressure(c)

AllowsQueueWorkMatchesSpec ==
  \A c \in Cases:
    ActualAllowsQueueWork(c) = SpecAllowsQueueWork(c)

PacingSignalsDeferButAllowQueueWork ==
  \A c \in Cases:
    /\ (c \in {"queue_saturated", "consensus_queue", "queue_and_consensus"})
       => /\ ActualShouldDefer(c)
          /\ ActualOnlyPacingBackpressure(c)
          /\ ActualAllowsQueueWork(c)

HardBackpressureBlocksQueueWork ==
  \A c \in Cases:
    HardBackpressure(c) =>
      /\ ActualShouldDefer(c)
      /\ ~ActualOnlyPacingBackpressure(c)
      /\ ~ActualAllowsQueueWork(c)

HealthyQueueWorkAllowed ==
  /\ ~ActualShouldDefer("healthy_none")
  /\ ~ActualOnlyPacingBackpressure("healthy_none")
  /\ ActualAllowsQueueWork("healthy_none")

Safety ==
  /\ ShouldDeferMatchesSpec
  /\ OnlyPacingMatchesSpec
  /\ AllowsQueueWorkMatchesSpec
  /\ PacingSignalsDeferButAllowQueueWork
  /\ HardBackpressureBlocksQueueWork
  /\ HealthyQueueWorkAllowed

=============================================================================
