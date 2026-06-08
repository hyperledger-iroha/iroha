---- MODULE SumeragiTopologyMutationGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for Sumeragi topology mutation helpers.

This slice pins the ordered-roster semantics of:
`Topology::new(...)`,
`rotate_preserve_view_to_front(...)`,
`update_peer_list(...)`,
`block_committed(...)`,
`canonicalize_order(...)`, and
`nth_rotation(...)`.

Peer ids are abstracted into small integers. The model records the resulting
length, first four peer positions, view-change index, returned rotation count,
and whether the bounded output is duplicate-free.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoPeer == -1

Mod(a, b) == a - (b * (a \div b))

\* @type: (Bool, Int, Int, Int, Int, Int, Int, Int, Bool) => <<Bool, Int, Int, Int, Int, Int, Int, Int, Bool>>;
Out(allowed, len, p1, p2, p3, p4, view, rotations, distinct) ==
  <<allowed, len, p1, p2, p3, p4, view, rotations, distinct>>

RotateCases == {
  "rotate_empty",
  "rotate_single_idx99",
  "rotate_len4_idx0",
  "rotate_len4_idx2",
  "rotate_len4_idx6"
}

NthCases == {
  "nth_same",
  "nth_forward_one",
  "nth_forward_three",
  "nth_full_cycle",
  "nth_large_mod",
  "nth_single_large",
  "nth_empty_forward",
  "nth_rewind"
}

NewCases == {
  "new_dedup_preserve",
  "new_all_duplicates",
  "new_single"
}

UpdateCases == {
  "update_mixed",
  "update_keep_all_reordered_input",
  "update_remove_all_add_two",
  "update_duplicates"
}

BlockCases == {
  "block_mixed",
  "block_keep_all_reordered_input",
  "block_remove_all_add_two"
}

CanonCases == {
  "canon_reverse",
  "canon_duplicates",
  "canon_empty"
}

Cases ==
  RotateCases \cup NthCases \cup NewCases \cup UpdateCases \cup BlockCases
    \cup CanonCases

PadP1(len, p1) == IF len >= 1 THEN p1 ELSE NoPeer
PadP2(len, p2) == IF len >= 2 THEN p2 ELSE NoPeer
PadP3(len, p3) == IF len >= 3 THEN p3 ELSE NoPeer
PadP4(len, p4) == IF len >= 4 THEN p4 ELSE NoPeer

RotateLen(c) ==
  CASE c = "rotate_empty" -> 0
    [] c = "rotate_single_idx99" -> 1
    [] OTHER -> 4

RotateIdx(c) ==
  CASE c = "rotate_single_idx99" -> 99
    [] c = "rotate_len4_idx2" -> 2
    [] c = "rotate_len4_idx6" -> 6
    [] OTHER -> 0

RotateView(c) == 7

RotateRem(c) ==
  IF RotateLen(c) = 0 THEN 0 ELSE Mod(RotateIdx(c), RotateLen(c))

RotatePos(c, pos) ==
  LET n == RotateLen(c) IN
  IF n = 0 \/ pos > n THEN NoPeer ELSE Mod(RotateRem(c) + pos - 1, n)

SpecRotateOutput(c) ==
  IF c = "rotate_empty" THEN
    Out(TRUE, 0, NoPeer, NoPeer, NoPeer, NoPeer, RotateView(c), 0, TRUE)
  ELSE
    Out(TRUE, RotateLen(c), RotatePos(c, 1), RotatePos(c, 2),
      RotatePos(c, 3), RotatePos(c, 4), RotateView(c), 0, TRUE)

ActualRotateOutput(c) ==
  CASE Bug = "rotate_ignores_modulo" /\ c = "rotate_len4_idx6" ->
         Out(TRUE, 4, 0, 1, 2, 3, RotateView(c), 0, TRUE)
    [] Bug = "rotate_off_by_one" /\ c = "rotate_len4_idx2" ->
         Out(TRUE, 4, 1, 2, 3, 0, RotateView(c), 0, TRUE)
    [] Bug = "rotate_resets_view" /\ c = "rotate_len4_idx2" ->
         Out(TRUE, 4, 2, 3, 0, 1, 0, 0, TRUE)
    [] Bug = "rotate_empty_mutates" /\ c = "rotate_empty" ->
         Out(TRUE, 1, 0, NoPeer, NoPeer, NoPeer, RotateView(c), 0, TRUE)
    [] OTHER -> SpecRotateOutput(c)

