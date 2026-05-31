---- MODULE SumeragiRbcChunkBroadcastOrderGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `compute_chunk_broadcast_order(...)`.

The helper builds an initial `0..chunk_count` order, optionally shuffles that
order only for multi-chunk payloads, and then drops every nth position when a
positive drop interval is configured. Filtering must preserve the preexisting
order and must never create duplicate or out-of-range chunk indices.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Bool;
  shuffle_applied,
  \* @type: Int;
  output_len,
  \* @type: Int;
  dropped_count,
  \* @type: Int;
  first_index,
  \* @type: Int;
  last_index,
  \* @type: Bool;
  has_duplicates,
  \* @type: Bool;
  has_out_of_range,
  \* @type: Bool;
  preserves_no_shuffle_order,
  \* @type: Bool;
  preserves_filter_order

\* @type: <<Str, Bool, Int, Int, Int, Int, Bool, Bool, Bool, Bool>>;
vars ==
  <<candidate, shuffle_applied, output_len, dropped_count, first_index,
    last_index, has_duplicates, has_out_of_range, preserves_no_shuffle_order,
    preserves_filter_order>>

Cases == {
  "empty_no_drop",
  "one_shuffle_no_drop",
  "six_no_shuffle_no_drop",
  "six_shuffle_no_drop",
  "six_drop_every_two",
  "five_drop_every_two",
  "six_drop_every_three",
  "six_drop_every_one",
  "six_drop_zero",
  "six_drop_large",
  "six_shuffle_drop_two"
}

NoIndex == 64
CountValues == 0..64
IndexValues == 0..64

ChunkCount(c) ==
  CASE c = "empty_no_drop" -> 0
    [] c = "one_shuffle_no_drop" -> 1
    [] c = "five_drop_every_two" -> 5
    [] OTHER -> 6

ShuffleRequested(c) ==
  c \in {
    "one_shuffle_no_drop",
    "six_shuffle_no_drop",
    "six_shuffle_drop_two"
  }

DropIntervalPresent(c) ==
  c \in {
    "six_drop_every_two",
    "five_drop_every_two",
    "six_drop_every_three",
    "six_drop_every_one",
    "six_drop_zero",
    "six_drop_large",
    "six_shuffle_drop_two"
  }

DropEvery(c) ==
  CASE c \in {"six_drop_every_two", "five_drop_every_two", "six_shuffle_drop_two"} -> 2
    [] c = "six_drop_every_three" -> 3
    [] c = "six_drop_every_one" -> 1
    [] c = "six_drop_large" -> 9
    [] OTHER -> 0

EffectiveDropEnabled(c) ==
  DropIntervalPresent(c) /\ DropEvery(c) > 0

SpecShuffleApplied(c) ==
  ShuffleRequested(c) /\ ChunkCount(c) > 1

SpecDroppedCount(c) ==
  IF EffectiveDropEnabled(c)
  THEN ChunkCount(c) \div DropEvery(c)
  ELSE 0

SpecOutputLen(c) ==
  ChunkCount(c) - SpecDroppedCount(c)

SpecNoShuffleFirst(c) ==
  CASE c \in {"empty_no_drop", "six_drop_every_one"} -> NoIndex
    [] OTHER -> 0

SpecNoShuffleLast(c) ==
  CASE c = "empty_no_drop" -> NoIndex
    [] c = "one_shuffle_no_drop" -> 0
    [] c = "six_no_shuffle_no_drop" -> 5
    [] c = "six_drop_every_two" -> 4
    [] c = "five_drop_every_two" -> 4
    [] c = "six_drop_every_three" -> 4
    [] c = "six_drop_every_one" -> NoIndex
    [] c = "six_drop_zero" -> 5
    [] c = "six_drop_large" -> 5
    [] OTHER -> 0

SpecFirstIndex(c) ==
  IF SpecOutputLen(c) = 0
  THEN NoIndex
  ELSE IF SpecShuffleApplied(c)
  THEN 0
  ELSE SpecNoShuffleFirst(c)

