---- MODULE SumeragiPacingGovernorGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for `evaluate_pacing_governor(...)`.

The helper evaluates committed block samples to decide whether the pacemaker
factor should increase under view-change or commit-spacing pressure, decrease
after a clear/stable window, or remain unchanged. The Rust implementation uses
saturating deltas between adjacent samples, integer permille ratios, a
hard lower factor floor of 10_000 bps, max>=min normalization, and suppresses
no-op decisions whose computed next factor equals the caller's input factor.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Str;
  candidate,
  \* @type: Int;
  sample_count,
  \* @type: Int;
  target_block_time_ms,
  \* @type: Int;
  current_factor_bps,
  \* @type: Int;
  spacing_sum,
  \* @type: Int;
  view_change_delta,
  \* @type: Int;
  avg_spacing_ms,
  \* @type: Int;
  commit_spacing_ratio_permille,
  \* @type: Int;
  view_change_ratio_permille,
  \* @type: Int;
  min_factor,
  \* @type: Int;
  max_factor,
  \* @type: Int;
  clamped_current,
  \* @type: Bool;
  decision_present,
  \* @type: Str;
  action,
  \* @type: Int;
  new_factor_bps

\* @type: <<Str, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Int, Bool, Str, Int>>;
vars ==
  <<candidate, sample_count, target_block_time_ms, current_factor_bps,
    spacing_sum, view_change_delta, avg_spacing_ms,
    commit_spacing_ratio_permille, view_change_ratio_permille, min_factor,
    max_factor, clamped_current, decision_present, action, new_factor_bps>>

MaxU32 == 4294967295
FactorFloor == 10000
DefaultViewPressure == 100
DefaultViewClear == 10
DefaultSpacingPressure == 1200
DefaultSpacingClear == 1100
DefaultStepUp == 1000
DefaultStepDown == 100

Cases == {
  "too_few_samples",
  "zero_target",
  "increase_view_pressure",
  "increase_spacing_pressure",
  "increase_both_pressures",
  "decrease_clear",
  "ambiguous_no_change",
  "increase_at_max",
  "decrease_at_min",
  "step_up_clamped",
  "step_down_clamped",
  "min_factor_floor",
  "max_factor_raised_to_min",
  "current_above_max_decrease",
  "spacing_saturates_downward",
  "view_change_saturates_downward",
  "exact_pressure_threshold",
  "exact_clear_threshold"
}

NoDecisionInputCases == {"too_few_samples", "zero_target"}
IncreasePressureCases == {
  "increase_view_pressure",
  "increase_spacing_pressure",
  "increase_both_pressures",
  "step_up_clamped",
  "min_factor_floor",
  "exact_pressure_threshold"
}
IncreaseSuppressedCases == {"increase_at_max", "max_factor_raised_to_min"}
DecreaseCases == {
  "decrease_clear",
  "step_down_clamped",
  "current_above_max_decrease",
  "spacing_saturates_downward",
  "view_change_saturates_downward",
  "exact_clear_threshold"
}
DecreaseSuppressedCases == {"decrease_at_min"}
AmbiguousCases == {"ambiguous_no_change"}
PressureCases == IncreasePressureCases \union IncreaseSuppressedCases
ClearCases == DecreaseCases \union DecreaseSuppressedCases
NoDecisionCases == NoDecisionInputCases \union IncreaseSuppressedCases
  \union DecreaseSuppressedCases \union AmbiguousCases

Min(a, b) == IF a <= b THEN a ELSE b
Max(a, b) == IF a >= b THEN a ELSE b
SatSub(a, b) == IF a >= b THEN a - b ELSE 0
RatioPermille(numerator, denominator) ==
  IF denominator = 0 THEN MaxU32 ELSE (numerator * 1000) \div denominator

SampleCount(c) == IF c = "too_few_samples" THEN 1 ELSE 3
Transitions(c) == IF SampleCount(c) < 2 THEN 0 ELSE SampleCount(c) - 1
TargetBlockTime(c) ==
  CASE c = "zero_target" -> 0
    [] c = "decrease_clear" -> 1200
    [] OTHER -> 1000

CfgMinFactor(c) ==
  CASE c = "step_down_clamped" -> 11500
    [] c = "min_factor_floor" -> 9000
    [] c = "max_factor_raised_to_min" -> 12000
    [] OTHER -> 10000

CfgMaxFactor(c) ==
  CASE c = "max_factor_raised_to_min" -> 10000
    [] OTHER -> 20000

