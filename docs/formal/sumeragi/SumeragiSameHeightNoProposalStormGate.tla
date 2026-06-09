---- MODULE SumeragiSameHeightNoProposalStormGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the same-height no-proposal storm helpers:

  * `frontier_recovery_dependency_progress_advanced(...)`
  * `reset_same_height_no_proposal_storm_if_progressed(...)`
  * `record_same_height_no_proposal_storm_timeout(...)`
  * `same_height_no_proposal_storm_count_for_height(...)`
  * `maybe_break_same_height_no_proposal_storm(...)`
  * `maybe_break_active_pending_no_proposal_storm(...)`

The model abstracts clocks and queues into finite branch cases, and pins the
deterministic recovery rules: progress is monotonic, non-progress preserves
storm ownership, true progress clears or resets state, timeout records advance
only once per new view, and the force breaker only runs for committed+1 stalls
with either pending work or explicit backlog signals.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Str;
  checked

\* @type: <<Str>>;
vars == <<checked>>

ProgressNoneToSome == "progress_none_to_some"
ProgressSomeToGreater == "progress_some_to_greater"
ProgressSomeToEqual == "progress_some_to_equal"
ProgressSomeToNone == "progress_some_to_none"

ResetNoStateNoop == "reset_no_state_noop"
ResetCommittedAdvancedClears == "reset_committed_advanced_clears"
ResetFrontierChangedClears == "reset_frontier_changed_clears"
ResetProposalSeenClears == "reset_proposal_seen_clears"
ResetDependencyAdvancedCooldownPreserves ==
  "reset_dependency_advanced_cooldown_preserves"
ResetDependencyAdvancedNoCooldownClears ==
  "reset_dependency_advanced_no_cooldown_clears"
ResetDependencyUnchangedStoresMerged ==
  "reset_dependency_unchanged_stores_merged"

RecordCreatesCountOne == "record_creates_count_one"
RecordSameViewPreservesCount == "record_same_view_preserves_count"
RecordHigherViewIncrements == "record_higher_view_increments"
RecordZeroCountSameViewIncrements == "record_zero_count_same_view_increments"
RecordDependencyMergeMax == "record_dependency_merge_max"
CountWrongHeightZero == "count_wrong_height_zero"

BreakBelowStreakSuppress == "break_below_streak_suppress"
BreakRoundLivenessSuppress == "break_round_liveness_suppress"
BreakNoPendingNoBacklogSuppress == "break_no_pending_no_backlog_suppress"
BreakNonFrontierSuppress == "break_nonfrontier_suppress"
BreakPendingThresholdForces == "break_pending_threshold_forces"
BreakBacklogNoPendingForces == "break_backlog_no_pending_forces"
BreakExistingStateMaxesView == "break_existing_state_maxes_view"
BreakAdvanceNoneReturnsFalse == "break_advance_none_returns_false"
BreakForcedStateCleans == "break_forced_state_cleans"

ActiveWrongHeightSuppress == "active_wrong_height_suppress"
ActiveRoundLivenessSuppress == "active_round_liveness_suppress"
ActiveNoViewAgeSuppress == "active_no_view_age_suppress"
ActiveNoQueueSinceSuppress == "active_no_queue_since_suppress"
ActivePacemakerBeforeQueueSuppress == "active_pacemaker_before_queue_suppress"
ActiveIdleNotTimedOutSuppress == "active_idle_not_timed_out_suppress"
ActiveTimedOutRecordsBreaks == "active_timed_out_records_breaks"

ProgressCases == {
  ProgressNoneToSome,
  ProgressSomeToGreater,
  ProgressSomeToEqual,
  ProgressSomeToNone
}

ResetCases == {
  ResetNoStateNoop,
  ResetCommittedAdvancedClears,
  ResetFrontierChangedClears,
  ResetProposalSeenClears,
  ResetDependencyAdvancedCooldownPreserves,
  ResetDependencyAdvancedNoCooldownClears,
  ResetDependencyUnchangedStoresMerged
}

RecordCases == {
  RecordCreatesCountOne,
  RecordSameViewPreservesCount,
  RecordHigherViewIncrements,
  RecordZeroCountSameViewIncrements,
  RecordDependencyMergeMax,
  CountWrongHeightZero
}

