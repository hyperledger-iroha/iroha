---- MODULE SumeragiRbcRebroadcastSelectionGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the RBC rebroadcast helper family:
`rbc_rebroadcasters_count(...)`, `rbc_ready_rebroadcasters_count(...)`, and
`rbc_rebroadcast_indices_with_count(...)`.

The concrete implementation shuffles non-leader candidates from a deterministic
seed. This model abstracts away the shuffle order and proves the stable safety
contract: count formulas, roster bounds, zero-count behavior, leader inclusion
for partial selections, all-roster selection when the count covers the roster,
and fail-closed absent-local membership checks.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Int;
  payload_count,
  \* @type: Int;
  ready_count,
  \* @type: Int;
  selected_count,
  \* @type: Bool;
  leader_selected,
  \* @type: Bool;
  all_roster_selected,
  \* @type: Bool;
  missing_local_selected

\* @type: <<Str, Int, Int, Int, Bool, Bool, Bool>>;
vars ==
  <<candidate, payload_count, ready_count, selected_count,
    leader_selected, all_roster_selected, missing_local_selected>>

Cases == {
  "empty_roster",
  "one_roster",
  "two_roster",
  "three_roster",
  "four_roster",
  "seven_roster",
  "count_zero_selection",
  "count_all_selection",
  "count_partial_selection",
  "missing_local_query"
}

CountValues == 0..32

RosterLen(c) ==
  CASE c = "empty_roster" -> 0
    [] c = "one_roster" -> 1
    [] c = "two_roster" -> 2
    [] c = "three_roster" -> 3
    [] c = "four_roster" -> 4
    [] c = "seven_roster" -> 7
    [] c \in {
         "count_zero_selection",
         "count_all_selection",
         "count_partial_selection",
         "missing_local_query"
       } -> 5

Min(a, b) ==
  IF a <= b THEN a ELSE b

CommitQuorum(len) ==
  CASE len <= 3 -> len
    [] OTHER -> (2 * len) \div 3 + 1

PayloadSpecCount(c) ==
  LET roster_len == RosterLen(c) IN
    IF roster_len = 0
    THEN 0
    ELSE Min(((roster_len - 1) \div 3) + 1, roster_len)

ReadySpecCount(c) ==
  LET roster_len == RosterLen(c) IN
    IF roster_len = 0
    THEN 0
    ELSE Min(CommitQuorum(roster_len), roster_len)

SelectionInput(c) ==
  CASE c = "count_zero_selection" -> 0
    [] c = "count_all_selection" -> 9
    [] c = "count_partial_selection" -> 3
    [] c = "missing_local_query" -> 3
    [] OTHER -> PayloadSpecCount(c)

SpecSelectedCount(c) ==
  LET roster_len == RosterLen(c) IN
  LET input == SelectionInput(c) IN
    IF input = 0 \/ roster_len = 0
    THEN 0
    ELSE IF input >= roster_len THEN roster_len ELSE input

SpecLeaderSelected(c) ==
  RosterLen(c) # 0 /\ SelectionInput(c) # 0

SpecAllRosterSelected(c) ==
  RosterLen(c) # 0 /\ SelectionInput(c) >= RosterLen(c)

SpecMissingLocalSelected(c) ==
  FALSE

ActualPayloadCount(c) ==
  LET roster_len == RosterLen(c) IN
    CASE Bug = "payload_count_zero_returns_one" /\ roster_len = 0 -> 1
      [] Bug = "payload_count_uses_ready_quorum" -> ReadySpecCount(c)
      [] Bug = "payload_count_drops_minimum" /\ roster_len # 0 ->
           (roster_len - 1) \div 3
      [] OTHER -> PayloadSpecCount(c)

ActualReadyCount(c) ==
  LET roster_len == RosterLen(c) IN
    CASE Bug = "ready_count_zero_returns_one" /\ roster_len = 0 -> 1
      [] Bug = "ready_count_uses_payload_formula" -> PayloadSpecCount(c)
      [] Bug = "ready_count_all_but_one" /\ roster_len # 0 ->
           roster_len - 1
      [] OTHER -> ReadySpecCount(c)

ActualSelectedCount(c) ==
  LET roster_len == RosterLen(c) IN
  LET input == SelectionInput(c) IN
    CASE Bug = "selection_ignores_zero_count" /\ input = 0 -> roster_len
      [] Bug = "selection_drops_full_roster" /\ input >= roster_len /\ roster_len # 0 ->
           input
      [] Bug = "selection_overselects_partial" /\ input # 0 /\ input < roster_len ->
           input + 1
      [] OTHER -> SpecSelectedCount(c)

ActualLeaderSelected(c) ==
  LET roster_len == RosterLen(c) IN
  LET input == SelectionInput(c) IN
    CASE Bug = "selection_omits_leader" /\ input # 0 /\ roster_len # 0 -> FALSE
      [] OTHER -> SpecLeaderSelected(c)

