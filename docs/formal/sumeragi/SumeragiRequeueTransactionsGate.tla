---- MODULE SumeragiRequeueTransactionsGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for transaction requeue helpers after Sumeragi commit
failure.

This slice covers `requeue_block_transactions(...)`,
`requeue_block_transactions_skipping_known_committed(...)`,
`block_external_transaction_hashes(...)`,
`drop_pending_block_and_requeue(...)`, and
`drop_pending_block_and_requeue_skipping_known_committed(...)` from
`crates/iroha_core/src/sumeragi/main_loop.rs`.

It abstracts queue/state/routing outcomes as finite cases while preserving the
observable contract:
- known committed hashes are checked before state, queue, routing, or push and
  count as duplicate work without gossip,
- state-committed transactions also count as duplicate work without gossip,
- already queued transactions count as duplicate work and are included in the
  returned gossip hash list,
- route plans prefer cached routing, fall back through stateless then
  stateful routing, and stateful routing failure counts as a requeue failure,
- push outcomes distinguish success, queue duplicate, chain duplicate, and
  other failure,
- gossip notification is emitted exactly when the returned gossip hash list is
  non-empty, and
- dropping a pending block removes it, reports the block transaction count, and
  returns the delegated requeue counters.
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
  "known_committed_skip",
  "state_committed_skip",
  "queue_contains_skip",
  "ledger_plan_push_ok",
  "without_state_plan_push_ok",
  "state_fallback_plan_push_ok",
  "state_route_failure",
  "push_is_in_queue",
  "push_in_blockchain",
  "push_other_error",
  "mixed_batch_counts",
  "empty_gossip_no_notify",
  "nonempty_gossip_notifies",
  "block_hashes_dedup",
  "drop_missing_pending",
  "drop_present_pending"
}

KnownChecked == 1
StateChecked == 2
QueueChecked == 3
LedgerPlanUsed == 4
WithoutStatePlanUsed == 5
StatePlanFallbackUsed == 6
PushAttempted == 7
RequeuedIncremented == 8
FailureIncremented == 9
DuplicateIncremented == 10
GossipHashReturned == 11
NoGossipHashReturned == 12
GossipNotified == 13
GossipNotNotified == 14
PendingRemoved == 15
PendingPreserved == 16
DropReturnsNone == 17
TxCountPreserved == 18
DelegatedCountsReturned == 19
HashSetDeduped == 20
KnownShortCircuits == 21

SpecActions(c) ==
  CASE c = "known_committed_skip" ->
      {KnownChecked, KnownShortCircuits, DuplicateIncremented,
       NoGossipHashReturned, GossipNotNotified}
    [] c = "state_committed_skip" ->
      {KnownChecked, StateChecked, DuplicateIncremented,
       NoGossipHashReturned, GossipNotNotified}
    [] c = "queue_contains_skip" ->
      {KnownChecked, StateChecked, QueueChecked, DuplicateIncremented,
       GossipHashReturned, GossipNotified}
    [] c = "ledger_plan_push_ok" ->
      {KnownChecked, StateChecked, QueueChecked, LedgerPlanUsed,
       PushAttempted, RequeuedIncremented, GossipHashReturned, GossipNotified}
    [] c = "without_state_plan_push_ok" ->
      {KnownChecked, StateChecked, QueueChecked, WithoutStatePlanUsed,
       PushAttempted, RequeuedIncremented, GossipHashReturned, GossipNotified}
    [] c = "state_fallback_plan_push_ok" ->
      {KnownChecked, StateChecked, QueueChecked, StatePlanFallbackUsed,
       PushAttempted, RequeuedIncremented, GossipHashReturned, GossipNotified}
    [] c = "state_route_failure" ->
      {KnownChecked, StateChecked, QueueChecked, StatePlanFallbackUsed,
       FailureIncremented, NoGossipHashReturned, GossipNotNotified}
    [] c = "push_is_in_queue" ->
      {KnownChecked, StateChecked, QueueChecked, WithoutStatePlanUsed,
       PushAttempted, DuplicateIncremented, GossipHashReturned,
       GossipNotified}
    [] c = "push_in_blockchain" ->
      {KnownChecked, StateChecked, QueueChecked, WithoutStatePlanUsed,
       PushAttempted, DuplicateIncremented, NoGossipHashReturned,
       GossipNotNotified}
    [] c = "push_other_error" ->
      {KnownChecked, StateChecked, QueueChecked, WithoutStatePlanUsed,
       PushAttempted, FailureIncremented, NoGossipHashReturned,
       GossipNotNotified}
    [] c = "mixed_batch_counts" ->
      {KnownChecked, StateChecked, QueueChecked, StatePlanFallbackUsed,
       PushAttempted, RequeuedIncremented, FailureIncremented,
       DuplicateIncremented, GossipHashReturned, GossipNotified}
    [] c = "empty_gossip_no_notify" ->
      {NoGossipHashReturned, GossipNotNotified}
    [] c = "nonempty_gossip_notifies" ->
      {GossipHashReturned, GossipNotified}
    [] c = "block_hashes_dedup" ->
      {HashSetDeduped}
    [] c = "drop_missing_pending" ->
      {DropReturnsNone, PendingPreserved}
    [] c = "drop_present_pending" ->
      {PendingRemoved, TxCountPreserved, DelegatedCountsReturned}
    [] OTHER -> {}

ActualActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "known_committed_not_short_circuited"
       /\ c = "known_committed_skip" ->
      (spec \ {KnownShortCircuits, NoGossipHashReturned, GossipNotNotified})
        \cup {StateChecked, QueueChecked, PushAttempted, GossipHashReturned,
              GossipNotified}
    [] Bug = "known_committed_gossiped"
       /\ c = "known_committed_skip" ->
      (spec \ {NoGossipHashReturned, GossipNotNotified}) \cup
        {GossipHashReturned, GossipNotified}
    [] Bug = "state_committed_not_duplicate"
       /\ c = "state_committed_skip" ->
      spec \ {DuplicateIncremented}
    [] Bug = "queue_duplicate_not_gossiped"
       /\ c = "queue_contains_skip" ->
      (spec \ {GossipHashReturned, GossipNotified}) \cup
        {NoGossipHashReturned, GossipNotNotified}
    [] Bug = "ledger_plan_ignored"
       /\ c = "ledger_plan_push_ok" ->
      (spec \ {LedgerPlanUsed}) \cup {WithoutStatePlanUsed}
    [] Bug = "stateless_plan_ignored"
       /\ c = "without_state_plan_push_ok" ->
      (spec \ {WithoutStatePlanUsed}) \cup {StatePlanFallbackUsed}
    [] Bug = "state_fallback_skipped"
       /\ c = "state_fallback_plan_push_ok" ->
      (spec \ {StatePlanFallbackUsed, PushAttempted, RequeuedIncremented,
               GossipHashReturned, GossipNotified}) \cup
        {FailureIncremented, NoGossipHashReturned, GossipNotNotified}
    [] Bug = "route_failure_counted_duplicate"
       /\ c = "state_route_failure" ->
      (spec \ {FailureIncremented}) \cup {DuplicateIncremented}
    [] Bug = "push_ok_not_requeued"
       /\ c = "without_state_plan_push_ok" ->
      spec \ {RequeuedIncremented}
    [] Bug = "push_in_queue_not_gossiped"
       /\ c = "push_is_in_queue" ->
      (spec \ {GossipHashReturned, GossipNotified}) \cup
        {NoGossipHashReturned, GossipNotNotified}
    [] Bug = "push_in_blockchain_gossiped"
       /\ c = "push_in_blockchain" ->
      (spec \ {NoGossipHashReturned, GossipNotNotified}) \cup
        {GossipHashReturned, GossipNotified}
    [] Bug = "push_other_error_requeued"
       /\ c = "push_other_error" ->
      (spec \ {FailureIncremented}) \cup {RequeuedIncremented}
    [] Bug = "empty_gossip_notifies"
       /\ c = "empty_gossip_no_notify" ->
      {NoGossipHashReturned, GossipNotified}
    [] Bug = "nonempty_gossip_skips_notify"
       /\ c = "nonempty_gossip_notifies" ->
      {GossipHashReturned, GossipNotNotified}
    [] Bug = "mixed_batch_loses_failure"
       /\ c = "mixed_batch_counts" ->
      spec \ {FailureIncremented}
    [] Bug = "block_hashes_keep_duplicates"
       /\ c = "block_hashes_dedup" ->
      {}
    [] Bug = "drop_missing_returns_counts"
       /\ c = "drop_missing_pending" ->
      {PendingPreserved, TxCountPreserved, DelegatedCountsReturned}
    [] Bug = "drop_present_keeps_pending"
       /\ c = "drop_present_pending" ->
      (spec \ {PendingRemoved}) \cup {PendingPreserved}
    [] Bug = "drop_present_wrong_tx_count"
       /\ c = "drop_present_pending" ->
      spec \ {TxCountPreserved}
    [] Bug = "drop_present_drops_requeue_counts"
       /\ c = "drop_present_pending" ->
      spec \ {DelegatedCountsReturned}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

BugModes == {
  "none",
  "known_committed_not_short_circuited",
  "known_committed_gossiped",
  "state_committed_not_duplicate",
  "queue_duplicate_not_gossiped",
  "ledger_plan_ignored",
  "stateless_plan_ignored",
  "state_fallback_skipped",
  "route_failure_counted_duplicate",
  "push_ok_not_requeued",
  "push_in_queue_not_gossiped",
  "push_in_blockchain_gossiped",
  "push_other_error_requeued",
  "empty_gossip_notifies",
  "nonempty_gossip_skips_notify",
  "mixed_batch_loses_failure",
  "block_hashes_keep_duplicates",
  "drop_missing_returns_counts",
  "drop_present_keeps_pending",
  "drop_present_wrong_tx_count",
  "drop_present_drops_requeue_counts"
}

TypeInvariant ==
  /\ Bug \in BugModes
  /\ checked = 0

RequeueTransactionsMatchesSpec ==
  \A c \in Cases:
    ActualActions(c) = SpecActions(c)

SafetyFast ==
  RequeueTransactionsMatchesSpec

====
