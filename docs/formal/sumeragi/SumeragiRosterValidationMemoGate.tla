---- MODULE SumeragiRosterValidationMemoGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the roster-validation memo caches.

This slice captures `MemoCache<K, V>` and the two-lane
`RosterValidationMemo` wrapper from `main_loop.rs`. Keys, values, and hashes
are collapsed into representative cases while preserving observable cache
contracts: construction empties entries/order and stores capacity, cache misses
do not touch recency, cache hits clone the value and move the key to the back,
zero-capacity inserts are no-ops, inserts touch and evict after updating,
existing-key inserts replace the value and deduplicate recency, eviction removes
oldest live keys until the capacity bound holds even when stale order entries
are present, commit-QC and checkpoint lanes stay isolated, and world refresh
replaces the memo with a fresh empty one.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NewEmpty == 1
NewCapacity == 2
GetMissReturnsNone == 3
GetMissPreservesOrder == 4
GetHitReturnsValue == 5
GetHitTouches == 6
InsertZeroCapacityNoop == 7
InsertNewUnderCap == 8
InsertNewAtCapEvictsOldest == 9
InsertExistingUpdates == 10
InsertExistingDedupsOrder == 11
TouchMovesToBack == 12
EvictUntilBound == 13
EvictSkipsStaleOrder == 14
CommitInsertIsolated == 15
CheckpointInsertIsolated == 16
CommitGetIsolated == 17
CheckpointGetIsolated == 18
RefreshClearsMemo == 19
MemoCachesShareCapacity == 20

Candidates == 1..20

EntriesEmpty == 1
OrderEmpty == 2
CapacityInput == 3
ReturnNone == 4
ReturnValue == 5
PreserveOrder == 6
TouchKey == 7
DedupOrder == 8
MoveKeyToBack == 9
NoInsert == 10
InsertEntry == 11
UpdateEntry == 12
UpdatedValue == 13
NoEvict == 14
EvictOldest == 15
EvictNewest == 16
CapacityBound == 17
DropStaleOrder == 18
ContinueEvict == 19
CommitChanged == 20
CommitUnchanged == 21
CheckpointChanged == 22
CheckpointUnchanged == 23
ReadCommit == 24
ReadCheckpoint == 25
CommitCleared == 26
CheckpointCleared == 27
NewMemo == 28
CommitCapacityInput == 29
CheckpointCapacityInput == 30
PreserveOldValue == 31

Actions == 1..31

