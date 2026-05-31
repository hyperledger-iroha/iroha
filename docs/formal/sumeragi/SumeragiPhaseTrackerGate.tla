---- MODULE SumeragiPhaseTrackerGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the Sumeragi `PhaseTracker` helper.

This slice pins the mutable state transitions in `main_loop.rs`: construction,
round start, view-change reset, phase recording, duplicate phase suppression,
view-age lookup, and current-view lookup. Concrete `Instant` values and phase
flags are collapsed into observable state actions so the model stays small
while preserving the helper contracts used by the main loop.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

NewHasNoHeight == 1
NewViewZero == 2
NewTimestampsNow == 3
StartSetsHeight == 4
StartResetsViewZero == 5
StartResetsTimers == 6
StartClearsFlags == 7
ViewChangeSameHeightSetsView == 8
ViewChangeNewHeightSetsHeight == 9
ViewChangeResetsTimers == 10
ViewChangeClearsFlags == 11
RecordNoRoundReturnsNone == 12
RecordWrongHeightReturnsNone == 13
RecordNewViewResetsRound == 14
RecordNewViewClearsFlags == 15
RecordDuplicateReturnsNone == 16
RecordFirstReturnsDuration == 17
RecordFirstMarksPhase == 18
RecordFirstUpdatesLastMarker == 19
RecordNextPhaseUsesLastMarker == 20
ViewAgeWrongHeightReturnsNone == 21
ViewAgeMatchingHeightReturnsDuration == 22
CurrentViewWrongHeightReturnsNone == 23
CurrentViewMatchingHeightReturnsView == 24

Candidates == 1..24

HeightNone == 1
SetHeightInput == 2
SetViewZero == 3
SetViewInput == 4
SetRoundStartNow == 5
ResetLastMarkerNow == 6
ResetFlags == 7
ReturnNone == 8
ReturnDurationSinceLastMarker == 9
ReturnDurationSinceRoundStart == 10
MarkPhase == 11
UpdateLastMarkerNow == 12
ReturnView == 13
PreserveRecordedFlags == 14
PreserveLastMarker == 15
PreserveView == 16
PreserveHeight == 17

Actions == 1..17

