---- MODULE SumeragiMissingQcHeightStallGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for same-height missing-QC stall dampening.

This slice captures `missing_qc_height_stall_snapshot(...)`,
`try_reserve_missing_qc_height_stall_rotation_window(...)`,
`missing_qc_height_stall_rotation_window_available(...)`,
`mark_missing_qc_height_stall_range_pull_window(...)`, and
`mark_missing_qc_height_stall_rotation_window(...)`.

The helper initializes only for the active consensus round with an unresolved
dependency, activates after three no-progress windows, resets on dependency
progress or commit progress, preserves an existing unresolved dependency height
across active-edge/committed-edge reclassification, and limits rotation plus
range-pull emissions to one reservation per active stalled window.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

WrongActiveHeight == "wrong_active_height"
NoDependency == "no_dependency"
InitialDependency == "initial_dependency"
BeforeActivationTwo == "before_activation_two"
ActivateAtThree == "activate_at_three"
ActiveNextWindow == "active_next_window"
ProgressResetInactive == "progress_reset_inactive"
ProgressResetActive == "progress_reset_active"
CommittedHeightAdvanced == "committed_height_advanced"
KeepExistingDependency == "keep_existing_dependency"
SwitchToNewDependency == "switch_to_new_dependency"

SnapshotCases == {
  WrongActiveHeight,
  NoDependency,
  InitialDependency,
  BeforeActivationTwo,
  ActivateAtThree,
  ActiveNextWindow,
  ProgressResetInactive,
  ProgressResetActive,
  CommittedHeightAdvanced,
  KeepExistingDependency,
  SwitchToNewDependency
}

ReserveNoSnapshot == "reserve_no_snapshot"
ReserveInactive == "reserve_inactive"
ReserveActiveFirst == "reserve_active_first"
ReserveActiveDuplicate == "reserve_active_duplicate"
ReserveActiveNextWindow == "reserve_active_next_window"
AvailableNoSnapshot == "available_no_snapshot"
AvailableInactive == "available_inactive"
AvailableActiveFirst == "available_active_first"
AvailableActiveDuplicate == "available_active_duplicate"
AvailableActiveNextWindow == "available_active_next_window"

ReservationCases == {
  ReserveNoSnapshot,
  ReserveInactive,
  ReserveActiveFirst,
  ReserveActiveDuplicate,
  ReserveActiveNextWindow,
  AvailableNoSnapshot,
  AvailableInactive,
  AvailableActiveFirst,
  AvailableActiveDuplicate,
  AvailableActiveNextWindow
}

MarkRangeActiveMatching == "mark_range_active_matching"
MarkRangeWrongHeight == "mark_range_wrong_height"
MarkRangeInactive == "mark_range_inactive"
MarkRotationActiveMatching == "mark_rotation_active_matching"
MarkRotationWrongHeight == "mark_rotation_wrong_height"
MarkRotationInactive == "mark_rotation_inactive"

MarkerCases == {
  MarkRangeActiveMatching,
  MarkRangeWrongHeight,
  MarkRangeInactive,
  MarkRotationActiveMatching,
  MarkRotationWrongHeight,
  MarkRotationInactive
}

Cases == SnapshotCases \cup ReservationCases \cup MarkerCases

NoSnapshot == 1
Snapshot == 2
StateCleared == 3
StateStored == 4
ModeActive == 5
ModeInactive == 6
Window0 == 7
Window1 == 8
DependencyExisting == 9
DependencyNew == 10
Windows0 == 11
Windows2 == 12
Windows3 == 13
RotationCleared == 14
ReacquireCleared == 15
RotationRetained == 16
ReacquireRetained == 17
EnteredNow == 18
EnteredProgress == 19
LastWindowNow == 20
LastWindowProgress == 21
CommittedHeightRetained == 22
ReserveAllowed == 23
ReserveRejected == 24
RotationMarked == 25
RotationUnchanged == 26
AvailableTrue == 27
AvailableFalse == 28
ReacquireMarked == 29
ReacquireUnchanged == 30

ActionUniverse == 1..30

InitialActions ==
  {Snapshot, StateStored, ModeInactive, Window0, DependencyNew, Windows0,
   RotationCleared, ReacquireCleared, EnteredNow, LastWindowNow,
   CommittedHeightRetained}

InactiveTwoWindowActions ==
  {Snapshot, StateStored, ModeInactive, Window0, DependencyExisting, Windows2,
   RotationRetained, ReacquireRetained, LastWindowNow,
   CommittedHeightRetained}

ActivatedActions ==
  {Snapshot, StateStored, ModeActive, Window0, DependencyExisting, Windows3,
   RotationCleared, ReacquireCleared, EnteredNow, LastWindowNow,
   CommittedHeightRetained}