SpecActions(candidate) ==
  CASE candidate = NewEmpty ->
      {EntriesEmpty, OrderEmpty}
    [] candidate = NewCapacity ->
      {CapacityInput}
    [] candidate = GetMissReturnsNone ->
      {ReturnNone}
    [] candidate = GetMissPreservesOrder ->
      {ReturnNone, PreserveOrder}
    [] candidate = GetHitReturnsValue ->
      {ReturnValue}
    [] candidate = GetHitTouches ->
      {ReturnValue, TouchKey, DedupOrder, MoveKeyToBack}
    [] candidate = InsertZeroCapacityNoop ->
      {NoInsert, PreserveOrder, NoEvict}
    [] candidate = InsertNewUnderCap ->
      {InsertEntry, TouchKey, MoveKeyToBack, NoEvict, CapacityBound}
    [] candidate = InsertNewAtCapEvictsOldest ->
      {InsertEntry, TouchKey, MoveKeyToBack, EvictOldest, CapacityBound}
    [] candidate = InsertExistingUpdates ->
      {UpdateEntry, UpdatedValue, TouchKey, DedupOrder, MoveKeyToBack, NoEvict}
    [] candidate = InsertExistingDedupsOrder ->
      {UpdateEntry, DedupOrder, MoveKeyToBack}
    [] candidate = TouchMovesToBack ->
      {TouchKey, DedupOrder, MoveKeyToBack}
    [] candidate = EvictUntilBound ->
      {EvictOldest, CapacityBound}
    [] candidate = EvictSkipsStaleOrder ->
      {DropStaleOrder, ContinueEvict, EvictOldest, CapacityBound}
    [] candidate = CommitInsertIsolated ->
      {CommitChanged, CheckpointUnchanged}
    [] candidate = CheckpointInsertIsolated ->
      {CheckpointChanged, CommitUnchanged}
    [] candidate = CommitGetIsolated ->
      {ReadCommit, CheckpointUnchanged, ReturnValue}
    [] candidate = CheckpointGetIsolated ->
      {ReadCheckpoint, CommitUnchanged, ReturnValue}
    [] candidate = RefreshClearsMemo ->
      {CommitCleared, CheckpointCleared, NewMemo}
    [] candidate = MemoCachesShareCapacity ->
      {CommitCapacityInput, CheckpointCapacityInput}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = NewEmpty /\ Bug = "new_not_empty" ->
      (spec \ {EntriesEmpty, OrderEmpty}) \cup {InsertEntry}
    [] candidate = NewCapacity /\ Bug = "new_wrong_capacity" ->
      spec \ {CapacityInput}
    [] candidate = GetMissReturnsNone /\ Bug = "get_miss_returns_value" ->
      (spec \ {ReturnNone}) \cup {ReturnValue}
    [] candidate = GetMissPreservesOrder /\ Bug = "get_miss_touches_order" ->
      (spec \ {PreserveOrder}) \cup {TouchKey, MoveKeyToBack}
    [] candidate = GetHitReturnsValue /\ Bug = "get_hit_returns_none" ->
      (spec \ {ReturnValue}) \cup {ReturnNone}
    [] candidate = GetHitTouches /\ Bug = "get_hit_skips_touch" ->
      (spec \ {TouchKey, DedupOrder, MoveKeyToBack}) \cup {PreserveOrder}
    [] candidate = InsertZeroCapacityNoop /\
          Bug = "insert_zero_capacity_inserts" ->
      (spec \ {NoInsert, PreserveOrder, NoEvict}) \cup
        {InsertEntry, TouchKey}
    [] candidate = InsertNewUnderCap /\ Bug = "insert_new_missing_entry" ->
      spec \ {InsertEntry}
    [] candidate = InsertNewUnderCap /\ Bug = "insert_new_skips_touch" ->
      (spec \ {TouchKey, MoveKeyToBack}) \cup {PreserveOrder}
    [] candidate = InsertNewAtCapEvictsOldest /\
          Bug = "insert_new_skips_evict" ->
      (spec \ {EvictOldest, CapacityBound}) \cup {NoEvict}
    [] candidate = InsertExistingUpdates /\
          Bug = "insert_existing_keeps_old_value" ->
      (spec \ {UpdateEntry, UpdatedValue}) \cup {PreserveOldValue}
    [] candidate = InsertExistingDedupsOrder /\
          Bug = "insert_existing_duplicates_order" ->
      spec \ {DedupOrder}
    [] candidate = TouchMovesToBack /\ Bug = "touch_keeps_old_position" ->
      (spec \ {DedupOrder, MoveKeyToBack}) \cup {PreserveOrder}
    [] candidate = EvictUntilBound /\ Bug = "evict_keeps_over_capacity" ->
      spec \ {CapacityBound}
    [] candidate = EvictUntilBound /\ Bug = "evict_drops_newest" ->
      (spec \ {EvictOldest}) \cup {EvictNewest}
    [] candidate = EvictSkipsStaleOrder /\
          Bug = "evict_stops_on_stale_order" ->
      (spec \ {ContinueEvict, EvictOldest, CapacityBound}) \cup {NoEvict}
    [] candidate = CommitInsertIsolated /\
          Bug = "commit_insert_updates_checkpoint" ->
      (spec \ {CheckpointUnchanged}) \cup {CheckpointChanged}
    [] candidate = CheckpointInsertIsolated /\
          Bug = "checkpoint_insert_updates_commit" ->
      (spec \ {CommitUnchanged}) \cup {CommitChanged}
    [] candidate = CommitGetIsolated /\ Bug = "commit_get_reads_checkpoint" ->
      (spec \ {ReadCommit, CheckpointUnchanged}) \cup {ReadCheckpoint}
    [] candidate = CheckpointGetIsolated /\ Bug = "checkpoint_get_reads_commit" ->
      (spec \ {ReadCheckpoint, CommitUnchanged}) \cup {ReadCommit}
    [] candidate = RefreshClearsMemo /\ Bug = "refresh_keeps_old_memo" ->
      (spec \ {CommitCleared, CheckpointCleared, NewMemo}) \cup
        {CommitUnchanged, CheckpointUnchanged}
    [] candidate = MemoCachesShareCapacity /\
          Bug = "memo_checkpoint_wrong_capacity" ->
      spec \ {CheckpointCapacityInput}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "new_not_empty",
       "new_wrong_capacity",
       "get_miss_returns_value",
       "get_miss_touches_order",
       "get_hit_returns_none",
       "get_hit_skips_touch",
       "insert_zero_capacity_inserts",
       "insert_new_missing_entry",
       "insert_new_skips_touch",
       "insert_new_skips_evict",
       "insert_existing_keeps_old_value",
       "insert_existing_duplicates_order",
       "touch_keeps_old_position",
       "evict_keeps_over_capacity",
       "evict_drops_newest",
       "evict_stops_on_stale_order",
       "commit_insert_updates_checkpoint",
       "checkpoint_insert_updates_commit",
       "commit_get_reads_checkpoint",
       "checkpoint_get_reads_commit",
       "refresh_keeps_old_memo",
       "memo_checkpoint_wrong_capacity"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

