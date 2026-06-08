---- MODULE SumeragiFrontierSlotHelpersGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the small `FrontierSlot` helper methods.

This slice covers helper semantics in `main_loop/slot_tracker.rs` that feed the
exact-frontier slot FSM: lag-start fallback, body-state predicates, local vote
locking, timeout view selection, progress/lag timer updates, catch-up markers,
and nested slot-state source consistency. Concrete hashes, peers, instants,
durations, and nested structs are collapsed into representative obligations.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

LagStartedUsesExistingLag == 1
LagStartedFallsBackProgress == 2
BodyMissingTrueForMissing == 3
BodyMissingFalseForAvailable == 4
LocalVoteLocksOnly == 5
TimeoutViewUsesActive == 6
TimeoutViewUsesRequested == 7
TimeoutViewUsesCandidate == 8
RecordProgressRefreshesTimers == 9
RecordProgressClearsLagAndRebroadcast == 10
NoteLagSetsAbsent == 11
NoteLagPreservesExisting == 12
MarkDeepCatchupState == 13
MarkDeepCatchupPreservesSlot == 14
MarkPassiveCatchupState == 15
MarkPassiveCatchupPreservesSlot == 16
NestedCandidateState == 17
NestedTimerState == 18
NestedRepairState == 19
MarkBodyAvailableUnknownState == 20
MarkBodyAvailablePendingState == 21

Candidates == 1..21

ReturnLagWindowStart == 1
ReturnLastProgressAt == 2
ReturnBodyMissingTrue == 3
ReturnBodyMissingFalse == 4
SetLocalVoteLock == 5
PreserveCandidate == 6
PreserveTimers == 7
ReturnActiveView == 8
ReturnRequestedView == 9
ReturnCandidateView == 10
SetLastProgressNow == 11
SetLastUpdatedNow == 12
ClearLagWindow == 13
ClearQuorumRebroadcast == 14
SetLagWindowNow == 15
KeepExistingLagWindow == 16
SetOwnerExactRepair == 17
SetModeDeepCatchup == 18
SetOwnerPassiveRetainedPayload == 19
SetModePassiveCatchup == 20
SetDeepCatchupReason == 21
SetLastReason == 22
SetDeepEnteredNow == 23
PreserveActiveView == 24
PreservePhase == 25
NestedCandidateView == 26
NestedCandidateHash == 27
NestedBodyPresent == 28
NestedBlockCreatedSeen == 29
NestedExactFetchArmed == 30
NestedFrontierInfo == 31
NestedLeader == 32
NestedVoters == 33
NestedObservedAt == 34
NestedLastUpdatedAt == 35
NestedLastFetchAt == 36
NestedFetchStage == 37
NestedRetryWindow == 38
NestedPendingRequesters == 39
SetBodyAvailable == 40
SetValidationPending == 41
PreservePendingValidation == 42
SetPhaseValidateBody == 43

Actions == 1..43