BreakCases == {
  BreakBelowStreakSuppress,
  BreakRoundLivenessSuppress,
  BreakNoPendingNoBacklogSuppress,
  BreakNonFrontierSuppress,
  BreakPendingThresholdForces,
  BreakBacklogNoPendingForces,
  BreakExistingStateMaxesView,
  BreakAdvanceNoneReturnsFalse,
  BreakForcedStateCleans
}

ActiveCases == {
  ActiveWrongHeightSuppress,
  ActiveRoundLivenessSuppress,
  ActiveNoViewAgeSuppress,
  ActiveNoQueueSinceSuppress,
  ActivePacemakerBeforeQueueSuppress,
  ActiveIdleNotTimedOutSuppress,
  ActiveTimedOutRecordsBreaks
}

Cases == ProgressCases \cup ResetCases \cup RecordCases \cup BreakCases \cup ActiveCases

ProgressAdvanced == 1
ProgressNotAdvanced == 2
NoStateNoop == 3
StateCleared == 4
StateStored == 5
StateCreated == 6
PhaseCatchUp == 7
CountZeroed == 8
CountOne == 9
CountIncremented == 10
CountPreserved == 11
CleanupCleared == 12
RotationCleared == 13
CooldownOwnerPreserved == 14
CooldownOwnerDropped == 15
DependencyMergedMax == 16
LastViewMaxed == 17
StatusObserved == 18
CountReturnedZero == 19
ForceThresholdChecked == 20
RoundLivenessChecked == 21
PendingOrBacklogChecked == 22
FrontierHeightChecked == 23
BreakSuppress == 24
BreakForce == 25
ForcedWindowsAtLeastTwo == 26
LastActionCleared == 27
AdvanceCalled == 28
AdvanceProposalSeenFalse == 29
AdvanceBacklogTrue == 30
AdvanceAllowRotationTrue == 31
AdvanceSourcePreserved == 32
AdvanceReturnedProgress == 33
AdvanceReturnedNone == 34
BreakReturnsTrue == 35
BreakReturnsFalse == 36
ActiveHeightChecked == 37
ActiveRoundLivenessChecked == 38
ActiveViewAgeChecked == 39
ActiveQueueReadyChecked == 40
ActivePacemakerChecked == 41
ActiveTimeoutChecked == 42
StormRecorded == 43
BreakCalled == 44
MissingQcSource == 45

ActionUniverse == 1..45

ClearedByProgress ==
  {ProgressAdvanced, DependencyMergedMax, PhaseCatchUp, CountZeroed,
   CleanupCleared, RotationCleared}

BreakChecks ==
  {ForceThresholdChecked, RoundLivenessChecked, PendingOrBacklogChecked,
   FrontierHeightChecked}

ForcedBreakState ==
  {BreakForce, StateStored, PhaseCatchUp, LastViewMaxed,
   ForcedWindowsAtLeastTwo, LastActionCleared, CleanupCleared}

ForcedAdvanceCall ==
  {AdvanceCalled, AdvanceProposalSeenFalse, AdvanceBacklogTrue,
   AdvanceAllowRotationTrue, AdvanceSourcePreserved}

ActiveChecks ==
  {ActiveHeightChecked, ActiveRoundLivenessChecked, ActiveViewAgeChecked,
   ActiveQueueReadyChecked, ActivePacemakerChecked, ActiveTimeoutChecked}