NthLen(c) ==
  CASE c = "nth_single_large" -> 1
    [] c = "nth_empty_forward" -> 0
    [] OTHER -> 4

NthCurrentView(c) ==
  CASE c = "nth_same" -> 2
    [] c = "nth_forward_three" -> 1
    [] c = "nth_rewind" -> 4
    [] OTHER -> 0

NthTargetView(c) ==
  CASE c = "nth_same" -> 2
    [] c = "nth_forward_one" -> 1
    [] c = "nth_forward_three" -> 4
    [] c = "nth_full_cycle" -> 4
    [] c = "nth_large_mod" -> 10
    [] c = "nth_single_large" -> 10
    [] c = "nth_empty_forward" -> 5
    [] c = "nth_rewind" -> 2
    [] OTHER -> 0

NthAllowed(c) == NthTargetView(c) >= NthCurrentView(c)

NthDelta(c) ==
  IF NthAllowed(c) THEN NthTargetView(c) - NthCurrentView(c) ELSE 0

NthRem(c) ==
  IF NthLen(c) = 0 \/ ~NthAllowed(c) THEN 0 ELSE Mod(NthDelta(c), NthLen(c))

NthPos(c, pos) ==
  LET n == NthLen(c) IN
  IF n = 0 \/ pos > n THEN NoPeer ELSE Mod(NthRem(c) + pos - 1, n)

SpecNthOutput(c) ==
  IF NthAllowed(c) THEN
    Out(TRUE, NthLen(c), NthPos(c, 1), NthPos(c, 2), NthPos(c, 3),
      NthPos(c, 4), NthTargetView(c), NthDelta(c), TRUE)
  ELSE
    Out(FALSE, NthLen(c), NthPos(c, 1), NthPos(c, 2), NthPos(c, 3),
      NthPos(c, 4), NthCurrentView(c), 0, TRUE)

ActualNthOutput(c) ==
  CASE Bug = "nth_allows_rewind" /\ c = "nth_rewind" ->
         Out(TRUE, 4, 2, 3, 0, 1, NthTargetView(c), 0, TRUE)
    [] Bug = "nth_uses_absolute_view_as_rem" /\ c = "nth_forward_three" ->
         Out(TRUE, 4, 0, 1, 2, 3, NthTargetView(c), NthDelta(c), TRUE)
    [] Bug = "nth_does_not_update_view" /\ c = "nth_forward_one" ->
         Out(TRUE, 4, 1, 2, 3, 0, NthCurrentView(c), NthDelta(c), TRUE)
    [] Bug = "nth_returns_remainder" /\ c = "nth_large_mod" ->
         Out(TRUE, 4, 2, 3, 0, 1, NthTargetView(c), NthRem(c), TRUE)
    [] Bug = "nth_zero_rem_rotates" /\ c = "nth_full_cycle" ->
         Out(TRUE, 4, 1, 2, 3, 0, NthTargetView(c), NthDelta(c), TRUE)
    [] OTHER -> SpecNthOutput(c)

SpecNewOutput(c) ==
  CASE c = "new_dedup_preserve" -> Out(TRUE, 3, 2, 1, 0, NoPeer, 0, 0, TRUE)
    [] c = "new_all_duplicates" -> Out(TRUE, 1, 1, NoPeer, NoPeer, NoPeer, 0, 0, TRUE)
    [] c = "new_single" -> Out(TRUE, 1, 4, NoPeer, NoPeer, NoPeer, 0, 0, TRUE)

ActualNewOutput(c) ==
  CASE Bug = "new_sorts_input" /\ c = "new_dedup_preserve" ->
         Out(TRUE, 3, 0, 1, 2, NoPeer, 0, 0, TRUE)
    [] Bug = "new_keeps_duplicates" /\ c = "new_dedup_preserve" ->
         Out(TRUE, 4, 2, 1, 2, 0, 0, 0, FALSE)
    [] Bug = "new_drops_first" /\ c = "new_dedup_preserve" ->
         Out(TRUE, 2, 1, 0, NoPeer, NoPeer, 0, 0, TRUE)
    [] OTHER -> SpecNewOutput(c)

UpdateView(c) == 9

SpecUpdateOutput(c) ==
  CASE c = "update_mixed" ->
         Out(TRUE, 4, 1, 3, 5, 4, UpdateView(c), 0, TRUE)
    [] c = "update_keep_all_reordered_input" ->
         Out(TRUE, 3, 0, 1, 2, NoPeer, UpdateView(c), 0, TRUE)
    [] c = "update_remove_all_add_two" ->
         Out(TRUE, 2, 4, 5, NoPeer, NoPeer, UpdateView(c), 0, TRUE)
    [] c = "update_duplicates" ->
         Out(TRUE, 2, 1, 2, NoPeer, NoPeer, UpdateView(c), 0, TRUE)