SpecActions(candidate) ==
  CASE candidate = LagStartedUsesExistingLag -> {ReturnLagWindowStart}
    [] candidate = LagStartedFallsBackProgress -> {ReturnLastProgressAt}
    [] candidate = BodyMissingTrueForMissing -> {ReturnBodyMissingTrue}
    [] candidate = BodyMissingFalseForAvailable -> {ReturnBodyMissingFalse}
    [] candidate = LocalVoteLocksOnly ->
      {SetLocalVoteLock, PreserveCandidate, PreserveTimers}
    [] candidate = TimeoutViewUsesActive -> {ReturnActiveView}
    [] candidate = TimeoutViewUsesRequested -> {ReturnRequestedView}
    [] candidate = TimeoutViewUsesCandidate -> {ReturnCandidateView}
    [] candidate = RecordProgressRefreshesTimers ->
      {SetLastProgressNow, SetLastUpdatedNow}
    [] candidate = RecordProgressClearsLagAndRebroadcast ->
      {ClearLagWindow, ClearQuorumRebroadcast}
    [] candidate = NoteLagSetsAbsent -> {SetLagWindowNow, PreserveTimers}
    [] candidate = NoteLagPreservesExisting ->
      {KeepExistingLagWindow, PreserveTimers}
    [] candidate = MarkDeepCatchupState ->
      {SetOwnerExactRepair, SetModeDeepCatchup, SetDeepCatchupReason,
       SetLastReason, ClearQuorumRebroadcast, SetDeepEnteredNow,
       SetLastUpdatedNow}
    [] candidate = MarkDeepCatchupPreservesSlot ->
      {PreserveCandidate, PreserveActiveView, PreservePhase}
    [] candidate = MarkPassiveCatchupState ->
      {SetOwnerPassiveRetainedPayload, SetModePassiveCatchup,
       SetDeepCatchupReason, SetLastReason, ClearQuorumRebroadcast,
       SetDeepEnteredNow, SetLastUpdatedNow}
    [] candidate = MarkPassiveCatchupPreservesSlot ->
      {PreserveCandidate, PreserveActiveView, PreservePhase}
    [] candidate = NestedCandidateState ->
      {NestedCandidateView, NestedCandidateHash, NestedBodyPresent,
       NestedBlockCreatedSeen, NestedExactFetchArmed, NestedFrontierInfo,
       NestedLeader, NestedVoters}
    [] candidate = NestedTimerState ->
      {NestedObservedAt, NestedLastUpdatedAt, NestedLastFetchAt}
    [] candidate = NestedRepairState ->
      {NestedFetchStage, NestedRetryWindow, NestedPendingRequesters}
    [] candidate = MarkBodyAvailableUnknownState ->
      {SetBodyAvailable, SetValidationPending, SetPhaseValidateBody,
       SetLastProgressNow, SetLastUpdatedNow, ClearLagWindow,
       ClearQuorumRebroadcast}
    [] candidate = MarkBodyAvailablePendingState ->
      {SetBodyAvailable, PreservePendingValidation, SetPhaseValidateBody,
       SetLastProgressNow, SetLastUpdatedNow, ClearLagWindow,
       ClearQuorumRebroadcast}
    [] OTHER -> {}