SpecActions(c) ==
  CASE c = ProgressNoneToSome ->
      {ProgressAdvanced}
    [] c = ProgressSomeToGreater ->
      {ProgressAdvanced}
    [] c = ProgressSomeToEqual ->
      {ProgressNotAdvanced}
    [] c = ProgressSomeToNone ->
      {ProgressNotAdvanced}
    [] c = ResetNoStateNoop ->
      {NoStateNoop}
    [] c = ResetCommittedAdvancedClears ->
      {StateCleared}
    [] c = ResetFrontierChangedClears ->
      {StateCleared}
    [] c = ResetProposalSeenClears ->
      {StateCleared}
    [] c = ResetDependencyAdvancedCooldownPreserves ->
      ClearedByProgress \cup {CooldownOwnerPreserved, StateStored}
    [] c = ResetDependencyAdvancedNoCooldownClears ->
      ClearedByProgress \cup {CooldownOwnerDropped, StateCleared}
    [] c = ResetDependencyUnchangedStoresMerged ->
      {ProgressNotAdvanced, DependencyMergedMax, StateStored}
    [] c = RecordCreatesCountOne ->
      {StateCreated, CountOne, CountIncremented, DependencyMergedMax,
       StatusObserved, StateStored}
    [] c = RecordSameViewPreservesCount ->
      {CountPreserved, DependencyMergedMax, StatusObserved, StateStored}
    [] c = RecordHigherViewIncrements ->
      {LastViewMaxed, CountIncremented, DependencyMergedMax, StatusObserved,
       StateStored}
    [] c = RecordZeroCountSameViewIncrements ->
      {CountOne, CountIncremented, DependencyMergedMax, StatusObserved,
       StateStored}
    [] c = RecordDependencyMergeMax ->
      {DependencyMergedMax, CountPreserved, StatusObserved, StateStored}
    [] c = CountWrongHeightZero ->
      {CountReturnedZero}
    [] c = BreakBelowStreakSuppress ->
      {ForceThresholdChecked, BreakSuppress, BreakReturnsFalse}
    [] c = BreakRoundLivenessSuppress ->
      {ForceThresholdChecked, RoundLivenessChecked, BreakSuppress,
       BreakReturnsFalse}
    [] c = BreakNoPendingNoBacklogSuppress ->
      {ForceThresholdChecked, RoundLivenessChecked, PendingOrBacklogChecked,
       BreakSuppress, BreakReturnsFalse}
    [] c = BreakNonFrontierSuppress ->
      BreakChecks \cup {BreakSuppress, BreakReturnsFalse}
    [] c = BreakPendingThresholdForces ->
      BreakChecks \cup ForcedBreakState \cup ForcedAdvanceCall
        \cup {AdvanceReturnedProgress, BreakReturnsTrue}
    [] c = BreakBacklogNoPendingForces ->
      BreakChecks \cup ForcedBreakState \cup ForcedAdvanceCall
        \cup {AdvanceReturnedProgress, BreakReturnsTrue}
    [] c = BreakExistingStateMaxesView ->
      BreakChecks \cup ForcedBreakState \cup ForcedAdvanceCall
        \cup {AdvanceReturnedProgress, BreakReturnsTrue}
    [] c = BreakAdvanceNoneReturnsFalse ->
      BreakChecks \cup ForcedBreakState \cup ForcedAdvanceCall
        \cup {AdvanceReturnedNone, BreakReturnsFalse}
    [] c = BreakForcedStateCleans ->
      BreakChecks \cup ForcedBreakState \cup ForcedAdvanceCall
        \cup {AdvanceReturnedProgress, BreakReturnsTrue}
    [] c = ActiveWrongHeightSuppress ->
      {ActiveHeightChecked, BreakSuppress, BreakReturnsFalse}
    [] c = ActiveRoundLivenessSuppress ->
      {ActiveHeightChecked, ActiveRoundLivenessChecked, BreakSuppress,
       BreakReturnsFalse}
    [] c = ActiveNoViewAgeSuppress ->
      {ActiveHeightChecked, ActiveRoundLivenessChecked, ActiveViewAgeChecked,
       BreakSuppress, BreakReturnsFalse}
    [] c = ActiveNoQueueSinceSuppress ->
      {ActiveHeightChecked, ActiveRoundLivenessChecked, ActiveViewAgeChecked,
       ActiveQueueReadyChecked, BreakSuppress, BreakReturnsFalse}
    [] c = ActivePacemakerBeforeQueueSuppress ->
      {ActiveHeightChecked, ActiveRoundLivenessChecked, ActiveViewAgeChecked,
       ActiveQueueReadyChecked, ActivePacemakerChecked, BreakSuppress,
       BreakReturnsFalse}
    [] c = ActiveIdleNotTimedOutSuppress ->
      ActiveChecks \cup {BreakSuppress, BreakReturnsFalse}
    [] c = ActiveTimedOutRecordsBreaks ->
      ActiveChecks \cup {StormRecorded, BreakCalled, MissingQcSource,
       BreakReturnsTrue}
    [] OTHER -> {}

