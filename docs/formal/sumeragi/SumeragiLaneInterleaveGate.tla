---- MODULE SumeragiLaneInterleaveGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for lane interleaving of routing decisions.

This slice pins `interleave_lane_indices_for_slot(...)` and
`interleave_lane_indices_from_offset(...)`: indices are grouped by lane id,
lane ids are traversed in sorted order, each lane preserves its original
intra-lane order, slot height/view rotate the starting lane when more than one
lane is present, and degenerate empty/single-lane inputs fall back to original
index order.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoIndex == -1

\* Output vectors are padded with NoIndex after the logical order length.
Cases == {
  "empty",
  "single_item",
  "single_lane",
  "two_lanes_balanced",
  "two_lanes_skewed",
  "offset_rotates",
  "offset_wraps",
  "sorted_lanes_not_first_seen",
  "three_lanes_missing_later"
}

\* @type: Str => Int;
Total(c) ==
  CASE c = "empty" -> 0
    [] c = "single_item" -> 1
    [] c = "single_lane" -> 3
    [] c = "two_lanes_balanced" -> 4
    [] c = "two_lanes_skewed" -> 5
    [] c = "offset_rotates" -> 4
    [] c = "offset_wraps" -> 4
    [] c = "sorted_lanes_not_first_seen" -> 4
    [] c = "three_lanes_missing_later" -> 5
    [] OTHER -> 0

\* @type: Str => Int;
LaneCount(c) ==
  CASE c = "empty" -> 0
    [] c = "single_item" -> 1
    [] c = "single_lane" -> 1
    [] c = "two_lanes_balanced" -> 2
    [] c = "two_lanes_skewed" -> 2
    [] c = "offset_rotates" -> 2
    [] c = "offset_wraps" -> 3
    [] c = "sorted_lanes_not_first_seen" -> 3
    [] c = "three_lanes_missing_later" -> 3
    [] OTHER -> 0

\* @type: Str => <<Int, Int, Int, Int, Int>>;
SpecOrder(c) ==
  CASE c = "empty" -> <<NoIndex, NoIndex, NoIndex, NoIndex, NoIndex>>
    [] c = "single_item" -> <<0, NoIndex, NoIndex, NoIndex, NoIndex>>
    [] c = "single_lane" -> <<0, 1, 2, NoIndex, NoIndex>>
    [] c = "two_lanes_balanced" -> <<0, 1, 2, 3, NoIndex>>
    [] c = "two_lanes_skewed" -> <<0, 3, 1, 4, 2>>
    [] c = "offset_rotates" -> <<1, 0, 3, 2, NoIndex>>
    [] c = "offset_wraps" -> <<2, 0, 1, 3, NoIndex>>
    [] c = "sorted_lanes_not_first_seen" -> <<1, 3, 0, 2, NoIndex>>
    [] c = "three_lanes_missing_later" -> <<0, 2, 3, 1, 4>>
    [] OTHER -> <<NoIndex, NoIndex, NoIndex, NoIndex, NoIndex>>

\* @type: Str => <<Int, Int, Int, Int, Int>>;
ActualOrder(c) ==
  CASE Bug = "empty_adds_index"
       /\ c = "empty" -> <<0, NoIndex, NoIndex, NoIndex, NoIndex>>
    [] Bug = "single_item_dropped"
       /\ c = "single_item" -> <<NoIndex, NoIndex, NoIndex, NoIndex, NoIndex>>
    [] Bug = "single_lane_reversed"
       /\ c = "single_lane" -> <<2, 1, 0, NoIndex, NoIndex>>
    [] Bug = "reverse_lane_order"
       /\ c = "two_lanes_balanced" -> <<1, 0, 3, 2, NoIndex>>
    [] Bug = "drain_lane_fully"
       /\ c = "two_lanes_skewed" -> <<0, 1, 2, 3, 4>>
    [] Bug = "drop_last_round"
       /\ c = "two_lanes_skewed" -> <<0, 3, 1, 4, NoIndex>>
    [] Bug = "unstable_intra_lane"
       /\ c = "two_lanes_skewed" -> <<1, 3, 0, 4, 2>>
    [] Bug = "ignore_slot_offset"
       /\ c = "offset_rotates" -> <<0, 1, 2, 3, NoIndex>>
    [] Bug = "offset_not_wrapped"
       /\ c = "offset_wraps" -> <<0, 1, 2, 3, NoIndex>>
    [] Bug = "first_seen_lane_order"
       /\ c = "sorted_lanes_not_first_seen" -> <<0, 1, 3, 2, NoIndex>>
    [] Bug = "index_order_only"
       /\ c = "three_lanes_missing_later" -> <<0, 1, 2, 3, 4>>
    [] OTHER -> SpecOrder(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "empty_adds_index",
       "single_item_dropped",
       "single_lane_reversed",
       "reverse_lane_order",
       "drain_lane_fully",
       "drop_last_round",
       "unstable_intra_lane",
       "ignore_slot_offset",
       "offset_not_wrapped",
       "first_seen_lane_order",
       "index_order_only"
     }
  /\ checked = 0

LaneInterleaveMatchesSpec ==
  /\ \A c \in Cases:
       ActualOrder(c) = SpecOrder(c)
  /\ \A c \in Cases:
       Total(c) \in 0..5
  /\ \A c \in Cases:
       LaneCount(c) \in 0..3

LaneInterleaveExactness ==
  /\ LaneInterleaveMatchesSpec
LaneInterleaveCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ LaneInterleaveExactness

SafetyFast ==
  LaneInterleaveExactness

BugEmptyAddsIndex ==
  ActualOrder("empty") = SpecOrder("empty")

BugSingleItemDropped ==
  ActualOrder("single_item") = SpecOrder("single_item")

BugSingleLaneReversed ==
  ActualOrder("single_lane") = SpecOrder("single_lane")

BugReverseLaneOrder ==
  ActualOrder("two_lanes_balanced") = SpecOrder("two_lanes_balanced")

BugDrainLaneFully ==
  ActualOrder("two_lanes_skewed") = SpecOrder("two_lanes_skewed")

BugDropLastRound ==
  ActualOrder("two_lanes_skewed") = SpecOrder("two_lanes_skewed")

BugUnstableIntraLane ==
  ActualOrder("two_lanes_skewed") = SpecOrder("two_lanes_skewed")

BugIgnoreSlotOffset ==
  ActualOrder("offset_rotates") = SpecOrder("offset_rotates")

BugOffsetNotWrapped ==
  ActualOrder("offset_wraps") = SpecOrder("offset_wraps")

BugFirstSeenLaneOrder ==
  ActualOrder("sorted_lanes_not_first_seen") =
    SpecOrder("sorted_lanes_not_first_seen")

BugIndexOrderOnly ==
  ActualOrder("three_lanes_missing_later") =
    SpecOrder("three_lanes_missing_later")

====
