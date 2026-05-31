---- MODULE SumeragiRbcStorePressureLogGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi RBC store pressure log throttling.

This slice captures `rbc_store_pressure_label(...)`,
`should_log_rbc_store_pressure(...)`, the log-state update branch in
`maybe_log_rbc_store_pressure(...)`, and the test-only
`reset_rbc_store_pressure_log_state_for_tests()` helper from `status.rs`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

ResetEmpty == 1
LabelNormal == 2
LabelSoft == 3
LabelHard == 4
LabelUnknown == 5
TransitionToSoftLogs == 6
TransitionToNormalLogs == 7
TransitionToUnknownLogs == 8
SameNormalSuppressed == 9
ElevatedRepeatBeforeIntervalSuppressed == 10
ElevatedRepeatAtIntervalLogs == 11
ElevatedRepeatAfterIntervalLogs == 12
BackwardsClockSuppressed == 13
SuppressedKeepsLevel == 14
SuppressedKeepsStamp == 15
LoggedUpdatesLevel == 16
LoggedUpdatesStamp == 17
ResetAfterRecordsClears == 18

Candidates == 1..18

ResetLogLevel == 1
ResetLogStamp == 2
NormalLabel == 3
SoftLabel == 4
HardLabel == 5
UnknownLabel == 6
LevelChangeLogs == 7
NormalRepeatSuppresses == 8
ElevatedBeforeIntervalSuppresses == 9
BoundaryLogs == 10
AfterIntervalLogs == 11
BackwardsClockSaturates == 12
SuppressedLevelPreserved == 13
SuppressedStampPreserved == 14
LoggedLevelStored == 15
LoggedStampStored == 16

Actions == 1..16

AllResetActions == {ResetLogLevel, ResetLogStamp}
AllLoggedStateActions == {LoggedLevelStored, LoggedStampStored}
AllSuppressedStateActions == {SuppressedLevelPreserved, SuppressedStampPreserved}

SpecActions(candidate) ==
  CASE candidate = ResetEmpty ->
      AllResetActions
    [] candidate = LabelNormal ->
      {NormalLabel}
    [] candidate = LabelSoft ->
      {SoftLabel}
    [] candidate = LabelHard ->
      {HardLabel}
    [] candidate = LabelUnknown ->
      {UnknownLabel}
    [] candidate = TransitionToSoftLogs ->
      {LevelChangeLogs, SoftLabel} \cup AllLoggedStateActions
    [] candidate = TransitionToNormalLogs ->
      {LevelChangeLogs, NormalLabel} \cup AllLoggedStateActions
    [] candidate = TransitionToUnknownLogs ->
      {LevelChangeLogs, UnknownLabel} \cup AllLoggedStateActions
    [] candidate = SameNormalSuppressed ->
      {NormalRepeatSuppresses} \cup AllSuppressedStateActions
    [] candidate = ElevatedRepeatBeforeIntervalSuppressed ->
      {ElevatedBeforeIntervalSuppresses} \cup AllSuppressedStateActions
    [] candidate = ElevatedRepeatAtIntervalLogs ->
      {BoundaryLogs} \cup AllLoggedStateActions
    [] candidate = ElevatedRepeatAfterIntervalLogs ->
      {AfterIntervalLogs} \cup AllLoggedStateActions
    [] candidate = BackwardsClockSuppressed ->
      {BackwardsClockSaturates, ElevatedBeforeIntervalSuppresses} \cup
        AllSuppressedStateActions
    [] candidate = SuppressedKeepsLevel ->
      {SuppressedLevelPreserved}
    [] candidate = SuppressedKeepsStamp ->
      {SuppressedStampPreserved}
    [] candidate = LoggedUpdatesLevel ->
      {LoggedLevelStored}
    [] candidate = LoggedUpdatesStamp ->
      {LoggedStampStored}
    [] candidate = ResetAfterRecordsClears ->
      AllResetActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = ResetEmpty /\ Bug = "reset_empty_keeps_level" ->
      spec \ {ResetLogLevel}
    [] candidate = ResetEmpty /\ Bug = "reset_empty_keeps_stamp" ->
      spec \ {ResetLogStamp}
    [] candidate = LabelNormal /\ Bug = "label_normal_unknown" ->
      (spec \ {NormalLabel}) \cup {UnknownLabel}
    [] candidate = LabelSoft /\ Bug = "label_soft_unknown" ->
      (spec \ {SoftLabel}) \cup {UnknownLabel}
    [] candidate = LabelHard /\ Bug = "label_hard_unknown" ->
      (spec \ {HardLabel}) \cup {UnknownLabel}
    [] candidate = LabelUnknown /\ Bug = "label_unknown_normal" ->
      (spec \ {UnknownLabel}) \cup {NormalLabel}
    [] candidate = TransitionToSoftLogs /\
          Bug = "transition_level_change_suppressed" ->
      (spec \ {LevelChangeLogs}) \cup {ElevatedBeforeIntervalSuppresses}
    [] candidate = TransitionToNormalLogs /\
          Bug = "transition_to_normal_suppressed" ->
      (spec \ {LevelChangeLogs}) \cup {NormalRepeatSuppresses}
    [] candidate = TransitionToUnknownLogs /\
          Bug = "transition_unknown_suppressed" ->
      (spec \ {LevelChangeLogs}) \cup {ElevatedBeforeIntervalSuppresses}
    [] candidate = SameNormalSuppressed /\ Bug = "same_normal_logs" ->
      (spec \ {NormalRepeatSuppresses}) \cup {BoundaryLogs}
    [] candidate = ElevatedRepeatBeforeIntervalSuppressed /\
          Bug = "repeat_before_interval_logs" ->
      (spec \ {ElevatedBeforeIntervalSuppresses}) \cup {BoundaryLogs}
    [] candidate = ElevatedRepeatAtIntervalLogs /\
          Bug = "repeat_at_boundary_suppressed" ->
      (spec \ {BoundaryLogs}) \cup {ElevatedBeforeIntervalSuppresses}
    [] candidate = ElevatedRepeatAfterIntervalLogs /\
          Bug = "repeat_after_interval_suppressed" ->
      (spec \ {AfterIntervalLogs}) \cup {ElevatedBeforeIntervalSuppresses}
    [] candidate = BackwardsClockSuppressed /\ Bug = "backwards_clock_logs" ->
      (spec \ {BackwardsClockSaturates, ElevatedBeforeIntervalSuppresses}) \cup
        {AfterIntervalLogs}
    [] candidate = SuppressedKeepsLevel /\ Bug = "suppressed_updates_level" ->
      (spec \ {SuppressedLevelPreserved}) \cup {LoggedLevelStored}
    [] candidate = SuppressedKeepsStamp /\ Bug = "suppressed_updates_stamp" ->
      (spec \ {SuppressedStampPreserved}) \cup {LoggedStampStored}
    [] candidate = LoggedUpdatesLevel /\ Bug = "logged_skips_level_update" ->
      spec \ {LoggedLevelStored}
    [] candidate = LoggedUpdatesStamp /\ Bug = "logged_skips_stamp_update" ->
      spec \ {LoggedStampStored}
    [] candidate = TransitionToSoftLogs /\ Bug = "logged_updates_stamp_only" ->
      spec \ {LoggedLevelStored}
    [] candidate = ElevatedRepeatAtIntervalLogs /\
          Bug = "logged_updates_level_only" ->
      spec \ {LoggedStampStored}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_level" ->
      spec \ {ResetLogLevel}
    [] candidate = ResetAfterRecordsClears /\
          Bug = "reset_after_records_keeps_stamp" ->
      spec \ {ResetLogStamp}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  \/ /\ checked < 18
     /\ checked' = checked + 1
  \/ /\ checked = 18
     /\ checked' = checked