ActiveNextWindowActions ==
  {Snapshot, StateStored, ModeActive, Window1, DependencyExisting, Windows3,
   RotationRetained, ReacquireRetained, LastWindowNow,
   CommittedHeightRetained}

ProgressResetInactiveActions ==
  {Snapshot, StateStored, ModeInactive, Window0, DependencyExisting, Windows0,
   RotationRetained, ReacquireRetained, EnteredProgress, LastWindowProgress,
   CommittedHeightRetained}

ProgressResetActiveActions ==
  {Snapshot, StateStored, ModeInactive, Window0, DependencyExisting, Windows0,
   RotationCleared, ReacquireCleared, EnteredProgress, LastWindowProgress,
   CommittedHeightRetained}

KeepExistingDependencyActions ==
  {Snapshot, StateStored, ModeInactive, Window0, DependencyExisting,
   CommittedHeightRetained}

SwitchToNewDependencyActions ==
  {Snapshot, StateStored, ModeInactive, Window0, DependencyNew,
   CommittedHeightRetained}

SpecActions(c) ==
  CASE c = WrongActiveHeight -> {NoSnapshot, StateCleared}
    [] c = NoDependency -> {NoSnapshot, StateCleared}
    [] c = InitialDependency -> InitialActions
    [] c = BeforeActivationTwo -> InactiveTwoWindowActions
    [] c = ActivateAtThree -> ActivatedActions
    [] c = ActiveNextWindow -> ActiveNextWindowActions
    [] c = ProgressResetInactive -> ProgressResetInactiveActions
    [] c = ProgressResetActive -> ProgressResetActiveActions
    [] c = CommittedHeightAdvanced -> {NoSnapshot, StateCleared}
    [] c = KeepExistingDependency -> KeepExistingDependencyActions
    [] c = SwitchToNewDependency -> SwitchToNewDependencyActions
    [] c = ReserveNoSnapshot -> {ReserveAllowed, RotationUnchanged}
    [] c = ReserveInactive -> {ReserveAllowed, RotationUnchanged}
    [] c = ReserveActiveFirst -> {ReserveAllowed, RotationMarked}
    [] c = ReserveActiveDuplicate -> {ReserveRejected, RotationUnchanged}
    [] c = ReserveActiveNextWindow -> {ReserveAllowed, RotationMarked}
    [] c = AvailableNoSnapshot -> {AvailableFalse}
    [] c = AvailableInactive -> {AvailableFalse}
    [] c = AvailableActiveFirst -> {AvailableTrue}
    [] c = AvailableActiveDuplicate -> {AvailableFalse}
    [] c = AvailableActiveNextWindow -> {AvailableTrue}
    [] c = MarkRangeActiveMatching -> {ReacquireMarked}
    [] c = MarkRangeWrongHeight -> {ReacquireUnchanged}
    [] c = MarkRangeInactive -> {ReacquireUnchanged}
    [] c = MarkRotationActiveMatching -> {RotationMarked}
    [] c = MarkRotationWrongHeight -> {RotationUnchanged}
    [] c = MarkRotationInactive -> {RotationUnchanged}

