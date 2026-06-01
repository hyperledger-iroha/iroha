---- MODULE SumeragiCompletedQuorumViewAdvanceGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for
`advance_view_after_completed_quorum_reschedule(...)` and the exact-frontier
`OnViewAdvanceRequested` slot event it delegates to.

The helper must route exact contiguous-frontier heights through the frontier
slot path so exhausted reassembly work cannot re-enter the generic repair
suppression gate. Non-exact heights still use the generic view-change trigger.
For exact slots, the slot event preserves the cause, chooses
`max(active_view, requested_view, candidate_view)`, advances the active view by
one with saturation, updates progress timestamps, and clears the
quorum-timeout rebroadcast latch.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ExactRequestedDominates == "ExactRequestedDominates"
ExactActiveDominates == "ExactActiveDominates"
ExactCandidateDominates == "ExactCandidateDominates"
ExactSaturatingIncrement == "ExactSaturatingIncrement"
ExactClearsRebroadcast == "ExactClearsRebroadcast"
ExactUpdatesTimestamps == "ExactUpdatesTimestamps"
ExactCausePreserved == "ExactCausePreserved"
ExactNoSlotFallback == "ExactNoSlotFallback"
ExactStaleSlotFallback == "ExactStaleSlotFallback"
LowerHeightGeneric == "LowerHeightGeneric"
FutureHeightGeneric == "FutureHeightGeneric"
GenericPreservesSlotState == "GenericPreservesSlotState"
GenericCausePreserved == "GenericCausePreserved"
NonExactNoSlotGeneric == "NonExactNoSlotGeneric"

Cases == {
  ExactRequestedDominates,
  ExactActiveDominates,
  ExactCandidateDominates,
  ExactSaturatingIncrement,
  ExactClearsRebroadcast,
  ExactUpdatesTimestamps,
  ExactCausePreserved,
  ExactNoSlotFallback,
  ExactStaleSlotFallback,
  LowerHeightGeneric,
  FutureHeightGeneric,
  GenericPreservesSlotState,
  GenericCausePreserved,
  NonExactNoSlotGeneric
}

RouteSlotApply == 1
RouteGenericTrigger == 2
CauseQuorumTimeout == 1
CauseStakeQuorumTimeout == 2
OwnerExactSlotRepair == 1
OwnerOther == 2
NoSlotState == 0
MaxView == 5

Max(a, b) == IF a >= b THEN a ELSE b
BoolToInt(b) == IF b THEN 1 ELSE 0
SatAddOne(v) == IF v >= MaxView THEN MaxView ELSE v + 1

CommittedHeight(c) == 2
FrontierHeight(c) == CommittedHeight(c) + 1

InputHeight(c) ==
  CASE c = LowerHeightGeneric -> CommittedHeight(c)
    [] c \in {
         FutureHeightGeneric,
         GenericPreservesSlotState,
         GenericCausePreserved,
         NonExactNoSlotGeneric
       } ->
       FrontierHeight(c) + 1
    [] OTHER -> FrontierHeight(c)

ExactHeight(c) ==
  InputHeight(c) = FrontierHeight(c)

RequestedView(c) ==
  CASE c = ExactRequestedDominates -> 4
    [] c = ExactSaturatingIncrement -> MaxView
    [] OTHER -> 2

CauseInput(c) ==
  IF c \in {ExactCausePreserved, GenericCausePreserved}
  THEN CauseStakeQuorumTimeout
  ELSE CauseQuorumTimeout

SlotPresentBefore(c) ==
  c \notin {ExactNoSlotFallback, NonExactNoSlotGeneric}

SlotStale(c) ==
  c = ExactStaleSlotFallback

SlotHeight(c) ==
  IF SlotStale(c) THEN FrontierHeight(c) + 1 ELSE FrontierHeight(c)

ActiveViewBefore(c) ==
  CASE c = ExactActiveDominates -> 4
    [] c = ExactSaturatingIncrement -> MaxView
    [] c = GenericPreservesSlotState -> 3
    [] OTHER -> 1