CurrentFactor(c) ==
  CASE c \in {"decrease_clear", "spacing_saturates_downward",
       "view_change_saturates_downward", "exact_clear_threshold"} -> 12000
    [] c = "increase_at_max" -> 20000
    [] c = "decrease_at_min" -> 10000
    [] c = "step_up_clamped" -> 19500
    [] c = "step_down_clamped" -> 11550
    [] c = "min_factor_floor" -> 9500
    [] c = "max_factor_raised_to_min" -> 12000
    [] c = "current_above_max_decrease" -> 25000
    [] OTHER -> 10000

Creation1(c) ==
  CASE c \in {"spacing_saturates_downward"} -> 3000
    [] OTHER -> 1000

Creation2(c) ==
  CASE c \in {"increase_spacing_pressure", "increase_both_pressures",
       "increase_at_max", "step_up_clamped", "min_factor_floor",
       "max_factor_raised_to_min"} -> 2500
    [] c = "decrease_clear" -> 2200
    [] c = "ambiguous_no_change" -> 2150
    [] c = "spacing_saturates_downward" -> 2000
    [] c = "exact_pressure_threshold" -> 2200
    [] c = "exact_clear_threshold" -> 2100
    [] OTHER -> 2000

Creation3(c) ==
  CASE c \in {"increase_spacing_pressure", "increase_both_pressures",
       "increase_at_max", "step_up_clamped", "min_factor_floor",
       "max_factor_raised_to_min"} -> 4000
    [] c = "decrease_clear" -> 3400
    [] c = "ambiguous_no_change" -> 3300
    [] c = "spacing_saturates_downward" -> 1000
    [] c = "exact_pressure_threshold" -> 3400
    [] c = "exact_clear_threshold" -> 3200
    [] OTHER -> 3000

ViewIndex1(c) == IF c = "view_change_saturates_downward" THEN 3 ELSE 0
ViewIndex2(c) ==
  CASE c \in {"increase_view_pressure", "increase_both_pressures"} -> 1
    [] c = "view_change_saturates_downward" -> 2
    [] OTHER -> 0
ViewIndex3(c) ==
  CASE c \in {"increase_view_pressure", "increase_both_pressures"} -> 2
    [] c = "view_change_saturates_downward" -> 1
    [] OTHER -> 0

SpecSpacingSum(c) ==
  IF SampleCount(c) = 3
  THEN SatSub(Creation2(c), Creation1(c))
    + SatSub(Creation3(c), Creation2(c))
  ELSE IF SampleCount(c) = 2
  THEN SatSub(Creation2(c), Creation1(c))
  ELSE 0

SpecViewDelta(c) ==
  IF SampleCount(c) = 3
  THEN SatSub(ViewIndex2(c), ViewIndex1(c))
    + SatSub(ViewIndex3(c), ViewIndex2(c))
  ELSE IF SampleCount(c) = 2
  THEN SatSub(ViewIndex2(c), ViewIndex1(c))
  ELSE 0

SpecAvgSpacing(c) ==
  IF Transitions(c) = 0 THEN 0 ELSE SpecSpacingSum(c) \div Transitions(c)

SpecCommitRatio(c) ==
  RatioPermille(SpecAvgSpacing(c), TargetBlockTime(c))

SpecViewRatio(c) ==
  RatioPermille(SpecViewDelta(c), Transitions(c))

SpecMinFactor(c) == Max(CfgMinFactor(c), FactorFloor)
SpecMaxFactor(c) == Max(CfgMaxFactor(c), SpecMinFactor(c))
SpecCurrent(c) == Min(Max(CurrentFactor(c), SpecMinFactor(c)), SpecMaxFactor(c))
SpecIncrease(c) ==
  SpecViewRatio(c) >= DefaultViewPressure
    \/ SpecCommitRatio(c) >= DefaultSpacingPressure
SpecDecrease(c) ==
  SpecViewRatio(c) <= DefaultViewClear
    /\ SpecCommitRatio(c) <= DefaultSpacingClear
SpecNextUp(c) == Min(SpecCurrent(c) + DefaultStepUp, SpecMaxFactor(c))
SpecNextDown(c) == Max(SpecCurrent(c) - DefaultStepDown, SpecMinFactor(c))

