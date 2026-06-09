---- MODULE SumeragiTipExtensionHelpersGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for pure tip-extension helpers.

This slice pins `pending_block_stale_for_tip(...)` and
`chain_extends_tip(...)`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PendingCases == {
  "pending_missing_committed",
  "pending_next_missing_parent",
  "pending_next_wrong_parent",
  "pending_next_matching_parent",
  "pending_same_height_wrong_parent",
  "pending_future_wrong_parent"
}

ChainCases == {
  "chain_height_below_tip",
  "chain_equal_match",
  "chain_equal_mismatch",
  "chain_child_extends",
  "chain_child_diverges",
  "chain_child_missing_parent",
  "chain_grandchild_extends",
  "chain_grandchild_diverges_at_tip",
  "chain_grandchild_missing_middle",
  "chain_grandchild_missing_tip_parent"
}

BoolToInt(b) == IF b THEN 1 ELSE 0

\* chain result encoding: 1 = Some(true), 0 = Some(false), -1 = None.
SomeTrue == 1
SomeFalse == 0
Unknown == -1

SpecPendingStale(c) ==
  c \in {"pending_next_missing_parent", "pending_next_wrong_parent"}

ActualPendingStale(c) ==
  CASE Bug = "pending_missing_committed_stale"
       /\ c = "pending_missing_committed" -> TRUE
    [] Bug = "pending_missing_parent_live"
       /\ c = "pending_next_missing_parent" -> FALSE
    [] Bug = "pending_wrong_parent_live"
       /\ c = "pending_next_wrong_parent" -> FALSE
    [] Bug = "pending_matching_parent_stale"
       /\ c = "pending_next_matching_parent" -> TRUE
    [] Bug = "pending_same_height_stale"
       /\ c = "pending_same_height_wrong_parent" -> TRUE
    [] Bug = "pending_future_height_stale"
       /\ c = "pending_future_wrong_parent" -> TRUE
    [] OTHER -> SpecPendingStale(c)

SpecPendingOutput(c) ==
  BoolToInt(SpecPendingStale(c))

ActualPendingOutput(c) ==
  BoolToInt(ActualPendingStale(c))

SpecChainResult(c) ==
  CASE c = "chain_height_below_tip" -> SomeFalse
    [] c = "chain_equal_match" -> SomeTrue
    [] c = "chain_equal_mismatch" -> SomeFalse
    [] c = "chain_child_extends" -> SomeTrue
    [] c = "chain_child_diverges" -> SomeFalse
    [] c = "chain_child_missing_parent" -> Unknown
    [] c = "chain_grandchild_extends" -> SomeTrue
    [] c = "chain_grandchild_diverges_at_tip" -> SomeFalse
    [] c = "chain_grandchild_missing_middle" -> Unknown
    [] OTHER -> Unknown

ActualChainResult(c) ==
  CASE Bug = "chain_below_tip_unknown"
       /\ c = "chain_height_below_tip" -> Unknown
    [] Bug = "chain_below_tip_true"
       /\ c = "chain_height_below_tip" -> SomeTrue
    [] Bug = "chain_equal_match_rejected"
       /\ c = "chain_equal_match" -> SomeFalse
    [] Bug = "chain_equal_mismatch_accepted"
       /\ c = "chain_equal_mismatch" -> SomeTrue
    [] Bug = "chain_child_extending_rejected"
       /\ c = "chain_child_extends" -> SomeFalse
    [] Bug = "chain_child_divergent_accepted"
       /\ c = "chain_child_diverges" -> SomeTrue
    [] Bug = "chain_missing_parent_false"
       /\ c = "chain_child_missing_parent" -> SomeFalse
    [] Bug = "chain_missing_parent_true"
       /\ c = "chain_child_missing_parent" -> SomeTrue
    [] Bug = "chain_grandchild_extending_rejected"
       /\ c = "chain_grandchild_extends" -> SomeFalse
    [] Bug = "chain_grandchild_stops_after_one_parent"
       /\ c = "chain_grandchild_extends" -> SomeFalse
    [] Bug = "chain_grandchild_divergent_accepted"
       /\ c = "chain_grandchild_diverges_at_tip" -> SomeTrue
    [] Bug = "chain_grandchild_missing_middle_false"
       /\ c = "chain_grandchild_missing_middle" -> SomeFalse
    [] Bug = "chain_grandchild_missing_tip_false"
       /\ c = "chain_grandchild_missing_tip_parent" -> SomeFalse
    [] OTHER -> SpecChainResult(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "pending_missing_committed_stale",
       "pending_missing_parent_live",
       "pending_wrong_parent_live",
       "pending_matching_parent_stale",
       "pending_same_height_stale",
       "pending_future_height_stale",
       "chain_below_tip_unknown",
       "chain_below_tip_true",
       "chain_equal_match_rejected",
       "chain_equal_mismatch_accepted",
       "chain_child_extending_rejected",
       "chain_child_divergent_accepted",
       "chain_missing_parent_false",
       "chain_missing_parent_true",
       "chain_grandchild_extending_rejected",
       "chain_grandchild_stops_after_one_parent",
       "chain_grandchild_divergent_accepted",
       "chain_grandchild_missing_middle_false",
       "chain_grandchild_missing_tip_false"
     }
  /\ checked = 0

