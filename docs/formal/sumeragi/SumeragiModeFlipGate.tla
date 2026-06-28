---- MODULE SumeragiModeFlipGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for runtime consensus-mode flip helpers.

This slice captures `update_pending_mode_flip(...)`, `mode_activation_lag(...)`,
and the top-level gating contract in `Actor::apply_mode_flip(...)`. The model
abstracts Permissioned/NPoS into finite cases while preserving the observable
contracts: pending flip trackers are cleared when the effective mode matches
the current mode, repeated pending targets do not emit duplicate detections,
activation lag is reported only after the activation height while the runtime
mode is still behind, busy commit pipelines defer mode application without
resetting consensus state, and idle flips clear volatile consensus state,
rebuild mode-specific epoch/collector state, clear highest/locked QC status,
update network consensus capabilities, clear the pending flip, and record a
success.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

TrackerSameNone == "tracker_same_none"
TrackerSameExisting == "tracker_same_existing"
TrackerNewNone == "tracker_new_none"
TrackerRepeat == "tracker_repeat"
TrackerRetarget == "tracker_retarget"

LagNoActivation == "lag_no_activation"
LagBeforeActivation == "lag_before_activation"
LagAtActivationDifferent == "lag_at_activation_different"
LagAfterActivationDifferent == "lag_after_activation_different"
LagSameModeAtActivation == "lag_same_mode_at_activation"

ApplySame == "apply_same"
ApplyBusyProcessing == "apply_busy_processing"
ApplyBusyInflight == "apply_busy_inflight"
ApplyBusyBoth == "apply_busy_both"
ApplyIdlePermToNpos == "apply_idle_perm_to_npos"
ApplyIdleNposToPerm == "apply_idle_npos_to_perm"

TrackerCases == {
  TrackerSameNone,
  TrackerSameExisting,
  TrackerNewNone,
  TrackerRepeat,
  TrackerRetarget
}

LagCases == {
  LagNoActivation,
  LagBeforeActivation,
  LagAtActivationDifferent,
  LagAfterActivationDifferent,
  LagSameModeAtActivation
}

BusyApplyCases == {
  ApplyBusyProcessing,
  ApplyBusyInflight,
  ApplyBusyBoth
}

IdleApplyCases == {
  ApplyIdlePermToNpos,
  ApplyIdleNposToPerm
}

ApplyCases == {ApplySame} \cup BusyApplyCases \cup IdleApplyCases

Cases == TrackerCases \cup LagCases \cup ApplyCases

PendingNone == 1
PendingPerm == 2
PendingNpos == 3
EmitNone == 4
EmitPerm == 5
EmitNpos == 6
LagNone == 7
LagOne == 8
LagThree == 9
ModePerm == 10
ModeNpos == 11
ResetDone == 12
ResetSkipped == 13
RebuildPermissioned == 14
RebuildNpos == 15
RebuildSkipped == 16
CollectorsNone == 17
CollectorsNpos == 18
StatusSuccess == 19
StatusBlockedProcessing == 20
StatusBlockedInflight == 21
StatusBlockedBoth == 22
StatusNoFlip == 23
QcCleared == 24
CapsUpdated == 25
PendingPreserved == 26
PendingCleared == 27

Actions == 1..27

