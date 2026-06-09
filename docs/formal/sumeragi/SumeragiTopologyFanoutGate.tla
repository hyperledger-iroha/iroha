---- MODULE SumeragiTopologyFanoutGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for topology fanout and redundant-send helpers.

This slice covers `topology_fanout_from_tail(...)`,
`redundant_send_r_from_len(...)`, `redundant_send_r_floor(...)`, and
`min_votes_for_view_change()`. It abstracts peer ids into numeric indices and
keeps the helper contracts finite over representative small and saturating
topology sizes.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

FanoutCases == {
  "single_count_positive",
  "len4_count_zero",
  "len4_count_two",
  "len4_count_three",
  "len4_count_ten",
  "len7_count_two",
  "len7_count_five",
  "len7_count_ten"
}

RedundantCases == {
  "redundant_len_zero",
  "redundant_len_one",
  "redundant_len_three",
  "redundant_len_four",
  "redundant_len_five",
  "redundant_len_seven",
  "redundant_len_ten",
  "redundant_len_huge"
}

ViewCases == {
  "view_len_one",
  "view_len_three",
  "view_len_four",
  "view_len_seven",
  "view_len_ten",
  "view_len_sixteen"
}

FloorCases == {
  "floor_len_one_cfg_zero",
  "floor_len_five_cfg_two",
  "floor_len_five_cfg_six",
  "floor_len_huge_cfg_one"
}

Min(a, b) == IF a <= b THEN a ELSE b
Max(a, b) == IF a >= b THEN a ELSE b

CommitQuorum(n) ==
  IF n <= 3 THEN n ELSE (n * 2) \div 3 + 1

ProxyTail(n) ==
  IF CommitQuorum(n) = 0 THEN 0 ELSE CommitQuorum(n) - 1

MaxFaults(n) ==
  IF n = 0 THEN 0 ELSE (n - 1) \div 3

RedundantLen(c) ==
  CASE c = "redundant_len_zero" -> 0
    [] c = "redundant_len_one" -> 1
    [] c = "redundant_len_three" -> 3
    [] c = "redundant_len_four" -> 4
    [] c = "redundant_len_five" -> 5
    [] c = "redundant_len_seven" -> 7
    [] c = "redundant_len_ten" -> 10
    [] c = "redundant_len_huge" -> 1020
    [] OTHER -> 1

SpecRedundantSendR(c) ==
  LET n == Max(RedundantLen(c), 1) IN
  LET r == (MaxFaults(n) * 2) + 1 IN
  Min(r, 255)

ViewLen(c) ==
  CASE c = "view_len_one" -> 1
    [] c = "view_len_three" -> 3
    [] c = "view_len_four" -> 4
    [] c = "view_len_seven" -> 7
    [] c = "view_len_ten" -> 10
    [] c = "view_len_sixteen" -> 16
    [] OTHER -> 1

SpecViewChangeQuorum(c) ==
  LET n == ViewLen(c) IN
  Min(MaxFaults(n) + 1, Max(n, 1))

FloorLen(c) ==
  CASE c = "floor_len_one_cfg_zero" -> 1
    [] c \in {"floor_len_five_cfg_two", "floor_len_five_cfg_six"} -> 5
    [] c = "floor_len_huge_cfg_one" -> 400
    [] OTHER -> 1

FloorConfigured(c) ==
  CASE c = "floor_len_five_cfg_six" -> 6
    [] OTHER -> 0

SpecRedundantFloor(c) ==
  Max(Max(FloorConfigured(c), Min(CommitQuorum(FloorLen(c)), 255)), 1)

FanoutLen(c) ==
  CASE c = "single_count_positive" -> 1
    [] c \in {"len7_count_two", "len7_count_five", "len7_count_ten"} -> 7
    [] OTHER -> 4

FanoutCount(c) ==
  CASE c \in {"len4_count_zero"} -> 0
    [] c \in {"len4_count_two", "len7_count_two"} -> 2
    [] c = "len4_count_three" -> 3
    [] c = "len7_count_five" -> 5
    [] OTHER -> 10

SpecFanoutLen(c) ==
  LET n == FanoutLen(c) IN
  LET count == FanoutCount(c) IN
  IF n <= 1 \/ count = 0 THEN 0 ELSE Min(count, n - 1)

SpecFanoutFirst(c) ==
  IF SpecFanoutLen(c) = 0 THEN 0 ELSE ProxyTail(FanoutLen(c))

