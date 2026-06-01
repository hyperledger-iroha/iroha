---- MODULE SumeragiModeStatusGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi mode/PRF/mode-flip status projection.

This slice captures `set_prf_context(...)`, `prf_context()`,
`set_mode_tags(...)`, `set_mode_activation_lag(...)`,
`set_mode_flip_kill_switch(...)`, `mode_flip_blocked()`,
`clear_mode_flip_blocked()`, `note_mode_flip_success(...)`,
`note_mode_flip_failure(...)`, `note_mode_flip_blocked(...)`, and their
`snapshot()` projection fields. It focuses on status observability; actor-side
mode-flip decisions are covered by `SumeragiModeFlipGate`.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLE
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

PrfInitialEmpty == 1
PrfStoresSeed == 2
PrfStoresHeightView == 3
StatusProjectsPrf == 4
ModeStoresCurrent == 5
ModeStoresStaged == 6
ModeStoresActivation == 7
LagNoneClears == 8
LagSomeStores == 9
StatusProjectsModeTags == 10
KillSwitchFalseProjects == 11
KillSwitchTrueProjects == 12
BlockedEventCounts == 13
BlockedEventState == 14
BlockedEventLastFields == 15
FailureEventCounts == 16
FailureEventClearsBlocked == 17
FailureEventLastFields == 18
SuccessEventCounts == 19
SuccessEventClearsBlocked == 20
SuccessEventLastFields == 21
ClearBlockedFlag == 22
RepeatedSuccessAccumulates == 23
RepeatedFailureAccumulates == 24
RepeatedBlockedAccumulates == 25
StatusProjectsModeFlipCounters == 26
StatusProjectsModeFlipLast == 27

Candidates == 1..27

PrfSeedAbsentInitially == 1
PrfHeightZeroInitially == 2
PrfViewZeroInitially == 3
PrfSeedStored == 4
PrfHeightStored == 5
PrfViewStored == 6
StatusPrfSeedMatches == 7
StatusPrfHeightMatches == 8
StatusPrfViewMatches == 9
ModeCurrentStored == 10
ModeStagedStored == 11
ModeActivationStored == 12
ModeLagCleared == 13
ModeLagStored == 14
StatusModeCurrentMatches == 15
StatusModeStagedMatches == 16
StatusModeActivationMatches == 17
StatusModeLagMatches == 18
KillSwitchStoredFalse == 19
KillSwitchStoredTrue == 20
StatusKillSwitchMatches == 21
BlockedCounterIncrement == 22
BlockedFlagSet == 23
BlockedTimestampStored == 24
BlockedErrorStored == 25
FailureCounterIncrement == 26
FailureClearsBlocked == 27
FailureTimestampStored == 28
FailureErrorStored == 29
SuccessCounterIncrement == 30
SuccessClearsBlocked == 31
SuccessTimestampStored == 32
SuccessErrorCleared == 33
ClearBlockedStoresFalse == 34
RepeatedSuccessAdds == 35
RepeatedFailureAdds == 36
RepeatedBlockedAdds == 37
StatusSuccessCounterMatches == 38
StatusFailureCounterMatches == 39
StatusBlockedCounterMatches == 40
StatusBlockedFlagMatches == 41
StatusLastTimestampMatches == 42
StatusLastErrorMatches == 43

PrfInitialActions ==
  {PrfSeedAbsentInitially, PrfHeightZeroInitially, PrfViewZeroInitially}
PrfStoreActions == {PrfSeedStored, PrfHeightStored, PrfViewStored}
StatusPrfActions ==
  {StatusPrfSeedMatches, StatusPrfHeightMatches, StatusPrfViewMatches}
ModeTagActions ==
  {ModeCurrentStored, ModeStagedStored, ModeActivationStored}
StatusModeActions ==
  {StatusModeCurrentMatches, StatusModeStagedMatches,
   StatusModeActivationMatches, StatusModeLagMatches}
BlockedLastActions == {BlockedTimestampStored, BlockedErrorStored}
FailureLastActions == {FailureTimestampStored, FailureErrorStored}
SuccessLastActions == {SuccessTimestampStored, SuccessErrorCleared}
StatusCounterActions ==
  {StatusSuccessCounterMatches, StatusFailureCounterMatches,
   StatusBlockedCounterMatches, StatusBlockedFlagMatches}
StatusLastActions == {StatusLastTimestampMatches, StatusLastErrorMatches}

