---- MODULE SumeragiPendingFastPathTimeoutGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for pending fast-path timeout derivation.

This slice pins `pending_fast_path_timeout_current()` and
`commit_validation_inline_fallback_timeout()`. The pending fast-path timeout
subtracts a 250 ms margin from the commit quorum timeout with saturating
arithmetic. If that result would fall below the 750 ms usable fast-timeout
floor, the helper falls back to half of the quorum timeout with a one
millisecond minimum. DA inline validation then applies its own 750 ms floor
only when DA is enabled.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

FastTimeoutFloor == 750
FastTimeoutMargin == 250
DaInlineValidationFloor == 750
MaxMillis == 10000

Cases == {
  "zero_quorum",
  "one_ms",
  "tiny_even",
  "tiny_odd",
  "below_margin",
  "at_margin",
  "below_combined_floor",
  "one_below_floor_boundary",
  "at_floor_boundary",
  "above_floor_boundary",
  "large_quorum"
}

DaCases == {TRUE, FALSE}

Max(a, b) == IF a >= b THEN a ELSE b
Min(a, b) == IF a <= b THEN a ELSE b
SaturatingSub(a, b) == IF a <= b THEN 0 ELSE a - b

\* @type: Str => Int;
QuorumTimeout(c) ==
  CASE c = "zero_quorum" -> 0
    [] c = "one_ms" -> 1
    [] c = "tiny_even" -> 2
    [] c = "tiny_odd" -> 3
    [] c = "below_margin" -> 200
    [] c = "at_margin" -> FastTimeoutMargin
    [] c = "below_combined_floor" -> 500
    [] c = "one_below_floor_boundary" -> 999
    [] c = "at_floor_boundary" -> FastTimeoutMargin + FastTimeoutFloor
    [] c = "above_floor_boundary" -> 1250
    [] c = "large_quorum" -> 5000
    [] OTHER -> 0

\* @type: Str => Int;
SpecFastMinusMargin(c) == SaturatingSub(QuorumTimeout(c), FastTimeoutMargin)

\* @type: Str => Int;
SpecPendingFastTimeout(c) ==
  LET fastTimeout == SpecFastMinusMargin(c)
  IN IF fastTimeout < FastTimeoutFloor
     THEN Max(QuorumTimeout(c) \div 2, 1)
     ELSE fastTimeout

\* @type: Str => Int;
ActualPendingFastTimeout(c) ==
  CASE Bug = "no_minimum_one"
       /\ c = "zero_quorum" -> QuorumTimeout(c) \div 2
    [] Bug = "missing_margin"
       /\ c = "large_quorum" -> QuorumTimeout(c)
    [] Bug = "floor_boundary_inclusive"
       /\ c = "at_floor_boundary" -> Max(QuorumTimeout(c) \div 2, 1)
    [] Bug = "uses_quorum_for_floor_check"
       /\ c = "one_below_floor_boundary" -> SpecFastMinusMargin(c)
    [] Bug = "below_floor_returns_floor"
       /\ c = "below_combined_floor" -> FastTimeoutFloor
    [] Bug = "division_rounds_up"
       /\ c = "one_below_floor_boundary" -> (QuorumTimeout(c) + 1) \div 2
    [] Bug = "margin_underflows"
       /\ c = "zero_quorum" -> MaxMillis
    [] OTHER -> SpecPendingFastTimeout(c)

\* @type: (Str, Bool) => Int;
SpecCommitValidationInlineFallback(c, daEnabled) ==
  IF daEnabled
  THEN Max(SpecPendingFastTimeout(c), DaInlineValidationFloor)
  ELSE SpecPendingFastTimeout(c)

\* @type: (Str, Bool) => Int;
ActualCommitValidationInlineFallback(c, daEnabled) ==
  CASE Bug = "da_floor_omitted"
       /\ daEnabled
       /\ c = "below_combined_floor" -> ActualPendingFastTimeout(c)
    [] Bug = "da_floor_without_da"
       /\ ~daEnabled
       /\ c = "below_combined_floor" -> Max(ActualPendingFastTimeout(c), DaInlineValidationFloor)
    [] Bug = "da_floor_uses_min"
       /\ daEnabled
       /\ c = "below_combined_floor" -> Min(ActualPendingFastTimeout(c), DaInlineValidationFloor)
    [] Bug = "da_floor_added"
       /\ daEnabled
       /\ c = "at_floor_boundary" -> ActualPendingFastTimeout(c) + DaInlineValidationFloor
    [] OTHER ->
       IF daEnabled
       THEN Max(ActualPendingFastTimeout(c), DaInlineValidationFloor)
       ELSE ActualPendingFastTimeout(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "no_minimum_one",
       "missing_margin",
       "floor_boundary_inclusive",
       "uses_quorum_for_floor_check",
       "below_floor_returns_floor",
       "division_rounds_up",
       "margin_underflows",
       "da_floor_omitted",
       "da_floor_without_da",
       "da_floor_uses_min",
       "da_floor_added"
     }
  /\ checked = 0