SpecActions(candidate) ==
  CASE candidate = NewHasNoHeight -> {HeightNone}
    [] candidate = NewViewZero -> {SetViewZero}
    [] candidate = NewTimestampsNow -> {SetRoundStartNow, ResetLastMarkerNow}
    [] candidate = StartSetsHeight -> {SetHeightInput}
    [] candidate = StartResetsViewZero -> {SetViewZero}
    [] candidate = StartResetsTimers -> {SetRoundStartNow, ResetLastMarkerNow}
    [] candidate = StartClearsFlags -> {ResetFlags}
    [] candidate = ViewChangeSameHeightSetsView -> {SetViewInput}
    [] candidate = ViewChangeNewHeightSetsHeight ->
      {SetHeightInput, SetViewInput}
    [] candidate = ViewChangeResetsTimers ->
      {SetRoundStartNow, ResetLastMarkerNow}
    [] candidate = ViewChangeClearsFlags -> {ResetFlags}
    [] candidate = RecordNoRoundReturnsNone ->
      {ReturnNone, PreserveHeight, PreserveView, PreserveLastMarker,
       PreserveRecordedFlags}
    [] candidate = RecordWrongHeightReturnsNone ->
      {ReturnNone, PreserveHeight, PreserveView, PreserveLastMarker,
       PreserveRecordedFlags}
    [] candidate = RecordNewViewResetsRound ->
      {SetViewInput, SetRoundStartNow, ResetLastMarkerNow,
       ReturnDurationSinceLastMarker, MarkPhase, UpdateLastMarkerNow}
    [] candidate = RecordNewViewClearsFlags ->
      {ResetFlags, ReturnDurationSinceLastMarker, MarkPhase,
       UpdateLastMarkerNow}
    [] candidate = RecordDuplicateReturnsNone ->
      {ReturnNone, PreserveRecordedFlags, PreserveLastMarker}
    [] candidate = RecordFirstReturnsDuration ->
      {ReturnDurationSinceLastMarker}
    [] candidate = RecordFirstMarksPhase ->
      {ReturnDurationSinceLastMarker, MarkPhase, UpdateLastMarkerNow}
    [] candidate = RecordFirstUpdatesLastMarker ->
      {ReturnDurationSinceLastMarker, MarkPhase, UpdateLastMarkerNow}
    [] candidate = RecordNextPhaseUsesLastMarker ->
      {ReturnDurationSinceLastMarker, MarkPhase, UpdateLastMarkerNow}
    [] candidate = ViewAgeWrongHeightReturnsNone -> {ReturnNone}
    [] candidate = ViewAgeMatchingHeightReturnsDuration ->
      {ReturnDurationSinceRoundStart}
    [] candidate = CurrentViewWrongHeightReturnsNone -> {ReturnNone}
    [] candidate = CurrentViewMatchingHeightReturnsView -> {ReturnView}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = NewHasNoHeight /\ Bug = "new_has_height" ->
      (spec \ {HeightNone}) \cup {SetHeightInput}
    [] candidate = NewViewZero /\ Bug = "new_view_nonzero" ->
      (spec \ {SetViewZero}) \cup {SetViewInput}
    [] candidate = NewTimestampsNow /\ Bug = "new_timers_not_now" ->
      spec \ {SetRoundStartNow, ResetLastMarkerNow}
    [] candidate = StartSetsHeight /\ Bug = "start_missing_height" ->
      spec \ {SetHeightInput}
    [] candidate = StartResetsViewZero /\ Bug = "start_keeps_view" ->
      (spec \ {SetViewZero}) \cup {PreserveView}
    [] candidate = StartResetsTimers /\ Bug = "start_keeps_old_timer" ->
      spec \ {SetRoundStartNow, ResetLastMarkerNow}
    [] candidate = StartClearsFlags /\ Bug = "start_keeps_recorded" ->
      (spec \ {ResetFlags}) \cup {PreserveRecordedFlags}
    [] candidate = ViewChangeSameHeightSetsView /\
          Bug = "view_change_same_height_ignores_view" ->
      (spec \ {SetViewInput}) \cup {PreserveView}
    [] candidate = ViewChangeNewHeightSetsHeight /\
          Bug = "view_change_new_height_keeps_old_height" ->
      (spec \ {SetHeightInput}) \cup {PreserveHeight}
    [] candidate = ViewChangeResetsTimers /\
          Bug = "view_change_skips_timer_reset" ->
      spec \ {SetRoundStartNow, ResetLastMarkerNow}
    [] candidate = ViewChangeClearsFlags /\
          Bug = "view_change_keeps_recorded" ->
      (spec \ {ResetFlags}) \cup {PreserveRecordedFlags}
    [] candidate = RecordNoRoundReturnsNone /\ Bug = "record_no_round_some" ->
      (spec \ {ReturnNone}) \cup {ReturnDurationSinceLastMarker}
    [] candidate = RecordWrongHeightReturnsNone /\
          Bug = "record_wrong_height_some" ->
      (spec \ {ReturnNone}) \cup {ReturnDurationSinceLastMarker}
    [] candidate = RecordNewViewResetsRound /\
          Bug = "record_new_view_keeps_old_view" ->
      (spec \ {SetViewInput}) \cup {PreserveView}
    [] candidate = RecordNewViewClearsFlags /\
          Bug = "record_new_view_keeps_flags" ->
      (spec \ {ResetFlags}) \cup {PreserveRecordedFlags}
    [] candidate = RecordDuplicateReturnsNone /\
          Bug = "record_duplicate_returns_duration" ->
      (spec \ {ReturnNone}) \cup {ReturnDurationSinceLastMarker}
    [] candidate = RecordFirstReturnsDuration /\
          Bug = "record_first_returns_none" ->
      (spec \ {ReturnDurationSinceLastMarker}) \cup {ReturnNone}
    [] candidate = RecordFirstMarksPhase /\
          Bug = "record_first_does_not_mark_phase" ->
      spec \ {MarkPhase}
    [] candidate = RecordFirstUpdatesLastMarker /\
          Bug = "record_first_skips_last_marker" ->
      spec \ {UpdateLastMarkerNow}
    [] candidate = RecordNextPhaseUsesLastMarker /\
          Bug = "record_next_phase_uses_round_start" ->
      (spec \ {ReturnDurationSinceLastMarker}) \cup
        {ReturnDurationSinceRoundStart}
    [] candidate = ViewAgeWrongHeightReturnsNone /\
          Bug = "view_age_wrong_height_some" ->
      (spec \ {ReturnNone}) \cup {ReturnDurationSinceRoundStart}
    [] candidate = ViewAgeMatchingHeightReturnsDuration /\
          Bug = "view_age_matching_height_none" ->
      (spec \ {ReturnDurationSinceRoundStart}) \cup {ReturnNone}
    [] candidate = CurrentViewWrongHeightReturnsNone /\
          Bug = "current_view_wrong_height_some" ->
      (spec \ {ReturnNone}) \cup {ReturnView}
    [] candidate = CurrentViewMatchingHeightReturnsView /\
          Bug = "current_view_matching_height_none" ->
      (spec \ {ReturnView}) \cup {ReturnNone}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "new_has_height",
       "new_view_nonzero",
       "new_timers_not_now",
       "start_missing_height",
       "start_keeps_view",
       "start_keeps_old_timer",
       "start_keeps_recorded",
       "view_change_same_height_ignores_view",
       "view_change_new_height_keeps_old_height",
       "view_change_skips_timer_reset",
       "view_change_keeps_recorded",
       "record_no_round_some",
       "record_wrong_height_some",
       "record_new_view_keeps_old_view",
       "record_new_view_keeps_flags",
       "record_duplicate_returns_duration",
       "record_first_returns_none",
       "record_first_does_not_mark_phase",
       "record_first_skips_last_marker",
       "record_next_phase_uses_round_start",
       "view_age_wrong_height_some",
       "view_age_matching_height_none",
       "current_view_wrong_height_some",
       "current_view_matching_height_none"
     }
  /\ checked = 0
  /\ \A c \in Candidates:
       /\ SpecActions(c) \subseteq Actions
       /\ ImplementationActions(c) \subseteq Actions