SpecActions(candidate) ==
  CASE candidate = PrfInitialEmpty ->
      PrfInitialActions
    [] candidate = PrfStoresSeed ->
      {PrfSeedStored}
    [] candidate = PrfStoresHeightView ->
      {PrfHeightStored, PrfViewStored}
    [] candidate = StatusProjectsPrf ->
      PrfStoreActions \cup StatusPrfActions
    [] candidate = ModeStoresCurrent ->
      {ModeCurrentStored}
    [] candidate = ModeStoresStaged ->
      {ModeStagedStored}
    [] candidate = ModeStoresActivation ->
      {ModeActivationStored}
    [] candidate = LagNoneClears ->
      {ModeLagCleared}
    [] candidate = LagSomeStores ->
      {ModeLagStored}
    [] candidate = StatusProjectsModeTags ->
      ModeTagActions \cup {ModeLagStored} \cup StatusModeActions
    [] candidate = KillSwitchFalseProjects ->
      {KillSwitchStoredFalse, StatusKillSwitchMatches}
    [] candidate = KillSwitchTrueProjects ->
      {KillSwitchStoredTrue, StatusKillSwitchMatches}
    [] candidate = BlockedEventCounts ->
      {BlockedCounterIncrement}
    [] candidate = BlockedEventState ->
      {BlockedFlagSet}
    [] candidate = BlockedEventLastFields ->
      BlockedLastActions
    [] candidate = FailureEventCounts ->
      {FailureCounterIncrement}
    [] candidate = FailureEventClearsBlocked ->
      {FailureClearsBlocked}
    [] candidate = FailureEventLastFields ->
      FailureLastActions
    [] candidate = SuccessEventCounts ->
      {SuccessCounterIncrement}
    [] candidate = SuccessEventClearsBlocked ->
      {SuccessClearsBlocked}
    [] candidate = SuccessEventLastFields ->
      SuccessLastActions
    [] candidate = ClearBlockedFlag ->
      {ClearBlockedStoresFalse}
    [] candidate = RepeatedSuccessAccumulates ->
      {SuccessCounterIncrement, RepeatedSuccessAdds}
    [] candidate = RepeatedFailureAccumulates ->
      {FailureCounterIncrement, RepeatedFailureAdds}
    [] candidate = RepeatedBlockedAccumulates ->
      {BlockedCounterIncrement, RepeatedBlockedAdds}
    [] candidate = StatusProjectsModeFlipCounters ->
      StatusCounterActions
    [] candidate = StatusProjectsModeFlipLast ->
      StatusLastActions
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = PrfInitialEmpty /\ Bug = "prf_initial_seed_set" ->
      spec \ {PrfSeedAbsentInitially}
    [] candidate = PrfStoresSeed /\ Bug = "prf_seed_not_stored" ->
      spec \ {PrfSeedStored}
    [] candidate = PrfStoresHeightView /\ Bug = "prf_height_view_not_stored" ->
      spec \ {PrfHeightStored, PrfViewStored}
    [] candidate = StatusProjectsPrf /\ Bug = "status_prf_dropped" ->
      spec \ StatusPrfActions
    [] candidate = ModeStoresCurrent /\ Bug = "mode_current_not_stored" ->
      spec \ {ModeCurrentStored}
    [] candidate = ModeStoresStaged /\ Bug = "mode_staged_not_stored" ->
      spec \ {ModeStagedStored}
    [] candidate = ModeStoresActivation /\ Bug = "mode_activation_not_stored" ->
      spec \ {ModeActivationStored}
    [] candidate = LagNoneClears /\ Bug = "lag_none_keeps_old" ->
      spec \ {ModeLagCleared}
    [] candidate = LagSomeStores /\ Bug = "lag_some_not_stored" ->
      spec \ {ModeLagStored}
    [] candidate = StatusProjectsModeTags /\ Bug = "status_mode_dropped" ->
      spec \ StatusModeActions
    [] candidate = KillSwitchFalseProjects /\ Bug = "kill_switch_false_not_stored" ->
      spec \ {KillSwitchStoredFalse, StatusKillSwitchMatches}
    [] candidate = KillSwitchTrueProjects /\ Bug = "kill_switch_true_not_stored" ->
      spec \ {KillSwitchStoredTrue, StatusKillSwitchMatches}
    [] candidate = BlockedEventCounts /\ Bug = "blocked_not_counted" ->
      spec \ {BlockedCounterIncrement}
    [] candidate = BlockedEventState /\ Bug = "blocked_flag_not_set" ->
      spec \ {BlockedFlagSet}
    [] candidate = BlockedEventLastFields /\ Bug = "blocked_timestamp_missing" ->
      spec \ {BlockedTimestampStored}
    [] candidate = BlockedEventLastFields /\ Bug = "blocked_error_missing" ->
      spec \ {BlockedErrorStored}
    [] candidate = FailureEventCounts /\ Bug = "failure_not_counted" ->
      spec \ {FailureCounterIncrement}
    [] candidate = FailureEventClearsBlocked /\ Bug = "failure_keeps_blocked" ->
      spec \ {FailureClearsBlocked}
    [] candidate = FailureEventLastFields /\ Bug = "failure_error_missing" ->
      spec \ {FailureErrorStored}
    [] candidate = SuccessEventCounts /\ Bug = "success_not_counted" ->
      spec \ {SuccessCounterIncrement}
    [] candidate = SuccessEventClearsBlocked /\ Bug = "success_keeps_blocked" ->
      spec \ {SuccessClearsBlocked}
    [] candidate = SuccessEventLastFields /\ Bug = "success_keeps_error" ->
      spec \ {SuccessErrorCleared}
    [] candidate = SuccessEventLastFields /\ Bug = "success_timestamp_missing" ->
      spec \ {SuccessTimestampStored}
    [] candidate = ClearBlockedFlag /\ Bug = "clear_blocked_noop" ->
      spec \ {ClearBlockedStoresFalse}
    [] candidate = RepeatedSuccessAccumulates /\
          Bug = "repeated_success_overwrites" ->
      spec \ {RepeatedSuccessAdds}
    [] candidate = RepeatedFailureAccumulates /\
          Bug = "repeated_failure_overwrites" ->
      spec \ {RepeatedFailureAdds}
    [] candidate = RepeatedBlockedAccumulates /\
          Bug = "repeated_blocked_overwrites" ->
      spec \ {RepeatedBlockedAdds}
    [] candidate = StatusProjectsModeFlipCounters /\
          Bug = "status_counters_mismatch" ->
      spec \ StatusCounterActions
    [] candidate = StatusProjectsModeFlipLast /\ Bug = "status_last_mismatch" ->
      spec \ StatusLastActions
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  /\ checked < 27
  /\ checked' = checked + 1