CandidateView(c) ==
  CASE c = ExactCandidateDominates -> 4
    [] c = ExactSaturatingIncrement -> MaxView
    [] OTHER -> 2

OwnerKindBefore(c) ==
  IF SlotPresentBefore(c) THEN OwnerOther ELSE NoSlotState

RebroadcastBefore(c) ==
  SlotPresentBefore(c)

UsableSlot(c) ==
  /\ ExactHeight(c)
  /\ SlotPresentBefore(c)
  /\ ~SlotStale(c)

SpecRoute(c) ==
  IF ExactHeight(c) THEN RouteSlotApply ELSE RouteGenericTrigger

SpecCurrentView(c) ==
  Max(Max(ActiveViewBefore(c), RequestedView(c)), CandidateView(c))

SpecSlotPresentAfter(c) ==
  IF ExactHeight(c) THEN UsableSlot(c) ELSE SlotPresentBefore(c)

SpecAppliedHeight(c) ==
  IF SpecRoute(c) = RouteSlotApply
  THEN FrontierHeight(c)
  ELSE InputHeight(c)

SpecAppliedView(c) ==
  IF SpecRoute(c) = RouteSlotApply
  THEN IF UsableSlot(c) THEN SpecCurrentView(c) ELSE RequestedView(c)
  ELSE RequestedView(c)

SpecOwnerKindAfter(c) ==
  IF ~SpecSlotPresentAfter(c) THEN NoSlotState
  ELSE IF SpecRoute(c) = RouteSlotApply /\ UsableSlot(c)
  THEN OwnerExactSlotRepair
  ELSE OwnerKindBefore(c)

SpecActiveViewAfter(c) ==
  IF ~SpecSlotPresentAfter(c) THEN NoSlotState
  ELSE IF SpecRoute(c) = RouteSlotApply /\ UsableSlot(c)
  THEN SatAddOne(SpecCurrentView(c))
  ELSE ActiveViewBefore(c)

SpecLastAdvanceUpdated(c) ==
  SpecRoute(c) = RouteSlotApply /\ UsableSlot(c)

SpecLastUpdatedAtUpdated(c) ==
  SpecRoute(c) = RouteSlotApply /\ UsableSlot(c)

SpecRebroadcastAfter(c) ==
  IF ~SpecSlotPresentAfter(c) THEN FALSE
  ELSE IF SpecRoute(c) = RouteSlotApply /\ UsableSlot(c)
  THEN FALSE
  ELSE RebroadcastBefore(c)

\* @type: (Str) => <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>;
SpecOutput(c) ==
  <<SpecRoute(c), SpecAppliedHeight(c), SpecAppliedView(c), CauseInput(c),
    BoolToInt(SpecSlotPresentAfter(c)), SpecOwnerKindAfter(c),
    SpecActiveViewAfter(c), BoolToInt(SpecLastAdvanceUpdated(c)),
    BoolToInt(SpecLastUpdatedAtUpdated(c)), BoolToInt(SpecRebroadcastAfter(c))>>

ActualRoute(c) ==
  CASE Bug = "exact_uses_generic_trigger"
       /\ c = ExactRequestedDominates -> RouteGenericTrigger
    [] Bug = "nonexact_uses_slot_event"
       /\ c = FutureHeightGeneric -> RouteSlotApply
    [] Bug = "no_slot_exact_uses_generic"
       /\ c = ExactNoSlotFallback -> RouteGenericTrigger
    [] OTHER -> SpecRoute(c)

ActualTreatsSlotAsUsable(c) ==
  CASE Bug = "stale_slot_not_dropped"
       /\ c = ExactStaleSlotFallback -> TRUE
    [] ActualRoute(c) = RouteSlotApply /\ ~ExactHeight(c) ->
       SlotPresentBefore(c)
    [] OTHER -> UsableSlot(c)