SpecFanoutSecond(c) ==
  CASE SpecFanoutLen(c) <= 1 -> 0
    [] FanoutLen(c) = 4 -> 3
    [] FanoutLen(c) = 7 -> 5
    [] OTHER -> 0

SpecFanoutLast(c) ==
  CASE SpecFanoutLen(c) = 0 -> 0
    [] FanoutLen(c) = 4 /\ SpecFanoutLen(c) = 2 -> 3
    [] FanoutLen(c) = 4 /\ SpecFanoutLen(c) = 3 -> 1
    [] FanoutLen(c) = 7 /\ SpecFanoutLen(c) = 2 -> 5
    [] FanoutLen(c) = 7 /\ SpecFanoutLen(c) = 5 -> 2
    [] FanoutLen(c) = 7 /\ SpecFanoutLen(c) = 6 -> 3
    [] OTHER -> 0

SpecFanoutWraps(c) ==
  FanoutLen(c) > 1 /\ SpecFanoutLen(c) > 0
    /\ ProxyTail(FanoutLen(c)) + SpecFanoutLen(c) > FanoutLen(c)

SpecFanoutHasLeader(c) == FALSE

SpecFanoutDistinct(c) == TRUE

SpecFanoutOutput(c) ==
  <<FanoutLen(c), FanoutCount(c), CommitQuorum(FanoutLen(c)),
    ProxyTail(FanoutLen(c)), SpecFanoutLen(c), SpecFanoutFirst(c),
    SpecFanoutSecond(c), SpecFanoutLast(c), SpecFanoutWraps(c),
    SpecFanoutHasLeader(c), SpecFanoutDistinct(c)>>

ActualRedundantSendR(c) ==
  CASE Bug = "redundant_zero_len_returns_zero"
       /\ c = "redundant_len_zero" -> 0
    [] Bug = "redundant_uses_commit_quorum"
       /\ c = "redundant_len_five" -> CommitQuorum(5)
    [] Bug = "redundant_uses_two_f"
       /\ c = "redundant_len_seven" -> 4
    [] Bug = "redundant_no_u8_clamp"
       /\ c = "redundant_len_huge" -> 679
    [] OTHER -> SpecRedundantSendR(c)

ActualViewChangeQuorum(c) ==
  CASE Bug = "view_change_zero_for_single" /\ c = "view_len_one" -> 0
    [] Bug = "view_change_uses_commit_quorum" /\ c = "view_len_seven" ->
         CommitQuorum(7)
    [] Bug = "view_change_over_roster" /\ c = "view_len_one" -> 2
    [] OTHER -> SpecViewChangeQuorum(c)

ActualRedundantFloor(c) ==
  CASE Bug = "floor_ignores_quorum" /\ c = "floor_len_five_cfg_two" ->
         FloorConfigured(c)
    [] Bug = "floor_drops_config" /\ c = "floor_len_five_cfg_six" ->
         CommitQuorum(FloorLen(c))
    [] Bug = "floor_zero_config_zero" /\ c = "floor_len_one_cfg_zero" -> 0
    [] Bug = "floor_no_u8_clamp" /\ c = "floor_len_huge_cfg_one" ->
         CommitQuorum(FloorLen(c))
    [] OTHER -> SpecRedundantFloor(c)

ActualFanoutLen(c) ==
  CASE Bug = "fanout_single_returns_local"
       /\ c = "single_count_positive" -> 1
    [] Bug = "fanout_zero_count_selects_tail"
       /\ c = "len4_count_zero" -> 1
    [] Bug = "fanout_no_cap_overselects"
       /\ c = "len4_count_ten" -> 4
    [] Bug = "fanout_no_wrap"
       /\ c = "len4_count_three" -> 2
    [] OTHER -> SpecFanoutLen(c)

ActualFanoutFirst(c) ==
  CASE Bug = "fanout_single_returns_local"
       /\ c = "single_count_positive" -> 0
    [] Bug = "fanout_wrong_start" /\ c = "len4_count_two" -> 3
    [] Bug = "fanout_includes_leader" /\ c = "len4_count_three" -> 0
    [] OTHER -> SpecFanoutFirst(c)

ActualFanoutSecond(c) ==
  CASE Bug = "fanout_duplicates" /\ c = "len4_count_three" ->
         ActualFanoutFirst(c)
    [] Bug = "fanout_includes_leader" /\ c = "len4_count_three" -> 2
    [] OTHER -> SpecFanoutSecond(c)