ImplementationActions(candidate) ==
  LET spec == SpecActions(candidate) IN
  CASE candidate = LagStartedUsesExistingLag /\ Bug = "lag_uses_progress" ->
      (spec \ {ReturnLagWindowStart}) \cup {ReturnLastProgressAt}
    [] candidate = LagStartedFallsBackProgress /\
          Bug = "lag_missing_returns_zero" ->
      spec \ {ReturnLastProgressAt}
    [] candidate = BodyMissingTrueForMissing /\
          Bug = "body_missing_missing_false" ->
      (spec \ {ReturnBodyMissingTrue}) \cup {ReturnBodyMissingFalse}
    [] candidate = BodyMissingFalseForAvailable /\
          Bug = "body_missing_available_true" ->
      (spec \ {ReturnBodyMissingFalse}) \cup {ReturnBodyMissingTrue}
    [] candidate = LocalVoteLocksOnly /\ Bug = "local_vote_does_not_lock" ->
      spec \ {SetLocalVoteLock}
    [] candidate = LocalVoteLocksOnly /\ Bug = "local_vote_mutates_candidate" ->
      spec \ {PreserveCandidate}
    [] candidate = TimeoutViewUsesActive /\
          Bug = "current_view_ignores_active" ->
      (spec \ {ReturnActiveView}) \cup {ReturnRequestedView}
    [] candidate = TimeoutViewUsesRequested /\
          Bug = "current_view_ignores_requested" ->
      (spec \ {ReturnRequestedView}) \cup {ReturnActiveView}
    [] candidate = TimeoutViewUsesCandidate /\
          Bug = "current_view_ignores_candidate" ->
      (spec \ {ReturnCandidateView}) \cup {ReturnRequestedView}
    [] candidate = RecordProgressRefreshesTimers /\
          Bug = "record_progress_skips_last_progress" ->
      spec \ {SetLastProgressNow}
    [] candidate = RecordProgressRefreshesTimers /\
          Bug = "record_progress_skips_last_updated" ->
      spec \ {SetLastUpdatedNow}
    [] candidate = RecordProgressClearsLagAndRebroadcast /\
          Bug = "record_progress_keeps_lag" ->
      spec \ {ClearLagWindow}
    [] candidate = RecordProgressClearsLagAndRebroadcast /\
          Bug = "record_progress_keeps_rebroadcast" ->
      spec \ {ClearQuorumRebroadcast}
    [] candidate = NoteLagSetsAbsent /\ Bug = "note_lag_missing_not_set" ->
      spec \ {SetLagWindowNow}
    [] candidate = NoteLagPreservesExisting /\
          Bug = "note_lag_overwrites_existing" ->
      (spec \ {KeepExistingLagWindow}) \cup {SetLagWindowNow}
    [] candidate = MarkDeepCatchupState /\ Bug = "mark_deep_wrong_owner" ->
      (spec \ {SetOwnerExactRepair}) \cup {SetOwnerPassiveRetainedPayload}
    [] candidate = MarkDeepCatchupState /\ Bug = "mark_deep_wrong_mode" ->
      (spec \ {SetModeDeepCatchup}) \cup {SetModePassiveCatchup}
    [] candidate = MarkDeepCatchupState /\ Bug = "mark_deep_keeps_rebroadcast" ->
      spec \ {ClearQuorumRebroadcast}
    [] candidate = MarkDeepCatchupState /\ Bug = "mark_deep_skips_reason" ->
      spec \ {SetDeepCatchupReason, SetLastReason}
    [] candidate = MarkDeepCatchupPreservesSlot /\
          Bug = "mark_deep_mutates_candidate" ->
      spec \ {PreserveCandidate}
    [] candidate = MarkPassiveCatchupState /\ Bug = "mark_passive_wrong_owner" ->
      (spec \ {SetOwnerPassiveRetainedPayload}) \cup {SetOwnerExactRepair}
    [] candidate = MarkPassiveCatchupState /\ Bug = "mark_passive_wrong_mode" ->
      (spec \ {SetModePassiveCatchup}) \cup {SetModeDeepCatchup}
    [] candidate = MarkPassiveCatchupState /\
          Bug = "mark_passive_keeps_rebroadcast" ->
      spec \ {ClearQuorumRebroadcast}
    [] candidate = MarkPassiveCatchupState /\ Bug = "mark_passive_skips_reason" ->
      spec \ {SetDeepCatchupReason, SetLastReason}
    [] candidate = MarkPassiveCatchupPreservesSlot /\
          Bug = "mark_passive_mutates_phase" ->
      spec \ {PreservePhase}
    [] candidate = NestedCandidateState /\ Bug = "nested_skips_candidate" ->
      spec \ {NestedCandidateView, NestedCandidateHash}
    [] candidate = NestedCandidateState /\ Bug = "nested_skips_body_state" ->
      spec \ {NestedBodyPresent}
    [] candidate = NestedCandidateState /\ Bug = "nested_skips_peer_sets" ->
      spec \ {NestedLeader, NestedVoters}
    [] candidate = NestedTimerState /\ Bug = "nested_skips_timers" ->
      spec \ {NestedObservedAt, NestedLastUpdatedAt}
    [] candidate = NestedRepairState /\ Bug = "nested_skips_repair" ->
      spec \ {NestedFetchStage, NestedRetryWindow}
    [] candidate = NestedRepairState /\ Bug = "nested_skips_requesters" ->
      spec \ {NestedPendingRequesters}
    [] candidate = MarkBodyAvailableUnknownState /\
          Bug = "body_available_skips_body_state" ->
      spec \ {SetBodyAvailable}
    [] candidate = MarkBodyAvailableUnknownState /\
          Bug = "body_available_unknown_skips_validation" ->
      spec \ {SetValidationPending}
    [] candidate = MarkBodyAvailablePendingState /\
          Bug = "body_available_pending_clears_validation" ->
      spec \ {PreservePendingValidation}
    [] candidate = MarkBodyAvailableUnknownState /\
          Bug = "body_available_skips_phase" ->
      spec \ {SetPhaseValidateBody}
    [] candidate = MarkBodyAvailableUnknownState /\
          Bug = "body_available_skips_record_progress" ->
      spec \ {SetLastProgressNow, SetLastUpdatedNow, ClearLagWindow,
              ClearQuorumRebroadcast}
    [] OTHER -> spec

Init ==
  checked = 0

Next ==
  UNCHANGED vars