TypeInvariant ==
  checked \in 0..18

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

AllCandidatesMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

AllSpecActionsWithinDomain ==
  \A candidate \in Candidates:
    SpecActions(candidate) \subseteq Actions

AllImplementationActionsWithinDomain ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) \subseteq Actions

ResetAnchors ==
  /\ ImplementationActions(ResetEmpty) = AllResetActions
  /\ ImplementationActions(ResetAfterRecordsClears) = AllResetActions

LabelAnchors ==
  /\ NormalLabel \in ImplementationActions(LabelNormal)
  /\ SoftLabel \in ImplementationActions(LabelSoft)
  /\ HardLabel \in ImplementationActions(LabelHard)
  /\ UnknownLabel \in ImplementationActions(LabelUnknown)
  /\ ~(UnknownLabel \in ImplementationActions(LabelNormal))
  /\ ~(NormalLabel \in ImplementationActions(LabelUnknown))

TransitionAnchors ==
  /\ LevelChangeLogs \in ImplementationActions(TransitionToSoftLogs)
  /\ SoftLabel \in ImplementationActions(TransitionToSoftLogs)
  /\ LevelChangeLogs \in ImplementationActions(TransitionToNormalLogs)
  /\ NormalLabel \in ImplementationActions(TransitionToNormalLogs)
  /\ LevelChangeLogs \in ImplementationActions(TransitionToUnknownLogs)
  /\ UnknownLabel \in ImplementationActions(TransitionToUnknownLogs)
  /\ LoggedLevelStored \in ImplementationActions(TransitionToSoftLogs)
  /\ LoggedStampStored \in ImplementationActions(TransitionToSoftLogs)

ThrottleAnchors ==
  /\ NormalRepeatSuppresses \in ImplementationActions(SameNormalSuppressed)
  /\ ElevatedBeforeIntervalSuppresses \in
       ImplementationActions(ElevatedRepeatBeforeIntervalSuppressed)
  /\ BoundaryLogs \in ImplementationActions(ElevatedRepeatAtIntervalLogs)
  /\ AfterIntervalLogs \in
       ImplementationActions(ElevatedRepeatAfterIntervalLogs)
  /\ BackwardsClockSaturates \in ImplementationActions(BackwardsClockSuppressed)
  /\ ElevatedBeforeIntervalSuppresses \in
       ImplementationActions(BackwardsClockSuppressed)
  /\ ~(BoundaryLogs \in
       ImplementationActions(ElevatedRepeatBeforeIntervalSuppressed))
  /\ ~(AfterIntervalLogs \in ImplementationActions(BackwardsClockSuppressed))