PhaseTrackerMatchesSpec ==
  \A c \in Candidates:
    ImplementationActions(c) = SpecActions(c)

NewTrackerMatchesSpec ==
  \A c \in {
    NewHasNoHeight,
    NewViewZero,
    NewTimestampsNow
  }:
    ImplementationActions(c) = SpecActions(c)

StartRoundMatchesSpec ==
  \A c \in {
    StartSetsHeight,
    StartResetsViewZero,
    StartResetsTimers,
    StartClearsFlags
  }:
    ImplementationActions(c) = SpecActions(c)

ViewChangeMatchesSpec ==
  \A c \in {
    ViewChangeSameHeightSetsView,
    ViewChangeNewHeightSetsHeight,
    ViewChangeResetsTimers,
    ViewChangeClearsFlags
  }:
    ImplementationActions(c) = SpecActions(c)

RecordRejectMatchesSpec ==
  \A c \in {
    RecordNoRoundReturnsNone,
    RecordWrongHeightReturnsNone
  }:
    ImplementationActions(c) = SpecActions(c)

RecordNewViewMatchesSpec ==
  \A c \in {
    RecordNewViewResetsRound,
    RecordNewViewClearsFlags
  }:
    ImplementationActions(c) = SpecActions(c)

RecordPhaseMatchesSpec ==
  \A c \in {
    RecordDuplicateReturnsNone,
    RecordFirstReturnsDuration,
    RecordFirstMarksPhase,
    RecordFirstUpdatesLastMarker,
    RecordNextPhaseUsesLastMarker
  }:
    ImplementationActions(c) = SpecActions(c)

LookupMatchesSpec ==
  \A c \in {
    ViewAgeWrongHeightReturnsNone,
    ViewAgeMatchingHeightReturnsDuration,
    CurrentViewWrongHeightReturnsNone,
    CurrentViewMatchingHeightReturnsView
  }:
    ImplementationActions(c) = SpecActions(c)

TimerResetPairsRoundStartAndMarker ==
  \A c \in Candidates:
    SetRoundStartNow \in ImplementationActions(c) =>
      ResetLastMarkerNow \in ImplementationActions(c)

