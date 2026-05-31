---- MODULE SumeragiProposalBatchGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for proposal batch trimming and canonicalization in
`main_loop/propose.rs`.

This slice pins `trim_batch_for_size_cap(...)`,
`trim_batch_for_size_cap_with_plans(...)`,
`canonicalize_parallel_batch_by_key(...)`, and
`canonicalize_proposal_batch_with_plans(...)`. The model abstracts
transactions as numeric ids and derives routing, plan, and size companions
from those ids so alignment bugs are visible in the output tuple.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NoItem == 0

TrimCases == {
  "trim_no_excess",
  "trim_remove_one",
  "trim_remove_multiple",
  "trim_keeps_single",
  "trim_zero_size_floor",
  "trim_with_plans_align"
}

CanonCases == {
  "canon_empty",
  "canon_single",
  "canon_already_sorted",
  "canon_reverse_keys",
  "canon_duplicate_keys_stable",
  "canon_with_plans"
}

TxRoute(tx) ==
  IF tx = NoItem THEN NoItem ELSE tx + 10

TxPlan(tx) ==
  IF tx = NoItem THEN NoItem ELSE tx + 20

TxSize(tx) ==
  IF tx = NoItem THEN NoItem ELSE tx + 30

InitialTrimLen(c) ==
  CASE c = "trim_remove_multiple" -> 4
    [] OTHER -> 3

SpecTrimRemovedCount(c) ==
  CASE c = "trim_no_excess" -> 0
    [] c \in {"trim_remove_one", "trim_zero_size_floor",
              "trim_with_plans_align"} -> 1
    [] OTHER -> 2

SpecTrimRemainingLen(c) ==
  InitialTrimLen(c) - SpecTrimRemovedCount(c)

SpecTrimRem1(c) ==
  IF SpecTrimRemainingLen(c) >= 1 THEN 1 ELSE NoItem

SpecTrimRem2(c) ==
  IF SpecTrimRemainingLen(c) >= 2 THEN 2 ELSE NoItem

SpecTrimRem3(c) ==
  IF SpecTrimRemainingLen(c) >= 3 THEN 3 ELSE NoItem

SpecTrimRem4(c) ==
  IF SpecTrimRemainingLen(c) >= 4 THEN 4 ELSE NoItem

SpecTrimRemoved1(c) ==
  IF SpecTrimRemovedCount(c) >= 1 THEN InitialTrimLen(c) ELSE NoItem

SpecTrimRemoved2(c) ==
  IF SpecTrimRemovedCount(c) >= 2 THEN InitialTrimLen(c) - 1 ELSE NoItem

SpecTrimRemoved3(c) ==
  IF SpecTrimRemovedCount(c) >= 3 THEN InitialTrimLen(c) - 2 ELSE NoItem

SpecTrimSizesLen(c) ==
  SpecTrimRemainingLen(c)

\* @type: (Str) => <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>;
SpecTrimOutput(c) ==
  <<SpecTrimRemainingLen(c), SpecTrimRemovedCount(c),
    SpecTrimRem1(c), SpecTrimRem2(c), SpecTrimRem3(c), SpecTrimRem4(c),
    SpecTrimRemoved1(c), SpecTrimRemoved2(c), SpecTrimRemoved3(c),
    TxRoute(SpecTrimRemoved1(c)), TxPlan(SpecTrimRemoved1(c)),
    SpecTrimSizesLen(c)>>

ActualTrimRemovedCount(c) ==
  CASE Bug = "trim_no_excess_removes" /\ c = "trim_no_excess" -> 1
    [] Bug = "trim_under_removes_excess" /\ c = "trim_remove_multiple" -> 1
    [] Bug = "trim_drops_singleton" /\ c = "trim_keeps_single" -> 3
    [] Bug = "trim_zero_size_not_floored" /\ c = "trim_zero_size_floor" -> 2
    [] OTHER -> SpecTrimRemovedCount(c)

ActualTrimRemainingLen(c) ==
  CASE Bug = "trim_removes_front" /\ c = "trim_remove_one" -> 2
    [] OTHER -> InitialTrimLen(c) - ActualTrimRemovedCount(c)

ActualTrimRem1(c) ==
  CASE Bug = "trim_removes_front" /\ c = "trim_remove_one" -> 2
    [] OTHER -> IF ActualTrimRemainingLen(c) >= 1 THEN 1 ELSE NoItem

ActualTrimRem2(c) ==
  CASE Bug = "trim_removes_front" /\ c = "trim_remove_one" -> 3
    [] OTHER -> IF ActualTrimRemainingLen(c) >= 2 THEN 2 ELSE NoItem