SpecDecisionPresent(c) ==
  IF SampleCount(c) < 2 \/ TargetBlockTime(c) = 0 THEN FALSE
  ELSE IF SpecIncrease(c) /\ SpecCurrent(c) < SpecMaxFactor(c)
  THEN SpecNextUp(c) # CurrentFactor(c)
  ELSE IF SpecDecrease(c) /\ SpecCurrent(c) > SpecMinFactor(c)
  THEN SpecNextDown(c) # CurrentFactor(c)
  ELSE FALSE

SpecAction(c) ==
  IF ~SpecDecisionPresent(c)
  THEN "none"
  ELSE IF SpecIncrease(c) /\ SpecCurrent(c) < SpecMaxFactor(c)
  THEN "increase"
  ELSE "decrease"

SpecNewFactor(c) ==
  CASE SpecAction(c) = "increase" -> SpecNextUp(c)
    [] SpecAction(c) = "decrease" -> SpecNextDown(c)
    [] OTHER -> 0

ActualSpacingSum(c) ==
  CASE Bug = "non_saturating_spacing_delta"
        /\ c = "spacing_saturates_downward" -> 2000
    [] OTHER -> SpecSpacingSum(c)

ActualViewDelta(c) ==
  CASE Bug = "non_saturating_view_delta"
        /\ c = "view_change_saturates_downward" -> 2
    [] OTHER -> SpecViewDelta(c)

ActualAvgSpacing(c) ==
  IF Transitions(c) = 0 THEN 0 ELSE ActualSpacingSum(c) \div Transitions(c)

ActualCommitRatio(c) ==
  RatioPermille(ActualAvgSpacing(c), TargetBlockTime(c))

ActualViewRatio(c) ==
  RatioPermille(ActualViewDelta(c), Transitions(c))

ActualMinFactor(c) ==
  CASE Bug = "min_factor_not_floored" /\ c = "min_factor_floor" ->
         CfgMinFactor(c)
    [] OTHER -> SpecMinFactor(c)

ActualMaxFactor(c) ==
  CASE Bug = "max_factor_not_raised_to_min"
        /\ c = "max_factor_raised_to_min" -> CfgMaxFactor(c)
    [] OTHER -> Max(CfgMaxFactor(c), ActualMinFactor(c))

ActualCurrent(c) ==
  CASE Bug = "use_raw_current_without_clamp"
        /\ c = "current_above_max_decrease" -> CurrentFactor(c)
    [] OTHER ->
         Min(Max(CurrentFactor(c), ActualMinFactor(c)), ActualMaxFactor(c))

ActualIncrease(c) ==
  ActualViewRatio(c) >= DefaultViewPressure
    \/ ActualCommitRatio(c) >= DefaultSpacingPressure

ActualDecrease(c) ==
  ActualViewRatio(c) <= DefaultViewClear
    /\ ActualCommitRatio(c) <= DefaultSpacingClear

ActualNextUp(c) == Min(ActualCurrent(c) + DefaultStepUp, ActualMaxFactor(c))
ActualNextDown(c) == Max(ActualCurrent(c) - DefaultStepDown, ActualMinFactor(c))

DefaultActualDecisionPresent(c) ==
  IF SampleCount(c) < 2 \/ TargetBlockTime(c) = 0 THEN FALSE
  ELSE IF ActualIncrease(c) /\ ActualCurrent(c) < ActualMaxFactor(c)
  THEN ActualNextUp(c) # CurrentFactor(c)
  ELSE IF ActualDecrease(c) /\ ActualCurrent(c) > ActualMinFactor(c)
  THEN ActualNextDown(c) # CurrentFactor(c)
  ELSE FALSE

ActualDecisionPresent(c) ==
  CASE Bug = "allow_too_few_samples" /\ c = "too_few_samples" -> TRUE
    [] Bug = "allow_zero_target" /\ c = "zero_target" -> TRUE
    [] Bug = "skip_view_pressure_increase"
          /\ c = "increase_view_pressure" -> FALSE
    [] Bug = "skip_spacing_pressure_increase"
          /\ c = "increase_spacing_pressure" -> FALSE
    [] Bug = "exact_pressure_not_increase"
          /\ c = "exact_pressure_threshold" -> FALSE
    [] Bug = "skip_clear_decrease" /\ c = "decrease_clear" -> FALSE
    [] Bug = "exact_clear_not_decrease"
          /\ c = "exact_clear_threshold" -> FALSE
    [] Bug = "increase_at_max" /\ c = "increase_at_max" -> TRUE
    [] Bug = "decrease_at_min" /\ c = "decrease_at_min" -> TRUE
    [] Bug = "current_above_max_no_decrease"
          /\ c = "current_above_max_decrease" -> FALSE
    [] Bug \in {"increase_when_ambiguous", "decrease_when_ambiguous"}
          /\ c = "ambiguous_no_change" -> TRUE
    [] OTHER -> DefaultActualDecisionPresent(c)

