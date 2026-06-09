---- MODULE SumeragiTimingMonitorGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi timing/log cooldown monitors.

This slice captures `TickTimingMonitor::observe_with_thresholds(...)` and
`ProposeAttemptMonitor::should_log(...)`. It abstracts time into representative
boundary cases while preserving observable contracts: tick elapsed time uses
saturating subtraction, the tick start is always recorded, lag/cost thresholds
are inclusive, gap and cost log cooldowns are independent, suppressed logs do
not move cooldown timestamps, below-threshold samples do not arm cooldowns,
custom thresholds are honored, proposal-attempt logging fires immediately on
the first attempt, and both timing monitors resume exactly at cooldown
boundaries.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

TickBelowThreshold == "tick_below_threshold"
TickGapAtThresholdFirst == "tick_gap_at_threshold_first"
TickCostAtThresholdFirst == "tick_cost_at_threshold_first"
TickBothAtThresholdFirst == "tick_both_at_threshold_first"
TickGapWithinCooldown == "tick_gap_within_cooldown"
TickCostWithinCooldown == "tick_cost_within_cooldown"
TickGapAtCooldownBoundary == "tick_gap_at_cooldown_boundary"
TickCostAtCooldownBoundary == "tick_cost_at_cooldown_boundary"
TickBackwardTimeSaturates == "tick_backward_time_saturates"
TickBelowThresholdDoesNotArmCooldown == "tick_below_threshold_does_not_arm_cooldown"
TickGapCostCooldownIndependent == "tick_gap_cost_cooldown_independent"
TickCustomThresholdsHonored == "tick_custom_thresholds_honored"
ProposeFirstAttempt == "propose_first_attempt"
ProposeWithinCooldown == "propose_within_cooldown"
ProposeAtCooldownBoundary == "propose_at_cooldown_boundary"
ProposeBackwardTimeSuppressed == "propose_backward_time_suppressed"
ProposeSuppressionPreservesCooldown == "propose_suppression_preserves_cooldown"

Cases == {
  TickBelowThreshold,
  TickGapAtThresholdFirst,
  TickCostAtThresholdFirst,
  TickBothAtThresholdFirst,
  TickGapWithinCooldown,
  TickCostWithinCooldown,
  TickGapAtCooldownBoundary,
  TickCostAtCooldownBoundary,
  TickBackwardTimeSaturates,
  TickBelowThresholdDoesNotArmCooldown,
  TickGapCostCooldownIndependent,
  TickCustomThresholdsHonored,
  ProposeFirstAttempt,
  ProposeWithinCooldown,
  ProposeAtCooldownBoundary,
  ProposeBackwardTimeSuppressed,
  ProposeSuppressionPreservesCooldown
}

SinceZero == 1
SinceSmall == 2
SinceAtLagThreshold == 3
SinceLarge == 4
CostSmall == 5
CostAtThreshold == 6
CostLarge == 7
GapLog == 8
NoGapLog == 9
CostLog == 10
NoCostLog == 11
LastTickUpdated == 12
LastTickUnchanged == 13
GapCooldownArmed == 14
GapCooldownPreserved == 15
GapCooldownClear == 16
CostCooldownArmed == 17
CostCooldownPreserved == 18
CostCooldownClear == 19
ProposeLog == 20
ProposeNoLog == 21
ProposeCooldownArmed == 22
ProposeCooldownPreserved == 23
ProposeCooldownClear == 24
BoundaryAllowed == 25
SuppressionDidNotMoveCooldown == 26
CustomThresholdUsed == 27
GapCostIndependent == 28

Actions == 1..28