RecordedDurationMarksAndUpdates ==
  \A c \in Candidates:
    ReturnDurationSinceLastMarker \in ImplementationActions(c) =>
      \/ c = RecordFirstReturnsDuration
      \/ /\ MarkPhase \in ImplementationActions(c)
         /\ UpdateLastMarkerNow \in ImplementationActions(c)

DuplicateRecordPreservesMarkerAndFlags ==
  /\ ReturnNone \in ImplementationActions(RecordDuplicateReturnsNone)
  /\ PreserveRecordedFlags \in ImplementationActions(RecordDuplicateReturnsNone)
  /\ PreserveLastMarker \in ImplementationActions(RecordDuplicateReturnsNone)

NewTrackerAnchors ==
  /\ SpecActions(NewHasNoHeight) = {HeightNone}
  /\ SpecActions(NewViewZero) = {SetViewZero}
  /\ SpecActions(NewTimestampsNow) =
       {SetRoundStartNow, ResetLastMarkerNow}

StartRoundAnchors ==
  /\ SpecActions(StartSetsHeight) = {SetHeightInput}
  /\ SpecActions(StartResetsViewZero) = {SetViewZero}
  /\ SpecActions(StartResetsTimers) =
       {SetRoundStartNow, ResetLastMarkerNow}
  /\ SpecActions(StartClearsFlags) = {ResetFlags}

ViewChangeAnchors ==
  /\ SpecActions(ViewChangeSameHeightSetsView) = {SetViewInput}
  /\ SpecActions(ViewChangeNewHeightSetsHeight) =
       {SetHeightInput, SetViewInput}
  /\ SpecActions(ViewChangeResetsTimers) =
       {SetRoundStartNow, ResetLastMarkerNow}
  /\ SpecActions(ViewChangeClearsFlags) = {ResetFlags}

RecordRejectAnchors ==
  /\ SpecActions(RecordNoRoundReturnsNone) =
       {ReturnNone, PreserveHeight, PreserveView, PreserveLastMarker,
        PreserveRecordedFlags}
  /\ SpecActions(RecordWrongHeightReturnsNone) =
       {ReturnNone, PreserveHeight, PreserveView, PreserveLastMarker,
        PreserveRecordedFlags}

RecordNewViewAnchors ==
  /\ SpecActions(RecordNewViewResetsRound) =
       {SetViewInput, SetRoundStartNow, ResetLastMarkerNow,
        ReturnDurationSinceLastMarker, MarkPhase, UpdateLastMarkerNow}
  /\ SpecActions(RecordNewViewClearsFlags) =
       {ResetFlags, ReturnDurationSinceLastMarker, MarkPhase,
        UpdateLastMarkerNow}

RecordPhaseAnchors ==
  /\ SpecActions(RecordDuplicateReturnsNone) =
       {ReturnNone, PreserveRecordedFlags, PreserveLastMarker}
  /\ SpecActions(RecordFirstReturnsDuration) =
       {ReturnDurationSinceLastMarker}
  /\ SpecActions(RecordFirstMarksPhase) =
       {ReturnDurationSinceLastMarker, MarkPhase, UpdateLastMarkerNow}
  /\ SpecActions(RecordFirstUpdatesLastMarker) =
       {ReturnDurationSinceLastMarker, MarkPhase, UpdateLastMarkerNow}
  /\ SpecActions(RecordNextPhaseUsesLastMarker) =
       {ReturnDurationSinceLastMarker, MarkPhase, UpdateLastMarkerNow}

LookupAnchors ==
  /\ SpecActions(ViewAgeWrongHeightReturnsNone) = {ReturnNone}
  /\ SpecActions(ViewAgeMatchingHeightReturnsDuration) =
       {ReturnDurationSinceRoundStart}
  /\ SpecActions(CurrentViewWrongHeightReturnsNone) = {ReturnNone}
  /\ SpecActions(CurrentViewMatchingHeightReturnsView) = {ReturnView}

Safety ==
  /\ PhaseTrackerMatchesSpec
  /\ NewTrackerMatchesSpec
  /\ StartRoundMatchesSpec
  /\ ViewChangeMatchesSpec
  /\ RecordRejectMatchesSpec
  /\ RecordNewViewMatchesSpec
  /\ RecordPhaseMatchesSpec
  /\ LookupMatchesSpec
  /\ TimerResetPairsRoundStartAndMarker
  /\ RecordedDurationMarksAndUpdates
  /\ DuplicateRecordPreservesMarkerAndFlags
  /\ NewTrackerAnchors
  /\ StartRoundAnchors
  /\ ViewChangeAnchors
  /\ RecordRejectAnchors
  /\ RecordNewViewAnchors
  /\ RecordPhaseAnchors
  /\ LookupAnchors

