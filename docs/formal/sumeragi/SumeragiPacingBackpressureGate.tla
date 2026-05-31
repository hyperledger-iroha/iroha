---- MODULE SumeragiPacingBackpressureGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi pacing backpressure helpers.

This slice captures `BackpressureGate::{new, refresh, state, should_defer}`
and `PacemakerBackpressure::update(...)`. It abstracts queue state to
Healthy/Saturated while preserving the observable contracts: construction
copies the receiver snapshot, `refresh()` reports true only on changed
snapshots and stores the new snapshot, `state()` returns the cached value
without reading the receiver, `should_defer()` refreshes before checking
saturation, and pacemaker deferral state reports first/subsequent/none while
setting or clearing the internal deferring flag exactly.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

GateNewHealthy == "gate_new_healthy"
GateNewSaturated == "gate_new_saturated"
GateRefreshUnchangedHealthy == "gate_refresh_unchanged_healthy"
GateRefreshUnchangedSaturated == "gate_refresh_unchanged_saturated"
GateRefreshHealthyToSaturated == "gate_refresh_healthy_to_saturated"
GateRefreshSaturatedToHealthy == "gate_refresh_saturated_to_healthy"
GateStateBeforeRefreshUsesCachedHealthy == "gate_state_before_refresh_uses_cached_healthy"
GateShouldDeferRefreshesToSaturated == "gate_should_defer_refreshes_to_saturated"
GateShouldDeferRefreshesToHealthy == "gate_should_defer_refreshes_to_healthy"
PacerEnterDeferral == "pacer_enter_deferral"
PacerStayDeferring == "pacer_stay_deferring"
PacerClearDeferral == "pacer_clear_deferral"
PacerRemainIdle == "pacer_remain_idle"
PacerReenterAfterClear == "pacer_reenter_after_clear"

Cases == {
  GateNewHealthy,
  GateNewSaturated,
  GateRefreshUnchangedHealthy,
  GateRefreshUnchangedSaturated,
  GateRefreshHealthyToSaturated,
  GateRefreshSaturatedToHealthy,
  GateStateBeforeRefreshUsesCachedHealthy,
  GateShouldDeferRefreshesToSaturated,
  GateShouldDeferRefreshesToHealthy,
  PacerEnterDeferral,
  PacerStayDeferring,
  PacerClearDeferral,
  PacerRemainIdle,
  PacerReenterAfterClear
}

RefreshFalse == 1
RefreshTrue == 2
CurrentHealthy == 3
CurrentSaturated == 4
RxHealthy == 5
RxSaturated == 6
StoredSnapshot == 7
StateUnchanged == 8
CachedRead == 9
ReceiverRead == 10
ShouldDeferFalse == 11
ShouldDeferTrue == 12
RefreshedBeforeCheck == 13
SkippedRefresh == 14
ActionFirst == 15
ActionSubsequent == 16
ActionNone == 17
DeferringTrue == 18
DeferringFalse == 19
ReentryFirst == 20
ReentrySubsequent == 21

Actions == 1..21