PendingFastTimeoutMatchesSpec ==
  /\ \A c \in Cases:
       ActualPendingFastTimeout(c) = SpecPendingFastTimeout(c)

InlineFallbackMatchesSpec ==
  /\ \A c \in Cases:
       \A daEnabled \in DaCases:
         ActualCommitValidationInlineFallback(c, daEnabled)
           = SpecCommitValidationInlineFallback(c, daEnabled)

MinimumOneAnchor ==
  /\ SpecPendingFastTimeout("zero_quorum") = 1
  /\ ActualPendingFastTimeout("zero_quorum") = 1

SaturatingMarginAnchors ==
  /\ SpecFastMinusMargin("zero_quorum") = 0
  /\ SpecFastMinusMargin("below_margin") = 0
  /\ SpecFastMinusMargin("at_margin") = 0

HalfFallbackAnchors ==
  /\ SpecPendingFastTimeout("below_combined_floor") = 250
  /\ SpecPendingFastTimeout("one_below_floor_boundary") = 499
  /\ ActualPendingFastTimeout("below_combined_floor") = 250
  /\ ActualPendingFastTimeout("one_below_floor_boundary") = 499

FloorBoundaryAnchors ==
  /\ SpecPendingFastTimeout("at_floor_boundary") = 750
  /\ ActualPendingFastTimeout("at_floor_boundary") = 750

LargeQuorumMarginAnchor ==
  /\ SpecPendingFastTimeout("large_quorum") = 4750
  /\ ActualPendingFastTimeout("large_quorum") = 4750

DaInlineFallbackAnchors ==
  /\ SpecCommitValidationInlineFallback("below_combined_floor", TRUE) = 750
  /\ SpecCommitValidationInlineFallback("below_combined_floor", FALSE) = 250
  /\ ActualCommitValidationInlineFallback("below_combined_floor", TRUE) = 750
  /\ ActualCommitValidationInlineFallback("below_combined_floor", FALSE) = 250
  /\ ActualCommitValidationInlineFallback("at_floor_boundary", TRUE) = 750

SafetyFast ==
  /\ PendingFastTimeoutMatchesSpec
  /\ InlineFallbackMatchesSpec
  /\ MinimumOneAnchor
  /\ SaturatingMarginAnchors
  /\ HalfFallbackAnchors
  /\ FloorBoundaryAnchors
  /\ LargeQuorumMarginAnchor
  /\ DaInlineFallbackAnchors

BugNoMinimumOne ==
  ActualPendingFastTimeout("zero_quorum") = SpecPendingFastTimeout("zero_quorum")

BugMissingMargin ==
  ActualPendingFastTimeout("large_quorum") = SpecPendingFastTimeout("large_quorum")

BugFloorBoundaryInclusive ==
  ActualPendingFastTimeout("at_floor_boundary") = SpecPendingFastTimeout("at_floor_boundary")

BugUsesQuorumForFloorCheck ==
  ActualPendingFastTimeout("one_below_floor_boundary")
    = SpecPendingFastTimeout("one_below_floor_boundary")

BugBelowFloorReturnsFloor ==
  ActualPendingFastTimeout("below_combined_floor")
    = SpecPendingFastTimeout("below_combined_floor")

BugDivisionRoundsUp ==
  ActualPendingFastTimeout("one_below_floor_boundary")
    = SpecPendingFastTimeout("one_below_floor_boundary")

BugMarginUnderflows ==
  ActualPendingFastTimeout("zero_quorum") = SpecPendingFastTimeout("zero_quorum")

BugDaFloorOmitted ==
  ActualCommitValidationInlineFallback("below_combined_floor", TRUE)
    = SpecCommitValidationInlineFallback("below_combined_floor", TRUE)

BugDaFloorWithoutDa ==
  ActualCommitValidationInlineFallback("below_combined_floor", FALSE)
    = SpecCommitValidationInlineFallback("below_combined_floor", FALSE)

BugDaFloorUsesMin ==
  ActualCommitValidationInlineFallback("below_combined_floor", TRUE)
    = SpecCommitValidationInlineFallback("below_combined_floor", TRUE)

BugDaFloorAdded ==
  ActualCommitValidationInlineFallback("at_floor_boundary", TRUE)
    = SpecCommitValidationInlineFallback("at_floor_boundary", TRUE)

====