TipExtensionHelpersMatchSpec ==
  /\ \A c \in PendingCases:
       ActualPendingOutput(c) = SpecPendingOutput(c)
  /\ \A c \in ChainCases:
       ActualChainResult(c) = SpecChainResult(c)

SafetyFast ==
  TipExtensionHelpersMatchSpec

BugPendingMissingCommittedStale ==
  ActualPendingOutput("pending_missing_committed") =
    SpecPendingOutput("pending_missing_committed")

BugPendingMissingParentLive ==
  ActualPendingOutput("pending_next_missing_parent") =
    SpecPendingOutput("pending_next_missing_parent")

BugPendingWrongParentLive ==
  ActualPendingOutput("pending_next_wrong_parent") =
    SpecPendingOutput("pending_next_wrong_parent")

BugPendingMatchingParentStale ==
  ActualPendingOutput("pending_next_matching_parent") =
    SpecPendingOutput("pending_next_matching_parent")

BugPendingSameHeightStale ==
  ActualPendingOutput("pending_same_height_wrong_parent") =
    SpecPendingOutput("pending_same_height_wrong_parent")

BugPendingFutureHeightStale ==
  ActualPendingOutput("pending_future_wrong_parent") =
    SpecPendingOutput("pending_future_wrong_parent")

BugChainBelowTipUnknown ==
  ActualChainResult("chain_height_below_tip") =
    SpecChainResult("chain_height_below_tip")

BugChainBelowTipTrue ==
  ActualChainResult("chain_height_below_tip") =
    SpecChainResult("chain_height_below_tip")

BugChainEqualMatchRejected ==
  ActualChainResult("chain_equal_match") =
    SpecChainResult("chain_equal_match")

BugChainEqualMismatchAccepted ==
  ActualChainResult("chain_equal_mismatch") =
    SpecChainResult("chain_equal_mismatch")

BugChainChildExtendingRejected ==
  ActualChainResult("chain_child_extends") =
    SpecChainResult("chain_child_extends")

BugChainChildDivergentAccepted ==
  ActualChainResult("chain_child_diverges") =
    SpecChainResult("chain_child_diverges")

BugChainMissingParentFalse ==
  ActualChainResult("chain_child_missing_parent") =
    SpecChainResult("chain_child_missing_parent")

BugChainMissingParentTrue ==
  ActualChainResult("chain_child_missing_parent") =
    SpecChainResult("chain_child_missing_parent")

BugChainGrandchildExtendingRejected ==
  ActualChainResult("chain_grandchild_extends") =
    SpecChainResult("chain_grandchild_extends")

BugChainGrandchildStopsAfterOneParent ==
  ActualChainResult("chain_grandchild_extends") =
    SpecChainResult("chain_grandchild_extends")

BugChainGrandchildDivergentAccepted ==
  ActualChainResult("chain_grandchild_diverges_at_tip") =
    SpecChainResult("chain_grandchild_diverges_at_tip")

BugChainGrandchildMissingMiddleFalse ==
  ActualChainResult("chain_grandchild_missing_middle") =
    SpecChainResult("chain_grandchild_missing_middle")

BugChainGrandchildMissingTipFalse ==
  ActualChainResult("chain_grandchild_missing_tip_parent") =
    SpecChainResult("chain_grandchild_missing_tip_parent")

====
