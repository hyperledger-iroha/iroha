---- MODULE SumeragiBlockSyncFutureWindowGate ----
EXTENDS Integers

(***************************************************************************
A bounded abstract model for `should_drop_future_block_sync_update(...)` and
the `BlockSyncUpdate` use of `should_drop_future_consensus_message(...)`.

The helper drops known-useless sparse future updates while preserving bounded
requested recovery and locally connected chains. Its order matters:
known local blocks bypass all gates, requested missing-block recovery is bounded
before parent availability can short-circuit, unresolved lower missing heights
drop far-ahead sparse updates before parent availability, then known parents can
admit connected chains before the generic future height/view window runs.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

MaxHeight == 9

Cases == {
  "known_block",
  "requested_within_margin",
  "requested_far",
  "requested_far_known_parent",
  "requested_saturated_boundary",
  "unrequested_lower_missing_far",
  "unrequested_lower_missing_far_known_parent",
  "unrequested_lower_missing_same_height",
  "unrequested_known_parent_far",
  "unrequested_parent_before_view_gate",
  "unrequested_generic_height_drop",
  "unrequested_generic_height_boundary",
  "unrequested_generic_windows_disabled",
  "unrequested_generic_view_drop",
  "unrequested_generic_view_boundary",
  "unrequested_generic_view_age_expired",
  "unrequested_generic_no_phase_view"
}

KnownBlock(c) == c = "known_block"

RequestedMissingBlock(c) ==
  c \in {
    "requested_within_margin",
    "requested_far",
    "requested_far_known_parent",
    "requested_saturated_boundary"
  }

ParentAvailable(c) ==
  c \in {
    "requested_far_known_parent",
    "unrequested_lower_missing_far_known_parent",
    "unrequested_known_parent_far",
    "unrequested_parent_before_view_gate"
  }

LowerMissingKnown(c) ==
  c \in {
    "unrequested_lower_missing_far",
    "unrequested_lower_missing_far_known_parent",
    "unrequested_lower_missing_same_height"
  }

LocalHeight(c) ==
  IF c = "requested_saturated_boundary" THEN MaxHeight ELSE 3

MissingMarginRaw(c) ==
  CASE c = "requested_within_margin" -> 3
    [] c = "requested_saturated_boundary" -> 0
    [] c \in {
         "unrequested_generic_height_drop",
         "unrequested_generic_height_boundary",
         "unrequested_generic_windows_disabled",
         "unrequested_generic_view_drop",
         "unrequested_generic_view_boundary",
         "unrequested_generic_view_age_expired",
         "unrequested_generic_no_phase_view",
         "unrequested_parent_before_view_gate",
         "unrequested_lower_missing_same_height"
       } -> 8
    [] OTHER -> 1

Max(a, b) == IF a >= b THEN a ELSE b

SatAdd(a, b) ==
  IF a + b > MaxHeight THEN MaxHeight ELSE a + b

RequestedMargin(c) == Max(MissingMarginRaw(c), 1)

Height(c) ==
  CASE c = "requested_within_margin" -> 6
    [] c = "requested_saturated_boundary" -> MaxHeight
    [] c = "unrequested_generic_height_boundary" -> 4
    [] c \in {
         "unrequested_generic_view_drop",
         "unrequested_generic_view_boundary",
         "unrequested_generic_view_age_expired",
         "unrequested_generic_no_phase_view",
         "unrequested_parent_before_view_gate"
       } -> 3
    [] OTHER -> 6

View(c) ==
  CASE c = "unrequested_generic_view_boundary" -> 1
    [] c \in {
         "unrequested_generic_view_drop",
         "unrequested_generic_view_age_expired",
         "unrequested_generic_no_phase_view",
         "unrequested_parent_before_view_gate"
       } -> 2
    [] OTHER -> 0

MissingHeight(c) ==
  CASE c = "unrequested_lower_missing_same_height" -> Height(c)
    [] OTHER -> 4

FarAheadByCommitted(c) ==
  Height(c) > SatAdd(LocalHeight(c), RequestedMargin(c))

LowerUnresolvedMissing(c) ==
  LowerMissingKnown(c) /\ MissingHeight(c) < Height(c)

BaseHeight(c) == 3

HeightWindow(c) ==
  CASE c = "unrequested_generic_windows_disabled" -> 0
    [] c = "unrequested_lower_missing_same_height" -> 8
    [] c \in {
         "unrequested_generic_view_drop",
         "unrequested_generic_view_boundary",
         "unrequested_generic_view_age_expired",
         "unrequested_generic_no_phase_view",
         "unrequested_parent_before_view_gate"
       } -> 0
    [] OTHER -> 1

ViewWindow(c) ==
  CASE c \in {
         "unrequested_generic_view_drop",
         "unrequested_generic_view_boundary",
         "unrequested_generic_view_age_expired",
         "unrequested_generic_no_phase_view",
         "unrequested_parent_before_view_gate"
       } -> 1
    [] OTHER -> 0

BaseViewKnown(c) == c # "unrequested_generic_no_phase_view"

BaseView(c) == 0

ViewAgeExpired(c) == c = "unrequested_generic_view_age_expired"

SpecGenericDrop(c) ==
  IF HeightWindow(c) = 0 /\ ViewWindow(c) = 0 THEN
    FALSE
  ELSE IF HeightWindow(c) # 0 /\ Height(c) > SatAdd(BaseHeight(c), HeightWindow(c)) THEN
    TRUE
  ELSE IF ViewWindow(c) = 0 THEN
    FALSE
  ELSE IF Height(c) # BaseHeight(c) THEN
    FALSE
  ELSE IF ~BaseViewKnown(c) THEN
    FALSE
  ELSE IF ViewAgeExpired(c) THEN
    FALSE
  ELSE
    View(c) > SatAdd(BaseView(c), ViewWindow(c))

SpecDrop(c) ==
  IF KnownBlock(c) THEN
    FALSE
  ELSE IF RequestedMissingBlock(c) THEN
    FarAheadByCommitted(c)
  ELSE IF LowerUnresolvedMissing(c) /\ FarAheadByCommitted(c) THEN
    TRUE
  ELSE IF ParentAvailable(c) THEN
    FALSE
  ELSE
    SpecGenericDrop(c)

ActualGenericDrop(c) ==
  CASE Bug = "generic_windows_disabled_drops"
       /\ c = "unrequested_generic_windows_disabled" -> TRUE
    [] Bug = "generic_height_gate_skipped"
       /\ c = "unrequested_generic_height_drop" -> FALSE
    [] Bug = "generic_height_boundary_inclusive"
       /\ c = "unrequested_generic_height_boundary" -> TRUE
    [] Bug = "generic_view_gate_skipped"
       /\ c = "unrequested_generic_view_drop" -> FALSE
    [] Bug = "generic_view_boundary_inclusive"
       /\ c = "unrequested_generic_view_boundary" -> TRUE
    [] Bug = "generic_view_age_ignored"
       /\ c = "unrequested_generic_view_age_expired" -> TRUE
    [] Bug = "generic_missing_phase_view_drops"
       /\ c = "unrequested_generic_no_phase_view" -> TRUE
    [] OTHER -> SpecGenericDrop(c)

ActualDrop(c) ==
  CASE Bug = "known_block_dropped"
       /\ c = "known_block" -> TRUE
    [] Bug = "requested_within_margin_dropped"
       /\ c = "requested_within_margin" -> TRUE
    [] Bug = "requested_far_allowed"
       /\ c = "requested_far" -> FALSE
    [] Bug = "requested_far_parent_short_circuits"
       /\ c = "requested_far_known_parent" -> FALSE
    [] Bug = "requested_saturated_boundary_dropped"
       /\ c = "requested_saturated_boundary" -> TRUE
    [] Bug = "lower_missing_far_allowed"
       /\ c = "unrequested_lower_missing_far" -> FALSE
    [] Bug = "lower_missing_after_parent_shortcut"
       /\ c = "unrequested_lower_missing_far_known_parent" -> FALSE
    [] Bug = "lower_missing_inclusive"
       /\ c = "unrequested_lower_missing_same_height" -> TRUE
    [] Bug = "known_parent_ignored"
       /\ c = "unrequested_known_parent_far" -> TRUE
    [] Bug = "parent_shortcut_after_generic"
       /\ c = "unrequested_parent_before_view_gate" -> TRUE
    [] OTHER ->
       IF KnownBlock(c) THEN
         FALSE
       ELSE IF RequestedMissingBlock(c) THEN
         FarAheadByCommitted(c)
       ELSE IF LowerUnresolvedMissing(c) /\ FarAheadByCommitted(c) THEN
         TRUE
       ELSE IF ParentAvailable(c) THEN
         FALSE
       ELSE
         ActualGenericDrop(c)

Matches(c) ==
  /\ ActualGenericDrop(c) = SpecGenericDrop(c)
  /\ ActualDrop(c) = SpecDrop(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "known_block_dropped",
       "requested_within_margin_dropped",
       "requested_far_allowed",
       "requested_far_parent_short_circuits",
       "requested_saturated_boundary_dropped",
       "lower_missing_far_allowed",
       "lower_missing_after_parent_shortcut",
       "lower_missing_inclusive",
       "known_parent_ignored",
       "parent_shortcut_after_generic",
       "generic_windows_disabled_drops",
       "generic_height_gate_skipped",
       "generic_height_boundary_inclusive",
       "generic_view_gate_skipped",
       "generic_view_boundary_inclusive",
       "generic_view_age_ignored",
       "generic_missing_phase_view_drops"
     }
  /\ checked = 0
  /\ MaxHeight = 9

FutureWindowMatchesSpec ==
  \A c \in Cases: Matches(c)

BlockSyncFutureWindowExactness ==
  /\ FutureWindowMatchesSpec

BlockSyncFutureWindowCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ BlockSyncFutureWindowExactness

SafetyFast == BlockSyncFutureWindowExactness

KnownBlockAllowed ==
  Matches("known_block")

RequestedWithinMarginAllowed ==
  Matches("requested_within_margin")

RequestedFarDropped ==
  Matches("requested_far")

RequestedFarKnownParentDropped ==
  Matches("requested_far_known_parent")

RequestedSaturatedBoundaryAllowed ==
  Matches("requested_saturated_boundary")

LowerMissingFarDropped ==
  Matches("unrequested_lower_missing_far")

LowerMissingFarKnownParentDropped ==
  Matches("unrequested_lower_missing_far_known_parent")

LowerMissingSameHeightAllowed ==
  Matches("unrequested_lower_missing_same_height")

KnownParentAllowed ==
  Matches("unrequested_known_parent_far")

ParentBeforeViewGateAllowed ==
  Matches("unrequested_parent_before_view_gate")

GenericWindowsDisabledAllowed ==
  Matches("unrequested_generic_windows_disabled")

GenericHeightDrop ==
  Matches("unrequested_generic_height_drop")

GenericHeightBoundaryAllowed ==
  Matches("unrequested_generic_height_boundary")

GenericViewDrop ==
  Matches("unrequested_generic_view_drop")

GenericViewBoundaryAllowed ==
  Matches("unrequested_generic_view_boundary")

GenericViewAgeExpiredAllowed ==
  Matches("unrequested_generic_view_age_expired")

GenericMissingPhaseViewAllowed ==
  Matches("unrequested_generic_no_phase_view")

====