ActualCurrentView(c) ==
  CASE Bug = "current_view_ignores_requested"
       /\ c = ExactRequestedDominates ->
       Max(ActiveViewBefore(c), CandidateView(c))
    [] Bug = "current_view_ignores_active"
       /\ c = ExactActiveDominates ->
       Max(RequestedView(c), CandidateView(c))
    [] Bug = "current_view_ignores_candidate"
       /\ c = ExactCandidateDominates ->
       Max(ActiveViewBefore(c), RequestedView(c))
    [] OTHER -> SpecCurrentView(c)

ActualSlotPresentAfter(c) ==
  IF ActualRoute(c) = RouteSlotApply
  THEN ActualTreatsSlotAsUsable(c)
  ELSE SlotPresentBefore(c)

ActualAppliedHeight(c) ==
  CASE Bug = "generic_wrong_height"
       /\ c = LowerHeightGeneric -> FrontierHeight(c)
    [] ActualRoute(c) = RouteSlotApply /\ ActualTreatsSlotAsUsable(c) ->
       SlotHeight(c)
    [] ActualRoute(c) = RouteSlotApply -> FrontierHeight(c)
    [] OTHER -> InputHeight(c)

ActualAppliedView(c) ==
  CASE Bug = "generic_wrong_view"
       /\ c = FutureHeightGeneric -> RequestedView(c) + 1
    [] ActualRoute(c) = RouteSlotApply /\ ActualTreatsSlotAsUsable(c) ->
       ActualCurrentView(c)
    [] ActualRoute(c) = RouteSlotApply -> RequestedView(c)
    [] OTHER -> RequestedView(c)

ActualCause(c) ==
  IF Bug = "cause_lost" /\ c \in {ExactCausePreserved, GenericCausePreserved}
  THEN CauseQuorumTimeout
  ELSE CauseInput(c)

ActualOwnerKindAfter(c) ==
  IF ~ActualSlotPresentAfter(c) THEN NoSlotState
  ELSE IF ActualRoute(c) = RouteSlotApply /\ ActualTreatsSlotAsUsable(c)
  THEN OwnerExactSlotRepair
  ELSE IF Bug = "generic_mutates_slot" /\ c = GenericPreservesSlotState
  THEN OwnerExactSlotRepair
  ELSE OwnerKindBefore(c)

ActualActiveViewAfter(c) ==
  IF ~ActualSlotPresentAfter(c) THEN NoSlotState
  ELSE IF ActualRoute(c) = RouteSlotApply /\ ActualTreatsSlotAsUsable(c)
  THEN
    CASE Bug = "active_view_not_incremented"
         /\ c = ExactActiveDominates -> ActualCurrentView(c)
      [] Bug = "active_view_wraps_at_max"
         /\ c = ExactSaturatingIncrement -> 0
      [] OTHER -> SatAddOne(ActualCurrentView(c))
  ELSE IF Bug = "generic_mutates_slot" /\ c = GenericPreservesSlotState
  THEN SatAddOne(ActiveViewBefore(c))
  ELSE ActiveViewBefore(c)

ActualLastAdvanceUpdated(c) ==
  IF ActualRoute(c) = RouteSlotApply /\ ActualTreatsSlotAsUsable(c)
  THEN ~(Bug = "timestamp_not_updated" /\ c = ExactUpdatesTimestamps)
  ELSE Bug = "generic_mutates_slot" /\ c = GenericPreservesSlotState

ActualLastUpdatedAtUpdated(c) ==
  IF ActualRoute(c) = RouteSlotApply /\ ActualTreatsSlotAsUsable(c)
  THEN ~(Bug = "timestamp_not_updated" /\ c = ExactUpdatesTimestamps)
  ELSE Bug = "generic_mutates_slot" /\ c = GenericPreservesSlotState

ActualRebroadcastAfter(c) ==
  IF ~ActualSlotPresentAfter(c) THEN FALSE
  ELSE IF ActualRoute(c) = RouteSlotApply /\ ActualTreatsSlotAsUsable(c)
  THEN Bug = "rebroadcast_not_cleared" /\ c = ExactClearsRebroadcast
  ELSE IF Bug = "generic_mutates_slot" /\ c = GenericPreservesSlotState
  THEN FALSE
  ELSE RebroadcastBefore(c)