SpecActions(c) ==
  CASE c = TickBelowThreshold ->
      {SinceSmall, CostSmall, NoGapLog, NoCostLog, LastTickUpdated,
       GapCooldownClear, CostCooldownClear}
    [] c = TickGapAtThresholdFirst ->
      {SinceAtLagThreshold, CostSmall, GapLog, NoCostLog, LastTickUpdated,
       GapCooldownArmed, CostCooldownClear, BoundaryAllowed}
    [] c = TickCostAtThresholdFirst ->
      {SinceSmall, CostAtThreshold, NoGapLog, CostLog, LastTickUpdated,
       GapCooldownClear, CostCooldownArmed, BoundaryAllowed}
    [] c = TickBothAtThresholdFirst ->
      {SinceAtLagThreshold, CostAtThreshold, GapLog, CostLog,
       LastTickUpdated, GapCooldownArmed, CostCooldownArmed, BoundaryAllowed,
       GapCostIndependent}
    [] c = TickGapWithinCooldown ->
      {SinceLarge, CostSmall, NoGapLog, NoCostLog, LastTickUpdated,
       GapCooldownPreserved, CostCooldownClear, SuppressionDidNotMoveCooldown}
    [] c = TickCostWithinCooldown ->
      {SinceSmall, CostLarge, NoGapLog, NoCostLog, LastTickUpdated,
       GapCooldownClear, CostCooldownPreserved, SuppressionDidNotMoveCooldown}
    [] c = TickGapAtCooldownBoundary ->
      {SinceLarge, CostSmall, GapLog, NoCostLog, LastTickUpdated,
       GapCooldownArmed, CostCooldownClear, BoundaryAllowed}
    [] c = TickCostAtCooldownBoundary ->
      {SinceSmall, CostLarge, NoGapLog, CostLog, LastTickUpdated,
       GapCooldownClear, CostCooldownArmed, BoundaryAllowed}
    [] c = TickBackwardTimeSaturates ->
      {SinceZero, CostSmall, NoGapLog, NoCostLog, LastTickUpdated,
       GapCooldownClear, CostCooldownClear}
    [] c = TickBelowThresholdDoesNotArmCooldown ->
      {SinceSmall, CostSmall, NoGapLog, NoCostLog, LastTickUpdated,
       GapCooldownClear, CostCooldownClear}
    [] c = TickGapCostCooldownIndependent ->
      {SinceLarge, CostLarge, NoGapLog, CostLog, LastTickUpdated,
       GapCooldownPreserved, CostCooldownArmed, GapCostIndependent}
    [] c = TickCustomThresholdsHonored ->
      {SinceSmall, CostSmall, GapLog, CostLog, LastTickUpdated,
       GapCooldownArmed, CostCooldownArmed, CustomThresholdUsed}
    [] c = ProposeFirstAttempt ->
      {ProposeLog, ProposeCooldownArmed}
    [] c = ProposeWithinCooldown ->
      {ProposeNoLog, ProposeCooldownPreserved, SuppressionDidNotMoveCooldown}
    [] c = ProposeAtCooldownBoundary ->
      {ProposeLog, ProposeCooldownArmed, BoundaryAllowed}
    [] c = ProposeBackwardTimeSuppressed ->
      {ProposeNoLog, ProposeCooldownPreserved, SuppressionDidNotMoveCooldown}
    [] c = ProposeSuppressionPreservesCooldown ->
      {ProposeLog, ProposeCooldownArmed, SuppressionDidNotMoveCooldown,
       BoundaryAllowed}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "gap_threshold_strict"
       /\ c = TickGapAtThresholdFirst ->
      (spec \ {GapLog, GapCooldownArmed, BoundaryAllowed})
        \cup {NoGapLog, GapCooldownClear}
    [] Bug = "cost_threshold_strict"
       /\ c = TickCostAtThresholdFirst ->
      (spec \ {CostLog, CostCooldownArmed, BoundaryAllowed})
        \cup {NoCostLog, CostCooldownClear}
    [] Bug = "gap_cooldown_boundary_suppressed"
       /\ c = TickGapAtCooldownBoundary ->
      (spec \ {GapLog, GapCooldownArmed, BoundaryAllowed})
        \cup {NoGapLog, GapCooldownPreserved}
    [] Bug = "cost_cooldown_boundary_suppressed"
       /\ c = TickCostAtCooldownBoundary ->
      (spec \ {CostLog, CostCooldownArmed, BoundaryAllowed})
        \cup {NoCostLog, CostCooldownPreserved}
    [] Bug = "gap_suppression_moves_cooldown"
       /\ c = TickGapWithinCooldown ->
      (spec \ {GapCooldownPreserved, SuppressionDidNotMoveCooldown})
        \cup {GapCooldownArmed}
    [] Bug = "cost_suppression_moves_cooldown"
       /\ c = TickCostWithinCooldown ->
      (spec \ {CostCooldownPreserved, SuppressionDidNotMoveCooldown})
        \cup {CostCooldownArmed}
    [] Bug = "below_threshold_arms_gap_cooldown"
       /\ c = TickBelowThresholdDoesNotArmCooldown ->
      (spec \ {GapCooldownClear}) \cup {GapCooldownArmed}
    [] Bug = "below_threshold_arms_cost_cooldown"
       /\ c = TickBelowThresholdDoesNotArmCooldown ->
      (spec \ {CostCooldownClear}) \cup {CostCooldownArmed}
    [] Bug = "gap_and_cost_share_cooldown"
       /\ c = TickGapCostCooldownIndependent ->
      (spec \ {CostLog, CostCooldownArmed, GapCostIndependent})
        \cup {NoCostLog, CostCooldownPreserved}
    [] Bug = "custom_thresholds_ignored"
       /\ c = TickCustomThresholdsHonored ->
      (spec \ {GapLog, CostLog, GapCooldownArmed, CostCooldownArmed,
               CustomThresholdUsed})
        \cup {NoGapLog, NoCostLog, GapCooldownClear, CostCooldownClear}
    [] Bug = "since_last_not_saturating"
       /\ c = TickBackwardTimeSaturates ->
      (spec \ {SinceZero}) \cup {SinceLarge}
    [] Bug = "last_tick_not_updated"
       /\ c \in {TickBelowThreshold, TickGapAtThresholdFirst,
                 TickCostAtThresholdFirst, TickBackwardTimeSaturates} ->
      (spec \ {LastTickUpdated}) \cup {LastTickUnchanged}
    [] Bug = "propose_first_suppressed"
       /\ c = ProposeFirstAttempt ->
      (spec \ {ProposeLog, ProposeCooldownArmed})
        \cup {ProposeNoLog, ProposeCooldownClear}
    [] Bug = "propose_boundary_suppressed"
       /\ c = ProposeAtCooldownBoundary ->
      (spec \ {ProposeLog, ProposeCooldownArmed, BoundaryAllowed})
        \cup {ProposeNoLog, ProposeCooldownPreserved}
    [] Bug = "propose_suppression_moves_cooldown"
       /\ c = ProposeWithinCooldown ->
      (spec \ {ProposeCooldownPreserved, SuppressionDidNotMoveCooldown})
        \cup {ProposeCooldownArmed}
    [] Bug = "propose_backward_time_logs"
       /\ c = ProposeBackwardTimeSuppressed ->
      (spec \ {ProposeNoLog, ProposeCooldownPreserved,
               SuppressionDidNotMoveCooldown})
        \cup {ProposeLog, ProposeCooldownArmed}
    [] Bug = "propose_suppressed_attempt_extends_cooldown"
       /\ c = ProposeSuppressionPreservesCooldown ->
      (spec \ {ProposeLog, BoundaryAllowed, SuppressionDidNotMoveCooldown})
        \cup {ProposeNoLog}
    [] OTHER -> spec