ActualAction(c) ==
  IF ~ActualDecisionPresent(c)
  THEN "none"
  ELSE CASE Bug \in {"allow_too_few_samples", "allow_zero_target",
           "increase_when_ambiguous", "increase_at_max"}
            /\ c \in {"too_few_samples", "zero_target",
              "ambiguous_no_change", "increase_at_max"} -> "increase"
    [] Bug \in {"decrease_when_ambiguous", "decrease_at_min"}
            /\ c \in {"ambiguous_no_change", "decrease_at_min"} ->
          "decrease"
    [] ActualIncrease(c) /\ ActualCurrent(c) < ActualMaxFactor(c) ->
          "increase"
    [] ActualDecrease(c) /\ ActualCurrent(c) > ActualMinFactor(c) ->
          "decrease"
    [] OTHER -> "increase"

ActualNewFactor(c) ==
  IF ~ActualDecisionPresent(c)
  THEN 0
  ELSE CASE Bug = "step_up_not_clamped" /\ c = "step_up_clamped" ->
          ActualCurrent(c) + DefaultStepUp
    [] Bug = "step_down_not_clamped" /\ c = "step_down_clamped" ->
          ActualCurrent(c) - DefaultStepDown
    [] ActualAction(c) = "increase" -> ActualNextUp(c)
    [] ActualAction(c) = "decrease" -> ActualNextDown(c)
    [] OTHER -> ActualNextUp(c)

Init ==
  /\ candidate \in Cases
  /\ sample_count = SampleCount(candidate)
  /\ target_block_time_ms = TargetBlockTime(candidate)
  /\ current_factor_bps = CurrentFactor(candidate)
  /\ spacing_sum = ActualSpacingSum(candidate)
  /\ view_change_delta = ActualViewDelta(candidate)
  /\ avg_spacing_ms = ActualAvgSpacing(candidate)
  /\ commit_spacing_ratio_permille = ActualCommitRatio(candidate)
  /\ view_change_ratio_permille = ActualViewRatio(candidate)
  /\ min_factor = ActualMinFactor(candidate)
  /\ max_factor = ActualMaxFactor(candidate)
  /\ clamped_current = ActualCurrent(candidate)
  /\ decision_present = ActualDecisionPresent(candidate)
  /\ action = ActualAction(candidate)
  /\ new_factor_bps = ActualNewFactor(candidate)

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "allow_too_few_samples",
       "allow_zero_target",
       "skip_view_pressure_increase",
       "skip_spacing_pressure_increase",
       "exact_pressure_not_increase",
       "skip_clear_decrease",
       "exact_clear_not_decrease",
       "increase_at_max",
       "decrease_at_min",
       "step_up_not_clamped",
       "step_down_not_clamped",
       "min_factor_not_floored",
       "max_factor_not_raised_to_min",
       "current_above_max_no_decrease",
       "non_saturating_spacing_delta",
       "non_saturating_view_delta",
       "use_raw_current_without_clamp",
       "increase_when_ambiguous",
       "decrease_when_ambiguous"
     }
  /\ candidate \in Cases
  /\ sample_count \in 0..3
  /\ target_block_time_ms \in 0..1200
  /\ current_factor_bps \in 0..25000
  /\ spacing_sum \in 0..4000
  /\ view_change_delta \in 0..4
  /\ avg_spacing_ms \in 0..4000
  /\ commit_spacing_ratio_permille \in 0..MaxU32
  /\ view_change_ratio_permille \in 0..MaxU32
  /\ min_factor \in 0..25000
  /\ max_factor \in 0..25000
  /\ clamped_current \in 0..25000
  /\ decision_present \in BOOLEAN
  /\ action \in {"none", "increase", "decrease"}
  /\ new_factor_bps \in 0..26000

SpacingSumMatchesSpec ==
  spacing_sum = SpecSpacingSum(candidate)

ViewChangeDeltaMatchesSpec ==
  view_change_delta = SpecViewDelta(candidate)

AverageSpacingMatchesSpec ==
  avg_spacing_ms = SpecAvgSpacing(candidate)