SpecLastIndex(c) ==
  IF SpecOutputLen(c) = 0
  THEN NoIndex
  ELSE IF SpecShuffleApplied(c)
  THEN 0
  ELSE SpecNoShuffleLast(c)

ActualShuffleApplied(c) ==
  CASE Bug = "shuffle_singleton" /\ c = "one_shuffle_no_drop" -> TRUE
    [] Bug = "skip_shuffle" /\ c = "six_shuffle_no_drop" -> FALSE
    [] OTHER -> SpecShuffleApplied(c)

ActualDroppedCount(c) ==
  CASE Bug = "drop_zero_enabled" /\ c = "six_drop_zero" -> 6
    [] Bug = "drop_every_one_keeps_one" /\ c = "six_drop_every_one" -> 5
    [] Bug = "drop_floor_off_by_one" /\ c = "five_drop_every_two" -> 3
    [] Bug = "drop_large_drops_one" /\ c = "six_drop_large" -> 1
    [] Bug = "drop_none_drops_one" /\ c = "six_no_shuffle_no_drop" -> 1
    [] OTHER -> SpecDroppedCount(c)

ActualOutputLen(c) ==
  CASE Bug = "drop_zero_enabled" /\ c = "six_drop_zero" -> 0
    [] Bug = "drop_every_one_keeps_one" /\ c = "six_drop_every_one" -> 1
    [] Bug = "drop_floor_off_by_one" /\ c = "five_drop_every_two" -> 2
    [] Bug = "drop_large_drops_one" /\ c = "six_drop_large" -> 5
    [] Bug = "drop_none_drops_one" /\ c = "six_no_shuffle_no_drop" -> 5
    [] OTHER -> SpecOutputLen(c)

ActualFirstIndex(c) ==
  CASE Bug = "drop_uses_zero_based_position" /\ c = "six_drop_every_two" -> 1
    [] Bug = "no_shuffle_reversed" /\ c = "six_no_shuffle_no_drop" -> 5
    [] Bug = "drop_zero_enabled" /\ c = "six_drop_zero" -> NoIndex
    [] Bug = "drop_every_one_keeps_one" /\ c = "six_drop_every_one" -> 0
    [] OTHER -> SpecFirstIndex(c)

ActualLastIndex(c) ==
  CASE Bug = "drop_uses_zero_based_position" /\ c = "six_drop_every_two" -> 5
    [] Bug = "no_shuffle_reversed" /\ c = "six_no_shuffle_no_drop" -> 0
    [] Bug = "drop_zero_enabled" /\ c = "six_drop_zero" -> NoIndex
    [] Bug = "drop_every_one_keeps_one" /\ c = "six_drop_every_one" -> 0
    [] Bug = "drop_large_drops_one" /\ c = "six_drop_large" -> 4
    [] Bug = "drop_none_drops_one" /\ c = "six_no_shuffle_no_drop" -> 4
    [] OTHER -> SpecLastIndex(c)

ActualHasDuplicates(c) ==
  CASE Bug = "duplicates_index" /\ c = "six_shuffle_drop_two" -> TRUE
    [] OTHER -> FALSE

ActualHasOutOfRange(c) ==
  CASE Bug = "out_of_range_index" /\ c = "six_shuffle_drop_two" -> TRUE
    [] OTHER -> FALSE

ActualPreservesNoShuffleOrder(c) ==
  CASE Bug = "no_shuffle_reversed" /\ c = "six_no_shuffle_no_drop" -> FALSE
    [] OTHER -> TRUE

ActualPreservesFilterOrder(c) ==
  CASE Bug = "drop_does_not_preserve_filter_order" /\ c = "six_drop_every_two" -> FALSE
    [] OTHER -> TRUE

TypeInvariant ==
  /\ Bug \in {
       "none",
       "shuffle_singleton",
       "skip_shuffle",
       "drop_zero_enabled",
       "drop_every_one_keeps_one",
       "drop_floor_off_by_one",
       "drop_uses_zero_based_position",
       "drop_does_not_preserve_filter_order",
       "no_shuffle_reversed",
       "duplicates_index",
       "out_of_range_index",
       "drop_large_drops_one",
       "drop_none_drops_one"
     }
  /\ candidate \in Cases
  /\ shuffle_applied \in BOOLEAN
  /\ output_len \in CountValues
  /\ dropped_count \in CountValues
  /\ first_index \in IndexValues
  /\ last_index \in IndexValues
  /\ has_duplicates \in BOOLEAN
  /\ has_out_of_range \in BOOLEAN
  /\ preserves_no_shuffle_order \in BOOLEAN
  /\ preserves_filter_order \in BOOLEAN