Bugs == {
  "none",
  "lag_uses_progress",
  "lag_missing_returns_zero",
  "body_missing_missing_false",
  "body_missing_available_true",
  "local_vote_does_not_lock",
  "local_vote_mutates_candidate",
  "current_view_ignores_active",
  "current_view_ignores_requested",
  "current_view_ignores_candidate",
  "record_progress_skips_last_progress",
  "record_progress_skips_last_updated",
  "record_progress_keeps_lag",
  "record_progress_keeps_rebroadcast",
  "note_lag_missing_not_set",
  "note_lag_overwrites_existing",
  "mark_deep_wrong_owner",
  "mark_deep_wrong_mode",
  "mark_deep_keeps_rebroadcast",
  "mark_deep_skips_reason",
  "mark_deep_mutates_candidate",
  "mark_passive_wrong_owner",
  "mark_passive_wrong_mode",
  "mark_passive_keeps_rebroadcast",
  "mark_passive_skips_reason",
  "mark_passive_mutates_phase",
  "nested_skips_candidate",
  "nested_skips_body_state",
  "nested_skips_peer_sets",
  "nested_skips_timers",
  "nested_skips_repair",
  "nested_skips_requesters",
  "body_available_skips_body_state",
  "body_available_unknown_skips_validation",
  "body_available_pending_clears_validation",
  "body_available_skips_phase",
  "body_available_skips_record_progress"
}

TypeInvariant ==
  /\ Bug \in Bugs
  /\ checked = 0
  /\ \A candidate \in Candidates:
       ImplementationActions(candidate) \subseteq Actions

FrontierSlotHelpersMatchSpec ==
  \A candidate \in Candidates:
    ImplementationActions(candidate) = SpecActions(candidate)