Safety ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

BugNewNotEmpty ==
  ImplementationActions(NewEmpty) = SpecActions(NewEmpty)

BugNewWrongCapacity ==
  ImplementationActions(NewCapacity) = SpecActions(NewCapacity)

BugGetMissReturnsValue ==
  ImplementationActions(GetMissReturnsNone) = SpecActions(GetMissReturnsNone)

BugGetMissTouchesOrder ==
  ImplementationActions(GetMissPreservesOrder) =
    SpecActions(GetMissPreservesOrder)

BugGetHitReturnsNone ==
  ImplementationActions(GetHitReturnsValue) = SpecActions(GetHitReturnsValue)

BugGetHitSkipsTouch ==
  ImplementationActions(GetHitTouches) = SpecActions(GetHitTouches)

BugInsertZeroCapacityInserts ==
  ImplementationActions(InsertZeroCapacityNoop) =
    SpecActions(InsertZeroCapacityNoop)

BugInsertNewMissingEntry ==
  ImplementationActions(InsertNewUnderCap) = SpecActions(InsertNewUnderCap)

BugInsertNewSkipsTouch ==
  ImplementationActions(InsertNewUnderCap) = SpecActions(InsertNewUnderCap)

BugInsertNewSkipsEvict ==
  ImplementationActions(InsertNewAtCapEvictsOldest) =
    SpecActions(InsertNewAtCapEvictsOldest)

BugInsertExistingKeepsOldValue ==
  ImplementationActions(InsertExistingUpdates) =
    SpecActions(InsertExistingUpdates)

BugInsertExistingDuplicatesOrder ==
  ImplementationActions(InsertExistingDedupsOrder) =
    SpecActions(InsertExistingDedupsOrder)

BugTouchKeepsOldPosition ==
  ImplementationActions(TouchMovesToBack) = SpecActions(TouchMovesToBack)

BugEvictKeepsOverCapacity ==
  ImplementationActions(EvictUntilBound) = SpecActions(EvictUntilBound)

BugEvictDropsNewest ==
  ImplementationActions(EvictUntilBound) = SpecActions(EvictUntilBound)

BugEvictStopsOnStaleOrder ==
  ImplementationActions(EvictSkipsStaleOrder) =
    SpecActions(EvictSkipsStaleOrder)

BugCommitInsertUpdatesCheckpoint ==
  ImplementationActions(CommitInsertIsolated) =
    SpecActions(CommitInsertIsolated)

BugCheckpointInsertUpdatesCommit ==
  ImplementationActions(CheckpointInsertIsolated) =
    SpecActions(CheckpointInsertIsolated)

BugCommitGetReadsCheckpoint ==
  ImplementationActions(CommitGetIsolated) = SpecActions(CommitGetIsolated)

BugCheckpointGetReadsCommit ==
  ImplementationActions(CheckpointGetIsolated) =
    SpecActions(CheckpointGetIsolated)

BugRefreshKeepsOldMemo ==
  ImplementationActions(RefreshClearsMemo) = SpecActions(RefreshClearsMemo)

BugMemoCheckpointWrongCapacity ==
  ImplementationActions(MemoCachesShareCapacity) =
    SpecActions(MemoCachesShareCapacity)

====