Init ==
  /\ candidate \in Cases
  /\ shuffle_applied = ActualShuffleApplied(candidate)
  /\ output_len = ActualOutputLen(candidate)
  /\ dropped_count = ActualDroppedCount(candidate)
  /\ first_index = ActualFirstIndex(candidate)
  /\ last_index = ActualLastIndex(candidate)
  /\ has_duplicates = ActualHasDuplicates(candidate)
  /\ has_out_of_range = ActualHasOutOfRange(candidate)
  /\ preserves_no_shuffle_order = ActualPreservesNoShuffleOrder(candidate)
  /\ preserves_filter_order = ActualPreservesFilterOrder(candidate)

Next ==
  UNCHANGED vars

ShuffleApplicationMatchesSpec ==
  shuffle_applied = SpecShuffleApplied(candidate)

DroppedCountMatchesSpec ==
  dropped_count = SpecDroppedCount(candidate)

OutputLengthMatchesSpec ==
  output_len = SpecOutputLen(candidate)

OutputLengthPlusDroppedEqualsInput ==
  output_len + dropped_count = ChunkCount(candidate)

DropNoneKeepsAll ==
  ~DropIntervalPresent(candidate) =>
    /\ output_len = ChunkCount(candidate)
    /\ dropped_count = 0

DropZeroIgnored ==
  DropIntervalPresent(candidate) /\ DropEvery(candidate) = 0 =>
    /\ output_len = ChunkCount(candidate)
    /\ dropped_count = 0

DropEveryOneDropsAll ==
  DropEvery(candidate) = 1 =>
    /\ output_len = 0
    /\ dropped_count = ChunkCount(candidate)
    /\ first_index = NoIndex
    /\ last_index = NoIndex

DropLargeDropsNone ==
  DropEvery(candidate) > ChunkCount(candidate) =>
    /\ output_len = ChunkCount(candidate)
    /\ dropped_count = 0

NoShufflePreservesOrder ==
  ~SpecShuffleApplied(candidate) => preserves_no_shuffle_order

NoShuffleFirstAndLastMatchSpec ==
  ~SpecShuffleApplied(candidate) =>
    /\ first_index = SpecFirstIndex(candidate)
    /\ last_index = SpecLastIndex(candidate)

FilteredOrderPreserved ==
  preserves_filter_order

NoDuplicateIndices ==
  ~has_duplicates

NoOutOfRangeIndices ==
  ~has_out_of_range

ShuffleDoesNotChangeCardinalityWithoutDrop ==
  ShuffleRequested(candidate) /\ ChunkCount(candidate) > 1 /\ ~EffectiveDropEnabled(candidate) =>
    /\ output_len = ChunkCount(candidate)
    /\ dropped_count = 0

DropCountUsesOneBasedPositions ==
  EffectiveDropEnabled(candidate) =>
    dropped_count = ChunkCount(candidate) \div DropEvery(candidate)

Safety ==
  /\ ShuffleApplicationMatchesSpec
  /\ DroppedCountMatchesSpec
  /\ OutputLengthMatchesSpec
  /\ OutputLengthPlusDroppedEqualsInput
  /\ DropNoneKeepsAll
  /\ DropZeroIgnored
  /\ DropEveryOneDropsAll
  /\ DropLargeDropsNone
  /\ NoShufflePreservesOrder
  /\ NoShuffleFirstAndLastMatchSpec
  /\ FilteredOrderPreserved
  /\ NoDuplicateIndices
  /\ NoOutOfRangeIndices
  /\ ShuffleDoesNotChangeCardinalityWithoutDrop
  /\ DropCountUsesOneBasedPositions

====