ImplementationActions(c) ==
  CASE Bug = "progress_none_some_not_advanced"
       /\ c = ProgressNoneToSome ->
      {ProgressNotAdvanced}
    [] Bug = "progress_equal_advanced"
       /\ c = ProgressSomeToEqual ->
      {ProgressAdvanced}
    [] Bug = "reset_committed_keeps_state"
       /\ c = ResetCommittedAdvancedClears ->
      {StateStored}
    [] Bug = "reset_frontier_changed_keeps_state"
       /\ c = ResetFrontierChangedClears ->
      {StateStored}
    [] Bug = "reset_proposal_seen_keeps_state"
       /\ c = ResetProposalSeenClears ->
      {StateStored}
    [] Bug = "reset_dependency_progress_keeps_count"
       /\ c = ResetDependencyAdvancedCooldownPreserves ->
      SpecActions(c) \ {CountZeroed}
    [] Bug = "reset_dependency_progress_drops_cooldown_owner"
       /\ c = ResetDependencyAdvancedCooldownPreserves ->
      (SpecActions(c) \ {CooldownOwnerPreserved, StateStored})
        \cup {CooldownOwnerDropped, StateCleared}
    [] Bug = "reset_dependency_progress_no_cooldown_keeps_state"
       /\ c = ResetDependencyAdvancedNoCooldownClears ->
      (SpecActions(c) \ {CooldownOwnerDropped, StateCleared})
        \cup {CooldownOwnerPreserved, StateStored}
    [] Bug = "reset_unchanged_clears_state"
       /\ c = ResetDependencyUnchangedStoresMerged ->
      (SpecActions(c) \ {StateStored}) \cup {StateCleared}
    [] Bug = "record_create_count_zero"
       /\ c = RecordCreatesCountOne ->
      (SpecActions(c) \ {CountOne, CountIncremented}) \cup {CountZeroed}
    [] Bug = "record_same_view_increments"
       /\ c = RecordSameViewPreservesCount ->
      (SpecActions(c) \ {CountPreserved}) \cup {CountIncremented}
    [] Bug = "record_higher_view_not_incremented"
       /\ c = RecordHigherViewIncrements ->
      (SpecActions(c) \ {CountIncremented}) \cup {CountPreserved}
    [] Bug = "record_zero_count_same_view_not_incremented"
       /\ c = RecordZeroCountSameViewIncrements ->
      (SpecActions(c) \ {CountOne, CountIncremented}) \cup {CountZeroed}
    [] Bug = "record_dependency_not_maxed"
       /\ c = RecordDependencyMergeMax ->
      SpecActions(c) \ {DependencyMergedMax}
    [] Bug = "count_wrong_height_returns_count"
       /\ c = CountWrongHeightZero ->
      {CountPreserved}
    [] Bug = "break_below_streak_forces"
       /\ c = BreakBelowStreakSuppress ->
      (SpecActions(c) \ {BreakSuppress, BreakReturnsFalse})
        \cup ForcedBreakState \cup ForcedAdvanceCall \cup {BreakReturnsTrue}
    [] Bug = "break_liveness_forces"
       /\ c = BreakRoundLivenessSuppress ->
      (SpecActions(c) \ {BreakSuppress, BreakReturnsFalse})
        \cup ForcedBreakState \cup ForcedAdvanceCall \cup {BreakReturnsTrue}
    [] Bug = "break_no_pending_no_backlog_forces"
       /\ c = BreakNoPendingNoBacklogSuppress ->
      (SpecActions(c) \ {BreakSuppress, BreakReturnsFalse})
        \cup ForcedBreakState \cup ForcedAdvanceCall \cup {BreakReturnsTrue}
    [] Bug = "break_nonfrontier_forces"
       /\ c = BreakNonFrontierSuppress ->
      (SpecActions(c) \ {BreakSuppress, BreakReturnsFalse})
        \cup ForcedBreakState \cup ForcedAdvanceCall \cup {BreakReturnsTrue}
    [] Bug = "break_pending_threshold_suppresses"
       /\ c = BreakPendingThresholdForces ->
      (SpecActions(c) \ {BreakForce, AdvanceCalled, BreakReturnsTrue})
        \cup {BreakSuppress, BreakReturnsFalse}
    [] Bug = "break_backlog_no_pending_suppresses"
       /\ c = BreakBacklogNoPendingForces ->
      (SpecActions(c) \ {BreakForce, AdvanceCalled, BreakReturnsTrue})
        \cup {BreakSuppress, BreakReturnsFalse}
    [] Bug = "break_does_not_force_windows"
       /\ c = BreakPendingThresholdForces ->
      SpecActions(c) \ {ForcedWindowsAtLeastTwo}
    [] Bug = "break_keeps_last_action"
       /\ c = BreakForcedStateCleans ->
      SpecActions(c) \ {LastActionCleared}
    [] Bug = "break_keeps_cleanup"
       /\ c = BreakForcedStateCleans ->
      SpecActions(c) \ {CleanupCleared}
    [] Bug = "break_uses_previous_phase"
       /\ c = BreakForcedStateCleans ->
      SpecActions(c) \ {PhaseCatchUp}
    [] Bug = "break_does_not_call_advance"
       /\ c = BreakPendingThresholdForces ->
      SpecActions(c) \ {AdvanceCalled}
    [] Bug = "break_wrong_advance_flags"
       /\ c = BreakPendingThresholdForces ->
      SpecActions(c) \ {AdvanceProposalSeenFalse, AdvanceBacklogTrue,
       AdvanceAllowRotationTrue}
    [] Bug = "break_false_when_advance_progresses"
       /\ c = BreakPendingThresholdForces ->
      (SpecActions(c) \ {BreakReturnsTrue}) \cup {BreakReturnsFalse}
    [] Bug = "break_true_when_advance_none"
       /\ c = BreakAdvanceNoneReturnsFalse ->
      (SpecActions(c) \ {BreakReturnsFalse}) \cup {BreakReturnsTrue}
    [] Bug = "active_wrong_height_records"
       /\ c = ActiveWrongHeightSuppress ->
      (SpecActions(c) \ {BreakSuppress, BreakReturnsFalse})
        \cup {StormRecorded, BreakCalled, BreakReturnsTrue}
    [] Bug = "active_liveness_records"
       /\ c = ActiveRoundLivenessSuppress ->
      (SpecActions(c) \ {BreakSuppress, BreakReturnsFalse})
        \cup {StormRecorded, BreakCalled, BreakReturnsTrue}
    [] Bug = "active_missing_queue_records"
       /\ c = ActiveNoQueueSinceSuppress ->
      (SpecActions(c) \ {BreakSuppress, BreakReturnsFalse})
        \cup {StormRecorded, BreakCalled, BreakReturnsTrue}
    [] Bug = "active_pacemaker_before_queue_records"
       /\ c = ActivePacemakerBeforeQueueSuppress ->
      (SpecActions(c) \ {BreakSuppress, BreakReturnsFalse})
        \cup {StormRecorded, BreakCalled, BreakReturnsTrue}
    [] Bug = "active_not_timed_out_records"
       /\ c = ActiveIdleNotTimedOutSuppress ->
      (SpecActions(c) \ {BreakSuppress, BreakReturnsFalse})
        \cup {StormRecorded, BreakCalled, BreakReturnsTrue}
    [] Bug = "active_timed_out_skips_record"
       /\ c = ActiveTimedOutRecordsBreaks ->
      SpecActions(c) \ {StormRecorded}
    [] Bug = "active_timed_out_wrong_source"
       /\ c = ActiveTimedOutRecordsBreaks ->
      SpecActions(c) \ {MissingQcSource}
    [] OTHER -> SpecActions(c)

