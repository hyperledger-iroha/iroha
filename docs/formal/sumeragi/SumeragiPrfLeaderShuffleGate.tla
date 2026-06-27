---- MODULE SumeragiPrfLeaderShuffleGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for Sumeragi PRF topology ordering helpers.

This slice covers `Topology::shuffle_prf(...)`,
`Topology::leader_index_prf(...)`, and `shuffled_for_prf_seed(...)`.

The Blake2 PRF is abstracted by fixed bounded permutations. The contract under
test is that helpers consume a single deterministic permutation per
seed/height, select leaders by `view % len`, keep empty/single rosters stable,
preserve permutation length/distinctness, and canonicalize/deduplicate wrapper
input before shuffling with view reset to zero.
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

\* @type: (Int, Int, Int, Int, Int, Int, Int, Bool, Bool, Bool) => <<Int, Int, Int, Int, Int, Int, Int, Bool, Bool, Bool>>;
Out(len, p1, p2, p3, p4, view, leader, distinct, periodic, cycleDistinct) ==
  <<len, p1, p2, p3, p4, view, leader, distinct, periodic, cycleDistinct>>

Cases == {
  "leader_empty",
  "leader_single",
  "leader_len4_view0",
  "leader_len4_view3",
  "leader_len4_view5",
  "leader_len4_periodic",
  "leader_len4_cycle_distinct",
  "shuffle_empty",
  "shuffle_single",
  "shuffle_len4",
  "wrapper_canonical_dedup",
  "wrapper_single_dedup",
  "wrapper_alt_height"
}

LeaderSelectionCases == {
  "leader_empty",
  "leader_single",
  "leader_len4_view0",
  "leader_len4_view3",
  "leader_len4_view5"
}

LeaderCycleCases == {
  "leader_len4_periodic",
  "leader_len4_cycle_distinct"
}

ShuffleCases == {
  "shuffle_empty",
  "shuffle_single",
  "shuffle_len4"
}

WrapperCases == {
  "wrapper_canonical_dedup",
  "wrapper_single_dedup",
  "wrapper_alt_height"
}

Bugs == {
  "none",
  "leader_empty_returns_one",
  "leader_single_uses_prf_slot",
  "leader_view_not_modded",
  "leader_uses_identity_order",
  "leader_period_breaks_cycle",
  "leader_cycle_has_duplicate",
  "shuffle_single_mutates",
  "shuffle_identity_for_multi",
  "shuffle_loses_peer",
  "shuffle_duplicates_peer",
  "wrapper_skips_sort",
  "wrapper_keeps_duplicates",
  "wrapper_preserves_view",
  "wrapper_skips_shuffle",
  "wrapper_alt_uses_base_height"
}

PermIndex(n, variant, pos) ==
  CASE n = 1 -> 0
    [] n = 3 /\ pos = 0 -> 1
    [] n = 3 /\ pos = 1 -> 2
    [] n = 3 /\ pos = 2 -> 0
    [] n = 4 /\ variant = "alt" /\ pos = 0 -> 1
    [] n = 4 /\ variant = "alt" /\ pos = 1 -> 3
    [] n = 4 /\ variant = "alt" /\ pos = 2 -> 0
    [] n = 4 /\ variant = "alt" /\ pos = 3 -> 2
    [] n = 4 /\ pos = 0 -> 2
    [] n = 4 /\ pos = 1 -> 0
    [] n = 4 /\ pos = 2 -> 3
    [] n = 4 /\ pos = 3 -> 1
    [] OTHER -> NoPeer

LeaderIndex(n, variant, view) ==
  IF n = 0 THEN 0 ELSE PermIndex(n, variant, Mod(view, n))

Distinct3(a, b, c) == a # b /\ a # c /\ b # c
Distinct4(a, b, c, d) ==
  a # b /\ a # c /\ a # d /\ b # c /\ b # d /\ c # d

PermDistinct(n, variant) ==
  CASE n = 0 -> TRUE
    [] n = 1 -> TRUE
    [] n = 3 -> Distinct3(
         PermIndex(3, variant, 0),
         PermIndex(3, variant, 1),
         PermIndex(3, variant, 2))
    [] n = 4 -> Distinct4(
         PermIndex(4, variant, 0),
         PermIndex(4, variant, 1),
         PermIndex(4, variant, 2),
         PermIndex(4, variant, 3))
    [] OTHER -> FALSE