ImplementationActions(c) ==
  CASE Bug = "snapshot_wrong_height_keeps_state"
       /\ c = WrongActiveHeight ->
      InitialActions
    [] Bug = "snapshot_no_dependency_keeps_state"
       /\ c = NoDependency ->
      InitialActions
    [] Bug = "initial_starts_active"
       /\ c = InitialDependency ->
      (InitialActions \ {ModeInactive}) \cup {ModeActive}
    [] Bug = "initial_uses_existing_dependency"
       /\ c = InitialDependency ->
      (InitialActions \ {DependencyNew}) \cup {DependencyExisting}
    [] Bug = "activate_requires_more_than_three"
       /\ c = ActivateAtThree ->
      InactiveTwoWindowActions
    [] Bug = "activate_after_two_windows"
       /\ c = BeforeActivationTwo ->
      ActivatedActions
    [] Bug = "activation_starts_window_one"
       /\ c = ActivateAtThree ->
      (ActivatedActions \ {Window0}) \cup {Window1}
    [] Bug = "active_window_not_incremented"
       /\ c = ActiveNextWindow ->
      (ActiveNextWindowActions \ {Window1}) \cup {Window0}
    [] Bug = "active_window_clears_markers"
       /\ c = ActiveNextWindow ->
      (ActiveNextWindowActions \ {RotationRetained, ReacquireRetained})
        \cup {RotationCleared, ReacquireCleared}
    [] Bug = "progress_inactive_not_reset"
       /\ c = ProgressResetInactive ->
      InactiveTwoWindowActions
    [] Bug = "progress_active_keeps_mode"
       /\ c = ProgressResetActive ->
      ActiveNextWindowActions
    [] Bug = "progress_active_keeps_markers"
       /\ c = ProgressResetActive ->
      (ProgressResetActiveActions \ {RotationCleared, ReacquireCleared})
        \cup {RotationRetained, ReacquireRetained}
    [] Bug = "commit_progress_keeps_stall"
       /\ c = CommittedHeightAdvanced ->
      ActiveNextWindowActions
    [] Bug = "reclassification_uses_new_dependency"
       /\ c = KeepExistingDependency ->
      SwitchToNewDependencyActions
    [] Bug = "resolved_old_keeps_existing_dependency"
       /\ c = SwitchToNewDependency ->
      KeepExistingDependencyActions
    [] Bug = "reserve_no_snapshot_blocks"
       /\ c = ReserveNoSnapshot ->
      {ReserveRejected, RotationUnchanged}
    [] Bug = "reserve_inactive_blocks"
       /\ c = ReserveInactive ->
      {ReserveRejected, RotationUnchanged}
    [] Bug = "reserve_duplicate_allows"
       /\ c = ReserveActiveDuplicate ->
      {ReserveAllowed, RotationMarked}
    [] Bug = "reserve_first_does_not_mark"
       /\ c = ReserveActiveFirst ->
      {ReserveAllowed, RotationUnchanged}
    [] Bug = "available_inactive_true"
       /\ c = AvailableInactive ->
      {AvailableTrue}
    [] Bug = "available_duplicate_true"
       /\ c = AvailableActiveDuplicate ->
      {AvailableTrue}
    [] Bug = "mark_range_wrong_height_records"
       /\ c = MarkRangeWrongHeight ->
      {ReacquireMarked}
    [] Bug = "mark_range_inactive_records"
       /\ c = MarkRangeInactive ->
      {ReacquireMarked}
    [] Bug = "mark_rotation_wrong_height_records"
       /\ c = MarkRotationWrongHeight ->
      {RotationMarked}
    [] Bug = "mark_rotation_inactive_records"
       /\ c = MarkRotationInactive ->
      {RotationMarked}
    [] OTHER -> SpecActions(c)

Bugs == {
  "none",
  "snapshot_wrong_height_keeps_state",
  "snapshot_no_dependency_keeps_state",
  "initial_starts_active",
  "initial_uses_existing_dependency",
  "activate_requires_more_than_three",
  "activate_after_two_windows",
  "activation_starts_window_one",
  "active_window_not_incremented",
  "active_window_clears_markers",
  "progress_inactive_not_reset",
  "progress_active_keeps_mode",
  "progress_active_keeps_markers",
  "commit_progress_keeps_stall",
  "reclassification_uses_new_dependency",
  "resolved_old_keeps_existing_dependency",
  "reserve_no_snapshot_blocks",
  "reserve_inactive_blocks",
  "reserve_duplicate_allows",
  "reserve_first_does_not_mark",
  "available_inactive_true",
  "available_duplicate_true",
  "mark_range_wrong_height_records",
  "mark_range_inactive_records",
  "mark_rotation_wrong_height_records",
  "mark_rotation_inactive_records"
}

Init ==
  checked = 0

Next ==
  \/ /\ checked < 25
     /\ checked' = checked + 1
  \/ /\ checked = 25
     /\ UNCHANGED vars

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked \in 0..25
  /\ \A c \in Cases:
       /\ SpecActions(c) \subseteq ActionUniverse
       /\ ImplementationActions(c) \subseteq ActionUniverse

SnapshotMatchesSpec ==
  \A c \in SnapshotCases:
    ImplementationActions(c) = SpecActions(c)

ReservationMatchesSpec ==
  \A c \in ReservationCases:
    ImplementationActions(c) = SpecActions(c)

MarkerMatchesSpec ==
  \A c \in MarkerCases:
    ImplementationActions(c) = SpecActions(c)

SnapshotLifecycleSafety ==
  /\ StateCleared \in ImplementationActions(WrongActiveHeight)
  /\ StateCleared \in ImplementationActions(NoDependency)
  /\ StateStored \in ImplementationActions(InitialDependency)
  /\ ModeInactive \in ImplementationActions(BeforeActivationTwo)
  /\ ModeActive \in ImplementationActions(ActivateAtThree)
  /\ Window0 \in ImplementationActions(ActivateAtThree)
  /\ Window1 \in ImplementationActions(ActiveNextWindow)
  /\ Windows0 \in ImplementationActions(ProgressResetActive)
  /\ ModeInactive \in ImplementationActions(ProgressResetActive)
  /\ StateCleared \in ImplementationActions(CommittedHeightAdvanced)