Init ==
  checked \in Cases

Next ==
  UNCHANGED checked

TypeInvariant ==
  /\ checked \in Cases
  /\ \A c \in Cases : SpecActions(c) \subseteq ActionUniverse
  /\ \A c \in Cases : ImplementationActions(c) \subseteq ActionUniverse

ProgressSafety ==
  \A c \in ProgressCases : ImplementationActions(c) = SpecActions(c)

ResetSafety ==
  \A c \in ResetCases : ImplementationActions(c) = SpecActions(c)

RecordSafety ==
  \A c \in RecordCases : ImplementationActions(c) = SpecActions(c)

BreakSafety ==
  \A c \in BreakCases : ImplementationActions(c) = SpecActions(c)

ActivePendingSafety ==
  \A c \in ActiveCases : ImplementationActions(c) = SpecActions(c)

SameHeightNoProposalStormCoreSafety ==
  /\ ProgressSafety
  /\ ResetSafety
  /\ RecordSafety
  /\ BreakSafety
  /\ ActivePendingSafety

SafetyFast ==
  SameHeightNoProposalStormCoreSafety

ProgressAnchors ==
  /\ ProgressSafety
  /\ ImplementationActions(ProgressNoneToSome) = SpecActions(ProgressNoneToSome)
  /\ ImplementationActions(ProgressSomeToGreater) = SpecActions(ProgressSomeToGreater)
  /\ ImplementationActions(ProgressSomeToEqual) = SpecActions(ProgressSomeToEqual)
  /\ ImplementationActions(ProgressSomeToNone) = SpecActions(ProgressSomeToNone)