LagStartMatchesSpec ==
  \A candidate \in {
    LagStartedUsesExistingLag,
    LagStartedFallsBackProgress
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

BodyStateMatchesSpec ==
  \A candidate \in {
    BodyMissingTrueForMissing,
    BodyMissingFalseForAvailable
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

LocalVoteMatchesSpec ==
  ImplementationActions(LocalVoteLocksOnly) = SpecActions(LocalVoteLocksOnly)

TimeoutViewMatchesSpec ==
  \A candidate \in {
    TimeoutViewUsesActive,
    TimeoutViewUsesRequested,
    TimeoutViewUsesCandidate
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

ProgressAndLagMatchesSpec ==
  \A candidate \in {
    RecordProgressRefreshesTimers,
    RecordProgressClearsLagAndRebroadcast,
    NoteLagSetsAbsent,
    NoteLagPreservesExisting
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

CatchupMarkerMatchesSpec ==
  \A candidate \in {
    MarkDeepCatchupState,
    MarkDeepCatchupPreservesSlot,
    MarkPassiveCatchupState,
    MarkPassiveCatchupPreservesSlot
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

NestedStateMatchesSpec ==
  \A candidate \in {
    NestedCandidateState,
    NestedTimerState,
    NestedRepairState
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

BodyAvailableMatchesSpec ==
  \A candidate \in {
    MarkBodyAvailableUnknownState,
    MarkBodyAvailablePendingState
  }:
    ImplementationActions(candidate) = SpecActions(candidate)

CatchupMarkersClearRebroadcast ==
  \A candidate \in Candidates:
    (SetModeDeepCatchup \in ImplementationActions(candidate) \/
     SetModePassiveCatchup \in ImplementationActions(candidate)) =>
      ClearQuorumRebroadcast \in ImplementationActions(candidate)

CatchupMarkersRecordReason ==
  \A candidate \in Candidates:
    (SetModeDeepCatchup \in ImplementationActions(candidate) \/
     SetModePassiveCatchup \in ImplementationActions(candidate)) =>
      /\ SetDeepCatchupReason \in ImplementationActions(candidate)
      /\ SetLastReason \in ImplementationActions(candidate)

NestedStateCompleteness ==
  \A candidate \in Candidates:
    NestedCandidateView \in ImplementationActions(candidate) =>
      /\ NestedCandidateHash \in ImplementationActions(candidate)
      /\ NestedBodyPresent \in ImplementationActions(candidate)
      /\ NestedBlockCreatedSeen \in ImplementationActions(candidate)
      /\ NestedExactFetchArmed \in ImplementationActions(candidate)

LagStartAnchors ==
  /\ SpecActions(LagStartedUsesExistingLag) = {ReturnLagWindowStart}
  /\ SpecActions(LagStartedFallsBackProgress) = {ReturnLastProgressAt}

BodyStateAnchors ==
  /\ SpecActions(BodyMissingTrueForMissing) = {ReturnBodyMissingTrue}
  /\ SpecActions(BodyMissingFalseForAvailable) = {ReturnBodyMissingFalse}

LocalVoteAnchors ==
  /\ SpecActions(LocalVoteLocksOnly) =
       {SetLocalVoteLock, PreserveCandidate, PreserveTimers}

TimeoutViewAnchors ==
  /\ SpecActions(TimeoutViewUsesActive) = {ReturnActiveView}
  /\ SpecActions(TimeoutViewUsesRequested) = {ReturnRequestedView}
  /\ SpecActions(TimeoutViewUsesCandidate) = {ReturnCandidateView}

ProgressAndLagAnchors ==
  /\ SpecActions(RecordProgressRefreshesTimers) =
       {SetLastProgressNow, SetLastUpdatedNow}
  /\ SpecActions(RecordProgressClearsLagAndRebroadcast) =
       {ClearLagWindow, ClearQuorumRebroadcast}
  /\ SpecActions(NoteLagSetsAbsent) = {SetLagWindowNow, PreserveTimers}
  /\ SpecActions(NoteLagPreservesExisting) =
       {KeepExistingLagWindow, PreserveTimers}

CatchupMarkerAnchors ==
  /\ SpecActions(MarkDeepCatchupState) =
       {SetOwnerExactRepair, SetModeDeepCatchup, SetDeepCatchupReason,
        SetLastReason, ClearQuorumRebroadcast, SetDeepEnteredNow,
        SetLastUpdatedNow}
  /\ SpecActions(MarkDeepCatchupPreservesSlot) =
       {PreserveCandidate, PreserveActiveView, PreservePhase}
  /\ SpecActions(MarkPassiveCatchupState) =
       {SetOwnerPassiveRetainedPayload, SetModePassiveCatchup,
        SetDeepCatchupReason, SetLastReason, ClearQuorumRebroadcast,
        SetDeepEnteredNow, SetLastUpdatedNow}
  /\ SpecActions(MarkPassiveCatchupPreservesSlot) =
       {PreserveCandidate, PreserveActiveView, PreservePhase}

NestedStateAnchors ==
  /\ SpecActions(NestedCandidateState) =
       {NestedCandidateView, NestedCandidateHash, NestedBodyPresent,
        NestedBlockCreatedSeen, NestedExactFetchArmed, NestedFrontierInfo,
        NestedLeader, NestedVoters}
  /\ SpecActions(NestedTimerState) =
       {NestedObservedAt, NestedLastUpdatedAt, NestedLastFetchAt}
  /\ SpecActions(NestedRepairState) =
       {NestedFetchStage, NestedRetryWindow, NestedPendingRequesters}

BodyAvailableAnchors ==
  /\ SpecActions(MarkBodyAvailableUnknownState) =
       {SetBodyAvailable, SetValidationPending, SetPhaseValidateBody,
        SetLastProgressNow, SetLastUpdatedNow, ClearLagWindow,
        ClearQuorumRebroadcast}
  /\ SpecActions(MarkBodyAvailablePendingState) =
       {SetBodyAvailable, PreservePendingValidation, SetPhaseValidateBody,
        SetLastProgressNow, SetLastUpdatedNow, ClearLagWindow,
        ClearQuorumRebroadcast}

FrontierSlotBasicHelperExact ==
  /\ LagStartMatchesSpec
  /\ BodyStateMatchesSpec
  /\ LocalVoteMatchesSpec
  /\ TimeoutViewMatchesSpec
  /\ LagStartAnchors
  /\ BodyStateAnchors
  /\ LocalVoteAnchors
  /\ TimeoutViewAnchors

FrontierSlotProgressLagExact ==
  /\ ProgressAndLagMatchesSpec
  /\ ProgressAndLagAnchors

FrontierSlotCatchupMarkerExact ==
  /\ CatchupMarkerMatchesSpec
  /\ CatchupMarkersClearRebroadcast
  /\ CatchupMarkersRecordReason
  /\ CatchupMarkerAnchors

FrontierSlotNestedStateExact ==
  /\ NestedStateMatchesSpec
  /\ NestedStateCompleteness
  /\ NestedStateAnchors

FrontierSlotBodyAvailableExact ==
  /\ BodyAvailableMatchesSpec
  /\ BodyAvailableAnchors

FrontierSlotHelpersExactness ==
  /\ FrontierSlotHelpersMatchSpec
  /\ FrontierSlotBasicHelperExact
  /\ FrontierSlotProgressLagExact
  /\ FrontierSlotCatchupMarkerExact
  /\ FrontierSlotNestedStateExact
  /\ FrontierSlotBodyAvailableExact

Safety ==
  FrontierSlotHelpersExactness

=============================================================================
====