CommitRatioMatchesSpec ==
  commit_spacing_ratio_permille = SpecCommitRatio(candidate)

ViewRatioMatchesSpec ==
  view_change_ratio_permille = SpecViewRatio(candidate)

MinFactorMatchesSpec ==
  min_factor = SpecMinFactor(candidate)

MaxFactorMatchesSpec ==
  max_factor = SpecMaxFactor(candidate)

ClampedCurrentMatchesSpec ==
  clamped_current = SpecCurrent(candidate)

DecisionPresenceMatchesSpec ==
  decision_present = SpecDecisionPresent(candidate)

ActionMatchesSpec ==
  action = SpecAction(candidate)

NewFactorMatchesSpec ==
  new_factor_bps = SpecNewFactor(candidate)

InvalidInputsDoNotDecide ==
  candidate \in NoDecisionInputCases =>
    /\ ~decision_present
    /\ action = "none"
    /\ new_factor_bps = 0

PressureWindowsIncreaseWhenBelowMax ==
  candidate \in IncreasePressureCases =>
    /\ decision_present
    /\ action = "increase"
    /\ new_factor_bps > clamped_current

ClearWindowsDecreaseWhenAboveMin ==
  candidate \in DecreaseCases =>
    /\ decision_present
    /\ action = "decrease"
    /\ new_factor_bps < clamped_current

AmbiguousWindowDoesNotDecide ==
  candidate \in AmbiguousCases =>
    /\ ~decision_present
    /\ action = "none"

BoundsSuppressNoopDecisions ==
  candidate \in (IncreaseSuppressedCases \union DecreaseSuppressedCases) =>
    /\ ~decision_present
    /\ action = "none"
    /\ new_factor_bps = 0

StepUpIsClampedToMax ==
  candidate = "step_up_clamped" =>
    /\ new_factor_bps = max_factor
    /\ new_factor_bps <= max_factor

StepDownIsClampedToMin ==
  candidate = "step_down_clamped" =>
    /\ new_factor_bps = min_factor
    /\ new_factor_bps >= min_factor

MinFactorUsesHardFloor ==
  candidate = "min_factor_floor" =>
    /\ min_factor = FactorFloor
    /\ clamped_current = FactorFloor

MaxFactorIsAtLeastMinFactor ==
  candidate = "max_factor_raised_to_min" =>
    /\ max_factor = min_factor
    /\ ~decision_present

DownwardSampleDeltasSaturateAtZero ==
  candidate \in {"spacing_saturates_downward",
      "view_change_saturates_downward"} =>
    /\ spacing_sum = SpecSpacingSum(candidate)
    /\ view_change_delta = SpecViewDelta(candidate)

CurrentAboveMaxStillUsesClampedDecision ==
  candidate = "current_above_max_decrease" =>
    /\ clamped_current = SpecMaxFactor(candidate)
    /\ decision_present
    /\ action = "decrease"
    /\ new_factor_bps = SpecMaxFactor(candidate) - DefaultStepDown

DecisionRequiresFactorChange ==
  decision_present => new_factor_bps # current_factor_bps

DecisionStaysWithinNormalizedBounds ==
  decision_present =>
    /\ new_factor_bps >= min_factor
    /\ new_factor_bps <= max_factor

Safety ==
  /\ SpacingSumMatchesSpec
  /\ ViewChangeDeltaMatchesSpec
  /\ AverageSpacingMatchesSpec
  /\ CommitRatioMatchesSpec
  /\ ViewRatioMatchesSpec
  /\ MinFactorMatchesSpec
  /\ MaxFactorMatchesSpec
  /\ ClampedCurrentMatchesSpec
  /\ DecisionPresenceMatchesSpec
  /\ ActionMatchesSpec
  /\ NewFactorMatchesSpec
  /\ InvalidInputsDoNotDecide
  /\ PressureWindowsIncreaseWhenBelowMax
  /\ ClearWindowsDecreaseWhenAboveMin
  /\ AmbiguousWindowDoesNotDecide
  /\ BoundsSuppressNoopDecisions
  /\ StepUpIsClampedToMax
  /\ StepDownIsClampedToMin
  /\ MinFactorUsesHardFloor
  /\ MaxFactorIsAtLeastMinFactor
  /\ DownwardSampleDeltasSaturateAtZero
  /\ CurrentAboveMaxStillUsesClampedDecision
  /\ DecisionRequiresFactorChange
  /\ DecisionStaysWithinNormalizedBounds

=============================================================================