SpecActions(c) ==
  CASE c = TrackerSameNone -> {PendingNone, EmitNone}
    [] c = TrackerSameExisting -> {PendingNone, EmitNone}
    [] c = TrackerNewNone -> {PendingNpos, EmitNpos}
    [] c = TrackerRepeat -> {PendingNpos, EmitNone}
    [] c = TrackerRetarget -> {PendingPerm, EmitPerm}
    [] c = LagNoActivation -> {LagNone}
    [] c = LagBeforeActivation -> {LagNone}
    [] c = LagAtActivationDifferent -> {LagOne}
    [] c = LagAfterActivationDifferent -> {LagThree}
    [] c = LagSameModeAtActivation -> {LagNone}
    [] c = ApplySame ->
      {ModePerm, PendingCleared, ResetSkipped, RebuildSkipped, StatusNoFlip}
    [] c = ApplyBusyProcessing ->
      {ModePerm, PendingPreserved, ResetSkipped, RebuildSkipped,
       StatusBlockedProcessing}
    [] c = ApplyBusyInflight ->
      {ModePerm, PendingPreserved, ResetSkipped, RebuildSkipped,
       StatusBlockedInflight}
    [] c = ApplyBusyBoth ->
      {ModePerm, PendingPreserved, ResetSkipped, RebuildSkipped,
       StatusBlockedBoth}
    [] c = ApplyIdlePermToNpos ->
      {ModeNpos, PendingCleared, ResetDone, RebuildNpos, CollectorsNpos,
       StatusSuccess, QcCleared, CapsUpdated}
    [] c = ApplyIdleNposToPerm ->
      {ModePerm, PendingCleared, ResetDone, RebuildPermissioned,
       CollectorsNone, StatusSuccess, QcCleared, CapsUpdated}
    [] OTHER -> {}

ImplementationActions(c) ==
  LET spec == SpecActions(c) IN
  CASE Bug = "tracker_same_keeps_pending"
       /\ c = TrackerSameExisting ->
      (spec \ {PendingNone}) \cup {PendingNpos}
    [] Bug = "tracker_new_drops_signal"
       /\ c = TrackerNewNone ->
      (spec \ {EmitNpos}) \cup {EmitNone}
    [] Bug = "tracker_new_does_not_store"
       /\ c = TrackerNewNone ->
      (spec \ {PendingNpos}) \cup {PendingNone}
    [] Bug = "tracker_repeat_reemits"
       /\ c = TrackerRepeat ->
      (spec \ {EmitNone}) \cup {EmitNpos}
    [] Bug = "tracker_retarget_keeps_old"
       /\ c = TrackerRetarget ->
      (spec \ {PendingPerm, EmitPerm}) \cup {PendingNpos, EmitNone}
    [] Bug = "lag_missing_activation_reports"
       /\ c = LagNoActivation ->
      {LagOne}
    [] Bug = "lag_before_activation_reports"
       /\ c = LagBeforeActivation ->
      {LagOne}
    [] Bug = "lag_at_activation_zero"
       /\ c = LagAtActivationDifferent ->
      {LagNone}
    [] Bug = "lag_after_off_by_one"
       /\ c = LagAfterActivationDifferent ->
      {LagOne}
    [] Bug = "lag_same_mode_reports"
       /\ c = LagSameModeAtActivation ->
      {LagOne}
    [] Bug = "apply_same_keeps_pending"
       /\ c = ApplySame ->
      (spec \ {PendingCleared}) \cup {PendingPreserved}
    [] Bug = "apply_same_resets_state"
       /\ c = ApplySame ->
      (spec \ {ResetSkipped, RebuildSkipped}) \cup {ResetDone,
        RebuildPermissioned}
    [] Bug = "apply_busy_changes_mode"
       /\ c \in BusyApplyCases ->
      (spec \ {ModePerm}) \cup {ModeNpos}
    [] Bug = "apply_busy_clears_pending"
       /\ c \in BusyApplyCases ->
      (spec \ {PendingPreserved}) \cup {PendingCleared}
    [] Bug = "apply_busy_uses_wrong_reason"
       /\ c = ApplyBusyInflight ->
      (spec \ {StatusBlockedInflight}) \cup {StatusBlockedProcessing}
    [] Bug = "apply_busy_resets_state"
       /\ c \in BusyApplyCases ->
      (spec \ {ResetSkipped, RebuildSkipped}) \cup {ResetDone, RebuildNpos}
    [] Bug = "apply_idle_skips_reset"
       /\ c \in IdleApplyCases ->
      (spec \ {ResetDone}) \cup {ResetSkipped}
    [] Bug = "apply_idle_keeps_old_mode"
       /\ c = ApplyIdlePermToNpos ->
      (spec \ {ModeNpos}) \cup {ModePerm}
    [] Bug = "apply_idle_keeps_pending"
       /\ c \in IdleApplyCases ->
      (spec \ {PendingCleared}) \cup {PendingPreserved}
    [] Bug = "apply_idle_skips_rebuild"
       /\ c \in IdleApplyCases ->
      (spec \ {RebuildNpos, RebuildPermissioned}) \cup {RebuildSkipped}
    [] Bug = "apply_npos_drops_collectors"
       /\ c = ApplyIdlePermToNpos ->
      (spec \ {CollectorsNpos}) \cup {CollectorsNone}
    [] Bug = "apply_permissioned_keeps_collectors"
       /\ c = ApplyIdleNposToPerm ->
      (spec \ {CollectorsNone}) \cup {CollectorsNpos}
    [] Bug = "apply_idle_skips_qc_clear"
       /\ c \in IdleApplyCases ->
      spec \ {QcCleared}
    [] Bug = "apply_idle_skips_caps_update"
       /\ c \in IdleApplyCases ->
      spec \ {CapsUpdated}
    [] Bug = "apply_idle_skips_success"
       /\ c \in IdleApplyCases ->
      (spec \ {StatusSuccess}) \cup {StatusNoFlip}
    [] OTHER -> spec