Bugs == {
  "none",
  "gap_threshold_strict",
  "cost_threshold_strict",
  "gap_cooldown_boundary_suppressed",
  "cost_cooldown_boundary_suppressed",
  "gap_suppression_moves_cooldown",
  "cost_suppression_moves_cooldown",
  "below_threshold_arms_gap_cooldown",
  "below_threshold_arms_cost_cooldown",
  "gap_and_cost_share_cooldown",
  "custom_thresholds_ignored",
  "since_last_not_saturating",
  "last_tick_not_updated",
  "propose_first_suppressed",
  "propose_boundary_suppressed",
  "propose_suppression_moves_cooldown",
  "propose_backward_time_logs",
  "propose_suppressed_attempt_extends_cooldown"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..1
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

TimingMonitorCoreSafety ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

NoBugInvariant == TimingMonitorCoreSafety

SafetyFast == TimingMonitorCoreSafety

BugGapThresholdStrict == NoBugInvariant
BugCostThresholdStrict == NoBugInvariant
BugGapCooldownBoundarySuppressed == NoBugInvariant
BugCostCooldownBoundarySuppressed == NoBugInvariant
BugGapSuppressionMovesCooldown == NoBugInvariant
BugCostSuppressionMovesCooldown == NoBugInvariant
BugBelowThresholdArmsGapCooldown == NoBugInvariant
BugBelowThresholdArmsCostCooldown == NoBugInvariant
BugGapAndCostShareCooldown == NoBugInvariant
BugCustomThresholdsIgnored == NoBugInvariant
BugSinceLastNotSaturating == NoBugInvariant
BugLastTickNotUpdated == NoBugInvariant
BugProposeFirstSuppressed == NoBugInvariant
BugProposeBoundarySuppressed == NoBugInvariant
BugProposeSuppressionMovesCooldown == NoBugInvariant
BugProposeBackwardTimeLogs == NoBugInvariant
BugProposeSuppressedAttemptExtendsCooldown == NoBugInvariant

====
