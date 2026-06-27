---- MODULE SumeragiRbcChunkTargetGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `rbc_chunk_target_count(...)` and
`select_rbc_chunk_targets(...)`.

The helper pair determines how many non-local peers receive initial RBC chunks
and which peers are selected. Target counts must preserve a commit-quorum floor,
default to all non-local peers, clamp large caps to the available non-local
peers, and never select the local peer.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Int;
  target_count,
  \* @type: Int;
  selected_count,
  \* @type: Bool;
  local_selected

\* @type: <<Str, Int, Int, Bool>>;
vars == <<candidate, target_count, selected_count, local_selected>>

Cases == {
  "empty_roster",
  "local_only",
  "default_full_seven",
  "cap_below_quorum_seven",
  "cap_between_seven",
  "cap_above_peers_seven",
  "target_zero_with_peers",
  "selection_truncate_five",
  "selection_all_candidates"
}

CountValues == 0..32

RosterLen(c) ==
  CASE c = "empty_roster" -> 0
    [] c = "local_only" -> 1
    [] c \in {"target_zero_with_peers", "selection_truncate_five", "selection_all_candidates"} -> 5
    [] OTHER -> 7

Peers(c) ==
  CASE RosterLen(c) = 0 -> 0
    [] OTHER -> RosterLen(c) - 1

CommitQuorum(len) ==
  CASE len <= 3 -> len
    [] OTHER -> (2 * len) \div 3 + 1

Min(a, b) ==
  IF a <= b THEN a ELSE b

Max(a, b) ==
  IF a >= b THEN a ELSE b

FanoutCap(c) ==
  CASE c = "cap_below_quorum_seven" -> 1
    [] c = "cap_between_seven" -> 5
    [] c = "cap_above_peers_seven" -> 32
    [] OTHER -> 0

HasFanoutCap(c) ==
  c \in {"cap_below_quorum_seven", "cap_between_seven", "cap_above_peers_seven"}

DesiredTarget(c) ==
  IF HasFanoutCap(c)
  THEN Min(FanoutCap(c), Peers(c))
  ELSE Peers(c)

MinTargets(c) ==
  Min(CommitQuorum(RosterLen(c)), Peers(c))

SpecTargetCount(c) ==
  IF Peers(c) = 0
  THEN 0
  ELSE Max(DesiredTarget(c), MinTargets(c))

SelectionInput(c, count) ==
  CASE c = "target_zero_with_peers" -> 0
    [] c = "selection_truncate_five" -> 2
    [] c = "selection_all_candidates" -> 10
    [] OTHER -> count

CandidateCount(c) ==
  Peers(c)

SpecSelectedCount(c, count) ==
  LET input == SelectionInput(c, count) IN
    IF input = 0
    THEN 0
    ELSE Min(input, CandidateCount(c))

SpecLocalSelected(c, count) ==
  FALSE

ActualTargetCount(c) ==
  CASE Bug = "ignore_min_quorum" ->
         IF Peers(c) = 0 THEN 0 ELSE DesiredTarget(c)
    [] Bug = "default_uses_min_targets" ->
         IF Peers(c) = 0
         THEN 0
         ELSE IF ~HasFanoutCap(c) THEN MinTargets(c) ELSE Max(DesiredTarget(c), MinTargets(c))
    [] Bug = "cap_not_clamped_to_peers" ->
         IF Peers(c) = 0
         THEN 0
         ELSE IF ~HasFanoutCap(c) THEN Peers(c) ELSE Max(FanoutCap(c), MinTargets(c))
    [] Bug = "zero_peer_returns_one" ->
         IF Peers(c) = 0 THEN 1 ELSE SpecTargetCount(c)
    [] OTHER -> SpecTargetCount(c)

ActualSelectedCount(c, count) ==
  LET input == SelectionInput(c, count) IN
    CASE Bug = "selection_ignores_zero_target" /\ input = 0 -> CandidateCount(c)
      [] Bug = "selection_ignores_target_count" /\ CandidateCount(c) # 0 -> CandidateCount(c)
      [] Bug = "selection_drops_when_target_ge_candidates" /\ input >= CandidateCount(c) -> input
      [] Bug = "selection_returns_empty" /\ CandidateCount(c) # 0 /\ input # 0 -> 0
      [] Bug = "selection_uses_roster_len_instead_of_candidates" /\ input # 0 ->
           Min(input, RosterLen(c))
      [] OTHER -> SpecSelectedCount(c, count)

ActualLocalSelected(c, count) ==
  Bug = "selection_includes_local" /\ SelectionInput(c, count) # 0 /\ RosterLen(c) # 0

TypeInvariant ==
  /\ Bug \in {
       "none",
       "ignore_min_quorum",
       "default_uses_min_targets",
       "cap_not_clamped_to_peers",
       "zero_peer_returns_one",
       "selection_ignores_zero_target",
       "selection_includes_local",
       "selection_ignores_target_count",
       "selection_drops_when_target_ge_candidates",
       "selection_returns_empty",
       "selection_uses_roster_len_instead_of_candidates"
     }
  /\ candidate \in Cases
  /\ target_count \in CountValues
  /\ selected_count \in CountValues
  /\ local_selected \in BOOLEAN

Init ==
  /\ candidate \in Cases
  /\ target_count = ActualTargetCount(candidate)
  /\ selected_count = ActualSelectedCount(candidate, target_count)
  /\ local_selected = ActualLocalSelected(candidate, target_count)

Next ==
  UNCHANGED vars

TargetCountMatchesSpec ==
  target_count = SpecTargetCount(candidate)

SelectionMatchesSpec ==
  selected_count = SpecSelectedCount(candidate, target_count)

NoPeersTargetZero ==
  candidate \in {"empty_roster", "local_only"} => target_count = 0

DefaultTargetsAllPeers ==
  candidate = "default_full_seven" => target_count = Peers(candidate)

CapBelowQuorumKeepsMinTargets ==
  candidate = "cap_below_quorum_seven" => target_count = MinTargets(candidate)

CapAbovePeersClamped ==
  candidate = "cap_above_peers_seven" => target_count = Peers(candidate)

TargetNeverExceedsPeers ==
  target_count <= Peers(candidate)

ZeroSelectionTargetSelectsNone ==
  candidate = "target_zero_with_peers" => selected_count = 0

SelectionExcludesLocal ==
  ~local_selected

SelectionTruncatesToTarget ==
  candidate = "selection_truncate_five" => selected_count = 2

SelectionAllCandidatesWhenTargetLarge ==
  candidate = "selection_all_candidates" => selected_count = CandidateCount(candidate)

SelectionNeverExceedsCandidates ==
  selected_count <= CandidateCount(candidate)

PositiveSelectionRequiresCandidatesAndInput ==
  selected_count # 0 =>
    /\ CandidateCount(candidate) # 0
    /\ SelectionInput(candidate, target_count) # 0

RbcChunkTargetExactness ==
  /\ TargetCountMatchesSpec
  /\ SelectionMatchesSpec
  /\ NoPeersTargetZero
  /\ DefaultTargetsAllPeers
  /\ CapBelowQuorumKeepsMinTargets
  /\ CapAbovePeersClamped
  /\ TargetNeverExceedsPeers
  /\ ZeroSelectionTargetSelectsNone
  /\ SelectionExcludesLocal
  /\ SelectionTruncatesToTarget
  /\ SelectionAllCandidatesWhenTargetLarge
  /\ SelectionNeverExceedsCandidates
  /\ PositiveSelectionRequiresCandidatesAndInput

RbcChunkTargetCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ RbcChunkTargetExactness

Safety == RbcChunkTargetExactness

====