ActualAllRosterSelected(c) ==
  LET roster_len == RosterLen(c) IN
  LET input == SelectionInput(c) IN
    CASE Bug = "selection_drops_full_roster" /\ input >= roster_len /\ roster_len # 0 ->
           FALSE
      [] OTHER -> SpecAllRosterSelected(c)

ActualMissingLocalSelected(c) ==
  Bug = "selection_selects_missing_local" /\ candidate = "missing_local_query"

TypeInvariant ==
  /\ Bug \in {
       "none",
       "payload_count_zero_returns_one",
       "payload_count_uses_ready_quorum",
       "payload_count_drops_minimum",
       "ready_count_zero_returns_one",
       "ready_count_uses_payload_formula",
       "ready_count_all_but_one",
       "selection_ignores_zero_count",
       "selection_omits_leader",
       "selection_drops_full_roster",
       "selection_overselects_partial",
       "selection_selects_missing_local"
     }
  /\ candidate \in Cases
  /\ payload_count \in CountValues
  /\ ready_count \in CountValues
  /\ selected_count \in CountValues
  /\ leader_selected \in BOOLEAN
  /\ all_roster_selected \in BOOLEAN
  /\ missing_local_selected \in BOOLEAN

Init ==
  /\ candidate \in Cases
  /\ payload_count = ActualPayloadCount(candidate)
  /\ ready_count = ActualReadyCount(candidate)
  /\ selected_count = ActualSelectedCount(candidate)
  /\ leader_selected = ActualLeaderSelected(candidate)
  /\ all_roster_selected = ActualAllRosterSelected(candidate)
  /\ missing_local_selected = ActualMissingLocalSelected(candidate)

Next ==
  UNCHANGED vars

PayloadCountMatchesSpec ==
  payload_count = PayloadSpecCount(candidate)

ReadyCountMatchesSpec ==
  ready_count = ReadySpecCount(candidate)

EmptyRosterCountsZero ==
  candidate = "empty_roster" => payload_count = 0 /\ ready_count = 0

PayloadCountTracksFaultTolerance ==
  /\ candidate = "one_roster" => payload_count = 1
  /\ candidate = "two_roster" => payload_count = 1
  /\ candidate = "three_roster" => payload_count = 1
  /\ candidate = "four_roster" => payload_count = 2
  /\ candidate = "seven_roster" => payload_count = 3

ReadyCountTracksCommitQuorum ==
  /\ candidate = "one_roster" => ready_count = 1
  /\ candidate = "two_roster" => ready_count = 2
  /\ candidate = "three_roster" => ready_count = 3
  /\ candidate = "four_roster" => ready_count = 3
  /\ candidate = "seven_roster" => ready_count = 5

CountsNeverExceedRoster ==
  /\ payload_count <= RosterLen(candidate)
  /\ ready_count <= RosterLen(candidate)

SelectionMatchesSpec ==
  /\ selected_count = SpecSelectedCount(candidate)
  /\ leader_selected = SpecLeaderSelected(candidate)
  /\ all_roster_selected = SpecAllRosterSelected(candidate)
  /\ missing_local_selected = SpecMissingLocalSelected(candidate)

ZeroSelectionSelectsNone ==
  candidate = "count_zero_selection" => selected_count = 0

PositiveSelectionIncludesLeader ==
  SelectionInput(candidate) # 0 /\ RosterLen(candidate) # 0 => leader_selected

FullCountSelectsWholeRoster ==
  candidate = "count_all_selection" =>
    /\ selected_count = RosterLen(candidate)
    /\ all_roster_selected

PartialSelectionKeepsRequestedCount ==
  candidate = "count_partial_selection" =>
    /\ selected_count = SelectionInput(candidate)
    /\ ~all_roster_selected

SelectionNeverExceedsRoster ==
  selected_count <= RosterLen(candidate)

MissingLocalQueryReturnsFalse ==
  candidate = "missing_local_query" => ~missing_local_selected

RbcRebroadcastSelectionCoreSafety ==
  /\ PayloadCountMatchesSpec
  /\ ReadyCountMatchesSpec
  /\ EmptyRosterCountsZero
  /\ PayloadCountTracksFaultTolerance
  /\ ReadyCountTracksCommitQuorum
  /\ CountsNeverExceedRoster
  /\ SelectionMatchesSpec
  /\ ZeroSelectionSelectsNone
  /\ PositiveSelectionIncludesLeader
  /\ FullCountSelectsWholeRoster
  /\ PartialSelectionKeepsRequestedCount
  /\ SelectionNeverExceedsRoster
  /\ MissingLocalQueryReturnsFalse

RbcRebroadcastSelectionExactness == RbcRebroadcastSelectionCoreSafety

RbcRebroadcastSelectionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcRebroadcastSelectionExactness

Safety == RbcRebroadcastSelectionExactness

====
