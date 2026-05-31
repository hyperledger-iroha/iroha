---- MODULE SumeragiNewViewStatsGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the Sumeragi NEW_VIEW statistics helper.

This slice captures `new_view_stats::note_receipt(...)` and
`new_view_stats::snapshot_counts()`. It abstracts peers and `(height, view)`
keys into a small finite set while preserving the observable contracts: receipt
senders are deduplicated per key, counts are per key rather than per event, the
bounded `BTreeMap` prunes the lexicographically oldest key only after exceeding
capacity, an inserted key that is itself lexicographically oldest can be pruned
immediately and return zero, and snapshots expose sorted flat counts.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

EmptySnapshot == "empty_snapshot"
FirstReceipt == "first_receipt"
DuplicateSender == "duplicate_sender"
DistinctSender == "distinct_sender"
DistinctViewSameSender == "distinct_view_same_sender"
AtCapNoPrune == "at_cap_no_prune"
NewerInsertPrunesOldest == "newer_insert_prunes_oldest"
OlderInsertPrunesItself == "older_insert_prunes_itself"
HeightDominatesViewPrune == "height_dominates_view_prune"
SnapshotLexicographicOrder == "snapshot_lexicographic_order"

Cases == {
  EmptySnapshot,
  FirstReceipt,
  DuplicateSender,
  DistinctSender,
  DistinctViewSameSender,
  AtCapNoPrune,
  NewerInsertPrunesOldest,
  OlderInsertPrunesItself,
  HeightDominatesViewPrune,
  SnapshotLexicographicOrder
}

Return0 == 1
Return1 == 2
Return2 == 3
SnapshotLen0 == 4
SnapshotLen1 == 5
SnapshotLen2 == 6
SnapshotLen3 == 7
SnapshotLen4 == 8
HasK10 == 9
HasK11 == 10
HasK20 == 11
HasK30 == 12
HasK40 == 13
NoK10 == 14
NoK11 == 15
NoK20 == 16
NoK30 == 17
NoK40 == 18
CountK10One == 19
CountK10Two == 20
CountK11One == 21
CountK20One == 22
CountK30One == 23
CountK40One == 24
OrderK10K11 == 25
OrderK10K20K30 == 26
OrderK20K30K40 == 27
OrderK11K20K30 == 28
OrderK10K11K20 == 29
OrderK20K10K11 == 30
OrderK20K30 == 31
OrderK10K20K30K40 == 32
OrderK10K30K40 == 33
OrderK11K30K40 == 34

Actions == 1..34