ActualTrimRem3(c) ==
  CASE Bug = "trim_removes_front" /\ c = "trim_remove_one" -> NoItem
    [] OTHER -> IF ActualTrimRemainingLen(c) >= 3 THEN 3 ELSE NoItem

ActualTrimRem4(c) ==
  IF ActualTrimRemainingLen(c) >= 4 THEN 4 ELSE NoItem

ActualTrimRemoved1(c) ==
  CASE Bug = "trim_removes_front" /\ c = "trim_remove_one" -> 1
    [] Bug = "trim_removed_order_reversed" /\ c = "trim_remove_multiple" -> 3
    [] OTHER ->
       IF ActualTrimRemovedCount(c) >= 1 THEN InitialTrimLen(c) ELSE NoItem

ActualTrimRemoved2(c) ==
  CASE Bug = "trim_removed_order_reversed" /\ c = "trim_remove_multiple" -> 4
    [] OTHER ->
       IF ActualTrimRemovedCount(c) >= 2 THEN InitialTrimLen(c) - 1 ELSE NoItem

ActualTrimRemoved3(c) ==
  IF ActualTrimRemovedCount(c) >= 3 THEN InitialTrimLen(c) - 2 ELSE NoItem

ActualTrimRemovedRoute1(c) ==
  CASE Bug = "trim_route_misaligned" /\ c = "trim_remove_one" ->
       TxRoute(1)
    [] OTHER -> TxRoute(ActualTrimRemoved1(c))

ActualTrimRemovedPlan1(c) ==
  CASE Bug = "trim_plan_misaligned" /\ c = "trim_with_plans_align" ->
       TxPlan(1)
    [] OTHER -> TxPlan(ActualTrimRemoved1(c))

ActualTrimSizesLen(c) ==
  CASE Bug = "trim_forgets_size_pop" /\ c = "trim_remove_one" ->
       InitialTrimLen(c)
    [] OTHER -> ActualTrimRemainingLen(c)

\* @type: (Str) => <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>;
ActualTrimOutput(c) ==
  <<ActualTrimRemainingLen(c), ActualTrimRemovedCount(c),
    ActualTrimRem1(c), ActualTrimRem2(c), ActualTrimRem3(c), ActualTrimRem4(c),
    ActualTrimRemoved1(c), ActualTrimRemoved2(c), ActualTrimRemoved3(c),
    ActualTrimRemovedRoute1(c), ActualTrimRemovedPlan1(c),
    ActualTrimSizesLen(c)>>

SpecCanonLen(c) ==
  CASE c = "canon_empty" -> 0
    [] c = "canon_single" -> 1
    [] c = "canon_duplicate_keys_stable" -> 4
    [] OTHER -> 3

SpecCanonTx1(c) ==
  CASE c = "canon_empty" -> NoItem
    [] c \in {"canon_single", "canon_already_sorted"} -> 1
    [] c = "canon_reverse_keys" -> 3
    [] c = "canon_duplicate_keys_stable" -> 2
    [] c = "canon_with_plans" -> 2

SpecCanonTx2(c) ==
  CASE c \in {"canon_empty", "canon_single"} -> NoItem
    [] c = "canon_already_sorted" -> 2
    [] c = "canon_reverse_keys" -> 2
    [] c = "canon_duplicate_keys_stable" -> 4
    [] c = "canon_with_plans" -> 1

SpecCanonTx3(c) ==
  CASE c \in {"canon_empty", "canon_single"} -> NoItem
    [] c = "canon_already_sorted" -> 3
    [] c = "canon_reverse_keys" -> 1
    [] c = "canon_duplicate_keys_stable" -> 3
    [] c = "canon_with_plans" -> 3

SpecCanonTx4(c) ==
  IF c = "canon_duplicate_keys_stable" THEN 1 ELSE NoItem

\* @type: (Str) => <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>;
SpecCanonOutput(c) ==
  <<SpecCanonLen(c),
    SpecCanonTx1(c), SpecCanonTx2(c), SpecCanonTx3(c), SpecCanonTx4(c),
    TxRoute(SpecCanonTx1(c)), TxRoute(SpecCanonTx2(c)),
    TxRoute(SpecCanonTx3(c)), TxRoute(SpecCanonTx4(c)),
    TxPlan(SpecCanonTx1(c)), TxPlan(SpecCanonTx2(c)),
    TxPlan(SpecCanonTx3(c)), TxPlan(SpecCanonTx4(c)),
    TxSize(SpecCanonTx1(c)), TxSize(SpecCanonTx2(c)),
    TxSize(SpecCanonTx3(c)), TxSize(SpecCanonTx4(c))>>