BugNewHasHeight ==
  ImplementationActions(NewHasNoHeight) = SpecActions(NewHasNoHeight)

BugNewViewNonzero ==
  ImplementationActions(NewViewZero) = SpecActions(NewViewZero)

BugNewTimersNotNow ==
  ImplementationActions(NewTimestampsNow) = SpecActions(NewTimestampsNow)

BugStartMissingHeight ==
  ImplementationActions(StartSetsHeight) = SpecActions(StartSetsHeight)

BugStartKeepsView ==
  ImplementationActions(StartResetsViewZero) = SpecActions(StartResetsViewZero)

BugStartKeepsOldTimer ==
  ImplementationActions(StartResetsTimers) = SpecActions(StartResetsTimers)

BugStartKeepsRecorded ==
  ImplementationActions(StartClearsFlags) = SpecActions(StartClearsFlags)

BugViewChangeSameHeightIgnoresView ==
  ImplementationActions(ViewChangeSameHeightSetsView) =
    SpecActions(ViewChangeSameHeightSetsView)

BugViewChangeNewHeightKeepsOldHeight ==
  ImplementationActions(ViewChangeNewHeightSetsHeight) =
    SpecActions(ViewChangeNewHeightSetsHeight)

BugViewChangeSkipsTimerReset ==
  ImplementationActions(ViewChangeResetsTimers) =
    SpecActions(ViewChangeResetsTimers)

BugViewChangeKeepsRecorded ==
  ImplementationActions(ViewChangeClearsFlags) =
    SpecActions(ViewChangeClearsFlags)

BugRecordNoRoundSome ==
  ImplementationActions(RecordNoRoundReturnsNone) =
    SpecActions(RecordNoRoundReturnsNone)

BugRecordWrongHeightSome ==
  ImplementationActions(RecordWrongHeightReturnsNone) =
    SpecActions(RecordWrongHeightReturnsNone)

BugRecordNewViewKeepsOldView ==
  ImplementationActions(RecordNewViewResetsRound) =
    SpecActions(RecordNewViewResetsRound)

BugRecordNewViewKeepsFlags ==
  ImplementationActions(RecordNewViewClearsFlags) =
    SpecActions(RecordNewViewClearsFlags)

BugRecordDuplicateReturnsDuration ==
  ImplementationActions(RecordDuplicateReturnsNone) =
    SpecActions(RecordDuplicateReturnsNone)

BugRecordFirstReturnsNone ==
  ImplementationActions(RecordFirstReturnsDuration) =
    SpecActions(RecordFirstReturnsDuration)

BugRecordFirstDoesNotMarkPhase ==
  ImplementationActions(RecordFirstMarksPhase) =
    SpecActions(RecordFirstMarksPhase)

BugRecordFirstSkipsLastMarker ==
  ImplementationActions(RecordFirstUpdatesLastMarker) =
    SpecActions(RecordFirstUpdatesLastMarker)

BugRecordNextPhaseUsesRoundStart ==
  ImplementationActions(RecordNextPhaseUsesLastMarker) =
    SpecActions(RecordNextPhaseUsesLastMarker)

BugViewAgeWrongHeightSome ==
  ImplementationActions(ViewAgeWrongHeightReturnsNone) =
    SpecActions(ViewAgeWrongHeightReturnsNone)

BugViewAgeMatchingHeightNone ==
  ImplementationActions(ViewAgeMatchingHeightReturnsDuration) =
    SpecActions(ViewAgeMatchingHeightReturnsDuration)

BugCurrentViewWrongHeightSome ==
  ImplementationActions(CurrentViewWrongHeightReturnsNone) =
    SpecActions(CurrentViewWrongHeightReturnsNone)

BugCurrentViewMatchingHeightNone ==
  ImplementationActions(CurrentViewMatchingHeightReturnsView) =
    SpecActions(CurrentViewMatchingHeightReturnsView)

====