TypeInvariant ==
  checked \in 0..27

Safety ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

BugPrfInitialSeedSet ==
  ImplementationActions(PrfInitialEmpty) = SpecActions(PrfInitialEmpty)

BugPrfSeedNotStored ==
  ImplementationActions(PrfStoresSeed) = SpecActions(PrfStoresSeed)

BugPrfHeightViewNotStored ==
  ImplementationActions(PrfStoresHeightView) =
    SpecActions(PrfStoresHeightView)

BugStatusPrfDropped ==
  ImplementationActions(StatusProjectsPrf) = SpecActions(StatusProjectsPrf)

BugModeCurrentNotStored ==
  ImplementationActions(ModeStoresCurrent) = SpecActions(ModeStoresCurrent)

BugModeStagedNotStored ==
  ImplementationActions(ModeStoresStaged) = SpecActions(ModeStoresStaged)

BugModeActivationNotStored ==
  ImplementationActions(ModeStoresActivation) =
    SpecActions(ModeStoresActivation)

BugLagNoneKeepsOld ==
  ImplementationActions(LagNoneClears) = SpecActions(LagNoneClears)

BugLagSomeNotStored ==
  ImplementationActions(LagSomeStores) = SpecActions(LagSomeStores)

BugStatusModeDropped ==
  ImplementationActions(StatusProjectsModeTags) =
    SpecActions(StatusProjectsModeTags)

BugKillSwitchFalseNotStored ==
  ImplementationActions(KillSwitchFalseProjects) =
    SpecActions(KillSwitchFalseProjects)

BugKillSwitchTrueNotStored ==
  ImplementationActions(KillSwitchTrueProjects) =
    SpecActions(KillSwitchTrueProjects)

BugBlockedNotCounted ==
  ImplementationActions(BlockedEventCounts) = SpecActions(BlockedEventCounts)

BugBlockedFlagNotSet ==
  ImplementationActions(BlockedEventState) = SpecActions(BlockedEventState)

BugBlockedTimestampMissing ==
  ImplementationActions(BlockedEventLastFields) =
    SpecActions(BlockedEventLastFields)

BugBlockedErrorMissing ==
  ImplementationActions(BlockedEventLastFields) =
    SpecActions(BlockedEventLastFields)

BugFailureNotCounted ==
  ImplementationActions(FailureEventCounts) = SpecActions(FailureEventCounts)

BugFailureKeepsBlocked ==
  ImplementationActions(FailureEventClearsBlocked) =
    SpecActions(FailureEventClearsBlocked)

BugFailureErrorMissing ==
  ImplementationActions(FailureEventLastFields) =
    SpecActions(FailureEventLastFields)

BugSuccessNotCounted ==
  ImplementationActions(SuccessEventCounts) = SpecActions(SuccessEventCounts)

BugSuccessKeepsBlocked ==
  ImplementationActions(SuccessEventClearsBlocked) =
    SpecActions(SuccessEventClearsBlocked)

BugSuccessKeepsError ==
  ImplementationActions(SuccessEventLastFields) =
    SpecActions(SuccessEventLastFields)

BugSuccessTimestampMissing ==
  ImplementationActions(SuccessEventLastFields) =
    SpecActions(SuccessEventLastFields)

BugClearBlockedNoop ==
  ImplementationActions(ClearBlockedFlag) = SpecActions(ClearBlockedFlag)

BugRepeatedSuccessOverwrites ==
  ImplementationActions(RepeatedSuccessAccumulates) =
    SpecActions(RepeatedSuccessAccumulates)

BugRepeatedFailureOverwrites ==
  ImplementationActions(RepeatedFailureAccumulates) =
    SpecActions(RepeatedFailureAccumulates)

BugRepeatedBlockedOverwrites ==
  ImplementationActions(RepeatedBlockedAccumulates) =
    SpecActions(RepeatedBlockedAccumulates)

BugStatusCountersMismatch ==
  ImplementationActions(StatusProjectsModeFlipCounters) =
    SpecActions(StatusProjectsModeFlipCounters)

BugStatusLastMismatch ==
  ImplementationActions(StatusProjectsModeFlipLast) =
    SpecActions(StatusProjectsModeFlipLast)

=============================================================================
====