SpecActions(c) ==
  CASE c = EmptySnapshot ->
      {SnapshotLen0}
    [] c = FirstReceipt ->
      {Return1, SnapshotLen1, HasK10, CountK10One}
    [] c = DuplicateSender ->
      {Return1, SnapshotLen1, HasK10, CountK10One}
    [] c = DistinctSender ->
      {Return2, SnapshotLen1, HasK10, CountK10Two}
    [] c = DistinctViewSameSender ->
      {Return1, SnapshotLen2, HasK10, HasK11, CountK10One, CountK11One,
       OrderK10K11}
    [] c = AtCapNoPrune ->
      {Return1, SnapshotLen3, HasK10, HasK20, HasK30, CountK10One,
       CountK20One, CountK30One, OrderK10K20K30}
    [] c = NewerInsertPrunesOldest ->
      {Return1, SnapshotLen3, NoK10, HasK20, HasK30, HasK40, CountK20One,
       CountK30One, CountK40One, OrderK20K30K40}
    [] c = OlderInsertPrunesItself ->
      {Return0, SnapshotLen3, NoK10, HasK20, HasK30, HasK40, CountK20One,
       CountK30One, CountK40One, OrderK20K30K40}
    [] c = HeightDominatesViewPrune ->
      {Return1, SnapshotLen3, NoK11, HasK20, HasK30, HasK40, CountK20One,
       CountK30One, CountK40One, OrderK20K30K40}
    [] c = SnapshotLexicographicOrder ->
      {Return1, SnapshotLen3, HasK10, HasK11, HasK20, CountK10One,
       CountK11One, CountK20One, OrderK10K11K20}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "empty_snapshot_nonzero"
       /\ c = EmptySnapshot ->
      (spec \ {SnapshotLen0}) \cup {SnapshotLen1}
    [] Bug = "duplicate_sender_increments"
       /\ c = DuplicateSender ->
      (spec \ {Return1, CountK10One}) \cup {Return2, CountK10Two}
    [] Bug = "distinct_sender_ignored"
       /\ c = DistinctSender ->
      (spec \ {Return2, CountK10Two}) \cup {Return1, CountK10One}
    [] Bug = "same_sender_cross_key_dedup"
       /\ c = DistinctViewSameSender ->
      (spec \ {SnapshotLen2, HasK11, CountK11One, OrderK10K11})
        \cup {SnapshotLen1, NoK11}
    [] Bug = "cap_prunes_at_limit"
       /\ c = AtCapNoPrune ->
      (spec \ {SnapshotLen3, HasK10, CountK10One, OrderK10K20K30})
        \cup {SnapshotLen2, NoK10, OrderK20K30}
    [] Bug = "cap_never_prunes"
       /\ c = NewerInsertPrunesOldest ->
      (spec \ {SnapshotLen3, NoK10, OrderK20K30K40})
        \cup {SnapshotLen4, HasK10, OrderK10K20K30K40}
    [] Bug = "prune_newest"
       /\ c = NewerInsertPrunesOldest ->
      (spec \ {NoK10, HasK40, CountK40One, OrderK20K30K40})
        \cup {HasK10, NoK40, OrderK10K20K30}
    [] Bug = "prune_insertion_order_not_key_order"
       /\ c = OlderInsertPrunesItself ->
      (spec \ {Return0, NoK10, HasK20, CountK20One, OrderK20K30K40})
        \cup {Return1, HasK10, NoK20, OrderK10K30K40}
    [] Bug = "older_insert_returns_inserted_count"
       /\ c = OlderInsertPrunesItself ->
      (spec \ {Return0}) \cup {Return1}
    [] Bug = "snapshot_unsorted"
       /\ c = SnapshotLexicographicOrder ->
      (spec \ {OrderK10K11K20}) \cup {OrderK20K10K11}
    [] Bug = "snapshot_counts_events"
       /\ c = DuplicateSender ->
      (spec \ {Return1, CountK10One}) \cup {Return2, CountK10Two}
    [] Bug = "snapshot_drops_view_from_key"
       /\ c = DistinctViewSameSender ->
      (spec \ {SnapshotLen2, HasK11, CountK11One, OrderK10K11})
        \cup {SnapshotLen1, NoK11, CountK10Two}
    [] Bug = "prune_orders_by_view_first"
       /\ c = HeightDominatesViewPrune ->
      (spec \ {NoK11, HasK20, CountK20One, OrderK20K30K40})
        \cup {HasK11, NoK20, OrderK11K30K40}
    [] Bug = "snapshot_includes_pruned_entry"
       /\ c = NewerInsertPrunesOldest ->
      (spec \ {SnapshotLen3, NoK10, OrderK20K30K40})
        \cup {SnapshotLen4, HasK10, OrderK10K20K30K40}
    [] Bug = "older_insert_keeps_pruned_key"
       /\ c = OlderInsertPrunesItself ->
      (spec \ {SnapshotLen3, NoK10, OrderK20K30K40})
        \cup {SnapshotLen4, HasK10, OrderK10K20K30K40}
    [] OTHER -> spec

Bugs == {
  "none",
  "empty_snapshot_nonzero",
  "duplicate_sender_increments",
  "distinct_sender_ignored",
  "same_sender_cross_key_dedup",
  "cap_prunes_at_limit",
  "cap_never_prunes",
  "prune_newest",
  "prune_insertion_order_not_key_order",
  "older_insert_returns_inserted_count",
  "snapshot_unsorted",
  "snapshot_counts_events",
  "snapshot_drops_view_from_key",
  "prune_orders_by_view_first",
  "snapshot_includes_pruned_entry",
  "older_insert_keeps_pruned_key"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

NoBugInvariant ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

SafetyFast == NoBugInvariant

BugEmptySnapshotNonzero == NoBugInvariant
BugDuplicateSenderIncrements == NoBugInvariant
BugDistinctSenderIgnored == NoBugInvariant
BugSameSenderCrossKeyDedup == NoBugInvariant
BugCapPrunesAtLimit == NoBugInvariant
BugCapNeverPrunes == NoBugInvariant
BugPruneNewest == NoBugInvariant
BugPruneInsertionOrderNotKeyOrder == NoBugInvariant
BugOlderInsertReturnsInsertedCount == NoBugInvariant
BugSnapshotUnsorted == NoBugInvariant
BugSnapshotCountsEvents == NoBugInvariant
BugSnapshotDropsViewFromKey == NoBugInvariant
BugPruneOrdersByViewFirst == NoBugInvariant
BugSnapshotIncludesPrunedEntry == NoBugInvariant
BugOlderInsertKeepsPrunedKey == NoBugInvariant

====