ActualCanonLen(c) ==
  CASE Bug = "canon_empty_mutates" /\ c = "canon_empty" -> 1
    [] Bug = "canon_length_changes" /\ c = "canon_duplicate_keys_stable" -> 3
    [] OTHER -> SpecCanonLen(c)

ActualCanonTx1(c) ==
  CASE Bug = "canon_empty_mutates" /\ c = "canon_empty" -> 1
    [] Bug = "canon_single_mutates" /\ c = "canon_single" -> 2
    [] Bug = "canon_skip_sort" /\ c = "canon_reverse_keys" -> 1
    [] Bug = "canon_reverse_sort" /\ c = "canon_already_sorted" -> 3
    [] Bug = "canon_duplicate_unstable" /\ c = "canon_duplicate_keys_stable" -> 4
    [] Bug = "canon_wrong_key_sort" /\ c = "canon_duplicate_keys_stable" -> 1
    [] OTHER -> SpecCanonTx1(c)

ActualCanonTx2(c) ==
  CASE Bug = "canon_skip_sort" /\ c = "canon_reverse_keys" -> 2
    [] Bug = "canon_reverse_sort" /\ c = "canon_already_sorted" -> 2
    [] Bug = "canon_duplicate_unstable" /\ c = "canon_duplicate_keys_stable" -> 2
    [] Bug = "canon_wrong_key_sort" /\ c = "canon_duplicate_keys_stable" -> 2
    [] OTHER -> SpecCanonTx2(c)

ActualCanonTx3(c) ==
  CASE Bug = "canon_skip_sort" /\ c = "canon_reverse_keys" -> 3
    [] Bug = "canon_reverse_sort" /\ c = "canon_already_sorted" -> 1
    [] Bug = "canon_wrong_key_sort" /\ c = "canon_duplicate_keys_stable" -> 3
    [] OTHER -> SpecCanonTx3(c)

ActualCanonTx4(c) ==
  CASE Bug = "canon_duplicate_unstable" /\ c = "canon_duplicate_keys_stable" -> 1
    [] Bug = "canon_wrong_key_sort" /\ c = "canon_duplicate_keys_stable" -> 4
    [] Bug = "canon_length_changes" /\ c = "canon_duplicate_keys_stable" -> NoItem
    [] OTHER -> SpecCanonTx4(c)

ActualCanonRoute1(c) ==
  CASE Bug = "canon_route_misaligned" /\ c = "canon_reverse_keys" -> TxRoute(1)
    [] OTHER -> TxRoute(ActualCanonTx1(c))

ActualCanonRoute2(c) ==
  CASE Bug = "canon_route_misaligned" /\ c = "canon_reverse_keys" -> TxRoute(2)
    [] OTHER -> TxRoute(ActualCanonTx2(c))

ActualCanonRoute3(c) ==
  CASE Bug = "canon_route_misaligned" /\ c = "canon_reverse_keys" -> TxRoute(3)
    [] OTHER -> TxRoute(ActualCanonTx3(c))

ActualCanonRoute4(c) ==
  TxRoute(ActualCanonTx4(c))

ActualCanonPlan1(c) ==
  CASE Bug = "canon_plan_misaligned" /\ c = "canon_with_plans" -> TxPlan(1)
    [] OTHER -> TxPlan(ActualCanonTx1(c))

ActualCanonPlan2(c) ==
  CASE Bug = "canon_plan_misaligned" /\ c = "canon_with_plans" -> TxPlan(2)
    [] OTHER -> TxPlan(ActualCanonTx2(c))

ActualCanonPlan3(c) ==
  CASE Bug = "canon_plan_misaligned" /\ c = "canon_with_plans" -> TxPlan(3)
    [] OTHER -> TxPlan(ActualCanonTx3(c))

ActualCanonPlan4(c) ==
  TxPlan(ActualCanonTx4(c))

ActualCanonSize1(c) ==
  CASE Bug = "canon_size_misaligned" /\ c = "canon_reverse_keys" -> TxSize(1)
    [] OTHER -> TxSize(ActualCanonTx1(c))

ActualCanonSize2(c) ==
  CASE Bug = "canon_size_misaligned" /\ c = "canon_reverse_keys" -> TxSize(2)
    [] OTHER -> TxSize(ActualCanonTx2(c))

ActualCanonSize3(c) ==
  CASE Bug = "canon_size_misaligned" /\ c = "canon_reverse_keys" -> TxSize(3)
    [] OTHER -> TxSize(ActualCanonTx3(c))