ActualFanoutLast(c) ==
  CASE Bug = "fanout_no_cap_overselects" /\ c = "len4_count_ten" -> 0
    [] Bug = "fanout_no_wrap" /\ c = "len4_count_three" -> 3
    [] Bug = "fanout_includes_leader" /\ c = "len4_count_three" -> 3
    [] OTHER -> SpecFanoutLast(c)

ActualFanoutWraps(c) ==
  CASE Bug = "fanout_no_wrap" /\ c = "len4_count_three" -> FALSE
    [] OTHER -> SpecFanoutWraps(c)

ActualFanoutHasLeader(c) ==
  CASE Bug = "fanout_includes_leader" /\ c = "len4_count_three" -> TRUE
    [] Bug = "fanout_no_cap_overselects" /\ c = "len4_count_ten" -> TRUE
    [] OTHER -> FALSE

ActualFanoutDistinct(c) ==
  CASE Bug = "fanout_duplicates" /\ c = "len4_count_three" -> FALSE
    [] OTHER -> TRUE

ActualFanoutOutput(c) ==
  <<FanoutLen(c), FanoutCount(c), CommitQuorum(FanoutLen(c)),
    ProxyTail(FanoutLen(c)), ActualFanoutLen(c), ActualFanoutFirst(c),
    ActualFanoutSecond(c), ActualFanoutLast(c), ActualFanoutWraps(c),
    ActualFanoutHasLeader(c), ActualFanoutDistinct(c)>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "redundant_zero_len_returns_zero",
       "redundant_uses_commit_quorum",
       "redundant_uses_two_f",
       "redundant_no_u8_clamp",
       "view_change_zero_for_single",
       "view_change_uses_commit_quorum",
       "view_change_over_roster",
       "floor_ignores_quorum",
       "floor_drops_config",
       "floor_zero_config_zero",
       "floor_no_u8_clamp",
       "fanout_single_returns_local",
       "fanout_zero_count_selects_tail",
       "fanout_no_cap_overselects",
       "fanout_no_wrap",
       "fanout_wrong_start",
       "fanout_includes_leader",
       "fanout_duplicates"
     }
  /\ checked = 0

TopologyFanoutCoreSafety ==
  /\ ActualRedundantSendR("redundant_len_zero") =
       SpecRedundantSendR("redundant_len_zero")
  /\ ActualRedundantSendR("redundant_len_one") =
       SpecRedundantSendR("redundant_len_one")
  /\ ActualRedundantSendR("redundant_len_three") =
       SpecRedundantSendR("redundant_len_three")
  /\ ActualRedundantSendR("redundant_len_four") =
       SpecRedundantSendR("redundant_len_four")
  /\ ActualRedundantSendR("redundant_len_five") =
       SpecRedundantSendR("redundant_len_five")
  /\ ActualRedundantSendR("redundant_len_seven") =
       SpecRedundantSendR("redundant_len_seven")
  /\ ActualRedundantSendR("redundant_len_ten") =
       SpecRedundantSendR("redundant_len_ten")
  /\ ActualRedundantSendR("redundant_len_huge") =
       SpecRedundantSendR("redundant_len_huge")
  /\ ActualViewChangeQuorum("view_len_one") =
       SpecViewChangeQuorum("view_len_one")
  /\ ActualViewChangeQuorum("view_len_three") =
       SpecViewChangeQuorum("view_len_three")
  /\ ActualViewChangeQuorum("view_len_four") =
       SpecViewChangeQuorum("view_len_four")
  /\ ActualViewChangeQuorum("view_len_seven") =
       SpecViewChangeQuorum("view_len_seven")
  /\ ActualViewChangeQuorum("view_len_ten") =
       SpecViewChangeQuorum("view_len_ten")
  /\ ActualViewChangeQuorum("view_len_sixteen") =
       SpecViewChangeQuorum("view_len_sixteen")
  /\ ActualRedundantFloor("floor_len_one_cfg_zero") =
       SpecRedundantFloor("floor_len_one_cfg_zero")
  /\ ActualRedundantFloor("floor_len_five_cfg_two") =
       SpecRedundantFloor("floor_len_five_cfg_two")
  /\ ActualRedundantFloor("floor_len_five_cfg_six") =
       SpecRedundantFloor("floor_len_five_cfg_six")
  /\ ActualRedundantFloor("floor_len_huge_cfg_one") =
       SpecRedundantFloor("floor_len_huge_cfg_one")
  /\ ActualFanoutOutput("single_count_positive") =
       SpecFanoutOutput("single_count_positive")
  /\ ActualFanoutOutput("len4_count_zero") =
       SpecFanoutOutput("len4_count_zero")
  /\ ActualFanoutOutput("len4_count_two") =
       SpecFanoutOutput("len4_count_two")
  /\ ActualFanoutOutput("len4_count_three") =
       SpecFanoutOutput("len4_count_three")
  /\ ActualFanoutOutput("len4_count_ten") =
       SpecFanoutOutput("len4_count_ten")
  /\ ActualFanoutOutput("len7_count_two") =
       SpecFanoutOutput("len7_count_two")
  /\ ActualFanoutOutput("len7_count_five") =
       SpecFanoutOutput("len7_count_five")
  /\ ActualFanoutOutput("len7_count_ten") =
       SpecFanoutOutput("len7_count_ten")