ActualUpdateOutput(c) ==
  CASE Bug = "update_reorders_old_by_new_input"
       /\ c = "update_keep_all_reordered_input" ->
         Out(TRUE, 3, 2, 0, 1, NoPeer, UpdateView(c), 0, TRUE)
    [] Bug = "update_keeps_removed_peer" /\ c = "update_mixed" ->
         Out(TRUE, 5, 0, 1, 3, 5, UpdateView(c), 0, FALSE)
    [] Bug = "update_drops_new_peer" /\ c = "update_mixed" ->
         Out(TRUE, 2, 1, 3, NoPeer, NoPeer, UpdateView(c), 0, TRUE)
    [] Bug = "update_duplicates_new_peer" /\ c = "update_duplicates" ->
         Out(TRUE, 3, 1, 2, 2, NoPeer, UpdateView(c), 0, FALSE)
    [] Bug = "update_resets_view" /\ c = "update_mixed" ->
         Out(TRUE, 4, 1, 3, 5, 4, 0, 0, TRUE)
    [] OTHER -> SpecUpdateOutput(c)

SpecBlockOutput(c) ==
  CASE c = "block_mixed" -> Out(TRUE, 4, 1, 3, 5, 4, 0, 0, TRUE)
    [] c = "block_keep_all_reordered_input" ->
         Out(TRUE, 3, 0, 1, 2, NoPeer, 0, 0, TRUE)
    [] c = "block_remove_all_add_two" ->
         Out(TRUE, 2, 4, 5, NoPeer, NoPeer, 0, 0, TRUE)

ActualBlockOutput(c) ==
  CASE Bug = "block_preserves_view" /\ c = "block_mixed" ->
         Out(TRUE, 4, 1, 3, 5, 4, UpdateView(c), 0, TRUE)
    [] Bug = "block_reorders_old_by_new_input" /\ c = "block_mixed" ->
         Out(TRUE, 4, 3, 1, 5, 4, 0, 0, TRUE)
    [] OTHER -> SpecBlockOutput(c)

SpecCanonOutput(c) ==
  CASE c = "canon_reverse" -> Out(TRUE, 4, 0, 1, 2, 3, 8, 0, TRUE)
    [] c = "canon_duplicates" -> Out(TRUE, 3, 0, 1, 2, NoPeer, 8, 0, TRUE)
    [] c = "canon_empty" -> Out(TRUE, 0, NoPeer, NoPeer, NoPeer, NoPeer, 8, 0, TRUE)

ActualCanonOutput(c) ==
  CASE Bug = "canon_preserves_unsorted" /\ c = "canon_reverse" ->
         Out(TRUE, 4, 3, 2, 1, 0, 8, 0, TRUE)
    [] Bug = "canon_resets_view" /\ c = "canon_reverse" ->
         Out(TRUE, 4, 0, 1, 2, 3, 0, 0, TRUE)
    [] Bug = "canon_keeps_duplicates" /\ c = "canon_duplicates" ->
         Out(TRUE, 5, 0, 1, 1, 2, 8, 0, FALSE)
    [] OTHER -> SpecCanonOutput(c)

SpecOutput(c) ==
  IF c \in RotateCases THEN SpecRotateOutput(c)
  ELSE IF c \in NthCases THEN SpecNthOutput(c)
  ELSE IF c \in NewCases THEN SpecNewOutput(c)
  ELSE IF c \in UpdateCases THEN SpecUpdateOutput(c)
  ELSE IF c \in BlockCases THEN SpecBlockOutput(c)
  ELSE SpecCanonOutput(c)

ActualOutput(c) ==
  IF c \in RotateCases THEN ActualRotateOutput(c)
  ELSE IF c \in NthCases THEN ActualNthOutput(c)
  ELSE IF c \in NewCases THEN ActualNewOutput(c)
  ELSE IF c \in UpdateCases THEN ActualUpdateOutput(c)
  ELSE IF c \in BlockCases THEN ActualBlockOutput(c)
  ELSE ActualCanonOutput(c)

CInit == TRUE

Init == checked = 0

Next == checked' = 1 - checked

TypeInvariant == checked \in 0..1

TopologyMutationCoreSafety ==
  \A c \in Cases: ActualOutput(c) = SpecOutput(c)