ActualCanonSize4(c) ==
  TxSize(ActualCanonTx4(c))

\* @type: (Str) => <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>;
ActualCanonOutput(c) ==
  <<ActualCanonLen(c),
    ActualCanonTx1(c), ActualCanonTx2(c), ActualCanonTx3(c), ActualCanonTx4(c),
    ActualCanonRoute1(c), ActualCanonRoute2(c),
    ActualCanonRoute3(c), ActualCanonRoute4(c),
    ActualCanonPlan1(c), ActualCanonPlan2(c),
    ActualCanonPlan3(c), ActualCanonPlan4(c),
    ActualCanonSize1(c), ActualCanonSize2(c),
    ActualCanonSize3(c), ActualCanonSize4(c)>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "trim_no_excess_removes",
       "trim_under_removes_excess",
       "trim_drops_singleton",
       "trim_zero_size_not_floored",
       "trim_removes_front",
       "trim_removed_order_reversed",
       "trim_route_misaligned",
       "trim_plan_misaligned",
       "trim_forgets_size_pop",
       "canon_empty_mutates",
       "canon_single_mutates",
       "canon_skip_sort",
       "canon_reverse_sort",
       "canon_duplicate_unstable",
       "canon_wrong_key_sort",
       "canon_route_misaligned",
       "canon_plan_misaligned",
       "canon_size_misaligned",
       "canon_length_changes"
     }
  /\ checked = 0

SafetyFast ==
  /\ \A c \in TrimCases: ActualTrimOutput(c) = SpecTrimOutput(c)
  /\ \A c \in CanonCases: ActualCanonOutput(c) = SpecCanonOutput(c)

BugTrimNoExcessRemoves ==
  ActualTrimOutput("trim_no_excess") = SpecTrimOutput("trim_no_excess")

BugTrimUnderRemovesExcess ==
  ActualTrimOutput("trim_remove_multiple") =
    SpecTrimOutput("trim_remove_multiple")

BugTrimDropsSingleton ==
  ActualTrimOutput("trim_keeps_single") = SpecTrimOutput("trim_keeps_single")

BugTrimZeroSizeNotFloored ==
  ActualTrimOutput("trim_zero_size_floor") =
    SpecTrimOutput("trim_zero_size_floor")

BugTrimRemovesFront ==
  ActualTrimOutput("trim_remove_one") = SpecTrimOutput("trim_remove_one")

BugTrimRemovedOrderReversed ==
  ActualTrimOutput("trim_remove_multiple") =
    SpecTrimOutput("trim_remove_multiple")

BugTrimRouteMisaligned ==
  ActualTrimOutput("trim_remove_one") = SpecTrimOutput("trim_remove_one")

BugTrimPlanMisaligned ==
  ActualTrimOutput("trim_with_plans_align") =
    SpecTrimOutput("trim_with_plans_align")

BugTrimForgetsSizePop ==
  ActualTrimOutput("trim_remove_one") = SpecTrimOutput("trim_remove_one")

BugCanonEmptyMutates ==
  ActualCanonOutput("canon_empty") = SpecCanonOutput("canon_empty")

BugCanonSingleMutates ==
  ActualCanonOutput("canon_single") = SpecCanonOutput("canon_single")

BugCanonSkipSort ==
  ActualCanonOutput("canon_reverse_keys") = SpecCanonOutput("canon_reverse_keys")

BugCanonReverseSort ==
  ActualCanonOutput("canon_already_sorted") =
    SpecCanonOutput("canon_already_sorted")

BugCanonDuplicateUnstable ==
  ActualCanonOutput("canon_duplicate_keys_stable") =
    SpecCanonOutput("canon_duplicate_keys_stable")

BugCanonWrongKeySort ==
  ActualCanonOutput("canon_duplicate_keys_stable") =
    SpecCanonOutput("canon_duplicate_keys_stable")

BugCanonRouteMisaligned ==
  ActualCanonOutput("canon_reverse_keys") = SpecCanonOutput("canon_reverse_keys")

BugCanonPlanMisaligned ==
  ActualCanonOutput("canon_with_plans") = SpecCanonOutput("canon_with_plans")

BugCanonSizeMisaligned ==
  ActualCanonOutput("canon_reverse_keys") = SpecCanonOutput("canon_reverse_keys")

BugCanonLengthChanges ==
  ActualCanonOutput("canon_duplicate_keys_stable") =
    SpecCanonOutput("canon_duplicate_keys_stable")

====