\* @type: (Str) => <<Int, Int, Int, Int, Int, Int, Int, Int, Int, Int>>;
ActualOutput(c) ==
  <<ActualRoute(c), ActualAppliedHeight(c), ActualAppliedView(c),
    ActualCause(c), BoolToInt(ActualSlotPresentAfter(c)),
    ActualOwnerKindAfter(c), ActualActiveViewAfter(c),
    BoolToInt(ActualLastAdvanceUpdated(c)),
    BoolToInt(ActualLastUpdatedAtUpdated(c)),
    BoolToInt(ActualRebroadcastAfter(c))>>

BugSet == {
  "none",
  "exact_uses_generic_trigger",
  "nonexact_uses_slot_event",
  "no_slot_exact_uses_generic",
  "stale_slot_not_dropped",
  "current_view_ignores_requested",
  "current_view_ignores_active",
  "current_view_ignores_candidate",
  "active_view_not_incremented",
  "active_view_wraps_at_max",
  "rebroadcast_not_cleared",
  "timestamp_not_updated",
  "cause_lost",
  "generic_wrong_height",
  "generic_wrong_view",
  "generic_mutates_slot"
}

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in BugSet
  /\ checked = 0

SelectionExact ==
  \A c \in Cases:
    ActualOutput(c) = SpecOutput(c)

ExactFrontierRoutesThroughSlot ==
  /\ SpecRoute(ExactRequestedDominates) = RouteSlotApply
  /\ SpecRoute(ExactNoSlotFallback) = RouteSlotApply
  /\ SpecAppliedHeight(ExactNoSlotFallback) = FrontierHeight(ExactNoSlotFallback)
  /\ SpecAppliedView(ExactNoSlotFallback) = RequestedView(ExactNoSlotFallback)
  /\ SpecAppliedHeight(ExactStaleSlotFallback) =
       FrontierHeight(ExactStaleSlotFallback)
  /\ SpecSlotPresentAfter(ExactStaleSlotFallback) = FALSE

NonExactRoutesThroughGeneric ==
  /\ SpecRoute(LowerHeightGeneric) = RouteGenericTrigger
  /\ SpecAppliedHeight(LowerHeightGeneric) = InputHeight(LowerHeightGeneric)
  /\ SpecRoute(FutureHeightGeneric) = RouteGenericTrigger
  /\ SpecAppliedHeight(FutureHeightGeneric) = InputHeight(FutureHeightGeneric)
  /\ SpecSlotPresentAfter(GenericPreservesSlotState) = TRUE
  /\ SpecOwnerKindAfter(GenericPreservesSlotState) = OwnerOther
  /\ SpecActiveViewAfter(GenericPreservesSlotState) =
       ActiveViewBefore(GenericPreservesSlotState)

SlotEventStateStable ==
  /\ SpecAppliedView(ExactRequestedDominates) = RequestedView(ExactRequestedDominates)
  /\ SpecAppliedView(ExactActiveDominates) = ActiveViewBefore(ExactActiveDominates)
  /\ SpecAppliedView(ExactCandidateDominates) = CandidateView(ExactCandidateDominates)
  /\ SpecActiveViewAfter(ExactSaturatingIncrement) = MaxView
  /\ SpecRebroadcastAfter(ExactClearsRebroadcast) = FALSE
  /\ SpecLastAdvanceUpdated(ExactUpdatesTimestamps)
  /\ SpecLastUpdatedAtUpdated(ExactUpdatesTimestamps)
  /\ SpecOutput(ExactCausePreserved)[4] = CauseStakeQuorumTimeout
  /\ SpecOutput(GenericCausePreserved)[4] = CauseStakeQuorumTimeout

SafetyFast ==
  /\ SelectionExact
  /\ ExactFrontierRoutesThroughSlot
  /\ NonExactRoutesThroughGeneric
  /\ SlotEventStateStable

Safety ==
  SafetyFast

=============================================================================
====
