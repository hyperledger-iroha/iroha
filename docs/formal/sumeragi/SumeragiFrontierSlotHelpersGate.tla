---- MODULE SumeragiFrontierSlotHelpersGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for the small `FrontierSlot` helper methods.

This slice covers helper semantics in `main_loop/slot_tracker.rs` that feed the
exact-frontier slot FSM: lag-start fallback, body-state predicates, local vote
locking, timeout view selection, progress/lag timer updates, catch-up markers,
and compatibility mirror synchronization. Concrete hashes, peers, instants,
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
SyncCandidateMirrors == 17
SyncTimerMirrors == 18
SyncRepairMirrors == 19

Candidates == 1..19

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
MirrorCandidateView == 26
MirrorCandidateHash == 27
MirrorBodyPresent == 28
MirrorBlockCreatedSeen == 29
MirrorExactFetchArmed == 30
MirrorFrontierInfo == 31
MirrorLeader == 32
MirrorVoters == 33
MirrorObservedAt == 34
MirrorLastUpdatedAt == 35
MirrorLastFetchAt == 36
MirrorFetchStage == 37
MirrorRetryWindow == 38
MirrorPendingRequesters == 39

Actions == 1..39

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
    [] candidate = SyncCandidateMirrors ->
      {MirrorCandidateView, MirrorCandidateHash, MirrorBodyPresent,
       MirrorBlockCreatedSeen, MirrorExactFetchArmed, MirrorFrontierInfo,
       MirrorLeader, MirrorVoters}
    [] candidate = SyncTimerMirrors ->
      {MirrorObservedAt, MirrorLastUpdatedAt, MirrorLastFetchAt}
    [] candidate = SyncRepairMirrors ->
      {MirrorFetchStage, MirrorRetryWindow, MirrorPendingRequesters}
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
    [] candidate = SyncCandidateMirrors /\ Bug = "sync_skips_candidate" ->
      spec \ {MirrorCandidateView, MirrorCandidateHash}
    [] candidate = SyncCandidateMirrors /\ Bug = "sync_skips_body_state" ->
      spec \ {MirrorBodyPresent}
    [] candidate = SyncCandidateMirrors /\ Bug = "sync_skips_peer_sets" ->
      spec \ {MirrorLeader, MirrorVoters}
    [] candidate = SyncTimerMirrors /\ Bug = "sync_skips_timers" ->
      spec \ {MirrorObservedAt, MirrorLastUpdatedAt}
    [] candidate = SyncRepairMirrors /\ Bug = "sync_skips_repair" ->
      spec \ {MirrorFetchStage, MirrorRetryWindow}
    [] candidate = SyncRepairMirrors /\ Bug = "sync_skips_requesters" ->
      spec \ {MirrorPendingRequesters}
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
  "sync_skips_candidate",
  "sync_skips_body_state",
  "sync_skips_peer_sets",
  "sync_skips_timers",
  "sync_skips_repair",
  "sync_skips_requesters"
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

SyncMirrorMatchesSpec ==
  \A candidate \in {
    SyncCandidateMirrors,
    SyncTimerMirrors,
    SyncRepairMirrors
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

SyncMirrorsNestedState ==
  \A candidate \in Candidates:
    MirrorCandidateView \in ImplementationActions(candidate) =>
      /\ MirrorCandidateHash \in ImplementationActions(candidate)
      /\ MirrorBodyPresent \in ImplementationActions(candidate)
      /\ MirrorBlockCreatedSeen \in ImplementationActions(candidate)
      /\ MirrorExactFetchArmed \in ImplementationActions(candidate)

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

SyncMirrorAnchors ==
  /\ SpecActions(SyncCandidateMirrors) =
       {MirrorCandidateView, MirrorCandidateHash, MirrorBodyPresent,
        MirrorBlockCreatedSeen, MirrorExactFetchArmed, MirrorFrontierInfo,
        MirrorLeader, MirrorVoters}
  /\ SpecActions(SyncTimerMirrors) =
       {MirrorObservedAt, MirrorLastUpdatedAt, MirrorLastFetchAt}
  /\ SpecActions(SyncRepairMirrors) =
       {MirrorFetchStage, MirrorRetryWindow, MirrorPendingRequesters}

Safety ==
  /\ FrontierSlotHelpersMatchSpec
  /\ LagStartMatchesSpec
  /\ BodyStateMatchesSpec
  /\ LocalVoteMatchesSpec
  /\ TimeoutViewMatchesSpec
  /\ ProgressAndLagMatchesSpec
  /\ CatchupMarkerMatchesSpec
  /\ SyncMirrorMatchesSpec
  /\ CatchupMarkersClearRebroadcast
  /\ CatchupMarkersRecordReason
  /\ SyncMirrorsNestedState
  /\ LagStartAnchors
  /\ BodyStateAnchors
  /\ LocalVoteAnchors
  /\ TimeoutViewAnchors
  /\ ProgressAndLagAnchors
  /\ CatchupMarkerAnchors
  /\ SyncMirrorAnchors

=============================================================================
====