SafetyFast ==
  TopologyFanoutCoreSafety

RedundantSendCountExact ==
  \A c \in RedundantCases:
    ActualRedundantSendR(c) = SpecRedundantSendR(c)

ViewChangeQuorumExact ==
  \A c \in ViewCases:
    ActualViewChangeQuorum(c) = SpecViewChangeQuorum(c)

RedundantSendFloorExact ==
  \A c \in FloorCases:
    ActualRedundantFloor(c) = SpecRedundantFloor(c)

TailFanoutSelectionExact ==
  \A c \in FanoutCases:
    ActualFanoutOutput(c) = SpecFanoutOutput(c)

TopologyFanoutHelperExactness ==
  /\ TopologyFanoutCoreSafety
  /\ RedundantSendCountExact
  /\ ViewChangeQuorumExact
  /\ RedundantSendFloorExact
  /\ TailFanoutSelectionExact

BugRedundantZeroLenReturnsZero ==
  ActualRedundantSendR("redundant_len_zero") =
    SpecRedundantSendR("redundant_len_zero")

BugRedundantUsesCommitQuorum ==
  ActualRedundantSendR("redundant_len_five") =
    SpecRedundantSendR("redundant_len_five")

BugRedundantUsesTwoF ==
  ActualRedundantSendR("redundant_len_seven") =
    SpecRedundantSendR("redundant_len_seven")

BugRedundantNoU8Clamp ==
  ActualRedundantSendR("redundant_len_huge") =
    SpecRedundantSendR("redundant_len_huge")

BugViewChangeZeroForSingle ==
  ActualViewChangeQuorum("view_len_one") =
    SpecViewChangeQuorum("view_len_one")

BugViewChangeUsesCommitQuorum ==
  ActualViewChangeQuorum("view_len_seven") =
    SpecViewChangeQuorum("view_len_seven")

BugViewChangeOverRoster ==
  ActualViewChangeQuorum("view_len_one") =
    SpecViewChangeQuorum("view_len_one")

BugFloorIgnoresQuorum ==
  ActualRedundantFloor("floor_len_five_cfg_two") =
    SpecRedundantFloor("floor_len_five_cfg_two")

BugFloorDropsConfig ==
  ActualRedundantFloor("floor_len_five_cfg_six") =
    SpecRedundantFloor("floor_len_five_cfg_six")

BugFloorZeroConfigZero ==
  ActualRedundantFloor("floor_len_one_cfg_zero") =
    SpecRedundantFloor("floor_len_one_cfg_zero")

BugFloorNoU8Clamp ==
  ActualRedundantFloor("floor_len_huge_cfg_one") =
    SpecRedundantFloor("floor_len_huge_cfg_one")

BugFanoutSingleReturnsLocal ==
  ActualFanoutLen("single_count_positive") =
    SpecFanoutLen("single_count_positive")

BugFanoutZeroCountSelectsTail ==
  ActualFanoutLen("len4_count_zero") = SpecFanoutLen("len4_count_zero")

BugFanoutNoCapOverselects ==
  ActualFanoutLen("len4_count_ten") = SpecFanoutLen("len4_count_ten")

BugFanoutNoWrap ==
  ActualFanoutWraps("len4_count_three") = SpecFanoutWraps("len4_count_three")

BugFanoutWrongStart ==
  ActualFanoutFirst("len4_count_two") = SpecFanoutFirst("len4_count_two")

BugFanoutIncludesLeader ==
  ActualFanoutHasLeader("len4_count_three") =
    SpecFanoutHasLeader("len4_count_three")

BugFanoutDuplicates ==
  ActualFanoutDistinct("len4_count_three") =
    SpecFanoutDistinct("len4_count_three")

Safety ==
  TopologyFanoutCoreSafety

=============================================================================
====