DependencyContinuitySafety ==
  /\ DependencyExisting \in ImplementationActions(KeepExistingDependency)
  /\ DependencyNew \in ImplementationActions(SwitchToNewDependency)
  /\ DependencyNew \in ImplementationActions(InitialDependency)

ReservationWindowSafety ==
  /\ ReserveAllowed \in ImplementationActions(ReserveNoSnapshot)
  /\ ReserveAllowed \in ImplementationActions(ReserveInactive)
  /\ RotationMarked \in ImplementationActions(ReserveActiveFirst)
  /\ ReserveRejected \in ImplementationActions(ReserveActiveDuplicate)
  /\ AvailableFalse \in ImplementationActions(AvailableInactive)
  /\ AvailableFalse \in ImplementationActions(AvailableActiveDuplicate)
  /\ AvailableTrue \in ImplementationActions(AvailableActiveFirst)

MarkerHeightModeSafety ==
  /\ ReacquireMarked \in ImplementationActions(MarkRangeActiveMatching)
  /\ ReacquireUnchanged \in ImplementationActions(MarkRangeWrongHeight)
  /\ ReacquireUnchanged \in ImplementationActions(MarkRangeInactive)
  /\ RotationMarked \in ImplementationActions(MarkRotationActiveMatching)
  /\ RotationUnchanged \in ImplementationActions(MarkRotationWrongHeight)
  /\ RotationUnchanged \in ImplementationActions(MarkRotationInactive)

SafetyFast ==
  /\ SnapshotMatchesSpec
  /\ ReservationMatchesSpec
  /\ MarkerMatchesSpec
  /\ SnapshotLifecycleSafety
  /\ DependencyContinuitySafety
  /\ ReservationWindowSafety
  /\ MarkerHeightModeSafety

SpecComparisonAnchors ==
  /\ SnapshotMatchesSpec
  /\ ReservationMatchesSpec
  /\ MarkerMatchesSpec

SnapshotLifecycleAnchors ==
  /\ SnapshotLifecycleSafety
  /\ StateCleared \in ImplementationActions(WrongActiveHeight)
  /\ StateCleared \in ImplementationActions(NoDependency)
  /\ StateStored \in ImplementationActions(InitialDependency)
  /\ ModeInactive \in ImplementationActions(BeforeActivationTwo)
  /\ ModeActive \in ImplementationActions(ActivateAtThree)
  /\ Window0 \in ImplementationActions(ActivateAtThree)
  /\ Window1 \in ImplementationActions(ActiveNextWindow)
  /\ Windows0 \in ImplementationActions(ProgressResetActive)
  /\ ModeInactive \in ImplementationActions(ProgressResetActive)
  /\ StateCleared \in ImplementationActions(CommittedHeightAdvanced)

DependencyContinuityAnchors ==
  /\ DependencyContinuitySafety
  /\ DependencyExisting \in ImplementationActions(KeepExistingDependency)
  /\ DependencyNew \in ImplementationActions(SwitchToNewDependency)
  /\ DependencyNew \in ImplementationActions(InitialDependency)

ReservationWindowAnchors ==
  /\ ReservationWindowSafety
  /\ ReserveAllowed \in ImplementationActions(ReserveNoSnapshot)
  /\ ReserveAllowed \in ImplementationActions(ReserveInactive)
  /\ RotationMarked \in ImplementationActions(ReserveActiveFirst)
  /\ ReserveRejected \in ImplementationActions(ReserveActiveDuplicate)
  /\ AvailableFalse \in ImplementationActions(AvailableInactive)
  /\ AvailableFalse \in ImplementationActions(AvailableActiveDuplicate)
  /\ AvailableTrue \in ImplementationActions(AvailableActiveFirst)

MarkerHeightModeAnchors ==
  /\ MarkerHeightModeSafety
  /\ ReacquireMarked \in ImplementationActions(MarkRangeActiveMatching)
  /\ ReacquireUnchanged \in ImplementationActions(MarkRangeWrongHeight)
  /\ ReacquireUnchanged \in ImplementationActions(MarkRangeInactive)
  /\ RotationMarked \in ImplementationActions(MarkRotationActiveMatching)
  /\ RotationUnchanged \in ImplementationActions(MarkRotationWrongHeight)
  /\ RotationUnchanged \in ImplementationActions(MarkRotationInactive)

MissingQcHeightStallSafetyAnchors ==
  /\ SpecComparisonAnchors
  /\ SnapshotLifecycleAnchors
  /\ DependencyContinuityAnchors
  /\ ReservationWindowAnchors
  /\ MarkerHeightModeAnchors

Safety ==
  MissingQcHeightStallSafetyAnchors

====