SpecActions(c) ==
  CASE c = GateNewHealthy ->
      {CurrentHealthy, RxHealthy, StoredSnapshot, ShouldDeferFalse}
    [] c = GateNewSaturated ->
      {CurrentSaturated, RxSaturated, StoredSnapshot, ShouldDeferTrue}
    [] c = GateRefreshUnchangedHealthy ->
      {RefreshFalse, CurrentHealthy, StateUnchanged}
    [] c = GateRefreshUnchangedSaturated ->
      {RefreshFalse, CurrentSaturated, StateUnchanged}
    [] c = GateRefreshHealthyToSaturated ->
      {RefreshTrue, CurrentSaturated, StoredSnapshot}
    [] c = GateRefreshSaturatedToHealthy ->
      {RefreshTrue, CurrentHealthy, StoredSnapshot}
    [] c = GateStateBeforeRefreshUsesCachedHealthy ->
      {CurrentHealthy, RxSaturated, CachedRead, ShouldDeferFalse}
    [] c = GateShouldDeferRefreshesToSaturated ->
      {RefreshTrue, CurrentSaturated, ShouldDeferTrue, RefreshedBeforeCheck,
       StoredSnapshot}
    [] c = GateShouldDeferRefreshesToHealthy ->
      {RefreshTrue, CurrentHealthy, ShouldDeferFalse, RefreshedBeforeCheck,
       StoredSnapshot}
    [] c = PacerEnterDeferral ->
      {ActionFirst, DeferringTrue}
    [] c = PacerStayDeferring ->
      {ActionSubsequent, DeferringTrue}
    [] c = PacerClearDeferral ->
      {ActionNone, DeferringFalse}
    [] c = PacerRemainIdle ->
      {ActionNone, DeferringFalse}
    [] c = PacerReenterAfterClear ->
      {ActionNone, DeferringFalse, ReentryFirst, DeferringTrue}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "init_ignores_rx"
       /\ c = GateNewSaturated ->
      (spec \ {CurrentSaturated, ShouldDeferTrue}) \cup
        {CurrentHealthy, ShouldDeferFalse}
    [] Bug = "refresh_reports_change_on_equal"
       /\ c \in {GateRefreshUnchangedHealthy, GateRefreshUnchangedSaturated} ->
      (spec \ {RefreshFalse, StateUnchanged}) \cup {RefreshTrue, StoredSnapshot}
    [] Bug = "refresh_misses_change"
       /\ c \in {GateRefreshHealthyToSaturated, GateRefreshSaturatedToHealthy} ->
      (spec \ {RefreshTrue, StoredSnapshot}) \cup {RefreshFalse, StateUnchanged}
    [] Bug = "refresh_does_not_store_snapshot"
       /\ c = GateRefreshHealthyToSaturated ->
      (spec \ {CurrentSaturated, StoredSnapshot}) \cup {CurrentHealthy}
    [] Bug = "state_reads_receiver_without_refresh"
       /\ c = GateStateBeforeRefreshUsesCachedHealthy ->
      (spec \ {CurrentHealthy, CachedRead, ShouldDeferFalse}) \cup
        {CurrentSaturated, ReceiverRead, ShouldDeferTrue}
    [] Bug = "should_defer_skips_refresh"
       /\ c = GateShouldDeferRefreshesToSaturated ->
      (spec \ {RefreshTrue, CurrentSaturated, ShouldDeferTrue,
               RefreshedBeforeCheck, StoredSnapshot}) \cup
        {RefreshFalse, CurrentHealthy, ShouldDeferFalse, SkippedRefresh}
    [] Bug = "should_defer_inverts_saturation"
       /\ c \in {GateShouldDeferRefreshesToSaturated,
                 GateShouldDeferRefreshesToHealthy} ->
      (spec \ {ShouldDeferTrue, ShouldDeferFalse}) \cup
        IF c = GateShouldDeferRefreshesToSaturated
        THEN {ShouldDeferFalse}
        ELSE {ShouldDeferTrue}
    [] Bug = "first_deferral_returns_subsequent"
       /\ c = PacerEnterDeferral ->
      (spec \ {ActionFirst}) \cup {ActionSubsequent}
    [] Bug = "first_deferral_keeps_idle"
       /\ c = PacerEnterDeferral ->
      (spec \ {DeferringTrue}) \cup {DeferringFalse}
    [] Bug = "repeated_deferral_repeats_first"
       /\ c = PacerStayDeferring ->
      (spec \ {ActionSubsequent}) \cup {ActionFirst}
    [] Bug = "clear_deferral_keeps_deferring"
       /\ c = PacerClearDeferral ->
      (spec \ {DeferringFalse}) \cup {DeferringTrue}
    [] Bug = "clear_deferral_returns_subsequent"
       /\ c = PacerClearDeferral ->
      (spec \ {ActionNone}) \cup {ActionSubsequent}
    [] Bug = "idle_update_enters_deferral"
       /\ c = PacerRemainIdle ->
      (spec \ {ActionNone, DeferringFalse}) \cup {ActionFirst, DeferringTrue}
    [] Bug = "reentry_after_clear_returns_subsequent"
       /\ c = PacerReenterAfterClear ->
      (spec \ {ReentryFirst}) \cup {ReentrySubsequent}
    [] Bug = "reentry_after_clear_keeps_cleared"
       /\ c = PacerReenterAfterClear ->
      (spec \ {ReentryFirst, DeferringTrue}) \cup {ReentrySubsequent}
    [] OTHER -> spec

Bugs == {
  "none",
  "init_ignores_rx",
  "refresh_reports_change_on_equal",
  "refresh_misses_change",
  "refresh_does_not_store_snapshot",
  "state_reads_receiver_without_refresh",
  "should_defer_skips_refresh",
  "should_defer_inverts_saturation",
  "first_deferral_returns_subsequent",
  "first_deferral_keeps_idle",
  "repeated_deferral_repeats_first",
  "clear_deferral_keeps_deferring",
  "clear_deferral_returns_subsequent",
  "idle_update_enters_deferral",
  "reentry_after_clear_returns_subsequent",
  "reentry_after_clear_keeps_cleared"
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

NoBugInvariant ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

SafetyFast == NoBugInvariant

BugInitIgnoresRx == NoBugInvariant
BugRefreshReportsChangeOnEqual == NoBugInvariant
BugRefreshMissesChange == NoBugInvariant
BugRefreshDoesNotStoreSnapshot == NoBugInvariant
BugStateReadsReceiverWithoutRefresh == NoBugInvariant
BugShouldDeferSkipsRefresh == NoBugInvariant
BugShouldDeferInvertsSaturation == NoBugInvariant
BugFirstDeferralReturnsSubsequent == NoBugInvariant
BugFirstDeferralKeepsIdle == NoBugInvariant
BugRepeatedDeferralRepeatsFirst == NoBugInvariant
BugClearDeferralKeepsDeferring == NoBugInvariant
BugClearDeferralReturnsSubsequent == NoBugInvariant
BugIdleUpdateEntersDeferral == NoBugInvariant
BugReentryAfterClearReturnsSubsequent == NoBugInvariant
BugReentryAfterClearKeepsCleared == NoBugInvariant

====