Bugs == {
  "none",
  "tracker_same_keeps_pending",
  "tracker_new_drops_signal",
  "tracker_new_does_not_store",
  "tracker_repeat_reemits",
  "tracker_retarget_keeps_old",
  "lag_missing_activation_reports",
  "lag_before_activation_reports",
  "lag_at_activation_zero",
  "lag_after_off_by_one",
  "lag_same_mode_reports",
  "apply_same_keeps_pending",
  "apply_same_resets_state",
  "apply_busy_changes_mode",
  "apply_busy_clears_pending",
  "apply_busy_uses_wrong_reason",
  "apply_busy_resets_state",
  "apply_idle_skips_reset",
  "apply_idle_keeps_old_mode",
  "apply_idle_keeps_pending",
  "apply_idle_skips_rebuild",
  "apply_npos_drops_collectors",
  "apply_permissioned_keeps_collectors",
  "apply_idle_skips_qc_clear",
  "apply_idle_skips_caps_update",
  "apply_idle_skips_success"
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

ModeFlipCoreSafety ==
  \A c \in Cases:
    ImplementationActions(c) = SpecActions(c)

ModeFlipExactness ==
  /\ ModeFlipCoreSafety

ModeFlipCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ ModeFlipExactness

NoBugInvariant == ModeFlipExactness

SafetyFast == ModeFlipExactness

Safety ==
  ModeFlipCorrectnessEnvelope

BugTrackerSameKeepsPending == NoBugInvariant
BugTrackerNewDropsSignal == NoBugInvariant
BugTrackerNewDoesNotStore == NoBugInvariant
BugTrackerRepeatReemits == NoBugInvariant
BugTrackerRetargetKeepsOld == NoBugInvariant
BugLagMissingActivationReports == NoBugInvariant
BugLagBeforeActivationReports == NoBugInvariant
BugLagAtActivationZero == NoBugInvariant
BugLagAfterOffByOne == NoBugInvariant
BugLagSameModeReports == NoBugInvariant
BugApplySameKeepsPending == NoBugInvariant
BugApplySameResetsState == NoBugInvariant
BugApplyBusyChangesMode == NoBugInvariant
BugApplyBusyClearsPending == NoBugInvariant
BugApplyBusyUsesWrongReason == NoBugInvariant
BugApplyBusyResetsState == NoBugInvariant
BugApplyIdleSkipsReset == NoBugInvariant
BugApplyIdleKeepsOldMode == NoBugInvariant
BugApplyIdleKeepsPending == NoBugInvariant
BugApplyIdleSkipsRebuild == NoBugInvariant
BugApplyNposDropsCollectors == NoBugInvariant
BugApplyPermissionedKeepsCollectors == NoBugInvariant
BugApplyIdleSkipsQcClear == NoBugInvariant
BugApplyIdleSkipsCapsUpdate == NoBugInvariant
BugApplyIdleSkipsSuccess == NoBugInvariant

====