ShufflePeer(n, variant, pos) ==
  IF pos >= n THEN NoPeer ELSE PermIndex(n, variant, pos)

CanonicalPeer(idx) == idx + 1

WrapperPeer(n, variant, pos) ==
  IF pos >= n THEN NoPeer ELSE CanonicalPeer(PermIndex(n, variant, pos))

SpecOutput(c) ==
  CASE c = "leader_empty" ->
         Out(0, NoPeer, NoPeer, NoPeer, NoPeer, 0, 0, TRUE, TRUE, TRUE)
    [] c = "leader_single" ->
         Out(1, 0, NoPeer, NoPeer, NoPeer, 0, LeaderIndex(1, "base", 9), TRUE, TRUE, TRUE)
    [] c = "leader_len4_view0" ->
         Out(4, NoPeer, NoPeer, NoPeer, NoPeer, 0, LeaderIndex(4, "base", 0), TRUE, TRUE, TRUE)
    [] c = "leader_len4_view3" ->
         Out(4, NoPeer, NoPeer, NoPeer, NoPeer, 0, LeaderIndex(4, "base", 3), TRUE, TRUE, TRUE)
    [] c = "leader_len4_view5" ->
         Out(4, NoPeer, NoPeer, NoPeer, NoPeer, 0, LeaderIndex(4, "base", 5), TRUE, TRUE, TRUE)
    [] c = "leader_len4_periodic" ->
         Out(4, NoPeer, NoPeer, NoPeer, NoPeer, 0, NoPeer, TRUE,
           LeaderIndex(4, "base", 1) = LeaderIndex(4, "base", 5), TRUE)
    [] c = "leader_len4_cycle_distinct" ->
         Out(4, NoPeer, NoPeer, NoPeer, NoPeer, 0, NoPeer, TRUE, TRUE,
           Distinct4(
             LeaderIndex(4, "base", 0),
             LeaderIndex(4, "base", 1),
             LeaderIndex(4, "base", 2),
             LeaderIndex(4, "base", 3)))
    [] c = "shuffle_empty" ->
         Out(0, NoPeer, NoPeer, NoPeer, NoPeer, 7, NoPeer, TRUE, TRUE, TRUE)
    [] c = "shuffle_single" ->
         Out(1, 0, NoPeer, NoPeer, NoPeer, 7, NoPeer, TRUE, TRUE, TRUE)
    [] c = "shuffle_len4" ->
         Out(4,
           ShufflePeer(4, "base", 0),
           ShufflePeer(4, "base", 1),
           ShufflePeer(4, "base", 2),
           ShufflePeer(4, "base", 3),
           7, NoPeer, PermDistinct(4, "base"), TRUE, TRUE)
    [] c = "wrapper_canonical_dedup" ->
         Out(3,
           WrapperPeer(3, "base", 0),
           WrapperPeer(3, "base", 1),
           WrapperPeer(3, "base", 2),
           NoPeer,
           0, NoPeer, PermDistinct(3, "base"), TRUE, TRUE)
    [] c = "wrapper_single_dedup" ->
         Out(1, 4, NoPeer, NoPeer, NoPeer, 0, NoPeer, TRUE, TRUE, TRUE)
    [] c = "wrapper_alt_height" ->
         Out(4,
           ShufflePeer(4, "alt", 0),
           ShufflePeer(4, "alt", 1),
           ShufflePeer(4, "alt", 2),
           ShufflePeer(4, "alt", 3),
           0, NoPeer, PermDistinct(4, "alt"), TRUE, TRUE)
    [] OTHER -> Out(0, NoPeer, NoPeer, NoPeer, NoPeer, 0, NoPeer, TRUE, TRUE, TRUE)