ResetAnchors ==
  /\ ResetSafety
  /\ ImplementationActions(ResetNoStateNoop) = SpecActions(ResetNoStateNoop)
  /\ ImplementationActions(ResetCommittedAdvancedClears) =
       SpecActions(ResetCommittedAdvancedClears)
  /\ ImplementationActions(ResetFrontierChangedClears) =
       SpecActions(ResetFrontierChangedClears)
  /\ ImplementationActions(ResetProposalSeenClears) =
       SpecActions(ResetProposalSeenClears)
  /\ ImplementationActions(ResetDependencyAdvancedCooldownPreserves) =
       SpecActions(ResetDependencyAdvancedCooldownPreserves)
  /\ ImplementationActions(ResetDependencyAdvancedNoCooldownClears) =
       SpecActions(ResetDependencyAdvancedNoCooldownClears)
  /\ ImplementationActions(ResetDependencyUnchangedStoresMerged) =
       SpecActions(ResetDependencyUnchangedStoresMerged)

RecordAnchors ==
  /\ RecordSafety
  /\ ImplementationActions(RecordCreatesCountOne) = SpecActions(RecordCreatesCountOne)
  /\ ImplementationActions(RecordSameViewPreservesCount) =
       SpecActions(RecordSameViewPreservesCount)
  /\ ImplementationActions(RecordHigherViewIncrements) =
       SpecActions(RecordHigherViewIncrements)
  /\ ImplementationActions(RecordZeroCountSameViewIncrements) =
       SpecActions(RecordZeroCountSameViewIncrements)
  /\ ImplementationActions(RecordDependencyMergeMax) = SpecActions(RecordDependencyMergeMax)
  /\ ImplementationActions(CountWrongHeightZero) = SpecActions(CountWrongHeightZero)

BreakAnchors ==
  /\ BreakSafety
  /\ ImplementationActions(BreakBelowStreakSuppress) = SpecActions(BreakBelowStreakSuppress)
  /\ ImplementationActions(BreakRoundLivenessSuppress) =
       SpecActions(BreakRoundLivenessSuppress)
  /\ ImplementationActions(BreakNoPendingNoBacklogSuppress) =
       SpecActions(BreakNoPendingNoBacklogSuppress)
  /\ ImplementationActions(BreakNonFrontierSuppress) = SpecActions(BreakNonFrontierSuppress)
  /\ ImplementationActions(BreakPendingThresholdForces) =
       SpecActions(BreakPendingThresholdForces)
  /\ ImplementationActions(BreakBacklogNoPendingForces) =
       SpecActions(BreakBacklogNoPendingForces)
  /\ ImplementationActions(BreakExistingStateMaxesView) =
       SpecActions(BreakExistingStateMaxesView)
  /\ ImplementationActions(BreakAdvanceNoneReturnsFalse) =
       SpecActions(BreakAdvanceNoneReturnsFalse)
  /\ ImplementationActions(BreakForcedStateCleans) = SpecActions(BreakForcedStateCleans)

ActivePendingAnchors ==
  /\ ActivePendingSafety
  /\ ImplementationActions(ActiveWrongHeightSuppress) =
       SpecActions(ActiveWrongHeightSuppress)
  /\ ImplementationActions(ActiveRoundLivenessSuppress) =
       SpecActions(ActiveRoundLivenessSuppress)
  /\ ImplementationActions(ActiveNoViewAgeSuppress) =
       SpecActions(ActiveNoViewAgeSuppress)
  /\ ImplementationActions(ActiveNoQueueSinceSuppress) =
       SpecActions(ActiveNoQueueSinceSuppress)
  /\ ImplementationActions(ActivePacemakerBeforeQueueSuppress) =
       SpecActions(ActivePacemakerBeforeQueueSuppress)
  /\ ImplementationActions(ActiveIdleNotTimedOutSuppress) =
       SpecActions(ActiveIdleNotTimedOutSuppress)
  /\ ImplementationActions(ActiveTimedOutRecordsBreaks) =
       SpecActions(ActiveTimedOutRecordsBreaks)

SameHeightNoProposalStormSafetyAnchors ==
  /\ ProgressAnchors
  /\ ResetAnchors
  /\ RecordAnchors
  /\ BreakAnchors
  /\ ActivePendingAnchors

Safety ==
  SameHeightNoProposalStormSafetyAnchors

====