NoBugInvariant == TopologyMutationCoreSafety

SafetyFast == TopologyMutationCoreSafety

TopologyRotationExact ==
  \A c \in RotateCases:
    ActualRotateOutput(c) = SpecRotateOutput(c)

TopologyNthRotationExact ==
  \A c \in NthCases:
    ActualNthOutput(c) = SpecNthOutput(c)

TopologyConstructionExact ==
  \A c \in NewCases:
    ActualNewOutput(c) = SpecNewOutput(c)

TopologyPeerListUpdateExact ==
  \A c \in UpdateCases:
    ActualUpdateOutput(c) = SpecUpdateOutput(c)

TopologyBlockCommitResetExact ==
  \A c \in BlockCases:
    ActualBlockOutput(c) = SpecBlockOutput(c)

TopologyCanonicalizationExact ==
  \A c \in CanonCases:
    ActualCanonOutput(c) = SpecCanonOutput(c)

TopologyOrderedRosterMutationExactness ==
  /\ SafetyFast
  /\ TopologyRotationExact
  /\ TopologyNthRotationExact
  /\ TopologyConstructionExact
  /\ TopologyPeerListUpdateExact
  /\ TopologyBlockCommitResetExact
  /\ TopologyCanonicalizationExact

BugRotateIgnoresModulo ==
  ActualOutput("rotate_len4_idx6") = SpecOutput("rotate_len4_idx6")

BugRotateOffByOne ==
  ActualOutput("rotate_len4_idx2") = SpecOutput("rotate_len4_idx2")

BugRotateResetsView ==
  ActualOutput("rotate_len4_idx2") = SpecOutput("rotate_len4_idx2")

ExpectedRotateEmpty ==
  Out(TRUE, 0, NoPeer, NoPeer, NoPeer, NoPeer, RotateView("rotate_empty"), 0, TRUE)

ActualRotateEmptyForBug ==
  IF Bug = "rotate_empty_mutates" THEN
    Out(TRUE, 1, 0, NoPeer, NoPeer, NoPeer, RotateView("rotate_empty"), 0, TRUE)
  ELSE
    ExpectedRotateEmpty

BugRotateEmptyMutates ==
  ActualRotateEmptyForBug = ExpectedRotateEmpty

BugNthAllowsRewind ==
  ActualOutput("nth_rewind") = SpecOutput("nth_rewind")

BugNthUsesAbsoluteViewAsRem ==
  ActualOutput("nth_forward_three") = SpecOutput("nth_forward_three")

BugNthDoesNotUpdateView ==
  ActualOutput("nth_forward_one") = SpecOutput("nth_forward_one")

BugNthReturnsRemainder ==
  ActualOutput("nth_large_mod") = SpecOutput("nth_large_mod")

BugNthZeroRemRotates ==
  ActualOutput("nth_full_cycle") = SpecOutput("nth_full_cycle")

BugNewSortsInput ==
  ActualOutput("new_dedup_preserve") = SpecOutput("new_dedup_preserve")

BugNewKeepsDuplicates ==
  ActualOutput("new_dedup_preserve") = SpecOutput("new_dedup_preserve")

BugNewDropsFirst ==
  ActualOutput("new_dedup_preserve") = SpecOutput("new_dedup_preserve")

BugUpdateReordersOldByNewInput ==
  ActualOutput("update_keep_all_reordered_input")
    = SpecOutput("update_keep_all_reordered_input")

BugUpdateKeepsRemovedPeer ==
  ActualOutput("update_mixed") = SpecOutput("update_mixed")

BugUpdateDropsNewPeer ==
  ActualOutput("update_mixed") = SpecOutput("update_mixed")

BugUpdateDuplicatesNewPeer ==
  ActualOutput("update_duplicates") = SpecOutput("update_duplicates")

BugUpdateResetsView ==
  ActualOutput("update_mixed") = SpecOutput("update_mixed")

BugBlockPreservesView ==
  ActualOutput("block_mixed") = SpecOutput("block_mixed")

BugBlockReordersOldByNewInput ==
  ActualOutput("block_mixed") = SpecOutput("block_mixed")

BugCanonPreservesUnsorted ==
  ActualOutput("canon_reverse") = SpecOutput("canon_reverse")

BugCanonResetsView ==
  ActualOutput("canon_reverse") = SpecOutput("canon_reverse")

BugCanonKeepsDuplicates ==
  ActualOutput("canon_duplicates") = SpecOutput("canon_duplicates")

====