ActualOutput(c) ==
  CASE Bug = "leader_empty_returns_one" /\ c = "leader_empty" ->
         Out(0, NoPeer, NoPeer, NoPeer, NoPeer, 0, 1, TRUE, TRUE, TRUE)
    [] Bug = "leader_single_uses_prf_slot" /\ c = "leader_single" ->
         Out(1, 0, NoPeer, NoPeer, NoPeer, 0, NoPeer, TRUE, TRUE, TRUE)
    [] Bug = "leader_view_not_modded" /\ c = "leader_len4_view5" ->
         Out(4, NoPeer, NoPeer, NoPeer, NoPeer, 0, NoPeer, TRUE, TRUE, TRUE)
    [] Bug = "leader_uses_identity_order" /\ c = "leader_len4_view0" ->
         Out(4, NoPeer, NoPeer, NoPeer, NoPeer, 0, 0, TRUE, TRUE, TRUE)
    [] Bug = "leader_period_breaks_cycle" /\ c = "leader_len4_periodic" ->
         Out(4, NoPeer, NoPeer, NoPeer, NoPeer, 0, NoPeer, TRUE, FALSE, TRUE)
    [] Bug = "leader_cycle_has_duplicate" /\ c = "leader_len4_cycle_distinct" ->
         Out(4, NoPeer, NoPeer, NoPeer, NoPeer, 0, NoPeer, TRUE, TRUE, FALSE)
    [] Bug = "shuffle_single_mutates" /\ c = "shuffle_single" ->
         Out(1, 1, NoPeer, NoPeer, NoPeer, 7, NoPeer, TRUE, TRUE, TRUE)
    [] Bug = "shuffle_identity_for_multi" /\ c = "shuffle_len4" ->
         Out(4, 0, 1, 2, 3, 7, NoPeer, TRUE, TRUE, TRUE)
    [] Bug = "shuffle_loses_peer" /\ c = "shuffle_len4" ->
         Out(3, 2, 0, 3, NoPeer, 7, NoPeer, TRUE, TRUE, TRUE)
    [] Bug = "shuffle_duplicates_peer" /\ c = "shuffle_len4" ->
         Out(4, 2, 0, 0, 1, 7, NoPeer, FALSE, TRUE, TRUE)
    [] Bug = "wrapper_skips_sort" /\ c = "wrapper_canonical_dedup" ->
         Out(3, 3, 2, 1, NoPeer, 0, NoPeer, TRUE, TRUE, TRUE)
    [] Bug = "wrapper_keeps_duplicates" /\ c = "wrapper_canonical_dedup" ->
         Out(4, 3, 1, 3, 2, 0, NoPeer, FALSE, TRUE, TRUE)
    [] Bug = "wrapper_preserves_view" /\ c = "wrapper_canonical_dedup" ->
         Out(3,
           WrapperPeer(3, "base", 0),
           WrapperPeer(3, "base", 1),
           WrapperPeer(3, "base", 2),
           NoPeer,
           7, NoPeer, TRUE, TRUE, TRUE)
    [] Bug = "wrapper_skips_shuffle" /\ c = "wrapper_canonical_dedup" ->
         Out(3, 1, 2, 3, NoPeer, 0, NoPeer, TRUE, TRUE, TRUE)
    [] Bug = "wrapper_alt_uses_base_height" /\ c = "wrapper_alt_height" ->
         Out(4,
           ShufflePeer(4, "base", 0),
           ShufflePeer(4, "base", 1),
           ShufflePeer(4, "base", 2),
           ShufflePeer(4, "base", 3),
           0, NoPeer, TRUE, TRUE, TRUE)
    [] OTHER -> SpecOutput(c)

Init == checked = 0

Next == checked' = 1 - checked

TypeInvariant == checked \in 0..1 /\ Bug \in Bugs

PrfLeaderShuffleMatchesSpec ==
  \A c \in Cases: ActualOutput(c) = SpecOutput(c)

SafetyFast == PrfLeaderShuffleMatchesSpec

PrfLeaderSelectionExact ==
  \A c \in LeaderSelectionCases:
    ActualOutput(c) = SpecOutput(c)

PrfLeaderCycleExact ==
  \A c \in LeaderCycleCases:
    ActualOutput(c) = SpecOutput(c)

PrfShufflePermutationExact ==
  \A c \in ShuffleCases:
    ActualOutput(c) = SpecOutput(c)

PrfWrapperCanonicalShuffleExact ==
  \A c \in WrapperCases:
    ActualOutput(c) = SpecOutput(c)

PrfLeaderShuffleExactness ==
  /\ PrfLeaderShuffleMatchesSpec
  /\ PrfLeaderSelectionExact
  /\ PrfLeaderCycleExact
  /\ PrfShufflePermutationExact
  /\ PrfWrapperCanonicalShuffleExact

PrfLeaderShuffleCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ PrfLeaderShuffleExactness

====