SuppressedStateAnchors ==
  /\ SuppressedLevelPreserved \in ImplementationActions(SuppressedKeepsLevel)
  /\ SuppressedStampPreserved \in ImplementationActions(SuppressedKeepsStamp)
  /\ ~(LoggedLevelStored \in ImplementationActions(SuppressedKeepsLevel))
  /\ ~(LoggedStampStored \in ImplementationActions(SuppressedKeepsStamp))
  /\ SuppressedLevelPreserved \in ImplementationActions(SameNormalSuppressed)
  /\ SuppressedStampPreserved \in ImplementationActions(SameNormalSuppressed)

LoggedStateAnchors ==
  /\ LoggedLevelStored \in ImplementationActions(LoggedUpdatesLevel)
  /\ LoggedStampStored \in ImplementationActions(LoggedUpdatesStamp)
  /\ LoggedLevelStored \in ImplementationActions(ElevatedRepeatAtIntervalLogs)
  /\ LoggedStampStored \in ImplementationActions(ElevatedRepeatAtIntervalLogs)
  /\ LoggedLevelStored \in
       ImplementationActions(ElevatedRepeatAfterIntervalLogs)
  /\ LoggedStampStored \in
       ImplementationActions(ElevatedRepeatAfterIntervalLogs)

PressureLogSafetyAnchors ==
  /\ AllCandidatesMatchSpec
  /\ AllSpecActionsWithinDomain
  /\ AllImplementationActionsWithinDomain
  /\ ResetAnchors
  /\ LabelAnchors
  /\ TransitionAnchors
  /\ ThrottleAnchors
  /\ SuppressedStateAnchors
  /\ LoggedStateAnchors

BugResetEmptyKeepsLevel ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugResetEmptyKeepsStamp ==
  ImplementationActions(ResetEmpty) = SpecActions(ResetEmpty)

BugLabelNormalUnknown ==
  ImplementationActions(LabelNormal) = SpecActions(LabelNormal)

BugLabelSoftUnknown ==
  ImplementationActions(LabelSoft) = SpecActions(LabelSoft)

BugLabelHardUnknown ==
  ImplementationActions(LabelHard) = SpecActions(LabelHard)

BugLabelUnknownNormal ==
  ImplementationActions(LabelUnknown) = SpecActions(LabelUnknown)

BugTransitionLevelChangeSuppressed ==
  ImplementationActions(TransitionToSoftLogs) =
    SpecActions(TransitionToSoftLogs)

BugTransitionToNormalSuppressed ==
  ImplementationActions(TransitionToNormalLogs) =
    SpecActions(TransitionToNormalLogs)

BugTransitionUnknownSuppressed ==
  ImplementationActions(TransitionToUnknownLogs) =
    SpecActions(TransitionToUnknownLogs)

BugSameNormalLogs ==
  ImplementationActions(SameNormalSuppressed) =
    SpecActions(SameNormalSuppressed)

BugRepeatBeforeIntervalLogs ==
  ImplementationActions(ElevatedRepeatBeforeIntervalSuppressed) =
    SpecActions(ElevatedRepeatBeforeIntervalSuppressed)

BugRepeatAtBoundarySuppressed ==
  ImplementationActions(ElevatedRepeatAtIntervalLogs) =
    SpecActions(ElevatedRepeatAtIntervalLogs)

BugRepeatAfterIntervalSuppressed ==
  ImplementationActions(ElevatedRepeatAfterIntervalLogs) =
    SpecActions(ElevatedRepeatAfterIntervalLogs)

BugBackwardsClockLogs ==
  ImplementationActions(BackwardsClockSuppressed) =
    SpecActions(BackwardsClockSuppressed)

BugSuppressedUpdatesLevel ==
  ImplementationActions(SuppressedKeepsLevel) =
    SpecActions(SuppressedKeepsLevel)

BugSuppressedUpdatesStamp ==
  ImplementationActions(SuppressedKeepsStamp) =
    SpecActions(SuppressedKeepsStamp)

BugLoggedSkipsLevelUpdate ==
  ImplementationActions(LoggedUpdatesLevel) =
    SpecActions(LoggedUpdatesLevel)

BugLoggedSkipsStampUpdate ==
  ImplementationActions(LoggedUpdatesStamp) =
    SpecActions(LoggedUpdatesStamp)

BugLoggedUpdatesStampOnly ==
  ImplementationActions(TransitionToSoftLogs) =
    SpecActions(TransitionToSoftLogs)

BugLoggedUpdatesLevelOnly ==
  ImplementationActions(ElevatedRepeatAtIntervalLogs) =
    SpecActions(ElevatedRepeatAtIntervalLogs)

BugResetAfterRecordsKeepsLevel ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

BugResetAfterRecordsKeepsStamp ==
  ImplementationActions(ResetAfterRecordsClears) =
    SpecActions(ResetAfterRecordsClears)

=============================================================================
